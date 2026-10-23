<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0036 — P5: the map-region census and golden images of the P5 panels

**Status:** Accepted (owner, 2026-10-16). Proposed 2026-10-11.

> **Review note (spec reviewer, Clean-Room-Role: review, 2026-10-13).** The map half of this
> record rests on `map-windows.md` §5 (the "every map region" definition §5.1, the counts §5.2,
> the categories §5.3 and the census procedure §5.4) and §6.7 (feed markers). That spec passed an
> independent CLEAN_ROOM.md §5.3 review on 2026-10-13 and is now **Accepted** with §5 and §6.7
> unchanged, so §1 of this record stands on a reviewed definition (the Open bullet on a review
> changing §5.1/§5.3 is resolved for the map part). The status stays **Proposed**: owner
> acceptance of the record itself, and the §2 tier-1 world-map golden (which waits for the
> fallback-window rule still OPEN in `map-windows.md` §8), are unchanged. The travel and instance
> halves of §1–§2 rest on `travel-windows.md` and `instances.md`, reviewed separately.

> **Review note (spec reviewer, Clean-Room-Role: review, 2026-10-13).** The scenario-feed
> mechanism this record relies on — the `map-region` family and the `markers` step with marker
> sets (`scenario-feed.md` revision 8 ("P5 travel"), §18.P5 T-D7), cited in §1.3 and the Consequences, and the
> travel and instance forms behind the §2 goldens — passed an independent CLEAN_ROOM.md §5.3
> review on 2026-10-13 and its P5 parts are now **Accepted**; the `lua-plugin-api.md` §15.5 mount
> views behind the criterion-2 presentation are likewise Accepted. So the feed half of §1–§2
> rests on a reviewed definition. The status stays **Proposed**: owner acceptance of the record
> itself is unchanged, and §1's `map-region` row and the §2 world-map tier-1 golden still carry
> the dependencies the map-half note records (the `travel-windows.md`/`instances.md` reviews and
> the still-OPEN fallback-window rule). Acceptance of the record is the owner's call, and this
> record stands on decision 0031 (still Proposed); a reviewer does not move either.

> **Review note (travel and instance census — spec reviewer, Clean-Room-Role: review, 2026-10-13).**
> The travel and instance halves that the map-half note left "reviewed separately" now rest on
> reviewed specs: the travel-destination census (`travel-windows.md` §8.1), the private-encounter
> template census (§8.2) and the finder definitions (`instances.md` B1–B8) behind §1–§2 and the
> §2 travel and instance-finder goldens. `travel-windows.md` passed an independent CLEAN_ROOM.md
> §5.3 re-review on 2026-10-13 and is now **Accepted** — the §8.1 `OtherReference`/`Unreferenced`
> reconciliation to 5 / 0 confirmed by an own read-only M1 probe of the owner's install — and
> `instances.md` is already Accepted. So the census definitions of §1–§2 stand on reviewed text.
> The record's own status stays **Proposed**: owner acceptance, and the §2 tier-1 world-map
> golden still waiting on the `map-windows.md` §8 fallback-window OPEN, are unchanged.

The number 0036 is out of sequence on purpose: P2 and P4 run in parallel on their own branches
and take numbers below it, so P5 starts at 0036.

## Context
[Decision 0024](0024-parity-track.md) §5 sets three exit criteria for P5 (map, travel, mounts
and instances):

1. The map panel renders every map region with scenario markers.
2. A mounted character walks in the walker with the install's mounted gaits.
3. Travel and instance-finder scenarios render.

Criterion 1 does not say what a "map region" is. The install has several things that look like
one: the nodes of the world map's tree, the maps of dungeons, map sets that only the minimap
uses, a map set named by an area that no traced path shows, minimap tiles, and the regions of
the region table. A test that "renders every map region" needs one list, a way to place a
marker on each entry, and a rule for entries that cannot be drawn. Decision 0024 §5 already
says how a census treats a record that cannot be shown: it is listed with a reason
(categorised), never a crash.

Criteria 1 and 3 ask for panels that "render". The panels show install content: layouts, map
images, text, fonts and icons. [Decision 0031](0031-parity-panel-goldens.md) (Proposed) set the
rules for golden images of such panels for P2, P3 and P6: committed synthetic goldens (tier 1),
local-only real-data goldens (tier 2), and what "golden images pass" means for an exit verifier
(§3). It does not name P5, whose criteria do not say "golden images pass" but still need a
visual check that the right content is drawn.

[`map-windows.md`](../specs/map-windows.md) §5 (status Review) gives the definition, the counts
on the owner's install, the categories of what cannot be shown and a census procedure.
[`travel-windows.md`](../specs/travel-windows.md) §8 (status Review) gives the census of travel
destinations and private-encounter templates. [`walker.md`](../specs/walker.md) §16.11 (status
Review) gives the check of criterion 2 from the walk log.

## Decision

### 1. "Every map region" (criterion 1)
1. **Definition.** A map region is what [`map-windows.md`](../specs/map-windows.md) §5.1
   defines: every node of the map tree (R-node, identified by its active element ID) and
   every distinct map-data set that a dungeon record names as its map (R-dungeon, identified
   by the set's DID). Not map regions: child-map targets that are no node, the map set used
   only by the minimap, the map set named only by an area, and minimap tiles (§5.1). The
   region table's regions are not map regions; they are an attribute of R-nodes.
2. **Census.** The list is built from the install at run time, never from counts or IDs
   written into a test. The census follows `map-windows.md` §5.4: every region, in its order,
   is shown in the full-screen presentation at UI scale 1, a scenario marker is placed at the
   region's test point, and the region is put in exactly one of six categories: C1 no element,
   C2 no image, C3 bad image, C4 no georeference, C5 frame outside the world (all §5.3), or
   "drawn and marked". C6 (frame partly outside, §5.3) is not a category of its own here: C6
   regions are drawn and marked normally, so they are in "drawn and marked" and are reported
   as a sub-count of it.
3. **Scenario markers.** The markers come from the scenario feed, not from code in the test:
   a scenario selects the family `map-region` and puts one marker on each element with the
   `markers` step (`scenario-feed.md`, revision 8, "P5 travel", §18.P5 T-D7). Feed markers follow
   `map-windows.md` §6.7.
4. **Pass.** Criterion 1 passes when the census visits every region exactly once
   (`map-windows.md` invariant MWI10), no region is a failure, and the counts per category are
   recorded in the P5 status. A **failure** is a region outside C1–C4 whose image is not drawn
   into the shown element, a region outside C1–C5 whose marker is not drawn centred within 1
   map pixel of the forward transform of its test point, or any region whose showing logs an
   error. C1–C4 regions are categorised (they cannot be drawn, with a reason) and are no
   failure; C5 regions must still be drawn (`map-windows.md` §5.4 steps 4–5) but carry no
   marker. The census runs headless on the CPU painter and needs no GPU.
5. **Counts are evidence, not constants.** The counts of `map-windows.md` §5.2 (1,525 regions
   on the owner's install of 2026-10-11) are what the exit verifier expects on that install
   and compares with; a later install may differ, and the test asserts the rules of item 4,
   not the numbers.

### 2. Golden images of the P5 panels (criteria 1 and 3)
Decision 0031's two tiers apply unchanged to these P5 panels:

| Panel | Tier 1 (committed, synthetic) | Tier 2 (local only, install) |
|---|---|---|
| World map with markers | Tessera's map window with a generated map image, notes and feed markers, from a scenario with literal refs and no install. Depends on a fallback map window that `map-windows.md` does not define yet (§8 OPEN); until that rule exists this golden cannot be made | A small fixed sample of regions, chosen by rules (tree position, first by ID), never by IDs written into the test: the root as an image without a marker (it is C5, and `map-windows.md` §6.6–§6.7 draw no marker on `WorldMap`), and the first territory map, one area map and one dungeon map, each with its marker |
| Travel window | The fallback window with literal rows: headers, a free row, a paid row, marks, an undiscovered and an unusable row, a selection | The install's travel layout with rows from install destinations and routes |
| Instance finder | The fallback finder page with literal entries: kind and category headers, a group row, size and tier menus, the queue page | The install's fellowship-maker page at the finder, with install templates |

The end-travel box, the loading-screen presentation, the invitation window and the
instance-travel window should have goldens too; criterion 3 does not wait for them.

- For criteria 1 and 3, "the panels render" requires, beside the census of §1 (criterion 1) and
  the censuses of `travel-windows.md` §8 with no `LayoutMissing` or `ElementMissing`
  (criterion 3), the steps of decision 0031 §3 for the panels above: tier-1 goldens committed
  and passing in the workspace run; tier-2 goldens blessed once into a fresh golden directory
  of the verification, judged by eye against the panel's spec, and matched on a second run
  with `TESSERA_GOLDEN_REQUIRE=1`. The status records the install version and the verdict per
  image by test and name, never the picture or its text.
- Everything else of decision 0031 holds as written: inputs, capture path, painter, tolerances,
  blessing variables, locations, the refusal of directories inside a git work tree, atomic
  writes and the determinism rules (virtual clock, fixed fonts and viewport, no running
  animation, no capture after a settle timeout).
- The census of §1 checks every region by numbers (image drawn, marker position); tier-2
  goldens cover only the sample. No golden of all 1,525 regions is kept.

### 3. Criterion 2 is checked from logs
A mounted walk is checked by the walk log and the frame log of `walker.md` §16.10 against values
the test derives from the install at run time (`walker.md` §16.11: mount state, movement layer,
speeds, gaits and the rider's and the mount's clips as the clip resolver gives them). No golden
image is part of criterion 2; the look of the mounted figure is an owner row.

## Consequences
- Criterion 1 has a fixed, countable meaning that a real-data test can check on any install, and
  the exit verifier compares the counts with those of `map-windows.md` §5.2 on the owner's
  install.
- The scenario feed needs the `map-region` family and marker sets (`scenario-feed.md` "P5
  travel"); a resolver that reports `map-region` as unsupported cannot pass criterion 1.
- `tessera-ui` gets committed synthetic goldens of the P5 panels; their images stay small
  (decision 0031 tier 1, under 1 MiB each).
- The P5 tier-2 goldens live in the same local directory scheme as P2's and P3's; a patch of
  the install starts a new set.
- This record stands on decision 0031. If the owner changes 0031, the P5 panels follow the
  changed rules; if the owner rejects it, §2 falls with it and §1 and §3 stand alone.
- Comparison with the original client stays with the owner (`docs/checklists/owner-observations.md`,
  rows from O140) and is not part of any P5 criterion.

## Open
- ~~The owner accepts this record or asks for changes.~~ Done: Accepted by the owner on
  2026-10-16.
- `map-windows.md` has no fallback map window for runs without an install (its §8 OPEN); the
  tier-1 world-map golden of §2 waits for that rule.
- `map-windows.md` and `travel-windows.md` are now both **Accepted** (independent reviews
  2026-10-13); their reviewed definitions (map-region categories `map-windows.md` §5.1/§5.3,
  travel/template census `travel-windows.md` §8) are what §1–§2 of this record stand on. No
  review changed `map-windows.md` §5.1/§5.3; `travel-windows.md` §8.1 was reconciled to
  `OtherReference` 5 / `Unreferenced` 0, which does not affect item 1 (a map-region, not a
  travel, definition).
