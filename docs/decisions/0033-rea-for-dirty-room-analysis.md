<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0033 — REA as an optional dirty-room analysis tool for spec writers

**Status:** Proposed, 2026-10-11. It becomes Accepted when the pilot in "Decision" passes and the owner signs off. If the pilot fails, it becomes Rejected.

## Context
Under decision 0011, dirty-room spec writers answer M4 questions on the offline client. They write Ghidra Java postScripts and run them with `analyzeHeadless` on a private copy of the shared, auto-analysed project (`$TESSERA_RE_DIR/ghidra/project`, copied to `$TESSERA_RE_DIR/work/<key>`). Each copy is ready in seconds, but every question needs a script to be written.

The owner proposed REA (`github.com/morluto/rea`, npm `rea-agents`). It is an MIT-licensed TypeScript MCP server and CLI that drives Ghidra headless (IDA and Hopper are optional backends). For native binaries it offers decompile, disassembly, typed xrefs, callers and callees, call paths, function dossiers, value tracing across calls, read-bytes and string search. It returns "Evidence" records that carry the target's SHA-256, the analysis profile and the query. It also has many tools that are out of scope for this project: process capture, crash tracing, browser, Electron, JavaScript, Android/adb, .NET, firmware, EVM, mitmproxy, and "reconstruction" ledgers.

The evaluation (2026-10-11, rea-agents 6.4.0, commit ce845ba, in `~/nvme/rea-eval`, nothing written to the repository) found the following.

- **Supply chain.**
  - The published package has no install hooks, and none of its production dependencies has an install script.
  - No telemetry, no LLM calls, and no network egress on the Ghidra path.
  - npm signatures pass for all 144 installed packages; 32 of them carry SLSA attestations.
  - Integrity `sha512-Vj1zWfSlna5bphGoaoI/TcVBuYvze9u+OyPC3Shz8YkBqaCTqB4COnnRoX7XDwMpwI24ZlFEel2yEj0k8LTWEQ==`.
  - Release churn is very high (6.0.0 to 6.4.0 in four days).
- **Ghidra fit.**
  - REA never opens an existing project. Each session imports the target into a temporary project with `-readOnly -deleteProject` and runs full auto-analysis. The shared project's analysis of this client took 1,269 s, and REA caps JVM GC and compiler threads, so it is probably slower.
  - The default startup deadline (330 s) is too short.
  - `rea doctor` accepts Ghidra 12.1.3. It flags only the non-executable `support/analyzeHeadless` of the Gentoo package; a symlink shim with one executable copy makes every scope check pass.
  - Saving and reopening analysis snapshots (`close_binary`/`open_binary`) might avoid re-analysis. This is untested.
- **Context cost.**
  - Name, string and procedure listings return the complete set with no paging, and the response cap is at least 10 MiB.
- **Contamination.**
  - `rea setup` writes a user-scope MCP registration and installs a skill (`~/.claude.json`, `~/.claude/skills`). The skill and the README frame REA as "decompile, then reconstruct", which is the role mixing CLEAN_ROOM.md §3 and §7 forbid.
  - The milestone workflow's `agent()` calls inherit the launching session's MCP tools, so any registration would reach clean planners, builders, reviewers and verifiers.
  - REA keeps state outside today's dirty list: `~/.rea`, its temporary Ghidra projects under `TMPDIR`, `/tmp/rea-*` socket and extraction directories, snapshots and Evidence bundles.
- **Trial.** The comparison against `analyzeHeadless` on one benchmark question did not run, because the session stopped while writing a stdio driver. No measured benefit exists yet.

The owner has also said that official-server compatibility is coming, and a later decision may widen the research scope for it. This record does not set the research scope. REA works within the M4 scope of decision 0011 as amended, whatever that scope is at the time.

## Decision
REA is adopted **only as an optional supplement** for dirty-room spec writers, on these terms. `analyzeHeadless` postScripts on a copy of the shared project stay the default M4 method.

1. **Scope.**
   - REA is used only within the M4 scope of decision 0011 as amended. When this record was proposed on 2026-10-11, that scope was a copy of the offline client binary (`x64/lotroclient64.exe`), never the launcher, patcher or DLLs. Decision 0034 widened it the same day; the change log gives the current targets.
   - No queries into an area that scope excludes. When this record was proposed, these were the network protocol, login and authentication, keys and encryption, the launcher and patcher, anti-cheat and anti-tamper, and code-signing checks; the change log gives the current list. Search and trace results that land in an excluded area are discarded, not recorded.
   - This decision grants no scope of its own. A later decision that widens the M4 scope must name REA explicitly before REA may be used on what that decision opens.
2. **Who and where.**
   - REA runs only in a dedicated dirty session started by the owner. That session runs in a dirty worktree or scratch directory, holds the spec-writer role for the subsystem, and never starts clean agents or the milestone workflow.
   - REA is never used inside `.claude/workflows/tessera-milestone.js` runs.
   - Clean implementer, reviewer, verifier and auditor sessions never have it.
3. **Install (pinned, isolated).**
   - Install `rea-agents@6.4.0` exactly into `$TESSERA_RE_DIR/tools/rea`, with `npm install --prefix $TESSERA_RE_DIR/tools/rea --ignore-scripts --no-fund --no-audit rea-agents@6.4.0`. Put the npm cache on the bulk drive (`npm_config_cache=~/nvme/rea-eval/npm-cache`).
   - After installing:
     - check the integrity above;
     - run `npm audit signatures`;
     - keep the resulting lock as `package-lock.reviewed.json` next to the install.
   - No global install and nothing on `PATH`; it is called by absolute path only.
   - Never run `install.sh`, `npx rea-agents`, `rea setup`, `rea update` or `rea uninstall`. Never install the `reverse-engineer-anything` skill.
   - Every version bump repeats this review and adds a row to this record's change log.
   - The Ghidra install is not modified. `GHIDRA_INSTALL_DIR` points at the shim `$TESSERA_RE_DIR/tools/ghidra-shim`.
4. **Invocation.**
   - The session is started with `claude --strict-mcp-config --mcp-config $TESSERA_RE_DIR/tools/rea/mcp-rea.json --settings $TESSERA_RE_DIR/tools/rea/settings-rea.json`.
   - The MCP config starts `$TESSERA_RE_DIR/tools/rea/node_modules/.bin/rea --mcp` with these environment settings:
     - `HOME`, `TMPDIR`: `$TESSERA_RE_DIR/work/rea-<key>/{home,tmp}`. The path has no element starting with a dot (otherwise REA falls back to `/tmp`).
     - `REA_ANALYSIS_PROVIDER=ghidra`
     - `GHIDRA_INSTALL_DIR`: the shim
     - `JAVA_HOME`: the resolved full JDK 21
     - `REA_GHIDRA_STARTUP_TIMEOUT_MS=5400000`
     - `GHIDRA_HEADLESS_MAXMEM=10G`
   - The session sets `MCP_TOOL_TIMEOUT` to match.
   - No REA registration exists in user, project (`.mcp.json`) or local scope of any checkout.
5. **Tool allow-list.**
   - `settings-rea.json` allows only the static native tools:
     - sessions and documents;
     - segments;
     - procedure search, info, assembly and pseudocode;
     - callers, callees and references;
     - instructions and xrefs;
     - address navigation;
     - session-only names, comments and bookmarks;
     - data-type, instruction and load-image inspection;
     - `read_bytes`, `address_to_file_offset`, binary layout and PE resources;
     - `analyze_function`, `batch_decompile`, `binary_overview`;
     - call graph and call paths, `trace_feature`, `trace_native_values`;
     - `compare_functions`, `find_changed_behavior`;
     - the unknowns tools and the Evidence bundle tools.
   - It denies every other tool in the 6.4.0 catalogue, which is generated once from `tools/list`. That includes the full listings `list_names`, `list_strings` and `list_procedures`, every capture, observe, crash, browser, Electron, JavaScript, adb/Android, JEB, managed, firmware and EVM tool, `extract_artifact`, `inspect_signature`, `inspect_native_dispatch_metadata`, `trace_native_ui_action`, and the reconstruction tools.
   - REA never executes or attaches to the client.
6. **Resources.**
   - One REA session at a time on the reference laptop. It counts as one of the at most 3 heavy jobs for its whole life.
   - The session is kept open for the whole spec task rather than restarted per question.
7. **Dirty material.**
   - The target is a read-only copy in `$TESSERA_RE_DIR/work/rea-<key>/`, never the file in the install.
   - Everything REA returns or writes is dirty: pseudocode, listings, traces, snapshots, Evidence bundles, `~/.rea` and its temporary projects. It stays under `$TESSERA_RE_DIR/work/rea-<key>/`.
   - That directory is deleted when the spec is done, followed by a check that `/tmp` holds no `rea-*` leftovers.
   - Specs receive prose facts only (CLEAN_ROOM.md §5). The workflow and session hand-off rules of decisions 0018 and 0021 apply: no REA output in clean prompts, summaries or the repository.
8. **Provenance.**
   - A `BIN` row for a fact found with REA names:
     - the client version and the SHA-256 of the analysed copy;
     - the tool (`rea 6.4.0 / Ghidra 12.1.3 headless`);
     - the question;
     - why M1–M3 were not enough.
   - Evidence bundle keys are cited only in `research/notes/`, never in specs.
9. **Pilot (before Accepted).** The owner runs it in `$TESSERA_RE_DIR/work/rea-pilot`. It must record:
   - `rea doctor --provider ghidra --format json` healthy with the shim;
   - a cold start on the client copy, with wall time, peak RSS and temporary disk use;
   - whether a saved analysis snapshot reopens in a new process without re-analysis, and how long that takes;
   - one in-scope benchmark question answered both with REA and with `analyzeHeadless`, comparing time, tokens and result quality.

   The record is Accepted only if the cold start succeeds and the owner judges the benefit worth the cost. Otherwise it is Rejected and the install is removed. If snapshot reuse works, a later revision may consider REA for workflow spec writers. This decision does not allow that.

## Consequences
- The edits listed below land with this record, so that the guards (deny lists, contamination checks, the ban on MCP registration) are in place before the pilot. While the record is Proposed, the only permitted REA use is the owner's pilot in item 9:
  - `.claude/agents/spec-writer.md`: static-analysis tools section; the stale per-question M4 text replaced by decision 0011.
  - `.claude/workflows/tessera-milestone.js`: DIRTY note that REA is not used; CLEAN_READ deny list; header warning.
  - `docs/CLEAN_ROOM.md` §2, §4, §7.4.
  - `CLAUDE.md` dirty-zone row and one Current-state line.
  - `docs/TOOLING.md` §5 (table row, MCP verdict, new setup subsection).
  - `docs/DEVELOPMENT.md`.
  - A contamination check in `tools/dev-setup.sh` and `tools/check-content.sh`.
- REA lowers the effort of writing scripts for xref-heavy and value-trace questions. It costs about 20 minutes or more of startup per session, and more context per query than a targeted postScript.
- An owner session that used REA is a spec writer for every subsystem it queried (CLEAN_ROOM.md §3), exactly as with Ghidra.
- The evaluation and trial sessions of 2026-10-11 copied the client binary and read REA's own code and catalogue, but read no client analysis output. They hold no client facts.
- Withdrawing this record means uninstalling REA and deleting `$TESSERA_RE_DIR/tools/rea` and the shim. Facts already in specs keep their BIN rows and fall under decision 0011, not this record.
- This record changes neither decision 0004 nor decision 0011, and it did not change the M8 gate (decision 0034 later set that gate aside). REA's scope follows decision 0011 as amended by later records, today by decision 0034 §2.

## Change log
| Date | Change |
|------|--------|
| 2026-10-11 | Proposed after the security, fit and trial evaluation of rea-agents 6.4.0. Guard edits applied with the record; REA use limited to the pilot until Accepted. |
| 2026-10-11 | [Decision 0034](0034-official-server-compatibility.md) §2 (same day) widens the M4 scope to the official game protocol and the login flow, for M8, and names REA for it, as item 1 requires. Item 1 now reads as follows. **Targets:** read-only copies of the client binary and of the official launcher binary, and of a library either loads where in-scope code lies in it; the launcher only for its part of the login flow (0034 §2 item 1). **Excluded:** anti-cheat, anti-tamper and code-signing checks, bypassing official authentication, and the patcher with its update and download function (0034 §2 item 2); REA is never used on those. The tool allow-list of item 5 is not widened, so REA's managed-code tools stay denied. All other terms (pilot first, dedicated owner session, never in workflow runs) are unchanged. |
