<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Terrain golden images

PNG references for `crates/terrain/tests/golden.rs`. Every image is rendered by Tessera's own
code from a synthetic heightfield generated in the test (integer-hashed value noise and a
rational bump, evaluated with basic f64 arithmetic only, so it is identical on every
platform); none contains or is derived from game data.

| Image | Scene |
|---|---|
| `terrain_patch.png` | 3 × 3 landblocks at mixed levels of detail (`LODS` in the test), seamless normals, default height/slope vertex colours, lit; oblique view |
| `terrain_lod_wireframe.png` | the same patch from straight above (north up), unlit vertex colours with the triangle wireframe: level 0 in the centre, levels 1–4 around it, and the stitch strips where levels meet |

Comparison and drivers: as for `crates/render/tests/golden/README.md` (the test includes
`crates/render/tests/support/golden.rs` and `gpu.rs`). Blessed on the reference workstation's
hardware Vulkan adapter on 2026-10-04 and verified on Mesa llvmpipe (GL, Mesa 26.1; 13
forgiven outliers in the wireframe image) and Mesa lavapipe (Vulkan, Mesa 25.2; 0 outliers).

Regenerate after an intended visual change, then review the PNGs before committing:

```sh
TESSERA_BLESS=1 cargo test -p tessera-terrain --test golden
```
