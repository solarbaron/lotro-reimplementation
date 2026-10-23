<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Player-music instruments: sample records, note mapping, sustain and sample choice

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review 2026-10-23 by spec-review:spec-gameplay:2 (independent session), a session that wrote no part of the 2026-10-23 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, revision 3, bookkeeping only: the two P4 implementation callouts of §12 are marked Answered, pointing to the answers already in §12 that the re-review of revision 2 passed; no fact, rule or section changed; no implemented behaviour changes; awaiting independent review). Before: **Accepted** (P4 revision 2 re-reviewed 2026-10-11: CLEAN_ROOM.md §5.3 passed, every item; B1–B3 of the first review resolved). Revision 2: the script checks before `/play` (I58, I43, I54; §8.8.1, §8.8.2, §8.8.6, §8.8.7, Q12; P4 Decisions 2 and 8), the catch-up measured against *S* + *P* (§8.7 Consequences, §8.8.4, P4 Decision 4, IN22, §14.10), fed starts take *S* and *A* from the scenario feed (P4 Decision 3), answers to the P4 implementation OPENs, counts and figures (OBS-S, §8.9). Revision 1 (P4): additive sections on the synchronised fellowship start — §8.7 (performance stepper), §8.8 (synchronised group start), §8.9 (**start tolerance**), new Decisions in §12, examples §14.8–§14.11, invariants IN17–IN21, provenance BIN-S, OBS-S, PLG and rows I43–I57, §15 rows, Q10–Q14. Every section and anchor of the Accepted text is unchanged. Before: Accepted (re-review 2026-10-06, fourth round) |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent); the P4 revision was reviewed 2026-10-11 by a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of either spec, their revisions or the P4 code (changes requested); revision 2 was re-reviewed 2026-10-11 by a second such session that also wrote no part of the first review (passed) |
| **Review record** | **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-gameplay:2 (independent session)): **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; bookkeeping only; both Answered callouts agree with "Answers to the P4 implementation OPENs above" (revision 2, already Accepted). Earlier: **Re-review 2026-10-11 of P4 revision 2** (commit 648a815; `git diff bb43e74..648a815` of `instruments.md` §1 rows BIN-S, OBS-S, I43–I58, §8.7–§8.9, §12 answers, P4 Decisions 1–8 and the answers to the P4 implementation OPENs, §13 IN17–IN22, §14.8–§14.11, §15, §16 Q10–Q14, and of `abc-dialect.md` §1 OBS-S, PV28–PV33, §8.5, §10.4, §11, §11.1, §16, §17), by a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of either spec, their revisions, the first review or the P4 code, against CLEAN_ROOM.md §5.3: **passed**, every item. Functional facts only; no code or pseudo-code (§8.7 and §12 Decision 4 are plain-word steps and conditions; no listing structure, no non-interoperability names; `Channeling_CurrentChannelingDID` and `Item_Music_Channeling_State` are property names of the data dictionary, needed to read the check); no game text (tokens, keys, argument IDs and PIDs only), keys, hostnames or traffic; every new fact has a row (I58 new, I43 and I54 revised) with its mark; invariants IN17–IN22 testable; unknowns are OPEN (Q10–Q14); decision 0004 kept, no closed gate. **First-review items.** B1 resolved (I58, I43, I54, §8.8.1 checks 1–3, §8.8.2, the §8.8.6 and §8.8.7 rows, Q12; §12 P4 Decision 2 states the group of one as a deliberate difference and Decision 8 treats the music-mode check as passed; `abc-dialect.md` PV28, PV33, §11 answers (1) and (f), §11.1 items 1 and 3 agree). B2 resolved (§8.7 Consequences, §8.8.4 and Decision 4 measure against *S* + *P* with *P* = 0; IN22, §14.10). B3 resolved (Decision 3 takes *S* and *A* of a delivered start from the event, as `scenario-feed.md` §6.7, Accepted, states; *t* + 0.7 only for a local start with a backend that refuses `StartPerformance`). N1–N3 and N5 taken up; N4 left (no effect on Tessera), acceptable. Revision 2 changes no implemented behaviour on branch p4: the catch-up of `tessera_audio::instruments` already compares with *S* + *P*, the client already places *S* from the feed, and Decisions 2 and 8 keep the client's command handling. **Own spot checks.** M1 (Tessera's `dattool`, read-only, own dirty scratch): record 0x56000000 holds 18 call sites of package-0x07DE natives (9 IDs) in 14 bodies; the helper the `/play` body calls after its usage check runs the music-mode check first (failure: key 0x10000335, end), then, only with the sync flag, the group check (failure: key 0x100003B0, end), then native 0x07DE0001; `/playstart` runs only the group check; the music-mode toggle command calls the same music-mode check; that check requires PID 0x10000BC3 of the player non-zero, an item found with argument 0x20040000, and the item's PID 0x10000BA1 equal to the player's value; dictionary 0x34000000 names the two PIDs as in I58; the own ready notice sets no argument on its empty-title branch (N2), the other-member notice sets name and title, both posted with output type 8; table 0x2500808A has 45 entries and contains neither key 0x10000335 nor 0x100003B0. M4 (own private project copy, decision 0011, build 4906.0071.1794.4035; question: does the catch-up compare *S* + *P* with *P* = 0 and always take the first event at or after *now* + 0.5, as IN22 states; why: BIN-S is its only source): the stepper tick, its catch-up and the finish agree with §8.7 steps 1–8 (catch-up constant 0.5 read from the executable's data; note-offs at *S* + *P* + *k*; events taken while *k* > 0 with zero-delta followers; *extra* = 0 and the tick ends if the end was taken; otherwise *A* − *ahead* below *A*, else *elapsed*; the next wake time); the synchronised-start entry and the music-end key agree with I45, I46, I52. §14.8–§14.10, including the **zG** case and the sample numbers of §14.9, recomputed: correct. **Non-blocking notes** (no change needed): (a) I58, §8.8.1: the instrument-item lookup of the music-mode check applies two successive checks to the item it finds, not one; "not traced further" covers it. (b) §8.7 State: the original's "not yet" wake time is the value 0, which the wait test always passes and which gives *elapsed* 0; equivalent to the text. Earlier: Review 2026-10-11 of the P4 revision (commit 7b55efa): **changes requested** (B1–B3, N1–N5), answered by revision 2 (change log). Earlier: Re-review 2026-10-06 (fourth round): **passed**, Accepted. Full record moved to the review log (decision 0018). |
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
- the performance stepper, the synchronised fellowship start and the **start tolerance**
  (§8.7–§8.9, revision 2026-10-11);
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
| BIN-S | Third M4 pass, same client build 4906.0071.1794.4035 (`lotro_live`), 2026-10-11, under [decision 0011](../decisions/0011-standing-m4-authorisation.md), for P4 (synchronised fellowship music). **Questions:** (12) what the client does with `/play … sync` and with a synchronised start: where the start time comes from and what the second start value means (`abc-dialect.md` §11 OPEN (3)); (13) how a started performance's events are handed on over time, including the 0.5 s constant of `abc-dialect.md` §8.5 OPEN; (14) which client paths withdraw a waiting synced part or end a running performance; (15) which notices the client module itself shows. **Why M1–M3 were not enough:** the archives hold the message entries and the game script that calls the client (OBS-S), but not the client's scheduling; published material gives only the command syntax and the fact that a group member starts the group (WEB, PLG); the owner has no second account and no recordings for an in-game trial (M3). **Read:** the player-music module's play entry and its synced branch, the synchronised-start entry, the start-of-performance function, the stepper and its catch-up and finish functions, the music-end key handler, the module's constructor (its subscriptions) and the handler that withdraws the waiting part, the registration of the music natives (package 0x07DE), and the module functions behind natives 0x07DE0004, 0x07DE0005, 0x07DE0006 and 0x07DE000A (queue insertions and the cancel). **Not examined** (decision 0004): the client-to-game-script posts the module makes and what the script does with them; the content, order and transport of the server notifications that the game script handles; the native that reads a time stamp from such a notification and whether it translates clocks; server behaviour. Nothing about network, login, keys, launcher or anti-tamper. |
| OBS-S | User install, 2026-10-11: the class library's code block (record 0x56000000, [`wlib-bytecode.md`](wlib-bytecode.md)) read with Tessera's disassembler for every call site of the package-0x07DE natives (18 sites in 14 bodies; revision 2: was "12 sites"), the bodies around them and the bodies of the `/play` and `/playstart` commands, including the helper that the `/play` command calls with its words (its two checks, I58, I54) and the music-mode check it shares with the music-mode toggle command; string table 0x2500808A (`UI_Music`, all 45 entries: tokens and argument IDs only; revision 2: was "46"); the command-word table 0x250001AB (named key 0x10000007, label `Commands`) and the command-help table 0x250001AA (tokens only, meaning summarised, no text quoted). |
| PLG | A published third-party plugin's localisation strings for its music helper (dirty-room plugin census of M5), read for usage only: it builds the fellowship command as `/play <file> sync <part>` (word before number) and starts with `/playstart`. No code or text copied. |
| WEB | As in [`abc-dialect.md`](abc-dialect.md) §1: published usage guides (`/play <file> [part] sync`, `/playstart` by a fellowship member). |

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
| I43 | `/play … sync` that has passed the two script checks of §8.8.1 (I58, I54) reads and checks the part like a solo `/play`; on success the client keeps the performance as the local player's **single waiting part** (an earlier waiting part is discarded), starts nothing, shows no begin notice and posts a ready notification with the title to the game script; a refused or missing file leaves an earlier waiting part as it was (§8.8.1). A `/play … sync` stopped by a check reads nothing and leaves both slots as they were | BIN-S; OBS-S (the checks, revision 2) | Confirmed (BIN) (client); Confirmed (OBS) (checks) |
| I44 | A waiting part is started only by the synchronised-start native 0x07DE0002, which the game script calls only from its handler of a server notification; `/playstart` sends a request without arguments and starts nothing locally (§8.8.2) | BIN-S (native registration, sole caller chain); OBS-S (every call site of 0x07DE0002; the `/playstart` body) | Confirmed (BIN + OBS) |
| I45 | The start notification supplies two time values, the **start time** *S* and the **lead** *A*; the game script hands both to the client unchanged (typed as time stamps). A solo start uses *S* = *t* + 0.7 and *A* = 0.7 (§8.7, §8.8.3) | BIN-S (both entries and the shared start function); OBS-S (the handler's instructions) | Confirmed (BIN + OBS); what the server puts in them and any clock translation not examined |
| I46 | The synchronised-start entry shows notice 0x012DD4A4 first, whether or not a part waits; then, if a part waits, starts it with (*S*, *A*) and empties the waiting slot (§8.8.3) | BIN-S | Confirmed (BIN) |
| I47 | Starting any performance (solo or synchronised) first stops the local player's running performance through the stop path with time 0, then checks the new event list's consistency, then resets the stepper with position 0 (§8.7) | BIN-S | Confirmed (BIN); what the stop path does after its game-script post: Probable (the cancel of §8.5) |
| I48 | Stepper: per update tick, finish when the end was taken; wait until the wake time; catch-up below 0.5 s ahead; budget *A* plus a correction; hand on events at *S* + position; next wake time (§8.7) | BIN-S | Confirmed (BIN) |
| I49 | The stepper's events carry the absolute time *S* + position on the client clock and leave the module in one batch per tick through the dispatch of §5.1 (§8.7 step 7) | BIN-S | Confirmed (BIN) (times); path after the dispatch Probable (Q1) |
| I50 | Notes of other characters enter the queue as timed note events through natives 0x07DE0004 (note-on with time and sequence), 0x07DE0005 (note-off), 0x07DE000A (a batch for one character); 0x07DE0006 is the cancel of §8.5 for one character. A listener does not read another member's file (§8.8.5) | BIN-S (the module functions behind the natives); OBS-S (their callers are game-script handlers of server notifications) | Confirmed (BIN) (what the natives do); Probable (that they carry other performers' notes, decision 0004) |
| I51 | The waiting part is withdrawn only by a later successful `/play … sync` (replaced), by the synchronised start (consumed), by the module's handler of two notifications it subscribes to on the local player's object (numbers 0x1000018F and 0x10000143, meaning not traced), which also finishes the running performance at its horizon, and at teardown. The music-end key and a solo `/play` leave it waiting (§8.8.7) | BIN-S (every writer of the waiting slot) | Confirmed (BIN); triggers of the two notifications OPEN (Q11) |
| I52 | Music-end key (action `MusicEndSong`, 0x100000EF): while a performance runs, it is finished at *S* + position (the stepper's horizon) and notice 0x0B87AC37 is shown; with no running performance the key does nothing (§8.7, §8.8.7) | BIN-S | Confirmed (BIN) |
| I53 | Ready notices are shown by the game script, not by the client module: own part 0x0871DF57 (title, argument ID 0x00058427) or 0x02D8DC54 (empty title; no argument set); another member's part 0x03A91887 (name 0x05506DA2, title 0x00058427) or 0x0B630494 (name only) (§8.8.6) | OBS-S (selection by empty title, argument IDs; all four entries in table 0x2500808A) | Confirmed (OBS) (selection, arguments); triggers Probable |
| I54 | Before sending, `/playstart` checks that an object of the local player is present (the same check guards the own ready notice); if it is absent the script shows the notice of key 0x100003B0 and sends nothing (§8.8.2). A `/play … sync` runs the **same** check, after the music-mode check of I58 and before native 0x07DE0001; if it fails, the script shows the same notice of key 0x100003B0 and does not call the native: no file is read, no part waits, no ready notice (§8.8.1; revision 2) | OBS-S | Confirmed (OBS) (check, order and key); meaning of the object Probable (group membership; Q12) |
| I55 | The group: members of the starter's group (fellowship) whose clients hold a waiting part; any member may send the start, leader or not (§8.8.2) | OBS-S (help entries 0x02C008E3, 0x0FB49C73, meaning only; the script checks no leadership); WEB; PLG | Reported (who receives the start is server behaviour) |
| I56 | Command layer of `/play`: part omitted → −1 ("none"); the words after the name are the sync word (entry 0x09657667 of the `Commands` table) and/or a number > 0, in either order; anything else shows the usage entry (`abc-dialect.md` §11) | OBS-S (the `/play` body) | Confirmed (OBS) |
| I57 | Start tolerance 0.0001 s (§8.9) | Decision, from I18, I45–I50 | Decision |
| I58 | Every `/play` (solo or `sync`), once its words have passed the usage check of `abc-dialect.md` §11, first passes the **music-mode check**; if it fails, the script shows the notice of key 0x10000335 and stops: native 0x07DE0001 is not called, no file is read, nothing starts or waits. The check passes when the local player's `Channeling_CurrentChannelingDID` (PID 0x10000BC3) is non-zero, the script finds the player's instrument item (an item lookup with argument 0x20040000, not traced further) and that item passes an item check, and the item's `Item_Music_Channeling_State` (PID 0x10000BA1) equals the player's current channelling DID. The music-mode toggle command uses the same check to choose between leaving and entering music mode (§8.8.1; revision 2) | OBS-S (the `/play` helper, the check body, the toggle command body; PIDs from dictionary 0x34000000) | Confirmed (OBS) (order, key, compared properties); meaning "in music mode" Probable (from the toggle); which notice key 0x10000335 resolves to not traced (Q12) |

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

<a id="s8-7"></a>
### 8.7 Performance stepper (Confirmed (BIN), I45, I47–I49, I52; revision 2026-10-11)

A started ABC performance of the local player (solo or synchronised) is not put into the
queue in one piece. The client keeps **one** started performance and a **stepper** that hands
its events on a little at a time, ahead of their due times. This section answers the OPEN of
`abc-dialect.md` §8.5: its 0.5 s constant is the stepper's catch-up threshold (step 4).

**State.** The event list of `abc-dialect.md` §9.4 (each event with its delta) and the index
of the next event; the **start time** *S* and the **lead** *A* (seconds on the client clock);
the **position** *P*, the sum of the deltas of the events taken so far; the **wake time** *W*
("not yet" until the first tick); and the set of **held** note keys (note-ons taken whose
note-off has not been taken).

| Start | *S* | *A* |
|---|---|---|
| Solo `/play` (`abc-dialect.md` §8.5) | *t* + 0.7, *t* = the client clock of the frame in which the command is processed | 0.7 |
| Synchronised start (§8.8.3) | from the start notification | from the start notification |

**Starting a performance** (both kinds):

1. If the local player has a running performance, it is stopped without finishing: the
   stepper detaches, and the client posts its stop notification with time 0 (in the
   original this leads to the cancel of §8.5, Probable: the path leaves the module, I47).
   A synced `/play` alone does not do this; only a start does.
2. The new event list is checked: every note-on names a key not held, every note-off a key
   that is held, and the end comes with nothing held. Lists built by `abc-dialect.md` §9
   always pass. A list that fails is not started (the old performance stays stopped).
3. Index 0, *P* = 0, *W* = not yet, held set empty; *S* and *A* stored; the stepper is
   attached to the client's update tick (as the queue is, I28).

**On each update tick**, with *now* = the client clock as read for that frame (the queue of
§8.3 reads the clock afresh instead; both are the same clock):

1. **Finish.** If the end event has been taken, hand on a note-off at *S* + *P* for every
   held key, detach and stop. (This happens on the tick after the end event is taken; events
   already handed on still play.)
2. **Wait.** If *W* is set and later than *now*, do nothing this tick.
3. *ahead* = *S* + *P* − *now*; *elapsed* = *now* − *W*, or 0 before the first wake.
4. **Catch-up.** If *ahead* < **0.5 s**: let *k* = 0.5 − *ahead*. Hand on a note-off at
   *S* + *P* + *k* (that is, *now* + 0.5) for every held key. Then, while *k* > 0 and the end
   has not been taken: take the next event, subtract its delta from *k*, add it to *P* and
   update the held set **without handing the event on**; then take every following event
   whose delta is 0 the same way. Set *extra* = 0. If the end has been taken, stop for this
   tick. Effect: the part of the performance that lies before *now* + 0.5 — and the first
   event at or after it, with its zero-delta followers — never sounds; the rest keeps its
   times. *S* never changes.
5. **Correction.** Otherwise *extra* = *A* − *ahead* if *ahead* < *A*, else *extra* =
   *elapsed*.
6. **Hand on.** Remember *P₀* = *P*. With *budget* = *A* + *extra*, while *budget* > 0 and the
   end has not been taken: take the next event, subtract its delta from *budget*, add it to
   *P*, and hand it on with time **S + P** (a note-on or note-off with the event's packed
   note, held set updated; the end event hands on nothing); then take and hand on every
   following event whose delta is 0 the same way.
7. The events handed on in this tick leave the module as one batch through the dispatch of
   §5.1 (Q1).
8. *W* = *now* + (*P* − *P₀*) − *extra*.

**Consequences.**

- Every event handed on has a time at or after *now* + 0.5 of the tick that hands it on
  (step 4 guarantees it, step 5 needs it), and normally about *A* or more ahead. A late tick
  enlarges the next budget by *elapsed*, so the horizon is kept.
- An event's time is always *S* plus its time-line offset (`abc-dialect.md` §8.4, summed as
  deltas). The catch-up test of steps 3–4 compares *now* with *S* + *P*, the position reached
  so far, **not** with the time of the next event: before any event is taken *P* = 0, so a
  performance whose stepper first runs less than 0.5 s before *S* itself **loses its head**
  (step 4) instead of starting late, and its time-0 stays *S*. The head is every event whose
  time lies before *now* + 0.5 **and** the first event at or after *now* + 0.5 with its
  zero-delta followers; since the first event is always taken once the test fires, a part
  that begins with a rest loses its first note even when that note lies 0.5 s or more after
  *now* (§14.10, revision 2).
- The music-end key (`MusicEndSong`) finishes a running performance as in step 1 at once:
  note-offs at *S* + *P*, the last handed-on position, and notice 0x0B87AC37. Notes already
  handed on still sound, so the music stops at the stepper's horizon, up to about
  *ahead* + *A* after the key (I52).
- The two 0.5 s rules are separate: the queue's late drop (§8.3 step 2) tests each note-on
  against the live clock when it is processed; the stepper's catch-up tests the performance
  position against the frame clock before anything is handed on.

<a id="s8-8"></a>
### 8.8 Synchronised group start (revision 2026-10-11)

How the original makes several players start their own parts together (`/play <song>
[part] sync` on each member, then `/playstart` by any member). Client behaviour is
Confirmed (BIN) or (OBS) as marked; everything between clients is server behaviour, which
this spec does not describe (decision 0004) beyond what the client observably receives.

<a id="s8-8-1"></a>
#### 8.8.1 The waiting part (Confirmed (BIN), I43; checks Confirmed (OBS), I54, I58)

**Checks before the client is called** (game script, revision 2). Once the command's words
have passed the usage check of `abc-dialect.md` §11, the script runs, in this order:

1. **Music-mode check** (every `/play`, solo or `sync`; I58): the local player must be in
   music mode, that is, its current channelling DID (`Channeling_CurrentChannelingDID`) is
   non-zero and equals the `Item_Music_Channeling_State` of the instrument item the script
   finds for it. If not, the notice of key 0x10000335 is shown and the command ends.
2. **Group check** (only with `sync`; I54): the same check as `/playstart`'s (§8.8.2). If it
   fails, the notice of key 0x100003B0 is shown and the command ends.
3. Native 0x07DE0001 is called with the name, the part and the sync flag.

A command that ends at check 1 or 2 calls no client function: no file is read, no
`abc-dialect.md` §10 message is shown, nothing starts, no part waits, no ready notice is
shown, and the waiting part and the running performance stay as they were (§8.8.7). Both
keys are shown through the script's general notice path, not as entries of table
0x2500808A; what they resolve to was not traced (Q12).

When both checks pass, `/play … sync` reads and checks the part exactly as a solo `/play`
does (`abc-dialect.md` §3–§10). If it is refused or the file is missing, the message of
`abc-dialect.md` §10 is shown and an earlier waiting part stays as it was. If it is accepted:

- the performance becomes the local player's **waiting part**; there is one slot, so an
  earlier waiting part is discarded (no message);
- nothing starts and the running performance, if any, keeps playing;
- no begin notice is shown (`abc-dialect.md` §10.4 rows 1–2 are for solo starts only);
- the client posts a ready notification with the part's title to the game script, which
  shows the own ready notice and tells the server (§8.8.6; the rest of that path is not
  examined).

A waiting part does not expire; it waits until one of the events of §8.8.7.

<a id="s8-8-2"></a>
#### 8.8.2 The group and the start request (I44, I54, I55)

- **Who forms the group** (Reported): the members of the starter's group — a fellowship —
  whose clients hold a waiting part. The client keeps no list of members and no state about
  other members' parts; the group exists only on the server.
- **Who may start** (Confirmed (OBS); Reported for the server side): any member. The
  `/playstart` command takes no arguments (with arguments it shows its usage entry), checks
  only that an object of the local player is present (Probable: membership of a group; Q12)
  and then sends one request without arguments. `/playstart` runs no music-mode check (I58).
  Leadership is not checked, and the starter
  need not have a waiting part itself.
- If the check fails, the script shows the notice of key 0x100003B0 (not an entry of table
  0x2500808A; resolved by the script's general notice path, not traced) and sends nothing.
- The **same check** guards `/play … sync` before the part is read (§8.8.1 check 2, I54) and
  the own ready notice (§8.8.6). So in the original a player for whom it fails can neither
  make a part wait nor send a start: both commands show the notice of key 0x100003B0.
- `/playstart` starts nothing locally. The starter's own waiting part starts, like every
  other member's, only when the start notification arrives (§8.8.3).

> **OPEN:** (Q12) what the checked object is (fellowship, raid or any group) and which
> notices keys 0x100003B0 and 0x10000335 resolve to. That `/play x sync` runs the same check
> with the same key is answered (revision 2, M1 on record 0x56000000, I54). Suggested method:
> owner test, `/playstart` outside any group and `/play x` outside music mode; M1 on the
> script's general notice path.

<a id="s8-8-3"></a>
#### 8.8.3 The start and its time (Confirmed (BIN + OBS), I45, I46)

Each client that receives the server's start notification hands two values to the client's
synchronised-start entry (native 0x07DE0002): the **start time** *S* and the **lead** *A*,
both as time stamps on the clock of §8.3 (the game script copies them unchanged). The entry:

1. shows notice **0x012DD4A4** (always, also when nothing waits);
2. if a part waits: starts it as in §8.7 ("Starting a performance") with this *S* and *A* —
   which stops the local player's running performance first — and empties the waiting slot.

So the common start time is chosen outside the client: every member's client is given the
same *S* by the server, and the client computes nothing from its own clock. The lead *A*
plays the role that 0.7 s plays for a solo start (the stepper's horizon); the solo start
is the special case *S* = *t* + 0.7, *A* = 0.7 (§8.7). This answers `abc-dialect.md` §11
OPEN (3) as far as the client goes.

> **OPEN:** (Q10) the values the server gives: how far *S* lies after the `/playstart`
> request, and *A*. Whether *S* is translated from a server clock to each client's clock is
> inside the not-examined reading native (decision 0004). Suggested method: owner
> observation with a stopwatch (time from pressing Enter on `/playstart` to the first note;
> compare with a solo `/play`, 0.7 s).

<a id="s8-8-4"></a>
#### 8.8.4 Scheduling each member's part (Confirmed (BIN), I47–I49)

Every member's client schedules **its own** waiting part with the common *S*: the stepper
(§8.7) hands each event on with time *S* + offset, where the offset is the event's place on
the time line of `abc-dialect.md` §8.4. With clocks that agree, every member's part therefore
has the same time-0. A client that processes the start late does not shift its part: if its
stepper's first tick comes less than 0.5 s before *S* + *P* — with *P* = 0 before any event
is taken, that is less than 0.5 s before *S* itself, whatever the offset of the part's first
note — the head is skipped (§8.7 steps 3–4; revision 2: was "before the part's next event"),
and the remaining notes keep their times. A part that begins with a rest therefore loses its
first note in that case (§14.10).

<a id="s8-8-5"></a>
#### 8.8.5 What a listener plays for other members (I50)

A listener never reads another member's ABC file. Notes of other characters reach its queue
as **timed note events** through natives that the game script calls when server
notifications arrive (0x07DE0004 note-on, 0x07DE0005 note-off, 0x07DE000A a batch for one
character, each with its time and sequence number; 0x07DE0006 cancels one character's
music, §8.5). Each event is then processed by the queue rules of §8.2–§8.4: played when
its time comes; a note-on that is 0.5 s or more late when processed is dropped; one that
arrives less late plays at once (late). Since every performer hands events on at least
0.5 s before their time (§8.7), events that cross the network in less than that arrive
before they are due and sound at their own times, that is at *S* + offset on the
listener's clock. That the events are other performers' notes and that their times are
already on the listener's clock is Probable (the notification path is not examined).

<a id="s8-8-6"></a>
#### 8.8.6 Notices (I46, I52, I53)

All entries are in the `UI_Music` table (key 0x10000069 → 0x2500808A, `abc-dialect.md`
§10.3) unless stated. Argument IDs: title in ready notices **0x00058427** (not the 0x0058E905
of the begin notice), member name **0x05506DA2**.

| When | Shown by | Token | Arguments |
|---|---|---|---|
| Own synced part accepted, title not empty | game script, on the client's ready post, after the check of §8.8.2 (trigger Probable) | 0x0871DF57 | title |
| Own synced part accepted, title empty | same | 0x02D8DC54 | none (no argument is set; revision 2) |
| Another member's part is ready, title not empty | game script, on a server notification (trigger Probable) | 0x03A91887 | name, title |
| Another member's part is ready, title empty | same | 0x0B630494 | name (title also set) |
| `/play` (solo or `sync`) refused by the music-mode check (§8.8.1 check 1) | game script, before the client is called | key 0x10000335 (other path) | — |
| `/play … sync` refused by the group check (§8.8.1 check 2) | game script, before the client is called | key 0x100003B0 (other path) | — |
| `/playstart` refused by its check | game script | key 0x100003B0 (other path) | — |
| A start notification is processed (every receiving client, with or without a waiting part) | client | 0x012DD4A4 | none |
| Music-end key while a performance runs | client | 0x0B87AC37 | none |

A synchronised start shows neither 0x00A19C55 nor 0x05D7A7A7 (`abc-dialect.md` §10.4).
Client and script post the table entries above with chat output type 8, the type of the
other player-music notices; the two keys marked "other path" go through the script's general
notice path, whose output type was not traced.

<a id="s8-8-7"></a>
#### 8.8.7 Withdrawing a waiting part, ending a running one (Confirmed (BIN), I47, I51, I52)

| Event | Waiting part | Running performance |
|---|---|---|
| `/play … sync` accepted | replaced by the new part | keeps playing |
| `/play … sync` refused, file missing | kept | keeps playing |
| `/play` (solo or `sync`) stopped by a script check (§8.8.1) | kept | keeps playing |
| Solo `/play` accepted | **kept** | stopped (§8.7 start step 1); the new one starts |
| Start notification | started and removed (kept nowhere) | stopped first if a part waits; untouched if none waits |
| Music-end key | **kept** | finished at its horizon (§8.7), notice 0x0B87AC37 |
| Either of the two subscribed notifications (0x1000018F, 0x10000143; Q11) | removed | finished at its horizon (no notice) |
| Teardown of the music module | removed | — |

There is no message when a waiting part is replaced or removed. Whether the server forgets a
member's readiness on any of these is not known (Q13).

> **OPEN:** (Q13) the server side of the group: whether a start reaches members without a
> waiting part, and whether a member's readiness is forgotten when it withdraws, replaces its
> part or leaves the group. Suggested method: owner observation with a second account.

> **OPEN:** (Q11) which situations raise the two notifications that withdraw the waiting
> part (expected: leaving music mode, or the instrument being unequipped). Suggested method:
> owner test with a second member: `/play x sync`, then `/music off` (or unequip), then
> `/playstart` by the other member; listen whether the part starts.

<a id="s8-9"></a>
### 8.9 Start tolerance (Decision, I57; revision 2026-10-11)

> **Decision:** the **start tolerance** is **0.0001 s** (100 µs). On one listener's audio
> clock:
>
> 1. the time-0 of any two performances started by one synchronised start differ by at most
>    0.0001 s; and
> 2. the time-0 of each such performance differs from the intended common start *S* by at
>    most 0.0001 s.
>
> The **time-0** of a performance on a listener is, for each of its events that is played,
> the time at which the listener schedules that event minus the event's offset on the
> time line of `abc-dialect.md` §8.4. A performance meets the tolerance when every played
> event does (time-0 is not taken from the first note alone, because the head of a late
> part may be skipped, §8.7 step 4, and a part may begin with rests).

**Why this value (provenance).** In the original every performance of one start is given
the same *S* (§8.8.3) and every event is placed at exactly *S* + offset (§8.7 step 6,
§8.8.5): by construction the time-0s are equal, not merely close. The only sub-millisecond
quantity the original applies to event times is its ordering tolerance (§8.2, I18): times
closer than 0.0001 s count as the same time. Tessera schedules on the audio clock with
sample accuracy (§12), so its residual error is the rounding of a time to a sample (at most
half a sample per time: 11.34 µs at 44,100 Hz, 10.42 µs at 48,000 Hz; a difference of two
rounded times at most one sample, 22.68 µs at 44,100 Hz). 0.0001 s is the original's own
"same time" and
leaves more than four samples of margin at 44,100 Hz. A larger value would admit an error the
original never makes; a smaller one would be tighter than the original's own notion of
simultaneity.

**Relation to the other two time constants.**

- **0.0001 s ordering tolerance (§8.2).** Same value, different use: §8.2 decides the order
  of events of the queue; §8.9 bounds where a performance's events are placed. Two
  performances within the start tolerance are, for the queue, simultaneous wherever their
  written times are equal, so the order of their events is the one the tie rule gives for
  exact alignment.
- **0.5 s late drop (§8.3) and catch-up (§8.7 step 4).** Neither is a tolerance on time-0.
  Both remove notes and never move a performance: a part that starts late loses its head, an
  event processed 0.5 s or more after its time is dropped, and the remaining events keep
  *S* + offset. The tolerance therefore applies to every event that is scheduled; an event that
  is processed after its time and played late (less than 0.5 s, §8.8.5) is outside the
  synchronised behaviour and fails the tolerance. In Tessera's sandbox every member's part is
  scheduled on the listener's own clock, so this must not happen, and it is a **requirement**
  on the player (revision 2, answering the P4 implementation OPEN (e) of §12): every event of
  a started part is handed to the audio scheduler before its time. A windowed client meets it
  by updating the player every frame with the look-ahead of §12 answer (e) (0.1 s); a single
  frame longer than the look-ahead may hand an event on late, which then plays late as in the
  original's network case (a stall, not a tolerance failure of the start). A caller that
  renders longer blocks, such as a headless run on a virtual clock (the check of P4 exit
  criterion 3 below), hands events on as far ahead as its block, at most 0.5 s.

**For P4 exit criterion 3** (a synchronised `/play` from a scenario starts within this
tolerance): run the scenario on its virtual clock; for every performance started by the
synchronised start and every note-on and note-off it schedules, |scheduled time − (*S* +
offset)| ≤ 0.0001 s, with *S* the start time of §12 (P4 Decisions). Examples in §14.9–§14.10.

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

**Answers to the implementation OPEN above (revision 2026-10-11, BIN-S):**

- **(b)** Original: starting a performance — a solo `/play` accepted, or a synchronised start
  that finds a waiting part — stops the local player's running performance first (§8.7, I47).
  A `/play … sync` does **not**: the running performance keeps playing until the start
  (§8.8.7). Tessera's choice stands for solo `/play`; for `/play … sync` it must not cancel
  the running performance.
- **(c)** Original: the end of a performance hands on note-offs for the keys still held at
  *S* + *P*, the end's own time (§8.7 step 1); with lists from `abc-dialect.md` §9 none are
  held. Tessera's choice (note-offs at the end time) agrees.
- **(d)** Answered by §8.8.3: the start time and the lead come from the start notification;
  the client adds nothing. Superseded for Tessera by the P4 Decisions below: **one** *S*
  shared by every waiting part, taken from the scenario feed's start when the session
  delivers one, else *t* + 0.7 with *A* = 0.7 (P4 Decision 3, revision 2).
- **(e)** Compatible with §8.7: the original hands events on at least 0.5 s early (normally
  about *A* or more), so deciding at most 0.1 s early is within what the original allows.

> **Decision (P4, 2026-10-11): synchronised start in Tessera.** Follow §8.7–§8.9:
>
> 1. **Waiting part.** One waiting part per character (the local player, and each fellowship
>    member the scenario feed makes ready). An accepted `/play … sync` replaces the local
>    player's waiting part and does not touch its running performance; the own ready notice
>    of §8.8.6 is shown (0x0871DF57 or 0x02D8DC54). A refused one leaves the earlier part.
> 2. **The group.** When the scenario feed reports a fellowship (`FellowshipChanged` with a
>    non-empty member list, `scenario-feed.md`), the group is the local player plus the listed
>    members, and its waiting parts are those of its members. Without a fellowship the group
>    is the local player alone, as before (`abc-dialect.md` §11 (f)). This is a **deliberate
>    difference** from the original (revision 2): there, `/play … sync` and `/playstart` both
>    run the group check of §8.8.1–§8.8.2 (I54), and a player for whom it fails (Probable: one
>    in no group, Q12) gets the notice of key 0x100003B0 from either command, with no file
>    read, no part waiting, no ready notice and no request sent. Tessera keeps the group of one
>    so that a player without a fellowship — the usual case in the sandbox, and the only case
>    with Tessera's own server, which has no fellowships (decision 0035) — can still rehearse a
>    synchronised part and its start; Tessera never shows the notice of key 0x100003B0.
>    Following the original instead (refusing `/play … sync` and `/playstart` without a
>    fellowship) would change the client's command handling and needs a new revision.
> 3. **Start.** One start produces one start time *S* and one lead *A* (revision 2,
>    reconciled with `scenario-feed.md` §6.7, which is normative for the feed's events):
>    - **A start the session delivers** — the feed's `PerformanceSyncStart`, whether the feed
>      attributes it to a member or sends it as the answer to the local `/playstart` (a
>      backend that accepts `StartPerformance`: the feed and the plain sandbox) — takes its
>      time from the event: *S* = the audio-clock time at which the client processes the event
>      plus `lead_ms`; in a headless run whose audio clock runs in step with the feed's virtual
>      clock, *S* = the audio-clock time of virtual time `at_ms`. *A* = `lead_ms` (*A* bounds
>      nothing in Tessera's scheduling, Decision 4). With such a backend the local `/playstart`
>      starts nothing itself, so one `/playstart` gives exactly one start.
>    - **A local start** — `/playstart` with a backend that refuses `StartPerformance` (the
>      test-server backend) — is Tessera's own: *S* = *t* + 0.7 and *A* = 0.7, *t* = the
>      audio-clock time at which Tessera processes the command.
>
>    The feed's default lead (700 ms) gives the same values as a local start. The original's
>    server values are unknown (Q10). The notice 0x012DD4A4 is shown once per start, also when
>    nothing waits. Every waiting part of the group — the local one and every fed member's — is
>    started with this same *S*; the local player's running performance is stopped first only
>    if its own part waits. The waiting slots are emptied. Leadership plays no part.
> 4. **Scheduling.** Every event of every started part is scheduled at *S* + offset on the
>    listener's audio clock, within the start tolerance of §8.9. The head of a part is skipped
>    by the rule of §8.7 steps 3–4 with *P* = 0 and *now* = the audio-clock time of the
>    scheduling: when *S* − *now* < 0.5 s, every event before *now* + 0.5 and the first event
>    at or after it, with its zero-delta followers, are not scheduled. The test compares with
>    *S* itself, not with the time of the part's first note, so a part that begins with a rest
>    loses its first note even when that note lies 0.5 s or more after *now* (revision 2: was
>    "less than 0.5 s before *S* + (offset of its next event)"; §14.10). Nothing is shifted.
>    The batching and horizon of the stepper need not be copied (§12 (e) stands).
> 5. **Fed members.** A member's part is the tune named by the feed, read from the user's own
>    music folder like a `PerformanceStarted` tune; when it is read and accepted, Tessera shows
>    0x03A91887 (or 0x0B630494 for an empty title) with the member's name. A part that cannot
>    be read or is refused shows Tessera's own log line and does not join. A
>    `PerformanceStarted` outside a synchronised start keeps its meaning (*S* = *t* + its
>    delay).
> 6. **Withdrawal** as in §8.8.7, plus: a fed member's waiting part is removed when the feed's
>    member list no longer contains that member, or when the feed stops that member's
>    performance (`PerformanceStopped`) (Tessera's choice: the server side is not known, Q13).
>    `/playstop` stays Tessera's own command (`abc-dialect.md` §11 (a)): it cancels the running
>    performance and withdraws the local waiting part; a `MusicEndSong` key action, when bound,
>    follows the original (finishes the running performance only, notice 0x0B87AC37, the part
>    keeps waiting).
> 7. **Tessera's own server** relays chat only (decision 0024, `server-content.md` §2); whether a
>    synchronised start may cross it is the owner's open question in decision 0024. Until it is
>    answered, synchronised starts with other members exist only through the scenario feed.
> 8. **Music-mode check** (revision 2). Tessera has no music mode yet: no channelling state and
>    no music-mode toggle command, and the `MusicUI` input map is ignored (`keymap.md`). Until
>    a spec adds music mode, check 1 of §8.8.1 counts as passed for every `/play`, the notice of
>    key 0x10000335 is never shown, and the client choice (c) of the OPEN of 2026-10-07 above
>    stands (with neither an instrument item nor a debug choice, `/play` reads and checks the
>    song and nothing plays, with Tessera's own message). This is a deliberate difference: the
>    original refuses such a `/play` before reading anything (I58). Once music mode exists,
>    `/play` must run check 1 first, as in §8.8.1.

> **Answered (spec writer, 2026-10-23; revision 3 bookkeeping):** see "Answers to the P4
> implementation OPENs above" below, items "P4 client, start time" and "Instrument of a fed
> member's part": the placement of *S* is the specified one (P4 Decision 3), and the built
> choice of instrument (the local player's instrument type, else the first type of the
> instrument table) is confirmed as Tessera's own rule until the feed names an instrument;
> the limits of 64 members and 16 remembered starts are Tessera's own. No implemented
> behaviour changes. The implementer's text is kept as the record.
>
> (Was **OPEN (implementation, P4 client, 2026-10-11):**) Decision 5 does not say which
> instrument a fed member's part plays on (the feed's `PerformanceReady` carries none). The
> client uses the local player's instrument type, else the first type of the instrument
> table. It allots each member a character of its own in the player (by name, at most 64),
> keeps the last 16 synchronised starts for the tolerance check, and places *S* on the audio
> clock at the processing time plus `lead_ms` in a windowed run, and at `at_ms` on an audio
> clock run in step with the virtual clock in the headless test (`scenario-feed.md` §6.7).

> **Answered (spec writer, 2026-10-23; revision 3 bookkeeping):** every reading (a)–(f) is
> confirmed in "Answers to the P4 implementation OPENs above" below: (a) is §8.7 steps 3–4
> (compare with *S* + *P*, *P* = 0 before any event; IN22, §14.10), (b) and (c) agree with
> the original, (d) and (f) are Tessera's own choices that stand, (e) is accepted for block
> renderers (at most 0.5 s early; a windowed client keeps 0.1 s, §8.9). The built
> `SyncGroup` behaviour is correct as it stands; no implemented behaviour changes. The
> implementer's text is kept as the record.
>
> (Was **OPEN (implementation, P4 audio, 2026-10-11):**) `tessera_audio::instruments::SyncGroup`
> had to choose where the P4 Decision is silent or reads two ways. Please confirm:
> (a) **Decision 4 wording.** "less than 0.5 s before *S* + (offset of its next event)" and
> §8.8.4 "before the part's next event" could be read as comparing with the first event's
> offset; §8.7 step 3 compares with *S* + *P*, and *P* = 0 before any event is taken. Tessera
> follows §8.7 (the rule Decision 4 names): a part beginning with a rest, scheduled 0.2 s
> before *S*, loses its first note even when that note lies 0.7 s ahead (as **zG**: on G at
> *S* + 0.5 is skipped). (b) A part whose whole event list falls to the catch-up (its end
> taken) queues nothing, but the start still stops that performer's running performance
> (§8.7 start step 1) and empties its waiting slot. (c) The note-offs of note-ons the
> catch-up skipped are queued at their own times, as the original hands them on (§14.10:
> they find no registry entry). (d) A start whose scheduling time is not known to the
> caller skips nothing.
> (e) **Update cadence.** §8.9's "that cannot happen" holds only while the player is updated
> at least once per look-ahead (0.1 s) of rendered audio: an event whose time passes between
> two updates is handed on late and fails the tolerance. A caller that renders longer blocks
> (a headless scenario on a virtual clock) passes the block length
> (`MusicPlayer::update_for`), which hands events on up to that far ahead, at most 0.5 s;
> that takes the registry and instance-limit decisions up to 0.5 s early, beyond the 0.1 s
> of answer (e) above. Please confirm 0.5 s is acceptable. (f) A part that does not fit
> Tessera's own queue limits at the start (the original has none) is refused alone with
> Tessera's own log line; its performer's running performance keeps playing, and its slot is
> emptied like the others (Decision 3), so it cannot block later starts.

**Answers to the P4 implementation OPENs above (revision 2, 2026-10-11):**

- **P4 audio (a)** Tessera's reading is the right one. The catch-up compares *now* with
  *S* + *P*, and *P* = 0 before any event is taken (§8.7 steps 3–4, confirmed by BIN-S and by
  the independent review's own check). §8.7 Consequences, §8.8.4 and Decision 4 now say so;
  the **zG** case is worked in §14.10 and stated as IN22.
- **P4 audio (b)** Agrees with the original: the start stops the performer's running
  performance (§8.7 start step 1) before the stepper runs, and the start empties the waiting
  slot (§8.8.3 step 2); a list wholly taken by the catch-up hands on nothing (§8.7 step 4)
  and is finished on the next tick with nothing held.
- **P4 audio (c)** Agrees with the original: a note-on taken by the catch-up enters the held
  set (§8.7 step 4), so its note-off, when it lies after the catch-up, is handed on at its own
  time (step 6) and finds no registry entry (§8.4; off E in §14.10).
- **P4 audio (d)** Not a fact of the original, whose starts always carry a time. Every start
  of Decision 3 has a processing time; a start without one is outside this spec, and skipping
  nothing for it is Tessera's own choice.
- **P4 audio (e)** Accepted for callers that render blocks longer than the look-ahead (a
  headless run on a virtual clock): they may hand events on up to 0.5 s early, no earlier
  than the original's stepper hands events to its queue (§8.7), although the original's queue
  then acts only at each event's time (§8.3). Registry and instance-limit decisions taken that
  early can therefore differ from the original only when a cancel or a limit decision falls
  inside the block; the tolerance check of §8.9 tests times only. A windowed
  client keeps the 0.1 s of answer (e). §8.9 states the requirement.
- **P4 audio (f)** Tessera's own choice (the original has no such limits); it agrees with
  Decision 3, which empties every slot of the group.
- **P4 client, start time.** Answered by Decision 3 (revision 2): the client's placement of
  *S* (processing time plus `lead_ms` windowed, `at_ms` headless) is the specified one.
  **Instrument of a fed member's part:** in the original another performer's notes carry
  their instrument type in each event (§4.5, §8.8.5); the feed names none, so the client's
  choice (the local player's type, else the first type of the table) is Tessera's own until
  the feed carries one. The limits of 64 members and 16 remembered starts are Tessera's own.
- **Client (c) of 2026-10-07** (what the original does for `/play` without an instrument):
  the original's script refuses every `/play` outside music mode before reading, with the
  notice of key 0x10000335 (§8.8.1 check 1, I58). Tessera keeps its choice (Decision 8).

> **OPEN:** (Q14) owner decision on decision 0024, open item 2: may a synchronised start (the
> ready and start notifications of §8.8) be relayed by Tessera's own server as chat?

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
- **IN17** (synthetic, revision 2026-10-11): whatever the tick times, every event a started
  performance hands on has the time *S* + its offset on the time line (§8.7 step 6).
- **IN18** (synthetic): every event handed on has a time at or after *now* + 0.5 of the tick
  that hands it on (§8.7).
- **IN19** (synthetic): a character has at most one waiting part; an accepted `/play … sync`
  replaces it; a start empties the slot (§8.8.1, §8.8.7).
- **IN20** (synthetic): a start that finds no waiting part changes nothing but showing
  0x012DD4A4; the running performance keeps playing (§8.8.3).
- **IN21** (synthetic): skipping a late part's head (§8.7 step 4) never changes the times of
  the events after it: every played event of every part of one start has time-0 *S*, within
  the start tolerance of §8.9.
- **IN22** (synthetic, revision 2): for a start processed at *now* with *S* − *now* < 0.5 s,
  the first event of every part is never scheduled, whatever its offset (a part that begins
  with a rest included); with *S* − *now* ≥ 0.5 s nothing is skipped (§8.7 steps 3–4, §14.10).

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

The examples below (revision 2026-10-11) use one part, written **CDEF**: the reference header
of `abc-dialect.md` §12 (unit 0.25 s) and the body `C D E F|`. Its events and offsets are:
on C 0, off C and on D 0.25, off D and on E 0.5, off E and on F 0.75, off F 1.0, end 1.0
(deltas 0, 0.25, 0, 0.25, 0, 0.25, 0, 0.25, 0).

<a id="s14-8"></a>
### 14.8 Stepper, solo start

`/play` processed at *t* = 100.0: *S* = 100.7, *A* = 0.7.

| Tick *now* | *ahead* | *extra* | Handed on | *W* after |
|---|---|---|---|---|
| 100.02 | 0.68 (≥ 0.5, < *A*) | 0.02 | on C 100.7; off C, on D 100.95; off D, on E 101.2; off E, on F 101.45 (budget 0.72 runs out after off E; its zero-delta follower on F goes with it) | 100.02 + 0.75 − 0.02 = 100.75 |
| 100.76 | 100.7 + 0.75 − 100.76 = 0.69 | 0.01 | off F 101.7; the end is taken | 101.0 |
| 100.78 | — | — | finish: nothing held, so nothing (step 1 comes before the wait) | — |

Every handed-on time is *S* + offset (IN17) and at least *now* + 0.5 (IN18).

<a id="s14-9"></a>
### 14.9 Synchronised start on time, and the start tolerance

Original: the start notification gives *S* = 10.0 and *A* = 0.7; the member's first stepper
tick is at *now* = 7.0. *ahead* = 3.0 ≥ *A*, *extra* = 0 (first wake), budget 0.7: on C 10.0;
off C, on D 10.25; off D, on E 10.5; off E, on F 10.75; *W* = 7.75. At 7.75: *ahead* 3.0,
*extra* = *elapsed* = 0: off F 11.0 and the end. The part is on time and keeps a 3 s horizon.

Tessera sandbox (§12 P4 Decision 3): the local player L waits with **CDEF**; fellowship member
M waits with the part **zG**: reference header, body `z2 G2|` (a rest, then on G at offset
0.5, off G and end at 1.0). The start is processed at *t* = 50.0, so *S* = 50.7 for both
(a local start, or a fed start with `lead_ms` 700; revision 2).

| Part | Event | Scheduled | Offset | Time-0 |
|---|---|---|---|---|
| L | on C | 50.7 | 0 | 50.7 |
| L | off F | 51.7 | 1.0 | 50.7 |
| M | on G | 51.2 | 0.5 | 50.7 |
| M | off G | 51.7 | 1.0 | 50.7 |

At 44,100 Hz the times are samples 2,235,870 (50.7 s), 2,257,920 (51.2 s) and 2,279,970
(51.7 s), all exact. Differences: L–M 0, each part–*S* 0: within 0.0001 s, **pass**.

Failing variants: (a) M is started with its own processing time instead of the common *S*,
for example 50.016 + 0.7 = 50.716: M's time-0 50.716, 0.016 s from L and from *S* — **fail**
(this is the error that one shared *S* prevents). (b) M's events are placed 0.00015 s late
(51.20015 and 51.70015): time-0 50.70015 — **fail**. (c) M's events are placed 0.00004 s late:
time-0 50.70004 — **pass**; at 44,100 Hz 51.20004 s rounds to sample 2,257,922, two samples
(45.4 µs) after L's grid.

<a id="s14-10"></a>
### 14.10 Synchronised start processed late

Original: *S* = 10.0, *A* = 0.7, **CDEF**, first stepper tick at *now* = 9.8. *ahead* = 0.2 <
0.5, so *k* = 0.3; nothing is held, so no note-offs at 10.3. Catch-up takes, without handing
on: on C (*k* 0.3, *P* 0); off C (*k* 0.05, *P* 0.25) with on D; off D (*k* −0.2, *P* 0.5) with
on E; *k* ≤ 0 stops it. *extra* = 0, budget 0.7: off E 10.75 with on F 10.75; off F 11.0 and
the end; *W* = 9.8 + 0.5 = 10.3. Only F sounds, at 10.75 = *S* + 0.75; E (offset 0.5) is
skipped although 10.5 was 0.7 s ahead, because the catch-up also takes the first event at or
after *now* + 0.5 − *S* = 0.3 (off D at 0.5) and its zero-delta follower. The off E handed on
finds no registry entry and stops nothing (§8.4). Time-0 of every played event: 10.0 (IN21).

A listener receiving the same performer's events late (§8.8.5): a note-on due at 10.25 that is
processed at 10.80 is 0.55 s late and dropped; one due at 10.75 processed at 10.80 plays at
10.80, 0.05 s late — outside the start tolerance; the original accepts this as a network
effect, Tessera's sandbox never produces it.

A part that begins with a rest (revision 2): *S* = 10.0, the part **zG** of §14.9 (on G at
offset 0.5, off G and end at 1.0), first stepper tick at *now* = 9.8. *ahead* = *S* + *P* −
*now* = 0.2 with *P* = 0 (not 0.7, the distance to the first note), so *k* = 0.3. The catch-up
takes on G (*k* −0.2, *P* 0.5) and stops. *extra* = 0, budget 0.7: off G at 11.0 and the end.
**Nothing sounds**, although on G at 10.5 was 0.7 s ahead of *now*; the off G finds no
registry entry. With the first tick at *now* = 9.5 instead, *ahead* = 0.5 is not below 0.5,
and on G is handed on at 10.5.

<a id="s14-11"></a>
### 14.11 Waiting part and running performance

| Step | Action | Waiting part | Running | Notice |
|---|---|---|---|---|
| 1 | `/play a sync` (accepted, title "a") | a | — | 0x0871DF57 (own ready) |
| 2 | `/play b` (solo, accepted) | a | b (*S* = *t* + 0.7) | 0x00A19C55 |
| 3 | `/play c sync` (accepted) | c (a discarded) | b | 0x0871DF57 |
| 4 | `/play d sync` (file missing) | c | b | file not found (`abc-dialect.md` §10.2) |
| 5 | music-end key | c | b finished at its horizon | 0x0B87AC37 |
| 6 | music-end key again | c | — (nothing runs) | none |
| 7 | start notification (*S*, *A*) | — | c | 0x012DD4A4 |
| 8 | second start notification | — | c keeps playing | 0x012DD4A4 |

With a running b at step 7 (skip step 5), b is stopped first, then c starts.

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
| `abc-dialect.md` §8.5 OPEN (revision 2026-10-11) | The 0.5 s constant of the performance stepper | It is the stepper's catch-up threshold (§8.7 step 4), a separate rule from the queue's late drop of §8.3. `abc-dialect.md` records the answer. |
| `abc-dialect.md` §11 OPEN (3) | Source of the synced start time; meaning of the second start value | Both come with the server's start notification; the second value is the stepper's lead *A* (0.7 for a solo start) (§8.8.3). `abc-dialect.md` records the answer. |
| `abc-dialect.md` §11 | "native 0x07DE0002 synced start" read as the `/playstart` command's call | 0x07DE0002 is called only by the game script's handler of the start notification; `/playstart` sends a request and calls no music native (I44). |
| `scenario-feed.md` §16 OPEN (P4) | "The synchronised start tolerance of `PerformanceStarted` is taken from `instruments.md`" | The tolerance is §8.9: 0.0001 s on the listener's audio clock, for every scheduled event of every part of one start. A synchronised start needs the feed to make a member's part wait and to attribute a start to a member (§12 P4 Decision, items 2, 3, 5, 6); `PerformanceStarted` with a delay stays a solo-like start. For the `scenario-feed.md` owner to adopt. |
| `scenario-feed.md` §16 OPEN (non-blocking, "P4 social"), reading (1) | With a backend that accepts `StartPerformance` the client starts only on the backend's `PerformanceSyncStart`, using its `lead_ms` as the lead | Agreed (revision 2): §12 P4 Decision 3 now takes *S* and *A* of every start the session delivers from the event, as §6.7 states, and keeps *S* = *t* + 0.7 only for a local start with a backend that refuses `StartPerformance`. Reading (2) is for `social-panels.md`. |

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
| Q10 | The start time and lead the server gives at a synchronised start (how far after the request; *A*); any clock translation | §8.8.3 | Owner observation with a stopwatch; the translation stays behind decision 0004 |
| Q11 | Which situations raise the two notifications that withdraw a waiting part | §8.8.7 | Owner test with a second member (`/music off` or unequip between `/play x sync` and `/playstart`) |
| Q12 | What the object checked by `/playstart`, `/play … sync` and the own ready notice is: fellowship, raid, or other group; which notices keys 0x100003B0 and 0x10000335 resolve to | §8.8.1, §8.8.2 | Partly answered (revision 2, M1): `/play … sync` runs the same check with the same key (I54); every `/play` first runs the music-mode check, key 0x10000335 (I58). Rest: owner test (`/playstart` outside any group, `/play x` outside music mode); M1 on the script's general notice path |
| Q13 | Server side of the group: whether the start reaches members without a waiting part, whether readiness is forgotten when a member withdraws, leaves the group or replaces its part | §8.8.2, §8.8.7 | Owner observation with a second account (watch for 0x012DD4A4 on a member that did not `/play … sync`) |
| Q14 | Whether a synchronised start may cross Tessera's own server (chat-only relay, `server-content.md` §2) | §12 P4 Decision 7 | Owner decision (decision 0024, open item 2) |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-23 | Revision 3 (spec writer, dirty room; bookkeeping, no new probe): §12, the callouts "OPEN (implementation, P4 client, 2026-10-11)" (instrument of a fed member's part, start time, limits) and "OPEN (implementation, P4 audio, 2026-10-11)" (`SyncGroup` readings (a)–(f)) are marked **Answered**, pointing to the "Answers to the P4 implementation OPENs above" of revision 2; the built behaviour (fed parts on the local player's instrument type, else the table's first; the catch-up against *S* + *P*; the end and note-off readings; block hand-on up to 0.5 s early) is confirmed. No fact, rule, section or anchor changed; no implemented behaviour changes. Status Accepted → Revised, awaiting independent review. |
| 2026-10-23 | Independent review (spec-review:spec-gameplay:2 (independent session)) of the 2026-10-23 revision: **passed**, CLEAN_ROOM.md §5.3 every item; status Revised → Accepted. Header status and review record only; no other text changed; no implemented behaviour changes beyond those the revision row states. |
| 2026-10-11 | Independent re-review of P4 revision 2 (spec reviewer, Clean-Room-Role: review): **passed**, every item of CLEAN_ROOM.md §5.3; status Revised → Accepted. B1–B3 resolved, own M1 checks of record 0x56000000, dictionary 0x34000000 and table 0x2500808A and an M4 check of the stepper and catch-up (decision 0011) agree; non-blocking notes (a), (b) in the review record. Header only, no content change. |
| 2026-10-11 | Revision 2 of the P4 revision, answering the independent review (status stays Revised, awaiting re-review). **B1:** new I58 and §8.8.1 "Checks before the client is called" (M1 on record 0x56000000, OBS-S): every `/play` first passes the music-mode check (key 0x10000335), and `/play … sync` then the same group check as `/playstart` (key 0x100003B0); a stopped command reads nothing and changes no slot. I43, I54, §8.8.2, §8.8.6 (two rows), §8.8.7 (one row), Q12 and its callout revised; P4 Decision 2 restates its reason (a deliberate difference: Tessera keeps the group of one); new P4 Decision 8 (no music mode in Tessera yet: the check counts as passed). **B2:** §8.7 Consequences bullet 2, §8.8.4 and P4 Decision 4 measure the catch-up against *S* + *P* (*P* = 0 before any event), as §8.7 steps 3–4 do; new IN22 and a §14.10 example (a part beginning with a rest). **B3:** P4 Decision 3 takes *S* and *A* of a start the session delivers from `scenario-feed.md` §6.7 (`lead_ms`, `at_ms` headless) and keeps *t* + 0.7 for a local start with a backend that refuses `StartPerformance`; §12 answer (d) and a §15 row follow. Non-blocking: OBS-S counts (45 entries; 18 call sites in 14 bodies, N1), I53 and §8.8.6 own empty-title row (no argument set, N2), §8.9 sample figures (N3), §8.9 scheduling requirement and answers to the P4 implementation OPENs (a)–(f), the P4 client OPEN and client (c) of 2026-10-07 (N5). N4 not taken up (no effect on Tessera). No section number or anchor changed. |
| 2026-10-11 | Independent review of the P4 revision (spec reviewer, Clean-Room-Role: review): changes requested, status stays Revised, not Accepted. Blocking B1 (script checks before `/play`, keys 0x10000335 and 0x100003B0), B2 (catch-up wording in `instruments.md` §8.7, §8.8.4, §12 Decision 4), B3 (`instruments.md` §12 Decision 3 against `scenario-feed.md` §6.7); non-blocking N1–N5. Header review record only, no content change. |
| 2026-10-11 | Revision for P4, additive (status Revised). Third M4 pass (BIN-S) and the game script of the class library (OBS-S). New §8.7 (performance stepper: start, catch-up below 0.5 s, budget, wake time, finish; answers `abc-dialect.md` §8.5 OPEN), §8.8 (synchronised group start: waiting part, group and start request, the start time and lead from the start notification, scheduling, what listeners play, notices, withdrawal), §8.9 (**start tolerance** 0.0001 s, Decision, with its relation to the 0.0001 s ordering tolerance and the 0.5 s rules). §12: answers to the implementation OPEN (b), (c), (d), (e) and the P4 Decision for fellowships. §14.8–§14.11 examples, IN17–IN21, I43–I57, §15 rows, Q10–Q14. No earlier section or anchor changed. |
| 2026-10-06 | Revision for the third-round re-review of 2026-10-06 (status stays Review). §4.4 and I26 corrected: the older copy 0x070017F9 does **not** name other sound infos for `Bagpipe`. Every one of its 2,120 Sound nodes names the same sound info as the matching case of 0x0700042F (335 distinct, all in the 746 instrument set), with the same channel, `FireAndForget`, `PitchVariance` and other fields, except `RampOutTime` 0 on the 222 `Bagpipe` nodes (0.2 in 0x0700042F). Added the path-intensity differences for `Flute`, `Lute`, `Theorbo`, `Bagpipe` and `Pibgorn` (1,110 Sound nodes), found by the same full comparison. Players are not affected (no avatar closure reaches 0x070017F9). |
| 2026-10-06 | Revision for the re-review of 2026-10-05 (status stays Review). I31 and §6.3: script 0x1000274C of table 0x07000078 has two fire-and-forget Sound nodes on channel 1, naming 0x2A00A826 **and** 0x2A00A824 (was: one effect, 0x2A00A826); re-checked against every Sound node of all decodable tables and scriptlets. §4.2: the lead-in of the value table no longer calls 0x2A00A824 unused; it is unreferenced only by the two music scripts. §6.2: the 0x2A00A824 row names its one user. |
| 2026-10-05 | Revision for the spec review of the same day (status stays Review). Second M4 pass (BIN-R). §5.1, §5.2, §8.4: one packed-note rule (key read from bits 0–7 everywhere, written as 24 bits; velocity above 127 dropped by the packers). §5.1: live-play handler facts (velocity 90, note-on at press, note-off at release; I41); UI option described instead of quoted. §9.1: rewritten; non-fire-and-forget sounds are subject to the sound-manager gates. §9.3: gate table for instrument notes. §10.1: the Sound node's own multiplier enters its volume (agrees with `sound.md`); clamp only when the variance is not 0 (disagrees with `sound.md` §9.3, Q8; census I42). §10.4: gain chain and channel-5 pool (Q9). §10.5: distance law only for flags 0x04/0x40, panning, reverb, camera listener. §4.2: flag 0x04 and fields `a`, `b` from `sound.md`; Q3 narrowed. §4.5: `PhysObj` copy and the no-`PhysObj` case; slot bits. §6.3: other users of instrument sound infos corrected (adds table 0x07000078). Callouts for Q1–Q9 at each unknown. Provenance rows I30–I42, tags BIN-R and SND; I14, I15, I22, I23, I29 revised. Invariants IN13–IN16; examples in §14.2 and §14.4; `sound.md` rows in §15; §12 live velocity 90 and the gate decision. |
| 2026-10-05 | First version: identifiers, the music scripts of table 0x0700042F and their tree, sound-info layout, sample census, per-instrument table, pitch mapping and measured octaves, playback queue, registry, cancel and instance limit, sustain and release behaviour, loudness and distance, note ranges, Tessera decisions, invariants, synthetic examples, corrections and open questions. BIN analysis of client build 4906.0071.1794.4035. |
