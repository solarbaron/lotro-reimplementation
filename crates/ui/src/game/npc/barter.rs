// SPDX-License-Identifier: GPL-3.0-or-later
//! The barter window (`npc-windows.md` §7.7): a `VendorShown` in which an entry has an
//! item cost (§7.3 Tessera rule). Entries of 20 per page, each with its reward and up to
//! three cost slots (item costs in order, then a coin slot when the coin price is above
//! 0); info texts with the required quantity, red when the player holds fewer.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npcvocab::NpcVocab;
use super::super::{IconImage, set_icon};
use super::coins::{CoinCounts, MoneyDisplay, MoneyOptions, fallback_display};
use super::kit::{
    ClickKind, FNode, FallbackArt, Listen, PAGE_NEXT_ID, PAGE_PREV_ID, click_of, colour_value,
    colours, commit_rows, find, ghost, icon_part, instantiate, make_row, mark_row, put_text,
    row_from_template, row_of, text_part,
};
use super::{NpcAction, NpcTexts, WindowId};

/// Entries per page (`UI_Barter_PageSize` 20 here, §7.7).
pub const PAGE_SIZE: usize = 20;

/// Most entries a barter window keeps (Tessera bound; the feed sends at most 1,024).
pub const MAX_ENTRIES: usize = 1024;

/// Cost slots per entry (`BarterItem1` … `BarterItem3`).
pub const COST_SLOTS: usize = 3;

/// One cost of a barter entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarterCost {
    /// An item cost: the item and the required quantity, with what the player holds (a
    /// wallet item's last reported amount, or the backpack's quantity of the record).
    Item {
        /// The item's name.
        name: String,
        /// The composed icon.
        icon: Option<IconImage>,
        /// The required quantity.
        required: i64,
        /// What the player holds.
        held: i64,
    },
    /// The coin price (drawn as a money display in its slot).
    Coin {
        /// The coins.
        coins: CoinCounts,
        /// Whether the player cannot pay it (`TooExpensive`).
        too_expensive: bool,
    },
}

/// One barter entry, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BarterEntry {
    /// The feed's entry number.
    pub entry: u16,
    /// The reward's name.
    pub name: String,
    /// The reward's icon.
    pub icon: Option<IconImage>,
    /// The reward's quantity (overlay above 1).
    pub quantity: i64,
    /// The costs: item costs in order, then the coin cost when the coin price is above 0.
    pub costs: Vec<BarterCost>,
}

/// The reward's tooltip for costs beyond the three slots (§7.7 Tessera rule): one line per
/// cost, "quantity name" for an item cost and the client's coin text
/// ([`NpcTexts::coins_text`]) for the coin cost; `None` when every cost has a slot.
#[must_use]
pub fn extra_costs(e: &BarterEntry, texts: &dyn NpcTexts) -> Option<String> {
    let extra: Vec<String> = e
        .costs
        .iter()
        .skip(COST_SLOTS)
        .map(|c| match c {
            BarterCost::Item { name, required, .. } => {
                format!("{} {name}", texts.amount(*required))
            }
            BarterCost::Coin { coins, .. } => texts.coins_text(*coins),
        })
        .collect();
    (!extra.is_empty()).then(|| extra.join("\n"))
}

/// Size of a barter entry (`BarterEntryTemplate`, 608 × 65).
pub const ENTRY_SIZE: (i32, i32) = (608, 65);

/// The x positions of the three cost slots (§7.7).
pub const COST_X: [i32; 3] = [388, 467, 545];

/// A barter window instance.
#[derive(Debug, Clone)]
pub struct BarterWindow {
    /// The feed window.
    pub window: WindowId,
    /// The root (`Barter_Window`).
    pub root: ElementKey,
    /// The entries.
    pub entries: Vec<BarterEntry>,
    /// The page shown (0-based).
    pub page: usize,
    /// The selected entry (index into `entries`).
    pub selected: Option<usize>,
    /// `BarterList_Listbox`.
    pub list: Option<ElementKey>,
    /// The rows of the page shown, with their entry indices.
    pub rows: Vec<(ElementKey, usize)>,
    buy: Option<ElementKey>,
    page_buttons: (Option<ElementKey>, Option<ElementKey>),
    template: Option<ExamTemplate>,
    coin_template: Option<ExamTemplate>,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

impl BarterWindow {
    /// Finds the window's parts below `root`: the profile menu shows the feed's one list
    /// and is ghosted (§7.7 Tessera rule); the page control `BarterList_PagCtrl` gets
    /// Tessera's previous and next buttons ([`page_buttons`]).
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        template: Option<ExamTemplate>,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        ghost(ui, f(ui, ids.barter_profile_menu), true);
        let mut w = Self {
            window: 0,
            root,
            entries: Vec::new(),
            page: 0,
            selected: None,
            list: f(ui, ids.barter_list),
            rows: Vec::new(),
            buy: f(ui, ids.barter_buy),
            page_buttons: page_buttons(ui, nv, f(ui, ids.barter_pages), art),
            template,
            coin_template: None,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        };
        w.listen.clicks(ui, w.buy);
        let (prev, next) = w.page_buttons;
        w.listen.clicks(ui, prev);
        w.listen.clicks(ui, next);
        ghost(ui, prev, true);
        ghost(ui, next, true);
        w
    }

    /// Other previous and next page buttons, replacing the page control's (for a window
    /// without `BarterList_PagCtrl`).
    pub fn set_page_buttons(
        &mut self,
        ui: &mut Ui,
        prev: Option<ElementKey>,
        next: Option<ElementKey>,
    ) {
        self.page_buttons = (prev, next);
        self.listen.clicks(ui, prev);
        self.listen.clicks(ui, next);
        ghost(ui, prev, self.page == 0);
        ghost(ui, next, self.page + 1 >= self.pages());
    }

    /// The install's money-display template for coin costs (the money template of layout
    /// `moneydisplay`, §9.2); without it a coin cost uses the fallback coin display, whose
    /// fonts and coin images exist only on the fallback path.
    pub fn set_coin_template(&mut self, template: Option<ExamTemplate>) {
        self.coin_template = template;
    }

    /// The previous and next page buttons in use.
    #[must_use]
    pub fn page_buttons(&self) -> (Option<ElementKey>, Option<ElementKey>) {
        self.page_buttons
    }

    /// The number of pages (at least 1).
    #[must_use]
    pub fn pages(&self) -> usize {
        self.entries.len().div_ceil(PAGE_SIZE).max(1)
    }

    /// Shows a `VendorShown` with item costs (§7.7): page 1.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        window: WindowId,
        entries: &[BarterEntry],
        texts: &dyn NpcTexts,
    ) {
        self.window = window;
        self.entries = entries.iter().take(MAX_ENTRIES).cloned().collect();
        self.selected = None;
        self.set_page(ui, 0, texts);
    }

    /// Shows page `page` (clamped), 20 entries in entry order.
    pub fn set_page(&mut self, ui: &mut Ui, page: usize, texts: &dyn NpcTexts) {
        self.page = page.min(self.pages() - 1);
        let Some(list) = self.list else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        super::kit::clear_rows(ui, list, &mut old);
        let start = self.page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.entries.len());
        for i in start..end {
            let e = self.entries[i].clone();
            let Some(k) = self.make_entry(ui, list) else {
                break;
            };
            self.fill_entry(ui, k, &e, texts);
            if self.selected == Some(i) {
                mark_row(ui, k, true);
            }
            self.listen.clicks(ui, Some(k));
            self.rows.push((k, i));
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let kept = commit_rows(ui, list, &ks);
        self.rows.retain(|(k, _)| kept.contains(k));
        ghost(ui, self.page_buttons.0, self.page == 0);
        ghost(ui, self.page_buttons.1, self.page + 1 >= self.pages());
    }

    fn make_entry(&self, ui: &mut Ui, list: ElementKey) -> Option<ElementKey> {
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = self.art.clone();
        let st = ui.vocab().states;
        make_row(ui, list, self.template, || {
            let mut n = FNode::field(ids.barter_entry, 0, 0, ENTRY_SIZE.0, ENTRY_SIZE.1)
                .image(&art.row)
                .state(st.normal, art.row.as_ref(), vec![])
                .state(st.highlight, art.row_highlight.as_ref(), vec![])
                .child(
                    FNode::field(ids.barter_reward_item, 10, 16, 362, 32)
                        .child(
                            FNode::field(0, 0, 0, 32, 32)
                                .image(&art.slot)
                                .child(FNode::field(ids.item_icon, 0, 0, 32, 32))
                                .child(
                                    FNode::text(
                                        ids.item_quantity,
                                        0,
                                        18,
                                        30,
                                        14,
                                        cp,
                                        colours::WHITE,
                                        "",
                                    )
                                    .hidden(),
                                ),
                        )
                        .child(FNode::text(
                            ids.item_name,
                            40,
                            7,
                            320,
                            18,
                            cp,
                            colours::WHITE,
                            "",
                        )),
                );
            for (j, (slot, info)) in [
                (ids.barter_cost1, ids.barter_info1),
                (ids.barter_cost2, ids.barter_info2),
                (ids.barter_cost3, ids.barter_info3),
            ]
            .into_iter()
            .enumerate()
            {
                n = n
                    .child(
                        FNode::field(slot, COST_X[j], 8, 32, 32)
                            .image(&art.slot)
                            .child(FNode::field(ids.item_icon, 0, 0, 32, 32)),
                    )
                    .child(FNode::text(
                        info,
                        COST_X[j] - 14,
                        44,
                        60,
                        16,
                        cp,
                        colours::WHITE,
                        "",
                    ));
            }
            n
        })
    }

    fn fill_entry(&self, ui: &mut Ui, k: ElementKey, e: &BarterEntry, texts: &dyn NpcTexts) {
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let reward = find(ui, k, ids.barter_reward_item).unwrap_or(k);
        // §7.7: the reward is the item display *or* the reputation field; every feed reward
        // is an item, so the reputation alternative (with the layout's sample lines) stays
        // hidden.
        if let Some(rep) = find(ui, k, ids.barter_reputation_reward) {
            ui.set_shown(rep, false);
        }
        ui.set_shown(reward, true);
        put_text(ui, text_part(ui, reward, Some(ids.item_name)), &e.name);
        if let Some(ic) = icon_part(ui, reward, Some(ids.item_icon)) {
            set_icon(ui, ic, e.icon.as_ref());
        }
        if let Some(q) = find(ui, reward, ids.item_quantity) {
            ui.set_shown(q, e.quantity > 1);
            if e.quantity > 1 {
                ui.set_text(q, Some(texts.amount(e.quantity)));
            }
        }
        let slots = [
            (ids.barter_cost1, ids.barter_info1),
            (ids.barter_cost2, ids.barter_info2),
            (ids.barter_cost3, ids.barter_info3),
        ];
        for (j, (slot_id, info_id)) in slots.into_iter().enumerate() {
            let slot = find(ui, k, slot_id);
            let info = find(ui, k, info_id);
            match e.costs.get(j) {
                None => {
                    super::super::show(ui, slot, false);
                    super::super::show(ui, info, false);
                }
                Some(BarterCost::Item {
                    icon,
                    required,
                    held,
                    ..
                }) => {
                    super::super::show(ui, slot, true);
                    if let Some(ic) = slot.and_then(|s| icon_part(ui, s, Some(ids.item_icon))) {
                        set_icon(ui, ic, icon.as_ref());
                    }
                    if let Some(i) = info {
                        ui.set_shown(i, true);
                        ui.set_text(i, Some(texts.amount(*required)));
                        if held < required {
                            ui.set_property(i, cp, colour_value(colours::RED));
                        }
                    }
                }
                Some(BarterCost::Coin {
                    coins,
                    too_expensive,
                }) => {
                    // §7.7 Tessera rule: the coin slot is a money display in place of the
                    // icon and the info text. The display sits where the slot is, moved
                    // left as far as its shown coins need to stay inside the entry.
                    super::super::show(ui, info, false);
                    let Some(s) = slot else { continue };
                    ui.set_shown(s, false);
                    let Some(holder) = ui.parent(s) else { continue };
                    let sb = ui.layout_box(s).unwrap_or_default();
                    let hw = ui.layout_box(holder).map_or(0, |b| b.w());
                    let shown = super::coins::visible(*coins, MoneyOptions::default())
                        .iter()
                        .filter(|v| **v)
                        .count();
                    let need = i32::try_from(shown).unwrap_or(3) * super::coins::COIN_SIZE.0;
                    let x = sb.l.min(hw.saturating_sub(4).saturating_sub(need)).max(0);
                    // The install's money template when there is one (its coin elements have
                    // the install's font and coin images), else the fallback display.
                    let from_template = self.coin_template.and_then(|t| {
                        let m = row_from_template(ui, t)?;
                        if ui.attach_raw(m, holder).is_err() {
                            ui.destroy(m);
                            return None;
                        }
                        let Some(d) = MoneyDisplay::find(ui, &self.nv, m) else {
                            ui.destroy(m);
                            return None;
                        };
                        // Coins read from the slot's left edge (as the fallback display's
                        // left-justified coins): the display's own left offset in the
                        // template is taken off.
                        ui.set_property(
                            d,
                            self.nv.pids.left_justified,
                            tessera_formats::property::PropertyValue::Bool(true),
                        );
                        let off = if ui.parent(d) == Some(m) {
                            ui.layout_box(d).map_or(0, |b| b.l)
                        } else {
                            0
                        };
                        ui.move_to(m, x.saturating_sub(off), sb.t + 5);
                        // The install's display lays its coins out from its right edge: it
                        // is sized to the shown coins so they start at the slot.
                        if d == m {
                            let h = ui.layout_box(d).map_or(22, |b| b.h());
                            ui.resize(d, need.max(1), h);
                        }
                        Some(d)
                    });
                    let display = from_template.or_else(|| {
                        let v = ui.vocab().clone();
                        let node = fallback_display(
                            &v,
                            &self.nv,
                            0,
                            x,
                            sb.t + 5,
                            MoneyOptions::LEFT_JUSTIFIED,
                        );
                        instantiate(ui, holder, &node)
                    });
                    if let Some(m) = display {
                        let mut d = MoneyDisplay::attach(ui, &self.nv, m, &self.art);
                        d.set(ui, *coins, *too_expensive, texts);
                    }
                }
            }
        }
        // Costs beyond three slots are listed in the reward's tooltip.
        if let Some(t) = extra_costs(e, texts) {
            ui.set_tooltip_text(reward, Some(t));
        }
    }

    /// The command for a UI event (§7.7 Tessera rule): `Barter_BuyButton`, a right click or
    /// a left double-click on an entry send `Buy { entry, quantity: 1 }`; a left click
    /// selects; the page buttons turn pages.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn NpcTexts,
    ) -> Option<NpcAction> {
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        let press = matches!(kind, ClickKind::Button | ClickKind::Left);
        if press && hit(self.buy) {
            let i = self.selected?;
            return Some(self.buy_action(i));
        }
        if press && hit(self.page_buttons.0) {
            let p = self.page.saturating_sub(1);
            self.set_page(ui, p, texts);
            return None;
        }
        if press && hit(self.page_buttons.1) {
            let p = self.page + 1;
            self.set_page(ui, p, texts);
            return None;
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let r = row_of(ui, &ks, src)?;
        let i = self.rows[r].1;
        if let Some(old) = self
            .selected
            .and_then(|s| self.rows.iter().find(|(_, j)| *j == s))
        {
            mark_row(ui, old.0, false);
        }
        self.selected = Some(i);
        mark_row(ui, ks[r], true);
        matches!(kind, ClickKind::Right | ClickKind::Double).then(|| self.buy_action(i))
    }

    fn buy_action(&self, i: usize) -> NpcAction {
        NpcAction::Buy {
            window: self.window,
            entry: self.entries.get(i).map_or(0, |e| e.entry),
            quantity: 1,
        }
    }

    /// `CloseWindow` for this window (`EscapeKey`).
    #[must_use]
    pub fn close_action(&self) -> NpcAction {
        NpcAction::CloseWindow {
            window: self.window,
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}

/// The previous and next page buttons of the page control `pc` (§7.7 page control; Tessera
/// rule while the control's parts are not specified, `npc-windows.md` §7.7 OPEN): the
/// buttons with Tessera's IDs [`PAGE_PREV_ID`] and [`PAGE_NEXT_ID`] below `pc` (the
/// fallback window builds them there); when there are none (the install's control), the
/// control's own children are hidden and the two buttons are created inside it, each half
/// the control's width (at most 30 pixels) and its full height, at its left edge.
pub fn page_buttons(
    ui: &mut Ui,
    nv: &NpcVocab,
    pc: Option<ElementKey>,
    art: &FallbackArt,
) -> (Option<ElementKey>, Option<ElementKey>) {
    let Some(pc) = pc else {
        return (None, None);
    };
    let prev = find(ui, pc, PAGE_PREV_ID);
    let next = find(ui, pc, PAGE_NEXT_ID);
    if prev.is_some() || next.is_some() {
        return (prev, next);
    }
    for c in ui.children(pc) {
        ui.set_shown(c, false);
    }
    let b = ui.layout_box(pc).unwrap_or_default();
    let h = b.h().clamp(12, 64);
    let w = (b.w() / 2).clamp(12, 30);
    let cp = nv.pids.text_font_color;
    let ghosted = ui.vocab().states.ghosted;
    let button = |id: u32, x: i32, t: &str| {
        FNode::button(
            id,
            x,
            0,
            w,
            h,
            (&art.button, &art.button_ghosted),
            cp,
            ghosted,
            t,
        )
    };
    let prev = instantiate(ui, pc, &button(PAGE_PREV_ID, 0, "<"));
    let next = instantiate(ui, pc, &button(PAGE_NEXT_ID, w, ">"));
    (prev, next)
}
