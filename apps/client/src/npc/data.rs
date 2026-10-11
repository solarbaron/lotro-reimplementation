// SPDX-License-Identifier: GPL-3.0-or-later
//! The install facts the NPC windows read (decision 0024 P3): `tessera-gamedata`'s NPC
//! window model ([`NpcPresenter`]: dialogue texts, offers, entries, coins, the wallet)
//! over the P1 facts ([`CharacterData`]: the record store, the strings, the tooltip
//! engine), plus the per-record values the client needs besides: the wallet views'
//! `WalletItem` values (`lua-plugin-api.md` §15.3), the use range (`interaction.md`
//! §3.4.1) and the sale flags of the sell list (`npc-windows.md` §7.6 rule 1,
//! `item-tooltips.md` §6.11 step 2).
//!
//! Built once per run, off the frame ([`NpcData::new`]). The presentation data borrows the
//! P1 store, which lives for the run (one per opened install, never freed), and is itself
//! allocated once per [`NpcData`] and never freed, so the presenter's caches (currencies,
//! wallet catalog, action entries) are read once.

use std::sync::Arc;

use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_gamedata::npc::{InstallPresentationData, NpcPresenter, PresentationData};
use tessera_gamedata::tooltip::TooltipData;
use tessera_gamedata::tooltip::rules::SaleFlags;
use tessera_plugins::WalletItemData;
use tessera_text::Values;

use crate::character::CharacterData;

/// Errors of [`NpcData::new`].
#[derive(Debug, thiserror::Error)]
pub enum NpcDataError {
    /// The presentation data (dictionary, labels, string tables) cannot be read.
    #[error("presentation data: {0}")]
    Presentation(#[from] tessera_gamedata::tooltip::TooltipError),
    /// The archives cannot be opened or a UI table is missing.
    #[error("UI data: {0}")]
    Ui(String),
}

/// Properties the client reads besides the gamedata model (`None`: not in the dictionary).
#[derive(Debug, Clone, Copy, Default)]
struct Pids {
    small_icon: Option<u32>,
    bind_account: Option<u32>,
    max_quantity: Option<u32>,
    icon_image: Option<u32>,
    name: Option<u32>,
    description: Option<u32>,
    min_usage: Option<u32>,
    value: Option<u32>,
    force_not_sellable: Option<u32>,
    quest_item: Option<u32>,
    indestructible: Option<u32>,
    barter_item: Option<u32>,
    legendary: Option<u32>,
    store_item: Option<u32>,
}

impl Pids {
    fn resolve(pid: &dyn Fn(&str) -> Option<u32>) -> Self {
        Self {
            small_icon: pid("PaperItem_SmallIcon"),
            bind_account: pid("Inventory_BindToAccount"),
            max_quantity: pid("Inventory_MaxQuantity"),
            icon_image: pid("Icon_Layer_ImageDID"),
            name: pid("Name"),
            description: pid("Description"),
            min_usage: pid("Usage_MinUsageDist"),
            value: pid("Item_Value"),
            force_not_sellable: pid("Item_ForceNotSellable"),
            quest_item: pid("Usage_IsQuestItem"),
            indestructible: pid("Inventory_Indestructible"),
            barter_item: pid("Barter_IsBarterItem"),
            legendary: pid("ItemAdvancement_Item"),
            store_item: pid("Item_IsPurchasedWebStoreItem"),
        }
    }
}

/// The NPC windows' install facts (module docs).
pub struct NpcData {
    character: Arc<CharacterData>,
    data: &'static InstallPresentationData<'static>,
    presenter: NpcPresenter<'static>,
    pids: Pids,
    /// `Usage_MinUsageDist` by record, including the records that have none or cannot be
    /// read (at most [`MAX_USAGE_RANGES`]).
    usage: std::sync::Mutex<std::collections::HashMap<u32, Option<f32>>>,
}

/// Most use ranges remembered by [`NpcData::min_usage_dist`] (Tessera bound; the map is
/// emptied when full).
pub const MAX_USAGE_RANGES: usize = 4096;

impl std::fmt::Debug for NpcData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcData").finish_non_exhaustive()
    }
}

/// What the sell list needs of an item record (`npc-windows.md` §7.6 rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaleInfo {
    /// The vendor rules let the item be sold (`items.md` §5.3).
    pub sellable: bool,
    /// `Item_Value` (the worth of one unit, `item-tooltips.md` §6.11).
    pub value: Option<i32>,
}

fn int_of(v: Option<&PropertyValue>) -> Option<i64> {
    match v? {
        PropertyValue::Int(i) => Some(i64::from(*i)),
        PropertyValue::Int64(i) => Some(*i),
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(i64::from(*k)),
        _ => None,
    }
}

fn bool_of(v: Option<&PropertyValue>) -> bool {
    match v {
        Some(PropertyValue::Bool(b)) => *b,
        Some(PropertyValue::TriState(t)) => *t == 1,
        Some(PropertyValue::Int(i)) => *i != 0,
        _ => false,
    }
}

fn did_of(v: Option<&PropertyValue>) -> u32 {
    match v {
        Some(PropertyValue::DidRef(d)) => d.0,
        _ => 0,
    }
}

impl NpcData {
    /// The NPC facts over the P1 facts of the same install.
    ///
    /// # Errors
    /// [`NpcDataError::Presentation`] when the dictionary or the labels cannot be read.
    pub fn new(character: Arc<CharacterData>) -> Result<Self, NpcDataError> {
        let data =
            InstallPresentationData::new(character.store(), Arc::clone(character.strings()))?;
        let data: &'static InstallPresentationData<'static> = Box::leak(Box::new(data));
        let presenter = NpcPresenter::new(data);
        let pids = Pids::resolve(&|n| data.pid(n));
        Ok(Self {
            character,
            data,
            presenter,
            pids,
            usage: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// The P1 facts (store, strings, tooltip engine, item facts).
    #[must_use]
    pub fn character(&self) -> &Arc<CharacterData> {
        &self.character
    }

    /// `tessera-gamedata`'s NPC window model.
    #[must_use]
    pub fn presenter(&self) -> &NpcPresenter<'static> {
        &self.presenter
    }

    /// The presentation data (records, strings, labels).
    #[must_use]
    pub fn data(&self) -> &'static dyn PresentationData {
        self.data
    }

    /// A record's property set (with inheritance); `None` when absent or unreadable.
    #[must_use]
    pub fn record(&self, did: Did) -> Option<Arc<PropertySet>> {
        self.data.record(did).ok().flatten()
    }

    fn render(&self, info: &StringInfo) -> String {
        self.data
            .render(
                tessera_gamedata::tooltip::TextRef::Info(info),
                &Values::new(),
            )
            .ok()
            .flatten()
            .map(|t| t.plain_text())
            .unwrap_or_default()
    }

    fn text_of(&self, set: &PropertySet, pid: Option<u32>) -> String {
        match pid.and_then(|p| set.get(p)) {
            Some(PropertyValue::StringInfo(i)) => self.render(i),
            Some(PropertyValue::String(s)) => s.clone(),
            _ => String::new(),
        }
    }

    /// The `WalletItem` values of a wallet item record (`lua-plugin-api.md` §15.3 getter
    /// table): rendered `Name` and `Description`, `Icon_Layer_ImageDID`,
    /// `PaperItem_SmallIcon`, `PaperItemControl_CapDefault` of its `PaperItemControl` entry
    /// when it has one else `Inventory_MaxQuantity`, `Inventory_BindToAccount`; empty, 0 or
    /// false for what the record lacks (and for an unreadable record).
    #[must_use]
    pub fn wallet_item(&self, did: Did) -> WalletItemData {
        let mut out = WalletItemData {
            record: Some(did.0),
            ..WalletItemData::default()
        };
        let Some(set) = self.record(did) else {
            return out;
        };
        let p = &self.pids;
        out.name = self.text_of(&set, p.name);
        out.description = self.text_of(&set, p.description);
        out.image = did_of(p.icon_image.and_then(|q| set.get(q)));
        out.small_image = did_of(p.small_icon.and_then(|q| set.get(q)));
        out.account_item = bool_of(p.bind_account.and_then(|q| set.get(q)));
        let cap = self.presenter.wallet_catalog().get(did).and_then(|w| w.cap);
        let max = cap.or_else(|| int_of(p.max_quantity.and_then(|q| set.get(q))));
        out.max_quantity = max.and_then(|m| u64::try_from(m).ok()).unwrap_or(0);
        out
    }

    /// `Usage_MinUsageDist` of a record (`interaction.md` §3.4.1 step 3), when stored.
    /// Remembered per record, also when the record has none or cannot be read.
    #[must_use]
    pub fn min_usage_dist(&self, did: Did) -> Option<f32> {
        let usage = || {
            self.usage
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        };
        if let Some(v) = usage().get(&did.0) {
            return *v;
        }
        let v = self
            .pids
            .min_usage
            .and_then(|pid| match self.record(did)?.get(pid)? {
                PropertyValue::Float(f) => Some(*f),
                _ => None,
            });
        let mut m = usage();
        if m.len() >= MAX_USAGE_RANGES {
            m.clear();
        }
        m.insert(did.0, v);
        v
    }

    /// The sale flags and value of an item record (`item-tooltips.md` §6.11 step 2,
    /// `items.md` §5.3). Tessera's reading of "the sale value is 0": the record's
    /// `Item_Value` is absent or 0 (the wear-state factor of an instance is not applied).
    #[must_use]
    pub fn sale(&self, did: Did) -> Option<SaleInfo> {
        let set = self.record(did)?;
        let p = &self.pids;
        let flag = |pid: Option<u32>| bool_of(pid.and_then(|q| set.get(q)));
        let value = int_of(p.value.and_then(|q| set.get(q))).and_then(|v| i32::try_from(v).ok());
        let flags = SaleFlags {
            force_not_sellable: flag(p.force_not_sellable),
            zero_sale_value: value.is_none_or(|v| v == 0),
            quest_item: flag(p.quest_item),
            indestructible: flag(p.indestructible),
            barter_item: flag(p.barter_item),
            legendary: p.legendary.is_some_and(|q| set.get(q).is_some()),
            store_item: flag(p.store_item),
        };
        Some(SaleInfo {
            sellable: flags.sellable(),
            value,
        })
    }

    /// The colour of a name-colour state (`item-tooltips.md` §6.2), `0xAARRGGBB`.
    #[must_use]
    pub fn name_colour(&self, state: &str) -> Option<u32> {
        self.character.engine().name_colour(state)
    }

    /// The coin currencies' values, largest first (`npc-windows.md` §9.3).
    #[must_use]
    pub fn coin_values(&self) -> Vec<u64> {
        self.presenter
            .currencies()
            .units
            .iter()
            .filter_map(|u| u64::try_from(u.value).ok())
            .collect()
    }
}
