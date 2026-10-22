<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0041 — Incident: disassembly prose in the M8 gameplay spec, M8 specs without independent review

**Status:** Proposed incident record, 2026-10-22. Recorded under `docs/CLEAN_ROOM.md` §10.
The owner decides the open points below.

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

- **Who read the gameplay spec:** in audit workflow `wf_0939180e-4d4`, the auditor, the
  four gameplay gap reviewers, their verifiers and the planner. None of them may implement
  the gameplay compat handlers.
- **The orchestrating session** saw only that spec's section headings, in a grep. It
  implements nothing in the gameplay area.
- **Other reviewers:** the login, session, world, play, wiring and own-protocol reviewers
  did not open the gameplay spec.
- **Code:** no gameplay handler code exists, so no code depends on the material.

## Actions taken (2026-10-22, branch `m8-protocols`)

- **The gameplay spec is held.** It moved to
  `research/held/incident-0041/official-messages-gameplay.md`, outside the clean zone
  (§10 step 3, revert forward). Links to it are dangling until it returns.
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

## Open for the owner

1. **Clean the spec.** A spec writer reduces the held gameplay spec to wire facts and
   removes internal names and layouts from the session and login specs.
2. **Review independently.** Each of the five specs and `property-system.md` §10 needs a
   session that wrote none of it, with the reviewer, date and result recorded in its
   header. This is dirty-room work on the official protocol. The orchestrating session
   does not take on official-protocol research, so the owner arranges it.
3. **Version scan.** Decide whether Tessera may scan the client binary's code at run time
   to find the version (`login.rs` `extract_netver_from_pe_bytes`), or whether it should
   use only the data-section fallback or a value the user supplies.
4. **Code provenance.** Record which session wrote `crates/compat` and its role. If that
   cannot be shown, decide whether a fresh clean session re-implements it from the
   re-accepted specs.
