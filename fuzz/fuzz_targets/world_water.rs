// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock water and water volumes (`docs/specs/water.md` §6–§7, §9) from hostile
//! inputs: water tables with arbitrary keys, duplicate entries and missing defaults; block
//! maps with arbitrary keys and values; random height grids; monitor property sets decoded
//! from arbitrary bytes or written by the harness (§7.1: an array of monitor structs with
//! hostile corners, offsets, flags, names and property DIDs, some elements not structs,
//! some fields missing, sometimes more monitors than `MAX_MONITORS_PER_OBJECT`); liquid
//! boxes with arbitrary corners, rotations and scales; query points anywhere (NaN, ±∞, far
//! off the block).
//!
//! Input layout: byte 0 selects the monitor set's form (even: the section as it is, odd:
//! written from the section's bytes), byte 1 is the length of that section, which follows;
//! the rest drives the water decision, the boxes, the queries and the fog-zone table. The
//! set sits at a fixed place, so its decoding does not depend on every earlier read.
//!
//! Nothing may panic. Invariants: a wet decision has *h* = `water_height(level)` bit for bit
//! and *h* > the lowest sample (§6.2); deciding twice gives bit-identical results (WA6); a
//! water sample's surface is at or above the query point and finite; "under the plane"
//! implies "under water"; the sector of a landblock lists each of its pairs once; at most
//! `MAX_MONITORS_PER_OBJECT` monitors are read, each with a finite box and no zero
//! property DID.
//!
//! Run with e.g. `cargo fuzz run world_water -- -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{BlockMapEntry, HeightGrid};
use tessera_formats::setup::Quaternion;
use tessera_formats::water::{
    FogZoneTable, WaterTable, WaterType, WetWater, decode_fog_zone_table,
};
use tessera_formats::{BlockMap, Did, PropertySet, PropertyType, TSize};
use tessera_physics::WaterQuery;
use tessera_physics::geometry::LiquidFlags;
use tessera_world::LandblockKey;
use tessera_world::water::{
    BlockWater, MAX_MONITORS_PER_OBJECT, MonitorPids, SectorWater, VolumeBox, VolumeTemplate,
    decide_water, fog_zone_corners, monitors_from_value, water_height,
};

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn f32(&mut self) -> f32 {
        const PICKS: [f32; 16] = [
            0.0, 1.0, -1.0, 0.5, 0.0002, -0.0002, 160.0, -1600.0, 149.0196, 80.0, 1e30, -1e30,
            0.25, -0.5, 1000.0, 1.0e-7,
        ];
        let tag = self.u8();
        if tag < 200 {
            PICKS[usize::from(tag % 16)]
        } else if tag < 240 {
            f32::from(self.u16()) / 64.0 - 512.0
        } else {
            f32::from_bits(self.u32())
        }
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.f32(), self.f32(), self.f32()]
    }
}

fn table(b: &mut Bytes<'_>) -> Option<WaterTable> {
    if b.u8().is_multiple_of(8) {
        return None;
    }
    let wet = |b: &mut Bytes<'_>, key: u32| WaterType {
        key,
        wet: (!b.u8().is_multiple_of(4)).then(|| WetWater {
            surface: Did(0x3100_0000 | u32::from(b.u8())),
            extra: [Did(0); 4],
            volume_entity: Did(0x4700_0000 | u32::from(b.u8())),
            value: 24.0,
        }),
    };
    let default = (!b.u8().is_multiple_of(3)).then(|| wet(b, 0));
    let n = usize::from(b.u8() % 12);
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let k = u32::from(b.u8() % 8);
        entries.push(wet(b, k));
    }
    let keys = [4, 7, b.u32()];
    let level_key = keys[usize::from(b.u8() % 3)];
    let type_key = keys[usize::from(b.u8() % 3)];
    Some(WaterTable::new(
        Did(0x1600_0000),
        0,
        level_key,
        type_key,
        default,
        entries,
    ))
}

fn block_map(b: &mut Bytes<'_>, key: LandblockKey) -> BlockMap {
    let n = usize::from(b.u8() % 6);
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let k = match b.u8() % 4 {
            0 => 4,
            1 => 7,
            2 => 3,
            _ => b.u32(),
        };
        let index = if b.u8().is_multiple_of(2) {
            u16::from(b.u8() % 8)
        } else {
            b.u16()
        };
        entries.push(BlockMapEntry { key: k, index });
    }
    BlockMap {
        did: key.block_map_did(),
        size: TSize {
            hint: 0,
            count: u32::try_from(n).unwrap_or(0),
        },
        entries,
    }
}

fn heights(b: &mut Bytes<'_>) -> HeightGrid {
    let base = b.u16();
    let spread = b.u16();
    let seed = b.u32();
    HeightGrid::from_fn(|i, j| {
        let h = (seed ^ u32::try_from(i * 33 + j).unwrap_or(0)).wrapping_mul(2_654_435_761);
        base.wrapping_add(u16::try_from(h % (u32::from(spread) + 1)).unwrap_or(0))
    })
}

fn boxes(b: &mut Bytes<'_>, h: f32) -> Vec<tessera_physics::geometry::LiquidBox> {
    let n = usize::from(b.u8() % 6);
    let mut tpl = VolumeTemplate {
        object: Did(0x4700_0001),
        boxes: Vec::new(),
        unreadable_sets: 0,
    };
    for _ in 0..n {
        let flags = b.u8();
        tpl.boxes.push(VolumeBox {
            min: b.v3(),
            max: b.v3(),
            scale_with_entity: flags & 1 != 0,
            flags: LiquidFlags {
                liquid: flags & 2 != 0,
                lava: flags & 4 != 0,
                surface: flags & 8 != 0,
                wading: flags & 16 != 0,
                deep: flags & 32 != 0,
            },
            properties: Did(0x7800_0073),
            name: None,
        });
    }
    let mut out = tpl.place_landblock(h);
    let rotation = Quaternion {
        w: b.f32(),
        x: b.f32(),
        y: b.f32(),
        z: b.f32(),
    };
    out.extend(tpl.place(b.v3(), rotation, b.v3()));
    out
}

fn schema() -> HashMap<u32, PropertyType> {
    [
        (1, 12),
        (2, 11),
        (3, 6),
        (4, 6),
        (5, 6),
        (6, 21),
        (7, 20),
        (8, 8),
    ]
    .into_iter()
    .map(|(p, t)| (p, PropertyType::from_code(t)))
    .collect()
}

/// A monitor property set (TSize, then PID, echo and value; `schema`): PID 1, the monitor
/// array, whose elements are mostly PID-2 structs of box corners (3, 4), offset (5), flag
/// (6), property DID (7) and name (8), each field present or not.
fn monitor_set(b: &mut Bytes<'_>) -> Vec<u8> {
    let mut out = vec![0, 1]; // TSize: hint 0, one entry
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    let n: u32 = if b.u8().is_multiple_of(16) {
        u32::try_from(MAX_MONITORS_PER_OBJECT).unwrap_or(64) + u32::from(b.u8() % 8)
    } else {
        u32::from(b.u8() % 5)
    };
    out.extend_from_slice(&n.to_le_bytes());
    for _ in 0..n {
        let kind = b.u8();
        if kind % 8 == 7 {
            // An element that is not a struct.
            out.extend_from_slice(&3u32.to_le_bytes());
            for c in b.v3() {
                out.extend_from_slice(&c.to_le_bytes());
            }
            continue;
        }
        out.extend_from_slice(&2u32.to_le_bytes());
        // Fields 3 and 4 mostly present, the others now and then.
        let fields: Vec<u32> = (3..=8u32)
            .filter(|f| {
                let bit = 1u8 << (f - 3);
                if *f <= 4 {
                    kind & bit == 0 || kind & 0x40 == 0
                } else {
                    kind & bit != 0
                }
            })
            .collect();
        out.push(0);
        out.push(u8::try_from(fields.len()).unwrap_or(0));
        for f in fields {
            out.extend_from_slice(&f.to_le_bytes());
            out.extend_from_slice(&f.to_le_bytes());
            match f {
                3..=5 => {
                    for c in b.v3() {
                        out.extend_from_slice(&c.to_le_bytes());
                    }
                }
                6 => out.push(b.u8() % 2),
                7 => {
                    let d = if b.u8().is_multiple_of(2) {
                        0
                    } else {
                        0x7800_0073
                    };
                    out.extend_from_slice(&u32::to_le_bytes(d));
                }
                _ => out.extend_from_slice(&b.u32().to_le_bytes()),
            }
        }
    }
    out
}

fuzz_target!(|input: &[u8]| {
    let [form, len, rest @ ..] = input else {
        return;
    };
    let (section, rest) = rest.split_at(usize::from(*len).min(rest.len()));
    let set_bytes = if form.is_multiple_of(2) {
        section.to_vec()
    } else {
        monitor_set(&mut Bytes(section))
    };
    let mut b = Bytes(rest);
    let key = LandblockKey::new(1, b.u8() % 255, b.u8() % 255).expect("valid key");
    let t = table(&mut b);
    let map = block_map(&mut b, key);
    let grid = heights(&mut b);
    let decision = decide_water(t.as_ref(), &map, &grid);
    // WA6.
    let again = decide_water(t.as_ref(), &map.clone(), &grid.clone());
    match (&decision, &again) {
        (Ok(a), Ok(c)) => {
            assert_eq!(a.height.to_bits(), c.height.to_bits());
            assert_eq!(a.height.to_bits(), water_height(a.level).to_bits());
            assert!(a.height > a.lowest_terrain);
            assert!(a.plane().is_some());
        }
        (a, c) => assert_eq!(a, c),
    }
    let h = decision.as_ref().map_or(100.0, |w| w.height);
    let bw = BlockWater {
        key,
        decision,
        boxes: boxes(&mut b, h),
        placed_volume_entities: 0,
        boxes_left_out: 0,
    };
    for _ in 0..8 {
        let p = b.v3();
        let under = bw.is_under_water(p);
        if let Some(s) = bw.water_at(p) {
            assert!(s.flags.liquid);
            assert!(s.surface >= p[2], "surface {} below {}", s.surface, p[2]);
            assert!(s.surface.is_finite());
            assert!(under, "in a liquid box but not under water");
        }
        if bw.plane_at(p[0], p[1]).is_some() && bw.water().is_some_and(|w| w.is_under(p[2])) {
            assert!(under);
        }
    }
    let sector = SectorWater::build(key, |_| decision.as_ref().ok().copied());
    for (i, p) in sector.pairs.iter().enumerate() {
        assert!(
            !sector.pairs[..i]
                .iter()
                .any(|q| q.water_type == p.water_type && q.height.to_bits() == p.height.to_bits())
        );
    }
    // Monitors from the property set of the input's monitor section.
    if let Ok(set) = PropertySet::parse(&set_bytes, &schema())
        && let Some(v) = set.get(1)
    {
        let pids = MonitorPids {
            monitors: Some(1),
            box_min: Some(3),
            box_max: Some(4),
            offset: Some(5),
            scale_with_entity: Some(6),
            properties: Some(7),
            name: Some(8),
        };
        let monitors = monitors_from_value(v, &pids);
        assert!(monitors.len() <= MAX_MONITORS_PER_OBJECT);
        for m in &monitors {
            assert!(m.min.iter().chain(&m.max).all(|c| c.is_finite()));
            assert!(m.offset.iter().all(|c| c.is_finite()));
            assert!(m.properties.is_none_or(|d| d.0 != 0));
            let _ = m.local_box();
        }
    }
    // Fog-zone corners of an arbitrary table.
    if let Ok(fog) = decode_fog_zone_table(b.0) {
        let fog: FogZoneTable = fog;
        let values = [Some(b.u16()), None, Some(0), Some(b.u16())];
        if let Some(c) = fog_zone_corners(&fog, values) {
            assert!(c.iter().flatten().any(|z| z.is_active()));
        }
    }
});
