<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Extended scripts: folders, manifest, content hash, grants, backend gate and the script tool

| Field | Value |
|---|---|
| Status | **Review** |
| Last updated | 2026-10-07 |
| Author role | Design (clean room). Everything here is **Tessera's own design**, written from [decision 0017](../decisions/0017-extended-scripting.md) and Tessera's own specs. It describes no other program, and no dirty-room material, disassembly or third-party source was read to write it |
| Reviewer | Independent clean-room review, 2026-10-07: **changes requested** (consistency with `ext-threat-model.md`, `ext-api-1a.md`, `ext-api-1b.md` and D17; prompt pointer-entry wording; hot-reload declaration set; re-ask table composition; grant-store I/O thread; `keymap-ext.toml` location; escape-catalogue IDs; and smaller items). **Revision 2** (the same day) answers every returned issue and aligns this spec with TM revision 2 (change log, §16). **Re-review of revision 2, 2026-10-07: changes requested**, status stays Review. Every issue of the first review is resolved and the content-hash vectors of §5.2 reproduce. Still open: (1) the store open and wipe (§8.8) and the log paths (§3.1) state no no-follow or entry-kind rule for `s_<id>/` and its entries, and do not name the temporary files, so TM invariant 15 is not concrete and a wipe through a linked store directory is not excluded; (2) `test` (§11.4) does not say how grants and dialogs behave at loads after the first (reloads, full reloads, `manager` loads), there is no trace event for a dialog, and nothing seeds a grant store, so the ST parts of TM X62, X90 and X92 cannot be built; (3) `no_file_outside` (§11.8) omits the settings crate's persistent lock file, and "the settings crate's lock file" (§8.5) is ambiguous; (4) invariants 9 and 13 contradict the re-read of valid hand-edited records (§8.5, TM §4.3 assumption 3), and a re-read that narrows or invalidates a loaded script's record is not said to suspend it (TM §8.23); plus smaller items (`/ext:reload` with developer mode off, autoload order for records outside `order`, the store size for the content notice, a phase-B load waiting on its write at a switch, `escape` results for mixed ST/CI tests, the O-11 list, the `dump-api` spelling against D17 §10, the L9 citation in §10.3). **Revision 3** (the same day) answers every point of this re-review and aligns this spec with TM revision 3: the store and log no-follow and entry-kind rules with a named temporary-file pattern (§3.6, §8.8, TM §8.8, X94); the harness's dialog policy, `reply` and `grants` walk events, `--grants`, a `dialog` trace event and expectation (§11.4–§11.6); the named lock and temporary files (§8.5, §11.8); the grant-store re-read as a revocation route and invariants 9 and 13 restated (§8.5, TM §8.23); the smaller items (change log, §16). Status stays Review. **Re-review of revision 3, 2026-10-07: changes requested**, status stays Review. Every point of the re-review of revision 2 is answered. §3.6 and §8.8 match TM §8.8 items 1–5, §8.5 and §8.4 match TM §8.23, §9.8 matches TM §8.24 and EA1b §8.1, §10.3–§10.4 match TM §7.4 and §8.21, and §11.4–§11.8 give X62, X91, X92 and X94 their ST routes. The §5.2 vectors reproduce again. Still open: (1) **security, liveness:** §3.4 step 3 (the snapshot walk, also used by `check`) checks each entry's kind only after opening it, and states no non-blocking open, so a FIFO in an unpacked folder (TM X54) blocks an asset worker on its open. The same holds for the log file's write open in §3.6 (`O_NOFOLLOW | O_APPEND`). Fix: open non-blocking, or classify first with `fstatat(…, AT_SYMLINK_NOFOLLOW)`, for both; TM X54's "Fails if" should name a blocked worker; (2) **§8.5:** a re-read replaces the grant table and re-applies only the changes submitted after the read was queued. So session-only decisions are lost. In a newer-document session (§8.6), every manager open re-reads an all-ignored document, which empties the table and, by the re-read rule, suspends every loaded script. A manager revocation whose write failed ("keeps the narrower state for the session") is undone by the next re-read, so a grant the user removed returns without a dialog. Fix: keep the session-only and failed-write changes as an overlay that every re-read result is narrowed by; (3) **testability:** `test` (§11.4) applies `--kind` to every FOLDER, ties developer mode to `--kind developer`, and does not say what starts the first load of each FOLDER (a user-started load at `Open`? in which frame? in command-line order?), nor how that load interacts with the autoload of a `--grants` record that has `autoload = true`. So D17 exit criterion 5's second-copy case (one id in both kinds) and its `/ext:reload` of an installed script with developer mode on cannot be expressed. That leaves the X90 ST part unbuildable, and "turning developer mode off unloads developer scripts" (§9.8) untestable; (4) **§8.5/§8.6 writes:** "replaces … exactly those records, keeping every other record" does not say what a write for a (kind, id) does to duplicate or invalid records of the same key, or when "the first write that would drop them" (§8.6) happens. Nor does it say what an Allow does when the document already holds 256 records or 256 `order` entries (TM L79). Writing a 257th would make the next start-up ignore records; (5) a manager revocation, Reset, Remove or local-only switch, and a narrowing re-read, during a load in progress (phases B–D, the apartment not yet loaded) are unspecified. §9.7 and §8.5 cover only loaded or suspended apartments, so a required grant revoked in phase C or D can still let the script finish loading without it, against D17 §2 "never loads degraded"; (6) **TM §0.1 rule 5 / O-11:** §4's preamble claims that every count, size and range has a TM L-number. Several do not: settings `min`/`max` ±1e15, setting key 1–32 bytes, capability names 1–32 bytes, module-name segments and version digits, the action and context name length, the walk file's `[[entity]] name` 48 cp, the grant-record integer bound 2^53, and the `--grants` copy bound. Register them in TM (or have TM exempt grammar lengths) and cite the rows; (7) **notices:** §9.8's "No loaded extended script '<id>'" differs from EA1b §8.2's "⟨id⟩ is not loaded". §10.5 maps every `/ext:` line outside `Open` to "No loaded extended script", which includes `/ext:list` and `/ext:help`. D17 §3 says those open the script manager, and §10.3 makes the manager available read-only in `Off`. It also includes `/ext:reload` with developer mode off, which §9.8 answers "Developer mode is off". Plus smaller items: §8.8 step 5's "created exclusively" marker when a plain marker is already present; §3.6 "never removes a log entry that is not a plain file" against a rotation that renames over a non-plain `s_<id>.1.log`; and TM §11's intro, which says `escape` reports unrun parts as `CLIENT`, where §11.8 also uses `ELSEWHERE` (a TM wording fix) |
| Implements into | `crates/ext` (`tessera-ext`: manifest parser and validation, folder rules applied to entries it is handed, content hash, grant-store format, the grant decision pipeline of §9.2, walk-file parser); `crates/ext-host` (`tessera-ext-host`: the folder walk, the grant-store queue and persistence, the store open with its owner tag and marker, the load pipeline, the script manager's grant pages, the headless harness); `apps/scripttool` (`tessera-scripttool`); `apps/client` (the backend gate states, consent prompts); `crates/net` (`Session::class()`, D17 §6.1); `fuzz/` (targets of §11.7) |
| Related documents | Decision [0017](../decisions/0017-extended-scripting.md) (abbreviated **D17**; §n means its section n); [`ext-threat-model.md`](ext-threat-model.md) (**TM**: normative for every limit, cited as TM L*n*, for the rules of its §7–§9, cited as TM §*n.m*, and for the escape tests, cited as TM X*n*; TM §0.1); [`ext-api-1a.md`](ext-api-1a.md) (**EA1a**) and [`ext-api-1b.md`](ext-api-1b.md) (**EA1b**), which together are D17's `ext-api.md` for slices 1a and 1b ("the API specs"); [0004](../decisions/0004-server-strategy.md); [0014](../decisions/0014-ui-input-bridge.md) (**IB**); [0015](../decisions/0015-ui-rendering.md) (**UR**); [`keymap.md`](keymap.md) (**KM**); [`lua-plugin-api.md`](lua-plugin-api.md) (**LPA**); [`plugin-runtime.md`](plugin-runtime.md) (**PR**); [`docs/LEGAL.md`](../LEGAL.md) §9.5 and §18; `docs/ARCHITECTURE.md` (**ARCH**) §17, §18, §21 |

---

<a id="s0"></a>
## 0. Purpose and scope

This spec defines the parts of Tessera's extended scripting that are files, records, user
dialogs and tools, as opposed to the Lua API:

- the **folder layout**: where scripts, their stores and their logs live, the rules a
  script folder must follow, and how store directories and log files are opened (§3);
- the **manifest** `script.toml`: its grammar, every field, its validation and the
  refusal rules (§4);
- the **content hash** that pins grants to the code (§5);
- the **capability catalogue** as the manifest, the grant store and the dialogs use it
  (§6);
- **effective rights** and the **load pipeline** in which they are decided (§7);
- the **grant store**: the file that keeps the user's decisions, and the queue that reads
  and writes it (§8);
- the **grant UI flow**: the decision pipeline, the consent dialog with its two groups,
  the developer folder and developer mode, and the script manager's grant pages (§9);
- the **backend gate states**: when ext scripts may exist at all (§10);
- the **script tool** `tessera-scripttool`: `check`, `test` with its walk file and trace,
  and the `escape` runner (§11);
- the **versioning rules** of everything above (§12).

Out of scope: the `Ext` global and every module's functions, the execution model, costs,
leases, the panic chord and ext UI (the API specs, EA1a and EA1b); the threat model, the
register of every numeric limit and the escape-test catalogue (TM); the ext keymap
document's schema and reading rules (KM, D17 §2).

**Precedence.**

- D17 is the accepted design. Where this spec and D17 differ, D17 governs until it is
  amended, and the difference is a defect of this spec.
- TM is normative for D17's open security and design items, its "other improvements",
  every numeric limit and every escape case (TM §0, §0.1). Where this spec and TM differ,
  TM governs and the difference is a defect of this spec. This spec cites TM's limits as
  "TM L*n*"; a number repeated next to such a citation is for the reader only. Escape
  cases are named only by their TM X-number.
- Where this spec adds a rule D17 does not state, the addition is marked **(design
  addition)** so a reviewer can check it against D17. Where it departs from a rule D17
  states, it is marked **(proposed D17 amendment)** and carries an OPEN item (§15) until
  the owner agrees; until then the OPEN item says what applies.

---

<a id="s1"></a>
## 1. Provenance

| Facts | Source | Mark |
|---|---|---|
| Folder kinds, paths, folder rules, snapshot and no-time-of-check reading | D17 §2 | Decision |
| Manifest field set, `id` = folder name, refusal is whole, mutually exclusive capabilities, one loaded script per id | D17 §2 | Decision |
| Capability names, groups, backends and slices; group O and group A | D17 §3 | Decision |
| Rights intersection; prompts, combinations, local-only rules, grant store contents, content hash encoding, re-ask rules, `Store` tied to the grant record, orphans | D17 §5.1, §5.2 | Decision |
| Prompt presentation rules (reserved layer, plain text, pointer-only "Allow", 500 ms, pointer entry after the delay) | D17 §5.3 | Decision |
| Backend classes, fail-closed gate, slice-1a admission, backend switch | D17 §4 "Lifecycle", §6 | Decision |
| `check`, `test`, `init`, `pack`, deterministic mode, headless harness | D17 §8 "Deterministic mode", §10 | Decision |
| Module majors, deprecation window, `Ext.Has`, `Ext.Version` | D17 §2 "Versioning" | Decision |
| Snapshot-root no-follow rule (TM §8.8), one suggested default per stroke (TM §8.16), retained snapshot charged to the script (TM §8.10), display strings refused at parse time with TM's mark rule (TM §8.2), the liveness floor and switch timeout (TM §7.4), partial loads at a switch (TM §8.21), `keymap-ext.toml` location (TM §8.19), revocation suspends, a grant-store re-read included (TM §8.23), no store or hand-off across an `author` change and the store owner tag (TM §8.24), consent pointer entry (TM §8.25), the store-directory and log-file no-follow rule (TM §8.8), loads waiting on a record write or store probe at a switch (TM §8.21), every limit, the manifest's per-field counts, the walk file, the folder listing and the owner tag included (TM §10, L86–L89), every escape case and per-part reporting (TM §11) | TM revision 3, from D17 "Open security and design items" and "Other improvements" | Decision (TM) |
| `Ext.Use` and the manifest's module tables, including `"not declared"` | EA1a §3 | Decision (EA1a) |
| Name grammar, stroke syntax, keymap limits, saving rules | KM §2, §4, §5, §6 | Decision (KM) |
| Activation filter `on` | IB D3 | Decision (IB) |
| Settings file location, atomic save, lock file, install guard, tolerant loading with backups | `crates/settings` rustdoc (the settings file has no spec; its rustdoc is its definition) | Decision (code) |
| Settings-crate lock and temporary-file names (`.<file name>.lock`, `.<file name>.tmp-…`) | `crates/settings` rustdoc of `SettingsStore::update` and the atomic write | Decision (code) |
| Everything else here (TOML shapes, the grant-store schema and queue, the store's file names and per-entry actions, the decision pipeline, dialog kinds and texts, gate state names, CLI syntax, walk file, the harness's dialog policy, trace format, `escape` runner, exit codes) | This document, 2026-10-07 (revisions 2 and 3 the same day) | Design |

No fact here comes from the original client, its data or any third-party tool.

---

<a id="s2"></a>
## 2. Terms and notation

| Term | Meaning |
|---|---|
| **Ext script**, **script** | A program for Tessera's extended API (D17). "Plugin" always means a standard plugin for the original client's API; plugins are not covered here |
| **Kind** (folder kind) | `scripts` (installed) or `scripts-dev` (developer). Shown to users as `installed` and `developer` |
| **(kind, id)** | The key of a grant record and of a `Store`. Two folders with the same id but different kinds are different scripts with separate grants and stores |
| **Snapshot** | The in-memory copy of a script folder, read once per load (§3.4) |
| **Source file** | A snapshot file whose name ends in `.lua` (§3.3) |
| **Group O / group A** | "Observe and show" / "Act for you" (D17 §3) |
| **Local-only capability** | `data.read` and `bridge.plugins` (D17 §5.2) |
| **Admitted class** | A backend class on which ext apartments may exist (§10.1) |
| **Developer mode** | Tessera's setting `[ext] developer_mode` (§9.8) |
| **Display string** | A manifest string shown to the user (§4.4) |
| **Grant table** | The ext host's in-memory copy of the grant store, kept on the main thread (§8.5) |
| **Grant-store queue** | The one serial worker queue that performs every read and write of the grant store (§8.5) |
| **Owner tag** | The host file `owner` in a store directory, naming the `author` the store belongs to (§8.8, TM §8.24) |
| **Byte order** | Ordering by unsigned byte values, the shorter string first when one is a prefix of the other |
| **cp** | Unicode code points |
| **TM L*n*, TM X*n*** | A limit of TM's register (TM §10) and an escape test of TM's catalogue (TM §11) |

"Refused" means the script is not loaded and the user gets a report; nothing of it runs
(D17 §2, "Refusal is whole"). "Reported" means a line in the script manager (and in
developer mode the console), never in the chat window (D17 §7, "Chat").

---

<a id="s3"></a>
## 3. Folder layout

<a id="s3-1"></a>
### 3.1 Directories

All paths are under Tessera's own directories (ARCH §17), never inside a game install or the
original client's `Plugins/` tree.

| Path | Holds | Written by |
|---|---|---|
| `data/scripts/<id>/` | Installed scripts, one folder each | The user (Tessera never writes here, except `scripttool init` when the user names this place) |
| `data/scripts-dev/<id>/` | Developer scripts. Listed, loaded and watched only while developer mode is on | The user |
| `data/script-store/<kind>/s_<id>/` | The script's `Store`: one file `k_<key>.store` per key, at most one write-behind temporary file `t_<key>.tmp` per key, the marker `local-only` (D17 §2, §8) and the owner tag `owner` (§8.8). Nothing else (§3.6) | The ext host, on the store's I/O queue |
| `logs/scripts/<kind>/s_<id>.log`, `s_<id>.1.log` | The script log, rotated once (D17 §8; record format TM §8.1) | The ext host, on a worker |
| `<config>/ext-grants.toml` | The grant store (§8); beside its resolved target, the persistent lock file `.ext-grants.toml.lock` and, only while a write runs, its temporary file (§8.5) | The ext host, on the grant-store queue |
| `keymap-ext.toml` in the directory of the **configured keymap path** | The ext keymap document (D17 §2; KM). Its directory is `$TESSERA_KEYMAP_FILE`'s directory when that is set and non-empty, otherwise the directory of the keymap file (KM §6.5); when the keymap file is a symbolic link, the link's own directory, not its target's (TM §8.19) | The binding UI |

`<config>` is the directory of the settings file (§8.1). The two documents differ in place
whenever `$TESSERA_KEYMAP_FILE` points outside `<config>`.

Rules that hold for every path in the table (D17 §2):

- No script-chosen string is ever a bare file name: ids and `Store` keys appear only behind
  the fixed prefixes `s_`, `k_` and `t_`. The four name forms of a store directory are
  disjoint: a **key file** `k_<key>.store`, a **temporary file** `t_<key>.tmp` (`<key>`
  in both is a `Store` key, TM L38), and the two **host files** `owner` and `local-only`,
  which start with neither prefix. Any other name in a store directory is unexpected
  (§3.6).
- Every file under `data/` and `logs/` is opened relative to an open directory handle
  (`openat` on POSIX; a handle-relative `NtCreateFile` on Windows), never through a path.
  The script folder (§3.4), the store directory and its entries, and the log files (§3.6)
  are never opened through a link.
- Saves of `keymap-ext.toml` pass the settings crate's install guard on the **resolved**
  target (TM §8.19).

<a id="s3-2"></a>
### 3.2 Script ids

- **Grammar:** 1–32 bytes of `a-z`, `0-9`, `_` (KM §2's name rule without `.`; TM L38).
- **Reserved, refused on every platform with "reserved name":** `list`, `reload`, `help` (the
  `/ext:` management words), and the Windows device names `con`, `prn`, `aux`, `nul`,
  `com0`–`com9`, `lpt0`–`lpt9`.
- The id is the folder's name, and the manifest's `id` must equal it byte for byte (§4.5).
- An id never contains upper case, so two ids never differ only in case.

<a id="s3-3"></a>
### 3.3 Script folder rules

A script folder is acceptable only if all of the following hold. Any failure refuses the
whole script (D17 §2). The numeric caps are TM L36; they are repeated here for the reader.

| Rule | Limit |
|---|---|
| Regular files | at most 256 |
| Total size of regular files | at most 8 MiB (8,388,608 bytes), counted on bytes read |
| One file | at most 3 MiB (3,145,728 bytes) |
| One source file (`.lua`) | at most 128 KiB (131,072 bytes) |
| The manifest `script.toml` | at most 64 KiB (65,536 bytes; TM L37) |
| Directory depth | at most 4 directory levels below the script folder, so a relative path has at most **5 segments** (`a/b/c/d/x.lua` is allowed; `a/b/c/d/e/x.lua` is refused) |
| Directories | at most 64 |
| Directory entries of any kind | at most 512 in total (files, directories, links and anything else, refused kinds included) |
| Path segment | 1–64 bytes of `a-z`, `A-Z`, `0-9`, `_`, `-`, `.`; not `.` or `..`; not ending in `.` **(design addition**: Windows drops a trailing dot, so `x.` and `x` would be one file there); not a Windows device name with or without an extension, where the **extension** is everything from the first `.` on, and the part before the first `.` is compared with the device names ASCII case-insensitively (`CON`, `nul.txt`, `Com1.lua`, `aux.tar.gz` are refused) |
| Case | no two paths of the folder may be equal under ASCII case folding (`Main.lua` beside `main.lua` is refused) **(design addition**: D17 wants a folder to read the same on every platform, and case-insensitive file systems cannot hold both) |
| Source suffix | a file whose last segment ends in `.lua` in any other ASCII case (`x.LUA`, `x.Lua`) is refused **(design addition**, for the same reason) |
| Entry kinds | regular files and directories only. A symbolic link, a reparse point, a special file (FIFO, socket, device) and a regular file whose link count on the open handle is above 1 are refused |
| Required files | `script.toml` at the root, and the manifest's `entry` file |

A **source file** is a regular file whose last segment ends in `.lua` (lower case). Every
other regular file is a **data file**. Directories are not part of the snapshot: an empty
directory changes nothing (it counts only toward the entry and directory caps).

<a id="s3-4"></a>
### 3.4 Reading a folder (the snapshot)

The reading rules close every time-of-check gap (D17 §2):

1. The ext host keeps an open directory handle of `data/scripts` (or `data/scripts-dev`).
2. **The script folder itself** is opened relative to that handle with `O_NOFOLLOW |
   O_DIRECTORY` on POSIX; on Windows, relative to the parent handle with
   `FILE_FLAG_OPEN_REPARSE_POINT`, and a reparse point is refused (TM §8.8). So a symbolic
   link named `data/scripts/<id>` (from an unpacked archive, say) never brings outside
   files into the snapshot. The data directory itself may be a link (it is the user's
   configuration).
3. The walk proceeds only through directory-relative opens that refuse links, checks every
   entry's type and link count on the open handle (never on the path), counts every entry
   it reads, and stops reading a listing as soon as the entry cap is reached.
4. Every file is read to its end through the caps: the per-file cap, the source cap and
   the 8 MiB total are counted on the bytes actually read. A file that grows past a cap
   while it is read refuses the script.
5. The walk runs on an asset worker, never on the main thread (ARCH §21).

The result is the **snapshot**: the list of (relative path, bytes) of every regular file.
Relative paths use `/` and contain no leading `/`.

- **What is hashed and compiled.** The content hash (§5) is computed over exactly the
  snapshot's bytes, and every source file is compiled from the same bytes. A file changed
  on disk after the snapshot changes nothing until the next load.
- **Memory (TM §8.10, TM L23, L24).** The whole snapshot (at most 8 MiB) is held under the
  load's **memory reservation** (§7.3), which counts toward the 512 MiB sum from the start
  of the load. After the last source file has been compiled, the host keeps only the
  snapshot's **data files** (to serve them to the script) and the content hash, drops the
  source bytes, and charges the kept bytes to the script's memory cap. There is no
  separate bound for retained snapshots outside the caps. (This closes the former OPEN
  O-4.)

<a id="s3-5"></a>
### 3.5 Which folders exist for the host

- At start-up, when the script manager opens, and when developer mode changes, the host
  lists `data/scripts/` and, with developer mode on, `data/scripts-dev/`, on a worker.
  Each listing reads at most 512 entries (TM L88) **(design addition**, the folder entry
  cap of TM L36 applied to the listing); past that the manager shows "Too many entries in
  `data/scripts`" and lists what it read. An entry that is not a directory, or whose name
  is not a valid id, is listed as "Not a script folder" with its name shown as plain text
  (TM §8.2's display rule), and is never loaded.
- A folder that is listed is not yet read: its manifest is read only by a load (§7.3) or by
  the script manager's details page (which reads `script.toml` alone, through the same
  64 KiB cap and the same parser, on a worker).

<a id="s3-6"></a>
### 3.6 Store directories and log files: no links, known entries only

This section is TM §8.8's rule for store directories and log files (TM invariant 15),
stated for this spec's file names. It applies to every job that touches a store (the open
of §8.8, the store probe, flushes, reads, wipes, Remove and Reset) and to the log writer.

**Directory handles.**

1. `data/script-store/` and `logs/scripts/` are opened as directory handles; each may be a
   symbolic link, as the data directory may (the user's configuration).
2. The `<kind>` directory (`scripts` or `scripts-dev`) below each is opened relative to that
   handle with `O_NOFOLLOW | O_DIRECTORY` (POSIX), or relative to the parent handle with
   `FILE_FLAG_OPEN_REPARSE_POINT` and a reparse-point refusal (Windows). A job that may
   create files (the store open, the log writer) creates it when it is missing (`mkdirat`,
   then opens it the same way); the store probe, Remove and Reset never create it.
3. `s_<id>` is opened relative to the `<kind>` handle the same way, and created the same
   way only by the store open of §8.8.
4. **A directory of steps 2–3 that is a symbolic link, a junction or other reparse point,
   or not a directory, is never used:** nothing in it or behind it is read, written or
   removed. The store open and the store probe report it as **unusable** (the open refuses
   the load; the script manager names the path, "The stored values folder of notes ·
   installed is not a plain folder: data/script-store/scripts/s_notes"); Remove and Reset
   report it and remove nothing. For `logs/scripts/<kind>`, the log is **unavailable**
   (below).

**Entries of `s_<id>/`.** Every entry is opened, created, renamed or removed relative to
the `s_<id>` handle, never by a path. An open uses `O_NOFOLLOW` (POSIX) or
`FILE_FLAG_OPEN_REPARSE_POINT` with a reparse-point refusal (Windows), and is
non-blocking for reading, so a FIFO never blocks a worker. The entry's kind and link
count are checked **on the opened handle** (and, when a listing classifies an entry before
opening it, with `fstatat(…, AT_SYMLINK_NOFOLLOW)` first). A **plain file** is a regular
file whose link count is 1. New files are created exclusively (`O_CREAT | O_EXCL`, or
`CREATE_NEW`); a rename (`renameat`) only replaces a directory entry. A **no-follow
removal** removes the entry itself (`unlinkat` without `AT_REMOVEDIR`; on Windows, deleting
the reparse point or file opened with `FILE_FLAG_OPEN_REPARSE_POINT`), so a link's target
and a hard link's other names are never touched.

| Entry found in `s_<id>/` | At the open (§8.8 step 2) | Elsewhere (probe, flush, read) |
|---|---|---|
| `k_<key>.store`, `<key>` valid (TM L38), a plain file | Kept; listed into the file index | Used |
| `t_<key>.tmp`, `<key>` valid, a plain file | A crash's leftover: removed (no-follow) | A flush that finds its own temporary name taken removes that entry (no-follow) and creates it again once; a second failure fails the flush of that key, which stays pending |
| `owner` or `local-only`, a plain file | Used (§8.8) | The probe reads `owner` |
| `local-only` that is **not** a plain file (a link, a reparse point, a special file, more than one link) | Removed (no-follow); the marker counts as **present** for the local-only check of §8.8 step 4, and if the store is not wiped a fresh plain marker is written in §8.8 step 5, so the store stays marked | The probe reports the marker present |
| `owner` that is not a plain file | Removed (no-follow); the tag counts as **missing** (so the store is wiped, §8.8 step 3) | The probe reports the tag as not matching |
| A `k_` or `t_` name that is not a plain file, or whose `<key>` is not valid | Removed (no-follow); never opened | Skipped; a read of that key answers as absent and the entry is left for the next open |
| Any other name that is not a directory (unknown files, links, reparse points, special files) | Removed (no-follow); never opened | Skipped |
| A **subdirectory** (any name) | The open fails closed: the load is refused as for an unusable directory; the host never enters or removes it | The probe reports the store unusable; a flush or read fails with an I/O error |
| An entry that a no-follow removal cannot remove | The open fails closed, as for a subdirectory | — |

An entry swapped between the listing and its open is caught by the check on the handle:
at the open it is removed as the table says; in a read it is skipped (absent); in a flush
the temporary file is created exclusively, so a swapped-in entry makes the create fail as
above. No entry that the table does not keep is ever listed into the file index or read.

**Log files.** `s_<id>.log` is opened relative to the `logs/scripts/<kind>` handle with
`O_NOFOLLOW | O_APPEND` (POSIX) or with `FILE_FLAG_OPEN_REPARSE_POINT` and append access
(Windows), and used only if it is a plain file; when it is missing it is created
exclusively. Rotation renames `s_<id>.log` over `s_<id>.1.log` relative to the same handle
(a rename never writes through the old `s_<id>.1.log`; if that name is a directory, the
rename fails). When the `<kind>` directory is unusable, or a log file is not a plain file,
or a create or rotation fails, the log is **unavailable** for that script until the next
load: nothing is written, the entry is left alone, the script still loads and runs, and
the script manager reports "The log of notes · installed cannot be written:
logs/scripts/scripts/s_notes.log". Tessera never removes a log entry that is not a plain
file.

**Tests.** TM X94 (the ST part through `escape`, §11.8, the U part in `tessera-ext-host`);
TM X54 for the script folder.

---

<a id="s4"></a>
## 4. The manifest `script.toml`

The resource bounds of the manifest are TM L37 (size, nesting, keys), L38 (ids), L60
(actions and contexts), L80 (display strings) and L87 (the per-field counts: capabilities,
modules, settings, choices, `max_length`, `version` and `license`). The character sets of
each field's grammar are this spec's; every count, size and range has its TM L-number (TM
§0.1 rule 5), and a number repeated below is for the reader only. (This closes the former
OPEN O-11.)

<a id="s4-1"></a>
### 4.1 Format and parsing

- **TOML 1.0.0**, UTF-8 without a byte-order mark, at most 64 KiB. The file is hostile
  input: it is parsed with a bounded parser and has a fuzz target (§11.7). Bounds (TM
  L37): at most 64 KiB of input, at most 1,024 keys in the whole document, and nesting at
  most **4 levels below the document root**: the root table is level 0, and a table or
  array that is a value inside a level-*n* table or array is at level *n* + 1. So
  `[[settings]]` is level 1 (the array) and 2 (each table), its `choices` array level 3,
  and a choice table level 4, which is allowed; nothing in a valid manifest is deeper.
- **Strict.** Every error refuses the script. Unknown keys refuse it ("unknown key"), at
  every level, so a misspelt field is never silently ignored. Datetime values are refused
  anywhere. Keys are case-sensitive.
- **Dotted capability names.** In TOML a bare key `entity.read = "…"` is a dotted key that
  makes a nested table. Inside `[capabilities]` and `[optional_capabilities]` the parser
  reports such a nested table as "capability names containing '.' must be quoted
  (`\"entity.read\" = …`)" and refuses the script.
- **Integers and floats.** A field documented as an integer refuses a float (`memory_mib =
  48.0` is refused); a field documented as a number accepts both and stores an `f64`.
  Non-finite numbers (`inf`, `nan`) are refused everywhere.

<a id="s4-2"></a>
### 4.2 Top-level fields

D17 §2's field table lists `id`, `name`, `version`, `author`, `description`, `license`,
`api`, `entry`, `capabilities`, `optional_capabilities`, `memory_mib`, `[[contexts]]`,
`[[actions]]`, `[[settings]]` and `autoload`. The rows marked **(design addition)** are
this spec's.

| Key | Type | Required | Rule |
|---|---|---|---|
| `manifest` | integer | yes | **(Design addition.)** The manifest format version this file uses (§12.1). First release: `1` |
| `id` | string | yes | §3.2; equals the folder name (§4.5) |
| `name` | display string | yes | at most 48 cp (§4.4) |
| `version` | string | yes | Semantic Versioning 2.0.0 (`1.4.0`, `2.0.0-beta.1`, `1.0.0+build.5`); at most 64 bytes; every numeric identifier at most 9 digits and without leading zeros (TM L87). Informational only (§12.4) |
| `author` | display string | yes | at most 48 cp. It **pins grants and the store**: a change re-asks every grant and empties the `Store` (§9.5) |
| `description` | display string | no | at most 512 cp; may contain line feeds (U+000A), at most 8 lines |
| `license` | string | no | at most 64 bytes (TM L87) of printable ASCII (0x20–0x7E). An SPDX licence expression is recommended; `check` warns when it is absent (W04) |
| `api` | integer | yes | The major version of the `Ext` global's own entries (`Use`, `Has`, `Load`, `Version`; EA1a §3) the script targets (§12.2). First release: `1` |
| `entry` | string | yes | The main source file, a relative path (§4.6) |
| `capabilities` | table | no | Required capabilities: quoted capability name → reason (§4.7) |
| `optional_capabilities` | table | no | Optional capabilities: quoted capability name → reason (§4.7) |
| `modules` | table | no | **(Design addition.)** Required modules: module name → version (§4.8) |
| `optional_modules` | table | no | **(Design addition.)** Optional modules: module name → version (§4.8) |
| `memory_mib` | integer | no | 32–64, default 32 (§4.9; TM L23) |
| `autoload` | boolean | no | Default `false`. The initial value of the dialog's "Load automatically" box at first enable (§9.3); the user's choice is what counts afterwards |
| `contexts` | array of tables (`[[contexts]]`) | no | At most 2 (§4.10; TM L60) |
| `actions` | array of tables (`[[actions]]`) | no | At most 64 (§4.11; TM L60) |
| `settings` | array of tables (`[[settings]]`) | no | At most 32 (§4.12; TM L87) |
| `server` | table (`[server]`) | no | **(Design addition.)** Server hints (§4.13) |

<a id="s4-3"></a>
### 4.3 A complete example

The following is a valid `manifest = 1` file for the note-taker reference script of the
M5.5 exit criteria (comments are allowed in TOML and ignored):

```toml
manifest = 1
id = "notes"
name = "Notes"
version = "1.2.0"
author = "Example Author"
description = "A small note pad. Press its key, type a line, and it is kept."
license = "CC0-1.0"
api = 1
entry = "main.lua"
memory_mib = 32
autoload = true

[capabilities]
ui = "Shows the note pad window."
input = "Lets you bind a key that opens the note pad."

[optional_capabilities]
"world.read" = "Adds your map position to each note."

[modules]
UI = 1
Input = 1
Store = 1
Commands = 1
Core = "1.0"

[optional_modules]
World = 1

[[contexts]]
name = "pad"
label = "Note pad"

[[actions]]
name = "toggle"
label = "Open or close the note pad"
kind = "button"
context = "pad"
default = "Ctrl+N"

[[settings]]
key = "stamp_position"
type = "bool"
label = "Add the map position to new notes"
default = true

[[settings]]
key = "max_notes"
type = "number"
label = "Notes to keep"
default = 200
min = 10
max = 1000
integer = true
```

<a id="s4-4"></a>
### 4.4 Display strings

`name`, `author`, `description`, every capability reason, and every `label`, choice label
and setting `description` are **display strings**: they reach the reserved layer, the
binding UI and the settings page. TM §8.2 draws every string that is not Tessera's through
its plain-text steps 1–5, and requires a manifest display string that those steps would
change to be refused when the manifest is parsed (the stricter rule). This spec states
that refusal; "M" below is the general category Mark (Mn, Mc and Me):

| Rule | Detail |
|---|---|
| Not empty | at least one character that is not white space (Unicode `White_Space`) |
| No surrounding space | no leading or trailing `White_Space` character |
| Removed characters (TM §8.2 step 2) | no character of general category Cc, Cf (every bidirectional control, U+200B–U+200D, U+2060, U+FEFF and the tag characters among them), Zl, Zp, Cs, Co or Cn, and no noncharacter (U+FDD0–U+FDEF and the last two code points of every plane). Single exception: U+000A in `description` |
| Variation selectors | U+FE00–U+FE0F and U+E0100–U+E01EF are refused anywhere (stricter than TM §8.2, which allows a spec to refuse more) |
| Combining marks (TM §8.2 step 3) | at most **2** characters of category M after each base character; a mark with no base (at the start of the string, or right after a line feed) is refused |
| Length (TM §8.2 step 4, TM L80) | at most the field's cap in code points **and** in UTF-8 bytes: `name`, `author`, labels and choice labels 48 cp and 192 bytes; reasons and setting descriptions 160 cp and 640 bytes; `description` 512 cp and 2,048 bytes, at most 8 lines |

A string that breaks a rule refuses the script with M07 (§11.3). A refused manifest's
strings are never displayed: every report about it names the script by its id and folder
kind only, and `check`'s M07 message names the field, the rule and the code-point position
without echoing the string (TM §8.2). Markup is not refused: `<b>Tessera</b>` is a valid
`name` and is drawn as literal text. So for a manifest that loads, the display rule
changes nothing, and it remains the only guard for strings that do not come from a
manifest (errors, log lines, typed `/ext:` words and ids). TM X47 tests both: part (a) the
parse-time refusal through `escape` (§11.8), parts (b) and (c) the display rule directly
and on non-manifest strings. (This closes the former OPEN O-9.)

The Unicode version of the category tables is the one the build carries; `check` prints it
(§11.3).

<a id="s4-5"></a>
### 4.5 `id` and the folder

- `id` must equal the folder's name byte for byte, or the script is refused ("id does not
  match the folder name").
- The (kind, id) is the folder's kind and this id. Nothing in the manifest names a kind.

<a id="s4-6"></a>
### 4.6 `entry` and other paths

- A relative path with `/` separators, every segment following §3.3's segment rule, no
  leading `/`, at most 5 segments (the depth §3.3 allows), ending in `.lua`.
- The path must name a source file of the snapshot. A missing entry refuses the script
  ("entry file not found").
- Every source file of the snapshot is compiled at load (D17 §2, "Loading"), whether the
  entry or `Ext.Load` reaches it or not. A source file that does not compile refuses the
  script, with its chunk name (its relative path) and line (§4.14 says when a load finds
  it). A source file that starts with the byte 0x1B (a binary chunk) is refused ("binary
  chunks are not accepted").

<a id="s4-7"></a>
### 4.7 Capabilities

- `[capabilities]` and `[optional_capabilities]` are tables whose keys are capability names
  (§6.1) and whose values are reasons (display strings, at most 160 cp) shown in the
  dialog.
- At most 16 entries in the two tables together (TM L87).
- A name in both tables refuses the script ("listed as both required and optional").
- An unknown capability name, in either table, refuses the script ("unknown capability"),
  as D17 §2 says. See OPEN O-1 (§15) for optional entries.
- **Local-only exclusion.** If `session.server` and a local-only capability (`data.read`,
  `bridge.plugins`) are **both required**, the script is refused ("cannot hold
  `session.server` together with a local-only capability"). If at least one of them is
  optional, the manifest is valid; the dialog lets the user grant one side only (§9.4).
- Required versus optional is the author's statement of what the script can live
  without. An optional capability that is withheld only makes `Ext.Has` false and
  `Ext.Use` answer `nil, "not granted"` (D17 §2, EA1a §3).

<a id="s4-8"></a>
### 4.8 Modules and their versions **(design addition)**

D17 §2 refuses an unknown module major; these tables let the load and `check` do so before
any code runs, and let a script state the minor it needs.

- `[modules]` and `[optional_modules]` map a **module name** to a **version**.
- **Module name:** one or two segments joined by `.`, each segment `A-Z` followed by 0–31
  bytes of `A-Z`, `a-z`, `0-9` (`World`, `Input.Drive`, `UI.Native`). At most 32 entries in
  the two tables together (TM L87); a name in both refuses the script.
- **Version:** either an integer major ≥ 1 (`World = 1`, meaning "major 1, any minor"), or
  a string `"M.m"` (`Core = "1.2"`, meaning "major M, minor at least m"), both parts
  decimal without leading zeros, at most 4 digits each.
- **Load-time check against the build's registry** (the language-neutral registry of
  D17 §1.3):

  | Case | Required module | Optional module |
  |---|---|---|
  | Name not in the registry | refused, "unknown module" | refused, "unknown module" |
  | Major not served | refused, "major not supported" | `Ext.Has(name, major)` is false; `Ext.Use` answers `nil, "major not supported"` |
  | Major served, minor lower than asked | refused, "needs a newer Tessera (World 1.3, this build has 1.1)" | as "major not supported" |
  | Major served but deprecated (§12.3) | loads; one warning in the script log per load | loads; one warning per load |

- **Consistency with capabilities.** Every registry module records the capability it
  needs, or none (`Core`, `Store`, `Commands` need none, D17 §3). Then:
  - a **required** module's capability must be in `[capabilities]`; if it is missing, or
    present only in `[optional_capabilities]`, the script is refused ("module UI needs
    capability `ui` in [capabilities]");
  - an **optional** module's capability must be in one of the two tables, or the script
    is refused;
  - a capability that no declared module needs is allowed, and `check` warns (W02).
- **Modules the script did not declare (EA1a §3).** A script uses only what its manifest
  states, `Core`, `Store` and `Commands` included. `Ext.Use(name, major)` for a registry
  module that neither table declares, or that is declared with another major, answers
  `nil, "not declared"`, the fifth reason EA1a §3 adds to D17 §2's four, and `Ext.Has`
  answers false. EA1a §3 gives the order in which `Ext.Use` checks its reasons. `check`
  warns (W09) when a source file calls `Ext.Use` or `Ext.Has` with a literal module name
  and major that the manifest does not declare. (This closes the former OPEN O-2.)

<a id="s4-9"></a>
### 4.9 Memory

`memory_mib` asks for a per-script memory cap above the 32 MiB default, at most 64 (D17
§8, TM L23). A value above 32 is shown in the dialog and pinned in the grant record (§8.3);
a later manifest that asks for more than the pinned value asks the user again (§9.2
step 3).

<a id="s4-10"></a>
### 4.10 `[[contexts]]`

| Key | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | 1 to (59 − length of `id`) bytes of `a-z`, `0-9`, `_` (D17 §8 "Action and context names": the registered name is `ext.<id>.<name>`, at most 64 bytes) |
| `label` | display string | no | **(Design addition.)** At most 48 cp; shown in the binding UI. Default: the name |

- At most 2 contexts (TM L60); names unique within the script.
- A context is never opaque, `opaque_keys`, `text` or pass-through, and the manifest has no
  key to ask for that (D17 §5.5). It has no `priority` key either: the host assigns every
  ext context's priority in KM's reserved range −1,000 to −1 (TM L60) as EA1a §8.1 says.
  (This closes the former OPEN O-3.)
- Contexts start inactive; the script activates them through `Input` (D17 §10, EA1a §8.1).

<a id="s4-11"></a>
### 4.11 `[[actions]]`

| Key | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | as a context name (§4.10); unique among the script's actions |
| `label` | display string | yes | at most 48 cp; shown in the binding UI |
| `kind` | string | no | `"button"` (default) or `"axis"` (KM §2). The modifier policy is KM §2's default for the kind (exact for buttons, subset for axes) |
| `context` | string | if the script declares 2 contexts | the name of one of the script's `[[contexts]]`. With exactly one context it defaults to that one. With none, an action refuses the script ("an action needs a context") |
| `default` | string or table | no | The **suggested default binding**, below |

At most 64 actions per script (TM L60).

**Suggested default (`default`).**

- Either a stroke string (`"Ctrl+N"`), or a table `{ input = "<stroke>", on = "<on>",
  value = <number> }` with `input` required and `on` and `value` optional.
- The stroke follows KM §5.1 (modifiers and one to four buttons, `+`-separated, no white
  space). A sequence (any white space), an axis name or an analog entry (`axis = …`) refuses
  the script: suggested defaults are single digital strokes (D17 §2).
- `on` is one of IB D3's five values (`press`, the default, `release`, `tap`, `double`,
  `double_nearby`), allowed only where IB D3 allows it: a non-`press` `on` needs a
  **button** action and a stroke with exactly one button that is not a wheel click.
  Otherwise the script is refused.
- `value` follows KM §5.3 (in −1..=1; ignored for button actions).
- **One suggested default per stroke per script** (TM §8.16, TM L61): two actions of one
  manifest whose suggested strokes are equal after KM §5.1's normalisation refuse the
  script ("two actions suggest the same stroke"), whatever their `on` values. So one press
  can never start more than one hold from one script's suggested defaults.
- Whether a suggested default is **applied** is decided at registration (D17 §5.5,
  "Collision"): a stroke that a built-in or plugin binding would match in either pass, in an
  active context or not, whatever either `on`, is withheld. `check` predicts the
  collisions against the built-in defaults it knows (KM §8, IB D2) as warning W03; it
  cannot know plugin contexts.
- User bindings are never in the manifest: they live in `keymap-ext.toml` (D17 §2, §3.1).

<a id="s4-12"></a>
### 4.12 `[[settings]]`

The host shows these as a settings page of the script in the script manager and hands the
values to the script read-only (D17 §2; EA1a §4.4). Key bindings are not a settings type.
D17 §2 names key, type, default, label, the number range and the text length cap; the rows
marked **(design addition)** are this spec's.

| Key | Type | Required | Rule |
|---|---|---|---|
| `key` | string | yes | 1–32 bytes of `a-z`, `0-9`, `_`; unique within the script |
| `type` | string | yes | `"bool"`, `"number"`, `"choice"` or `"text"` |
| `label` | display string | yes | at most 48 cp |
| `description` | display string | no | **(Design addition.)** At most 160 cp |
| `default` | by type | yes | must be valid for the type, below |
| `min`, `max` | number | for `number` | finite, `min` ≤ `max`, both within ±1e15 |
| `step` | number | no (`number` only) | **(Design addition.)** Finite, > 0 |
| `integer` | boolean | no (`number` only) | **(Design addition.)** Default `false`; when true, `min`, `max`, `default` and `step` must be integers |
| `choices` | array of tables | for `choice` | 1–32 entries (TM L87) `{ value = "<key grammar>", label = "<display string, 48 cp>" }`, values unique |
| `max_length` | integer | no (`text` only) | **(Design addition**, D17's "length cap" made a key.) 1–256 bytes, default 64 (TM L87) |

- `default` of a `number` lies in `min..=max`; of a `choice` is one of the values; of a
  `text` is at most `max_length` bytes and follows the display-string character rules
  (§4.4) except that it may be empty.
- A key that does not belong to the setting's type (`choices` on a `bool`) refuses the
  script, as every unknown key does.
- **Stored values** are kept in the grant record (§8.3). When a load finds a stored value
  that is no longer valid for the current schema (the type changed, the range shrank, the
  choice vanished), the value reverts to the default, with one line in the script log.
  Such values, and stored values of keys the schema no longer has, are **dropped by that
  load's record write** (§9.2 step 6): the key is removed from the record's `settings`.
  A load that has nothing else to update writes the record for this alone, so the drop
  happens once, at the first load that finds it. A reverted value is never written as its
  default: an absent key takes the manifest's default, so a later change of that default
  reaches the user. A load that writes nothing (Deny, "Not now", a refusal) leaves the
  stored values as they are; the next load drops them.
- Text values typed by the user follow the same character rules; the settings page refuses
  forbidden characters as they are typed.
- A change the user applies is written as §8.5 says and reaches a loaded script through
  `Core.Setting` and its settings-changed handler (EA1a §4.1, §4.4).

<a id="s4-13"></a>
### 4.13 `[server]`: server hints **(design addition)**

Hints only ever **narrow** what a script may do; nothing in them widens a right, and the
server's own policy (D17 §6.5) is always applied on top.

| Key | Type | Rule |
|---|---|---|
| `backends` | array of strings | The backend classes the script is meant to run on: any non-empty subset of `"offline"` and `"tessera-protocol"`, no duplicates. Default: both. On a class not listed, the script is not loaded and the manager shows "This script does not run on this connection" (not an error, and no prompt) |
| `entry` | string | The server-side half of slice 3 (D17 §11), a path as §4.6. Until a build serves slice 3, a manifest with this key is refused ("needs a newer Tessera"); its semantics are OPEN O-7 |

- `check` warns (W07) when a **required** capability cannot be available on a listed
  class (for example `data.read`, offline only, with `"tessera-protocol"` listed): the
  script can never load there. The effective `backends` then leave that class out.
- The hints are not a request to a server and are never sent to one.

<a id="s4-14"></a>
### 4.14 Validation stages, `check` and the load

The rules form four stages:

1. folder rules (§3.3) on the walk;
2. manifest syntax and size (§4.1), then every field rule (§4.2–§4.13);
3. the registry checks: capability names, module names and versions, consistency (§4.7,
   §4.8);
4. compilation of every source file (§4.6).

**`check`** runs all four on each folder, with no dialog and no store. It stops only when
stage 1 fails (nothing else can be read) and otherwise collects every finding of stages
2–4, so an author sees all of them at once; a manifest that does not parse (M01) skips the
field and registry checks, and the source files are still compiled.

**A load** runs the stages inside its phases (§7.3): stages 1 and 2 in phase A, stage 3 in
phase B before any dialog, stage 4 in phase D, after the grant decision and the store open
(one compilation item per source file, D17 §2). A load stops at the end of the first stage
that has an error and reports every finding of that stage. So a load refused in phase A or
B never asks the user and never touches the store, while a compilation error is found only
after the dialog. **A compilation failure in phase D:** the remaining compilation items
are dropped, the apartment's state gets **one close item** (no `Unload` handler, because
no script code ran), the load's memory reservation is released, and the script is not
loaded. The record written in phase B stays; the next load of a corrected folder follows
§9.2 (a content change asks again for group A outside developer mode). Running `check`
before installing finds compilation errors without any dialog.

Each finding has a code (§11.3). A load reports only the first finding in the reserved
layer, as "Not loaded · notes · installed: <reason>", and every finding in the script
manager's details page and the script log.

---

<a id="s5"></a>
## 5. The content hash

<a id="s5-1"></a>
### 5.1 Encoding

The **content hash** of a snapshot is SHA-256 over this byte string (D17 §5.2):

1. the 22 ASCII bytes `tessera-ext-content-v1`, then one zero byte;
2. the number of regular files of the snapshot, as an unsigned 32-bit little-endian
   integer;
3. for each regular file, in byte order of its relative path:
   - the length of the relative path in bytes, unsigned 32-bit little-endian;
   - the relative path's bytes (`/` separators, no leading `/`);
   - the length of the file's content in bytes, unsigned 64-bit little-endian;
   - the content bytes.

- The manifest is one of the files and is hashed like any other.
- Directories, timestamps, permissions and link counts are not part of it.
- Every boundary is length-prefixed, so no byte can move between a path and a content, or
  between two files, without changing the hash.
- It is written as 64 lowercase hexadecimal digits, with `content_hash_version = 1` beside
  it in the grant record (§8.3). The `v1` in the tag and the version number change together
  (§12.6).

<a id="s5-2"></a>
### 5.2 Test vectors

- **Empty snapshot** (not a valid script, but a valid encoding): the 27 bytes
  `746573736572612d6578742d636f6e74656e742d76310000000000` hash to
  `9c9228e0430c7c3388cf616c468d863449738930e0dd8813a3497558dacfcf70`.
- **Two files.** `main.lua` holds the 12 bytes `print("hi")` followed by a line feed.
  `script.toml` holds these 112 bytes (LF line ends, a final LF, no byte-order mark):

  ```toml
  manifest = 1
  id = "hello"
  name = "Hello"
  version = "1.0.0"
  author = "Example Author"
  api = 1
  entry = "main.lua"
  ```

  The encoding is 194 bytes; it begins
  `746573736572612d6578742d636f6e74656e742d7631 00 02000000 08000000 6d61696e2e6c7561
  0c00000000000000` (spaces added here only for reading: the tag, the zero byte, the file
  count 2, the path length 8, `main.lua`, the content length 12). The hash is
  `62eba7b09ef519d9c6a6fa1046fc1f340e2ff7483586b8dfad433deb362f59ab`.
- **One byte added.** With `main.lua` changed to `print("hi!")` and a line feed (13
  bytes), the hash is
  `c50c0fd8eb9fc728c01c7a8db521ff717dab94a719baac111409ddf0c7fc94c0`.

---

<a id="s6"></a>
## 6. Capability catalogue

<a id="s6-1"></a>
### 6.1 Names, groups, backends, slices

Capability names are 1–32 bytes: one or two segments of `a-z` followed by `a-z`, `0-9`,
`_`, joined by `.`. The catalogue below is D17 §3's, with the plain-word text the dialog
shows (Tessera's own strings).

| Capability | Group | Backends | Local-only | Slice | Dialog text |
|---|---|---|---|---|---|
| `ui` | O | Offline, TesseraProtocol | no | 1a | Show its own windows and labels. They take clicks and the mouse wheel where they lie. |
| `input` | O | Offline, TesseraProtocol | no | 1a | Add its own key bindings. They only fire for keys that no built-in binding uses. |
| `world.read` | O | Offline, TesseraProtocol | no | 1a | Read where you are: region, map position, terrain, water, time of day and sky. |
| `entity.read` | O | Offline, TesseraProtocol | no | 1a | See the characters and objects the game knows about near you: names, kinds and positions. |
| `overlay` | O | Offline, TesseraProtocol | no | 1b | Draw lines, boxes and markers over the 3D world. |
| `camera` | **A** | Offline; TesseraProtocol only if the server's policy lists it | no | 1b | Take over the camera while it runs. |
| `session.sandbox` | **A** | Offline only | no | 1b | Teleport you and change the time of day in the offline sandbox. |
| `files.pick` | O | Offline, TesseraProtocol | no | 2 | Read one file that you choose in a file dialog. |
| `data.read` | O | Offline only | **yes** | 2 | Read decoded data from your own game install (names, properties, icons). |
| `audio` | O | Offline, TesseraProtocol | no | 2 | Play sounds from your own game install. |
| `input.drive` | **A** | Offline only | no | 2 | Move your character and press game controls for you. |
| `entity.local` | **A** | Offline, TesseraProtocol | no | 2 | Place objects in the world that only you can see. |
| `ui.native` | **A** | Offline, TesseraProtocol | no | 2 | Show, hide and move Tessera's own panels. |
| `bridge.plugins` | **A** | Offline, TesseraProtocol | **yes** | 2 | List, load and unload your plugins, run their commands, and read their saved data. |
| `session.server` | **A** | TesseraProtocol, and the in-process loopback offline | no | 3 | Exchange messages with the server you are connected to. |

- "TesseraProtocol" applies only from slice 3 (§10.2); before that every capability is
  effectively Offline only.
- A build knows exactly the capabilities of the slices it serves. A name of a later slice
  is "unknown" to an earlier build (§4.7).
- The two group headings the dialog shows are **"Observe and show"** (group O) and **"Act
  for you"** (group A).

<a id="s6-2"></a>
### 6.2 Combination warnings

The dialog shows D17 §5.2's warnings for the set the user is about to grant, recomputed
whenever the user changes a box (§9.4). The pairs and texts are D17 §5.2's table:
`entity.read` with `overlay`, with `ui`, with `audio`; `entity.read` alone on a server;
`session.server` with `files.pick` or `ui`; `input` with `input.drive`; `camera` on a
`TesseraProtocol` server; `camera` with `entity.read`. `check` reports the same pairs for
the manifest's requested set as informational findings (W06).

---

<a id="s7"></a>
## 7. Effective rights and the load pipeline

<a id="s7-1"></a>
### 7.1 The intersection

For one load, the **effective set** is computed in this order (D17 §5.1); every layer only
removes:

| Layer | Removes | Report when a **required** capability is removed |
|---|---|---|
| 1. Build | capabilities of slices the build does not serve | (the manifest is already refused as "unknown capability", §4.7) |
| 2. Server hints | everything, if the session's class is not in `[server] backends` | "This script does not run on this connection" (not loaded, no prompt) |
| 3. Backend class | capabilities not available on the class (§6.1) | "unavailable on this backend" |
| 4. Server policy (slice 3) | names outside `allow`; optional members of complete `forbid` sets (§7.2) | "not allowed by this server"; a complete `forbid` set of required members: "not allowed together with …" |
| 5. Local-only rule | §7.2 | "not allowed together with …" |
| 6. The user's grant | everything the record does not grant (§8) | "not granted" |

A required capability removed by layers 2–5 refuses the script **before** any dialog:
asking the user for something that could not be used would be pointless. Layer 6 is
where the dialog may ask (§9). The effective set is fixed for the apartment's life. A
narrowing during its life, by the script manager (§9.7) or by a re-read of the grant store
(§8.5), suspends the script in that frame (TM §8.23), and natives re-check the grant on
every call (D17 §5.1); a widening, by any route, applies only at the next load.

<a id="s7-2"></a>
### 7.2 Exclusions, in a fixed order

- **Forbidden sets (slice 3).** For each `forbid` set of the server's policy, in the
  policy's order: if every member is in the set still offered, then if all members are
  required, refuse; otherwise withhold optional members, the one with the greatest name in
  byte order first, until the set is no longer complete. The result does not depend on any
  map's iteration order.
- **Local-only versus `session.server`.** If after layer 4 the offered set holds
  `session.server` and a local-only capability: if both are required, the manifest was
  already refused (§4.7); if `session.server` is required, every local-only capability is
  withheld; if a local-only capability is required, `session.server` is withheld;
  if all of them are optional, the user chooses in the dialog (§9.4), and the record never
  grants both (§8.4).

<a id="s7-3"></a>
### 7.3 The load pipeline

A **load** starts from the user (the script manager's "Load" or "Enable", `/ext:reload`)
or from autoload at world entry (D17 §4, "Lifecycle"). Autoload loads the records whose
`autoload` is true and whose folder exists, in the **script-list order**; a script without
a valid record is never autoloaded. The script-list order (used by autoload, the script
manager's list and the dialog queue of §9.1) is: first the valid records that `order`
names, in `order`'s sequence; then the valid records that `order` does not name, in byte
order of `"<kind>/<id>"`. A record created by an Allow is appended to the end of `order`
by that Allow's write; "Move in the list" (§9.7) rewrites `order` with every valid record.
`order` entries of records that are invalid or absent stay in place (§8.4) and are
skipped. Every load runs these phases. The gate state (§10) must be
`Open` when a load starts and stay `Open` until it ends; otherwise the load is cancelled
as §10.4 says.

**Memory reservation (TM §8.10, §8.21; TM L23, L24, L26).** When a load starts, it counts
toward the loaded scripts and apartments (16 and 17, TM L26) and reserves, in the 512 MiB
sum, its record's `memory_mib`, or 32 MiB without a record; a load that does not fit is
refused ("the memory limit of all scripts is reached" or "16 scripts are loaded"). The
snapshot is held under that reservation. After phase A the reservation becomes the
manifest's `memory_mib`; a growth that does not fit refuses the load the same way. The
reservation becomes the apartment's cap in phase D, and a refused or cancelled load
releases it. (A hot reload's new apartment reserves from the reload allowance instead:
TM §8.10, EA1b §8.)

| Phase | Where | What |
|---|---|---|
| **A. Read** | asset worker | The walk (§3.4), the content hash (§5), the manifest parse and field validation (§4.1–§4.13). Result: the snapshot, the hash, the parsed manifest, or the folder or manifest findings |
| **B. Decide** | main thread, native, outside ext time; its file work on workers | Registry checks (§4.7, §4.8); "one loaded script per id" (§7.4); the effective set before the grant (§7.1 layers 1–5); the decision pipeline of §9.2: no dialog, or a dialog that waits for the user. Ends with the record written (§8.5) or the load refused or cancelled. The main thread does no file I/O: the record write is a job on the grant-store queue, and the store probe (§8.8, needed by §9.2 step 2 and §9.3 "Stored values") a job on the store's I/O queue; phase B waits for their completions |
| **C. Open the store** | the (kind, id) store's I/O queue, on a worker | The store open of §8.8: the directory handles and the entry sweep of §3.6 (leftover temporary files and unexpected entries removed), the owner-tag check and the wipes, the durable owner tag and `local-only` marker, the file index (TM rule 7.6.1). Queued only after phase B's record write has completed, and after every earlier job of that store (D17 §8, "`Store` I/O") |
| **D. Snapshot result** | ext queue item (6a) | Delivered only after phase C completed. Then one compilation item per source file, then the top-level run (D17 §2, §4). A compilation failure ends the load as §4.14 says |

- Phases A to C together are D17's "snapshot job": the marker, the owner tag and the wipe
  are never a main-thread step or a 6a item. Phase B sits between them because the store's
  fate depends on the grant decision, which depends on the hash.
- A load that is refused in phase A or B never creates an apartment and never touches the
  store (the store probe only reads).
- The content hash pinned in phase B is the hash of the very bytes that phase D compiles.

<a id="s7-4"></a>
### 7.4 One loaded script per id

At most one script per id is loaded at a time, across both kinds (D17 §2, TM L26). Phase B
refuses a load whose id is already loaded from the other kind, with "notes · developer is
loaded; unload it first", and the script manager offers that unload. A second load of the
**same** (kind, id) while it is loaded or loading is ignored, except `/ext:reload`, which
is a reload (§9.8).

---

<a id="s8"></a>
## 8. The grant store

<a id="s8-1"></a>
### 8.1 File and location

- The grant store is the TOML document `ext-grants.toml` in the **directory of the settings
  file**: the directory of `$TESSERA_SETTINGS_FILE` when that is set and non-empty,
  otherwise `<platform config dir>/tessera/` (on Linux `~/.config/tessera/`). "The
  directory of" means the directory named by the configured path, before any symbolic
  link in it is resolved.
- **(Proposed D17 amendment, OPEN O-10.)** D17 §5.2 says grants live "in Tessera's
  settings". This spec keeps them in the settings directory but in a document of their
  own, so that a damaged or hostile grant document never resets the settings file, and the
  fail-closed reading of §8.4–§8.6 does not have to share the settings file's tolerant
  one. Until the owner agrees, OPEN O-10 says what applies.
- Developer mode is not in this document; it is the settings file's `[ext]
  developer_mode` (§9.8).
- Nothing in it is ever written by a script, by a plugin or through any `Ext` call. Within
  Tessera only the dialog (§9.3), a load's own record update (§9.2 step 6) and the script
  manager (§9.7) change it. The user may also edit the file by hand (TM §4.3 assumption
  3); such an edit is the user's own act, not a grant by Tessera, and Tessera honours it
  only as §8.4 and §8.5 say.

<a id="s8-2"></a>
### 8.2 Document shape

```toml
format = "tessera-ext-grants"
version = 1
order = ["scripts/notes", "scripts-dev/hud"]

[[script]]
kind = "scripts"
id = "notes"
author = "Example Author"
required = ["input", "ui"]
optional = ["world.read"]
granted = ["input", "ui", "world.read"]
denied = []
memory_mib = 32
content_hash = "62eba7b09ef519d9c6a6fa1046fc1f340e2ff7483586b8dfad433deb362f59ab"
content_hash_version = 1
autoload = true

[script.settings]
stamp_position = true
max_notes = 200.0
```

<a id="s8-3"></a>
### 8.3 Fields

The caps of this section are TM L79.

Root:

| Key | Type | Rule |
|---|---|---|
| `format` | string | `"tessera-ext-grants"`; any other value: the whole document is treated as damaged (§8.6) |
| `version` | integer | The grant-store format version, `1` (§12.5) |
| `order` | array of strings | The user's script list order, which autoload follows: entries `"<kind>/<id>"`, unique, at most 256 |
| `script` | array of tables | The grant records, at most 256 |

One record (`[[script]]`):

| Key | Type | Rule |
|---|---|---|
| `kind` | string | `"scripts"` or `"scripts-dev"` |
| `id` | string | §3.2 |
| `author` | string | The manifest's `author` when the record was last decided, exactly; display-string rules (§4.4) |
| `required`, `optional` | arrays of strings | The manifest's capability lists when the record was last decided (the **pinned capability list**): capability-name grammar (§6.1), each at most 16 entries, sorted in byte order, unique, disjoint |
| `granted`, `denied` | arrays of strings | The user's decisions: sorted, unique, disjoint, each a subset of `required` ∪ `optional`. A capability in neither is **undecided** |
| `memory_mib` | integer | 32–64: the memory the user accepted |
| `content_hash` | string | 64 lowercase hexadecimal digits (§5) |
| `content_hash_version` | integer | `1` |
| `autoload` | boolean | The user's "Load automatically" choice |
| `settings` | table | The `[[settings]]` values the user set: key (settings-key grammar) → boolean, number or string (at most 256 bytes, display-string character rules, may be empty); at most 32 entries. A number is a finite TOML float, or a TOML integer of magnitude at most 2^53, read as the equal float; the writer always writes numbers as floats (`200.0`). Absent keys take the manifest's default |
| `signing_key` | string | Reserved for slice 4 (D17 §5.7): `"ed25519:"` and 64 lowercase hexadecimal digits. A build before slice 4 keeps it unchanged and does not use it |

- **Invariant of every valid record:** `granted` never holds `session.server` together
  with `data.read` or `bridge.plugins`.
- **No other per-script state** is kept here or anywhere (D17 §5.2): no window geometry,
  active context, lease or UI preference.
- Within Tessera, a record is created only by the first **Allow** (§9.3), or by the
  `test` tool root's default answer (§11.4), and exists until "Remove" or "Reset" (§9.6,
  §9.7). Deny and "Not now" never create one. A record the user wrote by hand is read
  like any other (§8.4).

<a id="s8-4"></a>
### 8.4 Validation (the store is hostile input)

Users edit files, so the document is read bounded and strictly per record, and has a fuzz
target (§11.7):

- at most 4 MiB, nesting at most 4 levels below the root (as §4.1 counts), at most 256
  records, the per-field caps above (TM L79);
- unknown keys at the root and in a record are **kept** and written back unchanged (so a
  newer Tessera's fields survive an older one), but never interpreted;
- a record is **invalid** when any rule of §8.3 fails, including the invariant above. An
  invalid record is ignored for the session, as if absent, and reported once in the script
  manager ("The saved permissions of notes · installed are damaged and were ignored");
- two records with the same (kind, id) are **both** invalid;
- `order` entries that name no record, or are malformed, are ignored.

A **valid** record is honoured however it was written; Tessera cannot tell, and does not
try to tell, a record the user edited by hand from one it wrote. A hand edit therefore
never widens more than the user's own edit says, and only within these bounds: a record
that breaks §8.3 (for example one granting `session.server` with a local-only capability,
or a capability outside its pinned list) grants nothing; a re-read never widens a running
apartment (§8.5); and the next load compares the record's `author`, pinned capability list
and content hash with the folder as §9.2 does, so the record is honoured only for the code
and lists it names.

An ignored record grants nothing, so damage always fails closed: the next load of that
script is a first enable (§9.2 step 1). The store it finds is kept only if its owner tag
names the loading manifest's `author`; otherwise the store open wipes it (§8.8, TM §8.24),
and the dialog says which of the two will happen (§9.3, "Stored values").

<a id="s8-5"></a>
### 8.5 Reading and writing: the grant table and the grant-store queue

No blocking file I/O happens on the main thread after start-up (ARCH §21), and D17 keeps
all ext file I/O on workers. So:

- **The grant table.** The ext host keeps, on the main thread, an in-memory copy of the
  document's valid records, `order` and unknown keys. Phase B, the dialogs, the script
  manager and the natives' per-call grant checks read only the grant table. A change
  (a dialog's Allow, a load's record update, a manager action) is applied to the grant
  table at once and submitted as a write job.
- **The grant-store queue.** Every read and write of `ext-grants.toml` is a job on one
  serial queue that runs on an asset worker, one job at a time, in submission order.
- **Reads.** The document is read at start-up (the gate does not leave `Starting` before
  the result is applied, §10.3) and each time the script manager opens (to pick up hand
  edits). A read result replaces the grant table; the changes the main thread submitted
  after that read was queued are applied again on top, so a read never loses a decision.
- **A re-read is a revocation route (TM §8.23).** When a read result is applied while
  scripts are loaded, the host compares, for each loaded or suspended apartment, its
  effective set with the `granted` list of its (kind, id)'s record in the new grant table:
  - **Narrowed:** a capability of the effective set is no longer granted, because the
    record was removed, became invalid (§8.4, including a duplicate, or every record of a
    damaged or newer document, §8.6) or no longer grants it. That is a revocation of those
    capabilities: the script is **suspended in the frame the result is applied**, exactly
    as §9.7's "Uncheck" says (a revoked required capability: unloaded with one close item
    and no `Unload` handler; only optional ones revoked: the user may resume it, and the
    revoked modules then raise `"not granted"`). The script manager says "The saved
    permissions of notes · installed changed; the script was suspended".
  - **Widened** (a capability granted that the effective set lacks) or otherwise changed
    (`author`, pinned lists, hash, memory): nothing changes for the running apartment.
    The new record is used from the script's next load, which runs §9.2 against it.
  - A changed `settings` value reaches a loaded script as a change the user applied
    (§4.12); `autoload` and `order` take effect at their next use.
- **A write job** carries only what changed: whole records set or removed, `order`. It
  takes the grant store's own lock (below), reads the current document (with §8.4's
  bounds), replaces, inserts or removes exactly those records and entries (keeping every
  other record, every unknown key and other Tessera processes' changes), writes a
  temporary file in the destination's directory, flushes it to disk, renames it over the
  document, flushes the directory entry, releases the lock and reports completion to the
  main thread. A reader never sees a half-written document, and a completed write survives
  a crash.
- **Lock and temporary file (named).** The lock is the file `.ext-grants.toml.lock` in the
  directory of the **resolved** `ext-grants.toml` (the settings crate's lock naming,
  `.<file name>.lock`, applied to this document; never the settings file's own
  `.settings.toml.lock`, so a grant write and a settings save never wait on each other).
  It is an advisory lock taken on that file's handle; the file is created if missing, is
  never opened through a symbolic link, passes the install guard, and is **never
  removed** (removing it would race with other lockers), so it persists after the first
  write. The temporary file is created exclusively beside the resolved document with the
  settings crate's atomic-write naming, `.ext-grants.toml.tmp-<pid>-<nanos>-<n>`; it is
  renamed over the document on success and removed when the write fails, so only a crash
  leaves one behind. Backups follow §8.6.
- **Links and the install guard.** Symbolic links in the final path component are
  followed (at most 40) to the file that is really replaced, and the settings crate's
  read-only install guard must accept the resolved destination before anything is written
  (KM §6.5's rules).
- **Output form.** Records are written sorted by kind, then id, in byte order; lists
  sorted; numbers of `settings` as floats. A document that Tessera wrote, read and written
  again without change, gives the same bytes apart from unknown keys' formatting.
- **Before the store is opened.** A load's record write (§9.2 step 6) must **complete**
  before phase C is queued, so a crash never leaves a store opened under a grant that the
  document does not hold. If the write fails (an I/O error, the install guard, the lock),
  the change is taken back out of the grant table, the load is cancelled, and the manager
  reports "Permissions could not be saved: <reason>".
- **Newer document** (§8.6): no write job is ever submitted; changes live in the grant
  table for this session only, and phase C follows the change to the grant table.
- **Manager actions** (§9.7) take effect in the grant table at once (so a revocation acts
  in that frame) and are written in the background; a failed write is reported and the
  grant table keeps the narrower state for the session.

<a id="s8-6"></a>
### 8.6 Damaged, missing and newer documents

| Case | Reading | Writing |
|---|---|---|
| Missing | Empty: no records | Created by the first write |
| Not UTF-8, not TOML, over 4 MiB, wrong `format`, nesting too deep | Every record ignored (fail closed) | Before the first write, the damaged file is copied to `ext-grants.toml.corrupt-<unix seconds>[-n].bak` beside it (at most 8 such backups, the oldest removed first, as the settings crate keeps its own; TM L79), then a fresh document is written |
| `version` newer than the build reads | Every record ignored; the script manager says "Permissions were saved by a newer Tessera and are not used" | **Never written** by this build, so the newer build's data is kept. Dialogs still show; their decisions apply to this session only (§8.5) |
| Some records invalid | Those ignored (§8.4) | The file is backed up once as above before the first write that would drop them |

<a id="s8-7"></a>
### 8.7 Records, stores and orphans

- A (kind, id) has one record and one `Store` (D17 §5.2). The store directory is created by
  phase C's first open after the record exists; they are removed together.
- **Orphan record:** a record whose folder `data/<kind>/<id>/` does not exist or is not a
  directory when the script manager lists scripts. It is listed with its stored-value size
  and a "Remove" button.
- **Orphan store:** a store directory `data/script-store/<kind>/s_<id>/` with no valid
  record. It is listed the same way.
- **Remove** (TM §8.24) first queues the store's deletion on its I/O queue; when that job
  has completed, it removes the record (a grant-store write) and the id's bindings from
  `keymap-ext.toml`, unless a record of the other kind with the same id remains (D17 §2).
  A crash between the two leaves a record with no store, which is harmless. The store's
  deletion is the wipe of §8.8 followed by a removal of the then empty `s_<id>` directory
  relative to its `<kind>` handle; an unusable store directory (§3.6) is reported and
  nothing is removed, and the record stays.
- **The bindings' removal** **(design addition)** is a read-modify-write of
  `keymap-ext.toml` on a worker, made as §8.5's write job is, with KM §6.3's strict reader
  and KM §6.5's saving rules: the lock `.keymap-ext.toml.lock` in the directory of the
  resolved document (persistent, never removed, as in §8.5), and a temporary file
  `.keymap-ext.toml.tmp-<…>` beside it, renamed on success and removed on failure. The
  binding UI's own saves follow KM §6.5 and create the same temporary-file form.
- Orphans count toward the `Store` totals (TM L40, L41); a write refused by a total names
  them (D17 §5.2).

<a id="s8-8"></a>
### 8.8 The store open, the store probe, the owner tag and the marker

Phase C opens the (kind, id) store on its I/O queue, in this order (D17 §5.2 rule 3, D17 §8
"`Store` I/O", TM §7.6, §8.7, §8.8, §8.24). Every step works through the handles and the
per-entry rules of §3.6:

1. **Handles.** The `data/script-store/` and `<kind>` handles are opened, then `s_<id>`
   (§3.6 steps 1–3); a missing `s_<id>` is created. An unusable directory (§3.6 step 4)
   refuses the load, and nothing is read, written or removed.
2. **Entry sweep.** The directory is listed once, and each entry is classified and handled
   as §3.6's table says, before anything is read: crash leftovers (`t_<key>.tmp`, TM §8.7)
   and every unexpected entry are removed with a no-follow removal; a subdirectory, or an
   entry that cannot be removed, refuses the load (fail closed). The sweep notes whether a
   `local-only` entry of any kind was present.
3. **Owner check.** The owner tag `owner` is read through a no-follow open (at most 256
   bytes, TM L89). A tag that is missing (including one the sweep removed), unreadable,
   over 256 bytes, or not byte-equal to the loading manifest's `author` causes a **wipe**,
   and the script manager reports "Stored values of another author were erased". A new
   directory has no tag and nothing to wipe. An `author` change that the user allowed
   (§9.2 step 1) therefore always wipes, in every mode.
4. **Local-only check.** If the store held a `local-only` entry (step 2) and the
   apartment's effective set holds `session.server`, the store is wiped (D17 §5.2 rule 3).
5. **Durable tags.** If the directory is new or was wiped, the owner tag is created
   exclusively and written durably (written, flushed to disk with its directory entry):
   the `author`'s UTF-8 bytes, nothing else. If the effective set holds a local-only
   capability, or the sweep removed a `local-only` entry that was not a plain file and the
   store was not wiped, a plain `local-only` marker is created exclusively and written
   durably. Both are written before the open completes, so before the top-level run and
   before any value file of the new apartment can be flushed.
6. **File index.** The key files the sweep kept are listed into the file index (TM rule
   7.6.1), each with its counted size (TM §8.7).

A **wipe** removes every entry of `s_<id>` (key files, temporary files, the marker and the
owner tag) with the no-follow removal of §3.6, never entering a subdirectory (a
subdirectory or an entry it cannot remove refuses the load, so nothing it should have
removed is ever opened), and empties the pending-write map and the file index of that
store (TM rule 7.6.4). It runs after every earlier job of the store's queue, so after the
old apartment's final flush. It never removes `s_<id>` itself, and never anything outside
it.

The owner tag and the marker are host files: no `Store` key can name them, they are never
listed by `Store`, and they do not count toward the `Store` key or byte totals.

**The store probe** is the read-only job that phase B needs (§9.2 step 2, §9.3 "Stored
values"). It runs on the store's I/O queue, behind every earlier job of that store, and
uses §3.6's handles and no-follow opens, but it **never creates, removes or writes**
anything: a missing `s_<id>` gives "no store"; an unusable directory or a subdirectory
gives "unusable" (phase B then refuses the load before any dialog, since its open would
refuse it; §9.2 step 2, §9.3 "Stored values"); entries the open would remove are skipped. Its result is: whether a
store exists, whether its owner tag is a plain file of at most 256 bytes byte-equal to the
manifest's `author` (**matches**), and the number of plain key files and their total
counted size. The open of steps 1–6 re-checks everything and is what decides.

---

<a id="s9"></a>
## 9. The grant UI flow

<a id="s9-1"></a>
### 9.1 Principles

- **Only the user grants**, through the consent dialog in the reserved layer, under D17
  §5.3's rules and TM §8.25 (TM L65): plain text (TM §8.2), the id and kind beside every
  script string, Escape only refuses, `ui.dialog` and `ui.eat_keyboard` pushed while it
  shows, ext windows, labels and overlays neither drawn nor hit-tested while it shows.
- **The pointer-only "Allow".** A press on "Allow" counts only when all of these hold: the
  press comes at least 500 ms after the dialog appeared; the pointer **entered** the
  button, as a pointer move from outside the button to inside it **observed after the
  500 ms delay ended**; the pointer stayed inside from that entry to the press; and the
  release follows on the button. A pointer that entered during the first 500 ms and rests
  there, or that was over the button when the dialog appeared, must leave and enter again.
  No key activates it.
- **No script can raise a dialog.** Dialogs come only from loads that the user starts, from
  autoload at world entry (§7.3), and from the user's own actions in the script manager
  (§9.7). A script's own actions never cause a load.
- **Tessera widens only through the dialog**, including a widening started in the script
  manager (§9.7). Narrowing never needs one. A record the user edits by hand is the
  user's own act (§8.1): it is honoured only if valid (§8.4) and only from the script's
  next load (§8.5).
- **One dialog at a time.** Dialogs for several loads (an autoload of several scripts) are
  queued in the user's script-list order and shown one after another.

<a id="s9-2"></a>
### 9.2 The decision pipeline

Phase B decides with an ordered pipeline. Its inputs are the manifest and content hash of
phase A, the record of the (kind, id) in the grant table (valid, or none), the effective
set before the grant (§7.1 layers 1–5, called the **offered set** below), who started the
load (the user or autoload), the kind and developer mode. Every step runs, in order; a
step that ends the load says so.

**Step 1: baseline.** The first matching case sets the working decisions and the dialog
kind:

| Case | Working decisions | Dialog kind | Store |
|---|---|---|---|
| 1a. No valid record | all undecided | first enable | the owner check of §8.8 decides; the dialog says which (§9.3) |
| 1b. `author` differs from the record's | all undecided | changed script | wiped at the open (§8.8 step 3) |
| 1c. `required` or `optional` differs from the pinned list | all undecided | changed script | kept (except §8.8 step 4) |
| 1d. Otherwise | the record's `granted` and `denied` | none yet | kept (except §8.8 step 4) |

**Step 2: content** (only after case 1d). The content changed when the hash differs from
the record's, or when `content_hash_version` differs and the build can no longer compute
the record's version (§12.6; when it can, it compares that version's hash).

- Kind `scripts-dev` with developer mode on: the decisions are unchanged; the record's
  hash is to be updated.
- Otherwise: every **granted** group-A capability becomes **undecided** (D17 §5.2); group-O
  decisions are kept; the record's hash is to be updated; and a **content notice** is due
  (§9.6) if a group-O grant or stored values are kept. "Stored values are kept" is decided
  by the **store probe** (§8.8), which phase B runs here, before step 3: the probe found a
  store whose owner tag matches the manifest's `author` and that holds at least one key
  file. The notice shows the probe's total counted size. A probe that finds no store, or a
  store of another author (to be wiped at the open), keeps no values; an unusable one
  refuses the load here, as at the open (§3.6 step 4).

**Step 3: memory.** If the manifest's `memory_mib` is above the record's (or above 32 with
no record), the new amount must be asked. If it is lower, the record's value is to be
lowered, silently.

**Step 4: a denied required capability** (only after case 1d). If a required capability
of the offered set is **denied** in the working decisions (the user revoked it in the
manager, §9.7):

- a user-started load: the dialog kind becomes **first enable again**, which asks every
  capability of the offered set with the earlier choices preset (and anything steps 2 and
  3 found);
- autoload: the load ends, not loaded, no dialog, nothing written; the manager shows
  "Not loaded: you denied <capability>".

**Step 5: the dialog.** The **asked set** is every capability of the offered set for the
kinds first enable, changed script and first enable again; otherwise the offered
capabilities that are undecided. If no kind is set yet:

- a capability that step 2 made undecided is in the asked set: **update** dialog;
- otherwise, the asked set is not empty: **more permissions** dialog;
- otherwise, step 3 asks for memory: **memory** dialog;
- otherwise: no dialog.

Any dialog also asks for the memory of step 3. An optional capability that is **denied**
is never in the asked set of the last three kinds; the user can change it in the manager
(§9.7). An undecided capability that is not offered (for example `world.read` while it is
unavailable on the backend) stays undecided and is asked when a later load offers it.

**Step 6: the record write and the outcome.**

- **No dialog.** If step 2 or 3 left something to update, or stored settings values must
  be dropped (§4.12), the record is written (§8.5) with the manifest's hash and
  `memory_mib` and the working decisions (so a group-A grant that step 2 made undecided
  leaves `granted`). The write completes before phase C. The content notice of step 2, if
  due, is shown with this write, so it appears once per content change, not at every load.
- **A dialog.** The outcome is §9.3's table. Only **Allow** writes: the working decisions
  with the dialog's answers, the manifest's `author`, capability lists, hash and
  `memory_mib`, and the "Load automatically" box. The write completes before phase C.
  Deny, "Not now" and Escape write nothing and end the load, not loaded.

So every load ends phase B either refused or cancelled with nothing written, or with the
grant table and (unless the document is newer, §8.6) the file holding exactly the decision
that phase C opens the store under.

<a id="s9-3"></a>
### 9.3 The dialog

Every dialog kind uses one layout; only the title, the asked capabilities and the button
names differ. All texts except the script's display strings are Tessera's own.

| Part | Content |
|---|---|
| Title | First enable and first enable again: "Allow this extended script?" · Changed script: "This extended script has changed. Allow it again?" · Update: "This extended script was updated. Allow it to act for you again?" · More permissions: "This extended script asks for more." · Memory: "This extended script asks for more memory." |
| Script line | name, version, then the fixed-style id and kind ("notes · installed", "hud · developer"); author; description (at most 8 lines). A developer script also shows the developer badge |
| "Observe and show" | Each group-O capability asked, with the dialog text of §6.1 and the manifest's reason |
| "Act for you" | Each group-A capability asked, the same way |
| Kept grants | In an update dialog: "Still allowed: …" with the kept group-O names (read-only) |
| Not available here | Requested capabilities removed by layers 2–5 (§7.1) that were optional, each with its reason ("offline only", "not allowed by this server"); read-only |
| Memory | "Memory: up to 48 MiB" when `memory_mib` is above 32 or changed |
| Stored values | Case 1a: phase B first runs the **store probe** of §8.8 on the store's I/O queue (whether a store exists, its size, and whether its owner tag matches the manifest's `author`). With no store, no line. An unusable store directory (§3.6 step 4) refuses the load before any dialog, with §3.6's report and nothing written. Otherwise it shows "Stored values found (1.2 MiB). They will be kept." when the tag matches, or "Stored values of another author were found (1.2 MiB). They will be erased." when it does not. Case 1b, and a switch between a local-only capability and `session.server` (§9.4): "This script's stored values will be erased." No option to keep them in these cases (D17 §5.2, TM §8.24). The open of §8.8 re-checks and is what decides |
| Warnings | The combination warnings of §6.2 for the boxes as currently set |
| Load automatically | A box, initially the manifest's `autoload` at first enable, otherwise the record's value |
| Buttons | **Allow** and **Deny** (first enable, first enable again, changed script); **Allow** and **Not now** (update, more permissions, memory) |

**Controls.**

- A **required** capability has no box: Allow grants every asked required capability, and
  Deny refuses the script.
- An **optional** capability has a box. Group-O boxes start checked and group-A boxes start
  unchecked, except that in a "first enable again" dialog the user's earlier choices are
  preset.
- A **local-only choice** (§9.4) is a pair of mutually exclusive choices.
- Boxes, choices and "Load automatically" follow exactly Allow's input rule of §9.1 (TM
  §8.25, TM L65): a press counts only at least 500 ms after the dialog appeared, after a
  pointer entry into **that** control observed after the delay ended, with the pointer
  inside from that entry to the press. No key changes them.

**Outcomes.**

| Button | First enable, first enable again, changed script (a load) | Update, more permissions, memory (a load) | Started from the script manager (§9.7; no load) |
|---|---|---|---|
| Allow | Record written (§9.2 step 6): asked required and checked optional capabilities granted, unchecked optional denied, pinned list, author, hash, memory, autoload. The load continues with phase C | Asked capabilities as on the left; other decisions unchanged; hash and memory as the manifest. The load continues | Asked capabilities written; nothing loads. On a loaded script the new grant applies at the **next** load, and the page says so. After a local-only switch, §9.4 |
| Deny / Not now | Nothing written; not loaded. A changed script's old record stays unchanged | Nothing written; not loaded this time. The next autoload or user-started load asks again | Nothing written |
| Escape | as Deny | as Not now | as Not now |
| Dialog closed by the gate (§10.4) | as Not now, whatever the kind | as Not now | as Not now |

After Allow, a required capability is never unchecked, because it has no box; so an
allowed load always holds every required capability of the offered set.

<a id="s9-4"></a>
### 9.4 The local-only choice

When the offered set holds `session.server` and a local-only capability, and §7.2 left the
choice to the user (all of them optional), the dialog shows one choice instead of their
boxes: "Exchange messages with the server" **or** "Use local data (decoded game data,
plugins)", plus "Neither". The text explains: "A script can either talk to the server or
read your local data, never both, so that your local data never reaches a server."

- If the record's granted set moves from a local-only capability to `session.server`, the
  dialog states "This script's stored values will be erased", and the next store open
  wipes the marked store before the new apartment opens it (§8.8 step 4; D17 §5.2 rule 3).
- Such a switch is never applied to a live apartment. Started in the manager (§9.7), it
  unloads the script first (the ordinary unload items, with the final flush), then shows
  the dialog; after Allow, if the script was loaded, a user-started load follows, whose
  phase C runs after the old apartment's final flush (D17 §8).

<a id="s9-5"></a>
### 9.5 Re-asking, summarised

| Change since the record was decided | Installed (`scripts`), or developer with developer mode off | Developer (`scripts-dev`) with developer mode on |
|---|---|---|
| `author` | Every grant asked again; the store is wiped at the next open (§8.8) | Same |
| Capability list (required or optional) | Every grant asked again; store kept (except the local-only rule) | Same |
| Content only (hash) | Group O kept (notice with Review); granted group A asked again before the script next loads | Nothing asked; grants kept |
| `memory_mib` raised | Asked | Asked |
| `memory_mib` lowered | Updated silently | Updated silently |
| `version` field only | A content change like any other (the manifest is hashed) | Nothing asked |
| A signing key differs (slice 4) | A different script: empty grants and an empty store (D17 §5.7) | Same |

Before slice 4, a new folder that copies an installed script's id, `author` and capability
list cannot be told from an update, so it keeps group-O grants and the store; the notice of
§9.2 step 2 is what tells the user (D17 §5.2).

<a id="s9-6"></a>
### 9.6 The content-change notice and Review

- The notice is a reserved-layer notice (hit- and hover-transparent, D17 §5.3): "notes ·
  installed was updated. Still allowed: Show its own windows…, Read where you are….
  Stored values kept (1.2 MiB)." The size is the store probe's (§9.2 step 2); the stored
  values sentence is left out when the probe kept no values. It is rate-limited like
  every notice (TM L67).
- "Review" is not on the notice (notices take no clicks): it is the script's page in the
  script manager, which the notice names. The page lists the kept grants and the store size
  and has **Reset**, which unloads the script first if it is loaded, then empties the store
  (on its I/O queue) and, when that has completed, removes the record. The next load is a
  first enable.

<a id="s9-7"></a>
### 9.7 The script manager's grant pages

The script manager (D17 §5.3, §7) shows, per (kind, id): the state (not enabled, loaded,
suspended, not loaded with the reason, orphan), the granted, denied and undecided
capabilities with their groups, memory, autoload, signature state (slice 4), store size,
and the settings page.

| User action | Effect |
|---|---|
| Uncheck a granted capability (revoke) | The grant table changes at once, the write follows (§8.5). On a loaded script, required or optional, the script is **suspended in that frame** (TM §8.23; D17 §4; a re-read of the grant store that narrows a loaded script's record does the same, §8.5): its pending intents are discarded, its leases end (TM §8.17), nothing accrues, and no call runs until the user resumes it. A revoked **required** capability cannot be resumed: the script is unloaded (one close item, no `Unload` handler; D17 §2), and its next user-started load asks again (§9.2 step 4). After a revoked **optional** capability the user may resume the script; from then every native of that module raises `"not granted"`, `Ext.Has` answers false, and what the module made is gone (EA1a §2.9) |
| Check a denied or undecided capability (widen) | Opens the more-permissions dialog for it (§9.3, "Started from the script manager"). It asks only for capabilities of the record's pinned list, each with its dialog text and, when the folder's current manifest has the same pinned list, its reason. On a loaded script the new grant applies at the **next load** |
| Switch between a local-only capability and `session.server` | §9.4 |
| Change "Load automatically" | Written at once |
| Move in the list | Writes `order` |
| Reset | §9.6 |
| Remove (orphans) | §8.7 |
| Change a setting value | Written at once; a loaded script sees it as §4.12 says |

<a id="s9-8"></a>
### 9.8 The developer folder and developer mode

- **Developer mode** is the settings file's `[ext] developer_mode` (boolean, default
  `false`), changed only in Tessera's own settings page, never through any `Ext` call or
  keymap. (Other `[ext]` keys of the settings file, such as the panic chord, belong to the
  API specs; TM L64.)
- **With developer mode on:**
  - `data/scripts-dev/` is listed, and its scripts can be loaded (kind `scripts-dev`);
  - a permanent developer badge shows in the reserved layer (D17 §5.2), and every dialog
    and notice about a developer script shows "developer";
  - a content change of a developer script keeps every decision (§9.5); its record's hash
    is updated when it loads;
  - hot reload (slice 1b, EA1b §8) exists only for developer scripts: by `/ext:reload <id>`
    or by the file watcher. The host first validates the new manifest exactly as a load
    would (§4.14 stages 1–3, including the registry check of required majors and minors);
    a manifest a load would refuse ends the reload as a failure and the old apartment
    keeps running. The reload is **hot** (the store stays open and a hand-off table may
    pass) only when the new manifest has the same `manifest`, `author`, `api`, required
    and optional capability lists, `memory_mib`, `[[actions]]`, `[[contexts]]`,
    `[modules]`, `[optional_modules]` and `[server]` table as the running one (TM §8.24,
    D17 §10). Any difference makes it **full**: an unload, the pipeline of §9.2, and a
    load without a hand-off table; an `author` difference also wipes the store before the
    new apartment opens it (§8.8 step 3);
  - the developer console's REPL binds only to developer apartments (D17 §10).
- **`/ext:reload` of an installed script** (developer mode on) is always a full reload:
  unload, phase A, the pipeline of §9.2 (so a changed hash re-asks group A before new code
  runs), and a load without a hand-off table.
- **`/ext:reload` is a developer-mode command** (D17 §3, EA1b §8.1). With developer mode
  off, every `/ext:reload` line, whatever its id, is refused with the reserved-layer notice
  "Developer mode is off"; nothing is unloaded, read or loaded. With developer mode on, an
  `/ext:reload <id>` whose id names no **loaded** script (not loaded, unloaded after a
  suspension, only listed, or unknown) produces the notice "No loaded extended script
  '<id>'" (D17 §3; the id shown as plain text, TM §8.2) and starts no load: a reload never
  loads a script that was not running, and the user loads it from the script manager. A
  loaded script that is **suspended** is not reloaded: the notice is "notes is suspended:
  resume it first" (a suspension during a reload cancels it, EA1b §8). The file watcher
  follows the same rules (EA1b §8.2: events for a script that is not loaded queue
  nothing).
- **Turning developer mode off** unloads every loaded developer script (the ordinary unload
  items) and stops listing `data/scripts-dev/`. Their records stay. Turning it on again
  shows them; their next load follows §9.2 with developer mode on.
- **Developer scripts still need their grants once:** Tessera never grants without a
  dialog, in any mode (the only exception is `scripttool test`'s tool root, §11.4; a
  valid record the user wrote by hand is the user's own act, §8.4).

---

<a id="s10"></a>
## 10. Backend gate states

<a id="s10-1"></a>
### 10.1 Classes and admission

The class of a session is decided by `Session` from `tessera-net`'s sealed, crate-private
trait of its own concrete backends, never reported by a backend (D17 §6.1): `Offline` (the
in-process sandbox), `TesseraProtocol` (Tessera's own protocol, the project-owned test
server only), `Other` (anything else, including any wrapper of a `tessera-net` backend).

<a id="s10-2"></a>
### 10.2 Admission by slice

| Build | `Offline` | `TesseraProtocol` | `Other` |
|---|---|---|---|
| Client without the `ext` feature | — (no ext code) | — | — |
| Slices 1a–2 | admitted | **not admitted** (treated as `Other`), even if M7's backend exists | not admitted |
| Slice 3 and later | admitted | admitted, under the server's script policy (§7.1 layer 4) | not admitted |

No setting, flag, environment variable, manifest field or grant changes this table (D17
§6.2).

<a id="s10-3"></a>
### 10.3 States

The ext host is always in exactly one gate state:

| State | Entered when | Apartments | Loads | Ext steps 6a / 6b / 6c | Dialogs | Script manager |
|---|---|---|---|---|---|---|
| **Off** | At start-up; when `Draining` ends; for the whole life of a session whose class is not admitted | none exist | none start; a user's Load is refused with "Extended scripts are not available with this connection" | not run | none shown; none queued | lists scripts and records read-only, shows the gate state |
| **Starting** | An admitted session has connected (start-up step 8: its class is known), and the UI is not yet ready or the grant table has not yet been read (§8.5) | none | user-started loads wait (queued in order) | not run | none shown | as Off, with "starting" |
| **Open** | `Starting`, the UI is ready (start-up step 9) and the grant table has been read | loaded and loading ones | allowed (§7.3); autoload queued at **world entry** (the session's first entry into the world after its `Connect`, once per session) | run, under the per-frame rules of EA1a §2.6 and TM L5–L9 (cap and slices, L5–L7; heavy-frame gate, L8; floor frames, L9 and TM rule 7.4.1) | allowed | full |
| **Draining** | A backend switch is queued, or the session disconnects | existing ones are being unloaded | none start; loads in progress end as §10.4 says | 6a runs the unload and close items at the head of the ext queue, under the 6a slice (TM L6), the heavy-frame gate (TM L8) and its floor frames (TM rule 7.4.1, TM L9); **6b does not run**; 6c runs | the shown dialog closes as "Not now"; queued ones are dropped | read-only, "unloading" |

Transitions:

- `Off` → `Starting`: the new session is connected and its class is admitted.
- `Starting` → `Open`: the UI is ready and the grant table has been read. Waiting
  user-started loads start, in order.
- `Open` → `Draining`: a switch is queued (`Command::Connect` for any backend) or the session
  disconnects. This is the moment TM rule 7.4.2 calls "queued". In that frame the session
  generation rises (at every `Connect` and every disconnect, D17 §4), every pending intent
  is discarded, and every lease ends (TM §8.17).
- `Draining` → `Off`: the last close item has run and no load phase of the old session is
  still in progress except store work already started on a store's I/O queue (it
  completes there; its result is discarded, TM §8.21). Then `Command::Connect` is
  submitted for the new backend, if any.
- `Starting` → `Off`: the session disconnects before `Open`.

A session of a class that is not admitted never leaves `Off`. A switch from such a session
needs no draining (nothing exists).

<a id="s10-4"></a>
### 10.4 Loads, reloads, dialogs and the timeout at a transition

This states TM §8.21 (partial loads) and TM rule 7.4.2 (the switch timeout) for this
spec's phases:

| Where the load or apartment is when `Draining` begins | What happens |
|---|---|
| Phase A running or its result not yet consumed | The snapshot job is cancelled and its result discarded when it arrives. No apartment exists, nothing to close; the load's memory reservation is released |
| Phase B before its dialog, or with a dialog showing or queued | The dialog closes as "Not now", or is never shown (nothing written); the load is cancelled. A store probe already submitted completes on the store's queue and its result is discarded |
| Phase B past its dialog (answered with Allow, or none needed), waiting on its grant-record write or on its store probe (TM §8.21) | The write or probe completes in order on its queue: a record the user decided, or the load's own update (§9.2 step 6), is written whole, never half, and stays. Then the load is dropped with its reservation released, before phase C: no store open, no compilation, no top-level run. The next load in a later session follows §9.2 against the written record |
| Phase C queued or running | It completes on the store's queue (file work is never abandoned half-way, and the queue keeps its order for later loads); its result is discarded |
| Phase D: snapshot result or compilation items, top-level run not started | Remaining compilation items are dropped; if the apartment's state exists, it gets **one close item** (no `Unload` handler, because no script code ran); it counts among the at most 17 apartments (TM L26) |
| Top-level run finished (the script is loaded) | The ordinary unload: its `Unload` handler item, then its close item (or the close alone if it is suspended) |
| A hot reload in progress (slice 1b) | As EA1b §8 says: the new apartment as the row above that matches its phase; the old apartment as a loaded script |
| **Switch timeout** (TM rule 7.4.2, TM L10): the switch has been queued for 2 s of wall time (120 frames in deterministic mode) | Every apartment whose `Unload` handler has not started gets its close item **without** the `Unload` handler, exactly as for a suspended script (D17 §4): its `Store` writes are flushed at the close, and its script log records "unloaded without Unload: backend switch timed out". An `Unload` call already running finishes within its own deadline (TM L3) |

**Bounds.** Unload and close items obey the 6a slice like every item (D17 §4). Without
gating, at least one item runs per ungated frame (TM L6), so `Draining` lasts at most 34
ungated frames (17 `Unload` items and 17 close items). Under permanent gating, floor frames
run exactly one 6a item each and the timeout closes the rest, so a switch completes within
TM L11 (2 s + 17 × (250 ms + one frame), about 6.25 s; 392 frames in deterministic mode).
TM X85 and X87 test this. (This closes the former OPEN O-5.)

<a id="s10-5"></a>
### 10.5 What the gate does not change

- Records, stores, logs and keymap bindings are untouched by any transition.
- Per-script suspension (watchdog, aborts, panic chord, revocation, a changed process
  locale) is not a gate state; it is defined in EA1a §7.11 and §2.9, TM §8.23 and D17
  §4–§5.
- An `/ext:` line in any state other than `Open` produces the notice "No loaded extended
  script '<id>'" (D17 §3), never a chat line, and is never sent to a backend.

---

<a id="s11"></a>
## 11. The script tool `tessera-scripttool`

<a id="s11-1"></a>
### 11.1 General

- An app (`apps/scripttool`) over `tessera-ext-host`. It needs no game install and no GPU,
  and it never reads `TESSERA_GAME_DIR`.
- **It never touches the user's own Tessera directories.** Every command that loads a
  script works in a fresh temporary **tool root** (a directory with `data/`, `logs/` and a
  config directory), removed at exit unless `--keep-root` is given. `--root DIR` uses a
  given empty directory instead; a non-empty one is refused.
- **Script text reaching a terminal is escaped (design addition**, adopting D17 "Other
  improvements", script log): every string that comes from a script, a manifest, a walk
  file or a path is written with C0 and C1 control characters (U+0000–U+001F except TAB,
  U+007F–U+009F), U+2028, U+2029, a backslash, and the characters §4.4 refuses replaced by
  `\u{XXXX}` escapes (a backslash as `\\`), and every line the tool prints starts with its
  own prefix, so no output can carry terminal escape sequences or forge a line. (The
  script **log file** has its own format, TM §8.1.)
- **Exit status:** 0 success; 1 findings or failures (an error finding of `check`, a failed
  test, a failed escape case); 2 usage or input error (an unreadable path, an invalid walk
  file, a bad option); 3 internal error.
- **`--json`** (every command) writes one JSON object per line instead of text, UTF-8, no
  white space outside strings, keys in the order the table below lists them. A key that
  does not apply to an object is **omitted**, never `null`. Strings carry the same text as
  the text form, unescaped, and JSON's own escaping (`\u00XX` for every control character)
  keeps the line safe for a terminal.

  | `type` | Command | Keys after `type` |
  |---|---|---|
  | `build` | every command, first line | `tool` (Tessera version), `unicode` (version of the category tables), `registry` (registry version), `slices` (array, the slices the build serves) |
  | `finding` | `check` | `folder`, `path`, `line`, `column`, `severity` (`error` or `warning`), `code`, `message` |
  | `folder` | `check` | `folder`, `errors`, `warnings` (counts) |
  | `expect` | `test` | `index` (0-based position in the walk's `[[expect]]` list), `frame` or `at`, `result` (`pass` or `fail`), `observed` (on failure) |
  | `abort` | `test` | `frame`, `script` (`"<kind>/<id>"`), `message`, `expected` (boolean) |
  | `case` | `escape` | `id` (the TM X-number, `"X53"`), `part` (the harness code of this part: `ST`, `CI`, `U`, `FZ` or `RM`), `slice`, `runner` (`tool`, `client` or `crate`), `result` (`PASS`, `FAIL`, `SKIP`, `CLIENT` or `ELSEWHERE`, §11.8), `observed` (on failure) |
  | `module` | `modules` | `name`, `major`, `minor`, `capability` (omitted when none), `group`, `deprecated` (boolean) |
  | `capability` | `modules` | `name`, `group`, `backends` (array), `local_only` (boolean), `slice` |
  | `summary` | every command, last line | `status` (the exit status), `errors`, `warnings`, `passed`, `failed`, `skipped`, `client`, `elsewhere` (those that apply; for `escape` they count parts) |

<a id="s11-2"></a>
### 11.2 Commands

| Command | Slice | Synopsis |
|---|---|---|
| `check` | 1a | `tessera-scripttool check [--strict] [--policy default\|FILE] [--json] FOLDER...` |
| `test` | 1a | `tessera-scripttool test [--walk FILE] [--seed N] [--latency K] [--backend offline\|other\|tessera-protocol] [--deny CAP]... [--kind installed\|developer] [--grants FILE] [--trace FILE] [--expect-trace FILE] [--worker-jitter SEED] [--keymap-ext FILE] [--root DIR] [--keep-root] [--json] FOLDER...` |
| `escape` | 1a (cases added per slice) | **(Design addition.)** `tessera-scripttool escape [--list] [--case XN]... [--seed N] [--json]`: the runner of TM's escape tests (§11.8). D17 places the escape tests "in `scripttool test`"; this command runs them with `test`'s machinery and harness, so it adds a name, not a mechanism |
| `modules` | 1a | **(Design addition.)** `tessera-scripttool modules [--json]`: the build's registry: modules with their majors, minors, capability, group and deprecation, and the capability catalogue of §6.1 for the slices the build serves |
| `init` | 1b | `tessera-scripttool init ID [--dir PARENT]`: writes `PARENT/ID/script.toml` and `PARENT/ID/main.lua` (an original, CC0 skeleton) that pass `check`; refuses an existing folder |
| `pack` | 1b | `tessera-scripttool pack FOLDER [-o FILE.zip]`: a zip of exactly the snapshot's files after `check` passes, with the content guard of D17 §10 (refuses `.dat`/`.datx` files and files starting with the archive container's header as `tessera-dat` recognises it; warns that extracted game assets must not be packed, LEGAL §11) |
| `dump-api` | 1b | `tessera-scripttool dump-api --format luals\|reference [-o FILE]`: the annotation file and the reference tables generated from the registry (D17 §10; their content is EA1b §10). D17 §10 spells the annotation generator `--ext-dump-api`; the tool accepts exactly that spelling as well, `tessera-scripttool --ext-dump-api [-o FILE]`, meaning `dump-api --format luals`. **(Design addition**: the subcommand and the `reference` format; D17 names no program for the flag) |

<a id="s11-3"></a>
### 11.3 `check`

`check` applies §4.14's validation to each folder as §4.14 says, plus warnings a load does
not report. Each FOLDER is taken as given (its kind does not matter for the checks; `id`
must equal the folder's own name). It prints the build's Unicode version and registry
version once.

**Finding format (text):** `<relative path>[:<line>[:<column>]]: <error|warning>[<code>]: <message>`.
Lines and columns refer to the manifest or a source file where known.

**Codes:**

| Code | Kind | Meaning |
|---|---|---|
| F01 | error | The folder is missing, not a directory, or a link or reparse point (§3.4 step 2) |
| F02 | error | An entry of a refused kind: link, reparse point, special file, hard-linked file |
| F03 | error | A path segment outside the grammar, `.`/`..`, a trailing `.`, or a reserved device name |
| F04 | error | Two paths equal under ASCII case folding; a `.lua` suffix in another case |
| F05 | error | A cap of §3.3 exceeded (the message names the cap and the count read) |
| F06 | error | A file changed size while it was read |
| M01 | error | `script.toml` missing, over 64 KiB, not UTF-8, has a byte-order mark, or not TOML (with the parser's line and column) |
| M02 | error | An unknown key, or a nested table from an unquoted dotted capability name |
| M03 | error | A required key missing, a value of the wrong type, or nesting deeper than §4.1 allows |
| M04 | error | `manifest` newer than this build reads (§12.1), or `api` major not served |
| M05 | error | `id` grammar, reserved id, or `id` different from the folder name |
| M06 | error | `version` not Semantic Versioning 2.0.0 within the caps |
| M07 | error | A display string breaks §4.4 (the message names the field, the rule and the code-point position, never the string) |
| M08 | error | `entry` invalid, not a `.lua` file, or not in the snapshot |
| M09 | error | An unknown capability; a capability both required and optional; more than 16 |
| M10 | error | `session.server` and a local-only capability both required |
| M11 | error | A module unknown, its major not served, its minor too new; a module listed twice; more than 32 |
| M12 | error | A module's capability missing from the manifest, or required module with an optional capability |
| M13 | error | `memory_mib` out of range |
| M14 | error | `[[contexts]]`: more than 2, name grammar, name too long for the id, duplicate |
| M15 | error | `[[actions]]`: more than 64, name grammar or length, duplicate, `kind`, missing or unknown `context`, no context declared |
| M16 | error | `default` invalid: not one stroke, an axis or analog entry, `on` not allowed there, `value` out of range |
| M17 | error | Two actions suggest the same stroke |
| M18 | error | `[[settings]]` invalid (any rule of §4.12) |
| M19 | error | `[server]` invalid; `[server] entry` before slice 3 |
| S01 | error | A source file does not compile (chunk name and line) |
| S02 | error | A source file is a binary chunk |
| W01 | warning | A deprecated module major is declared (§12.3) |
| W02 | warning | A capability that no declared module needs |
| W03 | warning | A suggested default that collides with a built-in default binding and would be withheld (KM §8, IB D2; plugin contexts are not checked) |
| W04 | warning | No `license` |
| W05 | warning | A group-A capability is required (it will be asked again after every update) |
| W06 | warning | A combination of §6.2 among the requested capabilities |
| W07 | warning | A required capability is unavailable on a class listed in `[server] backends` |
| W08 | warning | `--policy`: a required capability that the policy removes (the script would be refused there), or an optional one it withholds |
| W09 | warning | A source file calls `Ext.Use` or `Ext.Has` with a literal module name and major the manifest does not declare (it would answer `"not declared"`, §4.8) |

- `--strict` turns every warning into an error for the exit status.
- `--policy default|FILE` (useful from slice 3, accepted from 1a) evaluates layer 4 of §7.1
  against the default policy of D17 §6.5 or against a local policy file (TOML: `allow =
  [names]`, `forbid = [[names], …]`), and reports W08. The file shape is provisional until
  `tessera-protocol.md` defines the handshake (OPEN O-7).

<a id="s11-4"></a>
### 11.4 `test`

`test` runs the given scripts together, headless, on a **synthetic world** with no install
and no GPU, in D17 §8's **deterministic mode**, along a walk file, and checks the walk
file's expectations and the scripts' own assertions.

**The run.**

- The harness of D17 §10: the sandbox backend over a synthetic `MemoryWorldSource`,
  `physics`, `entity`, the input mapper without a window, `tessera-ui` without a painter,
  and the ext host. Steps are frames of exactly 1/60 s, numbered from 0 (§11.5).
- **Deterministic mode:** the wall clock is off and every slice is counted in units (the
  6a slice 200,000, TM L6; the per-frame cap 400,000, TM L5; a call 800,000, TM L4; 6c
  one apartment per frame, TM L7); the heavy-frame gate is off unless the walk simulates
  frame times (§11.5 `[[frame]]`), and then its floor frames follow TM rule 7.4.1's frame
  condition only; the switch timeout is 120 frames (TM L10); every seed (generators, hash
  secrets, table and interning salts) derives from the run's seed; worker results complete
  at frame N + k in submission order (k = `--latency`, default 1); streaming is gated on
  residency (D17 §8).
- **Grants and dialogs (the one exception to "only the user grants", §9.1).** Every load
  of the run, the first and every later one (a `reload`, a full reload, a `manager`
  `load`, an autoload after a `switch`), runs the decision pipeline of §9.2 against the
  tool root's grant table, exactly as in the client. No dialog is drawn; each dialog that
  §9.2 or §9.7 raises (at a load, or by a `manager` `widen` or local-only switch) is
  **answered by the harness** in the frame it is raised, by this policy:
  1. If the walk has queued a `reply` for that script (§11.5) that has not been used,
     the oldest such reply is taken. Its `kind` must equal the dialog's kind; otherwise
     the run fails with "dialog kind <kind> raised, reply expects <kind>" and the dialog
     is answered as Escape.
  2. Otherwise, a **first enable** dialog gets the **default answer**: Allow, with every
     optional capability named by `--deny CAP` (repeatable) unchecked, every other
     optional box checked, and "Load automatically" as the manifest's `autoload`; if a
     required capability is named by `--deny`, the answer is Deny (the load is refused,
     which the walk can expect).
  3. Otherwise (any other kind, with no queued reply), the dialog is answered as
     **Escape** (Deny or "Not now" by kind, §9.3), and the run **fails** unless an
     `[[expect]] dialog` (§11.5) names that dialog. So no later dialog is ever allowed
     silently.

  Each answer is applied as §9.3's outcome table says (Allow writes the record through
  the grant-store queue, with its completion seen `--latency` frames later) and appears in
  the trace as a `dialog` event (§11.6). The records exist only in the tool root's grant
  store, and the tool never reads or writes the user's.
- **Seeding the grant store.** `--grants FILE` copies FILE's bytes, unchanged, to the tool
  root's `ext-grants.toml` before start-up's read (at most 4 MiB + 1 byte are copied, which
  is enough for §8.6's "over 4 MiB" case). Any bytes are accepted: a hand-edited, damaged,
  duplicated or newer-`version` document is read exactly as the client reads it (§8.4,
  §8.6). The walk's `grants` event (§11.5) replaces the document mid-run and triggers the
  re-read of §8.5, as the script manager's opening would. Without `--grants`, the tool
  root starts with no grant store.
- **Kind:** `--kind installed` (default) loads the folders as kind `scripts`;
  `--kind developer` as `scripts-dev` with developer mode on. The folders are read where
  they are, through §3.4's rules: the given folder's parent directory takes the place of
  `data/scripts` in step 1, and the folder is opened relative to it in step 2. Records,
  stores and logs go to the tool root.
- **Backend:** `--backend offline` (default) admits the scripts; `other` and
  `tessera-protocol` run the session with that class (a test-only constructor in
  `tessera-net`), so the gate stays `Off` and no apartment may exist, which the walk can
  expect (§10.2).
- **Failure:** the run fails when any expectation fails, when any ext call **aborts**
  (an error that reaches the host, the units budget, a refused native) unless an
  expectation names that error, when a script is suspended unless expected, when a dialog
  is answered as Escape for want of a `reply` (rule 3 above) unless expected, or when a
  `reply` does not match the dialog it meets (rule 1). A `reply` still unused at the end
  of the run is reported as a warning. So a script's own `assert` and `error` are its
  assertions.
- **Determinism check.** `--worker-jitter SEED` (a test hook) delays each real worker job
  by a random amount drawn from SEED; with `--expect-trace` it shows that traces do not
  depend on worker timing (M5.5 exit criterion 7). This check was EM's earlier escape case
  X-DET-1; it is an ordinary `test` check, not an escape case (TM §11.10).
- `--keymap-ext FILE` uses a given ext keymap document for user bindings (read with KM
  §6.3's strict reader); otherwise only suggested defaults are bound.

<a id="s11-5"></a>
### 11.5 The walk file

TOML, at most 1 MiB, strict (unknown keys are errors, exit status 2), with a fuzz target
(§11.7). Every count, size and range of the walk file below is TM L86. Without `--walk`, the run uses an empty walk of 600 frames on the `flat` world
and the default screen.

**Frame numbers.** Every frame number in the walk file and the trace is **0-based**: frame
0 is the first frame of the run, and a run of `frames = 900` has frames 0 to 899. Every
frame number must be below `frames`. An event "at frame *n*" is delivered in that frame
the way the client would deliver it (an input in step 1 of frame *n*, and so a script's
handler in 6b of frame *n*). Ranges: `[[frame]] from` and `to` are both inclusive; a
`hold` lasts from `frame` up to but not including `until`; an entity exists from `spawn`
up to but not including `despawn`.

```toml
format = "tessera-ext-walk"
version = 1
seed = 7            # overridden by --seed
latency = 1         # overridden by --latency
frames = 900

[screen]
width = 1920
height = 1080

[world]
preset = "flat"     # "flat" or "slope"
size = 3            # landblocks per side, odd, 1-9
start = { x = 96.0, y = 96.0, heading = 0.0 }

[[entity]]
name = "Marker A"
position = [100.0, 104.0, 0.0]
spawn = 0
despawn = 600

[[input]]
frame = 60
press = "Ctrl+N"

[[input]]
frame = 62
release = "Ctrl+N"

[[input]]
frame = 70
text = "buy milk"

[[input]]
frame = 80
command = "/ext:notes add bread"

[[input]]
frame = 120
pointer = [400.0, 300.0]
click = "MouseLeft"

[[input]]
frame = 200
hold = ["move_forward"]
until = 260

[[frame]]
from = 300
to = 330
elapsed_ms = 25.0

[[expect]]
frame = 60
log = { script = "notes", line = "pad opened" }

[[expect]]
at = "end"
store = { script = "notes", key = "items", value = ["buy milk", "bread"] }

[[expect]]
at = "end"
loaded = { script = "notes", value = true }
```

| Table | Keys | Meaning |
|---|---|---|
| root | `format`, `version`, `seed`, `latency`, `frames` | `frames` 1–216,000 (one hour at 60 frames per second; TM L86) |
| `[screen]` | `width`, `height` | The harness's virtual surface in physical pixels, the "screen" that TM §2 and §7.1 count coverage in. Default 1920 × 1080; `width` 320–7,680, `height` 200–4,320 (TM L86) |
| `[world]` | `preset`, `size`, `start` | `flat`: terrain height 0 m everywhere; `slope`: height rises 0.1 m per metre along +x from the west edge. `size` landblocks per side (odd, 1–9, TM L86) around a fixed centre landblock that the harness prints. `start` is the player's position and heading in the region frame of ARCH §9.1 (metres, degrees) |
| `[[entity]]` | `name` (at most 48 cp, §4.4 rules), `position` (region frame), `spawn`, `despawn` (frames; `despawn` optional) | Synthetic entities that the `Entity` module sees; at most 4,096 (TM L86) |
| `[[input]]` | `frame` and exactly one of: `press` / `release` (one stroke, KM §5.1, resolved by the mapper exactly as in the client, so ext bindings resolve only after every built-in context, D17 §5.5); `text` (typed text, delivered to the focused text box if any, else dropped); `command` (a line as if typed in the chat input and submitted; `/ext:` lines go to the ext host, anything else is ignored); `pointer` + `click` (move the pointer to UI-space coordinates and click a mouse button there); `hold` + `until` (game-band actions held, for movement); `focus` (`"lost"` or `"gained"`: the game window loses or regains keyboard focus, TM §8.17); `switch` (`"offline"`, `"other"`, `"tessera-protocol"` or `"disconnect"`: a backend switch or session end is queued, §10.3); `reload` (`{ script = "<id>", from = "<path>" }`: from this frame the script's folder is the one at `from`, a path relative to the walk file's directory whose last segment is the id, read through §3.4's rules, and a reload is requested as `/ext:reload <id>` would, EA1b §8); `manager` (`{ script = "<id>", action = "revoke" \| "widen" \| "resume" \| "unload" \| "load", capability = "<name>" }`: the script manager's action of §9.7, `capability` only with `revoke` and `widen`; a `load` or `widen` that raises a dialog has it answered as §11.4 says); `reply` (`{ script = "<id>", kind = "first-enable" \| "first-enable-again" \| "changed" \| "update" \| "more" \| "memory", choice = "allow" \| "deny" \| "not-now" \| "escape", uncheck = ["<capability>", …], autoload = <boolean> }`: queues, from this frame, the answer to the next dialog of that script, §11.4; `uncheck` names optional capabilities left unchecked (default none), `autoload` the box (default: as §9.3 presets it); `deny` is valid for the first three kinds, `not-now` for the last three, `allow` and `escape` for all); `grants` (`"<path>"`: a file relative to the walk file's directory whose bytes (at most 4 MiB + 1, as `--grants` copies) replace the tool root's `ext-grants.toml` in this frame, after which the re-read of §8.5 is queued, as when the script manager opens) | At most 65,536 entries (TM L86) |
| `[[frame]]` | `from`, `to`, `elapsed_ms` | **Gate simulation:** in those frames, the harness reports `elapsed_ms` as the time since the frame's work started when step 7 is reached, so the heavy-frame gate (22 ms, TM L8) is evaluated on these values instead of being off, and the floor frames of TM rule 7.4.1 follow the 15-frame condition. Deterministic, because it never reads a clock. `elapsed_ms` above 22 for every frame is TM rule 7.4.6's "every frame is heavy" hook |
| `[[expect]]` | `frame` or `at = "end"`, and exactly one of: `log` (`script`, `line`: a script-log record whose text equals `line`, logged in that frame); `log_contains` (`script`, `text`); `error` (`script`, `contains`: an aborted call whose message contains the text, in that frame); `store` (`script`, `key`, `value`; only with `at = "end"`, read after the final flush of the unload at the end of the run: the `Store` value equals the TOML value, compared as the bounded value model, a TOML array being a table with keys 1..n); `loaded` (`script`, `value` boolean); `refused` (`script`, `reason`: the load was refused with that reason text); `suspended` (`script`); `answer` (`script`, `query`, `value`: a query's answer recorded in the trace); `gate` (`state`: the gate state at the end of that frame); `dialog` (`script`, `kind`, and optionally `asked`, an array of capability names, and `choice`: a dialog of that kind was raised for that script in that frame, with exactly that asked set and answered with that choice; with `at = "end"`, at any frame of the run); `granted` (`script`, `kind` optional, default as `--kind`, `value`, an array of capability names: the grant table's `granted` list of that (kind, id) at the end of that frame, `[]` when it has no valid record); `grants_file` (`unchanged = true`, only with `at = "end"`: the tool root's `ext-grants.toml` holds exactly the bytes it was seeded with by `--grants` or the last `grants` event) | At most 4,096 entries (TM L86) |

OPEN O-6 covers further presets (water, cells).

<a id="s11-6"></a>
### 11.6 The trace

`--trace FILE` writes the run's **trace**: one JSON object per line, in the order the
events happened. It contains no wall-clock values, no addresses and no absolute paths, so
two runs with the same scripts, walk file and seed produce byte-identical traces (D17 §8,
M5.5 exit criterion 7). `--expect-trace FILE` compares the trace byte for byte with a
committed one and fails on the first difference, naming its line.

**Keys**, always in this order, each present only when the event's row below lists it
(never `null`):

| Key | Meaning |
|---|---|
| `f` | Frame number (0-based, §11.5) |
| `ev` | The event kind (rows below) |
| `s` | The script's (kind, id) as `"scripts/notes"` |
| `what` | What happened, per event kind |
| `u` | Work units charged |
| `r` | The result |
| `a` | For values: the first 16 hexadecimal digits of SHA-256 over the value's serialised form in the `Store` file encoding |
| `c` | A list of capability names, sorted in byte order, as a JSON array of strings |

**Events:**

| `ev` | Keys | `what` | `r` |
|---|---|---|---|
| `gate` | `f`, `ev`, `what`, `r` | the new state: `Off`, `Starting`, `Open`, `Draining` | the cause: `connect`, `ui-ready`, `switch`, `disconnect`, `drained` |
| `load` | `f`, `ev`, `s`, `what`, `r` | the phase boundary: `start`, `read` (phase A consumed), `decided` (phase B ended), `opened` (phase C completed), `loaded` (top-level run finished), `unloaded` | `ok`; `refused:<reason>`; `cancelled`; `failed:<reason>` (a compilation or top-level failure) |
| `item` | `f`, `ev`, `s`, `what`, `u`, `r` | the 6a item: `snapshot`, `compile:<relative path>`, `run`, `unload`, `close`, `close:timeout` (TM rule 7.4.2) | `ok` or an error class |
| `call` | `f`, `ev`, `s`, `what`, `u`, `r`, `a` | the 6b call: `tick`, `timer`, or the notification's name (for example `Input.Action`, `Commands.Command`, `Store.Get`) | `ok` or an error class; `a` only for notifications that carry a value |
| `gc` | `f`, `ev`, `s` | — | — |
| `notice` | `f`, `ev`, `s`, `what` | the notice's text, escaped as §11.1 says | — |
| `intent` | `f`, `ev`, `s`, `what`, `r` | the intent's kind (for example `Input.Activate`) | `made`, `applied`, `discarded` |
| `worker` | `f`, `ev`, `s`, `what`, `r` | the job: `snapshot`, `store-open`, `store-probe`, `store-flush`, `store-read`, `store-wipe`, `grant-write`, `grant-read`, `keymap-write`, `image-decode`; `s` omitted for `grant-read` | `submitted`, then `ok` or an error class at completion |
| `query` | `f`, `ev`, `s`, `what`, `u`, `a` | the query's name (for example `Entity.QueryRadius`) | — |
| `limit` | `f`, `ev`, `s`, `what`, `r` | the TM limit reached, as its L-number (`L27`); `s` omitted for a host-wide limit | the answer given: `busy`, `limit`, `full`, `dropped`, `refused` |
| `dialog` | `f`, `ev`, `s`, `what`, `r`, `c` | the dialog's kind: `first-enable`, `first-enable-again`, `changed`, `update`, `more`, `memory` (§9.2 step 5, §9.7) | the answer and its source: `allow:reply`, `allow:default`, `deny:reply`, `deny:default`, `not-now:reply`, `escape:reply`, `escape:unanswered`, `closed:gate` (§10.4); `c` is the asked set (§9.2 step 5) |
| `grants` | `f`, `ev`, `s`, `what`, `c` | the grant table's change for that (kind, id): `allow` (a dialog's Allow), `update` (§9.2 step 6), `revoke`, `reread`, `remove`; one event per changed record, in byte order of `s`; `c` is the record's `granted` list after the change (`[]` when it has none) | — |

The JSON objects have no white space outside strings, and numbers are written as
integers.

<a id="s11-7"></a>
### 11.7 Fuzz targets

| Target | Input |
|---|---|
| `ext_manifest` | `script.toml` bytes: parse and validate (§4), never panicking, bounded allocation |
| `ext_grant_store` | `ext-grants.toml` bytes: read (§8.4, §8.6) and write back; a record read as valid writes back to an equal record |
| `ext_walk_file` | Walk-file bytes (§11.5) |
| `ext_folder_rules` | A synthetic list of (path, kind, link count, size) entries for the folder rules applied to entries `tessera-ext` is handed (§3.3) |

These are the manifest and grant-store parts of TM X59. The Lua-side argument validation
of module natives and the neutral data reader have their own targets (D17 deliverables,
TM X59).

<a id="s11-8"></a>
### 11.8 `escape`: the runner of TM's escape tests

TM §11 is the only escape-test catalogue (TM §0.1). This section specifies only the
runner; it keeps no cases of its own.

- **Which tests.** `escape` lists every TM X-test whose Harness column includes **ST** or
  **CI**, for the slices the build serves, under its X-number (`escape --case X53`).
  Tests whose parts are only U, RM or FZ run in their crates, on the reference machine or
  as fuzz targets, and are not listed.
- **Parts (TM §11).** A listed test is reported **per part**, one line for each harness
  code of its Harness column, as `<X-number> <code>: <result>` (`X62 ST: PASS`):
  - an **ST** part runs here: `PASS` when none of the observations in TM's "Fails if"
    column occurs in the procedure TM's "Procedure" column gives for it, `FAIL` (with the
    observed outcome) otherwise, `SKIP` when the build does not serve the part's slice;
  - a **CI** part (it needs the client's UI routing with plugin windows, the chat window,
    the drawn consent dialog or real frame time) is reported `CLIENT`: the client's
    integration tests run it under the same X-number;
  - a **U**, **FZ** or **RM** part is reported `ELSEWHERE` (its crate's tests, the fuzz
    targets or the reference machine run it).

  So a test with ST and CI parts (for example X47, X62, X85, X87, X90) is never reported
  as passed as a whole by `escape`: TM §11 counts it as passed only when every part has
  passed under its own runner.
- **How a case runs.** Each case generates its own synthetic script folders (original code
  written for the tool, CC0, generated at run time, never stored as a binary fixture),
  runs them in a fresh tool root with the `test` machinery in deterministic mode, and uses
  the walk-file events of §11.5 (gate simulation, `switch`, `focus`, `reload`, `manager`,
  `reply`, `grants`), `--grants` and the `dialog`, `granted` and `grants_file`
  expectations for the situations TM's procedures name (X62's seeded documents, X90's
  and X92's dialogs, X91's re-read). Test hooks that no walk event covers (a file that
  grows during the walk, an entry swapped for a link between a listing and its open, a
  store flush held in flight, a kill during a flush or a grant write, the links and
  canaries of X94) exist only in the tool's harness, never in the client.
- **`no_file_outside`.** After every case, the tool root holds no file outside the allowed
  set, listed with a no-follow walk of the tool root (a link is reported, never
  followed):
  - the script folders the case generated;
  - in each `data/script-store/<kind>/s_<id>/`: key files `k_<key>.store`, the host files
    `owner` and `local-only`, and, only in a case whose procedure kills the host during a
    flush, temporary files `t_<key>.tmp` (§3.1, §3.6);
  - `logs/scripts/<kind>/s_<id>.log` and `s_<id>.1.log`;
  - in the tool root's config directory: `ext-grants.toml`; its lock file
    `.ext-grants.toml.lock` (persistent, §8.5); its backups
    `ext-grants.toml.corrupt-<unix seconds>[-n].bak` (§8.6); `keymap-ext.toml` and its lock
    file `.keymap-ext.toml.lock` (§8.7); and, only in a case whose procedure kills the
    host during a save, the temporary files `.ext-grants.toml.tmp-*` and
    `.keymap-ext.toml.tmp-*`;
  - the files a case's procedure places itself: X94's canary files and the link targets
    it creates, under the tool root's directory `canary/`, and the documents `--grants`
    and `grants` copy from.

  A file outside this set, a leftover temporary file in a case that killed nothing, or a
  canary that was opened (seen by the harness's open hook on the host's file layer),
  written, truncated or removed (X94) fails the case.
- **Output.** One line per part as above; the exit status is 1 if any part failed.
- **Earlier IDs.** The first version of this spec named its cases X-FS-1 … X-DET-1. They
  are retired; TM §11.10 maps each to its X-tests. `escape` does not accept them.

---

<a id="s12"></a>
## 12. Versioning rules

<a id="s12-1"></a>
### 12.1 The manifest format (`manifest`)

- An integer, starting at 1. A build reads every manifest version up to the one it knows.
  A higher one is refused with "needs a newer Tessera" (M04).
- Adding a field (always optional, with a default that keeps the old meaning) bumps the
  version. A manifest that uses the field must declare at least that version; a manifest
  of a lower version that contains it is refused as "unknown key" (M02), so an old build
  never half-understands a new manifest.
- No field is ever removed, renamed or given a new meaning under the same name. A change of
  that kind would be a new field name.

<a id="s12-2"></a>
### 12.2 The `Ext` global (`api`)

`api` names the major of the `Ext` global's own entries (`Use`, `Has`, `Load`, `Version`;
EA1a §3). It follows the module rules below. A build serves a set of `api` majors; a
manifest whose `api` major is not served is refused (M04).

<a id="s12-3"></a>
### 12.3 Modules (D17 §2)

- Each module has its own **major** and **minor**. Within a major only **additions** are
  allowed (a function, an optional argument, a field of a result, a new reason string for
  a new failure); each addition raises the minor.
- A rename, a removal or a behaviour change raises the major.
- When a new major ships, the previous major stays served for **at least one Tessera minor
  release**, marked deprecated: `check` warns (W01), a load logs one warning, and every call
  of a deprecated name logs a warning (rate-limited as the log is, TM L45).
- `Ext.Has(name, major)` tests support without granting anything; `Ext.Version` is
  Tessera's own version (Semantic Versioning).
- The registry (D17 §1.3) is the single source of the served majors and minors; `modules`
  (§11.2) prints it, and the generated reference and annotation file come from it.

<a id="s12-4"></a>
### 12.4 Script versions (`version`)

- Informational: shown in the dialog, the manager and `Core.Script()` (EA1a §4.4). Grants
  never depend on it; the content hash does (§9.5). A downgrade is a content change like
  any other.
- Two different contents may carry the same `version`; Tessera does not check that an
  author raised it.

<a id="s12-5"></a>
### 12.5 The grant store (`version`)

- An integer, starting at 1. A build reads its own version and older ones (migrating them
  in memory and writing its own on the next write, after a backup as in §8.6). A newer
  version is never written (§8.6).
- Unknown keys are kept and written back (§8.4), so additive changes need no version bump;
  a change that an older build would misread (a new meaning for an existing key) needs one.

<a id="s12-6"></a>
### 12.6 The content hash (`tessera-ext-content-v1`)

- The tag and `content_hash_version` change together, only if the encoding ever has to
  change. A build that introduces version 2 keeps computing version 1 for records that
  carry it for at least one Tessera minor release; a record whose version the build can no
  longer compute is treated as a content change (§9.2 step 2).

<a id="s12-7"></a>
### 12.7 Capabilities: never widened under the same name

- A capability name, once shipped, is never renamed or removed while any served module
  major needs it.
- **A grant never comes to allow more than it allowed when the user gave it.** A module
  change that would let an existing capability do more (a new function that changes game
  state, reaches the network, reads files, or moves a capability from group O to group A)
  needs a **new capability name**, which existing records do not hold, so the user is
  asked. Additions that stay within the capability's dialog text (§6.1) and group are
  allowed under the same name.
- A capability's group never moves from A to O. A move from O to A is a new name.
- A capability's backend availability may narrow at any time (the safe direction), and may
  widen only by a decision record that amends D17 §6.5.

<a id="s12-8"></a>
### 12.8 Names that are stable

The ids, the `ext.<id>.<name>` action and context names, the `/ext:` command words, the
`Store` file names (`s_<id>`, `k_<key>.store`, the temporary `t_<key>.tmp`,
`local-only`, `owner`), the log names (`s_<id>.log`, `s_<id>.1.log`), the folder names
(`scripts`, `scripts-dev`, `script-store`), the document names (`ext-grants.toml`,
`keymap-ext.toml`) and their lock files (`.ext-grants.toml.lock`, `.keymap-ext.toml.lock`) and the format tags (`tessera-ext-grants`, `tessera-ext-walk`) are part
of the contract and never change without a decision record.

---

<a id="s13"></a>
## 13. Invariants

Each is checked by a test of `tessera-ext`, `tessera-ext-host`, the client or a TM escape
test run as §11.8 says. TM §12's invariants hold as well; where one restates a TM
invariant, it names it.

1. A folder that breaks any rule of §3.3 is refused whole, and nothing of it is compiled or
   run.
2. No file is read through a symbolic link, a reparse point, a hard link, or a special file,
   including the script folder itself (§3.4), and no host file under a script-influenced
   name (the store directory, every entry in it, the log files) is opened, created or
   removed through a link; such an entry is used only when it is a plain file, and a wipe
   or a removal removes only entries of the store directory itself, never anything a
   link points to (§3.6, §8.8; TM invariant 15, X94).
3. The bytes hashed (§5) are exactly the bytes compiled and served (§3.4).
4. Two snapshots that differ in any path or content byte, or only by moving bytes between a
   path and a content, hash differently (§5; the test vectors of §5.2 reproduce).
5. A manifest with an unknown key, an unknown capability, an unknown module or an
   unserved major is refused (§4).
6. `id` equals the folder name in every loaded script.
7. No loaded script holds a required capability that is not in its effective set, and none
   loads without every required capability (§7.1).
8. No record and no apartment ever holds `session.server` together with `data.read` or
   `bridge.plugins` (§7.2, §8.3; TM invariant 12).
9. Tessera never grants a capability, in a record or in the grant table, except by an
   Allow that the user pressed with the pointer under §9.1's rule (TM §8.25), or by the
   `test` tool root's default answer (§11.4), whose grants exist only in that tool root.
   Deny, "Not now" and Escape never write a record. A valid record the user wrote by hand
   is the user's own act: it never widens a running apartment and is honoured only from
   the next load, through §9.2 (§8.4, §8.5).
10. No script call can cause a dialog, open the script manager or write the grant store.
11. After a content change, an installed script never runs new code under a group-A grant
    given to older code (§9.2 step 2, §9.8).
12. No `Store` value and no hand-off table reaches a script whose `author` differs from the
    one the store was created under: an `author` change wipes the store at the open and
    makes a reload full, and a store whose owner tag does not name the loading `author` is
    wiped before it is opened (§8.8, §9.8; TM invariant 24).
13. A damaged or duplicated record, and every record that breaks §8.3 however it was
    written (for example one granting `session.server` with a local-only capability, or
    a capability outside its pinned list), grants nothing (§8.4; TM §8.23).
14. A grant document of a newer version is never written by an older build (§8.6).
15. Phase C never opens a store before the decision it opens it under has been durably
    written to the grant store, or, with a newer document, applied to the grant table
    (§8.5).
16. At most one script per id is loaded across both kinds (§7.4).
17. Ext apartments exist only in `Open` and `Draining` (§10.3), and only for admitted
    classes (§10.2; TM invariant 22).
18. In `Draining` no 6b call runs and no load starts; `Connect` is submitted only after the
    last close item; an apartment whose `Unload` handler has not started when the switch
    timeout passes (TM L10) is closed without it; `Draining` ends within 34 ungated frames
    without gating and within TM L11 under permanent gating (§10.3, §10.4; TM invariant
    20).
19. Every load in progress at a switch ends as §10.4 says; no apartment of the old session
    survives into the new one, and no store work, store probe or grant-record write is
    left half-done (TM §8.21, X87).
20. Two `test` runs with the same scripts, walk file and seed produce byte-identical traces,
    whatever the worker timing (§11.6).
21. `scripttool` never writes outside its tool root, never reads `TESSERA_GAME_DIR`, and
    never prints an unescaped control character from script-supplied text (§11.1).
22. One press can start at most one hold from one script's suggested defaults (§4.11).
23. A capability name never allows more than it did when a record granted it (§12.7).
24. The grant store is read and written only on the grant-store queue, never on the main
    thread (§8.5).
25. A revocation suspends the script in the frame of the revocation (§9.7; TM invariant
    23), whether it comes from the script manager or from a re-read of the grant store
    that removes, invalidates or narrows the script's record (§8.5; TM §8.23, X91).
26. A re-read of the grant store never widens what a running apartment may do: a widened
    record is used only from the script's next load (§8.5; TM §8.23).
27. No grant-store or `keymap-ext.toml` write leaves any file other than the document, its
    persistent lock file and its backups, except a temporary file after a crash (§8.5,
    §8.7, §11.8).

---

<a id="s14"></a>
## 14. Worked examples (synthetic)

<a id="s14-1"></a>
### 14.1 First enable of the note-taker

The user has copied the folder of §4.3 to `data/scripts/notes/` and clicks "Enable" in the
script manager while the offline sandbox is in `Open`. The load reserves 32 MiB (no
record). Phase A reads the four files (`script.toml`, `main.lua`, `ui/pad.lua`,
`icon.png`), hashes them and parses the manifest. Phase B finds the registry happy (`UI`,
`Input`, `Store`, `Commands`, `Core` 1.0 required; `World` optional, its capability
`world.read` optional), no other `notes` loaded, the offered set `ui`, `input`,
`world.read` (all available offline), and no record: step 1a, the first-enable dialog
(there is no store, so no probe). It shows "Notes 1.2.0 · notes · installed", the author,
the description, under "Observe and show" `ui` and `input` (required, no box) and
`world.read` (a checked box) with their reasons; no warnings (no `entity.read`); "Load
automatically" checked (the manifest's `autoload`). The pointer happens to rest where
"Allow" appears; that does not count. The user moves it off and back onto "Allow" 2 s
later and clicks. The record is written (§8.2's example) on the grant-store queue; when
the write has completed, phase C creates the store directory and writes the owner tag
"Example Author" durably (no marker: no local-only capability), and phase D compiles
three source files and runs `main.lua`. The suggested `Ctrl+N` collides with no built-in
binding, so it is applied when the host registers `ext.notes.toggle` in `ext.notes.pad`.

<a id="s14-2"></a>
### 14.2 An update, installed versus developer

The author ships 1.3.0, which changes `ui/pad.lua` only. The user replaces the installed
folder. At the next world entry, autoload reads it: same `author`, same capability lists
(step 1d), a different hash (step 2). All three capabilities are group O, so nothing
becomes undecided; steps 3 to 5 find nothing to ask: no dialog. Step 6 writes the record
with the new hash, and with that write the notice "notes · installed was updated. Still
allowed: …. Stored values kept (14 KiB)." shows; when the write has completed, the load
continues. At the following world entry the hash matches and no notice shows. Had 1.3.0
also asked for `camera` (group A) optionally, the capability list would differ: step 1c,
the changed-script dialog with every decision cleared.

The same edit in `data/scripts-dev/notes/` with developer mode on (and the installed copy
not loaded) keeps every decision (step 2, developer case): nothing is asked, the record's
hash is updated, and the load proceeds. With the installed copy loaded, the developer load
is refused by §7.4 and the manager offers to unload `notes · installed`.

<a id="s14-3"></a>
### 14.3 The local-only choice (slice 3)

A script declares `optional_capabilities` `data.read` and `session.server`. Offline, the
offered set has `data.read` and, over the loopback, `session.server`, both optional: the
dialog shows the local-only choice (§9.4). The user picks "Use local data". The record
grants `data.read`, denies `session.server`; phase C writes the owner tag and the durable
`local-only` marker. Weeks later the user switches to "Exchange messages with the server"
in the manager: the script is unloaded first, the dialog says "This script's stored values
will be erased", the user allows, the record is written, and the load that follows runs
phase C after the old apartment's final flush: the store is marked and the new set holds
`session.server`, so it is wiped (marker, owner tag and values), a new owner tag is
written, and the new apartment reads no key.

<a id="s14-4"></a>
### 14.4 A backend switch during a load

Three scripts are loaded in `Open`; a fourth is in phase B with its first-enable dialog
showing; a fifth has delivered its snapshot result and compiled one of its two source
files. The user starts a switch to another backend. The host enters `Draining`: leases
end and intents are discarded in that frame; the dialog closes as "Not now" (no record
written); the fifth script's second compilation item is dropped and its apartment gets one
close item; the three loaded scripts get their `Unload` and close items. **Seven** items
run (3 × (`Unload` + close) + 1 close), one or more per ungated frame within the 1 ms 6a
slice, 6b not at all. When the last close has run and no phase A or B work is left, the
state becomes `Off` and `Command::Connect` is submitted. If the new session is `Other`, the
state stays `Off` for its whole life; if it is `Offline`, it passes `Starting` and `Open`,
and autoload at its world entry loads the autoload scripts again, in the user's order.

Had every frame been heavy (a slow machine, TM §7.4), only floor frames would have run
items, one each; 2 s after the switch was queued, the loaded scripts whose `Unload` had not
started would have been closed without it, each with "unloaded without Unload: backend
switch timed out" in its log.

<a id="s14-5"></a>
### 14.5 Refused manifests

| Manifest excerpt | Finding |
|---|---|
| `id = "notes"` in folder `data/scripts/Notes/` | The folder name is not a valid id (upper case): the manager lists it as "Not a script folder" (§3.5); `check` reports M05 (the id differs from the folder name) |
| `[capabilities]` then `entity.read = "…"` (unquoted) | M02: capability names containing '.' must be quoted |
| `capabilities` has `session.server` and `data.read` | M10 |
| `[modules] Camera = 1` with `camera` only in `[optional_capabilities]` | M12: module Camera needs capability `camera` in [capabilities] |
| Two actions with `default = "F5"` and `default = { input = "F5", on = "tap" }` | M17 |
| `default = "G G"` | M16: a suggested default is one stroke |
| `name = "Tessera‮"` | M07: `name`, removed character (category Cf), at code point 8 |
| `name = "Tessé̂̃ra"` (three combining marks on one letter) | M07: `name`, more than 2 marks after a base, at code point 8 |
| A file `lib/aux.tar.gz` or `data/x.` | F03 (device name before the first dot; trailing dot) |
| `memory_mib = 128` | M13 |
| `manifest = 2` in a build that reads 1 | M04: needs a newer Tessera |

<a id="s14-6"></a>
### 14.6 A test run

`tessera-scripttool test --walk notes.walk.toml --seed 7 --trace notes.trace data/scripts/notes`
with the walk of §11.5. When the load of `notes` reaches phase B (a frame or two after
the start, as `--latency` delivers phase A), it raises its first-enable dialog (no
`--grants`, so no record), which gets the harness's default answer (§11.4: Allow,
`world.read` checked). The trace records a `dialog` event with `what` `first-enable`, `r`
`allow:default` and `c` `["input","ui","world.read"]`, then the `grant-write` job, and
phase C starts only after that job's completion. Frame 60 presses `Ctrl+N`; the mapper
resolves it, after every built-in context, to `ext.notes.toggle`; 6b of frame 60 delivers the action start; the
script opens its window and focuses its box in that turn (a gesture turn, TM §8.13; no
window being active) and logs `pad opened`, which meets the expectation for frame 60.
Frame 70 types `buy milk` into the focused box; frame 80's `/ext:notes add bread` is
delivered in 6b of frame 80. Each write is held as pending and flushed by the store's
queue within a second, its completion seen one frame after submission (`latency = 1`).
At the end of the run the unload's final flush has completed, and the `store`
expectation reads `["buy milk", "bread"]`. Run again with `--worker-jitter 3` and
`--expect-trace notes.trace`: identical.

<a id="s14-7"></a>
### 14.7 Withholding under a forbid set (slice 3)

A server policy allows `ui`, `overlay`, `entity.read`, `camera` and forbids
`["camera", "entity.read"]`. A script requires `entity.read` and `ui` and has `camera` and
`overlay` optional. Layer 4: the forbid set is complete in the offered set; not all members
are required; the optional member with the greatest name in byte order is `camera` (the
only optional member): withheld. The dialog lists `camera` under "Not available here: not
allowed by this server" and asks for the rest.

---

<a id="s15"></a>
## 15. Open items

Item numbers are never reused; closed items stay listed with what closed them.

> **OPEN O-1 (optional capabilities unknown to the build):** D17 §2 refuses a script with an
> unknown capability even in `optional_capabilities`, so a script that wants to use a later
> slice's capability when present cannot load on an earlier build at all. Proposal: treat an
> unknown name in `optional_capabilities` (only there) as "unavailable on this build", so
> `Ext.Has` is false and the dialog lists it under "Not available here". This needs a D17
> amendment; until then §4.7 follows D17.

**O-2 (closed, revision 2):** `Ext.Use` of an undeclared module answers `nil, "not
declared"` (EA1a §3; §4.8 here, with `check` warning W09).

**O-3 (closed, revision 2):** ext context priorities are assigned by the host as EA1a §8.1
says; the manifest has no priority key (§4.10).

**O-4 (closed, revision 2):** the retained snapshot is charged to the script's memory cap
and held under the load's reservation; there is no separate 136 MiB bound (TM §8.10;
§3.4, §7.3 here).

**O-5 (closed, revision 2):** `Draining` liveness is TM §7.4's floor frames and switch
timeout (TM L9–L11); §10.3, §10.4 and invariant 18 state them, and TM X85 tests them.

> **OPEN O-6 (synthetic world presets):** `test` offers `flat` and `slope` (§11.5). Water
> queries and cell (interior) queries need presets with water and cells; they depend on
> what `tessera-world::testkit` can build, and are added with the reference scripts that
> need them.

> **OPEN O-7 (slice 3 fields):** `[server] entry` (the server-side half, D17 §11) and the
> local policy file of `check --policy` are provisional until slice 3's additions to D17,
> its API spec and `tessera-protocol.md` define the server half's accounting, grants and
> `Store`, and the handshake's policy shape.

> **OPEN O-8 (dot-files in developer folders):** a developer folder kept under version
> control holds `.git/`, which usually breaks the 512-entry and 256-file caps, so the
> folder is refused. Proposal: skip, without descending, every entry whose name starts with
> `.`; such entries are not read, not hashed and not served (no code or data path can name
> them, since `Ext.Load` names never yield an empty segment), and count once each toward
> the entry cap. This changes what the snapshot covers and needs the owner's agreement as
> a D17 amendment; until then §3.3 applies as written and `pack` is the way to produce a
> clean folder.

**O-9 (closed, revision 2):** display strings stay refused at parse time, the stricter
rule, with TM §8.2's mark rule (at most 2 of category M per base) and its removed
categories (Cn and noncharacters included); exit criterion 2's display part is TM X47
parts (b) and (c) (§4.4).

> **OPEN O-10 (the grant store's own document):** D17 §5.2 places grants "in Tessera's
> settings"; §8.1 places them in a document of their own beside the settings file. The
> owner agrees to this as a D17 amendment at the M5.5 plan review, or rejects it. Until
> then slice 1a is built as §8 says, because every rule of §8 (strict, fail-closed reading;
> the grant-store queue; backups) is the same whichever file holds the records, and
> moving them is a change of path and of the root table name only. If the owner rejects
> it, the records move into the settings file as a table `[ext_grants]` with the same
> schema and the same strict per-record reading, and the settings file's tolerant reading
> must never reset that table.

**O-11 (closed, revision 3):** TM revision 3 registers every count of this spec: the
manifest's per-field counts (TM L87), the walk file's entities, inputs, expectations,
`frames`, `[screen]` and `size` ranges (TM L86), the folder listing (TM L88) and the owner
tag (TM L89); TM §0.1 rule 5 keeps only character sets and formats in the defining spec.
§3.5, §4, §8.8 and §11.5 cite them.

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review), required by D17 "Required follow-ups" item 9 and the M5.5 "Specs needed" list. Folder layout and folder rules with the snapshot-root no-follow rule and the case rules (§3); the `script.toml` grammar with every field, display-string rules, module versions, contexts, actions with suggested defaults (one per stroke), settings and server hints (§4); the content-hash encoding with test vectors (§5); the capability catalogue with dialog texts (§6); effective rights and the four-phase load pipeline (§7); the grant store `ext-grants.toml` with its schema, validation, writing and damage rules (§8); the grant UI flow: dialog kinds, controls, outcomes, re-ask table, notice and Review, the manager's grant pages, developer mode (§9); the backend gate states `Off`, `Starting`, `Open`, `Draining` and loads at a transition (§10); `tessera-scripttool` with `check` codes, `test`, the walk file, the trace, fuzz targets and the `escape` catalogue covering D17's eight open security items (§11); versioning rules (§12); 23 invariants, 7 worked examples, 9 OPEN items. |
| 2026-10-07 | **Revision 2** (Review), answering the independent review and aligning with TM revision 2, EA1a, EA1b and D17. Header and §0: real companion names (EA1a and EA1b are D17's `ext-api.md`), TM normative for limits, rules and escape cases, "(proposed D17 amendment)" mark; limits cited as TM L-numbers throughout. §3.1: `keymap-ext.toml` in the configured keymap path's directory (TM §8.19); the store's `owner` tag. §3.3: device-name extension = everything from the first dot; trailing dot refused; depth stated as at most 5 path segments. §3.4 and §7.3: snapshot held under the load's memory reservation and data files charged to the cap (TM §8.10; O-4 closed). §3.5, §4.2, §4.8, §4.10, §4.12, §4.13: design additions marked. §4.1: nesting counted below the root. §4.4: refusal aligned with TM §8.2 (at most 2 marks of category M, no mark without a base, Cn and noncharacters, byte caps; O-9 closed, X47 route stated). §4.6: `entry` up to 5 segments. §4.8: undeclared modules answer `"not declared"` as EA1a §3 (O-2 closed), new W09. §4.10: priorities by EA1a §8.1 (O-3 closed). §4.14: validation stages for `check` and the load; a phase-D compilation failure closes the apartment with one close item. §8: grants created only by Allow; grant table and grant-store queue on a worker with a locked, flushed read-modify-write that completes before phase C (ARCH §21); settings numbers accept TOML integers; Remove deletes the store before the record; new §8.8, the store open with the owner check (TM §8.24), wipes, durable owner tag and marker. §9.1, §9.3: pointer entry observed after the 500 ms delay, for Allow, boxes and "Load automatically" (TM §8.25). §9.2: the re-ask table replaced by an ordered six-step pipeline with each case's record write (hash updated before phase C; the notice once per change). §9.3: outcomes for manager-started dialogs; store probe for the "Stored values" line; Deny writes nothing. §9.7: every revocation suspends in that frame (TM §8.23). §9.8: full-reload declaration set adds `author`, `api`, `manifest`, `[modules]`, `[optional_modules]`, `[server]`, after full validation (TM §8.24). §10.3, §10.4: floor frames, switch timeout row and bounds (TM §7.4, L9–L11; O-5 closed), leases end when a switch is queued, store work completes (TM §8.21). §11.1: `--json` objects specified. §11.4: X-DET-1 becomes an ordinary determinism check. §11.5: 0-based frame numbers and ranges; `[screen]`, `focus`, `switch`, `reload`, `manager` events and a `gate` expectation (TM §13). §11.6: per-event trace keys and `what` values. §11.8: runner only; cases are TM's X-tests, earlier IDs retired (TM §11.10). §13: invariants 9, 12, 15, 18 restated; 24 and 25 added. §14.4: seven items, with the gated case. §15: O-2, O-3, O-4, O-5, O-9 closed; O-10 (grant store document, proposed D17 amendment) and O-11 (field caps for TM's register) added. |
| 2026-10-07 | **Revision 3** (Review), answering the re-review of revision 2 and aligning with TM revision 3. **Store and log files** (new §3.6; §3.1, §8.8, §12.8): directory handles for `data/script-store/`, `logs/scripts/`, `<kind>` and `s_<id>` with `O_NOFOLLOW` (reparse points refused); an unusable store directory refuses the load and is never touched; every entry opened, created (exclusively), renamed or removed relative to the `s_<id>` handle with its kind and link count checked on the handle; a per-entry action table (key files kept, crash leftovers and unexpected entries removed by a no-follow removal, a non-plain `local-only` counted as present, a non-plain `owner` as missing, a subdirectory or unremovable entry fails closed); the write-behind temporary file named `t_<key>.tmp`; log files opened no-follow and otherwise left alone, with the log unavailable and reported (TM §8.8, invariant 15, X94). §8.8 restructured: handles, entry sweep, owner check, local-only check, durable tags, file index; the wipe never recurses or leaves `s_<id>`; the **store probe** defined (read-only, no creates or removals), used by §9.2 step 2 (content notice and its size) and §9.3. **Grant store** (§8.1, §8.3, §8.4, §8.5, §9.1, §9.7, §7.1): a valid hand-edited record is honoured only within §8.3 and from the next load; a re-read that removes, invalidates or narrows a loaded script's record suspends it in that frame, a widening waits for the next load (TM §8.23); the lock is `.ext-grants.toml.lock` beside the resolved document (persistent, never the settings file's lock) and the temporary file is named; the `keymap-ext.toml` bindings' removal is a locked read-modify-write (`.keymap-ext.toml.lock`, design addition). **`test`** (§11.4–§11.6): every load runs §9.2; dialogs are answered by a queued `reply`, by the default answer for a first enable, or as Escape with a run failure unless expected; `--grants FILE` seeds the tool root's grant store and the `grants` walk event replaces it mid-run with a re-read; `manager` gains `widen`; `dialog`, `granted` and `grants_file` expectations; `dialog` and `grants` trace events and the `c` key (TM X62, X90, X91, X92). **`escape`** (§11.8, §11.1): per-part results (`CLIENT` for CI parts, `ELSEWHERE` for U, FZ and RM parts), never a whole mixed test as PASS; `no_file_outside` names the lock files, backups, temporary files (only after a kill) and X94's canaries. **Smaller items:** `/ext:reload` with developer mode off, for an id not loaded and for a suspended script (§9.8); the script-list order for records outside `order` and new records appended (§7.3); §10.3 cites TM L5–L9; §10.4 rows for a load before its dialog and for one waiting on its record write or store probe (TM §8.21); §4.12 and §9.2 step 6 agree (invalid and obsolete settings values dropped by the load's own write, a reverted value never written as its default); `dump-api` accepts D17 §10's `--ext-dump-api`, and `escape`, `modules` and the subcommand are marked design additions; TM L86–L89 cited (§3.5, §4, §8.8, §11.5) and O-11 closed. Invariants 2, 9, 13, 19 and 25 restated; 26 and 27 added. |
