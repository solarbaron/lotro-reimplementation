// SPDX-License-Identifier: GPL-3.0-or-later
//! The character panel and the six bag windows in the game UI (`character-panels.md`
//! §2–§6): `Journal_Window` with its equipment page (header, paperdoll with its turning,
//! 23 equipment slots, stat rows), the bag windows `PlayerBag1_Template` …
//! `PlayerBag6_Template` with the backpack slots of §4.3, item tooltips on hover with
//! comparison and pinning (`item-tooltips.md` §8–§10), and the item-slot interactions of §6
//! turned into session commands.
//!
//! The panels change nothing on their own: a drop or an activation becomes a command
//! ([`CharacterUi::take_commands`]), and the slots change when the session's events have
//! changed the [`PlayerState`] ([`CharacterUi::refresh`]).
//!
//! # Tessera rules applied here
//!
//! - `ToggleJournalPanel` toggles `Journal_Window` (§2 OPEN); `QUICKSLOT_BACKPACKn` toggles
//!   bag *n*, `QUICKSLOT_BACKPACK` opens every offered bag when one is closed, else closes
//!   them all (§4.1 OPEN); a bag that is not offered (§4.3 rule 4) does not open.
//! - Only the player variant of the paperdoll is shown; the store button is hidden (§3.1,
//!   §3.2).
//! - Header: `PlayerName` shows the entered character's name, `PlayerLevel` its level and
//!   `PlayerRaceClassDisplay` the race and class display texts joined by a space (the
//!   original's format is not given; `character-panels.md` §3.2 OPEN added by this
//!   implementer); an unknown value shows an empty text. The `PropertyDisplay` elements are
//!   created as text elements (the UI's fallback widget for that game type draws no text;
//!   the UI host registers the type as text at start for the HUD frames P4 and P5,
//!   `combat-ui.md` §4.1, and the panel registers it again when it first opens).
//! - Stat rows (§3.4 rule): a vital row shows "current / max" of the reported vital; a
//!   rating row the player's last reported value of the property its
//!   `UIComponent_CombatPointsDisplay_StatProperty` (or `…_CombatPercentDisplay_…`) names,
//!   as an integer (percent rows: one decimal and a percent sign); the text goes on the row
//!   element itself (the value child is not named; §3.4 OPEN added by this implementer).
//! - Destroying (§6 last row): an item dragged out of every window is destroyed when it is
//!   dragged out again within [`DESTROY_CONFIRM_WINDOW`] (the confirmation; owner check
//!   O84).

use std::time::{Duration, Instant};

use tessera_formats::Did;
use tessera_gamedata::inventory::{
    Container, DropTarget, InventoryCommand, ItemAction, ItemId, ItemPlace, ItemRef,
};
use tessera_gamedata::tooltip::Purpose;
use tessera_gamedata::tooltip::rules::{WEAR_BROKEN, WEAR_DAMAGED, condition_wear_state};
use tessera_net::domain::{InventoryCommand as NetCommand, ItemId as NetItemId};
use tessera_net::{Command, EntityId, PropertyValue};
use tessera_ui::game::avatar::{AvatarInfo, Viewports};
use tessera_ui::game::examine::{ExamTemplate, ExamTooltip, ExamTooltips, PinOutcome};
use tessera_ui::game::items::{
    BagView, DropPlace, ItemSlots, SlotInteraction, SlotItem, SlotPlace, Wear, column_setting,
    columns_for_width, set_columns,
};
use tessera_ui::game::{CharVocab, find_below, subtree};
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::data::CharacterData;
use super::state::{PlayerChange, PlayerState, VitalKind};
use super::tooltip;

/// How long a destroy request waits for its confirmation (Tessera rule, module docs).
pub const DESTROY_CONFIRM_WINDOW: Duration = Duration::from_secs(10);

/// Number of bag windows (`character-panels.md` §4.1).
pub const BAG_WINDOWS: usize = 6;

/// `UILAYOUT` labels and element labels of the P1 panels (interoperability identifiers of
/// the install, `character-panels.md` §2, §4.1, §5).
pub mod labels {
    /// Layout of the character panel.
    pub const JOURNAL_LAYOUT: &str = "journal";
    /// Root of the character panel.
    pub const JOURNAL_WINDOW: &str = "Journal_Window";
    /// Layout of the bag windows.
    pub const CONTAINER_LAYOUT: &str = "containerdisplay";
    /// Layout of the item slot templates.
    pub const ITEM_DISPLAY_LAYOUT: &str = "itemdisplay";
    /// The item slot template.
    pub const SLOTTED_TEMPLATE: &str = "ItemDisplay_SlottedTemplate";
    /// Layout of the examination tooltips and the pinned window.
    pub const EXAMINATION_LAYOUT: &str = "examination";
    /// The bag windows' roots, bag 1 first.
    pub const BAGS: [&str; 6] = [
        "PlayerBag1_Template",
        "PlayerBag2_Template",
        "PlayerBag3_Template",
        "PlayerBag4_Template",
        "PlayerBag5_Template",
        "PlayerBag6_Template",
    ];
    /// Header elements (§3.2).
    pub const PLAYER_NAME: &str = "PlayerName";
    /// Header: level.
    pub const PLAYER_LEVEL: &str = "PlayerLevel";
    /// Header: race and class.
    pub const PLAYER_RACE_CLASS: &str = "PlayerRaceClassDisplay";
    /// The paperdoll variants other than the player's (§3.1), hidden.
    pub const OTHER_VARIANTS: [&str; 2] = [
        "RealPaperdollField_MonsterPlayer",
        "RealPaperdollField_SessionPlayer",
    ];
    /// The store button (§3.2), hidden.
    pub const ENHANCE_BUTTON: &str = "CharacterEquipmentPage_Player_EnhanceCharacter_Button";
    /// Input commands (`keymap.md` §12.14.2 rows of P1).
    pub const TOGGLE_JOURNAL: &str = "ToggleJournalPanel";
    /// All bags.
    pub const TOGGLE_BAGS: &str = "QUICKSLOT_BACKPACK";
    /// Bag *n* is this prefix and *n*.
    pub const TOGGLE_BAG_PREFIX: &str = "QUICKSLOT_BACKPACK";
    /// Pins the showing item tooltip (§10).
    pub const PIN_TOOLTIP: &str = "TOOLTIP_DETACH";
}

/// The layouts and element IDs of the panels, looked up by label at run time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelIds {
    /// `journal` layout.
    pub journal_layout: Option<Did>,
    /// `Journal_Window`.
    pub journal_window: Option<u32>,
    /// `containerdisplay` layout.
    pub container_layout: Option<Did>,
    /// `PlayerBag1_Template` … `6`.
    pub bags: [Option<u32>; BAG_WINDOWS],
    /// The slot template (`itemdisplay` layout, `ItemDisplay_SlottedTemplate`).
    pub slot_template: Option<ExamTemplate>,
    /// `examination` layout.
    pub examination_layout: Option<Did>,
    /// `PlayerName`, `PlayerLevel`, `PlayerRaceClassDisplay`.
    pub header: [Option<u32>; 3],
    /// Elements hidden in P1 (other paperdoll variants, the store button).
    pub hidden: Vec<u32>,
    /// Property IDs of the stat rows: vital type, combat points stat property, percent
    /// cache property.
    pub stat_pids: [Option<u32>; 3],
    /// The element type `PropertyDisplay` (the header's name and level).
    pub property_display: Option<u32>,
}

impl PanelIds {
    /// Looks every part up: layouts by `UILAYOUT` label, elements by `UIElementID` label,
    /// properties by dictionary name.
    pub fn resolve(
        layout: &dyn Fn(&str) -> Option<Did>,
        element: &dyn Fn(&str) -> Option<u32>,
        pid: &dyn Fn(&str) -> Option<u32>,
        element_type: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut bags = [None; BAG_WINDOWS];
        for (b, l) in bags.iter_mut().zip(labels::BAGS) {
            *b = element(l);
        }
        Self {
            journal_layout: layout(labels::JOURNAL_LAYOUT),
            journal_window: element(labels::JOURNAL_WINDOW),
            container_layout: layout(labels::CONTAINER_LAYOUT),
            bags,
            slot_template: layout(labels::ITEM_DISPLAY_LAYOUT)
                .zip(element(labels::SLOTTED_TEMPLATE))
                .map(|(l, e)| ExamTemplate {
                    layout: l.0,
                    element: e,
                }),
            examination_layout: layout(labels::EXAMINATION_LAYOUT),
            header: [
                element(labels::PLAYER_NAME),
                element(labels::PLAYER_LEVEL),
                element(labels::PLAYER_RACE_CLASS),
            ],
            hidden: labels::OTHER_VARIANTS
                .iter()
                .chain(std::iter::once(&labels::ENHANCE_BUTTON))
                .filter_map(|l| element(l))
                .collect(),
            stat_pids: [
                pid("UIComponent_VitalDisplay_Type"),
                pid("UIComponent_CombatPointsDisplay_StatProperty"),
                pid("UIComponent_CombatPercentDisplay_CacheProperty"),
            ],
            property_display: element_type("PropertyDisplay"),
        }
    }
}

/// What a panel toggle did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggled {
    /// Opened or shown.
    Opened,
    /// Hidden.
    Closed,
    /// Not available (no install, no layout, a bag not offered).
    Unavailable,
}

/// One stat row found on the equipment page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatRow {
    Vital(ElementKey, VitalKind),
    Points(ElementKey, u32),
    Percent(ElementKey, u32),
}

/// The character panel and bag windows (module docs).
#[derive(Debug)]
pub struct CharacterUi {
    vocab: CharVocab,
    ids: PanelIds,
    slots: ItemSlots,
    tips: ExamTooltips,
    journal: Option<ElementKey>,
    avatar: Option<AvatarInfo>,
    stats: Vec<StatRow>,
    bags: [Option<BagView>; BAG_WINDOWS],
    /// The item whose tooltip shows, its input, and the pointer it was placed at.
    hover: Option<(ItemId, ExamTooltip, (i32, i32))>,
    pending_destroy: Option<(ItemId, Instant)>,
    commands: Vec<Command>,
    target: Option<EntityId>,
    split: bool,
    compare_equipped: bool,
    /// Messages for the player (refused pins, destroy confirmations), for the chat window.
    notices: Vec<String>,
    /// The paperdoll's viewport as an external-content host (`render.md` §6.10).
    viewports: Viewports,
    /// The user's bag column settings (`character-panels.md` §4.1) by full name.
    column_prefs: super::settings::P1Settings,
    /// Column settings changed by a resize since the last take, for the settings file.
    column_changes: Vec<(String, i32)>,
    /// Per open bag: the window's width and the list's horizontal margin in it when the
    /// list was attached or last re-columned (a resize changes the window's width).
    bag_widths: [Option<(i32, i32)>; BAG_WINDOWS],
}

/// First external-content id of the character panel's viewports (the client's id space).
pub const PORTRAIT_CONTENT_ID: u64 = 0x0050_0000;

/// What the client renders into a viewport this frame (`render.md` §6.10): the
/// paperdoll's figure, or the character screens' figure in the creation viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Portrait {
    /// The external-content id the UI composes.
    pub id: u64,
    /// The target size: the viewport's layout box.
    pub size: (u32, u32),
    /// The figure's turn, degrees (`character-panels.md` §3.2; the turntable of
    /// `character-screens.md` §2.3).
    pub yaw: f32,
    /// Which figure.
    pub figure: PortraitFigure,
}

/// The figure a [`Portrait`] shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortraitFigure {
    /// The player's own figure (the paperdoll, `character-panels.md` §3.2).
    Player,
    /// The character screens' viewport (`character-screens.md` §2.1): the race scene and
    /// the figure through the stage camera; `None` without a camera frame (the figure
    /// alone).
    Screens(Option<crate::walker::stage::StageView>),
}

fn slot_place(p: SlotPlace) -> Option<ItemPlace> {
    match p {
        SlotPlace::Backpack(i) => Some(ItemPlace::backpack(u16::try_from(i).ok()?)),
        SlotPlace::Equipment(k) => Some(ItemPlace::equipment(u16::try_from(k).ok()?)),
        SlotPlace::Other { .. } => None,
    }
}

/// The session command of an inventory-model command.
#[must_use]
pub fn session_command(c: InventoryCommand) -> Command {
    let id = NetItemId;
    Command::Inventory(match c {
        InventoryCommand::EquipItem { item, slot } => NetCommand::EquipItem {
            item: id(item),
            slot: Some(slot),
        },
        InventoryCommand::UnequipItem { item } => NetCommand::UnequipItem { item: id(item) },
        InventoryCommand::MoveItem { item, to } => NetCommand::MoveItem {
            item: id(item),
            to: super::state::session_place(to),
        },
        InventoryCommand::SplitStack { item, quantity } => NetCommand::SplitStack {
            item: id(item),
            quantity,
        },
        InventoryCommand::DestroyItem { item } => NetCommand::DestroyItem { item: id(item) },
        InventoryCommand::UseItem { item, target } => NetCommand::UseItem {
            item: id(item),
            target: target.map(EntityId),
        },
    })
}

/// What slot `place` shows for the model's item there (`character-panels.md` §5 with its
/// Tessera rules).
#[must_use]
pub fn slot_item(
    state: &PlayerState,
    data: Option<&CharacterData>,
    id: ItemId,
) -> Option<SlotItem> {
    let it = state.inventory().item(id)?;
    let facts = match &it.what {
        ItemRef::Record(d) => data.and_then(|x| x.item(*d)),
        ItemRef::Literal(_) => None,
    };
    let record_wear = facts.as_ref().and_then(|f| f.wear_state);
    let wear = match it.condition {
        Some((c, m)) => condition_wear_state(c, m, record_wear),
        None => record_wear,
    };
    let level = state.level().and_then(|l| i32::try_from(l).ok());
    // An unknown level or class passes, as in the tooltip engine's requirement rows
    // (`character-panels.md` §5 OPEN added by this implementer).
    let unusable = facts.as_ref().is_some_and(|f| {
        let low = f.min_level.zip(level).is_some_and(|(min, l)| min > l);
        let class = !f.required_classes.is_empty()
            && state
                .class()
                .is_some_and(|c| !f.required_classes.contains(&c));
        low || class
    });
    // `icon-layers.md` §3.1 step 1: the instance's image override first.
    let icon = match (data, &facts) {
        (Some(d), Some(_)) => tooltip::item_icon(d, state, id),
        _ => None,
    };
    Some(SlotItem {
        item: id,
        icon,
        quantity: i64::from(it.quantity),
        wear: match wear {
            Some(WEAR_BROKEN) => Wear::Broken,
            Some(WEAR_DAMAGED) => Wear::Damaged,
            _ => Wear::Sound,
        },
        unusable,
        cooldown: None,
        fits: facts
            .as_ref()
            .and_then(|f| f.slots.compatible)
            .map_or(0, |b| (b & 0xFFFF_FFFF) as u32),
    })
}

/// A property value as the stat rows show it: an integer, or with `percent` one decimal and
/// a percent sign (`character-panels.md` §3.4 Tessera rule).
#[must_use]
pub fn stat_text(v: &PropertyValue, percent: bool) -> Option<String> {
    let x = match v {
        PropertyValue::Int(i) => f64::from(*i),
        PropertyValue::Int64(i) => *i as f64,
        PropertyValue::Float(f) if f.is_finite() => f64::from(*f),
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => f64::from(*k),
        _ => return None,
    };
    Some(if percent {
        format!("{x:.1}%")
    } else {
        format!("{}", x.round() as i64)
    })
}

impl CharacterUi {
    /// The panels over `vocab` with the layouts and IDs of `ids`.
    #[must_use]
    pub fn new(vocab: CharVocab, ids: PanelIds) -> Self {
        let mut tips = ExamTooltips::new(vocab.clone());
        if let Some(l) = ids.examination_layout {
            tips.set_pin_layout(l.0);
        }
        Self {
            slots: ItemSlots::new(vocab.clone()),
            viewports: Viewports::new(vocab.clone(), PORTRAIT_CONTENT_ID),
            vocab,
            ids,
            tips,
            journal: None,
            avatar: None,
            stats: Vec::new(),
            bags: Default::default(),
            hover: None,
            pending_destroy: None,
            commands: Vec::new(),
            target: None,
            split: false,
            compare_equipped: true,
            notices: Vec::new(),
            column_prefs: super::settings::P1Settings::default(),
            column_changes: Vec::new(),
            bag_widths: [None; BAG_WINDOWS],
        }
    }

    /// Applies the P1 settings (`[UI]` of Tessera's settings file): the comparison switch
    /// (`item-tooltips.md` §8.3) and the bag column settings (`character-panels.md` §4.1),
    /// which bags opened from now on use.
    pub fn apply_settings(&mut self, settings: &super::settings::P1Settings) {
        self.compare_equipped = settings.compare_equipped;
        self.column_prefs = settings.clone();
    }

    /// Whether item tooltips compare with equipped items.
    #[must_use]
    pub fn compare_equipped(&self) -> bool {
        self.compare_equipped
    }

    /// Column settings a resize changed since the last call (full name, columns), for
    /// Tessera's settings file.
    pub fn take_column_changes(&mut self) -> Vec<(String, i32)> {
        std::mem::take(&mut self.column_changes)
    }

    /// The layouts and IDs in use.
    #[must_use]
    pub fn ids(&self) -> &PanelIds {
        &self.ids
    }

    /// The item slots (tests and sheets).
    #[must_use]
    pub fn slots(&self) -> &ItemSlots {
        &self.slots
    }

    /// The character panel's root, when open.
    #[must_use]
    pub fn journal(&self) -> Option<ElementKey> {
        self.journal
    }

    /// The stat rows found on the equipment page (`character-panels.md` §3.4).
    #[must_use]
    pub fn stat_row_count(&self) -> usize {
        self.stats.len()
    }

    /// The paperdoll's turn, degrees (`character-panels.md` §3.2).
    #[must_use]
    pub fn paperdoll_yaw(&self) -> Option<f32> {
        self.avatar.as_ref().map(|a| a.yaw)
    }

    /// The paperdoll's viewport element, when the panel is open.
    #[must_use]
    pub fn paperdoll_viewport(&self) -> Option<ElementKey> {
        self.avatar.as_ref().and_then(|a| a.viewport)
    }

    /// The paperdoll's portrait to render this frame: its content id, the target size and
    /// the figure's turn; `None` while the character panel is closed or hidden.
    #[must_use]
    pub fn portrait(&self, ui: &Ui) -> Option<Portrait> {
        let a = self.avatar.as_ref()?;
        let v = a.viewport?;
        let t = self
            .viewports
            .targets(ui)
            .into_iter()
            .find(|t| t.host.element == v)?;
        if !t.displayed_now {
            return None;
        }
        let w = u32::try_from(t.size.0).ok().filter(|w| *w > 0)?;
        let h = u32::try_from(t.size.1).ok().filter(|h| *h > 0)?;
        Some(Portrait {
            id: t.host.id,
            size: (w, h),
            yaw: a.yaw,
            figure: PortraitFigure::Player,
        })
    }

    /// Bag window `n` (1–6), when open.
    #[must_use]
    pub fn bag(&self, n: usize) -> Option<&BagView> {
        self.bags.get(n.checked_sub(1)?)?.as_ref()
    }

    /// The comparison option (`item-tooltips.md` §8.3 Tessera rule: on by default; the
    /// settings key `ui.compare_equipped_items`).
    pub fn set_compare_equipped(&mut self, on: bool) {
        self.compare_equipped = on;
    }

    /// The player's current target (for `UseItem`, `character-panels.md` §6). The client
    /// has no target selection yet, so nothing sets it and `UseItem` carries no target.
    pub fn set_target(&mut self, target: Option<EntityId>) {
        self.target = target;
    }

    /// The split modifier (shift) state at the next drop.
    pub fn set_split_modifier(&mut self, on: bool) {
        self.split = on;
    }

    /// Commands for the session, in order (each is sent once).
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }

    /// Messages for the player since the last call.
    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    // ----- opening and closing -----------------------------------------------------------

    /// Handles a panel input command by name (`ToggleJournalPanel`, `QUICKSLOT_BACKPACK`,
    /// `QUICKSLOT_BACKPACK1` … `6`, `TOOLTIP_DETACH`); returns `false` for other names.
    pub fn command(
        &mut self,
        ui: &mut Ui,
        name: &str,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> bool {
        if name.eq_ignore_ascii_case(labels::TOGGLE_JOURNAL) {
            self.toggle_journal(ui, state, data);
            return true;
        }
        if name.eq_ignore_ascii_case(labels::PIN_TOOLTIP) {
            self.pin(ui);
            return true;
        }
        if name.eq_ignore_ascii_case(labels::TOGGLE_BAGS) {
            self.toggle_all_bags(ui, state, data);
            return true;
        }
        let upper = name.to_ascii_uppercase();
        if let Some(n) = upper
            .strip_prefix(labels::TOGGLE_BAG_PREFIX)
            .and_then(|r| r.parse::<usize>().ok())
            .filter(|n| (1..=BAG_WINDOWS).contains(n))
        {
            self.toggle_bag(ui, n, state, data);
            return true;
        }
        false
    }

    /// Whether `k` is an open, shown window.
    fn is_open(ui: &Ui, k: Option<ElementKey>) -> bool {
        k.is_some_and(|k| ui.exists(k) && ui.is_visible(k))
    }

    /// Toggles the character panel (§2 Tessera rule).
    pub fn toggle_journal(
        &mut self,
        ui: &mut Ui,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> Toggled {
        if Self::is_open(ui, self.journal) {
            if let Some(k) = self.journal {
                ui.set_shown(k, false);
            }
            return Toggled::Closed;
        }
        self.open_journal(ui, state, data)
    }

    /// Opens (or shows) the character panel.
    pub fn open_journal(
        &mut self,
        ui: &mut Ui,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> Toggled {
        if let Some(k) = self.journal.filter(|k| ui.exists(*k)) {
            ui.set_shown(k, true);
            ui.request_activation(k);
            self.refresh_all(ui, state, data);
            return Toggled::Opened;
        }
        let (Some(layout), Some(root)) = (self.ids.journal_layout, self.ids.journal_window) else {
            return Toggled::Unavailable;
        };
        if let Some(t) = self.ids.property_display {
            // The header's name and level are `PropertyDisplay` elements: shown as text
            // (module docs; the fallback widget draws none).
            ui.register_game_type(t, tessera_ui::WidgetClass::Text);
        }
        let k = match ui.open_panel(layout, root) {
            Ok(k) => k,
            Err(e) => {
                tracing::info!("character panel not opened: {e}");
                return Toggled::Unavailable;
            }
        };
        ui.apply_saved(k);
        for id in &self.ids.hidden {
            if let Some(e) = find_below(ui, k, *id) {
                ui.set_shown(e, false);
            }
        }
        let avatar_type = self.vocab.types.avatar_info;
        if let Some(a) = subtree(ui, k)
            .into_iter()
            .find(|e| ui.element_type(*e) == Some(avatar_type))
        {
            let info = AvatarInfo::attach(ui, &self.vocab, a);
            if let Some(v) = info.viewport {
                self.viewports.attach_one(ui, v);
            }
            match info.slot_field {
                Some(f) => {
                    let n = self.slots.attach_paperdoll(ui, f);
                    tracing::debug!(slots = n, "paperdoll slots attached");
                }
                None => {
                    let n = self.slots.attach_paperdoll(ui, a);
                    tracing::debug!(slots = n, "paperdoll slots attached (no slot field)");
                }
            }
            self.avatar = Some(info);
        } else {
            // No `AvatarInfo`: the slots are still found by their `Container_Slot`.
            let n = self.slots.attach_paperdoll(ui, k);
            tracing::debug!(slots = n, "paperdoll slots attached (no AvatarInfo)");
        }
        self.stats = self.find_stat_rows(ui, k);
        ui.request_activation(k);
        self.journal = Some(k);
        self.refresh_all(ui, state, data);
        Toggled::Opened
    }

    fn find_stat_rows(&self, ui: &Ui, root: ElementKey) -> Vec<StatRow> {
        let [vital, points, percent] = self.ids.stat_pids;
        let mut out = Vec::new();
        for k in subtree(ui, root) {
            if let Some(p) = vital {
                match ui.prop(k, p).key() {
                    1 => out.push(StatRow::Vital(k, VitalKind::Morale)),
                    2 => out.push(StatRow::Vital(k, VitalKind::Power)),
                    _ => {}
                }
            }
            if let Some(p) = points {
                let v = ui.prop(k, p).key();
                if v != 0 {
                    out.push(StatRow::Points(k, v));
                }
            }
            if let Some(p) = percent {
                let v = ui.prop(k, p).key();
                if v != 0 {
                    out.push(StatRow::Percent(k, v));
                }
            }
        }
        out
    }

    /// Toggles bag window `n` (1–6, §4.1).
    pub fn toggle_bag(
        &mut self,
        ui: &mut Ui,
        n: usize,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> Toggled {
        let open = self
            .bag(n)
            .is_some_and(|b| ui.exists(b.window) && ui.is_visible(b.window));
        if open {
            if let Some(b) = self.bag(n) {
                ui.set_shown(b.window, false);
            }
            return Toggled::Closed;
        }
        self.open_bag(ui, n, state, data)
    }

    /// The bags offered for the current backpack size (§4.3 rule 4), 1-based.
    #[must_use]
    pub fn offered_bags(state: &PlayerState) -> Vec<usize> {
        state
            .inventory()
            .offered_bags()
            .into_iter()
            .filter_map(|(b, _)| usize::try_from(b).ok())
            .filter(|b| (1..=BAG_WINDOWS).contains(b))
            .collect()
    }

    /// `QUICKSLOT_BACKPACK` (§4.1 Tessera rule): if one offered bag is closed, open all of
    /// them; otherwise close all of them.
    pub fn toggle_all_bags(
        &mut self,
        ui: &mut Ui,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> Toggled {
        let offered = Self::offered_bags(state);
        if offered.is_empty() {
            return Toggled::Unavailable;
        }
        let any_closed = offered.iter().any(|n| {
            !self
                .bag(*n)
                .is_some_and(|b| ui.exists(b.window) && ui.is_visible(b.window))
        });
        for n in offered {
            if any_closed {
                self.open_bag(ui, n, state, data);
            } else if let Some(b) = self.bag(n) {
                ui.set_shown(b.window, false);
            }
        }
        if any_closed {
            Toggled::Opened
        } else {
            Toggled::Closed
        }
    }

    /// Opens bag window `n` (1–6) with its slots (§4.3), or shows it again.
    pub fn open_bag(
        &mut self,
        ui: &mut Ui,
        n: usize,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> Toggled {
        let Some(bag) = u32::try_from(n).ok().filter(|b| (1..=6).contains(b)) else {
            return Toggled::Unavailable;
        };
        let size = state.inventory().backpack_size();
        let Some(range) = state.inventory().bags().bag_range(bag, size) else {
            return Toggled::Unavailable;
        };
        let (Some(layout), Some(root), Some(template)) = (
            self.ids.container_layout,
            self.ids.bags[n - 1],
            self.ids.slot_template,
        ) else {
            return Toggled::Unavailable;
        };
        let window = match self.bags[n - 1].as_ref().map(|b| b.window) {
            Some(k) if ui.exists(k) => {
                ui.set_shown(k, true);
                k
            }
            _ => match ui.open_panel(layout, root) {
                Ok(k) => {
                    ui.apply_saved(k);
                    k
                }
                Err(e) => {
                    tracing::info!(bag = n, "bag window not opened: {e}");
                    return Toggled::Unavailable;
                }
            },
        };
        let same = self.bags[n - 1]
            .as_ref()
            .is_some_and(|b| b.window == window && b.range == range);
        if !same && !self.attach_bag_slots(ui, n, window, template, range) {
            return Toggled::Unavailable;
        }
        ui.request_activation(window);
        self.refresh_all(ui, state, data);
        Toggled::Opened
    }

    /// (Re)fills bag window `n`'s list with the slots of `range` (§4.3) and its column
    /// setting (§4.1). On failure the window is destroyed and `false` returned.
    fn attach_bag_slots(
        &mut self,
        ui: &mut Ui,
        n: usize,
        window: ElementKey,
        template: ExamTemplate,
        range: std::ops::Range<u32>,
    ) -> bool {
        let bag = u32::try_from(n).unwrap_or(0);
        // §4.1: the user's column setting overrides the data's 5 columns.
        let columns = column_setting(ui, &self.vocab, window)
            .and_then(|name| self.column_prefs.columns(&name));
        match self
            .slots
            .attach_bag(ui, window, bag, template, range, columns)
        {
            Ok(v) => {
                self.bag_widths[n - 1] = Self::widths(ui, &v);
                self.bags[n - 1] = Some(v);
                true
            }
            Err(e) => {
                tracing::info!(bag = n, "bag slots not attached: {e}");
                ui.destroy(window);
                self.bags[n - 1] = None;
                false
            }
        }
    }

    // ----- content -----------------------------------------------------------------------

    /// Applies one change list of the player's state to the open panels.
    pub fn refresh(
        &mut self,
        ui: &mut Ui,
        state: &PlayerState,
        data: Option<&CharacterData>,
        changes: &[PlayerChange],
    ) {
        if changes.is_empty() {
            return;
        }
        // A new backpack size changes the bag ranges: open bags take their new slots.
        let resized = changes.iter().any(|c| {
            matches!(
                c,
                PlayerChange::Reset
                    | PlayerChange::Inventory(
                        tessera_gamedata::inventory::InventoryChange::BackpackSize { .. }
                    )
            )
        });
        if resized {
            for n in 1..=BAG_WINDOWS {
                let Some(b) = self.bag(n).cloned() else {
                    continue;
                };
                if !ui.exists(b.window) {
                    self.bags[n - 1] = None;
                    continue;
                }
                let size = state.inventory().backpack_size();
                match state
                    .inventory()
                    .bags()
                    .bag_range(u32::try_from(n).unwrap_or(0), size)
                {
                    Some(r) if r == b.range => {}
                    Some(r) if ui.is_visible(b.window) => {
                        // The window stays where it is (no re-opening, no activation);
                        // only its list takes the new range.
                        if let Some(template) = self.ids.slot_template {
                            self.attach_bag_slots(ui, n, b.window, template, r);
                        }
                    }
                    _ => {
                        ui.destroy(b.window);
                        self.bags[n - 1] = None;
                    }
                }
            }
            self.slots.prune(ui);
        }
        self.refresh_all(ui, state, data);
    }

    /// Shows the model's content in every attached slot, the header and the stat rows.
    pub fn refresh_all(&mut self, ui: &mut Ui, state: &PlayerState, data: Option<&CharacterData>) {
        self.slots.prune(ui);
        let places: Vec<SlotPlace> = self.slots.slots().iter().map(|s| s.place).collect();
        for p in places {
            let item = slot_place(p)
                .and_then(|ip| state.inventory().at(ip))
                .and_then(|it| slot_item(state, data, it.id));
            let current = self.slots.slot(p).and_then(|s| s.item.clone());
            if current != item {
                self.slots.set(ui, p, item);
            }
        }
        if let Some(root) = self.journal.filter(|k| ui.exists(*k)) {
            self.header(ui, root, state, data);
            self.stat_rows(ui, state);
        }
    }

    fn header(
        &self,
        ui: &mut Ui,
        root: ElementKey,
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) {
        let [name, level, race_class] = self.ids.header;
        // Unknown values show an empty text (not the layout's placeholder).
        if let Some(e) = name.and_then(|id| find_below(ui, root, id)) {
            ui.set_text(e, Some(state.name().unwrap_or_default().to_owned()));
        }
        if let Some(e) = level.and_then(|id| find_below(ui, root, id)) {
            ui.set_text(
                e,
                Some(state.level().map(|l| l.to_string()).unwrap_or_default()),
            );
        }
        if let Some(e) = race_class.and_then(|id| find_below(ui, root, id)) {
            ui.set_text(e, Some(race_class_text(state, data).unwrap_or_default()));
        }
    }

    fn stat_rows(&self, ui: &mut Ui, state: &PlayerState) {
        for r in &self.stats {
            let (k, text) = match *r {
                StatRow::Vital(k, kind) => (
                    k,
                    state.vital(kind).map(|v| {
                        format!("{} / {}", v.current.round() as i64, v.max.round() as i64)
                    }),
                ),
                StatRow::Points(k, pid) => {
                    (k, state.property(pid).and_then(|v| stat_text(v, false)))
                }
                StatRow::Percent(k, pid) => {
                    (k, state.property(pid).and_then(|v| stat_text(v, true)))
                }
            };
            if ui.exists(k) {
                ui.set_text(k, Some(text.unwrap_or_default()));
            }
        }
    }

    // ----- events ------------------------------------------------------------------------

    /// Handles one UI event: item tooltips on hover, slot interactions, the paperdoll's
    /// turning and the pinned windows' close buttons. Returns whether it was the panels'.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        state: &PlayerState,
        data: Option<&CharacterData>,
        now: Instant,
    ) -> bool {
        if self.tips.handle_event(ui, ev) {
            return true;
        }
        if let Some(a) = self.avatar.as_mut()
            && a.handle_event(ui, ev)
        {
            return true;
        }
        if let UiEvent::ExamineHover { element, pointer } = ev {
            return self.show_tooltip(ui, *element, *pointer, state, data);
        }
        let Some(i) = self.slots.handle_event(ui, ev, self.split) else {
            return false;
        };
        self.interaction(i, state, data, now);
        true
    }

    /// The window width and the list's margin in it (window width − list width).
    fn widths(ui: &Ui, b: &BagView) -> Option<(i32, i32)> {
        let w = ui.layout_box(b.window)?;
        let l = ui.layout_box(b.list)?;
        let ww = w.r - w.l + 1;
        Some((ww, ww - (l.r - l.l + 1)))
    }

    /// A resized bag window (`ui-behaviour.md` §8.2) takes as many columns as its new
    /// width holds (`character-panels.md` §4.1: the resize bar changes the column setting);
    /// the new count is kept as the window's column setting.
    fn follow_bag_resizes(&mut self, ui: &mut Ui) {
        for n in 0..BAG_WINDOWS {
            let Some(b) = self.bags[n].clone() else {
                self.bag_widths[n] = None;
                continue;
            };
            if !ui.exists(b.window) || !ui.is_visible(b.window) {
                continue;
            }
            let Some((old_w, margin)) = self.bag_widths[n] else {
                self.bag_widths[n] = Self::widths(ui, &b);
                continue;
            };
            let Some(w) = ui.layout_box(b.window).map(|x| x.r - x.l + 1) else {
                continue;
            };
            if w == old_w {
                continue;
            }
            self.bag_widths[n] = Some((w, margin));
            let Some(cell) = b
                .entries
                .first()
                .and_then(|e| ui.layout_box(*e))
                .map(|x| x.r - x.l + 1)
            else {
                continue;
            };
            let columns = columns_for_width(w - margin, cell).min(super::settings::MAX_BAG_COLUMNS);
            if ui.list_grid(b.list).map(|g| g.0) == Some(columns) {
                continue;
            }
            set_columns(ui, b.list, columns);
            if let Some(name) = column_setting(ui, &self.vocab, b.window) {
                let full = super::settings::full_name(&name);
                if self.column_prefs.columns.get(&full) != Some(&columns) {
                    self.column_prefs.columns.insert(full.clone(), columns);
                    self.column_changes.retain(|(k, _)| *k != full);
                    self.column_changes.push((full, columns));
                }
            }
        }
    }

    /// Per frame: the paperdoll's drag turn, the bag windows' resizes and the slots'
    /// rollover overlay.
    pub fn update(&mut self, ui: &mut Ui) {
        if let Some(a) = self.avatar.as_mut() {
            a.update(ui);
        }
        self.follow_bag_resizes(ui);
        self.slots.update_rollover(ui);
        if self.hover.is_some() && ui.tooltip_panel().is_none() {
            self.hover = None;
        }
    }

    /// Shows the examination tooltip of the item in the slot at `element` (§9 Tessera
    /// rule: every item slot asks for it).
    fn show_tooltip(
        &mut self,
        ui: &mut Ui,
        element: ElementKey,
        pointer: (i32, i32),
        state: &PlayerState,
        data: Option<&CharacterData>,
    ) -> bool {
        let Some(slot) = self.slots.slot_of(ui, element) else {
            return false;
        };
        let Some(item) = slot.item.as_ref().map(|i| i.item) else {
            return false;
        };
        let Some(data) = data else {
            return false;
        };
        match tooltip::exam_tooltip_of(data, state, item, Purpose::Hover, self.compare_equipped) {
            Ok(input) => match self.tips.show(ui, &input, pointer) {
                Ok(_) => {
                    self.hover = Some((item, input, pointer));
                }
                Err(e) => tracing::debug!(item, "item tooltip not shown: {e}"),
            },
            Err(e) => tracing::debug!(item, "no item tooltip: {e}"),
        }
        true
    }

    /// `TOOLTIP_DETACH` (§10 Tessera rule): pins the showing item tooltip, or closes its
    /// pinned copy.
    pub fn pin(&mut self, ui: &mut Ui) -> Option<PinOutcome> {
        let (_, input, pointer) = self.hover.clone()?;
        // A pinned tooltip uses the hover pair (§10) and shows no comparisons.
        let input = ExamTooltip {
            comparisons: Vec::new(),
            ..input
        };
        match self.tips.pin(ui, &input, pointer) {
            Ok(PinOutcome::Refused) => {
                self.notices
                    .push("No more tooltips can be pinned; close one first.".into());
                Some(PinOutcome::Refused)
            }
            Ok(o) => Some(o),
            Err(e) => {
                tracing::debug!("tooltip not pinned: {e}");
                None
            }
        }
    }

    /// The pinned tooltip windows.
    pub fn pinned(
        &self,
    ) -> impl Iterator<
        Item = &(
            tessera_ui::game::examine::ExamKey,
            tessera_ui::game::examine::PinnedWindow,
        ),
    > {
        self.tips.pinned()
    }

    /// Turns a slot interaction into commands (§6).
    fn interaction(
        &mut self,
        i: SlotInteraction,
        state: &PlayerState,
        data: Option<&CharacterData>,
        now: Instant,
    ) {
        let (item, action) = match i {
            SlotInteraction::Dropped {
                item,
                target,
                split,
                ..
            } => {
                let target = match target {
                    DropPlace::Slot(SlotPlace::Equipment(k)) => {
                        let Ok(k) = u16::try_from(k) else { return };
                        DropTarget::PaperdollSlot(k)
                    }
                    DropPlace::Slot(p) => match slot_place(p) {
                        Some(ip) => DropTarget::Slot(ip),
                        None => return,
                    },
                    DropPlace::Area(_) => DropTarget::BagWindow,
                };
                (item, ItemAction::Drop { target, split })
            }
            SlotInteraction::Activated { item, .. } => (item, ItemAction::Activate),
            SlotInteraction::DestroyRequested { item, .. } => match self.pending_destroy {
                Some((p, at)) if p == item && now.duration_since(at) <= DESTROY_CONFIRM_WINDOW => {
                    self.pending_destroy = None;
                    (item, ItemAction::DestroyConfirmed)
                }
                _ => {
                    self.pending_destroy = Some((item, now));
                    self.notices
                        .push("Drag the item out of the windows again to destroy it.".into());
                    return;
                }
            },
        };
        let slots = state
            .inventory()
            .item(item)
            .and_then(|it| match &it.what {
                ItemRef::Record(d) => data.and_then(|x| x.item(*d)),
                ItemRef::Literal(_) => None,
            })
            .map(|f| f.slots)
            .unwrap_or_default();
        let rules = data.map(|d| d.rules().clone()).unwrap_or_default();
        if let Some(c) =
            state
                .inventory()
                .choose_command(item, slots, action, &rules, self.target.map(|t| t.0))
        {
            tracing::debug!(?c, "item interaction");
            self.commands.push(session_command(c));
        }
    }

    /// Forgets closed windows and destroyed elements.
    pub fn prune(&mut self, ui: &mut Ui) {
        self.slots.prune(ui);
        self.tips.prune(ui);
        self.viewports.prune(ui);
        if self.journal.is_some_and(|k| !ui.exists(k)) {
            self.journal = None;
            self.avatar = None;
            self.stats.clear();
        }
        for b in &mut self.bags {
            if b.as_ref().is_some_and(|v| !ui.exists(v.window)) {
                *b = None;
            }
        }
    }
}

/// Whether `name` is one of the panel input commands [`CharacterUi::command`] runs.
#[must_use]
pub fn is_panel_command(name: &str) -> bool {
    name.eq_ignore_ascii_case(labels::TOGGLE_JOURNAL)
        || name.eq_ignore_ascii_case(labels::PIN_TOOLTIP)
        || name.eq_ignore_ascii_case(labels::TOGGLE_BAGS)
        || name
            .to_ascii_uppercase()
            .strip_prefix(labels::TOGGLE_BAG_PREFIX)
            .and_then(|r| r.parse::<usize>().ok())
            .is_some_and(|n| (1..=BAG_WINDOWS).contains(&n))
}

/// The race and class display texts of the player, joined by a space (module docs).
#[must_use]
pub fn race_class_text(state: &PlayerState, data: Option<&CharacterData>) -> Option<String> {
    use tessera_gamedata::tooltip::TooltipData;
    let data = data?;
    let d = data.engine().data();
    let race = match state.body() {
        Some(tessera_net::Body::Character { species, .. }) => d
            .pid("Usage_RequiredRaceEntry")
            .and_then(|p| d.enum_text(p, species)),
        _ => None,
    };
    let class = state
        .class()
        .and_then(|c| d.pid("Usage_ClassType").and_then(|p| d.enum_text(p, c)));
    let parts: Vec<String> = [race, class].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Whether `place` is in a container a slot shows.
#[must_use]
pub fn shown_container(place: ItemPlace) -> bool {
    matches!(place.container, Container::Backpack | Container::Equipment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_texts() {
        assert_eq!(
            stat_text(&PropertyValue::Int(12), false).as_deref(),
            Some("12")
        );
        assert_eq!(
            stat_text(&PropertyValue::Float(12.46), true).as_deref(),
            Some("12.5%")
        );
        assert_eq!(stat_text(&PropertyValue::Float(f32::NAN), false), None);
        assert_eq!(stat_text(&PropertyValue::String("x".into()), false), None);
    }

    #[test]
    fn commands_map_to_session_commands() {
        let c = session_command(InventoryCommand::EquipItem { item: 4, slot: 3 });
        assert_eq!(
            c,
            Command::Inventory(NetCommand::EquipItem {
                item: NetItemId(4),
                slot: Some(3)
            })
        );
        let c = session_command(InventoryCommand::MoveItem {
            item: 4,
            to: ItemPlace::backpack(10),
        });
        assert!(matches!(
            c,
            Command::Inventory(NetCommand::MoveItem { to, .. })
                if to.slot == 10 && to.container == tessera_net::domain::Container::Backpack
        ));
    }
}
