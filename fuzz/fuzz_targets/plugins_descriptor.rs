// SPDX-License-Identifier: GPL-3.0-or-later
//! `.plugin` descriptors, `PluginOptions.xml`, and the plugin-supplied names the host turns
//! into paths or commands (`docs/specs/plugin-descriptor.md` §3.2, §4.2, §5, §7.1, §10,
//! §11.1; `lua-plugin-api.md` §4.4, §10.2, §11.2) on arbitrary bytes: `parse_descriptor`,
//! `parse_options`, `is_valid_package_name`, `paths::split_resource_path`,
//! `paths::name_order`, `plugindata::validate_key`, `sanitize_folder_name` and
//! `split_command_line`.
//!
//! Nothing may panic. An accepted descriptor must satisfy the §5.4 invariants, and every
//! accepted path component or key must be safe to join under a folder.

#![no_main]

use std::cmp::Ordering;

use libfuzzer_sys::fuzz_target;
use tessera_plugins::descriptor::{MAX_DESCRIPTION_UNITS, MAX_DESCRIPTOR_BYTES, MAX_FIELD_UNITS};
use tessera_plugins::options::parse_options;
use tessera_plugins::paths::{
    is_valid_component, is_valid_package_name, name_order, split_resource_path,
};
use tessera_plugins::plugindata::validate_key;
use tessera_plugins::{parse_descriptor, sanitize_folder_name, split_command_line};

const TRIM: [char; 5] = [' ', '\n', '\r', '\t', '\u{0C}'];

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

fn safe_component(c: &str) -> bool {
    !c.is_empty() && c != "." && c != ".." && !c.contains(['/', '\\', ':', '\0'])
}

fuzz_target!(|data: &[u8]| {
    if let Ok(d) = parse_descriptor(data) {
        assert!(data.len() <= MAX_DESCRIPTOR_BYTES);
        assert!(!d.name.is_empty() && units(&d.name) <= MAX_FIELD_UNITS);
        assert_eq!(d.name.trim_matches(TRIM), d.name);
        assert!(!d.package.is_empty() && units(&d.package) <= MAX_FIELD_UNITS);
        assert!(units(&d.version) <= MAX_FIELD_UNITS && units(&d.author) <= MAX_FIELD_UNITS);
        if let Some(desc) = &d.description {
            assert!(!desc.is_empty() && units(desc) <= MAX_DESCRIPTION_UNITS);
        }
        if let Some(img) = &d.image {
            assert!(!img.is_empty() && units(img) <= MAX_FIELD_UNITS);
        }
        let _ = d.apartment_name();
        if is_valid_package_name(&d.package) {
            for comp in d.package_components() {
                assert!(safe_component(comp));
            }
        }
    }
    let _ = parse_options(data);

    let text = String::from_utf8_lossy(data);
    if is_valid_package_name(&text) {
        assert!(text.split('.').all(
            |c| safe_component(c) && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        ));
    }
    if let Some(parts) = split_resource_path(&text) {
        assert!(
            parts
                .iter()
                .all(|p| safe_component(p) && is_valid_component(p))
        );
    }
    if let Ok(key) = validate_key(data) {
        assert!((1..=64).contains(&key.len()));
        assert!(key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    }
    let folder = sanitize_folder_name(&text);
    assert!(
        !folder.is_empty()
            && !folder.contains(['/', '\\', '\0'])
            && folder != "."
            && folder != ".."
            && !folder.eq_ignore_ascii_case("AllServers")
            && !folder.eq_ignore_ascii_case("AllCharacters"),
        "sanitized folder {folder:?} is one path component"
    );
    if let Some((a, b)) = text.split_once('\u{1}') {
        let ab = name_order(a, b);
        assert_eq!(ab.reverse(), name_order(b, a));
        assert_eq!(ab == Ordering::Equal, a == b);
    }
    if let Some((word, rest)) = split_command_line(&text) {
        assert!(!word.is_empty() && !word.contains(char::is_whitespace));
        assert!(text.len() > word.len() + rest.len());
    }
});
