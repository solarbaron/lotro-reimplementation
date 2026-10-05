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

New records: copy the shape of an existing one (Context, Decision, Consequences), take the next
number, add a row here.
