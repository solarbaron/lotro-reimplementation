<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Chat window: windows, tabs and filters, the log, the entry line, the command line and built-in command names

| | |
|---|---|
| **Status** | **Accepted** (revision 4 with the fixes of review items B1–B3 and re-review item C1, 2026-10-11; independent re-review of the C1 fix passed, see the review record; implementers re-check the behaviour changes listed in the change log rows of the revision 4 fixes: B1, B3, note (d) and the session row label). Before: Revised (revision 4 with the fix of re-review item C1 and its note (a), 2026-10-11, awaiting an independent re-review; revision 3 stays the Accepted text until it passes; see the change log). Before: Revised, **changes requested** (independent re-review of the revision 4 fixes, 2026-10-11: one blocking item C1, see the review record; revision 3 stays the Accepted text until a corrected revision 4 passes). Before: Revised (revision 4 with the fixes of review B1–B3 and answers to the non-blocking notes (a)–(e), 2026-10-11, awaiting an independent re-review; revision 3 stays the Accepted text until it passes; see the change log). Before: Revised, **changes requested** (independent review of revision 4, 2026-10-11: three blocking items B1–B3, see the review record). Before: Revised (revision 4, 2026-10-11, awaiting an independent review): additive P4 social chat (new §15, rows CW43–CW57, §11.9–§11.14, CW-I17–CW-I30, Q20–Q28, §14 rows) and one correction of revision 3 (the reply and retell rows of the §6.5 mode table, CW43). Sections and anchors of revision 3 are unchanged. Before: Accepted (revision 3, 2026-10-09; accepted after review of the dispatcher answers in §6.5 and §10 items 2, 3, 11–13 and the new row CW42); Revised (revision 3, awaiting review); Accepted (revision 2, 2026-10-06) |
| **Last updated** | 2026-10-11 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent); revision 4 fixes: re-reviewed 2026-10-11 by an independent Claude review session that wrote no part of revision 4 or its fixes (changes requested, C1); C1 fix re-reviewed 2026-10-11 by an independent Claude review session that wrote no part of revision 4, its fixes or the C1 fix (passed, Accepted) |
| **Review record** | Re-review of the C1 fix, 2026-10-11 (independent review session that wrote no part of revision 4, of its fixes or of the C1 fix, CLEAN_ROOM.md §5.3; reviewed `git show 3936f02`: header, the §1 OBS-S "C1 fix" pass, row CW54, Q24, §15.6 "Sending" and "After the send", the CT11 test-server paragraph with the reworded reason of note (a), the change-log row; the P4 text as a whole was reviewed in the two earlier rounds below, `git diff bb43e74..45b7fb9` and `git diff b26038b..44f861d`): **passed**, Accepted. Checklist: functional facts only; no code or pseudo-code; no copied prose; no game text (tokens and argument IDs only); provenance complete (CW54 rests on OBS-S, whose C1 pass is recorded in §1; no BIN row is involved, so no decompiler session was needed); invariants testable; unknowns marked OPEN (Q24 now also asks which line the error routine posts for a channel-send code); no closed gate: all pass. **C1 fixed.** Own throwaway probes, read-only in the owner's install (M1, 2026-10-11, dirty scratch, not committed): the library 0x56000000 decoded under `wlib.md`, the code block searched for the tokens of §15.6 and §15.7 as little-endian u32 values, and the bodies disassembled with `tessera-dattool bytecode --body`; the class table (`wlib.md` §6) parsed to map bodies to virtual slots. The by-slot handler (free-slot line 0x0E2A7CE8 with mask 0x8, then the length check of 1,925 and 385 units, code 38) and the by-name handler (not-joined line 0x02E32555 with mask 0x8, the re-joined text, the length check) both end in one virtual call to the same slot, and in all five classes that hold the handlers that slot holds one body. That body, in this order: the channel test posting *Chat*(0x04068445) with mask 0x1; the character test posting *Weenie*(0x040CF227) with mask 0x8; the send; then the guarded result code and script test (the same code and test as the fellowship send, §15.7) to the generic error routine when the test holds, else on to the echo; any other non-zero result to the generic error routine with that code and no echo; zero posts *Chat*(0x009D15CC) with the channel's name, the text and the slot's mask. It does not reference 0x015B0782; that token occurs in the fellowship send body (as CW52 and §15.7 state) and in the third body, which sits at another slot and is called by neither handler. CW54, §15.6 "After the send", Q24 and the CT11 parenthesis agree with this. **Note (a) done:** CT11's reason now follows `test-server.md` §9.1 (channel key = lowercase of the NFC name; join checks in the order bucket, placed in the world, name rule, `ALREADY_JOINED`) and §15.10 (name-equal = default lower-casing, no normalisation), and the stated consequences (a re-typed join in another normalisation form refused as `ALREADY_JOINED`, a `chsay` or `part` getting CT5's 0x02E32555 or 0x0DF4808C line) follow from CT5 and `tessera-protocol.md` §10.6 (byte comparison with the server's spellings). *Changes implemented behaviour:* the C1 fix and note (a) change none (no code on `p4` posts 0x015B0782; note (a) adds no rule); the behaviour changes of B1, B3, note (d) and the session row label (earlier record, note (b)) still stand for the implementer. Non-blocking, for a later revision (none changes implemented behaviour, since CT5 and CT11 are Tessera rules): (i) the shared body's first test is on a value stored with the channel (the value the send takes as the channel), not on a world or session switch; CW54 and §15.6 call it "user chat unavailable" after the line it posts (the same token as join step 1); describe the condition as "the channel has no subsystem value (zero)" and leave its cause to Q28 or a new Q row. (ii) §15.6 "Sending", by name: the length check measures the handler's raw argument text (channel name included), as §15.2 note (b) says for tell, not the re-joined text; and a name followed by no text ends the handler after the length check with no line and no send (CT5 gives the usage instead, a Tessera rule that may say so). Earlier: Re-review of the revision 4 fixes, 2026-10-11 (independent review session that wrote no part of revision 4 or of its fixes, CLEAN_ROOM.md §5.3; reviewed `git diff b26038b..44f861d`: header, §1 OBS-S and BIN2 fixes passes and new OBS4, rows CW44, CW48, CW50, CW52, CW54 and new CW58, the §6.5 Narrowed callout, Q12, Q25, §11.9 step 4, §11.11 step 9, CW-I26, new CW-I31 and CW-I32, the §14 `scenario-feed.md` row, §15.2, §15.4 steps 1, 2, 4 and 6 and its answered callout, §15.5, §15.6 and §15.7 "Sending", CT3, CT4, CT5, CT6, CT11 and its answered callout, the change log, and owner row O134): **changes requested**. Checklist: functional facts only, no code or pseudo-code, no copied prose, no game text, provenance complete (OBS4 and the fixes passes of OBS-S and BIN2 are recorded in §1), CW-I31 and CW-I32 testable, unknowns marked OPEN, no closed gate: all pass. Earlier blocking items: **B1 fixed** (CW44, the §6.5 callout, §15.2, CT3, CT4, §11.9 step 4, Q12, new CW-I31); **B2 fixed** (CT11 and §11.11 step 9 state user chat on the test server by negotiated minor 2 and `CHAT_CHANNELS`, as `tessera-protocol.md` §10.6 and `scenario-feed.md` §9.8 do; the §14 row and the implementer OPEN follow); **B3 fixed** (§15.4 steps 1, 2 and 4, CW48, CW-I26 marked a Tessera rule, CT6, new CW-I32). Notes (a)–(e) are done; (f) is left to the owner. Spot checks, read-only in the owner's install (M1, 2026-10-11, own throwaway probes in the dirty scratch area): OBS4 and CW58 reproduced from 0x22000034, 0x22000004 and the `UIElementID` mapper 0x230000D1 (the four children of the row template with the stated labels, rectangles and bases; both bases are `Button` elements with seven states; the two markers are `Field` elements of 14 × 14 at (376, 2); 0x10000316 is labelled `ChatLogListbox`). OBS-S re-decoded with `tessera-dattool bytecode`: the tell handler applies the length check to its raw argument text (note b, §15.2 step 4); the fellowship send runs usage, membership, availability, length check, send, and its results after the send are as CW52 states. BIN2 re-checked under decision 0011 on a private copy of the shared project (deleted after use): CW44 (a word of a `Tell` row in neither callback set leaves the switch before the mode, the target or the entry text change; every switch that happens stores the resolved name, which is empty unless a callback returned one); §15.4 steps 1, 2 and 4 (a by-name hit takes the line's spelling in place with no hand-over; with auto-creation off the generic row takes the line and is not recorded under the ID; a rename happens only beyond case, and then the shown session's name, empty while the generic row is shown, goes to the `Tell` mode); CW49 (no line when creation fails); CW50 (the third kind closes only a session found by name, and otherwise shows the second failure line in the session found by instance ID, else in the generic row). **Blocking. C1 (CW54 describes the wrong script body).** Both user-channel send handlers (by slot: the free-slot line *Chat*(0x0E2A7CE8), then the length check; by name: the not-joined line, the re-joined text, the length check) hand the text and the channel to one shared channel-send body. That body checks user-chat availability (*Chat*(0x04068445), mask 0x1) and the character-age test (*Weenie*(0x040CF227), mask 0x8), sends, and then: a result of zero posts the echo *Chat*(0x009D15CC) with the channel's slot mask; the one result code guarded by the script test of CW52 goes to the generic error routine when the test holds and otherwise counts as zero; **every other non-zero result goes to the generic error routine with that code**. This body never posts *Chat*(0x015B0782). The pattern that CW54 and §15.6 "After the send" state (*Chat*(0x015B0782) with mask 0x1 and a second code sent to the error routine) belongs to a different script body, the send on a channel chosen by a chat-type mask, which neither user-channel handler reaches. Correct CW54, §15.6 "After the send" and the CT11 parenthesis "(§15.6, *Chat*(0x015B0782) after a send)", which holds for the fellowship channel (§15.7) but not for user channels. *Does not change implemented behaviour:* no code on branch `p4` posts that token. Non-blocking: (a) CT11 gives as the reason that the server's channel key is "the same lower-casing as name-equal"; `test-server.md` §9.1 keys by the lowercase of the **NFC** name, and name-equal (§15.10) has no normalisation step. The conclusion still holds (the server sends one spelling per channel, and slots hold only server spellings), so reword the reason, or note that a join typed in another normalisation form passes CT5's local duplicate check and comes back from the server as an `ALREADY_JOINED` refusal shown as a `System` line. (b) For the implementer, once a corrected revision 4 is Accepted: B1, B3, note (d) and the session row label change behaviour built on branch `p4`. Today `crates/ui` `game::chat::window` gives a plain `Tell` word the `Tell` mode with an empty target and leaves the target unchanged for words of other types (CW44 and CW-I31 ask for no switch, and an empty target respectively). `game::chat::im` hands a renamed row's name to the `Tell` mode (CT6 and CW-I32 ask for none) and labels the row's first `Text` element (§15.4 answered callout (a) asks for `IMSessionText`). Earlier: Review of revision 4, 2026-10-11 (independent review session that wrote no part of revision 4, CLEAN_ROOM.md §5.3; reviewed `git diff bb43e74..45b7fb9`: header, §1 tags OBS3, OBS-S, BIN2, rows CW43–CW57, the §4.6, §6.4, §6.5, §8.1 callouts, §10 pointers, §11.9–§11.14, CW-I17–CW-I30, Q20–Q28, §14 rows, §15, change log): **changes requested**. Checklist: functional facts only, no code or pseudo-code, no copied prose, no game text (placeholders `T`, `R`, `F` stand for the game's words), provenance complete, invariants testable, no closed gate: all pass. Spot checks, read-only in the owner's install (M1, 2026-10-11, own throwaway probes, text read to classify only): CW-I17 holds for all 35 listed `Chat` tokens and for the further tokens named in §15 (0x045DBD34, 0x046A54E7, 0x0C077CFC, 0x03A70DEC, 0x0360EAE4, 0x0B891524, 0x018E5F35 with the stated arguments); CW-I18 holds (tell 5 names, reply and retell 2 each, slot rows 3 each, none shared between tell, reply and retell); the `CommandHelp` pairs of CW43 describe reply as sending to the last sender and retell as sending to the last player told (classified, not quoted); CW-I19 and CW57 defaults (auto-create true, full IRC mode true, friends-and-guild-only false); CW49 (`max_rows` 128 in both list boxes). OBS-S re-decoded with `tessera-dattool bytecode`: CW55 (friend notices, bit 0, mask 0x8, name from the list, names not in the list post nothing), CW53 (slot masks of the eight slots; the constants 8, 50 and 20), the §15.6 list lines and masks, CW45 (usage below two tokens, 0x01A17E05 with mask 0x21, limits 1,925 and 385, code 38, retell target set after the send), CW52 (lines, masks 0x1 and 0x400); none of the tell-line tokens and neither 0x0B891524 nor 0x018E5F35 occurs in the script library. BIN2 re-checked under decision 0011 on a private copy of the shared project (deleted after use): CW43 (the retell entry's names feed the set whose words ask the recipient callback 0x01D60030, the reply entry's names the set that asks 0x01D60032), CW47, CW48 steps 1–3, 5 and 6, CW49 creation failure, CW50 (failure kinds; the third kind looks the session up by name only). **Blocking.** **B1 (CW44 is wrong for the plain tell word).** In the space-key switch, a word whose row has the `Tell` type but which is in neither the reply nor the retell word set leaves the switch at once: the mode, the target and the entry text stay as they were (the space stays typed), so `/T Cara hello` is submitted as a command. Only reply and retell words switch to `Tell`, with the callback's name or, when it has none, an empty target. Correct CW44, the §6.5 "Narrowed" callout (a), the §15.2 paragraph "The `Tell` mode", CT3 "Mode words", the last CT4 bullet and §11.9 step 4 (an empty `Tell` target is still reachable through the reply and retell words and the generic instant-message row). Tessera may keep its current behaviour only as a stated Tessera rule with a known difference. *Changes implemented behaviour:* `crates/ui` `game::chat::window` mode-switch step 3 switches a plain `Tell` word to `Tell` with an empty target. **B2 (CT11 and §11.11 step 9 contradict the P4 companions).** CT11's test-server row ("user chat is unavailable", `join` posts 0x04068445 and sends nothing) and §11.11 step 9 contradict `tessera-protocol.md` v1.2 §10.6 and `scenario-feed.md` §9.8 (decision 0035), and the implementer OPEN (a) of §15.10 asking for this revision is unanswered. State the rule by negotiated minor and `CHAT_CHANNELS` (as the client already does) and fix §11.11 step 9. *Does not change implemented behaviour.* **B3 (§15.4 step 4 misstates the rename).** In the original, a session found by **name** (case-insensitive) takes the line's spelling of the name in place, without any hand-over to the `Tell` mode; only a session found by **instance ID** whose stored name differs from the line's name beyond case is renamed, and then the **shown** session's name (empty for the generic row) is handed to the `Tell` mode again, whichever row was renamed. Rows are never moved, so CW-I26's strict order is a Tessera rule, not a property of the original. Correct step 4 and answer the §15.4 implementer OPEN (b) (moving a renamed row is then a Tessera rule; say so in CT6). *Changes implemented behaviour:* `game::chat::im` hands the new name to the `Tell` mode when a shown row is renamed by case. **Non-blocking notes.** (a) §15.7 "Sending" puts the length check first; the handler's order is: usage at one token or fewer, membership, fellowship-chat availability, length check, send (CW52's row order is right). After the send, a non-zero result from the chat subsystem posts *Chat*(0x015B0782) with mask 0x1 (one result code goes to the generic error routine instead) and no echo; §15.7 and probably §15.6 should name that refusal. (b) §15.2 step 4: the tell handler applies the length check to its raw argument text, not to the re-joined text. (c) CW50: the "third failure kind" looks the session up by name only. (d) CT5: a name of at most 50 code units but over 64 UTF-8 bytes gets the 50-limit line with a length of 50 or less; give it its own wording or say so. (e) Answer the remaining implementer OPENs: §15.4 (a), §15.10 (b) (UTC time of day versus §5.4's local time) and (c). (f) Decision 0011 scopes M4 to "everything M0–M7 need"; BIN2 serves the parity track; a sentence in 0011 or 0024 would put that beyond doubt. Earlier: Review of revision 3, 2026-10-09 (independent review session, CLEAN_ROOM.md §5.3; reviewed the diff against main: §1 tags OBS2 and D20, row CW42, the two §6.5 callouts, §10 items 2, 3, 11–13, the two answered §10 callouts, change log): **passed**, Accepted. Spot check: CW42 reproduced read-only in the owner's install on 2026-10-09 (the token-0x04DA4989 entries of 0x25000200 and 0x250001AB compared without printing their text: equal, one piece, no `;`, `:` or `,`, so one name under §7.1). Cross-references checked: D20 items 1–7, `tessera-protocol.md` §5.2 (`TEXT` 1–1,024), CW20, CW27a, §6.2 submit steps, §6.3, §7.2–§7.4. Non-blocking notes: (1) §10 item 11, submit order: the condition says it holds for every Tessera handler; say explicitly that plugin handlers are covered too (LPA gives a plugin no access to the entry text or the channel mode). (2) §10 item 12, positions: the equivalence argument for §7.2 step 4 rests on this install's one-character BMP prefix (CW27a); since the prefix is read at run time, state what Tessera does if an install's prefix has several characters or one outside the BMP. (3) The first §6.5 answered callout still names implementation modules (`tessera-ui`, `game::chat`); harmless, could say "Tessera". Earlier: review of revision 2, 2026-10-06: passed, Accepted (full record in the review log, decision 0018). |
| **Implements into** | `apps/client` (native chat panel P2 of `ui-layout.md` §17, entry line, command dispatch), `crates/ui` (`tessera-ui`: chat-log list, tab list, entry line widgets), `crates/plugins` (`Shell.IsCommand`, `Shell.AddCommand`, `ShellCommand:Execute` arguments, `Shell.WriteLine` lines), `crates/settings` (Tessera's own store of the chat-window configuration), `crates/net` sandbox backend (local echo of channel lines). Revision 4 (P4, §15): `apps/client` (the dispatcher's tell, reply, retell, fellowship and user-channel built-ins, the line forms of session chat, user-channel slots, the ignore filter, friend notices), `crates/ui` (the instant-message tab and its sessions) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (layout records, panel P2, resolution of base elements; **UL**), [`ui-behaviour.md`](ui-behaviour.md) (focus, key routing, text-entry keys, list scrolling; **UB**), [`fonts.md`](fonts.md) §9.3 (chat line font and colour; **FO**), [`lua-plugin-api.md`](lua-plugin-api.md) §6.6, §9.3, §11 (shell and chat API; **LPA**), [`keymap.md`](keymap.md) §8, §12 (Tessera actions, original input commands; **KM**), [`string-table.md`](string-table.md) (StringInfo, templates, markup; **ST**), [`property-system.md`](property-system.md) (property sets, bitfields, game-object property sets; **PS**), [`records.md`](records.md) §7–§8 (enum mappers, DID maps; **REC**), [`sound.md`](sound.md) (sound records), [`install-layout.md`](install-layout.md) §8 (per-user settings folder; **IL**), [`ui-widgets.md`](ui-widgets.md) §5, §6, §8 (Scrollable, Scrollbar, ListBox; **UW**). Tessera decision records (Accepted 2026-10-06): [`0014-ui-input-bridge.md`](../decisions/0014-ui-input-bridge.md) (UI commands as key-map actions, the UI band, frame loop; **IB**) and [`0015-ui-rendering.md`](../decisions/0015-ui-rendering.md) (UI paint and composition; **UR**). Revision 4: [`scenario-feed.md`](scenario-feed.md) §6.4, §7.6, §9 (Social steps, generic chat answer, domain events and commands; **SF**), [`tessera-protocol.md`](tessera-protocol.md) §6.4, §10 (chat on the test server; **TP**), [`social-panels.md`](social-panels.md) §3.7, §9 (friends and ignore lists, fellowship model; **SP**), [`wlib-bytecode.md`](wlib-bytecode.md) and [`wlib.md`](wlib.md) (the script library read as data, §1 OBS-S), decision [0024](../decisions/0024-parity-track.md) (parity track, client half only; **D24**) |

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
> compatible client cannot work without them (`docs/CLEAN_ROOM.md` §5.3 and
> `lua-plugin-api.md` §1). **No game text appears in this
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
  open questions (§13);
- (revision 4, P4) social chat as the original client presents it: tells and their lines,
  the instant-message tab and its sessions, user chat channels and their slots, the
  fellowship channel, ignore filtering and friend notices, and how each maps to the session
  layer and the command dispatcher (§15).

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
| OBS2 | The owner's Steam install, read in place and read-only on 2026-10-09 (revision 3) with the repository's `tessera-dattool strings` as a tool and a throwaway comparison in the dirty-room scratch area: the entries with token 0x04DA4989 of the string tables 0x25000200 (`CommandSetup`) and 0x250001AB (`Commands`) compared piece by piece (the text was compared, not reproduced). |
| D20 | The accepted Tessera decision record [0020](../decisions/0020-chat-command-routing.md) (one client-side command dispatcher). Source of no fact about the original; cited by §10 items 2, 3 and 11–13 (revision 3). |
| OBS3 | The owner's Steam install (data files dated 2026-10-04), read in place and read-only on 2026-10-11 (revision 4) with `tessera-dattool strings` and throwaway probes in the dirty-room scratch area: every entry of `Chat` (0x250001A7, 181 entries), `Commands` (0x250001AB, 261), `CommandHelp` (0x250001AA, 179), `ChatChannel` (0x250001A8, 4), `ChatChannelType` (0x250001D7, 31), `SecondaryChatChannelType` (0x250001D8, 3), `Fellowship` (STRINGTABLE key 0x10000017, 0x25000021) and `WeenieClientMsgs` (key 0x10000039, 0x250001C0) as token, pieces and argument IDs (text read to classify, not reproduced); the property dictionary 0x34000000 (names of the properties of §15); `GameplayOptionDefaults` 0x78000006 (the chat options); the enum mapper 0x230002A8 with base 0x230002A9 (chat-type labels); the instant-message elements of layout 0x22000034. |
| OBS-S | Method M1 on the script library: the record 0x56000000 of `client_gamelogic.dat` (the game scripts are data in the user's install). All 181 `Chat` tokens were searched as little-endian u32 values in its code block (94 occur in it, 113 occurrences in 49 function bodies); every body named in §15 was decoded with the repository's `tessera-dattool bytecode --body` under `wlib-bytecode.md`, and the message chunk (`wlib.md` §4.2) was read for the four messages the bodies use. The instruction listings were read in the dirty room; no listing, pseudo-code or message text is reproduced here, only what the bodies do. Read 2026-10-11. **Revision 4 fixes (2026-10-11, same method):** the tell body again (which text the length check measures), the fellowship send body and the shared user-channel send body (order of the checks; what the scripts do with the chat subsystem's result after a send). **C1 fix (2026-10-11, same method):** the by-slot and the by-name user-channel send handlers and the one body both hand the text and the channel to (its checks and its handling of the result), compared with the fellowship send body and with a third body that sends on a channel chosen by a chat-type mask; the revision 4 fixes pass had read the third body as the user-channel one. Neither handler calls the third body. |
| BIN2 | Method M4 under decision 0011, revision 4: the same client build as BIN (4906.0071.1794.4035), the shared auto-analysed Ghidra project (a private copy, headless, read-only), 2026-10-11, function by function. **Questions:** (13) which tell-target callback belongs to which of the two `Tell` rows with a callback (§6.5); (14) what the mode switch does with the target for a `Tell` word without a callback; (15) how the normal tabs and the instant-message tab receive incoming tells, the player's own tells and failed tells: which `Chat` templates, which arguments, which masks; (16) the instant-message session model: creation, lookup, order, renaming, limits, selection, the session menu; (17) whether any receive path of the logs consults the ignore list; (18) where the client uses `GOP_AutoCreateIMSessions`, `GOP_FullIRCMode` and `GOP_AcceptFriendsAndGuildMemberTellsOnly`. **Read:** the channel-mode table construction and the space-key switch again; the four tell-target callers; the `ChatModeReply` branch; the per-log receive slots for tells (incoming and own) and for general lines; the instant-message log's initialisation, session creation, lookup by identifier and by name, removal, session-line and failure slots, its menu and its name prompt answer; the routine that hands a name to the primary window's tell mode; an immediate-value search of the whole executable for the `Chat` tokens and the chat property IDs of §15. **Why M1–M3 were not enough:** the data holds the templates, the options and the layout, and the scripts (OBS-S) hold the channel and fellowship lines, but the tell lines are referenced only by client code (none of their tokens occurs in the script library), and the instant-message tab is client code only; no published source describes either; the original client cannot be observed in this session (M3). **Not read:** the chat subsystem below the receive slots (where the client hands an outgoing line or a join request to it, only that fact is recorded), the network protocol, login, keys, launcher, patcher and anti-tamper (decision 0004). No address, listing or symbol appears in this spec. **Revision 4 fixes (2026-10-11, review items B1 and B3, a second private copy of the shared project, deleted after use; same build):** questions (19) what the space-key switch does with a word of a `Tell` row that is in neither the reply nor the retell word set, and what it does with the stored target for words of other types; (20) when an instant-message session is renamed and what then goes to the `Tell` mode; (21) how the failure slot treats the kind that closes a session when no session has the name. **Read:** the space-key switch, the session line routine, the by-name session lookup, the session creation routine, the failure slot, the routine that hands a name to the primary window's `Tell` mode, and the client's string-info rendering routine that both name comparisons use. **Why M1–M3 were not enough:** as for BIN2. |
| OBS4 | The owner's Steam install (data files dated 2026-10-04, unchanged since): the layout records 0x22000034 (`chatwindow`) and 0x22000004, read in place and read-only on 2026-10-06 (the two records kept in the dirty-room scratch area) and decoded again on 2026-10-11 with a throwaway probe, for the labels, types, rectangles and bases of the children of `IMSessionsListboxItemTemplate` (0x1000032B) and `IMChatLogElement` (0x1000032E) and of the two base elements they name (revision 4 fixes). |
| SF, TP, SP, D24 | The companion specs and the decision record named in the header (revision 4). Tessera's session layer and its panels; the source of no fact about the original. |

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
| CW42 | The `CommandSetup` entry and the `Commands` entry with token 0x04DA4989 hold the same text, a single name, in this install (English); so the default channel word of §7.2 is the only name of the `Say` row of the mode table (§6.5, §10 item 3) | OBS2 | Confirmed (this install, 2026-10-09) |
| CW43 | **Correction of revision 3 (§6.5).** Of the two `Tell` rows with a callback, `Commands` token 0x0A48C3E9 is the **reply** entry (its `CommandHelp` entries 0x048C3E61/0x048C3E62 describe sending to the last player who sent a tell) and takes its target from callback 0x01D60032, the callback `ChatModeReply` uses; token 0x048CE78C is the **retell** entry (help 0x08CE78B1/0x08CE78B2: to the last player told) and takes its target from callback 0x01D60030. Revision 3 printed the two parentheticals against the wrong tokens (§6.5) | OBS3 (`Commands`, `CommandHelp`); BIN2 (the mode table stores the names of 0x048CE78C and 0x0A48C3E9 in two separate word sets; the switch asks callback 0x01D60030 for a word of the first set and 0x01D60032 for one of the second) | Confirmed (data) + BIN |
| CW44 | **Corrected in the revision 4 fixes (review B1).** In the space-key switch, a word of a `Tell` row that is in neither the reply nor the retell word set switches **nothing**: the mode, the stored target and the entry text stay as they were (the typed space stays in the entry). A reply or retell word switches to `Tell` with the callback's name as the target, or with an **empty** target when the callback returns none. A word of any other type switches to its mode and sets the stored target to empty. So every switch that happens overwrites the stored target (§6.5, §15.2) | BIN2 (switch routine, re-read for the fixes: the `Tell` branch asks a callback only for words of the two sets and otherwise leaves the routine before the mode is set or the entry cleared; the target store copies a name that is empty unless a callback returned one) | BIN |
| CW45 | The game script's `tell` handler (`Commands` 0x0DA4AC6C): fewer than two tokens → the command's usage as a type 4 line; recipient = the first token; text = the remaining tokens joined with one U+0020; empty text → `Chat` 0x01A17E05 (argument 0x05866BA4 = recipient) with mask 0x21, then the usage; a text over the length limits of §15.2 → the script's generic error routine with code 38; otherwise the recipient and text go to the client's tell send, and the recipient becomes the retell target (§15.2) | OBS-S (the handler body; message 9 of the library is one space) | Confirmed (script data); the error-38 line OPEN (Q24) |
| CW46 | The reply and retell handlers (`Commands` 0x0A48C3E9, 0x048CE78C): no token → usage (type 4); target = the last sender (reply) or the retell target (retell); no target → `Chat` 0x00CC89CC (reply) or 0x0811EFD4 (retell) with mask 0x21; text = the §7.4 arguments as typed (not re-joined); same length check; neither changes the retell target (§15.2) | OBS-S | Confirmed (script data); where the last sender is kept OPEN (Q26) |
| CW47 | Every normal tab's log builds the line of an **incoming** tell itself from `Chat` 0x0BCA09CC (0x4DD4 = the sender's instance ID inside the `Select` link, 0x52615 = the sender's name, 0x5277 = the text) and appends it with mask 0x20 when the log's mask has bit 5, and also, without bit 5, when an identifier flag of the sender is set and the same flag of the player is clear; the player's **own** tell comes back to every log whose mask has bit 5 as `Chat` 0x0AD74A23 (0x4DD4 = recipient's instance ID, 0x5866BA4 = recipient's name, 0x5277 = text), mask 0x20 (§15.3) | BIN2 (the two tell slots of the log's receive interface); OBS3 (argument IDs) | BIN; the flag's meaning OPEN (Q20) |
| CW48 | The instant-message tab keeps **sessions**: a first "generic" row (label `Chat` 0x0D93B0C5) created at start, then one row per conversation partner. A tell line (incoming or own) is put in the partner's session, found by the partner's instance ID or else by name (case-insensitive); with no session, `GOP_AutoCreateIMSessions` (boolean, default true) creates one (row label = the partner's name, inserted after the generic row in name order), else the generic row takes it; the line is `Chat` 0x0F2CF919 (0x58E15 = current time text, 0x4DD4, 0x52615 = the speaker's name: the partner for incoming lines, the player for own lines, 0x5277 = text), mask 0x20; a session found by name takes the line's spelling of the name in place, and a session whose stored name differs from the line's partner name beyond case is renamed in place, after which the shown session's name is handed to the `Tell` mode; rows are never moved (corrected in the revision 4 fixes, review B3); showing a session hands its name (empty for the generic row) to the primary window's `Tell` mode, which stores it as the target (§15.4) | BIN2; OBS3 (option default, labels, argument IDs); OBS (layout) | BIN; that the hand-over also sets the mode to `Tell` Probable; the generic row's relabelling Probable |
| CW49 | Session limits: the session list `IMSessionsListbox` (0x10000328) has `UICore_ListBox_max_rows` 128; the per-session log list (in `IMChatLogElement`, 0x10000316) has 128; the session menu's "start session" item (`Chat` 0x0780711E) is enabled only while the row count is below the session list's `max_rows`; when auto-creation fails, the line is not shown in the tab (§15.4) | OBS (layout 0x22000034); BIN2 (menu builder, session line routine) | Confirmed (layout) + BIN; the insertion failure at a full list Probable |
| CW50 | A failed tell reaches the instant-message tab only: the partner's session (by instance ID, else name), else the generic row, gets `Chat` 0x04E6F672 (0x58E15 = time) for one failure kind and `Chat` 0x0F2C3BE2 (0x58E15, 0x52615 = name) for the others, mask 0x20; a third kind closes the session found **by name** (case-insensitive) and shows nothing, and, when no session has that name, is shown like the others (*Chat*(0x0F2C3BE2) in the session found by instance ID, else the generic row; corrected in the revision 4 fixes). `Chat` 0x0B891524 and 0x018E5F35 (argument 0x5506DA2 = name) occur neither in client code nor in the script library (§15.5) | BIN2 (the failure slot of the instant-message log; immediate search); OBS-S (token search) | BIN; normal tabs' failure display and the server-side lines OPEN (Q21) |
| CW51 | Incoming channel lines go through one script dispatcher: fellowship → `Chat` 0x05D20D84 (0x4DD4, 0x8495FF2 = sender, 0x5277 = text) with mask 0x400 (variant 0x00D8DF00 under the game-state test of §6.5); a user channel → `Chat` 0x063C924C (0x7C632DC = the channel's name, 0x4DD4, 0x8495FF2, 0x5277) with the mask of that channel's slot; a channel ID matching no joined channel takes a fallback that only runs under a script condition (§15.6, §15.7) | OBS-S | Confirmed (script data); the fallback's condition OPEN (Q22) |
| CW52 | The fellowship send handler, in this order: no text after the command word → the usage; no fellowship → `Fellowship` table (STRINGTABLE key 0x10000017) token 0x06358717 with mask 0x1 (variant 0x0871B170), nothing sent; fellowship chat unavailable → `WeenieClientMsgs` (key 0x10000039) 0x064C4F95, mask 0x1; the length check of §15.2; then the send. A result of zero from the chat subsystem → the local echo `Chat` 0x04A3E744 (0x5277 = text) with mask 0x400 (variant 0x0E74EA10); any other result → `Chat` 0x015B0782 (no arguments) with mask 0x1 and no echo, except one result code, which under a script test goes to the generic error routine and otherwise is treated as zero (§15.7; order and results re-read in the revision 4 fixes) | OBS-S | Confirmed (script data) |
| CW53 | User chat channels: the scripts register the `Commands` entries 0x03A7284C (join), 0x0DCD958C (leave), 0x0A55AF03 (list), 0x098B3A44 (send by name) and 0x08B3A4E1–0x08B3A4E8 (send by slot 1–8); a player has **8** slots (the library returns 8), slot *k* (1–8) has chat type 27 + *k* for *k* ≤ 4 and 34 + *k* for *k* ≥ 5 (masks 0x08000000, 0x10000000, 0x20000000, 0x40000000, 0x4000000000, 0x8000000000, 0x10000000000, 0x20000000000); channel names at most **50**, passwords at most **20** (constants of the library); the join and leave checks, their order and their lines, the joined and left lines, the list lines and the send refusals of §15.6, all with mask 0x8 except the list rows (slot mask) | OBS-S (registration body, join, leave, list, send and notice bodies, the slot-mask function, the three constant functions) | Confirmed (script data) |
| CW54 | Both user-channel send handlers (by slot and by name, after their own checks and the length check) hand the text and the channel to one shared send body. In it, in this order: user chat unavailable → `Chat` 0x04068445 with mask 0x1; a character too new for the channel → `WeenieClientMsgs` 0x040CF227 with mask 0x8; then the send. A result of zero from the subsystem → the local echo `Chat` 0x009D15CC (0xAD99BC5 = the channel's name, 0x5277 = text) with the slot's mask; the one result code that CW52 guards by a script test → the generic error routine with that code when the test holds, otherwise treated as zero (the echo is posted); **any other non-zero result → the generic error routine with that code**, no echo. This body never posts `Chat` 0x015B0782: that refusal belongs to the fellowship send (CW52) and to a different body, a send on a channel chosen by a chat-type mask, which neither user-channel handler reaches (§15.6; results added in the revision 4 fixes, corrected in the C1 fix) | OBS-S | Confirmed (script data) |
| CW55 | Friend notices: when the status of a friend in the friends list changes, the script posts `Chat` 0x0A29AFC5 (0x52615 = name) when the online bit goes from clear to set, `Chat` 0x010538E5 when it goes from set to clear, both with mask 0x8 (type 4), and nothing otherwise (§15.9) | OBS-S (status-update body) | Confirmed (script data) |
| CW56 | No receive path that was read consults the ignore list: the logs' tell and general-line slots, the instant-message session line routine (BIN2) and the script's channel-line dispatcher (OBS-S) append lines without an ignore test (§15.8) | BIN2; OBS-S | BIN (absence in the paths read); server filtering Probable |
| CW57 | `GOP_AcceptFriendsAndGuildMemberTellsOnly` is a boolean option with default false; client code reads it only where options are loaded and shown, and none of the receive paths read in BIN2 uses it; `GOP_AutoCreateIMSessions` is read by the session line routine and the options code; `GOP_FullIRCMode` by the tab loading of §4.6, the tab menu's instant-message item and the options code (§15.4, §15.8) | OBS3 (0x78000006, 0x34000000); BIN2 (immediate search and the reading functions) | Confirmed (data) + BIN; the first option's effect Probable server-side (Q27) |
| CW58 | The session row template `IMSessionsListboxItemTemplate` (0x1000032B) has four children: `IMSessionText` (0x1000032C, 100 × 19 at (373, 0)), based on `IM_Base_Button` (0x10000CAF of layout 0x22000004, a `Button` with seven states and text colours); `IMSession_CloseButton` (0x1000032D, 14 × 14 at (457, 2)), based on `IM_Close_Button` (0x10000CAE of 0x22000004, a `Button`); `UnreadIndicator` (0x10000319) and `LoggingIndicator` (0x100012DD), `Field` elements of 14 × 14 at (376, 2). The list box of the session log `IMChatLogElement` (0x1000032E) is 0x10000316, labelled `ChatLogListbox` (§15.4; revision 4 fixes) | OBS4 | Confirmed (layout) |

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

> **OPEN (implementation, M5 clean room):** no window property names the tab list
> (`TabListBox`) or the log element (`ChatLogElement`, the template a tab's log is created from),
> and §4.2 does not say which element draws a tab's label. `tessera-ui` (`game::chat::window`)
> finds the tab list as the first descendant of type `ChatWindowTabListBox` (else by the element
> label `TabListBox`), creates logs from the top-level element `ChatLogElement`, and creates the
> game types `ChatWindowTabListBox` as a ListBox and `ChatTab`, `IMTab` and
> `PlayerChatTextEntry` as Text elements, the tab drawing its own label.

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

> **Answered in part (revision 4, 2026-10-11):** the session model (creation, lookup, order,
> renaming, the lines a session gets, failures, limits, closing by a failure, the effect of
> showing a session) is §15.4 and §15.5 (CW48–CW50, BIN2). Still open: the row's close
> button and unread marker, and what the "close session" menu item does beyond removing the
> row (Q25).

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

> **Note (P3, 2026-10-10, [`quest-journal.md`](quest-journal.md) §7; no fact of this spec
> changed):** the quest journal of the class library posts its own lines (new quest, new deed,
> completed, failed, and objective billboard texts) with the 64-bit mask 0x100000, chat type 21
> `Quest`, which the first default tab shows (§3.3). Their tokens are in table `Quest`
> (STRINGTABLE key 0x10000041); other quest lines of that table are server text.

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

> **OPEN (implementation, M5 clean room):** FO §9.3 step 3 sets the colour and "both font
> slots"; the line template also carries the six chat fonts as `UICore_Text_fonts`, which wins
> over a single font under Tessera's slot order (`fonts.md` §9.1). `tessera-ui` sets
> `UICore_Text_font_color`, `UICore_Text_font` and `UICore_Text_tag_font`, and also replaces
> `UICore_Text_fonts` and `UICore_Text_tag_fonts` by one-entry arrays holding the font, so that
> the line is drawn in its size step's font.
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

> **Narrowed (revision 4, CW43):** callback 0x01D60032 serves the reply word and
> `ChatModeReply` (the last sender), 0x01D60030 the retell word (the last player told); so
> Shift goes with senders and Ctrl with recipients. Whether a repeated press steps through
> several names is still open (Q11, Q26). Tessera rule for P4: §15.10 CT3.

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
| 0x048CE78C | 6 `Tell` (retell or tell-again word: target from callback 0x01D60030; corrected in revision 4, CW43) |
| 0x0A48C3E9 | 6 `Tell` (reply word: target from callback 0x01D60032; corrected in revision 4, CW43) |
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

> **Narrowed (revision 4, 2026-10-11; (a) corrected in the revision 4 fixes, review B1):**
> (a) Step 3 of the switch, corrected (CW44): if the row's type is `Tell`, a reply word asks
> callback 0x01D60032 and a retell word callback 0x01D60030 (CW43), and the target becomes
> the callback's name, or empty when it has none; a `Tell` word that is neither (the plain
> tell words) **stops the switch**: the mode, the target and the entry text stay as they
> were, with the typed space in the entry, so the player goes on typing a command line such
> as the word, a recipient and a text, which the submit runs as a command (§7.2, CW45). For a
> row of any other type the target becomes empty (not "unchanged", as step 3 said). An empty
> `Tell` target comes from a reply or retell word without a name, from showing the generic
> instant-message row (§15.4 step 6), and (Probable, the tab-switch read was not traced) from
> taking back a tab's stored `Tell` mode after another word emptied the target; a line typed in that state is built
> as *C*, two spaces, the text (§7.2 step 5), so the `tell` handler takes the first word of
> the text as the recipient. (b) The variant test: the scripts register the
> fixed fellowship word (`Commands` 0x038453D4) always and the first variant's word
> 0x053DBC60, with the same handler, only when a script test holds; the same test chooses
> the "group" forms of the fellowship lines and refusals (CW51, CW52) and the other variant
> registrations; Probable: the monster-play test. Still open: the map
> chat type → command word, and the channel button's menu.

> **Answered (2026-10-09, revision 3, Tessera rule; was OPEN, implementation, M5 clean room):**
> confirmed as the Tessera rule while Q12 is open. Until Q12 is settled, `tessera-ui`
> (`game::chat`) takes as the command word *C* of a mode (§6.2 step 2) the first name of the
> first ordinary row (not a reply or tell-again word) of that chat type in the mode table, or
> no word when there is none. The `OutputTextType` label shows the channel's display name
> when Tessera has one for the mode, else that command word (with the tell target after it
> for `Tell`); the original's template 0x04F7C6D9 is not used.

> **Answered (2026-10-09, revision 3; was OPEN, implementation, M5 clean room, 2026-10-08):**
> until Q12 names the game-state test, Tessera reads only the 26 fixed rows of the mode table
> and leaves out both variant groups; this is now the Tessera rule of §10 item 3. The two
> entries with token 0x04DA4989 (`CommandSetup` default word and the `Commands` entry of the
> `Say` row) hold the same text in this install (CW42), so registering the `Say` row's names
> and the default word gives one name. Both are still read from the install at run time and
> registered separately, because another language or a later build may hold different texts;
> the registry's first-registration rule (§7.1) makes a repeat a no-op.

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

> **OPEN (implementation, M5 clean room):** `tessera-ui` (`game::chat`) reports a trip to the
> client (`TextInputOutcome::GuardTripped`) instead of raising its UI sound event, because
> that event names a sound-info DID and 0x50 is a UI sound ID (Q13). Which record the client
> plays for it waits for Q13.

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

> **Narrowed (revision 4, OBS-S):** one script body registers 79 `Commands` entries by token
> (three pairs of them under the variant test of §6.5), among them the help, say, yell,
> tell, reply, retell, friend, ignore, fellowship, join, leave, list and user-channel entries
> of §15. Whether other bodies register more was not checked.

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
   *Revision 4 (P4):* the instant-message tab is now created by the §4.6 rule; §15.10 CT6.
2. **Lines in the sandbox and on a test server** (revised 2026-10-09 for D20). Tessera's own
   informational lines (help, player-music messages, notices) are type 8 `Log` lines; its
   refusals and the unknown-command line are type 1 `Error` lines. Plain text submitted for a
   channel runs as that channel's command through the client's dispatcher (§7.2 step 5, D20
   item 2); the channel command hands it to the world session (the sandbox backend, or the test
   server with `--server`), never to a log directly. The sandbox echoes it back as a session
   line of the channel's chat type (D20 item 3), formatted with Tessera's own wording (the
   original's channel templates are game text and server behaviour); that line reaches the logs
   (§4.3) and `Turbine.Chat.Received` like every session line, at the time item 9 states. The
   window keeps no local echo of its own. Without a session the line is refused with a type 1
   line and nothing is sent.
   *Revision 4 (P4):* session lines on `Tell`, `Fellowship` and `Named` channels take the
   original's line forms and chat types (§15.10 CT1); the lines of the P4 built-ins take the
   original's chat types (§15.10 CT2), which override the types of this item for them.
3. **Registry** (revised 2026-10-09 for D20). Tessera keeps one registry, owned by the
   client's command dispatcher (D20 items 1 and 6), with the rules of §7.1. Its own built-in
   commands (help, the plugin-manager command of LPA §6.4, the channel commands of D20 item 2,
   player music and its other own commands) are registered under Tessera's names **and**, where
   the original has the same command, under every name of the original's `Commands` entry read
   from the user's install: the plugin-manager entry (token 0x073F0613), the help entry (token
   0x0DA3EC70), and the channel entries of the **26 fixed rows** of the mode table of §6.5
   (Tessera implements them as the channel commands of item 2). The `CommandSetup` default
   channel word (token 0x04DA4989) is registered as a `Say` word too, so plain text always finds
   its command; in this install it is the same name as the `Say` row's (CW42), and the second
   registration is then a no-op. Neither variant group of §6.5 is registered while Q12 (the
   game-state test that picks one) is open, so `IsCommand` answers false for their words. No
   other original command name is registered; `IsCommand` answers false for them (difference
   recorded in §13 Q15). *Revision 4 (P4):* the user-channel entries of §15.6 are registered
   too (§15.10 CT2); the friend and ignore entries are `social-panels.md` §9 TR11.
   **When the names arrive.** The Tessera names are registered at start-up. The install's
   names are registered as soon as the dispatcher has read the `Commands` and `CommandSetup`
   tables, after the archives open. Plugins are loaded only after that read has finished or
   failed, so a plugin can never take an original name (D20 item 6: client names are
   registered before any plugin loads); start-up command lines wait for it as well (D20 item
   7). If a table cannot be read, only the Tessera names exist, the prefix is `/` (D20 item 1)
   and a warning is logged. The plugin host keeps no command table of its own: `Shell.AddCommand`,
   `RemoveCommand` and `IsCommand` reach this registry with the owner of the calling apartment
   (D20 item 6), and the plugin-manager command is a client built-in that calls the plugin
   manager.
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
   token 0x00CC89CC text with mask 0x21 (§11.8). For plugins, see Q18. *Revision 4 (P4):*
   with tells there is a reply target once a tell has arrived; §15.10 CT3.
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

11. **The entry line on the dispatcher** (D20 item 1; answers the implementer OPEN of
   2026-10-09). The chat window owns no registry, line pipeline or history; §6.3 and §7.1–§7.5
   are the dispatcher's, and the walker overlay before M5 used the same dispatcher.
   - **Order of a submit.** The window does §6.2 submit steps 1–2 (it reads the text and takes
     the mode's command word *C* and target *G*), then step 5 (it clears the entry and restores
     the mode from the shown tab), and then hands the text, *C* and *G* to the dispatcher, which
     does steps 3–4 (§7.2 and, on result 3, the §7.6 line). The window never sees the result
     code. This moves the original's step 5 before steps 3–4. The difference cannot be seen as
     long as no command handler reads or changes the entry text or the window's channel mode,
     which holds for every Tessera handler; a handler that needs either must be run before
     step 5 instead.
   - **When the line runs.** In frame-loop step 1 of IB D8, right after the input event (or
     pending offer) that submitted it, once the UI is no longer borrowed, with the world session
     of that frame (`--server` or the sandbox). Command lines of quickslot aliases run after
     step 7 of the same frame, as plain submissions in the `Say` mode with the default word as
     *C* and no target.
   - **Where the replies go.** Lines the dispatcher posts (the §7.6 line, help, refusals,
     player-music messages) are appended to the logs in step 7 of the same frame, by §4.3 with
     the masks of item 2 (type 1 or type 8), and are delivered to `Turbine.Chat.Received` like
     Tessera's notices (item 9).
   - **Mode words.** The window takes its mode table (§6.5) from the dispatcher's one read of the
     install's tables, so it offers only mode words that the registry holds. A typed word that
     the registry does not hold gives the §7.6 line; nothing is sent.
12. **Parser details** (D20 item 1; answers items (c)–(e) of the implementer OPEN of
   2026-10-08).
   - **Case folding.** Names are compared after Unicode default lower-casing, without a locale.
     The original folds case under the C-runtime locale of its language (§8.2, Probable). For
     the letters of English, French and German command names the two agree; they can differ only
     for characters outside those alphabets (for example a dotted capital I), a known
     difference.
   - **Positions.** The tokenizer counts UTF-16 code units, as §7.3 states. The removal of
     leading characters in §7.2 step 4 and §7.4 step 5 may compare whole characters: the result
     is the same for every line. In §7.4 step 5 an unquoted *W* is followed by a whitespace
     character, which never occurs in *W*, so the removal stops at the end of *W* in both
     counts, and a quoted *W* leaves the opening quote, which is not in *W*; in §7.2 step 4 the
     prefix is one character of the Basic Multilingual Plane (CW27a).
   - **Bounds.** A line longer than 4,096 bytes of UTF-8, from any source, is refused with a
     type 1 line before §7.2 runs, so it does not reach the history (the entry's 384 characters,
     CW20, stay below the bound). Channel text longer than the Tessera protocol's `TEXT` limit
     (1,024 bytes, `tessera-protocol.md` §5.2) is refused locally with a type 1 line and not sent.
   - **`tell` (Tessera name).** The first token of the arguments (§7.3; in double quotes when
     the name has spaces) is the recipient; the text is the rest of the arguments after that
     token and the whitespace that follows it, trimmed at both ends.
   - **Wording.** The refusal lines (channel not available, no reply target, no recipient in the
     sandbox, not connected, not in the world) are in Tessera's own words; only their types are
     normative here. A start-up command line (D20 item 7) that resolves to a channel command is
     logged, shown as a type 8 line and dropped.
13. **History sources** (answers item (f) of the implementer OPEN of 2026-10-08). Every
   submission that reaches §7.2 step 1 is added to the history (§6.3), whatever its source:
   the chat entry, the walker overlay before M5, a quickslot alias. Start-up command lines are
   the one exception (they are not the player's typing). Browsing works only in the chat entry
   (the overlay had no line up or down).

> **Answered (2026-10-09, revision 3; was OPEN, implementation, client command dispatcher,
> 2026-10-08):** (a) only the 26 fixed rows are registered while Q12 is open: §10 item 3.
> (b) when the install's names arrive: §10 item 3, with one correction: plugins load only
> after the dispatcher's read of the tables has finished or failed, so the case "a plugin that
> registered one of those names earlier keeps it" cannot arise. (c), (d), (e): §10 item 12.
> (f): §10 item 13. (g) is superseded by D20 item 6 as written into §10 item 3: the plugin host
> keeps no command table, and the plugin-manager command is a client built-in.

> **Answered (2026-10-09, revision 3; was OPEN, implementation, chat window on the
> dispatcher, 2026-10-09):** (a), (b), (c), (e): §10 item 11 (confirmed; the submit order is
> allowed under the condition stated there). (d): §10 item 2 (the window's local echo is
> replaced by the session's echo; without a session the line is refused).

> **OPEN (implementation, revision 3 conformance, 2026-10-09; engineering choices, no fact
> about the original):** §10 items 3 and 11 are implemented in `apps/client` (`commands/`,
> `plugins.rs`, `panels/chat.rs`, `ui_host/`) and `crates/plugins` (`root_ns/shell.rs`,
> `HostApi` command methods). Choices made where the spec is silent; please confirm or
> correct: (a) **`AddCommand` names** reach the registry as the trimmed pieces of LPA §11.1,
> with no other change (the earlier plugin host also removed a leading `/` and folded case
> to ASCII; that is gone, so a piece `/x` is stored as `/x`); `IsCommand` passes its argument
> unchanged (§8.2). (b) **What waits for the tables.** The plugin manager's whole per-frame
> work (not only the queued loads) waits until the dispatcher's read has finished or failed;
> with no archives it does not wait. (c) **What the window takes from the read.** Besides the
> mode table, the command prefix, the substitution marker and the default channel word come
> from the same read (an entry that was not read keeps Tessera's default); with no mode row
> read the window has one `Say` row on the default word. The window takes a new read in
> frame-loop step 7 and keeps each tab's current mode. (d) **Plugin command identity.** Each
> `ShellCommand` object of an apartment gets one handler key; `RemoveCommand` removes every
> name of that object; an apartment's unload removes every name it holds. `/help` asks a
> plugin command's `GetShortHelp` or `GetHelp` when it runs.

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

### 11.9 A tell, a reply in the `Tell` mode, and the instant-message tab (revision 4)

Synthetic: player P = `Ana`; default tabs (§3.3) plus the instant-message tab (§4.6);
`GOP_AutoCreateIMSessions` true. The tell word is written `T` and the reply word `R` (the real
words are game text; prefix `/`).

1. Event `ChatMessage { Tell { peer: "Bram" }, sender: "Bram", text: "hi" }`. *S* = `Bram`, not
   P: incoming (CT1). Tab 1 appends *Chat*(0x0BCA09CC; 0x52615 = `Bram`, 0x5277 = `hi`), mask
   0x20: type 6 style, sound 0x2A001004. Tab 2 does not (no bit 5). R = `Bram`. The
   instant-message tab has no `Bram` session: it creates row 1 `Bram` after the generic row
   (not shown; unread marker on) and appends *Chat*(0x0F2CF919; 0x58E15 = `14:02:07`, 0x52615 =
   `Bram`, 0x5277 = `hi`) to its log.
2. The player types `/R` and a space: the switch finds a reply word, the mode becomes `Tell`
   with target R = `Bram`, the entry is cleared. The player types `thanks` and Enter: the
   dispatcher runs `T␠Bram␠thanks` (§7.2 step 5); tokens `T`, `Bram`, `thanks`; recipient
   `Bram`, text `thanks`; `Chat { Tell { peer: "Bram" }, "thanks" }`; T (retell) = `Bram`.
   (Through the `reply` command instead, `/R thanks`, the same command is sent and T is not
   changed.)
3. Echo `ChatMessage { Tell { peer: "Bram" }, sender: "Ana", text: "thanks" }`: own (CT1).
   Tab 1 appends *Chat*(0x0AD74A23; 0x5866BA4 = `Bram`, 0x5277 = `thanks`), mask 0x20; session
   `Bram` appends *Chat*(0x0F2CF919; …, 0x52615 = `Ana`, 0x5277 = `thanks`). R stays `Bram`.
4. The plain tell word: `/T` and a space switches nothing (CW44); the entry holds `/T␠`, the
   mode stays as it was. Typing `Cara hello` and Enter submits `/T␠Cara hello` as a command
   line (§7.2): tokens `T`, `Cara`, `hello`; recipient `Cara`; T (retell) = `Cara`. An empty
   `Tell` target comes, for example, from a reply or retell word without a name (had no tell
   been received before step 1, `/R` and a space would give `Tell` with an empty target, and `Cara hello` would then run
   `T␠␠Cara hello`, the same tell) or from showing the generic row (§15.4 step 6).
5. In the sandbox, the tell to `Cara` is answered by the feed's `System` notice (SF §7.6),
   shown as a type 8 line in tab 1; no line goes to the instant-message tab (CT6).

### 11.10 Tell refusals (revision 4)

| Typed (P = `Ana`, no tell received yet, none sent) | Result |
|---|---|
| `/T␠Bram` | the usage, mask 0x8 (one token after the word) |
| `/R␠ok` | *Chat*(0x00CC89CC), mask 0x21: tab 1 appends it once (bits 0 and 5), styled type 6, no sound (§11.8) |
| retell word with `ok` | *Chat*(0x0811EFD4), mask 0x21 |
| `ChatModeReply` | *Chat*(0x00CC89CC), mask 0x21; the entry stays closed |

### 11.11 User channels (revision 4)

Scenario steps `channel join "Lore"` arrive after the player's `/join Lore` (`JoinChannel {
"Lore" }` sent). The words `/1` … `/8` stand for the slot rows' words.

1. `ChannelJoined { "Lore" }`: slot 1 ← `Lore`; *Chat*(0x09A99A08; 0x4E28A8 = 1), mask 0x8
   (type 4).
2. `/1 hello` → `Chat { Named { "Lore" }, "hello" }`. Echo `ChatMessage { Named { "Lore" },
   sender: "Ana", "hello" }` → *Chat*(0x009D15CC; 0xAD99BC5 = `Lore`, 0x5277 = `hello`), mask
   0x08000000 (type 28). A line from `Bram` → *Chat*(0x063C924C; 0x7C632DC = `Lore`, 0x8495FF2
   = `Bram`, 0x5277 = …), same mask.
3. `/join lore` → *Chat*(0x07548D0C; 0x2E8E425 = `lore`): name-equal to slot 1.
4. `/join Lore pw` → *Chat*(0x08DC4764) (no passwords).
5. `/3 x` → *Chat*(0x0E2A7CE8; 0x539D = 3), mask 0x8.
6. `/channels` → *Chat*(0x09AAEAC2) (mask 0x8); slot 1: *Chat*(0x000EE4F4; 1, `Lore`, empty)
   with mask 0x08000000; slots 2–8: *Chat*(0x0EBE4334; *k*) with their masks (slot 5:
   0x4000000000, type 39).
7. With slots 1–8 filled, `/join Nine` → *Chat*(0x002AAA94; 0x539D = 8); nothing sent.
8. `/part Lore` → `LeaveChannel { "Lore" }`; `ChannelLeft { "Lore" }` → slot 1 freed,
   *Chat*(0x03B095F8; 0x4E28A8 = 1), mask 0x8. `/part Lore` again → *Chat*(0x0DF4808C;
   0x2E8E425 = `Lore`).
9. On `--server` (CT11): with negotiated minor 2 or higher and `CHAT_CHANNELS` advertised,
   `/join Lore` sends `JoinChannel { "Lore" }` and steps 1–8 follow from the server's
   answers (a refusal arrives as a `System` line in Tessera's words, SF §9.8); below minor 2
   or without `CHAT_CHANNELS`, `/join Lore` → *Chat*(0x04068445), mask 0x8, nothing sent.

### 11.12 Fellowship (revision 4)

The fellowship word is written `F`. No `FellowshipChanged` yet: `/F hi` →
*Fellowship*(0x06358717), mask 0x1 (type 1); nothing sent. After `FellowshipChanged` with
`Ana` and `Bram`: `/F hi` → `Chat { Fellowship, "hi" }`; the echo (sender `Ana`) →
*Chat*(0x04A3E744; 0x5277 = `hi`), mask 0x400, type 11, sound 0x2A001003; `Bram`'s line →
*Chat*(0x05D20D84; 0x8495FF2 = `Bram`, …), mask 0x400. The mode `F` can be chosen before
joining a fellowship; a line typed in it gets the refusal.

### 11.13 Ignore filter (revision 4)

`IgnoreList { ["Cara"] }`. Dropped without a trace: `ChatMessage { Say, "cara", … }` (name-equal),
`ChatMessage { Tell { peer: "Cara" }, "Cara", … }` (R unchanged, no session row),
`ChatMessage { Named { "Lore" }, "CARA", … }`. Kept: `ChatMessage { System, none, … }`; the
player's own tell to `Cara` (sent, and its echo shown, since *S* = P).

### 11.14 Friend notices (revision 4)

| `FriendList` event (in order after world entry) | Lines (mask 0x8) |
|---|---|
| `Bram:off, Cara:on` | none (first list) |
| `Bram:on, Cara:on, Dain:on` | *Chat*(0x0A29AFC5; 0x52615 = `Bram`) (`Dain` is new: none) |
| `Cara:off` | *Chat*(0x010538E5; 0x52615 = `Cara`) (`Bram`, `Dain` removed: none) |

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

Revision 4 data invariants (check against the install):

| # | Invariant |
|---|---|
| CW-I17 | These `Chat` tokens exist with exactly these argument IDs in this order: 0x0BCA09CC (0x4DD4, 0x52615, 0x5277); 0x0AD74A23 (0x4DD4, 0x5866BA4, 0x5277); 0x0F2CF919 (0x58E15, 0x4DD4, 0x52615, 0x5277); 0x0F2C3BE2 (0x58E15, 0x52615); 0x04E6F672 (0x58E15); 0x01A17E05 (0x5866BA4); 0x05D20D84 (0x4DD4, 0x8495FF2, 0x5277); 0x04A3E744 (0x5277); 0x063C924C (0x7C632DC, 0x4DD4, 0x8495FF2, 0x5277); 0x009D15CC (0xAD99BC5, 0x5277); 0x09A99A08, 0x03B095F8, 0x0EBE4334 (0x4E28A8); 0x000EE4F4 (0x4E28A8, 0x2E8E425, 0x12E6A6F); 0x066B746F (0x688C124); 0x002AAA94, 0x0E2A7CE8 (0x539D); 0x02832AE7 (0x7C632DC, 0x50A2C88, 0xD0A7DE8); 0x01919744, 0x03C043D4 (0x7C632DC); 0x07548D0C, 0x0DF4808C, 0x02E32555 (0x2E8E425); 0x0A29AFC5, 0x010538E5 (0x52615); and without arguments 0x00CC89CC, 0x0811EFD4, 0x0D93B0C5, 0x0EA68129, 0x0AC8421C, 0x08DC4764, 0x04068445, 0x09AAEAC2, 0x01513613, 0x02E819D4. |
| CW-I18 | The `Commands` entries 0x0DA4AC6C, 0x0A48C3E9, 0x048CE78C, 0x038453D4, 0x03A7284C, 0x0DCD958C, 0x0A55AF03, 0x098B3A44 and 0x08B3A4E1–0x08B3A4E8 exist and each splits (§7.1) into at least one name; the `Fellowship` table (STRINGTABLE key 0x10000017) has token 0x06358717 without arguments. |
| CW-I19 | `GameplayOptionDefaults` holds the booleans `GOP_FullIRCMode` and `GOP_AutoCreateIMSessions`. |
| CW-I20 | The element named by `IMTabTemplate`'s `UI_ChatTab_IMLogElement` names, by `UI_ChatLog_IMListboxElement`, a list box with `UICore_ListBox_max_rows` > 0 whose `UI_ChatLog_IMTextElement` row template names, by `UI_ChatTab_LogElement`, a log whose list box has `UICore_ListBox_max_rows` > 0. |

Revision 4 behaviour invariants (synthetic tests, Tessera rules of §15.10):

| # | Invariant |
|---|---|
| CW-I21 | At most 8 slots hold a channel; no two held names are name-equal; every line of a channel in slot *k* carries slot *k*'s mask (§15.6 table); slots change only on `ChannelJoined`, `ChannelLeft`, `Disconnected` or a world entry. |
| CW-I22 | `JoinChannel` is sent only when fewer than 8 slots are held and no held name is name-equal to it; `Chat { Named { n } }` only for a held *n*; `LeaveChannel` only for a held name. |
| CW-I23 | A `ChatMessage` on `Say`, `Yell`, `Emote`, `Tell`, `Fellowship` or `Named` whose speaker is name-equal to an ignored name and not to P reaches no log, no instant-message session, no sound and no plugin event, and changes neither R nor the recent names; a `System` line is never dropped. |
| CW-I24 | A `Tell` line is own exactly when its speaker is name-equal to P and its `peer` is not; an own line never changes R. |
| CW-I25 | R is the speaker of the most recent incoming `Tell` line that passed CT8 (or none); T changes only when `tell` hands a tell to the session. |
| CW-I26 | Instant-message rows: the generic row is row 0; rows 1, 2, … are in strictly ascending ordinal order of their names, no two name-equal; the row count never exceeds the session list's `max_rows`. (The order is a Tessera rule, CT6: the original renames rows in place and never moves them, §15.4 step 4.) |
| CW-I27 | Every CT1 `Tell` line goes to exactly one instant-message session, except when the list is full and auto-creation is on (then to none). |
| CW-I28 | A `FriendList` event posts at most one notice per name, only for a name in both it and the previous list whose `online` differs; the first after a world entry posts none. |
| CW-I29 | `fellowship` never sends `Chat { Fellowship, … }` while the last `FellowshipChanged` list is empty (or none arrived). |
| CW-I30 | Every line Tessera posts for §15 has the mask the §15 tables give, whether its text comes from the install or from Tessera's fallback wording. |
| CW-I31 | (Revision 4 fixes.) A space typed after a plain tell word (a name of `Commands` 0x0DA4AC6C, alone in the trimmed entry) changes neither the mode, nor the `Tell` target, nor the entry text, which keeps the space; a space after any other mode word clears the entry, and the target is then the reply or retell name (possibly empty) for a reply or retell word and empty for every other word. |
| CW-I32 | (Revision 4 fixes.) Delivering a `Tell` line to the instant-message tab never changes the entry's mode or `Tell` target; only showing a row does (CT6). |

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
| Q12 | Variant test of the mode table; the mode → command-word map (the plain `Tell` word: answered, it switches nothing, CW44) | §6.5, §6.2 | M4 |
| Q13 | Which sound record UI sound 0x50 is | §6.6 | M4 on the UI sound table |
| Q14 | Substitution words and values; the whitespace switch | §7.5 | M4 on script callback 0x00F50007 |
| Q15 | Exact set of built-in command names the original registers | §8.1, §10 | M4 on the script natives; M3 (help output counted) |
| Q16 | `ChatDisplayText` (0x10000308): which window part uses it (relates to FO §9.3 Q14) | §3.1 | M4 |
| Q17 | Whether the game script that receives the reply line (0x21) and the unknown-command line (0x1) passes the mask and text back unchanged | §6.2, §7.6 | M4 on the script entry point; M3 |
| Q18 | `ChatType` reported to `Turbine.Chat.Received` for a line whose mask has several type bits | §9 | M4 on the plugin chat event's conversion; M3 with a logging plugin |
| Q19 | Which input map supplies the log-paging commands while the entry does not have focus (Tessera rule: §10 item 10, IB Known difference 17) | §6.2 | M4 on the window's activation; M3 (Page Up with the window active and the entry closed) |
| Q20 | The identifier flag that lets an incoming tell into a log without bit 5 (sender flag set, player's flag clear) (Tessera: not applied, CT1) | §15.3 | M4 on the instance-ID flag byte's users |
| Q21 | How tell failures show without the instant-message tab or in the normal tabs; which failure kind gives which line and which closes the session; which chat types the server filters for ignored players; whether *Chat*(0x0B891524) and 0x018E5F35 are server-composed (Tessera: no failure lines, CT6; client-side filter, CT8) | §15.5, §15.8 | M3 (a tell to an offline name with the instant-message tab off; ignore a speaker) |
| Q22 | Whether the server also sends the speaker's own fellowship and user-channel lines back besides the scripts' local echo; the condition of the dispatcher's fallback for a channel in no slot (Tessera: the session's echo shown in the echo form, CT1) | §15.6, §15.7 | M3 (speak on the fellowship channel and count the lines) |
| Q23 | Whether the library's string-length native counts a terminator (the "minus one" of the join checks); the case rule of the name comparison of channel names; which slot a joined channel takes (Tessera: code units, name-equal, lowest free slot, CT5) | §15.6 | M4 on the script natives (packages 225 and 176) |
| Q24 | The text transformation before the 385-unit check and the line of the generic error code 38; which line the generic error routine posts for the result codes of a user-channel send (Tessera: §10 item 12's bound, CT4; server refusals as `System` lines, CT11) | §15.2, §15.6 | M4 on native 3295.9 and the error routine's table |
| Q25 | What the row's close button (`IMSession_CloseButton`, identified by its label, CW58) and its unread and logging markers do; the "close session" item beyond removing the row; whether an auto-created row shows its unread marker (Tessera rule CT6) | §15.4 | M4 on the row class; M3 |
| Q26 | Where the scripts keep the last sender and the retell target, when they change, and whether Shift or Ctrl with line up and down step through several names (extends Q11; Tessera rule CT3) | §15.2, §6.4 | M4 on callbacks 0x01D6002F–0x01D60032 |
| Q27 | The effect of `GOP_AcceptFriendsAndGuildMemberTellsOnly` (Probable: a server filter; Tessera: not applied) | §15.8 | M3 |
| Q28 | The session restriction of the join check, the source of `World_UserDefinedChatEnabled`, the rate guard's fixed parameters (Tessera: none of them applied, CT5) | §15.6 | M4 on the script natives |

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
| `social-panels.md` §9 TR5 | "Friends notices of §3.7 are not derived" | **Superseded for the online and offline notices** (revision 4): the client half derives them from consecutive `FriendList` events (§15.10 CT9), as the original's script posts them on a change of a friend's online bit (CW55). The other §3.7 notices (added, removed, already, full, empty) stay as TR5 says. |
| `social-panels.md` §3.7 | Chat types of the friend notices | The online and offline notices are posted with mask 0x8, type 4 `Standard` (CW55). |
| `social-panels.md` §9 TR11 | Tessera command names | No overlap: §15.10 CT2 adds `join`, `part`, `channels`, `chsay`, `ch1`–`ch8`, `reply`, `retell`, `fellowship`; TR11's `leave` stays the fellowship command. |
| `scenario-feed.md` §6.4, §9.2, §9.4 | Social chat | `ChatMessage` has no own-line flag, so the client classifies a line as its own by the sender's name (§15.10 CT1); `JoinChannel` has no password (CT5 refuses one locally); the plain sandbox ignores `JoinChannel` (§9.5), so user channels show only with a scenario's `channel` steps (CT11); on the test server they need negotiated minor 2 with `CHAT_CHANNELS` (§9.8, CT11, revision 4 fixes). No change is asked for; a later revision may add `ChannelJoined` answers to the plain sandbox. |
| `lua-plugin-api.md` §11.3 | `ChatType` of the P4 lines | Tells 6, fellowship 11, user channels 28–31 and 39–42 by slot, channel and friend notices 4, refusals 1 (or 6 for the 0x21 lines, Q18); §15.10 CT10. |

---

<a id="s15"></a>
## 15. Social chat (revision 4, P4)

Decision 0024 P4 asks for social chat as the original presents it, fed by the scenario feed
and protocol v1.1, client half only. §15.1–§15.9 state what the original does (marks per
row of §1); §15.10 states Tessera's rules (CT1–CT12), all in terms of the decision-0020
dispatcher (§10 items 11–13).

**Notation.** *Chat*(*t*; *a* = *v*, …) is the entry with token *t* of the `Chat` table
(STRINGTABLE key 0x1000004D, 0x250001A7 here) rendered by ST §7 with argument ID *a* set to
*v*; *Fellowship*(*t*) and *Weenie*(*t*) are entries of the tables with STRINGTABLE keys
0x10000017 and 0x10000039. "Mask *m*" is the line's 64-bit chat-type mask (§2): 0x1 type 1
`Error`, 0x8 type 4 `Standard`, 0x20 type 6 `Tell`, 0x21 types 1 and 6, 0x400 type 11
`Fellowship`. A line "posted with mask *m*" goes to every log by §4.3 and is appended by
§5.1 (style, timestamp, sound, unread marker). No template text is reproduced; roles are
described in words.

<a id="s15-1"></a>
### 15.1 Overview and the session layer

| Feature | Original | Tessera command or event (SF §9) | Tessera rule |
|---|---|---|---|
| Sending a tell: tell, reply and retell commands, `Tell` mode, `ChatModeReply` | §15.2 | `Chat { channel: Tell { peer }, text }` | CT3, CT4 |
| A tell received, the player's own tell coming back | §15.3, §15.4 | `ChatMessage { channel: Tell { peer }, sender, text }` | CT1, CT6 |
| Fellowship line sent / received | §15.7 | `Chat { Fellowship, text }` / `ChatMessage { Fellowship, … }` | CT1, CT7 |
| Joining, leaving, listing user channels | §15.6 | `Social::JoinChannel { name }`, `Social::LeaveChannel { name }`; list: none (local) | CT5 |
| Joined, left | §15.6 | `Social::ChannelJoined { channel }`, `Social::ChannelLeft { channel }` | CT5 |
| User-channel line sent / received | §15.6 | `Chat { Named { name }, text }` / `ChatMessage { Named { name }, … }` | CT1, CT5 |
| Ignore filtering | §15.8 | `Social::IgnoreList { names }` | CT8 |
| Friend online and offline notices | §15.9 | `Social::FriendList { entries }` | CT9 |
| Fellowship membership (for the fellowship refusal) | §15.7 | `Social::FellowshipChanged { members }` (SP §9 TR2) | CT7 |

<a id="s15-2"></a>
### 15.2 Sending tells (BIN2: CW43, CW44; OBS-S: CW45, CW46)

**Command entries.** Three `Commands` entries send tells, all registered by the scripts
(§8.1) and all rows of the mode table (§6.5), so their names are both commands and mode
words: 0x0DA4AC6C (tell; several names), 0x0A48C3E9 (reply) and 0x048CE78C (retell). A fourth
entry with one name, 0x06D8CE2C, is not registered as a command (OBS-S).

**Tell** (CW45). The handler receives the token list of §7.3 and drops the command word.

1. With fewer than two tokens left, it posts the command's usage (mask 0x8) and stops.
2. The recipient is the first token. The text is the remaining tokens joined with one U+0020
   each (so runs of whitespace collapse and quotes of quoted tokens are gone).
3. An empty text posts *Chat*(0x01A17E05; 0x05866BA4 = recipient) with mask 0x21, then the
   usage, and stops. (With two or more tokens the text is never empty, because tokens are
   never empty, §7.3.)
4. **Length check** (the same in reply, retell, fellowship and user-channel sends): a text
   longer than 1,925 units by the library's string-length native, or longer than 385 units
   after a further library transformation of the text, goes to the scripts' generic error
   routine with code 38 and stops (the transformation and the line: Q24). In the tell
   handler the text measured is the handler's raw argument text (the §7.4 arguments,
   recipient included), not the re-joined text of step 2 (revision 4 fixes). The entry line
   holds at most 384 characters (CW20), so typed text passes.
5. The recipient and the text go to the client's tell send (the chat subsystem; not
   traced), and the recipient becomes the **retell target**.

**Reply and retell** (CW46). With no token after the command word, the usage (mask 0x8). The
target is the **last sender** (reply) or the retell target (retell). Without one:
*Chat*(0x00CC89CC) (reply) or *Chat*(0x0811EFD4) (retell), mask 0x21. Otherwise the length
check, then the tell send with the §7.4 arguments as typed (not re-joined). Neither command
changes the retell target. Where the scripts keep the last sender, and when it changes, was not
traced (Q26; Probable: the sender of the most recent incoming tell).

**The `Tell` mode** (CW44; §6.5; corrected in the revision 4 fixes, review B1). A space typed
after a reply or retell word switches the entry to `Tell`, with the reply callback's name
(reply word) or the retell callback's name (retell word) as the target, or an **empty**
target when the callback has none. A space typed after a plain tell word (a word of the tell
entry 0x0DA4AC6C, in neither set) switches nothing: the mode, the target and the entry text
stay, the space stays typed, and what the player types next is submitted as a command line
(§7.2), which the `tell` handler above reads. The `Tell` mode is also entered by
`ChatModeReply` and by showing an instant-message session (§15.4 step 6, the generic row
giving an empty target). In the `Tell` mode a submitted text runs as *C*, a space, the
target, a space, the text (§7.2 step 5): with a target the handler gets the target as the
first token; with an empty target the line has two spaces after *C*, so the first word the
player typed is the recipient.

**`ChatModeReply`** (§6.2): with a last sender, the entry opens and the name goes to the
`Tell` mode as its target (Probable); without, *Chat*(0x00CC89CC) with mask 0x21.
**Shift and Ctrl with line up and down** (§6.4): Shift asks the sender callbacks (0x01D60032
up, 0x01D60031 down), Ctrl the recipient callbacks (0x01D60030 up, 0x01D6002F down), CW43.

<a id="s15-3"></a>
### 15.3 Tell lines in the normal tabs (BIN2: CW47)

The tell lines are built by the client, not by the scripts: none of their tokens occurs in
the script library (OBS-S). Each log of a normal tab builds the line itself when the chat
subsystem reports the tell to the logs:

| Tell | Line | Mask | Which logs append it |
|---|---|---|---|
| Incoming | *Chat*(0x0BCA09CC; 0x4DD4 = the sender's instance ID, the target of the `Select` link (ST §4.8) around the name; 0x52615 = sender's name; 0x5277 = text) | 0x20 | every log whose mask has bit 5; also a log without bit 5 when an identifier flag of the sender is set and the same flag of the player is clear (Q20) |
| The player's own | *Chat*(0x0AD74A23; 0x4DD4 = recipient's instance ID; 0x5866BA4 = recipient's name; 0x5277 = text) | 0x20 | every log whose mask has bit 5 |

So by default only tab 1 shows them among the normal tabs (§3.3), each line is styled as
type 6 and plays the `Tell` sound 0x2A001004 (§5.6, an exact whole-mask match). The
instant-message tab gets its own form (§15.4). A third tell template,
*Chat*(0x045DBD34; 0x58A10A2, 0x58A1095, 0x5277), names both parties; a script posts it with
mask 0x4 (type 3 `Admin`) under a script condition (Probable: an administrator's view); it is
not part of P4.

<a id="s15-4"></a>
### 15.4 The instant-message tab and its sessions (BIN2: CW48, CW49; OBS)

**Elements** (layout 0x22000034, OBS). `IMTabTemplate` (§4.6) names its log with
`UI_ChatTab_IMLogElement` = `IMLogElement` (0x10000325, 512 × 440). That element names the
session list with `UI_ChatLog_IMListboxElement` = `IMSessionsListbox` (0x10000328: ListBox at
(400, 23), 101 × 417, one column, `max_rows` 128, click select, selected state 7, normal state
11, `selected_item_state_change`; `UI_ChatLog_IMTextElement` = the row template 0x1000032B)
and the parent of the shown session's log with `UI_ChatLog_IMLogParentField` (0x10000327,
392 × 440). The row template `IMSessionsListboxItemTemplate` (0x1000032B) names its log with
`UI_ChatTab_LogElement` = `IMChatLogElement` (0x1000032E: list box 0x10000316 with
`max_rows` 128, line template `UI_ChatLog_TextElement` 0x10000309, scroll bar 0x10000317), its
unread marker 0x10000319 and its logging marker 0x100012DD. Labels and bases (CW58, OBS4): the
session log's list box 0x10000316 is labelled `ChatLogListbox`, like the list box of the
normal logs (CW15a); the row's children are `IMSessionText` (0x1000032C, 100 × 19, based on
the `Button` `IM_Base_Button` of layout 0x22000004: the row's label, a button that shows the
name), `IMSession_CloseButton` (0x1000032D, 14 × 14 at (457, 2), based on the `Button`
`IM_Close_Button`; what it does beyond its name is Q25) and the two 14 × 14 markers
`UnreadIndicator` and `LoggingIndicator`.

**Start.** The tab's log reads its list and parent elements, creates the **generic** row
(label *Chat*(0x0D93B0C5), a reserved partner) and shows it, then counts as initialised.

**A tell line arrives** (incoming or the player's own; the partner is the sender or the
recipient):

1. Find the partner's session: by the partner's instance ID among the sessions recorded by
   ID; else by name among all rows (the generic row included, by its rendered label),
   compared case-insensitively. A session found by name takes the line's spelling of the
   name in place (its stored name and its row label; the row is not moved and nothing goes
   to the `Tell` mode) and is recorded under the partner's ID.
2. None found: if `GOP_AutoCreateIMSessions` (default true, CW57) is true, create a row
   labelled with the partner's name and record it under the ID; if the creation fails (a
   full list, CW49, Probable) the line is **not** shown in this tab. If the option is false,
   the generic row takes the line (it is not recorded under the ID).
3. A new row is inserted after the generic row, before the first row whose name compares
   greater (UTF-16 code units, ordinal, case-sensitive), so rows 1, 2, … stay in name order.
   It is shown only when no session is shown (never after start, since the generic row is)
   or when the player's own "start session" made it.
4. **Rename** (corrected in the revision 4 fixes, review B3). Whichever session takes the
   line, its stored name is compared with the line's partner name case-insensitively. Only
   when they differ **beyond case** is the session renamed: its stored name and row label
   become the line's name and it is recorded under the line's instance ID, in place (rows
   are never moved, so after a rename rows 1, 2, … need not be in name order). Then, once
   the tab has finished its start, the name of the **shown** session (empty when the generic
   row is shown) is handed to the primary window's `Tell` mode, whichever row was renamed.
   This happens to a session found by instance ID whose partner now has another name, and
   (Probable: the generic row's label renders as text and so differs) to the generic row
   each time it takes a line with auto-creation off: it then shows that partner's name and
   is found by that name, step 1, until another partner's line renames it again. A
   difference by case only never renames and never hands a name over (a session found by
   name already took the spelling in step 1).
5. The session's log gets *Chat*(0x0F2CF919; 0x58E15 = the current time as text, 0x4DD4 =
   the speaker's instance ID, 0x52615 = the speaker's name, 0x5277 = text), mask 0x20. The
   speaker is the partner for an incoming tell and the **player** for the player's own.
6. Showing a session (a click on its row, or step 3) hides the shown session's log, places
   the new one in the parent field, and hands the session's name, empty for the generic row,
   to the primary window's `Tell` mode, which stores it as the target and updates the label
   (revision 4 fixes, BIN2; Probable: the mode also becomes `Tell`). Step 4 uses the same
   hand-over.

**Menu** (the session list's context menu): "start session" (*Chat*(0x0780711E)), enabled
only while the row count is below the session list's `max_rows`; "close session"
(*Chat*(0x0E6C6E0D)); the tab menu's "enable instant messages" item (*Chat*(0x093D1E8D))
goes with `GOP_FullIRCMode`. "Start session" opens a text prompt (*Chat*(0x0D3D2E04)). An
empty answer posts *Chat*(0x0EA68129) with mask 0x1; an answer that the client's name
normalisation changes (compared case-insensitively) posts a refusal with mask 0x1 (Probable:
*Chat*(0x0C47E765)); otherwise the session is created as in steps 2–3 and shown.

**Limits.** At most `max_rows` of the session list rows, the generic row included (128 here);
each session's log holds at most its list box's `max_rows` lines (128 here, §5.2). No other
limit (time-out, count of lines per partner) was found.

This answers Q5 (§4.6) except the row's close button, unread and logging markers (Q25).

> **Answered (revision 4 fixes, 2026-10-11; was OPEN, implementation, P4 clean room,
> 2026-10-11):** (a) The labels are now given (CW58): the session log's list box is the
> element labelled `ChatLogListbox` (0x10000316), so taking `ChatLogListbox` (else the first
> `ListBox`) is right. The row's label is the child labelled `IMSessionText` (0x1000032C); its
> type comes from its base `IM_Base_Button`, a `Button`, so a search for the first `Text`
> element does not find it. Tessera puts the name on the element labelled `IMSessionText`
> (resolved through its base, UL), else on the first child that shows text, else on the row.
> (b) Step 4 is corrected: the original never moves a row, so the name order of step 3 can
> break after a rename, and CW-I26 is a **Tessera rule**. Tessera moves a row whose spelling
> changed to its place in name order (CT6), which also covers the only rename Tessera can
> see, a change of case through a session found by name.

<a id="s15-5"></a>
### 15.5 Tell refusals and failures (OBS-S: CW45, CW46; BIN2: CW50)

| Situation | Line | Mask | Shown in |
|---|---|---|---|
| tell with fewer than two tokens | the command's usage | 0x8 | logs (§4.3) |
| tell with an empty text (§15.2 step 3) | *Chat*(0x01A17E05; 0x05866BA4 = recipient), then the usage | 0x21 | logs |
| reply without a last sender; `ChatModeReply` without one | *Chat*(0x00CC89CC) | 0x21 | logs |
| retell before any tell | *Chat*(0x0811EFD4) | 0x21 | logs |
| text over the length limits | the generic error routine, code 38 (Q24) | — | — |
| the subsystem reports an invalid recipient name (one failure kind) | *Chat*(0x04E6F672; 0x58E15 = time) | 0x20 | the partner's session, else the generic row, of the instant-message tab |
| the subsystem reports another failure (Probable: offline or no such character) | *Chat*(0x0F2C3BE2; 0x58E15 = time, 0x52615 = name) | 0x20 | same |
| a third failure kind | the session found **by name** (case-insensitive, step 1 of §15.4 without the instance ID) is closed and no line is shown; when no session has that name, the line *Chat*(0x0F2C3BE2) of the row above, in the session found by instance ID, else the generic row (revision 4 fixes) | 0x20 when shown | — |
| the recipient ignores the player; the recipient is unavailable | *Chat*(0x0B891524) and *Chat*(0x018E5F35), argument 0x5506DA2 = name, exist but are referenced neither by client code nor by the scripts | — | Probable: lines composed by the server (Q21) |

The failure lines were found only in the instant-message log's receive interface; how a
failure shows with `GOP_FullIRCMode` off, or in the normal tabs, is Q21.

<a id="s15-6"></a>
### 15.6 User chat channels (OBS-S: CW51, CW53, CW54)

**Commands** (`Commands` entries, all registered by the scripts):

| Entry | Role | Arguments (tokens after the command word) |
|---|---|---|
| 0x03A7284C | join | a channel name and an optional password (1 or 2 tokens) |
| 0x0DCD958C | leave | a channel name (exactly 1 token) |
| 0x0A55AF03 | list | none |
| 0x098B3A44 | send by name | a channel name, then the text |
| 0x08B3A4E1 … 0x08B3A4E8 | send on slot 1 … 8 (also the mode words of §6.5) | the text |

**Slots.** A player has **8** slots (CW53). Slot *k* has one chat type and mask:

| Slot *k* | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| Chat type | 28 | 29 | 30 | 31 | 39 | 40 | 41 | 42 |
| Mask | 0x08000000 | 0x10000000 | 0x20000000 | 0x40000000 | 0x4000000000 | 0x8000000000 | 0x10000000000 | 0x20000000000 |

These are the types of the mode-table rows 0x08B3A4E1–0x08B3A4E8 (§6.5) and of the enum labels
`UserChat1`–`UserChat8`; tab 1 shows all eight by default (§3.3). The scripts keep the slots
as an array of 8 entries (free or a channel) and find "a free slot" as the first free entry;
where a joined channel is stored, and so which slot it takes, was not traced (Probable: the
first free one, Q23). They look a channel up by name with a string comparison whose case rule
was not identified (Q23).

**Join** (lines with mask 0x8 unless stated). With not 1 or 2 tokens, the usage. Otherwise, in
this order:

1. The world property `World_UserDefinedChatEnabled` false → *Chat*(0x04068445).
2. A session restriction (two player states and a further test, Q28) → *Chat*(0x056A564E).
3. Name and password: an empty name → *Chat*(0x0AC8421C); a name whose length by the
   string-length native, minus one, exceeds **50** → *Chat*(0x02832AE7; 0x7C632DC = name,
   0x50A2C88 = that length minus one, 0xD0A7DE8 = 50); a name that is a channel command
   (a name of the channel table the switch uses) → *Chat*(0x01919744; 0x7C632DC = name); a
   non-empty password whose length minus one exceeds **20** → *Chat*(0x046A54E7; 0x7C632DC =
   name, 0x688C124 = password, 0x50A2C88 = length minus one, 0xD0A7DE8 = 20); a password for
   one reserved channel name held by the library → *Chat*(0x08DC4764). (Whether the native
   counts a terminator, so that "minus one" gives the true length, is Q23.)
4. Already joined → *Chat*(0x07548D0C; 0x2E8E425 = name).
5. No free slot → *Chat*(0x002AAA94; 0x539D = 8).
6. A rate guard (`Antispam_UserChatChannelMod` with fixed parameters, Q28) → *Chat*(0x078F3ACE).
7. Otherwise the request goes to the chat subsystem. Refusals it reports later use the same
   lines, plus *Chat*(0x0360EAE4; 0x2E8E425 = name) and *Chat*(0x0DF4808C; 0x2E8E425 = name).

**Joined.** When the subsystem reports a channel entered: a user channel in slot *k* →
*Chat*(0x09A99A08; 0x4E28A8 = *k*); any other (area) channel → *Chat*(0x0C077CFC; 0x2E8E425 =
its name); mask 0x8.

**Leave.** With not exactly 1 token, the usage. An empty name → *Chat*(0x0AC8421C); a name
not joined that is a channel command → *Chat*(0x03C043D4; 0x7C632DC = name); any other name not
joined → *Chat*(0x0DF4808C; 0x2E8E425 = name); the rate guard; else the request goes to the
subsystem. **Left:** *Chat*(0x03B095F8; 0x4E28A8 = *k*) for slot *k*, or *Chat*(0x03A70DEC;
0x2E8E425 = name) for another channel; mask 0x8.

**List.** No channel joined → *Chat*(0x01513613), mask 0x8. Otherwise *Chat*(0x09AAEAC2)
(mask 0x8), then for *k* = 1 … 8 one line with **slot *k*'s mask**: joined →
*Chat*(0x000EE4F4; 0x4E28A8 = *k*, 0x2E8E425 = name, 0x12E6A6F = *Chat*(0x02E819D4), an empty
text, when the slot has no password, else *Chat*(0x066B746F; 0x688C124 = password)); free →
*Chat*(0x0EBE4334; 0x4E28A8 = *k*).

**Sending.** By slot (`/`*k* words and the mode `UserChat`*k*): slot *k* free →
*Chat*(0x0E2A7CE8; 0x539D = *k*), mask 0x8. By name: with no token the usage; the first token
is the name; not joined → *Chat*(0x02E32555; 0x2E8E425 = name), mask 0x8; the text is the
remaining tokens joined with one U+0020. Then, for both: the length check (§15.2), user chat
unavailable → *Chat*(0x04068445) with mask 0x1, a character too new for this channel →
*Weenie*(0x040CF227) with mask 0x8; otherwise the line goes to the subsystem. **After the
send** (CW54, revision 4 fixes, corrected in the C1 fix): both handlers reach the same send
body, so the results are the same for a send by slot and by name. A result of zero → the
script posts the local echo *Chat*(0x009D15CC; 0xAD99BC5 = the channel's name, 0x5277 =
text) with slot *k*'s mask. The one result code that a script test guards (the same code
and test as the fellowship send, §15.7) → the scripts' generic error routine with that code
when the test holds; otherwise it counts as zero and the echo is posted. Any other non-zero
result → the generic error routine with that code, and no echo (which line the routine
posts for a code is Q24). The user-channel send never posts *Chat*(0x015B0782); that line
follows a failed fellowship send (§15.7) and a send on a channel chosen by a chat-type mask,
a different script body that neither user-channel handler reaches.

**Received** (CW51): *Chat*(0x063C924C; 0x7C632DC = the channel's name, 0x4DD4 = sender's
instance ID, 0x8495FF2 = sender's name, 0x5277 = text) with the slot's mask. A line for a
channel in no slot takes a fallback built from library messages that runs only under a script
condition (Q22).

<a id="s15-7"></a>
### 15.7 The fellowship channel (OBS-S: CW51, CW52; BIN: CW24b)

**When it is offered.** The fellowship word (`Commands` 0x038453D4) is a fixed row of the mode
table (type 11) and a command the scripts always register; the space-key switch tests no
membership (§6.5), so the fellowship mode can be chosen at any time, in or out of a
fellowship. (Which modes the channel button's menu lists is Q3.)

**Sending** (CW52; order corrected in the revision 4 fixes), in this order: no text after the
command word → the usage; not in a fellowship → *Fellowship*(0x06358717) with mask 0x1 (type
1 `Error`), nothing sent (variant *Fellowship*(0x0871B170) under the §6.5 variant test);
fellowship chat unavailable → *Weenie*(0x064C4F95), mask 0x1; the length check (§15.2);
otherwise the line goes to the subsystem. **After the send:** a result of zero → the local
echo *Chat*(0x04A3E744; 0x5277 = text) with mask 0x400 (variant *Chat*(0x0E74EA10)); any
other result → *Chat*(0x015B0782) with mask 0x1 and no echo, except one result code that goes
to the generic error routine when a script test holds (otherwise it counts as zero).

**Received** (CW51): *Chat*(0x05D20D84; 0x4DD4 = sender's instance ID, 0x8495FF2 = sender's
name, 0x5277 = text), mask 0x400 (variant *Chat*(0x00D8DF00)). Mask 0x400 has the feedback
sound 0x2A001003 (CW19a). Whether the speaker also receives its own line from the server, in
addition to the local echo, is Q22.

<a id="s15-8"></a>
### 15.8 The ignore list and incoming chat (BIN2, OBS-S: CW56, CW57)

The client does not filter incoming chat by the ignore list in any path that was read: the
logs' receive slots for tells and for general lines, the instant-message session routine and
the scripts' channel-line dispatcher append without an ignore test (CW56). So an ignored
player's lines are kept from the player before they reach the client (Probable: by the
server), for every chat type the server filters (which types: Q21), and the player sees **no
notice**. The ignored player may get *Chat*(0x0B891524; 0x5506DA2 = name) on a tell, a line no
client code or script posts (Probable: composed by the server). The ignore list's own lines
(added, removed, list) are `social-panels.md` §3.7. `GOP_AcceptFriendsAndGuildMemberTellsOnly`
(default false) has no client-side effect that was found (Q27).

<a id="s15-9"></a>
### 15.9 Friend online and offline notices (OBS-S: CW55)

When the subsystem reports a new status for a friend in the friends list, the script compares
the friend's online bit (bit 0 of the status) before and after: clear → set posts
*Chat*(0x0A29AFC5; 0x52615 = the friend's name from the list), set → clear posts
*Chat*(0x010538E5; 0x52615 = name), both with mask 0x8 (type 4 `Standard`); no change posts
nothing. Status reports for names not in the list post nothing.

<a id="s15-10"></a>
### 15.10 Tessera rules for P4 (D24, D20)

These are Tessera's rules, not facts about the original. **Name-equal** means equal after
Unicode default lower-casing (§10 item 12). **P** is the player's character name, as the
session entered the world with it. A session line's **speaker** *S* is its `sender`, or for a
`Tell` line without a sender its `peer`. All built-ins below run in the dispatcher (§10 items
11–13): in frame-loop step 1, their lines appended in step 7, with history as item 13.

**CT1 Session lines.** After CT8, a `ChatMessage` becomes a line as follows (timestamps by
§5.4; templates by CT12):

| Channel | Condition | Line | Mask |
|---|---|---|---|
| `Tell { peer }` | own: *S* name-equal P and `peer` not name-equal P | *Chat*(0x0AD74A23; 0x5866BA4 = `peer`, 0x5277 = text) | 0x20 |
| `Tell { peer }` | otherwise (incoming) | *Chat*(0x0BCA09CC; 0x52615 = *S*, 0x5277 = text); *S* becomes the reply target (CT3) | 0x20 |
| `Fellowship` | *S* name-equal P | *Chat*(0x04A3E744; 0x5277 = text) | 0x400 |
| `Fellowship` | otherwise | *Chat*(0x05D20D84; 0x8495FF2 = *S*, 0x5277 = text) | 0x400 |
| `Named { name }` | *name* in slot *k* (CT5), *S* name-equal P | *Chat*(0x009D15CC; 0xAD99BC5 = the slot's name, 0x5277 = text) | slot *k*'s |
| `Named { name }` | in slot *k*, otherwise | *Chat*(0x063C924C; 0x7C632DC = the slot's name, 0x8495FF2 = *S*, 0x5277 = text) | slot *k*'s |
| `Named { name }` | in no slot | dropped (debug log, rate-limited); not given to plugins | — |
| `Fellowship`, `Named` | no sender | the text as a type 8 line (§10 item 2) | 0x80 |
| `Say`, `Yell`, `Emote`, `System` | — | unchanged (§10 item 2) | — |

The original posts the fellowship and user-channel echo forms locally when the subsystem accepts a line
(§15.6, §15.7) and builds the own-tell line from the subsystem's report (§15.3); Tessera keeps
no local echo (§10 item 2) and shows the same forms for the session's echo, so the result is
the same when the session echoes a line once. Known difference: a tell to oneself on the test
server arrives twice with *S* = `peer` = P and is shown twice as incoming.

**CT2 Built-ins and registration.** In addition to §10 item 3 the dispatcher registers, owner
`Client`: the Tessera names `reply`, `retell`, `fellowship`, `join`, `part`, `channels`,
`chsay` and `ch1` … `ch8`; and every name of the install's `Commands` entries 0x03A7284C
(→ `join`), 0x0DCD958C (→ `part`), 0x0A55AF03 (→ `channels`) and 0x098B3A44 (→ `chsay`). The
fixed mode-table rows already registered by §10 item 3 get these handlers: 0x0DA4AC6C →
`tell` (CT4), 0x0A48C3E9 → `reply`, 0x048CE78C → `retell`, 0x038453D4 → `fellowship` (CT7),
0x08B3A4E1 … 0x08B3A4E8 → `ch1` … `ch8` (CT5); the other fixed rows keep D20 item 2's
"not available" refusal. **Line types.** Every line of §15.2–§15.9 that Tessera posts uses
the original's template (CT12) and the original's **mask** as stated there, overriding §10 item
2 for these lines; usage lines are Tessera's own usage text (D20 item 5) with mask 0x8; a
session refusal (`NetError::InvalidCommand`) stays a type 1 line in Tessera's words (§10 item
12). The window's mode words stay those of §10 item 11; plain text in the mode `Tell`,
`Fellowship` or `UserChat`*k* runs that row's command.

**CT3 Targets.**
- The **reply target** R is *S* of the most recent incoming `Tell` line that passed CT8; none
  at world entry; cleared on `Disconnected`.
- The **retell target** T is the recipient of the most recent `tell` (CT4) handed to the
  session; `reply` and `retell` do not change it.
- **Recent names.** Tessera keeps the last 10 distinct incoming tell speakers and the last 10
  distinct `tell` recipients, newest first (a name-equal repeat moves to the front), each with
  a browse index reset to −1 whenever its list changes. Shift + line up / down (§6.4) moves
  the sender index up / down by one within the list (clamped), Ctrl + line up / down the
  recipient index, and the chosen name becomes the entry's `Tell` target (nothing happens with
  an empty list). This replaces "Shift and Ctrl … do nothing" of §10 item 5 (Q11, Q26 open).
- **`ChatModeReply`**: with R, the entry opens in the `Tell` mode with target R; without,
  *Chat*(0x00CC89CC) with mask 0x21 (§10 item 8).
- **Mode words** (§6.5 switch, CW44; corrected in the revision 4 fixes, review B1): a reply
  word gives `Tell` with target R (empty if none), a retell word `Tell` with T (empty if
  none); a plain tell word (a name of 0x0DA4AC6C) switches **nothing**: the mode, the
  target and the entry text stay, the typed space stays in the entry, and the line the
  player completes is submitted as a command line (§7.2), so it runs `tell` (CT4); a word of
  any other row sets that row's mode and empties the target.

**CT4 Sending tells.**
- `tell` (and the names of 0x0DA4AC6C): as §15.2 steps 1–3 (usage below two tokens; text = the
  remaining tokens joined with one U+0020; this replaces the `tell` argument rule of §10 item
  12). A recipient that fails `is_valid_name` is refused with a type 1 line. Then
  `Chat { Tell { peer: recipient }, text }`, and T ← recipient. The length bounds are §10 item
  12's (1,024 bytes); the original's 385-unit check is not applied (the entry holds 384
  characters, CW20; longer text can only come from aliases or plugins: a known difference).
- `reply` / `retell`: usage without arguments; without R (T): *Chat*(0x00CC89CC)
  (*Chat*(0x0811EFD4)) with mask 0x21; else `Chat { Tell { peer: R (T) }, text: the §7.4
  arguments }`.
- The `Tell` mode with a target G submits `C␠G␠text`; with an empty target (from a reply or
  retell word without a name, from showing the generic instant-message row, CT6, or from a
  tab's stored `Tell` mode taken back after another word emptied the target) `C␠␠text` (§7.2 step 5), which the `tell` handler reads as above. A plain tell word
  never enters the `Tell` mode (CT3).

**CT5 User channels.** Tessera keeps 8 slots (§15.6 table), each free or holding a channel
name as `ChannelJoined` spelt it; names in slots are never name-equal.
- `join` (1 or 2 tokens, else usage; lines mask 0x8): a backend without user chat (CT11) →
  *Chat*(0x04068445); a name longer than 50 UTF-16 code units →
  *Chat*(0x02832AE7; 0x7C632DC = name, 0x50A2C88 = its length in code units, 0xD0A7DE8 = 50);
  a name of at most 50 code units but longer than 64 bytes of UTF-8 (the protocol `NAME`
  bound) → the same template with 0x50A2C88 = its length in UTF-8 bytes and 0xD0A7DE8 = 64,
  so the line never shows a length within its limit (revision 4 fixes; the original has no
  byte bound);
  a name that fails `is_valid_name` otherwise → *Chat*(0x0AC8421C); a name name-equal to a
  registered channel-command name (the names of the 26 fixed rows and the Tessera names `say`,
  `yell`, `emote`, `tell`, `reply`, `retell`, `fellowship`, `ch1`–`ch8`) →
  *Chat*(0x01919744; 0x7C632DC = name); a second token (a password; `JoinChannel` has none) →
  *Chat*(0x08DC4764); already in a slot → *Chat*(0x07548D0C; 0x2E8E425 = name); 8 slots used →
  *Chat*(0x002AAA94; 0x539D = 8); else `JoinChannel { name }`. No rate guard; nothing is shown
  until `ChannelJoined`.
- `ChannelJoined { channel }`: name-equal to a slot's name → nothing (debug log); else the
  lowest free slot *k* takes it and *Chat*(0x09A99A08; 0x4E28A8 = *k*) is posted with mask 0x8;
  no free slot → dropped with a warning.
- `part` (exactly 1 token, else usage; mask 0x8): in a slot → `LeaveChannel { the slot's
  name }` (the slot stays until `ChannelLeft`); not in a slot and name-equal to a
  channel-command name → *Chat*(0x03C043D4; 0x7C632DC = name); otherwise *Chat*(0x0DF4808C;
  0x2E8E425 = name).
- `ChannelLeft { channel }`: in slot *k* → the slot is freed and *Chat*(0x03B095F8; 0x4E28A8 =
  *k*) is posted with mask 0x8; otherwise nothing (debug log).
- `channels`: the §15.6 list, argument 0x12E6A6F always *Chat*(0x02E819D4) (no passwords).
- `ch`*k* and the slot rows (text = the §7.4 arguments): an empty text → usage; a backend
  without user chat → *Chat*(0x04068445) with mask 0x1; slot *k* free → *Chat*(0x0E2A7CE8;
  0x539D = *k*) with mask 0x8; else `Chat { Named { slot k's name }, text }`.
- `chsay`: no token → usage; the first token is the name; not in a slot → *Chat*(0x02E32555;
  0x2E8E425 = name), mask 0x8; the text is the remaining tokens joined with one U+0020; an
  empty text → usage; else as `ch`*k* for that slot.
- `Disconnected` and every new world entry free all slots without lines.

**CT6 The instant-message tab.** Created by §4.6 when `GOP_FullIRCMode` is true (default from
`GameplayOptionDefaults`; Tessera's settings may override it, as they may
`GOP_AutoCreateIMSessions`). Its sessions follow §15.4 with these changes: sessions are keyed
by the partner's name (name-equal; the session layer has no instance IDs), so step 1 looks up
by name only; the partner of a CT1 `Tell` line is `peer`; the speaker of step 5 is *S* for an
incoming line and P for an own line; 0x58E15 is the time of day as §5.4's Tessera rule
formats it (`HH:MM:SS`, 24-hour), from the same clock as the §5.4 timestamps: local time when
the platform gives Tessera the local offset, else UTC (a known difference, answered callout
below); the line is appended only to the session's log (the normal tabs got CT1's line); a row
that gets a line while not shown shows its unread marker until it is shown (Q25). **Renames**
(revision 4 fixes, review B3): a row found by name whose stored spelling differs from the
line's partner name (necessarily by case only, since the lookup is name-equal) takes the
line's spelling, as §15.4 step 1, and nothing goes to the `Tell` mode; Tessera then moves the
row to its place in name order (a **Tessera rule** that keeps CW-I26; the original leaves the
row where it is). The rename beyond case of §15.4 step 4 never arises, because Tessera has no
instance IDs. The generic row never takes a partner's name: with auto-creation off it takes
the line and keeps its label (known difference: the original relabels it, Probable, §15.4
step 4). The row's name is shown on its `IMSessionText` element (§15.4 answered callout (a)).
Clicking a row shows it and sets the entry's mode to `Tell` with the row's name as target (an
empty target for the generic row). "Start session" validates the answer with `is_valid_name`
(*Chat*(0x0EA68129) for an empty answer, else Tessera's words, mask 0x1) and creates and shows
the row; "close session" removes a row and its log (not the generic row) and shows the generic
row if it was shown. Tessera posts no failure lines (the session layer reports no failure
kinds: the sandbox's notice of SF §7.6 and the test server's refusals arrive as `System`
lines, shown by §10 item 2).

**CT7 The fellowship channel.** The mode is always offered (§15.7). `fellowship` (and the
fixed row): an empty text → usage; the current fellowship (SP §9 TR2: the member list of the
last `FellowshipChanged`, empty at world entry) empty → *Fellowship*(0x06358717) with mask
0x1, nothing sent; else `Chat { Fellowship, text }`. The "group" variants are never used (no
monster play).

**CT8 Ignore filter.** The ignore list is the names of the last `IgnoreList` (empty at world
entry, cleared on `Disconnected`). A `ChatMessage` on `Say`, `Yell`, `Emote`, `Tell`,
`Fellowship` or `Named` whose *S* is name-equal to an ignored name, and that is not the
player's own (*S* not name-equal P), is dropped before every log, the instant-message tab,
sounds, plugin events, the reply target and the recent names; nothing is shown. `System`
lines are never dropped, and tells **to** an ignored name are not blocked. Tessera filters on
the client because its sessions do not (D24: the test server runs no gameplay rules, the feed
is scripted); the visible result is the original's (§15.8). Tessera never posts
*Chat*(0x0B891524). `GOP_AcceptFriendsAndGuildMemberTellsOnly` is not applied (Q27).

**CT9 Friend notices.** Tessera keeps the entries of the last `FriendList` (reset on
`Disconnected`). The first `FriendList` after a world entry posts nothing. For every later
one, each entry of the new list, in list order, that is name-equal to an entry of the previous
list with a different `online` posts *Chat*(0x0A29AFC5; 0x52615 = the name as the new list
spells it) when it became online and *Chat*(0x010538E5; 0x52615 = name) when it went offline,
mask 0x8. Added and removed names post no online or offline notice.

**CT10 Plugins.** Each line that CT1–CT9 append to the logs is delivered once to
`Turbine.Chat.Received` by §10 item 9, with the rendered text and the chat type of its mask by
the Q18 rule (6 for tells and 0x21 lines, 11, 28–31 and 39–42, 4, 1); the instant-message
tab's copy is not delivered again; dropped lines are not delivered.

**CT11 Backends.**
- Scenario feed: the `say … channel=tell|fellowship|named:NAME`, `system`, `channel join|leave`,
  `fellowship`, `friends` and `ignore` steps (SF §6.4) feed CT1, CT5, CT7–CT9; the generic chat
  answer (SF §7.6) is the session echo of CT1.
- Plain sandbox without a scenario: SF §9.5 accepts and ignores `JoinChannel` and
  `LeaveChannel`, so no slot is ever filled; a `Named` or `Fellowship` send is echoed (SF §7.6)
  and shown by CT1 only for a name in a slot.
- Test server (`--server`) and the overlay backend (corrected in the revision 4 fixes, review
  B2; decision 0035, TP §10.6, SF §9.8): `Tell` works (TP §10) at every minor. A test-server
  session **offers user chat** exactly when the negotiated protocol minor is 2 or higher and
  the server's `HelloAck` set `CHAT_CHANNELS`; an overlay run sends its channel commands to
  the test-server backend under the same rule (SF §9.8). With user chat, `join`, `part`, the
  slot rows, `ch`*k* and `chsay` run CT5 and send `JoinChannel`, `LeaveChannel` and
  `Chat { Named }`; the slots follow the server's `ChannelJoined` and `ChannelLeft` with the
  server's spelling; the server's refusals (`ChannelRefused`, `ChatRejected`) arrive as
  `System` lines in Tessera's words (SF §9.8) and are shown by §10 item 2, where the original
  would post its own lines (§15.6: the generic error routine with the subsystem's result
  code after a failed send; corrected in the C1 fix). The server sends each channel under one
  spelling (TP §10.6), and the slots hold only spellings the server sent, so CT5's name-equal
  matching against the slots finds the same slot as the byte comparison TP §10.6 asks for.
  The server's channel key is the lowercase of the **NFC** name (`test-server.md` §9.1), while
  name-equal (§15.10) does not normalise: a `join` typed in another normalisation form of a
  held name passes CT5's local duplicate check, is sent, and comes back as
  `ChannelRefused(ALREADY_JOINED)`, shown as a `System` line (§10 item 2); a `chsay` or
  `part` typed that way finds no held name and gets CT5's not-in-a-slot line
  (*Chat*(0x02E32555) or *Chat*(0x0DF4808C)). Without user chat (minor 0 or 1, or no
  `CHAT_CHANNELS`): `join` and the
  user-channel sends post *Chat*(0x04068445) (mask 0x8 for `join`, 0x1 for sends) and send
  nothing. At every minor the fellowship refusal of CT7 applies, since no `FellowshipChanged`
  arrives (SF §9.5, §9.8). `channels` works on the slots in both cases.

> **Answered (revision 4 fixes, 2026-10-11; was OPEN, implementation, P4 client,
> 2026-10-11):** (a) Yes: CT11's test-server row is revised to that rule (user chat at
> negotiated minor 2 or higher with `CHAT_CHANNELS`, else CT11's lines); §11.11 step 9 too.
> (b) Accepted as a known difference: 0x58E15 and the §5.4 timestamps use one clock, local
> time where the platform gives the offset and UTC otherwise; both places must agree. (c)
> Confirmed: a line CT1 drops is shown nowhere, and the lines of CT5, CT9 and
> `social-panels.md` TR4 are chat lines only; the original posts each of them as a chat line
> with the masks of §15 and no HUD notice (§15.6, §15.9).

**CT12 Templates.** Lines are rendered from the install's string tables at run time (ST §7)
with the argument IDs given. The argument 0x4DD4 (an instance ID) cannot be filled from the
session layer, so the `Select` link markup (ST §4.8) around the name is removed and its label,
the name, kept. Values from the session (names, channel names, text) are inserted as literal
text: neither the message grammar nor markup inside them is interpreted (TP §10.5; ST §4.6
escapes and the `<<>` literal of ST §4.8). The trailing line break of a template is removed by
the line element (§5.1). When a template is missing (no install, another language without the
token), Tessera's own wording with the same mask is used.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-11 | Revision 4, fix after the independent re-review of the revision 4 fixes (status Revised, awaiting an independent re-review; spec writer, dirty room; no section number or anchor changed). **C1 (corrected fact about the original):** both user-channel send handlers (by slot and by name) hand the send to one shared body; after the send a result of zero posts the echo *Chat*(0x009D15CC) with the slot's mask, the one result code guarded by CW52's script test goes to the generic error routine when the test holds (else counts as zero), and every other non-zero result goes to the generic error routine with that code; the body never posts *Chat*(0x015B0782), which belongs to the fellowship send and to a send on a channel chosen by a chat-type mask that neither user-channel handler reaches (the fixes pass had decoded that body as the user-channel one). Changed: CW54 (now also lists the body's availability and character-age checks), §15.6 "After the send", the CT11 parenthesis, Q24 (which line the error routine posts for a channel-send code), the §1 OBS-S row (C1 fix pass). **Note (a):** CT11's reason reworded: slots hold only the server's spellings; the server keys channels by the lowercase of the NFC name (`test-server.md` §9.1) while name-equal does not normalise, so a `join` typed in another normalisation form is refused by the server as `ALREADY_JOINED` (a `System` line), and a `chsay` or `part` typed that way gets CT5's not-in-a-slot line. **Changes behaviour implemented on branch `p4`:** none (no code on `p4` posts *Chat*(0x015B0782); note (a) rewords a reason and states a consequence of rules already in CT5 and CT11, it adds no rule). The behaviour changes listed in the previous row (B1, B3, note (d), the session row label) still stand. |
| 2026-10-11 | Revision 4, fixes after the independent review (status stays Revised, awaiting an independent re-review; spec writer, dirty room; no section number or anchor changed). **B1 (CW44, corrected fact about the original):** in the space-key switch a plain tell word switches nothing (mode, target and entry text stay, the space stays typed); only reply and retell words switch to `Tell`, with the callback's name or an empty target; a word of any other type empties the target (revision 3's step 3 said "unchanged"). Changed: CW44, the §6.5 Narrowed callout (a), Q12, the §15.2 paragraph "The `Tell` mode", CT3 "Mode words", the last CT4 bullet, §11.9 step 4; new CW-I31. **B2 (Tessera rule):** CT11 states user chat on the test server by negotiated minor 2 or higher and `CHAT_CHANNELS` (decision 0035, `tessera-protocol.md` §10.6, `scenario-feed.md` §9.8), overlay runs alike; §11.11 step 9 and the §14 `scenario-feed.md` row follow; the §15.10 implementer OPEN is Answered ((a) revised, (b) one clock, local time where the offset is known, else UTC, a known difference, (c) confirmed). **B3 (corrected fact about the original):** §15.4 steps 1, 2 and 4: a session found by name takes the line's spelling in place with no hand-over; only a stored name that differs beyond case is renamed, in place, and then the shown session's name goes to the `Tell` mode; rows are never moved; the generic row is relabelled when it takes a line with auto-creation off (Probable). CW48 reworded; CW-I26 marked a Tessera rule; CT6 states the Tessera rename rule (row moved to name order, no hand-over, the generic row keeps its label) and the label element; new CW-I32. The §15.4 implementer OPEN is Answered ((a) with the labels of new row CW58, (b) as above). **Non-blocking notes:** (a) §15.7 sending order (usage, membership, availability, length, send) and the results after a send (*Chat*(0x015B0782) with mask 0x1, no echo; the error-routine codes), also in §15.6, CW52, CW54; (b) §15.2 step 4: the tell handler measures its argument text, not the re-joined text; (c) CW50 and §15.5: the third failure kind closes the session found by name, and without one is shown like the other failures; (d) CT5: a name over 64 UTF-8 bytes but within 50 code units gets the length template with its byte length and the limit 64; (e) the §15.4 and §15.10 implementer OPENs answered; (f) the scope sentence for decision 0011 is left to the owner (this spec does not edit decision records). **New sources:** OBS4 (layout labels), the revision 4 fixes passes of OBS-S and BIN2 (§1). **Changes behaviour implemented on branch `p4`:** B1 (the mode switch's plain tell word, and the target of non-`Tell` words), B3 (no hand-over on a case-only rename), note (d) (the byte-bound line of `join`) and the session row label element (§15.4 answered callout (a)). **No behaviour change:** B2 (the client already follows it), notes (a)–(c) (facts about the original that Tessera's rules do not use), and the answers (b) and (c) of the §15.10 OPEN (they confirm what is built). |
| 2026-10-11 | Revision 4 (status Accepted → Revised, awaiting review; spec writer, dirty room; P4 social chat, decision 0024). **New section §15** (anchors s15, s15-1 … s15-10): sending tells, the `Tell` mode, reply and retell (§15.2); tell lines in the normal tabs (§15.3); the instant-message tab and its sessions, answering Q5 in part (§15.4); tell refusals and failures (§15.5); user chat channels: commands, 8 slots and their chat types, join, leave and list checks and lines, limits 8, 50, 20, sending and the local echo (§15.6); the fellowship channel (§15.7); the ignore list and incoming chat (§15.8); friend notices (§15.9); Tessera rules CT1–CT12 on the decision-0020 dispatcher and the scenario-feed events and commands (§15.10). **New sources:** OBS3, OBS-S (the script library read as data with the repository's decoder) and BIN2 (M4, same build) in §1; rows CW43–CW57. **Correction:** the §6.5 mode table had the reply and retell parentheticals against the wrong tokens; 0x0A48C3E9 is the reply entry (callback 0x01D60032), 0x048CE78C the retell entry (callback 0x01D60030) (CW43). **Narrowed:** Q5 (§4.6), Q11 (§6.4), Q12 (§6.5: a plain `Tell` word empties the target; the variant test also picks the "group" lines), Q15 (§8.1: one script body registers 79 entries). **Pointers** added to §10 items 1, 2, 3 and 8 (no rule of them deleted; CT1–CT4 and CT6 override them for the P4 lines as stated). **Added:** §11.9–§11.14, CW-I17–CW-I30, Q20–Q28, six §14 rows (`social-panels.md` TR5 superseded for online and offline notices). No other fact of revision 3 changed; section numbers and anchors kept. |
| 2026-10-10 | Note only (no fact changed), for P3: §4.7 note on the quest journal's chat lines (type 21 `Quest`, `quest-journal.md` §7). Status unchanged. |
| 2026-10-06 | Revision 2, corrections after review (status stays Revised, awaiting review; no fact about the original changed; section numbers kept). **§10 item 9:** queued `Received` delivery now skips the apartments named by the skip rule of `lua-plugin-api.md` §11.3 (so a `WriteLine` in a `Received` handler never reaches its own apartment and an echoing plugin cannot feed itself); LPA §11.3 is the single statement of that rule and governs; the earlier "this spec does not settle it" is withdrawn; `WriteLine` lines named among the queued lines. **§6.2:** a declined command going on to game code cites IB D7 instead of IB Options 4. **§14:** the LPA §11.3 timing row mentions the skip rule. |
| 2026-10-09 | Revision 3 (status Accepted → Revised, awaiting review; spec writer, dirty room). Answers the implementer OPENs of 2026-10-08 (client command dispatcher, items (a)–(g)) and 2026-10-09 (chat window on the dispatcher, items (a)–(e)) in §10 and the two implementation OPENs of §6.5 (the `tessera-ui` command-word rule, confirmed as a Tessera rule while Q12 is open, and the 2026-10-08 mode-table note); the four callouts are marked Answered. **New data fact:** CW42 (OBS2): the `CommandSetup` and `Commands` entries with token 0x04DA4989 hold the same single name in this install. **Revised Tessera rules:** §10 item 2 (plain text goes through the dispatcher to the session, whose echo is the line shown; no local echo; refusal without a session; line types of Tessera's own lines), §10 item 3 (only the 26 fixed mode-table rows; the default word registered as a `Say` word; the install's names registered after the tables are read and before any plugin loads; no plugin-host command table). **New Tessera rules:** §10 item 11 (submit order and its condition, timing in the frame loop, quickslot aliases, where replies go, one shared read of the mode table), item 12 (case folding, code-unit positions and why whole-character removal is equivalent, bounds, `tell` arguments, wording), item 13 (history sources). New provenance tags OBS2 and D20. No fact about the original changed; section numbers kept. |
| 2026-10-06 | Revision 2 (status Accepted → Revised, awaiting review; applies the accepted decision records [0014](../decisions/0014-ui-input-bridge.md) (**IB**) and [0015](../decisions/0015-ui-rendering.md) (**UR**); no new research; no fact about the original changed; section numbers kept). **Reworded Tessera rule:** §10 item 5, the mapping of the entry's Tessera actions: `open_chat` = `AcceptInput`, `menu` = `EscapeKey`, `text_line_up/down` = `CursorPreviousLine/NextLine` replace the retired `text_submit`, `text_cancel` and `text_history_prev/next` (IB D2). **Cross-references added:** §6.2 (the commands come from `text_entry` with focus and from Tessera's `ui.chat_log` without it; a declined command goes on to game code unchanged), §6.3 (history reads the line actions), §6.4 (Subset modifier policy lets Shift or Ctrl reach the window), §9 and §10 item 2 (plugin delivery time). **New Tessera items:** §10 item 9 (`Turbine.Chat.Received` is queued and delivered in frame-loop step 7, IB D8, Known difference 21; the interaction with LPA §9.6 is left to IB open review item 1) and §10 item 10 (paging without entry focus through `ui.chat_log`, IB D5, Known difference 17). **New OPEN:** Q19 (which input map supplies the paging commands without entry focus), §6.2 and §13. **§1:** provenance tag IB, UR; header companion entry. **§14:** the `keymap.md` §8 row now records the change; new row for `lua-plugin-api.md` §11.3. UR requires no change here (the chat log's repaint is its content event). |
| 2026-10-06 | Revision 1 after review (status stays Review, submitted for re-review). **B1:** layout 0x22000034 has 19 top-level elements (§1 OBS, CW1, §3.1). **B2:** marks redefined (Confirmed = data only); rows CW2, CW4, CW7, CW8, CW12, CW15, CW16, CW19, CW24, CW25, CW27, CW32, CW35 split into *a* (Confirmed) and *b* (BIN); CW22 reworded (reset on addition, not on every submission); headings of §3.1, §3.3, §3.5, §4.1, §4.3, §4.6, §5.2, §5.4, §5.6, §6.5, §6.6, §7.2, §7.4, §7.6, §8.1 name the split rows. **B3:** new BIN pass (same build): per-log any-bit test with the delivered mask including bit 24, at most once per tab, a suppression flag and a refill routine (§4.3, CW9 now BIN, Q4 narrowed); style of a multi-bit mask by FO §9.3 step 1 (§5.1 step 2); whole-mask exact sound match (§5.6, CW19b); the reply and unknown-command lines leave through one game-script hand-over (§6.2, §7.6, CW41); §2 term; §10 items 1 and 8; §11.8; CW-I15, CW-I16; new OPENs Q17, Q18; notes for LPA §11.3 and FO §9.3 in §14. **B4:** CW-I14 and CW-I9 restricted to submissions that reach the history; §6.2 step 1, §6.3 and §7.2 step 1 say that an empty submit or a result-2 run leaves the browsing index unchanged; §11.4 example. **Non-blocking:** IL and UW in the header and §3.4; Q2 and §13 cite UL §9.6/§9.6.1; §3.2 cites UL §9.2; §5.5 cites UB §4.7 (wheel) and UW §5, §6, §8; `CursorNextPage` and `CursorPreviousPage` go through the list box's clamped scroll-to (UW §5.3; `keep_in_bounds` default true, not set in the layout), so a negative target becomes 0 (CW18). |
| 2026-10-06 | First version (status Review). Chat windows, pool and defaults (§3), tabs and filters (§4), log (§5), entry line, history, channel modes and key-repeat guard (§6), command-line pipeline, tokenizer, arguments, substitutions and results (§7), built-in names and `IsCommand` (§8), plugin view (§9), Tessera decisions (§10), examples (§11), invariants (§12), OPEN list (§13), notes for other specs (§14). OBS on the owner's install and BIN on client 4906.0071.1794.4035, 2026-10-06. Answers M5 critique item A8. |
