<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Extended scripts: folders, manifest, content hash, grants, backend gate and the script tool

| Field | Value |
|---|---|
| Status | **Accepted** (2026-10-07) |
| Last updated | 2026-10-07 |
| Author role | Design (clean room). Everything here is **Tessera's own design**, written from [decision 0017](../decisions/0017-extended-scripting.md) and Tessera's own specs. It describes no other program, and no dirty-room material, disassembly or third-party source was read to write it |
| Reviewer | Independent clean-room review |
| Review record | Review of revision 6, 2026-10-07: **accepted**, with minor follow-ups for the next revision. Full record moved to the review log (decision 0018). |
| Implements into | `crates/ext` (`tessera-ext`: manifest parser and validation, folder rules applied to entries it is handed, content hash, grant-store format, the grant decision pipeline of §9.2, walk-file parser); `crates/ext-host` (`tessera-ext-host`: the folder walk, the grant-store queue and persistence, the store open with its owner tag and marker, the load pipeline, the script manager's grant pages, the headless harness); `apps/scripttool` (`tessera-scripttool`); `apps/client` (the backend gate states, consent prompts); `crates/net` (`Session::class()`, D17 §6.1); `fuzz/` (targets of §11.7) |
| Related documents | Decision [0017](../decisions/0017-extended-scripting.md) (abbreviated **D17**; §n means its section n); [`ext-threat-model.md`](ext-threat-model.md) (**TM**: normative for every limit, cited as TM L*n*, for the rules of its §7–§9, cited as TM §*n.m*, and for the escape tests, cited as TM X*n*; TM §0.1); [`ext-api-1a.md`](ext-api-1a.md) (**EA1a**) and [`ext-api-1b.md`](ext-api-1b.md) (**EA1b**), which together are D17's `ext-api.md` for slices 1a and 1b ("the API specs"); [0004](../decisions/0004-server-strategy.md); [0014](../decisions/0014-ui-input-bridge.md) (**IB**); [0015](../decisions/0015-ui-rendering.md) (**UR**); [`keymap.md`](keymap.md) (**KM**); [`lua-plugin-api.md`](lua-plugin-api.md) (**LPA**); [`plugin-runtime.md`](plugin-runtime.md) (**PR**); `docs/ARCHITECTURE.md` (**ARCH**) §17, §18, §21 |

> **Amended by [decision 0034](../decisions/0034-official-server-compatibility.md) §4
> (2026-10-11); not yet carried into this text.** The rule "extended scripts never run
> against an official server" is replaced: on an official-server session (a backend class
> of its own, named by the M8 and M9 specs) extended scripts run only after the user's
> explicit opt-in, which is off by default, is stored per user and can be
> set by nothing but the user in the client's own interface; per-script grants still apply.
> Automation (`input.drive` and every module that issues movement, input or game commands)
> stays offline only and is never enabled on an official server. `Other` stays fail-closed.
> D17 §6.3's build-level exclusion (`gated-compat` tag, `links` guard, CI check) is
> withdrawn. The M9 revision of this spec carries the change into its text; where the two
> differ before then, decision 0034 wins.

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
| Snapshot-root no-follow rule (TM §8.8), one suggested default per stroke (TM §8.16), retained snapshot charged to the script (TM §8.10), display strings refused at parse time with TM's mark rule (TM §8.2), the liveness floor and switch timeout (TM §7.4), partial loads at a switch (TM §8.21), `keymap-ext.toml` location (TM §8.19), revocation suspends, a grant-store re-read included (TM §8.23), no store or hand-off across an `author` change and the store owner tag (TM §8.24), consent pointer entry (TM §8.25), the store-directory and log-file no-follow rule (TM §8.8), loads waiting on a record write or store probe at a switch (TM §8.21), classify-before-open and non-blocking opens for the walk, the store, the log and Tessera's own files, bounded grant-store reads and backups, a lock taken without blocking, the tool root's no-follow removal (TM §8.8), overlapping suggested defaults (TM §8.16), the apartment's effective set, the session overlay, narrowings during a load and one id loaded or loading (TM §8.23), the store-root scan (TM §8.7, L92), every limit, the manifest's per-field counts, the walk file, the folder listing and the owner tag included (TM §10, L86–L89, L92), what the register holds (TM §0.1 rule 6), every escape case and per-part reporting (TM §11); from TM revision 5: only a re-read of an understood document narrows, an unusable or newer one changes nothing, a write's completion never narrows the keys it carried, a write replacing a damaged document writes every grant-table record (TM §8.23, invariant 35, X91), and the 1 s advisory-lock wait (TM L94); from TM revision 6: a failed load write taken back restores the overlay entry it superseded, turning developer mode off cancels every developer load in progress and refuses waiting ones, the user's unload of a loading copy cancels its load (TM §8.23), the store-root scan at every entry into `Starting` (TM L92), a full reload's load waiting until the old cap has left the sum (TM L84), the 500 ms dead time of every prompt control (TM L65) and the `[[flood]]` bounds (TM L86) | TM revisions 4, 5 and 6, from D17 "Open security and design items" and "Other improvements" | Decision (TM) |
| Angle form of the walk file's `[world] start` (`yaw` in radians) | EA1a §2.10 (KM §9.1) | Decision (EA1a) |
| `Ext.Use` and the manifest's module tables, including `"not declared"` | EA1a §3 | Decision (EA1a) |
| Name grammar, stroke syntax, keymap limits, saving rules | KM §2, §4, §5, §6 | Decision (KM) |
| Activation filter `on` | IB D3 | Decision (IB) |
| Settings file location, atomic save, lock file, install guard, tolerant loading with backups | `crates/settings` rustdoc (the settings file has no spec; its rustdoc is its definition) | Decision (code) |
| Settings-crate lock, temporary-file and backup names (`.<file name>.lock`, `.<file name>.tmp-…`, `<file name>.<reason>-<12-digit Unix seconds>[-<n>].bak`) | `crates/settings` rustdoc of `SettingsStore::update`, the atomic write and the backup constants | Decision (code) |
| Everything else here (TOML shapes, the grant-store schema and queue, the store's file names and per-entry actions, the decision pipeline, dialog kinds and texts, gate state names, CLI syntax, walk file, the harness's dialog policy, the record-flood hook, the frames of Reset and Remove, the overlay chain, the text-value rules, the synthetic world's region and its `region` input, the harness's local-only answer and dialog `delay`, trace format, `escape` runner, exit codes) | This document, 2026-10-07 (revisions 2 to 6 the same day) | Design |

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
| **Grant table** | The ext host's in-memory view of the grants, kept on the main thread: the last understood document read or written (the **document view**), combined with the session overlay (§8.5). Phase B, the dialogs and the script manager read it; natives never do |
| **Grant-store queue** | The one serial worker queue that performs every read and write of the grant store, and the store-root scan (§8.5, §8.7) |
| **Effective set** | The capabilities an apartment may use: decided at its load (§7.1), shrunk by every revocation during its life, never grown (TM §8.23). Natives check it, never the grant table |
| **Session overlay** | The user's decisions of this client session that the grant store document does not hold, kept per key as a short chain whose newest entry decides; every re-read result is combined with it, per key the newest decision wins over the completion of every earlier write, and a failed load write taken back restores the entry it superseded (§8.5, TM §8.23) |
| **Understood document** | A grant-store read that Tessera can use: read whole, of a version the build reads, not damaged as a whole; a missing file is understood as empty. Only such a read narrows or widens anything (§8.5, TM §8.23) |
| **Classify, then open** | An existing entry is classified with `fstatat(…, AT_SYMLINK_NOFOLLOW)` relative to its directory handle (Windows: the listing's attributes and reparse tag) before it is opened; only an entry of the expected kind is opened, non-blocking, and checked again on the handle (§3.4, §3.6; TM §8.8) |
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
2. **The script folder itself** is classified as in step 3 and opened relative to that
   handle with `O_NOFOLLOW | O_DIRECTORY | O_NONBLOCK` on POSIX; on Windows, relative to the parent handle with
   `FILE_FLAG_OPEN_REPARSE_POINT`, and a reparse point is refused (TM §8.8). So a symbolic
   link named `data/scripts/<id>` (from an unpacked archive, say) never brings outside
   files into the snapshot. The data directory itself may be a link (it is the user's
   configuration).
3. The walk proceeds only through directory handles, and **classifies every entry before
   opening it** (TM §8.8): `fstatat(dirfd, name, AT_SYMLINK_NOFOLLOW)` relative to the
   handle of the directory being listed (Windows: the listing's attributes and reparse
   tag). An entry classified as a symbolic link, a reparse point, a FIFO, a socket, a
   device node or any other kind that is neither a regular file nor a directory refuses
   the script **without being opened**, so no open ever waits on a FIFO and no device is
   opened with side effects. A directory is then opened with `O_RDONLY | O_DIRECTORY |
   O_NOFOLLOW | O_NONBLOCK`, a regular file with `O_RDONLY | O_NOFOLLOW | O_NONBLOCK`
   (Windows: relative to the parent handle with `FILE_FLAG_OPEN_REPARSE_POINT`, a reparse
   point refused), and each is checked again **on the opened handle** (`fstat`): the same
   kind, a link count of 1 for a file, and the same file as classified (device and inode;
   Windows: volume serial and file id). Any difference (an entry swapped between the two
   checks) refuses the script. The walk counts every entry it reads, and stops reading a
   listing as soon as the entry cap is reached. `scripttool check` uses this same walk
   (§11.3). TM X54 tests it: a symbolic link, a hard link, a FIFO, a device node, a
   swapped entry and a linked script folder each refuse the script, and no asset worker
   blocks.
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
  `data/scripts`" and lists what it read. Each entry is classified with `fstatat(…,
  AT_SYMLINK_NOFOLLOW)` and never opened by the listing. An entry that is not a directory
  (a link included), or whose name is not a valid id, is listed as "Not a script folder"
  with its name shown as plain text (TM §8.2's display rule), and is never loaded.
- With developer mode off, `data/scripts-dev/` is not listed, so the host does not know
  which developer folders exist. Records of kind `scripts-dev` are then shown as
  "Developer script (developer mode is off)" and are **never** classified as orphans
  (§8.7); they offer no Remove until developer mode is on and the listing shows that
  their folder is missing.
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
the `s_<id>` handle, never by a path. An existing entry is **classified before it is
opened** (`fstatat(…, AT_SYMLINK_NOFOLLOW)`; Windows: the listing's attributes and reparse
tag), and only a regular file is ever opened. **Every** open, for reading or for writing,
uses `O_NOFOLLOW | O_NONBLOCK` (POSIX) or `FILE_FLAG_OPEN_REPARSE_POINT` with a
reparse-point refusal (Windows), so a FIFO swapped in after the classification never
blocks a worker (a non-blocking read open of a FIFO returns at once; a non-blocking write
open of a FIFO with no reader fails with `ENXIO`) and a device node is never opened on
purpose. The entry's kind, link count and identity (the same file as classified) are
checked again **on the opened handle**; a mismatch is handled as the entry's row below
says for an entry that is not a plain file. A **plain file** is a regular file whose link
count is 1. New files are created exclusively (`O_CREAT | O_EXCL | O_NOFOLLOW |
O_NONBLOCK`, or `CREATE_NEW`); a rename (`renameat`) only replaces a directory entry and
never writes through it. A flush's rename of `t_<key>.tmp` over `k_<key>.store` first
classifies the destination: a destination that exists and is not a plain file is removed
first (no-follow), as the table says for such an entry. A **no-follow removal** removes
the entry itself (`unlinkat` without `AT_REMOVEDIR`; on Windows, deleting the reparse
point or file opened with `FILE_FLAG_OPEN_REPARSE_POINT`), so a link's target and a hard
link's other names are never touched.

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

**Log files.** Every log file is classified, opened, created and renamed relative to the
`logs/scripts/<kind>` handle, on the log writer's worker:

1. **Open.** `s_<id>.log` is classified first (`fstatat(…, AT_SYMLINK_NOFOLLOW)`). If it
   is missing, it is created with `O_WRONLY | O_CREAT | O_EXCL | O_APPEND | O_NOFOLLOW |
   O_NONBLOCK`. If it is a regular file, it is opened with `O_WRONLY | O_APPEND |
   O_NOFOLLOW | O_NONBLOCK` (Windows: `FILE_FLAG_OPEN_REPARSE_POINT`, append access, a
   reparse point refused). Any other kind is never opened. The opened handle is checked
   again: a plain file, the same file as classified. So a FIFO at that name never blocks
   the log writer: it is not opened when classified as a FIFO, and one swapped in after
   the classification makes the non-blocking write open fail with `ENXIO` (or, if a
   reader holds it open, fails the check on the handle, and the handle is closed unused).
2. **Rotation.** Before renaming `s_<id>.log` over `s_<id>.1.log`, the writer classifies
   `s_<id>.1.log`. If it exists and is not a plain file (a link, a reparse point, a FIFO,
   a device node, a directory, more than one link), the rotation is **not made**: the
   entry is left alone and the log becomes unavailable. Otherwise (missing, or a plain
   file) the rename is made; a rename replaces only the directory entry, never writes
   through it and never follows it.
3. **Unavailable log.** When the `<kind>` directory is unusable, or a log file is not a
   plain file, or a create, open or rotation fails, the log is **unavailable** for that
   script until the next load: nothing is written, the entry is left alone, the script
   still loads and runs, and the script manager reports "The log of notes · installed
   cannot be written: logs/scripts/scripts/s_notes.log".

Tessera never opens, writes, follows or removes a log entry that it classified as not a
plain file. The one entry a rotation can replace without having classified it as plain is
an `s_<id>.1.log` swapped in between the classification and the rename; the rename then
replaces that directory entry itself (a link is replaced as a link, never followed, and
nothing behind it is touched).

**Tests.** TM X94 (the ST part through `escape`, §11.8, the U part in `tessera-ext-host`;
it includes a log file and a rotated log file that are FIFOs or device nodes, and fails if
the log writer blocks); TM X54 for the script folder.

---

<a id="s4"></a>
## 4. The manifest `script.toml`

The resource bounds of the manifest are TM L37 (size, nesting, keys), L38 (ids), L60
(actions and contexts), L80 (display strings) and L87 (the per-field counts: capabilities,
modules, settings, choices, `max_length`, `version` and `license`); a number repeated
next to such a citation is for the reader only. TM registers only resource limits (TM
§0.1 rule 6). The other numbers of this section stay here and carry no L-number:
**argument-validity domains** (the settings' `min`/`max` bound of ±1e15) and **grammar
lengths** (setting keys 1–32 bytes, capability names 1–32 bytes, module-name segments
and version parts, action and context names up to 59 − the length of `id`), together with
each field's character set. (OPEN O-11 is closed by TM rule 6, §15.)

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
  anywhere (`check` code M03, §11.3). Keys are case-sensitive.
- **Dotted capability names.** In TOML a bare key `entity.read = "…"` is a dotted key that
  makes a nested table. Inside `[capabilities]` and `[optional_capabilities]` the parser
  reports such a nested table as "capability names containing '.' must be quoted
  (`\"entity.read\" = …`)" and refuses the script.
- **Integers and floats.** A field documented as an integer refuses a float (`memory_mib =
  48.0` is refused); a field documented as a number accepts both and stores an `f64`.
  Non-finite numbers (`inf`, `nan`) are refused everywhere. Both refusals are `check` code
  M03 (§11.3).

<a id="s4-2"></a>
### 4.2 Top-level fields

D17 §2's field table lists `id`, `name`, `version`, `author`, `description`, `license`,
`api`, `entry`, `capabilities`, `optional_capabilities`, `memory_mib`, `[[contexts]]`,
`[[actions]]`, `[[settings]]` and `autoload`. The rows marked **(design addition)** are
this spec's.

| Key | Type | Required | Rule |
|---|---|---|---|
| `manifest` | integer | yes | **(Design addition.)** The manifest format version this file uses (§12.1), at least 1. First release: `1` |
| `id` | string | yes | §3.2; equals the folder name (§4.5) |
| `name` | display string | yes | at most 48 cp (§4.4) |
| `version` | string | yes | Semantic Versioning 2.0.0 (`1.4.0`, `2.0.0-beta.1`, `1.0.0+build.5`); at most 64 bytes; every numeric identifier at most 9 digits and without leading zeros (TM L87). Informational only (§12.4) |
| `author` | display string | yes | at most 48 cp. It **pins grants and the store**: a change re-asks every grant and empties the `Store` (§9.5) |
| `description` | display string | no | at most 512 cp; may contain line feeds (U+000A), at most 8 lines |
| `license` | string | no | at most 64 bytes (TM L87) of printable ASCII (0x20–0x7E). An SPDX licence expression is recommended; `check` warns when it is absent (W04) |
| `api` | integer | yes | The major version of the `Ext` global's own entries (`Use`, `Has`, `Load`, `Version`; EA1a §3) the script targets (§12.2), at least 1. First release: `1` |
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
- **`input` is required to declare (design addition).** A manifest that declares any
  `[[contexts]]` or `[[actions]]` must list `input` in `[capabilities]` or
  `[optional_capabilities]`; otherwise it is refused ("actions and contexts need
  capability `input`", M14 or M15). The host **registers** the script's contexts and
  actions (as `ext.<id>.<name>`, counting toward TM L60's caps for all scripts), shows
  them in the binding UI and applies their suggested defaults (§4.11) **only when `input`
  is in the apartment's effective set** at load. With `input` optional and not granted
  (denied, undecided, or withheld by §7.1 layers 2–5), nothing is registered, nothing
  counts toward L60, and the binding UI does not list them. A later revocation of `input`
  suspends the script (§9.7); what `Input` made is then gone as EA1a §2.9 says.

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
- **No two overlapping suggested defaults per script and context** (TM §8.16, TM L61).
  Two actions of one manifest in the **same context** whose suggested strokes
  **overlap** refuse the script ("two actions suggest overlapping strokes"), whatever
  their `on` values. Two strokes overlap when one press can complete both: when there is
  a set of held buttons and modifiers and one button press under which KM §7.2's
  "completed by" holds for both strokes, each under its own action's modifier policy
  (exact for buttons, subset for axes, KM §2), with the modifiers that a stroke's own
  buttons contribute ignored as KM §5.1 says. Because KM §7.2 fires every complete
  matching binding of the consuming context, a chord and a single-button binding in one
  context both fire when the chord completes. So, for example:
  - equal strokes after KM §5.1's normalisation (`Ctrl+N` twice) overlap;
  - `A+B` and `B`, and `A+B` and `B+C` (press `B` while `A` and `C` are held), overlap:
    any two strokes that share a button can both be completed by a press of it, unless
    their modifier conditions exclude each other;
  - `Ctrl+B` and `B`, both button actions (exact): no overlap (no held modifier set
    equals both `{Ctrl}` and `{}`); `LCtrl+B` and `Ctrl+B` overlap (the `Ctrl` that `LCtrl`
    contributes is ignored for the first and meets the second);
  - for axis actions (subset policy): `Shift+Wheel` and `Wheel` overlap, because holding
    `Shift` meets both; and a button action `Ctrl+B` with an axis action `B` overlaps;
  - strokes with no button in common never overlap.

  The check is a finite one over the two strokes' buttons and the modifier sets.
  Defaults in **different** contexts of one script are not compared (KM §7.2: only the
  higher context fires), nor are a script's defaults compared with another script's or
  with the user's bindings. So one press can never start more than one hold from one
  script's suggested defaults.
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

- **Text-value rules.** A `text` value (its `default`, a value the user types, a stored
  value) is checked against these rows of §4.4, and only these: **"Removed characters"**,
  without its exception (U+000A is refused too: a text value is one line);
  **"Variation selectors"**; and **"Combining marks"**. The other rows do not apply:
  a text value may be empty ("Not empty" does not apply) and may start or end with white
  space, which is kept exactly as typed ("No surrounding space" does not apply; a value of
  spaces only is valid), and §4.4's "Length" row is replaced by `max_length`, counted in
  UTF-8 bytes. Everywhere this spec says a text value "follows the display-string
  character rules", it means these text-value rules.
- `default` of a `number` lies in `min..=max`; of a `choice` is one of the values; of a
  `text` is at most `max_length` bytes and follows the text-value rules above.
- A key that does not belong to the setting's type (`choices` on a `bool`) refuses the
  script, as every unknown key does.
- **Valid for the schema.** A stored value is valid for a setting when: for `bool`, a
  boolean; for `number`, a number in `min..=max`, and an integer when `integer` is true;
  for `choice`, a string equal to one of the `choices` values; for `text`, a string of at
  most `max_length` bytes that follows the text-value rules above. Anything else is
  invalid, a value of another type included.
- **Stored values** are kept in the grant record (§8.3). When a load finds a stored value
  that is no longer valid for the current schema (the type changed, the range shrank, the
  choice vanished, or a hand edit put an out-of-range number or an over-long text in the
  record), the value reverts to the default, with one line in the script log.
  Such values, and stored values of keys the schema no longer has, are **dropped by that
  load's record write** (§9.2 step 6): the key is removed from the record's `settings`.
  A load that has nothing else to update writes the record for this alone, so the drop
  happens once, at the first load that finds it. A reverted value is never written as its
  default: an absent key takes the manifest's default, so a later change of that default
  reaches the user. A load that writes nothing (Deny, "Not now", a refusal) leaves the
  stored values as they are; the next load drops them.
- Text values typed by the user follow the same text-value rules; the settings page
  refuses forbidden characters as they are typed.
- A change the user applies is written as §8.5 says and reaches a loaded script through
  `Core.Setting` and its settings-changed handler (EA1a §4.1, §4.4).
- **At a re-read** of the grant store (§8.5), every changed `settings` value of a loaded
  script's record is checked against the **running apartment's** manifest schema by the
  rule above before the script sees it. A valid value is delivered as a change the user
  applied. An invalid one (for example a hand-edited `max_notes = 1e15`, or a text longer
  than its `max_length`) is never delivered: the setting takes its manifest default, one
  line goes to the script log, and the script gets a settings change only if the default
  differs from the value it last read, carrying the default. So `Core.Setting` always
  answers a number in its range and a string within its cap, as EA1a §4.4 requires. A
  re-read writes nothing; the next load's record write drops the invalid value as above.

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
| `ui` | O | Offline, TesseraProtocol | no | 1a | Show its own windows and labels. They take clicks and the mouse wheel where they lie. To remove them, press ⟨chord⟩ (stops every extended script) or unload the script in the script manager. |
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
- ⟨chord⟩ in the `ui` text is the panic chord as currently bound in Tessera's settings
  (default `Ctrl+Shift+F12`, TM L64), in KM §5.1's written form. The text names the
  user's two routes to remove ext windows that catch the pointer, as EA1a §7.11 requires
  of the grant dialog.

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
where the dialog may ask (§9). The result is the apartment's **effective set** (§2). It
is never grown during the apartment's life. A narrowing during its life, by the script
manager (§9.7) or by a re-read of the grant store (§8.5), removes the revoked capabilities
from it and suspends the script in that frame (TM §8.23). Natives and the apply step of
every intent check the capability on every call (D17 §5.1) against this effective set,
**never** against the grant table, so a widening, by any route, applies only at the next
load (TM §8.23 "What natives check").

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
or from autoload at world entry (D17 §4, "Lifecycle"). Autoload loads the records of the grant table
(§8.5) whose `autoload` is true and whose folder the listing of §3.5 found (so a
`scripts-dev` record only with developer mode on), in the **script-list order**; a script
without a valid record is never autoloaded. The script-list order (used by autoload, the script
manager's list and the dialog queue of §9.1) is: first the valid records that `order`
names, in `order`'s sequence; then the valid records that `order` does not name, in byte
order of `"<kind>/<id>"`. A record created by an Allow is appended to the end of `order`
by that Allow's write; "Move in the list" (§9.7) rewrites `order` with every valid record.
`order` entries of records that are invalid stay in place and are skipped, so a repaired
record keeps its place; entries that are malformed, repeat an earlier entry, or name a key
with no `[[script]]` table at all are skipped and dropped by the next write (§8.5 write
step 5), so `order` never names more keys than the document has tables (TM L79). Every load runs these
phases. The gate state (§10) must be
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
TM §8.10, EA1b §8. A full reload's load starts only after the old apartment's close item
has run and its cap has left the sum at its final flush, so it waits for that flush
rather than being refused: §9.8, TM L84.)

| Phase | Where | What |
|---|---|---|
| **A. Read** | asset worker | The walk (§3.4), the content hash (§5), the manifest parse and field validation (§4.1–§4.13). Result: the snapshot, the hash, the parsed manifest, or the folder or manifest findings |
| **B. Decide** | main thread, native, outside ext time; its file work on workers | Registry checks (§4.7, §4.8); "one loaded script per id" (§7.4); the effective set before the grant (§7.1 layers 1–5); the decision pipeline of §9.2: no dialog, or a dialog that waits for the user. Ends with the record written (§8.5) or the load refused or cancelled. The main thread does no file I/O: the record write is a job on the grant-store queue, and the store probe (§8.8, needed by §9.2 step 2 and §9.3 "Stored values") a job on the store's I/O queue; phase B waits for their completions |
| **C. Open the store** | the (kind, id) store's I/O queue, on a worker | The store open of §8.8: the directory handles and the entry sweep of §3.6 (leftover temporary files and unexpected entries removed), the owner-tag check and the wipes, the durable owner tag and `local-only` marker, the file index (TM rule 7.6.1). Queued only after phase B's record write has completed, and after every earlier job of that store (D17 §8, "`Store` I/O") |
| **D. Snapshot result** | ext queue item (6a) | Delivered only after phase C completed. Then one compilation item per source file, then the top-level run (D17 §2, §4). The delivery and the top-level item each first **re-check** the load's decided set against the (kind, id)'s record in the grant table (§7.5); a narrowing cancels the load. A compilation failure ends the load as §4.14 says |

- Phases A to C together are D17's "snapshot job": the marker, the owner tag and the wipe
  are never a main-thread step or a 6a item. Phase B sits between them because the store's
  fate depends on the grant decision, which depends on the hash.
- A load that is refused in phase A or B never creates an apartment and never touches the
  store (the store probe only reads).
- The content hash pinned in phase B is the hash of the very bytes that phase D compiles.

<a id="s7-4"></a>
### 7.4 One loaded script per id

At most one script per id is loaded at a time, across both kinds (D17 §2, TM L26). A load
of an id is refused while the same id of the **other** kind is **loaded or loading**: from
the moment that other load took its memory reservation (at the start of phase A, §7.3)
until its close item has run, or until it was refused or cancelled and released its
reservation (TM §8.23 "One id, one apartment"). The check is made when the new load takes
its own reservation, on the main thread, and again in phase B; so of two loads of one id
from both kinds that start in the same frame (for example two `autoload = true` records
`scripts/notes` and `scripts-dev/notes` at world entry with developer mode on), the one
started first in that frame's order (here the script-list order) proceeds and the other is
refused before phase A. The refusal says "notes · developer is loaded; unload it first"
(or "… is loading"), and the script manager offers that unload. A second load of the
**same** (kind, id) while it is loaded or loading is ignored, except `/ext:reload`, which
is a reload (§9.8). TM X90's ST part tests both cases (§11.8).

<a id="s7-5"></a>
### 7.5 A narrowing during a load

A **narrowing** of a (kind, id) is any of: a manager revocation (§9.7), Reset (§9.6),
Remove (§8.7), a local-only switch (§9.4), and a re-read of an understood document whose
result narrows the record (§8.5; TM §8.23 "What narrows by another route"). Each is
applied to the grant table in one frame on the main thread: Reset and Remove too, whose
"no record" enters the session overlay in the frame the user chooses them, although
their grant-store write follows the store's deletion (§8.7 "Reset and Remove, step by
step"). For a load of that (kind, id) that is in progress, the rule is TM §8.23 "Loads in
progress": a load never proceeds without a capability revoked after its decision (D17 §2,
"never loads degraded"). The load's **decided set** is the effective set phase B decided.

| Where the load is when the narrowing is applied | What happens |
|---|---|
| Phase A running or its result not yet consumed | After a revocation, a local-only switch or a re-read: nothing yet, because phase B has not decided; it runs §9.2 against the narrowed grant table. After **Reset or Remove**: the load is cancelled, its result discarded when it arrives and its reservation released, because the (kind, id) is then **pending deletion** (§8.7) |
| Phase B before its dialog, or with its dialog showing or queued | The dialog closes as "Not now", or is never shown; nothing is written; the load is cancelled. A store probe already submitted completes and its result is discarded |
| Phase B past its dialog, waiting on its record write or store probe | The write or probe completes in order on its queue (the narrowing's own write follows it on the grant-store queue, so the document ends narrowed). Then the load is cancelled with its reservation released, before phase C |
| Phase C queued or running | It completes on the store's queue; its result is discarded and the load is cancelled. Reset's or Remove's store deletion is queued behind it on the same queue |
| Phase D: snapshot result or compilation items, top-level run not started | The re-check at the delivery or at the top-level item finds the narrowing: remaining items are dropped and, if the apartment's state exists, it gets **one close item** (no `Unload` handler; no script code ran) |
| Top-level run finished (the script is loaded) | Not a load in progress: the narrowing suspends the script as §9.7 says |

**The re-check.** Phase D's delivery and the top-level item each compare the decided set
with the `granted` list of the (kind, id)'s record in the grant table (after the session
overlay, §8.5). If the record is gone, or any capability of the decided set is no longer
granted, the load is cancelled as the table says. A widening of the record changes
nothing for the load. A narrowing applied in the same frame before the item runs is seen
by it; one applied after the top-level item has run meets a loaded script. The completion
of an **earlier** grant-store write never counts as a narrowing for the keys that write
carried (§8.5): a load that decided on a newer decision of the same key (a widen, or a
first-enable Allow after Reset or Remove) is never cancelled by it (TM §8.23, X91).

Every cancelled load is reported "Not loaded · notes · installed: its permissions changed
while it loaded" and appears in the trace as `load` with `r` = `cancelled` (§11.6). TM X91
tests a required grant revoked while a load is past its dialog and before its top level.

**Other cancellations of a load in progress (TM §8.23).** Two events that are not
narrowings cancel a load in progress by the same rows, with one difference in the first
row: the load is cancelled **at once** in phase A too (its result discarded when it
arrives, its reservation released), because nothing would decide it again.

- **Developer mode turned off** (§9.8) cancels, in that frame, every load in progress of
  kind `scripts-dev`, in the script-list order: a grant dialog showing or queued for it
  closes as "Not now" (nothing written); a load past its dialog waits for its record write
  or store probe to complete and is then cancelled before phase C; phase C completes on
  the store's queue and its result is discarded; in phase D the apartment's state, if it
  exists, gets **one close item** and the top level never runs. A hot reload in progress
  (slice 1b) has its new apartment's load cancelled this way and its old apartment
  unloaded as a loaded script (§9.8). Each is reported "Not loaded · hud · developer:
  developer mode was turned off". A developer load that is still **waiting** (a
  user-started load queued in `Starting`, §10.3, or a reload request not yet started,
  EA1b §8) is refused with "Developer mode is off" and never starts.
- **The user's unload of a loading copy** (the unload the script manager offers for a
  script that is loading, §7.4, or the walk's `manager` `unload` of a loading script,
  §11.5) cancels that load the same way, reported "Not loaded · notes · installed:
  unloaded by you".

Both appear in the trace as `load` with `r` = `cancelled` (§11.6). So no developer script
finishes loading, under a grant decided in developer mode, while developer mode is off,
and no load the user ended reaches its top level (TM X90).

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
content_hash = "62eba7b09ef519d9c6a6fa1046fc1f340e2ff7483586b8dfad433deb362f59ab"   # illustrative only: §5.2's `hello` vector, not the note-taker's hash
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
| `format` | string | **Required.** `"tessera-ext-grants"`; a missing key, a value of another type or any other value: the whole document is damaged (§8.6) |
| `version` | integer | **Required.** The grant-store format version, `1` (§12.5). A missing key, a value that is not a TOML integer, or an integer below 1: damaged as a whole (§8.6); an integer above the version the build reads: **newer** (§8.6) |
| `order` | array of strings | Optional (absent: empty). The user's script list order, which autoload follows: entries `"<kind>/<id>"`, unique, at most 256 |
| `script` | array of tables | Optional (absent: no records). The grant records, at most 256 |

So an **empty file** (zero bytes), and a file of only white space and comments, are
damaged as a whole, not an empty understood document: they lack `format`. Only a
**missing** file reads as an understood, empty document (§8.6). (A file an editor has
just truncated while saving therefore changes nothing at a re-read, §8.5 step 1, instead
of narrowing every record.) Tessera never writes such a file: every document it writes
holds `format` and `version` first.

One record (`[[script]]`):

| Key | Type | Rule |
|---|---|---|
| `kind` | string | `"scripts"` or `"scripts-dev"` |
| `id` | string | §3.2 |
| `author` | string | The manifest's `author` when the record was last decided, exactly; display-string rules (§4.4) |
| `required`, `optional` | arrays of strings | The manifest's capability lists when the record was last decided (the **pinned capability list**): capability-name grammar (§6.1), each at most 16 entries (TM L79: the reader's bound per list; a record Tessera writes holds at most 16 together, as the manifest does, TM L87), sorted in byte order, unique, disjoint |
| `granted`, `denied` | arrays of strings | The user's decisions: sorted, unique, disjoint, each a subset of `required` ∪ `optional`. A capability in neither is **undecided** |
| `memory_mib` | integer | 32–64: the memory the user accepted |
| `content_hash` | string | 64 lowercase hexadecimal digits (§5) |
| `content_hash_version` | integer | `1` |
| `autoload` | boolean | The user's "Load automatically" choice |
| `settings` | table | The `[[settings]]` values the user set: key (settings-key grammar) → boolean, number or string (at most 256 bytes, TM L79; the text-value rules of §4.12, so it may be empty); at most 32 entries (TM L79). A number is a finite TOML float, or a TOML integer of magnitude at most 2^53 (an argument-validity domain, TM §0.1 rule 6), read as the equal float; the writer always writes numbers as floats (`200.0`). Absent keys take the manifest's default. This is the record's own validity; whether a value fits the script's current schema is checked at load and at a re-read (§4.12) |
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
  records, the per-field caps above (TM L79). A document with more than 256 `[[script]]`
  tables or more than 256 `order` entries is damaged as a whole (§8.6), as an oversized
  one is;
- the document is read only through §8.5's read rule (non-blocking, a regular file checked
  on the handle, at most 4 MiB + 1 bytes);
- unknown keys at the root and in a record are **kept** and written back unchanged (so a
  newer Tessera's fields survive an older one), but never interpreted;
- a record is **invalid** when any rule of §8.3 fails, including the invariant above. An
  invalid record is ignored for the session, as if absent, and reported once in the script
  manager ("The saved permissions of notes · installed are damaged and were ignored");
- two records with the same (kind, id) are **both** invalid. A `[[script]]` table
  **belongs to** the (kind, id) its `kind` and `id` strings name when both are present
  and valid; a table without such a key belongs to none (it is invalid, and every write
  keeps it unchanged, §8.5 write step 4). It still counts toward the 256 tables;
- `order` entries that name no valid record, or are malformed, are ignored; those that
  name no table at all, are malformed or repeat an earlier entry are dropped by the next
  write (§8.5 write step 5).

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
### 8.5 Reading and writing: the grant table, the session overlay and the grant-store queue

No blocking file I/O happens on the main thread after start-up (ARCH §21), and D17 keeps
all ext file I/O on workers. Nothing on the grant-store queue ever waits on another
program (TM §8.8 "Tessera's own files"). So:

- **The grant-store queue.** Every read and write of `ext-grants.toml`, and the store-root
  scan of §8.7, is a job on one serial queue that runs on a worker of the ext host (never
  the main thread, and never as an asset-worker job, so it never delays asset decoding),
  one job at a time, in submission order.
- **Three views, kept on the main thread.**
  - The **document view**: the valid records, `order`, unknown keys and the number of
    `[[script]]` tables of the last **understood** document (below) that a job read or
    wrote (empty before the first such read, and so empty for the whole session when
    the start-up read is unreadable, damaged or newer, §8.6), and whether the document is
    **writable** (it is not, for the rest of the session, once a read or a write has found
    its `version` newer, §8.6). A read that is not understood never replaces it (TM §8.23).
  - The **session overlay** (§2, TM §8.23): for each (kind, id), the decisions of this
    client session that the document does not yet hold, as a **chain** of entries,
    oldest first. Each entry is a whole record, or "no record" after Reset or Remove.
    The **newest** entry of the chain is the key's **current entry**, and only it
    decides the grant table; the older ones are kept only so that a rollback can
    restore them (below). A decision applied to a key appends its entry to the key's
    chain (it **supersedes** the entry before it). An entry leaves the chain only in
    three ways: when a write that carried it completes successfully, that entry and
    every **older** entry of the chain are dropped (the record just written was decided
    from a grant table that already held them, so the document now holds what they
    said; newer entries stay); when it is a failed load write taken back (§8.5 "A failed
    write"), that entry alone is removed, wherever it is in the chain, and the entry
    after it then supersedes the one before it; and at the end of the session. A
    decision whose write fails otherwise stays for the rest of the session, and every
    decision stays while the document is not writable. Only a load's own record write
    is ever taken back, so when any **other** entry is appended (a manager action, a
    manager-started dialog's Allow, Reset, Remove, a local-only switch, a narrowed
    entry of step 2 below), every older entry of its chain is dropped: no rollback can
    reach past it. While the document is not writable nothing is ever taken back, so
    every append drops the older entries. With one load of a (kind, id) at a time
    (§7.4), which waits for its own write before phase C, a chain therefore holds at
    most two entries, an older decision and one load write above it, each at most one
    record (TM L79). A failed load write whose entry an append has already dropped
    restores nothing: the newer decision stays current.
  - The **grant table**: the document view combined with the overlay. For a (kind, id)
    with an overlay chain it holds the current entry's record (or none); otherwise the
    document view's valid record (or none). Phase B, the dialogs, the script manager and the
    re-checks of §7.5 read the grant table. **Natives never read it:** every native and
    every intent's apply step checks the apartment's **effective set** (§2, §7.1; TM
    §8.23 "What natives check"), which a revocation shrinks and nothing grows during the
    apartment's life. So after the user revokes optional `camera`, resumes the script and
    widens `camera` again in the manager, the grant table grants `camera` but the kept
    `Camera` natives still raise `"not granted"` until the next load (TM X91).
- **A decision** (a dialog's Allow, a load's record update of §9.2 step 6, a manager
  action of §9.7) is appended to its key's overlay chain as the current entry, and so
  applied to the grant table, at once, in that frame, and is submitted as a write job
  unless the document is not writable. Reset and Remove are applied the same way, but their write is submitted only when the store's
  deletion has completed (§8.7 "Reset and Remove, step by step").
- **Reads.** The document is read every time the gate enters `Starting` (each session
  start; the first of them is the **start-up read**), as the first of the two jobs the
  gate waits for there, the store-root scan of §8.7 being the second (the gate does not
  leave `Starting` before both results are applied, §10.3; TM L92), and each time the
  script manager opens (to pick up hand edits). A read at a later session's `Starting`
  is applied like any re-read (steps 1–5 below); no script is loaded then (§10.3), so it
  suspends nothing. The configured path is first classified (`stat`, following the symbolic links
  in the path, at most 40, as a write does) and opened only when it is a regular file,
  so a device is never opened on purpose; it is then opened **non-blocking** (`O_RDONLY |
  O_NONBLOCK`), the handle is checked again to be a **regular file**, the same one (`fstat`
  on the handle; a FIFO, socket, device or directory swapped in is never read and never
  waited on), and at most 4 MiB + 1 bytes are read
  (TM L79, TM §8.8 "Tessera's own files"). A missing file reads as missing; a failed open
  or read (`EACCES`, `EIO`, `EISDIR`, a file that is not regular; on Windows also a
  sharing violation while an editor or a virus scanner holds the file) reads as
  **unreadable**. The bytes are then classified by §8.4 and §8.6.
- **Three kinds of read result** (TM §8.23 "What narrows by another route"):
  - **understood**: a document read whole, of a version the build reads, not damaged as a
    whole (§8.6); a **missing** file is understood as an empty document;
  - **newer**: a document whose `version` is newer than the build reads (§8.6);
  - **unusable**: unreadable, or damaged as a whole (§8.6).

  Only an understood result can replace the document view, and so only it can narrow or
  widen anything.
- **Applying a read result.** On the main thread, in the frame its completion is seen,
  with P the current document view and R the result:
  1. **Newer or unusable** (TM §8.23 "A re-read that cannot be used changes nothing").
     P stays the document view and the base of the next comparison; the grant table does
     not change; nothing narrows, widens or is suspended, whatever R holds; the script
     manager reports the case (§8.6). A newer R also makes the document not writable for
     the rest of the session (§8.6), so every decision from then on stays in the overlay.
     A transient failure when the script manager opens therefore never suspends a script
     and never turns a session decision into `denied`. Steps 2–5 are skipped.
  2. **Narrowings by the document** (R understood). For each (kind, id) that is not an
     **excluded key** (a write's completion, below), the capabilities that P's record
     grants and R's record does not are **narrowed by the document**: all of P's when R
     has no valid record of that key (removed, made invalid, a duplicate, or the file
     missing). In **every** entry of that key's overlay chain, those capabilities are
     moved from `granted` to `denied` (so an entry that a rollback restores later is
     narrowed too). So a hand edit that narrows always narrows, and the document never
     undoes a decision of this session: a revocation whose write failed stays revoked,
     whatever a later read finds. **A write in flight** **(design addition)**: if a
     write job that carries that key is queued or running when step 2 narrows its chain,
     that job would put the capability the user removed by hand back on disk. So, in the
     same frame and when the document is writable, the narrowed current entry is
     appended to the chain as a new entry (it is not a load write) and a write job
     carrying it is submitted; it runs after the earlier job on the grant-store queue,
     so the document ends narrowed. An entry whose write has already failed needs no
     such write: the document R already holds the narrower record.
  3. R becomes the document view, and the grant table is recomputed.
  4. **Loaded apartments.** For each loaded or suspended apartment whose unload has not
     been requested, its effective set is compared with the `granted` list of its (kind,
     id)'s record in the new grant table:
     - **Narrowed** (a capability of the effective set is no longer granted): a
       revocation by another route. The script is **suspended in this frame**, exactly as
       §9.7's "Uncheck" says (a revoked required capability: unloaded with one close item
       and no `Unload` handler; only optional ones revoked: the user may resume it, and the
       revoked modules then raise `"not granted"`). The script manager says "The saved
       permissions of notes · installed changed; the script was suspended".
     - **Widened** (a capability granted that the effective set lacks) or otherwise
       changed (`author`, pinned lists, hash, memory): nothing changes for the running
       apartment. The record is used from the script's next load, which runs §9.2 against
       it.
     - A changed `settings` value is checked against the running apartment's schema and
       delivered, or replaced by the default, as §4.12 "At a re-read" says.
     - `autoload` and `order` take effect at their next use.
  5. **Loads in progress.** A narrowing of a (kind, id) whose load is in progress is §7.5's.
- **A write job** carries the overlay entries it writes (whole records, or "no record" for
  a removal; their keys are the job's **carried keys**) and, after "Move in the list", the
  new `order`. It also carries a copy of every record of the grant table and of the
  document view's `order` as they stood when it was submitted, used only by step 3. On the
  queue it:
  1. **Locks.** It takes the grant store's lock (below) without blocking: a lock that
     another process holds is retried for at most 1 s of wall-clock time (TM L94), then
     the write **fails** (TM §8.8 "Tessera's own files").
  2. **Reads** the current document by the read rule above. If it is unreadable, nothing
     is written and the write fails; its read result changes nothing (step 1 above). If
     its `version` is newer, nothing is written, the write fails as a **newer** failure
     (below), and its result is applied as step 1 above says.
  3. **Backs up** the document first (§8.6) when it is damaged as a whole, or when this
     job replaces or removes an invalid table (step 4). A damaged document is then
     replaced by a document that holds **every record of the grant-table copy** the job
     carries, in its `order`, with the job's own entries applied over them (TM §8.23
     "Replacing a damaged document narrows nothing"); every key it writes then counts as
     carried. So the replacement grants no less than the grant table did.
  4. **Replaces by key.** A `[[script]]` table **belongs to** a (kind, id) as §8.4 says.
     For each key the job carries, **every** table that belongs to it (its valid record,
     an invalid one, or each of two or more duplicates) is removed, and the job's record,
     if any, is inserted once. **What the record keeps:** when exactly one table that
     belonged to the key was a **valid** record of the document this job read in step 2,
     the inserted record keeps that table's unknown keys (with their values, §8.4) and
     its `signing_key` (§8.3), so a newer Tessera's fields and a slice-4 key survive a
     decision of this build; every key §8.3 defines otherwise comes from the job's
     record. When the replaced tables were invalid, duplicates or absent, nothing of them
     is kept (the backup of step 3 holds them). A "no record" removes the tables with
     everything in them. Every other table, invalid ones and tables that belong to no key
     included, is kept with its keys and values unchanged, and so is every unknown root
     key. This is the only way an invalid table leaves the document.
  5. **Checks the caps (TM L79).** A record whose key has no table in the document is
     inserted only if the document then holds at most 256 `[[script]]` tables; otherwise
     the write fails with "256 scripts have saved permissions; remove one in the script
     manager". `order` is rewritten: an entry that is malformed, repeats an earlier entry,
     or names a key with no table in the resulting document is dropped; the key of each
     inserted record that `order` does not name is appended (a "Move in the list" job
     writes its own sequence, filtered the same way). So `order` never names more keys
     than the document has tables, and never exceeds 256 entries, and a written document
     never breaks §8.4's caps.
  6. **Commits.** It writes a temporary file in the destination's directory, flushes it to
     disk, renames it over the document, flushes the directory entry, releases the lock,
     and reports completion together with the document it wrote and its carried keys.

  A reader never sees a half-written document, and a completed write survives a crash.
- **Applying a write's completion** (TM §8.23 "Tessera's own writes never narrow the
  user's decisions"). The written document is applied as an understood read result
  (above) whose **excluded keys** are the write's carried keys. So:
  - for a key the write did **not** carry, the written document counts like a hand edit:
    another process's change that step 2 of the write found is honoured and narrows as
    step 2 above says;
  - for a carried key, nothing narrows and the grant table does not change. The entry
    this write carried, and every older entry of the key's chain, are dropped. If that
    entry was the current one, the document view now holds the same record. If a
    **newer** decision of that key was applied while the write was in flight, its entry
    stays exactly as the user made it, and its own later write follows on the queue: per
    key, the newest decision wins over the completion of every earlier write.

  Example (TM X91): a manager revocation of optional `b` and a widen of `b` whose dialog
  the user allows, both before the revocation's write completes. The revocation's
  completion leaves `b` granted; no load that decided on `b` is cancelled (§7.5); when the
  widen's write completes, the document grants `b`. Likewise, after Reset or Remove
  (§8.7) a first-enable Allow of the same key, made before the Reset's write completes,
  is neither undone by that completion nor suspended or cancelled by it.
- **A failed write.** A load's own record write (§9.2 step 6, including a load dialog's
  Allow) that fails is **taken back**: its entry is removed from the key's overlay chain
  (§8.5 "Three views"), which **restores the entry it superseded** as the current entry,
  or leaves the key with no overlay entry (the document view decides) when it superseded
  none. It never simply empties the key's chain, so a rollback never undoes an earlier
  session decision whose own write failed (TM §8.23 "A failed write never undoes an
  earlier session decision"). If a newer decision of that key was applied meanwhile, that
  decision stays current. The load is cancelled, and the script manager reports
  "Permissions could not be saved: <reason>". Example (TM X91): with the disk full, the
  user revokes optional `b` (its write fails; the revocation stays in the chain); a later
  load of the script with a changed hash appends its update, built from the grant table
  and so with `b` denied, and its write fails too; taking it back restores the
  revocation, so `b` stays revoked, and the next write that succeeds writes `b` as
  denied. Likewise a Reset whose write failed, followed by a first-enable Allow whose
  write fails: taking the Allow back restores Reset's "no record", and the erased store's
  old record never returns. Every other decision whose write fails (a
  manager action, a manager-started dialog's Allow) **stays in the overlay** for the
  session, so a revocation keeps acting and is never undone by a later read, and the
  manager reports the failure. A **newer** failure (step 2 found a newer `version`) is
  not a failure of the decision: the document becomes not writable, the decision stays in
  the overlay for the session, and a load waiting on that write continues to phase C
  under it, as if the document had been not writable when it decided. While the document
  is not writable, no write job is submitted, every decision lives in the overlay for this
  session only (§8.6), and phase C follows the overlay.
- **Lock and temporary file (named).** The lock is the file `.ext-grants.toml.lock` in the
  directory of the **resolved** `ext-grants.toml` (the settings crate's lock naming,
  `.<file name>.lock`, applied to this document; never the settings file's own
  `.settings.toml.lock`, so a grant write and a settings save never wait on each other).
  It is an advisory lock taken on that file's handle with a non-blocking attempt (step 1);
  the file is created if missing (`O_CREAT | O_NOFOLLOW | O_NONBLOCK`, a regular file
  checked on the handle), is never opened through a symbolic link, passes the install
  guard, and is **never removed** (removing it would race with other lockers), so it
  persists after the first write. The temporary file is created exclusively beside the
  resolved document with the settings crate's atomic-write naming,
  `.ext-grants.toml.tmp-<pid>-<nanos>-<n>`; it is renamed over the document on success and
  removed when the write fails, so only a crash leaves one behind. Backups follow §8.6.
- **Links and the install guard.** Symbolic links in the final path component are
  followed (at most 40) to the file that is really replaced, and the settings crate's
  read-only install guard must accept the resolved destination before anything is written
  (KM §6.5's rules). "Beside the document" always means the directory of that resolved
  destination.
- **Output form.** Records are written sorted by kind, then id, in byte order (tables that
  belong to no key last, in their document order); lists sorted; numbers of `settings` as
  floats. A document that Tessera wrote, read and written again without change, gives the
  same bytes apart from unknown keys' formatting.
- **Before the store is opened.** A load's record write (§9.2 step 6) must **complete**
  before phase C is queued, so a crash never leaves a store opened under a grant that the
  document does not hold. A load that decides under an **overlay entry** (the key's
  record in the grant table is a current entry whose write is queued, running or has
  failed) therefore writes its record even when §9.2 found nothing to update, while the
  document is writable; that write is a load write like any other (taken back and the
  load cancelled if it fails). With a document that is not writable, the decision applied
  to the overlay is what phase C follows.

<a id="s8-6"></a>
### 8.6 Damaged, missing and newer documents

| Case | Reading | Writing |
|---|---|---|
| Missing | Understood as empty: no records. At a re-read, every record of the previous document view is narrowed by the document (§8.5 step 2; TM §8.23) | Created by the first write |
| Unreadable: the open or the read fails (`EACCES`, `EIO`, `EISDIR`, a Windows sharing violation, …), or the path is not a regular file (a FIFO, socket, device or directory, never waited on, §8.5) | Not understood. At start-up every record is ignored (fail closed: the document view stays empty). At a re-read **nothing changes**: the previous document view stays, nothing narrows, widens or is suspended (§8.5 step 1; TM §8.23). Either way the script manager says "Permissions could not be read: <reason>", and at start-up the gate still leaves `Starting` (§10.3) | A write job reads the document first (§8.5 step 2) and fails while it is unreadable, so nothing is ever written over a document Tessera could not read; decisions stay in the session overlay (§8.5) |
| Damaged as a whole: not UTF-8, not TOML, over 4 MiB, `format` or `version` missing or wrong (§8.3; an empty file included), nesting too deep, more than 256 `[[script]]` tables or `order` entries | Not understood. At start-up every record is ignored (fail closed). At a re-read nothing changes, as for an unreadable one. The script manager says "Permissions are damaged and were not used" | The first write backs the document up (below), then writes every record of the grant table it carries with its own entries applied (§8.5 step 3), so the replacement narrows nothing (TM §8.23) |
| `version` newer than the build reads | Not understood: every record of it ignored. At start-up the document view stays empty; at a re-read the previous document view stays. Such a read never narrows, widens or suspends anything (§8.5 step 1). The script manager says "Permissions were saved by a newer Tessera and are not used" | **Never written** by this build for the rest of the session, so the newer build's data is kept. Dialogs still show; their decisions apply to this session only, in the session overlay (§8.5) |
| Some records invalid | Those ignored (§8.4) | Kept as they are, except by a write of their own key (§8.5 step 4), which first backs the document up |

**Backups (TM L79, TM §8.8 "Tessera's own files").** A backup is named exactly as the
settings crate forms its own backup names, `<file name>.<reason>-<stamp>[-<n>].bak`, with
the file name `ext-grants.toml` and the reason `corrupt`:
`ext-grants.toml.corrupt-<stamp>.bak`, or `ext-grants.toml.corrupt-<stamp>-<n>.bak` when
that name exists. `<stamp>` is the backup's time in whole Unix seconds (UTC) written as
**exactly 12 decimal digits, zero-padded** (`001791084283`); `<n>` is the first of 1, 2,
… 99 (decimal, no leading zero) that gives a name not yet present, tried with exclusive
creates. When all 100 names of one second exist, no backup is made and the write job
fails before anything is replaced. Only names of exactly this form count as backups
(for the cap of 8 below and for `no_file_outside`, §11.8). A backup lies in the directory of the
**resolved** document, beside the lock and temporary file (§8.5), never behind the
configured path's link. Its bytes are bounded:

- a document that the write job read whole (at most 4 MiB) is backed up by writing
  **exactly the bytes it read** to a backup file created exclusively (`O_CREAT | O_EXCL |
  O_NOFOLLOW`) and flushed to disk;
- a document over 4 MiB (the read stopped at 4 MiB + 1 bytes) is backed up by **renaming**
  the resolved document to the backup name, before the fresh document is renamed into
  its place.

Nothing ever copies more than 4 MiB of a damaged document, and nothing reads one past
4 MiB + 1 bytes, so a huge file or a link to an endless device costs a bounded amount of
time and disk (the read open is non-blocking and checked to be a regular file, §8.5). Each
write job makes at most one backup. At most 8 backups are kept: making a ninth removes the
oldest (by its name's time and counter) with a no-follow removal of that entry itself, as
the settings crate keeps its own.

<a id="s8-7"></a>
### 8.7 Records, stores and orphans

- A (kind, id) has one record and one `Store` (D17 §5.2). The store directory is created by
  phase C's first open after the record exists; they are removed together.
- **Orphan record:** a valid record of the grant table whose folder `data/<kind>/<id>/` the
  latest listing of §3.5 did not find as a directory. Records of kind `scripts-dev` are
  **never** orphans while developer mode is off, because `data/scripts-dev/` is not listed
  then (§3.5); and no record of a kind is an orphan while that kind's listing stopped at its
  cap (TM L88). An orphan record is listed with its stored-value size and a "Remove"
  button.
- **Orphan store:** a store directory that the store-root scan (below) found as
  `data/script-store/<kind>/s_<id>/` with no valid record of its (kind, id) in the grant
  table. It is listed the same way.
- **Remove** (TM §8.24). An orphan may still be loaded (its folder was deleted after its
  snapshot was read) or loading. So Remove first ends it: a **loading** (kind, id) is
  cancelled in any phase (§7.5); a **loaded** one gets the ordinary unload (its `Unload`
  handler item, then its close item with the final flush; a suspended one its close item
  only). Then the store's deletion is queued on the store's I/O queue, behind the final
  flush and every earlier job of that store, so no apartment's flush ever meets a removed
  directory. When the deletion has completed, Remove's grant-store write removes the
  record, and the id's bindings are removed from `keymap-ext.toml`, unless a record of the
  other kind with the same id remains (D17 §2). A crash between the two leaves a record
  with no store, which is harmless. The store's deletion is the wipe of §8.8 followed by a
  removal of the then empty `s_<id>` directory relative to its `<kind>` handle. Reset
  (§9.6) ends a loaded or loading script the same way and deletes the store's contents
  (the wipe; `s_<id>` itself may stay). The order of frames is below.
- **Reset and Remove, step by step** **(design addition**, TM §8.23, §8.24). Let F be the
  frame in which the user chooses Reset or Remove of a (kind, id):
  1. **In frame F**, "no record" for that key enters the session overlay (§8.5), so the
     grant table has no record from F on, and the (kind, id) becomes **pending
     deletion**. In the same frame a load of it in progress is cancelled (§7.5, every
     phase), and a loaded one has its ordinary unload requested. No grant-store write is
     submitted yet.
  2. **The deletion** is submitted on the store's I/O queue in the first frame in which
     the (kind, id) has neither an apartment nor a load in progress (frame F itself when
     it had none), behind every earlier job of that store (the final flush, a cancelled
     load's phase C).
  3. **When the deletion has completed** (frame G), the grant-store write that carries
     "no record" for the key is submitted in G (and, for Remove, the bindings' removal
     below), then a store-root scan (§8.7). The key stops being pending deletion in G.
     If the document is not writable (§8.6), no write is submitted and the "no record"
     stays in the overlay for the session.
  4. **If the deletion fails** (an unusable store directory, a subdirectory or an entry
     that cannot be removed, §3.6), it is reported ("The stored values of notes ·
     installed could not be erased: <reason>"), no grant-store write is submitted, and
     the key stops being pending deletion. The "no record" stays in the overlay for the
     session, as every failed narrowing does (§8.5), so nothing the user removed returns
     in this session; the document keeps the record, which keeps "the store is deleted
     before its record" true across a restart.
  5. **Between F and G** a load of that (kind, id) is refused, whoever starts it, with
     "notes · installed: its stored values are being erased; try again when that has
     finished", and nothing is decided, probed or written for it; autoload never starts
     one, because the key has no record. The script manager shows the key as "erasing".
     A second Reset or Remove of a key that is pending deletion is ignored.
  6. **From G on** a load of that (kind, id) is a first enable (§9.2 step 1a). Its store
     probe and phase C run on the store's queue after the deletion, so they find no
     store. Its Allow is a newer decision of the same key than the Reset's or Remove's
     "no record": its write follows on the grant-store queue, and the earlier write's
     completion neither undoes it nor cancels or suspends the load (§8.5 "Applying a
     write's completion"; TM X91).
- **The bindings' removal** **(design addition)** is a read-modify-write of
  `keymap-ext.toml` on a worker, made as §8.5's write job is, with KM §6.3's strict reader
  and KM §6.5's saving rules: the lock `.keymap-ext.toml.lock` in the directory of the
  resolved document (persistent, never removed, taken without blocking and retried for at
  most 1 s, TM L94, as in §8.5), the document read with a non-blocking open checked on the handle
  and bounded at its cap + 1 bytes (TM L47, TM §8.8), and a temporary file
  `.keymap-ext.toml.tmp-<…>` beside it, renamed on success and removed on failure. The
  binding UI's own saves follow KM §6.5 and create the same temporary-file form.

**Totals and the store-root scan (TM §8.7, TM L40, L41, L92).** The all-scripts `Store`
totals count every store of a (kind, id), orphans and scripts not loaded included (D17
§5.2). A loaded or loading store contributes its file index and pending entries (TM rules
7.6.1, 7.6.5). Every other store contributes the figures of the **store-root scan**:

- **When.** On the grant-store queue (§8.5), **every time the gate enters `Starting`**
  (each session start, not once per client run; TM L92), right after that entry's read of
  the grant store and before any load of the session (the gate does not leave `Starting`
  before it has completed, §10.3), and again after the store deletion of every Reset and
  Remove (step 3 above). So a session after a backend switch scans again, and its
  `store-scan` worker event appears in the trace after that session's `grant-read`
  (§11.6). A store leaves the scanned set when a load's phase C opens it (its file index
  counts from then) and joins it again, with its file index's last figures, when its
  apartment's final flush has completed. So the totals are current without rescanning
  stores that are open.
- **How.** The scan opens `data/script-store/` and each `<kind>` directory as §3.6 steps
  1–2 say, but never creates one (a missing one has no stores). It lists the `<kind>`
  directory through its handle and classifies each entry with `fstatat(…,
  AT_SYMLINK_NOFOLLOW)`. An entry named `s_<id>` with a valid id (§3.2) that is a
  directory is opened relative to the `<kind>` handle with `O_RDONLY | O_DIRECTORY |
  O_NOFOLLOW | O_NONBLOCK` (Windows: `FILE_FLAG_OPEN_REPARSE_POINT`, a reparse point
  refused), checked on the handle, and listed. Each of its entries is classified with
  `fstatat` without following and counted: a regular file by its size rounded up to a
  multiple of 4 KiB, plus 4 KiB (TM §8.7, L41), any other entry as 4 KiB; each regular file whose name
  is a key file (§3.1) counts one key. Nothing in a store is opened, read, created or
  removed by the scan.
- **Entries that are not stores.** An entry of `<kind>` that is not such a directory (a
  link, a reparse point, a file, a special file, a name that is not `s_` and a valid id)
  counts nothing and is shown in the script manager as "Not a stored-values folder:
  data/script-store/scripts/<name>", the name as plain text (TM §8.2).
- **Caps (TM L92).** At most 1,024 entries are read from each `<kind>` listing and at most
  2,064 from each store directory. Past a store's cap that store counts as full (16 MiB
  and 1,024 keys, TM L40, L41); past a `<kind>` listing's cap the totals count as full
  until the next scan. Either way the totals stay conservative, and the script manager
  reports "Too many entries in data/script-store/scripts".
- Orphans count toward the totals; a write refused by a total names them (D17 §5.2).

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
   the `author`'s UTF-8 bytes, nothing else. The store must then carry a plain
   `local-only` marker when the effective set holds a local-only capability, or when the
   sweep removed a `local-only` entry that was not a plain file and the store was not
   wiped. A plain marker that the sweep kept (step 2) and the wipe did not remove is
   **kept as it is**; only when none is present (a new or wiped directory, or the sweep
   removed a non-plain one) is a plain marker created exclusively and written durably, so
   the exclusive create never meets an existing marker. The tags are written before the
   open completes, so before the top-level run and before any value file of the new
   apartment can be flushed.
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
  shows, ext windows, labels and overlays neither drawn nor hit-tested while it shows,
  and the developer console (EA1b §9) hidden and taking no input while it shows (D17
  §5.3), so no text a script prints can sit beside or over the dialog.
- **The pointer-only "Allow".** A press on "Allow" counts only when all of these hold: the
  press comes at least 500 ms after the dialog appeared; the pointer **entered** the
  button, as a pointer move from outside the button to inside it **observed after the
  500 ms delay ended**; the pointer stayed inside from that entry to the press; and the
  release follows on the button. A pointer that entered during the first 500 ms and rests
  there, or that was over the button when the dialog appeared, must leave and enter again.
  No key activates it.
- **Every control waits 500 ms** (D17 §5.3, TM L65). "Deny" and "Not now" also ignore a
  press during the first 500 ms after the dialog appeared; they need **no** pointer-entry
  rule (a refusal grants nothing), so a press on them counts from 500 ms on, wherever the
  pointer was before. The Escape key is not a control of the dialog: it refuses at any
  time (keys only refuse, TM L65), which can never grant anything.
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

**Record cap (TM L79).** A dialog whose Allow would add a record for a key that has no
`[[script]]` table in the document view (§8.5), while the document already holds 256
tables, is not shown: the load is refused with "256 scripts have saved permissions; remove
one in the script manager", and nothing is written. (The write job checks the cap again
against the document it reads, §8.5 write step 5.)

Any dialog also asks for the memory of step 3. An optional capability that is **denied**
is never in the asked set of the last three kinds; the user can change it in the manager
(§9.7). An undecided capability that is not offered (for example `world.read` while it is
unavailable on the backend) stays undecided and is asked when a later load offers it.

**Step 6: the record write and the outcome.**

- **No dialog.** If step 2 or 3 left something to update, or stored settings values must
  be dropped (§4.12), or the record the load decided under is an overlay entry of a
  writable document (§8.5 "Before the store is opened"), the record is written (§8.5)
  with the manifest's hash and
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
| Script line | name, version, then the fixed-style id and kind ("notes · installed", "hud · developer"); author; description (at most 8 lines). A developer script also shows the developer badge. At a load these come from the manifest that phase A read. A dialog started from the script manager (§9.7; no load) reads the folder's current `script.toml` alone, as the details page does (§3.5); it uses name, version and description only when that manifest is valid and its `author` and pinned capability lists equal the record's. Otherwise (missing, invalid, another author or other lists) the line shows only the id and kind and the record's `author` (§8.3), and the capability rows show their dialog texts without reasons |
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
- **Deny** and **Not now** ignore every press during the first 500 ms after the dialog
  appeared, like every control (D17 §5.3, TM L65), and have no pointer-entry rule
  (§9.1).

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

- **Preset and input.** The choice starts at the side the record grants (at "Neither"
  when it grants neither, and always at "Neither" at a first enable or a changed script,
  which have no earlier choices, §9.3 "Controls"); a switch started in the manager starts at the side the
  user asked for. It follows the boxes' input rule of §9.3 (pointer only, TM §8.25).
  "Exchange messages with the server" grants `session.server`; "Use local data" grants
  every local-only capability of the offered set; the side not taken is **denied**, and
  "Neither" denies both. The harness answers it only by a `reply` with `local_only`, or
  by "Neither" as its default (§11.4).

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
  and has **Reset**. In the frame the user chooses it, the record leaves the grant table
  (its "no record" enters the session overlay) and the script is ended if it is loaded
  (the ordinary unload, with the final flush) or loading (cancelled, §7.5). The store is
  then emptied on its I/O queue, behind that flush; when that has completed, the
  grant-store write that removes the record is submitted and a store-root scan queued.
  Until then a load of the script is refused. §8.7 "Reset and Remove, step by step" gives
  the frames. The next load is a first enable.

<a id="s9-7"></a>
### 9.7 The script manager's grant pages

The script manager (D17 §5.3, §7) shows, per (kind, id): the state (not enabled, loaded,
suspended, not loaded with the reason, orphan), the granted, denied and undecided
capabilities with their groups, memory, autoload, signature state (slice 4), store size,
and the settings page.

| User action | Effect |
|---|---|
| Uncheck a granted capability (revoke) | The capability moves from `granted` to `denied` (not to undecided, so §9.2 step 4 and step 5's "denied optional" rule apply to it). The grant table changes at once through the session overlay, the write follows, and a failed write keeps the revocation for the session (§8.5). During a load of that (kind, id), §7.5 applies. On a loaded script, required or optional, the script is **suspended in that frame** (TM §8.23; D17 §4; a re-read of the grant store that narrows a loaded script's record does the same, §8.5): its pending intents are discarded, its leases end (TM §8.17), nothing accrues, and no call runs until the user resumes it. A revoked **required** capability cannot be resumed: the script is unloaded (one close item, no `Unload` handler; D17 §2), and its next user-started load asks again (§9.2 step 4). After a revoked **optional** capability the user may resume the script; from then every native of that module raises `"not granted"`, `Ext.Has` answers false, and what the module made is gone (EA1a §2.9) |
| Check a denied or undecided capability (widen) | Opens the more-permissions dialog for it (§9.3, "Started from the script manager"). It asks only for capabilities of the record's pinned list, each with its dialog text and, when the folder's current manifest has the same pinned list, its reason. On a loaded script the new grant applies at the **next load** |
| Switch between a local-only capability and `session.server` | §9.4 |
| Change "Load automatically" | Written at once |
| Move in the list | Writes `order` |
| Reset | §9.6; the record leaves the grant table in that frame, a loaded script is unloaded and a load in progress cancelled (§7.5), and the record's write follows the store's deletion (§8.7 "Reset and Remove, step by step") |
| Remove (orphans) | §8.7; the same frames as Reset, then the store directory itself and the id's bindings are removed |
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
    new apartment opens it (§8.8 step 3). A full reload's **load** (phase A, with its
    memory reservation, §7.3) starts only after the old apartment's close item has run
    **and** its cap has left the 512 MiB sum at its final flush (TM L84, L24; EA1b §8),
    as a hot reload's new apartment waits for it. So §7.4's one-id rule (the old copy is
    no longer loaded) and the sum (with 16 × 32 MiB loaded, the load waits for that flush
    instead of being refused) never refuse it, and it never adds an apartment (TM L26);
  - a **hot** reload writes nothing to the grant store (EA1b §8.3 has no such write): the
    record keeps the hash of the last full load, which is harmless because a developer
    script's content change keeps every decision (§9.2 step 2); stored settings values
    that the new schema makes invalid are replaced by their defaults for the new
    apartment (EA1b §8.1, §4.12) and dropped by the next full load's record write;
    a **full** reload writes as §9.2 step 6 says;
  - the developer console's REPL binds only to developer apartments (D17 §10).
- **`/ext:reload` of an installed script** (developer mode on) is always a full reload:
  unload, phase A (started as the full reload above says), the pipeline of §9.2 (so a
  changed hash re-asks group A before new code runs), and a load without a hand-off
  table.
- **`/ext:reload` is a developer-mode command** (D17 §3, EA1b §8.1). With developer mode
  off, every `/ext:reload` line, whatever its id, is refused with the reserved-layer notice
  "Developer mode is off"; nothing is unloaded, read or loaded. With developer mode on, an
  `/ext:reload <id>` whose id names no **loaded** script (not loaded, unloaded after a
  suspension, only listed, or unknown) produces the notice "No loaded extended script
  '<id>'" (D17 §3; the id shown as plain text, TM §8.2) and starts no load. This is the
  one text for every "not loaded" case, because D17 §3 states it; the `/ext:` command
  line's (EA1a §6.2) and EA1b §8.2's reload notices use it too (O-12, closed). A reload
  never loads a script that was not running; the user loads it from the script manager. A
  loaded script that is **suspended** is not reloaded: the notice is "notes is suspended:
  resume it first" (a suspension during a reload cancels it, EA1b §8). The file watcher
  follows the same rules (EA1b §8.2: events for a script that is not loaded queue
  nothing).
- **Turning developer mode off** (TM §8.23 "Developer mode turned off"), in that frame:
  unloads every loaded developer script (the ordinary unload items; a suspended one its
  close item only); **cancels every developer load in progress** by §7.5's rows (a grant
  dialog showing or queued for it closes as "Not now" and writes nothing; store work
  already submitted completes on its queue and its result is discarded; in phase D the
  apartment's state gets one close item and the top level never runs; §7.5 "Other
  cancellations"); **refuses every waiting developer load** (a user-started load queued
  in `Starting`, a reload request not yet started) with "Developer mode is off"; and
  stops listing `data/scripts-dev/`. So no developer script finishes loading, and no
  developer load's dialog makes a decision, after developer mode is off (a decision made
  before, whose write is in flight, is written whole, as at a switch, §10.4).
  Their records stay. Turning it on again shows them; their next load follows §9.2 with
  developer mode on.
- **The user's unload of a loading copy** (the unload §7.4 offers when the other kind of
  an id is loading) cancels that load the same way (§7.5 "Other cancellations"), whatever
  its kind and whatever developer mode is.
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
| **Starting** | An admitted session has connected (start-up step 8: its class is known), and the UI is not yet ready, or this entry's read of the grant store (§8.5) or its store-root scan (§8.7) has not yet been applied. Both are submitted at **every** entry into `Starting`, read first (TM L92) | none | user-started loads wait (queued in order) | not run | none shown | as Off, with "starting" |
| **Open** | `Starting`, the UI is ready (start-up step 9), and this entry's grant-store read and store-root scan have been applied (an unreadable, damaged or newer document counts as read, §8.6) | loaded and loading ones | allowed (§7.3); autoload queued at **world entry** (the session's first entry into the world after its `Connect`, once per session; if that entry came while the state was still `Starting`, at the entry into `Open`) | run, under the per-frame rules of EA1a §2.6 and TM L5–L9 (cap and slices, L5–L7; heavy-frame gate, L8; floor frames, L9 and TM rule 7.4.1) | allowed | full |
| **Draining** | A backend switch is queued, or the session disconnects | existing ones are being unloaded | none start; loads in progress end as §10.4 says | 6a runs the unload and close items at the head of the ext queue, under the 6a slice (TM L6), the heavy-frame gate (TM L8) and its floor frames (TM rule 7.4.1, TM L9); **6b does not run**; 6c runs | the shown dialog closes as "Not now"; queued ones are dropped | read-only, "unloading" |

Transitions:

- `Off` → `Starting`: the new session is connected and its class is admitted.
- `Starting` → `Open`: the UI is ready, and this entry's grant-store read and store-root scan have
  been applied. Both are jobs of the grant-store queue, which never waits on another
  program (§8.5), so this transition cannot be held back by a FIFO, a device or a lock
  at the grant store's path. Waiting user-started loads start, in order.
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
gating, at least one item runs per ungated frame (TM L6). Under permanent gating, floor
frames run exactly one 6a item each and the timeout closes the rest, so a switch completes
within TM L11, 2 s + *a* × (250 ms + one frame) with *a* apartments:

| Slice | Apartments *a* (TM L26) | Without gating | Under permanent gating (TM L11) |
|---|---|---|---|
| 1a | at most 16 (no hot reload) | at most **32** ungated frames (16 `Unload` items and 16 close items) | about 6 s; at most **376** frames in deterministic mode |
| 1b and later | at most 17 (16 plus a hot reload's new apartment) | at most **34** ungated frames (17 + 17) | about 6.25 s; at most **392** frames in deterministic mode |

TM X85 and X87 test this. (This closes the former OPEN O-5.)

**Narrowings during a load.** A manager revocation, Reset, Remove, a local-only switch or a
narrowing re-read of the grant store that reaches a (kind, id) while its load is in
progress is not a gate transition. §7.5 states it with the same rows as the table above
(phase A; phase B before, at and past its dialog; phase C; phase D before the top-level
run; loaded), and phase D's delivery and the top-level item re-check the record, so no
load reaches its top-level run without a capability revoked after its decision (TM §8.23
"Loads in progress", X91). Developer mode turned off and the user's unload of a loading
copy cancel a load by the same rows (§7.5 "Other cancellations", TM X90).

<a id="s10-5"></a>
### 10.5 What the gate does not change

- Records, stores, logs and keymap bindings are untouched by any transition.
- Per-script suspension (watchdog, aborts, panic chord, revocation, a changed process
  locale) is not a gate state; it is defined in EA1a §7.11 and §2.9, TM §8.23 and D17
  §4–§5.
- An `/ext:` line in any state other than `Open` is never a chat line and is never sent to
  a backend. Outside `Open` no script takes lines (none is loaded in `Off` and `Starting`;
  in `Draining` 6b does not run). So:
  - `/ext:list`, `/ext:help [<id>]` and `/ext:<id>` with **no** command word open the
    script manager at the matching page, as in `Open` (D17 §3; EA1a §6.2: `/ext:<id>`
    alone opens the script's help page as `/ext:help <id>` does), read-only in the states
    where §10.3's table makes it so; no script gets anything;
  - `/ext:reload <id>` with developer mode off gives "Developer mode is off" (§9.8);
  - every other `/ext:` line (`/ext:reload <id>` with developer mode on, `/ext:<id>` **with**
    a command word) gives the notice "No loaded extended script '<id>'" (D17 §3, §9.8),
    the id shown as plain text (TM §8.2), rate-limited as every notice (TM L67).

---

<a id="s11"></a>
## 11. The script tool `tessera-scripttool`

<a id="s11-1"></a>
### 11.1 General

- An app (`apps/scripttool`) over `tessera-ext-host`. It needs no game install and no GPU,
  and it never reads `TESSERA_GAME_DIR`.
- **It never touches the user's own Tessera directories.** Every command that loads a
  script works in a fresh temporary **tool root** (a directory with `data/`, `logs/` and a
  config directory). `--root DIR` uses a given empty directory instead; a non-empty one is
  refused (exit status 2).
- **Removing the tool root (TM §8.8 "The harness's tool root").** A tool root that the tool
  made is removed at exit unless `--keep-root` is given. A user-given `--root DIR` is
  **never** removed or emptied: the tool leaves it as `--keep-root` would and prints its
  path. The removal is a **no-follow recursive removal through directory handles**: each
  entry is classified with `fstatat(…, AT_SYMLINK_NOFOLLOW)` relative to its directory's
  handle; a directory is opened relative to that handle with `O_RDONLY | O_DIRECTORY |
  O_NOFOLLOW | O_NONBLOCK`, checked on the handle, emptied the same way and then removed
  (`unlinkat` with `AT_REMOVEDIR`); every other entry (a symbolic link, a junction or other
  reparse point, a FIFO, a device node, a file) is removed as an entry (`unlinkat`; on
  Windows, deleting the reparse point opened with `FILE_FLAG_OPEN_REPARSE_POINT`), never
  opened and never followed. So X94's links and junctions are removed as links, their
  targets and the canaries outside the root are never touched, and nothing outside the
  root is removed. An entry that cannot be removed this way is left, and the tool reports
  it (exit status 3).
- **Script text reaching a terminal is escaped (design addition**, adopting D17 "Other
  improvements", script log). Every string that comes from a script, a manifest, a walk
  file, a grant store or a path is first turned into **escaped text**: a byte that is not
  part of valid UTF-8 (a POSIX file name need not be UTF-8) becomes `\xNN` (the byte's value in two
  upper-case hexadecimal digits; characters are always written as `\u{XXXX}`, so a byte
  escape and a character escape never mix up); C0 and C1 control characters (U+0000–U+001F except
  TAB, U+007F–U+009F), U+2028, U+2029, and every character §4.4 refuses (Cf with every
  bidirectional control, Zl, Zp, Cs, Co, Cn, noncharacters, variation selectors) become
  `\u{XXXX}`; a backslash becomes `\\`. The text form prints the escaped text, and every
  line the tool prints starts with its own prefix, so no output can carry terminal escape
  sequences, reorder a line or forge one. (The script **log file** has its own format, TM
  §8.1.)
- **Exit status:** 0 success; 1 findings or failures (an error finding of `check`, a failed
  test, a failed escape case); 2 usage or input error (an unreadable path, an invalid walk
  file, a bad option); 3 internal error.
- **`--json`** (every command) writes one JSON object per line instead of text, UTF-8, no
  white space outside strings, keys in the order the table below lists them. A key that
  does not apply to an object is **omitted**, never `null`. A string value is the same
  **escaped text** as the text form prints (above), so a file name that is not UTF-8 is
  carried as its `\xNN` escapes. It is written as a JSON string that is **pure printable
  ASCII**: `"` and `\` as `\"` and `\\`, and every character outside U+0020–U+007E as
  `\uXXXX` (lower-case hexadecimal; a pair of surrogate escapes above U+FFFF), not left to
  an encoder's defaults (RFC 8259 and common encoders escape only U+0000–U+001F, which
  would pass C1 controls, U+2028, U+2029 and bidirectional controls through). So every
  `--json` line is safe for a terminal on its own, and a consumer that decodes it gets the
  escaped text, never a raw control character.

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
| `test` | 1a | `tessera-scripttool test [--walk FILE] [--seed N] [--latency K] [--backend offline\|other\|tessera-protocol] [--deny CAP]... [--developer-mode] [--no-start] [--grants FILE] [--trace FILE] [--expect-trace FILE] [--worker-jitter SEED] [--keymap-ext FILE] [--root DIR] [--keep-root] [--json] (FOLDER \| --installed FOLDER \| --developer FOLDER)...`; a bare FOLDER means `--installed FOLDER` (§11.4) |
| `escape` | 1a (cases added per slice) | **(Design addition.)** `tessera-scripttool escape [--list] [--case XN]... [--seed N] [--json]`: the runner of TM's escape tests (§11.8). D17 places the escape tests "in `scripttool test`"; this command runs them with `test`'s machinery and harness, so it adds a name, not a mechanism |
| `modules` | 1a | **(Design addition.)** `tessera-scripttool modules [--json]`: the build's registry: modules with their majors, minors, capability, group and deprecation, and the capability catalogue of §6.1 for the slices the build serves |
| `init` | 1b | `tessera-scripttool init ID [--dir PARENT]`: writes `PARENT/ID/script.toml` and `PARENT/ID/main.lua` (an original, CC0 skeleton) that pass `check`; refuses an existing folder |
| `pack` | 1b | `tessera-scripttool pack FOLDER [-o FILE.zip]`: a zip of exactly the snapshot's files after `check` passes, with the content guard of D17 §10 (refuses `.dat`/`.datx` files and files starting with the archive container's header as `tessera-dat` recognises it; warns that extracted game assets must not be packed, `CLAUDE.md`) |
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
| M03 | error | A required key missing, a value of the wrong type (a float where an integer is documented included), a datetime value anywhere, a non-finite number (`inf`, `nan`) anywhere, nesting deeper than §4.1 allows, or more than 1,024 keys in the document (§4.1, TM L37) |
| M04 | error | `manifest` below 1 or newer than this build reads (§12.1); `api` below 1 or its major not served (§12.2) |
| M05 | error | `id` grammar, reserved id, or `id` different from the folder name |
| M06 | error | `version` not Semantic Versioning 2.0.0 within the caps; `license` not printable ASCII (0x20–0x7E) or over 64 bytes (§4.2, TM L87) |
| M07 | error | A display string breaks §4.4 (the message names the field, the rule and the code-point position, never the string) |
| M08 | error | `entry` invalid, not a `.lua` file, or not in the snapshot |
| M09 | error | An unknown capability; a capability both required and optional; more than 16 |
| M10 | error | `session.server` and a local-only capability both required |
| M11 | error | A module unknown, its major not served, its minor too new; a module listed twice; more than 32 |
| M12 | error | A module's capability missing from the manifest, or required module with an optional capability |
| M13 | error | `memory_mib` out of range |
| M14 | error | `[[contexts]]`: more than 2, name grammar, name too long for the id, duplicate; contexts declared without `input` in either capability table (§4.10) |
| M15 | error | `[[actions]]`: more than 64, name grammar or length, duplicate, `kind`, missing or unknown `context`, no context declared; actions declared without `input` in either capability table (§4.10) |
| M16 | error | `default` invalid: not one stroke, an axis or analog entry, `on` not allowed there, `value` out of range |
| M17 | error | Two actions of one context suggest overlapping strokes: equal after KM §5.1, or one press can complete both (a shared button, a chord and one of its buttons, modifier conditions one press meets together, subset-modifier matches for axes; §4.11, TM §8.16, L61) |
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
  §9.2 or §9.7 raises (at a load, or by a `manager` `widen` or `local-only` switch) is
  **answered by the harness** when it is shown: in the frame it is raised, or, while
  another dialog shows, in the frame it reaches the front of §9.1's queue. The answer is
  chosen in that frame by this policy, and applied in that frame unless the reply taken
  has a `delay` (§11.5), which keeps the dialog showing for that many frames first (so a
  narrowing, a switch or developer mode turned off can meet a dialog that shows, §7.5,
  §10.4; TM X90, X91):
  1. If the walk has queued a `reply` for that script (§11.5) that has not been used,
     the oldest such reply is taken. Its `kind` must equal the dialog's kind, and its
     `local_only` key must be present exactly when the dialog shows a local-only choice
     (§9.4) and `uncheck` must name no capability of that choice; otherwise the run
     fails with "dialog kind <kind> raised, reply expects <kind>" (or "reply does not fit
     the local-only choice") and the dialog is answered as Escape.
  2. Otherwise, a **first enable** dialog gets the **default answer**: Allow, with every
     optional capability named by `--deny CAP` (repeatable) unchecked, every other
     optional box checked, and "Load automatically" as the manifest's `autoload`; if a
     required capability is named by `--deny`, the answer is Deny (the load is refused,
     which the walk can expect). A **local-only choice** (§9.4, slice 3) is not a box:
     the default answer takes **"Neither"** for it, so it grants neither side and denies
     every capability of the choice (the fail-closed answer; a test that wants a side
     queues a `reply` with `local_only`).
  3. Otherwise (any other kind, with no queued reply), the dialog is answered as
     **Escape** (Deny or "Not now" by kind, §9.3), and the run **fails** unless an
     `[[expect]] dialog` (§11.5) names that dialog. So no later dialog is ever allowed
     silently.

  Each answer is applied as §9.3's outcome table says (Allow writes the record through
  the grant-store queue, with its completion seen `--latency` frames later) and appears in
  the trace as a `dialog` event (§11.6). The records exist only in the tool root's grant
  store, and the tool never reads or writes the user's.
- **Seeding the grant store.** `--grants FILE` copies FILE's bytes, unchanged, to the tool
  root's `ext-grants.toml` before start-up's read (at most 4 MiB + 1 bytes are copied, the
  bound of TM L79's read, which is enough for §8.6's "over 4 MiB" case). Any bytes are
  accepted: a hand-edited, damaged, duplicated or newer-`version` document is read exactly
  as the client reads it (§8.4, §8.6). The walk's `grants` event (§11.5) replaces the
  document mid-run and triggers the re-read of §8.5, as the script manager's opening would.
  Without `--grants`, the tool root starts with no grant store. (A grant store that is a
  FIFO or a link to a device, TM X94, is placed by `escape`'s own hook, not by
  `--grants`.)
- **Input files.** The `--walk`, `--grants` and `--keymap-ext` files, and the files and
  folders the walk names (`grants`, `reload.from`), are the user's own and may be reached
  through links, but each is opened **non-blocking** and checked on the handle to be a
  regular file (a directory for `reload.from`, which is then read through §3.4), and a
  file is read at most to its cap + 1 bytes (the walk file 1 MiB, TM L86; a grant store
  4 MiB, TM L79; an ext keymap 128 KiB, TM L47). A FIFO, a device, or a missing or
  unreadable file is an input error (exit status 2) before the run starts, or, for a walk
  event, a run failure in that frame; nothing waits on it. Only `--grants` and `grants`
  accept an oversized file (the copy stops at the cap + 1 bytes, as above); an oversized
  walk file is an input error.
- **Folders and their kinds.** Each folder is given as `--installed FOLDER` (kind
  `scripts`) or `--developer FOLDER` (kind `scripts-dev`); a bare FOLDER means
  `--installed FOLDER`. The options repeat and mix; their sequence on the command line is
  the **command-line order**. Each folder is read where it is, through §3.4's rules: its
  parent directory takes the place of `data/scripts` or `data/scripts-dev` in step 1, and
  the folder is opened relative to it in step 2. Two folders of **one** kind with the same
  id are a usage error (exit status 2); one id in **both** kinds is allowed (D17 exit
  criterion 5's second-copy case). The host's listings of §3.5 hold exactly the given
  folders of each kind (the developer ones only while developer mode is on), so a record
  whose (kind, id) has no given folder is an orphan by §8.7's rule. Records, stores and
  logs go to the tool root.
- **Developer mode** is off unless `--developer-mode` is given, whatever the folders'
  kinds, and the walk's `developer_mode` event (§11.5) turns it on or off during the run,
  with exactly the effects of §9.8: turning it off unloads every loaded developer script,
  cancels every developer load in progress and refuses every waiting one (§7.5 "Other
  cancellations"), and stops listing developer folders; turning it on lists them. In the
  trace, the frame of a `developer_mode = false` event shows, in this order: the
  `devmode` `off` event; for each developer load in progress, in the script-list order,
  its `dialog` event (closed as "Not now", `r` = `closed:devmode`) if one was showing,
  and its `load` event with `r` = `cancelled` (or, for a load waiting on its record write
  or store probe, or in phase C, that `load` event in the frame the job's completion is
  seen; for a phase-D load, its `close` item when it runs); then each loaded developer
  script's unload items as they run; and for each waiting developer load a `load` event
  with `r` = `refused:Developer mode is off`. **No settings file** is
  written: developer mode and every other setting of the run (the panic chord at its
  default included) are held in memory, so the tool root never holds a settings file or
  its lock file.
- **Start of the run and the first loads.** In frame 0 the harness connects the session
  (`Off` → `Starting`, §10.3), the UI is ready, and the session's read of the grant store
  (the start-up read) and then the store-root scan (§8.7) are submitted on the
  grant-store queue; they complete at frame *k* (`--latency`, the worker rule above), and
  the gate becomes `Open` in that frame (with `--backend other` or `tessera-protocol` it
  stays `Off`). Every later session (after a `switch` to an admitted class) submits the
  same two jobs at its own entry into `Starting` and opens when both have completed
  (§8.5, §8.7; TM L92). The harness's
  player is in the world from the connection, so **world entry** (§10.3) is the first
  frame of each session in which its gate is `Open`. In that frame, in this order:
  1. **autoload**, as §7.3 says: the grant table's records with `autoload = true` whose
     folder is listed, in the script-list order;
  2. in the run's **first** session only, and unless `--no-start` is given, a
     **user-started load** of each folder in command-line order. A load of a (kind, id)
     that autoload has already started is ignored, a load of an id whose other kind is
     loading or loaded is refused (§7.4), and a load of a developer folder while developer
     mode is off is refused with "Developer mode is off".

  Every later load comes from a walk event (`manager` `load`, `reload`) or from autoload
  at the world entry of a later session (after a `switch`). So D17 exit criterion 5's
  cases can be written: the second copy (`--installed a/notes --developer b/notes
  --developer-mode`: the developer load is refused while the installed one is loading or
  loaded), a reload of an installed script with developer mode on (`--developer-mode` and
  a `reload` event), two `autoload = true` records of one id in both kinds at world entry
  (`--grants`, TM X90), and "turning developer mode off unloads developer scripts" (a
  `developer_mode = false` event and an `[[expect]] loaded`).
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
- `--keymap-ext FILE` **copies** FILE's bytes (at most its cap + 1, TM L47) to
  `keymap-ext.toml` in the tool root's config directory, which is the directory of the
  run's configured keymap path (§3.1), before start-up; the run reads that copy with KM
  §6.3's strict reader, and every save or binding removal of the run (§8.7) writes only
  the copy, never FILE. Without it, only suggested defaults are bound.

<a id="s11-5"></a>
### 11.5 The walk file

TOML, at most 1 MiB, strict (unknown keys are errors, exit status 2), with a fuzz target
(§11.7). Every count, size and range of the walk file below is TM L86. Without `--walk`, the run uses an empty walk of 600 frames on the `flat` world
and the default screen.

**Frame numbers.** Every frame number in the walk file and the trace is **0-based**: frame
0 is the first frame of the run, and a run of `frames = 900` has frames 0 to 899. Every
frame number must be below `frames`. An event "at frame *n*" is delivered in that frame
the way the client would deliver it (an input in step 1 of frame *n*, and so a script's
handler in 6b of frame *n*). Several `[[input]]` entries of one frame are applied in
the walk file's order, so a `manager` `revoke` and a `manager` `widen` of one capability
in one frame (TM X91) run in that order. Ranges: `[[frame]] from` and `to` are both inclusive; a
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
start = { x = 96.0, y = 96.0, yaw = 1.5707963 }   # facing +Y (north)

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
| `[world]` | `preset`, `size`, `start`, `region` | **Region (design addition):** the synthetic world has world data, so its session reports a region number in 1–255, never 0 (EA1b §3.1); it is `region` (integer 1–255, default **1**, an argument-validity domain, TM §0.1 rule 6), the region of the player and of every `[[entity]]` and flood entity until a `region` input (below) changes it. `flat`: terrain height 0 m everywhere; `slope`: height rises 0.1 m per metre along +x from the west edge. `size` landblocks per side (odd, 1–9, TM L86) around a fixed centre landblock that the harness prints. `start` is the player's position `x`, `y` in the region frame of ARCH §9.1 (metres) and the direction the character faces as `yaw`: radians in EA1a §2.10's form (KM §9.1: counter-clockwise from +X seen from above, 0 facing +X, π/2 facing +Y = north; any finite value, read modulo 2π, an argument-validity domain, TM §0.1 rule 6). There is no `heading` key: as EA1a §2.10 says, no input takes a compass heading, and `World.Here().heading` is derived from this yaw (EA1a invariant 40). Default `yaw` 0 |
| `[[entity]]` | `name` (at most 48 cp, §4.4 rules), `position` (region frame), `spawn`, `despawn` (frames; `despawn` optional) | Synthetic entities that the `Entity` module sees; at most 4,096 (TM L86) |
| `[[flood]]` | `from`, `to` (frames, inclusive), `spawn` (entities per frame), `life` (frames, default 1), `radius` (metres, default 50) | **The record-flood hook** **(design addition**, a test hook of the tool only, never in the client; TM §7.5, X19; EA1a invariants 36 and 37). In each frame from `from` to `to`, `spawn` synthetic **flood entities** are spawned, and each is despawned `life` frames after its spawn frame (also after `to`; one whose despawn frame is not below `frames` lives to the end of the run). Their spawn and despawn records enter the record list of the frame's steps 2–5 exactly as a walk entity's records would, so TM rule 7.5.1's cap of 4,096 recorded records per frame and its overflow count apply to them. Within a frame the records come in this order: the `[[entity]]` records (in file order), then flood despawns (in spawn order), then flood spawns (in serial order). Flood entity *i* (a run-wide serial from 0) has the same synthetic kind as an `[[entity]]`, the name `flood-<i>`, and a position drawn from the run's seed, uniform in the disc of `radius` around `[world] start`, at terrain height, so two runs with the same seed flood identically. Flood entities do not count toward L86's 4,096 `[[entity]]` entries. Bounds (TM L86, registered in TM revision 6; O-13 closed): at most 64 `[[flood]]` entries; `spawn` 1–16,384 flood entities per frame; `life` 1–600 frames; at most 65,536 flood entities alive in any frame, checked when the walk file is read (a walk that breaks one is invalid, exit status 2). `radius` 1–500 m is an argument-validity domain (TM §0.1 rule 6) and stays here. Examples: TM X19's 10,000-entity integration per frame for 10 s is `{ from = 60, to = 659, spawn = 10000 }` (10,000 spawns and 10,000 despawns per frame); EA1a invariant 36's single frame is `{ from = 60, to = 60, spawn = 10000, life = 600 }`; invariant 37's 1,024 events per frame are `{ from = 60, to = 659, spawn = 512 }` |
| `[[input]]` | `frame` and exactly one of: `press` / `release` (one stroke, KM §5.1, resolved by the mapper exactly as in the client, so ext bindings resolve only after every built-in context, D17 §5.5); `text` (typed text, delivered to the focused text box if any, else dropped); `command` (a line as if typed in the chat input and submitted; `/ext:` lines go to the ext host, anything else is ignored); `pointer` + `click` (move the pointer to UI-space coordinates and click a mouse button there); `hold` + `until` (game-band actions held, for movement); `focus` (`"lost"` or `"gained"`: the game window loses or regains keyboard focus, TM §8.17); `region` (an integer 1–255 other than the current one: **(design addition)** the player changes region in this frame at the same position and yaw: the harness's sandbox applies a location report with the new region in step 2 of that frame, so a lease ends with `"region"` as EA1b §4.5 says, which lets that test run under `test` instead of only in the client; the synthetic world serves the same preset, size and entities under every region number, so only the number changes); `switch` (`"offline"`, `"other"`, `"tessera-protocol"` or `"disconnect"`: a backend switch or session end is queued, §10.3); `reload` (`{ script = "<script>", from = "<path>" }`: from this frame the script's folder is the one at `from`, a walk path whose last segment is the id, read through §3.4's rules, and a reload is requested as `/ext:reload <id>` would, EA1b §8); `developer_mode` (`true` or `false`: the user turns developer mode on or off in Tessera's settings page, with §9.8's effects, §11.4); `manager` (`{ script = "<script>", action = "revoke" \| "widen" \| "resume" \| "unload" \| "load" \| "reset" \| "remove" \| "autoload-on" \| "autoload-off" \| "local-only" \| "open", capability = "<name>", side = "server" \| "local" }`: the script manager's action of §9.7, `capability` only with `revoke` and `widen`, `side` only with `local-only`; `reset` and `remove` follow §8.7 "Reset and Remove, step by step" (`remove` only for an orphan, §8.7; otherwise the run fails in that frame); `unload` of a script whose load is in progress cancels that load (§7.5 "Other cancellations"); `local-only` (slice 3) is §9.4's switch to the named side (`server`: `session.server`; `local`: the record's local-only capabilities), started in the manager: the script is unloaded first if loaded, then the switch dialog (kind `more`, its asked set the new side's capabilities, with the erase line of §9.3 when it moves to `server`) is raised and answered as §11.4 says; a script whose record offers no such switch fails the run in that frame; `open` (no `script`) is the user opening the script manager: it queues the re-read of §8.5 without changing the file. **No other `manager` action re-reads**: the harness models the script manager as open from the run's start, so its actions apply to the grant table as it stands, and a walk that needs the re-read of an opening writes an `open` (or a `grants` event) first; a `load`, `widen` or `local-only` that raises a dialog has it answered as §11.4 says); `reply` (`{ script = "<script>", kind = "first-enable" \| "first-enable-again" \| "changed" \| "update" \| "more" \| "memory", choice = "allow" \| "deny" \| "not-now" \| "escape", uncheck = ["<capability>", …], autoload = <boolean>, local_only = "server" \| "local" \| "neither", delay = <frames> }`: queues, from this frame, the answer to the next dialog of that script, §11.4; `uncheck` names optional capabilities left unchecked (default none; never one of the local-only choice), `autoload` the box (default: as §9.3 presets it), `local_only` the side taken in a dialog that shows §9.4's choice (slice 3; required exactly for such a dialog, §11.4 rule 1), `delay` the number of frames the dialog stays showing before the answer is applied (0, the default, to 600, an argument-validity domain, TM §0.1 rule 6: it adds no work or memory; at most one dialog shows at a time, so later dialogs wait behind it, §9.1); `deny` is valid for the first three kinds, `not-now` for the last three, `allow` and `escape` for all); `grants` (`"<path>"`: a walk path naming a file whose bytes (at most 4 MiB + 1, as `--grants` copies) replace the tool root's `ext-grants.toml` in this frame, after which the re-read of §8.5 is queued, as when the script manager opens) | At most 65,536 entries (TM L86) |
| `[[frame]]` | `from`, `to`, `elapsed_ms` | **Gate simulation:** in those frames, the harness reports `elapsed_ms` as the time since the frame's work started when step 7 is reached, so the heavy-frame gate (22 ms, TM L8) is evaluated on these values instead of being off, and the floor frames of TM rule 7.4.1 follow the 15-frame condition. Deterministic, because it never reads a clock. `elapsed_ms` above 22 for every frame is TM rule 7.4.6's "every frame is heavy" hook |
| `[[expect]]` | `frame` or `at = "end"`, and exactly one of: `log` (`script`, `line`: a script-log record whose text equals `line`, logged in that frame); `log_contains` (`script`, `text`); `error` (`script`, `contains`: an aborted call whose message contains the text, in that frame); `store` (`script`, `key`, `value`; only with `at = "end"`, read after the final flush of the unload at the end of the run: the `Store` value equals the TOML value, compared as the bounded value model, a TOML array being a table with keys 1..n); `loaded` (`script`, `value` boolean); `refused` (`script`, `reason`: the load was refused with that reason text); `suspended` (`script`); `answer` (`script`, `query`, `value`: a query's answer recorded in the trace); `gate` (`state`: the gate state at the end of that frame); `dialog` (`script`, `kind`, and optionally `asked`, an array of capability names, and `choice`: a dialog of that kind was raised for that script in that frame, with exactly that asked set and answered with that choice; with `at = "end"`, at any frame of the run); `granted` (`script`, `value`, an array of capability names: the grant table's `granted` list of that (kind, id) at the end of that frame, `[]` when it has no valid record); `grants_file` (`unchanged = true`, only with `at = "end"`: the tool root's `ext-grants.toml` holds exactly the bytes it was seeded with by `--grants` or the last `grants` event) | At most 4,096 entries (TM L86) |

**Naming a script.** Wherever the walk names a `script` (events and expectations), the
value is either an id, when exactly one given folder has that id, or `"<kind>/<id>"` with
kind `scripts` or `scripts-dev` (the trace's form, §11.6). An id that folders of both
kinds share must be written with its kind; a bare id that names two folders, or an id that
names no given folder, is an invalid walk file (exit status 2).

**Walk paths.** A path in the walk file (`reload.from`, `grants`) is relative to the walk
file's directory: one or more segments joined by `/`, each following §3.3's segment rule
(so no empty segment, no `.` or `..`), with no leading `/`, no `\` and no drive prefix. So
a walk path is never absolute and never climbs out of the walk file's directory by name.
It is opened through §11.4's input-file rule (non-blocking, its kind checked on the
handle, bounded), relative to a handle of the walk file's directory.

OPEN O-6 covers further presets (water, cells).

<a id="s11-6"></a>
### 11.6 The trace

`--trace FILE` writes the run's **trace**: one JSON object per line, in the order the
events happened. It contains no wall-clock values, no addresses and no absolute paths, so
two runs with the same scripts, walk file, seed and **options** produce byte-identical
traces (D17 §8, M5.5 exit criterion 7). "Options" means every option of §11.2 that changes
what runs: `--latency`, `--backend`, `--deny`, `--developer-mode`, `--no-start`, the
folders with their kinds and command-line order, and the bytes of `--grants` and
`--keymap-ext`. `--trace`, `--expect-trace`, `--worker-jitter`, `--root`, `--keep-root`
and `--json` do not change the trace. `--expect-trace FILE` compares the trace byte for byte with a
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
| `worker` | `f`, `ev`, `s`, `what`, `r` | the job: `snapshot`, `store-open`, `store-probe`, `store-flush`, `store-read`, `store-wipe`, `grant-write`, `grant-read`, `store-scan`, `keymap-write`, `image-decode`; `s` omitted for `grant-read` and `store-scan` | `submitted`, then `ok` or an error class at completion |
| `query` | `f`, `ev`, `s`, `what`, `u`, `a` | the query's name (for example `Entity.QueryRadius`) | — |
| `limit` | `f`, `ev`, `s`, `what`, `r` | the TM limit reached, as its L-number (`L27`); `s` omitted for a host-wide limit | the answer given: `busy`, `limit`, `full`, `dropped`, `refused` |
| `dialog` | `f`, `ev`, `s`, `what`, `r`, `c` | the dialog's kind: `first-enable`, `first-enable-again`, `changed`, `update`, `more`, `memory` (§9.2 step 5, §9.7) | the answer and its source: `allow:reply`, `allow:default`, `deny:reply`, `deny:default`, `not-now:reply`, `escape:reply`, `escape:unanswered`, `closed:gate` (§10.4), `closed:narrowed` (a narrowing, §7.5), `closed:devmode` and `closed:unload` (§7.5 "Other cancellations"). The event is in the frame the dialog was answered or closed: the frame it was raised, unless a `reply` with `delay` kept it showing (§11.4, §11.5). With a local-only choice (§9.4) an Allow's `r` also names the side taken: `allow:reply:server`, `allow:default:neither` and so on (`server`, `local` or `neither`). `c` is the asked set (§9.2 step 5) |
| `devmode` | `f`, `ev`, `what` | `on` or `off`: developer mode changed (`--developer-mode` gives an `on` event in frame 0; a `developer_mode` walk event, §11.5) | — |
| `grants` | `f`, `ev`, `s`, `what`, `c` | why the grant table's entry of that (kind, id) changed: `allow` (a dialog's Allow, a widen's and a local-only switch's included), `update` (§9.2 step 6's write without a dialog, dropped settings values included), `revoke` (§9.7), `autoload` (the "Load automatically" choice), `settings` (a setting value changed in the manager), `reset`, `remove` (the "no record" entering the overlay, §8.7 step 1), `rollback` (a failed load write taken back, which restores the overlay entry it superseded or leaves none, §8.5 "A failed write"; no event when a newer decision stays current), `reread` (a read result or a write's completion changed the entry: a hand edit or another process's change, a narrowing of an overlay entry by the document included, §8.5 step 2); `order` ("Move in the list"; `s` and `c` omitted, one event). One event per changed entry, in byte order of `s`, in the frame the change is applied. A change of the document view that leaves the grant table's entry as it was (an overlay entry still decides; a write's completion for its carried keys; a newer or unusable read; a failed write whose decision stays) emits **no** event; the `worker` event shows the job. `c` is the entry's `granted` list after the change (`[]` when it has no record) | — |

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
  `reply`, `grants`, `developer_mode`), the record-flood hook `[[flood]]` (§11.5),
  per-folder kinds, `--developer-mode`, `--grants`, `--latency` and the `dialog`,
  `granted` and `grants_file` expectations for the situations TM's procedures name
  (X19's flood of 10,000 entities per frame, `[[flood]]` within TM L86; X62's seeded
  documents and, from slice 3, its local-only switch through `manager` `local-only` and a
  `reply` with `local_only`; X90's dialogs, its second copy of one id in both kinds and
  its two `autoload = true` records of one id at world entry, §11.4, and developer mode
  turned off by a `developer_mode = false` event while one developer load shows its
  dialog (a `reply` with `delay`), one is in phase C and one waits in `Starting`, plus a
  `manager` `unload` of a loading copy, each expected `cancelled` or refused with
  "Developer mode is off" and never `loaded`; X92's dialogs; X91's re-read (`manager`
  `open` or `grants`), failed-write revocation, newer document, a document damaged as a
  whole re-read and then replaced by a write, narrowing during a load, a `manager`
  `revoke` and `widen` of one optional grant in one frame under `--latency 2` with an
  `[[expect]] granted` that lists it after both writes, a `manager` `reset` followed,
  once the deletion has completed, by a first-enable `load` answered Allow before the
  Reset's write completes, expected neither cancelled nor suspended, and the **rollback
  cases** of §8.5 "A failed write", with the hook that makes grant-store writes fail: a
  `manager` `revoke` of optional `b` whose write fails, then a `reload` with a changed
  hash whose update write fails, with `[[expect]] granted` not listing `b` after both,
  and, once writes succeed again, a later write after whose completion (the overlay
  chain then empty, so the grant table is the written document's) `[[expect]] granted`
  still does not list `b`; and a `manager` `reset` whose write fails, then a
  first-enable Allow whose write fails, with `[[expect]] granted` `[]` after it and the
  next load a first enable (`[[expect]] dialog` of kind `first-enable`)). In the last two the harness also **holds the earlier
  grant-store write in flight** (a test hook) until the later decision has been applied,
  because under one uniform `--latency` the manager-started dialog's manifest read (§3.5)
  or the new load's phase A and store probe would otherwise complete in the same frame as
  that write, and the order of the two would not be the one the case needs. X91's re-read
  whose open fails is its U part (`tessera-ext-host`), reported `ELSEWHERE`.
  Test hooks that no walk event covers (a grant-store write held in flight, a file that
  grows during the walk, an entry swapped for a link between a listing and its open, a
  store flush held in flight, a kill during a flush or a grant write, a grant-store write
  made to fail, a lock held by another process, the links, FIFOs, device links and
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
    `.ext-grants.toml.lock` (persistent, §8.5); its backups, exactly
    `ext-grants.toml.corrupt-<12 digits>.bak` or `ext-grants.toml.corrupt-<12 digits>-<n>.bak`
    with `<n>` 1–99 without a leading zero (§8.6); `keymap-ext.toml` and its lock
    file `.keymap-ext.toml.lock` (§8.7); and, only in a case whose procedure kills the
    host during a save, the temporary files `.ext-grants.toml.tmp-*` and
    `.keymap-ext.toml.tmp-*`. There is no settings file and no settings lock file: the
    harness holds developer mode and every other setting in memory (§11.4), so a
    developer case leaves none, and one that did would fail;
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
    (§8.5). This includes a load that decides under an overlay entry whose write is in
    flight or failed earlier: it writes its record again, even with nothing to update,
    and is cancelled if that write fails (§8.5 "Before the store is opened", §9.2
    step 6).
16. At most one script per id is loaded **or loading** across both kinds: a load is refused
    while the same id of the other kind holds a reservation (§7.4; TM L26, X90).
17. Ext apartments exist only in `Open` and `Draining` (§10.3), and only for admitted
    classes (§10.2; TM invariant 22).
18. In `Draining` no 6b call runs and no load starts; `Connect` is submitted only after the
    last close item; an apartment whose `Unload` handler has not started when the switch
    timeout passes (TM L10) is closed without it; `Draining` ends within 32 ungated frames
    in slice 1a (16 apartments) and 34 from slice 1b (17) without gating, and within TM
    L11 under permanent gating (slice 1a: at most 376 deterministic frames; from slice 1b:
    at most 392) (§10.3, §10.4; TM invariant 20).
19. Every load in progress at a switch ends as §10.4 says; no apartment of the old session
    survives into the new one, and no store work, store probe or grant-record write is
    left half-done (TM §8.21, X87).
20. Two `test` runs with the same scripts, walk file, seed and options (§11.6) produce
    byte-identical traces, whatever the worker timing.
21. `scripttool` never writes or removes anything outside its tool root (its removal never
    follows a link, and a user-given `--root` is never removed), except the output files
    and folders the user names on its command line: `test --trace FILE`, `pack -o
    FILE.zip`, `dump-api -o FILE` (and `--ext-dump-api -o FILE`), and the new folder
    `PARENT/ID/` of `init --dir PARENT`, which it creates and never removes. It never reads
    `TESSERA_GAME_DIR`, and never prints, in text or `--json` form, an unescaped control,
    bidirectional or other refused character, or a raw byte of a non-UTF-8 name, from
    script-supplied text or a path (§11.1).
22. No two suggested defaults of one script in one context overlap: one press can never
    complete two of them, so it starts at most one hold from one script's suggested
    defaults (§4.11; TM §8.16, L61, X41).
23. A capability name never allows more than it did when a record granted it (§12.7).
24. The grant store is read and written only on the grant-store queue, never on the main
    thread (§8.5).
25. A revocation suspends the script in the frame of the revocation (§9.7; TM invariant
    23), whether it comes from the script manager or from a re-read of an **understood**
    grant store that removes, invalidates or narrows the script's record (§8.5; TM §8.23,
    X91). A revocation of this session is never undone by a re-read, even when its write
    failed (the session overlay, §8.5).
26. Nothing widens what a running apartment may do: natives and intent applies check the
    apartment's effective set, never the grant table, and a widened record (by the
    manager or by a re-read) is used only from the script's next load (§7.1, §8.5; TM
    §8.23).
27. No grant-store or `keymap-ext.toml` write leaves any file other than the document, its
    persistent lock file and its backups, except a temporary file after a crash (§8.5,
    §8.7, §11.8).
28. A narrowing (a manager revocation, Reset, Remove, a local-only switch, a narrowing
    re-read) that reaches a load past its grant decision cancels it: no load reaches its
    top-level run without a capability revoked after its decision (§7.5; TM §8.23, X91).
29. A read of a grant store that is not understood (a newer `version`, a document damaged
    as a whole, a failed open, kind check or read) narrows, widens and suspends nothing,
    and the previous document view stays the base; a write that replaces a damaged
    document writes every record of the grant table, so it narrows nothing either (§8.5,
    §8.6; TM §8.23, invariant 35).
30. No open of the snapshot walk, of a store or its entries, of a log file, of the grant
    store or `keymap-ext.toml`, and no step of the store-root scan, waits on a FIFO or
    opens a device node; and the grant-store queue never waits on another program's lock
    for more than 1 s (TM L94) (§3.4, §3.6, §8.5, §8.7; TM §8.8, X54, X94).
31. No store is deleted under a live apartment: Remove and Reset end a loaded or loading
    script first, and the deletion runs after its final flush (§8.7, §9.6).
32. A written grant store never holds more than 256 `[[script]]` tables or 256 `order`
    entries, and an invalid table of a document that is not damaged as a whole leaves it
    only through a write of its own key, after a bounded backup (§8.5, §8.6; TM L79).
33. The completion of a grant-store write never narrows a key it carried: per key, the
    newest decision of the session wins over the completion of every earlier write, so a
    widen or a first-enable Allow made while an older revocation's, Reset's or Remove's
    write is in flight stays granted, and no load or apartment that relies on it is
    cancelled or suspended by that completion (§8.5, §7.5; TM §8.23, invariant 35, X91).
    And a failed load write that is taken back restores the overlay entry it superseded
    (or none): it never undoes an earlier session decision whose own write failed, so a
    revocation, or Reset's or Remove's "no record", stays in force and is never written
    back as a grant (§8.5 "A failed write"; TM §8.23, X91).
34. Reset and Remove take the record out of the grant table in the frame the user chooses
    them; their grant-store write is submitted only after the store's deletion has
    completed, no load of that (kind, id) starts in between, and a failed deletion
    removes nothing and keeps the "no record" for the session (§8.7).
35. No developer script finishes loading while developer mode is off: turning it off
    unloads every loaded developer script, cancels every developer load in progress
    (its dialog closes as "Not now" and writes nothing) and refuses every waiting one;
    and a load the user unloads while it loads never reaches its top level (§7.5, §9.8;
    TM §8.23, X90).

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
`world.read` (all available offline), and no record: step 1a. Phase B runs the store probe on the store's I/O queue (§9.3
"Stored values"), which finds no `s_notes` directory, so the first-enable dialog shows no
stored-values line. It shows "Notes 1.2.0 · notes · installed", the author,
the description, under "Observe and show" `ui` and `input` (required, no box) and
`world.read` (a checked box) with their reasons; no warnings (no `entity.read`); "Load
automatically" checked (the manifest's `autoload`). The pointer happens to rest where
"Allow" appears; that does not count. The user moves it off and back onto "Allow" 2 s
later and clicks. The record is written (§8.2's example) on the grant-store queue; when
the write has completed, phase C creates the store directory and writes the owner tag
"Example Author" durably (no marker: no local-only capability), and phase D compiles
the two source files (`main.lua` and `ui/pad.lua`; the other two files are data and the
manifest) and runs `main.lua`'s top level. The suggested `Ctrl+N` collides with no built-in
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

**O-11 (closed, revision 4):** TM registers every **resource limit** of this spec: the
manifest's per-field counts (TM L87), the walk file's entities, inputs, expectations,
`frames`, `[screen]` and `size` ranges (TM L86), the folder listing (TM L88), the owner
tag (TM L89), the store-root scan (TM L92), and the grant store's caps with its bounded
read, which also bounds the `--grants` and `grants` copies at 4 MiB + 1 bytes (TM L79).
TM §0.1 rule 6 (TM revision 4) answers the rest: the settings' `min`/`max` bound of ±1e15
and the grant record's integer bound 2^53 are argument-validity domains, and setting keys,
capability names, module-name segments and version parts, action and context name
lengths and the walk file's `[[entity]] name` are grammar lengths; both stay in this spec
without an L-number. Revision 3 had claimed more than rule 5 then gave; §4's preamble now
claims an L-number only for what rule 6 registers. §3.5, §4, §8.3, §8.7, §8.8, §11.4 and
§11.5 cite the rows.

**O-12 (closed, revision 5):** one "not loaded" notice. EA1b revision 4 §8.2 uses D17
§3's "No loaded extended script '⟨id⟩'" for `/ext:reload` of an id that is not loaded, as
EA1a §6.2 and §9.8 here do; §9.8's sentence is corrected.

**O-13 (closed, revision 6):** the record-flood hook's bounds. TM revision 6 registers
them in L86 (at most 64 `[[flood]]` entries, `spawn` 1–16,384 flood entities per frame,
`life` 1–600 frames, at most 65,536 flood entities alive in any frame, flood entities not
counted toward the 4,096 `[[entity]]` entries), and TM §7.5's Tests and X19 cite them.
§11.5 cites L86 and no longer calls the figures interim; `radius` (1–500 m) stays here as
an argument-validity domain (TM §0.1 rule 6).

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review), required by D17 "Required follow-ups" item 9 and the M5.5 "Specs needed" list. Folder layout and folder rules with the snapshot-root no-follow rule and the case rules (§3); the `script.toml` grammar with every field, display-string rules, module versions, contexts, actions with suggested defaults (one per stroke), settings and server hints (§4); the content-hash encoding with test vectors (§5); the capability catalogue with dialog texts (§6); effective rights and the four-phase load pipeline (§7); the grant store `ext-grants.toml` with its schema, validation, writing and damage rules (§8); the grant UI flow: dialog kinds, controls, outcomes, re-ask table, notice and Review, the manager's grant pages, developer mode (§9); the backend gate states `Off`, `Starting`, `Open`, `Draining` and loads at a transition (§10); `tessera-scripttool` with `check` codes, `test`, the walk file, the trace, fuzz targets and the `escape` catalogue covering D17's eight open security items (§11); versioning rules (§12); 23 invariants, 7 worked examples, 9 OPEN items. |
| 2026-10-07 | **Revision 2** (Review), answering the independent review and aligning with TM revision 2, EA1a, EA1b and D17. Header and §0: real companion names (EA1a and EA1b are D17's `ext-api.md`), TM normative for limits, rules and escape cases, "(proposed D17 amendment)" mark; limits cited as TM L-numbers throughout. §3.1: `keymap-ext.toml` in the configured keymap path's directory (TM §8.19); the store's `owner` tag. §3.3: device-name extension = everything from the first dot; trailing dot refused; depth stated as at most 5 path segments. §3.4 and §7.3: snapshot held under the load's memory reservation and data files charged to the cap (TM §8.10; O-4 closed). §3.5, §4.2, §4.8, §4.10, §4.12, §4.13: design additions marked. §4.1: nesting counted below the root. §4.4: refusal aligned with TM §8.2 (at most 2 marks of category M, no mark without a base, Cn and noncharacters, byte caps; O-9 closed, X47 route stated). §4.6: `entry` up to 5 segments. §4.8: undeclared modules answer `"not declared"` as EA1a §3 (O-2 closed), new W09. §4.10: priorities by EA1a §8.1 (O-3 closed). §4.14: validation stages for `check` and the load; a phase-D compilation failure closes the apartment with one close item. §8: grants created only by Allow; grant table and grant-store queue on a worker with a locked, flushed read-modify-write that completes before phase C (ARCH §21); settings numbers accept TOML integers; Remove deletes the store before the record; new §8.8, the store open with the owner check (TM §8.24), wipes, durable owner tag and marker. §9.1, §9.3: pointer entry observed after the 500 ms delay, for Allow, boxes and "Load automatically" (TM §8.25). §9.2: the re-ask table replaced by an ordered six-step pipeline with each case's record write (hash updated before phase C; the notice once per change). §9.3: outcomes for manager-started dialogs; store probe for the "Stored values" line; Deny writes nothing. §9.7: every revocation suspends in that frame (TM §8.23). §9.8: full-reload declaration set adds `author`, `api`, `manifest`, `[modules]`, `[optional_modules]`, `[server]`, after full validation (TM §8.24). §10.3, §10.4: floor frames, switch timeout row and bounds (TM §7.4, L9–L11; O-5 closed), leases end when a switch is queued, store work completes (TM §8.21). §11.1: `--json` objects specified. §11.4: X-DET-1 becomes an ordinary determinism check. §11.5: 0-based frame numbers and ranges; `[screen]`, `focus`, `switch`, `reload`, `manager` events and a `gate` expectation (TM §13). §11.6: per-event trace keys and `what` values. §11.8: runner only; cases are TM's X-tests, earlier IDs retired (TM §11.10). §13: invariants 9, 12, 15, 18 restated; 24 and 25 added. §14.4: seven items, with the gated case. §15: O-2, O-3, O-4, O-5, O-9 closed; O-10 (grant store document, proposed D17 amendment) and O-11 (field caps for TM's register) added. |
| 2026-10-07 | **Revision 3** (Review), answering the re-review of revision 2 and aligning with TM revision 3. **Store and log files** (new §3.6; §3.1, §8.8, §12.8): directory handles for `data/script-store/`, `logs/scripts/`, `<kind>` and `s_<id>` with `O_NOFOLLOW` (reparse points refused); an unusable store directory refuses the load and is never touched; every entry opened, created (exclusively), renamed or removed relative to the `s_<id>` handle with its kind and link count checked on the handle; a per-entry action table (key files kept, crash leftovers and unexpected entries removed by a no-follow removal, a non-plain `local-only` counted as present, a non-plain `owner` as missing, a subdirectory or unremovable entry fails closed); the write-behind temporary file named `t_<key>.tmp`; log files opened no-follow and otherwise left alone, with the log unavailable and reported (TM §8.8, invariant 15, X94). §8.8 restructured: handles, entry sweep, owner check, local-only check, durable tags, file index; the wipe never recurses or leaves `s_<id>`; the **store probe** defined (read-only, no creates or removals), used by §9.2 step 2 (content notice and its size) and §9.3. **Grant store** (§8.1, §8.3, §8.4, §8.5, §9.1, §9.7, §7.1): a valid hand-edited record is honoured only within §8.3 and from the next load; a re-read that removes, invalidates or narrows a loaded script's record suspends it in that frame, a widening waits for the next load (TM §8.23); the lock is `.ext-grants.toml.lock` beside the resolved document (persistent, never the settings file's lock) and the temporary file is named; the `keymap-ext.toml` bindings' removal is a locked read-modify-write (`.keymap-ext.toml.lock`, design addition). **`test`** (§11.4–§11.6): every load runs §9.2; dialogs are answered by a queued `reply`, by the default answer for a first enable, or as Escape with a run failure unless expected; `--grants FILE` seeds the tool root's grant store and the `grants` walk event replaces it mid-run with a re-read; `manager` gains `widen`; `dialog`, `granted` and `grants_file` expectations; `dialog` and `grants` trace events and the `c` key (TM X62, X90, X91, X92). **`escape`** (§11.8, §11.1): per-part results (`CLIENT` for CI parts, `ELSEWHERE` for U, FZ and RM parts), never a whole mixed test as PASS; `no_file_outside` names the lock files, backups, temporary files (only after a kill) and X94's canaries. **Smaller items:** `/ext:reload` with developer mode off, for an id not loaded and for a suspended script (§9.8); the script-list order for records outside `order` and new records appended (§7.3); §10.3 cites TM L5–L9; §10.4 rows for a load before its dialog and for one waiting on its record write or store probe (TM §8.21); §4.12 and §9.2 step 6 agree (invalid and obsolete settings values dropped by the load's own write, a reverted value never written as its default); `dump-api` accepts D17 §10's `--ext-dump-api`, and `escape`, `modules` and the subcommand are marked design additions; TM L86–L89 cited (§3.5, §4, §8.8, §11.5) and O-11 closed. Invariants 2, 9, 13, 19 and 25 restated; 26 and 27 added. |
| 2026-10-07 | **Revision 4** (Review), answering the re-review of revision 3 and its follow-up review, aligned with TM revision 4 (TM §0.1 rule 6, §8.7 and L92, §8.8, §8.16, §8.23, L26, L79). **Non-blocking, classify-before-open** (§2, §3.4, §3.6): the snapshot walk (`check` included) classifies every entry with `fstatat(…, AT_SYMLINK_NOFOLLOW)` and opens only regular files and directories, non-blocking, re-checked on the handle; the log file is classified, then opened or created `O_NONBLOCK` (a FIFO write open fails with `ENXIO`); a rotation is not made over a non-plain `s_<id>.1.log`; TM X54 ("a worker blocks") and X94 (FIFO and device log files, a FIFO grant store) cite it. **Session overlay** (§2, §8.5): a document view, a session overlay and the grant table they make; a read narrows overlay entries by what the document removed, never undoes a session decision (a failed-write revocation stays revoked), and a newer-`version` document narrows, widens and suspends nothing; a completed write is applied as a read. **Effective set** (§2, §7.1, §8.5): natives and intent applies check the apartment's effective set, never the grant table, so a manager widening after revoke and resume does not revive kept natives (TM X91). **Grant-store writes** (§7.3, §8.4, §8.5, §9.2): a write replaces every table of its key, duplicates and invalid ones included, and keeps every other table; invalid tables leave only through a write of their key; `order` drops malformed, repeated and dangling entries; the 256-table cap is checked before a dialog and in the write; a failed load write is taken back, every other failed write stays in the overlay. **Grant store as hostile input** (§8.5, §8.6): reads non-blocking, regular file checked on the handle, at most 4 MiB + 1 bytes; an "unreadable" row (`EACCES`, `EIO`, `EISDIR`, FIFO) that is never written over; backups bounded (the bytes read, or a rename for an oversized file) in the resolved document's directory; the lock taken without blocking and retried for 1 s; the queue on a worker, never an asset-worker job. **Loads in progress** (§7.4, §7.5, §10.4): one id loaded **or loading** across both kinds; new §7.5 table for a narrowing during a load, with phase D's delivery and the top-level item re-checking the record (TM X90, X91). **Orphans and Remove** (§3.5, §8.7, §9.6, §9.7): `scripts-dev` records are never orphans while developer mode is off; Remove and Reset end a loaded or loading script before the store is deleted; the store-root scan (when, which queue, no-follow `fstatat` sizes, caps TM L92, how it stays current), and the gate waits for it in `Starting` (§10.3). **Manifest** (§4.10, §4.11, §4.12, §6.1, §11.3): declaring `[[contexts]]` or `[[actions]]` needs `input`, and they are registered only when `input` is effective; overlapping suggested defaults in one context refused (M17, invariant 22); settings values checked against the running schema at every re-read; the `ui` dialog text names the chord and the manager (EA1a §7.11); finding codes for `license`, `manifest`/`api` below 1 and the key cap. **Notices** (§9.8, §10.5, O-12): D17's "No loaded extended script '<id>'" is the one text (EA1b §8.2 asked to align); outside `Open`, `/ext:list` and `/ext:help` open the script manager and `/ext:reload` with developer mode off says "Developer mode is off". **`scripttool`** (§11.1, §11.2, §11.4–§11.8): per-folder kinds (`--installed`, `--developer`), an independent `--developer-mode` with a `developer_mode` walk event and a `devmode` trace event, `--no-start`, a stated start of the run (gate `Open` at frame `latency`, world entry, autoload then user-started loads in command-line order), scripts named by id or `"<kind>/<id>"`; no settings file written; `--keymap-ext` copied into the tool root; input files and walk paths opened non-blocking, kind-checked and bounded, walk paths relative without `..`; `--json` as pure printable ASCII with explicit `\uXXXX`, non-UTF-8 name bytes as `\xNN`; the tool root removed by a no-follow recursive removal, a user-given `--root` never removed; traces identical for the same options. **Smaller:** §8.8 step 5 keeps an existing plain marker; §9.1 hides the developer console while a dialog shows (D17 §5.3); §9.3 says where a manager-started dialog's script line comes from; §9.7's revoke moves the capability to `denied`; a hot reload writes nothing to the grant store (§9.8); §14.1's probe and two source files; O-11 closed by TM rule 6 with the right citations; invariants 16, 20, 21, 22, 25 and 26 restated, 28–32 added. |
| 2026-10-07 | **Revision 5** (Review), answering the re-review of revision 4, aligned with TM revision 5 (TM §8.23 "What narrows by another route", invariant 35, L94, X91). **(1) A write's completion** (§8.5, §7.5, §2): a write carries its keys; its completion is applied as an understood read result with those keys **excluded** from the narrowing step, so per key the newest decision of the session wins over the completion of every earlier write (a revoke then widen, or a Reset then a first-enable Allow, before the older write completes keeps the grant; no load is cancelled and no apartment suspended by it); an overlay entry is dropped only when the completing write carried that very entry; a failed load write is taken back unless a newer decision of the key exists; a write that finds a newer `version` keeps its decision in the overlay and lets its load continue. **(2) Record-flood hook** (§11.5, §11.8): the walk table `[[flood]]` (per-frame spawns, life, radius, seeded positions, record order within a frame) feeds TM rule 7.5.1's list beyond L86's 4,096 walk entities, for X19's ST part and EA1a invariants 36 and 37 (EA1a OPEN-12(e) answered); its bounds are interim and asked of TM (new OPEN O-13). **(3) `[world] start`** takes `yaw` in radians in EA1a §2.10's form (KM §9.1), never a heading (EA1a OPEN-12(d) answered). **(4) Unusable re-reads** (§8.5, §8.6, §2): read results are understood, newer or unusable; only an understood one replaces the document view and can narrow (a missing file is understood as empty); an unreadable (`EACCES`, `EIO`, a Windows sharing violation) or damaged one changes nothing and suspends nothing; a write that replaces a damaged document writes every record of the grant table it carries, so it narrows nothing (TM §8.23). **(5) Reset and Remove** (§8.7 new "Reset and Remove, step by step", §7.5, §9.6, §9.7): "no record" enters the overlay in the frame the user chooses them; a load in progress is cancelled in every phase; the (kind, id) is pending deletion and its loads are refused until the store deletion completes; the grant-store write is submitted only then; a failed deletion removes nothing and keeps the "no record" for the session. **(6)** O-12 closed (EA1b revision 4 §8.2 uses D17's notice); §9.8's sentence corrected. **Smaller:** (a) `/ext:<id>` with no command word opens the script manager in every gate state, as EA1a §6.2 and `/ext:help <id>` (§10.5); (b) the `grants` trace event names `autoload`, `settings`, `order`, `rollback` and `reread` (a narrowing by a completed write included) and says which changes emit no event (§11.6); (c) M03 covers datetime values and non-finite numbers (§4.1, §11.3). Also: the walk's `manager` gains `reset`, `remove`, `autoload-on` and `autoload-off`, entries of one frame apply in file order, and X91's two ordering cases hold the earlier write in flight with a harness hook (§11.5, §11.8); TM L94 cited for the lock wait (§8.5, §8.7, invariant 30); §2 defines the understood document; invariants 25 and 29 restated, 33 and 34 added. |
| 2026-10-07 | **Revision 6** (Review), answering the re-review of revision 5, aligned with TM revision 6 (TM §8.23 "A failed write never undoes an earlier session decision" and "Developer mode turned off", L65, L84, L86, L92, X90, X91). **(1) Rollback of a failed load write** (§8.5, §2): the session overlay holds, per key, a **chain** of entries whose newest decides; a decision appends; a completed write drops its entry and every older one; a failed load write is taken back by removing its entry alone, which **restores the entry it superseded** (or none), so a revocation or Reset's or Remove's "no record" whose own write failed stays in force; any entry that is not a load write drops the older ones, so a chain holds at most two entries; step 2 narrows every entry of a chain; the example of the review is worked in §8.5; X91's rollback cases (§11.8, with the write-failure hook) and invariant 33. **(2) Developer mode turned off** (§7.5 new "Other cancellations", §9.8, §10.4, §11.4): every developer load in progress is cancelled by §7.5's rows (a showing or queued dialog closes as "Not now", store work completes, phase D gets one close item, a hot reload's new apartment is cancelled), every waiting developer load is refused with "Developer mode is off", and the user's unload of a loading copy cancels it the same way; the trace of the `developer_mode = false` event is stated (§11.4), the `dialog` event gains `closed:devmode`, `closed:unload` and `closed:narrowed` (§11.6); X90's cases (§11.8); invariant 35. **Minor:** (a) O-13 closed: §11.5 cites TM L86 for the `[[flood]]` bounds; (b) invariant 21 excepts the output files the user names (`--trace`, `pack -o`, `dump-api -o`, `--ext-dump-api -o`, `init --dir`); (c) a load deciding under an overlay entry whose write is in flight or failed writes its record again before phase C (§8.5 "Before the store is opened", §9.2 step 6, invariant 15); (d) a decision's record keeps the unknown keys and `signing_key` of the valid table it replaces (§8.5 write step 4); (e) root `format` and `version` are required, and an empty file is damaged as a whole (§8.3, §8.6); (f) Deny and "Not now" ignore presses for 500 ms with no pointer-entry rule; Escape refuses at any time (§9.1, §9.3); (g) the slice-1a figures (16 apartments, 32 ungated frames, at most 376 deterministic frames) beside the 1b ones (§10.4, invariant 18); (h) a full reload's load starts after the old close item and once the old cap has left the sum (§7.3, §9.8; TM L84); (i) the backup name exactly as the settings crate forms it, `<stamp>` 12 zero-padded digits, `-<n>` 1–99 (§8.6, §11.8); (j) slice 3 in the harness: a `reply` key `local_only`, "Neither" as the default answer, a `manager` `local-only` action, and §9.4's preset (§9.4, §11.4, §11.5, §11.6); (k) a narrowing by the document of a key whose write is in flight submits a write of the narrowed entry (§8.5 step 2); (l) the grant-store read and the store-root scan run at every entry into `Starting` (§8.5, §8.7, §10.3, §11.4; TM L92); (m) no `manager` action re-reads except the new `open`, and the harness models the manager as open (§11.5); (n) the text-value rules name which rows of §4.4 apply to `text` values (§4.12, §8.3); (o) §8.2's example hash is marked as §5.2's `hello` vector. Also: a `reply` key `delay` keeps a dialog showing so narrowings, switches and developer mode can meet it (§11.4, §11.5); the synthetic world states its region (`[world] region`, default 1) and a `region` input changes it (§11.5, answering EA1b's review of its region-change lease test). |
| 2026-10-11 | Amendment note under the header for [decision 0034](../decisions/0034-official-server-compatibility.md) §4 (extended scripts on official servers by the user's opt-in, automation offline only, D17 §6.3's build-level exclusion withdrawn). Not yet carried into the text; the M9 revision does that. Status unchanged. |
