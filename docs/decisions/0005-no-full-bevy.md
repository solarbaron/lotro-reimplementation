<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0005 — No Bevy engine; `bevy_ecs` alone only if it wins the M4 benchmark

**Status:** Accepted, 2026-10-04 (owner decision)

## Context
Bevy was considered as a foundation. About 60 % of the code (archive, formats, properties,
text, mesh, material, WState) is engine-independent and unaffected. Bevy's renderer is
opinionated PBR (fidelity goal favours our own), it is pre-1.0 with breaking releases every
few months, pins its own wgpu version, and adds heavy build cost.

## Decision
- No full Bevy client and no Bevy prototype client.
- M4 includes an entity-storage decision record that benchmarks `bevy_ecs` (standalone)
  against a plain project-owned store, with 50 animated characters plus streamed landblocks:
  frame time, code clarity, build cost, upgrade churn. Adopt `bevy_ecs` only if it clearly wins.
- Renderer, assets and data crates stay on our own crates either way.
