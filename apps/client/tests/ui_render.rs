// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI pass in the client (decision 0015, `render.md` §6): a synthetic plugin window is
//! painted by the client's UI host, its GPU operations are applied to UI textures, and the
//! composition list is drawn after the scene, offscreen. Skipped (reported as passed) when
//! no GPU adapter is available.

mod ui_common;

use tessera_render::{OffscreenFormat, PassSpace, Renderer, RendererConfig};
use ui_common::{Rig, write_plugin};

const BOXES: &str = r#"
local w = Turbine.UI.Window()
w:SetPosition(40, 30)
w:SetSize(64, 48)
w:SetBackColor(Turbine.UI.Color(1, 1, 0, 0))
w:SetVisible(true)
local v = Turbine.UI.Window()
v:SetPosition(140, 30)
v:SetSize(32, 32)
v:SetBackColor(Turbine.UI.Color(1, 0, 0, 1))
v:SetOpacity(0.5)
v:SetVisible(true)
"#;

#[test]
fn plugin_windows_reach_the_frame() {
    let renderer = match Renderer::new_headless(&RendererConfig::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("no GPU adapter ({e}); skipping");
            return;
        }
    };
    let mut rig = Rig::new(|p| write_plugin(p, "Suite", "Boxes", Some("Boxes"), BOXES));
    rig.renderer = Some(renderer);
    rig.attach(&["Boxes"]);
    rig.wait_loaded(1);
    let target = rig
        .renderer
        .as_mut()
        .expect("renderer")
        .create_offscreen_target(256, 128, OffscreenFormat::Rgba8Srgb)
        .expect("target");
    let mut image = None;
    for _ in 0..20 {
        rig.frame();
        let renderer = rig.renderer.as_mut().expect("renderer");
        let mut frame = renderer.begin_offscreen_frame(&target).expect("frame");
        frame
            .pass("scene", PassSpace::Screen)
            .clear([0.0, 0.0, 0.0, 1.0]);
        rig.host.draw(&mut frame, false);
        let stats = renderer.end_frame(frame).expect("end frame");
        if stats.ui.quads >= 2 {
            image = Some(renderer.read_pixels(&target).expect("read back"));
            break;
        }
    }
    let image = image.expect("two UI quads drawn");
    let px = |x: u32, y: u32| {
        let i = ((y * image.width + x) * 4) as usize;
        [
            image.data[i],
            image.data[i + 1],
            image.data[i + 2],
            image.data[i + 3],
        ]
    };
    // Inside the opaque red window.
    let r = px(60, 50);
    assert!(r[0] > 200 && r[1] < 40 && r[2] < 40, "{r:?}");
    // Inside the half-transparent blue window: blue over black at about half.
    let b = px(150, 40);
    assert!(b[2] > 60 && b[2] < 230 && b[0] < 40, "{b:?}");
    // Outside every window: the cleared scene.
    let o = px(220, 110);
    assert!(o[0] < 10 && o[1] < 10 && o[2] < 10, "{o:?}");
    assert!(!rig.host.textures().is_empty());
    rig.shutdown();
    assert!(rig.host.textures().is_empty());
}
