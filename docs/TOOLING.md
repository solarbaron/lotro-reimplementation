<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tooling

**Status:** founding document, written 2026-10-03. **Owner:** project lead. **Audience:** everyone,
in both zones.

This document answers one question: *which tools do we need, and how do we set them up, so that a
small AI-assisted team can rebuild a game client compatible with LOTRO data files as efficiently as
possible without breaking the clean-room boundary?*

It covers what each tool is for, which zone may use it, its license, how to install it on Gentoo,
and when we need it. It ends with a day-1 checklist (§10) and a table of verified prior art (§9).

> This page is **clean-room safe**. It names tools and processes only. It contains no format
> facts, addresses, symbol names or decompiler output, so implementers may read it. Format facts
> belong in `docs/specs/`. Raw research belongs in `research/`, which implementers must not open.

---

## Contents

- [§0 How to read this document](#0-how-to-read-this-document)
- [§1 Provenance](#1-provenance)
- [§2 Ground rules that constrain tool use](#2-ground-rules-that-constrain-tool-use)
- [§3 (A) Build, test and development tools](#3-a-build-test-and-development-tools)
- [§4 (B) Data-format research (dirty room)](#4-b-data-format-research-dirty-room)
- [§5 (C) Static analysis of the client (dirty room, gated)](#5-c-static-analysis-of-the-client-dirty-room-gated)
- [§6 (D) Dynamic analysis under Wine/Proton (dirty room, gated)](#6-d-dynamic-analysis-under-wineproton-dirty-room-gated)
- [§7 (E) Network tooling](#7-e-network-tooling)
- [§8 (F) Claude Code setup](#8-f-claude-code-setup)
- [§9 Verified prior art and how we use it](#9-verified-prior-art-and-how-we-use-it)
- [§10 Recommended day-1 setup](#10-recommended-day-1-setup)
- [§11 Disk and directory layout for research artefacts](#11-disk-and-directory-layout-for-research-artefacts)
- [§12 Open questions](#12-open-questions)

---

## 0. How to read this document

### 0.1 Zones {#zones}

| Zone | Who | What they may touch |
|------|-----|---------------------|
| **clean** | implementers (people and Claude sessions) | `crates/`, `apps/`, `fuzz/`, `tools/`, `docs/` (including `docs/specs/`) |
| **dirty** | spec writers | the above **plus** `research/`, third-party LOTRO/AC/DDO source trees, game binaries, captures, decompiler output |
| **both** | anyone | general tools that never handle dirty material by themselves (compilers, linters, docs) |

A tool marked **dirty** can still be installed on a shared workstation. What matters is the
*session*. A person or Claude session that has looked at dirty output must not then write code in
the clean zone during that session. §8 explains how we enforce this.

### 0.2 Priority codes {#priority}

| Code | Meaning |
|------|---------|
| **now** | needed on day 1 (milestone M0) |
| **M1** | asset browser: container complete, string tables, textures, icons, `dattool` complete |
| **M2** | mesh and material viewer on wgpu |
| **M3** | terrain and landblock world walker (offline) |
| **M4** | characters and animation |
| **M5** | UI layouts, skins and the Lua 5.1 plugin host |
| **M6** | audio and player music |
| **M7** | own test server and own protocol (network session layer) |
| **opt** | optional; only if a concrete question needs it |
| **gated** | needs an explicit, recorded project-lead decision first (see §2) |

Milestones follow `docs/ROADMAP.md`, which holds the exit criteria for each one.

### 0.3 Install-hint notation {#install-hints}

- `cat/pkg` is an atom in `::gentoo`.
- `cat/pkg (overlay)` is an atom in a named overlay that this workstation already has
  (`pentoo`, `guru`, `bentoo` were all checked on 2026-10-03).
- `cargo install --locked X`, `uv tool install X` or `pip` mean the tool is not packaged.
- "upstream" means a release archive or a source build from the project's own site.

### 0.4 Fact status {#fact-status}

**Confirmed** means checked on the primary workstation (the owner's Gentoo machine) or in
the user's install on 2026-10-03. Facts about installed packages and disks (P1, P3, P4,
P19) describe that machine only; on another machine run `tools/dev-setup.sh` instead.
**Reported** means taken from the research notes or upstream documentation and not re-checked here.

---

## 1. Provenance

| # | Fact used in this document | Source(s) | Status |
|---|----------------------------|-----------|--------|
| P1 | Rust 1.97.1 is the Gentoo system toolchain (`dev-lang/rust`, USE `clippy rustfmt`, no `rust-analyzer`, no `rust-src`); `rustup` is not installed | `rustc --version`, `/var/db/pkg`, observed 2026-10-03 | Confirmed |
| P2 | Workspace MSRV is 1.90 (`apps/explorer`: 1.95), edition 2024; `rust-toolchain.toml` pins an exact version (1.99.0 as of 2026-10-04) + rustfmt + clippy, used by CI | repo `Cargo.toml`, `rust-toolchain.toml`, `.github/workflows/ci.yml` | Confirmed |
| P3 | Ghidra 12.1.3 is installed from `pentoo` at `/usr/share/ghidra`; OpenJDK 21 is the default JVM | `application.properties`, `java -version`, observed 2026-10-03 | Confirmed |
| P4 | Wireshark/tshark 4.6.7 is installed **without Lua** (`-Lua` in `tshark -v`) but with GnuTLS; 4.6.9 is available in `::gentoo` | `tshark -v`, portage tree, observed 2026-10-03 | Confirmed |
| P5 | wine-staging 11.19 (selected), gdb 17, `uv`/`uvx` and docker are installed. ImHex, RenderDoc, apitrace, Frida, mdBook, nextest, cargo-fuzz and cargo-deny are **not** installed yet | `command -v`, observed 2026-10-03 | Confirmed |
| P6 | Package atoms and versions in §3–§7 (`media-gfx/renderdoc-1.36-r1`, `dev-debug/apitrace-13.0`, `app-editors/imhex-1.38.1` (guru), `dev-util/frida-tools-14.10.4` (pentoo), `dev-util/cargo-fuzz-0.13.2` (bentoo), `dev-util/cargo-deny-0.20.2` (bentoo), `dev-util/cargo-nextest-0.9.144`, `app-text/mdbook-0.5.4`, `dev-util/reuse-6.2.0`, `app-emulation/dxvk-3.1`, `net-proxy/mitmproxy-12.2.3`, `dev-util/rustup-1.29.0`, `dev-python/kaitaistruct-0.11`, others) | `/var/db/repos/*`, observed 2026-10-03 | Confirmed |
| P7 | No Gentoo package exists in the configured repos for Tracy, cargo-about, cargo-insta, BinDiff, ghidriff, ScanCode or the Kaitai compiler | `/var/db/repos/*`, observed 2026-10-03 | Confirmed |
| P8 | The install ships a 64-bit client in `x64/` **and** a 32-bit PE client in the install root | `file`, observed in user install 2026-10-03 | Confirmed |
| P9 | The install has a `backup/` directory holding the previous launcher-era files, including the 32-bit client and the graphics-core DLLs. It holds **no** `x64/` subdirectory | `ls`, observed in user install 2026-10-03 | Confirmed (corrects `engine-client-re.md` §13, which reported `backup\x64\`) |
| P10 | The client is MSVC-built, unpacked, and carries MSVC RTTI, so RTTI-based class recovery applies | `re-tooling.md` §0 | Reported |
| P11 | OOAnalyzer (Pharos) handles 32-bit MSVC binaries only; the Kaiju release `260910` supports Ghidra 12.1.3 and contains the OOAnalyzer importer | `re-tooling.md` §2; `resources.json` (Kaiju, Pharos) | Reported |
| P12 | Ghidra MCP server features, licenses, ports and Ghidra-version pins (§5.3) | `resources.json` entries for pyghidra-mcp, bethington/ghidra-mcp, GhidrAssistMCP, themixednuts GhidraMCP, LaurieWired GhidraMCP; `re-tooling.md` §9 | Reported |
| P13 | x64dbg is unreliable under Wine; a Windows VM is the reliable route | `re-tooling.md` §3 (WineHQ reports) | Reported |
| P14 | DXVK honours RenderDoc capture, shader-dump and HUD environment variables | `re-tooling.md` §4 (DXVK source reading) | Reported |
| P15 | Frida works against Wine processes by running the Windows `frida-server.exe` inside the same prefix | `re-tooling.md` §3 (Freat project) | Reported |
| P16 | Captures from official servers must stay private; no official login/GLS implementation; no captures in repos or issues | `legal.md` §7 Tier X, §8.C; `network.md` §3 | Reported |
| P17 | Default dirty-room posture is black-box; binary disassembly only if explicitly authorized, by a lawful licensee, preferably in an EU/UK jurisdiction, confined to interoperability needs | `legal.md` §8.A | Reported |
| P18 | Prior-art licenses and recommended uses (§9) | `research/resources.json` (all 98 entries `exists: true`, verified 2026-10-03) | Reported |
| P19 | *Primary workstation only:* `/home` had ~20 GB free (96 % used) and a second disk (`~/nvme`, XFS with reflink) ~216 GB free. Other machines differ: put `$TESSERA_RE_DIR` (§11) and the build cache on whatever large private disk they have | `df -h`, observed 2026-10-03; reflink from `re-tooling.md` §0 | Confirmed (free space) / Reported (reflink) |
| P20 | Analogue projects all ship per-format CLI tools early, use schema-driven codegen, gate real-data tests on an env var, and fuzz parsers | `analogues.md` §4 | Reported |
| P21 | REA facts from the 2026-10-11 evaluation (rea-agents 6.4.0): licence, tool catalogue, no install hooks or network egress on the Ghidra path, npm signatures, per-session import and re-analysis, startup deadline, unpaged listings, state outside the repository (§5.1, §5.3, §5.4) | evaluation of 2026-10-11 in `~/nvme/rea-eval`, summarised in [decision 0033](decisions/0033-rea-for-dirty-room-analysis.md) | Reported (until the decision-0033 pilot) |

---

## 2. Ground rules that constrain tool use

These rules come from `CLAUDE.md` and the legal research. They decide which tools we use and how.

1. **The install is read-only.** No tool may write to, rename, lock or "repair" anything in the
   game directory. Read tools in place, or copy to `$TESSERA_RE_DIR/snapshots/` first (§11).
2. **Nothing dirty enters the repo.** That includes decompiler output, Ghidra/BinExport/BSim
   databases, symbol or RVA lists, RenderDoc `.rdc` files, DXVK shader dumps, pcaps from official
   servers, extracted assets, and seed corpora built from real `.dat` files.
3. **Black-box first.** Reading data files, observing our own traffic, frame captures and the
   client's own logging switches are the default research methods. Each one is "observe, study and
   test while lawfully running" (EU Software Directive Art. 5(3)).
4. **Disassembly and decompilation are gated.** Every tool in §5, and the invasive tools in §6
   (injection, debugger attach, memory reading), need a recorded project-lead authorization before
   first use. Without it they stay installed and unused. The authorization should name the person,
   their jurisdiction, the client build, and the interoperability question being answered.
5. **Never against live servers with a modified client.** No debugger, injector or memory tool may
   touch a client connected to official servers. Doing so breaks the ToS and anti-cheat rules and
   is Tier X in our legal policy.
   Decision 0034 (2026-10-11, M8 in scope) keeps this rule unchanged. Passive capture of the
   owner's own traffic with the unmodified client is allowed for M8 (§7).
6. **Outputs are prose specs.** The only thing a dirty-room tool produces for the project is a
   `docs/specs/*.md` page, written in our own words, with a provenance table.

---

## 3. (A) Build, test and development tools

All of these are **both**-zone tools. They never handle dirty material on their own. Real-data
tests read `TESSERA_GAME_DIR` and skip when it is unset.

### 3.1 Table {#a-table}

| Tool | Purpose | Zone | License | Gentoo install hint | Priority |
|------|---------|------|---------|---------------------|----------|
| Rust (system) | stable compiler, `cargo fmt`, `cargo clippy` | both | MIT / Apache-2.0 | `dev-lang/rust` (installed 1.97.1). Add USE `rust-src rust-analyzer` for IDE support | now |
| rustup | runs a nightly toolchain next to the system one (cargo-fuzz needs it) and honours `rust-toolchain.toml` | both | MIT / Apache-2.0 | `dev-util/rustup`, then `rustup toolchain install nightly --profile minimal` | now |
| rust-analyzer | IDE and LSP for every session, including Claude Code's LSP integration | both | MIT / Apache-2.0 | USE `rust-analyzer` on `dev-lang/rust`, or `rustup component add rust-analyzer` | now |
| cargo-nextest | fast parallel test runner with per-test timeouts (real-data tests are slow) | both | MIT / Apache-2.0 | `dev-util/cargo-nextest` | now |
| cargo-fuzz (libFuzzer) | one fuzz target per parser in `fuzz/` (required by `CLAUDE.md`) | both | MIT / Apache-2.0 | `dev-util/cargo-fuzz` (bentoo) or `cargo install --locked cargo-fuzz`; run with `cargo +nightly fuzz` | now |
| AFL++ / cargo-afl | second fuzzing engine for long campaigns | both | Apache-2.0 | `app-forensics/aflplusplus`; `cargo install --locked cargo-afl` | M1 |
| proptest / arbitrary | property tests and structure-aware fuzz input | both | MIT / Apache-2.0 | crate dependencies (`proptest` is already in the workspace) | now |
| insta + cargo-insta | snapshot tests of **derived metadata only** (counts, ID lists, hashes), never decoded content | both | Apache-2.0 | crate + `cargo install --locked cargo-insta` | M1 |
| cargo-llvm-cov | coverage, used to steer fuzzing at uncovered parser branches | both | MIT / Apache-2.0 | `dev-util/cargo-llvm-cov` (bentoo) | M1 |
| cargo-deny | license allow-list (GPL-3.0-or-later compatible), advisories, duplicate and banned crates | both | MIT / Apache-2.0 | `dev-util/cargo-deny` (bentoo) | now |
| cargo-about | generates the third-party license notice for binary releases | both | MIT / Apache-2.0 | `cargo install --locked cargo-about` (not packaged) | M2 |
| REUSE tool | checks SPDX headers and license files across the repo | both | GPL-3.0-or-later | `dev-util/reuse` | now |
| gitleaks | pre-commit and CI secret scan; add rules for key-like blobs | both | MIT | `dev-vcs/gitleaks` (bentoo or pentoo) | now |
| ScanCode Toolkit | license and similarity scan of AI-assisted code against known sources | both | Apache-2.0 | `uv tool install scancode-toolkit` | M1 |
| sccache, mold | faster rebuilds and linking on a 24-core box | both | Apache-2.0 / MIT | `dev-util/sccache`, `sys-devel/mold` | now |
| hyperfine, criterion | CLI and micro benchmarks for the container reader and decoders | both | MIT / Apache-2.0 | `app-benchmarks/hyperfine`; `criterion` dev-dependency | M1 |
| mdBook (+ mermaid, linkcheck) | renders `docs/` and `docs/specs/` as a browsable book with stable anchors | both | MPL-2.0 | `app-text/mdbook`; `cargo install --locked mdbook-mermaid mdbook-linkcheck` | M1 |
| Tracy | frame and zone profiler for the renderer and streaming; Rust side through `tracing-tracy` | both | BSD-3-Clause (viewer); MIT / Apache-2.0 (crate) | not packaged: build the upstream viewer with CMake, matched to the protocol version the `tracy-client` crate expects | M2 |
| RenderDoc | frame debugger for **our own** wgpu output | both (when capturing Tessera) | MIT | `media-gfx/renderdoc` | M2 |
| wgpu validation | wgpu debug/validation flags in dev builds plus Khronos validation layers | both | MIT / Apache-2.0; Apache-2.0 | `media-libs/vulkan-layers`, `dev-util/vulkan-tools` (vulkaninfo), `dev-util/spirv-tools` | M2 |
| lavapipe | software Vulkan for headless golden-image render tests in CI | both | MIT | `media-libs/mesa` with `VIDEO_CARDS="... lavapipe"` | M2 |
| hexyl | quick terminal hexdumps of our own synthetic fixtures | both | MIT / Apache-2.0 | `sys-apps/hexyl` (guru) | now |
| difftastic | structural diffs of JSON dumps produced by `dattool` | both | MIT | `dev-util/difftastic` | M1 |

### 3.2 Notes {#a-notes}

- **Two toolchains, one rule.** The system Rust builds and tests everything. Nightly from rustup
  is used only for `cargo +nightly fuzz`. Nothing in the workspace may need nightly features.
- **Fuzz corpora.** Seeds may be harvested locally from real `.dat` files to reach deep code
  paths, but they live under `fuzz/corpus/` (already git-ignored) and are never committed. Commit
  only minimised crashers that our own synthetic writer can regenerate.
- **CI baseline.** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run --workspace`, `cargo deny check`, `reuse lint`, `gitleaks`, a 60-second
  `cargo fuzz` smoke run per target, and `mdbook build`. CI also runs a **dirty-content guard**: a
  script that rejects committed `.dat`/`.datx`/`.rdc`/`.pcap*` files, binary blobs above a size
  limit, Ghidra-style auto-names (`FUN_`, `DAT_`, `LAB_`), and long hex address lists in `crates/`,
  `apps/`, `tools/` and `docs/specs/`.
- **wgpu debugging.** Keep validation on in debug builds. Run with the Vulkan backend under
  RenderDoc when a frame looks wrong. Use lavapipe for deterministic golden images.

---

## 4. (B) Data-format research (dirty room)

This is the main research loop. Most format knowledge can be recovered **without** looking at the
client binary: read the archives, form a hypothesis, check it on thousands of records, then write
it up. Every artefact below is dirty until it has been turned into a spec, **except** the committed
`tools/imhex/*.hexpat` and `tools/kaitai/*.ksy` files, which are clean-zone artefacts written *from*
specs (§4.3).

### 4.1 Table {#b-table}

| Tool | Purpose | Zone | License | Gentoo install hint | Priority |
|------|---------|------|---------|---------------------|----------|
| ImHex | interactive hex editor with a pattern language, entropy and diff views; the main viewer for archive and record layouts | dirty when pointed at game files | GPL-2.0 | `app-editors/imhex` (guru, `~amd64`) | now |
| `plcli` (PatternLanguage CLI) | runs a `.hexpat` headlessly and emits JSON, for differential tests against `dattool` output | both | LGPL-2.1 | build upstream WerWolv/PatternLanguage with CMake | M1 |
| Our `.hexpat` patterns (`tools/imhex/`) | executable version of each spec; drafted in scratch during research, committed only once written from the spec | clean (committed) / dirty (drafts) | GPL-3.0-or-later (ours) | in repo | now |
| Kaitai Struct compiler + Web IDE | second machine-readable rendering of specs (`.ksy`) with visual trees; optional Rust or Python output for probes | both | GPL-3.0 (compiler); MIT (runtimes) | compiler: upstream JVM zip; runtime: `dev-python/kaitaistruct` | M1 |
| Our `.ksy` files (`tools/kaitai/`) | same role as the hexpat files; one per spec page | clean (committed) | GPL-3.0-or-later (ours) | in repo | M1 |
| 010 Editor | commercial hex editor with `.bt` templates; only if a researcher already owns it | dirty | proprietary | upstream Linux build | opt |
| Python probes | one-off throwaway scripts (stdlib `struct`, `zlib`, `mmap`) that test a hypothesis across a whole archive | dirty | n/a (never committed) | system Python + `uv run` in the session scratchpad | now |
| `tessera-dattool` (ours, `apps/dattool`) | `detect`, `info`, `ls`, `stat`, `cat`, `strings`, `show`, `census`, `extract`, `validate`, `snapshot`, `diff` against the user's install (§4.4); becomes the oracle that specs are checked against | clean (code) / output on real data is dirty | GPL-3.0-or-later | `cargo run -p tessera-dattool --` | now |
| `dattool manifest` / `dattool diff` (planned) | per-archive manifest (ID, version, iteration, sizes, flags, hash of decoded bytes) in SQLite; manifest-to-manifest diff by record type | clean (code) | GPL-3.0-or-later | planned subcommands | M1 |
| reflink snapshots | near-free copies of the archive set before each game patch, so we can diff patch N against N+1 | dirty (data) | n/a | `cp --reflink=always` on a reflink-capable file system (XFS, Btrfs) holding `$TESSERA_RE_DIR` | M1 |
| diffoscope, sqlitebrowser | deep diffs of two snapshots; browsing manifests | dirty (on real data) | GPL-3.0-or-later; MPL-2.0 / GPL-3.0 | `dev-util/diffoscope`, `dev-db/sqlitebrowser` | M1 |
| Steam News API | labels each snapshot with the public update name ("Update X.Y") | both | n/a (public web API) | `curl` | M1 |
| LotroCompanion `delta-lotro-dat-utils` jar | **black-box oracle only**: run it on our own install and compare counts/IDs with `dattool`; never decompile, port or ship it | dirty | Apache-2.0 at distribution level (weak, see §9) | upstream zip from SourceForge; system JDK | opt |

### 4.2 Workflow {#b-workflow}

1. **Observe.** Open the archive in ImHex or write a probe. Record each hypothesis with the archive
   name, offsets and record counts in a dirty note in `research/notes/`.
2. **Confirm at scale.** A hypothesis counts as Confirmed only when a probe checks it against every
   record of that type in the user's install, not just a handful of samples.
3. **Cross-check against published sources.** Use the prior art in §9 as *second witnesses*. Any
   disagreement becomes an `> **OPEN:**` callout in the spec.
4. **Write the spec.** `docs/specs/<topic>.md` with byte tables, invariants and a provenance table.
5. **Encode the spec.** A spec writer, or better an implementer working only from the spec, writes
   `tools/imhex/<topic>.hexpat` and, if useful, `tools/kaitai/<topic>.ksy`.
6. **Differential test.** At M1, `plcli` and `dattool` both emit JSON for the same synthetic
   fixture, and CI diffs the two. On a developer box with `TESSERA_GAME_DIR` set, run the same
   comparison against real records and report pass/fail only.

### 4.3 Why patterns live in `tools/` {#b-patterns}

`tools/` is in the clean zone. A pattern written from a spec is a second, independent
implementation of that spec. When it disagrees with the Rust parser on the same bytes, either the
spec is ambiguous or one of the implementations is wrong. That disagreement is exactly what we
want to find. A pattern drafted during research is dirty and stays in the scratchpad until the spec
exists.

### 4.4 Record inspection and censuses with `dattool` {#b-dattool}

Every `tessera-dattool` command opens the archives read-only and never writes into the install;
`--json` gives machine-readable output for each. Two commands work on decoded records rather
than on the container:

| Command | What it does |
|---|---|
| `show <dir\|archive> <did> [--max-list N]` | Decodes one record and prints its fields: terrain type tables, terrain-scene and scene-choice tables, scenes, water and fog-zone tables, the sky description, day descriptions, game time, LOD classes, camera states, collision meshes and cell geometry, region descriptors with their eight table slots (and whether each named table is in the input), and landblock records (terrain and ceiling trees with a structural walk, ceilings, the box list, surface layers, light maps). Text output lists at most `N` rows per list (default 20, `0` for all). JSON lists every row, but gives property sets and the embedded render mesh of cell geometry as counts, and summarises landblock info, block maps, block-map descriptors and distant-landscape sectors as counts, as the text does. Other decodable records get a one-line summary (`cat` also names the decoded type, and points at `show` where it prints the fields) |
| `census <dir\|archive> [--prefix 0xHH]... [--max-failures N]` | Decodes every record of the selected DID prefixes with its directory stamp and the property dictionary, and counts records, decoded and failed records (by failure category), payload bytes and a per-kind item count. A 0x01 record counts as decoded only with its embedded render mesh (`collision.md` §5.7). It also walks every collision tree of the selected records and checks its keys (`collision.md` §3.5, CO4): 0x66 records, cell physics meshes, the levels of the 0x01 embedded render meshes, 0x06 mesh levels and landblock terrain and ceiling trees; mesh levels with §3.5's renumbering are counted, not failed. Without `--prefix` it covers the record kinds of the M4.5 decode criterion (the new world records and the M2 material family); `--prefix 06` adds the meshes (the criterion's mesh levels; large, so not in the default) and `--prefix 80` the landblock family. Exit status 1 when a record does not decode or a tree fails its walk or key check, so it serves as the decode criterion of a milestone |

The property dictionary is taken from the input, else from the `gamelogic` archive next to a
single-archive input, else from the game directory (`--game-dir`, `$TESSERA_GAME_DIR`,
detection). As §4.1 says for the tool as a whole, their output on real data is dirty-room
material.

---

## 5. (C) Static analysis of the client (dirty room, gated)

> **Legal gate.** Everything in this section is **gated** (§2 rule 4). The project's default is
> black-box research. US contributors who accepted the game's EULA are exposed in contract
> (*Davidson v. Jung*). EU Software Directive Art. 6 permits decompilation only when it is
> indispensable for interoperability and the information is not otherwise available, and Art. 8
> makes contrary contract terms void. Use these tools only for a written, specific
> interoperability question that data-file study (§4) and black-box observation (§6, §7) could not
> answer. **Output is prose only.** Ghidra projects, BinExport files, BSim databases and
> decompiler text never leave `$TESSERA_RE_DIR/`.

### 5.1 Table {#c-table}

| Tool | Purpose | Zone | License | Gentoo install hint | Priority |
|------|---------|------|---------|---------------------|----------|
| Ghidra 12.1.x | disassembly and decompilation; built-in PDB/Microsoft demangler, FunctionID (MSVC databases), BSim, Version Tracking, Ghidra Server, PyGhidra, Debugger | dirty | Apache-2.0 | `dev-util/ghidra` (pentoo; installed 12.1.3; needs `eselect gradle` < 9 to rebuild) | gated |
| RecoverClassesFromRTTIScript (bundled) | MSVC RTTI class-hierarchy, vftable and ctor/dtor recovery | dirty | Apache-2.0 | ships with Ghidra | gated |
| CERT Kaiju (OOAnalyzer importer, FnHash, GhiHorn) | imports Pharos/OOAnalyzer class-recovery JSON; function hashing across builds | dirty | BSD (SEI custom) | upstream release zip matching the Ghidra version (`260910` targets 12.1.3); File → Install Extensions | gated |
| Pharos / OOAnalyzer | object-oriented recovery for **32-bit** MSVC code; run it on the 32-bit client that still ships (P8), then carry names to x64 with BSim / Version Tracking | dirty | BSD (SEI custom) | official Docker image | gated, opt |
| Ghidra-Cpp-Class-Analyzer | older C++ class recovery | — | MIT | **skip**: archived 2023, targets Ghidra 10 | — |
| BSim + Version Tracking (built-in) | carry analyst annotations across builds and from 32-bit to 64-bit | dirty | Apache-2.0 | built-in; BSim database under `$TESSERA_RE_DIR` | gated |
| BinExport (Ghidra extension) | exports a program to the BinExport2 protobuf for BinDiff | dirty | Apache-2.0 | build from source against the installed Ghidra (`GHIDRA_INSTALL_DIR=/usr/share/ghidra`); the last binary release targets Ghidra 11.0.3 | gated, opt |
| BinDiff 8 | function matching and patch diffing between client builds (Ghidra support is Tier 4) | dirty | Apache-2.0 | upstream `.deb`, unpacked to `/opt` | gated, opt |
| ghidriff | command-line patch diff producing Markdown/JSON/HTML; the cheapest way to see what changed in a new client build | dirty | GPL-3.0-only (we only run it) | `uv tool install ghidriff` (needs `GHIDRA_INSTALL_DIR` and a JDK) | gated |
| LIEF | scripted PE header, import and export inspection (header-level, not decompilation) | dirty | Apache-2.0 | `dev-util/lief` (pentoo) or `uv tool install lief` | M1 |
| capa, Detect-It-Easy | identify compilers, packers and statically linked libraries | dirty | Apache-2.0; MIT | `uv tool install flare-capa`; DiE upstream | opt |
| rizin / Cutter | lightweight alternative disassembler | dirty | LGPL-3.0 / GPL-3.0 | `dev-util/rizin`, `dev-util/cutter` | opt |
| IDA Pro / Binary Ninja | commercial disassemblers; only if a researcher already owns one | dirty | proprietary | upstream | opt |
| REA (rea-agents 6.4.0) | MCP/CLI front end to Ghidra headless: decompile, typed xrefs, call paths, value traces, Evidence records | dirty | MIT | pinned npm install into `$TESSERA_RE_DIR/tools/rea` with `--ignore-scripts` ([§5.4](#c-rea)) | gated, opt ([decision 0033](decisions/0033-rea-for-dirty-room-analysis.md)) |

**Patch-diff source.** The launcher's `backup/` directory keeps previous copies of some binaries,
but on 2026-10-03 it held only the 32-bit client and DLLs, with no `x64/` copy (P9). So we keep our
own reflink snapshot of `x64/` before each patch (§11).

### 5.2 Static-analysis workflow (once authorized) {#c-workflow}

1. Snapshot the binaries to `$TESSERA_RE_DIR/snapshots/<build>/`. Never open them in place.
2. Import into a Ghidra project in `$TESSERA_RE_DIR/ghidra/`. Run auto-analysis with the RTTI analyzer
   and the matching MSVC FunctionID databases, then the class-recovery script.
3. Optionally run OOAnalyzer on the 32-bit build and port results to x64 with BSim.
4. For each new build, run `ghidriff old new` and BSim/Version Tracking to carry annotations
   forward.
5. Answer the one interoperability question that justified the session, then write the answer as a
   spec section in functional terms: layout, enum meaning, state machine. No addresses, no
   pseudo-code, no symbol dumps.

### 5.3 Ghidra MCP servers compared {#c-mcp}

Model Context Protocol (MCP) servers let a dirty-room Claude session query Ghidra directly. All of
them can return decompiler output, so **none may be configured for a clean session** (§8).

| Server | License | Ghidra pin | How it runs | Strengths | Weaknesses | Verdict |
|--------|---------|------------|-------------|-----------|------------|---------|
| **clearbluejar/pyghidra-mcp** | Apache-2.0 | uses the local install through `GHIDRA_INSTALL_DIR` | headless PyGhidra process; stdio or streamable HTTP on `127.0.0.1:8000/mcp`; optional `--gui` | headless-first, so one long-running server removes the per-query Ghidra start-up; recursive project import; batch decompile; semantic and literal code search; regex symbol search; about 40 tools | its indexes can lag behind Ghidra's own analysis; write tools (rename, comment) are exposed | **primary** for batch and question-driven work |
| **bethington/ghidra-mcp** | Apache-2.0 | built against 12.1.3 | Ghidra plugin (HTTP on 8089) plus Python bridge; stdio or streamable HTTP; Docker/headless mode | lazy tool groups keep the context small; **program selectors** (`GHIDRA_MCP_REQUIRE_PROGRAM_SELECTORS=1`) stop cross-program mistakes when the exe and DLLs are open together; read-only allow-list mode; cross-version function-hash matching; optional debugger bridge | very large write-capable surface by default; its naming-convention enforcer can rewrite names | **secondary**, interactive GUI work, always in read-only allow-list mode |
| **jtang613/GhidrAssistMCP** | MIT | Ghidra 11.4+ (tested on 12.1) | in-Ghidra plugin on `127.0.0.1:8080` (`/mcp` streamable HTTP, `/sse`); headless through an `analyzeHeadless` pre-script | official Java MCP SDK; resources by URI; risky tools (scripts, file import, export) **off by default**; batch rename | building from source needs Java 25 (use the release zip) | good alternative if pyghidra-mcp's indexing gets in the way |
| themixednuts/GhidraMCP | MIT | 0.9.0 pins **12.1.4**, newer than the installed 12.1.3 | in-Ghidra HTTP on 8080, no Python bridge | 14 consolidated tools, including Version-Tracking sessions and mapping a data type onto bytes (useful for checking a layout guess) | version lock; port clash with GhidrAssistMCP | consider after upgrading Ghidra |
| LaurieWired/GhidraMCP | Apache-2.0 | 11.3.2 | plugin + Python bridge on 8080 | the original; simple | stale since 2025-06; no struct tools, no batch, no headless | **avoid** |
| morluto/rea (`rea-agents` 6.4.0) | MIT | uses the local install through `GHIDRA_INSTALL_DIR` (accepts 12.1.3 through a shim, [§5.4](#c-rea)) | stdio MCP server or CLI; imports the target into a temporary project with `-readOnly -deleteProject` per session | decompile, typed xrefs, callers and callees, call paths, value traces across calls, function dossiers; Evidence records with the target's SHA-256 (P21) | many out-of-scope tools (capture, browser, Android, firmware, reconstruction); `rea setup` registers itself at user scope and installs a skill; very high release churn | **limited** ([decision 0033](decisions/0033-rea-for-dirty-room-analysis.md)): re-imports and re-analyses per session (~21 min or more), cannot open the shared project, unpaged listings; owner-started dedicated sessions only |

Settings that apply to every server: bind to `127.0.0.1` only, keep script-execution tools
disabled, and prefer read-only modes, because the job is extracting facts, not annotating a shared
project. No server is ever registered at user, project (`.mcp.json`) or local scope; a dirty
session attaches it with `--strict-mcp-config` (`CLEAN_ROOM.md` §7 rule 4).

### 5.4 REA setup (decision 0033) {#c-rea}

Added 2026-10-11 by [decision 0033](decisions/0033-rea-for-dirty-room-analysis.md) (Proposed).
While that record is Proposed, the only use is the owner's pilot (f). Everything below lives
under `$TESSERA_RE_DIR` on the machine, is dirty, and is not carried by git.

a. **Install.** `npm install --prefix $TESSERA_RE_DIR/tools/rea --ignore-scripts --no-fund
   --no-audit rea-agents@6.4.0`, with the npm cache on the bulk drive
   (`npm_config_cache=~/nvme/rea-eval/npm-cache`). Then check the package integrity
   (`sha512-Vj1zWfSlna5bphGoaoI/TcVBuYvze9u+OyPC3Shz8YkBqaCTqB4COnnRoX7XDwMpwI24ZlFEel2yEj0k8LTWEQ==`),
   run `npm audit signatures`, and keep the resulting lock as `package-lock.reviewed.json`
   beside the install (SHA-256
   `2dba038b1cbc097a3ff153f7346e09436e5233fad7b6dfd319807fb6ada1fa95`). No global install,
   nothing on `PATH`; REA is called by absolute path only. Never run `install.sh`,
   `npx rea-agents`, `rea setup`, `rea update` or `rea uninstall`, and never install the
   `reverse-engineer-anything` skill. A version bump repeats this review and adds a row to
   the decision's change log.
b. **Ghidra shim** at `$TESSERA_RE_DIR/tools/ghidra-shim`: top-level symlinks to
   `/usr/share/ghidra`, and `support/` as symlinks plus one executable copy of
   `analyzeHeadless` (the Gentoo package ships it non-executable). The system install stays
   untouched.
c. **`mcp-rea.json`** beside the install starts `<install>/node_modules/.bin/rea --mcp` with
   `HOME` and `TMPDIR` set to `$TESSERA_RE_DIR/work/rea-<key>/{home,tmp}` (no path element
   starting with a dot, or REA falls back to `/tmp`), `REA_ANALYSIS_PROVIDER=ghidra`,
   `GHIDRA_INSTALL_DIR` set to the shim, `JAVA_HOME` set to the resolved full JDK 21,
   `REA_GHIDRA_STARTUP_TIMEOUT_MS=5400000` and `GHIDRA_HEADLESS_MAXMEM=10G`.
d. **`settings-rea.json`** beside the install sets `permissions.allow` to the static native
   tools listed in decision 0033 item 5 and `permissions.deny` to the rest of the 6.4.0
   catalogue (generated from `tools/list`). Regenerate both lists on any version bump.
e. **Start** the dedicated dirty session in a dirty worktree or scratch directory, with
   `MCP_TOOL_TIMEOUT` set to match the startup deadline:
   `claude --strict-mcp-config --mcp-config $TESSERA_RE_DIR/tools/rea/mcp-rea.json --settings $TESSERA_RE_DIR/tools/rea/settings-rea.json`.
   Never start the milestone workflow or a clean agent from it. One REA session at a time; it
   counts as one of the at most three heavy jobs for its whole life.
f. **Pilot and cleanup.** The pilot runs in `$TESSERA_RE_DIR/work/rea-pilot` and records
   `rea doctor --provider ghidra --format json` health with the shim, cold-start wall time,
   peak RSS and temporary disk use, whether a saved analysis snapshot reopens in a new process
   without re-analysis, and one in-scope benchmark question answered with both REA and
   `analyzeHeadless`. After every session (pilot or not), delete `work/rea-<key>` and check
   that `/tmp` holds no `rea-*` leftovers. If the decision is Rejected, delete
   `$TESSERA_RE_DIR/tools/rea`, the shim and `~/nvme/rea-eval/prefix`.

---

## 6. (D) Dynamic analysis under Wine/Proton (dirty room, gated)

There are two kinds of tool here.

- **Passive observation** watches what the unmodified client does: Wine's own trace channels, the
  client's built-in logging switches, the DXVK HUD and logs, and RenderDoc/apitrace frame captures.
  This is ordinary "observe, study, test" and may be used once a spec question needs it.
- **Invasive tools** inject code, attach debuggers or read process memory: Frida, x64dbg, gdb
  attach, memory scanners. They are **gated**, and they must never touch a client connected to
  official servers (§2 rule 5).

> **OPEN:** the client needs a server connection to get past login. We have not yet established
> how much useful, legal dynamic observation is possible without connecting (start-up, settings,
> renderer selection, the login screen). Settle this before planning any invasive work.

### 6.1 Table {#d-table}

| Tool | Purpose | Kind | Zone | License | Gentoo install hint | Priority |
|------|---------|------|------|---------|---------------------|----------|
| Wine (staging) | runs the client; a separate prefix per experiment | — | dirty | LGPL-2.1-or-later | `app-emulation/wine-staging` (11.19 selected), `app-emulation/winetricks` | M2 |
| umu-launcher / Proton | Proton runtime outside Steam, matching what most Linux players run | — | dirty | GPL-3.0 | `games-util/umu-launcher` (guru) | opt |
| `WINEDEBUG` channels | free API monitor: DLL loads, exceptions, D3D/DXGI, winsock and TLS calls | passive | dirty | (Wine) | built-in | M2 |
| Client logging switches | the client's own diagnostic and log command-line options; the list itself is a research fact and lives in `research/notes/`, not here | passive | dirty | (client) | built-in | M2 |
| strace, Wine file tracing | which archives, settings files and folders the client opens, for `docs/specs/install-layout.md` questions | passive | dirty | LGPL-2.1-or-later; (Wine) | `dev-debug/strace`; `WINEDEBUG` file channels | M1 |
| DXVK | the D3D-to-Vulkan layer Proton already uses to run the client on Linux; its HUD, log-level and shader-dump settings help answer render-spec questions (P14) | passive | dirty | zlib | `app-emulation/dxvk`, or the copy bundled with Proton/umu | M2 |
| RenderDoc (Vulkan, through DXVK) | frame captures of the **original** client: pass order, render targets, bound textures and shader stages, for `docs/specs/render.md` | passive | dirty (`.rdc` files never leave `$TESSERA_RE_DIR/`) | MIT | `media-gfx/renderdoc` | M2 |
| apitrace | API call traces, for comparing call streams between the original client and Tessera; only through ordinary loader mechanisms, **never** by placing files in the install directory (§2 rule 1) | passive | dirty | MIT | `dev-debug/apitrace` (`~amd64`) | opt |
| MangoHud / DXVK HUD | frame-time and load-time comparisons between the original client and Tessera | passive | both | MIT; zlib | `games-util/mangohud` | M3 |
| Windows VM (QEMU/KVM + libvirt) | reference environment for native-D3D captures and for any gated tool that is unreliable under Wine (P13); keeps dirty sessions off the workstation's main prefix | — | dirty | GPL-2.0 | `app-emulation/qemu`, `app-emulation/libvirt`, `app-emulation/virt-manager` | opt |
| gdb / winedbg | debugger attach to the original client | invasive | dirty | GPL-3.0-or-later; LGPL-2.1-or-later | `dev-debug/gdb` (installed 17); `winedbg --gdb` ships with Wine | gated, opt |
| scanmem / GameConqueror, PINCE, edb | read process memory of the original client | invasive | dirty | GPL-3.0; GPL-3.0; GPL-2.0 | `dev-util/scanmem`; PINCE upstream script; `dev-debug/edb-debugger` (pentoo) | gated, opt |
| x64dbg | Windows debugger; use only inside the Windows VM (P13) | invasive | dirty | GPL-3.0 | upstream release inside the VM | gated, opt |
| Frida | dynamic instrumentation by **code injection** | invasive | — | wxWindows Library Licence 3.1 | `dev-util/frida-tools` (pentoo) | **forbidden on the official client** (see the note below); allowed on Tessera's own binaries |
| rr | record and replay of **Tessera's own** processes, for hard-to-reproduce bugs | — | both | MIT / BSD-2-Clause | `dev-debug/rr` | opt |

> **Where LEGAL.md is stricter, it wins.** `docs/LEGAL.md` §8 permanently forbids
> "modifying, patching, redirecting or injecting into the official client" at every risk tier.
> So code-injection tools (Frida, DLL wrappers, hook libraries) must never be used on the
> official client, gate or no gate. Debugger attach and memory reading count as method M4 in
> `docs/CLEAN_ROOM.md` §4: they need a recorded decision naming the question, the analyst and the
> client build, and they must never touch a client that is connected to official servers.

### 6.2 Passive-observation workflow {#d-workflow}

1. Create a **separate** Wine prefix per experiment under `$TESSERA_RE_DIR/prefixes/<name>/`. Point it
   at a reflink snapshot of the install (§11), never at the live Steam copy, so that nothing the
   client or Wine writes can change the user's install.
2. Write down the one question the session answers ("which archives are opened at start-up",
   "what passes make up the login-screen frame").
3. Capture with the passive tools above. Keep raw output (`.rdc`, logs, traces) under
   `$TESSERA_RE_DIR/captures/<date>-<question>/`.
4. Reduce the result to functional facts in a spec section, with a provenance row that names the
   tool and the capture date. Raw captures are never committed, attached to issues or pasted into
   a clean session.

---

## 7. (E) Network tooling

Tessera speaks **its own** protocol (`docs/specs/transport-design.md`) to the in-process sandbox
and, from M7, to a project-owned test server. That is risk tier T2 in `docs/LEGAL.md` §8. Network
tools therefore look at **our own** traffic. Observing official login or session traffic is
M8 work. Since 2026-10-11 it is in scope by the owner's decision
([decision 0034](decisions/0034-official-server-compatibility.md) §2, `docs/LEGAL.md` §9.3)
on these terms:

- only passive captures of the owner's own sessions of the **unmodified** client, on the
  owner's machine;
- no injection, debugger or memory reading on a client connected to official servers
  (§2 rule 5);
- decryption only by methods that need none of those tools;
- captures stay under `$TESSERA_RE_DIR/captures/` and are never committed or attached
  anywhere (P16);
- specs receive field-level facts only: never key values, hostnames, account data or
  other players' data;
- never anything about anti-cheat or anti-tamper.

### 7.1 Table {#e-table}

| Tool | Purpose | Zone | License | Gentoo install hint | Priority |
|------|---------|------|---------|---------------------|----------|
| Wireshark / tshark | inspect Tessera's own protocol on loopback and on the test server | both (own traffic); dirty (any capture involving official servers) | GPL-2.0-or-later | `net-analyzer/wireshark`; rebuild with `USE=lua` to load Lua dissectors (installed 4.6.7 has no Lua, P4) | M7 |
| Our Wireshark dissector (planned) | Lua dissector for the Tessera protocol, written from `docs/specs/transport-design.md` like the hexpat files; ideally generated from the same schema as the Rust codec | both | GPL-3.0-or-later (ours) | planned under `tools/` | M7 |
| wsdf | Rust proc-macro framework for Wireshark dissectors, an alternative to Lua; may need ABI updates for Wireshark 4.6 | both | MIT / Apache-2.0 | crate | opt |
| tcpdump | headless capture on the test server | both (own traffic) | BSD-3-Clause | `net-analyzer/tcpdump` | M7 |
| `tc netem` (iproute2) | inject loss, delay, duplication and reordering to exercise the reliability layer | both | GPL-2.0 | `sys-apps/iproute2` | M7 |
| fuzz targets `net_packet_decode`, `net_endpoint` | hostile-input coverage of the packet codec (already in `fuzz/`) | both | GPL-3.0-or-later (ours) | in repo | now |
| Headless bot client (planned) | load and soak tests against our own test server, built on `crates/net` | both | GPL-3.0-or-later (ours) | planned `apps/` binary | M7 |
| mitmproxy | inspect HTTP(S) between Tessera's own launcher/test-server components | both (own endpoints only) | MIT | `net-proxy/mitmproxy` | opt |

---

## 8. (F) Claude Code setup

AI sessions follow the same zone rules as people (`docs/CLEAN_ROOM.md` §7). The setup below makes
the safe path the default one.

1. **One role per session.** `.claude/agents/` defines three subagents:
   `spec-writer` (dirty room: reads `research/`, the install and published sources, writes
   `docs/specs/` and `research/notes/`), `clean-implementer` (clean room: reads only clean paths
   and specs, writes `crates/`, `apps/`, `fuzz/`, `tools/`) and `clean-room-auditor`
   (read-only checks before merge). A session that has used `spec-writer` for a subsystem must not
   implement that subsystem.
2. **Deny rules for clean sessions.** Clean sessions should run with permission rules that block
   dirty paths and tools, for example in `.claude/settings.json`:

   ```json
   {
     "permissions": {
       "deny": [
         "Read(./research/**)",
         "Read(**/clones/**)",
         "Bash(ghidra*)",
         "Bash(analyzeHeadless*)",
         "Bash(frida*)"
       ]
     }
   }
   ```

   Spec-writing sessions run from a separate checkout or from the lab directory (§11) with their
   own settings, so the deny list does not get in their way.
3. **MCP servers.** Reverse-engineering MCP servers (the Ghidra servers in §5.3, REA, and any
   ImHex, RenderDoc, Frida or packet-capture servers) are never registered at user, project
   (`.mcp.json`, in this repository or in the lab directory) or local scope. Each lives in a
   per-session configuration file under `$TESSERA_RE_DIR/tools/<tool>/` (§11) that a dirty
   session attaches with `claude --strict-mcp-config --mcp-config <file>` (§5.4,
   `CLEAN_ROOM.md` §7 rule 4). All of them can return dirty material. `tools/dev-setup.sh`
   fails on a decompiler registration at user or project scope and on any
   `$TESSERA_RE_DIR/.mcp.json`; `tools/check-content.sh` fails on a committed one.
   *(Changed 2026-10-11 by [decision 0033](decisions/0033-rea-for-dirty-room-analysis.md):
   the lab directory's own `.mcp.json` is no longer used.)*
4. **LSP.** `rust-analyzer` (§3.1) gives clean sessions go-to-definition, diagnostics and
   references without reading whole files.
5. **Hooks.** Recommended: a PreToolUse hook in clean sessions that refuses reads under
   `research/` and any `clones/` directory (a second line of defence behind the deny rules), and a
   pre-commit hook that runs `tools/check-content.sh` and `tools/check-spdx.sh`.
6. **Scratchpads.** Throwaway probes, extracted records and captures go into the session
   scratchpad or `$TESSERA_RE_DIR/`, never into the repository. Output produced from real archives is
   dirty material even when the code that produced it is clean.
7. **Parallel agents.** When several agents work at once, each owns specific crates or files.
   Workspace-level files (root `Cargo.toml`, `Cargo.lock` policy, CI) belong to the lead session.
8. **Commit trailers.** AI-assisted commits carry `Assisted-by:`; format work carries
   `Spec-Ref:`; spec and clean-path commits carry `Clean-Room-Role:` (`docs/CLEAN_ROOM.md` §8,
   checked by `tools/check-dco.sh`).

---

## 9. Verified prior art and how we use it

`research/resources.json` lists 98 published sources (code, documentation, legal texts). Every
entry was checked to exist on 2026-10-03, with its licence read from the source itself where
possible (P18). The rule for all of them is the same: **facts only, never code**, whatever the
licence (`CLAUDE.md`). A licence matters for what we may copy, and we copy nothing; it still
matters for how far a source may be relied on and quoted.

### 9.1 Sources by role {#prior-art}

| Source | Licence (Reported) | How we use it |
|---|---|---|
| LotroCompanion tools and libraries (Damien Morcellet), including the `delta-lotro-dat-utils` jar | Apache-2.0 at the jar-distribution level only; the GitHub source repos carry **no** licence file (all rights reserved) | Main second witness for the archive container, DID routing and record grammars. The jar may be run on our own install as a black-box oracle (§4.1); it is never decompiled or shipped |
| Mirarkitty `lotro_extractor` | Apache-2.0 | Independent clean reader used as a cross-check for container and property facts |
| Mirarkitty `lotro_mesh_viewer` | MIT (one file GPL-3.0-only) | Mesh, material and appearance facts for M2 and M4 specs |
| jtauber/lotro | none (all rights reserved) | Cross-check of header, B-tree and texture facts only |
| Middle-earth-Revenge `DATUnpacker`, `LocalDataExtractor` | GPL-3.0 | Historical cross-checks; some tools there depend on a proprietary DLL (see §9.2) |
| Gi1dor/Endevir `LotroDat`, text patchers | none / MIT, but bundling a proprietary DLL | Historical notes only |
| Chorizite DatReaderWriter, Chorizite.ACProtocol, WorldBuilder | MIT | Asheron's Call lineage: container and landblock concepts, schema-driven code generation and testing approach |
| ACEmulator ACE, ACViewer | AGPL-3.0 / GPL-3.0-only | Same lineage; architecture ideas only |
| Mogwai/Endevir DatExplorer (DDO) | AGPL-3.0-only | Sister-game container cross-check |
| JuneStepp/OneLauncher, PyLotRO, LotROLinux | GPL-3.0-or-later | Install locations on Linux and Wine, settings-folder layout (`docs/specs/install-layout.md`) |
| lunarwtr `vscode-lotro-api` | MIT | Plugin API names, signatures, enums and events (`docs/specs/lua-plugin-api.md`) |
| Official Lua documentation, skinning documentation (via lotrointerface) | publisher copyright, no licence | Behaviour to implement; prose is never copied |
| exyorha/hkxparse | MIT | Animation container structure facts (M4) |
| digero/Maestro | MIT | Player-music text-notation behaviour (M6) |
| Ghidra, Kaiju, Pharos/OOAnalyzer, BinDiff, BinExport, ghidriff, Ghidra MCP servers | Apache-2.0 / BSD (SEI) / GPL-3.0-only / MIT | Tools in §5; gated |
| Court records and statutes (bnetd, THJ, Directive 2009/24/EC, 17 U.S.C. §1201, UK CDPA) | public law | Basis of `docs/LEGAL.md` |

### 9.2 Never use {#never-use}

- The proprietary dat-export DLL that circulates with fan tools. It is not part of the retail
  install and nobody holds a licence for it. Not even as an oracle (`docs/ROADMAP.md` §2.2).
- Tools whose source appears decompiled from someone else's binary, or that ship tables dumped
  from the client binary. They are dirty material for facts at most, and their code is never
  read in a clean session.
- Binary-only fan clients and private-server code for the game. They raise the T3/T4 questions
  in `docs/LEGAL.md` and add nothing a data-file study cannot give.

---

## 10. Recommended day-1 setup

On the Gentoo workstation (atoms from §3; versions as observed in P1–P7):

```sh
# Toolchain and test tools
emerge --ask dev-util/rustup dev-util/cargo-nextest dev-util/reuse dev-vcs/gitleaks \
             dev-util/sccache sys-devel/mold sys-apps/hexyl
rustup toolchain install nightly --profile minimal     # only for cargo +nightly fuzz
cargo install --locked cargo-fuzz cargo-deny           # or the bentoo packages

# Research viewers (dirty room)
emerge --ask app-editors/imhex                         # guru, ~amd64

# Point Tessera at the install (read-only)
export TESSERA_GAME_DIR="$HOME/.local/share/Steam/steamapps/common/Lord of the Rings Online"
```

Then check that the base works:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                  # synthetic tests; real-data tests run when TESSERA_GAME_DIR is set
cargo run -p tessera-dattool -- validate "$TESSERA_GAME_DIR"
cargo run -p tessera-client -- --smoke
tools/check-content.sh && tools/check-spdx.sh
```

M2 adds RenderDoc, Vulkan tools and lavapipe (§3.1) and, for dirty-room render questions, DXVK
and a Wine prefix (§6). M7 adds the network tools in §7. Nothing in §5 is installed or used until
a decision record authorises it.

---

## 11. Disk and directory layout for research artefacts

Bulky and dirty artefacts live under one **dirty-room data root**, `$TESSERA_RE_DIR`,
outside the repository. It defaults to `~/.cache/tessera/re`; point it at any large private
disk (on the primary workstation that is `~/nvme/re`, P19). Each machine sets it in its own
shell profile; it is never committed.

| Path | Contents | Zone |
|---|---|---|
| `$TESSERA_RE_DIR/snapshots/<build>/` | reflink copies of the archive set and `x64/` before each game patch (`cp --reflink=always`) | dirty |
| `$TESSERA_RE_DIR/manifests/` | per-snapshot manifests and their diffs (§4.1) | dirty |
| `$TESSERA_RE_DIR/prefixes/<name>/` | one Wine prefix per experiment (§6.2) | dirty |
| `$TESSERA_RE_DIR/captures/` | RenderDoc `.rdc`, traces, logs, any packet capture | dirty |
| `$TESSERA_RE_DIR/ghidra/` | Ghidra projects, BSim databases, exports (gated, §5) | dirty |
| `$TESSERA_RE_DIR/corpora/` | fuzz seeds harvested from real archives (never committed; `fuzz/corpus/` is git-ignored as well) | dirty |
| `$TESSERA_RE_DIR/tools/<tool>/` | pinned dirty-tool installs (REA, §5.4) and their per-session MCP configuration and settings files (`mcp-<tool>.json`, `settings-<tool>.json`), attached with `--strict-mcp-config --mcp-config` (§8 item 3); never a `.mcp.json` here or in the lab directory's root | dirty |
| `<repo>/target`, `<repo>/fuzz/target` | build output; move with a machine-local `.cargo/config.toml` (`TESSERA_TARGET_DIR=... tools/dev-setup.sh --fix`) when the repo's disk is small | clean |

Nothing under `$TESSERA_RE_DIR/` is ever pushed, attached to an issue, or opened in a clean session.

---

## 12. Open questions

> **OPEN:** how much passive observation is possible without a server connection (§6, first
> callout). Settle it before planning any further §6 work.

> **OPEN:** can the client's renderer-selection setting pick an OpenGL backend under Wine? If so,
> Linux-native RenderDoc and apitrace could capture it directly, without DXVK in between.

> **OPEN:** commit the clean-session deny rules from §8 as `.claude/settings.json`, or keep them
> per user? Committed rules would also apply to spec-writing sessions run from this checkout.

> **OPEN:** build `plcli` and the Kaitai compiler in CI so the differential checks in §4.2 run on
> every change, or keep them as a developer-box step?

> **OPEN:** a self-hosted CI runner with `TESSERA_GAME_DIR` set would run the real-data tests on
> every change (pass/fail only). Decide whether the owner's machine should take that role.

> **OPEN:** Ghidra MCP choice (§5.3) depends on the Ghidra version: revisit after upgrading past
> 12.1.3. Only relevant if a §5 authorisation is ever recorded.
