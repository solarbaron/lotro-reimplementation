<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Sound: sound records, audio payloads, sound nodes, music and ambience

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review 2026-10-05: **accepted**. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/formats` (sound-info record 0x2A, sound payload record 0x0A, Ogg and WAV container checks; the existing `sound` module), `crates/audio` (`tessera-audio`: decoding, voices, buses, distance law, pitch, fades, voice limit, reverb environments, the player-property feed of §10.3), the script evaluator that runs script tables (Sound and SoundEnvironment nodes, §9), `apps/client` (audio settings named after the channel enum, §7), `apps/dattool` / `apps/explorer` (sound browser), `fuzz/` (one target per record type, §13) |
| **Companion specs** | [`records.md`](records.md) §9 (0x0A envelope and container sniffing; this spec extends and corrects it, §15). [`property-system.md`](property-system.md) §2–§5 (dictionary, value encodings, property sets), §8 (DID maps), §9 (DB-object-type codes: 0x12 sound info, 0x2A script table). [`script-table.md`](script-table.md) §2–§7 (framing, node layouts incl. kinds 48 and 62, control-flow nodes; §9 here adds the sound-node semantics that file does not yet hold). [`landblock.md`](landblock.md) §4 (cell properties), §5 (block maps and descriptors). [`environment.md`](environment.md) §3 (region descriptor), §6 and §11 (game clock). [`entity-descriptor.md`](entity-descriptor.md) (SED placement of 0x47 objects). [`setup.md`](setup.md) §3.1 (0x47 header). [`wstate.md`](wstate.md) (area, dungeon and landmark objects). [`install-layout.md`](install-layout.md) (the `[Sound]` section of the user preferences). Player-music instruments and the ABC dialect are out of scope (`instruments.md`, `abc-dialect.md`). |

---

<a id="s0"></a>
## 0. Purpose and terminology

This spec lets an implementer:

- decode every **sound payload** record (prefix 0x0A) and play its audio: Ogg Vorbis or PCM
  RIFF WAVE, plus eight non-audio DirectMusic records (§3);
- decode every **sound-info** record (prefix 0x2A), which names a payload and gives its
  playback parameters: flags, priority, minimum and maximum distance (§4);
- play a sound the way the original does: gain chain, channels and volume settings, the
  linear distance law, pitch mapping, random placement, looping, stopping with a ramp-out,
  the voice limit, the listener and reverb environments (§6–§8);
- run the two script-table node kinds that make sound, **Sound** (48) and
  **SoundEnvironment** (62), with their exact field semantics (§9; this section holds the
  facts that belong to `script-table.md`, which is locked to this writer);
- reproduce **area music and ambience**: these are not a hard-coded "music director" but a
  data-driven script graph that every player avatar runs, switching on player properties
  such as `Area_Music`; the spec lists those properties and where their values come from
  (block maps, area, dungeon and landmark objects, monitor volumes, the game clock), and
  how music changes and cross-fades (§10);
- place **positional emitters**: world objects whose start-up scripts play looping or
  repeating sounds (§11).

Terms:

| Term | Meaning |
|---|---|
| **payload** | A 0x0A record: the encoded audio (or DirectMusic data) plus a two-word trailer (§3). DB-object type 0x40. |
| **sound info** | A 0x2A record: flags, five floats, the payload DID and an asset name (§4). DB-object type 0x12. Script nodes and properties reference sound infos, never payloads directly. |
| **request** | One attempt to play a sound info, made by a Sound node (§9.2) or by client code for UI and weather sounds. |
| **voice** | A playing (or virtualised) instance of a request. |
| **channel** | The `SoundScript_Channel` enum value of a request (enum mapper labelled "SoundChannel", §7). Each channel has a volume setting; the client's internal name for the mixing group is "bus". |
| **intensity** | The node intensity of `script-table.md` §4.2 (`IntensityMultiplier`, variance, `UseParentIntensity`); for a Sound node it becomes the voice volume (§9.3). |
| **player properties** | The property values the music graph reads on the player entity (§10.2). Some are stored, some are fed by the world (§10.3). |

All integers are little-endian. "f32" is an IEEE single. Distances are metres; times are
seconds unless stated. DIDs quoted below are **observations** of this install; an
implementation finds them through the data (DID maps, includes, start-up script lists,
property names) at run time, never by hard-coding.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-05 (Steam, client data current on that date) with throwaway probes in the dirty-room scratchpad: **all 44,337** 0x0A records of `client_sound.dat`, `client_sound_aux_1.datx` and `client_local_English.dat` (envelope, trailer, every Ogg page with CRC check, Vorbis identification, comment and setup headers, full Vorbis decode with a pure-Rust decoder; every RIFF chunk walk; full decode of every WAV and of the 15 Ogg streams the first decoder rejected with an independent general-purpose decoder); **all 45,544** 0x2A records; all 7,526 script tables and 28,992 scriptlets walked node by node (126,562 Sound nodes, 21 SoundEnvironment nodes); all 384,950 property-set records and the property sets of 40,627 of the 40,643 0x47 descriptors (16 use a header form the probe did not read) searched for sound references; all 390,150 block maps and 5 block-map descriptors; the 464 monitor volumes of 332 descriptors; the 3,244,086 SED placements of all landblock-info records (from an SED list written by another dirty-room probe on the same day); the enum mappers and DID maps named in this spec. |
| BIN | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, PE time stamp 2026-09-27), disassembled on 2026-10-05 on address ranges only, around: the DB-type registration of types 0x12 and 0x40 and their deserialisers and constructor defaults; the Sound script-node class (field accessors, start routine, completion and intensity callbacks); the SoundEnvironment node and the environment selection; the sound manager's play gate and request queue; the OpenAL voice (creation, gain, distance law, position update, pitch, stop and fade-out job, virtualisation); the preference registration of master volume, voice limit and per-channel volumes; the start-up code that sets channel flags; the sound positioner. **Questions:** the meaning of the sound-info floats and flags and of the payload trailer; the run-time rules for gain, attenuation, pitch, random placement, looping, stopping, fades, voice limits, channels and reverb. **Why M1–M3 were not enough:** the records hold unlabelled floats and bit flags whose meaning the data cannot prove (correlations only); the playback rules exist only as code; no running client is available to this session (M3), and an audio capture of the original could not separate distance law, channel volume and node intensity. Network, login, launcher, anti-tamper and voice-chat code were not examined. |
| PS, ST, LB, ENV, ED, WS, IL | `property-system.md`, `script-table.md`, `landblock.md`, `environment.md`, `entity-descriptor.md`, `wstate.md`, `install-layout.md`, as companion specs. |
| DS | `research/notes/data-semantics.md` §9 (0x0A/sound-info outline from LotroCompanion), §6.5 (block maps). |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution), reported layout of the sound-info record and of the 0x0A envelope (via DS and `records.md`). |
| BIN2 | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)), repair session 2026-10-05.** Same build 4906.0071.1794.4035, address ranges only: the Sound node's start routine (its result codes, the internal-mute walk, the viewport redirect, the no-effects test, the volume floor), the sound manager's request function and the gate function it calls, the generic script-node runner (the routine that starts the children of a port, the routine that starts one node, the probability gate), the Random node's start routine, the client's WAV reader (which chunk tags it recognises), and the two routines that open Ogg payloads (decode at load, streaming) with the streaming refill and its loop rewind (which Vorbis-file library functions they call, in which order, with which arguments). **Questions:** what the original does with a node whose start fails (review item 6); the exact order and outcome of the request gates (§6.8); the source of §9.2 steps 2–4; whether `smpl` loop points are honoured (§3.4); which samples of an Ogg payload the original plays (§3.3.2, review item 2). **Why M1–M3 were not enough:** the data only show that 92 placeholder nodes exist and where they sit; how the runtime reacts to a failed start, and in which order requests are filtered, exist only as code; no running client is available (M3). Network, login, launcher, anti-tamper and voice-chat code were not examined. |
| REFDEC | The Xiph reference Vorbis libraries (general-purpose BSD-licensed software, not part of the game), driven by throwaway probes on 2026-10-05 over **all 40,874** Ogg payloads: libvorbis 1.3.7 of the dirty-room machine for the block size of every audio packet; and libogg 1.1 with libvorbis/libvorbisfile 1.0.1, built in the dirty scratch directory from Xiph's released source (the release whose version strings the install's library files carry), (a) decoding from the first page and (b) opened as a seekable source, seeked to sample 0, read up to the library's total length, the way BIN2 shows the client using it. The library source was read for its length computation (general engineering knowledge; nothing copied). |
| VORBIS | The public Xiph Vorbis I specification (general knowledge): §4.3.1 (decoding an audio packet: packet type, mode, window; packet sizes and the overlap of consecutive blocks, so that a packet after the first yields one quarter of the previous plus one quarter of the current block size per channel) and Appendix A (Ogg encapsulation; a page's granule position is the sample position at the end of the last packet completed on it; the final granule position marks the last sample and may cut the last block short). |
| RIFF | The public RIFF/WAVE and `fmt ` chunk description (general knowledge). |
| EFX | The public OpenAL EFX "EAX reverb" presets (OpenAL Soft `efx-presets.h`), whose 26 names match enum 0x23000080 (general knowledge). |

Marks: **Confirmed** = checked over every record of the kind on 2026-10-05; **BIN** = rests on
disassembly; **Reported** = third-party or general reference only; **Inferred** = fits all
observations but not proven; **Decision** = a Tessera choice where the data is silent,
always paired with an OPEN.

| # | Fact | Sources | Mark |
|---|---|---|---|
| A1 | 0x0A envelope: DID, size *S*, *S* audio bytes, u32 storage format, u32 reserved (§3.1) | LC; OBS (44,337 of 44,337); BIN (field order) | Confirmed |
| A2 | Storage format 2 = keep compressed (Ogg), 3 = decode to PCM at load (Ogg or WAV data), 7 = not audio (§3.2) | OBS (census); BIN (deserialiser: format ≠ 2 with Ogg bytes is decoded to PCM at load) | Confirmed census; meaning BIN |
| A3 | Ogg payloads: one logical Vorbis I stream each, 1–2 channels, 8–192 kHz, valid page CRCs, nothing after the last page (§3.3) | OBS (40,874 of 40,874) | Confirmed |
| A4 | 15 Ogg streams end with a page that holds one zero-length packet, and their granules run 63–84 samples ahead of the decoded position; the empty packet yields no samples and must not fail the decode (§3.3.3) | OBS; REFDEC (all 15 decode without error to the length of A7); review probe (an independent general-purpose decoder returned the same counts) | Confirmed 2026-10-05. Replaces the earlier "truncated last audio packet" reading. |
| A7 | Vorbis I stream length *N* = min(*G*, *T*), nothing cut at the start; granule census (§3.3.2) | OBS (40,874 of 40,874, packet by packet); REFDEC (a); VORBIS | Confirmed 2026-10-05 |
| A9 | The original plays decoded samples *A* … *A* + min(*L*, *T* − *A*) − 1 (skipped lead *A* and length *L* from the setup page and the page after it), and loops over that range (§3.3.2) | BIN2 (the executable imports the file library's open, sample-seek, time-seek, raw-seek, info, length, read and clear functions; both storage-format paths open seekable, seek to sample 0, cap the read at the total length; loops seek to time 0); OBS (version strings of the three library files in the install); REFDEC (b) over all 40,874 streams | BIN; Inferred that the shipped files behave like the released 1.0.1 source (OPEN in §3.3.2) |
| A5 | WAV payloads: PCM only (format tag 1), 16-bit (one 24-bit), 1–2 channels; chunk order varies (§3.4) | OBS (3,455 of 3,455; full decode) | Confirmed |
| A6 | 8 RIFF/DMAP records are DirectMusic audio-path data, the targets of the MUSIC DID map (§3.5) | OBS; `records.md` §9.2 | Confirmed |
| A8 | WAV loop points are not used: the client's WAV reader recognises only the `RIFF`, `WAVE`, `fmt ` and `data` tags; 5 files carry `smpl`, 2 of them with one forward loop, one of those behind a looping sound info (§3.4) | BIN2; OBS (all 3,455 WAV payloads, all 45,544 sound infos) | BIN / Confirmed census |
| I1 | 0x2A layout: DID, flags, five f32, payload DID, Pascal name; nothing after (§4.1) | LC (outline); OBS (45,544 of 45,544); BIN (field order, defaults) | Confirmed |
| I2 | Float meanings: two unused (always −1), priority, minimum distance, maximum distance (§4.2) | BIN (voice creation, distance law, priority score); OBS (ranges) | BIN |
| I3 | Flag bits 0x01 loop, 0x02 alternative voice mode, 0x04 positional with panning and reverb send, 0x40 distance-attenuated without panning; 0x10 not read by playback (§4.3) | BIN; OBS (census, correlation with channel and duration; median lengths recomputed 2026-10-05) | BIN (0x10 and the effect of 0x02: OPEN callouts in §4.3) |
| I4 | Where sound infos are referenced: Sound nodes (126,446 naming 28,710 distinct; 116 with 0), per-script and scriptlet DID lists, type-20 properties declaring code 0x12 (9 property names, 11,958 distinct targets), DID maps UNIQUEDB and UILAYOUT; 40,546 sound infos reached from nodes or properties (§5) | OBS (all script tables and scriptlets, all property-set records, the DID maps); the node figures and the DID-map targets re-checked by the review probe | Confirmed |
| P1 | Gain = min(volume, volume × attenuation × master × channel volume); a change below 0.0002 is not sent (§6.1) | BIN | BIN |
| P2 | Distance law: full volume within the minimum distance, silence beyond the maximum, linear in between; the audio library's own distance model is switched off (§6.2) | BIN | BIN |
| P3 | Pitch multiplier = (`Pitch` + 2) / 2 plus a uniform spread of ±`PitchVariance`, floored at 0.01 (§6.3) | BIN | BIN |
| P4 | Ramp-out is a linear gain fade in 10 ms steps over `RampOutTime` (§6.5) | BIN | BIN |
| P5 | At most `MaxPlayingSounds` voices (default 32) play; a new one-shot is skipped when full, loops are accepted and may be virtualised; priority score (2 − priority) × distance (§6.6) | BIN | BIN (stealing pass and the completion of a skipped tracked request: OPEN callouts in §6.6) |
| P6 | Listener = the active camera; sources are positioned relative to it; positions are updated only beyond 1.5 m (§6.7) | BIN | BIN |
| P7 | Request gates in the order of §6.8: back end present, sound-info filter, more than 250 waiting requests, suppression table, sound-info DID range, voice limit, then the gate function (disabled, volume ≤ 0.0002, exclusive channel, an optional check hook, positional range); refused loops and every queued request go to the pending list and are retried; an exclusive-channel request is always queued on its first attempt | BIN (play gate and request queue); BIN2 (order, the bound of 250 and the queueing rules re-read 2026-10-05) | BIN |
| C1 | Channels are the 9 values of the "SoundChannel" enum mapper; each has a volume setting named "Sound.<label>Volume" (§7) | OBS (enum, DID map, preference keys in IL); BIN (the client builds the setting names from the mapper labels) | Confirmed / BIN |
| C2 | Channel 0 plays at the Default volume; channel flags: Music 0x4+0x8, VO 0x4+0x10, Ambient 0x8, QuantizedPlayerMusic 0x2 (§7.2) | BIN | BIN (meaning of 0x4 and 0x8: OPEN callout in §7.2) |
| R1 | SoundEnvironment: the active reverb is the highest-priority running environment, ties to the newest; values select the standard EAX environment presets (§8) | BIN; EFX | BIN |
| N1 | Sound node field semantics and start algorithm (§9.1–§9.2) | BIN; OBS (126,562 nodes, 43 bytes each) | BIN / Confirmed |
| N2 | Node intensity = clamp(multiplier ± variance, 0, 1) × parent intensity (when `UseParentIntensity`); a playing tracked sound follows later intensity changes (§9.3) | BIN | BIN |
| N3 | §9.2 steps 2–4: the internal-object mute walk on `Sound_MuteWhenInternal` (PID 0xAF1), the viewport redirect on `Script_RunInViewport` (PID 0xB10), the no-effects state bit tested only for fire-and-forget nodes | BIN; BIN2 (re-read; the two PIDs checked against the property dictionary); OBS (5,004 object property sets set `Sound_MuteWhenInternal`, all to true) | BIN (which state sets the no-effects bit: OPEN) |
| N4 | A node whose start fails (for a Sound node: null `SoundScript_SoundID`) leaves no running instance and counts as completed at once; the failure is not passed to the parent and does not stop the remaining children of the port (§9.2 step 1, §9.6) | BIN2 (Sound node result codes; generic port runner; Random start); OBS (the 92 placeholder nodes: all fire-and-forget, each the only child of a weight-1 Random port; 80 of those Randoms are script roots, 12 sit in Collections) | BIN / Confirmed census |
| M1 | Area music and ambience are played by the avatar start-up script `SND_music_system_startup` (key 0x10000039, table 0x0700042E) switching on player properties (§10.1) | OBS (all avatar class tables; full graph walk) | Confirmed |
| M2 | `Area_Music` is never stored in data; its values are the music types of areas, dungeons and landmarks (§10.3) | OBS (967 case values vs 353 area, 333 dungeon, 283 landmark values) | Confirmed (the rule that sets it is OPEN) |
| M3 | Monitor volumes apply 0x78 property sets (music type, sky, water, fog) to the player inside a box (§10.3.4) | OBS (464 monitors) | Confirmed data; inside test Inferred |
| M4 | Music programs: tracked music sounds with ramp-out 1–4 s, silences of 5–30 s chosen at random, master fades as stepped timers (§10.4–§10.6) | OBS (graph census) | Confirmed |
| M5 | Block-map census of `Ambient_MusicType`, `Ambient_MusicRegion` and `Ambient_SoundEnum_BlockMapOverride` per region, palettes and out-of-range indices; 2 of the 20 block-map music types are case values of the graph's 41 `Ambient_MusicType` cases (§10.3.2) | OBS (all 390,150 block maps of regions 1–5 and 14, the 5 block-map descriptors; recounted 2026-10-05 from the full dump, case values recounted from the script graphs) | Confirmed |
| M6 | Four race-labelled music scripts in table 0x0700042E; Run nodes start three of them inside `Area_Music` cases, with no race Switch on the way; four race probability properties that no node of the music tables reads (§10.3.3) | OBS (tables 0x0700042E, 0x0700042F, 0x070017F8, 0x070017F9, scriptlets 0x0C009B74 and 0x0C000153; enum 0x23000097 labels; property dictionary) | Confirmed for the tables named |
| M7 | Script 0x10000039 holds 858 Run nodes (77 of them stop a script) over 74 distinct script keys; the reverb-script counts; table 0x0700042F holds 292 scripts (§10.7) | OBS (full walk of 0x0700042E and 0x0700042F; recounted 2026-10-05) | Confirmed |
| E1 | 240 placed object kinds play sounds from their start-up scripts; 43,274 SEDs place them (§11) | OBS | Confirmed |

---

<a id="s2"></a>
## 2. Record families and routing (Confirmed)

| Prefix | DB-object type (code, label in the "DBOType" mapper) | Archives (count) | Content | This spec |
|---|---|---|---|---|
| 0x2A | 0x12 SOUNDINFO | `client_general.dat` (45,544) | Sound info | §4 |
| 0x0A | 0x40 SOUND | `client_sound.dat` (12,621), `client_sound_aux_1.datx` (2,523), `client_local_English.dat` (29,193) | Payload: audio or DirectMusic data | §3 |

- The DB-object type codes are those a type-20 property definition declares
  (`property-system.md` §9) and those the client uses to load the records (BIN).
- Localised voice-over is in the locale archive: 29,192 of the 29,193 local payloads are
  Ogg, one is a WAV. A client loads the locale chosen for text.
- Routing follows `dat-container.md` (prefix 0x0A: sound, its aux members, then local).
  No 0x0A DID occurs in more than one archive in this install.
- Every 0x0A record except the 8 DirectMusic records is the target of at least one sound
  info (44,329 targets; 43,266 payloads have one sound info, 1,063 have 2–6).
- Every sound info names an existing payload (45,544 of 45,544).

---

<a id="s3"></a>
## 3. Sound payload record (prefix 0x0A)

<a id="s3-1"></a>
### 3.1 Layout (Confirmed, 44,337 of 44,337; BIN for field roles)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 4 | DID | `did` | Own DID (equal to the directory key in all records) |
| 4 | 4 | u32 | `size` *S* | Length of the container that follows, bytes |
| 8 | *S* | bytes | `data` | Ogg stream, RIFF WAVE, or RIFF DMAP (§3.3–§3.5) |
| 8 + *S* | 4 | u32 | `storage_format` | §3.2: 2 (37,644), 3 (6,685), 7 (8) |
| 12 + *S* | 4 | u32 | `reserved` | 0 in every record. The client reads it as a 32-bit word and keeps it (BIN). |

Invariants: payload length = *S* + 16; nothing follows; every record is stored
uncompressed in the archive. Smallest payload 376 bytes (a DMAP record), largest
40,916,068 bytes.

> **OPEN:** What the `reserved` word means. No playback code examined reads it after
> loading, and it is 0 in all 44,337 records. Readers keep the value and do not interpret
> it; writers store 0.

<a id="s3-2"></a>
### 3.2 Storage format (Confirmed census; meaning BIN)

| Value | Containers observed | Client behaviour (BIN) |
|---|---|---|
| 2 | 37,643 Ogg, 1 WAV (the 24-bit local WAV) | Keep the data as stored; Ogg is decoded while playing (streaming possible) |
| 3 | 3,454 WAV, 3,231 Ogg | Use as PCM: WAV data is used directly; if the data starts with the bytes `Ogg`, the client decodes the whole stream to PCM when it loads the record and fails the load if that decode fails |
| 7 | 8 DMAP | Not audio (§3.5) |

The value is a playback hint, not a codec identifier: always classify the container by its
magic bytes (§3.3–§3.5). An implementation may decode every sound fully or stream any of
them; format 3 only says that the original kept such sounds as PCM in memory (short
effects: format-3 WAVs last at most 2.2 s, format-3 Oggs at most 21.9 s with a median of
1.8 s, while format-2 Oggs reach 42 minutes).

<a id="s3-3"></a>
### 3.3 Ogg Vorbis payloads (Confirmed, 40,874 of 40,874)

<a id="s3-3-1"></a>
#### 3.3.1 Census

| Property | Observed |
|---|---|
| Logical streams per payload | exactly 1 (one beginning-of-stream page, one end-of-stream page, which is the last page) |
| Codec | Vorbis I, version 0, in every stream (identification header 30 bytes, framing bit set) |
| Channels | 1 (36,764), 2 (4,110) |
| Sample rate (Hz) | 44,100 (23,793), 48,000 (16,138), 16,000 (404), 32,000 (216), 96,000 (159), 22,050 (149), 24,000 (10), 192,000 (4), 8,000 (1) |
| Block sizes (log2 short/long) | 8/11 (40,310), 9/10 (563), 9/9 (1) |
| Page CRC | valid on every page |
| Granule positions | never −1, never decreasing |
| Bytes after the last page | none |
| Comment header | 0–14 user comments; none is a loop marker (no `LOOP…` keys) |
| Duration | 0.03 s to 2,543 s; local payloads total 32 h, `sound` 62 h, `sound_aux_1` 60 h |

<a id="s3-3-2"></a>
#### 3.3.2 Length and played range (Confirmed, 40,874 of 40,874; A7, A9)

Terms (VORBIS §4.3.1, Appendix A): every audio packet selects a block size (short or long,
from the identification header) through its mode number, which is in the first bits of the
packet. Decoding the first audio packet yields no samples; every later audio packet yields
one quarter of the previous audio packet's block size plus one quarter of its own, per
channel. A packet of length zero is not an audio packet: it yields nothing and does not
count as "previous" for a following packet (§3.3.3). For one payload:

- *T* is the number of samples per channel that all audio packets yield together;
- *G* is the granule position of the last page;
- *D*(*p*) is the number of samples yielded by the audio packets completed up to and
  including page *p*;
- decoded sample *i* (counting from 0) is the *i*-th sample a decoder outputs when it starts
  at the first page of the stream.

**Stream length (Vorbis I).** A decoder that follows the Vorbis I granule rules outputs
decoded samples 0 … *N* − 1, *N* = min(*G*, *T*): the last granule cuts the end, nothing is
cut at the start. What was checked, over every Ogg payload (OBS, REFDEC):

| Property | Streams |
|---|---|
| Every page except the last that completes an audio packet has granule = *D*(page): the stream starts at sample 0 and its granules count decoded samples exactly | 40,642 |
| Only one page completes audio packets, and it is the last page | 217 |
| Every page that completes an audio packet has granule = *D*(page) + 63, 64, 83 or 84, the amount varying by one within 9 of the streams; the last page holds one empty packet (§3.3.3) | 15 |
| A first audio page with granule < *D*(page) that is not the last page (which would mean samples to drop at the start) | 0 |
| *T* − *G* in the 40,859 streams of the first two rows | 0 to 1,023 (*N* = *G*) |
| *G* − *T* in the 15 streams of the third row | 63, 64 or 83 (*N* = *T*) |

The Xiph reference decoder of release 1.0.1, started at the first page, outputs exactly *N*
samples in all 40,874 streams (REFDEC (a)). The general-purpose decoder used by the review
output *N* for the 15 streams of the third row and for every multi-page stream of a
200-stream sample; it does not apply the end cut to single-audio-page streams (one such
stream in the sample: it output *T*).

**What the original plays (BIN2, A9).** The client decodes Ogg payloads with the Xiph
reference libraries shipped in its install (Ogg, Vorbis and Vorbis-file libraries, release
1.0.1 by their version strings; OBS). For both storage formats it opens the payload as a
seekable source, seeks to sample 0 with the file library's sample seek, asks the library for
the stream's total length *L*, and reads at most *L* samples per channel; a looping voice
seeks back to time 0 when it reaches that count and plays the same range again (BIN2).
That library version does not start at the first audio packet when the setup header and
audio packets share a page, and it computes *L* from the page after the headers. The
resulting played range, in words:

1. Let *h* be the page on which the third header packet (the setup header) completes, and
   *a* the page after it.
2. **Skipped lead** *A* = the samples yielded by the audio packets completed on page *h*
   (*D*(*h*)); 0 when page *h* completes no audio packet.
3. Let *Y* be the sum of the yields of the audio packets that begin and complete on page
   *a*, where each packet's yield is counted only when the packet before it also begins and
   completes on page *a* (so the first such packet contributes nothing).
4. *P* = granule(*a*) − *Y*, or 0 if that is negative. **Length** *L* = *G* − *P*.
5. The original plays decoded samples *A* … *A* + *M* − 1, with *M* = min(*L*, *T* − *A*).
   (If page *a* did not exist, *P* would be 0; that never occurs here.)

Census of the played range against the Vorbis I length (REFDEC (b), all 40,874 streams; the
library's total equals *L*, and the samples it delivers equal decoded samples *A* onwards,
in every stream):

| Case | Streams | Difference from decoded samples 0 … *N* − 1 |
|---|---|---|
| All other streams (no audio packet completes on the setup page; granules exact) | 30,247 | none: *A* = 0, *M* = *N* |
| The 15 streams of §3.3.3 | 15 | none in 14 (*M* = *T*); 0x0A01EE12 plays one sample fewer (its granule offset is 84 on page *a* but 83 at the end) |
| The first audio packet begins on the setup page and completes on page *a* | 2 | the last 128 samples are not played |
| Audio packets complete on the setup page | 10,610 (6,401 local, 4,185 in `client_sound.dat`, 24 in the aux archive) | the played range ends 128–2,048 samples before *N* (the yield of the first packet on page *a*); in 3,811 of them it also starts late, at *A* > 0 (commonest *A*: 576, 512, 768, 640, 1,600; largest 155,648 samples, 3.53 s, payload 0x0A00918C) |

253 looping sound infos (162 distinct payloads) name a payload of the last row, 61 of them
with *A* > 0; their loop period is *M*, not *N*.

**Rule for Tessera:** play the original's range (decoded samples *A* … *A* + *M* − 1) and
loop over exactly that range. A decoder that cannot start mid-stream decodes from the
first page and discards the first *A* samples. Tools (the sound browser, exports) may offer
the full Vorbis I stream (*N* samples) as an option.

> **OPEN:** The shipped library files were identified by their version strings only; that
> they behave like the released 1.0.1 source is inferred, not observed (no running client,
> M3). An owner observation that times a short looping sound with audio on its setup page
> would confirm it (§16, S-5).

<a id="s3-3-3"></a>
#### 3.3.3 Empty final packet (Confirmed, 15 streams; A4)

In 15 stereo streams (14 in `client_sound.dat`, one local; all written by two libvorbis
builds of 2014 and 2015) the **last page holds a single packet of length zero** (its segment
table is one lacing value 0); it carries the end-of-stream flag and the final granule
position *G*. These are exactly the streams whose granules run 63–84 samples ahead of the
decoded position (§3.3.2), so *G* exceeds *T* by 63, 64 or 83. Six of the 15 are the
payloads of positional sound infos, two of them looping (flags 0x05); the other nine are
behind sound infos without flags. No other stream contains a zero-length packet.

Rules:

- A zero-length packet yields no samples and is not an error; skip it.
- Decoding must not fail on these streams. A decoder that reports the empty packet as an
  error must be made to skip it.
- No samples are missing and nothing is zero-filled: the audio packets yield *T* samples,
  and the played range of §3.3.2 never asks for more than *T* − *A*. In two of the 15
  (0x0A01EDB7, 0x0A01EDB9) the library's total *L* is *T* + 1; the read simply ends at *T*.

This replaces the earlier reading of this section ("the last audio packet ends before its
residue data", with permission to drop that packet): the earlier census probe took the
empty packet for a truncated audio packet.

> **OPEN:** (clean-room implementation, `tessera_audio::decode`, 2026-10-07) Cases §3.3.2
> steps 1–5 do not cover; none occurs in this install, so `tessera-audio` handles them
> conservatively until the spec says otherwise: (1) page *a* has granule −1 (it completes
> no packet): *P* = 0, as if page *a* did not exist; (2) the last page has granule −1:
> *L* = *T* and *N* = *T*; (3) a zero-length packet on page *a* is skipped and is not "the
> packet before" the next one in step 3; (4) a packet with the header bit set after the
> three headers fails the decode; (5) page *a* does not exist (the setup header and every
> audio packet complete on the last page): steps 2 and 5 are applied as written, so
> *A* = *D*(*h*) = *T* and *M* = 0, the played range is empty and a looping voice has
> nothing to loop over (its source refuses to rewind); `tessera-audio` reports the stream
> (`OggIssue::NoPageAfterSetup`). Please confirm what the 1.0.1 libraries do in (1)–(5),
> in particular whether in (5) they play from sample 0 (*A* = 0, *M* = *N*) instead.
> Synthetic tests pin (1)–(5) (`crates/audio/tests/decode_synthetic.rs`, `open_case_*`).

<a id="s3-4"></a>
### 3.4 RIFF WAVE payloads (Confirmed, 3,455 of 3,455)

| Property | Observed |
|---|---|
| RIFF size field | always *S* − 8; form type `WAVE` |
| `fmt ` chunk size | 16 (3,368), 18 (71), 40 (16). Format tag 1 (PCM) in all, including the 40-byte ones, whose extra bytes are not an extensible-format block (their extension size is 0). Read the first 16 bytes and ignore the rest. |
| Channels / bits | 1 channel (3,444), 2 (11); 16-bit (3,454), 24-bit (1, local) |
| Sample rate | 44,100 (3,327), 22,050 (83), 48,000 (35), 32,000 (5), 24,000 (5) |
| Consistency | block align = channels × bits / 8 and byte rate = rate × block align in all; data length a multiple of the block align in all |
| Chunk order | `fmt ` first in 3,420; `bext` first in 19, `JUNK` first in 16. Files containing each other chunk: `PAD ` 2,597, `LIST`/`INFO` 675, `bext` 454, `minf` 39, `umid` 39, `regn` 38, `elm1` 35, `DGDA` 35, `JUNK` 34, `cue ` 22, `smpl` 5, `inst` 5, `_PMX` 5, `ovwf` 3, `elmo` 3, `iXML` 1 |
| Chunk padding | odd-sized chunks are followed by one pad byte; the walk ends exactly at *S* in all |
| Duration | 0.02–2.2 s in the sound archives; the local WAV is 6.2 s |

Rules: walk the chunks from offset 12 (8-byte header: four-character ID, u32 size; skip the
size rounded up to even); take the first `fmt ` and the first `data`; ignore everything
else. Five files carry a `smpl` chunk; in two of them it declares one forward loop, in the
other three no loop. 22 files carry a `cue ` chunk.

**Loop points are not used (BIN2).** The client's WAV reader recognises only the `RIFF`,
`WAVE`, `fmt ` and `data` tags; no `smpl` tag occurs in its code or constant data. One of the
two files with a `smpl` loop (0x0A014396, loop over sample frames 1,688–61,472 of 62,874) is
the payload of a looping sound info (0x2A006371, flags 0x05); the original loops the whole
`data` chunk, and so does Tessera (§6.4). The other (0x0A00218C) is behind a non-looping
sound info.

<a id="s3-5"></a>
### 3.5 DirectMusic records (Confirmed, 8 records)

0x0A000FE4–0x0A000FEB start with `RIFF` and form type `DMAP`: DirectMusic audio-path
configurations, 360–452 bytes, storage format 7. They are exactly the targets of the
"MUSIC" DID map (root key 0x1, sub-map 0x28000005), whose labels name 2D/3D, music/sound
and hardware/software paths. No sound info references them and the client's OpenAL path
does not use them (BIN). A reader classifies these records as "not audio" and skips them.

> **OPEN:** The client still knows a "DirectMusic" sound-provider name, the value space of
> the user setting "Sound.SoundProvider". Whether that provider can still be selected, and
> whether it would use these records, is not known. Tessera ignores the setting and never
> reads the DMAP records (Decision).

<a id="s3-6"></a>
### 3.6 Decode validation (Confirmed, 2026-10-05)

| Container | Records | Decoded to the end without error | Notes |
|---|---|---|---|
| Ogg Vorbis | 40,874 | 40,874 by an independent general-purpose decoder (review probe) and by the Xiph reference library of release 1.0.1, both from the first page and in the client's seekable mode (REFDEC); 40,859 by a pure-Rust decoder, which reports the empty final packet of the other 15 as an error | §3.3.2–§3.3.3 |
| RIFF WAVE (PCM) | 3,455 | 3,455 by an independent decoder; sample counts equal the data length in all | |
| RIFF DMAP | 8 | not audio | §3.5 |

All 44,329 audio payloads decode (ROADMAP M6 exit criterion 1), provided the rules of
§3.3.2–§3.3.3 (length, empty packet) and §3.4 (chunk walk) are followed.

---

<a id="s4"></a>
## 4. Sound-info record (prefix 0x2A)

<a id="s4-1"></a>
### 4.1 Layout (Confirmed, 45,544 of 45,544; field roles BIN)

| Offset | Size | Type | Field | Constructor default (BIN) | Observed |
|---|---|---|---|---|---|
| 0 | 4 | DID | `did` | | own DID in all |
| 4 | 4 | u32 | `flags` | 0 | §4.3 |
| 8 | 4 | f32 | `unused_a` | −1.0 | −1.0 in all |
| 12 | 4 | f32 | `unused_b` | −1.0 | −1.0 in all |
| 16 | 4 | f32 | `priority` | 0.0 | 0.0–1.0: 1.0 (32,028), 0.5 (9,133), 0.1 (1,646), 0.2 (1,239), 0.7 (884), 0.9 (300), 0.3 (177), 0.8 (64), 0.6 (43), 0.4 (23), 0.45 (4), 0.0 (3) |
| 20 | 4 | f32 | `min_distance` | 1.0 | 0.1–2,000 m, 52 values: 1.0 (36,428), 3.0 (4,656), 1.2 (2,465), 9.0 (387), 100 (333) … |
| 24 | 4 | f32 | `max_distance` | 50.0 | 1–2,020 m, 76 values: 32 (28,652), 15 (4,502), 64 (4,024), 30 (3,445), 60 (904), 40 (890) … |
| 28 | 4 | DID | `payload` | none | a 0x0A DID in all, all existing |
| 32 | var | Pascal string (`records.md` §1.4) | `name` | empty | ASCII asset name, 3–84 bytes, present in all, unique per record |
| — | | | *(end)* | | zero bytes left in all; record size 36–117 bytes |

- `min_distance` ≤ `max_distance` in all but 17 records. For those 17 the distance law of
  §6.2 still works (the minimum test comes first, so the sound is at full volume up to the
  minimum distance and silent beyond it).
- `unused_a` and `unused_b` are deserialised and copied but not read by any playback code
  examined (BIN). A writer stores −1.0.
- `name` is a tool label (it is not shown to players). Tessera may display it in tools.

<a id="s4-2"></a>
### 4.2 Field meanings (BIN)

| Field | Use |
|---|---|
| `priority` | Voice-limit ranking: every frame a positional voice gets the score (2 − `priority`) × its distance to the listener, a non-positional voice the score −1 (§6.6). Higher priority and nearer give lower scores (Inferred: lower scores are kept first). |
| `min_distance` | Within this distance a positional sound plays at full volume (§6.2). Also the reference distance handed to the audio library, which does not use it because its own distance model is off. |
| `max_distance` | Beyond this distance a positional sound is silent; a positional one-shot that would start beyond it is not started (§6.8). |
| `payload` | The 0x0A record to play. |

<a id="s4-3"></a>
### 4.3 Flags (Confirmed census; meanings BIN)

| Bit | Records with it set | Meaning |
|---|---|---|
| 0x01 | 1,377 | **Loop.** The voice loops (§6.4). Correlates with music beds, ambient loops and water/fire loops: median payload length 62.6 s over the 1,377 records, 64.2 s over their 1,270 distinct payloads (2 of the 1,377 records name WAV payloads, the rest Ogg). A request can override it (§9.2 sets no override). |
| 0x02 | 1 | Selects voice mode 4 instead of 2 when the request does not choose a mode (BIN; Sound-node requests never choose one). Both modes are subject to the voice limit (§6.8 item 6); any other effect is open (callout below). |
| 0x04 | 30,744 | **Positional**: distance law (§6.2), panning from the source position, and a send to the reverb environment (§8). |
| 0x10 | 546 | Not read by the playback code examined (short effects: median payload length 0.8 s over the 546 records). Meaning open (callout below). |
| 0x40 | 420 | **Distance-attenuated without panning**: distance law of §6.2, but the voice stays centred and has no reverb send. |
| others | 0 | never set |

Observed combinations: 0x04 (29,833), 0x00 (13,467), 0x01 (917), 0x14 (543), 0x05 (360),
0x40 (324), 0x41 (92), 0x45 (4), 0x15 (3), 0x07 (1). A sound with neither 0x04 nor 0x40 is
**non-positional** (2D): it ignores distance and plays centred at its volume.

> **OPEN:** What voice mode 4 (selected by flag 0x02, one record) changes beyond the voice
> limit, which treats modes 2 and 4 alike. Tessera plays the record like any other
> (Decision).

> **OPEN:** What flag 0x10 means. No playback code examined reads it; the 546 records with
> it are short effects. Readers keep the bit; Tessera ignores it (Decision).

---

<a id="s5"></a>
## 5. Where sound infos are referenced (Confirmed)

| Referrer | Field | Count |
|---|---|---|
| Sound script node (48) | `SoundScript_SoundID` | 126,446 nodes name 28,710 distinct sound infos; 116 nodes hold 0: 24 of them get the value from a driven property (§9.1), 92 are silent placeholders |
| Script-table per-script DID lists, scriptlet dependency lists | preload lists (`script-table.md` §2.5, §3) | 38,723 and 42,858 entries |
| Property sets (0x78, 0x79, 0x47) | type-20 properties declaring code 0x12, by value count: `QuestDispenser_VOSoundID` 20,369; `Usage_Sound` 11,148 (item use); `AIColorEventTable_Event_SoundID` 1,390; `PrivateEncounterTemplate_IntroVO` 908; `ErrorCategory_Sound` 673; `Hobby_Birding_BirdCall` 470; `UI_Util_SoundFile` 67 (UI sound categories, set 0x78000003); `Drama_SoundID` 67; `UI_Chat_FeedbackSound` 3 | 11,958 distinct |
| DID maps | UNIQUEDB (0x28000002): 11 weather sounds (rain loop, thunder), played by client code for weather; UILAYOUT (0x28000003): 1 | 12 |
| UI layouts (0x22) | UIData type 11 (`records.md` / research notes) | not counted here |

40,546 of the 45,544 sound infos are reached from sound nodes or property values (the union
of the 28,710 node targets and the 11,958 property targets). The other 4,998 are reached from
UI layouts, DID maps or client code, or are unused.

> **OPEN:** Which of the 4,998 sound infos that no node or property names are used, and by
> what (UI layouts were not counted). Nothing in playback depends on the answer.

> **OPEN:** The channel, positioning and timing of the weather sounds named by the UNIQUEDB
> DID map, and of other sounds that client code starts without a script node. Tessera plays
> them on the Ambient channel (7), non-positional, while the weather that names them is
> active (Decision).

Properties whose value is a sound DID can also **drive** a node's `SoundScript_SoundID`
(base-block driven map, `script-table.md` §4.2): observed sources are
`QuestDispenser_VOSoundID` (quest voice-over), `Drama_SoundID`, `Usage_Sound`, the
`GCParam_SoundScript_*` game-callback parameters and two others.

---

<a id="s6"></a>
## 6. Playback model (BIN unless marked)

<a id="s6-1"></a>
### 6.1 Gain chain

For a voice of a request with volume *v* (the node intensity at start, §9.3, a value in
[0, 1]) on channel *c*:

1. *a* = the distance factor of §6.2 for positional sounds (flag 0x04 or 0x40), else 1.
2. *g* = *v* × *a*.
3. The sent gain is min(*v*, *g* × *M* × *V*(*c*)), where *M* is the master volume setting
   and *V*(*c*) the channel's volume setting (§7). If the channel is muted (§7.2) the gain
   is 0.
4. A volume change (for example from an intensity update) is sent only if it differs from
   the last one by more than 0.0002; the per-frame distance update and the start always
   send the gain.
5. When the gain becomes exactly 0 and the channel is not muted: a non-looping voice is
   **stopped** (and completes, §6.4); a looping voice is **virtualised** (paused, kept)
   and resumes when its gain becomes positive again.

Volume settings are linear factors in [0, 1] (default 1.0 for master and every channel).

<a id="s6-2"></a>
### 6.2 Distance law

The client sets the audio library's distance model to "none" and computes attenuation
itself. With *d* the distance from the listener to the source, *m* = `min_distance` and
*X* = `max_distance`:

| Condition (tested in this order) | Factor *a* |
|---|---|
| *d* ≤ *m* | 1 |
| *d* ≥ *X* | 0 |
| otherwise | 1 − (*d* − *m*) / (*X* − *m*) |

The factor is recomputed every frame for every positional voice. There is no
air-absorption or rolloff exponent.

<a id="s6-3"></a>
### 6.3 Pitch

The pitch multiplier of a request is (`Pitch` + 2) × 0.5, so `Pitch` −1 … 1 gives 0.5 … 1.5
and 0 gives 1.0. If `PitchVariance` > 0, a value drawn uniformly from
[−`PitchVariance`, +`PitchVariance`] is added to the multiplier, and the result is raised to
at least 0.01. With `PitchVariance` ≤ 0 there is neither spread nor floor. The multiplier is
the playback-rate factor (1.0 = recorded rate); it is set once when the voice starts.

> **OPEN:** A stored `Pitch` of −2 or below with `PitchVariance` ≤ 0 would give a multiplier
> ≤ 0, which the audio library rejects; what the original then plays is not known. No such
> node occurs in this install: all 20 nodes with `Pitch` ≤ −2 have a positive variance, so
> their multiplier is floored at 0.01.
> Tessera uses 0.01 (Decision).

<a id="s6-4"></a>
### 6.4 Looping and completion

- A voice loops when its sound info has flag 0x01 (no request in the data overrides this).
  Looping is seamless over the whole payload: every sample of the played range (§3.3.2 for
  Ogg, the `data` chunk for WAV), then again from the first; no loop points are used (§3.4).
- A non-looping voice **completes** when its audio ends, when the gain rule of §6.1 step 5
  stops it, or when it is stopped (§6.5).
- A tracked request (Sound node with `FireAndForget` = 0, §9.2) reports completion to its
  node; a looping one therefore completes only when stopped.

<a id="s6-5"></a>
### 6.5 Stopping and ramp-out

When a playing voice is stopped (its Sound node stops because the script branch above it
stops, §9.2), and its `RampOutTime` *T* > 0: the gain at that moment is lowered linearly to 0
over *T* seconds in steps of 10 ms (gain = start gain × (1 − elapsed / *T*)), then the voice
stops. With *T* = 0, or when the voice is virtualised, it stops at once. Negative times are
treated as 0. There is no fade-in for sounds: a new sound starts at its full gain.

<a id="s6-6"></a>
### 6.6 Voice limit

- `MaxPlayingSounds` (user setting "Sound.MaxPlayingSounds", default 32) bounds the number
  of playing voices. When that many play, a new **non-looping** request is silently not
  started (the call still reports success and returns a request number); looping requests
  are always accepted and may be virtualised. The bound applies to requests in voice modes
  0, 2 and 4; client code that uses other modes bypasses it. Sound-node requests use mode 2
  or 4 (§4.3), so they never bypass it.
- Every frame each voice gets a score: −1 for non-positional voices, else
  (2 − `priority`) × distance (BIN). Inferred: lower scores are kept audible first and
  low-ranked looping voices are virtualised.
- When more than 250 requests are already waiting, a new request is refused (§6.8 item 3).

> **OPEN:** How a tracked Sound node (`FireAndForget` = 0) whose request was skipped by the
> voice limit completes. The manager reports success, so the node waits for a completion
> of that request; whether the manager ever delivers one was not traced. Tessera completes
> the node at once (Decision).

> **OPEN:** The exact voice-stealing pass: which voices are virtualised or stopped when the
> scores are ranked, and how channel flag 0x4 (§7.2) enters it. Tessera keeps the
> `MaxPlayingSounds` voices with the lowest scores audible and virtualises the other
> looping voices (Decision).

<a id="s6-7"></a>
### 6.7 Listener, position and panning

- The listener is the **active camera**: every frame, each positional voice's position is
  expressed relative to the camera position, in the camera's frame (right, up, forward),
  and handed to the audio library together with its velocity; the library's listener
  stays at the origin.
- A source's position is handed to the audio library only while the source is more than
  1.5 m from the listener; closer than that the last position is kept, and a source that
  starts within 1.5 m stays centred. This avoids panning flips at very short range.
- Only flag-0x04 sounds are panned and sent to the reverb (§8); flag-0x40 sounds use the
  distance law but stay centred.
- Source positions come from the request's positioner (§9.2 step 6).

> **OPEN:** (clean-room implementation, 2026-10-07) The panning law is not stated: how the
> audio library turns a camera-relative position into per-speaker gains (pan curve, centre
> level, front/back handling, elevation), and whether gain changes are stepped or smoothed.
> `tessera_audio::engine` uses a constant-power pan on the lateral component of the source
> direction (centre 0.7071 per side), ignores front/back and elevation, and spreads each gain
> change over 64 samples. Please state the original's law if it can be observed.

<a id="s6-8"></a>
### 6.8 Requests that are not played (BIN, BIN2; P7)

Before the sound manager is asked at all, a fire-and-forget Sound node with a volume
≤ 0.005 completes without a request (§9.2 step 9). The manager then checks a request in
this order; the first check that applies decides:

1. No audio back end is running → **refused**.
2. **Sound-info filter.** The manager holds either an exclusion set of sound-info DIDs (a
   request whose sound info is in it is refused, unless the request is marked to bypass the
   filter) or an inclusion set (a request whose sound info is not in it is refused) →
   **refused**. See the callout below.
3. More than 250 requests are already waiting (loading or pending) → **refused**.
4. The sound info is in the manager's per-DID suppression table → reported as **accepted**,
   nothing plays (callout below).
5. The DID is outside the DID range of sound infos (prefix 0x2A, DB-object type 0x12) →
   **refused**.
6. **Voice limit** (§6.6): the request does not loop, its voice mode is 0, 2 or 4, and
   `MaxPlayingSounds` voices already play → **silently skipped**: reported as accepted with
   a request number, nothing plays.
7. Sound is disabled or suspended (two state flags of the manager) → code *disabled*.
8. The master volume or the channel's volume is ≤ 0.0002 → code *silent*.
9. The channel is exclusive (channel flag 0x10, §7.2) → code *queued*. A request's first
   attempt on an exclusive channel is always queued; when it is retried from the pending
   list it is queued again only while that channel has a playing voice.
10. An optional check installed by client code returns a non-zero code → that code.
11. The sound is positional and its source is in a landblock that is not loaded, or farther
    than `max_distance` from the listener at that moment → code *out of range*.
12. Otherwise the voice is created and starts (§6.1–§6.7).

A request that gets a code in items 7–11 is put on the **pending list** when it loops or
when the code is *queued*; it is retried later, so ambient loops start once they come into
range or their volume is raised, and lines on the exclusive VO channel play one after
another. Any other request with a code (a refused one-shot) is **dropped**.

> **OPEN:** What fills the filter sets of item 2. The player properties
> `Filter_SoundArray` and `Account_Filter_SoundArray` (user sound filters) are the likely
> source, but the link was not traced, nor which requests may bypass the filter. Tessera
> has no filter UI and applies no filter (Decision).

> **OPEN:** What fills the suppression table of item 4. Tessera has none (Decision).

> **OPEN:** What the optional check of item 10 tests. Tessera has none (Decision).

---

<a id="s7"></a>
## 7. Channels and volume settings

<a id="s7-1"></a>
### 7.1 Channel enum (Confirmed; settings BIN)

`SoundScript_Channel` (PID 0x4EA) is an enum value of the mapper the EMAPPER DID map labels
"SoundChannel" (key 0xBA → 0x2300034D in this install, base 0x2300034E). For every label
*L* of that mapper the client creates a volume setting named "Sound.*L*Volume" (BIN); these
are exactly the nine per-channel volume keys of the `[Sound]` section in
`install-layout.md`. The other keys of that section are global: `MasterVolume`,
`MaxPlayingSounds`, `EAXEnabled`, `SoundDisabled`, `MusicDisabled`, `PersistentSound`,
`FocusBehavior`, `SoundFeatures`, `SoundProvider` (BIN: registered with these names).

| Value | Label | Volume setting | Sound nodes (§9) |
|---|---|---|---|
| 0 | Default | DefaultVolume | 9,516 |
| 1 | Combat | CombatVolume | 25,173 |
| 2 | Music | MusicVolume | 19,918 |
| 3 | UI | UIVolume | 15 |
| 4 | VO | VOVolume | 16,508 |
| 5 | QuantizedPlayerMusic | QuantizedPlayerMusicVolume | 2,216 |
| 6 | PlayerMusic | PlayerMusicVolume | 4,664 |
| 7 | Ambient | AmbientVolume | 17,675 |
| 8 | SFX | SFXVolume | 30,877 |

Implementations read the labels at run time and name their own settings after them. The
master volume ("Sound.MasterVolume") multiplies all channels (§6.1).

<a id="s7-2"></a>
### 7.2 Channel behaviour (BIN)

| Item | Rule |
|---|---|
| Default channel | A request on channel 0 plays at the Default volume; for channel-level bookkeeping (limits, pausing) the client files it under channel 8 (SFX). |
| Flags set at start-up | Music (2): 0x4 and 0x8. VO (4): 0x4 and 0x10. Ambient (7): 0x8. QuantizedPlayerMusic (5): 0x2. Others: none. |
| 0x1 | Mute: gain 0 (§6.1), voices kept. |
| 0x2 | Voices of this channel are started through a separate pool (Inferred: player-music synchronisation; out of scope). |
| 0x4 | Read by the voice-limit pass (exact effect: callout below; presumably it keeps the channel's voices audible). |
| 0x8 | When the client suspends sounds (focus loss or loading), voices of this channel are paused and kept instead of being stopped (Inferred from the stop path). |
| 0x10 | Exclusive: one voice at a time (§6.8 item 9). VO lines therefore never overlap. |

> **OPEN:** Exact effects of channel flags 0x4 and 0x8 and of the settings "MusicDisabled",
> "PersistentSound" and "FocusBehavior". Tessera: "MusicDisabled" mutes channel 2
> (Decision).

---

<a id="s8"></a>
## 8. Reverb environments (BIN; values Confirmed)

- A SoundEnvironment node (kind 62, §9.4) running on an entity registers an environment
  {`SoundEnvironmentScript_Name`, `SoundEnvironmentScript_Priority`, the node's intensity}
  with the sound manager and removes it when the node stops.
- The **active environment** is the registered one with the highest priority; among equal
  priorities the most recently registered wins. With none registered the client applies
  value 0 (GENERIC) at intensity 0. (No negative priority occurs in the data.)
- `SoundEnvironmentScript_Name` is an enum value of mapper 0x23000080 (EMAPPER label
  "SoundEnv"): 0 GENERIC, 1 PADDEDCELL, 2 ROOM, 3 BATHROOM, 4 LIVINGROOM, 5 STONEROOM,
  6 AUDITORIUM, 7 CONCERTHALL, 8 CAVE, 9 ARENA, 10 HANGAR, 11 CARPETEDHALLWAY, 12 HALLWAY,
  13 STONECORRIDOR, 14 ALLEY, 15 FOREST, 16 CITY, 17 MOUNTAINS, 18 QUARRY, 19 PLAIN,
  20 PARKINGLOT, 21 SEWERPIPE, 22 UNDERWATER, 23 DRUGGED, 24 DIZZY, 25 PSYCHOTIC,
  26 Undef. Values 0–25 select the standard EAX environment preset of the same name
  (EFX); 26 and anything larger select GENERIC (BIN).
- The reverb is applied only when the user setting "Sound.EAXEnabled" is on and the
  audio device supports effects. Only flag-0x04 voices feed it (§4.3).
- Census: 21 nodes, priority 0 except one node with priority 1 (value 18) and one with
  priority 5 (value 8); intensities 0.09–1.0. 14 of them form the seven "SND_EAX_*" scripts
  of table 0x0700042F (and their copies in 0x070017F9) that the music graph starts per
  location (§10.7).

> **OPEN:** How the environment's intensity is applied (presumably as the reverb send or
> effect gain). Tessera: scale the reverb send by it (Decision).

> **OPEN:** (clean-room implementation, 2026-10-07) The parameters of the 26 presets
> (decay time, damping, reflection and reverb levels and delays, diffusion, density) are
> not in this spec; "the standard EAX environment preset of the same name" names them but
> does not give the values. `tessera_audio::engine` uses its own placeholder values with a
> generic comb/all-pass reverb (decision 0019) until the values are specified. Also open:
> the send level of a flag-0x04 voice into the reverb (Tessera: the voice's own gain).

---

<a id="s9"></a>
## 9. Sound and SoundEnvironment nodes — facts for `script-table.md`

`script-table.md` is locked to this writer; this section holds the sound-node facts that
belong there (its §5.3 lists only the field order). Field names are interoperability
identifiers (property names in the dictionary).

<a id="s9-1"></a>
### 9.1 Sound node (kind 48) fields (layout Confirmed, 126,562 of 126,562, always 43 bytes; meanings BIN)

| Offset | Field (property, PID) | Type | Dictionary default; min/max | Meaning | Observed |
|---|---|---|---|---|---|
| 0 | `SoundScript_SoundID` (0x8C) | DID (0x2A) | none | Sound info to play | 0x2A in 126,446; 0 in 116 (24 driven, 92 placeholders) |
| 4 | `SoundScript_FireAndForget` (0x2C3) | bool8 | 1 | 1: start and finish at once; 0: the node runs until the sound completes and stops the sound when the node stops | 1 in 98,377 |
| 5 | `SoundScript_MoveWithEntity` (0x1D3) | bool8 | 1 | 1: the source follows the entity every frame; 0: fixed at the entity's position when the sound starts | 1 in 116,916 |
| 6 | `SoundScript_UseCameraPosition` (0x702) | bool8 | 0 | 1: the source is the camera position (live), not the entity | 1 in 6 (all with `MoveWithEntity` 1) |
| 7 | `SoundScript_MinRandomDistance` (0x2C2) | f32 | 0; 0–250 | Lower bound of the random offset distance, metres | 0 in 111,247; 15–60 typical |
| 11 | `SoundScript_MaxRandomDistance` (0x1D2) | f32 | 0; 0–250 | Upper bound, metres | 0 in 110,344; 60, 45, 70, 80 … |
| 15 | `SoundScript_Offset` (0x44F) | vec3 | (0, 0, 0) | Offset from the entity origin in the entity frame, metres | (0, 0, 0) in 124,229 |
| 27 | `SoundScript_Channel` (0x4EA) | u32 enum | 0 | Channel (§7) | table in §7.1 |
| 31 | `SoundScript_RampOutTime` (0xEC) | f32 | 0; 0–4 | Fade-out time when stopped, seconds (§6.5) | 0 in 97,755; 4 (6,677), 3 (6,120), 2, 0.2, 1 … |
| 35 | `SoundScript_Pitch` (0x583) | f32 | 0; −1–1 | Pitch shift (§6.3) | 0 in 105,403; −0.2, −0.1, −0.5, −0.3, 1, … |
| 39 | `SoundScript_PitchVariance` (0x3A2) | f32 | 0; 0–1 | Random pitch spread (§6.3) | 0 in 64,872; 0.2, 0.1, 0.05, 0.25 … |

The dictionary's minimum and maximum are **not** enforced on stored values: `Pitch` lies
outside [−1, 1] in 59 nodes (−8 to 2), `PitchVariance` is negative in 202 (no spread is
applied, §6.3) and above 1 in 26 (up to 2), `RampOutTime` exceeds 4 s in 5 (up to 15 s), a
random distance exceeds 250 m in 9 (up to 275 m), and the minimum random distance exceeds
the maximum in 3. Readers keep the stored values; the start algorithm (§9.2) clamps only
the random distances and the pitch floor.

Further census: no Sound node has ports or targets (126,562 of 126,562); `UseParentIntensity`
is 1 in all but 9; `IntensityMultiplier` 1.0 in 94,733 and 0.1–0.95 otherwise; variance 0 in
122,267 and 0.02–0.6 otherwise; `Probability` 1.0 in 126,123, 0 in 146 (never runs).
Driven fields (`script-table.md` §4.2): 31 nodes. 26 drive `SoundScript_SoundID` from a
game property (2 of them also both random distances, pitch, pitch variance and channel from
game-callback parameters, 1 also the channel); 5 drive only their intensity.

<a id="s9-2"></a>
### 9.2 Starting a Sound node (BIN)

When a Sound node runs (after the generic probability test and intensity computation of
`script-table.md` §4.2 and §9.3 here), the original does, in order (BIN; steps 1–4 and 9–10
re-read in BIN2):

1. If `SoundScript_SoundID` (after driving) is the engine's null DID, the node's start
   **fails**: it starts nothing and leaves no running instance. A failed start counts as
   **completed at once** (§9.6): the parent is not told about the failure and treats the
   node like a fire-and-forget node that has finished. The start also fails, in the same
   way, when the running entity is gone or no sound manager exists. A non-null value outside
   the sound-info DID range is refused later by the sound manager (§6.8 item 5), which also
   completes the node. Either way 0 plays nothing.
2. **Internal-object mute:** walk from the running entity up its parent chain, stopping at
   the first entity whose `Sound_MuteWhenInternal` (PID 0xAF1) is stored as true. If that
   entity has a parent (it is held or contained), the node completes without sound; if it
   has none, the walk ends there and the node goes on. An absent or false value continues
   the walk upwards. (5,004 object property sets set the property, all to true.)
3. **Viewport redirect:** the walk ends at the topmost entity of the chain. If that entity
   has `Script_RunInViewport` (PID 0xB10) = true and the client has a viewport entity
   (Inferred: the locally controlled character), the viewport entity is used as the sound's
   entity from here on.
4. If `FireAndForget` = 1 and the running script context is in a "no effects" state, the
   node completes without sound. Tracked nodes (`FireAndForget` = 0) skip this test.
5. **Random offset:** clamp `MinRandomDistance` and `MaxRandomDistance` to ≥ 0 and, if the
   minimum exceeds the maximum, set it to the maximum. Draw three independent uniform
   values in [−1, 1] and normalise them to a direction; if their length is ≤ 10⁻⁶ use
   straight up (0, 0, 1). Draw a distance uniformly in [minimum, maximum]. The offset is
   direction × distance + `Offset`. (With both distances 0 the offset is just `Offset`.)
6. **Positioner:** the source position is the entity's position plus the offset rotated by
   the entity's orientation, taken live every frame when `MoveWithEntity` = 1, or frozen at
   this moment when it is 0; with `UseCameraPosition` = 1 the camera position is used
   (live) instead of the entity's. If the entity is gone the position is invalid and the
   voice is silent.
7. **Volume:** if `UseParentIntensity` = 1 the volume is the node's intensity (§9.3);
   otherwise `IntensityMultiplier` plus a uniform draw in ±`IntensityMultiplierVariance`.
   The result is clamped to [0, 1].
8. **Pitch** per §6.3; `Channel` and `RampOutTime` are copied to the request.
9. If `FireAndForget` = 1 and the volume ≤ 0.005, the node completes without sound.
10. The request goes to the sound manager (§6.8 gates). If it is refused, the node
    completes. A request that is queued, put on the pending list, skipped by the voice limit
    or suppressed counts as accepted (the manager reports success).
11. If `FireAndForget` = 1 the node **completes immediately**; the sound plays on and is
    never stopped by the script.
12. If `FireAndForget` = 0 the node **stays running**: it completes when the voice
    completes (§6.4); if the node is stopped first (its branch stops, e.g. a Switch case
    changes), the voice is stopped with its ramp-out (§6.5).

> **OPEN:** Which state sets the "no effects" bit of step 4 (for example a hidden,
> culled or loading entity). Tessera never sets it (Decision).

> **OPEN:** 59 fire-and-forget nodes name a looping sound info (flag 0x01). Step 11 leaves
> such a voice with no owner that could stop it; whether the original lets it loop until
> the voice limit, a distance cull or the entity's removal ends it was not traced. Tessera
> stops it when its entity is removed (Decision).

> **OPEN:** (clean-room implementation, 2026-10-06) Step 5 clamps the random distances to
> ≥ 0 but does not say what happens to a stored value that is not finite (NaN or
> ±infinity; none in this install, only hostile data). A uniform draw in [*x*, +∞] is
> undefined and would give an infinite or NaN source position. `tessera-formats`
> (`SoundNode::random_distance_range`) treats every non-finite bound as 0, as it does NaN.
> Please confirm or state the original's behaviour.

<a id="s9-3"></a>
### 9.3 Intensity of a node instance (BIN; applies to every node kind)

See [`script-runtime.md`](script-runtime.md) §4 for the settled rule (clamp only when the variance is not 0; `instruments.md` Q8) and for driven multiplier changes.

When any node starts, its instance intensity is:

*i* = clamp(`IntensityMultiplier` + *u*, 0, 1) × *p*

where *u* is drawn uniformly from [−`IntensityMultiplierVariance`, +`IntensityMultiplierVariance`]
(skipped when the variance is 0), and *p* is the parent instance's intensity when
`UseParentIntensity` = 1, else 1. Intensities therefore multiply down the graph. When a
driven property changes an instance's multiplier (for example `Script_IntensityMultiplier`
driven by `Snd_SubMix_MusicVolume`, §10.6), the new intensity is passed down, and a playing
tracked sound (fire-and-forget 0) takes it as its new volume at once (§6.1 step 4).

<a id="s9-4"></a>
### 9.4 SoundEnvironment node (kind 62; layout Confirmed, 21 of 21, 8 bytes)

| Offset | Field (PID) | Type | Meaning |
|---|---|---|---|
| 0 | `SoundEnvironmentScript_Name` (0x21F) | u32 enum 0x23000080 | Environment preset (§8) |
| 4 | `SoundEnvironmentScript_Priority` (0x3F1) | i32 | Priority (§8) |

Starting it registers the environment (§8) with the node's intensity and keeps the node
running; stopping it unregisters it. It has no ports or targets in the data.

<a id="s9-5"></a>
### 9.5 Where sound nodes are (Confirmed)

66,878 in tables, 59,684 in scriptlets. Their parent nodes: Random (99,298: one of several
variants is chosen), Switch (9,598: by surface, creature type, sex, species, mount, volume
type …), Sequence (5,896), Timer (4,209), Collection (3,576), animation-node event ports
(2,279: footsteps and effects, `script-table.md` §6.4), script roots (1,380), Scriptlet
parameters (169), When (72), WhenRunning (54), OnStop (28), GameCallback (3). Music-channel
nodes sit almost all in table 0x0700042E (14,398), 0x070017F8 (3,358) and scriptlet
0x0C009B74 (2,056).

<a id="s9-6"></a>
### 9.6 Outcome of a node start (BIN2; applies to every node kind; facts for `script-table.md`)

Starting a node has one of three outcomes:

| Outcome | Meaning | Examples |
|---|---|---|
| **running** | The node leaves a running instance under its parent; it completes later (its sound ends, its timer runs out, its children finish) or is stopped with its branch. | A tracked Sound node whose request was accepted; a SoundEnvironment node. |
| **completed at once** | The node did its work (or nothing) and leaves no running instance. | A fire-and-forget Sound node; a Sound node muted by §9.2 steps 2, 4 or 9 or refused by the manager; a node skipped by its `Probability` draw. |
| **failed** | The node could not start (a Sound node with a null `SoundScript_SoundID`, no entity, no sound manager). It leaves no running instance. | The 92 placeholder Sound nodes (§9.1). |

Rules (BIN2):

1. The runner that starts the children of a port starts them one after another, in stored
   order. It ignores each child's outcome: a failed child neither stops the remaining
   children of that port nor is reported to the parent. (The runner stops early only on an
   internal abort condition that a failed start does not raise.)
2. A child that leaves no running instance, whether it completed at once or failed, is
   complete as far as its parent is concerned. A **failed start is therefore handled
   exactly like completion at once.**
3. A Random node (kind 17) runs the children of its chosen port; if none of them is left
   running afterwards, the Random node itself completes at once. Sequence steps and
   Collections likewise finish when none of their children is left running
   (`script-table.md` §7.5–§7.6; their start routines were not re-read in BIN2).

Census of the 92 placeholders (OBS, N4): all are fire-and-forget with `Probability` 1, and
each is the only child of a weight-1 port of a Random node; 80 of those Random nodes are
script roots and 12 sit inside Collections. None is a Sequence step or a Timer child. When
the Random draw picks a placeholder port, the Random completes at once and nothing plays
(Inferred: the placeholders are "sometimes silent" variants).

---

<a id="s10"></a>
## 10. Music and ambience

The chain from a place to a sound record, in short:

- **Area music:** landblock (*R*, *X*, *Y*) → its block map → key `Area_DID` → the area
  object's property set (DID + 0x09000000) → its `Ambient_MusicType` value *v* → the
  player's `Area_Music` = *v* (§10.3.1) → the case with value *v* of the `Area_Music` Switch
  in the avatar's start-up script (§10.1) → Sound nodes → `SoundScript_SoundID` (0x2A sound
  info, §4) → its payload (0x0A, §3).
- **Ambience and reverb:** block-map, cell and monitor-volume values of
  `Ambient_SoundEnum_BlockMapOverride`, `Ambient_MusicType` and `Environment_LocalSky`, plus
  `World_IsNight` and friends → player properties (§10.3) → Switch cases under the music
  case → Run nodes → scripts of table 0x0700042F → Sound nodes on channel 7 and
  SoundEnvironment nodes (§8, §10.7) → sound infos → payloads.
- The enum values themselves (mapper 0x23000140 "AmbientMusicType", 0x2300016B for the
  ambient override, 0x230000E9 for skies) carry no reference to sound records; only the
  script graph connects them.

<a id="s10-1"></a>
### 10.1 Entry point (Confirmed)

There is no music table keyed by area. Music, ambient beds and reverb environments are
played by one **script graph** that every player avatar runs:

- Every avatar class table lists start-up script names (`ScriptTable_StartupScriptNames`,
  `script-table.md` §8.1). One of them is the script key labelled
  `SND_music_system_startup` (0x10000039 in enum 0x23000097). Its body is in the shared table
  0x0700042E, which every avatar closure includes.
- Its root is a persistent Switch on `Entity_IsThePlayer` whose true case contains
  everything, so only the local player's avatar makes area music.
- 0x070017F8 (with 0x070017F9) is an older near-copy of these tables that only table
  0x07001819 includes; no avatar closure reaches it. All 14 avatar closures include
  0x0700042E and 0x0700042F.
- Inside, a Switch on `Music_Monitor` (player-music listening, out of scope) defaults to a
  Collection of two branches: a Switch on `HousingPropertyHook_IntMusic` (111 house-interior
  cases) whose default is the Switch on **`Area_Music`** (699 cases in this script), and a
  Label branch (a silence: a Random over one-shot Timers of 10–30 s) that 27 Goto nodes
  inside the music cases jump to (`script-table.md` §7.7).
- Under each case, further Switches on `Volume_ContentLayer`, `World_IsNight`,
  `Environment_LocalSky`, `PrivateEncounter_Template_DataID`, `Snd_Music_Advance`,
  `Ambient_MusicType`, `Ambient_SoundEnum_BlockMapOverride`, `Volume_IsOutside`,
  `Combat_Violent` and instance-stage booleans select the music program (§10.4) and start
  ambient and reverb scripts by **Run** nodes (§10.7).

An implementation runs this graph with its generic script evaluator; it needs no
area-specific code. What it must provide are the player properties of §10.2.

<a id="s10-2"></a>
### 10.2 Player properties the graph reads (Confirmed names; sources §10.3)

| Property (PID) | Type | Switch nodes (in script 0x10000039) | Source |
|---|---|---|---|
| `Entity_IsThePlayer` (0x5B0) | bool | 1 | true on the local player's avatar |
| `Area_Music` (0x10001493) | enum 0x23000140 ("AmbientMusicType") | 3 (699 cases); 967 distinct case values over 0x0700042E, 0x070017F8 and 0x0C009B74 | §10.3.1 |
| `Ambient_MusicType` (0x3F8) | enum 0x23000140 | 20 | block map (§10.3.2), monitor volumes (§10.3.4) |
| `Ambient_SoundEnum_BlockMapOverride` (0x46) | enum 0x2300016B | 4 | block map, cell properties, monitor volumes |
| `Environment_LocalSky` (0x1D8) | enum 0x230000E9 | 4 | block map / monitor volumes (`environment.md` §7.1) |
| `World_IsNight` (0x1F0) | bool | 150 | game clock (§10.3.5) |
| `Volume_IsOutside` (0x53B) | tri-state, default 255 | 1 | indoors/outdoors (§10.3.5) |
| `Volume_ContentLayer` (0x621) | enum via DID map 0x28000012 | 138 | current content layer (§10.3.5) |
| `Combat_Violent` (0x10000C6E) | bool | 7 | combat state; false in the offline walker |
| `HousingPropertyHook_IntMusic` (1), `Music_Monitor` (73), `Music_inMonitor`, `PrivateEncounter_Template_DataID`, `Dungeon_CurrentDungeon`, instance-stage booleans | various | | housing, player music, instances: absent in the walker (the Switches take their default ports) |
| `Snd_Music_Advance` (0x10000CDE), `Snd_IsOn` (0x10000DCE) | int, bool | 1,277 and 36 | **set by the graph itself** (SetProperty nodes) |
| `Snd_SubMix_MusicVolume` (0x10001474), `Snd_SubMix_Amb_Volume` (0x10000ED0) | float, default 1.0 | drive intensities | set by the master-fade scripts (§10.6) |

Properties are looked up by name in the dictionary at run time. A property that is absent
on the player makes its Switch take the Default port (`script-table.md` §7.1).
Precisely: it reads its dictionary default, else its type's zero value, and takes the Default port only when no case matches that value (`script-runtime.md` §6.2, R27).

> **OPEN:** (clean-room implementation, 2026-10-08) The table row for
> `HousingPropertyHook_IntMusic`, `Music_Monitor` … says "(the Switches take their default
> ports)", but `script-runtime.md` §6.2 (OBS-R) counts 13 of the 66 Switches on the first two
> without a Default port: on an avatar that has neither property those select nothing. The
> implementation follows `script-runtime.md` §6.2; please align the row.

<a id="s10-3"></a>
### 10.3 Where the values come from

<a id="s10-3-1"></a>
#### 10.3.1 `Area_Music` (Confirmed data; setting rule OPEN)

`Area_Music` occurs in **no** stored property set. Its case values come from three kinds of
game object:

| Object (0x79 property set) | Property | Objects | Distinct values | Covered by `Area_Music` cases |
|---|---|---|---|---|
| Area (has `Area_Name`, `Area_Territory` …) | `Ambient_MusicType` | 675 (372 values) | 353 among the 615 areas that block maps name | 345 of 353 |
| Dungeon (has `Dungeon_Name`, `Dungeon_ParentDungeon` …) | `Dungeon_Music` | 1,303 | 333 non-zero | 327 of 333 |
| Landmark (has `Name`, `Landmark_Music`) | `Landmark_Music` | 2,731 (2,409 zero) | 283 non-zero | 275 of 283 |

Every landblock but one names an area through the block-map key `Area_DID`
(`landblock.md` §5.5); all 615 areas named that way carry a music type. The script library
(WLib, DID 0x56000000) references `Area_Music`, `Dungeon_Music` and `Landmark_Music`, so the
client's game logic sets `Area_Music` from them; that bytecode was not decoded.

> **Decision (Tessera, offline):** set the player's `Area_Music` to, in order of
> precedence: the current dungeon's `Dungeon_Music` when the player is inside a dungeon
> cell whose dungeon has a non-zero value; else the `Landmark_Music` of a landmark the
> player is at (none in the walker); else the `Ambient_MusicType` of the area named by the
> current landblock's `Area_DID`. Re-evaluate on every landblock or cell change.
> **OPEN:** the original's precedence and whether `Area_Music` changes at landblock
> granularity or by area boundaries finer than a landblock.

<a id="s10-3-2"></a>
#### 10.3.2 Block-map keys (Confirmed, M5; key numbers from `landblock.md` §5.5)

Census over all 390,150 block maps (65,025 per region: 1, 2, 3, 4, 5 and 14). "Set" means a
value other than the palette's "None" entry; the palettes are those of the block-map
descriptors (`landblock.md` §5.4).

| Property | Region 1 (descriptor 0x18000000) | Regions 2, 14 (0x18000014) | Regions 3 (0x18000015), 4, 5 (0x1800001A) |
|---|---|---|---|
| `Ambient_MusicType` | palette of 67 values; set in 739 landblocks (20 distinct values) | same palette; none set | palette holds only "None" |
| `Ambient_MusicRegion` | palette of 15 values; set in 13,379 landblocks (14 distinct values) | same palette; none set | palette holds only "None"; 6 out-of-range indices in each of the three regions |
| `Ambient_SoundEnum_BlockMapOverride` | palette of 4 values; set in 3 landblocks (1 value); 759 out-of-range indices | same palette; none set | palette of 2 values; none set |

- The block-map `Ambient_MusicType` is a **local override**, not the area music: it never
  equals the landblock's area music type, and only 2 of its 20 values are case values of
  the graph's `Ambient_MusicType` Switches (41 case values over the three records of
  §10.2). The monitor volumes' 9 values add 2 more case values.
- Cells (interiors) carry `Ambient_SoundEnum_BlockMapOverride` and `Ambient_SoundEnum` in
  their property sets (`landblock.md` §4; `collision.md` §5.7 counts 521 cells with
  `Ambient_SoundEnum`). No Switch in the music tables watches `Ambient_SoundEnum`.

> **OPEN:** Where the other 37 case values of the `Ambient_MusicType` Switches come from.
> No stored block map, cell or monitor volume holds them; presumably game logic sets them.
> Tessera leaves those cases unreached (Decision).

> **OPEN:** Who reads `Ambient_MusicRegion` (set in 13,379 landblocks) and the cells'
> `Ambient_SoundEnum`. No Switch in the music tables watches either. Tessera does not feed
> them to the player (Decision).

> **Decision (Tessera):** the player's `Ambient_MusicType`, `Ambient_SoundEnum_BlockMapOverride`
> and `Environment_LocalSky` take the current landblock's block-map values (out-of-range
> indices give no value, `landblock.md` §5.4); inside a cell, the cell's own values replace
> them; a monitor volume containing the player replaces both (§10.3.4).

> **OPEN:** (clean-room implementation, 2026-10-07) The Decision does not say whether
> "the cell's own values replace them" replaces the three block-map values as a whole or
> only those the cell sets, nor the same for a monitor. `tessera-client`
> (`apps/client/src/audio/props.rs`) replaces them as a whole inside a cell (a property the
> cell lacks has no value) and per property for a monitor (only the properties its set
> carries). The offline walker has no cell test yet, so the cell rule is exercised by unit
> tests only. Please state the intended rule.

<a id="s10-3-3"></a>
#### 10.3.3 Race-labelled music scripts (Confirmed for the tables walked, M6)

- Table 0x0700042E holds four scripts whose labels in enum 0x23000097 name a playable race:
  keys 0x10000116, 0x100005F7, 0x10000914 and 0x10000B1E. Each is a Collection whose
  intensity is driven by a submix property (`Snd_SubMix_MusicVolume` for the first three,
  `Snd_SubMix_EmoteVolume` for 0x10000B1E) holding a looping Sequence of tracks and
  silences (the pattern of §10.4).
- In script 0x10000039, Run nodes start the first three inside some `Area_Music` cases
  (2, 5 and 9 Run nodes), either directly or below further Switches on
  `Volume_ContentLayer`, `Ambient_MusicType`, `Music_Monitor`, `Music_inMonitor` or
  instance-state booleans. None of the Switches between the case and the Run watches a
  race, a species or the race properties below. No Run node in the tables walked
  (0x0700042E, 0x0700042F, 0x070017F8, 0x070017F9 and scriptlets 0x0C009B74, 0x0C000153)
  starts 0x10000B1E.
- The dictionary defines four float properties, one per race, PIDs 0x1000109D,
  0x10001185, 0x1000138E and 0x100003DD, default 1.0. No node of those tables reads them;
  one SetProperty node in 0x0700042E sets 0x10001185 to 1.0.

An evaluator therefore needs no race logic: the Run nodes start these scripts like any
other, for every player in the area concerned.

> **OPEN:** What the four race probability properties do, and whether anything starts
> script 0x10000B1E. Tessera runs the graph as stored and ignores the properties (Decision).

<a id="s10-3-4"></a>
#### 10.3.4 Monitor volumes (Confirmed data)

332 descriptors (0x47) carry `Entity_Monitors`: an array of `Entity_Monitor` structs (464
monitors) with `Monitor_BoxMin` and `Monitor_BoxMax` (vec3, entity frame, metres; in all
464), `Monitor_Name` (enum, in all), `Monitor_VolumePropertiesToApply` (a 0x78 DID, 383),
`Monitor_Offset` (vec3, 382), `Monitor_ScaleWithEntity` (bool, 349), `Monitor_ClientOnly`
(bool, 274), `Monitor_RejectNotDetectableObjects` (145), `Monitor_IgnoreIndoorVisibility`
(138). The applied 0x78 sets carry, among others, `Environment_LocalSky` (156 monitors),
`Volume_IsLiquid` and the water-depth booleans (145), `Music_inMonitor` (50),
`Ambient_MusicType` and `Ambient_MusicRegion` (25), interior fog (7),
`Ambient_SoundEnum_BlockMapOverride` (3).

> **Inferred:** while the player is inside a placed monitor's box (offset applied, scaled with
> the entity when `Monitor_ScaleWithEntity`), the property set's values apply to the player;
> they are removed when the player leaves.

> **OPEN:** The exact inside test, and the precedence between overlapping monitors and
> against block-map and cell values. That part of the client was not decoded.

> **OPEN:** (clean-room implementation, 2026-10-07) Pending the rule above,
> `tessera-client` transforms the player position into the entity frame (inverse rotation,
> then divided by the entity scale when `Monitor_ScaleWithEntity`), tests it against
> `Monitor_BoxMin` + `Monitor_Offset` … `Monitor_BoxMax` + `Monitor_Offset` with the bounds
> included, takes the monitor list of the first descriptor in the parent chain that has
> one, and among overlapping monitors applies the one with the smallest box.

<a id="s10-3-5"></a>
#### 10.3.5 World state

> **Decision (Tessera, offline walker):** `World_IsNight` = true while the game clock's day
> fraction is ≥ `value_54` or < `value_58` of the game-time record (0.8 and 0.25 in this
> install; the candidate night boundary of `environment.md` §6);
> `Volume_IsOutside` = 0 inside a cell, 1 outdoors; `Volume_ContentLayer` = the default
> content layer (no value: Default ports); `Combat_Violent` = false;
> `Entity_IsThePlayer` = true on the walker's avatar.

> **OPEN:** The original's rule for `World_IsNight` (which clock values and boundaries),
> and the sources of `Volume_IsOutside` and `Volume_ContentLayer`. None of the three is
> stored in data.

> **OPEN:** (clean-room implementation, 2026-10-07) The Decision compares the day fraction
> with the stored f32 `value_54` and `value_58`; `tessera-client` compares at f32
> precision (the f64 time of day rounded to f32), so a stored 0.8 starts the night at 0.8
> exactly. The clock's epoch is the local system clock (`environment.md` §11.1 Decision).

<a id="s10-4"></a>
### 10.4 Music programs (Confirmed census)

The cases of the `Area_Music` Switch follow one pattern (counts for script 0x10000039):

- A **Sequence** that loops forever (1,787 looping sequences; 1,160 single-pass) whose
  steps alternate a **music track** and a **silence**.
- A track is a Collection whose `Script_IntensityMultiplier` is driven by
  `Snd_SubMix_MusicVolume` (1,542 such Collections in table 0x0700042E), holding a Sound
  node or a Random node over several Sound nodes (no repeats). Music Sound nodes are
  tracked (`FireAndForget` 0 in 11,198 of 11,204), on channel 2, non-positional, with
  `RampOutTime` 4 s (5,919), 3 s (2,241), 1 s (1,057), about 2 s (1,820) or 0 (118), and
  intensities 0.5–1.0.
- A silence is a Random node over one-shot Timers. The one-shot Timers of the script last
  15 s (560), 10 s (216), 20 s (171), 30 s (120), 5 s (47), 25 s (26) and a few other
  lengths. Each step finishes when its track or timer finishes, so music and silence
  alternate.
- In the instance (private-encounter) branches, `Snd_Music_Advance` is set to 0 when the
  branch starts and to 1 by switch-transition ports when an instance-stage boolean turns
  true; a Switch on it then plays a one-off track (single-pass Sequence) instead of the
  looping program.
- Switch-case `transition_time` is 0 in all 2,729 cases of this script.

Ambient beds use the same pattern on channel 7: tracked looping sounds (`RampOutTime` 3–4 s)
and fire-and-forget **frills** placed at a random distance around the player (15,156
ambient fire-and-forget nodes with random distances over all records; commonest
(minimum, maximum) pairs (15, 60) 2,313, (20, 60) 1,452, (40, 60) 1,199, (20, 45) 888,
(30, 60) 771; the widest (50, 250)) with pitch spreads, repeated by looping Timers (2–10 s,
some 120 s) whose Random children have probabilities 0.1–0.9.

<a id="s10-5"></a>
### 10.5 Area changes and cross-fades (Confirmed data; fades BIN; Switch rule Reported in `script-table.md` §7.1)

When `Area_Music` changes, the persistent Switch stops the running case and starts the new
one at once (`script-table.md` §7.1). Stopping the old case stops its tracked sounds, each
fading out linearly over its own `RampOutTime` (§6.5); the new case's first step starts
immediately at full gain. So the change is a **cross-fade with a 1–4 s fade-out of the old
track and a hard start of the new one**. The same holds for ambient beds. If the new case
begins with a silence step, the change sounds like a fade to silence.

<a id="s10-6"></a>
### 10.6 Master fades and combat music (Confirmed)

- `Music_Master_Fade_Out` (0x100004F8): a 3 s Timer whose ports set
  `Snd_SubMix_MusicVolume` to 0.95, 0.90 … 0.05 at increasing times (0.03 s … 2.71 s).
  `Music_Master_Fade_In` (0x100003F3): an 8 s Timer setting it from 0.01 (at 0 s) up to 1.0
  (at 7.87 s) in 0.05 steps. `Ambient_Sounds_Master_Fade_Out`/`_In` (0x100029E8,
  0x100029E7) do the same with `Snd_SubMix_Amb_Volume`. Because track Collections take
  their intensity from these properties (§9.3), these scripts fade all running music or
  ambience.
- Scriptlet 0x0C00846F, invoked inside the music cases, implements **combat music** with a
  persistent Switch on `Combat_Violent`:
  - true, and the player's `GOP_DisableCombatMusic` is not true: run
    `Music_Master_Fade_Out`, run `SND_Area_Combat` (0x100005F8), stop
    `Music_Master_Fade_In`;
  - true with combat music disabled, or false: stop `Music_Master_Fade_Out`; if
    `SND_Area_Combat` is running (WhenRunning), stop it and run `Music_Master_Fade_In`.
- Dictionary defaults: `Snd_SubMix_MusicVolume`, `Snd_SubMix_Amb_Volume`,
  `Snd_SubMix_EmoteVolume` 1.0; `Snd_SubMix_Music_AmbVolume` 0.2 and
  `Snd_SubMix_Music_MVVolume` 0.3 are defined but not used by any script.

> **OPEN:** What `Snd_SubMix_Music_AmbVolume` and `Snd_SubMix_Music_MVVolume` are for. No
> script reads them; Tessera ignores them (Decision).

<a id="s10-7"></a>
### 10.7 Ambient and reverb scripts (Confirmed, M7)

Under the music cases, 858 Run nodes (script 0x10000039; 77 of them with `RunScript_Stop`
= 1) start or stop 74 distinct scripts on the player by key (looked up through the avatar's
table closure). The most frequent targets are
the reverb scripts "SND_EAX_*" (CAVE_INT 117, SMALL_INT 72, LARGE_INT 59, GEN_OUTSIDE 56,
FOREST_OUTSIDE 28, COLD_OUTSIDE 20, BOG_OUTSIDE 7 Run nodes), each holding one
SoundEnvironment node (§8), and ambient-bed scripts "SND_Env_*" and dungeon beds of table
0x0700042F (292 scripts), plus the master-fade and combat scripts. A Run with
`RunScript_Stop` = 1 stops a script (its sounds ramp out).

---

<a id="s11"></a>
## 11. Positional emitters (Confirmed)

- At least 972 entity descriptors (0x47) have a Sound node in the `Startup` script (key 5)
  or a listed start-up script defined directly in their class or additional tables
  (scripts inherited through includes were not counted). 240 of them are placed in the
  world by **43,274 SEDs** in 6,174 landblocks (`entity-descriptor.md`).
- By their commonest channel and whether one of their sounds loops: SFX with a loop
  (20,738 SEDs), SFX one-shots only (19,530), ambient one-shots (1,469), ambient with a loop
  (654), combat one-shots (529), default channel with a loop (246), combat with a loop
  (108).
- They include dedicated emitter markers (1,877 SEDs, judged by their asset names) and
  props that also sound (fires, torches, waterfalls).
- An emitter plays when its entity is created (its start-up scripts run, `script-table.md`
  §8.2 Decision) and stops when the entity is destroyed (all its running nodes stop, so its
  tracked sounds ramp out). Their Sound nodes use `MoveWithEntity` and `Offset` as in §9.2,
  and the sound infos' minimum and maximum distances (§6.2).

> **OPEN:** Whether the original creates static emitters for all landblocks in view range
> or only within their sounds' `max_distance`. Tessera: create them with the landblock's
> other static entities; positional one-shots beyond `max_distance` are refused anyway
> (§6.8 item 11).

> **OPEN:** (clean-room implementation, 2026-10-07) The Decision above conflicts with the
> 250-request bound (§6.8 item 3): a looping emitter sound out of range goes to the pending
> list and counts as waiting, so with every emitter of the resident landblocks alive the
> pending list fills and later requests, music included, are refused. `tessera-client`
> therefore scans the landblocks within two landblocks of the viewer and spawns an emitter
> only while the listener is within its *reach* + 30 m (reach: the largest `max_distance`
> of the sound infos its start-up scripts reach, plus the node's random distance and
> offset), despawning it beyond reach + 90 m. Please confirm or state the original's rule.

---

<a id="s12"></a>
## 12. Algorithms in words

<a id="s12-1"></a>
### 12.1 Decoding a 0x2A record

1. Need ≥ 36 bytes. Read the DID, flags, five f32, the payload DID.
2. Read the Pascal string (`records.md` §1.4); reject a length that runs past the end.
3. Require that no bytes remain.
4. Report flags bits other than 0x01, 0x02, 0x04, 0x10, 0x40 as unknown (keep them).

> **OPEN:** (clean-room implementation, 2026-10-06) Step 1 asks for at least 36 bytes, but
> 32 fixed bytes plus a Pascal string need only 33 (an empty name), and the synthetic
> record of `instruments.md` §14.1 is 34 bytes and must decode. The observed minimum of 36
> bytes (§4.1) is a census, not a rule. `tessera-formats` requires only the 32 fixed bytes,
> a complete name and nothing after it. Please confirm or state a minimum.

<a id="s12-2"></a>
### 12.2 Decoding a 0x0A record

1. Need ≥ 8 bytes. Read the DID and *S*; require 8 + *S* + 8 = record length.
2. Read the storage format and the reserved word after the data.
3. Classify the data: `OggS` → Ogg; `RIFF` with `WAVE` at offset 8 → WAV; `RIFF` with `DMAP`
   → DirectMusic (skip); anything else → "unknown container" (none in this install).
4. Ogg: verify page structure (and CRCs when validating); decode Vorbis, skipping
   zero-length packets (§3.3.3); compute *A*, *L* and *M* from the page structure and keep
   decoded samples *A* … *A* + *M* − 1 (§3.3.2).
5. WAV: walk chunks (§3.4); take `fmt ` (PCM tag 1; 16- and 24-bit occur, 8-bit unsigned
   PCM is the general form to accept too; 1–2 channels) and `data`; reject a `data`
   shorter than its declared size or not a multiple of the block align.

<a id="s12-3"></a>
### 12.3 Playing a request

1. Apply the gates of §6.8.
2. Load the sound info and its payload (asynchronously; bound pending loads to 250).
3. Create a voice: volume, pitch (§6.3), loop flag, channel, positional class from the
   flags, min/max distance, priority, positioner.
4. Every frame: update positions relative to the camera (§6.7); positional voices
   recompute the distance factor (§6.2) and gain (§6.1); voices over the limit are
   virtualised or stopped (§6.6).
5. On stop: fade out over the ramp-out time (§6.5).

<a id="s12-4"></a>
### 12.4 Music for the offline walker

1. Create the player avatar entity and run its start-up scripts (`script-table.md` §8.1),
   which include `SND_music_system_startup`.
2. On every landblock or cell change, recompute the player properties of §10.2 by the
   Decisions of §10.3 and set them on the avatar; the persistent Switches react and the
   music and ambience change with the fades of §10.5.
3. Advance Timers, Sequences and Randoms with the game time; Sound nodes behave per §9.2.

---

<a id="s13"></a>
## 13. Invariants (Confirmed on this install unless marked)

| # | Invariant |
|---|---|
| SN1 | 0x0A: record length = *S* + 16; the first word equals the DID; the reserved word is 0. |
| SN2 | 0x0A: the data starts with `OggS`, `RIFF…WAVE` or `RIFF…DMAP`; storage format ∈ {2, 3, 7}; 7 exactly for DMAP. |
| SN3 | Ogg: one logical stream; first page is beginning-of-stream, last page is end-of-stream; every CRC is valid; granules non-decreasing and never −1; no trailing bytes; Vorbis I version 0; 1 or 2 channels. |
| SN3a | Ogg: a Vorbis I decoder started at the first page yields exactly *N* = min(*G*, *T*) samples per channel; *A* + *M* ≤ *T* in every stream (§3.3.2); no first audio page that is not also the last has a granule below *D*(page); zero-length packets occur only as the sole packet of a stream's last page. |
| SN4 | WAV: RIFF size = *S* − 8; PCM; block align = channels × bits/8; the chunk walk ends exactly at *S*. |
| SI1 | 0x2A: 32 fixed bytes + Pascal string, nothing after; the payload DID exists and is audio (never DMAP). |
| SI2 | 0x2A: `unused_a` = `unused_b` = −1.0; 0 ≤ `priority` ≤ 1; `max_distance` > 0. |
| SI3 | 0x2A: only flag bits 0x01, 0x02, 0x04, 0x10, 0x40 occur. |
| ND1 | Every Sound node's data is 43 bytes, every SoundEnvironment node's 8 bytes; neither has ports or targets. |
| ND2 | `SoundScript_Channel` ∈ 0–8; `RampOutTime` ≥ 0; random distances ≥ 0. (The dictionary limits of `Pitch`, `PitchVariance`, `RampOutTime` and the distances are exceeded by some nodes, §9.1; do not reject them.) |
| ND3 | Every non-zero `SoundScript_SoundID` is an existing 0x2A record. |
| MU1 | `Area_Music` is not stored in any property set; every area named by a block map has `Ambient_MusicType`. |

For real-data tests assert invariants, not the counts (counts change with patches).

---

<a id="s14"></a>
## 14. Worked synthetic examples (for tests)

<a id="s14-1"></a>
### 14.1 A sound-info record

Synthetic record 0x2A001234: flags 0x05 (loop, positional), priority 0.5, minimum 3 m,
maximum 40 m, payload 0x0A005678, name `abc` (36 bytes):

| Offset | Bytes | Value |
|---|---|---|
| 0 | `34 12 00 2A` | DID 0x2A001234 |
| 4 | `05 00 00 00` | flags 0x05 |
| 8 | `00 00 80 BF` | −1.0 |
| 12 | `00 00 80 BF` | −1.0 |
| 16 | `00 00 00 3F` | priority 0.5 |
| 20 | `00 00 40 40` | min 3.0 |
| 24 | `00 00 20 42` | max 40.0 |
| 28 | `78 56 00 0A` | payload 0x0A005678 |
| 32 | `03 61 62 63` | name length 3, `abc` |

<a id="s14-2"></a>
### 14.2 A payload record with a two-sample WAV

Synthetic record 0x0A005678 holding a 48-byte mono 16-bit 22,050 Hz WAV with two samples
(+1000, −1000), storage format 3 (record length 64):

| Offset | Bytes | Meaning |
|---|---|---|
| 0 | `78 56 00 0A` | DID |
| 4 | `30 00 00 00` | *S* = 48 |
| 8 | `52 49 46 46 28 00 00 00 57 41 56 45` | `RIFF`, size 40, `WAVE` |
| 20 | `66 6D 74 20 10 00 00 00` | `fmt `, 16 bytes |
| 28 | `01 00 01 00 22 56 00 00 44 AC 00 00 02 00 10 00` | PCM, 1 channel, 22,050 Hz, 44,100 B/s, align 2, 16 bits |
| 44 | `64 61 74 61 04 00 00 00 E8 03 18 FC` | `data`, 4 bytes, samples +1000, −1000 |
| 56 | `03 00 00 00` | storage format 3 |
| 60 | `00 00 00 00` | reserved |

A reader must also accept the same payload with a 1-byte `JUNK` chunk (plus pad byte)
before `fmt `.

<a id="s14-3"></a>
### 14.3 Gain, pitch and placement

- Positional sound (flags 0x04), min 3 m, max 40 m, node volume 0.8, master 1.0, channel
  volume 0.5, listener 21.5 m away: *a* = 1 − (21.5 − 3)/(40 − 3) = 0.5; gain =
  min(0.8, 0.8 × 0.5 × 1.0 × 0.5) = 0.2. At 2 m: min(0.8, 0.8 × 1 × 1 × 0.5) = 0.4. At 40 m:
  0 (a one-shot stops; a loop is virtualised).
- `Pitch` −0.2, `PitchVariance` 0.1: multiplier 0.9 plus a draw in [−0.1, 0.1], so 0.8–1.0.
  `Pitch` −1, variance 0.6: 0.5 + [−0.6, 0.6] → floored to 0.01 when below.
- Node intensity: parent 0.75, own multiplier 0.8, variance 0: 0.6. Own multiplier 0.95
  with variance 0.1 drawn +0.1: clamp(1.05) = 1.0, × parent 0.75 = 0.75.
- `MinRandomDistance` 15, `MaxRandomDistance` 60, `Offset` (0, 0, 2), draws (−1, 0, 0) for
  the direction and 30 for the distance: offset (−30, 0, 2) in the entity frame.
- Ramp-out 4 s from gain 0.6: after 1 s the gain is 0.45, after 4 s the voice stops.

<a id="s14-4"></a>
### 14.4 A synthetic music graph and its timeline

Build, with synthetic DIDs, a start-up script whose root is a persistent Switch on
`Area_Music` with two cases, *A* and *B*. Each case holds a Sequence (loop forever) with
step 1 = a Collection holding one tracked Sound node (channel 2, `FireAndForget` 0,
`RampOutTime` 4, sound info without flags, payload 60 s long) and step 2 = a Random over two
one-shot Timers of 10 s and 20 s (weights 1, 1). Expected behaviour:

| Time | Event | Audible |
|---|---|---|
| 0 s | `Area_Music` = *A*; case *A* starts; its track starts at full gain | track *A* |
| 60 s | track *A* ends; the Sequence moves to step 2; a 10 s or 20 s Timer starts | silence |
| 70 or 80 s | the Timer fires; the Sequence loops to step 1; track *A* starts again | track *A* |
| *t* (during track *A*) | `Area_Music` = *B*; case *A* stops: track *A* fades linearly to 0 over 4 s; case *B* starts at once | track *A* fading, track *B* at full gain |
| *t* + 4 s | track *A* has stopped | track *B* |

Changing `Area_Music` during a silence step starts case *B*'s track at once with nothing to
fade.

<a id="s14-5"></a>
### 14.5 Ogg stream length and played range (§3.3.2–§3.3.3)

A synthetic mono stream whose identification header gives block sizes 256 and 2048. Audio
packets 1–11 all select the short block (256), so packet 1 yields 0 samples and each of
packets 2–11 yields 256/4 + 256/4 = 128: *T* = 1,280. (A long packet after a short one would
yield 64 + 512 = 576.) Page *h* is the page on which the setup header completes, page *a*
the next one.

| Case | Pages | *N* | *A* | *Y*, *P*, *L* | *M*: decoded samples played by the original |
|---|---|---|---|---|---|
| Fresh first audio page | *h*: headers only; *a*: packets 1–5, granule 512; last: packets 6–11, granule 1,200 | 1,200 | 0 | *Y* = 4 × 128 = 512, *P* = 0, *L* = 1,200 | 1,200: samples 0–1,199 |
| Granule offset and empty final packet | *h*: headers only; *a*: packets 1–5, granule 582; next: packets 6–11, granule 1,350; last: one zero-length packet, granule 1,350 | 1,280 | 0 | *Y* = 512, *P* = 70, *L* = 1,280 | 1,280: samples 0–1,279 |
| Audio on the setup page | *h*: setup header and packets 1–4, granule 384; *a*: packets 5–8, granule 896; last: packets 9–11, granule 1,200 | 1,200 | 384 | *Y* = 3 × 128 = 384 (packet 5 not counted), *P* = 512, *L* = 688 | 688: samples 384–1,071 |

A looping sound info repeats exactly the *M* samples of its row, with no gap.

<a id="s14-6"></a>
### 14.6 A placeholder in a Random node (§9.2 step 1, §9.6)

A synthetic script root is a Random node with three Default ports of weight 1. Ports 1 and
2 each hold one fire-and-forget Sound node naming an existing sound info; port 3 holds one
fire-and-forget Sound node with `SoundScript_SoundID` = 0.

| Draw | What happens | Script |
|---|---|---|
| Port 1 or 2 | The Sound node requests its sound and completes at once (fire-and-forget); no child is left running, so the Random completes | finishes at once, sound plays on |
| Port 3 | The Sound node's start fails (null DID); it leaves no running instance; the Random completes at once | finishes at once, silence |

Had port 3 also held a second child after the placeholder (for example a SetProperty
node), that child would still start: a failed start does not stop its siblings.

---

<a id="s15"></a>
## 15. Corrections to other specs

| # | Spec, place | It says | Observed / BIN | Resolution |
|---|---|---|---|---|
| X1 | `records.md` §9.1 | trailer u32 (2, 3, 7) and f32 (0.0) unknown | The u32 is the storage format (§3.2); the second word is read as a u32 and is 0 | This spec §3.1–§3.2 |
| X2 | `records.md` §9.1 OPEN | sound-info records "may live under another prefix" | They are prefix 0x2A, DB type 0x12 (also `property-system.md` §9) | §4 |
| X3 | `records.md` §2.1 | 0x2A "unknown" | Sound info | §2 |
| X4 | `script-table.md` §5.3 / §5.1 | Sound (48) and SoundEnvironment (62): field lists only | Semantics, defaults, ranges, start algorithm | §9 here; `script-table.md` §4.2 "`IntensityMultiplier` scales the node's effect": the exact rule is §9.3 |
| X5 | `ARCHITECTURE.md` §14 (neutral doc) | "block-map properties give area music and ambient sound per landblock" | Area music comes from area, dungeon and landmark objects through `Area_Music`; block-map music keys are sparse overrides; music is played by a data-driven script graph | §10 |
| X6 | `research/notes/data-semantics.md` §9 | sound info: optional name | The name is present in all 45,544 records | §4.1 |
| X7 | `landblock.md` §5.5 | lists the audio keys | Adds: in regions 3, 4, 5 the descriptor palettes of `Ambient_MusicType` and `Ambient_MusicRegion` contain only "None"; usage counts in §10.3.2 | informational |
| X8 | `script-table.md` §7 and §11.3 | no outcome of a node start is defined; nothing says what a parent does when a child cannot start | A failed start leaves no running instance and counts as completed at once; it does not stop the remaining children of the port and is not reported to the parent; a Random node with no child left running completes at once (BIN2) | §9.6 here |

---

<a id="s16"></a>
## 16. Owner observations

None of O1–O17 in `docs/checklists/owner-observations.md` concerns audio, so this spec
answers none of them. Proposed new items (for the maintainer to add; the checklist is not
edited here):

| Proposal | Question | How |
|---|---|---|
| S-1 | Area-border music change | Walk across an area border with music playing; note whether the old track fades over a few seconds while the new one starts at once (§10.5). |
| S-2 | Silence between tracks | Time the gaps between tracks in one area (expect 5–30 s, §10.4). |
| S-3 | Positional falloff | Walk away from a waterfall or fire; note the distance where it becomes silent (its sound info's maximum distance, §6.2). |
| S-4 | Night and indoor variants | Compare the ambient bed at day and night, and inside a building (§10.3.5). |
| S-5 | Ogg played range | Record the client's output while a short one-shot sound whose payload has audio on its setup page plays (the maintainer can name a sound info from §3.3.2's census); compare its length with *M* and with *N* (§3.3.2). |

---

<a id="s17"></a>
## 17. Open questions (summary)

1. The reserved word of 0x0A records (§3.1).
2. Whether the "DirectMusic" sound provider can still be selected (§3.5).
3. Whether the shipped Vorbis library files behave like the released 1.0.1 source (§3.3.2).
4. The effect of voice mode 4 (flag 0x02) and the meaning of flag 0x10 (§4.3).
5. Which of the 4,998 sound infos no node or property names are used, and by what (§5).
6. Channel, positioning and timing of the weather sounds of the UNIQUEDB DID map and of
   other sounds started by client code (§5).
7. A `Pitch` multiplier ≤ 0 (§6.3).
8. How a tracked request skipped by the voice limit completes; the exact voice-stealing
   pass (§6.6).
9. What fills the sound-info filter, the suppression table and the optional check hook of
   the request gates (§6.8).
10. Channel flags 0x4 and 0x8; "MusicDisabled", "PersistentSound", "FocusBehavior" (§7.2).
11. How a reverb environment's intensity is applied (§8).
12. Which state sets the "no effects" bit; fire-and-forget loops (§9.2).
13. How the original sets `Area_Music` (precedence, granularity) (§10.3.1).
14. Sources of the `Ambient_MusicType` case values not found in stored data; who reads
    `Ambient_MusicRegion` and `Ambient_SoundEnum` (§10.3.2).
15. The race probability properties and script 0x10000B1E (§10.3.3).
16. Monitor-volume inside test and precedence (§10.3.4).
17. `World_IsNight` rule; `Volume_IsOutside` and `Volume_ContentLayer` sources (§10.3.5).
18. `Snd_SubMix_Music_AmbVolume` and `Snd_SubMix_Music_MVVolume` (§10.6).
19. Emitter creation range (§11).

Resolved in the revision of 2026-10-05: `smpl` loop points (§3.4: not used, BIN2); the
outcome of a failed node start (§9.6, BIN2); the length of the 15 streams formerly read as
truncated (§3.3.2–§3.3.3).

---

<a id="s18"></a>
## 18. Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version (Review): 0x0A and 0x2A layouts with censuses over all records; 100% decode validation; playback, channel, reverb and sound-node rules from BIN (build 4906.0071.1794.4035); music and ambience as data-driven graphs with property sources; positional emitters; corrections X1–X7. |
| 2026-10-05 | Revision after review (changes requested), status stays Review. Header: the client-internal class name quoted in the review record redacted. §1: new tags BIN2 and REFDEC, new rows A7, A8, A9, I4, P7, N3, N4, M5, M6, M7; A4 rewritten. §3.1, §3.5, §4.3, §5, §6.3, §6.6, §6.8, §9.2, §10.3.2, §10.3.5, §10.6: every inline unknown turned into a `> **OPEN:**` callout with a Decision. §3.3.2–§3.3.3 rewritten: the Vorbis I length *N* = min(*G*, *T*) with a granule census over all 40,874 streams ("every stream starts at granule 0" replaced by the checked properties), and one normative played range, the original's (skipped lead *A*, length *L*, *M* = min(*L*, *T* − *A*)), derived from how the client drives its shipped Vorbis 1.0.1 libraries (BIN2) and confirmed over all streams with the released 1.0.1 source; the 15 "truncated" streams in fact end with a zero-length packet and run 63–84 samples ahead in their granules, so nothing is zero-filled and no packet is dropped. §3.4: chunk counts corrected (`ovwf`, `elmo` in 3 files each; first-chunk split), `smpl` facts corrected (5 files, 2 with a loop, one of them behind a looping sound info) and the OPEN resolved (loop points not used, BIN2). §4.3: loop-flag median length corrected to 62.6 s (64.2 s over distinct payloads). §6.8: renumbered in check order and completed from BIN2 (filter, 250-request bound, suppression table, DID range, voice limit, gate codes, exclusive-channel queueing). §9.2 steps 1–4 made precise (BIN2); new §9.6 (outcome of a node start) and correction X8 for `script-table.md`. §10.3.2: per-region census with palettes and out-of-range indices. §10.3.3 rewritten (race-labelled scripts, no race logic needed). §10.3.4: client-internal class name removed. §10.7: Run-node census completed. §12.2, §13 (SN3a), new §14.5–§14.6, §17 updated. |
| 2026-10-06 | §9.3: one-line cross-reference to `script-runtime.md` §4 added; no fact changed here (the correction is recorded there as X1). |
