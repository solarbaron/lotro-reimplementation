// SPDX-License-Identifier: GPL-3.0-or-later
//! `.plugin` descriptors and the plugin-supplied names the host turns into paths
//! (`docs/specs/lua-plugin-api.md` §4.4, §5, §10.2, §10.3) on arbitrary bytes:
//! `parse_descriptor`, `validate_package`, `paths::split_resource_path`,
//! `plugindata::validate_key` and `sanitize_folder_name`.
//!
//! Nothing may panic. An accepted descriptor must satisfy the §5.3 invariants, and every
//! accepted path component or key must be safe to join under a folder.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_script::descriptor::{MAX_DESCRIPTOR_BYTES, validate_package};
use tessera_script::paths::{is_valid_component, split_resource_path};
use tessera_script::plugindata::validate_key;
use tessera_script::{parse_descriptor, sanitize_folder_name};

fn safe_component(c: &str) -> bool {
    !c.is_empty() && c != "." && c != ".." && !c.contains(['/', '\\', ':', '\0'])
}

fuzz_target!(|data: &[u8]| {
    if let Ok(d) = parse_descriptor(data) {
        assert!(data.len() <= MAX_DESCRIPTOR_BYTES);
        assert!(!d.name.is_empty() && !d.name.chars().any(char::is_control));
        assert_eq!(d.name.trim(), d.name);
        assert!(validate_package(&d.package).is_ok());
        for comp in d.package_components() {
            assert!(!comp.is_empty() && !comp.contains(['/', '\\', ':', '\0']));
        }
        if let Some(ap) = &d.apartment {
            assert!(!ap.is_empty() && !ap.chars().any(char::is_control));
        }
    }

    let text = String::from_utf8_lossy(data);
    if validate_package(&text).is_ok() {
        assert!(text.split('.').all(|c| !c.is_empty()));
    }
    if let Some(parts) = split_resource_path(&text) {
        assert!(
            parts
                .iter()
                .all(|p| safe_component(p) && is_valid_component(p))
        );
    }
    if let Ok(key) = validate_key(data) {
        assert!((1..=63).contains(&key.len()));
        assert!(safe_component(key), "key {key:?} is one safe file name");
        assert!(!key.ends_with('.') && !key.ends_with(' '));
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
});
