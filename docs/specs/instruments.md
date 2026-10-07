<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Player-music instruments: sample records, note mapping, sustain and sample choice

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested (first round; issues listed in the repair change-log row). Re-review 2026-10-05 after the repair round: changes requested. All first-round issues are resolved: `sound.md` points (a)–(d) are handled (Q8 callout and §15 row; I38; §10.5 flag condition; §9.1/§9.3 gates); one packed-note rule (I37); callouts Q1–Q9 at each unknown; no quoted UI label; rows I30–I36 added. One remaining issue: a Confirmed fact that does not hold. Script 0x1000274C of table 0x07000078 has **two** fire-and-forget Sound nodes on channel 1 that name instrument-set sound infos, 0x2A00A826 **and 0x2A00A824**. So I31 and §6.3 ("one `Fiddle_long` effect") are incomplete, and §4.2 ("the one unused one") is wrong. 0x2A00A824 is only unreferenced by the two music scripts, which is what §6.2 correctly says. Re-review spot checks that matched (independent probes over all records): dictionary rows of I30 (`Script_MaxNumInstances` int, default 1, minimum 1; `Script_IntensityMultiplier` float, default 1.0, limits 0–1; `PhysObj` and `Script_Item_PhysObj_Ranged` DID; `Item_EquipmentCategory` 64-bit bitfield on 0x2300004C; `Music_Cached_Instrument_Type` enum 0x230000FC). I42: 632,847 nodes in 7,524 tables and 28,992 scriptlets, and exactly 7 with variance 0 and a multiplier outside 0–1 (3 Sound, 3 Particle, 1 kind 17; 1.3–2 and 400,000). I31 scriptlets: 0x0C005BA3 and 0x0C0072B2 have 11 Sound nodes each, 6 distinct `Drum` sound infos, channel 8. IN15/I34: all 1,077 typed and 1,097 untyped `Instrument` items carry a `PhysObj` (prefix 0x47). I35: the labels of mapper 0x230000EE for keys 17–19; `Instrument` is key 7 of 0x2300004C, which is bit value 0x40 and agrees with the key − 1 rule. I39 and §4.2: 746 sound infos, flags 4 (734) and 5 (12), minimum distance 1 m for 74 and 3 m for 672, maximum distance 30 m for all. Sound-node counts: 4,760 in 0x0700042F and 2,120 in 0x070017F9. The arithmetic of §10.2, §10.5 and §14 also matched. **Re-review 2026-10-06 (third round), Claude review session (spec-writer agent): changes requested.** The second-round issue is fixed: I31, §4.2 (lead-in of the 746-set table), §6.2 and §6.3 now name both Sound nodes of script 0x1000274C of table 0x07000078 (channel 1, fire-and-forget, 0x2A00A826 and 0x2A00A824), and 0x2A00A824 has no other user. One remaining issue, a Confirmed fact that does not hold: §4.4 (I26) says the `Bagpipe` cases of the older copy 0x070017F9 "name other sound infos". An independent probe over every Sound node of both music scripts in both tables, keyed by (script key, branch, instrument case, note case), found that all 2,120 Sound nodes of 0x070017F9 name the same sound info as the matching case of 0x0700042F, and that channel, `FireAndForget` and `PitchVariance` also match. The only difference is that the 222 `Bagpipe` Sound nodes of 0x070017F9 have `RampOutTime` 0, where 0x0700042F has 0.2. The older copy's 335 distinct sound infos are all in the 746 instrument set. Fix §4.4 and I26 to say this and add a change-log row. The fact does not affect players, because no avatar closure reaches 0x070017F9, but a Confirmed mark must hold. Spot checks that matched (independent probes over all records, 2026-10-06): (1) I31: other users of the 746 instrument sound infos, outside both music-script copies, are only 0x07000078/0x1000274C (two Sound nodes) and scriptlets 0x0C005BA3 and 0x0C0072B2 (11 nodes each, 6 distinct sound infos in the `Drum` set, channel 8), across 7,524 tables and 28,992 scriptlets. (2) §4.1 / I21: table 0x0700042F has kind 2, no includes and 292 scripts; 0x10000BCD has 2,539 nodes (78/68/2,324/69) and 0x10000BCE has 2,663 (82/71/2,436/74); the DID lists are 745 + 14 and 745 + 15; `Script_MaxNumInstances` is 10 in both scripts of both tables. (3) §4.2 / I8 / I39: 45,544 records with no leftover bytes, `did` equal to the record DID everywhere, the flags histogram as stated, name lengths 3–84, every `sound` prefix 0x0A, and the 746-set table exactly (660 / 37 / 37 / 12). (4) §3.1, §3.2, §3.3, §3.5, §7.1: mapper labels, base mappers, script-key labels and the 37-entry note-key table. (5) §4.3 / I10 / I36: 744 Ogg Vorbis (mono, 44,100 Hz, trailer 2) and 2 RIFF WAVE (PCM, mono, 16-bit, `fmt `/`PAD `/`data`, trailer 3), with durations 0.25–10.23 s. Also matched: §6.1 `FireAndForget`, `RampOutTime` and `PitchVariance` per instrument; §6.2 note counts (33 `Fiddle_long`, 36 `Horn` song branch of 0x10000BCD, one default node with ramp 0 and `PitchVariance` 0.2). All other §5.3 checklist items pass: no code or pseudo-code, no copied prose, no game text, complete byte layouts with units, testable invariants, unknowns marked OPEN (Q1–Q9), and no closed gate. Non-blocking note: the marks Probable and Measured go beyond the Confirmed/Reported pair of CLEAN_ROOM.md §5.1. The legend defines them and the earlier rounds accepted them. **Re-review 2026-10-06 (fourth round), Claude review session (spec-writer agent): passed; status Accepted.** The third-round issue is fixed: §4.4 and I26 now say that the older copy 0x070017F9 names the same sound info as 0x0700042F case for case, and that `RampOutTime` on the 222 `Bagpipe` nodes is the only Sound-node field difference. A change-log row records this. Spot checks that matched (independent probes over all 6,880 Sound nodes of both music scripts in both tables, 2026-10-06): (1) I26: every one of the 2,120 Sound nodes of 0x070017F9 has a matching case in 0x0700042F, keyed by (script key, branch, instrument case, note case), and names the same sound info. (2) I26 / §4.4: comparing the whole Sound-node payload and base block of every pair, the only difference is the `RampOutTime` float on the 222 `Bagpipe` nodes (0.2 in 0x0700042F, 0 in 0x070017F9). (3) §4.4 table: the path products of the intensity multipliers differ on exactly 1,110 Sound nodes, for `Flute` (0.65 / 0.70), `Lute` (0.34 / 1.0), `Theorbo` (0.455 / 1.0), `Bagpipe` (0.65 / 0.70) and `Pibgorn` (0.70 / 0.175 from 0.7 × 0.25; 0.10 from 0.4 × 0.25 in the `Music_Note_From_Song` = 1 branch of 0x10000BCE). The other instruments and the default case of 0x10000BCE match, and the velocity-driven multiplier is not part of the products. (4) §6.1 Intensity column: for every instrument key 1–26 and the default case of 0x10000BCE in 0x0700042F, the path product equals the stated value, and at most one constant non-1 multiplier lies on each path. (5) The Sound-node counts are 4,760 in 0x0700042F and 2,120 in 0x070017F9. All CLEAN_ROOM.md §5.3 items pass: functional facts only, no code or pseudo-code, no copied prose, no game text or assets, a provenance mark on every fact, complete byte layouts with units, testable invariants, unknowns marked OPEN (Q1–Q9), and no closed gate. This review did not re-derive the BIN and BIN-R facts. Non-blocking note (unchanged): the marks Probable and Measured go beyond the Confirmed/Reported pair of §5.1; the legend defines them. |
| **Implements into** | `crates/formats` (sound-info record 0x2A, §4.2, unless `sound.md` lands it first; the Sound node fields are already framed by `script-table.md` §5.3), `crates/audio` (`tessera-audio`, M6: instrument table built from the script data §5.7, playback queue and playing-note registry §8, voices with release fade §9, loudness §10), `apps/client` (music mode, `/play`), `apps/dattool` or `apps/explorer` (instrument table listing, audition of one note), `fuzz/` (the 0x2A decoder; the instrument-table builder fed with arbitrary script tables) |
| **Companion specs** | [`abc-dialect.md`](abc-dialect.md) §7.11 (volume values), §8 (time line), §9 (note events, pitch slots). [`script-table.md`](script-table.md) §2 (table framing, per-script property sets), §4 (node frame, base block, ports), §5.3 (Sound node fields), §7.1 (Switch), §8.1–§8.2 (an entity's tables and their closure). [`records.md`](records.md) §2.1 (prefix 0x2A), §8 (DID maps), §9 (0x0A sound payloads). [`property-system.md`](property-system.md) §3–§5 (value types, property sets), §8 (enum mappers). [`sound.md`](sound.md) (in Review; normative for the generic sound engine): §4.1 and §4.3 (sound-info layout and flag bits), §6.1 (gain chain), §6.2 (distance law), §6.5 (ramp-out), §6.6 (voice limit), §6.7 (listener and panning), §6.8 (requests that are not played), §7 (channels and volume settings), §9.2 and §9.3 (Sound node start and node intensity). Points where the two specs disagree are listed in §10.1 and §15 and are open as Q8. [`keymap.md`](keymap.md) (live-play key actions). [`install-layout.md`](install-layout.md) §8.3 (`[Sound]` keys). [`appearance.md`](appearance.md) §5 (where instruments are held). |

---

<a id="s0"></a>
## 0. Purpose

This spec lets an implementer make a character play the original's instrument sounds from
the user's own data:

- which records hold the samples of every instrument and every note (§4);
- how the original chooses the sample for a note: the inputs of a note, the script key, the
  decision tree stored in the data, and the per-instrument facts that tree yields (§5, §6);
- how written pitch relates to the samples (§7);
- the playback queue: timing, late notes, the registry of sounding notes and the limit on
  simultaneous notes (§8);
- sustain, release and decay per instrument (§9), loudness and position (§10), and the
  note range per instrument (§11);
- Tessera's design decisions (§12), invariants (§13), synthetic examples (§14),
  corrections to other specs (§15) and open questions (§16).

Out of scope: reading ABC files and building note events (`abc-dialect.md`); how a
performance reaches other players (network, decision 0004); the generic sound engine
beyond what instruments need (`sound.md` §3–§9); instrument models, hand placement and playing
animations (`appearance.md`, `skeleton-anim.md`); the visual note effects that the same
scripts emit (§5.5 names their DIDs only).

All counts are observations of one install. Enum keys, DIDs, PIDs and node fields are data
and must be read at run time. Numbers that define the client's behaviour (the 0.5 s late
limit, the 0.0001 s tie tolerance, the divisor 127, the pitch formula, the 10 ms fade step,
the 0.005 audibility floor, the live-play velocity 90, and the engine values taken from
`sound.md` such as the 0.0002 volume gate and the default voice limit of 32) come from the
client executable and may be used as constants
([decision 0012](../decisions/0012-numeric-constants.md)).

Property names, enum labels (`Lute_of_Ages`, `Note_C2`, `PlayerMusic` …), script-key labels
and the DID-map label `MusicControl` appear because they are stored in the data and an
implementation looks values up by them (interoperability). No game text and no sample names
appear; samples are referred to by DID.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: observed in the user's install on 2026-10-05 by throwaway probes in the
  dirty-room scratchpad, over **all** records of the kind unless a sample is stated.
- **Confirmed (BIN)**: read from the client executable under decision 0011; see the BIN
  row. **Confirmed (BIN + OBS)**: read from the executable and agreeing with the data.
- **Measured**: computed from the decoded audio of the samples (MEAS row).
- **Probable**: supported by evidence that was not traced to the end.
- **Reported**: published material only.

| Tag | Source |
|---|---|
| OBS | User install, Steam, data current on 2026-10-05. Read in full: all 7,524 decodable script tables and 28,992 scriptlets of `client_gamelogic.dat` (the two music scripts of §4.1 node by node, every Sound node of both; the sound info of every Sound node and the intensity fields of all 632,847 nodes, for I31 and I42); all 45,544 sound-info records (prefix 0x2A, `client_general.dat`); the 746 sound payloads (prefix 0x0A, `client_sound.dat` and `client_sound_aux_1.datx`) named by the player-music sound infos; all 383,332 object property sets (0x79) for the instrument properties and `PhysObj`; enum mappers 0x230000FC, 0x2300015E, 0x2300034D, 0x2300034E, 0x230000AF, 0x23000097, 0x2300004C; DID map 0x2800000E; the `[Sound]` section of the owner's `UserPreferences.ini` (key names and values; the owner's settings folder has no `Music` folder). |
| MEAS | Audio analysis in the dirty room: every one of the 746 payloads decoded to mono 32-bit float PCM with a general-purpose decoder; fundamental period estimated with a YIN-style detector (cumulative mean normalised difference, threshold 0.15, 25–2,500 Hz) on a 16,384-sample window starting 0.15 s into the sample (a quarter of the way in for short samples); result compared with the note of the switch case that plays the sample. A harmonic-sum spectral check was also run; it is biased towards octave errors and was used only to flag outliers. |
| BIN | Client executable `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`), read with capstone on 2026-10-05 under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md). **Questions:** (1) how a note event becomes a sound: inputs, parameters, script key, the meaning of the two music scripts; (2) how a pitch becomes a `Music_Note` key; (3) what the Sound node fields `FireAndForget`, `RampOutTime`, `Pitch`, `PitchVariance`, `Channel` and the base fields `UseParentIntensity`, `IntensityMultiplier`, `IntensityMultiplierVariance` do; (4) note-on/note-off scheduling, late events, re-struck notes, limits on simultaneous notes; (5) the sound-info record decoder and its loop flag; the release fade; (6) where a character's instrument type comes from. **Why M1–M3 were not enough:** the data holds the decision tree and the samples (M1) but not the rule that picks the script key, the meaning of the node fields, the timing rules or the fade; no published source describes them (M2); the owner has made no recordings and has no music files (M3). **Read:** the player-music playback module (note queue, note-on, note-off, registry, cancel), the sequencer's pitch-to-slot conversion and default velocity (also covered by `abc-dialect.md`), the Sound node start function and its running instance, the generic node-start intensity rule, the script-run instance check, the sound-info constructor and decoder, the sound engine's voice creation, distance attenuation, stop and fade functions, and the equip handler's copy of the instrument type. Functions that hand note events to the module from outside it were seen only as callers; their message formats were **not** examined (decision 0004). Nothing about network, login, keys, launcher or anti-tamper. |
| BIN-R | Second M4 pass, same build, 2026-10-05, under decision 0011, to answer a spec review. **Questions:** (7) how many bits of the packed note form the note key, at note-on, in the registry and at note-off; (8) whether a Sound node's own `IntensityMultiplier` enters its volume, and when the node intensity is clamped; (9) the constructor defaults of the sound-info and Sound-node objects; (10) what the equip handler copies besides the instrument type; (11) what the live-play key handler puts into a note. **Why M1–M3 were not enough:** the data holds no packed notes and no run-time intensities (M1); no published source covers these points (M2); no running client is available to this session (M3). **Read:** the note-on and queue-processing functions of the playback module, the two note-packing functions (sequencer and live), the Sound node's start function and constructor, the generic node-start and intensity-update functions and the multiplier-factor helper, the sound-info constructor, the instrument part of the equip handler, and the live-play key handler. The live-play handler and the sequencer hand their events to a dispatch function that leaves the playback module; it was **not** followed (it may lead to the network layer, decision 0004). Nothing about network, login, keys, launcher or anti-tamper. |
| SND | [`sound.md`](sound.md) (Review, 2026-10-05): its own OBS and BIN rows (same client build). Cited for engine rules this spec does not re-derive. |
| MA | Maestro / ABC Player (`github.com/NikolaiVChr/maestro`, MIT, clone at commit 0440b25), studied for facts only: its per-instrument notes on octave shift, sustain and sample length, and its note that one instrument plays effect sounds on its lowest notes. No code, names, tables or prose copied. |
| ABC | [`abc-dialect.md`](abc-dialect.md) (pitch numbers 48–84, volume values, note events, pitch-slot table). |
| ST | [`script-table.md`](script-table.md) (node framing, Switch semantics, entity tables and closures). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| I1 | Instrument-type enum 0x230000FC: keys 0–12 and 16–26 with the labels of §3.1 | OBS | Confirmed |
| I2 | Note enum 0x2300015E: key 0 plus 37 labels `Note_C2` … `Note_C5` (§3.2) | OBS | Confirmed |
| I3 | Pitch 48–84 becomes a `Music_Note` key through a fixed table that equals the label rule of §7.1 | BIN; OBS (labels); ABC §9.5 | Confirmed (BIN + OBS) |
| I4 | The sample of a note is chosen by one of two scripts of table 0x0700042F; their tree is §5.5 | OBS (both scripts, every node) | Confirmed |
| I5 | Script key = 0x10000BCD when the note is quantized, 0x10000BCE otherwise (§5.3) | BIN | Confirmed (BIN) |
| I6 | The note-on hands the script five parameters; `Music_Note_Volume` = velocity / 127 (§5.2) | BIN | Confirmed (BIN) |
| I7 | In all 14 avatar class-table closures, 0x0700042F is the only table that defines either key (§4.1) | OBS | Confirmed (14 of 14) |
| I8 | Sound-info record layout (§4.2) | OBS (45,544 of 45,544, zero leftover bytes); BIN (decoder field order) | Confirmed (BIN + OBS) |
| I9 | Sound-info flag bit 0 makes the sample loop (§4.2, §9.4) | BIN | Confirmed (BIN) |
| I10 | Instrument samples: 744 Ogg Vorbis, 2 RIFF WAVE PCM; all mono, 44,100 Hz (§4.3) | OBS (746 of 746) | Confirmed |
| I11 | Every note case plays its own sample whose recorded note is that of the case; no case transposes (§7.2) | OBS (4,730 of the 4,748 note cases matched by the sound info's internal name; the other 18 are effect samples); MEAS (667 of 703 melodic samples within 30 cents) | Confirmed |
| I12 | Sounding pitch = written pitch + an instrument octave offset (§7.4) | MEAS; MA | Probable (measured; agrees with MA) |
| I13 | Per-instrument Sound-node fields (§6.1) | OBS | Confirmed |
| I14 | A node's intensity is its own factor times its parent's intensity; the factor is clamped to 0–1 only when `IntensityMultiplierVariance` is not 0 (§10.1) | BIN; BIN-R (factor helper, node start, intensity update) | Confirmed (BIN); disagrees with SND §9.3 (Q8) |
| I15 | Sound start: volume clamped to 0–1; a fire-and-forget sound at ≤ 0.005 is skipped; only non-fire-and-forget sounds keep a handle the script can stop (§9.1) | BIN; BIN-R | Confirmed (BIN) |
| I16 | Playback-rate factor = (`Pitch` + 2) / 2, plus a uniform random term within ±`PitchVariance`, not below 0.01 (§7.3) | BIN | Confirmed (BIN) |
| I17 | Stopping a sound fades its gain linearly to zero over max(stop ramp, `RampOutTime`) in 10 ms steps (§9.2) | BIN | Confirmed (BIN) |
| I18 | Queue order by time with 0.0001 s tolerance, then sequence; note-ons ≥ 0.5 s late are dropped (§8.2, §8.3) | BIN | Confirmed (BIN) |
| I19 | Registry of sounding notes keyed by (character, note key); first entry wins; a note-off stops the instance it names (§8.4) | BIN | Confirmed (BIN) |
| I20 | Cancelling a character's music discards its queued events and turns every sounding note off at once (§8.5) | BIN | Confirmed (BIN) |
| I21 | Both music scripts carry `Script_MaxNumInstances` = 10; a run is refused when that many instances are active (§8.6) | OBS; BIN | Confirmed (BIN + OBS) |
| I22 | Channels: 6 `PlayerMusic`, 5 `QuantizedPlayerMusic`; settings keys `<label>Volume` (§10.4) | OBS (enum labels, ini keys); SND §6.1, §7 (mixing) | Confirmed (names); mixing rule Confirmed (BIN) in SND |
| I23 | Distance: full volume up to the sound info's minimum distance, linear to silence at its maximum, for sound infos with flag 0x04 or 0x40 only (§10.5) | BIN; SND §4.3, §6.2; OBS (values, flags) | Confirmed (BIN) |
| I24 | A character's instrument type is the equipped instrument item's `Item_Music_Instrument_Type`, copied on equip (§4.5) | BIN; OBS (1,077 items) | Confirmed (BIN + OBS) |
| I25 | Live-play map: DID map 0x2800000E label `MusicControl` → 0x7900026B, 37 input actions → note keys (§4.6) | OBS | Confirmed |
| I26 | A second, older copy of the music scripts (0x070017F9) serves one non-player entity. It names the same sound infos as 0x0700042F case for case (all 2,120 Sound nodes, 335 distinct sound infos, all in the instrument set); it differs only in `RampOutTime` 0 on its 222 `Bagpipe` Sound nodes and in the path intensity of five instruments (§4.4) | OBS (all Sound nodes of both scripts in both tables, 2026-10-06) | Confirmed |
| I27 | Default velocity 90; volume-mark values | ABC §7.11; BIN | Confirmed (BIN) |
| I28 | The original checks its queue on the client's update tick while events are pending (§8.3) | BIN | Probable |
| I29 | "Quantized" is the live-play quantize audio option applied to live notes (§5.1) | property and option names only | Probable |
| I30 | Property table of §3.4: PIDs, value types, enum mappers, dictionary defaults and limits | OBS (property dictionary 0x34000000, every row looked up by name) | Confirmed |
| I31 | Other users of instrument sound infos: scriptlets 0x0C005BA3 and 0x0C0072B2 play 6 `Drum` sound infos on channel 8; script 0x1000274C of table 0x07000078 has two fire-and-forget Sound nodes on channel 1 that name the `Fiddle_long` effect sound infos 0x2A00A826 and 0x2A00A824; nothing else outside the two music-script tables (§6.3). 0x2A00A824 is therefore used only by that script (§4.2, §6.2) | OBS (every Sound node of all 7,524 decodable tables and 28,992 scriptlets; re-checked 2026-10-06 against a second, independent Sound-node census) | Confirmed |
| I32 | Sound-info object defaults before loading: `flags` 0, `a` = `b` = −1, `priority` 0, `min_distance` 1, `max_distance` 50, `sound` invalid, name empty (§4.2) | BIN; BIN-R (constructor); SND §4.1 (same values, read independently) | Confirmed (BIN) |
| I33 | Sound-node constructor defaults: `FireAndForget` 1, `MoveWithEntity` 1, `UseCameraPosition` 0, random distances 0, `Offset` (0, 0, 0), channel 0, `RampOutTime` 0, `Pitch` 0, `PitchVariance` 0, sound info invalid (§15) | BIN-R (constructor) | Confirmed (BIN) |
| I34 | On equip of an `Instrument` item in a weapon slot the item's `PhysObj` is copied to the wielder's `Script_Item_PhysObj_Ranged`; if the item has no valid `PhysObj`, nothing is copied and the cached instrument type is set to 0 (§4.5) | BIN-R (equip handler) | Confirmed (BIN) |
| I35 | The equip handler's slot test is a mask of three bits that are the bit numbers (key − 1) of `Weapon_Primary` (17), `Weapon_Secondary` (18) and `Weapon_Ranged` (19) of mapper 0x230000EE (§4.5) | BIN-R (mask 0x70000); OBS (mapper labels) | Probable (the key-to-bit rule is inferred) |
| I36 | Sample durations per instrument (§6.1, §9.5): last Ogg granule position ÷ 44,100, or WAVE `data` size ÷ 88,200 | OBS (746 of 746 payloads) | Confirmed |
| I37 | The note key is bits 0–7 of the packed note at note-on (parameter and registry key) and at note-off; the packing functions keep 24 bits of the key and put the velocity in bits 24–31 only if it is at most 127 (§5.1) | BIN-R (note-on, queue processing, both packers) | Confirmed (BIN) |
| I38 | A Sound node with `UseParentIntensity` = 1 takes its own node intensity (its own factor × the parent's intensity) as its volume, clamped to 0–1 (§10.1) | BIN-R (Sound start reads the intensity of the context that the generic node start built for that node) | Confirmed (BIN); agrees with SND §9.2 step 7 |
| I39 | All 746 instrument sound infos have flag 0x04 (positional: distance law, panning, reverb send) (§4.2, §10.5) | OBS (flags); SND §4.3, §6.7 (meaning) | Confirmed (flags); meaning Confirmed (BIN) in SND |
| I40 | Instrument requests pass the sound manager's gates: voice limit, master and channel volume ≤ 0.0002, positional source beyond `max_distance` or in an unloaded landblock; a refused request ends the Sound node at once (§9.1, §9.3) | SND §6.6, §6.8; BIN-R (the Sound start ends the node when the manager refuses) | Confirmed (BIN) |
| I41 | Live-play keys: a press of a mapped action builds a note-on with velocity 90 and both flag bytes 0, a release builds a note-off; presses and releases are ignored while the local player has a song performance running; the handler and the local sequencer hand their events to a dispatch function instead of the queue (§5.1, §8.1) | BIN-R (live-play key handler, sequencer note emission) | Probable (event meanings inferred; the path after the dispatch was not followed, decision 0004) |
| I42 | Over all 632,847 nodes of the 7,524 decodable script tables and 28,992 scriptlets, 7 nodes with variance 0 have a multiplier outside 0–1, none on an instrument path (§10.1) | OBS | Confirmed |

---

<a id="s2"></a>
## 2. Overview: from a note to a sample

1. A **note event** arrives for a character: a time, the character, its **instrument type**,
   a **note key** with a **velocity**, and two flags, **from song** and **quantized** (§5.1).
   ABC playback produces such events from a file (`abc-dialect.md` §9); live play produces
   them from key actions (§4.6).
2. Events wait in a time-ordered **queue** (§8). When a note-on is due, the client builds a
   small **parameter set** (§5.2) and runs one of two **music scripts** on the character
   (§5.3), looked up through the character's script tables (§5.4).
3. The script's tree (§5.5) selects, by the two flags, the instrument type and the note key,
   one **Sound node**. The Sound node names a **sound-info record** (0x2A, §4.2), which names
   the **sample** (0x0A, §4.3).
4. The Sound node's fields decide how the sample plays: channel, loudness, pitch factor, and
   whether a later note-off can stop it (**fire-and-forget**) and with which **release ramp**
   (§9). The sound info decides looping and the distance range (§10.5).
5. When the matching **note-off** is due, the client stops the script instance started by the
   note-on (§8.4). Sounds that are not fire-and-forget fade out over their ramp (§9.2);
   fire-and-forget sounds play on to the end of their sample (§9.3).

---

<a id="s3"></a>
## 3. Identifiers

<a id="s3-1"></a>
### 3.1 Instrument types (enum mapper 0x230000FC, Confirmed)

The mapper has no base mapper and 24 labels. Keys 13–15 are unused.

| Key | Label | Key | Label | Key | Label |
|---|---|---|---|---|---|
| 0 | `Undef` | 9 | `Bagpipe` | 20 | `Fiddle_vib` |
| 1 | `Clarinet` | 10 | `Cowbell` | 21 | `Fiddle_long` |
| 2 | `Horn` | 11 | `Pibgorn` | 22 | `Fiddle_strong` |
| 3 | `Flute` | 12 | `Harp_Special` | 23 | `Fiddle_basic` |
| 4 | `Lute` | 16 | `Bassoon` | 24 | `Bassoon_vib` |
| 5 | `Harp` | 17 | `Lute_of_Ages` | 25 | `Bassoon_short` |
| 6 | `Drum` | 18 | `Fiddle_short` | 26 | `Handknells` |
| 7 | `Theorbo` | 19 | `Fiddle_pizz` | | |
| 8 | `MoreCowbell` | | | | |

The labels are internal identifiers; they do not always match how the sounds behave (for
example `Fiddle_long` is the instrument with effect sounds on its lowest notes, §6.2). An
implementation keys everything on the numeric value and shows labels only for debugging.

<a id="s3-2"></a>
### 3.2 Notes (enum mapper 0x2300015E, Confirmed)

The mapper has no base mapper and 38 labels: key 0 `Undef` and 37 labels of the form
`Note_<name><octave>`, where *name* is one of `C`, `Db`, `D`, `Eb`, `E`, `F`, `Gb`, `G`,
`Ab`, `A`, `Bb`, `B` (flats only) and *octave* is 2, 3 or 4, plus `Note_C5`. The keys are
not in pitch order; §7.1 gives the rule and the observed table. These keys are the **pitch
slots** of `abc-dialect.md` §9.5.

<a id="s3-3"></a>
### 3.3 Sound channels (enum mapper 0x2300034D, Confirmed)

Base mapper 0x2300034E adds key 0 `Default`. Keys: 1 `Combat`, 2 `Music`, 3 `UI`, 4 `VO`,
5 `QuantizedPlayerMusic`, 6 `PlayerMusic`, 7 `Ambient`, 8 `SFX`. Instrument sounds use 5 and
6 only (§5.8).

<a id="s3-4"></a>
### 3.4 Properties (Confirmed, I30; PIDs from dictionary 0x34000000)

PIDs are data: look them up by name in the property dictionary (`property-system.md` §2).
The values below are this install's. Value types are the dictionary's (`property-system.md`
§3): enum 8, float 9, DID 20, bool 21, int 15, 64-bit bitfield 14, struct 11, array 12.

| Property | PID | Type | Role |
|---|---|---|---|
| `Music_Note` | 0x10001159 | enum 0x2300015E | note key of the note being played (§5.2) |
| `Music_Instrument_Type` | 0x10001230 | enum 0x230000FC | instrument type of the note (§5.2) |
| `Music_Note_Volume` | 0x10001DCF | float | velocity / 127 (§5.2) |
| `Music_Note_From_Song` | 0x10001709 | bool | 1 for notes of an ABC performance (§5.2) |
| `Music_Note_Quantized` | 0x1000170A | bool | 1 for quantized live notes (§5.2) |
| `Item_Music_Instrument_Type` | 0x10000C90 | enum 0x230000FC | instrument type of an item (§4.5) |
| `Music_Cached_Instrument_Type` | 0x100024F0 | enum 0x230000FC | the wielder's copy of it (§4.5) |
| `Usage_IsMusicalInstrument` | 0x1000173D | bool | true on every instrument item (§4.5) |
| `Item_EquipmentCategory` | 0x1000039E | bitfield, mapper 0x2300004C | `Instrument` = bit value 0x40 (§4.5) |
| `Music_Control_InputAction_Map` | 0x100001C8 | array of `Music_Control_InputAction_Map_Struct` (0x10000496) | live-play map (§4.6) |
| `Music_InputAction` | 0x10000A0D | enum 0x230000AF | an input action (§4.6) |
| `Script_MaxNumInstances` | 0x000003F2 | int, default 1, minimum 1 | per-script instance limit (§8.6) |
| `Script_IntensityMultiplier` | 0x000003A3 | float, default 1.0, limits 0–1 (the client does not enforce the limits, §10.1) | a node's `IntensityMultiplier` field (§10.1) |
| `PhysObj` | 0x0000047F | DID | an item's physical object (§4.5) |
| `Script_Item_PhysObj_Ranged` | 0x10000FC7 | DID | the wielder's copy of it (§4.5) |

<a id="s3-5"></a>
### 3.5 Script keys (enum mapper 0x23000097, Confirmed)

| Key | Label | Used for (§5.3) |
|---|---|---|
| 0x10000BCD | `SND_PLAYERMUSIC_PlayMusic_SUS_01` | quantized notes |
| 0x10000BCE | `SND_PLAYERMUSIC_PlayMusic_SUS_02` | all other notes |

---

<a id="s4"></a>
## 4. The records that hold instrument samples

<a id="s4-1"></a>
### 4.1 The music scripts (script table 0x0700042F, Confirmed)

Table 0x0700042F (`script-table.md` §2) has table kind 2, no includes and 292 scripts; most
are ambience and effect sounds. Two of its scripts hold every instrument sample choice:

| Script key | Roots | Nodes | Collection | Switch | Sound | Particle | Per-script property set | Per-script DID list |
|---|---|---|---|---|---|---|---|---|
| 0x10000BCD | 1 | 2,539 | 78 | 68 | 2,324 | 69 | `Script_MaxNumInstances` = 10 | 759 DIDs: 745 sound infos, 14 particle descriptors |
| 0x10000BCE | 1 | 2,663 | 82 | 71 | 2,436 | 74 | `Script_MaxNumInstances` = 10 | 760 DIDs: 745 sound infos, 15 particle descriptors |

The per-script DID list is a preload list (`script-table.md` §2): it names the same 745 sound
infos that the Sound nodes reach (§6.3) and the particle descriptors of §5.5.

How a player character reaches the table: each of the 14 avatar class tables
(0x07000417–0x0700041D, 0x07001D84, 0x07001D86, 0x0700204E, 0x07002053, 0x0700247A,
0x07002F15, 0x07002F16; `script-table.md` §8.1–§8.2) includes 0x0700042F somewhere in its
closure (148 or 149 tables), and in all 14 closures 0x0700042F is the **only** table that
defines 0x10000BCD or 0x10000BCE. The lookup-order question of `script-table.md` §8.2 (T18)
therefore does not affect instruments.

<a id="s4-2"></a>
### 4.2 Sound-info records (prefix 0x2A, `client_general.dat`; Confirmed, 45,544 of 45,544)

`records.md` §2.1 registers prefix 0x2A as DB type 0x12 (sound info). Layout, little-endian,
uncompressed, no leftover bytes in any record:

| Offset | Size | Type | Field | Meaning | Observed |
|---|---|---|---|---|---|
| 0 | 4 | DID | `did` | The record's own DID | equals the record DID in all |
| 4 | 4 | u32 | `flags` | Bit 0x01 = **loop** the sample until stopped (BIN, §9.4). Bit 0x04 = **positional**: distance law, panning and reverb send; bit 0x40 = distance law without panning (SND §4.3; §10.5). Bits 0x02 and 0x10: SND §4.3 (Q3). | 4 (29,833), 0 (13,467), 1 (917), 0x14 (543), 5 (360), 0x40 (324), 0x41 (92), 0x45 (4), 0x15 (3), 7 (1) |
| 8 | 4 | f32 | `a` | Read and kept by the client but not used by any playback code examined (SND §4.1) | −1.0 in all |
| 12 | 4 | f32 | `b` | As `a` | −1.0 in all |
| 16 | 4 | f32 | `priority` | Enters the engine's voice-ranking score (2 − `priority`) × distance (SND §4.2, §6.6) | 1.0 (32,028), 0.5 (9,133), 0.1 (1,646), 0.2 (1,239), 0.7 (884) … |
| 20 | 4 | f32 | `min_distance` | Metres; full volume at or inside this distance (BIN, §10.5) | 1 (36,428), 3 (4,656), 1.2 (2,465) … |
| 24 | 4 | f32 | `max_distance` | Metres; silent at or beyond this distance (BIN, §10.5) | 32 (28,652), 15 (4,502), 64 (4,024), 30 (3,445) … |
| 28 | 4 | DID | `sound` | The sample record (prefix 0x0A, §4.3) | prefix 0x0A in all |
| 32 | 1 + *n* | Pascal string | `name` | Internal name, Latin-1, VLE length (`records.md` §1.4) | *n* = 3–84, one-byte length in all |

The decoder in the client reads the seven 32-bit fields in this order, then the name (BIN).
The engine's in-memory defaults for a sound info that has not been loaded are
`flags` 0, `a` = `b` = −1, `priority` 0, `min_distance` 1, `max_distance` 50, `sound` the
invalid DID and an empty name (I32; the same values as SND §4.1; they never appear in
records, so they matter only to an implementation that creates sound infos itself).

Values in the 746 instrument sound infos: the 745 that the music scripts reach (§6.3) and
0x2A00A824, which neither music script references but table 0x07000078 does (§6.2, §6.3,
I31):

| Set | Count | `flags` | `a`, `b` | `priority` | `min_distance` | `max_distance` |
|---|---|---|---|---|---|---|
| `Pibgorn` samples | 37 | 4 | −1, −1 | 0.5 | 1 | 30 |
| `Handknells` samples | 37 | 4 | −1, −1 | 1.0 | 1 | 30 |
| `Bagpipe` samples for `Note_C2` … `Note_B2` | 12 | **5** (loop) | −1, −1 | 1.0 | 3 | 30 |
| every other instrument sample | 660 | 4 | −1, −1 | 1.0 | 3 | 30 |

Every one of these 746 sound infos has flag 0x04 set (I39), so every instrument note is a
positional sound (§10.5). Only bits 0x01 and 0x04 occur in them.

> **Note:** [`sound.md`](sound.md) §4 is normative for the sound-info record; this section
> repeats only what instruments need. On the record layout and field meanings the two specs
> agree. Their disagreement on node intensity is in §10.1 (Q8).

> **OPEN:** (Q3) the effect of sound-info flag bits 0x02 and 0x10 (SND §4.3 and its open
> question 3). Neither bit is set in any instrument sound info, so instrument playback does
> not depend on the answer. Fields `a` and `b` are answered by SND §4.1 (read, never used).

<a id="s4-3"></a>
### 4.3 Sample payloads (prefix 0x0A; Confirmed, 746 of 746)

Each instrument sound info names a distinct 0x0A record (746 records; the envelope and
container sniffing are `records.md` §9). Census:

| Container | Records | Format | Trailer u32 (`records.md` §9.1) |
|---|---|---|---|
| Ogg Vorbis | 744 | mono, 44,100 Hz; 574 streams have no user comment, 170 have one comment without `=` naming an encoding tool; no loop markers or other tags | 2 |
| RIFF WAVE | 2 (the two cowbells) | PCM format 1, mono, 44,100 Hz, 16-bit; a `PAD ` chunk before `data` | 3 |

Sample lengths per instrument are in §6.1 (I36, all 746 payloads). Lengths come from the Ogg
stream's last granule position divided by the sample rate (44,100), or from the WAVE `data`
size divided by 88,200 bytes per second (16-bit mono at 44,100 Hz).

<a id="s4-4"></a>
### 4.4 The older copy (script table 0x070017F9, Confirmed)

Table 0x070017F9 also defines 0x10000BCD and 0x10000BCE with the same tree shape, but only for
instrument types 1–11 (plus the default and `Undef` cases of 0x10000BCE, §5.5). It is reached
only from class table 0x07001819, which only entity descriptor 0x47005B1C names; no avatar
closure contains it. Players never use it.

Compared with 0x0700042F case by case, keyed by (script key, branch, instrument case, note
case), every one of its 2,120 Sound nodes has a matching case, and:

- **Sound infos are the same.** Every Sound node names the same sound info as the matching
  case of 0x0700042F. Its 335 distinct sound infos all belong to the 746 instrument set (§4.2).
- **Sound-node fields are the same except one.** Channel, `FireAndForget`, `PitchVariance` and
  every other Sound-node field match, except `RampOutTime` on the 222 `Bagpipe` Sound nodes:
  0 here, 0.2 in 0x0700042F (§6.1).
- **Intensity differs for five instruments** (1,110 Sound nodes). The product of the intensity
  multipliers on the path from the root to the Sound node (§10.1) is:

| Key | Label | 0x0700042F | 0x070017F9 |
|---|---|---|---|
| 3 | `Flute` | 0.65 | 0.70 |
| 4 | `Lute` | 0.34 | 1.0 |
| 7 | `Theorbo` | 0.455 | 1.0 |
| 9 | `Bagpipe` | 0.65 | 0.70 |
| 11 | `Pibgorn` | 0.70 | 0.175 (0.7 × 0.25 on two nodes); 0.10 (0.4 × 0.25) in the song branch of 0x10000BCE |

The other instruments, and the default case of 0x10000BCE, have the same products in both
tables. The products are the constant multipliers only; the velocity factor of 0x10000BCE
(§10.2) is not part of them.

<a id="s4-5"></a>
### 4.5 Where a character's instrument type comes from (Confirmed (BIN + OBS))

- **Items.** 1,077 object property sets (0x79) carry `Item_Music_Instrument_Type`; every one
  also has `Usage_IsMusicalInstrument` = true and `Item_EquipmentCategory` = `Instrument`
  (0x40). Values in use: 1 (80 items), 2 (80), 3 (170), 4 (108), 5 (94), 6 (118), 7 (104),
  8 (1), 9 (79), 10 (80), 11 (80), 12 (3), 16 (26), 17 (1), 18 (1), 19 (1), 20 (2), 21 (37),
  22 (2), 23 (7), 24 (1), 25 (1), 26 (1): every non-zero key of §3.1. A further 1,097 items
  have category `Instrument` but no instrument type.
- **Equip (BIN, I24, I34, I35).** The client acts when an item whose `Item_EquipmentCategory`
  is exactly `Instrument` (the whole 64-bit value equals 0x40) is put into a slot whose slot
  mask has any of the bits 0x10000, 0x20000, 0x40000. These bits are bit numbers 16, 17 and
  18, which equal key − 1 of `Weapon_Primary` (17), `Weapon_Secondary` (18) and
  `Weapon_Ranged` (19) in the container-slot mapper 0x230000EE (the key-to-bit rule is
  Probable). Then:
  1. If the item has a `PhysObj` value that is a valid DID, the client sets the wielder's
     `Script_Item_PhysObj_Ranged` to it and the wielder's `Music_Cached_Instrument_Type` to
     the item's `Item_Music_Instrument_Type` (0 when the item has none).
  2. Otherwise it copies nothing and sets `Music_Cached_Instrument_Type` to 0.

  In this install all 1,077 typed and all 1,097 untyped instrument items carry a non-zero
  `PhysObj` in their own property set (OBS, 2026-10-05), so case 2 does not occur.
- **Notes.** The instrument type travels in every note event (§5.1). For other players it
  arrives with the event; for the local player it is the cached value above.

An instrument item without a type yields type 0 (`Undef`): its live notes play no sound and
its song notes play the default sound set (§5.5).

<a id="s4-6"></a>
### 4.6 Live-play map (Confirmed)

DID map 0x2800000E (label `WEENIECONTENT` in the root map, `records.md` §8.2) maps label
`MusicControl` (key 0x10000075) to game object 0x7000026B. Its property set 0x7900026B holds
one property, `Music_Control_InputAction_Map`: 37 structs, each with a `Music_InputAction`
(enum 0x230000AF) and a `Music_Note`. In all 37 the action label `MUSIC_<n>` pairs with the
note label `Note_<n>` (for example `MUSIC_C2` with `Note_C2`). Resolve the object through the
DID map label at run time; do not hard-code its DID. The keys bound to these actions are in
`keymap.md`.

---

<a id="s5"></a>
## 5. How the client chooses the sample

<a id="s5-1"></a>
### 5.1 A note event (Confirmed (BIN))

| Field | Meaning |
|---|---|
| time | When the event is due, seconds on the client's playback clock (§8.3) |
| sequence | Creation order, used to order events with equal times (§8.2) |
| kind | 1 = note-on, 2 = note-off |
| character | The entity that plays |
| instrument type | Key of §3.1 |
| packed note | u32: note key (§3.2) in the low bits, velocity 0–127 in bits 24–31 (rule below) |
| quantized | Flag |
| from song | Flag |

**Packed note (Confirmed (BIN), I37).** One rule for writing and one for reading:

- *Writing.* Both packing functions of the client (the sequencer's and the live-play one)
  store the note key masked to bits 0–23 and, if the velocity is at most 127, the velocity in
  bits 24–31. If the velocity is above 127 the word is the note key alone, so bits 24–31 are
  0 and the note plays at volume 0 (§5.2).
- *Reading.* The playback module reads the **note key from bits 0–7 only**, in all three
  places that use it: the `Music_Note` parameter (§5.2), the registry key at note-on (§8.4)
  and the registry key at note-off (§8.3 step 4). It reads the velocity from bits 24–31.
  Bits 8–23 are never read.

Note keys are 1–37 in this install, so bits 8–23 are always 0 and the two widths cannot
differ in practice. Tessera writes the key into bits 0–7 with bits 8–23 zero, and reads it
from bits 0–7.

ABC playback sets **from song** and gives velocities from the volume marks
(`abc-dialect.md` §7.11: default 90, range 30–127). By name, **quantized** is the client's
live-play quantize audio option applied to live notes (Probable, I29).

**Live play (Probable, I41).** The live-play key handler reacts to the actions of the
`MusicControl` map (§4.6). On a key press it builds a **note-on** for the action's note key
with velocity **90** and its flag bytes 0, timed at the current clock. On the key release it
builds a **note-off** for the same note key. While the local player has a song performance
running, presses and releases make no note (the press leads to a feedback path instead). The
handler does not put the note into the playback queue directly: like the ABC sequencer, it
hands the event to a dispatch function outside the playback module, which was not followed
(decision 0004).

> **OPEN:** (Q1) how the events that leave the module through that dispatch come back into
> the queue (§8.1). In particular: whether live notes arrive with velocity 90 unchanged,
> which component sets the **quantized** and **from song** flags (the handler leaves its
> flag bytes 0), whether and how the quantize option delays a live note to a grid, and what
> triggers the cancel of §8.5. Until it is answered, Tessera follows the sandbox decision in
> §12. Suggested method: owner observation (hold a sustained instrument's note key for 2 s
> and for 10 s, release, listen for the 0.2 s fade; compare live loudness with `+mf+` and
> `+ffff+` song notes; play live with the quantize option on and off against a metronome).

<a id="s5-2"></a>
### 5.2 The parameter set (Confirmed (BIN))

At a note-on the client builds a property set and passes it to the script run as its
parameters (Switch nodes and driven fields read from it):

| Property | Value |
|---|---|
| `Music_Note` | the note key: bits 0–7 of the packed note (§5.1) |
| `Music_Note_Volume` | velocity ÷ 127 as a float, where velocity = bits 24–31 of the packed note; a velocity above 127 counts as 0 |
| `Music_Instrument_Type` | the instrument type |
| `Music_Note_Quantized` | the quantized flag |
| `Music_Note_From_Song` | the from-song flag |

Each property is set only if the dictionary gives it the expected type (enum for the first and
third, float for the volume, bool for the flags), which holds in this install.

<a id="s5-3"></a>
### 5.3 The script key (Confirmed (BIN))

The client runs script **0x10000BCD** when the quantized flag is set and **0x10000BCE**
otherwise. The from-song flag does not choose the script; it is tested inside both (§5.5).

<a id="s5-4"></a>
### 5.4 Running the script

The script key is looked up through the character's script tables (`script-table.md` §8.2;
§4.1 here). One **instance** of the script is started with the parameter set of §5.2; the
client keeps the instance's handle for the note-off (§8.4). Before starting, the client
applies the instance limit of §8.6.

<a id="s5-5"></a>
### 5.5 Tree shape (Confirmed, both scripts)

Both scripts have one root, a Collection (`script-table.md` §4.3: every child of every
Default port starts at once). Its single child is a Switch on `Music_Note_From_Song`:

- case 1 → **song branch**: a Switch on `Music_Instrument_Type`;
- Default → a Switch on `Music_Note_Quantized`:
  - case 1 → **quantized branch**: a Collection holding a Switch on `Music_Instrument_Type`;
  - Default → **live branch**: a Collection holding a Switch on `Music_Instrument_Type`.

Every case of every Switch in the two scripts lists exactly one value, uses operator Equal
(3) and has transition times 0 (`script-table.md` §4.3).

**Instrument switches.** In 0x10000BCD each of the three instrument switches has 23 cases
(every non-zero key of §3.1) and **no** Default port. In 0x10000BCE the song-branch switch has
the same 23 cases plus a Default port; the quantized and live switches have 24 cases (the 23
plus `Undef` = 0) and a Default port.

**Instrument subtree.** Each instrument case (and each Default port of an instrument switch)
leads through one Collection (two nested Collections in most cowbell cases) to:

- a **Particle** node (kind 59): the visual note effect. Descriptors used: 0x39000A23–
  0x39000A27, 0x39000BDE–0x39000BE2, 0x39000BE4–0x39000BE7, and 0x390005EF (the `Undef` case
  of 0x10000BCE, which has **only** this particle and no sound). Out of scope here.
- a **Switch on `Music_Note`** with one case per note key; each case holds exactly one Sound
  node (exceptions in §6.2). The cowbells (`Cowbell`, `MoreCowbell`) have no note switch:
  their Collection(s) hold **one** Sound node, so every note key plays the same sample. (One
  `Cowbell` case also holds an extra, empty Collection, and one cowbell Collection lacks the
  particle; neither changes the sound.)

Exactly one Collection on each instrument path may carry a multiplier other than 1.0; it is
the instrument's **intensity multiplier** of §6.1 (for `Cowbell` it sits on the inner
Collection in four of the six cases). All other nodes on the path have 1.0.

No Music_Note switch has a Default port: a note key without a case plays no sound (the
particle still runs).

**Default cases of 0x10000BCE.** The song-branch Default plays the `Clarinet` samples; the
quantized- and live-branch Defaults play the `Flute` samples (same sound infos as those
instruments, intensity 0.7, §6.2).

<a id="s5-6"></a>
### 5.6 Evaluation rules used here

Only four node kinds occur in the two scripts. To choose samples, an evaluator needs:

1. **Collection**: start every child of every port.
2. **Switch**: read the switched property from the parameter set; start the children of the
   case whose value equals it; if none matches, start the children of the Default port if
   there is one; otherwise start nothing.
3. **Sound**: start the sample of its sound info as described in §9 and §10.
4. **Particle**: visual only; an audio-only implementation may skip it.

Driven fields (`script-table.md` §4.2) occur in exactly two nodes: the root Collection of
0x10000BCE in table 0x0700042F (and of the same key in 0x070017F9), whose
`Script_IntensityMultiplier` is driven by `Music_Note_Volume` (§10.2).

<a id="s5-7"></a>
### 5.7 The instrument table (Decision for Tessera; equivalent to §5.5–§5.6)

Because the tree has no state, timers or randomness on the audio path, it can be flattened
once per data version:

1. For each script key (2), branch (song, quantized, live), instrument key (every value that
   has a case, plus "default") and note key (1–37, plus 0), walk the tree with that parameter
   set by the rules of §5.6.
2. Record for each combination the list of Sound nodes reached (zero or one in this install)
   and, for each, the product of the `IntensityMultiplier` values on the path from the root
   to the Sound node, excluding the root's driven multiplier (§10.1), plus a flag telling
   whether the root's multiplier is driven by `Music_Note_Volume`.
3. Look notes up in that table at note-on time.

Real-data tests should check that the walk and the table agree for every combination.

<a id="s5-8"></a>
### 5.8 Branch, channel and volume (Confirmed)

| Script | Branch | Channel of the Sound nodes | `Music_Note_Volume` applied (§10.2) |
|---|---|---|---|
| 0x10000BCD (quantized) | song | 6 `PlayerMusic` | no |
| 0x10000BCD | quantized | 5 `QuantizedPlayerMusic` | no |
| 0x10000BCD | live | 6 | no |
| 0x10000BCE (not quantized) | song | 6 | yes |
| 0x10000BCE | quantized | 5 | yes |
| 0x10000BCE | live | 6 | yes |

The channel is a field of each Sound node; read it, do not derive it. Exceptions in this
install: every `Harp_Special` Sound node uses channel 6, also in the quantized branches; the
`Pibgorn` `Note_C5` node of both quantized branches uses channel 6.

With the key choice of §5.3, a quantized note always runs 0x10000BCD and reaches its
quantized branch only when the from-song flag is clear; a non-quantized note runs 0x10000BCE
and reaches its song or live branch. The quantized branch of 0x10000BCE and the live branch of
0x10000BCD are therefore never reached by the client's own key choice; they mirror the others.

> **OPEN:** (Q4) whether a note ever arrives with both the quantized and the from-song flag
> set. Such a note would run 0x10000BCD and reach its song branch: velocity ignored (§10.2)
> and `Horn` `Note_C5` silent (§6.2). The flags are set outside the module that was read
> (Q1). Suggested method: owner observation of a `/play` performance with the quantize
> option on, listening for a missing top C on a horn and for loudness marks having no effect.

---

<a id="s6"></a>
## 6. The instruments (Confirmed unless marked)

<a id="s6-1"></a>
### 6.1 Per-instrument facts (table 0x0700042F; lengths I36)

"Notes" counts note cases with a sound; "FF" is the Sound nodes' `FireAndForget`; "Ramp" is
their `RampOutTime` in seconds; "Intensity" is the instrument Collection's multiplier (all
other nodes on the path have 1.0); sound-info DIDs are the lowest and highest of the set (sets
interleave, so a span is not a membership test); lengths are sample durations in seconds;
"Octave" is the measured offset of §7.4 with the number of samples that agree.

| Key | Label | Notes | FF | Ramp | Intensity | Sound infos | Length (s) | Octave |
|---|---|---|---|---|---|---|---|---|
| 1 | `Clarinet` | 37 | 0 | 0.2 | 0.70 | 37: 0x2A0026B3–0x2A0026D7 | 5.52–5.91 | +1 (37/37) |
| 2 | `Horn` | 37 | 0 | 0.2 | 0.70 | 37: 0x2A002780–0x2A0027A4 | 6.27–7.70 | 0 (37/37) |
| 3 | `Flute` | 37 | 0 | 0.2 | 0.65 | 37: 0x2A00275A–0x2A00277E | 5.14–6.67 | +2 (37/37) |
| 4 | `Lute` | 37 | 1 | 0 | 0.34 | 37: 0x2A0026DC–0x2A002723 | 0.65–1.00 | 0 (37/37) |
| 5 | `Harp` | 37 | 1 | 0 | 1.00 | 37: 0x2A0026DD–0x2A002725 | 0.59–1.21 | 0 (34/37) |
| 6 | `Drum` | 37 | 1 | 0 | 1.00 | 37: 0x2A0027FA–0x2A00281E | 0.25–0.96 | unpitched: each note key is a different drum sound |
| 7 | `Theorbo` | 37 | 1 | 0 | 0.455 | 37: 0x2A0027D5–0x2A0027F9 | 0.72–0.83 | −1 (35/37) |
| 8 | `MoreCowbell` | all (one sample) | 1 | 0 | 1.00 | 1: 0x2A002D2D | 0.25 | unpitched; `PitchVariance` 0.02 |
| 9 | `Bagpipe` | 37 | 0 | 0.2 | 0.65 | 37: 0x2A002D04–0x2A002D28 | 2.92 (12 looping drones, `Note_C2`–`Note_B2`); 6.44–6.96 | +1 (36/37) |
| 10 | `Cowbell` | all (one sample) | 1 | 0 | 0.70 | 1: 0x2A002D2C | 0.29 | unpitched; `PitchVariance` 0.02 |
| 11 | `Pibgorn` | 37 | 0 | 0.2 | 0.70 | 37: 0x2A006B5B–0x2A007049 | 6.04–7.68 | +2 (34/37) |
| 12 | `Harp_Special` | 37 | 1 | 0 | 0.45 | 37: 0x2A009628–0x2A00964C | 0.75–1.15 | 0 (36/37) |
| 16 | `Bassoon` | 37 | 0 | 0.2 | 0.95 | 37: 0x2A00AC88–0x2A00ACF4 | 5.64–6.38 | 0 (31/37) |
| 17 | `Lute_of_Ages` | 37 | 1 | 0 | 0.77 | 37: 0x2A009603–0x2A009627 | 0.62–1.11 | 0 (37/37) |
| 18 | `Fiddle_short` | 37 | 1 | 0.2 | 0.50 | 37: 0x2A00A795–0x2A00AC4E | 0.47–1.72 | +1 (37/37) |
| 19 | `Fiddle_pizz` | 37 | 1 | 0.2 | 0.65 | 37: 0x2A00A790–0x2A00AC4C | 0.40–1.40 | +1 (36/37) |
| 20 | `Fiddle_vib` | 37 | 0 | 0.2 | 1.00 | 37: 0x2A00A78B–0x2A00AC43 | 8.13–10.23 | +1 (37/37) |
| 21 | `Fiddle_long` | 33 (§6.2) | 0 | 0.2 | 1.00 | 33: 0x2A00A78D–0x2A00A828 | effects 0.56–2.00; notes 7.02–9.48 | effects unpitched; +1 (30/30) |
| 22 | `Fiddle_strong` | 37 | 0 | 0.2 | 1.00 | 37: 0x2A00AC50–0x2A00AC74 | 6.06–7.05 | +1 (37/37) |
| 23 | `Fiddle_basic` | 37 | 0 | 0.2 | 1.00 | 37: 0x2A00A78D–0x2A00AC4D | 7.02–9.48 | +1 (37/37) |
| 24 | `Bassoon_vib` | 37 | 0 | 0.2 | 0.95 | 37: 0x2A00AC8A–0x2A00ACF5 | 5.04–6.30 | 0 (29/37) |
| 25 | `Bassoon_short` | 37 | 0 | 0.2 | 0.95 | 37: 0x2A00AC87–0x2A00ACF3 | 0.37–0.54 | 0 (31/37) |
| 26 | `Handknells` | 37 | 1 | 0.2 | 1.00 | 37: 0x2A00EEFB–0x2A00EF44 | 1.50 | +3 (22/37; bell partials disturb the detector) |
| default (0x10000BCE only) | | 37 | 0 | 0.2 (one node 0, §6.2) | 0.70 | the `Clarinet` (song) or `Flute` (quantized, live) set | as those | as those |

The same values hold in both scripts and in all three branches, except as listed in §6.2 and
§5.8. Every Sound node of both scripts has `MoveWithEntity` = 1, `UseCameraPosition` = 0,
`MinRandomDistance` = `MaxRandomDistance` = 0, `Offset` = (0, 0, 0) and `Pitch` = 0; every node
on every path from a root to a Sound node has `UseParentIntensity` = 1,
`IntensityMultiplierVariance` = 0 and `Probability` = 1.

<a id="s6-2"></a>
### 6.2 Exceptions and quirks (Confirmed)

| Where | What | Effect |
|---|---|---|
| `Fiddle_long` (21), every branch of both scripts | Note keys of `Note_C2`, `Note_Db2`, `Note_D2` play three **effect** samples (sound infos 0x2A00A825, 0x2A00A826, 0x2A00A828; 2.00, 0.56 and 1.35 s; not pitched); `Note_Eb2` … `Note_Gb2` have no case; `Note_G2` … `Note_C5` play the same sound infos as `Fiddle_basic` | 33 sounding notes: 3 effects, a 4-note gap, 30 chromatic notes |
| sound info 0x2A00A824 | A fourth effect sample of the same family | Not referenced by either music script; its only user is the non-music script 0x1000274C of table 0x07000078 (§6.3, I31) |
| `Horn` (2), song branch of 0x10000BCD | The `Note_C5` case holds an empty Collection | Silent; reachable only by a quantized song note (Q4, §5.8) |
| `Pibgorn` (11), `Note_C5`, quantized branches | Channel 6 instead of 5 | §5.8 |
| `Harp_Special` (12), quantized branches | Channel 6 instead of 5 | §5.8 |
| Default case, song branch of 0x10000BCE, `Note_C2` | `RampOutTime` 0 and `PitchVariance` 0.2 (every other default node: 0.2 and 0) | That note stops without a fade and varies its playback rate by up to ±0.2 (§7.3) |
| `Cowbell` (10), `MoreCowbell` (8) | One Sound node with `PitchVariance` 0.02 and no note switch | Every note key plays the same sample at a slightly random rate |
| `Undef` (0) | 0x10000BCD: no case (nothing starts). 0x10000BCE: quantized and live branches have a particle-only case; the song branch has no `Undef` case, so its Default (clarinet set) plays | §4.5 |
| Instrument keys 13–15, 27 and up | No case | 0x10000BCD: nothing; 0x10000BCE: the Default |

<a id="s6-3"></a>
### 6.3 Shared samples (Confirmed)

The two scripts reach 745 distinct sound infos (each naming a distinct sample). Shared
between instrument sets: the 30 chromatic `Fiddle_long` notes use the `Fiddle_basic` sound
infos; the Default cases use the `Clarinet` and `Flute` sound infos. Two scriptlets
(0x0C005BA3, 0x0C0072B2; 11 Sound nodes each, 6 distinct `Drum` sound infos) also play
`Drum` sound infos on channel 8 as plain effects, and script 0x1000274C of table 0x07000078
has two fire-and-forget Sound nodes on channel 1 that play the `Fiddle_long` effect sound
infos 0x2A00A826 and 0x2A00A824 (I31). Apart from these
and the older copy 0x070017F9 (§4.4), no script table or scriptlet names an instrument sound
info (all 7,524 decodable tables and 28,992 scriptlets walked). These other uses are not
player music.

---

<a id="s7"></a>
## 7. Pitch

<a id="s7-1"></a>
### 7.1 Pitch to note key (Confirmed (BIN + OBS))

The ABC reader produces pitch numbers 48–84 (`abc-dialect.md` §7.3; 48 is ABC `C,`, 84 is
`c'`). The client converts pitch *p* to a note key as follows:

1. Let *i* = *p* − 48. If *i* is outside 0–36, the key is 0 (`Undef`).
2. Otherwise the note name is entry *i* mod 12 of the list C, Db, D, Eb, E, F, Gb, G, Ab, A,
   Bb, B, and the octave is 2 + ⌊*i* / 12⌋; the label is `Note_` + name + octave (so *p* = 84 is
   `Note_C5`).
3. The key is the key of that label in enum mapper 0x2300015E.

The client implements step 2–3 as a fixed 37-entry table (BIN); it equals the label rule for
all 37 pitches in this install, and it equals the pitch-slot table of `abc-dialect.md` §9.5.
Tessera should build the table from the labels at run time and may check it against these
observed keys:

| Octave | C | Db | D | Eb | E | F | Gb | G | Ab | A | Bb | B |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2 (pitch 48–59) | 28 | 12 | 2 | 1 | 21 | 31 | 30 | 5 | 20 | 9 | 17 | 16 |
| 3 (pitch 60–71) | 29 | 14 | 4 | 3 | 25 | 34 | 33 | 8 | 24 | 11 | 19 | 18 |
| 4 (pitch 72–83) | 32 | 15 | 7 | 6 | 27 | 37 | 36 | 10 | 26 | 13 | 23 | 22 |
| 5 (pitch 84) | 35 | | | | | | | | | | | |

The live-play map (§4.6) gives note keys directly; it uses the same labels.

<a id="s7-2"></a>
### 7.2 One sample per note, no transposition (Confirmed)

Every instrument except the cowbells has one recorded sample per note key. The client never
transposes a sample to reach a note: every instrument Sound node has `Pitch` = 0, and the only
non-zero `PitchVariance` values are the cowbells' 0.02 and the one default node of §6.2. Each
sound info's internal name encodes a note; in both scripts there are 4,748 Sound nodes under
`Music_Note` cases, and for 4,730 of them the name's note equals the case's note label. The
other 18 are the `Fiddle_long` effect cases (3 notes × 3 branches × 2 scripts), whose samples
are not pitched notes. Measured pitch classes agree (§7.4).

<a id="s7-3"></a>
### 7.3 Playback-rate factor (Confirmed (BIN))

When a Sound node starts (§9.1), its playback-rate factor is:

1. *f* = (`Pitch` + 2.0) × 0.5 (so `Pitch` 0 gives 1.0; the dictionary limits `Pitch` to −1 … 1,
   giving 0.5 … 1.5);
2. if `PitchVariance` > 0: *f* = *f* + *r*, with *r* uniform in [−`PitchVariance`,
   +`PitchVariance`]; then if *f* is not above 0.01, *f* = 0.01.

The factor is applied as a resampling rate (it changes pitch and length together). For
instruments *f* = 1.0, except the cowbells (1.0 ± 0.02, about ±34 cents) and the default node
of §6.2.

<a id="s7-4"></a>
### 7.4 Sounding pitch (Probable; measured)

Write a note label's pitch as a MIDI-style number *m* = 12 × (octave + 1) + index of the name
(so `Note_C2` = 36, `Note_A4` = 69 = 440 Hz). The measured fundamental of an instrument's
sample for that label is *m* + 12 × *k* within a few cents, where *k* is the instrument's
**octave offset** of §6.1: −1 `Theorbo`; 0 `Horn`, `Lute`, `Lute_of_Ages`, `Harp`,
`Harp_Special` and the three bassoons; +1 `Clarinet`, `Bagpipe` and all fiddle sets; +2
`Flute`, `Pibgorn`; +3 `Handknells`. Tuning is equal temperament with A = 440 Hz: for 667 of
the 703 samples of the 19 melodic sets the measured pitch class is within 30 cents of the
label (the rest are detector errors on weak fundamentals and bell partials; every `Lute`
sample is within 14 cents, every `Horn` and `Clarinet` sample within 2 cents). The offsets
agree with MA.

Tessera does not need the offsets to play notes (the samples carry their pitch); they serve
tests, displays and any export to MIDI. Do not hard-code them: measure or take them from
this table as test expectations only.

> **OPEN:** (Q5) the exact octave of the `Handknells` samples. Only 22 of 37 agree on +3;
> bell partials make the period estimate unreliable. Playback does not depend on it.
> Suggested method: a partial-series fit of the bell spectra, or a listening comparison
> with a reference tone.

---

<a id="s8"></a>
## 8. Playback queue (Confirmed (BIN) unless marked)

<a id="s8-1"></a>
### 8.1 What it holds

The client keeps one queue of note events (§5.1) for all characters. The notes in it
originate from the ABC performance of a character (`abc-dialect.md` §9: its note-ons,
note-offs and their times, each turned into an absolute time when the performance starts,
`abc-dialect.md` §8.5), from live notes (§5.1), and from the cancel of §8.5, which inserts
its note-offs directly. The local sequencer and the live-play handler do not insert their
events directly: both hand them in batches to the dispatch function named in §5.1 (BIN-R),
and the path by which events reach the queue from there was not read (Q1).

<a id="s8-2"></a>
### 8.2 Order

Events are ordered by time. Two times that differ by less than **0.0001 s** count as equal;
equal times are ordered by ascending sequence number (§5.1). This matches the generation
order of `abc-dialect.md` §9.2, so a note-off and a following note-on of the same pitch at the
same time are processed off first.

<a id="s8-3"></a>
### 8.3 Processing

While the queue is not empty the client checks it on its update tick (Probable: the check is
attached to a recurring client event while events are pending and detached when the queue
empties). At each check, with *now* the current time:

1. Take the earliest event. If its time is later than *now*, stop.
2. If it is a **note-on** whose time is at or before *now* − **0.5 s**, discard it without
   playing (it is too late) and go to step 1.
3. **Note-on**: if the character no longer exists, do nothing. Otherwise build the parameter
   set (§5.2), pick the script key (§5.3), run the script (§5.4) and register the instance
   (§8.4).
4. **Note-off**: look up the registry (§8.4) for (character, note key). If an entry exists,
   remove it and stop that script instance on the character (§9.2). Late note-offs are never
   discarded.
5. Remove the event and go to step 1.

<a id="s8-4"></a>
### 8.4 Registry of sounding notes

The registry maps a character to a map from **note key** to (script key, instance handle).

- The note key here is bits 0–7 of the packed note, at note-on and at note-off (§5.1).
- **Note-on:** the client takes a fresh instance handle from the character, adds
  (note key → script key, handle) to the registry, and then runs the script with that handle.
  If the character already has an entry for that note key, the **old entry is kept** and the
  new instance is not registered: no later note-off can stop the new instance, which then
  plays until its sounds end by themselves. Because the entry is added before the run, a run
  refused by the instance limit (§8.6) still leaves an entry that names no instance.
- **Note-off:** remove the entry and stop that instance. A note-off with no entry does
  nothing; stopping a handle that names no running instance does nothing.
- Entries are per note key only; instrument type and flags play no part.

The ABC sequencer never produces a second note-on for a sounding pitch without a note-off at
the same time first (`abc-dialect.md` §9.3), so the "old entry kept" case arises only when
events from other sources overlap.

<a id="s8-5"></a>
### 8.5 Cancelling a character's music

When a character's music is cancelled, the client removes every queued event of that
character from the queue and then queues, for every note key still registered for it, a
note-off due immediately (time 0, fresh sequence numbers). Sustained notes therefore end
with their release fade (§9.2). What triggers a cancel is outside the module that was read
(the stop command and leaving play are the expected triggers).

> **OPEN:** (Q1, cancel part) which events cancel a character's music (§5.1 holds the rest
> of Q1). Tessera (Decision): cancel on the stop command and when the character's performance ends or
> the character is removed.

<a id="s8-6"></a>
### 8.6 Instance limit (Confirmed (BIN + OBS))

Before any script run, the client counts the instances of that script key that are still
active on the character. The limit is the script's per-script property
`Script_MaxNumInstances` (`script-table.md` §2.4), taken as 1 when absent, not an int, or
below 1. If the count has reached the limit, the run is refused (logged, no sound). Both music
scripts carry 10, so at most **10** instances of each music script run at once per character
(quantized and non-quantized notes are counted separately). An instance stays active while
any of its nodes is running; a non-fire-and-forget Sound keeps it active until stopped or
until its sample ends. A non-fire-and-forget Sound whose request the sound manager refuses
ends at once (§9.1 step 7) and does not keep the instance active. One whose request was
skipped by the voice limit is reported as accepted (SND §6.6); how such a node ends is open
in SND (its open question 5).

> **OPEN:** (Q2) whether an instance whose only remaining activity is fire-and-forget sounds
> or the particle still counts as active. This decides whether fast plucked passages can hit
> the limit of 10. Suggested method: M4 on the Particle node's start and completion and on
> the instance count of the script-run check, or an owner test (a fast lute run of more than
> 10 notes within one second; listen for dropped notes).

---

<a id="s9"></a>
## 9. Sustain, release and decay

<a id="s9-1"></a>
### 9.1 Starting a note's sound (Confirmed (BIN), I15, I38, I40)

When a Sound node starts (after the generic node start has computed its intensity, §10.1),
the client takes the steps below. They are given for the field values that every instrument
Sound node has (§6.1); SND §9.2 gives the general algorithm, including steps that never
apply to instruments (internal-object mute, viewport redirect, random offsets).

1. If its sound-info DID is the invalid DID, nothing plays.
2. Its **volume** is its node intensity (§10.1), clamped to 0 … 1.
3. Position: the sound is attached to the character (`MoveWithEntity` = 1) at the character's
   position plus `Offset` (zero) plus a random offset (zero, since both random distances are
   0); it is not placed at the camera (`UseCameraPosition` = 0).
4. The request carries the channel, `RampOutTime`, the playback-rate factor (§7.3) and the
   `FireAndForget` flag.
5. **Fire-and-forget only** (`FireAndForget` = 1): if one flag of the running script's
   context is set (Q7), or if the volume is 0.005 or less, the node ends without asking for
   a sound.
6. The request goes to the sound manager, whose gates (§9.3; SND §6.6, §6.8) apply to both
   kinds. A non-fire-and-forget sound has no volume floor of its own, but like every request
   it is refused when its channel or the master volume is at or below 0.0002.
7. If the manager refuses a one-shot request, the node ends at once and nothing sounds. A
   looping request (the 12 `Bagpipe` drones) that fails the volume or distance gate is kept
   in the manager's pending list and started later (SND §6.8); how a note-off treats a
   drone that is still pending is part of Q6.
8. If it accepts: a **fire-and-forget** node ends at once and **no handle is kept**; nothing
   the script does later affects the sound. A **non-fire-and-forget** node keeps a running
   instance holding the sound's handle; stopping the script instance (§8.3 step 4) destroys
   it, which stops the sound (§9.2).

> **OPEN:** (Q7) which state sets the script-context flag of step 5 that suppresses
> fire-and-forget sounds (SND §9.2 step 4 and its open question 8). Instruments with
> fire-and-forget samples (§9.5) would be silent while it is set. Suggested method: M4 on the
> writers of that context flag, together with the `sound.md` writer.

<a id="s9-2"></a>
### 9.2 Note-off and the release fade (Confirmed (BIN))

Stopping a sound uses a fade time *T* = max(requested ramp, the sound's `RampOutTime`). Both
stop paths a note-off can take (the Sound node's own stop, and the engine's per-update stop of
voices whose script instance has gone) request 0, so *T* = `RampOutTime`. The engine's
per-update stop applies only to voices that are not fire-and-forget.

1. If *T* is 0, the sound stops at once.
2. Otherwise let *g*₀ be the sound's current gain and *D* = 1000 × *T* milliseconds. Every
   **10 ms** the elapsed time *e* grows by 10 and the gain is set to (1 − *e* / *D*) × *g*₀,
   until *e* reaches *D*; then the sound stops. The gain reaches 0 at *T* and the sound ends
   one step later.

For every non-fire-and-forget instrument *T* = 0.2 s: a held note is cut at note-off with a
200 ms linear fade.

<a id="s9-3"></a>
### 9.3 Fire-and-forget notes and the sound manager's gates (Confirmed (BIN + OBS))

A fire-and-forget sound plays its sample to the end whatever the note's length: the note-off
stops the script instance but not the sound. Its `RampOutTime` (0.2 for `Fiddle_short`,
`Fiddle_pizz` and `Handknells`) matters only when the sound engine stops the voice for its
own reasons (SND §6.5).

**Sound-manager gates for instrument notes (Confirmed (BIN) in SND, I40).** Applying SND
§6.6 and §6.8 to the instrument sound infos (all positional, flag 0x04; all non-looping
except the 12 `Bagpipe` drones):

| Condition at the moment the note starts | One-shot note (every instrument sample except the drones) | `Bagpipe` drone (loops) |
|---|---|---|
| `MaxPlayingSounds` voices (default 32) already playing | silently not started (reported as started) | accepted; may be virtualised |
| Master volume or the channel's volume (`PlayerMusic` or `QuantizedPlayerMusic`) ≤ 0.0002 | refused; the note never sounds | kept pending; starts when the volume is raised |
| The playing character is farther from the listener than the sound info's `max_distance` (30 m), or is in a landblock that is not loaded | refused; the note never sounds, even if the character comes closer while the note lasts | kept pending; starts when the character comes within range |

There is no voice stealing for these requests: a new one-shot note does not replace a
playing voice when the limit is reached. The listener is the active camera (SND §6.7), so
the 30 m test uses the camera position.

> **OPEN:** (Q6) how the engine's per-frame ranking (score (2 − `priority`) × distance, SND
> §6.6) treats playing instrument voices once the limit is reached, how a
> non-fire-and-forget note skipped by the limit ends (SND open question 5), and whether a
> note-off withdraws a `Bagpipe` drone that is still in the pending list. With 32 voices,
> a group of performers can reach the limit. Suggested method: answered by `sound.md` when
> its stealing pass (SND §6.6) is settled; an owner test with several performers could show dropped
> notes.

<a id="s9-4"></a>
### 9.4 Looping samples (Confirmed (BIN + OBS))

A sound whose sound info has flag bit 0 set loops its whole sample until stopped. Among
instruments only the 12 `Bagpipe` drones (`Note_C2`–`Note_B2`, 2.92 s samples) loop; they
are not fire-and-forget, so they sound from note-on to note-off and then fade over 0.2 s.
No fire-and-forget Sound node of either script names a looping sample.

<a id="s9-5"></a>
### 9.5 Behaviour classes (Confirmed; lengths I36)

| Class | Instruments (keys) | A note sounds for | At note-off |
|---|---|---|---|
| Sustained | 1, 2, 3, 9 (from `Note_C3`), 11, 16, 20, 21 (from `Note_G2`), 22, 23, 24 | min(note length, sample length): 5–10 s samples (§6.1) | 200 ms linear fade |
| Short, stoppable | 25; the `Fiddle_long` effects | min(note length, sample length): 0.37–0.54 s (25), 0.56–2.0 s (effects) | 200 ms fade if still sounding |
| Drone | 9 (`Note_C2`–`Note_B2`) | the note length (loops) | 200 ms linear fade |
| Plucked, struck, percussive | 4, 5, 6, 7, 8, 10, 12, 17, 18, 19, 26 | the sample length (0.25–1.72 s), whatever the note length | nothing |

Note lengths are 0.06–8 s (`abc-dialect.md` §8.3). A sustained note longer than its sample
ends when the sample ends (no loop). For example a `Clarinet` note of 7 s stops after about
5.5–5.9 s; a `Fiddle_vib` note of 8 s sounds its full 8 s (samples 8.13–10.23 s).

> **Note:** "sustained" here describes what the data does. MA's per-instrument sustain notes
> agree with these classes; the one difference is the instrument with 0.37–0.54 s samples,
> which MA lists as not sustainable: that matches its sample length, not its `FireAndForget`
> value.

---

<a id="s10"></a>
## 10. Loudness and position

<a id="s10-1"></a>
### 10.1 Intensity (Confirmed (BIN))

Each running node has an intensity, computed when it starts (I14, re-read under BIN-R):

1. The parent intensity *p* is 1.0, or, if the node's `UseParentIntensity` is 1, the
   intensity of the context the node starts in (its parent's).
2. The node's own **factor** is its `IntensityMultiplier` (the driven value if the field is
   driven). If `IntensityMultiplierVariance` is exactly 0 the factor is the multiplier
   **unchanged, not clamped**. Otherwise a uniform random term within ±variance is added and
   the result is clamped to 0 … 1.
3. The node's intensity is factor × *p*. The node runs in a context that carries this
   intensity; its children start from it.

**Sound node volume (I38).** A Sound node with `UseParentIntensity` = 1 takes as its volume
its **own** node intensity from step 3, which includes its own multiplier, clamped to 0 … 1
(§9.1 step 2). (With `UseParentIntensity` = 0 it draws its multiplier and variance afresh
and clamps the result to 0 … 1; SND §9.2 step 7. No instrument node does this.)

For instruments every multiplier on the path is in 0 … 1 (the instrument Collection's
0.34–1.0 of §6.1, the root's `Music_Note_Volume` of 0 … 1, and 1.0 elsewhere, including
every Sound node) and every variance is 0. The volume is therefore: root factor ×
instrument Collection multiplier, and no clamp ever changes it.

**Agreement with `sound.md`.** SND §9.2 step 7 also uses the node's own intensity: the two
specs agree on that point. They disagree on step 2: SND §9.3 clamps the factor to 0 … 1
always, while the multiplier helper read under BIN-R returns the stored multiplier
unchanged when the variance is exactly 0. The difference matters only for a node with variance 0 and a
multiplier outside 0 … 1. Over all 632,847 nodes of the 7,524 decodable script tables and
28,992 scriptlets there are 7 such nodes (3 Sound, 3 Particle and 1 node of kind 17; values
1.3–2 and one of 400,000), none on an instrument path (I42). A Sound node among them still
ends up clamped by §9.1 step 2; a non-Sound node passes its unclamped intensity to its
children.

> **OPEN:** (Q8) cross-spec disagreement with SND §9.3 on whether the intensity factor is
> clamped when `IntensityMultiplierVariance` is 0. This spec's reading (BIN-R): not clamped.
> It does not affect instruments. The `sound.md` writer should re-check the factor helper
> and either correct SND §9.3 or record why the readings differ; the matching OPEN belongs
> in `sound.md` (not edited here, §15).

<a id="s10-2"></a>
### 10.2 Note volume (Confirmed)

In 0x10000BCE the root's factor is `Music_Note_Volume` = velocity / 127 (§5.2); in
0x10000BCD the root's factor is 1.0 and the velocity is ignored. Velocities from ABC volume
marks (`abc-dialect.md` §7.11): `pppp` and `ppp` 30, `pp` 45, `p` 60, `mp` 75, `mf` 90 (the
default), `f` 105, `ff` 120, `fff` and `ffff` 127.

Resulting volume = velocity / 127 × instrument multiplier. Examples at the default velocity 90
(0.7087): `Lute` 0.241, `Theorbo` 0.322, `Harp_Special` 0.319, `Fiddle_short` 0.354,
`Flute`, `Bagpipe` and `Fiddle_pizz` 0.461, `Clarinet`, `Horn`, `Pibgorn` and the default
set 0.496, `Lute_of_Ages` 0.546, bassoons 0.673, and 0.709 for the instruments with
multiplier 1.0.

<a id="s10-3"></a>
### 10.3 Audibility floor (Confirmed (BIN))

A fire-and-forget note whose volume is 0.005 or less is not started (§9.1). With the
velocities above the smallest instrument volume is 30 / 127 × 0.34 = 0.080, so the floor
matters only for velocity 0 (a packed velocity above 127, §5.2).

<a id="s10-4"></a>
### 10.4 Channels and settings (Confirmed (names); mixing per `sound.md` §6.1, §7)

The channel of every instrument Sound node is 6 `PlayerMusic` or 5 `QuantizedPlayerMusic`
(§5.8). The `[Sound]` section of `UserPreferences.ini` (`install-layout.md` §8.3) has one
`<label>Volume` key for each label of the channel mapper (§3.3), including
`PlayerMusicVolume` and `QuantizedPlayerMusicVolume`, plus `MasterVolume`. The gain sent to
the audio device is min(*v*, *v* × *a* × master × channel volume), with *v* the note's
volume (§10.2) and *a* the distance factor (§10.5) (SND §6.1, normative). A channel or
master volume at or below 0.0002 refuses new notes (§9.3).

SND §7.2 reports that channel 5 (`QuantizedPlayerMusic`) carries channel flag 0x2, whose
voices are started through a separate pool (Inferred there); channel 5 has no other flag.

> **OPEN:** (Q9) what the separate voice pool of channel 5 does (SND §7.2, flag 0x2), in
> particular whether it delays or aligns quantized notes. Until it is known, Tessera plays
> channel 5 voices like channel 6 voices (Decision). Suggested method: M4 on the voice
> creation path for flag 0x2, together with the `sound.md` writer.

<a id="s10-5"></a>
### 10.5 Position and distance (Confirmed (BIN); `sound.md` §6.2, §6.7 normative)

Instrument sounds follow the playing character (§9.1). The distance law applies only to
sound infos with flag 0x04 or 0x40 (SND §4.3, §6.2); a sound info with neither bit plays at
its volume wherever it is. Every instrument sound info has flag 0x04 (I39), so the law
applies to every instrument note. With *d* the distance from the listener to the sound,
*d*₀ the sound info's `min_distance` and *d*₁ its `max_distance` (§4.2): the distance factor
is 1 when *d* ≤ *d*₀, else 0 when *d* ≥ *d*₁, else 1 − (*d* − *d*₀) / (*d*₁ − *d*₀) (the
tests in this order). For instrument samples *d*₀ = 3 m (1 m for `Pibgorn` and `Handknells`)
and *d*₁ = 30 m: music is at full strength within 3 m and silent beyond 30 m. The audio
library runs with its own distance model switched off; the factor above is applied by the
client and recomputed every frame.

Because the sounds have flag 0x04 they are also panned from the source position and feed
the active reverb environment (SND §6.7, §8). The listener is the active camera (SND §6.7):
*d* is measured from the camera, so the player's own notes are attenuated too once the
camera is farther than *d*₀ from the character (for example 0.926 at 5 m for a 3 m / 30 m
sample). A note whose character is beyond *d*₁ when it starts is not started at all
(§9.3).

---

<a id="s11"></a>
## 11. Note range per instrument (Confirmed)

Every instrument accepts the same 37 note keys, `Note_C2` … `Note_C5` (ABC `C,` … `c'`,
pitch 48–84). Notes outside that range are refused by the ABC reader (`abc-dialect.md` §7.3)
and map to key 0 elsewhere (§7.1), which no note switch has a case for. Within the range:

| Instruments | Sounding notes | Written range sounds as (§7.4, measured, Probable) |
|---|---|---|
| `Theorbo` | all 37 | C1–C4 |
| `Horn`, `Lute`, `Lute_of_Ages`, `Harp`, `Harp_Special`, `Bassoon`, `Bassoon_vib`, `Bassoon_short` | all 37 | C2–C5 |
| `Clarinet`, `Bagpipe` (lowest 12 are looping drones), `Fiddle_short`, `Fiddle_pizz`, `Fiddle_vib`, `Fiddle_strong`, `Fiddle_basic` | all 37 | C3–C6 |
| `Fiddle_long` | `Note_C2`–`Note_D2` effects; `Note_Eb2`–`Note_Gb2` silent; `Note_G2`–`Note_C5` | G3–C6 for the notes |
| `Flute`, `Pibgorn` | all 37 | C4–C7 |
| `Handknells` | all 37 | C5–C8 (bells) |
| `Drum` | all 37, each a different unpitched drum sound | — |
| `Cowbell`, `MoreCowbell` | every key plays the one sample | — |

---

<a id="s12"></a>
## 12. Tessera design (Decision)

> **Decision:** build the instrument table of §5.7 at load from the two scripts found
> through the player avatar's closure (§4.1), and use the script tree only through that table.
> Keep the script semantics of §5.6 so that data changes in a patch are followed.

> **Decision:** schedule note-ons and note-offs on the audio clock with sample accuracy rather
> than on a frame tick (§8.3). Keep the ordering rule of §8.2, the 0.5 s late-drop rule of
> §8.3 and the registry rule of §8.4.

> **Decision:** implement the release as a linear gain ramp from the current gain to 0 over
> `RampOutTime`, evaluated per audio block (equivalent to the 10 ms steps of §9.2 within one
> step), then free the voice.

> **Decision:** in the sandbox, the local player's instrument type comes from the equipped
> instrument item as in §4.5, or from a debug choice when no item model exists. Live notes
> carry velocity 90 (the value the client's live-play handler uses, §5.1), the from-song
> flag 0, and the quantized flag equal to the quantize option, with no delay to a grid,
> until Q1 is answered.

> **Decision:** apply the sound-manager gates of §9.3 (voice limit, channel and master volume,
> start beyond `max_distance`) to instrument notes as `sound.md` specifies them; do not add
> voice stealing for notes.

> **Decision:** loop samples whose sound info has flag bit 0 by repeating the whole decoded
> sample; apply the playback-rate factor of §7.3 by resampling.

> **OPEN (implementation, 2026-10-07):** `tessera_audio::instruments` had to choose where
> this spec is silent. Please confirm or state the original's behaviour:
> (a) **Q2:** an instance whose sounds are all fire-and-forget completes at once and does not
> count toward `Script_MaxNumInstances` (the Particle node is not run), following the
> node-completion rules of `sound.md` §9.6; so a fast plucked run never hits the limit of 10.
> (b) `/play` for a character whose performance is still queued or playing cancels that
> performance first (§8.5) and then queues the new one.
> (c) The end of a performance (`abc-dialect.md` §9.4 end event) cancels the character's music
> with the note-offs due at the end time rather than at time 0 (§8.5 Decision).
> (d) `/playstart` starts every waiting performance 0.7 s after the start command, the same
> delay as a solo start (`abc-dialect.md` §11 OPEN (3)).
> (e) For sample-accurate scheduling (§12) the player processes queued events up to 0.1 s
> before they are due, so registry and instance-limit decisions (§8.4, §8.6) are taken up to
> 0.1 s before the event time; the late-drop test (§8.3 step 2) still uses the audio clock.
> (f) A Switch in the music scripts on a property that is not one of the five note
> parameters of §5.2 (none in this install) reads no value and takes its Default port, as
> does a case with an operator other than 1–3.
> (g) A note handed to the engine ahead of its time under (e) counts as still queued for the
> cancel of §8.5: the cancel withdraws it unless it has started by the player's next update
> (Tessera's frame tick), fire-and-forget notes included.
> (h) When a note parameter is not set because the dictionary gives it another type (§5.2),
> a field driven by it (the root's `Music_Note_Volume` multiplier, §10.2) keeps its stored
> value; what the original's driven field reads then is not stated.

> **OPEN (implementation, client, 2026-10-07):** the client (`apps/client/src/audio/player_music.rs`)
> chose: (a) the walker's `--avatar` items carry no slot, so §4.5's slot mask is the item's
> `Inventory_DefaultSlot`; items are equipped in the order given and the last instrument item
> sets the type. (b) The debug choice of the §12 sandbox Decision is the chat command
> `/instrument <label|number|item>`; it overrides the equipped item until `item` is given.
> (c) With neither an instrument item nor a debug choice, `/play` reads and checks the song
> but nothing plays (Tessera's own message); whether the original refuses earlier and with
> which message is not stated. (d) The player music runs its own engine instance with the
> walker's backend choice, with the performer and the listener both at the origin; placing the
> performer at the avatar and sharing the world-audio engine (§10.5) wait for an engine hand-off
> in the walker.

---

<a id="s13"></a>
## 13. Invariants (Confirmed over all records unless stated)

- **IN1**: in every avatar class-table closure exactly one table defines 0x10000BCD and
  0x10000BCE, and it is the same table (0x0700042F here).
- **IN2**: both scripts consist only of Collection, Switch, Sound and Particle nodes; every
  Switch case has one value and operator Equal; no `Music_Note` switch has a Default port.
- **IN3**: every Sound node of both scripts has `MoveWithEntity` 1, `UseCameraPosition` 0,
  zero random distances, zero `Offset`, `Pitch` 0, channel 5 or 6, and `PitchVariance` 0
  except the cowbell nodes (0.02) and one default node (0.2).
- **IN4**: every node on a root-to-Sound path has `UseParentIntensity` 1, variance 0 and
  probability 1; the only driven field is the root multiplier of 0x10000BCE.
- **IN5**: for every instrument case and branch, the note cases with a sound are all 37 keys,
  except `Fiddle_long` (33) and `Horn` in the song branch of 0x10000BCD (36); the cowbells have
  no note switch.
- **IN6**: every sound info reached names a 0x0A record that exists and holds Ogg Vorbis or
  RIFF WAVE PCM, mono, 44,100 Hz.
- **IN7**: every reached sound info with flag bit 0 belongs to a Sound node with
  `FireAndForget` 0.
- **IN8**: both music scripts carry `Script_MaxNumInstances` = 10.
- **IN9**: every 0x2A record decodes exactly by §4.2 and its `sound` DID has prefix 0x0A.
- **IN10**: the label rule of §7.1 yields a key for each of the 37 pitches, and the 37 keys are
  distinct and non-zero.
- **IN11**: the `MusicControl` map has 37 entries whose action and note labels share the note
  part.
- **IN12**: every property set with `Item_Music_Instrument_Type` also has
  `Usage_IsMusicalInstrument` = true and `Item_EquipmentCategory` = `Instrument`.
- **IN13**: every sound info reached by the two music scripts has flag 0x04 set and no bits
  other than 0x01 and 0x04 (I39).
- **IN14**: every node on a root-to-Sound path of both music scripts has a multiplier in
  0 … 1 and variance 0, so the clamp question Q8 cannot change an instrument volume (I42).
- **IN15**: every instrument item (category `Instrument`) has a non-zero `PhysObj` in its own
  property set (§4.5).
- **IN16** (synthetic): a packed note whose bits 8–23 are not zero is played, registered and
  turned off under the key in bits 0–7 (§5.1, §14.2).

---

<a id="s14"></a>
## 14. Synthetic worked examples (for tests)

<a id="s14-1"></a>
### 14.1 A sound-info record

34 bytes: DID 0x2A001234, flags 4, `a` = `b` = −1.0, priority 1.0, distances 3.0 and 30.0,
sound 0x0A005678, name of one byte `x`:

`34 12 00 2A` `04 00 00 00` `00 00 80 BF` `00 00 80 BF` `00 00 80 3F` `00 00 40 40`
`00 00 F0 41` `78 56 00 0A` `01` `78`

Decodes to loop = no, min 3 m, max 30 m. With flags `05 00 00 00` it loops. Truncating the
record anywhere before the last byte must give a decode error.

<a id="s14-2"></a>
### 14.2 Pitch, key and packed note

Pitch 69 (ABC `A`): *i* = 21, name A, octave 3, label `Note_A3`, key 11 in this install. With
velocity 105 (`+f+`) the packed note is 0x6900000B; `Music_Note_Volume` = 105 / 127 =
0.826772. A packed note 0x8000000B (velocity bits 128) gives volume 0. Packing key 11 with
velocity 128 gives the word 0x0000000B (the velocity is dropped, §5.1), which also plays at
volume 0. A word 0x6900170B (bits 8–23 not zero) is read as note key 0x0B and velocity 105:
its note-on registers key 0x0B, and a note-off word 0x0000000B stops it.

<a id="s14-3"></a>
### 14.3 Script choice and table lookup

A synthetic tree built by the test's own encoder: root Collection (driven multiplier from
`Music_Note_Volume`) → Switch on `Music_Note_From_Song` with case 1 only → Switch on
`Music_Instrument_Type` with case 4 → Collection (multiplier 0.5) → Switch on `Music_Note`
with cases 28 → Sound A (FF 1, ramp 0) and 12 → Sound B (FF 0, ramp 0.2); the instrument
switch also has a Default → Collection (0.7) → Switch on `Music_Note` with case 28 → Sound C.
Expected table entries for the non-quantized key:

| from song | quantized | instrument | note key | Result |
|---|---|---|---|---|
| 1 | 0 | 4 | 28 | Sound A, intensity 0.5 × velocity / 127 |
| 1 | 0 | 4 | 12 | Sound B, intensity 0.5 × velocity / 127 |
| 1 | 0 | 4 | 2 | nothing |
| 1 | 0 | 99 | 28 | Sound C, intensity 0.7 × velocity / 127 |
| 0 | 0 | 4 | 28 | nothing (no Default on the from-song switch) |

<a id="s14-4"></a>
### 14.4 Loudness

`Lute` song note, velocity 90, listener 10 m away, `min_distance` 3, `max_distance` 30,
flags 4: volume = 90 / 127 × 0.34 = 0.240945; distance factor = 1 − (10 − 3) / (30 − 3) =
0.740741; product 0.178478. With master 1.0 and `PlayerMusic` volume 0.5 the sent gain is
min(0.240945, 0.178478 × 1.0 × 0.5) = 0.089239 (SND §6.1). At 2 m the factor is 1; at 30 m
it is 0. A note that starts while the listener is 31 m away is not started (§9.3); with
flags 0 instead of 4 the factor would be 1 at any distance (§10.5).

Node intensity (synthetic tree, §10.1): root Collection factor 0.6 (driven) → Collection
multiplier 0.5 → Sound node with `UseParentIntensity` 1 and multiplier 0.8, all variances 0:
the Sound's node intensity is 0.6 × 0.5 × 0.8 = 0.24, and that is its volume. If the Sound's
multiplier were 2.0 with variance 0, its intensity would be 0.6 (not clamped, factor 2.0
× 0.3) and its volume 0.6; with multiplier 2.0 and variance 0.1 the factor is clamped to
1.0 and the volume is 0.3.

<a id="s14-5"></a>
### 14.5 Release

A sustained note at gain 0.6 receives its note-off; `RampOutTime` 0.2 s gives *D* = 200 ms.
Gains after 10, 50, 100 and 200 ms: 0.57, 0.45, 0.30, 0.0; the voice stops at the next step.
With `RampOutTime` 0 the voice stops at once.

<a id="s14-6"></a>
### 14.6 Queue

Events: note-off key 29 at 1.00000 s (sequence 7), note-on key 29 at 1.00004 s (sequence 8),
note-on key 12 at 0.90 s (sequence 5). Processed at *now* = 1.45 s: the note-on at 0.90 s is
0.55 s late and is discarded; the off (sequence 7) is processed before the on (sequence 8)
because their times differ by less than 0.0001 s. If instead *now* = 1.30 s, all three are
processed, in the order key 12 on, key 29 off, key 29 on.

<a id="s14-7"></a>
### 14.7 Instance limit

Eleven non-quantized `Clarinet` notes of eleven different pitches start 0.1 s apart and each
lasts 3 s. Notes 1–10 start; note 11 is registered but its run is refused because ten
instances of 0x10000BCE are active, so it makes no sound; its note-off removes the entry and
stops nothing.

---

<a id="s15"></a>
## 15. Corrections and additions to other specs

| Spec | Statement | Correction or addition |
|---|---|---|
| `abc-dialect.md` §9.5 OPEN | Whether the slot numbers are values of an enumeration stored in the data | Yes: they are the keys of enum mapper 0x2300015E (`Music_Note`), labels `Note_C2` … `Note_C5` (§3.2, §7.1). |
| `abc-dialect.md` §9.3 OPEN | A re-struck pitch with a different volume reportedly plays nothing | The playback side does not refuse or silence it: the extra note-off removes the registry entry and stops the old instance, the note-on starts a new one (§8.4). The report is not explained by the client's playback path; owner check still useful. |
| `abc-dialect.md` §0, §7.11 | Loudness of the volume values left to `instruments.md`/`sound.md` | Volume = value / 127 × instrument multiplier for non-quantized notes; ignored for quantized notes (§10.2). |
| `records.md` §2.1, §9.1 OPEN | Sound-info (0x2A) layout not specified | Layout in §4.2 (45,544 of 45,544); `sound.md` normative once Accepted. |
| `script-table.md` §4.2 | `UseParentIntensity`, `IntensityMultiplier`, `IntensityMultiplierVariance` described by name only | Rule in §10.1 (BIN). |
| `script-table.md` §5.3 | Sound node fields listed without semantics | Semantics of `FireAndForget`, `RampOutTime`, `Pitch`, `PitchVariance`, `Channel`, `MoveWithEntity`, random distances and the audibility floor in §7.3, §9.1–§9.3 (BIN; the general form is SND §9.1–§9.2). Constructor defaults (I33): `FireAndForget` 1, `MoveWithEntity` 1, `UseCameraPosition` 0, random distances 0, `Offset` (0, 0, 0), channel 0, `RampOutTime` 0, `Pitch` 0, `PitchVariance` 0, sound info invalid. |
| `sound.md` §9.3 | The node intensity factor is clamp(`IntensityMultiplier` + *u*, 0, 1) in every case | BIN-R: when the variance is exactly 0 the factor is the stored multiplier, not clamped; the clamp applies only when a random term is added. Affects 7 nodes in the data, no instrument (§10.1, I42). Open as Q8 here; the `sound.md` writer should add the matching OPEN or correct §9.3. |
| `sound.md` §9.2 step 7 | A Sound node with `UseParentIntensity` = 1 uses the node's intensity | Agreed by BIN-R (I38); no change. |
| `sound.md` §6.6, §6.8 | Voice limit and refused requests | Applied to instruments in §9.3 here; no change. |
| `sound.md` §7.2 | Channel 5 flag 0x2 "separate pool", out of scope there | Open for instruments as Q9 here. |
| `script-table.md` §2.4 | Per-script property sets described as framing | `Script_MaxNumInstances` semantics in §8.6 (BIN). |
| `install-layout.md` §8.3 | `[Sound]` keys listed | Each `<label>Volume` key matches a label of channel mapper 0x2300034D (§10.4). |
| `ARCHITECTURE.md` §14 OPEN | Which sound records hold the instrument samples and how per-note pitch is derived | Answered by §4–§7 (neutral doc; its owner may update it). |
| `research/notes/engine-client-re.md` §7 | "Notes: ABC `C,` to `c'` (MIDI 36–72 …)" | The client's own numbers are 48–84; 36–72 is the sounding range of the octave-0 instruments (§7.1, §7.4). |
| `research/notes/data-semantics.md` §1 | 0x0700042F described as "Notes" | It is the shared sound-script table (292 scripts) that also holds the two music scripts (§4.1). |

---

<a id="s16"></a>
## 16. Open questions

Each question has a `> **OPEN:**` callout at the place named in "Where".

| # | Question | Where | State and suggested method |
|---|---|---|---|
| Q1 | Live play after the dispatch: whether live notes keep velocity 90, who sets the quantized and from-song flags, the quantize grid, and what triggers the cancel of §8.5 | §5.1, §8.5 | Partly answered (I41: velocity 90, note-on at press, note-off at release). Owner observation as in §5.1. M4 only where it stays clear of the network path (decision 0004). |
| Q2 | Whether instances with only fire-and-forget sounds or particles left count toward `Script_MaxNumInstances` | §8.6 | M4 (Particle node completion, instance count), or owner test: a fast lute run of more than 10 notes within one second |
| Q3 | Effect of sound-info flag bits 0x02 and 0x10 | §4.2 | Mostly answered by SND §4.1 and §4.3 (`a`, `b` unused; 0x04 positional). The two remaining bits occur in no instrument sound info; open in SND (its question 3). |
| Q4 | Whether a note ever arrives with both the quantized and the from-song flag set | §5.8 | Owner observation with the quantize option on during `/play` |
| Q5 | Exact octave of the `Handknells` samples | §7.4 | Partial-series fit or listening test; not needed for playback |
| Q6 | The per-frame voice ranking once the voice limit is reached; how a tracked note skipped by the limit ends; whether a note-off withdraws a pending `Bagpipe` drone | §9.3 | Partly answered by SND §6.6 and §6.8 (no stealing for new one-shots; refused or pending as in §9.3). Rest: SND open question 5. |
| Q7 | Which state sets the script-context flag that suppresses fire-and-forget sounds | §9.1 | M4 on the writers of that flag, with `sound.md` (SND §9.2 step 4, its question 8) |
| Q8 | Cross-spec: whether the intensity factor is clamped when the variance is 0 (this spec: no; SND §9.3: yes) | §10.1 | Re-check by the `sound.md` writer; no effect on instruments (IN14) |
| Q9 | What the separate voice pool of channel 5 (`QuantizedPlayerMusic`, flag 0x2) does to quantized notes | §10.4 | M4 on the voice-creation path, with `sound.md` (SND §7.2) |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | Revision for the third-round re-review of 2026-10-06 (status stays Review). §4.4 and I26 corrected: the older copy 0x070017F9 does **not** name other sound infos for `Bagpipe`. Every one of its 2,120 Sound nodes names the same sound info as the matching case of 0x0700042F (335 distinct, all in the 746 instrument set), with the same channel, `FireAndForget`, `PitchVariance` and other fields, except `RampOutTime` 0 on the 222 `Bagpipe` nodes (0.2 in 0x0700042F). Added the path-intensity differences for `Flute`, `Lute`, `Theorbo`, `Bagpipe` and `Pibgorn` (1,110 Sound nodes), found by the same full comparison. Players are not affected (no avatar closure reaches 0x070017F9). |
| 2026-10-06 | Revision for the re-review of 2026-10-05 (status stays Review). I31 and §6.3: script 0x1000274C of table 0x07000078 has two fire-and-forget Sound nodes on channel 1, naming 0x2A00A826 **and** 0x2A00A824 (was: one effect, 0x2A00A826); re-checked against every Sound node of all decodable tables and scriptlets. §4.2: the lead-in of the value table no longer calls 0x2A00A824 unused; it is unreferenced only by the two music scripts. §6.2: the 0x2A00A824 row names its one user. |
| 2026-10-05 | Revision for the spec review of the same day (status stays Review). Second M4 pass (BIN-R). §5.1, §5.2, §8.4: one packed-note rule (key read from bits 0–7 everywhere, written as 24 bits; velocity above 127 dropped by the packers). §5.1: live-play handler facts (velocity 90, note-on at press, note-off at release; I41); UI option described instead of quoted. §9.1: rewritten; non-fire-and-forget sounds are subject to the sound-manager gates. §9.3: gate table for instrument notes. §10.1: the Sound node's own multiplier enters its volume (agrees with `sound.md`); clamp only when the variance is not 0 (disagrees with `sound.md` §9.3, Q8; census I42). §10.4: gain chain and channel-5 pool (Q9). §10.5: distance law only for flags 0x04/0x40, panning, reverb, camera listener. §4.2: flag 0x04 and fields `a`, `b` from `sound.md`; Q3 narrowed. §4.5: `PhysObj` copy and the no-`PhysObj` case; slot bits. §6.3: other users of instrument sound infos corrected (adds table 0x07000078). Callouts for Q1–Q9 at each unknown. Provenance rows I30–I42, tags BIN-R and SND; I14, I15, I22, I23, I29 revised. Invariants IN13–IN16; examples in §14.2 and §14.4; `sound.md` rows in §15; §12 live velocity 90 and the gate decision. |
| 2026-10-05 | First version: identifiers, the music scripts of table 0x0700042F and their tree, sound-info layout, sample census, per-instrument table, pitch mapping and measured octaves, playback queue, registry, cancel and instance limit, sustain and release behaviour, loudness and distance, note ranges, Tessera decisions, invariants, synthetic examples, corrections and open questions. BIN analysis of client build 4906.0071.1794.4035. |
