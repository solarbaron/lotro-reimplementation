<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0011 — Standing authorisation of method M4 for the offline client

**Status:** Accepted, 2026-10-05. Scope amended by
[decision 0034](0034-official-server-compatibility.md) §2 (2026-10-11); see the note at the end.

## Context
Research method M4 (disassembly or decompilation of the official client, `docs/CLEAN_ROOM.md`
§4) needs a decision record that names the question. Decision 0009 authorised it after the fact
for one question, the script-table node classes. Many remaining facts are not in the data and
are not documented anywhere public. Examples:
- terrain texture blending, scenery and grass placement, collision use, lighting and the sky;
- how character sliders and the `scale_blend` animation layer are driven.

On 2026-10-05 the owner asked to "reverse engineer and decompile the client for all the stuff
such as the world". The owner is a lawful licensee in an EU member state.

## Decision
The owner authorises M4 as a standing method for the **offline client**, on these terms:

- **Scope:** everything M0–M7 need to reproduce the client's own behaviour on the owner's own
  data. That covers formats, the world (terrain, scenery, collision, environment), rendering,
  characters and animation, UI, audio and scripting.
- **Excluded**, and still gated by decision 0004, `docs/LEGAL.md` §9 and the "Forbidden" row of
  `CLEAN_ROOM.md` §4:
  - the network protocol;
  - login and authentication, keys and encryption;
  - the launcher and patcher;
  - anti-cheat and anti-tamper;
  - any circumvention of an access control.
- **Analyst:** dirty-room spec-writing sessions operated by the owner, under EU law (Directive
  2009/24/EC, Art. 6), on the client build installed at the time.
- **Order of methods:** a spec writer still tries M1–M3 first. M4 is for facts those methods
  cannot give, or for confirming a hypothesis.
- **Record:** each spec that uses M4 lists a `BIN` source in its provenance table: the client
  build, the question, and why M1–M3 were not enough. That entry replaces a separate decision
  record per question. Facts that rest only on BIN are marked as such.
- **Output:** prose and tables of functional facts in `docs/specs/` only. Disassembly or
  decompiler listings, pseudo-code, symbol maps and scripts stay in the dirty scratch area.
  They are never committed and never shown to a clean-room session. `CLEAN_ROOM.md` §6–§7 apply
  unchanged.

## Consequences
- `CLEAN_ROOM.md` §4 and the milestone workflow's spec-writer instructions point to this
  record.
- The owner accepts the remaining risks, which `docs/LEGAL.md` describes:
  - **contract:** EULA §4.2 and ToS §10 prohibit reverse engineering;
  - **outside the EU:** exposure where Art. 6 does not apply.
- The clean-room split, the content rules and the M8 gate are unchanged.
- Withdrawing this authorisation means removing the BIN-only facts from the specs and
  re-implementing the code built on them, as decision 0009 describes.

## Amendment (2026-10-11, decision 0034)
[Decision 0034](0034-official-server-compatibility.md) §2 widens the scope above. M4 and
black-box observation may now cover the official game protocol and the login flow (the
official launcher's part of it included) as far as interoperability needs them, including
where key material lies in the user's install, never its values. Targets are set by
subject: read-only copies of the client binary and of the official launcher binary (the
launcher only for its part of the login flow), and of a library either loads where in-scope
code lies in it. Still excluded:
anti-cheat, anti-tamper and code-signing checks, any circumvention of an access control,
and the patcher with its update and download function. Captures of official-server traffic stay
in the dirty room, are never committed, and are reduced to field-level facts. "Still gated
by decision 0004" above now reads "excluded by decision 0034 §2", and "the M8 gate" in
Consequences no longer exists (M8 is in scope).
