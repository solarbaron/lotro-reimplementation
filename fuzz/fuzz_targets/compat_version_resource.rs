// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::version_resource`: the PE header, section
//! table and resource directory walk, the `VS_VERSIONINFO` block parser, and
//! the network version lookup of decision 0042 item 1.
//!
//! Raw input rarely forms a PE image, so half of the modes start from valid
//! synthetic images (the builder shared with the crate's tests) and apply the
//! input as byte patches and a truncation. No corpus file is needed.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_compat::login::network_version_from_resource;
use tessera_compat::version_resource::{
    VersionResource, parse_version_info, read_version_resource,
};

#[path = "../../crates/compat/tests/support/pe_builder.rs"]
mod pe_builder;

fn exercise(resource: &VersionResource) {
    let _ = resource.string_pairs().count();
    if let Some((key, _)) = resource.string_pairs().next() {
        let key = key.to_owned();
        let _ = network_version_from_resource(resource, &key);
    }
    let _ = network_version_from_resource(resource, "SyntheticBuild");
}

/// Applies `(u16 LE offset, value)` patches from `patches` to `image`, then
/// truncates it by `cut` bytes.
fn patched(mut image: Vec<u8>, cut: u8, patches: &[u8]) -> Vec<u8> {
    let len = image.len();
    for &[lo, hi, value] in patches.as_chunks::<3>().0 {
        let offset = usize::from(u16::from_le_bytes([lo, hi]));
        if len > 0
            && let Some(byte) = image.get_mut(offset % len)
        {
            *byte = value;
        }
    }
    let keep = image.len().saturating_sub(usize::from(cut));
    image.truncate(keep);
    image
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let cut = rest.first().copied().unwrap_or(0) & 0x0F;
    let patches = rest.get(1..).unwrap_or_default();
    match mode & 3 {
        0 => {
            if let Ok(resource) = read_version_resource(rest) {
                exercise(&resource);
            }
        }
        1 => {
            if let Ok(resource) = parse_version_info(rest) {
                exercise(&resource);
            }
            let block = patched(pe_builder::version_info(pe_builder::STRINGS), cut, patches);
            if let Ok(resource) = parse_version_info(&block) {
                exercise(&resource);
            }
        }
        m => {
            let image = patched(pe_builder::synthetic_pe(m == 2), cut, patches);
            if let Ok(resource) = read_version_resource(&image) {
                exercise(&resource);
            }
        }
    }
});
