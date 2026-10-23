// SPDX-License-Identifier: GPL-3.0-or-later
//! The portraits of the player and target frames on the user's install (decision 0024 P2;
//! `combat-ui.md` §5.1, CU32; `ui-widgets.md` §12.1–§12.4): the client's headless loop with
//! the game UI and its HUD over the plain sandbox, and [`CombatPortraits`] with the walker's
//! figures. Checked from the UI tree, the composition list and the targets' pixels:
//!
//! - before a figure is drawn both portraits are hidden (§12.4);
//! - `PlayerPortrait` reads scene 0x21000004, a 62 × 62 target, opaque black and its
//!   texture-list mask, and draws the local player through the scene's `Camera_1`: inside
//!   the mask the target holds the figure on the black background, outside it is
//!   transparent; the element is shown and composed as external content;
//! - the targets are not drawn again once their drawing is complete (frame cost);
//! - selecting the player itself (the session's `TargetChanged`, with a health report so
//!   that P5 shows its vital field, where the portrait sits) fills P5, whose
//!   `OpponentPortrait` draws the same figure into its own target with its own mask;
//! - selecting a placed figure (an NPC), the usual P5 case, keeps P5 hidden: the walker
//!   offers no portrait draw of other entities yet (a known gap, `crate::portrait` module
//!   docs); this part is skipped when no placed figure is built near the start.
//!
//! The portraits are driven before the UI's step 8 (the order the module docs require).
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set, or without a GPU adapter.
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! combat_portraits_real_data -- --nocapture`. With `TESSERA_SHEET_DIR` the targets and the
//! frame are written there as PNG files (never inside a work tree, never committed). Prints
//! counts only.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::portrait::{CombatPortraits, content_id, render_combat_portraits};
use tessera_client::runtime::Runtime;
use tessera_client::ui_host::client::ClientUi;
use tessera_client::ui_host::cursor::NoWindow;
use tessera_client::walker::characters::FigureStatus;
use tessera_client::walker_host::{WalkerHost, save_png};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_render::{OffscreenFormat, Renderer, RendererConfig, RgbaImage};
use tessera_ui::game::combat::FrameKind;
use tessera_ui::paint::EntryTexture;

const SIZE: (u32, u32) = (1280, 720);
const FRAME: Duration = Duration::from_millis(16);
const MAX_FRAMES: u32 = 4800;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

struct Client<'r> {
    rt: Runtime,
    host: WalkerHost,
    ui: ClientUi,
    renderer: Renderer<'r>,
    portraits: CombatPortraits<FigureStatus>,
    last: Instant,
    viewport_set: bool,
    frame_target: tessera_render::OffscreenTarget,
}

impl Client<'static> {
    fn start(game: PathBuf, data: &std::path::Path) -> Option<Self> {
        let renderer = match Renderer::new_headless(&RendererConfig {
            meshes: tessera_client::walker::mesh_config(tessera_client::walker::arena_bytes(
                &tessera_client::walker_host::walker_config(&Args::parse_from(["tessera-client"])),
            )),
            ..RendererConfig::default()
        }) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("SKIPPED: no GPU adapter ({e})");
                return None;
            }
        };
        let mut args = Args::parse_from([
            "tessera-client",
            "--ui",
            "--size",
            "1280x720",
            // The walker's own HUD off; the game HUD (P4, P5) on.
            "--no-hud",
        ]);
        args.game_dir = Some(game.clone());
        args.data_dir = Some(data.join("data"));
        let rargs = ClientUi::runtime_args(&args);
        let mut rt = Runtime::start(&rargs, Instant::now()).expect("runtime");
        let shared = rt.walker_shared.clone().expect("walker session");
        let host = WalkerHost::new(&args, shared);
        let mut ui = ClientUi::new(&rargs);
        ui.prepare(&mut rt);
        // The masks are read from the install on the portraits' worker (read-only).
        let set = ArchiveSet::open(&game, SetOptions::default()).expect("archive set");
        let source: Arc<dyn tessera_props::RecordSource> = Arc::new(set);
        let frame_target = renderer
            .create_offscreen_target(SIZE.0, SIZE.1, OffscreenFormat::Rgba8Srgb)
            .expect("frame target");
        Some(Self {
            rt,
            host,
            ui,
            renderer,
            portraits: CombatPortraits::for_install(source),
            last: Instant::now(),
            viewport_set: false,
            frame_target,
        })
    }
}

impl Client<'_> {
    /// One frame of the client's loop with the portraits, and the frame drawn offscreen.
    fn frame(&mut self) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last).as_secs_f32();
        self.last = now;
        let _ = self.rt.poll_loader();
        self.rt.tick(now);
        self.host.poll(&mut self.rt, &mut self.renderer);
        assert!(self.host.failed.is_none(), "walker: {:?}", self.host.failed);
        self.ui.poll(&mut self.rt, self.host.walker.as_mut());
        assert!(self.ui.failed().is_none(), "UI: {:?}", self.ui.failed());
        let Some(walker) = self.host.walker.as_mut() else {
            std::thread::sleep(Duration::from_millis(20));
            return;
        };
        if !self.viewport_set {
            walker.set_viewport(SIZE.0, SIZE.1);
            self.viewport_set = true;
        }
        self.ui.before_update(walker, &mut self.rt, &mut NoWindow);
        let ev = walker.update(
            &mut self.renderer,
            self.rt
                .world
                .as_mut()
                .map(|w| w as &mut dyn tessera_client::walker::WalkerSession),
            &mut self.rt.plugins,
            now,
            dt,
        );
        self.ui.after_update(walker, &mut self.rt, &ev);
        if let Some(h) = self.ui.host() {
            render_combat_portraits(&mut self.portraits, h, walker, &mut self.renderer);
        }
        self.ui.step8(walker, &mut self.renderer, &mut self.rt);
        self.ui.render_portraits(walker, &mut self.renderer);
        let mut frame = self
            .renderer
            .begin_offscreen_frame(&self.frame_target)
            .expect("frame");
        walker.draw(&self.renderer, &mut frame);
        self.ui.draw(&mut frame, false);
        self.renderer.end_frame(frame).expect("end frame");
        self.ui.end_frame(&self.rt);
        let spent = now.elapsed();
        if spent < FRAME {
            std::thread::sleep(FRAME - spent);
        }
    }

    fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
        for _ in 0..MAX_FRAMES {
            self.frame();
            if done(self) {
                return;
            }
        }
        panic!(
            "{what}: not reached in {MAX_FRAMES} frames; stats {:?}",
            self.portraits.stats()
        );
    }

    /// The portrait element of `kind`, whether it is displayed, and whether the
    /// composition list holds its external content.
    fn portrait_state(&self, kind: FrameKind) -> Option<(bool, bool)> {
        let h = self.ui.host()?;
        let hud = h.combat_hud()?;
        let k = hud.widgets().frame(kind)?.portrait()?;
        let ui = h.ui();
        let composed = h
            .composition()
            .entries
            .iter()
            .any(|e| e.texture == EntryTexture::External(content_id(kind)));
        Some((ui.is_displayed(k), composed))
    }

    fn player(&self) -> Option<u64> {
        let w = self.host.walker.as_ref()?;
        w.characters().player_id().map(|p| p.0)
    }

    fn target_pixels(&self, kind: FrameKind) -> RgbaImage {
        let t = self.portraits.target(kind).expect("a target");
        self.renderer.read_viewport_pixels(t).expect("read back")
    }
}

/// Checks a drawn 62 × 62 portrait: transparent outside the mask, opaque inside, with the
/// figure (coloured pixels) on the black background.
fn check_portrait(name: &str, img: &RgbaImage) {
    assert_eq!(
        (img.width, img.height),
        (62, 62),
        "{name}: the 62 × 62 target"
    );
    let px = |x: u32, y: u32| img.pixel(x, y).expect("pixel");
    let corners = [(0, 0), (61, 0), (0, 61), (61, 61)];
    let clear_corners = corners.iter().filter(|(x, y)| px(*x, *y)[3] == 0).count();
    let centre = px(31, 31);
    let mut opaque = 0;
    let mut black = 0;
    let mut figure = 0;
    for p in img.data.as_chunks::<4>().0 {
        if p[3] == 255 {
            opaque += 1;
            if p[0] == 0 && p[1] == 0 && p[2] == 0 {
                black += 1;
            } else if p[0].max(p[1]).max(p[2]) > 24 {
                figure += 1;
            }
        }
    }
    eprintln!(
        "{name}: {clear_corners}/4 transparent corners, centre alpha {}, {opaque} opaque \
         pixels ({black} background, {figure} figure)",
        centre[3]
    );
    assert_eq!(centre[3], 255, "{name}: the mask is opaque at the centre");
    assert!(clear_corners >= 3, "{name}: the mask cuts the corners");
    assert!(figure > 100, "{name}: the figure is drawn");
    assert!(black > 50, "{name}: the opaque black background shows");
}

fn sheet(name: &str, img: &RgbaImage) {
    if let Some(dir) = std::env::var_os("TESSERA_SHEET_DIR") {
        let path = PathBuf::from(dir).join(format!("combat_portraits_{name}.png"));
        save_png(&path, img).expect("sheet");
        eprintln!("wrote {}", path.display());
    }
}

#[test]
fn portraits_draw_for_the_player_and_the_player_selected() {
    let Some(game) = game_dir() else { return };
    let data = tempfile::tempdir().expect("tempdir");
    let Some(mut c) = Client::start(game, data.path()) else {
        return;
    };
    // The game HUD with P4 and P5 and the combat widgets in their install look.
    c.until("the combat widgets", |c| {
        c.ui.host().and_then(|h| h.combat_hud()).is_some_and(|h| {
            h.widgets()
                .player
                .as_ref()
                .is_some_and(|p| !p.is_fallback())
        })
    });
    // §12.4: no figure drawn yet, no portrait.
    if !c.portraits.drawn(FrameKind::Player) {
        let (shown, composed) = c.portrait_state(FrameKind::Player).expect("PlayerPortrait");
        assert!(!shown && !composed, "hidden while no figure is drawn");
    }
    let spec = {
        let h = c.ui.host().expect("host");
        let hud = h.combat_hud().expect("hud");
        hud.widgets().portraits(&h.ui())
    };
    assert_eq!(spec.len(), 2, "P4 and P5 have portraits");
    for v in &spec {
        eprintln!(
            "{:?}: scene {:?}, target {:?}, background {:#010x}, mask {:?}, player {}",
            v.spec.kind,
            v.spec.scene.map(|d| format!("{:#010x}", d.0)),
            v.spec.target_size,
            v.spec.background,
            v.spec.mask.map(|d| format!("{:#010x}", d.0)),
            v.spec.use_player
        );
        assert_eq!(v.spec.scene.map(|d| d.0), Some(0x2100_0004), "CU32 scene");
        assert_eq!(v.spec.target_size, (62, 62), "CU32 target size");
        assert_eq!(v.spec.background, 0xFF00_0000, "opaque black");
        assert!(
            v.spec.mask.is_some_and(|d| d.0 >> 24 == 0x40),
            "a texture-list mask"
        );
    }
    assert!(spec[0].spec.use_player && !spec[1].spec.use_player);

    // P4: the local player's figure.
    c.until("the player portrait drawn", |c| {
        c.portraits.drawn(FrameKind::Player)
    });
    c.frame();
    let (shown, composed) = c.portrait_state(FrameKind::Player).expect("PlayerPortrait");
    assert!(shown, "PlayerPortrait shown with its scene");
    assert!(composed, "composed as external content");
    let p4 = c.target_pixels(FrameKind::Player);
    sheet("p4", &p4);
    check_portrait("PlayerPortrait", &p4);

    // Frame cost: once the drawing is complete (the figure stable and the walker settled)
    // the target is not drawn again.
    c.until("the player portrait complete", |c| {
        c.portraits.complete(FrameKind::Player)
    });
    let p4 = c.target_pixels(FrameKind::Player);
    sheet("p4_complete", &p4);
    check_portrait("PlayerPortrait (complete)", &p4);
    let renders = c.portraits.stats().renders;
    for _ in 0..90 {
        c.frame();
    }
    assert_eq!(
        c.portraits.stats().renders,
        renders,
        "a complete portrait is not drawn again"
    );
    eprintln!("complete: {:?}", c.portraits.stats());

    // P5: select the player itself; the target frame's portrait shows the same figure.
    // The plain sandbox answers no combat command, so the session's answer to
    // `SelectTarget` (`TargetChanged` for the player, `combat-ui.md` §3) is applied as the
    // session would deliver it.
    let player = c.player().expect("the player's entity");
    let me = tessera_net::EntityId(player);
    c.rt.world.as_mut().expect("world session").apply_polled(
        Instant::now(),
        &[
            // A health report gives the target frame its vital field, where the portrait
            // sits (`combat-ui.md` §5.1, TR12).
            tessera_net::Event::Combat(tessera_net::domain::CombatEvent::VitalChanged {
                entity: me,
                vital: tessera_net::domain::Vital::Health,
                current: 900.0,
                max: 1000.0,
            }),
            tessera_net::Event::Combat(tessera_net::domain::CombatEvent::TargetChanged {
                entity: me,
                target: Some(me),
            }),
        ],
    );
    c.until("the player selected", |c| {
        c.ui.host()
            .and_then(|h| h.combat_hud())
            .is_some_and(|h| h.widgets().selection() == Some(player))
    });
    let views = {
        let h = c.ui.host().expect("host");
        h.combat_hud().expect("hud").widgets().portraits(&h.ui())
    };
    eprintln!(
        "after the selection: {:?}",
        views
            .iter()
            .map(|v| (v.spec.kind, v.entity, v.displayed))
            .collect::<Vec<_>>()
    );
    c.until("the target portrait drawn", |c| {
        c.portraits.drawn(FrameKind::Target)
    });
    c.frame();
    let (shown, composed) = c
        .portrait_state(FrameKind::Target)
        .expect("OpponentPortrait");
    assert!(shown && composed, "OpponentPortrait shown and composed");
    let p5 = c.target_pixels(FrameKind::Target);
    sheet("p5", &p5);
    check_portrait("OpponentPortrait", &p5);
    let frame = c
        .renderer
        .read_pixels(&c.frame_target)
        .expect("frame read back");
    sheet("frame", &frame);

    // P5 with a placed figure (an NPC) selected: hidden, the known gap (module docs).
    let npc = c.host.walker.as_ref().and_then(|w| {
        let chars = w.characters();
        chars.placed().map(|(id, _)| id).find(|id| {
            Some(*id) != chars.player_id()
                && chars.figure_of(*id).is_some_and(|f| {
                    matches!(
                        f.state,
                        tessera_client::walker::characters::FigureState::Built { parts, .. }
                            if parts > 0
                    )
                })
        })
    });
    match npc {
        Some(npc) => {
            let them = tessera_net::EntityId(npc.0);
            c.rt.world.as_mut().expect("world session").apply_polled(
                Instant::now(),
                &[
                    tessera_net::Event::Combat(tessera_net::domain::CombatEvent::VitalChanged {
                        entity: them,
                        vital: tessera_net::domain::Vital::Health,
                        current: 50.0,
                        max: 100.0,
                    }),
                    tessera_net::Event::Combat(tessera_net::domain::CombatEvent::TargetChanged {
                        entity: me,
                        target: Some(them),
                    }),
                ],
            );
            c.until("the placed figure selected", |c| {
                c.ui.host()
                    .and_then(|h| h.combat_hud())
                    .is_some_and(|h| h.widgets().selection() == Some(npc.0))
            });
            for _ in 0..30 {
                c.frame();
            }
            let (shown, composed) = c
                .portrait_state(FrameKind::Target)
                .expect("OpponentPortrait");
            eprintln!("a placed figure selected: P5 shown {shown}, composed {composed} (gap)");
            assert!(
                !shown && !composed,
                "until the walker draws other entities' portraits, P5 stays hidden (§12.4)"
            );
            assert!(!c.portraits.drawn(FrameKind::Target));
        }
        None => eprintln!("no placed figure built near the start; the NPC selection is skipped"),
    }
    eprintln!("stats: {:?}", c.portraits.stats());
}
