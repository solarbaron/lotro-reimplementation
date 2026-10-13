// SPDX-License-Identifier: GPL-3.0-or-later
//! The texts the map windows ask the client for (`tessera-ui`'s [`MapTexts`]): map
//! coordinates of a point (`coordinates.md` §8, with the region's constants read from the
//! install), the `MapNoteType` display texts of the filter menu (`map-windows.md` §6.9),
//! and the filter menu's *all* and *none* items (`UI_Maps` tokens of §6.9). Every other
//! label keeps the layout's own text; without an install the defaults are Tessera's own.
//!
//! Note and marker tooltips are Tessera's wording (the label and the coordinates,
//! [`tessera_ui::game::mapmarks::marker_tooltip`]): the argument order of the `UI_Maps`
//! entry of MW15 is not stated (`map-windows.md` §6.3, OPEN).

use std::collections::BTreeMap;

use tessera_formats::StringInfo;
use tessera_formats::string_info::StringSource;
use tessera_gamedata::tooltip::TextRef;
use tessera_ui::game::map::MapPoint;
use tessera_ui::game::mapmarks::{MapLabel, MapTexts};
use tessera_world::MapConstants;

use crate::npc::NpcData;

/// The `UI_Maps` string table's label.
pub const UI_MAPS: &str = "UI_Maps";
/// `UI_Maps` token of the filter menu's *all* item (§6.9).
pub const FILTER_ALL: u32 = 0x09F0_86CC;
/// `UI_Maps` token of the filter menu's *none* item (§6.9).
pub const FILTER_NONE: u32 = 0x0EBA_8F2C;
/// The property whose enum mapper names the note types (`map-data.md` §4.2).
pub const NOTE_TYPE_PROPERTY: &str = "MapNote_Type";

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
        let d = self.data?;
        let table = d.data().table(UI_MAPS)?;
        let info = StringInfo {
            source: StringSource::Table { token, table },
            args: None,
        };
        d.data()
            .render(TextRef::Info(&info), &tessera_text::Values::new())
            .ok()
            .flatten()
            .map(|t| t.plain_text())
            .filter(|t| !t.trim().is_empty())
    }
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
    }
}
