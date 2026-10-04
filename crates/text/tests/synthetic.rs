// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests over synthetic records: table access, caching, StringInfo resolution,
//! named tables (`docs/specs/string-table.md` §9, §15).

mod common;

use common::{E, did_map, string_table};
use tessera_formats::{
    Did, StringArg, StringArgValue, StringArgs, StringInfo, StringSource, Utf16String, name_hash,
};
use tessera_text::{
    CacheLimits, Diagnostic, MemorySource, Message, MissReason, NamedTables, ROOT_DID_MAP,
    RenderMode, RenderOptions, Strings, Value, Values, render_plain,
};

const T: u32 = 0x2500_0010;
const T2: u32 = 0x2500_0011;

fn ids() -> (u32, u32) {
    (name_hash(b"ACTOR"), name_hash(b"COUNT"))
}

fn source() -> MemorySource {
    let (actor, count) = ids();
    let mut s = MemorySource::new();
    s.insert(
        T,
        string_table(
            T,
            &[
                E {
                    token: 1,
                    pieces: &["#1:{The [!n]}#1:", " lifts #1:{her[f]|his[m]|its} lantern."],
                    args: &[actor],
                    names: None,
                },
                E {
                    token: 2,
                    pieces: &["Copper Golem[e]"],
                    args: &[],
                    names: None,
                },
                E {
                    token: 3,
                    pieces: &[""],
                    args: &[],
                    names: None,
                },
                E {
                    token: 4,
                    pieces: &["Wren Hollow[fn]"],
                    args: &[],
                    names: None,
                },
                E {
                    token: 5,
                    pieces: &["#1:", " #1:{acorn[1]|acorns} for ", "."],
                    args: &[count, actor],
                    names: Some(&["COUNT", "ACTOR"]),
                },
            ],
        ),
    );
    s.insert(T2, b"\x11\x00\x00\x25 garbage".to_vec());
    s
}

fn table_ref(token: u32, table: u32) -> StringInfo {
    StringInfo {
        source: StringSource::Table {
            token,
            table: Did(table),
        },
        args: None,
    }
}

fn with_args(mut info: StringInfo, values: Vec<StringArg>) -> StringInfo {
    info.args = Some(Box::new(StringArgs {
        labels: Default::default(),
        values,
    }));
    info
}

#[test]
fn tables_and_entries() {
    let strings = Strings::new(source());
    assert_eq!(strings.table_dids().expect("dids"), vec![T, T2]);
    let t = strings.table(T).expect("read").expect("present");
    assert_eq!(t.len(), 5);
    assert!(strings.table(0x2500_9999).expect("read").is_none());
    assert!(strings.table(0x0100_0000).expect("read").is_none());
    assert!(strings.table(T2).is_err());
    // Second access is a cache hit.
    let before = strings.cache_stats();
    let _ = strings.table(T).expect("read");
    assert_eq!(strings.cache_stats().hits, before.hits + 1);
}

#[test]
fn render_entries() {
    let (actor, count) = ids();
    let strings = Strings::new(source());
    let plain = RenderOptions::default();
    let v = Values::new().with("ACTOR", "Copper Golem[e]");
    let r = strings.render_entry(T, 1, &v, &plain);
    assert!(r.is_clean());
    assert_eq!(r.plain_text(), "The Copper Golem lifts its lantern.");

    // Empty entry: empty text, no miss.
    let r = strings.render_entry(T, 3, &Values::new(), &plain);
    assert!(r.is_clean());
    assert_eq!(r.plain_text(), "");

    // Explorer mode uses the entry's own argument names.
    let r = strings.render_entry(T, 5, &Values::new(), &RenderOptions::explorer(None));
    assert_eq!(r.plain_text(), "${COUNT} acorns for ${ACTOR}.");
    let mut v = Values::new();
    v.insert(count, 1i64).insert(actor, "Wren Hollow[fn]");
    assert_eq!(
        strings.render_entry(T, 5, &v, &plain).plain_text(),
        "1 acorn for Wren Hollow."
    );
    let msg = strings.message(T, 5).expect("entry");
    assert_eq!(
        msg.format_string(None),
        "#1:${COUNT} #1:{acorn[1]|acorns} for ${ACTOR}."
    );
}

#[test]
fn misses() {
    let strings = Strings::new(source());
    let plain = RenderOptions::default();
    let explorer = RenderOptions::mode(RenderMode::Explorer);

    let r = strings.render_entry(T, 99, &Values::new(), &plain);
    assert_eq!(r.plain_text(), "");
    assert_eq!(r.miss().map(|m| &m.reason), Some(&MissReason::NoToken));
    let r = strings.render_entry(T, 99, &Values::new(), &explorer);
    assert_eq!(r.plain_text(), "[missing 0x25000010:0x00000063]");
    assert!(r.text.spans()[0].style.placeholder);

    let r = strings.resolve(&table_ref(1, 0x5800_0001), &Values::new(), &plain);
    assert_eq!(
        r.miss().map(|m| &m.reason),
        Some(&MissReason::NotStringTable)
    );
    let r = strings.resolve(&table_ref(1, 0x2500_7777), &Values::new(), &plain);
    assert_eq!(r.miss().map(|m| &m.reason), Some(&MissReason::NoTable));
    let r = strings.resolve(&table_ref(1, T2), &Values::new(), &plain);
    assert!(matches!(
        r.miss().map(|m| &m.reason),
        Some(MissReason::Unreadable(_))
    ));
}

#[test]
fn resolve_string_infos() {
    let (actor, count) = ids();
    let strings = Strings::new(source());
    let plain = RenderOptions::default();

    // Literal StringInfo: grammar and tag group apply.
    let lit = StringInfo {
        source: StringSource::Literal(Utf16String::from("Reed Boots[p]")),
        args: None,
    };
    assert_eq!(
        strings.resolve(&lit, &Values::new(), &plain).plain_text(),
        "Reed Boots"
    );

    // Reference with a nested StringInfo argument.
    let info = with_args(
        table_ref(1, T),
        vec![StringArg {
            var_id: actor,
            value: StringArgValue::Nested(Box::new(table_ref(4, T))),
        }],
    );
    let r = strings.resolve(&info, &Values::new(), &plain);
    assert!(r.is_clean(), "{:?}", r.diagnostics);
    assert_eq!(r.plain_text(), "Wren Hollow lifts her lantern.");

    // Integer argument and a nested miss (absent value, reported).
    let info = with_args(
        table_ref(5, T),
        vec![
            StringArg {
                var_id: count,
                value: StringArgValue::Integer(1),
            },
            StringArg {
                var_id: actor,
                value: StringArgValue::Nested(Box::new(table_ref(77, T))),
            },
        ],
    );
    let r = strings.resolve(&info, &Values::new().with("ACTOR", "ignored"), &plain);
    assert_eq!(r.plain_text(), "1 acorn for .");
    assert!(matches!(&r.diagnostics[..], [Diagnostic::Miss(m)] if m.token == 77));

    // Float argument.
    let info = with_args(
        table_ref(5, T),
        vec![StringArg {
            var_id: count,
            value: StringArgValue::Float(2.5),
        }],
    );
    assert_eq!(
        strings
            .resolve(&info, &Values::new().with("ACTOR", "Tobin[mn]"), &plain)
            .plain_text(),
        "2.5 acorns for Tobin."
    );

    // A StringInfo as a value of another message.
    let golem = strings.to_value(&table_ref(2, T)).expect("value");
    let r = strings.render_entry(T, 1, &[(actor, golem)].into_iter().collect(), &plain);
    assert_eq!(r.plain_text(), "The Copper Golem lifts its lantern.");
    assert!(strings.to_value(&table_ref(0, T)).is_err());
}

#[test]
fn string_info_bytes_round_trip() {
    // A StringInfo decoded from synthetic bytes resolves like a constructed one.
    let bytes = common::Bytes::new()
        .u8(0)
        .u32(4)
        .u32(T)
        .u8(0)
        .u8(1)
        .u8(0)
        .build();
    let info = StringInfo::parse(&bytes).expect("parse");
    let strings = Strings::new(source());
    assert_eq!(
        strings
            .resolve(&info, &Values::new(), &RenderOptions::default())
            .plain_text(),
        "Wren Hollow"
    );
}

#[test]
fn nested_string_info_depth_limit() {
    let (actor, _) = ids();
    let strings = Strings::new(source());
    // A chain of nested references, each filling ACTOR of entry 1.
    let mut info = table_ref(4, T);
    for _ in 0..20 {
        info = with_args(
            table_ref(1, T),
            vec![StringArg {
                var_id: actor,
                value: StringArgValue::Nested(Box::new(info)),
            }],
        );
    }
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert!(
        r.diagnostics
            .iter()
            .any(|d| matches!(d, Diagnostic::DepthLimit { .. }))
    );
}

#[test]
fn cache_eviction() {
    let mut s = MemorySource::new();
    for i in 0..50u32 {
        let did = 0x2500_1000 + i;
        s.insert(
            did,
            string_table(
                did,
                &[E {
                    token: 1,
                    pieces: &["x"],
                    args: &[],
                    names: None,
                }],
            ),
        );
    }
    let strings = Strings::new(s).with_cache_limits(CacheLimits {
        max_tables: 8,
        max_bytes: 1 << 20,
    });
    for i in 0..50u32 {
        let r = strings.render_entry(
            0x2500_1000 + i,
            1,
            &Values::new(),
            &RenderOptions::default(),
        );
        assert_eq!(r.plain_text(), "x");
    }
    assert!(strings.cache_stats().tables <= 8);
}

#[test]
fn named_tables() {
    let mut s = source();
    s.insert(
        ROOT_DID_MAP,
        did_map(
            ROOT_DID_MAP,
            &[(0, 0, "UNDEF"), (7, 0x2800_0004, "STRINGTABLE")],
        ),
    );
    s.insert(
        0x2800_0004,
        did_map(
            0x2800_0004,
            &[
                (0, 0, "NONE"),
                (3, T, "Things"),
                (4, 0x5800_0001, "NotAStringTable"),
            ],
        ),
    );
    let named = NamedTables::load(&s, ROOT_DID_MAP).expect("named tables");
    assert_eq!(named.tables().len(), 1);
    assert_eq!(named.by_label("Things"), Some(T));
    assert_eq!(named.by_key(3), Some(T));
    assert_eq!(named.by_label("NotAStringTable"), None);
    let strings = Strings::new(s).with_named_tables(named);
    let t = strings.named_tables().by_label("Things").expect("named");
    assert_eq!(
        strings
            .render_entry(t, 2, &Values::new(), &RenderOptions::default())
            .plain_text(),
        "Copper Golem"
    );
    assert!(NamedTables::load(&MemorySource::new(), ROOT_DID_MAP).is_err());
}

#[test]
fn strings_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Strings>();
    assert_send_sync::<Message>();
    assert_send_sync::<Value>();
}

#[test]
fn spec_validation_fixtures() {
    // A value tag group with spaces, escaped bracket at the end, non-letter group.
    let m = Message::from_format_string("${X}");
    let x = |s: &str| render_plain(&m, &Values::new().with("X", s));
    assert_eq!(x("Wren Hollow [fn]  "), "Wren Hollow");
    assert_eq!(x(r"see \[help\]"), "see [help]");
    assert_eq!(x("a [b c]"), "a [b c]");
    // Every escape, including \\n and a trailing backslash.
    let m = Message::literal(r"a\nb\qc\qd\te\[f\]g\\h\\ni\zj\");
    assert_eq!(
        render_plain(&m, &Values::new()),
        "a\nb\"c\"d\te[f]g\\h\\ni\\zj\\"
    );
    // A literal <placeholder> and a link element.
    let m = Message::literal(r"/tell <name> <Select:IID:>it<\\Select>");
    assert_eq!(render_plain(&m, &Values::new()), "/tell <name> it");
}

#[test]
fn to_value_keeps_nested_misses() {
    // Regression: a nested miss inside `to_value` is reported when the value renders.
    let (actor, _) = ids();
    let strings = Strings::new(source());
    let info = with_args(
        table_ref(1, T),
        vec![StringArg {
            var_id: actor,
            value: StringArgValue::Nested(Box::new(table_ref(7, 0x2500_0001))),
        }],
    );
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert!(matches!(
        r.miss().map(|m| &m.reason),
        Some(MissReason::NoTable)
    ));
    let value = strings.to_value(&info).expect("value");
    let top = Message::from_format_string("[${X}]");
    let r = tessera_text::render(
        &top,
        &Values::new().with("X", value),
        &RenderOptions::default(),
    );
    // Step 8 applies to the outermost message only, so the nested double space stays.
    assert_eq!(r.plain_text(), "[The  lifts its lantern.]");
    let miss = r.miss().expect("nested miss reported");
    assert_eq!((miss.table, miss.token), (0x2500_0001, 7));
    assert_eq!(miss.reason, MissReason::NoTable);
}

#[test]
fn resolve_builds_only_used_arguments_once() {
    // Regression: unused arguments are not built; repeated references parse once.
    let (actor, count) = ids();
    let strings = Strings::new(source());
    let args: Vec<StringArg> = (0..2000u32)
        .map(|i| StringArg {
            var_id: 0x0100_0000 + i,
            value: StringArgValue::Nested(Box::new(table_ref(4, T))),
        })
        .collect();
    let info = with_args(table_ref(2, T), args);
    let before = strings.cache_stats();
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert_eq!(r.plain_text(), "Copper Golem");
    let after = strings.cache_stats();
    // One table access for the top message, none for the 2000 unused arguments.
    assert_eq!(
        (after.hits + after.misses) - (before.hits + before.misses),
        1
    );

    // Two used arguments naming the same entry: one lookup for both (memoised).
    let info = with_args(
        table_ref(5, T),
        vec![
            StringArg {
                var_id: count,
                value: StringArgValue::Nested(Box::new(table_ref(4, T))),
            },
            StringArg {
                var_id: actor,
                value: StringArgValue::Nested(Box::new(table_ref(4, T))),
            },
        ],
    );
    let before = strings.cache_stats();
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert_eq!(r.plain_text(), "Wren Hollow acorns for Wren Hollow.");
    let after = strings.cache_stats();
    assert_eq!(
        (after.hits + after.misses) - (before.hits + before.misses),
        2
    );
}

#[test]
fn decode_failures_are_cached() {
    // Regression: a table that fails to decode is not re-read on every lookup.
    let strings = Strings::new(source());
    let e1 = strings.table(T2).expect_err("corrupt");
    let e2 = strings.table(T2).expect_err("corrupt");
    assert!(matches!(e1, tessera_text::TextError::Decode { .. }));
    assert!(matches!(
        e2,
        tessera_text::TextError::CachedFailure { did: T2, .. }
    ));
    let stats = strings.cache_stats();
    assert_eq!((stats.misses, stats.hits), (1, 1));
    // Many nested references to it cost one load.
    let (actor, _) = ids();
    let args: Vec<StringArg> = (0..500)
        .map(|_| StringArg {
            var_id: actor,
            value: StringArgValue::Nested(Box::new(table_ref(1, T2))),
        })
        .collect();
    let info = with_args(table_ref(1, T), args);
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert!(matches!(
        r.miss().map(|m| &m.reason),
        Some(MissReason::Unreadable(_))
    ));
    assert_eq!(strings.cache_stats().misses, 2); // T itself, nothing new for T2
    // Clearing the cache allows a retry.
    strings.clear_cache();
    assert!(matches!(
        strings.table(T2),
        Err(tessera_text::TextError::Decode { .. })
    ));
}

#[test]
fn miss_is_an_error_type() {
    fn lookup(s: &Strings) -> Result<Message, Box<dyn std::error::Error>> {
        Ok(s.message(T, 99)?)
    }
    let strings = Strings::new(source());
    let e = lookup(&strings).expect_err("miss");
    assert_eq!(
        e.to_string(),
        "string lookup miss: token 0x00000063 in table 0x25000010: no such token in the table"
    );
}

#[test]
fn formats_types_are_reexported() {
    // Consumers can name the record types through tessera-text alone.
    let info = tessera_text::StringInfo {
        source: tessera_text::StringSource::Literal(tessera_text::Utf16String::from("x")),
        args: None,
    };
    let strings = Strings::new(source());
    assert_eq!(
        strings
            .resolve(&info, &Values::new(), &RenderOptions::default())
            .plain_text(),
        "x"
    );
    let _: Option<std::sync::Arc<tessera_text::StringTable>> = strings.table(T).expect("t");
}
