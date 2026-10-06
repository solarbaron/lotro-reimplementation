// SPDX-License-Identifier: GPL-3.0-or-later
//
// Reusable milestone workflow for Claude Code (Workflow tool, ultracode).
// Runs one roadmap milestone end to end, following the clean-room split in CLAUDE.md:
//   Plan  -> a planner reads docs/ROADMAP.md, creates crate skeletons, emits a task graph
//   Specs -> dirty-room spec writers (may read research/ and the clone cache)
//   Build -> clean-room implementers, in dependency order, each followed by a
//            3-lens adversarial review and a fix pass
//   Exit  -> a verifier checks every exit criterion, updates ROADMAP/README status
// The workflow never commits; the orchestrating session reviews and commits.
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
// M8 is gated (docs/decisions/0004-server-strategy.md): this script refuses to run it.

export const meta = {
  name: 'tessera-milestone',
  description: 'Run one Tessera roadmap milestone: plan, dirty-room specs, clean-room build with review/fix, exit check',
  whenToUse: 'Continuing the roadmap (docs/ROADMAP.md) one milestone at a time; pass args.milestone like "M4" or "M4.5"',
  phases: [
    { title: 'Plan', detail: 'read the roadmap milestone, create skeleton crates, emit task graph' },
    { title: 'Specs', detail: 'dirty-room specs confirmed on real data' },
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
if (/^M8\b/.test(M)) {
  throw new Error('M8 is gated (docs/decisions/0004-server-strategy.md): record the legal review as a new decision and ask the owner first; this workflow will not run it')
}
if (!/^M(?:[0-7]|4\.5)$/.test(M)) {
  throw new Error(`args.milestone must be one of M0..M7 or M4.5 (got "${args.milestone}")`)
}
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

const CTX = `Repo: ${REPO} (Rust workspace "Tessera": clean-room client compatible with LOTRO data files). ${REPO}/CLAUDE.md is binding (clean-room zones, content rules, conventions). Decisions: ${REPO}/docs/decisions/ (binding). Roadmap: ${REPO}/docs/ROADMAP.md — we are working on milestone ${M}; earlier milestones' status tables say what exists. Read the public APIs of existing crates before adding to them.
${GAME ? `The user's game install is READ-ONLY at "${GAME}" — never write there; set TESSERA_GAME_DIR to it for real-data tests.` : 'No game install is available on this machine: real-data tests are skipped; say clearly which criteria could not be verified.'}
Scratch dir for throwaway files: the one named in your role below (create it). Delete large throwaway outputs when done. Do not create extra cargo target dirs.
Concurrency: other agents edit other crates at the same time. Only edit files you own. Add dependencies to YOUR crate's Cargo.toml only (workspace = true for crates in root [workspace.dependencies]); do NOT edit the root Cargo.toml (the planner already did). Fuzz targets: only append your own [[bin]] entries to fuzz/Cargo.toml. Never git commit.
${NOTES ? 'Extra instructions from the owner: ' + NOTES : ''}`

const DIRTY = `You are a SPEC WRITER (dirty room). You MAY read ${REPO}/research/ and third-party sources cloned under ${CLONES} (run ${REPO}/tools/fetch-research-clones.sh "${CLONES}" first if it is empty), and probe the real data with throwaway scripts in ${SCRATCH_DIRTY} (your scratch dir). Research methods (docs/CLEAN_ROOM.md §4): M1 (observe the data files) and M2 (published research and third-party tools, facts only) are allowed. M4 (disassembly or decompilation of the official client, e.g. with Ghidra) is authorised for the offline client by ${REPO}/docs/decisions/0011-standing-m4-authorisation.md: use it after M1–M3; record each use as a BIN provenance row (client build, question, why M1–M3 were not enough); never touch the network protocol, login, keys, launcher or anti-tamper (those stay gated: decision 0004); never put listings, pseudo-code or decompiler output in the repo, only prose facts. ${RE_DIR ? `Ghidra is installed (headless: bash /usr/share/ghidra/support/analyzeHeadless; the file is not executable, so run it through bash). An auto-analysed Ghidra project of the client is at ${RE_DIR}/ghidra/project (project name tessera-client). Ghidra locks an open project and several agents run at once: copy that project directory into your own dirty scratch dir under a path with no element starting with '.' (Ghidra rejects such paths; use ${RE_DIR}/work/<your key>), run your own headless scripts there (-process -noanalysis -readOnly -postScript ...), and delete your copy when done. ` : ''}Output prose + tables of functional facts only: no copied code or transliterated pseudo-code, no game text (refer to records by DID). Follow the style of existing ${REPO}/docs/specs/*.md: provenance table with Confirmed/Reported, numbered sections with <a id> anchors, byte layouts, algorithms in words, invariants, "> **OPEN:**" callouts. Confirm layouts over ALL records of each type with statistics where data is available.`

// Read restrictions shared by every clean-room role (planner, implementer, reviewer, fixer,
// exit verifier).
const CLEAN_READ = `CLEAN-ROOM RULES (non-negotiable, CLAUDE.md): do NOT open ${REPO}/research/, ${CLONES}, ${SCRATCH_DIRTY}${RE_DIR ? ', ' + RE_DIR : ''}, third-party LOTRO/AC/DDO source code or decompiler output, and do not look formats up online. Work only from ${REPO}/docs/ (specs, decisions, roadmap, architecture) and the existing crates/apps/fuzz/tools. Your scratch dir is ${SCRATCH_CLEAN}. If a spec lacks a fact or looks wrong, add a "> **OPEN:**" note to it or report it; never correct a spec's facts yourself.`

const CLEAN = `You are a CLEAN-ROOM IMPLEMENTER. ${CLEAN_READ} A missing fact means: implement conservatively and add a "> **OPEN:**" note to the spec. Requirements: no panics on hostile input, bounded allocation, thiserror errors, rustdoc on public API, synthetic unit tests, real-data tests gated on TESSERA_GAME_DIR (run them when available; --release for heavy ones), SPDX headers, cargo fmt + cargo clippy -p <crate> --all-targets -- -D warnings clean, a fuzz target for every new decoder (run 60 s each with tools/fuzz-smoke.sh -t 60 <target>). Finish with: public API summary, test summary lines, OPEN items.`

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

async function reviewFix(t, impl) {
  if (impl == null) return 'skipped: implementer returned nothing'
  const lenses = [
    ['correctness', `Correctness against ${t.spec || 'the ' + M + ' roadmap deliverables'}: layouts, algorithms, conventions, edge cases. Write throwaway tests (also against the real install when available); take and view screenshots for visual features.`],
    ['robustness', `Hostile input and runtime safety: truncation, huge counts, overflow, unbounded allocation, unwrap/expect/indexing on untrusted data, resource leaks, main-thread stalls, races and thread-safety.`],
    ['quality', `API design, idiomatic Rust, docs, test gaps, CLAUDE.md compliance (SPDX headers, tracing, no game data or text in the repo, clean-room provenance), clippy -D warnings, fit for later milestones.`],
  ]
  const fs = await parallel(lenses.map(([k, p]) => () => agent(`${CTX}\n\n${CLEAN_READ}\n\nYou are a skeptical REVIEWER of ${t.owns} (do not edit those files; put throwaway tests in ${SCRATCH_CLEAN} or remove them afterwards). Check against the specs in docs/specs/ only. Lens: ${p}\nReport only problems you verified, with file:line, a concrete failing scenario and a suggested fix.`,
    { label: `review:${t.key}:${k}`, phase: 'Review', schema: REVIEW_SCHEMA })))
  const ok = fs.filter(Boolean)
  if (!ok.length) return 'reviewers failed: no lens returned a result'
  const all = ok.flatMap(f => f.findings || [])
  const missing = lenses.filter((_, i) => !fs[i]).map(([k]) => k)
  if (!all.length) return missing.length ? `no findings (lenses that failed: ${missing.join(', ')})` : 'no findings'
  return agent(`${CTX}\n\n${CLEAN}\n\nYou own ${t.owns}. Verify each finding below; fix real ones with a regression test; explain rejections. Re-run fmt, clippy -D warnings and tests. Return a finding -> fixed/rejected table and the final test summary.\n\nFINDINGS:\n${JSON.stringify(all, null, 1)}`,
    { label: `fix:${t.key}`, phase: 'Fix' })
}

// Promises are created lazily and memoised, so the order of plan.tasks does not matter.
const P = new Map()
function start(k) {
  if (P.has(k)) return P.get(k)
  const n = NODES.get(k)
  let p
  if (n.kind === 'spec') {
    const s = n.item
    p = agent(`${CTX}\n\n${DIRTY}\n\nWrite ${s.files}.\n${s.task}\nReturn a short summary and the facts you confirmed vs corrected.`,
      { label: `spec:${s.key}`, phase: 'Specs' }).catch(e => ({ error: String(e) }))
  } else {
    const t = n.item
    p = Promise.all(t.deps.map(d => start(d).then(r => [d, r])))
      .then(async deps => {
        const bad = deps.filter(([, r]) => failed(r)).map(([d]) => d)
        if (bad.length) return { skipped: `dependency failed or was skipped: ${bad.join(', ')}` }
        const impl = await agent(`${CTX}\n\n${CLEAN}\n\nYou own ${t.owns}. Specs: ${t.spec || '(design task, no spec)'}.\n${t.task}`,
          { label: `build:${t.key}`, phase: 'Build' })
        return { impl, review: await reviewFix(t, impl) }
      })
      .catch(e => ({ error: String(e) }))
  }
  P.set(k, p)
  return p
}
const results = {}
for (const k of NODES.keys()) results[k] = await start(k)
const problems = Object.entries(results).filter(([, r]) => failed(r)).map(([k, r]) => `${k}: ${r == null ? 'no result' : r.error || r.skipped || 'implementer returned nothing'}`)
if (problems.length) log(`${M}: failed/skipped: ${problems.join('; ')}`)
const summary = Object.fromEntries(Object.entries(results).map(([k, r]) => [k,
  failed(r) ? { status: 'FAILED/SKIPPED', detail: r == null ? null : (r.error || r.skipped || 'implementer returned nothing') }
    : { status: 'done', impl: typeof r === 'object' && 'impl' in r ? r.impl : r, review: r.review }]))

phase('Exit')
const exit = await agent(`${CTX}\n\n${CLEAN_READ}\n\nFINAL EXIT-CRITERIA VERIFIER for ${M} (docs/ROADMAP.md). Check every ${M} exit criterion with real evidence: commands and numbers, and screenshots viewed with Read for visual criteria. Criteria that need the owner's observation in the original client: verify everything data-side, mark them "data-side done, owner observation pending" and make sure they are listed in docs/checklists/owner-observations.md. Also run: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (with TESSERA_GAME_DIR when available); a 60 s fuzz smoke run of new targets; tools/check-content.sh; tools/check-spdx.sh; doc link check. FIX failures in code, tests, tools and non-spec docs; for a spec problem add a "> **OPEN:**" note or report it, do not change spec facts. Tasks that FAILED or were SKIPPED below did not run: their criteria are not met unless you can show otherwise; report them. Update the ${M} status table in docs/ROADMAP.md, the README status, and the "Current state" section of CLAUDE.md. Return the criterion table with evidence and remaining issues.

PER-TASK RESULTS (implementer summaries, reviews, failures):
${JSON.stringify(summary, null, 1)}

OWNER OBSERVATIONS the planner listed (must appear in docs/checklists/owner-observations.md):
${JSON.stringify(plan.owner_observations || [], null, 1)}`,
  { label: `exit:${M}`, phase: 'Exit' })

return { plan, results, exit }
