// SPDX-License-Identifier: GPL-3.0-or-later
//! Lua 5.1 string patterns for `string.find`, `match`, `gmatch` (and its 5.1 alias
//! `gfind`) and `gsub`, written in Rust so hostile patterns stay bounded.
//!
//! The C matcher of the vendored interpreter recurses once per pattern item with no depth
//! limit and backtracks without the VM instruction hook ever running, so a single call could
//! overflow the native stack (aborting the process) or spin forever. This implementation
//! keeps the Lua 5.1 matching rules (classes, sets, `*`/`+`/`-`/`?`, captures, position
//! captures, back-references, `%b`, `%f`, anchors, NUL-terminated patterns) and adds:
//!
//! - a nesting cap of [`MAX_DEPTH`] matcher levels (the limit later Lua versions adopted),
//!   raising `pattern too complex`;
//! - a step counter charged to the apartment's instruction budget, so backtracking counts
//!   like VM instructions and is stopped by the same per-call limit (§3.3);
//! - an output cap for `gsub` so its Rust-side buffer cannot outgrow the apartment's memory
//!   budget;
//! - sets (`[...]`) compiled once into a 256-bit membership table per matcher (a small
//!   direct-mapped cache keyed by the set's position in the pattern), with the compile work
//!   charged to the budget, so a long set costs its length once instead of once per subject
//!   byte.
//!
//! `string.gfind` raises an error naming its replacement, as in the original's build
//! (`plugin-runtime.md` §4.1).
//!
//! Spec: `docs/specs/lua-plugin-api.md` §3.1 (Lua 5.1 semantics), §3.3 (limits), §18
//! invariant 15; `plugin-runtime.md` §4.2 (32 captures).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mlua::{Lua, LuaString, MultiValue, Table, Value};

use crate::budget::Budget;
use crate::native::{host_error, native_fn};

/// Most captures in one pattern (Lua 5.1 value).
pub(crate) const MAX_CAPTURES: usize = 32;

/// Deepest matcher nesting (each capture, `?` hit and quantified item adds one level).
pub(crate) const MAX_DEPTH: usize = 200;

/// Bytes that make `string.find` use the matcher instead of a plain search.
const SPECIALS: &[u8] = b"^$*+?.([%-";

/// Matching steps accumulated before they are charged to the budget.
const CHARGE_EVERY: u64 = 256;

/// Largest `gsub` result when the apartment has no memory cap.
const DEFAULT_MAX_OUTPUT: usize = 1 << 30;

/// Compiled sets kept per matcher (direct-mapped by the set's pattern position).
const SET_SLOTS: usize = 64;

/// Work units of compiling a set that count as one budget unit.
const SET_WORK_PER_UNIT: u64 = 8;

/// A set `[...]` compiled to a membership table.
#[derive(Debug, Clone, Copy)]
struct SetEntry {
    /// Pattern index of the `[` (`usize::MAX`: empty slot).
    start: usize,
    /// Pattern index just past the closing `]`.
    end: usize,
    bits: [u64; 4],
}

const EMPTY_SET: SetEntry = SetEntry {
    start: usize::MAX,
    end: 0,
    bits: [0; 4],
};

fn set_has(bits: &[u64; 4], c: u8) -> bool {
    (bits[usize::from(c >> 6)] >> (c & 63)) & 1 == 1
}

fn set_add(bits: &mut [u64; 4], c: u8) {
    bits[usize::from(c >> 6)] |= 1 << (c & 63);
}

fn perr(msg: &str) -> mlua::Error {
    host_error(msg.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapLen {
    Unclosed,
    Position,
    Len(usize),
}

#[derive(Debug, Clone, Copy)]
struct Capture {
    start: usize,
    len: CapLen,
}

/// A pattern up to its first NUL byte (Lua 5.1 patterns are C strings).
fn until_nul(p: &[u8]) -> &[u8] {
    &p[..p.iter().position(|&b| b == 0).unwrap_or(p.len())]
}

fn match_class(c: u8, cl: u8) -> bool {
    let res = match cl.to_ascii_lowercase() {
        b'a' => c.is_ascii_alphabetic(),
        b'c' => c.is_ascii_control(),
        b'd' => c.is_ascii_digit(),
        b'l' => c.is_ascii_lowercase(),
        b'p' => c.is_ascii_punctuation(),
        b's' => matches!(c, b' ' | 0x09..=0x0D),
        b'u' => c.is_ascii_uppercase(),
        b'w' => c.is_ascii_alphanumeric(),
        b'x' => c.is_ascii_hexdigit(),
        b'z' => c == 0,
        _ => return cl == c,
    };
    if cl.is_ascii_uppercase() { !res } else { res }
}

struct Matcher<'a> {
    src: &'a [u8],
    pat: &'a [u8],
    level: usize,
    caps: [Capture; MAX_CAPTURES],
    depth: usize,
    pending: u64,
    budget: &'a Budget,
    sets: [SetEntry; SET_SLOTS],
}

impl<'a> Matcher<'a> {
    fn new(src: &'a [u8], pat: &'a [u8], budget: &'a Budget) -> Self {
        Self {
            src,
            pat,
            level: 0,
            caps: [Capture {
                start: 0,
                len: CapLen::Unclosed,
            }; MAX_CAPTURES],
            depth: 0,
            pending: 0,
            budget,
            sets: [EMPTY_SET; SET_SLOTS],
        }
    }

    fn tick(&mut self, n: u64) -> mlua::Result<()> {
        self.pending += n;
        if self.pending >= CHARGE_EVERY {
            let p = std::mem::take(&mut self.pending);
            self.budget.charge(p)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> mlua::Result<()> {
        let p = std::mem::take(&mut self.pending);
        if p > 0 { self.budget.charge(p) } else { Ok(()) }
    }

    fn do_match(&mut self, s: usize, p: usize) -> mlua::Result<Option<usize>> {
        if self.depth >= MAX_DEPTH {
            return Err(perr("pattern too complex"));
        }
        self.depth += 1;
        let r = self.match_here(s, p);
        self.depth -= 1;
        r
    }

    /// Index just past the single-character class starting at `p`, and the compiled set
    /// when the class is a set `[...]`.
    fn class_info(&mut self, p: usize) -> mlua::Result<(usize, Option<[u64; 4]>)> {
        let pat = self.pat;
        match pat[p] {
            b'%' => {
                if p + 1 >= pat.len() {
                    Err(perr("malformed pattern (ends with '%')"))
                } else {
                    Ok((p + 2, None))
                }
            }
            b'[' => {
                let set = self.set_at(p)?;
                Ok((set.end, Some(set.bits)))
            }
            _ => Ok((p + 1, None)),
        }
    }

    /// The set starting at `p` (a `[`), compiled on first use in this matcher.
    fn set_at(&mut self, p: usize) -> mlua::Result<SetEntry> {
        let slot = p % SET_SLOTS;
        if self.sets[slot].start == p {
            return Ok(self.sets[slot]);
        }
        let pat = self.pat;
        // The end: the first set character is taken literally (so `[]]` is a set of `]`).
        let mut q = p + 1;
        if pat.get(q) == Some(&b'^') {
            q += 1;
        }
        loop {
            if q >= pat.len() {
                return Err(perr("malformed pattern (missing ']')"));
            }
            let c = pat[q];
            q += 1;
            if c == b'%' && q < pat.len() {
                q += 1;
            }
            if pat.get(q) == Some(&b']') {
                break;
            }
        }
        let ec = q;
        let end = q + 1;
        // The members, with the matching rules of Lua 5.1 sets: `%x` classes, `a-z` ranges
        // and single characters; `^` negates.
        let mut work = (end - p) as u64;
        let mut bits = [0u64; 4];
        let mut q = p + 1;
        let mut negate = false;
        if pat.get(q) == Some(&b'^') {
            negate = true;
            q += 1;
        }
        while q < ec {
            if pat[q] == b'%' {
                q += 1;
                if q < ec {
                    let cl = pat[q];
                    for c in 0..=u8::MAX {
                        if match_class(c, cl) {
                            set_add(&mut bits, c);
                        }
                    }
                    work += 256;
                }
                q += 1;
            } else if pat.get(q + 1) == Some(&b'-') && q + 2 < ec {
                let (lo, hi) = (pat[q], pat[q + 2]);
                if lo <= hi {
                    for c in lo..=hi {
                        set_add(&mut bits, c);
                    }
                    work += u64::from(hi - lo) + 1;
                }
                q += 3;
            } else {
                set_add(&mut bits, pat[q]);
                work += 1;
                q += 1;
            }
        }
        if negate {
            for w in &mut bits {
                *w = !*w;
            }
        }
        self.tick(work / SET_WORK_PER_UNIT + 1)?;
        let entry = SetEntry {
            start: p,
            end,
            bits,
        };
        self.sets[slot] = entry;
        Ok(entry)
    }

    fn single_match(&self, c: u8, p: usize, set: Option<&[u64; 4]>) -> bool {
        match (self.pat[p], set) {
            (_, Some(bits)) => set_has(bits, c),
            (b'.', None) => true,
            (b'%', None) => match_class(c, self.pat[p + 1]),
            (lit, None) => lit == c,
        }
    }

    fn match_here(&mut self, mut s: usize, mut p: usize) -> mlua::Result<Option<usize>> {
        let pat = self.pat;
        let src = self.src;
        loop {
            self.tick(1)?;
            let Some(&pc) = pat.get(p) else {
                return Ok(Some(s));
            };
            match pc {
                b'(' => {
                    return if pat.get(p + 1) == Some(&b')') {
                        self.start_capture(s, p + 2, CapLen::Position)
                    } else {
                        self.start_capture(s, p + 1, CapLen::Unclosed)
                    };
                }
                b')' => return self.end_capture(s, p + 1),
                b'$' if p + 1 == pat.len() => return Ok((s == src.len()).then_some(s)),
                b'%' => match pat.get(p + 1) {
                    Some(b'b') => match self.match_balance(s, p + 2)? {
                        Some(ns) => {
                            s = ns;
                            p += 4;
                            continue;
                        }
                        None => return Ok(None),
                    },
                    Some(b'f') => {
                        p += 2;
                        if pat.get(p) != Some(&b'[') {
                            return Err(perr("missing '[' after '%f' in pattern"));
                        }
                        let set = self.set_at(p)?;
                        let prev = if s == 0 { 0 } else { src[s - 1] };
                        let cur = src.get(s).copied().unwrap_or(0);
                        if !set_has(&set.bits, prev) && set_has(&set.bits, cur) {
                            p = set.end;
                            continue;
                        }
                        return Ok(None);
                    }
                    Some(&d) if d.is_ascii_digit() => match self.match_back(s, d)? {
                        Some(ns) => {
                            s = ns;
                            p += 2;
                            continue;
                        }
                        None => return Ok(None),
                    },
                    _ => {}
                },
                _ => {}
            }
            let (ep, set) = self.class_info(p)?;
            let m = s < src.len() && self.single_match(src[s], p, set.as_ref());
            match pat.get(ep) {
                Some(b'?') => {
                    if m && let Some(r) = self.do_match(s + 1, ep + 1)? {
                        return Ok(Some(r));
                    }
                    p = ep + 1;
                }
                Some(b'*') => return self.max_expand(s, p, ep, set),
                Some(b'+') => {
                    return if m {
                        self.max_expand(s + 1, p, ep, set)
                    } else {
                        Ok(None)
                    };
                }
                Some(b'-') => return self.min_expand(s, p, ep, set),
                _ => {
                    if !m {
                        return Ok(None);
                    }
                    s += 1;
                    p = ep;
                }
            }
        }
    }

    fn max_expand(
        &mut self,
        s: usize,
        p: usize,
        ep: usize,
        set: Option<[u64; 4]>,
    ) -> mlua::Result<Option<usize>> {
        let mut i = 0usize;
        while s + i < self.src.len() && self.single_match(self.src[s + i], p, set.as_ref()) {
            i += 1;
        }
        self.tick(i as u64 / 16)?;
        loop {
            if let Some(r) = self.do_match(s + i, ep + 1)? {
                return Ok(Some(r));
            }
            if i == 0 {
                return Ok(None);
            }
            i -= 1;
        }
    }

    fn min_expand(
        &mut self,
        mut s: usize,
        p: usize,
        ep: usize,
        set: Option<[u64; 4]>,
    ) -> mlua::Result<Option<usize>> {
        loop {
            if let Some(r) = self.do_match(s, ep + 1)? {
                return Ok(Some(r));
            }
            if s < self.src.len() && self.single_match(self.src[s], p, set.as_ref()) {
                s += 1;
            } else {
                return Ok(None);
            }
        }
    }

    fn start_capture(&mut self, s: usize, p: usize, what: CapLen) -> mlua::Result<Option<usize>> {
        if self.level >= MAX_CAPTURES {
            return Err(perr("too many captures"));
        }
        self.caps[self.level] = Capture {
            start: s,
            len: what,
        };
        self.level += 1;
        let r = self.do_match(s, p)?;
        if r.is_none() {
            self.level -= 1;
        }
        Ok(r)
    }

    fn end_capture(&mut self, s: usize, p: usize) -> mlua::Result<Option<usize>> {
        let l = (0..self.level)
            .rev()
            .find(|&i| self.caps[i].len == CapLen::Unclosed)
            .ok_or_else(|| perr("invalid pattern capture"))?;
        self.caps[l].len = CapLen::Len(s - self.caps[l].start);
        let r = self.do_match(s, p)?;
        if r.is_none() {
            self.caps[l].len = CapLen::Unclosed;
        }
        Ok(r)
    }

    /// `%bxy` with `x` at `p`.
    fn match_balance(&mut self, s: usize, p: usize) -> mlua::Result<Option<usize>> {
        if p + 1 >= self.pat.len() {
            return Err(perr("unbalanced pattern"));
        }
        let (open, close) = (self.pat[p], self.pat[p + 1]);
        if self.src.get(s) != Some(&open) {
            return Ok(None);
        }
        let mut depth = 1u64;
        for (i, &c) in self.src.iter().enumerate().skip(s + 1) {
            if c == close {
                depth -= 1;
                if depth == 0 {
                    self.tick((i - s) as u64 / 16)?;
                    return Ok(Some(i + 1));
                }
            } else if c == open {
                depth += 1;
            }
        }
        self.tick((self.src.len() - s) as u64 / 16)?;
        Ok(None)
    }

    /// Back-reference `%1`..`%9`.
    fn match_back(&mut self, s: usize, d: u8) -> mlua::Result<Option<usize>> {
        let l = usize::from(d - b'0');
        if l == 0 || l > self.level {
            return Err(perr("invalid capture index"));
        }
        let cap = self.caps[l - 1];
        let len = match cap.len {
            CapLen::Unclosed => return Err(perr("invalid capture index")),
            // Lua 5.1 compares with the position capture's negative length taken as an
            // unsigned size, so a back-reference to a position capture never matches.
            CapLen::Position => return Ok(None),
            CapLen::Len(n) => n,
        };
        self.tick(len as u64 / 16)?;
        let src = self.src;
        if src.len() - s >= len && src[cap.start..cap.start + len] == src[s..s + len] {
            Ok(Some(s + len))
        } else {
            Ok(None)
        }
    }

    fn one_capture(&self, lua: &Lua, i: usize, s: usize, e: usize) -> mlua::Result<Value> {
        if i >= self.level {
            return if i == 0 {
                Ok(Value::String(lua.create_string(&self.src[s..e])?))
            } else {
                Err(perr("invalid capture index"))
            };
        }
        let cap = self.caps[i];
        match cap.len {
            CapLen::Unclosed => Err(perr("unfinished capture")),
            CapLen::Position => Ok(Value::Number((cap.start + 1) as f64)),
            CapLen::Len(n) => Ok(Value::String(
                lua.create_string(&self.src[cap.start..cap.start + n])?,
            )),
        }
    }

    /// The captures of the last match; with no explicit captures, the whole match when
    /// `whole` is given (and nothing otherwise, as `find` wants).
    fn captures(&self, lua: &Lua, whole: Option<(usize, usize)>) -> mlua::Result<MultiValue> {
        let n = if self.level == 0 && whole.is_some() {
            1
        } else {
            self.level
        };
        let (s, e) = whole.unwrap_or((0, 0));
        let mut out = MultiValue::new();
        for i in 0..n {
            out.push_back(self.one_capture(lua, i, s, e)?);
        }
        Ok(out)
    }
}

/// Plain substring search, charged to the budget.
fn find_plain(hay: &[u8], needle: &[u8], budget: &Budget) -> mlua::Result<Option<usize>> {
    if needle.is_empty() {
        return Ok(Some(0));
    }
    if needle.len() > hay.len() {
        return Ok(None);
    }
    let last = hay.len() - needle.len();
    let first = needle[0];
    let mut i = 0usize;
    let mut work = 0u64;
    while i <= last {
        let Some(off) = hay[i..=last].iter().position(|&b| b == first) else {
            break;
        };
        i += off;
        work += off as u64 / 64 + 1 + needle.len() as u64 / 64;
        if hay[i..i + needle.len()] == *needle {
            budget.charge(work)?;
            return Ok(Some(i));
        }
        if work >= 4096 {
            budget.charge(std::mem::take(&mut work))?;
        }
        i += 1;
    }
    budget.charge(work)?;
    Ok(None)
}

fn type_name(args: &MultiValue, i: usize) -> &'static str {
    args.get(i).map_or("no value", Value::type_name)
}

fn bad_arg(n: usize, fname: &str, expected: &str, got: &str) -> mlua::Error {
    host_error(format!(
        "bad argument #{n} to '{fname}' ({expected} expected, got {got})"
    ))
}

fn arg_string(lua: &Lua, args: &MultiValue, i: usize, fname: &str) -> mlua::Result<LuaString> {
    match args.get(i) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(v @ (Value::Integer(_) | Value::Number(_))) => lua
            .coerce_string(v.clone())?
            .ok_or_else(|| bad_arg(i + 1, fname, "string", v.type_name())),
        _ => Err(bad_arg(i + 1, fname, "string", type_name(args, i))),
    }
}

fn arg_opt_int(
    lua: &Lua,
    args: &MultiValue,
    i: usize,
    fname: &str,
    default: i64,
) -> mlua::Result<i64> {
    match args.get(i) {
        None | Some(Value::Nil) => Ok(default),
        Some(v) => match lua.coerce_number(v.clone())? {
            // Truncation like a C cast; NaN becomes 0, huge values saturate.
            Some(n) => Ok(n as i64),
            None => Err(bad_arg(i + 1, fname, "number", v.type_name())),
        },
    }
}

/// Relative string position to a 0-based start offset clamped to `0..=len` (Lua 5.1 rules
/// for `find`/`match` initial positions).
fn start_offset(init: i64, len: usize) -> usize {
    let len_i = i64::try_from(len).unwrap_or(i64::MAX);
    let pos = if init < 0 {
        init.saturating_add(len_i).saturating_add(1)
    } else {
        init
    };
    let off = pos.saturating_sub(1).max(0);
    usize::try_from(off).map_or(len, |o| o.min(len))
}

fn find_or_match(
    lua: &Lua,
    budget: &Budget,
    args: MultiValue,
    find: bool,
) -> mlua::Result<MultiValue> {
    let fname = if find { "find" } else { "match" };
    let s = arg_string(lua, &args, 0, fname)?;
    let p = arg_string(lua, &args, 1, fname)?;
    let sb = s.as_bytes();
    let pb = p.as_bytes();
    let init = start_offset(arg_opt_int(lua, &args, 2, fname, 1)?, sb.len());
    let pat = until_nul(&pb);
    let plain = args
        .get(3)
        .is_some_and(|v| !matches!(v, Value::Nil | Value::Boolean(false)));
    if find && (plain || !pat.iter().any(|b| SPECIALS.contains(b))) {
        return Ok(match find_plain(&sb[init..], &pb, budget)? {
            Some(i) => MultiValue::from_vec(vec![
                Value::Number((init + i + 1) as f64),
                Value::Number((init + i + pb.len()) as f64),
            ]),
            None => MultiValue::from_vec(vec![Value::Nil]),
        });
    }
    let (anchor, p0) = if pat.first() == Some(&b'^') {
        (true, 1)
    } else {
        (false, 0)
    };
    let mut m = Matcher::new(&sb, pat, budget);
    let mut s1 = init;
    loop {
        m.level = 0;
        if let Some(e) = m.do_match(s1, p0)? {
            m.flush()?;
            return if find {
                let mut out = m.captures(lua, None)?;
                out.push_front(Value::Number(e as f64));
                out.push_front(Value::Number((s1 + 1) as f64));
                Ok(out)
            } else {
                m.captures(lua, Some((s1, e)))
            };
        }
        if anchor || s1 >= sb.len() {
            break;
        }
        s1 += 1;
    }
    m.flush()?;
    Ok(MultiValue::from_vec(vec![Value::Nil]))
}

fn gmatch(lua: &Lua, budget: Arc<Budget>, args: MultiValue) -> mlua::Result<mlua::Function> {
    let s = arg_string(lua, &args, 0, "gmatch")?;
    let p = arg_string(lua, &args, 1, "gmatch")?;
    let pos = AtomicUsize::new(0);
    native_fn(lua, "gmatch iterator", move |lua, ()| {
        let sb = s.as_bytes();
        let pb = p.as_bytes();
        // `^` is not an anchor here (it would stop the iteration); it matches literally.
        let mut m = Matcher::new(&sb, until_nul(&pb), &budget);
        let mut src = pos.load(Ordering::Relaxed);
        while src <= sb.len() {
            m.level = 0;
            if let Some(e) = m.do_match(src, 0)? {
                pos.store(if e == src { e + 1 } else { e }, Ordering::Relaxed);
                m.flush()?;
                return m.captures(lua, Some((src, e)));
            }
            src += 1;
        }
        pos.store(sb.len() + 1, Ordering::Relaxed);
        m.flush()?;
        Ok(MultiValue::new())
    })
}

/// Append a replacement value (`gsub` with a function or table) or the original match.
fn push_value(lua: &Lua, out: &mut Vec<u8>, v: Value, original: &[u8]) -> mlua::Result<()> {
    match v {
        Value::Nil | Value::Boolean(false) => out.extend_from_slice(original),
        Value::String(s) => out.extend_from_slice(&s.as_bytes()),
        v @ (Value::Integer(_) | Value::Number(_)) => {
            if let Some(s) = lua.coerce_string(v)? {
                out.extend_from_slice(&s.as_bytes());
            }
        }
        other => {
            return Err(host_error(format!(
                "invalid replacement value (a {})",
                other.type_name()
            )));
        }
    }
    Ok(())
}

fn gsub(
    lua: &Lua,
    budget: &Budget,
    max_output: usize,
    args: MultiValue,
) -> mlua::Result<(LuaString, f64)> {
    let s = arg_string(lua, &args, 0, "gsub")?;
    let p = arg_string(lua, &args, 1, "gsub")?;
    let repl = args.get(2).cloned().unwrap_or(Value::Nil);
    let repl_text = match &repl {
        Value::String(_) | Value::Integer(_) | Value::Number(_) => {
            Some(arg_string(lua, &args, 2, "gsub")?)
        }
        Value::Function(_) | Value::Table(_) => None,
        _ => {
            return Err(host_error(
                "bad argument #3 to 'gsub' (string/function/table expected)",
            ));
        }
    };
    let sb = s.as_bytes();
    let pb = p.as_bytes();
    let len_i = i64::try_from(sb.len()).unwrap_or(i64::MAX);
    let max_n = arg_opt_int(lua, &args, 3, "gsub", len_i.saturating_add(1))?;
    let pat = until_nul(&pb);
    let (anchor, p0) = if pat.first() == Some(&b'^') {
        (true, 1)
    } else {
        (false, 0)
    };
    let mut m = Matcher::new(&sb, pat, budget);
    let mut out: Vec<u8> = Vec::new();
    let mut n: i64 = 0;
    let mut src = 0usize;
    while n < max_n {
        m.level = 0;
        let e = m.do_match(src, p0)?;
        if let Some(e) = e {
            n += 1;
            let before = out.len();
            match (&repl, &repl_text) {
                (_, Some(text)) => {
                    let r = text.as_bytes();
                    let mut i = 0;
                    while i < r.len() {
                        let b = r[i];
                        i += 1;
                        if b != b'%' {
                            out.push(b);
                            continue;
                        }
                        let d = r.get(i).copied().unwrap_or(0);
                        i += 1;
                        if !d.is_ascii_digit() {
                            out.push(d);
                        } else if d == b'0' {
                            out.extend_from_slice(&sb[src..e]);
                        } else {
                            let v = m.one_capture(lua, usize::from(d - b'1'), src, e)?;
                            push_value(lua, &mut out, v, &[])?;
                        }
                        if out.len() > max_output {
                            return Err(host_error("string.gsub: result too large"));
                        }
                    }
                }
                (Value::Function(f), None) => {
                    let caps = m.captures(lua, Some((src, e)))?;
                    let v: Value = f.call(caps)?;
                    push_value(lua, &mut out, v, &sb[src..e])?;
                }
                (Value::Table(t), None) => {
                    let key = m.one_capture(lua, 0, src, e)?;
                    let v: Value = t.get(key)?;
                    push_value(lua, &mut out, v, &sb[src..e])?;
                }
                _ => {}
            }
            m.tick((out.len() - before) as u64 / 64)?;
        }
        match e {
            Some(e) if e > src => src = e,
            _ if src < sb.len() => {
                out.push(sb[src]);
                src += 1;
            }
            _ => break,
        }
        if out.len() > max_output {
            return Err(host_error("string.gsub: result too large"));
        }
        if anchor {
            break;
        }
    }
    if out.len() + (sb.len() - src) > max_output {
        return Err(host_error("string.gsub: result too large"));
    }
    out.extend_from_slice(&sb[src..]);
    m.flush()?;
    Ok((lua.create_string(out)?, n as f64))
}

/// Replace the pattern functions of the `string` table (also reached through the string
/// metatable's `__index`) with the bounded implementations. `max_output` caps the result
/// of one `gsub` call (normally the state's memory cap).
pub(crate) fn install(
    lua: &Lua,
    string: &Table,
    budget: &Arc<Budget>,
    max_output: usize,
) -> mlua::Result<()> {
    let budget = Arc::clone(budget);
    let max_output = if max_output > 0 {
        max_output
    } else {
        DEFAULT_MAX_OUTPUT
    };

    let b = Arc::clone(&budget);
    string.raw_set(
        "find",
        native_fn(lua, "find", move |lua, args: MultiValue| {
            find_or_match(lua, &b, args, true)
        })?,
    )?;
    let b = Arc::clone(&budget);
    string.raw_set(
        "match",
        native_fn(lua, "match", move |lua, args: MultiValue| {
            find_or_match(lua, &b, args, false)
        })?,
    )?;
    let b = Arc::clone(&budget);
    let gm = native_fn(lua, "gmatch", move |lua, args: MultiValue| {
        gmatch(lua, Arc::clone(&b), args)
    })?;
    string.raw_set("gmatch", gm)?;
    // The original's build has no `gfind` alias: the name raises (plugin-runtime.md §4.1).
    string.raw_set(
        "gfind",
        native_fn(lua, "gfind", |_, _: MultiValue| -> mlua::Result<()> {
            Err(host_error("'string.gfind' was renamed to 'string.gmatch'"))
        })?,
    )?;
    let b = Arc::clone(&budget);
    string.raw_set(
        "gsub",
        native_fn(lua, "gsub", move |lua, args: MultiValue| {
            gsub(lua, &b, max_output, args)
        })?,
    )?;
    Ok(())
}
