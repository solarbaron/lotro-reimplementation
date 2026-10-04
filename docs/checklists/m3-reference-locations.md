<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M3 reference locations (exit criteria 4 and 5)

| | |
|---|---|
| **Status** | Expected values complete; walker side verified 2026-10-04 (20 of 20, [§4.3](#s4-3)); owner observations pending |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Serves** | [ROADMAP M3](../ROADMAP.md#m3) exit criterion 4 (terrain height and object placement at 20 reference locations) and criterion 5 (area name and map coordinates agree with the original client at the same locations) |
| **Companion specs** | `entity-descriptor.md` §4.4 (SED frame), §5 (placement transform), §7 (scale), §8 (height scale); `wstate.md` §11.2 (area DID → name); [`landblock.md`](../specs/landblock.md) and [`coordinates.md`](../specs/coordinates.md) (normative) |
| **Owner list** | Every row's empty columns are an item of [`owner-observations.md`](owner-observations.md#o1) |

Twenty places in five regions where the data predicts exact facts: a terrain height, the
corner of a placed object, the area the landblock belongs to, and the map coordinates the
original client should show. The world walker is checked against the "expected" columns;
the original client is checked by the project owner, who fills in the empty columns.

Text only. No game text is recorded here: areas are named by their DID, objects by their
physics-object DID and by a kind in our own words. The owner must not paste area names,
server names or screenshots into this file (see [§4](#s4)).

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the owner's install, data current on 2026-10-04, read in place by throwaway dirty-room probes. All records of each type listed in §1.1 were decoded. |
| ED | `docs/specs/entity-descriptor.md` (SED layout, frame, quaternion order, scale, terrain validation). |
| WS | `docs/specs/wstate.md` §11 (area records). |
| ER | `research/notes/engine-client-re.md` §4 (`/loc` output fields, map-coordinate formula; lotro-wiki "Coordinate Systems"). |
| DS | `research/notes/data-semantics.md` §6.1 (map-coordinate formula), §6.5 (block maps and their descriptor records). |
| LT | LotroCompanion `lotro-tools` block-map and descriptor loaders, read for facts only (which descriptor record serves which region). |

| # | Fact used by this checklist | Sources | Mark |
|---|---|---|---|
| R1 | Terrain height in metres = raw u16 sample / 32.768; sample (*i*, *j*) at index 33 · *i* + *j*, *i* along *x*, 5 m spacing | ED §8; OBS (§1.1) | Confirmed |
| R2 | SED position is in the owning landblock's frame, *x* east, *y* north, *z* absolute height; quaternion stored (w, x, y, z), active | ED §4.4, §6 | Confirmed |
| R3 | Each landblock's block map (0x801RXXYY) holds 28 (key, index) pairs; key 0x1000000D selects `Area_DID` from the region's descriptor record | DS §6.5, LT; OBS (§1.1) | Confirmed |
| R4 | Descriptor record per region: 1 → 0x18000000; 2 → 0x18000014; 3 → 0x18000015; 4 and 5 → 0x1800001A | LT; OBS (§1.1: index fit) | Reported (LT), consistent with the data; regions 1–3 and 5 Confirmed by a tight index fit, region 4 consistent only. `landblock.md` should state how the client finds the record. |
| R5 | Map coordinates: latitude = (160 · *Y* + *y* − 24880) / 200, positive = N; longitude = (160 · *X* + *x* − 29360) / 200, positive = E; shown as "lat, lon" with one decimal | DS §6.1, ER §4 | Reported; rounding and the use in regions 2–5 are OPEN (§3) |
| R6 | `/loc` prints `r` region, `lx`/`ly` landcells (8 per landblock, 20 m each: `lx` = 8 · *X* + ⌊*x* / 20⌋), `ox`/`oy` the offset inside the landblock, `oz` the absolute height | ER §4 | Reported |

<a id="s1-1"></a>
### 1.1 Census behind the expected values (OBS)

| Record family | Decoded | Result |
|---|---|---|
| Landblock data 0x800RXXYY, regions 1–5 | 325,125 (65,025 per region: *X*, *Y* = 0–254) | Every record has grid count 1,089 (33 × 33) at offset 12. |
| Block maps 0x801RXXYY, regions 1–5 | 325,125 | Every record: 28 pairs, no bytes left over. Keys 0x3, 0x4, 0x6, 0x7 and (where present) 0x10000012 have no entry in any of the five descriptor records (0x18000000, …14, …15, …1A, …1E); with the R4 assignment key 0x5 indexes past its list in region 1 and key 0x10000010 in regions 3–5. Key 0x1000000D (`Area_DID`) is always in range. |
| R4 index fit | 5 regions × 5 descriptor records | Highest `Area_DID` index used per region, against the list lengths: region 1 uses 270 and only 0x18000000 (271 entries) is long enough; region 2 uses 191 = last entry of 0x18000014 (192; 0x18000000 is long enough but not tight); region 3 uses 53 = last of 0x18000015 (54); region 5 uses 156 = last of 0x1800001A (157); region 4 uses 147 of 0x1800001A's 157. |
| Area DIDs per landblock | 325,125 | 610 distinct values. 0x70003D6F (index 0 of every descriptor's Area_DID list) covers 287,788 landblocks; it is the default and no reference location uses it. |
| Landblock info 0x802RXXYY, regions 1–5 | 47,371 | 0 decode failures; 3,216,948 SEDs. |
| Height scale (R1) per region | 351,371–642,350 in-block SEDs per region | Least-squares slope on SEDs within 0.5 m of raw / 32.768: 1 / 32.7669 to 1 / 32.7682, intercept −0.002 to +0.016 m, in all five regions. |

---

<a id="s2"></a>
## 2. The reference locations

<a id="s2-1"></a>
### 2.1 How the places were chosen

1. All Physics SEDs (type 7, ED §4.1) without the interior bit, with a position inside the
   landblock square at least 10 m from its edges: 1,192,860.
2. Kept: SEDs whose world-builder label (read only in the dirty room) marks a building,
   tower, statue or obelisk, fountain, well, gateway or bridge; no `Entity_ContentLayers`
   property, so the object exists whatever content layer is active (ED §4.5.1); a pure
   turn about +Z; a resolvable part list (`setup.md` §3.1); an area other than the default;
   no zero samples in the landblock's height grid: 90,351.
3. Geometry, with the SED scale applied: footprint at least 6 m × 6 m (3 m for the other
   kinds) and 3 m tall; terrain under the four rotated bottom corners within 1.0 m of each
   other; mesh bottom between 1.5 m below and 0.25 m above the lowest of those terrain
   heights (the object sits on the ground); a **stand point** 1.5 m outside one corner,
   along the corner's diagonal, where the terrain varies by less than 0.4 m within 3 m and
   which lies at least 3 m inside the landblock: 6,237.
4. Only landblocks without cells, and only areas that cover at least 40 landblocks (open
   country, not small instanced spaces). Best candidate per area; then 20 picked by hand
   for spread over regions, areas and kinds.

At every stand point bilinear interpolation and both triangle splits of the grid cell
agree within 0.05 m, so the rows do not depend on the terrain interpolation rule, which
`landblock.md` settles.

<a id="s2-2"></a>
### 2.2 Expected facts and owner columns

Landblock = 0xXXYY (*X*, *Y* of the landblock in region *R*). Local *x*, *y* = the stand
point in the landblock frame (R2). Terrain height = bilinear terrain at the stand point
(R1). Map coordinates rounded to one decimal; where truncation would give a different
string it follows in brackets (§3). Anchor = the physics object (0x47) named by the SED; §2.3
gives the SED itself.

| ID | Region | Landblock | Local *x*, *y* (m) | Expected `/loc` fields | Map coordinates | Terrain height (m) | Area DID | Anchor (physics object) | Observed `/loc` | Observed area name | Pass/fail |
|---|---|---|---|---|---|---|---|---|---|---|---|
| M3-01 | 1 | 0x5378 | 56.36, 56.59 | `r1 lx666 ly962 ox56.36 oy56.59 oz455.05` | 28.1S, 80.1W | 455.05 | 0x700605C5 | 0x47000E92 | | | |
| M3-02 | 1 | 0xA197 | 137.35, 111.58 | `r1 lx1294 ly1213 ox137.35 oy111.58 oz389.25` | 3.0S, 17.3W | 389.25 | 0x700693CE | 0x470003D3 | | | |
| M3-03 | 1 | 0x6D8D | 62.96, 15.07 | `r1 lx875 ly1128 ox62.96 oy15.07 oz413.60` | 11.5S, 59.3W (11.5S, 59.2W) | 413.60 | 0x70003DA6 | 0x470003F2 | | | |
| M3-04 | 1 | 0x9B40 | 127.57, 145.41 | `r1 lx1246 ly519 ox127.57 oy145.41 oz580.57` | 72.5S, 22.2W (72.4S, 22.1W) | 580.57 | 0x70020A43 | 0x47001411 | | | |
| M3-05 | 1 | 0xA68E | 123.99, 113.58 | `r1 lx1334 ly1141 ox123.99 oy113.58 oz526.81` | 10.2S, 13.4W (10.2S, 13.3W) | 526.81 | 0x7006DD10 | 0x47009706 | | | |
| M3-06 | 2 | 0x6EA5 | 116.64, 69.58 | `r2 lx885 ly1323 ox116.64 oy69.58 oz517.46` | 7.9N, 58.2W | 517.46 | 0x70052188 | 0x470093DF | | | |
| M3-07 | 2 | 0x5C58 | 155.67, 112.37 | `r2 lx743 ly709 ox155.67 oy112.37 oz311.55` | 53.4S, 72.4W | 311.55 | 0x70032F44 | 0x4700618F | | | |
| M3-08 | 2 | 0x81CC | 7.34, 132.21 | `r2 lx1032 ly1638 ox7.34 oy132.21 oz692.96` | 39.5N, 43.6W (39.4N, 43.5W) | 692.96 | 0x7004E2AE | 0x470091C8 | | | |
| M3-09 | 2 | 0x7B68 | 131.54, 30.72 | `r2 lx990 ly833 ox131.54 oy30.72 oz432.50` | 41.0S, 47.7W | 432.50 | 0x7002CA68 | 0x47005EEE | | | |
| M3-10 | 3 | 0x693E | 19.47, 44.71 | `r3 lx840 ly498 ox19.47 oy44.71 oz225.37` | 74.6S, 62.7W (74.5S, 62.7W) | 225.37 | 0x7003EE0B | 0x47006DFD | | | |
| M3-11 | 3 | 0xA73F | 126.67, 41.80 | `r3 lx1342 ly506 ox126.67 oy41.80 oz311.98` | 73.8S, 12.6W (73.7S, 12.5W) | 311.98 | 0x7004290A | 0x47007788 | | | |
| M3-12 | 3 | 0x973B | 118.38, 132.54 | `r3 lx1213 ly478 ox118.38 oy132.54 oz175.87` | 76.5S, 25.4W | 175.87 | 0x7004290B | 0x47004B6E | | | |
| M3-13 | 3 | 0x622F | 147.76, 107.10 | `r3 lx791 ly381 ox147.76 oy107.10 oz217.66` | 86.3S, 67.7W (86.2S, 67.6W) | 217.66 | 0x70046C64 | 0x4700711B | | | |
| M3-14 | 4 | 0xC755 | 33.74, 131.37 | `r4 lx1593 ly686 ox33.74 oy131.37 oz155.40` | 55.7S, 12.6E (55.7S, 12.5E) | 155.40 | 0x70047BAC | 0x470085DA | | | |
| M3-15 | 4 | 0xBE69 | 129.59, 126.29 | `r4 lx1526 ly846 ox129.59 oy126.29 oz298.49` | 39.8S, 5.8E (39.7S, 5.8E) | 298.49 | 0x70046CA4 | 0x47007FBA | | | |
| M3-16 | 4 | 0x5659 | 47.58, 123.44 | `r4 lx690 ly718 ox47.58 oy123.44 oz229.98` | 52.6S, 77.8W (52.5S, 77.7W) | 229.98 | 0x70066142 | 0x4700175A | | | |
| M3-17 | 5 | 0x267B | 116.31, 34.90 | `r5 lx309 ly985 ox116.31 oy34.90 oz192.26` | 25.8S, 115.8W | 192.26 | 0x700671C5 | 0x4700D0B5 | | | |
| M3-18 | 5 | 0x757A | 79.02, 66.36 | `r5 lx939 ly979 ox79.02 oy66.36 oz228.76` | 26.5S, 52.8W (26.4S, 52.8W) | 228.76 | 0x7006B251 | 0x4700E31C | | | |
| M3-19 | 5 | 0x536C | 81.41, 98.01 | `r5 lx668 ly868 ox81.41 oy98.01 oz194.52` | 37.5S, 80.0W (37.5S, 79.9W) | 194.52 | 0x7006B247 | 0x4700E7C3 | | | |
| M3-20 | 5 | 0x4280 | 118.65, 50.38 | `r5 lx533 ly1026 ox118.65 oy50.38 oz158.94` | 21.7S, 93.4W | 158.94 | 0x700671C4 | 0x4700D3B8 | | | |

Region balance: 5 + 4 + 4 + 3 + 4 rows; 20 distinct area DIDs. Landblock 0x693E (M3-10)
also exists in region 4 with the same object at the same SED index; the row means the
region 3 copy.

<a id="s2-3"></a>
### 2.3 Anchor geometry (object placement check)

The **corner** is the bottom corner of the anchor's part bounds (union of its meshes'
bounds, `mesh.md`), scaled, turned by ψ and moved to the SED position (ED §5), in the
landblock frame. "Side" is the corner's compass direction seen from the footprint centre.
The stand point of §2.2 is 1.5 m outside this corner. Yaw ψ is counter-clockwise from
above (ED §4.4). Mesh bottom and top are the scaled bounds' lowest and highest *z* added to
the SED *z*. Exact map value = the unrounded R5 latitude and longitude of the stand point.

| ID | Kind (our words) | Landblock info, SED index | IID | Scale | Yaw ψ (°) | SED *x*, *y*, *z* | Footprint (m) | Corner *x*, *y* (side) | Terrain at corner | Mesh bottom / top (m) | Exact map value (lat, lon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| M3-01 | barn | 0x80215378 #0 | 0x0115378000000001 | 1.00 | 139.5 | 38.08, 55.79, 455.12 | 24.6 × 22.9 | 54.87, 56.48 (E) | 455.05 | 453.87 / 467.36 | −28.117, −80.118 |
| M3-02 | house | 0x8021A197 #23 | 0x011A197000000018 | 1.00 | 0.0 | 127.81, 102.47, 389.26 | 17.0 × 16.2 | 136.29, 110.52 (NE) | 389.25 | 389.16 / 407.78 | −3.042, −17.313 |
| M3-03 | fountain | 0x80216D8D #70 | 0x0116D8D0000000FA | 1.00 | 45.0 | 71.88, 15.08, 413.76 | 10.5 × 10.5 | 64.46, 15.07 (W) | 413.60 | 413.41 / 423.93 | −11.525, −59.285 |
| M3-04 | stone gateway | 0x80219B40 #19 | 0x0119B40000000018 | 1.20 | 25.5 | 120.58, 146.04, 580.62 | 12.9 × 7.6 | 126.16, 145.92 (E) | 580.57 | 579.79 / 589.15 | −72.473, −22.162 |
| M3-05 | watchtower | 0x8021A68E #26 | 0x011A68E00000001B | 1.00 | 106.0 | 130.07, 110.31, 526.54 | 7.5 × 7.8 | 125.30, 112.85 (NW) | 526.79 | 526.00 / 539.46 | −10.232, −13.380 |
| M3-06 | long barn | 0x80226EA5 #20 | 0x0126EA5000000038 | 1.00 | 307.3 | 139.60, 72.83, 517.46 | 17.2 × 38.4 | 118.12, 69.38 (W) | 517.46 | 516.60 / 531.31 | +7.948, −58.217 |
| M3-07 | stable | 0x80225C58 #46 | 0x0125C58000000074 | 1.00 | 165.0 | 145.92, 109.01, 311.84 | 15.0 × 9.4 | 154.37, 111.62 (E) | 311.55 | 311.16 / 321.37 | −53.438, −72.422 |
| M3-08 | broken statue | 0x802281CC #128 | 0x01281CC000000108 | 1.00 | 180.0 | 18.61, 140.29, 693.06 | 20.4 × 14.0 | 8.40, 133.27 (SW) | 692.96 | 692.96 / 722.09 | +39.461, −43.563 |
| M3-09 | tower | 0x80227B68 #72 | 0x0127B6800000006D | 1.00 | 45.0 | 121.88, 30.72, 432.63 | 11.5 × 11.6 | 130.04, 30.72 (E) | 432.50 | 432.19 / 465.32 | −41.046, −47.742 |
| M3-10 | two-storey building | 0x8023693E #15 | 0x013693E000000012 | 1.00 | 75.0 | 37.79, 50.11, 225.19 | 17.8 × 30.6 | 20.77, 45.46 (W) | 225.37 | 224.83 (level 0: 225.09, [§4.3](#s4-3)) / 257.05 | −74.576, −62.703 |
| M3-11 | large building | 0x8023A73F #523 | 0x013A73F0000002B1 | 1.00 | 90.0 | 107.94, 26.65, 311.99 | 28.1 × 35.3 | 125.61, 40.74 (NE) | 311.98 | 311.92 / 348.32 | −73.791, −12.567 |
| M3-12 | very tall statue | 0x8023973B #81 | 0x013973B000000074 | 1.00 | 345.0 | 130.07, 118.32, 175.69 | 27.7 × 18.0 | 119.13, 131.25 (NW) | 175.87 | 175.51 / 253.18 | −76.537, −25.408 |
| M3-13 | obelisk | 0x8023622F #38 | 0x013622F000000030 | 0.70 | 206.7 | 146.57, 103.02, 217.61 | 3.7 × 4.1 | 147.29, 105.68 (N) | 217.64 | 216.25 / 231.41 | −86.265, −67.661 |
| M3-14 | long building | 0x8024C755 #40 | 0x014C7550000000BD | 1.00 | 225.0 | 16.94, 135.56, 155.45 | 15.5 × 27.6 | 32.24, 131.37 (E) | 155.40 | 155.31 / 169.98 | −55.743, +12.569 |
| M3-15 | statue with basin | 0x8024BE69 #61 | 0x014BE6900000007E | 1.00 | 90.0 | 121.83, 135.15, 300.54 | 15.7 × 13.7 | 128.53, 127.35 (SE) | 298.49 | 298.37 / 332.19 | −39.769, +5.848 |
| M3-16 | ruined tower | 0x80245659 #156 | 0x014565900000009D | 1.75 | 279.8 | 37.79, 130.36, 229.98 | 14.9 × 14.9 | 46.35, 124.31 (SE) | 229.98 | 229.50 / 243.72 | −52.583, −77.762 |
| M3-17 | arched building complex | 0x8025267B #6 | 0x015267B00000000F | 1.00 | 15.0 | 95.08, 42.65, 192.16 | 34.9 × 23.8 | 115.01, 35.65 (E) | 192.26 | 192.06 / 229.93 | −25.826, −115.818 |
| M3-18 | two-storey building | 0x8025757A #6 | 0x015757A00000000A | 1.00 | 0.0 | 66.81, 54.55, 228.49 | 22.4 × 20.2 | 77.96, 65.30 (NE) | 228.76 | 228.41 / 252.75 | −26.468, −52.805 |
| M3-19 | animal statue | 0x8025536C #224 | 0x015536C000000122 | 1.00 | 90.0 | 76.32, 89.51, 194.55 | 14.9 × 8.1 | 80.35, 96.95 (NE) | 194.52 | 194.53 / 213.76 | −37.510, −79.993 |
| M3-20 | farmhouse | 0x80254280 #37 | 0x015428000000005A | 1.00 | 0.0 | 110.28, 45.15, 158.94 | 16.1 × 8.2 | 117.59, 49.32 (NE) | 158.94 | 158.55 / 164.48 | −21.748, −93.407 |

---

<a id="s3"></a>
## 3. Open points this checklist is meant to close

> **Resolved (walker side, 2026-10-04):** these values were computed from R5 and R6 before
> `coordinates.md` existed. Recomputed with `coordinates.md` §8.3 and §9 (§4.3): no change
> in the `/loc` column; the map-coordinate rounding rule is the next item.

> **OPEN:** Map-coordinate rounding. Twelve rows give different strings for rounding and
> truncation; the owner's readings of those rows decide the rule. Record the exact string
> the client shows.

> **OPEN:** Whether R5 applies unchanged in regions 2–5 (ER §4 reports that the regions'
> coordinate spaces overlap; no per-region offset is known). Rows M3-06 to M3-20 test it.
> A constant difference per region would point to a per-region offset in the map data.

> **OPEN:** The avatar's height reference. `/loc` `oz` at a stand point is expected to equal
> the terrain height; a constant offset over all rows (for example a fraction of a metre)
> would be the difference between the avatar origin and the ground, to be recorded in
> `coordinates.md`.

> **OPEN:** Whether `/loc` `r` equals the landblock DID's region number in every region
> (WS §11.3 lists region numbers 1, 2, 3, 4, 14 and 5).

---

<a id="s4"></a>
## 4. Procedure

<a id="s4-1"></a>
### 4.1 Walker side (implementer, criterion 4 and the walker half of 5)

For each row, spawn the sandbox avatar in the landblock (`--spawn R,X,Y`, ROADMAP M3), move
it to the stand point (or query the world model there directly in a real-data test) and
check:

1. Terrain height at the stand point equals the "Terrain height" column within 0.05 m.
2. The anchor SED (§2.3, by landblock info and index) is drawn, and the transformed bottom
   corner of its part bounds lies within 0.1 m of "Corner *x*, *y*" and its bottom and top
   within 0.1 m of "Mesh bottom / top".
3. The on-screen map coordinates equal the "Map coordinates" column (either form until §3
   is resolved).
4. The on-screen area name is the name of the row's area DID, resolved by WS §11.2.

<a id="s4-2"></a>
### 4.2 Original-client side (owner, criterion 5)

Item [O1 of the owner list](owner-observations.md#o1). For each row the owner can reach:

1. Travel to the map coordinates. Find the anchor by its kind (§2.3) next to the stand
   point, then the corner on the listed side. Stand on open ground touching that corner
   from outside, not on any steps, path edging or other object.
2. Type `/loc`. Copy only the numeric fields into "Observed `/loc`": `r`, `lx`, `ly`, any
   `i` or `c` field, `ox`, `oy`, `oz`, `h`. Leave out the server name and the timestamp.
3. "Observed area name": do **not** write the name (game text). Write `= walker` if the
   name shown with the minimap equals the walker's name for that row's area DID, or
   `≠ walker` and keep the name in a private note outside the repository. If the walker is
   not ready yet, write `seen` and keep the name privately until it can be compared.
4. Note the map-coordinate string shown under the minimap after the `/loc` fields, in the
   same cell (numbers and N/S/E/W only).
5. Pass/fail: **pass** when `r`, `lx` and `ly` equal the expected values; `ox` and `oy` are
   within 2 m of the expected values (standing slack); `oz` is within 0.5 m of the
   terrain height (recompute it for the observed `ox`, `oy` if they differ by more than
   1 m); the map coordinates equal either expected form; and the area name matches.
   Otherwise **fail**, with a short reason in our own words. A row the owner cannot reach
   (content not owned, or `/loc` shows an instance or interior field) is `n/a`.

No screenshots are committed. Captures, if any, stay private in the dirty room.

<a id="s4-3"></a>
### 4.3 Walker-side result (implementer, 2026-10-04)

`apps/client/tests/reference_locations.rs` runs the §4.1 checks for all 20 rows against the
world model the walker draws from (streamer settled at the stand point; terrain height,
anchor SED and its object, IID, scale and yaw, the placed part bounds, the map-coordinate
string, the `/loc` tokens and their parse back to the stand point, the block map's
`Area_DID` and its resolved name):
`cargo test --release -p tessera-client --test reference_locations -- --nocapture`.

| Check | Result |
|---|---|
| Terrain height at the stand point and at the corner (±0.05 m) | 20 of 20 |
| Anchor SED drawn, object, IID, scale, yaw and position as in §2.3 | 20 of 20 |
| Bottom corner (±0.1 m), mesh bottom and top (±0.1 m) | 20 of 20, with the M3-10 erratum below |
| Map coordinates | 20 of 20; the walker (`coordinates.md` §8.3, round half away from zero) shows the first form of every row, so the twelve rows with a bracketed form decide the rounding rule when the owner reads them |
| `/loc` tokens | 20 of 20 equal the expected column, recomputed by `coordinates.md` §9 (closes the first item of [§3](#s3) for the walker side: no change) |
| `Area_DID` and name | 20 of 20 resolve (names not recorded) |

**Erratum, M3-10 "Mesh bottom":** 224.83 m is the union of every submesh box of the part,
including lower detail levels. `mesh.md` §6.1 bounds a mesh by the submeshes its level 0
draws, which gives a bottom of 225.09 m (top unchanged); the test uses `mesh.md`. The row's
corner is within 0.1 m under both rules; the other 19 rows pass with the level-0 rule as
listed.

Two rows were also viewed in the walker with `--place "<expected /loc>"` (M3-01, M3-12): the
avatar stands at the anchor's listed corner and the HUD shows the expected map coordinates
and `/loc` tokens. The owner columns of §2.2 stay empty until [O1](owner-observations.md#o1)
is done.
