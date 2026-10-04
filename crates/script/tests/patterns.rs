// SPDX-License-Identifier: GPL-3.0-or-later
//! Differential test: the bounded Rust pattern functions installed in apartments must give
//! the same results (values and error messages) as the stock Lua 5.1 string library on
//! well-behaved inputs. Spec: `docs/specs/lua-plugin-api.md` §3.1, §3.3.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_script::mlua::{Lua, LuaOptions, StdLib};
use tessera_script::{DataScope, HostApi, ManagerConfig, PluginManager};

#[derive(Debug)]
struct QuietHost;

impl HostApi for QuietHost {
    fn write_line(&self, _text: &str) {}
    fn plugin_data_dir(&self, _scope: DataScope) -> Option<PathBuf> {
        None
    }
}

/// Expressions evaluated in both states. Each is wrapped so that results and errors are
/// rendered as one comparable string.
const CASES: &[&str] = &[
    // find: plain and pattern, init handling
    r#"string.find("hello world", "o w")"#,
    r#"string.find("hello world", "o", 6)"#,
    r#"string.find("hello world", "o", -3)"#,
    r#"string.find("hello world", "o", -100)"#,
    r#"string.find("hello", "", 10)"#,
    r#"string.find("hello", "", 0)"#,
    r#"string.find("hello", "l+")"#,
    r#"string.find("a.b.c", ".", 1, true)"#,
    r#"string.find("a+b", "+", 1, true)"#,
    r#"string.find("a(b", "(", 1, true)"#,
    r#"string.find("abc", "b", nil, false)"#,
    r#"string.find(12345, 34)"#,
    r#"string.find("x = 10", "(%w+)%s*=%s*(%d+)")"#,
    r#"string.find("abc", "()b()")"#,
    r#"string.find("", "")"#,
    r#"string.find("", "a")"#,
    r#"string.find("aaa", "a-", 2)"#,
    // match: classes, sets, anchors, quantifiers
    r#"string.match("  key = value  ", "^%s*(%S+)%s*=%s*(%S+)%s*$")"#,
    r#"string.match("Hello World 42!", "%u%l+")"#,
    r#"string.match("Hello World 42!", "%d+")"#,
    r#"string.match("Hello World 42!", "%p")"#,
    r#"string.match("Hello World 42!", "%A+")"#,
    r#"string.match("tab\there", "%c")"#,
    r#"string.match("vt\vx", "%s")"#,
    r#"string.match("0xFFg", "%x+")"#,
    r#"string.match("a\0b", "%z")"#,
    r#"string.match("a\0b", "[%z]")"#,
    r#"string.match("a]b", "[]]")"#,
    r#"string.match("a-b", "[a-]+")"#,
    r#"string.match("a-b", "[-a]+")"#,
    r#"string.match("abc123", "[^%d]+")"#,
    r#"string.match("abc123", "[%a%d]+$")"#,
    r#"string.match("x^y", "x^y")"#,
    r#"string.match("x$y", "x$y")"#,
    r#"string.match("abc", "^b")"#,
    r#"string.match("abc", "c$")"#,
    r#"string.match("abc", "b$")"#,
    r#"string.match("aaab", "a-b")"#,
    r#"string.match("aaab", "a*")"#,
    r#"string.match("aaab", "a?a?a?a?b")"#,
    r#"string.match("color", "colou?r")"#,
    r#"string.match("2026-10-04", "(%d+)-(%d+)-(%d+)")"#,
    r#"string.match("hello", "(h)(e)(l)(l)(o)")"#,
    r#"string.match("hello", "((h)(e))")"#,
    r#"string.match("hello", "()ll()")"#,
    r#"string.match("abcabc", "(abc)%1")"#,
    r#"string.match("xyzxyx", "(x)(y).%2%1")"#,
    r#"string.match("f(a(b)c)d", "%b()")"#,
    r#"string.match("f(a(b)c", "%b()")"#,
    r#"string.match("THE (quick) fox", "%f[%a]%a+")"#,
    r#"string.match("THE (quick) fox", "%f[%l]%a+")"#,
    r#"string.match("end", "%f[%z]")"#,
    r#"string.match("hello", ".-l")"#,
    r#"string.match("hello", "")"#,
    r#"string.match("hello", "x*")"#,
    r#"string.match("hello", "h", 2)"#,
    r#"string.match("hello", "l", -2)"#,
    r#"string.match("a.b", "%.")"#,
    r#"string.match("a%b", "%%")"#,
    r#"string.match("caf\195\169", "[\128-\255]+")"#,
    r#"string.match("ab\0cd", "b\0c")"#,
    // gmatch
    r#"(function() local t = {} for w in string.gmatch("one two  three", "%a+") do t[#t+1] = w end return table.concat(t, ",") end)()"#,
    r#"(function() local t = {} for k, v in string.gmatch("a=1, b=2, c=3", "(%w+)=(%w+)") do t[#t+1] = k .. ":" .. v end return table.concat(t, ",") end)()"#,
    r#"(function() local t = {} for w in string.gmatch("abc", "%a*") do t[#t+1] = "[" .. w .. "]" end return table.concat(t) end)()"#,
    r#"(function() local t = {} for w in string.gmatch("abc", "") do t[#t+1] = "[" .. w .. "]" end return table.concat(t) end)()"#,
    r#"(function() local t = {} for p in string.gmatch("abc", "()") do t[#t+1] = p end return table.concat(t, ",") end)()"#,
    r#"(function() local t = {} for w in string.gmatch("^a^b", "^%a") do t[#t+1] = w end return table.concat(t, ",") end)()"#,
    r#"(function() local t = {} for w in string.gfind("x y", "%a") do t[#t+1] = w end return table.concat(t, ",") end)()"#,
    // gsub
    r#"string.gsub("hello world", "o", "0")"#,
    r#"string.gsub("hello world", "o", "0", 1)"#,
    r#"string.gsub("hello world", "o", "0", 0)"#,
    r#"string.gsub("hello world", "o", "0", -1)"#,
    r#"string.gsub("hello world", "(%w+)", "<%1>")"#,
    r#"string.gsub("hello world", "%w+", "%0 %0")"#,
    r#"string.gsub("hello", "", "-")"#,
    r#"string.gsub("abc", "%w", "%%")"#,
    r#"string.gsub("abc", "%w", "%x")"#,
    r#"string.gsub("abc", "b", "%")"#,
    r#"string.gsub("abc", "^a", "X")"#,
    r#"string.gsub("aaa", "^a", "X")"#,
    r#"string.gsub("hello world", "(o)", { o = "0" })"#,
    r#"string.gsub("hello world", "%w+", { hello = "bye", world = false })"#,
    r#"string.gsub("hello world", "%w+", function(w) return #w end)"#,
    r#"string.gsub("hello world", "%w+", function(w) if w == "world" then return nil end return w:upper() end)"#,
    r#"string.gsub("$name is $age", "%$(%w+)", { name = "Ann", age = 7 })"#,
    r#"string.gsub("abc", "()", "%1")"#,
    r#"string.gsub("abc", "b", 5)"#,
    r#"string.gsub(123, 2, 9)"#,
    r#"string.gsub("a b c", " ", "", 1.9)"#,
    r#"("a,b,,c"):gsub(",", ";")"#,
    r#"("x"):rep(3):gsub("x", "%0%0")"#,
    r#"string.find("abc", "c", 10)"#,
    r#"string.find("abc", "c?", 10)"#,
    r#"string.match("word", "%f[%w]%w+")"#,
    r#"string.match("a-z", "[%a-z]+")"#,
    r#"string.match("%-a", "[a-%%]+")"#,
    r#"string.match('say "hi" now', '%b""')"#,
    r#"string.match("[[x]]", "%b[]")"#,
    r#"string.gsub("abc", "^", ">")"#,
    r#"string.gsub("abc", "$", "<")"#,
    r#"string.gsub("abc", ".-", "-")"#,
    r#"string.match("aXbXc", "(.-)X(.-)X(.*)")"#,
    r#"string.match("key: value", "^(%w+):%s*(.-)%s*$")"#,
    r#"string.match("  trim  ", "^%s*(.-)%s*$")"#,
    r#"string.find("a\nb", "\n", 1, true)"#,
    r#"string.match("x", "[^]")"#,
    r#"string.match("]", "[^]]")"#,
    r#"string.match("a", "[a")"#,
    r#"string.match("abc", "[%")"#,
    // errors (messages must match too)
    r#"string.find("abc", "%")"#,
    r#"string.find("abc", "[a")"#,
    r#"string.find("abc", "[")"#,
    r#"string.find("abc", "(a")"#,
    r#"string.match("abc", "(a")"#,
    r#"string.find("abc", "a)")"#,
    r#"string.find("abc", "%1")"#,
    r#"string.find("abc", "(a)%2")"#,
    r#"string.find("abc", "%b")"#,
    r#"string.find("abc", "%f")"#,
    r#"string.find("abc", "%fa")"#,
    r#"string.match("abc", "%0")"#,
    r#"string.gsub("abc", "a", "%2")"#,
    r#"string.gsub("abc", "(a)", "%2")"#,
    r#"string.gsub("abc", "a", {a = {}})"#,
    r#"string.gsub("abc", "a", function() return true end)"#,
    r#"string.gsub("abc", "a", true)"#,
    r#"string.gsub("abc", "a")"#,
    r#"string.find(nil, "a")"#,
    r#"string.find("a")"#,
    r#"string.find("a", "a", "x")"#,
    r#"string.match({}, "a")"#,
    r#"string.gmatch("a")"#,
    r#"string.gsub("abc", "a", "x", "y")"#,
];

const WRAPPER: &str = r##"
local function render(...)
    local out = {}
    for i = 1, select("#", ...) do
        local v = select(i, ...)
        out[#out + 1] = type(v) .. ":" .. tostring(v)
    end
    return table.concat(out, "|")
end
return render(pcall(function() local x = { CASE } return unpack(x, 1, table.maxn(x)) end))
"##;

fn eval(lua: &Lua, case: &str) -> String {
    // Results are captured in a table first so the case is never a tail call (error
    // positions then refer to the case line in both states).
    let src = WRAPPER.replace("CASE", case);
    lua.load(&src)
        .set_name("=case")
        .eval::<String>()
        .unwrap_or_else(|e| format!("host error: {e}"))
}

#[test]
fn patterns_match_reference_lua_5_1() {
    let reference = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH,
        LuaOptions::default(),
    )
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("Plugins");
    std::fs::create_dir_all(plugins.join("P")).unwrap();
    std::fs::write(
        plugins.join("P/P.plugin"),
        "<Plugin><Information><Name>P</Name><Author>T</Author><Version>1</Version></Information><Package>P.Main</Package></Plugin>",
    )
    .unwrap();
    std::fs::write(plugins.join("P/Main.lua"), "-- empty\n").unwrap();
    let host: Arc<dyn HostApi> = Arc::new(QuietHost);
    let mut m = PluginManager::new(ManagerConfig::new(&plugins), host);
    m.refresh();
    m.load_plugin("P").unwrap();

    let mut mismatches = Vec::new();
    for case in CASES {
        let want = eval(&reference, case);
        let got = m
            .with_lua("Default", |lua| eval(lua, case))
            .expect("apartment exists");
        if want != got {
            mismatches.push(format!("{case}\n   reference: {want}\n   tessera:   {got}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} cases differ:\n{}",
        mismatches.len(),
        CASES.len(),
        mismatches.join("\n")
    );
}

const RANDOM_CHUNK: &str = r##"
local s, p, r = ...
local function render(...)
    local out = {}
    for i = 1, select("#", ...) do
        local v = select(i, ...)
        out[#out + 1] = type(v) .. ":" .. tostring(v)
    end
    return table.concat(out, "|")
end
local function gm()
    local t, n = {}, 0
    for a, b in string.gmatch(s, p) do
        n = n + 1
        if n > 100 then break end
        t[#t + 1] = tostring(a) .. "," .. tostring(b)
    end
    return table.concat(t, ";")
end
return render(pcall(function() local x = { string.find(s, p) } return unpack(x, 1, table.maxn(x)) end))
    .. " # " .. render(pcall(function() local x = { string.match(s, p, 2) } return unpack(x, 1, table.maxn(x)) end))
    .. " # " .. render(pcall(function() local x = { string.gsub(s, p, r) } return unpack(x, 1, table.maxn(x)) end))
    .. " # " .. render(pcall(function() local x = { gm() } return unpack(x, 1, table.maxn(x)) end))
"##;

#[test]
fn random_patterns_match_reference_lua_5_1() {
    // Deterministic noise over pattern syntax (short subjects, so the reference matcher
    // cannot recurse deeply or backtrack for long): both implementations must agree on
    // every result and error message, and nothing may panic.
    let reference = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH,
        LuaOptions::default(),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("Plugins");
    std::fs::create_dir_all(plugins.join("P")).unwrap();
    std::fs::write(
        plugins.join("P/P.plugin"),
        "<Plugin><Information><Name>P</Name><Author>T</Author><Version>1</Version></Information><Package>P.Main</Package></Plugin>",
    )
    .unwrap();
    std::fs::write(plugins.join("P/Main.lua"), "-- empty\n").unwrap();
    let host: Arc<dyn HostApi> = Arc::new(QuietHost);
    let mut m = PluginManager::new(ManagerConfig::new(&plugins), host);
    m.refresh();
    m.load_plugin("P").unwrap();

    let alphabet: &[u8] = b"a b%()[]^$*+-?.0129dfbzsxAW\0]";
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let run = |lua: &Lua, s: &[u8], p: &[u8], r: &[u8]| -> String {
        (|| {
            lua.load(RANDOM_CHUNK).set_name("=random").call::<String>((
                lua.create_string(s)?,
                lua.create_string(p)?,
                lua.create_string(r)?,
            ))
        })()
        .unwrap_or_else(|e| format!("host error: {e}"))
    };
    let mut mismatches = Vec::new();
    for _ in 0..3000 {
        let plen = (next() % 12) as usize;
        let pat: Vec<u8> = (0..plen)
            .map(|_| alphabet[(next() as usize) % alphabet.len()])
            .collect();
        let slen = (next() % 16) as usize;
        let subject: Vec<u8> = (0..slen)
            .map(|_| b"ab()x "[(next() % 6) as usize])
            .collect();
        let repl: Vec<u8> = (0..(next() % 5) as usize)
            .map(|_| b"%0123x"[(next() % 6) as usize])
            .collect();
        // Back-references to position captures read undefined memory in the reference
        // implementation; leave them out of the comparison.
        if pat
            .windows(2)
            .any(|w| w[0] == b'%' && w[1].is_ascii_digit())
            && pat.windows(2).any(|w| w == b"()")
        {
            continue;
        }
        let want = run(&reference, &subject, &pat, &repl);
        let got = m
            .with_lua("Default", |lua| run(lua, &subject, &pat, &repl))
            .unwrap();
        if want != got && mismatches.len() < 20 {
            mismatches.push(format!(
                "s={:?} p={:?} r={:?}\n   reference: {want}\n   tessera:   {got}",
                String::from_utf8_lossy(&subject),
                String::from_utf8_lossy(&pat),
                String::from_utf8_lossy(&repl)
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
