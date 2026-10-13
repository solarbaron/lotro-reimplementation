<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0034 — Official-server compatibility (M8) in scope by owner decision, without the legal-review gate

**Status:** Accepted by the owner, 2026-10-11.

- It supersedes the gate part of [decision 0004](0004-server-strategy.md).
- It amends decisions [0011](0011-standing-m4-authorisation.md),
  [0012](0012-numeric-constants.md), [0016](0016-plugin-class-helper.md),
  [0017](0017-extended-scripting.md) and [0024](0024-parity-track.md).
- It sets the research scope that [decision 0033](0033-rea-for-dirty-room-analysis.md) (REA)
  follows.

**This is not a legal review.** No lawyer has reviewed the design, and no rights holder has
given permission. The residual risks are listed in §5, and the owner accepts them.

## Context
- **Decision 0004 (2026-10-03).** Official-server compatibility became milestone M8, gated
  by `docs/LEGAL.md` §9.3. The gate had five conditions:
  - G1, a written legal review;
  - G2, permission sought from Daybreak Game Company LLC and Middle-earth Enterprises;
  - G3, a maintainer decision record citing G1 and G2;
  - G4, a user-facing warning about the ban risk;
  - G5, the permanently forbidden list of LEGAL §8.

  Sessions stopped at M8. The milestone workflow refused it.
- **Decision 0011 (2026-10-05).** It authorised disassembly and decompilation (method M4) of
  the offline client. It excluded the network protocol, login and authentication, keys and
  encryption, the launcher and patcher, and anti-cheat and anti-tamper, "still gated by
  decision 0004".
- **Decision 0017 (2026-10-07).** It made the rule "extended scripts never run against an
  official server" binding (its §6.4 and §6.5, LEGAL §9.5). It kept automation
  (`input.drive`) offline only.
- **The owner's decision of 2026-10-11.**
  - The owner said: "the official server compatibility will be here so change the legal
    and the plans".
  - The owner was asked whether a lawyer's review or permission from the rights holders
    exists. The owner chose: "Owner decision, keep G4/G5".
  - The owner then added: "extended scripts will be able to run on official server its just
    up to the user if they want to bear the risk to do that".
- **The facts of LEGAL §9.2 have not changed.** EULA §4.3–§4.5 and ToS §7.1, §10 and §12
  forbid what M8 builds unless Daybreak authorises it. *Daybreak v. Takahashi* (2025–26)
  accepted DMCA §1201 theories against an emulator of another Daybreak game. Every user who
  logs in with a third-party client risks their account.

## Decision

### 1. M8 is in scope now
1. **M8 is no longer gated.** Official-server compatibility is in scope now, as the owner's
   decision. Sessions do not stop at M8, and the milestone workflow accepts it.
2. **G1 and G2 are not required.** The owner sets them aside knowingly:
   - no written legal review exists (G1);
   - no permission from Daybreak or MEE has been sought or given (G2).

   Both stay recommended:
   - LEGAL §15 (engagement) remains the project's preferred route;
   - the questions of LEGAL §16 remain open for counsel.
3. **G3 is satisfied by this record.** This record does not cite G1 and G2. It records their
   absence instead. The owner accepted it on 2026-10-11.
4. **G4 is binding.**
   - **When it is shown.** Before any connection to an official server is possible, the
     client shows a warning. It is shown on first use and again whenever its text changes.
   - **What it says.**
     - The account can be suspended or closed (EULA §5, ToS §26.2).
     - Using a third-party client may breach the EULA and ToS (LEGAL §3).
     - The project cannot protect the account and offers no indemnity (LEGAL §13).
   - **How it is accepted.** Only by the user's explicit confirmation in the client's own
     interface. No command-line flag, environment variable, configuration file written by
     anything else, plugin, script or server can accept it or skip it.
   - **Until it is accepted,** the client makes no connection to an official server.
5. **G5 is binding.** It is the LEGAL §8 list that is permanently forbidden at every tier:
   - shipping, mirroring or linking to client binaries, dat files, patches or old clients;
   - modifying, patching, redirecting or injecting into the official client;
   - embedding or publishing Daybreak keys or other secrets in the repository, releases or
     documentation. Key material is loaded at runtime from the user's own install only;
   - committing packet captures from official servers;
   - using leaked, NDA-covered or decompiled Turbine/SSG/Daybreak source code;
   - monetisation of any kind beyond donations for infrastructure costs;
   - bypassing account authentication, subscription checks, anti-cheat, or any other access
     control.
6. **Order.** M8 may run beside the parity track (P0–P8, decision 0024) whenever the owner
   schedules it. M9 (extended scripting) stays last. It starts after P8 and after M8.

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
   - any circumvention of an access control;
   - the patcher's update and download function. Tessera never patches an install
     (LEGAL §7 item 2).
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
     may be stated under the rules of decision 0012. That is an amendment of 0012's line
     about "anything belonging to the network protocol, login".
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
1. **A compatibility backend.** It implements the `Backend` trait (ARCHITECTURE §19.3) in its
   own crate, so that it can be withdrawn without restructuring the client (§6). The M8
   plan decides where the crate sits in the workspace and how it is built, within these
   limits:
   - CI never connects to an official server;
   - tests use synthetic fixtures;
   - no official endpoint, key or account is in the repository or in CI secrets.
2. **Runtime only.** Endpoints and key material are read at runtime from the user's own
   install. The rules in CLAUDE.md and LEGAL §7 item 4 hold unchanged.
3. **Login.**
   - The client authenticates only as the user, with credentials the user enters.
   - It bypasses no subscription check or access control.
   - If the service can be used only by circumventing anti-cheat, anti-tamper or another
     access control, M8 stops at that point and records why. No workaround is built.
4. **The client half.** The parity track's presentation (decision 0024) receives the same
   domain events from the compatibility backend as from the scenario feed.
5. **Tessera's own server is unchanged.**
   - The rules of LEGAL §11.1 still apply.
   - The server never connects to, proxies or imitates an official service
     (`server-content.md` O8).
   - T3 (emulated gameplay) stays "not planned".
6. **Plugins.** Standard plugins are the original client's own feature. On an official server
   they run as they do in the original client, under LEGAL §17.

### 4. Extended scripts on official servers (amends decision 0017 §6.4 and §6.5, LEGAL §9.5)
The owner's words: "extended scripts will be able to run on official server its just up to
the user if they want to bear the risk to do that".

1. **Off by default, at the user's own risk.** Extended scripts run on an official-server
   session only after an explicit opt-in by that user.
2. **The opt-in.** It is a confirmation of its own, separate from the G4 warning.
   - **What it shows:**
     - the service may treat scripts as cheating or interference (ToS §7.1, §10);
     - the account can be suspended or closed (EULA §5, ToS §26.2);
     - the project cannot know what the service tolerates and cannot protect the account.
   - **How it is kept and changed.**
     - It is stored per user.
     - The user can withdraw it at any time.
     - It is asked again whenever its text changes.
     - Only the user can set it, in the client's own interface. No command-line flag,
       environment variable, other program's configuration write, plugin, script or server
       can set it.
3. **Grants still apply on top.** The opt-in grants no capability by itself. Each script still
   needs the user's grant (0017 §5). An official-server session gets a backend class of its
   own. The M8 and M9 specs name it. The `Other` class stays fail-closed with no override
   (0017 §6.2).
4. **Automation stays offline only.** This is project policy kept by the maintainer. It is not
   part of the owner's words.
   - It covers `input.drive` and every module that issues movement, input or game commands
     on the user's behalf.
   - These are never enabled on any official server, with or without the opt-in, because
     their effect falls on other players and on the service, not only on the user who
     accepted the risk.
   - The rule of 0017 §6.5 is unchanged for every server.
   - The owner may revisit this by a later decision. That decision must address the proxy
     case of 0017 §6.5.
5. **Unchanged rules.** These rules exist for reasons other than official-server risk:
   - the local-only rules (0017 §5.2);
   - `data.read` offered offline only (LEGAL §18.4, an export guard);
   - the never-offered list (LEGAL §18.3 rule 3);
   - `Sandbox` only in the sandbox;
   - `Server` only on Tessera's own protocol.
6. **The build-level exclusion of 0017 §6.3 is withdrawn.** It enforced a rule that no longer
   holds. That covers:
   - the shared `links` guard;
   - the CI check that keeps `tessera-ext` and a compatibility crate out of one graph.

   The compatibility backend stays its own crate (§3 item 1). The M9 slice plan replaces
   the check with tests of the runtime gate:
   - no extended-script apartment runs on an official-server session without the opt-in;
   - no automation module is available there with it.
7. **When.** Extended scripting is M9. Nothing of this item is built before M9.

### 5. Residual risks the owner accepts
These are the risks of LEGAL §9.2, unchanged by this record:

- **Contract.**
  - EULA §4.3 forbids playing "through interception, emulation or redirection of the
    communication protocols". §4.4 forbids "any unauthorized connection". §4.5 forbids
    decrypting client–server data.
  - ToS §7.1 and §10 forbid deciphering transmissions and circumventing access measures.
    ToS §12 restricts the client software.
  - A third-party client on the live service breaches these on its face unless Daybreak
    authorises it. Contributors who accepted the EULA carry this personally (LEGAL §13).
- **DMCA §1201.**
  - The official login uses encryption and key material that ship inside the official
    client.
  - The *Daybreak v. Takahashi* court treated server-side authentication as an access
    control, and distributing a workaround as trafficking.
  - Implementing the official login in third-party code risks §1201(a)(1) circumvention
    claims. Publishing builds that contain it risks (a)(2) trafficking claims. In the 9th
    Circuit neither needs any copyright infringement.
  - §1201(f) is narrow. *bnetd* held it waived by contract.
  - Authenticating with the user's own credentials is not bypassing authentication under G5.
    A court may still see it differently.
- **Account bans.** Every user who logs in with Tessera risks their account, purchases and
  characters (EULA §5, ToS §26.2). Extended scripts on an official server add to that risk.
- **Service integrity.** A third-party client on a live service can be treated as cheating
  or interference (ToS §7.1, §10) even if it does nothing unusual.
- **EU law helps less than it seems.** Art 6 of Directive 2009/24/EC covers *obtaining*
  interface information. It does not license connecting to a service against its terms, and
  Art 6(3) protects the rightholder's "normal exploitation". Outside the EU and UK even
  that does not apply.
- **No review and no permission.** The project proceeds without counsel's view on any of
  the above and without the rights holders' consent.

### 6. What does not change
- **Objection from a rights holder.** The project will not proceed against an explicit
  objection from Daybreak or MEE (LEGAL §9.4, §14). On such an objection, M8 work stops and
  the compatibility crate is withdrawn.
- **Legal documents and process.**
  - The open questions of LEGAL §16 stay open for counsel.
  - The LEGAL banner's call for a lawyer's review before any public release stands (§7
    item 1).
  - The clean-room process, the content rules and the naming rules are unchanged.
- **Decision 0024.** Its "client half only" choice and LEGAL §11.1 rule 5 are unchanged.

### 7. Open for the owner
1. **Public release.** The LEGAL banner asks for a lawyer's review before any public release.
   Publishing builds that include the compatibility backend carries the trafficking risk of
   §5. This record does not decide when, or whether, such builds are published.
2. **The automation rule of §4 item 4.** It is the maintainer's. The owner may confirm it or
   revisit it by a later decision.
3. **`data.read` on an official-server session after the opt-in.** It stays offline only
   (§4 item 5) unless the owner decides otherwise and LEGAL §18.4 is re-assessed.

## Consequences
- **Decision records.**
  - Decision 0004 keeps its text, with a status note pointing here. The gate part is
    superseded, and its server strategy for M0–M7 stands.
  - Decisions 0011, 0012, 0016, 0017 and 0024 get short amendment notes pointing here.
  - Decision 0033 keeps its terms. Its change log records that this record names REA, and the
    targets and exclusions of §2 items 1 and 2 that it now follows (§2 item 5).
  - `docs/decisions/README.md` gets the 0034 row and a note on 0004.
- **LEGAL.md.** §1 items 3–4, §8 (T4 status), §9.1–§9.5, §13, §15, §16 and §18.2 are
  updated. §9.5 is renamed "Extended scripts and official servers", and its old anchor
  still works.
- **Other documents.** These are brought in line:
  - CLEAN_ROOM.md §4;
  - ROADMAP §1, §2.2, M8 and M9;
  - ARCHITECTURE §2, §19.2, §19.3 and the open-questions table;
  - TOOLING.md §2 and §7;
  - DEVELOPMENT.md;
  - CLAUDE.md, README.md, DISCLAIMER.md and CONTRIBUTING.md;
  - the spec-writer and clean-implementer agents;
  - the milestone workflow. It now runs M8 with the rules of this record in its dirty
    prompt, and still refuses M9.
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
- **Code.** These comments still describe the gate. They are reworded in the first M8
  implementation commit:
  - the doc comment of `crates/net/src/compat.rs`;
  - the section comment and the doc comment of the compression dictionaries in
    `crates/formats/src/gameplay/misc.rs` ("behind the M8 gate").
- **Withdrawing this record** means:
  - removing the compatibility crate;
  - restoring the gate of LEGAL §9.3;
  - restoring the rule "extended scripts never run against an official server";
  - moving facts that rest only on official-protocol research out of the specs.

## Change log
| Date | Change |
|------|--------|
| 2026-10-11 | Recorded and accepted by the owner. G1 and G2 set aside, G3 met by this record, G4 and G5 binding. Extended scripts on official servers by per-user opt-in, with automation kept offline only as maintainer policy. M4 scope widened to the official protocol and login flow. REA named for that scope. |
| 2026-10-11 | Clarifications after the independent review of the policy changes, same day; no decision of the owner changed. §2 item 1 states the targets (client and launcher binaries by subject, the launcher only for its part of the login flow, never the patcher). §2 item 5 ties REA to the targets and exclusions of items 1 and 2 and keeps its allow-list. Consequences list the specs that still say "gated" and the code comment in `crates/formats/src/gameplay/misc.rs`, and say that decision 0033 gets a change-log row. |
