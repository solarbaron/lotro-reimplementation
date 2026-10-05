<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Texture lists, resolution tiers and the surface format census

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested. Spot checks against the install agree on T1–T5, T10, TL1–TL10, the level-count histogram, the `flags` × placement table of §4.5, the 10 cubes and the sky-box face content, the §4.2 exception, the 77,981 highres surfaces and the format totals of §7.1. Issues (all count or scope errors in Confirmed statements; no rule is affected): (1) §5.1 archive spread does not reproduce: 54,398 / 26,234 / 493 lists with levels in one / two / three surface archives, not 54,208 / 26,480 / 437 (the writer's own 2026-10-05 dump gives the same as the review). (2) §6 and §7.1 count only the 81,135 general lists: over all 81,136 lists 596,245 surfaces are list levels and 811,807 are not, and the DXT3 "Level of a list" / "Not in any list" cells are 165,580 / 41,330; either say "general lists" or update. (3) §4.6 says "over all lists (both flag values)", but the format table sums to 81,125 (all lists: DXT1 48,139, DXT3 20,329, DXT5 7,643, R8G8B8 1,888, A8R8G8B8 3,137) and the level-0 size counts are those of `flags` = 0 chains only (all chains: 256×256 46,978, 1024×1024 12,046, 512×512 11,993, 512×1024 1,748). (4) §7.1 "Stored compressed": A8 654,130 and DXT5 32,384 observed, not 654,127 and 32,383. (5) §4.4: the 55 single-level lists whose one level is already at the floor fit both "Full" and "Single"; state that Single takes precedence. (6) §3.1: 50,719 "consecutive" chains is 48,307 multi-level chains plus single-level ones; state the definition. Writer response 2026-10-05: all six items and the non-blocking T19 note addressed (OBS3, §15); awaiting re-review. Re-review 2026-10-05, Claude review session (spec-writer agent): **accepted**. All six items are fixed and reproduce on the install (fresh whole-install probe): 48,307 consecutive chains of 78,713 multi-level chains; the §4.4 table with Single first, 55 lists in both classes (51 / 4), and 77,976 / 92 and 2,355 / 3 with Full first; §4.6 level-0 formats and sizes over all chains, largest 4096; §5.1 spread 54,399 / 26,234 / 493 (54,398 general), cubes in one archive, 77,980 + 1 highres surfaces and 77,996 lists; §6 and §7.1 596,245 / 811,807 and DXT3 165,580 / 41,330, A8 654,130, DXT5 32,384; §6.1 384,950 sets, the 2 local 0x78 sets hold no image property; 54 category-15 lists. The §5.4 OPEN is precise and does not block. All §5.3 checklist items pass. |
| **Implements into** | `crates/formats` (texture-list decoder), `crates/assets` and `crates/material` (texture-detail level selection, highres fallback, GPU upload plan), explorer texture viewer |
| **Companion specs** | `records.md` §5 (one render surface, prefix 0x41: header, format codes, pixel data). `dat-container.md` (archive set, DID routing, optional highres archives). `install-layout.md` §3.3 and §10 (optional highres tier, the `--HighResOutOfDate` option). `material.md` §12.3 and MA33 (no sRGB anywhere). `icon-layers.md` (icons, which reference surfaces directly). `terrain-texturing.md` §5 and §10 (terrain masks and light maps, two families of §6.3). `fonts.md` (font records and their atlases, §6.2). |

---

<a id="s0"></a>
## 0. Purpose

A render surface (prefix 0x41, `records.md` §5) holds exactly **one** image level. This spec
covers everything above that:

- the **texture-list** record (prefix 0x40), which groups surfaces into a mip chain or a cube
  map (§2–§4);
- how a chain is split between the optional **highres** archives and the **surface**
  archives, which levels the original client actually uses for a given texture-detail
  setting, and how a reader copes when highres data is missing (§5);
- how the chosen levels become a GPU texture (§5.5);
- which surfaces are reached **without** a texture list (icons, UI images, font atlases,
  terrain masks, light maps and others) (§6);
- a census of every pixel-format code over **all** 0x41 records, so the M1 exit criterion
  "every texture record decodes or is a listed unsupported format" can be checked (§7);
- whether any other record type carries image data (§8).

All counts are **observations** of one install on one date. They change with every patch.
Tests assert the invariants (§9), not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install by throwaway probes in the dirty-room
  scratchpad. A Confirmed fact states what was decoded and how much of it.
- **BIN**: read from the client executable under method M4 (decision 0011; see the BIN row).
  Facts that rest on BIN alone are marked **BIN**; facts also checked against data are
  marked Confirmed with both sources.
- **Reported**: comes only from third-party material and was not, or could not be, checked
  against data.
- **Inferred**: follows from the facts cited but was not observed directly.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). All 81,136 texture-list records (81,135 in `client_general.dat`, 1 in `client_local_English.dat`) decoded in full. The 24-byte headers of all 1,408,052 surfaces read across all 8 archives that hold 0x41. Every list level resolved against those headers. Font records (0x42, 48) checked for surface references. Byte-magic scan for embedded images over every record of general, local, gamelogic, mesh, cell_1 and map_1. |
| OBS2 | Observed in user install 2026-10-05. All 81,136 lists decoded again and every level DID located in its archive (census of `flags` × level count × placement of level 0, §4.5). The six faces of every cube surface that differs from its siblings were decoded and viewed (§3.2). The owner's two `UserPreferences.ini` files (Steam prefix and a third-party launcher prefix) read, read-only, for the `[Render] TextureDetail` value. |
| OBS3 | Observed in user install 2026-10-05 (repair pass after review). The 24-byte headers of all 1,408,052 surfaces and the bodies of all 81,136 lists dumped again by a fresh probe; the counts of §3.1, §4.4, §4.6, §5.1, §6 and §7.1 recomputed over **all** lists (general and local) and labelled with their scope. The property sets of general, gamelogic and local counted again by prefix (§6.1). |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read with objdump and capstone on address ranges in the dirty room on 2026-10-05. **Questions:** (a) how the client reads a 0x40 record: field order, what `tail_a`, `tail_b` and `flags` mean, which kinds it accepts, and the order of the DID array for cubes with several levels (§2.2, §3.2, §4.5); (b) which cube face a stored face index becomes (§3.2); (c) which levels the client uploads for each texture-detail setting, and what the `--HighResOutOfDate` option and missing highres data change (§5.4); (d) whether the client generates, fills or clamps the levels a stored chain lacks, and how it uploads a chain that breaks the halving rule (§5.5). **Why M1–M3 were not enough:** the data shows only constant `tail` values (0 and 15) and a flag whose effect is behaviour, not layout; no list with several cube levels exists to observe; level choice, upload and the option's effect are code paths; no published source describes them (M2); no running client or frame capture was available to this session (M3). Only offline texture-loading and preference code was read; nothing about network, login, launcher, patcher, keys or anti-tamper (decision 0004). Listings stay in the dirty scratch area. |
| DU | Middle-earth-Revenge DATUnpacker, `FORMATS/40XXXXXX_format.txt`. Facts only. |
| MV | Mirarkitty `lotro_mesh_viewer` docs (MIT): material → texture-list → surface chain, "first texture-list slot is the diffuse". Facts only. |
| DS | `research/notes/data-semantics.md` §7.4. |
| DC | `dat-container.md` §2 (highres archives are optional). |
| SPEC | Other Tessera specs, cited by section: `terrain-texturing.md` T14 and §10, `landblock.md` §6.2, `fonts.md` §3–§4, `install-layout.md` §3.3 and §10, `material.md` MA33. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| T1 | Texture-list layout: DID, kind byte, level-count byte, flag byte, two zero bytes, DID array, two trailing u32 (§2) | DU, DS; OBS (81,136 of 81,136, zero leftover bytes); BIN (a) (same field order in the client's reader) | Confirmed |
| T2 | Kind 2 = mip chain, kind 4 = cube map with 6 face DIDs per level (§3) | DU (count 6 for kind 4); OBS (cube faces are square and equal-sized); BIN (a) | Confirmed |
| T3 | Levels are ordered largest first; each is the previous halved, floor 1 (§4.2) | DS; OBS (81,125 of 81,126 chains) | Confirmed |
| T4 | Every level of a list has the same format and the same category (§4.3) | OBS (all 81,126 chains) | Confirmed |
| T5 | At most one level is in a highres archive, and it is always level 0 (§5.1) | OBS (all 81,136 lists) | Confirmed |
| T6 | Every highres surface is level 0 of at least one texture list and is never reached directly (§5.1) | OBS (77,981 of 77,981) | Confirmed |
| T7 | Highres archives are optional; without them the client never asks for level 0 of a reducible chain (§5.2, §5.4) | DC; `install-layout.md` §3.3; BIN (c) | BIN (rule); that absent archives count as "not usable" is Inferred; structure Confirmed |
| T8 | Icons and UI images are referenced directly by surface DID, never through a texture list (§6.1) | OBS, OBS3 (all icon references in all 384,950 property sets; see `icon-layers.md`) | Confirmed |
| T9 | Font records end with two A8 surface DIDs (glyph atlases) (§6.2) | OBS (47 of 48 font records); SPEC (`fonts.md`: the 48th has no outline atlas) | Confirmed |
| T10 | Only the seven format codes of `records.md` §5.2 occur (§7) | OBS (1,408,052 of 1,408,052) | Confirmed |
| T11 | No record type other than 0x41 embeds image bytes (§8) | OBS (scan, every hit checked) | Confirmed for the archives scanned |
| T12 | Materials (0x30) reference texture lists; the diffuse is texture slot 3000 | MV, DS; SPEC (`material.md` MA9 corrects "first slot") | See `material.md` |
| T13 | The client rejects a list whose kind is not 2 or 4 (§2.2) | BIN (a) | BIN |
| T14 | `tail_a` is the DID of a source object and `tail_b` an update rate per second, both used only by a client-internal texture kind that no record can select; for stored lists they have no effect (§2.2) | BIN (a); OBS (0 and 15 in every record) | BIN (meaning); values Confirmed |
| T15 | For a cube with several levels the DID array is level-major: all six faces of level 0, then all six of level 1, and so on (§3.2) | BIN (a) | BIN |
| T16 | Stored face index *k* is passed unchanged to the graphics back end as cube face *k*: +X, −X, +Y, −Y, +Z, −Z (§3.2) | BIN (b) (index passed through); OBS2 (the one cube with six distinct faces shows the zenith at index 2 and the nadir at index 3); Direct3D/OpenGL face numbering (general knowledge) | Confirmed (up/down); side order Inferred |
| T17 | `flags` bit 0 exempts a list from texture-detail reduction and marks that its level 0 is not a high-resolution-only level (§4.5) | BIN (a), (c); OBS2 (level 0 is in highres for 77,996 of 77,999 multi-level chains with bit 0 clear, and for none of the 714 with bit 0 set) | Confirmed |
| T18 | Texture-detail rule: the client drops the first *k* levels of a reducible chain, *k* = 4, 3, 2, 1 for detail 0–3 and 0 for detail 4 when highres data is usable (§5.4) | BIN (c) | BIN |
| T19 | Without usable highres data (archives absent, or `--HighResOutOfDate` given) detail 4 is not offered and behaves as detail 3 (§5.4) | BIN (c) (the option and the texture-detail code test the same high-resolution resource key); `install-layout.md` §10 (help text) | BIN (option); absent archives OPEN (§5.4), with no effect on Tessera's output |
| T20 | The GPU texture gets exactly the levels of the active chain: no generated levels; level *i* of the active chain is mip *i* (§5.5) | BIN (d) | BIN |
| T21 | The A8 128×128 family is terrain layer masks; most 128×128 DXT1 surfaces outside lists are landblock light maps (§6.3) | SPEC (`terrain-texturing.md` T14, §10; `landblock.md` §6.2) | Confirmed (by those specs) |
| T22 | Texture-detail preference values are stored as `VeryLow`, `Low`, `Medium`, `High`, `VeryHigh` = 0–4; default 3 (§5.4) | BIN (c) (default 3, range 0–4, label strings in this order); OBS2 (`High` and `VeryHigh` seen in the owner's files) | Inferred (label ↔ number); default and range BIN |

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
general first, then local. The client registers the range 0x40000000–0x40FFFFFF as DB object
type 0x0A (`records.md` §2.1).

<a id="s2-2"></a>
### 2.2 Layout (Confirmed, 81,136 records, zero leftover bytes; field roles BIN)

All integers are little-endian (`records.md` §1).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 4 | DID | `did` | The list's own DID (0x40…). Equals the DID looked up (`records.md` §1.6). |
| 4 | 1 | u8 | `kind` | 2 = mip chain (§3.1), 4 = cube map (§3.2). The client fails the load for any other value (T13). |
| 5 | 1 | u8 | `level_count` *n* | Number of levels. Observed 1–12. |
| 6 | 1 | u8 | `flags` | Bit field (§4.5). Observed 0 or 1. |
| 7 | 2 | u16 | `reserved` | Always 0. The client skips it when reading and writes 0. |
| 9 | 4·*m* | DID × *m* | `surfaces` | Surface DIDs (0x41…). *m* = *n* for kind 2, *m* = 6·*n* for kind 4, level-major (§3.2). |
| 9+4*m* | 4 | DID | `tail_a` | Source object DID of the client's internal "updated" texture kind; 0 = none. Always 0. |
| 13+4*m* | 4 | u32 | `tail_b` | Update rate, in updates per second, of that internal kind. Always 15, which is also the client's built-in default. |

Total size = 17 + 4·*m*. A reader checks this against the payload length exactly.

Notes:

- The DID array starts at the unaligned offset 9.
- DU describes bytes 5–8 as "count byte, word, byte". Data and the client's reader both show
  the count in byte 5, the flags in byte 6 and a 16-bit zero in bytes 7–8. Reading bytes 5–8
  as one u32 gives *n* + 256·flags. Readers must not use that u32 as the count.
- **`tail_a` and `tail_b` (T14, BIN).** The client keeps a fifth, internal texture kind whose
  pixels come from another object (named by `tail_a`) and are refreshed at most `tail_b` times
  per second. That kind can only be created by the client itself: the record reader accepts
  only kinds 2 and 4. So for every stored list both values are inert. One side effect remains:
  when `tail_a` is not 0, the client loads that DID together with the list (as a dependency).
  Tessera readers store both values, log values other than 0 and 15, and otherwise ignore them.
- **Rejected kinds (T13, BIN).** For a kind other than 2 or 4 the client's reader flags the
  record as failed and no texture is made. Tessera readers report such a record as an
  unsupported texture list (an error, not a warning). This replaces the earlier "decode and
  warn" advice (§12).

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

A two-level cube (synthetic; no such list exists in the data) with face surfaces
0x41CC00*fl* (*f* = face, *l* = level) stores, after byte 8, the twelve DIDs in this order:
0x41CC0000, 0x41CC0100, 0x41CC0200, 0x41CC0300, 0x41CC0400, 0x41CC0500 (level 0, faces 0–5),
then 0x41CC0001, 0x41CC0101, …, 0x41CC0501 (level 1). Total size 17 + 4·12 = 65 bytes.

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

A chain is called **consecutive** here when it has at least two levels and each level's DID is
the previous level's DID plus 1. Of the 78,713 chains with *n* > 1, 48,307 are consecutive and
30,406 are not (OBS3). (The 2,413 single-level chains are left out: the test says nothing about
them. An earlier count of 50,719 mixed both groups.) Readers must use the stored DIDs and never
derive one level's DID from another.

Lists may share surfaces. 17 surfaces appear more than once across all lists:

- 16 lists have identical bodies apart from their own DID, and so share every level;
- two chains share their levels 2–5;
- one cube-face surface is also level 2 of a chain.

A cache keyed by surface DID must therefore allow one surface to belong to several lists.

<a id="s3-2"></a>
### 3.2 Kind 4: cube map (Confirmed, 10 records; order BIN)

- `level_count` is 1 in all 10, and there are exactly 6 DIDs: one per face, one level each.
- All six faces of a cube are square, of equal size and of equal format. Observed sizes
  64, 256, 512 and 1024; formats DXT1, DXT5 and R8G8B8.
- In 9 of the 10 cubes the same surface DID appears more than once (a face reused).

**Array order for several levels (T15, BIN).** The client reads, for each level in turn, the
six face DIDs of that level. Entry 6·*l* + *f* of the DID array is face *f* of level *l*. No
stored cube has *n* > 1, so this rests on the reader alone.

**Face order (T16).** The client hands stored face index *f* to the graphics back end as cube
face *f*, unchanged. In the face numbering that Direct3D 9/10/11 and OpenGL share, that is:

| Index | Face |
|---|---|
| 0 | +X |
| 1 | −X |
| 2 | +Y |
| 3 | −Y |
| 4 | +Z |
| 5 | −Z |

Data check (OBS2): the only cube with six distinct faces (0x40003EB0, 512×512 R8G8B8) is a
star-field sky box. Faces 0, 1, 4 and 5 each show a horizon across the middle, face 2 is all
sky and face 3 is all black ground: +Y is "up" in the cube's own frame, as the table says.
Which of the four side faces lies east, north and so on depends on the shader that samples
the cube (`lighting.md`, `environment.md`), not on this record.

The other cubes with differing faces are light-projection shapes: one bright face on an
otherwise black cube (index 5 in 0x40000004 and 0x40001C5B, index 4 in 0x40000005, index 0 in
0x40016A77; index 3 differs in the DXT5 pair of 0x40003119).

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
How the client uploads it is in §5.5.

<a id="s4-3"></a>
### 4.3 Format and category are constant (Confirmed, all chains)

All levels of one list share the format code and the surface category field
(`records.md` §5.4). This holds even when level 0 lives in a highres archive and the rest in
a surface archive. The client checks this too: a level whose format differs from the first
level it uploads is reported as a data error (BIN (a)).

<a id="s4-4"></a>
### 4.4 Where a chain stops (Confirmed)

Let the **floor** be 4 for DXT1, DXT3 and DXT5 (one block), and 1 for R8G8B8 and A8R8G8B8.
Then let the **full length** of a chain be the number of halvings from level 0 until the
smaller dimension reaches the floor, plus one.

The classes are tested in this order, and a chain takes the first that fits: **Single**
(*n* = 1), then **Full**, **Short**, **Long**. Single takes precedence because a one-level chain
whose level 0 is already at the floor (full length 1) fits both Single and Full; there are 55
such lists (51 with `flags` = 0, 4 with `flags` = 1), and the table counts them as Single.
Counted with Full first instead, the first two rows would read 77,976 / 92 and 2,355 / 3.
The table covers all 81,126 mip chains (OBS3); cubes are not classified.

| Chain class | flags = 0 | flags = 1 |
|---|---|---|
| Full: *n* = full length | 77,925 | 88 |
| Single level: *n* = 1 | 2,406 | 7 |
| Short: 1 < *n* < full length | 73 | 626 |
| Long: *n* > full length | 1 (the §4.2 exception) | 0 |

Consequences for a reader:

- Do not compute the level count. Use `level_count`.
- A DXT chain never goes below 4 in its smaller dimension. Smaller DXT levels are not stored.
- The client does not add the missing small levels (§5.5).

<a id="s4-5"></a>
### 4.5 The `flags` byte (values Confirmed, meaning BIN)

| Bit | Value | Meaning | Observed |
|---|---|---|---|
| 0 | 0x01 | **Keep all levels.** The list is never reduced by the texture-detail setting (§5.4), and its level 0 is not treated as a high-resolution-only level. When the bit is clear and *n* > 1, the client loads level 0 as a separate class of dependency from the other levels. | 721 lists |
| 1 | 0x02 | Selects a different internal upload path in the client (the texture object is created directly instead of through an intermediate image). No visible effect on decoding. | never |
| 2–7 | | No reader found. | never |

Data agrees with bit 0 (T17, OBS2):

| Lists with *n* > 1 | Level 0 in a highres archive | Level 0 in a surface archive |
|---|---|---|
| bit 0 clear | 77,996 | 3 (0x40003AC9, 0x400047A9, 0x40005441) |
| bit 0 set | 0 | 714 |

All 2,413 single-level chains and all 10 cubes keep level 0 in a surface archive whatever the
bit says (7 single-level chains have it set). Lists with bit 0 set are usually short (626 of
721), commonly 4 levels with a level-0 width of 2048 (for example 2048×512 down to 256×64).

Tessera readers keep the whole byte, act on bit 0 as §5.4 says, ignore bit 1 and log any other
set bit.

<a id="s4-6"></a>
### 4.6 Shape statistics (Confirmed, informative)

Level-0 format, over all 81,136 lists (both kinds, both flag values, general and local;
OBS3):

| Format | Lists |
|---|---|
| DXT1 | 48,139 |
| DXT3 | 20,329 |
| DXT5 | 7,643 |
| A8R8G8B8 | 3,137 |
| R8G8B8 | 1,888 |
| A8, JPEG | 0 |

No texture list references an A8 or JPEG surface.

Most common level-0 sizes over all 81,126 mip chains (both flag values; OBS3): 256×256
(46,978), 1024×1024 (12,046), 512×512 (11,993), 512×1024 (1,748), 256×512 (1,630). The largest
is 4096 on a side. (The figures of the first version, 46,977 / 12,033 / 11,927 / 1,746, were
those of the `flags` = 0 chains alone and were wrongly labelled as all chains.)

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
  one list (one of these is the local list). The remaining one is level 0 of 16 lists whose
  bodies are byte-identical apart from their own DID (duplicate lists). So 77,980 + 16 =
  77,996 lists have a highres level 0.
- The levels of one chain that are **not** in a highres archive (levels 1… when level 0 is
  in highres, otherwise all levels) may be spread over up to three surface archives. Over
  all 81,126 mip chains (OBS3): one archive 54,399 chains; two 26,234; three 493. Over the
  81,125 general chains alone the first figure is 54,398. All 10 cubes use one archive. (The
  first version's 54,208 / 26,480 / 437 did not reproduce.)
- Surface DIDs are unique across all archives (`records.md` §5.5). There is never a highres
  and a surface version of the same DID.
- Whether a list has a highres level 0 does not depend on its size. Both groups include
  lists with level 0 from 8 px up to 4096 px. It depends on `flags` bit 0 (§4.5).

<a id="s5-2"></a>
### 5.2 Missing levels (normative for Tessera)

The highres archives are an optional download (DC, `install-layout.md` §3.3). The client
never needs a highres level when highres data is not usable: §5.4 then always drops level 0
of every reducible chain, and only those chains have a highres level 0 (§4.5, §5.1).

Tessera resolves a mip chain in two steps:

1. **Choose the active chain with the client's rule (§5.4).** With the highres tier absent,
   pass "highres not usable"; the active chain then never contains a highres level.
2. **Robustness extension (Tessera, not client behaviour).** The client fails the whole
   texture if any active level cannot be loaded (BIN (c)). Tessera instead, when an active
   level is missing or does not decode, uses the longest run of consecutive levels that all
   resolve and decode, starting at the first level that does. If no level resolves, the
   texture is unavailable; report it as a lookup miss, not as corruption. This only matters
   for damaged or partial installs; on a complete install it changes nothing.

A Tessera user cap on the top resolution (largest side in pixels) may be applied after step 1
by skipping further leading levels whose larger dimension exceeds the cap, always keeping the
last level.

<a id="s5-3"></a>
### 5.3 Cube maps

The 10 observed cubes keep all faces in surface archives (none in highres) and have one
level, so §5.4 never reduces them. For a cube with several levels, §5.4 drops whole levels
(all six faces of a level together); the duplicate test of step 4 compares face 0 of each
level.

<a id="s5-4"></a>
### 5.4 Texture detail and the active chain (BIN; normative)

The client uploads only a suffix of each list, the **active chain**. Inputs:

- the stored levels L₀ … Lₙ₋₁ and `flags` (§2.2);
- the **texture-detail setting** *t*, an integer 0–4 (T22: stored in the original's
  preferences as `[Render] TextureDetail` with the labels `VeryLow`, `Low`, `Medium`, `High`,
  `VeryHigh`; the client's default is 3);
- **highres usable** *h*: false when the client was started with `--HighResOutOfDate`
  (`install-layout.md` §10; BIN) or when the highres archives are absent (see the OPEN
  below); true otherwise.

> **OPEN:** Whether the client also treats *h* as false when the four highres archives are
> simply absent (no option given). BIN (c) shows only that the option and the texture-detail
> code test the same "high-resolution set available" key of the client's resource system;
> how that key is set when the archives are missing was not traced. Tessera's output does not
> depend on the answer: with the archives absent and *h* taken as true, step 2 at *t* = 4 gives
> *k* = 0, level 0 cannot be loaded, and the §5.2 extension starts the chain at level 1, which
> is the same active chain as *h* = false. Tessera therefore sets *h* = false when no highres
> archive is present, and the only visible difference is whether its options UI offers
> detail 4.

Preparation (at start-up, BIN): the setting is registered with the range 0–4 and its stored
value. When *h* is false and that value is 4, the client registers it again with the range
0–3 and sets it to 3. (Step 2 gives the same drop count for 4 and 3 when *h* is false, so the
re-registration changes only what the options UI offers.)

Steps, for one list:

1. If `flags` bit 0 is set, or *n* ≤ 1, the active chain is all *n* levels. Stop.
2. Let the drop count *k* be: 4 for *t* = 0, 3 for *t* = 1, 2 for *t* = 2, 1 for *t* = 3. For
   *t* = 4 it is 0 when *h* is true and 1 when *h* is false. Any other *t* gives 0.
3. If *k* ≥ *n*, set *k* = *n* − 1 (the last level is always kept).
4. Let *r* be the number of levels at the start that repeat the previous level's DID: the
   largest *r* such that L₀ … Lᵣ all have the same DID. If *r* > *k*, set *k* = *r*.
   (No stored list repeats a DID this way; the step only removes duplicate top levels.)
5. The active chain is L_k … Lₙ₋₁. Its level *i* is stored level *k* + *i*.

Consequences on current data:

- At *t* = 4 with highres usable, the 77,996 chains with a highres level 0 are used in full.
- At the default *t* = 3, and whenever highres is not usable, those chains start at level 1,
  so no highres surface is read. The 3 lists of §4.5 with bit 0 clear but no highres level
  also lose their level 0.
- Lists with bit 0 set (721) and single-level lists are always used in full.

Worked example (synthetic). A list with *n* = 9, `flags` = 0, level 0 1024×1024 in highres:

| *t* | *h* | *k* | Active chain (top size) |
|---|---|---|---|
| 4 | true | 0 | levels 0–8 (1024) |
| 4 | false | 1 | levels 1–8 (512) |
| 3 | either | 1 | levels 1–8 (512) |
| 2 | either | 2 | levels 2–8 (256) |
| 0 | either | 4 | levels 4–8 (64) |

The same list with `flags` = 1 is levels 0–8 for every *t*. A list with *n* = 3, `flags` = 0
at *t* = 0 gets *k* = min(4, 2) = 2: only level 2.

Tessera maps its own texture-quality setting onto *t* (0–4) and must not write the original's
preference file. When it chooses to mirror the original, it reads *t* from that file
(`install-layout.md`), with `High` (3) when absent or unreadable.

<a id="s5-5"></a>
### 5.5 GPU upload (BIN)

- The client creates one GPU texture per list from the active chain: width and height of
  active level 0, format of active level 0 (or a device format the back end prefers; colour
  values are kept, and no back end uses sRGB, `material.md` MA33), and a **mip count equal to
  the number of active levels**. For a cube, the face size of active level 0 and the same mip
  count for each face.
- Active level *i* fills mip *i* (of every face, for cubes: face-major, each face's levels in
  order). The client does **not** generate the levels a stored chain lacks (§4.4) and does not
  fill them. Sampling below the smallest stored level stays on that level, as the graphics
  API does for a texture created with fewer mips than a full pyramid. Tessera's upload uses
  the same mip count; a renderer that must create a full pyramid clamps the sampler's
  maximum level of detail to (active count − 1) instead of synthesising levels.
- Each level is copied into its mip slot; when a level's stored size differs from the slot
  size, the copy is clipped to the smaller width and height (top-left part). For the §4.2
  exception this matters only at *t* = 4 with highres usable: the second 1024×1024 level is
  then copied, clipped, into the 512×512 slot. At every other setting its level 0 is dropped
  and the active chain halves exactly. Tessera may instead skip a non-halving level and upload
  the longest exactly halving run (§13); no other list is affected.
- The client picks no level by on-screen size on the CPU. The GPU chooses per pixel from the
  uploaded mips (with the material's filter modes, `material.md` §7.5). Any CPU-side "best level
  for a size" choice (thumbnails, previews) is a Tessera tool feature (§13).

---

<a id="s6"></a>
## 6. Surfaces reached without a texture list

Only 596,245 of the 1,408,052 surfaces (42%) are levels of some texture list (all 81,136
lists, general and local; OBS3). The other 811,807 are referenced directly by DID from other
records. (Counting the 81,135 general lists alone gives 596,236 and 811,816: the local list
has 9 levels of its own.)

<a id="s6-1"></a>
### 6.1 Icons and UI images (Confirmed)

Every image-valued property in every property set (prefixes 0x78 and 0x79, 384,950 sets:
383,332 with prefix 0x79 and 1,616 with prefix 0x78 in `client_gamelogic.dat`, plus 2 with
prefix 0x78 in `client_local_English.dat`; `property-system.md` §5) names a **0x41 surface
directly**. None names a texture list. Such
a surface is a single image with no mips. Its category field is 15 in every one of these
references. See `icon-layers.md` for how icons are composed.

Category 15 marks UI images (`records.md` §5.4): 36,382 A8R8G8B8, 2,308 R8G8B8 and every
JPEG carry it. Only 54 texture lists have category 15.

<a id="s6-2"></a>
### 6.2 Font glyph atlases (Confirmed)

47 of the 48 font records (prefix 0x42, in general and local) end with two u32 DIDs. Each
names an A8 surface, and both surfaces have the same dimensions (for example 512×499). The
48th (0x4200002A) ends with one such DID and a 0, because it has no outline atlas. Glyph
atlases are one consumer of format 28 (A8): there the single channel is glyph coverage. The
font record layout and the 95 atlases are specified in `fonts.md` §3–§4.

<a id="s6-3"></a>
### 6.3 Other direct families (Confirmed shapes)

These large families appear in no texture list:

| Family | Count | Archives | Referenced by |
|---|---|---|---|
| A8, 128×128, category 0 | 633,470 | surface, surface_aux_1, surface_aux_2 | Terrain layer alpha masks of landblock surface chunks (`terrain-texturing.md` §5, T14: every one of the 646,073 masks named by a drawn layer is A8 and square 128, 256 or 512) |
| A8, other sizes (256², 512², odd sizes) | 25,738 | same | The larger terrain masks, the 95 font atlases (category 13, §6.2), and a remainder not named by a drawn terrain layer |
| DXT3, 96×96, category 0 | 41,325 | surface family | Unknown. RGB content with the alpha channel **0 everywhere** in every sample |
| DXT1, 128×128, category 0 | 5,502 | surface family | Mostly landblock light maps: `Landblock_LightMap` and `Landblock_CeilingLightMap` name 5,441 DXT1 128×128 surfaces (`landblock.md` §6.2, `terrain-texturing.md` §10). The remaining 61 are unattributed |
| JPEG (code 500), category 15 | 65,706 | surface family, local (221) | Maps and large UI images; minimap tiles (`landblock.md` §6.3) and map-image properties |

A raw scan for u32 values equal to these DIDs cannot tell which record type references them:
landblock and mesh records produce too many chance matches.

> **OPEN:** Which record type references the 41,325 DXT3 96×96 tiles (and the 61 remaining
> DXT1 128×128 tiles), and how a renderer interprets their all-zero DXT3 alpha. Decoding them
> needs nothing beyond `records.md` §5.

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
| 0x33545844 | DXT3 | 206,910 | 20,165 | 186,745 | 0 | 165,580 | 41,330 | 153,983 |
| 0x35545844 | DXT5 | 60,676 | 7,272 | 53,404 | 0 | 60,676 | 0 | 32,384 |
| 28 | A8 | 659,208 | 0 | 659,208 | 0 | 0 | 659,208 | 654,130 |
| 21 | A8R8G8B8 | 49,243 | 1,059 | 48,182 | 2 | 11,595 | 37,648 | 43,088 |
| 20 | R8G8B8 | 19,705 | 1,831 | 17,874 | 0 | 17,297 | 2,408 | 11,891 |
| 500 | JPEG | 65,706 | 0 | 65,485 | 221 | 0 | 65,706 | 5,443 |

"Level of a list" and "Not in any list" count over all 81,136 lists, general and local
(OBS3); the two columns sum to 596,245 and 811,807 (§6). "Stored compressed" counts records
whose directory entry marks them zlib-compressed (`dat-container.md`). The first version
counted the general lists only in these columns (DXT3 165,571 / 41,339): the local list's 9
levels are all DXT3, and its level 0 is the one highres surface that no general list
references. The first version's compressed counts for A8 (654,127) and DXT5 (32,383) did not
reproduce.

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
way. Show colour values as stored: the original never converts textures from sRGB
(`material.md` MA33).

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
(icons, fonts, terrain masks, light maps) or by texture-list DID (materials).

---

<a id="s9"></a>
## 9. Invariants

Real-data tests assert these. Failures are reported per record and do not abort a census.

| # | Invariant | Status |
|---|---|---|
| TL1 | payload length = 17 + 4·*m* (§2.2) | Confirmed, all |
| TL2 | `kind` ∈ {2, 4} (other kinds are rejected, §2.2); `flags` ∈ {0, 1}; bytes 7–8 = 0 | Confirmed, all (readers warn on other flag values and non-zero bytes 7–8) |
| TL3 | `tail_a` = 0, `tail_b` = 15 | Confirmed, all (warn only) |
| TL4 | kind 4 ⇒ *m* = 6·*n*; kind 2 ⇒ *m* = *n* | Confirmed, all |
| TL5 | every surface DID has prefix 0x41 and resolves in a full install | Confirmed, all |
| TL6 | all levels of a list share format and category | Confirmed, all |
| TL7 | widths and heights are non-increasing from level 0 | Confirmed, all |
| TL8 | at most one level is in a highres archive, and only level 0 | Confirmed, all |
| TL9 | a DXT level is never smaller than 4 in either dimension | Confirmed, all (follows from `records.md` §5.2) |
| TL10 | a list with a level in a highres archive has `flags` bit 0 clear and *n* > 1 | Confirmed, all 77,996 |
| TL11 | the §5.4 active chain is never empty, ends at the last stored level, and at *t* < 4 or with *h* false contains no highres surface | Follows from §5.4 and TL8/TL10; Confirmed on all lists |

---

<a id="s10"></a>
## 10. Synthetic fixtures for implementers

- A three-level DXT1 chain (the §2.3 example) with matching synthetic surfaces.
- A single-level A8R8G8B8 list.
- A kind-4 cube with six distinct 4×4 faces, and one with a repeated face.
- The two-level cube of §2.3 (level-major order; faces 0–5 of level 0 first).
- A list whose length disagrees with `level_count` (reject: truncated or trailing bytes).
- A list with `kind` = 3 (reject as unsupported) and one with `flags` = 2 (decode, warn).
- A nine-level chain with `flags` 0 and with `flags` 1, run through §5.4 for every *t* and
  both values of *h* (the table of §5.4).
- A chain whose first two levels have the same DID (§5.4 step 4 drops one even at *t* = 4).
- A chain whose level 0 is absent from the archive set (with *h* false the active chain
  starts at level 1 anyway; with *h* true the §5.2 extension starts it at level 1).
- A chain whose levels do not halve (accept, use the per-level sizes).
- A chain with a zero DID in the middle (the §5.2 extension ends the chain before it).

---

<a id="s11"></a>
## 11. Open questions

1. ~~Meaning of `tail_a`/`tail_b`~~: answered 2026-10-05 (§2.2).
2. ~~Cube-face order and multi-level cube layout~~: answered 2026-10-05 (§3.2). Which world
   direction each side face shows is a question for the shaders that sample cubes.
3. ~~Meaning of `flags` = 1~~: answered 2026-10-05 (§4.5).
4. ~~Client behaviour with an out-of-date highres archive~~: answered 2026-10-05 (§5.4).
5. ~~Font record layout~~: specified in `fonts.md`.
6. Referencing record type of the DXT3 96×96 tiles and the last 61 DXT1 128×128 tiles, and
   the meaning of the all-zero DXT3 alpha (§6.3). The A8 masks and the light maps are
   attributed.
7. Surface category meanings (`records.md` §5.4). This spec adds that category 15 is the UI
   category, and that a list never mixes categories.
8. Whether absent highres archives (without `--HighResOutOfDate`) make the client treat
   highres as unusable (§5.4). No effect on Tessera's output.

---

<a id="s12"></a>
## 12. Open items raised by the `formats` implementation (2026-10-04)

These came up while implementing the texture-list decoder in `crates/formats` from this spec
alone. The whole-install run of `crates/formats/tests/m1_real_data.rs` decodes all 81,136
lists and finds no TL1–TL9 violation, so none of these choices is exercised by current data.

> **Answered (2026-10-05):** The client accepts only kinds 2 and 4 and fails the load of any
> other kind (§2.2, T13). A reader should report another kind as an unsupported list instead of
> decoding it with one DID per level. The implementation's current behaviour (decode, flag as
> unusual) needs changing; no stored record is affected.

> **Answered (2026-10-05):** A list with `level_count` = 0 gives the client no texture (its
> upload step fails with zero levels). Accepting the record and reporting the texture as
> unavailable, as implemented, matches.

---

<a id="s13"></a>
## 13. Open items raised by the `material` implementation (2026-10-04)

These came up while implementing the mip-chain helpers in `crates/material` (`texture`
module). The real-data test resolves 500 random lists with no TL6, TL7 or TL9 finding.

> **Answered (2026-10-05):** The original picks no level for an on-screen size on the CPU. It
> uploads the §5.4 active chain and the GPU samples per pixel (§5.5). The implementation's
> "smallest level at least as large as the target" stays a Tessera tool choice (thumbnails,
> previews) and must not be used to choose the level-0 of a rendered texture; use §5.4 for that.

> **Answered (2026-10-05):** The original neither generates nor fills the levels below the
> last stored one: the GPU texture has exactly the active level count (§5.5). Reporting the
> difference and clamping the sampler, as implemented, matches; generating extra levels does
> not.

> **Answered (2026-10-05):** For the one chain that breaks §4.2, the original at *t* = 4 with
> highres usable uploads level 1 clipped into the 512×512 slot; at every other setting level 0
> is dropped by §5.4 and the chain halves exactly (§5.5). The implementation's choice (upload
> the exactly halving run from level 1) gives the same pixels as the original at the default
> setting and is accepted as a Tessera simplification.

> **OPEN (Decision, 2026-10-06):** `MaterialLibrary::texture_chain` ends the effective chain
> before the first later level that is larger than the level before it in width or height
> (TL7 broken, hostile data only) or whose larger side is above the resolution cap (the
> caller's cap and the configured texture limit), so every returned level is within the
> limit and the chain is largest first. On a complete install (TL7 holds for all chains)
> this changes nothing. The two equal levels of the §4.2 exception are kept.

---

<a id="s14"></a>
## 14. Corrections to other specs

| Spec | Says | Correction |
|---|---|---|
| `material.md` §18 OPEN (`MaterialLibrary::cube_faces`) | Face order is OPEN in this spec | Answered here (§3.2): stored index *f* is cube face *f* in the order +X, −X, +Y, −Y, +Z, −Z; several levels are level-major. |
| `material.md` §18 OPEN (Decision) on texture level selection | The configured largest side caps the top level | Compatible, but the client's own rule is §5.4 (drop count from the texture-detail setting, `flags` bit 0, highres usability). The cap is a Tessera extension on top of it (§5.2). |
| `render.md` §3 OPEN (mip chains) | Is level 0 always the largest; how are levels chosen | Level 0 is the largest (§4.2); the uploaded levels are the §5.4 active chain with exactly its own mip count (§5.5). |

---

<a id="s15"></a>
## 15. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-04 | all | First version (Review). |
| 2026-10-05 | §1, §2.2, §2.3, §3.2, §4.3–§4.5, §5.1–§5.5, §6.2, §6.3, §7.3, §8–§15 | M4 pass (BIN row, T13–T22) and new census (OBS2). §2.2: field roles from the client's reader; `tail_a`/`tail_b` meaning; kinds other than 2 and 4 are rejected. §2.3: multi-level cube example. §3.2: level-major array order and face order (OPENs closed). §4.5: `flags` bit 0 meaning with the placement census, bit 1. §5.2 rewritten (client rule first, Tessera robustness extension second). New §5.4 (texture detail, `--HighResOutOfDate`, worked example) and §5.5 (GPU upload: mip count, no generated levels, clipped copies). §6.2 points to `fonts.md`. §6.3: terrain masks and light maps attributed. §9: TL2 revised, TL10–TL11 added. §10: fixtures for §5.4. §11–§13: OPENs answered. New §14 (corrections to other specs) and §15. |
| 2026-10-05 | §1, §3.1, §4.4, §4.6, §5.1, §5.4, §6, §6.1, §7.1, §11 | Repair after review (changes requested). New provenance tag OBS3 (fresh whole-install dump). §3.1: "consecutive" defined (multi-level chains only: 48,307 of 78,713). §4.4: class order stated (Single before Full; 55 lists fit both). §4.6: level-0 formats and sizes recomputed over all lists. §5.1: archive spread recomputed (54,399 / 26,234 / 493 over all chains) and its scope stated; the highres reference sentence corrected (the local list's level 0 is one of the 77,980). §5.4 and T19: absent highres archives become an OPEN callout. §6 and §7.1: list-level counts over all lists (596,245 / 811,807; DXT3 165,580 / 41,330); compressed A8 654,130, DXT5 32,384. §6.1 and T8: 384,950 property sets. No rule changed. |
