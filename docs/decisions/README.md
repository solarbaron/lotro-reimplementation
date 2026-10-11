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
| [0013](0013-physics-crate.md) | Collision and movement in a new L3 crate `tessera-physics` (depends only on `formats`, `props`; `world` → `physics` sibling allowance; collision sets carry water; controller reused by the M7 server, set building OPEN); controller on its own fixed 1/60 s step owned by `physics`, with residency and input-validation rules; 30 Hz tick kept for other local simulation | Accepted | 2026-10-06 |
| [0014](0014-ui-input-bridge.md) | UI input bridge: UI commands as tessera-input actions with activation filters, a UI band above game contexts, pass-through pointer context, plugin code in frame steps 1 and 7 | Accepted | 2026-10-06 |
| [0015](0015-ui-rendering.md) | UI rendering: exact CPU compositing per surface, GPU composition of finished surfaces, own-opacity nested surfaces, frame budgets U1-U5 for M5 | Accepted | 2026-10-06 |
| [0016](0016-plugin-class-helper.md) | Ship a clean-room plugin `class` helper (user's original package wins if installed); extended scripting API later, separate namespace, compatibility API unchanged | Accepted | 2026-10-06 |
| [0017](0017-extended-scripting.md) | Extended scripting (M9, after full parity; planned as M5.5): Lua 5.1 ext apartments under `Ext`, capability-granted modules, fail-closed gate (never official servers), patched Lua in an external fork | Accepted | 2026-10-07 |
| [0018](0018-incident-review-notes-in-clean-prompts.md) | Incident: dirty-room reports reached clean-room implementer prompts in the M4.5 close-out; exposed work held; process rules; two owner decisions pending | Accepted (incident) | 2026-10-07 |
| [0019](0019-audio-backend.md) | Audio backend: cpal output behind the `device` feature, a project-owned lock-free mixer implementing `sound.md` §6–§8; null and offline backends for headless runs and tests; kira not used | Accepted | 2026-10-07 |
| [0020](0020-chat-command-routing.md) | Chat command routing: one client-side dispatcher (`chat-window.md` §7 parser and registry) fed by the chat line and `--command`; registered commands (music, help, plugins, client options, channel words) run locally, only channel commands send `ChatSend`; unknown commands answer locally and are never sent; same local behaviour with or without `--server`; M5 plugins register through `HostApi` hooks; typing in chat never moves the character | Accepted | 2026-10-08 |
| [0021](0021-incident-exit-verifier-summaries.md) | Incident: the milestone exit verifier's prompt carried spec writers' reports; workflow fixed, exposed verifiers' code audited (0018 B remedy) | Accepted | 2026-10-08 |
| [0022](0022-bundled-ui-font.md) | Bundle Noto Sans (OFL-1.1) as the UI fallback font; the only third-party binary asset | Accepted | 2026-10-08 |
| [0023](0023-plugin-host-split.md) | Plugin host split: plugin-free `tessera-script` core (no Turbine, import or chat state), `tessera-plugins` for the compatible surface; one-way `plugins` → `ui` allowance | Accepted | 2026-10-06 |
| [0024](0024-parity-track.md) | Parity track, client half (owner: "client half only"): milestones P0–P8 between M7 and M9; gameplay presentation from the install, driven by a sandbox scenario feed (new `scenario-feed.md`) and, if the gate passes, M8 Phase 2; new crate `tessera-gamedata`; LEGAL §11.1 rule 5 unchanged, the game's own rules stay T3; M9 only after P8 and M8 or its fallback | Accepted | 2026-10-09 |
| [0025](0025-p1-character-look-on-test-server.md) | P1 on M7's server: scenario overlay (`--scenario` with `--server`: the feed runs only the Character and Inventory groups over the test-server backend; equipment changes go out as coalesced `AppearanceSet`s, server unchanged and feed-free); protocol 1.1 (character looks: bounded opaque creation-choice property values in `Appearance`, stored and relayed by the test server; optional held-item slot in `Equipped`); creation mapping of `character-screens.md` §7.1.1 adopted; plain-sandbox creation; feed steps `remove-character`, `stat`, `item-property`; LEGAL §11.1 rule 5 unchanged | Accepted (review 2026-10-10; owner items open) | 2026-10-10 |
| [0030](0030-tessera-protocol-transport.md) | Tessera protocol transport: QUIC (quinn, rustls on ring), pinned Ed25519 server keys, control/world/chat streams plus RFC 9221 datagrams; TRD v1 frozen, unused by the protocol (ID out of sequence for parallel branches) | Accepted | 2026-10-06 |
| [0031](0031-parity-panel-goldens.md) | Panel golden images for the parity track (P2, P3, P6): committed synthetic goldens (no install, CPU painter or headless renderer, `TESSERA_BLESS`); local-only real-data goldens under `${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/<package>/<test binary>/<function>/<install version>/`, never committed, first run "blessed" not "passed", deterministic capture; "golden images pass" = bless, judge by eye, re-run and match (ID out of sequence for parallel branches) | Proposed | 2026-10-10 |

New records: copy the shape of an existing one (Context, Decision, Consequences), take the next
number, add a row here.
