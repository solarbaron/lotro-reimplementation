<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Environment: regions, sky, time of day, fog and weather (water: `water.md`)

| | |
|---|---|
| **Status** | Accepted (2026-10-08 revision, reviewed and passed; previously Accepted 2026-10-07 after the BIN-E re-review, and 2026-10-05). The 2026-10-08 revision changes no section number or anchor; it corrects one census figure (§5.3), states how the switch between the outdoor and indoor tracks reaches the frame (§11.6, new E37), folds in the two non-blocking notes of the 2026-10-07 re-review (§11.6 sun-change fade, §5.4 field 22) and adds rows E38–E39. Implementers re-check code against the rows the change log names. |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent), 2026-10-05; separate Claude review sessions, 2026-10-07 (BIN-E revision: review, then re-review) and 2026-10-08 (2026-10-08 revision) |
| **Review record** | Review 2026-10-08 of the 2026-10-08 revision (separate review session, spec-reviewer role): **passed**, Accepted. Re-review 2026-10-07 of the BIN-E revision (separate review session): **passed**, Accepted. Full records in the dirty-room review log (decision 0018). |
| **Implements into** | `crates/formats` (region descriptor, sky description 0x15, day description 0x1A, game time 0x35), `crates/world` (per-landblock sky key, game clock, day-description selection, keyframe blending, sky objects), `crates/render` (sky objects, fog, sun and ambient inputs of [`lighting.md`](lighting.md)) |
| **Companion specs** | `records.md` §2.1 (prefix → DB type table), §7 (enum mappers). `property-system.md` §8 (DID maps), §9 (DB-object-type codes). [`landblock.md`](landblock.md) (block maps, which carry the per-landblock sky key). `entity-descriptor.md` (static entities: lights, time-of-day overrides; 0x47 objects). [`lighting.md`](lighting.md) (how the values below light a surface). `material.md` §5.3 (float curves, shared with §5.3 here). The per-region tables named by the region descriptor are decoded in [`scenery.md`](scenery.md) (0x11, 0x12, 0x02), [`terrain-texturing.md`](terrain-texturing.md) (0x19) and [`water.md`](water.md) (0x16 water, 0x17 fog zones; all water). `coordinates.md` (map coordinates, §3.1 here). |

---

<a id="s0"></a>
## 0. Purpose and status

This spec lets an implementer:

- decode a region's **region descriptor** to the last byte and find its per-region tables
  (§3), and know which spec decodes each of them (§4);
- decode the shared **sky description** 0x15000007 (§5.1) and the 963 **day descriptions**
  (prefix 0x1A, §5.2–§5.5) to the last byte;
- run the **game clock** from the game-time record 0x35000000 (§6, §11.1);
- choose the day description for a landblock's sky key and the current game day (§11.2),
  blend its **time-of-day keyframes** (§11.3) into sun, ambient and fog values for
  [`lighting.md`](lighting.md), and animate the **sky objects** (sun, moon, clouds, domes)
  (§11.4, §12);
- know which properties select skies, interior fog and time-of-day behaviour (§7). Water and
  the region's fog zones are specified in [`water.md`](water.md) (§8 here is a pointer);
- run the client's **day-type chain** (§11.5), the **sky-key cross-fade** and the separate
  indoor and outdoor environment tracks (§11.6), and compose the final fog, ambient and
  precipitation values with the interior overrides and limits (§11.7) at the client's update
  cadence (§11.8).

Values quoted from the install are observations; implementations read them at run time.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 by throwaway probes in the dirty-room scratchpad: every record of prefixes 0x0F, 0x11, 0x12, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x32, 0x35 read; DID-like values located by scanning; block-map descriptors 0x18 fully decoded; property sets of all landblock-info records, cells, SEDs, 0x804 landblock properties and all 0x78/0x79 property sets scanned for the properties of §7. |
| OBS2 | Observed in user install 2026-10-05: 0x15000007 parsed to the last byte with the grammar of §5.1; all 963 day descriptions (0x1A) parsed to the last byte with §5.2–§5.4 (7,590 keyframes, 12,788 sky objects, 107,213 keyframe sky-object entries); 0x35000000 parsed to the last byte (§6); censuses of every field. Enum mappers dumped (labels quoted below come from them). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`lotroclient64.exe`, `lotro_live`), disassembled with a general-purpose disassembler around: the deserialisers of DB object types 0x3F (sky description), 0x3A (day description, its keyframes and sky objects) and 0x02 (game time); the keyframe and sky-object blending; the day-description selection; the game clock. **Question:** field boundaries that the data leaves ambiguous (variable-length parts inside keyframes and sky objects), the type of each record, and how keyframes are combined over time. **Why M1–M3 were not enough:** the keyframe tail and sky objects contain optional and variable parts whose boundaries cannot be found reliably from zero-heavy data; the blending, angle interpolation and selection rules exist only as code; no running client is available to this session for M3. Facts resting on BIN alone are marked **BIN**. |
| PS | `property-system.md` §8–§9 (DID maps; DB-object-type mapper "DBOType"). |
| DS | `research/notes/data-semantics.md` §6.5 (block maps), §13 (environment, sky, weather and water listed as unknown; no public decoder). |
| ER | `research/notes/engine-client-re.md` §3 (renderer settings), §4 (map coordinates). |
| AC | Asheron's Call lineage (published AC format descriptions in the research clones) used only as a naming analogy for "sky description", "day group", "sky object", "time-of-day keyframe". No AC layout is assumed: every layout below was checked against this install. |
| BIN-R | **M4 under decision 0011**, second pass on 2026-10-05, same client build (4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5). **Questions:** (a) which DB object type the client registers for each DID range (to replace the content-based kind hypotheses of §4 and §9); (b) the region descriptor's deserialiser and the order of its DID maps, and which code asks which map; (c) how the client turns a position into map coordinates with the region's fields; (d) the exact comparisons of the keyframe search of §11.3 at a keyframe's own time. **Why M1–M3 were not enough:** the region fields at offsets 8–32 and the empty maps cannot be told apart from content; the record kinds are named nowhere in the data except through these registrations; the boundary behaviour of §11.3 is code only; no running client is available for M3. |
| OBS3 | Observed in user install 2026-10-05: all 7 region descriptors parsed with §3.1 (zero leftover bytes); the offset-8 DIDs resolved; 0x32000000 parsed (§9); the record-prologue census of `records.md` §1.6. |
| OBS4 | Observed in user install 2026-10-06 (re-check for E16): all **11** prefix-0x08 records (only in `client_general.dat`, 1,007 bytes together) read with the header of [`walker.md`](walker.md) §11.5.1; each record's size is the one §11.5.3 gives for its controller type (161, 89, 36, 94 bytes for types 1, 2, 3, 4: 4, 1, 5 and 1 records); the first u32 is 0x47000002 or 0x47001A7C; near clip 0.1 and far clip 2,000 in all 11. |
| LC | LotroCompanion `lotro-tools` (readable source, its region loader; facts only, no code, comments or identifiers taken): reads region-descriptor offset 8 as an image DID and takes slots 1–6 of §3.1 as the scene, terrain, encounter, sky, water and fog tables, in that order. Reported only; the layout here rests on BIN-R and OBS3. |
| BIN-E | **M4 under decision 0011**, third pass on 2026-10-07, same client build 4906.0071.1794.4035 (`lotroclient64.exe`). **Questions:** (a) where the game clock's seconds come from, how the rate and the time offset act, and what decides its epoch (owner item O21); (b) how the current day type is set (O26; the former §10 OPEN); (c) which sky key wins among the player, cell, landblock and block-map sources, and whether the indoor ambience replaces it (the §7.1 OPEN); (d) what `InteriorFogType` values do and how the interior properties override the keyframe fog and ambience (the §12 OPEN, O26); (e) the axes of the sky-object angles, the keyframe-entry rules and the tint (the §11.4 and §12 OPENs); (f) when and how the client cross-fades between skies (the §11.2 step 5 and §11.4 OPENs); (g) the uses of the keyframe fields left OPEN in §5.3; (h) the game-time fields left OPEN in §6, and the clock's defaults when no game-time record is loaded; (i) whether the client checks non-finite floats and how it bounds counts (the §13 OPENs); (j) the per-type version branches of the keyframe and sky-object readers (for readers of other builds; every record of this install has the newest form). **Why M1–M3 were not enough:** all ten are run-time behaviour or reader code paths with no trace in the data (the data hold the inputs only; for example the day type and the epoch are never stored); no published source describes them; the owner's observations (M3, O21 and O26) are pending and could sample the outcome on a few days but not establish the rules. **Scope kept:** the game-time module, the sky-description and day-description classes, the environment update, the sky-object update, the environment-state composition and cross-fade, the property lookups they call, the frame timer's read path, and the property registry labels in the install. The frame timer has a re-base operation; one caller of the timer was opened, found to belong to the connection layer, and not analysed further (decision 0004). No network, login, launcher or anti-tamper code was read. Facts resting on this pass alone are marked **BIN**. **Second revision, 2026-10-07 (after review):** the pass's listings of the day-change routine, the sky evaluation and the sky-object update were re-read for the review's three findings (which sky description the day type and the pick read; the day type when it is missing; a replacement on an invisible sky object), and the sky-object update was disassembled once more, same build, to check which of its stores truncate to integers (only fields 13 and 14 of §5.4; the angles stay floats). |
| OBS5 | Observed in user install 2026-10-05: a scan of the property sets of every cell record (104,271), kept in the dirty scratch area, counted for the three `Interior…AmbientLight` properties on 2026-10-07 (it agrees with the §7.2 counts of `InteriorFogType` and `InteriorFogNear`, 775 each). |
| PR | Property registry of the install (`property-system.md`), labels read 2026-10-07 for the property ids that the BIN-E code uses: 0x1EF `World_TimeOffset` (float), 0x1F0 `World_IsNight` (bool), 0x235 `World_DayType` (enum mapper 0x230000E8), 0x682 `World_PresentTimeOfDay` (float), 0x6CC `World_TimeDilation` (float), 0x873 `World_RainIntensity`, 0x874 `World_SnowIntensity` (float), 0x18D `Environment_PlayerIndoorAmbianceOverride`, 0x1D8 `Environment_LocalSky`, 0x53D `Environment_PlayerLocalSkyOverride`, 0x5DC `Environment_IndoorAmbiance`, 0xB73 `Environment_LocalSky_HousingHook` (enum mapper 0x230000E9), 0x276 `InteriorFogNear`, 0x3FA `InteriorFogFar`, 0x3A4 `InteriorFogColor`, 0x623 `InteriorFogType`, 0x31B `InteriorAmbientLight`, 0x9D9 `InteriorUpperAmbientLight`, 0x9DA `InteriorLowerAmbientLight` (colours), 0x243 `Render_Color`, 0x63F `Render_Opacity`. Names are quoted for interoperability (an implementation finds the properties by name in the registry at run time). |
| BIN-E2 | **M4 under decision 0011**, fourth pass on 2026-10-08, same client build 4906.0071.1794.4035 (`lotroclient64.exe`), own copy of the Ghidra project (decompiler and listings in the dirty scratch area). **Questions:** (a) what reaches the frame when the client changes from the outdoor to the indoor track or back: whether that switch has a blend of its own; (b) the duration rule of the sun-change fade of §11.6 when a change arrives during a running fade (non-blocking note (a) of the 2026-10-07 re-review). **Why M1–M3 were not enough:** both are run-time behaviour of the environment update with no trace in the data; no running client is available for M3; nothing is published. **Scope kept:** the environment update that runs both tracks and picks one, the routine that applies the picked state with the sun-change fade, and the blend routine they share with the sky-key cross-fade. No network, login, launcher or anti-tamper code was read. |
| OBS6 | Observed in user install 2026-10-08: all 963 day descriptions (0x1A) re-read from `client_general.dat` (byte-identical to the 2026-10-05 dump), parsed to the last byte with §5.2–§5.4, census of keyframe field 6 over all 7,590 keyframes. |

| Mark | Meaning |
|---|---|
| Confirmed | Checked over all records of the kind |
| Observed | Seen in the records named; layout not decoded end to end |
| BIN | Rests on disassembly of the client (decision 0011) |
| Inferred | A reading that fits the observations but is not proven |
| Tessera Decision | Tessera's own rule where the original's is not known; never a statement about the original |

| # | Fact | Sources | Mark |
|---|---|---|---|
| E1 | 0x15 is DB type 0x3F (sky description), 0x1A is type 0x3A (day description), 0x35 is type 0x02 (game time) (§2) | BIN (the loader requests these type codes for these DIDs) | BIN |
| E2 | Sky description layout (§5.1) | OBS2 (1 of 1 record, zero bytes left); BIN (field order, the two optional forms) | Confirmed |
| E3 | Day description layout: header, keyframes, sky objects, transition time (§5.2–§5.4) | OBS2 (963 of 963, zero bytes left); BIN (boundaries of optional parts) | Confirmed |
| E4 | Keyframe field meanings: time, fog, directional light, ambient (§5.3) | BIN (blending code routes them to the fog and light state; sun direction built from heading and pitch) | BIN |
| E5 | Colour encoding u32 0xAARRGGBB, stored little-endian (bytes B, G, R, A) (§5.6) | BIN | BIN |
| E6 | Game-time layout and clock (§6, §11.1) | OBS2; BIN | Layout Confirmed; clock BIN |
| E7 | Day-description choice: weighted pick seeded by the game day number (§11.2) | BIN | BIN |
| E8 | Keyframe blending and angle interpolation (§11.3) | BIN | BIN |
| E9 | Sky-object animation (§11.4) | BIN; OBS2 | BIN |
| E10 | Day types (enum mapper 0x230000E8) | OBS2 (labels) | Confirmed labels |
| E11 | Region descriptor layout: fixed fields, nine DID maps, trailing terrain-type-table DID (§3.1) | BIN-R (b); OBS3 (7 of 7, zero leftover bytes) | Confirmed |
| E12 | Kind of every per-region table and of prefixes 0x02, 0x08, 0x2D, 0x32, 0x39, 0x44 (§2, §4, §9) | BIN-R (a); labels from the DBOType mapper (OBS) | Confirmed (BIN registration; content agrees) |
| E13 | Region offset 8 is the DID of a 0x41 map image; offsets 16–32 are the map-coordinate origin (in 20-unit steps), two map offsets and the map scale (§3.1) | BIN-R (b), (c); OBS3 (both offset-8 DIDs exist: 512 × 512 user-interface images); LC (names) | BIN; image DIDs Confirmed |
| E14 | The keyframe search of §11.3 uses exactly the comparisons stated there, so a time equal to an interior keyframe's time blends its two neighbours (§11.3) | BIN-R (d) | BIN |
| E15 | Prefix 0x32 is the render LOD-class table: DID, count, entries of (class key, 8 × f32) (§9) | BIN-R (a); OBS3 (1 of 1, zero leftover bytes) | Confirmed layout; field meanings OPEN |
| E16 | The camera-state layout (prefix 0x08) is the one of [`walker.md`](walker.md) §11.5; its first u32 is the camera entity's DID (a 0x47 descriptor), the three f32 that follow are the collision pull-in smoothing time, the near clip and the far clip (§9) | `walker.md` W9, W25, W-I1, W-I2 (OBS-W, BIN-W); OBS4 | Confirmed (layout owned by `walker.md`) |
| E17 | The game clock advances by the rate × the change of (frame clock + time offset); the frame clock counts seconds from the client's timer start unless re-based from outside the game-time module; the clock has no wall-clock epoch of its own; the day-change computation (start of day in f32, the "t > 1" loop), the publication of the time of day (threshold 0.001) and of the night flag; the console time-of-day setter (adds to the offset) and the rate setter (values above 0.0002 only) (§11.1, §11.1.1) | BIN-E (a); PR | BIN |
| E18 | `value_54` is the night start and `value_58` the night end (time-of-day fractions); night = *t* ≥ `value_54` or *t* < `value_58`; `unknown_last` is a name token of the date text; day of month = *d* − month `start_day` + 1 (§6, §11.1) | BIN-E (h); OBS2 (values) | BIN |
| E19 | The day type is computed by the client at every game-day change from the previous day type and the game day number, with two hashes and the groups' follow lists and chances, read from region 1's sky description; when that description is missing or fails to load, the new day type is 0 (§11.5) | BIN-E (b); OBS2 (inputs); step 1 corrected in the second revision of 2026-10-07 | BIN |
| E20 | The day-description choice skips a group whose entry gives nothing and goes on to the next group, and gives no description when the day type or the sky key is 0 (§11.2) | BIN-E (b) | BIN |
| E21 | Sky-key precedence: player housing hook, player sky override, player sky key, the position's sky key, else 2; indoor key: player indoor override, player indoor ambience, the position's indoor ambience, else 1; a value counts only with property type 8 or 10; the position lookup order (run-time object's two sets, else landblock-info set, then block map) (§7.1, §7.1.1) | BIN-E (c); PR | BIN |
| E22 | Two environment tracks (outdoor sky key, indoor ambience key); the outdoor one is used while the landscape is visible from the viewer, the indoor one otherwise, with the interior property overrides (§11.6, §11.7) | BIN-E (c), (d) | BIN |
| E23 | Interior fog: `InteriorFogType` present and not Linear (1) turns fog off; near, far and colour override only where present; `InteriorUpperAmbientLight` / `InteriorLowerAmbientLight` replace the two ambient terms and `InteriorAmbientLight` is added to both (§7.2, §11.7) | BIN-E (d); PR | BIN |
| E24 | Fog limits: outdoor far ≤ 10,000; every far ≥ 20; near ≤ far − 20 and ≥ −10,000; fog on only with the client's distance-fog setting; `fog_extra` kept only above 0.001; the weather override; the "sun present" test (threshold 0.0002) (§11.7) | BIN-E (d), (g) | BIN |
| E25 | Sky-object orientation: rotation about +Z by −B degrees, then about +Y by −A degrees; A from fields 5–6, B from fields 7–8 (§11.4) | BIN-E (e) | BIN |
| E26 | Keyframe entries: angle offsets and replacement come from keyframe *a* alone and are applied after the visibility test, so a replacement is drawn even while the object is outside its visible interval; the tint is blended only when both *a* and *b* have an entry, otherwise it keeps its last value; the tint goes to `Render_Color` (RGB, alpha 1) and its alpha × the sky's fade to `Render_Opacity`; sky-object fields 9–14, the sub-structure 16–21 and field 22 are copied to the drawn object (fields 13 and 14 truncated to integers, the sub-structure only when `layer_count` ≠ 0; field 15 `reverse` is not copied) (§5.4, §11.4) | BIN-E (e); PR; field 22 and field 15: E39 | BIN |
| E27 | A change of sky key cross-fades the old and new sky over the new day description's `transition_seconds` (else the old one's, else 8 s) with a sine-shaped weight; a change of day description under the same key does not cross-fade (§11.6) | BIN-E (f) | BIN |
| E28 | Keyframe float curve 1 is the rain intensity (clamped to [0, 1]) and curve 2 the snow intensity (clamped to [0, 2]); `fog_extra` above 0.99 with fog on hides the sky objects; `colour_3` RGB × 0.85; `value_b` clamped to [0, 1]; the defaults of `values_a` and `value_b`; `effect` taken from *k_b* within 0.0002 of *f* = 1 and, when not null, making the sun absent (§5.3, §11.3, §11.7) | BIN-E (g); PR | BIN |
| E29 | The client performs no finiteness check on keyframe or sky-object floats and bounds every count only by the bytes left in the record (§13) | BIN-E (i) | BIN |
| E30 | Update cadence: the sky is re-evaluated every light interval (midpoint, divided by the rate) or at once when its key, region or day type changes; the position's properties every 2.5 s of the raw timer or when the position's block or cell changes (§11.8) | BIN-E (c), (f) | BIN |
| E31 | Game-time defaults when no game-time record is loaded: day length 100,000 s, 1 day per year, night start 0.75, night end 0.25 (§6) | BIN-E (h) | BIN |
| E32 | Curve blend: the blended curve takes *kₐ*'s mode unless that mode is 1 or the factor exceeds 0.5, then *k_b*'s; each of the ten parameters blends linearly (§11.3 step 4a) | BIN-E (g) | BIN |
| E33 | Older per-type versions: keyframe without `sun_specular` below 3 (derived from the sun colour and brightness), without `ambient_colour_2` below 6 (a copy of colour 1), four f32 constants instead of the curves below 2, `effect` from 5, `colour_4` from 7; sky object without field 22 below 8 (§5.3, §5.4) | BIN-E (j) | BIN (no record of this install uses an older form) |
| E34 | Sun-change fade: when the "sun present" flag of the state in use changes, a 5 s fade on the raw timer with the weight shape of §11.6 step 4; a change during a running fade gets as its duration the smaller of the running fade's duration and the time since the previous change, and at least 0.0002 s; an immediate update skips it (§11.6) | BIN-E (f); duration rule corrected by BIN-E2 (b), 2026-10-08 (it was "the time since the previous change") | BIN |
| E35 | Each new sky takes the next block of 100 numbers from a global counter and registers its objects with the renderer under that number + the object's list position (§12) | BIN-E (e) | BIN (its use as a sort key Inferred) |
| E36 | Both the day-type chain and the day-description pick of every track read region 1's sky description (the region descriptor with region number 1, slot 4), whatever region the track is in; the track's own region supplies only the light and sky intervals; with no sky description for region 1 there is no day description (§11.2, §11.5, §11.8) | BIN-E (b), (f); found by the review of 2026-10-07 and re-checked in the second revision | BIN |
| E37 | The change between the outdoor and the indoor track has no blend of its own: on every update the client picks one track's state and hands it to the sun-change fade. That fade, with the blend rules of §11.6 step 4, is the only smoothing: a switch that changes "sun present" (always the case when the outdoor state has a sun, since the indoor state never has one) fades over 5 s; any other switch shows the new state in the same update, or continues a fade already running; an immediate update always switches at once (§11.6) | BIN-E2 (a) | BIN |
| E38 | Keyframe field 6 (`sun_brightness`): 0.0 in 557 keyframes and 0.0001 (f32) in exactly one, below the 0.0002 "sun present" limit of §11.7 step 9 (§5.3) | OBS6 | Confirmed (7,590 of 7,590) |
| E39 | Sky-object field 22 is copied to the drawn object; field 15 (`reverse`) is not (§5.4) | Re-review of 2026-10-07 (separate review session, its own Ghidra copy of the same build; non-blocking note (b)); not re-traced by the writer | BIN |

---

<a id="s2"></a>
## 2. Vocabulary from the data (Confirmed labels)

The DB-object-type enum mapper (labelled "DBOType" in the EMAPPER DID map; base mapper
0x230000F0 in this install, `property-system.md` §9) names the record kinds the engine
knows. The environment-related labels, with their codes:

| Code | Label | Code | Label |
|---|---|---|---|
| 0x02 | GAME_TIME | 0x31 | LAND_BLOCK_TERRAIN_LIST |
| 0x13 | ENCOUNTER_DESC | 0x35 | TERRAIN_TYPE_TABLE |
| 0x16 | FOG_DESC | 0x3A | DAY_DESC |
| 0x17 | WATER_DESC | 0x3F | SKY_DESC |
| 0x18 | FARVIEWENTITIES | 0x41 | DISTANT_LANDSCAPE_SECTOR |
| 0x1C | TERRAIN_DESC | 0x4A | SCENE_DESC |
| 0x1E | PSDESC | 0x4F | REGION |
| 0x30 | RENDER_IMPOSTER_TABLE | 0x56 | FORKPSDESC |

The code is **not** the DID prefix (for example RENDERSURFACE is code 0x05 but prefix 0x41).
The client registers one type code per DID range (BIN-R (a), E12); the complete table is in
`records.md` §2.1. The pairings that concern this spec:

| DID prefix | Type code | Label | Kind | Spec |
|---|---|---|---|---|
| 0x0F | 0x4F | REGION | Region descriptor | §3 |
| 0x11 | 0x4A | SCENE_DESC | Scene-choice table | `scenery.md` §4.3 |
| 0x12 | 0x1C | TERRAIN_DESC | Terrain-scene table | `scenery.md` §4.2 |
| 0x14 | 0x13 | ENCOUNTER_DESC | Encounter table | §4 (layout OPEN) |
| 0x15 | 0x3F | SKY_DESC | Sky description | §5.1 |
| 0x16 | 0x17 | WATER_DESC | Water table | `water.md` §4 |
| 0x17 | 0x16 | FOG_DESC | Fog-zone table | `water.md` §5 |
| 0x18 | 0x21 | PROPERTY_DESC | Block-map property descriptor | `landblock.md` §5.2 |
| 0x19 | 0x35 | TERRAIN_TYPE_TABLE | Terrain type table | `terrain-texturing.md` §3 |
| 0x1A | 0x3A | DAY_DESC | Day description | §5.2 |
| 0x35 | 0x02 | GAME_TIME | Game time | §6 |
| 0x02 | 0x3C | SCENE | Scene record | `scenery.md` §5 |
| 0x32 | 0x3B | RENDER_LOD_CLASSES | LOD-class table | §9 |
| 0x80 K 5 | 0x41 | DISTANT_LANDSCAPE_SECTOR | Distant-landscape sector | `landblock.md` §7 |

Note the crossed codes of 0x16 and 0x17: the DID prefix and the type code do not match
(`water.md` §3).

Related enum mappers (labels are data; read at run time):

| Mapper | Content |
|---|---|
| 0x230000E8 (base 0x23000185) | **Day types**, 13 values 0x10000001 – 0x1000000D. Labels name three weather words each (for example 0x10000001 "FairFairFair", 0x10000004 "FairCloudyFair", 0x10000008 "FairRainyFair", 0x1000000B "FairStormFair"). |
| 0x230000E9 (base 0x230001B4) | `Environment_LocalSky` sky keys (§7.1); base mapper value 2 is labelled "Default" |
| 0x230001E4 | Time-of-day names (§7.3) |

<a id="s3"></a>
## 3. Region descriptor (prefix 0x0F, `client_general.dat`; Confirmed, 7 of 7 records)

The REGION DID map (root key 0x11, sub-map 0x28000008, `property-system.md` §8.1) labels
eight 0x0F DIDs; seven exist (0x0F000000, 01, 02, 05, 06, 08, 09; the eighth target,
0x0F000007, does not). Every record is 114 bytes in this install; that size follows from the
grammar below (one entry in each of maps 1–7, none in maps 8 and 9) and is not a rule.

<a id="s3-1"></a>
### 3.1 Layout (Confirmed: BIN-R (b) and OBS3, 7 of 7, zero leftover bytes)

All fields little-endian. "Offset" is the offset in every record of this install.

| Order | Offset | Size | Type | Field | Observed | Meaning |
|---|---|---|---|---|---|---|
| 1 | 0 | 4 | DID | `did` | own DID | |
| 2 | 4 | 4 | u32 | `region` | 1, 2, 3, 4, 14, 5, 6 | **Landblock region number** *R* (`records.md` §2.2). 0x0F000006 → 14; 0x0F000009 → 6 (no `client_cell_6.dat` exists in this install). |
| 3 | 8 | 4 | DID | `map_image` | 0x4100000F (region 1), 0x410D9AE6 (the others) | A render surface (prefix 0x41): both are 512 × 512 user-interface images (category 15). The region's map image (Inferred; LC names it so). The first revision of this spec read these bytes as an f32 (8.0, 8.85). |
| 4 | 12 | 4 | u32 | `value_12` | 2 in all | OPEN |
| 5 | 16 | 4 | u32 | `origin_x` | 1,468 | Map-coordinate origin along X, in steps of 20 world units (BIN-R (c)) |
| 6 | 20 | 4 | u32 | `origin_y` | 1,244 | Map-coordinate origin along Y, in steps of 20 world units |
| 7 | 24 | 4 | f32 | `offset_x` | 0.0 | Added to the map X coordinate |
| 8 | 28 | 4 | f32 | `offset_y` | 0.0 | Added to the map Y coordinate |
| 9 | 32 | 4 | f32 | `map_scale` | 0.005 | Map units per world unit |
| 10 | 36 | 10 | DID map | slot 1 | 1 entry | Scene-choice table, prefix 0x11 (§4) |
| 11 | 46 | 10 | DID map | slot 2 | 1 entry | Terrain-scene table, 0x12 |
| 12 | 56 | 10 | DID map | slot 3 | 1 entry | Encounter table, 0x14 |
| 13 | 66 | 10 | DID map | slot 4 | 1 entry | Sky description, 0x15 (§5.1) |
| 14 | 76 | 10 | DID map | slot 5 | 1 entry | Water table, 0x16 (`water.md` §4) |
| 15 | 86 | 10 | DID map | slot 6 | 1 entry | Fog-zone table, 0x17 (`water.md` §5) |
| 16 | 96 | 10 | DID map | slot 7 | 1 entry | Block-map property descriptor, 0x18 (`landblock.md` §5) |
| 17 | 106 | 2 | DID map | map 8 | empty in all 7 | Kind OPEN |
| 18 | 108 | 2 | DID map | map 9 | empty in all 7 | Kind OPEN |
| 19 | 110 | 4 | DID | slot 8 | 0x19… | Terrain type table, 0x19 (`terrain-texturing.md` §3) |

A **DID map** is a TSize (`records.md` §1.3: hint byte, VLE count) followed by count ×
(u32 key, DID). Maps 1–7 hold exactly one entry here, with key 0: hint 1, count 1, key 0, DID
(10 bytes). Maps 8 and 9 are hint 1, count 0 (2 bytes). The client asks every map for key 0
only (BIN-R (b)); what other keys would select is OPEN. "Slot 1–8" is the numbering the other
specs use; slot 8 is the trailing DID, not a map.

**Versions (BIN):** the deserialiser asks the archive for a per-type version. Versions below
24 have three more u32 after field 9; versions below 4 have, in addition, one more u32 after
those and one more DID map between slots 2 and 3; versions below 2 have no trailing DID;
versions 3 and 4 end with one more u32. Every record of this install has the newest layout
above (24 or later), and a reader that knows only that layout rejects anything that does not
parse exactly.

**Map coordinates (BIN-R (c)).** For block-local (*x*, *y*) of landblock (X, Y):
map X = `offset_x` + `map_scale` × (160 · X + *x* − 20 · `origin_x`) and
map Y = `offset_y` + `map_scale` × (160 · Y + *y* − 20 · `origin_y`). The client displays
|map Y| with a north or south mark by its sign and |map X| with east or west. With this
install's values, map X = 0.005 × (160 · X + *x* − 29,360) and map Y = 0.005 × (160 · Y +
*y* − 24,880). `coordinates.md` owns map coordinates; these fields should be read at run time
rather than hard-coded.

<a id="s3-2"></a>
### 3.2 Table DIDs per region (Confirmed)

| Region descriptor | *R* | Slot 1 (0x11) | Slot 2 (0x12) | Slot 3 (0x14) | Slot 4 (0x15) | Slot 5 (0x16) | Slot 6 (0x17) | Slot 7 (0x18) | Slot 8 (0x19) |
|---|---|---|---|---|---|---|---|---|---|
| 0x0F000000 | 1 | 0x11000000 | 0x12000000 | 0x14000000 | 0x15000007 | 0x16000000 | 0x17000000 | 0x18000000 | 0x19000000 |
| 0x0F000001 | 2 | 0x11000001 | 0x12000001 | 0x14000001 | 0x15000007 | 0x16000004 | 0x17000001 | 0x18000014 | 0x19000001 |
| 0x0F000002 | 3 | 0x11000002 | 0x12000002 | 0x14000002 | 0x15000007 | 0x16000005 | 0x17000002 | 0x18000015 | 0x19000002 |
| 0x0F000005 | 4 | 0x11000007 | 0x12000007 | 0x14000007 | 0x15000007 | 0x1600000A | 0x17000007 | 0x1800001A | 0x19000007 |
| 0x0F000006 | 14 | as region 2 | | | | | | | |
| 0x0F000008 | 5 | as region 4 | | | | | | | |
| 0x0F000009 | 6 | 0x1100000A | 0x1200000A | 0x1400000A | 0x15000007 | 0x1600000E | 0x1700000A | 0x1800001E | 0x1900000A |

So the region descriptor is how a client finds a region's block-map descriptor and other
per-region tables, and the region-number ↔ descriptor mapping is data, not code. The DIDs are
observations; read them at run time.

---

<a id="s4"></a>
## 4. Per-region tables (kinds Confirmed, BIN-R (a))

Five records exist for each of the prefixes 0x11, 0x12, 0x14, 0x16, 0x17, 0x19 (serials 0, 1,
2, 7, 10 except 0x16: 0, 4, 5, 10, 14), one per distinct region set of §3.2, and one 0x15.
Each kind is decoded in the spec named here; this section replaces the content-based
hypotheses of the first revision (§15).

| Prefix | Slot | DB type | Kind | Sizes (bytes) | Decoded in |
|---|---|---|---|---|---|
| 0x11 | 1 | 0x4A SCENE_DESC | Scene-choice table: scene keys → weighted lists of scene records 0x02. Starts with u32 0, then its DID. | 1,802 – 19,613 | `scenery.md` §4.3 |
| 0x12 | 2 | 0x1C TERRAIN_DESC | Terrain-scene table: terrain-type ids of the coarse grid → scene keys | 882 – 10,934 | `scenery.md` §4.2 |
| 0x14 | 3 | 0x13 ENCOUNTER_DESC | Encounter table. Header: own DID, u32 0, u32 6 (the block-map key it reads, `landblock.md` §5.6), u32 16 or 1, then a named default entry and encounter lists. | 58 (four records), 1,687 (region 1) | Layout OPEN; the encounter pass is out of scope offline (`scenery.md` §7.7) |
| 0x15 | 4 | 0x3F SKY_DESC | Sky description (one record shared by all regions) | 30,184 | §5.1 |
| 0x16 | 5 | 0x17 WATER_DESC | Water table: water types (surface record, volume entity) and the block-map keys of water type and level | 188 – 1,574 | `water.md` §4, §6 |
| 0x17 | 6 | 0x16 FOG_DESC | Fog-zone table (block-map key 3); no zone is active in this install | 137, 1,244 | `water.md` §5 |
| 0x18 | 7 | 0x21 PROPERTY_DESC | Block-map property descriptor | | `landblock.md` §5.2 |
| 0x19 | 8 | 0x35 TERRAIN_TYPE_TABLE | Terrain type table: the surface-layer `material` keys | 40,637 – 173,939 | `terrain-texturing.md` §3; `scenery.md` §4.4 (its procedural flag) |

Corrections to the first revision: 0x11 does list DIDs (its entries name 0x02 scene records);
0x12 is the terrain-scene table, not environment zones; 0x16 is the water table and its 33-byte
elements are water types, not a terrain palette; 0x17 holds **no** texture list (the values
0x400043FA and 0x4000460C reported there are four bytes straddling the f32 fog distances 500.0
or 9,000.0 and 10,000.0 of `water.md` §5.2); 0x19 is TERRAIN_TYPE_TABLE, not TERRAIN_DESC. The
per-vertex grid after the height grid (`landblock.md` §3.7) selects scenery, not textures, and
the surface layers index the 0x19 table (`landblock.md` §3.8).

The record prologues (own DID first, or a u32 then the DID) are listed in `records.md` §1.6.

---

<a id="s5"></a>
## 5. Sky description and day descriptions

Conventions for §5 and §6: all integers little-endian; "f32" IEEE single; "f64" IEEE double;
"colour" a u32 described in §5.6; "weighted list" the structure of §5.1.3. Every count is
checked against the bytes left before allocating.

<a id="s5-1"></a>
### 5.1 Sky description 0x15000007 (DB type 0x3F; Confirmed, 1 of 1 record, 30,184 bytes)

The only 0x15 record. Slot 4 of every region descriptor names it (§3.2).

<a id="s5-1-1"></a>
#### 5.1.1 Top level

| Order | Size | Type | Field | Value in this install |
|---|---|---|---|---|
| 1 | 4 | u32 | `version` | 1. A reader rejects other values (BIN: the client refuses them). |
| 2 | 4 | DID | `did` | own DID |
| 3 | 8 | f64 | `light_interval_min` | 1.0 |
| 4 | 8 | f64 | `light_interval_max` | 1.0 |
| 5 | 8 | f64 | `sky_interval_min` | 180.0 |
| 6 | 8 | f64 | `sky_interval_max` | 180.0 |
| 7 | 4 | DID | `game_time` | 0x35000000 (§6) |
| 8 | 4 | u32 | `group_count` *g* | 4 |
| 9 | varies | | *g* day groups | §5.1.2 |
| 10 | varies | | named follow lists | §5.1.4 |
| 11 | varies | | named sky lists | §5.1.4 |
| 12 | 4 | u32 | `default_count` *m* | 540 |
| 13 | varies | | *m* sky entries (§5.1.3): the **local default** table | |

The two (min, max) pairs are update intervals in seconds of the game clock (BIN: the client
uses the midpoint of each pair: it recomputes the blended lighting every
½(`light_interval_min` + `light_interval_max`) seconds and marks a sky refresh every
½(`sky_interval_min` + `sky_interval_max`) seconds, each divided by the clock's rate,
§11.1; the full cadence is §11.8). Inferred names.

<a id="s5-1-2"></a>
#### 5.1.2 Day group

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 4 | u32 | `day_type` | A day-type value (enum mapper 0x230000E8, §2) |
| 2 | 4 | f32 | `chance` | Relative frequency of this day type (15, 2, 2, 1 in this install) |
| 3 | 1 | u8 | `follow_is_named` | 0 or 1 (a reader rejects ≥ 2) |
| 4 | 4 or varies | | follow | If `follow_is_named` = 1: u32 key of a named follow list (§5.1.4). If 0: an inline **follow list**: u32 `key` (0 here), then a weighted list body of (day type u32, weight f32) pairs (§5.1.3) |
| 5 | 4 | u32 | `entry_count` *n* | 208 – 236 |
| 6 | varies | | *n* sky entries | §5.1.3 |

The follow list gives the relative weights of the **next** day's type (all four groups list
the same four day types; for example after "FairFairFair": 15, 1, 1, 1). BIN (BIN-E (b),
E19): the client links each group to its follow list, reports a missing one as an error, and
uses the follow list of the current day type's group to pick the next day type (§11.5); the
`chance` values are used when the current day type matches no group (§11.5 step 3).

<a id="s5-1-3"></a>
#### 5.1.3 Sky entry and weighted list

Sky entry:

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 4 | u32 | `sky_key` | An `Environment_LocalSky` value (§7.1); the local default table also has keys 1 ("DefaultIndoor") and 2 ("Default") of base mapper 0x230001B4 |
| 2 | 1 | u8 | `list_is_named` | 0 or 1 |
| 3 | varies | | list | If 1: u32 key of a named sky list (§5.1.4). If 0: an inline weighted list: u32 `key` (0 here) and a weighted list body of (DID u32, weight f32) pairs |

Weighted list body: u32 `count` *k*, then *k* pairs of (u32 value, f32 weight). In sky
entries the values are day-description DIDs (prefix 0x1A).

Census: every sky entry of the four day groups and of the local default table uses an
inline list (`list_is_named` = 0 everywhere); 539 of the 540 default entries and every group
entry hold exactly one DID; the default entry for key 2 holds two (weights 2.0 and 1.0). All other weights
are 1.0, except 10.0 on the group entries for key 2 and the default entry for key 1.

<a id="s5-1-4"></a>
#### 5.1.4 Named lists

Two tables follow the groups. Each starts with a count encoded as a **VLE** (`records.md`
§1.2; BIN-R: the client reads both counts with its VLE routine, the same one that reads the
counts of `water.md` §4) and then that many entries. The counts are 1 here (one byte).

| Table | Entry |
|---|---|
| Named follow lists | u32 `key`, weighted list body of (day type, weight). This install: one entry, key 0x10000003, pairs (0x10000007, 10.0), (0x10000002, 10.0) |
| Named sky lists | u32 `key`, weighted list body of (DID, weight). This install: one entry, key 0x10000001, pairs (0x1A00002B, 5.0), (0x1A000000, 5.0) |

No entry of this install refers to them by name (all `…_is_named` flags are 0).

<a id="s5-2"></a>
### 5.2 Day description (prefix 0x1A, DB type 0x3A; Confirmed, 963 of 963 records)

A day description is one weather variant of one sky: a list of **time-of-day keyframes**
and a list of **sky objects**.

| Order | Size | Type | Field | Census |
|---|---|---|---|---|
| 1 | 4 | u32 | `unknown0` | 0 in all 963 |
| 2 | 4 | DID | `did` | own DID |
| 3 | 4 | u32 | `keyframe_count` *n* | 1 (335 records), 2 – 21 (628); never 0 here |
| 4 | varies | | *n* keyframes | §5.3; times ascending in all 963 |
| 5 | 4 | u32 | `object_count` *m* | 0 – 21 (13 most common) |
| 6 | varies | | *m* sky objects | §5.4 |
| 7 | 4 | f32 | `transition_seconds` | 0.0 (803), 2.0 (70), 8.0 (54), 1.0 (18), 0.5 (18) |

`transition_seconds` (Inferred name, BIN use): when the **sky key** changes, the client
cross-fades from the old sky to the new one over the new sky's current day description's
`transition_seconds` if it exceeds 0.0002, else over the old one's if that exceeds 0.0002,
else over 8 seconds (§11.6; BIN-E (f), E27). A new day description under the same sky key
(a new game day or day type) is not cross-faded. (The first revision said the cross-fade
followed a change of day description; corrected 2026-10-07.)

<a id="s5-3"></a>
### 5.3 Keyframe (Confirmed layout; meanings BIN)

| Order | Size | Type | Field | Meaning | Census (7,590 keyframes) |
|---|---|---|---|---|---|
| 1 | 4 | f32 | `time` | Time of day, fraction 0 – 1 (§11.1) | 0.0 (652), 1.0 (627), 0.2, 0.31, 0.75, 0.275, 0.79, 0.225 most common |
| 2 | 4 | f32 | `fog_near` | Fog start distance from the eye, world units (`coordinates.md` §3: one world unit, by convention a metre; the same unit as every other distance in the spec set) | 200, 100, −100, 0, … (60 values) |
| 3 | 4 | f32 | `fog_far` | Fog end distance from the eye, world units | 2500, 1200, 1700, … (92 values) |
| 4 | 4 | f32 | `fog_extra` | Amount of fog applied to the sky, 0 – 1 (Inferred name). BIN: blended like the others; kept only when fog is on and it exceeds 0.001; above 0.99 the client does not draw the sky objects (§11.7) | 0.0 (5,082), 0.5 (884), … |
| 5 | 4 | colour | `fog_colour` | Fog colour | |
| 6 | 4 | f32 | `sun_brightness` | Directional light brightness | 0.55 (5,508), 0.0 (557), … ; exactly one keyframe stores 0.0001 (f32 0x38D1B717), which is below the 0.0002 limit of §11.7 step 9, so that keyframe gives no sun (E38; the 2026-10-05 census counted it with the zeros as 558) |
| 7 | 4 | f32 | `sun_heading` | Degrees | 15 (5,128), 50 (1,571), 0, 90, … |
| 8 | 4 | f32 | `sun_pitch` | Degrees | 150 (5,030), 158 (1,568), 0, 50, … |
| 9 | 4 | colour | `sun_colour` | Directional light colour | |
| 10 | 4 | colour | `sun_specular` | Directional specular colour (grey in most keyframes) | |
| 11 | 4 | f32 | `ambient_brightness` | | 0.2 (5,539), 0.25, 0.3, 0.1, … |
| 12 | 4 | colour | `ambient_colour_1` | Upper (sky-facing) ambient; × `ambient_brightness` (BIN: indoors `InteriorUpperAmbientLight` replaces this product, §11.7; use in `lighting.md` §8.1) | |
| 13 | 4 | colour | `ambient_colour_2` | Lower (ground-facing) ambient; × `ambient_brightness` (BIN: indoors `InteriorLowerAmbientLight` replaces this product). Equal to colour 1 in 5,510 keyframes; black in 318 | |
| 14 | 4 | colour | `colour_3` | Blended like the others; the environment state carries its R, G and B × 0.85 and its alpha unchanged (BIN, §11.7); use OPEN | |
| 15 | 4 | u32 | `object_entry_count` *r* | | 0 – 21 |
| 16 | 20·*r* | | object entries | §5.3.1 | |
| 17 | varies | | four float curves | `material.md` §5.3 grammar; blended per curve, evaluated on the frame clock. Curve 1: **rain intensity**, clamped to [0, 1]; curve 2: **snow intensity**, clamped to [0, 2] (BIN, published as `World_RainIntensity` and `World_SnowIntensity`, §11.7); curves 3 and 4: precipitation-related values (set to 0 by the weather override, §11.7), use OPEN | 7,589 / 7,590 / 7,520 / 7,548 of them mode 1 |
| 18 | 24 | 6 × f32 | `values_a` | Blended linearly and carried into the environment state; the client's defaults are (0, 1, 2, 0.07, 0.6, 0) (BIN); use OPEN | (0.033, 1.0, 2.0, 0.07, 0.6, 0.03) most common |
| 19 | 4 | f32 | `value_b` | The client clamps the blended value to [0, 1] and carries it into the environment state; default 0 (BIN); use OPEN | 0.0 (7,231) |
| 20 | 4 | DID | `effect` | 0 (7,087) or a 0x47 object (503) | |
| 21 | 0 or 4 | u32 | `effect_extra` | Present only when `effect` ≠ 0; 0 in all 503 | |
| 22 | 4 | colour | `colour_4` | Blended (alpha included) and carried into the environment state (BIN); use OPEN. White (7,522), dark grey, pink | |

Field names are Tessera's. Meanings that rest on BIN: the client's keyframe blending routes
fields 2–5 into the fog state (near, far, extra, colour), fields 6–13 into the light state,
and builds the light direction from fields 7 and 8 (§11.3, `lighting.md` §8.1). Field 10:
for older data versions without it, the client derives a grey from the sun colour and
brightness, which supports the reading "specular colour".

`effect` (Inferred): a weather or precipitation effect object; it is taken from one of the
two blended keyframes, not interpolated (§11.3). BIN (BIN-E (g)): while the blended `effect`
is not the null DID, the client treats the sun as absent (§11.7, "sun present").

Older data versions (BIN-E (j), E33; for readers of other builds; every record of this
install has the newest form): the keyframe reads a per-type version; below 3
`sun_specular` is absent and derived (see above); below 6 `ambient_colour_2` is absent (a
copy of colour 1); below 2 the four curves are replaced by four f32 constants; `effect` is
present from version 5, `colour_4` from version 7 (below that it must equal the default).

> **OPEN:** Uses of `colour_3`, `colour_4`, curves 3 and 4, `values_a` and `value_b`
> beyond the routing stated above (BIN-E traced them into the environment state of §11.7
> but not into the renderer). Candidates from the renderer's settings vocabulary: sky glow,
> cloud cover and speed, precipitation wind. The use of `fog_extra` in the fog formula
> itself (beyond hiding the sky above 0.99) is `lighting.md`'s fog question.

<a id="s5-3-1"></a>
#### 5.3.1 Keyframe object entry (20 bytes)

| Offset | Size | Type | Field | Census (107,213 entries) |
|---|---|---|---|---|
| 0 | 4 | u32 | `object_index` | a sky object's `index` (§5.4); 3,214 entries name an index that the record does not define and have no effect |
| 4 | 4 | DID | `replacement` | 0 in all |
| 8 | 4 | f32 | `angle_b_offset` | 0 in all |
| 12 | 4 | f32 | `angle_a_offset` | 0 in all |
| 16 | 4 | colour | `tint` | alpha 0 in 61,529 entries, 255 in 22,819, others between |

At the keyframe's time the object of that index is drawn with this tint (alpha = opacity:
stars and moon fade by alpha), with its angles offset and, if `replacement` ≠ 0, with that
object instead (BIN; offsets and replacements are unused in this install). The exact rules
(which keyframe gives which part) are in §11.4 step 4 (BIN-E (e), E26).

<a id="s5-4"></a>
### 5.4 Sky object (Confirmed layout; meanings BIN where stated)

| Order | Size | Type | Field | Census (12,788 objects) |
|---|---|---|---|---|
| 1 | 4 | u32 | `index` | 1 – 136, unique within a record |
| 2 | 4 | DID | `object` | 0x47 objects (12,781; 129 distinct), 0 (7) |
| 3 | 4 | f32 | `begin_time` | 0.0 in all |
| 4 | 4 | f32 | `end_time` | 1.0 in all |
| 5 | 4 | f32 | `angle_a_begin` | degrees: 0 (9,218), 90, −90, 345, … |
| 6 | 4 | f32 | `angle_a_end` | degrees: 0, 430, 270, 345, … |
| 7 | 4 | f32 | `angle_b_begin` | degrees: 0, 75, 120, −30, 137, … |
| 8 | 4 | f32 | `angle_b_end` | degrees: 0, 75, 120, 210, 137, … |
| 9 | 4 | u32 | `unknown_20` | 0 in all |
| 10 | 1 | u8 | `flag_25` | 0 in all |
| 11 | 1 | u8 | `flag_26` | 0 in all |
| 12 | 1 | u8 | `flag_30` | 0 in all |
| 13 | 4 | f32 | `value_28` | 0 in all (the client converts it to an integer) |
| 14 | 4 | f32 | `value_2c` | 0 in all (converted to an integer) |
| 15 | 1 | u8 | `reverse` | 0 in all; 1 = angles run downward (§11.4) |
| 16 | 4 | u32 | `layers_a` | 0x3F800000 (11,997) or 0x40C00000 (791); read as f32: 1.0 or 6.0 |
| 17 | 4 | u32 | `layers_b` | 1.0 (11,997) or 0.6 (791) as f32 |
| 18 | 4 | u32 | `layer_count` *q* | 0 (11,997) or 9 (791) |
| 19 | 12·*q* | | *q* layers of (f32, texture-list DID 0x40…, f32) | first floats −1.0 … 0.7 ascending |
| 20 | 1 | u8 | `layers_flag` | 1 (12,787), 0 (1) |
| 21 | 4 | f32 | `layers_value` | 0.06 in all |
| 22 | 1 | u8 | `flag_31` | 0 (11,974), 1 (814) |

All u8 flags are 0 or 1 (the client rejects other values). Fields 16–21 form one
sub-structure (BIN boundary); its meaning is OPEN (the 9-layer form, with one texture list per
layer keyed by a value from −1 to 1, fits a moon-phase or cloud-cover selection). BIN
(BIN-E (e)): the client copies the sub-structure to the drawn object only when its
`layer_count` is not 0. Field 22 is read only from per-type version 8 on (BIN-E (j), E33;
every record of this install has it).

**Angles (BIN-E (e), E25).** Fields 5–6 give **angle A** and fields 7–8 **angle B**
(§11.4 step 3). A is a rotation about the world's +Y (north) axis and B a rotation about the
vertical +Z axis, both by the negated angle; B is applied first (§11.4 step 5). Fields 9–14
are copied into the drawn object unchanged (fields 13 and 14 truncated to integers); their
use is OPEN (all are 0 in this install). Field 22 is copied into the drawn object as well;
field 15 (`reverse`) is not, it acts only in §11.4 step 3 (E39).

The `object` DIDs are physics-object records (prefix 0x47, `entity-descriptor.md` §4.3), the
same kind placed entities use; they resolve to the geometry drawn for the sky (domes, sun,
moon, cloud layers, stars).

<a id="s5-5"></a>
### 5.5 Coverage (Confirmed)

The day groups mention 238 sky keys and the local default table 540 (612 distinct keys in
all); together the tables name all 963 day descriptions. Of the 647 sky keys of the
0x1000xxxx form, 610 appear.

<a id="s5-6"></a>
### 5.6 Colours (BIN)

A colour is a u32 whose value is 0xAARRGGBB, stored little-endian, so the bytes in the
record are blue, green, red, alpha. Each channel divided by 255 gives a value in [0, 1].

---

<a id="s6"></a>
## 6. Game time 0x35000000 (DB type 0x02; Confirmed layout, 180 bytes)

| Order | Size | Type | Field | Value in this install |
|---|---|---|---|---|
| 1 | 4 | DID | `did` | own DID |
| 2 | 4 | f32 | `day_length` | 11,160.0 seconds (3 h 6 min) |
| 3 | 4 | u32 | `days_per_year` | 360 |
| 4 | 8 | f64 | `zero_offset` | 3,600.0 seconds |
| 5 | 4 | u32 | `zero_year` | 34 |
| 6 | 4 | f32 | `value_54` | 0.8 |
| 7 | 4 | f32 | `value_58` | 0.25 |
| 8 | 4 | u32 | `period_count` *a* | 4 |
| 9 | 8·*a* | | *a* × (f32 `start`, u32 `name`) | starts 0.0, 0.25, 0.5, 0.75 |
| 10 | 4 | u32 | `weekday_count` *b* | 1 |
| 11 | 4·*b* | | *b* × u32 `name` | |
| 12 | 4 | u32 | `month_count` *c* | 12 |
| 13 | 8·*c* | | *c* × (u32 `start_day`, u32 `name`) | start days 0, 30, 60, … 330 |
| 14 | 4 | u32 | `unknown_last` | |

`name` values are string tokens (resolution OPEN; they do not decode as DIDs). Field names are
Tessera's; fields 2–5 and the month table have BIN-confirmed roles (§11.1).

BIN (BIN-E (h), E18):

- `value_54` is the **night start** and `value_58` the **night end**, as time-of-day
  fractions: it is night when *t* < `value_58` or *t* ≥ `value_54` (with this install's 0.8
  and 0.25: from 0.8 through midnight to 0.25). The client publishes this as the property
  `World_IsNight` (PR). Fields 6–7 keep their Tessera names for stability.
- The **period** in force is the last period whose `start` ≤ *t* (index 0 when *t* is below
  the first `start`); it is recomputed whenever *t* changes. Its `name` is one of the date
  text's tokens.
- The date text the client builds uses: the year number, the day of the month
  (*d* − the month's `start_day` + 1), the month's `name`, the period's `name` and
  `unknown_last` (a further name token; Inferred: the calendar's or age's name). The text
  template and the string table of the tokens are OPEN (§10).
- The weekday table (field 11) is read but no use was found in the date text.
- The client's defaults for a game-time object without data are a day length of 100,000 s,
  1 day per year, night start 0.75 and night end 0.25 (BIN-E (h), E31); they matter only to
  a reader that fails to load 0x35000000.

---

<a id="s7"></a>
## 7. How the world chooses an environment (property hooks)

<a id="s7-1"></a>
### 7.1 Sky per landblock (Confirmed)

- Property `Environment_LocalSky` (enum mapper 0x230000E9, which inherits from base mapper
  0x230001B4; 650 labelled values, 647 of them of the form 0x1000xxxx). Labels are
  internal identifiers (area-and-mood names); read them at run time.
- **Block maps** carry it: every region's block-map descriptor (0x18) has a key 0x1000000F
  whose value list is `Environment_LocalSky` values (250 values for region 1, 194 for 2, 33
  for 3, 98 for 4/5, 6 for 6). So every landblock selects a sky through its block map
  (`landblock.md`).
- Overrides: 299 cell property sets, 193 landblock-info property sets and 147 stand-alone
  0x78 property sets (content-layer and similar sets) also carry `Environment_LocalSky`.
  Related enum properties: `Environment_PlayerLocalSkyOverride`,
  `Environment_LocalSky_HousingHook`.
- Path to visuals (Confirmed, §5.1, §11.2): sky key → sky entry of 0x15000007 (a day group's,
  else the local default table's) → weighted list → one 0x1A day description per game day.

> **OPEN (implementer, 2026-10-06, `tessera-world` environment module):** which sky key wins
> when several sources apply (the player's `Environment_PlayerLocalSkyOverride`, a monitor
> set the player is inside, the cell's `Environment_IndoorAmbiance` or
> `Environment_LocalSky`, the landblock-info property set, the block map), and whether
> `Environment_IndoorAmbiance` replaces the sky key indoors at all (§7.2). Tessera
> (Decision): first present in that order.
>
> **Answered 2026-10-07 (BIN-E (c), E21, E22)** by §7.1.1 below: the client keeps two keys,
> an outdoor sky key and an indoor ambience key, each from its own precedence list; the
> indoor ambience does not replace the sky key but drives a separate indoor track, which is
> used instead of the outdoor one while the landscape is not visible from the viewer
> (§11.6). The Tessera Decision above should be replaced by §7.1.1.

<a id="s7-1-1"></a>
#### 7.1.1 Key precedence and the position lookup (BIN-E (c))

**Outdoor sky key.** The first present of:

1. the controlled character's own `Environment_LocalSky_HousingHook`;
2. the controlled character's own `Environment_PlayerLocalSkyOverride`;
3. the controlled character's own `Environment_LocalSky`;
4. `Environment_LocalSky` at the viewer's position (position lookup below);
5. otherwise 2 (the base mapper's "Default").

**Indoor ambience key.** The first present of:

1. the controlled character's own `Environment_PlayerIndoorAmbianceOverride`;
2. the controlled character's own `Environment_IndoorAmbiance`;
3. `Environment_IndoorAmbiance` at the viewer's position;
4. otherwise 1 (the base mapper's "DefaultIndoor").

The character's own properties are set by the game at run time (none is stored in the
install's data); offline, steps 1–3 of both lists find nothing. A value counts as present
only when its value has property type code 8 (enum value) or 10 (`property-system.md` §3);
any other stored type is reported and treated as absent. The interior properties of §7.2 are
taken only with their own types (float 9 for near and far, colour 16 for colours).

**Position lookup.** For a property at the viewer's position the client asks, in order:

1. if it holds a run-time object for the position (Inferred: the loaded cell or landblock
   object), that object's two property sets, the first before the second; the lookup ends
   there whether or not the property was found;
2. otherwise the property set of the position's landblock-info record (DB type 0x2E,
   `landblock.md` §4), then the region's block-map value at the position (the region's
   block-map property descriptor, DB type 0x21, `landblock.md` §5).

The same lookup serves the interior fog and ambient properties of §7.2. Which run-time object
step 1 consults, and the content of its two sets, are OPEN; **Tessera Decision:** a cell's
own property set first, then the landblock-info set, then the block map.

<a id="s7-2"></a>
### 7.2 Interior fog and ambience (Confirmed census)

| Property | Type | Where (count) |
|---|---|---|
| `InteriorFogType` | enum 0x2300016A (Invalid 0, Linear 1, None 2) | cells (775), landblock info (541) |
| `InteriorFogNear`, `InteriorFogFar` | f32 | cells (775 each), landblock info (546, 540), 0x78 sets (7) |
| `InteriorFogColor` | colour | cells (775), landblock info (533), 0x78 sets (7) |
| `Environment_IndoorAmbiance` | enum 0x230000E9 (the sky enum) | cells (777), landblock info (40) |
| `EnvironmentType` | 64-bit bitfield | landblock info (3) |
| `InteriorUpperAmbientLight`, `InteriorLowerAmbientLight` | colour | cells (643 each; OBS5) |
| `InteriorAmbientLight` | colour | cells (66; OBS5) |

Interiors therefore have linear fog with near/far distances and a colour, and choose an
"indoor ambiance" from the same enum as outdoor skies.

**How the client applies them (BIN-E (d), E23).** The client reads these properties at the
viewer's position with the lookup of §7.1.1 and applies them only to the **indoor track**
(§11.6), on top of the values of the indoor ambience key's day description:

| Property (PR) | Effect when present | When absent |
|---|---|---|
| `InteriorFogType` | Fog is on only if the value is 1 (Linear) **and** distance fog is enabled in the client's settings; 0 (Invalid), 2 (None) or any other value turns the fog **off** | The day description's fog on/off stands |
| `InteriorFogNear` | Replaces the fog near distance | Keyframe value stands |
| `InteriorFogFar` | Replaces the fog far distance (limits of §11.7 still apply, except the 10,000 cap) | Keyframe value stands |
| `InteriorFogColor` | Replaces the fog colour (all four channels) | Keyframe colour stands |
| `InteriorUpperAmbientLight`, `InteriorLowerAmbientLight`, `InteriorAmbientLight` | If **any** of the three is present, both ambient terms of the day description (upper = `ambient_colour_1` × `ambient_brightness`, lower = `ambient_colour_2` × `ambient_brightness`) are replaced: upper = `InteriorUpperAmbientLight` (opaque black when absent), lower = `InteriorLowerAmbientLight` (opaque black when absent); then, if `InteriorAmbientLight` is present, its R, G, B are added to both and every channel of both is clamped to at most 1 | If none is present, the day description's ambient terms stand |

A colour property counts only if its stored value has the colour type; a near or far only if
it is a float. The census of the three `Interior…AmbientLight` properties covers cells only
(OBS5); landblock-info and other property sets were not scanned for them.

<a id="s7-3"></a>
### 7.3 Time of day (Confirmed values)

- Enum mapper 0x230001E4 (time-of-day values, 15 including Undef): Night, Dusk, Day, Dawn,
  Day_Mid, Night_End, Night_Early, Day_Late, Day_Early, Night_Late, Night_Start, and others.
- `TimeOfDay@Entity.m_eTimeOverride` (enum mapper 0x23000161: Night 1, Automatic 2,
  Never 3, Dusk 4, Always 5, Day 6, Dawn 7) on 42,429 static entities and 24 object property
  sets: entities that exist or light up only at certain times (lamps at night, for example).
- Lights carry day and night variants of their parameters
  (`Light.Script@Entity.Night.m_fLightRange`, `…Day.m_bLightActivated` and similar, on
  2,700 – 12,800 light entities each; `entity-descriptor.md` §4.5).
- `TimeOfDay_Control_*` properties (array of time structs, starting time) exist in the
  dictionary; no stored value was found in the places scanned.

<a id="s7-4"></a>
### 7.4 Other environment properties (Confirmed census)

| Property | Where (count) | Note |
|---|---|---|
| `WindSpeed` (enum 0x23000362: Undef, Medium, Low, High, Interior, LowMed) | static entities (18,334) | Foliage wind class |
| `Render_IsFogVolume` | static entities (173) | Local fog volumes |
| `Render_EtherealToPrecipitation` | 0x47 physics objects (e.g. invisible walls) | Precipitation passes through |
| `Landblock_UseCeiling` | block-map key 9 (two values per region) | Landblocks with a terrain ceiling (caves built from landscape, ER §3) |
| `Landblock_Temperature` | block-map key 0x10000011 (7 values per region) | OPEN (cold/heat gameplay or visuals) |
| `WaterHeight`, `WaterType`, `DistanceFogType`, `ForceSunlight`, `GlobalCamera_UnderWater` | defined in the dictionary; **no stored value** in landblock info, cells, entities, landblock properties or 0x78/0x79 sets | Runtime-only or unused. Water levels and types are block-map values (`water.md` §6). |

---

<a id="s8"></a>
## 8. Water (moved to `water.md`)

Water is specified in [`water.md`](water.md), which supersedes the first-look notes that
stood here. In short:

- Each region's **water table** (slot 5, prefix 0x16) lists water types; each landblock
  selects a type and a level through block-map keys 7 and 4, and has water when the level's
  height (level × 1000 / 255) is above its lowest terrain sample (`water.md` §6; 89,913
  landblocks of this install have water).
- Water surfaces are drawn as one quad per wet landblock with the type's surface record and
  the water shaders (`water.md` §8); placed water entities add ponds, pools and falls with
  their own meshes and liquid volumes (`water.md` §7.3).
- The slot-6 table (prefix 0x17) is the **fog-zone table**, read through block-map key 3; no
  zone is active in this install (`water.md` §5). It holds no texture list.

The properties `WaterHeight` and `WaterType` of §7.4 stay unused: water levels are block-map
values, not properties.

---

<a id="s9"></a>
## 9. Other prefixes seen while looking (kinds Confirmed, BIN-R (a))

| Prefix | Records | DB type | Observed | Spec |
|---|---|---|---|---|
| 0x02 | 3,402 (general) | 0x3C SCENE | u32 0, then own DID; placement nodes | `scenery.md` §5 (decoded) |
| 0x08 | 11 (general) | 0x0D CAMERASTATE | No own DID: the first u32 is the camera entity's DID (a 0x47 descriptor), then f32 collision smoothing time, near clip, far clip, four flag bytes, the controller type and a body by type (E16) | [`walker.md`](walker.md) §11.5 (decoded) |
| 0x2D | 3 (general) | 0x62 COMPUTESHADER | A DXBC shader container | Not needed |
| 0x32 | 1 (general) | 0x3B RENDER_LOD_CLASSES | LOD-class distance table, decoded below | §9.1 |
| 0x39 | 11,155 (general) | 0x1E PSDESC | Particle-system descriptors; no own DID in the first 8 bytes | OPEN |
| 0x44 | 254 (general) | 0x56 FORKPSDESC | Texture file names and particle template labels; no own DID in the first 8 bytes | OPEN |

<a id="s9-1"></a>
### 9.1 LOD-class table 0x32000000 (Confirmed layout, 1 of 1, 1,160 bytes, zero leftover)

| Order | Size | Type | Field | Observed |
|---|---|---|---|---|
| 1 | 4 | DID | own DID | |
| 2 | 4 | u32 | `count` *n* | 32 |
| 3 | 36 · *n* | entry × *n* | `entries` | |

Entry: u32 `class` (a `Render_LODClass` value, enum mapper 0x2300008D), then eight f32
`d0` … `d7`. Observed: `d0` 25 – 4,000 (for example 500 for the class labelled
"FarView500", 1,000 for "Landmark", 25 for "SmallDecor"), `d1` 2 – 200, `d2` 2 – 100, `d3` 80
(26 once), `d4` 1,000, `d5` 1,000 (12 once), `d6` −1 (50 once), `d7` 1,100 (20 once).
Reading (Inferred): `d0` is the class's maximum draw distance in world units, the following
values distances of lower detail levels. The meaning of each field and how the client
applies it are OPEN (BIN not traced). `scenery.md` §9.1 uses this record for its LOD
classes and names the eight values `v1` … `v8` (= `d0` … `d7` here).

**Owner of the layout.** This section is the single statement of the 0x32 layout
(`records.md` §2.1 points here). The record's payload is **1,160 bytes**: own DID, `count`
32, and 32 entries of 36 bytes (4 + 4 + 32 × 36 = 1,160; OBS3, re-checked 2026-10-05).
There is no `length` field and no second copy of the DID in the payload.

> **Correction for `scenery.md` §9.1 and SC9 (for the scenery owner):** its table lists a
> `length` u32 (1,160) and a `did_echo` after the own DID and gives the record as 1,168
> bytes. Those 8 extra bytes are not in the record payload (they match a dump-file header
> of DID and length placed in front of it). The layout is the one above; SC9's check of
> `length` should be dropped, and the meaning of the values deferred to this section.

---

<a id="s10"></a>
## 10. Open questions (summary)

> **OPEN:** Region descriptor field 4 (offset 12, value 2), the kinds of maps 8 and 9 (empty
> here), and what keys other than 0 would select in the slot maps (§3.1); region number 6
> without a cell archive.

> **OPEN:** Layout of the encounter table, prefix 0x14 (§4). Not needed offline. (The other
> per-region tables are decoded in `scenery.md`, `terrain-texturing.md` and `water.md`.)

> **Answered 2026-10-07 (BIN-E (b), E19):** how the client sets the current **day type**.
> The first revision said it "starts at 2"; that was the default **sky key** (§7.1.1), not
> the day type. The game clock computes the day type itself at every game-day change (§11.5);
> once set, it is a type named by a day group or a follow list (in this install always one of
> the four group types); the exceptions are in the remarks of §11.5.
> Tessera should run §11.5 instead of the former Tessera Decision (local default table only).

> **Answered 2026-10-07 (implementer OPEN of 2026-10-06, sky keys listed only by the day
> groups):** because the day type is always a group's type (§11.5), §11.2 step 1 finds the
> 81 group-only keys in the current group, so they are not lit by the defaults. Only keys in
> no table (20 in the block-map palettes of this install) give no description, as before.
> The Tessera fallback `choose_day_with_fallback` (highest-`chance` group) is not the
> original's rule and should be replaced by §11.5 + §11.2.

> **OPEN:** Uses of keyframe `colour_3`, `colour_4`, curves 3 and 4, `values_a` and
> `value_b` beyond their routing (§5.3); sky-object fields 9–14 and 16–22 (§5.4); how a
> mode-10 curve (`material.md` §5.3) behaves after the curve blend of §11.3 step 4a, which
> carries only the mode and `w[0..9]`.

> **OPEN:** The string table and template of the date text, and so the resolution of the
> period, month and `unknown_last` name tokens (§6).

> **OPEN (needs the owner, O21):** the live client's **epoch**. In the code examined the game
> clock has no wall-clock epoch: it counts from the frame clock, which starts at 0 when the
> client starts and can be re-based from outside the game-time module, plus the
> `World_TimeOffset` property (§11.1.1). Which of these the live session sets, and to what,
> is decided outside that module (Inferred: by the connection layer, which is excluded by
> decision 0004). Two (game time, UTC) pairs from the owner fix the epoch for Tessera's
> settable start.

> **OPEN:** Sky rendering details not settled by BIN-E: whether the renderer centres sky
> objects on the camera, their scale and depth handling, the clear colour, how the sort keys
> of §12 order the draw, and how fog (`fog_extra` below 0.99) is applied to the sky in the
> shaders (§12).

> **OPEN:** The run-time object consulted first by the position lookup (§7.1.1 step 1) and
> the local environment sources the client mixes into a sky's state when its "local" flag
> is set (§11.7 step 1).

> **OPEN:** Layouts of 0x39 and 0x44; the meaning of the LOD-class fields (§9). (The 0x08
> camera-state layout is decoded in [`walker.md`](walker.md) §11.5; it is not open.)

<a id="s10-1"></a>
### 10.1 Owner observations in this spec's scope (answered from the client where BIN can)

The items of `docs/checklists/owner-observations.md` that this spec owns, with what BIN-E
settled (2026-10-07). The checklist itself is updated by its owner; this table is the
spec's record.

| Item | Question | Result | Answer here, or why the owner is still needed |
|---|---|---|---|
| O21 (1, 2) | Game clock, day length, rate, date | **Partly** | Rate: one game day per `day_length` / *ρ* real seconds (11,160 s = 3 h 6 min at the default *ρ* = 1; *ρ* is `World_TimeDilation`); year of 360 days, 12 months of 30 days; year number, day of month and month as in §6 and §11.1 (§11.1.1). **Epoch needs the owner:** the client's clock has no wall-clock epoch of its own; it counts from the timer's start plus values set from outside the game-time module (Inferred: the connection layer, excluded by decision 0004), so only the two (in-game time, UTC) pairs of O21 can fix the epoch of the live game. |
| O21 (3) | When dusk and night start | **Partly** | There is no single "dusk" event: light and fog follow the keyframes (§11.3). The client's night flag (`World_IsNight`) switches on at *t* = 0.8 and off at *t* = 0.25 (§6). The real times depend on the epoch (owner). |
| O21 (4), O26 (day type) | How the day type is chosen; weather per day | **Answered** (rule); live outcome needs the owner | §11.5: the client computes it at each game-day change from the previous day's type (follow list, hash *h*₁) or, from a fresh clock, from the groups' chances (hash *h*₂). The former "day type starts at 2" was the default sky key (§11.2). Whether the live client shows the same weather on a given date depends on the epoch, on when its clock was created, on whether the server sets `World_DayType` itself (not analysed), and on §11.8 (a day-description pick waits for a key change). Several days' weather at one spot (O21 (4)) checks this. |
| O26 | Sun and moon path and size | **Partly** | Path: §11.4 step 5 (rotation about the vertical axis by −B, then about the north–south axis by −A) with the day's A and B from §11.4 step 3. Apparent size (scale), camera centring and depth stay OPEN (§12). |
| O26 | Interior fog (Y5 and a cave) | **Answered** (rules) | §7.2 and §11.7: indoors the client uses the indoor ambience key's day description; `InteriorFogType` 1 keeps fog, 0, 2 or other values turn it off; near, far and colour override where present; limits far ≥ 20, near ≤ far − 20. The look (colour, distance at a given spot) is data and needs no observation; the owner may confirm. |
| O26 | Fog colour and distance at four times of day (Y1–Y4) | **Needs the owner** | The values are data and §11.3 blends them exactly, but whether Tessera's picture matches is a visual comparison; the time of day to stand there depends on the epoch (O21). |
| O26 | How fog applies to the sky | **Partly** | Above `fog_extra` 0.99 (with fog on) the sky objects are not drawn (§11.7 step 10); how smaller values tint the sky is a shader question (OPEN, §12). |
| O22 (4) | Fog clipping | **Partly** (fog values only) | The fog far distance used outdoors never exceeds 10,000 and never falls below 20 (§11.7 step 5). How draw distance follows the fog is `walker.md`'s (owner item O22 stays with that spec). |

Items O18–O20, O23–O25 and O27 are owned by other specs. One fact found here may help
`lighting.md` with O23 (shadows): the environment state's "sun present" flag (§11.7 step 9)
is false indoors, while a keyframe `effect` is active, and when the sun colour is opaque black
or its brightness is below 0.0002.

---

<a id="s11"></a>
## 11. Algorithms (BIN unless stated)

<a id="s11-1"></a>
### 11.1 Game clock and time of day

State: a clock *G* in seconds (f64) and a rate *ρ* (1 by default; the client has a console
setting for it).

1. Each frame, *G* increases by the elapsed real time × *ρ*.
2. *T* = *G* + `zero_offset`.
3. Year length *Y* = `days_per_year` × `day_length`. Year index *i* = floor(*T* / *Y*); the
   year number is *i* + `zero_year`.
4. *r* = *T* − *i* · *Y*. Day of year *d* = floor(*r* / `day_length`).
5. **Time of day** *t* = (*r* − *d* · `day_length`) / `day_length`, a fraction in [0, 1).
   Keyframe times (§5.3) and sky-object times (§5.4) are on this scale.
6. **Game day number** *n* = `days_per_year` × (year number) + *d*.
7. Month index = the last month-table entry whose `start_day` ≤ *d*.

<a id="s11-1-1"></a>
#### 11.1.1 Clock source, rate and offset (BIN-E (a), E17)

The client's game clock is built as follows (this refines step 1; steps 2–7 are unchanged).

- **Frame clock** *F*: seconds (f64) from the client's high-resolution timer, 0 when the
  timer starts at client start-up and advancing with real time. The same clock drives the
  float curves (`material.md` §5.3.1). The timer has a re-base operation that sets *F* to a
  value supplied by other code. Who calls it was not traced: the one caller of the timer
  that was opened belongs to the connection layer, and that layer is excluded (decision
  0004); that the live client re-bases *F* there is Inferred.
- **Time offset** *O*: f32 seconds, 0 at start; the property `World_TimeOffset` (PR) sets it.
- **Rate** *ρ*: f32, 1 at start; the property `World_TimeDilation` sets it. The console
  command that sets it accepts only values above 0.0002.
- Two further f64 terms of the sum below exist in the clock object; nothing in the build
  writes them after creation, so they are 0.

Per update (the world-render update calls it, once per frame, before the sky update of §11.8;
the per-frame call is Inferred from the caller):

1. *U* = *F* + *O*.
2. *G* ← *G* + *ρ* · (*U* − *U*prev); *U*prev ← *U*. (So a change of *O* moves *G* by
   *ρ* · Δ*O*, and a change of *ρ* affects only later time.)
3. *t* = (*G* − *S*) / `day_length` (f32), where *S* is the clock value at the start of the
   current game day. While *t* > 1, run the **day change** below and compute *t* again.
4. When *t* differs by more than 0.001 from the value last published, the client publishes it
   as `World_PresentTimeOfDay`; when night (§6: *t* < `value_58` or *t* ≥ `value_54`)
   changes, it publishes `World_IsNight`.

**Day change** (also run once when the clock object is created, with *G* = 0 and
*U*prev = 0): compute *T*, the year number, *r*, *d* and the month as in steps 2–4 and 7; set
*S* = *G* − (*r* − *d* · `day_length`), the product taken in f32; then update the day type
(§11.5) with the game day number *n* of step 6.

**Console.** Setting the time of day to *x* (accepted only for 0 ≤ *x* ≤ 1) adds
`day_length` × (*x* − *t*) to *O* when |*x* − *t*| > 0.000001; *G* then jumps by *ρ* times
that amount.

**Consequence for the epoch.** With no re-base of *F* and no `World_TimeOffset`, the clock
object is created with *G* = 0, and its creation day change sets the day of
*T* = `zero_offset` (in this install year 34, day 0, *n* = 12,240) with the day starting at
*t* = `zero_offset` / `day_length` = 0.3226. Because *U*prev is 0 at creation, the first
update after it adds *ρ* · *F* (the frame clock at that moment, times the rate) to *G*. So
*G* starts near 0 (and the game day near *t* = 0.3226 of day 12,240) only when the clock
object is created shortly after the client starts; created later, the clock starts
*ρ* · *F* seconds further on, and step 3 runs any day changes that this jump passes. Time
then runs at one game day per `day_length` / *ρ* real seconds (11,160 s = 3 h 6 min at
*ρ* = 1). When the live client creates its clock, and what it sets *F* and *O* to, is decided
outside the game-time module, so the live epoch stays OPEN (§10; owner item O21).

**Tessera Decision:** offline, Tessera chooses the epoch of *G* itself (for example from the
local clock, or a user-set time of day) because the original's live epoch is OPEN. A start
that reproduces the original without a connection is *G* = 0 at start-up (above).

<a id="s11-2"></a>
### 11.2 Choosing the day description

Inputs: the sky key *K* of the current track (outdoor sky key or indoor ambience key,
§7.1.1), the current day type *D* (§11.5; the sky reads it from the property
`World_DayType`, 0 until the clock has set it), the game day number *n*. (The first revision
gave *D* as "OPEN; initially 2": 2 is the default sky key, not a day type; corrected
2026-10-07.)

**Which sky description (BIN-E (b), E36).** The groups and the local default table of the
steps below are always those of **region 1's** sky description: the description named by
slot 4 of the region descriptor whose `region` number is 1 (§3.1, §3.2), the same lookup as
§11.5 step 1. This holds on both tracks and whatever region the viewer or the track is in;
the track's own region supplies only the light and sky intervals of §11.8 (§5.1.1). If
region 1 has no sky description, or it cannot be loaded, there is no day description. (In
this install every region names the same 0x15000007, so the rule changes nothing here; an
implementation must still not use the track region's own description for the pick.)

0. If *D* = 0 or *K* = 0 there is no day description (BIN-E (b), E20).
1. Walk the day groups in stored order. For each group with `day_type` = *D*, take its first
   sky entry (stored order) with `sky_key` = *K*, if any, and pick from that entry's weighted
   list with step 4. If the list is empty, its weight sum is ≤ 0, or the pick is the null DID
   (0), go on with the next group (BIN-E (b): the client does so; in this install each day
   type has one group and no group list is empty).
2. If no group gave a DID, take the first local default entry with `sky_key` = *K*; pick from
   its list with step 4. If that list is empty or its weight sum ≤ 0, there is no
   description (the client does not go on to a later default entry).
3. If neither exists, there is no day description (the client then uses the defaults of
   §11.3 step 8, BIN); **Tessera Decision:** draw a fixed sky colour as well.
4. **Weighted pick.** Let *W* be the sum of the list's weights; if *W* ≤ 0 there is no pick.
   Compute *h* from *n* with 32-bit wrap-around arithmetic: multiply *n* by 0x11A28787,
   subtract 0x6B3C5F11, multiply by *n* again, subtract 0x1B87D114, and keep the top 16 bits
   of the 32-bit result. Let *x* = *h* / 65536 × *W* (f32). Walk the pairs in stored order with
   a running sum of weights; pick the first pair whose running sum exceeds *x*; if none does,
   pick the last pair. (These constants are only needed to reproduce the original's choice of
   variant on a given game day; any deterministic per-day choice is acceptable otherwise.)
5. When a pick under the **same** sky key gives a different DID (the client re-picks under
   the same key only in a forced evaluation, §11.8), the new day description replaces the old
   one at once, without a cross-fade; the cross-fade happens only when the sky **key**
   changes (§11.6; BIN-E (f), E27). (The first revision described a cross-fade on every
   change of day description; corrected 2026-10-07.)

The pick depends only on *n* and *D*, so a sky keeps its weather variant for at least a whole
game day (in the original often longer, until the next re-pick of §11.8).

<a id="s11-3"></a>
### 11.3 Blending keyframes

Given the time of day *t* and a day description with keyframes *k*₀ … *k*ₙ₋₁ (ascending
`time`):

1. *a* = the first index *i* ≤ *n* − 2 with time(*kᵢ*) ≤ *t* ≤ time(*kᵢ₊₁*) (both comparisons
   inclusive); if none, *a* = *n* − 1.
2. *b* = the first index whose time is strictly greater than *t*; if none, *b* = 0.

   **Boundary behaviour (BIN-R (d), E14: these are the client's comparisons, confirmed
   2026-10-05).** The two searches are independent. When *t* equals the time of an interior
   keyframe *kⱼ* (0 < *j* < *n* − 1) and no earlier pair already contains *t*, step 1 stops at
   *a* = *j* − 1 (because time(*kⱼ*) ≤ *t* holds with equality) while step 2 skips *kⱼ* and
   finds *b* = *j* + 1. The client then blends *kⱼ₋₁* and *kⱼ₊₁* by
   (*t* − time(*kⱼ₋₁*)) / (time(*kⱼ₊₁*) − time(*kⱼ₋₁*)) and the values of *kⱼ* itself are not
   used at that exact instant; immediately before and after it the result is within rounding
   of *kⱼ*. An implementation that wants the original's output keeps these comparisons as
   they are; changing step 1 to a strict upper bound would differ from the original only at
   such instants (§14 has a worked example). Keyframe times that coincide exactly (several
   keyframes at 0.0, or at 1.0) are handled by the same rule.
3. Factor *f*, with *tₐ* = time(*kₐ*), *t_b* = time(*k_b*):
   - *tₐ* = *t_b*: *f* = 0;
   - *t_b* > *tₐ*: *f* = (*t* − *tₐ*) / (*t_b* − *tₐ*) when *tₐ* ≤ *t* ≤ *t_b*, else 0.5;
   - *t_b* < *tₐ* (wrapping past the end of the day): *u* = *t* − *tₐ*, plus 1 if negative;
     *s* = *t_b* − *tₐ* + 1; *f* = *u* / *s* (0 if *s* = 0).
4. Every scalar and every colour channel: value = *A* + *f* · (*B* − *A*), *A* from *kₐ*, *B*
   from *k_b*. The blended fog, sun, specular and ambient colours have alpha 1 (`colour_3`
   likewise; `colour_4` keeps its blended alpha).

   4a. **Curves (BIN-E (g), E32).** Each of the four float curves blends separately: the result
   takes the `mode` of *kₐ*'s curve, except that it takes *k_b*'s mode when *kₐ*'s mode is 1
   or *f* > 0.5; each of the ten parameters `w[0..9]` (`material.md` §5.3) blends linearly
   by *f* (a mode-1 curve contributes its constant as `w[0]`; Inferred: its other parameters
   count as 0). The blended curve is evaluated on the frame clock with phase offset 0
   (§11.7).
5. `sun_heading` and `sun_pitch` use **shortest-way angle interpolation** on a 360° circle:
   if going directly from *A* to *B* is shorter than going around, the result is
   *A* + *f* · (*B* − *A*) (not reduced); otherwise the client interpolates toward *B* ± 360
   and brings the result back into [0, 360).
6. The unit vector toward the sun is (sin *h* · cos *e*, cos *h* · cos *e*, sin *e*) with
   heading *h* and pitch *e* in degrees (world axes of `coordinates.md`, Z up). See
   `lighting.md` §8.1 for its use.
7. `effect` and `effect_extra` come from *kₐ* when `effect` ≠ 0 there and |*f* − 1| > 0.0002,
   else from *k_b* (BIN-E (g): the client switches to *k_b* when the factor is within 0.0002
   of 1).
8. With no day description the client uses the defaults of `lighting.md` §8.1, fog near 0,
   far 10,000, fog colour white.

<a id="s11-4"></a>
### 11.4 Animating sky objects

For each sky object at time of day *t*:

1. **Visibility.** Visible if `begin_time` = `end_time`; if `begin_time` < `end_time`, visible
   when `begin_time` ≤ *t* ≤ `end_time`; if `begin_time` > `end_time` (wrapping), visible when
   *t* < `end_time` or *t* > `begin_time`.
2. **Progress** *f*: as §11.3 step 3 with `begin_time` and `end_time` in place of the keyframe
   times. If `begin_time` = `end_time` the angles are the `…_begin` values.
3. **Angles A and B**, each from its (begin, end) pair: **A** from fields 5–6
   (`angle_a_begin`, `angle_a_end`) and **B** from fields 7–8 (BIN-E (e)). When `reverse` = 0
   and begin > end, subtract 360 from begin; when `reverse` = 1 and begin < end, subtract 360
   from end. Then angle = begin + *f* · (end − begin), plus 360 if negative. (In this install
   every sky object spans the whole day, 0 – 1, so angle A of, for example, −90 → 270 turns
   once per day.) An object that is not visible (step 1) gets no object to draw and its
   angles A and B are not recomputed: they keep the values of the previous update (0 for a
   newly created drawn object). Step 4 still runs for it, so if *kₐ* has an entry with a
   non-zero `replacement`, the replacement **is** drawn, with those kept angles plus the
   entry's offsets (BIN-E (e), E26; the offsets are then added again at every update while
   the object stays invisible). No entry of this install has a replacement or an offset, so
   here an invisible object is never drawn.
4. **Keyframe entries (BIN-E (e), E26).** Let *kₐ* and *k_b* be the keyframes of §11.3
   steps 1 and 2, and an object's entry in a keyframe the first object entry (§5.3.1) whose
   `object_index` equals the object's `index`.
   - If *kₐ* has an entry: when its `replacement` is not 0, the object drawn is the
     replacement (also when step 1 found the object not visible, step 3); its
     `angle_b_offset` is added to B and its `angle_a_offset` to A. *k_b*'s replacement and
     offsets are never used.
   - If *kₐ* and *k_b* both have an entry, the tint is the per-channel (alpha included)
     blend of the two entries' tints by the keyframe factor of §11.3 step 3.
   - Otherwise the tint is not updated: the drawn object keeps the tint it had at the
     previous update. The client keeps one drawn object per **position** in the day
     description's object list (not per `index`), creates it with an opaque white tint, and
     reuses it while the sky stays the same (a new day description under the same sky key
     keeps the drawn objects and their tints; a new sky starts with new ones, §11.6).
5. **Orientation (BIN-E (e), E25).** Wrap B into [0, 360) (remainder of division by 360,
   plus 360 if negative). The object's rotation quaternion, in the (w, x, y, z) order and
   with the Hamilton product and active convention of `coordinates.md` §11, is
   *Q* = *R_Y* ⊗ *R_Z*, where *R_Z* = (cos(−B/2), 0, 0, sin(−B/2)) is a rotation about +Z
   (up) by −B degrees (the identity when B = 0) and *R_Y* = (cos(−A/2), 0, sin(−A/2), 0) a
   rotation about +Y (north) by −A degrees (the identity when A = 0; A is not wrapped). A
   model point *v* is therefore first turned about the vertical axis by −B and then about the
   north–south axis by −A. The object's position in the sky's frame is (0, 0, 0). The drawn
   object receives `Render_Color` = (tint R, G, B, 1) and `Render_Opacity` = tint alpha × the
   sky's fade weight (1 outside a sky cross-fade, §11.6). Scale, camera centring and depth are
   OPEN (§12).

> **OPEN (implementer, 2026-10-06, `tessera-world` environment module):** step 4 does not
> say what happens when only one of *kₐ* and *k_b* has an entry for the object, nor which
> keyframe's `replacement` wins. Tessera (Decision): the side without an entry counts as
> untinted (white, opacity 1) with zero angle offsets, so tint and offsets are blended by
> *f* toward or away from those values; the replacement is *kₐ*'s when non-zero, else
> *k_b*'s. The keyframes' four float curves (§5.3 field 17) are not blended by Tessera
> while their use is OPEN.
>
> **Answered 2026-10-07 (BIN-E (e), (g)):** step 4 above (offsets and replacement from *kₐ*
> only; tint blended only when both have an entry, otherwise kept from the previous update)
> and §11.3 step 4a (curve blend). The Tessera Decision should be replaced by them.

> **OPEN (implementer, 2026-10-06):** the cross-fade of `lighting.md` §8.1 interpolates
> "every value linearly"; for `sun_heading` and `sun_pitch` that would sweep the long way
> round when the two states lie on either side of north. Tessera (Decision): angles take
> the shortest way of §11.3 step 5 during a cross-fade too, and the `effect` switches at
> *f* = ½.
>
> **Answered 2026-10-07 (BIN-E (f)):** the cross-fade does not blend angles. It blends the
> two states' unit light-direction **vectors** linearly and normalises the result (§11.6
> step 4), so no long-way sweep occurs; the `effect` is the old state's unless that is the
> null DID, then the new state's. The Tessera Decision should be replaced by §11.6.

<a id="s11-5"></a>
### 11.5 Day type (BIN-E (b), E19)

The game clock (§11.1.1) holds the current day type *D*, 0 when the clock is created, and
publishes it as the property `World_DayType`; each sky reads it from there (§11.2). At every
**day change** of §11.1.1, with the new game day number *n*:

1. Find the sky description of the region whose region number is 1 (its region descriptor's
   slot 4, §3.2; 0x15000007 in this install, shared by all regions; E36). If region 1 has no
   sky description, or it cannot be loaded, the new day type is **0**: skip steps 2–4 and go
   to step 5 (so a non-zero *D* becomes 0 and the change is published). (The first BIN-E
   text said *D* was left unchanged; corrected in the second 2026-10-07 revision.)
2. Walk the day groups (§5.1.2) in stored order. At the first group whose `day_type` = *D*
   and whose follow list (inline or named) has a weight sum *W* > 0, pick the next day type
   from that follow list: *h*₁ = the top 16 bits of the 32-bit wrap-around value
   (*n* × 0x3A535B70 − 0x6B3C5F11) × *n* + 0x11526F9F; *x* = *h*₁ / 65536 × *W* (f32); walk
   the (day type, weight) pairs in stored order with a running sum and take the first pair
   whose running sum exceeds *x*, else the last pair.
3. If no group qualifies (in particular while *D* = 0) and the sum *C* of all groups'
   `chance` is > 0, pick a group: *h*₂ = the top 16 bits of
   *n* × (*n* × 0x3FBB657F − 0x6B3C5F11) + 0x0719BAF0 (32-bit wrap-around); *x* = *h*₂ /
   65536 × *C*; walk the groups in stored order with a running sum of `chance`; the first
   whose running sum exceeds *x*, else the last group, gives the new day type (its
   `day_type`).
4. If *C* ≤ 0 the new day type is 0.
5. If the new day type differs from *D*, *D* takes it and the client publishes the change
   (old and new values) to the skies (§11.8).

Remarks:

- When the clock is created, its day change runs with *G* = 0, so the first day type is the
  step-3 pick for the day of *T* = `zero_offset` (in this install *n* = 12,240, which picks
  0x10000001, §14). If region 1's sky description is not available at that moment, *D*
  stays 0 (step 1).
- At any later day change that does not find region 1's sky description, *D* becomes 0 and
  the change is published. While *D* = 0 every sky has no day description (§11.2 step 0).
  The next day change that finds the description starts the chain afresh from the groups'
  chances (step 3, hash *h*₂), not from the type held before (provided no group has
  `day_type` 0; none has in this install).
- Every later day change applies step 2 to the previous day's type (a Markov chain over game
  days). The type of a given game day therefore depends on the chain since the clock was
  created: two clients whose clocks were created on different game days can hold different
  day types on the same day. With the install's follow lists the chain stays among the four
  group types.
- The hash constants are needed only to reproduce the original's sequence; any deterministic
  chain with these weights is otherwise acceptable.

<a id="s11-6"></a>
### 11.6 Indoor and outdoor tracks; sky-key cross-fade (BIN-E (c), (f), E22, E27)

**Two tracks.** The client runs the sky machinery twice per update: an **outdoor track** for
the outdoor sky key and an **indoor track** for the indoor ambience key (§7.1.1). Each track
holds its current key, a current sky (the key's day description by §11.2, its keyframe blend
by §11.3 and its sky objects by §11.4) and its own cross-fade, and composes an environment
state by §11.7; the interior overrides of §7.2 apply to the indoor track only. The client then
uses the outdoor track's state while the **landscape is visible** from the viewer, and the
indoor track's state otherwise. The landscape counts as visible when the viewer's position is
outdoors (the outdoor cell value, `coordinates.md` §7) or when the client's cell-visibility
test from the viewer's cell reaches the landscape (Inferred: interiors open to the outside).
Rain and snow intensities are published (`World_RainIntensity`, `World_SnowIntensity`) from
the outdoor state while the landscape is visible, else as 0.

**Cross-fade on a key change** (per track):

1. When the track's wanted key differs from its current key and no cross-fade is running, the
   current sky becomes the **old** sky and a **new** sky is created for the wanted key (it
   copies the day type and any forced day description of the old one). The start time is
   read from the timer's raw seconds *F*raw (the frame clock of §11.1.1 before any re-base;
   real seconds). While a cross-fade runs, further key changes wait until it ends.
2. If the update is marked immediate (Inferred: entering the world or a teleport), the track
   switches at once, without a cross-fade, and ends a running one.
3. The duration *T* is the new sky's current day description's `transition_seconds` if it
   exceeds 0.0002, else the old sky's if that exceeds 0.0002, else 8 seconds.
4. Each update: *s* = clamp((*F*raw − start) / *T*, 0, 1) and the weight *w* = ½ + ½ ·
   sin(π · (*s* − ½)). Both skies are evaluated and composed (§11.7) and the track's state is
   their blend: below *w* = 0.0002 the old state, above 0.9998 the new state, otherwise
   (1 − *w*) · old + *w* · new for every number and colour channel, except:
   - **sun:** the result has a sun if either side has one ("sun present", §11.7 step 9); the
     light direction is the linear blend of the two unit vectors, normalised (a side without
     a sun uses the other side's vector; a result shorter than 10⁻⁶ becomes (0, 0, −1)); a
     side without a sun counts as opaque black for the sun and specular colours and takes the
     other side's sun brightness;
   - **fog:** the result has fog if either side has; the fog colour of a side without fog is
     the other side's; a side without fog counts as near 999 and far 1,000; its `fog_extra`
     counts as 0;
   - **effect:** the old state's `effect` and `effect_extra` unless the old `effect` is the
     null DID, then the new state's.
5. The new sky's objects are drawn with opacity × *w* and the old sky's with × (1 − *w*)
   (`Render_Opacity`, §11.4 step 5), on the outdoor track while the landscape is visible.
6. When *F*raw − start ≥ *T*, the old sky is dropped and the new one becomes current.

**Sun-change fade (BIN-E (f), E34).** Independently of the tracks, when the "sun present"
flag of the state in use changes between two updates (for example going indoors, or a
keyframe `effect` starting), the client fades from the previous final state (kept fixed) to
the new one with the weight shape of step 4 over 5 seconds of the raw timer *F*raw; if
another change happens during that fade, the new fade starts from the state then shown and
its duration is the **smaller** of the running fade's duration and the time since the
previous change, and at least 0.0002 s (the two differ only when the change is seen on the
update at which the running fade would have ended; corrected 2026-10-08, E34). An immediate
update skips the fade. While no flag change happens, a running fade continues toward the
moving new state. The fade blends the two states field by field with the rules of step 4
(sun, fog and `effect` included).

**Switching between the tracks (BIN-E2 (a), E37).** The change from the outdoor track's state
to the indoor track's state, or back, has **no blend of its own**: on every update the client
takes the state of the track in use (by the landscape-visibility test above) and passes it to
the sun-change fade. So:

- a switch that changes "sun present" is faded over 5 s by the sun-change fade. Because the
  indoor state never has a sun (§11.7 step 9), this is every switch while the outdoor state
  has a sun: going indoors by day, the sun light, ambient terms, fog and the other values all
  blend from the outdoor values to the indoor ones over 5 s, and the result keeps a sun (with
  its colour fading toward black, step 4) until the fade ends;
- a switch that does not change "sun present" (the outdoor state has no sun: night keyframes
  with no sun light, a keyframe `effect`, `sun_brightness` below 0.0002) shows the new
  track's state in the same update, unless a sun-change fade is still running, which then
  continues toward the new state;
- an update marked immediate (step 2) shows the new state at once in every case.

The sky-key cross-fade of steps 1–6 belongs to each track and is not started by a track
switch.

<a id="s11-7"></a>
### 11.7 Composing the environment state (BIN-E (d), (g), E23, E24, E28)

For each sky, every sky update produces one state from the blended keyframe values
(§11.3):

1. **Local sources.** When the sky's render state carries a "local" flag, the client first
   mixes further local environment sources into the blended values (sources OPEN, §10); a
   local source can also force a weather mode (step 7).
2. **Sun:** the light direction (unit vector, the negated toward-sun vector of §11.3 step 6),
   the sun colour, brightness and specular colour.
3. **Ambient:** upper term = `ambient_colour_1` RGB × `ambient_brightness`, lower term =
   `ambient_colour_2` RGB × `ambient_brightness` (alpha unchanged); `lighting.md` §8.1 uses
   them. On the indoor track the overrides of §7.2 apply.
4. **`colour_3`:** R, G and B × 0.85, alpha unchanged.
5. **Fog:** on when the client's distance-fog setting is enabled (a setting of the client;
   Inferred: a user graphics option; its default was not traced); the sky itself always
   supplies fog (with no day description: near 0, far 10,000, white, §11.3 step 8); near,
   far and colour from §11.3. On the indoor track `InteriorFogType`,
   `InteriorFogNear`, `InteriorFogFar` and `InteriorFogColor` apply (§7.2). Limits, in this
   order:
   1. before the interior overrides: a far distance above 10,000 becomes 10,000;
   2. after them: a far distance below 20 becomes 20;
   3. near = max(−10,000, min(near, max(far − 20, −10,000))).
   `fog_extra` is kept only when fog is on and `fog_extra` > 0.001 (else it counts as 0).
6. **Precipitation:** rain = keyframe curve 1 evaluated at *F* (phase offset 0, `material.md`
   §5.3.1), clamped to [0, 1]; snow = curve 2, clamped to [0, 2]; two further values from
   curves 3 and 4, unclamped, and a fifth value 0.
7. **Weather override:** when a weather mode is forced (by a debug console command of the
   client with the modes rain, snow or none, or by a local source of step 1), rain and snow
   become (1, 0) for rain, (0, 1) for snow and (0, 0) for none, and the three further values
   of step 6 become 0. A fourth mode of the command removes the override.
8. **Carried values:** `colour_4` (with alpha), `values_a`, `value_b` (clamped to [0, 1]),
   `effect` and `effect_extra`.
9. **Sun present:** true on the outdoor track when the blended `effect` is the null DID, the
   sun colour is not opaque black (some channel differs from (0, 0, 0, 1) by more than
   0.0002) and the sun brightness is at least 0.0002; false otherwise and always false on the
   indoor track. Where it is false the renderer treats the sun as absent (`lighting.md`
   owns the consequences).
10. **Sky hidden by fog:** when the state in use has fog on and `fog_extra` > 0.99, the client
    does not draw the sky objects.

<a id="s11-8"></a>
### 11.8 Update cadence (BIN-E (c), (f), E30)

- **Position properties** (§7.1.1, §7.2) are read again when the viewer's landblock or cell
  changes, when more than 2.5 s of the timer's raw seconds (*F*raw, §11.6) have passed since
  the last read, or when the client forces it.
- **Sky evaluation** (per sky): the keyframe blend, the sky objects and the state of §11.7
  are recomputed when the frame clock reaches the sky's next light time, or at once when its
  key or region changes. After each evaluation the next light time is *F* + ½
  (`light_interval_min` + `light_interval_max`) / *ρ* (§5.1.1, §11.1.1; 1 s in this
  install). The two intervals (light here, sky below) are read, when the sky's region
  changes, from the sky description of the **sky's own region** (the track's region); this
  is the only use of that region's description, since the day type and the pick read
  region 1's (§11.2, §11.5; E36).
- **Day-description pick** (§11.2) happens only within an evaluation and only when the key
  or the region changed or the evaluation is forced. Forced evaluations are: the clock's
  "rate changed" flag (set when the clock is created and whenever *ρ* changes; cleared at the
  next sky refresh), and the day-type change notification of §11.5 step 5. That notification
  is itself an evaluation request: if it arrives before the sky's next light time, the
  request ends without effect (BIN; only the stored day type changes). So a new game day
  with an unchanged day type does not re-pick by itself, and a day-type change re-picks only
  when its notification finds the light time reached; otherwise the next pick happens at the
  next key or region change (a new sky, §11.6, always picks). **Tessera Decision:** re-pick
  at every key or region change and at every day change; this can differ from the original
  between key changes.
- **Sky refresh:** when the frame clock reaches the next sky time (*F* + ½
  (`sky_interval_min` + `sky_interval_max`) / *ρ*; 180 s here), or the evaluation was forced,
  or the clock's rate flag is set, or the day description changed, the client marks a refresh
  for the renderer (its effect was not traced) and clears the rate flag.

---

<a id="s12"></a>
## 12. Drawing the sky (Tessera Decision unless marked as a fact)

- Per frame: run §11.1, pick the day description with §11.2 when the sky key or the day
  changes (or every `sky_interval` seconds), and blend §11.3 every `light_interval` seconds
  (§5.1.1; 1 s in this install). (BIN-E: the client's exact cadence is §11.8; the two tracks
  and cross-fades are §11.6.)
- Feed the blended sun, ambient and fog values to [`lighting.md`](lighting.md) §8.1 and §10.
- Draw the visible sky objects (§11.4) around the camera before the world, without writing
  depth, each tinted by its keyframe tint (alpha = opacity). Their geometry comes from the
  0x47 objects like any physics object (`entity-descriptor.md` §4.3). The axes of angles A
  and B are a fact since BIN-E (§11.4 step 5: B turns the model about the vertical axis
  first, A then turns it about the north–south axis); this replaces the former Tessera
  Decision ("B is the azimuth of the object's arc and A the rotation along it"). The scale
  and whether fog applies are OPEN.
- Interiors: the indoor track (§11.6) with the interior fog and ambient overrides of §7.2
  and the limits of §11.7 (BIN-E; this replaces the first revision's simpler rule "cells
  with `InteriorFogType` Linear use their own fog values").

**Facts from BIN-E (e), (g) about drawing:**

- Each sky object becomes one drawn entity of the object's 0x47 DID, created in the order of
  the day description's object list. Each new sky takes the next block of 100 numbers from a
  global counter (E35), and each object is registered with the renderer under the sky's number +
  its position in the list (Inferred: a draw-sort key). Whether lower numbers draw first is
  OPEN.
- The entity's colour and opacity come from the properties `Render_Color` and
  `Render_Opacity` (§11.4 step 5); a sky cross-fade scales the opacity (§11.6 step 5).
- When fog is on and `fog_extra` > 0.99 the sky objects are not drawn (§11.7 step 10). How
  smaller `fog_extra` values fog the sky is OPEN (`lighting.md` owns the fog formula).
- A debug console command of the client forces the precipitation values (§11.7 step 7);
  Tessera can offer an equivalent debug switch under its own name.

> **OPEN (implementer, 2026-10-06, `tessera-world` environment module):** what
> `InteriorFogType` "None" (2) and "Invalid" (0) do: "no interior override" (the outdoor
> keyframe fog of the sky key stays in force) or "no fog at all" indoors. Tessera
> (Decision): no override; the stored near, far and colour are used only for "Linear".
>
> **Answered 2026-10-07 (BIN-E (d), E23):** "no fog at all". Where `InteriorFogType` is
> present at the viewer's position, the indoor track's fog is on only for 1 (Linear); 0, 2
> and any other value turn it off. The stored near, far and colour override independently
> of the type, each only where present (§7.2). Where `InteriorFogType` is absent, the indoor
> ambience key's own keyframe fog stays on. Note that indoors the fog comes from the
> **indoor ambience key's** day description, not from the outdoor sky key (§11.6). The
> Tessera Decision should be replaced by §7.2.

---

<a id="s13"></a>
## 13. Invariants

| # | Invariant | Status |
|---|---|---|
| EV1 | 0x15000007: `version` = 1; parses to the last byte with §5.1 | Confirmed |
| EV2 | Every 0x1A record parses to the last byte with §5.2–§5.4; keyframe times ascending; every u8 flag is 0 or 1; sky-object indices unique within a record | Confirmed, 963 of 963 |
| EV3 | Every DID in a sky-description weighted list resolves to a 0x1A record | Confirmed |
| EV4 | `effect_extra` is present exactly when `effect` ≠ 0 | Confirmed (by full parse) |
| EV5 | 0x35000000 parses to the last byte with §6; period starts ascending in [0, 1); month start days ascending | Confirmed |
| EV6 | Sky-object DIDs are 0 or prefix 0x47; keyframe `effect` DIDs are 0 or prefix 0x47 | Confirmed |
| EV7 | With this install's 0x15000007 and 0x35000000, a clock created with *G* = 0 sets the day type 0x10000001 (§11.5 step 3 for *n* = 12,240, *h*₂ = 47,931) | Computed 2026-10-07 (BIN formula, OBS inputs) |
| EV8 | §11.5 steps 2–3 applied to every game day of years 34–100 from any of the four group types give one of the four group types | Computed 2026-10-07 |
| EV9 | §11.4 step 5 maps the model point (1, 0, 0) with B = 0 to (cos A, 0, sin A): east at A = 0, zenith at A = 90, west at A = 180, nadir at A = 270 | Algebraic (§14) |

Hostile-input rules: reject unknown float-curve modes (their length is unknown), flags ≥ 2,
counts larger than the bytes left, and a sky-description `version` other than 1.

> **OPEN (implementer, 2026-10-06, `tessera-world` environment module):** no rule covers
> non-finite floats in day descriptions (keyframe times, fog, sun and ambient values,
> `values_a`, `value_b`; sky-object times and angles; keyframe-object angle offsets). Should
> the decoder reject them, as `water.md` §5.4 does for fog-zone entries? Tessera (Decision,
> until answered): the decoder accepts them; the blender skips keyframes whose time is not
> finite, replaces any non-finite blended or cross-faded value by its §11.3 step 8 default,
> hides a sky object with a non-finite time and counts its non-finite angles as 0.
>
> **Answered 2026-10-07 (BIN-E (i), E29):** the client's keyframe and sky-object readers do
> not check floats for finiteness (and no stored value is non-finite in this install), so
> the original has no rule to copy. The Tessera Decision stands as Tessera's own hostile-input
> rule.

> **OPEN (implementer, 2026-10-07, `tessera-explorer` sky timelines):** no bound is stated
> on a day description's keyframe count (at most 21 observed, OBS2; the decoder bounds it
> only by the record's length), and the §11.3 search visits every keyframe at every time
> sampled. Tessera (engineering choice, explorer only): a timeline blends among the first
> 256 keyframes with a finite time and shows a longer (hostile) record as that shorter day;
> `tessera-world` evaluates every keyframe.
>
> **Answered 2026-10-07 (BIN-E (i), E29):** the client bounds the keyframe count, the
> sky-object count and the entry counts only by the bytes left in the record (a count larger
> than the remaining byte count is an error); there is no fixed maximum. Tessera's 256-keyframe
> limit for the explorer is an engineering choice and stays.

---

<a id="s14"></a>
## 14. Worked examples (synthetic or computed from the formulas)

**Clock.** *G* = 0 with this install's game time: *T* = 3,600; *Y* = 360 × 11,160 =
4,017,600; *i* = 0, year 34; *d* = 0; *t* = 3,600 / 11,160 = 0.3226; *n* = 360 × 34 + 0 =
12,240.

**Weighted pick.** For *n* = 12,240 the hash gives *h* = 39,996, so *h* / 65536 = 0.6103. A
list with weights (2, 1) has *W* = 3 and *x* = 1.831; the running sums are 2 and 3; 2 > 1.831,
so the first DID is picked. For *n* = 12,241, *h* = 39,042, *x* = 1.787: again the first.

**Keyframe factor.** Keyframes at 0.2 (fog far 1,200) and 0.31 (fog far 2,500), *t* = 0.25:
*f* = 0.05 / 0.11 = 0.4545; fog far = 1,200 + 0.4545 × 1,300 = 1,790.9.

**Keyframe boundary (§11.3, the client's behaviour).** Keyframes at times 0, 0.2, 0.31 and 1.0
with fog far 1,000, 1,200, 2,500 and 1,000:

| *t* | *a* | *b* | *f* | fog far |
|---|---|---|---|---|
| 0.19999 | 0 | 1 | 0.99995 | 1,199.99 |
| 0.2 exactly | 0 (0 ≤ 0.2 ≤ 0.2) | 2 (first time > 0.2) | 0.2 / 0.31 = 0.6452 | 1,000 + 0.6452 × 1,500 = 1,967.7 |
| 0.20001 | 1 | 2 | 0.00009 | 1,200.12 |

**Angle.** Heading 350 → 10 with *f* = 0.5: the direct way (340°) is longer than the wrap
(20°), so the result is 350 + 0.5 × 20 = 360, reduced to 0.

**Sun direction.** Heading 15, pitch 150 (the most common keyframe values): toward-sun vector
(sin 15° · cos 150°, cos 15° · cos 150°, sin 150°) = (−0.224, −0.837, 0.5).

**Sky object.** Angle A from −90 to 270, `reverse` 0, *t* = 0.25 over 0 – 1: *f* = 0.25,
A = −90 + 0.25 × 360 = 0.

**Sky-object orientation (§11.4 step 5).** With B = 0, *Q* = *R_Y*(−A) and the model point
(1, 0, 0) goes to (cos A, 0, sin A): A = 0 → (1, 0, 0) (east horizon), A = 90 → (0, 0, 1)
(zenith), A = 180 → (−1, 0, 0) (west), A = 270 → (0, 0, −1) (nadir). With B = 75 and A = 90:
*R_Z*(−75°) takes (1, 0, 0) to (0.2588, −0.9659, 0), and *R_Y*(−90°) then takes that to
(0, −0.9659, 0.2588): south of the zenith, 15° above the southern horizon. (Which model
points the sky objects' geometry uses is data; these are only checks of the rotation.)

**Day type (§11.5).** Groups in stored order: 0x10000001 (`chance` 15), 0x10000008 (2),
0x10000004 (2), 0x1000000B (1); *C* = 20. Clock created with *G* = 0: *n* = 12,240,
*D* = 0 matches no group, so step 3: *h*₂ = 47,931, *x* = 47,931 / 65,536 × 20 = 14.627;
running sums 15, … → the first group, *D* = 0x10000001. Day 12,259 with *D* = 0x10000001,
whose follow list is (0x10000001, 15), (0x10000008, 1), (0x10000004, 1), (0x1000000B, 1),
*W* = 18: *h*₁ = 61,131, *x* = 16.790; running sums 15, 16, 17 → the third pair,
*D* = 0x10000004. (The chain from the creation day stays at 0x10000001 for days
12,241–12,258.)

**Sky-key cross-fade (§11.6).** New description `transition_seconds` 0, old 2.0 → *T* = 2 s;
both 0 → *T* = 8 s. At *F*raw − start = 2 s with *T* = 8: *s* = 0.25,
*w* = ½ + ½ · sin(−π/4) = 0.1464.

**Fog limits (§11.7 step 5).** Outdoor keyframe fog near 200, far 25,000 → far 10,000, near
200. Interior near 500, far 10 → far 20, near = max(−10,000, min(500, 0)) = 0.

---

<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First look (§0–§10). |
| 2026-10-05 | §1 provenance extended (OBS2, BIN). §2: prefix ↔ type pairings for 0x15, 0x1A, 0x35; day-type and sky-key mappers. §5 rewritten: sky description (§5.1), day description (§5.2), keyframe (§5.3), sky object (§5.4), coverage (§5.5), colours (§5.6), all Confirmed to the last byte; the first-look readings of §5.2 (keyframe count at offset 8, fog distances at 16 and 20) are confirmed and extended. §6 rewritten with the full game-time layout. §10 updated. New §11–§15. §3, §4, §7, §8, §9 unchanged. |
| 2026-10-05 (second revision) | Sweep against `scenery.md`, `terrain-texturing.md` and the new `water.md`, with new BIN facts (BIN-R, OBS3, E11–E15). §2: complete prefix ↔ DB type table for this spec's records (from the client's DID-range registration; full table in `records.md` §2.1). §3 rewritten: the region descriptor's full grammar (fixed fields, nine DID maps, trailing terrain-type-table DID), offset 8 is a map-image DID (not an f32), offsets 16–32 are the map-coordinate parameters with the client's formula, version rules. §4 rewritten: every per-region table's kind and the spec that decodes it; the first-look hypotheses for 0x11, 0x12, 0x16, 0x17 and 0x19 corrected (0x17 holds no texture list). §5.1.4: the named-list counts are VLEs (OPEN closed). §7.4: water note. §8 replaced by a pointer to `water.md`. §9: kinds of 0x02, 0x08, 0x2D, 0x32, 0x39, 0x44; new §9.1 LOD-class table. §10 updated. §11.3: the keyframe-search comparisons confirmed by BIN, boundary behaviour stated; §14 boundary example. Mark "Tessera Decision" added to the legend. |
| 2026-10-05 (third revision, after review) | Status Draft → Review. §1: source tag LC defined (cited by E13 and §3.1 field 3). §5.3: `fog_near` and `fog_far` in world units (one unit is by convention a metre, `coordinates.md` §3). §9.1: stale pointer to `scenery.md` §9 replaced by a cross-reference; this section named the single owner of the 0x32 layout (payload 1,160 bytes: DID, count, 32 × 36 bytes; re-checked on the install); correction note for `scenery.md` §9.1 and SC9 (`length`, `did_echo` and the 1,168 figure to be removed). |
| 2026-10-06 | Alignment with [`walker.md`](walker.md) §11.5, no other change; status stays Accepted. §9: the 0x08 row no longer calls the camera-state layout OPEN; it points to `walker.md` §11.5 and replaces the first-look reading ("a 0x47-like DID, then floats (0.15, 0.1, 2000.0)") with the decoded header (camera entity DID, collision smoothing time, near clip, far clip; 0.15 was the smoothing time of one record only). §10: 0x08 removed from the last OPEN. §1: OBS4 and E16 added (11 of 11 records re-checked against `walker.md` §11.5.1 and the per-type record sizes). |
| 2026-10-07 | OPEN-closing revision with a third disassembly pass (BIN-E) and property labels (PR); status Accepted → Revised, waiting for review. Section numbers and anchors unchanged; new §7.1.1, §10.1, §11.1.1, §11.5–§11.8 and invariants EV7–EV9. **Corrections:** the day type does not "start at 2" (2 is the default sky key; §11.2, §10); a cross-fade follows a change of **sky key**, not of day description, and its default is 8 s (§5.2, §11.2 step 5, §11.6). **Added facts:** game clock source, rate, offset and the consequence for the epoch (§11.1.1, E17); night start and end, period, date-text numbers (§6, E18); the day-type chain (§11.5, E19); group fall-through and *D* = 0 or *K* = 0 in the day-description pick (§11.2, E20); sky-key and indoor-ambience precedence and the position lookup (§7.1.1, E21); the indoor and outdoor tracks (§11.6, E22); interior fog and ambient overrides (§7.2, E23); fog limits (§11.7, E24); sky-object axes and keyframe-entry rules (§11.4, §5.4, E25, E26); the cross-fade rules (§11.6, E27); curve blend, rain and snow curves, `fog_extra` hiding the sky, `effect` switch near *f* = 1 (§5.3, §11.3, §11.7, E28); no finiteness checks, counts bounded by bytes left (§13, E29); update cadence (§11.8, E30); older keyframe and sky-object versions (§5.3, §5.4). **Implementer OPENs answered:** §7.1 (key precedence), §10 (group-only keys), §11.4 (entries; cross-fade angles), §12 (`InteriorFogType`), §13 (non-finite floats; keyframe bound). **Owner items:** O21, O26 and the fog part of O22 answered or narrowed in §10.1. **Remaining OPEN:** §10. |
| 2026-10-07 (second revision, after review) | Answers the review of 2026-10-07 (changes requested); status stays Revised, now waiting for re-review. No section number or anchor changed. **Corrections (BIN-E, re-read for the review):** §11.5 step 1: when region 1 has no sky description or it fails to load, the new day type is 0 (published if *D* was non-zero), not "unchanged"; remarks of §11.5 updated (the chain restarts from the chances). §11.4 step 3 and step 4: a non-zero `replacement` in *kₐ*'s entry is drawn even when the object is outside its visible interval, with the kept angles plus the offsets. §11.1.1: the first update after the clock's creation adds *ρ* · *F*, so the start near *t* = 0.3226 of day 12,240 holds only for a clock created shortly after start-up. **Added fact:** §11.2 and §11.8: the day-description pick reads region 1's sky description on every track; the track's region supplies only the light and sky intervals (E36). **Provenance:** BIN-E row gains questions (j) (older reader versions) and the clock defaults under (h), and a note on this revision's re-check; new rows E31 (game-time defaults, §6), E32 (curve-blend mode rule, §11.3 step 4a), E33 (older keyframe and sky-object versions, §5.3, §5.4), E34 (5 s sun-change fade on the raw timer, §11.6), E35 (blocks of 100 numbers, §12), E36 (region 1's sky description); E17, E19, E21, E24, E26 and E28 extended to cover every BIN-E fact stated inline in §7.1.1, §11.1.1, §11.3, §11.4 and §11.7. **Removed:** the two console names taken from the client binary (§11.7 steps 5 and 7, §12), replaced by plain descriptions; the names are also redacted from the review record's note (b). |
| 2026-10-08 | Revision (spec writer; M1 census OBS6 and a fourth disassembly pass BIN-E2 under decision 0011); status Accepted → Revised, waiting for review. No section number or anchor changed. **Correction:** §5.3 field 6 census: 0.0 occurs in 557 keyframes, not 558; the 558th stores 0.0001, which the 2026-10-05 census rounded to 0 and which matters because it is below the 0.0002 "sun present" limit (E38). §11.6 sun-change fade: a change during a running fade takes the smaller of the running duration and the time since the previous change (E34; the re-review's note (a), confirmed by BIN-E2 (b)). **Added:** §11.6 "Switching between the tracks": the outdoor ↔ indoor switch has no blend of its own and is smoothed only by the sun-change fade, so it fades over 5 s exactly when it changes "sun present" (E37; answers what `lighting.md` §8.1 and `landblock.md` §4.13 mean by an "immediate" switch); the sun-change fade uses the field rules of step 4. §5.4 and E26: field 22 is copied to the drawn object, field 15 is not (E39, the re-review's note (b)). **Provenance:** tags BIN-E2, OBS6; rows E37–E39; E26 and E34 extended. **Implementer OPENs:** the only one dated 2026-10-07 or later (§13, explorer keyframe bound) was already answered; no new one. |
