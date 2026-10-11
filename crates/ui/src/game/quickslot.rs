// SPDX-License-Identifier: GPL-3.0-or-later
//! Quickslot bars, the game type `QuickslotBar` (`game-panels.md` §10), and their skill
//! displays: content, cooldown sweep, slot states and activation ([`SkillSlots`],
//! `combat-ui.md` §9, §13.7).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::tree::ElementKey;
use crate::ui::Ui;

use super::data::{Shortcut, Shortcuts};
use super::vocab::GameVocab;
use super::{GameError, find_below};

/// Slots a bar builds (§10.1 step 2: always 12).
pub const SLOTS: i32 = 12;

/// One quickslot of a bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The `QuickslotButtonTemplate` instance.
    pub button: ElementKey,
    /// Its `UI_Shortcut_Index`.
    pub index: i32,
    /// Whether the slot's shortcut is empty.
    pub empty: bool,
}

/// One `QuickslotBar` (§10).
#[derive(Debug, Clone, PartialEq)]
pub struct QuickslotBar {
    /// The bar element.
    pub key: ElementKey,
    /// The list box named by `UIComponent_Quickslot_Listbox`.
    pub list: Option<ElementKey>,
    /// The element ID `UIComponent_Quickslot_Listbox` names (0 when absent).
    pub list_id: u32,
    /// The `quickslots` layout.
    pub layout: Option<Did>,
    /// The slots of the last build, in slot order.
    pub slots: Vec<Slot>,
    /// Whether a drag is in progress (§10.2).
    pub dragging: bool,
}

impl QuickslotBar {
    /// Finds the list box and builds the slots (§10.1).
    pub fn post_create(
        ui: &mut Ui,
        gv: &GameVocab,
        key: ElementKey,
        layout: Option<Did>,
        shortcuts: &dyn Shortcuts,
    ) -> Self {
        let lid = ui.prop(key, gv.pids.quickslot_listbox).key();
        let list = if lid == 0 {
            None
        } else {
            find_below(ui, key, lid)
        };
        let mut b = Self {
            key,
            list,
            list_id: lid,
            layout,
            slots: Vec::new(),
            dragging: false,
        };
        if let Err(e) = b.rebuild(ui, gv, shortcuts) {
            tracing::debug!("quickslot bar not built: {e}");
        }
        b
    }

    /// (Re)builds the 12 slots (§10.1): empties the list box; per slot creates the button
    /// template, sets `UI_Shortcut_Index` = offset + *i*, the key label, the content (an
    /// empty shortcut gets `QuickslotEmptyTemplate`), inserts it at index *i* and hides it
    /// when the shortcut is empty (§10.2: also during a drag; the next drag begin shows
    /// them again).
    ///
    /// # Errors
    /// [`GameError::MissingProperty`] when the bar names no list box,
    /// [`GameError::MissingElement`] when the named list box is not below the bar,
    /// [`GameError::NoLayout`] without the `quickslots` layout, or a creation error.
    pub fn rebuild(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        shortcuts: &dyn Shortcuts,
    ) -> Result<(), GameError> {
        let list = self.list.ok_or(if self.list_id == 0 {
            GameError::MissingProperty(gv.pids.quickslot_listbox)
        } else {
            GameError::MissingElement(self.list_id)
        })?;
        let layout = self.layout.ok_or(GameError::NoLayout("quickslots"))?;
        for e in ui.list_entries(list) {
            ui.list_remove(list, e);
        }
        self.slots.clear();
        let offset = ui.prop(self.key, gv.pids.quickslot_offset).int();
        for i in 0..SLOTS {
            let index = offset.saturating_add(i);
            let button = ui.create_element(None, layout, gv.ids.quickslot_button)?;
            ui.set_property(button, gv.pids.shortcut_index, PropertyValue::Int(index));
            if let Some(label) = find_below(ui, button, gv.ids.quickslot_label) {
                ui.set_text(label, shortcuts.key_label(index));
            }
            let shortcut = shortcuts.shortcut(index);
            let content = match &shortcut {
                Shortcut::Empty => Some(gv.ids.quickslot_empty),
                Shortcut::Kind(k) => shortcuts.content_template(*k),
            };
            if let Some(c) = content
                && let Err(e) = ui.create_element(Some(button), layout, c)
            {
                tracing::debug!("quickslot content {c:#x} not created: {e}");
            }
            if let Err(e) = ui.list_add(list, button, None) {
                ui.destroy(button);
                return Err(e.into());
            }
            let empty = shortcut == Shortcut::Empty;
            if empty {
                ui.set_shown(button, false);
            }
            self.slots.push(Slot {
                button,
                index,
                empty,
            });
        }
        Ok(())
    }

    /// `Element_drag_global_begin` (§10.2): every empty slot is shown.
    pub fn drag_begin(&mut self, ui: &mut Ui) {
        self.dragging = true;
        for s in self.slots.iter().filter(|s| s.empty) {
            ui.set_shown(s.button, true);
        }
    }

    /// `Element_drag_global_end` (§10.2): empty slots hide again (the per-bar counter of
    /// the §10.2 OPEN is taken as zero).
    pub fn drag_end(&mut self, ui: &mut Ui) {
        self.dragging = false;
        for s in self.slots.iter().filter(|s| s.empty) {
            ui.set_shown(s.button, false);
        }
    }
}

// ----- P2: skill displays, cooldowns, slot states and activation (combat-ui.md §9) -------

use std::collections::BTreeMap;

use crate::events::UiEvent;
use crate::geom::IBox;
use crate::widgets::WidgetClass;

use super::IconImage;
use super::combat::data::{CombatAction, cap_label};
use super::combat::look::{self, TextStyle};
use super::combat::vocab::CombatVocab;

/// Frames of the cooldown meter (§9.3: 60).
pub const COOLDOWN_FRAMES: usize = 60;

/// The default `UI_SkillDisplay_MinResetTimeForAvailabilityAlert` (§9.1: 2.0 s).
pub const MIN_ALERT: f64 = 2.0;

/// How long the error and availability overlays show (§9.4: 1 s animations).
pub const FLASH: f64 = 1.0;

/// Longest cooldown Tessera keeps, seconds (cap on hostile durations: thirty days).
pub const MAX_COOLDOWN: f64 = 2_592_000.0;

/// Highest shortcut index (§9.1 TR32: slots 1 … 1,024).
pub const MAX_SHORTCUT_INDEX: i32 = 1023;

/// Most running cooldowns remembered for skills (Tessera cap on hostile feeds; when full,
/// the cooldown that ends first is forgotten).
pub const MAX_COOLDOWNS: usize = 4096;

/// Slots of the fallback bar (§13.7).
pub const FALLBACK_SLOTS: i32 = 12;

/// A running cooldown of one slot (§9.3): the end time *T* and the total *D*, seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cooldown {
    /// *T*.
    pub end: f64,
    /// *D*.
    pub total: f64,
}

impl Cooldown {
    /// A cooldown of `duration` seconds started at `now`; `None` unless `duration` > 0.
    #[must_use]
    pub fn new(now: f64, duration: f64) -> Option<Self> {
        (duration.is_finite() && duration > 0.0).then(|| Self {
            end: now + duration.min(MAX_COOLDOWN),
            total: duration.min(MAX_COOLDOWN),
        })
    }

    /// The remaining fraction *p* = clamp(*R* ÷ *D*, 0, 1) with *R* = *T* − now.
    #[must_use]
    pub fn position(&self, now: f64) -> f32 {
        if self.total <= 0.0 {
            return 0.0;
        }
        (((self.end - now) / self.total) as f32).clamp(0.0, 1.0)
    }

    /// The frame shown: trunc(59 × *p* + 0.5) (§9.3, CUI5).
    #[must_use]
    pub fn frame(&self, now: f64) -> i32 {
        crate::widgets::meter::frame_index(COOLDOWN_FRAMES, self.position(now))
    }

    /// Whether the goal is reached.
    #[must_use]
    pub fn done(&self, now: f64) -> bool {
        now >= self.end
    }
}

/// What a slot holds (TR32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotKind {
    /// A skill, by the client's key for its ref (equal refs, equal keys).
    Skill(u64),
    /// An item, by instance ID.
    Item(u64),
}

/// A filled slot.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotView {
    /// The content.
    pub kind: SlotKind,
    /// The skill's or item's name (the fallback shows its first two characters).
    pub label: String,
    /// The icon.
    pub icon: Option<IconImage>,
    /// The record the hover tooltip examines (`UI_Examination_Tooltip_DID_Override`).
    pub tooltip: Option<u32>,
}

/// The fallback parts of one slot.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FallbackSlot {
    root: ElementKey,
    fill: ElementKey,
    text: Option<ElementKey>,
    label: Option<ElementKey>,
    sweep: ElementKey,
    playing: ElementKey,
    error: ElementKey,
    available: ElementKey,
}

/// The install parts of one slot (below its `QuickslotButtonTemplate`).
#[derive(Debug, Clone, PartialEq)]
struct InstallSlot {
    button: ElementKey,
    icon: Option<ElementKey>,
    timer: Option<ElementKey>,
    playing: Option<ElementKey>,
    error: Option<ElementKey>,
    available: Option<ElementKey>,
    min_alert: f64,
    /// The §13.7 fallback content of a slot whose content has no icon (a literal; TR32,
    /// TR41: the fallback applies element by element).
    literal: Option<ElementKey>,
}

/// The state of one shortcut index.
#[derive(Debug, Clone, PartialEq, Default)]
struct SlotState {
    view: Option<SlotView>,
    cooldown: Option<Cooldown>,
    frame: Option<i32>,
    playing: bool,
    error_until: Option<f64>,
    available_until: Option<f64>,
}

/// The skill displays of the quickslot bars: content, cooldown sweep, `playing`,
/// `skillerror` and `skillavailable` (§9.1–§9.4, TR32–TR36), activation by click
/// (TR33), in the install look (over the slots [`QuickslotBar`] builds) or the fallback
/// bar of §13.7.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillSlots {
    cv: CombatVocab,
    states: BTreeMap<i32, SlotState>,
    cooldowns: BTreeMap<u64, Cooldown>,
    /// The skill of the player's active cast (TR36), for slots filled during it.
    casting: Option<u64>,
    install: BTreeMap<i32, InstallSlot>,
    fallback: BTreeMap<i32, FallbackSlot>,
    button_id: u32,
    regs: Vec<(u32, u32)>,
}

fn skill_of(v: &Option<SlotView>) -> Option<u64> {
    match v.as_ref().map(|v| &v.kind) {
        Some(SlotKind::Skill(s)) => Some(*s),
        _ => None,
    }
}

impl SkillSlots {
    /// A controller without elements yet.
    #[must_use]
    pub fn new(cv: &CombatVocab) -> Self {
        Self {
            cv: *cv,
            states: BTreeMap::new(),
            cooldowns: BTreeMap::new(),
            casting: None,
            install: BTreeMap::new(),
            fallback: BTreeMap::new(),
            button_id: 0,
            regs: Vec::new(),
        }
    }

    /// Whether the fallback bar is in use.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        !self.fallback.is_empty()
    }

    fn listen(&mut self, ui: &mut Ui, id: u32, msg: u32) {
        if !self.regs.contains(&(id, msg)) {
            ui.register_handler(id, msg);
            self.regs.push((id, msg));
        }
    }

    /// Removes the game-code handler registrations (before dropping the controller).
    pub fn detach(&mut self, ui: &mut Ui) {
        for (id, m) in self.regs.drain(..) {
            ui.unregister_handler(id, m);
        }
    }

    /// Finds the skill displays of the bars' slots (after every bar rebuild) and applies the
    /// kept content, cooldowns and states to them. `button_id` is the element ID of
    /// `QuickslotButtonTemplate` ([`GameVocab`]`::ids.quickslot_button`); its `Button_click`
    /// uses the slot (TR33).
    pub fn attach_bars(&mut self, ui: &mut Ui, bars: &[QuickslotBar], button_id: u32, now: f64) {
        for s in self.install.values() {
            if let Some(k) = s.literal.filter(|k| ui.exists(*k)) {
                ui.destroy(k);
            }
        }
        self.install.clear();
        let ids = self.cv.ids;
        for bar in bars {
            for s in &bar.slots {
                if !ui.exists(s.button) {
                    continue;
                }
                let f = |id: u32| find_below(ui, s.button, id);
                let display = f(ids.skill_slot);
                let min_alert = display.map_or(MIN_ALERT, |d| {
                    let p = ui.prop(d, self.cv.pids.min_reset_alert);
                    if p.is_set() {
                        f64::from(p.float())
                    } else {
                        MIN_ALERT
                    }
                });
                let slot = InstallSlot {
                    button: s.button,
                    icon: f(ids.skill_slot_icon),
                    timer: f(ids.skill_slot_timer),
                    playing: f(ids.skill_slot_playing),
                    error: f(ids.skill_slot_error),
                    available: f(ids.skill_slot_available),
                    min_alert,
                    literal: None,
                };
                self.install.insert(s.index, slot);
            }
        }
        self.button_id = button_id;
        let click = ui.vocab().msgs.button_click;
        self.listen(ui, button_id, click);
        let indices: Vec<i32> = self.install.keys().copied().collect();
        for i in indices {
            if let Some(st) = self.states.get_mut(&i) {
                st.frame = None;
            }
            self.apply_content(ui, i);
            // §11: running cooldowns are reapplied to the new slot elements (TR35).
            self.start_meter(ui, i, now);
            self.refresh(ui, i, now);
        }
    }

    /// Builds the fallback bar of §13.7 under the HUD root (shortcut indices 0–11), with
    /// key labels from `shortcuts` (TR34).
    pub fn fallback_bar(&mut self, ui: &mut Ui, hud: ElementKey, shortcuts: &dyn Shortcuts) {
        let tp = self.cv.text;
        let click = ui.vocab().msgs.mouse_click;
        for i in 0..FALLBACK_SLOTS {
            if self.fallback.contains_key(&i) {
                continue;
            }
            let id = look::next_look_id();
            let Some(root) = look::element(
                ui,
                hud,
                WidgetClass::Field,
                IBox::from_xywh(302 + 35 * i, 728, 35, 35),
                id,
            ) else {
                return;
            };
            let Some(fill) = look::element(
                ui,
                root,
                WidgetClass::Field,
                IBox::from_xywh(0, 0, 35, 35),
                look::LOOK_ELEMENT_ID,
            ) else {
                return;
            };
            let text = look::text(
                ui,
                &tp,
                root,
                IBox::from_xywh(0, 0, 35, 35),
                "",
                TextStyle::new(12, 0xFFFF_FFFF).centred(),
            );
            let label = look::text(
                ui,
                &tp,
                root,
                IBox::from_xywh(2, 23, 31, 12),
                &shortcuts.key_label(i).unwrap_or_default(),
                TextStyle::new(10, 0xFFFF_FFFF).outlined(0xFF00_0000),
            );
            let Some(sweep) = look::element(
                ui,
                root,
                WidgetClass::Field,
                IBox::from_xywh(0, 0, 35, 35),
                look::LOOK_ELEMENT_ID,
            ) else {
                return;
            };
            ui.set_shown(sweep, false);
            let frame = |ui: &mut Ui, c: u32| {
                let f = look::element(
                    ui,
                    root,
                    WidgetClass::Field,
                    IBox::from_xywh(0, 0, 35, 35),
                    look::LOOK_ELEMENT_ID,
                )?;
                look::border_lines(ui, f, 35, 35, 2, c);
                ui.set_shown(f, false);
                Some(f)
            };
            let (Some(playing), Some(available), Some(error)) = (
                frame(ui, 0xFFFF_E060),
                frame(ui, 0xFFFF_FFFF),
                frame(ui, 0xFFFF_4040),
            ) else {
                return;
            };
            look::border_lines(ui, root, 35, 35, 1, 0xFF80_8080);
            self.listen(ui, id, click);
            self.fallback.insert(
                i,
                FallbackSlot {
                    root,
                    fill,
                    text,
                    label,
                    sweep,
                    playing,
                    error,
                    available,
                },
            );
            self.apply_content(ui, i);
        }
    }

    /// The content of a shortcut index.
    #[must_use]
    pub fn slot(&self, index: i32) -> Option<&SlotView> {
        self.states.get(&index).and_then(|s| s.view.as_ref())
    }

    /// The running cooldown of a shortcut index.
    #[must_use]
    pub fn cooldown(&self, index: i32) -> Option<Cooldown> {
        self.states.get(&index).and_then(|s| s.cooldown)
    }

    /// How many skill cooldowns are remembered (at most [`MAX_COOLDOWNS`]).
    #[must_use]
    pub fn remembered_cooldowns(&self) -> usize {
        self.cooldowns.len()
    }

    /// The cooldown frame a slot shows (§9.3), while its cooldown runs.
    #[must_use]
    pub fn frame(&self, index: i32) -> Option<i32> {
        self.states.get(&index).and_then(|s| s.frame)
    }

    /// Whether a slot shows `playing`, `skillerror`, `skillavailable` at `now`.
    #[must_use]
    pub fn flags(&self, index: i32, now: f64) -> (bool, bool, bool) {
        self.states.get(&index).map_or((false, false, false), |s| {
            (
                s.playing,
                s.error_until.is_some_and(|t| now < t),
                s.available_until.is_some_and(|t| now < t),
            )
        })
    }

    /// The shortcut index whose slot holds `k` (its button or a part), for clicks and the
    /// examination hover (`skill-tooltips.md` §12.1).
    #[must_use]
    pub fn slot_at(&self, ui: &Ui, k: ElementKey) -> Option<i32> {
        self.install
            .iter()
            .find(|(_, s)| ui.is_ancestor_or_self(s.button, k))
            .map(|(i, _)| *i)
            .or_else(|| {
                self.fallback
                    .iter()
                    .find(|(_, s)| ui.is_ancestor_or_self(s.root, k))
                    .map(|(i, _)| *i)
            })
    }

    /// `QuickslotChanged { slot, content }` (TR32): the content of shortcut index `index`
    /// (the feed's slot − 1). A skill whose cooldown runs starts it with the remaining
    /// time (TR35); a cast of it in progress shows `playing` (TR36).
    pub fn set_slot(&mut self, ui: &mut Ui, index: i32, view: Option<SlotView>, now: f64) {
        if !(0..=MAX_SHORTCUT_INDEX).contains(&index) {
            return;
        }
        let view = view.map(|mut v| {
            v.label = cap_label(&v.label);
            v
        });
        let skill = skill_of(&view);
        let cd = skill
            .and_then(|s| self.cooldowns.get(&s).copied())
            .filter(|c| !c.done(now));
        let st = self.states.entry(index).or_default();
        let same = skill.is_some() && skill_of(&st.view) == skill;
        st.view = view;
        if !same {
            st.cooldown = cd;
            st.frame = None;
            st.playing = skill.is_some() && skill == self.casting;
        }
        self.apply_content(ui, index);
        if !same && cd.is_some() {
            self.start_meter(ui, index, now);
        }
        self.refresh(ui, index, now);
    }

    /// `CooldownStarted { what, duration_ms }` (TR35), seconds: every slot showing the skill
    /// starts (*T* = now + *D*; a slot already counting keeps its *D*).
    pub fn cooldown_started(&mut self, ui: &mut Ui, skill: u64, duration: f64, now: f64) {
        let Some(c) = Cooldown::new(now, duration) else {
            return;
        };
        if !self.cooldowns.contains_key(&skill) && self.cooldowns.len() >= MAX_COOLDOWNS {
            self.cooldowns.retain(|_, c| !c.done(now));
            if self.cooldowns.len() >= MAX_COOLDOWNS
                && let Some(first) = self
                    .cooldowns
                    .iter()
                    .min_by(|a, b| a.1.end.total_cmp(&b.1.end))
                    .map(|(k, _)| *k)
            {
                self.cooldowns.remove(&first);
            }
        }
        self.cooldowns.insert(skill, c);
        let hits: Vec<i32> = self
            .states
            .iter()
            .filter(|(_, s)| skill_of(&s.view) == Some(skill))
            .map(|(i, _)| *i)
            .collect();
        for i in hits {
            if let Some(st) = self.states.get_mut(&i) {
                st.cooldown = Some(match st.cooldown {
                    Some(old) => Cooldown {
                        end: c.end,
                        total: old.total,
                    },
                    None => c,
                });
                st.frame = None;
            }
            self.start_meter(ui, i, now);
            self.refresh(ui, i, now);
        }
    }

    /// `CooldownEnded { what }` (TR35): the goal is reached at once.
    pub fn cooldown_ended(&mut self, ui: &mut Ui, skill: u64, now: f64) {
        self.cooldowns.remove(&skill);
        let hits: Vec<i32> = self
            .states
            .iter()
            .filter(|(_, s)| skill_of(&s.view) == Some(skill) && s.cooldown.is_some())
            .map(|(i, _)| *i)
            .collect();
        for i in hits {
            self.finish(ui, i, now);
        }
    }

    /// The player's cast (TR36): slots showing `skill` show `playing`; `None` clears it.
    pub fn set_casting(&mut self, ui: &mut Ui, skill: Option<u64>, now: f64) {
        self.casting = skill;
        let idx: Vec<i32> = self.states.keys().copied().collect();
        for i in idx {
            if let Some(st) = self.states.get_mut(&i) {
                st.playing = skill.is_some() && skill_of(&st.view) == skill;
            }
            self.refresh(ui, i, now);
        }
    }

    /// A refused use of a slot plays `skillerror` (TR36).
    pub fn error(&mut self, ui: &mut Ui, index: i32, now: f64) {
        if let Some(st) = self.states.get_mut(&index) {
            st.error_until = Some(now + FLASH);
        }
        self.refresh(ui, index, now);
    }

    /// Per-frame work: cooldown frames, their end steps (§9.3), the 1 s overlays.
    pub fn update(&mut self, ui: &mut Ui, now: f64) {
        self.cooldowns.retain(|_, c| !c.done(now));
        let idx: Vec<i32> = self.states.keys().copied().collect();
        for i in idx {
            let done = self
                .states
                .get(&i)
                .and_then(|s| s.cooldown)
                .is_some_and(|c| c.done(now));
            if done {
                self.finish(ui, i, now);
            } else {
                self.refresh(ui, i, now);
            }
        }
    }

    /// The end steps of §9.3: the availability alert when *D* ≥ the minimum, then the
    /// meter hides and *T*, *D* reset.
    fn finish(&mut self, ui: &mut Ui, i: i32, now: f64) {
        let min = self.install.get(&i).map_or(MIN_ALERT, |s| s.min_alert);
        if let Some(st) = self.states.get_mut(&i) {
            if let Some(c) = st.cooldown.take()
                && c.total >= min
            {
                st.available_until = Some(now + FLASH);
            }
            st.frame = None;
        }
        self.refresh(ui, i, now);
    }

    /// §9.3 start on the install meter: smooth movement from *R* ÷ *D* to 0 in *R*.
    fn start_meter(&self, ui: &mut Ui, i: i32, now: f64) {
        let (Some(s), Some(c)) = (self.install.get(&i), self.cooldown(i)) else {
            return;
        };
        let Some(m) = s.timer else { return };
        let p = ui.vocab().pids;
        let r = (c.end - now).max(0.0) as f32;
        ui.set_shown(m, true);
        ui.set_property(m, p.meter_smooth, PropertyValue::Bool(true));
        ui.set_property(m, p.meter_smooth_duration, PropertyValue::Float(r));
        ui.set_property(m, p.meter_position, PropertyValue::Float(c.position(now)));
        ui.set_property(m, p.meter_goal, PropertyValue::Float(0.0));
    }

    fn apply_content(&mut self, ui: &mut Ui, i: i32) {
        let view = self.states.get(&i).and_then(|s| s.view.clone());
        let pids = self.cv.pids;
        if let Some(s) = self.install.get(&i).cloned() {
            if let Some(icon) = s.icon {
                super::set_icon(ui, icon, view.as_ref().and_then(|v| v.icon.as_ref()));
            }
            self.install_literal(ui, i, &s, view.as_ref());
            match view.as_ref().and_then(|v| v.tooltip) {
                Some(t) => {
                    ui.set_property(
                        s.button,
                        pids.tooltip_did_override,
                        PropertyValue::DidRef(Did(t)),
                    );
                    ui.set_examine(s.button, true);
                }
                None => {
                    ui.clear_property(s.button, pids.tooltip_did_override);
                    ui.set_examine(s.button, view.is_some());
                }
            }
        }
        if let Some(s) = self.fallback.get(&i).cloned() {
            look::set_fill(ui, s.fill, view.as_ref().map(|_| 0xFF40_4060));
            if let Some(t) = s.text {
                let two: Option<String> = view.as_ref().map(|v| v.label.chars().take(2).collect());
                ui.set_text(t, two);
            }
            ui.set_examine(s.root, view.is_some());
        }
    }

    /// The §13.7 skill content (fill 0xFF404060 with the label's first two characters) over
    /// an install slot whose content has no icon (TR32: a literal ref takes the fallback
    /// icon; TR41); removed when the content changes.
    fn install_literal(&mut self, ui: &mut Ui, i: i32, s: &InstallSlot, view: Option<&SlotView>) {
        if let Some(k) = s.literal.filter(|k| ui.exists(*k)) {
            ui.destroy(k);
        }
        let mut made = None;
        if let Some(v) = view.filter(|v| v.icon.is_none()) {
            let parent = s.icon.unwrap_or(s.button);
            let (w, h) = ui
                .layout_box(parent)
                .map_or((32, 32), |b| (b.w().max(1), b.h().max(1)));
            if let Some(b) = look::boxed(
                ui,
                parent,
                IBox::from_xywh(0, 0, w, h),
                Some(0xFF40_4060),
                None,
            ) {
                let two: String = v.label.chars().take(2).collect();
                let tp = self.cv.text;
                look::text(
                    ui,
                    &tp,
                    b,
                    IBox::from_xywh(0, 0, w, h),
                    &two,
                    TextStyle::new(12, 0xFFFF_FFFF).centred(),
                );
                made = Some(b);
            }
        }
        if let Some(st) = self.install.get_mut(&i) {
            st.literal = made;
        }
    }

    /// Shows the slot's cooldown frame and overlays at `now`.
    fn refresh(&mut self, ui: &mut Ui, i: i32, now: f64) {
        let (frame, playing, error, available) = {
            let Some(st) = self.states.get_mut(&i) else {
                return;
            };
            if st.error_until.is_some_and(|t| now >= t) {
                st.error_until = None;
            }
            if st.available_until.is_some_and(|t| now >= t) {
                st.available_until = None;
            }
            let f = st.cooldown.map(|c| c.frame(now));
            // CUI5: the frame never increases during one cooldown.
            let f = match (f, st.frame) {
                (Some(n), Some(o)) => Some(n.min(o)),
                (n, _) => n,
            };
            st.frame = f;
            (
                f,
                st.playing,
                st.error_until.is_some(),
                st.available_until.is_some(),
            )
        };
        let states = self.cv.states;
        if let Some(s) = self.install.get(&i).cloned() {
            if frame.is_none()
                && let Some(m) = s.timer
            {
                ui.set_shown(m, false);
            }
            let overlay = |ui: &mut Ui, k: Option<ElementKey>, on: bool, st: u32| {
                if let Some(k) = k {
                    ui.set_shown(k, on);
                    if on {
                        super::switch_if_defined(ui, k, st);
                    }
                }
            };
            overlay(ui, s.playing, playing, states.playing);
            overlay(ui, s.error, error, states.skill_error);
            overlay(ui, s.available, available, states.skill_available);
        }
        if let Some(s) = self.fallback.get(&i).cloned() {
            match frame {
                Some(f) => {
                    let r = look::sweep_ref(0xA000_0000, 35, f);
                    look::set_procedural(ui, s.sweep, &r);
                    ui.set_shown(s.sweep, true);
                }
                None => ui.set_shown(s.sweep, false),
            }
            ui.set_shown(s.playing, playing);
            ui.set_shown(s.error, error);
            ui.set_shown(s.available, available);
        }
    }

    /// A click on a slot's button (TR33) gives [`CombatAction::UseShortcut`] when the slot
    /// is filled; an empty slot gives nothing.
    #[must_use]
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent) -> Option<CombatAction> {
        let UiEvent::Game { message } = ev else {
            return None;
        };
        if !self
            .regs
            .iter()
            .any(|(id, m)| *id == message.source_id && *m == message.msg)
        {
            return None;
        }
        let i = self.slot_at(ui, message.source)?;
        self.slot(i)?;
        Some(CombatAction::UseShortcut(i))
    }
}
