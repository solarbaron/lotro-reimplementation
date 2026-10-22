<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0032 — P2 on M7's server: local selection, target frames of test actors, no combat in overlay runs, no protocol minor

**Status:** Accepted (independent review 2026-10-11, see [Review](#review)). Before:
Proposed (spec writer, design, 2026-10-10). An independent review by a session that wrote no
part of this decision or of `scenario-feed.md` revision 6 must check it before it is Accepted.

**Number.** Out of sequence, like 0030: 0031 was taken on the parallel branch `p3`
(`0031-parity-panel-goldens.md`), so this record took the next free number of the
out-of-sequence range. It was planned as 0031; ROADMAP P2 and `docs/decisions/README.md`
now name it 0032 (refreshed 2026-10-14; before, this paragraph said ROADMAP called it 0031).

Tessera-own design. No client fact, research note, research clone or decompiler output was
needed or used; the inputs are `docs/` only (decision 0024, decision 0025,
`scenario-feed.md`, `tessera-protocol.md`, `test-server.md`, `server-content.md`, and the
event table of the Draft `combat-ui.md` §11, read only for what the client half does with
the events named here).

## Context

P2 exit criterion 4 (decision 0024 §5, ROADMAP P2) reads: "M7's test actors can be selected
and show a target frame." On M7's server those actors are the other players' characters
(kind `Player`) and the wanderers of `test-server.md` §5.4 (kind `NonPlayer`, a `Character`
body, a project-made name such as a numbered template, no class, no properties). The server
knows only accounts, presence, movement, chat and appearance (`server-content.md` §7); it has
no levels, vitals, effects or targets, and it must gain no combat state (ROADMAP P2 goal).

Today:

- The client half selects by sending `SelectTarget` and shows a new selection only when the
  session reports it as `TargetChanged` for the player (`scenario-feed.md` §9.2, §9.4; the
  Draft `combat-ui.md` §11 builds on that rule). The sandbox's feed answers `SelectTarget`
  generically (`scenario-feed.md` §7.6).
- The test-server (`remote`) backend refuses every Combat-group command with
  `NetError::InvalidCommand` (`scenario-feed.md` §9.5, §9.7), so on M7's server nothing can
  be selected, and overlay case O8 (§14.9) requires that refusal.
- The overlay subset (`scenario-feed.md` §12.5.1, decision 0025 D1) admits no Combat-group
  step, trigger or input form; nothing says whether that is deliberate for P2.

Binding frame: decision 0024 §2 (client half only; the server stays data-free and feed-free;
values scripted, never derived), `server-content.md` §4 and §7, the
server dependency guard (`scenario-feed.md` §3.1, `tools/check-server-deps.sh`), and
`scenario-feed.md` §0 item 3 (the client half handles events without knowing which backend
sent them).

## Decision

### D1. Selection on the test-server backend is local: the backend answers `SelectTarget` itself

The test-server (`remote`) backend accepts `SelectTarget` in `InWorld` and answers it in
`submit`, with **no message on the wire**:

1. **Known targets.** The target is acceptable when it is none, the player's own entity, or
   an entity whose creation the session has already delivered and whose removal it has not:
   a remote entity (`EntityCreated` delivered, no `EntityDespawned` since, `InterestReset`
   counting as a despawn of each) or a static the backend streamed from the client's own
   world source (`StaticSpawned` delivered, no `EntityDespawned` or `StaticsCleared` since).
   The check uses the backend's view as of the events it has handed to the session, not
   messages it has received and not yet mapped.
2. **Answer.** For an acceptable target the backend queues
   `TargetChanged { entity: the player, target }`. Queued answers are delivered **first** in
   the next poll, before any event the backend maps in that poll, so in the event stream a
   `TargetChanged` that names an entity always lies after that entity's creation and before
   its removal.
3. **Bound.** At most one answer is pending: a later acceptable `SelectTarget` before the
   next poll replaces the pending answer (selection is a state; only the latest counts).
4. **Unknown targets.** Any other target (never created, already removed, or an ID of
   another session) gives `Ok`, no event and a rate-limited debug log line; a pending answer
   stays. This is a race with a removal, not a malformed command, so it is not
   `InvalidCommand`.
5. **No state, no own events.** The backend keeps no selection. It never emits
   `TargetChanged` on its own: not on a despawn, `InterestReset`, teleport, appearance change
   or world entry, and never for an entity other than the player (the server has no targets
   to report).
6. **Everything else unchanged.** `SelectTarget` outside `InWorld` follows the session rule
   of `scenario-feed.md` §9.5 (`NetError::InvalidState`). `UseSkill`, `CancelCast`,
   `SetQuickslot` and every other new command stay refused with `NetError::InvalidCommand`.
7. **Overlay runs.** `SelectTarget` is not an Inventory-group command, so the overlay routes
   it to the test-server backend (`scenario-feed.md` §12.5.2), which answers it under 1–6;
   the feed never sees it.

**Alternatives rejected.** (a) *The client never sends `SelectTarget` on this backend and
sets the selection itself:* the client half would have to know its backend (against
`scenario-feed.md` §0 item 3), selection would have two paths (reported on the sandbox, local
on the server) and the rule "shown only when reported" would break; an official-server
backend would need the command anyway. (b) *A wire message that tells the
server the selection (and relays it to others for assist):* the server would hold who
targets whom, which is combat state, needs a protocol minor, and is not on `server-content.md`
§7's list (accounts, presence, movement, chat, appearance as opaque references, simple test
actors).
(c) *The overlay feed answers `SelectTarget` with its generic answer:* selection would
then need a scenario and `defaults on`, while a plain `--server` run without a scenario must
select too; and test actors are not feed entities.

**Server content scope.** The server receives nothing new and keeps nothing new: the selection
never leaves the client's process. `tessera-server` is unchanged, links no data crate, no
`tessera-net` and no feed, and `tools/check-server-deps.sh` is unchanged.

### D2. What a target frame shows for a test actor, and what happens on despawn

The client half needs no backend-specific rule: it shows what the session reports, and on
M7's server the session reports only these things about a test actor.

| Part of the target frame | Source on M7's server | Shown |
|---|---|---|
| Name | `EntityCreated.name` (the protocol's `EntityCreate.name`, cleaned as every inbound name) | As delivered; an empty name gives an empty name line, never a substitute text |
| Name colour | `EntityCreated.kind` | By kind, the "no levels known" case of the client's name-colour rule (`combat-ui.md` §5.4, Draft) |
| Figure (portrait) and decoration | `EntityCreated.appearance`, then each `AppearanceChanged` of that entity | The entity's body, worn items and look (decision 0025 D2 receiver rule), drawn through the shared figure path; the body also picks the frame decoration where the client's rule reads it |
| Level | No `LevelChanged` ever arrives | Nothing (no placeholder number) |
| Vitals | No `VitalChanged` ever arrives | The frame's no-vitals form; no meter filled with an assumed value |
| Effects, cast bar, selection's own target, combat state | No `EffectApplied`, `CastStarted`, `TargetChanged` for the actor or `CombatState` ever arrives | Empty or hidden |

Nothing is derived or invented: no level, health, reaction, consideration or "hostile"
state is computed for a test actor (decision 0024 §2.4).

**Despawn.** When the selected entity is removed (`EntityDespawned`, or the despawns of an
`InterestReset` after a region change or far teleport), the client half clears the selection
**locally**: the target frame and the world selection indicator hide, kept values of that
entity are dropped with it, and **no command is sent** (no `SelectTarget { target: None }`).
The backend emits no `TargetChanged` for it (D1 item 5). If the same entity is created again
(the protocol keeps an entity's ID when it re-enters interest), it is not selected again by
itself. When the session ends, the selection ends with it. This rule is the same on every
backend: the sandbox's feed also emits no `TargetChanged` on a `despawn` unless the scenario
scripts one.

### D3. Overlay runs do not script Combat-group events (P2)

The overlay subset of `scenario-feed.md` §12.5.1 stays without the Combat group, now stated
explicitly: no Combat-group step (`target`, `vital`, `combat`, `quickslot`, `cast`, `delay`,
`interrupt`, `cooldown`, `effect`, `effect-end`, `damage`, `heal`, `defeat`, `revive`), no
trigger on a Combat-group command (`target`, `use-skill`, `cancel-cast`), and no
Combat-group input form (`target`, `use-skill`, `cancel-cast`, `quickslot`). A file with one
is `NotAllowedInOverlay` (exit 3) before anything connects, as today.

Reasons:

1. **Test actors cannot be named.** Their IDs and names are the server's, known only at run
   time; a scenario would need a new entity reference bound to one server's content.
2. **Local combat on shared entities diverges from every observer.** A defeat, a cast or an
   effect on the player or on a test actor would exist on this screen only, while the server
   keeps moving and relaying them; the walker acts on some of these states (input, animation),
   which would contradict the server's movement authority.
3. **Not needed.** Exit criteria 1–3 run on the plain sandbox, where the feed owns every
   entity; criterion 4 needs only D1 and D2.
4. **Simple routing.** Combat-group commands keep one owner in an overlay run, the
   test-server backend.

This is a P2 choice, not a server-scope limit: scripting such events locally would send the
server nothing. A later milestone may open part of the Combat group to overlay runs (for
example values on the player only, or a reference to the current selection) by a decision of
its own.

### D4. No protocol minor

`tessera-protocol.md` stays at 1.1: D1 sends nothing, D2 uses only `EntityCreate`,
`EntityAppearance`, `EntityDestroy` and `InterestReset`, which M7 has, and D3 adds no
traffic. The cost is accepted: on M7's server no client learns another client's selection,
so the "assist" targeting action and the selection's-own-target display have nothing to show
there. Relaying selections would be a separate decision and an owner question first, because
`server-content.md` §7 does not list it.

### Hostile-input limits (summary; the specs are normative)

| Item | Limit | Exceeded |
|---|---|---|
| Local selection answers | At most one pending; delivered first in the next poll | A later accepted `SelectTarget` replaces the pending answer |
| Unknown selection target | — | `Ok`, no event, rate-limited debug log |
| Wire traffic from selection | None | — (a `SelectTarget` that reaches the wire is a defect; the conformance cases check it) |
| Overlay Combat-group statements | None allowed | `NotAllowedInOverlay`, exit 3, before connecting |

### Conformance vectors

`scenario-feed.md` §14.9 (O8 changed; O10–O12, overlay files with Combat-group statements)
and §14.11 (test-server selection cases R1–R10).

## Consequences

- **Spec revised (status Review; independent review required):** `scenario-feed.md`
  revision 6 (§9.5 other backends, §9.7, §12.5.1, §12.5.2, §14.9, §14.11, §15 invariant 11,
  §17, §18.4). `tessera-protocol.md`, `test-server.md` and `server-content.md` are
  unchanged.
- **For the Draft `combat-ui.md`** (its writer and reviewer): its selection-eligibility rule
  should cover test actors as they are, kind `Player` and `NonPlayer` with a `Character`
  body (`test-server.md` §5.4), not only objects without a body record; its despawn row of
  §11 agrees with D2.
- **Implementers:** `crates/net` (remote backend: the known-target check, the one pending
  local answer delivered first in the next poll, no `TargetChanged` of its own; overlay
  routing unchanged; the overlay subset check names the Combat group; conformance cases
  §14.9 and §14.11), `apps/client` (selection on any backend through `SelectTarget` and
  `TargetChanged` only; target frame from reported values only; local clearing on despawn).
  `apps/server`, `apps/loadtest` and `crates/protocol` are unchanged.
- **P2 exit criterion 4 test** (real data for the figure, in process for the server): the
  test server in process with one wanderer group and two clients at minor 1; client A selects
  a wanderer and the other client's character by `SelectTarget`, receives `TargetChanged`,
  and its target frame shows the name, the figure, no level and the no-vitals form; when
  client B disconnects, A's selection of B clears with no command sent; the server's
  received-message counters show no message caused by the selections.
- The server's dependency graph does not change; `check-server-deps` stays as it is.

## Open for the owner

1. **Quickslots on M7's server.** `SetQuickslot`, `UseSkill` and `CancelCast` stay refused
   there, so the quickslot bar cannot be edited or used in a `--server` run. A local answer
   like D1's would be possible without server-scope cost; it is not part of P2's criteria and
   is left for a later decision.
2. **Relaying selections** (D4) needs the owner before any design.

## Review

**2026-10-11, passed** (spec reviewer, Clean-Room-Role: review; a Claude session that wrote no
part of this decision, of `scenario-feed.md` revision 6, of the other P2 specs or of the P2
code; text at commit 7b14f27).

- **D1** agrees with `server-content.md` §7 (the server receives, keeps and runs nothing new;
  a selection never leaves the client) and with decision 0024 §2.1–§2.3 (client half only;
  M7's server drives only what it carries; the server's dependency graph is unchanged: no
  change under `apps/server`, `apps/loadtest`, `crates/protocol` or
  `tools/check-server-deps.sh` in the P2 diff). It keeps `scenario-feed.md` §0 item 3: one
  selection path through `SelectTarget` and `TargetChanged` on every backend.
- **D2** derives nothing for a test actor (decision 0024 §2.4): the frame shows only reported
  values; the despawn rule is client-local and sends no command.
- **D3** only states the existing overlay subset; it is a P2 choice, correctly not presented
  as a server-scope limit.
- **D4** adds no protocol minor, as decision 0024 §2.6 requires for anything `server-content.md`
  §7 does not allow; relaying selections stays an owner question.
- `scenario-feed.md` revision 6 carries D1–D4 faithfully; §14.9 O8, O10–O12 and §14.11 R1–R10
  were re-derived and invariant 11 checked (record: `scenario-feed.md` §18.4.1). No client
  fact, install probe or disassembly is involved.

Non-blocking: the "Number" paragraph's remark that ROADMAP calls this decision 0031 is stale
(ROADMAP now names 0032); `docs/decisions/README.md` and ROADMAP P2 still say Proposed and are
updated by their owners. The two owner questions above stay open. (Done 2026-10-14 by the
spec writer: the "Number" paragraph is refreshed; `docs/decisions/README.md` lists 0032 as
Accepted. Editorial only; D1–D4 are unchanged.)

**2026-10-14, passed** (spec reviewer, Clean-Room-Role: review; a Claude session that wrote no
part of the refresh in commit bf41c0b or of the code). The refreshed "Number" paragraph is
true: `docs/decisions/README.md` lists 0032 as Accepted and ROADMAP P2 names decision 0032
(planned as 0031, which branch `p3` took). Only the "Number" paragraph and the note above
changed; D1–D4, their consequences and the two owner questions are unchanged, so the status
stays Accepted.

Before this review: pending. The reviewer checks D1–D4 against `server-content.md` §7, decision 0024 §2,
`scenario-feed.md` §0 item 3 and revision 6 (§18.4), and re-derives the cases of
`scenario-feed.md` §14.9 (O8, O10–O12) and §14.11.
