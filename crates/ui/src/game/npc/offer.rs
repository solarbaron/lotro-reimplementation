// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest-offer window (`npc-windows.md` §6): the conversation window as a quest offer
//! with its header (§6.1), the offer line (§6.4), the objective block (§6.2), the reward
//! block in the order of §6.3 with the feed's amount rows (§10.3), accept and decline
//! (§6.5) and the local highlight of a selectable reward.
//!
//! The same reward rows fill the quest log's details pane (`quest-journal.md` §4.7 item 6),
//! through [`RewardRows`].

use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npcvocab::NpcVocab;
use super::super::{IconImage, set_icon, set_image_ref, show};
use super::coins::{CoinCounts, MoneyDisplay, MoneyOptions, fallback_display};
use super::kit::{
    ClickKind, FNode, FallbackArt, Listen, adopt, clear_rows, click_of, colours, commit_rows, find,
    fit_height, icon_part, instantiate, mark_row, put_text, row_from_template, row_of, text_part,
};
use super::{NpcAction, NpcLabel, NpcTexts, WindowId, label_text};

/// Most reward rows a block shows (Tessera bound; the feed sends at most 16 amounts).
pub const MAX_REWARD_ROWS: usize = 256;

/// The group type shown in the offer's quick-info field (§6.1 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GroupType {
    /// `Quest_SoloField`.
    #[default]
    Solo,
    /// `Quest_SmallFellowshipField`.
    SmallFellowship,
    /// `Quest_FellowshipField`.
    Fellowship,
    /// `Quest_RaidField`.
    Raid,
}

impl GroupType {
    /// §6.1 Tessera rule: raid when `Quest_IsRaidQuest`, else fellowship when
    /// `Quest_IsFellowshipRecommended`, else small fellowship when
    /// `Quest_IsSmallFellowshipRecommended`, else solo.
    #[must_use]
    pub fn from_flags(raid: bool, fellowship: bool, small_fellowship: bool) -> Self {
        if raid {
            Self::Raid
        } else if fellowship {
            Self::Fellowship
        } else if small_fellowship {
            Self::SmallFellowship
        } else {
            Self::Solo
        }
    }
}

/// A feed reward amount's kind (`scenario-feed.md` §9.1 `RewardKind`, §10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmountKind {
    /// Coins (`QuestRewards_MoneyRewardText`).
    Coin,
    /// Reputation with a faction (`QuestRewards_ReputationField`).
    Reputation,
    /// Glory (`QuestRewards_GloryField`).
    Glory,
    /// Item experience (`QuestRewards_ItemExpField`).
    ItemXp,
    /// Craft experience (`QuestRewards_CraftExpField`).
    CraftXp,
    /// Mount experience (`QuestRewards_MountExpField`).
    MountXp,
    /// Virtue experience (`QuestRewards_VirtueExpField`).
    VirtueXp,
    /// Class trait points (`QuestRewards_ClassTraitPointsField`).
    TraitPoints,
    /// A wallet item (`QuestRewards_PaperItemField`).
    Wallet,
}

/// One amount of a `QuestRewards` event, rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardAmount {
    /// The kind.
    pub kind: AmountKind,
    /// The amount (coins: in units of the value-1 currency).
    pub amount: i64,
    /// The coin counts of a coin amount (split by the client, §9.3).
    pub coins: CoinCounts,
    /// The faction's or wallet item's name.
    pub name: String,
    /// The wallet item's icon.
    pub icon: Option<ImageRef>,
}

/// One row of a reward block, in display order (§6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardRow {
    /// `QuestRewards_Header` (top of the block when any row exists).
    Header,
    /// The money row with its coins.
    Money(CoinCounts),
    /// A reputation row: the faction's name, the absolute amount and the sign's state.
    Reputation {
        /// The faction's name.
        faction: String,
        /// The absolute amount.
        amount: i64,
        /// Positive (`reward_positivereward`) or negative (`reward_negativereward`).
        positive: bool,
    },
    /// A glory, experience or trait-point row: the amount as a decimal integer.
    Amount {
        /// The kind.
        kind: AmountKind,
        /// The amount.
        amount: i64,
    },
    /// A wallet-item row: the amount with the item's name.
    Wallet {
        /// The item's name.
        name: String,
        /// The amount.
        amount: i64,
        /// The item's icon.
        icon: Option<ImageRef>,
    },
    /// An item (fixed or selectable): an item display with the quantity overlay.
    Item {
        /// The item's name.
        name: String,
        /// The composed icon.
        icon: Option<IconImage>,
        /// `QuestTreasure_ItemQuantity` (overlay above 1).
        quantity: i64,
        /// The client's reference for the examination tooltip.
        item: Option<u64>,
        /// A selectable reward.
        selectable: bool,
    },
    /// A trait: icon and name.
    Trait {
        /// The trait's name.
        name: String,
        /// The trait's icon.
        icon: Option<ImageRef>,
        /// A selectable reward.
        selectable: bool,
    },
    /// A title, emote or virtue: a text row.
    Text {
        /// The text.
        text: String,
        /// A selectable reward.
        selectable: bool,
    },
    /// `QuestRewards_SelectableRewardsHeader`.
    SelectableHeader,
}

impl RewardRow {
    /// Whether the row is a selectable reward.
    #[must_use]
    pub fn is_selectable(&self) -> bool {
        matches!(
            self,
            Self::Item {
                selectable: true,
                ..
            } | Self::Trait {
                selectable: true,
                ..
            } | Self::Text {
                selectable: true,
                ..
            }
        )
    }
}

/// The position of an amount row in §6.3 step 1 (lower first); `None` for the virtue
/// experience row, which Tessera places after the fixed virtues (the end of step 2, as
/// `tessera-gamedata` does; `npc-windows.md` §10.3 OPEN).
fn amount_rank(a: &RewardAmount) -> Option<u8> {
    Some(match a.kind {
        AmountKind::Coin => 0,
        AmountKind::Reputation if a.amount >= 0 => 1,
        AmountKind::Reputation => 2,
        AmountKind::Glory => 3,
        AmountKind::ItemXp => 4,
        AmountKind::CraftXp => 5,
        AmountKind::MountXp => 6,
        AmountKind::Wallet => 7,
        AmountKind::TraitPoints => 8,
        AmountKind::VirtueXp => return None,
    })
}

fn amount_row(a: &RewardAmount) -> RewardRow {
    match a.kind {
        AmountKind::Coin => RewardRow::Money(a.coins),
        AmountKind::Reputation => RewardRow::Reputation {
            faction: a.name.clone(),
            amount: a.amount.saturating_abs(),
            positive: a.amount >= 0,
        },
        AmountKind::Wallet => RewardRow::Wallet {
            name: a.name.clone(),
            amount: a.amount,
            icon: a.icon.clone(),
        },
        k => RewardRow::Amount {
            kind: k,
            amount: a.amount,
        },
    }
}

/// The reward block's rows (§6.3, §10.3): the amount rows in the order of step 1 (not
/// the event's order; several reputation or wallet rows keep their relative order,
/// positive reputation before negative), then the fixed rows as given (items, traits,
/// titles, emotes, virtues), then the virtue experience row, then, when there are
/// selectable rewards, the selectable header and those rows; the rewards header on top
/// when any row was added (step 4). At most [`MAX_REWARD_ROWS`] rows.
#[must_use]
pub fn reward_rows(
    amounts: &[RewardAmount],
    fixed: &[RewardRow],
    selectable: &[RewardRow],
) -> Vec<RewardRow> {
    let mut ranked: Vec<(u8, usize, &RewardAmount)> = amounts
        .iter()
        .enumerate()
        .filter_map(|(i, a)| amount_rank(a).map(|r| (r, i, a)))
        .collect();
    ranked.sort_by_key(|(r, i, _)| (*r, *i));
    let mut rows: Vec<RewardRow> = ranked.into_iter().map(|(_, _, a)| amount_row(a)).collect();
    rows.extend(fixed.iter().cloned());
    rows.extend(
        amounts
            .iter()
            .filter(|a| a.kind == AmountKind::VirtueXp)
            .map(amount_row),
    );
    if !selectable.is_empty() {
        rows.push(RewardRow::SelectableHeader);
        rows.extend(selectable.iter().cloned());
    }
    if !rows.is_empty() {
        rows.insert(0, RewardRow::Header);
    }
    rows.truncate(MAX_REWARD_ROWS);
    rows
}

/// Row templates of the objective and reward blocks (layouts `questobjectives`,
/// `questrewardslist` and the reward bases, §6.2, §6.3). `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowTemplates {
    /// `QuestObjectives_Header`.
    pub objectives_header: Option<ExamTemplate>,
    /// `QuestObjectives_Objective`.
    pub objective: Option<ExamTemplate>,
    /// `QuestRewards_Header`.
    pub rewards_header: Option<ExamTemplate>,
    /// `QuestRewards_MoneyRewardText`.
    pub money: Option<ExamTemplate>,
    /// `QuestRewards_ReputationField`.
    pub reputation: Option<ExamTemplate>,
    /// The glory, experience and trait-point rows (`QuestRewards_GloryField`, …), by
    /// [`AmountKind`] (the client fills the ones it has).
    pub glory: Option<ExamTemplate>,
    /// `QuestRewards_ItemExpField`.
    pub item_xp: Option<ExamTemplate>,
    /// `QuestRewards_CraftExpField`.
    pub craft_xp: Option<ExamTemplate>,
    /// `QuestRewards_MountExpField`.
    pub mount_xp: Option<ExamTemplate>,
    /// `QuestRewards_VirtueExpField`.
    pub virtue_xp: Option<ExamTemplate>,
    /// `QuestRewards_ClassTraitPointsField`.
    pub trait_points: Option<ExamTemplate>,
    /// `QuestRewards_PaperItemField`.
    pub wallet: Option<ExamTemplate>,
    /// `QuestRewards_StaticRewardField`.
    pub fixed: Option<ExamTemplate>,
    /// `QuestRewards_SelectableRewardsHeader`.
    pub selectable_header: Option<ExamTemplate>,
    /// `QuestRewards_SelectableRewardField`.
    pub selectable: Option<ExamTemplate>,
}

impl RowTemplates {
    fn for_row(&self, r: &RewardRow) -> Option<ExamTemplate> {
        match r {
            RewardRow::Header => self.rewards_header,
            RewardRow::Money(_) => self.money,
            RewardRow::Reputation { .. } => self.reputation,
            RewardRow::Amount { kind, .. } => match kind {
                AmountKind::Glory => self.glory,
                AmountKind::ItemXp => self.item_xp,
                AmountKind::CraftXp => self.craft_xp,
                AmountKind::MountXp => self.mount_xp,
                AmountKind::VirtueXp => self.virtue_xp,
                AmountKind::TraitPoints => self.trait_points,
                _ => None,
            },
            RewardRow::Wallet { .. } => self.wallet,
            RewardRow::SelectableHeader => self.selectable_header,
            r if r.is_selectable() => self.selectable,
            _ => self.fixed,
        }
    }
}

/// The label a fallback amount row puts before its amount.
fn amount_label(kind: AmountKind) -> &'static str {
    match kind {
        AmountKind::Coin => "",
        AmountKind::Reputation => "Reputation",
        AmountKind::Glory => "Glory",
        AmountKind::ItemXp => "Item experience",
        AmountKind::CraftXp => "Craft experience",
        AmountKind::MountXp => "Mount experience",
        AmountKind::VirtueXp => "Virtue experience",
        AmountKind::TraitPoints => "Trait points",
        AmountKind::Wallet => "",
    }
}

/// Width of a reward row (§6.3: text 319 at x 45).
pub const REWARD_ROW_WIDTH: i32 = 364;

/// Height of a reward row (Tessera: the 32-pixel icon with a 4-pixel margin).
pub const REWARD_ROW_HEIGHT: i32 = 40;

/// The rows of an objective-and-reward list, shared by the quest offer and the journal's
/// details pane.
#[derive(Debug, Clone)]
pub struct RewardRows {
    /// The list (or field) the rows are created in.
    pub list: ElementKey,
    /// The rows: element and its index into the last rows given (`None` for the
    /// objective rows).
    pub rows: Vec<(ElementKey, Option<usize>)>,
    /// The money displays of money rows.
    pub money: Vec<MoneyDisplay>,
    /// The selected selectable reward (row index into [`RewardRows::rows`]).
    pub selected: Option<usize>,
    selectable: Vec<bool>,
    templates: RowTemplates,
    art: FallbackArt,
    nv: NpcVocab,
}

impl RewardRows {
    /// Rows created in `list`.
    #[must_use]
    pub fn new(
        list: ElementKey,
        nv: &NpcVocab,
        templates: RowTemplates,
        art: &FallbackArt,
    ) -> Self {
        Self {
            list,
            rows: Vec::new(),
            money: Vec::new(),
            selected: None,
            selectable: Vec::new(),
            templates,
            art: art.clone(),
            nv: nv.clone(),
        }
    }

    /// Removes every row.
    pub fn clear(&mut self, ui: &mut Ui) {
        let mut ks: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        clear_rows(ui, self.list, &mut ks);
        self.money.clear();
        self.selectable.clear();
        self.selected = None;
    }

    /// The width of fallback rows: the list's width (Tessera rule), within 120 … 2048.
    fn width(&self, ui: &Ui) -> i32 {
        ui.layout_box(self.list)
            .map_or(REWARD_ROW_WIDTH, |b| b.w())
            .clamp(120, 2048)
    }

    /// Adds a header row with `text` (fallback) from `template`.
    fn header(&mut self, ui: &mut Ui, template: Option<ExamTemplate>, text: &str) {
        let cp = self.nv.pids.text_font_color;
        let w = self.width(ui);
        let k = match template.and_then(|t| row_from_template(ui, t)) {
            Some(k) => Some(k),
            None => instantiate(
                ui,
                self.list,
                &FNode::text(0, 0, 0, w, 20, cp, colours::YELLOW, text),
            ),
        };
        if let Some(k) = k
            && adopt(ui, self.list, k)
        {
            self.rows.push((k, None));
            self.selectable.push(false);
        }
    }

    /// Makes the rows from index `start` on entries of the list (one layout).
    fn commit(&mut self, ui: &mut Ui, start: usize) {
        let ks: Vec<ElementKey> = self.rows[start.min(self.rows.len())..]
            .iter()
            .map(|(k, _)| *k)
            .collect();
        let kept = commit_rows(ui, self.list, &ks);
        if kept.len() != ks.len() {
            let mut keep = Vec::with_capacity(self.rows.len());
            for (j, (k, _)) in self.rows.iter().enumerate() {
                keep.push(j < start || kept.contains(k));
            }
            let mut it = keep.iter();
            self.rows.retain(|_| *it.next().unwrap_or(&false));
            let mut it = keep.iter();
            self.selectable.retain(|_| *it.next().unwrap_or(&false));
        }
    }

    /// Fills the objective block (§6.2 Tessera rule): the header, then one entry with the
    /// objective's text (markup on, wrapping); nothing without a text.
    pub fn objective(&mut self, ui: &mut Ui, text: Option<&str>, texts: &dyn NpcTexts) {
        let Some(text) = text.filter(|t| !t.is_empty()) else {
            return;
        };
        let start = self.rows.len();
        let header = label_text(texts, NpcLabel::ObjectivesHeader);
        self.header(ui, self.templates.objectives_header, &header);
        self.lines(ui, &[text.to_string()]);
        self.commit(ui, start);
    }

    /// Adds one `QuestObjectives_Objective` entry per line (bulleted, markup on, wrapping;
    /// `quest-journal.md` §5.2 step 3).
    pub fn objective_lines(&mut self, ui: &mut Ui, lines: &[String]) {
        let start = self.rows.len();
        self.lines(ui, lines);
        self.commit(ui, start);
    }

    fn lines(&mut self, ui: &mut Ui, lines: &[String]) {
        let cp = self.nv.pids.text_font_color;
        let markup = self.nv.pids.parse_markup;
        let w = self.width(ui);
        for line in lines.iter().take(MAX_REWARD_ROWS) {
            let k = match self
                .templates
                .objective
                .and_then(|t| row_from_template(ui, t))
            {
                Some(k) => Some(k),
                None => instantiate(
                    ui,
                    self.list,
                    &FNode::text(self.nv.ids.objective, 0, 0, w, 16, cp, colours::YELLOW, "")
                        .prop(markup, tessera_formats::property::PropertyValue::Bool(true)),
                ),
            };
            let Some(k) = k else { break };
            let t = text_part(ui, k, Some(self.nv.ids.objective)).unwrap_or(k);
            put_text(ui, Some(t), line);
            if t == k {
                fit_height(ui, k, 16, 4096);
            }
            if adopt(ui, self.list, k) {
                self.rows.push((k, None));
                self.selectable.push(false);
            }
        }
    }

    /// Fills the reward rows (§6.3, Tessera rule (fill)). Rows past
    /// [`MAX_REWARD_ROWS`] are dropped.
    pub fn rewards(&mut self, ui: &mut Ui, rows: &[RewardRow], texts: &dyn NpcTexts) {
        let start = self.rows.len();
        for (i, r) in rows.iter().enumerate().take(MAX_REWARD_ROWS) {
            self.reward(ui, i, r, texts);
        }
        self.commit(ui, start);
    }

    fn reward(&mut self, ui: &mut Ui, i: usize, r: &RewardRow, texts: &dyn NpcTexts) {
        let template = self.templates.for_row(r);
        match r {
            RewardRow::Header => {
                let t = label_text(texts, NpcLabel::RewardsHeader);
                self.header(ui, template, &t);
                return;
            }
            RewardRow::SelectableHeader => {
                let t = label_text(texts, NpcLabel::SelectableHeader);
                self.header(ui, template, &t);
                return;
            }
            _ => {}
        }
        let fallback = template.is_none();
        let made = template.and_then(|t| row_from_template(ui, t));
        let k = match made {
            Some(k) => Some(k),
            None => instantiate(ui, self.list, &self.fallback_row(ui, r)),
        };
        let Some(k) = k else { return };
        if !adopt(ui, self.list, k) {
            return;
        }
        let ids = self.nv.ids;
        let st = self.nv.states;
        // §6.3: entries of `QuestReward_Base` use one of its parts (item display, trait
        // display, image with text, billing token); the others, which carry the layout's
        // sample texts, are hidden. Rows without these parts are unaffected.
        let part = |id: u32| find(ui, k, id);
        let (item_p, trait_p, image_p, text_p, billing_p) = (
            part(ids.reward_item_part),
            part(ids.reward_trait_part),
            part(ids.reward_image_part),
            part(ids.reward_text_part),
            part(ids.reward_billing_part),
        );
        let runic_p = part(ids.reward_runic_part);
        let has_parts = item_p.is_some() || trait_p.is_some() || text_p.is_some();
        let (name_el, icon_el) = if has_parts {
            let keep: [Option<ElementKey>; 2] = match r {
                RewardRow::Item { .. } => [item_p, None],
                RewardRow::Trait { .. } => [trait_p, None],
                _ => [image_p, text_p],
            };
            for p in [item_p, trait_p, image_p, text_p, billing_p, runic_p]
                .into_iter()
                .flatten()
            {
                ui.set_shown(p, keep.contains(&Some(p)));
            }
            match r {
                RewardRow::Item { .. } => (
                    item_p.and_then(|p| text_part(ui, p, Some(ids.item_name))),
                    item_p.and_then(|p| icon_part(ui, p, Some(ids.item_icon))),
                ),
                RewardRow::Trait { .. } => (
                    trait_p.and_then(|p| text_part(ui, p, None)).or(text_p),
                    trait_p.and_then(|p| icon_part(ui, p, None).or(Some(p))),
                ),
                _ => (text_p, image_p),
            }
        } else {
            (
                text_part(ui, k, Some(ids.item_name)),
                icon_part(ui, k, Some(ids.item_icon)),
            )
        };
        let amount = |label: &str, a: i64| {
            let n = texts.amount(a);
            if fallback && !label.is_empty() {
                format!("{label} {n}")
            } else {
                n
            }
        };
        match r {
            RewardRow::Money(c) => {
                if let Some(m) = MoneyDisplay::find(ui, &self.nv, k) {
                    let mut d = MoneyDisplay::attach(ui, &self.nv, m, &self.art);
                    d.set(ui, *c, false, texts);
                    self.money.push(d);
                }
            }
            RewardRow::Reputation {
                faction,
                amount: a,
                positive,
            } => {
                put_text(ui, name_el, &format!("{faction} {}", texts.amount(*a)));
                ui.switch_state_if_defined(
                    k,
                    if *positive {
                        st.reward_positive
                    } else {
                        st.reward_negative
                    },
                );
            }
            RewardRow::Amount { kind, amount: a } => {
                put_text(ui, name_el, &amount(amount_label(*kind), *a));
            }
            RewardRow::Wallet {
                name,
                amount: a,
                icon,
            } => {
                put_text(ui, name_el, &format!("{} {name}", texts.amount(*a)));
                if let (Some(ic), Some(img)) = (icon_el, icon) {
                    set_image_ref(ui, ic, img, super::kit::ART_MODE);
                }
            }
            RewardRow::Item {
                name,
                icon,
                quantity,
                item,
                ..
            } => {
                put_text(ui, name_el, name);
                if let Some(ic) = icon_el {
                    set_icon(ui, ic, icon.as_ref());
                    ui.set_examine(ic, item.is_some());
                }
                let q = find(ui, k, ids.item_quantity);
                if let Some(q) = q {
                    if *quantity > 1 {
                        ui.set_text(q, Some(texts.amount(*quantity)));
                        ui.set_shown(q, true);
                    } else {
                        ui.set_shown(q, false);
                    }
                }
            }
            RewardRow::Trait { name, icon, .. } => {
                put_text(ui, name_el, name);
                if let (Some(ic), Some(img)) = (icon_el, icon) {
                    set_image_ref(ui, ic, img, super::kit::ART_MODE);
                }
            }
            RewardRow::Text { text, .. } => put_text(ui, name_el, text),
            RewardRow::Header | RewardRow::SelectableHeader => {}
        }
        self.rows.push((k, Some(i)));
        self.selectable.push(r.is_selectable());
    }

    /// A fallback reward row of the §6.3 shape: background, a 32 × 32 icon at (12, 4), a
    /// text 319 × 33 at (45, 4); the money row holds a money display instead of the text.
    fn fallback_row(&self, ui: &Ui, r: &RewardRow) -> FNode {
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let st = ui.vocab().states;
        let id = match r {
            RewardRow::Money(_) => ids.money_reward,
            RewardRow::Reputation { .. } => ids.reputation_reward,
            RewardRow::Amount { kind, .. } => match kind {
                AmountKind::Glory => ids.glory_reward,
                AmountKind::ItemXp => ids.item_xp_reward,
                AmountKind::CraftXp => ids.craft_xp_reward,
                AmountKind::MountXp => ids.mount_xp_reward,
                AmountKind::VirtueXp => ids.virtue_xp_reward,
                _ => ids.trait_points_reward,
            },
            RewardRow::Wallet { .. } => ids.wallet_reward,
            r if r.is_selectable() => ids.selectable_reward,
            _ => ids.static_reward,
        };
        let w = self.width(ui);
        let mut row = FNode::field(id, 0, 0, w, REWARD_ROW_HEIGHT)
            .image(&self.art.row)
            .state(st.normal, self.art.row.as_ref(), vec![])
            .state(st.highlight, self.art.row_highlight.as_ref(), vec![]);
        if let RewardRow::Money(_) = r {
            let v = ui.vocab().clone();
            return row.child(fallback_display(
                &v,
                &self.nv,
                0,
                45,
                9,
                MoneyOptions::LEFT_JUSTIFIED,
            ));
        }
        let has_icon = matches!(
            r,
            RewardRow::Item { .. } | RewardRow::Trait { .. } | RewardRow::Wallet { .. }
        );
        if has_icon {
            row = row.child(
                FNode::field(0, 12, 4, 32, 32)
                    .image(&self.art.slot)
                    .child(FNode::field(ids.item_icon, 0, 0, 32, 32))
                    .child(
                        FNode::text(ids.item_quantity, 0, 18, 30, 14, cp, colours::WHITE, "")
                            .hidden(),
                    ),
            );
        }
        let colour = match r {
            RewardRow::Reputation {
                positive: false, ..
            } => colours::RED,
            _ => colours::BODY,
        };
        row.child(FNode::text(
            ids.item_name,
            45,
            12,
            w - 45,
            18,
            cp,
            colour,
            "",
        ))
    }

    /// A click on a selectable reward row highlights it locally (§6.5 Tessera rule);
    /// returns whether the click selected a row. Nothing is sent.
    pub fn click(&mut self, ui: &mut Ui, src: ElementKey) -> bool {
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let Some(i) = row_of(ui, &ks, src) else {
            return false;
        };
        if !self.selectable.get(i).copied().unwrap_or(false) {
            return false;
        }
        if let Some(old) = self.selected.and_then(|o| ks.get(o)) {
            mark_row(ui, *old, false);
        }
        mark_row(ui, ks[i], true);
        self.selected = Some(i);
        true
    }

    /// Registers clicks on the selectable rows.
    fn listen(&self, ui: &mut Ui, l: &mut Listen) {
        for (j, (k, _)) in self.rows.iter().enumerate() {
            if self.selectable.get(j).copied().unwrap_or(false) {
                l.clicks(ui, Some(*k));
            }
        }
    }
}

/// The header and the texts of a quest offer (§6.1, §6.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OfferView {
    /// The feed window.
    pub window: WindowId,
    /// `Quest_Name` (or the literal).
    pub name: String,
    /// The effective challenge level; `None` hides the level field
    /// (`Quest_HideChallengeLevel`, or a literal quest).
    pub level: Option<i32>,
    /// The group type.
    pub group: GroupType,
    /// `Quest_MaxTimesCompletable` is not 1.
    pub repeatable: bool,
    /// The offer line (§6.4; may be empty).
    pub text: String,
    /// The objective text of §6.2 (`None`: no objective block).
    pub objective: Option<String>,
    /// The reward rows ([`reward_rows`]).
    pub rewards: Vec<RewardRow>,
}

/// A quest-offer window instance (the conversation window, §6).
#[derive(Debug, Clone)]
pub struct OfferWindow {
    /// The feed window shown.
    pub window: WindowId,
    /// The root (`RootQuestField`).
    pub root: ElementKey,
    /// `QuestDispenser_Label`.
    pub label: Option<ElementKey>,
    /// `Quest_LevelLabel`.
    pub level_label: Option<ElementKey>,
    /// `Quest_LevelText`.
    pub level_text: Option<ElementKey>,
    /// The group fields: solo, small fellowship, fellowship, raid.
    pub groups: [Option<ElementKey>; 4],
    /// `RepeatableImageField`.
    pub repeatable: Option<ElementKey>,
    /// `QuestDispenserIntroText`.
    pub text: Option<ElementKey>,
    /// `QuestAcceptButton`.
    pub accept: Option<ElementKey>,
    /// `QuestDeclineButton`.
    pub decline: Option<ElementKey>,
    /// `QuestCloseButton`.
    pub close: Option<ElementKey>,
    /// The objective and reward rows (in `Quest_ObjectivesRewardsList`).
    pub rows: Option<RewardRows>,
    view: OfferView,
    fonts: super::kit::FontLadder,
    nv: NpcVocab,
    listen: Listen,
}

impl OfferWindow {
    /// Prepares the conversation window below `root` as a quest offer (§6.1): the quick
    /// info and the objective-and-reward field shown, the mission panes hidden, of the
    /// button fields only `AcceptDeclineField` shown with its store buttons hidden.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        templates: RowTemplates,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        for id in [
            ids.mission_pane,
            ids.twist_pane,
            ids.now_later_field,
            ids.teleport_field,
            ids.continue_field,
            ids.comment_field,
            ids.open_ui_menu_field,
            ids.open_instance_field,
            ids.increase_slots_button,
            ids.buy_now_button,
        ] {
            show(ui, f(ui, id), false);
        }
        for id in [
            ids.quick_info,
            ids.objectives_rewards,
            ids.accept_decline_field,
        ] {
            show(ui, f(ui, id), true);
        }
        let list = f(ui, ids.objectives_rewards_list);
        let mut w = Self {
            window: 0,
            root,
            label: f(ui, ids.dispenser_label),
            level_label: f(ui, ids.level_label),
            level_text: f(ui, ids.level_text),
            groups: [
                f(ui, ids.solo_field),
                f(ui, ids.small_fellowship_field),
                f(ui, ids.fellowship_field),
                f(ui, ids.raid_field),
            ],
            repeatable: f(ui, ids.repeatable_image),
            text: f(ui, ids.intro_text),
            accept: f(ui, ids.accept_button),
            decline: f(ui, ids.decline_button),
            close: f(ui, ids.quest_close),
            rows: list.map(|l| RewardRows::new(l, nv, templates, art)),
            view: OfferView::default(),
            fonts: super::kit::FontLadder::default(),
            nv: nv.clone(),
            listen: Listen::default(),
        };
        for k in [w.accept, w.decline, w.close] {
            w.listen.clicks(ui, k);
        }
        // The font-size arrows (§5.2 Tessera rule, `fonts.md` §9.2), as in the
        // conversation window.
        let (up, down) = (f(ui, ids.bestow_font_up), f(ui, ids.bestow_font_down));
        w.fonts = super::kit::FontLadder::new(ui, &mut w.listen, up, down);
        for id in [ids.intro_text, ids.mission_text, ids.bestow_font_example] {
            let k = f(ui, id);
            w.fonts.add(ui, nv, k);
        }
        w
    }

    /// Restores the font index of the font-size arrows from Tessera's settings. Returns
    /// whether it was taken.
    pub fn set_font_index(&mut self, ui: &mut Ui, index: u32) -> bool {
        let i = usize::try_from(index).unwrap_or(usize::MAX);
        let nv = self.nv.clone();
        self.fonts.set(ui, &nv, i)
    }

    /// Shows a `QuestOffered` (§6.1–§6.4).
    pub fn show(&mut self, ui: &mut Ui, view: &OfferView, texts: &dyn NpcTexts) {
        self.window = view.window;
        self.view = view.clone();
        put_text(ui, self.label, &view.name);
        match view.level {
            Some(l) => {
                put_text(ui, self.level_text, &l.to_string());
                show(ui, self.level_text, true);
                show(ui, self.level_label, true);
            }
            None => {
                show(ui, self.level_text, false);
                show(ui, self.level_label, false);
            }
        }
        let which = match view.group {
            GroupType::Solo => 0,
            GroupType::SmallFellowship => 1,
            GroupType::Fellowship => 2,
            GroupType::Raid => 3,
        };
        for (i, g) in self.groups.iter().enumerate() {
            show(ui, *g, i == which);
        }
        show(ui, self.repeatable, view.repeatable);
        put_text(ui, self.text, &view.text);
        self.fill_rows(ui, texts);
    }

    /// Replaces the reward rows (a `QuestRewards` for the open offer, §10.3: the client
    /// computes the rows again with [`reward_rows`]).
    pub fn set_rewards(&mut self, ui: &mut Ui, rewards: Vec<RewardRow>, texts: &dyn NpcTexts) {
        self.view.rewards = rewards;
        self.fill_rows(ui, texts);
    }

    fn fill_rows(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        let Some(rows) = self.rows.as_mut() else {
            return;
        };
        rows.clear(ui);
        rows.objective(ui, self.view.objective.as_deref(), texts);
        rows.rewards(ui, &self.view.rewards, texts);
        rows.listen(ui, &mut self.listen);
    }

    /// The command for a UI event (§6.5): Accept and Decline send `AnswerQuest`, the close
    /// button `CloseWindow`; a click on a selectable reward highlights it and sends
    /// nothing; a font-size arrow that changes the index gives [`NpcAction::FontSize`].
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<NpcAction> {
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        if matches!(kind, ClickKind::Button | ClickKind::Left)
            && let Some(changed) = self.fonts.click(ui, &self.nv, src)
        {
            return changed.map(|i| NpcAction::FontSize {
                index: u32::try_from(i).unwrap_or(u32::MAX),
            });
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if matches!(kind, ClickKind::Button | ClickKind::Left) {
            if hit(self.accept) {
                return Some(NpcAction::AnswerQuest {
                    window: self.window,
                    accept: true,
                });
            }
            if hit(self.decline) {
                return Some(NpcAction::AnswerQuest {
                    window: self.window,
                    accept: false,
                });
            }
            if hit(self.close) {
                return Some(self.close_action());
            }
        }
        if let Some(r) = self.rows.as_mut() {
            r.click(ui, src);
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

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
