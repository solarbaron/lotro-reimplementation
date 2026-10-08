<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Decision records

Project decisions that are not obvious from the code. Each record is short and stays in place
once accepted; a later record supersedes it instead of rewriting it. Sessions on any machine
(human or Claude) treat accepted records as binding.

| ID | Decision | Status | Date |
|----|----------|--------|------|
| [0001](0001-clean-room-process.md) | Clean-room split: research → specs → code | Accepted | 2026-10-03 |
| [0002](0002-stack-rust-wgpu.md) | Rust + wgpu, GPL-3.0-or-later | Accepted | 2026-10-03 |
| [0003](0003-game-data-in-place.md) | Read the user's install in place, read-only; no game data in the repo | Accepted | 2026-10-03 |
| [0004](0004-server-strategy.md) | Server target: owner wants official-server compatibility; gated behind M8 legal review | Accepted | 2026-10-03 |
| [0005](0005-no-full-bevy.md) | No Bevy engine; `bevy_ecs` alone only if the M4 benchmark favours it | Accepted | 2026-10-04 |
| [0006](0006-codename.md) | Working codename "Tessera"; naming rules | Accepted | 2026-10-03 |
| [0007](0007-multi-device-workflow.md) | Multi-device workflow: git is the only shared state | Accepted | 2026-10-04 |
| [0008](0008-entity-storage.md) | Entity storage: project-owned `tessera-entity` store; `bevy_ecs` lost the M4 benchmark (kept only behind a bench feature) | Accepted | 2026-10-05 |
| [0009](0009-script-table-disassembly.md) | Incident: unauthorised client disassembly for script tables; owner authorises M4 retroactively for that one question (EU) | Accepted | 2026-10-05 |
| [0010](0010-reference-machine-laptop.md) | Reference machine: the owner's laptop (i9-10885H, RTX 2070 Super Max-Q); M3 re-baselined there | Accepted | 2026-10-05 |
| [0011](0011-standing-m4-authorisation.md) | Standing M4 (disassembly/decompilation) authorisation for the offline client; network, login and anti-tamper excluded | Accepted | 2026-10-05 |
| [0012](0012-numeric-constants.md) | Numeric algorithm constants from the client may be used in specs and code (keys, hostnames, network and bulk data tables stay forbidden) | Accepted | 2026-10-05 |
| [0014](0014-ui-input-bridge.md) | UI input bridge: UI commands as tessera-input actions with activation filters, a UI band above game contexts, pass-through pointer context, plugin code in frame steps 1 and 7 | Accepted | 2026-10-06 |
| [0015](0015-ui-rendering.md) | UI rendering: exact CPU compositing per surface, GPU composition of finished surfaces, own-opacity nested surfaces, frame budgets U1-U5 for M5 | Accepted | 2026-10-06 |
| [0016](0016-plugin-class-helper.md) | Ship a clean-room plugin `class` helper (user's original package wins if installed); extended scripting API later, separate namespace, compatibility API unchanged | Accepted | 2026-10-06 |
| [0022](0022-bundled-ui-font.md) | Bundle Noto Sans (OFL-1.1) as the UI fallback font; the only third-party binary asset | Accepted | 2026-10-08 |
| [0023](0023-plugin-host-split.md) | Plugin host split: plugin-free `tessera-script` core (no Turbine, import or chat state), `tessera-plugins` for the compatible surface; one-way `plugins` → `ui` allowance | Accepted | 2026-10-06 |

New records: copy the shape of an existing one (Context, Decision, Consequences), take the next
number, add a row here.
