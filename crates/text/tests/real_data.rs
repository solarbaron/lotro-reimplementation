// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks against a user's own install (read-only).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install directory. Archives are opened
//! read-only through `tessera-dat`; nothing is ever written. The tests assert the
//! invariants of `docs/specs/string-table.md` §12 and the M1 exit criterion "100% of
//! string-table records render without error in the installed locale"; counts are printed,
//! not asserted, because they change with every patch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tessera_dat::Archive;
use tessera_formats::{EnumMapper, Record};
use tessera_text::{
    Diagnostic, GrammarIssue, LanguageId, Message, MissReason, NamedTables, ROOT_DID_MAP,
    RenderMode, RenderOptions, Strings, Values, render,
};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn open_strings() -> Option<Strings> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    Some(Strings::open_install(&dir, None).expect("open language archive"))
}

fn issue_kind(i: &GrammarIssue) -> &'static str {
    match i {
        GrammarIssue::UndefinedLabel(_) => "SG1 undefined label",
        GrammarIssue::StrayLabel => "SG2 stray label",
        GrammarIssue::LabelConflict(_) => "SG4 label conflict",
        GrammarIssue::UnterminatedSelector => "SG5 unterminated selector",
        GrammarIssue::SelectorWithoutSlots => "SG6 selector without slots",
        GrammarIssue::DollarBrace => "SG7 ${",
        GrammarIssue::ControlCharacter => "SG8 control character",
        GrammarIssue::Surrogate => "SG8 surrogate",
        GrammarIssue::KeepSpacesNotAtEnd => "SG9 keepspaces not at end",
        GrammarIssue::UnknownDirective(_) => "unknown directive",
        _ => "other",
    }
}

#[test]
fn render_every_entry() {
    let Some(strings) = open_strings() else {
        return;
    };
    let locale = strings.locale().expect("locale").clone();
    eprintln!(
        "locale {} language id {} ({})",
        locale.name,
        locale.language_id,
        locale.path.display()
    );
    if locale.name == "English" {
        assert_eq!(locale.language_id, LanguageId::ENGLISH);
    }

    let dids = strings.table_dids().expect("list tables");
    assert!(!dids.is_empty());
    let plain = RenderOptions::default();
    let explorer = RenderOptions::mode(RenderMode::Explorer);
    let empty = Values::new();

    let mut tables = 0usize;
    let mut entries = 0usize;
    let mut decode_errors = Vec::new();
    let mut not_clean = 0usize;
    let mut with_slots = 0usize;
    let mut own_tags = 0usize;
    let mut empty_out = 0usize;
    let mut keep_spaces = 0usize;
    let mut colored = 0usize;
    let mut links = 0usize;
    let mut leftover_selectors = 0usize;
    let mut labelled_carets = 0usize;
    let mut bare_carets = 0usize;
    let mut issues: BTreeMap<&'static str, (usize, Vec<String>)> = BTreeMap::new();

    for &did in &dids {
        let table = match strings.table(did) {
            Ok(Some(t)) => t,
            Ok(None) => {
                decode_errors.push(format!("{did:#010x}: listed but not found"));
                continue;
            }
            Err(e) => {
                decode_errors.push(format!("{did:#010x}: {e}"));
                continue;
            }
        };
        tables += 1;
        for entry in table.iter() {
            entries += 1;
            let msg = Message::from_entry(entry);
            for i in msg.issues() {
                let slot = issues.entry(issue_kind(i)).or_default();
                slot.0 += 1;
                if slot.1.len() < 5 {
                    slot.1.push(format!("{did:#010x}/{:#010x}", entry.token));
                }
            }
            if !msg.slots().is_empty() {
                with_slots += 1;
            }
            keep_spaces += usize::from(msg.keep_spaces());
            for slot in msg.slots().iter().filter(|s| s.caret) {
                if slot.label.is_some() {
                    labelled_carets += 1;
                } else {
                    bare_carets += 1;
                }
            }
            let r = render(&msg, &empty, &plain);
            if !r.is_clean() {
                not_clean += 1;
            }
            own_tags += usize::from(r.tags.is_some());
            empty_out += usize::from(r.text.is_empty());
            colored += usize::from(r.text.spans().iter().any(|s| s.style.color.is_some()));
            let text = r.plain_text();
            // The '^' modifier (spec §4.2.1) is never rendered.
            if msg.slots().iter().any(|s| s.caret) {
                assert!(
                    !text.contains('^'),
                    "{did:#010x}/{:#010x}: {text:?}",
                    entry.token
                );
            }
            if text.contains(":{") && text.contains('#') && text.contains('}') {
                // Possible unprocessed `#N:{…}`; counted for inspection, not asserted.
                leftover_selectors += 1;
            }
            let x = render(&msg, &empty, &explorer);
            // Link labels are often a slot, so count links in explorer mode.
            links += usize::from(x.text.spans().iter().any(|s| s.style.link.is_some()));
            assert!(
                x.is_clean(),
                "{did:#010x}/{:#010x}: {:?}",
                entry.token,
                x.diagnostics
            );
            // The format string round-trips through the parser.
            let f = msg.format_string(None);
            let back = Message::from_format_string(&f);
            assert_eq!(
                back.format_string(None),
                f,
                "{did:#010x}/{:#010x}",
                entry.token
            );
        }
    }

    eprintln!(
        "string tables: {tables} of {} listed; entries: {entries}",
        dids.len()
    );
    eprintln!(
        "rendered plain + explorer: {entries} ({:.3}% of entries), not clean: {not_clean}",
        if entries == 0 {
            0.0
        } else {
            100.0 * (entries - not_clean) as f64 / entries as f64
        }
    );
    eprintln!(
        "with slots: {with_slots}; own tag group: {own_tags}; empty output: {empty_out}; \
         keepspaces: {keep_spaces}; coloured: {colored}; links: {links}; \
         possible leftover selectors: {leftover_selectors}"
    );
    eprintln!("'^' modifiers before a slot: {labelled_carets} labelled, {bare_carets} bare");
    for (k, (n, samples)) in &issues {
        eprintln!("grammar issue {k}: {n} (e.g. {samples:?})");
    }
    let stats = strings.cache_stats();
    eprintln!("cache: {stats:?}");

    assert!(
        decode_errors.is_empty(),
        "decode errors: {:?}",
        &decode_errors[..decode_errors.len().min(10)]
    );
    assert_eq!(not_clean, 0);
    let invariant_violations: usize = issues
        .iter()
        .filter(|(k, _)| k.starts_with("SG"))
        .map(|(_, (n, _))| n)
        .sum();
    assert_eq!(
        invariant_violations, 0,
        "grammar invariants SG1–SG9 violated: {issues:?}"
    );
}

#[test]
fn resolve_enum_display_values() {
    let Some(strings) = open_strings() else {
        return;
    };
    let Some(dir) = game_dir() else { return };
    let general = Archive::open(dir.join("client_general.dat")).expect("open general");
    let plain = RenderOptions::default();
    let empty = Values::new();
    let mut mappers = 0usize;
    let mut displays = 0usize;
    let mut resolved = 0usize;
    let mut literals = 0usize;
    let mut token_misses = 0usize;
    let mut other_misses = Vec::new();
    for entry in general.entries_range(0x2300_0000..0x2400_0000) {
        let entry = entry.expect("directory");
        let bytes = general.read_entry(&entry).expect("read");
        let Ok(mapper) = EnumMapper::parse_expecting(&bytes, entry.did) else {
            continue;
        };
        mappers += 1;
        for (_, info) in mapper.displays() {
            displays += 1;
            if matches!(info.source, tessera_formats::StringSource::Literal(_)) {
                literals += 1;
            }
            let r = strings.resolve(info, &empty, &plain);
            match r.diagnostics.as_slice() {
                [] => resolved += 1,
                [Diagnostic::Miss(m)] if m.reason == MissReason::NoToken => token_misses += 1,
                d => other_misses.push(format!("{:#010x}: {d:?}", entry.did)),
            }
        }
    }
    eprintln!(
        "enum mappers: {mappers}; display values: {displays} (literals {literals}); \
         resolved: {resolved}; token misses: {token_misses}; other: {}",
        other_misses.len()
    );
    // SG10: every miss is a missing token in a table that exists.
    assert!(
        other_misses.is_empty(),
        "{:?}",
        &other_misses[..other_misses.len().min(10)]
    );
    assert!(resolved > 0);
}

#[test]
fn named_tables_resolve() {
    let Some(strings) = open_strings() else {
        return;
    };
    let Some(dir) = game_dir() else { return };
    let general = Archive::open(dir.join("client_general.dat")).expect("open general");
    let named = NamedTables::load(&general, ROOT_DID_MAP).expect("named tables");
    let mut present = 0usize;
    for t in named.tables() {
        if strings.table(t.did).expect("read").is_some() {
            present += 1;
        }
    }
    eprintln!(
        "named string tables: {} (present in locale: {present})",
        named.tables().len()
    );
    assert!(!named.tables().is_empty());
    assert_eq!(present, named.tables().len());
}
