// SPDX-License-Identifier: GPL-3.0-or-later
//! The progression widgets of decision 0024 P6 (`progression-panels.md`, Accepted): the
//! experience bar and level display (§2), the level-up presentation (§2.3), the trait panel
//! (tabs per tree, branches, cells with icon, rank and state, prerequisites, points, slotted
//! traits; §3) and the skill-training window (rows, prices with the P3 money display, the
//! train request; §4).
//!
//! `progression-panels.md` names install layouts for these panels (`toolbar` 0x22000033,
//! `trait` 0x22000091, `skilltrainer` 0x22000030); this module draws Tessera's own fallback
//! windows (§6), which are what the committed tier-1 goldens capture, and the experience bar
//! also works on the install's toolbar meter:
//!
//! - the experience bar works on the install's toolbar meter (the `LevelProgressMeter` game
//!   type, an engine Meter: §2.1, `game-panels.md` §11, `ui-widgets.md` §13 P3), whose
//!   `UICore_Meter_position` it sets, or on Tessera's fallback bar ([`fallback::experience_bar`]);
//! - the level-up banner, the trait panel and the skill-training window draw as Tessera's
//!   fallback windows ([`fallback`], §6) with Tessera's element IDs ([`super::p6vocab::own`]);
//! - every value is the feed's (`scenario-feed.md` §6.6 "Meaning for the client half"):
//!   experience, levels, ranks, points, trained skills and prices are scripted, never
//!   derived (decision 0024 §2 principle 4; §3.7, §4.4). A trait-cell click sends a rank
//!   request and the panel changes only on the answering events; Train sends the entry.
//!
//! As with the other P-track widgets, the client renders views (texts, icons, coin counts)
//! and passes every [`crate::UiEvent`] to the window's `handle_event`, which returns
//! [`ProgressionAction`]s.
//!
//! | Module | What |
//! |---|---|
//! | [`xpbar`] | the experience bar and the level-up banner |
//! | [`traits`] | the trait panel |
//! | [`trainer`] | the skill-training window |
//! | [`fallback`] | Tessera's fallback windows |

pub mod fallback;
pub mod trainer;
pub mod traits;
pub mod xpbar;

pub use trainer::SkillTrainerWindow;
pub use traits::TraitPanel;
pub use xpbar::{ExperienceBar, LevelUpBanner};

use super::IconImage;
use super::npc::WindowId;
use super::npc::coins::CoinCounts;

/// Most tabs of the trait panel (Tessera bound; a class has a handful of trees).
pub const MAX_TREES: usize = 32;

/// Most branch buttons of the trait panel (Tessera bound).
pub const MAX_BRANCHES: usize = 16;

/// Most cells the trait panel draws (Tessera bound; the install's largest tree places fewer
/// than 200 traits).
pub const MAX_CELLS: usize = 512;

/// Most slotted traits the trait panel lists (Tessera bound).
pub const MAX_SLOT_ROWS: usize = 64;

/// Most rows of the skill-training window (`scenario-feed.md` §6.6: at most 1,024 entries).
pub const MAX_SKILL_ROWS: usize = 1024;

/// The experience bar's values (`scenario-feed.md` §6.6 `xp` and `level`), rendered by the
/// client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExperienceView {
    /// The level shown.
    pub level: u32,
    /// The level text (Tessera: "Level *n*").
    pub level_text: String,
    /// The experience into the level.
    pub current: u64,
    /// The experience the level needs (0: the bar is empty).
    pub needed: u64,
    /// The value text (Tessera: "*current* / *needed*").
    pub text: String,
    /// The tooltip.
    pub tooltip: String,
}

impl ExperienceView {
    /// The fill: current ÷ needed clamped to 0 … 1; 0 when nothing is needed.
    #[must_use]
    pub fn position(&self) -> f32 {
        if self.needed == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let p = self.current as f64 / self.needed as f64;
        #[allow(clippy::cast_possible_truncation)]
        let p = p.clamp(0.0, 1.0) as f32;
        p
    }
}

/// A branch button of the trait panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchView {
    /// The branch key (`ChooseTraitBranch`).
    pub key: u32,
    /// The button's text.
    pub text: String,
    /// The chosen branch.
    pub chosen: bool,
}

/// One trait cell (`combat-data.md` §8.5 placements), rendered by the client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraitCellView {
    /// The client's handle of the cell's trait (returned in actions).
    pub id: u64,
    /// The grid row (Tessera: 0-based; the client maps the cell key).
    pub row: u16,
    /// The grid column (0-based).
    pub col: u16,
    /// The trait's icon.
    pub icon: Option<IconImage>,
    /// The trait's name.
    pub name: String,
    /// The rank the feed reported.
    pub rank: u16,
    /// The trait's highest rank (0: unknown, no limit drawn).
    pub max_rank: u16,
    /// The rank text (Tessera: "*rank*/*max*").
    pub rank_text: String,
    /// The prerequisites: `None` without any, `Some(met)` with.
    pub prerequisites: Option<bool>,
    /// The cell can take another rank (the client's reading of the tree, its points and the
    /// prerequisites); a left click asks for rank + 1 only then.
    pub available: bool,
    /// The tooltip (the trait's tooltip rows joined by the client).
    pub tooltip: String,
}

/// One slotted trait.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraitSlotView {
    /// The trait's icon.
    pub icon: Option<IconImage>,
    /// The row's text (Tessera: "*slot*: *name* (rank *n*)").
    pub text: String,
}

/// The trait panel's content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraitPanelView {
    /// The title.
    pub title: String,
    /// The tabs: one per trait tree, by name.
    pub trees: Vec<String>,
    /// The tab shown.
    pub current: usize,
    /// The client's handle of the shown tree (returned in actions).
    pub tree: u64,
    /// The shown tree's branches.
    pub branches: Vec<BranchView>,
    /// The shown tree's cells.
    pub cells: Vec<TraitCellView>,
    /// The points text (Tessera: "Points: *available* / *total*").
    pub points_text: String,
    /// The slotted traits.
    pub slots: Vec<TraitSlotView>,
}

/// One row of the skill-training window (`scenario-feed.md` §6.6 `trainer` and
/// `untrained`), rendered by the client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillRowView {
    /// The entry number (`TrainSkill`).
    pub entry: u16,
    /// The skill's icon.
    pub icon: Option<IconImage>,
    /// The skill's name.
    pub name: String,
    /// The level (and rank) text.
    pub level_text: String,
    /// The price in coin.
    pub coin: u64,
    /// The price split into coins (`npc-windows.md` §9.3).
    pub coins: CoinCounts,
    /// Already trained (flag bit 0): the row is ghosted.
    pub trained: bool,
    /// Requirements not met (flag bit 1): the row is ghosted and its level text red.
    pub unmet: bool,
}

impl SkillRowView {
    /// Whether the row is ghosted (trained or requirements not met).
    #[must_use]
    pub fn ghosted(&self) -> bool {
        self.trained || self.unmet
    }
}

/// The skill-training window's content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillTrainerView {
    /// The feed window.
    pub window: WindowId,
    /// The title (the trainer's name).
    pub title: String,
    /// The rows, in entry order.
    pub rows: Vec<SkillRowView>,
    /// The player's coin (the last `WalletChanged { Coin }`), for the prices'
    /// `TooExpensive` state; `None`: no comparison.
    pub player_coin: Option<u64>,
    /// The text shown while there is no row.
    pub empty_text: String,
}

/// A user action in a progression window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressionAction {
    /// The trait panel's tab `index` was chosen: the client renders that tree.
    ShowTree {
        /// The tab index.
        index: usize,
    },
    /// A cell asks for a rank: `SetTraitRank { tree, what, rank }` (`scenario-feed.md`
    /// §9.4).
    SetTraitRank {
        /// The tree's handle.
        tree: u64,
        /// The cell's trait handle.
        cell: u64,
        /// The requested rank.
        rank: u16,
    },
    /// A branch button: `ChooseTraitBranch { tree, branch }`.
    ChooseBranch {
        /// The tree's handle.
        tree: u64,
        /// The branch key.
        branch: u32,
    },
    /// The trait panel's close button.
    CloseTraits,
    /// Train: `TrainSkill { window, entry }`.
    TrainSkill {
        /// The trainer window.
        window: WindowId,
        /// The entry number.
        entry: u16,
    },
    /// The skill-training window's close button (the client sends `CloseWindow`).
    CloseTrainer {
        /// The trainer window.
        window: WindowId,
    },
}

/// A fixed label of the progression windows (Tessera's own text unless the client renders
/// the install's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgressionLabel {
    /// The level-up banner's title.
    LevelUp,
    /// The trait panel's title.
    Traits,
    /// The skill-training window's title.
    Training,
    /// The train button.
    Train,
    /// The slotted-traits heading.
    Slotted,
}

impl ProgressionLabel {
    /// Tessera's own short text.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::LevelUp => "Level up!",
            Self::Traits => "Traits",
            Self::Training => "Skill training",
            Self::Train => "Train",
            Self::Slotted => "Slotted traits",
        }
    }
}

/// Texts the progression windows ask the client for; the defaults are Tessera's own.
pub trait ProgressionTexts: super::npc::NpcTexts {
    /// A fixed label; `None` gives [`ProgressionLabel::fallback`].
    fn progression_label(&self, label: ProgressionLabel) -> Option<String> {
        let _ = label;
        None
    }
}

/// [`ProgressionTexts`] with every default.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultProgressionTexts;

impl super::npc::NpcTexts for DefaultProgressionTexts {}
impl ProgressionTexts for DefaultProgressionTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn progression_label(texts: &dyn ProgressionTexts, l: ProgressionLabel) -> String {
    texts
        .progression_label(l)
        .unwrap_or_else(|| l.fallback().to_string())
}

#[cfg(test)]
mod tests;
