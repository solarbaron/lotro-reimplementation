// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` P5 map and travel widgets (`map-windows.md`, `travel-windows.md`): an
//! arbitrary property-value tree, built from the input with the placeholder PIDs of
//! `GameVocab::default()` and `P5Vocab::default()`, is decoded as a map tree with its node
//! options (`MapTree::from_value`, `MapTree::read_options`); the fallback map window then
//! shows nodes, dungeon maps and plain elements, takes notes and feed markers at fuzzed
//! positions, goes up, follows the pointer and builds its filter menu; the filter and
//! radar commands, the travel model (grouping, search, selection, go) and the finder tree
//! and model run on fuzzed rows. Nothing may panic, loop without end or allocate without
//! bound.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct, PropertyValue};
use tessera_ui::game::finder::{FinderEntry, FinderModel, FinderView, kind_of};
use tessera_ui::game::map::{GameMap, MapPoint};
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapmarks::{
    DefaultMapTexts, FeedMarker, FilterMenuModel, IconTable, MapNote, RadarFilterCommand,
    radar_filter_command, read_filter,
};
use tessera_ui::game::mapwindow::{MapWindowView, PlayerMap};
use tessera_ui::game::travel::{TravelModel, TravelRow, TravelView};
use tessera_ui::game::worldmap::MapTree;
use tessera_ui::game::{GameVocab, NpcVocab, P5Vocab};
use tessera_ui::{MemoryEnv, Ui, Vocab};

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

    fn u64(&mut self) -> u64 {
        u64::from(self.u32()) << 32 | u64::from(self.u32())
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
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
    if depth > 24 || b.nodes > 4000 || b.done() {
        return PropertyValue::Int(i32::from(tag));
    }
    match tag % 12 {
        0 | 1 => {
            let n = usize::from(b.u8() % 10);
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
            let n = usize::from(b.u8() % 6);
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
        6 => PropertyValue::Float(b.f32()),
        7 => PropertyValue::DidRef(Did(b.u32())),
        8 => PropertyValue::Bool(b.u8() & 1 == 1),
        9 => PropertyValue::PropertyId(b.u32()),
        10 => PropertyValue::Int64(b.u64() as i64),
        _ => PropertyValue::Bitfield64(b.u64()),
    }
}

fn point(b: &mut Bytes<'_>) -> MapPoint {
    MapPoint {
        global: (b.f32(), b.f32()),
        region: b.u8() % 4,
        instance: u32::from(b.u8() % 2),
    }
}

fn text(b: &mut Bytes<'_>) -> String {
    let n = usize::from(b.u8() % 12);
    (0..n).map(|_| char::from(b'a' + b.u8() % 26)).collect()
}

fuzz_target!(|data: &[u8]| {
    let (gv, pv, nv) = (
        GameVocab::default(),
        P5Vocab::default(),
        NpcVocab::default(),
    );
    let mut pids: Vec<u32> = gv.pids.entries().into_iter().map(|(_, v)| v).collect();
    pids.extend(pv.pids.entries().into_iter().map(|(_, v)| v));
    let mut b = Bytes {
        data,
        pos: 0,
        nodes: 0,
    };
    let v = value(&mut b, &pids, 0);
    let mut tree = MapTree::from_value(&v, &gv.pids).unwrap_or_default();
    tree.read_options(&v, &gv.pids, &pv.pids);
    let n = tree.nodes.len();
    for i in 0..n.min(32) {
        let o = tree.options(i);
        let _ = FilterMenuModel::build(
            o.display_mask(),
            read_filter(b.u64()),
            &DefaultMapTexts,
            &IconTable::default(),
        );
        let _ = tree.is_within(i, b.u8().into());
    }
    let _ = tree.player_node(Some(b.u32()));
    let mut ui = Ui::new(
        Box::new(MemoryEnv::default()),
        Vocab::default(),
        (1024, 768),
    );
    let t = DefaultMapTexts;
    if let Ok(mut w) =
        MapWindowView::open_fallback(&mut ui, &gv, &pv, &nv, tree, &P5Art::default(), &t)
    {
        w.set_filter_value(pv.pids.gop_map_filter, b.u64());
        for _ in 0..usize::from(b.u8() % 12) {
            match b.u8() % 9 {
                0 => {
                    let _ = w.show_node(&mut ui, usize::from(b.u8()), &t);
                }
                1 => {
                    let m = GameMap {
                        image: b.u32(),
                        block_x: Some(b.u32() as i32),
                        block_y: Some(b.u32() as i32),
                        pixel_x: b.u32() as i32,
                        pixel_y: b.u32() as i32,
                        scale: Some(b.f32()),
                        ..GameMap::default()
                    };
                    let _ = w.show_dungeon(&mut ui, m, &t);
                }
                2 => {
                    let notes = (0..usize::from(b.u8() % 8))
                        .map(|i| MapNote {
                            id: i as u64,
                            note_type: b.u64(),
                            point: point(&mut b),
                            area: Some(b.u32()),
                            ..MapNote::default()
                        })
                        .collect();
                    w.set_notes(&mut ui, notes, &t);
                }
                3 => {
                    let m = FeedMarker {
                        id: b.u32(),
                        point: point(&mut b),
                        label: text(&mut b),
                        quest: b.u8() & 1 == 1,
                        note_type: Some(b.u32() % 70),
                        ..FeedMarker::default()
                    };
                    w.add_marker(&mut ui, m, &t);
                }
                4 => {
                    let id = b.u32();
                    w.remove_marker(&mut ui, id, &t);
                }
                5 => {
                    let p = PlayerMap {
                        point: point(&mut b),
                        heading: b.f32(),
                        valid: b.u8() & 1 == 1,
                        area: Some(b.u32()),
                        dungeon: None,
                    };
                    let _ = w.show_player_map(&mut ui, &p, &t);
                }
                6 => {
                    let _ = w.go_up(&mut ui, &t);
                }
                7 => {
                    let (x, y) = (b.u32() as i32 % 2000, b.u32() as i32 % 2000);
                    w.pointer_moved(&mut ui, x, y, &t);
                }
                _ => {
                    let _ = w.show_element(&mut ui, b.u32());
                }
            }
        }
        w.detach(&mut ui);
    }
    // Filters.
    let mut m = FilterMenuModel::build(b.u64(), b.u64(), &DefaultMapTexts, &IconTable::default());
    let f = m.select(usize::from(b.u8()), b.u64());
    for c in [
        RadarFilterCommand::OnlyOffscreen,
        RadarFilterCommand::Reset,
        RadarFilterCommand::Toggle(b.u32()),
    ] {
        let _ = radar_filter_command(f, b.u64(), c);
    }
    // The travel model.
    let rows: Vec<TravelRow> = (0..usize::from(b.u8() % 16))
        .map(|i| TravelRow {
            entry: i as u16,
            name: text(&mut b),
            territory: text(&mut b),
            area: text(&mut b),
            coin: b.u64(),
            min_level: b.u32() as u16,
            swift: b.u8() & 1 == 1,
            discovered: b.u8() & 1 == 1,
            met: b.u8() & 1 == 1,
            usable: b.u8() & 1 == 1,
            ..TravelRow::default()
        })
        .collect();
    let mut tm = TravelModel::default();
    tm.show(&TravelView {
        window: 1,
        title: String::new(),
        player_level: b.u32(),
        rows,
    });
    tm.filter = text(&mut b);
    let _ = tm.groups("swift");
    tm.selected = Some(usize::from(b.u8()));
    let _ = (tm.selection(), tm.go_button(), tm.go(1));
    // The finder.
    let entries: Vec<FinderEntry> = (0..usize::from(b.u8() % 16))
        .map(|i| FinderEntry {
            entry: i as u16,
            name: text(&mut b),
            kind: kind_of(b.u32()),
            category: b.u32() % 4,
            group: b
                .u8()
                .is_multiple_of(3)
                .then(|| (b.u32() % 3, text(&mut b))),
            sizes: (0..usize::from(b.u8() % 4))
                .map(|_| (b.u32() % 5, String::new()))
                .collect(),
            tiers: (0..usize::from(b.u8() % 4))
                .map(|_| (b.u32() % 5, String::new()))
                .collect(),
            level_range: Some((b.u32() as u16, b.u32() as u16)),
            level: (b.u8() & 1 == 1).then(|| b.u32() as u16),
            size: Some(b.u32() % 5),
            ..FinderEntry::default()
        })
        .collect();
    let mut fm = FinderModel::default();
    fm.show(&FinderView {
        window: 2,
        entries,
        featured: Some(b.u32() as u16),
        player_level: b.u32() as u16,
    });
    fm.select(Some(usize::from(b.u8())));
    fm.size = usize::from(b.u8());
    fm.tier = usize::from(b.u8());
    let _ = (
        fm.resolved(),
        fm.level_field(),
        fm.join(2),
        fm.featured_ghosted(),
    );
});
