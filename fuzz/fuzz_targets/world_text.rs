// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-world` text and coordinate parsers (`docs/specs/coordinates.md` §4, §5, §8, §9):
//! map coordinates ("12.3S, 45.6W"), `/loc` token lines, and the conversions between
//! region-global, block-local and landcell positions, on arbitrary input.
//!
//! The input is read as (lossy) UTF-8 text for the parsers, and its first 32 bytes also as
//! four f64 values for the conversions. Nothing may panic; accepted text re-formats to a
//! string that parses back to the same value (to the displayed precision); every accepted
//! position converts into a landblock-local position within [0, 160].

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_world::{LocReport, MapConstants, MapCoords, WorldPos, yaw_from_heading, yaw_rotation};

fuzz_target!(|input: &[u8]| {
    let text = String::from_utf8_lossy(input);
    if let Ok(m) = text.parse::<MapCoords>() {
        let shown = m.to_string();
        let again: MapCoords = shown.parse().expect("formatted map coordinates parse");
        assert!((again.latitude - m.latitude).abs() <= 0.051, "{shown}");
        assert!((again.longitude - m.longitude).abs() <= 0.051, "{shown}");
        let c = MapConstants::new([29_360.0, 24_880.0], 0.005).expect("constants");
        let g = c.to_region(m);
        let back = c.to_map(g);
        // `to_map` is single precision (coordinates.md §8.2 step 3): values up to 1,000
        // map units come back within a few f32 steps.
        assert!((back.latitude - m.latitude).abs() < 1e-3, "{m:?} {back:?}");
        assert!(
            (back.longitude - m.longitude).abs() < 1e-3,
            "{m:?} {back:?}"
        );
        if let Ok(p) = c.to_world(1, m) {
            assert!(p.landblock().is_some());
        }
    }
    if let Ok(r) = text.parse::<LocReport>() {
        let shown = r.to_string();
        let again: LocReport = shown.parse().expect("formatted /loc parses");
        assert_eq!((again.region, again.lx, again.ly), (r.region, r.lx, r.ly));
        assert_eq!((again.instance, again.inside), (r.instance, r.inside));
        let _ = r.to_world();
        if let Ok(l) = r.to_local() {
            assert_eq!(l.block.region(), r.region);
        }
    }
    let mut f = [0f64; 4];
    for (i, c) in input.as_chunks::<8>().0.iter().take(4).enumerate() {
        f[i] = f64::from_le_bytes(*c);
    }
    let region = input.first().map_or(1, |b| b % 17);
    let p = WorldPos::new(region, f[0], f[1], f[2]);
    if let Some(l) = p.to_local() {
        assert!((0.0..=160.0).contains(&l.local[0]) && (0.0..=160.0).contains(&l.local[1]));
        let w = l.to_world();
        assert!((w.x - p.x).abs() < 0.01 && (w.y - p.y).abs() < 0.01);
        let cell = l.landcell();
        assert_eq!(cell.landblock(), Some(l.block));
        let r = LocReport::from_local(&l, Some(f[3]));
        let _ = r.to_string();
    }
    let _ = p.landcell();
    let q = yaw_rotation(yaw_from_heading(f[3]));
    let h = tessera_world::heading_degrees(q);
    assert!((0.0..360.0).contains(&h));
});
