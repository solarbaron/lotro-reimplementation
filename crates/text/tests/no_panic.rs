// SPDX-License-Identifier: GPL-3.0-or-later
//! Hostile-input tests: parsing and rendering never panic and stay bounded.

mod common;

use proptest::prelude::*;
use tessera_text::{
    MemorySource, Message, RenderMode, RenderOptions, Strings, Value, Values, parse_markup, render,
    render_template,
};

/// Strings biased towards grammar characters.
fn grammar_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("#".to_owned()),
            Just("#1:".to_owned()),
            Just("#-2:{".to_owned()),
            Just("{".to_owned()),
            Just("}".to_owned()),
            Just("{{keepspaces}}".to_owned()),
            Just("[".to_owned()),
            Just("]".to_owned()),
            Just("|".to_owned()),
            Just("!".to_owned()),
            Just("\\".to_owned()),
            Just("<".to_owned()),
            Just(">".to_owned()),
            Just("<rgb=#FF00".to_owned()),
            Just("</rgb>".to_owned()),
            Just("<A:B:".to_owned()),
            Just("<\\\\A>".to_owned()),
            Just("$".to_owned()),
            Just("${".to_owned()),
            Just(" ".to_owned()),
            Just("é".to_owned()),
            "[a-zA-Z0-9.,]{0,3}",
            any::<char>().prop_map(String::from),
        ],
        0..40,
    )
    .prop_map(|v| v.concat())
}

fn value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        grammar_text().prop_map(Value::Text),
        any::<i64>().prop_map(Value::Integer),
        any::<f64>().prop_map(Value::Float),
        Just(Value::Unresolved(tessera_text::Diagnostic::DepthLimit {
            limit: 1
        })),
    ];
    leaf.prop_recursive(4, 16, 3, |inner| {
        (
            proptest::collection::vec(grammar_text(), 0..4),
            proptest::collection::vec(0u32..3, 0..4),
            proptest::collection::vec(inner, 0..3),
        )
            .prop_map(|(pieces, ids, vals)| {
                let values: Values = vals
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| (i as u32, v))
                    .collect();
                Value::message(Message::new(pieces, ids, None), values)
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn render_never_panics(
        pieces in proptest::collection::vec(grammar_text(), 0..6),
        ids in proptest::collection::vec(0u32..4, 0..7),
        vals in proptest::collection::vec(value(), 0..4),
        explorer in any::<bool>(),
    ) {
        let msg = Message::new(pieces, ids, None);
        let values: Values = vals.into_iter().enumerate().map(|(i, v)| (i as u32, v)).collect();
        let mode = if explorer { RenderMode::Explorer } else { RenderMode::Plain };
        let opts = RenderOptions { max_output: 1 << 16, ..RenderOptions::mode(mode) };
        let r = render(&msg, &values, &opts);
        prop_assert!(r.text.len() <= 1 << 16);
        let _ = msg.format_string(None);
        let _ = Message::from_format_string(&msg.format_string(None));
    }

    #[test]
    fn markup_and_templates_never_panic(s in grammar_text()) {
        let _ = parse_markup(&s);
        let _ = render_template(&s, &Values::new(), &RenderOptions::default());
        let _ = tessera_text::split_tag_group(&s);
        let _ = tessera_text::display_text(&s);
    }

    #[test]
    fn hostile_table_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..256), token in 0u32..4) {
        let did: u32 = 0x2500_0001;
        let mut payload = did.to_le_bytes().to_vec();
        payload.extend_from_slice(&bytes);
        let mut s = MemorySource::new();
        s.insert(did, payload);
        let strings = Strings::new(s);
        let _ = strings.render_entry(did, token, &Values::new(), &RenderOptions::default());
    }
}

#[test]
fn pathological_inputs_are_fast() {
    let start = std::time::Instant::now();
    for unit in [
        "{",
        "{{",
        "#1:{",
        "<",
        "<b>",
        "\\",
        "[",
        "#1:",
        "${",
        "<b> </b> ",
        "<A:B:",
        ":",
    ] {
        let s = unit.repeat(20_000);
        let msg = Message::new(vec![s.clone(), s.clone()], vec![1], None);
        let values: Values = [(1u32, Value::Text(s.clone()))].into_iter().collect();
        let _ = render(&msg, &values, &RenderOptions::default());
        let _ = parse_markup(&s);
    }
    assert!(start.elapsed().as_secs() < 20, "took {:?}", start.elapsed());
}
