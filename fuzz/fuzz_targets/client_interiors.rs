// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's interior visibility (`apps/client/src/walker/interiors.rs`, included by
//! path; `landblock.md` §4.3, §4.11–§4.12) on hostile cell lists: visibility cells, portals,
//! potentially visible sets, scene flags and the portal walk.
//!
//! Input layout: a stream of bytes read as small cell records (number, flags, group key,
//! lists A and B, neighbours), group entries and portal corners, then 16 floats of a clip
//! matrix and an eye. Neighbour entries carry hostile `portal` fields (not their position,
//! out of range, huge) and cells hostile `index_echo` values (their own number, another
//! cell, 0xFFFF). Nothing may panic; every visibility cell has a flag; a PVS is sorted,
//! free of duplicates, holds its cell and no 0xFFFF other than its own number, and leaves
//! out a cell's `index_echo` unless it is the cell itself (`landblock.md` §4.12); portals
//! connect known visibility cells, and each polygon is asked for by the entry's `portal`
//! field (§4.11, L39), never by the entry's position.

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
    /// A neighbour entry's `portal` field: mostly its position, else small, else anything.
    fn portal(&mut self, position: u32) -> u32 {
        match self.u8() % 4 {
            0 | 1 => position,
            2 => u32::from(self.u8() % 8),
            _ => u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()]),
        }
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
        // The echo of the cell's number (flag 0x20): mostly the number itself, else any
        // cell number (hostile).
        let index_echo =
            (flags & 0x20 != 0).then(|| if b.u8() & 1 == 0 { index } else { b.cell() });
        let list_a = b.list();
        let list_b = b.list();
        let k = b.u8() % 6;
        let neighbours = (0..k)
            .map(|o| CellNeighbour {
                ordinal: b.portal(u32::from(o)),
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
            index_echo,
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
    // Every polygon is asked for by an entry's `portal` field of the asking cell.
    let asked = std::cell::RefCell::new(Vec::new());
    let v = BlockVisibility::build(&info, |c, k| {
        asked.borrow_mut().push((c, k));
        if (usize::from(c) + k) % 2 == 0 {
            corners.clone()
        } else {
            Vec::new()
        }
    });
    for (c, k) in asked.into_inner() {
        assert!(
            info.cells().iter().any(|x| x.index == c
                && x.neighbours
                    .iter()
                    .any(|n| usize::try_from(n.ordinal).unwrap_or(usize::MAX) == k)),
            "portal polygon {k} of cell {c} named by a neighbour entry's `portal` field"
        );
    }
    let count = v.visibility_cells();
    for p in v.portals() {
        assert!(p.from < count && p.to < count, "portal between known cells");
    }
    let m: [f32; 16] = std::array::from_fn(|_| b.f32());
    let to_clip = Mat4::from_cols_array(&m);
    let eye = Vec3::new(b.f32(), b.f32(), b.f32());
    let _ = interiors::has_outdoor_entry(&info);
    let _ = interiors::outdoor_flags([&v]);
    for c in info.cells() {
        for n in &c.neighbours {
            let _ = interiors::back_reference(n);
        }
    }
    for c in info.cells() {
        let pvs = v.pvs(c.index);
        assert!(pvs.windows(2).all(|w| w[0] < w[1]), "sorted, no duplicates");
        assert!(pvs.contains(&c.index));
        assert!(
            pvs.iter().all(|&x| x != 0xFFFF || x == c.index),
            "0xFFFF skipped"
        );
        // (A repeated cell number answers with the PVS of its first cell.)
        let first = info.cells().iter().find(|x| x.index == c.index);
        if let Some(echo) = c.index_echo
            && echo != c.index
            && first.is_some_and(|f| f.index_echo == c.index_echo)
        {
            assert!(!pvs.contains(&echo), "`index_echo` skipped");
        }
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
