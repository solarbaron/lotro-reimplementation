// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic record builders for unit tests (our own encodings of the layouts in
//! `property-system.md` and `records.md`; no game data).

use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};
use tessera_formats::{StringInfo, StringSource};

use crate::set::PropertySet;
use crate::value::{Property, Value};

/// Smallest hint whose bucket count is at least half of `n` (§5.4 PS-H2), and its count.
fn hint_for(n: usize) -> (u8, u32) {
    for (h, &b) in TSIZE_BUCKET_COUNTS.iter().enumerate() {
        if (b as usize) * 2 >= n {
            return (h as u8, b);
        }
    }
    (12, TSIZE_BUCKET_COUNTS[12])
}

/// Sorts keyed items into bucket order for `n` items and returns the hint.
fn bucket_sort<T>(items: &mut [T], key: impl Fn(&T) -> u32) -> u8 {
    let (hint, b) = hint_for(items.len());
    items.sort_by_key(|i| key(i) % b);
    hint
}

fn tsize(out: &mut Vec<u8>, hint: u8, n: usize) {
    out.push(hint);
    write_vle(out, n as u32);
}

fn pascal(out: &mut Vec<u8>, s: &str) {
    let bytes: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
    write_vle(out, bytes.len() as u32);
    out.extend_from_slice(&bytes);
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// A literal StringInfo.
pub(crate) fn literal(text: &str) -> StringInfo {
    StringInfo {
        source: StringSource::Literal(tessera_formats::Utf16String(text.encode_utf16().collect())),
        args: None,
    }
}

fn string_info(out: &mut Vec<u8>, s: &StringInfo) {
    match &s.source {
        StringSource::Literal(units) => {
            out.push(1);
            write_vle(out, units.0.len() as u32);
            for u in &units.0 {
                out.extend_from_slice(&u.to_le_bytes());
            }
        }
        StringSource::Table { token, table } => {
            out.push(0);
            u32le(out, *token);
            u32le(out, table.0);
        }
    }
    assert!(
        s.args.is_none(),
        "testkit encodes StringInfo without arguments only"
    );
    out.extend_from_slice(&[0, 1, 0]);
}

/// One synthetic property definition.
#[derive(Debug, Clone)]
pub(crate) struct Def {
    pub pid: u32,
    pub name: &'static str,
    pub ty: u32,
    pub data: u32,
    pub children: Vec<u32>,
}

impl Def {
    pub fn new(pid: u32, name: &'static str, ty: u32) -> Self {
        Self {
            pid,
            name,
            ty,
            data: 0,
            children: Vec::new(),
        }
    }
    pub fn data(mut self, data: u32) -> Self {
        self.data = data;
        self
    }
    pub fn children(mut self, c: &[u32]) -> Self {
        self.children = c.to_vec();
        self
    }
}

/// A master property dictionary record (§2.1) with no default/min/max values.
pub(crate) fn dictionary(defs: &[Def]) -> Vec<u8> {
    dictionary_with_names(defs, &[])
}

/// Like [`dictionary`], with extra names that have no definition (such as PID 0, §2.5).
pub(crate) fn dictionary_with_names(defs: &[Def], extra: &[(u32, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, 0x3400_0000);
    out.extend_from_slice(&[0; 8]);
    let mut names: Vec<(u32, &str)> = defs.iter().map(|d| (d.pid, d.name)).collect();
    names.extend_from_slice(extra);
    let hint = bucket_sort(&mut names, |n| n.0);
    tsize(&mut out, hint, names.len());
    for (pid, name) in &names {
        u32le(&mut out, *pid);
        pascal(&mut out, name);
    }
    out.extend_from_slice(&[0, 0]);
    let mut sorted: Vec<&Def> = defs.iter().collect();
    let hint = bucket_sort(&mut sorted, |d| d.pid);
    tsize(&mut out, hint, sorted.len());
    for d in sorted {
        u32le(&mut out, d.pid);
        u32le(&mut out, d.pid);
        u32le(&mut out, d.ty);
        u32le(&mut out, 0); // field_a
        u32le(&mut out, 4); // field_b
        u32le(&mut out, d.data);
        u32le(&mut out, 0); // field_c
        u32le(&mut out, 0); // flags
        u32le(&mut out, 0x3FC0_0000);
        out.extend_from_slice(&[2, 2, 4, 0, 1]);
        write_vle(&mut out, d.children.len() as u32);
        for c in &d.children {
            u32le(&mut out, *c);
            u32le(&mut out, *c);
        }
        u32le(&mut out, 0); // subset count
        u32le(&mut out, 0); // terminator
    }
    out
}

/// An enum mapper record (`records.md` §7.1) with literal display texts.
pub(crate) fn enum_mapper(
    did: u32,
    base: u32,
    names: &[(u32, &str)],
    displays: &[(u32, &str)],
) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, did);
    u32le(&mut out, base);
    let mut names = names.to_vec();
    let hint = bucket_sort(&mut names, |n| n.0);
    tsize(&mut out, hint, names.len());
    for (k, n) in &names {
        u32le(&mut out, *k);
        pascal(&mut out, n);
    }
    let mut displays = displays.to_vec();
    let hint = bucket_sort(&mut displays, |n| n.0);
    tsize(&mut out, hint, displays.len());
    for (k, d) in &displays {
        u32le(&mut out, *k);
        string_info(&mut out, &literal(d));
    }
    out
}

/// A DID map record (`records.md` §8.1): `main` in group 0, `second` in group 1.
pub(crate) fn did_map(did: u32, main: &[(u32, u32, &str)], second: &[(u32, u32, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, did);
    for group in [main, second] {
        let mut g = group.to_vec();
        let hint = bucket_sort(&mut g, |e| e.0);
        tsize(&mut out, hint, g.len());
        for (k, v, _) in &g {
            u32le(&mut out, *k);
            u32le(&mut out, *v);
        }
        tsize(&mut out, hint, g.len());
        for (k, _, l) in &g {
            u32le(&mut out, *k);
            out.push(l.len() as u8);
            out.extend_from_slice(l.as_bytes());
        }
    }
    out
}

fn entries(out: &mut Vec<u8>, props: &[Property]) {
    let mut sorted: Vec<&Property> = props.iter().collect();
    let hint = bucket_sort(&mut sorted, |p| p.pid);
    tsize(out, hint, sorted.len());
    for p in sorted {
        u32le(out, p.pid);
        u32le(out, p.pid);
        value(out, &p.value);
    }
}

/// Encodes one value in its stored form (§4).
pub(crate) fn value(out: &mut Vec<u8>, v: &Value) {
    match v {
        Value::String(s) => pascal(out, s),
        Value::StringToken(t) => u32le(out, *t),
        Value::Waveform(w) => {
            u32le(out, w.subtype);
            for f in &w.values {
                out.extend_from_slice(&f.to_le_bytes());
            }
            if let Some(x) = &w.extra {
                out.extend_from_slice(&x.scalar.to_le_bytes());
                out.push(u8::from(x.flag));
                u32le(out, x.pairs.len() as u32);
                for (a, b) in &x.pairs {
                    out.extend_from_slice(&a.to_le_bytes());
                    out.extend_from_slice(&b.to_le_bytes());
                }
            }
        }
        Value::Timestamp(t) => out.extend_from_slice(&t.to_le_bytes()),
        Value::TriState(t) => out.push(*t),
        Value::Vector3(v) => v
            .iter()
            .for_each(|f| out.extend_from_slice(&f.to_le_bytes())),
        Value::InstanceId(i) => out.extend_from_slice(&i.to_le_bytes()),
        Value::Enum(k) => u32le(out, *k),
        Value::Float(f) => out.extend_from_slice(&f.to_le_bytes()),
        Value::PropertyId(p) => u32le(out, *p),
        Value::Struct(s) => entries(out, s.as_slice()),
        Value::Array(a) => {
            u32le(out, a.len() as u32);
            for e in a {
                u32le(out, e.pid);
                value(out, &e.value);
            }
        }
        Value::StringInfo(s) => string_info(out, s),
        Value::Bitfield64(b) => out.extend_from_slice(&b.to_le_bytes()),
        Value::Int(i) => out.extend_from_slice(&i.to_le_bytes()),
        Value::Colour(c) => out.extend_from_slice(c),
        Value::Position(p) => {
            out.push(p.flags());
            if let Some(r) = p.region() {
                out.push(r);
            }
            if let Some((x, y)) = p.landblock() {
                out.extend_from_slice(&[x, y]);
            }
            if let Some(i) = p.instance() {
                out.extend_from_slice(&i.to_le_bytes());
            }
            if let Some(c) = p.cell() {
                out.extend_from_slice(&c.to_le_bytes());
            }
            for f in p.offset().into_iter().flatten() {
                out.extend_from_slice(&f.to_le_bytes());
            }
            for f in p.orientation().into_iter().flatten() {
                out.extend_from_slice(&f.to_le_bytes());
            }
        }
        Value::Bitfield32(b) => u32le(out, *b),
        Value::Int64(i) => out.extend_from_slice(&i.to_le_bytes()),
        Value::DidRef(d) => u32le(out, d.0),
        Value::Bool(b) => out.push(u8::from(*b)),
        Value::BitfieldAny(b) => {
            write_vle(out, b.bits);
            out.extend_from_slice(&b.bytes);
        }
    }
}

/// A property-set record (§5.1) holding `set`.
pub(crate) fn property_set_record(did: u32, set: &PropertySet) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, did);
    entries(&mut out, set.as_slice());
    out
}
