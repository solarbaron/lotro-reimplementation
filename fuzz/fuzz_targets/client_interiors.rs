// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's interior visibility (`apps/client/src/walker/interiors.rs`, included by
//! path; `landblock.md` §4.3, §4.11–§4.12) on hostile cell lists: visibility cells, portals,
//! potentially visible sets, scene flags and the portal walk.
//!
//! Input layout: a stream of bytes read as small cell records (number, flags, group key,
//! lists A and B, neighbours), group entries and portal corners, then 16 floats of a clip
//! matrix and an eye. Nothing may panic; every visibility cell has a flag; a PVS is sorted,
//! free of duplicates and holds its cell; portals connect known visibility cells.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/walker/interiors.rs"]
mod interiors;

use interiors::{BlockVisibility, InteriorView, OUTDOORS, UNATTACHED};
use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{Cell, CellNeighbour, HeaderIndex, LandblockInfo, TSizeList};
use tessera_formats::{Did, PropertySet, TSize};
use tessera_render::glam::{Mat4, Vec3};

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
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    /// A cell number: mostly small, sometimes anything (0, 0xFFFF, unknown cells).
    fn cell(&mut self) -> u16 {
        let b = self.u8();
        if b & 0x80 != 0 {
            self.u16()
        } else {
            u16::from(b & 0x1F)
        }
    }
    fn list(&mut self) -> Vec<u16> {
        let n = self.u8() % 8;
        (0..n).map(|_| self.cell()).collect()
    }
}

fn list<T>(items: Vec<T>) -> TSizeList<T> {
    TSizeList {
        size: TSize {
            hint: 0,
            count: u32::try_from(items.len()).unwrap_or(u32::MAX),
        },
        items,
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes(data);
    let n = b.u8() % 40;
    let mut cells = Vec::new();
    for _ in 0..n {
        let index = b.cell();
        let flags = u32::from(b.u8());
        let extra = (flags & 0x20 != 0).then(|| b.cell());
        let list_a = b.list();
        let list_b = b.list();
        let k = b.u8() % 6;
        let neighbours = (0..k)
            .map(|o| CellNeighbour {
                ordinal: u32::from(o),
                cell: b.cell(),
                back_reference: b.u16(),
                reserved: [b.u16(), 0, 0],
                flag: b.u8() & 1,
            })
            .collect();
        cells.push(Cell {
            index,
            flags,
            extra,
            list_a,
            list_b,
            neighbours,
            ..Cell::default()
        });
    }
    let g = b.u8() % 6;
    let groups: Vec<HeaderIndex> = (0..g)
        .map(|_| HeaderIndex {
            index: b.cell(),
            values: b.list(),
        })
        .collect();
    let corners: Vec<[f32; 3]> = (0..b.u8() % 6)
        .map(|_| [b.f32(), b.f32(), b.f32()])
        .collect();
    let info = LandblockInfo {
        did: Did(0x8021_0101),
        flags: 5,
        size_hint: 0,
        cross_links: None,
        header_indices: (!groups.is_empty()).then(|| list(groups)),
        cells: Some(list(cells)),
        reserved_a: 0,
        entities: Vec::new(),
        links: Vec::new(),
        properties: PropertySet::default(),
        reserved_b: 0,
        weenies: TSizeList::default(),
        trailer: None,
        layout_rule: tessera_formats::landblock::LayoutRule::default(),
    };
    let v = BlockVisibility::build(&info, |c, k| {
        if (usize::from(c) + k) % 2 == 0 {
            corners.clone()
        } else {
            Vec::new()
        }
    });
    let count = v.visibility_cells();
    for p in v.portals() {
        assert!(p.from < count && p.to < count, "portal between known cells");
    }
    let m: [f32; 16] = std::array::from_fn(|_| b.f32());
    let to_clip = Mat4::from_cols_array(&m);
    let eye = Vec3::new(b.f32(), b.f32(), b.f32());
    let _ = interiors::has_outdoor_entry(&info);
    for c in info.cells() {
        let pvs = v.pvs(c.index);
        assert!(pvs.windows(2).all(|w| w[0] < w[1]), "sorted, no duplicates");
        assert!(pvs.contains(&c.index));
        let _ = v.scene_flags(c.index);
        let seen = v.visible_cells(c.index, eye, &to_clip);
        assert_eq!(seen.len(), count as usize);
        assert!(v.vcell_of(c.index).is_some_and(|x| seen[x as usize]));
    }
    let key = tessera_world::LandblockKey::new(1, 1, 1).unwrap();
    let root = info.cells().first().map_or(2, |c| c.index);
    let view = InteriorView::new(key, root, std::sync::Arc::new(v), eye, &to_clip);
    let _ = view.draws(key, OUTDOORS);
    let _ = view.draws(key, UNATTACHED);
    let _ = view.draws(key, root);
    let _ = view.visible_count();
});
