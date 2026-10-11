// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest-offer window (`npc-windows.md` §6): header, offer line, objective block and
//! reward rows, from a quest record and the feed's `QuestRewards` amounts.

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::quests::{QuestRecord, QuestTreasure, Role, RoleAction};
use tessera_formats::{Did, PropertySet, StringInfo};
use tessera_text::RichText;

use super::entries::{ItemDisplay, item_display_of, render_opt};
use super::money::CoinDisplay;
use super::{CoinOptions, NpcPresenter};
use crate::quest::{PlayerVars, QuestPresenter, RecordRef};

/// Species groups of the variant rule (`quests.md` §7.1): a group value matches either
/// species of its pair (class-library constants, decision 0012).
const SPECIES_GROUPS: [(u32, [u32; 2]); 3] = [(118, [65, 117]), (121, [73, 120]), (124, [81, 125])];

/// The group-type field shown (§6.1 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupType {
    /// `Quest_RaidField`.
    Raid,
    /// `Quest_FellowshipField`.
    Fellowship,
    /// `Quest_SmallFellowshipField`.
    SmallFellowship,
    /// `Quest_SoloField`.
    Solo,
}

/// Which rule of §6.4 gave the offer line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfferSource {
    /// Rule 1: the bestow role of the giver.
    GiverRole,
    /// Rule 2: the first bestow role with a line.
    FirstRole,
    /// Rule 3: `Quest_Description`.
    Description,
    /// Rule 4: nothing.
    Nothing,
    /// A literal quest.
    Literal,
}

/// A `QuestRewards` reward kind (`scenario-feed.md` §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RewardKind {
    /// Coins.
    Coin,
    /// Glory.
    Glory,
    /// Item experience.
    ItemXp,
    /// Craft experience.
    CraftXp,
    /// Mount experience.
    MountXp,
    /// Virtue experience.
    VirtueXp,
    /// Class trait points.
    TraitPoints,
    /// Reputation with a faction.
    Reputation,
    /// A wallet item.
    Wallet,
}

/// One `QuestRewards` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardIn {
    /// The kind.
    pub kind: RewardKind,
    /// The amount.
    pub amount: i64,
    /// The faction or wallet item (`Reputation`, `Wallet`).
    pub what: Option<RecordRef>,
}

/// One row of the reward block, in display order (§6.3, §10.3).
#[derive(Debug, Clone, PartialEq)]
pub enum RewardRow {
    /// `QuestRewards_Header`, at the top when any row follows.
    Header,
    /// The money row.
    Money(CoinDisplay),
    /// A reputation row.
    Reputation {
        /// The faction.
        faction: RecordRef,
        /// The faction's name.
        name: Option<RichText>,
        /// The absolute amount.
        amount: i64,
        /// `reward_positivereward` (true) or `reward_negativereward`.
        positive: bool,
    },
    /// A glory, experience or trait-point row with its amount as a decimal integer.
    Amount {
        /// The kind.
        kind: RewardKind,
        /// The amount.
        amount: i64,
    },
    /// A wallet-item row.
    Wallet {
        /// The item.
        item: RecordRef,
        /// Its name.
        name: Option<RichText>,
        /// The amount.
        amount: i64,
    },
    /// An item of the treasure record.
    Item {
        /// The item.
        item: ItemDisplay,
        /// `QuestTreasure_ItemQuantity` (overlay above 1).
        quantity: i32,
        /// A selectable reward.
        selectable: bool,
    },
    /// A trait (`Trait_Name`, `Trait_Icon`, `Trait_Static_Icon_Overlay`).
    Trait {
        /// The trait record.
        did: Did,
        /// Its name.
        name: Option<RichText>,
        /// `Trait_Icon`.
        icon: Option<Did>,
        /// `Trait_Static_Icon_Overlay`.
        overlay: Option<Did>,
    },
    /// A title (`Title_String`; no image).
    Title {
        /// The title record.
        did: Did,
        /// Its text.
        text: Option<RichText>,
        /// A selectable reward.
        selectable: bool,
    },
    /// An emote (`Emote_CommandString`; no image).
    Emote {
        /// The emote record.
        did: Did,
        /// Its text.
        text: Option<RichText>,
        /// A selectable reward.
        selectable: bool,
    },
    /// A virtue: the virtue property's label and the increment.
    Virtue {
        /// The label of the virtue's rank property.
        label: Option<String>,
        /// `QuestTreasure_Virtue_Increment`.
        increment: i32,
    },
    /// `QuestRewards_SelectableRewardsHeader`.
    SelectableHeader,
}

/// The quest-offer window's content (§6.1–§6.4).
#[derive(Debug, Clone, PartialEq)]
pub struct QuestOfferView {
    /// `QuestDispenser_Label`: the quest's name (the literal for a literal ref).
    pub title: Option<RichText>,
    /// The effective level; `None` hides the level field (`Quest_HideChallengeLevel`).
    pub level: Option<i32>,
    /// The group-type field shown.
    pub group: GroupType,
    /// `RepeatableImageField` shown.
    pub repeatable: bool,
    /// The offer line.
    pub offer: Option<RichText>,
    /// Which rule gave it.
    pub offer_source: OfferSource,
    /// The objective entry (the header shows exactly when this is present, §6.2).
    pub objective: Option<RichText>,
    /// The reward block.
    pub rewards: Vec<RewardRow>,
}

/// Whether a role's species value matches the player's species (§7.1 variant rule).
fn species_matches(value: u32, player: Option<u32>) -> bool {
    let Some(p) = player else { return false };
    value == p
        || SPECIES_GROUPS
            .iter()
            .any(|(g, pair)| *g == value && pair.contains(&p))
}

/// The line of a role by the variant rule of `quests.md` §7.1.
pub(crate) fn role_line<'a>(
    role: &Role<'a>,
    failure: bool,
    species: Option<u32>,
) -> Option<&'a StringInfo> {
    let variants = role.text_variants();
    if let Some(v) = variants
        .iter()
        .find(|v| v.species.is_some_and(|s| species_matches(s, species)))
    {
        return if failure {
            v.failure_text
        } else {
            v.success_text
        };
    }
    if failure {
        role.failure_text()
    } else {
        role.success_text()
    }
}

/// The offer line of §6.4 for a giver's body record (`None`: rules 2–3 only), with the rule
/// that gave it. A line that does not render counts as no line for the census but keeps
/// its rule.
pub(crate) fn offer_line_of(
    qp: &QuestPresenter<'_>,
    set: &PropertySet,
    giver: Option<Did>,
    player: &PlayerVars,
) -> (Option<RichText>, OfferSource) {
    let q = QuestRecord::new_unchecked(qp.labels(), set);
    let species = player.context.species;
    let roles = q.roles();
    let bestow: Vec<&Role<'_>> = roles
        .iter()
        .filter(|r| r.action() == Some(RoleAction::Bestow))
        .collect();
    let render = |info: &StringInfo| render_opt(qp.data(), Some(info), player);
    if let Some(g) = giver
        && let Some(info) = bestow
            .iter()
            .filter(|r| r.npc() == Some(g))
            .find_map(|r| role_line(r, false, species))
    {
        return (render(info), OfferSource::GiverRole);
    }
    if let Some(info) = bestow.iter().find_map(|r| role_line(r, false, species)) {
        return (render(info), OfferSource::FirstRole);
    }
    if let Some(info) = q.description() {
        return (render(info), OfferSource::Description);
    }
    (None, OfferSource::Nothing)
}

fn record_text(
    qp: &QuestPresenter<'_>,
    did: Did,
    pid: tessera_formats::gameplay::common::Pid,
    player: &PlayerVars,
) -> Option<RichText> {
    let set = qp.data().record(did).ok()??;
    render_opt(qp.data(), set.string_info(pid), player)
}

/// The reward rows of a quest's treasure record (§6.3 steps 2–3), without the header.
pub(crate) fn treasure_rows_of(
    qp: &QuestPresenter<'_>,
    set: &PropertySet,
    player: &PlayerVars,
) -> Vec<RewardRow> {
    let (fixed, selectable) = treasure_parts(qp, set, player);
    let mut out = fixed;
    out.extend(selectable);
    out
}

/// (fixed rows, selectable rows with their header) of the treasure record.
fn treasure_parts(
    qp: &QuestPresenter<'_>,
    set: &PropertySet,
    player: &PlayerVars,
) -> (Vec<RewardRow>, Vec<RewardRow>) {
    let q = QuestRecord::new_unchecked(qp.labels(), set);
    let Some(tdid) = q.treasure() else {
        return (Vec::new(), Vec::new());
    };
    let Ok(Some(tset)) = qp.data().record(tdid) else {
        return (Vec::new(), Vec::new());
    };
    let t = QuestTreasure::new(qp.labels(), &tset);
    let p = qp.npc_pids();
    let data = qp.data();
    let item_row = |it: &tessera_formats::gameplay::quests::TreasureItem, selectable: bool| {
        it.item.map(|d| RewardRow::Item {
            item: item_display_of(data, p, &RecordRef::Record(d), player),
            quantity: it.quantity.unwrap_or(1),
            selectable,
        })
    };
    let mut fixed: Vec<RewardRow> = t
        .fixed_items()
        .iter()
        .filter_map(|i| item_row(i, false))
        .collect();
    for d in t.fixed_traits() {
        let tr = data.record(d).ok().flatten();
        fixed.push(RewardRow::Trait {
            did: d,
            name: tr
                .as_ref()
                .and_then(|s| render_opt(data, s.string_info(p.trait_name), player)),
            icon: tr.as_ref().and_then(|s| s.did_nonnull(p.trait_icon)),
            overlay: tr.as_ref().and_then(|s| s.did_nonnull(p.trait_overlay)),
        });
    }
    for d in t.fixed_titles() {
        fixed.push(RewardRow::Title {
            did: d,
            text: record_text(qp, d, p.title_string, player),
            selectable: false,
        });
    }
    for d in t.fixed_emotes() {
        fixed.push(RewardRow::Emote {
            did: d,
            text: record_text(qp, d, p.emote_command, player),
            selectable: false,
        });
    }
    for v in t.fixed_virtues() {
        fixed.push(RewardRow::Virtue {
            label: v.virtue.and_then(|pid| data.property_name(pid)),
            increment: v.increment.unwrap_or(0),
        });
    }
    let mut selectable: Vec<RewardRow> = t
        .selectable_items()
        .iter()
        .filter_map(|i| item_row(i, true))
        .collect();
    for d in t.selectable_titles() {
        selectable.push(RewardRow::Title {
            did: d,
            text: record_text(qp, d, p.title_string, player),
            selectable: true,
        });
    }
    for d in t.selectable_emotes() {
        selectable.push(RewardRow::Emote {
            did: d,
            text: record_text(qp, d, p.emote_command, player),
            selectable: true,
        });
    }
    if !selectable.is_empty() {
        selectable.insert(0, RewardRow::SelectableHeader);
    }
    (fixed, selectable)
}

impl NpcPresenter<'_> {
    /// The amount rows of `QuestRewards` (§6.3 step 1, §10.3): money, reputation (positive
    /// then negative), glory, item, craft and mount experience, wallet items, trait
    /// points. Virtue experience is placed after the virtue rows of step 2 (see
    /// [`NpcPresenter::reward_block`]).
    fn amount_rows(
        &self,
        rewards: &[RewardIn],
        player: &PlayerVars,
    ) -> (Vec<RewardRow>, Vec<RewardRow>) {
        let p = self.pids();
        let of = |k: RewardKind| rewards.iter().filter(move |r| r.kind == k);
        let mut out = Vec::new();
        for r in of(RewardKind::Coin) {
            out.push(RewardRow::Money(self.coins(
                u64::try_from(r.amount).unwrap_or(0),
                CoinOptions::default(),
            )));
        }
        let rep = |positive: bool| {
            of(RewardKind::Reputation)
                .filter(move |r| (r.amount >= 0) == positive)
                .filter_map(|r| {
                    let faction = r.what.clone()?;
                    let name = match &faction {
                        RecordRef::Literal(s) => Some(RichText::plain(s)),
                        RecordRef::Record(d) => {
                            record_text(self.quest(), *d, p.faction_name, player)
                        }
                    };
                    Some(RewardRow::Reputation {
                        faction,
                        name,
                        amount: r.amount.saturating_abs(),
                        positive,
                    })
                })
                .collect::<Vec<_>>()
        };
        out.extend(rep(true));
        out.extend(rep(false));
        for k in [
            RewardKind::Glory,
            RewardKind::ItemXp,
            RewardKind::CraftXp,
            RewardKind::MountXp,
        ] {
            out.extend(of(k).map(|r| RewardRow::Amount {
                kind: k,
                amount: r.amount,
            }));
        }
        for r in of(RewardKind::Wallet) {
            let Some(item) = r.what.clone() else { continue };
            let name = match &item {
                RecordRef::Literal(s) => Some(RichText::plain(s)),
                RecordRef::Record(d) => record_text(self.quest(), *d, p.name, player),
            };
            out.push(RewardRow::Wallet {
                item,
                name,
                amount: r.amount,
            });
        }
        out.extend(of(RewardKind::TraitPoints).map(|r| RewardRow::Amount {
            kind: RewardKind::TraitPoints,
            amount: r.amount,
        }));
        let virtue = of(RewardKind::VirtueXp)
            .map(|r| RewardRow::Amount {
                kind: RewardKind::VirtueXp,
                amount: r.amount,
            })
            .collect();
        (out, virtue)
    }

    /// The reward block of a quest (§6.3): amount rows from the feed, the treasure
    /// record's fixed rows, the virtue-experience row, the selectable rows; the header on
    /// top when any row was added.
    #[must_use]
    pub fn reward_block(
        &self,
        quest: Option<&PropertySet>,
        rewards: &[RewardIn],
        player: &PlayerVars,
    ) -> Vec<RewardRow> {
        let (mut rows, virtue) = self.amount_rows(rewards, player);
        let (fixed, selectable) = quest
            .map(|s| treasure_parts(self.quest(), s, player))
            .unwrap_or_default();
        rows.extend(fixed);
        rows.extend(virtue);
        rows.extend(selectable);
        if !rows.is_empty() {
            rows.insert(0, RewardRow::Header);
        }
        rows
    }

    /// The quest-offer window for `QuestOffered` (§6.1–§6.4) with the latest
    /// `QuestRewards` of the window. `giver` is the giver's body record. `Err` when the
    /// quest record cannot be read.
    pub fn quest_offer(
        &self,
        quest: &RecordRef,
        giver: Option<Did>,
        rewards: &[RewardIn],
        player: &PlayerVars,
    ) -> Result<QuestOfferView, String> {
        let did = match quest {
            RecordRef::Literal(s) => {
                return Ok(QuestOfferView {
                    title: Some(RichText::plain(s)),
                    level: None,
                    group: GroupType::Solo,
                    repeatable: false,
                    offer: Some(RichText::plain(s)),
                    offer_source: OfferSource::Literal,
                    objective: None,
                    rewards: self.reward_block(None, rewards, player),
                });
            }
            RecordRef::Record(d) => *d,
        };
        let qp = self.quest();
        let set = qp.record(did)?;
        let q = QuestRecord::new_unchecked(qp.labels(), &set);
        let qpids = &qp.labels().quest;
        let level = (!q.hides_challenge_level()).then(|| qp.level(&set, &player.context).0);
        let group = if set.boolean(qpids.is_raid_quest) == Some(true) {
            GroupType::Raid
        } else if set.boolean(qpids.is_fellowship_recommended) == Some(true) {
            GroupType::Fellowship
        } else if set.boolean(qpids.is_small_fellowship_recommended) == Some(true) {
            GroupType::SmallFellowship
        } else {
            GroupType::Solo
        };
        let (offer, offer_source) = offer_line_of(qp, &set, giver, player);
        let objective = qp
            .objective_indices(&set)
            .first()
            .and_then(|&i| qp.objective_description(&set, i, player).ok().flatten());
        Ok(QuestOfferView {
            title: qp.name(&set, player).ok().flatten(),
            level,
            group,
            repeatable: set.int(qpids.max_times_completable).unwrap_or(1) != 1,
            offer,
            offer_source,
            objective,
            rewards: self.reward_block(Some(&set), rewards, player),
        })
    }
}
