<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Golden images

PNG references for the headless golden-image tests. Every image here is rendered by
Tessera's own code from a procedural scene; none contains or is derived from game data.

| Image | Test | Scene |
|---|---|---|
| `render_test_scene.png` | `crates/render/tests/golden.rs` | renderer primitives: grid, axes, box outline, nearest/linear/sRGB/linear textures, depth, blending, overlay, atlas UVs, render-to-texture |
| `../../../../apps/client/tests/golden/test_scene.png` | `apps/client/tests/scene_golden.rs` | the client's M0 test scene without game data |
| `../../../../apps/client/tests/golden/test_scene_surface.png` | same | the same scene with a synthetic stand-in for a decoded surface |

Comparison (`crates/render/tests/support/golden.rs`): a pixel matches when every channel is
within ±4 of the reference, or of any reference pixel within 1 px (checked both ways, so
one-pixel line-rasterisation differences between drivers pass but a missing or extra line
does not); up to 0.25 % of pixels may still be outliers. On a mismatch, and on any inexact
pass, `actual`, `expected` and `diff` PNGs are written to `target/tmp/golden/` (diff: red =
outlier, yellow = forgiven by the neighbourhood search).

Regenerate after an intended visual change, then review the new PNGs before committing:

```sh
TESSERA_BLESS=1 cargo test -p tessera-render --test golden
TESSERA_BLESS=1 cargo test -p tessera-client --test scene_golden
```

References were blessed on the reference workstation's hardware Vulkan adapter and are
verified to pass on Mesa llvmpipe (`WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1`, Mesa 26.1) and
on Mesa lavapipe (Vulkan, Mesa 25.2; point `VK_DRIVER_FILES` at an `lvp_icd` JSON). CI runs
them on Mesa lavapipe with `TESSERA_REQUIRE_GPU=1`, so a missing adapter fails instead of
skipping; Windows and macOS runners skip the golden comparisons. Check a re-blessed image
on lavapipe before committing it: lavapipe rasterises many 1-px lines packed into a few
rows near the horizon differently from hardware drivers (it can fill a solid block), which
is why the client scene's ground grid stops well short of the horizon
(`GRID_HALF_CELLS` in `apps/client/src/scene.rs`).
