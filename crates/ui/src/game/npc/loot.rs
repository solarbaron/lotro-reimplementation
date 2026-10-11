// SPDX-License-Identifier: GPL-3.0-or-later
//! The loot window (`npc-windows.md` §8): one row per untaken entry with the coin row
//! first (§8.2), the background state by source, Loot All with its pending close and the
//! button's label (§8.3), taking single entries and closing.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npcvocab::NpcVocab;
use super::super::{IconImage, set_icon, show};
use super::coins::{CoinCounts, MoneyDisplay, MoneyOptions, fallback_display};
use super::kit::{
    ClickKind, FNode, FallbackArt, Listen, click_of, colour_value, colours, commit_rows, find,
    icon_part, make_row, put_text, row_of, set_one_line, text_part,
};
use super::{NpcAction, NpcLabel, NpcTexts, WindowId, label_text};

/// Most entries a loot window lists (Tessera bound; the feed sends at most 1,024).
pub const MAX_ENTRIES: usize = 1024;

/// The loot model of §8.3: which entries are taken, whether Loot All was sent and is
/// pending, and the close that follows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LootModel {
    /// The entries (entry number, taken).
    pub entries: Vec<(u16, bool)>,
    /// The coin amount (`coin` of `LootShown`).
    pub coin: u64,
    /// A Loot All was sent.
    pub loot_all_sent: bool,
    /// The close after Loot All was sent.
    pub close_sent: bool,
}

impl LootModel {
    /// The model of a `LootShown` (at most [`MAX_ENTRIES`] entries).
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = u16>, coin: u64) -> Self {
        Self {
            entries: entries
                .into_iter()
                .take(MAX_ENTRIES)
                .map(|e| (e, false))
                .collect(),
            coin,
            loot_all_sent: false,
            close_sent: false,
        }
    }

    /// Whether an untaken entry is listed (the coin row is not counted, §8.3).
    #[must_use]
    pub fn has_untaken(&self) -> bool {
        self.entries.iter().any(|(_, t)| !t)
    }

    /// Loot All (button or Enter, §8.3 Tessera rule): `TakeLoot { entry: None }` while an
    /// untaken entry exists or the coin is above 0 and no Loot All was sent yet; otherwise
    /// `CloseWindow`.
    pub fn loot_all(&mut self, window: WindowId) -> NpcAction {
        if (self.has_untaken() || self.coin > 0) && !self.loot_all_sent {
            self.loot_all_sent = true;
            NpcAction::TakeLoot {
                window,
                entry: None,
            }
        } else {
            NpcAction::CloseWindow { window }
        }
    }

    /// Taking one entry (right click or double-click on its row): `TakeLoot { entry }` for
    /// an untaken entry.
    #[must_use]
    pub fn take(&self, window: WindowId, entry: u16) -> Option<NpcAction> {
        self.entries
            .iter()
            .any(|(e, t)| *e == entry && !t)
            .then_some(NpcAction::TakeLoot {
                window,
                entry: Some(entry),
            })
    }

    /// A `LootTaken`: marks the entry taken. Returns whether it was listed and untaken.
    pub fn taken(&mut self, entry: u16) -> bool {
        match self.entries.iter_mut().find(|(e, t)| *e == entry && !*t) {
            Some(x) => {
                x.1 = true;
                true
            }
            None => false,
        }
    }

    /// The end of a frame (§8.3): after a Loot All was sent, `CloseWindow` once in the
    /// first frame in which every entry has been taken.
    pub fn end_frame(&mut self, window: WindowId) -> Option<NpcAction> {
        if self.loot_all_sent && !self.close_sent && !self.has_untaken() {
            self.close_sent = true;
            return Some(NpcAction::CloseWindow { window });
        }
        None
    }
}

/// One loot entry, rendered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LootItem {
    /// The feed's entry number.
    pub entry: u16,
    /// The item's name (or the literal).
    pub name: String,
    /// The name's quality colour.
    pub colour: Option<u32>,
    /// The composed icon.
    pub icon: Option<IconImage>,
    /// The quantity (overlay above 1).
    pub quantity: i64,
    /// The client's reference for the examination tooltip.
    pub item: Option<u64>,
}

/// A `LootShown`, rendered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LootView {
    /// The feed window.
    pub window: WindowId,
    /// The source is an entity of kind creature, non-player or player
    /// (`container_corpse`); otherwise (an object or no source) `container_chest`.
    pub corpse: bool,
    /// The coin amount.
    pub coin: u64,
    /// The coin amount in coins (§9.3).
    pub coins: CoinCounts,
    /// The entries, in entry order.
    pub entries: Vec<LootItem>,
}

/// The loot window's row templates: `ItemDisplay_MediumTemplate` and
/// `MoneyDisplay_MoneyTemplate` (§8.2). `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LootTemplates {
    /// An entry row.
    pub entry: Option<ExamTemplate>,
    /// The coin row.
    pub coin: Option<ExamTemplate>,
}

/// Size of a fallback entry row (a medium item display: icon 32 × 32 and name 165 × 32).
pub const ROW_SIZE: (i32, i32) = (230, 36);

/// A loot window instance.
#[derive(Debug, Clone)]
pub struct LootWindow {
    /// The feed window.
    pub window: WindowId,
    /// The root (`ExternalContainerField`).
    pub root: ElementKey,
    /// The model.
    pub model: LootModel,
    /// The list (`ContainerListBox`).
    pub list: Option<ElementKey>,
    /// The entry rows with their entry numbers.
    pub rows: Vec<(ElementKey, u16)>,
    /// The coin row and its display.
    pub coin_row: Option<(ElementKey, Option<MoneyDisplay>)>,
    /// `ExternalContainerLootAllButton`.
    pub button: Option<ElementKey>,
    close: Option<ElementKey>,
    background: Option<ElementKey>,
    /// The button's own run-time text when attached (the fallback's; `None` for the
    /// layout's text entry).
    button_text: Option<String>,
    templates: LootTemplates,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

impl LootWindow {
    /// Finds the window's parts below `root` (§8.1); the timer is hidden (§8.2).
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        templates: LootTemplates,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        show(ui, f(ui, ids.loot_timer), false);
        let button = f(ui, ids.loot_all);
        let button_text = button
            .map(|b| text_part(ui, b, None).unwrap_or(b))
            .and_then(|el| ui.el(el).and_then(|e| e.text.runtime.clone()));
        let mut w = Self {
            window: 0,
            root,
            model: LootModel::default(),
            list: f(ui, ids.loot_list),
            rows: Vec::new(),
            coin_row: None,
            button,
            close: f(ui, ids.loot_close),
            background: f(ui, ids.loot_background),
            button_text,
            templates,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        };
        for k in [w.button, w.close] {
            w.listen.clicks(ui, k);
        }
        w
    }

    /// Shows a `LootShown` (§8.2): the coin row first when the coin is above 0, then one
    /// row per entry in entry order; the background state by source; the button's label.
    /// With `auto_loot_all` (the source was used with `AutoLootAll`, §3.1) Loot All is
    /// sent at once and returned.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        view: &LootView,
        auto_loot_all: bool,
        texts: &dyn NpcTexts,
    ) -> Option<NpcAction> {
        self.window = view.window;
        self.model = LootModel::new(view.entries.iter().map(|e| e.entry), view.coin);
        if let Some(b) = self.background {
            let s = if view.corpse {
                self.nv.states.container_corpse
            } else {
                self.nv.states.container_chest
            };
            ui.switch_state_if_defined(b, s);
        }
        self.fill(ui, view, texts);
        self.relabel(ui, texts);
        auto_loot_all.then(|| self.model.loot_all(self.window))
    }

    fn fill(&mut self, ui: &mut Ui, view: &LootView, texts: &dyn NpcTexts) {
        let Some(list) = self.list else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        old.extend(self.coin_row.take().map(|(k, _)| k));
        super::kit::clear_rows(ui, list, &mut old);
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = self.art.clone();
        let v = ui.vocab().clone();
        let nv = self.nv.clone();
        if view.coin > 0 {
            let row = make_row(ui, list, self.templates.coin, || {
                FNode::field(ids.money_template, 0, 0, ROW_SIZE.0, 26).child(fallback_display(
                    &v,
                    &nv,
                    0,
                    40,
                    2,
                    MoneyOptions::LEFT_JUSTIFIED,
                ))
            });
            if let Some(r) = row {
                let mut d = MoneyDisplay::find(ui, &self.nv, r)
                    .map(|m| MoneyDisplay::attach(ui, &self.nv, m, &self.art));
                if let Some(d) = d.as_mut() {
                    d.set(ui, view.coins, false, texts);
                }
                self.coin_row = Some((r, d));
            }
        }
        for it in view.entries.iter().take(MAX_ENTRIES) {
            let row = make_row(ui, list, self.templates.entry, || {
                FNode::field(ids.medium_item, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
                    .image(&art.row)
                    .child(
                        FNode::field(0, 2, 2, 32, 32)
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
                        9,
                        165,
                        18,
                        cp,
                        colours::WHITE,
                        "",
                    ))
            });
            let Some(r) = row else { break };
            let name = text_part(ui, r, Some(ids.item_name));
            // Tessera rule (§8.2 OPEN): the name is one line, cut with the element's
            // truncation suffix and tooltip (`fonts.md` §7.6) when longer.
            set_one_line(ui, &self.nv, name, true);
            put_text(ui, name, &it.name);
            if let (Some(n), Some(c)) = (name, it.colour) {
                ui.set_property(n, cp, colour_value(c));
            }
            if let Some(ic) = icon_part(ui, r, Some(ids.item_icon)) {
                set_icon(ui, ic, it.icon.as_ref());
                ui.set_examine(ic, it.item.is_some());
            }
            if let Some(q) = find(ui, r, ids.item_quantity) {
                if it.quantity > 1 {
                    ui.set_text(q, Some(texts.amount(it.quantity)));
                    ui.set_shown(q, true);
                } else {
                    ui.set_shown(q, false);
                }
            }
            self.listen.clicks(ui, Some(r));
            self.rows.push((r, it.entry));
        }
        let all: Vec<ElementKey> = self
            .coin_row
            .as_ref()
            .map(|(k, _)| *k)
            .into_iter()
            .chain(self.rows.iter().map(|(k, _)| *k))
            .collect();
        let kept = commit_rows(ui, list, &all);
        self.rows.retain(|(k, _)| kept.contains(k));
        if self
            .coin_row
            .as_ref()
            .is_some_and(|(k, _)| !kept.contains(k))
        {
            self.coin_row = None;
        }
    }

    /// The button's label (§8.3 rule 2, the coin row not counted): with an untaken entry
    /// the client's Loot All text, else the button's own text from when the window was
    /// attached (the layout's text entry); with none the empty-list text.
    fn relabel(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        let Some(b) = self.button else { return };
        let label = if self.model.has_untaken() {
            texts
                .label(NpcLabel::LootAll)
                .or_else(|| self.button_text.clone())
        } else {
            Some(label_text(texts, NpcLabel::LootAllEmpty))
        };
        let el = text_part(ui, b, None).unwrap_or(b);
        ui.set_text(el, label);
    }

    /// A `LootTaken`: the entry's row goes and the button is relabelled.
    pub fn taken(&mut self, ui: &mut Ui, entry: u16, texts: &dyn NpcTexts) {
        if !self.model.taken(entry) {
            return;
        }
        if let Some(i) = self.rows.iter().position(|(_, e)| *e == entry) {
            let (k, _) = self.rows.remove(i);
            match self.list {
                Some(l) if ui.class(l).is_some_and(crate::WidgetClass::is_list) => {
                    ui.list_remove(l, k);
                }
                _ => ui.destroy(k),
            }
        }
        self.relabel(ui, texts);
    }

    /// The end of a frame: the close after a Loot All (§8.3 Tessera rule).
    pub fn end_frame(&mut self) -> Option<NpcAction> {
        self.model.end_frame(self.window)
    }

    /// Loot All (`ExternalContainerLootAllButton`, or `AcceptInput` while the window is
    /// active).
    pub fn loot_all(&mut self) -> NpcAction {
        self.model.loot_all(self.window)
    }

    /// The command for a UI event (§8.3 Tessera rule (commands)).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<NpcAction> {
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if matches!(kind, ClickKind::Button | ClickKind::Left) {
            if hit(self.button) {
                return Some(self.loot_all());
            }
            if hit(self.close) {
                return Some(self.close_action());
            }
        }
        if matches!(kind, ClickKind::Right | ClickKind::Double) {
            let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
            let i = row_of(ui, &ks, src)?;
            return self.model.take(self.window, self.rows[i].1);
        }
        None
    }

    /// `CloseWindow` for this window (close button, `EscapeKey`).
    #[must_use]
    pub fn close_action(&self) -> NpcAction {
        NpcAction::CloseWindow {
            window: self.window,
        }
    }

    /// Dragging an entry row into a bag (§8.3): `TakeLoot { entry }` for the row at or
    /// above `source` (the drop's target slot is not sent).
    #[must_use]
    pub fn dragged(&self, ui: &Ui, source: ElementKey) -> Option<NpcAction> {
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let i = row_of(ui, &ks, source)?;
        self.model.take(self.window, self.rows[i].1)
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
