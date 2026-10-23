// SPDX-License-Identifier: GPL-3.0-or-later
//! The far view's `[Render]` preferences (`far-view.md` FD16–FD18, `install-layout.md` §8.3):
//! [`FarViewSettings::apply_render_preferences`] with hostile text for every key, and the
//! `LandscapeLightingQuality` level reading. Nothing may panic; the graphics core stays 5,
//! the seam switch stays on, the radii stay valid, and the overlay is never on without the
//! normal-mapped template.
//!
//! Input layout: the (lossy) UTF-8 text is split at `\n` into at most eight values, given
//! in turn to the keys the far view reads (and to two it must not read).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_world::far_view::{FarViewSettings, Template, lighting_quality_level};

const KEYS: [&str; 6] = [
    "FarLandscapeNormalMaps",
    "MultiPassLighting",
    "LandscapeLightingQuality",
    "DistantImposters",
    "NearFarSeamBlending",
    "GraphicsCore",
];

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let values: Vec<&str> = text.split('\n').take(8).collect();
    let _ = lighting_quality_level(&text);
    for v in &values {
        let _ = lighting_quality_level(v);
    }
    let mut s = FarViewSettings::default();
    s.apply_render_preferences(|k| {
        KEYS.iter()
            .position(|n| *n == k)
            .and_then(|i| values.get(i).copied())
    });
    assert_eq!(s.graphics_core, 5);
    assert!(s.seam_blending);
    assert!(s.validate().is_ok());
    if s.seam_overlay() {
        assert_eq!(s.template(), Template::NormalMapped);
        assert!(s.lighting_quality > 0 && s.multi_pass_lighting);
    }
});
