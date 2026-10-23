// SPDX-License-Identifier: GPL-3.0-or-later
//! The texts the map windows ask the client for (`tessera-ui`'s [`MapTexts`]): map
//! coordinates of a point (`coordinates.md` §8, with the region's constants read from the
//! install), the `MapNoteType` display texts of the filter menu (`map-windows.md` §6.9),
//! and the filter menu's *all* and *none* items (`UI_Maps` tokens of §6.9). Every other
//! label keeps the layout's own text; without an install the defaults are Tessera's own.
//!
//! A table note's world-map tooltip is the install's `UI_Maps` entry of MW15 and
//! `CoordinateDisplay`'s text the `WeenieClientMsgs` entry of MW19, each with the latitude
//! and longitude texts of `coordinates.md` §8.3 in the argument order of `map-windows.md`
//! §6.3 revision 1 (MW30); when an entry is missing (or renders empty) Tessera's wording is
//! used (the label and the coordinates, [`tessera_ui::game::mapmarks::marker_tooltip`]).
//! Feed markers keep Tessera's wording (§6.7 item 6). For the stable-master note types
//! MW15's first argument is the composite entry 0x0DDC06F5 only when a text of the note's
//! entity is known ([`InstallMapTexts::note_tooltip_with`]); the client half has no lookup
//! for it, so those notes take the plain note text (Tessera reading, implementer OPEN).
//!
//! The two coordinate texts are the English form of `coordinates.md` §8.3 ("4.0S",
//! "66.6W": magnitude with one decimal, then the letter), which that section states is
//! exact for English; the per-coordinate `Position` template is not rendered.

use std::collections::BTreeMap;

use tessera_formats::StringInfo;
use tessera_formats::string_info::StringSource;
use tessera_gamedata::tooltip::TextRef;
use tessera_text::{Value, Values};
use tessera_ui::game::map::MapPoint;
use tessera_ui::game::mapmarks::{MapLabel, MapTexts};
use tessera_world::{MapConstants, MapCoords};

use crate::npc::NpcData;

/// The `UI_Maps` string table's label.
pub const UI_MAPS: &str = "UI_Maps";
/// `UI_Maps` token of the filter menu's *all* item (§6.9).
pub const FILTER_ALL: u32 = 0x09F0_86CC;
/// `UI_Maps` token of the filter menu's *none* item (§6.9).
pub const FILTER_NONE: u32 = 0x0EBA_8F2C;
/// The property whose enum mapper names the note types (`map-data.md` §4.2).
pub const NOTE_TYPE_PROPERTY: &str = "MapNote_Type";
/// `UI_Maps` token of a world-map note's tooltip (MW15).
pub const NOTE_TOOLTIP: u32 = 0x0B0C_5A53;
/// MW15's argument: the note's text.
pub const NOTE_TOOLTIP_TEXT: u32 = 0x0CE9_2D54;
/// MW15's argument taking the latitude text (§6.3 revision 1, Tessera rule).
pub const NOTE_TOOLTIP_LATITUDE: u32 = 0x050F_3B05;
/// MW15's argument taking the longitude text (§6.3 revision 1, Tessera rule).
pub const NOTE_TOOLTIP_LONGITUDE: u32 = 0x050F_4F05;
/// The note-type bits of the stable-master types (MW15), whose first argument is a
/// composite entry built with a text of the note's entity.
pub const STABLE_MASTER_BITS: [u32; 4] = [21, 22, 23, 50];
/// `UI_Maps` token of the stable-master types' composite first argument (MW15).
pub const STABLE_MASTER_TEXT: u32 = 0x0DDC_06F5;
/// The composite's argument taking the note's text (MW15).
pub const STABLE_MASTER_NOTE: u32 = 0x04D9_4125;
/// The composite's argument taking the text of the note's entity (MW15).
pub const STABLE_MASTER_ENTITY: u32 = 0x0932_5495;
/// The `WeenieClientMsgs` string table's label.
pub const WEENIE_CLIENT_MSGS: &str = "WeenieClientMsgs";
/// `WeenieClientMsgs` token of `CoordinateDisplay`'s text (MW19, the coordinate pair).
pub const COORDINATE_PAIR: u32 = 0x053B_8BB3;
/// MW19's argument taking the latitude text (`ns`, MW30).
pub const COORDINATE_LATITUDE: u32 = 0x0753;
/// MW19's argument taking the longitude text (`ew`, MW30).
pub const COORDINATE_LONGITUDE: u32 = 0x06C7;

/// [`MapTexts`] over the install (module docs).
#[derive(Clone, Copy)]
pub struct InstallMapTexts<'a> {
    constants: Option<&'a BTreeMap<u8, MapConstants>>,
    data: Option<&'a NpcData>,
}

impl std::fmt::Debug for InstallMapTexts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallMapTexts").finish_non_exhaustive()
    }
}

impl<'a> InstallMapTexts<'a> {
    /// The texts of `constants` (each region's map constants) and `data` (the string
    /// tables and enum mappers); Tessera's defaults for what is absent.
    #[must_use]
    pub fn new(
        constants: Option<&'a BTreeMap<u8, MapConstants>>,
        data: Option<&'a NpcData>,
    ) -> Self {
        Self { constants, data }
    }

    fn ui_maps(&self, token: u32) -> Option<String> {
        self.entry(UI_MAPS, token, &Values::new())
    }

    /// Entry `token` of the string table labelled `table`, rendered with `values`; `None`
    /// when it is missing or renders empty.
    fn entry(&self, table: &str, token: u32, values: &Values) -> Option<String> {
        let d = self.data?;
        let table = d.data().table(table)?;
        let info = StringInfo {
            source: StringSource::Table { token, table },
            args: None,
        };
        d.data()
            .render(TextRef::Info(&info), values)
            .ok()
            .flatten()
            .map(|t| t.plain_text())
            .filter(|t| !t.trim().is_empty())
    }

    /// MW15's tooltip of a note of type `note_type` with the text `text` at `point`, with
    /// `entity_text` the text the client obtains for the note's entity (MW15's second text
    /// of the stable-master composite). For a stable-master type with an entity text the
    /// first argument is the composite entry 0x0DDC06F5 (the note's text in 0x04D94125, the
    /// entity's in 0x09325495); for every other note, and for a stable-master note whose
    /// entity lookup gave no text (always so in the client half, which has no such lookup),
    /// it is the note's text (Tessera reading of MW15's "when that lookup succeeds";
    /// implementer OPEN). `None` when the entry cannot be rendered.
    #[must_use]
    pub fn note_tooltip_with(
        &self,
        text: &str,
        note_type: u64,
        entity_text: Option<&str>,
        point: &MapPoint,
    ) -> Option<String> {
        let (lat, lon) = self.coordinate_texts(point)?;
        let first = match entity_text {
            Some(e) if is_stable_master(note_type) => {
                let mut c = Values::new();
                c.insert(STABLE_MASTER_NOTE, Value::Text(text.to_owned()));
                c.insert(STABLE_MASTER_ENTITY, Value::Text(e.to_owned()));
                self.entry(UI_MAPS, STABLE_MASTER_TEXT, &c)
                    .unwrap_or_else(|| text.to_owned())
            }
            _ => {
                if is_stable_master(note_type) {
                    tracing::trace!("stable-master note without an entity text: plain text");
                }
                text.to_owned()
            }
        };
        let mut v = Values::new();
        v.insert(NOTE_TOOLTIP_TEXT, Value::Text(first));
        v.insert(NOTE_TOOLTIP_LATITUDE, Value::Text(lat));
        v.insert(NOTE_TOOLTIP_LONGITUDE, Value::Text(lon));
        self.entry(UI_MAPS, NOTE_TOOLTIP, &v)
    }

    /// The latitude and longitude texts of `point` (`coordinates.md` §8.3).
    fn coordinate_texts(&self, point: &MapPoint) -> Option<(String, String)> {
        coordinate_pair(self.constants?, point)
    }
}

/// The latitude and longitude texts of `point` ("12.3S", "45.6W", `coordinates.md` §8.3)
/// with the region's constants; `None` without them.
#[must_use]
pub fn coordinate_pair(
    constants: &BTreeMap<u8, MapConstants>,
    point: &MapPoint,
) -> Option<(String, String)> {
    let c = constants.get(&point.region)?;
    let (x, y) = point.global;
    if !(x.is_finite() && y.is_finite()) {
        return None;
    }
    let m = c.to_map([f64::from(x), f64::from(y)]);
    let one = |latitude: f64, longitude: f64| MapCoords {
        latitude,
        longitude,
    };
    // `MapCoords`' display writes "latitude, longitude"; each half is one coordinate text.
    let lat = one(m.latitude, 0.0).to_string();
    let lon = one(0.0, m.longitude).to_string();
    Some((
        lat.split(", ").next()?.to_owned(),
        lon.split(", ").nth(1)?.to_owned(),
    ))
}

/// Whether a note of type `note_type` is of a stable-master type (MW15).
#[must_use]
pub fn is_stable_master(note_type: u64) -> bool {
    STABLE_MASTER_BITS
        .iter()
        .any(|b| note_type.checked_shr(*b).is_some_and(|v| v & 1 == 1))
}

/// The map coordinates text of `point` ("12.3S, 45.6W", `coordinates.md` §8) with the
/// region's constants; `None` without them.
#[must_use]
pub fn coordinates_text(
    constants: &BTreeMap<u8, MapConstants>,
    point: &MapPoint,
) -> Option<String> {
    let c = constants.get(&point.region)?;
    let (x, y) = point.global;
    if !(x.is_finite() && y.is_finite()) {
        return None;
    }
    Some(c.to_map([f64::from(x), f64::from(y)]).to_string())
}

impl MapTexts for InstallMapTexts<'_> {
    fn coordinates(&self, point: &MapPoint) -> Option<String> {
        coordinates_text(self.constants?, point)
    }

    /// MW19 with MW30's order (0x753 latitude, 0x6C7 longitude); Tessera's "lat, lon" when
    /// the entry is missing.
    fn coordinate_display(&self, point: &MapPoint) -> Option<String> {
        let (lat, lon) = self.coordinate_texts(point)?;
        let mut v = Values::new();
        v.insert(COORDINATE_LATITUDE, Value::Text(lat));
        v.insert(COORDINATE_LONGITUDE, Value::Text(lon));
        self.entry(WEENIE_CLIENT_MSGS, COORDINATE_PAIR, &v)
            .or_else(|| self.coordinates(point))
    }

    /// MW15 with §6.3 revision 1's order (0x050F3B05 latitude, 0x050F4F05 longitude),
    /// without an entity text ([`InstallMapTexts::note_tooltip_with`]: the client half has
    /// no lookup for the text of a note's entity).
    fn note_tooltip(&self, text: &str, note_type: u64, point: &MapPoint) -> Option<String> {
        self.note_tooltip_with(text, note_type, None, point)
    }

    fn note_type_label(&self, key: u32) -> String {
        self.data
            .and_then(|d| {
                let pid = d.data().pid(NOTE_TYPE_PROPERTY)?;
                d.data().enum_text(pid, key)
            })
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("Type {key}"))
    }

    fn label(&self, label: MapLabel) -> Option<String> {
        match label {
            MapLabel::FilterAll => self.ui_maps(FILTER_ALL),
            MapLabel::FilterNone => self.ui_maps(FILTER_NONE),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_need_the_region() {
        let mut c = BTreeMap::new();
        c.insert(
            1,
            MapConstants::new([29_360.0, 24_880.0], 0.005).expect("constants"),
        );
        let p = MapPoint {
            global: (29_360.0, 24_880.0),
            region: 1,
            instance: 0,
        };
        assert!(coordinates_text(&c, &p).is_some());
        let other = MapPoint { region: 2, ..p };
        assert_eq!(coordinates_text(&c, &other), None);
        let t = InstallMapTexts::new(Some(&c), None);
        assert_eq!(t.note_type_label(48), "Type 48");
        assert_eq!(t.label(MapLabel::FilterAll), None);
        // Without the string tables: Tessera's wording for `CoordinateDisplay`, and no
        // install tooltip (the caller then uses Tessera's).
        assert_eq!(t.coordinate_display(&p), coordinates_text(&c, &p));
        assert_eq!(t.note_tooltip("Camp", 0, &p), None);
    }

    #[test]
    fn the_pair_splits_into_latitude_and_longitude() {
        let mut c = BTreeMap::new();
        c.insert(1, MapConstants::new([0.0, 0.0], 0.005).expect("constants"));
        let p = MapPoint {
            global: (-13_320.0, -810.0),
            region: 1,
            instance: 0,
        };
        let (lat, lon) = coordinate_pair(&c, &p).expect("pair");
        assert_eq!(
            format!("{lat}, {lon}"),
            coordinates_text(&c, &p).expect("text")
        );
        assert!(lat.ends_with('S'), "{lat}");
        assert!(lon.ends_with('W'), "{lon}");
        let other = MapPoint { region: 2, ..p };
        assert_eq!(coordinate_pair(&c, &other), None);
    }

    #[test]
    fn stable_master_types_by_bit() {
        assert!(is_stable_master(1 << 21));
        assert!(is_stable_master(1 << 22));
        assert!(is_stable_master(1 << 23));
        assert!(is_stable_master(1 << 50));
        assert!(!is_stable_master(1 << 20 | 1 << 24 | 1 << 49 | 1 << 51));
        assert!(!is_stable_master(0));
    }
}
