// SPDX-License-Identifier: GPL-3.0-or-later
//
// Follow-up workflow for Claude Code (Workflow tool): builds independent work packages in
// parallel, one prepared git worktree per package, outside the milestone workflow.
//   clean-impl package: clean implementer -> reviewer(s) -> fixer -> clean-room audit
//   spec-work package:  spec writer -> contamination screen -> independent review
//                       (CLAUDE.md, CLEAN_ROOM.md §5.3), revised by fresh sessions, <= 3 rounds
// The orchestrating session prepares the worktrees BEFORE the run (one branch each; a clean
// package's worktree has a sparse checkout without research/), gives
// each its own .cargo/config.toml target-dir on the main drive, and afterwards reviews,
// commits and merges, running the full gate once. The workflow never commits.
//
//   Workflow({ name: "tessera-followups", args: {
//     repo: "/abs/repo", game: "/abs/install or omitted", cache: "/home/<user>/.cache/tessera",
//     max_parallel: 6,              // optional: packages in flight at once (default: all)
//     max_clean: 3,                 // optional: clean packages in flight (default: max_parallel)
//     packages: [{ key, kind: "clean-impl" | "spec-work", wt: "/abs/worktree", owns, specs,
//                  task }] } })
// Disk: one target dir PER clean worktree (never shared: cargo names workspace artifacts
// relative to the workspace root and judges freshness by mtime, so worktrees sharing a dir link
// each other's code). incremental = false (sccache shares dependency builds) and debug = 0 keep
// each dir near 20 GB; size max_clean so that 25 GB per clean package in flight still leaves
// 40 GB free, and delete each dir after its branch is merged.
// Never use it for M8 / official-protocol work (incident 0041) or M9.

export const meta = {
  name: 'tessera-followups',
  description: 'Build independent follow-up packages in parallel worktrees: clean build/review/fix/audit, or spec revision with independent review',
  whenToUse: 'Open items outside a milestone run (ROADMAP/CLAUDE.md open lists, implementer OPENs answered, owner decisions to apply); one prepared worktree per package',
  phases: [
    { title: 'Build', detail: 'clean implementers, one per package worktree' },
    { title: 'Spec', detail: 'spec revision, contamination screen, independent review' },
    { title: 'Review', detail: 'skeptical reviewers per package' },
    { title: 'Fix', detail: 'apply verified findings' },
    { title: 'Audit', detail: 'clean-room and content audit of each package diff' },
  ],
}

if (!args || !args.repo || !args.cache || !Array.isArray(args.packages) || !args.packages.length) {
  throw new Error('args.repo, args.cache and a non-empty args.packages array are required (see header)')
}
const REPO = args.repo
const GAME = args.game || null
const keys = new Set()
for (const p of args.packages) {
  for (const f of ['key', 'kind', 'wt', 'owns', 'task']) if (!p[f]) throw new Error(`package ${p.key || '?'}: missing ${f}`)
  if (!['clean-impl', 'spec-work'].includes(p.kind)) throw new Error(`package ${p.key}: kind must be clean-impl or spec-work`)
  if (keys.has(p.key)) throw new Error(`duplicate package key ${p.key}`)
  if (/compat|official|M8|M9/i.test(`${p.owns} ${p.specs || ''}`)) throw new Error(`package ${p.key}: M8/official-protocol and M9 work is not run here (incident 0041, decision 0024)`)
  keys.add(p.key)
}

const CTX = p => `Repo: ${REPO} (Rust workspace "Tessera": clean-room client compatible with LOTRO data files); CLAUDE.md is binding. YOUR WORKING TREE IS ${p.wt} (its own branch; start every shell command with cd ${p.wt}); never edit ${REPO} itself or another worktree. You own ${p.owns}; edit nothing else, except: a spec's OPEN notes, and fuzz/Cargo.toml [[bin]] entries you append. Do not edit docs/ROADMAP.md, README.md or CLAUDE.md (the orchestrator updates status at merge); report status changes in your answer instead. Never git commit, reset, stash, clean or checkout. The cargo target dir comes from ${p.wt}/.cargo/config.toml and belongs to this package only: never change it, never set CARGO_TARGET_DIR or --target-dir, and never create a build directory anywhere else (each one costs about 20 GB). If cargo prints "Blocking waiting for file lock", another agent of this package is building: wait (use long Bash timeouts). Before each build check df -h /: with less than 25 GB free, stop and report instead of building. Test only the crates you changed (cargo test -p <crate>; --release for real-data tests); read test output through its summary lines. ${GAME ? `The game install is READ-ONLY at "${GAME}": set TESSERA_GAME_DIR to it for real-data tests; never write there.` : 'No game install: real-data tests skip; say so.'} Scratch dir: ${args.cache}/scratch/followups/${p.key}/clean (dirty roles: .../dirty).`

const CLEAN_READ = `CLEAN-ROOM RULES (CLAUDE.md): do NOT open ${REPO}/research/, ${args.cache}/clones, any */dirty scratch dir, ~/tessera-re, ~/.rea, decompiler output or third-party game-tool source; do not look formats up online; run no disassembler or decompiler. Work only from docs/ and crates/ apps/ fuzz/ tools/. Build only from specs whose status is Accepted (with an independent review record); if a fact is missing or a spec looks wrong, add a "> **OPEN:**" note to the spec and implement conservatively, never correct a spec's facts.`

const DIRTY = `You are a SPEC WRITER (dirty room, .claude/agents/spec-writer.md rules). You may read ${REPO}/research/ and ${args.cache}/clones and probe the install read-only with throwaway scripts in your dirty scratch dir. Prose and tables of functional facts only; never code, pseudo-code, copied prose, game text, keys or hostnames; never disassembly in prose (registers, offsets, vtable slots, internal names, memory layouts; incident 0041). No official-protocol or login research (paused, incident 0041). Leave each spec you change in Review (or Revised for an Accepted spec) with a change-log row: only an independent reviewer accepts it. Never repeat facts in your answer.`

const IMPL = { type: 'object', properties: {
  summary: { type: 'string', description: 'at most 10 lines: what changed' },
  files: { type: 'array', items: { type: 'string' } },
  diff_lines: { type: 'number' },
  tests: { type: 'string' },
  spec_refs: { type: 'array', items: { type: 'string' }, description: 'docs/specs/<file>.md#<section> for Spec-Ref trailers' },
  opens: { type: 'array', items: { type: 'string' } },
  status_notes: { type: 'array', items: { type: 'string' }, description: 'ROADMAP/CLAUDE.md status lines to change at merge' },
  not_done: { type: 'array', items: { type: 'string' }, description: 'task items left undone and why' } },
  required: ['summary', 'files', 'diff_lines', 'tests', 'spec_refs', 'opens', 'status_notes', 'not_done'] }
const REVIEW = { type: 'object', properties: { findings: { type: 'array', items: { type: 'object', properties: {
  severity: { type: 'string' }, location: { type: 'string' }, problem: { type: 'string' }, fix: { type: 'string' } },
  required: ['severity', 'location', 'problem', 'fix'] } } }, required: ['findings'] }
const FIX = { type: 'object', properties: { fixed: { type: 'array', items: { type: 'string' } },
  rejected: { type: 'array', items: { type: 'string' } }, tests: { type: 'string' } }, required: ['fixed', 'rejected', 'tests'] }
const AUDIT = { type: 'object', properties: { pass: { type: 'boolean' },
  findings: { type: 'array', items: { type: 'string' } } }, required: ['pass', 'findings'] }
const SCREEN = { type: 'object', properties: { clean: { type: 'boolean' },
  findings: { type: 'array', items: { type: 'string' }, description: 'section + kind of sign; never quote it' } }, required: ['clean', 'findings'] }
const SPEC_REVIEW = { type: 'object', properties: { accepted: { type: 'boolean' },
  blocking: { type: 'array', items: { type: 'string' } }, record: { type: 'string' } }, required: ['accepted', 'blocking', 'record'] }
const SMALL_DIFF = 800

async function cleanPackage(p) {
  const label = `build:${p.key}`
  const impl = await agent(`${CTX(p)}\n\nYou are a CLEAN-ROOM IMPLEMENTER. ${CLEAN_READ} Requirements: no panics on hostile input, bounded allocation, thiserror errors, rustdoc on public API, synthetic tests, SPDX headers, cargo fmt and cargo clippy -p <crate> --all-targets -- -D warnings clean, a fuzz target for every new parser (60 s smoke with tools/fuzz-smoke.sh -t 60 <target>). Specs: ${p.specs || '(see task)'}.\n\nTASK:\n${p.task}`,
    { label, phase: 'Build', agentType: 'clean-implementer', schema: IMPL })
  if (!impl) return { key: p.key, error: 'implementer returned nothing', provenance: [label] }
  const lenses = [
    ['correctness', `against the specs (${p.specs || 'task'}): layouts, algorithms, edge cases, task items silently skipped; take and view screenshots for visual changes`],
    ['robustness', 'hostile input, overflow, unbounded allocation, unwrap/indexing on untrusted data, main-thread stalls, races'],
    ['quality', 'API design, idiomatic Rust matching the surrounding code, docs, test gaps, CLAUDE.md compliance, clippy'],
  ]
  const ask = (k, lens) => agent(`${CTX(p)}\n\n${CLEAN_READ}\n\nYou are a skeptical REVIEWER of the uncommitted changes in ${p.wt} (git -C ${p.wt} diff; new files via git status). Edit nothing; throwaway tests go in your scratch dir. Implementer's summary: ${impl.summary} Tests run: ${impl.tests}. Lens: ${lens}. Report only problems you verified, with file:line and a concrete failing scenario.`,
    { label: `review:${p.key}:${k}`, phase: 'Review', schema: REVIEW })
  const reviews = (impl.diff_lines || 0) <= SMALL_DIFF
    ? [await ask('all', lenses.map(([k, l]) => `(${k}) ${l}`).join('; '))]
    : await parallel(lenses.map(([k, l]) => () => ask(k, l)))
  const findings = reviews.filter(Boolean).flatMap(r => r.findings || [])
  const prov = [label, ...reviews.map((r, i) => r ? `review:${p.key}:${reviews.length === 1 ? 'all' : lenses[i][0]}` : null).filter(Boolean)]
  let fix = null
  if (findings.length) {
    fix = await agent(`${CTX(p)}\n\nYou are a CLEAN-ROOM IMPLEMENTER fixing review findings. ${CLEAN_READ} Verify each finding; fix real ones with a regression test; explain rejections. Re-run fmt, clippy -D warnings and the tests of the crates you changed.\n\nFINDINGS:\n${JSON.stringify(findings, null, 1)}`,
      { label: `fix:${p.key}`, phase: 'Fix', agentType: 'clean-implementer', schema: FIX })
    prov.push(`fix:${p.key}`)
  }
  const audit = await agent(`Audit the uncommitted changes in the worktree ${p.wt} (git -C ${p.wt} diff HEAD and untracked files) in default mode: spec traceability to Accepted specs (${p.specs || 'see task'}), contamination signs, game data, trademark names, SPDX headers; run tools/check-content.sh and tools/check-spdx.sh there. pass=false only for a rule violation.`,
    { label: `audit:${p.key}`, phase: 'Audit', agentType: 'clean-room-auditor', schema: AUDIT })
  prov.push(`audit:${p.key}`)
  return { key: p.key, kind: p.kind, impl, findings: findings.length, fix, audit, provenance: prov }
}

async function specPackage(p) {
  const prov = []
  let issues = null
  for (let round = 1; round <= 3; round++) {
    const w = `spec:${p.key}:${round}`
    const wrote = await agent(`${CTX(p)}\n\n${DIRTY}\n\n${round === 1 ? `TASK:\n${p.task}` : `Revise the spec(s) you are given (${p.owns}) for the task "${p.task}" to resolve every issue below; keep section numbers stable.\n\nISSUES:\n${JSON.stringify(issues, null, 1)}`}\nReturn one line only.`,
      { label: w, phase: 'Spec', agentType: 'spec-writer' })
    prov.push(w)
    if (wrote == null) return { key: p.key, kind: p.kind, error: 'spec writer returned nothing', provenance: prov }
    const s = `screen:${p.key}:${round}`
    const screen = await agent(`${CLEAN_READ}\n\nYou are a CONTAMINATION SCREENER (read-only). Read only the changed spec text in ${p.wt} (git -C ${p.wt} diff; files ${p.owns}). Report signs of dirty material: code or pseudo-code, disassembly in prose (registers, offsets, vtable slots, call sequences, internal names, memory layouts), copied prose, game text, keys, hostnames, account data. Name section and kind; never quote it.`,
      { label: s, phase: 'Spec', agentType: 'clean-room-auditor', schema: SCREEN })
    prov.push(s)
    const r = `spec-review:${p.key}:${round}`
    const review = await agent(`${CTX(p)}\n\n${DIRTY}\n\nYou are an INDEPENDENT SPEC REVIEWER (CLEAN_ROOM.md §5.3). You wrote none of the changes in ${p.wt} to ${p.owns}; do not write facts. Check every §5.3 item on the changed text, spot-check facts against cited sources and the install, and treat the contamination-screen findings below as blocking. If all hold and the screen is clean, set the status to Accepted and write the review record (reviewer "${r} (independent session)", date from \`date +%F\`, result) in the header; otherwise list blocking issues.\n\nSCREEN: ${JSON.stringify(screen)}`,
      { label: r, phase: 'Spec', agentType: 'spec-writer', schema: SPEC_REVIEW })
    prov.push(r)
    if (!screen || !review) return { key: p.key, kind: p.kind, error: 'screen or reviewer failed', provenance: prov }
    if (review.accepted && screen.clean) return { key: p.key, kind: p.kind, accepted: true, record: review.record, rounds: round, provenance: prov }
    issues = [...(screen.findings || []).map(f => `screen: ${f}`), ...(review.blocking || [])]
  }
  return { key: p.key, kind: p.kind, accepted: false, issues_count: issues ? issues.length : 0, provenance: prov }
}

// Caps packages in flight (args.max_parallel) and clean packages in flight (args.max_clean:
// each holds its own ~20 GB target dir and builds). Queued packages start as others finish; a
// freed slot passes straight to the next waiter, so a cap is never exceeded.
const limiter = n => {
  let active = 0
  const waiting = []
  return async fn => {
    if (active >= n) await new Promise(r => waiting.push(r))
    else active++
    try { return await fn() } finally { const next = waiting.shift(); if (next) next(); else active-- }
  }
}
const MAX_PAR = Math.max(1, Number(args.max_parallel) || args.packages.length)
const MAX_CLEAN = Math.max(1, Number(args.max_clean) || MAX_PAR)
const slot = limiter(MAX_PAR)
const cleanSlot = limiter(MAX_CLEAN)
const run = p => p.kind === 'spec-work' ? slot(() => specPackage(p)) : cleanSlot(() => slot(() => cleanPackage(p)))
const results = await pipeline(args.packages, p => run(p).catch(e => ({ key: p.key, error: String(e) })))
const bad = results.filter(r => !r || r.error || (r.audit && !r.audit.pass) || r.accepted === false)
if (bad.length) log(`attention: ${bad.map(r => r ? r.key : '?').join(', ')}`)
return { results }
