<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Player music: the ABC dialect of the original client

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), 2026-10-05: changes requested (§9.5 OPEN source, §13.2 operation-order claim, §4 digit-run limit; minor: MA trial labels vs §12 row IDs, §9.4 packed word). Revised 2026-10-05 by the spec writer (dirty room) to address every item; see the change log. Claude review session (spec-writer agent), re-review 2026-10-05: **accepted** — every §5.3 item passes; repairs spot-checked against the install and the executable (enum mappers 0x2300015E / 0x230000AF, the owner's key-mapping file, the §4.1 number branch, the §8.2 operation order and the §13.2 order table, the §9.4 packing, the §5.4 defaults). |
| **Implements into** | `tessera-audio::abc` (`crates/audio`, M6): file lookup, text decoding, tokenizer, part selection, field and music reader, timing, note-event list, error reporting; `apps/client` (`/play`-style command, §11); `fuzz/` (one target for the reader: arbitrary bytes in, events or an error out); a conformance test suite built from §12 |
| **Companion specs** | [`install-layout.md`](install-layout.md) §8.1 (settings folder that holds `Music/`). [`string-table.md`](string-table.md) §7, §9 (rendering a table entry with arguments, named tables). [`records.md`](records.md) §1.5 (name hash), §8 (DID maps). `instruments.md` and `sound.md` (not yet written: which sample each pitch slot plays, octave per instrument, volume curve, release). |

---

<a id="s0"></a>
## 0. Purpose

Players of the original game make music by typing a command that names a text file in ABC
notation. The client reads one **part** of that file, checks it, turns it into a list of
timed note events and plays them on the character's instrument. The client accepts its own
**dialect** of ABC: it reads a subset of ABC 2.1, ignores some constructs, stops reading at
some characters, refuses many others, and computes durations with one exact formula and
two hard limits.

This spec gives an implementer everything needed to build a reader that accepts, ignores,
truncates and refuses exactly the same files, and produces the same notes at the same times:

- where the file is found and how its bytes become characters (§3);
- the tokens (§4), lines, fields and part selection of multi-part files (§5);
- the field values M, L, Q, K, T, X (§6);
- notes, rests, accidentals, lengths, ties, chords, broken rhythm, tuplets, prefixes,
  volume marks and bar lines (§7);
- the timing rule, the 60 ms and 8 s limits and the time line (§8);
- the note-event list handed to the sound system (§9);
- the error model and the client's own message entries in the user's string tables (§10);
- the commands around the reader, as far as they concern it (§11);
- a conformance table of original tunes with expected results (§12), worked examples (§13),
  invariants and hostile-input notes (§14), differences from ABC 2.1 and from published
  models (§15), corrections (§16) and open questions (§17).

Out of scope: which sound sample a pitch plays on which instrument, octave shifts per
instrument, the loudness curve of the volume values, note release and sustain
(`instruments.md`, `sound.md`); how a performance reaches other players (network,
decision 0004); the live-play key bindings (keymap).

All values quoted from the user's string tables and DID maps are observations of one
install and must be read at run time. Numbers that define the reader's behaviour (limits,
factors, tables in §6–§9) come from the client executable and may be used as constants
([decision 0012](../decisions/0012-numeric-constants.md)).

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| BIN | The client executable of the user's install, `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, compiled 2026-09-27, changelist 711794), disassembled with capstone in the dirty room on 2026-10-05 under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md). Only the player-music reader was read (about 33 KB of code: file lookup, byte-order-mark test, character decoder, tokenizer, field and music parsers, sequencer, event ordering), plus the generic text-file opener and character decoder it calls, a few generic string and path helpers, and the script-native registrations that lead to it. **Questions:** (a) which ABC constructs the client accepts, ignores, stops at or refuses, and with which message; (b) the exact timing rule (formula, operation order, precision, limits); (c) the semantics of chords, pitch range, accidentals, keys, ties, tuplets and broken rhythm; (d) how one part of a multi-part file is chosen; (e) file lookup and text decoding. **Why M1–M3 were not enough:** ABC files are user files, not game data, so the archives cannot show the rules (M1 gives only the message table, §10); published tools model the client from in-game trials and are incomplete or contradictory in places (M2, see §15.2); in-game trials (M3) give one accept/refuse outcome per file and cannot show the operation order behind the 60 ms rounding or the many edge rules, and the owner's settings folder has no `Music` folder. **Not examined:** how music reaches other players, the game-script code of the chat commands, the audio engine; nothing about network, login, keys, launcher or anti-tamper. **Revision (same build, same day), further questions:** (f) how long a run of digits may be and which characters count as digits (the tokenizer's number branch, near 0x140BBECA4); (g) how slot and volume are packed into one word and which part §9.3 compares (the sequencer's event builder and the re-attack pass); (h) what the 0.5 s constant next to the 0.7 s start code is used for (identified only far enough to point to the right spec, §8.5). **Why M1–M3 were not enough:** (f) and (g) are internal to the reader and leave no trace in the data; an in-game trial of (f) would risk crashing the owner's client. |
| OBS | Observed in the user's install on 2026-10-05 by throwaway probes in the dirty-room scratchpad: string table 0x2500808A in `client_local_English.dat` (token values and argument IDs only, no text), the named-string-table DID map 0x28000004 in `client_general.dat`, enum mapper 0x230000AF (the InputActions mapper of [`keymap.md`](keymap.md); its 37 live-play note actions), enum mapper 0x2300015E (the note mapper of [`instruments.md`](instruments.md) §3.2; all 38 labels and keys), and the owner's settings folder (`lotro.keymap`, which binds 21 of the 37 note actions; `UserPreferences.ini`; no `Music` folder exists). |
| MA | Maestro / ABC Player, `github.com/NikolaiVChr/maestro` (MIT), clone at commit 0440b25, studied for facts only: the in-game trial notes recorded in its ABC reader and its behaviour tests, which it numbers B1–B81 (cited here as **MA B1** … **MA B81**, to keep them apart from the row IDs of §12), and its table of 164 in-game observations of the 60 ms limit. No code, test or prose copied; no tune from its suite is reused here. |
| WEB | Published usage guides for the command syntax (`/play <file> <n> sync`, `/playstart`, `Music` folder, sub-folders), read 2026-10-05: the Maestro project wiki page "Playback"; search-engine summaries of community guides. |
| ER | `research/notes/engine-client-re.md` §7 (earlier summary of the dialect from MA). |
| MODEL | The expected results in §12 and §13 were computed from the rules of this spec by a throwaway model in the dirty-room scratchpad; the model is not a source of facts. |

Marks: **Confirmed (BIN)** = read from the executable; **Confirmed (BIN + OBS)** = read from
the executable and agreeing with the user's data; **Confirmed (BIN + MA)** = read from the
executable and agreeing with published in-game observations; **Reported** = published
material only; **Probable** = BIN evidence that was not traced to the end.

| # | Fact | Sources | Mark |
|---|---|---|---|
| PV1 | Files live in `<settings folder>/Music`; lookup tries extension `.abc`, then `.txt` (§3.1) | BIN; WEB; ER | Confirmed (BIN) |
| PV2 | Byte-order-mark test: UTF-8 BOM accepted, UTF-16 BOMs refused, other BOMs treated as a missing file (§3.2) | BIN | Confirmed (BIN) |
| PV3 | Character decoding, comments, backslash, line breaks (§3.3–§3.5) | BIN | Confirmed (BIN); bytes ≥ 0x80 without BOM OPEN |
| PV4 | Token set and combination rules (§4) | BIN | Confirmed (BIN) |
| PV4b | Number tokens: digits copied into a 128-byte buffer without a length check (runs of 128 or more characters are undefined in the original); low byte of each character kept; value by decimal conversion with saturation at 2,147,483,647 (§4.1) | BIN | Confirmed (BIN); which non-ASCII characters count as digits OPEN |
| PV5 | Field lines: letter + colon, case-insensitive, allowed anywhere music may start; K L M Q T X recognised, all other letters ignored (§5.1–§5.2) | BIN; MA (ignored fields, blank lines) | Confirmed (BIN + MA) |
| PV6 | Part selection by X: and stop at the next X: (§5.3) | BIN; MA (empty X: refuses) | Confirmed (BIN + MA) |
| PV7 | M:, L:, Q: forms; M: resets the unit length; Q: beat ignored (§6.1–§6.3) | BIN; MA (Q: beat = meter denominator; M:none, text before the tempo refused) | Confirmed (BIN + MA) |
| PV8 | K: tonic, modes, three-letter rule, key-signature table, explicit accidentals (§6.4) | BIN; MA (empty K:, clef words, `exp` refused) | Confirmed (BIN); some MA reports contradicted (§15.2) |
| PV9 | T: title rules and notice length (§6.5) | BIN | Confirmed (BIN) |
| PV10 | Pitch numbering, letters, octave marks, range 48–84 (§7.3) | BIN; MA (range C, to c') | Confirmed (BIN + MA) |
| PV11 | Accidental scope: same letter and octave, to the next bar token; `\|1` `:\|2` do not clear (§7.3) | BIN; MA (own octave, bar and `:\|` clear) | Confirmed (BIN + MA); ending tokens BIN |
| PV12 | Length syntax including the `c3/` quirk (§7.5) | BIN | Confirmed (BIN) |
| PV13 | Ties join the next note of the same pitch anywhere later; unfinished tie refuses (§7.6, §9.1) | BIN; MA (tie over other notes and rests) | Confirmed (BIN + MA) |
| PV14 | Chords: duplicates dropped, rests count once, at most 6, nothing after `]`, length = shortest member (§7.7) | BIN; MA (MA B6, MA B11, MA B31, MA B65, MA B72, MA B73) | Confirmed (BIN + MA) |
| PV15 | Broken rhythm: 1–3 symbols, not after a chord, factor to the next note (§7.8) | BIN; MA (MA B5, MA B6, MA B78c) | Confirmed (BIN + MA) |
| PV16 | Tuplets: p, q, r in 2–9, defaults, adjacency rule (§7.9) | BIN | Confirmed (BIN) |
| PV17 | Prefixes: quoted text, grace group, one of `~ . u v` (§7.10) | BIN; MA (chord symbols and grace notes ignored) | Confirmed (BIN); `u v` contradicts MA (§15.2) |
| PV18 | Volume marks and their values (§7.11) | BIN; MA (marks work, capitals work, MA B74) | Confirmed (BIN + MA) |
| PV19 | Bars and endings: no repeats played; refused forms (§7.2) | BIN; MA (MA B2, MA B4, MA B18, MA B28, MA B77b) | Confirmed (BIN + MA) |
| PV20 | Illegal characters end the music silently (§7.1) | BIN; MA (`$`, `` ` ``, `!…!`, typographic quotes) | Confirmed (BIN + MA) |
| PV21 | At most 10,000 top-level events per part (§7.12) | BIN; MA | Confirmed (BIN + MA) |
| PV22 | Timing values are part-wide: the last M:/L:/Q: read applies to every note (§8.1) | BIN | Confirmed (BIN); contradicts an MA report (§15.2) |
| PV23 | Duration formula, double precision, limits 0.06 s and 8.0 s (§8.2–§8.3) | BIN; MA (164 of 164 in-game observations reproduced exactly) | Confirmed (BIN + MA) |
| PV23b | Operation order of the duration formula (§8.2) | BIN; MA rules out most other orders but not all (§13.2) | Confirmed (BIN) |
| PV24 | Time line, chord advance, empty chord −1 s, start 0.7 s after a solo command (§8.4–§8.5) | BIN | Confirmed (BIN) |
| PV25 | Event generation, ordering tolerance 0.0001 s, re-attack handling, packed slot/volume word, pitch-slot table (§9) | BIN | Confirmed (BIN) |
| PV25b | Every pitch slot is the key of the note label `Note_<name><octave>` in enum mapper 0x2300015E, with pitch 48 ↔ `Note_C2` … pitch 84 ↔ `Note_C5`; the 37 live-play note actions of enum mapper 0x230000AF carry the same 37 name suffixes (§9.5) | BIN (fixed table); OBS (all 37 keys of 0x2300015E; all 37 `MUSIC_` names of 0x230000AF) | Confirmed (BIN + OBS) |
| PV26 | Error kinds and message entries: named-table key 0x10000069, token values, argument IDs (§10) | BIN; OBS (all 25 error tokens of §10.2 and the 3 notice tokens of §10.4 present in table 0x2500808A with the listed argument IDs; key 0x10000069 resolves to it) | Confirmed (BIN + OBS) |
| PV27 | Command syntax `/play <file> [part] [sync]`, `/playstart` (§11) | WEB; ER; BIN (reader entry receives name, part, sync flag) | Reported (syntax); Confirmed (BIN) (reader inputs) |

---

<a id="s2"></a>
## 2. Overview

<a id="s2-1"></a>
### 2.1 Pipeline

1. The command gives a **file name**, an optional **part number** and a **sync** flag (§11).
2. The file is found in the `Music` folder (§3.1) and opened; its first bytes decide the
   text encoding or refuse the file (§3.2).
3. Characters are read one by one, with comments and backslashes removed on the fly (§3.3–§3.4),
   and grouped into **tokens** (§4).
4. The reader goes through the file line by line. Each line is a **field line**
   (letter + colon, §5.1), a blank line, or a **music line** made of **elements** (§7). Part
   selection by X: decides which lines belong to the requested part (§5.3).
5. Fields set the **song values** (meter, unit length, tempo, key, title; §6). Music elements
   add **events** to the song: single notes, rests and chords (§7).
6. After the part has been read, the **sequencer** computes every duration with the part-wide
   meter denominator, unit length and tempo, checks the limits, and builds the **note-event
   list** (§8, §9).
7. A solo performance starts 0.7 s later; a synced one waits for the start command (§8.5).

<a id="s2-2"></a>
### 2.2 Three outcomes

| Outcome | Cause | Effect |
|---|---|---|
| **Refused** | Any error of §10.2 during reading or sequencing | One message (the first error only); **nothing of the part plays** |
| **Truncated** | An illegal character (§4) where a music element should start (§7.1 gives the one exception), or an unclosed quote or grace group after other music | Reading **stops** there without a message; everything read before plays; nothing after it is read, not even later lines |
| **Ignored** | Unrecognised fields, comments, chord symbols, grace groups, slurs, some decorations, ending markers | Skipped without effect |

---

<a id="s3"></a>
## 3. Finding and reading the file

<a id="s3-1"></a>
### 3.1 Location and lookup (Confirmed (BIN))

1. The music folder is the folder `Music` inside the settings folder of
   [`install-layout.md`](install-layout.md) §8.1.
2. The name from the command is joined to that folder as a path. Sub-folder components in
   the name are therefore allowed (WEB reports players keeping songs in sub-folders).
3. The extension `.abc` is set on that path. If no file exists there, the extension `.txt` is
   set instead. Existence is a plain "does this path exist" test.
4. If neither exists, the part is refused with **file not found** (§10.2); its argument is the
   full path last tried (the one ending in `.txt`).

> **OPEN:** whether a name that already ends in an extension (for example `song.abc` or
> `song.mid`) keeps it or has it replaced. The client calls a set-extension helper on the
> joined path; the helper was not traced. A reader that **replaces** the last extension
> matches the observed two-step lookup.

> **OPEN (implementation, 2026-10-07):** whether the original lets a name leave the `Music`
> folder (an absolute name, or `..` components). Tessera (`tessera_audio::abc::find_song`)
> keeps lookups inside the folder: such names, and the empty name, are **file not found**.

> **OPEN (implementation, 2026-10-07):** letter case on a case-sensitive file system. The
> original runs on a file system that ignores letter case, and §11 lists `.ABC`/`.TXT` files
> too, so `/play Tune` finds `Tune.ABC` there. Tessera's `find_song` tries the exact path for
> each extension, then the first entry of that folder (in sort order) whose name equals it
> ignoring ASCII letter case, keeping the `.abc`-then-`.txt` order; only the last component
> is matched this way. Which file the original picks when several spellings exist was not
> traced. Tessera's listing gives a stem that itself contains a `.` as the whole file name,
> so that replacing the last extension finds the listed file again.

<a id="s3-2"></a>
### 3.2 Opening and the byte-order mark (Confirmed (BIN))

The file is opened read-only in the C runtime's text mode. The first bytes are examined:

| First bytes (hex) | Meaning | Result |
|---|---|---|
| `FE FF` or `FF FE` (this includes `FF FE 00 00`) | UTF-16 (or UTF-32 LE) BOM | Refused: **encoding not supported**, argument = the file's path |
| `EF BB BF` | UTF-8 BOM | Accepted; reading continues after these 3 bytes with the UTF-8 decoder (§3.3) |
| `0E FE FF`, `2B 2F 76`, `FB EE 28` | SCSU, UTF-7, BOCU-1 BOMs | Treated as a missing file: **file not found** |
| `00 00 FE FF`, `00 00 FF FE`, `DD 73 66 73` | UTF-32 BE and UTF-EBCDIC marks | Treated as a missing file: **file not found** |
| anything else, including files shorter than 4 bytes | no BOM | Accepted; reading restarts at byte 0 with the plain decoder (§3.3) |

An empty file is accepted and yields a part with no notes (it "plays" silence).

<a id="s3-3"></a>
### 3.3 Characters

**Plain files (no BOM)** (Confirmed (BIN) for ASCII): each byte is read as one character
through the C runtime's wide-character read in text mode. Bytes 0x00–0x7F are ASCII.

> **OPEN:** the character produced for bytes 0x80–0xFF depends on the C runtime locale of
> the original (the identity mapping to U+0080–U+00FF is expected for the default C locale,
> not verified). These bytes matter only inside titles, comments and skipped text; in a
> music line they cause an error or end the music (§7.1). Whether a 0x1A byte ends text-mode
> input in the original (the Windows C runtime convention) is also OPEN.

**UTF-8 files (with BOM)** (Confirmed (BIN)): one character per sequence:

- a byte below 0x80 is that character;
- a lead byte 0xC0–0xDF starts a 2-byte sequence;
- a lead byte 0xE0–0xFF starts a **3-byte** sequence (4-byte sequences are not supported);
- a sequence is converted strictly; if the conversion fails (bad continuation byte, overlong
  form, any 4-byte character such as an emoji), the result is character 0;
- a continuation byte 0x80–0xBF that does not follow a lead byte becomes the 16-bit code
  0xFF00 + byte.

> **OPEN:** how the C runtime classifies the codes 0xFF80–0xFFBF (letter or not). This only
> decides between "colon expected" and "music ends" for such a byte in a music line.

**Character 0** — a NUL byte, or a failed UTF-8 sequence — ends the input as if the file ended
there (Confirmed (BIN)).

**Line breaks** (Confirmed (BIN)): CR and LF each end a line. Because the file is read in
text mode, CR LF arrives as a single line break. A lone CR is a line break.

<a id="s3-4"></a>
### 3.4 Comments and backslashes (Confirmed (BIN))

These rules apply to **every** character read, everywhere in the file, including field values,
titles, quoted text and grace groups:

1. `%` starts a comment. Every character up to the next line break (or the end) is dropped;
   the line break itself is kept.
2. `\` is dropped and the character after it is read in its place, under the same rules.
   There is **no escape**: `\%` still starts a comment, and `\` before a line break leaves the
   line break in place (line continuation has no effect). `\"` yields a plain quote.

<a id="s3-5"></a>
### 3.5 Line and column numbers (Confirmed (BIN))

Lines are numbered from 1; columns from 1 at the start of each line. Every character read
advances the column by one, except line breaks, comment characters and backslashes. A token's
position is the line and column of its first character. These numbers are the arguments of
most error messages (§10).

---

<a id="s4"></a>
## 4. Tokens (Confirmed (BIN))

The tokenizer turns characters into tokens. Classification is tested in this order:
end of input, line break, white space, letter, digit, then the punctuation of the table.

| Token | Written as | Notes |
|---|---|---|
| end | end of input or character 0 | |
| line break | CR or LF | starts a new line |
| space | a run of white-space characters other than CR and LF | the whole run is one token; white space as classified by the C runtime's wide-character test (at least space, tab, VT, FF) |
| letter | one alphabetic character | **one token per letter**: `Am` is two tokens; non-ASCII letters are letters too |
| number | a run of digit characters | value as in §4.1: the digits' value, at most 2,147,483,647; runs of 128 or more characters are a special case |
| bar | `\|` | when not one of the combinations below |
| double bar | `\|\|` | |
| thin-thick bar | `\|]` | |
| repeat start | `\|:` | |
| first ending | `\|1` | |
| repeat end | `:\|` | when not followed by `2` |
| second ending | `:\|2` | |
| double repeat | `::` | |
| thick-thin bar | `[\|` | |
| ending one | `[1` | |
| ending two | `[2` | |
| colon | `:` | when not one of the combinations above |
| chord open | `[` | when not one of the combinations above |
| chord close | `]` | |
| open | `(` | slur or tuplet |
| close | `)` | |
| grace open, grace close | `{`, `}` | |
| quote | `"` | |
| apostrophe | `'` | |
| comma | `,` | |
| plus | `+` | |
| hash | `#` | |
| tilde | `~` | |
| dot | `.` | |
| caret | `^` | |
| underscore | `_` | |
| equals | `=` | |
| slash | `/` | |
| hyphen | `-` | |
| less | `<` | |
| greater | `>` | |
| **illegal** | every other character: `!` `$` `&` `*` `;` `?` `@` `` ` ``, control characters, and every non-ASCII character that is not a letter, digit or space (for example typographic quotes and dashes) | `%` and `\` never reach the tokenizer (§3.4) |

Combinations are decided by the single character that follows:

- after `|`: `|` gives double bar, `]` thin-thick bar, `:` repeat start, `1` first ending;
  anything else leaves a plain bar (and the character is read again as the next token);
- after `[`: `|` gives thick-thin bar, `1` ending one, `2` ending two; anything else leaves a
  chord open;
- after `:`: `:` gives double repeat; `|` gives repeat end, or second ending when a `2`
  follows the `|`; anything else leaves a colon.

So `|2` is a bar followed by the number 2, `:|1` is a repeat end followed by the number 1,
`[3` is a chord open followed by the number 3, `||1` is a double bar followed by the number 1,
and `:|:` is a repeat end followed by a colon. §7 says what each of these does.

> **OPEN (implementation, 2026-10-07):** which non-ASCII characters the C runtime's
> wide-character tests count as white space and as letters. Tessera uses Unicode's
> White_Space property for non-ASCII white space and Unicode's Alphabetic property for
> non-ASCII letters (so U+00A0 is white space, `é` a letter, `§` illegal).

<a id="s4-1"></a>
### 4.1 Number tokens (Confirmed (BIN))

A number token is read as follows in the original (client build 4906.0071.1794.4035; the
tokenizer's number branch near address 0x140BBECA4):

1. **Which characters are digits.** A character starts and continues a run when the C
   runtime's wide-character digit test accepts it. For the ASCII digits `0`–`9` this is
   certain; for other characters see the OPEN below. The run ends at the first character that
   is not a digit (that character is read again as the next token) or at the end of input.
   Line breaks, comments and backslashes are handled by §3.4 as everywhere else, so `1\2` is
   the single run `12` and `1%x⏎2` is the run `1` followed by a line break.
2. **Buffer.** Each character of the run is stored as **one byte, its low 8 bits**, into a
   fixed buffer of **128 bytes** on the stack, followed by a terminating zero byte. There is
   **no length check**.
3. **Value.** The stored bytes are converted with the C runtime's decimal string-to-long
   conversion (the C standard's `strtol` semantics with base 10, 32-bit `long`): optional
   leading white space and one sign are skipped, then ASCII digits are read up to the first
   byte that is not one; no digits gives 0; a value above 2,147,483,647 gives 2,147,483,647.
   For a run made only of ASCII digits the value is therefore the decimal value of the run,
   saturated at 2,147,483,647.
4. **Position.** The token's position is that of its first character; every character of
   the run advances the column by one (§3.5).

**Length limit.** A run of at most **127** characters fits the buffer together with its
terminating byte. A run of **128 or more** characters writes past the buffer: with 128
characters the terminating byte lands just past its end, longer runs put digit bytes there,
overwriting registers the function saved for its caller, and from 168 characters on the
function's return address. The function has no stack guard. The original's behaviour for such runs is **undefined** (most probably a crash of the
client when the tokenizer returns, possibly silent corruption of the caller's state for
runs just over the limit). No file that the original plays normally contains such a run.

**Tessera's choice** (no original behaviour to match): Tessera reads a run of any length
without a fixed buffer and gives it the value rule of step 3 applied to the whole run (so a
long run of ASCII digits saturates at 2,147,483,647, and leading zeros do not count towards
anything). It does not refuse the part for the length alone. A reader must still bound its
memory (§14.2): it need not keep more than the first non-zero digits up to the point of
saturation.

> **OPEN:** which non-ASCII characters the original's digit test accepts. The test is the
> Windows C runtime's wide-character digit classification, which is not observable here.
> Candidates are the superscript digits U+00B2, U+00B3, U+00B9 (reachable from a file with a
> UTF-8 BOM, and from bytes 0xB2, 0xB3, 0xB9 of a file without one if those map to U+00B2,
> U+00B3, U+00B9, §3.3) and the decimal digits of other scripts and the full-width digits
> U+FF10–U+FF19 (reachable only through UTF-8 with a BOM). If such a character is accepted, it
> joins the run and only its low byte enters the buffer, so it does **not** contribute its
> digit value: a full-width digit stores a control byte (0x10–0x19) and ends the conversion
> there; an Arabic-Indic digit (U+0660–U+0669) stores one of the bytes 0x60–0x69, which are
> not ASCII digits. For example, if U+00B2 is a digit, the run `1²` is one number token of
> value 1, and a run that starts with such a character usually has value 0 (unless its low
> byte is white space or a sign, which the conversion skips). A character that the letter test
> accepts is a letter before the digit test is made (§4 order); a character that neither test
> accepts and that is not white space is illegal.
> Until an owner check settles it, Tessera accepts **only** the ASCII digits `0`–`9` as digits;
> every other character keeps the classification of §4 (letter if alphabetic, otherwise
> illegal).

---

<a id="s5"></a>
## 5. Lines, fields and parts

<a id="s5-1"></a>
### 5.1 Field lines (Confirmed (BIN))

A **field** is a letter token immediately followed by a colon token (no space between).
The letter is case-insensitive: `k:` is the same field as `K:`. After the colon, one space
token is skipped (so any amount of white space may follow the colon). The field's reader
(§6) takes what it needs; then **every token up to and including the next line break is
discarded**. Text after a complete field value is therefore ignored.

| Letter | Field | Section |
|---|---|---|
| K | key | §6.4 |
| L | unit note length | §6.2 |
| M | meter | §6.1 |
| Q | tempo | §6.3 |
| T | title | §6.5 |
| X | part number | §5.3 |
| any other letter, either case | ignored: the whole line is skipped | |

The ignored fields include lyrics (`w:`, `W:`), voices (`V:`), part order (`P:`), information
fields (`A:` `B:` `C:` `D:` `F:` `G:` `H:` `N:` `O:` `R:` `S:` `Z:`), instructions (`I:`), symbol
lines (`s:`) and remarks (`r:`). Because the letter's case does not matter, an ABC macro line
`m:` is **not** ignored: it is read as the meter field and is normally refused (§6.1); likewise
`x:`, `t:`, `k:`, `l:` and `q:` are the X, T, K, L and Q fields. Directives such as `%%name` are
comments (§3.4).

<a id="s5-2"></a>
### 5.2 Structure of the file (Confirmed (BIN))

There is no separate header and body. The reader treats every line the same way:

- blank lines, lines with only white space and comment-only lines are skipped anywhere;
- a field may start wherever a music element may start: at the start of a line, or after
  other elements on the same line (a field that starts after notes takes the rest of that
  line, §12 row B8);
- fields after notes are allowed and act like fields before them, with the part-wide timing
  rule of §8.1;
- a line that starts with a letter which is neither a note letter, a rest letter, a field
  (letter + colon) nor an accepted prefix is refused with **colon expected** (§7.1). Free text
  in a file is therefore refused unless it sits in a part that is skipped (§5.3) or after the
  end of the selected part.

<a id="s5-3"></a>
### 5.3 Parts and X: (Confirmed (BIN))

A **part** is the stretch of a file from an X: line to the next X: line. The command passes a
requested part number or "none" (§11). The reader reads the file from the top:

1. Everything before the first X: line is read normally — fields **and** music. It applies to
   every request.
2. At an X: field the value must start with a number token; otherwise the part is refused
   with **number expected** (this includes an empty `X:`).
3. If a part has already been selected, the reader **stops** here: the rest of the file is not
   read, whatever it contains.
4. Otherwise, if no number was requested, or the value equals the requested number, this
   part is **selected** and reading continues normally.
5. Otherwise the reader **skips** forward line by line. It looks only at lines that start
   (after optional white space) with a letter immediately followed by a colon. For a line
   whose letter is X or x, the text after the colon and white space is converted with C-style
   base detection: `0x` or `0X` prefix = hexadecimal, a leading `0` = octal, otherwise decimal,
   stopping at the first character that does not fit (no digits gives 0). If that value
   equals the requested number, that part is selected and reading continues with the next
   line. Every other line, including fields and music of skipped parts, is passed over
   unread. If the file ends first, the part is refused with **part not found**.
6. A file without any X: line is read entirely, whatever number was requested.

Consequences:

- Only one part is ever read. Fields of skipped parts never apply.
- The first X: value is read as decimal (§4) but later ones are read with base detection:
  a second part written `X:010` is found by a request for 8, not for 10.
- An empty `X:` refuses the part only if the reader reaches it: as the first X: line, or as
  the line that ends the selected part. An empty `X:` that is skipped over does not matter.

<a id="s5-4"></a>
### 5.4 Defaults (Confirmed (BIN))

At the start of every reading: meter 4/4, unit length 1/8, tempo 120, key signature with no
alterations (C major), volume 90 (§7.11), title empty, no bar accidentals.

---

<a id="s6"></a>
## 6. Field values

All value forms below are written without spaces inside them unless a space is shown.

<a id="s6-1"></a>
### 6.1 M: meter (Confirmed (BIN))

| Value | Meter |
|---|---|
| `C` (capital) | 4/4 |
| `C\|` | 2/2 |
| number `/` number | numerator / denominator |
| anything else: `none`, a lower-case `c`, a sum such as `3+2/8`, spaces around the slash, a missing part | refused: **meter malformed** |

Effects:

1. The numerator and denominator are stored.
2. If the denominator is not zero, the **unit length is reset**: to 1/16 when
   numerator ÷ denominator is below 0.75 (computed in double precision), otherwise to 1/8.
   This happens on **every** M: field, even when an L: field came before it; an L: field that
   should win must come after the M: field.
3. The denominator is the **beat** of the tempo (§8.2). The numerator only chooses the
   default tuplet ratio (§7.9). Bar lengths are never checked.

A denominator of 0 is accepted here; every note then lasts 0 s and the part is refused when
it is sequenced (note too short).

<a id="s6-2"></a>
### 6.2 L: unit note length (Confirmed (BIN))

The value must be number `/` number with a non-zero denominator; anything else (including a
single number, or spaces around the slash) is refused with **unit length malformed**. The unit
length becomes numerator ÷ denominator (double). A numerator of 0 is accepted here and
leads to "note too short" when sequenced. Which unit length the notes use is decided by
§8.1.

<a id="s6-3"></a>
### 6.3 Q: tempo (Confirmed (BIN))

| Value | Tempo |
|---|---|
| number *N* | *N* |
| number `/` number `=` number *N* | *N* (the beat fraction is stored but never used) |
| `C` [length as in §7.5] `=` number *N* | *N* |
| anything else: an empty value, text in quotes **before** the tempo, a lower-case `c`, a space anywhere inside the `/` and `=` forms | refused: **tempo malformed** |

A bare number followed by a space ends the value there (`Q:100 = …` is tempo 100 with the rest
ignored).

Text after a complete value is ignored (for example a tempo word in quotes after the number).
The tempo always counts beats of 1/(meter denominator) per minute, whatever note length the
Q: value names (§8.2). There is no range check: a tempo of 0 makes every sounding length
infinite and the part is refused as **note too long** when sequenced.

<a id="s6-4"></a>
### 6.4 K: key (Confirmed (BIN))

Reading a K: value:

1. **Highland-pipe keys:** if the value starts with the capital letter `H`, then a following `P` or
   `p` refuses the part with **key word unknown**; any other follower refuses it with
   **key malformed**.
2. **Tonic:** a note letter `A`–`G` or `a`–`g` (case does not matter for keys). Anything else
   — an empty value, `none`, a digit — refuses the part with **key malformed**.
3. **Tonic accidental:** directly after the letter, a `#` raises the tonic by one semitone, or
   a lower-case letter `b` lowers it by one. Only one sign is read.
4. **Mode:** if what follows (directly, or after one space token) is a letter, the reader
   looks at the next **three** tokens. If all three are letters, their lower-cased spelling
   must be one of the prefixes of the mode table below, otherwise the part is refused with
   **key word unknown**; after a match, all further consecutive letters are skipped (so
   `minor`, `Mixolydian`, `Dorian` work). If fewer than three letters follow — for example
   the single `m` of `Am` — **no mode is read and the key is major**; the letters left over
   are ignored as trailing text.
5. **Signature:** index = (tonic pitch class − mode offset) mod 12, where pitch classes count
   semitones from C (C = 0 … B = 11). The key signature becomes row *index* of the table in
   §6.4.1.
6. **Explicit accidentals:** after the key, the reader repeatedly takes an optional space
   token, an accidental (§7.3) and a note letter. Each complete pair sets the alteration of
   that letter's pitch class (any octave) to the accidental's value, replacing the table's
   value. The first item that is not such a pair ends the value; the rest of the line is
   ignored.

> **OPEN (implementation, 2026-10-07):** (1) step 4 with fewer than three letters: whether the
> one or two letters are consumed before step 6 (this decides whether `K:Am =c` gets the
> explicit natural). Tessera leaves them, so step 6 sees a letter and the value ends there.
> (2) Step 6 with an accidental that no note letter follows (`K:G ^ c`, `K:G ^_c`): whether
> the value just ends or the accidental reader of §7.3 refuses the part (unexpected symbol).
> Tessera ends the value without an error.

| Mode prefix | Offset |
|---|---|
| `maj`, `ion` | 0 |
| `dor` | 2 |
| `phr` | 4 |
| `lyd` | 5 |
| `mix` | 7 |
| `min`, `aeo` | 9 |
| `loc` | 11 |

Effects: the new signature applies to notes read **after** the K: field; notes already read
keep their pitches. A K: field does not clear the bar accidentals (§7.3).

Consequences worth testing: `K:Am`, `K:Em`, `K:F#m` and `K:Bbm` are read as **major** keys;
`K:Amin`, `K:A minor`, `K:Aaeolian` are A minor. `K:G ^c` is G major with C sharp. Clef and
transposition words (`clef=…`, `treble`, `bass`, `transpose=…`, `exp`) start with three letters
that are not a mode and refuse the part; a key followed by a word of fewer than three letters
(or a word that starts with a non-letter) is accepted and the word is ignored.

<a id="s6-4-1"></a>
#### 6.4.1 Key-signature table (Confirmed (BIN))

Rows: the signature index of step 5. Columns: the pitch class of the **natural note letter**
(C, C♯/D♭ … B); only the columns of natural letters (C, D, E, F, G, A, B) are ever looked up.
Values: semitones added to a note of that letter that has no accidental of its own.

| Index | Major key | C | D | E | F | G | A | B |
|---|---|---|---|---|---|---|---|---|
| 0 | C | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 1 | D♭ (five flats) | 0 | −1 | −1 | 0 | −1 | −1 | −1 |
| 2 | D | +1 | 0 | 0 | +1 | 0 | 0 | 0 |
| 3 | E♭ | 0 | 0 | −1 | 0 | 0 | −1 | −1 |
| 4 | E | +1 | +1 | 0 | +1 | +1 | 0 | 0 |
| 5 | F | 0 | 0 | 0 | 0 | 0 | 0 | −1 |
| 6 | F♯ (six sharps) | +1 | +1 | +1 | +1 | +1 | +1 | 0 |
| 7 | G | 0 | 0 | 0 | +1 | 0 | 0 | 0 |
| 8 | A♭ | 0 | −1 | −1 | 0 | 0 | −1 | −1 |
| 9 | A | +1 | 0 | 0 | +1 | +1 | 0 | 0 |
| 10 | B♭ | 0 | 0 | −1 | 0 | 0 | 0 | −1 |
| 11 | B (five sharps) | +1 | +1 | 0 | +1 | +1 | +1 | 0 |

Each index has one fixed spelling. Keys whose usual spelling differs are therefore misread:
C♯ major and A♯ minor use row 1 (five flats), G♭ major and E♭ minor use row 6 (six sharps),
C♭ major and A♭ minor use row 11 (five sharps). A tune written in such a key with plain
letters sounds wrong in the original, and must sound the same wrong way in Tessera.

<a id="s6-5"></a>
### 6.5 T: title (Confirmed (BIN))

The title is shown only in the "now playing" notice (§10.4). It never selects an instrument
and has no effect on playback.

1. The first T: field read whose text is not empty after trimming becomes the title; later T:
   fields are ignored. T: fields in skipped parts are never read.
2. Text: everything after the colon (and the optional space token) up to the line break,
   with comments and backslashes removed (§3.4). If no space follows the colon, the token
   right after the colon is kept only if it is a letter: a leading number or punctuation mark
   is lost (`T:99 …` loses `99`).
3. Spaces, tabs, CR, LF and FF are trimmed from both ends.
4. The notice uses at most the first 63 characters of the title.

---

<a id="s7"></a>
## 7. Music elements

<a id="s7-1"></a>
### 7.1 Element kinds and what ends a line (Confirmed (BIN))

A music line is a sequence of elements. The first token of each element decides its kind:

| First token | Element | Section |
|---|---|---|
| space | skipped | |
| open `(` | tuplet if a number follows, else slur start (ignored) | §7.9 |
| close `)` | ignored | |
| bar, double bar, thin-thick bar, repeat start, repeat end, double repeat, thick-thin bar | bar line: clears the bar accidentals | §7.2 |
| first ending, second ending, ending one, ending two | ignored (bar accidentals are **not** cleared) | §7.2 |
| plus `+` | volume mark | §7.11 |
| letter immediately followed by colon | field: takes the rest of the line | §5.1 |
| anything else | note, rest or chord with optional prefixes, then an optional broken rhythm | §7.3–§7.10 |

Music is read in **rounds**. A round starts at the beginning of a line, or at the token
where the previous round stopped, and reads elements one after another until an element
cannot be read. An element fails either at its first token (a token that cannot start any
element: a stray number, `]`, `}`, `:`, `#`, `,`, `'`, `/`, `-`, `<`, `>`, a non-note
letter, a line break, the end, an illegal character), or after it has consumed one or more
**prefixes** that are not followed by a note, rest or chord (§7.10); such prefixes are lost.

1. If the round read **at least one** complete element, it ends quietly at the failing token.
   Then line breaks are skipped; at the **end** the part is complete; at an **illegal
   character** (§4) the part is **truncated** — reading stops for good and everything read so
   far plays, without a message; otherwise a new round starts at that token.
2. If the round failed on its **first** element, the part is refused: with **colon expected**
   if the failing token is a letter (reported at the token after that letter), otherwise with
   **unexpected symbol**.

A round never starts at an illegal character or a line break (rule 1 handles them first), so
an illegal character at the start of a line also truncates. The difference between rule 1
and rule 2 shows when a prefix is followed by something that is not a note: `G "Am" A`
plays G and A (the prefix is lost, the next round starts at the space), but a line that
**starts** with `"Am" A` is refused, because the first element of that round failed (§12.6,
rows L16–L20).

Inner readers (chords, tuplets, volume marks, keys, accidentals) raise their own errors
first, as their sections say.

<a id="s7-2"></a>
### 7.2 Bars, repeats and endings (Confirmed (BIN + MA))

Bars and repeat signs only clear the bar accidentals. **Repeats are never played**: every
section plays once, in written order, and every ending plays in turn. Accepted and refused
forms:

| Written | Result |
|---|---|
| `\|` `\|\|` `\|]` `\|:` `:\|` `::` `[\|` | bar line |
| `:\|\|:` | repeat end + repeat start (two bar lines) |
| `::\|` | double repeat + bar |
| `\|1` `:\|2` `[1` `[2` | ignored ending markers; `:\|2` and `\|1` do **not** clear accidentals although they contain a bar |
| `:\|:` `:\|]` `[\|]` `[\|:` `\|\|:` `\|\|1` `:::` | refused (**unexpected symbol** at the trailing `:` `]` or number) |
| `\|2` `:\|1` `\|3` | refused (the number after the bar is a stray number) |
| `[3`, `[1,2`, `[1-3` | `[3` refused with **chord not closed**; the list forms refused with **unexpected symbol** |

<a id="s7-3"></a>
### 7.3 Notes, pitch and accidentals (Confirmed (BIN))

A note is: accidental (optional), note letter, octave marks (optional), length (optional, §7.5),
tie (optional, §7.6). Prefixes (§7.10) may precede it; a broken rhythm (§7.8) may follow it.

**Accidental:** one or more `^` (+1 semitone each), or one or more `_` (−1 each), or a single
`=` (0, natural). There is no limit on repeats (`^^^` is +3). Mixed signs such as `^_` are
refused (unexpected symbol). An accidental not followed by a note letter is refused with
**unexpected symbol**.

**Note letters and pitch numbers** (pitch numbers equal MIDI note numbers with ABC `C` = 60):

| Letter | C | D | E | F | G | A | B | c | d | e | f | g | a | b |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Natural pitch | 60 | 62 | 64 | 65 | 67 | 69 | 71 | 72 | 74 | 76 | 77 | 79 | 81 | 83 |

**Octave marks:** any number of `'` (+12 each), or any number of `,` (−12 each). Only one
kind is read: after apostrophes, a comma is a new (refused) element, and vice versa.

**Alteration** of a note, decided in this order:

1. If the note has an accidental, it is used, and it is remembered for this note letter in
   this octave (natural pitch + 12 × octave marks) until the next bar line.
2. Else, if an accidental is remembered for this letter in this octave, it is used.
3. Else, the key signature's value for the letter (§6.4.1).

Remembered accidentals are cleared only by the bar-line tokens of §7.2 — not by line breaks,
K: fields, ending markers or chords. They apply to the same letter **in the same octave only**
(`^c` does not affect `c'` or `C`).

**Pitch** = natural pitch + 12 × octave marks + alteration. The pitch must lie in **48 to 84**
(ABC `C,` to `c'`); below is refused with **note too low**, above with **note too high**,
reported at the token after the octave marks. The check applies after the alteration:
`^c'` and `_C,` are out of range; `^B,` is in range.

The sounding pitch on an instrument is decided by the sound system (`instruments.md`); the
reader passes on the pitch slot of §9.5.

<a id="s7-4"></a>
### 7.4 Rests (Confirmed (BIN))

A rest is the lower-case letter `z` or `x`, followed by an optional length and an optional
tie. A rest takes time like a note and produces no sound. Upper-case `Z` and `X` are not rests
(refused with **colon expected**, unless followed by a colon, which makes them fields).
An accidental or octave mark on a rest is refused. A tie on a rest has no effect.

<a id="s7-5"></a>
### 7.5 Lengths (Confirmed (BIN))

A length is: an optional number (the numerator, default 1), then optionally a slash part:

- `/` then a number: the denominator is that number;
- `/` then a second `/`: the second slash is consumed; the denominator is **4 if the
  numerator is 1, otherwise it stays 1**;
- `/` alone: the denominator is **2 if the numerator is 1, otherwise it stays 1**.

So `c/` = 1/2, `c//` = 1/4, `c/3` = 1/3, `c3/4` = 3/4, but **`c3/` = 3 and `c3//` = 3**. A third
slash is a stray token (unexpected symbol). A denominator of 0 is refused with
**zero length denominator**. A numerator of 0 is accepted and leads to "note too short".
The written length is numerator ÷ denominator in double precision. No spaces are allowed
inside a length.

<a id="s7-6"></a>
### 7.6 Ties (Confirmed (BIN + MA))

A tie is a `-` directly after the note's length (before any broken rhythm). It means: this
note continues into **the next note of the same pitch that is read later in the part,
wherever it is** — not necessarily the next note. Any number of notes may be chained. What this
does to the sound is in §9.1. Rules:

- the continuing note must have the same final pitch; a bar line in between clears an
  accidental, so a sharp note tied across a bar to an unsharpened letter does not continue;
- a tie that is never continued by the end of the part refuses the part with
  **unfinished tie** (reported at the tied note);
- a `-` that does not directly follow a note (after a space, after a bar, after a chord's `]`)
  is a stray symbol and refuses the part.

<a id="s7-7"></a>
### 7.7 Chords (Confirmed (BIN + MA))

A chord is `[` followed by one or more chord entries and `]`. Each entry is a note or rest in
the form of §7.3/§7.4 including its own length and tie, with **no** prefixes, spaces, volume
marks or broken rhythm inside. Rules:

1. An entry whose final pitch equals an earlier entry's pitch is **dropped entirely**: it does
   not sound, does not count and does not affect the chord's time advance. All rests of one
   chord count as one entry (the second and later rests are dropped).
2. More than **6** entries after dropping refuses the part with **too many notes in chord**.
3. Any other token where an entry or `]` is expected (a space, a prefix, a volume mark, a
   bar, a line break) refuses the part with **chord not closed**.
4. Nothing may modify a chord after its `]`: a length or a tie after `]` is a stray symbol
   (refused); a broken rhythm after `]` is refused with **broken rhythm misplaced**.
5. `[]` is an empty chord. It is added to the part (see §8.4 for its odd time advance), but
   it counts as an element that **failed** (§7.1): after other elements of the round, reading
   simply goes on; as the first element of a round it refuses the part (unexpected symbol).
6. Every entry sounds for its own length; the chord advances the time line by the
   **shortest** entry, rests included (§8.4).
7. A chord counts as one event towards the limit of §7.12.

> **OPEN (implementation, 2026-10-07):** (1) whether rule 2 is checked when the seventh kept
> entry is read or at the `]` (this decides between **too many notes in chord** and **chord not
> closed** for `[CDEFGAB c|`). Tessera checks when the seventh kept entry is read. (2) An empty
> chord `[]` as an element of a tuplet (§7.9): Tessera refuses it with **non-note in tuplet**.

<a id="s7-8"></a>
### 7.8 Broken rhythm (Confirmed (BIN + MA))

One to three `>` or `<` written directly after a single note or rest (no space before them):

1. With *n* symbols let *f* = 2ⁿ. The long factor is (2*f* − 1)/*f* (3/2, 7/4, 15/8) and the
   short factor is 1/*f* (1/2, 1/4, 1/8).
2. `>`: the note before is multiplied by the long factor and a **pending** short factor is
   stored. `<`: the note before gets the short factor and the pending factor is the long one.
3. The pending factor is applied to the **next note or rest read**, wherever it is: after
   white space, after bars or fields, on a later line, or as the first entry of a chord (only
   that entry). It is applied to the written length before any tuplet factor.
4. Four or more symbols refuse the part with **broken rhythm too long**. A broken rhythm after
   a chord's `]` refuses with **broken rhythm misplaced**; so does a new broken rhythm while a
   pending factor is still unused (only possible after `[]`). A `>` or `<` anywhere else —
   after a space, a bar, a slur's `)`, a grace group, or at the start of a line — cannot start
   an element and refuses the part with **unexpected symbol** (§7.1).
5. A pending factor left at the end of the part is dropped silently.

> **OPEN (implementation, 2026-10-07):** a mixed run such as `G><A`: whether the symbols are
> counted together or the second kind starts a new (refused) element. Tessera counts only the
> run of the first symbol, so `<` after `>` cannot start an element (unexpected symbol).

<a id="s7-9"></a>
### 7.9 Tuplets (Confirmed (BIN))

A tuplet is `(` *p* [`:` [*q*] [`:` [*r*]]] or `(` *p* `::` *r*, written without spaces, followed by
*r* elements:

1. *p* is a number from 2 to 9. *q* and *r*, when written, must also be 2 to 9. Any other
   value refuses the part with **tuplet number out of range**.
2. If *q* is missing: *p* = 2 → 3, 3 → 2, 4 → 3, 6 → 2, 8 → 3; for *p* = 5, 7 or 9, *q* = 3 if
   the meter numerator in force is divisible by 3, else 2. If *r* is missing, *r* = *p*.
3. The next *r* elements must each be a note, rest or chord with its optional prefixes
   (§7.10) and optional broken rhythm, **directly adjacent**: a space, bar, line break, volume
   mark, field or anything else where an element is expected refuses the part with
   **non-note in tuplet**. Triplets must be written like `(3abc`, not `(3 a b c`.
4. After each element (and its broken rhythm), its length is multiplied by *q*/*p*; for a
   chord, every entry is multiplied.
5. The multiplied length is the one checked against the limits of §8.3.

`(` followed by anything but a number is a slur start and is ignored.

> **OPEN (implementation, 2026-10-07):** the position reported for **non-note in tuplet**
> when prefixes come first (`(3~ G A B|`). Tessera follows §10.2 and reports the token after
> the prefixes (the space), and an empty chord `[]` in a tuplet at its `[`; the original's
> choice was not traced.

<a id="s7-10"></a>
### 7.10 Prefixes (Confirmed (BIN))

Before a note, rest or chord the reader skips, in this order, each at most once:

1. **Quoted text** `"…"` (chord symbols, annotations): skipped up to the next quote token
   **anywhere later in the file**, across line breaks. An unclosed quote therefore swallows
   the rest of the file: the part ends there (what was read before plays), or, if the quote
   began the first element of a round, the part is refused with unexpected symbol (§7.1).
2. **Grace group** `{…}`: skipped up to the next `}` anywhere later in the file, the same way.
   Grace notes are never played.
3. **One decoration**: `~`, `.`, or the letter `u` or `v` — ignored.

The note, rest or chord must follow the last prefix **directly**. Prefixes followed by
anything else are lost and §7.1 decides what happens: reading goes on if the round had
already read an element, and the part is refused if the prefixes began the round.

Any other decoration — the letters `T H L M O P S J R`, `y`, `!…!` (the `!` is an illegal
character), a second decoration, a decoration after the note — follows §7.1: letters are
refused with **colon expected**, `!` truncates the part (or is refused when it directly
follows prefixes that began a round).

<a id="s7-11"></a>
### 7.11 Volume marks (Confirmed (BIN + MA))

`+` *name* `+`, where *name* is one or more letters, compared without regard to case:

| Name | pppp | ppp | pp | p | mp | mf | f | ff | fff | ffff |
|---|---|---|---|---|---|---|---|---|---|---|
| Volume value | 30 | 30 | 45 | 60 | 75 | 90 | 105 | 120 | 127 | 127 |

The value applies to every note read after the mark until the next mark (default 90). It is
attached to each note when the note is read, so it cannot change inside a chord. Errors:
a name not in the table (`+trill+`, `+fermata+`) → **unknown volume mark**; a non-letter
after the first `+` (a space, a digit, a second `+`) → **unknown volume mark**; no closing
`+` after the letters → **closing plus expected**. A volume mark inside a chord or a tuplet is
refused by those rules (§7.7, §7.9). The mapping of the values to loudness belongs to
`sound.md`.

> **OPEN (implementation, 2026-10-07):** which error wins when an unknown name also lacks its
> closing `+` (`+xyz G`). Tessera reads the letters, then requires the `+` (**closing plus
> expected**), then looks the name up (**unknown volume mark**).

<a id="s7-12"></a>
### 7.12 Event limit (Confirmed (BIN + MA))

Each single note, rest and chord read is one **event**. When the part has more than
**10,000** events the reader stops and the part is refused; this uses a separate notice, not
one of the entries of §10.2 (§10.4).

---

<a id="s8"></a>
## 8. Timing

<a id="s8-1"></a>
### 8.1 Part-wide timing values (Confirmed (BIN))

Durations are computed **after** the whole part has been read. They use the meter
denominator, the unit length and the tempo **in force at the end of reading** — that is, the
values of the last M:, L: and Q: fields read for this part (remembering that every M: resets
the unit length, §6.1). A field that appears after notes therefore changes the duration of
**every note of the part, including those before it**. By contrast, key signatures (§6.4) and
volume marks (§7.11) apply from where they are read.

<a id="s8-2"></a>
### 8.2 Duration formula (Confirmed (BIN + MA))

For each note and rest, including every chord entry:

1. *den* = the meter denominator, converted to double;
2. *L* = the unit length (double, §6.1/§6.2);
3. *len* = the written length (§7.5) as a double, then multiplied (rounding after each
   product) first by a pending factor from a previous broken rhythm, then by the note's own
   broken-rhythm factor (§7.8), then by its tuplet factor *q*/*p* (§7.9), as far as present;
4. *bpm* = the tempo, converted to double.

The duration in seconds is computed in IEEE-754 double precision, **in exactly this order**,
rounding after each step: *den* × *L*, then × *len*, then × 60, then ÷ *bpm*.

The beat is therefore one 1/*den* note: M:6/8 with Q:90 means 90 eighths per minute, whatever
note value the Q: field names. With the defaults (M:4/4, L:1/8, Q:120) a unit note lasts
0.25 s.

<a id="s8-3"></a>
### 8.3 Limits (Confirmed (BIN + MA))

Every duration of §8.2 — notes, rests and every chord entry, each tied note separately — must
satisfy:

- duration ≥ 0.06 (the double nearest to 0.06); otherwise **note too short**;
- duration ≤ 8.0; otherwise **note too long**.

Exactly 0.06 and exactly 8.0 are accepted. The first failing note in reading order is
reported. Because of the rounding in §8.2, some notes that last exactly 60 ms on paper come
out one unit below the double 0.06 and are refused, while others come out exactly at it and
pass; which ones depends on tempo, unit length and note length together (§13.2). A tied
chain may last longer than 8 s, because each tied note is checked on its own (§12 row N3c).

These checks happen after reading; a refusal here still plays nothing.

<a id="s8-4"></a>
### 8.4 Time line (Confirmed (BIN))

The time line starts at 0 s. Events are taken in reading order:

- a single note or rest starts at the current time and advances it by its own duration;
- a chord's entries all start at the current time; the chord advances it by its **shortest**
  entry, including rests;
- an empty chord `[]` advances it by **−1 s** (the time line moves back one second; later notes
  start earlier than earlier ones).

Rests make no sound. A rest at the start delays the first note; rests at the end add nothing
to the performance (it ends with its last note-off, §9.4).

<a id="s8-5"></a>
### 8.5 Start (Confirmed (BIN))

A solo performance (no sync flag) starts **0.7 s** after the command is processed. A synced
performance is read and checked immediately (errors are reported at once) and then waits;
the start command supplies the start time (§11).

> **OPEN (for `sound.md` / `instruments.md`):** the original's performance stepper, which hands
> the events of §9.4 to the sound system after the start, compares how far the performance
> position is ahead of the clock with a constant of **0.5 s** and, below it, walks forward
> through events while only updating which notes are held (BIN, identified, not analysed).
> It looks like catch-up for a performance that runs late, and may be the same rule as the
> 0.5 s late-note rule of [`instruments.md`](instruments.md) §8.2–§8.3. It does not affect what
> the reader accepts or the event list; it belongs to the playback specs, not to this one.

---

<a id="s9"></a>
## 9. Note events

<a id="s9-1"></a>
### 9.1 Generating events (Confirmed (BIN + MA))

The sequencer walks the events with the time line of §8.4. For every sounding note
(rests make none), with *t* its start and *d* its duration:

1. If no tie is pending for its pitch, emit a **note-on** at *t*. Then, if the note is not
   tied, emit a **note-off** at *t* + *d*; if it is tied, keep a **pending note-off** for its
   pitch at *t* + *d*.
2. If a tie is pending for its pitch, emit **no** note-on; add *d* to the pending note-off's
   time. If this note is not tied itself, emit the pending note-off now (at its accumulated
   time) and clear it; if it is tied, keep it pending.

So `G-AG` sounds one G from the start of the first G for the sum of both G durations, over
the A; the second G makes no sound of its own. The volume of a note-on and its note-off is the
volume of the note that started it.

<a id="s9-2"></a>
### 9.2 Ordering (Confirmed (BIN))

All note-ons and note-offs are sorted by time. Two times that differ by less than **0.0001 s**
count as equal; equal times keep the order in which the events were created. Events are
created per note in reading order: the note-on, then at once its note-off — or, for a tied
note, its pending note-off, which keeps this creation place even though it is released
later (§9.1).

> **OPEN (implementation, 2026-10-07):** the sort algorithm. "Closer than 0.0001 s counts as
> equal" is not transitive (0, 0.00006 and 0.00012 s), so the result depends on the algorithm,
> which is not described. Tessera sorts by exact time (then creation order), then puts each
> run of events lying within 0.0001 s of the run's first event into creation order. Within
> such a run a time may be up to 0.0001 s below its predecessor. Also, a full sort
> never lets output times decrease, but INV10 (§14.1) allows a decrease after an empty
> chord. Is the original's sort partial, for example a bounded backward insertion?

<a id="s9-3"></a>
### 9.3 Re-attacks (Confirmed (BIN))

The sorted events are then passed through once, keeping the set of pitches currently sounding:

- a note-on for a pitch that is **not** sounding is emitted and the pitch becomes sounding;
- a note-on for a pitch that **is** sounding is emitted after an extra note-off for that pitch
  at the same time (the extra note-off carries the **new** note's volume); the pitch stays
  sounding;
- a note-off for a sounding pitch is emitted and the pitch stops sounding;
- a note-off for a pitch that is not sounding is **dropped**.

Effect: when a pitch starts again while it still sounds (for example a long chord entry
followed by the same pitch), the old note is cut at the new start, and whichever note-off of
that pitch comes first afterwards ends the new note.

> **OPEN:** a published report (MA) says the original plays nothing of a part in which a
> still-sounding pitch is struck again with a different volume. The reader does not refuse
> such a part; if the effect is real it happens in the audio layer (the extra note-off carries
> the new volume, §9.4). To be settled with `sound.md` (owner check OC6 in §17).

<a id="s9-4"></a>
### 9.4 Output (Confirmed (BIN))

The performance handed to the sound system is a list of events, each with:

| Field | Meaning |
|---|---|
| delta | seconds since the previous emitted event (the first counts from time 0) |
| kind | note-on, note-off, or end |
| slot | pitch slot (§9.5) |
| volume | volume value of §7.11 (30–127) |

A single **end** event with delta 0 follows the last event.

In the original, slot and volume travel packed in one unsigned 32-bit word (Confirmed (BIN)):

| Bits | Content |
|---|---|
| 0–23 | slot (§9.5; 1–37, so bits 8–23 are always 0) |
| 24–31 | volume (§7.11; 30–127) |

The volume is packed only when it is at most 127, which every value of §7.11 is; a larger
value would leave bits 24–31 at 0. The tie bookkeeping of §9.1 is keyed by the slot, and the
re-attack pass of §9.3 by the **low 8 bits** of the word, which are the slot; neither looks at
the volume. Tessera need
not pack the fields; the layout is given for comparison with [`instruments.md`](instruments.md)
§5, which receives the same word.

<a id="s9-5"></a>
### 9.5 Pitch slots (Confirmed (BIN))

The sequencer converts each pitch to one of 37 **slot** numbers with a fixed table. What each
slot plays is decided by the instrument data (`instruments.md`).

| Pitch | 48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 | 56 | 57 | 58 | 59 | 60 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ABC | `C,` | `^C,` | `D,` | `^D,` | `E,` | `F,` | `^F,` | `G,` | `^G,` | `A,` | `^A,` | `B,` | `C` |
| Slot | 28 | 12 | 2 | 1 | 21 | 31 | 30 | 5 | 20 | 9 | 17 | 16 | 29 |

| Pitch | 61 | 62 | 63 | 64 | 65 | 66 | 67 | 68 | 69 | 70 | 71 | 72 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ABC | `^C` | `D` | `^D` | `E` | `F` | `^F` | `G` | `^G` | `A` | `^A` | `B` | `c` |
| Slot | 14 | 4 | 3 | 25 | 34 | 33 | 8 | 24 | 11 | 19 | 18 | 32 |

| Pitch | 73 | 74 | 75 | 76 | 77 | 78 | 79 | 80 | 81 | 82 | 83 | 84 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ABC | `^c` | `d` | `^d` | `e` | `f` | `^f` | `g` | `^g` | `a` | `^a` | `b` | `c'` |
| Slot | 15 | 7 | 6 | 27 | 37 | 36 | 10 | 26 | 13 | 23 | 22 | 35 |

**Meaning of the slots** (Confirmed (BIN + OBS), 2026-10-05): every slot of the table is the
key of a note label in enum mapper **0x2300015E** (the note mapper of
[`instruments.md`](instruments.md) §3.2, keys 1–37 plus key 0 `Undef`). For pitch *p* the label
is `Note_` + name + octave, with *i* = *p* − 48, name = entry *i* mod 12 of C, Db, D, Eb, E, F,
Gb, G, Ab, A, Bb, B, and octave = 2 + ⌊*i* / 12⌋. The probe checked all 37 pitches: each slot of
the client's fixed table equals the key of its label (pitch 48 → `Note_C2` → 28, pitch 60 →
`Note_C3` → 29, pitch 72 → `Note_C4` → 32, pitch 84 → `Note_C5` → 35). The labels are
interoperability identifiers; the octave digits in them are names, not sounding octaves
(`instruments.md` §7.4).

The client's fixed table reproduces the original exactly. [`instruments.md`](instruments.md)
§7.1 recommends deriving the same keys from the labels at run time; in this install both give
the same 37 values.

The live-play note actions are a separate set of names: enum mapper **0x230000AF** (the
InputActions mapper of [`keymap.md`](keymap.md)) holds 37 actions named `MUSIC_` + the same
37 name-and-octave suffixes (`MUSIC_C2` … `MUSIC_C5`; OBS, all 37 checked against the
suffixes of 0x2300015E). The binary does not link these actions to the pitch numbers of this
spec; that ABC `C,` corresponds to `MUSIC_C2` is an **inference** from the shared suffixes (37
action names against 37 note labels and 37 slots) and from the action/note pairs of the
live-play map ([`instruments.md`](instruments.md) §4.6, which pairs `MUSIC_<n>` with
`Note_<n>`). The owner's `lotro.keymap` binds only 21 of these actions (`MUSIC_B2` to
`MUSIC_C5`; no `MUSIC_C2`), so the key-mapping file is not a source for the full set.

> **OPEN:** whether a future data update could reorder the keys of 0x2300015E while the
> client's fixed table stays the same; then the fixed table and the label rule would disagree.
> Not observable in this install; `instruments.md` decides which one the sound data follows.

---

<a id="s10"></a>
## 10. Errors and messages

<a id="s10-1"></a>
### 10.1 Error model (Confirmed (BIN))

- The first error stops everything: no further checking, one message, and **nothing of the
  part plays**.
- Errors found while reading (file, fields, music) come before timing errors (§8.3) and the
  unfinished-tie check (§7.6), which run after the whole part has been read.
- Truncation by an illegal character (§7.1) is not an error and shows no message.

<a id="s10-2"></a>
### 10.2 Error kinds and their message entries (Confirmed (BIN + OBS))

The names in the first column are this spec's own. Each error is shown as a string-table
entry, identified by a **token** (the name hash of [`records.md`](records.md) §1.5 of the client's
internal identifier). Arguments are bound by argument ID (string-table.md §7): **line**
0x00050E25, **column** 0x04841A1E, **file name** 0x0E0A22B5.

| Error kind | Token | Arguments | Raised by |
|---|---|---|---|
| encoding not supported | 0x06042C77 | file name | §3.2 |
| file not found | 0x010998C4 | file name | §3.1, §3.2 |
| meter malformed | 0x0B61FD02 | line, column | §6.1 |
| tempo malformed | 0x0B4893AF | line, column | §6.3 |
| unit length malformed | 0x094B9348 | line, column | §6.2 |
| number expected | 0x0CE3A5F2 | line, column | §5.3 (X: value) |
| part not found | 0x0BCA2D44 | none (the entry has no argument slots) | §5.3 |
| key malformed | 0x07CB63F9 | line, column | §6.4 |
| key word unknown | 0x075D3819 | line, column | §6.4 |
| unexpected symbol | 0x01D6B85E | line, column | §7.1, §7.3 |
| colon expected | 0x03B3DC8E | line, column | §7.1, §5.2 |
| non-note in tuplet | 0x03C95A94 | line, column | §7.9 |
| tuplet number out of range | 0x0F9788D5 | line, column | §7.9 |
| unknown volume mark | 0x0F84EA5E | line, column | §7.11 |
| closing plus expected | 0x0D3AEA53 | line, column | §7.11 |
| broken rhythm misplaced | 0x063437FD | line, column | §7.7, §7.8 |
| broken rhythm too long | 0x0FE8D55D | line, column | §7.8 |
| chord not closed | 0x03FF4F07 | line, column | §7.7 |
| too many notes in chord | 0x0DD06FA4 | line, column | §7.7 |
| zero length denominator | 0x0316B41E | line, column | §7.5 |
| note too low | 0x08AE3917 | line, column | §7.3 |
| note too high | 0x0AE24858 | line, column | §7.3 |
| note too short | 0x0E53AE34 | line, column | §8.3 |
| note too long | 0x0AE393C7 | line, column | §8.3 |
| unfinished tie | 0x0D27C694 | line, column | §7.6 |

Positions: errors report the token at which the problem was seen; timing errors and the
unfinished tie report the note's letter (or rest letter).

<a id="s10-3"></a>
### 10.3 Finding the message table (Confirmed (BIN + OBS))

1. The client names the table by the **key 0x10000069** of the named-string-table DID map
   (string-table.md §9.1). In this install that map is 0x28000004 (resolved through the root
   label `STRINGTABLE`, records.md §8.2), and key 0x10000069 carries the label `UI_Music`
   (an interoperability identifier; resolve it at run time) and maps to table **0x2500808A**.
2. All 25 tokens of §10.2 exist in that table in the English archive, with exactly the
   argument IDs listed (observed 2026-10-05).
3. A reader shows the entry from the user's selected locale (string-table.md §10) and renders
   it with the arguments; when the table or entry is missing it shows its own text.

<a id="s10-4"></a>
### 10.4 Notices (Confirmed (BIN + OBS))

| When | Token (same table) | Argument |
|---|---|---|
| A solo performance starts and the title is not empty | 0x00A19C55 | title (ID 0x0058E905), at most 63 characters |
| A solo performance starts and the title is empty | 0x05D7A7A7 | none |
| The synced start command is processed | 0x012DD4A4 | none |

> **OPEN:** the source of the notice shown when a part exceeds 10,000 events (§7.12); the
> client shows it through a different message path than §10.3, and its text source was not
> traced. Tessera may use its own text.

---

<a id="s11"></a>
## 11. Commands around the reader

What reaches the reader (Confirmed (BIN)): a file name, a part number or "none", and a sync
flag. The chat commands themselves are game-script code that calls client functions
(native IDs 0x07DE0001 play, 0x07DE0002 synced start, 0x07DE0003 list the music folder);
the command syntax below is Reported (WEB, ER):

- `/play <name> [<part>] [sync]`: read part *part* of `<name>` (§3.1, §5.3). Names with spaces
  are written in quotes (WEB). Without `sync`, the performance starts 0.7 s later (§8.5).
  With `sync`, the checked performance waits.
- `/playstart`: starts every waiting performance of the fellowship at a common start time.
- `/playlist [<filter>]`: lists the songs in the music folder. The client lists files whose
  extension is `.abc` or `.txt` (letter case ignored) and, for each, the X: numbers found
  with the first T: after each one (Confirmed (BIN)).

> **OPEN:** (1) which part number the command layer passes when `<part>` is omitted. The
> reader treats "none" as "the first X: line" (§5.3); whether the script passes "none" or a
> number is in the game-script data and was not read. (2) The `/playlist` filter syntax and
> whether sub-folders are listed. (3) Where the synced start time comes from and the meaning
> of the second start value (0.7 s for a solo start).

> **OPEN (implementation, 2026-10-07):** how the listing reads the X: values (decimal as for
> the first X: of §5.3, or with base detection as for later ones) and which T: it shows (the
> first one after the X:, or the first non-empty one). Tessera's `list_songs`: a line counts
> when it starts, after optional white space, with a letter and a colon. X: values use base
> detection (what a request matches), and the first non-empty, trimmed T: before the next X:
> is shown. Sub-folders are not entered, and files that cannot be read list no parts.

> **OPEN (implementation, client commands, 2026-10-07):** the client's player-music commands
> (`apps/client/src/audio/player_music.rs`) had to choose where this section is silent.
> Please confirm or state the original's behaviour:
> (a) **Stop command name.** The spec names no stop command; Tessera uses `/playstop`
> (cancels the local player's queued and sounding notes, `instruments.md` §8.5, and
> withdraws a waiting synced part).
> (b) **`/playlist` filter** (OPEN (2) above): Tessera lists a song when the filter text
> occurs, ignoring ASCII letter case, in its file name, its file-level title or one of its
> part titles.
> (c) **Music folders.** Tessera looks in the original's settings-folder `Music` first and
> then in its own data folder's `Music`, reporting **file not found** with the first
> folder's `.txt` path; the original has only the first.
> (d) **Command arguments.** The part must be a decimal number in the 32-bit range (no base
> detection, unlike X: values); `sync` is accepted in any letter case; anything else after
> the name refuses the command with Tessera's own usage text (the original's script
> behaviour is not in this spec).
> (e) **When the 0.7 s starts.** Tessera reads the file (and decodes the samples it needs)
> off the frame thread and starts the solo performance 0.7 s after that read has finished
> ("after the command is processed", §8.5).
> (f) **Synchronised group in the sandbox.** Without fellowships, the group is every local
> player (one); `/playstart` shows the synced-start notice of §10.4 whenever it is processed,
> also when nothing waits.
> (g) **Command order.** Tessera processes the commands strictly in the order given: a
> command waits until every earlier one has finished, including an earlier `/play`'s file
> read, so `/playstop` or `/playstart` given while a `/play` is still being read acts after
> it, and of two `/play`s the later one plays. This assumes the original processes each
> command completely before the next (its reader runs inside the play call); whether its
> chat layer can reorder or drop commands typed in quick succession was not traced.
> The music folders are tested on every command, so a `Music` folder created while the
> client runs is used at once.

---

<a id="s12"></a>
## 12. Conformance table

All tunes are original to this spec. Unless a row says otherwise, a tune is the **reference
header** — six lines `X:1`, `T:t`, `M:4/4`, `L:1/8`, `Q:120`, `K:C` — followed by the body shown
(`⏎` marks a line break; every file ends with a line break). With this header one unit lasts
0.25 s. "Plays" lists note-on times in seconds and pitches; "refused" names the error kind of
§10.2. "Truncated" means it plays what is listed and nothing else, without a message.
Request = the part number passed by the command (default: none).

<a id="s12-1"></a>
### 12.1 Files, comments and lines

| ID | File / body | Expected |
|---|---|---|
| A1 | Reference tune `G A\|` saved as UTF-8 with BOM | plays 0 G(67), 0.25 A(69) |
| A2 | Same text saved as UTF-16 LE with BOM | refused: encoding not supported (argument = the file's path) |
| A3 | Same text preceded by the bytes `2B 2F 76` | refused: file not found |
| A4 | Same text with CR LF line ends, and again with lone CR line ends | identical to LF line ends |
| A5 | `G A\|` then a NUL byte, then `B c\|` on the same line | plays G, A only |
| A6 | UTF-8 with BOM, body `G A` then a 4-byte UTF-8 character, then `B c\|` | plays G, A only |
| A7 | Name `tune`: only `tune.txt` exists | found and played |
| A8 | Name `tune`: both `tune.abc` and `tune.txt` exist, with different notes | `tune.abc` is played |
| B1 | `G A % B c⏎B c\|` | plays 0 G, 0.25 A, 0.5 B(71), 0.75 c(72) |
| B1b | `G A \% B c⏎B c\|` | same as B1 (`\%` still starts a comment) |
| B2 | `G A \⏎B c\|` | same as B1 |
| B3 | `G A\|⏎⏎ ⏎B c\|` (blank and white-space lines) | same as B1 |
| B4 | `G A\|⏎w:one two⏎B c\|` | same as B1 |
| B5 | `G A\|⏎l:1/4⏎B c\|` (lower-case field letter) | plays 0, 0.5, 1.0, 1.5 (L:1/4 for all four notes, §8.1) |
| B5b | `G A\|⏎N:anything⏎R:reel⏎B c\|` | same as B1 |
| B6 | `G A\|⏎Play it softly⏎B c\|` | refused: colon expected |
| B7 | `G A\|⏎+:more` | refused: unknown volume mark |
| B8 | `G A K:D f c\|` | plays G, A only (the K: field takes the rest of the line) |
| B9 | Reference header with its key line written `K :C`, body `G A\|` | refused: colon expected |
| B10 | `   K:G⏎F A\|` (field after leading spaces) | plays 0 F♯(66), 0.25 A |

<a id="s12-2"></a>
### 12.2 Parts

Rows C1–C3 use this file: `T:Set`, `M:4/4`, `L:1/8`, `Q:120`, `X:1`, `K:C`, `L:1/4`, `G A|`,
`X:2`, `K:C`, `B c|`, `X:3`, `K:C`, `d e|` (one item per line).

| ID | File / request | Expected |
|---|---|---|
| C1 | request none | plays 0 G, 0.5 A (part 1, its own L:1/4) |
| C2 | request 2 | plays 0 B, 0.25 c (part 1's L:1/4 does not apply) |
| C3 | request 4 | refused: part not found |
| C4 | Reference tune without its X: line, body `G A\|`, request 3 | plays G, A |
| C5 | Reference tune `G A\|⏎X:⏎B c\|`, request none | refused: number expected (the empty X: ends the selected part) |
| C6 | Reference tune `G A\|⏎X:⏎B c\|⏎X:3⏎d e\|`, request 3 | plays 0 d(74), 0.25 e(76) (the empty X: is skipped) |
| C7 | Reference tune `G A\|⏎X:010⏎B c\|`, request 8 | plays B, c |
| C8 | Same as C7, request 10 | refused: part not found |
| C9 | Reference tune with the body `G A\|`, then a line `X:2`, then a line `!! [[`, then `B c\|` | plays G, A (nothing after X:2 is read) |
| C10 | `T:t`, `M:4/4`, `L:1/8`, `Q:120`, `K:C`, `E F\|`, `X:1`, `G A\|`, `X:2`, `B c\|`, request 2 | plays 0 E(64), 0.25 F(65), 0.5 B, 0.75 c (music before the first X: belongs to every request) |
| C11 | Reference tune with `x:1` in place of `X:1`, body `G A\|⏎x:2⏎B c\|` | plays G, A |

<a id="s12-3"></a>
### 12.3 Header fields

| ID | Header change / body | Expected |
|---|---|---|
| D1 | `M:C`; `G A\|` | plays 0, 0.25 |
| D2 | `M:C\|`; `G A\|` | plays 0, 0.125 (denominator 2) |
| D3 | `M:3/8`, no L: line; `G A\|` | plays 0, 0.25 (unit 1/16, beat 1/8) |
| D4 | `M:none` | refused: meter malformed |
| D5 | `M:3+2/8` | refused: meter malformed |
| D6 | Header order `L:1/4` then `M:4/4`; `G A\|` | plays 0, 0.25 (the M: resets the unit to 1/8) |
| D7 | `L:1` | refused: unit length malformed |
| D8 | `M:6/8`, `Q:3/8=90`; `G A\|` | plays 0, 0.6667 (90 eighths per minute) |
| D9 | `Q:"Slow" 1/4=100` | refused: tempo malformed |
| D10 | `Q:1/4=100 "Slow"`; `G A\|` | plays 0, 0.3 |
| D11 | `Q:C=100`; `G A\|` | plays 0, 0.3 |
| D12 | `K:Em`; `F G c d\|` | plays pitches 66, 68, 73, 75 (E **major**) |
| D13 | `K:E minor`; `F G c d\|` | plays 66, 67, 72, 74 |
| D14 | `K:Edor`; `F G c d\|` | plays 66, 67, 73, 74 |
| D15 | `K:D _B`; `F B c\|` | plays 66, 70, 73 |
| D16 | `K:Hp` | refused: key word unknown |
| D17 | `K:` (empty) | refused: key malformed |
| D18 | `K:G treble` | refused: key word unknown |
| D19 | `K:Gb`; `G B c\|` | plays 68, 71, 73 (row 6 of §6.4.1) |
| D20 | `G A B c\|⏎L:1/16⏎d e\|` | plays every note 0.125 s apart (0, 0.125 … 0.625) |
| D21 | `G A B c\|⏎Q:60⏎d e\|` | plays every note 0.5 s apart |
| D22 | `G A\|⏎M:3/4⏎B c\|` | plays 0, 0.25, 0.5, 0.75 (meter changes nothing else here) |
| D23 | Lower-case `m:3/4`, `l:1/4`, `q:60`, `k:G`; `F A\|` | plays 0 F♯(66), 1.0 A |
| D24 | Two lines `T:` and `T:   ` before `T:Second`; `G\|` | title "Second" |

<a id="s12-4"></a>
### 12.4 Notes, accidentals and lengths

| ID | Body | Expected |
|---|---|---|
| E1 | `C, c'\|` | plays 48, 84 |
| E2 | `B,, c\|` | refused: note too low |
| E3 | `^c' c\|` | refused: note too high |
| E4 | `^^^C D\|` | plays 63, 62 |
| E5 | `_B B, B b\|` | plays 70, 59, 70, 83 |
| E6 | `_e \| e\|` | plays 75, 76 |
| E7 | `_e \|1 e\|` | plays 75, 75 (ending marker keeps the flat) |
| E8 | `_e :\|2 e\|` | plays 75, 75 |
| E9 | `=_e\|` | refused: unexpected symbol |
| E10 | Key `K:D`; `f =f f\|f\|` | plays 78, 77, 77, 78 |
| F1 | `G/ A// B3/ c3/2 d\|` | plays at 0, 0.125, 0.1875, 0.9375, 1.3125 |
| F2 | `G/0 A\|` | refused: zero length denominator |
| F3 | `G0 A\|` | refused: note too short |
| F4 | `G/// A\|` | refused: unexpected symbol |
| F5 | `L:1/2000000` in the header; `G250000 A250000\|` | plays 0, 0.25 |
| F6 | `G00002 A\|` (leading zeros) | plays 0, 0.5 |
| F7 | `G99999999999 A\|` (above the 32-bit limit) | refused: note too long (the length saturates at 2,147,483,647, §4.1) |
| F8 | `G1\2 A\|` (backslash inside a digit run) | plays 0, 3.0 (the run is `12`, §3.4, §4.1) |
| G1 | `z2 G x2 A\|` | plays 0.5 G, 1.25 A |
| G2 | `G Z A\|` | refused: colon expected |
| G3 | `z- G\|` | plays 0.25 G |

<a id="s12-5"></a>
### 12.5 Chords, ties and broken rhythm

| ID | Body | Expected |
|---|---|---|
| H1 | `[CEG] c\|` | plays 60, 64, 67 at 0; c at 0.25 |
| H2 | `[G4B] c\|` | G and B at 0, c at 0.25; G sounds until 1.0 |
| H3 | `[G4z] c\|` | G at 0, c at 0.25; G sounds until 1.0 |
| H4 | `[CDEFGAB] c\|` | refused: too many notes in chord |
| H5 | `[CDEFGAz] c\|` | refused: too many notes in chord |
| H6 | `[CDEFGzz] c\|` | plays five notes at 0, c at 0.25 |
| H7 | `[CDEFGAA] c\|` | plays six notes at 0, c at 0.25 |
| H8 | `[G2G4] c\|` | one G at 0 lasting 0.5; c at 0.5 |
| H9 | `[GB]4 c\|` | refused: unexpected symbol |
| H10 | `[GB]- [GB]\|` | refused: unexpected symbol |
| H11 | `[G B] c\|` | refused: chord not closed |
| H12 | `[GB c\|` | refused: chord not closed |
| H13 | `G [] A\|` | plays G at 0, A at −0.75 (the empty chord moves the time line back 1 s) |
| H14 | `[] G A\|` | refused: unexpected symbol (the empty chord began the round) |
| I1 | `G4-G4\|` | one G at 0 lasting 2.0 |
| I2 | `G-AG B\|` | G at 0 (lasting 0.5, over the A), A at 0.25, B at 0.75 |
| I3 | `G-A B\|` | refused: unfinished tie |
| I4 | `^G-\|G A\|` | refused: unfinished tie |
| I5 | `[G-B]G c\|` | G and B at 0 (G lasting 0.5), c at 0.5 |
| J1 | `G2>A2 B2<c2 d4>>e4 f8>>>g8\|` | plays at 0, 0.75, 1.0, 1.25, 2.0, 3.75, 4.0, 7.75 |
| J2 | `G>>>>A\|` | refused: broken rhythm too long |
| J3 | `[GB]>c d\|` | refused: broken rhythm misplaced |
| J4 | `G>[Bd] c\|` | G at 0, B and d at 0.375 (B lasting 0.125, d 0.25), c at 0.5 |
| J5 | `G >A\|` | refused: unexpected symbol |
| J6 | `G>⏎A B\|` | plays 0, 0.375, 0.5 |

<a id="s12-6"></a>
### 12.6 Tuplets, prefixes, volume and bars

| ID | Body | Expected |
|---|---|---|
| K1 | `(3GAB c\|` | plays at 0, 0.1667, 0.3333, 0.5 |
| K2 | `(3G A B c\|` | refused: non-note in tuplet |
| K3 | `M:3/4` in the header; `(5GABcd e\|` | notes 0.15 s apart, e at 0.75 |
| K4 | `(5GABcd e\|` | notes 0.1 s apart, e at 0.5 |
| K5 | `(3:2:4GABc d\|` | four notes 0.1667 s apart, d at 0.6667 |
| K6 | `(10GABcdefgab c\|` | refused: tuplet number out of range |
| K7 | `(3[GB]cd e\|` | G and B at 0, then 0.1667, 0.3333, e at 0.5 |
| K8 | `Q:100` in the header; `(3G/4A/4B/4 c\|` | refused: note too short (each lasts 0.05 s) |
| K9 | `Q:100` in the header; `G/4 c\|` | plays 0, 0.075 |
| L1 | `"Dm"G A\|` | plays G, A |
| L2 | `"Dm G A\|⏎B c\|` | refused: unexpected symbol (the quote swallows the rest and began the round) |
| L3 | `{ab}G A\|` | plays G, A (grace notes not played) |
| L4 | `~G .A uB vc\|` | plays G, A, B, c |
| L5 | `~.G A\|` | refused: unexpected symbol |
| L6 | `G Hc\|` | refused: colon expected |
| L7 | `G A !p!B c\|⏎d e\|` | truncated: plays G, A |
| L8 | `G A $ B c\|`, and the same with a back-quote in place of the `$` and its spaces (`` G A`B c\| ``) | truncated: plays G, A |
| L9 | File saved as UTF-8 with BOM, body `“G” A\|` (typographic quotes) | truncated before any note: plays nothing, no message |
| L10 | `+p+G +F+A +FFF+B\|` | volumes 60, 105, 127 |
| L11 | `+fermata+G\|` | refused: unknown volume mark |
| L12 | `+mf G\|` | refused: closing plus expected |
| L13 | `G y A\|` | refused: colon expected |
| L14 | `(G A) B\|` | plays G, A, B |
| L15 | `(G A)>B c\|` | refused: unexpected symbol |
| L16 | `"Am" G A\|` (space after the quoted text, at the start of the line) | refused: unexpected symbol |
| L17 | `G "Am" A\|` | plays G, A |
| L18 | `G "Dm A\|⏎B c\|` | truncated: plays G only |
| L19 | `A ~.G\|` | plays A, G (the second round reads `.G`) |
| L20 | `{a} G\|` refused (unexpected symbol); `G {a} A\|` plays G, A | as stated |
| L21 | `~⏎G\|` refused (unexpected symbol); `G ~⏎A\|` plays G, A | as stated |
| M1 | `\|: G A :\| B c\|` | plays G, A, B, c once each |
| M2 | `\|: G \|1 A :\|2 B \|]` | plays G, A, B |
| M3 | `\|: G :: A :\|\|: B :\|` | plays G, A, B |
| M4 | `\|: G :\|: A :\|` | refused: unexpected symbol |
| M5 | `\|: G :\|] A\|` | refused: unexpected symbol |
| M6 | `G [\|] A\|` | refused: unexpected symbol |
| M7 | `G [\|: A :\|` | refused: unexpected symbol |
| M8 | `G \|\|: A :\|` | refused: unexpected symbol |
| M9 | `G \|\|1 A\|` | refused: unexpected symbol |
| M10 | `\|: G :\| \|2 A\|` | refused: unexpected symbol |
| M11 | `G [3 A\|` | refused: chord not closed |
| M12 | `G [1,2 A\|` | refused: unexpected symbol |

<a id="s12-7"></a>
### 12.7 Timing and events

| ID | Tune | Expected |
|---|---|---|
| N1 | Reference tune, `G\|` | G lasts 0.25 s |
| N2 | `M:4/4`, `L:1/4`, `Q:`*q*, one note written with length *q*/1000 (exactly 60 ms on paper) | refused (note too short) for *q* in the list of §13.2, plays for every other *q* from 1 to 240 |
| N3 | `M:4/4`, `L:1/4`, `Q:60`; `G8\|` | plays, G lasts 8.0 s |
| N3b | Same; `G9\|` | refused: note too long |
| N3c | Same; `G8-G8\|` | plays one G lasting 16.0 s |
| N4 | Reference header with `Q:0`; `G\|` | refused: note too long |
| N5 | Reference header with `M:4/0`; `G\|` | refused: note too short |
| N6 | Reference tune `G A B c\|⏎L:1/4⏎d e f g\|` | all eight notes 0.5 s apart (§13.3) |
| O1 | `[G4z] G2 A\|` | events: on G 0; off G 0.25 (extra); on G 0.25; off G 0.75; on A 0.75; off A 1.0 (the first G's own off at 1.0 is dropped) |
| O2 | `+pp+G +mp+A +ff+B\|` | volumes 45, 75, 120 |
| P1 | Reference tune with 10,000 notes `G` | plays (last note-off at 2,500 s) |
| P2 | Reference tune with 10,001 notes `G` | refused (event-limit notice, §10.4) |
| S1 | Any accepted solo tune | first note sounds 0.7 s after the command |

---

<a id="s13"></a>
## 13. Worked examples

<a id="s13-1"></a>
### 13.1 A two-part file

File (one item per line): `% synthetic`, `T:Example set`, `M:3/4`, `L:1/8`, `Q:90`, (blank),
`X:1`, `T:Melody`, `K:D`, `|: d2 fe dc | B4 A2 :|`, `[1 d6 :|2 d4 z2 |]`, (blank), `X:2`,
`T:Bass`, `L:1/4`, `K:D`, `D, A, D, | G,3 |`.

- Title: "Example set" (the first non-empty T:; the parts' own T: lines are ignored).
- M:3/4 resets the unit to 1/8 (3/4 is not below 0.75); L:1/8 keeps it.
- **Request none or 1:** part 1. One eighth lasts 4 × 0.125 × 60 ÷ 90 = 0.3333 s. K:D sharpens
  f and c. Note-ons: 0 d(74), 0.6667 f(78), 1.0 e(76), 1.3333 d(74), 1.6667 c(73),
  2.0 B(71), 3.3333 A(69), 4.0 d(74), 6.0 d(74). The repeat and both endings play once in
  order; the closing rest adds no time; the last note-off is at 7.3333 s.
- **Request 2:** fields before X:1 apply; part 1 is skipped; part 2 sets L:1/4, so a quarter
  lasts 0.6667 s. Note-ons: 0 D,(50), 0.6667 A,(57), 1.3333 D,(50), 2.0 G,(55) lasting
  1.3333 s.

> **OPEN (implementation, 2026-10-07):** `G,3` is three quarters, so by §6.2 and §8.2 it
> lasts 3 × 0.6667 = **2.0 s**, not 1.3333 s (the note-on times above agree with the rules).
> The conformance tests follow the rules (2.0 s). Please confirm or correct the text.
- **Request 3:** refused: part not found.

<a id="s13-2"></a>
### 13.2 The 60 ms boundary

With M:4/4, L:1/4, Q:*q* and a note of written length *q*/1000, the formula of §8.2 gives
1 × (*q*/1000) × 60 ÷ *q*, which is 0.06 on paper. In double precision:

- *q* = 10: 1.0 × 0.01 = 0.01; × 60 = 0.6; ÷ 10 = 0.06 → accepted.
- *q* = 9: 1.0 × 0.009 = 0.009; × 60 = 0.5399999999999999; ÷ 9 = 0.05999999999999999 → refused.

For *q* from 1 to 240 the refused tempos are: 9, 11, 13, 15, 18, 22, 26, 30, 36, 37, 43, 44,
45, 51, 52, 60, 72, 74, 86, 88, 90, 102, 104, 120, 141, 144, 148, 172, 173, 176, 180, 204,
208, 240. A published table of 164 in-game trials of the same kind (MA) agrees with the
formula in every case. That confirms the formula and the limit, and it rules out most other
operation orders, but not all of them:

| Order of the double-precision steps (each step rounded) | Trials not reproduced (of 164) |
|---|---|
| *den* × *L*, × *len*, × 60, ÷ *bpm* (§8.2) | 0 |
| *len* × *L*, × *den*, × 60, ÷ *bpm* | 0 |
| *len* × 60, ÷ *bpm*, × *den*, × *L* | 4 |
| *len* × 60 ÷ (*bpm* ÷ (*den* × *L*)) | 4 |
| 60 ÷ *bpm*, × *den*, × *L*, × *len* | 36 |
| *den* × *L*, × 60, ÷ *bpm*, × *len* | 38 |
| *den* × *L*, × *len*, ÷ *bpm*, × 60 | 42 |
| exact rational value, rounded once | 42 |

The order of §8.2 therefore rests on the executable (BIN); the in-game trials agree with it
but would also allow the second order of the table. The last row shows that the refusals at
60 ms are an effect of the rounding: the exact value is 0.06 for every trial of the table.
(*L* and *len* are themselves doubles rounded from their fractions, §6.2 and §7.5, in every
row but the last.)

<a id="s13-3"></a>
### 13.3 A unit-length field after the music

Reference tune with the body `G A B c|`, then a line `L:1/4`, then `d e f g|`. The last L: of
the part is 1/4, so **every** note lasts 4 × 0.25 × 60 ÷ 120 = 0.5 s: the part lasts 4 s, not
1 s + 2 s. Moving the `L:1/4` line into the header gives the same result.

---

<a id="s14"></a>
## 14. Invariants and hostile input

<a id="s14-1"></a>
### 14.1 Invariants

1. **INV1:** every note-on of an accepted part has a pitch from 48 to 84 and a slot from 1 to 37.
2. **INV2:** every duration computed for an accepted part (§8.2) is ≥ 0.06 and ≤ 8.0.
3. **INV3:** an accepted part has at most 10,000 events, and no chord with more than 6
   entries after duplicates are dropped.
4. **INV4:** a refused part produces no events (all or nothing).
5. **INV5:** in an accepted part, removing comments, blank lines, white-space-only lines,
   ignored field lines, chord symbols and grace groups written directly before a note, slurs,
   the decorations `~ . u v` directly before a note, and the ending markers `[1` `[2` never
   changes the events. (`|1` and `:|2` cannot be replaced by a plain bar: a bar clears
   accidentals and they do not.)
6. **INV6:** inserting a bar line between two elements outside chords and tuplets changes
   only pitches that depended on a remembered accidental; times never change.
7. **INV7:** moving an M:, L: or Q: line within a part changes no time as long as the final
   meter denominator, unit length and tempo stay the same (§8.1) and no tuplet with a default
   ratio sees a different meter numerator.
8. **INV8:** lower-casing a field letter changes nothing.
9. **INV9:** after §9.3, note-ons and note-offs of each slot alternate, starting with a
   note-on; the end event comes last.
10. **INV10:** event times are non-decreasing in output order (deltas ≥ 0) except after an
    empty chord, which can make them decrease (§8.4).
11. **INV11:** take a file whose lines before the first X: line are fields only, and whose X:
    values are distinct decimal numbers without leading zeros. Reading it with request *k*
    gives the same events as reading, with no request, a file made of those first lines
    followed by part *k* (from its X: line to the line before the next X: line).

<a id="s14-2"></a>
### 14.2 Hostile-input notes

- Bound the file size and the lengths of white-space runs and lines. Digit runs of any length
  are accepted (§4.1): the original copies them into a 128-byte buffer without a check, so a
  run of 128 or more characters is undefined behaviour there (probably a crash); Tessera
  instead converts the whole run without a fixed buffer, saturating at 2,147,483,647, and does
  not refuse the part for the length alone. Shorter runs saturate in the original too. Numbers
  must not overflow when multiplied (use doubles, §8.2).
- Characters other than `0`–`9` are never digits in Tessera (§4.1 OPEN).
- Backslash chains and comment skipping must not recurse without bound.
- Unclosed quotes and grace groups scan to the end of the file (§7.10); this must stay linear.
- The event limit (10,000) bounds the event list; chords add at most 6 entries each.
- Durations can be infinite or NaN (tempo 0, unit length 0 with tempo 0); comparisons must
  follow §8.3 (an infinite duration is "too long"; a NaN duration fails neither comparison
  and is accepted by the original, but plays no meaningful sound — Tessera may refuse it).
- An empty chord moves time backwards (§8.4); the audio scheduler must accept negative times
  or clamp them, without changing the accept/refuse result.

---

<a id="s15"></a>
## 15. Differences from ABC 2.1 and from published models

<a id="s15-1"></a>
### 15.1 Main differences from ABC 2.1

| Topic | ABC 2.1 | This dialect |
|---|---|---|
| Repeats and endings | played as written | never repeated; everything plays once in order |
| Tempo Q: | beat = the note length in Q: | beat = 1/(meter denominator); note length ignored |
| Unit length | L:, else from M: | the last M:/L:; every M: resets L; applies to the whole part |
| `c3/` | 3/2 | 3 |
| Mode `m` | minor | ignored (major); minor needs `min` |
| Accidentals | to the bar, same pitch | to the bar, same letter and octave; ending markers do not clear |
| Ties | to the next note | to the next note of the same pitch anywhere later |
| Chords | length suffix allowed; duration of the first note | no suffix; duration of the shortest entry; duplicates dropped |
| Broken rhythm with chords | allowed | only before a chord (affects its first entry) |
| Tuplets | spaces allowed | no spaces between the notes |
| Decorations | `!…!`, letters, `+…+` | only `~ . u v` (ignored) and volume marks; `!` truncates |
| Inline fields `[K:…]` | allowed | refused (chord not closed) |
| Voices, part order, lyrics | supported | ignored |
| Free text | allowed between tunes | refused unless skipped |
| Line continuation `\` | joins lines | dropped; lines stay separate |
| `\%` | literal percent | still a comment |

<a id="s15-2"></a>
### 15.2 Published reports that the executable contradicts

Maestro (MA) records the results of in-game trials and models the client on them. Most agree
with the executable. These do not; the spec follows the executable, and §17 proposes owner
checks:

| Report (MA) | Executable (this spec) |
|---|---|
| `K:G ^c` and keys with extra words such as `K:B` + a two-letter word are refused | accepted: explicit accidentals work (§6.4 step 6); words of fewer than three letters are ignored |
| The letters `u` and `v` before a note are refused | accepted and ignored (§7.10); the reported trial combined them with `T` and `H`, which are refused |
| A triplet whose plain notes would last 0.0625 s plays although each tuplet note lasts 0.042 s; the 60 ms check uses the written length | the check uses the length after the tuplet factor (§7.9 step 5, §8.2); such a part is refused |
| An L: line after notes changes nothing; the later notes keep the header's unit | the last L: applies to every note of the part, before and after it (§8.1); a trial that hears "no change between the halves" is consistent with this |
| A mode written `m` (as in `K:Am`) is minor (modelled, not a reported trial) | major (§6.4 step 4) |
| An M: in a header does not override an L: given before it in the same header (modelled) | every M: resets the unit length (§6.1) |
| A still-sounding pitch struck again with another volume makes the part silent | not refused by the reader (§9.3 OPEN) |

The numbering also differs: MA numbers the game's notes 12 lower than the pitch numbers
here (its range 36–72 is this spec's 48–84).

---

<a id="s16"></a>
## 16. Corrections to other specs and earlier reports

These rest on the disassembly of client build 4906.0071.1794.4035 (BIN). They are recorded
here, not applied by editing the other files, because this session owns only this file.

| Target | Current text | Correction | Basis |
|---|---|---|---|
| `install-layout.md` P23 and the settings-folder table (`Music/*.abc`, Reported) | Music files are `Music/*.abc` | The folder name `Music` is Confirmed (BIN). The client also accepts `.txt`: a name is tried with `.abc` first, then `.txt` (§3.1). | BIN |
| `string-table.md` §9.1 (named tables) | Named tables described in general | Example in use: key 0x10000069 (label `UI_Music`) → table 0x2500808A holds the player-music messages, with argument IDs for line, column, file name and title (§10.3–§10.4). An addition, not a change. | BIN + OBS |
| `research/notes/engine-client-re.md` §7 (ER) | "LOTRO plays repeats `\|: :\|` and `[1 [2`" | Accepted but never repeated: everything plays once in written order (§7.2). | BIN |
| ER §7 | "Shortest note 60 ms … strange BPMs (9, 11, 13, 15, 37, 43, 45, 51 and those values doubled repeatedly)" | The rule is the exact double-precision formula of §8.2–§8.3; which tempos fail depends on the unit and note lengths too, and the doubling description is incomplete (for example 141 and 173 also fail in the family of §13.2). | BIN + MA |
| ER §7 | Refused or ignored: "a length or tie after a chord, broken rhythm next to a chord, an empty K: or text after the key, … text in Q:, an L: after notes, T/H/L/M/O/P/S/u/v" | Broken rhythm **before** a chord is accepted (§7.8); text after the key is accepted unless it begins with three letters that are not a mode (§6.4); text in Q: is refused only before the tempo (§6.3); an L: after notes is accepted and applies to the whole part (§8.1); `u` and `v` are accepted (§7.10); `!…!`, `$` and `` ` `` truncate rather than refuse (§7.1). | BIN |
| `instruments.md` §5.1 and §5.2 (packed note: "bits 0–23" vs "low byte" as the note key) | Two descriptions of the note key | Both hold for the same word: the slot is packed into bits 0–23 (§9.4), the re-attack pass compares the low 8 bits, and slots are 1–37, so the two are always equal. The tie bookkeeping uses the slot value itself. An addition for the `instruments.md` writer to adopt, not an edit. | BIN |
| `docs/ARCHITECTURE.md` §14 (neutral doc) | Dialect "Reported in engine-client-re.md §7" | Now specified here from the executable. | — |

---

<a id="s17"></a>
## 17. Open questions

1. Name with an extension: kept or replaced (§3.1).
2. Characters for bytes 0x80–0xFF in files without a BOM; whether 0x1A ends input (§3.3).
3. Classification of stray UTF-8 continuation bytes (§3.3).
4. Part number passed when the command omits it; `/playlist` filter syntax and sub-folders;
   source of the synced start time (§11).
5. Source of the event-limit notice (§10.4).
6. Volume-to-loudness curve; release; instrument octaves (§7.11; `instruments.md`,
   `sound.md`). The meaning of the pitch slots is now settled (§9.5: keys of the note mapper);
   left open there only: a possible future reordering of the mapper's keys against the
   client's fixed table.
7. Re-attack with a different volume: silence reported in game, not refused by the reader
   (§9.3).
8. Which non-ASCII characters the original's digit test accepts, and therefore how runs with
   superscript, full-width or other-script digits read (§4.1). Tessera accepts only `0`–`9`
   meanwhile.
9. The 0.5 s constant of the original's performance stepper (§8.5): catch-up for late
   playback, to be specified with `sound.md` / `instruments.md`.

**Proposed owner checks** (method M3, own account; to be added to
`docs/checklists/owner-observations.md` by its maintainer). Each needs a small file in the
`Music` folder, `/play`, and listening; expected results are those of this spec:

| ID | File (reference header unless stated) | This spec predicts | MA predicts |
|---|---|---|---|
| OC1 | `K:Am`; `c c c c\|` | four C♯ (A major) | four C |
| OC2 | `K:G ^c`; `c c c c\|` | plays C♯ | refused |
| OC3 | `K:B ab`; `B B B B\|` | plays | refused |
| OC4 | `Q:100`; `(3G/4A/4B/4 c4\|` | refused (too short) | plays |
| OC5 | `G A B c\|⏎L:1/4⏎d e f g\|`, timed with a stopwatch | about 4 s | about 3 s |
| OC6 | `[c4z] +ff+c2 d2\|` | plays (cut first c) | plays nothing |
| OC7 | `uG vA uB vc\|` | plays | refused |
| OC8 | Header `L:1/4` before `M:4/4`; `G A B c\|`, timed | about 1 s | about 2 s |
| OC9 | Saved as UTF-8 with BOM; body `G2² A\|` (superscript two after the 2) | G (0.5 s) then A if U+00B2 is a digit (run value 2); G alone if it is not (illegal character, truncated) | — |
| OC10 | Saved as UTF-8 with BOM; body `G２ A\|` (full-width two, U+FF12) | refused (note too short: the run's low byte 0x12 gives value 0) if U+FF12 is a digit; G alone if it is not | — |

OC9 and OC10 settle the OPEN of §4.1 (a refusal with "colon expected" instead would mean the
character counts as a letter). Do **not** try a run of 128 or more digits in the
original: its behaviour is undefined and may crash the client (§4.1).

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version: file lookup and decoding, tokens, fields and part selection, field values, music elements, timing rule and limits, note events and pitch slots, errors and message entries, commands, conformance table, worked examples, invariants, differences and corrections. |
| 2026-10-05 | Revision after review (changes requested). §9.5: the OPEN about the slots is answered — each slot is the key of a `Note_` label in enum mapper 0x2300015E (all 37 checked); the live-play action names are cited from enum mapper 0x230000AF (37 names), the owner's key-mapping file is cited only for its 21 bound actions, and the link from ABC `C,` to `MUSIC_C2` is marked as an inference (new PV25b; OBS row extended). §13.2 and PV23: the in-game trials confirm the formula and rule out most operation orders, not the order itself; table of eight orders added; the order rests on BIN (new PV23b). New §4.1 (number tokens): 128-byte buffer without a length check, undefined behaviour for runs of 128 or more characters, Tessera's choice (convert the whole run, saturate), low byte kept, digit classification OPEN (new PV4b); §4 table row, §14.2 and §17 adjusted; rows F6–F8 and owner checks OC9–OC10 added. §9.4: packed word layout completed (slot bits 0–23, volume bits 24–31, which part is compared). §8.5: OPEN pointer to the 0.5 s constant of the performance stepper. MA trial labels prefixed (MA B6 …) so they no longer clash with §12 row IDs. §16: note on the packed-word wording of `instruments.md` §5.1/§5.2. |
