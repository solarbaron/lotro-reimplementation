// SPDX-License-Identifier: GPL-3.0-or-later
//
// Reusable milestone workflow for Claude Code (Workflow tool, ultracode).
// Runs one roadmap milestone end to end, following the clean-room split in CLAUDE.md:
//   Plan  -> a planner reads docs/ROADMAP.md, creates crate skeletons, emits a task graph
//   Specs -> dirty-room spec writers (may read research/ and the clone cache), then a
//            clean contamination screen and an independent review (CLEAN_ROOM.md §5.3) by a
//            session that wrote none of the spec; only an Accepted spec feeds the build
//   Build -> clean-room implementers, in dependency order, each followed by a
//            3-lens adversarial review and a fix pass
//   Exit  -> a verifier checks every exit criterion, updates ROADMAP/README status
// The workflow never commits; the orchestrating session reviews and commits, using the
// returned `provenance` (which agent wrote, screened, reviewed and built what) in the commit
// messages (incident 0041 finding 5).
// At most args.max_builds (default 3) build/fix agents run at once: they compile and test.
//
// Invoke from a Claude Code session in the repo root, e.g.:
//   Workflow({ name: "tessera-milestone", args: {
//     milestone: "M4",
//     repo: "/abs/path/to/repo",
//     game: "/abs/path/to/game install",          // omit if no install on this machine
//     cache: "/home/<user>/.cache/tessera",       // ${XDG_CACHE_HOME:-~/.cache}/tessera:
//                                                 // research clones + scratch (dirty/clean)
//     re_dir: "/abs/path/to/tessera-re",            // optional $TESSERA_RE_DIR (dirty zone): shared
//                                                 // Ghidra project at <re_dir>/ghidra/project
//     notes: "optional extra instructions, e.g. decisions to honour" } })
// See docs/DEVELOPMENT.md §3 ("Continuing the roadmap with Claude Code").
// M8 (official-server compatibility) is in scope by the owner's decision of 2026-10-11
// (docs/decisions/0034-official-server-compatibility.md), but building is paused by incident
// 0041 (docs/decisions/0041-incident-m8-spec-contamination.md) until its specs are cleaned and
// independently re-accepted: the script refuses M8 until the owner sets M8_PAUSED below to
// false. When it runs, it adds the 0034 rules (ban-risk consent of §1 item 4, the boundary
// list of §1 item 2, no key material, dirty-only captures) to its prompts. It always refuses
// M9: M9 comes after the P-track and M8, and running it here needs a change to this script.
// Never start this workflow from a Claude Code session that has REA or any decompiler MCP
// server attached (docs/decisions/0033-rea-for-dirty-room-analysis.md): agent() calls inherit
// the session's MCP tools, clean roles included.

export const meta = {
  name: 'tessera-milestone',
  description: 'Run one Tessera roadmap milestone: plan, dirty-room specs, clean-room build with review/fix, exit check',
  whenToUse: 'Continuing the roadmap (docs/ROADMAP.md) one milestone at a time; pass args.milestone like "M4", "M4.5" or "P0"',
  phases: [
    { title: 'Plan', detail: 'read the roadmap milestone, create skeleton crates, emit task graph' },
    { title: 'Specs', detail: 'dirty-room specs confirmed on real data' },
    { title: 'Spec review', detail: 'contamination screen + independent review per spec' },
    { title: 'Build', detail: 'clean-room implementation in dependency order' },
    { title: 'Review', detail: '3-lens adversarial review per task' },
    { title: 'Fix', detail: 'apply verified findings' },
    { title: 'Exit', detail: 'verify exit criteria, update roadmap status' },
  ],
}

if (!args || !args.milestone || !args.repo || !args.cache) {
  throw new Error('args.milestone, args.repo and args.cache are required (see header comment)')
}
const M = String(args.milestone).trim().toUpperCase()
if (/^P7\b/.test(M)) {
  throw new Error('P7 (client-run game code) was dropped by the owner on 2026-10-16: with server compatibility (M8) the official server runs that code (decision 0024, open item 1 answered); this workflow will not run it')
}
if (/^M9\b/.test(M)) {
  throw new Error('M9 (extended scripting) starts only after full parity: P8 (decision 0024) and M8 (decision 0034 §1 item 3) (owner, 2026-10-08); this workflow will not run it')
}
// Incident 0041 (2026-10-22): the M8 specs are held or back in Review. The owner sets this to
// false once they are cleaned and independently re-accepted.
const M8_PAUSED = true
if (M === 'M8' && M8_PAUSED) {
  throw new Error('M8 is paused by incident 0041 (docs/decisions/0041-incident-m8-spec-contamination.md): its specs must be cleaned and independently re-accepted first; the owner then sets M8_PAUSED = false in this script')
}
// P0..P8: the client-half parity track (docs/decisions/0024-parity-track.md, owner 2026-10-09).
if (!/^(?:M(?:[0-8]|4\.5)|P[0-8])$/.test(M)) {
  throw new Error(`args.milestone must be one of M0..M8, M4.5 or P0..P8 (got "${args.milestone}")`)
}
const IS_M8 = M === 'M8'
const REPO = args.repo
const GAME = args.game || null
// Dirty and clean agents get separate scratch dirs: dirty probe scripts and their output
// must never be readable by clean-room agents.
const SCRATCH_DIRTY = `${args.cache}/scratch/${M}/dirty`
const SCRATCH_CLEAN = `${args.cache}/scratch/${M}/clean`
const CLONES = `${args.cache}/clones`
const NOTES = args.notes || ''
// Optional dirty reverse-engineering dir ($TESSERA_RE_DIR, CLAUDE.md dirty zone): shared Ghidra project etc.
const RE_DIR = args.re_dir || null
const MAX_BUILDS = Math.max(1, Number(args.max_builds) || 3)
// Spec review rounds: review, revise, review again; a spec still not passing is not built from.
const SPEC_ROUNDS = 3

const CTX = `Repo: ${REPO} (Rust workspace "Tessera": clean-room client compatible with LOTRO data files). ${REPO}/CLAUDE.md is binding (clean-room zones, content rules, conventions). Decisions: ${REPO}/docs/decisions/ (binding). Roadmap: ${REPO}/docs/ROADMAP.md — we are working on milestone ${M}; earlier milestones' status tables say what exists. Read the public APIs of existing crates before adding to them.
${GAME ? `The user's game install is READ-ONLY at "${GAME}" — never write there; set TESSERA_GAME_DIR to it for real-data tests.` : 'No game install is available on this machine: real-data tests are skipped; say clearly which criteria could not be verified.'}
Scratch dir for throwaway files: the one named in your role below (create it). Delete large throwaway outputs when done. Do not create extra cargo target dirs.
Concurrency: other agents edit other crates in this same checkout at the same time, and the milestone's work stays UNCOMMITTED until the orchestrating session reviews it. Only edit files you own. Add dependencies to YOUR crate's Cargo.toml only (workspace = true for crates in root [workspace.dependencies]); do NOT edit the root Cargo.toml (the planner already did). Fuzz targets: only append your own [[bin]] entries to fuzz/Cargo.toml. Never git commit. NEVER run git reset --hard, git clean, git stash or git amend (and never git checkout/restore a file you do not own): they silently destroy other agents' uncommitted new files and edits in this shared checkout. For a throwaway experiment use your scratch dir, not git.
${NOTES ? 'Extra instructions from the owner: ' + NOTES : ''}`

// Official-server rules for M8 (docs/decisions/0034-official-server-compatibility.md §1–§3).
const M8_DIRTY = `M8 RULES (decision 0034, binding): ban-risk consent (§1 item 4): the design must show a warning about the account risk of a third-party client (account suspension or closure, no protection or indemnity from the project) before any connection to an official server is possible, shown on first use and whenever its text changes, accepted only by the user's explicit confirmation in the client's own interface, never by a flag, environment variable, configuration file written by anything else, plugin, script or server. The boundary list (§1 item 2): no shipping, mirroring or linking to client binaries, dat files, patches or old clients; no modifying, patching, redirecting or injecting into the official client; no embedding or publishing Daybreak keys or other secrets in the repository, releases or documentation; no committing packet captures from official servers; no using leaked, NDA-covered or decompiled Turbine/SSG/Daybreak source code; no bypassing account authentication, subscription checks, anti-cheat or any other access control. Key material, certificates, hostnames and endpoints: a spec states only WHERE the client finds them at runtime in the user's install, never their values; no account data and no other player's names, chat or data in any spec. Research: disassembly/decompilation of read-only copies of the client binary and of the official launcher binary (the launcher only for its part of the login flow; a library either loads only where in-scope code lies in it; never the patcher or its update and download code), and passive captures of the owner's own sessions of the unmodified client, on the owner's machine and account, only; no injection, debugger or memory reading on a client connected to an official server (docs/TOOLING.md §2 rule 5); captures stay in ${SCRATCH_DIRTY} or ${RE_DIR || '$TESSERA_RE_DIR'}/captures, are never committed or quoted, and are reduced to field-level facts. If the service can be used only by circumventing anti-cheat, anti-tamper or another access control, stop and report it as an OPEN; never design a workaround. Extended scripts are M9, not M8. Do not log in to an official server yourself: live logins are the owner's, on the owner's own account.`

const DIRTY = `You are a SPEC WRITER (dirty room). You MAY read ${REPO}/research/ and third-party sources cloned under ${CLONES} (run ${REPO}/tools/fetch-research-clones.sh "${CLONES}" first if it is empty), and probe the real data with throwaway scripts in ${SCRATCH_DIRTY} (your scratch dir). Research methods (docs/CLEAN_ROOM.md §4): M1 (observe the data files) and M2 (published research and third-party tools, facts only) are allowed. M4 (disassembly or decompilation of the official client, e.g. with Ghidra) is authorised for the offline client by ${REPO}/docs/decisions/0011-standing-m4-authorisation.md: use it after M1–M3; record each use as a BIN provenance row (client build and SHA-256 of the analysed binary, tool and version, question, why M1–M3 were not enough); ${IS_M8 ? 'for M8 the scope also covers the official game protocol and the login flow (the launcher\'s part included; targets in decision 0034 §2 item 1) as far as interoperability needs them (decision 0034 §2)' : 'the official network protocol and login belong to M8 only (decision 0034), so leave them alone in this milestone'}; never touch anti-cheat, anti-tamper, code-signing checks or the patcher's update function, and never circumvent an access control; never put listings, pseudo-code or decompiler output in the repo, only prose facts. ${RE_DIR ? `Ghidra is installed (headless: bash /usr/share/ghidra/support/analyzeHeadless; the file is not executable, so run it through bash). An auto-analysed Ghidra project of the client is at ${RE_DIR}/ghidra/project (project name tessera-client). Ghidra locks an open project and several agents run at once: copy that project directory into your own dirty scratch dir under a path with no element starting with '.' (Ghidra rejects such paths; use ${RE_DIR}/work/<your key>), run your own headless scripts there (-process -noanalysis -readOnly -postScript ...), and delete your copy when done. ` : ''}REA (decision 0033) is not used in workflow runs: do not install, start or call rea, rea-agents or npx rea-agents, and do not write MCP configuration; for M4, use analyzeHeadless on your own project copy. Output prose + tables of functional facts only: no copied code or transliterated pseudo-code, no game text (refer to records by DID). Follow the style of existing ${REPO}/docs/specs/*.md: provenance table with Confirmed/Reported, numbered sections with <a id> anchors, byte layouts, algorithms in words, invariants, "> **OPEN:**" callouts. Confirm layouts over ALL records of each type with statistics where data is available. Never describe disassembly in prose (incident 0041): no registers, stack or frame offsets, vtable slots, call sequences, client-internal function, class or member names, or in-memory object layouts; state the wire or file fact, not how the client's code establishes it. Leave the spec in status Review with an empty reviewer field: an independent reviewer accepts it, never you.${IS_M8 ? ' ' + M8_DIRTY : ''}`

// Read restrictions shared by every clean-room role (planner, implementer, reviewer, fixer,
// exit verifier).
const CLEAN_READ = `CLEAN-ROOM RULES (non-negotiable, CLAUDE.md): do NOT open ${REPO}/research/, ${CLONES}, ${SCRATCH_DIRTY}${RE_DIR ? ', ' + RE_DIR : ''}, ~/.rea, /tmp/rea-*, any tessera-re directory, third-party LOTRO/AC/DDO source code or decompiler output, and do not look formats up online. Do not run analyzeHeadless, rea, rea-agents or any disassembler or decompiler, and do not call any MCP tool that decompiles or disassembles; if such a tool is visible to you, stop and report it. Work only from ${REPO}/docs/ (specs, decisions, roadmap, architecture) and the existing crates/apps/fuzz/tools. Your scratch dir is ${SCRATCH_CLEAN}. If a spec lacks a fact or looks wrong, add a "> **OPEN:**" note to it or report it; never correct a spec's facts yourself.`

const CLEAN = `You are a CLEAN-ROOM IMPLEMENTER. ${CLEAN_READ} A missing fact means: implement conservatively and add a "> **OPEN:**" note to the spec. Requirements: no panics on hostile input, bounded allocation, thiserror errors, rustdoc on public API, synthetic unit tests, real-data tests gated on TESSERA_GAME_DIR (run them when available; --release for heavy ones), SPDX headers, cargo fmt + cargo clippy -p <crate> --all-targets -- -D warnings clean, a fuzz target for every new decoder (run 60 s each with tools/fuzz-smoke.sh -t 60 <target>). Test only the crates you changed (cargo test -p <crate>, --release for real-data tests); the exit verifier runs the whole workspace once. Read test output through its summary lines (grep "test result"/FAILED), not whole logs.${IS_M8 ? ' M8 (decision 0034): no official hostname, endpoint, key, certificate or account data in code, tests or fixtures (load them at runtime from the user\'s install as the spec says); no connection to an official server is possible before the user has accepted the ban-risk warning of decision 0034 §1 item 4 in the client UI; tests never connect to an official server.' : ''}`

// Compact, structured returns keep the reviewers', fixer's and exit verifier's prompts small.
const IMPL_SCHEMA = { type: 'object', properties: {
  summary: { type: 'string', description: 'at most 10 lines: public API added or changed, behaviour' },
  files: { type: 'array', items: { type: 'string' } },
  diff_lines: { type: 'number', description: 'added + removed lines (git diff --shortstat)' },
  tests: { type: 'string', description: 'one line per test command: passed/failed counts' },
  spec_refs: { type: 'array', items: { type: 'string' } },
  opens: { type: 'array', items: { type: 'string' }, description: 'OPEN notes filed: spec and section, one line each' } },
  required: ['summary', 'files', 'diff_lines', 'tests', 'spec_refs', 'opens'] }
const FIX_SCHEMA = { type: 'object', properties: {
  fixed: { type: 'array', items: { type: 'string' } }, rejected: { type: 'array', items: { type: 'string' } },
  tests: { type: 'string' } }, required: ['fixed', 'rejected', 'tests'] }
// Changes up to this size get one reviewer covering all three lenses; larger ones get one per lens.
const SMALL_DIFF = 800

const PLAN_SCHEMA = {
  type: 'object',
  properties: {
    summary: { type: 'string' },
    specs: { type: 'array', items: { type: 'object', properties: {
      key: { type: 'string' }, files: { type: 'string' }, task: { type: 'string' } },
      required: ['key', 'files', 'task'] } },
    tasks: { type: 'array', items: { type: 'object', properties: {
      key: { type: 'string' }, owns: { type: 'string', description: 'paths this task may edit' },
      deps: { type: 'array', items: { type: 'string' }, description: 'keys of specs or tasks this waits for' },
      spec: { type: 'string', description: 'spec files the implementer works from' },
      task: { type: 'string' } },
      required: ['key', 'owns', 'deps', 'spec', 'task'] } },
    owner_observations: { type: 'array', items: { type: 'string' } },
  },
  required: ['summary', 'specs', 'tasks', 'owner_observations'],
}

const REVIEW_SCHEMA = { type: 'object', properties: { findings: { type: 'array', items: { type: 'object', properties: {
  severity: { type: 'string' }, location: { type: 'string' }, problem: { type: 'string' }, evidence: { type: 'string' }, fix: { type: 'string' } },
  required: ['severity', 'location', 'problem', 'evidence', 'fix'] } } }, required: ['findings'] }

phase('Plan')
const plan = await agent(`${CTX}\n\n${CLEAN_READ}\n\nYou are the MILESTONE PLANNER for ${M} (a clean-room role: you may read docs/, not research/). Read docs/ROADMAP.md (the ${M} section: deliverables, specs needed, RE tasks, exit criteria), docs/ARCHITECTURE.md, docs/decisions/, and the existing workspace. Then:
1. Create skeleton crates/apps the milestone needs that do not exist yet (Cargo.toml using workspace package fields + thiserror/tracing deps + [lints] workspace = true; src/lib.rs with SPDX header), and register them in the root Cargo.toml members and [workspace.dependencies]. Run cargo check --workspace.
2. Emit a plan: specs (dirty-room writing tasks, each naming the files it writes under docs/specs/ or docs/checklists/), and implementation tasks with non-overlapping 'owns' paths and dependency keys (a task may depend on spec keys and other task keys). Prefer 4–8 tasks; split by crate. Every exit criterion must be covered by some task. Decision records required by the roadmap go to docs/decisions/NNNN-*.md (+ index row in docs/decisions/README.md) via a task.
3. List exit criteria that only the project owner can observe in the original client (owner_observations); they go to docs/checklists/owner-observations.md.
Do not implement features yourself.`, { label: `plan:${M}`, phase: 'Plan', schema: PLAN_SCHEMA })

if (!plan) throw new Error('planner failed')
log(`${M}: ${plan.specs.length} specs, ${plan.tasks.length} tasks`)

// Validate the task graph before starting anything: unique keys (specs and tasks share one
// namespace), every dependency known, no cycles.
const NODES = new Map()
for (const s of plan.specs) {
  if (NODES.has(s.key)) throw new Error(`plan: duplicate key "${s.key}"`)
  NODES.set(s.key, { kind: 'spec', item: s })
}
for (const t of plan.tasks) {
  if (NODES.has(t.key)) throw new Error(`plan: duplicate key "${t.key}"`)
  NODES.set(t.key, { kind: 'task', item: t })
}
for (const t of plan.tasks) {
  t.deps = (t.deps || []).filter(d => d !== t.key)
  const unknown = t.deps.filter(d => !NODES.has(d))
  if (unknown.length) throw new Error(`plan: task "${t.key}" depends on unknown key(s) ${unknown.join(', ')}`)
}
{
  const state = new Map() // key -> 'visiting' | 'done'
  const visit = (k, path) => {
    if (state.get(k) === 'done') return
    if (state.get(k) === 'visiting') throw new Error(`plan: dependency cycle ${[...path, k].join(' -> ')}`)
    state.set(k, 'visiting')
    const n = NODES.get(k)
    if (n.kind === 'task') for (const d of n.item.deps) visit(d, [...path, k])
    state.set(k, 'done')
  }
  for (const k of NODES.keys()) visit(k, [])
}

// A dependency result counts as failed when the agent returned nothing, threw, or was
// skipped because one of its own dependencies failed.
const failed = r => r == null || (typeof r === 'object' && (r.error || r.skipped || (r.impl === null)))

// Caps concurrent heavy agents (builds and fixes compile and test the workspace).
function limiter(n) {
  let active = 0
  const queue = []
  const next = () => {
    if (active >= n || !queue.length) return
    active++
    const [fn, resolve, reject] = queue.shift()
    fn().then(resolve, reject).finally(() => { active--; next() })
  }
  return fn => new Promise((resolve, reject) => { queue.push([fn, resolve, reject]); next() })
}
const heavy = limiter(MAX_BUILDS)

// Who did what, for the commit messages (incident 0041 finding 5). Agent labels only.
const PROV = {}
const prov = (k, entry) => { (PROV[k] = PROV[k] || []).push(entry) }

const SCREEN_SCHEMA = { type: 'object', properties: {
  clean: { type: 'boolean', description: 'true when no contamination sign was found' },
  findings: { type: 'array', items: { type: 'string' }, description: 'section + sign, one line each; never quote the offending text' } },
  required: ['clean', 'findings'] }
const SPEC_REVIEW_SCHEMA = { type: 'object', properties: {
  accepted: { type: 'boolean', description: 'true only when every CLEAN_ROOM.md §5.3 item holds and you set the status to Accepted' },
  blocking: { type: 'array', items: { type: 'string' } },
  record: { type: 'string', description: 'one line: reviewer label, date, result, as written in the header' } },
  required: ['accepted', 'blocking', 'record'] }

// Contamination screen: a clean auditor reads only the spec file(s) (docs/specs is the clean
// interface), so it may look for dirty-room signs a writer missed. Its result goes back to
// dirty revisers only, never to implementers.
const screenSpec = (s, round) => agent(`${CTX}\n\n${CLEAN_READ}\n\nYou are a CONTAMINATION SCREENER (read-only; edit nothing). Read only ${s.files}. Report signs that dirty material crossed into the spec (docs/CLEAN_ROOM.md §5.1, §5.3, incident 0041): code or pseudo-code in any language; disassembly described in prose (registers, stack or frame offsets, vtable slots, call sequences, client-internal function, class or member names, in-memory object layouts); copied third-party prose or comments; game text; keys, certificates, hostnames, endpoints, account data or traffic dumps. Name the section and the kind of sign; never quote the text.`,
  { label: `screen:${s.key}:${round}`, phase: 'Spec review', agentType: 'clean-room-auditor', schema: SCREEN_SCHEMA })

// Independent review (CLEAN_ROOM.md §5.3): a fresh spec-writer session that wrote none of the
// spec checks it; only it may set Accepted. A failing spec is revised by another fresh session
// and reviewed again by yet another.
async function reviewSpec(s) {
  let issues = null
  for (let round = 1; round <= SPEC_ROUNDS; round++) {
    if (issues) {
      const rev = `revise:${s.key}:${round}`
      await agent(`${CTX}\n\n${DIRTY}\n\nRevise ${s.files} (task: ${s.task}). Resolve every issue below; keep section numbers stable; add a change-log row; leave the status Review. Return one line only.\n\nISSUES:\n${JSON.stringify(issues, null, 1)}`,
        { label: rev, phase: 'Spec review', agentType: 'spec-writer' })
      prov(s.key, { role: 'spec', agent: rev })
    }
    const screen = await screenSpec(s, round)
    prov(s.key, { role: 'screen', agent: `screen:${s.key}:${round}`, clean: screen ? screen.clean : null })
    if (!screen) return { error: 'contamination screen failed' }
    const label = `spec-review:${s.key}:${round}`
    const review = await agent(`${CTX}\n\n${DIRTY}\n\nYou are an INDEPENDENT SPEC REVIEWER (CLEAN_ROOM.md §3, §5.3). You wrote none of ${s.files}; do not write facts into it. Check every §5.3 item, spot-check facts against the cited sources and the install, and treat each contamination-screen finding below as blocking until resolved. If every item holds AND the screen is clean, set the status to Accepted and write the review record (reviewer "${label} (independent session)", today's date from \`date +%F\`, result, items checked) in the header; otherwise leave it in Review and list the blocking issues. Never quote dirty material in your answer.\n\nCONTAMINATION SCREEN: ${JSON.stringify(screen)}`,
      { label, phase: 'Spec review', agentType: 'spec-writer', schema: SPEC_REVIEW_SCHEMA })
    prov(s.key, { role: 'review', agent: label, accepted: review ? review.accepted : null })
    if (!review) return { error: 'spec reviewer failed' }
    if (review.accepted && screen.clean) return { accepted: true, record: review.record }
    issues = [...(screen.findings || []).map(f => `screen: ${f}`), ...(review.blocking || [])]
    if (!issues.length) issues = ['reviewer did not accept and gave no reason: re-check every §5.3 item']
  }
  return { skipped: `spec ${s.key} not Accepted after ${SPEC_ROUNDS} review rounds` }
}

async function reviewFix(t, impl) {
  if (impl == null) return 'skipped: implementer returned nothing'
  const lenses = [
    ['correctness', `Correctness against ${t.spec || 'the ' + M + ' roadmap deliverables'}: layouts, algorithms, conventions, edge cases. Write throwaway tests (also against the real install when available); take and view screenshots for visual features.`],
    ['robustness', `Hostile input and runtime safety: truncation, huge counts, overflow, unbounded allocation, unwrap/expect/indexing on untrusted data, resource leaks, main-thread stalls, races and thread-safety.`],
    ['quality', `API design, idiomatic Rust, docs, test gaps, CLAUDE.md compliance (SPDX headers, tracing, no game data or text in the repo, clean-room provenance), clippy -D warnings, fit for later milestones.`],
  ]
  const scope = `Files changed: ${(impl.files || []).join(', ') || t.owns}. Tests the implementer ran: ${impl.tests || 'not reported'}.`
  const ask = (k, p) => agent(`${CTX}\n\n${CLEAN_READ}\n\nYou are a skeptical REVIEWER of ${t.owns} (do not edit those files; put throwaway tests in ${SCRATCH_CLEAN} or remove them afterwards). Check against the specs in docs/specs/ only (${t.spec || 'roadmap deliverables'}). ${scope} Lens: ${p}\nReport only problems you verified, with file:line, a concrete failing scenario and a suggested fix.`,
    { label: `review:${t.key}:${k}`, phase: 'Review', schema: REVIEW_SCHEMA })
  // One combined reviewer for small changes, one per lens for large ones.
  const fs = (impl.diff_lines || 0) <= SMALL_DIFF
    ? [await ask('all', lenses.map(([k, p]) => `(${k}) ${p}`).join(' '))]
    : await parallel(lenses.map(([k, p]) => () => ask(k, p)))
  const used = fs.length === 1 ? [['all']] : lenses
  const ok = fs.filter(Boolean)
  if (!ok.length) return 'reviewers failed: no lens returned a result'
  const all = ok.flatMap(f => f.findings || [])
  const missing = used.filter((_, i) => !fs[i]).map(([k]) => k)
  if (!all.length) return missing.length ? `no findings (lenses that failed: ${missing.join(', ')})` : 'no findings'
  prov(t.key, { role: 'implementation', agent: `fix:${t.key}`, owns: t.owns })
  return heavy(() => agent(`${CTX}\n\n${CLEAN}\n\nYou own ${t.owns}. Verify each finding below; fix real ones with a regression test; explain rejections. Re-run fmt, clippy -D warnings and the tests of the crates you changed. Return what you fixed and rejected (one line each) and the test summary.\n\nFINDINGS:\n${JSON.stringify(all, null, 1)}`,
    { label: `fix:${t.key}`, phase: 'Fix', schema: FIX_SCHEMA }))
}

// Promises are created lazily and memoised, so the order of plan.tasks does not matter.
const P = new Map()
function start(k) {
  if (P.has(k)) return P.get(k)
  const n = NODES.get(k)
  let p
  if (n.kind === 'spec') {
    const s = n.item
    prov(s.key, { role: 'spec', agent: `spec:${s.key}`, files: s.files })
    p = agent(`${CTX}\n\n${DIRTY}\n\nWrite ${s.files}.\n${s.task}\nReturn at most 5 lines: the files written and their status. Do not repeat the facts (no clean session may see them).`,
      { label: `spec:${s.key}`, phase: 'Specs', agentType: 'spec-writer' })
      .then(w => w == null ? { error: 'spec writer returned nothing' } : reviewSpec(s))
      .catch(e => ({ error: String(e) }))
  } else {
    const t = n.item
    p = Promise.all(t.deps.map(d => start(d).then(r => [d, r])))
      .then(async deps => {
        const bad = deps.filter(([, r]) => failed(r)).map(([d]) => d)
        if (bad.length) return { skipped: `dependency failed or was skipped: ${bad.join(', ')}` }
        prov(t.key, { role: 'implementation', agent: `build:${t.key}`, owns: t.owns, spec: t.spec })
        const impl = await heavy(() => agent(`${CTX}\n\n${CLEAN}\n\nYou own ${t.owns}. Specs: ${t.spec || '(design task, no spec)'}; build only from specs whose status is Accepted with an independent review record (stop and report otherwise).\n${t.task}`,
          { label: `build:${t.key}`, phase: 'Build', schema: IMPL_SCHEMA, agentType: 'clean-implementer' }))
        return { impl, review: await reviewFix(t, impl) }
      })
      .catch(e => ({ error: String(e) }))
  }
  P.set(k, p)
  return p
}
// Start every node first, then wait: awaiting start(k) key by key would run them one at a time.
const KEYS = [...NODES.keys()]
const settled = await Promise.all(KEYS.map(start))
const results = Object.fromEntries(KEYS.map((k, i) => [k, settled[i]]))
const problems = Object.entries(results).filter(([, r]) => failed(r)).map(([k, r]) => `${k}: ${r == null ? 'no result' : r.error || r.skipped || 'implementer returned nothing'}`)
if (problems.length) log(`${M}: failed/skipped: ${problems.join('; ')}`)
// The exit verifier is a clean-room session: it gets spec KEYS, files and status only, never
// the spec writers' returned text (dirty-room output; decision 0021). Implementation results
// come from clean sessions and may be passed on.
const summary = Object.fromEntries(Object.entries(results).map(([k, r]) => {
  const n = NODES.get(k)
  if (n.kind === 'spec') {
    return [k, failed(r) ? { kind: 'spec', files: n.item.files, status: 'FAILED/SKIPPED' }
      : { kind: 'spec', files: n.item.files, status: 'Accepted after independent review (read the spec header for the record)' }]
  }
  return [k, failed(r) ? { status: 'FAILED/SKIPPED', detail: r == null ? null : (r.error || r.skipped || 'implementer returned nothing') }
    : { status: 'done', impl: r.impl, review: r.review }]
}))

phase('Exit')
const exit = await agent(`${CTX}\n\n${CLEAN_READ}\n\nFINAL EXIT-CRITERIA VERIFIER for ${M} (docs/ROADMAP.md). Check every ${M} exit criterion with real evidence: commands and numbers, and screenshots viewed with Read for visual criteria. Criteria that need the owner's observation in the original client: verify everything data-side, mark them "data-side done, owner observation pending" and make sure they are listed in docs/checklists/owner-observations.md. Also run: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (with TESSERA_GAME_DIR when available); a 60 s fuzz smoke run of new targets; tools/check-content.sh; tools/check-spdx.sh; doc link check. FIX failures in code, tests, tools and non-spec docs; for a spec problem add a "> **OPEN:**" note or report it, do not change spec facts. Do not change any spec's status. Check that every spec a task built from is Accepted with an independent review record in its header; report any that is not. Tasks that FAILED or were SKIPPED below did not run: their criteria are not met unless you can show otherwise; report them. Update the ${M} status table in docs/ROADMAP.md, the README status, and the "Current state" section of CLAUDE.md. Return the criterion table with evidence and remaining issues.

PER-TASK RESULTS (implementer summaries, reviews, failures):
${JSON.stringify(summary, null, 1)}

OWNER OBSERVATIONS the planner listed (must appear in docs/checklists/owner-observations.md):
${JSON.stringify(plan.owner_observations || [], null, 1)}`,
  { label: `exit:${M}`, phase: 'Exit', agentType: 'clean-implementer' })
prov('exit', { role: 'implementation', agent: `exit:${M}` })

return { plan, results, exit, provenance: PROV }
