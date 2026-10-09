// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` game map and chat data decoders (`game-panels.md` §6.2, §8.4–§8.6, §9.1;
//! `chat-window.md` §3.3, §3.5, §5.6): an arbitrary property-value tree, built from the
//! input with the placeholder PIDs of `GameVocab::default()` so that the decoders see
//! their own fields, goes through `GameMap`, `MapTree`, `ChatConfig` (and back),
//! `StyleMap`, `FeedbackSounds`, `OpacityTable`, the hide-test comparison and the tile
//! range. Nothing may panic, loop without end or allocate without bound.
//!
//! Input layout: a byte stream read as a pre-order tree; each node starts with a tag byte
//! (struct, array or a scalar kind), then its PID index, then its payload.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct, PropertyValue};
use tessera_ui::game::chat::{ChatConfig, FeedbackSounds, OpacityTable, StyleMap};
use tessera_ui::game::hidewhen::{compare, compare_with_player};
use tessera_ui::game::map::{GameMap, grid_blocks, tile_range};
use tessera_ui::game::worldmap::MapTree;
use tessera_ui::game::{GameVocab, SandboxData};

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
    nodes: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }
}

fn pid(b: &mut Bytes<'_>, pids: &[u32]) -> u32 {
    let i = usize::from(b.u8());
    pids.get(i % (pids.len() + 1)).copied().unwrap_or(0x55)
}

fn value(b: &mut Bytes<'_>, pids: &[u32], depth: usize) -> PropertyValue {
    b.nodes += 1;
    let tag = b.u8();
    if depth > 48 || b.nodes > 20_000 || b.done() {
        return PropertyValue::Int(i32::from(tag));
    }
    match tag % 10 {
        0 | 1 => {
            let n = usize::from(b.u8() % 8);
            let mut s = PropertyStruct::default();
            for _ in 0..n {
                let p = pid(b, pids);
                let v = value(b, pids, depth + 1);
                if !s.fields.iter().any(|f| f.pid == p) {
                    s.fields.push(PropertyEntry { pid: p, value: v });
                }
            }
            s.size.count = s.fields.len() as u32;
            PropertyValue::Struct(s)
        }
        2 | 3 => {
            let n = usize::from(b.u8() % 8);
            let mut a = PropertyArray::default();
            for _ in 0..n {
                let p = pid(b, pids);
                a.elements.push(PropertyEntry {
                    pid: p,
                    value: value(b, pids, depth + 1),
                });
            }
            a.count = a.elements.len() as u32;
            PropertyValue::Array(a)
        }
        4 => PropertyValue::Int(b.u32() as i32),
        5 => PropertyValue::Enum(b.u32()),
        6 => PropertyValue::Float(f32::from_bits(b.u32())),
        7 => PropertyValue::DidRef(Did(b.u32())),
        8 => PropertyValue::Bool(b.u8() & 1 == 1),
        _ => PropertyValue::Bitfield64(u64::from(b.u32()) << 32 | u64::from(b.u32())),
    }
}

fuzz_target!(|data: &[u8]| {
    let gv = GameVocab::default();
    let pids: Vec<u32> = gv.pids.entries().into_iter().map(|(_, v)| v).collect();
    let mut b = Bytes {
        data,
        pos: 0,
        nodes: 0,
    };
    let v = value(&mut b, &pids, 0);
    let p = &gv.pids;
    if let Some(m) = GameMap::from_value(&v, p) {
        for l in [0, 1, 7, u32::MAX] {
            let _ = m.image_for_layer(l);
        }
        if let Some(g) = m.georef() {
            let _ = g.to_pixel(f32::from_bits(b.u32()), f32::from_bits(b.u32()));
        }
    }
    if let Ok(t) = MapTree::from_value(&v, p) {
        let _ = t.choose(Some(b.u32()), Some(b.u32()));
        for i in 0..t.nodes.len().min(64) {
            let _ = t.georef(i);
        }
    }
    if let Ok(c) = ChatConfig::from_value(&v, p) {
        let back = ChatConfig::from_value(&c.to_value(p, 1, 2), p).expect("own output");
        assert!(back.windows.len() <= c.windows.len());
    }
    let s = StyleMap::from_values(Some(&v), Some(&v), p);
    let _ = s.style(u64::from(b.u32()));
    let _ = FeedbackSounds::from_value(&v, p).sound(u64::from(b.u32()));
    let _ = OpacityTable::from_value(&v, p).image(b.u32() as i32);
    let data = SandboxData::default();
    for op in 0..8 {
        let _ = compare_with_player(&v, op, &data);
        let _ = compare(op, b.u32() % 24, &v, &v);
    }
    let x = b.u8();
    let _ = tile_range(x);
    let _ = grid_blocks(x, b.u8());
});
