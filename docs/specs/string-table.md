<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# String tables: message semantics, grammar tags and resolution

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested. Blocking: M4 facts in §9.2 (redirect DID), §9.4 (float formatting) and §9.5 (character-name tag group, vowel set) have no provenance row; the 0x58 record-to-field mapping (§10.3, singular-flag order) is Inferred with no OPEN; wrong Confirmed counts in §4.9 (only 1 entry is exactly `[E]`, not 12,081; 147 groups have a space before `[`, not 10), §4.7 (colour openers/closers 13,951/13,942 case-insensitive, `</p>` occurs 0 times) and §4.3 (6,843 labelled selectors). Independent re-implementation of §4.10/§7.3 parses and validates all 806,052 entries; label, variable, modifier, selector, escape, whitespace, tag-letter, locale-record and DID-map spot checks all matched. Re-review 2026-10-05 (Claude review session, spec-writer agent): **accepted**. Every blocking, minor and gap item of the first review is resolved: G34–G42 added, §10.3 field order cited to BIN2, §4.9/§4.7/§4.3/§7.6 counts re-checked over all 806,052 entries and all match (202,210 groups, 12,081 exactly `[E]`, 1 entry `[E]` alone, 147 space before `[`, 21 after `]`, 4 doubled `[[`; colour 13,951/13,942 in 11,677 entries, `</p>` 0; 6,843 of 7,057 selectors labelled; options 15/4/1/2; 26 entries with an unescaped `[`, 22 losing text); locale record field values and SS1/SS2, SG14 (`token_high` 0, name byte 0 or 1 in all entries), the f32 bias value, the 26-character vowel set (one 0x41–0xFC lookup table in the executable) and all §9.4.2 float examples were reproduced independently. Non-blocking notes for a later revision: §9.4.2 "the larger of 1 and *w*" should say that a NaN *w* stays the limit (the stated NaN consequence depends on it); the client adds 0.5×*r*^−*d* and *b* before adding the absolute value of *v*; label digit strings beyond 32 bits are unspecified; §7.2 versus §4.5.2 on whether pre-decoding of prefix/suffix values needs the `item` directive. |
| **Implements into** | `crates/text` (message model, inline grammar, tag selection, substitution, whitespace normalisation, display cut, escape decoding, StringInfo resolution, number formatting, locale selection, UI-text markup); `apps/explorer` string-table view |
| **Companion specs** | `docs/specs/records.md` §1.4–1.5 (UTF-16 strings, name hash), [§3](records.md#s3) (string-table **byte layout**), [§4](records.md#s4) (StringInfo **byte layout**), §8 (DID maps); `docs/specs/install-layout.md` §3.5 (language archives); `docs/specs/dat-container.md` §3.4 (subset word) |

---

<a id="s0"></a>
## 0. Purpose and scope

`records.md` §3 says how to turn the bytes of a 0x25 record into entries made of
**pieces**, **argument IDs** and optional **argument names**. This spec starts where that
one ends. It defines what the text inside those pieces means and how to turn an entry plus
a set of argument values into the text a player would see, the way the original client
does it:

- the message model: pieces, argument slots, argument IDs and names (§3);
- the inline grammar inside pieces: variables with labels and modifiers, selectors,
  directives, escapes, markup and value tag groups (§4), with the complete token and item
  rules (§4.10);
- the grammar-tag alphabet, how a value's tags are derived, and the option-selection rule
  (§5, §6);
- the end-to-end rendering pipeline: template assembly, parsing, validation, evaluation,
  whitespace normalisation, the display cut, escape decoding and UI-text markup (§7);
- the `${VAR}` text presentation and the substitution interface Tessera exposes (§8);
- how a StringInfo value (table DID + token, optional arguments) resolves to a rendered
  string, what happens on every kind of failure, and how numeric arguments are formatted
  (§9);
- how the locale archive is chosen, what the subset word means, and the per-locale grammar
  and number-format record (§10);
- whole-population statistics (§11), invariants (§12), discrepancies (§13), open questions
  (§14), test guidance (§15), worked examples (§16), corrections to other specs (§17) and
  the change log (§18).

The byte layout of string tables is **not** repeated here. Every example in this spec is
synthetic and was written for this document. None is game text.

---

<a id="s1"></a>
## 1. Provenance

Marks:

- **Confirmed**: measured on the user's own install by a throwaway probe in the dirty-room
  scratchpad. All 282,152 string tables (806,052 entries) in `client_local_English.dat`
  were decoded and every entry was scanned (2026-10-04, re-run 2026-10-05). Counts are
  given.
- **Confirmed (BIN)**: read from the client executable under decision 0011 (method M4); see
  the BIN and BIN2 rows. Where the data was also checked against the rule, the mark is
  "Confirmed (BIN + OBS)". These facts rest on the executable alone where no OBS is listed.
- **Reported**: comes from published third-party material only, and neither the data nor
  the executable confirms or refutes it.
- **Inferred**: deduced by the spec writer. It fits every observed case but has not been
  read from the executable. Inferred facts get an OPEN callout when getting them wrong
  would change what the player sees.

Sources:

| Key | Source |
|---|---|
| OBS | Observed in the user's install 2026-10-04 and 2026-10-05 (Steam install, English client data current on those dates). Data was read in place and read-only. |
| BIN | Client executable `x64/lotroclient64.exe`, build 4906.0071.1794.4035 (changelist 711794; SHA-256 prefix d22363eb9aff0fb5, the same build as `records.md` BIN), read with a disassembler (objdump) on 2026-10-05, on address ranges only. **Questions:** (1) the token and item grammar inside pieces (labels, selectors, modifiers, directives, escapes); (2) how a value's grammar tags are derived and how a selector option is chosen; (3) whitespace handling and what `{{keepspaces}}` switches off; (4) what is displayed when a lookup or a render fails; (5) how UI text interprets colour, emphasis, list and link markup; (6) how numeric arguments are formatted, and what the 0x58 record of the language archive is. **Why M1–M3 were not enough:** the data holds the templates but not the rules that interpret them; the published readers (LC, LCC) disagree with each other and with the data on the selection rule, on tag meanings and on `\q`; the original client cannot be observed running here (M3), and no published source describes failure display, whitespace or number formatting. Only the offline text-formatting code was read: the message template lexer, parser and evaluator, the string-table and StringInfo renderers, the number formatters and the UI-text markup tokenizer. Nothing about network, login, keys, launcher or anti-tamper was read. |
| BIN2 | Second M4 pass on the same build and the same day (2026-10-05), made to answer the review of this spec. **Questions:** (7) which record field of the 0x58 record fills which field of the loaded locale object, in particular the order of the five singular flags, of the three item-composition sets and of the `M`/`F` expansions; (8) whether the `token_high` u32 of a string-table entry is the redirect DID the renderer follows; (9) the exact float-formatting arithmetic (width, rounding bias, digit extraction, limits); (10) the character-name tag group and its vowel set; (11) whether an escaped `\]` closes a selector tag list; (12) the integer type used when a value is tested for `1` and `+`; (13) how the client reads the name-group byte of an entry. **Why M1–M3 were not enough:** in English every one of these fields equals a built-in default or is never exercised (no redirect, no float argument, no character name, no escaped `]` in a tag list, no out-of-range number in the data), so observation cannot tell the orders or limits apart, and no published source describes them. Only offline code was read: the locale-record serialiser and its field getters, the string-table entry serialiser and lookup, the StringInfo renderer, the integer and float formatters, the value-tag derivation, the item-composition step, the character-name builder and the selector tag-list scanner. Nothing about network, login, keys, launcher or anti-tamper was read. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0 (Apache-2.0 distribution), studied as compiled classes for facts only: string-table loader, string renderer, tag list, locale file manager. |
| LCC | LotroCompanion `lotro-core` (no licence file), facts only: text sanitizer and HTML conversion helpers. |
| API | The plugin-API enumeration dump summarised in `docs/specs/lua-plugin-api.md` Appendix A (`Turbine.Language`). |
| RS | `docs/specs/records.md` (this project), whose layouts are Confirmed there. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| G1 | Entry = pieces alternating with argument slots; slot *i* takes the value of `arg_ids[i]` | RS, LC; OBS; BIN (template assembly, §7.2) | Confirmed (BIN + OBS) |
| G2 | Stored text never contains `${`. The `${NAME}` form is a tool presentation, not data. | OBS (0 of 806,052 entries); LC renders `${…}` itself | Confirmed |
| G3 | **Label** `#N:` before a slot (or before `^`, `@`, `{`) names that variable or selector *N*; spaces and tabs are allowed around `-`, the digits and `:` | BIN (lexer); OBS (all 806,052 entries parse, §12 SG11) | Confirmed (BIN + OBS) |
| G4 | **Selector** `#N:{option\|option\|…}` and unlabelled `{…}`; option = text followed by an optional `[tags]` list | BIN (grammar); OBS | Confirmed (BIN + OBS) |
| G5 | Every character of an option's tag list is one tag; `!` negates the next character; tags are compared case-sensitively against the value's tag string | BIN (selection) | Confirmed (BIN) |
| G6 | Option choice: 2 points per satisfied tag, 1 point for an option without tags; the highest score above 0 wins; on a tie the **later** option wins | BIN | Confirmed (BIN) |
| G7 | `E` is an ordinary letter carried by data; the client never derives it | BIN (value tag derivation); OBS (12,081 single-piece entries end with exactly `[E]`) | Confirmed (BIN + OBS) |
| G8 | Derived tags: `b` for an empty value without a tag group; `+` for a value that is entirely a non-negative integer; `1` for an integer the locale record calls singular | BIN | Confirmed (BIN) |
| G9 | A value's own tag group: display text = text before its first unescaped `[`, trailing whitespace removed; tags = the characters up to the first unescaped `]`, spaces dropped, `M` and `F` expanded | BIN; OBS (202,210 entries end with a group) | Confirmed (BIN + OBS) |
| G10 | Escapes `\n` `\q` `\r` `\t` and `\` + one of `[ ] ! { } # \ \| ^ $ @`; the backslash protects the character from the grammar; decoding happens once, at display time | BIN (lexer and decoder); OBS (counts §4.6) | Confirmed (BIN + OBS) |
| G11 | Colour markup `<rgb=…>…</rgb>` with a colour stack; alpha forced to opaque | BIN (UI-text markup); OBS (13,951 opening tags, case-insensitive) | Confirmed (BIN + OBS) |
| G12 | Eight-digit colour values: the top byte is overwritten with 0xFF, so it is never an alpha | BIN | Confirmed (BIN) |
| G13 | `<b>` `<i>` `<u>` `<p>` `<li>` (any case) and closers; `<<>` is a literal `<` | BIN; OBS | Confirmed (BIN + OBS) |
| G14 | Link markup `<Kind:Type:…>label<\Kind>` is recognised by a registry of link kinds; semantics per kind OPEN | BIN (tokenizer path); OBS (27 entries, 9 kinds) | Confirmed (recognition path), meaning OPEN |
| G15 | `{{word}}` is a directive; it renders nothing. `keepspaces` turns off whitespace normalisation; `item` turns on item-name tag composition | BIN; OBS (37 entries use `keepspaces`, none use `item`) | Confirmed (BIN + OBS) |
| G16 | Modifier `^` capitalises the variable's first letter; `@` lowercases it; either may follow a label or stand alone | BIN; OBS (13 `^`, 0 `@`) | Confirmed (BIN + OBS) |
| G17 | An unlabelled selector binds to the unlabelled variable, else to the variable labelled −1 | BIN; OBS (203 + 11 selectors) | Confirmed (BIN + OBS) |
| G18 | StringInfo resolution: 5,890 of 6,348 enum display references resolve; all 458 misses are a missing token in a table that exists | OBS | Confirmed |
| G19 | No on-disk StringInfo that points at a real (table, token) has an argument block | OBS (heuristic scan of `gamelogic`, `general`, `local_English`: about 1.02 million references, all `has_args` = 0) | Confirmed |
| G20 | Local archive subset word 0x150 = 2, which is the plugin API's English language value | OBS (header); API (meaning) | Confirmed (value), Reported (meaning) |
| G21 | Named string tables: DID map 0x28000004 has 179 keys; 177 point at existing 0x25 tables; key 0x0B ("Language") points at the 0x58 record | OBS | Confirmed |
| G22 | Variables with a **negative** label are not displayed; they exist only for selectors | BIN; OBS (245 such variables) | Confirmed (BIN + OBS) |
| G23 | Whitespace normalisation: leading whitespace removed, every run of spaces and tabs reduced to its first character; trailing whitespace kept | BIN | Confirmed (BIN) |
| G24 | Display cut: the rendered text is cut at its first unescaped `[`, then trailing whitespace is removed | BIN | Confirmed (BIN) |
| G25 | A message with any slot whose argument has no value is not rendered (reason 6) | BIN | Confirmed (BIN) |
| G26 | Failure reasons 1–7 and the diagnostic text the client shows (§9.3) | BIN | Confirmed (BIN) |
| G27 | `PLAYERNAME` (hash 0x0DA47675) is supplied by the client when the caller does not supply it; the client stores it by rendering, in raw mode, a watched property of the local player's object | BIN; BIN2 (the setter) | Confirmed (BIN); which property is OPEN, its value Inferred |
| G28 | Nested StringInfo arguments are rendered raw (tag group kept, escapes not decoded) | BIN | Confirmed (BIN) |
| G29 | The 0x58 record (DB type STRING_STATE) holds the locale's number format, singular rule, `M`/`F` expansions and item-composition tag sets; layout §10.3 | BIN (field use); BIN2 (the record serialiser: field order, types and the object field each one fills); OBS (record parses exactly, 101 bytes); the type name is the label of DB type 0x46 in the data's DBOType enum mapper (`records.md` §2.1, provenance D2) | Confirmed (BIN + OBS) structure and record-to-field match; Confirmed (BIN) use of each field; three fields' meaning OPEN |
| G30 | Integer arguments are printed in base 10 with the record's digit grouping and negative template | BIN; OBS (record values) | Confirmed (BIN + OBS) |
| G31 | Literal StringInfo values are not parsed by the grammar; they get only the display cut and escape decoding | BIN | Confirmed (BIN) |
| G32 | Entry argument names matter only for item-name composition (names `PREFIX`/`PRE`, `MID`/`NAME`/`PLURALNAME`, `SUFFIX`/`POST`) | BIN; OBS (the 7 named entries use `NAME` only) | Confirmed (BIN + OBS) |
| G33 | Markup is interpreted only by UI text elements that enable it; unknown or malformed tags are shown literally | BIN | Confirmed (BIN) |
| G34 | An entry's second u32 (`records.md` §3.2 `token_high`) is a redirect DID: when it is not the null DID the renderer loads that record and looks the token up there; reason 5 when the record cannot be loaded, reason 4 when the token is missing from it (§9.2) | BIN2 (entry serialiser: the u32 after the token fills the field the lookup compares with the null DID; renderer: the two failure codes); OBS (the field is 0 in all 806,052 entries, so the path is never taken) | Confirmed (BIN); never exercised by the data |
| G35 | Float arguments: double-precision arithmetic, rounding bias of half a unit of the last place plus an absolute 0.00019999999494757503, digits produced from the last place upward with the low 32 bits of each truncated product, at least one integer digit, limits on places, radix and length (§9.4) | BIN2 (float formatter and its callers) | Confirmed (BIN) |
| G36 | Integer arguments: unsigned 64-bit digit loop over the absolute value, the record's padding, grouping and negative template; empty text without a record, for a radix ≤ 1 or for a digit alphabet shorter than the radix (§9.4) | BIN; BIN2 (integer formatter limits) | Confirmed (BIN) |
| G37 | Character-name values: display cut, trim at both ends, then a space and a tag group of `M`/`F`, `v` when the first character (after one optional `+`) is in a 26-character vowel set, and `s` when the lower-cased last character is `s` (§9.5) | BIN2 (character-name builder and its vowel test) | Confirmed (BIN) |
| G38 | Record field order of the 0x58 record: singular flags in the order *n* = 0, 1, −1, > 1, < −1; item-composition sets in the order prefix, name, suffix; then the `M` and then the `F` expansion (§10.3) | BIN2 (record serialiser; the singular test; the item-composition step; the value-tag derivation) | Confirmed (BIN) |
| G39 | A selector tag list ends at the first `]` after its `[` whatever precedes it: an escaped `\]` closes the list, and its backslash becomes a tag character (§4.3 rule 2, §4.10 token 5) | BIN2 (tag-list scanner) | Confirmed (BIN) |
| G40 | The integer test of §5.3 is the C library's `wcstol` in base 10 into a signed 32-bit integer: out-of-range digit strings saturate to −2,147,483,648 or 2,147,483,647 and still count as integers | BIN2 (value-tag derivation; the import it calls) | Confirmed (BIN) |
| G41 | The entry byte before the argument-name group is a flag: 0 = no group, 1 = one group; any other value is a stream error in the client (§17) | BIN2 (entry serialiser); OBS (every entry has 0 or 1) | Confirmed (BIN + OBS) |
| G42 | Census corrections of 2026-10-05: exactly-`[E]` entries (1), groups with a space before `[` (147), colour tags (13,951 / 13,942 in 11,677 entries), `</p>` (0), labelled selectors (6,843 of 7,057), options with several tags (22), entries with an unescaped `[` outside the trailing group (26, of which 22 lose visible text to the display cut) | OBS (re-run over all 806,052 entries) | Confirmed |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Table** | One 0x25 record (`records.md` §3.1). Identified by its DID. |
| **Token** | The u32 key of an entry inside a table (`records.md` §3.2). |
| **Entry** / **message** | One token's content: *p* pieces, *a* = max(*p* − 1, 0) argument IDs, optional argument names. |
| **Piece** | A UTF-16 literal string stored in the entry. Pieces may contain inline grammar (§4). |
| **Slot** | The position between piece *i* and piece *i*+1, 0-based *i*. Slot *i* is filled with the value of argument `arg_ids[i]`. |
| **Argument ID** | A 28-bit name hash (`records.md` §1.5) of the argument's **variable name**. |
| **Value** | The text that fills a slot at render time. Numbers and nested messages are turned into text first (§9.4, §9.2). |
| **Template** | The single string the client builds from an entry and its values (§7.2): pieces with each value framed by two U+0001 characters. |
| **Variable** | The grammar item made by one slot, with its optional label and modifier. |
| **Raw text** | Text as stored, before escapes are decoded and markup is interpreted. |
| **Tag group** | A `[letters]` group in a value's raw text that describes its grammar (gender, number, article behaviour, class). |
| **Tag string** | The characters a value carries for selection: its tag group (with `M` and `F` expanded) or its derived tags (§5.3). |
| **Label** | `#N:` before a variable or selector. *N* is a signed integer local to the entry. 0 means "unlabelled". |
| **Selector** | `{…}`, optionally labelled: picks one of several texts based on the tag string of the variable it binds to. |
| **Directive** | `{{word}}` at top level. |
| **Display mode / raw mode** | Whether the display cut (§7.6) is applied to a rendered StringInfo. |

---

<a id="s3"></a>
## 3. Message model

<a id="s3-1"></a>
### 3.1 Pieces and slots (Confirmed)

An entry renders as piece 0, slot 0, piece 1, slot 1, …, piece *p*−1. The byte layout
guarantees *a* = max(*p* − 1, 0) (records.md ST3). An entry with one piece has no slots.

- A message that starts with a value has an empty piece 0; one that ends with a value has
  an empty last piece.
- Observed *p* ranges from **1** to 12. No entry has *p* = 0 (see §13, D1). A renderer
  must still accept *p* = 0 and render it as the empty string.
- 12,156 entries are a single empty piece. They render as the empty string. This is a valid
  message, not a lookup miss.

<a id="s3-2"></a>
### 3.2 Argument IDs (Confirmed)

- Slot *i* takes the value supplied for `arg_ids[i]`. The **same ID may fill several
  slots**: 1,709 entries repeat an ID. Each occurrence gets the same value.
- 804 distinct IDs occur across 80,821 slot uses. The three most frequent IDs account for
  about 67 % of all uses.
- An ID is a hash. Recovering a readable variable name requires a candidate name list
  (§8.2). The data rarely stores names.

<a id="s3-3"></a>
### 3.3 Argument-name groups (Confirmed; role Confirmed (BIN))

The optional group after the argument IDs (records.md §3.3) lists the variable names in
slot order. Only **7 entries** carry one. In each of them, every name hashes to the
matching ID (records.md ST4). All 7 have two slots, both named `NAME`.

The client reads entry names for one purpose only, **item-name composition** (§4.5.2). It
compares each name exactly (case-sensitive) with three role lists; the names are
interoperability identifiers:

| Role | Names |
|---|---|
| prefix | `PREFIX`, `PRE` |
| name | `MID`, `NAME`, `PLURALNAME` |
| suffix | `SUFFIX`, `POST` |

Composition happens only when the entry has a name-role variable and at least one prefix-
or suffix-role variable. None of the 7 named entries qualifies, so in this install names
change nothing. Otherwise names only give the explorer readable names for that entry.

---

<a id="s4"></a>
## 4. Inline grammar inside pieces

Pieces are not plain text. The client assembles the pieces and the values into one
template string (§7.2) and runs a lexer and parser over it. §4.10 gives the complete token
and item rules; §4.1–§4.9 describe each construct.

<a id="s4-1"></a>
### 4.1 Overview of constructs

| Construct | Shape (abstract) | Where | Entries | Mark |
|---|---|---|---|---|
| Variable | a slot, optionally preceded by a label and/or a modifier | §4.2 | 52,328 | Confirmed (BIN + OBS) |
| Label | `#` signed integer `:` | §4.2 | 3,643 | Confirmed (BIN + OBS) |
| Modifier | `^` or `@` directly before a slot | §4.2.1, §4.2.2 | 13 (`^`) | Confirmed (BIN + OBS) |
| Selector | [label] `{` option (`\|` option)* `}` | §4.3, §4.4 | 3,807 | Confirmed (BIN + OBS) |
| Directive | `{{` word `}}` | §4.5 | 37 | Confirmed (BIN + OBS) |
| Escape | `\` + one character | §4.6 | 81,000+ | Confirmed (BIN + OBS) |
| Colour markup | `<rgb=` value `>` … `</rgb>` | §4.7 | 11,677 | Confirmed (BIN + OBS) |
| Emphasis / paragraph / list markup | `<b>` `<i>` `<u>` `<p>` `<li>` and closers | §4.7 | 1,236 | Confirmed (BIN + OBS) |
| Link markup | `<` Kind `:` Type `:` … `>` label `<\` Kind `>` | §4.8 | 27 | Confirmed (recognition) |
| Value tag group | `[` tags `]` inside a value | §4.9 | 202,210 | Confirmed (BIN + OBS) |

Markup (§4.7, §4.8) is **not** part of the message grammar. It is interpreted later, by UI
text, on the finished and escape-decoded string (§7.8).

<a id="s4-2"></a>
### 4.2 Variables and labels (Confirmed (BIN + OBS))

Each slot becomes one **variable**. A **label** in front of it names it:

- A label is `#`, optional spaces or tabs, an optional `-` (again followed by optional
  spaces or tabs), one or more decimal digits, optional spaces or tabs, `:` and optional
  spaces or tabs. The digits are read as a signed decimal integer *N*.
- It is a label **only** if the next character after it is the start of a slot, `^`, `@`
  or `{`. Otherwise the `#` and what follows are ordinary text.
- Labels are recognised only at top level, not inside a selector.
- The label text is **not** rendered.
- *N* is an identifier local to the entry. It is **not** the slot index: in partly
  labelled entries, slot 2 may carry label 1. Label 0 (`#0:`) is the same as no label.
- Observed labels on variables (census 2026-10-05): 1 (3,403), 2 (3,165), 3 (3,105), 4 (10),
  5 (4), 6 (1), 7 (1), −1 (238), −2 (3), −3 (1), −4 (3). 70,887 variables are unlabelled.
- Not every slot has a label. Only slots that a selector refers to need one. In 3,458
  entries every slot is labelled, and in 185 only some are. Where every slot is labelled,
  label *N* sits on slot *N*−1 in 3,271 of the 3,458 entries.
- Within an entry, two variables never carry the same non-zero label (the client rejects
  that, §7.3).

**Hidden variables.** A variable whose label is **negative** is not displayed. It renders
as nothing and exists only so that selectors can read its tags. 245 variables in the data
are hidden. A typical shape: a count slot labelled −1 with a selector `#-1:{…[1]|…}` that
picks a singular or plural word, while the number itself is shown elsewhere by the UI.

Example (synthetic). Pieces `"You hand the lamp to #1:"` and `". #1:{She[f]|He[m]|They} thanks you."`, one argument. The first piece's trailing `#1:` labels the variable of slot 0 as 1. The selector in piece 1 reads its tags.

<a id="s4-2-1"></a>
#### 4.2.1 The `^` modifier (capitalise)

`^` directly before a slot (after an optional label, `#N:^`, or on its own) capitalises the
value:

1. Starting at the first character of the value's display text (§5.3), skip characters
   that are whitespace or punctuation, and the two characters U+00A1 and U+00BF. Stop
   skipping at a backslash, `/` or `@` even though they are punctuation.
2. Convert the character where skipping stopped to upper case (simple Unicode mapping).
   If skipping reached the end, nothing changes.

Whitespace and punctuation follow the Unicode general categories as the Windows C runtime
classifies them (`iswspace`, `iswpunct`). Observed: 5 labelled `^` and 8 bare `^` (13 in
total), always directly before a slot. A `^` that is not directly followed by a slot makes
the message fail to parse (reason 7, §9.3); none occurs in the data.

<a id="s4-2-2"></a>
#### 4.2.2 The `@` modifier (lower case)

`@` in the same positions as `^` converts every character of the value's display text to
lower case (simple Unicode mapping), except whitespace and punctuation, which stay as they
are. No entry in this install uses `@`; a literal `@` in prose must be written `\@` (1
entry does).

<a id="s4-3"></a>
### 4.3 Selectors (Confirmed (BIN + OBS))

A **selector** is an optional label, `{`, one or more options separated by `|`, and `}`.
It may appear anywhere at top level, **before or after** the variable it reads. It renders
as the text of one chosen option (§6), or as nothing.

Option syntax:

1. An option is **text** followed by an optional **tag list**. The text must not be empty:
   an option that is empty or starts with a tag list makes the message fail to parse.
2. A tag list starts at a `[` for which a `]` occurs somewhere later in the template. It
   runs to the first `]`. All U+0020 spaces inside it are removed. Spaces and tabs after
   the `]` are skipped. Anything else between the `]` and the next `|` or `}` makes the
   message fail to parse. (In the data, 141 options have one space after the `]`.)
   The search for the `]` looks at characters only: a backslash before a `]` does not
   protect it, and slot markers (U+0001) and value text do not stop the search. So
   `[a\]` (a backslash written before the `]`) is a tag list of the two tags `a` and `\`,
   and an escaped `\]` later in the template counts as "a `]` occurring later" (G39). The
   `[` itself is protected as usual: `\[` never starts a tag list, because the text rule
   (§4.10 token 6) takes the backslash and the `[` together.
3. A `[` with no `]` anywhere after it is ordinary option text.
4. Inside a selector, `|` and `}` end the option text; `{`, `^`, `@` and `#` are ordinary
   text; a slot is not allowed (parse failure). Selectors never nest.
5. Option text is used **exactly**, with leading and trailing spaces kept. A common option
   text is a single space (e.g. `#1:{ [E]}`).
6. A selector may have at most **one option without a tag list** (an empty tag list `[]`
   counts as no tag list). Two or more make the message fail (§7.3).
7. The tag list is a sequence of single characters (§5.4). `!` negates the next character.

Binding: a labelled selector reads the variable with the same label. If no variable has
that label, the message fails (reason 7). Every labelled selector in the data resolves.

Abstract patterns, all 7,057 selectors: 6,843 labelled and 214 unlabelled (T = option text;
the class letter list is shortened):

| Count | Pattern | Typical use |
|---|---|---|
| 6,126 | `#N:{T[E]}` | separator text only when the value carries `E` |
| 200 | `#N:{T[f]\|T}` | feminine form, else default |
| 138 | `{T[m]\|T[f]}` (unlabelled) | pronoun by gender |
| 122 | `#N:{T[!n]}` | definite article unless proper noun |
| 88 | `#N:{T[m]\|T[f]}` | pronoun by gender |
| 84 | `#N:{T[1]\|T}` | singular when the count is singular, else plural |
| 49 | `#N:{T[m]\|T[f]\|T}` | pronoun with neuter default |
| 42 | `#N:{T\|T[1]}` | plural default, singular form |
| 26 | `#N:{T[!b]}` | unit suffix unless blank |
| 42 | `#N:{T\|T[<class>]\|…}` (12–14 options) | text by player class or race |
| 12 | `#N:{T[1!b]\|T[!b]}` and `#N:{T[!b1]\|T[!b]}` | singular/plural unit, nothing when blank |
| 3 | `#N:{T[n]\|T[p]\|T[v]\|T}` | indefinite article: none, none, vowel form, consonant form |
| 3 | `#N:{T[P]\|T[p]\|T}` | article by plural kind |
| 8 | `{T\|T[s]}` / `#N:{T\|T[s]}` | possessive suffix for values ending in s |
| ≤ 20 each | others: `[!p]`, `[!1]`, `[s]\|T`, `[f]\|[!f]`, `[b]`, `[1.0]\|T`, one 8-option gender/number selector | |

Options with more than one tag: 22 of 8,101 options. Counting each character as one tag and
`!` with its character as one tag (the rule of §6): 15 options have two tags, 4 have three,
1 has four and 2 have five.

<a id="s4-4"></a>
### 4.4 Unlabelled selectors (Confirmed (BIN + OBS))

A selector without a label (label 0) binds as follows:

1. to the **unlabelled** variable of the entry (a variable with label 0);
2. if the entry has no unlabelled variable, to the variable labelled **−1**;
3. otherwise the message fails (reason 7).

An entry that contains an unlabelled selector must not have more than one unlabelled
variable; otherwise the message fails (§7.3).

Census: 214 unlabelled selectors in 207 entries. 203 bind to the entry's single unlabelled
variable, 11 to a variable labelled −1. Positions: 189 come after at least one slot in the
same message, 25 in piece 0 before any slot. An unlabelled `{` at top level **always**
opens a selector, whether or not its body has a tag list; a literal brace must be written
`\{` (none is needed in the data).

<a id="s4-5"></a>
### 4.5 Directives (Confirmed (BIN + OBS))

At top level, `{{` starts a **directive**. Its word runs to the next `}}` (or to the end of
the template if there is none). A directive renders as **nothing**, whatever its word. The
client acts on two words, compared exactly:

| Word | Effect |
|---|---|
| `keepspaces` | Turns off whitespace normalisation (§7.5) for this message. 37 entries. |
| `item` | Turns on item-name tag composition (§4.5.2). No entry uses it. |

Any other word is accepted and ignored. `keepspaces` is always the last text of the
message in this install (SG9), but its position does not matter to the client.

<a id="s4-5-2"></a>
#### 4.5.2 Item-name composition (Confirmed (BIN); unused in this install)

When the message has the `item` directive and its entry names (§3.3) give a name-role
variable plus a prefix- and/or suffix-role variable:

1. Before assembly, the prefix and suffix values are escape-decoded (§4.6).
2. After evaluation, the client appends a tag group (`[`, letters, `]`) to the output. The
   letters are: those of the prefix variable's tag string that occur in the locale's
   prefix set, then those of the name variable's tag string in the name set, then those of
   the suffix variable's tag string in the suffix set. The sets come from the locale
   record (§10.3); in English they are `v`, `mfnp` and `s`.

The appended group makes the composed name usable as a value in other messages (vowel
start from the first word, gender and number from the name, the `s` ending from the last
word).

<a id="s4-6"></a>
### 4.6 Escapes (Confirmed (BIN + OBS))

Pieces store control characters and grammar characters as two-character escapes. Real
control characters are almost absent: only TAB (U+0009) occurs raw, in 7 entries. No raw
LF or CR occurs.

| Escape | Occurrences | Entries | Decodes to |
|---|---|---|---|
| `\n` | 210,329 | 79,465 | line feed U+000A |
| `\q` | 7,054 | 3,150 | quotation mark U+0022 |
| `\r` | 0 | 0 | carriage return U+000D |
| `\t` | 15 | 7 | TAB U+0009 |
| `\[` `\]` | 55 each | 43 | `[`, `]` |
| `\\` | 33 | 30 | `\` (also used in the link closers of §4.8) |
| `\$` | 1 | 1 | `$` |
| `\@` | 1 | 1 | `@` |
| `\!` `\{` `\}` `\#` `\|` `\^` | 0 | 0 | the character |
| any other `\x` (including `\` at the end) | 14 | 10 | the backslash stays; *x* is processed normally |

The **special characters** are the eleven characters `[ ] ! { } # \ | ^ $ @`.

Two separate stages use escapes:

1. **Protection, during lexing** (§4.10). A backslash followed by a special character makes
   both characters part of the surrounding text; that character never acts as grammar (no
   label, selector, modifier, tag list or option separator starts there). Both characters
   are **kept** in the text. A backslash before any other character protects nothing. Value
   tag groups also respect it: a `[` or `]` directly after a backslash is not a group
   bracket (§4.9). The display cut (§7.6) respects it the same way.
2. **Decoding, at display time** (§7.7). Left to right, single pass: a backslash followed by
   `n`, `q`, `r`, `t` or a special character is replaced by the decoded character; any other
   backslash stays as it is. `\\n` therefore decodes to a backslash followed by `n`.
   Decoding happens **once**, on the whole finished string, never on its parts.

<a id="s4-7"></a>
### 4.7 Colour, emphasis, paragraph and list markup (Confirmed (BIN + OBS))

Markup is interpreted by UI text elements whose markup option is on (a UI property; which
elements turn it on belongs to the UI layout spec). In other elements every character is
shown literally.

A **tag** is `<`, then everything up to the next `>`, then `>`. A `<` without a later `>`
is literal. Tags do not nest inside each other. Tag names are matched
**case-insensitively**.

| Markup | Opening | Closing | Occurrences (open/close) | Effect |
|---|---|---|---|---|
| Colour | `<rgb=` value `>` | `</rgb>` | 13,951 / 13,942 (counted case-insensitively; 13,944 / 13,935 are lower case and 7 of each are written in upper or mixed case) | Opening pushes the current colour on a stack and sets the new one; closing pops it |
| Bold | `<b>` | `</b>` | 78 / 78 | Turns bold on / off |
| Italic | `<i>` | `</i>` | 1 / 1 | Turns italic on / off |
| Underline | `<u>` | `</u>` | 1,120 / 1,120 | Turns underline on / off. Often used for headings. |
| Paragraph | `<p>` | `</p>` | 1 / 0 | Opening inserts a line break (an element option drops it when the tag is at the very start of the text); closing does nothing |
| List item | `<li>` | `</li>` | 136 / 134 | Opening inserts a line break as `<p>` does and turns list-item layout (indent) on; closing turns it off |
| Literal `<` | `<<>` | — | 0 | Shows one `<` |

Rules:

- Bold, italic, underline and list item are on/off switches, not counters: a closer turns
  the attribute off however many openers came before, and a closer with no opener is
  accepted and does nothing.
- Colour value: the text after `rgb=` up to the `>`.
  - If it starts with `#`, the rest is read as an unsigned hexadecimal number. Otherwise it
    is read as an unsigned number in automatic base (`0x` prefix = hexadecimal, other
    leading `0` = octal, else decimal). Leading whitespace and a sign are accepted, as by
    the C library's unsigned conversion.
  - The whole rest must be consumed. If it is not (for example letters without `#`), the
    tag is **not** markup and is shown literally.
  - The colour is the number with its top byte forced to 0xFF: ARGB = value OR 0xFF000000.
    So `#RRGGBB` gives opaque RRGGBB; an eight-digit `#AARRGGBB` value loses its AA byte
    (all 821 in the data have AA = FF anyway); `#12345` gives 0xFF012345; a value above
    0xFFFFFFFF saturates to opaque white.
- Observed colour values (all 13,951 openers): `#` + 6 digits 13,071; `#` + 8 digits 821;
  `#` + 5 digits 53; `#` + 7 digits 3; `#` + 4 digits 1; 6 hexadecimal letters without `#`
  2 (both shown literally by the client).
- 11,677 entries contain a colour opener or closer (any case); 11,675 of them contain an
  opener.
- A `</rgb>` when the colour stack is empty is **not** markup and is shown literally. An
  unclosed colour lasts to the end of the text.
- 13 entries have unbalanced colour tags: 11 have more openers than closers, 2 have more
  closers.
- Any other tag that is not a link (§4.8) is shown literally. Help and chat text uses angle
  brackets as literal placeholders (`<` + a lower-case word or phrase + `>`, 38 distinct
  words); they stay literal under this rule.

> **OPEN:** Whether a list item draws a bullet glyph and how deep the indent is. Both come
> from UI properties of the element, not from the text.

<a id="s4-8"></a>
### 4.8 Link markup (recognition Confirmed (BIN + OBS); meaning OPEN)

27 entries, mostly chat and command templates, contain **link elements**:

- Opening: `<`, a Kind name, `:`, a Type name, `:`, optional further `:`-separated fields,
  then `>`.
- Closing: `<`, a backslash, the same Kind name, `>`. In stored text the backslash is the
  escape `\\`, so it becomes a single backslash at display time (§7.7), before markup is
  interpreted.

Kinds observed: `Select` (16), `Examine` (2), `ExamineIA` (2), `ExamineItemInstance` (2),
`Append`, `MapNoteRef`, `QuestRef`, `NPCQuery`, `RunCmd` (1 each). Type names observed:
`IID`, `IIDDID`, `IAInfo`, `ItemInfo`, `String` and others. These names are interoperability
identifiers.

In every stored template the opener is split by a slot: one piece ends with `<Kind:Type:`,
the slot follows, and the next piece starts with `>`. Because markup is interpreted only on
the finished string (§7.8), the value simply becomes the field text of the opener. No
special joining is needed. A `>` inside such a value would end the opener early in the
client; the data never puts one there.

Client order for each tag (BIN): a `<<>` tag is a literal `<`; otherwise, while a link is
open, the tag is offered to that link (this is how its closer is found); otherwise the tag
is offered to the registry of link kinds, which may open a new link; otherwise it is tried
as §4.7 markup; otherwise it is literal.

> **OPEN:** The semantics of each link kind (what clicking does, field formats) and the
> complete list of kinds the registry accepts. For now Tessera renders the label text,
> keeps the Kind, Type and fields as an inert link annotation, and never acts on it.

<a id="s4-9"></a>
### 4.9 Value tag groups (Confirmed (BIN + OBS))

Text that is meant to be **inserted into other messages** (names of items, people,
creatures, places, classes, races) ends with a tag group: `[`, letters, `]`, then optional
trailing spaces.

- 202,210 entries end with such a group. All but 6 are single-piece entries. 202,208
  groups are letters only, and 202,094 use only the letters `e m p n s v f E`.
- 12,081 groups are exactly `[E]`. All 12,081 are single-piece entries. In 12,080 of them
  text comes before the group; exactly **one** entry is `[E]` with nothing before it.
- 21 groups have spaces after the `]` (spaces only, no tab). 147 groups have a space
  directly before the `[` (no tab occurs there).
- Two groups contain characters other than letters (spaces, or `=`).
- 4 entries open their trailing group with a doubled `[[` (text, `[[`, letters, `]`).
  §5.3 reads such a value with the second `[` as its first tag character; the display cut
  (§7.6) gives the same text as for a single `[`.

Counting rule for this section: a "trailing group" is an unescaped `[`, then characters
other than `[` and `]`, then `]`, then only spaces or tabs up to the end of the last piece.

How a value's group is read when the value fills a variable is §5.3. How the outermost
message loses its own group is the display cut, §7.6.

Example (synthetic): an item-name entry stored as `Reed Boots[p]` gives the display text
`Reed Boots` with the tag string `p`. A person entry `Wren Hollow[fn]` gives `Wren Hollow`
with `fn`.

<a id="s4-10"></a>
### 4.10 Tokens and items (Confirmed (BIN + OBS))

This is the complete grammar the client applies to a template (§7.2). The lexer has two
contexts: **top level** and **inside a selector** (between a selector's `{` and its `}`).

Tokens, tried in this order at each position:

| # | Token | Recognised when | Content |
|---|---|---|---|
| 1 | Slot | the character is U+0001 | the characters up to the next U+0001 (or the end); both U+0001 are consumed |
| 2 | Directive | top level and the next two characters are `{{` | the word up to `}}` (or the end); the `}}` is consumed |
| 3 | Single character | top level: `{` (enters the selector context), `^`, `@`; inside a selector: `\|`, `}` (leaves the context) | the character |
| 4 | Label | top level, §4.2 rules | the integer *N* |
| 5 | Tag list | inside a selector, at `[`, when a `]` occurs later in the template (any `]`, escaped or not, inside a value or not, §4.3 rule 2) | the characters up to the first `]` (a backslash before it is kept as a tag character), U+0020 spaces removed; trailing spaces and tabs skipped |
| 6 | Text | otherwise | the longest run of characters that does not reach a slot, a single-character token of the current context, or a position where token 4 or 5 would be recognised. A backslash followed by a special character (§4.6) takes both characters into the text. |

Items (the parsed message is a sequence of these):

| Item | Made of | Renders as |
|---|---|---|
| Text | one text token | its characters |
| Directive | one directive token | nothing (§4.5) |
| Variable | [label] [`^` or `@`] slot | the value's display text, modified; nothing if the label is negative |
| Selector | [label] `{` option (`\|` option)* `}`; option = text [tag list] | the chosen option's text, or nothing |

Any other token sequence is a **parse failure** (reason 7, §9.3). Examples of failures: a
`^` or `@` not followed by a slot; a top-level `{` whose body is not a list of options; an
empty option or one that starts with a tag list; text after a tag list inside an option; a
slot inside a selector; a selector that is never closed. Top-level `|` and `}` are plain
text. Every one of the 806,052 entries parses (SG11).

---

<a id="s5"></a>
## 5. Tag alphabet

<a id="s5-1"></a>
### 5.1 Matching is by character, case-sensitive (Confirmed (BIN))

A tag is a single character. An option's tag is satisfied by its presence in (or, after
`!`, its absence from) the bound variable's **tag string**. The comparison is exact and
**case-sensitive**: `P` and `p`, `N` and `n`, `V` and `v` are different tags. The only
case relationship is the expansion of value tags `M` and `F` (§5.3).

<a id="s5-2"></a>
### 5.2 Tag letters

Columns: on values = number of value tag groups (letters-only groups, §4.9) containing the
letter; in selectors = number of selectors, labelled or unlabelled, that use the letter in
any option's tag list. The client gives no letter a meaning of its own except the derived
tags and the `M`/`F` expansion; the meanings below describe how the data uses them.

| Tag | Use in the data | On values | In selectors | Mark |
|---|---|---|---|---|
| `m` | masculine | 29,755 | 299 | Reported (LC); usage Confirmed |
| `f` | feminine | 7,016 | 511 | Reported (LC); usage Confirmed |
| `n` | proper noun: no article | 28,240 | 145 | Reported (LC) |
| `p` | plural | 48,289 | 20 | Reported (LC) |
| `P` | a separate plural kind (distinct from `p`) | 24 | 3 | Confirmed (distinct, BIN) |
| `s` | ends in *s*: possessive takes only an apostrophe | 31,648 | 15 | Inferred (usage) |
| `v` | starts with a vowel sound: article "an" form | 15,622 | 4 | Reported (LC) |
| `e` | never read by a selector in this locale: inert | 76,405 | 0 | Confirmed (inert) |
| `E` | "insert a separator": set explicitly by the data, typically on title or rank entries whose trailing group is exactly `[E]` (12,081 such groups; one more group has `E` with other letters) | 12,082 | 6,127 | Confirmed (BIN + OBS) |
| `b` | derived: the value is empty (§5.3) | 0 | 39 | Confirmed (BIN) |
| `+` | derived: the value is a non-negative integer (§5.3) | 0 | 0 | Confirmed (BIN) |
| `1` | derived: the value is a singular integer (§5.3) | 0 | 164 | Confirmed (BIN) |
| `i` | one 8-option selector; never on values: inert | 0 | 1 | Confirmed (inert) |
| class letters `B C G H I K R T U W` | one letter per playable class | 3–7 each | 11–18 each | Reported (LC) |
| `D L O` | race letters (`L`, `O` double as class letters) | 9–10 each | 37–46 each | Reported (LC) |
| `M` / `F` on values | masculine / feminine with proper-noun reading: expand to `mnM` / `fnF` in English (§5.3) | 4–15 each | 0 | Confirmed (BIN) |
| upper-case `N V` on values | distinct letters with no selector use | 4–15 each | 0 | Confirmed (inert) |

Class and race letters must not be hard-coded to names. The display-name entries of the
classes and races carry their own letter as a tag group (Confirmed on the samples
inspected), so a renderer that needs "the letter for the player's class" takes it from that
display name's tag group at run time.

<a id="s5-3"></a>
### 5.3 A value's display text and tag string (Confirmed (BIN))

Every value is text by the time it fills a slot (numbers are formatted first, §9.4; nested
messages are rendered raw first, §9.2). For each variable the client derives:

**If the value contains a `[` that is not directly preceded by a backslash:**

1. Find the first such `[` and the first `]` in the value that is not directly preceded by
   a backslash.
2. If there is no such `]`, or it comes before the `[`, the message **fails** (reason 7).
3. If any character other than space or tab follows that `]`, the message **fails**.
4. **Display text** = the characters before the `[`, with trailing whitespace (space, LF,
   CR, TAB, form feed) removed.
5. **Tag string** = the characters between `[` and `]`, in order, without spaces. Each `M`
   is replaced by the locale's masculine expansion followed by `M`, and each `F` by the
   feminine expansion followed by `F` (§10.3; English: `mn` and `fn`, so `M` → `mnM`,
   `F` → `fnF`). Every other character is kept as it is.

**Otherwise:**

1. **Display text** = the whole value.
2. If the whole value is an integer (optional leading whitespace, optional sign, decimal
   digits, nothing after them; the empty value counts as the integer 0):
   - add `1` if the locale's singular rule selects that integer (§10.3; English: only 1);
   - add `+` if the integer is ≥ 0.
3. If the value is empty, add `b`.

So an empty value has the tag string `+b`, the value `1` has `1+`, `7` has `+`, `-1` has
nothing, and `1,000` or `2.5` (not an integer by this rule) have nothing.

**The integer test exactly** (Confirmed (BIN), G40). The client converts the whole value
with the C library's `wcstol` in base 10, whose result is a signed **32-bit** integer on
the client's platform, and treats the value as an integer when the conversion stops at the
end of the value:

- Leading whitespace is whatever `iswspace` accepts; then one optional `+` or `-`; then at
  least one decimal digit; then nothing at all. Trailing spaces (`5 `), a lone sign, a
  value of only whitespace and any other text are not integers. (The empty value is the
  one exception: the conversion stops at once, at the end, so it counts as 0.) `-0` and
  `+0` are the integer 0.
- **Out of range.** A digit string above 2,147,483,647 counts as the integer 2,147,483,647,
  and one below −2,147,483,648 counts as −2,147,483,648 (the conversion saturates and the
  client ignores the overflow report). The singular rule and the `+` tag then use that
  saturated number: with the English record, `99999999999` gets `+` only (*n* > 1 is not
  singular), and `-99999999999` gets nothing.
- Tessera must reproduce this with a saturating 32-bit conversion; it must not reject or
  wrap long digit strings.

The singular rule is evaluated with the five flags of the locale record (§10.3) in this
order: *n* = 0, *n* = 1, *n* = −1, *n* > 1, *n* < −1; the flag of the case that applies
decides. Without a locale record, only *n* = 1 is singular.

> **Note:** Tessera's earlier rule that derived `E` from "non-empty" is wrong: the client
> never derives `E`. A `{ [E]}` selector emits its space only when the value carries `E` in
> its own tag group.

<a id="s5-4"></a>
### 5.4 Digits and punctuation in tag lists (Confirmed (BIN))

There are no multi-character "numeric tags". Each character of a tag list is one tag, so
`[51,0]` is the four tags `5`, `1`, `,`, `0`, and `[1.0]` is `1`, `.`, `0`. Only `1` can ever
match a derived tag. In the data:

- 164 selectors use `1`. 145 of them have the shape `{T[1]|T}` or `{T|T[1]}` next to a count
  slot, and 12 combine `1` with `!b` for time units.
- Three selectors use other digits: `1.0`; `51` and `0` in one tag list; `350` and `0` as
  alternatives of one selector. Under the client rule the `5`, `3`, `0`, `,` and `.` tags
  are never satisfied, so these options score only through their `1` (if any).

---

<a id="s6"></a>
## 6. Option selection (Confirmed (BIN))

Given a selector's options and the bound variable's tag string *S*:

1. Score each option:
   - an option **without** a tag list (or with an empty one) scores **1**;
   - an option **with** a tag list scores **2 for every satisfied tag**: read the list left
     to right; `!` followed by a character *c* is satisfied when *c* ∉ *S* (the two
     characters count as one tag); any other character *c* is satisfied when *c* ∈ *S*.
     Repeated tags count each time.
2. Choose the option with the **highest score above 0**. On a tie, the **later** option
   wins.
3. If no option scores above 0, the selector renders as nothing.

Consequences:

- A tagged option with at least one satisfied tag always beats the default.
- A tagged option with no satisfied tag can never be chosen, even when there is no default.
- Ties between tagged options go to the later one. For the time-unit patterns
  `{T[1!b]|T[!b]}`, a value `2` (tag string `+`) scores 2 for both options and gets the
  second (plural) one; `1` (`1+`) scores 4 and 2 and gets the first (singular) one; an
  empty value (`+b`) scores 0 for both and renders nothing.

Worked examples (synthetic; the selector reads variable value *X*):

| Selector | *X* | Tag string | Output |
|---|---|---|---|
| `{she[f]\|he[m]\|they}` | `Wren Hollow[fn]` | `fn` | `she` |
| `{she[f]\|he[m]\|they}` | `Tobin Marsh[mn]` | `mn` | `he` |
| `{she[f]\|he[m]\|they}` | `Tobin Marsh[M]` | `mnM` | `he` |
| `{she[f]\|he[m]\|they}` | `Ash Owl[v]` | `v` | `they` (default) |
| `{the [!n]}` | `Copper Kettle[e]` | `e` | `the ` |
| `{the [!n]}` | `Tobin Marsh[mn]` | `mn` | nothing |
| `{ [n]\| [p]\| an[v]\| a}` | `Ash Owl[v]` | `v` | ` an` |
| `{ [n]\| [p]\| an[v]\| a}` | `Copper Kettle[e]` | `e` | ` a` |
| `{ [n]\| [p]\| an[v]\| a}` | `Oak Pails[vp]` | `vp` | ` an` (tie between `[p]` and `an[v]`; later wins) |
| `{acorn[1]\|acorns}` | `1` | `1+` | `acorn` |
| `{acorn[1]\|acorns}` | `4` | `+` | `acorns` |
| `{'s\|'[s]}` | `Moss Hills[ns]` | `ns` | `'` |
| `{ [E]}` | empty | `+b` | nothing |
| `{ [E]}` | `Warden Tobin` | (none) | nothing |
| `{ [E]}` | `Captain[E]` | `E` | ` ` |
| `{hour[1!b]\|hours[!b]}` | `2` | `+` | `hours` |

---

<a id="s7"></a>
## 7. Rendering an entry

Inputs: an entry (§3) and a **value map** from argument ID to value text (§9.2, §9.4 turn
other value kinds into text). Output: a string, plus a **status** (§9.3). The client's
order is §7.1–§7.7; §7.8 is the UI stage; §7.9 is Tessera's rich-text and explorer output.

<a id="s7-1"></a>
### 7.1 Collect values (Confirmed (BIN))

For each slot *i*, look up `arg_ids[i]` in the value map. If **any** slot has no value, the
message is not rendered: status = reason 6 (§9.3). (The client still checks every slot
first.) The same value fills every slot with the same ID.

<a id="s7-2"></a>
### 7.2 Assemble the template (Confirmed (BIN))

Concatenate piece 0, then for each slot *i*: U+0001, the value text of slot *i*, U+0001,
then piece *i*+1. An empty value gives two adjacent U+0001.

**Values that contain U+0001** (Tessera rule). The client inserts value text unchanged, so a
U+0001 inside a value would end that slot early and the rest of the value would be lexed
as template text. Stored data never contains U+0001 (SG8), and the client never produces
one in a value it builds itself. Tessera therefore checks every value text (after numbers
are formatted and nested messages rendered) before assembly: if any value contains U+0001,
the message is not rendered and the status is reason 7 (§9.3). Tessera does not strip the
character or re-frame the slot. The same applies to U+0001 in a piece, which SG8 already
excludes from valid data.

If the entry carries argument names that call for item-name composition (§3.3, §4.5.2), the
prefix and suffix values are escape-decoded first.

<a id="s7-3"></a>
### 7.3 Parse and validate (Confirmed (BIN + OBS))

Run the lexer and grammar of §4.10. While building each variable, derive its display text
and tag string (§5.3); a malformed value tag group is a failure. Then validate:

1. no two variables share a non-zero label;
2. if the entry contains an unlabelled selector, it has at most one unlabelled variable;
3. no selector has more than one option without a tag list;
4. every labelled selector has a variable with its label; every unlabelled selector has an
   unlabelled variable or one labelled −1 (§4.4). (The client checks this one while
   evaluating, when it reaches the selector; see the note in §9.3.)

Any parse failure, value-group failure or validation failure gives status = reason 7.

<a id="s7-4"></a>
### 7.4 Evaluate (Confirmed (BIN))

Go through the items in order and build the output:

- text: append it as stored (escapes still encoded);
- directive: append nothing; remember `keepspaces` and `item`;
- variable: if its label is negative, append nothing; otherwise append its display text,
  capitalised for `^` (§4.2.1) or lowercased for `@` (§4.2.2);
- selector: append the chosen option's text (§6), or nothing.

If `item` composition applies, append its tag group (§4.5.2).

<a id="s7-5"></a>
### 7.5 Normalise whitespace (Confirmed (BIN))

Unless the message has a `keepspaces` directive:

1. Remove leading whitespace: space, LF, CR, TAB and form feed characters at the start of
   the output. Escapes are not decoded yet, so a leading `\n` escape is **not** removed.
2. Reduce every run of two or more characters drawn from {space, TAB} to its **first**
   character.

Trailing whitespace is **kept** at this stage. The rule applies to the whole output,
including option text and values. In the data, 14,175 entries start with a space or tab in
piece 0 and 4,478 contain a run of two or more spaces/tabs (14 of those are protected by
`keepspaces`).

<a id="s7-6"></a>
### 7.6 Display cut (Confirmed (BIN))

In **display mode** (the normal case for a StringInfo shown to the player, §9.2):

1. Find the first `[` in the output that is not directly preceded by a backslash.
2. If there is one, cut the output there (the `[` and everything after it are dropped),
   then remove trailing whitespace (space, LF, CR, TAB, form feed).
3. If there is none, the output is unchanged, trailing whitespace included.

This removes the message's own trailing tag group. In **raw mode** the cut is skipped.

Census (counting rule): parse every entry under §4.10 with every value set to a plain word,
then look for an unescaped `[` in the text items at top level and in every option text,
ignoring only a trailing tag group (§4.9 counting rule) at the end of the last text item
when nothing but directives follows it. Selector tag lists are not text and are not
counted.

- **26** entries have such a `[`. All 26 are in top-level text; no option text contains
  one.
- In **4** of them the `[` is the first of a doubled `[[` that opens the trailing group
  (§4.9). The display cut removes exactly what it would remove for a normal group.
- In the other **22**, the first unescaped `[` of the rendered text is not the start of a
  trailing group, so the display cut removes visible text: 21 have a bracketed part
  without a slot followed by more text (18 of them single-piece entries), and 1 has a slot
  inside the brackets (`[` + slot + `]`) followed by more text. The UI code that shows them
  must use raw mode, or they lose that text.

> **OPEN:** Which UI elements and client features render StringInfo values in raw mode
> rather than display mode. Known need: the 22 entries above that lose visible text. Tessera
> uses display mode by default and raw mode for chat templates and for nested arguments.

<a id="s7-7"></a>
### 7.7 Decode escapes (Confirmed (BIN))

The caller that turns the result into on-screen text decodes escapes once over the whole
string (§4.6, stage 2). A raw-mode render used as a value inside another message is **not**
decoded; its escapes are decoded together with the outer message.

<a id="s7-8"></a>
### 7.8 UI-text markup (Confirmed (BIN))

The decoded string is handed to a UI text element. If that element has markup on, it
interprets §4.7 and §4.8 on the decoded string; otherwise it shows every character.

<a id="s7-9"></a>
### 7.9 Tessera output (normative for `crates/text`)

- The renderer returns the string of §7.1–§7.7 together with the status of §9.3.
  Rendering never panics; every malformed input yields a status, never a crash.
- Markup interpretation (§7.8) is a separate function that turns a decoded string into
  **rich text**: runs of text with attributes (colour, bold, italic, underline, list item,
  link annotation), plus line breaks for `<p>` and `<li>`. UI code applies it only where
  the element has markup on.
- **Explorer mode** (a Tessera presentation, not client behaviour): missing values do not
  stop the render. A missing value is assembled as the empty value (tag string `+b`) and is
  shown as its `${name}` placeholder (§8.3) in a distinct style. Placeholders never change
  selection. Raw mode is used by default so that tag groups and escapes stay visible.
- Recursion: nested StringInfo arguments share a depth counter. The suggested limit is 16
  levels (records.md §4.2). Past it, the nested value fails (reason 7 for the outer
  message).

Example (synthetic, end to end). Pieces `"#1:{The [!n]}#1:"`, `" lifts #1:{her[f]|his[m]|its} lantern"`, `"."`, with arg IDs [*A*, *B*]. Values: *A* = `Copper Golem[e]`, *B* = empty text.
- §7.2 builds `#1:{The [!n]}#1:` U+0001 `Copper Golem[e]` U+0001 ` lifts #1:{her[f]|his[m]|its} lantern` U+0001 U+0001 `.`
- §7.3: variable 1 has display text `Copper Golem`, tags `e`; the second variable is unlabelled, display text empty, tags `+b`.
- §7.4 gives `The ` + `Copper Golem` + ` lifts ` + `its` + ` lantern` + (empty) + `.`
- Result: `The Copper Golem lifts its lantern.`

---

<a id="s8"></a>
## 8. `${VAR}` presentation and the substitution interface

<a id="s8-1"></a>
### 8.1 What `${…}` is

Stored data never contains `${` (G2). The `${NAME}` form is a **Tessera presentation**
of an entry as one string, for the explorer, logs, diffs and tests. It is not a data
format and must never be written back to an archive.

**Format string** of an entry: concatenate the pieces, putting `${` + *name* + `}` in each
slot. Labels, selectors, escapes and markup stay exactly as stored, so the format string
shows everything the renderer will act on. Any literal `$` in a piece is written as `$$`, so
the form is reversible. The data has no `$` followed by `{`, but the rule keeps the
round trip exact.

Hostile entries (Tessera rule, adopted from the implementer's note of 2026-10-04): an ID of
2²⁸ or more (violating records.md H1) is written with 8 hex digits, and the parser accepts
`#` + 1–8 hex digits as a literal ID; a slot without an ID (fewer IDs than gaps, ST3
violated) is written `${}`. Format → parse → format is always stable and the parsed message
renders like the original. It equals the original except that a slot shown by a name gets
that name's hash as its ID, and slots past the last piece come back separated by empty
pieces.

<a id="s8-2"></a>
### 8.2 Choosing the name

For slot *i*, use the first of these that exists:

1. the entry's own argument-name group (§3.3);
2. a name from a **name dictionary** that maps hash → name, built at run time;
3. `#` followed by the ID as 7 upper-case hex digits (IDs are below 2²⁸), e.g. `${#03A1B2C}`.

A Tessera name dictionary may be seeded only from the user's own data, or from names Tessera
itself defines. The published third-party name lists must not be copied in. Names Tessera
defines because the client defines them (interoperability identifiers):

| Name | Hash | Why |
|---|---|---|
| `PLAYERNAME` | 0x0DA47675 | The client supplies this variable itself (§9.2 step 3). |
| `NAME` | 0x00052615 | Used by the 7 named entries and by item composition (§3.3). |
| `PREFIX`, `PRE`, `MID`, `PLURALNAME`, `SUFFIX`, `POST` | (hash them, records.md §1.5) | Item-composition roles (§3.3). |

Hashing every identifier in the property dictionary matches 3 of the 804 IDs.

> **OPEN:** A data source in the install that lists variable names. None was found in the
> archives. The client executable contains identifier strings whose upper-case form hashes
> to 294 of the 804 IDs (covering 47,847 of 80,821 slot uses); whether Tessera may build its
> dictionary at run time by scanning the user's own executable is an owner decision, and
> such names must not be committed as a table.

<a id="s8-3"></a>
### 8.3 Substitution interface (normative for `crates/text`)

- The caller supplies values keyed by **argument ID**. A convenience form keyed by name
  hashes the name with records.md §1.5 first. Names are ASCII. Hash the bytes as given,
  without changing case.
- Supplying a value for an ID that the entry does not use is not an error. The value is
  ignored.
- Missing values: in **client** (plain) mode the render stops with reason 6 (§7.1). In
  **explorer** mode it continues as §7.9 says.
- Values are text (§5.3), integers and floats (formatted by §9.4), or nested messages
  (rendered raw, §9.2 step 4).
- The same interface renders a StringInfo (§9): its argument block, if any, supplies the
  value map.

---

<a id="s9"></a>
## 9. Resolving a StringInfo

The StringInfo byte layout is records.md §4.1.

<a id="s9-0"></a>
### 9.0 Steps (Confirmed (BIN) unless marked)

1. **Literal** (`is_literal` = 1): the literal is the result text. It is **not** parsed by
   the grammar of §4: no selectors, no whitespace normalisation. In display mode the
   display cut (§7.6) applies; then escapes are decoded (§7.7). Only 4 of 6,348 enum display
   values are literals.
2. **Reference** (`is_literal` = 0): if the token is 0, or the table DID is the null DID
   (0), stop with reason 1. Otherwise load table DID `table` from the **selected locale
   archive** (§10); if it cannot be loaded as a string table, stop with reason 3. Find
   `token` in it; if absent, stop with reason 2. If the entry found has a redirect DID,
   follow it as §9.2 says (reasons 4 and 5).
   - Every observed reference names a 0x25 table. records.md §2 reports 0x26 as part of the
     range, but no 0x26 record exists in this install.
   - Over enum mappers, 458 of 6,348 references miss, and in all 458 the table exists but
     the token does not (reason 2).
   - 17 resolved references point at an empty single-piece entry. They render as empty
     text, which is not a miss.
3. **Arguments**: build the value map from the argument block (records.md §4.2): each
   argument value is turned into text and keyed by its `var_id`. Then, if no argument has
   the ID 0x0DA47675 (`PLAYERNAME`), the client adds the process-wide value it keeps for
   that ID, if it has one.
   - No on-disk StringInfo with an argument block was found (G19). Argument blocks are
     expected to come from the network or from client logic at run time.
   - The three auxiliary Pascal strings remain OPEN (records.md §4).
4. Argument value to text:
   - kind 1, nested StringInfo: rendered by these same steps in **raw** mode (its tag group
     kept, escapes not decoded). If it fails, its text is the diagnostic of §9.3, which
     makes the outer message fail too (the diagnostic contains `[` … `]` followed by more
     text, §5.3 step 3);
   - kind 4, integer: §9.4;
   - kind 2, float: §9.4.
5. **Render** the entry with that value map (§7.1–§7.5), then apply the display cut unless
   the caller asked for raw mode (§7.6).
6. The caller that shows the text decodes escapes (§7.7), only if the status is success.

The process-wide `PLAYERNAME` value is set by the client from a rendered string when the
player's character is established. BIN2 shows the mechanism: when a watched property of
the local player's object changes, the client renders that property's StringInfo in raw
mode (§9.0, tag group kept) and stores the result as the value for 0x0DA47675. Inferred:
the property is the character's name, whose value carries the tag group of §9.5. Until the
property is known, Tessera sets `PLAYERNAME` to the local character's name value built by
§9.5.

> **OPEN:** Which property (by property ID) the client watches to set `PLAYERNAME`, and
> whether its value is already the §9.5 form. Getting it wrong changes the gender and
> vowel selections of every message that names the player.

<a id="s9-1"></a>
### 9.1 Named tables

The root DID map 0x28000000 has a label that leads to the DID map of named string tables.
In this install that label (`STRINGTABLE`, root key 0x12) resolves to 0x28000004, but labels
must be resolved at run time (records.md §8.2). That map has 179 keys:

- one 0 key with value 0 (the sentinel);
- 177 values that are existing 0x25 tables, holding 14,409 entries in total;
- key 0x0B, label `Language`, value 0x58000000: the locale record of §10.3. It is not a
  string table.

A consumer that resolves a named table must check the 0x25 prefix before parsing.

<a id="s9-2"></a>
### 9.2 Lookup structure

Tables are sparse: 282,152 DIDs between 0x25000000 and 0x2505B8B2. Sizes vary widely:
144,031 tables have exactly one entry, and the largest has 4,795. Tokens are unique per
table but not sorted (records.md ST5). A reader builds a hash map per table on load and
caches tables by DID. Table records are stored uncompressed, which makes lazy loading cheap.

The u32 that records.md §3.2 calls `token_high` (always 0 in this install) is, in the
client, a **redirect DID** (Confirmed (BIN), G34; never exercised by the data). The entry
reader stores the u32 that follows the token as the entry's first field, and the renderer
does this after it has found the token in the table:

1. If the entry's redirect DID equals the null DID (0), render the entry itself.
2. Otherwise load the record with that DID. If it cannot be loaded, stop with reason 5.
3. Look up the **same token** in the loaded record. If it is not there, stop with reason 4.
4. Otherwise render the entry found there, instead of the entry in the first table. The
   redirect is followed once: the redirect field of the second entry is not consulted.

The client requests the redirect record with the string-table DB type (0x20 STRING_TABLE,
registered for DIDs 0x25000000–0x26FFFFFF, `records.md` §2.1), exactly as it requests the
first table. Tessera implements the same steps and treats a redirect DID outside
0x25000000–0x26FFFFFF, or a record that does not parse as a string table, as "cannot be
loaded" (reason 5).

> **OPEN:** What the client's record loader does when a typed request names a DID outside
> the range of that type (whether it refuses, as Tessera does, or loads whatever is there).
> No redirect exists in this install, so this cannot be observed.

<a id="s9-3"></a>
### 9.3 Failure reasons and what the client shows (Confirmed (BIN))

| Reason | Meaning |
|---|---|
| 0 | success |
| 1 | token is 0, or the table DID is the null DID |
| 2 | token not found in the table (the common miss) |
| 3 | table record cannot be loaded |
| 4 | redirect: token not found in the redirect record (§9.2) |
| 5 | redirect: redirect record cannot be loaded |
| 6 | at least one slot's argument has no value (§7.1) |
| 7 | the template does not parse, a value tag group is malformed, or validation fails (§7.3) |

When a reference StringInfo fails for **any** reason, the text the client shows is a
one-line diagnostic: angle brackets around a short English phrase that names the table DID
and the token, each written as `0x` + 8 upper-case hex digits. No partial message is shown.
Tessera's client must show its own diagnostic of the same shape (table DID and token in
hex, inside angle brackets); the explorer shows the same placeholder in a distinct style.
Tessera must not copy the client's phrase.

Edge case (BIN; never reached by the data): the client writes the evaluation output
directly into the result. A selector-binding failure (§7.3 rule 4) is found during
evaluation, after the result was cleared, so the client then shows the text built up to
that selector instead of the diagnostic. Tessera shows the diagnostic for every failure.

<a id="s9-4"></a>
### 9.4 Numbers as values (Confirmed (BIN); field values Confirmed (OBS))

Numeric arguments are formatted with the locale record (§10.3). If the record is missing
the client prints **nothing** for numbers; Tessera prints plain decimal numbers instead and
logs a warning.

Terms used below: *r* = the record's default radix (argument values always use it;
English 10); the **alphabet** = the record's digit string, whose character at index *i*
stands for digit value *i*; *g* = the record's group size; the pad flag and the minimum
digit count are the record's (English: pad off, minimum 5); "prepend" means "write to the
left of what has been produced so far", because digits are produced from the least
significant place upward.

**Limits shared by both formatters** (Confirmed (BIN), G35, G36). The result is the empty
text when *r* ≤ 1, when the alphabet has fewer than *r* characters, or when the digits and
separators would need more than 63 characters. The client divides by *g* when it groups;
a record with *g* = 0 would crash it. Tessera treats *g* = 0 as "no grouping".

**Final assembly, both formatters.** If the number is negative, the client uses the
negative template as a C format string with the digit text as its only string argument
(English `-%s` gives `-` before the digits). Tessera replaces the first `%s` of the
template with the digit text, turns `%%` into `%`, and keeps every other character as it
is. A non-negative number formatted in radix 16 gets the prefix `0x` (not reachable for
argument values in English, where *r* = 10).

<a id="s9-4-1"></a>
#### 9.4.1 Integers (kind 4)

The value is a signed 64-bit integer.

1. If it is negative, note that and take its two's-complement negation as an **unsigned**
   64-bit number (so the most negative value works too).
2. Repeat: divide by *r* (unsigned 64-bit); prepend the alphabet character for the
   remainder; continue with the quotient. Stop after a digit when the quotient is 0,
   unless the pad flag is set and fewer than the minimum digit count have been produced.
   At least one digit is always produced.
3. Grouping is always on for argument values: after the *k*-th digit, if the loop goes on
   and *k* is a multiple of *g*, prepend the group separator. No separator is ever written
   before the first digit.
4. Final assembly as above.

Examples: 0 → `0`; 1234567 → `1,234,567`; −7 → `-7`. Note that `1,000` is no longer an
integer for tag derivation (§5.3), so it gets no `+` tag.

<a id="s9-4-2"></a>
#### 9.4.2 Floats (kind 2) (Confirmed (BIN), G35)

The client holds a float argument as a 64-bit IEEE double together with a decimal-place
count *d* (an unsigned 16-bit number). A single-precision value from the data is widened
to double exactly. **All arithmetic below is in double precision.** Powers of *r* are
computed with the C library's `pow`.

1. If *d* > 20, or a shared limit applies, the result is the empty text.
2. Let *w* = |*v*| + 0.5 × *r*^(−*d*) + *b*, where *b* = 0.00019999999494757503. *b* is
   the single-precision value nearest to 0.0002, widened to double. It is an **absolute**
   amount: it is the same for every *d* and every *r*, and it is not scaled by the last
   decimal place. Let *limit* = the larger of 1 and *w* (computed once, here).
3. Let *q* = *r*^(−*d*) (the place value of the last decimal) and *k* = 0 (digits produced).
   Repeat:
   1. Multiply the current *w* by *r*^(*d* − *k*) and truncate the product toward zero to
      a signed 64-bit integer. Keep only its **low 32 bits**, read as an unsigned number.
      The digit is that number modulo *r*.
   2. Subtract digit × *q* from *w*. Prepend the alphabet character of the digit.
   3. Add 1 to *k* and multiply *q* by *r*.
   4. Stop if *q* > *limit*, unless the pad flag is set and *k* is below the minimum digit
      count.
   5. Otherwise, if *k* > *d* and *k* − *d* is a multiple of *g*, prepend the group
      separator (grouping is always on for argument values). If *k* = *d* and *d* > 0,
      prepend the decimal separator (English `.`).
4. Final assembly: the template is used when *v* < 0. Negative zero and NaN are not below
   0.

Consequences:

- At least one integer digit is written (the units digit always has *q* = 1 ≤ *limit*),
  and exactly *d* fraction digits; with *d* = 0 there is no decimal separator.
- The digits are those of *w* cut (not rounded again) at *d* places, as long as each
  product in step 3.1 stays below 2³². Larger products lose their high bits, so the client
  prints wrong digits for values of about 4.29 × 10⁹ / *r*^*d* and above. Tessera
  reproduces the same arithmetic, including the 32-bit cut.
- A negative value that rounds to zero is written with the template: −0.001 with *d* = 0
  gives `-0` in English.
- NaN and the infinities never satisfy the stop test, so the 63-character limit is reached
  and the result is the empty text.

Examples (English record, synthetic values): 2.5 with *d* = 1 → `2.5`; 1234.5678 with
*d* = 2 → `1,234.57`; 0.4999 with *d* = 0 → `1` (the bias lifts it past the half-point);
0.4997 with *d* = 0 → `0`; 3 with *d* = 2 → `3.00`; −1234.5 with *d* = 1 → `-1,234.5`;
5,000,000,000 with *d* = 0 → `4,999,999,994` (the 32-bit cut of step 3.1).

> **OPEN:** How *d* is stored for an on-disk kind-2 argument (records.md §4.2 shows only a
> marker byte and an f32, while the client's run-time float argument carries a u16 *d*
> next to a double). No on-disk argument block exists in this install. Until it is known,
> Tessera uses *d* = 0 for on-disk floats and the caller's choice for its own.

<a id="s9-5"></a>
### 9.5 Character-name values built by the client (Confirmed (BIN), G37)

When the client turns a character name into a value for other messages, it takes these
steps, in this order:

1. Apply the display cut (§7.6) to the name: drop everything from its first unescaped `[`.
2. Trim whitespace at both ends.
3. Build the tag letters, in this order:
   1. `M` for a male or `F` for a female character;
   2. `v` if the first character of the trimmed name is in the vowel set below. When the
      name has at least one character and its first character is `+`, the second
      character is tested instead (only one `+` is skipped; for the name `+` alone that is
      the end of the text, which is not a vowel);
   3. `s` if the last character of the trimmed name, lower-cased with the C library's
      `towlower`, is `s` (U+0073).
4. The value is the trimmed name, one space (U+0020), `[`, the letters, `]`.

Vowel set (an algorithm constant, decision 0012), exactly these 26 characters and no
others: `A E I O U a e i o u` and U+00C0 U+00C1 U+00C4 U+00C8 U+00C9 U+00D6 U+00DA U+00DC
U+00E0 U+00E1 U+00E4 U+00E8 U+00E9 U+00F6 U+00F9 U+00FC. The set is not symmetric in case
(for example U+00DA is in it and U+00FA is not; U+00F9 is in it and U+00D9 is not), and `Y`,
`y` are not vowels. Tessera uses the set as given.

The space before the group is kept in the value, but §5.3 step 4 removes trailing
whitespace from the display text, so the space never shows.

Example (synthetic): a female character `Elsa` becomes `Elsa [Fv]`, whose tag string is
`fnFv` (§5.3).

---

<a id="s10"></a>
## 10. Locale selection

<a id="s10-1"></a>
### 10.1 Which archive

String tables live only in the language archive `client_local_<Lang>.dat`. The 282,152
tables were all found there, and none in other archives. Choose `<Lang>` by the rule in
`install-layout.md` §3.5:

1. the `--language` value, if given;
2. otherwise `English`, if present;
3. otherwise the first language in lexicographic order.

All lookups for one rendering session use that single archive.

There is **no cross-locale fallback**. A table or token missing in the selected locale is
a miss (§9). LC also keeps one strings manager per locale with no fallback (Reported).

<a id="s10-2"></a>
### 10.2 The subset word as a language ID

Header word 0x150 of the language archive is 2 for `local_English` (Confirmed; also
`dat-container.md` §3.4). The plugin API's language enumeration (lua-plugin-api.md
Appendix A) gives:

| Language value | Name | Archive name reported | Mark |
|---|---|---|---|
| 0 | invalid | none | Reported |
| 2 | English | `English` | Confirmed (header value 2 observed) |
| 0x10000001 | English (GB) | `EN_GB` per LC | Reported |
| 0x10000003 | French | `FR` per LC; `FR` per install-layout P33 | Reported |
| 0x10000004 | German | `DE` per LC; `DE` per install-layout P33 | Reported |
| 0x10000007 | Russian (older API listing only) | `Russian` per LC | Reported |

Rules for `crates/text`:

- Read the subset word and report it as the locale's **language ID**. The plugin API's
  `GetLanguage()` returns it (lua-plugin-api.md).
- Do not key behaviour off the archive name's spelling. Use the language ID, and keep the
  name only for display and for `--language` matching.
- Language-dependent grammar is **data**: the `M`/`F` expansions, the singular rule, the
  item-composition sets and the number format come from the locale record (§10.3). The
  selection rule (§6) and tag derivation (§5.3) are otherwise language-neutral and must not
  special-case letters.

> **OPEN:** Confirm the subset words of the non-English archives and their locale records
> (needs a second locale installed).

<a id="s10-3"></a>
### 10.3 The locale record (prefix 0x58, DB type STRING_STATE)

The language archive holds one 0x58 record. The client finds it through the named
string-table map: root key 0x12 (`STRINGTABLE`), then key 0x0B (`Language`) in that map
(Confirmed (BIN + OBS)). Tessera resolves it the same way, by label, at run time. In this
install it is 0x58000000, 101 bytes, and it has no DID prologue (records.md §1.6).

The name STRING_STATE is the label of DB type 0x46, which the client registers for DIDs
0x58000000–0x58000001 (`records.md` §2.1; the label comes from the DBOType enum mapper in
the user's data). It is used here as an interoperability identifier.

Layout (Confirmed (BIN + OBS), G29, G38). The record's own serialiser was read (BIN2): it
reads the fields below in exactly this order, with these types, and stores each one in the
object field whose use is given in the Meaning column (the use was read in the number
formatters, the singular test, the item-composition step and the value-tag derivation).
The English record parses exactly with these types (OBS). So the order of the five
singular flags, of the three item-composition sets and of the two expansions is read from
the client, not inferred from English values. Strings are UTF-16 strings with a VLE unit
count (records.md §1.4). Every field marked "bool" must be 0 or 1: the client flags any
other value as a stream error. Tessera rejects such a record (and then behaves as if it
were missing).

| Offset | Size | Type | Meaning | English value |
|---|---|---|---|---|
| 0 | 4 | u32 | unknown | 1 |
| 4 | 2 | u16 | default radix for numbers | 10 |
| 6 | 2 | u16 | minimum digit count when padding | 5 |
| 8 | 1 | u8 (bool) | pad numbers to the minimum digit count (§9.4) | 0 |
| 9 | 2 | u16 | digit group size | 3 |
| 11 | var | UTF-16 string | digit alphabet; the character at index *d* is digit *d* | 17 units: `0`–`9`, then `0`, `A`–`F` |
| … | var | UTF-16 string | decimal separator | `.` |
| … | var | UTF-16 string | group separator | `,` |
| … | var | UTF-16 string | negative template; `%s` stands for the digits | `-%s` |
| … | 1 | u8 (bool) | singular when *n* = 0 (§5.3) | 0 |
| … | 1 | u8 (bool) | singular when *n* = 1 | 1 |
| … | 1 | u8 (bool) | singular when *n* = −1 | 0 |
| … | 1 | u8 (bool) | singular when *n* > 1 | 0 |
| … | 1 | u8 (bool) | singular when *n* < −1 | 0 |
| … | var | UTF-16 string | item composition: letters taken from the **prefix** variable's tag string (§4.5.2) | `v` |
| … | var | UTF-16 string | item composition: letters taken from the **name** variable's tag string | `mfnp` |
| … | var | UTF-16 string | item composition: letters taken from the **suffix** variable's tag string | `s` |
| … | var | UTF-16 string | expansion of value tag `M` (§5.3) | `mn` |
| … | var | UTF-16 string | expansion of value tag `F` | `fn` |
| … | 4 | DID | a font (0x42 prefix) | 0x42000028 |
| … | 4 | u32 | unknown (read by UI text) | 0 |
| … | 4 | u32 | unknown (read by another UI component) | 0 |

The record ends exactly after the last field (SS1). Besides the record bytes, the client's
loader also stores one number taken from the reading context (not from the record); it
does not change the layout. The English digit alphabet has `0`
again at index 10, so a radix-16 number would print `0` for ten; the client never formats
argument values in radix 16 by default.

If the record is missing, the client falls back to built-in values for the grammar part
that equal the English values above (`mn`, `fn`, `v`, `mfnp`, `s`, singular only for 1),
but prints numbers as empty text (§9.4).

> **OPEN:** The meaning of the first u32 and the two trailing u32 fields, and what the font
> DID is used for.

---

<a id="s11"></a>
## 11. Population statistics (Confirmed, all 282,152 tables)

| Metric | Value |
|---|---|
| Entries | 806,052 |
| Entries with ≥ 1 slot | 52,328 (6.49 %) |
| Pieces per entry | 1: 753,724; 2: 27,692; 3: 21,041; 4: 3,457; 5: 83; 6–12: 55 |
| Distinct argument IDs per entry | 0: 753,724; 1: 29,305; 2: 19,666; 3: 3,247; 4–11: 110 |
| Slot uses / distinct IDs | 80,821 / 804 |
| Entries repeating an ID | 1,709 (1,660 of them with no labels) |
| Entries with argument-name group | 7 (all names `NAME`) |
| Empty single-piece entries | 12,156 |
| Entries with a label | 3,643 (9,934 labels, 5 with `^`) |
| Variables: unlabelled / positive label / negative label (hidden) | 70,887 / 9,689 / 245 |
| Modifiers | `^`: 13 (5 labelled, 8 unlabelled); `@`: 0 |
| Entries with a selector | 3,807 (7,057 selectors: 6,843 labelled and 214 unlabelled; 8,101 options) |
| Unlabelled selectors bound to the unlabelled variable / to −1 | 203 / 11 |
| Entries that parse and validate under §4.10 and §7.3 | 806,052 (all) |
| Entries ending in a value tag group | 202,210 (12,081 of the groups are exactly `[E]`; 1 entry is `[E]` alone) |
| Entries with an unescaped `[` outside the trailing group (§7.6 counting rule) | 26 (22 of them lose visible text to the display cut; 4 open the group with `[[`) |
| Entries with `\n` / `\q` | 79,465 / 3,150 |
| Entries starting with a space or tab / with a run of ≥ 2 spaces or tabs | 14,175 / 4,478 |
| Entries starting with the escape `\n` (not trimmed) | 1,419 |
| Entries with colour markup (opener or closer, any case) | 11,677 (11,675 with an opener) |
| Entries with `<u>`/`<b>`/`<i>`/`<p>`/`<li>` | 1,236 |
| Entries with link markup | 27 |
| Entries with `{{keepspaces}}` / `{{item}}` | 37 / 0 |
| Entries containing `${` | 0 |
| Entries containing an HTML entity (`&name;`) | 0 (`&` occurs only as literal text) |
| Entries with a printf-style conversion (`%` + letter) | 3, all literal prose; `%` is literal text |
| Characters | no surrogates, nothing outside the BMP; non-ASCII is Latin-1 (U+00A0–U+00FF) plus General Punctuation (ellipsis, curly quotes, dashes, bullet) and a few symbols in U+21xx |
| Entries with leading / trailing whitespace (any piece edge) | 16,361 / 16,225 |
| Table flag at +4 | 1 in 282,151 tables; 0 in one table of 5 plain single-piece entries (no visible difference) |

Real-data tests must assert the invariants (§12), not these counts. The counts change with
every patch.

---

<a id="s12"></a>
## 12. Invariants

All Confirmed on every entry. A renderer must still handle violations (§7, §9.3), because
input is hostile.

- **SG1**: every labelled selector refers to a label defined on a variable in the same entry.
- **SG2**: a label appears only directly before a slot, a modifier or `{`. Modifiers (`^`,
  `@`) appear only directly before a slot. Observed: 5 labelled and 8 unlabelled `^`, no `@`.
- **SG3**: label numbers match `-?[0-9]+`. Observed −4 to 7.
- **SG4**: within an entry, no two variables carry the same non-zero label.
- **SG5**: selector bodies contain no unescaped `{` and no slot, and every selector is closed.
- **SG6**: selectors (labelled or unlabelled) occur only in entries with at least one slot.
- **SG7**: stored text contains no `${`.
- **SG8**: stored text contains no raw control characters other than TAB, no surrogates,
  and no characters above U+FFFF. (`crates/text` checks "control character" as Unicode
  Cc: U+0000–U+001F except TAB, U+007F and U+0080–U+009F.) In particular no U+0001, so the
  template framing of §7.2 is unambiguous.
- **SG9**: `{{keepspaces}}` has no text and no selector after it in its message (slots and
  empty pieces may follow: 2 entries store it at the end of piece 0 before a slot and an
  empty last piece). This is an observation; the client accepts the directive anywhere.
- **SG10**: every observed StringInfo reference names a 0x25 table that exists. Misses are
  missing tokens only.
- **SG11**: every entry parses under §4.10 and passes §7.3 when every value is a plain word
  without brackets.
- **SG12**: every unlabelled selector's entry has exactly one unlabelled variable, or none
  and exactly one variable labelled −1.
- **SG13**: no option text is empty, and no selector has two options without tag lists.
- **SG14**: every entry's `token_high` (redirect DID, §9.2) is the null DID; every
  name-group byte is 0 or 1 (G41). A renderer must still follow §9.2 when it is not.
- **SS1**: the locale record (§10.3) parses exactly, with no bytes left over.
- **SS2**: every bool field of the locale record (the pad flag and the five singular flags)
  is 0 or 1.

---

<a id="s13"></a>
## 13. Discrepancies

| # | Source says | Observed / read | Resolution |
|---|---|---|---|
| D1 | records.md §3.2: piece count "observed 0–12" | Minimum is 1. No entry has *p* = 0. | Readers still accept 0. records.md should say "observed 1–12". |
| D2 | Research notes: pieces are "interleaved with `${VAR}` slots" | No `${` in data. `${…}` is LC's own rendering. | §8.1 defines `${…}` as a Tessera presentation. |
| D3 | LC tag list has no `e`, `b` is "blank?", and `S` is "plural s" | `e` is the most common value tag and is never selected on. `b` is derived by the client for empty values. Values use lower-case `s`. | §5.2, §5.3. |
| D4 | LC parser: a selector's number is the order of first appearance among parsed references | It is a label bound by `#N:` (BIN). | §4.2 is normative. |
| D5 | LC renders `${NAME:options}` for selectors and drops the label text | Labels and selectors are separate constructs. | §8.1 keeps both visible. |
| D6 | LC selection: one point per matching tag, the earlier option wins ties | Client: two points per satisfied tag, one for the default, the **later** option wins ties (BIN) | §6. The earlier Tessera rule gave "2 hour" for the time-unit patterns; the client rule gives the plural. |
| D7 | LC: tag `E` is satisfied when the value is not empty | Client never derives `E` (BIN); data sets it explicitly (12,081 trailing groups are exactly `[E]`) | §5.3. |
| D8 | LCC: the plain-text sanitizer maps `\q` to an apostrophe; its HTML conversion maps it to a double quote | Client decodes `\q` to U+0022 (BIN) | §4.6. |
| D9 | LC lists upper/lower pairs (`m`/`M`, `f`/`F`, `n`/`N`, `p`/`P`, `v`/`V`) with the same meaning | Client compares exactly; only `M` and `F` on values expand, to `mnM` and `fnF` (BIN) | §5.1, §5.3. |
| D10 | This spec before 2026-10-05 (see §18) | Several rules were Tessera guesses that the client contradicts | Replaced by the BIN rules; listed in the change log. |
| D11 | records.md §3.3: LC reads the byte before the name groups as a "has names" flag, KD as a group count | The client reads it as a flag: 0 = none, 1 = one group, anything else is a stream error (BIN2, G41) | §17 correction for records.md. Data never distinguishes the two (every byte is 0 or 1). |
| D12 | This spec's first census (2026-10-04/05) | Several counts were wrong: exactly-`[E]` entries, spaces before `[`, colour tags, `</p>`, labelled selectors, options with several tags, unescaped `[` | Corrected from a re-run over all entries (G42); see §18. |

---

<a id="s14"></a>
## 14. Open questions (summary)

Closed on 2026-10-05 by BIN (former numbers kept): 1 (`^` capitalises, §4.2.1), 2
(unlabelled selector binding, §4.4), 3 (whitespace and `keepspaces`, §7.5), 4 (`\q` is a
double quote, §4.6), 5a (malformed colours, §4.7), 7 (no case folding; `e`, `i` inert; `P` ≠
`p`, §5), 8 (derived tags and per-character digits, §5.3–§5.4), 9a (why 7 entries keep
names, §3.3), 10 (display on a miss, §9.3), and the number formatting of integers (6a,
§9.4).

Closed on 2026-10-05 by BIN2: the record-to-field match of the locale record, including
the order of the singular flags, the item-composition sets and the `M`/`F` expansions
(§10.3); the redirect DID (§9.2); the float arithmetic (§9.4.2); the integer type of the
tag test (§5.3); escaped `]` in tag lists (§4.3).

Still open:

5b. Link-kind semantics and the full list of link kinds (§4.8); list-item bullet and indent
    (§4.7).
6b. Decimal places of on-disk kind-2 float arguments (§9.4).
9b. A source of variable names; whether a run-time scan of the user's own executable is
    allowed (§8.2).
9c. Which property the client watches to set `PLAYERNAME` (§9.0).
11. Subset words and locale records of the other locales (§10.2).
12. Meaning of the unknown fields of the locale record (§10.3).
13. Which UI elements render StringInfo values in raw rather than display mode (§7.6); the
    22 entries that lose visible text to the display cut need raw mode.
14. What the client's record loader does with a redirect DID outside the string-table range
    (§9.2); Tessera refuses it with reason 5.

---

<a id="s15"></a>
## 15. Validation guidance

Real-data tests (skipped without `TESSERA_GAME_DIR`):

- render every entry of every table in explorer mode with an **empty** value map and in
  client mode with a value map that gives every ID the plain text `X`, with no panic; in
  client mode every entry must succeed (SG11);
- assert SG1–SG14 over every table, and SS1–SS2 on the locale record;
- resolve every enum display StringInfo. Each one either renders or is a reason-2 miss;
- format the integers 0, 1, −7 and 1234567 with the install's locale record and compare
  with §9.4 (as an observation of this install, not a hard-coded table).

Synthetic fixtures (written by our own code; examples from this spec are fine):

- label binding where label *N* is not slot *N*−1; a selector before its variable; a
  negative label that hides its variable; `#0:` behaving as no label; spaces inside a label;
- `^` labelled and unlabelled, with leading punctuation and with `¡`; `@`; a `^` not
  followed by a slot (reason 7);
- each row of the §6 table; the time-unit patterns with `1`, `2` and empty values; an
  option list with no default and no match;
- unlabelled selector with one unlabelled variable, with only a −1 variable, with none
  (reason 7), and with two unlabelled variables (reason 7);
- duplicate non-zero labels (reason 7); two untagged options (reason 7); an empty option
  (reason 7); text after a tag list (reason 7); an undefined label (reason 7);
- value groups: trailing spaces after `]`, a space before `[`, an escaped `\]` with no other
  `]` (reason 7), `]` before `[` (reason 7), text after `]` (reason 7), `M` and `F`
  expansion, an empty value (`+b`), `1`, `-1`, `1,000`;
- escapes: protection of every special character during lexing, `\\n`, a trailing `\`,
  `\r`, decoding only once over nested values;
- whitespace: leading spaces and a leading `\n` escape; runs of spaces and tabs; trailing
  spaces kept; `keepspaces`;
- display cut: a trailing group, an escaped `\[`, a mid-text `[` (cut), raw mode (no cut);
- markup: nested, unbalanced and upper-case colour tags; 4-, 5-, 6-, 7- and 8-digit colour
  values; a decimal value without `#`; letters without `#` (literal); a stray `</rgb>`
  (literal); `<<>`; `<p>` and `<li>` line breaks; a literal `<placeholder>`; a link element
  whose opener contains a value;
- a value that is itself a message with selectors and a tag group, nested to the depth
  limit and one level past it;
- StringInfo failures for every reason 1–7, and the diagnostic placeholder;
- a synthetic locale record with a different group size, separators and singular flags;
  one with each singular flag set alone (checks the flag order of §10.3); one whose `M`
  and `F` expansions and item-composition sets differ from each other (checks their
  order); one with a bool byte of 2 (rejected, numbers then format as without a record);
  one with group size 0 (no grouping);
- tag lists: `[a\]` inside an option (tags `a` and `\`); an escaped `\]` later in the
  template that closes an earlier `[`; `\[` that never starts a tag list;
- a value containing U+0001 (reason 7);
- integer tags of out-of-range values: `2147483648` and `99999999999` (`+` only),
  `-2147483649` (nothing), `+5` (`+`), `5 ` with a trailing space and `  ` (no tags);
- floats: every example of §9.4.2, *d* = 21 (empty), NaN and an infinity (empty), a
  negative value that rounds to zero (`-0`);
- integers: 0, −7, 1234567, the most negative 64-bit value, the pad flag with a minimum
  digit count, a radix of 1 (empty) and an alphabet shorter than the radix (empty);
- character names: male and female, a leading vowel from the set, a leading `+` before a
  vowel, a final `S` (lower-cased match), a name with an existing tag group and with
  surrounding spaces, and a vowel outside the set (for example U+00FA);
- redirects: an entry whose redirect DID names a table holding the token (renders that
  entry), one lacking the token (reason 4), and one naming a missing record (reason 5).

---

<a id="s16"></a>
## 16. Worked examples (synthetic)

<a id="s16-1"></a>
### 16.1 Hidden count

Pieces `"Spare #-1:{wick[1]|wicks}: #-1:"` and `""`, one argument whose value is the
integer 3.

1. §9.4 formats 3 as `3`; §5.3 gives the tag string `+`.
2. The label −1 hides the variable; the selector binds to it and picks `wicks` (default,
   score 1; `[1]` scores 0).
3. Output: `Spare wicks: ` (the trailing space is kept: no `[` to cut, §7.6).

<a id="s16-2"></a>
### 16.2 Unlabelled selector and capitalisation

Pieces `"^"`, `" waves at {her[f]|him[m]} and smiles."` with one argument whose value is
`the miller's boy[m]`.

1. The variable has the `^` modifier and no label; its display text is `the miller's boy`,
   tags `m`.
2. The unlabelled selector binds to it and picks `him`.
3. `^` uppercases the first letter: `The miller's boy waves at him and smiles.`

<a id="s16-3"></a>
### 16.3 Whitespace

Pieces `"  Total:  "`, `" coins"`, value `12`. The output before §7.5 is
`  Total:  12 coins`. Without a directive, §7.5 removes the two leading spaces and reduces
the double space after the colon to one: `Total: 12 coins`. With `{{keepspaces}}` appended
to the last piece, the output stays `  Total:  12 coins`.

<a id="s16-4"></a>
### 16.4 Display cut and escapes

A single-piece entry `Lamp of \qDawn\q[e]` in display mode: §7.6 cuts at `[` →
`Lamp of \qDawn\q`; §7.7 decodes → `Lamp of "Dawn"`. In raw mode the result is
`Lamp of \qDawn\q[e]`, which another message can use as a value with tag string `e`.

<a id="s16-5"></a>
### 16.5 Failures

- The same entry with a value map that lacks one of its IDs: reason 6; the client shows the
  diagnostic with the table DID and the token.
- A value `Bent Nail[p] (old)` for any variable: text after `]` → reason 7.
- A token that is not in the table: reason 2.

<a id="s16-6"></a>
### 16.6 Colour values

`<rgb=#12345>` sets colour 0xFF012345; `<rgb=#80FF0000>` sets 0xFFFF0000 (the 80 is
ignored); `<rgb=16711680>` sets 0xFFFF0000 (decimal); `<rgb=FF0000>` is shown literally,
and so is the `</rgb>` that closes it if no other colour is open.

<a id="s16-7"></a>
### 16.7 Large and signed integer values

Pieces `"#-1:{one[1]|many}#-1:"` and `""`, one argument given as text, English record. The
variable is hidden (label −1); the output is the chosen option.

| Value text | Integer? | Tag string | Output |
|---|---|---|---|
| `1` | yes (1) | `1+` | `one` |
| `+1` | yes (1) | `1+` | `one` |
| `0` | yes (0) | `+` | `many` |
| `4294967297` | yes, saturated to 2,147,483,647 | `+` | `many` |
| `-4294967297` | yes, saturated to −2,147,483,648 | (none) | `many` |
| `1 ` (trailing space) | no: text follows the digits | (none) | `many` |

<a id="s16-8"></a>
### 16.8 Redirect

Table A holds token *t* with redirect DID B (not 0). Table B holds token *t* with pieces
`"Harbour bell"`. Rendering (A, *t*) gives `Harbour bell` (§9.2 step 4). If B lacks *t*, the
status is reason 4; if B does not exist, reason 5. Both show the diagnostic of §9.3 with
the DID of A and the token *t*.

---

<a id="s17"></a>
## 17. Corrections to other specs (not edited here)

| Spec | Section | Correction |
|---|---|---|
| `records.md` | §1.4 (OPEN on the escape and markup grammar) | Answered here: §4, §7. |
| `records.md` | §2.1, row 0x58 ("layout OPEN") | Layout and role: §10.3 of this spec. The record is found through the STRINGTABLE map, key 0x0B. |
| `records.md` | §3.2, `token_high` | In the client this u32 is a redirect DID, 0 = none (§9.2; Confirmed (BIN), G34; never non-zero in data). Suggested name: `redirect_did`. |
| `records.md` | §3.2 `name_group_count` and §3.3 | The client reads this byte as a flag (0 = no group, 1 = one group); any other value is a stream error (G41). Readers should reject values above 1 rather than loop. |
| `records.md` | §3.4 rendering model ("concatenating the pieces and replacing each slot") | Rendering is the full pipeline of §7; plain concatenation is only the template assembly step. |
| `records.md` | §8.2 root map labels | Add `STRINGTABLE` key 0x0B `Language` → the locale record (§9.1). |

---

<a id="s18"></a>
## 18. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First version from OBS, LC and LCC: message model, inline grammar as observed, tag alphabet, LC selection rule, rendering steps, `${VAR}` presentation, StringInfo resolution, locale selection, statistics, invariants. |
| 2026-10-05 | Revision from BIN (client build 4906.0071.1794.4035) and a new census. New: §4.10 token and item grammar (all 806,052 entries parse), §4.2.2 `@`, §4.5.2 item composition, §7 rewritten as the client pipeline (§7.1–§7.9), §9.0 steps with raw mode and `PLAYERNAME`, §9.3 failure reasons and display, §9.4 number formatting, §9.5 character-name values, §10.3 locale record (0x58), §16 worked examples, §17 corrections, SG11–SG13 and SS1. Changed (earlier rules were wrong): negative labels hide their variable; unlabelled selectors bind to the unlabelled variable or −1, not the nearest preceding slot, and every top-level `{` is a selector; selection scores 2 per tag and 1 for the default with ties to the later option; `E` is never derived; digits in tag lists are single-character tags; `+` is a new derived tag; escapes protect grammar characters during lexing and decode once at display time, `\q` is a double quote and `\r` exists; whitespace rule is leading trim plus run collapse with trailing whitespace kept; the message's own group goes by the display cut at the first unescaped `[`; unknown directives render nothing; duplicate labels, undefined labels and missing values fail the message instead of degrading; literals are not parsed; colour values follow the client's number parsing with forced opaque alpha and malformed tags stay literal; `<p>` and `<<>` added; link openers need no special join. Implementer OPENs of 2026-10-04 on bare `^`, escapes, link openers, selection ties, §7 details, hostile round trips and SG9 answered and folded in. |
| 2026-10-05 | Revision for the first review (changes requested), from BIN2 (same build) and a census re-run. **Blocking items:** new provenance rows G34 (redirect DID, §9.2), G35–G36 (float and integer formatting, §9.4), G37 (character-name group and vowel set, §9.5), G38 (locale-record field order) and G39–G42; §10.3 record-to-field match is now read from the record's serialiser (Confirmed (BIN + OBS)), with one table row per singular flag, so the former "Inferred" note is gone; §4.9 corrected (12,081 groups are exactly `[E]`, only 1 entry is `[E]` alone; 147 groups have a space before `[`), §5.2 `E` row and D7 likewise; §4.7 colour counts corrected to 13,951 / 13,942 in 11,677 entries (case-insensitive), `</p>` 0. **Minor:** §4.3 and §11 labelled selectors 6,843 (7,057 in all); option tag counts restated with the counting rule; §7.6 and §11 state the counting rule for unescaped `[` (26 entries, 22 losing text). **Gaps:** §4.3 rule 2 and §4.10 token 5 say that any `]`, escaped or not, closes a tag list; §7.2 adds the Tessera rule for U+0001 in values (reason 7); §9.4 split into §9.4.1 integers and §9.4.2 floats with the absolute bias, double precision, the 32-bit digit cut and all limits; §5.3 states the saturating 32-bit integer test; §10.3 cites the source of the name STRING_STATE. Also new: §9.2 redirect steps and an OPEN on typed loads, SS2, SG14, D11, D12, §16.7–§16.8, more §15 fixtures, a records.md correction for the name-group byte. |
