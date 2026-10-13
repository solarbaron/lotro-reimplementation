<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Scenario feed: scripted domain events for the sandbox

| | |
|---|---|
| **Status** | **Accepted** for the parts labelled "P5 travel" (revision 8: independent review 2026-10-11, CLEAN_ROOM.md §5.3 passed, every item; record in §18.P5); every other part keeps the status that follows. **Accepted** (revision 7, the P4 social parts of §18.5: independent review 2026-10-11, CLEAN_ROOM.md §5.3 passed, every item; H1–H6 agree with decision 0035 D2–D4, `tessera-protocol.md` v1.2 §10.6 and Appendix A, and the code on branch `p4`; Vector H re-derived line by line, the 8 hostile rows checked; non-blocking notes (a)–(c) in §18.5; written as the unnumbered "P4 social" revision and numbered 7 at the merge with P2 and P3, 2026-10-11). **Accepted** (revision 6, the P2 parts of §18.4: independent review 2026-10-11 of commit 7b14f27, CLEAN_ROOM.md §5.3 passed, every item; D1–D4 agree with decision 0032, LEGAL.md §11.1 rule 5 and decision 0024 §2; record in §18.4.1). **Accepted** (revision 5, the P3 parts of §18.P3: independent review 2026-10-11, CLEAN_ROOM.md §5.3 passed, every item; record in §18.P3). Before: Review (revision 7, "P4 social", 2026-10-11, spec writer, design, [decision 0035](../decisions/0035-p4-social-on-test-server.md) D2–D4, additive, §18.5: synchronised performances (`perform … sync`, `perform-start`, `perform-withdraw`), fellow vitals (`fellow-vital`), the commands `ReadyPerformance`, `StartPerformance`, `DisbandFellowship`, the remote backend at protocol minor 2 (§9.8), the plain sandbox's start answer, Vector H, P4 hostile and backend cases; no section renumbered, no earlier rule changed; an independent review must follow). Before: **Review** (revision 6, the P2 parts of §18.4, 2026-10-10, spec writer, design: [decision 0032](../decisions/0032-p2-targeting-on-test-server.md) D1–D4: local selection on the test-server backend, §9.5, §9.7, §12.5.1, §12.5.2, §14.9, §14.11, §15; an independent review by a session that wrote no part of revision 6 or of decision 0032 must check the changes listed in §18.4 and the cases of §14.9 and §14.11; P2's revision was written as revision 5 and renumbered 6 at the merge with P3, 2026-10-11). Before: **Accepted** (revision 4, independent review 2026-10-10: CLEAN_ROOM.md §5.3 passed, every item; D1–D5 agree with decision 0025, `tessera-protocol.md` v1.1 §6.2.17 and Appendix A, and the code on branch `p1`; Vector G and Vector F's changed lines re-derived, the 15 new hostile rows checked; non-blocking notes (a)–(f) in §18.3). Before: Review (revision 4, 2026-10-10, spec writer, design: [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D1–D5, §18.3; an independent review by a session that wrote no part of revision 4 must check the changes listed there, Vectors F and G and the new hostile rows). Before: Accepted (revision 3, 2026-10-09). An independent review by a session that wrote none of revisions 1–3 re-checked R1–R31, S1–S31, Vectors A–F and the hostile table and passed (P0 exit criterion 1); no blocking OPEN. The non-blocking OPENs of §3.1, §8.3, §11.1, §12.4, §13 and §16 stay open for the spec owner; the reviewer's rulings on them are review notes in the review record and change no fact |
| **Last updated** | 2026-10-13
| **Author role** | Spec writer (design) |
| **Reviewer** | Revision 2: spec reviewer (Clean-Room-Role: review), a session separate from the first-draft author, 2026-10-09; it also wrote R1–R31, so those fixes are unreviewed. Revision 3: pending (an independent session). Revision 4: a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of decision 0025, of the P1 revisions of `scenario-feed.md`, `tessera-protocol.md`, `test-server.md` and `server-content.md`, or of the code, 2026-10-10 (passed, §18.3). Revision 5 (P3 parts): a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of the P3 revision, of `npc-windows.md` or of the P3 code, 2026-10-11 (passed, §18.P3). Revision 6 (P2 parts): a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of revision 6, of decision 0032, of the other P2 specs or of the P2 code, 2026-10-11 (passed, §18.4.1). Revision 7 ("P4 social" parts): a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of decision 0035, of the P4 revisions of `tessera-protocol.md`, `test-server.md` or `scenario-feed.md`, or of the code, 2026-10-11 (passed, §18.5) Revision 8 (P5 travel parts): a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of the P5 revision or the P5 code, 2026-10-11 (passed, §18.P5). |
| **Review record** | 2026-10-11 (P5 travel parts, T-D1 … T-D10): independent review by a session that wrote no part of the P5 revision or the P5 code, against CLEAN_ROOM.md §5.3, decision 0024 §2 and §4 (client half only; values scripted, never derived) and LEGAL.md §11.1 rule 5: **passed, every item**. Every P5 statement is a functional fact of Tessera's own feed format (step keywords, options, event fields with types and ranges, log forms, limits) or an install-derived reference marked for its source; no code or pseudo-code (the scenario and log blocks are Tessera's own text format, the braces name event fields as §9 does); API, event, property and label names are interoperability identifiers; all vector text is invented (§14.T3 notes this), no game strings, assets, keys, hostnames or captures; nothing runs in or is linked by `tessera-server` (no protocol minor); no gate touched. Provenance "Design" is this own-format spec's convention (as in revisions 4 and 5); the one install-derived claim (the four dictionary names `MapNote_Type`, `FellowshipMakerUI_GroupSize`, `FellowshipMakerUI_DifficultyTier`, `Mount_Type` with their kinds) carries an inline M1 mark. Spot-checks (read-only, 2026-10-13): the four dictionary names and the enum mappers are used by `crates/gamedata` and `crates/net` selector resolution over the install (passing P5 real-data tests), and `MapNote_Type`/`Mount_Type` are decoded as bitfields in `crates/formats`, corroborating the M1 claim; the cross-referenced mount facts (status mapper 0x230002C9, war-steed bit 0x38, vital mapper 0x23000003) were confirmed against the `lua-plugin-api.md` §15.5 BIN pass reviewed in parallel. Vectors T1–T3 and the ranges of the hostile rows are internally consistent with the §6.5, §9, §10.3, §11.1 and §13 rules. **Rulings on the P5 OPENs** (review notes, not facts): the numeric-enum-key question (§16) is the same owner policy already open for P1–P3, non-blocking; `MountChanged.combat` as a Tessera field (§16) and the implementer's seven conservative-implementation points are endorsed as implemented and left for the spec owner. **Dependency note (non-blocking):** the `map-region` family (§8.3) and the `MapNoteType`/`GroupSize`/`DifficultyTier` contexts (§8.6) reference `map-windows.md` §5/§6.7/§7.2 and `travel-windows.md` §6.4, both still in Review under other reviewers; acceptance here follows the P3 precedent (revision 5 accepted while `npc-windows.md` was in Review) and the §8.3 note that the row "stands until that spec is Accepted"; if a sibling review changes those definitions, §8.3/§8.6 must be re-checked. 2026-10-11 (revision 7, "P4 social" parts): independent review against CLEAN_ROOM.md §5.3, decision 0035 D1–D4, decision 0024 §2 and LEGAL.md §11.1 rule 5: **passed**; the record is in §18.5. 2026-10-11 (revision 6, P2 parts D1–D4, commit 7b14f27): independent review against CLEAN_ROOM.md §5.3, decision 0032 D1–D4, LEGAL.md §11.1 rule 5, decision 0024 §2 and §0 item 3: **passed**; §14.9 O8, O10–O12 and §14.11 R1–R10 re-derived, invariant 11 checked; non-blocking notes (a)–(e); the record is in §18.4.1. 2026-10-11 (revision 5, P3 parts N-D1 … N-D4): independent review against CLEAN_ROOM.md §5.3 and `npc-windows.md` §5.3, §6.3, §7.8, §10: **passed**; the record is in §18.P3. 2026-10-10 (revision 4): independent review against CLEAN_ROOM.md §5.3, decision 0025 D1–D5, decision 0024 §2 and §4 and LEGAL.md §11.1 rule 5: **passed**; the record is in §18.3. Earlier: 2026-10-09 (revision 2), review against decision 0024 §2 and §4, LEGAL.md §11.1 rule 5, the CLAUDE.md content rules, ARCHITECTURE §19.1, the `tessera-net` API (`session.rs`, `model.rs`, `sandbox/mod.rs`) and every spec and decoder view cited in §8.3. 31 findings (R1–R31), all design defects fixed in this revision by the reviewer (allowed: the spec holds design facts only); no client fact was added. Every selector attribute was confirmed against an Accepted spec and, where named, its decoder view. Findings, fixes and the remaining non-blocking OPENs: §18.1. Revision 3 (spec owner, design role, 2026-10-09): 31 implementation-phase findings verified against this text and the `tessera-net` and client sources (§18.2: S1–S31): all real, 29 fixed, S12 accepted as a stated gap, the policy part of S24 recorded as an OPEN. Revision 3 independent review (spec reviewer, Clean-Room-Role: review, 2026-10-09; a session that wrote none of revisions 1–3 and no code): checked against `docs/CLEAN_ROOM.md` §5.3, decision 0024 §2 (client half only, data-free server, game rules stay T3) and §4 (values scripted, never derived), LEGAL.md §11.1 rule 5, the CLAUDE.md content rules, ARCHITECTURE §19.1 (every §9 variant listed; groups, session rules and the test-server refusals match) and the cited specs (all Accepted except `server-content.md`, Draft, from which the feed takes nothing LEGAL.md §11.1 does not state itself). Every §8.3 family and attribute was traced to its Accepted spec section and property name and to the named decoder view in `crates/formats` and `crates/wstate` (`ItemKind`'s 15 variants are the 15 keywords; `EffectRole`, `TemplateKind`, `MapKind`, `Role` variants exist as cited). R1–R31 and S1–S31: each fix is present and consistent with the rest of the text. Vectors: the SplitMix64 outputs, the seven samples and the trace of §14.3 were recomputed with a throwaway script and agree; Vectors A, B, D, E and F were re-derived line by line (times, event order, field order and formats, window and item ids, skips, drops, end times; facing 359.999 round-trips to 359.99899, written `0.00`) and agree; all 45 hostile rows agree with the grammar, the error order and the position rules; the implementation's vector tests agree (supporting evidence only). Result: **pass**. Rulings on the implementer OPENs (review notes, not facts; the spec owner writes them into the text at the next revision): §3.1 guard (1) agreed: the exception "`tessera-net` without `scenario`" cannot hold under cargo feature unification, drop it from the allow list; (2) and (3) confirmed (dev-dependencies are server builds; in-process server hosts such as `tessera-loadtest` are server builds and may link `tessera-net` only because `tessera-server` itself does not). §3.1 module naming: the `selector` module named in the placement table is placement, not an interface; `tessera-gamedata` exposing its selector types at the crate root conforms. §11.1 log size: non-blocking; a bound is recommended (for example 256 MiB with a run-time `LogOverflow`, exit 5), because a hostile local file within every limit can still write a very large log; until then the USER_GUIDE warning suffices. §12.4 (1) the unplayed windowed input script is an accepted P0 gap (windowed logs are not normative), refusing a windowed run whose `enter` needs an `accept-character` name (exit 2) is acceptable, and the proposed public input player in `tessera_net::scenario` is endorsed; (2) driver-only headless runs with an install, an exact or default spawn and no `--screenshot` are acceptable; (3) "the client's configured `avatar`" means the avatar the client actually configures for the sandbox: none in a driver-only run, which need not load the catalog for it; (4) exit 2 for a missing or non-regular `--scenario` path and 1 for an unreadable file are consistent with the exit table and should be added to it; (5) an install the user named that cannot be used exits 4 with `InstallUnreadable`: agreed (§8.7 covers only "no install found"). §13 (a) align rule 1 with `apps/client/tests/scenarios/`, where the replay test reads them; (b) keep "(or CC0-1.0)" (LEGAL.md §11.1 rule 7 and the CLAUDE.md fixture rule allow CC0) and widen the content test; a GPL-only test is stricter than the spec, not a violation. §16 `tessera-net` points: (1) `shared` in both `ItemPlace` and `ContainerSize` (the §10.3 example and the file keyword win over the general enum rule); (2)–(18) confirmed as implemented. §16 `tessera-gamedata` points (1)–(10) confirmed; for (5), keys 1–64 map to bits 0–63 (`property-system.md` §7.3), so a key above 64 matching nothing is right; for (7), `appearance.md` §2.1 (Accepted) states the two gender values, so using them with the per-install presence check is allowed. The owner-policy OPEN on numeric enum keys (§16) and the later-milestone OPENs (§8.3, §16) stay non-blocking. |
| **Implements into** | `crates/net` (`tessera_net::scenario`: parser, static checks, resolved scenario, run-time feed, headless driver, canonical log; the sandbox backend runs it; the overlay backend over the test-server backend, §12.5; plain-sandbox creation, §9.6), `crates/gamedata` (selector resolution against the user's install, §8; property labels and kinds, §5.8, §8.6), `apps/client` (`--scenario`, `--scenario-log`, `--scenario-input`, overlay runs, §12), `fuzz/` (§11.4) |
| **Companion documents** | [Decision 0024](../decisions/0024-parity-track.md) §2, §4, §5 (the feed's mandate); [LEGAL.md](../LEGAL.md) §8 and §11.1; `server-content.md` §3 and §7; `docs/ARCHITECTURE.md` §19 (domain events; it lists the variants of §9 by group); the gameplay specs cited in §8.3 (`items.md`, `combat-data.md`, `creatures.md`, `quests.md`, `economy.md`, `map-data.md`, `interaction.md`, `player-character.md`, `instances.md`), `appearance.md` §2 and §7, `coordinates.md` §4 and §7, `property-system.md` §7, `records.md` §7 |

This spec defines the **scenario feed** of decision 0024 §4: a project-defined text file that
scripts what a world source would tell the client (actors, items, vitals, casts, effects,
dialogue pages, quest states, vendor lists, loot, map markers, chat, fellowship and trade,
progression values), as timed steps and as scripted answers to client commands. The sandbox
backend of `tessera-net` plays it back as the same domain events another backend would
produce. It is a design, not a description of anything observed. No client facts were needed
or used: the research notes, the research clones and the disassembler were not consulted.

Keywords **MUST**, **MUST NOT**, **SHOULD**, **MAY** as in RFC 2119 and RFC 8174.

---

<a id="s0"></a>
## 0. Scope and legal frame

1. **Values are scripted, never derived.** Every number, outcome, duration and text a
   scenario produces comes from the scenario file. The feed contains no formula, table or
   behaviour taken from the game: no damage or healing computation, no hit chances, no loot
   rolls, no quest logic, no creature behaviour, no vendor pricing, no experience curves
   (decision 0024 §2.4; LEGAL.md §8 tier T3; `server-content.md` §7). The few automatic
   answers the feed gives (§7.6) are generic acknowledgements of Tessera's own design (a
   moved item lands where the client asked) and apply no rule from the data or from
   knowledge of the game.
2. **Records are chosen by the client.** A scenario names records only through selectors
   (§8). The client resolves them against its own install with `tessera-gamedata` before the
   session starts and hands the backend a **resolved scenario** that holds record
   identifiers (`DataId`), scripted values and literal text (§3.3). `tessera-net` decodes no
   game data and depends on no data-decoding crate.
3. **Same events as a server.** The feed's output is the domain-event vocabulary of §9,
   which a server backend would produce as well. The client half handles events without
   knowing which backend sent them. The only feed-specific interface is a diagnostic one
   (scenario status and notes through `Session::backend`, §3.4), which the headless driver
   uses and the client half never needs.
4. **The server never links the feed.** `tessera-server` MUST NOT link `tessera-gamedata`,
   any crate that decodes game data, or the scenario feed (decision 0024 §2.3; LEGAL.md
   §11.1 rule 5). The feed is the `scenario` cargo feature of `tessera-net` (§3.1); CI checks
   the server's dependency graph (§3.1).
5. **Tier.** The feed is T1/T2 tooling: offline, in process, driven by the user's own install
   or by no install at all. It is not a game server and plays no game.

Out of scope for v1: extended scripting (M9, decision 0017), running class-library code (P7),
anything on the wire of `tessera-protocol.md` (the overlay of §12.5 only uses the test-server
backend's existing `AppearanceSet`), multi-player scenarios (one player per run).

---

<a id="s1"></a>
## 1. Provenance

Legend: **Design** (decided here), **Project** (another Tessera spec, decision or crate, read
on the date given), **Standard** (a published algorithm or standard).

| Facts | Source | Mark |
|---|---|---|
| Everything not listed below: file format, grammar, step vocabulary, model, timing, triggers, defaults, selector language, attribute names, resolution contract, event and command variants, ordering rules, canonical log, limits, error kinds, client options, exit codes, content rules, vectors | Original design, 2026-10-09 (spec writer, design role). No game client, server or third-party tool was consulted | Design |
| The feed's mandate: selectors resolved by the client, values scripted, output as server events, committed scenarios synthetic and selector-only, deterministic headless tests, fuzzed parser | Decision 0024 §2, §4, §5 (Accepted 2026-10-09) | Project |
| Server rules: no game data, no gameplay reproduction, the CI check on the server's graph | LEGAL.md §8, §11.1 rules 1–5; `server-content.md` §3.1, §7; decision 0024 §2.3, P0 exit criterion 4 | Project |
| `Session`, `Backend`, `Event`, `Command`, the session states and the acceptance rules of `Session::poll_into`; `EntityId`, `DataId`, `Location`, `Transform`, `Appearance`, `Body`, `Equipped`, `ChatChannel`, `EntityKind`, `MAX_CHAT_LEN` = 1,024, `MAX_NAME_LEN` = 64, `MAX_EQUIPMENT` = 32, `MAX_LOCATION_COORDINATE` = 1e7, the heading convention and `heading_from_compass_degrees` / `compass_degrees_from_heading` | `crates/net/src/session.rs`, `crates/net/src/model.rs`, read 2026-10-09 | Project |
| Sandbox behaviour reused: world entry emits `WorldEntered`, `LocationReport` (cause `Entered`) and, with an avatar, `AppearanceChanged`, in that order, then wanderer and crowd spawns and a system welcome line (`finish_enter`), which a scenario run turns off (§3.2); the tell notice of `chat` is sent whatever `echo_chat` says; the default `SandboxConfig` spawn (region 0, origin, heading 0, exact) and resolve timeout (10 s); the walker's sandbox configuration (`apps/client/src/walker_host.rs`: a ground-snapped default spawn and a 300 s resolve timeout); ids allocated from 1; `WorldSource` handed in by the client; `SandboxConfig` fields (`characters`, `spawn`, `wanderers`, `crowd`, `echo_chat`, `latency`); `Session::backend` and `Backend::as_any` for diagnostics | `crates/net/src/sandbox/mod.rs`, `crates/net/src/sandbox/world.rs`, `apps/client/src/walker_host.rs`, `apps/client/src/runtime.rs`, read 2026-10-09; ARCHITECTURE §19.1–§19.2 | Project |
| `tessera-gamedata` layer and allowed dependencies (`formats`, `props`, `text`, `wstate`, `wvm`) | Decision 0024 §3; `crates/gamedata/src/lib.rs`, read 2026-10-09 | Project |
| Client options reused: `--headless` (needs a task of the `headless_task` group), `--no-data` (turns the walker off), `--game-dir`, `--spawn`, `--place`, `--heading`, `--screenshot`, `--settle-timeout`, `--ui-script`, `--server`; process exit 1 on failure, clap usage errors exit 2; the remote backend maps only Say, Yell, Emote and Tell chat channels to the wire | `apps/client/src/cli.rs`, `apps/client/src/main.rs`, read 2026-10-09 | Project |
| Record families and their filter attributes | The Accepted specs and decoder views cited per row in §8.3 (`crates/formats/src/gameplay`, `crates/formats/src/instance`, `crates/wstate/src/character`, read 2026-10-09) | Project |
| SplitMix64 generator: increment 0x9E3779B97F4A7C15, multipliers 0xBF58476D1CE4E5B9 and 0x94D049BB133111EB, shifts 30, 27, 31; first output for seed 0 is 0xE220A8397B1DCDAF | G. L. Steele, D. Lea, C. H. Flood, "Fast splittable pseudorandom number generators", OOPSLA 2014; S. Vigna's public-domain reference implementation. Chosen by Tessera (§8.5); outputs recomputed by the spec writer with a throwaway script, 2026-10-09 | Standard |
| Partial Fisher–Yates shuffle; unbiased bounded integers by rejection | R. Durstenfeld, CACM 7(7), 1964; the rejection threshold (2^64 − *b*) mod *b* is the textbook construction | Standard |
| UTF-8 | RFC 3629 | Standard |
| Revision 4 (§18.3): overlay runs, looks and property values, the held-item slot, creation on the test-server backend, plain-sandbox creation, `remove-character`, `stat`, `item-property`, Vector G, the new hostile rows | [Decision 0025](../decisions/0025-p1-character-look-on-test-server.md) (spec writer, design, 2026-10-10); `character-screens.md` §5.2, §7.1, §7.1.1, §7.2, §8, `character-panels.md` §3.4, §7, §9, `item-tooltips.md` §6.5, §11 (Accepted, read 2026-10-10); `tessera-protocol.md` v1.1 §5.3, §6.2, §7.4 (revised the same day); `property-system.md` §3 (type codes), MP4 (the two PID families), §7.3 (bit ↔ key); `player-character.md` §6.2 (`NationalityTable_Nationality`). No game client, server or third-party tool was consulted | Design |
| P3 revision (revision 5, §18.P3): the text function `action(…)` and `Text::Action`, the `rewards` and `buyback` steps, the events `QuestRewards` and `BuybackShown`, the command `BuyBack`, their patterns, input forms, log forms, limits, Vectors N1 and N2, the hostile rows N-H1 … N-H11 | [`npc-windows.md`](npc-windows.md) §5.3, §6.3, §7.8, §10 (spec writer, 2026-10-10; Review); `quests.md` §9 (Accepted). The windows' needs come from that dirty-room spec; the feed forms themselves are design | Design |
| Revision 7, "P4 social" (§18.5): waiting parts and one synchronised start with its time and lead, the player's `/play … sync` and `/playstart` as commands, fellow vitals, the disband command, the remote backend at minor 2, the plain sandbox's start answer, Vector H, the P4 hostile and backend cases (2026-10-11) | [Decision 0035](../decisions/0035-p4-social-on-test-server.md) (spec writer, design); `tessera-protocol.md` v1.2 §10.6 and Appendix A (revised the same day); `instruments.md` §8.8, §8.9 and §12 P4 Decision (start tolerance, waiting parts, the 0.7 s lead; revised 2026-10-11, referred to, not restated); `social-panels.md` §4.4, §8, §9 (draft, 2026-10-11). No game client, server or third-party tool was consulted for this revision | Design |
| Revision 8, "P5 travel" (§18.P5): the step options and steps `markers`, `travel-end`, `instance-queue`, `mount-vital`, the options of `marker`, `travel-list`, `travel`, `instances`, `mount` and of the input form `join-instance`, marker sets and run-time marker ids, the family `map-region` and the `effect` attributes `mount-type` and `war-steed`, the label contexts `MapNoteType`, `GroupSize`, `DifficultyTier`, the types, fields, events `TravelEnded`, `MountVitalChanged`, `InstanceQueueChanged` and command `LeaveQueue`, their log forms (with the rule that fields added to existing variants are written only when not at their default), limits, Vectors T1–T3, hostile rows T-H1 … T-H18 | What the windows and views must show: `travel-windows.md` §4.10, §4.11, §5.2, §6.5, §6.8, §9; `map-windows.md` §5.1, §5.4, §6.7, §7.2; `mounts.md` §2.3, §2.8, §4.1; `walker.md` §16.1, §16.5; `lua-plugin-api.md` §15.5 (all revised or written for P5 on 2026-10-11, Review). The four dictionary names the new label contexts and filters use (`MapNote_Type` a 64-bit bitfield, `FellowshipMakerUI_GroupSize` and `FellowshipMakerUI_DifficultyTier` enums, `Mount_Type` a 32-bit bitfield) were checked present with those kinds in the owner's install (M1, read-only, 2026-10-11). The feed forms themselves are design; no client code was consulted for this spec | Design |
| The review fixes of §18.1 (R1–R31) and §18.2 (S1–S31): design changes only | Review of 2026-10-09 (spec reviewer, review role) and the implementation-phase review of 2026-10-09 (fixed by the spec owner); the same clean sources as the first draft, read on that date. No game client, server or third-party tool was consulted | Design |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Scenario file** | A UTF-8 text file in the format of §4. |
| **Parsed scenario** | The result of parsing and the static checks of §11 (`Scenario`). Holds selectors unresolved. |
| **Resolved scenario** | The parsed scenario with every selector replaced by its resolved list (§8.6). It is what the backend receives (`ResolvedScenario`). |
| **Binding** | A `$name` introduced by a declaration: a selector, an actor, an item, a page or a marker. |
| **Element** | One resolved record of a selector binding, written `$name[i]`. |
| **Step** | One scripted action at a virtual time (§6). One step emits zero or more events. |
| **Agenda** | The feed's queue of scheduled steps, ordered by §7.4. |
| **Trigger** | A block that schedules steps when the client sends a matching command (§7.5). |
| **Input script** | A block of player commands that the headless driver sends at given times (§7.7). |
| **Tick** | The feed's fixed time step (§7.1). |
| **Virtual time** | Milliseconds since world entry, advanced in ticks. |
| **Ref** | A record reference or a literal label: `Ref::Record(DataId)` or `Ref::Literal(String)` (§5.2). |
| **No install** | The client runs without game data (`--no-data`, or no install found): every selector resolves empty (§8.7). |
| **Overlay run** | `--scenario` together with `--server`: the feed runs the Character and Inventory groups of a restricted file over the test-server backend (§12.5). |
| **Look** | A character's creation choices: 1–48 (property, value) entries carried in `Appearance` (§5.8, §9.1). |
| **Property value** | A scripted value of a data property, written `PROPERTY=VALUE` (§5.8). |
| **Synthetic scenario** | A scenario whose output does not depend on an install: run with no install, it produces its full intended log. |
| **Waiting part** | (P4 social) A fellowship member's performance that is ready and waits for a synchronised start (§6.7); at most one per member. |
| **Synchronised start** | (P4 social) One start that starts every waiting part at one common time, *S* (§6.7). |

---

<a id="s3"></a>
## 3. Architecture and data flow

<a id="s3-1"></a>
### 3.1 Placement

| Part | Crate | Notes |
|---|---|---|
| Lexer, parser, static checks, `Scenario`, `ScenarioError` | `tessera-net`, module `scenario` | Depends only on `std` and the crate's existing dependencies, so a fuzz target can drive it (§11.4). |
| `SelectorResolver` trait, `ResolvedScenario`, sampling (§8.5) | `tessera-net`, module `scenario` | The trait is implemented outside `tessera-net`. |
| Run-time feed | `tessera-net`, sandbox backend | `SandboxConfig` gains `scenario: Option<Arc<ResolvedScenario>>`. |
| Overlay backend (§12.5) | `tessera-net`, module `scenario` | Wraps the test-server (`remote`) backend and runs the feed's Character and Inventory groups; compiled only with the feature `scenario`. `tessera-server` links neither it nor `tessera-net` (§15 invariant 1). |
| Headless driver and canonical log writer | `tessera-net`, module `scenario` | Shared by the client and the net tests, so conformance runs need no window or GPU (§12.3). |
| Selector evaluation over the install | `tessera-gamedata`, module `selector` | Typed stores over the decoders; returns candidate lists (§8.6). It cannot depend on `tessera-net`, so it has its own query types mirroring §8. |
| Adapter `SelectorResolver` → `tessera-gamedata` | `apps/client` | Thin mapping between the two query types. |

The module is compiled under the cargo feature **`scenario`** of `tessera-net` (default on).
The CI check of decision 0024 §2.3 runs `cargo tree -p tessera-server -e normal,build -f
"{p} {f}"` and fails when the output names `tessera-gamedata`, any crate that decodes or reads
game data (at least `tessera-dat`, `tessera-formats`, `tessera-props`, `tessera-text`,
`tessera-wstate`, `tessera-wvm`, `tessera-material`, `tessera-mesh`, `tessera-havok`,
`tessera-terrain`, `tessera-world`, `tessera-physics`, `tessera-assets`, `tessera-audio`,
`tessera-scriptgraph`, and `tessera-install`, which reads installs, LEGAL.md §11.1 rule 1), or
`tessera-net` with the feature `scenario` enabled. Because a deny list misses crates added
later, the check SHOULD also fail on any workspace crate outside an allow list kept beside it
(today: `tessera-server`, `tessera-protocol`, and `tessera-net` without `scenario`), so a new
crate enters the server's graph only by a reviewed change to that list. Today `tessera-server`
depends on `tessera-protocol` only (not on `tessera-net`), which passes.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of
> `tools/check-server-deps.sh`, 2026-10-09) Three points where the guard is stricter than or
> goes beyond this section, pending a decision here: (1) the exception "`tessera-net` without
> `scenario`" does not survive cargo feature unification: `tessera-client` turns on the default
> features of `tessera-net` (`scenario` included), so in a workspace build (`cargo build
> --workspace`) a server that depends on `tessera-net` with `default-features = false` still
> links the feed. The guard therefore refuses `tessera-net` in any form; proposal: drop the
> exception from the allow list. (2) The guard also checks dev-dependencies of
> `tessera-server` (its examples and tests are server builds; decision 0024 §2.3 names plain
> `cargo tree -p tessera-server`, server-content.md §10 E1 says "MUST NOT depend on") and the
> features a workspace build turns on in `tessera-protocol` through other members. (3) It
> treats every other workspace crate whose normal or build graph contains `tessera-server`
> (today `tessera-loadtest`, which runs the server in-process) as a server build in the sense
> of server-content.md's scope, with the allow list `tessera-server`, `tessera-protocol`,
> `tessera-net`; a crate with `tessera-server` only as a dev-dependency (the client's tests) is
> not one. Confirm or narrow (2) and (3).

<a id="s3-2"></a>
### 3.2 Pipeline

1. **Parse.** The client reads the file (at most 1 MiB, §11.1) and calls the parser. Errors
   are typed with line and column (§11.2); the client exits 3 in headless mode (§12.2).
2. **Resolve.** The client calls `Scenario::resolve` with its `SelectorResolver` (an empty
   one when there is no install). Resolution runs once, before the session starts, in
   declaration order (§8.6). Shortfalls are notes, never errors (§8.6); only hard limits and
   an unreadable install are resolve errors (exit 4).
3. **Configure.** The client builds `SandboxConfig` with `scenario = Some(resolved)` and its
   usual `WorldSource`. With a scenario, the sandbox sets `wanderers` = 0, an empty `crowd`
   and `latency` = 0, and its character list comes from the scenario (§5.1). It sends **no
   welcome line** and no other unsolicited system notice of its own: the scenario owns the
   chat, and the log does not depend on the sandbox's wording. The sandbox's remaining notices
   answer a client `Teleport` or `QueryLocation` (which headless scenario runs never send,
   §12.2) or a ground or named spawn lookup (which a headless run without spawn options never
   makes, below). While the feed runs, the sandbox hands every `Chat` command to the feed and
   does not run its own chat path (neither the echo nor the tell notice); the feed's generic
   answer reproduces both (§7.6), so `defaults off` gives no chat answer at all. After
   `end-at` or `Horizon` (§7.3) the sandbox's own chat path runs again. The player's
   appearance is the scenario's base appearance when it has one (§5.1), otherwise the
   client's configured `avatar` (if any). Statics streaming is unchanged.

   **Spawn.** With `--spawn` or `--place` the sandbox spawns there as without a scenario.
   Without either, a **headless** scenario run (§12.2) spawns at an exact location whatever
   the install: the default `SandboxConfig` spawn (no spawn point, region 0, the origin,
   heading 0 = east, or the `--heading` given), so no `WorldSource` lookup is ever pending.
   Real-data scenarios that need terrain give an exact `--spawn R,X,Y,x,y,z`. A **windowed**
   run without either uses the walker's usual spawn (ground-snapped, so it waits for the
   loader; windowed logs are not normative, §12.4). The resolve timeout in force is the one
   of the configuration the client builds: 10 s for the plain sandbox (`--no-data` or no
   install), the walker's 300 s with an install.
4. **Run.** The session runs as usual. The feed starts its virtual clock at world entry
   (§7.1), executes the agenda, answers commands (§7.5, §7.6) and emits events.
5. **Finish.** The feed reports `Finished` through its status (§3.4). The headless driver
   writes the end line of the log, disconnects and exits 0.

<a id="s3-3"></a>
### 3.3 What crosses into `tessera-net`

A `ResolvedScenario` holds only: the step program (verbs, scripted numbers, durations,
literal strings, binding indices), the resolved bindings as lists of `ResolvedRecord`
(§8.6: a `DataId`, a table row number, optionally a `Location` and `Anchor`, optionally a
resolved name), the enum values resolved from labels outside selectors (§8.6), the property
IDs and value kinds resolved for property values and the resolved looks (§5.8; the values
themselves are scripted), the resolution notes, and the limits in force. Resolved names are install text held in memory only for the
one place where an existing event needs a string: actor names in `EntityCreated` /
`EntitySpawned` written `name(…)` (§5.4). Everything else travels as `DataId` for the client
half to resolve. Nothing in `tessera-net` opens an archive or interprets a DID.

**Order of events inside the sandbox.** In `poll`, the sandbox (1) advances its own pending
resolution and statics streaming as without a scenario, (2) lets the feed process its due
ticks (§7.1), appending the feed's output (at most 4,096 events per tick, §7.4) to the
sandbox's event queue after anything already queued, (3) delivers the queue in order. Events
the sandbox itself produces in `submit` (answers to `Move`, `Teleport`, `QueryLocation`,
`Connect`, `EnterWorld`, `Disconnect`) are queued at once, as today. Events of a feed step,
including the `LocationReport` and `EntityMoved` of the `teleport` step (§6.5), go through the
feed's output queue in execution order.

**Before world entry** there are no ticks (§7.1). The feed handles `Connect`,
`CreateCharacter` and `DeleteCharacter` synchronously in `submit`: it appends its output (the
`CharacterInfo` lines of §5.1 after `Connected`; the body of a fired character trigger, §7.5)
to the sandbox's event queue right after the sandbox's own answer to that command (if any),
outside the per-tick cap. The client receives it in its next poll, and the log writes it as
`pre` (§10.2). Its size is bounded by §7.5 (at most 64 pre-world firings of at most 16 lines
each) and counts toward the run limits of §11.1.

<a id="s3-4"></a>
### 3.4 Diagnostics interface

`SandboxBackend` offers, beyond the `Backend` trait:

| Method | Returns |
|---|---|
| `scenario_status()` | `None` without a scenario; otherwise `Running { time_ms }`, `Finished { time_ms, reason }` (`Complete` or `Horizon`) or `Failed { time_ms, error }` (a `ScenarioRuntimeError`, §11.3). |
| `scenario_counters()` | `skipped`, `dropped`, `spilled` (§7.4, §10.4). |

The headless driver reads them with `Session::backend::<SandboxBackend>()`. The client half
does not use them.

---

<a id="s4"></a>
## 4. The file format

<a id="s4-1"></a>
### 4.1 File, encoding, lines

- **Extension.** `.scenario` (for example `combat-basics.scenario`). The header line, not the
  extension, identifies the format; the client accepts any file name.
- **Encoding.** UTF-8 without a byte-order mark. A leading U+FEFF, an invalid UTF-8 sequence,
  or an overlong or surrogate encoding is an `Encoding` error at that character.
- **Lines.** Lines end with LF; a CR immediately before LF is removed. Any other CR, and any
  control character (U+0000–U+001F, U+007F–U+009F) other than TAB, is a `ControlCharacter`
  error at that character, inside or outside strings. A final line without LF is accepted.
- **Line length.** At most 4,096 bytes, not counting the terminator (§11.1).
- **Whitespace.** SPACE and TAB separate tokens. Leading whitespace is insignificant;
  indenting block bodies is recommended and has no meaning.
- **Comments.** `#` outside a string starts a comment that runs to the end of the line. It
  also ends the token it touches.
- **Blank lines** (only whitespace and comments) are ignored everywhere.
- **Case.** Keywords and attribute names are lower case and compared exactly. Labels
  (`@Label`) are compared exactly against the install's labels.

<a id="s4-2"></a>
### 4.2 Tokens

A **token** is a maximal run of non-whitespace characters, with one exception: a `"` begins a
string that extends to the matching unescaped `"` and belongs to the token it appears in, so
`name="Old guide"` is one token. Tokens of the form `key=value` carry options; there is no
whitespace around their `=`, and the key is the text before the **first** `=` (so
`vital=resource="Spark"` has the key `vital`). The `=` of a `select` declaration is a token
of its own.

A string may stand only where a form of this spec places one: as a whole token, as the value
of a `key=` option, as an item of a comma-separated list (§6.3), before `:` in the forms
`STRING:on`, `STRING:off` and `REF:N`, or after `named:` or `tell:`. Anything else (`"a"b`, `a"b"`) is
`UnexpectedToken` at the token's first character.

| Lexical class | Form | Notes |
|---|---|---|
| Keyword | a lower-case ASCII letter, then lower-case letters, digits and `-` | At most 32 bytes. Verbs, settings, family and attribute names, enumerated words (`yes`, `no`, `none`, `all`, `player`, `on`, `off`, ...). |
| Binding | `$`, a lower-case ASCII letter, then lower-case letters, digits and `_` | At most 32 bytes including `$`. One namespace for all bindings. |
| Element | binding `[` decimal integer `]` | Index into a selector binding, 0-based, at most 65,535. |
| Label | `@`, then an ASCII letter or `_`, then letters, digits and `_` | At most 128 bytes including `@`. An enum label of the install (§8.2). |
| Integer | optional `-`, then 1–20 decimal digits; or `0x` with 1–16 hexadecimal digits | No `+`, no separators, no leading zeros except `0` itself. Hexadecimal only in the `did` family (§8.2), in `seed=`, in the property number of `pid:` and in property values (§5.8). The value is read exactly (it fits a signed 128-bit integer) and then checked against the range of its position: `OutOfRange`, or the `LimitExceeded` kind that §11.1 names for that limit (`n=`, `repeat`). Enum values (class, species, sex, slot, damage type, filter values) are 0 … 2^32 − 1. |
| Decimal | optional `-`, digits, optionally `.` and 1–9 digits | No exponent. Converted to the nearest `f64` (ties to even), then range-checked (§11.1). A value an event carries as `f32` is that `f64` converted to the nearest `f32` (ties to even, Rust `as f32`). |
| Duration | decimal immediately followed by a unit `ms`, `s` or `min` | Converted exactly on the decimal digits (not through a float) to whole milliseconds; a value that is not a whole number of milliseconds (`0.0005s`) is a `BadDuration` error. Negative durations are errors. In a position that requires a duration, a token that is not one (`2h`) is `BadDuration`; likewise `BadNumber`, `BadPosition` and `BadFraction` for the other numeric forms. |
| Fraction | decimal `/` decimal | `current/max` values; both within the step's range. |
| Position | decimal `,` decimal `,` decimal | Metres east, north, up, relative to the player's spawn (§5.4). |
| String | `"` … `"` | Escapes: `\"`, `\\`, and `\u{H}` with 1–6 hexadecimal digits naming a Unicode scalar value that is not a control character or surrogate. Any other `\` sequence is `BadEscape`. At most 1,024 bytes after unescaping; contexts that need a name allow 64 (§11.1). May be empty only where a step says so. |
| Text function | `name(`element`)`, `desc(`element`)`, `comment(`element`)`, `role(`element`,`integer`)`, `role-failure(`element`,`integer`)`, `objective(`element`,`integer`)`, `place(`element`)` | One token, no spaces; the integer is 0 … 65,535. Meaning in §5.2 and §5.4 (`place(…)` is a position, not a text). |
| Timing | `at` followed by a duration token, or a single token `+`duration | §7.2. |
| Property value | `@Label=VALUE`, or `pid:`INTEGER`:`KIND`=`VALUE | One token (a string inside it may hold spaces and `=`); the key is the text before the first `=`. `KIND` is one of the keywords `bool`, `int`, `int64`, `float`, `enum`, `bitfield32`, `bitfield64`, `did`, `string`, `instance`, `colour`; another word is `UnexpectedToken`. Meaning and value forms: §5.8. |
| Colour | `rgba:`R`,`G`,`B`,`A` | Four decimal integers 0–255 (`OutOfRange` otherwise), red, green, blue, alpha. Only as a property value. |

**Text function `action` (P3 revision).** The class Text function also holds `action(`LABEL`)`
and `action(`INTEGER`)` (one token; the integer 0 … 2^32 − 1). It stands wherever a text
stands and names an NPC action (§5.2). A string may also stand as the REF of the forms
`rep:REF:N` and `wallet:REF:N` of the `rewards` step (§6.3).

**Numeric components.** Each numeric component (an integer, decimal or duration; one
coordinate of a Position; one side of a Fraction; one item of a numeric or hexadecimal list;
the number of `REF:N`) is at most 32 characters, sign and unit included; a longer one is
`BadNumber` at the token (§11.1), whatever form the position expects. The limit applies per
component, not per token, so a legal Position or a 256-item `did` list is never too long. The
length check comes before conversion and range checks. (With at most 31 digits, a duration
in minutes converts exactly in a signed 128-bit integer.)

<a id="s4-3"></a>
### 4.3 Header and settings

The first line that is not blank MUST consist of exactly the two tokens `tessera-scenario` and
`1` (a comment may follow). A missing header is `BadHeader`; another version number is
`UnsupportedVersion`. A file with a newer major version is refused, never guessed at.

Settings appear at top level, at most once each, before the first step line or block opener
(`repeat`, `on`, `input`); declarations may come before or between them:

| Setting | Default | Meaning |
|---|---|---|
| `title STRING` | none | A title for logs and the window caption. At most 128 bytes. |
| `tick DURATION` | `50ms` | The tick length, 10 ms to 1,000 ms (§7.1). |
| `end-at DURATION` | none | The virtual time at which the scenario ends (§7.3). At most 60 min. |
| `enter STRING` | the `player` character; without one the first `character`; without either the first name of the `Connected` list | The character the headless driver enters with (§5.1). It MUST name a declared `player` or `character`, or a name that an `accept-character` step of the file names; otherwise `UnknownCharacter` (detected at end of file, reported at the `enter` line). |
| `defaults on` / `defaults off` | `on` | Whether the generic answers of §7.6 run. |
| `requires install` | not required | With no install, resolution fails with `InstallRequired` (exit 4) instead of resolving empty. For real-data scenarios that are meaningless without data. |

<a id="s4-4"></a>
### 4.4 Statements and blocks

| Statement | Where | Defined in |
|---|---|---|
| Header, settings | top level | §4.3 |
| Declarations: `player`, `character`, `select`, `actor`, `item`, `page`, `option`, `look` | top level | §5 |
| Step line: timing, verb, arguments | top level, `on` and `repeat` bodies | §6, §7.2 |
| `repeat` block opener: timing `repeat` *N* `every` DURATION | top level and `on` bodies | §7.3 |
| `on` block opener: `on` pattern [`times=`*N*] | top level | §7.5 |
| `input` block opener | top level, at most once | §7.7 |
| Input line: `at` (DURATION or `pre`) command | `input` body only | §7.7 |
| `end` | closes the innermost open block | |

Blocks nest at most two deep (`repeat` inside `on`). A block opener at depth 3 or more is
`NestingTooDeep`; any other opener in a place this table does not allow is `NotAllowedHere`:
a `repeat` inside a top-level `repeat`, an `on` inside a block, an `input` inside a block or a
second `input` block, a `repeat` inside an `on create-character` or `on delete-character`
body (§7.5), and any block inside `input`. Both errors are reported at the opener's keyword
(`repeat`, `on`, `input`); the depth check comes first. A block left open at end of file is `UnclosedBlock` at its opening
line; `end` without an open block is `UnexpectedEnd`. Declarations may appear anywhere at top
level, but a binding must be declared on an earlier line than any use (§11.2).

<a id="s4-5"></a>
### 4.5 Grammar

Notation: `"x"` a literal keyword or symbol; `A B` sequence; `[A]` optional; `{A}` zero or
more; `A | B` choice; upper-case names are lexical classes of §4.2.

**Options.** Within a statement, options (`key=value` tokens, required ones such as `kind=`
and `pos=` included) may stand anywhere after the statement's first keyword (after the verb,
for a step line), in any order, each at most once (a repeat is `UnexpectedToken`). Positional
arguments keep their relative order; options do not count as positions. Three key=value forms
are positional, not options, and keep their place: `resource=REF` as the vital argument of
`vital`, `quickslot=N` as the skill argument of the input form `use-skill`, and `named:NAME`
(no `=`). Property values (`@Label=VALUE`, `pid:…=VALUE`, §5.8) are positional too: their key
is never a keyword, so they cannot be taken for options. In a `select` line, the tokens after the family (filters, the count, `seed=`) may
come in any order; `seed=` together with `all` is `UnexpectedToken`.

| Production | Definition |
|---|---|
| file | header-line { line } |
| header-line | `"tessera-scenario"` `"1"` |
| line | blank-line \| statement |
| statement | setting \| declaration \| step-line \| block-open \| input-line \| `"end"` |
| setting | `"title"` STRING \| `"tick"` DURATION \| `"end-at"` DURATION \| `"enter"` STRING \| `"defaults"` (`"on"` \| `"off"`) \| `"requires"` `"install"` |
| declaration | player-decl \| character-decl \| select-decl \| actor-decl \| item-decl \| page-decl \| option-decl \| look-decl |
| player-decl | `"player"` STRING { char-opt } |
| character-decl | `"character"` STRING { char-opt } |
| char-opt | `class=`value \| `level=`INTEGER \| `species=`value \| `sex=`sexvalue \| `look=`BINDING (a look, §5.8) |
| look-decl | `"look"` BINDING propval { propval } (1–48 property values, §5.8) |
| propval | property `=` pvalue (one token) |
| property | LABEL \| `pid:`INTEGER`:`pkind |
| pkind | `"bool"` \| `"int"` \| `"int64"` \| `"float"` \| `"enum"` \| `"bitfield32"` \| `"bitfield64"` \| `"did"` \| `"string"` \| `"instance"` \| `"colour"` |
| pvalue | `"yes"` \| `"no"` \| INTEGER \| DECIMAL \| STRING \| ELEMENT \| LABEL { `"\|"` LABEL } \| COLOUR \| `"none"` |
| sexvalue | INTEGER \| `"m"` \| `"f"` (§5.1) |
| select-decl | `"select"` BINDING `"="` family { filter } count [ `seed=`INTEGER ] |
| family | one keyword of §8.2, or `"did"` hex-list |
| hex-list | INTEGER { `","` INTEGER } (one token, hexadecimal) |
| filter | attribute`=`fvalue (attribute names per family, §8.3) |
| fvalue | alt { `"\|"` alt } \| range \| BINDING \| `"yes"` \| `"no"` |
| alt | INTEGER \| LABEL \| KEYWORD |
| range | [INTEGER] `".."` [INTEGER] (at least one bound) |
| count | `"all"` \| `n=`INTEGER |
| actor-decl | `"actor"` BINDING `kind=`akind `pos=`position [ `name=`aname ] [ `body=`(ELEMENT \| `"none"`) ] [ `facing=`(DECIMAL \| `"player"`) ] [ `level=`INTEGER ] [ `scale=`DECIMAL ] [ `species=`value `sex=`sexvalue ] (`species=` and `sex=` together or not at all; `body=` excludes them) |
| akind | `"npc"` \| `"creature"` \| `"object"` \| `"player"` |
| aname | STRING \| `name(`ELEMENT`)` |
| item-decl | `"item"` BINDING ref [ `qty=`INTEGER ] |
| page-decl | `"page"` BINDING [ `speaker=`entity ] text |
| option-decl | `"option"` BINDING INTEGER text [ `goto=`(BINDING \| `"close"`) ] |
| step-line | timing step |
| timing | `"at"` DURATION \| `+`DURATION |
| step | a verb of §6 and its arguments |
| block-open | timing `"repeat"` INTEGER `"every"` DURATION \| `"on"` pattern [ `times=`INTEGER ] \| `"input"` |
| pattern | a pattern of §7.5 |
| input-line | `"at"` (DURATION \| `"pre"`) command (a command form of §7.7) |
| entity | `"player"` \| BINDING (an actor) |
| ref | ELEMENT \| STRING |
| reflist | ref { `","` ref } (one token, 1 … 1,024 items; §6.3) |
| text | STRING \| text function (§4.2) |
| value | INTEGER \| LABEL |
| position | POSITION \| `place(`ELEMENT`)` |

A token that fits no production at its place is `UnexpectedToken`; an unknown first keyword of
a statement is `UnknownKeyword`; a missing required argument or option is `MissingArgument`
at the statement's first token.

**Error order.** The parser and the static checks make one forward pass over the file and
report the first error they detect, with its position (§11.2). Two checks can only be made at
end of file and run then, in this order: `UnclosedBlock` (reported at the block's opening
line) and `UnknownCharacter` (§4.3, reported at the `enter` line); so any error detected
inside the file wins over them. `FileTooLarge` is checked before anything else.

---

<a id="s5"></a>
## 5. Model

<a id="s5-1"></a>
### 5.1 Characters and the player

- `player STRING …` declares the character the player plays (at most once; a second
  `player` is `DuplicateBinding` at its first token); `character STRING …` declares further entries of the
  character list. Names are 1–64 bytes, pass `is_valid_name` and are unique (a repeated name
  is `DuplicateBinding` at its string). The list, in declaration order with the `player`
  first, holds at most 64 names (`LimitExceeded(characters)`); `accept-character` (§6.1) adds
  names at run time up to the same 64 (beyond that the step is skipped). Without any
  declaration, the list is the one the client configures for the sandbox (its default).
- `class=`, `level=` (1 … 10,000), `species=`, `sex=` are scripted details: `class` is a class
  enum value (`lua-plugin-api.md` §15 `Class`, mapper of `AdvTable_Class`, `combat-data.md`
  §9.1), `species` and `sex` the body values of `appearance.md` §2. A `class` or `species`
  label resolves through the mapper the property dictionary names for `AdvTable_Class` or
  `RaceTable_Race` (`appearance.md` §2.2); `sex=m` and `sex=f` resolve to the male and female
  gender values of the race table's gender entries (`RaceTable_GenderType`, `appearance.md`
  §2.1). The resolver does this (`enum_value`, §8.6); `tessera-net` holds no such values. With
  no install a label or `m`/`f` stays unresolved and the detail is absent. Numbers pass
  through. `look=$l` names a look (§5.8); its resolved entries, when there is at least one,
  are a detail too (`CharacterInfo.look`).
- After `Connected`, the feed sends one `CharacterInfo` (§9.2) per declared character with at
  least one detail that has a value (a number, or a label or `m`/`f` that resolved), in
  declaration order; a character whose details are all unresolved gets none. These lines are
  pre-world output (§3.3) and are logged as `pre`.
- On world entry with the player character: when species and sex are both known, the feed
  sets the player's base appearance to `Appearance::new(Body::Character { species, sex })`
  with the character's resolved look (§5.8; none when it has no look or none resolved), sent
  by the sandbox as its usual `AppearanceChanged`. Equipment added later updates it
  (§6.1 `equip`). The character entered may also be one an `accept-character` step added
  (§6.1); its details and look are those of that step.
- The player's entity is the sandbox's own player id; logs write it as `player`.

<a id="s5-2"></a>
### 5.2 References and texts

**Ref.** Every place where an event names a record accepts either an element (`$skills[0]`,
giving `Ref::Record(DataId)`) or a literal string (giving `Ref::Literal(String)`, at most 64
bytes). Literal refs let synthetic scenarios show casts, effects, items and quests with no
install; the client half shows a literal ref as its text with a placeholder icon.

**Text.** Window texts are `Text` values (§9.1):

| Written | Text value | The client half shows |
|---|---|---|
| `"…"` | `Literal(String)` | the string |
| `name($x[i])` | `Name(DataId)` | the record's display name (for example `Name`, `Skill_Name`, `Effect_Name`, `Quest_Name`, by family) |
| `desc($x[i])` | `Description(DataId)` | the record's description text |
| `role($q[i],N)` | `QuestRole { quest, role: N, failure: false }` | the success line of the *N*-th role (0-based, stored order) of the quest's `Quest_RoleArray`, with the variant rule of `quests.md` §7.1 |
| `role-failure($q[i],N)` | `QuestRole { …, failure: true }` | the failure line of that role |
| `comment($c[i])` | `Comment(DataId)` | the content of an NPC comment record (`quests.md` §7.7) |
| `objective($q[i],N)` | `Objective { quest, objective: N }` | the description of the objective whose `Quest_ObjectiveIndex` is *N* (`quests.md` §5.1: the indices of a quest are 1 … *n*, not always in stored order); no such objective → a placeholder |
| `action(@Label)`, `action(N)` (P3 revision) | `Action(u32)`: the label's key in the mapper the property dictionary names for `ActionPickerUI_Action` (`enum_value` context `NpcAction`, §8.6), or *N* as written | the install's text for that NPC action (`npc-windows.md` §5.3 rule 3). An unresolved label (no install, unknown label) gives `Literal` of the label without its `@` instead, with a `label` note (attribute `action`) when an install is present |

Chat lines and character, actor and fellow names are plain strings (the existing events take
`String`); text functions are not allowed there, except `name(…)` in actor declarations (§5.4).

<a id="s5-3"></a>
### 5.3 Identifiers

All identifiers are allocated deterministically:

| Kind | Type | Allocation |
|---|---|---|
| Actor | `EntityId` | `2^48 +` *k*, *k* = 1, 2, … in declaration order. Disjoint from the sandbox's counter, which starts at 1. |
| Item | `ItemId(u64)` | Declared items 1, 2, … in declaration order; items created at run time (fan-out `give`, loot, split, craft results) continue from (declared count + 1) in execution order. |
| Effect instance | `EffectId(u32)` | 1, 2, … in execution order of `effect` steps. |
| Window | `WindowId(u32)` | 1, 2, … each time a window kind opens while closed (§5.6). |
| Marker | `MarkerId(u32)` | 1, 2, … in file order of the marker bindings (a binding is declared by the first `marker` line that names it; later `marker` steps with the same binding reuse its id and replace the marker). |

Every counter uses checked arithmetic; a counter that would pass its type's maximum stops the
feed with the run-time error `IdOverflow` (§11.3). Run-time items are also capped by §11.1.

**Marker sets and run-time marker ids (P5 travel).** A marker binding first named by a
`markers` step (§6.5) is a **marker set**; one first named by `marker` is a **single marker**.
Both count toward the marker-binding limit of §11.1. The table's marker ids go to the single
markers only: 1 … *M* in file order of their first `marker` line, so a file without sets
numbers its markers exactly as before. Markers that `markers` steps create take **run-time
marker ids** *M* + 1, *M* + 2, … in execution order, never reused; at most 65,536 per run
(§11.1). A set holds the ids its last `markers` step created that no later `marker-remove`
removed. A set named by `marker`, or a single marker named by `markers`, is
`WrongBindingKind` at the binding; `marker-remove` takes both kinds.

<a id="s5-4"></a>
### 5.4 Actors

`actor $a kind=… pos=… [name=…] [body=…] [facing=…] [level=…] [scale=…] [species=… sex=…]`:

| Field | Meaning |
|---|---|
| `kind` | `npc` → `EntityKind::NonPlayer`, `creature` → `Creature`, `object` → `Object`, `player` → `Player` (another player character). |
| `pos=E,N,U` | Offset in metres east, north, up from the player's spawn location (the `Location` of the entry `LocationReport`), added in `f64`, in that location's region (the **entry region**). Each component within ±100,000 m; a sum outside `MAX_LOCATION_COORDINATE` makes the actor unplaceable. `pos=place($p[i])` uses the element's own `Location` (and region) instead (§8.6); a missing element, or an element without a `Location`, makes the actor unplaceable. |
| `name` | A string (1–64 bytes), or `name($x[i])`, resolved to text at resolution time (§3.3). Missing or unresolved: the empty string. |
| `body` | An element whose record is the figure (a creature or NPC template, `creatures.md` §4.2, `appearance.md` §7): `Body::Object(did)`. `species=`/`sex=` give `Body::Character`. With a body, `spawn` emits `EntityCreated` with `class = Some(did)` for `Body::Object` (`None` for `Body::Character`), scale `scale` (default 1, range (0, 100]), no equipment, idle `None`, tag `None`. Without a body (none given, `none`, or an empty element): `EntitySpawned`. An empty body element is not a skip. |
| `facing` | Compass degrees (0 = north, clockwise), converted with `heading_from_compass_degrees`; or `player` (the default): the heading from the actor toward the spawn location, `atan2(−n, −e)` in `f64` with any `−0` replaced by `+0`, then to `f32`; an actor exactly at the spawn takes the spawn heading. |
| `level` | Scripted level, sent as `LevelChanged` right after the spawn event. |

Actors are inactive until `spawn`; `despawn` removes them; they can be spawned again. An
actor's vitals, effects, cast and target persist across despawn (the feed keeps its own
record; events about a despawned actor are still emitted, as a server may report a fellow out
of view). The feed keeps each actor's **current position and heading**: the declared ones
until a `move` or `face` changes them; `spawn` uses the current ones.

**Regions.** Entity events carry positions in the player's current region. A `spawn` or
`face` of an actor that has no valid current position (unplaceable and never moved), or whose
current position lies in another region than the player's current one (after a `teleport` to
another region, §6.5), is a skip (counted, §10.4); so is a `move` whose target is missing,
invalid or in another region. A valid `move` gives an unplaceable actor a position.

<a id="s5-5"></a>
### 5.5 Containers, items, wallet

- **Containers**: `backpack`, `equipment`, `vault`, `shared`. `bag N [container=…]` sets a
  container's size in slots, *N* 0 … 65,535 (default size: every slot 0 … 65,535 usable).
  Shrinking a container moves nothing; items at slots ≥ *N* stay where they are. Every slot
  number (`slot=N`, equipment slot keys) is 0 … 65,535 (`OutOfRange`). Equipment slots are
  the data's slot keys (`items.md` §5.4: the key of a bit of `Inventory_DefaultSlot` /
  `Inventory_CompatibleSlot`; bit ↔ key per `property-system.md` §7.3), opaque to the feed.
- **Items**: `item $i REF [qty=N]` declares an instance (quantity default 1, 1 … 10^9). It
  exists in no container until `give`. The feed tracks each item's place, quantity and
  durability exactly as scripted.
- **First free slot**: the lowest slot index of the container not holding an item and below
  its size; none → the step is skipped (counted, §10.4). Implementations SHOULD find it in
  O(log *n*) (for example an ordered set of free slots per container, or a bitset with a
  lowest-free cursor), not by a linear scan: a fan-out `give` of 65,536 elements would
  otherwise cost about 2^31 slot checks in one tick.
- **Placing an item**: a slot is usable when it is below the container's size and holds no
  item. `give $item slot=N` (or `unequip … slot=N`) on a slot that is not usable is skipped
  (counted). A `give` of a declared item that is already in a container is skipped (counted);
  an item removed by `remove` or `DestroyItem` is in no container and may be given again,
  with the same id.
- **Wallet**: `Currency::Coin` (amounts in units of the smallest denomination, `economy.md`
  §7) and `Currency::Item(Ref)` (barter tokens, which are items, `economy.md` §6.1). Amounts
  0 … 10^15, set (never added) by steps.
- **Player appearance**: the base appearance (§5.1, or the client's `avatar`, §3.2) plus, in
  ascending equipment slot order, one `Equipped::new(did)` per item in the equipment
  container whose ref is a record, at most 32, each with `slot` = its equipment slot when
  that slot is 1 … 64 (a data slot key, `property-system.md` §7.3) and no slot otherwise
  (§9.1; decision 0025 D3). The client half places a held item by its slot when one is
  given (`character-panels.md` §7). Every change of that list emits
  `AppearanceChanged` for the player (in an overlay run §12.5.4 decides instead). Without a base appearance (neither species and sex nor
  an avatar) no `AppearanceChanged` is emitted, since an `Appearance` needs a `Body`; the
  equipment list is still tracked, and the first base appearance would include it.

<a id="s5-6"></a>
### 5.6 Windows

Window kinds: `dialogue`, `quest-offer`, `vendor`, `loot`, `trainer`, `travel`, `instances`,
`trade`. At most one window of each kind is open. A step that opens a kind already open
replaces its content and keeps its `WindowId`; otherwise a new id is allocated (§5.3).
`close KIND` and the generic answer to `CloseWindow` emit `WindowClosed`.

| Kind | Opened by | Content |
|---|---|---|
| dialogue | `show $page` | page text, speaker, options (declared with `page` and `option`, §6.3) |
| quest-offer | `offer REF` | the offered quest and giver |
| vendor | `vendor items=$sel …` | entries with scripted prices and stock |
| loot | `loot items=$sel …` | entries with scripted quantities and coin |
| trainer | `trainer skills=$sel …` | entries with scripted costs and levels |
| travel | `travel-list dest=$sel …` | destinations with scripted costs |
| instances | `instances $sel` | instance entries |
| trade | `trade ENTITY` | both sides' offers |

<a id="s5-7"></a>
### 5.7 Per-entity state the feed keeps

For the player and each actor: active flag, last scripted maximum per vital (for `left=`,
§6.2), active cast (ref, kind, scheduled end), active effect instances (id, ref, scheduled
expiry), target, combat flag. For the player in addition: quickslots (slot → content),
learned skills and recipes (for logs only), quest states, open windows. The feed never
computes a value from this state; it uses it only to address events (which instance to end,
which max to repeat). (P4 social, §6.7) The feed also keeps the **waiting parts**: member name
→ tune, at most 64 entries, set by `perform … sync` and `ReadyPerformance`, removed by
`perform-withdraw` and emptied by every synchronised start. Each piece of state follows the events the feed itself emits, from steps
and generic answers alike (for example a quickslot's content is the one of the last
`QuickslotChanged` the feed emitted for that slot); a command the feed does not answer changes
nothing.

<a id="s5-8"></a>
### 5.8 Property values and looks (revision 4, decision 0025 D2, D5)

**Property values.** A token `PROPERTY=VALUE` names one data property and a scripted value
for it. It appears in `look` declarations and in the `stat` and `item-property` steps (§6.1).

`PROPERTY` is one of:

- `@Label`: a name of the property dictionary (`property-system.md` §2.1). The resolver
  returns the property's ID and value kind (`property`, §8.6). With no install, or for a name
  the dictionary does not have, it stays unresolved (a `label` note with attribute
  `property`, with an install only).
- `pid:N:KIND`: the property ID *N* (1 … 4,294,967,295; 0 is `OutOfRange`) with its kind
  written out. With an install the resolver checks it (`property_by_id`, §8.6): an ID the
  dictionary does not have gives an `unknown` note, another kind a `mismatch` note, and the
  value is dropped. With no install, *N* and `KIND` are taken as written.

**Kinds** (Tessera's `PropertyKind`; the resolver maps the dictionary's type code of
`property-system.md` §3 to it):

| Kind | Type code | Value forms | Range and rule |
|---|---|---|---|
| `bool` | 21 | `yes`, `no` | — |
| `int` | 15 | INTEGER | −2^31 … 2^31 − 1 |
| `int64` | 19 | INTEGER | −2^63 … 2^63 − 1 |
| `float` | 9 | INTEGER or DECIMAL | Finite, \|v\| ≤ 10^9; converted as §4.2 (nearest `f64`, then nearest `f32`) |
| `enum` | 8 | INTEGER, or one LABEL | 0 … 2^32 − 1; a label resolves through the mapper the dictionary names for that property (`enum_value` with context `PropertyKey`, §8.6) |
| `bitfield32` | 18 | INTEGER (the mask), or LABEL { `\|` LABEL } | Mask 0 … 2^32 − 1; each label's key *k* must be 1 … 32 and sets bit *k* − 1 (`property-system.md` §7.3) |
| `bitfield64` | 14 | as `bitfield32` | Mask 0 … 2^64 − 1; keys 1 … 64 |
| `did` | 20 | ELEMENT, or a hexadecimal INTEGER | 0 … 2^32 − 1; an element gives its record's `DataId` (a missing element drops the value) |
| `string` | 1 | STRING | At most 256 bytes; in a look 0 … 128 bytes of U+0021–U+007E only |
| `instance` | 7 | INTEGER | 0 … 2^64 − 1; not allowed in a look |
| `colour` | 16 | COLOUR (`rgba:R,G,B,A`, §4.2) | Each component 0–255 |
| (unsupported) | every other code | — | The value is dropped with an `unsupported` note |

`none` as the value clears the property (`stat`, `item-property`); it is not allowed in a look.

**Checks.** With `pid:N:KIND` the kind is known when parsing: a value form that does not fit
the kind, and in a `look` the value `none`, the kind `instance` or a string outside the look
rule, is `BadValue` at the token; a number outside its kind's range is `OutOfRange` at the
token. With `@Label` the kind is known only after resolution: a value that does not fit it
(form or range) is dropped with a `mismatch` note. Labels inside a value resolve like other
labels (§8.6 item 6; unresolved: the value is dropped, with a `label` note of attribute
`value` when an install is present). **Dropped** means: a `stat` or `item-property` step is
a skip when it runs (counted, §10.4); a look entry is left out of its look.

**Looks.** `look $l PROPVAL { PROPVAL }` declares a look of 1–48 property values (a 49th is
`LimitExceeded(look)` at that token; at most 64 look bindings, `LimitExceeded(looks)`). A
look is named by `look=$l` on `player`, `character` and `accept-character` (§6.1) and on the
input form `create-character` (§7.7). Resolution makes each look canonical: the entries
whose property and value resolved, sorted by ascending property ID; of two entries with the
same property ID the later one in the line is kept and the earlier one dropped with a
`duplicate` note; when the encoded size (the `Look` structure of `tessera-protocol.md` §5.3:
one count byte, then per entry a 4-byte ID and the tagged value) exceeds 768 bytes, entries
are removed from the highest property ID down until it fits, with one `look` note giving how
many were cut. A look with no entry left is absent.

The feed derives no value: the dictionary tells the client only which kind a labelled
property has, and every value is the one written in the file (decision 0024 §4).

---

<a id="s6"></a>
## 6. Step vocabulary (v1)

Every step line is `timing verb arguments`. Positional arguments keep their relative order;
options may stand anywhere after the verb (§4.5).
"Fan-out" marks a position that accepts a whole selector binding (`$sel` without index):
the step then emits one event per element, in binding order (§8.4), at most 65,536 per step
(§11.1). An element that does not exist (index beyond the resolved list) makes the whole step
a **skip**: nothing is emitted, the skip is counted (§10.4) and logged at debug level; this
holds for an element anywhere in the step, list items of §6.3 included.

**Empty bindings.** A fan-out over a binding that resolved empty emits nothing and is not a
skip (the resolve note already records the shortfall). A window step whose entries come from
an empty binding (`vendor`, `loot`, `trainer`, `travel-list`, `instances`, `trade-offer
items=`) still opens or updates its window, with no entries. A reference filter (§8.2) naming
an empty binding matches nothing.

`ENTITY` is `player` or an actor binding; `REF`, `TEXT`, `POSITION` as in §4.5. Where a step
below gives no range: tiers and ranks 0 … 1,000; stock 0 … 10^9; item-cost and entry
quantities 1 … 10^9; trait points 0 … 10^6 each; trainer levels 0 … 10,000; profession
proficiency and mastery 0 … 10^9 with the current value ≤ the maximum (`OutOfRange`
otherwise).

<a id="s6-1"></a>
### 6.1 Characters and inventory

| Step | Arguments and options | Emits |
|---|---|---|
| `spawn` | `$actor` | `EntityCreated` or `EntitySpawned` (§5.4), then `LevelChanged` if the actor has a level |
| `despawn` | `$actor` | `EntityDespawned` |
| `move` | `$actor POSITION [over=DURATION]` | Without `over` (or `over=0ms`): one `EntityMoved` at the target with velocity 0. With `over`: let *J* = ⌈over / tick⌉, *from* the actor's current position when the step runs; on each of the next *J* ticks *j* = 1 … *J*, `EntityMoved` at *from* + ((to − *from*) × *j*) / *J* (componentwise in `f64`, then `f32`), velocity (to − *from*) × 1,000 / over_ms in m/s (in `f64`, then `f32`), except velocity 0 at *j* = *J*. Each emission updates the current position and uses the actor's current heading. The intermediate positions are **one** agenda entry (class 0) that re-arms itself for the next tick with a new sequence number until *j* = *J*; a later `move` of the same actor cancels it. |
| `face` | `$actor DEGREES` or `$actor player` | `EntityMoved` at the current position with the new heading, velocity 0 |
| `level` | `ENTITY N` (1 … 10,000) | `LevelChanged` |
| `bag` | `N [container=backpack\|vault\|shared]` | `ContainerSize` |
| `give` | `$item [to=CONTAINER] [slot=N]` | `ItemAdded` at the slot or the first free slot of `to` (default `backpack`); a slot that is not usable, or an item already in a container, → skip (§5.5) |
| `give` | fan-out `$sel [to=…] [qty=N]` | per element: a new item (id §5.3) and `ItemAdded` at the first free slot |
| `remove` | `$item` | `ItemRemoved` |
| `quantity` | `$item N` | `ItemQuantity` |
| `equip` | `$item slot=VALUE` | as the generic `EquipItem` answer (§7.6): `ItemMoved` (and the occupant's `ItemMoved` to the backpack first), then `AppearanceChanged` if the list of §5.5 changed |
| `unequip` | `$item [slot=N]` | `ItemMoved` to backpack slot `N` or the first free slot (not usable or none → skip); `AppearanceChanged` if changed |
| `condition` | `$item CUR/MAX` (integers 0 … 10^6, CUR ≤ MAX) | `ItemCondition` |
| `wallet` | `coin N` or `REF N` | `WalletChanged` |
| `character` | (declaration, §5.1) | `CharacterInfo` after `Connected` |
| `refuse-character` | `STRING TEXT` (only in `on create-character` triggers) | `CharacterRefused` |
| `accept-character` | `STRING [class=…] [level=…] [species=… sex=…] [look=($l\|asked)]` (only in `on create-character`) | `CharacterInfo` for the new entry; the name joins the list for `EnterWorld`. `look=$l` gives it that look (§5.8); `look=asked` copies the look of the `CreateCharacter` command whose processing fired the trigger (none when that command carried none). The step's details are what world entry with that name uses (§5.1) |
| `remove-character` | `STRING` (only in `on create-character` and `on delete-character` triggers) | `CharacterRemoved` (§9.2); the name leaves the list, so `EnterWorld` with it then fails like an unknown name. A name not in the list → skip |
| `stat` | `ENTITY PROPVAL` (§5.8) | `PropertyChanged { entity, property, value }` (§9.2); `VALUE` `none` → `value` none. A dropped value (§5.8) → skip |
| `item-property` | `$item PROPVAL` (§5.8) | `ItemProperty { item, property, value }` (§9.2), whether or not the item is in a container; `none` → `value` none. A dropped value → skip |

`slot=VALUE` for `equip` is a number (0 … 65,535) or a label resolved at resolution time
through the mapper the property dictionary names for `Inventory_DefaultSlot` (`enum_value`,
§8.6); unresolved (no install, unknown label) → skip.

**Character steps** (§7.5) are `accept-character`, `refuse-character` and `remove-character`;
only they may stand in `on create-character` and `on delete-character` bodies, and
`accept-character` and `refuse-character` only in `on create-character` bodies (another
character step there is `NotAllowedHere`). `stat` and `item-property` are ordinary steps (not
before world entry). The client keeps the properties the session reports per entity and per
item, at most 1,024 each (§9.5).

<a id="s6-2"></a>
### 6.2 Targeting, skills and combat

| Step | Arguments and options | Emits |
|---|---|---|
| `target` | `ENTITY (ENTITY\|none)` | `TargetChanged` (whose target, and the new target) |
| `vital` | `ENTITY (health\|power\|resource=REF) CUR/MAX` (finite, 0 ≤ CUR ≤ MAX ≤ 10^9) | `VitalChanged`; remembers MAX |
| `combat` | `ENTITY on\|off` | `CombatState` |
| `quickslot` | `N (REF\|$item\|empty)` (N 1 … 1,024) | `QuickslotChanged`; content `Skill(ref)`, `Item(id)` or none |
| `cast` | `ENTITY REF DURATION [target=ENTITY\|none] [kind=cast\|induction\|channel\|craft]` (defaults `none` and `cast`) | If the entity has an active cast: `CastEnded` (`interrupted`) first. Then `CastStarted`, and schedules the automatic `CastEnded` (`completed`) at start + duration (§7.4 class 0) |
| `delay` | `ENTITY DURATION` | `CastDelayed`; moves the scheduled end later by DURATION (re-armed with a new sequence number). No active cast → skip |
| `interrupt` | `ENTITY [outcome=interrupted\|cancelled\|completed]` (default `interrupted`) | `CastEnded`; removes the scheduled end. No active cast → skip |
| `cooldown` | `REF DURATION` or `REF clear` | `CooldownStarted` (and schedules `CooldownEnded` at its end, class 0, replacing an end still scheduled for the same ref) or `CooldownEnded` (removing a scheduled end, if any) |
| `effect` | `ENTITY REF [for=DURATION] [stacks=N] [from=ENTITY]` (stacks 1 … 1,000) | `EffectApplied` with a new instance id; with `for`, schedules `EffectRemoved` at the end (class 0). An entity already holding 1,024 active instances → skip (§11.1) |
| `effect-end` | `ENTITY REF` | `EffectRemoved` for the active instance of that ref on the entity with the highest `EffectId` (the most recently applied), removing its scheduled expiry; none → skip |
| `damage` | `(ENTITY\|none) ENTITY AMOUNT [vital=health\|power\|resource=REF] [result=…] [with=REF] [type=VALUE] [left=CUR]` | `VitalDelta` (kind `damage`); with `left`, then `VitalChanged` with current CUR and the remembered max of that vital (CUR if none) |
| `heal` | same as `damage` | `VitalDelta` (kind `heal`), then optional `VitalChanged` |
| `defeat` | `ENTITY [by=ENTITY]` | `Defeated` |
| `revive` | `ENTITY` | `Revived` |

`AMOUNT` is a decimal 0 … 10^9. `result` is one of `normal` (default), `critical`, `miss`,
`evade`, `parry`, `block`, `resist`, `immune`. `type` is a damage-type value (a number, or a
label resolved by `enum_value` through the mapper of `Effect_DamageType`, `combat-data.md`
§6.1; an unresolved label gives `damage_type` none), opaque to the feed. `vital=` defaults to
`health`. The feed never changes a vital by itself: `left=` is a scripted value.

**Start of a scheduled end.** "Start" in this section (the automatic `CastEnded` of `cast`,
the `EffectRemoved` of `effect … for=`, the `CooldownEnded` of `cooldown`) is the virtual
time of the tick that executes the step, not the step's written time: a step written at
0.05 s with a 100 ms tick runs at 0.100, and a 30 ms cast it starts ends at 0.130, so in tick
0.200 (Vector F, §14.7). `move … over` counts its ticks from that tick too (§6.1), and
trigger bodies are scheduled relative to the current tick (§7.5).

<a id="s6-3"></a>
### 6.3 NPC windows: dialogue, quests, vendors and loot

Declarations: `page $p [speaker=ENTITY] TEXT` and `option $p N TEXT [goto=$page|close]`
(N 1 … 16, unique per page, at most 16 options; options are shown in ascending N).

| Step | Arguments and options | Emits |
|---|---|---|
| `show` | `$page` | `DialogueShown` in the dialogue window |
| `offer` | `REF [from=ENTITY]` | `QuestOffered` in the quest-offer window |
| `quest` | `REF` or fan-out `$sel`, then `active\|completed\|failed\|removed` | `QuestState` per quest |
| `objective` | `REF N COUNT/TOTAL [done]` (N 0 … 1,024, the objective's `Quest_ObjectiveIndex` as in §5.2; integers, COUNT ≤ TOTAL ≤ 10^6) | `QuestObjective` |
| `track` | `REF on\|off` | `QuestTracked` |
| `vendor` | `items=($sel\|REFLIST) [npc=ENTITY] [price=N\|N,N,…] [barter=REF:N] [stock=N]` | `VendorShown`: entry *i* (1-based) per element or list item; `price` one coin price for all or a list (the last list value repeats; default 0); `barter` one item cost added to every entry; `stock` default none (unlimited) |
| `loot` | `items=($sel\|REFLIST) [from=ENTITY] [qty=N] [coin=N]` | `LootShown` (entry quantity `qty`, default 1; `coin` default 0) |
| `loot-taken` | `N` | `LootTaken` for entry N of the open loot window; none → skip |
| `close` | `dialogue\|quest-offer\|vendor\|loot\|trainer\|travel\|instances\|trade` | `WindowClosed`; not open → skip |
| `rewards` (P3 revision) | 1 … 16 reward tokens: `KIND:N` with KIND one of `coin`, `glory`, `item-xp`, `craft-xp`, `mount-xp`, `virtue-xp`, `trait-points` (each at most once), or `rep:REF:N`, `wallet:REF:N` (any number) | `QuestRewards` (§9.2) in the open quest-offer window, entries in written order; no quest-offer window open → skip |
| `buyback` (P3 revision) | `items=($sel\|REFLIST) [qty=N] [price=N\|N,N,…] [barter=REF:N]` | `BuybackShown` (§9.2) in the open vendor window: entry *i* (1-based) per element or list item, quantity `qty` (default 1), `price` and `barter` as for `vendor`; no vendor window open → skip |

Price and stock entries are scripted; the feed does not compute a price from item data
(`economy.md` §11.6 describes the server's rule, which the feed does not run).

**Entry lists.** Every window step whose entries come from `items=`, `skills=`, `dest=` or the
`instances` argument accepts either a selector binding (fan-out, binding order) or a
**REFLIST**: one token of 1 … 1,024 comma-separated refs (elements or literal strings, for
example `items="Bread","Ale",$swords[0]`), in written order. Literal refs let synthetic
scenarios show filled windows with no install. For `trade-offer`, a list item may also be a
declared `$item`, which gives the entry's `item` and the item's ref, and a trade side holds
at most 32 entries: its REFLIST has 1 … 32 items (`LimitExceeded(entries)` at the `items=`
key otherwise), and a binding longer than 32 is cut to its first 32 with a `window` note
(§8.6 item 5).

**Rewards and buy-back (P3 revision).** `rewards` amounts are integers: `coin`, the five
experience and glory kinds and `wallet` 0 … 10^15, `trait-points` 0 … 10^6, `rep` −10^12 …
10^12 (`OutOfRange` otherwise, at the token); a token of another form or kind, or a second
token of one of the seven single kinds, is `UnexpectedToken` at that token; a 17th token is
`LimitExceeded(entries)` at that token; no token is `MissingArgument`. `rep` names a
faction (an element, for example of a `faction` selector, or a literal) and `wallet` a wallet
item. The amounts are scripted: the feed computes none from reward tables (`quests.md` §9 is
server data). `buyback` takes its entries like `vendor` (REFLIST or binding, at most 1,024).

> **OPEN:** (implementer, P3 revision, 2026-10-10; non-blocking) Facts the revision leaves
> open, implemented conservatively in `tessera-net`: (a) the buy-back page belongs to the
> vendor window that got the `BuybackShown`; a later `vendor` step on the still-open window
> keeps it, a later `buyback` step replaces it, and closing the window drops it. A
> `buy-back` pattern matches only a `BuyBack` for that window whose entry lies in its last
> buy-back list (§7.6 "unknown entry"). Please confirm whether a `vendor` step should drop
> the page instead. (b) Error precedence within `rewards`: the 17-token limit is checked
> before the 17th token's form, and a repeated single kind is reported after that token's
> own form and range checks (`coin:1 coin:-5` gives `OutOfRange` at the second token). A
> malformed amount (`coin:1.5`, `coin:0x10`) is `BadNumber`, a missing one (`coin:`) is
> `UnexpectedToken`. (c) A non-numeric entry of the `buy-back` input form (`buy-back x`,
> `buy-back entry=1`) or pattern (`entry=x`) is `BadNumber`, as for `buy`. (d) `npc-windows.md` §7.8 cites
> "§10.4" for `BuybackShown`; that section does not exist (§10.1 lists the event).

<a id="s6-4"></a>
### 6.4 Social

| Step | Arguments and options | Emits |
|---|---|---|
| `say` | `(ENTITY\|STRING) STRING [channel=say\|yell\|emote\|fellowship\|tell\|named:NAME]` | `ChatMessage` with sender = the actor's name, the player's name or the string; for `tell`, `ChatChannel::Tell { peer: sender }` |
| `system` | `STRING` | `ChatMessage` on `System`, sender none |
| `channel` | `join\|leave STRING` | `ChannelJoined` / `ChannelLeft` |
| `fellowship` | `(ENTITY\|STRING) { (ENTITY\|STRING) } [leader=(ENTITY\|STRING)]` or `none` | `FellowshipChanged` with the full member list (1 … 64, the player included only if listed); `none` → empty list (disbanded) |
| `invite` | `STRING` | `FellowshipInvite` |
| `trade` | `ENTITY` | `TradeShown` |
| `trade-offer` | `own\|peer [items=($sel\|REFLIST)] [qty=N] [coin=N]` | `TradeOffer` with the full side content (entry quantity `qty`, default 1; no `items` → no entries; `coin` default 0) |
| `trade-ready` | `own\|peer on\|off` | `TradeReady` |
| `friends` | `{ STRING:on \| STRING:off }` (0 … 1,024 entries; in practice at most 63, the token limit of a line) | `FriendList` |
| `ignore` | `{ STRING }` | `IgnoreList` |
| `perform` | `ENTITY STRING [delay=DURATION]` | `PerformanceStarted` (tune name as written; the client looks it up in the user's own music folder, `instruments.md`) |
| `perform-stop` | `ENTITY` | `PerformanceStopped` |

`named:NAME` is one token, NAME a string (1–64 bytes): `channel=named:"Lore"`.

P4 social additions (synchronised performances, fellow vitals): §6.7.

<a id="s6-5"></a>
### 6.5 Map, travel, mounts and instances

| Step | Arguments and options | Emits |
|---|---|---|
| `marker` | `$m POSITION TEXT [icon=ELEMENT] [quest=REF]` | `MarkerAdded` (the marker binding is declared by its first `marker` step) |
| `marker-remove` | `$m` | `MarkerRemoved` |
| `travel-list` | `dest=($sel\|REFLIST) [from=ENTITY] [coin=N\|N,N,…]` | `TravelShown` (`coin` as `price` of `vendor`: one value or a list whose last value repeats; default 0) |
| `travel` | `REF DURATION` | `TravelStarted` |
| `teleport` | `POSITION [facing=DEGREES]` | the sandbox's teleport path with an exact destination (`Destination::Location`, or `Destination::Anchored` with the element's anchor), never a ground or named lookup, so it completes at once: `LocationReport` (cause `Teleported`), then `EntityMoved` for the player. Without `facing`, the player's current heading |
| `mount` | `ENTITY (REF\|none)` | `MountChanged` |
| `instances` | `$sel` or REFLIST | `InstanceListShown` |
| `instance` | `REF\|none` | `InstanceChanged` |
| `markers` (P5 travel) | `$ms $sel (TEXT\|named) [icon=ELEMENT] [quest=REF] [note-type=VALUE]` (`$ms` a marker set, `$sel` a selector binding) | First `MarkerRemoved` for each marker the set holds, in ascending id order; then, fan-out over `$sel` in binding order, one `MarkerAdded` with a new run-time marker id (§5.3) for each element that has a `Location`, at that location. The label is TEXT, or with `named` the element's resolved name as `Text::Literal` (§8.6; the empty string when it has none). Elements without a location add nothing and are not skips. A step whose new markers would pass the run-time marker cap (§11.1) is a skip: nothing is emitted, removals included |
| `marker-remove` (P5 travel) | `$ms` (a marker set) | `MarkerRemoved` for each marker the set holds, ascending id order; the set is then empty. An empty set emits nothing and is not a skip |
| `travel-end` (P5 travel) | — | `TravelEnded` (the feed keeps no travel state; the step always emits) |
| `instance-queue` (P5 travel) | `STATE [REF] [seconds=N]`: STATE one of `none`, `confirm`, `queued`, `others-confirm`, `join-pending`, `confirm-enter-queue`, `others-confirm-enter-queue`, `world-join-queue`; N 0 … 86,400 | `InstanceQueueChanged { state, instance: REF or none, seconds: N or none }` |
| `mount-vital` (P5 travel) | `ENTITY (durability\|morale\|power\|fury) CUR/MAX` (finite, 0 ≤ CUR ≤ MAX ≤ 10^9) | `MountVitalChanged`. The feed remembers nothing and computes nothing from it |

`teleport` changes the frame of later `pos=` offsets: they stay relative to the **entry**
spawn location, so a scenario that teleports far should use `place(…)` positions after it.

**Options of existing steps (P5 travel).** All are optional; without them a step emits
exactly what it emitted before. "Last value repeats" works as for `price` of `vendor` (§6.3):
entry *i* takes the *i*-th list value, or the last one when the list is shorter. An
**entry list** `E,E,…` is one token of 1 … 1,024 distinct entry numbers 1 … 1,024; a number
beyond the step's last entry names nothing and is ignored. A **value list** `V,V,…` is one
token of 1 … 1,024 items.

| Step | Option | Event field |
|---|---|---|
| `marker`, `markers` | `note-type=VALUE`: an integer 1 … 64, or a label resolved with `enum_value` context `MapNoteType` (§8.6); a label whose key lies outside 1 … 64 counts as unresolved; unresolved: none, with a `label` note (attribute `note-type`) when an install is present | `MarkerAdded.note_type` (the map-note type whose icon the client half draws when `icon` is absent, `map-windows.md` §6.7) |
| `travel-list` | `min-level=N\|N,N,…` (0 … 10,000; last value repeats) | entry `min_level` |
| `travel-list` | `undiscovered=`, `unmet=`, `unusable=` (entry lists) | entry `flags` bits 0, 1, 2 |
| `travel-list` | `swift=`, `slow=` (entry lists; an entry named by both is `UnexpectedToken` at the later of the two keys in the line) | entry `route` (`Swift`, `Slow`) |
| `travel-list` | `area=REFLIST` (1 … 1,024 refs, usually elements of an `area` selector; last value repeats) | entry `area` |
| `travel` | `route=swift\|slow` | `TravelStarted.route` |
| `instances` | `from=ENTITY`, `featured=REF` | `InstanceListShown.source`, `.featured` |
| `instances` | `level=N\|N,N,…` (1 … 10,000; last value repeats) | entry `level` (the level shown) |
| `instances` | `size=V\|V,V,…`, `tier=V\|V,V,…` (value lists: integers 0 … 2^32 − 1 or labels resolved with `enum_value` contexts `GroupSize`, `DifficultyTier`; last value repeats; an unresolved label gives none for the entries that take it, with a `label` note when an install is present) | entry `size`, `tier` (the group-size and tier keys shown) |
| `instances` | `locked=` (entry list) | entry `locked` (the row is drawn in its error look) |
| `mount` | `kind=basic\|combat`, only with a REF (with `none` it is `UnexpectedToken` at the key) | `MountChanged.combat` (`basic` → `Some(false)`, `combat` → `Some(true)`; absent → none) |

Every value is scripted: the feed computes no level, cost, flag, route, queue state, mount
class or vital from the data (decision 0024 §2 principle 4). With `kind=` absent the client
half classifies a recorded mount itself (`lua-plugin-api.md` §15.5); the feed does not.

<a id="s6-6"></a>
### 6.6 Progression and crafting

| Step | Arguments and options | Emits |
|---|---|---|
| `xp` | `CUR/NEEDED` (integers 0 … 10^15) | `ExperienceChanged` |
| `learn` | `REF` or fan-out `$sel` | `SkillLearned` |
| `forget` | `REF` | `SkillForgotten` |
| `trainer` | `skills=($sel\|REFLIST) [npc=ENTITY] [coin=N\|N,N,…] [level=N\|N,N,…]` | `TrainerShown` (`coin` and `level` as `price` of `vendor`; defaults 0) |
| `trait-points` | `AVAILABLE/TOTAL` | `TraitPoints` |
| `trait` | `N (REF\|none) [rank=N]` (slot 0 … 1,024, rank 0 … 1,000) | `TraitSlotted` |
| `profession` | `REF tier=N proficiency=A/B mastery=C/D` | `ProfessionChanged` |
| `recipe` | `REF` or fan-out `$sel` | `RecipeLearned` |
| `craft-result` | `REF success\|critical\|failed [item=$item]` | `CraftResult`; with `item`, the item must already have been given |
| `reputation` | `REF POINTS [tier=N]` (POINTS −10^12 … 10^12) | `ReputationChanged` |
| `title` | `REF` | `TitleGranted` |

A crafting bar is `cast player REF DURATION kind=craft` followed by `craft-result`.

<a id="s6-7"></a>
### 6.7 P4 social: synchronised performances and fellow vitals (decision 0035 D4)

| Step | Arguments and options | Emits |
|---|---|---|
| `perform` … `sync` | `(ENTITY\|STRING) STRING sync` | `PerformanceReady { member, entity, tune }`: the member's part waits for a synchronised start. `member` is the STRING, the player's character name for `player`, or the actor's name; `entity` is the player or the actor, none for a STRING. Replaces the member's earlier waiting part. A member with an empty name, or a 65th distinct waiting member → skip |
| `perform-start` | `[by=(ENTITY\|STRING)] [lead=DURATION]` (lead 0 … 10 s, default 700 ms) | `PerformanceSyncStart { by, at_ms, lead_ms }`, then the feed's waiting parts are emptied. `by` is the starter's name as for `member` (none without `by=`); `at_ms` = the virtual time of the tick that executes the step + `lead_ms`. Emitted also when nothing waits |
| `perform-withdraw` | `ENTITY\|STRING` | `PerformanceWithdrawn { member }`; removes the member's waiting part. None waiting → skip |
| `fellow-vital` | `STRING (health\|power\|resource=REF) CUR/MAX` (name 1–64 bytes; values as `vital`, §6.2) | `FellowVitalChanged { member, vital, current, max }` |

**Forms.** `sync` is a keyword that stands after the tune; with it the first argument may be
a STRING, and `delay=` is not allowed (`UnexpectedToken` at the `delay=` key). Without `sync`,
`perform` keeps its §6.4 form (an ENTITY first; a STRING there is `UnexpectedToken`).
`fellow-vital` takes a STRING only (an ENTITY there is `UnexpectedToken`): a member with an
entity reports its vitals through `vital` of that entity (§6.2).

**Meaning for the client half** (normative for the feed's events; the playing is
`instruments.md` §8.8–§8.9 and §12 P4 Decision, which the client follows):

- `PerformanceReady` makes the named member's part wait (the tune is read from the user's
  own music folder, like a `PerformanceStarted` tune). A `PerformanceReady` for the player
  is handled like an accepted `/play … sync` of that tune by the player.
- `PerformanceSyncStart` is **one** synchronised start. Every part waiting in the client at
  that moment, the player's and every member's, starts with the same start time *S*: on the
  client's audio clock, the time at which it processes the event plus `lead_ms`; in a
  headless run on the virtual clock, `at_ms` itself. The **start tolerance** is the one of
  `instruments.md` §8.9; the feed defines none. The default lead of 700 ms equals the lead
  `instruments.md` §12 P4 Decision 3 gives a local start.
- `PerformanceWithdrawn` removes that member's waiting part, if any.
- `FellowVitalChanged` is a vital of the fellowship member named `member` as the group
  reports it, whether or not the member has an entity (`social-panels.md` §4.4: a row keeps
  its vitals out of view). The client half shows, for each member row, the latest of the
  member's `FellowVitalChanged` and (for a member with an entity) its entity's
  `VitalChanged`.
- `PerformanceStarted` with `delay` keeps its meaning (a solo-like start, *S* = processing time
  + delay) and never starts a waiting part.

**The player's commands** (§9.4): the client sends `ReadyPerformance { tune }` once it has
accepted the player's `/play … sync` locally, and `StartPerformance` for `/playstart`. The feed
records the player's part as waiting on `ReadyPerformance` (no event) and answers
`StartPerformance`, under `defaults on`, with one `PerformanceSyncStart` by the player's
character name with the default lead (§7.6). With a backend that accepts `StartPerformance`
(the feed and the plain sandbox, §9.8) the client starts waiting parts **only** on the
`PerformanceSyncStart` that comes back, never also locally, so a `/playstart` gives exactly
one start; with a backend that refuses it (the test-server backend) the client keeps its
local start of `instruments.md` §12 P4 Decision 3.

Values stay scripted: the feed computes nothing but `at_ms` = tick time + lead, a sum of two
values of the run (§15 invariant 12).

---

<a id="s7"></a>
## 7. Timing, repeats, triggers and input

<a id="s7-1"></a>
### 7.1 Virtual clock

- The clock is in whole milliseconds. Tick *k* has virtual time *k* · tick.
- **Time 0** is world entry: the feed records `t0` = the `now` at which the sandbox emits
  `WorldEntered` and processes tick 0 in that same call, after the entry events.
- On each `poll(now)`, the feed processes, in order, the ticks *k* not yet processed with
  `t0 + k · tick ≤ now`, **at most 8 per poll** (draining ticks of §7.3 included). The
  remaining due ticks are processed by later polls: virtual time then lags behind `now` and
  catches up over the following frames, and the windowed HUD shows the lag (§12.4).
  Wall-clock stalls never skip a tick. The headless driver advances one tick per frame
  (§12.2), so the cap never binds there and determinism is unaffected; in windowed runs it
  keeps one poll from delivering more than 8 ticks' worth of events (§7.4 caps) on the frame
  thread after a stall.
- Before world entry there is no virtual time and no tick; the feed's pre-world output is
  produced in `submit` (§3.3) and logged as `pre` (§10.2).

<a id="s7-2"></a>
### 7.2 Timing of step lines

- `at T` gives absolute virtual time *T* (top level only; in a block it is
  `UnexpectedToken`).
- `+D` gives the time of the previous step line **of the same block** plus *D*. The first
  step line of a block counts from the block's start: 0 at top level, the fire time in an
  `on` body, the iteration start in a `repeat` body.
- A `repeat` opener is itself a step line for this rule: it takes its time from its timing,
  and the next line after its `end` counts from that time.
- A step whose time is not a multiple of the tick runs at the first tick at or after it.
- Every absolute time computed statically (top level and repeat iterations) MUST be ≤ 60 min
  and, with `end-at`, ≤ `end-at` (`OutOfRange` otherwise). Steps in `on` bodies are checked at
  run time: one due beyond the horizon is dropped (counted as `dropped`).

<a id="s7-3"></a>
### 7.3 Repeats and the end of a run

`timing repeat N every D` … `end` runs its body *N* times (1 … 10,000), iteration *i*
(0-based) starting at S + *i* · D, where S is the opener's time. D ≥ 1 ms. Bodies may overlap
when they last longer than D. The product *N* × (body step lines), summed over every repeat
(those inside `on` bodies counted once per declaration), MUST be ≤ 200,000 (`LimitExceeded`).
There are no unbounded loops: the only entry that schedules itself is the bounded
intermediate-position entry of `move … over` (§6.1, at most ⌈over / tick⌉ runs), and steps
never send commands.

**End of the run.**

- With `end-at E`: the run ends when tick ⌈E / tick⌉ has been processed (reason `Complete`).
  Agenda entries due later are dropped and counted.
- Without `end-at`: let *L* be the virtual time of the tick that **processes** the last input
  line the run plays: max(1, ⌈*T* / tick⌉) · tick for a line `at T` (the driver sends a line
  in frame max(1, ⌈*T* / tick⌉), §12.2 step 4), 0 for `at pre` lines, and 0 without input or
  in a windowed run without `--scenario-input`. The run ends (reason `Complete`) at the first
  tick at or after *L*, after processing it, at which the agenda is empty. A scenario whose
  only input is `at 0s` therefore runs at least through tick 1 (Vector F, §14.7).
- In both cases, a run still going when tick ⌈3,600,000 ms / tick⌉ has been processed ends
  with reason `Horizon` (not an error); agenda entries are dropped and counted.
- **Draining.** The status becomes `Finished` only once the output queue (§7.4) is empty: if
  events are still held over by the per-tick cap when the run ends, the following ticks
  deliver them (at most 4,096 each) without processing commands or the agenda, and
  `Finished` carries the time of the tick that delivered the last one.
- **After `Finished`.** In a headless run the driver writes the end line and disconnects
  (§12.2), so nothing follows. In a windowed run (§12.4) the session stays open: after
  `Complete` without `end-at`, commands still fire triggers and get generic answers, and a
  firing sets the status back to `Running` until the agenda and queue are empty again
  (interactive scenarios built only from triggers therefore keep working); after `end-at` or
  `Horizon`, the feed processes no more ticks and answers no command (new-variant commands
  are ignored as by a sandbox without a scenario, and chat is echoed by the sandbox's
  `echo_chat`).

<a id="s7-4"></a>
### 7.4 Agenda and ordering within one tick

Every scheduled entry has a due time, a **class** (0 = automatic end: cast end, effect
expiry, cooldown end, `move … over` intermediate positions; 1 = a scripted step) and a
**sequence number** from one 64-bit counter that starts at 0: top-level step lines and repeat
iterations receive theirs at load time in file order, a top-level `repeat` expanded in place
iteration by iteration (all body lines of iteration 0 in line order, then iteration 1, …), and
every later scheduling (a trigger body expanded the same way, a re-armed or rescheduled entry)
takes the next value. Sequence numbers are unique, so (due time, class, sequence number) is a
total order.

Processing tick *k* (time *t*):

1. **Commands.** Commands submitted since the previous tick, in submission order (the feed
   holds at most 1,024 unprocessed commands; `submit` refuses more with
   `NetError::SendQueueFull`, which a headless driver counts as `input-skipped`). For each:
   the first matching trigger (§7.5) is chosen on the feed state **before** the command is
   answered; then the generic answer runs (§7.6, if `defaults on`); then the chosen
   trigger's body lines are scheduled (due ≥ *t*, sequence numbers in line order).
2. **Agenda.** Repeatedly take the entry with the smallest (due time, class, sequence number)
   among those with due time ≤ *t*, and execute it. Entries scheduled during this phase with
   due time ≤ *t* run in the same tick.
3. **Emit.** Events go to the output queue in execution order. Events leave the queue in
   order while the tick's delivery budget allows: at most 4,096 events **and** at most 65,536
   weight units per tick (weight below). Every event the feed can emit weighs less than
   65,536, so the first held event of a tick always leaves. The rest stay queued, in order,
   for the next ticks (counted as `spilled`; §10.4).

**Weight.** An event's weight is 1, plus 1 per element of each list it carries (window
entries, dialogue options, fellowship members, friend and ignore entries, the item costs of
each price, equipment entries), plus ⌈*b* / 64⌉ for each string of *b* bytes it carries
(names, literal refs, literal texts, chat text, tune names, channel names). The largest
possible event (a 1,024-entry vendor window, each entry with a literal ref and eight literal
item costs) weighs 18,433.

**Queue limits, checked on every push.** The output queue holds at most 1,048,576 events and
at most 2,097,152 weight units in total, and a run emits at most 16,777,216 events (pre-world
output included). The limits are checked on every push, never only at the end of a phase: a
push that would pass one of them stops the feed at once with the runtime error
`QueueOverflow` (the queue limits) or `EventOverflow` (the run total), carrying the line of
the step being executed (line 0 for a generic answer), and the rest of the tick (commands,
agenda) is not executed (§11.3). A fan-out step computes its element count, and a window step
its event's weight, before it allocates anything, and checks them against the remaining
capacity; a step that cannot fit fails the same way without building its events. The queued
events' heap is therefore bounded by a fixed multiple of 2,097,152 units (well under 1 GiB;
§11.4).

A command submitted with a `now` is processed at the first tick not yet processed whose time
is ≥ `now − t0`. The agenda holds at most 262,144 entries (`AgendaOverflow`, a runtime error).

<a id="s7-5"></a>
### 7.5 Triggers

`on PATTERN [times=N]` … `end` scripts an answer to a client command. Its body holds step
lines (timing `+` only) and `repeat` blocks. When a command is processed (§7.4 step 1), the
**first** trigger in file order whose pattern matches and that has firings left fires once:
its body is scheduled relative to the current tick. `times` (1 … 10,000) limits its firings;
without it a trigger fires any number of times. At most 256 triggers.

A pattern is a command word and optional filters; an omitted filter matches anything. `any`
matches anything explicitly; `none` matches an absent value.

| Pattern | Matches the command | Filters |
|---|---|---|
| `use-skill [REF] [quickslot=N] [target=ENTITY\|none\|any]` | `UseSkill` | `REF`: the skill; `quickslot=N`: the skill equals the content the feed last set for slot N (§5.7) |
| `use-item [$item] [target=…]` | `UseItem` | |
| `use ENTITY` | `UseEntity` | |
| `target (ENTITY\|none\|any)` | `SelectTarget` | |
| `cancel-cast` | `CancelCast` | |
| `choose $page [N]` | `ChooseOption` while the dialogue window shows `$page` | option N |
| `accept [REF]`, `decline [REF]` | `AnswerQuest` | the offered quest |
| `abandon [REF]` | `AbandonQuest` | |
| `track [REF]` | `TrackQuest` | |
| `buy [entry=N]` | `Buy` | |
| `sell [$item]` | `Sell` | |
| `take-loot [entry=N\|all]` | `TakeLoot` | |
| `equip [$item]`, `unequip [$item]`, `move-item [$item]`, `destroy [$item]`, `split [$item]` | the inventory commands | |
| `chat [channel=…] [text=STRING]` | `Chat` | exact text |
| `join-channel [STRING]`, `leave-channel [STRING]` | channel commands | |
| `invite [STRING]`, `answer-invite [accept\|decline]`, `leave-fellowship`, `kick [STRING]`, `set-leader [STRING]` | fellowship commands | |
| `open-trade [ENTITY]`, `offer-trade`, `trade-ready` | trade commands | |
| `friend [STRING]`, `unfriend`, `ignore-player`, `unignore` | list commands | |
| `play-sync [tune=STRING]`, `play-start`, `disband` | (P4 social) `ReadyPerformance`, `StartPerformance`, `DisbandFellowship` | `tune=`: exact tune |
| `choose-travel [entry=N]`, `join-instance [entry=N]`, `leave-instance`, `dismount` | world commands | |
| `train [entry=N]`, `slot-trait [N]`, `craft [REF]` | progression commands | |
| `create-character [name=STRING]`, `delete-character [STRING]` | character commands (before world entry) | The body may hold only character steps (§6.1) |
| `close KIND` | `CloseWindow` of that kind | |
| `buy-back [entry=N]` (P3 revision) | `BuyBack` | |
| `leave-queue` (P5 travel) | `LeaveQueue` | |

A trigger on `create-character` or `delete-character` fires before world entry; its body
runs at once in the processing of that command in `submit` (§3.3), in line order, with times
ignored (there is no clock yet; any timing other than `+0s` is `OutOfRange`). Only character
steps (§6.1) are allowed there (another verb is `NotAllowedHere`), no `repeat` (§4.4), and at
most 16 step lines per body (`LimitExceeded(steps)` at the 17th). A run fires at most 64
pre-world triggers in total; a later `CreateCharacter` or `DeleteCharacter` fires none and
gets no answer. Pre-world output is therefore at most 64 + 64 × 16 = 1,088 events, delivered
in the next poll and logged as `pre`.

**Chat patterns.** `channel=` in a `chat` pattern and in the input form `chat` (§7.7) takes
`say`, `yell`, `emote`, `fellowship`, `tell:NAME` and `named:NAME` (NAME a string, 1–64 bytes,
one token like `named:`); a pattern also takes `tell` alone, matching a tell to any peer.

<a id="s7-6"></a>
### 7.6 Generic answers (`defaults on`)

These acknowledgements are Tessera's own and check nothing from the data: no slot
compatibility, binding, stack limits, prices, levels or requirements.

| Command | Generic answer |
|---|---|
| `SelectTarget` | `TargetChanged` for the player |
| `SetQuickslot` | `QuickslotChanged` |
| `ChooseOption` on an option with `goto=$page` | `DialogueShown` of that page in the same window; `goto=close` → `WindowClosed` |
| `CloseWindow` | `WindowClosed` (if open) |
| `AnswerQuest` accept | `QuestState` (`active`) for the offered quest, then `WindowClosed`; decline → `WindowClosed` |
| `TrackQuest` | `QuestTracked` with the command's quest and flag |
| `MoveItem` | `ItemMoved`; an item already at the target first swaps into the moved item's old place (`ItemMoved` for the moved item, then for the occupant); `AppearanceChanged` if the list of §5.5 changed. A target slot at or above the container's size → nothing |
| `EquipItem` with a slot | occupant (if any) `ItemMoved` to the first free backpack slot, then `ItemMoved` of the item, then `AppearanceChanged` if changed. Without a slot, or with an occupant and no free backpack slot → nothing (the `equip` step: skip) |
| `UnequipItem` | `ItemMoved` to the first free backpack slot; `AppearanceChanged` if changed. No free backpack slot → nothing |
| `SplitStack` | `ItemQuantity` (old minus the split quantity), then `ItemAdded` of a new item at the first free slot of the same container; quantity not below the item's, or no free slot in that container → nothing at all (no `ItemQuantity` either) |
| `DestroyItem` | `ItemRemoved` |
| `TakeLoot` | per taken entry, in entry order: `LootTaken`, then `ItemAdded` (new item, quantity of the entry) at the first free backpack slot; from the first entry for which no backpack slot is free, nothing more (neither `LootTaken` nor `ItemAdded`) |
| `CancelCast` | `CastEnded` (`cancelled`) for the player's active cast, removing its scheduled end |
| `StartPerformance` | (P4 social) `PerformanceSyncStart { by: Some(the player's character name), at_ms: the tick's time + 700, lead_ms: 700 }`, then the waiting parts are emptied (§6.7) |
| `ReadyPerformance` | (P4 social) no event; the player's part is recorded as waiting with the command's tune (§5.7, §6.7) |
| `Chat` | as the sandbox's own chat path does without a scenario, which does not run while the feed runs (§3.2): a `ChatMessage` on the same channel and text with the player's name as sender; a tell to *PEER* instead gets `ChatMessage` on `System`, sender none, text *PEER* followed by ` is not in the sandbox.` (Vector F) |

Every other command gets no generic answer. A command that refers to an unknown item, window
or entry gets no answer (and no trigger fires on a pattern whose filter names it).

<a id="s7-7"></a>
### 7.7 Input script

`input` … `end` lists player commands for headless runs. Each line is `at TIME command` or
`at pre command`; at most 10,000 lines; times need not be sorted (the driver sorts by time,
keeping file order for equal times). Times obey the horizon. With `end-at E`, a line `at T`
MUST be processed by the run: max(1, ⌈*T* / tick⌉) ≤ ⌈*E* / tick⌉, otherwise `OutOfRange` at
its time token (so `end-at 0s` allows only `at pre` lines). Optional quantities (`qty=`)
default to 1.

**Ranges.** Every number of an input form is range-checked at parse time (`OutOfRange`
otherwise), so every line that parses can be built into its command: option numbers
(`choose`) 1 … 16; entry numbers (`buy`, `take-loot`, `choose-travel`, `join-instance`,
`train`) 1 … 1,024; quickslot numbers 1 … 1,024; trait slots 0 … 1,024; container slots
(`move-item … slot=`) 0 … 65,535; `equip … slot=` 0 … 65,535 or a label; quantities (`qty=` of
`buy`, `sell`, `offer-trade`; `split` *Q*) 1 … 10^9; `craft … qty=` 1 … 65,535 (the width of
`Craft.quantity`); `offer-trade coin=` 0 … 10^15; `create-character … nationality=` 0 … 2^32 − 1
or a label.

**The driver's mirror.** Forms that refer to the client's view (a quickslot's content, the
current target, the open window of a kind, the page it shows) read the driver's mirror, which
reflects the events received up to the previous poll; two input lines sent in the same frame
both see the state before that frame's poll.

| Command form | Sends |
|---|---|
| `target (ENTITY\|none)` | `SelectTarget` |
| `use-skill (REF\|quickslot=N) [target=(ENTITY\|current\|none)]` | `UseSkill`; `quickslot=N` takes the skill or item the driver last saw in `QuickslotChanged` for slot N (an item sends `UseItem`); `target` default `current` (the driver's last `TargetChanged` for the player) |
| `use-item $item [target=…]` | `UseItem` |
| `use ENTITY` | `UseEntity` |
| `cancel-cast` | `CancelCast` |
| `choose $page N` / `choose N` | `ChooseOption` in the open dialogue window (with `$page`: only while that window shows `$page`) |
| `track REF on\|off` | `TrackQuest` |
| `accept`, `decline` | `AnswerQuest` for the open quest-offer window |
| `abandon REF` | `AbandonQuest` |
| `buy N [qty=Q]`, `sell $item [qty=Q]`, `take-loot (N\|all)` | in the open vendor or loot window |
| `equip $item slot=VALUE`, `unequip $item`, `move-item $item to=CONTAINER slot=N`, `destroy $item`, `split $item Q` | inventory commands |
| `quickslot N (REF\|$item\|empty)` | `SetQuickslot` |
| `chat STRING [channel=…]` | `Chat` (channel values §7.5; default `say`) |
| `play-sync STRING`, `play-start`, `disband` | (P4 social) `ReadyPerformance { tune }` (tune 1–256 bytes), `StartPerformance`, `DisbandFellowship` |
| `join-channel STRING`, `leave-channel STRING`, `invite STRING`, `answer-invite accept\|decline`, `leave-fellowship`, `kick STRING`, `set-leader STRING`, `open-trade ENTITY`, `offer-trade ($item [qty=Q]\|coin=N)`, `trade-ready on\|off`, `friend STRING`, `unfriend STRING`, `ignore-player STRING`, `unignore STRING` | social commands |
| `choose-travel N`, `join-instance N`, `leave-instance`, `dismount` | world commands |
| `train N`, `slot-trait N (REF\|none)`, `craft REF [qty=Q]` | progression commands |
| `create-character STRING [class=VALUE] [species=VALUE sex=VALUE] [nationality=VALUE] [look=$l]`, `delete-character STRING` | character commands; `at pre` only. `CreateCharacter.class` is the class value, none when `class=` is absent or its label unresolved; `nationality` likewise (a number, or a label resolved through the mapper the dictionary names for `NationalityTable_Nationality`, `player-character.md` §6.2; `enum_value` context `Nationality`); `appearance` is `Appearance::new(Body::Character { species, sex })` with the resolved look of `$l` (if any) when species and sex both resolve, otherwise none, and then the look is not sent (§9.4). Unresolved labels here do not skip the line |
| `close KIND` | `CloseWindow` |
| `buy-back N` (P3 revision; N 1 … 1,024) | `BuyBack` in the open vendor window |
| `join-instance N [size=VALUE] [tier=VALUE] [level=N]` (P5 travel options; `level` 1 … 10,000; `size` and `tier` an integer 0 … 2^32 − 1 or a label, contexts `GroupSize`, `DifficultyTier`) | `JoinInstance` in the open instances window with those fields (absent options: none). An unresolved label skips the line (`input-skipped`) |
| `leave-queue` (P5 travel) | `LeaveQueue` |

An input line whose target window, slot, page or element does not exist, whose label or
`m`/`f` value is unresolved (except in `create-character`, above), or that the session refuses (`Session::send` returns an error)
sends nothing, writes no log line and is counted as `input-skipped` (§10.4). In windowed runs
the input script is ignored unless `--scenario-input` is given (§12.4).

---

<a id="s8"></a>
## 8. Selectors

<a id="s8-1"></a>
### 8.1 Purpose

A selector names a set of records by what they are, never by number:
`select $swords = item kind=weapon n=3 seed=7`, `select $quests = quest all`,
`select $skills = skill class=@Label n=5 seed=7`. The client evaluates selectors against its
install; the scenario file stays free of record identifiers and game text.

<a id="s8-2"></a>
### 8.2 Language

`select $name = FAMILY {FILTER} COUNT [seed=S]`:

- **Family**: one keyword of §8.3. The special family `did` takes a comma-separated list of
  hexadecimal DIDs (at most 256): it resolves to those that the resolver finds as records of
  any family of §8.3. It exists for local, user-picked scenarios; committed scenarios MUST NOT
  use it (§13).
- **Filters** are ANDed. Each is `attribute=value` with an attribute of the family (an unknown
  attribute is `UnknownAttribute`; repeating one is `UnexpectedToken`). The attribute's type
  (§8.3 "Type" column) fixes which values it takes; any other value is `UnexpectedToken`:

  | Type | Accepts | Matches when |
  |---|---|---|
  | value | an alternation `a\|b\|c` of integers and labels | the record's value (or, for a bitfield or list, one of its keys or elements) equals one alternative |
  | keyword | an alternation of the keywords listed for the attribute | the record's classification (kind, role or level, as the row says) equals one alternative |
  | range | `lo..hi` (inclusive; either bound may be omitted, not both) or a single integer (= `n..n`) | the record's integer lies in the range |
  | integer | an alternation of integers | the record's integer equals one alternative |
  | yes/no | `yes` or `no` | the record's boolean (with the absence rule below) equals it |
  | binding | a selector binding declared earlier, of the family named in the row | the attribute's record is one of that binding's **resolved** records (after its count) |

- **Labels** (`@Name`) resolve through the enum mapper that the property dictionary names for
  the attribute's property, the one named in the "Defined by" column (for a list or struct
  attribute, the element's property) (`property-system.md` §7; `records.md` §7), comparing the
  mapper's label exactly. Display texts of a mapper are never matched. An unknown label
  matches nothing: that alternative is dropped and a note is logged; if no alternative
  remains, the filter matches nothing. With no install there are no label notes. A mapper
  name that the Label class of §4.2 cannot spell (one holding `:`, `.`, `-`, a space or
  another character outside `[A-Za-z0-9_]`) cannot be selected by label; this is accepted in
  v1. Local scenarios use the numeric value instead; committed ones cannot name such a value
  (§13 rule 2).
- **Count**: `all`, or `n=N` (1 … 65,536). With `seed=S` (0 … 2^64 − 1, decimal or
  hexadecimal), `n=N` is a seeded sample (§8.5); without a seed, the first N records in
  canonical order (§8.4). A missing count is `MissingCount`.
- An attribute whose property is absent from a record does not match any value except `no`
  for boolean attributes whose absence means false (marked in §8.3).

<a id="s8-3"></a>
### 8.3 Families and attributes

Every family and attribute is defined through an Accepted spec and the decoder view that
implements it. No attribute introduces a fact that those specs do not state. "Elements"
says what one resolved element is when it is not a plain record.

| Family | Records (spec, decoder view) | Attribute | Type | Defined by |
|---|---|---|---|---|
| `item` | Item records by the rule of `items.md` §2.4 (`ItemClasses::is_item`, `ItemView`) | `kind` | keyword: `weapon`, `recipe-scroll`, `multi-recipe-scroll`, `package`, `text-item`, `property-token`, `key`, `door`, `interactive-object`, `jewellery`, `shield`, `travel-point`, `milestone`, `wearable`, `general` | `items.md` §2.4 rule 4; `ItemView::kind` (`ItemKind`) |
| | | `slot` | value (slot mapper) | the bit of key *v* is set in `Inventory_DefaultSlot` (`items.md` §5.4; `ItemView::slots`; bit ↔ key `property-system.md` §7.3) |
| | | `fits` | value (slot mapper) | the bit of key *v* is set in `Inventory_CompatibleSlot` (`items.md` §5.4) |
| | | `equippable` | yes/no (absent = no) | `Inventory_DefaultSlot` has the bit of the slot mapper's label `Equipment` (`items.md` §5.4) |
| | | `category` | value | `Item_Class` (`items.md` §5.1; `ItemView::item_class`) |
| | | `quality` | value | `Item_Quality` (`items.md` §5.1; `ItemView::quality`) |
| | | `level` | range | `Item_Level` (`items.md` §5.2; `ItemView::level`) |
| | | `min-level` | range | `Usage_MinLevel` (`items.md` §5.2; `ItemView::min_level`) |
| | | `class` | value | one of the required classes (`items.md` §5.10; `ItemView::requirements` `classes`) |
| | | `stackable` | yes/no (absent = no) | `Inventory_MaxQuantity` > 1 compared unsigned (`items.md` §5.4; `ItemView::max_quantity`) |
| | | `has-icon` | yes/no (absent = no) | any `Icon_Layer_*` present (`items.md` §5.1; `icon-layers.md` §2.2) |
| | | `listed-by` | binding of `vendor-inventory` or `recipe` | the item is an element of a bound inventory's `VendorInventory_Items` (`economy.md` §6.3) or a bound recipe's `CraftRecipe_ResultItem` (`economy.md` §5.1) |
| `skill` | Skills (`combat-data.md` §2.1 rule 4; `CombatFamily::Skill`) | `class` | value (class mapper) | listed in `AdvTable_AvailableSkillEntryList` of the class table whose `AdvTable_Class` is *v* (`combat-data.md` §9.1) |
| | | `mount` | yes/no (absent = no) | listed in some class table's `AdvTable_AvailableMountSkillEntryList` (`combat-data.md` §9.1) |
| | | `type` | value | the bit of key *v* is set in `Skill_SkillType` (`combat-data.md` §7.2) |
| | | `harmful` | yes/no | `Skill_Harmful` (`combat-data.md` §7.2) |
| | | `monster` | yes/no (absent = no) | `Skill_IsMonsterSkill` (`combat-data.md` §7.2) |
| | | `induction` | yes/no (absent = no) | `Skill_InductionAction` present and non-zero (`combat-data.md` §7.9) |
| | | `toggle` | yes/no (absent = no) | `Skill_Toggle_Hook_Number` present (`combat-data.md` §7.9) |
| `effect` | Effects (`combat-data.md` §2.1 rule 1; `CombatFamily::Effect`) | `role` | keyword: the roles of `combat-data.md` §6.1 as the decoder's `EffectRole` names them, lower case, words joined by `-` (for example `property-modification`, `vital-over-time`, `instant-vital`, `mount`) | `CombatClasses::effect_role` |
| `trait` | Traits (`combat-data.md` §2.1 rule 2) | `nature` | value | `Trait_Nature` (`combat-data.md` §8.2) |
| | | `class` | value (class mapper) | listed in `AdvTable_ClassCharacteristic_List` of the class table whose `AdvTable_Class` is *v* (`combat-data.md` §9.1) |
| | | `min-level` | range | `Trait_Minimum_Level` (`combat-data.md` §8.2) |
| `trait-tree` | Trait trees (`combat-data.md` §8.5; `RelatedObject::TraitTree`) | — | | |
| `class` | Class advancement tables (`combat-data.md` §9.1; `player-character.md` §7, `Role::ClassTable`) | `class` | value | `AdvTable_Class` |
| `race` | Race tables (`player-character.md` §5.2; `Role::RaceTable`) | — | | |
| `creature` | Creature and NPC templates (`creatures.md` §2.2; `TemplateFinder`, `WeenieKinds`) | `kind` | keyword: `mob`, `npc`, `container` | `TemplateKind` (`creatures.md` §2.2) |
| | | `species` | value | `Agent_Species` (`creatures.md` §4.1; `Identity`) |
| | | `genus` | value | the bit of key *v* is set in `Agent_Genus` (`creatures.md` §4.1) |
| | | `alignment` | value | `Agent_Alignment` (`creatures.md` §4.1) |
| | | `agent-class` | value | `Agent_Class` (`creatures.md` §4.1) |
| | | `has-appearance` | yes/no (absent = no) | `Appearance_ClassList` present (`creatures.md` §4.2) |
| | | `vendor` | yes/no (absent = no) | `Vendor_InventoryList` present (`creatures.md` §4.6; `economy.md` §6.3) |
| | | `trainer` | yes/no (absent = no) | `SkillTrainer_Class` present (`creatures.md` §4.6) |
| | | `quest-giver` | yes/no (absent = no) | `QuestDispenser_ActiveQuests` non-empty (`creatures.md` §4.6) |
| `quest` | Quests and deeds (`quests.md` §2.1 kind "Quest or deed"; `QuestFamilyKind::Quest`, `QuestRecord`) | `deed` | yes/no | `QuestRecord::is_deed` (`quests.md` §4.1) |
| | | `category` | value | `Quest_Category` (`quests.md` §4.2) |
| | | `level` | range | stored `Quest_ChallengeLevel` (`quests.md` §4.2; not the effective level of §4.3) |
| | | `objectives` | range | number of objectives (`quests.md` §5.1; `QuestRecord::objectives`) |
| | | `class` | value | one of the top-level required classes (`quests.md` §8; `TopLevelUsage::required_classes`) |
| `npc-comment` | NPC comments (`quests.md` §7.7; `QuestFamilyKind::NpcComment`) | — | | |
| `title` | Titles (`quests.md` §12.1) | — | | |
| `emote` | Emotes (`quests.md` §12.2) | — | | |
| `faction` | Factions (`quests.md` §13.1) | — | | |
| `recipe` | Recipes (`economy.md` §5.1; `EconomyKind::Recipe`) | `profession` | binding of `profession` | `CraftRecipe_Profession` |
| | | `tier` | value | `CraftRecipe_Tier` (`economy.md` §5.1) |
| `profession` | Professions (`economy.md` §5.2) | — | | |
| `vendor-inventory` | Vendor inventories (`economy.md` §6.3) | `of` | binding of `creature` | named in a bound NPC's `Vendor_InventoryList` |
| `trophy-list` | Trophy lists (`economy.md` §4.1) | — | | |
| `place` | Entries of the places table (`map-data.md` §3; `PlacesTable`). Elements: DID = the entry's `area` field (0 for most entries, `map-data.md` §3.2; such an element is still an element, and as a ref it is `Record(0x00000000)`, which the client half shows as unknown), row = its stored index, `Location` and anchor from its position (`coordinates.md` §4, §7), resolved name (for actor `name(…)`) = the entry's name | `region` | integer | the position's region (`map-data.md` §3.2) |
| `map-note` | Rows of the map-notes table (`map-data.md` §4; `MapNotesTable`). Elements: DID = the row's entity, row = stored index, `Location` and anchor from its position | `type` | value | the bit of key *v* is set in the row's `type` field (`MapNote_Type` bitfield, `map-data.md` §4.2; `MapNote::type_keys`) |
| | | `region` | integer | the row's region (`map-data.md` §4.6) |
| `travel-destination` | Travel destinations (`map-data.md` §8.3; `MapKind::TravelDestination`) | — | | |
| `area` | Areas, territories, regions and dungeons (`map-data.md` §2.3; `MapKind`) | `level` | keyword: `area`, `territory`, `region`, `dungeon` | `MapKind::Area` / `Territory` / `Region` / `Dungeon` |
| `instance` | Private-encounter templates (`instances.md` §4; `InstanceKind::Template`) | — | | |
| `hotspot` | Hotspots (`interaction.md` §4), found by `interaction.md` §2.2 | — | | |
| `pet` | Cosmetic pets (`interaction.md` §7.1) | — | | |
| `induction` | Induction actions (`interaction.md` §3.8; `combat-data.md` §7.9) | — | | |
| `map-region` (P5 travel) | Map regions (`map-windows.md` §5.1: R-nodes and R-dungeons). Elements: an R-node gives DID = its active element ID (a UI element ID, opaque to the feed) and row = 1 + its position in depth-first order of the map tree (`map-windows.md` §4.1); an R-dungeon gives DID = the map-data set's DID and row 0. `Location` = the test point of `map-windows.md` §5.4 step 2, absent for a region of categories C1–C5 (§5.3 there); no anchor. Resolved name = rendered `UI_Map_MenuName` of the node; for an R-dungeon the rendered `Dungeon_Name` of the lowest-DID dungeon whose `Dungeon_MapData` names the set (`map-windows.md` §7.2). As a ref an element names no record the client half can show (shown as unknown) | `kind` | keyword: `node`, `dungeon` | the region's kind (`map-windows.md` §5.1) |
| | | `region` | integer | the region of the element's `Location` (no location: matches nothing) |
| `effect` (P5 travel attributes) | as above | `mount-type` | value (mount-type mapper) | the bit of key *v* is set in `Mount_Type` (`mounts.md` §2.8; the `gameplay::mounts` view) |
| | | `war-steed` | yes/no (absent = no) | `Mount_Type` has a bit of 0x38 (`mounts.md` §2.8, war-steed rule) |

The families of `interaction.md` have no decoder view in `crates/formats` yet; `tessera-gamedata`
implements their finding rule (`interaction.md` §2.2) when it adds their stores. Until then a
resolver MAY report them as `Unsupported` (§8.6), which resolves empty with a note.

**P5 travel families.** `map-region` cites `map-windows.md` (Review on 2026-10-11) and stands
until that spec is Accepted, like the P3 rows citing specs then in Review; a resolver MAY
report it `Unsupported` until `tessera-gamedata` builds the region list. With it, a scenario
can put one marker on every map region (`select $r = map-region all`, then
`markers $all $r named`), which is the marker census of `map-windows.md` §5.4. Every mount
effect is `effect role=mount all`; `war-steed=` and `mount-type=` narrow it.

> **OPEN:** (non-blocking) P1–P6 will want more attributes (for example item sets, deed
> categories, recipe vocations). Each addition is a revision of this table citing an Accepted
> spec; the grammar does not change.

<a id="s8-4"></a>
### 8.4 Canonical order

Candidates are sorted ascending by (DID as an unsigned 32-bit number, row number), and
(DID, row) is a candidate's identity: of several candidates with the same (DID, row), only
the one whose (location, anchor) sorts first is kept, under this total order: no location
before a location; locations by region, then *x*, *y*, *z* by `f64::total_cmp`; then no
anchor before an anchor; anchors by the block's region, *x*, *y*, then the cell, all
unsigned. Plain records have row 0. `tessera-net` sorts and deduplicates
itself and never relies on the resolver's order. Every list a step fans out over, every
index `$x[i]` and every window entry order follows this order.

<a id="s8-5"></a>
### 8.5 Seeded sampling (Tessera's choice)

**Generator: SplitMix64** with a 64-bit state, all arithmetic modulo 2^64:

1. state ← state + 0x9E3779B97F4A7C15;
2. z ← state; z ← (z XOR (z >> 30)) × 0xBF58476D1CE4E5B9;
3. z ← (z XOR (z >> 27)) × 0x94D049BB133111EB;
4. output z XOR (z >> 31).

The initial state is the selector's seed unchanged.

**Bounded draw** for a bound *b* ≥ 1: threshold *t* = (2^64 − *b*) mod *b*. Draw outputs until
one is ≥ *t*; return it mod *b*. (Unbiased; the loop ends with probability 1 and rejects
fewer than one draw in 2^32 for the bounds allowed here.)

**Sample** of *n* from the *m* candidates in canonical order:

1. If *n* ≥ *m*, the result is all candidates.
2. Otherwise copy the candidates into an array *a*. For *i* = 0 … *n* − 1: *j* = *i* + draw(*m*
   − *i*); swap *a*[*i*] and *a*[*j*].
3. The result is *a*[0 … *n* − 1], sorted again into canonical order.

An implementation may keep the swaps in a sparse map instead of copying *a*, provided the
result is identical. Each selector has its own generator; resolution order does not affect
results. Vectors: §14.3.

<a id="s8-6"></a>
### 8.6 Resolution contract

**Trait shape** (`tessera_net::scenario::SelectorResolver`, implemented by the client's
adapter over `tessera-gamedata`; synchronous, called before the session starts, never from
`Backend::poll`):

| Method | In | Out |
|---|---|---|
| `has_install` | — | whether data is available |
| `candidates` | a `SelectorQuery` (family, filters with labels still unresolved, the resolved lists of earlier bindings for reference filters), a candidate limit (1,048,576), an output vector | appends `Candidate { did: DataId, row: u32, location: Option<Location>, anchor: Option<Anchor> }` and returns a `CandidateOutcome { status, unmatched_labels }`. `status` is `Ready`, `Truncated` (more candidates exist than the limit), `NoInstall`, `Unsupported` (family not available), or `Failed(reason)` (the install as a whole could not be opened or read). `unmatched_labels` lists the labels of this query that matched nothing, as (attribute, label) pairs in the order the labels appear in the `select` line, for notes (empty without an install); the result carries them, so no hidden state links two calls. A record that fails to decode is not a candidate (the resolver may log it); it does not make the call fail. When it truncates, the resolver MUST return the first *limit* candidates in canonical order (§8.4), so the result does not depend on its iteration order |
| `enum_value` | a label context (`Class`, `Species`, `Sex`, `EquipSlot`, `DamageType`, `Nationality`, or `PropertyKey(property ID)`) and a label (`@Name`, or `m` / `f` for `Sex`) | `Option<u32>`: the value through the mapper or table of §5.1, §6.1, §6.2, §7.7 (`AdvTable_Class`, `RaceTable_Race`, `RaceTable_GenderType`, `Inventory_DefaultSlot`, `Effect_DamageType`, `NationalityTable_Nationality`), or for `PropertyKey` the key of the label in the enum mapper the dictionary names for that property (type codes 8, 14, 18; `property-system.md` §2.2 `data`); `None` without an install, for an unknown label, or for a property without such a mapper |
| `property` | a label (`@Name`) | `Option<PropertyInfo { id: u32, kind: PropertyKind }>`: the dictionary's property of that name and its kind (§5.8; `Unsupported` for the other type codes); `None` without an install or for an unknown name |
| `property_by_id` | a property ID | `Option<PropertyInfo>` as above; `None` without an install or for an ID the dictionary does not define |
| `display_name` | a `DataId` and, for table families, the row | `Option<String>`: the record's display name or, for a `place` element, the table entry's name; at most 64 bytes, cleaned with `sanitize_text` |

**Resolution, in declaration order of the `select` lines:**

1. Call `candidates`. `Failed` → resolve error `InstallUnreadable` (exit 4). `NoInstall` and
   `Unsupported` → an empty list with a note.
2. Sort and deduplicate (§8.4), cap at the limit (a note if `Truncated`).
3. Apply the count (§8.2, §8.5). Fewer records than requested → a note; never an error.
4. Store the list. The total over all bindings MUST be ≤ 1,048,576 (`ResolveLimit` at the
   offending `select` line, exit 4).
5. Fan-out steps (§6) over a binding longer than 65,536 use its first 65,536 elements, window
   steps over a binding longer than 1,024 its first 1,024, and `trade-offer … items=` over a
   binding longer than 32 its first 32 (the trade-side limit, §6.3), each with a note naming
   the step's line.
6. Actor `name(…)` texts are fetched with `display_name` (with the row for a table family);
   labels and `m` / `f` values outside selectors (`class=`, `species=`, `sex=`, `slot=`,
   `type=`, `nationality=`, in declarations, steps and input lines) are resolved with
   `enum_value`, in line order; each unknown label adds a `label` note (none without an
   install).
7. Property values (§5.8), in line order and, within a line, token order: the property with
   `property` or `property_by_id` (with an install), then the value's labels with
   `enum_value` (`PropertyKey`), then the form and range check against the kind. Each value
   dropped for a reason other than an unresolved label adds a `property` note
   (`unknown`, `mismatch` or `unsupported`; none without an install, where `pid:` forms are
   taken as written and `@Label` forms stay unresolved silently). Then each look is made
   canonical (§5.8), adding `duplicate` `property` notes and a `look` note when cut.

**Notes** are deterministic, in this order, and go to the canonical log (§10.2): one
`resolve` line per selector in declaration order, then the `label`, `fanout`, `window`,
`property` and `look` notes in line order (within a line, in the order of the tokens they
concern; a look's `duplicate` and cut notes after the notes of its tokens).

The `ResolvedScenario` that `tessera-net` receives holds `DataId`s, rows, locations, the
scripted values and literal text, the enum values of item 6, the resolved names of item 6,
and the property IDs, kinds and canonical looks of item 7 (whose values are the scripted
ones). It holds no property value read from the install, no record content and no other
install text.

**Action labels (P3 revision).** `enum_value` also takes the context `NpcAction`: the key of
the label in the enum mapper the property dictionary names for `ActionPickerUI_Action`
(`npc-windows.md` §5.3). Resolution item 6 resolves the labels of `action(@Label)` texts in
line order with that context; an unresolved one becomes `Text::Literal` of the label without
its `@` and adds a `label` note (attribute `action`, label as written) when an install is
present.

**P5 travel contexts.** `enum_value` also takes the contexts `MapNoteType` (the key of the label
in the enum mapper the property dictionary names for `MapNote_Type`, `map-data.md` §4.2),
`GroupSize` and `DifficultyTier` (the mappers it names for `FellowshipMakerUI_GroupSize` and
`FellowshipMakerUI_DifficultyTier`, `travel-windows.md` §6.4). Resolution item 6 resolves the
labels of `note-type=`, `size=` and `tier=` (steps and input lines) in line order, token order
and, within a list, item order; each unresolved one adds a `label` note whose attribute is the
option key. Item 6 also fetches, for each `markers … named` step in line order, the
`display_name` (with the row) of every element of its selector binding that the step uses
(at most the first 65,536, §8.6 item 5), in binding order; the resolved scenario holds these
names. The `effect` attributes `mount-type` and `war-steed` and the `map-region` family use
`candidates` like every other filter and family.

<a id="s8-7"></a>
### 8.7 No install

With no install (`--no-data`, or no install found and none required), the client passes the
empty resolver: `has_install` false, `candidates` returns `NoInstall`. Every selector
resolves empty; every element is missing; steps that need one are skipped; actors without a
resolvable body spawn as `EntitySpawned`; literal refs and texts work unchanged; property
values written `pid:N:KIND` are taken as written and `@Label` ones stay unresolved (§5.8).
Synthetic scenarios therefore play fully. A scenario with `requires install` fails with
`InstallRequired` (exit 4).

---

<a id="s9"></a>
## 9. Domain events and commands

The new variants extend `Event` and `Command` (both `#[non_exhaustive]`) by **group**, each
group an enum of its own: `Event::Character(CharacterEvent)`, `Event::Inventory(…)`,
`Event::Combat(…)`, `Event::Npc(…)`, `Event::Social(…)`, `Event::World(…)`,
`Event::Progression(…)`, and the same groups for `Command`. Each group enum is itself
`#[non_exhaustive]`, so later milestones add variants without a breaking change, and
`Command::name` returns the inner variant's name. ARCHITECTURE §19 lists them. Log
names (§10) are the inner variant names, unique across groups.

<a id="s9-1"></a>
### 9.1 Shared types

| Type | Definition |
|---|---|
| `ItemId(u64)`, `EffectId(u32)`, `WindowId(u32)`, `MarkerId(u32)` | §5.3 |
| `Ref` | `Record(DataId)` \| `Literal(String)` (1–64 bytes, no control characters) |
| `Text` | `Literal(String)` (≤ 1,024 bytes) \| `Name(DataId)` \| `Description(DataId)` \| `QuestRole { quest: DataId, role: u16, failure: bool }` \| `Comment(DataId)` \| `Objective { quest: DataId, objective: u16 }` |
| `Container` | `Backpack` \| `Equipment` \| `Vault` \| `SharedStorage` |
| `ItemPlace` | `{ container: Container, slot: u16 }` |
| `Currency` | `Coin` \| `Item(Ref)` |
| `Vital` | `Health` \| `Power` \| `Resource(Ref)` (a class resource such as a pip record, `player-character.md` §10.2) |
| `CastKind` | `Cast` \| `Induction` \| `Channel` \| `Craft` |
| `CastOutcome` | `Completed` \| `Interrupted` \| `Cancelled` |
| `DeltaKind` | `Damage` \| `Heal` |
| `HitResult` | `Normal` \| `Critical` \| `Miss` \| `Evade` \| `Parry` \| `Block` \| `Resist` \| `Immune` |
| `Shortcut` | `Skill(Ref)` \| `Item(ItemId)` |
| `QuestStatus` | `Active` \| `Completed` \| `Failed` \| `Removed` |
| `Price` | `{ coin: u64, items: Vec<(Ref, u32)> }` (at most 8 item costs) |
| `Side` | `Own` \| `Peer` |
| `RewardKind` (P3 revision) | `Coin` \| `Glory` \| `ItemXp` \| `CraftXp` \| `MountXp` \| `VirtueXp` \| `TraitPoints` \| `Reputation` \| `Wallet` |
| `Text` (P3 revision) | gains `Action(u32)`: an NPC action key (§5.2) |
| `ChatChannel` (existing) | gains `Fellowship` and `Named { name: String }` |
| `PropertyKind` | `Bool` \| `Int` \| `Int64` \| `Float` \| `Enum` \| `Bitfield32` \| `Bitfield64` \| `Did` \| `String` \| `Instance` \| `Colour` \| `Unsupported` (§5.8) |
| `PropertyValue` | `Bool(bool)` \| `Int(i32)` \| `Int64(i64)` \| `Float(f32)` (finite) \| `Enum(u32)` \| `Bitfield32(u32)` \| `Bitfield64(u64)` \| `Did(DataId)` \| `String(String)` (≤ 256 bytes, no control characters) \| `Instance(u64)` \| `Colour([u8; 4])` (R, G, B, A) |
| `LookEntry` | `{ property: u32, value: PropertyValue }`, `property` ≠ 0, `value` not `Instance`, a `String` 0–128 bytes of U+0021–U+007E |
| `Look` | 1–48 `LookEntry` with strictly ascending `property`, encoded size ≤ 768 bytes (`tessera-protocol.md` §5.3) |
| `Equipped` (existing, `model.rs`) | gains `slot: Option<u8>`: the data's equipment slot key 1 … 64 that holds the item, when known (decision 0025 D3) |
| `Appearance` (existing, `model.rs`) | gains `look: Option<Box<Look>>` (decision 0025 D2); `Appearance::is_valid` also checks the `Look` and slot rules above |
| `RouteMode` (P5 travel) | `Swift` \| `Slow` |
| `TravelFlags` (P5 travel) | `u8`: bit 0 not discovered, bit 1 requirements not met, bit 2 not usable; the other bits 0 |
| `TravelEntry` (existing; P5 travel) | gains, after `coin`: `min_level: Option<u16>`, `flags: TravelFlags`, `route: Option<RouteMode>`, `area: Option<Ref>` |
| `InstanceEntry` (existing; P5 travel) | gains, after `instance`: `level: Option<u16>`, `size: Option<u32>`, `tier: Option<u32>`, `locked: bool` |
| `QueueState` (P5 travel) | `None` \| `Confirm` \| `Queued` \| `OthersConfirm` \| `JoinPending` \| `ConfirmEnterQueue` \| `OthersConfirmEnterQueue` \| `WorldJoinQueue`: the keywords of `instance-queue` (§6.5) in that order. The client half maps `Confirm` … `WorldJoinQueue` to the player-state keys 1 … 7 of `travel-windows.md` §6.5 and `None` to 0; `tessera-net` holds no data key |
| `MountVital` (P5 travel) | `Durability` \| `Morale` \| `Power` \| `Fury` |

Numbers in events are the scripted values: `f32` for vitals and amounts (finite, within the
ranges of §6), integers elsewhere.

<a id="s9-2"></a>
### 9.2 Events

Fields are listed in log order (§10.3).

| Group | Variant | Fields |
|---|---|---|
| Character | `CharacterInfo` | `name: String`, `body: Option<Body>`, `class: Option<u32>`, `level: Option<u32>`, `look: Option<Box<Look>>` |
| | `CharacterRemoved` | `name: String` |
| | `PropertyChanged` | `entity: EntityId`, `property: u32`, `value: Option<PropertyValue>` (none: cleared) |
| | `CharacterRefused` | `name: String`, `reason: Text` |
| | `LevelChanged` | `entity: EntityId`, `level: u32` |
| Inventory | `ContainerSize` | `container: Container`, `slots: u16` |
| | `ItemAdded` | `item: ItemId`, `what: Ref`, `at: ItemPlace`, `quantity: u32` |
| | `ItemMoved` | `item: ItemId`, `at: ItemPlace` |
| | `ItemQuantity` | `item: ItemId`, `quantity: u32` |
| | `ItemRemoved` | `item: ItemId` |
| | `ItemCondition` | `item: ItemId`, `current: u32`, `max: u32` |
| | `WalletChanged` | `currency: Currency`, `amount: u64` |
| | `ItemProperty` | `item: ItemId`, `property: u32`, `value: Option<PropertyValue>` (none: cleared) |
| Combat | `TargetChanged` | `entity: EntityId`, `target: Option<EntityId>` |
| | `VitalChanged` | `entity: EntityId`, `vital: Vital`, `current: f32`, `max: f32` |
| | `CombatState` | `entity: EntityId`, `in_combat: bool` |
| | `QuickslotChanged` | `slot: u16`, `content: Option<Shortcut>` |
| | `CastStarted` | `entity: EntityId`, `cast: CastKind`, `what: Ref`, `target: Option<EntityId>`, `duration_ms: u32` |
| | `CastDelayed` | `entity: EntityId`, `added_ms: u32` |
| | `CastEnded` | `entity: EntityId`, `outcome: CastOutcome` |
| | `CooldownStarted` | `what: Ref`, `duration_ms: u32` |
| | `CooldownEnded` | `what: Ref` |
| | `EffectApplied` | `entity: EntityId`, `effect: EffectId`, `what: Ref`, `duration_ms: Option<u32>`, `stacks: u16`, `source: Option<EntityId>` |
| | `EffectRemoved` | `entity: EntityId`, `effect: EffectId` |
| | `VitalDelta` | `source: Option<EntityId>`, `target: EntityId`, `vital: Vital`, `kind: DeltaKind`, `amount: f32`, `result: HitResult`, `what: Option<Ref>`, `damage_type: Option<u32>` |
| | `Defeated` | `entity: EntityId`, `by: Option<EntityId>` |
| | `Revived` | `entity: EntityId` |
| Npc | `DialogueShown` | `window: WindowId`, `speaker: Option<EntityId>`, `text: Text`, `options: Vec<{ option: u8, text: Text }>` |
| | `QuestOffered` | `window: WindowId`, `quest: Ref`, `giver: Option<EntityId>` |
| | `QuestState` | `quest: Ref`, `state: QuestStatus` |
| | `QuestObjective` | `quest: Ref`, `objective: u16`, `count: u32`, `total: u32`, `done: bool` |
| | `QuestTracked` | `quest: Ref`, `tracked: bool` |
| | `VendorShown` | `window: WindowId`, `vendor: Option<EntityId>`, `entries: Vec<{ entry: u16, what: Ref, price: Price, stock: Option<u32> }>` |
| | `LootShown` | `window: WindowId`, `source: Option<EntityId>`, `entries: Vec<{ entry: u16, what: Ref, quantity: u32 }>`, `coin: u64` |
| | `LootTaken` | `window: WindowId`, `entry: u16` |
| | `WindowClosed` | `window: WindowId` (every window kind) |
| | `QuestRewards` (P3 revision) | `window: WindowId`, `entries: Vec<{ kind: RewardKind, amount: i64, what: Option<Ref> }>` (1 … 16 entries; `what` present exactly for `Reputation` and `Wallet`) |
| | `BuybackShown` (P3 revision) | `window: WindowId`, `entries: Vec<{ entry: u16, what: Ref, quantity: u32, price: Price }>` |
| Social | `ChannelJoined` / `ChannelLeft` | `channel: String` |
| | `FellowshipChanged` | `members: Vec<{ name: String, entity: Option<EntityId>, leader: bool }>` (empty: no fellowship) |
| | `FellowshipInvite` | `from: String` |
| | `TradeShown` | `window: WindowId`, `peer: EntityId` |
| | `TradeOffer` | `window: WindowId`, `side: Side`, `entries: Vec<{ what: Ref, item: Option<ItemId>, quantity: u32 }>`, `coin: u64` |
| | `TradeReady` | `window: WindowId`, `side: Side`, `ready: bool` |
| | `FriendList` | `entries: Vec<{ name: String, online: bool }>` |
| | `IgnoreList` | `names: Vec<String>` |
| | `PerformanceStarted` | `entity: EntityId`, `tune: String`, `delay_ms: u32` |
| | `PerformanceStopped` | `entity: EntityId` |
| | `PerformanceReady` (P4 social) | `member: String`, `entity: Option<EntityId>`, `tune: String` |
| | `PerformanceSyncStart` (P4 social) | `by: Option<String>`, `at_ms: u64`, `lead_ms: u32` |
| | `PerformanceWithdrawn` (P4 social) | `member: String` |
| | `FellowVitalChanged` (P4 social) | `member: String`, `vital: Vital`, `current: f32`, `max: f32` |
| World | `MarkerAdded` | `marker: MarkerId`, `location: Location`, `label: Text`, `icon: Option<DataId>`, `quest: Option<Ref>` |
| | `MarkerRemoved` | `marker: MarkerId` |
| | `TravelShown` | `window: WindowId`, `source: Option<EntityId>`, `entries: Vec<{ entry: u16, destination: Ref, coin: u64 }>` |
| | `TravelStarted` | `destination: Ref`, `duration_ms: u32` |
| | `MountChanged` | `entity: EntityId`, `mount: Option<Ref>` |
| | `InstanceListShown` | `window: WindowId`, `entries: Vec<{ entry: u16, instance: Ref }>` |
| | `InstanceChanged` | `instance: Option<Ref>` |
| Progression | `ExperienceChanged` | `current: u64`, `needed: u64` |
| | `SkillLearned` / `SkillForgotten` | `what: Ref` |
| | `TrainerShown` | `window: WindowId`, `trainer: Option<EntityId>`, `entries: Vec<{ entry: u16, what: Ref, coin: u64, level: u32 }>` |
| | `TraitPoints` | `available: u32`, `total: u32` |
| | `TraitSlotted` | `slot: u16`, `what: Option<Ref>`, `rank: u16` |
| | `ProfessionChanged` | `profession: Ref`, `tier: u16`, `proficiency: u32`, `proficiency_max: u32`, `mastery: u32`, `mastery_max: u32` |
| | `RecipeLearned` | `what: Ref` |
| | `CraftResult` | `recipe: Ref`, `outcome: Success \| Critical \| Failed`, `item: Option<ItemId>` |
| | `ReputationChanged` | `faction: Ref`, `points: i64`, `tier: u16` |
| | `TitleGranted` | `what: Ref` |
| World (P5 travel) | `MarkerAdded` (existing) | gains `note_type: Option<u32>` (a map-note type key 1 … 64) after `quest` |
| | `TravelShown` (existing) | its entries carry the `TravelEntry` fields of §9.1 |
| | `TravelStarted` (existing) | gains `route: Option<RouteMode>` after `duration_ms` |
| | `TravelEnded` | — (no field) |
| | `MountChanged` (existing) | gains `combat: Option<bool>` after `mount` |
| | `MountVitalChanged` | `entity: EntityId`, `vital: MountVital`, `current: f32`, `max: f32` |
| | `InstanceListShown` (existing) | gains `source: Option<EntityId>` and `featured: Option<Ref>` after `entries`; its entries carry the `InstanceEntry` fields of §9.1 |
| | `InstanceQueueChanged` | `state: QueueState`, `instance: Option<Ref>`, `seconds: Option<u32>` |

<a id="s9-3"></a>
### 9.3 Existing events reused

| Existing event | Produced by |
|---|---|
| `Connected` | the sandbox, with the scenario's character list (§5.1) |
| `WorldEntered`, `LocationReport` (`Entered`), `AppearanceChanged` | the sandbox's world entry (§1), in that order; with a scenario it adds no wanderers, crowd or welcome line (§3.2) |
| `EntityCreated` / `EntitySpawned` | `spawn` (§5.4) |
| `EntityMoved` | `move`, `face`; the player after `teleport` |
| `EntityDespawned` | `despawn` |
| `AppearanceChanged` | equipment changes of the player (§5.5) |
| `LocationReport` (`Teleported`) | `teleport` |
| `ChatMessage` | `say`, `system`, the generic chat answer (§7.6) |
| `StaticSpawned`, `StaticsCleared` | the sandbox's statics streaming, unchanged and independent of the scenario |
| `Disconnected` | `ServerClosed("scenario error: …")` on a runtime error (§11.3); the usual answer to `Disconnect` |

<a id="s9-4"></a>
### 9.4 Commands

| Group | Variant | Fields |
|---|---|---|
| Character | `CreateCharacter` | `name: String`, `appearance: Option<Box<Appearance>>` (with the look, if any), `class: Option<u32>`, `nationality: Option<u32>` (the creation screen fills all three; the input script may leave them out, §7.7) |
| | `DeleteCharacter` | `name: String` |
| Inventory | `MoveItem` | `item: ItemId`, `to: ItemPlace` |
| | `EquipItem` | `item: ItemId`, `slot: Option<u16>` |
| | `UnequipItem` | `item: ItemId` |
| | `SplitStack` | `item: ItemId`, `quantity: u32` |
| | `DestroyItem` | `item: ItemId` |
| | `UseItem` | `item: ItemId`, `target: Option<EntityId>` |
| Combat | `SelectTarget` | `target: Option<EntityId>` |
| | `UseSkill` | `what: Ref`, `target: Option<EntityId>` |
| | `CancelCast` | — |
| | `SetQuickslot` | `slot: u16`, `content: Option<Shortcut>` |
| Npc | `UseEntity` | `entity: EntityId` |
| | `ChooseOption` | `window: WindowId`, `option: u8` |
| | `CloseWindow` | `window: WindowId` |
| | `AnswerQuest` | `window: WindowId`, `accept: bool` |
| | `AbandonQuest` / `TrackQuest` | `quest: Ref` / `quest: Ref`, `tracked: bool` |
| | `Buy` | `window: WindowId`, `entry: u16`, `quantity: u32` |
| | `Sell` | `window: WindowId`, `item: ItemId`, `quantity: u32` |
| | `TakeLoot` | `window: WindowId`, `entry: Option<u16>` (none = all) |
| | `BuyBack` (P3 revision) | `window: WindowId`, `entry: u16` |
| Social | `JoinChannel` / `LeaveChannel` | `name: String` |
| | `InviteToFellowship` / `KickFromFellowship` / `SetFellowshipLeader` | `name: String` |
| | `AnswerInvite` | `accept: bool` |
| | `LeaveFellowship` | — |
| | `OpenTrade` | `entity: EntityId` |
| | `OfferTrade` | `window: WindowId`, `item: Option<ItemId>`, `quantity: u32`, `coin: u64` |
| | `SetTradeReady` | `window: WindowId`, `ready: bool` |
| | `AddFriend` / `RemoveFriend` / `Ignore` / `Unignore` | `name: String` |
| | `ReadyPerformance` (P4 social) | `tune: String` (1–256 bytes, no control characters) |
| | `StartPerformance` / `DisbandFellowship` (P4 social) | — |
| World | `ChooseTravel` / `JoinInstance` | `window: WindowId`, `entry: u16` |
| | `LeaveInstance` / `Dismount` | — |
| Progression | `TrainSkill` | `window: WindowId`, `entry: u16` |
| | `SlotTrait` | `slot: u16`, `what: Option<Ref>` |
| | `Craft` | `recipe: Ref`, `quantity: u16` |
| World (P5 travel) | `JoinInstance` (existing) | gains `size: Option<u32>`, `tier: Option<u32>`, `level: Option<u16>` after `entry` |
| | `LeaveQueue` | — |

The existing `Chat` command carries the new channels. Mounting is `UseSkill` of a mount
skill.

<a id="s9-5"></a>
### 9.5 Session rules for the new variants

- **States.** Character events (`CharacterInfo`, `CharacterRefused`, `CharacterRemoved`,
  `LevelChanged`) are surfaced in `Connected`, `EnteringWorld` and `InWorld`, and in `Closing`
  by the chat rule (surfaced when the session was `Connected` or `InWorld` before
  `Disconnect`, stale otherwise); every other new event, `PropertyChanged` and
  `ItemProperty` included, follows the existing entity rule (surfaced in `InWorld`, stale in
  `Closing` after entry, invalid before). Character commands are allowed in `Connected`;
  every other new command in `InWorld`.
- **Validation of commands** (`NetError::InvalidCommand` otherwise): names pass
  `is_valid_name` (the `Named` chat channel's name included); `Ref::Literal` 1–64 bytes
  without control characters; quantities 1 … 10^9 where a command names an item or entry
  (`OfferTrade` without an item: 0); `Price` item lists ≤ 8; a `CreateCharacter` appearance,
  when present, valid (`Appearance::is_valid`, which covers the look and the slots, §9.1).
- **Cleaning of events**: as for existing text, inbound strings are cut and cleaned
  (`MAX_CHAT_LEN` for `Text::Literal` and chat, `MAX_NAME_LEN` for names and literal refs,
  256 bytes for `PropertyValue::String`); non-finite vitals, amounts, locations and property
  floats, and a look or slot that breaks §9.1, make the event invalid (dropped with a
  warning, like other invalid events).
- **Client-side bound.** The client half keeps the properties reported by `PropertyChanged`
  and `ItemProperty`, at most 1,024 per entity and 1,024 per item; a report that would add a
  further property is ignored with a warning (rate-limited), a report for a property already
  kept replaces it, and `value` none removes it. They are dropped with their entity or item.
- **P3 revision variants.** `QuestRewards` and `BuybackShown` follow the entity rule (surfaced
  in `InWorld`); `BuyBack` is allowed in `InWorld` and needs `entry` ≥ 1. An event is invalid
  (dropped with a warning) when a `QuestRewards` has 0 or more than 16 entries, an entry's
  `what` does not match its kind, or an amount lies outside the step's range (§6.3); a
  `BuybackShown` entry follows the `VendorShown` rules. `Text::Action` needs no cleaning. The
  test-server backend refuses `BuyBack` like the other new commands. The feed gives no
  generic answer to `BuyBack` (§7.6).
- **P5 travel variants.** `TravelEnded`, `MountVitalChanged` and `InstanceQueueChanged`
  follow the entity rule (surfaced in `InWorld`); `LeaveQueue` is allowed in `InWorld`. A
  `JoinInstance` whose `level` is present must have it 1 … 10,000 (`InvalidCommand`
  otherwise). Literal refs in `TravelEntry.area`, `InstanceListShown.featured` and
  `InstanceQueueChanged.instance` are cleaned like other literal refs. An event is invalid
  (dropped with a warning) when a `MarkerAdded.note_type` lies outside 1 … 64, a
  `TravelFlags` has a bit above 2, a `min_level` or an instance entry's `level` is above
  10,000, or a `MountVitalChanged` number is not finite. The test-server backend refuses
  `LeaveQueue` like the other new commands (it already refuses `JoinInstance`), never
  produces the P5 events and needs no protocol minor (LEGAL.md §11.1 rule 5: its server
  runs no gameplay). The feed gives no generic answer to `LeaveQueue` (§7.6). An
  `InstanceQueueChanged` with state `None` carries no instance and no seconds when the feed
  sends it from `instance-queue none` without them; another backend may send them, and the
  client half ignores them for `None`.
- **Other backends.** The test-server backend follows §9.7: it takes `CreateCharacter` under
  `character-screens.md` §7.1.1, answers `SelectTarget` locally without sending anything
  (revision 6, decision 0032 D1), and refuses every other new command with
  `NetError::InvalidCommand` (rule 5: its server has no gameplay). It also refuses `Chat` on
  the new channels `Fellowship` and `Named` with `NetError::InvalidCommand` until a minor
  version of `tessera-protocol.md` gives them a wire form (rule 5 allows chat). The sandbox
  without a scenario answers `CreateCharacter` (§9.6) and accepts and ignores the other new
  commands (debug log), so client panels can be opened offline. The overlay backend of
  §12.5 routes the Inventory group to the feed and everything else to the test-server
  backend.
- **P4 social** (decision 0035). The events of §6.7 follow the entity rule (surfaced
  `InWorld`); `ReadyPerformance`, `StartPerformance` and `DisbandFellowship` are allowed
  `InWorld`. Validation: `ReadyPerformance.tune` 1–256 bytes without control characters;
  inbound `member` and `by` are names (cut and cleaned to `MAX_NAME_LEN`), `tune` 256 bytes;
  a non-finite `FellowVitalChanged` value makes the event invalid. On the test-server backend
  at protocol minor 2, `JoinChannel`, `LeaveChannel` and `Chat` on `Named` are accepted
  (§9.8); the sentence above about `Named` applies below minor 2. The plain sandbox answers
  `StartPerformance` (§9.8).

> **OPEN:** (non-blocking; for the spec owner, raised by the P3 implementer of `crates/net`
> after review, 2026-10-10) The parser refuses a repeated single reward kind (`coin`,
> `glory`, `item-xp`, `craft-xp`, `mount-xp`, `virtue-xp`, `trait-points` each at most once,
> §6.3, N-H5), but the validity rule above does not say whether a `QuestRewards` from another
> backend with two entries of one single kind (two `Coin` entries, say) is invalid. The
> implementation follows the text as written and surfaces it; `npc-windows.md` §10.3 assumes
> one money row and one row per experience kind. Decide whether a repeated single kind makes
> the event invalid.

<a id="s9-6"></a>
### 9.6 Plain sandbox: creating a character (revision 4, decision 0025 D5)

Without a scenario the sandbox answers `CreateCharacter` itself, in `submit`, with a
Tessera acknowledgement that applies no game rule (decision 0024 §2.4):

1. If the command's name equals (byte for byte) a name of the sandbox's character list:
   `CharacterRefused { name, reason: Text::Literal("A character with this name already
   exists.") }`.
2. Otherwise, if the list holds 64 names: `CharacterRefused { name, reason:
   Text::Literal("The character list is full.") }`.
3. Otherwise the name is appended to the list and the sandbox emits
   `CharacterInfo { name, body: the appearance's body, class: the command's class, level:
   None, look: the appearance's look }` (body and look none when the command carried no
   appearance). The sandbox remembers the command's appearance for that name.

On `EnterWorld` with such a name, the player's base appearance is the remembered appearance
(look included) instead of the client's configured avatar; without a remembered appearance
the configured avatar applies as before. The list and the appearances live in memory for
the sandbox's lifetime only. `DeleteCharacter` stays accepted and ignored (the screens keep
the delete button ghosted on this backend, `character-screens.md` §7.2). With a scenario the
feed owns character answers (§7.5) and this rule does not run. Conformance: §14.10.

<a id="s9-7"></a>
### 9.7 Test-server backend (revision 4, decision 0025 D2, D4; revision 6, decision 0032 D1)

The normative rule for `CreateCharacter` on the test-server (`remote`) backend is
`character-screens.md` §7.1.1 (capability, command, `CharacterList` and
`CharacterCreateFailed` answers, states); `tessera-protocol.md` Appendix A maps it. In
addition:

- **Look and nationality.** When the negotiated minor version is 1 or higher
  (`tessera-protocol.md` §3.1) the backend sends the appearance's look with `CharacterCreate`
  and maps the `looks` of `LoginOk` and `CharacterList` and the `look` of `WorldEnter`,
  `EntityCreate` and `EntityAppearance` into the events' `Appearance.look` and
  `CharacterInfo.look`. At minor 0 it sends no look (the creation screen then keeps the
  choices in its own settings, `character-screens.md` §7.1) and every received look is
  none. `class` and `nationality` are never sent (`server-content.md` §7).
- **Held-item slots.** At minor 1 or higher `Equipped.slot` travels both ways; at minor 0 the
  backend sends no slot and receives none.
- **Character list at login.** After `Connected` (from `LoginOk`) the backend emits one
  `CharacterInfo { name, body: Some(the entry's body), class: None, level: None, look: the
  entry's look }` per entry, in the list's order, so the selection screen can show each
  character's figure.
- **Local selection** (revision 6, [decision 0032](../decisions/0032-p2-targeting-on-test-server.md)
  D1). In `InWorld` the backend answers `SelectTarget { target }` in `submit` and sends
  **nothing** to the server, at any minor version:
  1. The target is **known** when it is none, the player's entity, or an entity whose
     creation the session has delivered and whose removal it has not: a remote entity
     (`EntityCreated` delivered, no `EntityDespawned` since; `InterestReset` delivers a
     despawn for each) or a streamed static (`StaticSpawned` delivered, no `EntityDespawned`
     or `StaticsCleared` since). The check uses the events already handed to the session,
     not messages received and not yet mapped.
  2. For a known target the backend queues `TargetChanged { entity: the player, target }`.
     Queued answers are delivered **first** in the next poll, before every event the backend
     maps in that poll. So a `TargetChanged` naming an entity always lies between that
     entity's creation and its removal in the event stream.
  3. At most one answer is pending: a later `SelectTarget` with a known target before the
     next poll replaces it.
  4. Any other target: `Ok(())`, no event, a rate-limited debug log line; a pending answer
     stays.
  5. The backend keeps no selection and never emits `TargetChanged` on its own (not on a
     despawn, `InterestReset`, teleport, appearance change or world entry), and never for an
     entity other than the player.
  6. Outside `InWorld`, `SelectTarget` is `NetError::InvalidState` (§9.5 states).
  The client half clears a selection whose entity is removed by itself, sending no command
  (decision 0032 D2); that rule is the same on every backend.
- `DeleteCharacter`, `UseSkill`, `CancelCast`, `SetQuickslot` and every other new command:
  refused as in §9.5.

> **OPEN:** (implementer of `crates/net`, clean room, 2026-10-10; non-blocking) Item 1's
> "known" is the backend's view of the events it handed to `Session`. `Session` may still drop
> a handed `EntityCreated` or `StaticSpawned` as invalid (§9.5 event checks), which the
> backend cannot see; such an entity stays known to the backend, so a `TargetChanged` naming
> it could surface although its creation never did (invariant 11's "between creation and
> removal" then holds only for the handed stream). The mapper builds those events from
> protocol-validated messages and checked statics, so this is not expected to happen; the
> implementation follows the text as written. Should the backend apply `Session`'s checks
> itself, or should invariant 11 be read on the handed stream? Also: "static 900" of §14.11
> is a label; a streamed static's ID comes from the backend's local range
> (`tessera-protocol.md` §9.1), so the conformance tests use the ID its `StaticSpawned`
> carried, and R8's statics are cleared by a region change, which needs `InterestReset` first
> (§9.5 of the protocol), so 43 and 44 are despawned in that case too.

<a id="s9-8"></a>
### 9.8 P4 social: the test-server backend at minor 2 and the plain sandbox (decision 0035 D1, D4)

**Test-server backend.** At negotiated minor 2 or higher with `CHAT_CHANNELS` advertised
(`tessera-protocol.md` §3.1, §6.2.2, §10.6, Appendix A):

- `JoinChannel { name }` sends `ChannelJoin`, `LeaveChannel { name }` sends `ChannelLeave`,
  and `Chat { channel: Named { name } }` sends `ChatSend` with channel 6.
- `ChannelJoined` and `ChannelLeft` from the server surface as `ChannelJoined { channel }` and
  `ChannelLeft { channel }` (§9.2) with the server's spelling of the name; a `ChatMessage` on
  channel 6 surfaces as `ChatMessage` on `ChatChannel::Named { name }`; `ChannelRefused`
  surfaces as a System `ChatMessage` in Tessera's own words (Appendix A there).
- Below minor 2, or without `CHAT_CHANNELS`: the three commands are refused with
  `NetError::InvalidCommand` and nothing is sent.
- At every minor: `Chat` on `Fellowship` is refused (no wire value; a fellowship exists only
  in the feed), and so is every other Social command, `ReadyPerformance`, `StartPerformance`
  and `DisbandFellowship` included (decision 0035 D2: synchronised-music relay is the
  owner's open question). The client then keeps its local start for `/playstart` (§6.7).
- The overlay backend (§12.5) keeps its P1 subset (decision 0035 D3): Social-group
  statements stay `NotAllowedInOverlay`, and Social commands of an overlay run, chat and
  channel commands included, go to the test-server backend under these rules.

**Plain sandbox** (no scenario): `StartPerformance` is answered in `submit` with
`PerformanceSyncStart { by: Some(the entered character's name), at_ms, lead_ms: 700 }`,
where `at_ms` is the milliseconds since world entry by the `now` of that `submit`, rounded
down, plus 700. It keeps no waiting parts; `ReadyPerformance`, `DisbandFellowship`,
`JoinChannel` and `LeaveChannel` stay accepted and ignored (§9.5). Conformance: §14.13.

---

<a id="s10"></a>
## 10. Determinism and the canonical event log

<a id="s10-1"></a>
### 10.1 Guarantee

In a headless run (§12.2), the same scenario file bytes, the same install contents (or no
install) and the same client options give the same canonical log, byte for byte, on every
run. To keep this:

- the headless driver supplies `now` = t0 + *k* · tick for frame *k* (and the pre-world
  values of §12.2) and never reads the wall clock for the feed; virtual times in the log are
  relative to t0, so how many pre-world polls were needed does not show;
- no output depends on hash-map iteration order, thread timing, addresses or the order in
  which loader threads finish (resolution results are sorted, §8.4);
- floats are formatted by §10.3;
- events of the sandbox's statics streaming are excluded from the log (they depend on loader
  timing and budgets, not on the scenario).

The guarantee covers runs whose spawn resolves without a pending `WorldSource` lookup: no
spawn option (a headless scenario run then spawns at the exact default of §3.2, with or
without an install), or an exact `--spawn R,X,Y,x,y,z`. A spawn that snaps to the ground or
names a place (`--place`, `--spawn` without *z*) waits for loader threads and may time out
and keep a different height (with a system notice) between runs; scenario runs SHOULD use an
exact spawn or none. The `teleport` step never waits (§6.5). Headless scenario runs send no
`Move` and no `QueryLocation` (§12.2), so no timing-dependent sandbox correction or location
report reaches the log.

<a id="s10-2"></a>
### 10.2 Log structure

`tessera-client --scenario FILE --scenario-log OUT` writes UTF-8 text with LF line ends (`-`
for standard output):

1. `tessera-scenario-log 1`
2. `install none` or `install present`
3. `tick N` (milliseconds)
4. `title STRING` if the scenario has a title
5. Resolution notes (§8.6), one per line:
   `resolve $NAME line=L family=F requested=(N|all) candidates=C got=G [note=(no-install|unsupported|truncated|short)]`,
   then `label line=L attribute=A label=@X`, `fanout line=L cut=N`, `window line=L cut=N`,
   `property line=L property=P note=(unknown|mismatch|unsupported|duplicate)` (*P* the
   property as written: `@Name`, or `pid:0x` and 8 upper-case hexadecimal digits) and
   `look line=L cut=N`.
   *C* is the number of candidates after sorting, deduplication and the cap; *G* the length
   of the stored list; *N* in a cut note the number of elements left out; `attribute` is the
   filter attribute, or the option key (`class`, `species`, `sex`, `slot`, `type`,
   `nationality`) outside selectors, `property` for the property label of a property value
   or `value` for a label inside its value (§5.8), and `label` is the label as written (`@Name`, or `m` / `f`). At most one
   `note=` is written, the first that applies in the order `no-install`, `unsupported`,
   `truncated`, `short` (`short`: *G* < the requested *N*); `family=did` for the `did`
   family.
6. Command and event lines in the order the client sent and received them:
   - `TIME > COMMAND fields…` for each command the client sent (excluding `Move` and
     `QueryLocation`, which carry walker noise; headless runs send neither, §12.2, so only
     windowed logs, which are not normative, ever leave them out);
   - `TIME EVENT fields…` for each event the client received, excluding `StaticSpawned`,
     `StaticsCleared`, `EntityCreated` with a tag and `EntityDespawned` of tagged or static
     ids.
7. `TIME end reason=(complete|horizon|error) skipped=N input-skipped=N dropped=N spilled=N`
   (`error` adds `error=KIND line=L`; `line=0` when no scenario line caused it, for example
   `error=Disconnected` for a session that ended before `Finished`, or `error=LogWrite`).
   Nothing is logged after the end line, so the events that answer the driver's
   `Disconnect` never appear.

`TIME` is `pre` before world entry, then the virtual time of the poll that delivered the
event (or that processed the command) in seconds with exactly three decimals (`0.000`,
`12.350`).

<a id="s10-3"></a>
### 10.3 Field formatting

Each line is the name followed by `field=value` items separated by one space, in the field
order of §9.2 / §9.4 (existing events: the order below). Every field is always written;
an absent optional value is `none`.

| Value | Written |
|---|---|
| Entity | `player`, `$name` for actors, `entity:N` (decimal) otherwise |
| Item | `$name` for declared items, `item:N` for run-time items |
| Window, effect, marker ids | decimal |
| `DataId` | `0x` and 8 upper-case hexadecimal digits |
| `Ref` | a `DataId`, or a quoted string |
| `Text` | a quoted string, `name(0x…)`, `desc(0x…)`, `role(0x…,N)`, `role-failure(0x…,N)`, `comment(0x…)`, `objective(0x…,N)` |
| String | `"` … `"` with `\"`, `\\` and `\u{H}` (lower-case hex) for any character outside printable ASCII and space |
| Integer | decimal |
| `f32` amount, vital, scale | the shortest decimal that reads back to the same `f32` (no exponent, at least one digit, no trailing `.0`; `−0` written `0`) |
| Position (`Vec3`, `f32`) | `x,y,z`, each rounded to 3 decimals, ties to even on the exact binary value, `−0.000` written `0.000` |
| `Location` | `R:x,y,z`, region decimal, each `f64` coordinate rounded to 3 decimals, ties to even on the exact binary value, `−0.000` written `0.000` |
| Heading | `compass_degrees_from_heading(h)` rounded to 2 decimals, ties to even, always 2 decimals; a value that rounds to `360.00` is written `0.00` (for example a declared `facing=359.999`, Vector F) |
| Boolean | `yes` / `no` |
| Enum values of §9.1, `EntityKind`, `LocationCause` | lower case, words joined by `-` (`non-player`, `critical`, `quest-offer` …), `Vital::Resource(r)` as `resource:REF`, `Shortcut` as `skill:REF` / `item:ITEM`, `Currency` as `coin` / `item:REF`, `ItemPlace` as `CONTAINER:SLOT` (`backpack:0`, `equipment:12`, `shared:3`) |
| `Body` | `character:SPECIES:SEX` or `object:0x…` |
| `Appearance` in a command (`CreateCharacter`) | its `Body` form; `none` when absent (its look is written as the command's `look` field) |
| Property ID | `0x` and 8 upper-case hexadecimal digits |
| `PropertyValue` | `bool:yes` / `bool:no`, `int:N`, `int64:N`, `float:F` (as an `f32` amount), `enum:N`, `bitfield32:0x` and 8 upper-case hexadecimal digits, `bitfield64:0x` and 16, `did:0x…` (a `DataId`), `string:"…"`, `instance:N`, `colour:R,G,B,A` (decimal); an absent value `none` |
| `Look` | `[` entries `{property=0x… value=…}` separated by `,` `]` in ascending property order; an absent look `none` |
| `Price` | `coin:N` followed by `+REF*N` per item cost |
| Lists | `[` items separated by `,` `]`; a structured item is `{field=value …}` with single spaces |

Existing events: `Connected characters=[…]`; `WorldEntered entity name at facing`;
`EntitySpawned entity kind name at facing`; `EntityCreated entity kind name class at facing
body scale equipment idle look`; `EntityMoved entity at facing velocity`; `AppearanceChanged
entity body scale equipment idle look`; `LocationReport entity at facing anchor cause` —
written in that order, where `at` is a position (`Transform`) or `Location` as the event
holds, `anchor` is `R:X:Y:CELL` (all decimal) or `none`, `equipment` a list of `0x…`, each
followed by `@SLOT` (decimal) when the entry has a slot (dyes and stowing omitted), `idle` a
decimal or `none`, `look` as above (revision 4 added `look` and `@SLOT`); `EntityDespawned entity`; `ChatMessage channel sender
text` with `channel` one of `say`, `yell`, `emote`, `system`, `fellowship`, `tell:"…"`,
`named:"…"`; `Disconnected reason` with `reason` one of `client-requested`,
`server-closed:"…"`, `timed-out`, `transport-failed:"…"`. Existing commands: `Connect`,
`EnterWorld character`, `Teleport destination`, `Chat channel text`, `Disconnect`, where
`destination` is `location(LOC,H)`, `ground(LOC,H)`, `anchored(LOC,H,R:X:Y:CELL)` or
`named("…")`, *LOC* a `Location` and *H* a heading as above. `CreateCharacter` is written
`name appearance class nationality look`, `look` being the look of its appearance (`none`
without an appearance or a look). A variant added to
`Event`, `Command` or one of their groups later MUST be given a log form in this table by the
revision that adds it; until then the log writer writes its name followed by `unlogged`.
(P4 social) The variants of §6.7 and §9.8 use the general rule of this section with the
value forms above: `PerformanceReady member entity tune`, `PerformanceSyncStart by at_ms
lead_ms`, `PerformanceWithdrawn member`, `FellowVitalChanged member vital current max`,
`ReadyPerformance tune`, `StartPerformance` and `DisbandFellowship` (no fields); `at_ms` and
`lead_ms` decimal (Vector H, §14.12).

**P3 revision log forms.** `Text::Action(n)` is written `action(N)` (*N* decimal);
`RewardKind` values lower case with words joined by `-` (`coin`, `glory`, `item-xp`,
`craft-xp`, `mount-xp`, `virtue-xp`, `trait-points`, `reputation`, `wallet`); the reward
`amount` is a signed decimal integer. `QuestRewards`, `BuybackShown` and `BuyBack` write their
fields in the order of §9.2 and §9.4 (Vectors N1, N2).

**P5 travel log forms.** New variants write every field, in the order of §9.2 and §9.4:
`TravelEnded` and `LeaveQueue` are their names alone; `MountVitalChanged entity vital current
max` with `vital` one of `durability`, `morale`, `power`, `fury` and the numbers as `f32`
amounts; `InstanceQueueChanged state instance seconds` with `state` the `instance-queue`
keyword of §6.5 and `seconds` decimal or `none`. **Fields that this revision adds to existing
variants and entries are written only when they are not at their default** (none for an
optional value, 0 for `flags`, `no` for `locked`): an exception to "every field is always
written", so that every log written before it, such as the committed `world.log`, stays
byte for byte the same. They follow the existing fields in the order of §9.1–§9.4:
`MarkerAdded … note_type=N`; `TravelStarted … route=(swift|slow)`; `MountChanged …
combat=(yes|no)`; `InstanceListShown … source=ENTITY featured=REF`; `JoinInstance … size=N
tier=N level=N`; inside a `TravelShown` entry, after `coin`, `min_level=N`, `flags=…`,
`route=…`, `area=REF`; inside an `InstanceListShown` entry, after `instance`, `level=N`,
`size=N`, `tier=N`, `locked=yes`. `flags` is the names of its set bits in bit order
(`undiscovered`, `unmet`, `unusable`) joined by `+`, for example `flags=undiscovered+unusable`
(Vectors T1–T3).

<a id="s10-4"></a>
### 10.4 Counters

| Counter | Counts |
|---|---|
| `skipped` | steps skipped by the feed (missing element, no active cast or instance, no free or usable slot, an item already placed, the effect-instance cap, window not open, a dropped property value, `remove-character` of a name not in the list) |
| `input-skipped` | input lines the driver could not send (§7.7) |
| `dropped` | agenda entries discarded at the end or beyond the horizon |
| `spilled` | events held over by the per-tick cap (§7.4), each counted once, in the tick it was first held over |

(P5 travel) `skipped` also counts a `markers` step skipped by the run-time marker cap
(§6.5, §11.1).

<a id="s10-5"></a>
### 10.5 Which logs may be committed

A canonical log may be committed only when its header says `install none` and it is the log
of a committed synthetic scenario (§13). Logs of runs with an install are local: they list
record identifiers chosen from the user's install and may contain install text (actor and
place names). CI replays every committed log with `--no-data` and fails on any difference.

---

<a id="s11"></a>
## 11. Hostile input

The parser, the static checks, resolution and the feed treat the file as hostile: no panic on
any input, every allocation bounded by the limits below, and each violation a typed error.

<a id="s11-1"></a>
### 11.1 Limits

| Limit | Value | Error kind |
|---|---|---|
| File size | 1,048,576 bytes (checked before reading further) | `FileTooLarge` |
| Line length | 4,096 bytes | `LineTooLong` |
| Lines | 65,536 | `LimitExceeded(lines)` |
| Tokens per line | 64 | `LimitExceeded(tokens)` |
| String after unescaping | 1,024 bytes; names (player, character, actor, fellow, friend, channel, peer) 64; literal refs 64; title 128; tune names 256 | `TooLong` |
| Keyword, binding | 32 bytes | `TooLong` |
| Label | 128 bytes | `TooLong` |
| Numeric component (§4.2: one number of a token, not the whole token) | 32 characters | `BadNumber` |
| Bindings | selectors 256, actors 1,024, items 4,096, pages 1,024, markers 1,024 | `LimitExceeded(selectors)`, `(actors)`, `(items)`, `(pages)`, `(markers)` |
| Options per page | 16 | `LimitExceeded(options)` |
| Step lines (all blocks) | 20,000 | `LimitExceeded(steps)` |
| Triggers | 256 | `LimitExceeded(triggers)` |
| Input lines | 10,000 | `LimitExceeded(input)` |
| Block nesting | 2 | `NestingTooDeep` |
| Repeat count | 10,000; expanded total 200,000 (§7.3) | `LimitExceeded(repeat)` |
| Trigger `times` | 10,000 | `OutOfRange` |
| `n=` per selector | 65,536 | `LimitExceeded(selector)` |
| `did` list | 256 | `LimitExceeded(did)` |
| Candidates per selector | 1,048,576 (truncated with a note) | — |
| Total resolved records | 1,048,576 | `ResolveLimit` (resolve error) |
| Fan-out per step | 65,536 (cut with a note) | — |
| Window entries | 1,024; fellowship 64; friends and ignore 1,024; trade side 32 | `LimitExceeded(entries)` (static) or cut with a note (from a binding, §8.6 item 5) |
| Tick | 10 … 1,000 ms | `OutOfRange` |
| Time horizon | 60 min (3,600,000 ms) for every time and duration | `OutOfRange` |
| Element index | 0 … 65,535 | `OutOfRange` |
| Numeric ranges | as stated per step in §6; positions ±100,000 m; vitals and amounts 0 … 10^9; coin 0 … 10^15; quantities 1 … 10^9 | `OutOfRange` |
| Events delivered per tick | 4,096 events and 65,536 weight units (held over, §7.4) | — |
| Ticks per poll | 8 (later polls catch up, §7.1) | — |
| Queued events | 1,048,576 events and 2,097,152 weight units, checked on every push (§7.4) | `QueueOverflow` (runtime) |
| Events per run | 16,777,216, pre-world output included | `EventOverflow` (runtime) |
| Active effect instances | 1,024 per entity | the `effect` step is skipped (counted) |
| Pre-world triggers | 16 step lines per body; 64 firings per run (§7.5) | `LimitExceeded(steps)` (static); later commands fire none |
| Agenda entries | 262,144 | `AgendaOverflow` (runtime) |
| Run-time items | 65,536 | `ItemOverflow` (runtime) |
| Unprocessed commands | 1,024 | `submit` returns `NetError::SendQueueFull` (§7.4) |
| Characters in the list | 64 (declared; `accept-character` beyond it is skipped) | `LimitExceeded(characters)` |
| Window entry list (REFLIST) | 1,024 items; 32 for `trade-offer` | `LimitExceeded(entries)` |
| Look bindings | 64 | `LimitExceeded(looks)` |
| Property values per look | 48 | `LimitExceeded(look)` |
| Encoded look | 768 bytes (cut from the highest property ID, with a note, §5.8) | — |
| Property value strings | 256 bytes | `TooLong` |
| Look strings | 0–128 bytes, only U+0021–U+007E | `BadValue` (`pid:` form) or a `mismatch` note (`@Label` form) |
| Property value numbers | per kind (§5.8): float finite with \|v\| ≤ 10^9, integers within the kind | `OutOfRange` (`pid:` form) or a `mismatch` note (`@Label` form) |
| Properties kept by the client half | 1,024 per entity and 1,024 per item (§9.5) | further properties ignored with a warning |
| Overlay `AppearanceSet` | one in flight, at least 250 ms apart, answer within 10 s (§12.5) | coalesced; a missing answer clears the in-flight mark with a warning |
| Reward tokens per `rewards` step (P3 revision) | 1 … 16 | `MissingArgument`, `LimitExceeded(entries)` |
| Reward amounts (P3 revision) | per kind (§6.3) | `OutOfRange` |
| Buy-back entries (P3 revision) | 1,024 (REFLIST) | `LimitExceeded(entries)` (static) or cut with a note (binding) |
| Entry lists (P5 travel: `undiscovered=`, `unmet=`, `unusable=`, `swift=`, `slow=`, `locked=`) | 1 … 1,024 distinct items, each 1 … 1,024 | `LimitExceeded(entries)` beyond 1,024 items; `OutOfRange`; a repeated number, or an entry in both `swift=` and `slow=`, `UnexpectedToken` |
| Value lists (P5 travel: `min-level=`, `level=`, `size=`, `tier=`) | 1 … 1,024 items; `min-level` 0 … 10,000; `level` 1 … 10,000; `size`, `tier` 0 … 2^32 − 1 or labels | `LimitExceeded(entries)`; `OutOfRange`; another item form `UnexpectedToken` |
| `area=` of `travel-list` (P5 travel) | 1 … 1,024 refs | `LimitExceeded(entries)` |
| Map-note type (P5 travel) | 1 … 64 | `OutOfRange` |
| Queue seconds (P5 travel) | 0 … 86,400 | `OutOfRange` |
| Mount vitals (P5 travel) | finite, 0 ≤ CUR ≤ MAX ≤ 10^9 | `OutOfRange` |
| Run-time marker ids (P5 travel) | 65,536 per run (§5.3) | the `markers` step is skipped (counted) |
| (P4 social) Waiting parts | 64 members (§5.7, §6.7) | a `perform … sync` for a 65th member is skipped (counted); `ReadyPerformance` beyond it records nothing |
| (P4 social) `perform-start` lead | 0 … 10 s | `OutOfRange` |
| (P4 social) `ReadyPerformance.tune` (input form `play-sync`) | 1–256 bytes | `TooLong` (static); an empty tune `OutOfRange`; a longer one from the client half: `NetError::InvalidCommand` |

> **OPEN:** (non-blocking; for the spec owner, raised by the documentation review of P0 and
> recorded by the P0 exit verifier, 2026-10-09) No row bounds the size of the canonical log
> (§10). A hostile file within every limit above can still make `--scenario-log FILE` write
> a very large file (for example many fan-out steps over the 60-minute horizon). Decide
> whether a log-size limit belongs here (for example 256 MiB, a typed `LogOverflow`
> run-time error, exit code 5) or whether the USER_GUIDE warning is enough.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of
> `tessera_net::scenario` and `apps/client`, 2026-10-09) Implemented as the review
> recommended, pending this decision: `MAX_LOG_BYTES` = 268,435,456 bytes counts the header,
> note, command and event lines (each with its LF); the end line is outside the limit. A
> line that would pass it is not written, the run stops after that poll (headless) or at
> the next poll (windowed), the end line is `reason=error … error=LogOverflow line=0` and the
> client exits 5. `LogOverflow` is not a `RuntimeErrorKind` of the feed (the feed does not
> fail; only the log does), and §10.2 item 7 and the §12.2 exit table do not name it yet.

> **OPEN:** (non-blocking; for the spec owner, raised by the P4 review of `tessera_net` and
> recorded by its implementer, 2026-10-11) A string may hold a raw TAB (§4.1), but
> `ReadyPerformance.tune` has no control characters (§9.4), and the row above names only
> `TooLong` and `OutOfRange` for the length. Implemented pending this decision: a TAB in the
> tune of the input form `play-sync` or of the pattern `play-sync tune=` is the static error
> `OutOfRange` at the tune's column (before, the line was only counted as input-skipped when
> `Session` refused it). Decide the error kind, and whether `perform … sync` with such a tune
> should be skipped too: today its `PerformanceReady` is emitted (the client half cleans the
> tune to a space, §9.5) while the feed's waiting record keeps the raw tune.

<a id="s11-2"></a>
### 11.2 Static errors

`ScenarioError { kind, line, column }`, displayed as `FILE:LINE:COLUMN: message`. Lines and
columns count from 1; the column counts Unicode scalar values. The position is:

- for `Encoding` and `ControlCharacter`: the offending character (for a byte that is not
  valid UTF-8, the number of scalars before it on the line plus one);
- for `FileTooLarge` and `BadHeader` on an empty file: 1:1; for `LineTooLong`: column 1;
- for `UnterminatedString`: the opening `"`; for `BadEscape`: the `\`;
- for a missing element (`MissingCount`, a missing required option or argument): the first
  token of the statement;
- for `UnclosedBlock`: the block's opening line, column of its first token;
- otherwise: the first character of the offending token (for `key=value`, the key).

| Kind | When |
|---|---|
| `Encoding`, `ControlCharacter`, `FileTooLarge`, `LineTooLong` | §4.1, §11.1 |
| `BadHeader`, `UnsupportedVersion` | §4.3 |
| `UnknownKeyword`, `UnexpectedToken`, `UnknownAttribute`, `UnknownFamily` | §4.5, §8.2 |
| `BadNumber`, `BadDuration`, `BadPosition`, `BadFraction`, `BadEscape`, `UnterminatedString` | §4.2 |
| `TooLong` | §11.1 |
| `UndefinedBinding`, `DuplicateBinding`, `WrongBindingKind` | a binding used before its declaration, declared twice (also a second `player` or a repeated character name, §5.1), or of the wrong kind (an actor where a selector is needed, …) |
| `UnknownCharacter` | `enter` names no character of the file (§4.3) |
| `MissingCount`, `MissingArgument` | §8.2, §6 |
| `DuplicateSetting`, `SettingAfterStep` | §4.3 |
| `UnclosedBlock`, `UnexpectedEnd`, `NestingTooDeep`, `NotAllowedHere` | §4.4, §7.5 |
| `OutOfRange` | §11.1, §7.2 |
| `LimitExceeded(which)` | §11.1 |
| `BadValue` | a property value whose form does not fit its written kind, or that a look does not allow (§5.8) |
| `NotAllowedInOverlay` | a setting, declaration, step, trigger pattern or input line outside the overlay subset, in an overlay run (§12.5.1) |

Resolve errors (`InstallUnreadable`, `InstallRequired`, `ResolveLimit`) carry the line of the
`select` concerned (line 0 for `InstallRequired`).

<a id="s11-3"></a>
### 11.3 Run-time errors

`QueueOverflow`, `EventOverflow` (§7.4), `AgendaOverflow`, `ItemOverflow` and `IdOverflow`
(§5.3) stop the feed at once (the rest of the tick is not executed): it emits
`Disconnected(ServerClosed("scenario error: KIND at line L"))`, its status becomes `Failed`
with the kind and the line of the step that caused it, and the headless client exits 5. The
feed never panics; every arithmetic step on times uses checked operations bounded by the
horizon.

<a id="s11-4"></a>
### 11.4 Fuzzing

Two fuzz targets (appended to `fuzz/Cargo.toml` by the implementer of `crates/net`):

| Target | Input | Asserts |
|---|---|---|
| `net_scenario_parse` | arbitrary bytes as a scenario file | parse and static checks return `Ok` or a typed error; no panic; peak allocation within a fixed multiple of the input size |
| `net_scenario_run` | a scenario file plus bytes that drive a synthetic resolver (candidate counts and DIDs; since revision 4 also the answers of `property`, `property_by_id` and `enum_value`, kinds included); the target also applies the overlay check of §12.5.1 to every file that parses | resolution, then a headless run of at most 72,000 ticks with the input script; no panic, every limit of §11.1 honoured; the queued events never exceed 2,097,152 weight units (asserted after every tick), and the target runs within libFuzzer's default RSS limit (2,048 MB) on any input, which the weight limit and the per-push checks of §7.4 guarantee |

(P5 travel) No new target: the synthetic resolver of `net_scenario_run` also answers the
`enum_value` contexts `MapNoteType`, `GroupSize` and `DifficultyTier`, gives `map-region`
candidates with and without a `Location`, and answers `display_name` for them.

---

<a id="s12"></a>
## 12. Client integration

<a id="s12-1"></a>
### 12.1 Options

| Option | Meaning |
|---|---|
| `--scenario FILE` | Play FILE with the sandbox backend; with `--server`, run it as an overlay over the test-server backend instead (§12.5). Conflicts with `--walk-path`, `--fly-path`, `--duration`, `--smoke`, `--showcase`, `--ui-script` (both would drive the player's commands). Works with `--no-data` (empty resolver), `--game-dir`, `--spawn`, `--place`, `--heading` (not in overlay runs, where the server places the player); `--screenshot` only with an install (§12.2). |
| `--scenario-log FILE` | Write the canonical log (§10) to FILE (`-` = standard output). Requires `--scenario`. |
| `--scenario-input` | Windowed runs: also play the input script. Headless runs always play it. |

`--scenario` belongs to the group of tasks that end a headless run (`headless_task`), so
`--headless --scenario FILE` is valid on its own.

<a id="s12-2"></a>
### 12.2 Headless runs

`tessera-client --scenario FILE --headless [--scenario-log OUT] [--no-data | --game-dir DIR]`:

1. Parse (exit 3 on error, message on standard error with `FILE:LINE:COLUMN`).
2. Resolve (exit 4 on a resolve error). Write the log header and notes.
3. Pre-world, `now` = base (an `Instant` taken once): send `Connect`; poll. Then the
   `at pre` input lines in order, each followed by a poll at the same `now`. Then send
   `EnterWorld` with the `enter` character and poll until `WorldEntered` arrives: wait poll
   *w* = 0, 1, 2, … uses `now` = base + *w* · tick, and between two wait polls the driver
   pauses one tick of wall-clock time so loader threads can answer a pending spawn lookup
   (only `--spawn` without *z* and `--place` make one, §3.2). The resolve timeout in force
   (§3.2: 10 s for the plain sandbox, 300 s for the walker's) therefore elapses in virtual
   time and the wait is bounded: after base + resolve timeout + one tick without `WorldEntered`, or on
   `Disconnected` before it (an unknown character), the driver writes the end line with
   `reason=error error=Disconnected line=0` and exits 5. t0, time 0, is the `now` of the poll
   that delivers `WorldEntered`; the feed processes tick 0 in that poll (§7.1).
4. Frames *k* = 1, 2, …: `now` = t0 + *k* · tick. Send the input lines with
   max(1, ⌈T / tick⌉) = *k* (integer milliseconds), in time order and file order for equal
   times; poll; log. (An input line at time 0 is therefore processed in tick 1.)
5. When the feed's status is `Finished`, write the end line, send `Disconnect`, exit 0. On
   `Failed`, or when `Disconnected` arrives before `Finished`, write the end line with
   `reason=error`, exit 5.

**What runs.** In a headless scenario run the client sends no `Move` and no `QueryLocation`:
the player is driven only by scenario steps and input lines, so no sandbox correction or
location report depends on frame timing.

- **Without an install** (`--no-data`, or none found): the client runs only
  `HeadlessDriver` over the plain sandbox (§12.3): no walker, no window, no GPU. `--screenshot`
  is refused: with `--no-data` as a clap conflict (exit 2); when no install is found at run
  time, the client prints the usage message and exits 2 before parsing the scenario. This is
  the mode of every vector (§14) and of the CI replay (§10.5).
- **With an install**: the walker runs offscreen and renders from the events the driver
  receives; the driver owns the clock and the session. `--screenshot PNG` changes meaning
  under `--scenario`: it captures the frame after the last tick (after `Finished`), once the
  area around the player has loaded or `--settle-timeout` has passed, instead of the spawn point
  once loaded.

| Exit code | Meaning |
|---|---|
| 0 | Finished (`complete` or `horizon`) |
| 1 | Other client failure (existing behaviour) |
| 2 | Command-line usage error (clap) |
| 3 | Parse error |
| 4 | Resolve error |
| 5 | Run-time error (feed failure, log write failure, unexpected disconnect) |

A windowed run reports parse and resolve errors the same way and exits with the same codes
before opening the world.

<a id="s12-3"></a>
### 12.3 The driver outside the client

The driver of §12.2 (clock, input injection, the mirror of quickslots, target and open
windows it needs for input lines, and the log writer) is `tessera_net::scenario::HeadlessDriver`.
The client uses it for `--headless`; the net crate's tests use it with the empty resolver
or a synthetic one, so the conformance vectors of §14 run in `cargo test` without a window,
a GPU or an install.

<a id="s12-4"></a>
### 12.4 Windowed walker

`tessera-client --scenario FILE` (no `--headless`) runs the walker with the sandbox playing
FILE in real time: `now` is the frame time, the feed catches up tick by tick (§7.1), commands
come from the player's input through the client half (and from the input script with
`--scenario-input`). A HUD line shows the scenario state; when it finishes, the world stays
and the player may continue to look around. The feed processes at most 8 ticks per poll
(§7.1); when it lags behind the frame time, the HUD line shows the lag. `--scenario-log` works
but its times follow the wall clock and are not normative.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of `apps/client`,
> 2026-10-09, revised after review the same day) Five client points this section and
> §12.1–§12.2 leave open, implemented conservatively for now: (1) `--scenario-input` in a
> windowed run: the only public player of the input script is `HeadlessDriver` (§12.3),
> which owns the session and the clock, so the windowed client cannot send the input lines;
> it accepts the option (the run's end, §7.3, then waits for the last input time) and logs
> that the windowed input script is not played yet. A consequence: a scenario whose `enter`
> names a character that only an `accept-character` step adds (§4.3) could never enter the
> world windowed (its pre-world `create-character` is never sent), so a windowed run of such
> a file is refused as a usage error (exit 2) with a hint to use `--headless`. Proposal: a
> public input player in `tessera_net::scenario` that, given the virtual time and the events
> received, yields the due commands (the mirror of §7.7). (2) Headless with an install and
> without `--screenshot`: only the driver runs, not the walker offscreen, when the spawn is
> exact or the default (§3.2): a headless walker sends no command (§12.2 "What runs"), its
> frames are seen by nobody and no lookup is pending, so the log is the same apart from (3),
> and no GPU is needed. When the spawn needs a lookup (`--place`, or `--spawn` without a
> height) the walker runs offscreen as with `--screenshot` (it is the only world source
> that answers the lookup), so such a run needs a GPU. With `--screenshot` the walker
> renders from the driver's events as §12.2 says, drawing one frame per 100 ms of virtual
> time (every frame while settling). Confirm. (3) The player's appearance in a headless
> run: the walker's default avatar (`human,m` without `--avatar`) needs the install's
> character catalog, which only the walker loads; a driver-only run therefore has no client
> avatar and logs no `AppearanceChanged` for it, while a walker run uses the walker's avatar
> (default or `--avatar`) and does. Decide whether "the client's configured `avatar`"
> (§3.2) means `--avatar` only, and whether a driver-only run should load the catalog for
> it. (4) A `--scenario` path that does not exist or is not a regular file is a clap usage
> error (exit 2); a file that exists but cannot be read exits 1; the exit-code table of
> §12.2 names neither. (5) An install the user named (`--game-dir`, `TESSERA_GAME_DIR`) that
> cannot be used is treated as an unreadable install: after a successful parse the run
> exits 4 with `InstallUnreadable`, whether or not the file has a selector, instead of
> becoming a no-install run (§8.7 covers only "no install found"). The windowed
> `--scenario-log` is written by the client's world session from the commands it sends and
> the events it receives; its times are milliseconds of wall-clock time since the frame
> that delivered `WorldEntered`, the end line carries the feed's virtual time, it ends at
> the end line (or `reason=error error=Disconnected` when the session ends first, including
> closing the window), its counters have `input-skipped=0`, and a write failure exits 5.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of `apps/client`,
> 2026-10-09, after the review's rulings on the OPEN above) Point (1) is now implemented
> with the public input player the review endorsed, `tessera_net::scenario::InputPlayer`,
> which the headless driver uses too (the headless log is unchanged). A windowed run with
> `--scenario-input`: (a) sends the `at pre` lines after `Connected`, in file order and in
> the same frame, and sends `EnterWorld` (the `enter` character, else the first of
> `Connected`) after the next poll, so a character an `accept-character` step adds can be
> entered; the refusal (exit 2) applies only without `--scenario-input`; (b) before each
> poll while the feed runs, sends the timed lines due by tick *k* + 1, where *k* is the
> feed's last processed tick (`time_ms` / tick of `scenario_status`), so no line reaches
> the feed before its tick and a lagging feed receives lines late, never early; (c) counts
> lines that send nothing as `input-skipped`, which the windowed end line now carries. The
> windowed `--scenario-log` is formatted on the frame thread and written by a writer thread
> behind a bounded channel (256 chunks; a chunk the full channel refuses stays with the
> frame thread and is offered again with the next one); when 16 MiB (16,777,216 bytes) are
> formatted but not yet written, the log drops the lines from there on and ends with
> `reason=error … error=LogWrite line=0` (exit 5) instead of blocking the frame; a log
> that cannot be drained within 5 s of the session's end is incomplete (exit 5). Confirm
> or change (a)–(c) and the `LogWrite` ending.

<a id="s12-5"></a>
### 12.5 Overlay runs: `--scenario` with `--server` (revision 4, decision 0025 D1)

`tessera-client --scenario FILE --server CONNECT …` runs FILE as an **overlay**: the session's
backend is the `OverlayBackend` of `tessera_net::scenario`, which wraps the test-server
(`remote`) backend that the `--server` options build as without a scenario. The feed plays
only the Character and Inventory groups for the local player. Login, the character list,
character creation, world entry, movement, other entities, chat and teleports stay with the
server, unchanged.

<a id="s12-5-1"></a>
#### 12.5.1 The overlay subset

After parsing (§11.2) and before connecting, the client checks the file against this subset.
The first statement outside it is `NotAllowedInOverlay` at its first token (at the entity
token for `level` and `stat` with another entity), exit 3; a windowed run reports it the same
way.

| Part | Allowed |
|---|---|
| Settings | `title`, `tick`, `end-at`, `defaults`, `requires install` (not `enter`: the character is chosen on the server) |
| Declarations | `select`, `item` (not `player`, `character`, `actor`, `page`, `option`, `look`) |
| Steps | `bag`, `give`, `remove`, `quantity`, `equip`, `unequip`, `condition`, `wallet`, `item-property`; `level` and `stat` with the entity `player` only |
| Blocks | `repeat`; `on` with the patterns `equip`, `unequip`, `move-item`, `destroy`, `split`, `use-item`; `input` |
| Input lines | `at T` lines (not `at pre`) with the forms `equip`, `unequip`, `move-item`, `destroy`, `split`, `use-item` (target `player`, `current` or `none`) |

**No Combat group** (revision 6, [decision 0032](../decisions/0032-p2-targeting-on-test-server.md)
D3). The subset deliberately holds no Combat-group statement: no step of §6.2 (`target`,
`vital`, `combat`, `quickslot`, `cast`, `delay`, `interrupt`, `cooldown`, `effect`,
`effect-end`, `damage`, `heal`, `defeat`, `revive`), no trigger on `target`, `use-skill` or
`cancel-cast`, and no input form `target`, `use-skill`, `cancel-cast` or `quickslot`. Each is
`NotAllowedInOverlay` at its line's first token (§14.9 O10–O12). Selection in an overlay run
is the test-server backend's local answer (§9.7, §12.5.2); test actors are the server's
entities, which a scenario cannot name, and combat values scripted on them would be seen by
this client only.

<a id="s12-5-2"></a>
#### 12.5.2 Routing and order

- **Commands.** The Inventory group (`MoveItem`, `EquipItem`, `UnequipItem`, `SplitStack`,
  `DestroyItem`, `UseItem`) goes to the feed, which processes it at its next tick (§7.4) with
  the generic answers of §7.6 under `defaults on`. Every other command goes to the
  test-server backend unchanged, which answers `SelectTarget` locally and refuses the other
  new commands (§9.7; revision 6). The feed never sees `SelectTarget`.
- **Events.** In each poll the overlay first polls the test-server backend and queues its
  events, then lets the feed process its due ticks (at most 8, §7.1) and appends the feed's
  output. The per-tick delivery caps of §7.4 apply to the feed's output only.
- **Diagnostics.** `scenario_status()` and `scenario_counters()` as in §3.4, and
  `appearance_shared()`: whether the server advertised `APPEARANCE_CHANGE`.
- **P4 social** (decision 0035 D3). The subset of §12.5.1 is unchanged: no Social-group step
  (§6.4, §6.7), pattern or input form is allowed in an overlay file, and every Social command
  goes to the test-server backend (§9.8).

<a id="s12-5-3"></a>
#### 12.5.3 Clock and end

- Virtual time 0 is the poll that delivers the server's `WorldEntered`; the feed processes
  tick 0 in that poll, after the server's events. `now` is the frame time (windowed) or the
  wall clock, the driver pacing one tick per frame (headless). The server's timing reaches
  the log, so overlay logs are never normative and never committed (§10.5).
- The end of the run (§7.3) follows the windowed rules: after `Complete` without `end-at`
  triggers keep firing; after `end-at` or `Horizon` the feed answers no command (the overlay
  drops Inventory-group commands with a debug log) and the session with the server goes on.
- A run-time error of the feed (§11.3) stops the feed (status `Failed`); the session with the
  server goes on in a windowed run, and a headless run writes the end line and exits 5.
  When the server's session ends first, the feed stops and a headless run writes
  `reason=error error=Disconnected line=0` and exits 5 (§12.2).

<a id="s12-5-4"></a>
#### 12.5.4 The player's look

Let *B* be the appearance of the latest `AppearanceChanged` for the player that the server
delivered (the first comes with `WorldEnter`), and *E* the feed's equipment list of §5.5
(slots included). The **desired appearance** is *B* with its equipment replaced by *E*
(body, scale, idle and look of *B* kept).

With `APPEARANCE_CHANGE` advertised:

1. The feed emits no `AppearanceChanged` for the player.
2. The overlay marks the appearance **dirty** when *E* changes, and once when the first *B*
   arrives if *B*'s equipment differs from *E* (so equipment stored on the server from an
   earlier session is replaced by the scenario's). Nothing else marks it dirty.
3. At the end of each poll (after the feed's ticks) it sends `AppearanceSet` with the
   desired appearance (the backend leaves the look off the wire and sends slots only at
   minor ≥ 1, §9.7) when the appearance is dirty, *B* is known, no `AppearanceSet` is in
   flight, and at least 250 ms of wall-clock time have passed since the previous one; then it
   clears dirty and marks one in flight. Several changes in
   between are thus coalesced into the latest *E*.
4. The next `AppearanceChanged` for the player from the server clears the in-flight mark
   (the server answers every `AppearanceSet` with `EntityAppearance`,
   `tessera-protocol.md` §6.2.17) and becomes the new *B*; it passes to the client half
   unchanged, so the walker shows what the server holds. Without it within 10 s the mark is
   cleared with a warning.
5. A refused `AppearanceSet` (the server answers with the old appearance and a system line)
   changes nothing in the feed and is not retried until *E* changes again. The inventory
   panels show the feed's state; the worn look shows the server's.

Without `APPEARANCE_CHANGE`: the feed emits `AppearanceChanged` for the player with the
desired appearance whenever *E* changes and *B* is known, as the sandbox does (§5.5), and the
overlay logs once at info level that other players will not see the change.

Observers on the same server see the change as an `AppearanceChanged` of the player's entity
(their backend maps `EntityAppearance`, `tessera-protocol.md` Appendix A). This is P1 exit
criterion 3 (decision 0024).

<a id="s12-5-5"></a>
#### 12.5.5 Bounds and the server's rules

- The server receives from an overlay run only what any client may send: `AppearanceSet`
  with a body, scale, idle and at most 32 opaque item DIDs (with slots at minor ≥ 1), at most
  one in flight and at most 4 per second, below the protocol's control budget
  (`tessera-protocol.md` §13.1). Bags, items, quantities, conditions, wallet, levels, stats and
  item properties stay in the client and are never sent (LEGAL.md §11.1 rule 5;
  `server-content.md` §7).
- `tessera-server` is unchanged: it links neither `tessera-net` nor the feed (§3.1, §15
  invariant 1), and the server dependency check is unchanged.
- Conformance: §14.9.

> **OPEN:** (implementer of `crates/net`, clean room, 2026-10-10; non-blocking) Readings the
> implementation took where §12.5.4 is silent, for the spec owner to confirm or change:
> (a) "*B*'s equipment differs from *E*" compares the item DIDs and slots in order, not the
> dyes or the stowed flag (the feed's list carries none); (b) an `AppearanceSet` the server
> backend refuses to queue (closed session, full queue) clears the dirty mark with a warning
> and is not retried until *E* changes again, like a refused one (rule 5); (c) a run-time
> error of the feed stops the feed without the `Disconnected` the sandbox would emit
> (§12.5.3: the session with the server goes on); (d) the overlay sends the desired
> appearance with its slots and the test-server backend's encoder leaves them off at minor 0
> (§9.7), so the overlay itself never looks at the minor.

---

<a id="s13"></a>
## 13. Committed-content rules

1. **Original and synthetic.** Committed scenarios are original work of contributors, licensed
   GPL-3.0-or-later (or CC0-1.0), in `apps/client/scenarios/`. They contain no game text: no
   names, dialogue, quest, item or place text from the game; literal strings are project-made.
2. **Selectors only.** Committed scenarios use selectors to name records and MUST NOT contain
   record identifiers: no `did` family, and no attribute value that is a DID. Enum values
   anywhere in the file (selector filters, the `class=`, `species=` and `sex=` options of
   declarations, steps and input lines, `slot=` of `equip` and `type=` of `damage`/`heal`) are
   labels (`@Label`) that an Accepted
   spec already names, Tessera-defined keywords, or `m` / `f`; numeric enum keys stay in
   local scenarios (§16). Since revision 4 the same holds for property values (§5.8): the
   property is a dictionary label an Accepted spec names, never `pid:N:KIND`; a value of kind
   `enum`, `bitfield32` or `bitfield64` is a label (numeric keys and masks are enum keys),
   one of kind `did` an element, never a number, and other kinds take their literals
   (booleans, numbers, strings, colours: scripted values, not keys); the `nationality=` of
   `create-character` is a label too. Whether `pid:` forms may be committed
   is part of the §16 owner question. Container slot numbers (`give … slot=`, `unequip … slot=`,
   `move-item … slot=`), quickslot and trait slot numbers, entry and option numbers are plain
   numbers, not enum values.
3. **Local scenarios.** Scenarios that name DIDs the user picked, or that copy text from the
   install, stay local (git-ignored or outside the repository).
4. **Logs.** Only `install none` logs of committed synthetic scenarios (§10.5).
5. **Checks.** A test parses every committed scenario, rejects the `did` family, `pid:`
   property forms and numeric values in the enum positions of rule 2 (the kind of a labelled
   property is unknown without an install, so its value rule is a review item), and replays every committed log. Review uses the checklist of
   `server-content.md` §9.3 for text (no game names or text).
6. **P5 travel enum positions.** `note-type=` of `marker` and `markers`, and `size=` and
   `tier=` of `instances` and of the input form `join-instance`, are enum positions under
   rule 2 (labels only in committed scenarios); so is a `mount-type=` filter value. The
   `route=`, `kind=`, `instance-queue` and `mount-vital` words are Tessera-defined keywords;
   entry lists, levels, `min-level`, `seconds` and vital amounts are plain numbers.

> **OPEN:** (non-blocking; for the spec owner, raised by the P3 implementer of `crates/net`
> after review, 2026-10-10) Rule 2's list of enum positions predates the P3 revision: the
> integer of the text function `action(N)` (§5.2) is a key of the NPC-action enum mapper
> (§8.6), so it reads as an enum position, but the list does not name it. The implementation
> treats `action(N)` as a numeric enum value (`Scenario::numeric_enum_values` reports it in
> every text position) and `action(@Label)` as a label. Name `action(…)` in rule 2's list, or
> say that `action(N)` may be committed.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of `apps/client`,
> 2026-10-09) Rule 1 names `apps/client/scenarios/`; the P0 plan placed the committed
> scenarios and their logs in `apps/client/tests/scenarios/` (`NAME.scenario` with
> `NAME.log` beside it), which is where they are today and where the replay test
> (`apps/client/tests/scenarios.rs`) reads them. Align rule 1 with that path, or move them.

> **OPEN:** (non-blocking; for the spec owner, raised by the documentation review of P0 and
> recorded by the P0 exit verifier, 2026-10-09) Rule 1 allows committed scenarios under
> CC0-1.0, but the content test of rule 5 (`apps/client/tests/scenarios.rs`) requires the
> `GPL-3.0-or-later` SPDX header and the USER_GUIDE says so. Either drop "(or CC0-1.0)" here
> or widen the test.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of `apps/client`,
> 2026-10-09) Following the review's ruling on the two OPENs above, the content test now
> accepts a first line `# SPDX-License-Identifier: GPL-3.0-or-later` or
> `# SPDX-License-Identifier: CC0-1.0` (and the USER_GUIDE says so), and the committed
> scenarios stay in `apps/client/tests/scenarios/`; rule 1 still names
> `apps/client/scenarios/` until the next revision.

---

<a id="s14"></a>
## 14. Conformance vectors

All vectors run headless with `--no-data` and no spawn options: `HeadlessDriver` over the
plain sandbox (§12.2) at the exact default spawn of §3.2 (region 0, origin, heading 0 = east,
compass 90), with no welcome line (§3.2). Files are exactly the lines shown, each
ending in LF.

<a id="s14-1"></a>
### 14.1 Vector A: actor, vital, chat

```text
tessera-scenario 1
title "Vector A"
tick 50ms
player "Testa"
actor $guide kind=npc name="Guide" pos=2,0,0 facing=270
at 0s spawn $guide
+0s vital player health 1000/1000
+0.5s say $guide "Welcome, traveller."
at 1s despawn $guide
```

Expected log:

```text
tessera-scenario-log 1
install none
tick 50
title "Vector A"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$guide kind=non-player name="Guide" at=2.000,0.000,0.000 facing=270.00
0.000 VitalChanged entity=player vital=health current=1000 max=1000
0.500 ChatMessage channel=say sender="Guide" text="Welcome, traveller."
1.000 EntityDespawned entity=$guide
1.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-2"></a>
### 14.2 Vector B: trigger, repeat, input, empty selector

```text
tessera-scenario 1
title "Vector B"
tick 100ms
player "Testa"
select $skills = skill harmful=yes n=2 seed=7
actor $dummy kind=creature name="Practice post" pos=0,3,0 facing=180
at 0s spawn $dummy
+0s vital $dummy health 500/500
+0s quickslot 1 "Kindle"
+0s quickslot 2 $skills[0]
on use-skill quickslot=1 target=$dummy
+0s cast player "Kindle" 1s target=$dummy
+1s damage player $dummy 120 result=critical left=380
+0s effect $dummy "Smoulder" for=2s
end
at 2s repeat 3 every 500ms
+0s say $dummy "creaks" channel=emote
end
input
at 0.2s target $dummy
at 0.3s use-skill quickslot=1
end
```

Expected log (the step `quickslot 2 $skills[0]` is skipped; at 1.300 the automatic cast end,
class 0, precedes the scripted steps, §7.4):

```text
tessera-scenario-log 1
install none
tick 100
title "Vector B"
resolve $skills line=5 family=skill requested=2 candidates=0 got=0 note=no-install
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$dummy kind=creature name="Practice post" at=0.000,3.000,0.000 facing=180.00
0.000 VitalChanged entity=$dummy vital=health current=500 max=500
0.000 QuickslotChanged slot=1 content=skill:"Kindle"
0.200 > SelectTarget target=$dummy
0.200 TargetChanged entity=player target=$dummy
0.300 > UseSkill what="Kindle" target=$dummy
0.300 CastStarted entity=player cast=cast what="Kindle" target=$dummy duration_ms=1000
1.300 CastEnded entity=player outcome=completed
1.300 VitalDelta source=player target=$dummy vital=health kind=damage amount=120 result=critical what=none damage_type=none
1.300 VitalChanged entity=$dummy vital=health current=380 max=500
1.300 EffectApplied entity=$dummy effect=1 what="Smoulder" duration_ms=2000 stacks=1 source=none
2.000 ChatMessage channel=emote sender="Practice post" text="creaks"
2.500 ChatMessage channel=emote sender="Practice post" text="creaks"
3.000 ChatMessage channel=emote sender="Practice post" text="creaks"
3.300 EffectRemoved entity=$dummy effect=1
3.300 end reason=complete skipped=1 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-3"></a>
### 14.3 Vector C: generator and sampling

SplitMix64, first three outputs:

| Seed | Outputs |
|---|---|
| 0 | 0xE220A8397B1DCDAF, 0x6E789E6AA1B965F4, 0x06C45D188009454F |
| 1 | 0x910A2DEC89025CC1, 0xBEEB8DA1658EEC67, 0xF893A2EEFB32555E |
| 7 | 0x63CBE1E459320DD7, 0x044C3CD7F43C661C, 0xE6984080BAB12A02 |

Sampling from the ten candidates 0x70000001 … 0x7000000A (rows 0), given by a synthetic
resolver in any order:

| n | seed | Result (canonical order) |
|---|---|---|
| 3 | 7 | 0x70000001, 0x70000005, 0x70000008 |
| 3 | 0 | 0x70000002, 0x70000006, 0x7000000A |
| 5 | 1 | 0x70000002, 0x70000004, 0x70000006, 0x70000008, 0x70000009 |
| 1 | 7 | 0x70000008 |
| 10 | 7 | all ten |
| 12 | 7 | all ten (note `short`) |
| 3 | none | 0x70000001, 0x70000002, 0x70000003 |

Trace of (n = 3, seed 7): draw 0x63CBE1E459320DD7 with bound 10 → *j* = 7; draw
0x044C3CD7F43C661C with bound 9 → *j* = 1 + 6 = 7; draw 0xE6984080BAB12A02 with bound 8 → *j* =
2 + 2 = 4. The array after the swaps begins 0x70000008, 0x70000001, 0x70000005.

<a id="s14-4"></a>
### 14.4 Vector D: dialogue, quest offer, generic answers

```text
tessera-scenario 1
title "Vector D"
tick 100ms
player "Testa"
actor $elder kind=npc name="Elder" pos=-2,0,0 facing=90
page $thanks speaker=$elder "Thank you."
page $hello speaker=$elder "Good day. Will you help?"
option $hello 1 "Gladly." goto=$thanks
option $hello 2 "Not now." goto=close
at 0s spawn $elder
on use $elder
+0s show $hello
end
on choose $hello 1
+0.5s offer "Fetch water" from=$elder
end
input
at 0.1s use $elder
at 0.4s choose $hello 1
at 1s accept
end
```

The trigger `on choose $hello 1` matches because matching looks at the window before the
generic answer replaces its page (§7.4 step 1). Expected log:

```text
tessera-scenario-log 1
install none
tick 100
title "Vector D"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$elder kind=non-player name="Elder" at=-2.000,0.000,0.000 facing=90.00
0.100 > UseEntity entity=$elder
0.100 DialogueShown window=1 speaker=$elder text="Good day. Will you help?" options=[{option=1 text="Gladly."},{option=2 text="Not now."}]
0.400 > ChooseOption window=1 option=1
0.400 DialogueShown window=1 speaker=$elder text="Thank you." options=[]
0.900 QuestOffered window=2 quest="Fetch water" giver=$elder
1.000 > AnswerQuest window=2 accept=yes
1.000 QuestState quest="Fetch water" state=active
1.000 WindowClosed window=2
1.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-5"></a>
### 14.5 Hostile inputs

Line 1 of every file is `tessera-scenario 1` unless the input says otherwise; "line 2" is the
next line.

| Input | Expected error |
|---|---|
| empty file | `BadHeader` 1:1 |
| `tessera-scenario 2` | `UnsupportedVersion` 1:18 |
| 1,048,577 bytes of anything | `FileTooLarge` 1:1 |
| line 2 `title "a` followed by byte 0xFF and `"` | `Encoding` 2:9 |
| line 2 `title "a` followed by U+0000 and `"` | `ControlCharacter` 2:9 |
| line 2 of 5,000 bytes | `LineTooLong` 2:1 |
| line 2 `title "abc` | `UnterminatedString` 2:7 |
| line 2 `title "\q"` | `BadEscape` 2:8 |
| line 2 `title "\u{7}"` | `BadEscape` 2:8 |
| line 2 `tick 5ms` | `OutOfRange` 2:6 |
| line 2 `at 2h system "x"` | `BadDuration` 2:4 |
| line 2 `at 61min system "x"` | `OutOfRange` 2:4 |
| line 2 `at 0.0005s system "x"` | `BadDuration` 2:4 |
| line 2 `at 1s spawn $ghost` | `UndefinedBinding` 2:13 |
| line 2 `select $a = item n=70000` | `LimitExceeded(selector)` 2:18 |
| line 2 `select $a = item kind=weapon` | `MissingCount` 2:1 |
| line 2 `select $a = item colour=@Red all` | `UnknownAttribute` 2:18 |
| line 2 `actor $a kind=npc pos=1,2` | `BadPosition` 2:19 |
| lines 2–3 `actor $a kind=npc pos=0,0,0` twice | `DuplicateBinding` 3:7 |
| line 2 `actor $a kind=npc pos=0,0,0`, line 3 `at 0s spawn $a[0]` | `WrongBindingKind` 3:13 |
| line 2 `on use player` and nothing after | `UnclosedBlock` 2:1 |
| line 2 `end` | `UnexpectedEnd` 2:1 |
| lines 2–4 `on cancel-cast`, `+0s repeat 2 every 1s`, `+0s repeat 2 every 1s` | `NestingTooDeep` 4:5 |
| line 2 `at 0s repeat 20000 every 1ms` | `LimitExceeded(repeat)` 2:14 |
| line 2 `on cancel-cast`, line 3 `at 1s system "x"` | `UnexpectedToken` 3:1 |
| line 2 `at 0s system "x"`, line 3 `tick 100ms` | `SettingAfterStep` 3:1 |
| line 2 `input`, line 3 `on cancel-cast` | `NotAllowedHere` 3:1 |
| a scenario declaring 1,025 actors | `LimitExceeded(actors)` at the 1,025th declaration, column 1 |
| Vector D with line 6 (`page $thanks …`) moved below the `option` lines | `UndefinedBinding` 7:27 |
| line 2 `title "a"b` | `UnexpectedToken` 2:7 |
| line 2 `select $a = item all seed=1` | `UnexpectedToken` 2:22 |
| line 2 `player "Testa"`, line 3 `enter "Nobody"` | `UnknownCharacter` 3:7 |
| line 2 `player "A"`, line 3 `player "B"` | `DuplicateBinding` 3:1 |
| line 2 `player "A"`, line 3 `character "A"` | `DuplicateBinding` 3:11 |
| line 2 `player "A"`, line 3 `enter "B"`, line 4 `on cancel-cast`, end of file (no `end`) | `UnclosedBlock` 4:1 (of the two end-of-file errors, the first in the order of §4.5) |
| lines 2–3 `at 0s repeat 2 every 1s`, `+0s repeat 2 every 1s` | `NotAllowedHere` 3:5 (depth 2, but `repeat` inside a top-level `repeat`, §4.4) |
| lines 2–3 `on cancel-cast`, `on cancel-cast` | `NotAllowedHere` 3:1 |
| lines 2–4 `input`, `end`, `input` | `NotAllowedHere` 4:1 (a second `input` block) |
| lines 2–3 `on create-character`, `+0s system "x"` | `NotAllowedHere` 3:5 (only character steps before world entry, §7.5) |
| line 2 `actor $a kind=npc pos=0,0,` followed by 31 digits `1000…0` and `.5` | `BadNumber` 2:19 (a 33-character component, §4.2) |
| line 2 `actor $a kind=npc pos=-99999.999999999,-99999.999999999,-99999.999999999` | no error (50 characters, each component 16, §4.2) |
| line 2 `at 0s trade-offer own items=` followed by 33 literal refs `"a","a",…,"a"` | `LimitExceeded(entries)` 2:23 (trade side 32, §6.3) |
| lines 2–4 `input`, `at 1s choose 17`, `end` | `OutOfRange` 3:14 (options 1 … 16, §7.7) |
| lines 2–4 `input`, `at 1s craft "Stew" qty=70000`, `end` | `OutOfRange` 3:20 (`Craft.quantity` is `u16`, §7.7) |
| lines 2–5 `end-at 0s`, `input`, `at 0s cancel-cast`, `end` | `OutOfRange` 4:4 (processed in tick 1, after `end-at`, §7.7) |
| line 2 `at 0s stat player pid:0x7FFF0001:int=1.5` | `BadValue` 2:19 (a decimal for kind `int`, §5.8) |
| line 2 `at 0s stat player pid:0x7FFF0001:int=2147483648` | `OutOfRange` 2:19 |
| line 2 `at 0s stat player pid:0:int=1` | `OutOfRange` 2:19 (property ID 0) |
| line 2 `at 0s stat player pid:0x7FFF0001:text=1` | `UnexpectedToken` 2:19 (unknown kind) |
| line 2 `at 0s stat player pid:0x7FFF0001:colour=rgba:256,0,0,0` | `OutOfRange` 2:19 |
| line 2 `look $l pid:0x7FFF0001:int=none` | `BadValue` 2:9 (`none` in a look) |
| line 2 `look $l pid:0x7FFF0001:string="a b"` | `BadValue` 2:9 (a space in a look string) |
| line 2 `look $l pid:0x7FFF0001:instance=1` | `BadValue` 2:9 (kind `instance` in a look) |
| line 2 `look $l` followed by 49 tokens `pid:0x7FFF0001:int=1`, one space apart | `LimitExceeded(look)` 2:1017 (the 49th, §5.8) |
| line 2 `at 0s remove-character "A"` | `NotAllowedHere` 2:7 (character steps only in pre-world bodies) |
| lines 2–3 `on delete-character`, `+0s accept-character "A"` | `NotAllowedHere` 3:5 (`accept-character` only in `on create-character`, §6.1) |
| lines 2–3 `on create-character`, `+0s stat player @Example_Stat=1` | `NotAllowedHere` 3:5 |
| overlay run (§12.5.1), line 2 `actor $a kind=npc pos=0,0,0` | `NotAllowedInOverlay` 2:1 |
| overlay run, lines 2–3 `on chat`, `end` | `NotAllowedInOverlay` 2:1 |
| overlay run, lines 2–5 `item $i "Ring"`, `input`, `at pre destroy $i`, `end` | `NotAllowedInOverlay` 4:1 (no `at pre` lines) |

**P3 revision hostile rows** (named N-H*n*; line 1 is the header as above):

| # | Input | Expected error |
|---|---|---|
| N-H1 | lines 2–3 `page $p "x"`, `option $p 1 action(Vendor)` | `UnexpectedToken` 3:13 (an `action` argument is a label or an integer) |
| N-H2 | line 2 `at 0s rewards` | `MissingArgument` 2:1 |
| N-H3 | line 2 `at 0s rewards gold:5` | `UnexpectedToken` 2:15 (unknown kind) |
| N-H4 | line 2 `at 0s rewards coin:-5` | `OutOfRange` 2:15 |
| N-H5 | line 2 `at 0s rewards coin:1 coin:2` | `UnexpectedToken` 2:22 (a single kind twice) |
| N-H6 | line 2 `at 0s rewards` followed by 17 tokens `rep:"a":1`, one space apart | `LimitExceeded(entries)` 2:175 (the 17th token) |
| N-H7 | line 2 `at 0s rewards trait-points:1000001` | `OutOfRange` 2:15 |
| N-H8 | line 2 `at 0s rewards rep:"a":1000000000001` | `OutOfRange` 2:15 |
| N-H9 | line 2 `at 0s buyback qty=2` | `MissingArgument` 2:1 (no `items=`) |
| N-H10 | lines 2–4 `input`, `at 1s buy-back 0`, `end` | `OutOfRange` 3:16 (entries 1 … 1,024) |
| N-H11 | line 2 `at 0s say player action(@Vendor)` | `UnexpectedToken` 2:18 (text functions are not allowed in chat, §5.2) |

**P5 travel hostile rows** (named T-H*n*; line 1 is the header as above):

| # | Input | Expected error |
|---|---|---|
| T-H1 | line 2 `at 0s travel-list dest="a" min-level=10001` | `OutOfRange` 2:28 |
| T-H2 | line 2 `at 0s travel-list dest="a" undiscovered=0` | `OutOfRange` 2:28 (entry numbers 1 … 1,024) |
| T-H3 | line 2 `at 0s travel-list dest="a" swift=1 slow=1` | `UnexpectedToken` 2:36 (entry 1 in both, reported at the later key) |
| T-H4 | line 2 `at 0s travel "a" 1s route=fast` | `UnexpectedToken` 2:21 |
| T-H5 | line 2 `at 0s instances "a" size=1,@Small,x` | `UnexpectedToken` 2:21 (`x` is neither an integer nor a label) |
| T-H6 | line 2 `at 0s instances "a" locked=1,1` | `UnexpectedToken` 2:21 (a repeated entry number) |
| T-H7 | line 2 `at 0s instance-queue waiting` | `UnexpectedToken` 2:22 |
| T-H8 | line 2 `at 0s instance-queue queued seconds=86401` | `OutOfRange` 2:29 |
| T-H9 | line 2 `at 0s mount player none kind=combat` | `UnexpectedToken` 2:25 (`kind` only with a ref) |
| T-H10 | line 2 `at 0s mount-vital player morale 5/4` | `OutOfRange` 2:33 (CUR above MAX) |
| T-H11 | line 2 `at 0s mount-vital player stamina 1/1` | `UnexpectedToken` 2:26 |
| T-H12 | line 2 `at 0s marker $m 0,0,0 "x" note-type=65` | `OutOfRange` 2:27 |
| T-H13 | line 2 `select $s = map-region all`, line 3 `at 0s marker $m 0,0,0 "x"`, line 4 `at 1s markers $m $s named` | `WrongBindingKind` 4:15 (`$m` is a single marker) |
| T-H14 | line 2 `select $s = map-region kind=area all` | `UnexpectedToken` 2:24 (`area` is not a keyword of `kind`) |
| T-H15 | lines 2–4 `input`, `at 1s join-instance 1 level=0`, `end` | `OutOfRange` 3:23 |
| T-H16 | line 2 `at 0s travel-end now` | `UnexpectedToken` 2:18 |
| T-H17 | line 2 `select $s = map-region all`, line 3 `at 0s markers $ms $s` | `MissingArgument` 3:1 (no label and no `named`) |
| T-H18 | line 2 `at 0s travel-list dest="a" min-level=` followed by 1,025 items `1,1,…,1` | `LimitExceeded(entries)` 2:28 |

<a id="s14-6"></a>
### 14.6 Vector E: inventory, generic answers, `move … over`, list entries, `end-at`

```text
tessera-scenario 1
title "Vector E"
tick 100ms
end-at 1s
player "Testa"
actor $cart kind=object name="Cart" pos=0,0,0 facing=0
item $bread "Bread" qty=5
item $ale "Ale"
at 0s spawn $cart
+0s bag 4
+0s give $bread
+0s give $ale slot=2
+0s wallet coin 1234
+0.1s move $cart 0,3,0 over=300ms
+0s cooldown "Rest" 2s
at 0.7s vendor items="Bread","Ale" npc=$cart price=10,25
input
at 0.5s split $bread 2
at 0.6s move-item $ale to=backpack slot=0
end
```

Expected log (`move … over` emits on the three ticks after its own; the split's new item is the
first run-time item, id 3, at the first free slot; the move onto an occupied slot swaps; the
cooldown end at 2.100 lies beyond `end-at` and is dropped):

```text
tessera-scenario-log 1
install none
tick 100
title "Vector E"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$cart kind=object name="Cart" at=0.000,0.000,0.000 facing=0.00
0.000 ContainerSize container=backpack slots=4
0.000 ItemAdded item=$bread what="Bread" at=backpack:0 quantity=5
0.000 ItemAdded item=$ale what="Ale" at=backpack:2 quantity=1
0.000 WalletChanged currency=coin amount=1234
0.100 CooldownStarted what="Rest" duration_ms=2000
0.200 EntityMoved entity=$cart at=0.000,1.000,0.000 facing=0.00 velocity=0.000,10.000,0.000
0.300 EntityMoved entity=$cart at=0.000,2.000,0.000 facing=0.00 velocity=0.000,10.000,0.000
0.400 EntityMoved entity=$cart at=0.000,3.000,0.000 facing=0.00 velocity=0.000,0.000,0.000
0.500 > SplitStack item=$bread quantity=2
0.500 ItemQuantity item=$bread quantity=3
0.500 ItemAdded item=item:3 what="Bread" at=backpack:1 quantity=2
0.600 > MoveItem item=$ale to=backpack:0
0.600 ItemMoved item=$ale at=backpack:0
0.600 ItemMoved item=$bread at=backpack:2
0.700 VendorShown window=1 vendor=$cart entries=[{entry=1 what="Bread" price=coin:10 stock=none},{entry=2 what="Ale" price=coin:25 stock=none}]
1.000 end reason=complete skipped=0 input-skipped=0 dropped=1 spilled=0
```

<a id="s14-7"></a>
### 14.7 Vector F: pre-world output, character triggers, `at 0s` input, tick-time starts, chat answer

```text
tessera-scenario 1
title "Vector F"
tick 100ms
player "Testa" level=5
character "Bruni" class=@Example_Class
actor $post kind=object name="Post" pos=0,1,0 facing=359.999
item $apple "Apple"
item $pear "Pear"
at 0s spawn $post
+0s give $apple slot=1
+0s give $pear slot=1
+0s give $apple
on create-character name="Cora"
+0s accept-character "Cora" level=1
end
on create-character name="Dora"
+0s refuse-character "Dora" "That name is taken."
end
on chat
+0.05s cast player "Ember" 30ms
end
input
at pre create-character "Cora"
at pre create-character "Dora"
at 0s chat "hi" channel=tell:"Bruni"
end
```

Expected log. Before world entry the feed's output follows the sandbox's answer in the same
poll and is logged `pre` (§3.3): `CharacterInfo` for "Testa" (a numeric detail) but none for
"Bruni" (its only detail is an unresolved label, §5.1), then the trigger bodies of the two
`CreateCharacter` commands, which carry no class and no appearance (§7.7). The welcome line is
absent (§3.2). `facing=359.999` is written `0.00` (§10.3). Of the three `give` steps, the
second targets an occupied slot and the third an item already placed: two skips (§5.5). The
`at 0s` input is sent in frame 1, so *L* = 0.100 (§7.3): the run does not end at tick 0,
where the agenda is already empty. The tell gets the feed's generic answer (§7.6). The
trigger body is due at 0.150 and runs in tick 0.200; the cast's end counts from that tick,
0.230, so it is emitted at 0.300 (§6.2), and the agenda is empty from then on. (Revision 4
added the fields `look` to `CharacterInfo` and `nationality` and `look` to `CreateCharacter`,
§10.3; nothing else in this vector changed.)

```text
tessera-scenario-log 1
install none
tick 100
title "Vector F"
pre > Connect
pre Connected characters=["Testa","Bruni"]
pre CharacterInfo name="Testa" body=none class=none level=5 look=none
pre > CreateCharacter name="Cora" appearance=none class=none nationality=none look=none
pre CharacterInfo name="Cora" body=none class=none level=1 look=none
pre > CreateCharacter name="Dora" appearance=none class=none nationality=none look=none
pre CharacterRefused name="Dora" reason="That name is taken."
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$post kind=object name="Post" at=0.000,1.000,0.000 facing=0.00
0.000 ItemAdded item=$apple what="Apple" at=backpack:1 quantity=1
0.100 > Chat channel=tell:"Bruni" text="hi"
0.100 ChatMessage channel=system sender=none text="Bruni is not in the sandbox."
0.200 CastStarted entity=player cast=cast what="Ember" target=none duration_ms=30
0.300 CastEnded entity=player outcome=completed
0.300 end reason=complete skipped=2 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-8"></a>
### 14.8 Vector G: looks, character removal, stats, item properties (revision 4)

The property IDs below lie outside both ID families of the property dictionary
(`property-system.md` MP4: below 0x10000000, or 0x1000xxxx), so they name no property of any
install; the vector runs with no install, where `pid:` forms are taken as written (§5.8).

```text
tessera-scenario 1
title "Vector G"
tick 100ms
look $face pid:0x7FFF0002:colour=rgba:200,150,100,255 pid:0x7FFF0001:float=0.25 pid:0x7FFF0003:string="80ff"
player "Testa" look=$face
character "Ward" level=3
item $ring "Ring"
at 0s give $ring
+0s item-property $ring pid:0x7FFF0010:string="Testa"
+0s stat player pid:0x7FFF0020:int=42
+0s stat player @Example_Stat=7
+0.1s stat player pid:0x7FFF0020:int=none
on delete-character
+0s remove-character "Ward"
+0s remove-character "Nobody"
end
input
at pre delete-character "Ward"
at pre create-character "Cora" nationality=@Example_Nationality
end
```

Expected log. "Testa" has a look and so a `CharacterInfo`, its entries sorted by property ID
(§5.8); it has no species and sex, so no base appearance and no `AppearanceChanged` (§5.5).
The deletion fires the trigger in `submit`: "Ward" is removed, "Nobody" is not in the list
(a skip). The creation fires no trigger and gets no answer (a scenario owns character
answers; §9.6 does not run); its nationality label stays unresolved without an install and
writes no note (§8.6 item 6). `@Example_Stat` stays unresolved, so that `stat` step is a skip
(§5.8). The last step clears the property at 0.100, after which the agenda is empty.

```text
tessera-scenario-log 1
install none
tick 100
title "Vector G"
pre > Connect
pre Connected characters=["Testa","Ward"]
pre CharacterInfo name="Testa" body=none class=none level=none look=[{property=0x7FFF0001 value=float:0.25},{property=0x7FFF0002 value=colour:200,150,100,255},{property=0x7FFF0003 value=string:"80ff"}]
pre CharacterInfo name="Ward" body=none class=none level=3 look=none
pre > DeleteCharacter name="Ward"
pre CharacterRemoved name="Ward"
pre > CreateCharacter name="Cora" appearance=none class=none nationality=none look=none
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 ItemAdded item=$ring what="Ring" at=backpack:0 quantity=1
0.000 ItemProperty item=$ring property=0x7FFF0010 value=string:"Testa"
0.000 PropertyChanged entity=player property=0x7FFF0020 value=int:42
0.100 PropertyChanged entity=player property=0x7FFF0020 value=none
0.100 end reason=complete skipped=2 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-9"></a>
### 14.9 Overlay cases (revision 4)

Overlay runs are not deterministic as logs (§12.5.3); these cases state what the overlay MUST
send and emit. They run with a **test double** of the test-server backend that advertises the
capabilities given, negotiates the minor version given, delivers `WorldEntered` and the
player's `AppearanceChanged` with the base appearance *B*₀ = Character 23/1, scale 1, no idle,
no look, no items (unless a case says otherwise), and answers each `AppearanceSet` 50 ms later
with an `AppearanceChanged` for the player carrying the appearance it accepted. The resolver
is a synthetic one whose `did` family returns its listed DIDs (`install present`). Tick
100 ms; the clock is the test's. Every case uses:

```text
tessera-scenario 1
title "Overlay"
tick 100ms
select $gear = did 0x70001234,0x70005678 all
item $sword $gear[0]
item $shield $gear[1]
at 0s give $sword
+0s give $shield
+0s equip $sword slot=17
+0s equip $shield slot=18
at 0.1s unequip $shield
at 0.2s equip $shield slot=18
```

| # | Double | Expected |
|---|---|---|
| O1 | `APPEARANCE_CHANGE`, minor 1 | At tick 0: the feed's `ItemAdded` and `ItemMoved` events, no `AppearanceChanged` from the feed, and one `AppearanceSet` with *B*₀'s body and scale and the items 0x70001234 slot 17, 0x70005678 slot 18 (both equips of the tick coalesced). At 0.05 s the double's answer surfaces as the player's `AppearanceChanged` with those items. |
| O2 | as O1 | The unequip at 0.1 s and the equip at 0.2 s change *E* twice; no `AppearanceSet` before 0.25 s of wall-clock time after the first; one `AppearanceSet` in the first poll at or after 0.25 s, with both items again. Two sends in all. |
| O3 | no `APPEARANCE_CHANGE`, minor 1 | No `AppearanceSet`. At tick 0 the feed emits the player's `AppearanceChanged` with both items and slots; at 0.1 s with the sword only; at 0.2 s with both. One info log line that observers will not see the change. |
| O4 | `APPEARANCE_CHANGE`, minor 0 | As O1, but the `AppearanceSet`'s items carry no slot (flag bit 3 clear, `tessera-protocol.md` §5.3) and the answer's items have none. |
| O5 | as O1, but *B*₀ holds the item 0x70009999 | *B*₀ marks the appearance dirty (its equipment differs from the empty *E*); the send at the end of the tick-0 poll carries both items, so the stored 0x70009999 is replaced. One send at tick 0, as in O1. (With no equip step in the file, that send would carry no items.) |
| O6 | as O1, but the double refuses every `AppearanceSet` (answers *B*₀) | After the tick-0 send and its answer, no further `AppearanceSet` until *E* changes at 0.1 s; never more than one in flight. |
| O7 | any | A file with `at 0s say "x"` is refused with `NotAllowedInOverlay` before anything connects (exit 3). |
| O8 | as O1 | A `Chat` and a `Teleport` go to the double; an `EquipItem` goes to the feed; a `SelectTarget { target: None }` after world entry is answered by the test-server backend with `TargetChanged { entity: player, target: None }`, first in the next poll, the double receives no message for it, and no trigger of the feed sees it (revision 6, §9.7; before: refused). A `UseSkill` is refused with `NetError::InvalidCommand`. |
| O9 | `tessera-server` in process, `appearance_change` true, two clients at minor 1 | The observer sees an `AppearanceChanged` for the overlay player's entity with the items 0x70001234 slot 17 and 0x70005678 slot 18 (P1 exit criterion 3). |
| O10 | any | (Revision 6.) The file `tessera-scenario 1` / `at 0s target player none` is refused with `NotAllowedInOverlay` 2:1 before anything connects (exit 3). So is `at 0s vital player health 5/10`. |
| O11 | any | (Revision 6.) The file `tessera-scenario 1` / `on target any` / `end` is refused with `NotAllowedInOverlay` 2:1 (exit 3). |
| O12 | any | (Revision 6.) The file `tessera-scenario 1` / `input` / `at 1s target none` / `end` is refused with `NotAllowedInOverlay` 3:1 (exit 3). |

> **OPEN:** (implementer of `crates/net`, clean room, 2026-10-10; non-blocking) O7's file
> `at 0s say "x"` does not parse (`say` needs a sender before its text, §6.4), so it is
> refused with `MissingArgument` (also exit 3) before the overlay check runs. The
> conformance test uses `at 0s say player "x"`, which parses and gives
> `NotAllowedInOverlay` 2:1. O3's "at tick 0 … with both items" is read with §5.5 "every
> change of that list emits": the two equips of tick 0 give two events in that tick, the
> sword alone and then both; the test asserts both.

<a id="s14-10"></a>
### 14.10 Plain-sandbox creation cases (revision 4)

The plain sandbox (no scenario) with the character list ["Testa"] and a configured avatar:

| # | Commands | Expected events |
|---|---|---|
| P1 | `CreateCharacter { name: "Cora", appearance: Character 23/1 with the look [0x7FFF0001 float 0.25], class: Some(5), nationality: Some(2) }`, then `EnterWorld "Cora"` | `CharacterInfo { name: "Cora", body: Character 23/1, class: Some(5), level: None, look: [0x7FFF0001 float 0.25] }`; after `WorldEntered`, the player's `AppearanceChanged` has body 23/1 and that look, not the configured avatar |
| P2 | `CreateCharacter { name: "Testa", … }` | `CharacterRefused { name: "Testa", reason: "A character with this name already exists." }` |
| P3 | 63 creations of new names, then a 64th new name | 63 `CharacterInfo`; then `CharacterRefused { reason: "The character list is full." }` |
| P4 | `CreateCharacter { name: "Dora", appearance: None, class: None, nationality: None }`, then `EnterWorld "Dora"` | `CharacterInfo { name: "Dora", body: None, class: None, level: None, look: None }`; world entry uses the configured avatar |
| P5 | `DeleteCharacter "Cora"` after P1 | No event; "Cora" stays in the list |

<a id="s14-n1"></a>
### 14.N1 Vector N1: action label, quest offer with rewards (P3 revision)

```text
tessera-scenario 1
title "Vector N1"
tick 100ms
player "Testa"
actor $smith kind=npc name="Smith" pos=0,2,0 facing=180
page $greet speaker=$smith "Well met."
option $greet 1 action(@Vendor)
option $greet 2 "Farewell." goto=close
at 0s spawn $smith
on use $smith
+0s show $greet
end
on choose $greet 1
+0s offer "Iron ore" from=$smith
+0s rewards coin:1250 rep:"Smiths":300
end
input
at 0.1s use $smith
at 0.2s choose 1
at 0.3s accept
end
```

With no install the label `@Vendor` stays unresolved, so option 1 carries `Text::Literal("Vendor")`
and no note is written (§8.6, §8.7). Option 1 has no `goto`, so the trigger alone answers it; both
body lines run in the tick of the command (§7.4). Expected log:

```text
tessera-scenario-log 1
install none
tick 100
title "Vector N1"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$smith kind=non-player name="Smith" at=0.000,2.000,0.000 facing=180.00
0.100 > UseEntity entity=$smith
0.100 DialogueShown window=1 speaker=$smith text="Well met." options=[{option=1 text="Vendor"},{option=2 text="Farewell."}]
0.200 > ChooseOption window=1 option=1
0.200 QuestOffered window=2 quest="Iron ore" giver=$smith
0.200 QuestRewards window=2 entries=[{kind=coin amount=1250 what=none},{kind=reputation amount=300 what="Smiths"}]
0.300 > AnswerQuest window=2 accept=yes
0.300 QuestState quest="Iron ore" state=active
0.300 WindowClosed window=2
0.300 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

With an install whose action mapper has the label `Vendor` with key 2, option 1 is
`text=action(2)` and nothing else changes.

<a id="s14-n2"></a>
### 14.N2 Vector N2: vendor, buy-back, sell (P3 revision)

```text
tessera-scenario 1
title "Vector N2"
tick 100ms
player "Testa"
actor $trader kind=npc name="Trader" pos=3,0,0 facing=270
item $knife "Knife"
at 0s spawn $trader
+0s give $knife
on use $trader
+0s vendor items="Bread","Rope" npc=$trader price=12,250 stock=5
+0s buyback items="Lamp" price=40
end
on buy entry=2
+0s wallet coin 750
end
on sell $knife
+0s remove $knife
+0s wallet coin 1750
end
on buy-back entry=1
+0s wallet coin 1710
end
input
at 0.1s use $trader
at 0.2s buy 2 qty=3
at 0.3s sell $knife
at 0.4s buy-back 1
at 0.5s close vendor
end
```

Expected log (`Buy`, `Sell` and `BuyBack` have no generic answer; the triggers script the money):

```text
tessera-scenario-log 1
install none
tick 100
title "Vector N2"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$trader kind=non-player name="Trader" at=3.000,0.000,0.000 facing=270.00
0.000 ItemAdded item=$knife what="Knife" at=backpack:0 quantity=1
0.100 > UseEntity entity=$trader
0.100 VendorShown window=1 vendor=$trader entries=[{entry=1 what="Bread" price=coin:12 stock=5},{entry=2 what="Rope" price=coin:250 stock=5}]
0.100 BuybackShown window=1 entries=[{entry=1 what="Lamp" quantity=1 price=coin:40}]
0.200 > Buy window=1 entry=2 quantity=3
0.200 WalletChanged currency=coin amount=750
0.300 > Sell window=1 item=$knife quantity=1
0.300 ItemRemoved item=$knife
0.300 WalletChanged currency=coin amount=1750
0.400 > BuyBack window=1 entry=1
0.400 WalletChanged currency=coin amount=1710
0.500 > CloseWindow window=1
0.500 WindowClosed window=1
0.500 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-11"></a>
### 14.11 Test-server selection cases (revision 6)

The test-server backend with a **test double** of the server at minor 1 (R9 also at minor 0)
that records every client message, in `InWorld` with player entity 7. Before each case it
has delivered `EntityCreated` for entity 43 (kind `NonPlayer`, name `Wanderer 1`) and
entity 44 (kind `Player`), and the backend has streamed static 900 (`StaticSpawned`), each
in an earlier poll than the case's first command. "Next poll" is the first poll after the
command. Moves and heartbeats are not counted as messages.

| # | Commands and server traffic | Expected |
|---|---|---|
| R1 | `SelectTarget { target: Some(43) }` | `Ok`; the next poll's first event is `TargetChanged { entity: 7, target: Some(43) }`; the double receives no message |
| R2 | `SelectTarget { target: None }` | Next poll: `TargetChanged { entity: 7, target: None }`; no message |
| R3 | `SelectTarget { target: Some(7) }` | Next poll: `TargetChanged { entity: 7, target: Some(7) }` |
| R4 | `SelectTarget { target: Some(99) }` (never created) | `Ok`; no event; no message |
| R5 | `SelectTarget { target: Some(43) }`; before the next poll the double sends `EntityDestroy` 43 | Next poll: `TargetChanged { entity: 7, target: Some(43) }`, then `EntityDespawned { id: 43 }`; no further `TargetChanged` in any later poll |
| R6 | The double sends `EntityDestroy` 44; after the poll that delivers `EntityDespawned { id: 44 }`, `SelectTarget { target: Some(44) }` | `Ok`; no event |
| R7 | `SelectTarget { target: Some(43) }`, then `SelectTarget { target: Some(44) }`, then `SelectTarget { target: Some(99) }`, all before the next poll | Next poll: exactly one `TargetChanged { entity: 7, target: Some(44) }` |
| R8 | `SelectTarget { target: Some(900) }`; after its answer, the backend clears its statics (`StaticsCleared` delivered), then `SelectTarget { target: Some(900) }` | First: `TargetChanged { entity: 7, target: Some(900) }`; second: no event |
| R9 | R1 at minor 0 | As R1 |
| R10 | `SelectTarget` in `Connected` (before world entry); `UseSkill`, `CancelCast`, `SetQuickslot` in `InWorld` | `NetError::InvalidState`; each of the others `NetError::InvalidCommand`; no message |

---

<a id="s14-12"></a>
### 14.12 Vector H: fellow out of view, synchronised start, player's commands (P4 social)

```text
tessera-scenario 1
title "Vector H"
tick 100ms
player "Tam"
actor $brin kind=player name="Brin" pos=3,0,0 facing=270
at 0s spawn $brin
+0s fellowship player $brin "Cael" leader=player
+0s fellow-vital "Cael" health 300/500
+0s vital $brin health 800/800
+0.2s perform $brin "Reel" sync
+0s perform "Cael" "Reel" sync
at 1s perform-withdraw "Cael"
+0s perform "Cael" "Jig" sync
at 1.5s perform-start by="Cael" lead=1s
on play-sync
+0s say $brin "Ready when you are."
end
on play-start
+0s fellow-vital "Cael" power 50/100
end
input
at 0.5s play-sync "Reel"
at 2s play-start
end
```

Expected log. "Cael" has no entity, so its vitals come as `FellowVitalChanged` (§6.7). At
0.500 the player's `ReadyPerformance` fires the first trigger; it has no generic event. At
1.500 three parts wait (Brin "Reel", Cael "Jig", the player's "Reel"); the start is
attributed to "Cael", and *S* = 1.500 + 1.000 = 2.500 on the virtual clock. At 2.000 the
player's `StartPerformance` gets the generic answer (lead 700 ms, *S* = 2.700) before the
trigger body (§7.4 step 1), although nothing waits any more. The last input is processed at
2.000 and the agenda is then empty (§7.3).

```text
tessera-scenario-log 1
install none
tick 100
title "Vector H"
pre > Connect
pre Connected characters=["Tam"]
pre > EnterWorld character="Tam"
0.000 WorldEntered entity=player name="Tam" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$brin kind=player name="Brin" at=3.000,0.000,0.000 facing=270.00
0.000 FellowshipChanged members=[{name="Tam" entity=player leader=yes},{name="Brin" entity=$brin leader=no},{name="Cael" entity=none leader=no}]
0.000 FellowVitalChanged member="Cael" vital=health current=300 max=500
0.000 VitalChanged entity=$brin vital=health current=800 max=800
0.200 PerformanceReady member="Brin" entity=$brin tune="Reel"
0.200 PerformanceReady member="Cael" entity=none tune="Reel"
0.500 > ReadyPerformance tune="Reel"
0.500 ChatMessage channel=say sender="Brin" text="Ready when you are."
1.000 PerformanceWithdrawn member="Cael"
1.000 PerformanceReady member="Cael" entity=none tune="Jig"
1.500 PerformanceSyncStart by="Cael" at_ms=2500 lead_ms=1000
2.000 > StartPerformance
2.000 PerformanceSyncStart by="Tam" at_ms=2700 lead_ms=700
2.000 FellowVitalChanged member="Cael" vital=power current=50 max=100
2.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

For P4 exit criterion 3 a client test plays this vector (or a scenario like it with tunes
from a test music folder) and checks the parts started at 1.500 against `instruments.md` §8.9
with *S* = 2.500.

<a id="s14-13"></a>
### 14.13 P4 social: hostile rows and backend cases

Hostile rows (as §14.5: line 1 is `tessera-scenario 1`):

| Input | Expected error |
|---|---|
| line 2 `at 0s perform-start lead=11s` | `OutOfRange` 2:21 |
| line 2 `at 0s perform-start by=$ghost` | `UndefinedBinding` 2:21 |
| line 2 `at 0s perform "Ann" "Tune"` | `UnexpectedToken` 2:15 (a STRING member only with `sync`) |
| line 2 `at 0s perform "Ann" "Tune" sync delay=1s` | `UnexpectedToken` 2:33 (`delay=` with `sync`) |
| line 2 `at 0s fellow-vital "Ann" health 5/4` | `OutOfRange` 2:33 (CUR above MAX) |
| line 2 `at 0s fellow-vital player health 1/1` | `UnexpectedToken` 2:20 (an entity reports through `vital`) |
| overlay run (§12.5.1), line 2 `at 0s perform-start` | `NotAllowedInOverlay` 2:1 |
| overlay run, lines 2–3 `on play-start`, `end` | `NotAllowedInOverlay` 2:1 |

Backend cases (test doubles as in §14.9; no scenario file unless stated):

| # | Backend | Commands | Expected |
|---|---|---|---|
| S1 | test-server double, minor 2, `CHAT_CHANNELS` | `JoinChannel "Lore"`; the double answers `ChannelJoined "Lore"` | one `ChannelJoin "Lore"` sent; event `ChannelJoined { channel: "Lore" }` |
| S2 | as S1, joined | `Chat { Named "Lore", "hi" }`; the double echoes | one `ChatSend` channel 6 sent; event `ChatMessage` on `Named { name: "Lore" }`, sender the player's name |
| S3 | as S1 | `JoinChannel "lore"`; the double answers `ChannelJoined "Lore"` (its spelling) | event `ChannelJoined { channel: "Lore" }` |
| S4 | as S1 | `JoinChannel "Lore"` twice; the double answers `ChannelJoined`, then `ChannelRefused(ALREADY_JOINED)` | one `ChannelJoined` event, then one System `ChatMessage`; nothing else |
| S5 | test-server double, minor 1 (or minor 2 without `CHAT_CHANNELS`) | `JoinChannel "Lore"`, `LeaveChannel "Lore"`, `Chat { Named "Lore", … }` | each refused with `NetError::InvalidCommand`; nothing sent |
| S6 | test-server double, minor 2, `CHAT_CHANNELS` | `Chat { Fellowship, … }`, `ReadyPerformance { "Reel" }`, `StartPerformance`, `DisbandFellowship`, `InviteToFellowship { "Brin" }` | each refused with `NetError::InvalidCommand`; nothing sent |
| S7 | plain sandbox, character "Testa" entered at `now` = *t*₀ | `StartPerformance` at *t*₀ + 1,234.5 ms | `PerformanceSyncStart { by: Some("Testa"), at_ms: 1,934, lead_ms: 700 }` |
| S8 | plain sandbox | `ReadyPerformance { "Reel" }`, `DisbandFellowship`, `JoinChannel "Lore"` | no event |
| S9 | `tessera-server` in process, two clients at minor 2 | A joins "Lore", B joins "lore", A sends "hi" on it, B leaves, A sends "again" | A: `ChannelJoined "Lore"`, then its echo of "hi", then its echo of "again"; B: `ChannelJoined "Lore"`, "hi" from A, `ChannelLeft "Lore"`, and not "again" (P4 exit criterion 2) |

<a id="s15"></a>
<a id="s14-t1"></a>
### 14.T1 Vector T1: travel rows, route, travel end, note type, empty marker set (P5 travel)

```text
tessera-scenario 1
title "Vector T1"
tick 100ms
player "Testa"
actor $groom kind=npc name="Groom" pos=0,3,0 facing=180
select $regions = map-region all
at 0s spawn $groom
+0s marker $camp 4,5,0 "Camp" note-type=7
+0s markers $maps $regions named
on use $groom
+0s travel-list dest="Low road","High pass","Ferry" from=$groom coin=0,250 min-level=1,40 undiscovered=3 unusable=3 swift=2 area="Vale"
end
on choose-travel entry=2
+0s close travel
+0s travel "High pass" 2s route=swift
end
on dismount
+0s travel-end
end
input
at 0.1s use $groom
at 0.2s choose-travel 2
at 0.5s dismount
end
```

With no install `$regions` resolves empty, so the `markers` step emits nothing and is not a
skip; `note-type=7` is a number, so it needs no install (a committed scenario would write a
label, §13 rule 6). `coin`, `min-level` and `area` repeat their last value for entry 3.
Expected log:

```text
tessera-scenario-log 1
install none
tick 100
title "Vector T1"
resolve $regions line=6 family=map-region requested=all candidates=0 got=0 note=no-install
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$groom kind=non-player name="Groom" at=0.000,3.000,0.000 facing=180.00
0.000 MarkerAdded marker=1 location=0:4.000,5.000,0.000 label="Camp" icon=none quest=none note_type=7
0.100 > UseEntity entity=$groom
0.100 TravelShown window=1 source=$groom entries=[{entry=1 destination="Low road" coin=0 min_level=1 area="Vale"},{entry=2 destination="High pass" coin=250 min_level=40 route=swift area="Vale"},{entry=3 destination="Ferry" coin=250 min_level=40 flags=undiscovered+unusable area="Vale"}]
0.200 > ChooseTravel window=1 entry=2
0.200 WindowClosed window=1
0.200 TravelStarted destination="High pass" duration_ms=2000 route=swift
0.500 > Dismount
0.500 TravelEnded
0.500 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-t2"></a>
### 14.T2 Vector T2: finder rows, join options, queue, mount class and mount vital (P5 travel)

```text
tessera-scenario 1
title "Vector T2"
tick 100ms
player "Testa"
actor $warden kind=npc name="Warden" pos=2,0,0 facing=270
at 0s spawn $warden
+0s instances "Old mine","Deep vault" from=$warden featured="Deep vault" level=20,45 size=3,6 tier=1 locked=2
on join-instance entry=1
+0s instance-queue queued "Old mine" seconds=0
end
on leave-queue
+0s instance-queue none
+0s close instances
end
at 1s mount player "Grey steed" kind=combat
+0s mount-vital player morale 800/1000
+0.5s mount player none
input
at 0.1s join-instance 1 size=3 tier=1 level=20
at 0.3s leave-queue
end
```

`JoinInstance` and `LeaveQueue` have no generic answer; the triggers script the queue. The last
input line is processed at 0.300, and the agenda empties at 1.500. Expected log:

```text
tessera-scenario-log 1
install none
tick 100
title "Vector T2"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$warden kind=non-player name="Warden" at=2.000,0.000,0.000 facing=270.00
0.000 InstanceListShown window=1 entries=[{entry=1 instance="Old mine" level=20 size=3 tier=1},{entry=2 instance="Deep vault" level=45 size=6 tier=1 locked=yes}] source=$warden featured="Deep vault"
0.100 > JoinInstance window=1 entry=1 size=3 tier=1 level=20
0.100 InstanceQueueChanged state=queued instance="Old mine" seconds=0
0.300 > LeaveQueue
0.300 InstanceQueueChanged state=none instance=none seconds=none
0.300 WindowClosed window=1
1.000 MountChanged entity=player mount="Grey steed" combat=yes
1.000 MountVitalChanged entity=player vital=morale current=800 max=1000
1.500 MountChanged entity=player mount=none
1.500 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

<a id="s14-t3"></a>
### 14.T3 Resolver case T3: marker sets over map regions (P5 travel)

A headless run with a synthetic resolver that has an install and answers the `map-region`
query with three candidates: (0x78000010, row 0, `Location` 2:100,200,0), (0x10000500, row 3,
`Location` 1:5,6,0) and (0x10000400, row 1, no location), with `display_name` "Pit", "Vale"
and "North" for them. Canonical order (§8.4) is 0x10000400, 0x10000500, 0x78000010. The
spawn is the exact default of §3.2, and the resolver gives no character details.

```text
tessera-scenario 1
title "Vector T3"
tick 100ms
player "Testa"
select $regions = map-region all
at 0s marker $camp 4,5,0 "Camp"
+0s markers $maps $regions named note-type=7
+0.5s markers $maps $regions "Again"
+0.5s marker-remove $maps
```

The single marker `$camp` takes id 1 (*M* = 1), so run-time ids start at 2; the element
without a location adds nothing. Expected log:

```text
tessera-scenario-log 1
install present
tick 100
title "Vector T3"
resolve $regions line=5 family=map-region requested=all candidates=3 got=3
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 MarkerAdded marker=1 location=0:4.000,5.000,0.000 label="Camp" icon=none quest=none
0.000 MarkerAdded marker=2 location=1:5.000,6.000,0.000 label="Vale" icon=none quest=none note_type=7
0.000 MarkerAdded marker=3 location=2:100.000,200.000,0.000 label="Pit" icon=none quest=none note_type=7
0.500 MarkerRemoved marker=2
0.500 MarkerRemoved marker=3
0.500 MarkerAdded marker=4 location=1:5.000,6.000,0.000 label="Again" icon=none quest=none
0.500 MarkerAdded marker=5 location=2:100.000,200.000,0.000 label="Again" icon=none quest=none
1.000 MarkerRemoved marker=4
1.000 MarkerRemoved marker=5
1.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
```

This log names no record and holds only invented text, but it is not an `install none` log, so
it is a test vector of `tessera-net`, never a committed log (§10.5).

## 15. Invariants

1. `tessera-net` never interprets a `DataId` and has no dependency on a data-decoding crate;
   `tessera-server`'s graph contains neither `tessera-gamedata`, nor a data-decoding crate,
   nor `tessera-net` with feature `scenario` (§3.1).
2. Every number in an emitted event comes from the scenario file, from the resolved
   scenario's DIDs, or from the feed's deterministic counters; none is computed from game
   data.
3. The parser and the feed never panic and allocate within the limits of §11.1.
4. A selector never fails a run by resolving short or empty; only `InstallUnreadable`,
   `InstallRequired` and `ResolveLimit` are resolve errors.
5. With no install every selector is empty, and every synthetic scenario plays to the end.
6. Same file, same install, same options: same canonical log (§10.1).
7. Within a tick, events follow §7.4: commands first, then the agenda by (time, class,
   sequence).
8. Committed scenarios contain no DID and no game text; committed logs are `install none`.
9. In an overlay run (§12.5) the server receives nothing from the feed but `AppearanceSet`s,
   at most one in flight and at most 4 per second; inventory, levels, stats and item
   properties never leave the client.
10. Every value of a `PropertyChanged`, `ItemProperty` or look entry the feed emits is the
    value written in the file (or a `DataId` of a resolved element); the install supplies
    only property IDs, kinds and label keys (§5.8).
11. (Revision 6.) A selection on the test-server backend never reaches the server: no client
    message is caused by `SelectTarget`; the backend's only `TargetChanged` events answer a
    `SelectTarget`, name the player as `entity`, and lie between the target's creation and
    removal in the event stream (§9.7).
12. (P4 social) A `PerformanceSyncStart`'s `at_ms` is the virtual time of the tick that emits
    it plus its `lead_ms`, and `lead_ms` is the value written (700 for the generic answer);
    every `FellowVitalChanged` value is the value written. After a `PerformanceSyncStart` the
    feed holds no waiting part.
13. (P4 social) The test-server backend sends nothing for a Social command except
    `ChannelJoin`, `ChannelLeave` and `ChatSend` channel 6, and those only at minor ≥ 2 with
    `CHAT_CHANNELS`; an overlay run sends the server nothing new (§12.5.5).
14. (P5 travel) Every travel row value (cost, minimum level, flags, route, area), every
    instance row value (level, size, tier, locked), queue state and time, mount class and
    mount vital the feed emits is the one written in the file; the feed reads none of them
    from the install and never derives one from another.

---

<a id="s16"></a>
## 16. Open questions

> **Answered (revision 4, 2026-10-10, decision 0025 D2; was OPEN, non-blocking, P1):**
> `CreateCharacter` gains `nationality` and its appearance carries the look (slider values
> and every other choice a creation control writes, §5.8, §9.1, §9.4); the input form
> `create-character` takes `nationality=` and `look=` (§7.7), and `accept-character` takes
> `look=` (§6.1). A generated name travels as the command's `name`.

> **OPEN:** (non-blocking, P4) The synchronised start tolerance of `PerformanceStarted` is
> taken from `instruments.md` when P4 specifies the synchronised `/play`; v1 carries only the
> scripted delay.
>
> **Answered ("P4 social", 2026-10-11, decision 0035 D4):** a synchronised `/play` is the
> waiting parts and the one start of §6.7 (`PerformanceReady`, `PerformanceSyncStart` with
> `at_ms` and `lead_ms`, `PerformanceWithdrawn`); the tolerance is `instruments.md` §8.9,
> referred to and not restated. `PerformanceStarted` keeps its meaning (a scripted delay, no
> tolerance).

> **OPEN:** (non-blocking, "P4 social", for the owners of `instruments.md` and
> `social-panels.md`) Two readings this revision asks of them: (1) `instruments.md` §12 P4
> Decision 3 starts on the local `/playstart`; with a backend that accepts
> `StartPerformance` the client starts only on the backend's `PerformanceSyncStart` (§6.7),
> using its `lead_ms` as the lead, so one `/playstart` gives one start; and a
> `PerformanceReady` for the player acts as an accepted `/play … sync`. (2) `social-panels.md`
> TR3 shows member rows' vitals from `FellowVitalChanged` too (§6.7), and Q12's disband action
> sends `DisbandFellowship`. Level, defeat and effects of a member **without** an entity have
> no event yet; their row fields stay empty in the sandbox. If the owner's comparison (O130)
> shows they matter, a later revision adds events like `FellowVitalChanged` for them.

> **OPEN:** (non-blocking, P2) Golden images may need captures at several times within one
> run (for example mid-cast). v1 captures only the frame after the last tick; a later
> revision may add an input-script line `at T capture NAME`.

> **OPEN:** (owner policy; non-blocking for P0, to be decided before P1; listed for the owner
> in decision 0025, which does not decide it) Whether committed
> scenarios may use numeric enum keys that an Accepted spec records (for example a slot key),
> instead of labels only (§13 rule 2). Since revision 4 the question also covers property
> values written `pid:N:KIND` and numeric values of the kinds `enum`, `bitfield32`,
> `bitfield64` (§5.8, §13 rule 2). Until decided, labels only, and labels never resolve
> without an install (§8.7). Consequence: the no-install CI replay (§10.5) cannot exercise
> `equip` with a slot (the step and the `EquipItem` generic answer), damage types, or class,
> species and sex details. Create-character itself is covered, since its class and
> appearance may be absent (§7.7, §9.4; Vector F). Options: (a) allow opaque numeric keys
> in committed files (`slot=3` names no record and no text); (b) keep labels only and cover
> these paths by local real-data runs.

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of
> `tessera_net::scenario`, 2026-10-09) Points this text leaves open, implemented
> conservatively as below; confirm or change each: (1) §10.3 writes `Container` values as
> lower-case words joined by `-` (`shared-storage`) but its `ItemPlace` example says
> `shared:3`; the implementation writes `shared` in both `ItemPlace` and `ContainerSize`.
> (2) A fan-out `give` whose container runs out of free slots stops at the first element
> without one and counts one skip for the step. (3) "Character steps" in pre-world trigger
> bodies (§7.5) are taken to be `accept-character` and `refuse-character` only, so an
> `on delete-character` body can hold no step. (4) `face $a player` turns toward the
> player's current position as the feed knows it (the spawn, or the last `teleport`
> step's destination), with §5.4's formula. (5) A repeated option number on one page is
> `UnexpectedToken` at the number. (6) A command naming a window that is not the open one
> of its kind, an unknown item, or an entry beyond the window's list fires no trigger,
> filtered or not, and gets no generic answer; an already taken loot entry counts as
> unknown (`loot-taken` on it is a skip; `TakeLoot` naming it fires no trigger); a
> `TakeLoot` of all entries names the window, not an entry, so it fires a matching trigger
> while the loot window is open, even with every entry taken. (7) `EquipItem`, `UnequipItem` (only for an item
> in `equipment`), `MoveItem`, `SplitStack`, `DestroyItem` and the `equip` / `unequip`
> steps act only on placed items; an unplaced one gives nothing (a step: a skip);
> `remove`, `quantity` and `condition` of an unplaced item still emit their event.
> (8) Settings and declarations inside any block are `NotAllowedHere` at their keyword; a
> `+` timing inside `input` is `UnexpectedToken`, except before `repeat`, which is a block
> opener inside `input` (`NotAllowedHere` at `repeat`, §4.4); with two blocks open at end of file,
> `UnclosedBlock` names the outer one. (9) A `repeat` count of 0 and `every 0ms` are
> `OutOfRange`; a top-level repeat whose last iteration passes the horizon or `end-at` is
> `OutOfRange` at its `every` duration. (10) `species=` without `sex=` (or the reverse) is
> `MissingArgument`; `body=` together with either is `UnexpectedToken` at the
> `species=`/`sex=` key. (11) An `AgendaOverflow` while scheduling a trigger body carries
> the trigger's `on` line. On any run-time error the feed's undelivered output is
> discarded; `Disconnected` follows the events already delivered. A push checks the queue
> limits before the run total. (12) Input lines whose command the session refuses in that
> state (a character command at a time, another command `at pre`) are no static error:
> they are counted as `input-skipped` (§7.7). (13) The generic `Chat` answer honours
> `SandboxConfig::echo_chat` for non-tell channels, as the sandbox's own chat path does.
> (14) `accept-character` always emits `CharacterInfo`, even with no detail. (15) The
> driver's mirror keeps the speaker, text and options of the last `DialogueShown` of the
> open dialogue window; `choose $page N` is sent when `$page`'s content equals them
> (§7.7). The event carries no page identity, so pages with identical content (for
> example differing only in `goto=`) cannot be told apart: any of them matches.
> (16) The `say` step rejects `channel=tell:NAME` (§6.4 lists bare `tell` only). (17) A
> header line with a third token, or a file of blank lines only, is `BadHeader` (at the
> token, or 1:1). (18) In a windowed run that finished `Complete` without `end-at`,
> reaching the horizon tick makes the run terminal (no further ticks).

> **OPEN:** (non-blocking; for the spec owner, raised by the implementer of
> `tessera-gamedata`'s selector resolution, 2026-10-09) Points §8.3 and §8.6 leave open,
> implemented conservatively as below; confirm or change each: (1) **Table locations.** A
> `place` or `map-note` element whose position lacks a region or landblock, or has region 0,
> a landblock index 0xFF, cell 0xFFFF or a non-finite offset (invalid, `coordinates.md` §4)
> gets neither location nor anchor; otherwise the location is region-global (160 · index +
> block-local *x*, *y*; *z* as stored) and an anchor (block and cell) is added only for an
> interior position (cell ≠ 0). (2) **`display_name` with a row** carries no family: the
> resolver answers with the places-table entry's name when that row's `area` equals the DID;
> when the map-notes row's entity equals the DID, with the entity record's display name
> (§8.6 "the record's display name"; the row's own text is not used, and link rows have
> none); else nothing. Should the row's text serve when the entity has no name?
> (3) **`display_name` without a row** uses the family's own name property first:
> `Skill_Name`, `Effect_Name`, `Trait_Name`, `Quest_Name` (§5.2), `TravelDisplayName`
> (`map-data.md` §8.3), `Area_Name` or `Dungeon_Name` (`wstate.md` §11),
> `Reputation_Faction_Name` (`quests.md` §13.1), `CraftRecipe_Name`, `CraftProfession_Name`
> (`economy.md` §5.1, §5.2), `PrivateEncounterTemplate_Name` (`instances.md` §4.2); then
> `Name`. No Accepted spec names a display-name property for `trait-tree`, `class`, `race`,
> `npc-comment`, `title`, `emote`, `vendor-inventory`, `trophy-list` or `induction`, nor for
> the recipes without `CraftRecipe_Name` (most of them, `economy.md` §5.1): those give no
> name unless they carry `Name`. `Title_String` and `Emote_CommandString` (`quests.md`
> §12) are candidates, but the specs do not call them names. Names are cleaned with the
> `sanitize_text` rule (control character → space, cut to 64 bytes; nothing trimmed).
> (4) **`skill class=` and `mount`** use only the `AdvTable_Skill` field of each
> `AdvTable_AvailableSkillEntry` (the entry's own skill, not every DID inside the list,
> which also holds prerequisite skills); a list without such fields lists no skill. (5) **Bitfield attributes**
> (`slot`, `fits`, `type`, `genus`, map-note `type`) test the bit of key *v* in the low 64
> bits; a key above 64 matches nothing. (6) **`did`** finds a DID when it is a member of a
> record family of §8.3; table elements (whose DIDs are area or entity fields) are not
> searched. (7) **`sex=m` / `f`** resolve to the `RaceTable_GenderType` values that
> `appearance.md` §2.1 records as male and female (0x1000, 0x2000), each only when some race
> table of the install has a gender entry of that value; otherwise they stay unresolved.
> These two values are observations, not an install-derived rule: is there one (for example
> the labels of the gender mapper)? (8) **`has-icon`** is the presence of any property
> whose dictionary label starts with `Icon_Layer_`, a null DID included (`icon-layers.md`
> §2.2 counts a layer as shown only when non-zero). (9) **`hotspot` and `pet`** report
> `Unsupported` (no decoder view yet, §8.3 last paragraph); `induction` is resolved through
> the combat-data related-object class (`combat-data.md` §7.9). (10) **Recipe `tier`**
> matches the top-level `CraftRecipe_Tier` or any `CraftRecipe_Tier` element of
> `CraftRecipe_TierArray` (`economy.md` §5.1 rule 1; §8.2 "for a list, one of its
> elements").

> **OPEN:** (implementer note, "P4 social", `tessera-net`, 2026-10-11; non-blocking)
> Readings the implementation of the "P4 social" revision had to choose; a reviewer should
> confirm or correct them. (1) **`ReadyPerformance` under `defaults off`.** §6.7 says the feed
> records the player's part on `ReadyPerformance` and puts "under `defaults on`" only on the
> `StartPerformance` answer, but §7.6 lists the recording among the generic answers and §5.7
> says a command the feed does not answer changes nothing. The code follows §5.7 and §7.6: with
> `defaults off` a `ReadyPerformance` records nothing (a later `perform-withdraw player` then
> skips). (2) **`perform-start by=ENTITY` and `perform-withdraw ENTITY`** take the player's or
> actor's name and never skip for an actor that is not spawned (they carry no entity), while
> `perform ENTITY … sync` skips like `fellowship` when the entity has no ID. (3) **Plain
> sandbox with a finished scenario.** Once a scenario's feed has ended, the sandbox answers
> `StartPerformance` as the plain sandbox of §9.8 does (as its chat path already answers chat
> after the feed's end); §9.8 speaks only of "no scenario". (4) **Test-server backend, channel
> answers** (`tessera-protocol.md` Appendix A): a `ChannelRefused` System line names the
> channel as the client sent it (the request it answers), not the refusal's sanitised `name`
> field; a channel request the protocol layer cannot send (the connection already closing, or
> its 64 outstanding channel requests, `tessera-protocol.md` §17.4) gives one System line in
> Tessera's words instead of an answer; `ChannelLeft(REMOVED)` gives `ChannelLeft`, then a
> System line. (5) **Inbound `PerformanceStarted.tune`** is still cut to the chat length (1,024
> bytes) as before this revision; the new events cut `tune` to 256 bytes (§9.5). §9.5 could
> state the 256-byte cut for `PerformanceStarted` too.

> **OPEN:** (non-blocking; P5 travel, for the spec owner) (1) The owner question on numeric
> enum keys above now also covers `note-type=`, `size=` and `tier=` (§13 rule 6): until it is
> decided, committed scenarios cannot show a map-note icon choice, a group size or a tier
> without an install. (2) `MountChanged.combat` is a Tessera field: the original decides the
> mount object's class from the rider's mount type (`lua-plugin-api.md` §15.5), which a server
> would send as a property; if a later revision carries the mount type itself, `combat`
> becomes a fallback. (3) `travel-windows.md` §7.1 names a feed `SystemMessage` for refused
> travel; the feed has none: a refusal is a `system` step (`ChatMessage` on `System`).

> **OPEN:** (non-blocking; P5 travel, for the spec owner, raised by the implementer of
> `tessera_net::scenario`, 2026-10-11) Points the P5 travel parts leave open, implemented
> conservatively as below; confirm or change each: (1) **Value-list items.** §4.2 makes a
> token that is not a number `BadNumber` in a numeric position, while the §11.1 row for value
> lists says "another item form `UnexpectedToken`" (T-H5). Implemented for `min-level=`,
> `level=`, `size=` and `tier=` lists: an item that does not start with a digit or `-` (and,
> for `size`/`tier`, not with `@`) is `UnexpectedToken`; a number-shaped item that is not an
> integer (`1.5`, `0x10`) is `BadNumber`. Entry lists (`undiscovered=` … `locked=`) and the
> single values of the input form `join-instance` follow §4.2 (`x` is `BadNumber`).
> (2) **`area=` limit.** 1,025 refs cannot fit in one 4,096-byte line (the shortest ref,
> `"v"`, takes 4 bytes with its comma), so `LimitExceeded(entries)` for `area=` is
> unreachable: `LineTooLong` comes first. (3) **`markers … named` names** are fetched once
> per selector binding (at the first such step in line order), only for the elements with a
> `Location` among its first 65,536, always with the row (`display_name(did, Some(row))`,
> whatever the family); later steps over the same binding reuse them. The log cannot tell
> the difference; a stateful resolver can. `map-region` elements are table elements for
> actor `name=` too (asked with their row, like `place` and `map-note`). (4) **`instance-queue
> STATE none`**: `none` is not a REF; it is `UnexpectedToken` (write the step without a
> REF). (5) **Weight** (§7.4): `TravelEntry.area`, `InstanceListShown.featured` and
> `InstanceQueueChanged.instance` count ⌈b/64⌉ for a literal, like other refs. A `markers`
> step reserves its removals and additions before emitting any (so a queue overflow is
> raised before the set changes). (6) **Errors of one line** follow the option order of the
> §6.5 table, not their columns, as for the existing steps. (7) The session's warning for a
> dropped invalid World event names a marker location whatever the cause (text only).

---

<a id="s17"></a>
## 17. Change log

| Date | Change |
|---|---|
| 2026-10-09 | First draft (P0, decision 0024 §4): format, model, step vocabulary, timing and triggers, selectors with SplitMix64 sampling, domain events and commands, canonical log, limits, client integration, content rules, vectors. |
| 2026-10-09 | Review (spec reviewer, separate session): 31 design findings fixed in place (§18.1: R1–R31), Vector E and six hostile rows added; Status set to Accepted, though the same session wrote the fixes. |
| 2026-10-09 | Revision 3 (spec owner, design role): 31 implementation-phase findings verified (§18.2: S1–S31). Fixed: no welcome line and the feed owns chat (§3.2, §7.6); exact headless spawn and the resolve timeout in force (§3.2, §10.1, §12.2); pre-world output in `submit` (§3.3, §5.1, §7.5); *L* from the processing tick and input lines within `end-at` (§7.3, §7.7); queue weight, per-push checks, run total, effect cap, pre-world bounds, 8 ticks per poll (§7.1, §7.4, §7.5, §11); start = executing tick (§6.2); inventory edge cases (§5.5, §6.1, §7.6); block error kinds (§4.4); numeric components (§4.2); trade side 32 (§6.3, §8.6); input ranges (§7.7); optional create-character class and appearance (§7.7, §9.4); (DID, row) identity (§8.4); `CandidateOutcome` replaces `label_notes` (§8.6); remote chat channels (§9.5); heading `360.00` (§10.3); headless modes and `--screenshot` (§12); invented example name (§4.2). Vector F and eleven hostile rows added. Status back to Review. |
| 2026-10-10 | Revision 4 (spec writer, design; decision 0025): property values and looks (§4.2, §4.5, §5.8), looks on characters and in the base appearance (§5.1), held-item slots in the player's appearance (§5.5), steps `remove-character`, `stat`, `item-property` and `accept-character … look=` (§6.1), `create-character … nationality= look=` (§7.7), resolver methods `property`, `property_by_id` and contexts `Nationality`, `PropertyKey` (§8.6), new types, events and command field (§9.1–§9.4), session rules and the client-side property bound (§9.5), plain-sandbox creation (§9.6), the test-server backend's creation, looks, slots and character list (§9.7), log forms (§10.2–§10.3), limits and error kinds (§11), overlay runs with `--server` (§12.1, §12.5), content rules (§13), Vector F's new fields, Vector G, overlay and plain-sandbox cases, 15 hostile rows (§14), invariants 9–10 (§15), §16 OPEN 1 answered. Status Accepted → Review (§18.3). |
| 2026-10-10 | Independent review of revision 4 (spec reviewer, Clean-Room-Role: review): passed, every item of CLEAN_ROOM.md §5.3; status Review → Accepted. Non-blocking notes (a)–(f) in §18.3 |
| 2026-10-10 | P3 revision (revision 5; spec writer, for `npc-windows.md`, additive only, nothing renumbered): text function `action(…)` and `Text::Action` (§4.2, §5.2, §8.6, §9.1), steps `rewards` and `buyback` (§6.3), events `QuestRewards` and `BuybackShown` and command `BuyBack` (§9.1, §9.2, §9.4, §9.5), pattern `buy-back` (§7.5), input form `buy-back` (§7.7), log forms (§10.3), limits (§11.1), hostile rows N-H1 … N-H11 (§14.5), Vectors N1 and N2 (§14.N1, §14.N2), §18.P3. The parts of revision 5 are in Review (awaiting independent review); the Accepted revision-4 text is unchanged. |
| 2026-10-10 | Revision 6 (spec writer, design; [decision 0032](../decisions/0032-p2-targeting-on-test-server.md)): local selection on the test-server backend (§9.5, §9.7), no Combat group in the overlay subset stated (§12.5.1), routing of `SelectTarget` in overlay runs (§12.5.2), O8 changed and O10–O12 added (§14.9; the implementer OPEN moved below the table), test-server selection cases R1–R10 (§14.11), invariant 11 (§15). No protocol change. Status Accepted → Review (§18.4). |
| 2026-10-11 | Independent review of the P3 revision (revision 5; spec reviewer, Clean-Room-Role: review): passed, every item of CLEAN_ROOM.md §5.3; the P3 parts move Review → Accepted. Non-blocking notes (a)–(h) in §18.P3; no text other than the status, review record and this row changed |
| 2026-10-11 | Merge of the P2 and P3 revisions (spec writer, design; no fact changed): both had been called revision 5, so P2's is renumbered **revision 6** (header, §9.5, §9.7, §12.5.1, §12.5.2, §14.9 O10–O12, §14.11, §17, §18.4); P3's revision 5 and §18.P3 stay. The vector and hostile-row names of the two do not collide (P2: O8, O10–O12, R1–R10 of §14.11, invariant 11; P3: N1, N2, N-H1 … N-H11, N-D1 … N-D4). Status: revision 5 (P3 parts) Accepted, revision 6 (P2 parts) Review. |
| 2026-10-11 | Independent review of revision 6 (P2 parts; spec reviewer, Clean-Room-Role: review): passed, every item of CLEAN_ROOM.md §5.3; revision 6 moves Review → Accepted, and decision 0032 Proposed → Accepted. Non-blocking notes (a)–(e) in §18.4.1. No text other than the status, reviewer and review-record rows (rejoined onto one line each, so the header table renders), §18.4.1 and this row changed |
| 2026-10-11 | Revision 7, the "P4 social" revision (spec writer, design; [decision 0035](../decisions/0035-p4-social-on-test-server.md) D2–D4), additive, no earlier section renumbered: terms (§2), waiting parts in the feed state (§5.7), new §6.7 (`perform … sync`, `perform-start`, `perform-withdraw`, `fellow-vital`), patterns `play-sync`, `play-start`, `disband` (§7.5), generic answers to `StartPerformance` and `ReadyPerformance` (§7.6), input forms (§7.7), events `PerformanceReady`, `PerformanceSyncStart`, `PerformanceWithdrawn`, `FellowVitalChanged` (§9.2), commands `ReadyPerformance`, `StartPerformance`, `DisbandFellowship` (§9.4), session rules (§9.5), new §9.8 (test-server backend at protocol minor 2: channel join and leave and `Named` chat; plain-sandbox start answer), log forms (§10.3), limits (§11.1), overlay subset unchanged (§12.5.2), Vector H (§14.12), hostile rows and backend cases S1–S9 (§14.13), invariants 12–13 (§15), §16 P4 OPEN answered and a new OPEN for the `instruments.md` and `social-panels.md` owners, §18.5. Status Accepted → Review |
| 2026-10-11 | Independent review of the "P4 social" revision (spec reviewer, Clean-Room-Role: review): passed, every item of CLEAN_ROOM.md §5.3; status Review → Accepted. Record and non-blocking notes (a)–(c) in §18.5 |
| 2026-10-11 | Merge of the P4 revision (spec writer, design; no fact changed): the "P4 social" revision, written unnumbered, is numbered **revision 7**; its sections that collided with P2's and P3's are renumbered: Vector H §14.11 → **§14.12**, the P4 hostile rows and backend cases §14.12 → **§14.13**, the review section §18.4 → **§18.5**, invariants 11–12 → **12–13** (P2's invariant 11 stays; anchors `s14-12`, `s14-13`, `s18-5`). §6.7 and §9.8 do not collide and keep their numbers. Statuses: revision 7 (P4 social) Accepted, revision 6 (P2) Accepted, revision 5 (P3) Accepted, each as its own review recorded. |
| 2026-10-11 | P5 travel revision, numbered **revision 8** at the merge with P2, P3 and P4 (spec writer, for decision 0024 P5; additive only; its invariant is renumbered **14** and its sections keep the numbers §14.T1–§14.T3 and §18.P5): marker sets and run-time marker ids (§5.3), steps `markers`, `marker-remove` of a set, `travel-end`, `instance-queue`, `mount-vital` and the options of `marker`, `travel-list`, `travel`, `instances`, `mount` (§6.5), pattern `leave-queue` (§7.5), input forms `join-instance … size= tier= level=` and `leave-queue` (§7.7), family `map-region` and `effect` attributes `mount-type`, `war-steed` (§8.3), label contexts (§8.6), types, fields, events `TravelEnded`, `MountVitalChanged`, `InstanceQueueChanged` and command `LeaveQueue` (§9.1, §9.2, §9.4, §9.5), log forms with added fields written only off their default (§10.3), counters (§10.4), limits (§11.1), fuzz note (§11.4), rule 6 (§13), hostile rows T-H1 … T-H18 (§14.5), Vectors T1, T2 and resolver case T3 (§14.T1–§14.T3), invariant 11 (§15), one OPEN (§16), §18.P5. The P5 parts are in Review; every other part keeps its status. |
| 2026-10-11 | Independent review of the P5 travel revision (spec reviewer, Clean-Room-Role: review; a session that wrote no part of the P5 revision or the P5 code): passed, every item of CLEAN_ROOM.md §5.3; the P5 parts move Review → Accepted. Non-blocking notes and the dependency on `map-windows.md`/`travel-windows.md` (in Review) in the header review record and §18.P5; no text other than the status lines, review record and this row changed |
| 2026-10-13 | Merge of the P5 revision (spec writer, design; no fact changed): the "P5 travel" revision, written unnumbered, is numbered **revision 8**; its invariant 11 (the feed derives no travel, instance or mount value) is renumbered **14** because main's invariants 11–13 (P2, P4) came first; Vectors T1–T3 (§14.T1–§14.T3) and the review record §18.P5 keep their numbers (no collision with main's §14.11–§14.13 and §18.4–§18.5). Code and test comments that cite invariant 11 for the P5 rule, or the revision as "P5 travel", follow. |

---

<a id="s18"></a>
## 18. Review

<a id="s18-1"></a>
### 18.1 Revision 2 review

**Review of 2026-10-09** (spec reviewer, Clean-Room-Role: review; a fresh session, not the
author). Checked against decision 0024 §2 (2.3 data-free server, 2.4 scripted values never
derived, rules stay T3) and §4; LEGAL.md §11.1 rule 5; the CLAUDE.md content rules; ARCHITECTURE
§19.1; the `tessera-net` API as it is on branch `p0` (`session.rs`: `Event`, `Command`,
`Backend`, `Session::poll_into` acceptance and cleaning; `model.rs`; `sandbox/mod.rs`: entry
order, `emit`, `begin_enter`/`begin_teleport`, chat echo, defaults); `cargo tree -p
tessera-server` (workspace crates in its graph: `tessera-server`, `tessera-protocol`); and every
spec and decoder view of §8.3.

**Mandate and legal frame: pass.** The feed plays scripted events only; no step derives a
number, outcome or text from data (§0.1, §15 invariant 2); the generic answers of §7.6 are
acknowledgements that check nothing from the data. `tessera-net` receives only DIDs, rows,
locations, scripted values, literal text, resolved enum values and actor names; it opens no
archive and links no decoding crate (§3.3, §15). The server never links the feed, and the CI
check of §3.1 enforces it. Committed scenarios are selector-only and committed logs are
`install none` (§10.5, §13).

**Selector attributes: pass.** Every family and attribute of §8.3 was traced to its Accepted
spec section and, where named, its decoder view: `items.md` §2.4 rule 4 (`ItemKind`, the
15 keywords match its variants), §5.1, §5.2, §5.4 (stackable rule, `Equipment` label), §5.10
(`Requirements::classes`); `combat-data.md` §2.1, §6.1 (`EffectRole` names), §7.2, §7.9, §8.2,
§8.5 (`RelatedObject::TraitTree`), §9.1; `creatures.md` §2.2 (`TemplateKind`), §4.1, §4.2, §4.6
with `economy.md` §6.3 (`Vendor_InventoryList`); `quests.md` §2.1, §4.1 (`is_deed`), §4.2, §5.1,
§7.1, §7.7, §8, §12.1, §12.2, §13.1; `economy.md` §4.1, §5.1, §5.2, §6.3, §7; `map-data.md`
§2.3 (`MapKind`), §3, §4.2 (`MapNote::type_keys`), §4.6, §8.3; `instances.md` §4
(`InstanceKind::Template`); `player-character.md` §5.2, §7, §10.2 (`Role::RaceTable`,
`Role::ClassTable`); `interaction.md` §2.2, §3.8, §4, §7.1 (no decoder view yet; `Unsupported`
allowed); `appearance.md` §2, §7; `icon-layers.md` §2.2; `property-system.md` §7.3;
`records.md` §7; `lua-plugin-api.md` §15. All cited specs are Accepted. One citation used a
wrong field name (R29).

**Findings and fixes.** All are design defects; each was fixed in this revision.

| # | Finding | Fix (section) |
|---|---|---|
| R1 | Nondeterminism / hang: pre-world polls kept `now` = base, so the sandbox's resolve timeout never elapsed and a pending spawn lookup could wait forever | Virtual wait polls base + *w* · tick with a one-tick wall-clock pause, bounded by the resolve timeout; exit 5 on failure; t0 = the `WorldEntered` poll (§12.2 step 3, §10.1) |
| R2 | A run could finish while events were still held by the per-tick cap, losing them | `Finished` only once the output queue is empty (§7.3 Draining) |
| R3 | In windowed runs a scenario made only of triggers finished at tick 0 and then ignored every trigger | After `Complete` without `end-at`, windowed runs keep firing triggers; `end-at` and `Horizon` are terminal (§7.3) |
| R4 | A truncating resolver could return any subset, so the result depended on its iteration order | The resolver returns the first *limit* candidates in canonical order; undecodable records are not candidates (§8.6) |
| R5 | Labels outside selectors (`class=`, `species=`, `sex=`, `slot=`, `type=`) had no resolution path, and no cited spec names a mapper for sex | Resolver method `enum_value`; `sex=m`/`f` resolve through the race table's gender entries (`appearance.md` §2.1); `tessera-net` holds no such values (§5.1, §6.1, §6.2, §8.6) |
| R6 | "Options after the positional arguments" contradicted `page $p speaker=… TEXT` (Vector D) | Options anywhere after the first keyword; three positional key=value forms named (§4.5) |
| R7 | Integers had 19 digits but seeds go to 2^64 − 1 (20 digits) | 20 digits, exact value, then range check; enum values 0 … 2^32 − 1 (§4.2) |
| R8 | A string inside a token (`"a"b`) and the split of `key=value` with a nested `=` were undefined | String placement rule; key = text before the first `=` (§4.2) |
| R9 | Error order undefined: a hostile vector expects `UnexpectedToken` at line 3 although the block opened at line 2 is never closed | One forward pass; `UnclosedBlock` and `UnknownCharacter` are end-of-file checks, in that order (§4.5) |
| R10 | `BadDuration` for a token that is not a duration at all (`2h`, a vector) was not defined | Defined for every numeric form (§4.2) |
| R11 | Decimal → `f32` (double rounding) and the duration conversion were unspecified | `f64` nearest, then `as f32`; durations exact on the digits (§4.2) |
| R12 | Filter values per attribute type were incomplete (integer attributes, single-value ranges, `seed=` with `all`, token order in `select`) | Type table (§8.2); select token order and `seed=` rule (§4.5) |
| R13 | Label-note order within a line, the meaning of `candidates=`/`got=`, and several applicable notes were unspecified | Defined (§8.6, §10.2) |
| R14 | Behaviour for empty bindings in fan-out and window steps and in reference filters was undefined | Fan-out emits nothing (no skip); windows open empty; reference filter matches nothing (§6) |
| R15 | Window steps took only selector bindings, so committed synthetic scenarios (no install) could never show a filled vendor, loot, trainer, travel or instance window | REFLIST entry lists of literal refs and elements (§4.5, §6.3, §6.4–§6.6) |
| R16 | `move … over` scheduled one entry per tick (up to 360,000 with a 10 ms tick, above the agenda limit); overlapping moves and the velocity unit were undefined | One self-re-arming class-0 entry, cancelled by a later `move`; exact formulas; current position and heading kept per actor (§6.1, §5.4, §7.3) |
| R17 | Sequence numbers of rescheduled entries (`delay`), a second `cooldown` on the same ref, and `effect-end`'s choice were unspecified | Re-armed with a new sequence number; replacement; highest `EffectId` (§6.2) |
| R18 | Repeat expansion order and the sequence counter were ambiguous | Iteration-major expansion, 64-bit counter, total order (§7.4) |
| R19 | Actors after a teleport into another region, `place(…)` with a missing element or no `Location`, and unplaceable actors were undefined | Entry region; skip rules (§5.4) |
| R20 | The `teleport` step could take the sandbox's ground or named path (a pending, timing-dependent lookup); its default facing was undefined | Exact destination only; current heading by default (§6.5) |
| R21 | The order of feed events against sandbox-produced events, the sandbox's chat echo doubling the feed's chat answer, and avatar precedence were undefined | §3.3 event order; echo off while the feed runs; avatar rule (§3.2) |
| R22 | Several `player` lines, duplicate names, an unbounded character list and an `enter` naming nobody were allowed | Rules, `UnknownCharacter`, `LimitExceeded(characters)` (§4.3, §5.1, §11) |
| R23 | Commands submitted between ticks were unbounded | 1,024, then `NetError::SendQueueFull` (§7.4, §11.1) |
| R24 | Identifier counters could overflow without a defined outcome | Checked; `IdOverflow` stops the feed (§5.3, §11.3) |
| R25 | Ranges were missing for `bag`, tiers, ranks, stock, trait points, profession values, trainer levels, slot numbers and enum values | Defaults sentence (§6), §5.5, §4.2 |
| R26 | Log forms were missing for `DisconnectReason`, the `Teleport` destination, anchor digits, `Location` (`f64`) rounding, `EntityKind`/`LocationCause`, and variants added later; the group enums were not `non_exhaustive` | §10.3; group enums `#[non_exhaustive]`, `Command::name` (§9) |
| R27 | `objective(…,N)` and `QuestObjective.objective` did not say whether *N* is a stored position or the objective's index | `Quest_ObjectiveIndex` (`quests.md` §5.1) (§5.2, §6.3) |
| R28 | §3.3 and §8.6 said place names serve marker labels and that `place(…)` fetches names, but `place(…)` is a position; `place` elements mostly have DID 0 | Resolved names only for actor `name(…)`; DID 0 elements described (§3.3, §8.3, §8.6) |
| R29 | The `map-note` `type` attribute cited a field `note_type` that `map-data.md` §4.2 does not have | The row's `type` field (`MapNote_Type`) (§8.3) |
| R30 | The CI deny list missed `tessera-scriptgraph` and `tessera-install` (which reads installs, rule 1) and any crate added later | Both added; an allow list SHOULD back the deny list (§3.1) |
| R31 | Smaller gaps: no `TrackQuest` input, pattern or answer; input lines with unresolved labels or refused by the session; the driver's mirror timing; `choose $page` while another page shows; input times after `end-at`; character events while closing; `Named` channel validation; `OfferTrade` quantity without an item; no vectors for inventory, generic answers, `move … over`, entry lists, `end-at` and the new errors | Added (§7.5–§7.7, §9.5); Vector E (§14.6) and six hostile rows (§14.5) |

**Checks of the vectors.** The SplitMix64 outputs and every sampling result of §14.3 were
recomputed independently and agree. Vectors A, B and D were re-derived step by step under the
revised rules and are unchanged (their columns, times, orders and counters hold); the hostile
rows were re-counted. Vector E was derived by the reviewer.

**Not changed, non-blocking.**

- ARCHITECTURE §19 does not yet list the grouped variants of §9 that this spec says it lists;
  the ARCHITECTURE update is a separate P0 deliverable (decision 0024 §5 P0 "Specs").
- The OPENs of §8.3 and §16 are non-blocking: each concerns a later milestone (P1, P2, P4) or a
  policy choice that has a safe default (labels only).
- `server-content.md`, cited for the server's rules, is Draft; the feed relies only on rules
  LEGAL.md §11.1 states itself.

**Status.** No blocking OPEN remained; the session set the status to Accepted (P0 exit
criterion 1). That approval does not count: the same session wrote R1–R31 and Vector E
(`docs/CLEAN_ROOM.md` §3), and Vectors A, B, D and E missed the sandbox's welcome line
(§18.2 S1). Superseded by revision 3.

<a id="s18-2"></a>
### 18.2 Revision 3: implementation-phase findings

**2026-10-09** (spec owner, design role). An implementation-phase review raised 31 findings.
Each was checked against this text and, where it cited code, against `crates/net/src/sandbox/`,
`crates/net/src/model.rs`, `crates/net/src/remote/` and `apps/client/src/` (`cli.rs`,
`runtime.rs`, `walker_host.rs`); heading round trips were recomputed with a throwaway script
that mirrors `heading_from_compass_degrees` and `compass_degrees_from_heading`. No client fact
was needed; every fix is a design change. Several findings were raised twice (S19 = S1,
S20 ⊃ S3, S22 = S2, S24 ⊃ S8).

| # | Finding | Verdict | Fix (section) |
|---|---|---|---|
| S1 | Vectors A, B, D, E omit the sandbox's welcome line, which `finish_enter` always sends | Real | With a scenario the sandbox sends no welcome line or other unsolicited notice; vectors unchanged (§1, §3.2, §9.3, §14) |
| S2 | *L* used the written time of the last input line, so an `at 0s` input could end the run at tick 0 before it was sent; `end-at 0s` with timed input undefined | Real | *L* = the processing tick; timed input lines must be processed by `end-at` (§7.3, §7.7); Vector F, hostile row |
| S3 | "No spawn option" waits on a ground lookup with an install (the walker's default spawn snaps to the ground) | Real | Headless runs without a spawn option use the exact default spawn whatever the install (§3.2, §10.1, §14) |
| S4 | Pre-world feed output (`CharacterInfo` after `Connected`, create-character trigger bodies) had no delivery rule, so `pre` vs `0.000` was open | Real | Handled in `submit`, queued after the sandbox's answer, logged `pre` (§3.3, §5.1, §7.1, §7.5); Vector F |
| S5 | `trade-offer` entry limits contradicted each other (1,024 vs 32) | Real | Trade side 32 in REFLISTs and binding cuts; `LimitExceeded(entries)` (§6.3, §8.6, §11.1); hostile row |
| S6 | §6 still said options follow positional arguments | Real | Sentence aligned with §4.5 (§6) |
| S7 | A heading could be written `360.00` | Real (recomputed: `facing=359.999` gives 359.99899) | `360.00` is written `0.00` (§10.3); Vector F |
| S8 | `create-character` without species and sex had no appearance to send; committed scenarios could never send it | Real | `CreateCharacter.appearance` and `.class` optional; unresolved labels do not skip that form (§7.7, §9.4, §9.5, §10.3); Vector F |
| S9 | `give` to an occupied or out-of-size slot, a second `give`, `SplitStack` with no free slot and `AppearanceChanged` without a base appearance were undefined | Real | Skip rules; no answer at all; no event, list still tracked; also `unequip`, `EquipItem`, `UnequipItem`, `MoveItem`, `TakeLoot` without room (§5.5, §6.1, §7.6) |
| S10 | The sandbox's tell notice ignores `echo_chat`, so "echo off" still doubled or leaked notices | Real | The feed takes every `Chat` while it runs; its generic answer reproduces echo and tell notice (exact text) (§3.2, §7.6); channel syntax `tell:NAME` for input and patterns (§4.2, §7.5); Vector F |
| S11 | Error kinds for a top-level `repeat` in `repeat`, `on` in `on`, a second `input` were open | Real | `NotAllowedHere` at the opener keyword; `NestingTooDeep` only from depth 3 (§4.4, §7.5); four hostile rows |
| S12 | The Label class cannot spell every mapper name | Real, not blocking | Gap accepted and stated (§8.2) |
| S13 | The queue was bounded by event count only; list-carrying events made that bound meaningless | Real | Event weight; 2,097,152 weight units queued and 65,536 delivered per tick (§7.4, §11.1, §11.4) |
| S14 | `QueueOverflow` was checked after the whole tick was built | Real | Checked on every push; fan-out and window steps check before allocating; the tick stops at once (§7.4, §11.3) |
| S15 | Trigger firings, active effect instances and pre-world output were unbounded | Real | 16,777,216 events per run (`EventOverflow`); 1,024 effect instances per entity; pre-world bodies ≤ 16 lines, ≤ 64 firings (§6.2, §7.4, §7.5, §11.1) |
| S16 | A windowed catch-up poll could process any number of ticks | Real | At most 8 ticks per poll; virtual time lags, HUD shows it; headless unaffected (§7.1, §11.1, §12.4) |
| S17 | The 32-character limit was per token, rejecting legal positions and lists | Real | Per numeric component (§4.2, §11.1); two hostile rows |
| S18 | First free slot by linear scan is O(*n*²) for a large fan-out `give` | Real | SHOULD keep an O(log *n*) free-slot structure (§5.5) |
| S19 | Same as S1 | Real | As S1 |
| S20 | As S3, and the 10 s resolve timeout is not the walker's 300 s | Real | As S3; timeout in force named (§3.2, §12.2) |
| S21 | `--scenario --headless --no-data` had no defined code path; `--screenshot` meaning changed silently; `--ui-script` unclear | Real | Driver-only mode without an install, `--screenshot` refused there; walker mode with an install and the new `--screenshot` meaning; `--ui-script` conflicts (§12.1, §12.2) |
| S22 | Same as S2 | Real | As S2 |
| S23 | The Accepted status came from the session that wrote the fixes | Real | Status back to Review; an independent session must re-check R1–R31, S1–S31 and all vectors (header, §18.1) |
| S24 | No committed scenario can exercise `equip`, damage types or class details without an install | Real, policy | Create-character covered (S8); the rest recorded as the consequence of the §16 OPEN, to be decided before P1 (§13, §16) |
| S25 | Sandbox answers to walker `Move`/`QueryLocation` could reach headless logs | Real | Headless scenario runs send neither (§10.1, §10.2, §12.2) |
| S26 | "Start" of automatic ends could be the written time or the tick time | Real | The executing tick's time (§6.2); Vector F |
| S27 | Input forms had no ranges matching the command field widths | Real | Ranges per form, `OutOfRange` at parse time (§7.7); two hostile rows |
| S28 | Duplicates with equal (DID, row) but different locations kept the resolver's order | Real | (DID, row) is the identity; a total order picks the kept one (§8.4) |
| S29 | `label_notes` read hidden state of the last call | Real | `CandidateOutcome { status, unmatched_labels }`; method removed (§8.6) |
| S30 | The remote backend had no rule for `Chat` on `Fellowship` and `Named` | Real | Refused with `InvalidCommand` until a protocol minor adds them (§9.5) |
| S31 | An example used a resource name from the game | Real | Replaced by an invented name (§4.2) |

Rejected: none. Recorded rather than
fixed: S12 (gap accepted) and the policy part of S24 (§16 OPEN).

**Status.** Review. Vectors A, B, D and E need no change under the S1 fix; Vector F is new and
was derived by the spec owner, so it, like Vector E and R1–R31, awaits independent checking.

<a id="s18-3"></a>
### 18.3 Revision 4: decision 0025

**2026-10-10** (spec writer, design role). Tessera-own design for decision 0024 P1, recorded
in [decision 0025](../decisions/0025-p1-character-look-on-test-server.md). Inputs: `docs/`
only (decision 0024, LEGAL.md §11.1, `character-screens.md`, `character-panels.md`,
`item-tooltips.md`, `property-system.md`, `player-character.md`, `tessera-protocol.md`,
`test-server.md`, `server-content.md`); no research note, clone, client or decompiler output.

| # | Change | Sections | Answers |
|---|---|---|---|
| D1 | Overlay runs: `--scenario` with `--server`, the overlay subset, routing, clock, the player's look through coalesced `AppearanceSet`s | §0, §3.1, §11.1, §12.1, §12.5, §14.5, §14.9, §15 | P1 exit criterion 3; §12.1's conflict with `--server` lifted |
| D2 | Looks: property values (`PROPERTY=VALUE`), `look` declarations, looks on characters, `CharacterInfo.look`, `Appearance.look`, `CreateCharacter.nationality`, the test-server backend at minor 1 | §2, §4.2, §4.5, §5.1, §5.8, §6.1, §7.7, §8.6, §9.1–§9.4, §9.7, §10, §13 | `character-screens.md` §7.1 OPEN (Q7); §16 OPEN 1 |
| D3 | `Equipped.slot` filled from the equipment slot | §5.5, §9.1, §10.3 | `character-panels.md` §7 OPEN |
| D4 | §9.5 and §9.7 adopt `character-screens.md` §7.1.1; `CharacterInfo` per `LoginOk` entry | §9.5, §9.7 | `character-screens.md` §7.1.1 OPEN (Q11) |
| D5 | Plain-sandbox creation; `remove-character` → `CharacterRemoved`; `stat` → `PropertyChanged`; `item-property` → `ItemProperty`; client-side property bound | §6.1, §9.2, §9.5, §9.6, §14.8, §14.10 | `character-screens.md` §7.2 (feed) and §8 OPENs; `character-panels.md` §9 OPEN; `item-tooltips.md` §11 OPEN (Q16) |

Rule-5 check (LEGAL.md §11.1): nothing in this revision runs in or is linked by
`tessera-server`; the overlay sends only the appearance messages M7 already has, at a bounded
rate; values stay scripted (decision 0024 §4; §15 invariant 10). The owner-policy OPEN of §16
on numeric keys is not decided here and now also covers `pid:` forms.

**Status.** Review. An independent session that wrote no part of revision 4 must check D1–D5
against the cited specs, re-derive Vector G and the changed lines of Vector F, check the 15
new hostile rows against the grammar and the position rules, and check that §12.5 agrees with
`tessera-protocol.md` v1.1 §6.2.17 and Appendix A.

**Review of revision 4** (2026-10-10, spec reviewer, Clean-Room-Role: review; a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of decision 0025, of the P1 revisions of `scenario-feed.md`, `tessera-protocol.md`, `test-server.md` and `server-content.md`, or of the code). Checked against `docs/CLEAN_ROOM.md` §5.3, decision 0025 D1–D5, decision 0024 §2 (client half only) and §4 (values scripted, never derived), LEGAL.md §11.1 rule 5, `tessera-protocol.md` v1.1 §3.1, §5.3, §6.2.17, §7.4 and Appendix A, and `test-server.md`. Result: **pass**, every item; status Review → Accepted.

- **Checklist.** Design facts only; no code or pseudo-code (the overlay rules of §12.5.4 are numbered conditions in prose); no copied prose; no game text, DIDs of an install, keys, hostnames or traffic (Vector G's property IDs lie outside both dictionary families, the labels are invented); provenance (Design, inputs `docs/` only); invariants 9–10 testable; unknowns marked OPEN; no closed gate.
- **D1–D5.** Each change of the table above is present and consistent with the rest of the text and with decision 0025, with one exception in the decision: D1 lists `look` among the overlay's declarations, while §12.5.1 excludes it (and the implementation follows §12.5.1). §12.5.1 is the consistent reading, since every place that binds a look (`player`, `character`, `accept-character`, the `create-character` input form) is outside the subset; the decision's review record notes the erratum. §12.5.4 agrees with `tessera-protocol.md` §6.2.17 (every `AppearanceSet` answered by `EntityAppearance`; no look on `AppearanceSet`; slots only at minor ≥ 1) and with Appendix A.
- **Rule 5.** Nothing of revision 4 runs in or is linked by `tessera-server` (`tools/check-server-deps.sh` passed on branch `p1`); the overlay sends the server only `AppearanceSet`s (at most one in flight, at most 4 per second, below the control budget); inventory, wallet, levels, stats and item properties stay in the client; values stay scripted (invariant 10).
- **Bounds.** Every new input has a limit and a typed outcome in §11.1: 48 values per look, 64 look bindings, 768 encoded bytes (cut with a note), strings 256 bytes and 128 in looks (U+0021–U+007E), floats finite with \|v\| ≤ 10^9, integers within their kind, property ID 0 refused, 1,024 properties per entity and per item in the client half, the overlay's send interval and time-out. The model's `Look` (1–48 entries) maps to the wire's empty look as none (§9.7).
- **Vectors.** Vector G re-derived line by line (no-install `pid:` forms taken as written; entries sorted 0x7FFF0001–0x7FFF0003; `CharacterInfo` for both declared characters; the deletion trigger's removal and its skip; the creation with no answer and an unresolved nationality label without a note; the unresolved `@Example_Stat` skip; the clear at 0.100; `skipped=2`); Vector F's changed lines carry only the new fields, in the §10.3 order. The 15 new hostile rows agree with the grammar and the position rules (for example the 49th look token at column 9 + 48 × 21 = 1,017). §14.9 and §14.10 agree with §9.6, §12.5 and `tessera-protocol.md`. The implementation's vector, hostile, overlay and plain-sandbox tests on branch `p1` pass (supporting evidence only).

Non-blocking, for the spec owner at the next revision:

- (a) §14.9 O7: the file `at 0s say "x"` does not parse (`say` needs a sender, §6.4), so it gives `MissingArgument` before the overlay check; the implementer's OPEN is right. Write `at 0s say player "x"` with `NotAllowedInOverlay` 2:1.
- (b) §14.9 O3: by §5.5 ("every change of that list emits") tick 0 gives two `AppearanceChanged` events, the sword alone and then both items; say so instead of "with both items". The implementer OPEN stands inside the O-table and splits it (rows O8 and O9 render outside the table); move it below the table.
- (c) §12.5 implementer OPEN (a)–(d): all four readings agree with §12.5.4 and rule 5 (the feed's list carries no dyes or stowing; a send the backend cannot queue is treated like a refusal; the session with the server outlives the feed; the backend's encoder, not the overlay, applies the minor). Reviewer's recommendation: adopt them as written.
- (d) §12.5.4 item 4 clears the in-flight mark on any server `AppearanceChanged` for the player, not only the answer to the overlay's own `AppearanceSet`; that is safe (the server answers in order) but could be stated.
- (e) §12.5.4 says nothing about re-sending after the 10 s time-out; the implementation sends again only when *E* changes. State it.
- (f) §9.5's client-side bound (1,024 properties per entity and per item) bounds each object; the total is bounded only through the run's event limits (§11.1). Acceptable for a scripted local run; note it if a later backend can report properties.

<a id="s18-p3"></a>
### 18.P3 P3 revision (revision 5): NPC windows

**2026-10-10** (spec writer, for decision 0024 P3; inputs: `npc-windows.md` §5.3, §6.3, §7.8,
§10, `quests.md` §9, this spec). Status of these additions: **Review (awaiting independent
review)**; the revision-4 text stays Accepted. P2 revises this spec in parallel; the P3 parts
are additive (new rows, subsections and paragraphs, vectors named N1, N2 and hostile rows
N-H*n*) so the two revisions merge without renumbering.

| # | Change | Sections | Why (`npc-windows.md`) |
|---|---|---|---|
| N-D1 | `action(@Label)` / `action(N)` and `Text::Action(u32)`, resolved with `enum_value` context `NpcAction`; unresolved → literal label | §4.2, §5.2, §8.6, §9.1, §10.3 | choice rows with the install's action texts (§5.3) |
| N-D2 | `rewards` step and `QuestRewards` event (`RewardKind`) | §6.3, §9.1, §9.2, §9.5, §10.3, §11.1 | reward amounts the client must not derive (§6.3) |
| N-D3 | `buyback` step, `BuybackShown` event, `BuyBack` command, pattern and input form | §6.3, §7.5, §7.7, §9.2, §9.4, §9.5, §10.3, §11.1 | the vendor's buy-back page (§7.8) |
| N-D4 | Vectors N1, N2; hostile rows N-H1 … N-H11 | §14.5, §14.N1, §14.N2 | conformance |

Not added, because existing forms suffice (`npc-windows.md` §10.2): install-text dialogue
pages (`Text::QuestRole`, `Comment`, `Name`, `Objective`), sell lists (the client builds them
from the inventory model), wallet entries with names (a `Currency::Item` ref names its
record), loot coin (`LootShown.coin`).

Rule-5 check (LEGAL.md §11.1): nothing here runs in or is linked by `tessera-server`; the
test-server backend refuses `BuyBack` (§9.5); every value stays scripted (§15 invariants 2
and 10). The `net_scenario_run` fuzz target's synthetic resolver also answers `enum_value`
for the context `NpcAction`. ARCHITECTURE §19 gains the three variants.

**Status.** Accepted (2026-10-11, independent review below). Before: Review; an independent
session that wrote no part of the P3 revision had to check N-D1 … N-D4 against
`npc-windows.md`, re-derive Vectors N1 and N2 line by line, and check the hostile rows against
the grammar and the position rules.

**Review record (2026-10-11).** Reviewer: a Claude session in the spec-reviewer role
(Clean-Room-Role: review) that wrote no part of the P3 revision, of `npc-windows.md` or of the
P3 code. Result: **passed**, every item of CLEAN_ROOM.md §5.3.

- **Scope.** The revision-5 text is additive: against the revision-4 text (4ec1f4a) commit
  b62d0ae only inserts lines (246 insertions, no deletion), and the text on branch `p3` is
  unchanged since b62d0ae, so the Accepted revision-4 text stands as reviewed in §18.3.
- **Checklist.** Every new statement is a design fact (token forms, ranges, error kinds and
  positions, event and command fields, log forms, limits). No code or pseudo-code: the two
  vectors are files and logs in the project's own formats. No copied prose. No game text: the
  vector strings are invented, and `Vendor` is an enum label of the install, quoted as an
  interoperability identifier. Provenance: one Design row (§1). No byte layout. Vectors and
  hostile rows make every rule testable. Unknowns are OPEN callouts (§6.3, §9.5, §13).
- **N-D1 … N-D4 against `npc-windows.md`.** N-D1 serves §5.3 Tessera rule 3 (an option's text
  `Text::Action(a)` picks the action entry whose `ActionPickerUI_Action` is *a*); N-D2 serves
  §6.3 (amount rows only from feed amounts) and §10.3 (one row kind per `RewardKind`); N-D3
  serves §7.8 (buy-back list, `BuyBack { window, entry }`) and §10.1. The kinds of §10.2 that
  the revision does not add are covered by existing forms, as §18.P3 says.
- **Install spot check (M1, read-only, 2026-10-11).** A throwaway probe over the repository's
  property decoders: the property dictionary has `ActionPickerUI_Action` and names one enum
  mapper for it, the mapper whose name is `NPCActionPickerType`; it has no base mapper, 34
  names, no display texts, no repeated name, keys 0 … 42 (within the 0 … 2^32 − 1 of
  `action(N)`), and `Vendor` has key 2, as Vector N1's install case assumes. This agrees with
  `npc-windows.md` NW3. No other new fact depends on the install, and none rests only on a BIN
  row (the reward-row order of NW17 is not used by this spec), so no disassembly was needed.
- **Vectors.** N1 and N2 re-derived line by line from §5.2, §5.6, §6.1, §6.3, §7.4–§7.7 and
  §10.3: times, window ids (the dialogue window 1 stays open, so the offer gets 2), the missing
  generic answer to an option without `goto`, the order `QuestOffered` then `QuestRewards` in
  one tick, field order and formats (`what=none`, `price=coin:N`, `stock=5`), the generic
  answers to `AnswerQuest` and `CloseWindow`, the absence of generic answers to `Buy`, `Sell`
  and `BuyBack`, and the end times. They agree. The tests `scenario_p3_vectors` and
  `scenario_p3_hostile` of `tessera-net` pass on branch `p3` (supporting evidence only).
- **Hostile rows.** N-H1 … N-H11 checked against §4.2, §4.5, §6.3, §6.4, §7.7 and §11.2:
  every error kind and every line:column agrees (N-H6: the 17th token starts at column
  15 + 16 × 10 = 175; N-H10: the entry number starts at column 16; N-H11: `say` takes a string,
  not a text, at column 18).

Non-blocking notes, for the spec owner at the next revision (review notes, not facts):

- (a) §6.3 OPEN (a): keeping the buy-back page across a later `vendor` step on the same
  window agrees with §5.6 (a step on an open kind replaces that kind's content only); state
  it. Dropping the page instead would change implemented behaviour.
- (b) §6.3 OPEN (b): `BadNumber` for a malformed amount follows §4.2 (a numeric position given
  a token that is not a number) and should be stated, since the paragraph's "a token of
  another form … is `UnexpectedToken`" can be read otherwise. The stated precedence matches
  the implementation; writing a different one would change implemented behaviour.
- (c) §6.3 OPEN (c): agreed, as for `buy`.
- (d) §6.3 OPEN (d) is a defect of `npc-windows.md` §7.8 (its "§10.4"; also its §5.3 rule 3
  "§10.2", which reads as this spec's log section): for that spec's reviewer.
- (e) §9.5 OPEN (repeated single kind from another backend): making it invalid would match
  the parser and `npc-windows.md` §10.3 but changes implemented behaviour (the
  implementation surfaces it); either way, state it.
- (f) §13 OPEN: name `action(N)` in rule 2's list of enum positions; the implementation
  already reports it there, so this changes no behaviour.
- (g) The provenance row marks the revision Design; the one install fact it relies on (the
  dictionary names a mapper for `ActionPickerUI_Action`) could cite `npc-windows.md` NW3
  (Confirmed) and this review's check.
- (h) The lexical-class table of §4.2 and the `Text` row of §10.3 do not list `action(…)`;
  the P3 paragraphs below them do. Fold them in when the text is next renumbered.

<a id="s18-4"></a>
### 18.4 Revision 6: decision 0032

**2026-10-10** (spec writer, design role). Tessera-own design for P2 exit criterion 4 ("M7's
test actors can be selected and show a target frame"), recorded in
[decision 0032](../decisions/0032-p2-targeting-on-test-server.md). Inputs: `docs/` only
(decisions 0024 and 0025, LEGAL.md §11.1, this spec, `tessera-protocol.md`, `test-server.md`,
`server-content.md`, and the event table of the Draft `combat-ui.md` §11 for what the client
half does with the events named here); no research note, clone, client or decompiler output.

| # | Change | Sections | Answers |
|---|---|---|---|
| D1 | The test-server backend answers `SelectTarget` locally with `TargetChanged` for the player: known targets only, at most one pending answer, delivered first in the next poll, no wire message, no own `TargetChanged` | §9.5, §9.7, §12.5.2, §14.9 O8, §14.11, §15 invariant 11 | P2 exit criterion 4 (selection) |
| D2 | Target frames of test actors and clearing on despawn: client-half rules, no feed change; §9.7 records that the backend emits nothing on a despawn | §9.7 | P2 exit criterion 4 (target frame) |
| D3 | The overlay subset holds no Combat-group step, trigger or input form (stated; unchanged in effect) | §12.5.1, §14.9 O10–O12 | Whether overlay runs script combat locally |
| D4 | No protocol minor | — (§0 out of scope unchanged) | — |

Editorial: the implementer OPEN inside the §14.9 table now stands below it (§18.3 note (b)),
so rows O8–O12 render in the table; its text is unchanged.

Rule-5 check (LEGAL.md §11.1): the server receives nothing new (a selection never leaves the
client), keeps nothing new and runs no rule; `tessera-server` and the server dependency
check are unchanged.

**Status.** Review. An independent session that wrote no part of revision 6 or of decision
0032 must check D1–D4 against the cited specs and rule 5, re-derive §14.9 O8, O10–O12 (the
positions against §12.5.1's first-token rule) and §14.11 R1–R10, and check invariant 11.

<a id="s18-4-1"></a>
#### 18.4.1 Review record (revision 6)

**2026-10-11**, spec reviewer (Clean-Room-Role: review), a Claude session that wrote no part
of revision 6, of decision 0032, of the other P2 specs or of the P2 code. Text reviewed: this
file and decision 0032 at commit 7b14f27 (`git diff origin/main...7b14f27`).

Checked against CLEAN_ROOM.md §5.3, every item:

- Functional facts only: the additions are Tessera's own design (a local answer, an ordering
  rule, a bound, refusals, conformance cases); no client fact was added, so no provenance row
  is due beyond the header statement that this spec holds design facts only. No code or
  pseudo-code: §9.7 items 1–6 are plain rules. No copied prose, no game text, keys, hostnames
  or traffic; "Wanderer 1" is a project-made name of `test-server.md` §5.4. No byte layout.
- No install probe and no disassembly were needed: revision 6 states no fact about the
  client or its data and has no `BIN` row (decision 0011 not engaged).
- Gate and rule 5: D1 sends nothing new to the server and the server keeps nothing new;
  `apps/server`, `apps/loadtest`, `crates/protocol` and `tools/check-server-deps.sh` are
  unchanged in the P2 diff. D2 derives nothing (decision 0024 §2.4), D3 narrows overlay runs,
  D4 adds no protocol minor (decision 0024 §2.6). §0 item 3 holds: the client half selects
  through `SelectTarget` and `TargetChanged` on every backend.
- Cases re-derived: O8 against §12.5.2 and §9.7; O10–O12 against the step, trigger and input
  grammars (§6.2, §7.5, §7.7), the first-token rule of §12.5.1 and the existing hostile rows
  (an empty `on` body parses, as in the `on chat` row), giving 2:1, 2:1 and 3:1; R1–R10
  against §9.7 items 1–6 and the §9.5 states (R5: answer before the despawn mapped in the same
  poll; R6: removed before the command; R7: the unknown 99 leaves 44 pending; R8: statics
  known until `StaticsCleared`; R10: `InvalidState` outside `InWorld`, `InvalidCommand` for
  the other Combat commands). Invariant 11 is testable by counting the double's messages and
  the `TargetChanged` events. The implementation's tests for these cases agree (supporting
  evidence only).

Result: **pass**. Non-blocking notes for the spec owner (review notes, not facts; none
changes implemented behaviour):

- (a) The §14.9 preamble describes a double *of the test-server backend*; O8 (as O9) needs the
  real test-server backend over a double of the server, since the backend's own answer is
  under test. The conformance test is built that way. Reword at the next revision.
- (b) Ruling on the §9.7 implementer OPEN: read item 1 and invariant 11 on the stream the
  backend hands to `Session`; the backend need not repeat `Session`'s event checks, because it
  builds those events from validated messages and checked statics. The implementation as
  written conforms. "Static 900" of §14.11 is a label for the ID its `StaticSpawned` carried,
  and R8's region change despawning 43 and 44 through `InterestReset` agrees with item 1.
- (c) §9.7 does not say what happens to a pending answer when the session ends before the next
  poll. The implementation drops it; entity events in `Closing` are stale anyway (§9.5), so
  either reading is conformant. State it at the next revision.
- (d) Decision 0032's "Number" paragraph says ROADMAP calls it 0031; ROADMAP now names 0032.
  `docs/decisions/README.md` and ROADMAP P2 still list 0032 as Proposed; their owners update
  them (outside this review's edit scope).
- (e) D2 and the consequence for `combat-ui.md` (selection eligibility of test actors with a
  `Character` body) bind that spec's own independent review; `combat-ui.md` is still Draft.

<a id="s18-5"></a>
### 18.5 "P4 social" revision: decision 0035

**2026-10-11** (spec writer, design role). Tessera-own design for decision 0024 P4, recorded
in [decision 0035](../decisions/0035-p4-social-on-test-server.md). Inputs: `docs/` only
(decisions 0024, 0025, LEGAL.md §11.1, `tessera-protocol.md` v1.2, `test-server.md`,
`instruments.md` §8.8–§8.9 and §12, `social-panels.md` §4, §8, §9); no research note, clone,
client or decompiler output was used for this revision.

| # | Change | Sections | Answers |
|---|---|---|---|
| H1 | Waiting parts, one synchronised start with `at_ms` and `lead_ms`, withdrawal | §2, §5.7, §6.7, §9.2, §11.1, §15 | §16 OPEN (P4); P4 exit criterion 3 |
| H2 | The player's `/play … sync` and `/playstart` as `ReadyPerformance` and `StartPerformance`; generic answer; one start per `/playstart` | §6.7, §7.5–§7.7, §9.4 | `instruments.md` §12 P4 Decision (start path) |
| H3 | `FellowVitalChanged` for members without an entity | §6.7, §9.2 | `social-panels.md` §4.4 vs TR3 |
| H4 | `DisbandFellowship` | §7.5, §7.7, §9.4 | `social-panels.md` Q12 |
| H5 | Test-server backend at minor 2 (channels, `Named` chat); other Social commands refused; plain-sandbox start answer; overlay unchanged | §9.5, §9.8, §12.5.2 | decision 0035 D1–D3; P4 exit criterion 2 |
| H6 | Vector H, 8 hostile rows, backend cases S1–S9 | §14.12, §14.13 | — |

Rule-5 check (LEGAL.md §11.1): fellowship, trade, lists, vitals and performances stay in the
client's feed; the server gains only the chat channels of `tessera-protocol.md` v1.2; an
overlay run sends nothing new (§15 invariant 13).

**Status.** Review. An independent session that wrote no part of this revision or of decision
0035 must check H1–H6 against decision 0035 and the cited specs, re-derive Vector H line by
line, check the 8 hostile rows against the grammar and the position rules of §11.2, and
check §9.8 against `tessera-protocol.md` v1.2 §10.6 and Appendix A.

**Review of the "P4 social" revision** (2026-10-11, spec reviewer, Clean-Room-Role: review; a
Claude review session in the spec-reviewer role that wrote no part of decision 0035, of the
P4 revisions of `tessera-protocol.md`, `test-server.md` or `scenario-feed.md`, or of the
code). Checked against `docs/CLEAN_ROOM.md` §5.3, decision 0035 D1–D4, decision 0024 §2 and
LEGAL.md §11.1 rule 5 (`git diff bb43e74..45b7fb9` of this file): **passed**, every item.

- **Checklist.** Functional design facts only (steps, events, commands, limits, log forms,
  backend rules); no code or pseudo-code; no copied prose; no game text (the tune and member
  names of Vector H are made up); the new provenance row is Design and names its inputs
  (`docs/` only); no Confirmed or BIN fact was added, so no install probe or decompiler
  session was needed; invariants 12–13 testable; unknowns marked OPEN; nothing under a closed
  gate.
- **H1–H6.** Each change of the table above is present in the sections it names and agrees
  with decision 0035 D2–D4 (waiting parts, one synchronised start with `at_ms` and `lead_ms`,
  the player's two commands, fellow vitals, the disband command, the 64-member and 0–10 s
  limits of the decision's table).
- **Vector H**, re-derived line by line from §6.7, §7.2–§7.7 and §10.3: the step times (0,
  0.2, 1.0, 1.5), the input ticks (5 and 20 at 100 ms), the trigger order of §7.4 step 1
  (generic answer before the trigger body: the player's `PerformanceSyncStart` at 2.000
  before the `FellowVitalChanged`), *S* = 2,500 and 2,700, and the end at *L* = 2.000 with an
  empty agenda all match the expected log; the vector in `crates/net/tests/scenario_p4.rs`
  is the same text and passes.
- **Hostile rows.** All eight positions checked against §4.5 and §11.2 (columns 21, 21, 15,
  33, 33, 20; the two overlay rows at 2:1 as §12.5.1 and the rows of §14.9).
- **§9.8 against `tessera-protocol.md` v1.2.** The three channel commands only at minor ≥ 2
  with `CHAT_CHANNELS`, `Fellowship` chat and the other Social commands refused at every
  minor, `ChannelRefused` as a System line in Tessera's words, the server's spelling in
  `ChannelJoined`: all agree with §10.6 and Appendix A. Rule 5: the server receives nothing
  new beyond the chat of the protocol spec; an overlay run sends nothing new.
- **Code on branch `p4`**, read to compare only: the `tessera-net` tests `scenario_p4`
  (Vector H, the hostile rows, S7–S8) and `remote_channels` (S1–S6) and the
  `tessera-loadtest` test `v12_channels` (S9 against an in-process `tessera-server`) passed,
  with the protocol and server tests (291 passed, 0 failed).

Non-blocking, for the next revision (none changes implemented behaviour unless said):

- (a) §15 invariant 13 says the test-server backend "sends nothing for a Social command
  except `ChannelJoin`, `ChannelLeave` and `ChatSend` channel 6", but `Chat` is not a
  Social-group command (§9.4: the existing `Chat` carries the channels). Word it as: for
  Social-group commands only `ChannelJoin` and `ChannelLeave`, and for `Chat` on `Named` only
  `ChatSend` channel 6, all only at minor ≥ 2 with `CHAT_CHANNELS`.
- (b) Rulings on the "P4 social" implementer note of §16: (1) confirmed: recording the
  player's part on `ReadyPerformance` is a generic answer (§7.6) and does not run under
  `defaults off`; reword §6.7 "The player's commands" so that "under `defaults on`" covers
  both. (2) Confirmed: `by=` and `perform-withdraw` take names and never skip for an actor
  that is not spawned. (3) Confirmed, provided one `/playstart` still gets exactly one
  answer: the feed's while it answers commands (§7.3), the plain sandbox's after the feed's
  end; §9.8 may say "no scenario, or a scenario whose feed no longer answers commands".
  (4) Agrees with `tessera-protocol.md` Appendix A ("naming the channel"). (5) Stating the
  256-byte cut for `PerformanceStarted.tune` in §9.5 would change implemented behaviour (today
  1,024 bytes); the spec owner decides.
- (c) The TAB OPEN of §11.1 stays with the spec owner; `OutOfRange` at the tune's column is a
  consistent choice (the tune's value range excludes control characters, §9.4).
<a id="s18-p5"></a>
### 18.P5 P5 travel revision (revision 8): map, travel, mounts and instances

**2026-10-11** (spec writer, for decision 0024 P5; inputs: `travel-windows.md` §4.10, §4.11,
§5.2, §6.5, §6.8, §9; `map-windows.md` §5, §6.7, §7.2; `mounts.md` §2.3, §2.8, §4.1;
`walker.md` §16.1, §16.5; `lua-plugin-api.md` §15.5; this spec). Status of these additions:
**Accepted** (independent review 2026-10-11, CLEAN_ROOM.md §5.3 passed, every item; review
record below and in the header); every other part keeps its status. P2 and P4 revise
this spec on their branches; the P5 parts are additive (new rows, options, steps,
paragraphs, a rule 6 in §13, vectors named T1–T3 and hostile rows T-H*n*), so the revisions
merge without renumbering. The revision number is given at merge.

**Check of the existing forms.** The forms of §6.5 render the four presentations without
change: `marker`/`marker-remove` (`MarkerAdded`, `MarkerRemoved`) for map markers,
`travel-list`/`travel` (`TravelShown`, `TravelStarted`, `ChooseTravel`) for the travel
window, `instances`/`instance` (`InstanceListShown`, `InstanceChanged`, `JoinInstance`,
`LeaveInstance`) for the finder, `mount` (`MountChanged`, `Dismount`) for the mounted walker
(`walker.md` §16.1) and `teleport` for arrival. The additions carry values the presentation
must show but must not derive:

| # | Change | Sections | Why |
|---|---|---|---|
| T-D1 | `travel-list` options `min-level`, `undiscovered`, `unmet`, `unusable`, `swift`, `slow`, `area` and the `TravelEntry` fields | §6.5, §9.1, §9.2, §10.3, §11.1 | the travel row's level text, marks, ghosting, territory and area headers (`travel-windows.md` §4.10 rules 3–5) for literal and recorded destinations alike |
| T-D2 | `travel … route=` (`TravelStarted.route`), `travel-end` (`TravelEnded`) | §6.5, §9.2, §10.3 | the loading-screen or end-travel-box choice for literal destinations, and the end of the presentation on the end-travel button (`travel-windows.md` §5.2 rules 1–4) |
| T-D3 | `instances` options `from`, `featured`, `level`, `size`, `tier`, `locked`; `InstanceListShown.source`, `.featured`, the `InstanceEntry` fields | §6.5, §9.1, §9.2, §10.3, §11.1 | the instance-travel window's source, the featured field, the level, size and tier shown and the error look (`travel-windows.md` §4.11, §6.3 rule 5, §6.8) |
| T-D4 | `JoinInstance.size`, `.tier`, `.level`; input options | §7.7, §9.4, §9.5, §10.3 | the finder's chosen size, tier and level (`travel-windows.md` §6.8 rule 5) |
| T-D5 | `instance-queue` (`InstanceQueueChanged`, `QueueState`), `LeaveQueue` with pattern and input form | §6.5, §7.5, §7.7, §9.1, §9.2, §9.4, §9.5 | the confirm page, its timers and the queue alert (`travel-windows.md` §6.5, §6.8 rule 6, §7.2) |
| T-D6 | `note-type=` (`MarkerAdded.note_type`, context `MapNoteType`) | §6.5, §8.6, §9.2 | an icon choice for markers by map-note type, since no selector family gives image DIDs for `icon=` (`map-windows.md` §6.7 item 2) |
| T-D7 | `markers` with marker sets and run-time ids; family `map-region` | §5.3, §6.5, §8.3, §8.6 | markers on every map region in one step (`map-windows.md` §5.4), and markers on every element of a `place` or `map-note` selector |
| T-D8 | `effect` attributes `mount-type`, `war-steed` | §8.3 | choosing normal or war-steed mounts for the walker and the plugin views by what they are |
| T-D9 | `mount … kind=` (`MountChanged.combat`), `mount-vital` (`MountVitalChanged`, `MountVital`) | §6.5, §9.1, §9.2, §10.3 | the plugin mount views' class and values, which in the original are server state (`lua-plugin-api.md` §15.5) |
| T-D10 | Log rule for added fields; Vectors T1, T2, resolver case T3; hostile rows T-H1 … T-H18; rule 6; invariant 14 | §10.3, §13, §14, §15 | conformance, and committed logs that stay byte-identical |

Not added, because existing forms suffice: a list of every mount effect (`effect role=mount
all`); travel destinations and routes (`travel-destination`; a route ref is resolved to its
destination by the client half, `travel-windows.md` §4.10 rule 2); arrival (`teleport`);
leaving an instance (`leave-instance`, `instance none`); refusals and chat lines (`system`,
`say`); the instance-travel window (an `instances` step with `from=` and one entry,
`travel-windows.md` §4.11).

**Protocol.** None: the test-server backend refuses the new command and never produces the
new events (§9.5); no protocol minor is needed (LEGAL.md §11.1 rule 5).

**Rule-5 check (LEGAL.md §11.1).** Nothing here runs in or is linked by `tessera-server`. The
feed's numbers stay scripted (invariants 2, 10 and 14); the client half alone reads the install
for names, icons, mount types and route records. ARCHITECTURE §19 gains `TravelEnded`,
`MountVitalChanged`, `InstanceQueueChanged` and `LeaveQueue` (for its owner).

**What an independent review must check.** T-D1 … T-D10 against the cited sections of
`travel-windows.md`, `map-windows.md`, `mounts.md`, `walker.md` and `lua-plugin-api.md`;
Vectors T1 and T2 and case T3 line by line (times, window and marker ids, field order, the
rule for added fields); T-H1 … T-H18 against §4.2, §4.5, §6.5, §7.7, §11.1 and §11.2; that the
committed `world.log` is unchanged under §10.3.

**Review result (2026-10-11, independent spec reviewer, Clean-Room-Role: review).** **Passed,
every item of CLEAN_ROOM.md §5.3.** T-D1 … T-D10 were read against the cited sections of the
P5 specs; every addition carries a value the presentation must show and the spec restates
invariant 14 that the feed derives none of them. The §5.3 checklist: all statements are
functional facts of Tessera's own format or install-derived references with a source mark; no
code or pseudo-code; no copied prose; API/event/property/label names are interoperability
identifiers; all vector text is invented and no game strings, keys, hostnames or captures
appear; nothing is linked by `tessera-server` and no protocol minor is added; the scope rules
of §9.3 (no anti-cheat, key values, certificates, endpoints or account data) are met. The
single install-derived claim (four dictionary names and their kinds) was corroborated by
read-only spot-checks of the install through the P5 decoders and selector resolution and by
the §15.5 BIN pass reviewed in parallel; Vectors T1–T3 and the hostile-row ranges are
internally consistent with §6.5, §9, §10.3, §11.1 and §13. Non-blocking: the `map-region`
family and the new label contexts cite `map-windows.md` and `travel-windows.md`, still in
Review; acceptance follows the P3 precedent and the §8.3 "stands until Accepted" note, and
§8.3/§8.6 must be re-checked if those reviews change the cited definitions. The P5 OPENs stay
non-blocking for the spec owner. Status of the P5 parts: **Review → Accepted**.
