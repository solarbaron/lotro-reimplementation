// SPDX-License-Identifier: GPL-3.0-or-later
//! The wallet and money views of `lua-plugin-api.md` §15.3 through the client's plugin host
//! on scenarios (decision 0024 P3; §18 checks 80–85): each check plays a scenario headless
//! on its virtual clock, feeds every poll's events to the client's player state and the
//! session's wallet model (`scenario::plugins::ScenarioPlugins`, the path of
//! `--scenario … --headless --load-plugin`), and reads what synthetic plugins logged and
//! what their getters return.
//!
//! Checks 80–82, 84 and 85 use literal currency refs and need no install; check 83 reads
//! the install (skipped, reported as passed, without `TESSERA_GAME_DIR`). Every name is
//! invented; no game text is printed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::character::CharacterData;
use tessera_client::cli::Args;
use tessera_client::scenario::plugins::ScenarioPlugins;
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_net::domain::{Currency, InventoryEvent, Ref};
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::HeadlessDriver;
use tessera_net::{DisconnectReason, Event, Session};

/// A plugin that logs every wallet and money event with the getters read at delivery.
const LOGGER: &str = r#"
local p = Turbine.Gameplay.LocalPlayer.GetInstance()
A = p:GetAttributes()
W = p:GetWallet()
ITEMS = {}
local function say(s) Turbine.Shell.WriteLine("log " .. s) end
A.MoneyChanged = function(sender, args)
    local c, s, g = A:GetMoneyComponents()
    say("money " .. A:GetMoney() .. " " .. tostring(next(args) == nil) .. " "
        .. c .. "," .. s .. "," .. g)
end
W.ItemAdded = function(sender, args)
    say("added " .. args.Index .. " " .. args.Item:GetName() .. " "
        .. tostring(W:GetItem(args.Index) == args.Item) .. " " .. W:GetSize())
    ITEMS[#ITEMS + 1] = args.Item
    args.Item.QuantityChanged = function(item, a)
        say("quantity " .. item:GetName() .. " " .. item:GetQuantity() .. " "
            .. tostring(next(a) == nil))
    end
    args.Item.MaxQuantityChanged = function() say("maxquantity") end
end
W.ItemRemoved = function(sender, args)
    say("removed " .. args.Index .. " " .. args.Item:GetName() .. " "
        .. tostring(args.Item == ITEMS[#ITEMS] or args.Item == ITEMS[#ITEMS - 1]
            or args.Item == ITEMS[#ITEMS - 2] or args.Item == ITEMS[1]))
end
say("start " .. A:GetMoney() .. " " .. W:GetSize())
"#;

/// A plugin `name` in apartment `apartment` with `source` as its main file.
fn write_plugin(plugins: &Path, name: &str, apartment: &str, source: &str) {
    let dir = plugins.join("T");
    std::fs::create_dir_all(&dir).expect("plugins dir");
    std::fs::write(
        dir.join(format!("{name}.plugin")),
        format!(
            "<Plugin><Information><Name>{name}</Name><Author>Tessera</Author>\
             <Version>1</Version></Information><Package>T.{name}</Package>\
             <Configuration Apartment=\"{apartment}\"/></Plugin>"
        ),
    )
    .expect("descriptor");
    std::fs::write(dir.join(format!("{name}.lua")), source).expect("plugin");
}

struct Run {
    sp: ScenarioPlugins,
    _dir: tempfile::TempDir,
}

impl Run {
    /// The `log …` lines of the plugins, without the prefix.
    fn log(&self) -> Vec<String> {
        self.sp
            .lines()
            .into_iter()
            .filter_map(|l| l.strip_prefix("log ").map(str::to_owned))
            .collect()
    }
}

/// Evaluates `src` in `apartment` with the plugins' package globals (namespace `T`) as
/// locals `W`, `A`, `ITEMS`.
fn eval<T: tessera_plugins::mlua::FromLuaMulti>(
    sp: &ScenarioPlugins,
    apartment: &str,
    src: &str,
) -> T {
    let src = format!("local W, A, ITEMS = T.W, T.A, T.ITEMS\n{src}");
    sp.plugins()
        .manager()
        .with_lua(apartment, |lua| lua.load(&src).eval::<T>())
        .expect("apartment")
        .unwrap_or_else(|e| panic!("{src}: {e}"))
}

/// Plays `scenario` headless with the plugins `load` (from `setup`) and `data`; `each`
/// runs after every poll with that poll's events.
fn run(
    scenario: &str,
    install: Option<&Path>,
    data: Option<Arc<CharacterData>>,
    setup: impl FnOnce(&Path),
    load: &[&str],
    mut each: impl FnMut(&mut ScenarioPlugins, &[Event]),
) -> Run {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins: PathBuf = dir.path().join("Plugins");
    std::fs::create_dir_all(&plugins).expect("plugins");
    setup(&plugins);
    let file = dir.path().join("check.scenario");
    std::fs::write(&file, scenario).expect("write scenario");
    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario = Some(file);
    args.headless = true;
    args.no_data = install.is_none();
    let choice = install.map_or(InstallChoice::None, |p| {
        InstallChoice::Found(p.to_path_buf())
    });
    let resolved = Arc::new(prepare(&args, &choice).expect("scenario prepares"));
    let config = headless_config(&args, Arc::clone(&resolved)).expect("config");
    let backend = SandboxBackend::new(config).expect("sandbox");
    let mut driver: HeadlessDriver<Vec<u8>> =
        HeadlessDriver::new(Session::new(backend), resolved, None);
    let load: Vec<String> = load.iter().map(|s| (*s).to_owned()).collect();
    let mut sp = ScenarioPlugins::start(&plugins, dir.path().join("data"), &load, data);
    let outcome = driver.run_with(|events| {
        sp.observe(events);
        each(&mut sp, events);
    });
    assert_eq!(outcome.exit_code(), 0, "the scenario finishes: {outcome:?}");
    assert!(sp.all_loaded(), "the plugins loaded: {:?}", sp.lines());
    Run { sp, _dir: dir }
}

fn logger(plugins: &Path) {
    write_plugin(plugins, "Logger", "Default", LOGGER);
}

/// Whether `events` hold `WalletChanged` of literal currency `name` with `amount`.
fn wallet_event(events: &[Event], name: &str, amount: u64) -> bool {
    events.iter().any(|e| {
        matches!(e, Event::Inventory(InventoryEvent::WalletChanged {
            currency: Currency::Item(Ref::Literal(n)),
            amount: a,
        }) if n == name && *a == amount)
    })
}

#[test]
fn check_80_money() {
    let r = run(
        "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
         at 0.5s wallet coin 123456789\n+0.5s wallet coin 123456789\n+0.5s wallet coin 0\n",
        None,
        None,
        logger,
        &["Logger"],
        |_, _| {},
    );
    assert_eq!(
        r.log(),
        vec![
            "start 0 0",
            "money 123456789 true 89,567,1234",
            "money 0 true 0,0,0",
        ]
    );
    let size: u32 = eval(&r.sp, "Default", "return W:GetSize()");
    assert_eq!(size, 0, "no Wallet event and an empty wallet");
}

#[test]
fn checks_81_and_82_wallet_list_and_quantities() {
    let scenario = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
         at 0.5s wallet \"A\" 5\n+0.1s wallet \"B\" 7\n+0.1s wallet \"C\" 1\n\
         +0.5s wallet \"B\" 0\n+0.5s wallet \"B\" 4\n\
         +0.5s wallet \"A\" 9\n+0.5s wallet \"A\" 9\n+0s wallet \"D\" 0\n+0s wallet coin 10\n\
         +0.5s wallet \"Lit\" 2\n";
    let mut stage = 0;
    let r = run(scenario, None, None, logger, &["Logger"], |sp, events| {
        if wallet_event(events, "C", 1) {
            stage = 1;
            let ok: bool = eval(
                sp,
                "Default",
                "return W:GetSize() == 3 and W:GetItem(1):GetName() == 'A' \
                 and W:GetItem(1):GetQuantity() == 5 and W:GetItem(0) == nil \
                 and W:GetItem(4) == nil and W:GetItem(-0.5) == nil and W:GetItem(nil) == nil \
                 and W:GetItem(1.9) == W:GetItem(1) and W:GetItem('2') == W:GetItem(2)",
            );
            assert!(ok, "check 81 getters");
            let () = eval(sp, "Default", "B_OLD = W:GetItem(2); C_OBJ = W:GetItem(3)");
        }
        if wallet_event(events, "B", 0) {
            stage = 2;
            let ok: bool = eval(
                sp,
                "Default",
                "return W:GetSize() == 2 and W:GetItem(2) == C_OBJ",
            );
            assert!(ok, "check 81: removal renumbers");
        }
        if wallet_event(events, "B", 4) {
            stage = 3;
            let ok: bool = eval(
                sp,
                "Default",
                "return W:GetItem(3) ~= B_OLD and W:GetItem(3):GetQuantity() == 4 \
                 and B_OLD:GetQuantity() == 7",
            );
            assert!(ok, "check 81: a currency that comes back is a new object");
        }
    });
    assert_eq!(stage, 3);
    assert_eq!(
        r.log(),
        vec![
            "start 0 0",
            "added 1 A true 1",
            "added 2 B true 2",
            "added 3 C true 3",
            "removed 2 B true",
            "added 3 B true 3",
            "quantity A 9 true",
            // `wallet coin 10` is a money event, not a Wallet or WalletItem one (check 82).
            "money 10 true 10,0,0",
            "added 4 Lit true 4",
        ]
    );
    let ok: bool = eval(
        &r.sp,
        "Default",
        "local l = W:GetItem(4) return l:GetName() == 'Lit' and l:GetDescription() == '' \
         and l:GetImage() == 0 and l:GetSmallImage() == 0 and l:GetMaxQuantity() == 0 \
         and l:IsAccountItem() == false",
    );
    assert!(ok, "check 82: a literal entry's values");
}

#[test]
fn check_84_objects_and_apartments() {
    let scenario = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
         at 0.5s wallet \"A\" 5\n+0.5s wallet \"B\" 7\n+0.1s wallet \"C\" 1\n";
    let mut third = false;
    let r = run(
        scenario,
        None,
        None,
        |p| {
            write_plugin(p, "One", "First", LOGGER);
            write_plugin(p, "Two", "Second", LOGGER);
            write_plugin(
                p,
                "Three",
                "Third",
                "SEEN = {}\nfunction LATE()\n  W3 = Turbine.Gameplay.LocalPlayer.GetInstance():GetWallet()\n  W3.ItemAdded = function(s, a) SEEN[#SEEN + 1] = a.Item:GetName() end\nend\n",
            );
        },
        &["One", "Two", "Three"],
        |sp, events| {
            if wallet_event(events, "A", 5) {
                // A chat command after A's ItemAdded was delivered: the third plugin's first
                // GetWallet().
                let () = eval(sp, "Third", "T.LATE()");
                third = true;
                let ok: bool = eval(
                    sp,
                    "Third",
                    "return T.W3:GetSize() == 1 and T.W3:GetItem(1):GetName() == 'A'",
                );
                assert!(ok, "the late wallet lists A");
            }
        },
    );
    assert!(third);
    for ap in ["First", "Second"] {
        let ok: bool = eval(
            &r.sp,
            ap,
            "local p = Turbine.Gameplay.LocalPlayer.GetInstance() return p:GetWallet() == W",
        );
        assert!(ok, "{ap}: one Wallet object per apartment");
    }
    let lines = r.log();
    for name in ["A", "B", "C"] {
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.starts_with("added ") && l.split(' ').nth(2) == Some(name))
                .count(),
            2,
            "both apartments saw {name}: {lines:?}"
        );
    }
    let seen: String = eval(&r.sp, "Third", "return table.concat(T.SEEN, ',')");
    assert_eq!(seen, "B,C", "no ItemAdded for A in the late apartment");
    // Different objects across apartments: the first apartment's W is not the second's.
    let w1: String = eval(&r.sp, "First", "return tostring(W)");
    let w2: String = eval(&r.sp, "Second", "return tostring(W)");
    assert!(!w1.is_empty() && !w2.is_empty());
}

#[test]
fn check_85_order_and_leaving_the_world() {
    let scenario = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
         at 0.5s wallet \"A\" 5\n+0s wallet \"A\" 0\n\
         +0.5s wallet \"A\" 5\n+0s wallet \"B\" 6\n+0s wallet \"C\" 7\n+0s wallet coin 50\n";
    let mut r = run(scenario, None, None, logger, &["Logger"], |_, _| {});
    let lines = r.log();
    // Getters at delivery (§15): in the ItemAdded handler A has already left the list.
    assert_eq!(&lines[1..3], ["added 1 A false 0", "removed 1 A true"]);
    r.sp.observe(&[Event::Disconnected {
        reason: DisconnectReason::ClientRequested,
    }]);
    let lines = r.log();
    let n = lines.len();
    assert_eq!(
        &lines[n - 4..],
        [
            "removed 3 C true",
            "removed 2 B true",
            "removed 1 A true",
            "money 0 true 0,0,0"
        ]
    );
    let ok: bool = eval(
        &r.sp,
        "Default",
        "return W:GetSize() == 0 and A:GetMoney() == 0",
    );
    assert!(ok);
    assert!(
        !lines.iter().any(|l| l == "maxquantity"),
        "no MaxQuantityChanged (§15.3)"
    );
}

#[test]
fn check_83_wallet_item_values_from_the_install() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
    else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let npc = tessera_client::npc::NpcData::new(Arc::clone(&data)).expect("NPC data");
    let catalog = npc.presenter().wallet_catalog();
    let mut items: Vec<_> = catalog.items().to_vec();
    items.sort_by_key(|i| i.did.0);
    let pid = |n: &str| data.pid(n);
    let (bind, small) = (pid("Inventory_BindToAccount"), pid("PaperItem_SmallIcon"));
    let rec = |d: tessera_formats::Did| npc.record(d).expect("wallet item record");
    let has = |d, p: Option<u32>| p.and_then(|p| rec(d).get(p).cloned());
    let a = items
        .iter()
        .find(|i| i.cap.is_some())
        .expect("(a) an item with CapDefault");
    let b = items
        .iter()
        .find(|i| {
            i.did != a.did
                && matches!(
                    has(i.did, bind),
                    Some(tessera_formats::PropertyValue::Bool(true))
                )
                && has(i.did, small).is_some()
        })
        .expect("(b)");
    let c = items
        .iter()
        .find(|i| i.did != a.did && i.did != b.did && has(i.did, small).is_none())
        .expect("(c)");
    eprintln!(
        "wallet items: a {:#010x}, b {:#010x}, c {:#010x}",
        a.did.0, b.did.0, c.did.0
    );
    let scenario = format!(
        "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
         select $w = did {:#x},{:#x},{:#x} all\n\
         at 0.5s wallet $w[0] 1\n+0s wallet $w[1] 1\n+0s wallet $w[2] 1\n",
        a.did.0, b.did.0, c.did.0
    );
    let r = run(
        &scenario,
        Some(&dir),
        Some(Arc::clone(&data)),
        logger,
        &["Logger"],
        |_, _| {},
    );
    // The did selector sorts by DID: the entries follow the item order a, b, c by DID.
    let mut sorted = [a, b, c];
    sorted.sort_by_key(|i| i.did.0);
    for (k, it) in sorted.iter().enumerate() {
        let want = npc.wallet_item(it.did);
        let i = k + 1;
        let name: String = eval(
            &r.sp,
            "Default",
            &format!("return W:GetItem({i}):GetName()"),
        );
        let desc: String = eval(
            &r.sp,
            "Default",
            &format!("return W:GetItem({i}):GetDescription()"),
        );
        let image: f64 = eval(
            &r.sp,
            "Default",
            &format!("return W:GetItem({i}):GetImage()"),
        );
        let small_image: f64 = eval(
            &r.sp,
            "Default",
            &format!("return W:GetItem({i}):GetSmallImage()"),
        );
        let max: f64 = eval(
            &r.sp,
            "Default",
            &format!("return W:GetItem({i}):GetMaxQuantity()"),
        );
        let account: bool = eval(
            &r.sp,
            "Default",
            &format!("return W:GetItem({i}):IsAccountItem()"),
        );
        assert_eq!(name, want.name);
        assert!(!name.is_empty(), "rendered Name");
        assert_eq!(desc, want.description);
        assert_eq!(image as u32, want.image);
        assert_eq!(small_image as u32, want.small_image);
        assert_eq!(max as u64, want.max_quantity);
        assert_eq!(account, want.account_item);
        if it.did == a.did {
            assert_eq!(Some(max as i64), a.cap, "(a) CapDefault");
        }
        if it.did == b.did {
            assert!(account, "(b) bound to the account");
        }
        if it.did == c.did {
            assert_eq!(small_image, 0.0, "(c) no small icon");
        }
    }
}
