// SPDX-License-Identifier: GPL-3.0-or-later
//! The travel and instance-finder windows on the game UI (`travel-windows.md` §4–§7): one
//! instance of each window, opened from the install's layout by label ([`P5Refs`]) or as
//! Tessera's fallback widgets, attached to `tessera-ui`'s P5 controllers and filled from
//! the session state ([`super::present`]).
//!
//! Each frame the client calls [`TravelUi::frame`] with the P5 state, the install's facts
//! when present, the frame's UI events and the client clock; it returns the session
//! commands to send:
//!
//! 1. **sync**: the travel window follows the state's travel window (`TravelShown` opens or
//!    refills it, `WindowClosed` destroys it; the client never closes it on its own, §4.10
//!    rule 6); the instance window is shown as the instance-travel window when §4.11's
//!    Tessera rule holds, else as the finder page (§6.8); the queue state drives the
//!    confirm page and the alert (§6.8 rule 6, §7.2); `InstanceChanged` ghosts the join
//!    button (§6.6 rule 1);
//! 2. **presentation**: a `TravelStarted` starts the loading screen (swift) or the
//!    end-travel box (slow) for its duration; a teleport or `TravelEnded` ends it (§5.2);
//! 3. **events**: the windows' controls give `ChooseTravel`, `JoinInstance`, `LeaveQueue`,
//!    `Dismount` and `CloseWindow` (§4.10 rule 6, §6.8 rules 5–6, §5.2 rule 4).
//!
//! `--open-panel` names (`travel`, `instance-finder`, `instance-travel`, `end-travel`,
//! Tessera's own) open the windows empty ([`TravelUi::open_panel`]).

use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_net::Command;
use tessera_net::domain::{NpcCommand, WindowId, WorldCommand};
use tessera_ui::game::context_menu::MenuRefs;
use tessera_ui::game::finder::{
    FinderAction, FinderView, FinderWindow, InstanceTravelWindow, QueueAlert,
};
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::npc::keep_on_screen;
use tessera_ui::game::travel::{
    EndTravelBox, TravelAction, TravelPresentation, TravelShow, TravelView, TravelWindow,
};
use tessera_ui::game::travelfallback;
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::present::{
    InstallTravelTexts, finder_view, instance_travel, journey_swift, travel_view,
};
use super::refs::{P5Refs, P5Window};
use super::state::P5State;
use crate::npc::NpcData;

/// Most distinct failure reasons kept (Tessera bound).
pub const MAX_FAILURES: usize = 64;

/// What the client knows about the player and the entities the windows name.
pub struct TravelContext<'a> {
    /// The local player's level (the failed-requirements mark, the finder's level field).
    pub player_level: u32,
    /// The name of an entity (the travel window's title, §4.10 rule 1).
    pub entity_name: &'a dyn Fn(tessera_net::EntityId) -> Option<String>,
    /// Whether an entity's body holds `PrivateEncounter_TravelArray` (§4.11; `None`:
    /// unknown).
    pub holds_travel_array: &'a dyn Fn(tessera_net::EntityId) -> Option<bool>,
    /// The client clock in seconds (the travel presentation and the queue timers).
    pub now: f64,
}

impl std::fmt::Debug for TravelContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TravelContext")
            .field("player_level", &self.player_level)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

/// The travel and instance windows on the game UI (module docs).
pub struct TravelUi {
    refs: Arc<P5Refs>,
    art: P5Art,
    registered: bool,
    travel: Option<(TravelWindow, u64)>,
    finder: Option<(FinderWindow, u64)>,
    pe: Option<(InstanceTravelWindow, u64)>,
    end_box: Option<EndTravelBox>,
    loading: Option<(ElementKey, Option<ElementKey>)>,
    alert: Option<QueueAlert>,
    presentation: Option<(TravelPresentation, u64)>,
    queue_seen: u64,
    /// The displayed queue: (state key, scripted seconds, client time it arrived); replayed
    /// on a finder created later (§6.8 rule 6, §6.5).
    last_queue: Option<(u8, Option<u32>, f64)>,
    in_instance: bool,
    /// Windows opened empty by name.
    panels: Vec<(P5Window, ElementKey)>,
    failures: Vec<String>,
    /// Windows that could be built neither from the layout nor as a fallback: not tried
    /// again in this run (no layout decode and no warning per frame).
    gave_up: Vec<P5Window>,
    /// The finder's size and tier menus over the install's `ContextMenu` layout
    /// (`travel-windows.md` §6.8, `ui-widgets.md` §19), resolved once the install's facts
    /// were seen; until then (and without an install) the finder keeps its code-made
    /// menus.
    finder_menus: Option<MenuRefs>,
}

impl std::fmt::Debug for TravelUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TravelUi")
            .field("travel", &self.travel.as_ref().map(|t| t.0.window))
            .field("finder", &self.finder.as_ref().map(|t| t.0.window))
            .field("failures", &self.failures.len())
            .finish_non_exhaustive()
    }
}

impl TravelUi {
    /// No window open yet.
    #[must_use]
    pub fn new(refs: Arc<P5Refs>, art: P5Art) -> Self {
        Self {
            refs,
            art,
            registered: false,
            travel: None,
            finder: None,
            pe: None,
            end_box: None,
            loading: None,
            alert: None,
            presentation: None,
            queue_seen: 0,
            last_queue: None,
            in_instance: false,
            panels: Vec::new(),
            failures: Vec::new(),
            gave_up: Vec::new(),
            finder_menus: None,
        }
    }

    /// Resolves the finder's context-menu names from the install's presentation data once
    /// (as the map window's menus, [`crate::map::ui::p5_menu_refs`]) and gives them to a
    /// finder already open.
    fn ensure_menu_refs(&mut self, data: Option<&NpcData>) {
        if self.finder_menus.is_some() {
            return;
        }
        let Some(d) = data else { return };
        let refs = crate::map::ui::p5_menu_refs(Some(d.character()), &self.art);
        if refs.layout.is_none() {
            tracing::warn!("instance finder: ContextMenu layout missing; code-made menus");
        }
        if let Some((f, _)) = self.finder.as_mut() {
            f.set_menu_refs(refs.clone());
        }
        self.finder_menus = Some(refs);
    }

    /// The finder's context-menu names in use: the install's once resolved (tests).
    #[must_use]
    pub fn finder_menu_refs(&self) -> Option<&MenuRefs> {
        self.finder_menus.as_ref()
    }

    /// The names in use.
    #[must_use]
    pub fn refs(&self) -> &Arc<P5Refs> {
        &self.refs
    }

    /// Distinct failure reasons so far (`LayoutMissing`, `ElementMissing` and creation
    /// errors; the census of `travel-windows.md` §8.2 counts them).
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// The travel window, when open.
    #[must_use]
    pub fn travel(&self) -> Option<&TravelWindow> {
        self.travel.as_ref().map(|t| &t.0)
    }

    /// The travel window, mutably (selection, search; tests and tools).
    pub fn travel_mut(&mut self) -> Option<&mut TravelWindow> {
        self.travel.as_mut().map(|t| &mut t.0)
    }

    /// The finder, when open.
    #[must_use]
    pub fn finder(&self) -> Option<&FinderWindow> {
        self.finder.as_ref().map(|t| &t.0)
    }

    /// The finder, mutably (selection, menus; tests and tools).
    pub fn finder_mut(&mut self) -> Option<&mut FinderWindow> {
        self.finder.as_mut().map(|t| &mut t.0)
    }

    /// The instance-travel window, when open.
    #[must_use]
    pub fn instance_travel(&self) -> Option<&InstanceTravelWindow> {
        self.pe.as_ref().map(|t| &t.0)
    }

    /// The end-travel box, once created.
    #[must_use]
    pub fn end_box(&self) -> Option<&EndTravelBox> {
        self.end_box.as_ref()
    }

    /// The loading screen's root, once created.
    #[must_use]
    pub fn loading_screen(&self) -> Option<ElementKey> {
        self.loading.map(|l| l.0)
    }

    /// The queue alert, once created.
    #[must_use]
    pub fn alert(&self) -> Option<&QueueAlert> {
        self.alert.as_ref()
    }

    /// The running travel presentation.
    #[must_use]
    pub fn presentation(&self) -> Option<&TravelPresentation> {
        self.presentation.as_ref().map(|p| &p.0)
    }

    /// The windows opened empty by name.
    #[must_use]
    pub fn panels(&self) -> &[(P5Window, ElementKey)] {
        &self.panels
    }

    /// Records a failure; logged once per distinct reason.
    fn fail(&mut self, why: String) {
        if self.failures.contains(&why) {
            return;
        }
        tracing::warn!("{why}");
        if self.failures.len() < MAX_FAILURES {
            self.failures.push(why);
        }
    }

    fn register(&mut self, ui: &mut Ui) {
        if !self.registered {
            self.refs.nv.register_classes(ui);
            self.refs.pv.register_classes(ui);
            self.registered = true;
        }
    }

    /// Opens window `w` from the install's layout, else as the fallback widget; `None`
    /// (with a failure) when neither can be built, after which `w` is not tried again.
    fn open_native(
        &mut self,
        ui: &mut Ui,
        w: P5Window,
        texts: &InstallTravelTexts<'_>,
    ) -> Option<ElementKey> {
        if self.gave_up.contains(&w) {
            return None;
        }
        let k = self.try_open(ui, w, texts);
        if k.is_none() {
            self.gave_up.push(w);
        }
        k
    }

    fn try_open(
        &mut self,
        ui: &mut Ui,
        w: P5Window,
        texts: &InstallTravelTexts<'_>,
    ) -> Option<ElementKey> {
        self.register(ui);
        if let Some((layout, root)) = self.refs.window(w) {
            match ui.open_panel(layout, root) {
                Ok(k) => {
                    keep_on_screen(ui, k);
                    return Some(k);
                }
                Err(e) => self.fail(format!(
                    "{}: LayoutMissing ({e}); fallback window",
                    w.panel_name()
                )),
            }
        } else if self.refs.has_install() {
            self.fail(format!(
                "{}: LayoutMissing; fallback window",
                w.panel_name()
            ));
        }
        let (nv, pv, art) = (&self.refs.nv, &self.refs.pv, &self.art);
        let r = match w {
            P5Window::Travel => travelfallback::travel(ui, nv, pv, art, texts),
            P5Window::Finder => travelfallback::finder(ui, nv, pv, art, texts),
            P5Window::InstanceTravel => travelfallback::instance_travel(ui, nv, pv, art, texts),
            P5Window::EndTravel => travelfallback::end_travel(ui, nv, pv, art, texts),
            P5Window::QueueAlert => travelfallback::queue_alert(ui, pv, art),
            // No fallback loading screen: without an install the presentation is state
            // only (`travel-windows.md` §5.2 needs the install's image and meter).
            P5Window::LoadScreen => return None,
        };
        match r {
            Ok(k) => {
                keep_on_screen(ui, k);
                Some(k)
            }
            Err(e) => {
                self.fail(format!("{}: {e}", w.panel_name()));
                None
            }
        }
    }

    fn ensure_travel(&mut self, ui: &mut Ui, texts: &InstallTravelTexts<'_>) -> bool {
        if self.travel.as_ref().is_some_and(|t| ui.exists(t.0.root)) {
            return true;
        }
        let Some(k) = self.open_native(ui, P5Window::Travel, texts) else {
            return false;
        };
        let (nv, pv) = (self.refs.nv.clone(), self.refs.pv.clone());
        let w = TravelWindow::attach(ui, &nv, &pv, k, self.refs.travel, &self.art);
        self.travel = Some((w, 0));
        true
    }

    fn ensure_finder(&mut self, ui: &mut Ui, texts: &InstallTravelTexts<'_>) -> bool {
        if self.finder.as_ref().is_some_and(|t| ui.exists(t.0.root)) {
            return true;
        }
        let Some(k) = self.open_native(ui, P5Window::Finder, texts) else {
            return false;
        };
        let (nv, pv) = (self.refs.nv.clone(), self.refs.pv.clone());
        let mut w = FinderWindow::attach(ui, &nv, &pv, k, self.refs.finder, &self.art, texts);
        if let Some(m) = &self.finder_menus {
            w.set_menu_refs(m.clone());
        }
        w.set_in_instance(ui, self.in_instance, texts);
        // The displayed queue state, as an existing finder shows it (§6.8 rule 6).
        if let Some((state, seconds, since)) = self.last_queue {
            w.set_queue(ui, state, seconds, since, texts);
        }
        self.finder = Some((w, 0));
        true
    }

    fn ensure_pe(&mut self, ui: &mut Ui, texts: &InstallTravelTexts<'_>) -> bool {
        if self.pe.as_ref().is_some_and(|t| ui.exists(t.0.root)) {
            return true;
        }
        let Some(k) = self.open_native(ui, P5Window::InstanceTravel, texts) else {
            return false;
        };
        let pv = self.refs.pv.clone();
        self.pe = Some((InstanceTravelWindow::attach(ui, &pv, k, texts), 0));
        true
    }

    fn ensure_end_box(&mut self, ui: &mut Ui, texts: &InstallTravelTexts<'_>) {
        if self.end_box.as_ref().is_some_and(|b| ui.exists(b.root)) {
            return;
        }
        if let Some(k) = self.open_native(ui, P5Window::EndTravel, texts) {
            let pv = self.refs.pv.clone();
            self.end_box = Some(EndTravelBox::attach(ui, &pv, k, texts));
        }
    }

    fn ensure_alert(&mut self, ui: &mut Ui, texts: &InstallTravelTexts<'_>) {
        if self.alert.as_ref().is_some_and(|a| ui.exists(a.root)) {
            return;
        }
        if let Some(k) = self.open_native(ui, P5Window::QueueAlert, texts) {
            let pv = self.refs.pv.clone();
            self.alert = Some(QueueAlert::attach(ui, &pv, k));
        }
    }

    fn ensure_loading(&mut self, ui: &mut Ui, texts: &InstallTravelTexts<'_>) {
        if self.loading.is_some_and(|l| ui.exists(l.0)) {
            return;
        }
        if let Some(k) = self.open_native(ui, P5Window::LoadScreen, texts) {
            let meter = tessera_ui::game::find_below(ui, k, self.refs.pv.ids.load_meter);
            ui.set_shown(k, false);
            self.loading = Some((k, meter));
        }
    }

    /// Opens window `w` empty (`--open-panel`, Tessera rule): the travel window and the
    /// finder with no row, the instance-travel window without a template, the end-travel
    /// box shown.
    ///
    /// # Errors
    /// When neither the layout nor the fallback window can be built.
    pub fn open_panel(
        &mut self,
        ui: &mut Ui,
        w: P5Window,
        data: Option<&NpcData>,
    ) -> Result<ElementKey, String> {
        self.ensure_menu_refs(data);
        let texts = InstallTravelTexts::new(data);
        let root = match w {
            P5Window::Travel => {
                if !self.ensure_travel(ui, &texts) {
                    return Err("travel: no window".into());
                }
                let t = self.travel.as_mut().map(|t| &mut t.0);
                if let Some(t) = t {
                    t.show(ui, &TravelView::default(), &texts);
                    t.root
                } else {
                    return Err("travel: no window".into());
                }
            }
            P5Window::Finder => {
                if !self.ensure_finder(ui, &texts) {
                    return Err("instance-finder: no window".into());
                }
                match self.finder.as_mut() {
                    Some((f, _)) => {
                        f.show(ui, &FinderView::default(), &texts);
                        f.root
                    }
                    None => return Err("instance-finder: no window".into()),
                }
            }
            P5Window::InstanceTravel => {
                if !self.ensure_pe(ui, &texts) {
                    return Err("instance-travel: no window".into());
                }
                match self.pe.as_ref() {
                    Some((p, _)) => p.root,
                    None => return Err("instance-travel: no window".into()),
                }
            }
            P5Window::EndTravel => {
                self.ensure_end_box(ui, &texts);
                match self.end_box.as_ref() {
                    Some(b) => {
                        b.set_shown(ui, true);
                        b.root
                    }
                    None => return Err("end-travel: no window".into()),
                }
            }
            P5Window::LoadScreen => {
                self.ensure_loading(ui, &texts);
                match self.loading {
                    Some((k, _)) => {
                        ui.set_shown(k, true);
                        k
                    }
                    None => return Err("loading-screen: no install".into()),
                }
            }
            P5Window::QueueAlert => {
                self.ensure_alert(ui, &texts);
                match self.alert.as_ref() {
                    Some(a) => {
                        a.set_state(ui, 2, &texts);
                        a.root
                    }
                    None => return Err("queue-alert: no window".into()),
                }
            }
        };
        if !self.panels.iter().any(|(p, _)| *p == w) {
            self.panels.push((w, root));
        }
        Ok(root)
    }

    /// `ToggleWorldJoin` (`travel-windows.md` TW14, TW15): shows the finder when hidden
    /// (opening it empty when no feed window is open), hides it when shown.
    pub fn toggle_finder(&mut self, ui: &mut Ui, data: Option<&NpcData>) {
        self.ensure_menu_refs(data);
        if let Some((f, _)) = self.finder.as_ref()
            && ui.exists(f.root)
        {
            let on = !ui.is_visible(f.root);
            ui.set_shown(f.root, on);
            if on {
                ui.request_activation(f.root);
            }
            return;
        }
        if let Err(e) = self.open_panel(ui, P5Window::Finder, data) {
            self.fail(e);
        }
    }

    /// One frame (module docs): sync with `state`, the travel presentation, then `events`.
    /// Returns the session commands to send, in order.
    pub fn frame(
        &mut self,
        ui: &mut Ui,
        state: &P5State,
        data: Option<&NpcData>,
        ctx: &TravelContext<'_>,
        events: &[UiEvent],
    ) -> Vec<Command> {
        self.ensure_menu_refs(data);
        let texts = InstallTravelTexts::new(data);
        self.sync(ui, state, data, ctx, &texts);
        self.present(ui, state, data, ctx.now, &texts);
        let mut out = Vec::new();
        for ev in events {
            self.handle_event(ui, ev, &texts, &mut out);
        }
        out
    }

    fn sync(
        &mut self,
        ui: &mut Ui,
        state: &P5State,
        data: Option<&NpcData>,
        ctx: &TravelContext<'_>,
        texts: &InstallTravelTexts<'_>,
    ) {
        // The travel window (§4.10).
        match state.travel() {
            Some(feed) => {
                let seen = self.travel.as_ref().map_or(0, |t| t.1);
                if (seen != feed.shown || self.travel.is_none()) && self.ensure_travel(ui, texts) {
                    let name = feed.source.and_then(|s| (ctx.entity_name)(s));
                    let view = travel_view(feed, data, ctx.player_level, name.as_deref());
                    if let Some((w, s)) = self.travel.as_mut() {
                        w.show(ui, &view, texts);
                        ui.set_shown(w.root, true);
                        ui.request_activation(w.root);
                        *s = feed.shown;
                    }
                }
            }
            None => {
                if let Some((mut w, s)) = self.travel.take() {
                    if s == 0 && self.panels.iter().any(|(p, _)| *p == P5Window::Travel) {
                        // Opened empty by name: kept until the user closes it.
                        self.travel = Some((w, s));
                    } else {
                        w.detach(ui);
                        ui.destroy(w.root);
                    }
                }
            }
        }
        // In an instance (§6.6 rule 1).
        let inside = state.instance().is_some();
        if inside != self.in_instance {
            self.in_instance = inside;
            if let Some((f, _)) = self.finder.as_mut() {
                f.set_in_instance(ui, inside, texts);
            }
        }
        // The instance window: instance-travel (§4.11) or the finder (§6.8).
        match state.instances() {
            Some(feed) => {
                let holds = feed.source.and_then(|s| (ctx.holds_travel_array)(s));
                if let Some((name, intro)) = instance_travel(feed, data, holds) {
                    let seen = self.pe.as_ref().map_or(0, |p| p.1);
                    if seen != feed.shown && self.ensure_pe(ui, texts) {
                        if let Some((p, s)) = self.pe.as_mut() {
                            p.show(ui, feed.window.0, &name, &intro);
                            ui.set_shown(p.root, true);
                            ui.request_activation(p.root);
                            *s = feed.shown;
                        }
                        // Showing it hides the invitation window (§4.11); Tessera shows no
                        // invitation window in P5, and the finder of the same window goes.
                        self.drop_finder(ui);
                    }
                } else {
                    let seen = self.finder.as_ref().map_or(0, |f| f.1);
                    if seen != feed.shown && self.ensure_finder(ui, texts) {
                        let level = u16::try_from(ctx.player_level).unwrap_or(u16::MAX);
                        let view = finder_view(feed, data, level);
                        if let Some((f, s)) = self.finder.as_mut() {
                            f.show(ui, &view, texts);
                            ui.set_shown(f.root, true);
                            ui.request_activation(f.root);
                            *s = feed.shown;
                        }
                        self.drop_pe(ui);
                    }
                }
            }
            None => {
                if self.finder.as_ref().is_some_and(|f| f.1 != 0) {
                    self.drop_finder(ui);
                }
                if self.pe.as_ref().is_some_and(|p| p.1 != 0) {
                    self.drop_pe(ui);
                }
            }
        }
        // The queue (§6.8 rule 6, §7.2).
        if let Some(q) = state.queue()
            && q.changed != self.queue_seen
        {
            self.queue_seen = q.changed;
            self.last_queue = Some((q.state, q.seconds, ctx.now));
            if let Some((f, _)) = self.finder.as_mut() {
                f.set_queue(ui, q.state, q.seconds, ctx.now, texts);
            }
            if q.state != 0 || self.alert.is_some() {
                self.ensure_alert(ui, texts);
            }
            if let Some(a) = self.alert.as_ref() {
                a.set_state(ui, q.state, texts);
            }
        } else if state.queue().is_none() && self.last_queue.take().is_some() {
            // The session ended (a reset): no queue is shown any more.
            self.queue_seen = 0;
            if let Some((f, _)) = self.finder.as_mut() {
                f.set_queue(ui, 0, None, ctx.now, texts);
            }
            if let Some(a) = self.alert.as_ref() {
                a.set_state(ui, 0, texts);
            }
        }
        if let Some((f, _)) = self.finder.as_mut() {
            f.tick(ui, ctx.now, texts);
        }
    }

    fn drop_finder(&mut self, ui: &mut Ui) {
        if let Some((mut f, _)) = self.finder.take() {
            f.detach(ui);
            ui.destroy(f.root);
        }
    }

    fn drop_pe(&mut self, ui: &mut Ui) {
        if let Some((mut p, _)) = self.pe.take() {
            p.detach(ui);
            ui.destroy(p.root);
        }
    }

    fn present(
        &mut self,
        ui: &mut Ui,
        state: &P5State,
        data: Option<&NpcData>,
        now: f64,
        texts: &InstallTravelTexts<'_>,
    ) {
        if state.journey().is_none() {
            // The session ended (a reset): no travel is presented any more.
            self.presentation = None;
        }
        if let Some(j) = state.journey() {
            let seen = self.presentation.as_ref().map(|p| p.1);
            if seen != Some(j.started) {
                // Rule 7: a new travel replaces a running one.
                let p = TravelPresentation::start(now, journey_swift(j, data), j.duration_ms);
                self.presentation = Some((p, j.started));
            }
            if j.ended
                && let Some((p, _)) = self.presentation.as_mut()
            {
                p.end();
            }
        }
        let show = self
            .presentation
            .as_ref()
            .map_or(TravelShow::Nothing, |p| p.0.shown(now));
        if show == TravelShow::EndTravelBox {
            self.ensure_end_box(ui, texts);
        }
        if show == TravelShow::LoadingScreen {
            self.ensure_loading(ui, texts);
        }
        let opened_box = self.panels.iter().any(|(p, _)| *p == P5Window::EndTravel);
        if let Some(b) = self.end_box.as_ref() {
            b.set_shown(ui, show == TravelShow::EndTravelBox || opened_box);
        }
        if let Some((k, meter)) = self.loading {
            let on = show == TravelShow::LoadingScreen;
            ui.set_shown(k, on);
            if on && let Some(m) = meter {
                let v = self.presentation.as_ref().map_or(0.0, |p| p.0.meter(now));
                let pid = ui.vocab().pids.meter_position;
                ui.set_property(m, pid, PropertyValue::Float(v));
            }
        }
        // Once over, the presentation is kept with its travel's sequence number, so the
        // same `TravelStarted` never starts it again (§5.2 rules 5 and 7).
    }

    fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &InstallTravelTexts<'_>,
        out: &mut Vec<Command>,
    ) {
        let mut travel_actions = Vec::new();
        if let Some((w, _)) = self.travel.as_mut()
            && let Some(a) = w.handle_event(ui, ev, texts)
        {
            travel_actions.push(a);
        }
        if let Some(b) = self.end_box.as_ref()
            && let Some(a) = b.handle_event(ui, ev)
        {
            travel_actions.push(a);
        }
        for a in travel_actions {
            match a {
                TravelAction::Choose { window, entry } => {
                    out.push(Command::World(WorldCommand::ChooseTravel {
                        window: WindowId(window),
                        entry,
                    }));
                }
                TravelAction::Close { window } => self.close(ui, P5Window::Travel, window, out),
                TravelAction::Dismount => out.push(Command::World(WorldCommand::Dismount)),
            }
        }
        let mut finder_actions = Vec::new();
        if let Some((f, _)) = self.finder.as_mut()
            && let Some(a) = f.handle_event(ui, ev, texts)
        {
            finder_actions.push((P5Window::Finder, a));
        }
        if let Some((p, _)) = self.pe.as_ref()
            && let Some(a) = p.handle_event(ui, ev)
        {
            finder_actions.push((P5Window::InstanceTravel, a));
        }
        if let Some(a) = self.alert.as_ref()
            && let Some(x) = a.handle_event(ui, ev)
        {
            finder_actions.push((P5Window::QueueAlert, x));
        }
        for (from, a) in finder_actions {
            match a {
                FinderAction::Join {
                    window,
                    entry,
                    size,
                    tier,
                    level,
                } => {
                    out.push(Command::World(WorldCommand::JoinInstance {
                        window: WindowId(window),
                        entry,
                        size,
                        tier,
                        level: level.filter(|l| *l >= 1),
                    }));
                    // Teleport-now closes the instance-travel window (§4.11).
                    if from == P5Window::InstanceTravel {
                        self.close(ui, from, window, out);
                    }
                }
                FinderAction::LeaveQueue => out.push(Command::World(WorldCommand::LeaveQueue)),
                FinderAction::Close { window } => self.close(ui, from, window, out),
                FinderAction::OpenFinder => {
                    if self.finder.is_none() {
                        let _ = self.ensure_finder(ui, texts);
                        if let Some((f, _)) = self.finder.as_mut() {
                            f.show(ui, &FinderView::default(), texts);
                        }
                    }
                    if let Some((f, _)) = self.finder.as_ref() {
                        ui.set_shown(f.root, true);
                        ui.request_activation(f.root);
                    }
                }
            }
        }
    }

    /// A close control of window `w` (feed window `window`): a feed window sends
    /// `CloseWindow` and stays until the feed answers (§4.10 rule 6); a window opened
    /// empty by name (window 0) closes at once.
    fn close(&mut self, ui: &mut Ui, w: P5Window, window: u32, out: &mut Vec<Command>) {
        if window != 0 {
            out.push(Command::Npc(NpcCommand::CloseWindow {
                window: WindowId(window),
            }));
            return;
        }
        self.panels.retain(|(p, _)| *p != w);
        match w {
            P5Window::Travel => {
                if let Some((mut t, _)) = self.travel.take() {
                    t.detach(ui);
                    ui.destroy(t.root);
                }
            }
            P5Window::Finder => self.drop_finder(ui),
            P5Window::InstanceTravel => self.drop_pe(ui),
            _ => {}
        }
    }

    /// Destroys every window (the UI is going away or the session ended).
    pub fn close_all(&mut self, ui: &mut Ui) {
        if let Some((mut t, _)) = self.travel.take() {
            t.detach(ui);
            ui.destroy(t.root);
        }
        self.drop_finder(ui);
        self.drop_pe(ui);
        if let Some(mut b) = self.end_box.take() {
            b.detach(ui);
            ui.destroy(b.root);
        }
        if let Some(mut a) = self.alert.take() {
            a.detach(ui);
            ui.destroy(a.root);
        }
        if let Some((k, _)) = self.loading.take() {
            ui.destroy(k);
        }
        self.panels.clear();
        self.presentation = None;
        self.last_queue = None;
        self.queue_seen = 0;
    }
}
