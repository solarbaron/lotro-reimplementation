// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` wallet and money views (`GetWallet`, `Wallet`, `WalletItem`,
//! `GetMoney`, `GetMoneyComponents`, `MoneyChanged`) against `lua-plugin-api.md` §15.3 and
//! its conformance checks 80–82, 84 and 85, with synthetic plugin trees and a synthetic
//! wallet source. Check 83 (wallet item values against the install) needs the client's
//! wallet source, which builds the values from the install; it is not covered here.
//!
//! The source below plays the client's part: the session's wallet model of §15.3 (coin
//! amount *C*, ordered entry list *E* of currencies with a non-zero amount) applying
//! feed-like `wallet` steps and deriving the plugin events by the §15.3 model table, with
//! coin, literal currencies and synthetic barter currencies (record refs with precomputed
//! values). The plugin host relays the events, queued to step 7 (§15), and owns the getter
//! rules, object identity and which apartments receive an event, which these tests check.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::{
    CoreHost, DataScope, GameplayEvent, HostApi, LocalPlayerInfo, LocalPlayerSource, ManagerConfig,
    PluginDataLocation, PluginManager, ScriptErrorReport, WalletEntry, WalletItemData,
    WalletSource, WalletViewEvent, mlua,
};

// ---------------------------------------------------------------------------------------
// Host and fixture
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Host {
    errors: Mutex<Vec<ScriptErrorReport>>,
    data: Option<PluginDataLocation>,
}

impl CoreHost for Host {
    fn report_error(&self, report: &ScriptErrorReport) {
        self.errors.lock().unwrap().push(report.clone());
    }
}

impl HostApi for Host {
    fn write_line(&self, _text: &str) {}
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.as_ref().and_then(|d| d.dir_for(scope))
    }
}

struct Fx {
    _dir: tempfile::TempDir,
    plugins: PathBuf,
    host: Arc<Host>,
}

impl Fx {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        std::fs::create_dir_all(plugins.join("T")).unwrap();
        Self {
            _dir: dir,
            plugins,
            host: Arc::new(Host::default()),
        }
    }

    fn plugin(&self, name: &str, apartment: Option<&str>, source: &str) {
        let cfg = apartment
            .map(|a| format!("<Configuration Apartment=\"{a}\"/>"))
            .unwrap_or_default();
        std::fs::write(
            self.plugins.join(format!("T/{name}.plugin")),
            format!(
                "<Plugin><Information><Name>{name}</Name><Author>Au</Author>\
                 <Version>1</Version></Information><Package>T.{name}</Package>{cfg}</Plugin>"
            ),
        )
        .unwrap();
        std::fs::write(self.plugins.join(format!("T/{name}.lua")), source).unwrap();
    }

    fn manager(&self, wallet: Option<&Arc<FakeWallet>>) -> PluginManager {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(c, host);
        m.refresh();
        m.set_local_player_source(Some(Player::new(wallet)));
        m
    }

    fn assert_no_errors(&self) {
        let errors = self.host.errors.lock().unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }
}

fn eval<T: mlua::FromLuaMulti>(m: &PluginManager, apartment: &str, src: &str) -> T {
    m.with_lua(apartment, |lua| lua.load(src).eval::<T>())
        .expect("apartment exists")
        .expect("evaluates")
}

/// The plugin log, one entry per line, and clear it.
fn take_log(m: &PluginManager, apartment: &str) -> Vec<String> {
    let s: String = eval(
        m,
        apartment,
        "local s = table.concat(T.Log, '\\n') T.Log = {} return s",
    );
    s.lines().map(str::to_owned).collect()
}

struct Player {
    wallet: Option<Arc<FakeWallet>>,
    /// `LocalPlayerInfo::alignment` (1 free people, 2 monster player).
    alignment: AtomicU32,
}

impl Player {
    fn new(wallet: Option<&Arc<FakeWallet>>) -> Arc<Self> {
        Arc::new(Self {
            wallet: wallet.cloned(),
            alignment: AtomicU32::new(1),
        })
    }
}

impl LocalPlayerSource for Player {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        Some(LocalPlayerInfo {
            name: "Wanderer".into(),
            alignment: self.alignment.load(Ordering::Relaxed),
            ..LocalPlayerInfo::default()
        })
    }
    fn wallet(&self) -> Option<Arc<dyn WalletSource>> {
        self.wallet.clone().map(|w| w as Arc<dyn WalletSource>)
    }
}

// ---------------------------------------------------------------------------------------
// Synthetic wallet model (the client's part)
// ---------------------------------------------------------------------------------------

/// A currency ref of a feed `WalletChanged { Item(r), a }`: a record or a literal string.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ref {
    Literal(String),
    Record(u32),
}

fn lit(s: &str) -> Ref {
    Ref::Literal(s.into())
}

#[derive(Default)]
struct Model {
    revision: u64,
    coin: u64,
    entries: Vec<(Ref, Arc<WalletEntry>)>,
    next_id: u64,
}

/// The session's wallet model of §15.3 with a synthetic record table for barter
/// currencies (the client precomputes these values from the install).
struct FakeWallet {
    model: Mutex<Model>,
    records: Vec<(u32, Arc<WalletItemData>)>,
}

/// Synthetic barter currencies: a capped, account-wide one with both images, and one
/// without a small image or any optional property.
fn barter_records() -> Vec<(u32, Arc<WalletItemData>)> {
    vec![
        (
            0x7000_1001,
            Arc::new(WalletItemData {
                record: Some(0x7000_1001),
                name: "Marks".into(),
                description: "Synthetic barter marks.".into(),
                image: 0x4100_0001,
                small_image: 0x4100_0002,
                max_quantity: 500,
                account_item: true,
            }),
        ),
        (
            0x7000_1002,
            Arc::new(WalletItemData {
                record: Some(0x7000_1002),
                name: "Shards".into(),
                description: String::new(),
                image: 0x4100_0003,
                small_image: 0,
                max_quantity: 999_999_999,
                account_item: false,
            }),
        ),
    ]
}

impl FakeWallet {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            model: Mutex::new(Model::default()),
            records: barter_records(),
        })
    }

    /// The model of a new world session after `prev`'s: empty, with the change count and
    /// the entry IDs continuing `prev`'s (the [`WalletSource`] contract).
    fn continuing(prev: &FakeWallet) -> Arc<Self> {
        let p = prev.model.lock().unwrap();
        Arc::new(Self {
            model: Mutex::new(Model {
                revision: p.revision,
                next_id: p.next_id,
                ..Model::default()
            }),
            records: barter_records(),
        })
    }

    fn info(&self, r: &Ref) -> Arc<WalletItemData> {
        match r {
            Ref::Literal(s) => Arc::new(WalletItemData::literal(s.as_str())),
            Ref::Record(did) => self
                .records
                .iter()
                .find(|(d, _)| d == did)
                .map(|(_, i)| Arc::clone(i))
                .unwrap_or_else(|| {
                    Arc::new(WalletItemData {
                        record: Some(*did),
                        ..WalletItemData::default()
                    })
                }),
        }
    }

    /// `wallet REF a` (`WalletChanged { Item(r), a }`), by the §15.3 model table.
    fn item(&self, r: Ref, a: u64) -> Vec<WalletViewEvent> {
        let info = self.info(&r);
        let mut m = self.model.lock().unwrap();
        let pos = m.entries.iter().position(|(k, _)| *k == r);
        match pos {
            None if a > 0 => {
                m.revision += 1;
                m.next_id += 1;
                let entry = Arc::new(WalletEntry {
                    id: m.next_id,
                    info,
                    quantity: a,
                });
                m.entries.push((r, Arc::clone(&entry)));
                vec![WalletViewEvent::ItemAdded {
                    revision: m.revision,
                    index: m.entries.len() as u32,
                    entry,
                }]
            }
            Some(k) if a == 0 => {
                m.revision += 1;
                let (_, entry) = m.entries.remove(k);
                vec![WalletViewEvent::ItemRemoved {
                    revision: m.revision,
                    index: k as u32 + 1,
                    entry,
                }]
            }
            Some(k) if m.entries[k].1.quantity != a => {
                m.revision += 1;
                let entry = Arc::new(WalletEntry {
                    quantity: a,
                    ..(*m.entries[k].1).clone()
                });
                m.entries[k].1 = Arc::clone(&entry);
                vec![WalletViewEvent::QuantityChanged {
                    revision: m.revision,
                    entry,
                }]
            }
            _ => Vec::new(),
        }
    }

    /// `wallet coin a` (`WalletChanged { Coin, a }`).
    fn coin(&self, a: u64) -> Vec<WalletViewEvent> {
        let mut m = self.model.lock().unwrap();
        if m.coin == a {
            return Vec::new();
        }
        m.coin = a;
        m.revision += 1;
        vec![WalletViewEvent::MoneyChanged {
            revision: m.revision,
        }]
    }

    /// Leaving the world (§15.3): the entries leave from the last to the first, then *C*
    /// becomes 0.
    fn leave_world(&self) -> Vec<WalletViewEvent> {
        let mut out = Vec::new();
        {
            let mut m = self.model.lock().unwrap();
            while let Some((_, entry)) = m.entries.pop() {
                m.revision += 1;
                out.push(WalletViewEvent::ItemRemoved {
                    revision: m.revision,
                    index: m.entries.len() as u32 + 1,
                    entry,
                });
            }
        }
        out.extend(self.coin(0));
        out
    }
}

impl WalletSource for FakeWallet {
    fn revision(&self) -> u64 {
        self.model.lock().unwrap().revision
    }
    fn money(&self) -> u64 {
        self.model.lock().unwrap().coin
    }
    fn entry_count(&self) -> u32 {
        self.model.lock().unwrap().entries.len() as u32
    }
    fn entry(&self, position: u32) -> Option<Arc<WalletEntry>> {
        let m = self.model.lock().unwrap();
        m.entries.get(position as usize).map(|(_, e)| Arc::clone(e))
    }
    fn entry_by_id(&self, id: u64) -> Option<Arc<WalletEntry>> {
        let m = self.model.lock().unwrap();
        m.entries
            .iter()
            .find(|(_, e)| e.id == id)
            .map(|(_, e)| Arc::clone(e))
    }
}

fn raise(m: &mut PluginManager, events: Vec<WalletViewEvent>) {
    for e in events {
        m.raise_gameplay_event(GameplayEvent::Wallet(e));
    }
}

/// One `wallet` step per tick: raise its events and run step 7.
fn step(m: &mut PluginManager, events: Vec<WalletViewEvent>) {
    raise(m, events);
    m.pump();
}

/// A plugin that holds its `Wallet` and attributes objects from load and logs every
/// wallet, `WalletItem` and money event, with the getters read at delivery.
const LOGGER: &str = r#"
local p = Turbine.Gameplay.LocalPlayer.GetInstance()
Log = {}
Objs = {}
local function add(s) Log[#Log + 1] = s end
W = p:GetWallet()
A = p:GetAttributes()
W.ItemAdded = function(sender, args)
    local item = args.Item
    add("added " .. args.Index .. " " .. item:GetName() .. " "
        .. tostring(sender == W and W:GetItem(args.Index) == item) .. " " .. W:GetSize())
    Objs[item:GetName()] = item
    item.QuantityChanged = function(s, a)
        add("qty " .. s:GetName() .. " " .. tostring(next(a) == nil) .. " " .. s:GetQuantity())
    end
    item.MaxQuantityChanged = function() add("max " .. item:GetName()) end
end
W.ItemRemoved = function(sender, args)
    add("removed " .. args.Index .. " " .. args.Item:GetName() .. " "
        .. tostring(Objs[args.Item:GetName()] == args.Item))
    Gone = Gone or {}
    Gone[args.Item:GetName()] = args.Item
end
A.MoneyChanged = function(sender, args)
    add("money " .. tostring(sender == A and next(args) == nil) .. " " .. A:GetMoney())
end
"#;

fn logger_manager(fx: &Fx, wallet: &Arc<FakeWallet>) -> PluginManager {
    fx.plugin("Purse", None, LOGGER);
    let mut m = fx.manager(Some(wallet));
    m.load_plugin("Purse").unwrap();
    m
}

// ---------------------------------------------------------------------------------------
// Checks 80–82, 84, 85
// ---------------------------------------------------------------------------------------

#[test]
fn check_80_money() {
    let fx = Fx::new();
    let w = FakeWallet::new();
    let mut m = logger_manager(&fx, &w);
    let money = |m: &PluginManager| {
        eval::<String>(
            m,
            "Default",
            "local c, s, g = T.A:GetMoneyComponents()
             return table.concat({ T.A:GetMoney(), c, s, g }, ' ')",
        )
    };
    assert_eq!(money(&m), "0 0 0 0");

    raise(&mut m, w.coin(123_456_789));
    // Queued: nothing before step 7, but the getter already reads the model.
    assert!(take_log(&m, "Default").is_empty());
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["money true 123456789"]);
    assert_eq!(money(&m), "123456789 89 567 1234");

    step(&mut m, w.coin(123_456_789));
    assert!(take_log(&m, "Default").is_empty());
    step(&mut m, w.coin(0));
    assert_eq!(take_log(&m, "Default"), ["money true 0"]);
    assert_eq!(money(&m), "0 0 0 0");
    assert_eq!(eval::<u32>(&m, "Default", "return T.W:GetSize()"), 0);
    fx.assert_no_errors();
}

/// Checks 81's steps, with its assertions.
fn check_81_steps(m: &mut PluginManager, w: &FakeWallet) {
    for (name, a) in [("A", 5), ("B", 7), ("C", 1)] {
        step(m, w.item(lit(name), a));
    }
    assert_eq!(
        take_log(m, "Default"),
        ["added 1 A true 1", "added 2 B true 2", "added 3 C true 3"]
    );
    let s: String = eval(
        m,
        "Default",
        "local W = T.W return table.concat({ W:GetSize(), W:GetItem(1):GetName(),
            W:GetItem(1):GetQuantity(), tostring(W:GetItem(0)), tostring(W:GetItem(4)),
            tostring(W:GetItem(-0.5)), tostring(W:GetItem(nil)), tostring(W:GetItem()),
            tostring(W:GetItem('x')), tostring(W:GetItem(1.9) == W:GetItem(1)),
            tostring(W:GetItem('2') == W:GetItem(2)), tostring(W:GetItem(' 3 ') == T.Objs.C),
            tostring(W:GetItem(1/0)), tostring(W:GetItem(0/0)), tostring(W:GetItem(2^31 + 1)) },
            ' ')",
    );
    assert_eq!(
        s,
        "3 A 5 nil nil nil nil nil nil true true true nil nil nil"
    );

    step(m, w.item(lit("B"), 0));
    assert_eq!(take_log(m, "Default"), ["removed 2 B true"]);
    let s: String = eval(
        m,
        "Default",
        "return table.concat({ T.W:GetSize(), tostring(T.W:GetItem(2) == T.Objs.C) }, ' ')",
    );
    assert_eq!(s, "2 true");

    eval::<()>(m, "Default", "T.OldB = T.Objs.B");
    step(m, w.item(lit("B"), 4));
    assert_eq!(take_log(m, "Default"), ["added 3 B true 3"]);
    let s: String = eval(
        m,
        "Default",
        "return table.concat({ tostring(T.W:GetItem(3) ~= T.OldB), T.W:GetItem(3):GetQuantity(),
            T.OldB:GetQuantity(), T.OldB:GetName() }, ' ')",
    );
    assert_eq!(s, "true 4 7 B");
}

#[test]
fn check_81_wallet_list() {
    let fx = Fx::new();
    let w = FakeWallet::new();
    let mut m = logger_manager(&fx, &w);
    check_81_steps(&mut m, &w);
    fx.assert_no_errors();
}

#[test]
fn check_82_quantities() {
    let fx = Fx::new();
    let w = FakeWallet::new();
    let mut m = logger_manager(&fx, &w);
    check_81_steps(&mut m, &w);

    step(&mut m, w.item(lit("A"), 9));
    assert_eq!(take_log(&m, "Default"), ["qty A true 9"]);
    assert_eq!(
        eval::<u64>(&m, "Default", "return T.Objs.A:GetQuantity()"),
        9
    );
    step(&mut m, w.item(lit("A"), 9));
    step(&mut m, w.item(lit("D"), 0));
    // The coin step gives only `MoneyChanged`, never a `Wallet` or `WalletItem` event.
    step(&mut m, w.coin(10));
    assert_eq!(take_log(&m, "Default"), ["money true 10"]);

    let s: String = eval(
        &m,
        "Default",
        "local a = T.Objs.A return table.concat({ '[' .. a:GetDescription() .. ']',
            a:GetImage(), a:GetSmallImage(), a:GetMaxQuantity(), tostring(a:IsAccountItem()) },
            ' ')",
    );
    assert_eq!(s, "[] 0 0 0 false");
    fx.assert_no_errors();
}

#[test]
fn check_84_objects_and_apartments() {
    let fx = Fx::new();
    fx.plugin("One", Some("A1"), LOGGER);
    fx.plugin("Two", Some("A2"), LOGGER);
    fx.plugin("Three", Some("A3"), "Log = {}");
    fx.plugin("Four", Some("A4"), "Log = {}");
    let w = FakeWallet::new();
    let mut m = fx.manager(Some(&w));
    for p in ["One", "Two", "Three", "Four"] {
        m.load_plugin(p).unwrap();
    }
    let same = "local p = Turbine.Gameplay.LocalPlayer.GetInstance()
        return p:GetWallet() == T.W and p:GetWallet() == p:GetWallet()";
    assert!(eval::<bool>(&m, "A1", same));
    assert!(eval::<bool>(&m, "A2", same));

    step(&mut m, w.item(lit("A"), 5));
    // A third plugin asks for its wallet after `ItemAdded` of A was delivered: it sees A
    // listed at that moment.
    let first: String = eval(
        &m,
        "A3",
        "T.W = Turbine.Gameplay.LocalPlayer.GetInstance():GetWallet()
         T.W.ItemAdded = function(s, a) T.Log[#T.Log + 1] = 'added ' .. a.Index end
         return table.concat({ T.W:GetSize(), T.W:GetItem(1):GetName() }, ' ')",
    );
    assert_eq!(first, "1 A");
    raise(&mut m, w.item(lit("B"), 7));
    // A fourth asks after the change of B was made but before its delivery: it did not hold
    // its wallet when the change was made, so it gets no `ItemAdded` for B.
    eval::<()>(
        &m,
        "A4",
        "T.W = Turbine.Gameplay.LocalPlayer.GetInstance():GetWallet()
         T.W.ItemAdded = function(s, a) T.Log[#T.Log + 1] = 'added ' .. a.Index end",
    );
    m.pump();
    step(&mut m, w.item(lit("C"), 1));

    let expected = ["added 1 A true 1", "added 2 B true 2", "added 3 C true 3"];
    assert_eq!(take_log(&m, "A1"), expected);
    assert_eq!(take_log(&m, "A2"), expected);
    assert_eq!(take_log(&m, "A3"), ["added 2", "added 3"]);
    assert_eq!(take_log(&m, "A4"), ["added 3"]);
    let s: String = eval(
        &m,
        "A3",
        "return table.concat({ T.W:GetSize(), T.W:GetItem(1):GetName(),
            tostring(T.W:GetItem(1) == T.W:GetItem(1)) }, ' ')",
    );
    assert_eq!(s, "3 A true");
    fx.assert_no_errors();
}

#[test]
fn check_85_order_and_leaving_world() {
    let fx = Fx::new();
    let w = FakeWallet::new();
    let mut m = logger_manager(&fx, &w);

    // Two steps at the same tick: getters at delivery.
    let mut ev = w.item(lit("A"), 5);
    ev.extend(w.item(lit("A"), 0));
    step(&mut m, ev);
    assert_eq!(
        take_log(&m, "Default"),
        ["added 1 A false 0", "removed 1 A true"]
    );
    assert!(eval::<bool>(&m, "Default", "return T.Gone.A == T.Objs.A"));

    for (name, a) in [("A", 5), ("B", 7), ("C", 1)] {
        step(&mut m, w.item(lit(name), a));
    }
    step(&mut m, w.coin(50));
    take_log(&m, "Default");

    step(&mut m, w.leave_world());
    assert_eq!(
        take_log(&m, "Default"),
        [
            "removed 3 C true",
            "removed 2 B true",
            "removed 1 A true",
            "money true 0"
        ]
    );
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.W:GetSize(), T.A:GetMoney(), T.Gone.B:GetQuantity() }, ' ')",
    );
    assert_eq!(s, "0 0 7");
    fx.assert_no_errors();
}

// ---------------------------------------------------------------------------------------
// Getters, identity and sources
// ---------------------------------------------------------------------------------------

/// The getters of the wallet's entry `i`, joined by `|`.
fn describe(m: &PluginManager, i: u32) -> String {
    eval::<String>(
        m,
        "Default",
        &format!(
            "local e = T.W:GetItem({i}) return table.concat({{ e:GetName(),
                '[' .. e:GetDescription() .. ']', e:GetImage(), e:GetSmallImage(),
                string.format('%.0f', e:GetQuantity()), e:GetMaxQuantity(),
                tostring(e:IsAccountItem()), type(e:GetImage()) }}, '|')"
        ),
    )
}

#[test]
fn barter_currency_getters_agree_with_source() {
    let fx = Fx::new();
    let w = FakeWallet::new();
    let mut m = logger_manager(&fx, &w);
    step(&mut m, w.coin(1_234_567));
    step(&mut m, w.item(Ref::Record(0x7000_1001), 120));
    step(&mut m, w.item(Ref::Record(0x7000_1002), 3));
    step(&mut m, w.item(Ref::Record(0x7000_1999), 2));
    step(&mut m, w.item(lit("Tokens"), 1_000_000_000_000_000));
    assert_eq!(
        take_log(&m, "Default"),
        [
            "money true 1234567",
            "added 1 Marks true 1",
            "added 2 Shards true 2",
            "added 3  true 3",
            "added 4 Tokens true 4",
        ]
    );
    let image = 0x4100_0001_u32;
    let small = 0x4100_0002_u32;
    assert_eq!(
        describe(&m, 1),
        format!("Marks|[Synthetic barter marks.]|{image}|{small}|120|500|true|number")
    );
    assert_eq!(
        describe(&m, 2),
        format!("Shards|[]|{}|0|3|999999999|false|number", 0x4100_0003_u32)
    );
    // A record without any of the properties: the "Absent" column.
    assert_eq!(describe(&m, 3), "|[]|0|0|2|0|false|number");
    // Amounts above 2^32 are returned unchanged.
    assert_eq!(
        describe(&m, 4),
        "Tokens|[]|0|0|1000000000000000|0|false|number"
    );
    let s: String = eval(
        &m,
        "Default",
        "local c, s, g = T.A:GetMoneyComponents() return table.concat({ c, s, g }, ' ')",
    );
    assert_eq!(s, "67 345 12");
    // The source's values are read at every call.
    step(&mut m, w.item(Ref::Record(0x7000_1001), 121));
    assert_eq!(take_log(&m, "Default"), ["qty Marks true 121"]);
    assert!(describe(&m, 1).contains("|121|"));
    fx.assert_no_errors();
}

#[test]
fn no_max_quantity_event_and_no_events_for_entries_present_at_first_call() {
    let fx = Fx::new();
    fx.plugin("Late", None, "Log = {}");
    let w = FakeWallet::new();
    let mut m = fx.manager(Some(&w));
    m.load_plugin("Late").unwrap();
    step(&mut m, w.item(lit("A"), 5));
    step(&mut m, w.item(lit("B"), 6));
    step(&mut m, w.coin(9));
    // First calls now: the entries already listed get no `ItemAdded`; money already set
    // gives no `MoneyChanged`.
    eval::<()>(
        &m,
        "Default",
        "local p = Turbine.Gameplay.LocalPlayer.GetInstance()
         T.W = p:GetWallet() T.A = p:GetAttributes()
         local function add(s) T.Log[#T.Log + 1] = s end
         T.W.ItemAdded = function(s, a) add('added ' .. a.Index) end
         T.W.ItemRemoved = function(s, a) add('removed ' .. a.Index .. ' ' .. a.Item:GetName()) end
         T.A.MoneyChanged = function() add('money') end
         T.First = T.W:GetItem(1)
         T.First.QuantityChanged = function(s) add('qty ' .. s:GetQuantity()) end
         T.First.MaxQuantityChanged = function() add('max') end",
    );
    m.pump();
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(eval::<u32>(&m, "Default", "return T.W:GetSize()"), 2);
    assert_eq!(eval::<u64>(&m, "Default", "return T.A:GetMoney()"), 9);

    step(&mut m, w.item(lit("A"), 50));
    step(&mut m, w.item(lit("A"), 0));
    step(&mut m, w.item(lit("A"), 2));
    assert_eq!(
        take_log(&m, "Default"),
        ["qty 50", "removed 1 A", "added 2"]
    );
    // The removed object keeps its handler fields and last values, receives nothing more.
    step(&mut m, w.item(lit("A"), 3));
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(
        eval::<u64>(&m, "Default", "return T.First:GetQuantity()"),
        50
    );
    fx.assert_no_errors();
}

#[test]
fn without_a_wallet_source_the_views_are_empty() {
    let fx = Fx::new();
    fx.plugin("Empty", None, "Log = {}");
    let mut m = fx.manager(None);
    m.load_plugin("Empty").unwrap();
    let s: String = eval(
        &m,
        "Default",
        "local p = Turbine.Gameplay.LocalPlayer.GetInstance()
         local w, a = p:GetWallet(), p:GetAttributes()
         local c, s, g = a:GetMoneyComponents()
         return table.concat({ tostring(w ~= nil and w == p:GetWallet()), w:GetSize(),
            tostring(w:GetItem(1)), a:GetMoney(), c, s, g,
            tostring(a == p:GetAttributes()) }, ' ')",
    );
    assert_eq!(s, "true 0 nil 0 0 0 0 true");
    fx.assert_no_errors();
}

#[test]
fn identity_is_per_entry_and_survives_getters() {
    let fx = Fx::new();
    let w = FakeWallet::new();
    let mut m = logger_manager(&fx, &w);
    step(&mut m, w.item(lit("A"), 1));
    step(&mut m, w.item(lit("B"), 1));
    take_log(&m, "Default");
    // A field set on the object persists across `GetItem` calls; renumbering after a
    // removal keeps each entry's object.
    eval::<()>(&m, "Default", "T.W:GetItem(2).Tag = 'b'");
    step(&mut m, w.item(lit("A"), 0));
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.W:GetItem(1).Tag, tostring(T.W:GetItem(1) == T.Objs.B),
            tostring(T.W:GetItem(2)) }, ' ')",
    );
    assert_eq!(s, "b true nil");
    fx.assert_no_errors();
}

#[test]
fn money_changed_once_on_the_current_attributes_object() {
    let fx = Fx::new();
    fx.plugin("Both", None, "Log = {}");
    let w = FakeWallet::new();
    let mut m = fx.manager(Some(&w));
    let player = Player::new(Some(&w));
    m.set_local_player_source(Some(player.clone()));
    m.load_plugin("Both").unwrap();
    let handlers = |m: &PluginManager, key: &str| {
        eval::<()>(
            m,
            "Default",
            &format!(
                "T.{key} = Turbine.Gameplay.LocalPlayer.GetInstance():GetAttributes()
                 T.{key}.MoneyChanged = function() T.Log[#T.Log + 1] = '{key}' end"
            ),
        );
    };
    handlers(&m, "fp");
    // The character becomes a monster player within the session.
    player.alignment.store(2, Ordering::Relaxed);
    handlers(&m, "mp");
    assert!(eval::<bool>(&m, "Default", "return T.fp ~= T.mp"));
    step(&mut m, w.coin(5));
    assert_eq!(take_log(&m, "Default"), ["mp"]);
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.fp:GetMoney(), T.mp:GetMoney() }, ' ')",
    );
    assert_eq!(s, "0 5");
    fx.assert_no_errors();
}

#[test]
fn a_new_session_model_continues_the_count_and_its_events_arrive() {
    let fx = Fx::new();
    fx.plugin("Later", None, LOGGER);
    let w1 = FakeWallet::new();
    let mut m = fx.manager(Some(&w1));
    // Two changes before the plugin loads: its objects are noted at revision 2.
    raise(&mut m, w1.item(lit("A"), 5));
    raise(&mut m, w1.coin(30));
    m.pump();
    m.load_plugin("Later").unwrap();
    step(&mut m, w1.leave_world());
    assert_eq!(
        take_log(&m, "Default"),
        ["removed 1 A false", "money true 0"]
    );

    // A new `WorldEntered`: the client hands out a new model that continues the count.
    let w2 = FakeWallet::continuing(&w1);
    m.set_local_player_source(Some(Player::new(Some(&w2))));
    step(&mut m, w2.item(lit("X"), 3));
    step(&mut m, w2.coin(70));
    step(&mut m, w2.item(lit("Y"), 4));
    assert_eq!(
        take_log(&m, "Default"),
        ["added 1 X true 1", "money true 70", "added 2 Y true 2"]
    );
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.W:GetSize(), T.Gone.A:GetName(), T.Gone.A:GetQuantity(),
            T.A:GetMoney() }, ' ')",
    );
    assert_eq!(s, "2 A 5 70");
    fx.assert_no_errors();
}

#[test]
fn a_removed_object_keeps_its_values_when_an_id_comes_back() {
    let fx = Fx::new();
    let w1 = FakeWallet::new();
    let mut m = logger_manager(&fx, &w1);
    step(&mut m, w1.item(lit("A"), 5));
    step(&mut m, w1.leave_world());
    take_log(&m, "Default");
    // A client that breaks the contract and restarts the entry IDs: the removed object of
    // A (ID 1) never reads the new entry with ID 1.
    let w2 = FakeWallet::new();
    m.set_local_player_source(Some(Player::new(Some(&w2))));
    step(&mut m, w2.item(lit("X"), 3));
    assert_eq!(take_log(&m, "Default"), ["added 1 X true 1"]);
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.Gone.A:GetName(), T.Gone.A:GetQuantity(),
            tostring(T.Gone.A ~= T.W:GetItem(1)) }, ' ')",
    );
    assert_eq!(s, "A 5 true");
    fx.assert_no_errors();
}

/// Registry values of the apartment's `WalletItem` bookkeeping: (count, next prune size).
fn item_counts(m: &PluginManager) -> (i64, i64) {
    m.with_lua("Default", |lua| {
        (
            lua.named_registry_value::<i64>("tessera.plugins.wallet.count")
                .unwrap(),
            lua.named_registry_value::<i64>("tessera.plugins.wallet.prune")
                .unwrap(),
        )
    })
    .unwrap()
}

#[test]
fn pruning_keeps_listed_objects_and_held_detached_ones() {
    const N: u32 = 1030;
    let fx = Fx::new();
    fx.plugin("Many", None, "Log = {}");
    let w = FakeWallet::new();
    let mut m = fx.manager(Some(&w));
    m.load_plugin("Many").unwrap();
    for i in 1..=N {
        // Not raised: the plugin does not hold its wallet yet.
        w.item(lit(&format!("e{i}")), u64::from(i));
    }
    eval::<()>(
        &m,
        "Default",
        "T.W = Turbine.Gameplay.LocalPlayer.GetInstance():GetWallet()
         T.W.ItemRemoved = function(s, a)
             T.Log[#T.Log + 1] = 'removed ' .. a.Index .. ' ' .. a.Item:GetName() .. ' '
                 .. a.Item:GetQuantity() .. ' ' .. tostring(a.Item == T.Held)
         end
         T.Keep = T.W:GetItem(1) T.Keep.Tag = 'k'
         T.Held = T.W:GetItem(2) T.Held.Tag = 'h'",
    );
    assert_eq!(item_counts(&m), (2, 1024));
    // e2 leaves the source; its `ItemRemoved` is held back (as by a full queue).
    let removed = w.item(lit("e2"), 0);
    // Listing every entry reaches the prune size: e2's object is detached.
    let s: String = eval(
        &m,
        "Default",
        "for i = 1, T.W:GetSize() do T.W:GetItem(i) end
         return table.concat({ T.W:GetSize(), T.W:GetItem(1).Tag,
            tostring(T.W:GetItem(1) == T.Keep), T.Held:GetName(), T.Held:GetQuantity() }, ' ')",
    );
    assert_eq!(s, format!("{} k true e2 2", N - 1));
    // 1023 objects listed at the prune (e1, e3 … e1024), six made after it.
    assert_eq!(item_counts(&m), (1029, 2046));

    // The late `ItemRemoved` carries the object the plugin still holds.
    step(&mut m, removed);
    assert_eq!(take_log(&m, "Default"), ["removed 2 e2 2 true"]);
    assert_eq!(item_counts(&m), (1029, 2046));
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.Held.Tag == 'h' and T.W:GetItem(2):GetName() == 'e3'"
    ));
    fx.assert_no_errors();
}
