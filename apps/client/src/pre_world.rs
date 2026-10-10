// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's start-up before world entry with the character screens
//! (`--character-select`; decision 0024 P1, `character-screens.md` §6): instead of entering
//! the first (or `--character`) character on `Connected`, the session goes to the
//! [`CharacterScreens`] of `crate::screens`, shown in the game UI by their [`ScreenUi`],
//! until the player enters the world (or quits).
//!
//! While the screens are open they own the session: the world session does not poll
//! ([`World::set_external_poll`]); each frame [`PreWorld::frame`] pumps the screens, hands
//! the polled events to the world session ([`World::apply_polled`]) so its entity table,
//! chat and player state stay current, syncs the UI and turns the frame's UI events into
//! the player's actions. Escape is the selection screen's quit (`crate::screens::ui`
//! module docs). On `WorldEntered` the screens close and the world session polls again.
//!
//! **Not done here** (`character-screens.md` §6 implementer OPEN (1)): the viewport's figure
//! is not drawn; the screens' layouts, lists and controls are.

use std::sync::Arc;
use std::time::Instant;

use tessera_net::Event;
use tessera_ui::{Ui, UiEvent};

use crate::character::CharacterData;
use crate::net::World;
use crate::screens::data::InstallTexts;
use crate::screens::ui::{ChargenRefs, ScreenUi};
use crate::screens::{Action, CharacterScreens, ScreenData, ScreenOptions, Status};

/// How the pre-world flow stands after a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreWorldState {
    /// The screens are open.
    Open,
    /// The world was entered with this character: the screens closed.
    Entered(String),
    /// The player quit, or the session ended (the reason): the client ends.
    Ended(String),
}

/// The character screens before world entry (module docs).
#[derive(Debug)]
pub struct PreWorld {
    screens: CharacterScreens,
    ui: Option<ScreenUi>,
    data: Arc<CharacterData>,
    scratch: Vec<Event>,
    state: PreWorldState,
    seed: u64,
}

impl PreWorld {
    /// Opens the screens over `world`'s session (which must not enter by itself:
    /// [`World::set_manual_enter`]) with the install's screen data; with `ui` the chargen
    /// layouts open in it (`refs` from the install's labels).
    pub fn open(
        world: &mut World,
        screen_data: Arc<ScreenData>,
        data: Arc<CharacterData>,
        options: ScreenOptions,
        ui: Option<(&mut Ui, ChargenRefs)>,
    ) -> Self {
        world.set_manual_enter(true);
        world.set_external_poll(true);
        let screens = CharacterScreens::new(world.session_mut(), screen_data, options);
        let ui = ui.map(|(ui, refs)| {
            let s = ScreenUi::open(ui, refs);
            for m in &s.missing {
                tracing::warn!("character screens: {m} not found");
            }
            s
        });
        Self {
            screens,
            ui,
            data,
            scratch: Vec::new(),
            state: PreWorldState::Open,
            seed: 0x5EED_0001,
        }
    }

    /// The screens.
    #[must_use]
    pub fn screens(&self) -> &CharacterScreens {
        &self.screens
    }

    /// Where the flow stands.
    #[must_use]
    pub fn state(&self) -> &PreWorldState {
        &self.state
    }

    /// One action of the player (Escape: [`Action::Quit`]).
    pub fn act(&mut self, action: Action, now: Instant) {
        self.screens.act(action, now);
    }

    /// One frame: the UI events of the frame become actions, the screens pump the session,
    /// the world session takes the polled events, the UI follows the model. Returns the
    /// state after the frame; on `Entered` and `Ended` the screens' layouts are closed and
    /// the world session polls again.
    pub fn frame(
        &mut self,
        world: &mut World,
        mut ui: Option<&mut Ui>,
        ui_events: &[UiEvent],
        now: Instant,
    ) -> PreWorldState {
        if self.state != PreWorldState::Open {
            return self.state.clone();
        }
        if let (Some(sui), Some(ui)) = (self.ui.as_mut(), ui.as_deref_mut()) {
            for ev in ui_events {
                self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
                for a in sui.handle_event(ui, ev, self.screens.model(), self.seed) {
                    self.screens.act(a, now);
                }
            }
        }
        self.scratch.clear();
        let mut events = std::mem::take(&mut self.scratch);
        self.screens.pump(world.session_mut(), now, &mut events);
        world.apply_polled(now, &events);
        self.scratch = events;
        if let (Some(sui), Some(ui)) = (self.ui.as_mut(), ui.as_deref_mut()) {
            let texts = InstallTexts {
                store: self.data.store(),
                strings: self.data.strings().as_ref(),
            };
            sui.sync(ui, self.screens.model(), &texts);
        }
        self.state = match self.screens.status() {
            Status::Entered(name) => PreWorldState::Entered(name.clone()),
            Status::Quit => PreWorldState::Ended("the player quit".into()),
            Status::Closed(reason) => PreWorldState::Ended(reason.clone()),
            Status::Open | Status::Entering(_) => PreWorldState::Open,
        };
        if self.state != PreWorldState::Open {
            world.set_external_poll(false);
            if let (Some(sui), Some(ui)) = (self.ui.take(), ui) {
                sui.close(ui);
            }
            self.screens.wait_saved();
            tracing::info!(state = ?self.state, "character screens closed");
        }
        self.state.clone()
    }
}
