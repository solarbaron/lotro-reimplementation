// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M5 exit criterion 3 in the client: "a user skin folder changes the art of the
//! panels it maps", by the procedure of `skin-definition.md` §12.7 over the native panels
//! P1–P15 of the client's own UI host (layouts, workers, painter), on the user's install.
//!
//! The skin is synthetic (CC0, generated here: one-colour TGA files of the replaced
//! surfaces' own sizes) and lives in a temporary skins root (D24); nothing in the install or
//! the user's settings folder is touched, and no DID, label or count is hard-coded. The
//! pixel check reads the CPU canvases the client paints (decision 0015), so it needs no GPU.
//! Only counts are printed. Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! ui_skin_real_data -- --nocapture`.

mod ui_common;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tessera_client::panels::PanelId;
use tessera_client::plugins::Plugins;
use tessera_client::ui_host::boot::UiBoot;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{Record, Surface};
use tessera_ui::env::ImageRef;
use tessera_ui::{ElementKey, Ui};

/// Test screen (`skin-definition.md` §12.7 step 2).
const SCREEN: (u32, u32) = (1024, 768);
/// Draw modes Normal and Alphablend (`ui-layout.md` §3.3 keys).
const DRAW_NORMAL: u32 = 2;
const DRAW_ALPHABLEND: u32 = 4;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// An uncompressed 32-bit top-left TGA of one colour (`skin-definition.md` §7.2).
fn tga_solid(w: u16, h: u16, argb: u32) -> Vec<u8> {
    let mut v = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&[32, 0x28]);
    let px = argb.to_le_bytes();
    for _ in 0..(usize::from(w) * usize::from(h)) {
        v.extend_from_slice(&px);
    }
    v
}

/// A distinct opaque colour per index.
fn colour(i: usize) -> u32 {
    let hues = [
        (0xE0, 0x20, 0x20),
        (0x20, 0xE0, 0x20),
        (0x20, 0x20, 0xE0),
        (0xE0, 0xE0, 0x20),
        (0xE0, 0x20, 0xE0),
        (0x20, 0xE0, 0xE0),
        (0xF0, 0x80, 0x10),
        (0x80, 0x10, 0xF0),
    ];
    let (r, g, b) = hues[i % hues.len()];
    let k = (i / hues.len()) as u32 * 7;
    0xFF00_0000
        | ((r as u32 ^ k) & 0xFF) << 16
        | ((g as u32 ^ k) & 0xFF) << 8
        | ((b as u32 ^ k) & 0xFF)
}

fn close(a: u32, b: u32) -> bool {
    a.to_le_bytes()
        .iter()
        .zip(b.to_le_bytes())
        .all(|(x, y)| x.abs_diff(y) <= 1)
}

fn boot(
    set: &Arc<ArchiveSet>,
    dir: &Path,
    skins: &Path,
) -> tessera_client::ui_host::boot::UiBooted {
    UiBoot::spawn(
        Arc::clone(set),
        dir.to_path_buf(),
        Some(skins.to_path_buf()),
    )
    .expect("spawn")
    .wait()
    .expect("UI data")
}

fn host_for(b: tessera_client::ui_host::boot::UiBooted) -> UiHost {
    let options = UiOptions {
        screen: SCREEN,
        hud: false,
        ..UiOptions::default()
    };
    UiHost::from_boot(b, &options)
}

/// The most frames the UI may take to settle after a change.
const SETTLE_FRAMES: usize = 20_000;

/// Runs frames until the UI has settled ([`UiHost::settled`]: nothing waits for step 7, no
/// UI asset job is in flight, and the paint scheduler is idle with every surface drawn). At
/// least one frame runs, so a change made before the call is taken. The paint step works in
/// time slices, so the number of frames needed depends on the machine's load; a frame that
/// paints no band is not proof of a finished paint, so only the settled state ends the wait,
/// bounded by a generous frame count.
fn settle(host: &mut UiHost, plugins: &mut Plugins) {
    for _ in 0..SETTLE_FRAMES {
        host.step7(plugins, None);
        host.step8(None, plugins);
        host.end_frame(plugins, 0.0);
        if host.settled() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("UI did not settle in {SETTLE_FRAMES} frames");
}

fn subtree(ui: &Ui, root: ElementKey) -> Vec<ElementKey> {
    tessera_ui::game::subtree(ui, root)
}

/// The nearest element at or above `k` that owns a surface.
fn surface_owner(ui: &Ui, mut k: ElementKey) -> Option<ElementKey> {
    for _ in 0..64 {
        if ui.owns_surface(k) {
            return Some(k);
        }
        k = ui.parent(k)?;
    }
    None
}

/// A screen pixel inside `k` covered by none of its shown descendants (centre first, then
/// a coarse grid), and not drawn over by an overlay or text of `k` itself.
fn free_pixel(ui: &Ui, k: ElementKey) -> Option<(i32, i32)> {
    if ui.overlay(k).is_some_and(|(img, _, _)| img.is_some()) {
        return None;
    }
    if ui.class(k).is_some_and(|c| c.is_text()) {
        return None;
    }
    let b = ui.absolute_layout_box(k);
    let covers: Vec<_> = subtree(ui, k)
        .into_iter()
        .filter(|d| *d != k && ui.is_displayed(*d))
        .map(|d| ui.absolute_layout_box(d))
        .collect();
    let (w, h) = (b.w(), b.h());
    let mut candidates = vec![(b.l + w / 2, b.t + h / 2)];
    for gy in 1..4 {
        for gx in 1..4 {
            candidates.push((b.l + w * gx / 4, b.t + h * gy / 4));
        }
    }
    candidates.into_iter().find(|&(x, y)| {
        !covers
            .iter()
            .any(|c| x >= c.l && x <= c.r && y >= c.t && y <= c.b)
    })
}

#[derive(Debug, Clone)]
struct Kept {
    did: u32,
}

#[test]
fn synthetic_skin_changes_the_panel_art() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let tmp = tempfile::tempdir().expect("tempdir");
    let empty_root = tmp.path().join("empty-skins");
    std::fs::create_dir_all(&empty_root).expect("mkdir");
    let plugins_dir = tmp.path().join("Plugins");
    std::fs::create_dir_all(&plugins_dir).expect("mkdir");
    let mut plugins = Plugins::start(&plugins_dir, tmp.path().join("data"), &[]);

    // Steps 1–3: the panels without a skin; the skinnable elements and their DIDs.
    let b = boot(&set, &dir, &empty_root);
    let tables = b.skins.as_ref().expect("skin tables").tables.clone();
    let mut host = host_for(b);
    host.attach_plugins(&mut plugins, &[]);
    let mut by_panel: BTreeMap<PanelId, BTreeSet<u32>> = BTreeMap::new();
    let mut kept: Vec<Kept> = Vec::new();
    for id in PanelId::ALL {
        let Ok(root) = host.open_panel(&mut plugins, id) else {
            println!("{id}: did not open");
            continue;
        };
        if id == PanelId::TargetVitals {
            host.set_target(&mut plugins, true);
        }
        let ui = host.ui();
        for k in subtree(&ui, root) {
            let Some((ImageRef::Did(did), mode)) = ui.base_image(k) else {
                continue;
            };
            if tables.base.art_asset(did).is_none()
                || !(mode == DRAW_NORMAL || mode == DRAW_ALPHABLEND)
                || !ui.is_displayed(k)
            {
                continue;
            }
            let bx = ui.absolute_layout_box(k);
            if bx.w() <= 0 || bx.h() <= 0 {
                continue;
            }
            let p = ui.vocab().pids;
            let mut opaque = true;
            let mut a = Some(k);
            while let Some(e) = a {
                if (ui.prop(e, p.object_opacity).float() - 1.0).abs() > 1e-6 {
                    opaque = false;
                }
                a = ui.parent(e);
            }
            if (ui.prop(k, p.alphablendmod).float() - 1.0).abs() > 1e-6 || !opaque {
                continue;
            }
            kept.push(Kept { did });
            by_panel.entry(id).or_default().insert(did);
        }
    }
    let mut chosen: BTreeMap<PanelId, Vec<u32>> = BTreeMap::new();
    for (id, dids) in &by_panel {
        chosen.insert(*id, dids.iter().copied().take(8).collect());
    }
    let all_chosen: BTreeSet<u32> = chosen.values().flatten().copied().collect();
    let missing_did = kept
        .iter()
        .map(|k| k.did)
        .filter(|d| !all_chosen.contains(d))
        .min();
    for id in PanelId::ALL {
        if !chosen.contains_key(&id) {
            println!("{id}: no skinnable art in the default view");
        }
    }

    // Steps 4–6: the synthetic skin.
    let root = tmp.path().join("skins");
    let folder = root.join("TestSkin");
    std::fs::create_dir_all(folder.join("art")).expect("mkdir");
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opt>\n<SkinName Name=\"Tessera Test Skin\"/>\n",
    );
    let mut colours: BTreeMap<u32, u32> = BTreeMap::new();
    for (i, did) in all_chosen.iter().enumerate() {
        let art = tables.base.art_asset(*did).expect("in the base table");
        let label = tables.art_assets.label(art).expect("label");
        let payload = set.read(*did).expect("surface");
        let s = Surface::parse(&payload).expect("surface header");
        let (w, h) = (s.width.min(4096) as u16, s.height.min(4096) as u16);
        let c = colour(i);
        colours.insert(*did, c);
        std::fs::write(
            folder.join("art").join(format!("{did:08x}.tga")),
            tga_solid(w, h, c),
        )
        .expect("write tga");
        let _ = writeln!(
            xml,
            "<Mapping ArtAssetID=\"{label}\" FileName=\"art\\{did:08x}.tga\"/>"
        );
    }
    if let Some(d) = missing_did {
        let art = tables.base.art_asset(d).expect("in the base table");
        let label = tables.art_assets.label(art).expect("label");
        let _ = writeln!(
            xml,
            "<Mapping ArtAssetID=\"{label}\" FileName=\"art\\missing.tga\"/>"
        );
    }
    xml.push_str("<Mapping ArtAssetID=\"Tessera_No_Such_Art_Asset\" FileName=\"art\\x.tga\"/>\n");
    let override_panel = PanelId::from_number(6).expect("P6");
    let override_label = override_panel.root_label();
    let _ = writeln!(
        xml,
        "<PanelFile ID=\"ID_Test\"><Element ID=\"{override_label}\" X=\"40\" Y=\"30\" Width=\"240\" Height=\"290\"/></PanelFile>"
    );
    xml.push_str("</opt>\n");
    std::fs::write(folder.join("SkinDefinition.xml"), xml).expect("write definition");

    // Step 7: discovery lists exactly the one skin.
    let b2 = boot(&set, &dir, &root);
    let skins = b2.skins.as_ref().expect("skins").skins.clone();
    assert_eq!(skins.skins().len(), 1, "one skin discovered");
    assert_eq!(skins.skins()[0].name(), "Tessera Test Skin");
    let mut host = host_for(b2);
    host.attach_plugins(&mut plugins, &[]);
    let mut roots = BTreeMap::new();
    for id in chosen.keys() {
        if let Ok(k) = host.open_panel(&mut plugins, *id) {
            roots.insert(*id, k);
        }
        if *id == PanelId::TargetVitals {
            host.set_target(&mut plugins, true);
        }
    }
    let p6_before = host
        .panels()
        .and_then(|p| p.root(override_panel))
        .map(|k| host.ui().layout_box(k));
    settle(&mut host, &mut plugins);

    // Step 8: activation without rebuilding.
    assert!(host.set_skin("Tessera Test Skin"), "skin activated");
    settle(&mut host, &mut plugins);
    // The D16 placeholder's colours (the missing file's art must show one of them).
    let mut placeholder_colours: Vec<u32> =
        tessera_material::ui_image::placeholder().pixels().to_vec();
    placeholder_colours.sort_unstable();
    placeholder_colours.dedup();
    let (mut missing_checked, mut missing_passed) = (0usize, 0usize);
    let mut report = Vec::new();
    let mut changed_per_panel: BTreeMap<PanelId, (usize, usize, usize)> = BTreeMap::new();
    for (id, root) in &roots {
        let ui = host.ui();
        let mut checked = 0;
        let mut passed = 0;
        let mut skipped = 0;
        for k in subtree(&ui, *root) {
            let Some((image, _)) = ui.base_image(k) else {
                continue;
            };
            match image {
                ImageRef::Skin { did, .. } => {
                    assert!(
                        all_chosen.contains(&did) || Some(did) == missing_did,
                        "{id}: an unmapped DID changed"
                    );
                    if Some(did) == missing_did {
                        if ui.is_displayed(k)
                            && let (Some(owner), Some((x, y))) =
                                (surface_owner(&ui, k), free_pixel(&ui, k))
                        {
                            let ob = ui.absolute_layout_box(owner);
                            missing_checked += 1;
                            if ui
                                .surface_pixel(owner, x - ob.l, y - ob.t)
                                .is_some_and(|px| placeholder_colours.iter().any(|c| close(px, *c)))
                            {
                                missing_passed += 1;
                            }
                        }
                        continue;
                    }
                    let Some(&c) = colours.get(&did) else {
                        continue;
                    };
                    if !ui.is_displayed(k) {
                        continue;
                    }
                    let (Some(owner), Some((x, y))) = (surface_owner(&ui, k), free_pixel(&ui, k))
                    else {
                        skipped += 1;
                        continue;
                    };
                    let ob = ui.absolute_layout_box(owner);
                    checked += 1;
                    match ui.surface_pixel(owner, x - ob.l, y - ob.t) {
                        Some(px) if close(px, c) => passed += 1,
                        _ => {}
                    }
                }
                ImageRef::Did(did) => {
                    assert!(
                        !all_chosen.contains(&did) || !tables.base.art_asset(did).is_some(),
                        "{id}: a mapped DID {did:#010x} still shows its base art"
                    );
                }
                ImageRef::File(_) => {}
            }
        }
        changed_per_panel.insert(*id, (chosen.get(id).map_or(0, Vec::len), checked, skipped));
        report.push(format!(
            "{id}: chosen {}, checked {checked}, passed {passed}, skipped {skipped}",
            chosen.get(id).map_or(0, Vec::len)
        ));
        if checked > 0 {
            assert!(passed > 0, "{id}: art changed to the mapped file");
        }
    }
    for r in &report {
        println!("{r}");
    }
    let changed = changed_per_panel
        .values()
        .filter(|(_, c, _)| *c > 0)
        .count();
    println!(
        "panels with a checked art change: {changed} of {}",
        roots.len()
    );
    assert!(
        changed >= 7,
        "at least the seven §8.6 panels change their art"
    );
    println!("missing-file art: checked {missing_checked}, placeholder {missing_passed}");
    if missing_checked > 0 {
        assert!(
            missing_passed > 0,
            "the missing skin file shows the D16 placeholder"
        );
    }

    // Step 10: the override applies when the panel is built again.
    if let Some(before) = p6_before.flatten() {
        assert_ne!(
            (before.l, before.t),
            (40, 30),
            "a panel built before keeps its rectangle"
        );
    }

    // Step 11: deactivation restores the base art.
    assert!(!host.set_skin(""));
    settle(&mut host, &mut plugins);
    for root in roots.values() {
        let ui = host.ui();
        for k in subtree(&ui, *root) {
            if let Some((img, _)) = ui.base_image(k) {
                assert!(
                    !matches!(img, ImageRef::Skin { .. }),
                    "skin art left after deactivation"
                );
            }
        }
    }
    host.shutdown(&mut plugins, None);
    drop(host);

    // Step 10 with the skin active from the start: P6 is built with the override.
    let b3 = boot(&set, &dir, &root);
    let options = UiOptions {
        screen: SCREEN,
        hud: false,
        prefs: tessera_settings::UiPrefs {
            user_skin: "Tessera Test Skin".into(),
            ..tessera_settings::UiPrefs::default()
        },
        ..UiOptions::default()
    };
    let mut host = UiHost::from_boot(b3, &options);
    host.attach_plugins(&mut plugins, &[]);
    let k = host.open_panel(&mut plugins, override_panel).expect("P6");
    let b = host.ui().layout_box(k).expect("box");
    println!(
        "P6 with the override: ({}, {}, {}, {})",
        b.l,
        b.t,
        b.w(),
        b.h()
    );
    assert_eq!((b.l, b.t, b.w(), b.h()), (40, 30, 240, 290));
    host.shutdown(&mut plugins, None);
}
