<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0034 — Official-server compatibility (M8) architecture and research scope

**Status:** Accepted by the owner, 2026-10-11.

- It supersedes the staging gate of [decision 0004](0004-server-strategy.md).
- It amends decisions [0011](0011-standing-m4-authorisation.md),
  [0012](0012-numeric-constants.md), [0016](0016-plugin-class-helper.md),
  [0017](0017-extended-scripting.md) and [0024](0024-parity-track.md).
- It sets the research scope that [decision 0033](0033-rea-for-dirty-room-analysis.md) (REA)
  follows.

## Context
- **Decision 0004 (2026-10-03).** Official-server compatibility was placed at milestone M8
  behind a staging boundary so that M0–M7 would target only the in-process sandbox backend
  and the project-owned test server. Sessions stopped at M8 and the milestone workflow
  refused it.
- **Decision 0011 (2026-10-05).** Authorised disassembly and decompilation (method M4) of
  the offline client, while excluding the network protocol, login and authentication, keys
  and encryption, the launcher and patcher, and anti-cheat and anti-tamper until M8 entered
  scope.
- **Decision 0017 (2026-10-07).** Set the initial rule that extended scripts run offline and
  on Tessera's own protocol only (its §6.4 and §6.5), and kept automation (`input.drive`)
  offline only.
- **The owner's decision of 2026-10-11.** The owner placed M8 official-server compatibility
  in scope, kept the repository content boundaries (no game data, binaries, keys, endpoints,
  or packet captures in the repository; no bypassing of official authentication), and opened
  extended scripts on official-server sessions behind an explicit per-user opt-in (off by
  default, with automation remaining offline only).

## Decision

### 1. M8 is in scope now
1. **M8 is in scope.** Official-server compatibility is an active milestone. Sessions do not
   stop at M8, and the milestone workflow accepts it.
2. **Repository content and boundary rules remain binding (`CLAUDE.md`):**
   - never ship, mirror or link to client binaries, `.dat` files, patches or old clients;
   - never modify, patch, redirect or inject into the official client;
   - never embed or publish cryptographic keys, certificates, hostnames, endpoints or other
     secrets in the repository, releases or documentation — key material and service
     endpoints are loaded at runtime from the user's own install only;
   - never commit packet captures from official servers;
   - never use leaked, NDA-covered or decompiled vendor source code;
   - never bypass account authentication, subscription checks, anti-cheat, or any other
     access control.
3. **Order.** M8 may run beside the parity track (P0–P8, decision 0024) whenever the owner
   schedules it. M9 (extended scripting) stays last: it starts after P8 and after M8.
4. **Ban-risk consent stays binding (owner, 2026-10-22).** Before any connection to an
   official server is possible, the client shows a warning about the account risk of using
   a third-party client:
   - it is shown on first use and again whenever its text changes;
   - only the user's explicit confirmation in the client's own interface accepts it. No
     command-line flag, environment variable, configuration file written by anything else,
     plugin, script or server can accept or skip it;
   - until it is accepted, the client makes no official connection.

### 2. Research scope (amends decision 0011)
1. **What M4 may now cover.** Disassembly and decompilation (decision 0011) may now cover the
   official game protocol and the login flow, as far as interoperability needs them:
   - message framing and layouts, and session set-up;
   - the authentication sequence that the official client performs with the user's own
     credentials, including the part the official launcher performs;
   - the encryption as the protocol uses it: the algorithm, and **where** the key material
     lies in the user's install. Never the key values.

   **Targets.** The scope is set by subject, not by file. M4 may analyse read-only copies of
   the client binary and of the official launcher binary, and of a library either of them
   loads where in-scope code lies in it. The launcher is analysed only for its part of the
   login flow. The patcher and its update and download code are never analysed (item 2).
2. **What stays excluded:**
   - anti-cheat, anti-tamper and code-signing checks;
   - bypassing official authentication or access controls;
   - the patcher's update and download function. Tessera never patches an install.
3. **Observation (method M3).** The owner's own sessions of the **unmodified** official
   client may be captured passively, on the owner's own machine and account.
   - No injection, debugger or memory reading may touch a client that is connected to an
     official server. `docs/TOOLING.md` §2 rule 5 is unchanged.
   - Decrypting a capture is in scope only by a method that needs none of those tools.
   - Captures are dirty-room material only. They are never committed, attached to issues or
     shown to a clean session.
   - Captures are reduced to field-level facts. Specs carry no other player's name, chat
     text or account data.
4. **Specs.**
   - Official-protocol specs are written like any other spec: prose and tables, with `BIN`
     or `OBS` provenance.
   - Numeric constants that define the protocol (message codes, sizes, algorithm constants)
     may be stated under the rules of decision 0012. That amends 0012's line about "anything
     belonging to the network protocol, login".
   - These stay forbidden in specs, code and the repository:
     - keys, certificates and other secrets;
     - hostnames and endpoints;
     - account data.

     A spec says where the client finds them at runtime in the user's install.
5. **REA (decision 0033).** Decision 0033 item 1 requires a later decision to name REA before
   REA may be used on what that decision opens. This record names it. REA may be used within
   the amended M4 scope of item 1, on the targets item 1 names, and never on anything item 2
   excludes. Its tool allow-list (decision 0033 item 5) is not widened. Every other term of
   decision 0033 still applies:
   - the pilot comes first;
   - REA runs only in an owner-started dedicated session;
   - REA is never used in workflow runs;
   - its output stays dirty.

### 3. What may be built
1. **A compatibility backend in `crates/compat`.** It implements the `Backend` trait
   (ARCHITECTURE §19.3) in its own crate (`crates/compat`), so that official-protocol logic
   stays isolated from `crates/net` and is never linked by `tessera-server`:
   - CI and automated tests never connect to an official server;
   - tests use synthetic fixtures;
   - no official endpoint, hostname, port, key, certificate or account is in the repository
     or in CI secrets.
2. **Runtime only.** Endpoints and key material are read at runtime from the user's own
   install (`CLAUDE.md` content rules).
3. **Login.**
   - The client authenticates only as the user, with credentials the user enters.
   - It bypasses no subscription check or authentication step.
   - If the service requires anti-cheat, anti-tamper or another excluded mechanism that
     Tessera does not implement, M8 stops at that point and records why. No workaround is
     built.
4. **The client half.** The parity track's presentation (decision 0024) receives the same
   domain events from the compatibility backend as from the scenario feed.
5. **Tessera's own server is unchanged.**
   - The rules of `server-content.md` still apply.
   - The server never connects to, proxies or imitates an official service
     (`server-content.md` O8), and never links `crates/compat` (`tools/check-server-deps.sh`).
   - Server-side gameplay emulation stays out of scope (`server-content.md` §7).
6. **Plugins.** Standard plugins are the original client's own feature. On an official server
   they run through `crates/plugins` as they do in the original client.

### 4. Extended scripts on official servers (amends decision 0017 §6.4 and §6.5)
1. **Off by default.** Extended scripts run on an official-server session only after an
   explicit opt-in by that user.
2. **The opt-in.**
   - It is stored per user and can be withdrawn at any time.
   - Only the user can set it, in the client's own interface. No command-line flag,
     environment variable, other program's configuration write, plugin, script or server can
     set it.
3. **Grants still apply on top.** The opt-in grants no capability by itself. Each script still
   needs the user's grant (0017 §5). An official-server session gets a backend class of its
   own. The M8 and M9 specs name it. The `Other` class stays fail-closed with no override
   (0017 §6.2).
4. **Automation stays offline only.** This is project policy kept by the maintainer:
   - It covers `input.drive` and every module that issues movement, input or game commands
     on the user's behalf.
   - These are never enabled on any official server, with or without the opt-in.
   - The rule of 0017 §6.5 is unchanged for every server.
   - The owner may revisit this by a later decision. That decision must address the proxy
     case of 0017 §6.5.
5. **Unchanged rules:**
   - the local-only rules (0017 §5.2);
   - `data.read` offered offline only (an export guard);
   - the never-offered list (0017 §3);
   - `Sandbox` only in the sandbox;
   - `Server` only on Tessera's own protocol.
6. **The build-level exclusion of 0017 §6.3 is withdrawn.** That covers:
   - the shared `links` guard;
   - the CI check that keeps `tessera-ext` and a compatibility crate out of one graph.

   The compatibility backend stays its own crate (§3 item 1). The M9 slice plan replaces
   the check with tests of the runtime gate:
   - no extended-script apartment runs on an official-server session without the opt-in;
   - no automation module is available there with it.
7. **When.** Extended scripting is M9. Nothing of this item is built before M9.

### 5. What does not change
- **Clean-room process, content rules and naming rules.** `docs/CLEAN_ROOM.md` and `CLAUDE.md`
  are unchanged.
- **Decision 0024.** Its "client half only" choice and the data-free server scope
  (`server-content.md` §7) are unchanged.

### 6. Open for the owner
1. **The automation rule of §4 item 4.** The owner may confirm it or revisit it by a later
   decision.
2. **`data.read` on an official-server session after the opt-in.** It stays offline only
   (§4 item 5) unless the owner decides otherwise.

## Consequences
- **Decision records.**
  - Decision 0004 keeps its text, with a status note pointing here. The staging gate is
    superseded, and its server strategy for M0–M7 stands.
  - Decisions 0011, 0012, 0016, 0017 and 0024 get short amendment notes pointing here.
  - Decision 0033 keeps its terms. Its change log records that this record names REA, and the
    targets and exclusions of §2 items 1 and 2 that it now follows (§2 item 5).
  - `docs/decisions/README.md` gets the 0034 row and a note on 0004.
- **Other documents.** These are brought in line:
  - CLEAN_ROOM.md §4;
  - ROADMAP §1, §2.2, M8 and M9;
  - ARCHITECTURE §2, §19.2, §19.3 and the open-questions table;
  - TOOLING.md §2 and §7;
  - DEVELOPMENT.md;
  - CLAUDE.md, README.md, DISCLAIMER.md and CONTRIBUTING.md;
  - the spec-writer and clean-implementer agents;
  - the milestone workflow, which runs M8 with the rules of this record and still refuses M9.
- **Specs.**
  - The specs of the extended-scripting design (`ext-threat-model.md`, `ext-manifest.md`,
    `ext-api-1a.md`, `ext-api-1b.md`) get a note pointing to §4 and a change-log row. The M9
    revision carries the change into their text.
  - Specs that left a fact open "behind the M8 gate" may now answer it within §2. Their
    provenance notes about what was not examined stay true for the passes that wrote them.
  - Specs edited with this record (`game-controls.md`, `misc-records.md`,
    `tessera-protocol.md`, `transport-design.md`, `server-content.md`) say "gated" only in the
    past tense and carry a 2026-10-11 change-log row or status note.
  - Other specs still say "gated", "the M8 gate" or "gated by decision 0004" in the present
    tense, among them `string-table.md` (the OPENs of §4.8 and §9.0 and the §14 summary),
    `property-system.md` (the §4.3 OPEN and the §13 summary), `ui-behaviour.md`,
    `npc-drama.md` and `player-character.md`. Read each as "not researched before decision
    0034". Each is reworded, with a change-log row, in the next revision of that spec.

## Change log
| Date | Change |
|------|--------|
| 2026-10-11 | Recorded and accepted by the owner. M8 placed in scope in a dedicated `crates/compat` crate. Extended scripts on official servers by per-user opt-in, with automation kept offline only as maintainer policy. M4 scope widened to the official protocol and login flow. REA named for that scope. |
| 2026-10-11 | Clarifications after independent review, same day. §2 item 1 states the targets (client and launcher binaries by subject, the launcher only for its part of the login flow, never the patcher). §2 item 5 ties REA to the targets and exclusions of items 1 and 2 and keeps its allow-list. Consequences list the specs that still say "gated" and say that decision 0033 gets a change-log row. |
| 2026-10-20 | Owner confirmation of the login-research scope: (a) the official launcher's part of the login flow, from published third-party launchers (M2) and observation of the install (M1), with M4 on the launcher binary for its login part where those are not enough; and (b) M4 on the client binary's authentication and session-ticket handling, so that Tessera can log in with the user's own credentials. Unchanged: the user's own credentials only; no key, certificate, hostname or endpoint values in the repository (read at runtime from the user's install); the §2 item 2 exclusions; no bypassing of authentication or access controls (§3 item 3). |
