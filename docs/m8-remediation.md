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

**Decisions on N1, N5, N6 and N7:** [decision 0042](decisions/0042-m8-remediation-decisions.md)
(owner-delegated, 2026-10-23). Drop step B2 of `official-login.md` §2.4.1 and the fallback
URL built from path fragments (§2.4.1 then names only the user value and the version
resource); admin/GM classes are out of scope; namespace and SOAPAction identifiers may stay as
interoperability names; reviewers never edit what they review.

## 8. Audit of the second remediation pass (2026-10-24, commit 6c71fe66): BLOCK

A default-mode clean-room audit of `m8-remediation` at `6c71fe66` found that most of §7 is
fixed (B2, N1, N3 for the cited items, N4–N7, N9, N10), and the guard scripts pass. It still
blocks the merge; M8 stays paused (decision 0042 item 4). Line numbers refer to `6c71fe66`.

**Blocking**

- **B4 remains, with new contradictions.** Not settled from §7: fragment header field 8b
  (`fragDataLen` in session 399/408 vs `blobChannel` in world 106, and world 199 lists fields
  the session spec lacks); the 5-byte `0xE0` integer form (world 106/199 vs session 379,
  400–408); the routing header (world 114 vs session §9.1 747–752); 0x179/0x186 as chat-server
  events (play 749/762) vs envelope containers (session §9.4.2 786–802), with no statement
  that the chat connection has its own opcode space. New: the `base_flags` byte (session
  761 vs session 28, world 436/690/701/1359, play 297); opcode reuse in the world §4.4.3
  table (922–947: 0x15, 0x63, 0x76, 0xC7, 0xDC, 0xFE with other layouts than §3.3, §4.3.1,
  §5.6.1, §5.8.1, and framed against E0/E2; 0xFE has three meanings, world 946/1209, play
  554); the RPC envelopes (play §5.2 319–368 vs gameplay §2.4 135–146 for 0x28, 0x5F, 0xC9,
  0x30); `ActionPayload` (play 122/131 vs world 1147/1158; world 1186 vs 1143); 0xCE with mode
  bit 0x10 (world 293/1096 vs play 209/213); 0x24 offset 9 (world 1110 vs play 103); the
  in-game ticket refresh and launch arguments (login §4.3 596–657, 538/542/543/572 vs play
  §6.1.2 444–461); the chat-server connection state (play 618/658/938 vs session 462–463; play
  606 old name and wrong section); the gameplay opcode census (394 for 0xCC, 404–408 for
  0x3000001B–1F vs world and play); handshake header names differ between session, world
  (96, 133–135, 721, 841, 854) and login. Settle each against the install or the binary and
  record it once (transport in the session spec, world events in the world spec, RPC
  envelopes in one spec); say explicitly where opcode spaces are separate. The world,
  session and play specs go back to Review.
- **Client-internal residue:** world 764 (`flag_60`, reads like a structure offset); play
  600–602 (an in-memory flag word `0x00300400`, a source ID and priority that are not wire
  fields); session 632 (an in-memory "arrived encrypted" bit); session 923 (`ListenEndpoint`,
  "6-byte" for an 8-byte range); play 496 (`InterfaceId`, `ServiceType`), play 606
  (`Credentials`), gameplay 3988 (`BoS`) not marked as invented; registration and pipeline
  prose at world 692, 742, 956, 968, 1204, play 270, 599, 651, 716, world 758 and play 236–238
  ("world-view controller"). Reduce to wire behaviour, mark invented names, or state the
  interoperability source.
- **Process evidence (decision 0042 item 7, CLEAN_ROOM.md §8):** writer and reviewer work
  are in one commit with an invalid `Clean-Room-Role`, so "reviewers made no body edits"
  cannot be checked. Commit each pass in two steps: a writer commit (`Clean-Room-Role: spec`,
  specs in Review), then a reviewer commit (`Clean-Room-Role: review`) touching only headers
  and review records. `Assisted-by:` uses the `TOOL:MODEL` form (`19acce5` has none;
  `6c71fe66` has a free-text value).

**Notes**

- **N2 residue:** C-style expressions at session 73, 79, 96; play 257–259, 688; gameplay 61, 96.
- **Unknown meanings without OPEN:** play 106 (`slot_flags` 0x6E), 112 (`trailing_seq`),
  129 (`aux_word_a`), 227 (`set_pos_flags`), 144 (a heading variant with no condition), the
  chat-server `status_a`/`status_b`/`meta_*` fields; gameplay 3864–3866 ("status callback
  1/2/3"); session 225 ("if tolerated" is a guess).
- **Content:** the login test vectors (774–847, 273, 513, 604) put the real service path
  fragments and ports on `.example.invalid` hosts; use neutral paths and ports. Game-string
  enum tables (world 664–670, play 833–834): keep only values the logic needs, say the rest
  is read from the `0x23xxxxxx` records at run time.
- **Decision 0042 item 2:** play 753 (`chat_admin_move_room_req` 0x017D) and play 524 (an
  administrative tell 0x84EC002C) need the never-send / ignore-and-count rule.
- **Headers:** say in each spec's status that acceptance is gated by decision 0042 item 4.
- Minor: `HashTable<…>` notation (gameplay 156–167), the anchor `netblob-event-census`
  (gameplay 178), `0x0465AB10..12` in world 897 breaking the month pattern.

**Next step:** a third pass on `m8-remediation`, with the process of the "Blocking: process"
item, then this audit again.

## 9. Pre-review audit of the third pass, writer commit (2026-10-24, a780a67): NOT READY

The writer commit follows the §8 process (role `spec`, `Assisted-by` in TOOL:MODEL form, signed
off, all six specs in Review; the reviewer commit is still to come). A default-mode audit of
the writer state finds most of §8 fixed (field 8b, routing header, 0x24, ticket refresh and
launch arguments, the chat connection state, handshake names, most residue, the login test
vectors, the admin messages, the headers), but it would block review. Prefixes: S session,
W world, P play, G gameplay, L login; line numbers of `a780a67`.

**Blocking**

- **0x6F has three meanings:** world entry request (W701–707), logout with a u32 reason
  (P207/P705, which calls the world layout canonical but describes another), and portal-exit
  property sync (P19, P34, P157; W849–850 has the client send only 0xF4 after a portal).
- **Contamination sign:** G1604 and G1615–1616 name fields `tail_a8`/`tail_ac` and flags
  `has_tail_a8`/`has_tail_ac`, which read like structure offsets (same bytes and defaults as
  W1073–1074). Delete G §3.3, point to W §5.4, and have a writer check where the names came from.
- **The ref (state-object) header disagrees with itself:** a VariableU32 count (G90, P233) vs one
  byte (P844) vs `u32 ref_count` without `node_flags` (G1895, G1927, G1977, G1994, G2039, G2108,
  G2121).
- **G §3.2 (G1572–1594)** puts a containment block after the entity header in 0x98/0x52; W969–981
  has none.
- **B4 base_flags / RPC envelopes still unsettled:** W154–158 and S761–764 say base_flags is
  always 0x00; G63, G104–105, G115, G135, G1881–1883 and P227 give it a bit 5 that adds an
  entity_id. G115/G134–145 describe an "11/19-byte outer header" and cite W §3.2/§3.2.5
  (character-creation sections); G1877–1885 puts the opcode inside the payload; the G vectors
  1837–1866 and 2096–2127 start with `30`/`CE 00`. All conflict with S §9, W §1.1 and P32.
  Settle base_flags against the binary and keep one envelope definition.

**Also to fix**

- 0xE0 tier shape: W206 accepts first bytes 0xE0..0xFF, S381 rejects 0xE1..0xFF; W208 vs
  S381/S409 on the opcode word size.
- The G309–370 census still lists 0x15C–0x195 and 0x1A2 as primary-connection opcodes (the
  chat space is separate: S790, P407, P533); G403–411 lists 9 opcodes of the 0x3000001B range,
  P21/P685 claim the whole range.
- 0xC7 (W769 vs W1120; field order G171 vs W324); 0xCE mode bit 0x10 reintroduced with the
  opposite meaning at G1616; ActionPayload carrier list W1120 vs P105.
- Other shared-layout mismatches: 0x00E1 count (G173 vs P233); 0x00A6 (G168) vs
  EncodedPropertyTable; the dictionary size 29,219 (G45) vs 27,546 elsewhere; 0x51 (S764 vs
  G137); S764 lists 0x100E, 0x10B5, 0x10B6, 0x10BA, 0x1000201C found nowhere else (G3534 has
  0x1000201C as a property ID); chat authentication ticket raw (P423–431) vs UTF-16 string
  (S487–500), authType values; the dictionary redirect rule at S160 loops on 0x0E010001 (L224
  is right); L223–235 and L590 repeat S §4.1.3 and §6.3–§6.4 under other names, L590 mislabels
  key 0x10000001.
- Session contradictions: S256 receive mask vs the reply header of bit 26 (S247, S717); pakSize
  range S215 vs 0xFF7B at S225/S816; status and error codes that look client-internal (S318,
  S461, S534, S599–609, S721): keep only those on the wire.
- Remaining residue: service type and service IDs at P255 and P415–417 (in no wire layout);
  registration/pipeline prose at W862, W912, W1005; P141, P177, P266, P287, P305, P375, P379,
  P669–673; S156, S492–494, S687; L570, L574, L601–610.
- N2: Rust-like signatures at G1660 and G1721; `==`/`!=` at W952–955 and L224.
- Test vectors: P10.7 is 36 bytes, text says 37 (P852–859); G TV1 is 23 bytes, text says 22
  (G1845); W Vector 8 labels vs tables (W1321–1322 vs W802–822).
- References and tags: W860 cites S §5.3 (ClockSync is §8.1); P699/P751 cite W §5.6.2 for 0xCC;
  P882 cites `BIN-MOV-PARSE` with an offset that disagrees with P143 ("31-byte" for a variable
  layout); G2164–2168 cite `BIN-CLIENT`/`BIN-WLIB` not in the legend; L936 cites §8 labels that
  do not exist.
- Facts missing without an OPEN: W328 ("u16 (or u8)"), W117 (zlib threshold), W410–414, P183–184,
  P551 (two 0x16A variants, no selector), P529 ("mode word").
- Decision 0042 item 2: 0x30000045 is called "administrative" (P374, P301, P716) with no rule.
- Enum label tables still at W830–843, W988–1000, W1183–1199, P605–646 (keep only values the
  logic needs). Labelling policy differs: W37 (labels invented) vs G35 (names observed in the
  data); state one rule. W903 (month tokens) contradicts itself and adds a guess.
- Decision 0041 on the branch: add the pass-3 writers to the exposure list and record the §8
  BLOCK; the branch lacks decision 0042 and §7–§9 of this brief, so the specs' links to 0042 are
  dead there: **merge main into `m8-remediation` before the reviewer pass.**

**Next step:** writer fixes for the above (one canonical definition per wire fact; delete the
duplicate envelope, 0xCE and ref-header layouts in the gameplay spec), commit, merge main, then
the independent reviewer commit, then this audit again.

## 10. Final audit of the third pass (2026-10-24, ed9b5b9): BLOCK

The process was followed: a writer commit `3e6a06e` (role `spec`), main merged in (`51583e9`),
and a header-only reviewer commit `ed9b5b9` (role `review`). Most of §9 is fixed: 0x6F, the
ref header, `tail_a8`/`tail_ac`, the 0xE0 tier, the chat opcodes, 0xC7, the ActionPayload
carriers, the dictionary size and redirect, the receive mask, pakSize, the error codes,
the service IDs, the test vectors, the missing OPENs, the enum tables of the world and play
specs and the labelling policy. It still blocks. Line numbers refer to `ed9b5b9`.

**Blocking**

- **New contamination (CLEAN_ROOM.md §5.3 item 2, §10):** `3e6a06e` added client-internal C++
  class and method names to the gameplay spec (G1558–1560, 1563, 1569, 1615, 1616, 1622, 1625,
  1911–1913, 2194, 2197, 2496, 2558), script-bytecode internals written as pseudo-code (G60,
  G62), shift-and-mask expressions (G58), "bytecode local slot" (G102) and VM lifecycle
  primitives (G80–82). None of it was in `a780a67`; reviewer `116b34f7` accepted it. Decision
  0041 on the branch (116–119) and the `3e6a06e` commit message state client in-memory
  structure offsets. Apply §10: hold the gameplay text, void its acceptance, add writer
  `f0a991a9`, reviewer `116b34f7` and the auditing session to the exposure list.
- **Contradictions, almost all involving the gameplay spec:** the property-update opcodes and
  type codes (G1577–1591 vs W1136–1152 and W355–378; G1761); script class 0x051E (logout in
  P242, P606–616; quickslot/targeting in G404, G725–732, G1204–1214, G1754; logout in class
  0x0510 at G394); 0xAA (W749 vs P368); the 0x00D3 class (G169 vs W §2.5.1); the 0x5F trailing
  field (G135 vs P686); the gameplay census lacks 0x0D, 0x24, 0xF4 and gives 0xDC the wrong
  direction (G280 vs W597).
- **Process records:** the writer sessions named in `a780a67`'s message and the change logs
  (7aa0951a, c3312844, 5ae75fbf) differ from decision 0041's (cc6fd3c4, dd9e328f, a4d01e0f,
  which are also missing from its exposure list); G4213 cites a review session recorded
  nowhere; `3e6a06e` names no writers.

**Decision ([0042](decisions/0042-m8-remediation-decisions.md) items 8–9, owner-delegated):** M8
is built in stages. **Next pass:**
1. Move `official-messages-gameplay.md` back out of `docs/specs/` (held), and void its
   acceptance.
2. Finish the five core specs (login, session, world, play, property-system §10) consistent
   with each other, with no link that needs the gameplay spec. Remaining core items from this
   audit:
   - 0xAA (W749 vs P368);
   - login §2.4.3 (L197–216) still has its own 0x28000010 layout and names (L572, L740);
   - the 0xBF size (P131, P825 give 5–53; its fields add up to 6–57);
   - client-structure prose at S922–923 and S246–251/706;
   - 0x30000045 is still listed among the logout triggers (P21, P628);
   - the 0x3000001B-band lists in the play spec (P21/P628/P663/P817) cite a P §6.5.2 that
     does not exist;
   - L900 cites §8 labels that do not exist;
   - the notes on S96, S107, S225, S487–492, L571/L581/L632, S770, L187, the schema label,
     P712, P537, W1060 and P105;
   - change-log rows must quote no removed internal values (W1310, P847, L901);
   - mark the world PID tables and calendar values as verification values that the code
     computes at run time.
3. Rebuild the branch so that its merged history carries no contaminated text (item 9). For
   example, branch fresh from main and commit only the cleaned core specs, as a writer commit
   and then a header-only reviewer commit by new sessions. One consistent table of every writer
   and reviewer session goes in decision 0041, with a complete exposure list.
4. Then this audit again, for the core stage only.

## 11. Audit of stage 1, the core specs (2026-10-25, 9b31155): BLOCK, close

The rebuilt branch follows decision 0042 items 8–9: fresh from main, a writer commit `5470c8f`
(role `spec`) and a header-only reviewer commit `9b31155` (role `review`), the gameplay spec
held, no link to it, no earlier pass in its history. **No contamination was found** (specs,
decision 0041, both commit messages). Most §10 core items are fixed (0xAA, login's own
0x28000010 layout, the 0xBF size, 0x30000045, the band citation, L900, the change-log rows,
the verification-value markings). It still blocks on consistency; line numbers refer to
`9b31155` (L login, S session, W world, P play, PS property-system, D decision 0041).

**Blocking**

- **B1 Chat server address:** S90 says it arrives "via an in-band game message" that no spec
  defines; L514, P270, P289–290 and P425 take it from `--chatserver`, falling back to `--host`.
  Link S90 to L §4.1 (or specify the message).
- **B2 Launch parameters:** S69, S72, S76 (`localaddress`/`localport`) and S87
  (`serveraddress`/`serverport`, a bracketed host:host:port form) cite the login spec, which
  defines only `-h`, `-p` and `--outport` (L503–527, L549–552).
- **B3 Ticket and auth mode:** S490, S497, P434, P721 fix the ticket blob at 108 bytes from a
  144-character ticket; L385 says about 180–256 characters. Make `blobLen` the decoded length.
  S490–492 do not say which `authType` the primary connection uses: give one selection rule.
- **B4 `checkValue`:** in `ConnectAcceptHeader`, `checkValue` (S527) is "computed from the
  connection's time/state" with no derivation and no OPEN; the handshake cannot be built. Give
  the derivation, or an OPEN with a clean-room fallback.
- **B5 Counts:** the band has "18 opcodes" (P21, P645, P649, P839, P871) but the listed ranges
  hold 20; `ActionPayload` maximum 52 bytes (W82, W1106, P105, D184, D197) adds up to 56 (each
  VariableU32 can be 5 bytes); S312: 0x1FBC4 is 129,988, not 130,004.

**Also fix in the same pass**

- Session prose: the "inbound optional-header parser mask 0x093FFB00" and the low-byte flag
  names (S246–253, S887), dispatcher/handler wording (S686, S699); S900 quotes the mask in a
  change-log row; W1321 and W1324 (and W889) quote a client pre-load fallback value.
- S96 `localRecId` range vs S99/S466 (0xFEFF); S107/S215/S225 `pakSize` vs the 65,404-byte
  datagram; `pos_sync_flags` (P716, P816) vs `sync_flags` (W778); `interp_param_a`/`_b`
  (W1058–1059) defaults without meaning or OPEN; W1070 says the 0x24 stamp is echoed from 0xCC,
  0xCE or 0xBF but only 0xCE carries one (P147–157); `sub_entry_flag` vs `sub_flag` (P702);
  vestigial anchors at P223; the CompactPID reader on first bytes 0x81–0xBF (W275–279 vs PS
  §10.2: both reject); state that the UTF-16 length prefixes differ (S478, S733–734 VariableU32;
  W229 3-tier); OPENs or receiver rules for the 3DES key source (S603), the 0xBF `state_flags`
  (P136), 0x9F (W1158), the bare opcode lists (W706, W756) and 0xED with an unknown controller
  GUID (P183–184); how code finds the world PIDs at run time (labels invented, .dat names not
  given); provenance wording about client internals (P46, P49–51).
- Content: W711 quotes a game UI label with a Tolkien mark; L714 test tokens should be neutral
  synthetic values.
- Decision 0041: D117 cites a commit `9b4081e` that does not exist (§8 records `6c71fe66`); the
  exposure list (D46–50) lacks the §7, §8 and §9 auditors (who read the gameplay spec), names
  the `ed9b5b9` auditor without a session ID, and lacks the pass-4 parent session.

**Next step:** one writer commit for the above on `m8-remediation`, then a header-only reviewer
commit by new sessions, then this audit again (stage 1 only).

## 12. Audit of stage 1 after pass 5 (2026-10-25, 2e30790): BLOCK, narrow

No contamination. Every §11 blocking item is fixed (the chat server address, the launch
parameters, the ticket and `authType` rule, `checkValue` now an OPEN with a fallback, the
counts), and nearly all of the "also fix" list. Three small points remain; line numbers refer
to `2e30790` (S session, W world, P play, PS property-system, D decision 0041).

**Blocking**

1. **CompactPID:** W274 and W279 reject first bytes 0x81..0xFF "matching" PS §10.2, but PS800
   (§10.2) still reads any first byte other than 0x80 as the 2-byte form. Add the rejection rule
   to PS §10.2; a new reviewer re-accepts PS §10.
2. **Earlier review rows were rewritten:** the pass-5 writer changed the change-log rows of
   earlier writers and reviewers (S908–912; W1324–1328; P870–872), so they now say those passes
   checked 20 opcodes, 56 bytes, 0xFEFF and "0 optional header bytes", which is not what they
   accepted. Restore those rows as written at `9b31155`, replacing only the quoted internal
   values with a marked redaction (for example "[value removed under §11]").
3. **Exposure list (D44–95):** add the session IDs of the pass-4 and pass-5 orchestrating
   (parent) sessions, the §7–§10 auditors (now named only by audited commit), and the
   `ed9b5b9` auditor; where an ID was not recorded, say so plainly.

**Notes to fix in the same pass**

- Citations: P434 and P721 cite a session "§10.1 synthetic test vector" that does not exist;
  W1069 cites play §4.4.4, which does not exist.
- P147 (§4.5 step 2): say which message supplies `expected_stamp` (0xCE, per W1069).
- L385, L509, L554, L561: the ⌊L×3/4⌋ formula is wrong with padding; say "the decoded length".
- The `<localHost:remoteHost:port>` form (login and session): define it (brackets literal or
  not, how it differs from `host:port`), or replace it with an OPEN.
- Sender rules for the 0x24 `trailer_u32` and the OPEN `slot_flags` bits (W1081).
- `LoginQueueStatusMessage`: optional at L185, but S741 and W §1.1.4 put it before 0x6C without
  saying it may be absent.
- Decision 0041: D147 claims every §11 item resolved; D174 says §6.1 (the tokens are in §7.1);
  D182 calls 0x1FBC4 a byte count (it is a packet count); add a BLOCK bullet for pass 4.
- Optional: the unreferenced anchors `s5-2-1-msg-0030` and the P234 aliases.

**Carry to the build:** `OPEN-SESSION-5` (`checkValue`) has a guessed fallback; synthetic tests
can pass while a live handshake fails (M8 exit criterion 4). Pass 4 renumbered session §7 and §8,
so the `crates/compat` conformance check (0042 item 6) re-maps code citations.

**Merge (decision 0042 item 9, owner-delegated):** when stage 1 passes, it is merged into main as
**one squashed commit** of the final text, so the intermediate commits (`5470c8f` and `9b31155`
carry a client parser mask, low-byte flag names and a pre-load fallback value that the final
text no longer has) do not reach main's history.

**Next step:** one writer commit for the three blocking points and the notes, then a header-only
reviewer commit by new sessions (re-accepting PS §10), then a check of that diff.

## 13. Check of pass 6 (2026-10-25, 2ebe9bc + 3f98b17): BLOCK, one sentence

No contamination. All three §12 blocking points are fixed (the CompactPID rejection in PS §10.2;
the earlier change-log rows restored exactly, with only the internal values redacted; the
exposure list complete), and nearly all §12 notes. Line numbers refer to `3f98b17`.

**Blocking**

1. **Teleport stamp contradiction:** play P150 (§4.5 step 2) takes `expected_stamp` from the
   0xCE `seq_byte` when bit 3 is **clear**, but world W1047 (§5.4 row 5) and play P152 say
   `seq_byte` is present only when `header_flags` bit 3 is **set**. P150 also mixes up
   `mode_flags` bit 3 (extended attachment) with `header_flags` bit 3. Restate step 2 using
   `header_flags` bit 3 set, matching W §5.4; a new reviewer re-accepts the play spec.

**Notes to fix in the same pass**

- `checkValue` (S526, S531): the new derivation cites `BIN-03`, whose provenance row (S44)
  covers byte layouts only, and pass 5 described a different source; add the observation that
  supports the derivation to the provenance table. Name the §6.3 fields that bytes +0x08..+0x0F
  are (`assignedClientRecId`, `serverRecId`, `outgoingSeed`), and add a synthetic handshake
  test vector to §10.2.
- `slot_flags` (W1072, W1082, W1307): the sender and receiver rules mask 0xEE, but the OPEN still
  names 0x6E, so bit 7 is neither defined nor OPEN; extend the OPEN to 0xEE.
- W1047 still cites play §4.4.4, which does not exist.
- L385, L509, L554, L561: "135..192" bytes for 180..256 characters ignores padding; say "up to".
- Optional: say whether the CompactPID rejection (PS97, PS800–802) is the client's behaviour or
  a Tessera hostile-input rule.

**Commit form:** `2ebe9bc` and `3f98b17` have no `Signed-off-by` (check-dco fails) and use
`AI-Tool:` instead of `Assisted-by: <tool>:<model>`; their session lines sit in separate
paragraphs, so git does not read them as trailers. Use `git commit -s` and a single final trailer
paragraph, as in `2e30790`. (The squashed merge to main carries correct trailers either way.)

**Carry to the build:** `crates/compat` computes `check_value` from server time and the local
connection ID, which contradicts S531, so the 0042 item 6 conformance check changes it; the rule
is confirmed live only under exit criterion 4.

**Next step:** one writer commit for the blocking point and the notes, then a header-only
reviewer commit by new sessions, then a check of that diff.

## 14. Check of pass 7 (2026-10-25, 478e6e4 + 61eb7f6): PASS WITH NOTES; stage 1 merged

No contamination. The §13 blocking point (the teleport stamp now follows `header_flags` bit 3
set, in world §5.4 and play §4.5 alike) and every §13 note are fixed. The new handshake test
vector (session §10.2, Vector 5) is synthetic and was recomputed by hand. Both pass-7 commits
are signed off with correct trailers. Stage 1 was merged into main as one squashed commit
(`21f9596`, decision 0042 item 9); incident 0041 closes for the core stage, and stays open for
stage 2 (the held gameplay spec).

**Non-blocking findings for a later spec pass**

- W278 and the W1334 change-log row cite property-system §7.1, which is about enum values; the
  intended section is probably world §7.1.
- Play §4.5 steps 2 and 3 set and clear the wait flag from the same 0xCE message, and no other
  trigger of a portal-space transition is named; add one, or an OPEN.
- Session S44 speaks of a "handler" and a "serializer"; reword to behaviour only.
- W278, PS97, PS800, PS802 describe the client's branch test; state the client's behaviour
  instead (it reads first bytes 0x81..0xFF in the 2-byte form), beside the Tessera rule.
- Vector 5 sets `localRecId` to `assignedClientRecId`, but session §6.4 step 1 does not say that
  the client adopts the assigned ID, or what happens when it is 0; add the rule.
- The "up to 135..192 (133..192)" wording would read better as: decoded bytes = 3L/4 minus the
  padding (0..2).

**Carried into the build**

- `crates/compat` computes `check_value` from server time and the local ID, against session
  S531; the conformance package (0042 item 6) replaces it, with Vector 5 as its unit test.
- The CompactPID rejection is a Tessera hostile-input rule, logged when it fires.
- The teleport stamp follows world §5.4 (`header_flags` bit 3), not `mode_flags`.
- A sender leaves the `slot_flags` bits under mask 0xEE at 0; a receiver ignores them.
- Session §7 and §8 were renumbered after the code was written: re-map code citations.
- Exit criterion 4 (owner live verification) confirms `checkValue`, the stamp rules and the
  0x81..0xFF case.

**Next for the other AI:** stage 2 (the gameplay spec held in `research/held/incident-0041/`),
under the same rules: a writer commit, a header-only reviewer commit by new sessions, signed off
with one trailer paragraph.

## 15. Spec work found by the stage-1 build (2026-10-25)

For the next spec pass on the core specs (same rules: a writer commit, then a header-only
reviewer commit by new sessions). From the `m8-play` build and its audit:

- **Play §5.1 vs §6.3–§7.1:** §5.1 says RPC signatures are read from the install at run time,
  but §6.3–§7.1 give them in full. Reconcile per decision 0042 item 10, and record how a
  method's signature is read from the install, or an OPEN for it.
- **Play §4.5 step 2:** which server event starts a portal-space wait, and where
  `expected_stamp` comes from before the 0xCE that step 3 handles.
- **Play §4.2 and §8:** the mapping from a neutral move command (position, heading, velocity) to
  0x24's `move_state` and three input axes: the `move_state` values and the axis scale. Also
  what counts as "moving" for the 20 Hz cadence (§4.2 item 1).
- **Play §4.3.2:** correction tolerances and how many unacknowledged steps to keep.
- **Play §4.4.1 / world §2.5.2:** the reference and direction of the 1-byte heading (the code
  reads it as the `coordinates.md` §11.3 yaw, Tessera heading = wire + π/2).
- **Play §4.5 step 4:** the portal-space loading timeout.
- **Play §4.6.1:** `path_type` meanings, `speed_param` units, interpolation along 0x59 paths.
- **Play §6.7:** the chat category numbers of Yell and Fellowship, and which install enum labels
  they come from.
- **Play §6.6.1–§6.6.5:** the chat server's delivery queue and order numbering; the wait for
  0x165; when the initial delay and when the retry interval applies; the 0x16A `msg_flags`,
  `delivery_flag` and the local sender descriptor; the room ID when joining a user room by
  name; whether `/join` sends 0x16E or 0x177 (or both, in which order) and with what flags.
- **Play §6.6.4:** the 0x08040000 note names `chat_mode_word`, which that message type does not
  contain.
- **Play §6.5.1:** properties typed "Prefixed UTF-16LE string", which no property type encodes.
- **Play §6.3.2:** the meaning of 0x84EC001A's `status_code`.
- **Play §8:** the mapping from a neutral emote command to 0x84F90007 / 0x84F90005.

From the `m8-conform` build (session S, login L, world W):

- **S §6.3 / §6.4 step 1:** name `assignedClientRecId` in the step (Vector 5 implies the client
  adopts it) and say what happens with a non-zero value outside 0x0100..=0xFEFF.
- **S §4.2:** the iteration and table values the client writes in its `DatagramHeader`.
- **S §6.2:** the meaning of `authFlags` bits 3..11.
- **S §4.4.2:** the send side computes the payload checksum per piece, the receive side over the
  whole payload; make them agree.
- **L V4/V6:** the synthetic ticket is not valid Base64, so the strict decoder that L §4.2 and
  S §6.2.1 require refuses it; use a valid synthetic ticket.
- **L §2.4.3 and S §4.1.3** name the dictionary-map DID explicitly, while property-system §8.1
  and records §8.2 forbid hard-coding map DIDs; say it is found through the root map (key 3).
- **L netver limit:** the spec says 24 characters; confirm (the code allowed 64).
- **L change log:** a reviewer row records the install's build number and the binary's SHA-256;
  move such identifiers out of the spec or say why they are needed.
- **W §1.1.2:** which entity is the target when a routing header is present.
- **W §4.4.2:** the `world_day_type` rollover rule and what "updated in steps of 0.01" means.
- **W §5:** whether the 0x0D tail flag appears when the batch is nested; the policy for undefined
  header, mode and action flag bits; a bound on container record counts (§9.4).
- **Blocking OPENs for the core build:** the `EntityAppearanceDesc` bit-0 `model_b` gate means no
  0x6C entry with a base model can be decoded, and the missing wire argument key of
  `LocalizedTokenText` blocks every text with arguments.
- **property-system.md §8.1 vs S §4.1.3:** §8.1 says root-map labels are looked up by name, §4.1.3
  by numeric key 3; reconcile.

From the `m8-play` fix round:

- **Play §4.3.2:** 0xCC `sequence_or_reason` carries either an `ack_step_seq` or a teleport
  reason (world §4.3.1); how a receiver tells them apart, and how to treat an ack ahead of the
  newest step sent (the code refuses and counts it).
- **Play §7.2 item 2:** what counts as "moves/acts" for cancelling a logout countdown, and
  whether the cancel is resent while waiting for 0x851E0003.
- **Play §6.6.3:** whether a reconnect disconnects the old chat session gracefully and waits for
  its flush, and whether each attempt uses a new socket (the code does both).

From the `m8-conform` fix round:

- **S §3.2 Case B:** for k near 11, `(ip mod 0xFA07) + 256 k` can exceed 0xFFFF: is the candidate
  skipped, truncated, or does the loop stop (the code skips it)? A Case B test vector.
- **W §4.4.2:** how the client finds directory 0x0F000002 (key 0x96) at run time: which root-map
  or sub-map key leads to it.
- **W §5.4:** whether `mode_flags` can carry 0x08 without 0x04, or 0x10 with 0x04, and what a
  receiver does then (the code refuses both).
- **W §2.5.1:** whether a base Position (no quaternion) can carry flag bit 0x20, and whether it is
  kept.
- **S §9.4.2:** whether bytes after the M records of 0x186, or after `resultCode` of 0x179, are an
  error (the code ignores the first and refuses the second).
- **S §3.4:** whether a server may give a new connection a recipient ID that an earlier,
  removed connection used.

From the `m8-connector` build (entry E = `official-entry.md`):

- **L §5.1 vs E §8.3:** §5.1 orders the world choice before `LoginAccount`, while E §8.3's progress
  texts imply authenticating first; the connector authenticates first and keeps the ticket, so a
  later world choice needs no password. Confirm the order.
- **L §2.4.1 step 3:** Tessera has no prompt for a missing netver value; it ends in an error.
- **L §3.5:** the poll interval when `PollTimer` is absent or extreme (the code clamps to 5-120 s,
  5 s when absent).
- **S §9.3:** whether 0x12C is sent only after 0x12F or proactively; the `logonResult` values of
  0x130.
- **S §5.6.2:** which delivery queue 0x12C, 0x6F, 0x24 and the RPCs use (the code uses 2).
- **S §6.5:** whether the client closes the login connection after the referral.
- **P §8:** play maps the world-teardown opcodes and the later 0x6C to `Event::Connected`, but a
  `tessera-net` session refuses `Connected` outside `Connecting`, so returning from the world to
  character select cannot be expressed; also the neutral `Command::Move` (location, velocity)
  vs 0x24's input axes and `move_state`.
- **P §4.5:** no neutral "loading done" signal exists, so the connector ends portal space as soon
  as the server stamp allows.
- **E §8.1 / §8.6:** the world-list status mapping and the refusal mapping are implementer
  choices (recorded in the code); confirm or specify.
- **E §8.7 vs W §6 and §3.2.5:** the character screens' create and delete need install enum
  mappers that no spec assigns to the connector or the client.
- **E §4.4:** what "Available" means once a live connector exists but the §8.7 hand-over is not
  built.
- **E §8.6:** a category for an install that cannot be used (F02), whose target is the start
  screen.
- **L §6.2:** whether a password or ticket may go to an HTTPS URL learned over plain HTTP (the
  code refuses).
- **S §3.2:** whether the `UserPreferences.ini` bind keys must be read, and the bind rules for the
  chat socket.
- **E OE-T8 (b):** name `ureq` in the manifest clause as the one HTTPS dependency of the
  network-access file.
- **L §3.5:** whether `LeaveQueue` is sent when the user cancels the login while queued (the code
  sends nothing after a cancel).
- **L §3:** which `User-Agent` the exchanges carry and whether a system proxy is used (the code
  sends a neutral value and uses no proxy).
- **W §3.6:** whether a `WorldReferralHeader` is valid outside world entry, and how long the client
  waits between 0x6F and 0x90 (the code ignores it outside entry or the world and fails the entry
  after 60 s).
- **S §8.2:** how long the client waits for the disconnect flush at logout (the code stops after
  5 s).
- **P §6.6.3:** the delay after a chat attempt that could not bind (the code waits 30 s).
- **E §8.5:** a setting for the network version when the install gives none (`[Official]` allows
  only `last_world`).
- **S §3.4 step 2:** a datagram naming a connection's ID from another address: discarded, or does
  step 3 apply? May a world server assign a client ID the login connection holds? (The code
  discards, except while a connection from that address waits for its challenge.)
- **W §3.6 / §3.4:** whether 0xB3 ends the session in the login and in-world stages too (the code
  does), and the world-entry deadline (the code fails the entry after 60 s).
- **E §7.3:** extend "no credentials in logs" to install paths and other user paths (the code logs
  kinds only).

From the `m8-hand-over` build (C3):

- **P §4.5 item 4:** the length of the portal-space loading timeout (the code uses 30 s).
- **P §7.2 item 3:** answered by the item itself ("upon receiving this notice ... returns to
  Character Selection"); the code now returns at the notice with the list it has.
- **P §7.2 / W §3.5:** which connection carries 0x6F after a return from the world (the code uses
  the one that delivered the latest 0x6C).
- **P §6.1.3 / §7.2:** whether the chat-server session stays up or is re-opened (with the new
  character) across a return to character selection.
- **P §8:** maps the logout request to `Command::Disconnect`, while E §8.2 makes `Disconnect` end
  the session; Tessera adds a neutral `Command::LeaveWorld` and `Event::ReturnedToCharacterSelect`.
- **W §6:** names `CharacterSummary` fields (`public_props`, `visual_desc`) the neutral type does
  not have.
- **E §8.7 (rev 6):** still describes only the placeholder; it needs a revision for the hand-over.
- **P §7.2 item 3 / §8:** whether the client returns to character selection on the teardown
  notice itself, with the list it has (Tessera's reading), or only on the following 0x6C; whether
  a later 0x6C at character selection is a full replacement (surfaced as removed/added rows).
- **P §4.5 items 2 and 4:** what starts portal space for a teleport inside the world.
- **E §8.7 (rev 6):** for the hand-over: which options the character-screen client keeps
  (Tessera: an allowlist), the message when the handed part ends (§8.6 rows), and that the
  session never enters a character without the player's choice.
- **E §4.2:** whether a typed `/logout` counts as device input and is the only allowed source of
  the leave-world request (start-up lines refused).
- **P §8:** a later 0x6C at character selection is surfaced as removed and added rows
  (`CharacterRemoved` / `CharacterInfo`); §8 maps a later list only to `Connected` or
  `CharacterInfo`.
- **E §8.6 / §8.7:** which text shows when the handed part ends because the client itself cannot
  start the character screens or the walker (not a service reply).
- **P §7.2 / §8:** whether a chat line refused after the teardown, before the client sees the
  return, is reported to the player, and with which event and text (the code sends a server
  notice with Tessera's text; refused moves stay silent).

From the `m8-world-feed` build (C4):

- **W §5.2.3:** which field of 0x98 / 0x52 carries the classification enums named there; confirm
  the rule (NPC and object flags over the template baseline), how NPC and creature differ, and
  what to show when neither flag is known. The section names enums no wire field carries.
- **W §1.1.3 / §5.1:** whether entity-scoped sub-events after a 0x98 in a batch sent to the
  default recipient apply to the entity that 0x98 created (the code redirects only a 0x52 directly
  after the 0x98, per §5.2.2; everything else keeps the batch's target).
- **W §6:** 0x98 + 0x52 maps to an `EntitySpawned` with fields the neutral model does not have;
  which event, and when, if no position has arrived yet (the code creates the entity at its first
  position).
- **W §5.2.4 / §6:** whether the template baseline is merged per PID and whether 0x0E / 0xE7 fall
  back to it (the code does both).
- **W §2.8 / §5.7:** which descriptor fields map to the neutral appearance (only a uniform scale is
  mapped).
- **W §5.3:** how often 0xD0 may be sent (the code: once per entity per world).
- **W §4.3.1 steps 1-2:** the visit order of a Scriptlet node and its port children, and whether
  definitions in included tables count (the code: yes to both; matches the verification tables).
- **W §4.4.2 vs environment.md §3:** directory 0x0F000002 is region 3's descriptor, whose maps hold
  only key 0, so the calendar lookup cannot work as written.
- **W §2.6 vs string-table.md §9.0 step 1:** mode 2 is a table reference in one and a source-text
  placeholder in the other.
- **P §4.6.2 / §4.6.3 / §8:** the neutral event for 0xED (§8 says `EntityMoved`, but 0xED has no
  position) and 0xBD.
- **P §6.7:** which room a world-server chat line in a user-slot category belongs to.
- **W §4.3.1 step 1:** the code now starts the schema closure at 0x07000417 plus 0x07000000 and
  follows script-table §8.2, as the spec says (answered; kept for the record).
- **W §5.2.2 / §1.1.3:** when a 0x0D addressed to entity X carries a 0x98 for entity Y followed by
  a 0x52, does the 0x52 apply to X or to Y (the code keeps X); may null frames sit between the
  0x98 and the 0x52 (the code: no).
- **W §4.3.1:** the table lists three schemas, but four script keys of 0x07000029 are keys of the
  script-key enum 0x23000097 on the reference install: is every such key a `state_schema_id`?
- **W §2.6 / string-table §9.0 step 1:** whether a literal text received on the wire is shown in
  display mode (display cut, then escapes; the code: yes).
- **W §5.1:** whether a 0x0D carried inside another batch ends with the tail flag (the spec covers
  only a top-level 0x0D; the code accepts both and counts which).
