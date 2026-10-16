// SPDX-License-Identifier: GPL-3.0-or-later
//! The experience bar and the level-up banner (`progression-panels.md` §2; see [`super`]).
//!
//! The bar works on the install's toolbar meter (§2.1): the first element of the `LevelProgressMeter`
//! game type below the panel root (`game-panels.md` §11; an engine Meter, `ui-widgets.md`
//! §13 P3), whose `UICore_Meter_position` takes the fill and whose tooltip the value text;
//! or on Tessera's fallback bar ([`super::fallback::experience_bar`]), whose fill is sized
//! and whose level and value texts are set.
//!
//! The level-up banner is Tessera's own visual presentation of a level-up
//! (`progression-panels.md` §2.3 Decision; `scenario-feed.md` §6.6: a `LevelChanged` for the
//! player that raises the level and is not `quiet`; the client decides that, this widget only
//! presents it; the chat notice and audio cue of §2.3 are the client's, not this widget's):
//! shown for [`LEVELUP_SECONDS`] on the
//! client's clock, fading in over [`FADE_IN`] and out over [`FADE_OUT`] through the
//! element's opacity. Its state at a time is a pure function of the start time, so a golden
//! image freezes it at a fixed virtual time.

use tessera_formats::property::PropertyValue;

use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::npc::kit::{find, put_text};
use super::super::p6vocab::own;
use super::super::vocab::GameVocab;
use super::ExperienceView;

/// How long the level-up banner is shown, in seconds (Tessera).
pub const LEVELUP_SECONDS: f64 = 5.0;

/// The banner's fade-in, in seconds (Tessera).
pub const FADE_IN: f64 = 0.5;

/// The banner's fade-out at the end, in seconds (Tessera).
pub const FADE_OUT: f64 = 1.0;

/// The experience bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceBar {
    /// The panel root the bar was found in (or the fallback bar's root).
    pub root: ElementKey,
    /// The install's meter (`LevelProgressMeter`), when found.
    pub meter: Option<ElementKey>,
    /// The fallback bar's back.
    pub back: Option<ElementKey>,
    /// The fallback bar's fill.
    pub fill: Option<ElementKey>,
    /// The fallback bar's level text.
    pub level: Option<ElementKey>,
    /// The fallback bar's value text.
    pub text: Option<ElementKey>,
    shown: Option<ExperienceView>,
}

impl ExperienceBar {
    /// Finds the bar below `root`: the first `LevelProgressMeter` element (the install's
    /// toolbar) or the fallback bar's parts.
    #[must_use]
    pub fn attach(ui: &Ui, gv: &GameVocab, root: ElementKey) -> Self {
        let ty = gv.types.level_progress_meter;
        let meter = super::super::subtree(ui, root)
            .into_iter()
            .find(|e| ty != 0 && ui.element_type(*e) == Some(ty));
        Self {
            root,
            meter,
            back: find(ui, root, own::XP_BACK),
            fill: find(ui, root, own::XP_FILL),
            level: find(ui, root, own::XP_LEVEL),
            text: find(ui, root, own::XP_TEXT),
            shown: None,
        }
    }

    /// Whether the bar found a meter or the fallback parts.
    #[must_use]
    pub fn is_attached(&self) -> bool {
        self.meter.is_some() || self.fill.is_some()
    }

    /// The view last shown.
    #[must_use]
    pub fn shown(&self) -> Option<&ExperienceView> {
        self.shown.as_ref()
    }

    /// Shows `view`: the fill, the level and value texts and the tooltip.
    pub fn show(&mut self, ui: &mut Ui, view: &ExperienceView) {
        if self.shown.as_ref() == Some(view) {
            return;
        }
        let p = view.position();
        if let Some(m) = self.meter {
            if ui.class(m) == Some(WidgetClass::Meter) {
                let pid = ui.vocab().pids.meter_position;
                ui.set_property(m, pid, PropertyValue::Float(p));
            }
            let tip = if view.tooltip.is_empty() {
                format!("{} {}", view.level_text, view.text)
            } else {
                view.tooltip.clone()
            };
            ui.set_tooltip_text(m, Some(super::super::npc::kit::cut(&tip)));
        }
        if let (Some(b), Some(f)) = (self.back, self.fill) {
            let bb = ui.layout_box(b).unwrap_or_default();
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let w = (bb.w() as f32 * p) as i32;
            ui.set_layout_box(f, IBox::from_xywh(0, 0, w, bb.h()));
        }
        put_text(ui, self.level, &view.level_text);
        put_text(ui, self.text, &view.text);
        if !view.tooltip.is_empty() {
            ui.set_tooltip_text(self.root, Some(super::super::npc::kit::cut(&view.tooltip)));
        }
        self.shown = Some(view.clone());
    }

    /// The fill width of the fallback bar in pixels (for tests and inspection).
    #[must_use]
    pub fn fill_width(&self, ui: &Ui) -> Option<i32> {
        self.fill.and_then(|f| ui.layout_box(f)).map(|b| b.w())
    }
}

/// What the level-up banner shows at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BannerPhase {
    /// Not shown.
    Hidden,
    /// Shown with this opacity (0 … 1).
    Shown(f32),
}

/// The banner's phase `t` seconds after its start (Tessera rule): fading in over
/// [`FADE_IN`], fully shown, fading out over the last [`FADE_OUT`] of [`LEVELUP_SECONDS`],
/// then hidden. A negative or non-finite `t` is hidden.
#[must_use]
pub fn banner_phase(t: f64) -> BannerPhase {
    if !t.is_finite() || t < 0.0 || t >= LEVELUP_SECONDS {
        return BannerPhase::Hidden;
    }
    let o = if t < FADE_IN {
        t / FADE_IN
    } else if t > LEVELUP_SECONDS - FADE_OUT {
        (LEVELUP_SECONDS - t) / FADE_OUT
    } else {
        1.0
    };
    #[allow(clippy::cast_possible_truncation)]
    BannerPhase::Shown(o.clamp(0.0, 1.0) as f32)
}

/// The level-up banner.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelUpBanner {
    /// The banner's root.
    pub root: ElementKey,
    /// The level text.
    pub text: Option<ElementKey>,
    started: Option<(f64, u32)>,
}

impl LevelUpBanner {
    /// Finds the banner's parts below `root` (the fallback banner).
    #[must_use]
    pub fn attach(ui: &Ui, root: ElementKey) -> Self {
        Self {
            root,
            text: find(ui, root, own::LEVELUP_TEXT),
            started: None,
        }
    }

    /// Starts the presentation of a level-up to `level` at `now` (seconds on the client's
    /// clock) with the client's text; a later level-up restarts it.
    pub fn start(&mut self, ui: &mut Ui, now: f64, level: u32, text: &str) {
        self.started = Some((now, level));
        put_text(ui, self.text, text);
        self.update(ui, now);
    }

    /// The level being presented, while shown.
    #[must_use]
    pub fn level(&self) -> Option<u32> {
        self.started.map(|(_, l)| l)
    }

    /// The phase at `now`.
    #[must_use]
    pub fn phase(&self, now: f64) -> BannerPhase {
        match self.started {
            Some((t0, _)) => banner_phase(now - t0),
            None => BannerPhase::Hidden,
        }
    }

    /// Per-frame work: shows the banner with its opacity, or hides it (and forgets the
    /// level-up) once over.
    pub fn update(&mut self, ui: &mut Ui, now: f64) {
        match self.phase(now) {
            BannerPhase::Shown(o) => {
                ui.control_set_opacity(self.root, o);
                ui.set_shown(self.root, true);
            }
            BannerPhase::Hidden => {
                if self.started.is_some_and(|(t0, _)| now >= t0) {
                    self.started = None;
                }
                ui.set_shown(self.root, false);
            }
        }
    }
}
