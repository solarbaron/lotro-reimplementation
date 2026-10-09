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
fn ring_enters_in_client_order_and_leaves_with_hysteresis() {
    let s = FrillSettings::default();
    let mut ring = FrillRing::new(&s);
    assert_eq!(ring.radius(), 11);
    let here = WorldPos::new(2, 48.0 * 160.0 + 80.0, 80.0 * 160.0 + 80.0, 0.0);
    let d = ring.update(&here);
    let c = ring.centre().unwrap();
    assert_eq!(d.entered[0], c);
    assert!(d.left.is_empty());
    let off = |k: &FrillSectorKey| {
        (
            i32::from(k.gx) - i32::from(c.gx),
            i32::from(k.gy) - i32::from(c.gy),
        )
    };
    let dist = |k: &FrillSectorKey| {
        let (dx, dy) = off(k);
        dx.abs().max(dy.abs())
    };
    assert!(d.entered.windows(2).all(|w| dist(&w[0]) <= dist(&w[1])));
    assert!(d.entered.iter().all(|k| dist(k) <= 11));
    // The client's square: (2 · 11 + 1)² sectors, each once.
    assert_eq!(ring.len(), 23 * 23);
    assert_eq!(d.entered.len(), 23 * 23);
    // Ring 1 in the client's order: south-west corner, north along the west column, east
    // along the north row, south along the east column, west along the south row.
    let ring1: Vec<(i32, i32)> = d.entered[1..9].iter().map(off).collect();
    assert_eq!(
        ring1,
        [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
            (1, 0),
            (1, -1),
            (0, -1)
        ]
    );
    let ring2: Vec<(i32, i32)> = d.entered[9..25].iter().map(off).collect();
    assert_eq!(ring2.first(), Some(&(-2, -2)));
    assert_eq!(ring2.get(4), Some(&(-2, 2)));
    assert_eq!(ring2.get(8), Some(&(2, 2)));
    assert_eq!(ring2.get(12), Some(&(2, -2)));
    assert_eq!(ring2.last(), Some(&(-1, -2)));
    let n = ring.len();
    // Standing still changes nothing.
    assert_eq!(ring.update(&here), FrillRingDiff::default());
    // One sector east: one column enters; nothing leaves yet (hysteresis).
    let east = WorldPos::new(2, here.x + 10.0, here.y, 0.0);
    let d = ring.update(&east);
    assert_eq!(d.entered.len(), 23);
    assert!(d.left.is_empty());
    // Far away: everything old leaves.
    let far = WorldPos::new(2, here.x + 2_000.0, here.y, 0.0);
    let d = ring.update(&far);
    assert!(d.left.len() >= n);
    // Another region: everything leaves.
    let other = WorldPos::new(3, far.x, far.y, 0.0);
    let held = ring.len();
    let d = ring.update(&other);
    assert_eq!(d.left.len(), held);
    // Off the grid: everything leaves.
    let off_grid = WorldPos::new(2, -500.0, -500.0, 0.0);
    let d = ring.update(&off_grid);
    assert_eq!(ring.len(), 0);
    assert!(!d.left.is_empty());
    // Radius 0 holds nothing.
    let mut none = FrillRing::new(&FrillSettings {
        distance_level: 0,
        ..FrillSettings::default()
    });
    assert!(none.update(&here).entered.is_empty());
}

/// Shortest horizontal distance in units between the 10 × 10 squares of two sectors at
/// offset (dx, dy).
fn square_gap(dx: i32, dy: i32) -> f64 {
    let gap = |d: i32| f64::from((d.abs() - 1).max(0)) * f64::from(FRILL_SECTOR_SIZE);
    gap(dx).hypot(gap(dy))
}

#[test]
fn sc26_sector_set_holds_every_sector_within_the_far_distance() {
    // SC26: n = 6 (level 3 dialled back), e = 1, D = 57: the sectors at (±5, ±5) lie
    // outside a disc of 7 sectors but within D of the centre sector; the square holds them.
    let s = FrillSettings {
        distance_level: 3,
        dial_back: true,
        ..FrillSettings::default()
    };
    assert_eq!(s.radius_sectors(), 6);
    assert!((s.distances().far - 57.0).abs() < 1e-4);
    let here = WorldPos::new(2, 48.0 * 160.0 + 85.0, 80.0 * 160.0 + 85.0, 0.0);
    let mut ring = FrillRing::new(&s);
    ring.update(&here);
    let c = ring.centre().unwrap();
    let held: std::collections::BTreeSet<(i32, i32)> = ring
        .sectors()
        .map(|k| {
            (
                i32::from(k.gx) - i32::from(c.gx),
                i32::from(k.gy) - i32::from(c.gy),
            )
        })
        .collect();
    for corner in [(5, 5), (-5, 5), (5, -5), (-5, -5)] {
        assert!(held.contains(&corner), "{corner:?}");
        assert!(square_gap(corner.0, corner.1) < 57.0);
    }
    // The Tessera Decision of §8.2, for every level, both dial-back settings and e = 0, 1,
    // 2: every sector with some point within D of the centre sector is held.
    for level in 1..=5 {
        for dial_back in [false, true] {
            for extra in 0..=2 {
                let s = FrillSettings {
                    distance_level: level,
                    dial_back,
                    extra_precache_levels: extra,
                    ..FrillSettings::default()
                };
                let d = f64::from(s.distances().far);
                let mut ring = FrillRing::new(&s);
                ring.update(&here);
                let c = ring.centre().unwrap();
                let span = i32::try_from(s.radius_sectors()).unwrap() + 3;
                for dy in -span..=span {
                    for dx in -span..=span {
                        if square_gap(dx, dy) >= d {
                            continue;
                        }
                        let k = FrillSectorKey {
                            region: c.region,
                            gx: u16::try_from(i32::from(c.gx) + dx).unwrap(),
                            gy: u16::try_from(i32::from(c.gy) + dy).unwrap(),
                        };
                        assert!(
                            ring.sectors().any(|h| *h == k),
                            "level {level} dial back {dial_back} e {extra}: ({dx}, {dy})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn capped_ring_still_holds_every_sector_within_the_far_distance() {
    // §8.2 Tessera Decision with non-default settings: the ring holds the client's square
    // (n + e) or every sector within D of the centre sector. Without depth transitions
    // D = 1,000 units, so a large `ExtraPreCacheRadiusLevels` must not cut the ring below
    // ⌊D / S⌋ + 1 = 101 sectors.
    assert_eq!(far_reach_sectors(1_000.0), 101);
    assert_eq!(far_reach_sectors(97.0), 10);
    assert_eq!(far_reach_sectors(0.0), 0);
    assert_eq!(far_reach_sectors(f32::NAN), 0);
    assert_eq!(far_reach_sectors(f32::INFINITY), 0);
    let here = WorldPos::new(2, 128.0 * 160.0 + 85.0, 128.0 * 160.0 + 85.0, 0.0);
    let off = FrillSettings {
        use_depth_transitions: false,
        extra_precache_levels: 1_000,
        ..FrillSettings::default()
    };
    assert!((off.distances().far - 1_000.0).abs() < 1e-3);
    let mut ring = FrillRing::new(&off);
    assert_eq!(ring.radius(), 101);
    ring.update(&here);
    let c = ring.centre().unwrap();
    let held: std::collections::BTreeSet<(i32, i32)> = ring
        .sectors()
        .map(|k| {
            (
                i32::from(k.gx) - i32::from(c.gx),
                i32::from(k.gy) - i32::from(c.gy),
            )
        })
        .collect();
    let mut within = 0usize;
    for dy in -103..=103 {
        for dx in -103..=103 {
            if square_gap(dx, dy) <= 1_000.0 {
                within += 1;
                assert!(held.contains(&(dx, dy)), "({dx}, {dy})");
            }
        }
    }
    assert!(within > 0);
    // Below the cap the client's square applies, also beyond the old fixed cap of 64
    // sectors: n + e = 10 + 70 = 80 ≤ 101.
    let square = FrillSettings {
        extra_precache_levels: 70,
        ..off
    };
    assert_eq!(FrillRing::new(&square).radius(), 80);
    // With depth transitions D = (n − 0.3) · S ≤ n sectors, so the fixed cap of 64 already
    // covers D.
    let on = FrillSettings {
        extra_precache_levels: 1_000,
        ..FrillSettings::default()
    };
    assert_eq!(FrillRing::new(&on).radius(), MAX_RING_RADIUS);
    assert!(far_reach_sectors(on.distances().far) <= MAX_RING_RADIUS);
    // Saturating: the largest setting does not overflow.
    let huge = FrillSettings {
        extra_precache_levels: u32::MAX,
        ..off
    };
    assert_eq!(FrillRing::new(&huge).radius(), 101);
}

#[test]
fn sc25_ring_centre() {
    let cam = WorldPos::new(1, 0.0, 0.0, 0.0);
    let player = WorldPos::new(1, 10.0, 0.0, 4.0);
    let at = |p: WorldPos| (p.region, p.x, p.y, p.z);
    let centre = |blend: f32| at(ring_centre(&cam, &player, blend));
    assert_eq!(centre(1.0), at(player));
    assert_eq!(centre(0.5), (1, 5.0, 0.0, 2.0));
    assert_eq!(centre(2.0), (1, 20.0, 0.0, 8.0), "not clamped");
    for blend in [0.0, -0.0, -0.5, -1.0, f32::NEG_INFINITY] {
        assert_eq!(centre(blend), at(cam), "blend {blend}");
    }
    for blend in [f32::NAN, f32::INFINITY] {
        assert_eq!(centre(blend), at(player), "blend {blend}");
    }
    // No player: the camera, whatever the blend.
    for blend in [1.0, 0.5, 2.0, f32::NAN, -1.0] {
        assert_eq!(at(ring_centre_of(&cam, None, blend)), at(cam));
    }
    // Camera and player in different regions: the player above 0, the camera otherwise.
    let elsewhere = WorldPos::new(2, 10.0, 0.0, 4.0);
    assert_eq!(at(ring_centre(&cam, &elsewhere, 0.5)), at(elsewhere));
    assert_eq!(at(ring_centre(&cam, &elsewhere, f32::NAN)), at(elsewhere));
    assert_eq!(at(ring_centre(&cam, &elsewhere, -1.0)), at(cam));
    assert_eq!(at(ring_centre(&cam, &elsewhere, 0.0)), at(cam));
}

#[test]
fn frill_booleans_follow_rule_8() {
    // §8.3 rule 8 (S52, SC29): only a stored boolean; every other type counts as absent.
    use crate::scenery::value_flag;
    assert_eq!(value_flag(&PropertyValue::Bool(false)), Some(false));
    assert_eq!(value_flag(&PropertyValue::Bool(true)), Some(true));
    let wide = tessera_formats::BitfieldAny {
        bits: 72,
        bytes: vec![0, 0, 0, 0, 0, 0, 0, 0, 1],
    };
    for v in [
        PropertyValue::Int(-2),
        PropertyValue::Int(0),
        PropertyValue::Enum(1),
        PropertyValue::Bitfield64(0),
        PropertyValue::BitfieldAny(wide),
        PropertyValue::DidRef(Did(0x4700_0001)),
        PropertyValue::StringToken(5),
        PropertyValue::PropertyId(7),
        PropertyValue::InstanceId(9),
        PropertyValue::TriState(1),
        PropertyValue::Float(1.0),
    ] {
        assert_eq!(value_flag(&v), None, "{v:?}");
    }
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
