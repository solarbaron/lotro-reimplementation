// SPDX-License-Identifier: GPL-3.0-or-later
//! The NPC windows on the game UI (`npc-windows.md` §4–§9): one native-window instance per
//! open feed window (§4 Tessera rule (instances)), opened from the install's layout by
//! label ([`NpcRefs`]) or as Tessera's fallback widgets, attached to `tessera-ui`'s P3
//! controllers and filled from the session state ([`super::present`]).
//!
//! Each frame the client calls [`NpcUi::frame`] with the player state, the UI events of
//! the frame and, when it has one, the world geometry; it returns the [`NpcAction`]s to
//! send (mapped by [`super::commands::command_of`]):
//!
//! 1. **sync**: windows the state no longer holds are destroyed (`WindowClosed`, §5.7);
//!    new ones open; a window whose revision changed is filled again (a `LootTaken`
//!    removes only its row, so a pending Loot All survives, §8.3); a dialogue page that
//!    needs another native window replaces the old instance at its position, centre to
//!    centre (§5.1); the vendor window's money and sell list follow the wallet and the
//!    backpack;
//! 2. **events**: every UI event goes to each window's controller; hovering an item shows
//!    its examination tooltip (`item-tooltips.md` §3, §9) with P1's examination panel; the
//!    vendor's quantity mode opens the quantity dialog (§7.5);
//! 3. **end of frame**: Loot All's close (§8.3), and one `CloseWindow` for a window whose
//!    entity despawned or moved out of use range (§5.7 Tessera rules).
//!
//! `--open-panel` names (`conversation`, `choice`, `npc-comment`, `vendor`,
//! `vendor-quantity`, `barter`, `loot`, `wallet`, §4 Tessera rule) open the windows empty
//! ([`NpcUi::open_panel`]); the wallet window, opened that way or by
//! [`NpcUi::toggle_wallet`], shows the wallet (§9.4).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tessera_formats::Did;
use tessera_gamedata::npc::NpcCensus;
use tessera_gamedata::tooltip::{Instance, Purpose, Subject, TooltipRequest};
use tessera_net::EntityId;
use tessera_net::domain::Currency;
use tessera_ui::game::examine::{ExamKey, ExamTooltips};
use tessera_ui::game::npc::barter::{BarterWindow, page_buttons};
use tessera_ui::game::npc::dialogue::{DialogueKind, DialogueWindow, centre_on};
use tessera_ui::game::npc::loot::{LootItem, LootWindow};
use tessera_ui::game::npc::offer::{OfferWindow, RewardRow};
use tessera_ui::game::npc::quantity::{QuantityDialog, QuantityOutcome, QuantityRequest};
use tessera_ui::game::npc::vendor::{BuyToggles, VendorOutcome, VendorPage, VendorWindow};
use tessera_ui::game::npc::wallet::WalletWindow;
use tessera_ui::game::npc::{FallbackArt, NpcAction, NpcTexts, NpcWindowKind, fallback};
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::commands::UseGeometry;
use super::data::NpcData;
use super::present::{Present, VendorView};
use super::refs::NpcRefs;
use super::state::{FeedWindow, WindowContent};
use super::texts::InstallTexts;
use crate::character::PlayerState;

/// Most windows opened empty by name at once (`--open-panel`; Tessera bound).
pub const MAX_PANELS: usize = 16;
/// Most distinct reasons kept in [`NpcUi::failures`] (Tessera bound).
pub const MAX_FAILURES: usize = 64;
/// Most action-template icons remembered (Tessera bound; the install has 32 entries).
const MAX_ACTION_ICONS: usize = 256;

/// One open feed window's controller (the controllers are large; each instance lives in
/// a box of its own).
#[allow(clippy::large_enum_variant)]
enum Win {
    Dialogue(DialogueWindow),
    Offer {
        win: OfferWindow,
        rewards: Vec<RewardRow>,
    },
    Vendor {
        win: VendorWindow,
        quantity: Option<(ElementKey, QuantityDialog)>,
        sell_key: u64,
        coin: Option<u64>,
        /// The buy toggles last given to the window (`None`: give them again).
        toggles: Option<BuyToggles>,
    },
    Barter(BarterWindow),
    Loot {
        win: LootWindow,
        items: Vec<LootItem>,
        /// The feed content ([`FeedWindow::shown`]) the rows were filled from; `None`
        /// before the first fill.
        filled: Option<u64>,
        taken: BTreeSet<u16>,
    },
}

impl Win {
    fn native(&self) -> NpcWindowKind {
        match self {
            Self::Dialogue(d) => match d.kind {
                DialogueKind::Conversation => NpcWindowKind::Conversation,
                DialogueKind::Choice => NpcWindowKind::Choice,
                DialogueKind::Comment => NpcWindowKind::Comment,
            },
            Self::Offer { .. } => NpcWindowKind::Conversation,
            Self::Vendor { .. } => NpcWindowKind::Vendor,
            Self::Barter(_) => NpcWindowKind::Barter,
            Self::Loot { .. } => NpcWindowKind::Loot,
        }
    }

    fn detach(&mut self, ui: &mut Ui) {
        match self {
            Self::Dialogue(d) => d.detach(ui),
            Self::Offer { win, .. } => win.detach(ui),
            Self::Vendor { win, quantity, .. } => {
                win.detach(ui);
                if let Some((k, mut q)) = quantity.take() {
                    q.detach(ui);
                    ui.destroy(k);
                }
            }
            Self::Barter(b) => b.detach(ui),
            Self::Loot { win, .. } => win.detach(ui),
        }
    }
}

struct Open {
    revision: u64,
    /// Whether the window was filled with the install's facts (it is filled again when
    /// they arrive later).
    with_data: bool,
    root: ElementKey,
    win: Box<Win>,
    entity: Option<EntityId>,
}

/// A window opened empty by name.
struct Panel {
    kind: NpcWindowKind,
    root: ElementKey,
    wallet: Option<WalletWindow>,
    seen: Option<Vec<(Currency, u64)>>,
}

/// The NPC windows on a UI (module docs).
pub struct NpcUi {
    refs: Arc<NpcRefs>,
    art: FallbackArt,
    windows: BTreeMap<u32, Open>,
    panels: Vec<Panel>,
    tips: ExamTooltips,
    census: NpcCensus,
    auto_loot: BTreeSet<EntityId>,
    closes_sent: BTreeSet<u32>,
    font: Option<u32>,
    registered: bool,
    failures: Vec<String>,
    failures_dropped: u64,
    /// Feed windows whose instance could not be opened, with the revision tried (not tried
    /// again until the revision changes).
    failed: BTreeMap<u32, u64>,
    /// Whether the last sync had the install's facts.
    had_data: bool,
    /// The icons of the choice rows' action templates (§5.3 rule 3), by template ID.
    action_icons: BTreeMap<u32, Option<tessera_ui::ImageRef>>,
}

impl std::fmt::Debug for NpcUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcUi")
            .field("windows", &self.windows.len())
            .field("panels", &self.panels.len())
            .finish_non_exhaustive()
    }
}

/// A fingerprint of the backpack (the sell list is rebuilt when it changes).
fn backpack_key(state: &PlayerState) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for it in state
        .inventory()
        .in_container(tessera_gamedata::inventory::Container::Backpack)
    {
        (it.id, it.place.slot, it.quantity).hash(&mut h);
    }
    h.finish()
}

fn coin_of(state: &PlayerState) -> Option<u64> {
    state.coin()
}

impl NpcUi {
    /// The windows over `refs` (the install's names, or [`NpcRefs::fallback`]) with the
    /// fallback widgets' `art`.
    #[must_use]
    pub fn new(refs: Arc<NpcRefs>, art: FallbackArt) -> Self {
        let mut tips = ExamTooltips::new(refs.char_vocab.clone());
        if let Some(l) = refs.examination {
            tips.set_pin_layout(l.0);
        }
        Self {
            refs,
            art,
            windows: BTreeMap::new(),
            panels: Vec::new(),
            tips,
            census: NpcCensus::default(),
            auto_loot: BTreeSet::new(),
            closes_sent: BTreeSet::new(),
            font: None,
            registered: false,
            failures: Vec::new(),
            failures_dropped: 0,
            failed: BTreeMap::new(),
            had_data: false,
            action_icons: BTreeMap::new(),
        }
    }

    /// Records why a window could not be opened: logged and kept once per distinct
    /// reason, at most [`MAX_FAILURES`] reasons (further ones are counted).
    fn fail(&mut self, why: String) {
        if self.failures.contains(&why) {
            return;
        }
        if self.failures.len() >= MAX_FAILURES {
            self.failures_dropped = self.failures_dropped.saturating_add(1);
            return;
        }
        tracing::warn!("{why}");
        self.failures.push(why);
    }

    /// Failures not kept because [`MAX_FAILURES`] distinct reasons were already kept.
    #[must_use]
    pub fn failures_dropped(&self) -> u64 {
        self.failures_dropped
    }

    /// The names in use.
    #[must_use]
    pub fn refs(&self) -> &Arc<NpcRefs> {
        &self.refs
    }

    /// What the windows rendered so far, by the census categories of `npc-windows.md` §11.
    #[must_use]
    pub fn census(&self) -> &NpcCensus {
        &self.census
    }

    /// Windows that could not be opened, with the reason (`LayoutMissing`,
    /// `ElementMissing` of §11 are failures).
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// The root of open feed window `window`.
    #[must_use]
    pub fn root(&self, window: u32) -> Option<ElementKey> {
        self.windows.get(&window).map(|o| o.root)
    }

    /// The open feed windows: window, native window kind and root.
    pub fn open_windows(&self) -> impl Iterator<Item = (u32, NpcWindowKind, ElementKey)> + '_ {
        self.windows
            .iter()
            .map(|(w, o)| (*w, o.win.native(), o.root))
    }

    /// The vendor quantity dialog's root, when it is open for `window`.
    #[must_use]
    pub fn quantity_root(&self, window: u32) -> Option<ElementKey> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Vendor {
                quantity: Some((k, q)),
                ..
            } if q.open => Some(*k),
            _ => None,
        }
    }

    /// The windows opened empty by name: kind and root.
    pub fn panels(&self) -> impl Iterator<Item = (NpcWindowKind, ElementKey)> + '_ {
        self.panels.iter().map(|p| (p.kind, p.root))
    }

    /// The vendor window of feed window `window`, when it shows one (tests, sheets).
    #[must_use]
    pub fn vendor(&self, window: u32) -> Option<&VendorWindow> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Vendor { win, .. } => Some(win),
            _ => None,
        }
    }

    /// The loot window of feed window `window`, when it shows one (tests, sheets).
    #[must_use]
    pub fn loot(&self, window: u32) -> Option<&LootWindow> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Loot { win, .. } => Some(win),
            _ => None,
        }
    }

    /// The entries the rows of loot window `window` show (tests, sheets).
    #[must_use]
    pub fn loot_items(&self, window: u32) -> Option<&[LootItem]> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Loot { items, .. } => Some(items),
            _ => None,
        }
    }

    /// The dialogue window of feed window `window`, when it shows one.
    #[must_use]
    pub fn dialogue(&self, window: u32) -> Option<&DialogueWindow> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Dialogue(d) => Some(d),
            _ => None,
        }
    }

    /// The quest-offer window of feed window `window`, when it shows one.
    #[must_use]
    pub fn offer(&self, window: u32) -> Option<&OfferWindow> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Offer { win, .. } => Some(win),
            _ => None,
        }
    }

    /// The barter window of feed window `window`, when it shows one.
    #[must_use]
    pub fn barter(&self, window: u32) -> Option<&BarterWindow> {
        match self.windows.get(&window)?.win.as_ref() {
            Win::Barter(b) => Some(b),
            _ => None,
        }
    }

    /// Shows `page` of vendor window `window` (the tabs do the same; tests and sheets).
    /// Returns whether the window is a vendor window.
    pub fn show_vendor_page(&mut self, ui: &mut Ui, window: u32, page: VendorPage) -> bool {
        match self.windows.get_mut(&window).map(|o| o.win.as_mut()) {
            Some(Win::Vendor { win, .. }) => {
                win.show_page(ui, page);
                true
            }
            _ => false,
        }
    }

    /// Opens the quantity dialog of vendor window `window` for buy entry `entry` with the
    /// entry's stack size as its maximum (§7.4 case 2, as the quantity mode does).
    pub fn ask_quantity(
        &mut self,
        ui: &mut Ui,
        window: u32,
        entry: u16,
        state: &PlayerState,
        data: Option<&NpcData>,
    ) -> bool {
        let Some(max) = self
            .vendor(window)
            .and_then(|v| v.entry(entry))
            .map(|e| e.stack_size.max(1))
        else {
            return false;
        };
        let texts = InstallTexts::new(data);
        let present = Present::new(data, state);
        self.open_quantity(ui, window, entry, max, &present, &texts);
        self.quantity_root(window).is_some()
    }

    /// Marks `entity`'s next loot window for Loot All (`AutoLootAll`, `npc-windows.md`
    /// §3.1, §8.3; P2's input calls this at merge).
    pub fn mark_auto_loot(&mut self, entity: EntityId) {
        if self.auto_loot.len() < 1024 {
            self.auto_loot.insert(entity);
        }
    }

    fn register(&mut self, ui: &mut Ui) {
        if !self.registered {
            self.refs.nv.register_classes(ui);
            self.registered = true;
        }
    }

    /// Opens the native window `kind`: the install's layout, else the fallback widgets.
    /// Returns the root and, for the barter and wallet windows, their extra buttons.
    fn open_native(
        &mut self,
        ui: &mut Ui,
        kind: NpcWindowKind,
        texts: &dyn NpcTexts,
    ) -> Result<(ElementKey, Option<ElementKey>, Option<ElementKey>), String> {
        self.register(ui);
        if let Some((layout, root)) = self.refs.window(kind) {
            match ui.open_panel(layout, root) {
                Ok(k) => {
                    let (a, b) = if kind == NpcWindowKind::Barter {
                        let pc = tessera_ui::game::find_below(ui, k, self.refs.nv.ids.barter_pages);
                        page_buttons(ui, &self.refs.nv, pc, &self.art)
                    } else {
                        (None, None)
                    };
                    return Ok((k, a, b));
                }
                Err(e) => {
                    let why = format!(
                        "{}: LayoutMissing ({e}); fallback window",
                        kind.panel_name()
                    );
                    self.fail(why);
                }
            }
        }
        let nv = &self.refs.nv;
        let art = &self.art;
        let r = match kind {
            NpcWindowKind::Conversation => {
                fallback::conversation(ui, nv, art, texts).map(|k| (k, None, None))
            }
            NpcWindowKind::Choice => fallback::choice(ui, nv, art, texts).map(|k| (k, None, None)),
            NpcWindowKind::Comment => {
                fallback::comment(ui, nv, art, texts).map(|k| (k, None, None))
            }
            NpcWindowKind::Vendor => fallback::vendor(ui, nv, art, texts).map(|k| (k, None, None)),
            NpcWindowKind::Quantity => {
                fallback::quantity(ui, nv, art, texts).map(|k| (k, None, None))
            }
            NpcWindowKind::Barter => fallback::barter(ui, nv, art, texts),
            NpcWindowKind::Loot => fallback::loot(ui, nv, art, texts).map(|k| (k, None, None)),
            NpcWindowKind::Wallet => fallback::wallet(ui, nv, art, texts),
        };
        r.map_err(|e| format!("{}: {e}", kind.panel_name()))
    }

    /// Opens window `kind` empty (`--open-panel`, §4 Tessera rule); the wallet window
    /// shows the wallet of `state`.
    ///
    /// # Errors
    /// When neither the layout nor the fallback window can be built.
    pub fn open_panel(
        &mut self,
        ui: &mut Ui,
        kind: NpcWindowKind,
        state: &PlayerState,
        data: Option<&NpcData>,
    ) -> Result<ElementKey, String> {
        if self.panels.len() >= MAX_PANELS {
            return Err(format!("{MAX_PANELS} windows open by name"));
        }
        let texts = InstallTexts::new(data);
        let (root, prev, next) = self.open_native(ui, kind, &texts)?;
        let nv = self.refs.nv.clone();
        let t = self.refs.templates;
        let wallet = match kind {
            NpcWindowKind::Conversation | NpcWindowKind::Choice | NpcWindowKind::Comment => {
                let dk = match kind {
                    NpcWindowKind::Conversation => DialogueKind::Conversation,
                    NpcWindowKind::Choice => DialogueKind::Choice,
                    _ => DialogueKind::Comment,
                };
                let mut d = DialogueWindow::attach(ui, &nv, root, dk, t.choice, &self.art);
                d.detach(ui);
                None
            }
            NpcWindowKind::Vendor => {
                let mut v = VendorWindow::attach(ui, &nv, root, t.vendor, &self.art);
                let present = Present::new(data, state);
                let coin = coin_of(state).unwrap_or(0);
                v.set_wallet(ui, coin, present.coins(coin), &texts);
                v.detach(ui);
                None
            }
            NpcWindowKind::Quantity => {
                let mut q = QuantityDialog::attach(ui, &nv, root, &self.art);
                q.detach(ui);
                None
            }
            NpcWindowKind::Barter => {
                let mut bw = BarterWindow::attach(ui, &nv, root, t.barter, &self.art);
                bw.set_page_buttons(ui, prev, next);
                bw.detach(ui);
                None
            }
            NpcWindowKind::Loot => {
                let mut l = LootWindow::attach(ui, &nv, root, t.loot, &self.art);
                l.detach(ui);
                None
            }
            NpcWindowKind::Wallet => {
                let mut w = WalletWindow::attach(ui, &nv, root, t.wallet, &self.art);
                w.set_buttons(ui, prev, next);
                Some(w)
            }
        };
        self.panels.push(Panel {
            kind,
            root,
            wallet,
            seen: None,
        });
        self.refresh_wallet(ui, state, data);
        Ok(root)
    }

    /// Opens the wallet window, or closes it when open (`TogglePaperItemPanel`,
    /// `Player_Wallet_Button`, §9.4).
    pub fn toggle_wallet(&mut self, ui: &mut Ui, state: &PlayerState, data: Option<&NpcData>) {
        if let Some(i) = self
            .panels
            .iter()
            .position(|p| p.kind == NpcWindowKind::Wallet)
        {
            let mut p = self.panels.remove(i);
            if let Some(w) = p.wallet.as_mut() {
                w.detach(ui);
            }
            ui.destroy(p.root);
            return;
        }
        if let Err(e) = self.open_panel(ui, NpcWindowKind::Wallet, state, data) {
            tracing::warn!("wallet window: {e}");
        }
    }

    fn refresh_wallet(&mut self, ui: &mut Ui, state: &PlayerState, data: Option<&NpcData>) {
        let texts = InstallTexts::new(data);
        for p in &mut self.panels {
            let Some(w) = p.wallet.as_mut() else { continue };
            if p.seen.as_deref() == Some(state.wallet()) {
                continue;
            }
            p.seen = Some(state.wallet().to_vec());
            let present = Present::new(data, state);
            let (coins, entries) = present.wallet(&mut self.census);
            w.show(ui, coins, &entries, &texts);
        }
    }

    /// One frame (module docs): sync with `state`, the UI `events`, the end-of-frame
    /// closes. Returns the actions to send, in order.
    pub fn frame(
        &mut self,
        ui: &mut Ui,
        state: &PlayerState,
        data: Option<&NpcData>,
        events: &[UiEvent],
        geometry: Option<&dyn UseGeometry>,
    ) -> Vec<NpcAction> {
        let mut out = Vec::new();
        self.sync(ui, state, data, &mut out);
        for ev in events {
            self.handle_event(ui, ev, state, data, &mut out);
        }
        self.end_frame(state, geometry, &mut out);
        // Font-size steps are local settings: applied to every dialogue window here.
        let mut sends = Vec::with_capacity(out.len());
        for a in out {
            if let NpcAction::FontSize { index } = a {
                self.font = Some(index);
                for o in self.windows.values_mut() {
                    match o.win.as_mut() {
                        Win::Dialogue(d) => {
                            d.set_font_index(ui, index);
                        }
                        Win::Offer { win, .. } => {
                            win.set_font_index(ui, index);
                        }
                        _ => {}
                    }
                }
            }
            sends.push(a);
        }
        sends
    }

    /// The font index the font-size arrows chose last (kept in Tessera's settings).
    #[must_use]
    pub fn font_index(&self) -> Option<u32> {
        self.font
    }

    fn destroy(&mut self, ui: &mut Ui, window: u32) {
        if let Some(mut o) = self.windows.remove(&window) {
            o.win.detach(ui);
            if ui.exists(o.root) {
                ui.destroy(o.root);
            }
        }
        self.closes_sent.remove(&window);
    }

    /// Step 1 of [`Self::frame`].
    pub fn sync(
        &mut self,
        ui: &mut Ui,
        state: &PlayerState,
        data: Option<&NpcData>,
        out: &mut Vec<NpcAction>,
    ) {
        let npc = state.npc();
        // The install's facts arrived: every window and wallet panel is filled again.
        let new_data = data.is_some() && !self.had_data;
        self.had_data = data.is_some();
        if new_data {
            for p in &mut self.panels {
                p.seen = None;
            }
        }
        if self.windows.is_empty()
            && self.failed.is_empty()
            && self.panels.is_empty()
            && npc.windows().next().is_none()
        {
            return;
        }
        let gone: Vec<u32> = self
            .windows
            .keys()
            .copied()
            .filter(|w| npc.window(tessera_net::domain::WindowId(*w)).is_none())
            .collect();
        for w in gone {
            self.destroy(ui, w);
        }
        self.failed
            .retain(|w, _| npc.window(tessera_net::domain::WindowId(*w)).is_some());
        let mut order: Vec<&FeedWindow> = npc.windows().collect();
        order.sort_by_key(|w| w.opened);
        for fw in order {
            let id = fw.id.0;
            if self
                .windows
                .get(&id)
                .is_some_and(|o| o.revision == fw.revision && o.with_data == data.is_some())
                || self.failed.get(&id) == Some(&fw.revision)
            {
                continue;
            }
            self.show(ui, fw, state, data, out);
        }
        // The vendor windows' money and sell lists follow the wallet and the backpack.
        if self
            .windows
            .values()
            .any(|o| matches!(*o.win, Win::Vendor { .. }))
        {
            self.sync_vendors(ui, state, data);
        }
        self.refresh_wallet(ui, state, data);
    }

    /// The vendor windows' money and sell lists (step 1, with a vendor window open).
    fn sync_vendors(&mut self, ui: &mut Ui, state: &PlayerState, data: Option<&NpcData>) {
        let coin = coin_of(state);
        let key = backpack_key(state);
        let texts = InstallTexts::new(data);
        let present = Present::new(data, state);
        for o in self.windows.values_mut() {
            if let Win::Vendor {
                win,
                sell_key,
                coin: shown,
                ..
            } = o.win.as_mut()
            {
                if *shown != coin {
                    *shown = coin;
                    let c = coin.unwrap_or(0);
                    win.set_wallet(ui, c, present.coins(c), &texts);
                }
                if *sell_key != key {
                    *sell_key = key;
                    win.set_sell_items(ui, &present.sell_list(), &texts);
                }
            }
        }
    }

    /// Opens or refills the instance of feed window `fw`.
    fn show(
        &mut self,
        ui: &mut Ui,
        fw: &FeedWindow,
        state: &PlayerState,
        data: Option<&NpcData>,
        out: &mut Vec<NpcAction>,
    ) {
        let id = fw.id.0;
        let texts = InstallTexts::new(data);
        let present = Present::new(data, state);
        let nv = self.refs.nv.clone();
        let t = self.refs.templates;
        // Which native window the content needs (§5.1, §7.3).
        let (native, view) = match &fw.content {
            WindowContent::Dialogue {
                speaker,
                text,
                options,
            } => {
                let (k, mut page, templates) =
                    present.dialogue(id, *speaker, text, options, &mut self.census);
                let native = match k {
                    DialogueKind::Conversation => NpcWindowKind::Conversation,
                    DialogueKind::Choice => NpcWindowKind::Choice,
                    DialogueKind::Comment => NpcWindowKind::Comment,
                };
                // §5.3 rule 3: an action row shows its action entry's template icon.
                for (option, template) in templates {
                    let icon = self.action_icon(ui, template);
                    if let Some(o) = page.options.iter_mut().find(|o| o.option == option) {
                        o.icon = icon;
                    }
                }
                (native, View::Dialogue(k, page))
            }
            WindowContent::QuestOffer {
                quest,
                giver,
                rewards,
            } => (
                NpcWindowKind::Conversation,
                View::Offer(present.offer(id, quest, *giver, rewards)),
            ),
            WindowContent::Vendor {
                entries, buyback, ..
            } => match present.vendor(entries, buyback.as_deref(), &mut self.census) {
                VendorView::Barter(list) => (NpcWindowKind::Barter, View::Barter(list)),
                VendorView::Vendor { items, buyback } => {
                    (NpcWindowKind::Vendor, View::Vendor(items, buyback))
                }
            },
            WindowContent::Loot {
                source,
                entries,
                coin,
                ..
            } => (
                NpcWindowKind::Loot,
                View::Loot(present.loot(id, *source, entries, *coin, &mut self.census)),
            ),
        };
        let is_offer = matches!(view, View::Offer(_));
        // Reuse the instance when its native window and role fit, else replace it.
        let reuse = self.windows.get(&id).is_some_and(|o| {
            o.win.native() == native && matches!(*o.win, Win::Offer { .. }) == is_offer
        });
        let old_root = (!reuse)
            .then(|| self.windows.get(&id).map(|o| o.root))
            .flatten();
        if !reuse {
            let entity = fw.content.entity();
            let (root, a, b) = match self.open_native(ui, native, &texts) {
                Ok(r) => r,
                Err(e) => {
                    // Not tried again until the window's content changes.
                    self.failed.insert(id, fw.revision);
                    self.fail(format!("NPC window: {e}"));
                    return;
                }
            };
            self.failed.remove(&id);
            if let Some(old) = old_root
                && ui.exists(old)
            {
                centre_on(ui, root, old);
            }
            self.destroy(ui, id);
            let win = match &view {
                View::Dialogue(k, _) => Win::Dialogue(DialogueWindow::attach(
                    ui, &nv, root, *k, t.choice, &self.art,
                )),
                View::Offer(_) => Win::Offer {
                    win: OfferWindow::attach(ui, &nv, root, t.offer, &self.art),
                    rewards: Vec::new(),
                },
                View::Vendor(..) => Win::Vendor {
                    win: VendorWindow::attach(ui, &nv, root, t.vendor, &self.art),
                    quantity: None,
                    sell_key: 0,
                    coin: None,
                    toggles: None,
                },
                View::Barter(_) => {
                    let mut b2w = BarterWindow::attach(ui, &nv, root, t.barter, &self.art);
                    b2w.set_page_buttons(ui, a, b);
                    // Coin costs use the install's money display (the loot window's coin
                    // row template, layout `moneydisplay`).
                    b2w.set_coin_template(t.loot.coin);
                    Win::Barter(b2w)
                }
                View::Loot(_) => Win::Loot {
                    win: LootWindow::attach(ui, &nv, root, t.loot, &self.art),
                    items: Vec::new(),
                    filled: None,
                    taken: BTreeSet::new(),
                },
            };
            ui.activate(root);
            self.windows.insert(
                id,
                Open {
                    revision: 0,
                    with_data: false,
                    root,
                    win: Box::new(win),
                    entity,
                },
            );
        }
        let auto = fw
            .content
            .entity()
            .is_some_and(|e| self.auto_loot.contains(&e));
        let font = self.font;
        let Some(o) = self.windows.get_mut(&id) else {
            return;
        };
        o.revision = fw.revision;
        // A refill for newly arrived facts redraws everything.
        let refill = o.with_data != data.is_some();
        o.with_data = data.is_some();
        o.entity = fw.content.entity();
        match (o.win.as_mut(), view) {
            (Win::Dialogue(d), View::Dialogue(_, page)) => {
                d.show(ui, &page, &texts);
                if let Some(f) = font {
                    d.set_font_index(ui, f);
                }
            }
            (Win::Offer { win, rewards }, View::Offer(v)) => {
                win.show(ui, &v, &texts);
                if let Some(f) = font {
                    win.set_font_index(ui, f);
                }
                *rewards = v.rewards;
            }
            (
                Win::Vendor {
                    win,
                    quantity,
                    sell_key,
                    coin,
                    toggles,
                },
                View::Vendor(items, buyback),
            ) => {
                win.show(ui, id, &items, &texts);
                // The new rows take the held toggles at the next frame.
                *toggles = None;
                if let Some(b) = buyback {
                    win.set_buyback(ui, &b, &texts);
                }
                let c = coin_of(state);
                win.set_wallet(ui, c.unwrap_or(0), present.coins(c.unwrap_or(0)), &texts);
                *coin = c;
                win.set_sell_items(ui, &present.sell_list(), &texts);
                *sell_key = backpack_key(state);
                if let Some((_, q)) = quantity.as_mut() {
                    q.cancel();
                }
            }
            (Win::Barter(b), View::Barter(list)) => b.show(ui, id, &list, &texts),
            (
                Win::Loot {
                    win,
                    items,
                    filled,
                    taken,
                },
                View::Loot(v),
            ) => {
                let WindowContent::Loot { taken: ft, .. } = &fw.content else {
                    return;
                };
                if !refill && *filled == Some(fw.shown) && ft.is_superset(taken) {
                    // Only entries were taken (§8.3): remove their rows, keep Loot All.
                    for e in ft.difference(taken).copied().collect::<Vec<_>>() {
                        win.taken(ui, e, &texts);
                    }
                } else {
                    // A first fill, or content that replaced the shown one (§8.2).
                    if let Some(a) = win.show(ui, &v, auto, &texts) {
                        out.push(a);
                    }
                    for e in ft {
                        win.taken(ui, *e, &texts);
                    }
                    *filled = Some(fw.shown);
                    *items = v.entries;
                }
                *taken = ft.clone();
                if auto && let Some(e) = fw.content.entity() {
                    self.auto_loot.remove(&e);
                }
            }
            _ => {}
        }
    }

    /// Step 2 of [`Self::frame`] for one event.
    fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        state: &PlayerState,
        data: Option<&NpcData>,
        out: &mut Vec<NpcAction>,
    ) {
        if let UiEvent::ExamineHover { element, pointer } = ev {
            self.hover(ui, *element, *pointer, state, data);
            return;
        }
        if let UiEvent::DragEnded { source, target, .. } = ev {
            out.extend(self.dragged(ui, *source, *target));
            return;
        }
        if self.windows.is_empty() && self.panels.is_empty() {
            return;
        }
        let texts = InstallTexts::new(data);
        let mut open_quantity: Option<(u32, u16, u32)> = None;
        for (id, o) in &mut self.windows {
            match o.win.as_mut() {
                Win::Dialogue(d) => out.extend(d.handle_event(ui, ev)),
                Win::Offer { win, .. } => out.extend(win.handle_event(ui, ev)),
                Win::Barter(b) => out.extend(b.handle_event(ui, ev, &texts)),
                Win::Loot { win, .. } => out.extend(win.handle_event(ui, ev)),
                Win::Vendor { win, quantity, .. } => {
                    if let Some((_, q)) = quantity.as_mut() {
                        match q.handle_event(ui, ev, &texts) {
                            Some(QuantityOutcome::Buy(a)) => out.push(a),
                            Some(QuantityOutcome::Closed) => {}
                            None => {}
                        }
                    }
                    match win.handle_event(ui, ev, &texts) {
                        Some(VendorOutcome::Send(v)) => out.extend(v),
                        Some(VendorOutcome::AskQuantity { entry, max }) => {
                            open_quantity = Some((*id, entry, max));
                        }
                        None => {}
                    }
                }
            }
        }
        if let Some((window, entry, max)) = open_quantity {
            let present = Present::new(data, state);
            self.open_quantity(ui, window, entry, max, &present, &texts);
        }
        for p in &mut self.panels {
            if let Some(w) = p.wallet.as_mut() {
                w.handle_event(ui, ev, &texts);
            }
        }
    }

    /// The icon of the choice-row template `template` of the choice layout (§5.3 rule 3:
    /// an action row uses its action entry's `ActionPickerUI_ListEntryTemplate`, whose
    /// templates differ only in the icon): the base image of the template's
    /// `ActionPickerTemplate_Icon` part, read from a throwaway instance and remembered.
    /// `None` without the install's choice layout or when the template has no icon image.
    pub fn action_icon(&mut self, ui: &mut Ui, template: u32) -> Option<tessera_ui::ImageRef> {
        if let Some(i) = self.action_icons.get(&template) {
            return i.clone();
        }
        let layout = self.refs.templates.choice.row?.layout;
        let icon = match ui.create_element(None, Did(layout), template) {
            Ok(k) => {
                let id = self.refs.nv.ids.action_icon;
                let part = if ui.element_id(k) == Some(id) {
                    Some(k)
                } else {
                    tessera_ui::game::find_below(ui, k, id)
                };
                let img = part.and_then(|p| ui.base_image(p)).map(|(i, _)| i);
                ui.destroy(k);
                img
            }
            Err(e) => {
                tracing::debug!(template, "choice row template not created: {e}");
                None
            }
        };
        if self.action_icons.len() >= MAX_ACTION_ICONS {
            self.action_icons.clear();
        }
        self.action_icons.insert(template, icon.clone());
        icon
    }

    /// Opens (or reuses) the quantity dialog of vendor window `window` for `entry` (§7.5).
    fn open_quantity(
        &mut self,
        ui: &mut Ui,
        window: u32,
        entry: u16,
        max: u32,
        present: &Present<'_>,
        texts: &dyn NpcTexts,
    ) {
        let needs_root = match self.windows.get(&window).map(|o| o.win.as_ref()) {
            Some(Win::Vendor { quantity, .. }) => quantity.is_none(),
            _ => return,
        };
        let opened = if needs_root {
            match self.open_native(ui, NpcWindowKind::Quantity, texts) {
                Ok((k, _, _)) => {
                    Some(QuantityDialog::attach(ui, &self.refs.nv, k, &self.art)).map(|q| (k, q))
                }
                Err(e) => {
                    self.fail(format!("quantity dialog: {e}"));
                    return;
                }
            }
        } else {
            None
        };
        let values = present.coin_values().to_vec();
        let Some(Win::Vendor {
            win: vendor,
            quantity,
            ..
        }) = self.windows.get_mut(&window).map(|o| o.win.as_mut())
        else {
            return;
        };
        if let Some(q) = opened {
            *quantity = Some(q);
        }
        let Some(item) = vendor.entry(entry).cloned() else {
            return;
        };
        if let Some((k, q)) = quantity.as_mut() {
            ui.set_shown(*k, true);
            q.open(
                ui,
                QuantityRequest {
                    window,
                    entry,
                    max,
                    unit_price: item.price,
                    coin_values: values,
                    name: item.name.clone(),
                    icon: item.icon.clone(),
                },
                texts,
            );
            ui.activate(*k);
        }
    }

    /// Step 3 of [`Self::frame`].
    fn end_frame(
        &mut self,
        state: &PlayerState,
        geometry: Option<&dyn UseGeometry>,
        out: &mut Vec<NpcAction>,
    ) {
        let npc = state.npc();
        for (id, o) in &mut self.windows {
            if let Win::Loot { win, .. } = o.win.as_mut()
                && let Some(a) = win.end_frame()
            {
                out.push(a);
            }
            if self.closes_sent.contains(id) {
                continue;
            }
            let gone = npc
                .window(tessera_net::domain::WindowId(*id))
                .is_some_and(|w| w.entity_gone);
            let far = match (geometry, o.entity) {
                (Some(g), Some(e)) => super::commands::in_range(g, e).is_some_and(|(ok, _, _)| !ok),
                _ => false,
            };
            if gone || far {
                self.closes_sent.insert(*id);
                out.push(NpcAction::CloseWindow { window: *id });
            }
        }
    }

    /// Hides closed quantity dialogs (after the events of a frame).
    pub fn hide_closed_dialogs(&mut self, ui: &mut Ui) {
        for o in self.windows.values_mut() {
            if let Win::Vendor {
                quantity: Some((k, q)),
                ..
            } = o.win.as_mut()
                && !q.open
                && ui.exists(*k)
            {
                ui.set_shown(*k, false);
            }
        }
    }

    /// The item under `element` in an open window: a record (`Ok`) or an inventory item
    /// (`Err`, the sell list).
    fn item_at(&self, ui: &Ui, element: ElementKey) -> Option<Result<Did, u64>> {
        let row_hit = |rows: &[ElementKey]| {
            rows.iter()
                .position(|r| ui.is_ancestor_or_self(*r, element))
        };
        for o in self.windows.values() {
            if !ui.is_ancestor_or_self(o.root, element) {
                continue;
            }
            match o.win.as_ref() {
                Win::Vendor { win, .. } => {
                    for page in [VendorPage::Buy, VendorPage::BuyBack, VendorPage::Sell] {
                        let Some(i) = row_hit(win.rows(page)) else {
                            continue;
                        };
                        return match page {
                            VendorPage::Buy => win.buy.get(i)?.item.map(|d| Ok(Did(d as u32))),
                            VendorPage::BuyBack => win
                                .buyback
                                .as_ref()?
                                .get(i)?
                                .item
                                .map(|d| Ok(Did(d as u32))),
                            VendorPage::Sell => win.sell.get(i).map(|s| Err(s.item)),
                        };
                    }
                }
                Win::Loot { win, items, .. } => {
                    let keys: Vec<ElementKey> = win.rows.iter().map(|(k, _)| *k).collect();
                    let i = row_hit(&keys)?;
                    let entry = win.rows[i].1;
                    return items
                        .iter()
                        .find(|it| it.entry == entry)?
                        .item
                        .map(|d| Ok(Did(d as u32)));
                }
                Win::Offer { win, rewards } => {
                    let rows = win.rows.as_ref()?;
                    let keys: Vec<ElementKey> = rows.rows.iter().map(|(k, _)| *k).collect();
                    let i = row_hit(&keys)?;
                    if let Some(RewardRow::Item { item: Some(d), .. }) =
                        rows.rows[i].1.and_then(|j| rewards.get(j))
                    {
                        return Some(Ok(Did(*d as u32)));
                    }
                    return None;
                }
                _ => {}
            }
        }
        None
    }

    /// Shows the examination tooltip of the item under `element` (`item-tooltips.md` §9).
    fn hover(
        &mut self,
        ui: &mut Ui,
        element: ElementKey,
        pointer: (i32, i32),
        state: &PlayerState,
        data: Option<&NpcData>,
    ) {
        let (Some(data), Some(item)) = (data, self.item_at(ui, element)) else {
            return;
        };
        let cd = data.character();
        let tip = match item {
            Err(id) => {
                crate::character::tooltip::exam_tooltip_of(cd, state, id, Purpose::Hover, false)
            }
            Ok(did) => {
                let req = TooltipRequest {
                    subject: Subject::Record(did),
                    instance: Instance::default(),
                    purpose: Purpose::Hover,
                    override_pair: None,
                    override_compare_kind: None,
                };
                cd.engine()
                    .build(&req, &crate::character::tooltip::viewer(state, false))
                    .and_then(|m| {
                        crate::character::tooltip::convert(
                            cd,
                            &m,
                            ExamKey {
                                instance: 0,
                                record: did.0,
                            },
                        )
                        .ok_or(tessera_gamedata::tooltip::TooltipError::NoTooltip)
                    })
            }
        };
        match tip {
            Ok(input) => {
                if let Err(e) = self.tips.show(ui, &input, pointer) {
                    tracing::debug!("NPC item tooltip not shown: {e}");
                }
            }
            Err(e) => tracing::debug!("no NPC item tooltip: {e}"),
        }
    }

    /// Closes every window (the UI shuts down).
    pub fn close_all(&mut self, ui: &mut Ui) {
        let ids: Vec<u32> = self.windows.keys().copied().collect();
        for id in ids {
            self.destroy(ui, id);
        }
        for mut p in std::mem::take(&mut self.panels) {
            if let Some(w) = p.wallet.as_mut() {
                w.detach(ui);
            }
            if ui.exists(p.root) {
                ui.destroy(p.root);
            }
        }
    }
}

/// A key the NPC windows take while one of them is the active panel (`npc-windows.md`
/// §5.7, §6.5, §7.5, §7.8, §8.3, §9.4; `ui-behaviour.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcKey {
    /// `EscapeKey`: a feed window sends `CloseWindow` (it stays until `WindowClosed`); the
    /// quantity dialog closes without buying; a window opened by name closes.
    Escape,
    /// `AcceptInput` (Enter): Loot All in the loot window (§8.3 rule 1), OK in the
    /// quantity dialog (§7.5 rule 5).
    Accept,
}

/// Which NPC window the active panel is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyOwner {
    Window(u32),
    Quantity(u32),
    Panel(ElementKey),
}

impl NpcUi {
    fn key_owner(&self, ui: &Ui) -> Option<KeyOwner> {
        let a = ui
            .active_panel()
            .filter(|a| ui.exists(*a) && ui.is_visible(*a))?;
        for (id, o) in &self.windows {
            if let Win::Vendor {
                quantity: Some((k, q)),
                ..
            } = o.win.as_ref()
                && *k == a
                && q.open
            {
                return Some(KeyOwner::Quantity(*id));
            }
            if o.root == a {
                return Some(KeyOwner::Window(*id));
            }
        }
        self.panels
            .iter()
            .any(|p| p.root == a)
            .then_some(KeyOwner::Panel(a))
    }

    /// Whether `key` goes to an NPC window: one of them is the active panel and the key
    /// does something there. The caller then keeps the key from the rest of the UI and
    /// hands it to [`Self::key`].
    #[must_use]
    pub fn takes_key(&self, ui: &Ui, key: NpcKey) -> bool {
        match (self.key_owner(ui), key) {
            (None, _) => false,
            (Some(_), NpcKey::Escape) | (Some(KeyOwner::Quantity(_)), NpcKey::Accept) => true,
            (Some(KeyOwner::Window(id)), NpcKey::Accept) => self
                .windows
                .get(&id)
                .is_some_and(|o| matches!(*o.win, Win::Loot { .. })),
            (Some(KeyOwner::Panel(_)), NpcKey::Accept) => false,
        }
    }

    /// Runs `key` on the active NPC window ([`NpcKey`]); returns the actions to send.
    pub fn key(&mut self, ui: &mut Ui, key: NpcKey, data: Option<&NpcData>) -> Vec<NpcAction> {
        let mut out = Vec::new();
        let texts = InstallTexts::new(data);
        match (self.key_owner(ui), key) {
            (Some(KeyOwner::Window(id)), NpcKey::Escape) => {
                // §5.7: the window stays until the feed's `WindowClosed`.
                out.push(NpcAction::CloseWindow { window: id });
            }
            (Some(KeyOwner::Window(id)), NpcKey::Accept) => {
                if let Some(Win::Loot { win, .. }) =
                    self.windows.get_mut(&id).map(|o| o.win.as_mut())
                {
                    out.push(win.loot_all());
                }
            }
            (Some(KeyOwner::Quantity(id)), k) => {
                if let Some(Win::Vendor {
                    quantity: Some((_, q)),
                    ..
                }) = self.windows.get_mut(&id).map(|o| o.win.as_mut())
                {
                    match k {
                        NpcKey::Escape => q.cancel(),
                        NpcKey::Accept => out.extend(q.confirm(ui, &texts)),
                    }
                }
                self.hide_closed_dialogs(ui);
            }
            (Some(KeyOwner::Panel(root)), NpcKey::Escape) => {
                if let Some(i) = self.panels.iter().position(|p| p.root == root) {
                    let mut p = self.panels.remove(i);
                    if let Some(w) = p.wallet.as_mut() {
                        w.detach(ui);
                    }
                    if ui.exists(p.root) {
                        ui.destroy(p.root);
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// Gives the held buy toggles (`VendorBuyUI`, §7.4: Alt, Shift, Tab) to the vendor
    /// window that is the active panel (or whose quantity dialog is); the other vendor
    /// windows have none held. A window is told only when its toggles change.
    pub fn set_buy_toggles(&mut self, ui: &mut Ui, held: BuyToggles, data: Option<&NpcData>) {
        let active = ui.active_panel();
        let texts = InstallTexts::new(data);
        for o in self.windows.values_mut() {
            let root = o.root;
            if let Win::Vendor {
                win,
                quantity,
                toggles,
                ..
            } = o.win.as_mut()
            {
                let is_active = active
                    .is_some_and(|a| a == root || quantity.as_ref().is_some_and(|(k, _)| *k == a));
                let want = if is_active {
                    held
                } else {
                    BuyToggles::default()
                };
                if *toggles != Some(want) {
                    *toggles = Some(want);
                    win.set_toggles(ui, want, &texts);
                }
            }
        }
    }

    /// A finished drag (§8.3: dragging a loot row into a bag takes that entry): a drag
    /// from a loot window's row that ended on an element outside that window.
    fn dragged(
        &self,
        ui: &Ui,
        source: ElementKey,
        target: Option<ElementKey>,
    ) -> Option<NpcAction> {
        let t = target?;
        self.windows.values().find_map(|o| match o.win.as_ref() {
            Win::Loot { win, .. }
                if ui.is_ancestor_or_self(o.root, source) && !ui.is_ancestor_or_self(o.root, t) =>
            {
                win.dragged(ui, source)
            }
            _ => None,
        })
    }
}

/// A window's view, before it is shown.
enum View {
    Dialogue(DialogueKind, tessera_ui::game::npc::dialogue::DialoguePage),
    Offer(tessera_ui::game::npc::offer::OfferView),
    Vendor(
        Vec<tessera_ui::game::npc::vendor::VendorItem>,
        Option<Vec<tessera_ui::game::npc::vendor::VendorItem>>,
    ),
    Barter(Vec<tessera_ui::game::npc::barter::BarterEntry>),
    Loot(tessera_ui::game::npc::loot::LootView),
}
