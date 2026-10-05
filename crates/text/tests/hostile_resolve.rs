// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for hostile StringInfo resolution over synthetic records: work bounds
//! on nested messages, table-cache behaviour with oversized tables and the (0, 0)
//! reference (`docs/specs/string-table.md` §9, `docs/specs/property-system.md` §4.8).

mod common;

use std::time::{Duration, Instant};

use common::{E, string_table};
use tessera_formats::{Did, StringArg, StringArgValue, StringArgs, StringInfo, StringSource};
use tessera_text::{
    CacheLimits, Diagnostic, MemorySource, RenderMode, RenderOptions, Strings, Values,
};

const T: u32 = 0x2500_0020;
const BIG: u32 = 0x2500_0021;
const SMALL: u32 = 0x2500_0022;

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

/// A source with entry 1 of table `T`: `slots` slots with the distinct IDs 1..=slots, and
/// entry 2: plain text.
fn wide_source(slots: u32) -> MemorySource {
    let pieces: Vec<&str> = vec![""; slots as usize + 1];
    let args: Vec<u32> = (1..=slots).collect();
    let mut s = MemorySource::new();
    s.insert(
        T,
        string_table(
            T,
            &[
                E {
                    token: 1,
                    pieces: &pieces,
                    args: &args,
                    names: None,
                },
                E {
                    token: 2,
                    pieces: &["leaf"],
                    args: &[],
                    names: None,
                },
            ],
        ),
    );
    s
}

fn has_work_limit(diags: &[Diagnostic]) -> bool {
    diags
        .iter()
        .any(|d| matches!(d, Diagnostic::WorkLimit { .. }))
}

/// Finding 0, N×S shape: N nested references to one S-slot entry. Before the fix this did
/// N×S work with no output and no diagnostic.
#[test]
fn nested_values_sharing_a_wide_entry_are_bounded() {
    let s = 3000u32;
    let strings = Strings::new(wide_source(s));
    let args: Vec<StringArg> = (1..=s)
        .map(|id| StringArg {
            var_id: id,
            value: StringArgValue::Nested(Box::new(table_ref(1, T))),
        })
        .collect();
    let info = with_args(table_ref(1, T), args);
    for mode in [RenderMode::Plain, RenderMode::Explorer] {
        let start = Instant::now();
        let r = strings.resolve(&info, &Values::new(), &RenderOptions::mode(mode));
        let took = start.elapsed();
        assert!(has_work_limit(&r.diagnostics), "{:?}", r.diagnostics);
        assert!(took < Duration::from_secs(20), "{mode:?} took {took:?}");
    }
}

/// Finding 0, depth×S shape: a chain of nested references, each filling the last slot of a
/// wide entry. Every level used to keep a full slot table alive.
#[test]
fn nested_chain_through_a_wide_entry_is_bounded() {
    let s = 1u32 << 17;
    let strings = Strings::new(wide_source(s));
    let mut info = table_ref(2, T);
    for _ in 0..15 {
        info = with_args(
            table_ref(1, T),
            vec![StringArg {
                var_id: s,
                value: StringArgValue::Nested(Box::new(info)),
            }],
        );
    }
    let start = Instant::now();
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    let took = start.elapsed();
    assert!(has_work_limit(&r.diagnostics), "{:?}", r.diagnostics);
    assert!(took < Duration::from_secs(20), "took {took:?}");
}

/// The work bound does not touch ordinary nesting.
#[test]
fn ordinary_nesting_is_not_limited() {
    let strings = Strings::new(wide_source(4));
    let args: Vec<StringArg> = (1..=4)
        .map(|id| StringArg {
            var_id: id,
            value: StringArgValue::Nested(Box::new(table_ref(2, T))),
        })
        .collect();
    let info = with_args(table_ref(1, T), args);
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert!(r.is_clean(), "{:?}", r.diagnostics);
    assert_eq!(r.plain_text(), "leafleafleafleaf");
}

fn big_and_small() -> MemorySource {
    // BIG: 49 entries of 8 KiB text each (about 784 KiB as UTF-16).
    let text = "x".repeat(8 << 10);
    let tokens: Vec<u32> = (1..=49).collect();
    let pieces = [text.as_str()];
    let entries: Vec<E<'_>> = tokens
        .iter()
        .map(|&token| E {
            token,
            pieces: &pieces,
            args: &[],
            names: None,
        })
        .collect();
    let mut s = MemorySource::new();
    s.insert(BIG, string_table(BIG, &entries));
    // SMALL entry 1: 49 slots with the IDs 1..=49.
    let small_pieces = vec![""; 50];
    s.insert(
        SMALL,
        string_table(
            SMALL,
            &[E {
                token: 1,
                pieces: &small_pieces,
                args: &tokens,
                names: None,
            }],
        ),
    );
    s
}

/// Finding 1: a table larger than the cache's byte limit used to evict every other table
/// and then itself, and a StringInfo naming K of its tokens reloaded it K times.
#[test]
fn oversized_table_is_loaded_once_per_call_and_keeps_others() {
    let strings = Strings::new(big_and_small()).with_cache_limits(CacheLimits {
        max_tables: 64,
        max_bytes: 1 << 19,
    });
    let info = with_args(
        table_ref(1, SMALL),
        (1..=49)
            .map(|token| StringArg {
                var_id: token,
                value: StringArgValue::Nested(Box::new(table_ref(token, BIG))),
            })
            .collect(),
    );
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::default());
    assert!(r.is_clean(), "{:?}", r.diagnostics);
    assert_eq!(r.plain_text().len(), 49 * (8 << 10));
    let stats = strings.cache_stats();
    // SMALL and BIG were each read from the source once.
    assert_eq!(stats.misses, 2, "{stats:?}");
    // The small table survived the oversized one.
    let hits = stats.hits;
    assert!(strings.table(SMALL).expect("read").is_some());
    let stats = strings.cache_stats();
    assert_eq!((stats.hits, stats.misses), (hits + 1, 2), "{stats:?}");
}

/// Finding 2: (table 0, token 0) means "no text" (property-system.md §4.8).
#[test]
fn null_reference_is_no_text() {
    let strings = Strings::new(wide_source(1));
    let null = table_ref(0, 0);
    for mode in [RenderMode::Plain, RenderMode::Explorer] {
        let r = strings.resolve(&null, &Values::new(), &RenderOptions::mode(mode));
        assert_eq!(r.plain_text(), "", "{mode:?}");
        assert!(r.is_clean(), "{mode:?}: {:?}", r.diagnostics);
    }
    // As a value and as a nested argument it is empty text too.
    assert!(strings.to_value(&null).is_ok());
    let info = with_args(
        table_ref(1, T),
        vec![StringArg {
            var_id: 1,
            value: StringArgValue::Nested(Box::new(null)),
        }],
    );
    let r = strings.resolve(&info, &Values::new(), &RenderOptions::explorer(None));
    assert!(r.is_clean(), "{:?}", r.diagnostics);
    assert_eq!(r.plain_text(), "");
    // Only the exact pair is special: token 0 in a real table, or a token in table 0,
    // still miss.
    assert!(strings.to_value(&table_ref(0, T)).is_err());
    assert!(strings.to_value(&table_ref(1, 0)).is_err());
}
