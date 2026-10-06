<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Chat window: windows, tabs and filters, the log, the entry line, the command line and built-in command names

| | |
|---|---|
| **Status** | Accepted (revision 2, 2026-10-06: applies the accepted decisions 0014 and 0015; review 2026-10-06: accepted after revision for decisions 0014/0015). History: first version, 2026-10-06; review 2026-10-06: changes requested; revision 1, 2026-10-06: submitted for re-review; re-review 2026-10-06: accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | **Review of revision 2 (Claude review session, spec-writer agent), 2026-10-06: passed, accepted after revision for decisions 0014/0015; status Accepted.** CLEAN_ROOM §5.3: every item holds. The text is functional facts only. There is no code or pseudo-code: §7.3, §6.6 and §4.3 are plain-word steps, and no address or listing appears. There is no copied prose, and the identifiers are interoperability names under the naming note. There is no game text (only the two delimiter code points, U+002F and U+003B; the §11 command words are invented), and no key, hostname or traffic. Every fact has a provenance row with a mark. The new Tessera material is sourced to IB/UR and marked as not a fact about the original. The property tables give type and meaning. The invariants (CW-I1 to CW-I16) are testable. Each unknown is an OPEN with a Tessera rule. Q19 is precise and does not block M5. Nothing falls under a closed gate. **Decision alignment:** §10 item 5 matches 0014 D2 (`open_chat` = `AcceptInput`, `menu` = `EscapeKey`, `text_line_up/down` = `CursorPreviousLine/NextLine`; the old names are retired and their overrides migrated). §6.2 matches the D5 push table (`text_entry` pushed by a focused field at priority 50; `ui.chat_log` pushed by the chat window for itself at priority 0 while it is the active panel). It also matches D7 (a declined command goes on to game code as the same action). §6.4 matches the D2 Subset policy for `text_line_*`. §10 item 9 matches D8 "Game-raised plugin events" and Known difference 21, and defers the skip set to LPA §11.3, which states it. §10 item 10 matches D5 and Known difference 17. 0015 has no owners item for this spec, and the 1024 × 1024 maximum it cites from §3.1 holds. No fact about the original changed relative to the accepted revision 1. **Spot checks** (own read-only probes on the owner's install, 2026-10-06): (1) CW1, CW2a, CW11, CW15a, CW18 OBS and CW20: layout 0x22000034 has 19 top-level elements, and the parse ends at the record end (13,298 bytes). The nine window elements carry WindowID 1–9 with the element IDs of §3.1. Only 0x1000031E has PrimaryWindow true, and it is based on 0x10001389; the other eight are based on 0x1000030A. Every window element has save_loc and save_size true, min 256 × 128, max 1024 × 1024, and a 16 × 16 resize bar with bottom and right borders. Only the primary template contains 0x10000310 and 0x10000313 (max_chars 384, one line). The tab list is 2 rows × 9 columns with flipped layout and wrap indentation 20 in the primary template, and 1 row in the other; selected state 7, normal state 11. The tab template is 100–216 × 23 and the instant-message tab 100–200 × 23. `ChatLogListbox` has max_rows 128 and no keep_in_bounds. `UI_ChatLog_TextElement` is 0x10000309. Both templates set scaler group 0x2D: match. (2) DID maps: UNIQUEDB 0x68 → 0x78000006, 0x1000000C → 0x7800000C and 0x10000017 → 0x78000005; STRINGTABLE 2 → 0x25000200, 0x10000007 → 0x250001AB, 0x1000004D → 0x250001A7, 0x10000065 → 0x250001BB and 0x1000004C → 0x250001AA; UILAYOUT 0x10000007 → 0x22000034. All carry the labels the spec names: match. (3) CW7a and §3.1 scale: 33 opacity entries, each with min ≤ max, covering 0–100 exactly once. The surfaces are consecutive from 0x41005F5F to 0x41005F7F, and 25 falls in 24–26 → 0x41005F67, with `GOP_ChatOpacity` defaulting to 25. In 0x78002E53, group 45 names `GOP_UIScale_Chat`, with minimum 0.4, maximum 4.0 and 73 stops: match. (4) CW27a, CW32a, CW-I6, CW-I7 and CW35a: `CommandSetup` has 9 entries. 0x0DA28738 and 0x02409858 are one piece of one character each (U+002F, U+003B), with no arguments. 0x04DA4989 and 0x04FE7F13 are one piece with no arguments. 0x03E71864 has the arguments 0x00004814, 0x0054665D and 0x000059D4 in that order; 0x00E5CD23 has 0x000059D4; 0x0455D18C has 0x05569AE8 and 0x000059D4. `Commands` has 261 entries: 238 name lists, 25 with several names. All 31 mode-table tokens (distinct), plus 0x073F0613 and 0x0DA3EC70, are name lists: match. (5) CW4a, CW5, CW8a and CW-I3: the defaults hold one window (ID 1, visible) with two tabs. The masks are 0x7FF9F8FEFFFD (40 types; of the 48 keys of 0x230002A8 with its base 0x230002A9, it lacks exactly 0, 2, 17, 25, 26, 27, 34 and 35) and 0x600880004 (types 3, 20, 24, 34, 35). Every set bit is a key. The labels are table 0x250001BB, tokens 0x0554A31C and 0x0918F32C. `GOP_ChatWindow_TabChatTextTypes` is Bitfield64, `GOP_ChatWindow_TabTextTypes` Bitfield32 and `GOP_ChatWindows` an Array: match. **Non-blocking notes for a later revision:** (a) The §3.1 table row for `ChatDisplayText` cites "§13 Q14", but that question is Q16 (Q14 is substitutions). (b) The two heading marks noted by the previous re-review are still there: §4.2's last paragraph and §6.1's last sentence are behaviour (BIN), under headings marked Confirmed. (c) §10 item 10 lists four keys, but 0014's key-map list also gives `ui.chat_log` Ctrl+Shift+Home and Ctrl+Shift+End. `keymap.md` §8 has not yet been revised for 0014, so until it is, the defaults come from 0014's key-map list item 13. (d) §6.2 could say that, without entry focus, `AcceptInput` reaches the window as the global `open_chat` action of game code (`keymap.md` global context; UB §5.3 Tessera note). **Revision 2 (spec writer, 2026-10-06): submitted for review.** It applies the accepted decision records 0014 and 0015: the Tessera key mapping of §10 item 5 is replaced, the paging commands without entry focus and the plugin delivery of chat lines point to the decisions, and one unknown about the original that 0014 records (which map supplies the paging commands) becomes OPEN Q19 (change log). No fact about the original is changed. **Re-review 2026-10-06 (Claude review session, spec-writer agent): passed; status Accepted.** Every item of the CLEAN_ROOM §5.3 checklist holds: no code or pseudo-code (§7.3, §6.6 and §4.3 are plain-word steps), no copied prose, only interoperability identifiers, no game text (only the two delimiter code points), no addresses, nothing under a closed gate; each fact has a provenance row and a mark, and each unknown an OPEN with a Tessera rule. B1 resolved: §1 OBS, CW1 and §3.1 say 19, and the §3.1 table names exactly 19 elements. B2 resolved: the marks are redefined and CW2, CW4, CW7, CW8, CW12, CW15, CW16, CW19, CW24, CW25, CW27, CW32 and CW35 are split into *a* (data) and *b* (BIN), and the section headings follow. B3 resolved: §4.3 (any shared bit, bit 24 included, at most once per log), §5.1 step 2 (highest set bit with a style entry), §5.6 (exact whole-mask match) and §6.2 (script hand-over, Q17) cover masks with several bits; §11.8 agrees with the default masks bit by bit (0x21 reaches tab 1 only, 0x01000020 reaches tab 1 only, 0x04 both tabs). B4 resolved: CW-I9 and CW-I14 cover only submissions that reach the history, and the §11.4 browsing walk-through agrees with §6.3 (k from 1 to min(2, 2) = 2). All five non-blocking items are fixed. UL §9.2, UL §9.6/§9.6.1, UB §4.7, UW §5.3 (`keep_in_bounds` defaults to true, the clamp is min then 0) and `install-layout.md` §8 exist and say what is cited. New probes, read-only on the owner's install, matched these new Confirmed facts: (1) CW12a: `Chat` 0x250001A7 token 0x0E3E83D5, one piece, no arguments; 0x00CC89CC, one piece; (2) CW16a: 0x0D0F4F30 with arguments 0x00048685 and 0x00005277; 0x0938FCCE with 0x013C8E45, 0x00048685 and 0x00005277; also 0x04F7C6D9 with 0x00059E45 and 0x00052615 (§6.5); (3) CW32a: `CommandSetup` 0x04FE7F13, one piece, no arguments; 0x00E5CD23 with argument 0x000059D4; 0x0455D18C with 0x05569AE8 and 0x000059D4 (§7.7); (4) CW19a, CW12a, CW16a, CW25a, CW7a: feedback masks 0x20, 0x400 and 0x800 map to 0x2A001004, 0x2A001003 and 0x2A001003; defaults FullIRC true, key-repeat false, timestamps false, opacity 25; the default style array has entries for types 1 and 6, so §5.1 step 2 styles 0x21 as type 6; (5) CW24a: 0x7900021C has ten `Communication_ChatChannel_List` entries whose `Commands` (0x250001AB) tokens and types match the §6.5 rows for types 15, 46, 47, 16, 18, 27, 19, 38, 44 and 45 exactly; `Communication_LocalizedChannel_List` has two entries, types 9 and 10, each with localized type 19, and its tokens are in table 0x2500481E. **Non-blocking notes for a later revision:** §4.2's heading says "(Confirmed, CW11)", but its last paragraph (selecting a tab shows its log in `LogParentField`) is behaviour; mark it BIN or Probable, since re-parenting is OPEN in Q3. §6.1's heading says "(Confirmed, CW20)", but the sentence saying the chat window handles §6.2 commands before the text widget is BIN (CW21). Step 6 of §5.1 says "*T* is one of the player-speech types", which is unclear for a multi-bit *T*; Tessera applies no filter (Q7), so this does not block. **Revision 1 (spec writer, dirty room, 2026-10-06): submitted for re-review.** It answers every item of the review below. B1: the layout has 19 top-level elements (§1 OBS, CW1, §3.1). B2: the marks are redefined (Confirmed = observed in the data only) and every row that mixed data and behaviour is split into an observed row *a* (Confirmed) and a behaviour row *b* (BIN): CW2, CW4, CW7, CW8, CW12, CW15, CW16, CW19, CW24, CW25, CW27, CW32, CW35; section headings name the split rows. B3: a new BIN pass on the same build gives the multi-bit rules (new row CW41; §2, §4.3, §5.1, §5.6, §6.2, §7.6, §10 item 8, §11.8, CW-I15, CW-I16): a log takes a line when its mask shares **any** bit with the line's mask as delivered, at most once; style is FO §9.3 step 1 (highest set bit with an entry); the sound needs an exact match of the whole mask without bit 24; the 0x21 line goes out through the same game-script entry as the unknown-command line. CW9 moves from Probable to BIN; Q4 is narrowed; new OPENs Q17, Q18. B4: CW-I14 and CW-I9 are restricted to submissions that reach the history, §6.3 and §6.2 say what an empty or result-2 submit does to the browsing index, and §11.4 has the reviewer's example. Non-blocking: IL and UW are defined in the header; Q2 and §13 cite UL §9.6/§9.6.1; §3.2 cites UL §9.2; §5.5 cites UB §4.7 for the wheel and UW §5–§6 for list scrolling and scroll bars; `CursorNextPage` is clamped to 0 by the list box's scroll-to (BIN, new CW18 evidence). Earlier records follow. First version submitted for review on 2026-10-06. It answers item A8 of the M5 completeness critique: the chat window has no spec, `ui-layout.md` §17.5 item (2) (which chat windows exist), the `lua-plugin-api.md` §11.2 OPEN on the command-line parser, and how `Shell.IsCommand` answers for built-in names. **Review 2026-10-06 (Claude review session, spec-writer agent): changes requested; status stays Review.** CLEAN_ROOM §5.3: no code or pseudo-code (the §7.3 tokenizer and §6.6 guard are plain-word steps); no copied prose; only interoperability identifiers; no game text (only the two delimiter code points); no addresses; no closed gate. Independent probes, read-only on the owner's install, matched these Confirmed facts: (1) layout 0x22000034 (in `local_English` only) has nine `UI_ChatWindow_WindowID` elements 1–9, only 0x1000031E is primary, and it is based on 0x10001389; `ChatLogListbox` `max_rows` 128; the entry has `max_chars` 384, the line template 128,000; tab list 2 × 9 (primary) and 1 row (other); (2) 0x78000006: one window, ID 1, visible; tab masks 0x7FF9F8FEFFFD (40 types, missing exactly 0, 2, 17, 25, 26, 27, 34, 35 of the 48 enum keys) and 0x600880004 (3, 20, 24, 34, 35); labels `UI` tokens 0x0554A31C and 0x0918F32C; defaults FullIRC true, opacity 25, timestamps false, key-repeat false, profanity true; (3) 0x7800000C: 33 ranges covering 0–100 exactly once, consecutive surfaces 0x41005F5F–0x41005F7F, 25 → 24–26 → 0x41005F67; (4) 0x78000005: types 6, 11, 12 → 0x2A001004, 0x2A001003, 0x2A001003; all nine 0x79 sets give the §4.7 table, and each 0x70 object's `GOP_ChatTextType` equals `TypeToAdd`; (5) `CommandSetup` 9 entries, tokens 0x0DA28738 and 0x02409858 are U+002F and U+003B; `Commands` 261 entries, 238 name lists, 25 multi-name, 23 other; all 31 mode tokens and 0x073F0613/0x0DA3EC70 are name lists; CW-I7 argument IDs; STRINGTABLE keys 2, 0x10000007, 0x1000004D, 0x10000065, 0x1000004C. The §11.3 tokenizer table and the §7.3 consequences were re-derived from the §7.3 steps and agree. **Blocking:** (B1) the layout has **19** top-level elements, not 23 (TSize count 19, the parse ends at the record end, and the §3.1 table already names exactly those 19). Fix the §1 OBS text, CW1 and §3.1. (B2) Several rows are marked Confirmed where the data supports only a default, a token or a subset, and the behaviour rests on BIN alone: CW8 (has-all, toggle, save on toggle), CW12 (instant-message tab creation), CW16 (template choice), CW19 (exact-match lookup, volume 1.0), CW24 (21 of 31 mode rows and the whole switch rule), CW25 (1.0 s, more than 10 repeats, sound 0x50), CW32 (result codes 2, 3, 5). The section headings repeat the claim (for example "§6.6 (Confirmed, CW25)"). Split each row into its OBS part (Confirmed) and its behaviour part (BIN), or mark them "Confirmed (OBS: …); BIN (…)". (B3) §6.2 `ChatModeReply` posts a line of mask 0x21, which is types 1 **and** 6. That contradicts the one-type-bit line model of §2, §4.3 and §5.1. State how a multi-bit mask is styled (FO §9.3 step 1), dispatched to tabs (any bit or all bits) and matched for the exact-match sound lookup, or add an OPEN with a Tessera rule. (B4) CW-I14 is false as stated: an empty submit (§6.2 step 1) or a result-2 run (§7.2 step 1) does not reset the browsing index. Example: browse, delete the text, press Enter. Restrict the invariant to non-empty submissions with a registry, or specify the reset. **Non-blocking:** "IL §8" (§3.4) is not defined in the header or §1 (use `install-layout.md` §8); Q2 and §13 cite UL §9.5 (Media player) for image modes, but the draw modes are UL §9.6/§9.6.1; §3.2 step 2 cites UL §9.1 (Root panels) for the create path, which is UL §9.2; §5.5 cites UB §4.7 for "scrollable list boxes" but that section covers only the wheel; §5.5 `CursorNextPage` gives a negative offset when *c* < *v* (state the clamp to 0 or mark it OPEN). |
| **Implements into** | `apps/client` (native chat panel P2 of `ui-layout.md` §17, entry line, command dispatch), `crates/ui` (`tessera-ui`: chat-log list, tab list, entry line widgets), `crates/plugins` (`Shell.IsCommand`, `Shell.AddCommand`, `ShellCommand:Execute` arguments, `Shell.WriteLine` lines), `crates/settings` (Tessera's own store of the chat-window configuration), `crates/net` sandbox backend (local echo of channel lines) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (layout records, panel P2, resolution of base elements; **UL**), [`ui-behaviour.md`](ui-behaviour.md) (focus, key routing, text-entry keys, list scrolling; **UB**), [`fonts.md`](fonts.md) §9.3 (chat line font and colour; **FO**), [`lua-plugin-api.md`](lua-plugin-api.md) §6.6, §9.3, §11 (shell and chat API; **LPA**), [`keymap.md`](keymap.md) §8, §12 (Tessera actions, original input commands; **KM**), [`string-table.md`](string-table.md) (StringInfo, templates, markup; **ST**), [`property-system.md`](property-system.md) (property sets, bitfields, game-object property sets; **PS**), [`records.md`](records.md) §7–§8 (enum mappers, DID maps; **REC**), [`sound.md`](sound.md) (sound records), [`install-layout.md`](install-layout.md) §8 (per-user settings folder; **IL**), [`ui-widgets.md`](ui-widgets.md) §5, §6, §8 (Scrollable, Scrollbar, ListBox; **UW**). Tessera decision records (Accepted 2026-10-06): [`0014-ui-input-bridge.md`](../decisions/0014-ui-input-bridge.md) (UI commands as key-map actions, the UI band, frame loop; **IB**) and [`0015-ui-rendering.md`](../decisions/0015-ui-rendering.md) (UI paint and composition; **UR**) |

> **Naming note (interoperability).** Property names (`UI_ChatWindow_…`, `UI_ChatTab_…`,
> `UI_ChatLog_…`, `UI_ChatOpacity_…`, `UI_Chat_…`, `GOP_ChatWindow…`, `GOP_Chat…`,
> `GOP_EnableTimestampsOnChat`, `GOP_FullIRCMode`, `GOP_ProfanityFilterEnabled`,
> `Version_AddNewChatTextType_…`, `Communication_…`, `UICore_…`), element labels
> (`NewChatWindow1` … `NewChatWindow9`, `ChatWindowTextEntry`, …), widget-type labels
> (`ChatWindow`, `ChatWindowTabListBox`, `PlayerChatTextEntry`, `ChatTab`, `ChatLog`, `IMTab`,
> `IMLog`, `IMChatLog`, `IMSessionLog`), DID-map labels (`STRINGTABLE` entries `CommandSetup`,
> `Commands`, `Chat`, `UI`; `UNIQUEDB` entries `GameplayOptionDefaults`, `UI_ChatOpacities`,
> `ChatMarker_TextTypeFeedback`), chat-type enum labels (`Say`, `Tell`, …), input-command names
> (`AcceptInput`, `EscapeKey`, `START_COMMAND`, …) and plugin-API names (`Turbine.Shell`,
> `IsCommand`, `AddCommand`, `ShellCommand`, `Execute`, `WriteLine`, `Turbine.Chat`) are
> identifiers that the user's data, key map and plugins use. They appear only because a
> compatible client cannot work without them (the *Google v. Oracle* row of `docs/LEGAL.md`
> §4.1 and the plugin-namespace record in `docs/LEGAL.md` §17). **No game text appears in this
> spec.** Command words, tab names, channel names and messages are game text in the user's
> language; they are identified here only by string table and token, and Tessera reads them
> from the user's install at run time or uses its own wording. The only characters quoted from
> string tables are the two one-character delimiters of the command syntax (the command prefix
> U+002F and the substitution marker U+003B in this install), stated as code points because
> they are syntax; Tessera reads them at run time too. Command words in the examples of §11
> are invented.

---

<a id="s0"></a>
## 0. Purpose

`ui-layout.md` §17 lists the chat window (panel P2) as one of the native panels M5 draws. Its
geometry and art are in UL; its line font and colour are in FO §9.3. This spec gives an
implementer the rest:

- which chat windows the original creates, from which elements, with which default tabs, and
  where that configuration lives (§3);
- tabs, their 64-bit chat-type filters, the tab list and the unread marker (§4);
- the log: appending a line, the line cap, timestamps, sticking to the newest line, page and
  document keys, feedback sounds (§5);
- the entry line: which window has it, opening, submitting and cancelling, history, channel
  modes, and the key-repeat guard (§6);
- the command line: prefix, whitespace, tokenizer and quotes, command word and arguments,
  `;` substitutions, result codes, the unknown-command message (§7);
- built-in command names and how `Shell.IsCommand` answers (§8), and what plugins receive
  (§9);
- Tessera's decisions for the M5 sandbox (§10), worked examples (§11), invariants (§12) and
  open questions (§13).

Everything a server would do (delivering other players' lines, tells, channel membership,
the profanity word list) is out of scope; this spec stops where the original hands a line to
the server or to game scripts, and says so.

All counts are observations of one install on one date. Implementations read the data at run
time (layouts, string tables, property sets) and tests assert the invariants (§12), not the
counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | The owner's Steam install, read in place and read-only on 2026-10-06 with throwaway probes in the dirty-room scratch area: the UI layout `chatwindow` (UILAYOUT key 0x10000007, record 0x22000034) decoded in full with every property value (all 19 top-level elements, the TSize count of the element table, and their children; the parse ends exactly at the end of the record); the property sets 0x78000006 (`GameplayOptionDefaults`), 0x78000005 (`ChatMarker_TextTypeFeedback`), 0x7800000A (`UI_ChatEmotes`), 0x7800000C (`UI_ChatOpacities`) and 0x7900021C decoded in full; the nine game objects 0x70035169, 0x70035199–0x7003519B, 0x70040072, 0x70047DC0–0x70047DC3 and their property sets (prefix 0x79, PS §6.2); a scan of every record of `client_gamelogic.dat`, `client_general.dat` and `client_local_English.dat` for 30 chat-related property IDs stored with their echo; the property dictionary 0x34000000 (names and types); the DID maps `STRINGTABLE` (0x28000004) and `UNIQUEDB` (0x28000002); the enum mappers 0x230002A8 (chat types, with base) and 0x230000AF (input commands); every entry of the string tables 0x25000200 (`CommandSetup`), 0x250001AB (`Commands`), 0x250001A7 (`Chat`) and 0x250001BB (`UI`) as token, piece count, piece lengths and argument IDs (the text was read in the dirty room to classify entries and is not reproduced); the script record 0x56000000 searched for references to the `Commands` table; the UI-scale definition set 0x78002E53 (entry of group 0x2D); the owner's settings folder in the Proton prefix (file names, and `UserPreferences.ini` searched for chat keys: none). |
| BIN | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), disassembled with capstone and pefile in the dirty room on 2026-10-06, function by function (address ranges only). Numeric constants are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). **Questions:** (1) what `Shell.IsCommand` tests and how command names are compared; (2) how a submitted line becomes a command: prefix, whitespace, quotes, tokens, command word, arguments, the default channel, `;` substitutions, result codes; (3) the command history: size, duplicates, browsing; (4) which chat windows exist, how window IDs map to layout elements, which window has the entry line, and how the configuration is read and written; (5) tab filters and the unread marker; (6) how a line is appended to a log: cap, timestamps, sticking to the newest line, sounds, logging; (7) which input commands the chat window handles and what each does; (8) channel modes of the entry line and the space-key switch; (9) the key-repeat guard; (10) the chat background opacity. **Revision 1 questions (same build, read 2026-10-06):** (11) how a line whose mask has more than one type bit is styled, given to logs and matched for its feedback sound, and how the reply line of `ChatModeReply` and the unknown-command line leave the chat window; (12) what bounds the page keys' scroll offset. **Read in revision 1:** the log's line-receive handler (the per-log mask test) and its refill handler, the client-wide object both consult, the feedback-sound lookup, the post routine both client-posted lines use (as far as the hand-over to a game-script entry point), the page and document branches of the input-command handler, and the list box's scroll-to routine. **Read:** the plugin layer's shell bindings (`IsCommand`, `Execute` call); the command module (registry construction, registration with alias splitting, lookup, the execution pipeline, the tokenizer, the substitution expander and its trigger test, the history add, trim and browse routines, the help listing); the chat-window game widget (initialisation, configuration save and load, tab serialisation, the input-command handler and its cursor and history sub-handler, the submit routine, the space-key mode switch, the channel-mode table construction, the log append routine and its at-bottom test, the unread marker, the key-repeat guard, the opacity application); the chat manager's window creation, window-ID lookup, opacity table and feedback-sound table; the client's language-to-C-runtime-locale switch. **Why M1–M3 were not enough:** the data gives the layout, the defaults and the string tables but no behaviour (parsing, history, routing, caps, key handling); no published source documents the client's command parser; the running client is not observable in this session (M3) and could not show tie-break rules anyway. Only offline UI, text, command and option code was read; channel commands are handled by game scripts, which were not read, and nothing that sends lines to a server, nor anything about network, login, keys, launcher, patcher or anti-tamper, was read. No address, listing or symbol appears in this spec. |
| CEN | The plugin API usage census (`plugin-api-usage.md`, the dirty-room census behind it): which names published plugins pass to `Shell.IsCommand` (counted, not quoted). |
| UL, UB, FO, LPA, KM, ST, PS, REC | The companion specs named in the header. Facts taken from them keep their marks. |
| IB, UR | The accepted Tessera decision records 0014 and 0015 (header). They decide Tessera's behaviour only and are the source of no fact about the original; they are cited by §6.2–§6.4 and §10 (revision 2). |

Marks: **Confirmed** = observed in the install's data (or, for CW38, in the census) for every
record of the kind; **BIN** = rests on the disassembly, even where the data holds the values the
behaviour uses; **Probable** = supported by BIN or data with one step not traced; **Reported** =
third-party only; **OPEN** = unknown, with a callout in the section and a row in §13. A fact that
has an observed part and a behaviour part is split into two rows: *N*a (what the data shows,
Confirmed) and *N*b (what the client does with it, BIN).

| # | Fact | Sources | Mark |
|---|---|---|---|
| CW1 | Layout `chatwindow` holds nine window elements `NewChatWindow1`–`9` with `UI_ChatWindow_WindowID` 1–9; only `NewChatWindow1` has `UI_ChatWindow_PrimaryWindow` true and is based on `NewPrimaryChatWindowTemplate`; the others are based on `NewChatWindowTemplate` (§3.1) | OBS (all 19 top-level elements) | Confirmed |
| CW2a | Only the primary template has the entry line as a child (`ChatEntryElement` with `ChatWindowTextEntry`); only `NewChatWindow1` sets `UI_ChatWindow_PrimaryWindow` (§3.1) | OBS | Confirmed |
| CW2b | Only a window whose `UI_ChatWindow_PrimaryWindow` is true takes the entry line, its handlers and the initial channel mode (§3.1, §6.1) | BIN (window initialisation reads `PrimaryWindow` before taking the entry) | BIN |
| CW3 | The client keeps a pool of the window elements; a saved window ID selects the element whose `UI_ChatWindow_WindowID` equals it; a new window takes the first free element; a used element leaves the pool (§3.2) | BIN (window-ID lookup over the layout's top-level elements; pool take routine) | BIN |
| CW4a | The gameplay option `GOP_ChatWindows` and its parts exist with the types of §3.3 (an array of window property sets, each with ID, visibility and an array of tabs, each a StringInfo name and a 64-bit chat-type mask); `GOP_ChatWindow_TabTextTypes` has no value anywhere (§3.3) | OBS (0x78000006, dictionary 0x34000000, scan of three archives) | Confirmed |
| CW4b | The client loads and saves the chat-window configuration through exactly these properties; a window is saved only if it has at least one tab (§3.3) | BIN (load, save and tab serialisation) | BIN |
| CW5 | Defaults: one window (ID 1, visible) with two tabs; tab 1 shows 40 chat types, tab 2 shows 5 (§3.3, table) | OBS (0x78000006) | Confirmed |
| CW6 | No chat-window configuration is stored in the settings folder (§3.4) | OBS (settings folder: no chat file, no chat key in `UserPreferences.ini`) | Confirmed (this install) |
| CW7a | `UI_ChatOpacities` has 33 entries with disjoint ranges covering 0–100 and consecutive surfaces; `GOP_ChatOpacity` defaults to 25, which lies in the range naming 0x41005F67 (§3.5) | OBS (0x7800000C, 0x78000006) | Confirmed |
| CW7b | The client builds a value → image map from the table and, on reading or changing the option, sets the image (mode value 4) on each window's `LogFieldBackground` and on the primary window's `ChatEntryBackGround` (§3.5) | BIN (opacity table build and application) | BIN |
| CW8a | The default tab masks are 64-bit values whose set bits *i* all have *i* + 1 among the keys of enum 0x230002A8 (bit *i* = chat type *i* + 1, as FO §9.3) (§3.3, CW-I3) | OBS (0x78000006, 0x230002A8) | Confirmed |
| CW8b | "Has all" tests (mask AND *M*) = *M*; "toggle" flips the bits of *M*; a toggle saves the configuration (§4.1, §4.5) | BIN | BIN |
| CW9 | Every log tests each delivered line: it appends the line when the log's mask and the line's mask as delivered (bit 24 included) share at least one bit, and only while a client-wide chat-history object exists and its suppression flag is clear; a refill routine appends stored earlier lines by the same test (§4.3) | BIN (the log's line-receive and refill handlers; the broadcast loop that calls them was not traced) | BIN; order and callers OPEN (Q4) |
| CW10 | Unread marker: a line appended to a tab that is not its window's selected tab shows that tab's `UnreadIndicator` (§4.4) | BIN | BIN |
| CW11 | Tab list layout: horizontal list box, selected tab in state `highlight`, others in `normal`; primary window: up to 2 rows × 9 columns; other windows: 1 row (§4.2) | OBS | Confirmed |
| CW12a | `GOP_FullIRCMode` is a boolean option with default true; `Chat` token 0x0E3E83D5 exists (one piece, no arguments); the layout's `IMTabTemplate` is the element named by `UI_ChatWindow_IMTabElement` (§4.6) | OBS (0x78000006, 0x250001A7, layout 0x22000034) | Confirmed |
| CW12b | When the primary window loads its tabs, the option is true and the window has no instant-message tab, it creates one with that label, mask 0x20, `UI_ChatWindow_IsIMTab` true and not draggable (§4.6) | BIN | BIN |
| CW13 | Nine game objects each add one chat type to the tabs that already show a given type (property set: `Version_AddNewChatTextType_TypeToAdd`, `…_AddToTabsWithType`) and carry that type's default font and colour (§4.7) | OBS (9 of 9 pairs match) | Confirmed (data); application code not found (OPEN) |
| CW14 | Append steps: strip the `Unfiltered` bit, look up style, test "at bottom", drop the oldest line when the log is full, create a line from `UI_ChatLog_TextElement`, filter, timestamp, style, text, log file, scroll, sound, unread marker (§5.1) | BIN | BIN |
| CW15a | `ChatLogListbox` sets `UICore_ListBox_max_rows` 128 (§5.2) | OBS | Confirmed |
| CW15b | The log's line cap is read from that property; the oldest line is removed when a line arrives in a full log (§5.1, §5.2) | BIN | BIN |
| CW16a | `GOP_EnableTimestampsOnChat` is a boolean with default false; `Chat` token 0x0D0F4F30 has the arguments 0x00048685 and 0x00005277, token 0x0938FCCE the arguments 0x013C8E45, 0x00048685 and 0x00005277 (§5.4) | OBS (0x78000006, 0x250001A7) | Confirmed |
| CW16b | When the option is true, the line becomes the first template with the local time and the message; the second template, with the frame clock printed with three decimals and zero-padded to 13 characters, is chosen only when an unidentified condition holds (§5.4) | BIN | BIN; condition OPEN (Q8) |
| CW17 | "At bottom" test and scrolling to the newest line after an append (§5.5) | BIN | BIN |
| CW18 | Page and document keys scroll the shown tab's log; both page keys go through the list box's scroll-to, whose bounds clamp (UW §5.3) makes a negative target 0 because `ChatLogListbox` leaves `UICore_Scrollable_keep_in_bounds` at its default true (§5.5) | BIN (page and document branches, scroll-to routine); OBS (the property is not set in layout `chatwindow`) | BIN |
| CW19a | `ChatMarker_TextTypeFeedback` holds three entries: masks 0x20, 0x400 and 0x800 (types 6, 11, 12) → sounds 0x2A001004, 0x2A001003, 0x2A001003 (§5.6) | OBS (0x78000005) | Confirmed |
| CW19b | After an append the client looks up the line's mask without bit 24 as a whole (exact match, no bit test) and plays the found sound at volume factor 1.0 (§5.6) | BIN (the feedback-sound lookup) | BIN |
| CW20 | Entry line: one line, at most 384 characters (§6.1) | OBS (`UICore_Text_max_chars` 384) | Confirmed |
| CW21 | Chat-window commands: `AcceptInput`, `EscapeKey`, `START_COMMAND`, `START_ALIAS`, `ChatModeReply`, `CursorPreviousLine`, `CursorNextLine`, `CursorPreviousPage`, `CursorNextPage`, `CursorStartOfDocument`, `CursorEndOfDocument`, with the effects of §6.2 | BIN; KM (command IDs) | BIN |
| CW22 | History: one shared list, newest first, at most 19 lines, an identical older line is removed before adding, the browsing index is reset by every addition to the list (not by a submit that adds nothing); older/newer browsing rules (§6.3) | BIN | BIN |
| CW23 | With Shift or Ctrl held, the line-up and line-down commands call game-script callbacks instead of browsing history (§6.4) | BIN | BIN |
| CW24a | All 31 tokens of the mode table exist in the `Commands` table and are name lists; for ten of them the data holds the same (token, chat type) pair in `Communication_ChatChannel_List` of 0x7900021C (types 15, 16, 18, 19, 27, 38, 44, 45, 46, 47); `Communication_LocalizedChannel_List` there has two entries for types 9 and 10 (§6.5) | OBS (0x250001AB, 0x7900021C) | Confirmed |
| CW24b | The mode table itself: the default mode `Say`, the 31 (token, chat type) rows including the 21 the data does not list, the two variant groups and the game-state test between them, the registration of two mode words per name, and the space-key switch rule (§6.5) | BIN | BIN |
| CW25a | `GOP_ChatKeyRepeatProtection` is a boolean option with default false (§6.6) | OBS (0x78000006) | Confirmed |
| CW25b | The guard's steps, its 1.0 s window, the "more than 10 repeats" limit and UI sound 0x50 (§6.6) | BIN | BIN |
| CW26 | One command registry per client; names compared case-insensitively; `Shell.IsCommand` is a pure lookup of the name as given (§7.1, §8.2) | BIN | BIN |
| CW27a | `CommandSetup` (STRINGTABLE key 2) has the tokens 0x0DA28738, 0x02409858 and 0x04DA4989, each one piece without arguments; the first two are one character each (U+002F and U+003B in this install) (§7.2) | OBS (0x25000200, 0x28000004) | Confirmed |
| CW27b | Their roles: command prefix, substitution marker and default channel word (§7.2, §7.5) | BIN | BIN |
| CW28 | Execution pipeline: history, substitution test and expansion, whitespace trim, prefix test, prefix strip or default-channel line, execution (§7.2) | BIN | BIN |
| CW29 | Whitespace set: space, line feed, carriage return, tab, form feed (§7.2) | BIN | BIN |
| CW30 | Tokenizer: whitespace-separated tokens; a double quote at a token start opens a quoted token that keeps its inner whitespace; the two quirks of §7.3 | BIN | BIN |
| CW31 | Command word = first token as typed, looked up case-insensitively; arguments = the line with the command word's leading characters stripped, then whitespace-trimmed (§7.4) | BIN | BIN |
| CW32a | `CommandSetup` token 0x04FE7F13 exists (one piece, no arguments); token 0x00E5CD23 exists with one argument 0x000059D4 (§7.6) | OBS (0x25000200) | Confirmed |
| CW32b | Result codes 2, 3, 5 and the handler's code (§7.4); on result 3 the submit posts token 0x04FE7F13 as a line of mask 0x1; token 0x00E5CD23 appears nowhere in the client code (§7.6) | BIN (submit routine; immediate search of the executable) | BIN |
| CW33 | `;word` substitution: trigger test, expansion through a game-script callback, recursion guard (§7.5) | BIN | BIN |
| CW34 | Plugin commands allow substitution (§7.5, §9) | BIN | BIN |
| CW35a | The `Commands` table (STRINGTABLE key 0x10000007) has 261 entries here, 238 of them name lists (one or more names separated by `;`, `:` or `,` with optional whitespace; 25 with several names) and 23 message templates (§8.1) | OBS (0x250001AB) | Confirmed |
| CW35b | Built-in registration splits an entry at those separators, trims each name and ignores empty names (§7.1, §8.1) | BIN (alias splitting and trimming) | BIN |
| CW36 | Most built-in commands are registered by game scripts at run time; the script record references 217 distinct tokens under the `Commands` key, the client's channel table 31 (29 of them also by scripts) (§8.1) | OBS (script record 0x56000000); BIN (script natives reach the registration routines) | Probable |
| CW37 | The client sets its C-runtime locale from its language (English, British English, French, German) before plugins run; name comparison folds case under that locale (§8.2) | BIN; general C-runtime knowledge | Probable |
| CW38 | Plugins probe `IsCommand` with localized names of the help and plugin-manager commands to detect the client language (§8.3) | CEN; LPA §11.1 (S5) | Confirmed (census) |
| CW39 | `ShellCommand:Execute(command, arguments)`: `command` = first token as typed, `arguments` = §7.4 arguments (§9) | BIN; LPA P61 | BIN |
| CW40 | The instant-message tab, sessions and their lists use `IMTabTemplate`, `IMLogElement`, `IMSessionsListboxItemTemplate` (§4.6) | OBS | Confirmed (layout); behaviour OPEN |
| CW41 | Lines whose mask has more than one type bit (the `ChatModeReply` line, mask 0x21): style from the highest set bit that has a style entry (FO §9.3 step 1), appended once to every log whose mask shares any bit with it, a feedback sound only on an exact whole-mask match; the reply line and the unknown-command line are handed with their 64-bit mask to the same game-script entry point, which delivers them back to the chat (§2, §4.3, §5.1, §5.6, §6.2, §7.6) | BIN (post routine to the script hand-over, the log's receive handler, the style and sound lookups); FO §9.3 | BIN; the script's hand-back Probable (Q17) |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Chat type** | A key of the chat-type enum mapper 0x230002A8 (base 0x230002A9; the `ChatType` values of LPA Appendix A are a subset). Examples: 1 `Error`, 4 `Standard`, 5 `Say`, 6 `Tell`, 8 `Log`, 25 `Unfiltered`. |
| **Type mask** | A 64-bit bitfield where bit *i* (from 0) stands for chat type *i* + 1 (FO §9.3; PS code 14). A tab's mask usually has many bits. A line's mask usually has one type bit, optionally plus the `Unfiltered` bit (bit 24), but the client also posts lines with more than one type bit (the `ChatModeReply` line of §6.2 has mask 0x21, types 1 and 6). Every rule of this spec is stated for a mask with any number of bits: §4.3 (which logs take it), §5.1 step 2 (style), §5.6 (sound). |
| **Window** | One chat panel: an instance of one of the nine window elements of layout `chatwindow`. |
| **Primary window** | The window built from `NewChatWindow1` (`UI_ChatWindow_PrimaryWindow` true). Only it has the entry line. |
| **Tab** | One entry of a window's tab list; it owns one log and one type mask. |
| **Log** | The list of lines of one tab (a `ChatLog` element with a list box of line elements). |
| **Entry line** | The editable one-line text element `ChatWindowTextEntry` of the primary window. |
| **Channel mode** | The chat type that plain (non-command) text is sent to; shown in the `OutputTextType` label. |
| **Registry** | The client's single table of command names → handlers (built-in and plugin). |
| **Command prefix** | The text that marks a line as a command (`CommandSetup` token 0x0DA28738; one solidus character in this install). |

---

<a id="s3"></a>
## 3. Which chat windows exist

<a id="s3-1"></a>
### 3.1 The layout (Confirmed: CW1, CW2a; BIN: CW2b)

Layout `chatwindow` (UILAYOUT key 0x10000007, record 0x22000034 here) has 19 top-level
elements; the table names all of them:

| Element (ID) | Type | Role |
|---|---|---|
| `NewPrimaryChatWindowTemplate` (0x10001389) | `ChatWindow` | Template of the primary window: tab list, log field, entry line |
| `NewChatWindowTemplate` (0x1000030A) | `ChatWindow` | Template of the other windows: tab list and log field, no entry line |
| `NewChatWindow1` (0x1000031E) | (based on the primary template) | Window ID 1, primary |
| `NewChatWindow2`–`5` (0x10000320–0x10000323), `NewChatWindow6`–`9` (0x10004090–0x10004093) | (based on `NewChatWindowTemplate`) | Window IDs 2–9 |
| `ChatTabTemplate` (0x10000318) | `ChatTab` | One tab (text, unread and logging markers, three frame pieces) |
| `IMTabTemplate` (0x10000324) | `IMTab` | The instant-message tab |
| `ChatLogElement` (0x10000315) | `ChatLog` | One log: `ChatLogListbox` (`ListBox`) and `ChatLogScrollbar` |
| `ChatLogListboxItemTemplate` (0x10000309) | `Text` | One log line |
| `IMLogElement` (0x10000325), `IMChatLogElement` (0x1000032E), `IMSessionsListboxItemTemplate` (0x1000032B) | `IMLog`, `IMChatLog`, `IMSessionLog` | Instant-message log, per-session log, session list row |
| `ChatDisplayText` (0x10000308) | `Text` | A text element no window property names (§13 Q14) |

Each window element sets `UI_ChatWindow_WindowID` (1–9), `UICore_Element_save_loc` and
`save_size` true, `min_width` 256, `min_height` 128, `max_width` 1024, `max_height` 1024, and
has a `ChatWindow_Resize` resize bar (16 × 16, bottom and right borders). Its design rectangle
and edges are UL §17 P2 (window 1: (5, 444), 313 × 199, edges (5, 4, 4, 5); windows 2–9:
(72, 79), 313 × 199).

The window templates name their parts through properties (each value an element ID):

| Property | Primary template | Other template | Names |
|---|---|---|---|
| `UI_ChatWindow_TabElement` | 0x10000318 | 0x10000318 | the tab template |
| `UI_ChatWindow_IMTabElement` | 0x10000324 | 0x10000324 | the instant-message tab template |
| `UI_ChatWindow_LogParentField` | 0x1000030D | 0x1000030D | `LogField`, the parent of the shown log |
| `UI_ChatWindow_EntryElement` | 0x10000310 | 0x10000310 | `ChatEntryElement` (absent as a child in the other template) |
| `UI_ChatWindow_TextEntryElement` | 0x10000313 | — | `ChatWindowTextEntry` |
| `UI_ChatWindow_OutputTextElement` | 0x10000312 | 0x10000312 | `OutputTextType`, the channel label |
| `UI_ChatWindow_OutputButtonElement` | 0x10000311 | 0x10000311 | `OutputFilterButton`, opens the channel menu |

Both templates set `UIComponent_UIScaler_Group` 0x2D with origin (0, 0), `activatable` and
`activate_on_show` true. Group 0x2D is the chat scaling group: the UI-scale definition set
(UB §9; 0x78002E53 here) gives it the option `GOP_UIScale_Chat`, range 0.4–4.0, 73 stops (OBS).

**Primary template children** (design 512 × 496): `TabListBox` (512 × 46 at the top),
`TabScrollbar`, `LogField` (from y 46, 512 × 431, with `LogFieldBackground` 512 × 430 and a
2-pixel `LogFieldBackground_Separator` at y 429), `ChatEntryElement` (512 × 20 at y 476) holding
`OutputFilterButton` (16 × 16 at (2, 3)), `OutputTextType` (81 × 20 at (18, 0)),
`ChatWindowTextEntry` (394 × 20 at (100, 0)) and `ChatEntryBackGround` (512 × 20). The other
template has the tab list (512 × 23), the scroll bar and the log field (from y 23, 512 × 473).

When a window initialises, it reads `UI_ChatWindow_PrimaryWindow`; only when it is true does
it take the entry line, the entry's handlers and the initial channel mode (§6.5) (BIN, CW2b). Of
the nine window elements only `NewChatWindow1` sets it (Confirmed, CW2a).

<a id="s3-2"></a>
### 3.2 Window IDs and the element pool (BIN, CW3)

1. The chat manager keeps a **pool** of window element IDs: the top-level elements of layout
   `chatwindow` that carry `UI_ChatWindow_WindowID`.
2. To create the window with saved ID *n*, it walks the top-level elements of the layout and
   takes the element whose `UI_ChatWindow_WindowID` equals *n*. That element must still be in
   the pool; it is removed from the pool and the window is created from it (the
   create-element path of UL §9.2, no parent).
3. A request for a new window without an ID takes the first element still in the pool.
4. So at most nine windows exist at once, window ID *n* always uses element `NewChatWindow`*n*,
   and window 1 is the only one with an entry line.
5. After creating a window from the configuration, the manager loads its tabs (§3.3), records
   the primary window as the receiver of the chat input commands (§6.2), and, when the player's saved
   panel positions are in use, applies the saved location (UL §9.9, UB §8).

<a id="s3-3"></a>
### 3.3 Configuration and defaults (Confirmed: CW4a, CW5; BIN: CW4b)

The configuration is the gameplay option `GOP_ChatWindows` (PS code 12, an array):

| Property | Type | Meaning |
|---|---|---|
| `GOP_ChatWindows` | array of `GOP_ChatWindow` | One entry per window, in creation order |
| `GOP_ChatWindow` | property set | One window |
| `GOP_ChatWindow_WindowID` | integer | Window ID (§3.2) |
| `GOP_ChatWindow_Visibility` | boolean | Whether the window is shown |
| `GOP_ChatWindow_Tabs` | array of `GOP_ChatWindow_Tab` | Tabs, in tab-list order |
| `GOP_ChatWindow_Tab` | property set | One tab |
| `GOP_ChatWindow_TabName` | StringInfo | Tab label (a string-table reference, or literal text for a name the player typed) |
| `GOP_ChatWindow_TabChatTextTypes` | 64-bit bitfield over 0x230002A8 | The tab's type mask |

`GOP_ChatWindow_TabTextTypes` (an older 32-bit field over enum 0x23000025) exists in the
dictionary but has no value anywhere in the data.

**Loading** (BIN, CW4b; per configured window): find the element by ID (§3.2), create the window, then
for each `GOP_ChatWindow_Tab` create a tab from `UI_ChatWindow_TabElement`, set its label from
`GOP_ChatWindow_TabName` and its mask from `GOP_ChatWindow_TabChatTextTypes`. **Saving**: for
each tab of a window, write the label and the mask back; a window is written with its ID and
its tabs only if it has at least one tab.

**Defaults** (`GameplayOptionDefaults`, UNIQUEDB key 0x68, property set 0x78000006 here):

| Window | Visible | Tab | Label | Mask (hex) | Chat types shown |
|---|---|---|---|---|---|
| 1 | true | 1 | `UI` table (STRINGTABLE key 0x10000065) token 0x0554A31C | 0x00007FF9F8FEFFFD | 1, 3–16, 18–24, 28–33, 36–47 (40 types: every key of 0x230002A8 except 0, 2, 17, 25, 26, 27, 34, 35) |
| 1 | | 2 | `UI` table token 0x0918F32C | 0x0000000600880004 | 3, 20, 24, 34, 35 |

So by default the original shows **one** chat window (ID 1, the primary) with two tabs, plus
the instant-message tab of §4.6. A player's own `GOP_ChatWindows` replaces the defaults.

<a id="s3-4"></a>
### 3.4 Where the live configuration lives (Confirmed for this install, CW6)

The owner's settings folder (`install-layout.md` §8) holds no chat-window file, and `UserPreferences.ini` has no
chat key. The live `GOP_ChatWindows` value is therefore not kept locally in this install
(like the chat font table, FO §9.3 Q9).

> **OPEN (Q1):** where the original keeps a player's `GOP_ChatWindows` (most likely with the
> character on the server, which this project does not study), and whether the XML options
> export of FO §9.3 includes it. Tessera's rule is in §10.

<a id="s3-5"></a>
### 3.5 Background opacity (Confirmed: CW7a; BIN: CW7b)

`UI_ChatOpacities` (UNIQUEDB key 0x1000000C, property set 0x7800000C here) is an array of
`UI_ChatOpacity_Struct`, each with `UI_ChatOpacity_MinRange`, `UI_ChatOpacity_MaxRange`
(inclusive integers) and `UI_ChatOpacity_ImageDataID` (a surface DID). Here: 33 entries with
disjoint ranges that together cover 0–100 exactly, naming the consecutive surfaces
0x41005F5F–0x41005F7F (Confirmed, CW7a). Steps 1–2 are BIN (CW7b):

1. The manager builds a map value → image DID from every entry (each integer of each range).
2. When the option `GOP_ChatOpacity` (integer, default 25) is read or changed, it looks up the
   value. If there is an image for it, it sets that image (the call passes mode value 4) on
   the `LogFieldBackground` element (0x1000030E) of every window, and on the
   `ChatEntryBackGround` element (0x10000314) of the primary window.
3. Default: 25 lies in the range 24–26, image 0x41005F67.

> **OPEN (Q2):** what the mode value 4 passed with the image means (the draw modes of UL §9.6
> and §9.6.1) and
> whether the image replaces the element's layout media or is drawn as an extra layer.

---

<a id="s4"></a>
## 4. Tabs and filters

<a id="s4-1"></a>
### 4.1 The tab model (Confirmed: CW8a; BIN: CW8b)

A tab has a label (StringInfo, ST) and a log; the log holds the tab's type mask (a 64-bit mask
over 0x230002A8, as every default mask in the data is: CW8a). Two operations are used by the
tab's context menu (BIN, CW8b):

- **has all**: a set of types *M* is "shown" when (mask AND *M*) = *M*;
- **toggle**: mask ← mask XOR *M*; after a toggle the manager saves the configuration (§3.3).

<a id="s4-2"></a>
### 4.2 The tab list (Confirmed, CW11)

`TabListBox` (`ChatWindowTabListBox`) is a list box with `UICore_ListBox_horizontal` true,
`click_select` true, `item_selected_state` 7 (`highlight`), `item_normal_state` 11 (`normal`),
`selected_item_state_change` true, `notify_on_add` true, `drag_rollover` true and a horizontal
scroll bar (`TabScrollbar`). In the primary template it also sets `max_rows` 2,
`max_columns` 9, `flipped_layout` true, `ignore_cell_alignment` true and
`wrap_indentation_amount` 20; in the other template `max_rows` 1.

A tab (`ChatTabTemplate`) is 100–216 pixels wide and 23 pixels high, draws its label with
`UICore_Text_font` 0x42000007, outline on, one line, left and right margins 16, both
justifications 3, truncation with a suffix and an automatic tooltip of the cut text (UB §10,
FO §7.6), `UICore_Element_DrawAfterChildren` true, a context menu, and seven states
(`highlight_pressed`, `normal_pressed`, `highlight`, `normal_rollover`, `normal`,
`highlight_rollover`, `ghosted`) that change the label colour and the frame images. The
`UnreadIndicator` and `LoggingIndicator` children are hidden in the layout. The instant-message
tab is the same but 100–200 wide and has no logging marker.

Selecting a tab (click) puts it in `highlight` and shows its log in the window's
`UI_ChatWindow_LogParentField`; the window's page and document keys (§5.5) act on that log.

> **OPEN (Q3):** the context-menu items of a tab and of the channel button (new tab, rename,
> close, filters, new window), the drag of a tab to another window, and how the shown log is
> re-parented on selection. These are game-code behaviours; Tessera's M5 rule is in §10.

<a id="s4-3"></a>
### 4.3 Which tab gets a line (BIN, CW9, CW41)

A line arrives with a type mask *L*: one or more type bits, plus bit 24 when it is `Unfiltered`
(§2). Every log (the log of each tab of each window) is offered the line with *L* exactly as
delivered, bit 24 included, and makes its own decision:

1. If the client-wide chat-history object does not exist, or its suppression flag is set, the
   log ignores the line. (This object also keeps the list of earlier lines used by the refill
   below; what sets the flag was not identified, OPEN Q4.)
2. Let *M* be the log's (the tab's) mask. If *M* AND *L* is zero, the log ignores the line.
3. Otherwise the log appends the line once (§5.1), with *L* as delivered.

Consequences:

- The test is **any bit**, not all bits: a line with several type bits is appended to every tab
  that shows at least one of its types, and it is appended **once** per tab however many of its
  bits the tab shows. A tab never gets two copies of one delivered line.
- Bit 24 takes part in the test: an `Unfiltered` line also reaches a tab whose mask has bit 24
  but none of the line's type bits. No default mask has bit 24 (§3.3), and Tessera's tab menu
  does not offer it (§10 item 1).
- A line whose types no tab shows appears nowhere (it still reaches plugins, LPA §11.3).

**Refill (BIN).** A log can also be filled from the chat-history object's list of earlier
lines, each stored with its mask: walking the list from its first entry, every stored line
whose mask passes step 2 is appended (§5.1). This routine checks only that the object exists,
not the flag of step 1. When the client calls it (for example for a new tab, a changed filter
or a re-created window) was not traced.

> **OPEN (Q4, narrowed):** (a) the order in which the logs are offered a line (window order,
> tab order or registration order: the broadcast loop that calls the per-log test was not
> traced); (b) which object owns the suppression flag of step 1 and when it is set; (c) when
> the refill runs, and what the history list holds (how many lines, and whether lines with
> bit 24 are kept); (d) whether the instant-message log is offered lines by the same broadcast
> (Probable: its class uses the same per-log test, so with its mask 0x20 it takes every line
> that has the `Tell` bit, in addition to normal tabs). Tessera rule: offer each line to the
> windows in window-ID order and within a window to the tabs in tab-list order; no suppression
> flag; no refill (a new or changed tab starts empty or keeps what it has, §4.5).

<a id="s4-4"></a>
### 4.4 Unread marker (BIN, CW10)

After appending a line to a tab's log: if that tab is not the selected tab of its window, the
tab's `UnreadIndicator` child is shown. The marker is hidden when the tab is selected (Probable;
the clearing call was not traced).

<a id="s4-5"></a>
### 4.5 Changing filters

A filter change toggles bits (§4.1) and saves `GOP_ChatWindows`. Lines already in the log stay
(the log is not rebuilt). (BIN for the toggle and save, CW8b; "not rebuilt" is Probable: no
rebuild or refill call follows the toggle in the toggle routine, but the callers of the refill
of §4.3 were not traced, Q4.)

<a id="s4-6"></a>
### 4.6 The instant-message tab (Confirmed: CW12a, CW40; BIN: CW12b)

The option `GOP_FullIRCMode` is a boolean with default true, and the label token below exists
(Confirmed, CW12a). The creation rule is BIN (CW12b): when the primary window loads its tabs,
and `GOP_FullIRCMode` is true, and the window has no instant-message tab yet, it creates one from
`UI_ChatWindow_IMTabElement` with the label `Chat` table (STRINGTABLE key 0x1000004D) token
0x0E3E83D5 and the type mask 0x20 (type 6, `Tell`), marked with `UI_ChatWindow_IsIMTab` true
and not draggable. Its log is `IMLogElement`: a session list (`IMSessionsListbox`, one row per
conversation partner from `IMSessionsListboxItemTemplate`, each with close button, unread and
logging markers) beside a per-session log (`IMChatLogElement`).

> **OPEN (Q5):** the instant-message session model (when a session row is created, which lines
> go to which session, closing). Not needed in the M5 sandbox (no other players).

<a id="s4-7"></a>
### 4.7 Chat types added after release (Confirmed data, CW13)

Nine game objects (0x70035169, 0x70035199–0x7003519B, 0x70040072, 0x70047DC0–0x70047DC3) each
carry one `GOP_ChatTextTypeFontColorStruct` (FO §9.3), and their property sets (prefix 0x79,
PS §6.2) carry `Version_AddNewChatTextType_TypeToAdd` and
`Version_AddNewChatTextType_AddToTabsWithType` (both 64-bit masks). In all nine, the
struct's chat type equals `TypeToAdd`:

| Type added | Added to tabs that show |
|---|---|
| 34 `CombatPlayer`, 35 `CombatEnemy` | 2 `Combat` |
| 36 `Loot`, 37 `FellowLoot`, 38 `World`, 39–42 `UserChat5`–`8` | 4 `Standard` |

They read as migration steps for saved configurations: when a new chat type was introduced,
add it to every tab that already shows the related type, with this default font and colour.
The defaults of §3.3 already contain all nine types.

> **OPEN (Q6):** the client code never names these property IDs as constants; which code (or
> server) applies the migration, and when, was not found. This also answers part of FO §9.3
> Q10 (which data refers to the nine objects: their own property sets).

---

<a id="s5"></a>
## 5. The log

<a id="s5-1"></a>
### 5.1 Appending a line (BIN, CW14)

Input: the line's type mask *L*, its text (with markup, ST §4.7), and the tab whose log it is.

1. If *L* has bit 24 (`Unfiltered`), remember that and remove the bit; *T* = the remaining mask.
2. Look up the line style (size step and colour) for *T* by FO §9.3 step 1: of the bits set in
   *T*, the **highest** one that has an entry in the chat-type style map decides; with no such
   bit the line is Small and opaque white. For a one-bit *T* this is that type's style; for
   0x21 (types 1 and 6) it is the style of type 6 when type 6 has an entry (it has one in the
   defaults here), else that of type 1.
3. Note whether the log is **at the bottom** (§5.5) before adding.
4. If the log already holds the cap (§5.2) or more lines, remove the oldest line (the first).
5. Create a line element from the log's `UI_ChatLog_TextElement` (element 0x10000309,
   `ChatLogListboxItemTemplate`, of layout `chatwindow`) and add it at the end of the log's list
   box.
6. Text filters: if the line was not `Unfiltered` and *T* is one of the player-speech types,
   pass the text through the profanity filter (option `GOP_ProfanityFilterEnabled`, default
   true; its word list is game data and server policy, out of scope); then pass every line
   through a second text pass of the same routine with a different flag.
7. If `GOP_EnableTimestampsOnChat` is true, wrap the text with a timestamp (§5.4).
8. Apply the style of step 2 to the line element (FO §9.3 steps 2–4) and set its text.
9. If the tab is logging to a file, append the text and a line break to that file.
10. If step 3 found the log at the bottom, scroll so that the newest line is shown.
11. Play the feedback sound for *T* (§5.6; whole-mask match, so a multi-bit *T* usually has no
    sound).
12. Show the tab's unread marker (§4.4).

The line template (`ChatLogListboxItemTemplate`) is a `Text` element with
`UICore_Text_parse_markup_tags` true, `one_line` false (lines wrap), `fit_to_text` 2,
`selectable` true, `editable` false, `trim_trailing_white_space` true, outline on, margins
left 2, right 2, bottom 2, `max_chars` 128,000, `enable_hover_on_texttag_creation` true, and the
six chat fonts of FO §9.2/§9.3; so markup in a line (links, colours) is interpreted, and a line
can be several rows high.

> **OPEN (Q7):** which chat types count as player speech in step 6 (the client tests a fixed
> set of types; Tessera has no other players and applies no filter), and what the second pass
> of step 6 does (Probable: an escape or normalisation step that does not change plain text).

<a id="s5-2"></a>
### 5.2 Line cap (Confirmed: CW15a; BIN: CW15b)

The cap is the integer property `UICore_ListBox_max_rows` of the log's list box
(`ChatLogListbox`) (BIN, CW15b): 128 here (Confirmed, CW15a). A log never holds more than
the cap; when a line arrives in a full log, the oldest line is removed first (BIN). The cap
counts lines (elements), not wrapped rows.

<a id="s5-3"></a>
### 5.3 Font and colour

As FO §9.3: per chat type a size step and a colour from the gameplay option
`GOP_ChatTextTypeFontColorArray` (defaults in 0x78000006), size step → font through
`UI_ChatFonts`. Each line keeps the font and colour it was appended with.

<a id="s5-4"></a>
### 5.4 Timestamps (Confirmed: CW16a; BIN: CW16b)

The option, its default and the two templates with their argument IDs are in the data
(Confirmed, CW16a); how the client uses them is BIN (CW16b). When `GOP_EnableTimestampsOnChat`
(boolean, default false) is true, the line text becomes the
`Chat`-table (STRINGTABLE key 0x1000004D) template token 0x0D0F4F30 with argument 0x00048685 =
the current local time as a string and argument 0x00005277 = the message. A second template,
token 0x0938FCCE, adds argument 0x013C8E45 = the client's frame clock in seconds printed with
three decimals and zero-padded to 13 characters; the client uses it only when a condition the
spec does not identify is true.

> **OPEN (Q8):** the exact local-time format the client passes (it comes from a general time
> formatter), and the condition that selects the second template (Probable: a developer
> setting). Tessera: 24-hour `HH:MM:SS` local time with the first template.

<a id="s5-5"></a>
### 5.5 Scrolling (BIN, CW17, CW18)

**At bottom.** With *o* = the log's scroll offset, *v* = its visible height, *c* = its content
height, and *h* = the height of the last line when the log has more than one line (else 0),
the log is at the bottom when *o* + *v* ≥ *c* − *h*. So a log counts as at the bottom while any
part of its last line is on screen. After an append (§5.1 step 10), a log that was at the bottom
scrolls to show the new last line; a log the player has scrolled up stays where it is.

**Keys** (the window's command handler, acting on the log of the shown tab):

| Command | Effect |
|---|---|
| `CursorNextPage` | target ← min(*o* + *v*, *c* − *v*); scroll to it (below) |
| `CursorPreviousPage` | target ← max(*o* − *v*, 0); scroll to it (below) |
| `CursorStartOfDocument` | if the log has lines, scroll to the first line |
| `CursorEndOfDocument` | if the log has lines, scroll to the last line |

Both page keys pass their target to the list box's scroll-to (UW §5.3), not forced, with the
horizontal offset unchanged. `ChatLogListbox` does not set `UICore_Scrollable_keep_in_bounds`,
so it keeps its default true and UW §5.3 step 1 applies: the target is limited to the list
box's content height minus its height (*Ch* − *H* in UW) and then becomes 0 if it is 0 or less. So when the content is shorter than the view
(*c* < *v*), `CursorNextPage` gives *c* − *v* < 0, which the scroll-to turns into 0: the log
stays at the top (BIN, CW18). Tessera applies the same clamp: a page target is never below 0 or
above max(*c* − *v*, 0).

The wheel follows UB §4.7 (it bubbles to the log's list box, which steps its vertical scroll
bar by one line per notch); list scrolling, the scroll bar and its line and page steps are
UW §5, §6 and §8 (Scrollable, Scrollbar, ListBox).

<a id="s5-6"></a>
### 5.6 Feedback sounds (Confirmed: CW19a; BIN: CW19b)

`ChatMarker_TextTypeFeedback` (UNIQUEDB key 0x10000017, property set 0x78000005 here) is
`UI_Chat_ChatTextType_FeedbackSounds`, an array of `UI_Chat_ChatTextType_FeedbackSound`, each a
type mask `UI_Chat_ChatTextType` and a sound DID `UI_Chat_FeedbackSound` (prefix 0x2A). Here
(Confirmed, CW19a): mask 0x20 (type 6 `Tell`) → 0x2A001004; masks 0x400 (type 11 `Fellowship`)
and 0x800 (type 12 `Guild`) → 0x2A001003.

Lookup (BIN, CW19b): the client keys a map by the whole 64-bit mask of each entry. After an
append (§5.1 step 11) it looks up *T*, the line's mask without bit 24, as one value: only an
entry whose mask **equals** *T* matches; no bit of *T* is tested on its own. If an entry
matches, its sound is played at volume factor 1.0 (sound.md); otherwise nothing is played. So
a `Tell` line (0x20, or 0x01000020 with `Unfiltered`) plays 0x2A001004, while the
`ChatModeReply` line (0x21, §6.2) matches no entry here and is silent.

<a id="s5-7"></a>
### 5.7 Logging to a file

A tab can log its lines to a text file; while it does, its `LoggingIndicator` is shown and each
appended line is written followed by a line break. Starting and stopping post a status line of
type 8 to the tab. File name, folder and encoding are not specified here (OPEN Q9); Tessera
does not offer chat logging in M5.

---

<a id="s6"></a>
## 6. The entry line

<a id="s6-1"></a>
### 6.1 Element (Confirmed, CW20)

`ChatWindowTextEntry` (`PlayerChatTextEntry`) is an editable, selectable, one-line `Text`
element with `UICore_Text_max_chars` 384, outline on, left and right margins 2, horizontal
justification 5, vertical 3, the six chat fonts and the nine-colour list of the layout. Only the
primary window has it (§3.1). Its key behaviour is the text-entry table of UB §5.4 plus the
chat-window commands below; the chat window handles the commands of §6.2 before the text
widget would.

<a id="s6-2"></a>
### 6.2 Commands the chat window handles (BIN, CW21)

"The entry has focus" means the UI's keyboard focus element (UB §5.2) is the entry.

| Command (KM §12) | Entry has focus | Entry does not have focus |
|---|---|---|
| `AcceptInput` | Release focus; **submit** (below) | Give the entry focus (open the line) |
| `EscapeKey` | Release focus; clear the text and selection; restore the channel mode from the shown tab (§6.5) | Not handled (the command goes on to game code) |
| `START_COMMAND`, `START_ALIAS` | Give the entry focus; clear its text and selection | Same |
| `ChatModeReply` | Not handled | Ask game code for the reply target (§6.4); if there is none, post `Chat`-table token 0x00CC89CC as a line of mask 0x21 (types 1 `Error` and 6 `Tell`; see below); else give the entry focus and hand the name to the window's tell-target holder (Probable: this also selects the `Tell` mode) |
| `CursorPreviousLine`, `CursorNextLine` | History (§6.3), or with Shift or Ctrl §6.4 | — |
| `CursorPreviousPage`, `CursorNextPage`, `CursorStartOfDocument`, `CursorEndOfDocument` | Scroll the shown log (§5.5) | Same |

The `START_COMMAND` and `START_ALIAS` handlers do not insert any character themselves.

> **OPEN (Q19):** which input map supplies the paging commands of the last row while the
> entry does not have focus. `EditControls`, which binds them, is the map of a focused text
> element (UB §5.2 OPEN), and no layout element names an input map (UB §15). Suggested method:
> M4 on the chat window's activation (which maps it pushes), or M3 (press Page Up in the
> original with the chat window active and the entry closed, and see whether the log pages
> or the camera zooms). Tessera rule: item 10 of §10.

**Tessera (IB D2, D5).** In Tessera the commands of this table are actions of its key map that
carry these command names (IB D1). With entry focus they come from the `text_entry` context
(`EditControls`), which the focused entry pushes at priority 50: Enter and numeric Enter give
`open_chat` (`AcceptInput`), Escape gives `menu` (`EscapeKey`), and line up and down give
`text_line_up` / `text_line_down` (`CursorPreviousLine` / `CursorNextLine`). A command that
the window does not handle goes on to game code as the same action (IB D7). Without
entry focus, the paging commands come from Tessera's own context `ui.chat_log` (§10 item 10).

**Lines the chat window posts (BIN, CW41).** The reply line above and the unknown-command line
of §7.6 are not appended to a log directly. The window hands the line's text and its 64-bit
mask (0x21 for the reply line, 0x1 for the unknown-command line) to one game-script entry point
of the game layer, which shows them as chat lines like other system messages; the line then
comes back to the logs through §4.3 with that mask (Probable: the scripts were not read, Q17).
With mask 0x21 the line is treated by the general rules for any mask: it is appended once to
every tab that shows type 1 **or** type 6 (§4.3), it is styled as type 6 when type 6 has a style
entry (§5.1 step 2), and it has no feedback sound unless an entry's mask is exactly 0x21 (§5.6).
Worked example: §11.8.

> **OPEN (Q17):** whether the game script that receives these two lines passes the mask back
> to the chat unchanged (it may, for example, reduce 0x21 to one type), and whether it adds
> anything to the text. Suggested method: M4 on the script entry point (script table), or M3
> (press the reply key with no tell received and look at the line's colour and which tabs show
> it). Tessera rule: §10 item 8 (the mask as handed over, the text unchanged).

**Submit:**

1. Read the entry text. If it is empty, stop: nothing else changes (the history and its
   browsing index of §6.3 stay as they are; focus was already released).
2. Take the window's channel mode (§6.5): its command word *C* and, for `Tell`, the stored
   target name. *C* comes from a map chat type → command word (StringInfo) that the game layer
   fills when the window is created (not traced, OPEN Q12); when the map has no entry for the
   mode, *C* is empty and §7.2 uses the default word.
3. Run the command line (§7.2) with the text, the mode's command word and the target.
4. If the result is 3 (unknown command), post the unknown-command line (§7.6).
5. Clear the entry text and restore the channel mode from the shown tab (§6.5).

> **OPEN (Q10):** whether the character of the key bound to `START_COMMAND` (or `START_ALIAS`)
> reaches the newly focused entry as typed text (so that the line starts with the prefix), as
> the command name suggests. The handler only focuses and clears. Suggested method: M3 (press
> the key in the original client). Tessera rule: `START_COMMAND` opens the entry with the
> command prefix typed; `START_ALIAS` opens it empty.

<a id="s6-3"></a>
### 6.3 History (BIN, CW22)

There is one history list for the whole client (it belongs to the command registry, §7.1).

- **Capacity** 20. After every addition, the oldest entries are removed while the count is at
  least the capacity, so the list holds **at most 19** lines.
- **Adding** happens at the start of every command-line run (§7.2 step 1) when the submitted
  text is not empty: first an existing entry with exactly the same text (case-sensitive) is
  removed, then the text is inserted as the newest entry, then the list is trimmed. The text is
  added as typed, before substitution and trimming, whether it is a command or a plain line.
- **Browsing index** *k*: −1 means "not browsing". Every addition resets *k* to −1. A submit
  that adds nothing leaves *k* unchanged: an empty entry (§6.2 submit step 1) and a run that
  returns 2 at §7.2 step 1 (no registry or no game context) do not reach the addition. So after
  browsing to an entry, deleting the text and pressing Enter, the next older-browse continues
  from *k* + 1 (§11.4).
- **Older** (`CursorPreviousLine` without Shift or Ctrl): *k* ← min(*k* + 1, count − 1). If
  *k* ≥ 0, the entry text becomes history entry *k* (0 = newest) and the caret is moved
  (Probable: to the end of the text; the caret call was not traced).
- **Newer** (`CursorNextLine` without Shift or Ctrl): *k* ← *k* − 1. If *k* < 0, *k* ← −1 and
  **nothing changes** (the recalled text stays in the entry). Otherwise the entry text becomes
  history entry *k*.
- Browsing acts only while the entry has focus.

In Tessera, history is the chat window's reading of the `text_line_up` / `text_line_down`
actions (`CursorPreviousLine` / `CursorNextLine`); there is no separate history action
(IB D2; the earlier `text_history_prev` / `text_history_next` are retired).

<a id="s6-4"></a>
### 6.4 Shift and Ctrl with line up and down (BIN, CW23)

While the entry has focus, `CursorPreviousLine` and `CursorNextLine` check the modifiers first:

| Command | Shift held | else Ctrl held |
|---|---|---|
| `CursorPreviousLine` | game-script callback 0x01D60032 | callback 0x01D60030 |
| `CursorNextLine` | callback 0x01D60031 | callback 0x01D6002F |

Each callback returns a name (or nothing); a returned name is handed to the window's
tell-target holder (Probable: it becomes the `Tell` target of the entry line). `ChatModeReply` (§6.2) and the reply word of §6.5 use callback 0x01D60032; the
"tell again" word uses 0x01D60030.

> **OPEN (Q11):** the meaning of the four callbacks (Probable: Shift cycles through the players
> who sent the last tells, Ctrl through the players the player last told). They are game
> scripts; the M5 sandbox has no tells and Tessera makes these keys do nothing.

In Tessera the line actions match with extra modifiers held (IB D2, "Modifier policies":
Subset), so Shift+Up and Ctrl+Up give `text_line_up` with the modifier in the delivery, and the
window can tell this case from plain line up as the table requires.

<a id="s6-5"></a>
### 6.5 Channel modes (Confirmed: CW24a; BIN: CW24b)

Everything in this section is BIN (CW24b) except what the data shows: that the 31 tokens are
name lists of the `Commands` table, and the ten rows and two localized entries found in
0x7900021C (Confirmed, CW24a, paragraph after the table).

The primary window has a **channel mode**: a chat type. It starts as type 5 `Say` (mask 0x10).
Plain text submitted in the entry is sent to the mode's channel (§7.2 step 5). The
`OutputTextType` label shows the channel's display name (`Chat`-table template token 0x04F7C6D9
with argument 0x00059E45 = channel name and 0x00052615 = the tell target); the channel names are
game data.

**The mode table** (built from client code; each token is an entry of the `Commands` table,
STRINGTABLE key 0x10000007; each entry may hold several names, §8.1). It has 26 fixed rows plus
one of two variant groups (3 or 2 rows), 31 tokens in all; types are given by number (the
enum labels where they are unambiguous):

| `Commands` token | Chat type |
|---|---|
| 0x04DA4989 | 5 `Say` |
| 0x0DA4AC6C | 6 `Tell` |
| 0x048CE78C | 6 `Tell` (reply word: target from callback 0x01D60032) |
| 0x0A48C3E9 | 6 `Tell` (tell-again word: target from callback 0x01D60030) |
| 0x038453D4 | 11 `Fellowship` |
| 0x07B53A04 | 23 `Raid` |
| 0x06F047CE | 15 |
| 0x02533A8E | 46 |
| 0x02533B7E | 47 |
| 0x0AE9AEE7 | 16 |
| 0x0AE9AE73 | 18 `OOC` |
| 0x0AE98903 | 27 |
| 0x040E21EC | 19 |
| 0x03ACBD91, 0x03ACBD92 | 9, 10 |
| 0x08B3A4E1–0x08B3A4E4 | 28–31 |
| 0x08B3A4E5–0x08B3A4E8 | 39–42 |
| 0x098423C4 | 38 |
| 0x06E882B4 | 44 |
| 0x053882B4 | 45 |
| one of two variants, chosen by a game-state test: either 0x02A035D0 → 32, 0x0356ACA0 → 13, 0x053DBC60 → 11; or 0x0CF2A034 → 12, 0x038356A4 → 13 | |

The data holds a partial copy of this table: the property set 0x7900021C (of game object
0x7000021C) has `Communication_ChatChannel_List`, ten `Communication_ChannelInfo_Struct`
entries each with `Communication_CommandString` (a `Commands` StringInfo),
`Communication_CommandHelp1` and `…Help2` (StringInfos in the `CommandHelp` table, STRINGTABLE
key 0x1000004C) and `Communication_ChatTextType` (a type mask). Its ten command tokens and types
are exactly the rows for types 15, 16, 18, 19, 27, 38, 44, 45, 46 and 47 above (OBS). It also has
`Communication_LocalizedChannel_List` with two entries for types 9 and 10, whose command words
are in another table (0x2500481E here) and which also carry `Communication_LocalizedChatTextType`
type 19; the mode table names `Commands` tokens 0x03ACBD91 and 0x03ACBD92 for these types.

For every name of every row, the client registers two **mode words**: the command prefix
followed by the name, and the substitution marker (`CommandSetup` token 0x02409858, one
semicolon here) followed by the name.

**Switching** (on every space character typed into the entry):

1. Take the entry text, trimmed of whitespace (§7.2). If it contains a space, stop.
2. Look the whole text up in the mode words, **case-sensitively**. If it is not one, stop.
3. If the row's type is `Tell` and the word is a reply or tell-again word, ask the callback for
   the target; otherwise the target is unchanged.
4. Set the window's mode (and remember the previous mode), store the mode on the shown tab, and
   update the `OutputTextType` label (Probable: one condition on the target that guards the
   label update was not resolved).
5. Clear the entry text.

After a submit or Escape the window takes the mode stored on the shown tab again, so each tab
remembers its own channel (Probable: the restore reads a value the tab holds; the write in step
4 was traced, the tab-switch read was not).

> **OPEN (Q12):** the game-state test that selects the variant rows (Probable: whether the
> character is a monster-play character); the map chat type → command word used by the submit
> (§6.2 step 2; filled by game code, Probable: from the same `Commands` entries); how a plain
> `Tell` word gets its target (the next word cannot be part of the switch because step 1 stops
> at a space); and what the channel button's menu does (Q3).

<a id="s6-6"></a>
### 6.6 Key-repeat guard (Confirmed: CW25a; BIN: CW25b)

The option `GOP_ChatKeyRepeatProtection` is a boolean with default false (Confirmed, CW25a). The
guard itself, its limits and its sound are BIN (CW25b). When the option is true, every printable
character typed into the entry (code ≥ U+0020 and not U+007F) is checked:

1. If it differs from the previous character: remember it, set the repeat count to 0 and the
   start time to "unset"; accept the character.
2. If it is the same: increase the repeat count. If the start time is unset, set it to now and
   accept.
3. Otherwise, if less than **1.0 second** has passed since the start time and the repeat count
   is **greater than 10**: release the entry's focus, delete the repeated characters before the
   caret (as many as the repeat count), play UI sound 0x50 (OPEN Q13 which sound record),
   forget the previous character, set the start time to now, and swallow this character.
4. Otherwise accept.

---

<a id="s7"></a>
## 7. The command line

<a id="s7-1"></a>
### 7.1 The registry (BIN, CW26)

One registry maps command names to handlers. Built-in commands (game scripts, client code,
§8.1) and plugin commands (`Shell.AddCommand`, LPA §11.1) share it.

- Names are stored without the command prefix.
- Lookup is **case-insensitive** (§8.2). `AddCommand` registers a name only if the registry does
  not already hold it (LPA §11.1).
- Registration of built-in names from a string-table entry splits the entry at every `;`, `:`
  and `,`, trims each piece of whitespace (§7.2) and ignores empty pieces, the same rule as
  `Shell.AddCommand` (LPA §11.1).
- Each handler says whether lines that run it may use `;` substitutions (§7.5).

<a id="s7-2"></a>
### 7.2 Running a line (Confirmed: CW27a; BIN: CW27b, CW28, CW29)

**Whitespace** throughout this section is the set U+0020, U+000A, U+000D, U+0009, U+000C.
**Trim** removes characters of a set from the start and/or the end of a text until a character
outside the set is met.

Inputs: the text, the channel mode's command word *C*, the tell target *G* (possibly empty).

1. If there is no registry or no game context, the result is 2 and nothing is added to the
   history. If the text is empty, the result is 5. Otherwise add the text to the history
   (§6.3), which also resets the browsing index.
2. **Substitution test** (§7.5). If it says yes, expand the `;` tokens of the text (§7.5).
3. Trim whitespace at both ends. If the text is now empty, the result is 5.
4. **Prefix test.** The command prefix *P* is the text of `CommandSetup` (STRINGTABLE key 2,
   table 0x25000200 here) token 0x0DA28738. If the text **starts with** *P*: remove from the
   start every character that occurs in *P* (with a one-character prefix: all leading prefix
   characters, so a doubled prefix is accepted), and if the text is then empty the result is 5;
   otherwise **execute** the text (§7.4) and return its result.
5. Otherwise the text is plain chat for the channel: build the line *C*, a space, *G*, a space,
   the text (the `CommandSetup` template token 0x03E71864, whose argument IDs 0x00004814,
   0x0054665D and 0x000059D4 receive *C*, *G* and the text; with an empty *G* the line has two
   spaces after *C*), then **execute** that line and return its result.

When *C* is empty, the default word `CommandSetup` token 0x04DA4989 (the `Say` channel's
command word) is used.

<a id="s7-3"></a>
### 7.3 Tokenizer (BIN, CW30)

The executed text is split into tokens. The rule, with positions counted in UTF-16 code units
from 0:

1. The **current token** starts empty; the **token start** is 0; the position *i* is 0.
2. While the character at *i* is not the end of the text:
   - **Quote** (`"`, U+0022) at *i*: the token start becomes *i* + 1. Scan from *i* + 1 for the
     next `"`.
     - Found at *j*: the current token becomes the text from *i* + 1 to *j* (exclusive), kept
       exactly, inner and edge whitespace included (it cannot contain a quote). Set *i* = *j* + 1.
     - Not found: the quote is ignored; the current token is **left unchanged** (it still holds
       the previous token); set *i* = the old *i* + 1.
   - **Any other character** at *i*: scan from *i* forward to the first whitespace character or
     the end; call that position *e*. The current token becomes the text from the token start
     to *e*, trimmed of whitespace at both ends. Set *i* = *e*.
   - **Emit:** if the current token is not empty, append it to the token list.
   - The token start becomes *i*; then *i* = *i* + 1 (never past the end).
3. The list may be empty.

Consequences (all BIN):

- Runs of whitespace produce no empty tokens; leading and trailing whitespace never appear in an
  unquoted token.
- A quote counts only where a token starts: at the start of the text or right after a
  whitespace character, but never right after a closing quote. Elsewhere it is an ordinary
  character (`ab"c` is one token with the quote; `"a""b"` gives `a` and `"b"`).
- `""` gives no token.
- **Quirk 1:** a line that ends with exactly one character right after a closing quote loses
  that character (`"a b"c` gives one token, `a b`); with two or more characters after the quote
  they form the next token (`"a b"cd` gives `a b`, `cd`).
- **Quirk 2:** an unmatched quote re-emits the previous token: `go "hi there` gives `go`,
  `go`, `hi`, `there`; at the very start (`"hi`) nothing is re-emitted and the line gives
  `hi`.

<a id="s7-4"></a>
### 7.4 Command word, arguments, execution and results (BIN, CW31, CW32b)

To **execute** a text:

1. If the text is empty, the result is 2.
2. Trim whitespace at both ends; if empty, the result is 5.
3. Tokenize (§7.3). If there are no tokens, the result is 5.
4. **Command word** *W* = the first token, as typed.
5. **Arguments** *A* = the trimmed text with every leading character that occurs anywhere in
   *W* removed (stopping at the first character that does not occur in *W*), then trimmed of
   whitespace at both ends. For an unquoted *W* this is "everything after the command word,
   without the separating whitespace and without trailing whitespace". When *W* was quoted,
   the opening quote is not in *W*, so nothing is removed and *A* is the whole trimmed text.
6. Look *W* up in the registry (case-insensitively, §8.2). If absent, the result is **3**.
7. Otherwise call the handler with the caller's context, *A* and the token list; the result is
   the handler's.

| Result | Meaning |
|---|---|
| 2 | No registry, or an empty line given to execute |
| 3 | Unknown command |
| 5 | Nothing to run (empty after trimming, or no tokens) |
| other | The handler's result (game handlers use 1 for success) |

<a id="s7-5"></a>
### 7.5 `;` substitutions (BIN, CW33, CW34)

**Test** (step 2 of §7.2): look for the command prefix anywhere in the text (characters
compared case-insensitively).

- Not found: substitute (plain chat text always gets substitutions).
- Found at position *p*: the word from *p* + length of the prefix up to the next space (U+0020)
  or the end is looked up in the registry. If it names a command, substitute when that
  command's handler allows it; if it names no command, do not substitute.

Plugin commands allow substitution (CW34); built-in handlers decide one by one.

**Expansion:** every `;` (U+003B) followed by a run of letters and digits (the run may be empty)
is a **substitution token**; the token text includes the semicolon. For each token:

1. If the same token is already being expanded (an outer expansion), leave it unchanged.
2. Ask the game for its value (game-script callback 0x00F50007 with the token text). If the game
   has no value, leave the token unchanged.
3. Otherwise expand the value itself by this same rule, then replace the token by it.

> **OPEN (Q14):** the set of substitution words and their values (game scripts; Probable: the
> current target's name and similar), and an internal switch, off by default, that would also
> remove the whitespace after a replaced token (no code that turns it on was found). Tessera
> rule: no substitution words in M5, so every token stays as typed.

<a id="s7-6"></a>
### 7.6 Unknown command (Confirmed: CW32a; BIN: CW32b)

The `CommandSetup` tokens 0x04FE7F13 (one piece, no arguments) and 0x00E5CD23 (an
"unrecognised command" template with the word as argument 0x000059D4) exist in the data
(Confirmed, CW32a). The rest is BIN (CW32b): when the submit of §6.2 gets result 3, the client
posts the text of token 0x04FE7F13 as a chat line of mask 0x1 (type 1 `Error`), through the
game-script hand-over of §6.2 ("Lines the chat window posts") that shows other system messages.
Token 0x00E5CD23 is not used by the client code (its value appears nowhere in the executable).

<a id="s7-7"></a>
### 7.7 Help listing (BIN)

The help command (registered by game scripts, §8.1) without arguments lists the registered
commands: a heading from `CommandSetup` token 0x0455D18C (arguments 0x05569AE8 = the command
prefix and 0x000059D4 = the list), then one list item per command with its name and its short
help (for plugins, `GetShortHelp`, LPA §11.2), using the underline and list markup of ST §4.7.
With an argument it shows that command's long help (`GetHelp` for plugins). Tessera uses its
own wording and layout for help (§10).

---

<a id="s8"></a>
## 8. Built-in command names and `Shell.IsCommand`

<a id="s8-1"></a>
### 8.1 Where built-in names come from (Confirmed: CW35a; BIN: CW35b; Probable: CW36)

- Every built-in command name is game text from the `Commands` string table (STRINGTABLE key
  0x10000007; 0x250001AB in this install, 261 entries). Entries of the table are of two kinds:
  **name lists** (here 238 entries: one or more names made of letters, digits and underscores,
  separated by `;`, `:` or `,` with optional whitespace; 25 of them hold several names) and
  message templates (the other 23) (Confirmed, CW35a). The client registers each name of a name
  list separately, by the splitting rule of §7.1 (BIN, CW35b).
- Most built-in commands are registered at run time by **game scripts** (the script table,
  `script-table.md`) through script natives that reach the registry's registration routines.
  The script record 0x56000000 references 217 distinct tokens under the `Commands` key (216
  exist in the table, 214 of them name lists); the client code references the 31 tokens of the
  mode table (§6.5), all name lists, 29 of which the scripts also reference. 22 name lists are
  referenced by neither. Which of the referenced tokens are registered as commands (and not
  only used as words inside other commands) was not traced.
- The plugin-manager command (LPA §6.4) and the help command are among the script-registered
  ones: their names are the `Commands` entries with tokens 0x073F0613 and 0x0DA3EC70.

> **OPEN (Q15):** the exact set of names the original registers (it depends on game scripts and
> may depend on the character's state). Suggested method: M4 on the script natives that
> register commands, or M3 (the help command's output in the original client, counted, not copied).

<a id="s8-2"></a>
### 8.2 How `IsCommand` answers (BIN, CW26, CW37)

`Turbine.Shell.IsCommand(name)` converts the Lua string to the client's text (UTF-16) and
returns whether the registry holds that name:

- the name is used **exactly as given**: no trimming, no prefix removal (a name with a leading
  command prefix is not found), no splitting;
- comparison is case-insensitive;
- built-in and plugin commands answer alike, and so do all aliases of a multi-name entry;
- an empty name is never registered, so it answers false.

Case folding uses the C runtime's locale, which the client sets at start-up from its language:
the Windows locales for English (the default), British English, French and German. With those
locales lower-casing covers non-ASCII letters too (Probable: C-runtime behaviour, not traced
inside the runtime).

<a id="s8-3"></a>
### 8.3 Why it matters (Confirmed, CW38)

Published plugins call `IsCommand` with the localized names of the help and plugin-manager
commands to find out the client's language (CEN; LPA §11.1). In an English client such a probe
with the French or German name answers false and the English name answers true; in a German
client the German name answers true. A compatible host must therefore answer for those names in
the language of the user's data, which Tessera does by reading them from the install (§10).

---

<a id="s9"></a>
## 9. What plugins see

- **`ShellCommand:Execute(command, arguments)`** (LPA §11.2): `command` is the command word *W*
  of §7.4 as typed (original case; without quotes if it was quoted); `arguments` is *A* of §7.4.
  So the original **does** trim: leading whitespace after the command word and trailing
  whitespace are removed (the five characters of §7.2), inner runs of whitespace and all quotes
  are kept, and `arguments` is `""` when nothing follows. `;` substitutions (§7.5) have already
  been applied. This answers the `lua-plugin-api.md` §11.2 OPEN (§14).
- **`Shell.WriteLine(text)`** adds a line of type 8 `Log` (LPA §11.1); it is appended to every
  tab showing type 8 (by default only tab 1, §3.3), with markup interpreted (§5.1).
- **Script errors** are lines of type 1 `Error` (LPA §6.6), shown by tab 1 by default.
- **`Turbine.Chat.Received`** (LPA §11.3) receives the chat lines independently of tabs. (When
  Tessera delivers it: §10 item 9.)

> **OPEN (Q18):** `ChatType` in `Received` is one chat type (LPA §11.3), but a line's mask can
> have several type bits (§2, §6.2). How the original turns such a mask into one `ChatType`
> value was not traced (the plugin-side conversion was not read). Tessera rule: report the
> chat type of the highest set bit other than bit 24, the same bit that chooses the style
> (§5.1 step 2 without the style-entry condition); so the reply line of §6.2 is reported as
> type 6.

---

<a id="s10"></a>
## 10. Tessera decisions for M5

These are Tessera's rules, not facts about the original.

1. **Windows and tabs.** At start, Tessera reads `GOP_ChatWindows` from
   `GameplayOptionDefaults` (by UNIQUEDB key 0x68) and builds those windows and tabs (one window,
   two tabs here) with labels from the string tables named by the StringInfos. Changes the player
   makes (filters, tab names, positions, sizes) are kept in Tessera's own settings, never in the
   install or the original's settings folder. The instant-message tab is **not** created in M5
   (there are no tells in the sandbox); `GOP_FullIRCMode` is ignored until tells exist. Tab
   context menus offer only: rename, filter toggles by chat type (type 25 `Unfiltered` is not
   offered, because bit 24 would admit every unfiltered line, §4.3), new tab, close tab.
2. **Lines in the sandbox.** Tessera's own system messages are type 8 `Log` lines. Plain text
   submitted for a channel is echoed locally as a line of the channel's chat type, formatted
   with Tessera's own wording (the original's channel templates are game text and server
   behaviour); it is also delivered to `Turbine.Chat.Received`, at the time item 9 states.
3. **Registry.** Tessera keeps one registry with the rules of §7.1. Its own built-in commands
   (help, the plugin-manager command of LPA §6.4, and its other own commands) are registered
   under Tessera's names **and**, where the original has the same command, under every name of the original's
   `Commands` entry read from the user's install: the plugin-manager entry (token 0x073F0613),
   the help entry (token 0x0DA3EC70) and the channel entries of the mode table of §6.5 (which
   Tessera implements as the local echo of item 2). No other original command name is
   registered; `IsCommand` answers false for them (difference recorded in §13 Q15).
4. **Parser.** Tessera implements §7.2–§7.4 exactly, including the two tokenizer quirks, so
   that plugins and Tessera's commands see the original's command word and arguments. The
   command prefix, the substitution marker and the default channel word are read from the
   `CommandSetup` table at run time. No substitution words exist (§7.5 OPEN Q14).
5. **History, keys, cap.** As §6.3, §6.2, §5.2 and §5.5, read from the data where the data gives
   the value (the cap from `UICore_ListBox_max_rows`). Shift and Ctrl with line up and down do
   nothing (§6.4). `START_COMMAND` opens the entry with the prefix typed (§6.2 OPEN Q10).
   The Tessera actions of the entry follow decision 0014 (IB D2, `keymap.md` §8 as revised for
   it): `open_chat` = `AcceptInput` (Enter, numeric Enter), `menu` = `EscapeKey` (Escape),
   `text_line_up` / `text_line_down` = `CursorPreviousLine` / `CursorNextLine` (line up and
   down, which §6.3 reads as history and §6.4 with Shift or Ctrl), all in the `text_entry`
   context. These replace the earlier mapping `text_submit` = `AcceptInput`, `text_cancel` =
   `EscapeKey`, `text_history_prev` / `text_history_next` = line up / line down; those actions
   are retired, and saved overrides of their names are migrated by the key map.
6. **Unknown command** posts the `CommandSetup` token 0x04FE7F13 text as a type 1 line.
7. **Opacity, timestamps, key-repeat guard, feedback sounds** follow §3.5, §5.4, §6.6 and §5.6
   with the defaults from the install; chat logging to files is not offered in M5.
8. **Lines with several type bits.** Tessera keeps the line's whole mask from posting to the
   logs and applies §4.3 (any bit, once per tab), §5.1 step 2 (highest set bit with a style
   entry) and §5.6 (exact whole-mask match) to it. Tessera has no game scripts: it appends the
   lines of §6.2 and §7.6 itself, with the masks the original hands over (0x21 and 0x1). In the
   M5 sandbox there is never a reply target, so `ChatModeReply` always posts the `Chat`-table
   token 0x00CC89CC text with mask 0x21 (§11.8). For plugins, see Q18.
9. **When plugins receive chat lines** (IB D8, "Game-raised plugin events"; Known difference
   21). The window's own log takes a line at once. The `Turbine.Chat.Received` event for that
   line is not raised synchronously: the line intake appends it, with its argument values
   taken at that moment, to the plugin host's deferred-work queue, and the host delivers it in
   frame-loop step 7 of IB D8, after the UI tick, as one item of the deferred work, in queue
   order, to the handlers set at delivery, **except** the apartments that the line's skip
   rule excludes; it may come in a later frame when that frame's deferred-work slice is used
   up. This holds for lines from the session, for the local echo of item 2, for `WriteLine`
   lines and for Tessera's own system notices, which the client posts at the start of its
   redraw. The skip rule is stated once, in `lua-plugin-api.md` §11.3, together with its
   status (it is the fix for open review item 1 of decision 0014, which keeps the rule of
   `lua-plugin-api.md` §9.6): a line produced while plugin code handles a `Received` event
   is not delivered to the apartments that rule names, so a `WriteLine` in a `Received`
   handler never reaches its own apartment's handlers and an echoing plugin cannot feed
   itself. This spec adds nothing to that rule; where the two read differently, LPA §11.3
   governs. When the original delivers this event is not stated in the cited specs (LPA
   §11.3 OPEN).
10. **Paging the log without entry focus** (IB D5, Known difference 17; §6.2 OPEN Q19). While
   the chat window is the active panel, it pushes Tessera's context `ui.chat_log` for itself
   at priority 0; that context binds Page Up, Page Down, Ctrl+Home and Ctrl+End to the
   paging commands of the last row of §6.2 (`text_page_up` / `text_page_down` and
   `text_doc_start` / `text_doc_end`), so they page the shown log instead of reaching game
   bindings of the same keys (without the push, Page Up and Page Down would reach the camera
   zoom). The default keys are those of `keymap.md` §8 as revised for decision 0014.

---

<a id="s11"></a>
## 11. Worked examples

Command prefix `/`, substitution marker `;`, registry holding (among others) a plugin command
`foo` and the `Say` command word; `␠` marks a space.

### 11.1 Arguments of a plugin command

Line typed: `/FOO␠␠a␠"b␠c"␠␠`.

1. History gets the line exactly as typed.
2. Substitution test: the prefix is found at 0, the word up to the next space is `FOO`, the
   registry holds `foo` (case-insensitive) and plugin commands allow substitution: expand; there
   is no `;`, nothing changes.
3. Trim: `/FOO␠␠a␠"b␠c"`. It starts with `/`: remove leading `/` characters → `FOO␠␠a␠"b␠c"`.
4. Tokens: `FOO`, `a`, `b␠c`.
5. *W* = `FOO`; remove leading characters from {F, O} → `␠␠a␠"b␠c"`; trim → `a␠"b␠c"`.
6. The plugin's `Execute` receives `command` = `FOO`, `arguments` = `a␠"b␠c"`.

### 11.2 Plain text

Mode `Say`, *C* = the `Say` command word (written `S` here; the real word is game text), no
target. Text typed: `␠hello␠`.

1. History gets `␠hello␠`.
2. No prefix in the text: substitution runs (no `;`).
3. Trim → `hello`; no prefix.
4. Line built: `S␠␠hello`; executed: tokens `S`, `hello`; *W* = `S`; *A*: remove leading
   {S} → `␠␠hello` → trim → `hello`. The `Say` handler runs with `hello`.

### 11.3 Tokenizer quirks

| Executed text | Tokens |
|---|---|
| `a␠␠␠b` | `a`, `b` |
| `x␠"a␠b"c` | `x`, `a␠b` (the final `c` is lost) |
| `x␠"a␠b"cd` | `x`, `a␠b`, `cd` |
| `go␠"hi␠there` | `go`, `go`, `hi`, `there` |
| `"␠a␠"␠b` | `␠a␠`, `b` |
| `""` | none (result 5) |

### 11.4 History

Capacity 20. Submit `a`, `b`, `a`: the list is `a`, `b` (newest first; the older `a` was
removed). Press line up: `a`; line up: `b`; line up: `b` (clamped at the oldest); line down:
`a`; line down: nothing changes, the entry still shows `a`. After 25 distinct submissions the
list holds the newest 19.

Empty submit while browsing (§6.2 step 1, §6.3): the list is `c`, `b`, `a` (newest first) and
*k* = −1. Line up twice: *k* = 1, the entry shows `b`. Delete all the text and press Enter: the
entry is empty, so the submit stops at step 1; focus is released, nothing is added, *k* stays 1.
Press Enter (the entry gets focus) and line up: *k* = 2, the entry shows `a`. Had the text `b`
been submitted instead, `b` would have moved to the front (list `b`, `c`, `a`) and *k* would be
−1, so line up would show `b`.

### 11.5 `IsCommand`

Registry holds `foo` and an entry registered from a `Commands` entry with the names `abc` and
`ab`. `IsCommand("FOO")` → true; `IsCommand("/foo")` → false; `IsCommand("ab")` → true;
`IsCommand("␠foo")` → false; `IsCommand("")` → false.

### 11.6 Line cap and bottom

A log holding 128 lines at the bottom receives a line: the oldest is removed, the new line is
added and shown. If the player had scrolled up so that the last line was off screen, the view
stays put.

### 11.7 Opacity

`GOP_ChatOpacity` 25 → entry with range 24–26 → image 0x41005F67 on every window's
`LogFieldBackground` and the primary window's `ChatEntryBackGround`.

### 11.8 Lines with several bits and with `Unfiltered`

Default tabs (§3.3): tab 1 mask 0x00007FF9F8FEFFFD (bits 0 and 5 set, bit 24 clear), tab 2 mask
0x0000000600880004 (bits 2, 19, 23, 33, 34). Feedback entries as §5.6.

| Line mask *L* | Tab 1 | Tab 2 | Style (§5.1 step 2) | Sound (§5.6) |
|---|---|---|---|---|
| 0x21 (`ChatModeReply`, no target) | appended once (bits 0 and 5 both shown) | not appended (no common bit) | type 6 (highest set bit with an entry) | none (no entry equals 0x21) |
| 0x20 (`Tell`) | appended | not appended | type 6 | 0x2A001004 |
| 0x01000020 (`Tell` + `Unfiltered`) | appended (bit 5 shown) | not appended | type 6 (bit 24 removed first) | 0x2A001004 (*T* = 0x20) |
| 0x01 (unknown command, §7.6) | appended | not appended | type 1 | none |
| 0x04 (type 3) | appended | appended | type 3 | none |

A tab whose mask were 0x01000000 (bit 24 only) would take the third line and no other line of
this table.

---

<a id="s12"></a>
## 12. Invariants

Data invariants (check against the install):

| # | Invariant |
|---|---|
| CW-I1 | Layout `chatwindow` has exactly nine top-level elements with `UI_ChatWindow_WindowID`, with distinct values 1–9; exactly one of them resolves `UI_ChatWindow_PrimaryWindow` true, and its ID is 1. |
| CW-I2 | Only the resolved primary window has a child named by `UI_ChatWindow_TextEntryElement`. |
| CW-I3 | Every `GOP_ChatWindow_TabChatTextTypes` in `GameplayOptionDefaults` sets only bits *i* for which *i* + 1 is a key of enum 0x230002A8. |
| CW-I4 | The `UI_ChatOpacities` ranges are disjoint, each has min ≤ max, and together they cover 0–100 with no gap. |
| CW-I5 | For each of the nine 0x70 objects of §4.7, the chat type of its `GOP_ChatTextTypeFontColorStruct` equals `Version_AddNewChatTextType_TypeToAdd` in its 0x79 property set. |
| CW-I6 | Every token of the mode table (§6.5) and the tokens 0x073F0613 and 0x0DA3EC70 exist in the `Commands` table, and each splits (§7.1) into at least one non-empty name. |
| CW-I7 | `CommandSetup` tokens 0x0DA28738, 0x02409858, 0x04DA4989 and 0x04FE7F13 exist and have no arguments; 0x03E71864 has the argument IDs 0x00004814, 0x0054665D, 0x000059D4 in that order. |
| CW-I8 | `ChatLogListbox` has an integer `UICore_ListBox_max_rows` greater than 0. |

Behaviour invariants (synthetic tests):

| # | Invariant |
|---|---|
| CW-I9 | The history never holds more than capacity − 1 lines and never holds two identical lines; its first entry is the most recent non-empty text that reached §7.2 step 1 while a registry and a game context existed (empty submits and result-2 runs add nothing). |
| CW-I10 | A log never holds more lines than its cap. |
| CW-I11 | `arguments` passed to `Execute` never starts or ends with any of the five whitespace characters. |
| CW-I12 | `IsCommand(n)` = `IsCommand(m)` whenever *n* and *m* differ only in letter case; `IsCommand(P + n)` is false for a non-empty prefix *P* when no registered name starts with *P*. |
| CW-I13 | Tokenizing never yields an empty token; the token list of any text without quotes equals splitting the text at whitespace runs. |
| CW-I14 | After a submit the entry text is empty. If the submitted text was non-empty and §7.2 step 1 added it to the history (a registry and a game context exist), the browsing index is −1 afterwards. If the text was empty (§6.2 step 1) or the run returned 2 at §7.2 step 1, the history and the browsing index are unchanged (§11.4). |
| CW-I15 | For any line mask *L* and tab mask *M* (with the step-1 condition of §4.3 met), the line is appended to that tab's log exactly once when *M* AND *L* ≠ 0, and not at all when *M* AND *L* = 0; the number of bits *L* and *M* have in common does not matter. |
| CW-I16 | A feedback sound is played for an appended line only when some entry's mask equals the line's mask with bit 24 cleared; a line whose mask has two or more type bits plays no sound unless an entry has exactly that mask. The style of a line is the style of the highest set bit of that mask that has a style entry (FO §9.3). |

---

<a id="s13"></a>
## 13. Open questions

| # | Question | Section | Suggested method |
|---|---|---|---|
| Q1 | Where a player's `GOP_ChatWindows` is kept; whether the XML options export includes it | §3.4 | M3 (export an options profile in the original); server side out of scope |
| Q2 | Meaning of the mode value 4 of the opacity image; layer or replacement | §3.5 | M4 on the image setter; UL §9.6, §9.6.1 (draw modes) |
| Q3 | Tab and channel-button context menus, tab dragging, re-parenting of the shown log | §4.2, §6.5 | M4 on the menu builders; M3 |
| Q4 | Narrowed: order in which logs are offered a line; owner and setter of the suppression flag; when the refill runs and what the history list keeps; instant-message log as a receiver (the any-bit test and the role of bit 24 are now BIN, §4.3) | §4.3, §4.5 | M4 on the broadcast loop and the refill's callers |
| Q5 | Instant-message session model | §4.6 | M4; not needed for M5 |
| Q6 | Which code applies the nine chat-type migrations | §4.7 | M4 (search by object DIDs), FO Q10 |
| Q7 | Player-speech type set of the profanity filter; second text pass | §5.1 | M4 on the filter routine |
| Q8 | Local-time format of timestamps; condition of the second template | §5.4 | M3 (screenshot with timestamps on); M4 |
| Q9 | Chat log file name, folder, encoding | §5.7 | M3 |
| Q10 | Whether the `START_COMMAND` key's character reaches the entry | §6.2 | M3 |
| Q11 | Meaning of the four tell-target callbacks | §6.4 | M4 on the script table, M3 |
| Q12 | Variant test of the mode table; the mode → command-word map; target of a plain `Tell` word | §6.5, §6.2 | M4 |
| Q13 | Which sound record UI sound 0x50 is | §6.6 | M4 on the UI sound table |
| Q14 | Substitution words and values; the whitespace switch | §7.5 | M4 on script callback 0x00F50007 |
| Q15 | Exact set of built-in command names the original registers | §8.1, §10 | M4 on the script natives; M3 (help output counted) |
| Q16 | `ChatDisplayText` (0x10000308): which window part uses it (relates to FO §9.3 Q14) | §3.1 | M4 |
| Q17 | Whether the game script that receives the reply line (0x21) and the unknown-command line (0x1) passes the mask and text back unchanged | §6.2, §7.6 | M4 on the script entry point; M3 |
| Q18 | `ChatType` reported to `Turbine.Chat.Received` for a line whose mask has several type bits | §9 | M4 on the plugin chat event's conversion; M3 with a logging plugin |
| Q19 | Which input map supplies the log-paging commands while the entry does not have focus (Tessera rule: §10 item 10, IB Known difference 17) | §6.2 | M4 on the window's activation; M3 (Page Up with the window active and the entry closed) |

---

<a id="s14"></a>
## 14. Notes for other specs

These are answers and corrections for specs this document does not own; their owners should
apply them.

| Spec | Item | Note |
|---|---|---|
| `lua-plugin-api.md` §11.2 | OPEN on the command-line parser; Tessera rule "remove exactly one separating whitespace character, keep trailing whitespace" | **Answered, and the Tessera rule differs from the original.** The original trims the line, removes the command word's leading characters, then trims all of the five whitespace characters at both ends; inner whitespace and quotes are kept; `command` keeps the user's case (§7.4, §9). |
| `lua-plugin-api.md` §11.1 | `IsCommand` | Agrees; adds: the name is used exactly as given, no prefix removal, locale-dependent case folding (§8.2). |
| `ui-layout.md` §17.5 item (2) | Which chat windows game code creates | **Answered:** the windows of `GOP_ChatWindows` (default: window 1 only), element by `UI_ChatWindow_WindowID`, no parent (§3). |
| `ui-layout.md` §17.4 P2 | "one tab" | The original's default is two tabs plus the instant-message tab; Tessera builds the two default tabs (§10 item 1). |
| `fonts.md` §9.3 Q10 | Nine game objects | Their 0x79 property sets hold chat-type migration steps (§4.7); the applying code is still unknown (Q6). |
| `keymap.md` §8 | `text_entry` actions | Superseded by decision 0014 (revision 2 here): `text_submit`, `text_cancel` and `text_history_prev/next` are replaced by `open_chat`, `menu` and `text_line_up/down` in `text_entry`, and the Tessera context `ui.chat_log` is added (IB D2, D5; §10 items 5 and 10). The key-map revision itself belongs to that spec's owner. |
| `lua-plugin-api.md` §11.3 | When `Received` fires | Tessera queues the event and delivers it in frame-loop step 7 (IB D8, Known difference 21; §10 item 9 here), skipping the apartments named by the skip rule that LPA §11.3 states. IB's owners list asks that spec's owner whether anything is known about the original's timing. |
| `lua-plugin-api.md` §11.3 | `ChatType` of `Received` | A line's mask can have several type bits (the reply line, 0x21, §6.2); the conversion to one `ChatType` is OPEN here (Q18) with a Tessera rule (highest set bit other than bit 24). LPA may want the same OPEN. |
| `fonts.md` §9.3 | Step 1 (chat-type map lookup) | Agrees and is used for multi-bit masks: the original does post a line of mask 0x21 (§6.2), which step 1 styles as type 6 (§11.8). |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | Revision 2, corrections after review (status stays Revised, awaiting review; no fact about the original changed; section numbers kept). **§10 item 9:** queued `Received` delivery now skips the apartments named by the skip rule of `lua-plugin-api.md` §11.3 (so a `WriteLine` in a `Received` handler never reaches its own apartment and an echoing plugin cannot feed itself); LPA §11.3 is the single statement of that rule and governs; the earlier "this spec does not settle it" is withdrawn; `WriteLine` lines named among the queued lines. **§6.2:** a declined command going on to game code cites IB D7 instead of IB Options 4. **§14:** the LPA §11.3 timing row mentions the skip rule. |
| 2026-10-06 | Revision 2 (status Accepted → Revised, awaiting review; applies the accepted decision records [0014](../decisions/0014-ui-input-bridge.md) (**IB**) and [0015](../decisions/0015-ui-rendering.md) (**UR**); no new research; no fact about the original changed; section numbers kept). **Reworded Tessera rule:** §10 item 5, the mapping of the entry's Tessera actions: `open_chat` = `AcceptInput`, `menu` = `EscapeKey`, `text_line_up/down` = `CursorPreviousLine/NextLine` replace the retired `text_submit`, `text_cancel` and `text_history_prev/next` (IB D2). **Cross-references added:** §6.2 (the commands come from `text_entry` with focus and from Tessera's `ui.chat_log` without it; a declined command goes on to game code unchanged), §6.3 (history reads the line actions), §6.4 (Subset modifier policy lets Shift or Ctrl reach the window), §9 and §10 item 2 (plugin delivery time). **New Tessera items:** §10 item 9 (`Turbine.Chat.Received` is queued and delivered in frame-loop step 7, IB D8, Known difference 21; the interaction with LPA §9.6 is left to IB open review item 1) and §10 item 10 (paging without entry focus through `ui.chat_log`, IB D5, Known difference 17). **New OPEN:** Q19 (which input map supplies the paging commands without entry focus), §6.2 and §13. **§1:** provenance tag IB, UR; header companion entry. **§14:** the `keymap.md` §8 row now records the change; new row for `lua-plugin-api.md` §11.3. UR requires no change here (the chat log's repaint is its content event). |
| 2026-10-06 | Revision 1 after review (status stays Review, submitted for re-review). **B1:** layout 0x22000034 has 19 top-level elements (§1 OBS, CW1, §3.1). **B2:** marks redefined (Confirmed = data only); rows CW2, CW4, CW7, CW8, CW12, CW15, CW16, CW19, CW24, CW25, CW27, CW32, CW35 split into *a* (Confirmed) and *b* (BIN); CW22 reworded (reset on addition, not on every submission); headings of §3.1, §3.3, §3.5, §4.1, §4.3, §4.6, §5.2, §5.4, §5.6, §6.5, §6.6, §7.2, §7.4, §7.6, §8.1 name the split rows. **B3:** new BIN pass (same build): per-log any-bit test with the delivered mask including bit 24, at most once per tab, a suppression flag and a refill routine (§4.3, CW9 now BIN, Q4 narrowed); style of a multi-bit mask by FO §9.3 step 1 (§5.1 step 2); whole-mask exact sound match (§5.6, CW19b); the reply and unknown-command lines leave through one game-script hand-over (§6.2, §7.6, CW41); §2 term; §10 items 1 and 8; §11.8; CW-I15, CW-I16; new OPENs Q17, Q18; notes for LPA §11.3 and FO §9.3 in §14. **B4:** CW-I14 and CW-I9 restricted to submissions that reach the history; §6.2 step 1, §6.3 and §7.2 step 1 say that an empty submit or a result-2 run leaves the browsing index unchanged; §11.4 example. **Non-blocking:** IL and UW in the header and §3.4; Q2 and §13 cite UL §9.6/§9.6.1; §3.2 cites UL §9.2; §5.5 cites UB §4.7 (wheel) and UW §5, §6, §8; `CursorNextPage` and `CursorPreviousPage` go through the list box's clamped scroll-to (UW §5.3; `keep_in_bounds` default true, not set in the layout), so a negative target becomes 0 (CW18). |
| 2026-10-06 | First version (status Review). Chat windows, pool and defaults (§3), tabs and filters (§4), log (§5), entry line, history, channel modes and key-repeat guard (§6), command-line pipeline, tokenizer, arguments, substitutions and results (§7), built-in names and `IsCommand` (§8), plugin view (§9), Tessera decisions (§10), examples (§11), invariants (§12), OPEN list (§13), notes for other specs (§14). OBS on the owner's install and BIN on client 4906.0071.1794.4035, 2026-10-06. Answers M5 critique item A8. |
