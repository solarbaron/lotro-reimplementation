// SPDX-License-Identifier: GPL-3.0-or-later
//! The landblock and sky views (M4.5).
//!
//! - **Landblock** (any landblock-family record): a top-down map of the terrain heights
//!   with the landblock water, the generated scenery (`scenery.md` §7), the static
//!   entities, the cells with their portals and the liquid boxes; tables of the generated
//!   objects, statics, cells and liquid boxes; the scenery counters, the water decision
//!   (`water.md` §6.2) and the collision summary (`collision.md` §12.1). Everything is
//!   built on a worker by `tessera-world` exactly as the walker builds it
//!   ([`crate::world::LandblockDecoder`]). The 3D view of the same record draws the
//!   collision geometry as overlays.
//! - **Sky** (0x15 sky descriptions, 0x1A day descriptions): the blended keyframes over a
//!   game day (`environment.md` §11.3) as colour bands and curves, the keyframe times,
//!   sky-object visibility (§11.4), and the values at a chosen time of day. For a sky
//!   description the day description is chosen per sky key, day type and game day
//!   (§11.2) and the choices of the following days are listed.
//!
//! Views only request products and draw them. Timelines are computed on the workers: a
//! day description's product carries its timeline ([`crate::world::SkyProduct::Day`]), and
//! the sky view requests the product of the day description it chose. On the UI thread
//! only the §11.2 choice of a few days is made (cached per sky key, day type and day).

use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use tessera_assets::{AssetKey, AssetState, Handle};
use tessera_formats::environment::day::DAY_DESCRIPTION_PREFIX;
use tessera_world::environment::{ChoiceSource, DayChoice, SkyValues};

use crate::app::{Current, Loaded};
use crate::cli::ViewKind;
use crate::views::{Action, ensure, ready};
use crate::world::{
    LandblockProduct, ObjectRow, START_DAY_TYPE, SkyProduct, TIMELINE_SAMPLES, Timeline,
    defaults_timeline, initial_sky_key, landblock_of,
};

/// Side of a landblock in units.
const SIDE: f32 = 160.0;

/// The landblock view's table tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LandblockTab {
    /// Generated scenery.
    #[default]
    Generated,
    /// Static entities of landblock info.
    Statics,
    /// Cells.
    Cells,
    /// Water decision and liquid boxes.
    Water,
    /// Collision summary.
    Collision,
    /// Scenery counters.
    Scenery,
    /// Bundle problems.
    Problems,
}

impl LandblockTab {
    const ALL: [Self; 7] = [
        Self::Generated,
        Self::Statics,
        Self::Cells,
        Self::Water,
        Self::Collision,
        Self::Scenery,
        Self::Problems,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Generated => "Generated",
            Self::Statics => "Statics",
            Self::Cells => "Cells",
            Self::Water => "Water",
            Self::Collision => "Collision",
            Self::Scenery => "Scenery",
            Self::Problems => "Problems",
        }
    }
}

/// Layers of the landblock map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapLayers {
    /// Landblock water tint.
    pub water: bool,
    /// Generated objects.
    pub generated: bool,
    /// Static entities.
    pub statics: bool,
    /// Cells and portals.
    pub cells: bool,
    /// Liquid boxes.
    pub liquids: bool,
    /// Ceiling instead of terrain heights.
    pub ceiling: bool,
}

impl Default for MapLayers {
    fn default() -> Self {
        Self {
            water: true,
            generated: true,
            statics: true,
            cells: true,
            liquids: true,
            ceiling: false,
        }
    }
}

/// Which list an object of the map belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectList {
    /// [`LandblockProduct::generated`].
    Generated,
    /// [`LandblockProduct::statics`].
    Statics,
}

/// What the cached day choices were made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChoicesKey {
    did: u32,
    sky_key: u32,
    day_type: u32,
    day_number: i64,
}

/// The §11.2 choices of consecutive days: (day number, choice).
type DayChoices = Arc<[(i64, Option<DayChoice>)]>;

/// Days whose choice the sky view lists (the shown day first).
const LISTED_DAYS: usize = 7;

/// Per-selection state of the landblock and sky views.
#[derive(Debug)]
pub struct WorldViewState {
    /// The landblock product.
    pub landblock: Option<Handle<LandblockProduct>>,
    /// The sky product.
    pub sky: Option<Handle<SkyProduct>>,
    /// Table tab.
    pub tab: LandblockTab,
    /// Map layers.
    pub layers: MapLayers,
    /// Selected object.
    pub selected: Option<(ObjectList, usize)>,
    /// Sky key of a sky description's timeline.
    pub sky_key: Option<u32>,
    /// Day type of a sky description's timeline.
    pub day_type: u32,
    /// Game day number of a sky description's timeline; `None` until the sky product is
    /// ready (it then starts at the calendar's day at *G* = 0). Any day from 0 can be shown.
    pub day_number: Option<i64>,
    /// Cursor time of day, fraction 0–1.
    pub time: f32,
    /// The product of the day description the sky view chose (its timeline).
    pub day: Option<Handle<SkyProduct>>,
    /// Scroll the object table to the selection on its next draw (set when the map selects
    /// an object; consumed by the table).
    pub scroll_to_selected: bool,
    /// Whether the sky view has drawn a finished timeline (screenshot readiness).
    pub timeline_shown: bool,
    choices: Option<(ChoicesKey, DayChoices)>,
    defaults: Option<Arc<Timeline>>,
}

impl Default for WorldViewState {
    fn default() -> Self {
        Self {
            landblock: None,
            sky: None,
            tab: LandblockTab::default(),
            layers: MapLayers::default(),
            selected: None,
            sky_key: None,
            day_type: START_DAY_TYPE,
            day_number: None,
            time: 0.5,
            day: None,
            scroll_to_selected: false,
            timeline_shown: false,
            choices: None,
            defaults: None,
        }
    }
}

impl WorldViewState {
    /// Cancels the pending products.
    pub fn cancel(&self) {
        if let Some(h) = &self.landblock {
            h.cancel();
        }
        if let Some(h) = &self.sky {
            h.cancel();
        }
        if let Some(h) = &self.day {
            h.cancel();
        }
    }
}

/// Whether the sky view of `w` is complete (screenshot mode): the product is finished, and
/// for a sky description a finished timeline was drawn.
#[must_use]
pub(crate) fn sky_ready(w: &WorldViewState) -> bool {
    let Some(h) = &w.sky else { return false };
    if !h.is_finished() {
        return false;
    }
    match h.get() {
        Some(p) if matches!(*p, SkyProduct::Sky { .. }) => w.timeline_shown,
        _ => true,
    }
}

// ---------------------------------------------------------------------------------------
// Landblock view
// ---------------------------------------------------------------------------------------

/// The map colour of height `h` in `range` (low green → high brown → white).
#[must_use]
pub fn height_colour(h: f32, range: (f32, f32)) -> Color32 {
    let span = (range.1 - range.0).max(1e-3);
    let f = if h.is_finite() {
        ((h - range.0) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let stops: [(f32, [f32; 3]); 4] = [
        (0.0, [46.0, 92.0, 52.0]),
        (0.45, [120.0, 140.0, 70.0]),
        (0.8, [130.0, 100.0, 70.0]),
        (1.0, [225.0, 225.0, 220.0]),
    ];
    let mut c = stops[0].1;
    for w in stops.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if f >= a && f <= b {
            let k = (f - a) / (b - a).max(1e-6);
            c = std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * k);
        }
    }
    let b = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

/// The 32 × 32 quad image of the map: each quad coloured by the mean of its four corner
/// heights, tinted blue where the mean lies below the water height. Row 0 is north.
#[must_use]
pub fn map_image(heights: &[f32], range: (f32, f32), water: Option<f32>) -> egui::ColorImage {
    let h = |i: usize, j: usize| heights.get(33 * j + i).copied().unwrap_or(0.0);
    let mut pixels = Vec::with_capacity(32 * 32);
    for row in 0..32 {
        let j = 31 - row;
        for i in 0..32 {
            let mean = (h(i, j) + h(i + 1, j) + h(i, j + 1) + h(i + 1, j + 1)) * 0.25;
            let mut c = height_colour(mean, range);
            if let Some(w) = water
                && mean < w
            {
                let depth = ((w - mean) / 10.0).clamp(0.0, 1.0);
                let mix =
                    |a: u8, b: u8, k: f32| (f32::from(a) * (1.0 - k) + f32::from(b) * k) as u8;
                let k = 0.55 + 0.35 * depth;
                c = Color32::from_rgb(mix(c.r(), 30, k), mix(c.g(), 80, k), mix(c.b(), 170, k));
            }
            pixels.push(c);
        }
    }
    egui::ColorImage::new([32, 32], pixels)
}

/// Landblock-frame (x, y) → a point in `rect` (north up).
fn to_screen(rect: Rect, p: [f32; 3]) -> egui::Pos2 {
    pos2(
        rect.left() + p[0] / SIDE * rect.width(),
        rect.bottom() - p[1] / SIDE * rect.height(),
    )
}

/// Draws the landblock view.
pub(crate) fn landblock_view(
    ui: &mut Ui,
    l: &Loaded,
    cur: &mut Current,
    actions: &mut Vec<Action>,
) {
    let did = cur.nav.did;
    let Some(key) = landblock_of(did) else {
        ui.label("not a landblock record");
        return;
    };
    // Every record of the landblock shows the same product (keyed by its data DID).
    ensure(
        &mut cur.world.landblock,
        &l.stores.landblock,
        AssetKey::new(key.data_did().0, 0),
    );
    let Some(p) = ready(ui, cur.world.landblock.as_ref()) else {
        ui.label(
            RichText::new(
                "building the landblock (scenery, water, collision); the first one loads the \
                 world context",
            )
            .weak(),
        );
        return;
    };
    ui.horizontal_wrapped(|ui| {
        for (k, v) in &p.summary {
            ui.label(format!("{k}: {v}"));
        }
        ui.label(RichText::new(format!("built in {} ms", p.build_ms)).weak());
    });
    ui.horizontal_wrapped(|ui| {
        for (caption, d) in [
            ("data", key.data_did()),
            ("info", key.info_did()),
            ("block map", key.block_map_did()),
            ("properties", key.properties_did()),
        ] {
            if d.0 != did
                && ui
                    .small_button(format!("{caption} {:#010x}", d.0))
                    .clicked()
            {
                actions.push(Action::Open(d.0));
            }
        }
        ui.separator();
        for (caption, dx, dy) in [("◀ W", -1, 0), ("▶ E", 1, 0), ("▲ N", 0, 1), ("▼ S", 0, -1)]
        {
            let n = key.offset(dx, dy);
            if ui
                .add_enabled(n.is_some(), egui::Button::new(caption).small())
                .clicked()
                && let Some(n) = n
            {
                actions.push(Action::Open(n.data_did().0));
            }
        }
        ui.separator();
        if ui
            .small_button("3D collision")
            .on_hover_text("The 3D view of this landblock: terrain with collision overlays")
            .clicked()
        {
            actions.push(Action::View(ViewKind::Mesh));
        }
    });
    ui.separator();
    let avail = ui.available_size();
    let side = (avail.x * 0.5).min(avail.y - 40.0).clamp(160.0, 900.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(side);
            map_controls(ui, cur, &p);
            landblock_map(ui, cur, &p, side, actions);
        });
        ui.separator();
        ui.vertical(|ui| landblock_tables(ui, cur, &p, actions));
    });
}

fn map_controls(ui: &mut Ui, cur: &mut Current, p: &LandblockProduct) {
    let layers = &mut cur.world.layers;
    ui.horizontal_wrapped(|ui| {
        ui.toggle_value(&mut layers.generated, "generated")
            .on_hover_text(
                "Generated scenery (green; outlined black when it blocks a walker, violet when \
                 it has collision a walker passes through)",
            );
        ui.toggle_value(&mut layers.statics, "statics")
            .on_hover_text(
                "Static entities of landblock info (grey; outlined black when they block a \
                 walker, violet when a walker passes through their collision)",
            );
        ui.toggle_value(&mut layers.cells, "cells")
            .on_hover_text("Cell positions and portal outlines (cyan)");
        ui.toggle_value(&mut layers.liquids, "liquids")
            .on_hover_text("Liquid boxes (blue outlines)");
        ui.toggle_value(&mut layers.water, "water")
            .on_hover_text("Tint quads below the landblock water height");
        ui.add_enabled(
            p.ceiling.is_some(),
            egui::Checkbox::new(&mut layers.ceiling, "ceiling"),
        )
        .on_hover_text("Colour by ceiling heights instead of terrain heights");
    });
}

fn landblock_map(
    ui: &mut Ui,
    cur: &mut Current,
    p: &LandblockProduct,
    side: f32,
    actions: &mut Vec<Action>,
) {
    let layers = cur.world.layers;
    let ceiling = layers.ceiling && p.ceiling.is_some();
    let tex_key = format!(
        "landblock-map/{:08x}/{}{}",
        p.key.data_did().0,
        u8::from(ceiling),
        u8::from(layers.water)
    );
    let texture = cur
        .gpu
        .entry(tex_key.clone())
        .or_insert_with(|| {
            let (heights, range) = match (&p.ceiling, ceiling) {
                (Some(c), true) => {
                    let (lo, hi) = c
                        .iter()
                        .filter(|h| h.is_finite())
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), h| {
                            (lo.min(*h), hi.max(*h))
                        });
                    (c.as_slice(), if lo <= hi { (lo, hi) } else { (0.0, 0.0) })
                }
                _ => (p.heights.as_slice(), p.range),
            };
            let water = if layers.water && !ceiling {
                p.water
            } else {
                None
            };
            ui.ctx().load_texture(
                tex_key,
                map_image(heights, range, water),
                egui::TextureOptions::NEAREST,
            )
        })
        .clone();
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    let painter = ui.painter_at(rect);
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    // Quad grid every 8 quads (40 units).
    for k in 1..4 {
        let f = k as f32 * 0.25;
        let c = Color32::from_white_alpha(28);
        painter.vline(rect.left() + f * side, rect.y_range(), Stroke::new(1.0, c));
        painter.hline(rect.x_range(), rect.top() + f * side, Stroke::new(1.0, c));
    }
    if layers.liquids {
        for b in &p.liquids {
            let a = to_screen(rect, b.bounds.0);
            let z = to_screen(rect, b.bounds.1);
            painter.rect_stroke(
                Rect::from_two_pos(a, z),
                0.0,
                Stroke::new(1.5, Color32::from_rgb(70, 140, 255)),
                egui::StrokeKind::Middle,
            );
        }
    }
    if layers.cells {
        let cyan = Color32::from_rgb(80, 220, 230);
        for c in &p.cells {
            for o in &c.outlines {
                let pts: Vec<egui::Pos2> = o.iter().map(|v| to_screen(rect, *v)).collect();
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(1.0, cyan));
                }
                if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
                    painter.line_segment([*b, *a], Stroke::new(1.0, cyan));
                }
            }
            if let Some(pos) = c.position {
                painter.circle_filled(to_screen(rect, pos), 2.5, cyan);
            }
        }
    }
    let dot = |list: ObjectList, rows: &[ObjectRow], fill: Color32, radius: f32| {
        for (i, o) in rows.iter().enumerate() {
            let at = to_screen(rect, o.position);
            if !rect.expand(4.0).contains(at) {
                continue;
            }
            let selected = cur.world.selected == Some((list, i));
            painter.circle_filled(at, radius, fill);
            if o.blocks {
                painter.circle_stroke(at, radius + 1.0, Stroke::new(1.0, Color32::BLACK));
            } else if o.collides {
                painter.circle_stroke(at, radius + 1.0, Stroke::new(1.0, PASS_THROUGH));
            }
            if selected {
                painter.circle_stroke(at, radius + 4.0, Stroke::new(2.0, Color32::YELLOW));
            }
        }
    };
    if layers.statics {
        dot(
            ObjectList::Statics,
            &p.statics,
            Color32::from_gray(200),
            2.5,
        );
    }
    if layers.generated {
        dot(
            ObjectList::Generated,
            &p.generated,
            Color32::from_rgb(90, 230, 90),
            2.0,
        );
    }
    // Hover and click: the nearest shown object within 8 points.
    if let Some(pointer) = resp.hover_pos() {
        let mut best: Option<(f32, ObjectList, usize)> = None;
        let mut consider = |list: ObjectList, rows: &[ObjectRow]| {
            for (i, o) in rows.iter().enumerate() {
                let d = to_screen(rect, o.position).distance(pointer);
                if d < 8.0 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, list, i));
                }
            }
        };
        if layers.generated {
            consider(ObjectList::Generated, &p.generated);
        }
        if layers.statics {
            consider(ObjectList::Statics, &p.statics);
        }
        let x = (pointer.x - rect.left()) / side * SIDE;
        let y = (rect.bottom() - pointer.y) / side * SIDE;
        let (i, j) = ((x / 5.0).floor() as usize, (y / 5.0).floor() as usize);
        let h = p
            .heights
            .get(33 * j.min(32) + i.min(32))
            .copied()
            .unwrap_or(0.0);
        let mut text = format!("x {x:.1} · y {y:.1} · sample ({i}, {j}) height {h:.2}");
        if let Some((_, list, i)) = best {
            let rows = match list {
                ObjectList::Generated => &p.generated,
                ObjectList::Statics => &p.statics,
            };
            if let Some(o) = rows.get(i) {
                text.push('\n');
                text.push_str(&object_text(list, o));
            }
            if resp.clicked() {
                cur.world.selected = Some((list, i));
                cur.world.scroll_to_selected = true;
                cur.world.tab = match list {
                    ObjectList::Generated => LandblockTab::Generated,
                    ObjectList::Statics => LandblockTab::Statics,
                };
            }
            if resp.double_clicked()
                && let Some(obj) = rows.get(i).and_then(|o| o.object)
            {
                actions.push(Action::Open(obj));
            }
        }
        resp.on_hover_text_at_pointer(text);
    }
}

fn object_text(list: ObjectList, o: &ObjectRow) -> String {
    format!(
        "{} {} · {} · ({:.2}, {:.2}, {:.2}) · heading {:.1}° · scale {:.2}{}{}",
        match list {
            ObjectList::Generated => "generated",
            ObjectList::Statics => "static",
        },
        o.index,
        o.object
            .map_or_else(|| "no object".into(), |d| format!("{d:#010x}")),
        o.position[0],
        o.position[1],
        o.position[2],
        o.heading,
        o.scale,
        walker_text(o),
        o.lod_class
            .map_or_else(String::new, |c| format!(" · LOD class {c}")),
    )
}

/// The outline of objects whose collision a walker passes through.
const PASS_THROUGH: Color32 = Color32::from_rgb(190, 120, 255);

/// What an object does to a walker, for the map's tooltip.
fn walker_text(o: &ObjectRow) -> &'static str {
    if o.blocks {
        " · blocks a walker"
    } else if o.collides {
        " · collision a walker passes through (ethereal to Player)"
    } else {
        ""
    }
}

/// The walker column of the object table.
fn walker_cell(o: &ObjectRow) -> &'static str {
    if o.blocks {
        "blocks"
    } else if o.collides {
        "passes"
    } else {
        "—"
    }
}

fn kv_grid(ui: &mut Ui, id: &str, rows: &[(String, String)]) {
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (k, v) in rows {
                ui.label(RichText::new(k).strong());
                ui.label(RichText::new(v).monospace());
                ui.end_row();
            }
        });
}

fn landblock_tables(
    ui: &mut Ui,
    cur: &mut Current,
    p: &LandblockProduct,
    actions: &mut Vec<Action>,
) {
    ui.horizontal_wrapped(|ui| {
        for t in LandblockTab::ALL {
            let n = match t {
                LandblockTab::Generated => Some(p.generated.len()),
                LandblockTab::Statics => Some(p.statics.len()),
                LandblockTab::Cells => Some(p.cells.len()),
                LandblockTab::Problems => Some(p.problems.len()),
                _ => None,
            };
            let title = n.map_or_else(|| t.title().to_owned(), |n| format!("{} ({n})", t.title()));
            ui.selectable_value(&mut cur.world.tab, t, title);
        }
    });
    ui.separator();
    match cur.world.tab {
        LandblockTab::Generated => {
            object_table(ui, cur, ObjectList::Generated, &p.generated, actions)
        }
        LandblockTab::Statics => object_table(ui, cur, ObjectList::Statics, &p.statics, actions),
        LandblockTab::Cells => {
            let row_h = ui.text_style_height(&egui::TextStyle::Monospace).max(18.0);
            egui::ScrollArea::vertical()
                .id_salt("lb-cells")
                .auto_shrink([false, false])
                .show_rows(ui, row_h, p.cells.len(), |ui, range| {
                    for c in &p.cells[range] {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("cell {:>4}", c.index)).monospace());
                            if ui
                                .link(RichText::new(format!("{:#010x}", c.geometry)).monospace())
                                .clicked()
                            {
                                actions.push(Action::Open(c.geometry));
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{} · {} portals",
                                    c.position.map_or_else(
                                        || "not placed".into(),
                                        |q| format!("({:.2}, {:.2}, {:.2})", q[0], q[1], q[2])
                                    ),
                                    c.portals
                                ))
                                .monospace(),
                            );
                        });
                    }
                });
        }
        LandblockTab::Water => {
            kv_grid(ui, "lb-water", &p.water_rows);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("lb-liquids")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, b) in p.liquids.iter().enumerate() {
                        ui.label(
                            RichText::new(format!(
                                "box {i}: {:?} … {:?} {}",
                                b.bounds.0, b.bounds.1, b.flags
                            ))
                            .monospace(),
                        );
                    }
                });
        }
        LandblockTab::Collision => kv_grid(ui, "lb-collision", &p.collision),
        LandblockTab::Scenery => kv_grid(ui, "lb-scenery", &p.scenery),
        LandblockTab::Problems => {
            if p.problems.is_empty() {
                ui.label("no problems");
            }
            egui::ScrollArea::vertical()
                .id_salt("lb-problems")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for pr in &p.problems {
                        ui.label(RichText::new(pr).monospace());
                    }
                });
        }
    }
}

fn object_table(
    ui: &mut Ui,
    cur: &mut Current,
    list: ObjectList,
    rows: &[ObjectRow],
    actions: &mut Vec<Action>,
) {
    if rows.is_empty() {
        ui.label(match list {
            ObjectList::Generated => "no generated objects",
            ObjectList::Statics => "no static entities",
        });
        return;
    }
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace).max(18.0);
    ui.label(
        RichText::new("index · object · x y z · heading · scale · LOD class · walker · drawn")
            .strong(),
    )
    .on_hover_text(
        "walker: blocks (a collision body the walker collides with), passes (a body whose \
         Physics_EtherealToType contains Player, collision.md §9.2) or — (no body)",
    );
    let selected = cur.world.selected;
    let mut scroll_to = None;
    if let Some((l, i)) = selected
        && l == list
    {
        scroll_to = Some(i);
    }
    let mut area = egui::ScrollArea::vertical()
        .id_salt(("lb-objects", list == ObjectList::Generated))
        .auto_shrink([false, false]);
    // Only once, when the map selected the object: scrolling on every click would move
    // the rows under the pointer between hit-testing and the click.
    if let Some(i) = scroll_to
        && std::mem::take(&mut cur.world.scroll_to_selected)
    {
        area = area.vertical_scroll_offset(
            (i as f32 * (row_h + ui.spacing().item_spacing.y) - 80.0).max(0.0),
        );
    }
    area.show_rows(ui, row_h, rows.len(), |ui, range| {
        for i in range {
            let o = &rows[i];
            let is_sel = selected == Some((list, i));
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(is_sel, RichText::new(format!("{:>5}", o.index)).monospace())
                    .clicked()
                {
                    cur.world.selected = Some((list, i));
                }
                match o.object {
                    Some(d) => {
                        if ui
                            .link(RichText::new(format!("{d:#010x}")).monospace())
                            .clicked()
                        {
                            actions.push(Action::Open(d));
                        }
                    }
                    None => {
                        ui.label(RichText::new("—").monospace());
                    }
                }
                ui.label(
                    RichText::new(format!(
                        "{:>8.2} {:>8.2} {:>8.2} · {:>6.1}° · {:.2} · {} · {} · {}",
                        o.position[0],
                        o.position[1],
                        o.position[2],
                        o.heading,
                        o.scale,
                        o.lod_class.map_or_else(|| "—".into(), |c| c.to_string()),
                        walker_cell(o),
                        if o.drawn { "drawn" } else { "—" },
                    ))
                    .monospace(),
                );
            });
        }
    });
}

// ---------------------------------------------------------------------------------------
// Sky view
// ---------------------------------------------------------------------------------------

/// A colour of the sky values as an egui colour (values are 0–1 per channel).
fn rgb(c: [f32; 3]) -> Color32 {
    let b = |v: f32| {
        (if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        } * 255.0)
            .round() as u8
    };
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

/// Time of day as `hh:mm` of a 24-hour scale.
#[must_use]
pub fn clock_text(t: f32) -> String {
    let m = (t.rem_euclid(1.0) * 1440.0).floor() as u32;
    format!("{:02}:{:02}", (m / 60) % 24, m % 60)
}

/// Draws the sky view.
pub(crate) fn sky_view(ui: &mut Ui, l: &Loaded, cur: &mut Current, actions: &mut Vec<Action>) {
    let did = cur.nav.did;
    ensure(&mut cur.world.sky, &l.stores.sky, AssetKey::new(did, 0));
    let Some(p) = ready(ui, cur.world.sky.as_ref()) else {
        return;
    };
    let timeline = match &*p {
        SkyProduct::Day { day, timeline } => {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("day description").strong());
                ui.label(format!(
                    "{} keyframes · {} sky objects · transition {} s",
                    day.keyframes.len(),
                    day.objects.len(),
                    day.transition_seconds
                ));
                // The timeline keeps the first MAX_KEYFRAMES keyframes with a finite time;
                // say how many it leaves out, as the other captions do.
                let left_out = day.keyframes.len().saturating_sub(timeline.keyframes.len());
                if left_out > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{left_out} keyframes left out of the timeline (no finite time, \
                             or past the first {})",
                            crate::timeline::MAX_KEYFRAMES
                        ))
                        .color(Color32::GRAY),
                    );
                }
            });
            Arc::clone(timeline)
        }
        SkyProduct::Sky {
            library,
            keys,
            day_types,
            days,
        } => {
            let w = &mut cur.world;
            if w.sky_key.is_none_or(|k| keys.binary_search(&k).is_err()) {
                w.sky_key = initial_sky_key(library.sky(), w.day_type, keys);
            }
            let mut day_number = *w
                .day_number
                .get_or_insert_with(|| library.calendar().read(0.0).day_number);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("sky description").strong());
                ui.label(format!(
                    "{} sky keys · {} day types · {days} day descriptions · day length {} s · \
                     {} days a year",
                    keys.len(),
                    day_types.len(),
                    library.calendar().day_length,
                    library.calendar().days_per_year
                ));
                if !library.sky().game_time.is_null()
                    && ui
                        .small_button(format!("game time {:#010x}", library.sky().game_time.0))
                        .clicked()
                {
                    actions.push(Action::Open(library.sky().game_time.0));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("sky key");
                egui::ComboBox::from_id_salt("sky-key")
                    .selected_text(w.sky_key.map_or_else(|| "—".into(), |k| format!("{k:#x}")))
                    .show_ui(ui, |ui| {
                        for k in keys {
                            ui.selectable_value(&mut w.sky_key, Some(*k), format!("{k:#x}"));
                        }
                    });
                ui.label("day type");
                egui::ComboBox::from_id_salt("sky-day-type")
                    .selected_text(w.day_type.to_string())
                    .show_ui(ui, |ui| {
                        for t in day_types {
                            ui.selectable_value(&mut w.day_type, *t, t.to_string());
                        }
                    });
                ui.label("game day");
                let max_day = i64::from(u32::MAX);
                if ui.small_button("◀").clicked() {
                    day_number = day_number.saturating_sub(1).clamp(0, max_day);
                }
                ui.add(egui::DragValue::new(&mut day_number).range(0..=max_day));
                if ui.small_button("▶").clicked() {
                    day_number = day_number.saturating_add(1).clamp(0, max_day);
                }
            });
            w.day_number = Some(day_number);
            let Some(sky_key) = w.sky_key else {
                ui.label("the sky description lists no sky key");
                return;
            };
            // The choices of this and the following days (§11.2; pure, cached).
            let key = ChoicesKey {
                did,
                sky_key,
                day_type: w.day_type,
                day_number,
            };
            let choices = match &w.choices {
                Some((k, c)) if *k == key => Arc::clone(c),
                _ => {
                    let c: DayChoices = crate::timeline::day_choices(
                        library.sky(),
                        sky_key,
                        w.day_type,
                        day_number,
                        LISTED_DAYS,
                    )
                    .into();
                    w.choices = Some((key, Arc::clone(&c)));
                    c
                }
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("days").strong());
                for (n, c) in choices.iter() {
                    let text = c.map_or_else(
                        || format!("{n}: none"),
                        |c| format!("{n}: {:#010x}", c.day.0),
                    );
                    if ui.small_button(text).clicked() {
                        w.day_number = Some(*n);
                    }
                }
            });
            let choice = choices.first().and_then(|(_, c)| *c);
            match chosen_timeline(ui, l, w, choice, actions) {
                Some(t) => t,
                None => return,
            }
        }
    };
    cur.world.timeline_shown = true;
    ui.separator();
    timeline_widget(ui, &mut cur.world.time, &timeline);
    ui.separator();
    values_at(ui, &timeline, cur.world.time, actions);
}

/// The timeline of the day description `choice` names (requested from the sky store,
/// computed on a worker), with a line that says which it is. Without a choice, or when the
/// record is not a usable day description, the defaults of `environment.md` §11.3 step 8
/// (as the walker evaluates an unusable one). `None` while the day description loads.
fn chosen_timeline(
    ui: &mut Ui,
    l: &Loaded,
    w: &mut WorldViewState,
    choice: Option<DayChoice>,
    actions: &mut Vec<Action>,
) -> Option<Arc<Timeline>> {
    let source = |c: &DayChoice| match c.source {
        ChoiceSource::DayGroup => "day group",
        ChoiceSource::LocalDefault => "local default",
        ChoiceSource::GroupFallback => "fallback: another day group (Tessera Decision)",
    };
    let mut defaults = || {
        Arc::clone(
            w.defaults
                .get_or_insert_with(|| Arc::new(defaults_timeline(TIMELINE_SAMPLES))),
        )
    };
    let Some(c) = choice else {
        ui.label("no day description: the defaults of environment.md §11.3 step 8");
        return Some(defaults());
    };
    let mut unusable = |ui: &mut Ui, why: String| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("day description ({})", source(&c)));
            if ui.link(format!("{:#010x}", c.day.0)).clicked() {
                actions.push(Action::Open(c.day.0));
            }
            ui.label(format!(
                "unusable ({why}): the defaults of environment.md §11.3 step 8"
            ));
        });
    };
    if c.day.prefix() != DAY_DESCRIPTION_PREFIX {
        unusable(ui, "not a day description".into());
        return Some(defaults());
    }
    ensure(&mut w.day, &l.stores.sky, AssetKey::new(c.day.0, 0));
    let state = w.day.as_ref().map(Handle::state)?;
    match state {
        AssetState::Ready(p) => match &*p {
            SkyProduct::Day { timeline, .. } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("day description ({})", source(&c)));
                    if ui.link(format!("{:#010x}", c.day.0)).clicked() {
                        actions.push(Action::Open(c.day.0));
                    }
                });
                Some(Arc::clone(timeline))
            }
            SkyProduct::Sky { .. } => {
                unusable(ui, "not a day description".into());
                Some(defaults())
            }
        },
        AssetState::Failed(e) => {
            unusable(ui, e.to_string());
            Some(defaults())
        }
        AssetState::Cancelled => {
            // Request it again on the next frame.
            w.day = None;
            ui.ctx().request_repaint();
            None
        }
        AssetState::Queued | AssetState::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("computing the timeline of {:#010x} …", c.day.0));
            });
            None
        }
    }
}

/// Colour bands of the timeline: (label, colour of a sample).
type Band = (&'static str, fn(&SkyValues) -> [f32; 3]);
/// Curves of the timeline: (label, value, colour).
type Curve = (&'static str, fn(&SkyValues) -> f32, Color32);

const BANDS: [Band; 6] = [
    ("fog colour", |v| v.fog_colour),
    ("sun colour", |v| v.sun_colour),
    ("sun specular", |v| v.sun_specular),
    ("ambient 1", |v| v.ambient_colour_1),
    ("ambient 2", |v| v.ambient_colour_2),
    ("colour 3", |v| v.colour_3),
];

const CURVES: [Curve; 6] = [
    (
        "sun brightness",
        |v| v.sun_brightness,
        Color32::from_rgb(250, 210, 80),
    ),
    (
        "ambient brightness",
        |v| v.ambient_brightness,
        Color32::from_rgb(140, 200, 255),
    ),
    (
        "sun pitch",
        |v| v.sun_pitch,
        Color32::from_rgb(255, 120, 90),
    ),
    (
        "sun heading",
        |v| v.sun_heading,
        Color32::from_rgb(200, 120, 255),
    ),
    ("fog near", |v| v.fog_near, Color32::from_rgb(160, 160, 160)),
    ("fog far", |v| v.fog_far, Color32::from_rgb(230, 230, 230)),
];

fn timeline_widget(ui: &mut Ui, time: &mut f32, t: &Timeline) {
    let n = t.values.len();
    if n == 0 {
        return;
    }
    ui.horizontal(|ui| {
        ui.label("time of day");
        ui.add(egui::Slider::new(time, 0.0..=0.9999).fixed_decimals(4));
        ui.label(clock_text(*time));
    });
    let label_w = 120.0;
    let width = (ui.available_width() - label_w - 8.0).max(120.0);
    let band_h = 14.0;
    let curve_h = 110.0;
    let objects = t.objects.len().min(24);
    let height = BANDS.len() as f32 * (band_h + 2.0) + curve_h + 22.0 + objects as f32 * 11.0 + 8.0;
    let (outer, resp) =
        ui.allocate_exact_size(vec2(label_w + width, height), Sense::click_and_drag());
    let painter = ui.painter_at(outer);
    let left = outer.left() + label_w;
    let x_of = |f: f32| left + f * width;
    let font = egui::FontId::proportional(11.0);
    let mut y = outer.top();
    let col_w = width / n as f32;
    for (label, colour) in BANDS {
        painter.text(
            pos2(outer.left(), y),
            egui::Align2::LEFT_TOP,
            label,
            font.clone(),
            Color32::GRAY,
        );
        for (k, v) in t.values.iter().enumerate() {
            let x = left + k as f32 * col_w;
            painter.rect_filled(
                Rect::from_min_size(pos2(x, y), vec2(col_w + 0.5, band_h)),
                0.0,
                rgb(colour(v)),
            );
        }
        y += band_h + 2.0;
    }
    // Curves, each normalised to its own range over the day.
    let plot = Rect::from_min_size(pos2(left, y + 2.0), vec2(width, curve_h));
    painter.rect_stroke(
        plot,
        0.0,
        Stroke::new(1.0, Color32::from_gray(60)),
        egui::StrokeKind::Inside,
    );
    let mut ly = y + 2.0;
    for (label, value, colour) in CURVES {
        let vals: Vec<f32> = t.values.iter().map(value).collect();
        let (lo, hi) = vals
            .iter()
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        if lo > hi {
            continue;
        }
        let span = (hi - lo).max(1e-6);
        let pts: Vec<egui::Pos2> = vals
            .iter()
            .enumerate()
            .map(|(k, v)| {
                let f = if v.is_finite() { (v - lo) / span } else { 0.0 };
                pos2(
                    left + (k as f32 + 0.5) * col_w,
                    plot.bottom() - 3.0 - f * (curve_h - 6.0),
                )
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.5, colour)));
        painter.text(
            pos2(outer.left(), ly),
            egui::Align2::LEFT_TOP,
            format!("{label} {lo:.2}…{hi:.2}"),
            egui::FontId::proportional(10.0),
            colour,
        );
        ly += 13.0;
    }
    y = plot.bottom() + 2.0;
    // Keyframe ticks and hour marks.
    for h in 0..=24 {
        let x = x_of(h as f32 / 24.0);
        let long = h % 6 == 0;
        painter.vline(
            x,
            y..=y + if long { 8.0 } else { 4.0 },
            Stroke::new(1.0, Color32::from_gray(90)),
        );
        if long && h < 24 {
            painter.text(
                pos2(x + 2.0, y + 8.0),
                egui::Align2::LEFT_TOP,
                format!("{h:02}"),
                egui::FontId::proportional(9.0),
                Color32::from_gray(140),
            );
        }
    }
    for kt in &t.keyframes {
        if kt.is_finite() {
            let x = x_of(kt.clamp(0.0, 1.0));
            painter.vline(
                x,
                outer.top()..=y + 10.0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
            );
        }
    }
    painter.text(
        pos2(outer.left(), y),
        egui::Align2::LEFT_TOP,
        "keyframes / hours",
        font.clone(),
        Color32::GRAY,
    );
    y += 24.0;
    for o in t.objects.iter().take(objects) {
        for (k, vis) in o.visible.iter().enumerate() {
            if *vis {
                painter.rect_filled(
                    Rect::from_min_size(pos2(left + k as f32 * col_w, y), vec2(col_w + 0.5, 6.0)),
                    0.0,
                    Color32::from_rgb(120, 170, 230),
                );
            }
        }
        painter.text(
            pos2(outer.left(), y - 2.0),
            egui::Align2::LEFT_TOP,
            format!("object {}", o.index),
            egui::FontId::proportional(9.0),
            Color32::GRAY,
        );
        y += 11.0;
    }
    // Cursor.
    let cx = x_of(time.clamp(0.0, 1.0));
    painter.vline(cx, outer.y_range(), Stroke::new(1.5, Color32::YELLOW));
    if let Some(pos) = resp.interact_pointer_pos()
        && (resp.clicked() || resp.dragged())
        && pos.x >= left
    {
        *time = ((pos.x - left) / width).clamp(0.0, 0.9999);
    }
    if let Some(pos) = resp.hover_pos()
        && pos.x >= left
    {
        let f = ((pos.x - left) / width).clamp(0.0, 0.9999);
        resp.on_hover_text_at_pointer(format!("{f:.4} ({})", clock_text(f)));
    }
}

fn values_at(ui: &mut Ui, t: &Timeline, time: f32, actions: &mut Vec<Action>) {
    let Some(k) = t.sample_at(time) else { return };
    let v = &t.values[k];
    let light = v.lighting();
    let f3 = |c: [f32; 3]| format!("{:.3} {:.3} {:.3}", c[0], c[1], c[2]);
    let rows: Vec<(String, String)> = vec![
        (
            "sample".into(),
            format!("{:.4} ({})", t.times[k], clock_text(t.times[k])),
        ),
        (
            "fog".into(),
            format!(
                "{:.1} … {:.1} (extra {:.3})",
                v.fog_near, v.fog_far, v.fog_extra
            ),
        ),
        ("fog colour".into(), f3(v.fog_colour)),
        (
            "sun".into(),
            format!(
                "brightness {:.3} · heading {:.2}° · pitch {:.2}°",
                v.sun_brightness, v.sun_heading, v.sun_pitch
            ),
        ),
        ("toward sun".into(), f3(v.sun_toward)),
        (
            "sun colour / specular".into(),
            format!("{} / {}", f3(v.sun_colour), f3(v.sun_specular)),
        ),
        (
            "ambient".into(),
            format!(
                "{:.3} · {} / {}",
                v.ambient_brightness,
                f3(v.ambient_colour_1),
                f3(v.ambient_colour_2)
            ),
        ),
        (
            "colour 3 / colour 4".into(),
            format!("{} / {:?}", f3(v.colour_3), v.colour_4),
        ),
        (
            "values a / b".into(),
            format!("{:?} / {:.3}", v.values_a, v.value_b),
        ),
        (
            "light: ambient up / down".into(),
            format!("{} / {}", f3(light.ambient_up), f3(light.ambient_down)),
        ),
        ("light: main colour".into(), f3(light.sun_colour)),
        ("light: travel".into(), f3(light.sun_travel)),
    ];
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| kv_grid(ui, "sky-values", &rows));
        ui.separator();
        ui.vertical(|ui| {
            if let Some(e) = v.effect {
                ui.horizontal(|ui| {
                    ui.label("effect");
                    if ui.link(format!("{:#010x}", e.object.0)).clicked() {
                        actions.push(Action::Open(e.object.0));
                    }
                });
            }
            ui.label(RichText::new("sky objects").strong());
            for o in t.objects.iter().take(64) {
                let vis = o.visible.get(k).copied().unwrap_or(false);
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} {}",
                        o.index,
                        if vis { "visible" } else { "hidden" }
                    ));
                    if !o.object.is_null() && ui.link(format!("{:#010x}", o.object.0)).clicked() {
                        actions.push(Action::Open(o.object.0));
                    }
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_colours_span_the_range() {
        let lo = height_colour(0.0, (0.0, 100.0));
        let hi = height_colour(100.0, (0.0, 100.0));
        assert_ne!(lo, hi);
        assert_eq!(height_colour(f32::NAN, (0.0, 100.0)), lo);
        assert_eq!(height_colour(-5.0, (0.0, 100.0)), lo);
        // A flat landblock does not divide by zero.
        let _ = height_colour(3.0, (3.0, 3.0));
    }

    #[test]
    fn map_images_tint_water_and_put_north_up() {
        // Heights rise to the north (j).
        let heights: Vec<f32> = (0..33 * 33).map(|k| (k / 33) as f32).collect();
        let img = map_image(&heights, (0.0, 32.0), Some(10.0));
        assert_eq!(img.size, [32, 32]);
        // The bottom row (south, low) is under water: blue dominates.
        let south = img.pixels[31 * 32];
        assert!(south.b() > south.g() && south.b() > south.r());
        // The top row (north, high) is above the water: its plain height colour.
        assert_eq!(img.pixels[0], height_colour(31.5, (0.0, 32.0)));
        // Short height lists are tolerated.
        let small = map_image(&[1.0, 2.0], (0.0, 1.0), None);
        assert_eq!(small.pixels.len(), 32 * 32);
    }

    #[test]
    fn clock_texts() {
        assert_eq!(clock_text(0.0), "00:00");
        assert_eq!(clock_text(0.5), "12:00");
        assert_eq!(clock_text(0.75), "18:00");
        assert_eq!(clock_text(1.25), "06:00");
    }

    #[test]
    fn screen_mapping_is_north_up() {
        let r = Rect::from_min_size(pos2(10.0, 20.0), vec2(160.0, 160.0));
        assert_eq!(to_screen(r, [0.0, 0.0, 0.0]), pos2(10.0, 180.0));
        assert_eq!(to_screen(r, [160.0, 160.0, 0.0]), pos2(170.0, 20.0));
    }
}
