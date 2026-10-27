// SPDX-License-Identifier: GPL-3.0-or-later
//! P4 exit criterion 1 (decision 0024 P4): "Fellowship, trade and friends scenarios render;
//! leader change, removal and disbanding are shown." The committed scenarios
//! `tests/scenarios/fellowship.scenario`, `trade.scenario` and `friends.scenario` play
//! headless on their virtual clock against the user's install (read-only; selectors resolve,
//! nothing of a DID is written in the files); every poll's events reach the client's social
//! state and player state as the world session hands them over, and the client's game-UI host
//! draws the social panels from the install's layouts (`social-panels.md` §3–§6). At
//! checkpoints the test checks what the panels show (rows, names, leader markers, the trade
//! sides, their icons and ready states) and, with a GPU adapter, renders the screen offscreen
//! and counts the covered pixels of the panel's rectangle against the background: a shown
//! panel covers a good part of its rectangle, a hidden one (after the disbanding, after the
//! trade) none. With `TESSERA_SHEET_DIR` set the pictures are written there (never committed:
//! they show the install's art). The fellowship and trade pictures are also compared with
//! local-only goldens (decision 0031 tier 2, `ui_common/goldens.rs`: `TESSERA_GOLDEN_DIR`,
//! keyed by package, test binary, function and install version; blessed when missing,
//! `TESSERA_GOLDEN_REQUIRE=1` makes a missing golden a failure). Revision 1 of
//! `social-panels.md` is checked on the install: entry names in `ItemDisplay_Name` (§6.2),
//! the wallet and the peer's coin as coin elements (§6.1), and no voice, squelched or
//! sending-data icon on the member rows or the player's portrait (TR14). Only counts and
//! invented names are printed.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test social_panels_real_data -- --nocapture`.

#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

#[path = "../../../crates/render/tests/support/golden.rs"]
mod golden;
#[path = "ui_common/goldens.rs"]
mod goldens;
mod ui_common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::character::{CharacterData, PlayerState};
use tessera_client::cli::Args;
use tessera_client::net::ChatLine;
use tessera_client::panels::PanelId;
use tessera_client::scenario::InstallChoice;
use tessera_client::social::SocialState;
use tessera_client::social::ui::SocialOpen;
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_net::scenario::{HeadlessDriver, InputPlayer, LogTime};
use tessera_net::{EntityId, Event, Session};
use tessera_render::{OffscreenFormat, OffscreenTarget, PassSpace, Renderer, RendererConfig};
use tessera_ui::game::find_below;
use tessera_ui::{ElementKey, IBox};
use ui_common::Rig;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// The client's state as the runtime keeps it: the social state, the player state and
/// the combat state (its changes go to the combat widgets once they are attached).
struct Client {
    social: SocialState,
    player: PlayerState,
    combat: tessera_client::combat::CombatState,
    combat_changes: Vec<tessera_client::combat::CombatChange>,
    combat_install: Option<Arc<tessera_client::combat::CombatInstall>>,
    start: Instant,
    names: std::collections::BTreeMap<EntityId, String>,
    lines: Vec<ChatLine>,
    /// The same events through a social state without the install's tables (Tessera's own
    /// wording), to tell the install's lines from the fallback.
    plain: SocialState,
    plain_lines: Vec<ChatLine>,
}

impl Client {
    fn new(data: &Arc<CharacterData>) -> Self {
        let mut social = SocialState::new(true);
        tessera_client::social::attach_data(&mut social, data);
        Self {
            social,
            player: PlayerState::new(data.bags()),
            combat: tessera_client::combat::CombatState::new(),
            combat_changes: Vec::new(),
            combat_install: None,
            start: Instant::now(),
            names: std::collections::BTreeMap::new(),
            lines: Vec::new(),
            plain: SocialState::new(true),
            plain_lines: Vec::new(),
        }
    }

    /// A command the session accepted (as the client's world notes it).
    fn sent(&mut self, c: &tessera_net::Command) {
        self.social.note_command(c);
        self.plain.note_command(c);
    }

    fn apply(&mut self, ev: &Event) {
        if let Event::EntitySpawned { id, name, .. } | Event::EntityCreated { id, name, .. } = ev {
            self.names.insert(*id, name.clone());
        }
        let mut changes = Vec::new();
        self.player.apply(ev, &mut changes);
        let now = self.start.elapsed().as_secs_f64();
        self.combat.apply(ev, now, &mut self.combat_changes);
        let names = &self.names;
        self.lines
            .extend(self.social.apply(ev, &|id| names.get(&id).cloned()));
        self.plain_lines
            .extend(self.plain.apply(ev, &|id| names.get(&id).cloned()));
        if let Some(c) = self.player.coin() {
            self.social.set_wallet(c);
        }
    }
}

/// The offscreen picture of the rig's UI.
struct Picture {
    target: OffscreenTarget,
}

impl Picture {
    fn new(rig: &mut Rig) -> Option<Self> {
        let r = rig.renderer.as_mut()?;
        let (w, h) = ui_common::SCREEN;
        let target = r
            .create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)
            .ok()?;
        Some(Self { target })
    }

    /// Draws until the UI has settled; returns (width, height, RGBA bytes).
    fn take(
        &self,
        rig: &mut Rig,
        client: &mut Client,
        data: &CharacterData,
    ) -> (u32, u32, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut image = None;
        while Instant::now() < deadline {
            frame(rig, client, data);
            let renderer = rig.renderer.as_mut().expect("renderer");
            let mut f = renderer.begin_offscreen_frame(&self.target).expect("frame");
            f.pass("scene", PassSpace::Screen)
                .clear([0.05, 0.05, 0.08, 1.0]);
            rig.host.draw(&mut f, false);
            renderer.end_frame(f).expect("end frame");
            image = Some(renderer.read_pixels(&self.target).expect("read back"));
            if rig.host.settled() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let i = image.expect("a frame");
        (i.width, i.height, i.data)
    }
}

/// One frame of the rig with the social panels (as the windowed client runs them after
/// step 7, with the frame's one list of UI events).
fn frame(rig: &mut Rig, client: &mut Client, data: &CharacterData) {
    rig.frame();
    // The combat frame after the character panels, as the client runs it (nothing while
    // the combat widgets are not attached).
    let changes = std::mem::take(&mut client.combat_changes);
    let now = client.start.elapsed().as_secs_f64();
    let anchors =
        tessera_client::combat::anchors::SyntheticAnchors::new(&client.combat, ui_common::SCREEN);
    let _ = rig.host.combat_frame(
        &client.combat,
        &changes,
        Vec::new(),
        client.combat_install.as_deref(),
        now,
        &anchors,
    );
    let events = rig.host.take_game_events();
    let _ = rig.host.social_frame(
        &mut client.social,
        Some(data),
        Some(&client.player),
        &events,
    );
}

/// The share of `b`'s pixels that differ from the background `bg`.
fn covered(img: &(u32, u32, Vec<u8>), b: IBox, bg: [u8; 3]) -> f64 {
    let (w, h, px) = img;
    let (mut n, mut hit) = (0u64, 0u64);
    for y in b.t.max(0)..=b.b.min(*h as i32 - 1) {
        for x in b.l.max(0)..=b.r.min(*w as i32 - 1) {
            let i = (y as usize * *w as usize + x as usize) * 4;
            n += 1;
            let d = (0..3).map(|c| px[i + c].abs_diff(bg[c])).max().unwrap_or(0);
            if d > 12 {
                hit += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { hit as f64 / n as f64 }
}

fn background(img: &(u32, u32, Vec<u8>)) -> [u8; 3] {
    let (w, h, px) = img;
    // The bottom-right corner: no social panel lies there.
    let i = ((*h as usize - 1) * *w as usize + (*w as usize - 1)) * 4;
    [px[i], px[i + 1], px[i + 2]]
}

/// A picture: (width, height, RGBA bytes).
type Shot = (u32, u32, Vec<u8>);

/// Compares a GPU picture with its golden under the render crate's default tolerance.
fn gpu_compare(want: &goldens::Rgba, got: &goldens::Rgba) -> (bool, String, goldens::Rgba) {
    let image = |g: &goldens::Rgba| tessera_render::RgbaImage {
        width: g.width,
        height: g.height,
        data: g.data.clone(),
    };
    let tol = golden::Tolerance::default();
    let (c, diff) = golden::compare(&image(want), &image(got), tol);
    (
        c.passes(tol),
        format!(
            "{} outliers, {} strict mismatches of {} pixels, max difference {}",
            c.outliers, c.strict_mismatches, c.pixels, c.max_diff
        ),
        goldens::Rgba {
            width: diff.width,
            height: diff.height,
            data: diff.data,
        },
    )
}

fn save(name: &str, img: &(u32, u32, Vec<u8>)) {
    let Some(out) = std::env::var_os("TESSERA_SHEET_DIR") else {
        return;
    };
    let path = PathBuf::from(out).join(format!("p4-{name}.png"));
    let file = std::fs::File::create(&path).expect("sheet file");
    let mut enc = png::Encoder::new(file, img.0, img.1);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(&img.2))
        .expect("write sheet");
    eprintln!("sheet written: {}", path.display());
}

/// Plays `scenario` headless against the install, calling `at(ms, …)` once at the first
/// poll at or after each checkpoint.
fn play(
    dir: &Path,
    scenario: &str,
    checkpoints: &[u64],
    rig: &mut Rig,
    client: &mut Client,
    data: &CharacterData,
    at: impl FnMut(u64, &mut Rig, &mut Client),
) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/scenarios")
        .join(format!("{scenario}.scenario"));
    play_path(dir, &path, checkpoints, rig, client, data, at);
}

/// [`play`] for a scenario file at `path`.
fn play_path(
    dir: &Path,
    path: &Path,
    checkpoints: &[u64],
    rig: &mut Rig,
    client: &mut Client,
    data: &CharacterData,
    mut at: impl FnMut(u64, &mut Rig, &mut Client),
) {
    let scenario = path.display();
    let args = Args::try_parse_from([
        "tessera-client",
        "--headless",
        "--scenario",
        path.to_str().expect("utf-8"),
    ])
    .expect("args");
    let sc = Arc::new(
        tessera_client::scenario::prepare(&args, &InstallChoice::Found(dir.to_path_buf()))
            .expect("the scenario resolves against the install"),
    );
    let config = tessera_client::scenario::headless_config(&args, Arc::clone(&sc)).expect("config");
    let backend = tessera_net::sandbox::SandboxBackend::new(config).expect("sandbox");
    // The input script's commands as the windowed client's world sends them (the headless
    // driver sends them itself): a second player over the same events yields the same
    // commands at the same ticks, and the social state notes them (TR4).
    let mut input = InputPlayer::new(Arc::clone(&sc));
    let tick = sc.tick_ms().max(1);
    let mut driver = HeadlessDriver::<Vec<u8>>::new(Session::new(backend), sc, None);
    let mut next = 0usize;
    let outcome = driver.run_frames(|f| {
        let due: Vec<_> = match f.time {
            LogTime::At(ms) => std::iter::from_fn(|| input.next_due(ms / tick)).collect(),
            _ => std::iter::from_fn(|| input.next_pre()).collect(),
        };
        for c in due.into_iter().filter_map(|l| l.command) {
            client.sent(&c);
        }
        for ev in f.events {
            input.observe(ev);
            client.apply(ev);
        }
        frame(rig, client, data);
        if let LogTime::At(ms) = f.time {
            while next < checkpoints.len() && ms >= checkpoints[next] {
                at(checkpoints[next], rig, client);
                next += 1;
            }
        }
    });
    assert_eq!(outcome.exit_code(), 0, "{scenario}: {outcome:?}");
    assert_eq!(
        next,
        checkpoints.len(),
        "{scenario}: every checkpoint reached"
    );
}

fn text_of(host: &UiHost, root: ElementKey, id: u32) -> Option<String> {
    let mut ui = host.ui_mut();
    find_below(&ui, root, id).map(|k| ui.text_of(k))
}

fn visible(host: &UiHost, root: ElementKey, id: u32) -> bool {
    let ui = host.ui();
    find_below(&ui, root, id).is_some_and(|k| ui.is_visible(k))
}

#[test]
fn social_scenarios_render_on_the_install() {
    // One GPU test at a time in this binary (`gpu.rs`), held for the whole test.
    let _gpu = gpu::GpuLock::acquire();
    let Some(dir) = game_dir() else { return };
    // Decision 0031 tier 2: refuse a golden folder in a work tree before any work.
    let golden_dir = goldens::golden_dir(
        env!("CARGO_PKG_NAME"),
        "social_panels_real_data/social_scenarios_render_on_the_install",
        &goldens::install_version(&dir),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut flags = goldens::Flags::from_env();
    flags.sheet = None;
    let mut shots: Vec<(String, Shot)> = Vec::new();
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let mut rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    if let Ok(r) = Renderer::new_headless(&RendererConfig::default()) {
        rig.renderer = Some(r);
    }
    // The player-vitals panel holds `PlayerLeaderIcon` (social-panels.md §4.3).
    let p4 = rig
        .host
        .open_panel(&mut rig.plugins, PanelId::PlayerVitals)
        .expect("P4 opens");
    let picture = Picture::new(&mut rig);
    if picture.is_none() {
        eprintln!("no GPU adapter; the panels are checked, not drawn");
    }
    let ids = rig.host.social().expect("social panels").vocab().ids;

    // ----- fellowship: formed, grown, leader change, removal, disbanding ---------------
    let mut client = Client::new(&data);
    let mut panel_box: Option<IBox> = None;
    let mut first_cover: Option<f64> = None;
    let mut pictures = 0usize;
    let checks = [900, 1300, 2600, 4600, 6300, 8200];
    play(
        &dir,
        "fellowship",
        &checks,
        &mut rig,
        &mut client,
        &data,
        |ms, rig, client| {
            if ms == 900 {
                // The invitation window (TR7) shows the inviter; the player accepts it with a
                // click on its Accept button (the input script answers the feed at 1 s).
                let (root, accept) = {
                    let s = rig.host.social().expect("social panels");
                    let w = s.invite().expect("invitation window");
                    let ui = rig.host.ui();
                    assert!(ui.is_visible(w.root), "the invitation is shown");
                    let k = find_below(&ui, w.root, ids.invite_accept).expect("Accept");
                    (w.root, ui.displayed_box(k))
                };
                if let Some(p) = picture.as_ref() {
                    let img = p.take(rig, client, &data);
                    save("invitation-900", &img);
                    pictures += 1;
                }
                let (x, y) = (
                    f64::from(accept.l + (accept.r - accept.l) / 2),
                    f64::from(accept.t + (accept.b - accept.t) / 2),
                );
                // The window fades in when shown (its auto-fade, UB §11.1); a fading window is
                // not hit (§4.2).
                let deadline = Instant::now() + Duration::from_secs(5);
                while rig.host.ui().is_fading(root) {
                    assert!(Instant::now() < deadline, "the invitation window fades in");
                    frame(rig, client, &data);
                    std::thread::sleep(Duration::from_millis(10));
                }
                rig.move_to(x, y);
                frame(rig, client, &data);
                rig.click(x, y);
                frame(rig, client, &data);
                assert!(
                    !client.social.invitation().shown(),
                    "Accept answers the invitation (§5.2)"
                );
                // The window fades out (its auto-fade, UB §11.1), then hides.
                let deadline = Instant::now() + Duration::from_secs(5);
                while rig.host.ui().is_visible(root) {
                    assert!(Instant::now() < deadline, "the invitation window hides");
                    frame(rig, client, &data);
                    std::thread::sleep(Duration::from_millis(10));
                }
                return;
            }
            let s = rig.host.social().expect("social panels");
            let f = s.fellowship().expect("fellowship panels");
            let panel1 = *f.panels.first().expect("panel 1");
            let shown = rig.host.ui().is_visible(panel1);
            let names: Vec<String> = f
                .rows
                .iter()
                .filter_map(|r| text_of(&rig.host, *r, ids.fellow_name))
                .collect();
            let leaders: Vec<bool> = f
                .rows
                .iter()
                .map(|r| visible(&rig.host, *r, ids.fellow_leader))
                .collect();
            let player_leads = f.player_leader.is_some_and(|k| rig.host.ui().is_visible(k));
            eprintln!(
                "fellowship at {ms} ms: shown {shown}, rows {names:?}, leader markers {leaders:?}, \
             player leads {player_leads}"
            );
            match ms {
                1300 => {
                    assert!(shown);
                    assert_eq!(names, ["Brin"]);
                    // TR14: no voice, squelched or sending-data icon (the red and green
                    // one) on the rows or the portrait; no volume control.
                    let voice = [
                        ids.fellow_voice,
                        ids.fellow_voice_squelched,
                        ids.fellow_sending,
                        ids.fellow_volume,
                    ];
                    for r in &f.rows {
                        for id in voice {
                            assert!(!visible(&rig.host, *r, id), "TR14: {id:#x} on a row");
                        }
                    }
                    for id in [
                        ids.player_voice,
                        ids.player_voice_squelched,
                        ids.player_sending,
                        ids.player_assistant,
                    ] {
                        assert!(!visible(&rig.host, p4, id), "TR14: {id:#x} on P4");
                    }
                    assert_eq!(leaders, [false]);
                    assert!(
                        player_leads,
                        "PlayerLeaderIcon while the player leads (SPI6)"
                    );
                    // Like the trade window below: no picture of a panel that is still
                    // fading in (decision 0031).
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while rig.host.ui().is_fading(panel1) {
                        assert!(Instant::now() < deadline, "the fellowship panel fades in");
                        frame(rig, client, &data);
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                2600 => {
                    assert_eq!(
                        names,
                        ["Brin", "Cael", "Eda"],
                        "the player gets no row (SPI6)"
                    );
                    assert!(player_leads);
                    let rows = client.social.fellowship().rows();
                    let eda = rows.iter().find(|r| r.name == "Eda").expect("Eda");
                    assert!(
                        eda.morale.is_some_and(|v| (v.fill() - 0.5).abs() < 1e-6),
                        "a member without an entity shows its FellowVitalChanged (TR3)"
                    );
                }
                4600 => {
                    assert_eq!(names, ["Brin", "Cael", "Eda"]);
                    assert_eq!(leaders, [true, false, false], "the leader change is shown");
                    assert!(!player_leads);
                }
                6300 => {
                    assert_eq!(names, ["Brin", "Eda"], "the removal is shown");
                    assert_eq!(leaders, [true, false]);
                }
                _ => {
                    assert!(!shown, "the disbanding hides the fellowship panel (TR2)");
                    assert!(names.is_empty() || !shown);
                }
            }
            if let Some(p) = picture.as_ref() {
                let img = p.take(rig, client, &data);
                let b = *panel_box.get_or_insert_with(|| rig.host.ui().displayed_box(panel1));
                let c = covered(&img, b, background(&img));
                eprintln!(
                    "fellowship at {ms} ms: {:.0} % of panel 1's rectangle covered",
                    c * 100.0
                );
                let first = *first_cover.get_or_insert(c);
                if ms < 8200 {
                    assert!(c > 0.05, "the fellowship panel is drawn");
                } else {
                    // Only the corner of the player-vitals panel above it is left.
                    assert!(
                        c < 0.1 && c < first * 0.5,
                        "the fellowship panel is gone after the disbanding ({c} vs {first})"
                    );
                }
                save(&format!("fellowship-{ms}"), &img);
                shots.push((format!("fellowship-{ms:05}ms"), img));
                pictures += 1;
            }
        },
    );
    let notices = client
        .lines
        .iter()
        .filter(|l| l.shown.as_ref().is_some_and(|(m, _)| *m == 0x400))
        .count();
    eprintln!("fellowship lines (chat type 11): {notices}");
    assert!(
        notices >= 7,
        "the derived notices (TR4) and the fellowship chat lines (CT1)"
    );
    // The lines come from the install's tables (CT12), not Tessera's own wording: the same
    // lines rendered without tables differ (the plain state replays the same events).
    let differ = client
        .lines
        .iter()
        .zip(&client.plain_lines)
        .filter(|(a, b)| a.shown.is_some() && a.display_text() != b.display_text())
        .count();
    eprintln!("fellowship lines rendered from the install: {differ}");
    assert!(
        differ > 0,
        "CT1 lines come from the install's string tables"
    );
    // The input script's kick (TR4): the dismissed form (Tessera's wording in the plain
    // replay: "... was removed from the group."), not the left form.
    let dismissed = client
        .plain_lines
        .iter()
        .filter(|l| l.display_text().contains("was removed from the group"))
        .count();
    assert_eq!(
        dismissed, 1,
        "the kick of Cael derives the dismissed line (TR4)"
    );

    // ----- trade: offers with the install's icons, ready states, completion ------------
    let mut client = Client::new(&data);
    let mut trade_box: Option<IBox> = None;
    let checks = [1100, 1600, 2800, 3800, 4300, 4700];
    play(
        &dir,
        "trade",
        &checks,
        &mut rig,
        &mut client,
        &data,
        |ms, rig, client| {
            let s = rig.host.social().expect("social panels");
            let t = s.trade().expect("trade window");
            let troot = t.root;
            let shown = rig.host.ui().is_visible(troot);
            let (own, peer) = (t.entries.0.len(), t.entries.1.len());
            let model = client.social.trade();
            eprintln!(
                "trade at {ms} ms: shown {shown}, own {own}, peer {peer}, ready {}/{}",
                model
                    .offer(tessera_ui::game::social::trade::Side::Own)
                    .ready,
                model
                    .offer(tessera_ui::game::social::trade::Side::Peer)
                    .ready
            );
            match ms {
                1100 => {
                    assert!(shown);
                    assert_eq!(peer, 2);
                    // The window fades in when shown (its auto-fade, UB §11.1): the picture
                    // is taken once the fade is over, so it does not depend on the wall
                    // clock (decision 0031).
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while rig.host.ui().is_fading(troot) {
                        assert!(Instant::now() < deadline, "the trade window fades in");
                        frame(rig, client, &data);
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                1600 => {
                    assert_eq!(peer, 3, "a literal and two items chosen by the selector");
                    // §6.2, TR9: every entry's name is in `ItemDisplay_Name`; the literal
                    // entry first, in `Object_ColorName`.
                    let entry_names: Vec<String> = t
                        .entries
                        .1
                        .iter()
                        .filter_map(|r| text_of(&rig.host, *r, ids.item_name))
                        .collect();
                    eprintln!("trade at {ms} ms: {} peer entry names", entry_names.len());
                    assert_eq!(entry_names.len(), 3);
                    assert_eq!(entry_names[0], "Silver whistle");
                    assert!(entry_names.iter().all(|n| !n.is_empty()), "names drawn");
                    let states = s.vocab().states;
                    let first = find_below(&rig.host.ui(), t.entries.1[0], ids.item_name)
                        .and_then(|k| rig.host.ui().state(k));
                    assert_eq!(first, Some(states.object_name), "TR9: literal colour");
                    // §6.2 Tessera rule: the selector's items take their quality states.
                    let label = |st: Option<u32>| {
                        [
                            (states.quality_legendary, "legendary"),
                            (states.quality_rare, "rare"),
                            (states.quality_incomparable, "incomparable"),
                            (states.quality_uncommon, "uncommon"),
                            (states.quality_common, "common"),
                            (states.object_name, "object"),
                        ]
                        .into_iter()
                        .find(|(s, _)| Some(*s) == st)
                        .map_or("other", |(_, l)| l)
                    };
                    let name_states: Vec<&str> = t.entries.1[1..]
                        .iter()
                        .map(|r| {
                            label(
                                find_below(&rig.host.ui(), *r, ids.item_name)
                                    .and_then(|k| rig.host.ui().state(k)),
                            )
                        })
                        .collect();
                    eprintln!("trade at {ms} ms: item name states {name_states:?}");
                    assert!(!name_states.contains(&"other"), "a name state of §6.2");
                    // §6.1: the wallet (1,234,567) and the peer's coin (40,000) as coin
                    // elements: 12 / 345 / 67 and 4 / 0 / 0 hidden-leading 0 gold.
                    let counts = |d: Option<&tessera_ui::game::npc::MoneyDisplay>| {
                        d.and_then(|d| d.counts())
                            .map(|c| [c.gold, c.silver, c.copper])
                    };
                    assert_eq!(counts(t.wallet.as_ref()), Some([12, 345, 67]), "wallet");
                    assert_eq!(
                        counts(t.peer_money.as_ref()),
                        Some([0, 400, 0]),
                        "peer coin"
                    );
                    let ui = rig.host.ui();
                    let shown = |d: Option<&tessera_ui::game::npc::MoneyDisplay>| {
                        d.map_or(0, |d| {
                            d.coins
                                .iter()
                                .flatten()
                                .filter(|c| ui.is_visible(c.element))
                                .count()
                        })
                    };
                    assert_eq!(shown(t.wallet.as_ref()), 3, "ForceShowAll");
                    assert_eq!(
                        shown(t.peer_money.as_ref()),
                        2,
                        "SuppressLeadingZeroes: silver and copper"
                    );
                    let icons = t
                        .slots
                        .slots()
                        .iter()
                        .filter(|s| s.item.as_ref().is_some_and(|i| i.icon.is_some()))
                        .count();
                    assert_eq!(icons, 2, "the selector's items show the install's icons");
                }
                2800 => {
                    assert_eq!(own, 2);
                    let own_names: Vec<String> = t
                        .entries
                        .0
                        .iter()
                        .filter_map(|r| text_of(&rig.host, *r, ids.item_name))
                        .collect();
                    assert_eq!(own_names, ["Copper ring", "Travel loaf"], "§6.2");
                }
                3800 => assert!(model.button_on()),
                4300 => assert!(
                    model
                        .offer(tessera_ui::game::social::trade::Side::Peer)
                        .ready
                ),
                _ => {
                    assert!(!model.shown(), "the completed trade closes the trade (TR9)");
                    // The window fades out (its auto-fade), then hides.
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while rig.host.ui().is_visible(troot) {
                        assert!(
                            Instant::now() < deadline,
                            "the completed trade hides the window"
                        );
                        frame(rig, client, &data);
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            if let Some(p) = picture.as_ref() {
                let img = p.take(rig, client, &data);
                let b = *trade_box.get_or_insert_with(|| rig.host.ui().displayed_box(troot));
                let c = covered(&img, b, background(&img));
                eprintln!("trade at {ms} ms: {:.0} % covered", c * 100.0);
                if ms < 4700 {
                    assert!(c > 0.2, "the trade window is drawn");
                } else {
                    assert!(c < 0.01, "nothing is drawn after the trade");
                }
                save(&format!("trade-{ms}"), &img);
                shots.push((format!("trade-{ms:05}ms"), img));
                pictures += 1;
            }
        },
    );

    // ----- friends and ignore lists ------------------------------------------------------
    let mut client = Client::new(&data);
    rig.host
        .social_mut()
        .expect("social panels")
        .request_open(SocialOpen::Friends);
    let checks = [100, 1100, 3300, 4300];
    play(
        &dir,
        "friends",
        &checks,
        &mut rig,
        &mut client,
        &data,
        |ms, rig, client| {
            let s = rig.host.social().expect("social panels");
            let root = s.panel().expect("Social panel").root;
            let shown = rig.host.ui().is_visible(root);
            let friends = s.friends().expect("friends page");
            let names: Vec<String> = friends
                .rows
                .iter()
                .filter_map(|r| text_of(&rig.host, *r, ids.friend_row_name))
                .collect();
            let ignored = s.ignore().expect("ignore page").rows.len();
            eprintln!("friends at {ms} ms: shown {shown}, rows {names:?}, ignored {ignored}");
            assert!(shown, "the Social panel at its friends page");
            match ms {
                100 => {
                    assert_eq!(names, ["Brin", "cael", "Eda", "Fenn"], "by name (§3.4)");
                    assert_eq!(ignored, 2);
                }
                1100 => {
                    let notices = client
                        .lines
                        .iter()
                        .filter(|l| l.shown.as_ref().is_some_and(|(m, _)| *m == 0x8))
                        .count();
                    assert_eq!(notices, 2, "one came online, one went offline (CT9)");
                }
                3300 => assert_eq!(names.len(), 5),
                _ => {
                    assert_eq!(ignored, 3);
                    let tells: Vec<&ChatLine> = client
                        .lines
                        .iter()
                        .filter(|l| matches!(l.channel, tessera_net::ChatChannel::Tell { .. }))
                        .collect();
                    assert_eq!(
                        tells.len(),
                        1,
                        "the ignored speaker's tell is dropped (CT8)"
                    );
                }
            }
            if let Some(p) = picture.as_ref() {
                let img = p.take(rig, client, &data);
                let b = rig.host.ui().displayed_box(root);
                let c = covered(&img, b, background(&img));
                eprintln!("friends at {ms} ms: {:.0} % covered", c * 100.0);
                assert!(c > 0.2, "the Social panel is drawn");
                save(&format!("friends-{ms}"), &img);
                pictures += 1;
            }
        },
    );
    eprintln!("pictures: {pictures}");
    rig.shutdown();

    // Local-only goldens of the fellowship and trade pictures (decision 0031 tier 2).
    let (mut blessed, mut matched) = (0, 0);
    for (name, (width, height, data)) in shots {
        let pic = goldens::Rgba {
            width,
            height,
            data,
        };
        match goldens::check_with_compare(&golden_dir, &name, &pic, &flags, gpu_compare) {
            Ok(goldens::Outcome::Blessed(_)) => blessed += 1,
            Ok(goldens::Outcome::Passed(_)) => matched += 1,
            Err(e) => panic!("{e}"),
        }
    }
    eprintln!(
        "goldens in {}: {blessed} blessed, {matched} passed",
        golden_dir.display()
    );
}

/// A fellowship whose members carry effects (invented names; the effect records are chosen
/// by selector from the install, the literal one has no record).
const MEMBER_EFFECTS: &str = "\
# SPDX-License-Identifier: GPL-3.0-or-later
tessera-scenario 1
title \"Lantern wards\"
tick 100ms
player \"Tamsin\" level=20
select $fx = effect role=vital-over-time n=4 seed=9
actor $brin kind=player name=\"Brin\" pos=3,0,0 facing=270 level=31
actor $cael kind=player name=\"Cael\" pos=-3,1,0 facing=90 level=29

at 0s spawn $brin
+0s spawn $cael
+0.2s fellowship player $brin $cael leader=player
+0s vital $brin health 640/800
+0s vital $brin power 210/400
+0s vital $cael health 350/700
+0s vital $cael power 300/300
+0.2s effect $brin $fx[0] for=30s
+0s effect $brin $fx[1] for=40s
+0s effect $brin \"Ember\" for=50s from=$cael
+0s effect $cael $fx[2] for=60s
+0s effect $cael $fx[3] for=45s
+0s target player $brin
at 2s vital $brin health 600/800
";

/// `social-panels.md` TR16 on the install: the member rows' `FellowEffectDisplay` show the
/// members' effects through the combat widgets' lists (`combat-ui.md` §8.1–§8.2, at most
/// 6 icons per row, icon size 20); and the combat frames of the same screen keep TR14's
/// voice icons hidden on P4 and draw no portrait without a scene (`ui-widgets.md` §12).
/// With `TESSERA_SHEET_DIR` the picture is written there (never committed).
#[test]
fn member_effects_render_on_the_install() {
    // One GPU test at a time in this binary (`gpu.rs`), held for the whole test.
    let _gpu = gpu::GpuLock::acquire();
    let Some(dir) = game_dir() else { return };
    let golden_dir = goldens::golden_dir(
        env!("CARGO_PKG_NAME"),
        "social_panels_real_data/member_effects_render_on_the_install",
        &goldens::install_version(&dir),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut flags = goldens::Flags::from_env();
    flags.sheet = None;
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let install = Arc::new(
        tessera_client::combat::CombatInstall::new(Arc::clone(&data)).expect("combat data"),
    );
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let cv = booted.combat.vocab;
    let mut rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    if let Ok(r) = Renderer::new_headless(&RendererConfig::default()) {
        rig.renderer = Some(r);
    }
    rig.host.attach_combat(install.floaty_settings(), 0.0);
    let picture = Picture::new(&mut rig);
    let ids = rig.host.social().expect("social panels").vocab().ids;
    let mut client = Client::new(&data);
    client.combat_install = Some(Arc::clone(&install));
    let scenario = empty.path().join("member-effects.scenario");
    std::fs::write(&scenario, MEMBER_EFFECTS).expect("scenario file");
    let mut shot: Option<Shot> = None;
    play_path(
        &dir,
        &scenario,
        &[1500],
        &mut rig,
        &mut client,
        &data,
        |_, rig, client| {
            // Two more frames: the lists the social panels report are bound by the next
            // combat frame.
            frame(rig, client, &data);
            frame(rig, client, &data);
            let rows: Vec<(ElementKey, Option<u64>)> = rig
                .host
                .social()
                .expect("social panels")
                .member_effect_lists()
                .to_vec();
            assert_eq!(rows.len(), 2, "Brin and Cael have rows");
            let hud = rig.host.combat_hud().expect("combat widgets");
            let bars = &hud.widgets().member_effects;
            assert_eq!(bars.len(), 2, "one list per member with an entity");
            let ui = rig.host.ui();
            let mut shown = 0;
            let mut install_icons = 0;
            for (list, entity) in &rows {
                let bar = bars
                    .iter()
                    .find(|b| b.display() == Some(*list))
                    .expect("the row's list is bound");
                assert_eq!(
                    bar.owner(),
                    tessera_ui::game::combat::EffectOwner::Member(entity.expect("entity"))
                );
                assert!(bar.max_columns() <= 6, "TR16: at most 6 icons per row");
                for r in 0..2 {
                    for e in bar.rows().shown(r, bar.max_columns()) {
                        let icon = bar.icon(e.instance).expect("a shown icon");
                        assert!(ui.is_displayed(icon), "icon {} displayed", e.instance);
                        assert!(ui.is_ancestor_or_self(*list, icon));
                        assert_eq!(ui.layout_box(icon).map(|b| b.w()), Some(20));
                        if ui.element_id(icon) == Some(cv.ids.effect_icon) {
                            install_icons += 1;
                        }
                        shown += 1;
                    }
                }
            }
            eprintln!("member effects: {shown} icons shown, {install_icons} in the install look");
            assert_eq!(shown, 5, "three on Brin's row, two on Cael's");
            assert!(install_icons >= 1, "record effects use TimedEffectTemplate");
            // The same screen's combat frames: no voice icon on P4 (TR14), no portrait
            // without a scene on P5 (`ui-widgets.md` §12).
            let w = hud.widgets();
            let p4 = w.player.as_ref().expect("player frame").root();
            for id in [
                ids.player_voice,
                ids.player_voice_squelched,
                ids.player_sending,
                ids.player_assistant,
            ] {
                if let Some(k) = find_below(&ui, p4, id) {
                    assert!(!ui.is_displayed(k), "TR14: {id:#x} on P4");
                }
            }
            let target = w.target.as_ref().expect("target frame");
            assert!(
                ui.is_displayed(target.root()),
                "P5 shows the selected member"
            );
            let portrait = target.portrait().expect("OpponentPortrait");
            assert!(!ui.is_displayed(portrait), "no portrait without a scene");
            drop(ui);
            if let Some(p) = picture.as_ref() {
                shot = Some(p.take(rig, client, &data));
            }
        },
    );
    let Some(img) = shot else {
        eprintln!("no GPU adapter; the lists are checked, not drawn");
        return;
    };
    save("member-effects-1500", &img);
    let pic = goldens::Rgba {
        width: img.0,
        height: img.1,
        data: img.2,
    };
    match goldens::check_with_compare(
        &golden_dir,
        "member-effects-01500ms",
        &pic,
        &flags,
        gpu_compare,
    ) {
        Ok(o) => eprintln!("golden: {o:?}"),
        Err(e) => panic!("{e}"),
    }
}
