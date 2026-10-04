<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# World coordinates: regions, landblocks, cells, map coordinates, `/loc`, rotations

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/world` (coordinate types, conversions, map-coordinate formatting and parsing), `crates/terrain` (height lookup), `crates/net` sandbox (`--spawn`, reported position), `apps/client` (on-screen coordinates, minimap), `apps/explorer` |
| **Companion specs** | `landblock.md` (landblock records, height scale §3.4, triangulation §3.11, minimap tiles §6). `entity-descriptor.md` §4.4–§6 (placement frame, rotation evidence, placement transform). `environment.md` §3 (region descriptor 0x0F, which stores the map-coordinate constants). `property-system.md` §4.6 (stored Position encoding). `records.md` §2.2 (landblock DIDs). `mesh.md` §11 (model axes and units). |

---

<a id="s0"></a>
## 0. Purpose

This spec fixes every coordinate frame Tessera needs for the offline world walker and the
conversions between them:

- region, landblock, landcell, block-local and region-global positions (§2–§5);
- units, axes and handedness (§3);
- heights (§6) and interior cells (§7);
- the in-game **map coordinates** ("12.3S, 45.6W") and their constants (§8);
- the `/loc` chat output format (§9);
- minimap pixel coordinates (§10);
- rotations stored as quaternions, yaw and compass heading (§11);
- render-space rebasing (§12), invariants and test vectors (§13).

Every worked example uses synthetic numbers chosen for this spec.

---

<a id="s1"></a>
## 1. Provenance

Legend as in `landblock.md` §1: **Confirmed** (checked on the user's install on
2026-10-04 over all records named), **Probable**, **Reported** (third-party only).

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04: all landblock records of regions 1–5 and 14 (`landblock.md` §1), all 7 region descriptors, all 3,253,862 placed static entities, 120 minimap tiles. |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution; decompiled `PositionDecoder`, `GeoLoader`) and `lotro-tools` `MapUtils` (readable source). Facts only. |
| ER | `research/notes/engine-client-re.md` §3–§4: lotro-wiki "Coordinate Systems" page (landcells, `/loc` fields, map-coordinate formula), engine position struct and resource keys. |
| DS | `research/notes/data-semantics.md` §6.1 (Position encoding, 160-unit landblocks, LC's map formula). |
| AYA | aya-po Lotro-Map-Viewer `regions.xml` (no licence; facts only): area bounds written as signed map coordinates, south and west negative. |
| LB | `landblock.md` (this install, same date). |
| ED | `entity-descriptor.md` (this install, same date). |
| ENV | `environment.md` §3 (this install, same date). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| C1 | Landblocks are 160 × 160 units; X, Y in 0–254 per region (§4) | DS, ER; OBS (LB §2.3) | Confirmed |
| C2 | Axes: +X east, +Y north, +Z up, right-handed (§3) | ER; OBS (minimap orientation, terrain seams, layer winding, quaternion test) | Confirmed (east/north naming follows the north-up map) |
| C3 | Block-local x, y measured from the landblock's south-west corner, 0–160; z absolute (§5, §6) | DS, ED; OBS | Confirmed |
| C4 | Height scale stored / 32.768 (§6) | LB §3.4 | Confirmed |
| C5 | Landcells: 8 × 8 per landblock, 20 units each (§5.2) | ER | Reported |
| C6 | Map coordinates: (global − origin) × 0.005, origin (29,360, 24,880) (§8) | LC, ER, DS; OBS (constants stored in every region descriptor) | Confirmed constants; formula Reported (two independent sources) |
| C7 | Map-coordinate signs: north and east positive, displayed with N/S, E/W letters (§8.3) | ER, AYA, LC | Reported |
| C8 | `/loc` token format (§9) | ER | Reported |
| C9 | Minimap tile frame: 1.25 px per unit, top-left = north-west corner (§10) | LC (`MapUtils`); OBS (60,450 properties records, 120 tiles) | Confirmed |
| C10 | Rotations: unit quaternion stored (w, x, y, z), Hamilton product, active rotation from object to landblock frame (§11) | LC (field order), ED; OBS (slope-aligned objects) | Confirmed |
| C11 | Compass heading `h`: 0 = north (§11.4) | ER | Reported; clockwise sense and the yaw relation are derived (OPEN) |

---

<a id="s2"></a>
## 2. Frames at a glance

| Frame | Coordinates | Range | Where used |
|---|---|---|---|
| **Region** | *R* (u8) | 1, 2, 3, 4, 5, 14 have terrain (LB §2.2) | Every position. Regions are separate worlds whose coordinate ranges overlap. |
| **Landblock** | (*R*, X, Y), X and Y u8 | 0 – 254 | Record DIDs `0x80KRXXYY` (LB §2.1), streaming |
| **Block-local** | (x, y, z) f32 inside landblock (X, Y) | x, y in [0, 160] outdoors; z absolute | Stored positions (entities, cells, Position values) |
| **Region-global** ("ox, oy" in LC) | (G_x, G_y) = (160 · X + x, 160 · Y + y), plus z | 0 – 40,800 | Distances across blocks, map coordinates, physics |
| **Landcell** | (lx, ly) integers | 0 – 2,039 | `/loc` output (§9), engine resource keys |
| **Map coordinates** | (latitude, longitude) one-decimal numbers with N/S, E/W | about −124.4 … +79.6 and −146.8 … +57.2 | UI under the minimap (§8) |
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
  X while latitude grows north with Y (§8; LC, AYA).
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
- A **landblock** (X, Y) covers the square 160 · X ≤ G_x ≤ 160 · (X + 1),
  160 · Y ≤ G_y ≤ 160 · (Y + 1). X and Y are 0 – 254, so G_x and G_y are 0 – 40,800.
- The landblock DIDs of (R, X, Y) are `0x80KRXXYY` (LB §2.1): for example the height record of
  region 1, X = 126, Y = 140 is 0x80017E8C.
- **Containing landblock of a global point**: X = ⌊G_x / 160⌋, x = G_x − 160 · X (likewise
  for Y). A point exactly on a shared edge belongs to either block; pick the lower one unless
  the higher index would exceed 254.

The engine's in-memory position (ER §3) is a landblock (u8 X, u8 Y, u16 region) plus
block-local f32 x, y, z, the same split Tessera uses for stored positions.

> **OPEN:** The edge rule above can be read two ways. `tessera-world` uses
> X = ⌊G_x / 160⌋ everywhere (a point on a shared edge belongs to the block east or north
> of it, with x = 0), except that G = 40,800, the far edge of the grid, stays in block 254
> with x = 160. Which block the original reports for a point exactly on an edge is
> unobserved; it changes only `/loc` output and area lookups at that line.

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
### 5.2 Landcells (Reported)

A landblock is divided into 8 × 8 **landcells** of 20 × 20 units. A landcell is addressed
region-wide by

- lx = 8 · X + ⌊x / 20⌋ = ⌊G_x / 20⌋, and ly = 8 · Y + ⌊y / 20⌋ = ⌊G_y / 20⌋;
- conversely X = ⌊lx / 8⌋ and the landcell's south-west corner is at
  G_x = 20 · lx, G_y = 20 · ly.

Range 0 – 2,039. Landcells carry no data of their own in the landblock records; they appear
in the `/loc` output (§9) and in engine resource keys (ER §3). A landcell is **not** the same
thing as an interior cell (§7).

> **OPEN (implementation choice):** For a block-local position with x or y = 160 (or, for
> interiors, outside 0–160) `tessera-world` clamps ⌊x / 20⌋ to 0–7, so the landcell stays
> in the owning block, in line with ER's report that lx, ly stay fixed indoors (§7, §9).

---

<a id="s6"></a>
## 6. Heights (Confirmed)

- z is an **absolute** height in world units, not an offset from the ground (ED §4.4).
- Terrain stores heights as u16 with **height = stored / 32.768** (LB §3.4); 0 – 1,999.97.
  Observed terrain spans 0 to about 1,999 units.
- The terrain height under (x, y) is defined by the triangulation of LB §3.11.
- `/loc` reports this z as `oz` (§9).

---

<a id="s7"></a>
## 7. Interior cells and instances

- A position may name an **interior cell** (u16, non-zero) of its landblock's
  landblock-info record (LB §4.4). Cell geometry is placed in the same landblock frame, but
  interior coordinates are **not** confined to [0, 160]: 768,911 of the 1,001,648 placed
  entities in landblocks that have cells (77 %) lie outside the square, up to about ±960
  units (ED §4.4). Never
  re-home such a position to a neighbouring landblock.
- Positions flagged as outdoors carry cell 0 (`property-system.md` §4.6).
- A position may also carry a u16 **instance** number (Position flag 0x04; `/loc` token `i`).
  It selects an instanced copy of the same space and does not change the coordinates.

> **OPEN:** How the original computes landcells (lx, ly) for an interior position (ER says
> they stay fixed while `ox`/`oy` keep changing), and the instance semantics.

---

<a id="s8"></a>
## 8. Map coordinates (the "12.3S, 45.6W" convention)

<a id="s8-1"></a>
### 8.1 Constants (Confirmed; stored in the data)

Every region descriptor (ENV §3.1; all 7 records identical in these fields) stores:

| Offset | Type | Value | Meaning |
|---|---|---|---|
| 16 | u32 | 1,468 | Map origin, X, in 20-unit landcells → G_x0 = 29,360 |
| 20 | u32 | 1,244 | Map origin, Y, in landcells → G_y0 = 24,880 |
| 32 | f32 | 0.005 | Map units per world unit (one map unit = 200 world units) |

These equal the constants LC hard-codes and the lotro-wiki formula (ER §4). A client reads
them from the region descriptor of the position's region at run time instead of
hard-coding them.

> **Implementation note (`tessera-world`):** the stored f32 nearest 0.005 is
> 0.004999999888…; widened as is, every position on a half tenth (an odd multiple of 10
> units from the origin) would round toward zero under §8.3. `tessera-world` widens the
> f32 through its shortest round-trip decimal, which gives exactly 0.005.

<a id="s8-2"></a>
### 8.2 Formula (Reported, two independent sources; constants Confirmed)

- longitude = (G_x − G_x0) × scale
- latitude = (G_y − G_y0) × scale

with G_x = 160 · X + x and G_y = 160 · Y + y. **Positive longitude is east, positive latitude
is north.** The same constants serve every region, so map coordinates of different regions
overlap; they do not identify a place without the region.

Inverse: G_x = G_x0 + longitude / scale, G_y = G_y0 + latitude / scale, then split into
landblock and block-local as in §4.

Over the landblock range, longitude runs from −146.8 (G_x = 0) to +57.2 (G_x = 40,800) and
latitude from −124.4 to +79.6.

<a id="s8-3"></a>
### 8.3 Display format (Reported)

The UI shows **latitude first, then longitude**, each as an absolute value with one decimal
place followed by a letter: `S` for negative latitude, `N` for positive; `W` for negative
longitude, `E` for positive; the two parts separated by a comma and a space. Example form:
`12.3S, 45.6W`.

> **OPEN:** (1) Rounding versus truncation to one decimal, and the letter used for an exact
> 0.0. (2) Whether the original shows map coordinates for interior positions. Both need one
> observation of the original client. Until then Tessera rounds half away from zero and
> writes 0.0 with `N` / `E`.

<a id="s8-4"></a>
### 8.4 Worked examples (synthetic)

**A. Block-local → map.** Region 1, landblock (100, 150), block-local (40.0, 80.0).

- G_x = 100 · 160 + 40 = 16,040; G_y = 150 · 160 + 80 = 24,080.
- longitude = (16,040 − 29,360) × 0.005 = −66.6 → `66.6W`.
- latitude = (24,080 − 24,880) × 0.005 = −4.0 → `4.0S`.
- Displayed: `4.0S, 66.6W`.

**B. Map → landblock.** `12.3S, 45.6W` means latitude −12.3, longitude −45.6.

- G_x = 29,360 + (−45.6 / 0.005) = 29,360 − 9,120 = 20,240 → X = 126, x = 80.
- G_y = 24,880 + (−12.3 / 0.005) = 24,880 − 2,460 = 22,420 → Y = 140, y = 20.
- Region 1 height record: 0x80017E8C.

**C. North-east quadrant.** Landblock (200, 170), block-local (0, 0): G = (32,000, 27,200);
longitude 13.2, latitude 11.6 → `11.6N, 13.2E`.

**D. Origin.** G = (29,360, 24,880) is landblock (183, 155), block-local (80, 80), map
(0.0, 0.0).

---

<a id="s9"></a>
## 9. The `/loc` output (Reported, ER §4)

The original's `/loc` chat command replies with a sentence that contains these tokens, in
this order (other words of the sentence are not interpreted):

| Token | Example form | Meaning |
|---|---|---|
| `r` + integer | `r1` | Region *R* |
| `lx` + integer, `ly` + integer | `lx802 ly1204` | Landcell (§5.2) |
| `i` + integer (optional) | `i43` | Instance (§7) |
| `cInside` (optional) | | Present when the position is in an interior cell |
| `ox`, `oy`, `oz` + decimal | `ox40.00 oy80.00 oz399.99` | Block-local x, y (block = ⌊lx / 8⌋, ⌊ly / 8⌋) and absolute z, two decimals |
| `h` + decimal (optional) | `h270.0` | Compass heading in degrees, 0 = north (§11.4); omitted within about 1.3° of north |

Rules:

- Region-global position: G_x = 160 · ⌊lx / 8⌋ + ox, G_y = 160 · ⌊ly / 8⌋ + oy.
- Outdoors, ⌊ox / 20⌋ = lx mod 8 and ⌊oy / 20⌋ = ly mod 8 (derived from §5.2).
- Indoors (`cInside`), `ox`/`oy` are unbounded and lx/ly stay fixed (ER).

Example (synthetic, matching §8.4 A with stored terrain height 13,107): region 1, landblock
(100, 150), local (40, 80), z = 13,107 / 32.768 = 399.99 gives the tokens
`r1 lx802 ly1204 ox40.00 oy80.00 oz399.99`.

Tessera's own `/loc`-style output (client debug line) uses the same tokens so that
coordinates can be compared with the original side by side; its sentence wording is
Tessera's own. Its parser of pasted `/loc` text accepts the tokens in any order (an
implementation choice; the original writes them in the order above).

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
order **(w, x, y, z)**: scalar first.

Evidence: 3,253,835 entity rotations all have length 1 within 0.001; in 95.8 % the
second and third components are zero, the pattern of a pure turn about the vertical axis
when the scalar is first (the scalar-last reading would put
the zeros in the first two components; only 2,916 rotations look like that, and those are
genuine combined turns).

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
### 11.3 Yaw

For a pure turn about +Z, q = (cos(ψ / 2), 0, 0, sin(ψ / 2)) and the yaw is
ψ = 2 · atan2(z, w), **counter-clockwise seen from above** (from +X towards +Y). The identity
(1, 0, 0, 0) leaves model axes aligned with the world: model +Y (the forward direction of
characters, `mesh.md` §11) points north.

<a id="s11-4"></a>
### 11.4 Compass heading

The `/loc` heading `h` is reported as degrees with 0 = north (ER). Assuming the usual compass
sense (clockwise from north, east = 90°), the heading of an object whose forward axis is
model +Y is

- h = (360° − ψ) mod 360°, with ψ in degrees.

(ED §4.4 states the same relation as a bearing.)

> **OPEN:** confirm the clockwise sense and this relation with one `/loc` observation of the
> original client while facing a known direction.

> **OPEN (implementation choice):** For a rotation that also tilts the object,
> `tessera-world` takes the yaw of the rotated model +Y axis projected onto the ground
> (ψ = atan2(−f_x, f_y) for the rotated axis f), which equals 2 · atan2(z, w) for a pure
> turn. How the original derives `h` for a tilted avatar or object is unobserved.

> **OPEN (implementation choice):** Map-coordinate input (spawn points, pasted text) is
> parsed leniently by `tessera-world`: lower-case letters, a space before the letter, a
> missing comma, longitude first when both letters are present, or two signed numbers in
> latitude, longitude order. Letters decide the axis; a sign together with a letter is
> rejected. This is Tessera's own input convention, not an observation of the original.

<a id="s11-5"></a>
### 11.5 Worked examples (synthetic)

| q = (w, x, y, z) | Effect | Model +Y goes to | Heading of model +Y |
|---|---|---|---|
| (1, 0, 0, 0) | identity | (0, 1, 0), north | 0° |
| (0.70711, 0, 0, 0.70711) | yaw ψ = 90° | (−1, 0, 0), west | 270° |
| (0, 0, 0, 1) | yaw ψ = 180° | (0, −1, 0), south | 180° |
| (0.70711, 0, 0, −0.70711) | yaw ψ = −90° | (1, 0, 0), east | 90° |
| (0.99619, 0.08716, 0, 0) | tilt 10° about +X | model +Z goes to (0, −0.17365, 0.98481): the top leans south | 0° |

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
- **CO2**: in landblocks without cells, every placed entity has 0 ≤ x, y ≤ 160
  (2,247,227 of 2,247,227); every out-of-square entity belongs to a landblock that has cells,
  and 99.9 % of them carry the interior flag (ED §4.4).
- **CO3**: every stored rotation has unit length within 0.001.
- **CO4**: all seven region descriptors store origin (1,468, 1,244) landcells and scale 0.005.

Test vectors for `crates/world` (all synthetic):

| Input | Expected |
|---|---|
| (R 1, X 100, Y 150, x 40, y 80) → map | latitude −4.0, longitude −66.6, text `4.0S, 66.6W` |
| text `12.3S, 45.6W` → landblock | X 126, Y 140, x 80, y 20; height DID 0x80017E8C (R 1) |
| (X 200, Y 170, x 0, y 0) → map | `11.6N, 13.2E` |
| G (29,360, 24,880) → landblock | X 183, Y 155, x 80, y 80; map (0.0, 0.0) |
| (X 100, Y 150, x 40, y 80) → landcell | lx 802, ly 1,204 |
| `/loc` tokens `r1 lx802 ly1204 ox40.00 oy80.00 oz399.99` → position | R 1, X 100, Y 150, (40, 80, 399.99) |
| stored height 13,107 | 399.99 units |
| tile (100, 150), pixel (50, 100), px0 0, py0 200, s 1.25 | G (16,040, 24,080) |
| q (0.70711, 0, 0, 0.70711) applied to (0, 1, 0) | (−1, 0, 0); heading 270° |

<a id="s14"></a>
## 14. Open questions (summary)

1. Rounding and the zero letter in the map-coordinate display; interior display (§8.3).
2. Clockwise sense of `/loc` heading and its relation to yaw (§11.4).
3. Landcells and instances for interior positions (§7).
4. Minimap pixel corners versus centres (§10).
5. In-game reference points (region, landblock, local x, y, `oz`, map text, heading) for
   exit criteria 4 and 5 of M3; to be recorded here as a table once observed.
