<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Havok-format data: skeletons (in 0x04 setups) and animation clips (0x05)

| | |
|---|---|
| **Status** | Accepted (revision of 2026-10-07 reviewed and passed, 2026-10-07) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-reviewer, a different session from the writer) |
| **Review record** | Re-review, 2026-10-07 (Claude review session, spec-reviewer, a different session from the writer): **passed, Accepted kept**. The file was last modified when the review below edited the header, so its body is the one that review passed. CLEAN_ROOM.md §5.3 is still met (no code, no pseudo-code, no copied identifiers, no client strings or addresses). Review of the 2026-10-07 revision: Claude review session (spec-reviewer, a different session from the writer), 2026-10-07: **passed** (CLEAN_ROOM.md §5.3, every item; remaining OPENs precise and non-blocking; the implementer answers are stated as Tessera bounds, not client facts). Re-checked on the same build: H22 (the routine that creates a clip's animation control sets the end crop to 1/30 s, the float 0.033333335, only when the cycle count is exactly 1, and lowers a starting local time above `duration` − 1/30 s to that value; the two callers pass the count through). Earlier record: Revision of 2026-10-07 (§16): not yet reviewed. Earlier record: Claude review session (spec-writer agent), review 2026-10-05: changes requested (§12.2 row 2 *i*, encoding tie-break, undefined tag ARCH, crop-amount OPEN). Re-review 2026-10-05: **passed** (CLEAN_ROOM.md §5.3, every item; remaining OPENs precise and non-blocking). Re-review spot checks: all four §12.2 rows and the alternative `FF F7 7F FE 3F` re-encoded and decoded with an independent encoder following the stated tie-break; DS §8 supports the §2.4 remark; independent re-run over all 25,395 clip records: envelope flag 25,285 / 110, VLE forms 25,355 / 40, weights 18,460 empty / 6,935 full, 21 duration mismatches, 24,800 spline / 595 interleaved, `maxFramesPerBlock` 256, 31,294 blocks ending at `floatBlockOffsets`, quantisation byte 0x45 on 1,645,618 masks, degree counts as in §8. Non-blocking note: in §2.3 the sentences after the crop-amount OPEN ("Sampling also uses the tagfile values ...") continue inside the callout; they are normative and could move out of it. |
| **Implements into** | `crates/havok` (container reader, skeleton and clip models, spline decoder), `crates/anim` (sampling, §7.4–§7.8), `fuzz/` (one target per structure kind, §11), `apps/dattool` / `apps/explorer` (clip and skeleton inspection) |
| **Companion specs** | `setup.md` §6.1 (the 0x04 record that carries the skeleton tagfile; this spec starts at its field 3). `records.md` §1.2 (VLE, used once in the 0x05 envelope). `mesh.md` §5.7 (mesh bone indices address the skeleton bone array of this spec). `skeleton-anim.md` (M4: track-to-bone mapping, attach points, root motion; it consumes §4 and §7 of this spec). |

---

<a id="s0"></a>
## 0. Purpose

The archives store character and object skeletons and all animation clips in a binary
format produced by a licensed third-party animation middleware (Havok). This spec describes
**exactly the subset present in this install**, and nothing else, in enough detail that
`tessera-havok` can be written with no other source:

- where Havok data occurs, and the 0x05 record envelope around it (§2);
- the container grammar: a self-describing binary *tagfile*, version 3 (§3);
- the 13 classes that occur and their members (§4);
- the skeleton (§4.2), annotation tracks (§4.4) and the extracted-motion reference frame
  (§4.5);
- the spline-compressed clip, byte by byte, including the 40-bit quaternion (§5);
- the uncompressed interleaved clip (§6);
- every decoding and sampling algorithm in words (§7);
- a census over **all** 28,555 skeleton tagfiles and **all** 25,395 clip records (§8);
- the exact meaning of "skeleton record" and "clip record" for ROADMAP M4 exit criterion 1,
  and the failure categories (§9);
- invariants (§10), the structure kinds the fuzz targets must cover (§11), and synthetic
  worked examples that tests can construct (§12).

All counts are observations of one install on one date. They change with patches. Tests
assert the invariants (§10), not the counts.

Names of Havok classes and members (`hkaSkeleton`, `numFrames`, ...) appear in this spec
because they are stored in every tagfile as format-level identifiers, and a decoder looks
members up by them. No game text appears here: bone and track names are described only by
count and pattern.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install on 2026-10-04 by throwaway probes in the
  dirty-room scratchpad. A Confirmed fact states what was decoded and how much of it.
- **Reported**: comes only from third-party material and was not, or could not be, checked
  against data (usually because the case never occurs in this install).
- **BIN**: rests on the client's code alone (method M4, decision 0011); removed if that
  authorisation is withdrawn.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). **All 28,555** 0x04 records of `client_general.dat` (one tagfile each) and **all 25,395** 0x05 records of `client_anim.dat` (25,505 tagfiles) parsed with a generic tagfile reader written by the spec writer; every spline block of every clip (31,294 blocks, 1,645,618 track headers, 51.7 million quaternion control points) parsed to the exact byte length declared by the clip; spline curves evaluated at every frame of 1,200 sampled clips; block seams compared on every multi-block clip. Archives scanned for the Havok tagfile and packfile magics: general, gamelogic, anim, mesh, mesh_aux_1, cell_1, local_English (every record). |
| HX | exyorha/hkxparse (MIT): binary tagfile grammar (magic, tag numbers, signed varint, string pool with two reserved entries, metadata layout, member type codes, presence bitmap, struct arrays stored column by column, int and vec4 array prefixes). Facts only. |
| HL | PredatorCZ/HavokLib (GPL-3.0): spline-compressed block layout, mask bit meanings, quantisation codes, 40-bit quaternion field layout, padding rules, block/time mapping. Studied for facts only; no code taken. |
| MV | Mirarkitty `lotro_mesh_viewer`, `docs/animation.md`: envelope of 0x05, record counts, no binding records, all clips 40-bit, positional track binding. Facts only. |
| DS | `research/notes/data-semantics.md` §1 (0x04 row) and §8. |
| NB | Public B-spline mathematics (Piegl and Tiller, *The NURBS Book*, knot-span search and Cox–de Boor basis recurrence). General reference. |
| SET | `setup.md` §6.1 (0x04 outer layout). |
| SA | `skeleton-anim.md` (§3 binding, §6.6 track weights, §7.3 extracted motion; its BIN session and census of 2026-10-05). |
| BIN | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)).** The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035), disassembled in the dirty room for facts only on 2026-10-05: the clip record reader, the clip player's use of a clip's length, and the animation middleware's own routines inside the client for sampling interleaved clips and reference frames. **Questions:** which duration governs playback when the envelope and the tagfile disagree (§2.3); how frames of interleaved clips are interpolated (§7.8); how reference-frame samples map to time (§4.5, §7.8). **Why M1–M3 were not enough:** the data hold two durations and no rule; interpolation and sample timing are code, not data; no published source states them for this client; observing the running client (M3) cannot tell a 1-frame shift from a stretched sample array. Only offline animation code was read. **Second session, same build, 2026-10-07** (Ghidra, read-only copy of the project, by address): **questions** (4) where the clip player's start and end crop amounts come from (§2.3); (5) whether the spline sampler aligns quaternion signs (§7.6). **Why M1–M3 were not enough:** no record field holds a crop amount, so the data cannot show whether one is ever set; the sign question concerns 0.007 % of control-point pairs, which no running client or capture would isolate. Code read: the game's routine that creates the middleware's animation control for a clip, both places that call it, the control's constructor, and every instruction of the client that stores a float at the control's two crop fields; for (5) the spline block decompressor again. Nothing of the network, login, launcher or anti-tamper code. |
| CNS2 | Recount of 2026-10-05 with the repository's `tessera-havok` as a tool: roots per skeleton and non-unit reference scales over all 28,555 skeletons. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| H1 | Havok data occurs only in 0x04 setups (one tagfile each) and 0x05 clips (one or two tagfiles each); no packfile exists anywhere (§2.1) | DS, MV, SET; OBS (scan) | Confirmed. Corrects DS (packfile magic in 0x04). |
| H2 | Every container is a binary tagfile, version 3; no SDK version string (§2.4, §3) | HX, MV; OBS (54,060 of 54,060) | Confirmed |
| H3 | 0x05 envelope: duration, track count, reference-frame flag, one or two length-prefixed tagfiles, VLE-counted per-track weight array, nothing after (§2.3) | MV (first three fields, partly); OBS (25,395 of 25,395, zero leftover bytes) | Confirmed. Corrects MV and DS ("u8 0", "size + 1"). |
| H4 | Tagfile grammar: varints, string pool, metadata, objects with presence bitmaps, column-wise struct arrays (§3) | HX; OBS | Confirmed for every form that occurs; forms that never occur are Reported (§3.12) |
| H5 | 13 classes are declared; each has one identical member list everywhere (§4.1) | OBS | Confirmed |
| H6 | Absent members take the zero/empty default (§3.10) | HX (bitmap); OBS (column patterns, §4.2) | Confirmed by consistency, not by a writer |
| H7 | Skeleton: parent indices, bones (name, lock flag), reference pose as translation/rotation/scale (§4.2) | DS, MV; OBS (28,555 of 28,555) | Confirmed |
| H8 | No animation container, binding, skeleton mapper or root-level container exists (§4.6) | DS, MV; OBS | Confirmed |
| H9 | Annotation tracks occur in 5,602 clips, one per transform track, holding only a track name; no clip has any annotation event (§4.4) | MV (names); OBS | Confirmed. Corrects MV/DS ("annotation data"). |
| H10 | Spline block layout, mask bits, channel layouts, padding (§5) | HL, MV; OBS (31,294 of 31,294 blocks end exactly at their declared length) | Confirmed |
| H11 | Every track uses 16-bit position, 40-bit rotation, 16-bit scale (quantisation byte 0x45) (§5.3) | MV (40-bit only); OBS (1,645,618 of 1,645,618) | Confirmed |
| H12 | 40-bit quaternion: three 12-bit fields, 2-bit index of the dropped component, sign bit (§5.6) | HL, MV; OBS (dropped component is the largest in 13.1 million of 13.1 million checked) | Confirmed |
| H13 | Knots are u8 frame numbers local to the block; blocks share their boundary frame; clamped knot vectors (§5.4, §7.4) | HL; OBS (all knot vectors; seams continuous in 99.8 % of channels to 1e-3, the rest to 1e-2) | Confirmed |
| H14 | Control points of a vector channel are interleaved per point, components in x, y, z order (§5.4) | HL; OBS (block-seam test: 22,817 for, 0 against) | Confirmed |
| H15 | Interleaved clips store `numFrames × tracks` transforms, frame-major (§6) | OBS (505 for, 0 against, 90 static clips undecidable) | Confirmed |
| H16 | Tracks map to bones by position: track *j* drives bone *j*, and a clip fits a skeleton exactly when its track count equals the bone count (§4.4) | MV, DS; SA §3 (frame-0 pose agreement on 24,839 clips, reversed-order control, 1,908 name-matched clips) | Confirmed (by SA, 2026-10-05). Owned by `skeleton-anim.md` §3. |
| H17 | The envelope `track_weights` are per-track opacities of layered playback (§2.3) | SA §6.6 (pattern census; layer rule BIN) | Confirmed (by SA) |
| H18 | The clip player takes a clip's playable length from the tagfile `duration` (§2.3) | BIN | BIN |
| H19 | Interleaved clips interpolate translation and scale linearly and rotation by sign-aligned normalised linear interpolation; the frame index is *t* × (*N* − 1) / `duration` (§7.8) | BIN | BIN |
| H20 | Reference-frame samples are spread over the duration by their own count and interpolated linearly in all four components; the rotation is the angle *w* about `up` (§4.5, §7.8) | BIN; SA §7.3 (meaning of *w*) | BIN |
| H21 | Roots per skeleton 1, 2, 3, 4 (§4.2) | CNS2 | Confirmed (recount) |
| H22 | The start crop is always 0; the end crop is 1/30 s when the clip is started for exactly one cycle, else 0 (§2.3) | BIN (question 4) | BIN |

---

<a id="s2"></a>
## 2. Where Havok data occurs

<a id="s2-1"></a>
### 2.1 Occurrence (Confirmed)

Every record of the scanned archives (H1 row of §1) was searched for the 8-byte tagfile magic
`1E 0D B0 CA CE FA 11 D0` and the packfile magic `57 E0 E0 57 10 C0 C0 10`.

| Archive | DID prefix | Records | With tagfile magic | With packfile magic |
|---|---|---|---|---|
| `client_general.dat` | 0x04 (setup) | 28,555 | 28,555 | 0 |
| `client_anim.dat` | 0x05 (animation clip) | 25,395 | 25,395 | 0 |
| any other scanned archive or prefix | – | – | 0 | 0 |

`client_anim.dat` holds only 0x05 records (plus 5 archive-metadata records, `dat-container.md`).
No other prefix contains Havok data. **Packfiles do not occur**; `tessera-havok` implements
the tagfile only.

<a id="s2-2"></a>
### 2.2 In a 0x04 setup (Confirmed)

`setup.md` §6.1: field 1 is the record DID, field 2 a u32 length *t*, field 3 *t* bytes of
tagfile. The tagfile always starts at record offset 8 and ends exactly at offset 8 + *t*
(28,555 of 28,555). It holds exactly one object, an `hkaSkeleton` (§4.2). *t* ranges from 349
to 25,097 bytes; the most common lengths are 358 (16,376 records), 402, 351 and 349.

<a id="s2-3"></a>
### 2.3 The 0x05 clip record envelope (Confirmed, 25,395 of 25,395, zero leftover bytes)

A 0x05 record does **not** begin with its own DID (unlike most records, `records.md` P10).
All values little-endian.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `duration` | f32 | Clip length in seconds. Equals the tagfile's `hkaAnimation.duration` in 25,374 records; differs in 21 (§8.4). |
| 2 | `track_count` | u32 | Equals the tagfile's `numberOfTransformTracks` in 25,395 of 25,395. |
| 3 | `has_reference_frame` | u8 | 0 (25,285 records) or 1 (110 records). Any other value is malformed. |
| 4 | `reference_frame` | only if field 3 = 1: u32 length *r*, then *r* bytes of tagfile | A tagfile whose only object is an `hkaDefaultAnimatedReferenceFrame` (§4.5). |
| 5 | `tagfile_length` *t* | u32 | Length of field 6 |
| 6 | `tagfile` | *t* bytes | The clip tagfile. Its object 1 is the animation (§4.3). |
| 7 | `weight_count` *w* | VLE (`records.md` §1.2) | 0 (18,460 records) or exactly `track_count` (6,935 records). 1-byte form in 25,355 records, 2-byte form in 40 (all with *w* ≥ 128). |
| 8 | `track_weights` | f32 × *w* | One value per transform track, in track order. Every value lies in [0, 1]; no non-empty array is all 1.0. |

The record ends exactly after field 8. Record sizes range from 618 bytes to 1,972,742
bytes (median 11,765).

**`track_weights` (answered by `skeleton-anim.md` §6.6, 2026-10-05):** they are per-track
**opacities** for layered playback. A clip whose animation node has `UseBoneOpacity` = 1 and
that carries the array weights each bone's contribution by its track's value; an absent array
means 1 on every track. The dominant pattern on player skeletons is "upper body only" (legs
0.01, arms and head 1.0). The layer rule that uses them is normative in `skeleton-anim.md`
§6.6. The decoder only exposes the array.

**Which duration (BIN, 2026-10-05):** the client's record reader stores the envelope `duration`
next to the decoded animation, but its clip player computes a clip's playable length from the
**tagfile** `duration`, minus a start and an end crop amount that the player keeps for the
playing clip. No use of the envelope value was found. Sampling also uses the tagfile values,
because the spline block layout is tied to them (§7.4). Decoders keep exposing the envelope
value separately.

**Crop amounts (BIN, 2026-10-07, row H22; answers the OPEN of 2026-10-05):** the client
creates one animation control per playing clip, with a **cycle count** (how many times the
clip repeats; −1 means without end; the count comes from the animation node or script that
starts the clip, `skeleton-anim.md`). The control starts with both crop amounts 0. Then:

1. If the cycle count is exactly 1 (the clip plays once), the end crop becomes **1/30 s**
   (the float 0.033333335), and if the control's starting local time is greater than
   `duration` − 1/30 s it is lowered to that value.
2. Otherwise both crop amounts stay 0.
3. The start crop is never set: no code of the client stores a value into it after the
   control is created.

So a clip that plays once ends 1/30 s before its tagfile `duration` (its playable length is
`duration` − 1/30 s); a looping clip plays its full `duration`. Worked example (synthetic):
`duration` 1.5 s played once ends at local time 1.4666667 s; the same clip looping wraps at
1.5 s.

> **OPEN:** whether anything outside the clip player reads the envelope `duration` (21 records
> differ, §8.4). Not traced in the 2026-10-07 session either. Tessera uses the tagfile value
> everywhere.

<a id="s2-4"></a>
### 2.4 Container kind and version (Confirmed)

| Location | Tagfiles | Kind | Version |
|---|---|---|---|
| 0x04 field 3 | 28,555 | binary tagfile | 3 |
| 0x05 field 4 | 110 | binary tagfile | 3 |
| 0x05 field 6 | 25,395 | binary tagfile | 3 |

Version 3 tagfiles carry no SDK version string (HX: version 4 would add one). Class versions
stored in the metadata (§3.5) are 3 for `hkaSkeleton` and `hkaAnimation` and 0 for every
other class; this fits the Havok 6.x era reported in DS (`research/notes/data-semantics.md`
§8, a 2008 licence of the version 6 middleware; Reported, not needed for decoding).

---

<a id="s3"></a>
## 3. Tagfile container grammar

A binary tagfile is a byte stream with **no alignment and no fixed offsets**: the 8-byte
magic, then a sequence of tagged entries. Class descriptors are inside the file, so a
reader is generic; it does not need compiled-in class layouts. Unlike a packfile, a
tagfile stores no member offsets, sizes or alignment: members are written one after
another in declaration order, and only those flagged present.

<a id="s3-1"></a>
### 3.1 Header (Confirmed)

| Offset | Size | Value |
|---|---|---|
| 0 | 8 | `1E 0D B0 CA CE FA 11 D0` (the little-endian u32 pair 0xCAB00D1E, 0xD011FACE) |

The byte-swapped magic would mean big-endian data (HX); it never occurs and a reader rejects
it.

<a id="s3-2"></a>
### 3.2 Signed varint (Confirmed)

Every integer in the tag stream (tags, counts, lengths, indices, `int` values) is a
sign-magnitude varint of 1 to 5 bytes:

1. First byte *b*: bit 0 is the **sign** (1 = negative); bits 1–6 are the low 6 bits of the
   magnitude; bit 7 = 1 means another byte follows.
2. Each following byte contributes its low 7 bits as the next 7 magnitude bits (bits 6–12,
   13–19, 20–26, 27–31); its bit 7 again means "more".
3. The value is the magnitude, negated if the sign bit was set.

Observed: 1-, 2- and 3-byte forms; every varint uses the shortest form; negative zero never
occurs. A reader accepts up to 5 bytes (a 32-bit magnitude) and reports a longer chain, or a
magnitude that does not fit `i32`, as malformed. Examples: 0 → `00`, 1 → `02`, −1 → `03`,
−2 → `05`, 36 → `48`, 81 → `A2 01`, 256 → `80 04`, 13,200 → `A0 CE 01`.

<a id="s3-3"></a>
### 3.3 Tag stream (Confirmed)

After the magic, the reader repeatedly reads a varint **tag** and the entry it introduces:

| Tag | Name | Payload | Observed |
|---|---|---|---|
| 1 | file info | varint version (always 3) | first entry of every tagfile |
| 2 | metadata | a class descriptor (§3.5) | yes |
| 3 | object (not remembered) | – | never (Reported, HX) |
| 4 | object (remembered) | varint class index, then the object body (§3.7) | yes |
| 5 | back-reference | – | never (Reported, HX) |
| 6 | null object | – | never (Reported, HX) |
| 7 | end of file | none | last entry of every tagfile |
| 0 or > 7 | – | – | never |

Observed tag sequences, all tagfiles: file info; every metadata entry; one object (two in 104
clip tagfiles); end. The end tag is always the last byte of the tagfile (54,060 of 54,060).
A reader rejects tags other than 1, 2, 4, 7, a missing or repeated file-info entry, a version
other than 3, and any byte after the end tag. It should accept metadata entries appearing
between objects (HX allows it), even though it never happens here.

<a id="s3-4"></a>
### 3.4 Strings and the string pool (Confirmed)

One pool per tagfile holds every string read so far: class names, member names, and
`cstring` values alike. It starts with two reserved entries (indices 0 and 1, both empty).
A string is read as a varint *n*:

- *n* > 0: the next *n* bytes are a new string (observed: ASCII only; treat as Latin-1),
  appended to the pool. The first new string gets index 2.
- *n* ≤ 0: a reference to the existing pool entry at index −*n*.

Observed: 1,578,537 new strings and 190,509 references across all tagfiles; references to
indices 0 and 1 never occur, so no empty string appears anywhere. A reference beyond the
pool's current end is malformed.

<a id="s3-5"></a>
### 3.5 Metadata entry: class descriptor (Confirmed)

| Order | Field | Encoding | Meaning |
|---|---|---|---|
| 1 | `name` | string (§3.4) | Class name |
| 2 | `class_version` | varint | 3 or 0 (§2.4); informational |
| 3 | `parent` | varint | Index of the parent class, 0 = none |
| 4 | `member_count` *m* | varint | Members declared by this class (not inherited ones) |
| 5 | members | *m* × member descriptor | |

Member descriptor:

| Order | Field | Encoding | Present when |
|---|---|---|---|
| 1 | `name` | string | always |
| 2 | `type` | varint (§3.6) | always |
| 3 | `tuple_size` | varint | `type` has the tuple bit (0x20); never observed |
| 4 | `class_name` | string | the basic kind of `type` is 8 (object) or 9 (struct) |

Class indices: index 0 is a built-in "none" class with no members. Each metadata entry gets
the next index, starting at 1, in file order. `parent` must name an already declared class
(observed: always an earlier index). The **full member list** of a class is the full list of
its parent followed by its own members, so members are numbered root class first. That
numbering is used by the presence bitmap (§3.7).

A `class_name` inside a member descriptor is resolved **by name, when a value is read**, not
when the metadata is read: `hkaAnimation` names `hkaAnnotationTrack` before that class is
declared.

<a id="s3-6"></a>
### 3.6 Member type codes (Confirmed for the codes listed as observed)

`type` = basic kind (low 4 bits) + 0x10 if array + 0x20 if tuple. Array and tuple together are
malformed; any bit above 0x3F is malformed.

| Basic kind | Name | Scalar value encoding | Observed as |
|---|---|---|---|
| 0 | void | no data; must never be flagged present | 0 (two `hkReferencedObject` members) |
| 1 | byte | 1 raw byte | 1 (only inside a struct array column) |
| 2 | int | varint | 2, 18 |
| 3 | real | f32 | 3, 19 |
| 4 | vec4 | 4 × f32 (scalar); see §3.8 for arrays | 4, 20 |
| 5 | vec8 | 8 × f32 | never |
| 6 | vec12 | 12 × f32 | 22 (array only) |
| 7 | vec16 | 16 × f32 | never |
| 8 | object | varint object index (§3.9) | 8 |
| 9 | struct | an embedded object body (§3.7) of the named class | 25 (array only) |
| 10 | cstring | string (§3.4) | 10, 26 |
| 11–15 | – | – | never; malformed |

Declared codes over all metadata: 0, 1, 2, 3, 4, 8, 10, 17, 18, 19, 20, 22, 25, 26. Codes that
are declared but whose values never appear (because the member is never present) are 19
(`referenceFloats`, `floats`), 26 (`floatSlots`) and the int and object members of
`hkaSkeletonLocalFrameOnBone`.

<a id="s3-7"></a>
### 3.7 Object body and presence bitmap (Confirmed)

An object entry (tag 4) gives a varint class index (≥ 1, < number of declared classes), then
the body:

1. Let *M* be the length of the class's full member list (§3.5). Read ⌈*M*/8⌉ bitmap bytes.
   Member *i* is present when bit (*i* mod 8) of byte ⌊*i*/8⌋ is set (least significant bit
   first). Bits at positions ≥ *M* are 0 in every observed bitmap; a reader may ignore them.
2. For each member in full-list order, if present, read its value (§3.8); absent members
   take their default (§3.10).

Each object entry is assigned the next **object index**, starting at 1 in each tagfile.
Object 1 is the root.

<a id="s3-8"></a>
### 3.8 Value encodings (Confirmed where observed)

| Member type | Encoding |
|---|---|
| scalar byte, int, real, vec*k*, object, cstring | as in §3.6 |
| scalar struct (9) | an object body (§3.7) of the class named by `class_name` (Reported: if `class_name` is empty, a varint class index precedes the body; never observed) |
| byte array (17) | varint count *n*, then *n* raw bytes |
| int array (18) | varint count *n*, then a varint **width prefix**, then *n* varints. The prefix is 4 in all 78,155 int arrays; its value never changes how the elements are read (they are varints). |
| real array (19) | varint count *n*, then *n* × f32 (Reported, never observed) |
| vec4 array (20) | varint count *n*, then a varint **component count** *k* (1–4), then *n* × *k* × f32. Missing components are 0. Observed *k*: 4 (177 arrays) and 3 (37 arrays). |
| vec12 array (22) | varint count *n*, then *n* × 12 × f32 |
| cstring array (26) | varint count *n*, then *n* strings (Reported, never observed) |
| struct array (25) | §3.9 |
| tuples (0x20 bit) | as arrays, but with the count fixed by `tuple_size` and no count varint; a byte tuple is `tuple_size` raw bytes (Reported, never observed) |

Counts are never negative. A reader bounds every count by the bytes remaining (each element
needs at least one byte, or its fixed float size) before allocating.

<a id="s3-9"></a>
### 3.9 Struct arrays (column-wise) and object references (Confirmed)

A struct array (type 25) is stored **column by column**, not element by element:

1. varint count *n*;
2. the element class is the member's `class_name` (Reported: an empty `class_name` would be
   followed by a varint class index; never observed);
3. **one** presence bitmap for the whole array, sized by the element class's full member list;
4. for each present member, in full-list order, a column of *n* values. A column of a scalar
   member is encoded exactly like an array of that member's type **without the leading count**:
   an `int` column has the width prefix, a `vec4` column the component-count prefix, a
   `byte` column is *n* raw bytes, a `cstring` column *n* strings.

Observed columns: `hkaBone.name` (cstring), `hkaBone.lockTranslation` (byte),
`hkaAnnotationTrack.trackName` (cstring). A present column whose member is itself an array
or struct never occurs; a reader classifies it as unsupported (§9.3) rather than guessing.

An `object` value is a varint object index: 0 = null, otherwise the index of a remembered
object (§3.7). The referenced object may come **later** in the file (forward reference):
in 104 clip tagfiles, object 1 (the animation) refers to object 2. After the end tag every
non-null reference must resolve to an object of the stream; an unresolved reference is
malformed. Observed references: only `hkaAnimation.extractedMotion`.

<a id="s3-10"></a>
### 3.10 Absent members and defaults (Confirmed by consistency)

The writer omits a member when its value is the default; the reader substitutes:

| Type | Default |
|---|---|
| int, byte | 0 |
| real | 0.0 |
| vec*k* | all components 0.0 |
| cstring | none (no string) |
| object | null |
| any array | empty |
| struct | all members default |

Evidence: whole columns are absent exactly when every value would be the default (bone
names absent in 4,658 skeletons whose bones all lack a name; `lockTranslation` present only
in 564 skeletons, and then 1 for every bone); `numberOfFloatTracks`, `floatOffsets`,
`transformOffsets` and `endian` are absent in every clip, consistent with 0 or empty.

<a id="s3-11"></a>
### 3.11 Unknown, extra and missing members

- A reader finds members **by name** in the full member list, never by position, because
  the member list comes from the file.
- A present member the reader does not know is decoded with the generic rules and ignored.
  An unknown class is decoded generically and reported as an unknown object (§9.3); it does
  not stop the decode of other objects.
- A member the reader needs but that is absent takes its default (§3.10). Whether that
  default makes the object unusable is checked by the object's own invariants (§10), not by
  the container layer.
- A member whose declared type differs from the type in §4 (for example `numFrames` declared
  as real) is a type mismatch (§9.3).

<a id="s3-12"></a>
### 3.12 Forms that never occur

Tags 3, 5 and 6; tuples; scalar structs; struct arrays with an empty `class_name`; real and
cstring array values; vec8, vec16; array-typed columns in struct arrays; big-endian magic.
Their encodings above are Reported (HX) and untested. A reader may implement them, but must
not need them, and the fuzz targets must still reach their rejection paths.

---

<a id="s4"></a>
## 4. Classes present

<a id="s4-1"></a>
### 4.1 Class table (Confirmed)

Every tagfile that declares one of these classes declares it with exactly this member list
(name, type code, class name). "Full index" is the member's position in the full member list
used by the bitmap.

| Class | Version | Parent | Declared in | Instances |
|---|---|---|---|---|
| `hkBaseObject` | 0 | – | all 54,060 | never (base) |
| `hkReferencedObject` | 0 | `hkBaseObject` | all 54,060 | never (base) |
| `hkaSkeleton` | 3 | `hkReferencedObject` | 28,555 (0x04) | 28,555 objects |
| `hkaBone` | 0 | – | 28,555 | 53,370 struct elements |
| `hkaSkeletonLocalFrameOnBone` | 0 | – | 28,555 | never |
| `hkLocalFrame` | 0 | `hkReferencedObject` | 28,555 | never |
| `hkaAnimation` | 3 | `hkReferencedObject` | 25,395 | never directly (base) |
| `hkaAnimatedReferenceFrame` | 0 | `hkReferencedObject` | 25,505 | never directly (base) |
| `hkaAnnotationTrack` | 0 | – | 25,395 | 214,186 struct elements |
| `hkaAnnotationTrackAnnotation` | 0 | – | 25,395 | never |
| `hkaSplineCompressedAnimation` | 0 | `hkaAnimation` | 24,800 | 24,800 objects |
| `hkaInterleavedUncompressedAnimation` | 0 | `hkaAnimation` | 595 | 595 objects |
| `hkaDefaultAnimatedReferenceFrame` | 0 | `hkaAnimatedReferenceFrame` | 214 | 214 objects |

Base members (full indices 0 and 1 of every class derived from `hkReferencedObject`):
`memSizeAndFlags` (0, void) and `referenceCount` (0, void). They are never present.

<a id="s4-2"></a>
### 4.2 Skeleton: `hkaSkeleton` (Confirmed, 28,555 of 28,555)

| Full index | Member | Type | Present in | Meaning |
|---|---|---|---|---|
| 2 | `name` | 10 cstring | 0 | – |
| 3 | `parentIndices` | 18 int array | 28,555 | Parent bone index per bone, −1 for a root |
| 4 | `bones` | 25 struct array of `hkaBone` | 28,555 | One element per bone |
| 5 | `referencePose` | 22 vec12 array | 28,555 | Bind (reference) pose per bone, local to the parent |
| 6 | `referenceFloats` | 19 real array | 0 | – |
| 7 | `floatSlots` | 26 cstring array | 0 | – |
| 8 | `localFrames` | 25 struct array of `hkaSkeletonLocalFrameOnBone` | 0 | – |

Observed object bitmap: `38 00` in every skeleton.

`hkaBone`: `name` (10 cstring; column present in 23,897 skeletons), `lockTranslation`
(1 byte; column present in 564 skeletons, value 1 for every bone there; 0 elsewhere). The two
columns are never present together in this install.

`hkaSkeletonLocalFrameOnBone`: `localFrame` (8 object, class `hkLocalFrame`), `boneIndex`
(2 int). `hkLocalFrame` has no members. Neither is ever instantiated.

**Reference pose element** (12 f32, one vec12):

| Floats | Meaning |
|---|---|
| 0–2 | translation x, y, z |
| 3 | 0.0 (all 53,370 bones) |
| 4–7 | rotation quaternion x, y, z, w (w last; identity is 0, 0, 0, 1). Unit length within 1.3e-7 on every bone. |
| 8–10 | scale x, y, z. Within 1e-3 of (1, 1, 1) on 53,146 bones; 82 other distinct values on the remaining 224 (in 181 skeletons). Bit-exactly (1, 1, 1) on 52,572 bones: 574 more carry rounding noise below 1e-3 (CNS2). |
| 11 | 0.0 |

The transform of a bone relative to its parent applies scale, then rotation, then translation
(Reported: the usual meaning of this translation/rotation/scale layout; composition rules
belong to `skeleton-anim.md`). Floats 3 and 11 are padding; a reader ignores them.

**Bones**: count = `parentIndices` length = `referencePose` length = `bones` length in
28,555 of 28,555. Bone *i*'s parent is −1 or an index **less than** *i* (parents always
precede children; 33,753 roots, 19,617 child links). Skeletons have 1 root (23,365), 2 roots
(5,183), 3 roots (6) or 4 roots (1) (recounted 2026-10-05, CNS2; the earlier "3 roots (7)"
merged the last two).

**Bone names** (by pattern only): 16,787 skeletons have ASCII identifier names (a rig-family
prefix, an underscore and a lowercase bone token); 7,110 have the decimal strings of their
own indices in order ("0", "1", ...); 4,658 have no names (column absent). Where names are
present they are unique within the skeleton (every multi-bone named skeleton).

The bone order is the order mesh bone indices address (`mesh.md` §5.7) and, by position,
the order clip tracks address (§4.4, H16).

<a id="s4-3"></a>
### 4.3 Animation base: `hkaAnimation` (Confirmed)

| Full index | Member | Type | Present in | Meaning |
|---|---|---|---|---|
| 2 | `type` | 2 int | 25,395 | 3 on every spline clip, 1 on every interleaved clip |
| 3 | `duration` | 3 real | 25,395 | Seconds |
| 4 | `numberOfTransformTracks` | 2 int | 25,395 | Tracks; ≥ 1 |
| 5 | `numberOfFloatTracks` | 2 int | 0 | Always 0 |
| 6 | `extractedMotion` | 8 object (`hkaAnimatedReferenceFrame`) | 104 | Reference to object 2 of the same tagfile (§4.5) |
| 7 | `annotationTracks` | 25 struct array of `hkaAnnotationTrack` | 5,602 | §4.4 |

A decoder classifies a clip by the **class name** of object 1 and may cross-check `type`.

<a id="s4-4"></a>
### 4.4 Annotation tracks (Confirmed)

`hkaAnnotationTrack`: `trackName` (10 cstring), `annotations` (25 struct array of
`hkaAnnotationTrackAnnotation`). `hkaAnnotationTrackAnnotation`: `time` (3 real), `text`
(10 cstring).

In the 5,602 clips that carry `annotationTracks`:

- the array length equals `numberOfTransformTracks` (5,602 of 5,602): annotation track *i*
  belongs to transform track *i*;
- the struct-array bitmap is `01`: only `trackName` is present; **`annotations` is never
  present, so no clip contains a single timed annotation (event)**;
- every track name is present, non-empty and unique within its clip (214,186 names in all;
  1 to 143 per clip, median 36). Pattern: ASCII identifiers made of a rig-family prefix, an
  underscore and a mixed-case bone token.

Matched case-insensitively against the bone-name lists of all 0x04 skeletons, the track
name list of 975 clips equals some skeleton's bone list **in the same order**, which supports
positional binding (H16). The other 4,627 match no 0x04 skeleton, not even as a set.

**Binding (answered by `skeleton-anim.md` §3, 2026-10-05; H16).** No clip names its skeleton.
Track *j* drives bone *j* of whatever skeleton the playing object has; a clip is compatible
exactly when its track count equals the bone count, and an incompatible clip is not played.
Bone and track names are never needed (they are a diagnostic only, `skeleton-anim.md` §3.5);
clips aimed at the 7,110 index-named and the 4,658 unnamed skeletons bind by count like every
other clip. Which clip an object plays comes from its script tables (`script-table.md`), not
from the clip. Animation events (footsteps, sounds, effects) are not in the Havok data either;
they come from the script tables.

The decoder exposes the annotation tracks as a list of (track name, list of (time, text)),
which is a list of names with empty event lists for every clip of this install.

<a id="s4-5"></a>
### 4.5 Extracted motion: `hkaDefaultAnimatedReferenceFrame` (Confirmed)

| Full index | Member | Type | Present in | Meaning |
|---|---|---|---|---|
| 2 | `up` | 4 vec4 | 214 | Up axis (unit, w = 0) |
| 3 | `forward` | 4 vec4 | 214 | Forward axis (unit, w = 0) |
| 4 | `duration` | 3 real | 214 | Equals the clip's `duration` (214 of 214) |
| 5 | `referenceFrameSamples` | 20 vec4 array | 214 | One sample per frame |

Observed object bitmap: `3C`. Axes: up (0, 0, 1) with forward (1, 0, 0) in 207 objects; up
(0, 1, 0) with forward (0, 0, 1) in 7 (all in leading tagfiles).

Where they occur:

| Case | Records | Reference frame objects |
|---|---|---|
| Leading tagfile (envelope field 4) **and** embedded as object 2 of the clip tagfile, referenced by `extractedMotion` | 102 | 2 per record, sample-for-sample identical (102 of 102) |
| Leading tagfile only | 8 (7 spline, 1 interleaved) | 1 |
| Embedded only | 2 | 1 |

Samples: count = clip frame count in 211 of 214 (one leading frame is 1 short, two are 44
short). Each sample is a vec4 with component count 4 (177 objects) or 3 (37 objects, the
fourth component then defaults to 0). Components x, y, z are a translation of the character's
reference frame (the first sample is all zero in 175 objects); w ranges from −2.36 to π and is
monotonic in 53 of the 88 four-component leading objects.

**Meaning (`skeleton-anim.md` §7.3, Confirmed there):** *w* is an accumulated yaw angle in
radians about `up`; x, y, z an accumulated translation; the two copies of a record are
identical whenever both exist (102 of 102), so either may be used.

**Sampling (BIN, 2026-10-05, H20).** The animation middleware inside the client samples this
object as follows. With *S* the object's own sample count and *D* its `duration`:

1. *t* ≥ *D*: the last sample. *t* < 0: the first sample.
2. Otherwise the fractional sample index is *t* × (*S* − 1) / *D*; take its integer part *i*,
   limited to *S* − 2 (the fraction then measured from that *i*), and interpolate samples *i*
   and *i* + 1 linearly in all four components.
3. The result is a transform with translation (x, y, z), rotation by the angle *w* about the
   `up` axis, and unit scale.

So a short array is **spread over the whole duration** by its own count; it is not placed on
the clip's frame grid. (`skeleton-anim.md` §7.3 says "hold its last sample"; see §15.)
Whether the client's player always reaches extracted motion through this routine was not
traced; it is the object's own sampler and Tessera follows it.

<a id="s4-6"></a>
### 4.6 Classes that do not occur (Confirmed)

No tagfile declares `hkRootLevelContainer`, `hkaAnimationContainer`, `hkaAnimationBinding`,
`hkaSkeletonMapper`, `hkaMeshBinding`, `hkaDeltaCompressedAnimation`,
`hkaWaveletCompressedAnimation` or any physics or scene class. In particular there is **no
binding object**: no track-to-bone index array, no skeleton name in a clip, and no blend hint.
Clip-to-skeleton binding and blending rules belong to `skeleton-anim.md`; the closest thing to
a per-clip blend datum is the envelope's `track_weights` (§2.3).

---

<a id="s5"></a>
## 5. Spline-compressed clip: `hkaSplineCompressedAnimation`

<a id="s5-1"></a>
### 5.1 Object members (Confirmed, 24,800 of 24,800)

| Full index | Member | Type | Present in | Meaning |
|---|---|---|---|---|
| 8 | `numFrames` | 2 int | 24,800 | Frames *N* in the clip, ≥ 2; first at t = 0, last at t = duration |
| 9 | `numBlocks` | 2 int | 24,800 | Blocks *B* ≥ 1 |
| 10 | `maxFramesPerBlock` | 2 int | 24,800 | *F* = 256 in every clip |
| 11 | `maskAndQuantizationSize` | 2 int | 24,800 | Bytes of the mask section = 4 × tracks + float tracks |
| 12 | `blockDuration` | 3 real | 24,800 | (*F* − 1) × `frameDuration` |
| 13 | `blockInverseDuration` | 3 real | 24,800 | 1 / `blockDuration` |
| 14 | `frameDuration` | 3 real | 24,800 | `duration` / (*N* − 1) |
| 15 | `blockOffsets` | 18 int array | 24,800 | *B* byte offsets into `data`, one per block |
| 16 | `floatBlockOffsets` | 18 int array | 24,800 | *B* values: the length of each block's transform section, relative to the block start (with no float tracks, the float section would start there) |
| 17 | `transformOffsets` | 18 int array | 0 | Empty |
| 18 | `floatOffsets` | 18 int array | 0 | Empty |
| 19 | `data` | 17 byte array | 24,800 | All blocks |
| 20 | `endian` | 2 int | 0 | 0 (little-endian data) |

Observed object bitmaps: `1C FF 09` (19,094 clips), `9C FF 09` (with `annotationTracks`,
5,602), `5C FF 09` (with `extractedMotion`, 104).

<a id="s5-2"></a>
### 5.2 Blocks (Confirmed, 31,294 of 31,294)

- Block *b* covers clip frames *b* × (*F* − 1) through *b* × (*F* − 1) + *F_b* − 1, where
  *F_b* = min(*F*, *N* − *b* × (*F* − 1)) is its frame count. Consecutive blocks **share
  one frame**: the last frame of block *b* is the first frame of block *b* + 1.
- *B* = max(1, ⌈(*N* − 1) / (*F* − 1)⌉) in 24,800 of 24,800.
- `blockOffsets[0]` = 0. Every block starts at a multiple of 16 bytes from the start of `data`.
  `blockOffsets[b + 1]` = the next multiple of 16 at or after `blockOffsets[b]` +
  `floatBlockOffsets[b]`, and the length of `data` is the next multiple of 16 after the last
  block's transform section (31,294 of 31,294). The 0, 4, 8 or 12 bytes between are padding.
- A block's transform section is: the mask section (§5.3), then one **track record** per
  transform track, in track order. Parsing all track records ends exactly at
  `floatBlockOffsets[b]` bytes after the block start (31,294 of 31,294).

Alignment ("pad to 4") below always means: advance to the next offset that is a multiple of
4 from the start of `data` (identical to "from the block start", since blocks are 16-aligned).

<a id="s5-3"></a>
### 5.3 Mask section and track mask (Confirmed)

The mask section is 4 bytes per transform track, then one byte per float track (never
present), then pad to 4. The 4 bytes of a track's mask:

| Byte | Name | Content |
|---|---|---|
| 0 | quantisation | bits 0–1: position quantisation; bits 2–5: rotation quantisation; bits 6–7: scale quantisation |
| 1 | position mask | bits 0, 1, 2: component x, y, z is **static**; bits 4, 5, 6: component x, y, z is **dynamic** (spline); bits 3 and 7: unused (0 in every mask) |
| 2 | rotation mask | low nibble ≠ 0: static; high nibble ≠ 0: dynamic. Dynamic wins if both are non-zero (observed: 73,732 dynamic masks also have low-nibble bits). Both zero: identity. |
| 3 | scale mask | as the position mask |

Quantisation codes:

| Field | Code | Meaning | Observed |
|---|---|---|---|
| position, scale | 0 | 8-bit control points | never (Reported, HL) |
| position, scale | 1 | 16-bit control points | **every track** |
| position, scale | 2, 3 | – | never; unsupported |
| rotation | 0 | 32-bit polar quaternion | never (Reported, HL) |
| rotation | 1 | 40-bit quaternion (§5.6) | **every track** |
| rotation | 2, 3, 4, 5 | 48-bit, 24-bit, 16-bit, uncompressed (Reported names, HL) | never |
| rotation | 6–15 | – | never; unsupported |

The quantisation byte is 0x45 in all 1,645,618 track masks. A component is never both static
and dynamic. Per-component bits in the rotation mask only select the channel kind; a static
or dynamic 40-bit rotation always stores whole quaternions.

<a id="s5-4"></a>
### 5.4 Track record (Confirmed)

A track record is three channels in this order: position, rotation, scale.

**Vector channel (position or scale)**. Let *S* be the set of static components and *D* the
dynamic ones (from the channel's mask).

- *D* empty (**constant channel**): for each component x, y, z in order, if it is in *S*, one
  f32 value. Components in neither set take the default: 0.0 for position, 1.0 for scale.
  No padding (everything is 4-byte sized).
- *D* not empty (**spline channel**):

| Order | Field | Encoding |
|---|---|---|
| 1 | `n` | u16: index of the last control point (control points = *n* + 1) |
| 2 | `p` | u8: spline degree |
| 3 | knots | (*n* + *p* + 2) × u8 |
| 4 | – | pad to 4 |
| 5 | per component x, y, z in order | in *S*: one f32 value; in *D*: f32 `min`, f32 `max`; in neither: nothing (default as above) |
| 6 | control points | for *k* = 0 … *n*: for each component in *D* in x, y, z order, one u16 (16-bit) or u8 (8-bit) |
| 7 | – | pad to 4 |

A 16-bit value *q* dequantises to `min` + (`max` − `min`) × *q* / 65535; an 8-bit value to
`min` + (`max` − `min`) × *q* / 255 (Reported). Static components of a spline channel are
constant over the block.

**Rotation channel (40-bit)**:

- identity: nothing; the value is (0, 0, 0, 1);
- static: one 5-byte quaternion (§5.6);
- dynamic: u16 `n`, u8 `p`, (*n* + *p* + 2) u8 knots, then (*n* + 1) × 5-byte quaternions,
  with **no padding** between knots and quaternions;
- then, in all three cases, pad to 4.

**Knot vectors** (all 1,065,899 observed, every channel kind):

- values are frame numbers **local to the block** (0 … *F_b* − 1), as u8;
- non-decreasing; the first *p* + 1 knots are 0 and the last *p* + 1 knots are *F_b* − 1
  (clamped), so the curve passes through the first and last control points;
- interior knots lie strictly between 0 and *F_b* − 1 and repeat at most *p* times
  (degree 3: multiplicity 1, 2 or 3 all common; degree 1: always 1), so curves are
  continuous but zero-length knot spans occur;
- degree *p* is 1 or 3; *p* ≤ *n*; *n* + 1 ≤ *F_b* (control points: 2 to 256).

| Channel | Degree 1 | Degree 3 |
|---|---|---|
| position spline | 23,786 | 92,851 |
| rotation dynamic | 278,148 | 664,206 |
| scale spline | 334 | 6,574 |

<a id="s5-5"></a>
### 5.5 Channel kinds (Confirmed, counts per track per block)

| Channel | Default (no data) | Constant / static | Spline / dynamic |
|---|---|---|---|
| position | 81,412 | 1,447,569 | 116,637 |
| rotation | 432,027 (identity) | 271,237 | 942,354 |
| scale | 1,636,005 | 2,705 | 6,908 |

Per component: position 1,283,112 default, 3,370,056 static, 283,686 dynamic; scale
4,915,627 default, 5,893 static, 15,334 dynamic. Scale is not always 1: 9,613 channels carry
scale data (corrects MV, "scales uniformly 1.0").

<a id="s5-6"></a>
### 5.6 The 40-bit quaternion (Confirmed)

Read 5 bytes as a little-endian unsigned 40-bit integer *v* (byte 0 holds bits 0–7, byte 4
holds bits 32–39).

| Bits of *v* | Field | Meaning |
|---|---|---|
| 0–11 | *a₀* | first stored component, unsigned 12-bit |
| 12–23 | *a₁* | second stored component |
| 24–35 | *a₂* | third stored component |
| 36–37 | *i* | index (0 = x, 1 = y, 2 = z, 3 = w) of the **dropped** component |
| 38 | *s* | sign of the dropped component (1 = negative) |
| 39 | – | 0 in every observed value; ignore |

Reconstruction:

1. For each stored field, *c_k* = (*a_k* − 2047) / (2047 × √2). The constant
   1 / (2047 × √2) ≈ 0.000345436. Raw fields range 0 … 4094 here, so *c_k* lies in
   [−1/√2, +1/√2].
2. The dropped component *d* = √(max(0, 1 − *c₀*² − *c₁*² − *c₂*²)), negated if *s* = 1.
3. The quaternion (x, y, z, w) has *d* at position *i*; the remaining three positions, in
   ascending order, receive *c₀*, *c₁*, *c₂*. For example, *i* = 1 gives (*c₀*, *d*, *c₁*,
   *c₂*); *i* = 3 gives (*c₀*, *c₁*, *c₂*, *d*).

The dropped component is always the one of largest magnitude (13,136,165 of 13,136,165
checked, tolerance 1e-3), and 1 − Σ*c²* is never negative in this data. Index
distribution: w 95.2 %, z 2.1 %, x 1.8 %, y 0.9 %. Reconstructed quaternions are unit length
by construction.

<a id="s5-7"></a>
### 5.7 Padding summary (Confirmed)

| Where | Rule |
|---|---|
| after the mask section | pad to 4 |
| vector spline channel: after the knots | pad to 4 |
| vector spline channel: after the control points | pad to 4 |
| constant vector channel | none needed |
| rotation, 40-bit: between knots and quaternions | none |
| after every rotation channel | pad to 4 |
| between blocks and at the end of `data` | pad to 16 |

Padding bytes are not checked by the decoder (their content is irrelevant).

<a id="s5-8"></a>
### 5.8 Float tracks (Confirmed absent)

`numberOfFloatTracks` is absent (0) in every clip, every `floatOffsets` is empty, and every
block's float section is empty. A clip with float tracks is classified as unsupported
(§9.3).

---

<a id="s6"></a>
## 6. Uncompressed interleaved clip: `hkaInterleavedUncompressedAnimation` (Confirmed, 595 of 595)

| Full index | Member | Type | Present in | Meaning |
|---|---|---|---|---|
| 8 | `transforms` | 22 vec12 array | 595 | All samples |
| 9 | `floats` | 19 real array | 0 | Empty (no float tracks) |

Observed object bitmap: `1C 01`.

- Each element has the reference-pose layout of §4.2 (translation, quaternion x y z w,
  scale; floats 3 and 11 are 0). Quaternions are unit length within 1e-3 (1,605,753 of
  1,605,753).
- With *T* = `numberOfTransformTracks`, the array length is a multiple of *T* (595 of 595);
  frames *N* = length / *T* (3 to 542, median 49).
- Order is **frame-major**: element *f* × *T* + *j* is track *j* at frame *f*.
- Frame *f* is at time *f* × `duration` / (*N* − 1). Effective rates are near 30 frames
  per second for most clips (16 clips at 120); durations range 0.0167 s to 18 s.

---

<a id="s7"></a>
## 7. Algorithms (in words)

<a id="s7-1"></a>
### 7.1 Reading a tagfile

1. Check the magic (§3.1). Create the string pool with its two reserved entries, the class
   table with the built-in class 0, and an empty object table; the next object index is 1.
2. Read tags until tag 7 (§3.3), dispatching on the tag: file info (check version 3),
   metadata (append a class, §3.5), remembered object (read the class index and body, §3.7,
   assign the next object index). Any other tag is an error.
3. After tag 7, require that the stream is exhausted and that every non-null object
   reference resolves (§3.9).
4. Enforce limits on hostile input: member counts and array counts bounded by the bytes
   remaining; struct nesting depth bounded (observed depth: 2, an object holding a struct
   array; a limit of 16 is ample); total objects and classes bounded by the input length.

<a id="s7-2"></a>
### 7.2 Reading a clip record (0x05)

1. Read the envelope (§2.3). If `has_reference_frame` = 1, read and decode the leading
   tagfile; it must contain an `hkaDefaultAnimatedReferenceFrame`.
2. Decode the clip tagfile. Object 1 must be an `hkaSplineCompressedAnimation` or an
   `hkaInterleavedUncompressedAnimation`. If `extractedMotion` is non-null it must reference
   an `hkaDefaultAnimatedReferenceFrame` of the same tagfile.
3. Read the weight array; require that the record ends exactly there.
4. Check `track_count` against `numberOfTransformTracks` (mismatch: §9.3) and record a
   duration mismatch as a warning, not an error.
5. Build the clip model: kind, duration, frame count, track count, per-track channels,
   annotation tracks, reference frame (prefer the embedded one, else the leading one), and
   the weight array.

<a id="s7-3"></a>
### 7.3 Parsing a spline block

For block *b*: start at `blockOffsets[b]`; the block may extend to the next block offset
(or the end of `data`). Read the masks (§5.3) and pad. For each track, parse the position
channel, the rotation channel and the scale channel as §5.4 describes, applying §5.7 padding.
Every read is bounds-checked against the block end. When all tracks are parsed, the position
must equal `blockOffsets[b]` + `floatBlockOffsets[b]`. Reject unsupported quantisation codes
and degrees outside 1 … *n* or above 7 (a Tessera bound; only 1 and 3 occur) before reading
control points. Store each channel as either a
constant 3- or 4-vector or a curve (degree, knots as numbers, dequantised control points).
Decoding a block once and caching it is cheaper than re-parsing per sample.

<a id="s7-4"></a>
### 7.4 From time to block and local frame (spline clips)

Given a time *t* in seconds:

1. If *t* is not finite or below 0, use 0. If *t* is above `duration`, use `duration`
   (clamping at the end is the decoder's rule; looping and wrapping are the caller's
   business in `tessera-anim`).
2. Block *b* = ⌊*t* × `blockInverseDuration`⌋, then limited to *B* − 1.
3. Local frame *u* = (*t* − *b* × `blockDuration`) / `frameDuration`, then limited to the
   range 0 … *F_b* − 1.

At an exact block boundary both blocks give the same value (the shared frame), so either
rounding is acceptable. At *t* = `duration` this selects the last block and *u* = *F_b* − 1,
the clip's last frame, which equals the last control point of each curve.

<a id="s7-5"></a>
### 7.5 Evaluating a curve at local frame *u* (B-spline, NB)

A curve has degree *p*, control points *P₀* … *Pₙ* and knots *U₀* … *U*ₙ₊ₚ₊₁ (the u8 values
as numbers).

1. **Knot span**: if *u* ≥ *U*ₙ₊₁, the span is *s* = *n*. Otherwise *s* is the index in
   *p* … *n* with *U_s* ≤ *u* < *U*ₛ₊₁ (it is unique and its span has non-zero length; a
   binary or linear search over that range finds it).
2. **Basis functions**: compute the *p* + 1 non-zero B-spline basis functions
   *N*ₛ₋ₚ … *N_s* of degree *p* at *u* with the Cox–de Boor recurrence: start from degree 0,
   where only the function of span *s* is 1, and raise the degree one step at a time; at
   degree *j* each function is the left neighbour's lower-degree function weighted by
   (*u* − *U_i*) / (*U*ᵢ₊ⱼ − *U_i*) plus its own weighted by (*U*ᵢ₊ⱼ₊₁ − *u*) /
   (*U*ᵢ₊ⱼ₊₁ − *U*ᵢ₊₁). Any fraction whose denominator is 0 counts as 0 (repeated knots).
   The functions are non-negative and sum to 1.
3. **Value**: Σ *N*ₛ₋ₚ₊ₖ × *P*ₛ₋ₚ₊ₖ for *k* = 0 … *p*, componentwise.

Properties a test can check: at *u* = 0 the value is *P₀*; at *u* = *F_b* − 1 it is *Pₙ*; a
degree-1 curve is the straight line between neighbouring control points; with all control
points equal the value is that point.

<a id="s7-6"></a>
### 7.6 Evaluating a rotation curve

1. Take the *p* + 1 control points that §7.5 uses. Walking from the first to the last of
   them, negate any quaternion whose dot product with the (possibly negated) previous one is
   negative, so the window lies in one hemisphere.
2. Blend componentwise with the basis functions (§7.5).
3. Normalise to unit length. If the length is below 1e-6, return the control point with the
   largest basis weight instead.

Without normalisation the blended quaternion is short by up to 5 % (observed over 22.7
million sampled evaluations; 99.95 % are within 1 %). Step 1 matters for 3,743 neighbouring
control-point pairs (of 51.7 million) with a negative dot product, 3,460 of them close to −1: the same rotation
stored with opposite sign. Step 1 makes such a pair a constant rotation, as intended; blending
them unaligned would pass through a near-zero quaternion.

> **OPEN:** Whether the original aligns signs before blending (step 1) or plays a visible
> glitch on those 3,743 pairs (0.007 %). Read again on 2026-10-07 (BIN question 5): the
> decompressor's quaternion path rebuilds the dropped component and normalises, but the
> routine that blends a window of control points was not identified, so the question stays
> open. The recommendation stands unless the visual checklist shows otherwise.

<a id="s7-7"></a>
### 7.7 Sampling a spline clip

For time *t*: find *b* and *u* (§7.4); for each track *j*, evaluate the three channels of
track *j* in block *b*: a constant channel yields its value; a vector spline yields the
curve value at *u* for its dynamic components and the stored or default value for the other
components; a rotation curve per §7.6. The result per track is (translation, rotation,
scale), local to the parent bone, in the same layout as a reference-pose element.

Two clips contain extreme but finite translation values (one track above 10⁴ m, one above
10¹⁰ m); the decoder returns them unchanged and `tessera-anim` may treat them as broken.

<a id="s7-8"></a>
### 7.8 Sampling an interleaved clip and a reference frame (BIN, H19–H20)

The animation middleware inside the client samples an interleaved clip as follows (BIN). With
*N* frames (`transforms` length / tracks) and *D* = `duration`:

1. *t* ≥ *D*: every track takes its last frame exactly.
2. Otherwise *x* = *t* × (*N* − 1) / *D*; *f* = the integer part of *x*. When *f* > *N* − 2,
   use *f* = *N* − 2 and α = 1; else α = *x* − *f*, limited to 0 … 1.
3. Per track: translation and scale are (1 − α) × *a* + α × *b* of frames *f* and *f* + 1
   (Tessera's normative arithmetic, §14; equal to *a* + α × (*b* − *a*) in exact arithmetic);
   rotation is the **normalised linear interpolation** (1 − α) × *q_a* + α′ × *q_b*, where α′ =
   α when the dot product of *q_a* and *q_b* is ≥ 0 and −α when it is negative, followed by
   normalisation (the client uses one Newton step of a reciprocal square root; an exact
   normalisation is equivalent within floating-point error).

The client's routine has no separate branch for *t* < 0: for −1 < *x* < 0 it returns frame 0
(α limited to 0), and for *x* ≤ −1 its index test treats the negative index as too large and
returns the **last** frame. Tessera clamps *t* to [0, *D*] first, as §7.4 does, and never
relies on that quirk. *N* = 1 (never observed) means a constant pose.

Reference frames (§4.5) are sampled with their **own** sample count *S* in place of *N*, over
their own `duration`, interpolating x, y, z and w linearly; *w* then becomes a rotation about
`up`. A short sample array is therefore stretched over the whole duration.
`tessera-havok`'s `ReferenceFrame::sample` follows this rule; `ReferenceFrame::sample_on_grid`
(clip frame grid, last sample held) does not match the client.

---

<a id="s8"></a>
## 8. Census (Confirmed, all records, 2026-10-04)

<a id="s8-1"></a>
### 8.1 Containers and objects

| Item | Count |
|---|---|
| Tagfiles, version 3 | 54,060 (28,555 in 0x04, 25,395 + 110 in 0x05) |
| Tagfiles of any other version, packfiles | 0 |
| `hkaSkeleton` objects | 28,555 (one per 0x04 tagfile) |
| `hkaSplineCompressedAnimation` | 24,800 |
| `hkaInterleavedUncompressedAnimation` | 595 |
| `hkaDefaultAnimatedReferenceFrame` | 214 (110 leading, 104 embedded) |
| `hkaBone` elements | 53,370 |
| `hkaAnnotationTrack` elements | 214,186 in 5,602 clips |
| `hkaAnnotationTrackAnnotation` elements | 0 |
| Unknown classes | 0 |

Tag sequences (tag numbers, "2 × 7" = seven metadata entries): skeleton tagfiles 1, 2 × 6,
4, 7 (28,555); clip tagfiles 1, 2 × 7, 4, 7 (25,291, one object) or 1, 2 × 8, 4, 4, 7 (104,
animation plus embedded reference frame); leading reference-frame tagfiles 1, 2 × 4, 4, 7
(110).

<a id="s8-2"></a>
### 8.2 Skeletons

| Statistic | Value |
|---|---|
| Bones min / median / max | 1 / 1 / 248 |
| Most common counts | 1 (22,434), 2 (5,344), 3 (110), 4 (79), 36 (68) |
| 30 or more bones | 293 skeletons (median 37; 36 ×68, 37 ×20, 89 ×11) |
| Roots per skeleton | 1 (23,365), 2 (5,183), 3 (6), 4 (1) (recounted 2026-10-05) |
| Name kinds | identifiers 16,787, index strings 7,110, absent 4,658 |
| `lockTranslation` column | 564 skeletons (1,344 bones, all 1) |
| Non-unit scale in reference pose | 224 bones beyond 1e-3 (181 skeletons); 798 not bit-exactly 1 |
| Decode completely | 28,555 of 28,555 |

<a id="s8-3"></a>
### 8.3 Clips

| Statistic | Spline (24,800) | Interleaved (595) |
|---|---|---|
| Tracks min / median / max | 1 / 47 / 248 | 1 / 43 / 86 |
| Duration min / median / max (s) | 0.016 / 1.5 / 218.25 | 0.0167 / 1.567 / 18.0 |
| Frames min / median / max | 2 / 110 / 13,096 | 3 / 49 / 542 |
| Blocks | 1 (21,026), 2 (2,533), 3 (721), 4 (276), 5 (121), up to 52; 31,294 in total | – |
| `maxFramesPerBlock` | 256 (all) | – |
| Frame rate (1 / `frameDuration`, or (*N* − 1) / duration) | ≈ 60 (19,293), ≈ 120 (4,746), ≈ 30 (132), other 629 (324 of them 2-frame holds); range 0.485 – 120 | ≈ 30 (most), 120 (16) |
| Quantisation (pos / rot / scale) | 16-bit / 40-bit / 16-bit on all 1,645,618 track masks | – |
| `data` bytes | 391,967,680 in total | – |
| With annotation tracks | 5,602 | 0 |
| With `track_weights` | 6,915 | 20 |
| With a reference frame (leading, embedded or both) | 111 | 1 |
| Decode completely | 24,800 of 24,800 | 595 of 595 |

Over all clips, tracks min / median / max are 1 / 45 / 248; the most common track counts are
62 (5,838 clips), 89 (4,958), 36 (3,180) and 35 (1,735). Envelope: `has_reference_frame` 0 in
25,285, 1 in 110; weight arrays empty in 18,460 and full in 6,935.

<a id="s8-4"></a>
### 8.4 Duration mismatches

In 21 records the envelope `duration` differs from the tagfile's:

| Envelope | Tagfile | Records |
|---|---|---|
| 0.8 | 1.0 | 5 |
| 0.75 | 1.0 | 5 |
| 0.75 | 1.2 | 5 |
| 0.75 | 1.25 | 5 |
| 250.467 | 1.467 | 1 |

All other 25,374 agree bit for bit. See the OPEN in §2.3.

---

<a id="s9"></a>
## 9. Exit criterion 1: what counts, and failure categories

<a id="s9-1"></a>
### 9.1 Skeleton record

A **skeleton record** is every record with DID prefix 0x04 in `client_general.dat`
(28,555 today). It **decodes** when: the setup's outer layout delivers field 3 (`setup.md`
§6.1); the tagfile parses per §3 with no leftover bytes; object 1 is an `hkaSkeleton`;
`parentIndices`, `bones` and `referencePose` have the same non-zero length; every parent
index is −1 or smaller than its own index; every reference-pose quaternion has a length
within 1e-3 of 1 and every float is finite. The criterion requires 100 % (today 28,555 of
28,555).

<a id="s9-2"></a>
### 9.2 Clip record

A **clip record** is every record with DID prefix 0x05 in `client_anim.dat` (25,395 today),
including the 110 with a leading reference-frame tagfile (each still holds exactly one
animation). It **decodes** when: the envelope parses with no leftover bytes (§2.3); every
tagfile parses (§3); object 1 of the clip tagfile is a supported animation (§5 or §6); for a
spline clip every block parses to exactly its declared length (§7.3) and every curve's
invariants hold (§10); for an interleaved clip the transform count is a positive multiple of
the track count; and sampling every frame of every track yields finite values. The criterion
requires at least 99 % (today 25,395 of 25,395).

<a id="s9-3"></a>
### 9.3 Failure categories

A decoder reports exactly one category per failed record. Counts are for this install.

| Category | Meaning | Count |
|---|---|---|
| `envelope.truncated` | 0x05 envelope shorter than its fields or lengths | 0 |
| `envelope.bad_flag` | `has_reference_frame` not 0 or 1 | 0 |
| `envelope.trailing_bytes` | bytes after the weight array | 0 |
| `envelope.weight_count` | weight count not 0 and not the track count | 0 |
| `tagfile.bad_magic` | magic wrong (including byte-swapped) | 0 |
| `tagfile.bad_version` | file-info version not 3, or file info missing | 0 |
| `tagfile.truncated` | end of input inside an entry, varint or string | 0 |
| `tagfile.bad_varint` | varint longer than 5 bytes or out of `i32` range | 0 |
| `tagfile.unknown_tag` | tag other than 1, 2, 4, 7 | 0 |
| `tagfile.bad_class_index` | class or parent index out of range | 0 |
| `tagfile.bad_string_ref` | string reference beyond the pool | 0 |
| `tagfile.bad_member_type` | type code with unknown kind or flags, or array and tuple together | 0 |
| `tagfile.unsupported_form` | a §3.12 form the reader does not implement (void present, array column, tuple ...) | 0 |
| `tagfile.unresolved_ref` | object reference to no object | 0 |
| `tagfile.trailing_bytes` | bytes after the end tag | 0 |
| `object.missing` | no skeleton (0x04) or no animation (0x05) as object 1 | 0 |
| `object.unknown_class` | object 1 has a class this spec does not describe | 0 |
| `object.type_mismatch` | a needed member is declared with a different type code | 0 |
| `object.inconsistent` | counts disagree (bones vs parents vs pose, blocks vs offsets, mask size, track count vs envelope), bad parent index | 0 |
| `spline.unsupported_quantisation` | a quantisation code other than those implemented | 0 |
| `spline.float_tracks` | `numberOfFloatTracks` > 0 | 0 |
| `spline.bad_mask` | a component both static and dynamic, or bits 3/7 set | 0 |
| `spline.bad_curve` | degree outside 1 … *n* or above 7 (Tessera bound), knots decreasing, not clamped, knot beyond the block's last frame, an interior knot repeated more than *p* times | 0 |
| `spline.block_overrun` | a channel reads past the block end | 0 |
| `spline.block_length` | parsing ends elsewhere than `floatBlockOffsets[b]` | 0 |
| `interleaved.bad_shape` | transform count not a positive multiple of the track count | 0 |
| `sample.non_finite` | a sampled value is NaN or infinite | 0 |

Assignments settled with the implementation (2026-10-05, §14): a negative count is
`tagfile.bad_varint`; a count beyond the bytes left (also of a column-less struct array) is
`tagfile.truncated`; Tessera's resource limits, vec4 component counts outside 1–4, object
arrays and object columns are `tagfile.unsupported_form`; a struct-array element class with no
declared name is `tagfile.bad_class_index`; the pose checks of §9.1, an `extractedMotion` of
another class, a spline clip with 0 tracks, negative track counts, `maxFramesPerBlock` < 2,
`numFrames` < 1 and a non-zero `endian` are `object.inconsistent`. Settled on 2026-10-07: more
than 1,048,576 decoded values in one tagfile (present object members plus stored struct-array
column values; fewer than 100 occur) is `tagfile.unsupported_form` (a Tessera bound).

---

<a id="s10"></a>
## 10. Invariants (Confirmed over all records)

| ID | Invariant |
|---|---|
| **HV1** | Every tagfile: magic; first entry file info version 3; tags only 1, 2, 4, 7; ends with tag 7 at its last byte. |
| **HV2** | Varints use the shortest form; no negative zero; string references never point at indices 0 or 1. |
| **HV3** | Presence-bitmap bits beyond the member count are 0; void members are never present. |
| **HV4** | A class name, wherever declared, always has the member list of §4. |
| **HV5** | Int-array width prefix = 4; vec4-array component count ∈ {3, 4}. |
| **HV6** | 0x05 envelope: flag ∈ {0, 1}; weight count ∈ {0, track count}; weights ∈ [0, 1]; zero leftover bytes; `track_count` = `numberOfTransformTracks`. |
| **HV7** | Skeleton: equal lengths of parents, bones, pose (≥ 1); parent −1 or < own index; quaternion length within 1e-6 of 1; floats 3 and 11 of each pose element = 0; names all present and unique or column absent. |
| **HV8** | Spline object: `blockOffsets` and `floatBlockOffsets` lengths = *B* ≥ 1; *B* = max(1, ⌈(*N* − 1)/(*F* − 1)⌉); `maskAndQuantizationSize` = 4 × tracks + float tracks; `frameDuration` × (*N* − 1) = `duration` (abs 1e-4); `blockDuration` = (*F* − 1) × `frameDuration` (rel 1e-6); `blockDuration` × `blockInverseDuration` = 1 (1e-5). |
| **HV9** | Spline blocks: `blockOffsets[0]` = 0; every offset a multiple of 16; next offset = align16(offset + `floatBlockOffsets`); `data` length = align16(last offset + last `floatBlockOffsets`). |
| **HV10** | Track masks: quantisation byte 0x45; position and scale bits 3 and 7 clear; no component static and dynamic. |
| **HV11** | Curves: degree ∈ {1, 3}; 1 ≤ *p* ≤ *n*; *n* + 1 ≤ *F_b*; knots non-decreasing, clamped at 0 and *F_b* − 1; interior multiplicity ≤ *p*; dequantisation ranges `min` ≤ `max`. |
| **HV12** | 40-bit quaternions: bit 39 = 0; raw fields ≤ 4094; the dropped component has the largest magnitude. |
| **HV13** | Interleaved: transform count = tracks × frames, frames ≥ 2; quaternions unit within 1e-3; floats 3 and 11 = 0. |
| **HV14** | Reference frame: `duration` = clip duration; `up` and `forward` unit axes with w = 0. |

A reader enforces HV1, HV6 (except the weight range), HV7 parent rule and lengths, HV8 block
count and array lengths, HV9, HV11 structure and HV13 shape as errors (§9.3). The others are
observations; a reader may warn but must not reject on them (for example a non-0x45
quantisation byte with supported codes is fine).

---

<a id="s11"></a>
## 11. Structure kinds for fuzzing (ROADMAP M4 exit criterion 5)

Each kind below must be reachable from at least one fuzz target, and each target must be able
to drive the decoder into every failure category of its kinds.

| Kind | What the fuzzer must exercise | Suggested target |
|---|---|---|
| K1 container header | magic, byte-swapped magic, file-info tag and version | `havok_tagfile` |
| K2 varint and string pool | over-long varints, negative and huge counts, pool references out of range | `havok_tagfile` |
| K3 type table (metadata) | class and parent indices, member type codes and flags, tuple sizes, class names resolved late, deep inheritance | `havok_tagfile` |
| K4 object stream and references | class index, bitmaps, forward, null and dangling references, unknown tags | `havok_tagfile` |
| K5 struct arrays | column bitmaps, int and vec4 prefixes in columns, unsupported array columns | `havok_tagfile` |
| K6 skeleton | length mismatches, bad parent indices, missing members, non-finite pose | `havok_skeleton` (tagfile bytes in) |
| K7 0x05 envelope | flag, both length fields, VLE weight count forms, leftover bytes | `havok_clip_record` (record bytes in) |
| K8 spline object fields and block table | counts, offsets, durations (0, negative, NaN), *B* vs arrays | `havok_clip_record` |
| K9 spline block and channels | masks, quantisation codes, degrees, knot vectors, overruns, block length | `havok_spline_block` (synthesised header numbers + raw block bytes) |
| K10 curve evaluation and sampling | arbitrary *t* (NaN, ±∞, huge), zero-length knot spans, sign flips | `havok_clip_record` (samples every decoded clip at fuzzer-chosen times) |
| K11 interleaved clip | shapes, empty arrays, track count 0 | `havok_clip_record` |
| K12 annotation tracks | names, absent names, annotations present (unseen form) | `havok_clip_record` |
| K13 reference frame | vec4 component counts, sample counts, leading vs embedded | `havok_clip_record` |

Seed corpora are synthetic (§12), never extracted records.

---

<a id="s12"></a>
## 12. Synthetic worked examples (for tests)

All values below were invented for this spec. Tests build them with their own writer.

<a id="s12-1"></a>
### 12.1 Minimal skeleton tagfile (264 bytes)

Three bones; a reduced class set is enough, because the reader looks members up by name.

| Item | Value |
|---|---|
| Classes | 1 = `hkaSkeleton` (version 3, no parent) with members `parentIndices` (18), `bones` (25, class `hkaBone`), `referencePose` (22); 2 = `hkaBone` (version 0, no parent) with `name` (10), `lockTranslation` (1) |
| Object 1 | class 1, bitmap `07` |
| `parentIndices` | −1, 0, 1 (count 3, prefix 4) |
| `bones` | count 3, column bitmap `01`, names "b0", "b1", "b2" |
| `referencePose` | bone 0: T (0, 0, 0), R (0, 0, 0, 1), S (1, 1, 1); bone 1: T (0, 0, 1), R identity, S 1; bone 2: T (0, 0, 1), R (0, 0, √½, √½) (90° about z), S 1 |

Byte stream (hex), annotated:

| Bytes | Meaning |
|---|---|
| `1E 0D B0 CA CE FA 11 D0` | magic |
| `02 06` | tag 1, version 3 |
| `04 16` "hkaSkeleton" `06 00 06` | tag 2; new string (11 bytes, pool 2); class version 3; parent 0; 3 members |
| `1A` "parentIndices" `24` | member: new string (pool 3), type 18 |
| `0A` "bones" `32 0E` "hkaBone" | member: new string (pool 4), type 25, class name new string (pool 5) |
| `1A` "referencePose" `2C` | member: new string (pool 6), type 22 |
| `04 0B 00 00 04` | tag 2; string reference −5 → pool 5 ("hkaBone"); version 0; parent 0; 2 members |
| `08` "name" `14` | member (pool 7), type 10 |
| `1E` "lockTranslation" `02` | member (pool 8), type 1 |
| `08 02 07` | tag 4; class 1; bitmap: members 0, 1, 2 present |
| `06 08 03 00 02` | `parentIndices`: count 3, prefix 4, values −1, 0, 1 |
| `06 01 04` "b0" `04` "b1" `04` "b2" | `bones`: count 3, column bitmap (name only), three new strings |
| `06` + 3 × 48 bytes | `referencePose`: count 3, then 36 f32 (bone 2's rotation z and w are `F3 04 35 3F`) |
| `0E` | tag 7 |

<a id="s12-2"></a>
### 12.2 40-bit quaternions

| Quaternion (x, y, z, w) | Bytes | Fields *a₀*, *a₁*, *a₂* | *i* | *s* | Decoded |
|---|---|---|---|---|---|
| (0, 0, 0, 1) | `FF F7 7F FF 37` | 2047, 2047, 2047 | 3 | 0 | (0, 0, 0, 1) |
| (0, 0, √½, √½) | `FF F7 7F FE 2F` | 2047, 2047, 4094 | 2 | 0 | (0, 0, 0.707107, 0.707107) |
| (0.2, −0.4, 0.1, −0.888819) | `42 9A 37 20 79` | 2626, 889, 2336 | 3 | 1 | (0.200007, −0.400014, 0.099831, −0.888830) |
| (0.1, −0.9, 0.3, 0.3) | `20 39 B6 63 5B` | 2336, 2915, 2915 | 1 | 1 | (0.099831, −0.900127, 0.299838, 0.299838) |

Encoding (for test writers): drop the component of largest magnitude; **on equal magnitudes,
drop the one with the lowest index** (x before y before z before w). Store its index and sign
(*s* = 1 when it is negative), and quantise each remaining component *c*, in ascending index
order, to round(*c* × 2047 × √2) + 2047. In row 2, z and w have equal magnitude, so z
(*i* = 2) is dropped and w is stored in *a₂*; that gives byte 4 = 0x2F. Dropping w instead
(*i* = 3, z stored in *a₂*) would give `FF F7 7F FE 3F`, which decodes to the same quaternion:
a decoder must accept both (§5.6 reconstructs any *i*), but tests that compare encoder output
byte by byte expect the tie-break above. The data of this install never shows which
tie-break the original encoder used.

<a id="s12-3"></a>
### 12.3 Spline clip: one track, one block

| Field | Value |
|---|---|
| `duration` | 4/30 s |
| `numberOfTransformTracks` | 1 |
| `numFrames` *N* | 5 |
| `numBlocks` *B* | 1 |
| `maxFramesPerBlock` *F* | 256 |
| `maskAndQuantizationSize` | 4 |
| `frameDuration` | 1/30 |
| `blockDuration`, `blockInverseDuration` | 255/30, 30/255 |
| `blockOffsets`, `floatBlockOffsets` | [0], [48] |
| `data` (48 bytes) | `45 21 F0 00` `01 00 01 00 00 04 04` `00` `00 00 00 3F` `00 00 00 00` `00 00 00 40` `00 00 FF FF` `01 00 01 00 00 04 04` `FF F7 7F FF 37` `FF F7 7F FE 2F` `00 00 00` |

Reading the data: mask (quantisation 0x45; position x static, y dynamic; rotation dynamic;
scale default); position spline *n* = 1, *p* = 1, knots 0 0 4 4, one pad byte; x static 0.5;
y range 0.0 … 2.0; control points y = 0 and 65535; rotation curve *n* = 1, *p* = 1, knots
0 0 4 4, quaternions identity and 90° about z; three pad bytes. Scale is (1, 1, 1).

Expected samples:

| *t* | Frame *u* | Translation | Rotation (x, y, z, w) |
|---|---|---|---|
| 0 | 0 | (0.5, 0, 0) | (0, 0, 0, 1) |
| 1/30 | 1 | (0.5, 0.5, 0) | (0, 0, 0.187366, 0.982290) |
| 2/30 | 2 | (0.5, 1.0, 0) | (0, 0, 0.382683, 0.923880) (45° about z) |
| 4/30, or any *t* > 4/30 | 4 | (0.5, 2.0, 0) | (0, 0, 0.707107, 0.707107) |

<a id="s12-4"></a>
### 12.4 Curve evaluation (scalar control points)

| Degree | Knots | Control points | Values at *u* = 0, 1, 2, 3, 4 |
|---|---|---|---|
| 1 | 0 0 4 4 | 0, 2 | 0, 0.5, 1, 1.5, 2 |
| 3 | 0 0 0 0 4 4 4 4 | 0, 1, 1, 0 | 0, 0.5625, 0.75, 0.5625, 0 |
| 3 | 0 0 0 0 2 4 4 4 4 | 0, 1, 2, 3, 4 | 0, 1.1875, 2.0, 2.8125, 4.0 |
| 3 | 0 0 0 0 2 2 2 4 4 4 4 (zero-length spans) | 0, 1, 2, 3, 4, 5, 6 | 0, 1.5, 3.0, 4.5, 6.0 |

<a id="s12-5"></a>
### 12.5 Block selection

*N* = 300, *F* = 256, `frameDuration` = 1/60, so *B* = 2, `blockDuration` = 4.25 s; block 0
holds frames 0–255 (*F₀* = 256), block 1 frames 255–299 (*F₁* = 45).

| *t* (s) | Block | Local frame *u* |
|---|---|---|
| 0 | 0 | 0 |
| 4.0 | 0 | 240 |
| 4.25 | 1 | 0 (same pose as block 0, *u* = 255) |
| 4.3 | 1 | 3 |
| 299/60 (= duration) or later | 1 | 44 |

<a id="s12-6"></a>
### 12.6 Interleaved clip

`numberOfTransformTracks` = 2, `duration` = 1.0, `transforms` has 6 elements, so *N* = 3 and
Δ = 0.5 s. Element order: (frame 0, track 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1). Track 0
translations (0, 0, 0), (1, 0, 0), (2, 0, 0) at frames 0, 1, 2: sampling at *t* = 0.25 gives
(0.5, 0, 0); at *t* = 1.0 or later (2, 0, 0).

<a id="s12-7"></a>
### 12.7 Annotation tracks and reference frame

- Annotation tracks for a 2-track clip: count 2, column bitmap `01`, names "t0", "t1"; the
  decoder yields two named tracks with no events. A variant with bitmap `03` and an
  `annotations` column is an unsupported array column (§3.9) and must be rejected without a
  panic.
- Reference frame: `up` (0, 0, 1, 0), `forward` (1, 0, 0, 0), `duration` 1.0, samples
  (component count 4) (0, 0, 0, 0), (0.5, 0, 0, 0.1), (1, 0, 0, 0.2); at *t* = 0.25 the
  interpolated sample is (0.25, 0, 0, 0.05). The same with component count 3 drops the w
  values (they read as 0).

<a id="s12-8"></a>
### 12.8 0x05 envelope

`duration` 1.0, `track_count` 2, no reference frame, a clip tagfile of *t* bytes, weights
1.0 and 0.5:

`00 00 80 3F` `02 00 00 00` `00` *t* as u32 LE, *t* tagfile bytes, `02` `00 00 80 3F`
`00 00 00 3F`.

With no weights the record ends with a single `00`. With a reference frame, byte 8 is `01` and
a u32 length plus the reference-frame tagfile precede the clip tagfile's length.

---

<a id="s13"></a>
## 13. Corrections to earlier notes

| Earlier statement | Where | Finding |
|---|---|---|
| 0x04 records have the packfile magic `57 E0 E0 57` at offset 8 | DS §1 | Offset 8 holds the **tagfile** magic in all 28,555; no packfile exists in any scanned archive. |
| 0x05 layout "`f32 duration; u32 numTransformTracks; u8 0; u32 tagfileSize + 1; tagfile`" | DS §8, MV | The u8 is a flag for a leading reference-frame tagfile (1 in 110 records). The u32 is the exact tagfile length; the apparent "+1" is the VLE weight count that follows the tagfile (0 in 18,460 records; 6,935 records have a full per-track weight array). |
| "A handful of non-animation records with a distinct type marker" | MV | These are the 110 clips with a leading reference-frame tagfile; each still contains an animation. |
| Clips expose "per-track annotation data"; "annotation tracks carry bone names" | MV, DS §8 | Only 5,602 clips have annotation tracks; they hold names only, and no clip has any timed annotation. |
| Skeleton fields include `boneIndex` and `autobone` | MV | `autobone` is not declared anywhere; `boneIndex` belongs to a declared but never instantiated local-frame class. |
| "Decoded scales are uniformly 1.0" | MV | 9,613 clip channels and 224 reference-pose bones have other scales. |
| Records begin with their own DID | `records.md` P10 | Not 0x05 records (§2.3). |
| About 24,800 spline and 600 interleaved clips | DS, MV | Exactly 24,800 and 595. |

---

<a id="s14"></a>
## 14. Open questions

> **Resolved (2026-10-05):** meaning of the envelope `track_weights`: per-track opacities of
> layered playback (§2.3; `skeleton-anim.md` §6.6).

> **Resolved in part (2026-10-05, BIN):** the clip player uses the tagfile `duration` (§2.3).
> **OPEN:** whether anything else reads the envelope value (21 records differ, §8.4).
> **Resolved (2026-10-07, BIN, H22):** the start crop is always 0; the end crop is 1/30 s when
> the clip is started for exactly one cycle, else 0 (§2.3).

> **Resolved (2026-10-05, `skeleton-anim.md` §3):** binding is positional by track count for
> every clip, named or not (§4.4, H16).

> **Resolved (2026-10-05):** the reference-frame *w* is a yaw angle about `up`
> (`skeleton-anim.md` §7.3); short sample arrays are stretched over the duration by their own
> sample count (§4.5, §7.8, BIN).

> **OPEN:** whether the original aligns quaternion signs before blending spline control
> points (§7.6). The client's spline decompressor is vectorised and was not traced to a
> conclusion in this session; the interleaved sampler does align signs (§7.8). Keep the
> recommendation of §7.6.

> **Resolved (2026-10-05, BIN):** interpolation between interleaved frames: linear for
> translation and scale, sign-aligned normalised linear for rotation (§7.8).

None of these blocks decoding; they affect playback choices in `tessera-anim`.

Raised by the clean-room implementation (`tessera-havok`, 2026-10-05). The real-data run
decoded 28,555 of 28,555 skeleton records and 25,395 of 25,395 clip records, every
failure category at 0, and reproduced §5.5, §8.3 and §8.4 exactly. Answers of the spec
writer, 2026-10-05 (the implementation's choices are confirmed unless stated otherwise):

> **Answered:** roots per skeleton are 1 (23,365), 2 (5,183), 3 (6) and 4 (1), as the
> implementation counted; §4.2 and §8.2 are corrected (CNS2).
> Original note: the spec said "3 roots (7)" and no 4.

> **Answered:** "224" and "53,146" use a tolerance of 1e-3 on the largest component
> difference from 1; 798 bones are not bit-exactly (1, 1, 1) (§4.2, CNS2).

> **Answered:** a negative count is `tagfile.bad_varint`; a count larger than the bytes left is
> `tagfile.truncated`, also for a struct array with no present columns. Both are now part of
> §9.3's reading.

> **Answered:** resource limits (more than 4,096 classes, a full member list longer than
> 1,024, inheritance deeper than 64, more than 65,536 objects; observed 8, 21, 3, 2) are
> `tagfile.unsupported_form`. The limits are Tessera's own hostile-input bounds.

> **Answered:** a vec4 component count outside 1–4, object arrays (type 24) and object columns
> are `tagfile.unsupported_form`; a struct-array element class with no declared name is
> `tagfile.bad_class_index`. None occurs.

> **Answered:** only rotation code 1 (40-bit) and vector codes 0 and 1 are supported; every
> other code is `spline.unsupported_quantisation`, checked only when the channel stores data.
> None of the other codes occurs; their layouts stay unspecified here.

> **Answered:** the pose checks of §9.1, `extractedMotion` pointing at another class, a spline
> clip with 0 tracks, a negative track count, `maxFramesPerBlock` < 2, `numFrames` < 1 and a
> present non-zero `endian` are `object.inconsistent`.

> **Answered:** a 40-bit value whose three stored components square-sum above 1 yields a dropped
> component of 0 and is not renormalised by the decoder (it never occurs; HV12). Consumers that
> need unit rotations normalise, as curve evaluation already does.

> **Answered:** every non-finite *t* (NaN, +∞, −∞) samples *t* = 0, as §7.4 says. (The
> client's interleaved sampler has quirks for negative *t*, §7.8; nothing depends on them.)

> **Answered:** an annotation-track count that differs from `numberOfTransformTracks` is
> accepted: the first `numberOfTransformTracks` names belong to the tracks, a shorter array is
> kept whole (never observed).

> **Answered:** `maxFramesPerBlock` > 256 is accepted as the implementation does (never
> observed); such a block can only hold constant channels.

> **Answered:** the order of envelope checks is the implementation's choice; each failed record
> still reports exactly one category.

Raised by the clean-room implementation after an audit (`tessera-havok`, 2026-10-06). These
are hostile-input choices of the implementation; none changes a result on real data.

> **Answered (2026-10-07, spec writer):** confirmed. The limit is Tessera's own hostile-input
> bound, like the other resource limits; it is now part of the §9.3 assignments. Reading and
> checking the columns of a struct array with count 0 without keeping them is correct.
> Original note: Tessera now caps the decoded values of one tagfile (present object members plus
> stored struct-array columns) at 1,048,576 (fewer than 100 occur) and reports more as
> `tagfile.unsupported_form`, like its other resource limits. A present member can cost one
> bit of input, so without a cap a small record decoded to a model hundreds of times its size.
> The columns of a struct array with count 0 are still read and checked (prefixes included)
> but not kept, since each equals the §3.10 default. Please confirm or state other limits.

> **Answered (2026-10-07, spec writer):** yes. The data have degrees 1 and 3 only, and no
> larger degree was traced in the client. §7.3 and the §9.3 `bad_curve` row now state the
> Tessera bound: a degree above 7 is `spline.bad_curve`, checked before any control point is
> read.
> Original note: Tessera rejects a curve degree above 7 as `spline.bad_curve` (§5.4 and HV11 observe
> only 1 and 3). §7.3 and the §9.3 `bad_curve` row reject only degrees outside 1 … *n*, which
> allows up to 255; evaluation costs O(*p*²) per channel, so a degree-255 clip made one sample
> thousands of times slower. Should §5.4, §7.3 and §9.3 state a degree limit, and which?

> **Answered (2026-10-07, spec writer):** an interior knot repeated more than *p* times is
> `spline.bad_curve` (HV11 structure); the row now lists it. Rejecting it is right.
> Original note: the §9.3 `bad_curve` row does not list "an interior knot repeated more than *p*
> times", though §5.4 and HV11 require multiplicity ≤ *p* and §10 enforces "HV11 structure".
> The implementation now rejects it as `spline.bad_curve` (previously it was accepted and the
> curve jumped at that knot). Please add it to the row, or say if it should be accepted.

> **Answered (2026-10-07, spec writer):** acceptable. (1 − α) × *a* + α × *b* is the normative
> form for Tessera from now on (§7.8); it equals *a* + α × (*b* − *a*) in exact arithmetic and
> on every real clip within float rounding. Sampling each interleaved frame through the
> playback sampler in the §9.2 check is also right.
> Original note: §7.8 words the interleaved and reference-frame blend as *a* + α × (*b* − *a*).
> For finite frames of opposite sign near the `f32` limit, *b* − *a* overflows and the sample
> becomes NaN or infinite although both frames are finite. Tessera evaluates the same blend as
> (1 − α) × *a* + α × *b* (equal in exact arithmetic, exact at α = 0 and 1, never overflows
> for finite frames). The §9.2 finiteness check also samples each interleaved frame at its
> own time through the playback sampler, not only as stored. Please confirm that this form is
> acceptable, or state the required arithmetic.

> **Answered (2026-10-07, spec writer):** confirmed. Keep the `f32` sum, and only when a
> component is not finite recompute it in `f64` and limit it to the range of the window's
> control points. §9.2 keeps checking whole frames only; it does not sample between frames
> and does not reject such ranges (none occur, §7.7).
> Original note: §7.5 step 3 sums *N* × *P* in floating point. The `f32` basis functions can sum
> a hair above 1 between knots, so control points that are all finite but near the `f32`
> limit sum to infinity between frames, while the §9.2 check (whole frames only) passes.
> Tessera keeps the `f32` sum and, only when it is not finite, recomputes that component in
> `f64` and limits it to the range of the window's control points (the blend is a convex
> combination), so finite control points always give a finite value. Please confirm, or state
> whether §9.2 should also sample between frames or reject such ranges.

> **Answered (2026-10-07, spec writer):** confirmed, including the case of a `duration` that
> is 0, negative or not finite (every *t* gives the last frame) and the rotation at *t* ≥ *D*
> (the last frame's own quaternion, normalised; this is what the client's step 1 returns).
> Original note: §7.8 step 1 and §4.5 step 1 (*t* ≥ *D*: the last frame exactly). Tessera tests
> *t* ≥ *D* after the §7.4 clamp, so a `duration` that is 0, negative or not finite (never
> observed) clamps every *t* to 0 and gives the **last** frame; a non-finite *t* still reads
> as 0 (§7.4) and gives the first frame when *D* > 0. At *t* ≥ *D* the rotation is the last
> frame's own quaternion, normalised, not the sign-aligned blend (which can be its negation).
> Please confirm.

---

<a id="s15"></a>
## 15. Corrections to other specs

Recorded here instead of editing specs owned by other writers.

| Spec | Statement | Correction |
|---|---|---|
| `skeleton-anim.md` §7.3 | "When the reference frame has fewer samples than the clip has frames (3 cases), hold its last sample." | The middleware's own sampler (BIN, §4.5 and §7.8 here) spreads the samples over the whole `duration` by their own count; a short array is stretched, not held. Its implementation OPEN on extracted motion over loops uses `sample_on_grid`, which therefore differs from the client for those 3 objects. |
| `skeleton-anim.md` §6.1 | Duration mismatch "OPEN there" (`havok-data.md` §8.4) | The clip player uses the tagfile `duration` (§2.3 here, BIN). |
| `docs/ARCHITECTURE.md` §11.2 | "Tracks map to bones by position" (Reported) | Confirmed (`skeleton-anim.md` §3; H16 here). |

---

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-04 | all | First complete version (Review). | Spec writer (dirty room) |
| 2026-10-05 | §1 (SA, BIN, CNS2; legend; H16 Confirmed, H17–H21), §2.3, §4.2, §4.4, §4.5, §7.8 rewritten, §8.2, §9.3, §14, new §15, §16 | Answers from `skeleton-anim.md` folded in (binding, track weights, meaning of *w*); M4 (decision 0011): the clip player's duration, interleaved interpolation and reference-frame sampling; roots and scale tolerance recounted; every implementation OPEN of §14 answered. | Spec writer (dirty room) |
| 2026-10-05 | header, §2.3, §2.4, §12.2, §14, §16 | Review changes: §12.2 row 2 *i* corrected from 3 to 2 (its bytes `…2F` were right); tie-break for the test encoding (on equal magnitudes drop the lowest index); the Havok-version remark cites DS (`research/notes/data-semantics.md` §8) instead of the undefined tag ARCH; OPEN callout with Tessera default 0 for the clip player's crop amounts. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN second session, H22), §2.3, §7.3, §7.6, §7.8, §9.3, §14, §16 | M4 (decision 0011), questions 4–5: the crop amounts are answered (start crop always 0; end crop 1/30 s for clips started for exactly one cycle; OPEN closed); spline sign alignment re-read, still OPEN. Answers to the six implementer notes of 2026-10-06: the 1,048,576-value cap and a degree bound of 7 are Tessera bounds; interior knot multiplicity above *p* is `bad_curve`; (1 − α) × *a* + α × *b* is the normative blend form; the `f64` fallback and the *t* ≥ *D* rule are confirmed. Status Accepted → Revised. | Spec writer (dirty room) |
