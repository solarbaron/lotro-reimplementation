// SPDX-License-Identifier: GPL-3.0-or-later
//! The NPC windows of decision 0024 P3 (`docs/specs/npc-windows.md`): dialogue (the
//! conversation, choice and comment windows), the quest offer with its objective and
//! reward rows, the vendor window with its quantity dialog, sell and buy-back pages, the
//! barter window, the loot window, money displays and the wallet.
//!
//! `tessera-ui` links neither the session layer nor the data layer. The client turns the
//! feed's events (`DialogueShown`, `QuestOffered`, `VendorShown`, `LootShown`, …) into the
//! plain view structs of these modules (texts already rendered, icons as image references,
//! prices as coin counts) and calls each window's `show`; it passes every
//! [`crate::UiEvent`] to the window's `handle_event`, which returns [`NpcAction`]s: the
//! session commands of `npc-windows.md` §10.1 that the client sends. Labels the client
//! renders from the install's string tables come in through [`NpcTexts`].
//!
//! Every window works on two kinds of element tree with the same element IDs
//! ([`super::NpcVocab`]): the install's layout (opened by the client by `UILAYOUT` label,
//! `npc-windows.md` §4) and Tessera's fallback widgets ([`fallback`], decision 0031 tier 1
//! goldens and runs without an install). Rows are created from install templates the
//! client names ([`super::examine::ExamTemplate`]) or, without one, as fallback rows.
//!
//! The client never hides a feed window before `WindowClosed` arrives (§5.7): a close
//! button or Escape only sends [`NpcAction::CloseWindow`].
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`coins`] | coin split, coin visibility, the money display | NW §9.2, §9.3 |
//! | [`dialogue`] | window choice, conversation, choice and comment windows | NW §5 |
//! | [`offer`] | quest offer: header, offer line, objective and reward blocks | NW §6, §10.3 |
//! | [`vendor`] | vendor window: pages, entries, buy modes, selling, buy-back | NW §7.1–§7.6, §7.8 |
//! | [`quantity`] | the quantity dialog | NW §7.5 |
//! | [`barter`] | the barter window | NW §7.7 |
//! | [`loot`] | the loot window | NW §8 |
//! | [`wallet`] | the wallet window | NW §9.4 |
//! | [`fallback`] | fallback windows built from code-made elements | decision 0031 |
//! | [`kit`] | fallback art, shared helpers | — |

pub mod barter;
pub mod coins;
pub mod dialogue;
pub mod fallback;
pub mod kit;
pub mod loot;
pub mod offer;
pub mod quantity;
pub mod vendor;
pub mod wallet;

pub use coins::{CoinCounts, CoinKind, MoneyDisplay, MoneyOptions};
pub use kit::{FallbackArt, keep_on_screen};

/// A feed window (`scenario-feed.md` §5.6 `WindowId`).
pub type WindowId = u32;

/// A user action in an NPC window: the session command the client sends
/// (`npc-windows.md` §10.1; `scenario-feed.md` §9.4 `NpcCommand`), or, for
/// [`NpcAction::FontSize`], a local setting the client keeps (never sent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcAction {
    /// `ChooseOption { window, option }` (§5.3).
    ChooseOption {
        /// The dialogue window.
        window: WindowId,
        /// The option number.
        option: u8,
    },
    /// `CloseWindow { window }` (every window).
    CloseWindow {
        /// The window.
        window: WindowId,
    },
    /// `AnswerQuest { window, accept }` (§6.5).
    AnswerQuest {
        /// The quest-offer window.
        window: WindowId,
        /// Accept or decline.
        accept: bool,
    },
    /// `Buy { window, entry, quantity }` (§7.4, §7.5, §7.7).
    Buy {
        /// The vendor or barter window.
        window: WindowId,
        /// The entry number.
        entry: u16,
        /// How many.
        quantity: u32,
    },
    /// `Sell { window, item, quantity }` (§7.6).
    Sell {
        /// The vendor window.
        window: WindowId,
        /// The client's item reference.
        item: u64,
        /// How many (the item's quantity).
        quantity: u32,
    },
    /// `BuyBack { window, entry }` (§7.8).
    BuyBack {
        /// The vendor window.
        window: WindowId,
        /// The buy-back entry number.
        entry: u16,
    },
    /// `TakeLoot { window, entry }` (§8.3); `None` takes everything.
    TakeLoot {
        /// The loot window.
        window: WindowId,
        /// The entry, `None` for all.
        entry: Option<u16>,
    },
    /// Not a session command: the font-size arrows of the conversation (or quest-offer)
    /// window chose a new font index (§5.2 Tessera rule, `fonts.md` §9.2); the client keeps
    /// it in Tessera's settings and restores it with
    /// [`dialogue::DialogueWindow::set_font_index`] (or
    /// [`offer::OfferWindow::set_font_index`]).
    FontSize {
        /// The font index (0 = the first font of the text pane's list).
        index: u32,
    },
}

/// A fixed label of the NPC windows. The install's layouts carry their own; the client
/// gives the ones game code sets ([`NpcTexts::label`]) and the fallback widgets show
/// either the client's text or Tessera's own short text ([`NpcLabel::fallback`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NpcLabel {
    /// The loot window's button while an entry is listed (`UI` token 0x0A5A16DC, §8.3).
    LootAll,
    /// The loot window's button with no entry listed (`UI` token 0x02E15B29, §8.3).
    LootAllEmpty,
    /// The loot window's title.
    LootTitle,
    /// Continue (§5.2).
    Continue,
    /// A close button's text.
    Close,
    /// Accept (§6.5).
    Accept,
    /// Decline (§6.5).
    Decline,
    /// The objective header (`QuestObjectives_Header`, §6.2).
    ObjectivesHeader,
    /// The rewards header (`QuestRewards_Header`, §6.3).
    RewardsHeader,
    /// The selectable-rewards header (§6.3).
    SelectableHeader,
    /// The level label (`Quest_LevelLabel`, §6.1).
    Level,
    /// The vendor window's title.
    VendorTitle,
    /// The Buy tab.
    TabBuy,
    /// The Sell tab.
    TabSell,
    /// The Buy-back tab.
    TabBuyBack,
    /// The Buy button.
    Buy,
    /// The Sell button.
    Sell,
    /// The Sell All button.
    SellAll,
    /// The Buy-back button.
    BuyBack,
    /// The player's money label.
    YourMoney,
    /// The sell value label.
    SellValue,
    /// The quantity dialog's question (`HowManyText`).
    HowMany,
    /// OK.
    Ok,
    /// Cancel.
    Cancel,
    /// The Full preset.
    Full,
    /// The Half preset.
    Half,
    /// The third preset (sets 1).
    One,
    /// The barter window's title.
    BarterTitle,
    /// The wallet window's title.
    WalletTitle,
    /// The wallet's money row label (`MoneyDisplay_Label`).
    Coins,
    /// The wallet's search label.
    Search,
    /// Expand all.
    ExpandAll,
    /// Collapse all.
    CollapseAll,
    /// The stack hint of the buy page for a buy mode (§7.4).
    StackHint(vendor::BuyMode),
}

impl NpcLabel {
    /// Tessera's own short text, used by the fallback widgets when the client gives none.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        use vendor::BuyMode as M;
        match self {
            Self::LootAll => "Take all",
            Self::LootAllEmpty | Self::Close => "Close",
            Self::LootTitle => "Loot",
            Self::Continue => "Continue",
            Self::Accept => "Accept",
            Self::Decline => "Decline",
            Self::ObjectivesHeader => "Objective",
            Self::RewardsHeader => "Rewards",
            Self::SelectableHeader => "Choose one of",
            Self::Level => "Level",
            Self::VendorTitle => "Vendor",
            Self::TabBuy | Self::Buy => "Buy",
            Self::TabSell | Self::Sell => "Sell",
            Self::TabBuyBack | Self::BuyBack => "Buy back",
            Self::SellAll => "Sell all",
            Self::YourMoney => "Your money",
            Self::SellValue => "Value",
            Self::HowMany => "How many?",
            Self::Ok => "OK",
            Self::Cancel => "Cancel",
            Self::Full => "Max",
            Self::Half => "Half",
            Self::One => "1",
            Self::BarterTitle => "Barter",
            Self::WalletTitle => "Wallet",
            Self::Coins => "Coins",
            Self::Search => "Search",
            Self::ExpandAll => "+",
            Self::CollapseAll => "-",
            Self::StackHint(M::Single) => "Shift: a stack. Alt: choose a quantity.",
            Self::StackHint(M::Stack) => "Buying a full stack.",
            Self::StackHint(M::Quantity) => "Choose a quantity.",
            Self::StackHint(M::Fill) => "Filling your partial stacks.",
        }
    }
}

/// Texts the NPC windows ask the client for (rendered from the install's string tables;
/// the defaults are Tessera's own).
pub trait NpcTexts {
    /// A fixed label; `None` keeps the layout's own text (and gives the fallback widgets
    /// [`NpcLabel::fallback`]).
    fn label(&self, label: NpcLabel) -> Option<String> {
        let _ = label;
        None
    }

    /// A coin element's count (`UI` token 0x07480C44 with the count, §9.2); default the
    /// decimal integer.
    fn coin_count(&self, count: u64) -> String {
        count.to_string()
    }

    /// A coin icon's tooltip (`UI` tokens of §9.2); default none.
    fn coin_tooltip(&self, kind: CoinKind) -> Option<String> {
        let _ = kind;
        None
    }

    /// An amount shown as a decimal integer (reward rows, wallet totals, quantities).
    fn amount(&self, amount: i64) -> String {
        amount.to_string()
    }

    /// A coin amount as one line of text (a barter coin cost beyond the three cost slots,
    /// listed in the reward's tooltip, §7.7 Tessera rule). Default (Tessera's own): the
    /// non-zero coins, largest first, as "*n* gold *n* silver *n* copper"; "0 copper" for
    /// no coins.
    fn coins_text(&self, coins: CoinCounts) -> String {
        let parts: Vec<String> = [
            (coins.gold, "gold"),
            (coins.silver, "silver"),
            (coins.copper, "copper"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| format!("{n} {w}"))
        .collect();
        if parts.is_empty() {
            "0 copper".to_string()
        } else {
            parts.join(" ")
        }
    }
}

/// [`NpcTexts`] with every default (Tessera's own texts).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultTexts;

impl NpcTexts for DefaultTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn label_text(texts: &dyn NpcTexts, label: NpcLabel) -> String {
    texts
        .label(label)
        .unwrap_or_else(|| label.fallback().to_string())
}

/// The window kinds of the NPC windows, as the client's `--open-panel` names them
/// (`npc-windows.md` §4 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NpcWindowKind {
    /// `conversation`.
    Conversation,
    /// `choice`.
    Choice,
    /// `npc-comment`.
    Comment,
    /// `vendor`.
    Vendor,
    /// `vendor-quantity`.
    Quantity,
    /// `barter`.
    Barter,
    /// `loot`.
    Loot,
    /// `wallet`.
    Wallet,
}

impl NpcWindowKind {
    /// The `--open-panel` name.
    #[must_use]
    pub fn panel_name(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Choice => "choice",
            Self::Comment => "npc-comment",
            Self::Vendor => "vendor",
            Self::Quantity => "vendor-quantity",
            Self::Barter => "barter",
            Self::Loot => "loot",
            Self::Wallet => "wallet",
        }
    }

    /// The kind of an `--open-panel` name.
    #[must_use]
    pub fn from_panel_name(name: &str) -> Option<Self> {
        [
            Self::Conversation,
            Self::Choice,
            Self::Comment,
            Self::Vendor,
            Self::Quantity,
            Self::Barter,
            Self::Loot,
            Self::Wallet,
        ]
        .into_iter()
        .find(|k| k.panel_name() == name)
    }
}
