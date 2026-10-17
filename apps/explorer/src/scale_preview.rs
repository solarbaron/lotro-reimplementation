// SPDX-License-Identifier: GPL-3.0-or-later
//! An optional UI-scale preview (`docs/specs/ui-scaling.md` §7.4, §9.2).
//!
//! This is a small, self-contained inspection tool: it reads no install data and decodes no
//! bytes. It lets the browser choose a window size, a user scale *S* and a safe-area aspect,
//! optionally a per-window node scale *p* (§7.4), then draws a sample layout the way the fit
//! rules of `ui-scaling.md` §5 would place it, so the result can be inspected at the named
//! configurations C1–C12 (§9.2). Every geometry call goes through [`tessera_ui::scaling`], so
//! the preview shows exactly what the real UI would compute: the effective scale *Se* (§4.3),
//! the UI screen *F* (§3.2), the safe area *A* (§5.1), Type A/B fit containers (§5.4),
//! fit-to-screen (§5.6) and keep-on-screen (§5.8).
//!
//! The sample panels are illustrative synthetic layouts, not any record from the install: a
//! full-cover fit root with a centred-bottom content box, two side panels, an anchored HUD
//! panel and one optionally per-window-scaled window.

use eframe::egui::{self, Color32};
use tessera_ui::scaling::screen::{MAX_WINDOW_PX, SCALE_MAX, SCALE_MIN, SCALE_STEP};
use tessera_ui::scaling::{
    ComputedBox, ContentBox, DBox, RHO, UiScreen, effective_scale, fit_to_screen, fit_type_a,
    fit_type_b, keep_on_screen, round_scale, s_auto, safe_area,
};

/// A named configuration from `ui-scaling.md` §9.2: a window size, a user scale and a
/// safe-area aspect. `safe` is `None` for the `"full"` setting and a ratio otherwise.
#[derive(Debug, Clone, Copy)]
pub struct NamedConfig {
    /// The label (`"C1"`…`"C12"`).
    pub label: &'static str,
    /// Window width in physical pixels.
    pub wp: i32,
    /// Window height in physical pixels.
    pub hp: i32,
    /// The user scale *S*.
    pub scale: f64,
    /// The safe-area aspect ratio (`None` = `"full"`, so *A* = *F*).
    pub safe: Option<f64>,
}

/// The 21:9 safe-area ratio used by the §9.2 table (its tests pass `2.4`).
pub const ASPECT_21_9: f64 = 2.4;

/// The named configurations C1–C12 of `ui-scaling.md` §9.2. C11 is headless-only in the spec
/// but is offered here for inspection.
pub const NAMED_CONFIGS: [NamedConfig; 12] = [
    NamedConfig {
        label: "C1",
        wp: 1280,
        hp: 720,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C2",
        wp: 1920,
        hp: 1080,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C3",
        wp: 1920,
        hp: 1080,
        scale: 1.25,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C4",
        wp: 2560,
        hp: 1440,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C5",
        wp: 3840,
        hp: 2160,
        scale: 1.5,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C6",
        wp: 3840,
        hp: 2160,
        scale: 2.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C7",
        wp: 3440,
        hp: 1440,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C8",
        wp: 5120,
        hp: 1440,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C9",
        wp: 5120,
        hp: 1440,
        scale: 1.0,
        safe: None,
    },
    NamedConfig {
        label: "C10",
        wp: 1600,
        hp: 1200,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C11",
        wp: 7680,
        hp: 4320,
        scale: 3.0,
        safe: Some(ASPECT_21_9),
    },
    NamedConfig {
        label: "C12",
        wp: 3840,
        hp: 2160,
        scale: 1.0,
        safe: Some(ASPECT_21_9),
    },
];

/// A choice for the safe-area aspect dropdown. Stored as a ratio, or `None` for `"full"`.
const ASPECT_CHOICES: [(&str, Option<f64>); 6] = [
    ("full", None),
    ("4:3", Some(4.0 / 3.0)),
    ("16:10", Some(1.6)),
    ("16:9", Some(16.0 / 9.0)),
    ("21:9", Some(ASPECT_21_9)),
    ("32:9", Some(32.0 / 9.0)),
];

/// The control state of the UI-scale preview. It holds only numbers chosen in the UI; it owns
/// no install data and never allocates per record, so it costs nothing while hidden.
#[derive(Debug, Clone)]
pub struct ScalePreview {
    /// Window width in physical pixels (1…[`MAX_WINDOW_PX`]).
    pub wp: i32,
    /// Window height in physical pixels.
    pub hp: i32,
    /// When set, the user scale is [`s_auto`] of `sigma` and `hp`; otherwise it is `scale`.
    pub auto: bool,
    /// The operating-system scale factor used by `auto` (§4.2). The explorer cannot know the
    /// real one for a hypothetical window, so the user sets it here.
    pub sigma: f64,
    /// The user scale *S* when `auto` is off (§4.1): 0.50…4.00, step 0.05.
    pub scale: f64,
    /// The safe-area aspect ratio (`None` = `"full"`).
    pub safe: Option<f64>,
    /// The per-window node scale *p* of the sample window (§7.4): 0.50…2.00, step 0.05.
    pub per_window: f64,
}

impl Default for ScalePreview {
    fn default() -> Self {
        // C2 (1920 × 1080 at S = 1.0, 21:9) is the reference configuration.
        Self {
            wp: 1920,
            hp: 1080,
            auto: false,
            sigma: 1.0,
            scale: 1.0,
            safe: Some(ASPECT_21_9),
            per_window: 1.0,
        }
    }
}

/// The geometry the preview derives from the current controls (`ui-scaling.md` §3–§5). All of
/// it is computed through [`tessera_ui::scaling`].
#[derive(Debug, Clone, Copy)]
pub struct Derived {
    /// The resolved user scale *S* (auto or chosen), rounded into range.
    pub s: f64,
    /// The effective scale *Se* (§4.3).
    pub se: f64,
    /// The UI screen *F* (§3.2).
    pub screen: UiScreen,
    /// The safe area *A* (§5.1), in UI units.
    pub safe_area: DBox,
}

impl ScalePreview {
    /// Applies a named configuration (§9.2) to the controls, turning auto off.
    pub fn apply(&mut self, c: &NamedConfig) {
        self.wp = c.wp;
        self.hp = c.hp;
        self.auto = false;
        self.scale = c.scale;
        self.safe = c.safe;
    }

    /// The resolved user scale *S*: [`s_auto`] when `auto`, otherwise [`round_scale`] of
    /// `scale`. Both clamp into the §4.1 range, so the result is always in 0.50…4.00.
    #[must_use]
    pub fn resolved_scale(&self) -> f64 {
        if self.auto {
            s_auto(self.sigma, self.hp)
        } else {
            round_scale(self.scale)
        }
    }

    /// Derives the screen, scale and safe area for the current controls, or `None` when the
    /// window has a zero side (a minimised window has no screen, §3.2).
    #[must_use]
    pub fn derive(&self) -> Option<Derived> {
        let s = self.resolved_scale();
        let se = effective_scale(s, self.wp, self.hp);
        let screen = UiScreen::new(self.wp, self.hp, se)?;
        let safe_area = safe_area(&screen, self.safe);
        Some(Derived {
            s,
            se,
            screen,
            safe_area,
        })
    }

    /// The sample layout for a screen: the fit root, its content box, the two side panels, the
    /// anchored HUD and the per-window-scaled window, each as a box in UI units with a colour
    /// and label. The panels are synthetic; only the placement comes from the spec's fit rules.
    #[must_use]
    pub fn sample_layout(&self, d: &Derived) -> Vec<SampleBox> {
        let (wu, hu) = d.screen.ui_size();
        let a = d.safe_area;
        let mut out = Vec::with_capacity(6);

        // 1. A full-cover fit root (Type A, §5.4), design 2560 × 1080, with a centred-bottom
        //    content box. P = F, guard G = F ∩ A = A (A ⊆ F).
        let (dw, dh) = (2560.0, 1080.0);
        let content = ContentBox {
            x: (dw - 900.0) / 2.0,
            y: dh - 220.0,
            w: 900.0,
            h: 180.0,
        };
        let fit = fit_type_a(
            f64::from(wu),
            f64::from(hu),
            (0.0, 0.0, dw, dh),
            Some(content),
            a,
            RHO,
        );
        out.push(SampleBox {
            b: fit.displayed(dw, dh, (0.0, 0.0), (1.0, 1.0)),
            kind: BoxKind::FitRoot,
            label: format!("fit root (Type A) f={:.3}", fit.f),
        });
        let cfit = fit.member(0.0, 0.0, content.x, content.y);
        out.push(SampleBox {
            b: cfit.displayed(content.w, content.h, (0.0, 0.0), (1.0, 1.0)),
            kind: BoxKind::Content,
            label: "content box".into(),
        });

        // 2. Two side panels (Type B, §5.4) laid out in the safe area A (so P = A). The height
        //    drives; design 300 × 1080.
        let computed = ComputedBox {
            left: 0.0,
            top: 0.0,
            right: a.w - 1.0,
            bottom: a.h - 1.0,
        };
        let left = fit_type_b(a.h, a.w, 300.0, 1080.0, computed, false);
        out.push(SampleBox {
            b: left.displayed(300.0, 1080.0, (a.l, a.t), (1.0, 1.0)),
            kind: BoxKind::Panel,
            label: format!("side panel L f={:.3}", left.f),
        });
        let right = fit_type_b(a.h, a.w, 300.0, 1080.0, computed, true);
        out.push(SampleBox {
            b: right.displayed(300.0, 1080.0, (a.l, a.t), (1.0, 1.0)),
            kind: BoxKind::Panel,
            label: format!("side panel R f={:.3}", right.f),
        });

        // 3. A fixed-size HUD panel anchored to the bottom centre of A (§5.8 anchoring, shown
        //    by formula): it tracks the safe area, not the full screen.
        let (hw, hh) = (400.0, 120.0);
        let hud = DBox::new(a.l + (a.w - hw) / 2.0, a.bottom() - hh - 16.0, hw, hh);
        out.push(SampleBox {
            b: hud,
            kind: BoxKind::Hud,
            label: "HUD (anchored to A)".into(),
        });

        // 4. A window with the optional per-window node scale p (§7.4): its displayed box is
        //    the design size × p about its origin, then fit-to-screen (§5.6) and keep-on-screen
        //    (§5.8).
        let p = self.per_window.clamp(0.5, 2.0);
        let (ww, wh) = (420.0, 300.0);
        let mut disp_w = ww * p;
        let mut disp_h = wh * p;
        let q = fit_to_screen(disp_w, disp_h, wu, hu);
        disp_w *= q;
        disp_h *= q;
        let (ox, oy) = (a.l + 40.0, 72.0);
        let (wx, wy) = keep_on_screen(ox, oy, disp_w, disp_h, wu, hu);
        out.push(SampleBox {
            b: DBox::new(wx, wy, disp_w, disp_h),
            kind: BoxKind::Window,
            label: if q < 1.0 {
                format!("window p={p:.2} (fit-to-screen q={q:.3})")
            } else {
                format!("window p={p:.2}")
            },
        });

        out
    }

    /// Shows the preview window. `open` is the View-menu toggle; the window closes it with its
    /// own title-bar close button.
    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("UI scale preview")
            .open(open)
            .resizable(true)
            .default_size([560.0, 560.0])
            .show(ctx, |ui| self.body(ui));
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        self.controls(ui);
        ui.separator();
        match self.derive() {
            Some(d) => {
                self.readout(ui, &d);
                ui.add_space(4.0);
                self.canvas(ui, &d);
            }
            None => {
                ui.colored_label(
                    Color32::from_rgb(0xD0, 0x80, 0x40),
                    "window has a zero side — no UI screen (§3.2)",
                );
            }
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Preset:");
            for c in &NAMED_CONFIGS {
                if ui.button(c.label).on_hover_text(preset_tip(c)).clicked() {
                    self.apply(c);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Window px:");
            ui.add(
                egui::DragValue::new(&mut self.wp)
                    .speed(8.0)
                    .range(1..=MAX_WINDOW_PX),
            );
            ui.label("×");
            ui.add(
                egui::DragValue::new(&mut self.hp)
                    .speed(8.0)
                    .range(1..=MAX_WINDOW_PX),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.auto, "auto scale");
            if self.auto {
                ui.add(
                    egui::Slider::new(&mut self.sigma, 0.5..=4.0)
                        .step_by(0.25)
                        .text("OS factor σ"),
                );
            } else {
                ui.add(
                    egui::Slider::new(&mut self.scale, SCALE_MIN..=SCALE_MAX)
                        .step_by(SCALE_STEP)
                        .text("user scale S"),
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label("Safe aspect:");
            let current = aspect_label(self.safe);
            egui::ComboBox::from_id_salt("uiscale-aspect")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (label, ratio) in ASPECT_CHOICES {
                        ui.selectable_value(&mut self.safe, ratio, label);
                    }
                });
        });
        ui.add(
            egui::Slider::new(&mut self.per_window, 0.5..=2.0)
                .step_by(0.05)
                .text("per-window scale p (§7.4)"),
        );
    }

    fn readout(&self, ui: &mut egui::Ui, d: &Derived) {
        let (wu, hu) = d.screen.ui_size();
        let a = d.safe_area;
        ui.monospace(format!(
            "S = {:.3}    Se = {:.4}    F = {} × {} UI units",
            d.s, d.se, wu, hu
        ));
        ui.monospace(format!(
            "Kx = {:.5}    Ky = {:.5}    (physical px per UI unit)",
            d.screen.kx(),
            d.screen.ky()
        ));
        ui.monospace(format!(
            "A = {} × {} at ({}, {})",
            a.w as i32, a.h as i32, a.l as i32, a.t as i32
        ));
    }

    fn canvas(&self, ui: &mut egui::Ui, d: &Derived) {
        let (wu, hu) = d.screen.ui_size();
        let avail = ui.available_size();
        // Reserve a preview area that keeps F's aspect, bounded by the available space.
        let max_w = avail.x.max(64.0);
        let max_h = avail.y.clamp(120.0, 420.0);
        let aspect = f64::from(wu) / f64::from(hu).max(1.0);
        let (mut w, mut h) = (max_w, max_w / aspect as f32);
        if h > max_h {
            h = max_h;
            w = max_h * aspect as f32;
        }
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let view = View::fit(rect, f64::from(wu), f64::from(hu));

        // F: the full UI screen.
        painter.rect_filled(rect, 0.0, Color32::from_gray(22));
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, Color32::from_gray(90)),
            egui::StrokeKind::Inside,
        );

        for s in self.sample_layout(d) {
            let r = view.rect(s.b);
            let (fill, stroke) = s.kind.colors();
            painter.rect_filled(r, 0.0, fill);
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, stroke),
                egui::StrokeKind::Inside,
            );
            if r.width() > 48.0 && r.height() > 16.0 {
                let mut tp = r.left_top();
                tp.x += 3.0;
                tp.y += 2.0;
                painter.text(
                    tp,
                    egui::Align2::LEFT_TOP,
                    &s.label,
                    egui::FontId::proportional(10.0),
                    Color32::from_gray(235),
                );
            }
        }

        // A: the safe area, drawn last as a dashed-looking bright outline over everything.
        let ar = view.rect(d.safe_area);
        painter.rect_stroke(
            ar,
            0.0,
            egui::Stroke::new(1.5, Color32::from_rgb(90, 150, 220)),
            egui::StrokeKind::Inside,
        );
    }
}

/// One box in the sample layout, in UI units.
#[derive(Debug, Clone)]
pub struct SampleBox {
    /// The displayed box, in UI units.
    pub b: DBox,
    /// What the box represents (its colour).
    pub kind: BoxKind,
    /// A short label drawn inside the box when it is large enough.
    pub label: String,
}

/// The kinds of sample box, each with its own colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    /// The full-cover fit root (Type A).
    FitRoot,
    /// The fit root's content box.
    Content,
    /// A side panel (Type B).
    Panel,
    /// The anchored HUD panel.
    Hud,
    /// The per-window-scaled window.
    Window,
}

impl BoxKind {
    /// The `(fill, stroke)` colours of this kind.
    #[must_use]
    pub fn colors(self) -> (Color32, Color32) {
        match self {
            BoxKind::FitRoot => (
                Color32::from_rgba_unmultiplied(60, 110, 70, 60),
                Color32::from_rgb(110, 200, 130),
            ),
            BoxKind::Content => (
                Color32::from_rgba_unmultiplied(120, 110, 40, 40),
                Color32::from_rgb(220, 200, 90),
            ),
            BoxKind::Panel => (
                Color32::from_rgba_unmultiplied(120, 80, 50, 90),
                Color32::from_rgb(220, 150, 90),
            ),
            BoxKind::Hud => (
                Color32::from_rgba_unmultiplied(80, 80, 130, 100),
                Color32::from_rgb(150, 150, 230),
            ),
            BoxKind::Window => (
                Color32::from_rgba_unmultiplied(120, 70, 95, 110),
                Color32::from_rgb(220, 130, 170),
            ),
        }
    }
}

/// A view transform from UI units (0…Wu, 0…Hu) to an egui screen rectangle, keeping *F*'s
/// aspect and centring it in the rectangle.
#[derive(Debug, Clone, Copy)]
struct View {
    origin: egui::Pos2,
    scale: f32,
}

impl View {
    /// Fits a UI screen of `wu` × `hu` units into `rect`, centred, with a uniform scale.
    fn fit(rect: egui::Rect, wu: f64, hu: f64) -> Self {
        let (wu, hu) = (wu.max(1.0) as f32, hu.max(1.0) as f32);
        let scale = (rect.width() / wu)
            .min(rect.height() / hu)
            .max(f32::MIN_POSITIVE);
        let origin = egui::pos2(
            rect.left() + (rect.width() - wu * scale) / 2.0,
            rect.top() + (rect.height() - hu * scale) / 2.0,
        );
        Self { origin, scale }
    }

    /// Maps a UI point to a screen position.
    fn pos(&self, ux: f64, uy: f64) -> egui::Pos2 {
        egui::pos2(
            self.origin.x + ux as f32 * self.scale,
            self.origin.y + uy as f32 * self.scale,
        )
    }

    /// Maps a UI-unit box to a screen rectangle.
    fn rect(&self, b: DBox) -> egui::Rect {
        egui::Rect::from_min_max(self.pos(b.l, b.t), self.pos(b.right(), b.bottom()))
    }
}

/// The label for a safe-aspect choice.
fn aspect_label(safe: Option<f64>) -> &'static str {
    match safe {
        None => "full",
        Some(r) => ASPECT_CHOICES
            .iter()
            .find(|(_, v)| matches!(v, Some(v) if (v - r).abs() < 1e-6))
            .map_or("custom", |(l, _)| l),
    }
}

/// A hover tip for a preset button.
fn preset_tip(c: &NamedConfig) -> String {
    let aspect = aspect_label(c.safe);
    format!("{} × {} at S={} ({})", c.wp, c.hp, c.scale, aspect)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every §9.2 preset derives the Se and F of the spec's configuration table (through the
    /// `tessera-ui` scaling core).
    #[test]
    fn presets_match_the_spec_table() {
        // (label, expected Se, expected Wu, expected Hu) from `ui-scaling.md` §9.2.
        let want = [
            ("C1", 0.9375, 1365, 768),
            ("C2", 1.0, 1920, 1080),
            ("C3", 1.25, 1536, 864),
            ("C4", 1.0, 2560, 1440),
            ("C5", 1.5, 2560, 1440),
            ("C6", 2.0, 1920, 1080),
            ("C7", 1.0, 3440, 1440),
            ("C8", 1.0, 5120, 1440),
            ("C9", 1.0, 5120, 1440),
            ("C10", 1.0, 1600, 1200),
            ("C11", 3.0, 2560, 1440),
            ("C12", 1.0, 3840, 2160),
        ];
        for (label, se, wu, hu) in want {
            let c = NAMED_CONFIGS.iter().find(|c| c.label == label).unwrap();
            let mut p = ScalePreview::default();
            p.apply(c);
            let d = p.derive().unwrap();
            assert!((d.se - se).abs() < 1e-9, "{label}: Se {} != {se}", d.se);
            assert_eq!(d.screen.ui_size(), (wu, hu), "{label}: F");
        }
    }

    /// C8's safe area is 3456 wide at x = 832 (`ui-scaling.md` §9.2, §10.3).
    #[test]
    fn c8_safe_area() {
        let c = NAMED_CONFIGS.iter().find(|c| c.label == "C8").unwrap();
        let mut p = ScalePreview::default();
        p.apply(c);
        let a = p.derive().unwrap().safe_area;
        assert_eq!((a.l, a.w), (832.0, 3456.0));
    }

    /// A minimised window (a zero side) has no screen; the preview reports it and does not
    /// panic (§3.2).
    #[test]
    fn zero_side_has_no_screen() {
        let p = ScalePreview {
            hp: 0,
            ..ScalePreview::default()
        };
        assert!(p.derive().is_none());
    }

    /// The resolved scale always lands in the §4.1 range for any control value, including
    /// non-finite and out-of-range ones, and auto never exceeds the chosen bound.
    #[test]
    fn resolved_scale_is_always_in_range() {
        for &scale in &[f64::NAN, -3.0, 0.01, 1.03, 100.0, f64::INFINITY] {
            for &auto in &[false, true] {
                for &hp in &[1, 720, 2160, 4320, MAX_WINDOW_PX + 999] {
                    let p = ScalePreview {
                        scale,
                        auto,
                        sigma: scale,
                        hp,
                        ..ScalePreview::default()
                    };
                    let s = p.resolved_scale();
                    assert!(
                        (SCALE_MIN..=SCALE_MAX).contains(&s),
                        "scale {scale} auto {auto} hp {hp} -> {s}"
                    );
                }
            }
        }
    }

    /// The sample layout never panics and stays finite for a sweep of controls, and every box
    /// has a positive, finite extent.
    #[test]
    fn sample_layout_is_finite_everywhere() {
        for &(wp, hp) in &[
            (1, 1),
            (1280, 720),
            (5120, 1440),
            (3840, 2160),
            (16384, 16384),
        ] {
            for &scale in &[0.5, 1.0, 2.0, 4.0] {
                for &safe in &[None, Some(ASPECT_21_9), Some(100.0)] {
                    for &per_window in &[0.5, 1.0, 2.0] {
                        let p = ScalePreview {
                            wp,
                            hp,
                            auto: false,
                            sigma: 1.0,
                            scale,
                            safe,
                            per_window,
                        };
                        let d = p.derive().unwrap();
                        for s in p.sample_layout(&d) {
                            assert!(
                                s.b.l.is_finite()
                                    && s.b.t.is_finite()
                                    && s.b.w.is_finite()
                                    && s.b.h.is_finite(),
                                "{wp}x{hp}@{scale} {safe:?} p={per_window}: {s:?}"
                            );
                            assert!(s.b.w > 0.0 && s.b.h > 0.0, "{:?}", s);
                        }
                    }
                }
            }
        }
    }

    /// The per-window window is kept inside the screen and fit to it: it never extends past F.
    #[test]
    fn per_window_window_stays_on_screen() {
        let p = ScalePreview {
            per_window: 2.0,
            ..ScalePreview::default()
        };
        let d = p.derive().unwrap();
        let (wu, hu) = d.screen.ui_size();
        let w = p
            .sample_layout(&d)
            .into_iter()
            .find(|s| s.kind == BoxKind::Window)
            .unwrap()
            .b;
        assert!(w.l >= -1e-6 && w.t >= -1e-6);
        assert!(w.right() <= f64::from(wu) + 1e-6 && w.bottom() <= f64::from(hu) + 1e-6);
    }

    /// The view transform maps F's corners to the fitted rectangle's corners.
    #[test]
    fn view_maps_corners() {
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(640.0, 360.0));
        let v = View::fit(rect, 1920.0, 1080.0);
        let tl = v.pos(0.0, 0.0);
        let br = v.pos(1920.0, 1080.0);
        assert!((tl.x - 10.0).abs() < 1e-3 && (tl.y - 20.0).abs() < 1e-3);
        assert!((br.x - 650.0).abs() < 1e-2 && (br.y - 380.0).abs() < 1e-2);
    }
}
