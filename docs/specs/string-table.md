<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# String tables: message semantics, grammar tags and resolution

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/text` (message model, inline grammar, tag selection, substitution, StringInfo resolution, locale selection); `apps/explorer` string-table view |
| **Companion specs** | `docs/specs/records.md` §1.4–1.5 (UTF-16 strings, name hash), [§3](records.md#s3) (string-table **byte layout**), [§4](records.md#s4) (StringInfo **byte layout**), §8 (DID maps); `docs/specs/install-layout.md` §3.5 (language archives); `docs/specs/dat-container.md` §3.4 (subset word) |

---

<a id="s0"></a>
## 0. Purpose and scope

`records.md` §3 says how to turn the bytes of a 0x25 record into entries made of
**pieces**, **argument IDs** and optional **argument names**. This spec starts where that
one ends. It defines what the text inside those pieces means and how to turn an entry plus
a set of argument values into the text a player would see:

- the message model: pieces, argument slots, argument IDs and names (§3);
- the inline grammar inside pieces: slot labels, selectors, directives, escapes, markup and
  value tag groups (§4);
- the grammar-tag alphabet and the option-selection rule (§5, §6);
- the end-to-end rendering algorithm (§7);
- the `${VAR}` text presentation and the substitution interface Tessera exposes (§8);
- how a StringInfo value (table DID + token, optional arguments) resolves to a rendered
  string (§9), including named tables;
- how the locale archive is chosen and what the subset word means (§10);
- whole-population statistics (§11), invariants (§12), discrepancies (§13), open questions
  (§14) and test guidance (§15).

The byte layout is **not** repeated here. Every example in this spec is synthetic and was
written for this document. None is game text.

---

<a id="s1"></a>
## 1. Provenance

Marks:

- **Confirmed**: measured on the user's own install on 2026-10-04 by a throwaway probe in
  the dirty-room scratchpad. All 282,152 string tables (806,052 entries) in
  `client_local_English.dat` were decoded and every entry was scanned. Counts are given.
- **Reported**: comes from published third-party material only, and the data cannot
  confirm or refute it.
- **Inferred**: deduced by the spec writer from consistent usage patterns in the data. It
  fits every observed case but has not been checked against the original client's
  behaviour. Inferred facts get an OPEN callout when getting them wrong would change what
  the player sees.

Sources:

| Key | Source |
|---|---|
| OBS | Observed in the user's install 2026-10-04 (Steam install, English client data current on that date). Data was read in place and read-only. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0 (Apache-2.0 distribution), studied as compiled classes for facts only: string-table loader, string renderer, tag list, locale file manager. |
| LCC | LotroCompanion `lotro-core` (no licence file), facts only: text sanitizer and HTML conversion helpers. |
| API | The plugin-API enumeration dump summarised in `docs/specs/lua-plugin-api.md` Appendix A (`Turbine.Language`). |
| RS | `docs/specs/records.md` (this project), whose layouts are Confirmed there. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| G1 | Entry = pieces alternating with argument slots; slot *i* takes the value of `arg_ids[i]` | RS, LC; OBS | Confirmed |
| G2 | Stored text never contains `${`. The `${NAME}` form is a tool presentation, not data. | OBS (0 of 806,052 entries); LC renders `${…}` itself | Confirmed |
| G3 | **Slot label** `#N:` at the very end of the piece before a slot names that slot *N* | OBS (9,934 labels in 3,643 entries; every selector reference resolves to one); LC (parser) | Confirmed (syntax and binding) |
| G4 | **Selector** `#N:{option|option|…}`, where each option is text optionally followed by `[tags]` | LC; OBS (6,844 selectors) | Confirmed (syntax) |
| G5 | Tag letters are compared against the tag group carried by the bound value | LC; OBS | Reported (rule); Confirmed (letters observed) |
| G6 | Option choice: the option with the most satisfied tags wins, earlier option on ties, else the first untagged option, else nothing | LC | Reported |
| G7 | Tag `E` is satisfied when the value is not empty | LC | Reported |
| G8 | Tag `b` means "value is blank"; a digit run such as `1` is a numeric tag, "value equal to this number" | OBS (usage: §5.3, §5.4) | Inferred |
| G9 | Values carry their own tags as a trailing `[letters]` group, which is removed for display | LC (name fix-up removes from the last `[`); OBS (202,210 entries end with such a group) | Confirmed (occurrence), Reported (removal) |
| G10 | Escapes `\n` `\q` `\t` `\\` `\[` `\]` | OBS (counts §4.6); LC (`\n` → newline); LCC (`\q` → quotation mark) | Confirmed (occurrence), Reported/Inferred (meaning, per row in §4.6) |
| G11 | Colour markup `<rgb=#RRGGBB>…</rgb>` | LCC; OBS (13,944 opening tags) | Confirmed (syntax), Reported (meaning) |
| G12 | 8-digit colour values are `#AARRGGBB` | OBS (all 821 have AA = FF) | Inferred |
| G13 | Emphasis/list markup `<u>`, `<b>`, `<i>`, `<li>` with matching closers | OBS | Confirmed (occurrence), Inferred (meaning) |
| G14 | Link markup `<Kind:Type:…>label<\Kind>` | OBS (27 entries, 9 kinds) | Confirmed (occurrence); meaning OPEN |
| G15 | `{{keepspaces}}` directive, always at the end of a message | OBS (37 entries) | Confirmed (occurrence), Inferred (meaning) |
| G16 | A label may carry a one-character modifier `^` | OBS (5 labels) | Confirmed (occurrence); meaning OPEN |
| G17 | Bare selectors `{…}` with no `#N:` prefix exist | OBS (214 in 207 entries) | Confirmed (occurrence); binding OPEN |
| G18 | StringInfo resolution: 5,890 of 6,348 enum display references resolve; all 458 misses are a missing token in a table that exists | OBS | Confirmed |
| G19 | No on-disk StringInfo that points at a real (table, token) has an argument block | OBS (heuristic scan of `gamelogic`, `general`, `local_English`: about 1.02 million references, all `has_args` = 0) | Confirmed |
| G20 | Local archive subset word 0x150 = 2, which is the plugin API's English language value | OBS (header); API (meaning) | Confirmed (value), Reported (meaning) |
| G21 | Named string tables: DID map 0x28000004 has 179 keys; 177 point at existing 0x25 tables | OBS | Confirmed |

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
| **Value** | What fills a slot at render time: text, an integer, a float, or a nested message. |
| **Raw text** | Text as stored, before escapes are decoded and markup is interpreted. |
| **Tag group** | A trailing `[letters]` group in a value's raw text that describes its grammar (gender, number, article behaviour, class). |
| **Tag set** | The set of characters in a value's tag group plus derived tags (§5.3). |
| **Label** | `#N:` at the end of a piece: names the slot that follows as *N*, local to the entry. |
| **Selector** | `#N:{…}`: picks one of several texts based on the tag set of the value in the slot labelled *N*. |

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
  slots**: 1,709 entries repeat an ID. Each occurrence renders the same value.
- 804 distinct IDs occur across 80,821 slot uses. The three most frequent IDs account for
  about 67 % of all uses.
- An ID is a hash. Recovering a readable variable name requires a candidate name list
  (§3.3, §8.2). The data rarely stores names.

<a id="s3-3"></a>
### 3.3 Argument-name groups (Confirmed)

The optional group after the argument IDs (records.md §3.3) lists the variable names in
slot order. Only **7 entries** carry one. In each of them, every name hashes to the
matching ID (records.md ST4), and the names are all-capital ASCII identifiers. A name group
gives the explorer readable names for that entry only. It does not change rendering.

> **OPEN:** Why these 7 entries keep their names and no others do. Probably a build-time
> option. A reader must not depend on names being present.

---

<a id="s4"></a>
## 4. Inline grammar inside pieces

Pieces are not plain text. Five layers of syntax share them. A renderer handles them in
the order of §7. This section defines each one.

<a id="s4-1"></a>
### 4.1 Overview of constructs

| Construct | Shape (abstract) | Where | Entries | Mark |
|---|---|---|---|---|
| Slot label | `#` integer `:` [`^`] at the end of a piece followed by a slot | §4.2 | 3,643 | Confirmed |
| Selector | `#` integer `:{` option (`\|` option)* `}` | §4.3 | 3,602 | Confirmed |
| Bare selector | `{` option (`\|` option)* `}` with no `#N:` before it | §4.4 | 207 | Confirmed (syntax) |
| Directive | `{{` word `}}` | §4.5 | 37 | Confirmed |
| Escape | `\` + one character | §4.6 | 81,000+ | Confirmed |
| Colour markup | `<rgb=` value `>` … `</rgb>` | §4.7 | 11,675 | Confirmed |
| Emphasis / list markup | `<u>` `<b>` `<i>` `<li>` and closers | §4.7 | 1,235 | Confirmed |
| Link markup | `<` Kind `:` Type `:` … `>` label `<\` Kind `>` | §4.8 | 27 | Confirmed |
| Value tag group | `[` letters `]` at the very end of the text | §4.9 | 202,210 | Confirmed |

<a id="s4-2"></a>
### 4.2 Slot labels (Confirmed)

A **label** is the character `#`, a decimal integer *N* (optionally preceded by `-`), and
`:`. It appears as the **last characters of a piece that is followed by a slot**, and may be
followed by the single modifier character `^` (§4.2.1).

- The label names the slot directly after that piece as *N*. The label text is **not**
  rendered.
- *N* is an identifier local to the entry. It is **not** the slot index: in partly
  labelled entries, slot 2 may carry label 1. Observed values: 1 to 7 and −1 to −4. Label 1
  occurs 3,398 times, 2 occurs 3,165 times, 3 occurs 3,105 times, −1 occurs 238 times, and
  the rest occur 1 to 10 times each.
- Not every slot has a label. Only slots that a selector refers to need one. In 3,458 entries
  every slot is labelled, and in 185 only some are. Where every slot is labelled, label *N*
  sits on slot *N*−1 in 3,271 of the 3,458 entries. The rest use negative or out-of-order
  numbers.
- No label appears at the end of the last piece (no slot follows it). No label appears
  anywhere else in a piece except as the start of a selector (§4.3).
- Within an entry, the same label never names slots with different argument IDs. In one
  entry, two different labels name slots with the same ID.

Example (synthetic). Pieces `"You hand the lamp to #1:"` and `". #1:{She[f]|He[m]|They} thanks you."`, one argument. The first piece's trailing `#1:` labels slot 0 as 1. The selector in piece 1 reads slot 0's value.

<a id="s4-2-1"></a>
#### 4.2.1 The `^` modifier

5 labels have the form `#N:^`. In all five, a selector of the form "capital-letter article
unless proper noun" comes before the label, and the labelled slot holds the actor of the
sentence.

> **OPEN:** What `^` does (capitalise the value's first letter, or something else). Tessera
> treats `^` as part of the label, renders the value unchanged and logs once per entry at
> debug level.

> **OPEN:** (implementer, 2026-10-04) A **bare** `^` also occurs: in 8 entries a piece
> that is followed by a slot ends with `^` and no label (for example a bare
> "article unless proper noun" selector, a space, `^`, then the slot; or `^` alone as
> piece 0). With the labelled form that makes 13 `^` modifiers (5 labelled, 8 bare);
> SG2 and the §11 label count cover only the labelled 5. `crates/text` treats a bare `^`
> at the end of any piece followed by a slot as the same modifier: it is removed from the
> text (otherwise players would see a stray `^`), recorded on the slot, and the value is
> rendered unchanged. A `^` at the end of the last piece stays literal. Please confirm and
> extend §4.2/§4.2.1, SG2 and §11 to cover the bare form.

<a id="s4-3"></a>
### 4.3 Selectors (Confirmed syntax)

A **selector** is `#`, a decimal integer *N* (optionally preceded by `-`), `:`, `{`, a body
and `}`. It may appear anywhere in any piece, **before or after** the slot labelled *N*. It
renders as the text of one chosen option (§6), or as nothing.

Body syntax:

1. The body is everything up to the first `}`. Selectors never nest, and no observed body
   contains `{`. All 6,844 observed selectors are closed within their piece.
2. The body is split on `|` into **options**. 1 to 14 options were observed. 6,279
   selectors have a single option.
3. In each option, the text before the first `[` is the **option text**. If a `[` is
   present, the characters up to the next `]` are the option's **tag list**. If there is no
   `]`, the tag list runs to the end of the option.
4. Characters after the `]` are ignored. In 141 observed options they are a single space.
5. The tag list is read left to right. `,` is a separator and is otherwise ignored. `!`
   negates the next tag. A maximal run of ASCII digits, optionally followed by `.` and more
   digits, is one **numeric tag** (§5.4). Every other character is one **letter tag**.
   Examples: `!b1` is "not b" and "= 1"; `mps` is three letter tags; `51,0` is two numeric
   tags.
6. Option text is used **exactly**, with leading and trailing spaces kept. A common option
   text is a single space (e.g. `#1:{ [E]}`: a space when the value is non-empty). No
   observed option has empty text, but an empty option text is valid.

Every observed selector refers to a label defined in the same entry: 6,844 of 6,844. A
selector whose label is undefined is evaluated against an **absent** value (§7, step 4).

Abstract patterns, all 6,844 `#N:` selectors and 214 bare selectors (T = option text; the
class letter list is shortened):

| Count | Pattern | Typical use |
|---|---|---|
| 6,126 | `#N:{T[E]}` | separator text only when the value is non-empty |
| 200 | `#N:{T[f]\|T}` | feminine form, else default |
| 138 | `{T[m]\|T[f]}` (bare) | pronoun by gender |
| 122 | `#N:{T[!n]}` | definite article unless proper noun |
| 88 | `#N:{T[m]\|T[f]}` | pronoun by gender |
| 84 | `#N:{T[1]\|T}` | singular when the count is one, else plural |
| 49 | `#N:{T[m]\|T[f]\|T}` | pronoun with neuter default |
| 42 | `#N:{T\|T[1]}` | plural default, singular when one |
| 26 | `#N:{T[!b]}` | unit suffix unless blank |
| 42 | `#N:{T\|T[<class>]\|…}` (12–14 options) | text by player class or race |
| 12 | `#N:{T[1!b]\|T[!b]}` and `#N:{T[!b1]\|T[!b]}` | singular/plural unit, nothing when blank |
| 3 | `#N:{T[n]\|T[p]\|T[v]\|T}` | indefinite article: none, none, vowel form, consonant form |
| 3 | `#N:{T[P]\|T[p]\|T}` | article by plural kind |
| 8 | `{T\|T[s]}` / `#N:{T\|T[s]}` | possessive suffix for values ending in s |
| ≤ 20 each | others: `[!p]`, `[!1]`, `[s]\|T`, `[f]\|[!f]`, `[b]`, `[1.0]\|T`, one 8-option gender/number selector | |

Options with more than one tag: 22 of 8,101 options (15 with two tags, 7 with three).

<a id="s4-4"></a>
### 4.4 Bare selectors

214 brace groups in 207 entries have selector syntax but no `#N:` prefix. Example shape: a
piece `" waves at {her[f]|him[m]} and smiles."` that comes right after a slot. Positions
observed: 189 come after at least one slot in the same message (34 of them right after a
slot, or after only spaces), and 25 are in piece 0 before any slot.

> **OPEN:** What the original client binds a bare selector to. Tessera binds it to the
> **nearest preceding slot**, or to **slot 0** when no slot comes before it, and renders it
> like a `#N:` selector. In an entry with no slots it renders as literal text. Bare
> selectors are recognised only when the body contains a `[`. Braces without a tag list are
> literal text.

<a id="s4-5"></a>
### 4.5 Directives

`{{keepspaces}}` is the only directive observed (37 entries). It is always the last thing
in the message (14 times as the whole last piece). Many of these messages end with a space
before the directive.

- Tessera removes the directive text and sets a "keep spaces" flag for that render (§7,
  step 8).
- Any other `{{word}}` is unknown. Tessera renders it literally and logs at debug level.

> **OPEN:** The exact whitespace rule the directive switches off. Inferred: by default the
> client trims and/or collapses spaces in rendered messages. §7 step 8 states Tessera's
> rule.

<a id="s4-6"></a>
### 4.6 Escapes

Pieces store control characters as two-character escapes. Real control characters are
almost absent: only TAB (U+0009) occurs raw, in 7 entries. No raw LF or CR occurs.

| Escape | Occurrences | Entries | Decodes to | Mark |
|---|---|---|---|---|
| `\n` | 210,329 | 79,465 | line break (U+000A) | Reported (LC) |
| `\q` | 7,054 | 3,150 | quotation mark U+0022 | Reported (LCC's HTML conversion); Inferred from paired use as a nested quote delimiter |
| `\[` | 55 | 43 | literal `[` | Inferred. Used in help and chat text so that brackets are not read as a tag group. |
| `\]` | 55 | 43 | literal `]` | Inferred, as above |
| `\\` | 33 | 30 | literal `\` | Inferred. Also appears in the link closers of §4.8. |
| `\t` | 15 | 7 | TAB | Inferred |
| other `\x` | 14 | 9 | `\` followed by *x*, both kept | Inferred. These are stray backslashes in prose, for example a literal backslash before a space or quote. |
| `\` at end of text | 2 | 2 | literal `\` | Inferred |

Decoding is single-pass and left to right. `\\n` is a literal backslash followed by `n`,
not a line break.

> **OPEN:** (implementer, 2026-10-04) Escapes and grammar characters. Tokenising (§7
> step 3) runs on raw text before escapes are decoded (step 5), so `crates/text` does not
> let a backslash protect `#` or `{`: `\#1:{…}` is a literal backslash followed by a
> selector, and a piece ending in `\#1:` still ends in a label. The data has no `\#`,
> `\{`, `\}`, `\|` or `\<`, so this only matters for hostile text. Please confirm, or
> list `\#` / `\{` as escapes that make the character literal.

> **OPEN:** LCC's plain-text sanitizer maps `\q` to an apostrophe, while its HTML
> conversion maps it to a double quote. Tessera uses U+0022.

<a id="s4-7"></a>
### 4.7 Colour, emphasis and list markup

| Markup | Opening | Closing | Occurrences (open/close) | Meaning |
|---|---|---|---|---|
| Colour | `<rgb=` value `>` | `</rgb>` | 13,944 / 13,935 (7 of each in capitals) | Text colour for the enclosed run (Reported, LCC) |
| Underline | `<u>` | `</u>` | 1,120 / 1,120 | Underlined run. Often used for headings. (Inferred) |
| Bold | `<b>` | `</b>` | 78 / 78 | Bold run (Inferred) |
| Italic | `<i>` | `</i>` | 1 / 1 | Italic run (Inferred) |
| List item | `<li>` | `</li>` | 136 / 134 | Bulleted item (Inferred) |

Rules:

- Markup names are matched **case-insensitively**, because both `<rgb=` and `<RGB=` occur.
- Colour values: `#` + 6 hex digits = RRGGBB (13,071 times). `#` + 8 hex digits = AARRGGBB
  (821 times, every one with AA = FF; Inferred). Malformed values also occur: 5 digits (53),
  7 digits (3), 4 digits (1), and 6 digits without `#` (2). Hex digits may be either case.
- Colour runs can nest with emphasis, for example a bold run inside a colour run.
- 13 entries have unbalanced colour tags: 11 have more openers than closers, 2 have more
  closers. An unclosed run ends
  at the end of the message. A stray closer is ignored.
- Help and chat text uses angle brackets as **literal placeholders**: `<` + a lower-case
  word or phrase + `>`, 38 distinct words. A tag is markup only if its name is in
  the table above, or it has the link shape of §4.8. Everything else is literal text.

> **OPEN:** How the client renders malformed colour values. Tessera parses the hex digits
> present. If there are not exactly 6 or 8, it ignores the colour (the run renders in the
> default colour) and logs at debug level.

<a id="s4-8"></a>
### 4.8 Link markup

27 entries, mostly chat and command templates, contain **link elements**:

- Opening: `<`, a Kind name, `:`, a Type name, `:`, optional further `:`-separated fields,
  then `>`.
- Closing: `<`, a backslash (stored as the escape `\\`), the same Kind name, then `>`.

Kinds observed: `Select` (16), `Examine` (2), `ExamineIA` (2), `ExamineItemInstance` (2),
`Append`, `MapNoteRef`, `QuestRef`, `NPCQuery`, `RunCmd` (1 each). Type names observed:
`IID`, `IIDDID`, `IAInfo`, `ItemInfo`, `String` and others. In stored templates the field
after the Type is empty, so the client fills it at run time. The label between opener and
closer is often empty or a slot.

> **OPEN:** The semantics of each link kind (what clicking does, field formats). For M1,
> Tessera renders the label text, keeps the Kind/Type/fields as an inert link annotation,
> and never acts on it.

> **OPEN:** (implementer, 2026-10-04) In every stored link template the opener is **split
> by a slot**: one piece ends with `<Kind:Type:`, the slot follows, and the next piece
> starts with `>`. The "empty field" above is therefore a slot. `crates/text` joins such
> openers before interpreting markup: the slot values between `<Kind:Type:` and the next
> `>` become the opener's field text. With this rule all 27 link entries yield a link
> annotation. Please confirm and fold this into the prose above.
> Details of the join: a `:` in the template's own text separates fields, while a slot
> value is field text as is (its display text, escapes decoded, markup dropped; a `:` or
> `>` inside a value neither ends the opener nor adds a field; an absent value adds
> nothing, also in the explorer). Field text is capped (4 KiB in total, 32 fields), and a
> `<Kind:Type>` with no `:` after the Type is not a link opener (it stays literal like a
> §4.7 placeholder).

<a id="s4-9"></a>
### 4.9 Value tag groups (Confirmed occurrence)

Text that is meant to be **inserted into other messages** (names of items, people,
creatures, places, classes, races) ends with a tag group: `[`, one or more ASCII letters,
`]`, then optional trailing spaces.

- 202,210 entries end with such a group. All but 6 are single-piece entries. 202,208
  groups are letters only, and 202,094 use only the letters `e m p n s v f E`.
- 21 have spaces after the `]`. 10 have a space before the `[`.
- The group is detected on **raw** text. A `]` stored as `\]` does not end a group (9
  entries end with an escaped `]`).
- For display, the group is removed, together with any spaces directly before `[` and
  after `]` (Inferred; Reported for the removal itself: LC cuts at the last `[`).
- The group's letters form the value's **stored tags** (§5).
- Two groups contain characters other than letters (spaces, or `=`). A group is recognised
  only if it is non-empty and contains only ASCII letters and digits. Anything else stays
  as literal text.
- Uppercase multi-letter groups (for example a three-capital-letter marker in 4 entries)
  look like developer annotations. They follow the same rule: removed and treated as tag
  letters.

Example (synthetic): an item-name entry stored as `Reed Boots[p]` displays as `Reed Boots`
with stored tags {p}. A person entry `Wren Hollow[fn]` displays as `Wren Hollow` with tags
{f, n}.

---

<a id="s5"></a>
## 5. Tag alphabet

<a id="s5-1"></a>
### 5.1 Matching is by character, case-sensitive

A tag is a single character. A selector tag is satisfied by membership in the value's tag
set (§5.3). The comparison is **case-sensitive**: one selector pattern offers `[P]` and
`[p]` as different options (3 selectors). LC also compares characters exactly.

> **OPEN:** LC's tag list gives several upper/lower pairs (`m`/`M`, `f`/`F`, `n`/`N`, `p`/`P`,
> `v`/`V`) the same meaning. Values carry upper-case variants rarely (about 50 entries), and
> selectors use lower case except `P`. Whether the client folds case is unknown. Tessera
> does not fold.

<a id="s5-2"></a>
### 5.2 Tag letters

Columns: on values = number of value tag groups (letters-only groups, §4.9) containing the
letter; in selectors = number of selectors, labelled or bare, that use the letter in any
option's tag list.

| Tag | Meaning | On values | In selectors | Mark |
|---|---|---|---|---|
| `m` | masculine | 29,755 | 299 | Reported (LC) |
| `f` | feminine | 7,016 | 511 | Reported (LC) |
| `n` | proper noun: no article | 28,240 | 145 | Reported (LC) |
| `p` | plural | 48,289 | 20 | Reported (LC) |
| `P` | plural (variant; distinct from `p` in selectors) | 24 | 3 | Reported (LC); distinction OPEN |
| `s` | ends in *s*: possessive takes only an apostrophe; also used for plural verb agreement | 31,648 | 15 | Inferred (usage). LC lists `S` as "plural s". |
| `v` | starts with a vowel sound: article "an" form | 15,622 | 4 | Reported (LC) |
| `e` | unknown; never referenced by a selector in this locale | 76,405 | 0 | OPEN |
| `E` | non-empty (§5.3) | 12,082 | 6,127 | Reported (LC) |
| `b` | blank (§5.3) | 0 | 39 | Inferred; LC lists "blank?" |
| `1` (numeric tag) | count equals one: singular (§5.4) | 0 | 164 | Reported (LC "singular"); numeric reading Inferred |
| other numeric tags | `1.0`, `0`, `51`, `350` in 3 selectors (§5.4) | 0 | 3 | Inferred |
| `i` | unknown (one 8-option selector) | 0 | 1 | OPEN; LC lists only `I` |
| class letters `B C G H I K R T U W` | one letter per playable class | 3–7 each | 11–18 each | Reported (LC) |
| `D L O` | race letters (`L`, `O` double as class letters) | 9–10 each | 37–46 each | Reported (LC) |
| upper-case `M F N V` | variants of `m f n v` | 4–15 each | 0 | Reported (LC: same meaning); see OPEN above |

Class and race letters must not be hard-coded to names. The display-name entries of the
classes and races carry their own letter as a tag group (Confirmed on the samples
inspected), so a renderer that needs "the letter for the player's class" takes it from that
display name's tag group at run time.

<a id="s5-3"></a>
### 5.3 The value's tag set, including derived tags

The **tag set** of a value is the union of:

1. **Stored tags**: the letters of its trailing tag group (§4.9). Text without a group has
   none.
2. **`E`** (non-empty): added when the value's display text (after removing the group) is
   not empty. Reported (LC), which tests the raw value. Tessera tests display text.
3. **`b`** (blank): added when the display text is empty. Inferred from the time-format
   messages: each unit is preceded by `{ [!b]}` and followed by `{unit[!b]}`, and a
   `{00[b]}` option fills a blank minutes field. That only makes sense if a zero unit is
   passed as blank text.
> **OPEN:** Whether integer 0 should count as blank. Tessera never derives `b` from a number.
> A caller that wants blank passes empty text.

An absent value (no argument supplied, or an undefined label) has empty display text, so
its tag set is {`b`}.

<a id="s5-4"></a>
### 5.4 Numeric tags (Inferred)

A numeric tag (§4.3 step 5) is satisfied when the bound value is a **number** (an integer,
a float, or text that is entirely an optionally signed decimal number) whose value equals
the tag's number. A negated numeric tag is satisfied when the value is not a number or is
not equal to it. Integer 1 and float 1.0 both satisfy `1` and `1.0`.

Evidence: of the 164 selectors that use `1`, 145 have the shape `{T[1]|T}` or `{T|T[1]}`
(singular form tagged `1`, untagged plural as the default) next to a count slot, and 12 combine
`1` with `!b` for time units. Three selectors use other
numbers as tags: `1.0`; `51` and `0` in one tag list; and `350` and `0` as alternatives of
one selector. Reading digits as single characters would make these options unreachable,
because no value carries digit letters (0 of 202,210 tag groups contain a digit).

> **OPEN:** Whether the client compares numbers like this, or derives a `1` tag only for
> the value one. On the data the two readings differ only for the 3 selectors above.

---

<a id="s6"></a>
## 6. Option selection (Reported, LC; applies to `#N:` and bare selectors)

Given a selector's options and the bound value's tag set *S*:

1. For each option with a tag list, its **score** is the number of its tags that are
   satisfied. A plain tag *t* is satisfied when *t* ∈ *S*. A negated tag `!t` is satisfied
   when *t* ∉ *S*. Repeated tags count each time.
2. If at least one option has a score > 0, choose the **first** option with the highest
   score.
3. Otherwise choose the **first option without a tag list** (the default).
4. If there is none, the selector renders as nothing.

Notes:

- An option with a tag list can never be the default, even when its score is 0.
- On the 22 multi-tag options this "count" rule differs from an "all tags must hold" rule.
  Example: `[51,0]` holds two alternative numbers. Under the count rule a value equal to
  either one chooses the option, while under the "all" rule it could never be chosen.
  Tessera uses the count rule.
- Ties go to the earlier option, so option order expresses priority. Example: for
  `{ [n]| [p]| an[v]| a}` a value tagged {v, p} chooses the `[p]` option.

> **OPEN:** (implementer, 2026-10-04) Under the count rule the time-unit patterns of §4.3
> (`{T[1!b]|T[!b]}` and `{T[!b1]|T[!b]}`) choose the **first** (singular) option for any
> non-blank value other than 1: both options score 1 and the tie goes to the earlier one.
> That reads as wrong grammar ("2 hour"), which suggests the client's rule differs on
> multi-tag options (for example "all tags must hold", or ties preferring fewer tags).
> `crates/text` implements the count rule exactly as specified until this is settled.

Worked examples (synthetic; the selector reads slot value *X*):

| Selector | *X* | Tag set | Output |
|---|---|---|---|
| `{she[f]\|he[m]\|they}` | `Wren Hollow[fn]` | f, n, E | `she` |
| `{she[f]\|he[m]\|they}` | `Tobin Marsh[mn]` | m, n, E | `he` |
| `{she[f]\|he[m]\|they}` | `Ash Owl[v]` | v, E | `they` (default) |
| `{the [!n]}` | `Copper Kettle[e]` | e, E | `the ` |
| `{the [!n]}` | `Tobin Marsh[mn]` | m, n, E | nothing |
| `{ [n]\| [p]\| an[v]\| a}` | `Ash Owl[v]` | v, E | ` an` |
| `{ [n]\| [p]\| an[v]\| a}` | `Copper Kettle[e]` | e, E | ` a` |
| `{acorn[1]\|acorns}` | integer 1 | E; number 1 | `acorn` |
| `{acorn[1]\|acorns}` | integer 4 | E; number 4 | `acorns` |
| `{'s\|'[s]}` | `Moss Hills[ns]` | n, s, E | `'` |
| `{ [E]}` | empty text | b | nothing |

---

<a id="s7"></a>
## 7. Rendering an entry

Inputs: an entry (§3) and a **value map** from argument ID to value. A value is text
(raw, which may carry a tag group and escapes), an integer, a float, or a nested message
(another entry plus its own value map, typically from a StringInfo argument of kind 1).

Output: **rich text**, a sequence of runs. Each run is decoded text plus attributes
(colour, underline, bold, italic, list item, link annotation). Plain-text consumers
concatenate the runs.

Steps:

1. **Bind labels.** For each slot *i*, if piece *i* ends with a label `#N:` or `#N:^`, bind
   label *N* to slot *i* and remove the label from the piece. If a label is defined twice,
   the first binding wins.
2. **Prepare values.** For each slot *i*, look up the value for `arg_ids[i]`:
   - Text: detect and remove the tag group (§4.9). The remainder is the display raw text.
   - Integer: decimal digits, with `-` if negative. Float: see §14 Q6.
   - Nested message: render it recursively (steps 1–7, without step 8). Then detect and
     remove a tag group on its raw result.
   - Missing: an absent value (§5.3). Its display is empty for plain rendering, or a
     placeholder (§8.3) in the explorer.
   - Compute the tag set (§5.3). Keep the numeric value, if any, for numeric tags (§5.4).
3. **Tokenise each piece** left to right into: literal raw text, `#N:{…}` selectors, bare
   selectors (§4.4), `{{…}}` directives, and markup tags (§4.7, §4.8). An unterminated
   construct, such as `#N:{` with no `}` or `<` with no `>`, is literal text.
4. **Evaluate selectors.** Look up the bound slot (label *N*, or the §4.4 rule for bare
   selectors) and select an option (§6) against that slot's tag set. An undefined label
   evaluates against an absent value. The chosen option text is treated as literal raw text
   from this piece, so it may itself contain escapes.
5. **Decode escapes** (§4.6) in all literal raw text from pieces and options. Decode each
   value's display raw text separately with the same rule, **exactly once**. Never decode
   text that has already been decoded.
6. **Interpret markup** into run attributes (§4.7, §4.8). Values may contain their own
   markup, which stays scoped to the value: tags opened in a value are closed at its end.
7. **Concatenate** piece 0, value 0, piece 1, …, piece *p*−1. The same value appears at
   every slot whose ID matches.
8. **Normalise whitespace** (outermost render only; Tessera policy, see §4.5 OPEN). Unless
   the "keep spaces" flag is set:
   - (a) where a selector output or value meets adjacent text and both sides contribute a
     U+0020, keep one;
   - (b) trim leading and trailing U+0020 from the whole message.

   Line breaks and tabs are never touched. Runs of spaces inside authored text are left
   alone: 4,478 entries contain double spaces of their own.

> **OPEN:** (implementer, 2026-10-04) Points this section leaves to the implementation,
> and what `crates/text` does:
> - **The outermost message's own tag group.** §4.9 says the group is removed for display,
>   but the steps above only remove groups from values. Tessera also removes a trailing tag
>   group from the outermost message (from its last non-empty literal text, after selector
>   evaluation) and reports the letters to the caller as the message's stored tags. With
>   this rule 202,208 entries render without their group (the 2 non-alphanumeric groups of
>   §4.9 stay literal).
> - **Step 8a with empty values.** A slot or selector that contributes nothing still
>   counts as a junction: `"Hello "` + empty value + `" world"` renders `Hello world`.
> - **Nested messages (step 2).** `crates/text` renders a nested message through steps
>   1–7 to rich text, so each of its own values is decoded and markup-scoped separately
>   (an escape can never span a value and the next piece). Its own trailing tag group is
>   removed and taken as the value's stored tags by the same rule as for the outermost
>   message (from its last literal text that is not selector output); a group produced by
>   one of its values belongs to that value and does not propagate. Emptiness (`E`/`b`) and
>   the numeric value come from its text. Its runs are inserted with the outer style
>   underneath (attributes the nested runs set win). Step 8 is not applied inside nested
>   messages.
> - **Explorer mode and selection.** Explorer placeholders never change selection: a
>   nested message whose only text is placeholders is empty (`b`), and placeholders do not
>   count as text when looking for the outermost message's own tag group. Explorer output
>   therefore always makes the same choices as plain output.

Recursion: nested messages share a depth counter. The suggested limit is 16 levels
(records.md §4.2). When it is exceeded, the value renders as absent, and Tessera reports a
warning, not a panic. Rendering a message must never fail. Every malformed construct falls
back to literal text or an absent value as stated above.

Example (synthetic, end to end). Pieces: `"#1:{The [!n]}#1:"`, `" lifts #1:{her[f]|his[m]|its} lantern"`, `"."`, with arg IDs [*A*, *B*]. Values: *A* = `Copper Golem[e]`, *B* = empty text.
- Step 1 binds label 1 to slot 0.
- Step 4 gives `The ` from the first selector (tags {e, E}, `!n` satisfied) and `its` from the second (no f or m, so the default).
- Result: `The Copper Golem lifts its lantern.` Slot 1 contributes nothing.

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

> **OPEN:** (implementer, 2026-10-04) Hostile entries the round trip has to cover:
> `crates/text` writes an ID of 2²⁸ or more (violating records.md H1) with 8 hex digits
> and accepts `#` + 1–8 hex digits as a literal ID; a slot without an ID (fewer IDs than
> gaps, ST3 violated) is written `${}`. Format → parse → format is always stable and the
> parsed message renders like the original. It equals the original except that a slot
> shown by a name gets that name's hash as its ID, and slots past the last piece come
> back separated by empty pieces.

<a id="s8-2"></a>
### 8.2 Choosing the name

For slot *i*, use the first of these that exists:

1. the entry's own argument-name group (§3.3);
2. a name from a **name dictionary** that maps hash → name, built at run time;
3. `#` followed by the ID as 7 upper-case hex digits (IDs are below 2²⁸), e.g. `${#03A1B2C}`.

A Tessera name dictionary may be seeded only from the user's own data, or from names Tessera
itself defines (for example, names the client code passes when it fills slots). The
published third-party name lists must not be copied in. The data offers few names: hashing
every identifier in the property dictionary matches 3 of the 804 IDs.

> **OPEN:** A data source in the install that lists variable names. None was found in the
> archives. Until one is found, the explorer shows hex IDs for most slots.

<a id="s8-3"></a>
### 8.3 Substitution interface (normative for `crates/text`)

- The caller supplies values keyed by **argument ID**. A convenience form keyed by name
  hashes the name with records.md §1.5 first. Names are ASCII. Hash the bytes as given,
  without changing case.
- Supplying a value for an ID that the entry does not use is not an error. The value is
  ignored.
- Missing values: in **plain** mode a missing value renders as empty text. In **explorer**
  mode it renders as its `${name}` placeholder, in a distinct style, and selectors bound to
  it still evaluate as absent (§5.3).
- The same interface renders a StringInfo (§9): its argument block, if any, supplies the
  value map.

---

<a id="s9"></a>
## 9. Resolving a StringInfo

The StringInfo byte layout is records.md §4.1. Resolution:

1. **Literal** (`is_literal` = 1): the literal is the message text. Treat it as a
   single-piece entry. Grammar, escapes, markup and a trailing tag group all apply. Only 4
   of 6,348 enum display values are literals.
2. **Reference** (`is_literal` = 0): load table DID `table` from the **selected locale
   archive** (§10), then find `token` in it.
   - The table DID's high byte should be 0x25. Every observed reference is 0x25.
     records.md §2 reports 0x26 as part of the range, but no 0x26 record exists in this
     install. A reference to any other prefix is a miss.
   - Table missing, or token missing: a **lookup miss**. It is not an error. Over enum
     mappers, 458 of 6,348 references miss, and in all 458 the table exists but the token
     does not.
   - 17 resolved references point at an empty single-piece entry. They render as empty
     text, which is not a miss.
3. **Arguments** (`has_args` = 1): each argument value (records.md §4.2) fills the slots
   whose argument ID equals its `var_id`. Kind 1 is a nested StringInfo (a nested message,
   §7 step 2). Kind 2 is a float. Kind 4 is an integer.
   - No on-disk StringInfo with an argument block was found (G19). Argument blocks are
     expected to come from the network or from client logic at run time.
   - The three auxiliary Pascal strings remain OPEN (records.md §4).
4. **Render** the resolved entry with that value map (§7).

On a miss, plain mode renders the empty string and reports the miss. Explorer mode renders
a visible placeholder showing the table DID and token in hex.

> **OPEN:** What the original client displays on a miss. It is unobservable from data.

<a id="s9-1"></a>
### 9.1 Named tables

The root DID map 0x28000000 has a label that leads to the DID map of named string tables.
In this install that label resolves to 0x28000004, but labels must be resolved at run time
(records.md §8.2). That map has 179 keys:

- one 0 key with value 0 (the sentinel);
- 177 values that are existing 0x25 tables, holding 14,409 entries in total;
- 1 value whose prefix is 0x58, which is not a string table.

A consumer that resolves a named table must check the 0x25 prefix before parsing.

<a id="s9-2"></a>
### 9.2 Lookup structure

Tables are sparse: 282,152 DIDs between 0x25000000 and 0x2505B8B2. Sizes vary widely:
144,031 tables have exactly one entry, and the largest has 4,795. Tokens are unique per
table but not sorted (records.md ST5). A reader builds a hash map per table on load and
caches tables by DID. Table records are stored uncompressed, which makes lazy loading cheap.

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
- The tag grammar of §5 is what this English archive uses. Other locales probably use
  other tag letters (for example cases or genders). The selection rule (§6) is
  language-neutral and must not special-case letters, except the derived tags of §5.3 and
  numeric tags (§5.4).

> **OPEN:** Confirm the subset words of the non-English archives (needs a second locale
> installed). Confirm whether other locales use additional tag letters or derived tags.

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
| Entries with argument-name group | 7 |
| Empty single-piece entries | 12,156 |
| Entries with a label | 3,643 (9,934 labels, 5 with `^`) |
| Entries with a `#N:` selector | 3,602 (6,844 selectors, 8,101 options counting bare selectors) |
| Entries with a bare selector | 207 (214 occurrences) |
| Selectors in entries without slots | 0 |
| Entries ending in a value tag group | 202,210 |
| Entries with `\n` / `\q` | 79,465 / 3,152 |
| Entries with colour markup | 11,675 |
| Entries with `<u>`/`<b>`/`<i>`/`<li>` | 1,235 |
| Entries with link markup | 27 |
| Entries with `{{keepspaces}}` | 37 |
| Entries containing `${` | 0 |
| Entries containing an HTML entity (`&name;`) | 0 (`&` occurs only as literal text) |
| Entries with a printf-style conversion (`%` + letter) | 3, all literal prose; `%` is literal text |
| Characters | no surrogates, nothing outside the BMP; non-ASCII is Latin-1 (U+00A0–U+00FF) plus General Punctuation (ellipsis, curly quotes, dashes, bullet) and a few symbols in U+21xx |
| Entries with leading / trailing whitespace | 16,361 / 16,225 |
| Table flag at +4 | 1 in 282,151 tables; 0 in one table of 5 plain single-piece entries (no visible difference) |

Real-data tests must assert the invariants (§12), not these counts. The counts change with
every patch.

---

<a id="s12"></a>
## 12. Invariants

All Confirmed on every entry. A renderer must still handle violations (§7), because input is
hostile.

- **SG1**: every `#N:{` selector refers to a label defined in the same entry.
- **SG2**: a label appears only at the end of a piece that is followed by a slot. It is
  followed by nothing or by `^`.
- **SG3**: label numbers match `-?[0-9]+`. Observed −4 to 7.
- **SG4**: within an entry, a label never names slots with different argument IDs.
- **SG5**: selector bodies contain no `{`, and every selector is closed within its piece.
- **SG6**: selectors (labelled or bare) occur only in entries with at least one slot.
- **SG7**: stored text contains no `${`.
- **SG8**: stored text contains no raw control characters other than TAB, no surrogates,
  and no characters above U+FFFF. (`crates/text` checks "control character" as Unicode
  Cc: U+0000–U+001F except TAB, U+007F and U+0080–U+009F.)
- **SG9**: `{{keepspaces}}` occurs only at the end of a message.

> **OPEN:** (implementer, 2026-10-04) Two entries store `{{keepspaces}}` at the end of
> piece 0, followed by a slot and an empty last piece, so the directive is the last
> *text* but not the last part of the message. `crates/text` checks SG9 as "no text and no
> selector after the directive" (slots and empty pieces may follow). Please confirm this is
> the intended reading of "end of a message".
- **SG10**: every observed StringInfo reference names a 0x25 table that exists. Misses are
  missing tokens only.

---

<a id="s13"></a>
## 13. Discrepancies

| # | Source says | Observed | Resolution |
|---|---|---|---|
| D1 | records.md §3.2: piece count "observed 0–12" | Minimum is 1. No entry has *p* = 0. | Readers still accept 0. records.md should say "observed 1–12". |
| D2 | Research notes: pieces are "interleaved with `${VAR}` slots" | No `${` in data. `${…}` is LC's own rendering. | §8 defines `${…}` as a Tessera presentation. |
| D3 | LC tag list has no `e`, `b` is "blank?", and `S` is "plural s" | `e` is the most common value tag (76,413) and is never selected on. `b` is used only negated or in time formats. Values use lower-case `s` (31,651). | §5.2 lists the observed letters. Meanings are marked per row. |
| D4 | LC parser: a selector's number is the order of first appearance among parsed references | It is a label bound to a slot by `#N:` before that slot. In partly labelled entries the two readings differ. | §4.2 (label binding) is normative. |
| D5 | LC renders `${NAME:options}` for selectors and drops the label text | Labels and selectors are separate constructs in the data. | §8.1 keeps both visible. |

---

<a id="s14"></a>
## 14. Open questions (summary)

1. Meaning of the `^` label modifier (§4.2.1).
2. Binding of bare selectors (§4.4).
3. Exact whitespace behaviour and what `{{keepspaces}}` switches off (§4.5, §7 step 8).
4. `\q` as a double quote or an apostrophe (§4.6).
5. Rendering of malformed colour values; link-kind semantics (§4.7, §4.8).
6. Number formatting: thousands grouping for integers, and the decimal places and
   rounding for kind-2 floats. Tessera prints integers as plain decimals, and floats in the
   shortest form that round-trips, with no exponent for magnitudes from 10⁻⁴ to 10⁹.
7. Case folding of tag letters; the meanings of `e`, `i` and `P` versus `p` (§5).
8. Whether `b` is derived by the client or supplied by callers; numeric-tag comparison
   (§5.3, §5.4).
9. Why 7 entries keep argument names; a source of variable names in the install (§3.3, §8.2).
10. Client behaviour on a StringInfo miss (§9).
11. Subset words and tag alphabets of the other locales (§10).

---

<a id="s15"></a>
## 15. Validation guidance

Real-data tests (skipped without `TESSERA_GAME_DIR`):

- render every entry of every table with an **empty** value map in plain mode and in
  explorer mode, with no panic and no error (M1 exit criterion 2);
- assert SG1–SG10 over every table;
- resolve every enum display StringInfo. Each one either renders or is a token miss.

Synthetic fixtures (written by our own code; examples from this spec are fine):

- label binding where label *N* is not slot *N*−1; a selector before its label; a
  negative label; a label with `^`;
- each row of the §6 table; a multi-tag option (`[1!b]`); an option list with no default
  and no match;
- an undefined label; an unclosed `#N:{`; a bare selector before any slot and after a slot;
- a value tag group with leading and trailing spaces; an escaped `\]` at the end (no group);
  a non-letter group (stays literal);
- every escape in §4.6, including `\\n` and a trailing `\`;
- nested, unbalanced and upper-case colour tags; 6-, 8- and 5-digit colour values; a
  literal `<placeholder>`; a link element;
- `{{keepspaces}}` with trailing spaces; junction space collapse (`{ [n]| a}` followed by a
  space and a value);
- a value that is itself a message with selectors and a tag group, nested to the depth
  limit and one level past it;
- a StringInfo that points at a missing token, and one that points at a non-0x25 DID.
