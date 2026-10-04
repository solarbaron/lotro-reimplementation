// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-text` on hostile input (`docs/specs/string-table.md`): message grammar
//! (selectors, tag groups, slots), markup, `${NAME}` format strings and templates, and a
//! string-table record rendered through `Strings`. Nothing may panic and output stays
//! within the configured bound.
//!
//! Input layout: the first byte picks the mode and a slot ID offset; the rest is split on
//! 0x00 into message pieces (lossy UTF-8). The whole input is also tried as a string-table
//! record payload.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_text::{
    MemorySource, Message, RenderMode, RenderOptions, Strings, TagSet, Value, Values, display_text,
    parse_format_string, parse_markup, parse_options, parse_tag_list, render, render_template,
    select_option, split_tag_group,
};

const MAX_OUTPUT: usize = 1 << 16;

fuzz_target!(|data: &[u8]| {
    let Some((&head, rest)) = data.split_first() else {
        return;
    };
    let mode = if head & 1 == 0 {
        RenderMode::Plain
    } else {
        RenderMode::Explorer
    };
    let opts = RenderOptions {
        max_output: MAX_OUTPUT,
        ..RenderOptions::mode(mode)
    };

    let text = String::from_utf8_lossy(rest);
    let pieces: Vec<String> = rest
        .split(|&b| b == 0)
        .take(16)
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect();
    let ids: Vec<u32> = (0..pieces.len().saturating_sub(1) as u32)
        .map(|i| (i + u32::from(head >> 1)) % 6)
        .collect();

    // Values: plain text, numbers and one nested message built from the same pieces.
    let mut values = Values::new();
    for (i, p) in pieces.iter().enumerate().take(4) {
        let id = i as u32;
        match i % 3 {
            0 => values.insert(id, Value::Text(p.clone())),
            1 => values.insert(id, Value::Integer(p.len() as i64 - 1)),
            _ => values.insert(id, Value::Float(p.len() as f64 / 3.0)),
        };
    }
    let nested = Message::new(pieces.clone(), ids.clone(), None);
    values.insert(5, Value::message(nested, values.clone()));

    let msg = Message::new(pieces, ids, None);
    let r = render(&msg, &values, &opts);
    assert!(r.text.plain_text().len() <= MAX_OUTPUT);
    let fs = msg.format_string(None);
    let _ = Message::from_format_string(&fs);

    let _ = parse_markup(&text);
    let _ = display_text(&text);
    let (_, group) = split_tag_group(&text);
    let set = TagSet::new(group.unwrap_or(""), head & 2 != 0, Some(f64::from(head)));
    let _ = parse_tag_list(&text);
    let options = parse_options(&text);
    if let Some(i) = select_option(&options, &set) {
        assert!(i < options.len());
    }
    let _ = parse_format_string(&text);
    let t = render_template(&text, &values, &opts);
    assert!(t.text.plain_text().len() <= MAX_OUTPUT);

    // The raw bytes as a string-table record (DID forced to the table's own).
    let did: u32 = 0x2500_0001;
    let mut payload = did.to_le_bytes().to_vec();
    payload.extend_from_slice(rest);
    let mut source = MemorySource::new();
    source.insert(did, payload);
    let strings = Strings::new(source);
    let _ = strings.render_entry(did, u32::from(head), &values, &opts);
});
