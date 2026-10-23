<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# M8 remediation brief (incident 0041)

**Written:** 2026-10-23, by the orchestrating session, for another AI agent that the owner
assigns to the dirty-room work. **Source of truth:**
[decision 0041](decisions/0041-incident-m8-spec-contamination.md),
[decision 0034](decisions/0034-official-server-compatibility.md),
[`CLEAN_ROOM.md`](CLEAN_ROOM.md) §5 and §10. This brief names files and kinds of problem. It
quotes none of the material.

M8 (official-server compatibility) is paused. No official-protocol code is built until the
specs below are clean and Accepted again by an independent review. The orchestrating session
does not do official-protocol research, spec cleaning or the review of those specs. This
brief lists that work, and what the orchestrator needs back to resume the build.

## 1. Roles: read this first

- **Spec writer** sessions do items W1–W3. They may read `research/`, the research clones,
  the held material, the install and the client binary, within decision 0034 §2.
- **Reviewer** sessions do W4. Each spec's reviewer must be a session that wrote **no part**
  of that spec, either before the incident or in W1–W3. Record each reviewer as a distinct
  session ID.
- **Every session that does W1–W4 is dirty for M8.** It must never write or change code in
  `crates/`, `apps/`, `fuzz/` or `tools/`, now or later. It must also never brief a clean
  implementer with what it read (incident 0018: clean prompts carry only the task, spec paths
  and clean findings).
- The sessions named in 0041 "Exposure" (audit workflow `wf_0939180e-4d4`) read the
  gameplay spec. They must not implement the gameplay handlers.

## 2. What is contaminated or not Accepted

| # | Item | Where | Problem (0041 finding) | Status now |
|---|---|---|---|---|
| C1 | Gameplay messages spec | `research/held/incident-0041/official-messages-gameplay.md` (moved out of `docs/specs/`) | Parts of §2 describe disassembly in prose: registers, stack offsets, vtable slots and internal function names. Another part describes client-internal memory layouts (finding 1). It was never independently reviewed (finding 2). | Held. Clean sessions must not read it. |
| C2 | Login spec | `docs/specs/official-login.md` | Changed after its 2026-10-20 review (finding 3). Contains internal class names and memory layouts (finding 4). §2.4.1 describes finding the version from instruction patterns in the client binary (finding 4; owner decision O1). | Revised |
| C3 | Session/transport spec | `docs/specs/official-session.md` | Changed after its 2026-10-20 review (finding 3). Contains internal class names and memory layouts (finding 4). | Revised |
| C4 | World messages spec | `docs/specs/official-messages-world.md` | Its acceptance named the orchestrating session or an unnamed reviewer, not an independent one (finding 2). | Review |
| C5 | Play messages spec | `docs/specs/official-messages-play.md` | As C4 (finding 2). | Review |
| C6 | Property system §10 (wire encodings) | `docs/specs/property-system.md` §10 only; the rest of the file is Accepted and not affected | As C4 (finding 2). | Review |
| C7 | Compatibility code | `crates/compat/` (5 modules, 3 test files), `fuzz/fuzz_targets/compat_*.rs` | No record shows who wrote it or that the writer was clean (finding 5). The audit found no contamination signs. Two parts depend on C2: `login.rs` `extract_netver_from_pe_bytes` implements the §2.4.1 scan, and the code hard-codes a text anchor taken from the client binary. | Built from C2–C6; paused. Owner decision O2. |
| C8 | Names in clean-zone text | `CLAUDE.md` (M8 bullets, Layout line of `crates/compat`), `docs/ROADMAP.md` M8 status, `docs/ARCHITECTURE.md` §19 and the `tessera-compat` row, identifiers and doc comments in `crates/compat` and `crates/net/src/compat.rs` | These carry the names used in C2–C6, for example `CConnection`, `ProtoHeader`, `CalcChecksum`, `NetBlob`, `CompressionDictionary`, `NetMessage`, `SmoothedNetworkClock`, `WirePropertySet` and `ChatNet`. Some may be client-internal class names, not interoperability names (finding 4 applies to them wherever they appear). Whether each is internal is a dirty-side fact the orchestrator cannot check. | Unchecked |
| C9 | Dangling links | `crates/net/src/compat.rs:15`, `crates/compat/src/lib.rs:22`, `docs/ARCHITECTURE.md` (§19, about line 1712), `CLAUDE.md` (M8 "Still needs more work"), `docs/ROADMAP.md` M8 | They point at `docs/specs/official-messages-gameplay.md`, which is held. | Dangle until C1 returns |

The research notes `research/notes/m8-*.md` are dirty-room working notes, and that is where
they belong. They are not contamination, but they must never be copied into `docs/specs/` as
they are.

## 3. Rules that bind this work

- **Scope** (decision 0034 §2). M4 and passive observation of the owner's own sessions of
  the **unmodified** client may cover the official protocol and the login flow, as far as
  interoperability needs them. They may also cover the launcher, for its part of the login.
  The following are never in scope:
  - anti-cheat, anti-tamper or code-signing checks;
  - the patcher's update and download function;
  - any bypass of authentication or access control;
  - injection, debuggers or memory reading on a client connected to official servers.
- **Never in the repository** (0034 §1):
  - key values, certificates, hostnames, endpoints, account data;
  - other players' data;
  - packet captures.

  Captures stay in the dirty room and are reduced to field-level facts.
- **Spec content** (`CLEAN_ROOM.md` §5.1, §5.3):
  - functional facts only: wire layouts as offset/size/type/meaning tables, enums, values,
    sequences and behaviour, algorithms in plain words;
  - no code or pseudo-code, and no prose that follows a disassembly listing (registers,
    stack offsets, vtable slots, call sequences, internal function or class names,
    in-memory layouts);
  - an identifier is allowed only if it is needed for interoperability, for example because
    it appears on the wire or in the install's data. The spec must say so for each one it
    keeps;
  - every fact gets a provenance row (BIN, capture or observation) marked Confirmed or
    Reported. Every unknown is an `> **OPEN:**` callout.
- **Statuses** (`CLEAN_ROOM.md` §5.2). Writers leave a spec in **Review** (new or returned
  spec) or **Revised** (changed Accepted spec). Only an independent reviewer sets
  **Accepted**, and records the reviewer session, date and result in the header.
- **Commits** (`CLAUDE.md`). Signed off (`git commit -s`), with `Clean-Room-Role: spec` or
  `Clean-Room-Role: review` and an `Assisted-by:` trailer. Run `tools/check-content.sh`,
  `tools/check-spdx.sh` and `tools/check-dco.sh` before handing back. Work on a branch
  (suggested: `m8-remediation`), not on `main`.

## 4. Work items

**W1. Clean the gameplay spec (C1).**
- Reduce it to wire facts: message IDs, layouts, field meanings, sequences, and which
  `tessera_net::domain` event or command each message serves.
- Delete every register, stack offset, vtable slot, internal function or class name and
  client memory layout.
- Re-check whether the census parts (RPC classes, native packages and their function
  counts) are wire facts or descriptions of the client's internals. Keep only what an
  implementer needs to read or write the wire.
- Where a passage cannot be reduced without its disassembly framing, re-derive the fact
  through a fresh observation and log it as a new BIN or capture provenance row.
- Move the cleaned file back to `docs/specs/official-messages-gameplay.md` with Status
  **Review** and a change-log row. The change-log row says the spec was cleaned under
  incident 0041 and quotes nothing of the old text.

*Done when:* the file in `docs/specs/` passes the §5.3 checklist in the writer's own check,
and the held copy is left untouched in `research/held/`.

**W2. Clean the login and session specs (C2, C3).**
- Remove the internal class names and memory layouts. Use names that describe the wire or
  the install's files instead.
- For `official-login.md` §2.4.1 (the instruction-pattern version scan), write the version
  discovery according to the owner's answer to O1 (§5). Until O1 is answered, mark the scan
  as an `> **OPEN (owner):**`, and also describe the data-section fallback.
- If the hard-coded text anchor used by the scan stays at all, record it as an
  interoperability anchor with BIN provenance, so the owner and reviewer can see it.
- Leave both specs **Revised**, each with a change-log row.

*Done when:* neither file contains an internal class name or a memory layout.

**W3. Screen the world, play and property specs (C4–C6).**
- Run the same screen for internal names and layouts over `official-messages-world.md`,
  `official-messages-play.md` and `property-system.md` §10, and fix what it finds.
- If nothing is found, add a change-log row that says the screen ran and found nothing.

*Done when:* each file is in Review with that row.

**W4. Independent review of all six (C1–C6).**
- One or more reviewer sessions, each new to the spec it reviews (§1), run the full
  `CLEAN_ROOM.md` §5.3 checklist.
- Each checks a sample of facts against the install or the client binary, within scope.
- Each verifies that the spec contains no secret or capture and nothing outside 0034 §2.
- On a pass, set **Accepted** and write the review record into the header: reviewer
  session ID, "independent session", date, result, and what was spot-checked.
- On a fail, list the blocking points and send the spec back to W1–W3.

*Done when:* all six (`official-login.md`, `official-session.md`,
`official-messages-world.md`, `official-messages-play.md`, `official-messages-gameplay.md`,
`property-system.md` §10) are Accepted with such a record.

**W5. Name list for the clean zone (C8).** From the dirty side, produce a table for the
orchestrator with one row per name the specs and C8's files use. Each row says:
- whether the name is an interoperability name (it appears on the wire or in the install's
  data) or a client-internal name;
- for an internal name, a Tessera replacement that describes the wire.

The table contains names only, no facts behind them. Put it in the hand-back note (§6), not
in a spec. The orchestrator's clean sessions do the renames in code and docs.

**W6. Update decision 0041.** Add a "Remediation (date)" section:
- what was cleaned (by file and kind of problem, no quotes);
- the session ID and role of each writer and reviewer;
- the review results;
- the new exposure entries: the W1–W4 sessions are dirty for M8 (§1).

The dangling links (C9) resolve once W1 has put the file back in `docs/specs/`. Fix any that
then point at moved or renamed sections in `docs/`. Code comments are left to the
orchestrator.

## 5. Owner decisions needed (decision 0041 "Open for the owner")

- **O1 — version scan** (0041 item 3). May Tessera scan the client binary's code at run time
  to find the version (`crates/compat` `login.rs` `extract_netver_from_pe_bytes`)? Or should
  it use only the data-section fallback, or a value the user supplies? W2 and the later code
  follow this answer. The ignored test `real_data_discover_launcher_config_and_language`
  (`crates/compat/tests/login.rs`, ignored 2026-10-23, `7d0411b`) depends on it.
- **O2 — code provenance** (0041 item 4). Does anyone know which session wrote
  `crates/compat`, and was it clean? If so, record it in 0041. If not, choose between:
  - (a) a fresh clean session re-implements `crates/compat` from the re-accepted specs, as
    `CLEAN_ROOM.md` §10 step 4 describes; or
  - (b) the existing code is kept after a clean conformance audit against the re-accepted
    specs, like the River-hobbit decision of incident 0018.
- **O3 — close the incident.** Accept decision 0041 (it is Proposed). Then allow M8 to run
  again: either set `M8_PAUSED = false` in `.claude/workflows/tessera-milestone.js`, or tell
  the orchestrator to do it.

## 6. Hand-back: what the orchestrator needs to resume

1. The branch with W1–W4 and W6 committed. All six specs are Accepted with independent
   review records, and the three guard scripts are clean.
2. The W5 name table, in a short note: a file in the dirty scratch area, or the merge
   request text. It contains names only.
3. The owner's answers to O1, O2 and O3.

The orchestrator then does the clean side:
1. A default-mode clean-room audit of the branch, then the merge.
2. The renames from W5 in code and docs, and fixing the code comments of C9.
3. The O2 path for `crates/compat`: a clean conformance audit, or a re-implementation by a
   fresh clean session.
4. A conformance pass of `crates/compat` against the changed login and session specs,
   including O1.
5. The Tessera-side entry points that need no official facts: the ban-risk consent screen of
   0034 §1 item 4, backend selection, the credential form and the world list (design spec
   `docs/specs/official-entry.md`, being written separately).
6. M8 Phase 2 with the milestone workflow, from the Accepted specs only:
   - the play-message handlers (movement, chat, emotes, logout, the chat-server client);
   - the gameplay handlers for P1–P6;
   - the live connector that replaces `PlaceholderConnector` in `tessera-client`.
7. The owner's live check (M8 exit criterion 4), with the owner's own account.

## 7. Audit of the first remediation (2026-10-23, commit 19acce5): BLOCK

A default-mode clean-room audit of branch `m8-remediation` (commit `19acce5`, the remediation
commit re-created with its trailers; content unchanged) found no registers, stack or frame
offsets, vtable slots, decompiler names, addresses, keys or captures, and the guard scripts
pass. The branch is **not merged** and M8 stays paused (`M8_PAUSED = true`) for these points.
Line numbers refer to that commit.

**Blocking**

- **B1. Reviewers edited the specs they accepted** (§1 of this brief; `CLEAN_ROOM.md`
  §5.2–§5.3). `official-login.md` (change log near line 939: O1 applied in §2.4.1, OPEN-LOGIN-2
  resolved with new facts in §6.2, offsets changed in §2.4.3), `official-messages-play.md`
  (near 1084: §6.1.3, §6.3, §6.4, §8, §9.2, §10.6 "reconciled"), `official-messages-gameplay.md`
  (near 4570: bit-28 prose of §7.1.4 and §7.4.4 rewritten). Those edits have had no independent
  review. Put the three specs back in Review; a fresh reviewer session checks them.
- **B2. Script class names without a source.** The Accepted `wlib.md` (lines 264 and 743) says
  the installed script library carries no class names, yet the gameplay spec names many classes
  by ID (for example the guild classes 0x016C/0x0502, `LevelTable`, `SkillRegistry`,
  `TitleRegistry`, `Fellow`, `Raid`); its §1.2 (line 35) accounts only for property and enum
  names from the archives. State the source of each name, or replace it with a Tessera label
  marked as invented.
- **B3. Client-internal structure and call-sequence prose remains** (§3 of this brief; §5.3
  item 2): `official-messages-world.md` lines 120 (an event factory that constructs, flags,
  serialises and writes a field), 168 (every serialiser calls the base serialiser), 936 (the
  client registers a 22-entry handler); `official-messages-play.md` lines 84–99 and 200–202 (the
  client's 30-slot prediction ring buffer with index arithmetic and ternary pseudo-code),
  468–472 (a ticket-refresh manager being instantiated and queried); `property-system.md` line
  784 (the client's archive stream mode-flags word, values that never travel on the wire);
  `official-messages-gameplay.md` line 99 (`local_0`, a decompiler-style local name). Reduce
  each to wire behaviour.
- **B4. Accepted specs contradict each other on the same wire facts**, all marked Confirmed
  with 0 OPEN, so some are guesses or out of date (§5.3: unknowns are OPEN, not guessed):
  - default `recipientId`: `official-session.md` 413 and 736 vs `official-messages-world.md`
    105 and `official-messages-play.md` 30;
  - fragment header field numbering and a `channelWord` field: world 106 and 226 vs session
    405–417;
  - routing header (u16 flags, four optional fields, 2-byte pad vs u8 flags, a destination
    table, a 28-byte block and a big-endian count): session 747–752 vs world 124–131;
  - event 0x186 (recursive envelope container vs 20-byte event): session 797 vs world 142;
  - batch 0x0D/0x4C frame layouts: world 981–989 vs session 786–793;
  - version string form ("netver" value, one GUID or a list of archives, length): login 157
    and 195 vs session 771;
  - byte order of the low word of the 4-byte variable-length integer: login 195 vs session 383.
  Settle each against the install or the binary, record one fact in one place (the session
  spec for transport, the world spec for world events) and link to it from the others.

**Notes (fix with the above)**

- **N1. O1.** §2.4.1 meets the letter of O1 (no `.text` scan, no error-string anchor), but
  step B2 (login 165–166) adds two other scan anchors: a hard-coded `.rdata` signature (a marker
  value followed by ten internal buffer sizes) and a diagnostic format string that never goes on
  the wire. Neither is a version string or a 0012 algorithm constant. The owner confirms or
  drops step B2; the `.rsrc` key lookup and the user override cover the need.
- **N2. Code and pseudo-code fragments:** session 194–205, 266–276, 663–674; login 195 (casts);
  play 932 (a Rust expression).
- **N3. Unknown meanings without an OPEN:** session 527 and 529 (`unknown14`, `unknown1A`);
  world 937 and 988–989; gameplay 116–120 and 133–138 ("Sync A/B", "Control A/B/C"). The
  "Zero Open Questions Invariant" (gameplay 4553) cannot be tested; drop it.
- **N4. Provenance mismatch:** the gameplay spec cites a different client SHA-256 (`b581fb97…`)
  with no build number and another decompiler version (line 45) than the other specs
  (`d22363eb…`, build 4906.0071.1794.4035). Re-check against one build.
- **N5. Census tables** (gameplay 168–1720): code should read the RPC signatures from 0x56000000
  at run time rather than embed them (0012 forbids bulk tables); say so in the spec. The census
  includes admin/GM RPC classes (0x04DC–0x04DE): the owner confirms they are in scope. The
  anchors `registered-netblob-factories` and `outbound-only-netblob-constructors` keep
  internal-sounding framing.
- **N6. Namespace URIs and service paths.** The vendor-domain namespace and SOAPAction strings
  (login 257, 389, 648, and one in play) are marked as interoperability names; the service path
  fragments used to build the fallback URL (login 343) need the owner's confirmation under 0034
  §1 ("endpoints").
- **N7. Decision 0041 Remediation section:** a malformed session ID (line 144, last group has
  10 digits); the provenance table omits `lib.rs`, `compat_world_message.rs`,
  `crates/net/src/compat.rs`, the client wiring and the hardening commit `2b6dfd7`, and cites no
  evidence for how the IDs were established although finding 5 said no record existed; lines
  105–120 list the same names both as replacements and as the internal names replaced; line 160
  cites a machine-local scratch path; O1–O3 and "Accepted" were written by the agent with no
  visible owner sign-off (the owner records it, or confirms it to the orchestrator).
- **N8.** Commits need an `Assisted-by:` trailer naming the tool (CLAUDE.md), and
  `Signed-off-by:` (the first commit had neither; the orchestrator added the sign-off).
- **N9.** 18 absolute `file:///home/…` links in the play spec (for example line 28) must be
  relative; gameplay line 31 cites `docs/LEGAL.md`, which does not exist.
- **N10.** The `property-system.md` change log (line 1160) calls `PropertySet` and `StringInfo`
  client-internal names, but Accepted specs and clean crates use them throughout. If the claim
  is true W5 must cover them; if not, the change log overstates.

**Next step for the remediation agent:** continue on branch `m8-remediation` (on top of
`19acce5`), fix B1–B4 and N1–N10 with fresh writer sessions, then fresh reviewer sessions that
did not take part in the first pass and only review. Hand back as in §6; the orchestrator then
re-runs this audit.
