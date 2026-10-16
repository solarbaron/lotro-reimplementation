<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0024 — Parity track, client half: milestones P0–P8 between M7 and M9

**Status:** Accepted (owner, 2026-10-09). Amended 2026-10-11 by
[decision 0034](0034-official-server-compatibility.md): M8 is in scope without a gate, so
"if the M8 gate passes" below reads "with M8", "M8 Phase 1 (the legal gate)" no longer
exists, and the fallback "if the gate fails" applies only if M8 stops under LEGAL §9.4 (an
explicit objection from a rights holder). M8 may run beside the track; M9 still starts only
after P8 and M8. The client half only, LEGAL §11.1 rule 5 and the data-free server are
unchanged. Amended 2026-10-16 by the owner (answer to open item 1): **P7 (client-run game
code) is dropped.** With server compatibility (M8) the official server runs that code, so
Tessera does not build a client-side interpreter for it; the track is P0–P6 and P8, and P8
depends on P1–P6.

**Owner's choice: "Client half only."** LEGAL.md §11.1 rule 5 and `server-content.md` §7
stay as they are. The track builds only what the original client shows and computes
locally, from the user's install. It is driven by a local scenario feed in the sandbox and,
if the M8 gate passes, by an official server (M8 Phase 2). Tessera's own server keeps M7's
scope (accounts, presence, movement, chat, appearance as opaque references, simple
project-made test actors) and gains nothing rule 5 does not allow.

## Context
The owner decided on 2026-10-08 that extended scripting (decision 0017) is the last
milestone, M9, to start only after full parity with the client and compatibility with the
server. Nothing between M7 and M9 plans the gameplay presentation that client parity needs:
character creation, inventory and equipment, target frames, skills and cast bars, effects,
dialogue, quest log, vendors, social panels, maps, progression and crafting panels.

The decoders exist: items (`items.md`), combat data (`combat-data.md`), creatures
(`creatures.md`), quests (`quests.md`), economy (`economy.md`), map data (`map-data.md`),
particles (`particles.md`), misc records (`misc-records.md`), player characters
(`player-character.md`), instances (`instances.md`), game controls (`game-controls.md`),
interaction (`interaction.md`), `wstate` and the `wvm` decoder (`wlib-bytecode.md`). Today
only the explorer and dattool use them.

Parity has two halves. The **client half** (panels, tooltips, maps, quest log, creation
screen, target frames, cast bars, effects, the plugin `Turbine.Gameplay` views of
`lua-plugin-api.md` §15, and game code the client runs locally) reads the user's install and
is needed with every backend. The **server half** (the rules that decide damage, loot, quest
state) is Daybreak's on an official server and is not built by Tessera (LEGAL.md §8 T3,
§11.1 rule 5).

## Decision

### 1. Numbering
The track is numbered **P0–P8** ("parity"), not M7.x: nine milestones of their own, mostly
client work, each about the size of M4.5. M8 and M9 keep their numbers and meaning; M8
Phase 1 (the legal gate, paperwork only) can run at any time beside the track. The milestone
workflow (`.claude/workflows/tessera-milestone.js`) must accept `P0`–`P8`.

### 2. Principles
1. **Client half only, from the install.** Each system's presentation reads the user's
   install through `tessera-gamedata` (§3) and is fed by `tessera-net` domain events. The
   same code serves the sandbox, M7's server (within its scope) and, if the gate passes, M8
   Phase 2.
2. **Two drivers.** (a) The **scenario feed** (§4), always available offline, for every
   system. (b) An official server through M8 Phase 2, only if the gate passes. M7's server
   drives only what it already carries: other players, their appearance, chat and test
   actors.
3. **The server stays data-free.** LEGAL.md §11.1 is unchanged. `tessera-server` never links
   a crate that decodes game data, `tessera-gamedata` or the scenario feed; a CI check on
   `cargo tree -p tessera-server` enforces it (the M7 verification ran it by hand).
4. **The game's own rules stay T3.** Running the game's combat, quests, creature AI, loot
   tables or server-side script code from install data is T3, offline in the sandbox
   included, and is not part of this track. The scenario feed plays back scripted events; it
   never computes outcomes. The "sandbox backend" consumers named in the headers of
   `items.md`, `creatures.md`, `quests.md`, `economy.md`, `interaction.md`,
   `player-character.md`, `map-data.md` and `instances.md` (and the "Tessera sandbox
   server" decisions inside them) mean running those rules; they wait for T3 permission,
   which would come with M8's G2. The scenario feed is not one of them.
5. **Client-side game code is in scope.** What the original client evaluates locally for
   records a server names (tooltip and description text, progression display, the
   class-library code the client itself runs) belongs to the client half. Server-side game
   code stays T3.
6. **Per milestone:** a scenario per system, real-data tests that read `TESSERA_GAME_DIR`
   and skip when it is unset, a fuzz target per new parser, the matching `Turbine.Gameplay`
   views, and owner checklist rows for comparison with the original client. Owner rows go
   to `docs/checklists/owner-observations.md` and block nothing until P8. A protocol minor
   is added only for something rule 5 allows.

### 3. New crate

| Crate | Layer | Role | May depend on |
|---|---|---|---|
| `tessera-gamedata` (`crates/gamedata`) | L3 | The "future gameplay-data crate" the specs name: typed stores over `props`/`wstate` for items, skills, effects, creatures, quests, economy, map and progression records; tooltip and description evaluation; scenario selector resolution (§4). Client and tools only. | `formats`, `props`, `text`, `wstate`, `wvm` |

The ARCHITECTURE §3.1 OPEN about merging `props`, `text` and `wstate` into a crate of that
name is settled as "they stay separate"; the name goes to this presentation data layer.

### 4. The scenario feed
A feature of the sandbox backend in `tessera-net`, specified in a new spec,
`scenario-feed.md` (written in P0).

- **Input:** a scenario file in a project-defined text format. It describes actors,
  containers, vitals, effects, casts, dialogue pages, quest states, vendor lists, map
  markers, chat lines and the like, as timed steps and as scripted answers to client
  commands ("when the player uses quickslot 1, start cast C, then apply effect E").
- **Records are chosen by the client.** A scenario names records by selectors ("items with
  equip slot S", "every quest record", "n skills of class K, seed 7"). The client resolves
  them against its own install with `tessera-gamedata` and hands the resolved scenario to
  the backend, the way it already hands the sandbox its `WorldSource` (ARCHITECTURE §19).
  `tessera-net` decodes no game data.
- **Values are scripted, never derived.** Numbers, outcomes and timings come from the
  scenario file. The feed has no formulas, tables or behaviour taken from the game.
- **Output:** the same domain events an official server backend would produce, so the
  client half cannot tell the feed from a server.
- **Committed scenarios** are original and synthetic and use selectors only, no DID lists
  (CLAUDE.md content rules). Scenarios that name DIDs a user picked stay local.
- **Tests:** deterministic and headless, so real-data tests, golden images and the
  explorer's sheet mode can use it. The scenario parser is hostile-input safe and fuzzed.

### 5. The milestones

| ID | Theme | Builds on |
|---|---|---|
| **P0** | Foundations: scenario feed and `tessera-gamedata` | M5, M7 |
| **P1** | Characters, inventory and equipment | P0, M4 |
| **P2** | Targeting, skills and combat presentation | P1, M4.5 |
| **P3** | NPC windows: dialogue, quests, vendors and loot | P1 |
| **P4** | Social panels | P1 |
| **P5** | Map, travel, mounts and instances | P1 |
| **P6** | Progression and crafting panels | P2 |
| ~~**P7**~~ | ~~Client-run game code~~: dropped by the owner 2026-10-16 (not needed with M8) | — |
| **P8** | Parity audit | P1–P6 |

Risk tier: T1 throughout (client only, offline). P3–P6 can run in parallel once P1 is in.
Open M4.5 and M7 items continue beside the track and block nothing here. "Every record"
below means every record of that kind in the install at `TESSERA_GAME_DIR`; a record that
cannot be shown is listed with a reason (categorised), never a crash.

#### P0: Foundations
- **Specs:** new `scenario-feed.md`; ARCHITECTURE §3 (`tessera-gamedata`) and §19 (the new
  domain events, grouped by system).
- **Exit criteria:**
  1. `scenario-feed.md` Accepted after an independent review, no blocking OPEN.
  2. `tessera-client --scenario <file>` plays a synthetic scenario headless; the same file
     gives the same event sequence on every run.
  3. The scenario parser has a fuzz target; hostile files are refused without a panic.
  4. ARCHITECTURE §3 lists `tessera-gamedata`; CI fails when `tessera-server`'s dependency
     graph holds a data-decoding crate, `tessera-gamedata` or the feed.

#### P1: Characters, inventory and equipment
- **Theme:** creation and selection screens, character sheet, bags and equipment, worn
  look, item tooltips.
- **Specs:** `player-character.md`, `appearance.md`, `items.md`, `icon-layers.md`,
  `game-panels.md`, `ui-behaviour.md`, `lua-plugin-api.md` §15, `scenario-feed.md`.
- **Exit criteria:**
  1. The creation screen offers every race, class and body choice of the install; a made
     character enters the sandbox, and on M7's server other clients see its look
     (appearance as opaque references, rule 2).
  2. Every item record's tooltip renders (or is categorised) in a real-data test.
  3. Equipping an item from a scenario changes the worn look in the walker; observers on
     M7's server see it through `AppearanceChanged`.
  4. `GetBackpack()` and `GetEquipment()` pass their conformance rows on a scenario.

#### P2: Targeting, skills and combat presentation
- **Theme:** selection and target frames, quickslot bar, cast and induction bars,
  cooldowns, vitals, floating text, effect icons, combat animation and particles, defeat
  display.
- **Specs:** `combat-data.md`, `interaction.md`, `game-controls.md`, `particles.md`,
  `script-table.md`, `skeleton-anim.md`, `game-panels.md`; new spec: combat UI behaviour
  (floating text, target frame, effect bar) observed in the original client.
- **Exit criteria:**
  1. A combat scenario (target, cast, effect, damage numbers, defeat) runs headless with
     every element on screen; golden images pass.
  2. Every skill and effect record's tooltip and icon render (or are categorised).
  3. Every particle descriptor decodes or is categorised; script-table particle nodes play
     in the walker.
  4. M7's test actors can be selected and show a target frame.

#### P3: NPC windows: dialogue, quests, vendors and loot
- **Theme:** NPC use, dialogue windows, quest log and tracker, objective counters, quest
  markers, vendor and loot windows, wallet display.
- **Specs:** `interaction.md`, `quests.md`, `economy.md`, `game-panels.md`,
  `chat-window.md`, `string-table.md`.
- **Exit criteria:**
  1. Log and tracker show every quest record from a scenario without failure.
  2. Dialogue, vendor and loot scenarios render with install text and icons; golden images
     pass.

#### P4: Social panels
- **Theme:** chat channels beyond M7 (tells, join and leave), fellowship panel with shared
  vitals, trade window, friends and ignore lists, synchronised fellowship music.
- **Specs:** `chat-window.md`, decision 0020, `game-controls.md`, `instruments.md`,
  `abc-dialect.md`, `game-panels.md`.
- **Exit criteria:**
  1. Fellowship, trade and friends scenarios render; leader change, removal and disbanding
     are shown.
  2. Tells and channel join and leave work on M7's server (chat, rule 5).
  3. A synchronised `/play` from a scenario starts within the tolerance of
     `instruments.md`.

#### P5: Map, travel, mounts and instances
- **Theme:** world map and minimap with markers, place lookup, travel and instance-finder
  windows, mounts and mounted gaits.
- **Specs:** `map-data.md`, `coordinates.md`, `game-controls.md`, `instances.md`,
  `walker.md`, `game-panels.md`.
- **Exit criteria:**
  1. The map panel renders every map region with scenario markers.
  2. A mounted character walks in the walker with the install's mounted gaits.
  3. Travel and instance-finder scenarios render.

#### P6: Progression and crafting panels
- **Theme:** experience bar, levels, trait and skill-training panels, crafting panel and
  recipes, gathering hotspots.
- **Specs:** `combat-data.md`, `player-character.md`, `economy.md`, `interaction.md`.
- **Exit criteria:**
  1. Every trait tree and recipe record renders (or is categorised).
  2. A progression scenario (level up, train, craft) runs headless; golden images pass.

#### P7: Client-run game code (dropped)
> **Dropped by the owner on 2026-10-16:** "it won't be needed with the server
> compatibility". With M8 the official server runs this code. The text below is kept as
> the record of what was planned; nothing of it is built.

- **Theme:** the class-library code the client itself runs (a `wvm` interpreter for those
  classes only, never linked by the server).
- **Specs:** `wlib-bytecode.md`, `wlib.md`, `wstate.md`; new spec: the classes the client
  runs locally and the native functions they call.
- **Exit criteria:**
  1. The interpreter passes the spec's conformance vectors and is fuzzed (bytecode and
     natives at the boundary).
  2. Every client-run class in the install loads under the load-time rules; the census
     lists any it cannot run.

#### P8: Parity audit
- **Theme:** one checklist of every client system, compared by the owner with the original
  client.
- **Exit criteria:**
  1. `docs/checklists/parity.md`: every row passes or has an owner-accepted deviation
     (tooltips of 50 items and 50 skills included).
  2. Every new parser has a fuzz target.
  3. Frame-time budgets hold in the busiest scenario on the reference machine
     (`TESSERA_PERF=1`, decision 0010).

### 6. Place in the roadmap, and M9
- Order: M7 → **P0–P8** → M9. M8 Phase 1 runs beside the track. If the gate passes, M8
  Phase 2 plugs a compatibility backend into the client half built here.
- **M9 starts only after P8 exits and after M8 or its fallback.** With the client-only
  scope, "server compatibility" for M9 means M8 Phase 2's exit if the gate passes;
  otherwise it means the client half on the scenario feed (P8) plus M7's own server.

## Consequences
- ROADMAP §1 gets a P-track row and a section pointing here; the workflow accepts P IDs;
  P0 starts. ARCHITECTURE §3 and §19 grow in P0.
- LEGAL.md and `server-content.md` are not changed. Private or LAN use only (rule 6) and
  the LEGAL.md banner still apply to M7's server.
- No playable loop on Tessera's server: outcomes exist only as scripted scenario events or
  from an official server.
- Dirty-room work is limited to presentation behaviour (P2's combat UI spec; P7's
  client-run classes were dropped on 2026-10-16).

## Open for the owner
1. ~~Is P7 (client-run game code) part of "full parity", or does it move to M8 Phase 2?~~
   **Answered 2026-10-16 (owner):** P7 is dropped; it "won't be needed with the server
   compatibility", because the official server runs that code.
2. Does relaying a synchronised `/play` start through M7's server count as chat under rule
   5? Until answered, P4 tests it on the scenario feed only.
