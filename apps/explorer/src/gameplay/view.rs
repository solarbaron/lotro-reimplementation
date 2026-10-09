// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI of the gameplay view: the card header, the icon layers of objects (from the icon
//! view's product), and the sections as collapsible tables with DID links and a filter.
//!
//! Like every view it only requests products and draws what is ready.

use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, RichText, TextureOptions, Ui, pos2, vec2};
use tessera_assets::{AssetKey, Handle};

use super::card::did_text;
use super::{GameplayProduct, Section};
use crate::app::{Current, Loaded};
use crate::views::{Action, ensure, ready};

/// Most rows of one section drawn at a time; the filter narrows the rest.
pub const MAX_DRAWN_ROWS: usize = 400;

/// The gameplay view's state for the selected DID.
#[derive(Default)]
pub struct GameplayViewState {
    /// The card being built or shown.
    pub handle: Option<Handle<GameplayProduct>>,
    /// Row filter (case-insensitive substring of any cell).
    pub filter: String,
}

impl std::fmt::Debug for GameplayViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameplayViewState")
            .field("requested", &self.handle.as_ref().map(Handle::key))
            .field("filter", &self.filter)
            .finish()
    }
}

impl GameplayViewState {
    /// Cancels the pending build.
    pub fn cancel(&self) {
        if let Some(h) = &self.handle {
            h.cancel();
        }
    }

    /// Whether the card has finished (ready or failed), for headless screenshots.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(Handle::is_finished)
    }
}

/// The rows of `s` that match `filter` (lower-case; empty matches all).
#[must_use]
pub fn matching_rows<'s>(s: &'s Section, filter: &str) -> Vec<&'s super::Row> {
    if filter.is_empty() {
        return s.rows.iter().collect();
    }
    s.rows
        .iter()
        .filter(|r| r.cells.iter().any(|c| c.to_lowercase().contains(filter)))
        .collect()
}

/// Draws the gameplay view.
pub(crate) fn gameplay_view(
    ui: &mut Ui,
    l: &Loaded,
    cur: &mut Current,
    checker: &egui::TextureHandle,
    actions: &mut Vec<Action>,
) {
    let did = cur.nav.did;
    ensure(
        &mut cur.gameplay.handle,
        &l.stores.gameplay,
        AssetKey::from(did),
    );
    let Some(p) = ready(ui, cur.gameplay.handle.as_ref()) else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&p.kind).strong().size(16.0));
        ui.label(RichText::new(format!("({})", p.family.name())).weak());
        if let Some(t) = &p.title {
            ui.label(RichText::new(t).size(16.0));
        }
    });
    ui.horizontal_wrapped(|ui| {
        if let Some(o) = p.object.filter(|&o| o != did)
            && ui.link(format!("object {}", did_text(o.into()))).clicked()
        {
            actions.push(Action::Open(o));
        }
        if let Some(s) = p.property_set.filter(|&s| s != did)
            && ui
                .link(format!("property set {}", did_text(s.into())))
                .clicked()
        {
            actions.push(Action::Open(s));
        }
        if let Some(c) = p.root_class {
            ui.label(RichText::new(format!("root class {c}")).weak());
        }
    });
    for n in &p.notes {
        ui.label(RichText::new(n).color(Color32::from_rgb(220, 180, 90)));
    }
    if p.object.is_some() {
        icons(ui, l, cur, checker, actions);
    }
    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(egui::TextEdit::singleline(&mut cur.gameplay.filter).desired_width(220.0));
    });
    let filter = cur.gameplay.filter.trim().to_lowercase();
    egui::ScrollArea::vertical()
        .id_salt("gameplay-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, s) in p.sections.iter().enumerate() {
                section(ui, i, s, &filter, actions);
            }
        });
}

fn section(ui: &mut Ui, index: usize, s: &Section, filter: &str, actions: &mut Vec<Action>) {
    let rows = matching_rows(s, filter);
    if rows.is_empty() && !filter.is_empty() {
        return;
    }
    let title = format!(
        "{}  ({}{})",
        s.title,
        rows.len(),
        if s.truncated { ", cut" } else { "" }
    );
    let open = !s.preformatted && s.rows.len() <= MAX_DRAWN_ROWS;
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .id_salt(("gameplay-section", index))
        .default_open(open)
        .show(ui, |ui| {
            egui::Grid::new(("gameplay-grid", index))
                .striped(true)
                .num_columns(s.columns.len() + 1)
                .show(ui, |ui| {
                    if !s.preformatted {
                        for c in &s.columns {
                            ui.label(RichText::new(*c).weak());
                        }
                        ui.end_row();
                    }
                    for r in rows.iter().take(MAX_DRAWN_ROWS) {
                        let mut linked: Vec<u32> = Vec::new();
                        for cell in &r.cells {
                            let target = r
                                .links
                                .iter()
                                .copied()
                                .find(|&d| did_text(d.into()) == *cell);
                            match target {
                                Some(d) => {
                                    linked.push(d);
                                    if ui.link(RichText::new(cell).monospace()).clicked() {
                                        actions.push(Action::Open(d));
                                    }
                                }
                                None => {
                                    let t = RichText::new(cell);
                                    let t = if s.preformatted { t.monospace() } else { t };
                                    ui.add(egui::Label::new(t).wrap());
                                }
                            }
                        }
                        ui.horizontal_wrapped(|ui| {
                            for &d in r.links.iter().filter(|d| !linked.contains(d)) {
                                if ui
                                    .small_button(RichText::new(did_text(d.into())).monospace())
                                    .clicked()
                                {
                                    actions.push(Action::Open(d));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            if rows.len() > MAX_DRAWN_ROWS {
                ui.label(
                    RichText::new(format!(
                        "{} more rows; narrow them with the filter",
                        rows.len() - MAX_DRAWN_ROWS
                    ))
                    .weak(),
                );
            }
        });
}

/// The icon layers of the selected object, as the icon view composes them.
fn icons(
    ui: &mut Ui,
    l: &Loaded,
    cur: &mut Current,
    checker: &egui::TextureHandle,
    actions: &mut Vec<Action>,
) {
    let did = cur.nav.did;
    ensure(&mut cur.icon, &l.stores.icon, AssetKey::from(did));
    let Some(h) = cur.icon.as_ref() else { return };
    let Some(p) = h.get() else { return };
    if p.composites.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (i, c) in p.composites.iter().enumerate() {
            ui.vertical(|ui| {
                ui.label(RichText::new(&c.family).weak());
                if let Some(img) = c.image.as_ref().filter(|i| i.size[0] > 0 && i.size[1] > 0) {
                    let key = format!("icon/{did:08x}/{i}");
                    let tex = cur.gpu.entry(key.clone()).or_insert_with(|| {
                        ui.ctx().load_texture(
                            key,
                            egui::ImageData::Color(Arc::clone(img)),
                            TextureOptions::NEAREST,
                        )
                    });
                    let base = vec2(img.size[0] as f32, img.size[1] as f32);
                    let scale = (64.0 / base.x.max(base.y).max(1.0)).floor().clamp(1.0, 4.0);
                    let (rect, resp) = ui.allocate_exact_size(base * scale, egui::Sense::hover());
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
                    resp.on_hover_ui(|ui| {
                        for layer in &c.layers {
                            ui.label(format!(
                                "{} {:#010x} {}",
                                layer.role, layer.did, layer.status
                            ));
                        }
                    });
                }
                for layer in &c.layers {
                    if ui
                        .small_button(
                            RichText::new(format!("{} {:#010x}", layer.role, layer.did))
                                .monospace(),
                        )
                        .clicked()
                    {
                        actions.push(Action::Open(layer.did));
                    }
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matches_any_cell() {
        let mut s = Section::new("t", &["a", "b"]);
        s.push(vec!["Alpha".into(), "1".into()], Vec::new());
        s.push(vec!["beta".into(), "two".into()], Vec::new());
        assert_eq!(matching_rows(&s, "").len(), 2);
        assert_eq!(matching_rows(&s, "alp").len(), 1);
        assert_eq!(matching_rows(&s, "two").len(), 1);
        assert!(matching_rows(&s, "zzz").is_empty());
    }
}
