<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0009 — Script-table disassembly: incident and retroactive M4 authorisation

**Status:** Accepted, 2026-10-05

## Context
During the M4 milestone run (2026-10-04 to 2026-10-05), the dirty-room spec writer for
`docs/specs/script-table.md` disassembled the official client executable of the owner's
install (`x64/lotroclient64.exe`, client build 4906.0071.1794.4035). Research method M4
(`docs/CLEAN_ROOM.md` §4) is "not authorised by default", and no decision record allowed it.
The milestone workflow's spec-writer instructions did not mention the method gate, which is
how it happened. The pre-commit clean-room audit caught it before anything was committed.
Under `docs/CLEAN_ROOM.md` §10 this is an incident, and it is recorded here.

**What was analysed:** only the client's script-table node classes. That means the node-kind
registration (code → class) and the reflected field list of each node class: field order,
value kind, bound property ID and parent class. Nothing else in the binary was in scope.

**What it fed:** the `BIN` source in `script-table.md`'s provenance table, and facts T4, T7,
T8 and T9 (§4.2, §5.1–§5.3, §6.1). Two kinds of fact rest on it alone:
- the field names and order of the animation node (kind 36, §6.1);
- the layouts of the seven node kinds that never occur in the data.

All other facts in that spec, and the other M4 specs (`havok-data.md`, `skeleton-anim.md`,
`appearance.md`), come from methods M1 and M2.

**Containment:** the spec contains prose and tables only, with no disassembly listings and
no transliterated code. The clean-room implementers of `crates/formats/src/script_table/`
and the code that uses it worked only from the spec. The audit found no contamination in
`crates/`, `apps/`, `fuzz/` or `tools/`.

**Why M1–M3 could not answer it:** the 0x07 and 0x0C records carry no field names or type
tags. The public third-party layout (LotroCompanion's `delta-lotro-dat-utils`, M2) has no
layout for the animation node, and it fails on about a third of this install's records.
Observation (M1) gives byte sizes and value ranges, but not the field meaning needed to drive
clip playback. Kinds that never occur in the data cannot be observed at all. Black-box
observation of the running client (M3) shows nothing about serialised field order.

## Decision
- The owner retroactively authorises method M4 for exactly this question: the node-kind
  registration and the reflected field lists of the script-table node classes. The purpose is
  interoperability with the script-table records in the owner's own install.
- **Analyst:** a dirty-room AI spec-writing session operated by the owner (`solarbaron`) on
  the owner's local install.
- **Build:** client 4906.0071.1794.4035.
- **Output:** prose and tables of functional facts in `docs/specs/script-table.md` only. No
  disassembly output, scripts or notes from that analysis enter the repository. The dirty
  scratch directory stays on the analyst's machine.
- The `script-table.md` facts and the code built from them stand.
- This authorisation does not extend to any other question, subsystem or client build. Any
  further M4 work needs its own decision record first.

## Consequences
- `script-table.md`'s `BIN` provenance row cites this record.
- The milestone workflow (`.claude/workflows/tessera-milestone.js`) now tells spec writers
  that M4 is forbidden unless a decision record names the exact question. Without one, they
  stop and file an `> **OPEN:**` instead.
- If the owner later withdraws this authorisation, follow `docs/CLEAN_ROOM.md` §10 steps 3–4:
  - remove the BIN-only facts from the spec;
  - revert forward the code that depends on them (the animation-node field mapping and the
    readers for unobserved kinds in `crates/formats/src/script_table/`);
  - have an unexposed session re-implement them from the revised spec.
