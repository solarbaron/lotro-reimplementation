// SPDX-License-Identifier: GPL-3.0-or-later
//! The particle preview against a user's own install, read-only: for an even sample of the
//! particle-system descriptors (0x39; all of them with `TESSERA_EXPLORER_FULL=1`), the
//! preview's source loads (record, surface material, colour texture, blend state, the
//! surface program of `particle-runtime.md` §9.1.1), the
//! seeded simulation runs for one second, and the first few are rendered when a GPU
//! exists. Fork records (0x44) report that they are not simulated. Counts are printed.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. The output stays on the local
//! terminal.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tessera_explorer::data::DataContext;
use tessera_explorer::particle_view::{PreviewGpu, PreviewSim, load_source, render_draws};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn dids(ctx: &DataContext, prefix: u8) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let mut out = std::collections::BTreeSet::new();
    for m in ctx.set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            out.insert(e.expect("directory walk").did);
        }
    }
    out.into_iter().collect()
}

#[test]
fn particle_previews_load_and_run() {
    let Some(dir) = game_dir() else {
        return;
    };
    let ctx = DataContext::open(&dir, None).expect("open install");
    let all = dids(&ctx, 0x39);
    assert!(!all.is_empty());
    let full = std::env::var_os("TESSERA_EXPLORER_FULL").is_some_and(|v| v == "1");
    let want = if full { all.len() } else { 300.min(all.len()) };
    let step = (all.len() / want.max(1)).max(1);
    let sample: Vec<u32> = all.iter().copied().step_by(step).take(want).collect();
    let mut gpu = PreviewGpu::new().ok();
    let mut textured = 0;
    let mut notes = 0;
    let mut modes: BTreeMap<String, usize> = BTreeMap::new();
    let mut live = 0usize;
    let mut rendered = 0;
    let mut surfaced = 0;
    let mut unlisted = 0;
    let mut layered = 0;
    let mut hidden = 0;
    for &did in &sample {
        let src = load_source(&ctx, did).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        textured += usize::from(src.texture.is_some());
        notes += usize::from(!src.notes.is_empty());
        let mode = format!("{:?}", src.look.alpha);
        let has_surface = !src.desc.surface.is_null() || !src.desc.extra_surface.is_null();
        surfaced += usize::from(has_surface);
        unlisted += usize::from(has_surface && src.look.program.is_none());
        layered += usize::from(src.look.shading.layers.is_some());
        hidden += usize::from(!src.look.drawn);
        let key = mode.split(['(', ' ', '{']).next().unwrap_or("").to_owned();
        *modes.entry(key).or_default() += 1;
        let mut sim = PreviewSim::new(src.desc.clone(), did);
        sim.run_to(1.0);
        live += sim.sim().live_particles();
        if rendered < 5
            && let Some(g) = gpu.as_mut()
        {
            g.set_texture(0, src.texture.as_deref())
                .expect("texture upload");
            g.set_texture(1, src.second_texture.as_deref())
                .expect("texture upload");
            let cam = tessera_explorer::viewport::OrbitView::new().camera();
            let draws = sim
                .draw(&cam)
                .map(|d| render_draws(&d, g.textures(), src.look))
                .unwrap_or_default();
            let img = g.render(draws, &cam, 96, 96).expect("render");
            assert_eq!(img.size, [96, 96]);
            rendered += 1;
        }
    }
    let forks = dids(&ctx, 0x44);
    if let Some(&f) = forks.first() {
        assert!(
            load_source(&ctx, f).is_err(),
            "fork effects are not simulated"
        );
    }
    eprintln!(
        "particle previews: {} of {} records loaded; {textured} with a diffuse texture, \
         {notes} with notes; blend modes {modes:?}; {live} live particles after 1 s in all; \
         {rendered} rendered; surface programs: {surfaced} with a surface, {unlisted} not in \
         runtime §9.1.1, {layered} with two layers, {hidden} distortion (not drawn)",
        sample.len(),
        all.len()
    );
    assert!(textured > sample.len() / 2);
    // §9.1.1: every particle surface uses one of the 19 listed shader records.
    assert_eq!(
        unlisted, 0,
        "surfaces outside the 19 records of runtime §9.1.1"
    );
}
