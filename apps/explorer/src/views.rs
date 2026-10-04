// SPDX-License-Identifier: GPL-3.0-or-later
//! The central panel: the selected DID's header, view tabs and the views themselves.
//!
//! Views only request products from the asset stores and draw what is ready; they never
//! read or decode records.

use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, RichText, TextureOptions, Ui, pos2, vec2};
use tessera_assets::{AssetKey, AssetState, AssetStore, Decoder, Handle, Priority};
use tessera_formats::{Did, LandblockId, SurfaceFormat};
use tessera_text::{Message, Values};

use crate::app::{Current, ExplorerApp, Loaded, Nav, texture_key};
use crate::cli::ViewKind;
use crate::decoders::{
    AUTO_LEVEL, ContextSource, PropsProduct, StringsProduct, TextureProduct, render_entry,
};
use crate::hex::{BYTES_PER_LINE, hex_line, human_bytes, line_count, thousands};
use crate::image_ops::ChannelMask;
use crate::richtext::{Palette, layout_job};

/// Something a view asks the app to do after drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Open(u32),
    Go(Nav),
    View(ViewKind),
    ToggleBookmark(u32),
    Reveal,
}

/// Requests `key` from `store` into `slot` unless the slot already holds that key.
pub(crate) fn ensure<D: Decoder>(
    slot: &mut Option<Handle<D::Output>>,
    store: &AssetStore<ContextSource, D>,
    key: AssetKey,
) {
    if slot.as_ref().is_some_and(|h| h.key() == key) {
        return;
    }
    if let Some(old) = slot.take() {
        old.cancel();
    }
    *slot = Some(store.request(key, Priority::Immediate));
}

/// Shows a loading spinner or the failure of a handle; returns the product when ready.
fn ready<T>(ui: &mut Ui, h: Option<&Handle<T>>) -> Option<Arc<T>> {
    let h = h?;
    match h.state() {
        AssetState::Ready(p) => Some(p),
        AssetState::Failed(e) => {
            ui.colored_label(Color32::LIGHT_RED, e.to_string());
            None
        }
        AssetState::Cancelled => {
            ui.label(RichText::new("cancelled").weak());
            None
        }
        AssetState::Queued | AssetState::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("loading …");
            });
            None
        }
    }
}

/// Draws the central panel.
pub(crate) fn central(app: &mut ExplorerApp, ui: &mut Ui) {
    let checker = app.checker(ui.ctx());
    let mut actions = Vec::new();
    let mut cur = app.current.take();
    let mut settings = app.mesh_settings;
    let wants_viewport = cur.as_ref().is_some_and(|c| c.nav.view == ViewKind::Mesh);
    if wants_viewport && app.viewport.is_none() {
        let ctx = ui.ctx().clone();
        app.viewport = Some(crate::viewport::Viewport::spawn(move || {
            ctx.request_repaint()
        }));
    }
    if !wants_viewport && let Some(vp) = &mut app.viewport {
        // Another view (or no install): the render thread releases the scene's GPU
        // meshes and textures and its reference to the scene, which is outside the asset
        // cache's budget.
        vp.clear_scene();
    }
    let mut viewport = app.viewport.take();
    let bookmarked = cur
        .as_ref()
        .is_some_and(|c| app.persist.is_bookmarked(c.nav.did));
    egui::CentralPanel::default().show(ui, |ui| {
        let (Some(cur), Some(l)) = (cur.as_mut(), app.loaded()) else {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.heading("Select a DID");
                ui.label(concat!(
                    "Pick a type group in the archive tree, search a DID or a name, ",
                    "or open the root DID map."
                ));
                if ui.button("Open the root DID map (0x28000000)").clicked() {
                    actions.push(Action::Open(tessera_props::ROOT_DID_MAP.0));
                }
            });
            return;
        };
        header(ui, l, cur, bookmarked, &mut actions);
        ui.separator();
        match cur.nav.view {
            ViewKind::Info => info_view(ui, l, cur, &mut actions),
            ViewKind::Hex => hex_view(ui, l, cur),
            ViewKind::Texture => texture_view(ui, l, cur, &checker),
            ViewKind::Strings => strings_view(ui, l, cur),
            ViewKind::Props => props_view(ui, l, cur, &mut actions),
            ViewKind::Icon => icon_view(ui, l, cur, &checker, &mut actions),
            ViewKind::Record => record_view(ui, l, cur, &mut actions),
            ViewKind::Mesh => match viewport.as_mut() {
                Some(vp) => crate::mesh_view::mesh_view(
                    ui,
                    l,
                    cur,
                    &mut settings,
                    vp,
                    &checker,
                    &mut actions,
                ),
                None => {
                    ui.label("the render thread is not running");
                }
            },
        }
    });
    app.current = cur;
    app.viewport = viewport;
    app.mesh_settings = settings;
    for a in actions {
        match a {
            Action::Open(did) => app.open_did(did),
            Action::Go(nav) => app.navigate(nav),
            Action::View(v) => {
                if let Some(c) = &app.current {
                    let nav = Nav { view: v, ..c.nav };
                    app.navigate(nav);
                }
            }
            Action::Reveal => {
                if !app.reveal_current() {
                    app.status = Some("not in an archive index (yet)".into());
                }
            }
            Action::ToggleBookmark(did) => {
                let label = Did(did).kind().name().to_owned();
                app.persist.toggle_bookmark(did, &label);
            }
        }
    }
}

/// The archives (by index) that hold `did`.
fn holders(l: &Loaded, did: u32) -> Vec<usize> {
    l.indexes
        .iter()
        .flatten()
        .filter(|i| i.find(did).is_some())
        .map(|i| i.member)
        .collect()
}

fn header(ui: &mut Ui, l: &Loaded, cur: &Current, bookmarked: bool, actions: &mut Vec<Action>) {
    let did = cur.nav.did;
    let kind = Did(did).kind();
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("{did:#010x}"))
                .monospace()
                .strong()
                .size(18.0),
        );
        ui.label(RichText::new(kind.name()).size(16.0));
        ui.label(RichText::new(format!("({:?})", kind.evidence())).weak());
        let star = if bookmarked { "★" } else { "☆" };
        if ui
            .button(star)
            .on_hover_text(if bookmarked {
                "Remove bookmark"
            } else {
                "Bookmark"
            })
            .clicked()
        {
            actions.push(Action::ToggleBookmark(did));
        }
        let held: Vec<String> = holders(l, did)
            .into_iter()
            .map(|m| l.ctx.member_name(m))
            .collect();
        let from = match cur.nav.member {
            Some(m) => format!("from {}", l.ctx.member_name(m)),
            None => "routed".to_owned(),
        };
        if ui.small_button("show in tree").clicked() {
            actions.push(Action::Reveal);
        }
        ui.label(
            RichText::new(format!(
                "{from} · in {}",
                if held.is_empty() {
                    "?".to_owned()
                } else {
                    held.join(", ")
                }
            ))
            .weak(),
        );
    });
    ui.horizontal_wrapped(|ui| {
        for v in ViewKind::ALL {
            let on = cur.nav.view == v;
            let r = ui.add_enabled(v.applies_to(did), egui::Button::selectable(on, v.title()));
            if r.clicked() && !on {
                actions.push(Action::View(v));
            }
        }
    });
}

// ---------------------------------------------------------------------------------------
// Info
// ---------------------------------------------------------------------------------------

fn info_view(ui: &mut Ui, l: &Loaded, cur: &mut Current, actions: &mut Vec<Action>) {
    let did = cur.nav.did;
    let variant = cur.nav.member.map_or(0, |m| m as u32 + 1);
    ensure(&mut cur.raw, &l.stores.raw, AssetKey::new(did, variant));
    let d = Did(did);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("info-grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("DID");
                    ui.monospace(format!(
                        "{did:#010x}  (prefix {:#04x}, serial {:#08x})",
                        d.prefix(),
                        d.serial()
                    ));
                    ui.end_row();
                    ui.label("Type");
                    ui.label(format!("{} — {:?}", d.kind().name(), d.kind().evidence()));
                    ui.end_row();
                    if let Some(lb) = LandblockId::from_did(d) {
                        ui.label("Landblock");
                        ui.label(format!(
                            "{:?} region {} x {} y {}",
                            lb.kind, lb.region, lb.x, lb.y
                        ));
                        ui.end_row();
                    }
                    ui.label("Indexed in");
                    ui.horizontal_wrapped(|ui| {
                        let held = holders(l, did);
                        if held.is_empty() {
                            ui.label(RichText::new("no index lists it (yet)").weak());
                        }
                        for m in held {
                            let text = l.ctx.member_name(m);
                            if ui
                                .link(text)
                                .on_hover_text("Read from this archive")
                                .clicked()
                            {
                                actions.push(Action::Go(Nav {
                                    did,
                                    member: Some(m),
                                    view: cur.nav.view,
                                }));
                            }
                        }
                        if cur.nav.member.is_some() && ui.link("(routed)").clicked() {
                            actions.push(Action::Go(Nav {
                                did,
                                member: None,
                                view: cur.nav.view,
                            }));
                        }
                    });
                    ui.end_row();
                    match did >> 24 {
                        0x70..=0x77 => {
                            if let Some(set) = tessera_formats::object_property_set_did(d) {
                                ui.label("Property set");
                                if ui.link(format!("{:#010x}", set.0)).clicked() {
                                    actions.push(Action::Open(set.0));
                                }
                                ui.end_row();
                            }
                        }
                        0x79..=0x7F => {
                            let obj = did.wrapping_sub(0x0900_0000);
                            ui.label("Object");
                            if ui.link(format!("{obj:#010x}")).clicked() {
                                actions.push(Action::Open(obj));
                            }
                            ui.end_row();
                        }
                        _ => {}
                    }
                });
            ui.add_space(8.0);
            let Some(raw) = ready(ui, cur.raw.as_ref()) else {
                return;
            };
            egui::Grid::new("entry-grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    let e = &raw.entry;
                    let rows: Vec<(&str, String)> = vec![
                        (
                            "Archive",
                            format!(
                                "{}{}",
                                raw.archive,
                                if raw.routed { " (routed)" } else { "" }
                            ),
                        ),
                        ("Routing candidates", raw.routing.join(", ")),
                        (
                            "Stored length",
                            format!(
                                "{} ({})",
                                thousands(u64::from(e.length)),
                                human_bytes(u64::from(e.length))
                            ),
                        ),
                        (
                            "Record size",
                            format!(
                                "{} ({})",
                                thousands(raw.bytes.len() as u64),
                                human_bytes(raw.bytes.len() as u64)
                            ),
                        ),
                        ("Compressed", e.is_compressed().to_string()),
                        ("Flags", format!("{:#06x}", e.flags)),
                        ("Stamp", format!("{:#06x}", e.stamp)),
                        ("Offset", format!("{:#010x}", e.offset)),
                        ("Allocation", thousands(u64::from(e.alloc_size))),
                        ("Time", e.time.to_string()),
                        ("Iteration", e.iteration.to_string()),
                    ];
                    for (k, v) in rows {
                        ui.label(k);
                        ui.monospace(v);
                        ui.end_row();
                    }
                });
        });
}

// ---------------------------------------------------------------------------------------
// Hex
// ---------------------------------------------------------------------------------------

fn hex_view(ui: &mut Ui, l: &Loaded, cur: &mut Current) {
    let variant = cur.nav.member.map_or(0, |m| m as u32 + 1);
    ensure(
        &mut cur.raw,
        &l.stores.raw,
        AssetKey::new(cur.nav.did, variant),
    );
    let Some(raw) = ready(ui, cur.raw.as_ref()) else {
        return;
    };
    ui.label(format!(
        "{} bytes from {} · {} lines of {BYTES_PER_LINE}",
        thousands(raw.bytes.len() as u64),
        raw.archive,
        thousands(line_count(raw.bytes.len()) as u64)
    ));
    let h = ui.text_style_height(&egui::TextStyle::Monospace);
    egui::ScrollArea::both()
        .id_salt("hex-scroll")
        .auto_shrink([false, false])
        .show_rows(ui, h, line_count(raw.bytes.len()), |ui, range| {
            for line in range {
                ui.label(RichText::new(hex_line(&raw.bytes, line)).monospace());
            }
        });
}

// ---------------------------------------------------------------------------------------
// Textures
// ---------------------------------------------------------------------------------------

fn level_text(p: &TextureProduct, i: usize) -> String {
    let Some(l) = p.levels.get(i) else {
        return "?".into();
    };
    let what = if p.list.as_ref().is_some_and(|l| l.cube) {
        "face"
    } else {
        "level"
    };
    match &l.header {
        Some(h) => format!(
            "{what} {i}: {}×{} {} · {}",
            h.width,
            h.height,
            h.format.name(),
            l.archive.as_deref().unwrap_or("?")
        ),
        None => format!(
            "{what} {i}: {:#010x} — {}",
            l.did,
            l.error.as_deref().unwrap_or("missing")
        ),
    }
}

fn texture_view(ui: &mut Ui, l: &Loaded, cur: &mut Current, checker: &egui::TextureHandle) {
    let did = cur.nav.did;
    ensure(
        &mut cur.texture,
        &l.stores.texture,
        AssetKey::new(did, cur.tex_req.variant()),
    );
    // Pick up a finished product.
    if let Some(h) = &cur.texture {
        match h.state() {
            AssetState::Ready(p) => {
                // An A8 surface opens as greyscale (`texture-list.md` §7.3).
                let mut req = cur.tex_req;
                if cur.mask_auto {
                    cur.mask_auto = false;
                    let a8 = p
                        .shown
                        .and_then(|s| p.levels[s].header)
                        .is_some_and(|h| h.format == SurfaceFormat::A8);
                    if a8 && req.mask == ChannelMask::RGBA {
                        req.mask = ChannelMask::ALPHA;
                    }
                }
                if req != cur.tex_req {
                    cur.tex_req = req;
                    ensure(
                        &mut cur.texture,
                        &l.stores.texture,
                        AssetKey::new(did, req.variant()),
                    );
                } else if cur
                    .texture_shown
                    .as_ref()
                    .is_none_or(|s| !Arc::ptr_eq(s, &p))
                {
                    // The GPU cannot hold an empty texture (decoders never produce one).
                    if let Some(img) = p.image.as_ref().filter(|i| has_area(i)) {
                        let key = texture_key(&p);
                        if !cur.gpu.contains_key(&key) {
                            // Keep only the texture on screen plus the new one.
                            let keep = cur.texture_shown.as_ref().map(|s| texture_key(s));
                            cur.gpu
                                .retain(|k, _| Some(k) == keep.as_ref() || k.starts_with("icon/"));
                            let tex = ui.ctx().load_texture(
                                key.clone(),
                                egui::ImageData::Color(Arc::clone(img)),
                                TextureOptions {
                                    magnification: egui::TextureFilter::Nearest,
                                    minification: egui::TextureFilter::Linear,
                                    ..TextureOptions::default()
                                },
                            );
                            cur.gpu.insert(key, tex);
                        }
                    }
                    cur.texture_shown = Some(p);
                }
            }
            AssetState::Failed(e) => {
                ui.colored_label(Color32::LIGHT_RED, e.to_string());
                return;
            }
            _ => {}
        }
    }
    let Some(p) = cur.texture_shown.clone() else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("decoding …");
        });
        return;
    };
    let loading = cur.texture.as_ref().is_some_and(|h| !h.is_finished());

    // Toolbar: level, channels, zoom.
    let mut req = cur.tex_req;
    ui.horizontal_wrapped(|ui| {
        if p.levels.len() > 1 {
            let selected = p
                .shown
                .map_or_else(|| "none".to_owned(), |s| level_text(&p, s));
            egui::ComboBox::from_id_salt("level")
                .selected_text(selected)
                .width(320.0)
                .show_ui(ui, |ui| {
                    for i in 0..p.levels.len() {
                        let usable = p.levels[i].header.is_some();
                        let r = ui.add_enabled(
                            usable,
                            egui::Button::selectable(p.shown == Some(i), level_text(&p, i)),
                        );
                        if r.clicked() {
                            req.level = u8::try_from(i).unwrap_or(AUTO_LEVEL);
                        }
                    }
                });
        }
        ui.separator();
        ui.label("Channels");
        let mut m = req.mask;
        ui.toggle_value(
            &mut m.r,
            RichText::new("R").color(Color32::from_rgb(0xFF, 0x60, 0x60)),
        );
        ui.toggle_value(
            &mut m.g,
            RichText::new("G").color(Color32::from_rgb(0x60, 0xFF, 0x60)),
        );
        ui.toggle_value(
            &mut m.b,
            RichText::new("B").color(Color32::from_rgb(0x80, 0x80, 0xFF)),
        );
        ui.toggle_value(&mut m.a, "A");
        if ui.small_button("RGBA").clicked() {
            m = ChannelMask::RGBA;
        }
        if ui.small_button("A as grey").clicked() {
            m = ChannelMask::ALPHA;
        }
        req.mask = m;
        ui.separator();
        ui.checkbox(&mut cur.fit, "Fit");
        let mut zoom = cur.zoom;
        if ui
            .add(
                egui::Slider::new(&mut zoom, 0.0625..=16.0)
                    .logarithmic(true)
                    .text("zoom"),
            )
            .changed()
        {
            cur.fit = false;
        }
        cur.zoom = zoom;
        if ui.small_button("1:1").clicked() {
            cur.zoom = 1.0;
            cur.fit = false;
        }
        if loading {
            ui.spinner();
        }
    });
    if req != cur.tex_req {
        cur.tex_req = req;
        cur.mask_auto = false;
    }
    // Info line.
    if let Some(s) = p.shown
        && let Some(h) = p.levels[s].header
    {
        let mut info = format!(
            "{} · {}×{} · category {} · {} bytes stored",
            h.format.name(),
            h.width,
            h.height,
            h.category,
            thousands(h.data_len as u64)
        );
        if let Some(list) = &p.list {
            info.push_str(&format!(
                " · {} of {} levels, effective {:?}{}",
                list.kind,
                list.level_count,
                p.effective,
                if list.unusual {
                    " · unusual fields"
                } else {
                    ""
                }
            ));
        }
        if p.shrink > 1 {
            info.push_str(&format!(" · shown at 1/{} (GPU size limit)", p.shrink));
        }
        ui.label(RichText::new(info).weak());
        if let Some(st) = p.stats {
            ui.label(
                RichText::new(format!(
                    "min/max  R {}–{}  G {}–{}  B {}–{}  A {}–{}{}",
                    st.min[0],
                    st.max[0],
                    st.min[1],
                    st.max[1],
                    st.min[2],
                    st.max[2],
                    st.min[3],
                    st.max[3],
                    if st.opaque() { " (opaque)" } else { "" }
                ))
                .monospace()
                .small(),
            );
        }
    }
    if let Some(e) = &p.error {
        ui.colored_label(Color32::LIGHT_RED, e);
    }
    let Some(img) = &p.image else { return };
    let key = texture_key(&p);
    let Some(tex) = cur
        .gpu
        .get(&key)
        .or_else(|| cur.gpu.values().find(|t| t.name().starts_with("tex/")))
    else {
        return;
    };
    let size = vec2(img.size[0] as f32, img.size[1] as f32) * p.shrink as f32;
    let avail = ui.available_size();
    if cur.fit {
        let s = (avail.x / size.x.max(1.0))
            .min(avail.y / size.y.max(1.0))
            .clamp(0.0625, 16.0);
        cur.zoom = s;
    }
    // Ctrl + wheel zooms.
    let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
    let shown = size * cur.zoom;
    let tex_id = tex.id();
    let checker_id = checker.id();
    let r = egui::ScrollArea::both()
        .id_salt("texture-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (rect, resp) = ui.allocate_exact_size(shown, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let tiles = vec2(rect.width() / 16.0, rect.height() / 16.0);
            painter.image(
                checker_id,
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(tiles.x, tiles.y)),
                Color32::WHITE,
            );
            painter.image(
                tex_id,
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            resp.hovered()
        });
    if r.inner && (zoom_delta - 1.0).abs() > f32::EPSILON {
        cur.fit = false;
        cur.zoom = (cur.zoom * zoom_delta).clamp(0.0625, 16.0);
    }
}

// ---------------------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------------------

/// Parses user variable values: `NAME=text; #03A1B2C=12` (`;` or newline separated).
/// Names are hashed (`string-table.md` §8.3); `#` + hex is a literal argument ID.
/// Integer and decimal texts become numbers, so selectors can test them.
#[must_use]
pub fn parse_values(input: &str) -> Values {
    let mut values = Values::new();
    for part in input.split([';', '\n']).take(256) {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if k.is_empty() {
            continue;
        }
        let value: tessera_text::Value = if let Ok(i) = v.parse::<i64>() {
            i.into()
        } else if let Ok(f) = v.parse::<f64>()
            && f.is_finite()
        {
            f.into()
        } else {
            v.into()
        };
        if let Some(hex) = k.strip_prefix('#') {
            if let Ok(id) = u32::from_str_radix(hex, 16) {
                values.insert(id, value);
            }
        } else {
            values.insert_named(k, value);
        }
    }
    values
}

fn strings_view(ui: &mut Ui, l: &Loaded, cur: &mut Current) {
    let did = cur.nav.did;
    ensure(&mut cur.strings, &l.stores.strings, AssetKey::from(did));
    let Some(p) = ready(ui, cur.strings.as_ref()) else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{} entries", thousands(p.rows.len() as u64)));
        if let Some(n) = &p.named {
            ui.label(RichText::new(format!("named “{n}”")).strong());
        }
        ui.label(RichText::new(format!("locale {}", p.locale)).weak());
        if !p.issues.is_empty() {
            ui.colored_label(
                Color32::from_rgb(0xE0, 0xB0, 0x60),
                format!("{} record issues", p.issues.len()),
            )
            .on_hover_text(p.issues.join("\n"));
        }
        ui.separator();
        ui.label("Filter");
        ui.add(egui::TextEdit::singleline(&mut cur.string_filter).desired_width(200.0));
    });
    let filter = cur.string_filter.trim().to_lowercase();
    let rows: Vec<usize> = if filter.is_empty() {
        cur.string_filtered = None;
        Vec::new()
    } else {
        if cur
            .string_filtered
            .as_ref()
            .is_none_or(|(f, _)| *f != filter)
        {
            let hits = filter_strings(&p, &filter);
            cur.string_filtered = Some((filter.clone(), hits));
        }
        cur.string_filtered
            .as_ref()
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let count = if filter.is_empty() {
        p.rows.len()
    } else {
        rows.len()
    };
    let palette = Palette::for_dark_mode(ui.visuals().dark_mode);
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
    let detail_h = if cur.string_sel.is_some() {
        (ui.available_height() * 0.45).max(120.0)
    } else {
        0.0
    };
    let list_h = (ui.available_height() - detail_h).max(80.0);
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("strings-scroll")
        .max_height(list_h)
        .auto_shrink([false, true])
        .show_rows(ui, row_h, count, |ui, range| {
            let width = ui.available_width();
            for i in range {
                let idx = if filter.is_empty() { i } else { rows[i] };
                let Some(row) = p.rows.get(idx) else { continue };
                let mut job =
                    layout_job(&row.text, &font, &palette, true, (width - 100.0).max(50.0));
                job.halign = egui::Align::LEFT;
                ui.horizontal(|ui| {
                    let sel = cur.string_sel == Some(idx);
                    let r = ui.selectable_label(
                        sel,
                        RichText::new(format!("{:08X}", row.token)).monospace(),
                    );
                    if r.clicked() {
                        clicked = Some(idx);
                    }
                    if !row.notes.is_empty() {
                        ui.colored_label(Color32::from_rgb(0xE0, 0xB0, 0x60), "!")
                            .on_hover_text(row.notes.join("\n"));
                    }
                    if ui
                        .add(egui::Label::new(job).sense(egui::Sense::click()))
                        .clicked()
                    {
                        clicked = Some(idx);
                    }
                });
            }
        });
    if let Some(c) = clicked {
        cur.string_sel = Some(c);
    }
    let Some(sel) = cur.string_sel else { return };
    let Some(row) = p.rows.get(sel) else { return };
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("string-detail")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("token {:#010x}", row.token)).strong());
                ui.label(format!("{} slots", row.slots));
                if ui.small_button("close").clicked() {
                    cur.string_sel = None;
                }
            });
            ui.label("Format string (${VAR} form):");
            ui.add(egui::Label::new(RichText::new(&row.format).monospace()).wrap());
            ui.horizontal(|ui| {
                ui.label("Values");
                ui.add(
                    egui::TextEdit::singleline(&mut cur.string_values)
                        .hint_text("NAME=text; #03A1B2C=12")
                        .desired_width(360.0),
                );
            });
            let width = ui.available_width();
            let rendered = if cur.string_values.trim().is_empty() {
                None
            } else {
                p.table.get(row.token).map(|entry| {
                    let msg = Message::from_entry(entry);
                    render_entry(&msg, &parse_values(&cur.string_values))
                })
            };
            let text = rendered.as_ref().map_or(&row.text, |r| &r.text);
            ui.label("Rendered:");
            ui.add(
                egui::Label::new(layout_job(
                    text,
                    &egui::TextStyle::Body.resolve(ui.style()),
                    &palette,
                    false,
                    width,
                ))
                .wrap(),
            );
            if let Some(r) = &rendered {
                for d in &r.diagnostics {
                    ui.label(RichText::new(d.to_string()).small().weak());
                }
                if let Some(t) = &r.tags {
                    ui.label(RichText::new(format!("tags: {t}")).small().weak());
                }
            }
            for n in &row.notes {
                ui.label(
                    RichText::new(n)
                        .small()
                        .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
                );
            }
        });
}

/// Rows of a string table whose text or token contains `filter` (lowercase).
fn filter_strings(p: &StringsProduct, filter: &str) -> Vec<usize> {
    p.rows
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            format!("{:08x}", r.token).contains(filter)
                || r.text
                    .spans()
                    .iter()
                    .any(|s| s.text.to_lowercase().contains(filter))
        })
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------

/// Rows to show: descendants of collapsed rows hidden; with a filter, every matching
/// row (lowercase `filter`).
#[must_use]
pub fn visible_prop_rows(
    p: &PropsProduct,
    collapsed: &std::collections::HashSet<usize>,
    filter: &str,
) -> Vec<usize> {
    if !filter.is_empty() {
        return p
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.name.to_lowercase().contains(filter)
                    || r.value.to_lowercase().contains(filter)
                    || r.label.to_lowercase().contains(filter)
            })
            .map(|(i, _)| i)
            .collect();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < p.rows.len() {
        out.push(i);
        if collapsed.contains(&i) {
            i += p.rows[i].descendants + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn props_view(ui: &mut Ui, l: &Loaded, cur: &mut Current, actions: &mut Vec<Action>) {
    let did = cur.nav.did;
    ensure(&mut cur.props, &l.stores.props, AssetKey::from(did));
    let Some(p) = ready(ui, cur.props.as_ref()) else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        if p.set_did != did {
            ui.label("property set");
            if ui.link(format!("{:#010x}", p.set_did)).clicked() {
                actions.push(Action::Open(p.set_did));
            }
            ui.label("(+0x09000000 rule) ·");
        }
        ui.label(format!(
            "{} properties · {} rows · depth {}",
            p.count,
            thousands(p.rows.len() as u64),
            p.depth
        ));
        if p.trailing > 0 {
            ui.colored_label(
                Color32::from_rgb(0xE0, 0xB0, 0x60),
                format!("{} trailing bytes", p.trailing),
            );
        }
        if p.truncated {
            ui.colored_label(Color32::from_rgb(0xE0, 0xB0, 0x60), "rows truncated");
        }
        ui.separator();
        if ui.small_button("collapse all").clicked() {
            cur.props_collapsed = p
                .rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r.descendants > 0)
                .map(|(i, _)| i)
                .collect();
            cur.props_visible = None;
        }
        if ui.small_button("expand all").clicked() {
            cur.props_collapsed.clear();
            cur.props_visible = None;
        }
        ui.label("Filter");
        if ui
            .add(egui::TextEdit::singleline(&mut cur.props_filter).desired_width(200.0))
            .changed()
        {
            cur.props_visible = None;
        }
    });
    let filter = cur.props_filter.trim().to_lowercase();
    if cur.props_visible.is_none() {
        cur.props_visible = Some(visible_prop_rows(&p, &cur.props_collapsed, &filter));
    }
    let visible = cur.props_visible.clone().unwrap_or_default();
    let row_h = ui
        .text_style_height(&egui::TextStyle::Monospace)
        .max(ui.spacing().interact_size.y);
    let mut toggle = None;
    egui::ScrollArea::both()
        .id_salt("props-scroll")
        .auto_shrink([false, false])
        .show_rows(ui, row_h, visible.len(), |ui, range| {
            for i in range {
                let Some(&idx) = visible.get(i) else { continue };
                let Some(row) = p.rows.get(idx) else { continue };
                ui.horizontal(|ui| {
                    ui.add_space(f32::from(row.depth) * 16.0);
                    if row.descendants > 0 {
                        let open = !cur.props_collapsed.contains(&idx);
                        if ui.small_button(if open { "▼" } else { "▶" }).clicked() {
                            toggle = Some(idx);
                        }
                    } else {
                        ui.add_space(18.0);
                    }
                    ui.label(RichText::new(&row.name).monospace().strong())
                        .on_hover_text(format!("pid {:#x} · {}", row.pid, row.declared));
                    ui.label(RichText::new(&row.declared).small().weak());
                    ui.label(RichText::new(&row.value).monospace());
                    if !row.label.is_empty() {
                        ui.label(
                            RichText::new(&row.label).color(Color32::from_rgb(0x90, 0xC8, 0xFF)),
                        );
                    }
                    for (caption, target) in &row.links {
                        if ui
                            .small_button(format!("{caption} {target:#010x}"))
                            .on_hover_text(Did(*target).kind().name())
                            .clicked()
                        {
                            actions.push(Action::Open(*target));
                        }
                    }
                });
            }
        });
    if let Some(t) = toggle {
        if !cur.props_collapsed.remove(&t) {
            cur.props_collapsed.insert(t);
        }
        cur.props_visible = None;
    }
}

// ---------------------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------------------

fn icon_view(
    ui: &mut Ui,
    l: &Loaded,
    cur: &mut Current,
    checker: &egui::TextureHandle,
    actions: &mut Vec<Action>,
) {
    let did = cur.nav.did;
    ensure(&mut cur.icon, &l.stores.icon, AssetKey::from(did));
    let Some(p) = ready(ui, cur.icon.as_ref()) else {
        return;
    };
    if p.composites.is_empty() {
        ui.label(RichText::new("This record names no icon.").weak());
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("icon-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, c) in p.composites.iter().enumerate() {
                ui.heading(format!(
                    "{}{}",
                    c.family,
                    c.size
                        .map(|(w, h)| format!("  {w}×{h}"))
                        .unwrap_or_default()
                ));
                ui.horizontal(|ui| {
                    match c.image.as_ref().filter(|i| has_area(i)) {
                        Some(img) => {
                            let key = format!("icon/{did:08x}/{i}");
                            let tex = cur.gpu.entry(key.clone()).or_insert_with(|| {
                                ui.ctx().load_texture(
                                    key,
                                    egui::ImageData::Color(Arc::clone(img)),
                                    TextureOptions::NEAREST,
                                )
                            });
                            let base = vec2(img.size[0] as f32, img.size[1] as f32);
                            let scale = (128.0 / base.x.max(base.y).max(1.0))
                                .floor()
                                .clamp(1.0, 8.0);
                            let (rect, _) =
                                ui.allocate_exact_size(base * scale, egui::Sense::hover());
                            let tiles = vec2(rect.width() / 16.0, rect.height() / 16.0);
                            ui.painter().image(
                                checker.id(),
                                rect,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(tiles.x, tiles.y)),
                                Color32::WHITE,
                            );
                            ui.painter().image(
                                tex.id(),
                                rect,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        None => {
                            ui.label(RichText::new("no layer present (no icon)").weak());
                        }
                    }
                    egui::Grid::new(("icon-layers", i))
                        .striped(true)
                        .show(ui, |ui| {
                            for layer in &c.layers {
                                ui.label(&layer.role);
                                if ui.link(format!("{:#010x}", layer.did)).clicked() {
                                    actions.push(Action::Open(layer.did));
                                }
                                ui.label(
                                    RichText::new(layer.property.as_deref().unwrap_or("")).weak(),
                                );
                                ui.label(&layer.status);
                                ui.end_row();
                            }
                        });
                });
                ui.separator();
            }
        });
}

// ---------------------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------------------

fn record_view(ui: &mut Ui, l: &Loaded, cur: &mut Current, actions: &mut Vec<Action>) {
    let did = cur.nav.did;
    let variant = cur.nav.member.map_or(0, |m| m as u32 + 1);
    ensure(
        &mut cur.record,
        &l.stores.record,
        AssetKey::new(did, variant),
    );
    let Some(p) = ready(ui, cur.record.as_ref()) else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&p.type_name).strong());
        for (k, v) in &p.summary {
            ui.label(format!("{k}: {v}"));
        }
        for (caption, target) in &p.links {
            if ui
                .small_button(format!("{caption} {target:#010x}"))
                .clicked()
            {
                actions.push(Action::Open(*target));
            }
        }
    });
    if p.rows.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(egui::TextEdit::singleline(&mut cur.record_filter).desired_width(200.0));
    });
    let filter = cur.record_filter.trim().to_lowercase();
    let rows: Option<Vec<usize>> = if filter.is_empty() {
        None
    } else {
        if cur
            .record_filtered
            .as_ref()
            .is_none_or(|(f, _)| *f != filter)
        {
            let hits = p
                .rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r.cells.iter().any(|c| c.to_lowercase().contains(&filter)))
                .map(|(i, _)| i)
                .collect();
            cur.record_filtered = Some((filter.clone(), hits));
        }
        cur.record_filtered.as_ref().map(|(_, v)| v.clone())
    };
    let count = rows.as_ref().map_or(p.rows.len(), Vec::len);
    let cols = p.columns.len().max(1);
    let row_h = ui
        .text_style_height(&egui::TextStyle::Monospace)
        .max(ui.spacing().interact_size.y);
    let col_w = ((ui.available_width() - 24.0) / cols as f32).max(60.0);
    ui.horizontal(|ui| {
        for c in &p.columns {
            ui.add_sized([col_w, row_h], egui::Label::new(RichText::new(*c).strong()));
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("record-scroll")
        .auto_shrink([false, false])
        .show_rows(ui, row_h, count, |ui, range| {
            for i in range {
                let idx = rows.as_ref().map_or(i, |r| r[i]);
                let Some(row) = p.rows.get(idx) else { continue };
                ui.horizontal(|ui| {
                    for (c, cell) in row.cells.iter().enumerate() {
                        let is_link = row.link.is_some()
                            && p.columns
                                .get(c)
                                .is_some_and(|n| matches!(*n, "value" | "data" | "surface"));
                        if is_link {
                            let r = ui.add_sized(
                                [col_w, row_h],
                                egui::Link::new(RichText::new(cell).monospace()),
                            );
                            if r.clicked()
                                && let Some(t) = row.link
                            {
                                actions.push(Action::Open(t));
                            }
                        } else {
                            ui.add_sized(
                                [col_w, row_h],
                                egui::Label::new(RichText::new(cell).monospace()).truncate(),
                            );
                        }
                    }
                });
            }
        });
}

/// Whether an image has pixels (an empty texture is a GPU validation error).
fn has_area(img: &egui::ColorImage) -> bool {
    img.size[0] > 0 && img.size[1] > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoders::PropRow;
    use std::collections::HashSet;
    use tessera_text::render_plain;

    #[test]
    fn values_parse() {
        let v = parse_values("PLAYER=Wren Hollow[fn]; COUNT=3;#0000001=x; =skip; junk");
        assert_eq!(v.len(), 3);
        let msg = Message::from_format_string("${PLAYER} has ${COUNT}");
        assert_eq!(render_plain(&msg, &v), "Wren Hollow has 3");
        assert!(parse_values("").is_empty());
        assert!(parse_values("#zz=1").is_empty());
    }

    fn row(depth: u16, descendants: usize, name: &str) -> PropRow {
        PropRow {
            depth,
            pid: 1,
            name: name.into(),
            declared: "Int".into(),
            value: "1".into(),
            label: String::new(),
            links: Vec::new(),
            descendants,
        }
    }

    #[test]
    fn prop_visibility() {
        let p = PropsProduct {
            did: 0x7900_0001,
            set_did: 0x7900_0001,
            trailing: 0,
            count: 2,
            depth: 2,
            rows: vec![
                row(0, 2, "A"),
                row(1, 0, "B"),
                row(1, 0, "C"),
                row(0, 0, "D"),
            ],
            truncated: false,
        };
        let none = HashSet::new();
        assert_eq!(visible_prop_rows(&p, &none, ""), vec![0, 1, 2, 3]);
        let collapsed: HashSet<usize> = [0].into_iter().collect();
        assert_eq!(visible_prop_rows(&p, &collapsed, ""), vec![0, 3]);
        assert_eq!(visible_prop_rows(&p, &collapsed, "c"), vec![2]);
    }
}
