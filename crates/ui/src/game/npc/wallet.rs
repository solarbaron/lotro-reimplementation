// SPDX-License-Identifier: GPL-3.0-or-later
//! The wallet window (`npc-windows.md` §9.4 Tessera rule (fill)): the money row first,
//! then one category row per category that has a listed entry (ascending category key),
//! its entries sorted by name without case, then by DID; the search filter; expand and
//! collapse.

use std::collections::BTreeSet;

use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npcvocab::NpcVocab;
use super::super::{set_image_ref, show};
use super::coins::{CoinCounts, MoneyDisplay, MoneyOptions, fallback_display};
use super::kit::{
    ClickKind, FNode, FallbackArt, Listen, TreeRow, click_of, colour_value, colours,
    commit_tree_rows, cut, find, make_row, put_text, row_of, text_part,
};
use super::{NpcLabel, NpcTexts, label_text};

/// Most entries the wallet lists (Tessera bound; the install has 610 wallet items).
pub const MAX_ENTRIES: usize = 4096;

/// Most characters of the search entry (`PaperItemPanel_SearchEntry`: at most 80).
pub const MAX_SEARCH_CHARS: usize = 80;

/// One wallet entry, rendered (§9.4 rule 3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalletEntry {
    /// The item's DID (0 for a literal ref).
    pub did: u32,
    /// The item's name (or the literal).
    pub name: String,
    /// `PaperItem_Category` (a literal goes under `Misc`, chosen by the client).
    pub category: u32,
    /// The category's display text (its label when it has none).
    pub category_title: String,
    /// `PaperItem_SmallIcon`, else the composed icon (scaled to 16 × 16 by the client).
    pub icon: Option<ImageRef>,
    /// The last `WalletChanged` amount.
    pub amount: i64,
    /// `PaperItemControl_CapDefault`.
    pub cap: Option<i64>,
}

/// One row of the wallet list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletRow {
    /// The money row.
    Money,
    /// A category row (its key).
    Category(u32),
    /// An entry (index into the entries given).
    Entry(usize),
}

/// Lower-cases `s` by the simple lower-case mapping of each character (a character whose
/// lower case is several characters is kept), as `quest-journal.md` §4.3 Tessera rule.
#[must_use]
pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        })
        .collect()
}

/// The rows of §9.4 rules 1–3 and 5: the money row; then, in ascending category key, one
/// category row per category with a listed entry (amount above 0, name containing the
/// search text without case), followed by its entries sorted by name without case, then
/// by DID, unless the category is collapsed.
#[must_use]
pub fn wallet_rows(
    entries: &[WalletEntry],
    search: &str,
    collapsed: &BTreeSet<u32>,
) -> Vec<WalletRow> {
    rows_with(entries, search, collapsed, false)
}

/// [`wallet_rows`]; with `all`, the entries of collapsed categories too (for a Tree, which
/// hides them itself).
fn rows_with(
    entries: &[WalletEntry],
    search: &str,
    collapsed: &BTreeSet<u32>,
    all: bool,
) -> Vec<WalletRow> {
    let needle = fold(search);
    let mut listed: Vec<(u32, String, u32, usize)> = entries
        .iter()
        .enumerate()
        .take(MAX_ENTRIES)
        .filter(|(_, e)| e.amount > 0 && (needle.is_empty() || fold(&e.name).contains(&needle)))
        .map(|(i, e)| (e.category, fold(&e.name), e.did, i))
        .collect();
    listed.sort();
    let mut rows = vec![WalletRow::Money];
    let mut last: Option<u32> = None;
    for (cat, _, _, i) in listed {
        if last != Some(cat) {
            rows.push(WalletRow::Category(cat));
            last = Some(cat);
        }
        if all || !collapsed.contains(&cat) {
            rows.push(WalletRow::Entry(i));
        }
    }
    rows
}

/// The wallet's row templates (`PaperItemPanel_MoneyEntry`, `PaperItem_TreeCategory`,
/// `PaperItem_EntryTemplate`; top level of layout `paperitem`). `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WalletTemplates {
    /// The money row.
    pub money: Option<ExamTemplate>,
    /// A category row.
    pub category: Option<ExamTemplate>,
    /// An entry row.
    pub entry: Option<ExamTemplate>,
}

/// Width of a fallback wallet row.
pub const ROW_WIDTH: i32 = 420;

/// The wallet window.
#[derive(Debug, Clone)]
pub struct WalletWindow {
    /// The root (`PaperItemPanel_Window_V2`).
    pub root: ElementKey,
    /// The list (`PaperItemListBox`).
    pub list: Option<ElementKey>,
    /// The entries.
    pub entries: Vec<WalletEntry>,
    /// The coin amount (the last `WalletChanged { Coin }`) in coins.
    pub coins: CoinCounts,
    /// The search text.
    pub search: String,
    /// The collapsed categories.
    pub collapsed: BTreeSet<u32>,
    /// The rows shown.
    pub rows: Vec<(ElementKey, WalletRow)>,
    /// The money row's display.
    pub money: Option<MoneyDisplay>,
    search_entry: Option<ElementKey>,
    buttons: (Option<ElementKey>, Option<ElementKey>),
    templates: WalletTemplates,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

impl WalletWindow {
    /// Finds the window's parts below `root`; the coin overflow row, the store elements and
    /// the upsell flyout are hidden (§9.4 rules 1 and 6).
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        templates: WalletTemplates,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        for id in [
            ids.wallet_overflow_entry,
            ids.wallet_upsell_button,
            ids.wallet_currency_upsell,
            ids.wallet_flyout,
            ids.wallet_perks_field,
        ] {
            show(ui, f(ui, id), false);
        }
        Self {
            root,
            list: f(ui, ids.wallet_list),
            entries: Vec::new(),
            coins: CoinCounts::default(),
            search: String::new(),
            collapsed: BTreeSet::new(),
            rows: Vec::new(),
            money: None,
            search_entry: f(ui, ids.wallet_search),
            buttons: (None, None),
            templates,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        }
    }

    /// The expand-all and collapse-all buttons (the fallback window's; the layout names
    /// them only by role).
    pub fn set_buttons(
        &mut self,
        ui: &mut Ui,
        expand: Option<ElementKey>,
        collapse: Option<ElementKey>,
    ) {
        self.buttons = (expand, collapse);
        self.listen.clicks(ui, expand);
        self.listen.clicks(ui, collapse);
    }

    /// Shows the wallet: the coin amount and the entries (§9.4 rules 1–4).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        coins: CoinCounts,
        entries: &[WalletEntry],
        texts: &dyn NpcTexts,
    ) {
        self.coins = coins;
        self.entries = entries.iter().take(MAX_ENTRIES).cloned().collect();
        self.rebuild(ui, texts);
    }

    /// Sets the search text (rule 5) and rebuilds.
    pub fn set_search(&mut self, ui: &mut Ui, text: &str, texts: &dyn NpcTexts) {
        let t: String = text.chars().take(MAX_SEARCH_CHARS).collect();
        if t != self.search {
            self.search = t;
            self.rebuild(ui, texts);
        }
    }

    /// Expands (or collapses) every category.
    pub fn expand_all(&mut self, ui: &mut Ui, expand: bool, texts: &dyn NpcTexts) {
        if expand {
            self.collapsed.clear();
        } else {
            self.collapsed = self.entries.iter().map(|e| e.category).collect();
        }
        self.rebuild(ui, texts);
    }

    /// The rows of the current state.
    #[must_use]
    pub fn row_kinds(&self) -> Vec<WalletRow> {
        wallet_rows(&self.entries, &self.search, &self.collapsed)
    }

    fn rebuild(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        let Some(list) = self.list else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        super::kit::clear_rows(ui, list, &mut old);
        self.money = None;
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = self.art.clone();
        let v = ui.vocab().clone();
        let nv = self.nv.clone();
        let st = self.nv.states;
        // A Tree (the install's `PaperItemListBox`) gets each entry as a child node of its
        // category row and collapses categories itself; a list box gets only the expanded
        // categories' entries.
        let is_tree = ui.class(list) == Some(crate::WidgetClass::Tree);
        let mut parents: Vec<Option<ElementKey>> = Vec::new();
        let mut category: Option<ElementKey> = None;
        for r in rows_with(&self.entries, &self.search, &self.collapsed, is_tree) {
            parents.push(match r {
                WalletRow::Entry(_) => category,
                _ => None,
            });
            let k = match r {
                WalletRow::Money => {
                    let label = label_text(texts, NpcLabel::Coins);
                    let k = make_row(ui, list, self.templates.money, || {
                        FNode::field(ids.wallet_money_entry, 0, 0, ROW_WIDTH, 26)
                            .image(&art.row)
                            .child(FNode::text(0, 8, 4, 120, 18, cp, colours::TITLE, &label))
                            .child(fallback_display(
                                &v,
                                &nv,
                                0,
                                ROW_WIDTH - 170,
                                2,
                                MoneyOptions {
                                    force_show_all: true,
                                    ..MoneyOptions::LEFT_JUSTIFIED
                                },
                            ))
                    });
                    if let Some(k) = k
                        && let Some(m) = MoneyDisplay::find(ui, &self.nv, k)
                    {
                        let mut d = MoneyDisplay::attach(ui, &self.nv, m, &self.art);
                        d.set(ui, self.coins, false, texts);
                        self.money = Some(d);
                    }
                    k
                }
                WalletRow::Category(cat) => {
                    let title = self
                        .entries
                        .iter()
                        .find(|e| e.category == cat)
                        .map(|e| e.category_title.clone())
                        .unwrap_or_default();
                    let k = make_row(ui, list, self.templates.category, || {
                        FNode::text(
                            ids.wallet_category,
                            0,
                            0,
                            ROW_WIDTH,
                            20,
                            cp,
                            colours::TITLE,
                            "",
                        )
                    });
                    if let Some(k) = k {
                        put_text(ui, text_part(ui, k, None).or(Some(k)), &cut(&title));
                        self.listen.clicks(ui, Some(k));
                    }
                    category = k;
                    k
                }
                WalletRow::Entry(i) => {
                    let e = self.entries[i].clone();
                    let k = make_row(ui, list, self.templates.entry, || {
                        FNode::field(ids.wallet_entry, 0, 0, ROW_WIDTH, 20)
                            .image(&art.row)
                            .child(FNode::text(
                                ids.wallet_entry_name,
                                23,
                                2,
                                240,
                                16,
                                cp,
                                colours::WHITE,
                                "",
                            ))
                            .child(
                                FNode::text(
                                    ids.wallet_entry_total,
                                    270,
                                    2,
                                    103,
                                    16,
                                    cp,
                                    colours::WHITE,
                                    "",
                                )
                                .state(
                                    st.item_capped,
                                    None,
                                    vec![(cp, colour_value(colours::CAPPED))],
                                ),
                            )
                            .child(FNode::field(ids.wallet_entry_icon, 380, 2, 16, 16))
                    });
                    if let Some(k) = k {
                        put_text(ui, find(ui, k, ids.wallet_entry_name), &e.name);
                        let total = find(ui, k, ids.wallet_entry_total);
                        put_text(ui, total, &texts.amount(e.amount));
                        if let Some(t) = total
                            && e.cap.is_some_and(|c| e.amount >= c)
                        {
                            ui.switch_state_if_defined(t, st.item_capped);
                        }
                        if let (Some(ic), Some(img)) = (find(ui, k, ids.wallet_entry_icon), &e.icon)
                        {
                            set_image_ref(ui, ic, img, super::kit::ART_MODE);
                        }
                    }
                    k
                }
            };
            match k {
                Some(k) => self.rows.push((k, r)),
                None => {
                    parents.pop();
                    break;
                }
            }
        }
        let tree_rows: Vec<TreeRow> = self
            .rows
            .iter()
            .zip(parents)
            .map(|((k, r), parent)| TreeRow {
                row: *k,
                parent,
                expanded: !matches!(r, WalletRow::Category(c) if self.collapsed.contains(c)),
            })
            .collect();
        let kept: std::collections::HashSet<ElementKey> =
            commit_tree_rows(ui, list, &tree_rows).into_iter().collect();
        self.rows.retain(|(k, _)| kept.contains(k));
    }

    /// A UI event: a click on a category row collapses or expands it (Tessera rule), the
    /// expand and collapse buttons act on all categories, and the search entry's text is
    /// read on every event.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent, texts: &dyn NpcTexts) {
        if let Some(s) = self.search_entry {
            let t = ui.text_of(s);
            if t != self.search {
                self.set_search(ui, &t, texts);
            }
        }
        let Some((src, kind)) = click_of(ui, ev) else {
            return;
        };
        if !matches!(kind, ClickKind::Button | ClickKind::Left) || !ui.exists(src) {
            return;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if hit(self.buttons.0) {
            self.expand_all(ui, true, texts);
            return;
        }
        if hit(self.buttons.1) {
            self.expand_all(ui, false, texts);
            return;
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        if let Some(i) = row_of(ui, &ks, src)
            && let WalletRow::Category(c) = self.rows[i].1
        {
            let expanded = self.collapsed.remove(&c);
            if !expanded {
                self.collapsed.insert(c);
            }
            match self.list {
                // A Tree hides and shows the category's entries itself (the press may
                // already have toggled it; setting the state is idempotent).
                Some(l) if ui.class(l) == Some(crate::WidgetClass::Tree) => {
                    ui.tree_set_expanded(l, self.rows[i].0, expanded);
                }
                _ => self.rebuild(ui, texts),
            }
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
