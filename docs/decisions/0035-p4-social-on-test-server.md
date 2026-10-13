<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0035 — P4 on M7's server: named chat channels only; social panels and synchronised starts on the scenario feed

**Status:** Accepted (independent review 2026-10-11 by a Claude review session in the
spec-reviewer role, Clean-Room-Role: review, that wrote no part of this decision, of the P4
revisions of `tessera-protocol.md`, `test-server.md` or `scenario-feed.md`, or of the code;
see [Review](#review)). Before: Proposed (spec writer, design, 2026-10-11). The item under
[Open for the owner](#open-for-the-owner) stays open: this decision does not decide it.

Tessera-own design. No client fact, research note, research clone or decompiler output was
needed or used for the decision itself; its inputs are `docs/` only (decision 0024, decision
0025, LEGAL.md §11.1, the specs named below).

## Context

Decision 0024 P4 ("Social panels") has three exit criteria:

1. "Fellowship, trade and friends scenarios render; leader change, removal and disbanding
   are shown."
2. "Tells and channel join and leave work on M7's server (chat, rule 5)."
3. "A synchronised `/play` from a scenario starts within the tolerance of `instruments.md`."

What exists:

- `tessera-protocol.md` v1.1 carries Say, Yell, Emote, Tell and System chat (§10). Tells
  already work on M7's server. There is no channel to join or leave, and no way to send or
  receive a line on a named channel.
- `scenario-feed.md` revision 4 already has the Social group (§6.4, §9.2, §9.4): the steps
  `say … channel=named:…`, `channel join|leave`, `fellowship`, `invite`, `trade`,
  `trade-offer`, `trade-ready`, `friends`, `ignore`, `perform` and `perform-stop`, and the
  matching commands. The test-server backend refuses every Social command and `Chat` on the
  channels `Fellowship` and `Named` (§9.5).
- The feed's `perform` step starts one performance after a scripted delay. It has no form
  for parts that wait for a common start, and the player's own `/play … sync` and
  `/playstart` reach no backend (`scenario-feed.md` §16 OPEN, P4).
- Fellowship rows show vitals only for members that have an entity (`VitalChanged` takes an
  `EntityId`). In the original a member's row keeps its vitals while the member is out of
  view (`social-panels.md` §4.4).
- Decision 0024 open item 2 asks the owner whether relaying a synchronised `/play` start
  through M7's server counts as chat under rule 5. Until answered, P4 tests it on the
  scenario feed only.

Binding frame: decision 0024 §2 (client half only; the server stays data-free and feed-free;
values scripted, never derived), LEGAL.md §11.1 rules 2 and 5, `server-content.md` §7, the
server dependency guard (`tools/check-server-deps.sh`, `scenario-feed.md` §3.1) and decision
0025 D1 (the overlay run).

## Decision

### D1. What M7's server gains: named chat channels (exit criterion 2)

LEGAL.md §11.1 rule 5 lets Tessera's own server relay chat. P4 adds to it **only chat**:

- **Tells** stay as in v1.0 (`ChatSend` channel 4, `tessera-protocol.md` §10.3). No change.
- **Named chat channels** with join, leave and relay, as protocol **minor version 2**
  (`tessera-protocol.md` v1.2, §3.1): `ChannelJoin` and `ChannelLeave` requests, the answers
  `ChannelJoined` and `ChannelLeft`, the refusal `ChannelRefused`, the channel value
  `Named` (6) with the channel's name in `ChatSend` and `ChatMessage`, a capability bit
  `CHAT_CHANNELS`, and the chat refusal code `NOT_JOINED`. Channel names use the `NAME`
  text class; the server adds its own name rule and matching key (`test-server.md`).
- **Test server.** A channel registry: a channel is created by its first join and dropped
  when its last member leaves; per-character and server-wide limits; a line on a channel
  reaches only its joined members; membership is per session and is never written to disk;
  channel names are logged only under `log_chat` (`test-server.md` §9.1).
- **Fellowship chat stays refused** on the test-server backend (`NetError::InvalidCommand`)
  and has no wire value: a fellowship exists only in the scenario feed, so the server has no
  members to relay to.

**Rule 5.** A channel is a set of connections that asked to share chat lines, keyed by a
player-chosen name. The server learns nothing beyond who joined which name and the chat
text it already relays; it keeps no game state, judges nothing and links no data crate. The
dependency graph of `tessera-server` does not change; `check-server-deps` stays as it is.

**Alternatives rejected.** (a) Channels as a server-side configuration list only (no
player-created channels): exit criterion 2 asks for join and leave of channels the player
names, and the original's channel commands take a name. (b) Fellowship chat as a server
"party" of connections: that would be a gameplay group on the server (who leads, who may
invite), which rule 5 does not list; a channel any player can join covers chat among friends
without it. (c) Channel membership persisted per character: adds a file format and a
migration for no P4 criterion; the client can re-join from its own settings at world entry.

### D2. What stays on the scenario feed

Fellowship (members, leader, invitations), trade (window, both sides, coin, ready states),
the friend and ignore lists, member vitals, and every performance event, synchronised
starts included, exist only on the scenario feed. On the test-server backend their commands
stay refused (`scenario-feed.md` §9.5) and the panels stay empty (`social-panels.md` TR13).

**Synchronised-music relay is not decided here.** Whether a synchronised `/play` start
(the ready and start notifications of `instruments.md` §8.8) may be relayed by M7's server
as chat is decision 0024 open item 2, an owner question. Protocol v1.2 carries no message,
property or capability for it, and the test-server backend refuses the two new performance
commands (D4). If the owner answers yes, a later minor adds it; if no, it stays on the feed.

### D3. Overlay runs keep the P1 subset (no Social group)

The overlay run (`scenario-feed.md` §12.5, decision 0025 D1) keeps its subset: the feed
runs the Character and Inventory groups only. Social-group steps, declarations and input
forms stay `NotAllowedInOverlay`, and every Social command of an overlay run goes to the
test-server backend (which takes `JoinChannel`, `LeaveChannel` and `Chat` on `Named` at
minor 2 and refuses the rest).

Reasons: (1) no P4 criterion needs it: criteria 1 and 3 run on the plain feed, criterion 2
on the server without a scenario; (2) the overlay subset has no actors, so scripted
fellows, trade peers and performers could have no entity, while the walker shows the
server's real players, who would not be grouped, trading or performing on the server: the
panels would contradict the world; (3) a scripted synchronised start next to real players
would look like the relay that open item 2 has not allowed; (4) the rule-5 reading of
decision 0025 D1 stays unchanged (the server receives nothing new from an overlay run).
This can be revisited when the owner answers open item 2.

### D4. Feed additions (scenario-feed.md revision 7, "P4 social"; values scripted, never derived)

- **Synchronised performances.** A `sync` word on the `perform` step makes a member's part
  **wait** (event `PerformanceReady`); a new step `perform-start` makes **one** start that
  starts every waiting part at one common time (event `PerformanceSyncStart`, carrying the
  start time on the feed's virtual clock and the lead); `perform-withdraw` removes a waiting
  part (event `PerformanceWithdrawn`). The start tolerance is the one of `instruments.md`
  §8.9; the feed refers to it and defines none.
- **The player's own commands.** `/play … sync` (once the client has accepted the part) is
  the command `ReadyPerformance { tune }`; `/playstart` is `StartPerformance`. The feed's
  generic answer to `StartPerformance` is one `PerformanceSyncStart` attributed to the
  player; triggers may answer either command. The plain sandbox answers `StartPerformance`
  the same way. The test-server backend refuses both (D2), and the client then keeps its
  local start (`instruments.md` §12, P4 Decision 3).
- **Members without an entity** carry vitals through a new event `FellowVitalChanged`
  (member name, vital, current, maximum) and a step `fellow-vital`. A member with an entity
  keeps using `VitalChanged` of its entity.
- **Disbanding** gets a command, `DisbandFellowship` (`social-panels.md` Q12), answered
  only by triggers.
- **Remote backend at minor 2:** `JoinChannel`, `LeaveChannel` and `Chat` on `Named` are
  accepted; every other new command stays refused.

### Hostile-input limits (summary; the specs are normative)

| Item | Limit | Exceeded |
|---|---|---|
| Channel name on the wire | `NAME` (1–64 bytes, no control characters) | Above 64 bytes: protocol violation. Empty or control characters from the client: `ChannelRefused(INVALID_NAME)` |
| Channel name, server rule | NFC, 1–50 scalar values (at most 64 bytes) of letters, digits, single spaces, `'`, `-`, `_`; reserved words | `ChannelRefused(INVALID_NAME)` |
| Channels per character | `max_channels_per_character`, default 10 (1–32) | `ChannelRefused(TOO_MANY_CHANNELS)` |
| Channels on the server | `max_channels`, default 256 (1–4,096) | `ChannelRefused(SERVER_LIMIT)` |
| Members per channel | `max_channel_members`, default 256 (2–4,096) | `ChannelRefused(CHANNEL_FULL)` |
| Join and leave requests | Bucket of 5, one token per 6 s, per connection | `ChannelRefused(RATE_LIMITED)`; counts toward the `FLOODING` rule |
| Minor-2 messages and channel 6 at minor ≤ 1, `ChannelJoin` without `CHAT_CHANNELS` | — | Protocol violation |
| Feed: waiting parts | One per member name; at most 64 members | A later `perform … sync` for the same member replaces its part; a 65th member is a skip (counted) |
| Feed: `perform-start` lead | 0 … 10 s, default 700 ms | `OutOfRange` |
| Feed: tune names | 256 bytes (unchanged) | `TooLong` |

### Conformance vectors

`tessera-protocol.md` §16.9 (1.2 frames and negative vectors), `test-server.md` §13.1
(hostile 1.2 cases), `scenario-feed.md` §14.12 (Vector H: fellowship with a member out of
view, synchronised start, player commands) and §14.13 (P4 hostile rows and remote cases).

## Consequences

- **Specs revised (status Review; independent review required):** `tessera-protocol.md`
  v1.2, `test-server.md` (channel registry, `[chat]` keys, limits, logs, invariants),
  `scenario-feed.md` revision 7 ("P4 social", additive: no earlier section renumbered; at the merge
  with the P2 and P3 revisions its Vector H became §14.12, its hostile rows and remote cases
  §14.13, its review record §18.5 and its invariants 12 and 13).
- **Implementers:** `crates/protocol` (minor 2: the five messages, channel value 6, bit 3,
  code 6, vectors), `apps/server` (registry, limits, relay, logs), `apps/loadtest` (1.2
  cases), `crates/net` (remote backend at minor 2; the new events, commands and steps; the
  plain-sandbox answer to `StartPerformance`), `apps/client` (chat window on the decision
  0020 dispatcher, channel commands, `/play … sync` and `/playstart` as commands, the
  fellowship panel's vitals for members without an entity), `fuzz/` (protocol and scenario
  targets cover the new forms).
- **Specs that must take the feed's new forms into account** (owners of those specs):
  `instruments.md` §12 P4 Decision 3 (with a backend that accepts `StartPerformance`, the
  start is the backend's `PerformanceSyncStart`), `social-panels.md` TR3 (members without an
  entity take `FellowVitalChanged`) and Q12 (`DisbandFellowship`), `chat-window.md`
  (channel join and leave on the server).
- The server's dependency graph does not change; `check-server-deps` stays as it is.
- If another branch takes protocol minor 2 first, the number is reconciled at merge
  (`tessera-protocol.md` §18).

## Open for the owner

1. **Decision 0024 open item 2** (unchanged): does relaying a synchronised `/play` start
   through M7's server count as chat under rule 5? Until answered, synchronised starts with
   other players exist only on the scenario feed (D2, D3). Owner row O137.

## Review

2026-10-11, spec reviewer (Clean-Room-Role: review), against LEGAL.md §11.1 rule 5,
decision 0024 §2 and CLEAN_ROOM.md §5.3: **passed**.

- **Rule 5.** D1 adds only chat to M7's server: tells unchanged, and named channels that are
  sets of connections keyed by a player-chosen name. The server keeps no game state, judges
  nothing, links no data crate (`tools/check-server-deps.sh` passes on branch `p4`) and
  persists no channel. Fellowship chat, which would need a server-side group, stays off the
  wire. Rules 1 and 2 are untouched: no game data and no new opaque reference travel.
- **Decision 0024 §2.** Principle 2 (M7's server drives only other players, appearance, chat
  and test actors), principle 3 (the server stays data-free and feed-free), principle 4 (the
  feed only plays back scripted values: `at_ms` is tick time plus a written lead) and
  principle 6 (a protocol minor only for something rule 5 allows) all hold. Open item 2
  (synchronised `/play` relay) is left to the owner: v1.2 has no message, property or
  capability for it, and the test-server backend refuses `ReadyPerformance` and
  `StartPerformance` (D2).
- **D3.** The overlay subset is unchanged, so decision 0025 D1's rule-5 reading still
  holds.
- **Agreement with the specs.** D1–D4 and the hostile-input table agree with
  `tessera-protocol.md` v1.2 §3.1, §10.6, §13.1 and §14.8, `test-server.md` §4.6 and §9.1,
  and `scenario-feed.md` §6.7, §9.8, §11.1 and §12.5.1, each Accepted in the same review
  (records in those specs). The non-blocking notes there (the §10.6 Leave codes, the bot
  minors of `test-server.md` §13.1, the wording of `scenario-feed.md` invariant 13) do not
  touch this decision.
