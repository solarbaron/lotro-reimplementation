// SPDX-License-Identifier: GPL-3.0-or-later
//! Map inspectors (`map-data.md`): the map kinds of game objects (§2.3) with their typed
//! views (§7–§11), map-note properties and stable masters on any object (§6, §8), and the
//! three 0x0E tables found by their UNIQUEDB labels (§2.1): named places (§3), map notes
//! (§4) and dungeon quest notes (§5).

use tessera_formats::gameplay::map::{
    DUNGEON_QUEST_NOTES_LABEL, House, HouseList, HousingHook, MAP_NOTES_LABEL, MapKind,
    MapNoteProperties, MapNotesTable, Neighbourhood, NeighbourhoodGate, NeighbourhoodTemplate,
    NoteBase, PLACES_TABLE_LABEL, PlacesTable, PositionValues, QuestNotesTable, Ripcord,
    StableMaster, TravelDestination, TravelHop, TravelMode, TravelPad, TravelRoute, TravelSegment,
    TravelVersioning, TravelWeb, classify_set,
};
use tessera_formats::{Did, Position};

use super::card::{Card, Section, dump, opt_did_text, opt_text, yes};
use super::{Built, Cx, Family, GameplayProduct, Object, product};
use crate::error::ExplorerError;

fn kind_name(k: MapKind) -> String {
    let mut out = String::new();
    for (i, c) in format!("{k:?}").chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// The map card, when the set is of a map kind (§2.3).
pub(super) fn card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let p = &cx.s.map;
    let kind = classify_set(obj.set, p)?;
    let set = obj.set;
    let name = kind_name(kind);
    let title = format!("{name} (map-data.md)");
    let decoded: Result<Option<Section>, String> = (|| {
        let e = |e: tessera_formats::FormatError| e.to_string();
        Ok(Some(match kind {
            MapKind::TravelWeb => dump(&title, &TravelWeb::read(set, p).map_err(e)?),
            MapKind::TravelDestination => {
                dump(&title, &TravelDestination::read(set, p).map_err(e)?)
            }
            MapKind::TravelRoute => dump(&title, &TravelRoute::read(set, p).map_err(e)?),
            MapKind::TravelSegment => dump(&title, &TravelSegment::read(set, p)),
            MapKind::TravelHop => dump(&title, &TravelHop::read(set, p)),
            MapKind::TravelPad => dump(&title, &TravelPad::read(set, p).map_err(e)?),
            MapKind::TravelMode => dump(&title, &TravelMode::read(set, p)),
            MapKind::TravelVersioning => dump(&title, &TravelVersioning::read(set, p)),
            MapKind::Neighbourhood => dump(&title, &Neighbourhood::read(set, p).map_err(e)?),
            MapKind::NeighbourhoodTemplate => {
                dump(&title, &NeighbourhoodTemplate::read(set, p).map_err(e)?)
            }
            MapKind::HouseList => dump(&title, &HouseList::read(set, p)),
            MapKind::House => dump(&title, &House::read(set, p).map_err(e)?),
            MapKind::NeighbourhoodGate => {
                dump(&title, &NeighbourhoodGate::read(set, p).map_err(e)?)
            }
            MapKind::HousingHook => dump(&title, &HousingHook::read(set, p)),
            MapKind::Ripcord => dump(&title, &Ripcord::read(set, p)),
            // Landmarks are map-note carriers (shown by `object_sections`); areas,
            // territories, regions and dungeons belong to `wstate.md` §11.
            _ => return Ok(None),
        }))
    })();
    let mut b = Built {
        kind: name.clone(),
        ..Built::default()
    };
    match decoded {
        Ok(Some(s)) => b.card.add(s),
        Ok(None) => {}
        Err(e) => b.notes.push(format!("{name}: {e}")),
    }
    b.groups = vec![("Map properties (map-data.md)", p.all())];
    Some(b)
}

/// Map-note properties (§6) and stable-master data (§8), on any object that has them.
pub(super) fn object_sections(cx: &Cx<'_>, obj: &Object<'_>, card: &mut Card) {
    let p = &cx.s.map;
    let l = &cx.labels;
    if obj.set.get(p.note_type.id().unwrap_or(0)).is_some() {
        let m = MapNoteProperties::read(obj.set, p);
        let mut s = Section::fields("Map note (§6)");
        s.field(p.note_type.label(), l.opt_bits(p.note_type, m.note_type));
        s.field(p.note_level.label(), l.opt_bits(p.note_level, m.level));
        if let Some(t) = &m.text {
            s.field(p.note_text.label(), l.text(t));
        }
        s.did(p.note_icon.label(), m.icon);
        s.opt("min range", m.min_range);
        s.opt("max range", m.max_range);
        s.opt("discoverable", m.discoverable);
        s.opt("enabled", m.enabled.map(yes));
        s.opt("linkable", m.linkable.map(yes));
        card.add(s);
    }
    if let Some(sm) = StableMaster::read(obj.set, p) {
        card.add(dump("Stable master (§8)", &sm));
    }
}

fn position_text(p: &Position) -> String {
    let v = PositionValues::of(p);
    format!(
        "region {} block {:02X}{:02X} cell {:#06x} ({:.2}, {:.2}, {:.2})",
        v.region, v.landblock.0, v.landblock.1, v.cell, v.offset[0], v.offset[1], v.offset[2]
    )
}

fn zone_links(base: &NoteBase) -> Vec<u32> {
    [base.area, base.dungeon, base.entity]
        .into_iter()
        .filter(|d| !d.is_null())
        .map(|d| d.0)
        .collect()
}

/// The 0x0E table `did` names in the UNIQUEDB map, if any.
fn uniquedb_label(cx: &Cx<'_>, did: u32) -> Option<String> {
    let map = cx.s.uniquedb.as_ref()?;
    map.iter()
        .find(|e| e.value.0 == did)
        .map(|e| e.label.to_string())
}

/// The card of a 0x0E map table: places, map notes or dungeon quest notes.
pub(super) fn table_product(cx: &Cx<'_>, did: u32) -> Result<GameplayProduct, ExplorerError> {
    let label = uniquedb_label(cx, did);
    let schema = cx.s.registry.dictionary();
    let not_map = || {
        ExplorerError::NotApplicable(format!(
            "{did:#010x} is not one of the map tables ({PLACES_TABLE_LABEL}, {MAP_NOTES_LABEL}, \
             {DUNGEON_QUEST_NOTES_LABEL}){}",
            label
                .as_deref()
                .map(|l| format!("; its UNIQUEDB label is {l}"))
                .unwrap_or_default()
        ))
    };
    let Some(which) = label.clone() else {
        return Err(not_map());
    };
    let raw = crate::decoders::read_raw(cx.ctx, did, None)?;
    let l = &cx.labels;
    let mut p = product(did, Family::Map, which.clone());
    match which.as_str() {
        PLACES_TABLE_LABEL => {
            let t = PlacesTable::parse_expecting(&raw.bytes, Did(did))?;
            p.kind = "named places (§3)".into();
            let mut s = Section::new("Places (§3)", &["key", "position", "zone"]);
            for pl in &t.places {
                s.push(
                    vec![
                        pl.name.to_string(),
                        position_text(&pl.position),
                        opt_did_text(pl.zone()),
                    ],
                    pl.zone().map(|d| d.0).into_iter().collect(),
                );
            }
            p.sections.push(s);
        }
        MAP_NOTES_LABEL => {
            let t = MapNotesTable::parse_expecting(&raw.bytes, Did(did), schema)?;
            p.kind = "map notes (§4)".into();
            let mp = &cx.s.map;
            let mut s = Section::new(
                "Map notes (§4)",
                &[
                    "text", "type", "position", "area", "entity", "range", "link",
                ],
            );
            for n in &t.notes {
                let mut links = zone_links(&n.base);
                links.extend(Some(n.icon).filter(|d| !d.is_null()).map(|d| d.0));
                if let Some(k) = &n.link {
                    links.extend(Some(k.target).filter(|d| !d.is_null()).map(|d| d.0));
                }
                s.push(
                    vec![
                        l.text(&n.text),
                        l.bits(mp.note_type, n.note_type),
                        position_text(&n.base.position),
                        opt_did_text(Some(n.base.area)),
                        opt_did_text(Some(n.base.entity)),
                        format!("{} … {}", n.min_range, n.max_range),
                        n.link
                            .as_ref()
                            .map_or_else(String::new, |k| opt_did_text(Some(k.target))),
                    ],
                    links,
                );
            }
            p.sections.push(s);
        }
        DUNGEON_QUEST_NOTES_LABEL => {
            let t = QuestNotesTable::parse_expecting(&raw.bytes, Did(did), schema)?;
            p.kind = "dungeon quest notes (§5)".into();
            let mut s = Section::new(
                "Dungeon quest notes (§5)",
                &["position", "area", "dungeon", "entity", "quest info"],
            );
            for n in &t.notes {
                s.push(
                    vec![
                        position_text(&n.base.position),
                        opt_did_text(Some(n.base.area)),
                        opt_did_text(Some(n.base.dungeon)),
                        opt_did_text(Some(n.base.entity)),
                        format!("{} properties", n.quest_info.len()),
                    ],
                    zone_links(&n.base),
                );
            }
            p.sections.push(s);
        }
        _ => return Err(not_map()),
    }
    let mut info = Section::fields("Table");
    info.field("UNIQUEDB label", which);
    info.field("rows", opt_text(p.sections.first().map(|s| s.rows.len())));
    p.sections.insert(0, info);
    Ok(p)
}
