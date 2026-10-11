// SPDX-License-Identifier: GPL-3.0-or-later
//! The fellowship display (`social-panels.md` §4, Tessera rules §9 TR2–TR4): the member
//! list of the last `FellowshipChanged`, the rows of panel 1 (every member but the player,
//! at most five), the leader marker on the rows and on `PlayerLeaderIcon`, the member data
//! (level, morale, power, defeat, effects, assist target), the chat lines derived from
//! consecutive member lists, and the targeting and assist actions.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::chat::notice::{self as n, Notice, TextRef, args as A, name_equal, tables as T};
use super::super::subtree;
use super::vocab::SocialVocab;
use super::{
    ContextMenu, MenuEntry, SocialAction, SocialIntent, ensure_rows, find, game_message, listen_id,
    mouse_button, row_of,
};

/// Rows panel 1 shows at most (TR2: six places, the player and five rows).
pub const MAX_ROWS: usize = 5;

/// Most members a `FellowshipChanged` is read with (the feed's bound, `scenario-feed.md`
/// §6.4).
pub const MAX_MEMBERS: usize = 64;

/// Most effects kept per member (Tessera cap; the row shows 2 × 6).
pub const MAX_EFFECTS: usize = 64;

/// Most entities (and member names) whose data is kept (Tessera cap). When full, the
/// oldest entry that does not belong to a current member is evicted for a new one; a
/// current member's entry is never evicted.
pub const MAX_TRACKED: usize = 4096;

/// One member of `FellowshipChanged`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The name.
    pub name: String,
    /// The member's entity, if it has one.
    pub entity: Option<u64>,
    /// Whether the member leads.
    pub leader: bool,
}

/// A vital (morale or power) as current and maximum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vital {
    /// Current value.
    pub current: f32,
    /// Maximum.
    pub max: f32,
}

impl Vital {
    /// The fill in [0, 1] (0 for a non-positive or non-finite maximum).
    #[must_use]
    pub fn fill(&self) -> f32 {
        if self.max.is_finite() && self.max > 0.0 && self.current.is_finite() {
            (self.current / self.max).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Which vital.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VitalKind {
    /// Morale (health).
    Morale,
    /// Power.
    Power,
}

/// The data of one entity, from the feed's character events (TR3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemberData {
    /// `LevelChanged`.
    pub level: Option<i64>,
    /// Morale, with the sequence number of its last change.
    pub morale: Option<(Vital, u64)>,
    /// Power, with the sequence number of its last change.
    pub power: Option<(Vital, u64)>,
    /// `Defeated` (true) / `Revived` (false).
    pub defeated: bool,
    /// Applied effects.
    pub effects: BTreeSet<u64>,
    /// The last `TargetChanged` of the entity.
    pub target: Option<u64>,
    /// The entity has despawned (and not spawned again): a click on its member posts the
    /// out-of-range line (TR3).
    pub despawned: bool,
}

/// What one row shows (§4.2 with TR3).
#[derive(Debug, Clone, PartialEq)]
pub struct MemberRow {
    /// The name.
    pub name: String,
    /// The entity, if any.
    pub entity: Option<u64>,
    /// The level, if known.
    pub level: Option<i64>,
    /// Morale, if known.
    pub morale: Option<Vital>,
    /// Power, if known.
    pub power: Option<Vital>,
    /// Defeated.
    pub defeated: bool,
    /// Effects. Modelled but not drawn yet: `FellowEffectDisplay` stays as the layout has
    /// it (`social-panels.md` §4.2 OPEN (implementation), effect icons).
    pub effects: Vec<u64>,
    /// The leader marker is shown.
    pub leader: bool,
}

/// A vital with the sequence number of its last change.
type Stamped = Option<(Vital, u64)>;

/// Morale and power reported by name (`FellowVitalChanged`).
type NamedVitals = (Stamped, Stamped);

/// The fellowship model (TR2–TR4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fellowship {
    player: String,
    player_entity: Option<u64>,
    members: Vec<Member>,
    sent_leave: bool,
    sent_kicks: Vec<String>,
    data: BTreeMap<u64, MemberData>,
    /// The keys of `data`, oldest first (eviction order).
    data_order: VecDeque<u64>,
    by_name: BTreeMap<String, NamedVitals>,
    /// The keys of `by_name`, oldest first.
    by_name_order: VecDeque<String>,
    seq: u64,
    warned_missing: bool,
    warned_overflow: bool,
}

impl Fellowship {
    /// A model for the player `player` with entity `entity`.
    #[must_use]
    pub fn new(player: &str, entity: Option<u64>) -> Self {
        Self {
            player: player.to_string(),
            player_entity: entity,
            ..Self::default()
        }
    }

    /// The member list (the player appended when the event's list lacked it, TR2).
    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// Whether a fellowship exists (panel 1 shown, TR2).
    #[must_use]
    pub fn exists(&self) -> bool {
        !self.members.is_empty()
    }

    /// The names of the members.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.members.iter().map(|m| m.name.clone()).collect()
    }

    fn player_index(members: &[Member], name: &str, entity: Option<u64>) -> Option<usize> {
        entity
            .and_then(|e| members.iter().position(|m| m.entity == Some(e)))
            .or_else(|| members.iter().position(|m| m.name == name))
    }

    fn is_player(&self, m: &Member) -> bool {
        match Self::player_index(&self.members, &self.player, self.player_entity) {
            Some(i) => std::ptr::eq(&self.members[i], m),
            None => false,
        }
    }

    /// The leader: the first member with `leader` set (TR2).
    #[must_use]
    pub fn leader(&self) -> Option<&Member> {
        self.members.iter().find(|m| m.leader)
    }

    /// Whether the player leads (`PlayerLeaderIcon` shown, SPI6).
    #[must_use]
    pub fn player_leads(&self) -> bool {
        self.leader().is_some_and(|l| self.is_player(l))
    }

    /// The members with rows: every member but the player, in list order, at most
    /// [`MAX_ROWS`] (TR2, SPI6).
    #[must_use]
    pub fn row_members(&self) -> Vec<&Member> {
        let p = Self::player_index(&self.members, &self.player, self.player_entity);
        self.members
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != p)
            .map(|(_, m)| m)
            .take(MAX_ROWS)
            .collect()
    }

    /// Records a command the client sent (TR4 reads `LeaveFellowship` and
    /// `KickFromFellowship` since the previous event).
    pub fn note_sent(&mut self, intent: &SocialIntent) {
        match intent {
            SocialIntent::LeaveFellowship => self.sent_leave = true,
            SocialIntent::KickFromFellowship { name } if self.sent_kicks.len() < MAX_MEMBERS => {
                self.sent_kicks.push(name.clone());
            }
            _ => {}
        }
    }

    /// `FellowshipChanged { members }` (TR2, TR4): replaces the list and returns the
    /// derived lines, mask 0x400 (rendered from the install by the client; on a backend
    /// that sends no `FellowshipChanged` nothing is derived).
    pub fn changed(&mut self, members: &[Member]) -> Vec<Notice> {
        let mut new: Vec<Member> = members.iter().take(MAX_MEMBERS).cloned().collect();
        if members.len() > MAX_ROWS + 1 && !self.warned_overflow {
            self.warned_overflow = true;
            tracing::warn!("fellowship with more than six members: further rows not shown");
        }
        if !new.is_empty() && Self::player_index(&new, &self.player, self.player_entity).is_none() {
            if !self.warned_missing {
                self.warned_missing = true;
                tracing::warn!("fellowship list without the player: the player is appended");
            }
            new.push(Member {
                name: self.player.clone(),
                entity: self.player_entity,
                leader: false,
            });
        }
        let old = std::mem::replace(&mut self.members, new);
        let lines = self.derive(&old);
        self.sent_leave = false;
        self.sent_kicks.clear();
        lines
    }

    fn derive(&self, old: &[Member]) -> Vec<Notice> {
        let line = |r: TextRef| Notice::of(r, n::FELLOWSHIP);
        let new = &self.members;
        if !old.is_empty() && new.is_empty() {
            let t = if self.sent_leave {
                0x0307_3954
            } else {
                0x018C_CD74
            };
            return vec![line(TextRef::new(T::FELLOWSHIP, t))];
        }
        if old.is_empty() && !new.is_empty() {
            let t = if self.player_leads() {
                0x0245_83E7
            } else {
                0x0CDF_FC77
            };
            return vec![line(TextRef::new(T::FELLOWSHIP, t))];
        }
        let mut out = Vec::new();
        if old.is_empty() {
            return out;
        }
        let has = |list: &[Member], name: &str| list.iter().any(|m| m.name == name);
        for m in old.iter().filter(|m| !has(new, &m.name)) {
            let kicked = self.sent_kicks.iter().any(|k| name_equal(k, &m.name));
            let t = if kicked { 0x0928_4BE7 } else { 0x0C75_F607 };
            out.push(line(
                TextRef::new(T::FELLOWSHIP, t).text(A::NAME, m.name.clone()),
            ));
        }
        for m in new.iter().filter(|m| !has(old, &m.name)) {
            out.push(line(
                TextRef::new(T::FELLOWSHIP, 0x047E_6917).text(A::NAME, m.name.clone()),
            ));
        }
        let old_leader = old.iter().find(|m| m.leader).map(|m| m.name.as_str());
        let new_leader = self.leader().map(|m| m.name.as_str());
        if old_leader != new_leader
            && let Some(l) = self.leader()
        {
            if self.player_leads() {
                out.push(line(TextRef::new(T::FELLOWSHIP, 0x043F_25A6)));
            } else {
                out.push(line(
                    TextRef::new(T::FELLOWSHIP, 0x03FD_5682).text(A::NAME, l.name.clone()),
                ));
            }
        }
        out
    }

    /// Evicts the oldest entry of `data` that is not a current member's entity. Returns
    /// whether one was evicted.
    fn evict_entity(&mut self) -> bool {
        let protected: BTreeSet<u64> = self.members.iter().filter_map(|m| m.entity).collect();
        for _ in 0..self.data_order.len() {
            let Some(k) = self.data_order.pop_front() else {
                break;
            };
            if !self.data.contains_key(&k) {
                continue;
            }
            if protected.contains(&k) {
                self.data_order.push_back(k);
                continue;
            }
            self.data.remove(&k);
            return true;
        }
        false
    }

    fn entity_data(&mut self, entity: u64) -> Option<&mut MemberData> {
        if !self.data.contains_key(&entity) {
            if self.data.len() >= MAX_TRACKED && !self.evict_entity() {
                return None;
            }
            self.data_order.push_back(entity);
        }
        Some(self.data.entry(entity).or_default())
    }

    /// Evicts the oldest entry of `by_name` that is not a current member's name.
    fn evict_name(&mut self) -> bool {
        let protected: BTreeSet<String> =
            self.members.iter().map(|m| m.name.to_lowercase()).collect();
        for _ in 0..self.by_name_order.len() {
            let Some(k) = self.by_name_order.pop_front() else {
                break;
            };
            if !self.by_name.contains_key(&k) {
                continue;
            }
            if protected.contains(&k) {
                self.by_name_order.push_back(k);
                continue;
            }
            self.by_name.remove(&k);
            return true;
        }
        false
    }

    fn next_seq(&mut self) -> u64 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// `VitalChanged` of an entity (TR3).
    pub fn vital(&mut self, entity: u64, kind: VitalKind, current: f32, max: f32) {
        let s = self.next_seq();
        if let Some(d) = self.entity_data(entity) {
            let v = Some((Vital { current, max }, s));
            match kind {
                VitalKind::Morale => d.morale = v,
                VitalKind::Power => d.power = v,
            }
        }
    }

    /// `FellowVitalChanged { member, … }` (`scenario-feed.md` "P4 social": a member's vital
    /// as the group reports it, with or without an entity).
    pub fn fellow_vital(&mut self, member: &str, kind: VitalKind, current: f32, max: f32) {
        let s = self.next_seq();
        let key = member.to_lowercase();
        if !self.by_name.contains_key(&key) {
            if self.by_name.len() >= MAX_TRACKED && !self.evict_name() {
                return;
            }
            self.by_name_order.push_back(key.clone());
        }
        let e = self.by_name.entry(key).or_default();
        let v = Some((Vital { current, max }, s));
        match kind {
            VitalKind::Morale => e.0 = v,
            VitalKind::Power => e.1 = v,
        }
    }

    /// `LevelChanged` of an entity.
    pub fn level(&mut self, entity: u64, level: i64) {
        if let Some(d) = self.entity_data(entity) {
            d.level = Some(level);
        }
    }

    /// `Defeated` (true) or `Revived` (false) of an entity.
    pub fn defeated(&mut self, entity: u64, on: bool) {
        if let Some(d) = self.entity_data(entity) {
            d.defeated = on;
        }
    }

    /// `EffectApplied` (true) or `EffectRemoved` (false) of an entity.
    pub fn effect(&mut self, entity: u64, effect: u64, on: bool) {
        if let Some(d) = self.entity_data(entity) {
            if on {
                if d.effects.len() < MAX_EFFECTS {
                    d.effects.insert(effect);
                }
            } else {
                d.effects.remove(&effect);
            }
        }
    }

    /// `TargetChanged` of an entity (for assist, §8.1).
    pub fn target_changed(&mut self, entity: u64, target: Option<u64>) {
        if let Some(d) = self.entity_data(entity) {
            d.target = target;
        }
    }

    /// An entity spawned (true) or despawned (false): a click on a despawned member posts
    /// the out-of-range line (TR3). Data is kept after a despawn (SF §5.4).
    pub fn presence(&mut self, entity: u64, present: bool) {
        if present {
            if let Some(d) = self.data.get_mut(&entity) {
                d.despawned = false;
            }
        } else if let Some(d) = self.entity_data(entity) {
            d.despawned = true;
        }
    }

    fn row_of(&self, m: &Member) -> MemberRow {
        let d = m.entity.and_then(|e| self.data.get(&e));
        let by_name = self.by_name.get(&m.name.to_lowercase());
        let latest = |a: Stamped, b: Stamped| match (a, b) {
            (Some(x), Some(y)) => Some(if x.1 >= y.1 { x.0 } else { y.0 }),
            (Some(x), None) | (None, Some(x)) => Some(x.0),
            (None, None) => None,
        };
        MemberRow {
            name: m.name.clone(),
            entity: m.entity,
            level: d.and_then(|d| d.level),
            morale: latest(d.and_then(|d| d.morale), by_name.and_then(|b| b.0)),
            power: latest(d.and_then(|d| d.power), by_name.and_then(|b| b.1)),
            defeated: d.is_some_and(|d| d.defeated),
            effects: d
                .map(|d| d.effects.iter().copied().collect())
                .unwrap_or_default(),
            leader: self.leader().is_some_and(|l| std::ptr::eq(l, m)),
        }
    }

    /// The rows of panel 1 (TR2, TR3).
    #[must_use]
    pub fn rows(&self) -> Vec<MemberRow> {
        self.row_members()
            .into_iter()
            .map(|m| self.row_of(m))
            .collect()
    }

    fn target_member(&self, m: &Member) -> SocialAction {
        match m.entity {
            Some(e) if !self.data.get(&e).is_some_and(|d| d.despawned) => {
                SocialAction::Intent(SocialIntent::SelectTarget { target: Some(e) })
            }
            // TR3: a type 1 line (Tessera's choice), nothing sent.
            _ => SocialAction::Post(Notice::of(
                TextRef::new(T::FELLOWSHIP, 0x0571_E4C5).text(A::MEMBER, m.name.clone()),
                n::ERROR,
            )),
        }
    }

    /// A left click on row `i` (§4.4, §8.1, TR3).
    #[must_use]
    pub fn click_row(&self, i: usize) -> Option<SocialAction> {
        self.row_members().get(i).map(|m| self.target_member(m))
    }

    /// `FellowAssist` on row `i`, or `AssistFellowN` with `i` = N − 2 (§8.1): the member's
    /// last reported target, if any.
    #[must_use]
    pub fn assist_row(&self, i: usize) -> Option<SocialAction> {
        let m = self.row_members().get(i).copied()?;
        let t = m
            .entity
            .and_then(|e| self.data.get(&e))
            .and_then(|d| d.target)?;
        Some(SocialAction::Intent(SocialIntent::SelectTarget {
            target: Some(t),
        }))
    }

    /// `SelectFellowOne` … `SelectFellowSix` (§7.2 Tessera rule, §8.1): N = 1 the player
    /// (nothing while the player's entity is unknown, rather than clearing the target),
    /// N ≥ 2 the (N − 1)-th row.
    #[must_use]
    pub fn select_fellow(&self, n: usize) -> Option<SocialAction> {
        match n {
            0 => None,
            1 => self
                .player_entity
                .map(|e| SocialAction::Intent(SocialIntent::SelectTarget { target: Some(e) })),
            _ => self.click_row(n - 2),
        }
    }

    /// `AssistFellowTwo` … `AssistFellowSix` (§8.1).
    #[must_use]
    pub fn assist_fellow(&self, n: usize) -> Option<SocialAction> {
        if n < 2 {
            return None;
        }
        self.assist_row(n - 2)
    }

    /// The player menu's group submenu for row `i`'s member (§4.5 for a fellowship): leave
    /// is the player's own entry and so not on a member row; dismiss when the player leads
    /// and the member does not; transfer leadership when the player leads.
    #[must_use]
    pub fn row_menu(&self, i: usize) -> Option<ContextMenu> {
        let m = self.row_members().get(i).copied()?;
        let leads = self.player_leads();
        let mut entries = Vec::new();
        if leads && !m.leader {
            entries.push(MenuEntry {
                label: TextRef::new(T::PLAYER_ACTION, 0x0599_3B0C),
                enabled: true,
                intent: Some(SocialIntent::KickFromFellowship {
                    name: m.name.clone(),
                }),
            });
        }
        if leads {
            entries.push(MenuEntry {
                label: TextRef::new(T::PLAYER_ACTION, 0x0E3B_2F5C),
                enabled: true,
                intent: Some(SocialIntent::SetFellowshipLeader {
                    name: m.name.clone(),
                }),
            });
        }
        Some(ContextMenu {
            heading: Some(TextRef::new(T::UI_GUILD, 0x08AC_5E85)),
            entries,
        })
    }

    /// The player's own group submenu (§4.5): leave while in a group.
    #[must_use]
    pub fn self_menu(&self) -> Option<ContextMenu> {
        self.exists().then(|| ContextMenu {
            heading: Some(TextRef::new(T::UI_GUILD, 0x08AC_5E85)),
            entries: vec![MenuEntry {
                label: TextRef::new(T::PLAYER_ACTION, 0x0A21_737C),
                enabled: true,
                intent: Some(SocialIntent::LeaveFellowship),
            }],
        })
    }
}

/// The four fellowship panels and `PlayerLeaderIcon` on the element tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FellowshipPanels {
    vocab: SocialVocab,
    /// `FellowMiniPanel_1` … `_4` (the ones opened).
    pub panels: Vec<ElementKey>,
    /// Panel 1's list box.
    pub list: Option<ElementKey>,
    /// The rows of panel 1, in list order.
    pub rows: Vec<ElementKey>,
    /// `PlayerLeaderIcon` of the player-vitals panel, when given.
    pub player_leader: Option<ElementKey>,
}

impl FellowshipPanels {
    /// Opens the four panels of layout `minifellow` (§4.1; TR1: hidden while there is no
    /// fellowship, panels 2–4 always hidden) and registers the row handlers. A panel that
    /// cannot be opened is skipped.
    pub fn open(ui: &mut Ui, vocab: &SocialVocab) -> Self {
        vocab.register_classes(ui);
        let ids = vocab.ids;
        let mut panels = Vec::new();
        for id in [
            ids.fellow_panel_1,
            ids.fellow_panel_2,
            ids.fellow_panel_3,
            ids.fellow_panel_4,
        ] {
            match ui.open_panel_by_key(vocab.layouts.minifellow, id) {
                Ok(k) => {
                    ui.set_shown(k, false);
                    panels.push(k);
                }
                Err(e) => tracing::debug!("fellowship panel not opened: {e}"),
            }
        }
        Self::attach(ui, vocab, panels)
    }

    /// Binds panels the client opened itself (panel 1 first; the client called
    /// [`SocialVocab::register_classes`] before building them).
    pub fn attach(ui: &mut Ui, vocab: &SocialVocab, panels: Vec<ElementKey>) -> Self {
        let ids = vocab.ids;
        let list = panels.first().and_then(|p| find(ui, *p, ids.fellow_list));
        let ms = ui.vocab().msgs;
        listen_id(ui, ids.fellow_template, ms.mouse_click);
        listen_id(ui, ids.fellow_assist, ms.button_click);
        Self {
            vocab: vocab.clone(),
            panels,
            list,
            rows: Vec::new(),
            player_leader: None,
        }
    }

    /// Shows the model (§4.6 rebuild, TR2, TR3): panel 1 shown while a fellowship exists,
    /// panels 2–4 hidden, one row per member but the player, each filled, the leader
    /// markers, and `PlayerLeaderIcon`.
    pub fn refresh(&mut self, ui: &mut Ui, model: &Fellowship) {
        let ids = self.vocab.ids;
        let st = self.vocab.states;
        for (i, p) in self.panels.iter().enumerate() {
            ui.set_shown(*p, i == 0 && model.exists());
        }
        if let Some(pl) = self.player_leader {
            ui.switch_state_if_defined(pl, st.leader_fellowship);
            ui.set_shown(pl, model.player_leads());
        }
        let Some(list) = self.list else { return };
        let rows = model.rows();
        self.rows = ensure_rows(
            ui,
            list,
            self.vocab.layouts.minifellow,
            ids.fellow_template,
            rows.len(),
        );
        for (k, r) in self.rows.clone().into_iter().zip(&rows) {
            if let Some(e) = find(ui, k, ids.fellow_name) {
                ui.set_text(e, Some(r.name.clone()));
            }
            if let Some(e) = find(ui, k, ids.fellow_level) {
                // TR3: an unknown level is empty, never the layout's own text.
                ui.set_text(e, Some(r.level.map(|l| l.to_string()).unwrap_or_default()));
            }
            if let Some(e) = find(ui, k, ids.fellow_leader) {
                ui.switch_state_if_defined(e, st.leader_fellowship);
                ui.set_shown(e, r.leader);
            }
            if let Some(e) = find(ui, k, ids.fellow_link_dead) {
                ui.set_shown(e, false);
            }
            if let Some(e) = find(ui, k, ids.fellow_defeated) {
                ui.set_shown(e, r.defeated);
            }
            for (id, v) in [(ids.vital_health, r.morale), (ids.vital_power, r.power)] {
                if let Some(e) = find(ui, k, id) {
                    set_meters(ui, e, v.map_or(0.0, |v| v.fill()));
                }
            }
        }
    }

    /// Sets `PlayerLeaderIcon` (of the player-vitals panel P4) and shows it by the model.
    pub fn set_player_leader(&mut self, ui: &mut Ui, k: Option<ElementKey>, model: &Fellowship) {
        self.player_leader = k;
        if let Some(pl) = k {
            ui.switch_state_if_defined(pl, self.vocab.states.leader_fellowship);
            ui.set_shown(pl, model.player_leads());
        }
    }

    /// Handles a UI event (§4.4, §4.5): a left click on a row targets the member (or posts
    /// the out-of-range line), a right click asks for the group menu, `FellowAssist`
    /// assists.
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent, model: &Fellowship) -> Vec<SocialAction> {
        let Some(m) = game_message(ev) else {
            return Vec::new();
        };
        let ms = ui.vocab().msgs;
        let Some(i) = row_of(ui, &self.rows, m.source) else {
            return Vec::new();
        };
        if m.msg == ms.button_click && m.source_id == self.vocab.ids.fellow_assist {
            return model.assist_row(i).into_iter().collect();
        }
        if m.msg == ms.mouse_click {
            return match mouse_button(m) {
                1 => model.click_row(i).into_iter().collect(),
                2 => model
                    .row_menu(i)
                    .map(SocialAction::Menu)
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            };
        }
        Vec::new()
    }
}

/// Sets every Meter at or below `k` to `fill` (the fallback of `MultiVitalDisplay`, GP §11:
/// its meters show the member's vital).
fn set_meters(ui: &mut Ui, k: ElementKey, fill: f32) {
    let pos = ui.vocab().pids.meter_position;
    for e in subtree(ui, k) {
        if ui.el(e).is_some_and(|x| x.class == WidgetClass::Meter) {
            ui.set_property(e, pos, PropertyValue::Float(fill));
        }
    }
}
