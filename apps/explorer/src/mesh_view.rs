// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh view ("3D" tab): toolbar, 3D viewport, part and draw list, and the material
//! inspector.
//!
//! The scene is built on asset workers ([`crate::scene`]) and drawn by the render thread
//! ([`crate::viewport`]); this module only requests both and shows what is ready, so a
//! UI frame never decodes or renders.
//!
//! Controls: drag to orbit, drag with the secondary or middle button (or Shift + drag) to
//! pan, wheel to zoom, double-click or `F` to frame the scene, `R` to reset the angles,
//! `W` to toggle the wireframe (keys only without modifiers, so `Ctrl+F` stays search).

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, RichText, TextureOptions, Ui, pos2, vec2};
use tessera_assets::{AssetKey, AssetState, Handle};
use tessera_render::MeshShading;

use crate::app::{Current, Loaded, Nav};
use crate::cli::{ShadingArg, ViewKind};
use crate::hex::thousands;
use crate::scene::{AlphaKind, SceneMaterial, SceneProduct, SceneRequest};
use crate::viewport::{
    CollisionDraw, DrawRequest, DyeChannel, DyeOverride, OrbitView, Viewport, fit_viewport,
};
use crate::views::{Action, ensure};

/// Display settings of the mesh view; they persist while browsing from DID to DID.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshSettings {
    /// Surface shading.
    pub shading: MeshShading,
    /// Wireframe overlay.
    pub wireframe: bool,
    /// Vertex-normal lines.
    pub normals: bool,
    /// Ground grid and axes.
    pub grid: bool,
    /// Outline the selected draw.
    pub highlight: bool,
    /// Dim every draw but the selected one.
    pub isolate: bool,
    /// Vertex colours (RGB) modulate the base colour (opt-in: `mesh.md` §5.4 OPEN).
    pub vertex_colors: bool,
    /// The materials' diffuse colour 2000 modulates the base colour (opt-in:
    /// `material.md` §19 OPEN).
    pub diffuse_tint: bool,
    /// Dye preview on.
    pub dye_on: bool,
    /// Dye preview colour (sRGB).
    pub dye_color: [u8; 3],
    /// Dye channel previewed.
    pub dye_channel: DyeChannel,
    /// Explode factor for multi-part scenes.
    pub explode: f32,
    /// Camera angles (yaw, pitch in degrees) applied when a scene is first framed.
    pub angles: Option<[f32; 2]>,
    /// Collision overlays (M4.5) shown.
    pub collision: bool,
    /// Collision triangles filled translucently, not only their edges.
    pub collision_fill: bool,
    /// Collision overlays drawn over the scene instead of depth-tested.
    pub collision_xray: bool,
}

impl Default for MeshSettings {
    fn default() -> Self {
        Self {
            shading: MeshShading::Lit,
            wireframe: false,
            normals: false,
            grid: true,
            highlight: true,
            isolate: false,
            vertex_colors: false,
            diffuse_tint: false,
            dye_on: false,
            dye_color: [0x2E, 0x6F, 0xC8],
            dye_channel: DyeChannel::Primary,
            explode: 0.0,
            angles: None,
            collision: true,
            collision_fill: false,
            collision_xray: false,
        }
    }
}

/// Start-up options of the mesh view (command line). The default turns every option
/// off (vertex colours and the diffuse tint are opt-in).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MeshOptions {
    /// Shading.
    pub shading: Option<ShadingArg>,
    /// Vertex colours modulate the base colour.
    pub vertex_colors: bool,
    /// Diffuse colour 2000 modulates the base colour.
    pub diffuse_tint: bool,
    /// Wireframe overlay.
    pub wireframe: bool,
    /// Normal lines.
    pub normals: bool,
    /// Level for the first selection.
    pub lod: Option<u8>,
    /// Choice for the first selection.
    pub choice: Option<u8>,
    /// Dye preview colour.
    pub dye: Option<[u8; 3]>,
    /// Camera yaw and pitch in degrees.
    pub angles: Option<[f32; 2]>,
    /// Draw (part, draw) selected in the first selection.
    pub select: Option<(usize, u32)>,
}

impl MeshOptions {
    /// The display settings these options start with.
    #[must_use]
    pub fn settings(&self) -> MeshSettings {
        let mut s = MeshSettings::default();
        if let Some(sh) = self.shading {
            s.shading = match sh {
                ShadingArg::Lit => MeshShading::Lit,
                ShadingArg::Base => MeshShading::BaseColor,
                ShadingArg::Normals => MeshShading::Normals,
                ShadingArg::Uv => MeshShading::UvChecker,
                ShadingArg::Wire => {
                    s.wireframe = true;
                    MeshShading::Hidden
                }
            };
        }
        s.wireframe |= self.wireframe;
        s.normals = self.normals;
        s.vertex_colors = self.vertex_colors;
        s.diffuse_tint = self.diffuse_tint;
        if let Some(c) = self.dye {
            s.dye_on = true;
            s.dye_color = c;
        }
        s.angles = self.angles;
        s
    }

    /// The mesh state of the first selection.
    #[must_use]
    pub fn start(&self) -> MeshState {
        let mut m = MeshState::new(SceneRequest {
            lod: self.lod.unwrap_or(0),
            choice: self.choice.unwrap_or(0),
            vertex_colors: self.vertex_colors,
        });
        m.selected = self.select;
        m
    }
}

/// Per-selection state of the mesh view.
#[derive(Debug, Default)]
pub struct MeshState {
    /// The scene request (level, choice, vertex colours).
    pub request: SceneRequest,
    /// The pending or finished scene.
    pub handle: Option<Handle<SceneProduct>>,
    /// The last finished scene (kept on screen while another level loads).
    pub shown: Option<Arc<SceneProduct>>,
    /// Why the last request failed.
    pub failed: Option<String>,
    /// The camera.
    pub orbit: OrbitView,
    /// The (DID, choice) the camera was framed for.
    pub framed: Option<(u32, usize)>,
    /// Frame again on the next draw.
    pub reframe: bool,
    /// Selected draw (part, draw).
    pub selected: Option<(usize, u32)>,
    /// Hidden parts.
    pub hidden_parts: HashSet<usize>,
    /// Hidden draws.
    pub hidden: HashSet<(usize, u32)>,
    /// The scene id the viewport gave the shown scene.
    pub scene_id: Option<u64>,
    /// Hidden collision overlays; `None` until the first scene sets the defaults
    /// ([`crate::collision_overlay::OverlayKind::shown_by_default`]).
    pub hidden_overlays: Option<HashSet<usize>>,
}

impl MeshState {
    /// State for a new selection, starting with `request`.
    #[must_use]
    pub fn new(request: SceneRequest) -> Self {
        Self {
            request,
            ..Self::default()
        }
    }

    /// Draws level `lod`. Draw numbers mean different draws at different levels, so a
    /// change of level clears the selection and the hidden draws and parts.
    pub fn set_lod(&mut self, lod: u8) {
        if self.request.lod != lod {
            self.request.lod = lod;
            self.clear_selection();
        }
    }

    /// Shows alternative `choice` (clears the selection and the hidden draws and parts).
    /// Another alternative has its own collision overlays, so the hidden overlays are
    /// reset too: the next scene sets their defaults again.
    pub fn set_choice(&mut self, choice: u8) {
        if self.request.choice != choice {
            self.request.choice = choice;
            self.clear_selection();
            self.hidden_overlays = None;
        }
    }

    fn clear_selection(&mut self) {
        self.selected = None;
        self.hidden.clear();
        self.hidden_parts.clear();
    }

    /// Cancels the pending scene.
    pub fn cancel(&self) {
        if let Some(h) = &self.handle {
            h.cancel();
        }
    }
}

/// Whether the mesh view of `cur` is complete: the scene is built (or failed) and the
/// frame on screen shows it as last requested (or rendering failed).
#[must_use]
pub(crate) fn mesh_ready(cur: &Current, vp: Option<&Viewport>) -> bool {
    let m = &cur.mesh;
    let Some(h) = &m.handle else { return false };
    match h.state() {
        AssetState::Ready(p) => {
            if !p.drawable() {
                return true;
            }
            let Some(vp) = vp else { return false };
            let shown_is_current = m.shown.as_ref().is_some_and(|s| Arc::ptr_eq(s, &p));
            shown_is_current
                && (vp.error().is_some()
                    || m.scene_id
                        .is_some_and(|id| vp.shows(id) && vp.texture().is_some()))
        }
        AssetState::Failed(_) | AssetState::Cancelled => true,
        AssetState::Queued | AssetState::Loading => false,
    }
}

/// An egui image of a scene thumbnail (already opaque); a 1×1 blank for an inconsistent
/// image, never a panic.
fn thumb_color_image(img: &tessera_formats::RgbaImage) -> egui::ColorImage {
    if !crate::geometry::image_ok(img) {
        return egui::ColorImage::new([1, 1], vec![Color32::TRANSPARENT]);
    }
    egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.pixels)
}

fn srgb_color(c: [f32; 4]) -> Color32 {
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

fn shading_name(s: MeshShading) -> &'static str {
    match s {
        MeshShading::Lit => "Lit",
        MeshShading::BaseColor => "Base colour",
        MeshShading::Normals => "Normals",
        MeshShading::UvChecker => "UV checker",
        MeshShading::Hidden => "Hidden (wire only)",
    }
}

/// Draws the mesh view.
pub(crate) fn mesh_view(
    ui: &mut Ui,
    l: &Loaded,
    cur: &mut Current,
    settings: &mut MeshSettings,
    vp: &mut Viewport,
    checker: &egui::TextureHandle,
    actions: &mut Vec<Action>,
) {
    let did = cur.nav.did;
    let key = AssetKey::new(did, cur.mesh.request.variant());
    ensure(&mut cur.mesh.handle, &l.stores.scene, key);
    if let Some(h) = &cur.mesh.handle {
        match h.state() {
            AssetState::Ready(p) => {
                cur.mesh.failed = None;
                if cur.mesh.shown.as_ref().is_none_or(|s| !Arc::ptr_eq(s, &p)) {
                    cur.mesh.shown = Some(p);
                }
            }
            AssetState::Failed(e) => cur.mesh.failed = Some(e.to_string()),
            _ => {}
        }
    }
    let loading = cur.mesh.handle.as_ref().is_some_and(|h| !h.is_finished());
    let Some(scene) = cur.mesh.shown.clone() else {
        match &cur.mesh.failed {
            Some(e) => {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("building the scene …");
                });
            }
        }
        return;
    };
    if scene.request.vertex_colors != settings.vertex_colors {
        cur.mesh.request.vertex_colors = settings.vertex_colors;
    }
    toolbar(ui, cur, settings, vp, &scene, loading);
    if let Some(e) = &cur.mesh.failed {
        ui.colored_label(Color32::LIGHT_RED, e);
    }
    egui::Panel::right("mesh-inspector")
        .resizable(true)
        .default_size(420.0)
        .min_size(240.0)
        .show(ui, |ui| {
            inspector(ui, cur, settings, &scene, checker, actions)
        });
    viewport_widget(ui, cur, settings, vp, &scene);
}

fn toolbar(
    ui: &mut Ui,
    cur: &mut Current,
    settings: &mut MeshSettings,
    vp: &Viewport,
    scene: &Arc<SceneProduct>,
    loading: bool,
) {
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("mesh-shading")
            .selected_text(shading_name(settings.shading))
            .show_ui(ui, |ui| {
                for s in [
                    MeshShading::Lit,
                    MeshShading::BaseColor,
                    MeshShading::Normals,
                    MeshShading::UvChecker,
                    MeshShading::Hidden,
                ] {
                    ui.selectable_value(&mut settings.shading, s, shading_name(s));
                }
            });
        ui.toggle_value(&mut settings.wireframe, "Wireframe")
            .on_hover_text("Triangle edges (W over the viewport)");
        ui.toggle_value(&mut settings.normals, "Normals")
            .on_hover_text("Vertex-normal lines");
        ui.toggle_value(&mut settings.grid, "Grid")
            .on_hover_text("Ground grid on the XY plane (Z up) and axes (X red, Y green, Z blue)");
        ui.toggle_value(&mut settings.highlight, "Highlight")
            .on_hover_text("Outline the selected draw's bounds");
        ui.toggle_value(&mut settings.isolate, "Isolate")
            .on_hover_text("Dim every draw but the selected one");
        ui.toggle_value(&mut settings.vertex_colors, "Vertex colours")
            .on_hover_text(
                "Vertex colours (RGB; alpha is never used) modulate the base colour \
                 (mesh.md §5.4 OPEN)",
            );
        ui.toggle_value(&mut settings.diffuse_tint, "Diffuse colour")
            .on_hover_text(
                "The materials' diffuse colour 2000 modulates the base colour \
                 (material.md §19 OPEN; some materials set it black)",
            );
        ui.separator();
        if !scene.level_labels.is_empty() {
            // A level beyond the scene's last is clamped per part (scene::build_scene).
            let current = usize::from(cur.mesh.request.lod).min(scene.level_labels.len() - 1);
            let text = scene.level_labels[current].clone();
            egui::ComboBox::from_id_salt("mesh-lod")
                .selected_text(text)
                .width(240.0)
                .show_ui(ui, |ui| {
                    for (i, label) in scene.level_labels.iter().enumerate().take(256) {
                        if ui.selectable_label(current == i, label).clicked() {
                            cur.mesh.set_lod(i as u8);
                        }
                    }
                });
        }
        if ui
            .button("Frame")
            .on_hover_text("Frame the scene (F, double-click)")
            .clicked()
        {
            cur.mesh.reframe = true;
        }
        if ui
            .button("Reset view")
            .on_hover_text("Default angles (R)")
            .clicked()
        {
            cur.mesh.orbit.reset_angles();
            cur.mesh.reframe = true;
        }
        if !scene.collision.is_empty() {
            ui.separator();
            ui.toggle_value(&mut settings.collision, "Collision")
                .on_hover_text("Collision overlays (collision.md; the list is in the inspector)");
            ui.toggle_value(&mut settings.collision_fill, "fill")
                .on_hover_text("Fill collision triangles translucently");
            ui.toggle_value(&mut settings.collision_xray, "x-ray")
                .on_hover_text("Draw collision over the scene instead of depth-tested");
        }
        if scene.parts.len() > 1 {
            ui.add(
                egui::Slider::new(&mut settings.explode, 0.0..=2.0)
                    .text("explode")
                    .fixed_decimals(2),
            );
        }
        if loading {
            ui.spinner();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if scene.choices.len() > 1 {
            let current = usize::from(cur.mesh.request.choice).min(scene.choices.len() - 1);
            egui::ComboBox::from_id_salt("mesh-choice")
                .selected_text(&scene.choices[current])
                .width(360.0)
                .show_ui(ui, |ui| {
                    for (i, c) in scene.choices.iter().enumerate() {
                        if ui.selectable_label(current == i, c).clicked() {
                            cur.mesh.set_choice(i as u8);
                        }
                    }
                });
            ui.separator();
        }
        let dyeable = scene.materials.iter().any(|m| m.dyeable);
        ui.add_enabled(dyeable, egui::Checkbox::new(&mut settings.dye_on, "Dye"))
            .on_hover_text(if dyeable {
                "Preview a dye colour on the dyeable areas (material.md §12)"
            } else {
                "No dyeable material in this scene"
            });
        ui.add_enabled_ui(dyeable && settings.dye_on, |ui| {
            ui.color_edit_button_srgb(&mut settings.dye_color);
            let two = scene.materials.iter().any(|m| m.gpu.dye[1].is_some());
            if two {
                ui.selectable_value(&mut settings.dye_channel, DyeChannel::Primary, "dye 1");
                ui.selectable_value(&mut settings.dye_channel, DyeChannel::Secondary, "dye 2")
                    .on_hover_text(
                        "Only materials with a second dye channel change; the renderer \
                         draws one channel at a time",
                    );
            }
        });
        ui.separator();
        let draws: usize = scene.parts.iter().map(|p| p.submeshes.len()).sum();
        let mut stats = format!(
            "{} parts · {draws} draws · {} vertices · {} triangles · {} textures · built in {} ms",
            scene.parts.len(),
            thousands(scene.vertices as u64),
            thousands(scene.triangles as u64),
            scene.textures.len(),
            scene.build_ms
        );
        if let Some((s, ms)) = vp.stats {
            stats.push_str(&format!(
                " · render {ms:.1} ms, {} draw calls",
                s.draw_calls
            ));
        }
        ui.label(RichText::new(stats).weak());
        if let Some(a) = &vp.adapter {
            ui.label(RichText::new(format!("GPU: {a}")).weak().small());
        }
    });
}

fn viewport_widget(
    ui: &mut Ui,
    cur: &mut Current,
    settings: &mut MeshSettings,
    vp: &mut Viewport,
    scene: &Arc<SceneProduct>,
) {
    let size = ui.available_size().max(vec2(64.0, 64.0));
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let ppp = ui.ctx().pixels_per_point();
    // One scale for both sides, so the image keeps the rectangle's aspect.
    let (width, height) = fit_viewport(
        (rect.width() * ppp).round() as u32,
        (rect.height() * ppp).round() as u32,
        vp.max_side(),
    );
    let aspect = width as f32 / height as f32;
    let m = &mut cur.mesh;

    // Input.
    let (shift, scroll) = ui.input(|i| (i.modifiers.shift, i.smooth_scroll_delta.y));
    if resp.dragged() {
        let d = resp.drag_delta();
        let pan = resp.dragged_by(egui::PointerButton::Secondary)
            || resp.dragged_by(egui::PointerButton::Middle)
            || shift;
        if pan {
            m.orbit.pan(d.x, d.y, rect.height());
        } else {
            m.orbit.rotate(d.x, d.y);
        }
    }
    if resp.hovered() && scroll != 0.0 {
        m.orbit.zoom(scroll / 50.0);
    }
    if resp.double_clicked() {
        m.reframe = true;
    }
    if resp.hovered() && !ui.ctx().egui_wants_keyboard_input() {
        let (f, r, w) = ui.input(|i| {
            let plain = |k| i.modifiers.is_none() && i.key_pressed(k);
            (
                plain(egui::Key::F),
                plain(egui::Key::R),
                plain(egui::Key::W),
            )
        });
        if f {
            m.reframe = true;
        }
        if r {
            m.orbit.reset_angles();
            m.reframe = true;
        }
        if w {
            settings.wireframe = !settings.wireframe;
        }
    }

    // Framing: once per (DID, choice), and on request.
    let frame_key = (scene.did, scene.choice);
    if m.framed != Some(frame_key) || m.reframe {
        if m.framed.is_none()
            && let Some([yaw, pitch]) = settings.angles
        {
            m.orbit.orbit.yaw = yaw.to_radians();
            m.orbit.orbit.pitch = pitch.to_radians();
        }
        m.orbit.frame(scene.sphere.0, scene.sphere.1, aspect);
        m.framed = Some(frame_key);
        m.reframe = false;
    }

    let id = vp.set_scene(scene);
    m.scene_id = Some(id);
    let overlay_hidden = m
        .hidden_overlays
        .get_or_insert_with(|| {
            scene
                .collision
                .iter()
                .enumerate()
                .filter(|(_, o)| !o.kind.shown_by_default())
                .map(|(i, _)| i)
                .collect()
        })
        .clone();
    // A scene of collision only (a 0x66 record) always shows it.
    let only_collision = scene.parts.iter().all(|p| p.prepared.is_none());
    let collision = (settings.collision || only_collision).then(|| {
        let mut hidden: Vec<usize> = overlay_hidden.into_iter().collect();
        hidden.sort_unstable();
        CollisionDraw {
            hidden,
            fill: settings.collision_fill,
            xray: settings.collision_xray,
        }
    });
    let mut hidden: Vec<(usize, u32)> = m.hidden.iter().copied().collect();
    hidden.sort_unstable();
    let mut hidden_parts: Vec<usize> = m.hidden_parts.iter().copied().collect();
    hidden_parts.sort_unstable();
    vp.request(DrawRequest {
        scene: id,
        width,
        height,
        camera: m.orbit.camera(),
        shading: settings.shading,
        wireframe: settings.wireframe,
        normals: settings.normals,
        // A landblock lies far above the origin's ground grid (z is absolute).
        grid: settings.grid && scene.source != crate::scene::SceneSource::Landblock,
        highlight: settings.highlight,
        isolate: settings.isolate,
        hidden_parts,
        hidden,
        selected: m.selected,
        dye: settings.dye_on.then_some(DyeOverride {
            color: settings.dye_color.map(|c| f32::from(c) / 255.0),
            channel: settings.dye_channel,
        }),
        diffuse_tint: settings.diffuse_tint,
        explode: if scene.parts.len() > 1 {
            settings.explode
        } else {
            0.0
        },
        pose: None,
        collision,
    });

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 15, 17));
    if let Some(t) = vp.texture() {
        painter.image(
            t.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    let mut overlay = Vec::new();
    if let Some(e) = vp.error() {
        overlay.push((e.to_owned(), Color32::LIGHT_RED));
    } else if vp.waiting() && vp.texture().is_none() {
        overlay.push(("rendering …".to_owned(), Color32::GRAY));
    }
    if !scene.drawable() {
        overlay.push((
            "nothing to draw (see the notes on the right)".to_owned(),
            Color32::from_rgb(0xE0, 0xB0, 0x60),
        ));
    }
    if let Some((problems, _)) = vp.loaded(id)
        && !problems.is_empty()
    {
        overlay.push((
            format!("{} upload problems: {}", problems.len(), problems[0]),
            Color32::from_rgb(0xE0, 0xB0, 0x60),
        ));
    }
    let mut y = rect.top() + 6.0;
    for (text, color) in overlay {
        painter.text(
            pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(13.0),
            color,
        );
        y += 18.0;
    }
    painter.text(
        pos2(rect.left() + 8.0, rect.bottom() - 6.0),
        egui::Align2::LEFT_BOTTOM,
        "Z up · drag: orbit · right/middle/Shift drag: pan · wheel: zoom · F: frame · \
         R: reset · W: wireframe",
        egui::FontId::proportional(11.0),
        Color32::from_gray(120),
    );
    if vp.waiting() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

/// The collision overlays with a visibility box each.
fn collision_list(ui: &mut Ui, cur: &mut Current, scene: &SceneProduct) {
    ui.separator();
    ui.label(RichText::new("Collision").strong());
    let hidden = cur.mesh.hidden_overlays.get_or_insert_with(|| {
        scene
            .collision
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.kind.shown_by_default())
            .map(|(i, _)| i)
            .collect()
    });
    for (i, o) in scene.collision.iter().enumerate() {
        ui.horizontal(|ui| {
            let mut shown = !hidden.contains(&i);
            if ui.checkbox(&mut shown, "").changed() {
                if shown {
                    hidden.remove(&i);
                } else {
                    hidden.insert(i);
                }
            }
            let c = o.kind.color();
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(r, 2.0, srgb_color([c[0], c[1], c[2], 1.0]));
            ui.label(RichText::new(&o.caption).small())
                .on_hover_text(format!(
                    "{} · {} triangles · {} lines",
                    o.kind.name(),
                    o.triangles,
                    o.lines.len()
                ));
        });
    }
}

fn inspector(
    ui: &mut Ui,
    cur: &mut Current,
    settings: &mut MeshSettings,
    scene: &Arc<SceneProduct>,
    checker: &egui::TextureHandle,
    actions: &mut Vec<Action>,
) {
    egui::ScrollArea::vertical()
        .id_salt("mesh-inspector-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {:#010x}", scene.source.name(), scene.did)).strong(),
            );
            if !scene.chain.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("chain:");
                    for (i, link) in scene.chain.iter().enumerate() {
                        if i > 0 {
                            ui.label("→");
                        }
                        if ui
                            .link(format!("{:#010x}", link.did))
                            .on_hover_text(&link.caption)
                            .clicked()
                        {
                            actions.push(Action::Open(link.did));
                        }
                    }
                });
            }
            for n in &scene.notes {
                ui.label(
                    RichText::new(n)
                        .small()
                        .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
                );
            }
            if !scene.collision.is_empty() {
                collision_list(ui, cur, scene);
            }
            if !scene.dyes.is_empty() {
                dye_table(ui, settings, scene);
            }
            ui.separator();
            parts_list(ui, cur, scene, actions);
            ui.separator();
            let inspected = cur.mesh.selected.or_else(|| {
                scene.parts.iter().enumerate().find_map(|(pi, p)| {
                    p.submeshes
                        .iter()
                        .find(|s| s.material.is_some())
                        .map(|s| (pi, s.draw))
                })
            });
            let material = inspected.and_then(|(pi, d)| {
                let sub = scene
                    .parts
                    .get(pi)?
                    .submeshes
                    .iter()
                    .find(|s| s.draw == d)?;
                Some((pi, sub, scene.materials.get(sub.material?)?))
            });
            match material {
                Some((pi, sub, m)) => {
                    ui.heading("Material");
                    ui.label(
                        RichText::new(format!(
                            "part {pi} · draw {} · submesh {} · format {:#010x}",
                            sub.draw, sub.submesh, sub.format
                        ))
                        .weak(),
                    );
                    material_inspector(ui, cur, scene, m, checker, actions);
                }
                None => {
                    ui.label(RichText::new("No material to inspect.").weak());
                }
            }
        });
}

/// The dye colours of the entry's blocks; a click previews one.
fn dye_table(ui: &mut Ui, settings: &mut MeshSettings, scene: &SceneProduct) {
    ui.label(RichText::new("Dye blocks (click to preview)").strong());
    ui.horizontal_wrapped(|ui| {
        for d in &scene.dyes {
            for (channel, color) in [d.primary, d.secondary].into_iter().enumerate() {
                let Some(c) = color else { continue };
                let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 3.0, srgb_color(c));
                let resp = resp.on_hover_text(format!(
                    "variant {:.2} · dye {} ({:.3}, {:.3}, {:.3})",
                    d.variant,
                    channel + 1,
                    c[0],
                    c[1],
                    c[2]
                ));
                if resp.clicked() {
                    let b = srgb_color(c);
                    settings.dye_on = true;
                    settings.dye_color = [b.r(), b.g(), b.b()];
                    settings.dye_channel = DyeChannel::from_index(channel);
                }
            }
        }
    });
}

fn parts_list(
    ui: &mut Ui,
    cur: &mut Current,
    scene: &Arc<SceneProduct>,
    actions: &mut Vec<Action>,
) {
    let open_all = scene.parts.len() <= 4;
    for (pi, part) in scene.parts.iter().enumerate() {
        let title = format!(
            "{} · {:#010x} · {} draws · {} triangles",
            part.caption,
            part.mesh,
            part.submeshes.len(),
            thousands(part.triangles as u64)
        );
        egui::CollapsingHeader::new(title)
            .id_salt(("mesh-part", scene.did, pi))
            .default_open(open_all)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let mut visible = !cur.mesh.hidden_parts.contains(&pi);
                    if ui.checkbox(&mut visible, "visible").changed() {
                        if visible {
                            cur.mesh.hidden_parts.remove(&pi);
                        } else {
                            cur.mesh.hidden_parts.insert(pi);
                        }
                    }
                    if part.mesh != 0 && ui.link("open mesh").clicked() {
                        actions.push(Action::Open(part.mesh));
                    }
                    ui.label(
                        RichText::new(format!(
                            "flags {:#x} · level {}/{}",
                            part.flags,
                            part.level,
                            part.levels.len()
                        ))
                        .weak(),
                    );
                });
                if !part.levels.is_empty() {
                    let levels: Vec<String> = part
                        .levels
                        .iter()
                        .enumerate()
                        .map(|(i, l)| format!("{i}: {} {}△", l.kind, l.triangles))
                        .collect();
                    ui.label(RichText::new(levels.join(" · ")).small().weak());
                }
                if let Some(e) = &part.error {
                    ui.colored_label(Color32::LIGHT_RED, e);
                }
                for issue in &part.issues {
                    ui.label(
                        RichText::new(issue)
                            .small()
                            .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
                    );
                }
                for s in &part.submeshes {
                    ui.horizontal(|ui| {
                        let mut visible = !cur.mesh.hidden.contains(&(pi, s.draw));
                        if ui
                            .checkbox(&mut visible, "")
                            .on_hover_text("visible")
                            .changed()
                        {
                            if visible {
                                cur.mesh.hidden.remove(&(pi, s.draw));
                            } else {
                                cur.mesh.hidden.insert((pi, s.draw));
                            }
                        }
                        let selected = cur.mesh.selected == Some((pi, s.draw));
                        let mat = s
                            .material
                            .and_then(|i| scene.materials.get(i))
                            .map_or("—", |m| m.title.as_str());
                        let text = format!(
                            "#{} {} · {}△{}",
                            s.draw,
                            mat,
                            s.triangles,
                            if s.skinned { " · skinned" } else { "" }
                        );
                        if ui
                            .selectable_label(selected, RichText::new(text).monospace())
                            .clicked()
                        {
                            cur.mesh.selected = if selected { None } else { Some((pi, s.draw)) };
                        }
                    });
                }
            });
    }
}

fn link_button(ui: &mut Ui, caption: &str, did: u32, actions: &mut Vec<Action>) {
    if ui
        .small_button(format!("{caption} {did:#010x}"))
        .on_hover_text(tessera_formats::Did(did).kind().name())
        .clicked()
    {
        actions.push(Action::Open(did));
    }
}

fn material_inspector(
    ui: &mut Ui,
    cur: &mut Current,
    scene: &Arc<SceneProduct>,
    m: &SceneMaterial,
    checker: &egui::TextureHandle,
    actions: &mut Vec<Action>,
) {
    ui.label(RichText::new(&m.title).monospace().strong());
    if let Some(e) = &m.error {
        ui.colored_label(Color32::LIGHT_RED, e);
    }
    let v = &m.view;
    ui.horizontal_wrapped(|ui| {
        for (caption, did) in &v.links {
            link_button(ui, caption, *did, actions);
        }
    });
    let g = &m.gpu;
    let alpha = match g.alpha {
        AlphaKind::Opaque => "opaque".to_owned(),
        AlphaKind::Mask(t) => format!("alpha test {t}"),
        AlphaKind::TestBlend(t) => format!("alpha test {t}, blended"),
        AlphaKind::Blend => "blended".to_owned(),
    };
    ui.label(
        RichText::new(format!(
            "viewer: {alpha}{}{}{}{}{}",
            if g.two_sided { " · two-sided" } else { "" },
            if g.unlit { " · unlit" } else { "" },
            if g.normal.is_some() {
                " · normal map"
            } else {
                ""
            },
            if g.specular.is_some() {
                " · specular"
            } else {
                ""
            },
            if g.dye[0].is_some() { " · dye" } else { "" },
        ))
        .small(),
    );
    egui::Grid::new(("mat-facts", scene.did, m.title.as_str()))
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (k, val) in &v.facts {
                ui.label(RichText::new(k).weak());
                ui.label(RichText::new(val).small());
                ui.end_row();
            }
            if !v.flags.is_empty() {
                ui.label(RichText::new("flags").weak());
                ui.label(RichText::new(v.flags.join(", ")).small());
                ui.end_row();
            }
        });
    for n in &v.notes {
        ui.label(
            RichText::new(n)
                .small()
                .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
        );
    }
    if !v.slots.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("Texture slots").strong());
        for s in &v.slots {
            ui.horizontal(|ui| {
                let side = 48.0;
                let (rect, resp) = ui.allocate_exact_size(vec2(side, side), egui::Sense::click());
                let thumb = s.thumb.and_then(|i| scene.thumbs.get(i));
                if let Some(t) = thumb {
                    let key = format!("mthumb/{:08x}/{}", t.list, t.alpha);
                    let tex = cur.gpu.entry(key.clone()).or_insert_with(|| {
                        ui.ctx().load_texture(
                            key,
                            thumb_color_image(&t.image),
                            TextureOptions::LINEAR,
                        )
                    });
                    let [w, h] = [t.image.width as usize, t.image.height as usize];
                    let scale = side / (w.max(h).max(1) as f32);
                    let r = Rect::from_center_size(rect.center(), vec2(w as f32, h as f32) * scale);
                    ui.painter().image(
                        checker.id(),
                        r,
                        Rect::from_min_max(
                            pos2(0.0, 0.0),
                            pos2(r.width() / 16.0, r.height() / 16.0),
                        ),
                        Color32::WHITE,
                    );
                    ui.painter().image(
                        tex.id(),
                        r,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    let resp = resp.on_hover_text(&t.info);
                    if resp.clicked() {
                        actions.push(Action::Go(Nav {
                            did: t.list,
                            member: None,
                            view: ViewKind::Texture,
                        }));
                    }
                } else {
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        (1.0, Color32::from_gray(60)),
                        egui::StrokeKind::Inside,
                    );
                }
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{} {}", s.id, s.kind)).strong());
                        ui.label(RichText::new(&s.role).monospace());
                        if let Some(t) = s.texture
                            && ui.link(format!("{t:#010x}")).clicked()
                        {
                            actions.push(Action::Go(Nav {
                                did: t,
                                member: None,
                                view: ViewKind::Texture,
                            }));
                        }
                    });
                    ui.label(
                        RichText::new(format!(
                            "{} · used as: {} · {} · {}",
                            s.packing, s.used_as, s.sampler, s.origin
                        ))
                        .small()
                        .weak(),
                    );
                });
            });
        }
    }
    if !v.params.is_empty() {
        ui.add_space(4.0);
        egui::CollapsingHeader::new(format!("Parameters ({})", v.params.len()))
            .id_salt(("mat-params", scene.did, m.title.as_str()))
            .default_open(v.params.len() <= 24)
            .show(ui, |ui| {
                egui::Grid::new(("mat-params-grid", scene.did, m.title.as_str()))
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        for p in &v.params {
                            ui.label(RichText::new(p.id.to_string()).monospace());
                            ui.label(RichText::new(p.name).small());
                            ui.horizontal(|ui| {
                                if let Some(c) = p.color {
                                    let (r, _) = ui.allocate_exact_size(
                                        vec2(14.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(r, 2.0, srgb_color(c));
                                }
                                match p.link {
                                    Some(t) => {
                                        if ui.link(RichText::new(&p.value).monospace()).clicked() {
                                            actions.push(Action::Go(Nav {
                                                did: t,
                                                member: None,
                                                view: ViewKind::Texture,
                                            }));
                                        }
                                    }
                                    None => {
                                        ui.label(RichText::new(&p.value).monospace().small());
                                    }
                                }
                            });
                            ui.label(RichText::new(&p.origin).small().weak());
                            ui.end_row();
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_to_settings() {
        let o = MeshOptions {
            shading: Some(ShadingArg::Wire),
            dye: Some([1, 2, 3]),
            lod: Some(2),
            choice: Some(1),
            ..MeshOptions::default()
        };
        let s = o.settings();
        assert_eq!(s.shading, MeshShading::Hidden);
        assert!(s.wireframe && s.dye_on);
        assert_eq!(s.dye_color, [1, 2, 3]);
        let r = o.start().request;
        assert_eq!((r.lod, r.choice, r.vertex_colors), (2, 1, false));
        assert_eq!(MeshOptions::default().settings(), MeshSettings::default());
        // Vertex colours and the diffuse tint are opt-in (both OPEN in the specs).
        let d = MeshSettings::default();
        assert!(!d.vertex_colors && !d.diffuse_tint);
        let on = MeshOptions {
            vertex_colors: true,
            diffuse_tint: true,
            ..MeshOptions::default()
        };
        assert!(on.settings().vertex_colors && on.settings().diffuse_tint);
        assert!(on.start().request.vertex_colors);
    }

    #[test]
    fn a_level_change_clears_the_draw_selection() {
        // Regression: draw numbers of one level selected or hid other draws at another.
        let mut m = MeshState::new(SceneRequest::default());
        m.selected = Some((0, 1));
        m.hidden.insert((0, 1));
        m.hidden_parts.insert(2);
        m.set_lod(0);
        assert_eq!(m.selected, Some((0, 1)), "same level: kept");
        m.set_lod(1);
        assert_eq!(m.request.lod, 1);
        assert!(m.selected.is_none() && m.hidden.is_empty() && m.hidden_parts.is_empty());
        m.selected = Some((0, 0));
        m.set_choice(3);
        assert_eq!(m.request.choice, 3);
        assert!(m.selected.is_none());
    }

    #[test]
    fn a_choice_change_resets_the_hidden_overlays() {
        // Regression: overlay indices of one alternative hid or showed overlays of another.
        let mut m = MeshState::new(SceneRequest::default());
        m.hidden_overlays = Some(HashSet::from([0]));
        m.set_choice(0);
        assert_eq!(
            m.hidden_overlays,
            Some(HashSet::from([0])),
            "same choice: kept"
        );
        m.set_lod(2);
        assert!(
            m.hidden_overlays.is_some(),
            "a level shows the same collision"
        );
        m.set_choice(1);
        assert!(m.hidden_overlays.is_none());
    }

    #[test]
    fn thumbnails_convert_without_panics() {
        let ok = tessera_formats::RgbaImage {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };
        assert_eq!(thumb_color_image(&ok).size, [2, 1]);
        let bad = tessera_formats::RgbaImage {
            width: 9,
            height: 9,
            pixels: vec![1, 2, 3],
        };
        assert_eq!(thumb_color_image(&bad).size, [1, 1]);
    }

    #[test]
    fn colours_are_clamped() {
        assert_eq!(
            srgb_color([2.0, -1.0, f32::NAN, 1.0]),
            Color32::from_rgb(255, 0, 0)
        );
    }
}
