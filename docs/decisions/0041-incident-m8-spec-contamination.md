<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0041 — Incident: disassembly prose in the M8 gameplay spec, M8 specs without independent review

**Status:** Closed for the Stage 1 core specs on 2026-10-25: the default-mode audit of pass 7 (`61eb7f6`) passed with notes ([`m8-remediation.md`](../m8-remediation.md) §14) and the five core specs reached `main` as one squashed commit (`21f9596`, [decision 0042](0042-m8-remediation-decisions.md) item 9). Open for Stage 2 (the gameplay spec held in `research/held/incident-0041/`). Recorded under `docs/CLEAN_ROOM.md` §10; passes 1–7 are recorded below.

## Context

On 2026-10-22 the owner's uncommitted M8 work was committed to `main` as found:
- `a6b1d2d`: the governance and process rewrite;
- `ce1267a`: the five official-protocol specs, the `property-system.md` §10 additions and
  the M8 research notes;
- `c080a54`: `crates/compat`, three fuzz targets and the wiring.

A decompiler-output file lay in the repository root, a clean-zone path. It was moved
unread to the dirty scratch area and was never committed.

A clean-room audit then ran on branch `m8-protocols`. It was a default-mode
`clean-room-auditor` session that opened no dirty material. Ten spec-conformance reviewers
ran beside it.

## Findings

1. **Disassembly in a spec (blocker).** Parts of §2 of `official-messages-gameplay.md`
   describe disassembly in prose: registers, stack offsets, vtable slots and internal
   function names. Another part describes client-internal memory layouts. This breaks
   `CLEAN_ROOM.md` §5.1 item 6, §5.3 item 2 and §9 item 2. The material sat in
   `docs/specs/`, which clean sessions read.
2. **No independent review.** The review records of `official-messages-world.md`,
   `official-messages-play.md`, `property-system.md` §10 and the gameplay spec name the
   orchestrating session, or an unnamed "clean-room reviewer", as reviewer. Nothing shows
   that this session was separate from the writers. That does not meet §5.3.
3. **Revised after acceptance.** `official-login.md` and `official-session.md` changed
   after their 2026-10-20 reviews. Under §5.2 they are Revised, not Accepted.
4. **Internal names and layouts.** `official-session.md` and `official-login.md` contain
   internal class names and memory layouts. `official-login.md` §2.4.1 describes reading
   the version from instruction patterns in the client binary, and `login.rs` implements
   that scan.
5. **Code provenance not recorded.** No record shows which session wrote `crates/compat`,
   or that its writer was clean. The audit found no contamination signs in the code: its
   names describe the wire, no client-internal identifiers are used as code, and its
   constants trace to the specs.

## Exposure

- **Pass-9 independent reviewer (2026-10-30):** Codex / GPT-6, task `/root/pass9_review_coordinator/review_session_login`; session UUID unavailable (host does not expose it). Reviewed login and session at writer commit `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`, PASS. Fresh reviewer started with no inherited history by coordinator `/root/pass9_review_coordinator`, which wrote none of this pass. Dirty for M8; never implement M8.
- **Pass-9 independent reviewer (2026-10-30):** Codex / GPT-6, task `/root/pass9_review_coordinator/review_world_play`; session UUID unavailable (host does not expose it). Reviewed world and play at writer commit `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`, PASS. Fresh reviewer started with no inherited history by coordinator `/root/pass9_review_coordinator`, which wrote none of this pass. Dirty for M8; never implement M8.
- **Pass-9 independent reviewer (2026-10-30):** Codex / GPT-6, task `/root/pass9_review_coordinator/review_property`; session UUID unavailable (host does not expose it). Reviewed property-system §10 at writer commit `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`, PASS. Fresh reviewer started with no inherited history by coordinator `/root/pass9_review_coordinator`, which wrote none of this pass. Dirty for M8; never implement M8.
- **Pass-9 review coordinator (2026-10-30):** Codex / GPT-6, task `/root/pass9_review_coordinator`; session UUID unavailable (host does not expose it). Wrote none of this pass and started the three fresh independent reviewers with no inherited history. Coordinates their results; claims no independent spec acceptance of its own. Dirty for M8; never implement M8.

- **Archived pass-8 exposure (2026-10-30; authorship/exposure only):** The imported
  Stage 1 spec text was written through Antigravity by Gemini (exact model not recorded),
  writer/parent session `c0194bf2-43a9-4f65-a6c2-3e421509c2f7`. The archived pass-8
  record also lists dirty-room research sessions `54cfa179-c929-4e89-b574-3a7a5e9e47a2`,
  `3d301227-2114-4e4b-b247-9f458f8e8a76`, and `ea61b9d8-afeb-40ff-8bcc-203a513284cb`;
  historical reviewer sessions `f9eb187f-2699-4b27-95a2-bdb310ee2dea` (login),
  `648588d9-ee15-400c-a88e-9e7a3b646959` (session),
  `c167ceb9-eca5-44a2-8d47-aec372ff1f9f` (world),
  `fd299a75-1f58-4bfb-88ea-ffb3b93b2001` (play), and
  `91c29719-a154-45b6-84ff-5c7f7129dd11` (property §10). Source: the archived
  pass-8 decision record at `b0d3415`, Exposure. All are dirty for M8 and must never
  implement M8. This entry records no review outcome and assigns none of them to pass 9.
- **Pass-9 continuation writer (2026-10-30):** Same Codex / GPT-6 writer conversation
  as `73c0571`, task `/root`; session ID **not recorded** (host does not expose it).
  Read origin/main remediation §17 and the Stage 1 specs and neutral records; wrote the
  §17 corrections. Remains dirty for M8; no implementation work.


- **Pass 9 writer (2026-10-30):** Codex (GPT-6), one writer session, session ID **not recorded** (not exposed by the host), task path `/root`. Read the five Stage 1 specs, binding neutral records and dirty-room notes; dirty for M8 and never eligible to implement M8. No subagents were started by this writer. No held incident-0041 text was opened.


- **Who read the gameplay spec:**
  - In initial audit workflow `wf_0939180e-4d4` (2026-10-22, recorded on `main` in commit
    `ec28767` under clean-room parent orchestrating session
    `session_016AYk1rTJPsWsU2hLiLsh19`), the default-mode `clean-room-auditor`, the four
    gameplay gap reviewers, their verifiers, and the planner read
    `official-messages-gameplay.md` (their individual subagent session IDs were not recorded
    in the repository).
  - In the `docs/m8-remediation.md` §7–§10 audits of passes 1–3 (before
    `official-messages-gameplay.md` was held again for Stage 2), the default-mode clean-room
    auditing subagents spawned under clean-room parent orchestrating session
    `session_016AYk1rTJPsWsU2hLiLsh19` read `official-messages-gameplay.md`:
    - the §7 auditor of Pass 1 commit `19acce5` (recorded in `main` commit `a6e47bb`;
      individual auditor subagent session ID was not recorded in the repository);
    - the §8 auditor of Pass 2 commit `6c71fe66` (recorded in `main` commit `c5b4527`;
      individual auditor subagent session ID was not recorded in the repository);
    - the §9 auditor of Pass 3a writer commit `a780a67` (recorded in `main` commit `7f72d1c`;
      individual auditor subagent session ID was not recorded in the repository); and
    - the §10 auditor of Pass 3b writer commit `3e6a06e` and Pass 3b reviewer commit
      `ed9b5b9` (recorded in `main` commit `0f510d4`; individual auditor subagent session ID
      was not recorded in the repository).
  - On the dirty-room remediation side, the single remediation orchestrating (parent) session
    `fa9950b7-7f4d-4d9f-b0bf-2c191d044cd8` (which served as the parent orchestrating session
    across all remediation passes: Passes 1–3 and Stage 1 core Passes 4–7) and the pass 1–3
    gameplay writer and reviewer sessions (`95f8fd58-e269-427f-8ff4-ca955aa5cdcc`,
    `e74ae85e-d157-4eed-ae01-e184492985e0`, `7aac837b-7c9f-4a3c-a67f-853951f73909`,
    `74ca1ac7-4228-4cce-96af-b39f41f8ba7f`, `5ae75fbf-2c81-45ed-9881-94448337644c`,
    `a4d01e0f-97f9-4733-b7c4-7ff21034e3e2`, `f0a991a9-0eab-4b03-b4eb-0ef3b61fc5fe`,
    `4259b94e-898d-44ca-8c7b-407e8934aacf`, and `116b34f7-0255-4a34-84b8-e5e126974abd`) read
    `official-messages-gameplay.md`.
  - None of the above sessions may implement the gameplay compat handlers.
- **The clean-room orchestrating session (`session_016AYk1rTJPsWsU2hLiLsh19`)** saw only that
  spec's section headings, in a grep, and recorded the audit reports in `docs/m8-remediation.md`
  (`ec28767`, `a6e47bb`, `c5b4527`, `7f72d1c`, `0f510d4`, `aedc9d9`, `1f0b6c4`, `b470bcb`). The
  §11 auditor of Pass 4 commit `9b31155` (recorded in `main` commit `aedc9d9` under parent
  `session_016AYk1rTJPsWsU2hLiLsh19`; individual auditor subagent session ID not recorded in
  the repository), the §12 auditor of Pass 5 commit `2e30790` (recorded in `main` commit
  `1f0b6c4` under parent `session_016AYk1rTJPsWsU2hLiLsh19`; individual auditor subagent
  session ID not recorded in the repository), and the §13 check of Pass 6 commits `2ebe9bc` +
  `3f98b17` (recorded in `main` commit `b470bcb` under parent
  `session_016AYk1rTJPsWsU2hLiLsh19`; individual auditor subagent session ID not recorded in
  the repository) audited Stage 1 core only and did not open the held gameplay spec. The
  clean-room orchestrating session implements nothing in the gameplay area.
- **Other reviewers:** the login, session, world, play, wiring and own-protocol reviewers
  did not open the gameplay spec.
- **Code:** no gameplay handler code exists, so no code depends on the material.
- **Remediation sessions (2026-10-23 to 2026-10-25, passes 1–7):** the single dirty-room
  remediation orchestrating (parent) session (`fa9950b7-7f4d-4d9f-b0bf-2c191d044cd8`, which
  orchestrated Pass 1, Pass 2, Pass 3, Stage 1 core Pass 4, Stage 1 core Pass 5, Stage 1 core
  Pass 6, and Stage 1 core Pass 7 — no other dirty-room parent session existed), the four pass-1
  writer/screener sessions (`95f8fd58-e269-427f-8ff4-ca955aa5cdcc`,
  `df6da885-d0e7-4f60-87d5-f42404d7b9a6`, `d429b64d-c76e-4bca-b71e-442bb5062022`,
  `ae7300ae-541e-4c08-baa5-458b0bbef6f6`), the six pass-1 reviewer sessions
  (`e74ae85e-d157-4eed-ae01-e184492985e0`, `a8686178-b869-4b5e-a35f-08a974755163`,
  `29b19850-a364-4f43-938b-df926dea4769`, `c15822ee-4ee3-4d3e-8c3f-595c95266e01`,
  `a21d3e1e-3dd2-492c-9a44-0c96e2107eb0`, `b19d415b-dc7b-4c43-9311-fb9eee064c9f`), the
  four pass-2 writer sessions (`7aac837b-7c9f-4a3c-a67f-853951f73909`,
  `8abac49d-40d9-4385-8491-f8bd2e0dedba`, `737386f0-70e7-4347-9d02-ae041ef73558`,
  `e960b359-2849-4558-bb15-0b467a99ed50`), the six pass-2 independent reviewer sessions
  (`74ca1ac7-4228-4cce-96af-b39f41f8ba7f`, `1d1d854d-7fcb-4e05-b387-04da8ffbe515`,
  `b36f699d-1eff-4747-a2ae-744dfebfaf48`, `856892dd-b105-44a0-befd-821295c1f831`,
  `f033e79b-34af-4992-b11e-fb7e3d28d961`, `507f2057-5e45-4f15-ae54-693f00478682`), all
  eleven pass-3 writer sessions (`ba457993-a4b9-4887-a1d3-8b63ac36ed4d`,
  `7aa0951a-c68e-4c56-a650-a34b46cf46fa`, `c3312844-b4be-45f4-80bc-63fc0c829bb1`,
  `5ae75fbf-2c81-45ed-9881-94448337644c`, `cc6fd3c4-ab42-49b7-ab17-609b574896c8`,
  `dd9e328f-6f10-4c93-95ae-49e38fa7da52`, `a4d01e0f-97f9-4733-b7c4-7ff21034e3e2`,
  `f305d416-179b-4abd-8b1b-3b6427182a58`, `038c082b-018b-4b93-b2eb-e27ea4be2b9e`,
  `ed3283ee-a5ea-4317-88d7-eb3bfda24c8b`, `f0a991a9-0eab-4b03-b4eb-0ef3b61fc5fe`), all
  twelve pass-3 reviewer sessions (`d3229743-7267-495e-b8a3-e55b02e29a6f`,
  `0b7c813a-61ab-47ae-b197-34ddfe5443e2`, `bc6504c2-c3cf-4d1f-9015-7319597afc04`,
  `4259b94e-898d-44ca-8c7b-407e8934aacf`, `81151ca4-75f4-4645-b1b1-7687cd69d5ae`,
  `2e4ef4ef-b384-4ecc-b126-4fe7a2397532`, `116b34f7-0255-4a34-84b8-e5e126974abd`,
  `a4c73c75-5bab-45c5-aff3-4b5331946564`, `738ede78-985d-4b43-9068-9e92f990df9c`,
  `85005cdc-c106-4409-bc36-da1d31d27825`, `40391aa7-cd75-450f-a618-897ef8b0ed1d`,
  `596830ae-af45-4251-8074-07f61e7c500c`), the four Stage 1 core pass-4 writer sessions
  (`4010c46b-4d10-45e2-8f45-857bee54a524`, `264deacc-73e6-4508-bb8c-df9e3714bd4a`,
  `bf91e2ab-3d69-4d2b-a98a-5ecbcaae3fbc`, `5f20d593-e8c6-4c5f-b050-7cc670ee5d2d`), the
  five Stage 1 core pass-4 independent reviewer sessions
  (`2cbf5815-ab41-434e-9e3d-60ea0d3009b8`, `7acae139-aa07-49c1-97fa-33fa0073a3d3`,
  `e0a5cb26-334b-46c3-8e06-3614811c1eca`, `288f3a73-ed06-466d-97cc-3daf326bcce9`,
  `4078b4b7-91c0-435c-b3c8-6a2107eb17c2`), the four Stage 1 core pass-5 writer sessions
  (`9013ee1d-b5e8-4dec-94e9-01362f120e0d`, `ce0d1358-3438-49bb-b91b-4fa0142af25f`,
  `41b0f4d7-8715-4d46-b80f-626563b2e2f6`, `4a9cb71d-5f7e-4b8c-853e-073d0e4aa021`), the
  four Stage 1 core pass-5 independent reviewer sessions
  (`bd1d6391-5a95-4501-8b24-1ae896887eb3`, `e44bf1f4-ee55-4439-a768-153e6903f8f2`,
  `c2d28819-ada6-4a19-aa21-26edbba5f249`, `25ed04cc-c22b-4da9-9e9d-3f9bb8b1dc72`), the
  five Stage 1 core pass-6 writer sessions (`06a67769-d71f-4771-be1f-af1c7d781ddf`,
  `e39a5eb7-89c1-4e5b-a082-a2d6fbbaba04`, `a07d616c-b2ca-4f12-afa1-b261f1565e32`,
  `38afcada-724b-44bb-a844-95ef24c431b7`, `981692e4-e96e-4876-9bb3-f976b7bd9a41`), the
  five Stage 1 core pass-6 independent reviewer sessions
  (`a971230f-5b5e-4d3f-880a-cbae947c26eb`, `77a385bd-dab3-47b7-b8d1-82afe29e2737`,
  `1f3a190e-df7a-43e5-a0a7-6eed77016a5b`, `b35b4f62-0feb-4286-b2d8-69d4f405beb6`,
  `ff0c954a-39ac-4855-a6c7-21980f50ee55`), the five Stage 1 core pass-7 writer sessions
  (`e0a90b15-7c74-477f-af82-fe24284a82ef`, `678162f3-afcd-42f0-ba8e-010bb9398c5c`,
  `0fa89d0d-b63a-4d65-a422-c928b386c11f`, `685c8123-ea45-4b5e-85a5-6bf376f44935`,
  `44206635-35e0-490e-a18f-34279eb1697b`), and the five Stage 1 core pass-7 independent
  reviewer sessions (`51f17dc6-f21c-4df8-9e60-d0e41af7ff2b`,
  `2ceefe8a-7d00-4de4-90e1-d2d03716cbd9`, `eb6cb6a5-3201-48b2-8777-7291cf0a772d`,
  `2a632aec-4250-47d2-b153-2e6d2f5fc1aa`, `c7513562-9148-4289-b736-0a35d332ea59`) are dirty
  for M8 and must never write project code.

## Actions taken (2026-10-22, branch `m8-protocols`)

- **The gameplay spec is held.** It moved to
  `research/held/incident-0041/official-messages-gameplay.md`, outside the clean zone
  (§10 step 3, revert forward).
- **Statuses corrected:**
  - `official-login.md` and `official-session.md` are Revised;
  - `official-messages-world.md`, `official-messages-play.md` and `property-system.md` §10
    are in Review.
- **Clean fixes continued.** Fixes that need no held or unreviewed fact continued with
  clean implementers who never opened the gameplay spec:
  - hostile-input hardening in `crates/compat`: a fragment-reassembly panic, unchecked
    packets acting before checksum verification, PKCS#7 bounds, non-finite positions, and
    one `unreachable!`;
  - one conformance fix in Tessera's own protocol (the Airborne gait).
- **Building paused.** No further official-protocol code is built from these specs until
  they are Accepted again.

## Remediation (2026-10-23 to 2026-10-25, branch `m8-remediation`)

### 1. Audit history and staged remediation ([decision 0042](0042-m8-remediation-decisions.md) items 8–9)

- **Pass 1 (`19acce5`, 2026-10-23) — `BLOCK` (`docs/m8-remediation.md` §7):** Reviewers edited
  three of the specifications they accepted (`B1`), unsourced PascalCase script class names
  remained in the gameplay spec (`B2`), client-structure prose remained in three specs (`B3`),
  cross-spec wire contradictions remained (`B4`), and notes `N1`–`N10` were raised.
- **Pass 2 (`6c71fe66`, 2026-10-23) — `BLOCK` (`docs/m8-remediation.md` §8):** Writer and
  reviewer changes were combined in a single commit (`B1`), cross-spec contradictions remained
  on `FragmentedBlob` header fields, `VariableU32` `0xE0`, routing headers, `0x24`, `0xCC`,
  `0xCE`, `base_flags`, ticket refresh, and `ChatSession` connection state (`B4`), and
  remaining client-internal residue, C/Rust operator prose (`N2`), login test-vector byte
  counts, and status-header gating were flagged.
- **Pass 3 (`a780a67`, `3e6a06e`, `ed9b5b9`, 2026-10-24) — `BLOCK` (`docs/m8-remediation.md`
  §9–§10):** Pass 3 separated the writer and reviewer commits and resolved most of §8 and §9
  across the core specifications, but writer `f0a991a9-0eab-4b03-b4eb-0ef3b61fc5fe` introduced
  client-internal C++ class/method names and bytecode internals into
  `official-messages-gameplay.md` in `3e6a06e` (which reviewer
  `116b34f7-0255-4a34-84b8-e5e126974abd` missed), `3e6a06e` and `0041` quoted in-memory
  structure offsets, contradictions remained in the gameplay spec, and a small set of core
  items remained in `official-login.md`, `official-session.md`,
  `official-messages-world.md`, and `official-messages-play.md`.
- **Pass 4 — Stage 1 Core (`5470c8f`, `9b31155`, 2026-10-24) — `BLOCK, close`
  (`docs/m8-remediation.md` §11):** Pass 4 held `official-messages-gameplay.md` for Stage 2,
  removed all links to it from the five Stage 1 core specifications, and rebuilt
  `m8-remediation` freshly from `main`, but was blocked by `B1` (`ChatSession` endpoint
  described in `official-session.md` §3.3 as an in-band world message rather than
  `--chatserver`), `B2` (launch parameter names in `official-session.md` §3.2–§3.3 not matching
  `official-login.md` §4.1–§4.2), `B3` (two unresolved `SessionAuthHeader` `0x00010000`
  `authType` alternatives and fixed `108`-byte vs variable `144..256`-character Base64 ticket
  lengths), `B4` (`ConnectAcceptHeader` `0x00080000` `checkValue` formula missing), `B5`
  (`0x1FBC4` decimal conversion, `20` vs `18` world-teardown opcodes, and `56`-byte vs `52`-byte
  `ActionPayload` maximum), and non-blocking notes (internal parser mask and low-byte flag
  names, pre-load calendar fallback, `localRecId` and `pakSize` bounds, `OPEN` callouts, and
  product tokens in `official-login.md` §7.1).
- **Pass 5 — Stage 1 Core (`fc19276`, `2e30790`, 2026-10-25) — `BLOCK, narrow`
  (`docs/m8-remediation.md` §12):** Pass 5 resolved all §11 blocking items (`B1`–`B5`) and
  nearly all §11 notes with zero contamination, but was blocked by three narrow items:
  1. `CompactPID` first-byte `0x81..0xFF` rejection in `official-messages-world.md` §2.4.3 was
     not yet mirrored in `property-system.md` §10.2 (requiring a writer update and new
     independent review of `property-system.md` §10);
  2. the pass-5 writers modified earlier `2026-10-23` and `2026-10-24` Change Log rows in
     `official-session.md`, `official-messages-world.md`, and `official-messages-play.md` beyond
     redacting the two removed internal values; and
  3. the Exposure section of this record did not explicitly identify the pass-4/pass-5
     orchestrating parent session or state that the individual subagent session IDs of the
     §7–§10 and `ed9b5b9` auditors were not recorded in the repository;
  plus minor notes on section citations, `0x000000CE` `expected_stamp`, Base64 decoded byte
  length wording, `localHost:remoteHost:port` syntax, `0x24` sender rules,
  `LoginQueueStatusMessage` (`0x00000130`) optionality, and three line errors in this record.
- **Pass 6 — Stage 1 Core (`2ebe9bc`, `3f98b17`, 2026-10-25) — `BLOCK, one sentence`
  (`docs/m8-remediation.md` §13):** Pass 6 resolved all three §12 blocking items and nearly all
  §12 notes with zero contamination, but `official-messages-play.md` §4.5 step 2 (`P150`) stated
  that `0x000000CE` `seq_byte` is present when bit 3 is clear (contradicting `official-messages-world.md`
  §5.4 row 5 and `official-messages-play.md` §4.5 step 3 where `seq_byte` is present when
  `header_flags` bit 3 `0x08` is set, and conflating `mode_flags` bit 3 with `header_flags`
  bit 3), alongside minor notes on `BIN-03` provenance, §6.3 field names, and a §10.2 synthetic
  handshake test vector for `checkValue` in `official-session.md`, `slot_flags` `OPEN` mask
  `0xEE` and the `W1047` citation in `official-messages-world.md`, `"up to"` Base64 decoded byte
  lengths in `official-login.md`, and the `CompactPID` `0x81..0xFF` Tessera hostile-input rule
  note in `property-system.md`.
- **Stage 1 Core Remediation (`Pass 7`, 2026-10-25,
  [decision 0042](0042-m8-remediation-decisions.md) items 8–9):** Per owner-delegated
  [decision 0042](0042-m8-remediation-decisions.md) items 8 and 9 and
  `docs/m8-remediation.md` §12–§13:
  1. `official-messages-gameplay.md` remains held in
     `research/held/incident-0041/official-messages-gameplay.md` outside `docs/specs/`, with its
     pass-3 acceptance voided (`Stage 2` waits until Stage 1 is merged and built).
  2. All links and dependencies on `official-messages-gameplay.md` are absent from the five
     Stage 1 core specifications (`official-login.md`, `official-session.md`,
     `official-messages-world.md`, `official-messages-play.md`, and `property-system.md` §10),
     and Pass 7 addresses the `docs/m8-remediation.md` §13 blocking sentence and all §13 notes.
  3. Each remediation pass is split into a writer commit (`Clean-Room-Role: spec`) placing the
     modified specifications in `Review`, followed by a header-only reviewer commit
     (`Clean-Room-Role: review`) by brand-new independent reviewer sessions; when Stage 1 passes
     audit, `m8-remediation` merges into `main` as **one squashed commit** of the final clean
     text so intermediate commits (`5470c8f` and `9b31155`) do not enter `main`'s history.

### 2. What was cleaned by file and kind of problem

- **`research/held/incident-0041/official-messages-gameplay.md` (`C1`, `W1`, held for Stage 2):**
  Held outside `docs/specs/` with its acceptance voided per [decision 0042](0042-m8-remediation-decisions.md)
  item 8 and `docs/m8-remediation.md` §10. None of the five Stage 1 core specifications link to
  or depend on it.
- **`docs/specs/official-login.md` (`C2`, `W2`, passes 1–7):** Stripped client-internal and
  launcher-internal class, struct, handler, listener, and variable names, code-scan instruction
  patterns, data-section signature tables, diagnostic format-string anchors, and quoted internal
  values from change-log rows (`O1`, `N1`, [decision 0042](0042-m8-remediation-decisions.md)
  item 1, §8–§11). Deduplicated `VariableU32`, Master DID Map `3` (`0x28000010`), and
  dictionary/codec negotiation in §2.4.3, §4.2, `I16`, and `V7` by linking to
  `official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4 (`B4`, `N2`, §9–§10), reconciled the
  in-game `RefreshTicket` schedule, launch switches (`-h` / `--host` including the colon-delimited
  `remoteHost`, `remoteHost:port`, and `localHost:remoteHost:port` forms with metasyntactic angle
  brackets, `-p` / `--port`, `--outport`, `--chatserver`), `SessionAuthHeader` (`0x00010000`)
  `authType` selection and decoded byte length of the Base64 ticket string (including `"up to"`
  padding ranges), and `WorldReferralHeader` (§6.5) citations with `official-messages-play.md`
  and `official-session.md` (§8–§13), removed the fallback status URL built from path fragments
  and documented SOAP namespace/action URIs as non-endpoint protocol identifiers (`N6`,
  [decision 0042](0042-m8-remediation-decisions.md) item 3), replaced product token examples in
  §7.1 with synthetic tokens (§11), and aligned all synthetic test vector byte counts and §7.2
  labels (§8–§10).
- **`docs/specs/official-session.md` (`C3`, `W2`, passes 1–7):** Stripped client-internal
  class, struct, function, and optional-header names, in-memory connection-table and socket
  staging-buffer prose, dormant dead-code branches, client-structure prose in open-question and
  change-log notes, and client-internal status/reason codes that do not travel on the wire
  (`W2`, §8–§12). Removed all links to the held gameplay spec (`§10`), unified the 4-tier (`1`,
  `2`, `4`, `5` byte) `VariableU32` definition (`0xE0` prefix), `localRecId` (`0x0100..0xFEFF`)
  and `pakSize` (`0..65,403` / `0..65,382`) bounds, `129,988` (`0x1FBC4`) retransmission packet
  count, dictionary redirect rule (`redirectDid` non-zero and not equal to the record's own DID),
  launch parameters (`-h` / `--host` including the colon-delimited `remoteHost`,
  `remoteHost:port`, and `localHost:remoteHost:port` forms with metasyntactic angle brackets,
  `-p` / `--port`, `--outport`, `--chatserver`), `SessionAuthHeader` (`0x00010000`)
  deterministic `authType` selection rule and Base64-decoded ticket length, transport vs world
  UTF-16LE string length-prefix note, `LoginQueueStatusMessage` (`0x00000130`) optionality when
  no login queue is active, and `base_flags = 0x00` inner event offset across all core specs
  (`B1`–`B5`, §8–§13), disambiguated the separate `ChatSession` opcode space for `0x0179`/`0x0186`
  (§8), replaced pseudo-code blocks and C operators with prose steps (`N2`, §8), derived the
  deterministic `ConnectAcceptHeader` (`0x00080000`) `checkValue` (`u64 LE` at `+0x00..+0x07`)
  formula from wire bytes `+0x08..+0x0F` (`assignedClientRecId`, `serverRecId`, `outgoingSeed`) of
  `ConnectChallengeHeader` (`0x00040000`, `BIN-03` provenance in §2 and synthetic handshake test
  vector 5 in §10.2, resolving `OPEN-SESSION-5`, §12–§13), restored the `2026-10-23` and
  `2026-10-24` Change Log rows to their `9b31155` text with `[value removed under §11]` redactions
  (§12), and documented explicit `OPEN` callouts (`OPEN-SESSION-1`..`OPEN-SESSION-4` and
  `OPEN-SESSION-6`, `5` `OPEN` total, §8–§13).
- **`docs/specs/official-messages-world.md` (`C4`, `W3`, passes 1–7):** Screened and stripped
  client-internal class and type names, event-factory/handler/pipeline registration prose,
  internal archive mode-flag tables, game-string enum label tables, pre-load fallback defaults,
  and quoted internal values in change-log rows (`W3`, `B3`, §8–§12). Consolidated the single
  canonical definitions of `0x0000006F`, `0x00000043`/`0x000000AA`
  (`target_or_channel_mask`)/`0x000000FA` (§4.1), `0x00000024` (with `u8 teleport_stamp` at
  inner offset `0x09` echoing `0xCE` per `official-messages-play.md` §4.5, and explicit sender
  and receiver rules for `trailer_u32` and the `OPEN` `slot_flags` bits `1, 2, 3, 5, 6, 7` mask
  `0xEE` in §5.5 and §8), `ActionPayload` (`11` to `56` bytes before `action_props`), `0x000000CC`
  (`surface_controlled_physics`), `0x000000CE` (citing `official-messages-play.md` §4.5 in row 5),
  and `0x00000098`/`0x00000052` (`B4`–`B5`, §8–§13), stated `LoginQueueStatusMessage`
  (`0x00000130`) optionality in §1.1.4 (§12), aligned `CompactPID` invalid first-byte range
  (`0x81..0xFF` as a Tessera hostile-input hardening rule) with `property-system.md` §10.2
  (§11–§13), clarified the runtime architecture for generic `.dat`-driven `EncodedPropertyTable`
  decoding (`0x34000000`), `0x07000029` state schemas, and `0x35000000` calendar parameters vs
  protocol-level numeric `PID` adapter matching ([decision 0012](0012-numeric-constants.md),
  [decision 0034](0034-official-server-compatibility.md), §10–§11), restored the `2026-10-23` and
  `2026-10-24` Change Log rows to their `9b31155` text with `[value removed under §11]`
  redactions (§12), and added explicit `OPEN` callouts (`N3`, §8–§13).
- **`docs/specs/official-messages-play.md` (`C5`, `W3`, passes 1–7):** Screened and stripped
  client-internal class and type names, in-memory prediction ring-buffer prose, internal
  archive mode-flag references, non-wire service type/IDs, internal UI notice IDs, enum label
  tables, internal provenance descriptions, and quoted internal values in change-log rows (`W3`,
  `B3`, §8–§12). Removed all links to the held gameplay spec and made §5.2–§5.3 (`0x00000030`
  `script_rpc_message` and `ScriptArgFrame` `0x008E` primitive/`ScriptStateObject` `ref` wire
  encoding) and §7.2 (`20` world-teardown opcodes in `0x3000001B`–`0x30000043`, excluding
  `0x30000045` from logout triggers) completely self-contained (`B5`, §10–§11), linked to
  `official-messages-world.md` for the canonical `0x6F`, `0x24` (aligning Test Vector 1 field
  names), `ActionPayload` (`11` to `56` bytes and `sub_entry_flag`), `0xCC`
  (`surface_controlled_physics`), `0xCE` (supplying `expected_stamp` in `seq_byte` when bit 3
  `0x08` of `header_flags` is set in §4.5 steps 2–3, §12–§13), `0xAA`/`0xFA`/`0x43`, and `0xB3`
  (`sync_flags`) layouts and to `official-login.md` §4.1–§4.3 for CLI switches and
  `RefreshTicket` (`B3`–`B5`, §8–§13), corrected the `0x000000BF` wire size range (`6` to `57`
  bytes, §10) and `0x0000016A` field list (§10), aligned `ChatSession` connection states and
  `SessionAuthHeader` with `official-session.md` (`B3`, §8–§13), added the normative ignore rule
  for `0x30000045`, `0x84EC002C`, `0x0000015F`, `0x0000017D`, and `0x000000ED` controller
  payloads ([decision 0042](0042-m8-remediation-decisions.md) item 2, §10–§11), restored the
  `2026-10-24` Change Log rows verbatim to `9b31155` and removed broken section citations and
  unreferenced anchors (§12), and added explicit `OPEN` callouts (§8–§11).
- **`docs/specs/property-system.md` §10 (`C6`, `W3`, passes 1–4, 6–7):** Screened §10 for
  client-internal names and layouts (`W3`), removed internal archive stream mode-flag bit
  tables (`B3`), retained Tessera's own clean-room type names (`N10`), added the explicit
  `0x81..0xFF` first-byte rejection rule for `CompactPID` in §10.2 and provenance row `N2` as a
  Tessera hostile-input hardening rule matching `official-messages-world.md` §2.4.3 and §7.1
  (§12–§13), and gated §10 acceptance on [decision 0042](0042-m8-remediation-decisions.md) items
  4 & 8 (§8–§13).

### 3. Complete writer and independent reviewer session table (passes 1–7)

Every writer and reviewer session recorded in the specifications' Change Logs across passes 1–7 is listed below in one consistent table:

| Item | Specification | Pass 1 Writer (`W1`–`W3`) | Pass 1 Reviewer (`W4`) | Pass 2 Writer | Pass 2 Reviewer | Pass 3 Writers (`3a` / `3b`) | Pass 3 Reviewers (`3a` / `3b`) | Pass 4 Writer / Reviewer (Stage 1 Core) | Pass 5 Writer / Reviewer (Stage 1 Core) | Pass 6 Writer / Reviewer (Stage 1 Core) | Pass 7 Writer (Stage 1 Core) | Pass 7 Independent Reviewer (Stage 1 Core — review only, zero body edits) | Stage 1 Core Status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `C1` | `research/held/incident-0041/official-messages-gameplay.md` | `95f8fd58-e269-427f-8ff4-ca955aa5cdcc` | `e74ae85e-d157-4eed-ae01-e184492985e0` | `7aac837b-7c9f-4a3c-a67f-853951f73909` | `74ca1ac7-4228-4cce-96af-b39f41f8ba7f` | `5ae75fbf-2c81-45ed-9881-94448337644c`, `a4d01e0f-97f9-4733-b7c4-7ff21034e3e2`, `f0a991a9-0eab-4b03-b4eb-0ef3b61fc5fe` | `4259b94e-898d-44ca-8c7b-407e8934aacf`, `116b34f7-0255-4a34-84b8-e5e126974abd` | — *(Held for Stage 2 per decision 0042 item 8)* | — | — | — | — *(Pass 3 acceptance voided)* | **Held outside `docs/specs/` (Stage 2)** |
| `C2` | `docs/specs/official-login.md` | `df6da885-d0e7-4f60-87d5-f42404d7b9a6` | `a8686178-b869-4b5e-a35f-08a974755163` | `8abac49d-40d9-4385-8491-f8bd2e0dedba` | `1d1d854d-7fcb-4e05-b387-04da8ffbe515` | `5ae75fbf-2c81-45ed-9881-94448337644c`, `a4d01e0f-97f9-4733-b7c4-7ff21034e3e2`, `f0a991a9-0eab-4b03-b4eb-0ef3b61fc5fe` | `81151ca4-75f4-4645-b1b1-7687cd69d5ae`, `a4c73c75-5bab-45c5-aff3-4b5331946564` | `5f20d593-e8c6-4c5f-b050-7cc670ee5d2d` / `2cbf5815-ab41-434e-9e3d-60ea0d3009b8` | `9013ee1d-b5e8-4dec-94e9-01362f120e0d` / `bd1d6391-5a95-4501-8b24-1ae896887eb3` | `e39a5eb7-89c1-4e5b-a082-a2d6fbbaba04` / `a971230f-5b5e-4d3f-880a-cbae947c26eb` | `685c8123-ea45-4b5e-85a5-6bf376f44935` | `51f17dc6-f21c-4df8-9e60-d0e41af7ff2b` | 2026-10-25 — **Pass** (`Accepted`, gated by decision 0042 items 4 & 8; `0` `OPEN`) |
| `C3` | `docs/specs/official-session.md` | `d429b64d-c76e-4bca-b71e-442bb5062022` | `29b19850-a364-4f43-938b-df926dea4769` | `737386f0-70e7-4347-9d02-ae041ef73558` | `b36f699d-1eff-4747-a2ae-744dfebfaf48` | `ba457993-a4b9-4887-a1d3-8b63ac36ed4d`, `f305d416-179b-4abd-8b1b-3b6427182a58` | `d3229743-7267-495e-b8a3-e55b02e29a6f`, `738ede78-985d-4b43-9068-9e92f990df9c` | `4010c46b-4d10-45e2-8f45-857bee54a524` / `7acae139-aa07-49c1-97fa-33fa0073a3d3` | `ce0d1358-3438-49bb-b91b-4fa0142af25f` / `e44bf1f4-ee55-4439-a768-153e6903f8f2` | `06a67769-d71f-4771-be1f-af1c7d781ddf` / `77a385bd-dab3-47b7-b8d1-82afe29e2737` | `678162f3-afcd-42f0-ba8e-010bb9398c5c` | `2ceefe8a-7d00-4de4-90e1-d2d03716cbd9` | 2026-10-25 — **Pass** (`Accepted`, gated by decision 0042 items 4 & 8; `5` `OPEN`) |
| `C4` | `docs/specs/official-messages-world.md` | `ae7300ae-541e-4c08-baa5-458b0bbef6f6` | `c15822ee-4ee3-4d3e-8c3f-595c95266e01` | `e960b359-2849-4558-bb15-0b467a99ed50` | `856892dd-b105-44a0-befd-821295c1f831` | `7aa0951a-c68e-4c56-a650-a34b46cf46fa`, `cc6fd3c4-ab42-49b7-ab17-609b574896c8`, `038c082b-018b-4b93-b2eb-e27ea4be2b9e` | `0b7c813a-61ab-47ae-b197-34ddfe5443e2`, `85005cdc-c106-4409-bc36-da1d31d27825` | `264deacc-73e6-4508-bb8c-df9e3714bd4a` / `e0a5cb26-334b-46c3-8e06-3614811c1eca` | `41b0f4d7-8715-4d46-b80f-626563b2e2f6` / `c2d28819-ada6-4a19-aa21-26edbba5f249` | `a07d616c-b2ca-4f12-afa1-b261f1565e32` / `1f3a190e-df7a-43e5-a0a7-6eed77016a5b` | `0fa89d0d-b63a-4d65-a422-c928b386c11f` | `eb6cb6a5-3201-48b2-8777-7291cf0a772d` | 2026-10-25 — **Pass** (`Accepted`, gated by decision 0042 items 4 & 8; `9` `OPEN`) |
| `C5` | `docs/specs/official-messages-play.md` | `ae7300ae-541e-4c08-baa5-458b0bbef6f6` | `a21d3e1e-3dd2-492c-9a44-0c96e2107eb0` | `e960b359-2849-4558-bb15-0b467a99ed50` | `f033e79b-34af-4992-b11e-fb7e3d28d961` | `c3312844-b4be-45f4-80bc-63fc0c829bb1`, `dd9e328f-6f10-4c93-95ae-49e38fa7da52`, `ed3283ee-a5ea-4317-88d7-eb3bfda24c8b` | `bc6504c2-c3cf-4d1f-9015-7319597afc04`, `40391aa7-cd75-450f-a618-897ef8b0ed1d` | `bf91e2ab-3d69-4d2b-a98a-5ecbcaae3fbc` / `288f3a73-ed06-466d-97cc-3daf326bcce9` | `4a9cb71d-5f7e-4b8c-853e-073d0e4aa021` / `25ed04cc-c22b-4da9-9e9d-3f9bb8b1dc72` | `38afcada-724b-44bb-a844-95ef24c431b7` / `b35b4f62-0feb-4286-b2d8-69d4f405beb6` | `e0a90b15-7c74-477f-af82-fe24284a82ef` | `2a632aec-4250-47d2-b153-2e6d2f5fc1aa` | 2026-10-25 — **Pass** (`Accepted`, gated by decision 0042 items 4 & 8; `5` `OPEN`) |
| `C6` | `docs/specs/property-system.md` §10 | `ae7300ae-541e-4c08-baa5-458b0bbef6f6` | `b19d415b-dc7b-4c43-9311-fb9eee064c9f` | `e960b359-2849-4558-bb15-0b467a99ed50` | `507f2057-5e45-4f15-ae54-693f00478682` | `5ae75fbf-2c81-45ed-9881-94448337644c`, `dd9e328f-6f10-4c93-95ae-49e38fa7da52` | `2e4ef4ef-b384-4ecc-b126-4fe7a2397532`, `596830ae-af45-4251-8074-07f61e7c500c` | `5f20d593-e8c6-4c5f-b050-7cc670ee5d2d` / `4078b4b7-91c0-435c-b3c8-6a2107eb17c2` | — / `4078b4b7-91c0-435c-b3c8-6a2107eb17c2` *(Pass 4)* | `981692e4-e96e-4876-9bb3-f976b7bd9a41` / `ff0c954a-39ac-4855-a6c7-21980f50ee55` | `44206635-35e0-490e-a18f-34279eb1697b` | `c7513562-9148-4289-b736-0a35d332ea59` | 2026-10-25 — **Pass** (`Accepted` §10, gated by decision 0042 items 4 & 8; `0` `OPEN` in §10) |

### 4. Resolution of open points (`O1`–`O3` and `crates/compat`)

The open points (`O1`–`O3`), the disposition of `crates/compat` (kept on the basis of the
clean audit in Finding 5 above together with the `W5` neutral renames, the `O1` removal,
and a conformance check against the re-accepted specifications), the staged delivery of M8
(Stage 1 core specs first with `official-messages-gameplay.md` held for Stage 2), and the
clean-history branch rebuild and audit gate for closing this incident record and resuming M8
(`M8_PAUSED = false`) are settled in [decision 0042](0042-m8-remediation-decisions.md).

## Pass 9 writer remediation (2026-10-30)

Pass 8 was blocked by the audit in `docs/m8-remediation.md` §16. Pass 9 starts from
`origin/main` and imports the spec content of writer diff `e26848b..2bc806a`, credited
to Gemini through Antigravity, writer session `c0194bf2-43a9-4f65-a6c2-3e421509c2f7`.
It does not import pre-recorded review results; the archived exposure identities are
retained separately above, without assigning them any pass-9 role. The five specs remain **Review**.
The accepted pass-7 text and its historical session records above describe that earlier pass.
No pass-9 acceptance is recorded by the writer.

The pass-8 decision-summary additions are not imported: they disagree with the specs.
The origin of those unsupported values is not known to this session. The dirty-room
reconciliation note is `research/notes/m8-pass9-reconciliation.md`; it records that limitation
rather than asserting an observation that did not occur.

The current functional answers are solely in the specs:

| Topic | Authoritative spec sections |
|---|---|
| Bind and Case B | `official-session.md` §3.1–§3.4 |
| Datagram layout, checksums and first packet | `official-session.md` §4.1–§4.4, §6.2–§6.4, §10.2 Vectors 5–6 |
| Queue assignments and version exchange | `official-session.md` §5.6.2 and §9.3; `official-messages-play.md` §4.2, §4.5 and §6.6.3 |
| Disconnect behavior and state labels | `official-session.md` §6.1 and §8.2 |
| Login address policy and remaining launcher questions | `official-login.md` §2, §3 and §6.2 |
| S1 world fields and movement | `official-messages-world.md` §2.6, §2.8 and §5.5; `official-messages-play.md` §4.2 and §4.5 |
| Documented RPC layouts and unresolved generic signatures | `official-messages-play.md` §5.1, §6.3–§7.1; decision 0042 item 10 |
| Property wire arguments | `property-system.md` §10.4 |

### Pass 9 writer sessions

| Tool / model | Session ID | Role | Scope | State |
|---|---|---|---|---|
| Codex / GPT-6 | not recorded (host does not expose it); task `/root` | spec writer | `official-login.md` | Review |
| Codex / GPT-6 | same session, not recorded | spec writer | `official-session.md` | Review |
| Codex / GPT-6 | same session, not recorded | spec writer | `official-messages-world.md` | Review |
| Codex / GPT-6 | same session, not recorded | spec writer | `official-messages-play.md` | Review |
| Codex / GPT-6 | same session, not recorded | spec writer | `property-system.md` §10 | Review |
| Codex / GPT-6 | same session, not recorded; continuation after `73c0571` | spec writer | §17 corrections across the five Stage 1 specs and neutral records | Review |
| Codex / GPT-6 | UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator/review_session_login` | independent reviewer | `official-login.md` | 2026-10-30 — PASS at `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`; Accepted, gated by decision 0042 items 4 & 8 |
| Codex / GPT-6 | UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator/review_session_login` | independent reviewer | `official-session.md` | 2026-10-30 — PASS at `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`; Accepted, gated by decision 0042 items 4 & 8 |
| Codex / GPT-6 | UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator/review_world_play` | independent reviewer | `official-messages-world.md` | 2026-10-30 — PASS at `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`; Accepted, gated by decision 0042 items 4 & 8 |
| Codex / GPT-6 | UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator/review_world_play` | independent reviewer | `official-messages-play.md` | 2026-10-30 — PASS at `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`; Accepted, gated by decision 0042 items 4 & 8 |
| Codex / GPT-6 | UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator/review_property` | independent reviewer | `property-system.md` §10 | 2026-10-30 — PASS at `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`; Accepted, gated by decision 0042 items 4 & 8 |
| Codex / GPT-6 | UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator` | review coordinator | Started three fresh reviewers; wrote none of this pass | Results coordinated; no independent spec acceptance claimed |

Independent reviewer sessions must be started by a session that wrote none of this pass.
They append their own exposure and session rows and make only the permitted header and
change-log edits. This writer has neither assigned reviewer identities nor recorded results.

**Owner confirmation (2026-10-30):** the pass-9 review coordinator started as a fresh Codex
session without the writer's conversation; the pass-9 review is accepted as independent
(`docs/m8-remediation.md` §18).
