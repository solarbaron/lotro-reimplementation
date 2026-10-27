// SPDX-License-Identifier: GPL-3.0-or-later
//! Targeting, skills and combat presentation in the client (decision 0024 P2; client half
//! only): the player's combat state from the session's events, selection by mouse and
//! targeting keys, the combat widgets of `tessera-ui` filled from `tessera-gamedata`, skill
//! and effect tooltips, the plugin combat views, and the headless combat screens of the
//! exit criteria.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`state`] | [`CombatState`]: selection, kept vitals, levels, combat state, effects, casts, quickslots, cooldowns, skills; local clearing on despawn | `combat-ui.md` §11; decision 0032 D2 |
//! | [`targeting`] | the targeting actions' `SelectTarget` | `combat-ui.md` §3.3 TR4 |
//! | [`commands`] | the combat commands of keys, clicks and widgets | `combat-ui.md` §11, TR3, TR5, TR28, TR33 |
//! | [`view`] | frame, selection, cast, effect, floating-text and slot views; [`CombatInstall`] | `combat-ui.md` §4–§10, §13 |
//! | [`hud`] | [`CombatHud`]: the widgets fed from the state; skill and effect tooltips on hover | `combat-ui.md`; `skill-tooltips.md` §12 |
//! | [`anchors`] | screen anchors: the walker's, or the synthetic camera without a world | `combat-ui.md` §6.4, §13.6 |
//! | [`plugin`] | [`PluginCombat`]: the plugin combat views' source | `lua-plugin-api.md` §15.2 |
//! | [`boot`] | the install's combat layouts and vocabulary, resolved at UI start | `combat-ui.md` §2 |
//! | [`headless`] | the combat screens of a headless scenario run (goldens, checkpoints) | `combat-ui.md` §14, §16 |

pub mod anchors;
pub mod boot;
pub mod commands;
pub mod headless;
pub mod hud;
pub mod plugin;
pub mod state;
pub mod targeting;
pub mod view;

use std::sync::Arc;
use std::time::Instant;

pub use commands::CombatInput;
pub use hud::CombatHud;
pub use plugin::PluginCombat;
pub use state::{CombatChange, CombatState};
pub use view::CombatInstall;

use crate::character::CharacterData;

/// The runtime's P2 glue: the install's combat data (once the P1 facts have loaded) and the
/// plugin combat views fed with each frame's events.
#[derive(Debug, Default)]
pub struct CombatHub {
    install: Option<Arc<CombatInstall>>,
    plugin: PluginCombat,
    failed: bool,
    /// The install data was built and no consumer has taken the news yet
    /// ([`Self::take_arrived`]).
    arrived: bool,
}

impl CombatHub {
    /// Nothing loaded yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the combat data over the loaded install facts (once).
    pub fn set_data(&mut self, data: &Arc<CharacterData>) {
        if self.install.is_some() || self.failed {
            return;
        }
        match CombatInstall::new(Arc::clone(data)) {
            Ok(i) => {
                self.install = Some(Arc::new(i));
                self.arrived = true;
            }
            Err(e) => {
                self.failed = true;
                tracing::warn!("combat data unavailable: {e}");
            }
        }
    }

    /// Whether the install's combat data was built since the last call (the flag is
    /// cleared): a HUD attached before it rebuilds its displays with it.
    pub fn take_arrived(&mut self) -> bool {
        std::mem::take(&mut self.arrived)
    }

    /// The install's combat data, once built.
    #[must_use]
    pub fn install(&self) -> Option<&Arc<CombatInstall>> {
        self.install.as_ref()
    }

    /// Hands the plugin manager the combat views' source (`lua-plugin-api.md` §15.2).
    pub fn attach_plugins(&self, manager: &mut tessera_plugins::PluginManager) {
        self.plugin.attach(manager);
    }

    /// One frame's session events for the plugin views, at plugin game time `game_time`.
    pub fn after_events(
        &mut self,
        events: &[tessera_net::Event],
        game_time: f64,
        manager: &mut tessera_plugins::PluginManager,
    ) {
        if events.is_empty() {
            return;
        }
        let install = self.install.clone();
        self.plugin
            .observe(events, game_time, install.as_deref(), manager);
    }
}

/// Seconds of `now` since `base` (the combat clock; never negative).
#[must_use]
pub fn seconds_since(base: Instant, now: Instant) -> f64 {
    now.saturating_duration_since(base).as_secs_f64()
}

/// Whether a session event matters to the combat state or the plugin combat views (the
/// events the world session keeps for [`CombatHub::after_events`]).
#[must_use]
pub fn is_combat_relevant(ev: &tessera_net::Event) -> bool {
    use tessera_net::{CharacterEvent, Event, ProgressionEvent};
    match ev {
        Event::WorldEntered { .. }
        | Event::Disconnected { .. }
        | Event::ReturnedToCharacterSelect { .. }
        | Event::EntitySpawned { .. }
        | Event::EntityCreated { .. }
        | Event::EntityDespawned { .. }
        | Event::StaticsCleared
        | Event::Combat(_) => true,
        Event::Character(c) => matches!(
            c,
            CharacterEvent::CharacterInfo { .. } | CharacterEvent::LevelChanged { .. }
        ),
        Event::Progression(p) => matches!(
            p,
            ProgressionEvent::SkillLearned { .. } | ProgressionEvent::SkillForgotten { .. }
        ),
        _ => false,
    }
}
