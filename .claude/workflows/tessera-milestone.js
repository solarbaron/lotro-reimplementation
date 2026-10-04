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
//     cache: "/home/<user>/.cache/tessera",       // scratch + research clones live here
//     notes: "optional extra instructions, e.g. decisions to honour" } })
// See docs/DEVELOPMENT.md ("Running a milestone").

export const meta = {
  name: 'tessera-milestone',
  description: 'Run one Tessera roadmap milestone: plan, dirty-room specs, clean-room build with review/fix, exit check',
  whenToUse: 'Continuing the roadmap (docs/ROADMAP.md) one milestone at a time; pass args.milestone like "M4"',
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
const M = args.milestone
const REPO = args.repo
const GAME = args.game || null
const SCRATCH = `${args.cache}/scratch/${M}`
const CLONES = `${args.cache}/clones`
const NOTES = args.notes || ''

const CTX = `Repo: ${REPO} (Rust workspace "Tessera": clean-room client compatible with LOTRO data files). ${REPO}/CLAUDE.md is binding (clean-room zones, content rules, conventions). Decisions: ${REPO}/docs/decisions/ (binding). Roadmap: ${REPO}/docs/ROADMAP.md — we are working on milestone ${M}; earlier milestones' status tables say what exists. Read the public APIs of existing crates before adding to them.
${GAME ? `The user's game install is READ-ONLY at "${GAME}" — never write there; set TESSERA_GAME_DIR to it for real-data tests.` : 'No game install is available on this machine: real-data tests are skipped; say clearly which criteria could not be verified.'}
Scratch dir for throwaway files: ${SCRATCH} (create it). Delete large throwaway outputs when done. Do not create extra cargo target dirs.
Concurrency: other agents edit other crates at the same time. Only edit files you own. Add dependencies to YOUR crate's Cargo.toml only (workspace = true for crates in root [workspace.dependencies]); do NOT edit the root Cargo.toml (the planner already did). Fuzz targets: only append your own [[bin]] entries to fuzz/Cargo.toml. Never git commit.
${NOTES ? 'Extra instructions from the owner: ' + NOTES : ''}`

const DIRTY = `You are a SPEC WRITER (dirty room). You MAY read ${REPO}/research/ and third-party sources cloned under ${CLONES} (run ${REPO}/tools/fetch-research-clones.sh "${CLONES}" first if it is empty), and probe the real data with throwaway scripts in ${SCRATCH}. Output prose + tables of functional facts only: no copied code or transliterated pseudo-code, no game text (refer to records by DID). Follow the style of existing ${REPO}/docs/specs/*.md: provenance table with Confirmed/Reported, numbered sections with <a id> anchors, byte layouts, algorithms in words, invariants, "> **OPEN:**" callouts. Confirm layouts over ALL records of each type with statistics where data is available.`

const CLEAN = `You are a CLEAN-ROOM IMPLEMENTER: do NOT read ${REPO}/research/, ${CLONES}, third-party LOTRO/AC/DDO source code, or look formats up online. Work only from ${REPO}/docs/specs/ and the existing crates. A missing fact means: implement conservatively and add a "> **OPEN:**" note to the spec. Requirements: no panics on hostile input, bounded allocation, thiserror errors, rustdoc on public API, synthetic unit tests, real-data tests gated on TESSERA_GAME_DIR (run them when available; --release for heavy ones), SPDX headers, cargo fmt + cargo clippy -p <crate> --all-targets -- -D warnings clean, a fuzz target for every new decoder (run 60 s each with tools/fuzz-smoke.sh -t 60 <target>). Finish with: public API summary, test summary lines, OPEN items.`

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
const plan = await agent(`${CTX}\n\nYou are the MILESTONE PLANNER for ${M}. Read docs/ROADMAP.md (the ${M} section: deliverables, specs needed, RE tasks, exit criteria), docs/ARCHITECTURE.md, docs/decisions/, and the existing workspace. Then:
1. Create skeleton crates/apps the milestone needs that do not exist yet (Cargo.toml using workspace package fields + thiserror/tracing deps + [lints] workspace = true; src/lib.rs with SPDX header), and register them in the root Cargo.toml members and [workspace.dependencies]. Run cargo check --workspace.
2. Emit a plan: specs (dirty-room writing tasks, each naming the files it writes under docs/specs/ or docs/checklists/), and implementation tasks with non-overlapping 'owns' paths and dependency keys (a task may depend on spec keys and other task keys). Prefer 4–8 tasks; split by crate. Every exit criterion must be covered by some task. Decision records required by the roadmap go to docs/decisions/NNNN-*.md (+ index row in docs/decisions/README.md) via a task.
3. List exit criteria that only the project owner can observe in the original client (owner_observations); they go to docs/checklists/owner-observations.md.
Do not implement features yourself.`, { label: `plan:${M}`, phase: 'Plan', schema: PLAN_SCHEMA })

if (!plan) throw new Error('planner failed')
log(`${M}: ${plan.specs.length} specs, ${plan.tasks.length} tasks`)

async function reviewFix(t) {
  const lenses = [
    ['correctness', `Correctness against ${t.spec || 'the ' + M + ' roadmap deliverables'}: layouts, algorithms, conventions, edge cases. Write throwaway tests (also against the real install when available); take and view screenshots for visual features.`],
    ['robustness', `Hostile input and runtime safety: truncation, huge counts, overflow, unbounded allocation, unwrap/expect/indexing on untrusted data, resource leaks, main-thread stalls, races and thread-safety.`],
    ['quality', `API design, idiomatic Rust, docs, test gaps, CLAUDE.md compliance (SPDX headers, tracing, no game data or text in the repo, clean-room provenance), clippy -D warnings, fit for later milestones.`],
  ]
  const fs = await parallel(lenses.map(([k, p]) => () => agent(`${CTX}\n\nYou are a skeptical REVIEWER of ${t.owns} (do not edit those files; put throwaway tests in ${SCRATCH} or remove them afterwards). Lens: ${p}\nReport only problems you verified, with file:line, a concrete failing scenario and a suggested fix.`,
    { label: `review:${t.key}:${k}`, phase: 'Review', schema: REVIEW_SCHEMA })))
  const all = fs.filter(Boolean).flatMap(f => f.findings)
  if (!all.length) return 'no findings'
  return agent(`${CTX}\n\n${CLEAN}\n\nYou own ${t.owns}. Verify each finding below; fix real ones with a regression test; explain rejections. Re-run fmt, clippy -D warnings and tests. Return a finding -> fixed/rejected table and the final test summary.\n\nFINDINGS:\n${JSON.stringify(all, null, 1)}`,
    { label: `fix:${t.key}`, phase: 'Fix' })
}

const P = {}
for (const s of plan.specs) {
  P[s.key] = agent(`${CTX}\n\n${DIRTY}\n\nWrite ${s.files}.\n${s.task}\nReturn a short summary and the facts you confirmed vs corrected.`,
    { label: `spec:${s.key}`, phase: 'Specs' }).catch(e => ({ error: String(e) }))
}
for (const t of plan.tasks) {
  const deps = t.deps.filter(d => d !== t.key)
  P[t.key] = Promise.all(deps.map(d => P[d] || Promise.resolve(null)))
    .then(() => agent(`${CTX}\n\n${CLEAN}\n\nYou own ${t.owns}. Specs: ${t.spec || '(design task, no spec)'}.\n${t.task}`,
      { label: `build:${t.key}`, phase: 'Build' }))
    .then(async impl => ({ impl, review: await reviewFix(t) }))
    .catch(e => ({ error: String(e) }))
}
const results = {}
for (const k of Object.keys(P)) results[k] = await P[k]

phase('Exit')
const exit = await agent(`${CTX}\n\nFINAL EXIT-CRITERIA VERIFIER for ${M} (docs/ROADMAP.md). Check every ${M} exit criterion with real evidence: commands and numbers, and screenshots viewed with Read for visual criteria. Criteria that need the owner's observation in the original client: verify everything data-side, mark them "data-side done, owner observation pending" and make sure they are listed in docs/checklists/owner-observations.md. Also run: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (with TESSERA_GAME_DIR when available); a 60 s fuzz smoke run of new targets; tools/check-content.sh; tools/check-spdx.sh; doc link check. FIX failures (any file). Update the ${M} status table in docs/ROADMAP.md, the README status, and the "Current state" section of CLAUDE.md. Return the criterion table with evidence and remaining issues.`,
  { label: `exit:${M}`, phase: 'Exit' })

return { plan, results, exit }
