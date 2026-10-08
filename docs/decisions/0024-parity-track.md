<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0024 — Gameplay-parity track: milestones P0–P9 between M7 and M9

**Status:** Proposed, 2026-10-09, for the owner. Not binding until the owner accepts it. On
acceptance the ROADMAP gets a P-track section built from §4 below.

## Context
The owner decided on 2026-10-08 that extended scripting (decision 0017) is the last milestone,
M9, to start only "after we have full parity and compatibility with the server and client"
(ROADMAP M9). The roadmap goes from M7 (Tessera's own protocol and test server) to M8 (gated
official-server compatibility) and then M9. Nothing between them plans the gameplay that
client parity needs: characters, inventory and equipment, skills and combat, NPCs, quests and
dialogue, loot and vendors, chat channels and fellowships, map and travel, crafting.

The decoders exist: items (`items.md`), combat data (`combat-data.md`), creatures
(`creatures.md`), quests (`quests.md`), economy (`economy.md`), map data (`map-data.md`),
particles (`particles.md`), misc records (`misc-records.md`), player characters
(`player-character.md`), instances (`instances.md`), game controls (`game-controls.md`),
interaction (`interaction.md`), `wstate` and the `wvm` decoder (`wlib-bytecode.md`). They
are used today by the explorer and dattool only.

Parity has two halves:

- **The client half**: what the original client shows and computes locally. Panels, item
  and skill tooltips, cast bars, target frames, effect icons, particle effects, maps,
  quest log and tracker, character creation, the plugin `Turbine.Gameplay` views
  (`lua-plugin-api.md` §15). It reads the user's install and is needed with every backend,
  including an official one (M8 Phase 2).
- **The server half**: the rules that decide outcomes (damage, loot, quest state). With an
  official server they are Daybreak's. With Tessera's own server they must be Tessera's.

The current rules forbid the server half on Tessera's server. LEGAL.md §8 puts a server that
reproduces "quests, NPC scripts, combat or world" in tier T3 (needs written permission from
Daybreak and Middle-earth Enterprises). LEGAL.md §11.1 rule 5 and `server-content.md` §7 (Draft,
owner confirmation pending) go further: combat, quests, NPC behaviour and "items as gameplay
(inventory, loot, vendors, crafting, trade, currency …)" are out of scope "with or without
game data". A track that simply builds the game's systems on the server would break these
rules. This proposal is shaped around them.

## Decision (proposed)

### 1. Numbering: a P-series
The track is numbered **P0–P9** ("parity"), not M7.1, M7.2, …:

- Decimal numbers read as small follow-ups of M7. These are ten milestones of their own,
  each about the size of M4.5, and most of their work is client-side, not protocol work.
- M8 and M9 keep their numbers and their meaning; M8 Phase 1 (the legal gate, paperwork
  only) can run at any time beside the P-track.
- A separate letter shows in commit trailers and checklists which track an item belongs to.

The milestone workflow (`.claude/workflows/tessera-milestone.js`) takes milestone IDs from
the ROADMAP; it must accept `P0`–`P9` once this record is accepted.

### 2. Principles

1. **Client half from the install, for every backend.** Each system's presentation reads
   the user's install (new client-side crate `tessera-gamedata`, see §3) and is fed by
   domain events of `tessera-net`. The same code serves Tessera's server, the sandbox and,
   if the gate passes, M8 Phase 2.
2. **Tessera's rules, not the game's.** Tessera's server and the sandbox run
   **Tessera's own, original game rules** (new crate `tessera-rules`) with original,
   project-made content. Like `ext-api` (decision 0017), the rules spec carries no BIN or
   OBS facts about game mechanics: no formulas, tables, numbers, progressions, quest
   scripts, creature AI or text from the data or from knowledge of the game. Genre mechanics
   (bags, a skill bar, a quest log) are designed from scratch in our own terms.
3. **The server stays data-free.** LEGAL.md §11.1 rules 1–4 and 6–7 stay as they are. The
   server never links a crate that decodes game data; a CI check on `cargo tree -p
   tessera-server` enforces it (the M7 verification ran it by hand).
4. **Stand-ins.** Original server items, creatures and abilities are shown with visuals
   and icons that **each player's client chooses from its own install** by a generic
   category ("one-handed blade", "wolf-sized quadruped"). The server never holds a list of
   DIDs; this is LEGAL.md §11.1 rule 2 applied to content (new spec `content-stand-ins.md`).
5. **Two feeds per system.** Each system's client half is tested with (a) Tessera's server
   and original content, and (b) a local **scenario feed** in the sandbox that sends events
   naming install records the client itself chose, the way an official server would name
   them. (b) proves presentation of real records without any server holding them.
6. **The game's own rules stay T3.** Running the game's combat, quests, creature AI, loot
   tables or server-side script code from install data is T3, offline in the sandbox
   included, and is not part of this track. The "sandbox backend" consumers named in the
   headers of `items.md`, `creatures.md`, `quests.md`, `economy.md`, `interaction.md`,
   `player-character.md`, `map-data.md` and `instances.md` (and the "Tessera sandbox
   server" decisions inside them) wait for T3 permission, which would come with M8's G2.
7. **Per milestone:** protocol additions as minor versions of `tessera-protocol.md` (§3,
   compatible changes only), a fuzz target per new parser, a `tessera-loadtest` bot
   scenario, hostile-input tests, the matching `Turbine.Gameplay` views, and an owner
   checklist for comparisons with the original client.

### 3. New crates

| Crate | Layer | Role | May depend on |
|---|---|---|---|
| `tessera-rules` (`crates/rules`) | L2 | Tessera's original, content-free rules engine: actors, vitals, effects, abilities, containers, loot, currency, quests and dialogue as state machines, groups. Deterministic, tick-driven, no I/O. Used by `apps/server` and the sandbox. | no Tessera crate that decodes game data (`dat`, `formats`, `havok`, `props`, `text`, `wstate`, `wvm`) |
| `tessera-gamedata` (`crates/gamedata`) | L3 | The "future gameplay-data crate" the specs name: typed stores over `props`/`wstate` for items, skills, effects, creatures, quests, economy and map records; tooltip evaluation. Client and tools only. | `formats`, `props`, `text`, `wstate`, `wvm` |

The ARCHITECTURE §3.1 OPEN about merging `props`, `text` and `wstate` into a crate of that
name would be settled as "they stay separate"; the name goes to the presentation data layer.

### 4. The milestones

| ID | Theme | Builds on | Risk tier |
|---|---|---|---|
| **P0** | Scope gate and foundations | M7, LEGAL §8, §11.1 | docs |
| **P1** | Characters, inventory and equipment | P0, M4, M5, M7 | T1 client, T2 server |
| **P2** | Targeting, skills and combat | P1, M4.5 physics | T1 / T2 |
| **P3** | Loot, currency and vendors: first playable loop | P2 | T1 / T2 |
| **P4** | NPCs, dialogue and quests | P3 | T1 / T2 |
| **P5** | Social: chat channels, fellowships, trade | P0 (trade: P1; loot modes: P3) | T1 / T2 |
| **P6** | Map, travel, mounts and instances | P1 | T1 / T2 |
| **P7** | Progression and crafting | P2, P3 | T1 / T2 |
| **P8** | Client-side game code and install-resolved presentation | P2 | T1 (client only) |
| **P9** | Parity audit | P1–P8 | T2 |

"T2 server" means T2 under the rule change of P0. Order: P1 → P2 → P3 is the shortest
path to a playable loop (create, equip, fight, loot, sell, buy). P4 gives the loop a
purpose. P5, P6 and P8 can run beside P2–P4 once their inputs exist; P7 needs combat and
the economy. All of the track needs M5's UI on main; open M4.5 and M7 items continue in
parallel and block nothing here.

#### P0: Scope gate and foundations
- **Theme:** decide whether Tessera's server may run Tessera's own rules, and lay the
  foundations every later milestone uses.
- **Specs:** LEGAL.md §11.1 rule 5 and §16 (new counsel question: is a server with
  original genre mechanics and original content still T2?); `server-content.md` §7 revised;
  new spec: `game-rules.md` (Tessera's rule model; own design, no BIN/OBS game-mechanics
  facts); new spec: `content-stand-ins.md`; `tessera-protocol.md` §3 (plan of the minors
  1.1–1.x and a capability bit per system); `test-server.md` §5 (how the content file grows).
- **Crates/apps:** docs; empty `crates/rules` and `crates/gamedata` with their dependency
  rules in ARCHITECTURE §3; CI check on the server's dependency graph.
- **Exit criteria:**
  1. The owner's decision on the rule change is recorded (decision record, LEGAL.md §11.1
     and `server-content.md` §7 updated to match), with the counsel question in LEGAL §16.
  2. `game-rules.md` and `content-stand-ins.md` Accepted after an independent review, no
     blocking OPEN; a reviewer confirms `game-rules.md` holds no fact about the game's
     mechanics.
  3. ARCHITECTURE §3 lists both crates; CI fails when `tessera-server`'s graph holds a
     data-decoding crate.
  4. The owner's answer on ground for server-side actors (`server-content.md` §3.2 OPEN:
     content-given heights, client reports, or the operator's own install) is recorded.

#### P1: Characters, inventory and equipment
- **Theme:** character creation and selection screens, character sheet, bags and equipment,
  equipping changes the worn look, item tooltips.
- **Specs:** `player-character.md`, `appearance.md`, `items.md`, `icon-layers.md`,
  `game-panels.md`, `ui-behaviour.md` (drag and drop), `lua-plugin-api.md` §15,
  `game-rules.md` (containers, equip slots), `content-stand-ins.md`, `tessera-protocol.md`
  1.1 (inventory messages), `test-server.md` (character files).
- **Crates/apps:** `gamedata`, `rules`, `protocol`, `net`, `ui`, `plugins`, `client`,
  `server`, `loadtest`.
- **Exit criteria:**
  1. A character made in the creation screen (body choices from the install) enters
     Tessera's server and survives a server restart.
  2. Items from the default world's content show stand-in icons; equipping changes the worn
     look for every observer within one snapshot interval.
  3. Scenario feed: tooltips of at least 1,000 install items chosen by the client render
     without failure; owner checklist rows for 20 items against the original client.
  4. Hostile tests (duplicate moves, full bags, stale slots, floods) cause no duplicate
     item, crash or desync; every new message is fuzzed.
  5. `GetBackpack()` and `GetEquipment()` views pass their conformance rows.

#### P2: Targeting, skills and combat
- **Theme:** selection and target frames, quickslot bar, skill use with induction bars and
  cooldowns, vitals, damage and effect display, combat animation and particle effects,
  defeat and respawn; project-made test creatures with generic behaviour (aggro radius,
  chase, leash, return).
- **Specs:** `combat-data.md` (skill and effect presentation), `interaction.md` (use range,
  cast bar, defeat), `game-controls.md` (vital display), `particles.md` (§8.10–§8.12
  pipeline), `script-table.md`, `skeleton-anim.md`, `game-panels.md` (quickslots),
  `game-rules.md` (abilities, effects, damage, actor behaviour), `tessera-protocol.md` 1.2;
  new spec: combat UI behaviour (floating text, target frame, effect bar) observed in the
  original client.
- **Crates/apps:** `rules`, `gamedata`, `render` (particles), `anim`, `entity`, `physics`,
  `ui`, `client`, `server`, `protocol`, `net`, `plugins`, `loadtest`.
- **Exit criteria:**
  1. A player defeats a test creature with skills on Tessera's server; defeat and respawn
     work for both sides.
  2. One-hour soak, 32 bots fighting: 0 vitals desync (client shows the server value within
     one snapshot), no memory growth, server tick within budget.
  3. Every particle descriptor decodes or is categorised; script-table particle nodes play
     in the walker.
  4. Hostile tests: cooldown bypass, out-of-range use and command floods are refused.
  5. Owner checklist rows (cast bar, floating text, target frame, effect icons) pass.

#### P3: Loot, currency and vendors (first playable loop)
- **Theme:** corpse loot with original loot tables, wallet, vendor buy, sell and buy-back,
  stack splitting.
- **Specs:** `economy.md` (vendor and loot window presentation), `items.md`,
  `game-rules.md` (loot, currency, vendors), `server-content.md` (vendor and loot content),
  `tessera-protocol.md` 1.3.
- **Exit criteria:**
  1. A `loop` scenario (create, equip, kill, loot, sell, buy, equip) passes 100 bot runs on
     the default world, and a human completes it in the client.
  2. The same loop passes offline: the sandbox runs `tessera-rules` with the default world.
  3. Currency and items balance against the server ledger over a one-hour 32-bot soak.
  4. No duplication under concurrent loot, double sell or disconnect mid-transaction.

#### P4: NPCs, dialogue and quests
- **Theme:** NPC use, dialogue windows, quest log and tracker, objective counters, rewards,
  quest markers on the minimap; original quests and dialogue text.
- **Specs:** `interaction.md`, `quests.md` (log, tracker and status presentation),
  `game-panels.md`, `chat-window.md` (quest lines), `string-table.md` (UI text),
  `game-rules.md` (quests, dialogue), `server-content.md` (quest content rules and
  `check-content`), `tessera-protocol.md` 1.4.
- **Exit criteria:**
  1. The default world has at least three original quests (defeat, collect, talk), finished
     end to end by bots and by a human; state survives reconnect and server restart.
  2. Scenario feed: the log and tracker show install quests the client chose, with no
     failure across all quest records.
  3. `check-content` enforces the quest and dialogue text rules; its parser is fuzzed.

#### P5: Social: chat channels, fellowships, trade
- **Theme:** channels beyond M7 (tells, join and leave, fellowship channel), fellowships
  with a leader and a shared vitals panel, loot modes, player trade, friends and ignore
  lists, synchronised fellowship music over the network.
- **Specs:** `chat-window.md`, decision 0020, `tessera-protocol.md` §10 and 1.5,
  `game-controls.md` (social managers), `instruments.md`, `abc-dialect.md`, `game-panels.md`,
  `game-rules.md` (groups, trade).
- **Exit criteria:**
  1. Six clients form a fellowship, see each other's vitals and chat on its channel; leader
     change, removal and disbanding work.
  2. Trade is two-phase and atomic: no duplication when a side disconnects mid-trade.
  3. A synchronised `/play` among a fellowship starts on every client within the tolerance
     of `instruments.md`.
  4. Channel limits hold under hostile floods.

#### P6: Map, travel, mounts and instances
- **Theme:** world map and minimap with markers (install map images, client-side place
  lookup), travel between content-defined points, mounts, private instances for original
  content, the instance-finder UI.
- **Specs:** `map-data.md`, `game-panels.md`, `coordinates.md`, `game-controls.md` (mounted
  gaits), `instances.md` (presentation), `walker.md`, `test-server.md` §7,
  `server-content.md` §5, `tessera-protocol.md` 1.6.
- **Exit criteria:**
  1. The map panel renders every region of the install with fellowship and quest markers.
  2. Travel between two content-defined points works; mounted speeds pass server movement
     validation.
  3. A fellowship enters a private instance and leaves it; observers outside never see it.

#### P7: Progression and crafting
- **Theme:** experience and levels, trait and skill training panels, gathering, crafting
  panel and recipes, all with original rules and content.
- **Specs:** `combat-data.md` (traits, progressions presentation), `player-character.md`
  (levels), `economy.md` (recipe presentation), `interaction.md` (gathering hotspots),
  `game-rules.md` (progression, crafting), `tessera-protocol.md` 1.7.
- **Exit criteria:**
  1. A bot levels from 1 to the content's cap and trains every original skill.
  2. A gather, craft, equip chain passes as a bot scenario and by hand.
  3. Scenario feed: install recipes and trait trees render without failure.

#### P8: Client-side game code and install-resolved presentation
- **Theme:** what the original client computes locally for records a server names:
  tooltip and description evaluation (`items.md` §6, `combat-data.md` progressions) and the
  class-library code the client itself runs (a `wvm` interpreter for those classes only).
- **Specs:** `wlib-bytecode.md`, `wlib.md`, `wstate.md`, `items.md`, `combat-data.md`; new
  spec: the classes the client runs locally and the native functions they call.
- **Crates/apps:** `wvm` (interpreter), `gamedata`, `client`. Never linked by the server.
  Running server-side game code stays T3 (principle 6).
- **Exit criteria:**
  1. The interpreter passes the spec's conformance vectors and is fuzzed (bytecode and
     natives at the boundary).
  2. Tooltips of 50 items and 50 skills match the original client on the owner's checklist.

#### P9: Parity audit
- **Theme:** one checklist of every client system, compared by the owner with the original
  client; independent review of the protocol minors; gameplay soaks.
- **Exit criteria:**
  1. `docs/checklists/parity.md`: every row passes or has an owner-accepted deviation.
  2. `tessera-protocol.md` 1.x Accepted after an independent review with a second codec (as
     M7 criterion 2).
  3. A one-hour soak between two machines on a LAN, 16 players running the P3 loop and P4
     quests: 0 desync, no leak.
  4. Every new parser has a fuzz target; frame-time budgets hold in the busiest scenario on
     the reference machine (`TESSERA_PERF=1`, decision 0010).

### 5. Place in the roadmap, and M9
- Order: M7 → **P0–P9** → M9. M8 Phase 1 runs beside the track whenever the owner wants.
  If the gate passes, M8 Phase 2 plugs a compatibility backend into the client half built
  here.
- **M9 starts only after P9 exits and either M8 Phase 2 exits, or the M8 gate has failed.**
  In the second case P9's exit is the "client parity with Tessera's own server" that
  ROADMAP M9 names as the fallback.
- **If the owner rejects the rule change in P0**, the track keeps only the client halves and
  the scenario feed: no server-side parts of P1–P7, no playable loop on Tessera's server.
  P9 then audits presentation only, and the owner restates what "parity with Tessera's own
  server" means for M9.

## Consequences
- On acceptance: a P-track section in the ROADMAP (§1 row and one section per milestone),
  the workflow accepts P IDs, P0 starts. ARCHITECTURE §3 and §19 grow with the new crates
  and events in P0.
- LEGAL.md §11.1 rule 5 and `server-content.md` §7 change only through P0's own decision
  record; this record changes neither. Private or LAN use only (rule 6) still holds, and the
  LEGAL.md banner (counsel before networked features beyond the sandbox) still applies.
- Main risk: original rules drifting toward the game's numbers and behaviour. Guard: the
  rules and content specs carry no game-mechanics facts, reviewers check it, and owner
  observations are used only for client presentation.
- Size: ten milestones, most about the size of M4.5. The dirty-room work is limited to
  presentation behaviour (P2's combat UI spec, P8's client-run classes).

## Questions for the owner
1. Accept the P-series and its order?
2. Change LEGAL.md §11.1 rule 5 and `server-content.md` §7 to allow Tessera's original rules
   and content on Tessera's server (P0), with the counsel question?
3. Confirm that running the game's own rules from the install, offline included, is T3 and
   waits for written permission?
4. Ground for server-side actors: content-given heights, client reports, or the operator's
   own install read-only (`server-content.md` §3.2 OPEN)?
5. Is P8 (client-run game code) part of "full parity", or does it move to M8 Phase 2?
