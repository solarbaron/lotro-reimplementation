// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_wstate::ObjectState::parse` on arbitrary bytes (`docs/specs/wstate.md`): the
//! header, import list, bundle (labels, class table, items, every native codec), link and
//! available-property sections must never panic and must stay within bounded memory. The
//! graph queries (resolution, reachability, reference counts, typed attribute access) run
//! on whatever decodes.
//!
//! The first input byte selects the decode context: bit 0 decodes with a small synthetic
//! class library (classes 1–3 with every value type), bit 1 with a synthetic property
//! schema (so native property values 39/166 can decode). The rest is the record; its own
//! DID is accepted whatever it is.

#![no_main]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::{NoSchema, PropertyType};
use tessera_wstate::testkit::LibraryBuilder;
use tessera_wstate::{ClassLibrary, DecodeContext, ObjectHeader, ObjectState, ValueType};

fn library() -> &'static ClassLibrary {
    static LIB: OnceLock<ClassLibrary> = OnceLock::new();
    LIB.get_or_init(|| {
        let b = LibraryBuilder::new(4)
            .define(1, &[(0x11, ValueType::Reference), (0x22, ValueType::Int)])
            .define(
                2,
                &[
                    (0x11, ValueType::Reference),
                    (0x22, ValueType::Int),
                    (0x33, ValueType::Int64),
                    (0x44, ValueType::Double),
                    (0x55, ValueType::Float),
                    (0x66, ValueType::DoubleSigned),
                ],
            )
            .define(3, &[])
            .parent(2, 1);
        ClassLibrary::parse(&b.build()).expect("synthetic library")
    })
}

fn schema() -> &'static BTreeMap<u32, PropertyType> {
    static S: OnceLock<BTreeMap<u32, PropertyType>> = OnceLock::new();
    S.get_or_init(|| {
        (1..=22)
            .map(|code| (0x100 + code, PropertyType::from_code(code)))
            .collect()
    })
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, record)) = data.split_first() else {
        return;
    };
    let _ = ObjectHeader::parse(record);
    let _ = ObjectHeader::parse_with_imports(record);
    let ctx = DecodeContext {
        library: (mode & 1 != 0).then(library),
        schema: if mode & 2 != 0 { schema() } else { &NoSchema },
    };
    let Ok(state) = ObjectState::parse_unchecked(record, ctx) else {
        return;
    };
    let _ = state.reachable(0).len();
    let _ = state.reference_counts().len();
    let _ = state.did_items().count();
    for item in state.items().iter().take(4096) {
        let _ = item.class();
    }
    for i in 0..state.items().len().min(256) {
        let Some(view) = state.item(i) else { continue };
        for r in view.referenced().take(64) {
            let _ = r.is_null();
        }
        if let Some(s) = view.as_script() {
            for a in s.attributes() {
                let _ = s.follow(a.hash);
            }
        }
    }
    for label in [0, 1, 2, u32::MAX] {
        let _ = state.item_by_label(label);
    }
});
