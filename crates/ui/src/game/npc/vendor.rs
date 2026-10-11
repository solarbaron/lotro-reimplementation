// SPDX-License-Identifier: GPL-3.0-or-later
//! The vendor window (`npc-windows.md` §7.1–§7.6, §7.8): the buy, sell and buy-back pages
//! with their two-column entry lists (§7.2), prices in coins with `TooExpensive` (§7.2,
//! §7.3), the buy modes of the held toggles and the purchase quantities (§7.4), selling
//! (§7.6), buying back and the command table (§7.8).

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::examine::ExamTemplate;
use super::super::npcvocab::NpcVocab;
use super::super::{IconImage, set_icon, show};
use super::coins::{CoinCounts, MoneyDisplay, MoneyOptions, fallback_display, set_too_expensive};
use super::kit::{
    ClickKind, FNode, FallbackArt, Listen, click_of, colour_value, colours, commit_rows, find,
    ghost, icon_part, make_row, mark_row, put_text, row_of, text_part,
};
use super::{NpcAction, NpcLabel, NpcTexts, WindowId, label_text};

/// Most entries one page lists (Tessera bound; the feed sends at most 1,024).
pub const MAX_ENTRIES: usize = 1024;

/// The buy mode of the held `VendorBuyUI` toggles (§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BuyMode {
    /// No toggle held: buy one.
    #[default]
    Single,
    /// `ToggleQuantityDisplay` (Alt): the quantity dialog.
    Quantity,
    /// `ToggleStackDisplay` (Shift): a full stack.
    Stack,
    /// `ToggleFillStackDisplay` (Tab): fill the partial stacks.
    Fill,
}

/// The held toggles of the `VendorBuyUI` input map (§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuyToggles {
    /// `ToggleQuantityDisplay` held.
    pub quantity: bool,
    /// `ToggleStackDisplay` held.
    pub stack: bool,
    /// `ToggleFillStackDisplay` held.
    pub fill: bool,
}

impl BuyToggles {
    /// The mode: several can be held, the first of quantity, stack, fill wins; none →
    /// single.
    #[must_use]
    pub fn mode(&self) -> BuyMode {
        if self.quantity {
            BuyMode::Quantity
        } else if self.stack {
            BuyMode::Stack
        } else if self.fill {
            BuyMode::Fill
        } else {
            BuyMode::Single
        }
    }
}

/// What buying the selected entry does (§7.4 cases 1–5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyDecision {
    /// Send `Buy` with this quantity.
    Buy(u32),
    /// Open the quantity dialog with this maximum (case 2).
    Dialog(u32),
}

/// §7.4: a stack size below 2 always buys one; quantity mode opens the dialog with the
/// stack size as its maximum; stack mode buys a full stack; fill mode buys `fill`
/// ([`fill_quantity`]); single mode buys one.
#[must_use]
pub fn buy_decision(mode: BuyMode, stack_size: u32, fill: u32) -> BuyDecision {
    if stack_size < 2 {
        return BuyDecision::Buy(1);
    }
    match mode {
        BuyMode::Quantity => BuyDecision::Dialog(stack_size),
        BuyMode::Stack => BuyDecision::Buy(stack_size),
        BuyMode::Fill => BuyDecision::Buy(fill.max(1)),
        BuyMode::Single => BuyDecision::Buy(1),
    }
}

/// §7.4 Tessera rule (commands), case 4: the quantity that fills the backpack's partial
/// stacks of the record: stack size minus quantity, summed over the record's backpack
/// stacks below the stack size; 1 when there is none.
#[must_use]
pub fn fill_quantity(stack_size: u32, stacks: &[u64]) -> u32 {
    let s = u64::from(stack_size);
    let sum = stacks
        .iter()
        .filter(|q| **q < s)
        .fold(0u64, |acc, q| acc.saturating_add(s - q));
    if sum == 0 {
        1
    } else {
        u32::try_from(sum).unwrap_or(u32::MAX)
    }
}

/// One entry of the buy or buy-back list, rendered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VendorItem {
    /// The feed's entry number.
    pub entry: u16,
    /// The item's name (or the literal).
    pub name: String,
    /// The name's quality colour (`item-tooltips.md` §6.2), `0xAARRGGBB`.
    pub colour: Option<u32>,
    /// The composed icon.
    pub icon: Option<IconImage>,
    /// The unit coin price (units of the value-1 currency).
    pub price: u64,
    /// The unit coin price split into coins (§9.3).
    pub coins: CoinCounts,
    /// `Inventory_MaxQuantity` (the stack size; 0 or 1: not stackable).
    pub stack_size: u32,
    /// The feed's stock (`None`: unlimited).
    pub stock: Option<u32>,
    /// The entry's quantity (buy-back entries).
    pub quantity: u32,
    /// The fill-mode quantity ([`fill_quantity`] over the backpack's stacks of the record).
    pub fill: u32,
    /// The client's reference for the examination tooltip.
    pub item: Option<u64>,
}

/// One entry of the sell list (§7.6 Tessera rule 1), rendered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SellItem {
    /// The client's item reference (the item instance).
    pub item: u64,
    /// The item's name.
    pub name: String,
    /// The name's quality colour.
    pub colour: Option<u32>,
    /// The composed icon.
    pub icon: Option<IconImage>,
    /// The stack quantity.
    pub quantity: i64,
    /// The worth (`item-tooltips.md` §6.11: value × quantity) in coins.
    pub worth: CoinCounts,
}

/// The vendor window's pages (§7.1; the repair page is hidden in P3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VendorPage {
    /// The buy page (open at start).
    #[default]
    Buy,
    /// The sell page.
    Sell,
    /// The buy-back page.
    BuyBack,
}

/// The entry templates (`VendorItem` and `VendorSellItem` of layout `vendor_item`, §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VendorTemplates {
    /// Buy and buy-back entries.
    pub entry: Option<ExamTemplate>,
    /// Sell entries.
    pub sell_entry: Option<ExamTemplate>,
}

/// What a vendor-window event asks the client to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorOutcome {
    /// Send these commands, in order.
    Send(Vec<NpcAction>),
    /// Open the quantity dialog for this entry with this maximum (§7.4 case 2).
    AskQuantity {
        /// The entry number.
        entry: u16,
        /// *M*.
        max: u32,
    },
}

/// One page's list.
#[derive(Debug, Clone, Default)]
struct PageList {
    page: Option<ElementKey>,
    tab: Option<ElementKey>,
    list: Option<ElementKey>,
    rows: Vec<ElementKey>,
    money: Vec<Option<MoneyDisplay>>,
    overlays: Vec<Option<ElementKey>>,
    selected: Option<usize>,
}

impl PageList {
    fn select(&mut self, ui: &mut Ui, i: Option<usize>) {
        if let Some(old) = self.selected.and_then(|o| self.rows.get(o)) {
            mark_row(ui, *old, false);
        }
        self.selected = i.filter(|i| *i < self.rows.len());
        if let Some(r) = self.selected.and_then(|i| self.rows.get(i)) {
            mark_row(ui, *r, true);
        }
    }
}

/// Size of a vendor entry (`VendorItemDisplay`, 183 × 64).
pub const ENTRY_SIZE: (i32, i32) = (183, 64);

/// A vendor window instance.
#[derive(Debug, Clone)]
pub struct VendorWindow {
    /// The feed window.
    pub window: WindowId,
    /// The root (`Vendor_Window`).
    pub root: ElementKey,
    /// The page shown.
    pub page: VendorPage,
    /// The buy entries.
    pub buy: Vec<VendorItem>,
    /// The sell entries.
    pub sell: Vec<SellItem>,
    /// The buy-back entries (`None` until a `BuybackShown`).
    pub buyback: Option<Vec<VendorItem>>,
    /// The held toggles.
    pub toggles: BuyToggles,
    /// The player's money (the last `WalletChanged { Coin }`).
    pub wallet: u64,
    /// The player's money display (`Vendor_PlayerMoney`).
    pub player_money: Option<MoneyDisplay>,
    /// The sell value display (`Vendor_SellValue`).
    pub sell_value: Option<MoneyDisplay>,
    lists: [PageList; 3],
    close: Option<ElementKey>,
    buy_button: Option<ElementKey>,
    sell_button: Option<ElementKey>,
    sell_all: Option<ElementKey>,
    buyback_button: Option<ElementKey>,
    stack_hint: Option<ElementKey>,
    templates: VendorTemplates,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

fn index(p: VendorPage) -> usize {
    match p {
        VendorPage::Buy => 0,
        VendorPage::Sell => 1,
        VendorPage::BuyBack => 2,
    }
}

impl VendorWindow {
    /// Finds the window's parts below `root` (§7.1) and applies the P3 rules: the repair
    /// tab and page, `Vendor_FindItemsButton` and `Vendor_BuyTryOnButton` hidden; the sort
    /// and filter menus ghosted; the buy-back tab hidden until a `BuybackShown`; the buy
    /// page shown.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        templates: VendorTemplates,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        for id in [ids.repair_tab, ids.repair_page, ids.find_items, ids.try_on] {
            show(ui, f(ui, id), false);
        }
        for id in [ids.buy_sort, ids.buy_filter, ids.sell_filter] {
            ghost(ui, f(ui, id), true);
        }
        let list_in = |ui: &Ui, field: u32| {
            f(ui, field).and_then(|fl| {
                find(ui, fl, ids.vendor_list).or_else(|| {
                    super::super::subtree(ui, fl)
                        .into_iter()
                        .find(|k| ui.class(*k).is_some_and(WidgetClass::is_list))
                })
            })
        };
        let lists = [
            PageList {
                page: f(ui, ids.buy_page),
                tab: f(ui, ids.buy_tab),
                list: list_in(ui, ids.buy_list_field),
                ..PageList::default()
            },
            PageList {
                page: f(ui, ids.sell_page),
                tab: f(ui, ids.sell_tab),
                list: list_in(ui, ids.sell_list_field),
                ..PageList::default()
            },
            PageList {
                page: f(ui, ids.buyback_page),
                tab: f(ui, ids.buyback_tab),
                list: list_in(ui, ids.buyback_list_field),
                ..PageList::default()
            },
        ];
        let money = |ui: &mut Ui, id: u32| {
            f(ui, id)
                .and_then(|k| MoneyDisplay::find(ui, nv, k))
                .map(|m| MoneyDisplay::attach(ui, nv, m, art))
        };
        let player_money = money(ui, ids.player_money);
        let sell_value = money(ui, ids.sell_value);
        let mut w = Self {
            window: 0,
            root,
            page: VendorPage::Buy,
            buy: Vec::new(),
            sell: Vec::new(),
            buyback: None,
            toggles: BuyToggles::default(),
            wallet: 0,
            player_money,
            sell_value,
            lists,
            close: f(ui, ids.vendor_close),
            buy_button: f(ui, ids.buy_button),
            sell_button: f(ui, ids.sell_button),
            sell_all: f(ui, ids.sell_all_button),
            buyback_button: f(ui, ids.buyback_button),
            stack_hint: f(ui, ids.stack_description),
            templates,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        };
        for k in [
            w.close,
            w.buy_button,
            w.sell_button,
            w.sell_all,
            w.buyback_button,
        ] {
            w.listen.clicks(ui, k);
        }
        for l in &w.lists {
            w.listen.clicks(ui, l.tab);
        }
        show(ui, w.lists[2].tab, false);
        w.show_page(ui, VendorPage::Buy);
        w
    }

    /// Shows a `VendorShown` (§7.2): one buy entry per feed entry, in feed order, two per
    /// row; the buy page opens.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        window: WindowId,
        entries: &[VendorItem],
        texts: &dyn NpcTexts,
    ) {
        self.window = window;
        self.buy = entries.iter().take(MAX_ENTRIES).cloned().collect();
        let items = self.buy.clone();
        self.fill_items(ui, 0, &items, texts);
        self.update_buy_button(ui);
        self.show_page(ui, VendorPage::Buy);
    }

    /// Shows a `BuybackShown` (§7.8): the buy-back list, and the buy-back tab from now on.
    pub fn set_buyback(&mut self, ui: &mut Ui, entries: &[VendorItem], texts: &dyn NpcTexts) {
        let items: Vec<VendorItem> = entries.iter().take(MAX_ENTRIES).cloned().collect();
        self.fill_items(ui, 2, &items, texts);
        self.buyback = Some(items);
        show(ui, self.lists[2].tab, true);
    }

    /// Sets the sell list (§7.6 Tessera rule 1: the sellable backpack items in slot order,
    /// from the inventory model).
    pub fn set_sell_items(&mut self, ui: &mut Ui, items: &[SellItem], texts: &dyn NpcTexts) {
        self.sell = items.iter().take(MAX_ENTRIES).cloned().collect();
        let l = &mut self.lists[1];
        let list = l.list;
        let mut rows = std::mem::take(&mut l.rows);
        if let Some(list) = list {
            super::kit::clear_rows(ui, list, &mut rows);
        }
        l.money.clear();
        l.overlays.clear();
        l.selected = None;
        let Some(list) = list else { return };
        let sells = self.sell.clone();
        for s in &sells {
            let Some(k) = self.make_entry(ui, list, self.templates.sell_entry, true) else {
                break;
            };
            let m = self.fill_entry(
                ui,
                k,
                &s.name,
                s.colour,
                s.icon.as_ref(),
                (s.quantity > 1).then_some(s.quantity),
                texts,
            );
            let mut m = m;
            if let Some(d) = m.as_mut() {
                // Tessera rule (§7.6 OPEN): a worth of 0 (an item without sale data, a
                // literal item) shows the copper coin with 0, as §9.2 shows an amount of 0
                // with `ForceShowAll` and `SuppressLeadingZeroes`; any other worth uses the
                // template's own options.
                if s.worth.is_zero() {
                    d.options.force_show_all = true;
                    d.options.suppress_leading_zeroes = true;
                }
                d.set(ui, s.worth, false, texts);
            }
            let l = &mut self.lists[1];
            l.rows.push(k);
            l.money.push(m);
            l.overlays.push(None);
            self.listen.clicks(ui, Some(k));
        }
        self.commit(ui, 1);
        self.update_sell_value(ui, texts);
    }

    /// The player's money (`WalletChanged { Coin }`): the `Vendor_PlayerMoney` display and
    /// the `TooExpensive` state of every buy and buy-back entry whose coin price exceeds
    /// it (§7.2).
    pub fn set_wallet(&mut self, ui: &mut Ui, coin: u64, coins: CoinCounts, texts: &dyn NpcTexts) {
        self.wallet = coin;
        if let Some(d) = self.player_money.as_mut() {
            d.set(ui, coins, false, texts);
        }
        self.update_too_expensive(ui);
    }

    fn update_too_expensive(&mut self, ui: &mut Ui) {
        let nv = self.nv.clone();
        for (li, items) in [(0usize, Some(&self.buy)), (2, self.buyback.as_ref())] {
            let Some(items) = items else { continue };
            for (i, it) in items.iter().enumerate() {
                if let Some(Some(d)) = self.lists[li].money.get(i) {
                    set_too_expensive(ui, &nv, d, it.price > self.wallet);
                }
            }
        }
    }

    /// The held toggles changed (§7.4): refreshes the stack overlays of the buy entries
    /// and the stack description label.
    pub fn set_toggles(&mut self, ui: &mut Ui, toggles: BuyToggles, texts: &dyn NpcTexts) {
        self.toggles = toggles;
        let mode = toggles.mode();
        let st = self.nv.states;
        for (i, it) in self.buy.iter().enumerate() {
            let Some(Some(o)) = self.lists[0].overlays.get(i).copied() else {
                continue;
            };
            let state = if it.stack_size >= 2 {
                match mode {
                    BuyMode::Stack => Some(st.overlay_stack),
                    BuyMode::Quantity => Some(st.overlay_quantity),
                    BuyMode::Fill => Some(st.overlay_fill),
                    BuyMode::Single => None,
                }
            } else {
                None
            };
            ui.switch_state_if_defined(o, state.unwrap_or(st.overlay_off));
            ui.set_shown(o, state.is_some());
        }
        if self.stack_hint.is_some() {
            let t = label_text(texts, NpcLabel::StackHint(mode));
            put_text(ui, self.stack_hint, &t);
        }
    }

    /// Shows one page and its tab as selected; the others are hidden (tabs switch pages
    /// locally, §7.8).
    pub fn show_page(&mut self, ui: &mut Ui, page: VendorPage) {
        if page == VendorPage::BuyBack && self.buyback.is_none() {
            return;
        }
        self.page = page;
        let st = ui.vocab().states;
        for (i, l) in self.lists.iter().enumerate() {
            let on = i == index(page);
            show(ui, l.page, on);
            if let Some(t) = l.tab {
                ui.switch_state_if_defined(t, if on { st.highlight } else { st.normal });
            }
        }
    }

    fn make_entry(
        &self,
        ui: &mut Ui,
        list: ElementKey,
        template: Option<ExamTemplate>,
        sell: bool,
    ) -> Option<ElementKey> {
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = &self.art;
        let v = ui.vocab().clone();
        let st = v.states;
        let nst = self.nv.states;
        let nv = &self.nv;
        make_row(ui, list, template, || {
            let id = if sell {
                ids.vendor_sell_item
            } else {
                ids.vendor_item
            };
            let mut n = FNode::field(id, 0, 0, ENTRY_SIZE.0, ENTRY_SIZE.1)
                .typed(if sell {
                    nv.types.vendor_sell_item_display
                } else {
                    nv.types.vendor_item_display
                })
                .image(&art.row)
                .state(st.normal, art.row.as_ref(), vec![])
                .state(st.highlight, art.row_highlight.as_ref(), vec![])
                .child(
                    FNode::field(ids.vendor_item_icon, 8, 7, 32, 32)
                        .image(&art.slot)
                        .child(FNode::field(ids.item_icon, 0, 0, 32, 32))
                        .child(
                            FNode::text(ids.item_quantity, 0, 18, 30, 14, cp, colours::WHITE, "")
                                .hidden(),
                        ),
                )
                .child(FNode::text(
                    ids.item_name,
                    46,
                    8,
                    131,
                    30,
                    cp,
                    colours::WHITE,
                    "",
                ))
                .child(fallback_display(
                    &v,
                    nv,
                    ids.vendor_item_money,
                    17,
                    41,
                    MoneyOptions::LEFT_JUSTIFIED,
                ));
            if !sell {
                n = n.child(
                    FNode::field(ids.stack_overlay, 154, 23, 20, 20)
                        .state(nst.overlay_stack, art.mode_overlay.as_ref(), vec![])
                        .state(nst.overlay_quantity, art.mode_overlay.as_ref(), vec![])
                        .state(nst.overlay_fill, art.mode_overlay.as_ref(), vec![])
                        .state(nst.overlay_off, None, vec![])
                        .hidden(),
                );
            }
            n
        })
    }

    /// Fills an entry's icon, name (with its quality colour) and quantity overlay (shown
    /// with `quantity` when given); returns its money display.
    #[allow(clippy::too_many_arguments)]
    fn fill_entry(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        name: &str,
        colour: Option<u32>,
        icon: Option<&IconImage>,
        quantity: Option<i64>,
        texts: &dyn NpcTexts,
    ) -> Option<MoneyDisplay> {
        let ids = self.nv.ids;
        let name_el = text_part(ui, k, Some(ids.item_name));
        put_text(ui, name_el, name);
        if let (Some(n), Some(c)) = (name_el, colour) {
            ui.set_property(n, self.nv.pids.text_font_color, colour_value(c));
        }
        let holder = find(ui, k, ids.vendor_item_icon);
        let icon_el = holder
            .and_then(|h| find(ui, h, ids.item_icon))
            .or(holder)
            .or_else(|| icon_part(ui, k, Some(ids.item_icon)));
        if let Some(ic) = icon_el {
            set_icon(ui, ic, icon);
            ui.set_examine(ic, icon.is_some());
        }
        if let Some(q) = find(ui, k, ids.item_quantity) {
            match quantity {
                Some(n) => {
                    ui.set_text(q, Some(texts.amount(n)));
                    ui.set_shown(q, true);
                }
                None => ui.set_shown(q, false),
            }
        }
        let m =
            find(ui, k, ids.vendor_item_money).and_then(|m| MoneyDisplay::find(ui, &self.nv, m));
        m.map(|m| MoneyDisplay::attach(ui, &self.nv, m, &self.art))
    }

    fn fill_items(&mut self, ui: &mut Ui, li: usize, items: &[VendorItem], texts: &dyn NpcTexts) {
        let list = self.lists[li].list;
        let mut rows = std::mem::take(&mut self.lists[li].rows);
        if let Some(list) = list {
            super::kit::clear_rows(ui, list, &mut rows);
        }
        self.lists[li].money.clear();
        self.lists[li].overlays.clear();
        self.lists[li].selected = None;
        let Some(list) = list else { return };
        for it in items {
            let Some(k) = self.make_entry(ui, list, self.templates.entry, false) else {
                break;
            };
            // §7.2 Tessera rule: the stock, when present, in the icon's quantity overlay;
            // else the entry's quantity above 1.
            let q = it
                .stock
                .map(i64::from)
                .or_else(|| (it.quantity > 1).then_some(i64::from(it.quantity)));
            let mut m = self.fill_entry(ui, k, &it.name, it.colour, it.icon.as_ref(), q, texts);
            if let Some(d) = m.as_mut() {
                d.set(ui, it.coins, false, texts);
                set_too_expensive(ui, &self.nv, d, it.price > self.wallet);
            }
            let overlay = find(ui, k, self.nv.ids.stack_overlay);
            if let Some(o) = overlay {
                ui.set_shown(o, false);
            }
            let l = &mut self.lists[li];
            l.rows.push(k);
            l.money.push(m);
            l.overlays.push(overlay);
            self.listen.clicks(ui, Some(k));
        }
        self.commit(ui, li);
        if li == 0 {
            let t = self.toggles;
            self.set_toggles(ui, t, texts);
        }
    }

    /// Makes page `li`'s rows entries of its list (one layout).
    fn commit(&mut self, ui: &mut Ui, li: usize) {
        let Some(list) = self.lists[li].list else {
            return;
        };
        let rows = self.lists[li].rows.clone();
        let kept = commit_rows(ui, list, &rows);
        if kept.len() != rows.len() {
            let l = &mut self.lists[li];
            let keep: Vec<bool> = l.rows.iter().map(|r| kept.contains(r)).collect();
            let mut it = keep.iter();
            l.money.retain(|_| *it.next().unwrap_or(&false));
            let mut it = keep.iter();
            l.overlays.retain(|_| *it.next().unwrap_or(&false));
            l.rows.retain(|r| kept.contains(r));
        }
    }

    /// Ghosts the Buy button while the selected entry's stock is 0 (§7.2 Tessera rule).
    fn update_buy_button(&mut self, ui: &mut Ui) {
        let out = self.lists[0]
            .selected
            .and_then(|i| self.buy.get(i))
            .is_some_and(|it| it.stock == Some(0));
        ghost(ui, self.buy_button, out);
    }

    fn update_sell_value(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        let worth = self.lists[1]
            .selected
            .and_then(|i| self.sell.get(i))
            .map(|s| s.worth)
            .unwrap_or_default();
        if let Some(d) = self.sell_value.as_mut() {
            d.set(ui, worth, false, texts);
        }
    }

    /// Selects entry `i` of `page` (as `SelectLeft` does).
    pub fn select(
        &mut self,
        ui: &mut Ui,
        page: VendorPage,
        i: Option<usize>,
        texts: &dyn NpcTexts,
    ) {
        self.lists[index(page)].select(ui, i);
        match page {
            VendorPage::Buy => self.update_buy_button(ui),
            VendorPage::Sell => self.update_sell_value(ui, texts),
            VendorPage::BuyBack => {}
        }
    }

    /// The selected entry of `page`.
    #[must_use]
    pub fn selected(&self, page: VendorPage) -> Option<usize> {
        self.lists[index(page)].selected
    }

    /// The entry rows of `page`.
    #[must_use]
    pub fn rows(&self, page: VendorPage) -> &[ElementKey] {
        &self.lists[index(page)].rows
    }

    /// The money display of entry `i` of `page`.
    #[must_use]
    pub fn entry_money(&self, page: VendorPage, i: usize) -> Option<&MoneyDisplay> {
        self.lists[index(page)]
            .money
            .get(i)
            .and_then(Option::as_ref)
    }

    /// Buys the selected buy entry in the current mode (§7.4).
    fn buy_selected(&self) -> Option<VendorOutcome> {
        let i = self.lists[0].selected?;
        let it = self.buy.get(i)?;
        if it.stock == Some(0) {
            return None;
        }
        Some(
            match buy_decision(self.toggles.mode(), it.stack_size, it.fill) {
                BuyDecision::Buy(q) => VendorOutcome::Send(vec![NpcAction::Buy {
                    window: self.window,
                    entry: it.entry,
                    quantity: q,
                }]),
                BuyDecision::Dialog(max) => VendorOutcome::AskQuantity {
                    entry: it.entry,
                    max,
                },
            },
        )
    }

    fn sell_action(&self, s: &SellItem) -> NpcAction {
        NpcAction::Sell {
            window: self.window,
            item: s.item,
            quantity: u32::try_from(s.quantity.max(1)).unwrap_or(u32::MAX),
        }
    }

    /// Selling an item dropped on the window (§7.6 Tessera rule 4): `Sell` with the
    /// item's quantity. The client gets the drop from its item slots (the window is a drop
    /// area, [`super::super::items::ItemSlots::attach_area`]).
    #[must_use]
    pub fn sell_dropped(&self, item: u64, quantity: i64) -> NpcAction {
        NpcAction::Sell {
            window: self.window,
            item,
            quantity: u32::try_from(quantity.max(1)).unwrap_or(u32::MAX),
        }
    }

    /// The commands for a UI event (§7.4, §7.6, §7.8 command table).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn NpcTexts,
    ) -> Option<VendorOutcome> {
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        let press = matches!(kind, ClickKind::Button | ClickKind::Left);
        if press {
            if hit(self.close) {
                return Some(VendorOutcome::Send(vec![self.close_action()]));
            }
            for p in [VendorPage::Buy, VendorPage::Sell, VendorPage::BuyBack] {
                if hit(self.lists[index(p)].tab) {
                    self.show_page(ui, p);
                    return None;
                }
            }
            if hit(self.buy_button) {
                return self.buy_selected();
            }
            if hit(self.sell_button) {
                let i = self.lists[1].selected?;
                let s = self.sell.get(i)?;
                return Some(VendorOutcome::Send(vec![self.sell_action(s)]));
            }
            if hit(self.sell_all) {
                let v: Vec<NpcAction> = self.sell.iter().map(|s| self.sell_action(s)).collect();
                return (!v.is_empty()).then_some(VendorOutcome::Send(v));
            }
            if hit(self.buyback_button) {
                let i = self.lists[2].selected?;
                let e = self.buyback.as_ref()?.get(i)?.entry;
                return Some(VendorOutcome::Send(vec![NpcAction::BuyBack {
                    window: self.window,
                    entry: e,
                }]));
            }
        }
        for p in [VendorPage::Buy, VendorPage::Sell, VendorPage::BuyBack] {
            let li = index(p);
            let Some(i) = row_of(ui, &self.lists[li].rows, src) else {
                continue;
            };
            match kind {
                ClickKind::Left | ClickKind::Button => {
                    self.select(ui, p, Some(i), texts);
                    return None;
                }
                // A right click selects and acts; on the buy page a left double-click buys
                // the selected entry (§7.4), on the sell and buy-back pages it selects the
                // entry and sells or buys it back (§7.6, §7.8).
                ClickKind::Right | ClickKind::Double => {
                    if kind == ClickKind::Right
                        || p != VendorPage::Buy
                        || self.lists[li].selected.is_none()
                    {
                        self.select(ui, p, Some(i), texts);
                    }
                    return match p {
                        VendorPage::Buy => self.buy_selected(),
                        VendorPage::Sell => {
                            let s = self.sell.get(self.lists[1].selected?)?;
                            Some(VendorOutcome::Send(vec![self.sell_action(s)]))
                        }
                        VendorPage::BuyBack => {
                            let e = self.buyback.as_ref()?.get(self.lists[2].selected?)?.entry;
                            Some(VendorOutcome::Send(vec![NpcAction::BuyBack {
                                window: self.window,
                                entry: e,
                            }]))
                        }
                    };
                }
            }
        }
        None
    }

    /// `CloseWindow` for this window (`Vendor_CloseButton`, `EscapeKey`).
    #[must_use]
    pub fn close_action(&self) -> NpcAction {
        NpcAction::CloseWindow {
            window: self.window,
        }
    }

    /// The buy entry with feed entry number `entry`.
    #[must_use]
    pub fn entry(&self, entry: u16) -> Option<&VendorItem> {
        self.buy.iter().find(|e| e.entry == entry)
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
