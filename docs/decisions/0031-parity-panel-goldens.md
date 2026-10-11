<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0031 — Golden images of panels that show install content (parity track)

**Status:** Proposed, 2026-10-10 (waits for the owner).

The number 0031 is out of sequence on purpose. P2 runs in parallel on its own branch and takes
the next free numbers from 0026, so P3 takes a number after
[0030](0030-tessera-protocol-transport.md). The record serves P2, P3 and P6.

## Context
[Decision 0024](0024-parity-track.md) §5 sets "golden images pass" as an exit criterion three
times: P2 criterion 1 (a combat scenario with every element on screen), P3 criterion 2
(dialogue, vendor and loot scenarios that render with install text and icons) and P6
criterion 2 (a progression scenario). Principle 6 of §2 asks each milestone for real-data
tests that read `TESSERA_GAME_DIR` and skip when it is unset.

The panels in question show what the install provides: layouts, text, fonts and icons. Their
pixels are game data. CLAUDE.md's content rules forbid committing extracted textures, strings,
fonts and similar material, and [ARCHITECTURE §23.3](../ARCHITECTURE.md#testing-golden) already
allows real-data visual regression only as a local tier whose images stay in the user's own
cache. That tier has no rules yet: where the images go, how the install version is kept apart,
what a first run without an image reports, and what an exit verifier has to do before writing
"golden images pass".

The committed tier exists. `crates/render/tests/support/golden.rs` compares a render with a
PNG next to the test, with a per-channel tolerance, a neighbourhood search and a budget of
outlier pixels. `TESSERA_BLESS=1` rewrites the PNG. The scenes are synthetic.

`tools/check-content.sh` refuses game-data extensions and magic numbers, binary blobs over
1 MiB and LFS filters. It does not recognise a PNG made from install pixels, so it cannot
tell such a picture from a synthetic golden of the same size.

## Decision
Panel goldens come in two tiers. A panel may have goldens in both.

### 1. Committed synthetic goldens
- **Input:** synthetic only. Tessera's fallback widgets, generated or original CC0 art
  (third-party assets need their own decision, [0022](0022-bundled-ui-font.md)), the bundled
  Noto Sans ([0022](0022-bundled-ui-font.md)), and scenarios that use literal refs and run with
  no install (`install none`; [`scenario-feed.md`](../specs/scenario-feed.md) §10.5, §13). No
  install file is opened, so nothing in the picture can come from one.
- **Capture path:** in-process, inside the test: the scenario runs on `HeadlessDriver`
  (`scenario-feed.md` §12.3) and the test paints the resulting panel state itself. The client
  CLI is not used, because `tessera-client --scenario --no-data` refuses `--screenshot`
  (`scenario-feed.md` §12.2) and has no renderer.
- **Painting:** `tessera-ui`'s CPU painter (exact compositing per surface,
  [0015](0015-ui-rendering.md)), or the headless renderer when GPU composition is part of what
  is tested.
- **Comparison:** a per-pixel tolerance and a maximum ratio of differing pixels, with the rules
  of `crates/render/tests/support/golden.rs`. GPU images start from that helper's defaults.
  CPU-painter images start from a tighter tolerance, because the painter is exact: one step per
  channel, no neighbourhood search, no outliers. A test that loosens either tolerance says why
  in a comment.
- **Blessing:** `TESSERA_BLESS=1` rewrites the image and the test passes, as in `golden.rs`.
  Only tier 1 reads `TESSERA_BLESS`; tier 2 ignores it (below).
  Each image is checked by eye before it is committed. A test that fails on only one machine
  is a finding, not a reason to bless again (`docs/DEVELOPMENT.md` §2).
- **Location:** a `golden/` directory next to the test (as `crates/render/tests/golden/`).
  Images stay small, well below `check-content.sh`'s 1 MiB limit.

### 2. Local-only real-data goldens
- **Input:** the user's install through `TESSERA_GAME_DIR`, which gives panels with install
  layouts, text, fonts and icons. Without `TESSERA_GAME_DIR` the test prints a skip notice and
  returns successfully (ARCHITECTURE §23.2). Auto-detection is not used.
- **Location:** `${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/<test>/<install version>/<name>.png`.
  - `<test>` is `<package>/<test binary>/<function>`, for example
    `tessera-ui/ui_real_data/vendor_window`. The package comes from `CARGO_PKG_NAME`, because
    test binary names are unique only within one package (`ui_real_data` exists in both
    `tessera-ui` and `tessera-render`).
  - `<install version>` is the short level-1 content fingerprint of
    [`install-layout.md`](../specs/install-layout.md) §9.4 (`tessera-install`
    `ContentFingerprint`). When level 1 cannot be computed, the short level-0 schema
    fingerprint is used with the prefix `schema-`. A patch therefore starts a new set of
    goldens, and older sets stay as they are.
  - `<name>` names the captured state, for example `page-2` or `buyback`. When a test picks a
    language or a UI scale, the choice is part of `<name>`.
- **Never committed.** The directory is outside every git work tree. The golden helper
  canonicalises the nearest existing ancestor of the resolved directory (symlinks resolved)
  and refuses the directory, failing the test, when that path or any of its ancestors contains
  a `.git` entry (a directory, or the file a linked worktree has). This covers the current
  work tree, sibling worktrees of the same repository and any other repository, and it holds
  whether `TESSERA_GOLDEN_DIR` or `XDG_CACHE_HOME` pointed it there. The test never
  writes to the install directory either. On a mismatch the actual image and the diff image go
  next to the golden (`<name>.actual.png`, `<name>.diff.png`), never to `target/` or another
  path inside the tree. Goldens, actual and diff images are written atomically: to a
  temporary file in the same directory, then renamed over the target, so a concurrent reader
  or an interrupted run never sees a half-written PNG. Logs print paths and counts, never
  decoded text.
- **First run:** when the golden is missing, the test writes it, prints
  `golden: blessed <path>` on stderr and returns successfully. That run counts as "blessed",
  not "passed". `TESSERA_GOLDEN_BLESS_LOCAL=1` rewrites existing tier-2 images the same way.
  Tier 2 does not read `TESSERA_BLESS`, and tier-1 checks do not read
  `TESSERA_GOLDEN_BLESS_LOCAL`, so re-blessing local goldens never rewrites a committed PNG.
  `TESSERA_GOLDEN_REQUIRE=1` makes a missing golden a failure instead, so a matching run can be
  told apart from a first run. When `TESSERA_GOLDEN_REQUIRE` and `TESSERA_GOLDEN_BLESS_LOCAL`
  are both set, the helper refuses the combination and the test fails without writing.
- **Passing:** a golden passes only when a later run matches it within the tolerance. The
  tolerance is the same as in tier 1 for the same painter.
- **Determinism:**
  - The scene runs on the scenario feed's virtual clock
    ([`scenario-feed.md`](../specs/scenario-feed.md) §7.1), never the wall clock, so the
    capture happens at a fixed virtual time. Through the client CLI that is the one frame after
    the last tick (`scenario-feed.md` §12.2); several captures per run wait for the OPEN on
    capture times in `scenario-feed.md` (P2, `at T capture NAME`). A test that needs another
    state uses another scenario, or captures in-process.
  - Fonts are fixed: the install's fonts ([`fonts.md`](../specs/fonts.md)) and the bundled
    Noto Sans. No system fonts are used.
  - The viewport size, UI scale and settings are fixed. Settings come from defaults or a
    temporary file, never the user's settings.
  - The captured frame has no time-dependent animation. Caret blink, fades, tooltip delays,
    cooldown sweeps and particle phases are either finished or frozen at a fixed time.
  - The capture waits until the asset workers have no requests pending for the panel, so no
    picture misses parts that are still loading. A capture taken because a wall-clock limit
    ran out (the client's `--settle-timeout`, `scenario-feed.md` §12.2, or a test's own limit)
    is a test failure: it is never written as a golden and never compared as one.
  - Two runs on the same machine and install must produce the same image within the
    tolerance. A test that cannot promise this does not use this tier.

### 3. What "golden images pass" means for an exit verifier
For a milestone criterion that asks for golden images (P2 1, P3 2, P6 2):
1. Tier 1 goldens for the panels involved are committed and pass in the workspace test run.
2. **Bless once.** With `TESSERA_GAME_DIR` set, run the real-data golden tests with
   `TESSERA_GOLDEN_DIR` set to a fresh directory of this verification alone (for example under
   the milestone's scratch dir, outside every work tree). Every image reports `blessed`. The
   shared default cache is not used, because parallel branches and worktrees bless into it.
   `TESSERA_BLESS` stays unset, so committed tier-1 goldens are not touched.
3. **View and judge.** Open each blessed image and judge it against the spec of the panel:
   layout, text, icons, states and the elements the criterion names. Record the verdict per
   image in the milestone's status table or checklist, by test and name. Never record the
   picture or its text there. A wrong image is a failure, even when it is stable.
4. **Run again and match.** Run the same tests with the same `TESSERA_GOLDEN_DIR` as step 2,
   with `TESSERA_GOLDEN_REQUIRE=1` and without `TESSERA_BLESS` or
   `TESSERA_GOLDEN_BLESS_LOCAL`. Every image must match its golden.

The criterion passes only after steps 1–4. The status records the install version used and
that steps 2 and 4 used the same golden directory.
Comparison with the original client stays an owner row (`docs/checklists/owner-observations.md`)
and is not part of this criterion.

## Consequences
- P2, P3 and P6 can claim "golden images pass" on real install content without committing
  that content. Every machine and install version keeps its own goldens, so a patch or a
  second machine starts with a blessing run.
- Real-data goldens catch regressions on one machine only; CI sees tier 1 alone. Each panel
  that has a real-data golden should also have a synthetic one.
- A shared golden helper is needed outside `tessera-render`. It handles the local directory
  and its `<package>/<test binary>/<function>` key, the work-tree refusal, atomic writes,
  `blessed` reporting, `TESSERA_GOLDEN_REQUIRE`, `TESSERA_GOLDEN_BLESS_LOCAL`, and comparing CPU
  canvases without a GPU. The implementing milestone puts it beside `golden.rs` or in a
  test-support module, included by path in the same way.
- `docs/DEVELOPMENT.md`'s environment table should list `TESSERA_GOLDEN_DIR`,
  `TESSERA_GOLDEN_REQUIRE` and `TESSERA_GOLDEN_BLESS_LOCAL` once a test uses them.

## Open
- `tools/check-content.sh` accepts any PNG under 1 MiB. Nothing but the location rule above
  keeps an install-derived golden out of a commit. A follow-up for the owner of `tools/` is a
  rule that refuses PNG files outside the committed golden directories (`*/tests/golden/`,
  today in `crates/render`, `crates/terrain` and `apps/client`) or under any path segment
  named `goldens`.
- The owner accepts this record or asks for changes.
