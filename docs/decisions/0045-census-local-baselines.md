<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0045 — Real-data census counts are checked against local per-install baselines

**Status:** Accepted (owner, 2026-10-31).

Refines [decision 0037](0037-p6-trait-tree-and-recipe-census.md) §1 item 5 ("counts are
evidence, not constants") and [decision 0036](0036-p5-map-census-and-panel-goldens.md)'s rule
of the same words, and applies the local-only scheme of
[decision 0031](0031-parity-panel-goldens.md) tier 2 (and its 2026-10-31 amendment) to counts.

## Context
Real-data tests read the user's own install and count what they find: records per class,
instruction and call sites, census categories, tooltips laid out. Many of them pinned those
counts in the test or compared them with the census tables of the specs. After a game update
on 2026-10-31, 22 real-data tests failed only because such counts changed (for example Skill
records 13,250 → 13,622, collision mesh records 47,570 → 51,073, the economy and instance
census tables, and the class library's code length L 1,919,916 → 1,921,992). No rule was
broken: nothing failed to decode, nothing was misclassified. Every update would repeat this,
and editing the numbers in the tests each time hides real regressions among data changes.

The specs' numbers are still useful: they are what a spec writer observed on a named install,
and a spec writer verifying a table needs to compare with them.

## Decision
1. **Counts versus rules.** A *count* is a number the install decides (records, sites,
   categories, offsets of a data item). A *rule check* is a property every install must have
   (0 failures, 0 misclassified, every record visited once, an invariant between two counts,
   a decoder rule). Rule checks stay strict assertions. Counts are checked against a local
   baseline of the install version, never against a number written in the test.
2. **Location.** `${TESSERA_CENSUS_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/census}/<package>/<test binary>/<function>/<install version>.tsv`
   (name, value, spec value). The install version is the one of decision 0031: the short
   level-1 content fingerprint, else `schema-` and the short level-0 one. A directory inside
   a git work tree is refused. Baselines, like tier-2 goldens, are local only and never
   committed.
3. **Baseline present.** Every recorded name must match it exactly; a missing or extra name,
   or any differing value, fails with a table of name, baseline and observed.
4. **No baseline yet** (the first run on an install version). The baseline is written
   atomically and the census passes. A drift report is printed against the newest previous
   install version's baseline of the same test (the most recently modified sibling baseline),
   or, without one, against the spec values the test gives (name, previous, observed, spec).
   The drifted rows are kept in `<base>/drift/<install version>/<package>/<test>/<function>.tsv`:
   one file per test, written atomically, so parallel test binaries never share a file (the
   owner allowed one file per test or atomic appends to one file per version; one file per
   test was chosen, and `tools/census-drift.sh` reads them as one list).
   `<base>/drift/last-version` holds the version of the latest census. Drift is information,
   not a failure.
5. **Spec numbers stay in the tests**, beside the count they describe, as the values the
   drift report shows and strict mode checks.
6. **Switches.** `TESSERA_CENSUS_STRICT=1` additionally fails when an observed value differs
   from its spec value (for spec writers verifying a table). `TESSERA_CENSUS_REBASE=1`
   rewrites the baseline of the current install version, after the owner or a builder has
   accepted a change seen on that version.
7. **Logs** carry names, numbers and paths only, never record content.
8. **Helper.** One dev-only workspace crate, `tessera-testkit` (`crates/testkit`,
   `publish = false`, only ever a dev-dependency), holds the helper (`Census`), and
   `tools/census-drift.sh` prints the drift rows of one install version (version lookup as in
   `tools/golden-review.sh`).

## Consequences
- After a game update the gate passes unless a rule check fails; the counts that moved are
  listed by `tools/census-drift.sh`. Nothing needs approval for counts alone.
- A count that changes **without** an install update fails: a regression in Tessera's own
  code is still caught on a machine that has run the test before.
- A first run on a machine (or a new install version) cannot catch a regression in a count;
  it is compared with the previous version or the spec only as a report. Rule checks still
  apply.
- Spec census tables are checked only under `TESSERA_CENSUS_STRICT=1`; a spec writer
  updating a table runs it so.
- Tests that check counts take `tessera-testkit` as a dev-dependency. The crate links
  `tessera-install` and `tessera-dat` for the install version; no shipped binary links it.
- Decisions 0036 and 0037 keep their rules; the exit verifiers' comparisons with the spec
  counts on the owner's install are the strict-mode run.

## Implementation notes (2026-10-31, after review)
Engineering choices inside the decision, not changes to it:
- The baseline key is computed only from readers that succeed: an archive whose header check
  fails or that does not open, or a lookup, read or parse of a version record that fails,
  fails the census; only a record that is absent is an input of the fingerprint. So a
  dat-layer regression on an unchanged install cannot move every census to a new key that
  passes. The "no baseline yet" message lists each archive's input.
- A test finishes its census after its last rule check, so a run that breaks a rule writes
  no baseline (a panic before `finish` writes nothing).
- Tests that sample in debug builds record no census in a sampled run, so the build profile
  never decides the baseline.
- A census directory at or under the game install is refused, as one inside a work tree is.

## Open
- The tests that pinned counts are converted one by one; until a test is converted it keeps
  its pinned numbers.
