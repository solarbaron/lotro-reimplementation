// SPDX-License-Identifier: GPL-3.0-or-later
//! From the session state to the windows' views: each feed window's content, filled from
//! `tessera-gamedata`'s NPC window model when the install is present (install texts,
//! names, icons, coins, reward rows; `npc-windows.md` §5–§9) and from the literal refs
//! and texts alone without one (Tessera's fallback, as the feed's literal refs intend,
//! `scenario-feed.md` §5.2). The views are `tessera-ui`'s plain structs (texts rendered,
//! icons as image references, prices as coin counts).
//!
//! Every dialogue line and list entry the views show is also counted in an
//! [`NpcCensus`] (`npc-windows.md` §11), so a real-data run can report what it rendered.
//!
//! Without an install: a record ref shows an empty name and no icon, a text other than a
//! literal renders empty, coins split with the values of `lua-plugin-api.md` P39
//! ([`DEFAULT_COIN_VALUES`]), and every backpack item is listed for sale with worth 0 (no
//! sale data to test; Tessera rule).

use tessera_formats::Did;
use tessera_gamedata::inventory::ItemRef;
use tessera_gamedata::npc::{
    BarterEntry as GBarter, CoinDisplay, CostSlot, DialogueLine, DialogueText, GroupType as GGroup,
    ItemDisplay, LootEntryIn, LootSource, NpcCensus, PlayerContext, PlayerVars, Price as GPrice,
    RecordRef, RewardIn, RewardKind as GKindReward, RewardRow as GRow, VendorEntry as GVendor,
    VendorEntryIn, VendorWindow as GWindow, WalletIcon,
};
use tessera_gamedata::tooltip::rules::worth_amount;
use tessera_net::domain::{
    BuybackEntry, Currency, DialogueOption, LootEntry, Price, Ref, RewardEntry, RewardKind, Text,
    VendorEntry,
};
use tessera_net::{Body, EntityId, EntityKind};
use tessera_ui::ImageRef;
use tessera_ui::game::IconImage;
use tessera_ui::game::npc::barter::{BarterCost, BarterEntry};
use tessera_ui::game::npc::coins::{CoinCounts, coin_counts};
use tessera_ui::game::npc::dialogue::{
    CommentTexts, DialogueKind, DialogueOption as UiOption, DialoguePage, window_for,
};
use tessera_ui::game::npc::loot::{LootItem, LootView};
use tessera_ui::game::npc::offer::{
    AmountKind, GroupType, OfferView, RewardAmount, RewardRow, reward_rows,
};
use tessera_ui::game::npc::vendor::{SellItem, VendorItem, fill_quantity};
use tessera_ui::game::npc::wallet::WalletEntry;

use super::data::NpcData;
use super::state::NpcWindows;
use crate::character::PlayerState;

/// Coin values without an install: gold 100,000, silver 100, copper 1 (the split of
/// `lua-plugin-api.md` P39; `npc-windows.md` §9.3 gives the same values for this install).
pub const DEFAULT_COIN_VALUES: [u64; 3] = [100_000, 100, 1];
/// The wallet category of literal refs without an install (`npc-windows.md` §9.4 rule 3).
pub const MISC_CATEGORY: &str = "Misc";
/// The category key literal refs sort under (after every install category).
pub const LITERAL_CATEGORY: u32 = u32::MAX;

/// The vendor window's view (`npc-windows.md` §7.3: a barter window when any entry has an
/// item cost).
#[derive(Debug, Clone, PartialEq)]
pub enum VendorView {
    /// The vendor window.
    Vendor {
        /// The buy entries.
        items: Vec<VendorItem>,
        /// The buy-back entries (`None`: no `BuybackShown` yet).
        buyback: Option<Vec<VendorItem>>,
    },
    /// The barter window.
    Barter(Vec<BarterEntry>),
}

/// The player context of the dialogue variables (`npc-windows.md` §5.5 Tessera rule):
/// name, species, sex, class and level from the player state.
#[must_use]
pub fn player_context(state: &PlayerState) -> PlayerContext {
    let (species, sex) = match state.body() {
        Some(Body::Character { species, sex }) => (Some(species), Some(sex)),
        _ => (None, None),
    };
    PlayerContext {
        name: state.name().map(str::to_owned),
        species,
        sex,
        class: state.class(),
        level: state.level().and_then(|l| i32::try_from(l).ok()),
        ..PlayerContext::default()
    }
}

/// A feed text as `tessera-gamedata`'s dialogue text.
#[must_use]
pub fn dialogue_text(t: &Text) -> DialogueText {
    match t {
        Text::Literal(s) => DialogueText::Literal(s.clone()),
        Text::Name(d) => DialogueText::Name(Did(d.0)),
        Text::Description(d) => DialogueText::Description(Did(d.0)),
        Text::QuestRole {
            quest,
            role,
            failure,
        } => DialogueText::QuestRole {
            quest: Did(quest.0),
            role: *role,
            failure: *failure,
        },
        Text::Comment(d) => DialogueText::Comment(Did(d.0)),
        Text::Objective { quest, objective } => DialogueText::Objective {
            quest: Did(quest.0),
            objective: *objective,
        },
        Text::Action(a) | Text::ActionFor { action: a, .. } => DialogueText::Action(*a),
    }
}

/// A feed ref as `tessera-gamedata`'s record ref.
#[must_use]
pub fn record_ref(r: &Ref) -> RecordRef {
    match r {
        Ref::Record(d) => RecordRef::Record(Did(d.0)),
        Ref::Literal(s) => RecordRef::Literal(s.clone()),
    }
}

fn reward_kind(k: RewardKind) -> GKindReward {
    match k {
        RewardKind::Coin => GKindReward::Coin,
        RewardKind::Glory => GKindReward::Glory,
        RewardKind::ItemXp => GKindReward::ItemXp,
        RewardKind::CraftXp => GKindReward::CraftXp,
        RewardKind::MountXp => GKindReward::MountXp,
        RewardKind::VirtueXp => GKindReward::VirtueXp,
        RewardKind::TraitPoints => GKindReward::TraitPoints,
        RewardKind::Reputation => GKindReward::Reputation,
        RewardKind::Wallet => GKindReward::Wallet,
    }
}

fn amount_kind(k: GKindReward) -> AmountKind {
    match k {
        GKindReward::Coin => AmountKind::Coin,
        GKindReward::Glory => AmountKind::Glory,
        GKindReward::ItemXp => AmountKind::ItemXp,
        GKindReward::CraftXp => AmountKind::CraftXp,
        GKindReward::MountXp => AmountKind::MountXp,
        GKindReward::VirtueXp => AmountKind::VirtueXp,
        GKindReward::TraitPoints => AmountKind::TraitPoints,
        GKindReward::Reputation => AmountKind::Reputation,
        GKindReward::Wallet => AmountKind::Wallet,
    }
}

/// The coin counts of a gamedata money display (gold, silver, copper).
#[must_use]
pub fn counts_of(d: &CoinDisplay) -> CoinCounts {
    CoinCounts {
        gold: d.coins[0].count,
        silver: d.coins[1].count,
        copper: d.coins[2].count,
    }
}

/// The composed icon of icon layers.
fn icon_of(l: Option<tessera_gamedata::npc::IconLayers>) -> Option<IconImage> {
    let stack = l?.stack();
    (!stack.is_empty())
        .then(|| IconImage::Layers(stack.into_iter().map(|d| ImageRef::Did(d.0)).collect()))
}

fn plain(t: Option<&tessera_text::RichText>) -> String {
    t.map(tessera_text::RichText::plain_text)
        .unwrap_or_default()
}

fn line_text(l: &DialogueLine) -> String {
    plain(l.text.as_ref())
}

/// Appends `text` so that the UI-text reader gives it back unchanged: `\\` for a backslash
/// (`string-table.md` §4.6). A `<` stays as it is: the client's reader
/// (`tessera_text::parse_markup`) has no `<<>` literal (§4.7, G13), so a `<` followed by
/// text that forms a valid tag is read as that tag (a `tessera-text` gap, reported).
fn push_escaped(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
}

/// `rich` as the text of an element with `UICore_Text_parse_markup_tags` true
/// (`npc-windows.md` §5.5, `string-table.md` §4.7): its colour, bold, italic, underline and
/// list-item runs written as markup, its backslashes escaped, so that the element's reader
/// gives back the same runs (before, the runs were flattened and their styles lost). Link runs keep their text without the link (Tessera rule: the
/// windows' texts carry no links to follow).
#[must_use]
pub fn markup_of(rich: &tessera_text::RichText) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut in_list = false;
    for span in rich.spans() {
        let st = &span.style;
        let mut text = span.text.as_str();
        if st.list_item && !in_list {
            // A break the run starts with stays before the item's opener, which then adds
            // none of its own (§4.7 line breaks).
            let rest = text.trim_start_matches('\n');
            push_escaped(&mut out, &text[..text.len() - rest.len()]);
            text = rest;
            out.push_str("<li>");
        } else if !st.list_item && in_list {
            out.push_str("</li>");
        }
        in_list = st.list_item;
        if let Some(c) = st.color {
            let _ = write!(out, "<rgb=#{:02X}{:02X}{:02X}>", c.r, c.g, c.b);
        }
        for (on, tag) in [(st.bold, "<b>"), (st.italic, "<i>"), (st.underline, "<u>")] {
            if on {
                out.push_str(tag);
            }
        }
        push_escaped(&mut out, text);
        for (on, tag) in [
            (st.underline, "</u>"),
            (st.italic, "</i>"),
            (st.bold, "</b>"),
        ] {
            if on {
                out.push_str(tag);
            }
        }
        if st.color.is_some() {
            out.push_str("</rgb>");
        }
    }
    if in_list {
        out.push_str("</li>");
    }
    out
}

/// A dialogue line for a markup-enabled element ([`markup_of`]).
fn line_markup(l: &DialogueLine) -> String {
    l.text.as_ref().map(markup_of).unwrap_or_default()
}

/// The examination reference of a list entry: the record DID (`None`: a literal, no
/// tooltip).
fn exam_of(r: &RecordRef) -> Option<u64> {
    r.did().map(|d| u64::from(d.0))
}

/// The views of the NPC windows over a player state, with or without install facts.
pub struct Present<'a> {
    data: Option<&'a NpcData>,
    state: &'a PlayerState,
    vars: Option<PlayerVars>,
    values: Vec<u64>,
    /// The backpack's stack quantities by item ref, built on first use (one walk of the
    /// backpack per view, not one per entry).
    backpack: std::cell::OnceCell<std::collections::HashMap<ItemRef, Vec<u64>>>,
}

impl std::fmt::Debug for Present<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Present")
            .field("install", &self.data.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> Present<'a> {
    /// The views of `state` with `data` (the install's facts, when present).
    #[must_use]
    pub fn new(data: Option<&'a NpcData>, state: &'a PlayerState) -> Self {
        let vars = data.map(|d| d.presenter().player(player_context(state)));
        let values = data
            .map(NpcData::coin_values)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_COIN_VALUES.to_vec());
        Self {
            data,
            state,
            vars,
            values,
            backpack: std::cell::OnceCell::new(),
        }
    }

    /// The coin values in use, largest first.
    #[must_use]
    pub fn coin_values(&self) -> &[u64] {
        &self.values
    }

    /// The coins of `amount`.
    #[must_use]
    pub fn coins(&self, amount: u64) -> CoinCounts {
        coin_counts(amount, &self.values)
    }

    fn npc(&self) -> &NpcWindows {
        self.state.npc()
    }

    fn entity_name(&self, id: Option<EntityId>) -> Option<String> {
        self.npc()
            .entity(id?)
            .map(|e| e.name.clone())
            .filter(|n| !n.is_empty())
    }

    fn entity_body(&self, id: Option<EntityId>) -> Option<Did> {
        self.npc().entity(id?)?.body.map(|d| Did(d.0))
    }

    fn colour(&self, item: &ItemDisplay) -> Option<u32> {
        self.data?.name_colour(item.quality_state)
    }

    fn literal_name(r: &Ref) -> String {
        match r {
            Ref::Literal(s) => s.clone(),
            Ref::Record(_) => String::new(),
        }
    }

    /// The window kind and page of a `DialogueShown` (§5.1–§5.5), and the row template of
    /// each option that names one (§5.3 rule 3: option number, template element ID of the
    /// choice layout; the caller resolves its icon).
    pub fn dialogue(
        &self,
        window: u32,
        speaker: Option<EntityId>,
        text: &Text,
        options: &[DialogueOption],
        census: &mut NpcCensus,
    ) -> (DialogueKind, DialoguePage, Vec<(u8, u32)>) {
        let kind = window_for(matches!(text, Text::Comment(_)), options.len());
        let mut page = DialoguePage {
            window,
            speaker: self.entity_name(speaker),
            ..DialoguePage::default()
        };
        let (Some(data), Some(vars)) = (self.data, self.vars.as_ref()) else {
            page.text = match text {
                Text::Literal(s) => s.clone(),
                _ => String::new(),
            };
            page.options = options
                .iter()
                .map(|o| UiOption {
                    option: o.option,
                    text: match &o.text {
                        Text::Literal(s) => s.clone(),
                        _ => String::new(),
                    },
                    icon: None,
                })
                .collect();
            return (kind, page, Vec::new());
        };
        let p = data.presenter();
        let body = self.entity_body(speaker);
        let dt = dialogue_text(text);
        let mut templates = Vec::new();
        match (kind, &dt) {
            (DialogueKind::Comment, DialogueText::Comment(c)) => {
                let w = p.comment_window(*c, vars);
                for l in [&w.title, &w.window_label, &w.content, &w.close] {
                    census.add_line(l);
                }
                page.text = line_markup(&w.content);
                page.comment = Some(CommentTexts {
                    title: line_text(&w.title),
                    window_label: line_text(&w.window_label),
                    close_text: line_text(&w.close),
                });
            }
            (DialogueKind::Choice, _) => {
                let opts: Vec<(u8, DialogueText)> = options
                    .iter()
                    .map(|o| (o.option, dialogue_text(&o.text)))
                    .collect();
                let c = p.choice_page(&dt, &opts, body, vars);
                census.add_line(&c.greeting);
                page.text = line_text(&c.greeting);
                templates = c
                    .options
                    .iter()
                    .filter_map(|r| Some((r.option, r.template?)))
                    .collect();
                page.options = c
                    .options
                    .iter()
                    .map(|r| {
                        census.add_line(&r.text);
                        UiOption {
                            option: r.option,
                            text: line_text(&r.text),
                            icon: None,
                        }
                    })
                    .collect();
            }
            _ => {
                let l = p.dialogue_text(&dt, body, vars);
                census.add_line(&l);
                page.text = line_markup(&l);
            }
        }
        (kind, page, templates)
    }

    /// The quest-offer view of a `QuestOffered` with its latest `QuestRewards` (§6), and
    /// the examination reference of each reward row (the item rows' records).
    pub fn offer(
        &self,
        window: u32,
        quest: &Ref,
        giver: Option<EntityId>,
        rewards: &[RewardEntry],
    ) -> OfferView {
        let rewards_in: Vec<RewardIn> = rewards
            .iter()
            .map(|r| RewardIn {
                kind: reward_kind(r.kind),
                amount: r.amount,
                what: r.what.as_ref().map(record_ref),
            })
            .collect();
        let fallback = || {
            let amounts: Vec<RewardAmount> = rewards
                .iter()
                .map(|r| RewardAmount {
                    kind: amount_kind(reward_kind(r.kind)),
                    amount: r.amount,
                    coins: self.coins(u64::try_from(r.amount).unwrap_or(0)),
                    name: r.what.as_ref().map(Self::literal_name).unwrap_or_default(),
                    icon: None,
                })
                .collect();
            OfferView {
                window,
                name: Self::literal_name(quest),
                level: None,
                group: GroupType::Solo,
                repeatable: false,
                text: Self::literal_name(quest),
                objective: None,
                rewards: reward_rows(&amounts, &[], &[]),
            }
        };
        let (Some(data), Some(vars)) = (self.data, self.vars.as_ref()) else {
            return fallback();
        };
        let giver_body = self.entity_body(giver);
        let v =
            match data
                .presenter()
                .quest_offer(&record_ref(quest), giver_body, &rewards_in, vars)
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!("quest offer: {e}");
                    return fallback();
                }
            };
        OfferView {
            window,
            name: plain(v.title.as_ref()),
            level: v.level,
            group: match v.group {
                GGroup::Raid => GroupType::Raid,
                GGroup::Fellowship => GroupType::Fellowship,
                GGroup::SmallFellowship => GroupType::SmallFellowship,
                GGroup::Solo => GroupType::Solo,
            },
            repeatable: v.repeatable,
            text: v.offer.as_ref().map(markup_of).unwrap_or_default(),
            objective: v.objective.as_ref().map(markup_of),
            rewards: v.rewards.iter().map(|r| self.reward_row(r)).collect(),
        }
    }

    fn reward_row(&self, r: &GRow) -> RewardRow {
        match r {
            GRow::Header => RewardRow::Header,
            GRow::Money(d) => RewardRow::Money(counts_of(d)),
            GRow::Reputation {
                name,
                amount,
                positive,
                ..
            } => RewardRow::Reputation {
                faction: plain(name.as_ref()),
                amount: *amount,
                positive: *positive,
            },
            GRow::Amount { kind, amount } => RewardRow::Amount {
                kind: amount_kind(*kind),
                amount: *amount,
            },
            GRow::Wallet { item, name, amount } => RewardRow::Wallet {
                name: plain(name.as_ref()),
                amount: *amount,
                icon: item
                    .did()
                    .and_then(|d| self.data.map(|n| n.wallet_item(d).small_image))
                    .filter(|d| *d != 0)
                    .map(ImageRef::Did),
            },
            GRow::Item {
                item,
                quantity,
                selectable,
            } => RewardRow::Item {
                name: plain(item.name.as_ref()),
                icon: icon_of(item.icon),
                quantity: i64::from(*quantity),
                item: exam_of(&item.what),
                selectable: *selectable,
            },
            GRow::Trait { name, icon, .. } => RewardRow::Trait {
                name: plain(name.as_ref()),
                icon: icon.filter(|d| !d.is_null()).map(|d| ImageRef::Did(d.0)),
                selectable: false,
            },
            GRow::Title {
                text, selectable, ..
            }
            | GRow::Emote {
                text, selectable, ..
            } => RewardRow::Text {
                text: plain(text.as_ref()),
                selectable: *selectable,
            },
            GRow::SelectableHeader => RewardRow::SelectableHeader,
        }
    }

    /// The amount the player holds of a cost item (`npc-windows.md` §7.7 Tessera rule): a
    /// wallet item's last reported amount, else the sum of the backpack's quantities of
    /// that record.
    #[must_use]
    pub fn held(&self, what: &RecordRef) -> u64 {
        let wallet = self.state.wallet().iter().find(|(c, _)| match (c, what) {
            (Currency::Item(Ref::Record(a)), RecordRef::Record(b)) => a.0 == b.0,
            (Currency::Item(Ref::Literal(a)), RecordRef::Literal(b)) => a == b,
            _ => false,
        });
        if let Some((_, a)) = wallet {
            return *a;
        }
        self.backpack_quantities(what).iter().sum()
    }

    fn backpack_quantities(&self, what: &RecordRef) -> &[u64] {
        let map = self.backpack.get_or_init(|| {
            let mut m: std::collections::HashMap<ItemRef, Vec<u64>> =
                std::collections::HashMap::new();
            for i in self
                .state
                .inventory()
                .in_container(tessera_gamedata::inventory::Container::Backpack)
            {
                m.entry(i.what.clone())
                    .or_default()
                    .push(u64::from(i.quantity));
            }
            m
        });
        let want = match what {
            RecordRef::Record(d) => ItemRef::Record(*d),
            RecordRef::Literal(s) => ItemRef::Literal(s.clone()),
        };
        map.get(&want).map_or(&[], Vec::as_slice)
    }

    fn vendor_item(&self, e: &GVendor, census: &mut NpcCensus) -> VendorItem {
        census.add_entry(&e.item);
        let stack = u32::try_from(e.item.stack_size.max(0)).unwrap_or(0);
        VendorItem {
            entry: e.entry,
            name: plain(e.item.name.as_ref()),
            colour: self.colour(&e.item),
            icon: icon_of(e.item.icon),
            price: e.money.amount,
            coins: counts_of(&e.money),
            stack_size: stack,
            stock: e.stock,
            quantity: e.quantity.unwrap_or(1),
            fill: fill_quantity(stack, self.backpack_quantities(&e.item.what)),
            item: exam_of(&e.item.what),
        }
    }

    fn literal_vendor_item(
        &self,
        entry: u16,
        what: &Ref,
        price: &Price,
        stock: Option<u32>,
        quantity: u32,
    ) -> VendorItem {
        VendorItem {
            entry,
            name: Self::literal_name(what),
            colour: None,
            icon: None,
            price: price.coin,
            coins: self.coins(price.coin),
            stack_size: 1,
            stock,
            quantity,
            fill: 1,
            item: None,
        }
    }

    fn price_in(p: &Price) -> GPrice {
        GPrice {
            coin: p.coin,
            items: p.items.iter().map(|(r, q)| (record_ref(r), *q)).collect(),
        }
    }

    /// The view of a `VendorShown` with its latest `BuybackShown` (§7).
    pub fn vendor(
        &self,
        entries: &[VendorEntry],
        buyback: Option<&[BuybackEntry]>,
        census: &mut NpcCensus,
    ) -> VendorView {
        let coin = self
            .state
            .wallet()
            .iter()
            .find(|(c, _)| *c == Currency::Coin)
            .map(|(_, a)| *a);
        let (Some(data), Some(vars)) = (self.data, self.vars.as_ref()) else {
            if entries.iter().any(|e| !e.price.items.is_empty()) {
                return VendorView::Barter(
                    entries
                        .iter()
                        .map(|e| {
                            let mut costs: Vec<BarterCost> = e
                                .price
                                .items
                                .iter()
                                .map(|(r, q)| BarterCost::Item {
                                    name: Self::literal_name(r),
                                    icon: None,
                                    required: i64::from(*q),
                                    held: i64::try_from(self.held(&record_ref(r)))
                                        .unwrap_or(i64::MAX),
                                })
                                .collect();
                            if e.price.coin > 0 {
                                costs.push(BarterCost::Coin {
                                    coins: self.coins(e.price.coin),
                                    too_expensive: coin.is_some_and(|w| e.price.coin > w),
                                });
                            }
                            BarterEntry {
                                entry: e.entry,
                                name: Self::literal_name(&e.what),
                                icon: None,
                                quantity: 1,
                                costs,
                            }
                        })
                        .collect(),
                );
            }
            return VendorView::Vendor {
                items: entries
                    .iter()
                    .map(|e| self.literal_vendor_item(e.entry, &e.what, &e.price, e.stock, 1))
                    .collect(),
                buyback: buyback.map(|b| {
                    b.iter()
                        .map(|e| {
                            self.literal_vendor_item(e.entry, &e.what, &e.price, None, e.quantity)
                        })
                        .collect()
                }),
            };
        };
        let p = data.presenter();
        let input: Vec<VendorEntryIn> = entries
            .iter()
            .map(|e| VendorEntryIn {
                entry: e.entry,
                what: record_ref(&e.what),
                price: Self::price_in(&e.price),
                stock: e.stock,
                quantity: None,
            })
            .collect();
        match p.vendor_window(&input, coin, &|r| self.held(r), vars) {
            GWindow::Vendor(list) => VendorView::Vendor {
                items: list.iter().map(|e| self.vendor_item(e, census)).collect(),
                buyback: buyback.map(|b| {
                    b.iter()
                        .map(|e| {
                            let g = p.vendor_entry(
                                &VendorEntryIn {
                                    entry: e.entry,
                                    what: record_ref(&e.what),
                                    price: Self::price_in(&e.price),
                                    stock: None,
                                    quantity: Some(e.quantity),
                                },
                                coin,
                                vars,
                            );
                            self.vendor_item(&g, census)
                        })
                        .collect()
                }),
            },
            GWindow::Barter(list) => {
                VendorView::Barter(list.iter().map(|e| self.barter(e, census)).collect())
            }
        }
    }

    fn barter(&self, e: &GBarter, census: &mut NpcCensus) -> BarterEntry {
        census.add_entry(&e.reward);
        let costs = e
            .slots
            .iter()
            .chain(e.overflow.iter())
            .map(|c| match c {
                CostSlot::Item {
                    item,
                    quantity,
                    held,
                    ..
                } => {
                    census.add_entry(item);
                    BarterCost::Item {
                        name: plain(item.name.as_ref()),
                        icon: icon_of(item.icon),
                        required: i64::from(*quantity),
                        held: i64::try_from(*held).unwrap_or(i64::MAX),
                    }
                }
                CostSlot::Coin { money, short } => BarterCost::Coin {
                    coins: counts_of(money),
                    too_expensive: *short,
                },
            })
            .collect();
        BarterEntry {
            entry: e.entry,
            name: plain(e.reward.name.as_ref()),
            icon: icon_of(e.reward.icon),
            quantity: 1,
            costs,
        }
    }

    /// The sell list (§7.6 Tessera rule 1): the backpack items in slot order whose record
    /// is sellable, each with its worth (value × quantity, §6.11).
    #[must_use]
    pub fn sell_list(&self) -> Vec<SellItem> {
        let mut items: Vec<&tessera_gamedata::inventory::Item> = self
            .state
            .inventory()
            .in_container(tessera_gamedata::inventory::Container::Backpack)
            .collect();
        items.sort_by_key(|i| i.place.slot);
        items
            .into_iter()
            .filter_map(|it| {
                let (name, colour, icon, worth) = match (&it.what, self.data) {
                    (tessera_gamedata::inventory::ItemRef::Literal(s), _) => {
                        (s.clone(), None, None, 0)
                    }
                    (tessera_gamedata::inventory::ItemRef::Record(d), Some(data)) => {
                        let sale = data.sale(*d)?;
                        if !sale.sellable {
                            return None;
                        }
                        let vars = self.vars.as_ref()?;
                        let disp = data.presenter().item_display(&RecordRef::Record(*d), vars);
                        let worth = worth_amount(sale.value, Some(i64::from(it.quantity)))
                            .and_then(|w| u64::try_from(w).ok())
                            .unwrap_or(0);
                        (
                            plain(disp.name.as_ref()),
                            self.colour(&disp),
                            crate::character::tooltip::item_icon(
                                data.character(),
                                self.state,
                                it.id,
                            ),
                            worth,
                        )
                    }
                    (tessera_gamedata::inventory::ItemRef::Record(_), None) => {
                        (String::new(), None, None, 0)
                    }
                };
                Some(SellItem {
                    item: it.id,
                    name,
                    colour,
                    icon,
                    quantity: i64::from(it.quantity),
                    worth: self.coins(worth),
                })
            })
            .collect()
    }

    /// The loot view of a `LootShown` (§8.2): every entry of the event (the taken ones are
    /// removed by the window as `LootTaken` arrives).
    pub fn loot(
        &self,
        window: u32,
        source: Option<EntityId>,
        entries: &[LootEntry],
        coin: u64,
        census: &mut NpcCensus,
    ) -> LootView {
        let kind = source.and_then(|s| self.npc().entity(s)).map(|e| e.kind);
        let corpse = matches!(
            kind,
            Some(EntityKind::Creature | EntityKind::NonPlayer | EntityKind::Player)
        );
        let (Some(data), Some(vars)) = (self.data, self.vars.as_ref()) else {
            return LootView {
                window,
                corpse,
                coin,
                coins: self.coins(coin),
                entries: entries
                    .iter()
                    .map(|e| LootItem {
                        entry: e.entry,
                        name: Self::literal_name(&e.what),
                        quantity: i64::from(e.quantity),
                        ..LootItem::default()
                    })
                    .collect(),
            };
        };
        let input: Vec<LootEntryIn> = entries
            .iter()
            .map(|e| LootEntryIn {
                entry: e.entry,
                what: record_ref(&e.what),
                quantity: e.quantity,
            })
            .collect();
        let source_kind = if corpse {
            LootSource::Agent
        } else {
            LootSource::Object
        };
        let v = data.presenter().loot_view(&input, coin, source_kind, vars);
        LootView {
            window,
            corpse,
            coin,
            coins: v.coin.as_ref().map_or_else(|| self.coins(coin), counts_of),
            entries: v
                .rows
                .iter()
                .map(|r| {
                    census.add_entry(&r.item);
                    LootItem {
                        entry: r.entry,
                        name: plain(r.item.name.as_ref()),
                        colour: self.colour(&r.item),
                        icon: icon_of(r.item.icon),
                        quantity: i64::from(r.quantity),
                        item: exam_of(&r.item.what),
                    }
                })
                .collect(),
        }
    }

    /// The wallet window's money and entries (§9.4 rules 1–4): the coin amount and every
    /// currency with an amount above 0, with category, name, icon and cap.
    pub fn wallet(&self, census: &mut NpcCensus) -> (CoinCounts, Vec<WalletEntry>) {
        let wallet = self.state.wallet();
        let coin = wallet
            .iter()
            .find(|(c, _)| *c == Currency::Coin)
            .map_or(0, |(_, a)| *a);
        let (Some(data), Some(vars)) = (self.data, self.vars.as_ref()) else {
            let entries = wallet
                .iter()
                .filter_map(|(c, a)| match c {
                    Currency::Item(r) if *a > 0 => Some(WalletEntry {
                        did: 0,
                        name: Self::literal_name(r),
                        category: LITERAL_CATEGORY,
                        category_title: MISC_CATEGORY.to_owned(),
                        icon: None,
                        amount: i64::try_from(*a).unwrap_or(i64::MAX),
                        cap: None,
                    }),
                    _ => None,
                })
                .collect();
            return (self.coins(coin), entries);
        };
        let amounts: Vec<(tessera_gamedata::npc::Currency, u64)> = wallet
            .iter()
            .map(|(c, a)| {
                (
                    match c {
                        Currency::Coin => tessera_gamedata::npc::Currency::Coin,
                        Currency::Item(r) => tessera_gamedata::npc::Currency::Item(record_ref(r)),
                    },
                    *a,
                )
            })
            .collect();
        let view = data.presenter().wallet_view(&amounts, "", vars);
        let mut out = Vec::new();
        for cat in &view.categories {
            for e in &cat.entries {
                if !cat.no_display_text {
                    census.wallet_shown += 1;
                } else {
                    census.wallet_no_category_text += 1;
                }
                out.push(WalletEntry {
                    did: e.what.did().map_or(0, |d| d.0),
                    name: plain(e.name.as_ref()),
                    category: cat.key.unwrap_or(LITERAL_CATEGORY),
                    category_title: cat.title.clone(),
                    icon: match e.icon {
                        WalletIcon::Small(d) => Some(ImageRef::Did(d.0)),
                        // The wallet row takes one image for the composed icon: the item's
                        // image layer.
                        WalletIcon::Composed(l) => {
                            (!l.image.is_null()).then_some(ImageRef::Did(l.image.0))
                        }
                        WalletIcon::None => None,
                    },
                    amount: i64::try_from(e.amount).unwrap_or(i64::MAX),
                    cap: e
                        .what
                        .did()
                        .and_then(|d| data.presenter().wallet_catalog().get(d))
                        .and_then(|w| w.cap),
                })
            }
        }
        (counts_of(&view.money), out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_gamedata::inventory::BagLayout;
    use tessera_net::domain::InventoryEvent;
    use tessera_net::{Event, Transform, Vec3};

    fn state() -> PlayerState {
        let mut s = PlayerState::new(BagLayout {
            base_slots: 0,
            slots_per_bag: 0,
            bags: 1,
        });
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            &mut out,
        );
        s.apply(
            &Event::EntitySpawned {
                id: EntityId(9),
                kind: EntityKind::NonPlayer,
                name: "Grocer".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            &mut out,
        );
        s.apply(
            &Event::Inventory(InventoryEvent::WalletChanged {
                currency: Currency::Coin,
                amount: 1250,
            }),
            &mut out,
        );
        s
    }

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    #[test]
    fn literal_views_without_an_install() {
        let s = state();
        let p = Present::new(None, &s);
        let mut census = NpcCensus::default();
        let (kind, page, _) = p.dialogue(
            1,
            Some(EntityId(9)),
            &Text::Literal("Fresh from the orchard.".into()),
            &[DialogueOption {
                option: 1,
                text: Text::Literal("Show me".into()),
            }],
            &mut census,
        );
        assert_eq!(kind, DialogueKind::Choice);
        assert_eq!(page.speaker.as_deref(), Some("Grocer"));
        assert_eq!(page.options[0].text, "Show me");
        let (kind, _, _) = p.dialogue(
            1,
            None,
            &Text::Comment(tessera_net::DataId(5)),
            &[],
            &mut census,
        );
        assert_eq!(kind, DialogueKind::Comment);
        let VendorView::Vendor { items, buyback } = p.vendor(
            &[VendorEntry {
                entry: 1,
                what: lit("Pear cider"),
                price: Price {
                    coin: 100_005,
                    items: Vec::new(),
                },
                stock: Some(3),
            }],
            None,
            &mut census,
        ) else {
            panic!("vendor");
        };
        assert!(buyback.is_none());
        assert_eq!(items[0].name, "Pear cider");
        assert_eq!(
            items[0].coins,
            CoinCounts {
                gold: 1,
                silver: 0,
                copper: 5
            }
        );
        // An item cost makes it a barter window.
        let VendorView::Barter(b) = p.vendor(
            &[VendorEntry {
                entry: 1,
                what: lit("Cloak"),
                price: Price {
                    coin: 30,
                    items: vec![(lit("Token"), 4)],
                },
                stock: None,
            }],
            None,
            &mut census,
        ) else {
            panic!("barter");
        };
        assert_eq!(b[0].costs.len(), 2);
        let offer = p.offer(
            2,
            &lit("Lost satchel"),
            None,
            &[RewardEntry {
                kind: RewardKind::Coin,
                amount: 1250,
                what: None,
            }],
        );
        assert_eq!(offer.name, "Lost satchel");
        assert_eq!(offer.rewards.first(), Some(&RewardRow::Header));
        let loot = p.loot(
            3,
            Some(EntityId(9)),
            &[LootEntry {
                entry: 1,
                what: lit("Strap"),
                quantity: 2,
            }],
            75,
            &mut census,
        );
        assert!(loot.corpse, "a non-player source is a corpse");
        assert_eq!(loot.entries[0].quantity, 2);
        let (coins, entries) = p.wallet(&mut census);
        assert_eq!(coins.silver, 12);
        assert!(entries.is_empty());
    }

    #[test]
    fn markup_text_reads_back_as_the_same_runs() {
        for raw in [
            "plain text",
            "a <rgb=#FF8000>orange</rgb> word and <b>bold</b>, <i>it</i>, <u>under</u>",
            "a backslash \\\\ and \\q quote, \\n a break",
            "intro<li>first</li><li>second</li>outro",
            "<li>at the start</li>",
            "<rgb=#102030><u>both</u></rgb><p>next",
            "unknown <tags> stay literal",
        ] {
            let rich = tessera_text::parse_markup(raw);
            let back = tessera_text::parse_markup(&markup_of(&rich));
            assert_eq!(back, rich, "{raw:?} -> {:?}", markup_of(&rich));
        }
    }
}
