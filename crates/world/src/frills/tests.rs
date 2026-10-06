// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the frill layer (`scenery.md` §8.2–§8.4).

use tessera_formats::setup::Quaternion;
use tessera_formats::{Did, PropertyEntry, PropertySet, PropertyValue};

use super::*;
use crate::coords::{LandblockKey, WorldPos};
use crate::scenery::{ChainLevel, ObjectProperties};

#[test]
fn default_distances() {
    let s = FrillSettings::default();
    let d = s.distances();
    assert_eq!(d.radius, 10);
    assert!((d.far - 97.0).abs() < 1e-4);
    assert!((d.fade_length - 30.0).abs() < 1e-4);
    assert!((d.fade_start - 67.0).abs() < 1e-4);
    assert!((d.reduction_start - 40.0).abs() < 1e-4);
    assert!(d.reduction);
}

#[test]
fn radius_per_level_and_dial_back() {
    let radius = |level, dial_back| {
        FrillSettings {
            distance_level: level,
            dial_back,
            ..FrillSettings::default()
        }
        .radius_sectors()
    };
    assert_eq!(
        (0..=5).map(|l| radius(l, false)).collect::<Vec<_>>(),
        [0, 8, 10, 12, 14, 16]
    );
    assert_eq!(radius(6, false), 0);
    assert_eq!(radius(-1, false), 0);
    assert_eq!(radius(2, true), 5);
    assert_eq!(radius(1, true), 4);
    let off = FrillSettings {
        draw_frills: false,
        ..FrillSettings::default()
    };
    assert_eq!(off.radius_sectors(), 0);
    assert_eq!(off.distances().far, 0.0);
    // Without depth transitions D is 1,000; the fade is capped by D.
    let flat = FrillSettings {
        use_depth_transitions: false,
        ..FrillSettings::default()
    }
    .distances();
    assert_eq!(flat.far, 1_000.0);
    assert_eq!(flat.fade_start, 970.0);
    // Level 1 dialled back: radius 4, D = 37, 2·Rd = 80 > D → no reduction.
    let small = FrillSettings {
        distance_level: 1,
        dial_back: true,
        ..FrillSettings::default()
    }
    .distances();
    assert!((small.far - 37.0).abs() < 1e-4);
    assert!(!small.reduction);
}

#[test]
fn group_draw_and_reduction() {
    let d = FrillSettings::default().distances();
    assert_eq!(d.group_draw(10.0 * 10.0), GroupDraw::Plain);
    assert_eq!(d.group_draw(70.0 * 70.0), GroupDraw::Fading);
    assert_eq!(d.group_draw(97.0 * 97.0), GroupDraw::Hidden);
    assert_eq!(d.group_draw(f32::NAN), GroupDraw::Hidden);
    // Inside Rd: everything.
    assert_eq!(d.reduced_count(true, 20.0 * 20.0, 64, 8), 64);
    // At D: one granule.
    assert_eq!(d.reduced_count(true, 97.0 * 97.0, 64, 8), 8);
    // Halfway in d²: f = 0.5 → 8 × trunc(0.5 × 64 / 8) = 32.
    let mid = (40.0f32 * 40.0 + 97.0 * 97.0) / 2.0;
    assert_eq!(d.reduced_count(true, mid, 64, 8), 32);
    // Not a multiple of the granule, or not allowed: no reduction.
    assert_eq!(d.reduced_count(true, 97.0 * 97.0, 63, 8), 63);
    assert_eq!(d.reduced_count(false, 97.0 * 97.0, 64, 8), 64);
    assert_eq!(d.reduced_count(true, 97.0 * 97.0, 64, 0), 64);
}

#[test]
fn fade_in_curve() {
    assert_eq!(fade_in_opacity(0.0), 0.0);
    assert!((fade_in_opacity(0.75) - 0.5).abs() < 1e-6);
    assert_eq!(fade_in_opacity(1.5), 1.0);
    assert_eq!(fade_in_opacity(10.0), 1.0);
    assert_eq!(fade_in_opacity(-1.0), 0.0);
    assert_eq!(fade_in_opacity(f32::NAN), 0.0);
}

const P_ANIM: u32 = 1;
const P_TINT: u32 = 2;
const P_OPAQUE: u32 = 3;
const P_REDUCE: u32 = 4;

fn level(did: u32, entries: Vec<(u32, PropertyValue)>) -> ChainLevel {
    ChainLevel {
        did: Did(did),
        sets: vec![PropertySet {
            entries: entries
                .into_iter()
                .map(|(pid, value)| PropertyEntry { pid, value })
                .collect(),
            ..PropertySet::default()
        }],
    }
}

#[test]
fn object_values_resolve_along_the_chain() {
    let pids = FrillPids {
        allow_reduction: Some(P_REDUCE),
        opaque: Some(P_OPAQUE),
        animation_intensity: Some(P_ANIM),
        terrain_color_intensity: Some(P_TINT),
    };
    let s = FrillSettings::default();
    // Defaults.
    let d = FrillObjectValues::resolve(None, &pids, &s);
    assert_eq!(d, FrillObjectValues::default());
    assert_eq!(d.terrain_color_intensity, 1.0);
    assert!(!d.is_animated());
    // SC14: the parent's 0.2 when the object has none; its own value wins otherwise.
    let parent = level(
        0x4700_0002,
        vec![
            (P_ANIM, PropertyValue::Float(0.2)),
            (P_OPAQUE, PropertyValue::Bool(false)),
        ],
    );
    let chain = ObjectProperties {
        levels: vec![level(0x4700_0001, vec![]), parent.clone()],
        incomplete: false,
    };
    let v = FrillObjectValues::resolve(Some(&chain), &pids, &s);
    assert!((v.animation_intensity - 0.2).abs() < 1e-7 && v.is_animated());
    assert!(!v.opaque);
    let chain = ObjectProperties {
        levels: vec![
            level(
                0x4700_0001,
                vec![
                    (P_ANIM, PropertyValue::Float(0.1)),
                    (P_TINT, PropertyValue::Float(7.0)),
                    (P_REDUCE, PropertyValue::Bool(false)),
                ],
            ),
            parent,
        ],
        incomplete: false,
    };
    let v = FrillObjectValues::resolve(Some(&chain), &pids, &s);
    assert!((v.animation_intensity - 0.1).abs() < 1e-7);
    assert_eq!(v.terrain_color_intensity, 1.0, "clamped");
    assert!(!v.allow_distance_reduction);
    // Terrain colour off.
    let off = FrillSettings {
        terrain_color: false,
        ..FrillSettings::default()
    };
    assert_eq!(
        FrillObjectValues::resolve(Some(&chain), &pids, &off).terrain_color_intensity,
        0.0
    );
}

#[test]
fn sector_keys_and_vertices() {
    let key = LandblockKey::new(2, 48, 80).unwrap();
    let k = FrillSectorKey::new(key, 3, 7).unwrap();
    assert_eq!((k.gx, k.gy), (48 * 16 + 3, 80 * 16 + 7));
    assert_eq!(k.landblock(), Some(key));
    assert_eq!(k.local(), (3, 7));
    assert_eq!(k.centre(), [(768.0 + 3.5) * 10.0, (1280.0 + 7.5) * 10.0]);
    assert!(FrillSectorKey::new(key, 16, 0).is_none());
    let pos = WorldPos::new(2, 48.0 * 160.0 + 35.0, 80.0 * 160.0 + 79.9, 0.0);
    assert_eq!(FrillSectorKey::at(&pos), Some(k));
    assert_eq!(sector_vertices(3, 7), [(3, 7)]);
    assert_eq!(sector_vertices(15, 7), [(15, 7), (16, 7)]);
    assert_eq!(sector_vertices(3, 15), [(3, 15), (3, 16)]);
    assert_eq!(
        sector_vertices(15, 15),
        [(15, 15), (16, 15), (15, 16), (16, 16)]
    );
    // Every vertex 0–16 is covered exactly once by the 256 sectors.
    let mut seen = [[0u8; 17]; 17];
    for sx in 0..16 {
        for sy in 0..16 {
            for (i, j) in sector_vertices(sx, sy) {
                seen[i][j] += 1;
            }
        }
    }
    assert!(seen.iter().flatten().all(|&n| n == 1));
}

#[test]
fn ring_enters_nearest_first_and_leaves_with_hysteresis() {
    let s = FrillSettings::default();
    let mut ring = FrillRing::new(&s);
    assert_eq!(ring.radius(), 11);
    let here = WorldPos::new(2, 48.0 * 160.0 + 80.0, 80.0 * 160.0 + 80.0, 0.0);
    let d = ring.update(&here);
    let c = ring.centre().unwrap();
    assert_eq!(d.entered[0], c);
    assert!(d.left.is_empty());
    let dist = |k: &FrillSectorKey| {
        (i32::from(k.gx) - i32::from(c.gx))
            .abs()
            .max((i32::from(k.gy) - i32::from(c.gy)).abs())
    };
    assert!(d.entered.windows(2).all(|w| dist(&w[0]) <= dist(&w[1])));
    assert!(d.entered.iter().all(|k| dist(k) <= 11));
    let n = ring.len();
    assert!(n > 300 && n < 450, "{n}");
    // Standing still changes nothing.
    assert_eq!(ring.update(&here), FrillRingDiff::default());
    // One sector east: one column enters; nothing leaves yet (hysteresis).
    let east = WorldPos::new(2, here.x + 10.0, here.y, 0.0);
    let d = ring.update(&east);
    assert!(!d.entered.is_empty());
    assert!(d.left.is_empty());
    // Far away: everything old leaves.
    let far = WorldPos::new(2, here.x + 2_000.0, here.y, 0.0);
    let d = ring.update(&far);
    assert!(d.left.len() >= n);
    // Off the grid: everything leaves.
    let off = WorldPos::new(2, -500.0, -500.0, 0.0);
    let d = ring.update(&off);
    assert_eq!(ring.len(), 0);
    assert!(!d.left.is_empty());
    // Radius 0 holds nothing.
    let mut none = FrillRing::new(&FrillSettings {
        distance_level: 0,
        ..FrillSettings::default()
    });
    assert!(none.update(&here).entered.is_empty());
}

#[test]
fn ring_centre_blend() {
    let cam = WorldPos::new(1, 100.0, 200.0, 10.0);
    let player = WorldPos::new(1, 110.0, 220.0, 0.0);
    assert_eq!(ring_centre(&cam, &player, 1.0), player);
    assert_eq!(ring_centre(&cam, &player, 0.0), cam);
    let half = ring_centre(&cam, &player, 0.5);
    assert_eq!((half.x, half.y, half.z), (105.0, 210.0, 5.0));
    assert_eq!(ring_centre(&cam, &player, f32::NAN), player);
}

#[test]
fn thinner_is_uniform_and_keeps_all_at_full_density() {
    let mut t = DensityThinner::with_seed(42);
    assert!((0..1000).all(|_| t.keep(1.0)));
    let mut sum = 0.0f64;
    for _ in 0..10_000 {
        let u = t.next_unit();
        assert!((0.0..1.0).contains(&u));
        sum += f64::from(u);
    }
    assert!((sum / 10_000.0 - 0.5).abs() < 0.02);
    let kept = (0..10_000).filter(|_| t.keep(0.25)).count();
    assert!((2_200..2_800).contains(&kept), "{kept}");
    let _ = FrillInstance {
        object: Did(0x4700_0001),
        position: [0.0; 3],
        rotation: Quaternion::IDENTITY,
        scale: [1.0; 3],
    };
}
