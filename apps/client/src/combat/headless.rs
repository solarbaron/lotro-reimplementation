// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat screens of a headless scenario run (`combat-ui.md` §14, §16; ROADMAP P2 exit
//! criterion 1): the combat widgets fed from the driver's events on the scenario's virtual
//! clock (CUI9), laid out and painted by the client's UI host without plugins, rendered
//! offscreen at checkpoints, and checked from the UI tree (§16.1 step 3: not from pixels).
//!
//! Without an install the widgets take their fallback looks (§13) and floating elements
//! use the synthetic camera of §13.6; with one they use the install's panels (with the
//! synthetic camera too: no world is drawn in these screens).

use std::sync::Arc;
use std::time::Instant;

use tessera_net::Event;
use tessera_render::{OffscreenFormat, PassSpace, RenderError, Renderer, RgbaImage};
use tessera_ui::game::combat::{CastOwner, EffectOwner, effects::pulses};
use tessera_ui::{ElementKey, Ui};

use super::anchors::SyntheticAnchors;
use super::hud::CombatHud;
use super::state::{CombatChange, CombatState};
use super::view::CombatInstall;
use crate::ui_host::{UiHost, UiOptions};

/// The clear colour behind the UI in the screens (a dark grey, so every look shows).
pub const CLEAR: [f32; 4] = [0.08, 0.09, 0.1, 1.0];
/// Longest wall-clock wait of [`CombatScreens::settle`] at one checkpoint. The settle ends
/// on [`UiHost::settled`], never on a frame count: the paint works in time slices and on
/// the asset workers, so a loaded machine needs more frames and more time for the same
/// work; the bound only stops a UI that never settles.
pub const SETTLE_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

/// The elements of `combat-ui.md` §14 (§2 numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Element {
    /// E1 selection indicator.
    E1,
    /// E3 name block.
    E3,
    /// E5 player frame (with name, level, health, power).
    E5,
    /// E6 target frame with its vital field.
    E6Vital,
    /// E6 target frame with its non-vital field.
    E6NonVital,
    /// E7 selection's target.
    E7,
    /// E8 player effect row.
    E8,
    /// E9 target effect row.
    E9,
    /// E10 pulsing effect icon.
    E10,
    /// E11 player cast bar.
    E11,
    /// E12 target cast bar.
    E12,
    /// E13 floating text.
    E13,
    /// E15/E16 skill slot with a running cooldown.
    E16,
    /// E17 combat state.
    E17,
    /// E18 defeat panel.
    E18,
    /// E20 class resource.
    E20,
}

/// The headless combat screens (module docs).
pub struct CombatScreens {
    host: UiHost,
    state: CombatState,
    install: Option<Arc<CombatInstall>>,
    base: Option<Instant>,
    now: f64,
    screen: (u32, u32),
    changes: Vec<CombatChange>,
}

impl std::fmt::Debug for CombatScreens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatScreens")
            .field("now", &self.now)
            .field("screen", &self.screen)
            .finish_non_exhaustive()
    }
}

fn options(screen: (u32, u32)) -> UiOptions {
    UiOptions {
        screen,
        hud: false,
        ..UiOptions::default()
    }
}

impl CombatScreens {
    /// The screens without an install: the fallback looks (§13).
    #[must_use]
    pub fn without_install(screen: (u32, u32)) -> Self {
        let mut host = UiHost::without_install(&options(screen));
        host.attach_combat(tessera_ui::game::combat::FloatySettings::default(), 0.0);
        Self::from_parts(host, None, screen)
    }

    /// The screens over the install's UI data (`booted`) and combat data: the HUD panels
    /// P3–P5 and the `gameplayactivatables` roots are opened, the combat widgets attach to
    /// them.
    #[must_use]
    pub fn with_install(
        booted: crate::ui_host::boot::UiBooted,
        install: Arc<CombatInstall>,
        screen: (u32, u32),
    ) -> Self {
        let mut host = UiHost::from_boot(booted, &options(screen));
        host.attach_combat(install.floaty_settings(), 0.0);
        Self::from_parts(host, Some(install), screen)
    }

    fn from_parts(host: UiHost, install: Option<Arc<CombatInstall>>, screen: (u32, u32)) -> Self {
        Self {
            host,
            state: CombatState::new(),
            install,
            base: None,
            now: 0.0,
            screen,
            changes: Vec::new(),
        }
    }

    /// The UI host.
    #[must_use]
    pub fn host(&self) -> &UiHost {
        &self.host
    }

    fn hud(&self) -> Option<&CombatHud> {
        self.host.combat_hud()
    }

    /// The kept combat state.
    #[must_use]
    pub fn state(&self) -> &CombatState {
        &self.state
    }

    /// The screens' clock (seconds since world entry).
    #[must_use]
    pub fn now(&self) -> f64 {
        self.now
    }

    /// One driver poll's events at `now` (the driver's virtual clock): applied to the
    /// state, then the host's combat frame (changes, queued UI events, the widgets' update
    /// with the synthetic anchors; headless screens send no command). Nothing is painted:
    /// [`Self::settle`] and [`Self::render`] paint (always with the same renderer, or
    /// always without one: GPU operations of a paint without the renderer are lost).
    pub fn observe(&mut self, now: Instant, events: &[Event]) {
        // The clock starts at world entry, so it equals the log's virtual time.
        if events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
        {
            self.base = Some(now);
        }
        let base = *self.base.get_or_insert(now);
        self.now = super::seconds_since(base, now);
        self.changes.clear();
        for ev in events {
            self.state.apply(ev, self.now, &mut self.changes);
        }
        let changes = std::mem::take(&mut self.changes);
        self.combat_frame(&changes);
        self.changes = changes;
    }

    fn combat_frame(&mut self, changes: &[CombatChange]) {
        let install = self.install.clone();
        let anchors = SyntheticAnchors::new(&self.state, self.screen);
        // Headless screens have no input: whatever the widgets ask for is not sent (CUI8).
        let _ = self.host.combat_frame(
            &self.state,
            changes,
            Vec::new(),
            install.as_deref(),
            self.now,
            &anchors,
        );
    }

    /// One UI frame at the current time: the host's combat frame, then layout and paint
    /// (GPU operations applied to `renderer` when given).
    pub fn frame(&mut self, renderer: Option<&mut Renderer<'_>>) {
        self.host.set_clock(self.now);
        self.combat_frame(&[]);
        let _ = self.host.frame_without_plugins(renderer);
    }

    /// Runs frames at the current time until the UI has settled (images and fonts loaded,
    /// no paint job on the frame thread or the workers, nothing left to paint, no surface in
    /// a preview), for at most [`SETTLE_LIMIT`]; returns whether it settled.
    ///
    /// Every frame asks for the **capture settle** of `ui-scaling.md` §9.3 (revision 15): a
    /// pending change of *Se* is settled at once (§4.5 step 4) and every geometry preview as
    /// if its 150 ms settle time had passed (§6.1 item 4), while the scenario clock, and with
    /// it the UI clock that drives floating text, fades, cast bars and cooldowns, stays at the
    /// checkpoint's moment. So a checkpoint captures the settled, crisp picture of that moment,
    /// never a preview, and never content advanced by 150 ms.
    pub fn settle(&mut self, mut renderer: Option<&mut Renderer<'_>>) -> bool {
        let start = Instant::now();
        loop {
            self.request_capture_settle();
            self.frame(renderer.as_deref_mut());
            if self.host.settled() {
                // One more frame on the same clock, so that the GPU operations of the settled
                // state are applied; it changes nothing, so the UI stays settled.
                self.frame(renderer.as_deref_mut());
                if self.host.settled() {
                    return true;
                }
            }
            if start.elapsed() >= SETTLE_LIMIT {
                tracing::warn!(limit = ?SETTLE_LIMIT, "combat screens did not settle");
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Asks for the capture settle of `ui-scaling.md` §9.3 at the next frame ([`Self::settle`]):
    /// the §4.5 preview of a change of *Se* and every §6.1 geometry preview settle as if their
    /// settle time had passed; no clock moves.
    fn request_capture_settle(&mut self) {
        self.host.settle_scale_repaint();
        self.host.ui_mut().settle_previews_at_next_paint();
    }

    /// Whether the UI shows a preview (`ui-scaling.md` §4.5 step 3 or §6.1): a capture now
    /// would not be the settled picture of §9.3. `false` after [`Self::settle`] returned true.
    #[must_use]
    pub fn preview_pending(&self) -> bool {
        self.host.scale_repaint_pending() || self.host.ui().geometry_preview_pending()
    }

    /// Renders the UI offscreen at the screen size.
    ///
    /// # Errors
    ///
    /// The renderer's errors.
    pub fn render(&mut self, renderer: &mut Renderer<'_>) -> Result<RgbaImage, RenderError> {
        if !self.settle(Some(renderer)) || self.preview_pending() {
            tracing::warn!("combat screens captured before the capture settle ended");
        }
        let (w, h) = self.screen;
        let target = renderer.create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)?;
        let mut frame = renderer.begin_offscreen_frame(&target)?;
        frame.pass("scene", PassSpace::Screen).clear(CLEAR);
        self.host.draw(&mut frame, false);
        renderer.end_frame(frame)?;
        renderer.read_pixels(&target)
    }

    /// Whether the slot of shortcut index `i` is shown: the first element (in tree order)
    /// the skill displays place in that slot.
    fn slot_shown(&self, ui: &Ui, hud: &CombatHud, i: i32) -> bool {
        let mut stack = vec![ui.screen_root()];
        let mut seen = 0usize;
        while let Some(k) = stack.pop() {
            seen += 1;
            if seen > 1 << 20 {
                return false;
            }
            if hud.slots().slot_at(ui, k) == Some(i) {
                return self.shown(ui, k);
            }
            let mut c = ui.children(k);
            c.reverse();
            stack.extend(c);
        }
        false
    }

    fn shown(&self, ui: &Ui, k: ElementKey) -> bool {
        if !ui.exists(k) || !ui.is_displayed(k) {
            return false;
        }
        let b = ui.displayed_box(k);
        let (sw, sh) = ui.screen_size();
        let opacity = ui.vocab().pids.object_opacity;
        let transparent = ui
            .property(k, opacity)
            .is_some_and(|v| matches!(v, tessera_formats::PropertyValue::Float(o) if *o <= 0.0));
        b.w() > 0 && b.h() > 0 && b.r >= 0 && b.b >= 0 && b.l < sw && b.t < sh && !transparent
    }

    /// Whether a frame's name (and, when it reports one, its level) is shown as text
    /// (`combat-ui.md` §14: "with name, level"): the element is shown, draws text and
    /// the view has the value.
    fn frame_texts(&self, ui: &Ui, f: &tessera_ui::game::combat::UnitFrame) -> bool {
        let Some(v) = f.view() else {
            return false;
        };
        let text = |k: Option<ElementKey>| {
            k.is_some_and(|k| self.shown(ui, k) && ui.class(k).is_some_and(|c| c.is_text()))
        };
        let (name, level) = f.name_and_level();
        let level_ok = match v.level {
            tessera_ui::game::combat::LevelShown::Level(_) if level.is_some() => text(level),
            _ => true,
        };
        !v.name.is_empty() && text(name) && level_ok
    }

    /// TR38: in the install look, the OK button and the ghosted revive button are shown
    /// and no other button of the panel is (the fallback look draws its own two).
    fn defeat_buttons(&self, ui: &Ui, d: &tessera_ui::game::combat::DefeatPanel) -> bool {
        if d.is_fallback() {
            return true;
        }
        let (Some(panel), (Some(ok), Some(revive))) = (d.panel(), d.buttons()) else {
            return false;
        };
        let ghosted = ui.vocab().pids.button_ghosted;
        let revive_ghosted = ui.prop(revive, ghosted).bool();
        let others = tessera_ui::game::subtree(ui, panel).into_iter().any(|k| {
            k != ok
                && k != revive
                && ui.class(k) == Some(tessera_ui::WidgetClass::Button)
                && self.shown(ui, k)
        });
        self.shown(ui, ok) && self.shown(ui, revive) && revive_ghosted && !others
    }

    /// Whether element `e` of §14 is shown now (visible, opacity above 0, a non-empty box
    /// on screen), read from the UI tree.
    #[must_use]
    pub fn is_shown(&self, e: Element) -> bool {
        let ui = self.host.ui();
        let Some(hud) = self.hud() else {
            return false;
        };
        let w = hud.widgets();
        let now = self.now;
        match e {
            Element::E5 => w.player.as_ref().is_some_and(|f| {
                f.view().is_some() && self.shown(&ui, f.root()) && self.frame_texts(&ui, f)
            }),
            Element::E6Vital | Element::E6NonVital => w.target.as_ref().is_some_and(|f| {
                let want = e == Element::E6Vital;
                f.view().is_some_and(|v| v.has_vitals == want)
                    && self.shown(&ui, f.root())
                    && self.frame_texts(&ui, f)
            }),
            Element::E7 => w
                .selection_target
                .as_ref()
                .is_some_and(|f| f.view().is_some() && self.shown(&ui, f.root())),
            Element::E1 | Element::E3 => w.marker.as_ref().is_some_and(|m| {
                let (indicator, block) = m.elements();
                let k = if e == Element::E1 { indicator } else { block };
                m.view().is_some() && k.is_some_and(|k| self.shown(&ui, k))
            }),
            Element::E8 | Element::E9 => {
                let bar = if e == Element::E8 {
                    &w.player_effects
                } else {
                    &w.target_effects
                };
                bar.as_ref().is_some_and(|b| {
                    (0..2).any(|r| {
                        b.rows()
                            .shown(r, 8)
                            .iter()
                            .any(|v| b.icon(v.instance).is_some_and(|k| self.shown(&ui, k)))
                    })
                })
            }
            Element::E10 => {
                let in_combat = w.in_combat();
                [&w.player_effects, &w.target_effects]
                    .into_iter()
                    .flatten()
                    .any(|b| {
                        (0..2).any(|r| {
                            b.rows().shown(r, 8).iter().any(|v| {
                                pulses(v, now, in_combat)
                                    && b.icon(v.instance).is_some_and(|k| self.shown(&ui, k))
                            })
                        })
                    })
            }
            Element::E11 | Element::E12 => {
                let bar = if e == Element::E11 {
                    &w.player_cast
                } else {
                    &w.target_cast
                };
                bar.as_ref()
                    .is_some_and(|b| b.cast().is_some() && self.shown(&ui, b.root()))
            }
            Element::E13 => w
                .floaty
                .as_ref()
                .is_some_and(|f| f.shown().iter().any(|t| t.opacity > 0.0 && t.bx.2 > 0.0)),
            Element::E16 => (0..12).any(|i| {
                hud.slots().cooldown(i).is_some()
                    && hud.slots().frame(i).is_some()
                    && self.slot_shown(&ui, hud, i)
            }),
            Element::E17 => w.combat_state.as_ref().is_some_and(|c| {
                c.in_combat() && c.indicator().is_some_and(|k| self.shown(&ui, k))
            }),
            Element::E18 => w.defeat.as_ref().is_some_and(|d| {
                d.panel().is_some_and(|k| self.shown(&ui, k)) && self.defeat_buttons(&ui, d)
            }),
            // The resource bar when one shows it (both looks, TR11 with TR41); a pip
            // display or the wrath meter is checked by the data only.
            Element::E20 => {
                let reported = w
                    .player
                    .as_ref()
                    .and_then(|f| f.view())
                    .and_then(|v| v.resource.as_ref());
                match (reported, w.resource_bar()) {
                    (Some(tessera_ui::game::combat::ClassResource::Plain(_)), bar) => {
                        bar.is_some_and(|k| self.shown(&ui, k))
                    }
                    (Some(_), _) => true,
                    (None, _) => false,
                }
            }
        }
    }

    /// The floating texts shown now: (entity, text).
    #[must_use]
    pub fn floating_texts(&self) -> Vec<(u64, String)> {
        self.hud()
            .and_then(|h| h.widgets().floaty.as_ref())
            .map(|f| {
                f.shown()
                    .iter()
                    .filter(|t| t.opacity > 0.0)
                    .map(|t| (t.entity, t.text.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The cast bars' phases (player, target).
    #[must_use]
    pub fn cast_positions(&self) -> (Option<f32>, Option<f32>) {
        let Some(w) = self.hud().map(CombatHud::widgets) else {
            return (None, None);
        };
        let p = |o: CastOwner| {
            let b = match o {
                CastOwner::Player => &w.player_cast,
                CastOwner::Target => &w.target_cast,
            };
            b.as_ref().and_then(|b| b.position(self.now))
        };
        (p(CastOwner::Player), p(CastOwner::Target))
    }

    /// The effect counts of the player's and the target's bars.
    #[must_use]
    pub fn effect_counts(&self) -> (usize, usize) {
        let Some(w) = self.hud().map(CombatHud::widgets) else {
            return (0, 0);
        };
        let n = |o: EffectOwner| {
            let b = match o {
                EffectOwner::Player => &w.player_effects,
                EffectOwner::Target => &w.target_effects,
                EffectOwner::Member(_) => &None,
            };
            b.as_ref().map_or(0, |b| b.rows().len())
        };
        (n(EffectOwner::Player), n(EffectOwner::Target))
    }
}

/// Plays `scenario` headless over a sandbox with `config` (the driver of
/// `scenario-feed.md` §12.2, on its virtual clock), feeding every poll to `screens` and
/// calling `at` after each poll of world time (milliseconds). Writes the canonical log to
/// `log` when given and returns the driver's outcome and the log.
///
/// # Errors
///
/// When the sandbox refuses the configuration.
pub fn play<W: std::io::Write>(
    config: tessera_net::sandbox::SandboxConfig,
    scenario: Arc<tessera_net::scenario::ResolvedScenario>,
    log: Option<W>,
    screens: &mut CombatScreens,
    mut at: impl FnMut(u64, &mut CombatScreens),
) -> Result<(tessera_net::scenario::DriverOutcome, Option<W>), tessera_net::NetError> {
    use tessera_net::scenario::{HeadlessDriver, LogTime};
    let backend = tessera_net::sandbox::SandboxBackend::new(config)?;
    let mut driver = HeadlessDriver::new(tessera_net::Session::new(backend), scenario, log);
    let outcome = driver.run_frames(|f| {
        screens.observe(f.now, f.events);
        if let LogTime::At(ms) = f.time {
            at(ms, screens);
        }
    });
    Ok((outcome, driver.into_log()))
}

/// The checkpoints of `combat-ui.md` §17.1 (virtual seconds) and the elements each must
/// show (§14).
pub const REFERENCE_CHECKPOINTS: [(f64, &[Element]); 11] = [
    (0.5, &[Element::E5, Element::E20]),
    (1.2, &[Element::E6NonVital, Element::E1, Element::E3]),
    (2.1, &[Element::E6Vital, Element::E7, Element::E17]),
    (2.5, &[Element::E11]),
    (3.1, &[Element::E11, Element::E13, Element::E16]),
    (3.4, &[Element::E8, Element::E9, Element::E10]),
    (3.8, &[Element::E13]),
    (4.0, &[Element::E13]),
    (4.3, &[Element::E12]),
    (4.7, &[Element::E13]),
    (8.1, &[Element::E18]),
];
