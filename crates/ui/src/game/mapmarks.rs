// SPDX-License-Identifier: GPL-3.0-or-later
//! Overlays on the world map and the minimap (`map-windows.md` §6; `map-data.md` §4.8;
//! `game-panels.md` §8.8): note types and masks, the note-type filters and their menu,
//! the icon tables, table and run-time notes, scenario-feed markers, their placement by the
//! region rule of §6.1, their order, visibility and tooltips, and the minimap's note
//! blips.
//!
//! The client selects the notes (the map-note manager's queries, discovery, content layer
//! and world-event states, `map-data.md` §4.8.1–§4.8.2, §4.8.6) and renders their texts;
//! this module applies what the views do with them: the creation test by the display mask
//! and the area's node (§6.3 item 1), the visibility test by the filter value (§6.3 item 3),
//! the images of the view's icon table (`map-data.md` §4.8.5), placement and z (§6.2), and
//! the minimap's radius rule (§6.8). Feed markers follow the Tessera rules of §6.7.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::property::PropertyValue;

use crate::env::{ImageRef, ImageState};
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};
use crate::widgets::WidgetClass;

use super::map::{Georef, MapPoint, pixel_in_region};
use super::npc::kit::{FNode, cut, instantiate};
use super::p5vocab::P5Vocab;
use super::radar::{Radar, RadarInput};
use super::vocab::GameVocab;
use super::worldmap::MapTree;
use super::{move_to_front, show};

/// The mask a map filter option is ANDed with when it is read (`map-windows.md` MW12,
/// §6.9; BIN).
pub const FILTER_READ_MASK: u64 = 0x9B7C_7FFF_FEFF_0002;

/// Bits of a display mask that get no filter-menu item (`map-windows.md` MW13): 0 `Link`,
/// 1 `Quest`, 38 `NoIcon`, 45 `MapNoteOffscreen`, 46 `FellowOffscreen`, 48 `Admin`.
pub const UNLISTED_FILTER_BITS: [u32; 6] = [0, 1, 38, 45, 46, 48];

/// The note type of a link row (bit 0, `map-data.md` §4.3).
pub const LINK_TYPE: u64 = 1;

/// Bit of the `Quest` note type (`map-windows.md` §6.7 item 2).
pub const QUEST_BIT: u32 = 1;

/// Bit of the `Waypoint` note type (§6.7 item 2: plain feed markers).
pub const WAYPOINT_BIT: u32 = 47;

/// Bit of `MapNoteOffscreen` (the radar's ranged notes take its entry, `map-data.md`
/// §4.8.3 step 4; the radar filter command "only bit 45", §6.8).
pub const OFFSCREEN_BIT: u32 = 45;

/// Type value of `Fellow` notes (`map-windows.md` MW16): drawn 1 z-level in front.
pub const FELLOW_TYPE: u64 = 0x4_0000_0000;

/// Type value of `FellowshipLeader` notes (MW16): drawn 2 z-levels in front.
pub const LEADER_TYPE: u64 = 0x800_0000;

/// z-level of `MapNoteTemplate` in this install's layout (§3.3); the fallback note's z.
pub const NOTE_Z: i32 = -1;

/// Size of `MapNoteTemplate` (§3.3); the fallback note's size.
pub const NOTE_SIZE: (i32, i32) = (30, 30);

/// Most notes (table plus run-time) kept for one shown map (Tessera bound).
pub const MAX_NOTES: usize = 16_384;

/// Most feed markers kept (Tessera bound; the feed's run-time cap is lower).
pub const MAX_MARKERS: usize = 4096;

/// Most icon-table entries kept (Tessera bound; the tables have 46 and 47).
pub const MAX_ICON_ENTRIES: usize = 4096;

/// The feed-marker id Tessera's place lookup uses for its marker (`map-windows.md` §7.3:
/// "id outside the feed's range"; the feed's ids are below it).
pub const LOOKUP_MARKER_ID: u32 = u32::MAX;

/// The type value of a note-type key (`map-data.md` §6.3: key *k* is bit *k* − 1); `None`
/// outside 1 … 64.
#[must_use]
pub fn type_of_key(key: u32) -> Option<u64> {
    (1..=64).contains(&key).then(|| 1u64 << (key - 1))
}

/// The type value with only bit `bit` set (`None` past bit 63).
#[must_use]
pub fn type_of_bit(bit: u32) -> Option<u64> {
    (bit < 64).then(|| 1u64 << bit)
}

/// A map filter option's value as the window uses it (§6.9 "Reading"): ANDed with
/// [`FILTER_READ_MASK`].
#[must_use]
pub fn read_filter(option: u64) -> u64 {
    option & FILTER_READ_MASK
}

/// Whether a table note is **created** on a map (`map-windows.md` §6.3 item 1, MW11): any
/// type on a dungeon map, else when its type shares a bit with the display mask.
#[must_use]
pub fn created_on_map(note_type: u64, display_mask: u64, dungeon: bool) -> bool {
    dungeon || note_type & display_mask != 0
}

/// Whether a note is **visible** under a filter value (§6.3 item 3, MW12): its type shares
/// a bit with the filter; for a link row (type 1) the test is filter bit 1.
#[must_use]
pub fn visible_under(note_type: u64, filter: u64) -> bool {
    if note_type == LINK_TYPE {
        filter & 2 != 0
    } else {
        note_type & filter != 0
    }
}

/// The z-level of a note (§6.2 row 4, §6.3 item 2, MW16): the template's z plus the icon
/// entry's relative z, and 2 (`FellowshipLeader`) or 1 (`Fellow`) levels in front (lower z).
#[must_use]
pub fn note_z(template_z: i32, note_type: u64, relative_z: i32) -> i32 {
    let z = template_z.saturating_add(relative_z);
    match note_type {
        LEADER_TYPE => z.saturating_sub(2),
        FELLOW_TYPE => z.saturating_sub(1),
        _ => z,
    }
}

/// Whether a note is a **ranged** note (`map-data.md` §4.7 step 1).
#[must_use]
pub fn is_ranged(min_range: f32, max_range: f32) -> bool {
    min_range.abs() > 0.0002 && max_range.abs() > 0.0002
}

/// Whether a ranged note at squared distance `d2` shows (`map-data.md` §4.7 step 3, f32):
/// hidden when *d*² > max² or *d*² < min².
#[must_use]
pub fn ranged_visible(d2: f32, min_range: f32, max_range: f32) -> bool {
    !(d2 > max_range * max_range || d2 < min_range * min_range)
}

/// The radar's world radius (`map-windows.md` §6.8, MW22): `UI_Radar_RadarRadius` (map
/// pixels) ÷ the georeference scale; `None` for a scale of 0.
#[must_use]
pub fn radar_world_radius(radius_pixels: i32, scale: f32) -> Option<f32> {
    #[allow(clippy::cast_precision_loss)]
    let r = radius_pixels as f32 / scale;
    (scale != 0.0 && r.is_finite()).then_some(r)
}

/// The three images of a note state set (`map-data.md` §4.8.5): normal, selected,
/// highlight.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoteIcons {
    /// The normal image.
    pub normal: Option<ImageRef>,
    /// The selected image.
    pub selected: Option<ImageRef>,
    /// The highlight (mouse-over) image.
    pub highlight: Option<ImageRef>,
}

impl NoteIcons {
    /// Icons with one image in every state (a marker's own `icon`, §6.7 item 2).
    #[must_use]
    pub fn single(img: ImageRef) -> Self {
        Self {
            normal: Some(img.clone()),
            selected: Some(img.clone()),
            highlight: Some(img),
        }
    }
}

/// One entry of a view's icon table (`map-data.md` §4.8.5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IconEntry {
    /// `UI_Map_MapNoteOnscreenImageStruct`.
    pub onscreen: NoteIcons,
    /// `UI_Map_MapNoteOffscreenImageStruct`, when present.
    pub offscreen: Option<NoteIcons>,
    /// `UI_Map_MapNoteMenuImage` (the filter menu's item icon, §6.9).
    pub menu: Option<ImageRef>,
    /// `UI_Map_MapNoteRelativeZLevel` (0 when absent).
    pub relative_z: i32,
}

/// A view's icon table, keyed by the whole 64-bit type value (`map-data.md` §4.8.5: the
/// first entry wins for a repeated value). The client fills it from the install's table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IconTable {
    entries: BTreeMap<u64, IconEntry>,
}

impl IconTable {
    /// Adds an entry; a repeated type value keeps the first, and past
    /// [`MAX_ICON_ENTRIES`] entries nothing is added. Returns whether it was added.
    pub fn insert(&mut self, type_value: u64, entry: IconEntry) -> bool {
        if self.entries.contains_key(&type_value) || self.entries.len() >= MAX_ICON_ENTRIES {
            return false;
        }
        self.entries.insert(type_value, entry);
        true
    }

    /// The entry for a type value (no entry: the note shows no icon).
    #[must_use]
    pub fn get(&self, type_value: u64) -> Option<&IconEntry> {
        self.entries.get(&type_value)
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A table note (or a run-time note, §6.4) as the client selected and rendered it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapNote {
    /// The client's key of the note (unique among the notes given together).
    pub id: u64,
    /// The note's `type` (64-bit).
    pub note_type: u64,
    /// Where it is.
    pub point: MapPoint,
    /// The note's `area` (table notes; `None`: registered to the root, MD43).
    pub area: Option<u32>,
    /// The world map's tooltip (MW15, rendered by the client; empty: none).
    pub tooltip: String,
    /// The note's text (the radar tooltip's first argument, MW22).
    pub text: String,
    /// The world-event state's images replacing the icon table's on-screen images
    /// (`map-data.md` §4.8.6 item 2), when one applies.
    pub icons: Option<NoteIcons>,
    /// The world-event state's `MapNote_OffScreenIcon`, when one applies.
    pub offscreen: Option<ImageRef>,
    /// `min_range`, `max_range` (`map-data.md` §4.7), when the note is ranged.
    pub range: Option<(f32, f32)>,
    /// The note's height (for the ranged notes' three-dimensional distance).
    pub height: f32,
    /// A run-time note (§6.4): not subject to the display-mask and area tests.
    pub runtime: bool,
}

/// A scenario-feed marker (`MarkerAdded`, `scenario-feed.md` §6.5; `map-windows.md` §6.7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeedMarker {
    /// The feed's marker id ([`LOOKUP_MARKER_ID`] for Tessera's place lookup).
    pub id: u32,
    /// The location.
    pub point: MapPoint,
    /// The label (the tooltip's first line).
    pub label: String,
    /// The marker's own icon, when given (drawn at its own size; kept on mouse-over).
    pub icon: Option<ImageRef>,
    /// A quest marker (the feed's `quest` is present).
    pub quest: bool,
    /// The feed's `note_type` key (1 … 64) whose icon is drawn when `icon` is absent.
    pub note_type: Option<u32>,
}

impl FeedMarker {
    /// The type value whose icon-table entry gives the marker's images when it has no own
    /// icon (§6.7 item 2; `scenario-feed.md` `note-type=`): the `note_type` key when
    /// given, else `Quest` for a quest marker and `Waypoint` for a plain one.
    #[must_use]
    pub fn icon_type(&self) -> u64 {
        self.note_type
            .and_then(type_of_key)
            .unwrap_or(1u64 << if self.quest { QUEST_BIT } else { WAYPOINT_BIT })
    }

    /// The marker's images from a view's table (§6.7 item 2).
    #[must_use]
    pub fn icons(&self, table: &IconTable) -> NoteIcons {
        match &self.icon {
            Some(i) => NoteIcons::single(i.clone()),
            None => table
                .get(self.icon_type())
                .map(|e| e.onscreen.clone())
                .unwrap_or_default(),
        }
    }
}

/// Texts the map views ask the client for (rendered from the install's string tables and
/// the coordinate rules of `coordinates.md` §8; the defaults are Tessera's own).
pub trait MapTexts {
    /// The two map-coordinate texts of a point (CO §8), joined as the client shows them;
    /// `None` when they cannot be formed.
    fn coordinates(&self, point: &MapPoint) -> Option<String> {
        let _ = point;
        None
    }

    /// `CoordinateDisplay`'s text for a point (MW19: the `WeenieClientMsgs` entry with the
    /// two coordinate texts); default the coordinates themselves.
    fn coordinate_display(&self, point: &MapPoint) -> Option<String> {
        self.coordinates(point)
    }

    /// The `MapNoteType` display text of key `key` (a filter-menu item, MW13); default
    /// Tessera's own "Type *key*".
    fn note_type_label(&self, key: u32) -> String {
        format!("Type {key}")
    }

    /// A fixed label of the map window.
    fn label(&self, label: MapLabel) -> Option<String> {
        let _ = label;
        None
    }

    /// The radar tooltip of a note (MW22: the `UI_Maps` entry with the note's text and the
    /// horizontal distance); default Tessera's own "*text* (*distance*)".
    fn radar_tooltip(&self, text: &str, distance: f32) -> String {
        format!("{text} ({distance:.0})")
    }
}

/// A fixed label of the map window: the install's layouts carry their own; the client
/// gives the ones game code sets and the fallback widgets show Tessera's own short text
/// ([`MapLabel::fallback`]) when the client gives none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapLabel {
    /// The filter menu's *all* item (`UI_Maps` 0x09F086CC).
    FilterAll,
    /// The filter menu's *none* item (`UI_Maps` 0x0EBA8F2C).
    FilterNone,
    /// `FilterMenu_Label`.
    FilterMenu,
    /// `ShowMap_Label`.
    MapMenu,
    /// The windowed root's title.
    Title,
}

impl MapLabel {
    /// Tessera's own short text.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::FilterAll => "Show all",
            Self::FilterNone => "Hide all",
            Self::FilterMenu => "Filter",
            Self::MapMenu => "Maps",
            Self::Title => "Map",
        }
    }
}

/// [`MapTexts`] with every default.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultMapTexts;

impl MapTexts for DefaultMapTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn map_label(texts: &dyn MapTexts, l: MapLabel) -> String {
    texts.label(l).unwrap_or_else(|| l.fallback().to_string())
}

/// Tessera's tooltip of a feed marker on the world map (§6.7 item 6): the label and, on a
/// second line, the location's map coordinates (Tessera wording).
#[must_use]
pub fn marker_tooltip(texts: &dyn MapTexts, m: &FeedMarker) -> String {
    match texts.coordinates(&m.point) {
        Some(c) if !m.label.is_empty() => format!("{}\n{c}", m.label),
        Some(c) => c,
        None => m.label.clone(),
    }
}

// ----- the filter menu (§6.9) ----------------------------------------------------------

/// One item of the note-type filter menu (`map-windows.md` MW13, §6.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterItem {
    /// *all*: sets every listed bit.
    All,
    /// *none*: clears every listed bit.
    None,
    /// One note type.
    Type {
        /// The bit.
        bit: u32,
        /// The `MapNoteType` display text of key *bit* + 1.
        label: String,
        /// The type's menu image (`UI_Map_MapNoteMenuImage`).
        icon: Option<ImageRef>,
        /// Whether the bit is set in the filter value.
        checked: bool,
    },
}

/// The model of `FilterMenu` for one shown map (`map-windows.md` §6.9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterMenuModel {
    /// The items: *all*, *none*, then one per listed bit sorted by label.
    pub items: Vec<FilterItem>,
    /// The listed bits as a mask.
    pub listed: u64,
}

/// Tessera's collation of menu labels (`map-windows.md` §4.5 and §6.9 Tessera rules):
/// ascending Unicode scalar values after simple lower-casing.
#[must_use]
pub fn collation_key(s: &str) -> Vec<char> {
    s.chars().flat_map(char::to_lowercase).collect()
}

impl FilterMenuModel {
    /// Builds the menu (MW13): two command items, then one check item per bit of the
    /// display mask except [`UNLISTED_FILTER_BITS`], labelled by `texts`, sorted by label
    /// (ties by bit), checked when the bit is set in `filter`.
    #[must_use]
    pub fn build(display_mask: u64, filter: u64, texts: &dyn MapTexts, icons: &IconTable) -> Self {
        let mut types: Vec<(Vec<char>, u32, String)> = (0..64u32)
            .filter(|b| display_mask & (1u64 << b) != 0 && !UNLISTED_FILTER_BITS.contains(b))
            .map(|b| {
                let label = cut(&texts.note_type_label(b + 1));
                (collation_key(&label), b, label)
            })
            .collect();
        types.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut listed = 0u64;
        let mut items = vec![FilterItem::All, FilterItem::None];
        for (_, bit, label) in types {
            listed |= 1u64 << bit;
            items.push(FilterItem::Type {
                bit,
                label,
                icon: icons.get(1u64 << bit).and_then(|e| e.menu.clone()),
                checked: filter & (1u64 << bit) != 0,
            });
        }
        Self { items, listed }
    }

    /// Selecting item `i` (§6.9 "Selecting"): *all* sets and *none* clears every listed
    /// bit, a type item toggles its bit. Returns the new filter value (unchanged for an
    /// index outside the menu) and updates the check marks.
    pub fn select(&mut self, i: usize, filter: u64) -> u64 {
        let new = match self.items.get(i) {
            Some(FilterItem::All) => filter | self.listed,
            Some(FilterItem::None) => filter & !self.listed,
            Some(FilterItem::Type { bit, .. }) => filter ^ (1u64 << bit),
            None => return filter,
        };
        self.refresh(new);
        new
    }

    /// Sets every check mark from a filter value.
    pub fn refresh(&mut self, filter: u64) {
        for it in &mut self.items {
            if let FilterItem::Type { bit, checked, .. } = it {
                *checked = filter & (1u64 << *bit) != 0;
            }
        }
    }
}

/// A command of the radar's filter menu (`map-windows.md` §6.8, MW23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarFilterCommand {
    /// Only bit 45.
    OnlyOffscreen,
    /// Reset to the radar's display mask.
    Reset,
    /// Toggle one type's bit (toggling bit 1 toggles bits 0 and 1).
    Toggle(u32),
}

/// Applies a radar filter command (MW23) and returns the new filter value.
#[must_use]
pub fn radar_filter_command(filter: u64, display_mask: u64, cmd: RadarFilterCommand) -> u64 {
    match cmd {
        RadarFilterCommand::OnlyOffscreen => 1u64 << OFFSCREEN_BIT,
        RadarFilterCommand::Reset => display_mask,
        RadarFilterCommand::Toggle(1) => filter ^ 0b11,
        RadarFilterCommand::Toggle(b) if b < 64 => filter ^ (1u64 << b),
        RadarFilterCommand::Toggle(_) => filter,
    }
}

// ----- elements ------------------------------------------------------------------------

/// The size of an image when it is loaded.
fn image_size(ui: &mut Ui, img: &ImageRef) -> Option<(i32, i32)> {
    match ui.env.image(img) {
        ImageState::Ready(i) => Some((
            i32::try_from(i.width()).unwrap_or(i32::MAX),
            i32::try_from(i.height()).unwrap_or(i32::MAX),
        )),
        _ => None,
    }
}

/// The shape of a note element: the `MapNoteTemplate` instance's size and z-level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteShape {
    /// The template's size.
    pub size: (i32, i32),
    /// The template's z-level.
    pub z: i32,
}

impl Default for NoteShape {
    fn default() -> Self {
        Self {
            size: NOTE_SIZE,
            z: NOTE_Z,
        }
    }
}

/// A note element (§6.3 item 2, §6.7 item 3): a field of the template's shape with the
/// template's element ID, holding `MTButton` whose `normal` state shows the normal image,
/// its rollover states the highlight image and its `highlight` state the selected image;
/// the button is sized to the normal image (the template's size while it is not loaded)
/// and centred in the field (Tessera rule: the icon is centred on the note's pixel). The
/// tooltip goes on the button.
fn note_element(
    ui: &mut Ui,
    pv: &P5Vocab,
    parent: ElementKey,
    shape: NoteShape,
    icons: &NoteIcons,
    tooltip: &str,
) -> Option<ElementKey> {
    let (w, h) = (shape.size.0.clamp(1, 512), shape.size.1.clamp(1, 512));
    let (bw, bh) = icons
        .normal
        .as_ref()
        .and_then(|i| image_size(ui, i))
        .map_or((w, h), |(a, b)| (a.clamp(1, 512), b.clamp(1, 512)));
    let st = ui.vocab().states;
    let high = icons.highlight.clone().or_else(|| icons.normal.clone());
    let sel = icons.selected.clone().or_else(|| icons.normal.clone());
    let button = FNode::new(
        WidgetClass::Button,
        pv.ids.note_button,
        (w - bw) / 2,
        (h - bh) / 2,
        bw,
        bh,
    )
    .image(&icons.normal)
    .state(st.normal, icons.normal.as_ref(), vec![])
    .state(st.normal_rollover, high.as_ref(), vec![])
    .state(st.highlight, sel.as_ref(), vec![])
    .state(st.highlight_rollover, high.as_ref(), vec![]);
    let node = FNode::field(pv.ids.note_template, 0, 0, w, h).child(button);
    let k = instantiate(ui, parent, &node)?;
    ui.set_z(k, shape.z);
    if !tooltip.is_empty()
        && let Some(b) = ui.children(k).first().copied()
    {
        ui.set_tooltip_text(b, Some(cut(tooltip)));
        let p = ui.vocab().pids.tooltips_on;
        ui.set_property(b, p, PropertyValue::Bool(true));
    }
    Some(k)
}

/// Moves `k` so that its centre lies on `pos` (parent coordinates).
fn centre_at(ui: &mut Ui, k: ElementKey, pos: (i32, i32)) {
    let (w, h) = ui.layout_box(k).map_or((0, 0), |b| (b.w(), b.h()));
    ui.move_to(k, pos.0.saturating_sub(w / 2), pos.1.saturating_sub(h / 2));
}

/// Whether a pixel lies inside a frame (`0 ≤ x < w`, `0 ≤ y < h`).
#[must_use]
pub fn in_frame(px: (i32, i32), frame: (i32, i32)) -> bool {
    (0..frame.0).contains(&px.0) && (0..frame.1).contains(&px.1)
}

/// What an overlay element shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OverlayId {
    /// A table or run-time note, by its [`MapNote::id`].
    Note(u64),
    /// A feed marker, by its id.
    Marker(u32),
}

/// The shown map the world-map overlays are placed on (from
/// [`super::mapwindow::MapWindowView::context`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapContext {
    /// The shown map element (the node's active element, or `DungeonMap_ImageField`):
    /// notes are its children, in its pixel frame.
    pub parent: ElementKey,
    /// The shown map's georeference (`None`: nothing can be placed).
    pub georef: Option<Georef>,
    /// The pixel frame.
    pub frame: (i32, i32),
    /// The node's region (`None`: the root or a dungeon map).
    pub region: Option<u8>,
    /// A dungeon map is shown.
    pub dungeon: bool,
    /// The shown node (`None` for a dungeon map).
    pub node: Option<usize>,
    /// The display mask (all bits on a dungeon map).
    pub display_mask: u64,
    /// The filter value (read, §6.9).
    pub filter: u64,
}

impl MapContext {
    /// The note's or marker's map pixel by §6.1.
    #[must_use]
    pub fn pixel(&self, p: &MapPoint) -> Option<(i32, i32)> {
        let g = self.georef?;
        let inst = if self.region.is_some() { Some(0) } else { None };
        Some(pixel_in_region(&g, self.region, inst, p))
    }
}

/// What the minimap's overlays need besides the radar and its input.
#[derive(Debug, Clone, Copy)]
pub struct RadarContext<'a> {
    /// The radar's icon table (`UI_RadarNoteIcons_Map`).
    pub icons: &'a IconTable,
    /// The radar's filter value (`GOP_Radar_MapNoteFilter` minus the private encounter's
    /// disallowed types, §6.8).
    pub filter: u64,
    /// The player's height (for ranged notes' three-dimensional distance).
    pub player_height: f32,
    /// The player's instance (0 outside an instance): the radar's origin is the player's
    /// position including its instance (§6.8 through §6.1), so only notes and markers of
    /// this instance are drawn.
    pub instance: u32,
}

/// Where an element of a minimap overlay is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RadarMode {
    /// Inside the radius: on-screen images at the pixel.
    Inside,
    /// On the rim: off-screen images.
    Rim,
}

/// Where and how one minimap overlay is drawn: mode, images, position in the viewport,
/// rotation (degrees) and horizontal distance.
type RadarPlace = (RadarMode, NoteIcons, (i32, i32), f32, f32);

/// The overlays of the world map and the minimaps: table and run-time notes, feed markers
/// and their elements.
#[derive(Debug, Clone, Default)]
pub struct MapOverlays {
    notes: Vec<MapNote>,
    markers: BTreeMap<u32, FeedMarker>,
    map: BTreeMap<OverlayId, ElementKey>,
    /// Minimap elements: (element, mode, tooltip text last set).
    radar: BTreeMap<(ElementKey, OverlayId), (ElementKey, RadarMode, String)>,
    stale: Vec<ElementKey>,
}

impl MapOverlays {
    /// Replaces the notes (table notes of the shown map's query and the run-time notes;
    /// at most [`MAX_NOTES`]). Their elements go; the caller places them again. Ids are
    /// unique: of notes with the same id only the first is kept (Tessera rule).
    pub fn set_notes(&mut self, ui: &mut Ui, notes: Vec<MapNote>) {
        let mut seen = BTreeSet::new();
        let before = notes.len();
        let mut notes: Vec<MapNote> = notes.into_iter().filter(|n| seen.insert(n.id)).collect();
        if notes.len() != before {
            tracing::warn!(
                "map notes: {} with a repeated id dropped",
                before - notes.len()
            );
        }
        notes.truncate(MAX_NOTES);
        self.notes = notes;
        self.forget(|id| matches!(id, OverlayId::Note(_)));
        self.destroy_stale(ui);
    }

    /// The notes.
    #[must_use]
    pub fn notes(&self) -> &[MapNote] {
        &self.notes
    }

    /// A `MarkerAdded` (§6.7 item 7): added, or replacing the marker with the same id. Past
    /// [`MAX_MARKERS`] a new marker is dropped (returns false). Its elements go; the caller
    /// places the markers again.
    pub fn add_marker(&mut self, ui: &mut Ui, m: FeedMarker) -> bool {
        if !self.markers.contains_key(&m.id) && self.markers.len() >= MAX_MARKERS {
            tracing::warn!("feed markers: more than {MAX_MARKERS}; marker dropped");
            return false;
        }
        let id = m.id;
        self.markers.insert(id, m);
        self.forget(|o| o == OverlayId::Marker(id));
        self.destroy_stale(ui);
        true
    }

    /// A `MarkerRemoved`: the marker and its elements go. Returns whether it existed.
    pub fn remove_marker(&mut self, ui: &mut Ui, id: u32) -> bool {
        let had = self.markers.remove(&id).is_some();
        self.forget(|o| o == OverlayId::Marker(id));
        self.destroy_stale(ui);
        had
    }

    /// The feed markers, by id.
    #[must_use]
    pub fn markers(&self) -> &BTreeMap<u32, FeedMarker> {
        &self.markers
    }

    /// The world-map element of an overlay, when created.
    #[must_use]
    pub fn map_element(&self, id: OverlayId) -> Option<ElementKey> {
        self.map.get(&id).copied()
    }

    /// The minimap element of an overlay on radar `radar`, when created and shown there.
    #[must_use]
    pub fn radar_element(&self, radar: ElementKey, id: OverlayId) -> Option<ElementKey> {
        self.radar.get(&(radar, id)).map(|(k, _, _)| *k)
    }

    fn forget(&mut self, f: impl Fn(OverlayId) -> bool) {
        let gone: Vec<OverlayId> = self.map.keys().copied().filter(|i| f(*i)).collect();
        for g in gone {
            if let Some(k) = self.map.remove(&g) {
                self.stale.push(k);
            }
        }
        let gone: Vec<(ElementKey, OverlayId)> =
            self.radar.keys().copied().filter(|(_, i)| f(*i)).collect();
        for g in gone {
            if let Some((k, _, _)) = self.radar.remove(&g) {
                self.stale.push(k);
            }
        }
    }

    fn destroy_stale(&mut self, ui: &mut Ui) {
        let stale = std::mem::take(&mut self.stale);
        let live: Vec<ElementKey> = stale.into_iter().filter(|k| ui.exists(*k)).collect();
        ui.destroy_many(&live);
    }

    /// Removes every world-map element (a map is hidden or replaced).
    pub fn clear_map(&mut self, ui: &mut Ui) {
        self.forget_map();
        self.destroy_stale(ui);
    }

    fn forget_map(&mut self) {
        let all: Vec<ElementKey> = std::mem::take(&mut self.map).into_values().collect();
        self.stale.extend(all);
    }

    /// Forgets the elements of destroyed parents (closed panels, replaced maps).
    pub fn prune(&mut self, ui: &Ui) {
        self.map.retain(|_, k| ui.exists(*k));
        self.radar.retain(|_, (k, _, _)| ui.exists(*k));
    }

    /// Places every overlay on the shown world map (§6.2–§6.4, §6.7): the previous
    /// elements go; table notes are created when the creation test holds (§6.3 item 1:
    /// the display mask, any type on a dungeon map, and outdoors the node of the note's
    /// area being the shown node or a descendant) and their pixel lies in the frame,
    /// run-time notes when their pixel lies in the frame; notes are visible under the
    /// filter (§6.3 item 3); then each feed marker whose pixel lies in the frame, in id
    /// order, after (so in front of) the notes, never filtered (§6.7 items 3–5).
    #[allow(clippy::too_many_arguments)]
    pub fn place_on_map(
        &mut self,
        ui: &mut Ui,
        pv: &P5Vocab,
        ctx: &MapContext,
        tree: Option<&MapTree>,
        icons: &IconTable,
        texts: &dyn MapTexts,
        shape: NoteShape,
    ) {
        self.forget_map();
        self.destroy_stale(ui);
        if ctx.georef.is_none() || !ui.exists(ctx.parent) {
            return;
        }
        let notes = self.notes.clone();
        for n in &notes {
            if !n.runtime {
                if !created_on_map(n.note_type, ctx.display_mask, ctx.dungeon) {
                    continue;
                }
                if !ctx.dungeon
                    && let (Some(t), Some(shown)) = (tree, ctx.node)
                {
                    let node = t.player_node(n.area).unwrap_or(0);
                    if !t.is_within(node, shown) {
                        continue;
                    }
                }
            }
            let Some(px) = ctx.pixel(&n.point).filter(|p| in_frame(*p, ctx.frame)) else {
                continue;
            };
            let entry = icons.get(n.note_type);
            let images = n
                .icons
                .clone()
                .or_else(|| entry.map(|e| e.onscreen.clone()))
                .unwrap_or_default();
            let z = note_z(shape.z, n.note_type, entry.map_or(0, |e| e.relative_z));
            let Some(k) = note_element(
                ui,
                pv,
                ctx.parent,
                NoteShape { z, ..shape },
                &images,
                &n.tooltip,
            ) else {
                break;
            };
            centre_at(ui, k, px);
            ui.set_shown(k, visible_under(n.note_type, ctx.filter));
            self.map.insert(OverlayId::Note(n.id), k);
        }
        let ids: Vec<u32> = self.markers.keys().copied().collect();
        for id in ids {
            if !self.put_marker(ui, pv, ctx, icons, texts, shape, id) {
                break;
            }
        }
    }

    /// Places one feed marker on the shown world map after a `MarkerAdded` (§6.7 items
    /// 3–5), without re-placing the notes: its previous element goes, it is created when
    /// its pixel lies in the frame, and the placed markers with a greater id are created
    /// again after it, so the markers stay in id order in front of the notes.
    #[allow(clippy::too_many_arguments)]
    pub fn place_feed_marker(
        &mut self,
        ui: &mut Ui,
        pv: &P5Vocab,
        ctx: &MapContext,
        icons: &IconTable,
        texts: &dyn MapTexts,
        shape: NoteShape,
        id: u32,
    ) {
        if let Some(k) = self.map.remove(&OverlayId::Marker(id)) {
            self.stale.push(k);
        }
        let later: Vec<u32> = self
            .markers
            .range(id.saturating_add(1)..)
            .map(|(k, _)| *k)
            .filter(|k| *k > id && self.map.contains_key(&OverlayId::Marker(*k)))
            .collect();
        for l in &later {
            if let Some(k) = self.map.remove(&OverlayId::Marker(*l)) {
                self.stale.push(k);
            }
        }
        self.destroy_stale(ui);
        if ctx.georef.is_none() || !ui.exists(ctx.parent) {
            return;
        }
        for i in std::iter::once(id).chain(later) {
            if !self.put_marker(ui, pv, ctx, icons, texts, shape, i) {
                break;
            }
        }
    }

    /// Creates the world-map element of marker `id` when its pixel lies in the frame.
    /// Returns false when no element could be made (the element cap).
    #[allow(clippy::too_many_arguments)]
    fn put_marker(
        &mut self,
        ui: &mut Ui,
        pv: &P5Vocab,
        ctx: &MapContext,
        icons: &IconTable,
        texts: &dyn MapTexts,
        shape: NoteShape,
        id: u32,
    ) -> bool {
        let Some(m) = self.markers.get(&id) else {
            return true;
        };
        let Some(px) = ctx.pixel(&m.point).filter(|p| in_frame(*p, ctx.frame)) else {
            return true;
        };
        let images = m.icons(icons);
        let Some(k) = note_element(
            ui,
            pv,
            ctx.parent,
            NoteShape { z: NOTE_Z, ..shape },
            &images,
            &marker_tooltip(texts, m),
        ) else {
            return false;
        };
        centre_at(ui, k, px);
        self.map.insert(OverlayId::Marker(id), k);
        true
    }

    /// A new filter value (§6.9 "Selecting"): every note's visibility is updated; feed
    /// markers are not filtered.
    pub fn apply_filter(&self, ui: &mut Ui, filter: u64) {
        for n in &self.notes {
            if let Some(k) = self.map.get(&OverlayId::Note(n.id)) {
                ui.set_shown(*k, visible_under(n.note_type, filter));
            }
        }
    }

    /// Places the overlays on a minimap after its position update (§6.8): each note
    /// passing the radar's filter (link rows never) inside the radius shows its on-screen
    /// images at its pixel; outside the radius, and every ranged note within its range,
    /// its off-screen images on the circle of radius `UI_Radar_RadarRadius` around the
    /// viewport centre in the direction of the note, turned that way (Tessera rule:
    /// the off-screen image points east unturned, as the player marker does); a note
    /// without off-screen images is not drawn there. Feed markers are drawn inside the
    /// radius only. Points of another region or instance than the player's are not drawn.
    /// Everything is behind `MinimapPlayerMarker`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_radar(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        pv: &P5Vocab,
        radar: &Radar,
        input: &RadarInput,
        ctx: &RadarContext<'_>,
        texts: &dyn MapTexts,
    ) {
        self.destroy_stale(ui);
        let Some(viewport) = radar.viewport() else {
            return;
        };
        let georef = radar.shown_georef().filter(|_| input.valid());
        let radius_px = ui.prop(radar.key, gv.pids.radar_radius).int();
        let world_r = georef.and_then(|g| radar_world_radius(radius_px, g.scale));
        let player = MapPoint {
            global: input.global,
            region: input.region,
            instance: ctx.instance,
        };
        // The notes and markers are taken out while their elements are placed (put_radar
        // does not read them) and put back below.
        let notes = std::mem::take(&mut self.notes);
        let markers = std::mem::take(&mut self.markers);
        let mut seen: BTreeSet<OverlayId> = BTreeSet::new();
        for n in &notes {
            let id = OverlayId::Note(n.id);
            seen.insert(id);
            let place = (|| {
                let (g, r) = (georef?, world_r?);
                if n.note_type & LINK_TYPE != 0 || !visible_under(n.note_type, ctx.filter) {
                    return None;
                }
                if n.point.region != player.region || n.point.instance != player.instance {
                    return None;
                }
                let dx = n.point.global.0 - player.global.0;
                let dy = n.point.global.1 - player.global.1;
                let d = (dx * dx + dy * dy).sqrt();
                let ranged = n.range.filter(|(a, b)| is_ranged(*a, *b));
                if let Some((min, max)) = ranged {
                    let dz = n.height - ctx.player_height;
                    if !ranged_visible(dx * dx + dy * dy + dz * dz, min, max) {
                        return None;
                    }
                }
                let pp = g.to_pixel(player.global.0, player.global.1);
                let np = g.to_pixel(n.point.global.0, n.point.global.1);
                if ranged.is_none() && d < r {
                    let images = n
                        .icons
                        .clone()
                        .or_else(|| ctx.icons.get(n.note_type).map(|e| e.onscreen.clone()))?;
                    let pos = (
                        radar.centre.0.saturating_add(np.0.saturating_sub(pp.0)),
                        radar.centre.1.saturating_add(np.1.saturating_sub(pp.1)),
                    );
                    return Some((RadarMode::Inside, images, pos, 0.0, d));
                }
                // Off-screen: the entry of the note's type, or for a ranged note the
                // entry of bit 45 only (`map-data.md` §4.8.3 step 4).
                let key = if ranged.is_some() {
                    1u64 << OFFSCREEN_BIT
                } else {
                    n.note_type
                };
                let mut images = ctx.icons.get(key).and_then(|e| e.offscreen.clone())?;
                if let Some(o) = &n.offscreen {
                    images.normal = Some(o.clone());
                }
                #[allow(clippy::cast_precision_loss)]
                let (vx, vy) = (
                    np.0.saturating_sub(pp.0) as f32,
                    np.1.saturating_sub(pp.1) as f32,
                );
                let len = (vx * vx + vy * vy).sqrt();
                let (ux, uy) = if len > 0.0 {
                    (vx / len, vy / len)
                } else {
                    (1.0, 0.0)
                };
                #[allow(clippy::cast_precision_loss)]
                let rf = radius_px as f32;
                #[allow(clippy::cast_possible_truncation)]
                let pos = (
                    radar.centre.0.saturating_add((ux * rf).round() as i32),
                    radar.centre.1.saturating_add((uy * rf).round() as i32),
                );
                // Screen y points down; Tessera's rotation is counter-clockwise positive.
                let angle = (-uy).atan2(ux).to_degrees().rem_euclid(360.0);
                Some((RadarMode::Rim, images, pos, angle, d))
            })();
            self.put_radar(ui, pv, radar.key, viewport, id, place, |d| {
                texts.radar_tooltip(&n.text, d)
            });
        }
        for m in markers.values() {
            let id = OverlayId::Marker(m.id);
            seen.insert(id);
            let place = (|| {
                let (g, r) = (georef?, world_r?);
                if m.point.region != player.region || m.point.instance != player.instance {
                    return None;
                }
                let dx = m.point.global.0 - player.global.0;
                let dy = m.point.global.1 - player.global.1;
                let d = (dx * dx + dy * dy).sqrt();
                if d >= r {
                    return None;
                }
                let pp = g.to_pixel(player.global.0, player.global.1);
                let np = g.to_pixel(m.point.global.0, m.point.global.1);
                let pos = (
                    radar.centre.0.saturating_add(np.0.saturating_sub(pp.0)),
                    radar.centre.1.saturating_add(np.1.saturating_sub(pp.1)),
                );
                Some((RadarMode::Inside, m.icons(ctx.icons), pos, 0.0, d))
            })();
            self.put_radar(ui, pv, radar.key, viewport, id, place, |d| {
                texts.radar_tooltip(&m.label, d)
            });
        }
        self.notes = notes;
        self.markers = markers;
        // Elements of overlays that no longer exist.
        let gone: Vec<(ElementKey, OverlayId)> = self
            .radar
            .keys()
            .copied()
            .filter(|(r, i)| *r == radar.key && !seen.contains(i))
            .collect();
        for g in gone {
            if let Some((k, _, _)) = self.radar.remove(&g) {
                self.stale.push(k);
            }
        }
        self.destroy_stale(ui);
        if let Some(pm) = radar.player_marker() {
            move_to_front(ui, pm);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn put_radar(
        &mut self,
        ui: &mut Ui,
        pv: &P5Vocab,
        radar: ElementKey,
        viewport: ElementKey,
        id: OverlayId,
        place: Option<RadarPlace>,
        tooltip: impl Fn(f32) -> String,
    ) {
        let key = (radar, id);
        let Some((mode, images, pos, angle, d)) = place else {
            if let Some((k, _, _)) = self.radar.get(&key) {
                ui.set_shown(*k, false);
            }
            return;
        };
        let existing = self
            .radar
            .get(&key)
            .filter(|(k, m, _)| ui.exists(*k) && *m == mode)
            .map(|(k, _, _)| *k);
        let k = match existing {
            Some(k) => k,
            None => {
                if let Some((old, _, _)) = self.radar.remove(&key)
                    && ui.exists(old)
                {
                    ui.destroy(old);
                }
                let Some(k) = note_element(ui, pv, viewport, NoteShape::default(), &images, "")
                else {
                    return;
                };
                // §6.8 (MW22): the radar's notes are template instances with object mode 1.
                // Tessera rule: only the rim's notes, which turn, take it (and so a surface
                // of their own, `render.md` §6.10); an unturned note in object mode 1 at
                // its own size draws the same pixels in the viewport's canvas.
                if mode == RadarMode::Rim {
                    let om = ui.vocab().pids.object_mode;
                    ui.set_property(k, om, PropertyValue::Enum(object_mode::STRETCH));
                }
                self.radar.insert(key, (k, mode, String::new()));
                k
            }
        };
        // The tooltip is set only when its text changes (the distance moved on).
        let text = cut(&tooltip(d));
        if let Some((_, _, last)) = self.radar.get_mut(&key)
            && *last != text
            && let Some(b) = ui.children(k).first().copied()
        {
            ui.set_tooltip_text(b, Some(text.clone()));
            let p = ui.vocab().pids.tooltips_on;
            ui.set_property(b, p, PropertyValue::Bool(true));
            *last = text;
        }
        centre_at(ui, k, pos);
        if mode == RadarMode::Rim {
            ui.window_set_rotation(k, [0.0, 0.0, angle]);
        }
        show(ui, Some(k), true);
    }
}
