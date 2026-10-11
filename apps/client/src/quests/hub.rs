// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's quest-journal glue (windowed client): builds the journal windows' names
//! once the NPC windows' names and facts are loaded ([`crate::npc::NpcHub`] loads them off
//! the frame), and runs the windows ([`QuestUi`]) on the game UI each frame: the session's
//! quest notes become quest chat lines (chat type 21, `quest-journal.md` §7.1) through the
//! UI host's session lines, the windows' commands go to the world session, and the quest
//! markers follow the radars and the world map (§8.3).
//!
//! Frame order (after the NPC windows' frame): [`QuestHub::frame`] with the frame's UI
//! events (the NPC hub collects them, [`NpcHub::frame_events`]) and the walker's radar
//! input; then [`QuestHub::update_indicators`] with the walker's entity positions; in the
//! draw, [`QuestHub::draw_indicators`] after the walker's scene and before the game UI.
//!
//! `ToggleQuestPanel` (`npc-windows.md` NW10: key L, no modifier, `Main`) is
//! [`QuestHub::log_key`]: the hub follows the modifier keys and toggles on L alone.
//! `tessera-input`'s action registry has no `ToggleQuestPanel` action yet, so a keymap
//! binding of it is not read (the default binding only).

use std::sync::Arc;
use std::time::Instant;

use tessera_input::{KeyCode, RawEvent};
use tessera_net::{ChatChannel, Command, EntityId};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{Frame, PassSpace, Renderer};
use tessera_ui::UiEvent;
use tessera_ui::game::npc::kit::FallbackArt;
use tessera_ui::game::radar::RadarInput;

use super::indicators::{Indicator, IndicatorOverlay, anchored};
use super::present::{JournalView, RING_PROPERTY};
use super::refs::QuestRefs;
use super::ui::{QUEST_CHAT_TYPE, QuestUi};
use crate::npc::NpcHub;
use crate::ui_host::UiHost;

/// A journal window `--open-panel` names (`quest-journal.md` §4.1 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JournalPanel {
    /// `quest-log`: the quest log P9.
    Log,
    /// `tracker`: the quest tracker P11.
    Tracker,
}

/// Parses a journal window name of `--open-panel`: `quest-log` or `tracker` (ASCII case
/// ignored).
#[must_use]
pub fn parse_journal_panel(name: &str) -> Option<JournalPanel> {
    match name.trim().to_ascii_lowercase().as_str() {
        "quest-log" => Some(JournalPanel::Log),
        "tracker" => Some(JournalPanel::Tracker),
        _ => None,
    }
}

/// The input key of `ToggleQuestPanel` (L by default, `npc-windows.md` NW10).
pub const LOG_KEY: tessera_input::KeyCode = tessera_input::KeyCode::L;

/// Whether `ev` is a press of [`LOG_KEY`] (modifiers not considered; [`LogKey`] does).
#[must_use]
pub fn is_log_key(ev: &RawEvent) -> bool {
    matches!(
        ev,
        RawEvent::Key {
            key: LOG_KEY,
            pressed: true,
            repeat: false,
        }
    )
}

/// The modifier keys, each tracked on its own (left and right).
const MODIFIER_KEYS: [KeyCode; 8] = [
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::SuperLeft,
    KeyCode::SuperRight,
];

/// `ToggleQuestPanel`'s default binding (NW10: [`LOG_KEY`] with no modifier): follows the
/// modifier keys and reports a press of L while none is held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogKey {
    held: [bool; 8],
}

impl LogKey {
    /// Takes one raw event; `true` when it triggers `ToggleQuestPanel`.
    pub fn event(&mut self, ev: &RawEvent) -> bool {
        let RawEvent::Key {
            key,
            pressed,
            repeat,
        } = ev
        else {
            return false;
        };
        if let Some(i) = MODIFIER_KEYS.iter().position(|k| k == key) {
            self.held[i] = *pressed;
            return false;
        }
        *key == LOG_KEY && *pressed && !*repeat && !self.held.iter().any(|h| *h)
    }

    /// Forgets the held modifiers (the window lost the focus: their releases may not
    /// arrive).
    pub fn release_all(&mut self) {
        self.held = [false; 8];
    }
}

/// Scale of the indicator glyphs (the walker HUD's bitmap font, Tessera choice).
pub const INDICATOR_SCALE: u32 = 2;

/// The runtime's quest-journal glue (module docs).
#[derive(Debug)]
pub struct QuestHub {
    refs: Option<Arc<QuestRefs>>,
    ui: Option<QuestUi>,
    pending: Vec<JournalPanel>,
    toggle: bool,
    log_key: LogKey,
    /// The overhead quest icons setting (§8.2, default on).
    icons: bool,
    /// The ringed entities and their keys (recomputed when the journal says rings changed).
    rings: Vec<(EntityId, &'static str)>,
    rings_stale: bool,
    shown: Vec<Indicator>,
    overlay: IndicatorOverlay,
    overlay_ready: bool,
    /// The commands the last frame handed to the world session.
    sent: Vec<Command>,
}

impl Default for QuestHub {
    fn default() -> Self {
        Self {
            refs: None,
            ui: None,
            pending: Vec::new(),
            toggle: false,
            log_key: LogKey::default(),
            icons: true,
            rings: Vec::new(),
            rings_stale: true,
            shown: Vec::new(),
            overlay: IndicatorOverlay::default(),
            overlay_ready: false,
            sent: Vec::new(),
        }
    }
}

impl QuestHub {
    /// Nothing built; `panels` (from `--open-panel`) open once the player is in the world.
    #[must_use]
    pub fn new(panels: Vec<JournalPanel>) -> Self {
        Self {
            pending: panels,
            ..Self::default()
        }
    }

    /// With the overhead quest icons setting `on` (§8.2; default on).
    #[must_use]
    pub fn with_icons(mut self, on: bool) -> Self {
        self.icons = on;
        self
    }

    /// Takes one raw event for `ToggleQuestPanel` ([`LogKey`]); `true` when it triggers
    /// (the caller requests the toggle unless a text field has the keyboard).
    pub fn log_key(&mut self, ev: &RawEvent) -> bool {
        self.log_key.event(ev)
    }

    /// The window lost the focus: held modifiers are forgotten.
    pub fn release_keys(&mut self) {
        self.log_key.release_all();
    }

    /// The commands the last frame handed to the world session.
    #[must_use]
    pub fn sent(&self) -> &[Command] {
        &self.sent
    }

    /// The indicators of the last [`QuestHub::update_indicators`].
    #[must_use]
    pub fn indicators(&self) -> &[Indicator] {
        &self.shown
    }

    /// The windows, once built.
    #[must_use]
    pub fn ui(&self) -> Option<&QuestUi> {
        self.ui.as_ref()
    }

    /// `ToggleQuestPanel` was triggered; the log opens or closes in the next frame.
    pub fn request_toggle(&mut self) {
        self.toggle = true;
    }

    /// Builds the names once the NPC windows' names are loaded (and, with an install,
    /// their facts): the install's names, or the fallback windows' without an install.
    fn ensure_refs(&mut self, npc: &NpcHub) -> Option<Arc<QuestRefs>> {
        if self.refs.is_none() {
            let nrefs = npc.refs()?;
            let refs = if nrefs.has_install() {
                match npc.data() {
                    Some(d) => QuestRefs::from_install(nrefs, d),
                    None if npc.failure().is_some() => QuestRefs {
                        nv: nrefs.nv.clone(),
                        ..QuestRefs::fallback()
                    },
                    None => return None,
                }
            } else {
                QuestRefs {
                    nv: nrefs.nv.clone(),
                    ..QuestRefs::fallback()
                }
            };
            if !refs.missing.is_empty() {
                tracing::warn!(missing = ?refs.missing, "quest journal without some install names");
            }
            self.refs = Some(Arc::new(refs));
        }
        self.refs.clone()
    }

    /// One frame of the journal on `host` (module docs). `events`: the frame's UI events;
    /// `radar`: the walker's radar input (the markers follow it).
    pub fn frame(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        npc: &NpcHub,
        events: &[UiEvent],
        radar: Option<&RadarInput>,
        now: Instant,
    ) {
        let Some(refs) = self.ensure_refs(npc) else {
            return;
        };
        self.sent.clear();
        if world.player().is_none() {
            return;
        }
        let data = npc.data().cloned();
        // Only the ring property of other entities is kept once it is known (session.rs).
        if let Some(pid) = data
            .as_deref()
            .and_then(|d| d.presenter().quest().data().pid(RING_PROPERTY))
        {
            world.player_state().npc().quests().set_ring_property(pid);
        }
        let ui = self
            .ui
            .get_or_insert_with(|| QuestUi::new(refs, FallbackArt::default()));
        let out = {
            let view = JournalView::new(world.player_state(), data.as_deref());
            let mut hui = host.ui_mut();
            for p in std::mem::take(&mut self.pending) {
                let r = match p {
                    JournalPanel::Log => ui.open_log(&mut hui, &view).map(|_| ()),
                    JournalPanel::Tracker => ui.open_tracker(&mut hui, &view).map(|_| ()),
                };
                if let Err(e) = r {
                    tracing::warn!("{e}");
                }
            }
            if std::mem::take(&mut self.toggle)
                && let Err(e) = ui.toggle_log(&mut hui, &view)
            {
                tracing::warn!("{e}");
            }
            ui.frame(&mut hui, &view, events)
        };
        self.rings_stale |= out.rings_changed;
        if ui.has_markers()
            && let (Some(panels), Some(input)) = (host.panels(), radar)
        {
            ui.update_markers(&mut host.ui_mut(), &panels.game, input);
        }
        for c in out.commands {
            match world.send_command(now, c.clone()) {
                Ok(()) => self.sent.push(c),
                Err(e) => tracing::info!("quest command not sent: {e}"),
            }
        }
        if !out.lines.is_empty() {
            host.post_session_lines(out.lines.into_iter().map(|text| crate::net::ChatLine {
                sender: String::new(),
                text,
                channel: ChatChannel::System,
                chat_type: QUEST_CHAT_TYPE,
            }));
        }
        if !out.notices.is_empty() {
            host.post_notices(out.notices);
        }
    }
}

impl QuestHub {
    /// The indicators over ringed NPCs for this frame (§8.2): the ring list is recomputed
    /// when the journal reported a ring change, the anchors follow `position` (the walker's
    /// render-space entity positions) every frame. Nothing while the overhead quest icons
    /// setting is off. `renderer`: the glyphs are uploaded on first use.
    pub fn update_indicators(
        &mut self,
        renderer: Option<&mut Renderer<'_>>,
        world: &crate::net::World,
        npc: &NpcHub,
        position: &dyn Fn(EntityId) -> Option<Vec3>,
    ) {
        if !self.icons || self.ui.is_none() || world.player().is_none() {
            self.shown.clear();
            return;
        }
        if std::mem::take(&mut self.rings_stale) {
            let state = world.player_state();
            let view = JournalView::new(state, npc.data().map(AsRef::as_ref));
            self.rings = view.rings(state);
        }
        self.shown = anchored(&self.rings, position);
        if !self.shown.is_empty()
            && !self.overlay_ready
            && let Some(r) = renderer
        {
            self.overlay.prepare(r, INDICATOR_SCALE);
            self.overlay_ready = true;
        }
    }

    /// Draws the indicators after the walker's scene (a screen pass of its own, before the
    /// game UI's), with the walker's view-projection matrix.
    pub fn draw_indicators(&self, frame: &mut Frame, view_proj: Option<Mat4>) {
        let Some(vp) = view_proj else { return };
        if !self.overlay_ready || self.shown.iter().all(|i| i.anchor.is_none()) {
            return;
        }
        let size = frame.size();
        let pass = frame.pass("quest-indicators", PassSpace::Screen);
        self.overlay.draw(pass, vp, size, &self.shown);
    }

    /// Frees the glyph textures.
    pub fn release(&mut self, renderer: &mut Renderer<'_>) {
        self.overlay.release(renderer);
        self.overlay_ready = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_names() {
        assert_eq!(parse_journal_panel("quest-log"), Some(JournalPanel::Log));
        assert_eq!(
            parse_journal_panel(" Tracker "),
            Some(JournalPanel::Tracker)
        );
        assert_eq!(parse_journal_panel("quest"), None);
        assert!(is_log_key(&tessera_input::RawEvent::Key {
            key: LOG_KEY,
            pressed: true,
            repeat: false
        }));
    }

    fn key(key: KeyCode, pressed: bool) -> RawEvent {
        RawEvent::Key {
            key,
            pressed,
            repeat: false,
        }
    }

    #[test]
    fn the_log_key_needs_no_modifier() {
        let mut k = LogKey::default();
        assert!(k.event(&key(KeyCode::L, true)));
        assert!(!k.event(&key(KeyCode::L, false)));
        assert!(!k.event(&RawEvent::Key {
            key: KeyCode::L,
            pressed: true,
            repeat: true
        }));
        // Ctrl+L and Alt+L do not toggle (NW10: no modifier).
        assert!(!k.event(&key(KeyCode::ControlLeft, true)));
        assert!(!k.event(&key(KeyCode::L, true)));
        assert!(!k.event(&key(KeyCode::ControlLeft, false)));
        assert!(!k.event(&key(KeyCode::AltRight, true)));
        assert!(!k.event(&key(KeyCode::L, true)));
        // A lost focus forgets the held modifier.
        k.release_all();
        assert!(k.event(&key(KeyCode::L, true)));
        assert!(!k.event(&key(KeyCode::M, true)));
    }
}
