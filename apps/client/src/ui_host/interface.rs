// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's bridge to Tessera's own **Interface** window (`ui-scaling.md` §7.2): it
//! builds the window's [`InterfaceModel`] from the UI host's scaling state ([`ScaleState`])
//! and settings ([`UiPrefs`]), opens the game-styled control window of [`tessera_ui::game::
//! interface`], and converts between the window's UI-side choice enums and the settings
//! types ([`SafeAspect`], [`ArtFilter`]).
//!
//! The window is presentation only; this module holds the glue the UI host calls each frame:
//! it opens the window on request, feeds UI events to it, and turns the returned
//! [`InterfaceAction`] back into model values for the host to apply live (§4.5) and persist
//! to the `[UI]` keys (§4.4, §7.1).

use tessera_settings::{ArtFilter, SafeAspect, UiPrefs, UiScale};
use tessera_ui::Ui;
use tessera_ui::game::interface::{
    ArtChoice, AspectChoice, InterfaceModel, InterfaceWindow, scale_for_stop, stop_for_scale,
    stop_for_text, text_for_stop,
};
use tessera_ui::game::npc::kit::FallbackArt;

use super::scaling::ScaleState;

/// The optional-row settings an Interface-window action changed, to be written to the `[UI]`
/// section off the main thread (`ui-scaling.md` §4.4, §7.1). The required rows (scale, Auto,
/// safe aspect) persist through the debounced scale path; this carries the optional ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtraPersist {
    /// `ui_art_filter` (§7.6).
    pub art: ArtFilter,
    /// `ui_text_scale` (§7.5).
    pub text_scale: f32,
    /// Whether every `[UI.panels]` entry was cleared (§7.2 row 7).
    pub clear_panels: bool,
    /// Whether every `[UI.group_scale]` entry was cleared (§7.2 row 10).
    pub clear_groups: bool,
}

/// The Interface window plus the glue to build and refresh it from the host's state.
#[derive(Debug, Clone)]
pub struct InterfaceController {
    window: InterfaceWindow,
}

impl InterfaceController {
    /// Opens the window with the current scale and settings. Neutral fallback art is used
    /// (the install's own art is added by a skin through the normal media path); missing art
    /// draws the labels and button text on plain fields (`ui-scaling.md` §7.2).
    pub fn open(
        ui: &mut Ui,
        scale: &ScaleState,
        prefs: &UiPrefs,
    ) -> Result<Self, tessera_ui::game::GameError> {
        let model = model_of(scale, prefs);
        let window = InterfaceWindow::open(ui, &FallbackArt::default(), &model)?;
        Ok(Self { window })
    }

    /// The outcome of a UI event in the window.
    pub fn handle_event(
        &mut self,
        ui: &Ui,
        ev: &tessera_ui::UiEvent,
    ) -> Option<tessera_ui::game::interface::InterfaceAction> {
        self.window.handle_event(ui, ev)
    }

    /// Re-shows the window from the current scale and settings (after a live change).
    pub fn refresh(&mut self, ui: &mut Ui, scale: &ScaleState, prefs: &UiPrefs) {
        self.window.refresh(ui, &model_of(scale, prefs));
    }

    /// Closes and destroys the window.
    pub fn close(&mut self, ui: &mut Ui) {
        self.window.close(ui);
    }

    /// The UI-scale slider's scroll-bar element (for keyboard stepping and tests).
    #[must_use]
    pub fn scale_bar(&self) -> Option<tessera_ui::ElementKey> {
        self.window.scale_bar()
    }

    /// The text-size slider's scroll-bar element (`ui-scaling.md` §7.2 row 8; tests).
    #[must_use]
    pub fn text_bar(&self) -> Option<tessera_ui::ElementKey> {
        self.window.text_bar()
    }
}

/// Builds the window's model from the scaling state and settings (`ui-scaling.md` §7.2 rows).
#[must_use]
pub fn model_of(scale: &ScaleState, prefs: &UiPrefs) -> InterfaceModel {
    let auto = matches!(scale.user_scale(), UiScale::Auto);
    // The slider sits at the shown scale: the effective *Se* when the window limits *S*
    // (§4.3), else the resolved *S* (§7.2 rows 2, 4).
    let shown = if scale.is_limited() {
        scale.effective_scale() as f32
    } else {
        scale.resolved_scale() as f32
    };
    let scale_label = if auto {
        format!("Auto ({} %)", scale.display_percent())
    } else {
        format!("{} %", scale.display_percent())
    };
    let limited = scale.is_limited().then(|| {
        format!(
            "Limited to {} % by the window size",
            scale.display_percent()
        )
    });
    InterfaceModel {
        scale_stop: stop_for_scale(shown),
        scale_label,
        auto,
        limited,
        aspect: aspect_to_choice(scale.safe_aspect()),
        text_stop: stop_for_text(prefs.text_scale),
        text_label: format!("{} %", (prefs.text_scale * 100.0 + 0.5).floor() as u32),
        art: art_to_choice(prefs.art_filter),
    }
}

/// The UI scale a scale-slider stop stands for (`ui-scaling.md` §4.1).
#[must_use]
pub fn scale_of_stop(stop: u8) -> f32 {
    scale_for_stop(stop)
}

/// The text-size factor a text-slider stop stands for (`ui-scaling.md` §7.5).
#[must_use]
pub fn text_of_stop(stop: u8) -> f32 {
    text_for_stop(stop)
}

/// The window's aspect choice for a settings [`SafeAspect`], or `None` for a stored custom
/// ratio (no button marked, `ui-scaling.md` §7.2 row 6).
#[must_use]
pub fn aspect_to_choice(a: SafeAspect) -> Option<AspectChoice> {
    match a {
        SafeAspect::R4_3 => Some(AspectChoice::R4_3),
        SafeAspect::R16_10 => Some(AspectChoice::R16_10),
        SafeAspect::R16_9 => Some(AspectChoice::R16_9),
        SafeAspect::R21_9 => Some(AspectChoice::R21_9),
        SafeAspect::Full => Some(AspectChoice::Full),
        SafeAspect::Custom(_) => None,
    }
}

/// The settings [`SafeAspect`] for a window aspect choice.
#[must_use]
pub fn choice_to_aspect(c: AspectChoice) -> SafeAspect {
    match c {
        AspectChoice::R4_3 => SafeAspect::R4_3,
        AspectChoice::R16_10 => SafeAspect::R16_10,
        AspectChoice::R16_9 => SafeAspect::R16_9,
        AspectChoice::R21_9 => SafeAspect::R21_9,
        AspectChoice::Full => SafeAspect::Full,
    }
}

/// The window's art choice for a settings [`ArtFilter`].
#[must_use]
pub fn art_to_choice(a: ArtFilter) -> ArtChoice {
    match a {
        ArtFilter::Sharp => ArtChoice::Sharp,
        ArtFilter::Smooth => ArtChoice::Smooth,
        ArtFilter::Nearest => ArtChoice::Nearest,
    }
}

/// The settings [`ArtFilter`] for a window art choice.
#[must_use]
pub fn choice_to_art(c: ArtChoice) -> ArtFilter {
    match c {
        ArtChoice::Sharp => ArtFilter::Sharp,
        ArtChoice::Smooth => ArtFilter::Smooth,
        ArtChoice::Nearest => ArtFilter::Nearest,
    }
}
