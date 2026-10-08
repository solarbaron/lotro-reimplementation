<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# World coordinates: regions, landblocks, cells, map coordinates, `/loc`, rotations

| | |
|---|---|
| **Status** | **Accepted** (re-review of 2026-10-07 passed; covers the second repair revision of 2026-10-07, §16: the range test before the 160 → 0 carry in §4 and C22, two CO9 vectors). Previously Accepted 2026-10-07 and 2026-10-06. Sections, anchors and numbers of the earlier Accepted text are unchanged. |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review of the second repair revision, 2026-10-07 (a session separate from the writer's): **passed**, Accepted. Details in the review log (decision 0018). |
| **Implements into** | `crates/world` (coordinate types, conversions, map-coordinate formatting and parsing), `crates/terrain` (height lookup), `crates/net` sandbox (`--spawn`, reported position), `apps/client` (on-screen coordinates, minimap), `apps/explorer` |
| **Companion specs** | `landblock.md` (landblock records, height scale §3.4, triangulation §3.11, minimap tiles §6). `entity-descriptor.md` §4.4–§6 (placement frame, rotation evidence, placement transform). `environment.md` §3 (region descriptor 0x0F, which stores the map-coordinate constants). `property-system.md` §4.6 (stored Position encoding). `records.md` §2.2 (landblock DIDs), §8 (DID maps). `string-table.md` §7–§9 (StringInfo arguments, used by the map-coordinate text). `mesh.md` §11 (model axes and units). |

---

<a id="s0"></a>
## 0. Purpose

This spec fixes every coordinate frame Tessera needs for the offline world walker and the
conversions between them:

- region, landblock, landcell, block-local and region-global positions (§2–§5);
- units, axes and handedness (§3);
- heights (§6) and interior cells (§7);
- the in-game **map coordinates** ("12.3S, 45.6W"), their constants, the exact arithmetic,
  rounding and lettering the original uses (§8);
- the exact `/loc` position text (§9);
- minimap pixel coordinates (§10);
- rotations stored as quaternions, yaw, pitch, bank and compass heading (§11);
- render-space rebasing (§12), invariants and test vectors (§13).

Every worked example uses synthetic numbers chosen for this spec. Rules marked BIN were read
from the client executable (§1) and are stated here in words; numeric constants that define
those rules are functional facts kept with their provenance ([decision
0012](../decisions/0012-numeric-constants.md)).

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = checked on the user's install over all records named (date given
below); **BIN** = read from the client executable (method M4, [decision
0011](../decisions/0011-standing-m4-authorisation.md)) and, where stated, checked against
data; facts that rest on BIN alone say so; **Reported** = third-party only.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04: all landblock records of regions 1–5 and 14 (`landblock.md` §1), all 7 region descriptors, all 3,253,862 placed static entities, 120 minimap tiles. Re-checked 2026-10-05: the REGION DID map 0x28000008 (all 9 entries), the region descriptors' offsets 4–35 (all 7), the STRINGTABLE DID map 0x28000004 entries used in §8.3, the string table it names under the label `Position` (all 5 entries), and the 20 reference rows of `docs/checklists/m3-reference-locations.md` recomputed with §8.2–§8.3. Repair re-check 2026-10-05: the block-local position of every one of the 3,253,835 SEDs with a frame against whether its landblock-info record has cells and whether the SED has the interior flag (CO2, §7), and the bank-arcsine argument of every stored SED rotation evaluated in the client's f32 order (§11.3). Re-check 2026-10-06: the near-zero component patterns of all 3,253,835 framed SED rotations at the tolerances stated in §11.1. |
| BIN | The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035, compiled 2026-09-27), disassembled in the dirty room on 2026-10-05 for facts only. Questions: (1) how the `/loc` position text is produced: tokens, order, landcell rule, number formats, omissions (owner item O1); (2) how its heading relates to a stored rotation, and so to placed objects (O2); (3) how map coordinates are computed, rounded, lettered and composed, and where their constants come from (O1, the former §8.3 OPEN); (4) what the region number is. Why M1–M3 were not enough: the data hold the constants and the string templates but no rule; the published description (ER) gives the output only by example, and its omission rule for `h` disagrees with the code; the owner's in-game readings (M3, items O1 and O2) are pending and could sample the rounding but not establish it. Only position-formatting, map-coordinate, region-lookup, angle, number-formatting and string-argument code was read; no network, login, launcher or anti-tamper code. **Second use, 2026-10-05 (repair session, same build, same method, address ranges only):** (5) how the client computes the flag byte of a stored Position, bits 0x01–0x80 (§15); (6) which cell values mean "outdoors", "interior" and "no valid cell", and where the outdoor value comes from (§7); (7) which region and block values make a position invalid (§4, §8.2, §9.2); (8) which members, separators and defaults the client's inverse position parser accepts (§9.8); (9) whether the reduction of h, p and b and their omission tests run in single or double precision, and with which constants (§9.4); (10) what the client does when the bank arcsine argument leaves [−1, 1] (§11.3); (11) whether the callers of the map-coordinate routine test the cell (§8.3). **Why M1–M3 were not enough for (5)–(11):** the data hold only values that already satisfy the rules (no stored Position disagrees with its flags, none has region 0, block 0xFF or cell 0xFFFF), so they cannot show the rule or its edge cases; precision and out-of-domain behaviour leave no trace in data; no published source states them; the owner's readings (M3) cannot reach a parser that only developer facilities use. Only the Position flag writer, the position-text formatter, the static initialisers of the cell constants, the angle helpers, the quaternion decomposition, the inverse parser's strings and member table, and the three direct callers of the map-coordinate routine were read. **Third use, 2026-10-07 (OPEN-closing session, same build, same method, address ranges only):** (12) which block the client assigns to a point exactly on a block edge, and what it does at the far edge of the grid (§4 OPEN); (13) the defaults of the inverse position parser for members that are absent (§9.8 implementer OPEN); (14) whether the client links its C runtime statically or uses the system's, and which printf rounding option it selects (§9.5 and §11.3 OPENs). **Why M1–M3 were not enough for (12)–(14):** no stored position lies exactly on an edge or at the grid's far edge; the parser is a developer facility the owner cannot reach; tie rounding and the runtime choice leave no trace in data or in ordinary `/loc` output (ties need binary-exact values). Read: the client's general position normaliser and its constants, the inverse parser's per-member cases and its final assembly of the position, the default position constructor, the cell constants' static initialisers, the C runtime import table, the printf option accessor and the start-up routine that sets its bits. No network, login, launcher or anti-tamper code. **Repair re-read, 2026-10-07 (same build, same method, after the review of the same date):** question (12) again, for the order in which the normaliser handles *x* and *y* and what a failure leaves behind; only the normaliser was read. **Second repair re-read, 2026-10-07 (same build, the analysed copy's file hash equal to the installed executable's; same method):** question (12) again, for whether the normaliser tests the target block range before or after the 160 → 0 carry; only the normaliser was read. |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution; decompiled `PositionDecoder`, `GeoLoader`) and `lotro-tools` `MapUtils` (readable source). Facts only. |
| ER | `research/notes/engine-client-re.md` §3–§4: lotro-wiki "Coordinate Systems" page (landcells, `/loc` fields, map-coordinate formula), engine position struct and resource keys. |
| DS | `research/notes/data-semantics.md` §6.1 (Position encoding, 160-unit landblocks, LC's map formula). |
| AYA | aya-po Lotro-Map-Viewer `regions.xml` (no licence; facts only): area bounds written as signed map coordinates, south and west negative. |
| LB | `landblock.md` (this install, same date). |
| ED | `entity-descriptor.md` (this install, same date). |
| ENV | `environment.md` §3 (this install, same date). |
| MESH | `mesh.md` §11.1 (characters face model +Y; origin at the foot point). |
| MSDOC | Microsoft C runtime documentation ("`printf` family" floating-point rounding change for Windows 10 version 2004, programs built with Visual Studio 2019 16.2 or later; learn.microsoft.com), consulted 2026-10-07 for the runtime's behaviour only. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| C1 | Landblocks are 160 × 160 units; X, Y in 0–254 per region (§4) | DS, ER; OBS (LB §2.3); BIN (160 is the block size in the map arithmetic) | Confirmed |
| C2 | Axes: +X east, +Y north, +Z up, right-handed (§3) | ER; OBS (minimap orientation, terrain seams, layer winding, quaternion test); BIN (longitude grows with X and is lettered E when positive, latitude grows with Y, §8) | Confirmed |
| C3 | Block-local x, y measured from the landblock's south-west corner, 0–160; z absolute (§5, §6) | DS, ED; OBS | Confirmed |
| C4 | Terrain height in units = stored u16 × 2,000.0 × f32(1 / 65,535), each product rounded to f32; 65,535 = 2,000.0 units (§6). Replaces the former stored / 32.768 | LB §3.4 (row L4: rule from the client, scale fixed by 3.25 M placed entities) | Rule BIN (via LB); scale Confirmed. Revised 2026-10-06 |
| C5 | Landcells: 20 units, 8 per landblock; `/loc` landcell = 8 · X + x / 20 truncated toward zero outdoors, 8 · X indoors (§5.2, §9.2) | ER; BIN | BIN (formula); the 20-unit size is the same for every region |
| C6 | Map coordinates: longitude = ΔL + (G_x − 20 · X0) · s, evaluated in single precision in a fixed order; constants X0, Y0, ΔL, Δφ, s read from the region descriptor (§8.1–§8.2) | LC, ER, DS; OBS (constants in every descriptor); BIN | Constants Confirmed; formula and evaluation order BIN, consistent with LC and ER |
| C7 | Letters: S when the computed latitude is below 0, else N; W when the longitude is below 0, else E; number before letter, no space; latitude first (§8.3) | ER, AYA, LC; BIN; OBS (string tables) | BIN |
| C8 | `/loc` position tokens `r`, `lx`, `ly`, `i`, `cInside`, `ox`, `oy`, `oz`, `h`, `p`, `b`, their order, formats and omission rules (§9) | ER (example); BIN | BIN; matches ER's published example |
| C9 | Minimap tile frame: 1.25 px per unit, top-left = north-west corner (§10) | LC (`MapUtils`); OBS (60,450 properties records, 120 tiles) | Confirmed |
| C10 | Rotations: unit quaternion stored (w, x, y, z), Hamilton product, active rotation from object to landblock frame (§11) | LC (field order), ED; OBS (slope-aligned objects); BIN (the client's identity quaternion is (1, 0, 0, 0) in stored order) | Confirmed |
| C11 | Compass heading `h` = minus the yaw, in degrees, reduced to 0 ≤ h < 360: clockwise from north, east = 90 (§11.4) | ER (0 = north); BIN; MESH (characters face +Y) | BIN |
| C12 | Map-coordinate number: one decimal; magnitude + 0.0502, truncated to tenths (§8.3) | BIN; OBS (string table argument layout) | BIN |
| C13 | Yaw, pitch and bank of a rotation: z-y-x decomposition (§11.3) | BIN | BIN |
| C14 | Region number R = key of the REGION DID map = the u32 at offset 4 of the region descriptor = R of the landblock DIDs (§4) | OBS (7 of 7 descriptors, 9 map entries); BIN (region lookup by key) | Confirmed |
| C15 | Flag byte of a stored Position: 0x01 region ≠ 0; 0x02 X ≠ 0xFF and Y ≠ 0xFF; 0x04 instance ≠ 0; 0x08 cell ≠ 0xFFFF; 0x10 any of x, y, z ≠ 0 exactly; 0x20 any quaternion component ≠ the identity's exactly; 0x40 region = 1; 0x80 cell = 0 (§15) | BIN (question 5: the client's flag writer); PS (`property-system.md` §4.6: 6,115 of 6,115 stored Positions follow every rule, observed 2026-10-05) | Confirmed |
| C16 | Cell values: 0xFFFF = no valid cell (invalid position); 0 = outdoors, a constant the client derives at start-up as 0xFFFF + 1 in 16 bits; 1 = the first interior cell; every other value is an interior cell (§7) | BIN (question 6: static initialisers, the formatter and the flag writer); PS (no stored cell is 0xFFFF; 0 occurs exactly where bit 0x80 is set) | BIN; consistent with data |
| C17 | A position is invalid when R = 0, X = 0xFF, Y = 0xFF or cell = 0xFFFF; it then has no map coordinates and `/loc` prints `lx0 ly0` (§4, §8.2, §9.2) | BIN (question 7: the same four tests open the formatter, the map-coordinate routine and the flag writer); OBS (CO1: no landblock DID and no stored Position has X or Y = 255) | BIN; consistent with data |
| C18 | The client's inverse position parser: members `r`, `bx`, `by`, `lx`, `ly`, `i`, `c`, `ox`, `oy`, `oz`, `h`, `p`, `b`; separators space, comma, semicolon and colon; `r` optional with default 1; `c` a number or a word for outside or inside (§9.8) | BIN (question 8: the separator literal and member letters in the parser, and the client's own help text for it, which states the default) | BIN |
| C19 | h, p and b are reduced and tested in single precision: f32 product with 57.29578, the runtime's single-precision remainder by 360, f32 addition of 360, f32 comparisons with 0.0002 and 0.011459156 (§9.4) | BIN (question 9: the angle helpers and their constants) | BIN |
| C20 | The bank arcsine is not clamped: the single-precision argument goes straight to the runtime's single-precision arcsine (§11.3) | BIN (question 10); OBS (3,091 of 3,253,835 stored SED rotations give an argument beyond ±1 in the client's f32 order, census 2026-10-05) | BIN; census Confirmed |
| C21 | The map-coordinate routine accepts interior positions; its three direct callers do not test the cell (§8.3) | BIN (question 11) | BIN |
| C22 | The client's general outdoor-position normaliser: a block-local *x* or *y* outside [0, 160) moves the position by ⌊value / 160⌋ blocks (f32) and keeps the remainder; a remainder of exactly 160 becomes 0 in the next block; a target block outside 0–254 fails (or, when the caller asks for clamping, gives 0 in block 0 below, 159.9998 with block 0xFF above); *x* is finished and written before *y* is examined, so a failure on *y* leaves *x* and X already re-homed; the range test uses X + ⌊value / 160⌋ before the carry, so a value just below 0 in block 0 fails (or clamps) even when the carry would have brought it back to 0 in block 0; a point exactly on an edge belongs to the block east or north of it (§4) | BIN (question 12: the normaliser, called from 73 places, and its constants 160.0 and 159.9998; order of *x* and *y* re-read 2026-10-07) | BIN |
| C23 | Defaults of the client's inverse position parser: `r` 1, `i` 0, `c` 0 (outdoors; the words `outside` → 0, `inside` and `limbo` → 1); absent offset members 0, except that with the block given by `lx` `ly`, an outdoor cell and no offset member at all, the offset is the landcell's south-west corner; absent `h`, `p`, `b` 0; the result is normalised by C22 without clamping (§9.8) | BIN (question 13) | BIN |
| C24 | The client uses the system's Universal C runtime (printf family and `asinf` imported, not linked in) and sets the printf options "legacy wide specifiers" (0x4) and "standard rounding" (0x20) at start-up; with standard rounding, Microsoft's runtime on Windows 10 version 2004 and later rounds binary-exact ties to even (§9.5, §11.3) | BIN (question 14: import table, option accessor, start-up routine); MSDOC (Microsoft C runtime documentation, `printf` family, Windows 10 2004 change) | BIN; the runtime's tie behaviour Reported |

---

<a id="s2"></a>
## 2. Frames at a glance

| Frame | Coordinates | Range | Where used |
|---|---|---|---|
| **Region** | *R* (one hex digit of a landblock DID; u8 in stored Positions; u32 in the client's memory) | 1, 2, 3, 4, 5, 14 have terrain (LB §2.2) | Every position. Regions are separate worlds whose coordinate ranges overlap. |
| **Landblock** | (*R*, X, Y), X and Y u8 | 0 – 254 | Record DIDs `0x80KRXXYY` (LB §2.1), streaming |
| **Block-local** | (x, y, z) f32 inside landblock (X, Y) | x, y in [0, 160] outdoors; z absolute | Stored positions (entities, cells, Position values) |
| **Region-global** ("ox, oy" in LC) | (G_x, G_y) = (160 · X + x, 160 · Y + y), plus z | 0 – 40,800 | Distances across blocks, map coordinates, physics |
| **Landcell** | (lx, ly) integers | 0 – 2,040 | `/loc` output (§9), engine resource keys |
| **Map coordinates** | (latitude, longitude) one-decimal numbers with N/S, E/W | about −124.4 … +79.6 and −146.8 … +57.2 | UI (§8) |
| **Minimap pixel** | (px, py) per tile | 0 – 200 | Landblock minimap tiles (§10) |
| **Interior cell** | cell index (u16) + block-local f32 | x, y may leave [0, 160] | Dungeons and buildings (§7) |
| **Render** | f32 relative to a camera anchor | small | GPU (§12) |

---

<a id="s3"></a>
## 3. Units, axes and handedness (Confirmed)

- **Unit.** One world unit; by convention a metre (DS, ER; `mesh.md` §11 sizes characters
  in the same unit). A landblock is 160 units on a side; the height grid spacing is 5.
- **Axes.** +X points **east**, +Y points **north**, +Z points **up**. The frame is
  **right-handed** (X × Y = Z).

Evidence:

- Landblock X + 1 is the tile to the **right** and Y + 1 the tile **above** on the
  north-up minimap (LB §6.3, 40 of 40 pairs each), and the in-game longitude grows east with
  X while latitude grows north with Y (§8; LC, AYA; BIN: the client adds 160 · X + x into
  the longitude and letters a positive longitude E).
- Terrain triangles stored by the data are counter-clockwise when viewed from +Z
  (458 million of 458 million, LB §3.8.3), the convention of a right-handed, Z-up world with
  counter-clockwise front faces (`mesh.md` §11).
- Objects tilted to follow the slope have their local +Z rotated onto the terrain normal
  under the standard right-handed quaternion rotation (§11.2), not under its mirror.

Implementations keep one internal convention (this one) everywhere. A conversion to a
renderer's clip space (for example Y-up) happens only at the camera.

---

<a id="s4"></a>
## 4. Regions and landblocks (Confirmed)

- A **region** number *R* selects a separate terrain grid. Regions 1, 2, 3, 4, 5 and 14
  have landblock data; region 6 has only a region descriptor; region 15 has empty archives
  (LB §2.2). Two positions are comparable only if their regions are equal.
- **Region numbers are DID-map keys.** The REGION DID map (root key 0x11, `property-system.md`
  §8.1) has one entry per region whose **key is the region number** and whose target is
  that region's descriptor (0x0F record); the descriptor repeats the number as its u32 at
  offset 4 (C14: keys 1, 2, 3, 4, 5, 6, 14 → descriptors whose offset 4 holds the same
  number; key 15 names a descriptor that does not exist). The client finds a region's
  descriptor by looking up the map with the region number (BIN); the map's labels are
  internal names that `/loc` and the map coordinates never use. Region number 0 means "no
  region".
- A **landblock** (X, Y) covers the square 160 · X ≤ G_x ≤ 160 · (X + 1),
  160 · Y ≤ G_y ≤ 160 · (Y + 1). X and Y are 0 – 254, so G_x and G_y are 0 – 40,800. The
  value 255 (0xFF) in X or Y marks an invalid position, as do region 0 and cell 0xFFFF
  (C17: the client's position-text formatter, map-coordinate routine and Position flag
  writer all start with these four tests). No landblock DID and no stored Position in this
  install uses X or Y = 255 (CO1).
- The landblock DIDs of (R, X, Y) are `0x80KRXXYY` (LB §2.1): for example the height record of
  region 1, X = 126, Y = 140 is 0x80017E8C.
- **Containing landblock of a global point**: X = ⌊G_x / 160⌋, x = G_x − 160 · X (likewise
  for Y). A point exactly on a shared edge belongs to the block **east or north** of it
  (*x* = 0 there), as the client's normaliser below decides (C22); the grid's far edge
  G = 40,800 has no valid block (X would be 255).

The engine's in-memory position (ER §3; BIN) is a region number, a landblock (u8 X, u8 Y),
a cell (u16), an instance (u16), block-local f32 x, y, z and a rotation quaternion — the
same split Tessera uses for stored positions.

**Re-homing a position (C22, BIN 2026-10-07; closes the former edge-rule OPEN).** The
client has one general routine that brings an outdoor position back into its block (called
from 73 places, among them the inverse parser of §9.8). It acts only on a valid position of a
known region (1–15) whose cell is 0 (outdoors); *z* is never changed. For *x* (then
independently for *y*), with the block size 160.0 as an f32:

1. If 0 ≤ *x* < 160, nothing changes.
2. Otherwise *k* = ⌊*x* / 160⌋ (single precision, rounded toward minus infinity) and the
   new block index is X + *k*.
3. If the new index lies in 0–254: *x* becomes *x* − *k* · 160 (single precision); if that
   result equals 160 exactly (an f32 rounding case), it becomes 0 and the index grows by
   one more. The position takes the new index.
4. If the new index lies outside 0–254, the routine stops and reports failure, unless its
   caller asked for clamping; then a negative index gives *x* = 0 in block 0, and an index
   above 254 gives *x* = 159.9998 with block index 0xFF, which makes the position invalid
   (C17), and the routine goes on.

The range test of steps 3–4 looks at X + *k*, **before** the carry of step 3. A negative offset
of magnitude at most 2⁻¹⁷ (about 7.63 · 10⁻⁶) gives *k* = −1, and *x* − *k* · 160 then
rounds to 160.0 exactly in single precision (ties to even), so the carry brings it back to 0
in its own block; in block 0, though, the range test already sees block −1, and the routine
fails (or, with clamping, gives 0 in block 0). Example: *x* = −10⁻⁶ has the f32 quotient
−6.25 · 10⁻⁹. A slightly larger magnitude (for example −7.7 · 10⁻⁶) gives 159.99998 in the
block below.

The routine finishes *x* (steps 1–4, including writing the new offset and block index)
before it looks at *y*. So a failure on *x* leaves the position unchanged, but a failure on
*y* (without clamping) leaves an *x* offset and X index that have **already been
re-homed**, with *y* and Y as they were. A caller that needs all-or-nothing behaviour must
keep a copy of the position. With clamping the routine always succeeds, and a clamped *x*
(block 0 or 0xFF) does not stop it from treating *y*.

For an interior position whose cell is the generic value 1 (§7) the same routine instead
asks a cell-lookup service for the cell containing the position and stores it (not traced
further).

Consequences: a point exactly on a shared edge belongs to the block **east or north** of it
(*x* = 0 there), which is what `tessera-world` does. The far edge of the grid, G = 40,800,
has **no** valid block in the client: X + 1 = 255 is outside 0–254, so the routine fails (or
invalidates the position when clamping). `tessera-world` keeps such a point in block 254 with
*x* = 160 (a Tessera choice; the client never produces such a position, because it cannot
re-home onto it).

---

<a id="s5"></a>
## 5. Block-local positions and landcells

<a id="s5-1"></a>
### 5.1 Block-local (Confirmed)

Outdoors, stored positions give (x, y) in [0, 160] measured from the landblock's
**south-west corner** (smallest G_x and G_y), and z as an absolute height (§6). This is the
frame of static entity positions (ED §4.4), cell positions (LB §4.4) and of the Position
value type (`property-system.md` §4.6) when it carries a landblock.

<a id="s5-2"></a>
### 5.2 Landcells (BIN)

A landblock is divided into 8 × 8 **landcells** of 20 × 20 units. The cell size is a
constant 20 for every region (the client's per-region query returns 20 whatever the region).
The landcell the original reports for an outdoor position is

- lx = 8 · X + t(x / 20) and ly = 8 · Y + t(y / 20), where t truncates toward zero and the
  division is done in single precision;
- conversely, for an outdoor position with x < 160, X = ⌊lx / 8⌋, and the landcell's
  south-west corner is at G_x = 20 · lx, G_y = 20 · ly.

There is **no clamp**: a position with x = 160 exactly (the block's east edge) reports
lx = 8 · X + 8, the first landcell of the next block. Interior positions report the block's
south-west landcell (§7). Range 0 – 2,040 (2,040 = 8 · 254 + 8). Landcells carry no data of
their own in the landblock records; they appear in the `/loc` output (§9) and in engine
resource keys (ER §3). A landcell is **not** the same thing as an interior cell (§7).

> **Correction (implementation choice withdrawn):** `tessera-world` clamps t(x / 20) to
> 0–7 so the landcell stays in the owning block. The original does not clamp; for exact
> compatibility, compute lx as above (it differs only at x or y = 160 exactly).

---

<a id="s6"></a>
## 6. Heights (scale Confirmed; rule BIN via `landblock.md` §3.4)

- z is an **absolute** height in world units, not an offset from the ground (ED §4.4).
- Terrain stores heights as u16 samples. **Height in units = stored × 2,000.0 × f32(1 / 65,535)**,
  evaluated in single precision as LB §3.4 states (convert the sample to f32, multiply by
  2,000.0, round to f32, multiply by the f32 nearest 1 / 65,535 = 1.5259022 · 10⁻⁵, round to
  f32). The range is 0 – 2,000.0 units: stored 0 → 0.0, 13,107 → 400.0, 32,768 → 1,000.0153,
  65,535 → 2,000.0. The rule is the client's (LB row L4, BIN); the scale is Confirmed. The
  former rule of this spec, stored / 32.768, is superseded; it differs by at most 0.0305
  units. Observed stored values reach 65,502 (1,998.99 units, LB §3.4).
- A position's z (and so `oz`) is an f32 of its own, not a stored height sample; it is not
  quantised to steps of 2,000 / 65,535.
- The terrain height under (x, y) is defined by the triangulation of LB §3.11.
- `/loc` reports the position's z as `oz` (§9). For the player that is the origin of the
  avatar's frame, which `mesh.md` §11.1 puts at the foot point.

> **OPEN:** whether the original's movement code keeps the avatar's origin exactly on the
> terrain surface or at a small fixed distance above it (a constant offset of `oz` against
> the terrain height). Not traced in the client; the owner readings of
> `m3-reference-locations.md` (item O1) show it.

---

<a id="s7"></a>
## 7. Interior cells and instances

- A position may name an **interior cell** (u16) of its landblock's landblock-info record
  (LB §4.4). Cell values (C16):

  | Value | Meaning |
  |---|---|
  | 0xFFFF | no valid cell: the position is invalid (§4, §9.2) |
  | 0 | outdoors. The client does not store this as a literal: at start-up it computes its outdoor constant as the invalid value 0xFFFF plus 1 in 16-bit arithmetic, which wraps to 0. |
  | 1 | the first interior cell (the client computes it as the outdoor value plus 1; decal and light objects built from interior SEDs get it, ED §4.8) |
  | any other value | an interior cell of the landblock |

  A stored Position sets flag 0x80 exactly when its cell is 0 and flag 0x08 exactly when
  it is not 0xFFFF (C15, §15).
- Cell geometry is placed in the same landblock frame, but interior coordinates are **not**
  confined to [0, 160]. In landblocks that have cells, 768,911 of the 1,001,648 SEDs with a
  frame (77 %) lie outside the square, up to about ±960 units, and every one of them carries
  the interior flag (ED §4.4, §4.8). Never re-home such a position to a neighbouring
  landblock.
- Landblocks whose landblock-info record has **no cells** also hold out-of-square SEDs:
  4,960 of their 2,252,187 SEDs with a frame (0.22 %). 4,052 of them carry the interior flag
  (Physics 3,887, Weenie 144, Light 17, TeleportDestination 3, LBIOnly 1): interior content
  placed in the frame of a block that stores no cell list of its own (ED §4.8 OPEN). 908 do
  not (Physics 886, Light 22). By their distance outside the square, measured per axis
  (the larger of the two amounts by which x and y leave [0, 160], each 0 when that
  coordinate is inside) and bucketed with half-open bounds: under 1 unit 83, at least 1 and
  under 10 units 194, at least 10 and under 100 units 464, 100 units or more 167. (The
  Euclidean distance to the square gives 83 / 193 / 464 / 168 for the same buckets; both
  censuses over all 908, OBS 2026-10-05.) These 908 are the only out-of-square
  SEDs without the interior flag in the install. They are not re-homed either: a SED always
  stays in the frame of the block that stores it (OBS, repair re-check 2026-10-05).
- For an interior position `/loc` reports lx = 8 · X and ly = 8 · Y (the block's
  south-west landcell, whatever x and y are) and adds the token `cInside` (BIN, §9.2).
- A position may also carry a u16 **instance** number (Position flag 0x04; `/loc` token `i`,
  printed only when it is not 0). It selects an instanced copy of the same space and does
  not change the coordinates. The client also writes it into bits 24–35 of the instance IDs
  of the static entities of an instanced landblock copy, and uses "instance 0" in the
  content-layer test (`entity-descriptor.md` §4.2, §4.5.1; BIN).

---

<a id="s8"></a>
## 8. Map coordinates (the "12.3S, 45.6W" convention)

<a id="s8-1"></a>
### 8.1 Constants (Confirmed; stored in the data, read per region)

Every region descriptor (ENV §3.1; all 7 records identical in these fields) stores:

| Offset | Type | Value (all 7) | Meaning |
|---|---|---|---|
| 4 | u32 | the region number | *R* (C14) |
| 16 | u32 | 1,468 | Map origin X0, in 20-unit landcells (unsigned) → G_x0 = 29,360 |
| 20 | u32 | 1,244 | Map origin Y0, in landcells → G_y0 = 24,880 |
| 24 | f32 | 0.0 | ΔL: added to the longitude, in map units (BIN) |
| 28 | f32 | 0.0 | Δφ: added to the latitude, in map units (BIN) |
| 32 | f32 | 0.004999999888… (the f32 nearest 0.005) | Scale s: map units per world unit (one map unit = 200 world units) |

Code constants used with them (BIN): the landcell size L = 20 (§5.2) and the block size 160.
The client reads the five fields from the descriptor of the **position's own region**
(REGION DID map key R, §4) every time; there is no hard-coded origin. Regions 2–5 and 14
therefore use the same numbers as region 1 only because their descriptors hold the same
values.

<a id="s8-2"></a>
### 8.2 Formula and evaluation order (BIN; constants Confirmed)

Input: a position (R, X, Y, cell, x, y). Output: a latitude and a longitude, or nothing.

1. If R = 0, X = 0xFF, Y = 0xFF or cell = 0xFFFF, there are no map coordinates. Interior
   positions (cell ≠ 0) are **not** excluded: they get map coordinates from their block and
   their (possibly far out-of-square) offsets.
2. Find the region descriptor (REGION DID map key R). If there is none, there are no map
   coordinates.
3. Compute, every operation in **single precision** (f32) and in this order:
   - a = (X · 160 + x) · s, where X is converted to f32 first;
   - b = ((X0 converted to f32) · L) · s;
   - longitude = (ΔL − b) + a;
   - likewise latitude = (Δφ − ((Y0 converted to f32) · L) · s) + (Y · 160 + y) · s.

Positive longitude is east, positive latitude is north. The same constants serve every
region, so map coordinates of different regions overlap; they do not identify a place
without the region.

Inverse (Tessera, for spawning at map coordinates; not a client rule):
G_x = G_x0 + (longitude − ΔL) / s, G_y = G_y0 + (latitude − Δφ) / s, then split into
landblock and block-local as in §4.

Over the landblock range, longitude runs from −146.8 (G_x = 0) to +57.2 (G_x = 40,800) and
latitude from −124.4 to +79.6.

> **Implementation note:** the client multiplies by the stored f32 in f32 arithmetic; it
> does not widen it to an exact 0.005. An implementation that wants identical strings in
> every case evaluates step 3 exactly as written (f32, same order). The bias of §8.3 makes
> the f32 error irrelevant at half tenths (an odd multiple of 10 units from the origin
> always rounds away from zero), but values within about 10⁻⁵ of the rounding threshold
> can still differ between f32 and f64 arithmetic.

<a id="s8-3"></a>
### 8.3 Display (BIN; templates Confirmed in the data)

**Letters.** The latitude gets `S` when the computed f32 latitude is **below 0**, otherwise
`N`; the longitude gets `W` when it is below 0, otherwise `E`. The letter follows the sign
of the unrounded value, so −0.03 shows as `0.0S`; 0.0 and −0.0 show `N` / `E`.

**Number.** Each coordinate is shown as its magnitude with exactly **one decimal**:

1. Take |v| (the f32 value) and widen it to f64.
2. Add 0.0502, that is half a unit of the last place (0.05) plus a fixed bias of 0.0002
   (the f32 constant 0.0002 widened: 0.00019999999494757503).
3. Multiply by 10 and truncate toward zero to an integer N.
4. Write N div 10 in decimal (no leading zeros except a single 0), then the locale's
   decimal separator, then the digit N mod 10.

The threshold is therefore 0.0498 above a tenth, not 0.05: a magnitude of 66.6497 shows as
66.6 but 66.6499 as 66.7, and 4.04975 shows as 4.0. There is no sign. (The
client's general number formatter does this digit by digit and would insert the locale's
group separator into a long integer part, which never happens for map coordinates.) The
decimal separator and the digit characters come from the locale record: the 0x58 record
named by key 0x0B (label `Language`) of the STRINGTABLE DID map, whose layout is
`string-table.md` §10.3 (digit alphabet, decimal separator, group separator; in the English
data the digits `0`–`9` and the separator `.`). That spec's number rule (§9.4 there) is the
same rounding as step 2 above.

> **OPEN:** `string-table.md` §10.3 matches the record's fields to the client's locale
> object by order, type and value (Inferred there), not from the client's loader. If that
> match turns out wrong for the digit alphabet or the decimal separator, the map-coordinate
> text of non-English locales changes. Until it is settled, Tessera takes the digit
> alphabet and the decimal separator from the fields named in `string-table.md` §10.3, and
> falls back to `0`–`9` and `.` when the record is missing or does not parse (Decision; the
> client itself prints numbers as empty text when the record is missing, `string-table.md`
> §9.4).

**One coordinate string** = the number immediately followed by its letter, no space:
`4.0S`, `66.6W`.

**The pair** = latitude string, a comma and a space, longitude string: `4.0S, 66.6W`.

Where the text comes from (Confirmed in the English data; the client looks the tables up by
STRINGTABLE map key, BIN): each coordinate string is the template of the string table under
STRINGTABLE key 0xC (label `Position`; 0x250001FD in this install), token 0x08D46694, whose
two slots are argument 0x539D (the number, a float argument with one decimal place) and
argument 0x48E2 (the letter, a nested StringInfo); the letters are tokens 0x005569A8 (`N`),
0x005A6CA8 (`S`), 0x0004B8A4 (`E`) and 0x0005DCA4 (`W`) of the same table. The pair is
token 0x053B8BB3 of the table under key 0x10000039 (label `WeenieClientMsgs`), whose slots
are argument 0x753 (latitude) and 0x6C7 (longitude). Other locales may order or punctuate
differently through their own tables; a client that renders through the tables
(`string-table.md` §9) reproduces them, one that writes the English form directly is exact
for English.

The same pair is what the client's map windows show for a pointed-at map location and in
map-note tooltips (another template places a name before the pair in parentheses). The
computation accepts interior positions (§8.2 step 1), and none of the three direct callers
of the client's map-coordinate routine tests the cell before calling it (C21).

> **OPEN:** whether any UI panel that shows the **player's own** map coordinates (for
> example a readout next to the minimap) hides them, or shows something else, while the
> player is in an interior cell. The panel code that does this was not identified; the
> routine and its three direct callers do not make the distinction, so if a panel hides the
> numbers it does so elsewhere (UI layout or script). Until settled, Tessera shows the
> computed coordinates indoors too (Decision). An owner observation would settle it: stand
> inside a building or dungeon, note whether and where map coordinates are shown, and what
> `/loc` prints there (a proposed addition to `docs/checklists/owner-observations.md`).

<a id="s8-4"></a>
### 8.4 Worked examples (synthetic; computed with §8.2–§8.3)

**A. Block-local → map.** Region 1, landblock (100, 150), block-local (40.0, 80.0).

- G_x = 100 · 160 + 40 = 16,040; G_y = 150 · 160 + 80 = 24,080.
- longitude (f32) = −66.600006…; latitude = −4.0.
- Displayed: `4.0S, 66.6W`.

**B. Map → landblock (Tessera's inverse).** `12.3S, 45.6W` means latitude −12.3, longitude
−45.6.

- G_x = 29,360 + (−45.6 / 0.005) = 29,360 − 9,120 = 20,240 → X = 126, x = 80.
- G_y = 24,880 + (−12.3 / 0.005) = 24,880 − 2,460 = 22,420 → Y = 140, y = 20.
- Region 1 height record: 0x80017E8C.

**C. North-east quadrant.** Landblock (200, 170), block-local (0, 0): G = (32,000, 27,200);
longitude 13.199997, latitude 11.600006 → `11.6N, 13.2E`.

**D. Origin.** G = (29,360, 24,880) is landblock (183, 155), block-local (80, 80): both
values 0.0 → `0.0N, 0.0E`.

More vectors (landblock (183, 155) unless stated; block-local x, y):

| x, y | Longitude (f32) | Latitude (f32) | Display |
|---|---|---|---|
| 90.0, 80.0 | 0.0499878 | 0.0 | `0.0N, 0.1E` |
| 70.0, 80.0 | −0.0500031 | 0.0 | `0.0N, 0.1W` |
| 74.0, 76.0 | −0.0299988 | −0.0199966 | `0.0S, 0.0W` |
| 70.03, 80.0 | −0.0498657 | 0.0 | `0.0N, 0.1W` (above the 0.0498 threshold) |
| 70.06, 80.0 | −0.0496979 | 0.0 | `0.0N, 0.0W` (below it) |
| landblock (254, 254), 160.0, 160.0 | 57.199997 | 79.600006 | `79.6N, 57.2E` |
| landblock (0, 0), 0.0, 0.0 | −146.8 | −124.399994 | `124.4S, 146.8W` |

<a id="s8-5"></a>
### 8.5 Map-coordinate input in Tessera (Decision)

This is Tessera's own convention for reading map coordinates that a user types or pastes
(spawn points, "go to" fields). It is not a client rule and needs no confirmation.

- Accepted forms: the display form of §8.3 (`12.3S, 45.6W`); lower-case letters; a space
  between the number and its letter; the comma missing; and two signed numbers without
  letters, read as latitude then longitude (`-12.3, -45.6`).
- With letters, the **letter decides the axis**: N or S is latitude, E or W longitude, so
  `45.6W, 12.3S` is accepted with longitude first. Both axes must be given exactly once,
  and either both numbers carry a letter or neither does.
- A number that carries both a sign and a letter (`-12.3S`) is rejected as ambiguous.
- The result is turned into a position with the inverse of §8.2 (block and block-local
  offsets of §4).

Test vectors: `12.3s 45.6w`, `12.3 S, 45.6 W`, `45.6W, 12.3S` and `-12.3, -45.6` all give
latitude −12.3 and longitude −45.6 (landblock (126, 140), block-local (80, 20), §8.4 B);
`-12.3S, 45.6W` and `12.3S` alone are rejected.

---

<a id="s9"></a>
## 9. The `/loc` position text (BIN; matches ER's published example)

The original's `/loc` chat command replies with a localized sentence (it names the world and
the game time; that wording is game text and is not reproduced) into which the client
inserts the **position text** of the local player's position, produced by the rules below.
The same text appears wherever the client prints a position (bug reports, developer
output).

<a id="s9-1"></a>
### 9.1 Tokens and order

The text is the concatenation, with single spaces between tokens and no other characters:

| # | Token | Present when | Format |
|---|---|---|---|
| 1 | `r` + R | always | decimal integer (the u32 region number, §4) |
| 2 | `lx` + lx, `ly` + ly | always | decimal integers (§9.2) |
| 3 | `i` + instance | instance ≠ 0 | decimal integer |
| 4 | `cInside` | the position is valid and its cell is not 0 (interior) | literal |
| 5 | `ox` + x, `oy` + y, `oz` + z | x, y or z differs from 0 by more than 0.0002 | fixed point, two decimals (§9.5) |
| 6 | `h` + heading | heading not within 0.0114591559° of 0 (§9.4) | fixed point, one decimal |
| 7 | `p` + pitch | the magnitude of pitch exceeds 0.0002 (degrees) | fixed point, one decimal, signed |
| 8 | `b` + bank | the magnitude of bank exceeds 0.0002 (degrees) | fixed point, one decimal, signed |

Example forms: `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h270.0`,
`r1 lx800 ly1200 i43 cInside ox-300.50 oy412.25 oz20.00 h315.0`.

For an avatar standing upright, pitch and bank are 0 and tokens 7–8 never appear. The
tokens `p` and `b` matter for objects and cameras printed through the same rule.

<a id="s9-2"></a>
### 9.2 Landcells

- **Valid outdoor position** (R ≠ 0, X ≠ 0xFF, Y ≠ 0xFF, cell = 0): lx = 8 · X + t(x / 20),
  ly = 8 · Y + t(y / 20) as in §5.2 (truncation toward zero of a single-precision
  division; no clamp).
- **Valid interior position** (cell neither 0 nor 0xFFFF): lx = 8 · X, ly = 8 · Y.
- **Invalid position** (R = 0, X or Y = 0xFF, or cell = 0xFFFF): lx = ly = 0 and no
  `cInside`.

The block is the one the position holds; the client never re-homes the offsets into a
neighbouring block for printing. A reader recovers the block as X = ⌊lx / 8⌋ outdoors (for
x < 160) and always indoors.

<a id="s9-3"></a>
### 9.3 Offsets

`ox`, `oy`, `oz` are the position's block-local f32 x, y, z, unchanged (indoors x and y are
unbounded; ER). The whole group is left out only when all three are within 0.0002 of zero,
which in practice means a position at the block's corner at height 0. Negative values
carry a leading `-`.

<a id="s9-4"></a>
### 9.4 Heading, pitch, bank

From the position's rotation quaternion (w, x, y, z) (§11.1):

1. Decompose it as in §11.3 into yaw ψ (about +Z), pitch φ (about +X) and bank θ (about
   +Y), in radians.
2. **h**: take −ψ (a sign flip of the f32 value), convert to degrees by multiplying with
   the f32 constant 57.29578 (bit pattern 0x42652EE1, 57.295780181884766) in single
   precision, then **reduce**: take the remainder of division by 360 with the C runtime's
   single-precision remainder (truncating, so the remainder has the sign of the dividend;
   a remainder is always exact, so its precision does not matter); if the remainder is
   below 0, add 360 in single precision, and if that f32 sum is 360 or more, replace it
   by 0.
3. **p**: φ in degrees (same f32 product), reduced the same way, then mapped to
   (−180, 180] by subtracting 360 in single precision when the reduced value is greater than
   180. **b**: θ treated the same way (θ lies in [−90, 90], or is NaN, §11.3).
4. **Omission tests**, all comparisons between f32 values:
   - h: compute d = h − 0 in f32, reduce d as in step 2, subtract 360 when d is greater than
     180, take the magnitude and reduce it once more (which leaves it unchanged). h is
     **omitted** when the f32 constant 0.011459156 (bit pattern 0x3C3BBF2F, 0.0002 radians
     in degrees) is **greater than** that magnitude.
   - p and b: omitted when their magnitude is **at most** the f32 constant 0.0002 (bit
     pattern 0x3951B717, 0.00019999999494757503). A NaN is never omitted (every comparison
     with NaN is false).

Precision (C19): nothing in steps 2–4 is done in double precision. The values are widened
to f64 only to be printed (§9.5). The f32 addition of 360 matters: a reduced negative value
whose magnitude is at most 2⁻¹⁶ (about 1.53 · 10⁻⁵, half the f32 spacing just below 360;
an exact tie rounds to 360, the even neighbour) gives an f32 sum of exactly 360, which then
becomes 0.

Examples (synthetic): −ψ in degrees = −0.00001 → remainder −0.00001 → +360 rounds to 360.0 →
h = 0 → omitted. −ψ in degrees = −0.00002 → 359.99997 (the f32 below 360) → for the
omission test d wraps to −0.0000305, magnitude below 0.011459156 → omitted. −ψ in degrees
= −0.02 → h = 359.98 → wrapped magnitude 0.02 → printed as `h360.0`.

So h is a compass bearing, clockwise from north (§11.4). It is printed with one decimal, so
a heading between 359.95° and 359.9885° prints as `h360.0`, and one between 0.0115° and
0.05° as `h0.0`.

> **Correction:** ER reports that `h` is omitted "within about 1.3° of north". The client's
> own rule is the 0.0002-radian band above. A wider band seen in play would come from the
> heading the server or the movement code stores (for example a quantised heading), which
> is not traced here. Owner item O2 records `h` values and would show such quantisation.

<a id="s9-5"></a>
### 9.5 Number formats

`ox`, `oy`, `oz` use two decimals and `h`, `p`, `b` one decimal, through the C runtime's
fixed-point formatting of the f32 value widened to f64: the correctly rounded decimal of
the exact binary value (so 19.999 prints as `20.00` while its landcell, from the unrounded
value, stays 0). A negative value whose magnitude rounds to zero keeps its minus sign, as
that runtime format does (−0.00043 prints as `-0.0`, §9.7 row 11). An exact tie arises only for binary-exact values (for example 40.125 at two
decimals).

**Exact ties (C24; closes the former minor OPEN).** The client does not carry its own
number formatter for these values: it calls the system's Universal C runtime, and at start-up
it selects that runtime's "standard rounding" printf option. With that option Microsoft's
runtime on Windows 10 version 2004 and later rounds a binary-exact tie to even (40.125 →
`40.12`; Reported, MSDOC), which is what Tessera does. MSDOC adds that this rounding
follows the current floating-point rounding mode (round to nearest, ties to even, unless a
program changes it; no change of the mode by the client was looked for). On an older Windows runtime, or a
different implementation of the same runtime interface (for example under a compatibility
layer), ties may round up; that is a property of the host, not of the client.

<a id="s9-6"></a>
### 9.6 Edge cases

- A position with R = 0 prints `r0 lx0 ly0` followed by the other tokens by their own
  rules.
- The text has no trailing space and no other punctuation; the sentence around it adds its
  own.

<a id="s9-7"></a>
### 9.7 Worked examples (synthetic)

Region 1, landblock (100, 150), outdoors, instance 0 unless stated:

| Offset (x, y, z) | Rotation | Text |
|---|---|---|
| 40.0, 80.0, 399.99 | identity | `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99` |
| 40.0, 80.0, 399.99 | yaw ψ = +90° | `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h270.0` |
| 40.0, 80.0, 399.99 | yaw ψ = −90° | `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h90.0` |
| 40.0, 80.0, 399.99, instance 43 | yaw ψ = 180° | `r1 lx802 ly1204 i43 ox40.00 oy80.00 oz399.99 h180.0` |
| −300.5, 412.25, 20.0, cell 5 | yaw ψ = 45° | `r1 lx800 ly1200 cInside ox-300.50 oy412.25 oz20.00 h315.0` |
| 0, 0, 0 | identity | `r1 lx800 ly1200` |
| 160.0, 159.99, 12.5 | yaw ψ = 359.97° | `r1 lx808 ly1207 ox160.00 oy159.99 oz12.50 h0.0` |
| 10, 10, 5 | yaw ψ = 0.005° | `r1 lx800 ly1200 ox10.00 oy10.00 oz5.00` |
| 10, 10, 5 | q = (0.96225019, 0.08418598, 0.02255757, 0.25783416) (yaw 30° after a 10° turn about +X) | `r1 lx800 ly1200 ox10.00 oy10.00 oz5.00 h330.0 p10.0` |
| 10, 10, 5 | q = (0.95125124, 0.04494346, −0.16773126, 0.25488700) (yaw 30° after −20° about +Y) | `r1 lx800 ly1200 ox10.00 oy10.00 oz5.00 h330.0 b-20.0` |
| 10, 10, 5 | q = (0.95125, 0.04494, −0.16773, 0.25489) (the previous rotation rounded to five decimals) | `r1 lx800 ly1200 ox10.00 oy10.00 oz5.00 h330.0 p-0.0 b-20.0` |

The quaternions of the last three rows are inputs as written: each decimal is converted to
the nearest f32 and then decomposed in single precision as in §11.3 and reduced and tested
as in §9.4. The eight-decimal values of rows 9 and 10 are the exact composed rotations
rounded to f32 precision; with them the angle that should vanish stays near
4 · 10⁻⁷ degrees (bank in row 9, pitch in row 10), far below the 0.0002 threshold. Row 11
shows why the precision of a test input matters: rounding the same rotation to five
decimals moves its pitch to −0.00043°, which is above the threshold, so `p` is printed,
as `p-0.0` (one decimal of a small negative value keeps its sign, §9.5). Row 9 rounded to
five decimals would give a bank of about 0.00019°, just inside the threshold; do not use
that rounding as a test input.

The z values of these rows (399.99, 20.0, 12.5, 5, 100.0) are free synthetic offsets, chosen
for the number format; they are not derived from a stored terrain height, and none of them
needs to lie on the terrain. (No stored sample gives 399.99 under §6: 13,106 gives 399.96948
and 13,107 gives 400.0.)

Region 14, landblock (3, 7), offset (19.999, 20.0, 100.0), yaw −90° →
`r14 lx24 ly57 ox20.00 oy20.00 oz100.00 h90.0`.

<a id="s9-8"></a>
### 9.8 Tessera's own position text and parsing

Tessera's `/loc`-style output (client debug line, explorer) produces the token text of
§9.1–§9.5 exactly, so that it can be compared with the original character for character;
the sentence around it is Tessera's own. Its parser of pasted `/loc` text accepts the tokens
in any order (an implementation choice; the original writes them in the order of §9.1) and
inverts §9.2–§9.4: block = ⌊lx / 8⌋, offsets as given, rotation from h, p, b with ψ = −h.

The client also contains an inverse parser (a developer facility; C18, BIN) that takes the
same members plus `bx` and `by` (the block directly) and `c` (a cell number, or a word for
outside or inside). Members may be separated by spaces, commas, semicolons or colons (the
four separator characters the parser splits on). `r` is optional and defaults to region 1,
as the client's own help text for the parser states; `i`, `c`, `ox`, `oy`, `oz`, `h`, `p`
and `b` are optional too, and the block must be given by both `bx` and `by` or both `lx`
and `ly`. The first BIN use read the member table, the separator set and the help text;
the third use (2026-10-07) read the per-member cases and the final assembly, so the defaults
of absent members, including a missing `c` (0, outdoors), are given below (C23). The rest of
the grammar (the bracketed offset form) and the parser's error messages were not
traced, and Tessera does not need them: Tessera's own parser (above) accepts the tokens of
§9.1, separated by any of the same four characters.

**Defaults of the client's inverse parser (C23, BIN 2026-10-07; answers the implementer
OPEN of 2026-10-07).** The parser starts from an invalid position and fills in what the text
gives; for what it does not give:

| Member | Default when absent |
|---|---|
| `r` | 1. A region outside 1–15, or one the client does not know, is an error. |
| block | none: the block must be given by both `bx` and `by` or both `lx` and `ly` (an error otherwise, also when the two schemes are mixed); with landcells, X = ⌊`lx` / 8⌋ and Y = ⌊`ly` / 8⌋; a block index above 255 is an error |
| `i` | 0 |
| `c` | 0 (outdoors). The word `outside` gives 0; `inside` and `limbo` give 1, the generic interior value of §7; otherwise a number |
| `ox`, `oy`, `oz` | 0 each, individually: a text giving only some of them gets 0 for the others. When **none** of them is given, the block was given by `lx` `ly` and the cell is outdoors, the offset is the landcell's south-west corner, ((`lx` mod 8) · 20, (`ly` mod 8) · 20, 0) with the region's landcell size (§5.2); otherwise it is (0, 0, 0), the block corner |
| `h`, `p`, `b` | 0 each; when none is given the rotation is the identity, otherwise it is built from the three angles (ψ = −*h*, §11.3) |

A bracketed form `o[` … `]` that gives the three offsets together also exists; its grammar
was not traced. After assembly the position goes through the normaliser of §4 without
clamping, so an offset beyond the block moves the position into the neighbouring block.

Tessera's parser (above) is not bound by these defaults: the original never writes a partial
`ox oy oz` group or omits `r`, so `tessera-world`'s stricter choices (rejecting a partial
group, requiring `r`) lose nothing when reading the original's own text (Decision, kept).

---

<a id="s10"></a>
## 10. Minimap pixel frame (Confirmed for landblock tiles)

A landblock-properties record (LB §6) gives the tile image and a georeference:
`UI_Map_BlockOffsetX` = bx, `UI_Map_BlockOffsetY` = by, `UI_Map_PixelOffsetX` = px0,
`UI_Map_PixelOffsetY` = py0, `UI_Map_Scale` = s (pixels per world unit). Pixel (px, py) has
its origin at the image's top-left, px to the right, py down. Then

- G_x = 160 · bx + (px − px0) / s
- G_y = 160 · by + (py0 − py) / s

and inversely px = px0 + (G_x − 160 · bx) · s, py = py0 − (G_y − 160 · by) · s.

For every landblock tile in this install bx, by = the record's own X, Y, (px0, py0) =
(0, 200) and s = 1.25, so pixel (0, 0) is the block's north-west corner, pixel (200, 200)
its south-east corner and the image is 200 × 200. Example: in tile (100, 150), pixel
(50, 100) is G = (16,040, 24,080), the point of §8.4 A.

The same `UI_Map_*` properties georeference larger area and dungeon maps (DS §6.7; LC's
`MapUtils` uses the same two formulas). Those are outside M3.

> **OPEN:** whether pixel coordinates address pixel corners (as assumed) or centres.

---

<a id="s11"></a>
## 11. Rotations

<a id="s11-1"></a>
### 11.1 Storage (Confirmed)

Orientations of placed entities (ED §4.4), cells (LB §4.4) and Position values with flag
0x20 (`property-system.md` §4.6) are four f32 forming a unit quaternion, stored in the
order **(w, x, y, z)**: scalar first. The client reads the four values into memory in the
same order and its identity quaternion is (1, 0, 0, 0) (BIN).

Evidence (census of all 3,253,835 SEDs with a placement frame, 2026-10-06): every rotation
has length 1 within 0.001. In 3,115,694 (95.8 %) the second and third stored components
have magnitude below 10⁻⁴, the pattern of a pure turn about the vertical axis when the
scalar is first; with an exact-zero test the count is 3,049,348 (93.7 %), the rest carrying
rounding residue. The scalar-last reading would instead put the near-zeros in the first two
components while the other two are not: only 2,916 rotations have |w| ≤ 10⁻⁶ and
|x| ≤ 10⁻⁶ with both |y| > 10⁻⁶ and |z| > 10⁻⁶ (half turns about an axis in the y–z plane,
genuine combined turns under the scalar-first reading). With "y and z non-zero" in place of
|y|, |z| > 10⁻⁶ the count is 9,908, because rounding residue makes many vertical half turns
(y ≈ 0, z ≈ ±1) qualify.

<a id="s11-2"></a>
### 11.2 Meaning (Confirmed)

The quaternion is an **active rotation from the object's own frame into the landblock
frame**, with the standard (Hamilton) product. For q = (w, x, y, z) the rotation matrix,
rows top to bottom, is:

| | column 1 | column 2 | column 3 |
|---|---|---|---|
| row 1 | 1 − 2(y² + z²) | 2(xy − wz) | 2(xz + wy) |
| row 2 | 2(xy + wz) | 1 − 2(x² + z²) | 2(yz − wx) |
| row 3 | 2(xz − wy) | 2(yz + wx) | 1 − 2(x² + y²) |

A model-space vector v becomes M · v in the landblock frame.

Evidence: for objects placed on slopes with a tilting rotation (|x| or |y| > 10⁻⁴, terrain
slope over 4°), the object's rotated +Z axis lies within 1.5° of the terrain normal (LB
§3.11) in 10,550 cases under this convention against 186 under the conjugate (mirror)
convention.

Placement of a model into the world composes scale, then rotation, then translation:
p_block = position + M · (scale ⊙ p_model), where scale is the uniform vector property
`Render_AdjustableScale` (always uniform and positive in this install; ED §4.5, §5).

<a id="s11-3"></a>
### 11.3 Yaw, pitch and bank (BIN)

The client splits a rotation into three angles, in radians, with the rotation equal to
R_Z(ψ) · R_Y(θ) · R_X(φ) (first φ about +X, then θ about +Y, then ψ about +Z):

- **yaw** ψ = atan2(2(xy + wz), w² + x² − y² − z²) — the matrix entries row 2 / row 1 of
  column 1;
- **pitch** φ = atan2(2(wx + yz), w² − x² − y² + z²) — about the object's +X axis, its
  right-hand side (the object faces +Y, `mesh.md` §11.1);
- **bank** θ = asin(2(wy − xz)) — about the object's +Y axis, its forward axis.

The client evaluates every product and sum in single precision, in this order (C13), and
passes the results to the C runtime's single-precision two-argument arctangent and
arcsine:

| Angle | First argument (f32) | Second argument (f32) |
|---|---|---|
| ψ | the sum x·y + w·z, then that sum added to itself | ((x·x + w·w) − y·y) − z·z |
| φ | the sum w·x + y·z, then that sum added to itself | ((w·w − x·x) − y·y) + z·z |
| θ | (x·z − w·y) multiplied by −2 (arcsine argument) | — |

**Arcsine domain (C20).** The client does **not** clamp the arcsine argument. For a unit
quaternion the exact value lies in [−1, 1], but f32 rounding can push it just outside: in
this install 3,091 of the 3,253,835 stored SED rotations give an argument beyond ±1 in the
order above (2,769 more give exactly ±1, which is in the domain). For such an argument the
runtime's arcsine reports a domain error and returns NaN (C standard behaviour of the
single-precision arcsine), so θ is NaN, the `b` omission test of §9.4 fails, and the
position text would contain `b` followed by the runtime's text for a NaN. The avatar is
always upright (θ = 0), so `/loc` for the player never meets this; it can only appear for
frames printed for objects.

> **OPEN (minor):** the exact characters printed for a NaN bank. Both the arcsine and the
> number formatting come from the system's Universal C runtime, not from the client (C24),
> so the text is the host runtime's spelling of the NaN that its single-precision arcsine
> returns for an argument outside [−1, 1] (sign and payload included). It was not
> established for any runtime.

**Decision (Tessera):** Tessera clamps the arcsine argument to [−1, 1] before taking the
arcsine, everywhere it decomposes a rotation (explorer read-outs, the heading of placed
objects, its own position text). An argument just beyond ±1 therefore gives θ = ±90° and,
in position text, `b90.0` or `b-90.0` where the client would print its NaN text. Yaw and
pitch are unaffected (they use the arctangent). Test vectors (synthetic, f32 inputs):
q = (0.7071068, 0, 0.7071068, 0) gives the argument 1.0000001 in the order above, so the
client's θ is NaN and Tessera's θ is 90° (`b90.0`); q = (0.70710677, 0, 0.70710677, 0)
gives 0.99999994, inside the domain, and both give θ = 89.98° (`b90.0`).

For a pure turn about +Z,
q = (cos(ψ / 2), 0, 0, sin(ψ / 2)) and ψ = 2 · atan2(z, w), **counter-clockwise seen from
above** (from +X towards +Y). The identity (1, 0, 0, 0) leaves model axes aligned with the
world: model +Y (the forward direction of characters, `mesh.md` §11) points north.

<a id="s11-4"></a>
### 11.4 Compass heading (BIN; answers owner item O2)

The `/loc` heading is

- h = (360° − ψ) mod 360°, with ψ the yaw of §11.3 in degrees, reduced as in §9.4,

so h is a **compass bearing measured clockwise from north** (east = 90°, south = 180°, west
= 270°). For a pure turn it is the bearing of the object's model +Y axis, which for a
character is the direction it faces (`mesh.md` §11.1). The same rule applies to every
frame the client prints, so the heading `/loc` shows for an avatar facing exactly along a
placed object's model +Y axis equals that object's h; the bearing formula of
`entity-descriptor.md` §4.4 is the client's.

For a rotation that also tilts the object, h is still −ψ with ψ from the z-y-x
decomposition (the bearing of the rotated model +X axis projected onto the ground, minus
90°), **not** the bearing of the projected model +Y axis. Example: q = (0.8924, 0.36964,
0.23912, −0.09905), a 45° turn about +X followed by 30° about +Y, has ψ = 0 (so `/loc` omits
h and prints `p45.0 b30.0`) while its model +Y axis projects to a bearing of 26.6°.

> **Correction (implementation choice withdrawn):** `tessera-world` takes the yaw of the
> projected model +Y axis (ψ = atan2(−f_x, f_y)). That equals the client's ψ for pure turns
> and for a pitch or bank alone, but not for combined tilts; use §11.3 for compatibility.
> **Done (implementer, 2026-10-06):** `tessera-world`'s `yaw` and `heading_degrees` now
> use the z-y-x decomposition of §11.3 (the same f32 evaluation as `tessera-formats`).

<a id="s11-5"></a>
### 11.5 Worked examples (synthetic)

| q = (w, x, y, z) | Effect | Model +Y goes to | `/loc` angles |
|---|---|---|---|
| (1, 0, 0, 0) | identity | (0, 1, 0), north | h omitted |
| (0.70711, 0, 0, 0.70711) | yaw ψ = 90° | (−1, 0, 0), west | h270.0 |
| (0, 0, 0, 1) | yaw ψ = 180° | (0, −1, 0), south | h180.0 |
| (0.70711, 0, 0, −0.70711) | yaw ψ = −90° | (1, 0, 0), east | h90.0 |
| (0.99619, 0.08716, 0, 0) | tilt 10° about +X | model +Z goes to (0, −0.17365, 0.98481): the top leans south | h omitted, p10.0 |
| (0.8924, 0.36964, 0.23912, −0.09905) | 45° about +X, then 30° about +Y | (0.354, 0.707, 0.612) | h omitted, p45.0, b30.0 |
| (0.70106, 0.0923, 0.43046, 0.56099) | the row above, then yaw 90° | — | h270.0, p45.0, b30.0 |

---

<a id="s12"></a>
## 12. Render space (Decision, ARCH §9.1)

Region-global coordinates reach 40,800, where f32 precision is about 4 mm. Tessera keeps
positions as (region, landblock, f32 block-local) or as f64 globals, and the renderer
works in f32 relative to a camera anchor that is rebased when the camera crosses a landblock
(`docs/ARCHITECTURE.md` §9.1). Vertex data of a landblock is built relative to the
landblock's south-west corner and translated by (160 · (X − X_anchor), 160 · (Y −
Y_anchor), 0) at draw time.

---

<a id="s13"></a>
## 13. Invariants and test vectors

Invariants (Confirmed on this install):

- **CO1**: 0 ≤ X, Y ≤ 254 in every landblock DID and every stored Position with a landblock.
- **CO2** (census over all 3,253,835 SEDs with a frame; "the square" is 0 ≤ x, y ≤ 160;
  "has cells" means the landblock-info record's cell list is not empty):
  - (a) in landblocks **with** cells, every out-of-square SED carries the interior flag
    (768,911 of 768,911; 1,001,648 SEDs with a frame there);
  - (b) in landblocks **without** cells, 2,247,227 of the 2,252,187 SEDs with a frame lie in
    the square; the other 4,960 lie outside it, 4,052 of them with the interior flag and
    908 without (§7);
  - (c) every out-of-square SED **without** the interior flag is in a landblock without
    cells (908 of 908), so 772,963 of all 773,871 out-of-square SEDs (99.9 %) carry the
    flag (ED §4.4, ED7).

  A test of (a)–(c) reproduces these counts; an implementation must not assume that a SED
  outside the square is interior or that a block without cells has only in-square SEDs.
- **CO3**: every stored rotation has unit length within 0.001.
- **CO4**: all seven region descriptors store origin (1,468, 1,244) landcells, offsets
  (0.0, 0.0) and scale 0.005 (f32); each descriptor's offset 4 equals its REGION DID map key.

Invariants of the rules (testable on synthetic input):

- **CO5**: the `/loc` text matches the token sequence of §9.1: `r`, `lx`, `ly`, then the
  optional tokens in that order, single spaces, nothing else.
- **CO6**: outdoors, ⌊lx / 8⌋ = X whenever x < 160; indoors lx mod 8 = 0 and ly mod 8 = 0.
- **CO7**: a printed `h` lies in 0.0 – 360.0; `p` in −180.0 – 180.0; `b` in −90.0 – 90.0.
- **CO8**: the map-coordinate letter depends only on the sign of the unrounded f32 value;
  the number has exactly one decimal and no sign.
- **CO9** (re-homing, §4, C22; region 1, cell 0, f32 arithmetic): X = 10, *x* = 160.0 →
  X = 11, *x* = 0.0; X = 10, *x* = −0.5 → X = 9, *x* = 159.5; X = 10, *x* = 325.0 → X = 12,
  *x* = 5.0; X = 254, *x* = 160.0 → failure without clamping, and with clamping
  *x* = 159.9998, X = 0xFF (invalid); X = 0, *x* = −1.0 → failure without clamping, and with
  clamping *x* = 0.0, X = 0. X = 10, *x* = 170.0, Y = 254, *y* = 160.0 → failure without
  clamping, leaving X = 11, *x* = 10.0, Y = 254, *y* = 160.0 (*x* already re-homed); with
  clamping X = 11, *x* = 10.0, Y = 0xFF, *y* = 159.9998. X = 10, *x* = −10⁻⁶ (as an f32) → X = 10, *x* = 0.0 (the carry undoes the move to block 9); X = 0, *x* = −10⁻⁶ → failure without clamping, and with clamping *x* = 0.0, X = 0 (the range test sees block −1 before the carry); the same holds for *y* and Y. A position with cell ≠ 0 is never moved.
- **CO10** (inverse parser defaults, §9.8, C23): `lx125 ly1100` alone gives region 1,
  X = 15, Y = 137, instance 0, cell 0, offset (100.0, 80.0, 0.0) (landcell 125 mod 8 = 5 and
  1100 mod 8 = 4 times 20), identity rotation; `r1 bx15 by137 ox3` gives offset
  (3.0, 0.0, 0.0).

Test vectors for `crates/world` (all synthetic):

| Input | Expected |
|---|---|
| (R 1, X 100, Y 150, x 40, y 80) → map | latitude −4.0, longitude −66.600006 (f32), text `4.0S, 66.6W` |
| text `12.3S, 45.6W` → landblock | X 126, Y 140, x 80, y 20; height DID 0x80017E8C (R 1) |
| (X 200, Y 170, x 0, y 0) → map | `11.6N, 13.2E` |
| G (29,360, 24,880) → landblock | X 183, Y 155, x 80, y 80; map `0.0N, 0.0E` |
| (X 183, Y 155, x 74, y 76) → map | `0.0S, 0.0W` |
| (X 183, Y 155, x 70.06, y 80) → map | `0.0N, 0.0W` |
| (X 183, Y 155, x 70.03, y 80) → map | `0.0N, 0.1W` |
| (X 100, Y 150, x 40, y 80) → landcell | lx 802, ly 1,204 |
| (X 100, Y 150, x 160, y 159.99), outdoors → landcell | lx 808, ly 1,207 |
| (X 100, Y 150, cell 5, x −300.5, y 412.25) → landcell | lx 800, ly 1,200, `cInside` |
| `/loc` tokens `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99` → position | R 1, X 100, Y 150, (40, 80, 399.99), heading 0 |
| position of the previous row with q (0.70711, 0, 0, 0.70711) → text | `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h270.0` |
| stored height 13,107 → height (§6, f32) | 400.0 units |
| stored height 65,535 / 32,768 / 0 → height (§6, f32) | 2,000.0 / 1,000.01526 / 0.0 units |
| tile (100, 150), pixel (50, 100), px0 0, py0 200, s 1.25 | G (16,040, 24,080) |
| q (0.70711, 0, 0, 0.70711) applied to (0, 1, 0) | (−1, 0, 0); heading 270° |
| §9.7 and §11.5 rows | as listed |
| −ψ in degrees −0.00001, f32 reduction (§9.4) | h = 0, omitted |
| −ψ in degrees −0.02 (§9.4) | `h360.0` |
| q (0.7071068, 0, 0.7071068, 0), Tessera (§11.3) | arcsine argument 1.0000001 clamped to 1, θ = 90°, `b90.0` |
| map input `45.6W, 12.3S` (§8.5) | latitude −12.3, longitude −45.6 |
| map input `-12.3S, 45.6W` (§8.5) | rejected |

The `oz399.99` inputs and outputs of the `/loc` rows above and of §9.7 are free synthetic
offsets, not tied to the stored-height row: a position's z is its own f32 (§6). (Answers the
formats implementation's question of 2026-10-06: stored 13,107 is 400.0 units, as
`tessera-formats` returns; the former 399.99 followed the superseded rule.)

<a id="s14"></a>
## 14. Open questions (summary)

1. Whether the original's movement code keeps the avatar's origin exactly on the terrain
   (`oz` against terrain height, §6); owner item O1.
2. Minimap pixel corners versus centres (§10).
3. Whether some UI panels hide the player's own map coordinates indoors (§8.3; Tessera shows
   them, Decision).
4. Whether the record-to-field match of the locale record (`string-table.md` §10.3) is right
   for the digit alphabet and decimal separator used by map coordinates (§8.3).
5. The exact text printed for a NaN bank, now known to be the host runtime's (§11.3, minor;
   Tessera clamps, Decision).

Closed in the 2026-10-07 revision (BIN third use): the block of a point exactly on an edge
and the far edge of the grid (§4, C22); exact-tie rounding of the `/loc` numbers (§9.5, C24);
the inverse parser's defaults (§9.8, C23; the implementer OPEN of 2026-10-07).

Owner items this revision answers (`docs/checklists/owner-observations.md`):

- **O1** (`/loc` and map coordinates at 20 reference locations): the map formula, its
  per-region constants, the rounding (§8.3: the **first**, rounded form of every row of
  `m3-reference-locations.md` §2.2; recomputed with §8.2–§8.3, 20 of 20), the letters, the
  `/loc` landcell and offset rules and the meaning of `r` (§4, §9) are now BIN facts; the
  2026-10-07 revision adds the edge rule (§4, C22) and the tie rounding (§9.5, C24). Status:
  **partly answered**. The owner's readings remain needed for open points 1 and 3 above
  (the `oz` offset of the avatar above the terrain, which lives in movement code not traced;
  indoor display of map coordinates) and for the area-name column, and are useful as
  confirmation of the rest.
- **O2** (bearing of placed objects against the `/loc` heading): answered by §11.3–§11.4;
  the expected values in O2's table under "Expected `h` if §4.4 is right" are the client's.
  A reading is now only a confirmation (and would reveal heading quantisation, §9.4).
  Status: **answered**. The same rule gives the facings of the static NPC figures of owner
  item O17 (`entity-descriptor.md` §9.4).

---

<a id="s15"></a>
## 15. Corrections to other specs (not edited here)

| Spec | Statement there | Finding (this spec) |
|---|---|---|
| `property-system.md` §4.6 | (Before its 2026-10-05 revision) bits 0x40, 0x80 meaning OPEN; quaternion order OPEN | **Already incorporated there** (its BIN question 5 and the census of 6,115 stored Positions): the client sets 0x01 when the region ≠ 0, 0x02 when X and Y ≠ 0xFF, 0x04 when the instance ≠ 0, 0x08 when the cell ≠ 0xFFFF, **0x40 when the region equals 1**, **0x80 when the cell equals 0 (outdoors)**, 0x10 when any offset component is not exactly 0, 0x20 when the rotation differs from (1, 0, 0, 0) in any component (exact comparisons); the quaternion is (w, x, y, z) (§11.1). This spec's C15 re-read the flag writer and agrees; no correction is left. |
| `environment.md` §3.1 | Offsets 16–32 "Hypothesis, strong"; offsets 24–31 unexplained | BIN: offsets 16 and 20 are the unsigned map origin in 20-unit landcells, 24 and 28 are f32 longitude and latitude offsets in map units (0.0 in all 7), 32 is the f32 scale; the client reads them per region (§8.1–§8.2). Offset 4 equals the REGION DID map key. |
| `string-table.md` §9.4, §10.3 (formerly §14 item 6) | Decimal places and rounding of kind-2 (float) arguments; layout of the locale record | **Agrees** (both specs revised 2026-10-05): the client prints exactly the argument's number of decimals after adding half a unit of the last place plus 0.0002 to the magnitude and cutting the digits (§8.3 here), with digits, decimal separator, grouping and negative form from the locale record (key 0x0B `Language` of the STRINGTABLE map, prefix 0x58), whose layout is `string-table.md` §10.3. For map coordinates the number of decimals is 1 and the value is a magnitude, so grouping and the negative form never apply. The field match of §10.3 there is Inferred (§8.3 OPEN here). |
| `docs/checklists/m3-reference-locations.md` §3 | Rounding versus truncation; whether R5 applies in regions 2–5; whether `r` equals the landblock region | Rounding (the first form of every row; §8.3); R5 applies per region with that region's descriptor, whose constants are equal in all regions (§8.1); `r` is the region number used by the landblock DIDs and the REGION DID map (§4, §9.1). The `oz` height reference stays open (§6). |
| `engine-client-re` notes (ER §4), as quoted in §9 before this revision | `h` omitted within about 1.3° of north | The client's rule is 0.0002 rad (§9.4). |
| `entity-descriptor.md` §8 | Terrain height in metres is raw / 32.768 | Superseded by `landblock.md` §3.4 (row L4): stored × 2,000.0 × f32(1 / 65,535) in f32 (§6 here, C4). The §8 evidence (fitted scale 1 / 32.7665) fits both rules; only the stated formula is out of date. |
| `docs/checklists/m3-reference-locations.md` §1 (row R1) | Terrain height in metres = raw u16 sample / 32.768 | Same correction as the previous row; recomputed `oz` expectations move by at most 0.0305 units. |
| `tessera-world` containing-block rule (§4 here, before 2026-10-07) | G = 40,800 stays in block 254 with x = 160 | The client has no valid block for G = 40,800 (C22): re-homing onto it fails, or with clamping marks the position invalid. Tessera's choice is harmless (the client never produces such a position) and may stay; code that wants the client's behaviour treats it as outside the world. |
| `walker.md` §5–§6 (`WorldSource` location descriptions, HUD lines) | Built on the pre-BIN rules of §8.3 (round half away from zero in f64 with an exact 0.005) and §9 | Follow §8.2–§8.3 and §9 (f32 order, 0.0502 rounding, letters by sign, `ox` group omission, `h` tolerance, `p`/`b`) for character-exact output. |

---

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-04 | all | First version (Review). | Spec writer (dirty room) |
| 2026-10-05 | header, §0, §1 (BIN source, C5–C8, C10–C14), §2, §4, §5.2, §6, §7, §8.1–§8.4, §9 (rewritten as §9.1–§9.8), §11.1, §11.3–§11.5, §13, §14, new §15–§16 | Map-coordinate arithmetic, rounding, letters and text templates; the `/loc` position text; landcells; the yaw/pitch/bank decomposition and the heading rule, all from the client (BIN, decision 0011) and recomputed against the 20 reference rows. Former OPENs on rounding, zero letter, heading sense, interior landcells and tilted headings closed; two implementation choices of `tessera-world` (landcell clamp, projected +Y heading) marked as deviations. Owner items O1 and O2 answered (§14). | Spec writer (dirty room) |
| 2026-10-05 | header, §1 (OBS re-check, BIN second use with questions 5–11, new C15–C21), §4, §7, §8.3, new §8.5, §9.4, §9.8, §11.3, §11.4, §13 (CO2 rewritten, new test vectors), §14, §15 | Repair after review. CO2 was false: it counted only in-square SEDs; it now states the full census (768,911 of 768,911 out-of-square SEDs in blocks with cells are interior; 4,960 of 2,252,187 framed SEDs in blocks without cells are out of the square, 4,052 interior and 908 not; all 908 non-interior out-of-square SEDs are in blocks without cells), and §7 describes the 4,960. BIN facts that lacked provenance now have rows and listed questions (Position flag bits C15; cell values 0, 1 and 0xFFFF and how the client derives them C16; invalid R, X, Y and cell C17; inverse-parser members, separators and the `r` default C18). New BIN facts: §9.4 reductions and omission tests run in f32 with the stated constants (C19); the bank arcsine is not clamped and 3,091 stored rotations leave its domain in f32 (C20); the map-coordinate routine's callers do not test the cell (C21). New OPEN callouts: locale-record field match (§8.3), indoor hiding of the player's map coordinates (§8.3), NaN text of `b` (§11.3); Tessera decisions recorded for each. The lenient map-coordinate input moved from an OPEN in §11.4 to a Decision in new §8.5, with test vectors. | Spec writer (dirty room) |
| 2026-10-06 | header, §7, §9.5, §9.7 | Repair after re-review. §9.7: rows 9 and 10 now give their quaternions to eight decimals (the exact composed rotations rounded to f32), which produce the listed texts under §11.3 and §9.4 with the vanishing angle near 4 · 10⁻⁷°; the former five-decimal input of row 10 is kept as new row 11 with its correct text `… h330.0 p-0.0 b-20.0` (pitch −0.00043°, above the 0.0002 threshold), and a note explains how the inputs are evaluated and why row 9 must not be rounded to five decimals (bank 0.00019°). §9.5: a negative value that rounds to zero prints with its minus sign. §7: the distance of the 908 non-interior out-of-square SEDs is defined as the larger per-axis distance outside the square, with half-open buckets; the Euclidean counts (83 / 193 / 464 / 168) are given for comparison. | Spec writer (dirty room) |
| 2026-10-06 | §1 (OBS, C4), §6 (title and bullets), §9.7, §11.1, §13, §15 | Repair after the second re-review. C4 and §6 now state the client's height rule of `landblock.md` §3.4 (row L4): stored × 2,000.0 × f32(1 / 65,535) in f32, 0 – 2,000.0 units; C4 is marked rule BIN (via LB), scale Confirmed, instead of citing the withdrawn stored / 32.768 as Confirmed. §13: the stored-height vector is now 13,107 → 400.0 (plus 65,535, 32,768 and 0); the implementer OPEN of 2026-10-06 is answered and removed (it is therefore not in §14): the `oz399.99` values of §9.7 and §13 are free synthetic offsets, not derived from a stored height, and §6 and §9.7 say so. §11.1: the evidence now states its tolerances (95.8 % with \|x\|, \|y\| < 10⁻⁴; 93.7 % with exact zeros; the 2,916 scalar-last-looking rotations are those with \|w\|, \|x\| ≤ 10⁻⁶ and \|y\|, \|z\| > 10⁻⁶; 9,908 with "y, z non-zero"), re-counted on the install 2026-10-06. §15: rows for `entity-descriptor.md` §8 and `m3-reference-locations.md` R1, which still state stored / 32.768. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN third use with questions 12–14, new source MSDOC, new C22–C24), §4, §9.5, §9.8, §11.3, §14, §15 | OPEN-closing revision (BIN, decision 0011). §4: the client's general outdoor-position normaliser (C22) settles the edge rule: a point on a shared edge belongs to the block east or north of it, and the grid's far edge G = 40,800 has no valid block; the former OPEN is closed and `tessera-world`'s choice at 40,800 is recorded as harmless (§15). §9.5: the client formats through the system's Universal C runtime with the "standard rounding" option, so on current Windows exact ties round to even (C24; Reported for the runtime via MSDOC); the minor OPEN is closed. §9.8: the inverse parser's defaults for absent members (C23) answer the implementer OPEN of 2026-10-07, which is replaced by the answer; Tessera's stricter parser stays a Decision. §11.3: the NaN-bank OPEN is narrowed to the host runtime. §14: summary renumbered; O1 partly answered, O2 answered. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN repair re-read of question 12; C22), §4, §9.5, §9.8, §13 (CO9) | Repair after the review of 2026-10-07 (changes requested, three items). (1) §4, bullet "Containing landblock of a global point": the stale "either block; pick the lower one" rule is replaced by the client's rule (east or north; G = 40,800 has no valid block), matching C22. (2) §9.8: the paragraph no longer says the default of a missing `c` was not traced; it points to C23 (0, outdoors) and names what remains untraced (the bracketed offset form, error messages). (3) §4 step 4 and C22: the normaliser finishes *x* before it examines *y*, so a failure on *y* leaves *x* and X already re-homed (BIN, re-read); a clamped *x* does not stop the treatment of *y*; new CO9 vector. §9.5: MSDOC's note that the rounding follows the floating-point rounding mode (reviewer's suggestion). | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN second repair re-read of question 12; C22), §4, §13 (CO9) | Second repair, from the note of the passing re-review of 2026-10-07 (not blocking). §4 and C22: the normaliser tests the target block X + *k* before the 160 → 0 carry, so a tiny negative offset in block 0 fails or clamps while in any other block the carry returns it to 0 in its own block (BIN, re-read). CO9: two vectors for that case (X = 10 and X = 0 with *x* = −10⁻⁶), recomputed in f32. No rule changed; status Revised until a re-review. | Spec writer (dirty room) |
