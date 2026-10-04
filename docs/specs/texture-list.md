<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Texture lists, resolution tiers and the surface format census

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` (texture-list decoder), `crates/assets` (level selection, highres fallback), explorer texture viewer |
| **Companion specs** | `records.md` §5 (one render surface, prefix 0x41: header, format codes, pixel data). `dat-container.md` (archive set, DID routing, optional highres archives). `icon-layers.md` (icons, which reference surfaces directly). |

---

<a id="s0"></a>
## 0. Purpose

A render surface (prefix 0x41, `records.md` §5) holds exactly **one** image level. This spec
covers everything above that:

- the **texture-list** record (prefix 0x40), which groups surfaces into a mip chain or a cube
  map (§2–§4);
- how a chain is split between the optional **highres** archives and the **surface**
  archives, and how a reader picks a level when highres data is missing (§5);
- which surfaces are reached **without** a texture list (icons, UI images, font atlases and
  others) (§6);
- a census of every pixel-format code over **all** 0x41 records, so the M1 exit criterion
  "every texture record decodes or is a listed unsupported format" can be checked (§7);
- whether any other record type carries image data (§8).

All counts are **observations** of one install on one date. They change with every patch.
Tests assert the invariants (§9), not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install on 2026-10-04 by throwaway probes in the
  dirty-room scratchpad. A Confirmed fact states what was decoded and how much of it.
- **Reported**: comes only from third-party material and was not, or could not be, checked
  against data.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). All 81,136 texture-list records (81,135 in `client_general.dat`, 1 in `client_local_English.dat`) decoded in full. The 24-byte headers of all 1,408,052 surfaces read across all 8 archives that hold 0x41. Every list level resolved against those headers. Font records (0x42, 48) checked for surface references. Byte-magic scan for embedded images over every record of general, local, gamelogic, mesh, cell_1 and map_1. |
| DU | Middle-earth-Revenge DATUnpacker, `FORMATS/40XXXXXX_format.txt`. Facts only. |
| MV | Mirarkitty `lotro_mesh_viewer` docs (MIT): material → texture-list → surface chain, "first texture-list slot is the diffuse". Facts only. |
| DS | `research/notes/data-semantics.md` §7.4. |
| DC | `dat-container.md` §2 (highres archives are optional). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| T1 | Texture-list layout: DID, kind byte, level-count byte, flag byte, two zero bytes, DID array, two trailing u32 (§2) | DU, DS; OBS (81,136 of 81,136, zero leftover bytes) | Confirmed |
| T2 | Kind 2 = mip chain, kind 4 = cube map with 6 face DIDs (§3) | DU (count 6 for kind 4); OBS (cube faces are square and equal-sized) | Confirmed (shape). Face order is OPEN. |
| T3 | Levels are ordered largest first; each is the previous halved, floor 1 (§4.2) | DS; OBS (81,125 of 81,126 chains) | Confirmed |
| T4 | Every level of a list has the same format and the same category (§4.3) | OBS (all 81,126 chains) | Confirmed |
| T5 | At most one level is in a highres archive, and it is always level 0 (§5.1) | OBS (all 81,136 lists) | Confirmed |
| T6 | Every highres surface is level 0 of at least one texture list and is never reached directly (§5.1) | OBS (77,981 of 77,981) | Confirmed |
| T7 | Highres archives are optional; a reader must cope with level 0 being absent (§5.2) | DC; OBS (structure supports it) | Reported (behaviour); structure Confirmed |
| T8 | Icons and UI images are referenced directly by surface DID, never through a texture list (§6.1) | OBS (all icon references in all 384,948 property sets; see `icon-layers.md`) | Confirmed |
| T9 | Font records end with two A8 surface DIDs (glyph atlases) (§6.2) | OBS (47 of 48 font records) | Confirmed (reference). Font layout is out of scope. |
| T10 | Only the seven format codes of `records.md` §5.2 occur (§7) | OBS (1,408,052 of 1,408,052) | Confirmed |
| T11 | No record type other than 0x41 embeds image bytes (§8) | OBS (scan, every hit checked) | Confirmed for the archives scanned |
| T12 | Materials (0x30) reference texture lists; the first list slot of a material is its diffuse | MV, DS | Reported. Out of scope here; see the future material spec. |

---

<a id="s2"></a>
## 2. Texture-list record (prefix 0x40)

<a id="s2-1"></a>
### 2.1 Where they live (Confirmed)

| Archive | Count | Notes |
|---|---|---|
| `client_general.dat` | 81,135 | All stored **uncompressed** |
| `client_local_<Locale>.dat` | 1 | A localized list. Its DID does **not** also exist in general. Its levels live in the highres and surface archives, like any other list. |

Routing (which archives to search for a 0x40 DID) is in `dat-container.md`. A reader looks in
general first, then local.

<a id="s2-2"></a>
### 2.2 Layout (Confirmed, 81,136 records, zero leftover bytes)

All integers are little-endian (`records.md` §1).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 4 | DID | `did` | The list's own DID (0x40…). Equals the DID looked up (`records.md` §1.6). |
| 4 | 1 | u8 | `kind` | 2 = mip chain (§3.1), 4 = cube map (§3.2). No other value observed. |
| 5 | 1 | u8 | `level_count` *n* | Number of levels. Observed 1–12. |
| 6 | 1 | u8 | `flags` | 0 or 1 (§4.5). No other value observed. |
| 7 | 2 | u16 | `reserved` | Always 0. |
| 9 | 4·*m* | DID × *m* | `surfaces` | Surface DIDs (0x41…). *m* = *n* for kind 2, *m* = 6·*n* for kind 4. |
| 9+4*m* | 4 | u32 | `tail_a` | Always 0 |
| 13+4*m* | 4 | u32 | `tail_b` | Always 15 (0x0F) |

Total size = 17 + 4·*m*. A reader checks this against the payload length exactly.

Notes:

- The DID array starts at the unaligned offset 9.
- DU describes bytes 5–8 as "count byte, word, byte". Data shows the count in byte 5, a flag
  in byte 6, and zeros in bytes 7–8. Reading bytes 5–8 as one u32 gives *n* + 256·flags.
  Readers must not use that u32 as the count.

> **OPEN:** The meaning of `tail_a` (0) and `tail_b` (15). They are constant across all records.
> A reader should log, not reject, other values.

<a id="s2-3"></a>
### 2.3 Example (synthetic)

A three-level DXT1 chain whose surfaces are 0x41ABC000, 0x41ABC001 and 0x41ABC002, stored as
list 0x40ABCDEF:

| Bytes | Field |
|---|---|
| `EF CD AB 40` | `did` = 0x40ABCDEF |
| `02` | `kind` = 2 |
| `03` | `level_count` = 3 |
| `00` | `flags` = 0 |
| `00 00` | `reserved` |
| `00 C0 AB 41` `01 C0 AB 41` `02 C0 AB 41` | three surface DIDs, level 0 first |
| `00 00 00 00` | `tail_a` |
| `0F 00 00 00` | `tail_b` |

Total: 29 bytes = 17 + 4·3.

---

<a id="s3"></a>
## 3. List kinds

<a id="s3-1"></a>
### 3.1 Kind 2: mip chain (Confirmed, 81,126 records)

`surfaces[i]` is mip level *i*. Level 0 is the largest. Level-count histogram:

| *n* | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Lists | 2,413 | 11 | 72 | 703 | 256 | 1,201 | 46,944 | 13,979 | 13,631 | 1,823 | 92 | 1 |

(The local list has *n* = 9.)

In 50,719 chains the level DIDs are consecutive serial numbers. In the rest they are not.
Readers must use the stored DIDs and never derive one level's DID from another.

Lists may share surfaces. 17 surfaces appear more than once across all lists:

- 16 lists have identical bodies apart from their own DID, and so share every level;
- two chains share their levels 2–5;
- one cube-face surface is also level 2 of a chain.

A cache keyed by surface DID must therefore allow one surface to belong to several lists.

<a id="s3-2"></a>
### 3.2 Kind 4: cube map (Confirmed, 10 records)

- `level_count` is 1 in all 10, and there are exactly 6 DIDs: one per face, one level each.
- All six faces of a cube are square, of equal size and of equal format. Observed sizes
  64, 256, 512 and 1024; formats DXT1, DXT5 and R8G8B8.
- In 9 of the 10 cubes the same surface DID appears more than once (a face reused).

The intended layout for *n* > 1 has not been observed. The natural reading, by analogy
with kind 2, is 6 faces × *n* levels, but whether the array is face-major or level-major is
unknown.

> **OPEN:** Face order. The Direct3D 9 cube-face order (+X, −X, +Y, −Y, +Z, −Z) is the
> likely convention but is not confirmed. Since most observed cubes repeat one face image,
> a wrong guess is visible only on the cube with six distinct faces.

> **OPEN:** Array order for a cube with `level_count` > 1.

---

<a id="s4"></a>
## 4. Level rules for mip chains

These hold for kind 2. Dimensions and formats come from each level's own surface header
(`records.md` §5.1). The list itself stores neither.

<a id="s4-1"></a>
### 4.1 Every level resolves (Confirmed)

Every DID in every list (81,136 lists) names an existing 0x41 record. No list contains a
zero DID. Readers must still handle a missing level (§5.2).

<a id="s4-2"></a>
### 4.2 Halving (Confirmed, 81,125 of 81,126 chains)

For every *i* ≥ 1, width(*i*) = max(1, ⌊width(*i*−1) / 2⌋), and the same for height.
Non-square chains keep halving the larger side after the smaller one reaches its floor.
Example (synthetic): 64×16, 32×8, 16×4 for a DXT chain that stops at 4.

One exception: one chain has two 1024×1024 levels. Level 0 is in a highres archive and
level 1 in a surface archive, and halving resumes from level 1. Readers must **not**
reject a chain that breaks the halving rule. They take each level's size from its own header.

<a id="s4-3"></a>
### 4.3 Format and category are constant (Confirmed, all chains)

All levels of one list share the format code and the surface category field
(`records.md` §5.4). This holds even when level 0 lives in a highres archive and the rest in
a surface archive.

<a id="s4-4"></a>
### 4.4 Where a chain stops (Confirmed)

Let the **floor** be 4 for DXT1, DXT3 and DXT5 (one block), and 1 for R8G8B8 and A8R8G8B8.
Then let the **full length** of a chain be the number of halvings from level 0 until the
smaller dimension reaches the floor, plus one.

| Chain class | flags = 0 | flags = 1 |
|---|---|---|
| Full: *n* = full length | 77,925 | 88 |
| Single level: *n* = 1 | 2,406 | 7 |
| Short: 1 < *n* < full length | 73 | 626 |
| Long: *n* > full length | 1 (the §4.2 exception) | 0 |

Consequences for a reader:

- Do not compute the level count. Use `level_count`.
- A DXT chain never goes below 4 in its smaller dimension. Smaller DXT levels are not stored.
- A GPU upload that needs a complete chain must either generate the missing small levels
  or clamp the sampler's maximum LOD to *n* − 1.

<a id="s4-5"></a>
### 4.5 The `flags` byte (Confirmed values, meaning OPEN)

721 lists have flags = 1. In this install every one of them:

- has **no** level in a highres archive (§5.1);
- is a kind-2 chain;
- is usually short (626 of 721), commonly 4 levels with a level-0 width of 2048
  (for example 2048×512 down to 256×64).

> **OPEN:** What flag 1 means: perhaps "partial chain, do not expect a full mip pyramid", or
> a streaming or usage hint. Readers need nothing from it beyond §4.4.

<a id="s4-6"></a>
### 4.6 Shape statistics (Confirmed, informative)

Level-0 format, over all lists (both flag values):

| Format | Lists |
|---|---|
| DXT1 | 48,131 |
| DXT3 | 20,328 |
| DXT5 | 7,642 |
| A8R8G8B8 | 3,137 |
| R8G8B8 | 1,887 |
| A8, JPEG | 0 |

No texture list references an A8 or JPEG surface.

Most common level-0 sizes: 256×256 (46,977), 1024×1024 (12,033), 512×512 (11,927),
512×1024 (1,746), 256×512 (1,630). The largest is 4096 on a side.

---

<a id="s5"></a>
## 5. Resolution tiers: highres and surface archives

<a id="s5-1"></a>
### 5.1 Placement (Confirmed)

| Placement of levels | Lists |
|---|---|
| Level 0 in a highres archive, levels 1… in surface archives | 77,996 |
| All levels in surface archives | 3,130 chains + all 10 cubes |
| Any other arrangement (highres level other than 0, two highres levels, highres mixed in later) | **0** |

Also:

- Every one of the 77,981 surfaces in the four highres archives is level 0 of some list,
  and none is referenced directly by any other record type. 77,980 are level 0 of exactly
  one list. One is level 0 of 16 lists whose bodies are byte-identical apart from their own
  DID (duplicate lists). The 77,981st is level 0 of the local list.
- The lower levels of one list may be spread over up to three surface archives
  (one archive: 54,208 lists; two: 26,480; three: 437).
- Surface DIDs are unique across all archives (`records.md` §5.5). There is never a highres
  and a surface version of the same DID.
- Whether a list has a highres level 0 does not depend on its size. Both groups include
  lists with level 0 from 8 px up to 4096 px.

<a id="s5-2"></a>
### 5.2 Level selection when highres data is missing (behaviour Reported, algorithm normative)

The highres archives are an optional download (DC). An install may lack them, or carry
them out of date. A Tessera reader resolves a mip chain like this:

1. Decode the list (§2). If it fails, the texture is unavailable.
2. For each level *i* in order, look up `surfaces[i]` through the normal 0x41 routing
   (`dat-container.md`).
3. The **effective chain** is the longest run of levels, starting at the first level that
   resolves, in which every level resolves and decodes. If level 0 is missing because no
   highres archive is present, the effective chain starts at level 1.
4. If no level resolves, the texture is unavailable. Report it as a lookup miss, not as
   corruption.
5. A user setting may cap the top resolution. Skip leading levels whose larger dimension is
   above the cap, but always keep at least the last level.

Because §5.1 guarantees that only level 0 can be in highres, a missing highres archive costs
exactly one level, and the remaining chain still halves correctly from level 1.

> **OPEN:** How the original client chooses between level 0 and level 1 when the highres
> archive is present but marked out of date (there is a launcher flag for that state). It
> does not affect decoding.

<a id="s5-3"></a>
### 5.3 Cube maps

The 10 observed cubes keep all faces in surface archives (none in highres). §5.2 applies per
face.

---

<a id="s6"></a>
## 6. Surfaces reached without a texture list

Only 596,236 of the 1,408,052 surfaces (42%) are levels of some texture list. The other
811,816 are referenced directly by DID from other records, or by records not yet
identified.

<a id="s6-1"></a>
### 6.1 Icons and UI images (Confirmed)

Every image-valued property in every property set (prefixes 0x78 and 0x79, 384,948 sets,
`property-system.md` §5) names a **0x41 surface directly**. None names a texture list. Such
a surface is a single image with no mips. Its category field is 15 in every one of these
references. See `icon-layers.md` for how icons are composed.

Category 15 marks UI images (`records.md` §5.4): 36,382 A8R8G8B8, 2,308 R8G8B8 and every
JPEG carry it. Only 54 texture lists have category 15.

<a id="s6-2"></a>
### 6.2 Font glyph atlases (Confirmed reference)

47 of the 48 font records (prefix 0x42, in general and local) end with two u32 DIDs. Each
names an A8 surface, and both surfaces have the same dimensions (for example 512×499). The
48th ends with one such DID followed by a value that is not a surface DID. Glyph atlases are
one consumer of format 28 (A8): there the single channel is glyph coverage.

> **OPEN:** The font record layout. It belongs in a future font spec.

<a id="s6-3"></a>
### 6.3 Unattributed families (Confirmed shapes, referencing record OPEN)

These large families appear in no texture list:

| Family | Count | Archives | Observed content |
|---|---|---|---|
| A8, 128×128, category 0 | 633,470 | surface, surface_aux_1, surface_aux_2 | Greyscale masks with soft edges, consistent with terrain blend masks |
| A8, other sizes (256², 512², odd sizes) | 25,738 | same | Includes the font atlases of §6.2 |
| DXT3, 96×96, category 0 | 41,325 | surface family | RGB content with the alpha channel **0 everywhere** in every sample |
| DXT1, 128×128, category 0 | 5,502 | surface family | Flat or map-like colour tiles |
| JPEG (code 500), category 15 | 65,706 | surface family, local (221) | Maps and large UI images. Some are the targets of map-image properties. |

A raw scan for u32 values equal to these DIDs cannot tell which record type references them:
landblock and mesh records produce too many chance matches. Treat the referencing record
type as unknown.

> **OPEN:** Which record types reference the A8 128×128 masks, the 96×96 DXT3 tiles and the
> 128×128 DXT1 tiles (landblock terrain is the leading candidate), and how a renderer
> interprets the all-zero DXT3 alpha. Decoding them needs nothing beyond `records.md` §5.

---

<a id="s7"></a>
## 7. Pixel-format census (Confirmed, all 1,408,052 surfaces)

<a id="s7-1"></a>
### 7.1 Codes present

Exactly the seven codes of `records.md` §5.2 occur. **No format code is missing from that
spec.** Every surface satisfies RS1 (24 + *L* = payload length) and RS2 (*L* matches the
format's size formula). Every JPEG has width = height = 0.

| Code | Name | Total | In highres | In surface family | In local | Level of a list | Not in any list | Stored compressed |
|---|---|---|---|---|---|---|---|---|
| 0x31545844 | DXT1 | 346,604 | 47,654 | 298,950 | 0 | 341,097 | 5,507 | 244,561 |
| 0x33545844 | DXT3 | 206,910 | 20,165 | 186,745 | 0 | 165,571 | 41,339 | 153,983 |
| 0x35545844 | DXT5 | 60,676 | 7,272 | 53,404 | 0 | 60,676 | 0 | 32,383 |
| 28 | A8 | 659,208 | 0 | 659,208 | 0 | 0 | 659,208 | 654,127 |
| 21 | A8R8G8B8 | 49,243 | 1,059 | 48,182 | 2 | 11,595 | 37,648 | 43,088 |
| 20 | R8G8B8 | 19,705 | 1,831 | 17,874 | 0 | 17,297 | 2,408 | 11,891 |
| 500 | JPEG | 65,706 | 0 | 65,485 | 221 | 0 | 65,706 | 5,443 |

The single DXT3 highres surface that no **general** list references is level 0 of the
**local** list (§2.1). Counted with that list, highres is 100% referenced.

<a id="s7-2"></a>
### 7.2 Format × category

| Category | DXT1 | DXT3 | DXT5 | A8 | A8R8G8B8 | R8G8B8 | JPEG |
|---|---|---|---|---|---|---|---|
| 0 | 332,853 | 134,876 | 25,368 | 659,113 | 5,181 | 12,888 | |
| 1 | 2,133 | 2,872 | 124 | | 10 | 1,015 | |
| 2 | 6 | 6 | | | | | |
| 4 | 5,492 | 19,464 | 6,917 | | 1,241 | 2,672 | |
| 5 | 3,428 | 6,731 | 161 | | 11 | 413 | |
| 6 | 1,570 | 5,356 | 2,535 | | 330 | 195 | |
| 8 | 23 | 631 | 17 | | 18 | | |
| 10 | 965 | 36,435 | 25,458 | | 5,780 | 198 | |
| 11 | 22 | 6 | 2 | | 18 | 16 | |
| 13 | | | | 95 | | | |
| 14 | 60 | 427 | 78 | | 272 | | |
| 15 | 52 | 106 | 16 | | 36,382 | 2,308 | 65,706 |

A blank cell is zero.

<a id="s7-3"></a>
### 7.3 Viewer guidance (M1 exit criterion 1)

Every surface decodes with the rules of `records.md` §5.3, so the explorer has no
"unsupported format" bucket for this install. It must still keep one, for codes that appear
in other data versions (`records.md` §5.2). Suggested sample set for the visual pass/fail
check: one surface of each code, plus one cube face, one level-0 highres surface and its
level 1, one icon with category 15, and one 128×128 A8 mask.

For A8 display, show the channel as greyscale, and offer an "as alpha" toggle (white with
alpha = channel). The two observed uses, masks and glyph coverage, both read naturally that
way.

---

<a id="s8"></a>
## 8. Other image-bearing record types (Confirmed: none)

Every record of general, local, gamelogic, mesh, cell_1 and map_1 (excluding 0x41, 0x0A and
0x25) was searched for JPEG (`FF D8 FF`), DDS (`DDS `), PNG, GIF and BMP signatures.

- All 42 JPEG-signature hits, in mesh records, were chance byte runs inside vertex data:
  none is followed by a JFIF/EXIF segment.
- The one DDS hit, in a landblock record, was inside text.
- The GIF and BMP hits were short chance matches.

So all image data lives in 0x41 records. Other record types reference images by surface DID
(icons, fonts) or by texture-list DID (materials, Reported).

---

<a id="s9"></a>
## 9. Invariants

Real-data tests assert these. Failures are reported per record and do not abort a census.

| # | Invariant | Status |
|---|---|---|
| TL1 | payload length = 17 + 4·*m* (§2.2) | Confirmed, all |
| TL2 | `kind` ∈ {2, 4}; `flags` ∈ {0, 1}; bytes 7–8 = 0 | Confirmed, all (readers warn on other values) |
| TL3 | `tail_a` = 0, `tail_b` = 15 | Confirmed, all (warn only) |
| TL4 | kind 4 ⇒ *m* = 6·*n*; kind 2 ⇒ *m* = *n* | Confirmed, all |
| TL5 | every surface DID has prefix 0x41 and resolves in a full install | Confirmed, all |
| TL6 | all levels of a list share format and category | Confirmed, all |
| TL7 | widths and heights are non-increasing from level 0 | Confirmed, all |
| TL8 | at most one level is in a highres archive, and only level 0 | Confirmed, all |
| TL9 | a DXT level is never smaller than 4 in either dimension | Confirmed, all (follows from `records.md` §5.2) |

---

<a id="s10"></a>
## 10. Synthetic fixtures for implementers

- A three-level DXT1 chain (the §2.3 example) with matching synthetic surfaces.
- A single-level A8R8G8B8 list.
- A kind-4 cube with six distinct 4×4 faces, and one with a repeated face.
- A list whose length disagrees with `level_count` (reject: truncated or trailing bytes).
- A list with `kind` = 3 and with `flags` = 2 (decode, warn).
- A chain whose level 0 is absent from the archive set (effective chain starts at level 1,
  §5.2).
- A chain whose levels do not halve (accept, use the per-level sizes).
- A chain with a zero DID in the middle (the effective chain ends before it).

---

<a id="s11"></a>
## 11. Open questions

1. Meaning of `tail_a`/`tail_b` (§2.2).
2. Cube-face order and multi-level cube layout (§3.2).
3. Meaning of `flags` = 1 (§4.5).
4. Client behaviour with an out-of-date highres archive (§5.2).
5. Font record layout (§6.2).
6. Referencing record types of the A8 masks, 96×96 DXT3 tiles and 128×128 DXT1 tiles, and
   the meaning of the all-zero DXT3 alpha (§6.3).
7. Surface category meanings (`records.md` §5.4). This spec adds that category 15 is the UI
   category, and that a list never mixes categories.

---

<a id="s12"></a>
## 12. Open items raised by the `formats` implementation (2026-10-04)

These came up while implementing the texture-list decoder in `crates/formats` from this spec
alone. The whole-install run of `crates/formats/tests/m1_real_data.rs` decodes all 81,136
lists and finds no TL1–TL9 violation, so none of these choices is exercised by current data.

> **OPEN:** §2.2 gives the DID count only for kinds 2 and 4, and §10 asks readers to decode a
> list with `kind` = 3 and warn. The implementation assumes one DID per level for every kind
> other than 4 (*m* = *n*), checks TL1 with that *m*, and reports the record through
> `has_unusual_fields()`. A different count for an unknown kind is reported as truncated or
> trailing bytes.

> **OPEN:** A list with `level_count` = 0 (17 bytes, no DIDs) is accepted; its effective chain
> is empty, so the texture is unavailable. No such list is observed.

---

<a id="s13"></a>
## 13. Open items raised by the `material` implementation (2026-10-04)

These came up while implementing the mip-chain helpers in `crates/material` (`texture`
module). The real-data test resolves 500 random lists with no TL6, TL7 or TL9 finding.

> **OPEN:** Which level the original client shows for a given on-screen size. The
> implementation picks the smallest level that is at least as large as the target on both
> axes (minify, never magnify), or level 0 when every level is smaller. That is a Tessera
> choice, not an observed client rule.

> **OPEN:** §4.4 defines a full chain by the *smaller* side and the DXT floor of 4, while a
> GPU mip pyramid runs to 1×1 on the *larger* side. The implementation reports the
> difference (`MipPlan::missing`, typically 2 for a square DXT chain) and the LOD clamp
> (`uploadable − 1`), and leaves it to the renderer to generate small levels or clamp the
> sampler. How the original client fills or clamps those levels is unknown.

> **OPEN:** A GPU texture needs mip *i* to be exactly the halved size of mip 0, so a chain
> that breaks §4.2 cannot be uploaded level for level. The implementation uploads the
> longest run of exactly halving levels that starts at a level of level 0's size, capped at
> the GPU mip count. For the one §4.2 exception (two 1024×1024 levels) that is the run from
> level 1, so level 0 is not uploaded and nothing is lost. Which copy the original client
> uses is unknown.
