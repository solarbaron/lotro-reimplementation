<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Golden images

PNG references for the headless golden-image tests. Every image here is rendered by
Tessera's own code from a procedural scene; none contains or is derived from game data.

| Image | Test | Scene |
|---|---|---|
| `render_test_scene.png` | `crates/render/tests/golden.rs` | renderer primitives: grid, axes, box outline, nearest/linear/sRGB/linear textures, depth, blending, overlay, atlas UVs, render-to-texture |
| `mesh_opaque.png` | `crates/render/tests/mesh_golden.rs` | opaque pipeline: textured cube, specular sphere with vertex colours, one cube drawn as two submeshes with two materials, lit floor |
| `mesh_alpha_test.png` | same | alpha test: crossed two-sided cut-out cards, one alpha ramp cut at thresholds 0.25 and 0.75 |
| `mesh_alpha_blend.png` | same | alpha blend: three panes and a two-sided sphere, sorted back to front (a second test checks the result is independent of submission order) |
| `mesh_two_sided.png` | same | open tubes and cards seen from behind, one-sided vs two-sided |
| `mesh_dye.png` | same | dye: striped mask texture with three dye colours, and a base-alpha mask |
| `mesh_normal_map.png` | same | tangent-space bump map on a plane without tangents (ignored), with tangents, and on a specular sphere, under a grazing light |
| `mesh_debug.png` | same | debug views: hidden surfaces with wireframe, world normals, UV checker, lit with wireframe and normal-line overlays; camera framed with `OrbitCamera::frame_sphere` |
| `skinned_bend.png` | `crates/render/tests/skinned_golden.rs` | GPU skinning: one two-bone cylinder drawn with three palettes in one frame (straight, bent forward, bent back), textured and specular |
| `skinned_weights.png` | same | rigid weights (hard crease), smooth two-bone weights, and a three-bone chain whose vertices use all four influence slots |
| `skinned_alpha_test.png` | same | an alpha-tested, two-sided skinned card folded forwards and backwards |
| `skinned_dye.png` | same | dye on a skinned limb: texture mask with three colours and two palettes, base-alpha mask with specular |
| `skinned_debug.png` | same | debug views following the skinned surface: wireframe only, world normals, UV checker with normal lines |
| `ui_tile.png` | `crates/render/tests/ui_golden.rs` | UI pass: a 32 × 32 canvas tiled 1 : 1 and at node scale 1.5 (render.md §6.13 T7) |
| `ui_clip.png` | same | a nested entry cut at its root's edge, unrotated and with the root at z = 30 (T8) |
| `ui_clip_chain.png` | same | clip chains: own-surface child turned by 30, mode-0 child with the parent turned, mode-0 and own-surface children past the parent (T9) |
| `ui_external.png` | same | premultiplied `Rgba8Unorm` external content at 1 : 1, scaled and rotated, over dark and light backgrounds (T10) |
| `ui_round_minimap.png` | same | a disc-alpha radar canvas over an opaque background (T11, P1 check 1) |
| `ui_p1.png` | same | a P1-shaped radar: root, radar, rim cut only by the root, marker at heading 45 (render.md §6.14) |
| `ui_filters.png` | same | scaled linear and nearest, alpha threshold at opacity 1 and 0.5, z, x and y rotations, opacity |
| `../../../../apps/client/tests/golden/test_scene.png` | `apps/client/tests/scene_golden.rs` | the client's M0 test scene without game data |
| `../../../../apps/client/tests/golden/test_scene_surface.png` | same | the same scene with a synthetic stand-in for a decoded surface |

Comparison (`crates/render/tests/support/golden.rs`): a pixel matches when every channel is
within ±4 of the reference, or of any reference pixel within 1 px (checked both ways, so
one-pixel line-rasterisation differences between drivers pass but a missing or extra line
does not); up to 0.25 % of pixels may still be outliers. On a mismatch, and on any inexact
pass, `actual`, `expected` and `diff` PNGs are written to `<cargo target dir>/tmp/golden/`
(`$CARGO_TARGET_TMPDIR/golden`: `./target/tmp/golden/` by default, or under the target dir
a machine-local `.cargo/config.toml` points to) (diff: red = outlier, yellow = forgiven by
the neighbourhood search).

Regenerate after an intended visual change, then review the new PNGs before committing:

```sh
TESSERA_BLESS=1 cargo test -p tessera-render --test golden
TESSERA_BLESS=1 cargo test -p tessera-render --test mesh_golden
TESSERA_BLESS=1 cargo test -p tessera-render --test skinned_golden
TESSERA_BLESS=1 cargo test -p tessera-render --test ui_golden
TESSERA_BLESS=1 cargo test -p tessera-client --test scene_golden
```

References were blessed on the reference workstation's hardware Vulkan adapter and are
verified to pass on Mesa llvmpipe (`WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1`, Mesa 26.1) and
on Mesa lavapipe (Vulkan, Mesa 25.2; point `VK_DRIVER_FILES` at an `lvp_icd` JSON). On a
machine whose GL vendor library is NVIDIA's, `LIBGL_ALWAYS_SOFTWARE=1` alone is ignored by EGL;
add `__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json` (and check that
the test log says `llvmpipe ... (Gl, Cpu)`). CI runs
them on Mesa lavapipe with `TESSERA_REQUIRE_GPU=1`, so a missing adapter fails instead of
skipping; Windows and macOS runners skip the golden comparisons. Check a re-blessed image
on lavapipe before committing it: lavapipe rasterises many 1-px lines packed into a few
rows near the horizon differently from hardware drivers (it can fill a solid block), which
is why the client scene's ground grid stops well short of the horizon
(`GRID_HALF_CELLS` in `apps/client/src/scene.rs`).

The `skinned_*` images were blessed the same way and verified on lavapipe from Mesa 24.2 (the
Android SDK emulator's `lvp_icd.x86_64.json` works as `VK_DRIVER_FILES`) and on llvmpipe GL
(Mesa 26.1).

The `ui_*` images were blessed on the hardware Vulkan adapter and verified on Mesa lavapipe
(Mesa 24.2) and on llvmpipe GL (Mesa 26.1). They render into `Rgba8Unorm` targets, so GL's
linear fallback of `render.md` §6.4 (which applies to sRGB targets only) does not change them.
