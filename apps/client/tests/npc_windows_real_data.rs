// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC windows on the user's install (read-only; decision 0024 P3 exit criterion 2:
//! "Dialogue, vendor and loot scenarios render with install text and icons; golden images
//! pass"). Each committed scenario plays headless in-process with the install (its
//! selectors resolve to install records), every poll's events reach the player state and
//! the NPC windows on the game UI built from the install's layouts (the headless UI path:
//! the UI host without a window, painted by the CPU painter), and every window the
//! scenario shows is pictured once the UI has settled and checked against a local-only
//! golden (decision 0031 tier 2, `ui_common/goldens.rs`; the run prints `blessed` or
//! `passed` per picture; with `TESSERA_SHEET_DIR` the pictures are copied there).
//!
//! Dialogue pages use install text where the spec gives a source (`npc-windows.md` §5.6:
//! quest roles, comments, action labels); vendor and loot entries are install items with
//! their icons. The entries and lines rendered are counted by the census categories of
//! §11, with no failure (`LayoutMissing`, `ElementMissing`, `RecordUnreadable`). Only
//! counts, categories and paths are printed.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test npc_windows_real_data -- --nocapture`.

#[path = "ui_common/goldens.rs"]
mod goldens;
mod npc_windows_common;
mod ui_common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use npc_windows_common::{capture, game_dir, play};
use tessera_client::character::{CharacterData, PlayerChange};
use tessera_client::npc::present::Present;
use tessera_client::npc::state::WindowContent;
use tessera_client::npc::ui::NpcKey;
use tessera_client::npc::{NpcChange, NpcData, NpcRefs, NpcUi, command_of};
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_gamedata::npc::{EntryCategory, LineCategory, NpcCensus};
use tessera_ui::game::npc::vendor::{BuyToggles, VendorPage};
use tessera_ui::game::npc::{FallbackArt, NpcAction, NpcWindowKind};
use ui_common::Rig;

/// Longest wait for the UI to settle before a picture (a capture after this is a failure,
/// decision 0031).
const SETTLE_LIMIT: Duration = Duration::from_secs(120);

struct Install {
    dir: PathBuf,
    data: Arc<CharacterData>,
    npc: Arc<NpcData>,
    refs: Arc<NpcRefs>,
    rig: Rig,
    version: String,
    _skins: tempfile::TempDir,
}

fn install(dir: PathBuf) -> Install {
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let npc = Arc::new(NpcData::new(Arc::clone(&data)).expect("NPC data"));
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let refs = Arc::new(NpcRefs::load(&set).expect("NPC window names"));
    assert!(
        refs.missing.is_empty(),
        "LayoutMissing: {:?} (NWI1)",
        refs.missing
    );
    let skins = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(skins.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    let version = goldens::install_version(&dir);
    Install {
        dir,
        data,
        npc,
        refs,
        rig,
        version,
        _skins: skins,
    }
}

/// Runs frames until the UI host has settled (assets loaded, every surface painted).
fn wait_settled(rig: &mut Rig) {
    let start = Instant::now();
    loop {
        rig.frame();
        if rig.host.settled() {
            return;
        }
        assert!(
            start.elapsed() < SETTLE_LIMIT,
            "the UI did not settle within {SETTLE_LIMIT:?}; no picture is taken (decision 0031)"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Pictures of every window shown, checked against their goldens.
struct Pictures {
    dir: PathBuf,
    flags: goldens::Flags,
    seq: usize,
    results: Vec<(String, &'static str)>,
}

impl Pictures {
    fn new(test: &str, version: &str) -> Self {
        let dir = goldens::golden_dir(env!("CARGO_PKG_NAME"), test, version)
            .unwrap_or_else(|e| panic!("{e}"));
        // Sheets go to a folder per test, so names of different tests never collide.
        let mut flags = goldens::Flags::from_env();
        flags.sheet = flags.sheet.map(|s| s.join(test));
        Self {
            dir,
            flags,
            seq: 0,
            results: Vec::new(),
        }
    }

    fn take(&mut self, rig: &mut Rig, root: tessera_ui::ElementKey, what: &str) {
        wait_settled(rig);
        self.seq += 1;
        let name = format!("{:02}-{what}", self.seq);
        let (w, h, data) = {
            let mut ui = rig.host.ui_mut();
            capture(&mut ui, root).unwrap_or_else(|| panic!("{name}: the window has no surface"))
        };
        let img = goldens::Rgba {
            width: w,
            height: h,
            data,
        };
        match goldens::check_with(&self.dir, &name, &img, &self.flags) {
            Ok(o) => {
                println!("{name}: {} ({w}x{h})", o.word());
                self.results.push((name, o.word()));
            }
            Err(e) => panic!("{e}"),
        }
    }
}

fn kind_word(k: NpcWindowKind) -> &'static str {
    k.panel_name()
}

/// Whether a poll changed the NPC windows.
fn windows_changed(changes: &[PlayerChange]) -> bool {
    changes.iter().any(|c| {
        matches!(
            c,
            PlayerChange::Npc(
                NpcChange::Shown { .. }
                    | NpcChange::Rewards { .. }
                    | NpcChange::Buyback { .. }
                    | NpcChange::LootTaken { .. }
            )
        )
    })
}

fn print_census(name: &str, c: &NpcCensus) {
    println!(
        "{name}: lines {:?}; entries {:?}; wallet shown {}",
        c.lines, c.entries, c.wallet_shown
    );
}

/// Plays `scenario` on the install with the windows on the UI; pictures every window a
/// poll changed; `extra` runs after each poll's pictures for more states.
fn run(
    i: &mut Install,
    scenario: &str,
    test: &str,
    mut extra: impl FnMut(
        &mut Rig,
        &mut NpcUi,
        &tessera_client::character::PlayerState,
        &NpcData,
        &mut Pictures,
    ),
) -> (NpcUi, Pictures) {
    let mut npc = NpcUi::new(Arc::clone(&i.refs), FallbackArt::default());
    let mut pics = Pictures::new(test, &i.version);
    let data = Arc::clone(&i.npc);
    let rig = &mut i.rig;
    let dir = i.dir.clone();
    let (outcome, _, _) = play(scenario, Some(&dir), Some(i.data.bags()), |p| {
        rig.frame();
        let actions = {
            let mut ui = rig.host.ui_mut();
            npc.frame(&mut ui, p.state, Some(&data), &[], None)
        };
        // The windows send nothing by themselves in these scenarios.
        assert!(
            actions.iter().filter_map(|a| command_of(*a)).count() == 0,
            "unexpected commands {actions:?}"
        );
        if windows_changed(p.changes) {
            let open: Vec<(u32, NpcWindowKind, tessera_ui::ElementKey)> =
                npc.open_windows().collect();
            for (w, k, root) in open {
                pics.take(rig, root, &format!("w{w}-{}", kind_word(k)));
            }
            extra(rig, &mut npc, p.state, &data, &mut pics);
        }
    });
    assert_eq!(outcome.exit_code(), 0, "{scenario}: {outcome:?}");
    assert!(npc.failures().is_empty(), "failures: {:?}", npc.failures());
    (npc, pics)
}

fn entries(c: &NpcCensus, k: EntryCategory) -> usize {
    c.entries.get(&k).copied().unwrap_or(0)
}

fn lines(c: &NpcCensus, k: LineCategory) -> usize {
    c.lines.get(&k).copied().unwrap_or(0)
}

#[test]
fn dialogue_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir);
    let mut action_rows = 0usize;
    let mut action_icons = 0usize;
    let mut item_rewards = 0usize;
    let (npc, pics) = run(
        &mut i,
        "dialogue",
        "npc_windows_real_data/dialogue_on_the_install",
        |rig, npc, state, data, _| {
            // §6.3: an item reward row of `QuestReward_Base` shows its item display only; the
            // other parts (which carry the layout's sample texts) are hidden.
            let ids = npc.refs().nv.ids;
            for (w, _, _) in npc.open_windows().collect::<Vec<_>>() {
                let Some(rows) = npc.offer(w).and_then(|o| o.rows.as_ref()) else {
                    continue;
                };
                let ui = rig.host.ui();
                for (r, _) in &rows.rows {
                    let part = |id| npc_windows_common::part(&ui, *r, id);
                    let Some(_) = part(ids.reward_item_part).filter(|k| ui.is_visible(*k)) else {
                        continue;
                    };
                    item_rewards += 1;
                    for other in [
                        ids.reward_trait_part,
                        ids.reward_text_part,
                        ids.reward_billing_part,
                        ids.reward_runic_part,
                        ids.reward_image_part,
                    ] {
                        assert!(
                            part(other).is_none_or(|k| !ui.is_visible(k)),
                            "an item reward row hides part {other:#x}"
                        );
                    }
                }
            }
            for (w, k, _) in npc.open_windows().collect::<Vec<_>>() {
                if k == NpcWindowKind::Choice
                    && let Some(d) = npc.dialogue(w)
                {
                    action_rows = action_rows.max(d.rows.len());
                    // §5.3 rule 3: an action option's row shows its template's icon.
                    let Some(fw) = state.npc().window(tessera_net::domain::WindowId(w)) else {
                        continue;
                    };
                    let WindowContent::Dialogue {
                        speaker,
                        text,
                        options,
                    } = &fw.content
                    else {
                        continue;
                    };
                    let mut census = NpcCensus::default();
                    let (_, _, templates) = Present::new(Some(data), state).dialogue(
                        w,
                        *speaker,
                        text,
                        options,
                        &mut census,
                    );
                    let rows = npc.dialogue(w).map(|d| d.rows.clone()).unwrap_or_default();
                    let icon_id = npc.refs().nv.ids.action_icon;
                    let misc = npc.refs().templates.choice.row.map(|t| t.element);
                    for (option, template) in templates {
                        let mut ui = rig.host.ui_mut();
                        let want = npc.action_icon(&mut ui, template);
                        assert!(want.is_some(), "template {template:#x} has an icon");
                        if Some(template) != misc {
                            let generic = misc.and_then(|m| npc.action_icon(&mut ui, m));
                            assert_ne!(want, generic, "template {template:#x}: its own icon");
                        }
                        let row = rows.iter().find(|(_, o)| *o == option).map(|(k, _)| *k);
                        let shown = row
                            .and_then(|r| npc_windows_common::part(&ui, r, icon_id))
                            .and_then(|k| ui.base_image(k))
                            .map(|(i, _)| i);
                        assert_eq!(shown, want, "option {option} shows its action icon");
                        action_icons += 1;
                    }
                }
            }
        },
    );
    assert!(action_icons >= 1, "an action row with its template icon");
    assert!(
        item_rewards >= 2,
        "the offer's install item rewards ({item_rewards})"
    );
    let c = npc.census();
    print_census("dialogue", c);
    assert!(lines(c, LineCategory::Shown) >= 4, "install lines shown");
    assert!(action_rows >= 2, "choice rows");
    // Choice (greeting), conversation, comment, quest offer, choice (quest role).
    let kinds: Vec<&str> = pics.results.iter().map(|(n, _)| n.as_str()).collect();
    println!("pictures: {kinds:?}");
    for want in ["choice", "conversation", "npc-comment"] {
        assert!(
            kinds.iter().any(|n| n.ends_with(want)),
            "a {want} window was pictured"
        );
    }
    println!(
        "dialogue: {} pictures ({} blessed, {} passed)",
        pics.results.len(),
        pics.results.iter().filter(|r| r.1 == "blessed").count(),
        pics.results.iter().filter(|r| r.1 == "passed").count()
    );
}

#[test]
fn vendor_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir);
    let mut extra_done = false;
    let mut barter_rows_checked = 0usize;
    let (npc, pics) = run(
        &mut i,
        "vendor",
        "npc_windows_real_data/vendor_on_the_install",
        |rig, npc, state, data, pics| {
            // §7.7: every barter reward is an item, so each entry's reputation alternative
            // (with the layout's sample lines) stays hidden.
            for (w, k, _) in npc.open_windows().collect::<Vec<_>>() {
                if k != NpcWindowKind::Barter {
                    continue;
                }
                let rep = npc.refs().nv.ids.barter_reputation_reward;
                let ui = rig.host.ui();
                for (r, _) in npc.barter(w).map(|b| b.rows.clone()).unwrap_or_default() {
                    if let Some(f) = npc_windows_common::part(&ui, r, rep) {
                        assert!(!ui.is_visible(f), "barter entry: reputation field hidden");
                        barter_rows_checked += 1;
                    }
                }
            }
            // Once the install list and its buy-back page are shown: the sell and buy-back
            // pages and the quantity dialog (§7.1, §7.5, §7.8).
            let Some(v) = npc.vendor(1) else { return };
            if extra_done || v.buy.len() != 6 || v.buyback.as_ref().is_none_or(|b| b.len() != 2) {
                return;
            }
            extra_done = true;
            assert!(
                v.buy.iter().all(|e| !e.name.is_empty() && e.icon.is_some()),
                "install items with names and icons"
            );
            let root = npc.root(1).expect("root");
            for (page, word) in [(VendorPage::Sell, "sell"), (VendorPage::BuyBack, "buyback")] {
                {
                    let mut ui = rig.host.ui_mut();
                    assert!(npc.show_vendor_page(&mut ui, 1, page));
                }
                pics.take(rig, root, &format!("w1-vendor-{word}"));
            }
            {
                let mut ui = rig.host.ui_mut();
                npc.show_vendor_page(&mut ui, 1, VendorPage::Buy);
            }
            // Shift held on the active vendor window: stack mode buys a full stack (§7.4
            // case 3, §13.2).
            let (entry, stack) = npc
                .vendor(1)
                .and_then(|v| v.buy.iter().find(|e| e.stack_size > 1))
                .map(|e| (e.entry, e.stack_size))
                .expect("a stackable entry");
            let row = {
                let v = npc.vendor(1).expect("vendor");
                let i = v.buy.iter().position(|e| e.entry == entry).expect("row");
                v.rows(VendorPage::Buy)[i]
            };
            let actions = {
                let mut ui = rig.host.ui_mut();
                ui.activate(root);
                let stack_mode = BuyToggles {
                    stack: true,
                    ..BuyToggles::default()
                };
                npc.set_buy_toggles(&mut ui, stack_mode, Some(data));
                let ev = npc_windows_common::right(&ui, row);
                let a = npc.frame(&mut ui, state, Some(data), &[ev], None);
                npc.set_buy_toggles(&mut ui, BuyToggles::default(), Some(data));
                a
            };
            assert_eq!(
                actions,
                vec![NpcAction::Buy {
                    window: 1,
                    entry,
                    quantity: stack
                }],
                "Shift + right click buys a stack"
            );
            {
                let mut ui = rig.host.ui_mut();
                assert!(npc.ask_quantity(&mut ui, 1, v_entry(npc), state, Some(data)));
            }
            let q = npc.quantity_root(1).expect("quantity dialog");
            wait_settled(rig);
            let b = rig.host.ui().displayed_box(q);
            assert!(
                b.w() >= 256 && b.h() >= 164,
                "the install's quantity dialog is below its minimum size ({}x{}; §4: design \
                 295 x 45, minimum 256 x 164)",
                b.w(),
                b.h()
            );
            pics.take(rig, q, "w1-vendor-quantity");
            // Escape on the active dialog closes it without buying (§7.5 rule 6).
            let mut ui = rig.host.ui_mut();
            ui.activate(q);
            assert!(npc.takes_key(&ui, NpcKey::Escape));
            assert!(npc.key(&mut ui, NpcKey::Escape, Some(data)).is_empty());
            assert!(npc.quantity_root(1).is_none(), "the dialog closed");
        },
    );
    assert!(extra_done, "the install vendor list was shown");
    assert!(
        barter_rows_checked >= 3,
        "barter entries with the install's reward template ({barter_rows_checked})"
    );
    let c = npc.census();
    print_census("vendor", c);
    assert!(
        entries(c, EntryCategory::Shown) >= 8,
        "install entries shown"
    );
    assert_eq!(entries(c, EntryCategory::RecordUnreadable), 0);
    let kinds: Vec<&str> = pics.results.iter().map(|(n, _)| n.as_str()).collect();
    println!("pictures: {kinds:?}");
    assert!(
        kinds.iter().any(|n| n.ends_with("barter")),
        "the barter window"
    );
}

/// The first stackable buy entry of vendor window 1.
fn v_entry(npc: &NpcUi) -> u16 {
    npc.vendor(1)
        .and_then(|v| v.buy.iter().find(|e| e.stack_size > 1).or(v.buy.first()))
        .map_or(1, |e| e.entry)
}

/// Windows shown before the install facts load are filled again when they arrive (the
/// windowed client loads the facts on a worker after the P1 facts).
#[test]
fn facts_arriving_later_refill_the_windows() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir);
    let mut npc = NpcUi::new(Arc::clone(&i.refs), FallbackArt::default());
    let data = Arc::clone(&i.npc);
    let rig = &mut i.rig;
    let dir2: &Path = &i.dir;
    let mut polls_with_list = 0usize;
    let mut checked = false;
    let (outcome, _, _) = play("vendor", Some(dir2), Some(i.data.bags()), |p| {
        rig.frame();
        let shown = npc.vendor(1).is_some_and(|v| v.buy.len() == 6);
        if shown {
            polls_with_list += 1;
        }
        // The facts arrive two polls after the install list was first shown.
        let facts = (polls_with_list > 2).then_some(&*data);
        let mut ui = rig.host.ui_mut();
        let _ = npc.frame(&mut ui, p.state, facts, &[], None);
        if facts.is_none() && shown {
            assert!(
                npc.vendor(1)
                    .is_some_and(|v| v.buy.iter().all(|e| e.name.is_empty())),
                "no names before the facts"
            );
        }
        if facts.is_some() && !checked && npc.vendor(1).is_some_and(|v| v.buy.len() == 6) {
            checked = true;
            assert!(
                npc.vendor(1)
                    .is_some_and(|v| v.buy.iter().all(|e| !e.name.is_empty() && e.icon.is_some())),
                "names and icons once the facts are there"
            );
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert!(
        checked,
        "the vendor list was shown with and without the facts"
    );
}

#[test]
fn loot_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir);
    let mut install_rows = 0usize;
    let (npc, pics) = run(
        &mut i,
        "loot",
        "npc_windows_real_data/loot_on_the_install",
        |_, npc, _, _, _| {
            if let Some(l) = npc.loot(2) {
                install_rows = install_rows.max(l.rows.len());
            }
        },
    );
    assert!(install_rows >= 4, "the chest's install entries");
    let c = npc.census();
    print_census("loot", c);
    assert!(
        entries(c, EntryCategory::Shown) >= 5,
        "install entries shown"
    );
    assert_eq!(entries(c, EntryCategory::RecordUnreadable), 0);
    println!("pictures: {}", pics.results.len());
}

#[test]
fn wallet_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir);
    let mut pics = Pictures::new("npc_windows_real_data/wallet_on_the_install", &i.version);
    let mut npc = NpcUi::new(Arc::clone(&i.refs), FallbackArt::default());
    let data = Arc::clone(&i.npc);
    let rig = &mut i.rig;
    let dir2: &Path = &i.dir;
    let mut root = None;
    let (outcome, _, state) = play("vendor", Some(dir2), Some(i.data.bags()), |p| {
        rig.frame();
        let mut ui = rig.host.ui_mut();
        if root.is_none() && p.state.player().is_some() {
            root = Some(
                npc.open_panel(&mut ui, NpcWindowKind::Wallet, p.state, Some(&data))
                    .expect("wallet window"),
            );
        }
        let _ = npc.frame(&mut ui, p.state, Some(&data), &[], None);
    });
    assert_eq!(outcome.exit_code(), 0);
    let root = root.expect("opened");
    pics.take(rig, root, "wallet");
    let c = npc.census();
    print_census("wallet", c);
    assert!(c.wallet_shown >= 1, "the token is listed");
    assert!(state.coin().is_some());
}
