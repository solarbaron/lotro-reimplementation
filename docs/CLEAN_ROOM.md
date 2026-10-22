<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Clean-room process

Status: adopted 2026-10-03. This process is **binding** for every contributor, human or AI.
It puts into practice the clean-room rules in [`CLAUDE.md`](../CLAUDE.md); read that first.

---

## 1. Why we do this

A clean-room reimplementation must be able to **show** that its code was written from
functional facts (field layouts, enums, state machines, algorithms in plain words) and not
copied from third-party source code or decompiler output.

So the project separates **finding facts** from **writing code**, and keeps a record linking
every implemented behaviour to a written spec with stated sources.

---

## 2. Zones

| Zone | Paths and material | Who may read it |
|---|---|---|
| **Dirty room** (research) | `research/`; any `clones/` directory of third-party code; game binaries and data files examined byte by byte; disassembly or decompiler output ([§4](#4-research-methods)); state and output of decompiler tools (Ghidra project copies, REA snapshots and Evidence bundles, `~/.rea`, REA temporary projects and `/tmp/rea-*`); private packet or frame captures | **Spec writers only** |
| **Interface** | `docs/specs/*.md` | Everyone |
| **Clean room** (implementation) | `crates/`, `apps/`, `fuzz/`, `tools/` | Implementers, working **only** from `docs/specs/` |
| **Neutral** | `docs/` other than `docs/specs/` (this file, roadmap, tooling notes), root files such as `CLAUDE.md`, `Cargo.toml` | Everyone; must contain no format or protocol facts that are not also in a spec |

Rules:

1. `docs/specs/` is the **only** path by which facts move from the dirty room to the clean
   room.
2. Nothing moves the other way except `> **OPEN:**` questions ([§5.4](#54-open-questions)).
3. Material counts as **dirty** if it is any of the following:
   - third-party source code about LOTRO, DDO or Asheron's Call, under any licence or none;
   - third-party format/protocol write-ups, wikis and forum posts that have not been
     reduced to a spec;
   - the contents of `research/`;
   - hex dumps, probes or captures of the game's binaries, data or traffic;
   - disassembly, decompilation, symbol maps, debugger sessions of the official client.
4. General engineering knowledge (Rust, zlib, B-trees, DDS/BCn, wgpu, Lua 5.1, the
   published DirectX format enums) is **not** dirty and may be used anywhere.

---

## 3. Roles

A person or AI session holds **one role per subsystem at a time**. The subsystems are the
crate boundaries (`dat`, `formats`, `install`, `net`, `render`, `script`) plus any spec family
in `docs/specs/`.

| Role | May read | Produces | May not |
|---|---|---|---|
| **Spec writer** | Dirty and neutral material; real data files in their own install (read-only) | Specs in `docs/specs/` | Write or edit code in the clean-room paths for a subsystem they researched |
| **Spec reviewer** | The spec, and the cited sources for spot checks | A review record ([§5.3](#53-review-checklist)) | Approve a spec they wrote |
| **Implementer** | Specs, neutral docs, clean-room code, general references | Code, tests, fuzz targets | Open dirty material of any kind; consult anyone holding dirty knowledge about a fact missing from a spec |
| **Maintainer** | As their other role | Merges, gate decisions, incident handling | Merge their own role-violating work |

**Taint rules:**

- Someone who has read dirty material about a subsystem is a **spec writer for that
  subsystem** and may not implement it. They may implement *other* subsystems they have not
  researched.
- Someone who has seen **Turbine, SSG or Daybreak source code** (leaked, under NDA, as an
  employee or contractor) may not contribute code or specs to any component that code
  touches, and must declare it ([§9](#9-clean-room-attestation)).
- Someone who has read **decompiler or disassembler output** of the official client may write
  specs only, never code, for the affected subsystem.
- Taint does not wear off with time. A reassignment is recorded in `docs/decisions/`.

---

## 4. Research methods

Ordered from most to least preferred. Spec writers use the first method that answers the
question.

| # | Method | Status |
|---|---|---|
| M1 | **Observe the data files** of your own install: hex dumps, small throwaway probes, statistics, validating a hypothesis across many records. Read-only, in place. | **Default. Allowed.** |
| M2 | **Read published research and third-party tools** for facts (layouts, enums, behaviour). Record each source. Never copy code, comments or identifier names beyond those needed for interoperability. | **Allowed.** Licence does not matter for facts; it matters for code, which is never copied. |
| M3 | **Black-box observation of the running official client** (your own account): its logs, files it writes, frame captures (e.g. RenderDoc), your own network traffic. | **Allowed** for rendering, file I/O and UI behaviour, and, by [decision 0034](decisions/0034-official-server-compatibility.md) §2 (2026-10-11), for passive capture of the owner's own sessions of the **unmodified** client with official servers, on the owner's own machine and account, to learn the official game protocol and login flow for interoperability. No injection, debugger or memory reading on a client connected to official servers. Captures stay private in the dirty room, are never committed, and are reduced to field-level facts (no other player's names, chat or account data). |
| M4 | **Disassembly or decompilation** of the official client. | **Authorised for the offline client by [decision 0011](decisions/0011-standing-m4-authorisation.md)** (standing, from 2026-10-05). Use it after M1–M3, and log each use as a `BIN` provenance row in the spec: build, question, and why M1–M3 were not enough. Since [decision 0034](decisions/0034-official-server-compatibility.md) §2 (2026-10-11) it also covers the official game protocol and the login flow (the launcher's part included) as far as interoperability needs them, and where key material lies in the install, never its values. Targets are set by subject: read-only copies of the client binary and of the official launcher binary (the launcher only for its part of the login flow), and of a library either loads where in-scope code lies in it. Anti-cheat, anti-tamper, code-signing checks, any circumvention of an access control and the patcher with its update function stay excluded. Output is prose facts only. Tools: Ghidra headless on a private copy of the shared project (default); REA only under [decision 0033](decisions/0033-rea-for-dirty-room-analysis.md). |
| — | Leaked or NDA source; official server software; modifying or injecting into the official client; bypassing any access control | **Forbidden.** |

Spec writers never write to the install directory and never commit any data observed there
except as **functional facts** (sizes, offsets, enum values, counts, magic numbers).

---

## 5. Spec lifecycle

### 5.1 What a spec contains

Every file in `docs/specs/` has, in order:

1. **Header**: title, status (see 5.2), last-updated date, author role, reviewer.
2. **Purpose**: what the spec lets an implementer build.
3. **Provenance table**: each fact or group of facts, its source(s), and a mark:
   - **Confirmed**: observed in real data, with the date (e.g. "observed in user install
     2026-10-03");
   - **Reported**: from published sources only, not yet observed.
4. **Numbered sections with stable anchors** (`## 3. Directory nodes`, `### 3.2 Entry layout`).
   Once a spec is Accepted, section numbers are never reused for different content, because
   commits cite them.
5. **Byte layouts** as tables: offset, size, type, meaning.
6. **Algorithms in plain words**: steps, conditions and limits. No code, no pseudo-code, no
   transliteration of anyone's source or of decompiler output.
7. **Invariants** a reader can check against real data or a synthetic fixture.
8. **`> **OPEN:**` callouts** for every unknown.

What a spec must **not** contain: code in any language; pseudo-code mirroring a source's
structure; third-party comments or prose; identifier names copied from a third-party
source unless they are needed for interoperability (and then say so); game strings, text
or other expressive content beyond the minimum needed to identify a format; keys,
hostnames or bulk data tables extracted from the client binary (numeric algorithm constants
are functional facts, [decision 0012](decisions/0012-numeric-constants.md)); captures of
official traffic.

### 5.2 States

| State | Meaning | Who moves it |
|---|---|---|
| **Draft** | Being written in the dirty room. Implementers do not use it. | Spec writer |
| **Review** | Complete; waiting for the checklist in 5.3. | Spec writer |
| **Accepted** | Passed review. Implementers may build from it. | Spec reviewer |
| **Implemented** | Code cites it with `Spec-Ref:` and passes its invariants. | Implementer |
| **Revised** | An Accepted spec changed; the change log lists what changed and why. Implementers re-check affected code. | Spec writer + reviewer |
| **Superseded** | Replaced by another spec, which is linked. | Spec reviewer |

### 5.3 Review checklist

The reviewer is the filter between the rooms. A spec passes only when every item is true.
The review record (reviewer, date, result) goes in the spec header and the pull request.

- [ ] Every statement is a functional fact: a layout, value, enum, constraint, sequence or
      behaviour.
- [ ] No code, no pseudo-code, and nothing that follows the structure of a source file or
      decompiler listing.
- [ ] No copied prose, comments or non-interoperability identifiers from any source.
- [ ] No game text, assets, keys, hostnames or official-traffic dumps.
- [ ] Each fact has a provenance entry and is marked Confirmed or Reported.
- [ ] Byte layouts are complete (offset, size, type, meaning) and units are stated.
- [ ] Invariants are concrete enough to test.
- [ ] Unknowns are marked `> **OPEN:**`, not guessed.
- [ ] Nothing in it falls outside the research scope of
      [decision 0034](decisions/0034-official-server-compatibility.md) §2: no anti-cheat,
      anti-tamper or code-signing facts, no key values, certificates, hostnames, endpoints or
      account data, and no other player's data from a capture.

### 5.4 OPEN questions

When an implementer needs a fact the spec does not give, they **stop** and add an
`> **OPEN:** ...` callout to the spec (a spec-only pull request), describing the question,
not a guess. They do **not** look it up in dirty material or ask someone with dirty knowledge
directly. A spec writer answers by revising the spec. The implementer resumes from the
revised spec.

---

## 6. Rules for implementers

1. Work only from `docs/specs/`, neutral docs, the clean-room code, and general references.
2. Do not open `research/`, `clones/`, third-party LOTRO/DDO/Asheron's Call source, or
   decompiler output, not even "just to check".
3. Test against **synthetic fixtures** built by our own code. Real-data tests read
   `TESSERA_GAME_DIR` (or an auto-detected install), are skipped when it is absent, and never
   write to it.
4. Treat all input as hostile: no panics, bounded allocations, `Result` errors, a fuzz
   target per parser (see `CLAUDE.md`).
5. Cite the spec section in the commit ([§8](#8-commit-trailers)).
6. If the code and the spec disagree, fix the spec first through an OPEN question.

---

## 7. AI-assistance rules

The project allows AI coding assistants under these rules. This is a conscious decision:
Wine bans LLM-generated code because no one can guarantee what the model was trained on. We
accept that risk and reduce it with session separation, scanning and human review.

1. **An AI session has a role, like a person.** A session is either a dirty-room
   (spec-writing) session or a clean-room (implementation) session, never both. Agent
   instructions (`CLAUDE.md`) carry these rules into every session.
2. **Clean-room sessions** may read only clean-room paths, neutral docs and `docs/specs/`.
   They must not be given, shown or allowed to fetch `research/`, `clones/`, third-party
   LOTRO/DDO/AC code, decompiler output, or Daybreak files.
3. **Never paste** decompiled or disassembled client code, extracted game files, or
   third-party LOTRO tool code into a session that writes project code.
4. **Dirty-room sessions** output prose specs only. M4 is authorised for the offline client
   ([decision 0011](decisions/0011-standing-m4-authorisation.md)). Disassembler-assisted
   analysis (Ghidra, IDA, REA, other MCP bridges) runs only in such a session. An MCP server
   for such a tool is never registered at user, project or local scope; it is attached per
   session with `--strict-mcp-config`
   ([decision 0033](decisions/0033-rea-for-dirty-room-analysis.md)), and no workflow that
   starts clean agents is ever run from such a session.
5. **Disclose** AI use on every assisted commit with an `Assisted-by:` trailer.
6. **The human who commits is responsible** for the contribution: its licence, provenance
   and correctness. AI tools are not authors and cannot sign off.
7. **Similarity scans.** AI-generated code is scanned with ScanCode Toolkit (licence and
   copyright detection, plus matching against known third-party LOTRO/AC code) before merge.
   A match is treated as a contamination incident ([§10](#10-contamination-incidents)).

---

## 8. Commit trailers

| Trailer | Required on | Format | Example |
|---|---|---|---|
| `Signed-off-by:` | **Every commit** | Developer Certificate of Origin 1.1 sign-off (`git commit -s`), real name or a consistent pseudonym | `Signed-off-by: Jane Doe <jane@example.org>` |
| `Spec-Ref:` | Commits that implement or change a format, protocol or algorithm described in a spec | `docs/specs/<file>.md#<anchor>`; repeat for several sections | `Spec-Ref: docs/specs/dat-container.md#5-directory-b-tree` |
| `Assisted-by:` | Commits where an AI tool generated or materially shaped code or text | `TOOL:MODEL` | `Assisted-by: Claude Code:claude-opus-5-5` |
| `Clean-Room-Role:` | Commits under `docs/specs/` or the clean-room paths | `spec`, `review` or `implementation` | `Clean-Room-Role: implementation` |

Example:

```
dat: parse directory node entries

Decode the fixed-size entry table and reject nodes whose entry count
exceeds the documented maximum.

Spec-Ref: docs/specs/dat-container.md#5-directory-b-tree
Clean-Room-Role: implementation
Assisted-by: Claude Code:claude-opus-5-5
Signed-off-by: Jane Doe <jane@example.org>
```

By adding `Signed-off-by:` you certify the DCO 1.1 (https://developercertificate.org/) **and**
the clean-room attestation in [§9](#9-clean-room-attestation).

**Planned CI enforcement** (tracked in the roadmap): trailer checks; REUSE/SPDX header
check; ScanCode licence and similarity scan; gitleaks/trufflehog secret scanning with rules
for key blobs; a file-type allowlist and magic-number check rejecting archives, textures,
audio, fonts and other binary blobs; a maximum blob size with Git LFS disabled; and a
neutral-naming word-list check on identifiers and UI strings (`CLAUDE.md` Naming).

---

## 9. Clean-room attestation

Every contributor agrees to the following by signing off their first contribution. A
contributor who cannot agree to any item must say so to the maintainers before
contributing.

> **Tessera Clean-Room Attestation (version 1, 2026-10-03)**
>
> For every contribution I sign off, I attest that:
>
> 1. I have not seen source code of Turbine, Standing Stone Games or Daybreak Game Company
>    software, whether leaked, under NDA, through employment or contract, or reconstructed
>    by decompilation, **or** I have disclosed such exposure to the maintainers and I am not
>    contributing to any component it affects.
> 2. My contribution does not copy, translate or paraphrase decompiler or disassembler output,
>    and does not copy code, comments or prose from any source whose licence does not allow
>    it, including unlicensed third-party tools for this game. (Functional facts learned under
>    an authorised method M4, [decision 0011](decisions/0011-standing-m4-authorisation.md),
>    and stated in a spec in our own words are not such output.)
> 3. My contribution contains no game assets, extracted game data, client binaries, keys,
>    hostnames or bulk data tables extracted from the client, or captures of official-server
>    traffic.
> 4. I acted in the role stated in my commit (`spec`, `review` or `implementation`). If I
>    acted as an implementer, I worked only from `docs/specs/` and general knowledge, and I did
>    not consult dirty-room material for the subsystem concerned.
> 5. I have disclosed any use of AI tools with an `Assisted-by:` trailer, and I take
>    responsibility for the result as if I had written it myself.

Changes to this text get a new version number. Contributors are asked to re-confirm
the new version on their next contribution.

---

## 10. Contamination incidents

If dirty material reaches a clean-room path, or a clean-room session or person is exposed to
dirty material for the subsystem they are implementing:

1. **Stop** work on the affected code and tell a maintainer.
2. **Record** what was exposed, to whom, and which files may be affected, in
   `docs/decisions/` (no copies of the material itself).
3. **Revert** the affected commits. Do not rewrite published history; revert forward.
4. **Reimplement** from the spec by a different implementer (or a fresh AI session) who was
   not exposed.
5. **Fix the cause**: if a spec was missing a fact, add it through the normal lifecycle.
