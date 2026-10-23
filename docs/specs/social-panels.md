<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Social panels: friends and ignore lists, fellowship display, fellowship invitation, trade window

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review 2026-10-23 by spec-review:spec-gameplay:2 (independent session), a session that wrote no part of the 2026-10-23 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, revision 3: TR16 interim retired and TR16a–TR16d added, confirming the member effect lists as built (implementer choices (8)–(11) of 2026-10-14); no implemented behaviour changes; awaiting independent review). Before: **Accepted** (2026-10-14: review 3, an independent review of revision 2, passed; see the review record). Revision 2 (2026-10-14): review 2 notes (a)–(c) applied, the §9.1 implementer items (1)–(5) turned into Tessera decisions (e)–(i), items (6)–(7) answered in [`lua-plugin-api.md`](lua-plugin-api.md) §15.4; see the change log for the one related behaviour change, which is in that spec. Before: **Accepted** (2026-10-11: review 2, an independent re-review of revision 1, passed; see the review record). Before that: Revised (revision 1, 2026-10-11, answering review 1 items B1–B7 and notes (a)–(d)); review 1 (2026-10-11): changes requested. Code was built from the first version on branch `p4`; the change log's revision-1 row lists the seven answers that change that behaviour, which the implementation must now follow |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Spec reviewer, independent dirty-room sessions that wrote no part of this spec or of its P4 notes (review 1, 2026-10-11; review 2, 2026-10-11, a different session that did not do review 1; review 3, 2026-10-14, a session that wrote no part of revision 2 or of reviews 1–2) |
| **Review record** | **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-gameplay:2 (independent session)): **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; TR16a–TR16d checked against `combat-ui.md` §8 and TR30a (the second icon row ends 3 pixels past the 37-pixel box with *e* = 20). Earlier: **Review 3, 2026-10-14** (revision 2, commit 7c545cd on branch `specfix`; this spec except the TR16 text, with the same commit's `lua-plugin-api.md` §15.4/§19 and `ui-widgets.md` §6.4 parts, recorded in those headers; independent session that wrote no part of the spec, its revisions or its reviews): **passed, Accepted.** Checklist (CLEAN_ROOM.md §5.3): every new statement (SP30 count, SP34–SP36, OBS-R2, (q11), §4.2 note and table rows, §6.2 sentence, TR14, TR15, §9.1 (e)–(i), SPI3, SPI14, SPI15, the change-log row) is a layout, value, order or behaviour, or a marked Tessera decision; no code or pseudo-code (no addresses, no listing-shaped steps); no copied prose; no game text (element, state and property names are data identifiers); no keys, hostnames or traffic; BIN within decision 0011 (offline client UI only; build and SHA-256 prefix, tool, question and why M1–M3 were not enough are stated); each new fact has a row; the implementer items (1)–(5) are Decisions with reasons and none changes implemented behaviour, items (6)–(7) are answered in `lua-plugin-api.md` §15.4; the TR16 text is unchanged. The follow-up list is answered: SP30 32 states, the micro template's missing `VitalHealthFieldBg` (§4.2, SPI3), TR15 state `normal`. **Spot checks** (read-only, 2026-10-14, own probes in a private dirty scratch area): layout records 0x22000024, 0x22000033, 0x2200004E, 0x22000051, 0x2200005C, 0x22000061, 0x2200006C, 0x22000604 read again from the install, byte-identical to the 2026-10-06 dump; resolved by UL §7: SP34 (both row templates 48 instances; the sets of element IDs differ only by the roots, `VitalHealthFieldBg` in the mini template and `FellowTemplateAssistant` in the micro template; `VitalHealthFieldBg` a Field 200 × 80 with 112 states, default state `normal`, which carries media), SP35 layout (`FellowTemplateVolumeControl` a Scrollbar hidden in its own block and in its default state `normal`, `hide_when_disabled` true, `disabled` absent), SP30 and SP36 (10 item-display templates whose `ItemDisplay_Name` has 32 states, 31 `…_ColorName` and `Floaty_CriticalDamage`, the five quality states and `Object_ColorName` among them; own block and every state a one-entry `UICore_Text_font_colors` and no `UICore_Text_font_color`; no default state; `ItemDisplay_MediumTemplate` root without states and with flag bit 0 clear, the name a direct child), §9.1 (h) (`MoneyDisplay_CoinTemplate` 53 × 22 with a Text and an icon holder); (e) agrees with `string-table.md` §4.8.3 step 3. BIN (a private read-only copy of the analysis project, Ghidra headless, deleted after use; build SHA-256 prefix d22363eb9aff0fb5 checked): SP35 (the row set-up fetches the volume control, sets its scroll position under a member condition, and later writes its visibility from the row's voice flag, which is cleared when the member record's voice flag is not set; the bar's position-property handler lays the bar out). All agree. **Non-blocking note** for the next revision (no behaviour change): in SP35, the §4.2 table row and SPI15, "position" is the bar's value `UICore_Scrollbar_position` (the property whose change lays the bar out, `ui-widgets.md` §6.5), not its placement in the row; name the property. **Implemented behaviour:** no change from this spec; the related change is `lua-plugin-api.md` §15.4 further rule 7. Earlier: **Review 2, 2026-10-11** (re-review of revision 1, commit 6406b82 on branch `p4`, diff `45b7fb9..6406b82` for this spec, the `game-panels.md` §14 note and the `ui-layout.md` §17.4 note; independent session that wrote no part of the spec, its notes or revision 1 and did not do review 1): **passed, Accepted.** Checklist (CLEAN_ROOM.md §5.3): every new statement (§4.2 visibility table, §4.3 P4 voice elements, §6.1 coin entries and money displays, §6.2 entry parts, §6.3, TR3, TR9, TR11, TR13–TR16, §9.1, SPI9–SPI13) is a layout, value, mapping or behaviour, or a marked Tessera rule; no code or pseudo-code (no addresses, no listing-shaped steps); no copied prose; no game text (strings by table and token; scenario names in §12 are invented); no keys, hostnames or traffic; BIN under decision 0011 (offline client UI only); every new fact has a row (SP25–SP33, OBS-R1, (q8)–(q10)); the remaining unknowns are OPEN with Tessera rules (Q1–Q11, Q13, Q14). All pass. B1–B7 and notes (a)–(d) of review 1 are answered as the change log says; the exit-verification gap (trade entries without names, literal-entry icons, wallet and peer coin; the red and green icon) is answered by the data: entry parts by ID (§6.2), the literal entry's "no icon" defined (TR9), the two money displays as coin-element lists (§6.1, in agreement with `npc-windows.md` §9.2–§9.3 as Accepted on `main`), and the icon is `FellowTemplateVoiceChatSquelched` / `PlayerVoiceChatSquelched` (image 0x410202F4, the red and green one; the green 0x41007DD0 and red 0x41007DD1 are the voice and sending-data icons), hidden by TR14. **Spot checks** (read-only, 2026-10-11, own probes): layout records 0x22000024, 0x22000033, 0x2200004E, 0x22000051, 0x2200005C, 0x22000061, 0x2200006C, 0x22000604 re-extracted from the install, byte-identical to the 2026-10-06 dump; resolved by UL §7: SP25 (every row child, rectangle and image of §4.2; none of the seven status/voice elements hidden; pet vitals and volume control hidden; `FellowTemplateAssistant` only in the micro template), SP27 layout (P4 `PlayerVoiceChat`, `…Squelched`, `PlayerSendingData` at (71, 11) 20 × 20 not hidden; `PlayerLeaderIcon`, `PlayerAssistantIcon` hidden), SP29 layout (the `UI_VitalDisplay_Array` meters, the inner `VitalPowerField` of the same ID), SP30 (`ItemDisplay_QuantityOverlay` a hidden Text inside `ItemDisplay_Icon`, before `ItemDisplay_Name` in tree order), SP31 layout (the three editable entries with 5 characters and no IME, labels are Fields, currency tags; both money displays without children, `ForceShowAll`, `SuppressLeadingZeroes` on the peer's; `MoneyDisplay_CoinTemplate` 0x10000114 with `MoneyDisplay_Text` 0x10000115 and icon states `Gold`, `Silver`, `Copper`), SP33 and SPI12 (every declared argument of (UI, 0x070F9F3C), (UI, 0x038B9CDC), (UF, 0x06CDDF0C), (UF, 0x019353F5), (UF, 0x0803A974), (FT, 0x0571E4C5), (FT, 0x04CF6294), (UI, 0x07480C44), (UST, 0x0CFA21C2), (UI, 0x07D4E2D4)); BIN (a private copy of the analysis project, deleted after use): SP26 (row creation: link-dead and defeated from two flags of the member record, the voice button from a third, the squelched marker from voice activity and the button's squelch bit, sending-data, assistant and leader marker hidden at creation), SP27 code (P4 set-up hides sending-data and squelched, watches PID 0x54A), SP28 (the level token is the start-up hash of a name string, 0x070F9F3C, with argument 0x7C83B5 likewise; the row sets it with the member's level), SP29 binding (effect list set player-only for the local player, else bound to the member), SP31 (the trade code names exactly 0x10000127, 0x10000123, 0x1000011F). All agree. **Non-blocking notes** for the next revision (no behaviour change): (a) SP30 says the name text has 33 colour states; the resolved `ItemDisplay_Name` has 32 states (31 `…_ColorName` states and `Floaty_CriticalDamage`), the six named in §6.2 among them; (b) §4.2 says the micro template has "the same child IDs" plus `FellowTemplateAssistant`; it also lacks `VitalHealthFieldBg`, so SPI3 holds for the micro template only without that element (the micro template is not used, TR2); (c) TR15 does not say which state `VitalHealthFieldBg` takes in Tessera (the original sets the member's override state, §4.2); the layout default `normal` is the evident reading, state it. **Implemented behaviour:** this review requests no change; the seven behaviour changes of revision 1 (change log) are still to be built on `p4` (the code still shows the voice icons and fills entries by the first Text). Earlier: **Review 1, 2026-10-11** (text as of 45b7fb9 on branch `p4`, diff `bb43e74..45b7fb9`: this new spec, its notes in `game-panels.md` §14 and `ui-layout.md` §17.4; `lua-plugin-api.md` §15.4 is recorded in that spec's header): **changes requested.** Checklist (CLEAN_ROOM.md §5.3): every statement is a layout, value, mapping, order or behaviour, or a marked Tessera rule; no code or pseudo-code (§3.4, §4.6, §6.3–§6.6 and TR1–TR13 are plain steps, not listing-shaped; no addresses); no copied prose; no game text (every string named by table key and token; the three command sub-words of §7.1 are interoperability names, as in CW §8); labels and property names are data identifiers; no keys, hostnames or traffic; BIN within decision 0011 (offline client UI; network, login, keys, launcher not read); every fact has an SP row; the Probable mark is defined in §1, as in CW and GP. Those items pass. It fails on "unknowns marked OPEN, not guessed" and completeness: eight implementer OPENs are unanswered, the data or BIN answers most of them, and several gaps cause the defects seen at exit verification. **Spot checks** (read-only, 2026-10-11; layout records 0x2200004E, 0x22000024, 0x22000051, 0x2200005C, 0x22000604, 0x22000061, 0x22000033 re-extracted from the install and byte-identical to the 2026-10-06 dump; own probes; a private copy of the analysis project, deleted after use): SP1 (`social` 881 instances; page lists of `Social_Panel` and `FriendsPagePanel`; `Social_Field` rectangle, group 31, origin), SP9 (four panels, positions, edges, groups 15–18), SP15 (`fellowinvite` children and row template), SP17 (`securetrade` tree, currency tags 0x70000218/9/A, trade button boolean, `OtherTrade_Label` without text), SPI5 (all 171 (table, token) pairs exist; table sizes 261, 179, 144, 34, 14, 57, 235, 84 as in §2; every argument ID stated in §3.3, §3.6, §3.7, §4.4, §4.6, §6.4, §6.5 matches its entry), SP11 by BIN (marker state `raid`, else `monsterplaygroup` for alignment 3, else `fellowship`; hidden at row creation): all agree. **Blocking:** **B1 (voice, ready-check and status elements; the red and green icon).** §4.2 calls the voice-chat and ready-check children "out of scope" but must say whether they show. Data: none of `FellowTemplateVoiceChat` (0x1000043C, Button, default image green), `FellowTemplateVoiceChatSquelched` (0x100010CE, red-and-green image), `FellowTemplateSendingData` (0x1000043D, red image), all three at (38, 0) 20 × 20, carries `UICore_Element_hide`; nor do `FellowTemplateLinkDead`, `FellowTemplateDefeated` (default state `normal`, images) or `FellowTemplateLeader`; `FellowReadyCheckTemplate` (0x1000106A) has no default image (states `PendingReadyCheck`, `Ready`, `NotReady`). The same holds on P4's `AvatarPanel`: `PlayerVoiceChat` (0x10000B50), `PlayerVoiceChatSquelched` (0x100010CF), `PlayerSendingData` (0x10000B51), at (71, 11) 20 × 20, not hidden. BIN (row creation and P4 set-up): the voice button is shown only when the member's voice flag is set; the squelched marker only when voice is active and the button's squelch flag is set; sending-data is hidden; `FellowTemplateAssistant` (0x10000447) is hidden; link-dead and defeated each take their visibility from a flag of the member's group record at creation; on P4 sending-data and squelched are hidden at set-up and `PlayerVoiceChat` follows a boolean property of the player (PID 0x54A). State these, with a Tessera rule (no voice chat: all six voice elements hidden; the ready-check element left without state), and add the P4 portrait elements to `game-panels.md` §3 or its §14 note (elements game code hides). Changes implemented behaviour (the icons now show). **B2 (trade window, §6.2 implementer OPEN; GP §11 OPEN).** Data: the medium item display's name is `ItemDisplay_Name` (0x100000F5; Text at (35, 0) 165 × 32, left, truncation (UI, 0x037DDBC3), the name-colour states including `ItemQuality_*_ColorName`); its icon `ItemDisplay_Icon` (0x100000E4, `ItemIconDisplay`) under `ItemDisplay_ShortIcon`; `ItemDisplay_QuantityOverlay` (0x100000E7) is also a Text and precedes the name in the tree, so "the entry's first Text element" picks the quantity overlay (the missing names). Coin entries: `MoneyDisplay_Entry_GoldEntry` 0x10000127, `…SilverEntry` 0x10000123, `…CopperEntry` 0x1000011F (the editable texts); the labels `…GoldLabel` 0x10000126, `…SilverLabel` 0x10000122, `…CopperLabel` 0x1000011E carry the tooltips; the trade code keeps exactly these three entry IDs (BIN). `SelfTrade_TotalMoney` (`PlayerMoneyDisplay`) and `OtherTrade_MoneyDisplay` (`MoneyDisplay`) have no layout children and no text: they are horizontal one-row, three-column lists filled in reverse whose coin element is `MoneyDisplay_CoinTemplate` (0x10000114) of layout `moneydisplay` (0x2200006C), `ForceShowAll` on both, `SuppressLeadingZeroes` on the peer's: the display of `npc-windows.md` §9.2–§9.3 (Accepted on `main`); cite it once `main` is merged, instead of "set the display's own text" (which shows nothing). Define the "placeholder icon" of TR9 and §12.2 (no spec defines it; `npc-windows.md` §11 uses the same undefined word): a Tessera rule naming the image, or none. Changes implemented behaviour. **B3 (level texts, §4.2 implementer OPEN (a)).** BIN: a fellowship row's level is (UI, 0x070F9F3C) with argument 0x7C83B5 = the level (row creation and the level update use the same pair); an invitation row's level is (UI, 0x038B9CDC) with argument 0x50AA9C. The layouts' own entries of these texts (table 0x25033E35) are design-time placeholders. Changes implemented behaviour (plain number). **B4 (§3.5 implementer OPEN).** The heading (UF, 0x019353F5) declares argument 0xA2D94C5 (the note dialog's), not 0x52615; the implementation passes 0x52615, so the name is not substituted. Changes implemented behaviour. **B5 (§5.1 implementer OPEN).** The title label is `FellowInvite_TitleBar_Label` (0x100016A4), a child of `FellowInvite_TitleBar`; write the full label (implementation already uses it). **B6 (§4.2 implementer OPENs (b) and effects).** `VitalHealthField`, `VitalPowerField` (`MultiVitalDisplay`) and `FellowEffectDisplay` (`EffectDisplayList`, `PlayerOnly`, 2 × 6) are the game types `combat-ui.md` specifies on branch `p2` (Accepted); answer by pointing there (BIN: at row creation the effect list is bound to the member, or set player-only for the local player) or keep the Tessera fallback as an explicit rule; record that `FellowTemplatePetVitals` and `FellowTemplateVolumeControl` are hidden by the layout. **B7 (consistency with Accepted P4 texts).** Decision 0035 (Accepted) and `scenario-feed.md` "P4 social" (Accepted) add `DisbandFellowship` and `FellowVitalChanged`/`fellow-vital`, and name `social-panels.md` TR3 and Q12 as owners: TR3 (members without an entity take `FellowVitalChanged`), TR11, §8.1, §8.2, Q12 and §13 still say there is no disband command and that such members have empty meters; TR13 must follow the remote backend at minor 2 (`JoinChannel`, `LeaveChannel`, `Chat` on `Named` accepted). Code already sends `DisbandFellowship` (implementer OPEN under TR11), so aligning the text changes no behaviour. **Non-blocking:** (a) answer the TR12 key OPEN and the engineering-choices OPEN after §9 (a)–(d) as Tessera rules, and give that callout an anchor; (b) the §6.3 implementer OPEN (which entry is cleared) is a BIN question: answer or keep with a Tessera rule; (c) TR3 posts (FT, 0x0571E4C5) whose argument is 0x4AA1147, not the usual name argument: say so in TR3; (d) the status should be Review, not Draft, once complete, since code was built from it. |
| **Implements into** | `crates/ui` (`tessera-ui` `game` module: the social, fellowship, invitation and trade views and their fallback widgets), `apps/client` (`tessera-client`: the native panel P8, the fellowship panels, the invitation window, the trade window, the built-in commands of §7, the commands sent to the session), `crates/gamedata` (string look-ups by table key and token), `crates/plugins` (`Turbine.Gameplay` `Party` and `PartyMember`, with [`lua-plugin-api.md`](lua-plugin-api.md) §15.4) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (**UL**: layouts, inheritance §7, §17.2 row P8), [`ui-widgets.md`](ui-widgets.md) (**UW**: Panel §10, ListBox §8, Tree §9, Meter §4, Scrollbar §6, GroupBox §3.4), [`ui-behaviour.md`](ui-behaviour.md) (**UB**: input, dialogs §5.5, drag and drop §7, window manager §8), [`game-panels.md`](game-panels.md) (**GP**: drag-box frames §4, game types §11), [`character-panels.md`](character-panels.md) (**CP**: item displays §5), [`item-tooltips.md`](item-tooltips.md) (**IT**), [`icon-layers.md`](icon-layers.md) (**IL**), [`economy.md`](economy.md) §7 (**EC**: coin denominations), [`chat-window.md`](chat-window.md) (**CW**: command line §7, built-in names §8, Tessera decisions §10), [`keymap.md`](keymap.md) (**KM**: input commands §12.11, key-map records §12.9), [`game-controls.md`](game-controls.md) (**GC**: `Dialog_*` markers §3.2, social managers §8), [`string-table.md`](string-table.md) (**ST**), [`scenario-feed.md`](scenario-feed.md) (**SF**: steps §6.4, events §9.2, commands §9.4, session rules §9.5), [`lua-plugin-api.md`](lua-plugin-api.md) (**LPA**: §9.3, §15.4) |

> **Naming note (interoperability).** Element-ID labels (`Social_Field`, `FriendTemplate`, …),
> widget-type labels, state labels, input-command labels (`ToggleSocialPanel`, …), UI-layout
> labels (`social`, `securetrade`, …), string-table labels (`Fellowship`, `Chat`, …) and
> property names (`UI_Friend_…`, `UICore_…`) are identifiers read from the user's own data
> (enum mappers, the property dictionary and DID maps, UL §3.3). They are used only to find
> records, elements, states, properties and strings at run time. Numeric IDs and DIDs next to
> them are this install's values (2026-10-11) and are looked up by label or key at run time.
> **No game text is reproduced:** every string is named by its table (STRINGTABLE key) and
> token, and described only by its role. Command names are named by their `Commands` token
> (CW §8.1).

---

<a id="s0"></a>
## 0. Purpose and scope

Decision 0024 P4 asks for the social panels of the original client, presented from the
install's data and fed by the scenario feed (SF) and protocol v1.1, with the client half
only (`server-content.md` §7 unchanged: Tessera's server runs no gameplay rules). This spec
gives, for each panel, the layout structure, what each element shows, how the original's
game code drives it (where it was traced), the system chat lines that accompany the changes,
the built-in command names and key-map commands that open or act on it, and how every user
action and every display maps to the scenario feed's Social commands and events.

| Part | Section |
|---|---|
| Social panel P8 (UL §17.2): pages, friends list, ignore list, other pages | §3 |
| Fellowship display (the four `FellowshipVitals` panels), member rows, leader marker, removal, disbanding | §4 |
| Fellowship invitation prompt | §5 |
| Trade window | §6 |
| Built-in commands and key-map commands | §7 |
| Mapping to the session layer (SF §9.2, §9.4) | §8 |
| Tessera rules for the sandbox | §9 |
| Plugin views (pointer to LPA §15.4) | §10 |

**Out of scope:** tells and chat channels (CW; P4's chat work), synchronised music
(`instruments.md`), the kinship, raid, raid-lock, who and grouping pages beyond their
structure (§3.6), the fellowship maker (looking-for-group tool, GC §8.4), ready checks, voice
chat, pets, mail and auction. The **visibility** of the voice, ready-check and pet elements of
the panels in scope is in scope (§4.2, TR14).

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The owner's install (Steam; archives and `x64/lotroclient64.exe` unchanged since 2026-10-04), read in place and read-only on 2026-10-11 with throwaway probes in the dirty scratch area: the 302 UI layout records (the dump of 2026-10-06, re-resolved with a probe that merges by UL §7: derived properties replace base properties by PID; the P8 instance count 881, text elements 61 and string references 185 reproduce UL §17.2); the layouts `social` (0x2200004E), `friends` (0x22000061), `ignore` (0x22000062), the friends-page base 0x2200009B, `grouping` (0x2200004F), `minifellow` (0x2200005C), `fellowinvite` (0x22000604), `securetrade` (0x22000024), `itemdisplay` (0x22000051), `toolbar` (0x22000033); the STRINGTABLE DID map 0x28000004 (179 keys) and the string tables it names below (every entry of `Commands`, `CommandHelp`, `Fellowship`, `Trade`, `UI_SecureTrade`, `UI_Friends`, `UI_Social`, `PlayerActionHandler` read; text compared, not reproduced); the enum mappers `UIElementID`, `UIStateID`, `UIElementType`, `InputActions` (0x230000AF, base 0x23000176), `UIScalingGroupType`; the four default key-map records 0x1D000000, 0x1D000001, 0x1D000004, 0x1D000009 (parsed by KM §12.9 with no bytes left over). |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`), the shared auto-analysed Ghidra project (a private copy, headless, read-only), 2026-10-11. Only the routines that reference the element IDs, string tokens and event names below, and their direct callees, were read, by address range; no listing or decompiler output is kept. **Questions:** (q1) what the friends and ignore pages do on header, button, row and menu messages; which elements a friend row fills and from what; sort keys and default; (q2) how the fellowship panels are created, which members get a row, which template, which children are set from what, and how the leader marker is chosen; (q3) what the invitation window shows and what its buttons do; (q4) how the trade window is wired: sides, coin entries, item drops, the trade button, accept states, resets, closing, the request dialog; (q5) which input commands open which Social page; (q6) whether client code posts the fellowship, friends and trade notices or command names itself; (q7) the argument tables of `Party.MemberAdded`/`MemberRemoved` and the semantics of the `Party` getters (LPA §9.3 OPEN). **Why M1–M3 were not enough:** the layout data name elements, states and templates but not which code fills them, which rows are created, which states are chosen, or what the buttons send; the original client cannot be observed in this session; S2 and S3 document the plugin API only. The network protocol, login, keys, launcher and anti-tamper code were not read (decision 0004): where a routine hands an action to the session layer, only the fact that it does so is recorded. **Revision 1** (2026-10-11, same build, a private copy of the analysis project, headless, deleted after use; routines located by the element IDs and string tokens named below, read by address range): (q8) which of a fellowship row's status, voice and ready-check children the row-creation routine shows or hides, and from what; which of the player-vitals panel's voice elements its set-up and property handler show or hide; (q9) which `UI`-table entry and argument the fellowship row and the invitation row use for the level; how the row's vital displays and effect list are bound; (q10) which coin entry the trade's coin-commit handler clears and what it then sends. M1–M3 were not enough for the same reason as above: the layouts leave these elements visible and carry design-time texts. **Revision 2** (2026-10-14, same build, SHA-256 prefix d22363eb9aff0fb5 verified 2026-10-14, Ghidra 12.1.3 headless on a private read-only copy of the shared project, deleted after use; the row set-up located by the element ID of `FellowTemplateVolumeControl`, the scroll bar's handlers by its property IDs, read by address range): (q11) in which order the fellowship row's set-up writes `FellowTemplateVolumeControl`'s position and its visibility, and whether the scroll bar's own layout can show it again (the scroll-bar part is `ui-widgets.md` W29). M1–M3 were not enough: the data hide the bar, but whether code shows it again is behaviour. |
| **OBS-R2** | Revision 2 probes, 2026-10-14, read-only: the layout records 0x2200005C, 0x22000051, 0x22000033, 0x2200006C, 0x22000024 read again from the install with the repository's dump tool into the dirty scratch area, byte-identical to the 2026-10-06 dump (archives unchanged since 2026-10-04); resolved by UL §7 and instantiated; every state of `ItemDisplay_Name` in every item-display template counted and its colour properties listed (values not reproduced); the element IDs of `FellowMiniTemplate` and `FellowMicroTemplate` compared as sets; `VitalHealthFieldBg` and `FellowTemplateVolumeControl` read with their states. |
| **OBS-R1** | Revision 1 probes, 2026-10-11, read-only: the layout records 0x2200005C, 0x22000033, 0x22000051, 0x22000024, 0x2200006C re-extracted from the install and byte-identical to the 2026-10-06 dump; resolved by UL §7; the string tables `UI`, `UI_Friends`, `Fellowship` re-read for the arguments each entry declares; the client executable searched for method-name strings (no game text read or kept). |
| **S2** | The official Lua API reference (U25 pages in the research clone `lunarwtr_lotro-api-docs`), read for facts only (LPA key S2). |
| **S3** | `lunarwtr_vscode-lotro-api`: `TurbineEventsDump.json` (event argument tables captured in game), read for facts only (LPA key S3). |
| **DEC** | Accepted Tessera decisions [0024](../decisions/0024-parity-track.md) (parity track, P4), [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge), [0015](../decisions/0015-ui-rendering.md) (UI rendering), [0020](../decisions/0020-chat-command-routing.md) (one command dispatcher), [0035](../decisions/0035-p4-social-on-test-server.md) (P4 social on the test server: `DisbandFellowship`, `FellowVitalChanged`, the remote backend at protocol minor 2), and the Accepted SF (with its "P4 social" revision). Statements taken from them are **Tessera rules**. |

Marks: **Confirmed** = observed in the data for every record or element of the kind (OBS),
with date; **BIN** = rests on the disassembly; **Probable** = supported by data or BIN with one
step not traced; **Reported** = third-party only; **Tessera rule** = Tessera's own choice
(DEC or this spec), not a fact about the original.

| # | Fact | Sources | Mark |
|---|---|---|---|
| SP1 | The Social panel root `Social_Field`: scaled window (scaling group `Panels`), five tabs and pages in a `Panel` with the page list of §3.1; the Grouping ("Fellowing") page opens at set-up; the friends page has its own two-page panel (friends, ignore), friends open (§3.1) | OBS | Confirmed 2026-10-11 |
| SP2 | Friends page and ignore page elements, the row templates `FriendTemplate` (layout `friends`) and `IgnoreTemplate` (layout `ignore`), their children, colour properties and states (§3.2, §3.5) | OBS | Confirmed 2026-10-11 |
| SP3 | How a friend row is filled (name and online colour, class icon, level, area, fellowship state, last-online text, note tooltip), with the unknown-value token (§3.3) | BIN (q1) | BIN |
| SP4 | Friend sorting: header button → column element → key; default column `Member`, ascending; a second click on the same header reverses; tie-break by name (§3.4) | BIN (q1: the column map, the comparators, the initial values 0x100009A2 / true) | BIN |
| SP5 | Friends buttons: enablement by selection and online state, the add dialog (text-input dialog with a prompt token), remove, invite, tell, note, row menu entries and their conditions (§3.5) | BIN (q1) | BIN |
| SP6 | Ignore page: rows, sort by name or note with reversal, add dialog, remove, row menu (§3.6) | BIN (q1); OBS | BIN |
| SP7 | The friends and ignore notices are entries of the `Chat` table; the fellowship notices of the `Fellowship` table; the trade notices of the `Trade` table; none of their tokens is referenced by client code (§3.7, §4.6, §6.7) | OBS (tables); BIN (q6: immediate search of the executable found none) | Confirmed 2026-10-11 (data); BIN (absence) |
| SP8 | The notices are sent by the server as string references (they reach the chat as session lines) | SP7; the tables' roles | Probable |
| SP9 | Four fellowship panels `FellowMiniPanel_1` … `_4` (drag-box frames, scaling groups `FellowshipVitals1` … `4`), each with a mouse-invisible list box; panels 2–4 hidden at creation (§4.1) | OBS; BIN (q2) | Confirmed 2026-10-11 (layout); BIN (creation) |
| SP10 | Member row templates `FellowMiniTemplate` and `FellowMicroTemplate`; children and their data (§4.2) | OBS; BIN (q2) | Confirmed 2026-10-11 (layout); BIN (wiring) |
| SP11 | The local player gets no row; the player's own leader marker is `PlayerLeaderIcon` of the player-vitals panel P4; marker state `fellowship`, `raid` or `monsterplaygroup` by group kind; visible only for the leader (§4.3) | BIN (q2) | BIN |
| SP12 | Group size: six places (the player and five rows); evidence: input commands `SelectFellowOne` … `SelectFellowSix` bound to F1–F6, quest-tracker and quest-log fields `…FellowField1` … `5` and `Fellow1ImageField` … `Fellow5ImageField`, four raid groups (`Raid_Group_1_Box` … `4`) (§4.3) | OBS | Probable (the limit itself is server data) |
| SP13 | Clicking a member row targets the member; a member that is not in view gives a notice (out of range, or link-dead) instead (§4.5) | BIN (q2) | BIN |
| SP14 | A right click on a member opens the player menu; its group submenu offers leave (self), invite (non-member), dismiss and transfer leadership (leader), raid-assistant entries (raid); labels from `PlayerActionHandler` (§4.5) | BIN (q2: the submenu builder; the chain from the row to it Probable) | BIN / Probable |
| SP15 | The invitation window `FellowInvite` (layout `fellowinvite`): title, invite text and warning text supplied with the invitation, one row per member of the inviting group (class icon, level, name, area), accept, decline, close; accept answers yes, decline and close answer no; no client timer (§5) | OBS; BIN (q3) | Confirmed 2026-10-11 (layout); BIN (behaviour) |
| SP16 | The `Dialog_Fellowship*` markers of GC §3.2 are not the invitation prompt (§5.4) | BIN (q3: the prompt is the `fellowinvite` window) | BIN |
| SP17 | Trade window `SecureTradeField` (layout `securetrade`): own and peer sides, item lists of one column with the medium item display, three coin entries tagged with currency records, own wallet and peer coin displays, trade (boolean) and cancel buttons, a wait overlay (§6.1–§6.3) | OBS | Confirmed 2026-10-11 |
| SP18 | Trade behaviour: accept highlights the side's field; un-accept and any content change return both fields to `normal` and clear the trade button; the trade button is ghosted while both offers are empty; coin validation and its notices; item drops only from the own pack; cancel and close cancel the trade; a request from the peer opens a confirmation dialog first (§6.4–§6.6) | BIN (q4) | BIN |
| SP19 | The input commands `ToggleSocialPanel`, `ToggleSocialFellowingPanel`, `ToggleSocialFriendsPanel`, `ToggleSocialKinshipPanel`, `ToggleSocialRaidPanel`, `ToggleSocialGroupStagePanel` open P8 at the pages of §3.1 (§7.2) | BIN (q5); OBS (labels) | BIN |
| SP20 | Default key bindings: `ToggleSocialPanel` O; `SelectFellowOne` … `Six` F1 … F6; `AssistFellowTwo` … `Six` Shift+F2 … F6; the other social toggles unbound (§7.2) | OBS (0x1D000000) | Confirmed 2026-10-11 |
| SP21 | The social command names (`Commands` entries) are not referenced by client code except the trade channel's (CW §6.5); they are registered by game scripts (§7.1) | BIN (q6); CW §8.1 | BIN; registration Probable (CW CW36) |
| SP22 | `Party.MemberAdded` and `MemberRemoved` carry the field `Player` (the member), filled by the same routine as `AssistTargetAdded`/`Removed`; the field is absent when the member object is not available; `LeaderChanged` and `Player.PartyChanged` carry none (LPA §15.4) | BIN (q7); S3 (captures of `AssistTargetAdded` with `Player`, `AssistTargetRemoved`, `LeaderChanged`, `PartyChanged` without) | BIN |
| SP23 | `Party` getters: `GetMember(i)` and `GetAssistTarget(i)` are 1-based and give nil outside 1 … count; the member list holds the local player as an entry; `GetLeader` and `GetMember` give the `LocalPlayer` instance for the player (LPA §15.4) | BIN (q7) | BIN |
| SP24 | Everything marked **Tessera rule** in §3–§10 | DEC | Tessera rule |
| SP25 | The status, voice and ready-check children of the member rows: rectangles, images and states; none of them, nor `FellowTemplateLeader`, carries `UICore_Element_hide`; `FellowTemplatePetVitals` and `FellowTemplateVolumeControl` do; `FellowTemplateAssistant` exists only in `FellowMicroTemplate` (§4.2) | OBS-R1 | Confirmed 2026-10-11 |
| SP26 | At row creation the game code shows the voice button only when the member's group record has its voice flag set; shows the squelched marker only when voice is active and the button's squelch flag is set; hides the sending-data marker and the assistant marker; sets link-dead and defeated visibility each from a flag of the member's group record; gives the ready-check element no state (§4.2) | BIN (q8) | BIN |
| SP27 | The player-vitals panel P4's `PlayerVoiceChat`, `PlayerVoiceChatSquelched`, `PlayerSendingData` are not hidden by the layout; the panel's set-up hides the sending-data and squelched elements; `PlayerAssistantIcon` is hidden by the layout and hidden again by a further handler of the panel; `PlayerVoiceChat` follows a boolean property of the player (PID 0x54A) when it changes, and the squelched element follows voice activity and the button's squelch flag (§4.2) | OBS-R1; BIN (q8) | Confirmed 2026-10-11 (layout); BIN (code) |
| SP28 | Level texts: a fellowship row's level is (UI, 0x070F9F3C) with argument 0x7C83B5 = the level, at creation and on a level update; an invitation row's level is (UI, 0x038B9CDC) with argument 0x50AA9C; both entries declare exactly that one argument; the layouts' own entries of these texts (table 0x25033E35) are design-time texts (§4.2, §5.1) | BIN (q9; the first token is computed at start-up from a name string, the hash reproduced); OBS-R1 (declared arguments) | BIN; Confirmed 2026-10-11 (arguments) |
| SP29 | The row's two `MultiVitalDisplay` elements name their meters through `UI_VitalDisplay_Array` (health: current, temporary, dread-clamp meters and a hidden text; power: current, temporary meters and a hidden text, then a `Wrath` pip entry whose replacement field is the power field); `FellowEffectDisplay` is an `EffectDisplayList` with 2 rows of at most 6 icons and `PlayerOnly` true in the layout; at creation the vital displays are bound to the member, and the effect list is bound to the member, or set player-only when the member is the local player (§4.2) | OBS-R1; BIN (q9) | Confirmed 2026-10-11 (layout); BIN (binding) |
| SP30 | The medium item display: `ItemDisplay_Icon` (an `ItemIconDisplay`) holds the Text `ItemDisplay_QuantityOverlay` (hidden by the layout), which precedes the name Text `ItemDisplay_Name` in tree order; the name text has 32 states (corrected in revision 2 from 33): 31 `…_ColorName` states and `Floaty_CriticalDamage`; they include the five `ItemQuality_*_ColorName` states and `Object_ColorName` (§6.2) | OBS-R1; OBS-R2 (the count) | Confirmed 2026-10-11; count Confirmed 2026-10-14 |
| SP31 | The coin entries of the own side are the Text elements `MoneyDisplay_Entry_GoldEntry`, `…SilverEntry`, `…CopperEntry` (editable, one line, at most 5 characters, no IME); the trade code names exactly these three IDs; `SelfTrade_TotalMoney` and `OtherTrade_MoneyDisplay` have no layout children and are horizontal one-row lists of three columns of `MoneyDisplay_CoinTemplate` of layout `moneydisplay` (0x2200006C), filled in reverse, `ForceShowAll` on both and `SuppressLeadingZeroes` on the peer's (§6.1, §6.3) | OBS-R1; BIN (q10) | Confirmed 2026-10-11 (layout); BIN (IDs used) |
| SP32 | The coin-commit handler tests only the committed entry's text for a number and clears only that entry on each refusal; it posts its notices as chat type 1; in every case it then reads the three entries and sends the amounts present (§6.3) | BIN (q10) | BIN |
| SP33 | The row-menu heading (UF, 0x019353F5) declares the argument 0xA2D94C5 (as the note question does); the out-of-range notices (FT, 0x0571E4C5) and (FT, 0x04CF6294) declare 0x4AA1147 (§3.5, §4.4) | OBS-R1 | Confirmed 2026-10-11 |
| SP34 | The two row templates differ in exactly two element IDs besides their roots: `VitalHealthFieldBg` only in `FellowMiniTemplate`, `FellowTemplateAssistant` only in `FellowMicroTemplate` (48 instances each). `VitalHealthFieldBg` (a Field, 200 × 80) has 112 states: its default state `normal`, which carries the plain background image, and 111 others, the vital-display override backgrounds (§4.2, TR15) | OBS-R2 | Confirmed 2026-10-14 |
| SP35 | At row creation the game code sets `FellowTemplateVolumeControl`'s position (under a condition on the member that was not traced) **before** it writes the bar's visibility from the row's voice flag (clear when the member record's voice flag is not set, SP26), so that write is the last; the bar's own layout writes the same visibility (`ui-widgets.md` §6.4, W29). In the layout the bar is hidden in its own block and in its default state `normal`, with `UICore_Scrollbar_hide_when_disabled` true and not disabled (§4.2, TR14) | BIN (q11); OBS-R2 | BIN; Confirmed 2026-10-14 (layout) |
| SP36 | In each of the 10 item-display templates whose `ItemDisplay_Name` has the 32 colour states, the name's own block and each of its 32 states carry a `UICore_Text_font_colors` array of exactly one colour and no `UICore_Text_font_color`; the name's default state is none; `ItemDisplay_MediumTemplate`'s root has no states and its own block does not pass states to its children (UL §9.4 step 4), so only code requests change the name's state (§6.2) | OBS-R2 | Confirmed 2026-10-14 |

---

<a id="s2"></a>
## 2. Conventions

**String tables.** Strings are named `(table, token)`. Tables are found through the
STRINGTABLE DID map (root key 0x12, `records.md` §8.2) by **key**; label and this install's
DID are given for orientation only:

| Short | Key | Label | DID here | Entries |
|---|---|---|---|---|
| CMD | 0x10000007 | `Commands` | 0x250001AB | 261 |
| CH | 0x1000004C | `CommandHelp` | 0x250001AA | 179 |
| CT | 0x1000004D | `Chat` | 0x250001A7 | |
| FT | 0x10000017 | `Fellowship` | 0x25000021 | 144 |
| TT | 0x10000062 | `Trade` | 0x250001BA | 34 |
| UST | 0x10000031 | `UI_SecureTrade` | 0x250001DB | 14 |
| UF | 0x1000005E | `UI_Friends` | 0x250001D4 | 57 |
| US | 0x1000002F | `UI_Social` | 0x250001ED | 235 |
| UI | 0x10000065 | `UI` | 0x250001BB | |
| PA | 0x10000001 | `PlayerActionHandler` | 0x250001E0 | 84 |
| UG | 0x10000066 | `UI_Guild` | 0x250001B5 | |

A string with arguments is rendered by ST with the argument named by its ID; the argument IDs
below are the ones the entry declares (0x52615 is the usual player-name argument).

**Elements and states.** Elements are named by their `UIElementID` label (ID here); states by
their `UIStateID` label: `normal` (0x0B), `highlight` (0x07), `ghosted` (0x12) and the
group-kind states `fellowship` (0x100000C2), `raid` (0x1000012B), `monsterplaygroup`
(0x1000026B). "Ghost" means request state `ghosted` (UW §7.3); "un-ghost" means request
`normal`.

**Layouts** are opened by `UILAYOUT` key (UL §17.1): `social` 0x10000073, `friends`
0x10000077, `ignore` 0x1000002F, `minifellow` 0x1000003B, `fellowinvite` 0x100000A0,
`securetrade` 0x1000002B, `itemdisplay` 0x1000004A (keys of this install).

---

<a id="s3"></a>
## 3. The Social panel (P8)

<a id="s3-1"></a>
### 3.1 Root, tabs and pages (Confirmed, SP1; BIN, SP19)

`Social_Field` (0x10000ABE) of layout `social`: type `UIScaler`, design rectangle
(262, 118, 587 × 577), scaling group `Panels` (31), origin (293, 285), saved location,
activates on show, auto-fade 0.1, minimum size 587 × 577, object mode CanvasResize; its
title text is (UI, 0x06A7AA5C). Children: base box, title bar, drag bar `MoveBar`, close button
`Social_CloseButton` (tooltip (UI, 0x08921EF4)), the panel `Social_Panel` and a resize bar.

`Social_Panel` (type `Panel`, UW §10) holds five tabs (Buttons based on `Tab_Tier1_Left`,
100 × 29 except the last, 150 × 29, in a row at y = 2) and their pages; page list in stored
order:

| Tab (label, x) | Tab text | Page | Open at set-up | Page content |
|---|---|---|---|---|
| `Friends_Tab` (107) | (UI, 0x06220FA2) | `Friends_Page` (0x10000AC8), based on `FriendsPageBase` of 0x2200009B | no | §3.2–§3.6 |
| `Kinship_Tab` (209) | (UI, 0x079A1902) | `Kinship_Page`, based on `Guild2PageField` of layout `guild` | no | kinship (§3.8) |
| `GroupingTab` (5) | (UI, 0x007FAE52) | `GroupingPage` (layout `grouping`) | **yes** | who and quest-grouping lists (§3.8) |
| `Raid_Tab` (311) | (UI, 0x0DAD7C92) | `Raid_Page`, based on `Raid_Base` of 0x2200009C | no | four raid groups (§3.8) |
| `RaidLock_Tab` (413) | (UI, 0x05B5A702) | `RaidLock_Page`, based on `RaidLocksPage` of 0x220000BF | no | lock lists (§3.8) |

So the panel opens on the grouping page (UW §10.2). Opening by input command (BIN, SP19):
the game code maps `ToggleSocialPanel` (0x10000035) to `Social_Field` itself and five page
commands to pages: `ToggleSocialFellowingPanel` (0x100001C3) → `GroupingPage`,
`ToggleSocialFriendsPanel` (0x100001C5) → `Friends_Page`, `ToggleSocialKinshipPanel`
(0x100001C1) → `Kinship_Page`, `ToggleSocialRaidPanel` (0x100001C4) → `Raid_Page`,
`ToggleSocialGroupStagePanel` (0x100001C2) → `RaidLock_Page`.

> **OPEN (Q1):** whether a page command on an open panel showing another page switches the
> page or closes the panel, and whether `ToggleSocialPanel` on an open panel closes it
> (toggle semantics of the activatables, not traced). **Tessera rule:** `ToggleSocialPanel`
> shows the panel at the page it last showed (first time: set-up page) or hides it when
> shown; a page command shows the panel at its page, and hides it only when it is shown at
> that page already.

The friends page holds its own `Panel`, `FriendsPagePanel`, with two tab Buttons at the
bottom (y = 470, 160 × 20): `FriendsTabMarker` (text (US, 0x006D07EC), tooltip
(US, 0x07F50264)) → `FriendsPage`, open at set-up; `IgnoreTabMarker` (text (US, 0x04C135BC),
tooltip (US, 0x04D04CA4)) → `IgnorePage`.

<a id="s3-2"></a>
### 3.2 Friends page elements (Confirmed, SP2)

`FriendsPage` (0x1000099B, 565 × 470) holds:

| Element (ID) | Type, rect | Data / role |
|---|---|---|
| `FriendsListBox_BG` | Field (7, 54, 544 × 380) | box art |
| `FriendsHeader` (0x100009A1) | GroupBox (6, 34, 544 × 17); default button `FriendHeaderMemberButton`, a click on the selected button allowed (UW §3.4) | the column headers below |
| `FriendHeaderFellowshipButton` (0x100009A5) | Button (3, 2, 16 × 16) | sort by grouped |
| `FriendHeaderClassButton` (0x100009A4) | (25, 0, 40 × 16); text (UF, 0x0E1B7A3C); tooltip (US, 0x0DDAAC82) | sort by class |
| `FriendHeaderLevelButton` (0x100009A3) | (64, 0, 35 × 16); (UF, 0x008B346C); (US, 0x050648A2) | sort by level |
| `FriendHeaderMemberButton` (0x100009A2) | (92, 0, 120 × 16); (UF, 0x03E5EA3C); (US, 0x04D0FD12) | sort by name |
| `FriendHeaderLocationButton` (0x100009A6) | (215, 0, 100 × 16); (UF, 0x018BB5CC); (US, 0x07F06092) | sort by area |
| `FriendHeaderLastOnButton` (0x100009A7) | (323, 0, 109 × 16); (UF, 0x03C6F26C); (US, 0x05AFDDD2) | sort by last time online |
| `FriendHeaderNoteButton` (0x100022FB) | (434, 0, 109 × 16); (UF, 0x0AE5EBDC) | sort by note |
| `FriendListBox` (0x100009A8) | **Tree** (10, 58, 540 × 371); item states: selected `highlight`, other `normal`; click selects; scroll bar `FriendsScrollbar` | one `FriendTemplate` row per friend |
| `FriendsScrollbar` | Scrollbar (553, 54, 10 × 380); tooltip (UI, 0x0A1C58DC) | |
| `FriendAddButton` (0x1000099F) | text button (29, 439, 93 × 20); (UF, 0x061A078E) | add (§3.5) |
| `FriendRemoveButton` (0x100009A0) | (124, 439); (UF, 0x05A155FE) | remove |
| `FriendNoteButton` (0x10002316) | (250, 439); (UF, 0x01F1BC1E) | note |
| `FriendInviteButton` (0x10001012) | (343, 439); (UF, 0x02CE839E) | invite to the fellowship |
| `FriendTellButton` (0x10001011) | (437, 439); (UF, 0x08F0A88E) | tell |

**Row template** `FriendTemplate` (0x100009AA, layout `friends`): a Button 539 × 30 (min and
max height 30), roll-over on, draggable, context menu on, examination tooltip override layout
0x22000000 element `Friend_ExamineLayout`; states `normal`, `highlight`, roll-over, pressed,
`ghosted`. Children:

| Child (ID) | Type, rect | Shows |
|---|---|---|
| `FriendTemplate_BG` | Field 539 × 30 | box art per state |
| `FriendTemplateFellowship` (0x100009B0) | Field (8, 6, 16 × 16); states `fellowship_unknown` (default), `fellowship_none`, `fellowship_fellowed` | grouped marker |
| `FriendTemplateClassIcon` (0x100009AC) | ClassDisplay (27, 4, 20 × 20), a 22-entry class → icon table; updates its tooltip | class icon |
| `FriendTemplateClass` (0x100009AE) | Text (27, 4, 30 × 20), **hidden by the layout** | class name (sort key only) |
| `FriendTemplateLevel` (0x100009AF) | Text (50, 4, 35 × 20), centred | level |
| `FriendTemplateLabel` (0x100009AD) | Text (88, 4, 120 × 20), left; `UI_Friend_OnlineColor` 0xFF00FF00, `UI_Friend_OfflineColor` 0xFFDBDB00, `UI_Friend_DeletedColor` 0xFFFF0000 | name |
| `FriendTemplateLocation` (0x100009B1) | Text (210, 4, 106 × 20), centred | area |
| `FriendTemplateLastOn` (0x100009B2) | Text (319, 4, 107 × 20) | last time online |
| `FriendTemplateNote` (0x100022FA) | Text (430, 2, 108 × 22), centred, multi-line | note |

Every text child truncates with the suffix (UI, 0x037DDBC3) (the note with (UI, 0x00930E13))
and shows the full text as a tooltip when truncated (UB §10).

<a id="s3-3"></a>
### 3.3 What a friend row shows (BIN, SP3)

For each friend the game code creates one `FriendTemplate` row from layout `friends`, fills
it, stores the friend's character ID on the row (`UI_Friend_InstanceID`) and adds it to
`FriendListBox`; a friend already shown gets no second row. "Known" below means the friend
record carries the value and the game code's visibility test for that friend passes (a test
on the record, not traced further); otherwise the column shows the **unknown text**
(UF, 0x0A8290DF).

| Column | Rule |
|---|---|
| Name | The friend's name. Colour: `UI_Friend_OnlineColor` when the record's status bit 0 (online) is set, else `UI_Friend_OfflineColor`. (`UI_Friend_DeletedColor` is not used by this routine.) |
| Class | Class known: the class name into the hidden text, and the class icon by the ClassDisplay table; else the unknown text. |
| Level | Known: the template (UF, 0x06CDDF0C) with argument 0x50AA9C = level; else the unknown text. |
| Area | Online, known and not flagged hidden: the name of the area record the friend is in; else the unknown text; set through the template (UF, 0x0FDC19AC), argument 0x376892E. |
| Grouped marker | State `fellowship_fellowed` when online, known and the record says the friend is in a group; otherwise `fellowship_unknown` (the routine never requests `fellowship_none`). |
| Last online | Template (UF, 0x0FE845CC), argument 0x9480CA5 = online: (UF, 0x0C3E9831) when status bit 1 (away) is set, else (UF, 0x0FF052F5); offline with a last-logout time: that date and time, formatted; offline without, or not known: the unknown text. |
| Note | The friend's note text (also kept as the row's `UI_Friend_Note`); empty: cleared. |

> **OPEN (Q2):** the record test behind "known" (Probable: whether the friend allows the
> details to be seen) and where `UI_Friend_DeletedColor` is used (a deleted character's row,
> not traced). **Tessera rule:** §9 TR5 (the feed carries no details).

<a id="s3-4"></a>
### 3.4 Sorting (BIN, SP4)

Each header button selects a column element of the row and a comparison:

| Header | Column element | Order (ascending) |
|---|---|---|
| Member | `FriendTemplateLabel` | displayed text, case-insensitive |
| Level | `FriendTemplateLevel` | level number; equal levels by name, case-insensitive |
| Class | `FriendTemplateClass` | displayed class text, case-insensitive |
| Grouped | `FriendTemplateFellowship` | not grouped before grouped; equal by name |
| Area | `FriendTemplateLocation` | displayed text, case-insensitive |
| Last online | `FriendTemplateLastOn` | offline before online; offline by last-logout time, earliest first; online by name |
| Note | `FriendTemplateNote` | displayed text, case-insensitive |

The sort column and direction are client-wide values, initially Member and ascending. A click
on a header: if it is the current column the direction flips, otherwise the column changes
and the direction becomes ascending; then the list is sorted. Descending negates the
comparison. For the level, grouped and last-online orders, rows whose friend record is
missing sort before rows that have one.

<a id="s3-5"></a>
### 3.5 Buttons, row menu and dialogs (BIN, SP5)

**Selection.** A click on a row selects it (the list's click-select); a right click selects it
and opens the row menu. The selection drives the buttons, re-evaluated on every selection
change and on creation:

| Button | Normal when | Else | Action |
|---|---|---|---|
| Remove | a row is selected | ghosted | remove the selected friend |
| Note | a row is selected | ghosted | note dialog |
| Tell | the selected friend is online | ghosted | start a tell to the friend in the chat entry (CW §6.5) |
| Invite | the selected friend is online and not grouped | ghosted | invite the friend to the player's fellowship |
| Add | always | — | add dialog |

A double click on a row also starts a tell (only when online).

**Add dialog:** an engine text-input dialog (UB §5.5; `UICore_Dialog_type` text input) with
the question (UF, 0x0BC25DB4), a maximum length taken from the client's name-length limit,
and focus on the field; its answer adds the entered name. **Note dialog:** a text-input
dialog with the question (UF, 0x0803A974) (argument 0xA2D94C5 = the friend's name) and the
current note as default answer.

**Row menu** (in this order): a heading (UF, 0x019353F5) with argument 0xA2D94C5 = the friend's
name (SP33; the same argument as the note question); send tell
(UF, 0x0743846D), enabled when online or away; invite to fellowship (UF, 0x04529325),
enabled when the player is in no group or leads one that may grow and the friend is online
or away; remove friend (UF, 0x033D8545); ignore (UF, 0x044B1505); edit note
(UF, 0x038601E5); and a toggle between "auto-fellow" (UF, 0x065BE227) and "no auto-fellow"
(UF, 0x00CB5BA7).

> **OPEN (Q10):** notes and auto-fellow have no field in the feed or protocol v1.1.
> **Tessera rule:** §9 TR5 (ghosted).

> **Answered (revision 1, 2026-10-11; was an implementer OPEN):** the heading's argument is
> 0xA2D94C5, not 0x52615 (SP33). With 0x52615 the name is not substituted.

<a id="s3-6"></a>
### 3.6 Ignore page (Confirmed, SP2; BIN, SP6)

`IgnorePage` (0x1000099C): box art; `IgnoreHeader` GroupBox with `IgnoreHeaderMemberButton`
(text (UF, 0x061B325C), tooltip (US, 0x04D0FD12)) and `IgnoreHeaderNoteButton`
((UF, 0x0DDA788E)); `IgnoreListBox` (0x10000AA2, **ListBox**, 540 × 371, item states as the
friends list, click select) with `IgnoreScrollbar` (tooltip (UI, 0x0FB043BC)); buttons
`IgnoreAddButton` ((UF, 0x03B674EE)), `IgnoreRemoveButton` ((UF, 0x02F72F3E)),
`IgnoreNoteButton` ((UF, 0x07BBFCEE)).

Row template `IgnoreTemplate` (0x10000AA5, layout `ignore`): Button 539 × 30, context menu on;
children `IgnoreTemplate_BG`, `IgnoreTemplateLabel` (5, 4, 257 × 20: the name) and
`IgnoreTemplateNote` (266, −1, 267 × 30: the note, truncated with (UI, 0x00930E13)).

Behaviour: Remove and Note are normal only with a selected row (else ghosted); Add opens a
text-input dialog with the question (UF, 0x09037E74); Remove removes the selected name; the
row menu has a heading with the name (UF, 0x0B607755, argument 0x46EFA25), "remove from
ignore" (UF, 0x0177E995) and "edit note" (UF, 0x0F57CB45). The two headers sort by name or
note; a second click on the current header reverses (initial values not traced; Tessera rule
§9 TR6: name, ascending).

> **OPEN (Q7):** the ignore list's initial sort column and direction (a client-wide value
> whose initialiser was not read). **Tessera rule:** §9 TR6.

<a id="s3-7"></a>
### 3.7 Friends and ignore notices (Confirmed data, SP7; Probable, SP8)

The game code of the panel posts none of these lines; they are `Chat`-table entries that the
original receives as session lines:

| Role | (table, token) | Argument |
|---|---|---|
| friend added | (CT, 0x0D523E83) | 0x52615 name |
| friend removed | (CT, 0x0FF08EA3) | name |
| friend came online | (CT, 0x0A29AFC5) | name |
| friend went offline | (CT, 0x010538E5) | name |
| already a friend | (CT, 0x004E65B3) | name |
| not a friend | (CT, 0x033C0074) | name |
| cannot add oneself | (CT, 0x07DA5D66) | — |
| friends list full | (CT, 0x0D8B619C) | — |
| friends list empty | (CT, 0x06577433) | — |
| ignored added | (CT, 0x05582E33) | name |
| ignored removed | (CT, 0x01A6B823) | name |
| already ignored | (CT, 0x00190E13) | — |
| not ignored | (CT, 0x06522024) | — |
| cannot ignore oneself | (CT, 0x028CE766) | — |
| ignore list full | (CT, 0x0B3166CC) | — |
| ignore list header | (CT, 0x043D716F) | — |
| ignore list empty | (CT, 0x01785864) | — |

<a id="s3-8"></a>
### 3.8 The other pages (Confirmed structure, SP1)

The grouping page holds a `Panel` with a who page (open at set-up: filters by class, level and
name, a member list with invite and tell buttons) and a quest-grouping page (quest selector,
member list, join, tell, find, add, remove, an unavailable overlay). The kinship page shows
the kinship's name, rank, message of the day and a member list with commands. The raid page
shows four groups (`Raid_Group_1_Box` … `4`) with a list each, a convert-to-raid button and a
button for the raid assist window. The raid-lock page lists personal, group and chest locks.
Their data are server data (kinship, who lists, raids, locks); see §9 TR8.

---

<a id="s4"></a>
## 4. Fellowship display ("fellowship vitals")

<a id="s4-1"></a>
### 4.1 The four panels (Confirmed, SP9; BIN)

Layout `minifellow` holds four top-level panels `FellowMiniPanel_1` … `_4`
(0x10000442 … 0x10000445), each based on `FellowMiniPanel` (itself based on
`HiddenDragBox_Template`), so each is a drag-box frame (GP §4) titled (UI, 0x089C58DC):

| Panel | Design position | Edges | Scaling group (LPA Appendix A `UIScalingGroupType`) |
|---|---|---|---|
| `FellowMiniPanel_1` | (14, 121) | (5, 5, 4, 4) | `FellowshipVitals1` (15) |
| `FellowMiniPanel_2` | (234, 121) | (5, 5, 4, 4) | `FellowshipVitals2` (16) |
| `FellowMiniPanel_3` | (586, 121) | (4, 5, 5, 4) | `FellowshipVitals3` (17) |
| `FellowMiniPanel_4` | (810, 121) | (4, 5, 5, 4) | `FellowshipVitals4` (18) |

Each is 221 × 452, saves its location, is not activatable and does not block clicks; its
`HiddenDragBox_AdjustForChildSizeChanges` and scaler target name its list box
`FellowMiniPanelListBox` (0x1000044B: type `ListboxMouseInvisible`, (0, 20, 209 × 432),
click-select off, own surface in mode Stretch). The game code creates all four with the
layout (BIN) and hides panels 2–4 at creation; panel 1 is the player's own group. When a
raid is shown split by groups (a gameplay option), panels 2–4 take the other raid groups in
group order (BIN; the option's identity not traced, Q4).

> **OPEN (Q4):** which gameplay option splits a raid over panels 2–4, and when the game code
> uses `FellowMicroTemplate` instead of `FellowMiniTemplate` (a flag of the group display, not
> traced to an option). **Tessera rule:** no raids in P4; panel 1 and the mini template only.

<a id="s4-2"></a>
### 4.2 Member rows (Confirmed layout, SP10; BIN wiring)

A row is `FellowMiniTemplate` (0x10000436, 221 × 75, context menu on) or, in the compact form
of a raid display, `FellowMicroTemplate` (0x10000446, 156 × 70, based on
`ClassDisplay_RaidVitalBackTemplate`); the micro template has the same child IDs except
`VitalHealthFieldBg`, which it lacks, and in addition `FellowTemplateAssistant` (0x10000447),
which the mini template lacks (SP25, SP34; corrected in revision 2). The game code fills them
from the member's group record (BIN):

| Child (ID) | Mini rect | Shows |
|---|---|---|
| `FellowTemplateNameText` (0x10000438) | (56, 2, 133 × 14) | the member's name (the layout's own entry, of table 0x25033E35, is a design-time text and is replaced) |
| `FellowTemplateClassField` (0x10000439) | ClassDisplay (5, 3, 50 × 50) | class icon (22-entry table) |
| `FellowTemplateLevelText` (0x10000440) | (12, 42, 32 × 19) | the level: (UI, 0x070F9F3C) with argument 0x7C83B5 = the level, set at creation and again on each level update (SP28); the layout's own entry is a design-time text |
| `FellowTemplateVitalField` (0x10000437) | (3, −3, 170 × 80) | `VitalHealthField` and `VitalPowerField` (`MultiVitalDisplay`, at (52, 25) and (51, 32), 99 × 6), both bound to the member at creation; `VitalHealthFieldBg` (200 × 80, mini template only; default state `normal`, SP34) takes the member's vital-display override state (below, SP29); Tessera: TR15 |
| `FellowTemplateLeader` (0x1000043A) | (39, 34, 16 × 16) | leader marker (§4.3) |
| `FellowTemplateLinkDead` (0x1000043B) | (55, 20, 112 × 16), image, child text (UI, 0x00B8F659) | the member is link-dead |
| `FellowTemplateDefeated` (0x100010C9) | (55, 20, 112 × 16), image, child text (UI, 0x08FAF389) | the member is defeated |
| `FellowEffectDisplay` (0x1000043E) | (54, 37, 150 × 37); `EffectDisplayList`, 2 rows of at most 6 icons, `PlayerOnly` true in the layout | the member's effects: bound to the member at creation, or set player-only when the member is the local player (SP29) |
| `FellowAssist` (0x1000043F) | Button (153, 21, 16 × 16), tooltip (UI, 0x0C110D2E) | assist the member |
| `FellowTemplateVoiceChat` (0x1000043C) | Button (38, 0, 20 × 20), default image 0x41007DD0 | voice chat on |
| `FellowTemplateVoiceChatSquelched` (0x100010CE) | Field (38, 0, 20 × 20), image 0x410202F4 | the member is squelched |
| `FellowTemplateSendingData` (0x1000043D) | Field (38, 0, 20 × 20), image 0x41007DD1 | the member is speaking |
| `FellowReadyCheckTemplate` (0x1000106A) | Field (150, −8, 64 × 67), no default image; states `PendingReadyCheck`, `Ready`, `NotReady` with images | ready check |
| `FellowTemplatePetVitals` (0x10000441) | hidden by the layout | the member's pet |
| `FellowTemplateVolumeControl` (0x1000422B) | Scrollbar, hidden by the layout | voice volume |

<a id="s4-2-1"></a>
**Visibility of the status, voice and ready-check elements** (SP25, SP26). In the layout none
of `FellowTemplateLeader`, `…LinkDead`, `…Defeated`, `…VoiceChat`, `…VoiceChatSquelched`,
`…SendingData` or `FellowReadyCheckTemplate` is hidden; the first six carry images, so a row
built from the layout alone shows all of them stacked. The game code decides:

| Element | Original, at row creation (BIN) | Tessera (TR14) |
|---|---|---|
| `FellowTemplateLeader` | state by group kind; shown only on the leader's row (§4.3) | TR2 |
| `FellowTemplateLinkDead` | shown or hidden from the member record's link-dead flag | hidden (TR3: link-dead is never reported) |
| `FellowTemplateDefeated` | shown or hidden from another flag of the member record | shown while the member is defeated (TR3), else hidden |
| `FellowTemplateVoiceChat` | shown only when the member record's voice flag is set | hidden |
| `FellowTemplateVoiceChatSquelched` | shown only when voice is active and the voice button's squelch flag is set | hidden |
| `FellowTemplateSendingData` | hidden | hidden |
| `FellowTemplateAssistant` (micro only) | hidden | hidden (the micro template is not used, TR2) |
| `FellowReadyCheckTemplate` | no state requested (no default image, so nothing is drawn) | no state requested |
| `FellowTemplateVolumeControl` | shown only together with voice: the row writes its visibility after setting its position, so the bar's own layout (which writes the same visibility, `ui-widgets.md` §6.4) does not undo it (SP35) | hidden, by keeping the bar disabled with `hide_when_disabled` (TR14, §9.1 (f)) |
| `FellowTemplatePetVitals` | hidden by the layout | hidden |

The vital displays and effect list (SP29) follow the game types `combat-ui.md` specifies
(branch `p2`, Accepted: §4.3 for `MultiVitalDisplay` parts named through
`UI_VitalDisplay_Array`, §8.1–§8.2 for `EffectDisplayList`). Until that spec reaches this
branch, and after it for the member-specific parts, TR15 and TR16 are the rules. In this row:

| Display | `UI_VitalDisplay_Array` entries (in order) |
|---|---|
| `VitalHealthField` | vital `Health`: meter `VitalCurrentHealthMeter` (0x10000460), temporary meter `VitalTemporaryHealthMeter` (0x10002CF3), clamped meter `VitalCurrentDreadMeter` (0x10000462), text `VitalCurrentHealthText` (0x10000463, hidden by the layout) |
| `VitalPowerField` | vital `Power`: meter `VitalCurrentPowerMeter` (0x10000466), temporary meter `VitalTemporaryPowerMeter` (0x10002CF4), text `CurrentPowerText` (0x10000467, hidden); then a pip entry of type `Wrath`: field `RageMeter` (0x10003B77), meter `CurrentRageMeter` (0x10003B79), text `RageText` (hidden), replacement field `VitalPowerField` (the inner field of the same ID) |

The other `Meter` elements below the displays (`VitalHealthBackground`, `VitalPowerBackground`,
`VitalHealthParentField`) are background and grouping elements, not named by the array.

Meter colours follow the meters' own states (`vitalmeter_green`, `_red`, `_yellow`, `_blue`,
`_grey`, UW §4); the game code's choice among them is `combat-ui.md`'s and is not changed here.

> **Answered (revision 1, 2026-10-11; were implementer OPENs):** (a) the level templates are
> (UI, 0x070F9F3C)/0x7C83B5 for the fellowship row and (UI, 0x038B9CDC)/0x50AA9C for the
> invitation row (SP28). (b) Setting `UICore_Meter_position` on every `Meter` below the vital
> displays also fills the background, temporary, dread and wrath meters; only the array's
> current meters take the value (TR15). (c) The effect list follows `combat-ui.md` §8 with the
> row's own limits (TR16).

<a id="s4-3"></a>
### 4.3 Who gets a row, order, limit, leader marker (BIN, SP11; Probable, SP12)

- **The local player gets no row** (BIN): the member list routine skips the player's own
  character. The player's vitals are panel P4; its child `PlayerLeaderIcon` (0x10000B4E,
  hidden by the layout, image 0x41007DCE) is the player's leader marker.
- **The player's own voice elements** (SP27): P4's `VitalsParent` also holds `PlayerVoiceChat`
  (0x10000B50, image 0x41007DD0), `PlayerVoiceChatSquelched` (0x100010CF, image 0x410202F4)
  and `PlayerSendingData` (0x10000B51, image 0x41007DD1), all three at (71, 11) 20 × 20 and
  **not** hidden by the layout. The panel's set-up hides `PlayerSendingData` and
  `PlayerVoiceChatSquelched` (`PlayerAssistantIcon`, 0x10000B4F, is hidden by the layout and
  hidden again by a further handler of the panel); `PlayerVoiceChat` is shown
  or hidden when the player's boolean property PID 0x54A changes, and the squelched element
  then follows voice activity and the voice element's squelch flag (BIN). Tessera hides all
  three (TR14).
- **Leader marker** (BIN): on every row and on `PlayerLeaderIcon`, the game code requests
  state `raid` when the group is a raid, else `monsterplaygroup` when the player is a monster
  player (alignment 3), else `fellowship`; and shows the element only for the group's leader.
  The three states differ only in their tooltip: `fellowship` (UI, 0x052C4C12), `raid`
  (UI, 0x0FF8FA02), `monsterplaygroup` (UI, 0x046656A2).
- **Order** (Probable): rows are appended to the list box as members are reported; a rebuild
  walks the group's member list. The server's order is not visible here (Q3).
- **Limit** (Probable, SP12): six places, the player and five rows. Five rows of 75 pixels fit
  the 432-pixel list box.

> **OPEN (Q3):** the order in which the server reports members, and so the row order and the
> order of `Party:GetMember`. **Tessera rule:** the order of the feed's list (§9 TR2, LPA
> §15.4).

<a id="s4-4"></a>
### 4.4 A member out of view (BIN, SP13)

A row stays while the member is in the group, whether or not the member is in view; its
vitals come from the group record, not from a visible entity. A left click on a row targets
the member; when the member's entity is not available the game code instead posts a notice
from the `Fellowship` table: (FT, 0x04CF6294) when the member is link-dead, else
(FT, 0x0571E4C5) (out of range), argument 0x4AA1147 = the member's name (SP33; not the usual
name argument 0x52615), and clears the target.

<a id="s4-5"></a>
### 4.5 Menu and assist (BIN, SP14)

A right click on a row opens the client's player menu for that member (Probable chain). Its
group submenu, headed (PA, 0x005085BC) in a raid, else (UG, 0x08AC5E85) for a fellowship or
(UG, 0x0C5E8280) for a monster player's group, holds:

| Entry | Label | Shown when |
|---|---|---|
| leave | (PA, 0x0A21737C) | the target is the player and the player is in a group |
| invite | (PA, 0x0CDD782C) | the target is an eligible player who is not a member, and the player is in no group, or leads (or assists in a raid) |
| dismiss | (PA, 0x05993B0C) | the player leads (or assists in a raid), the target is a member, not the leader |
| transfer leadership | (PA, 0x0E3B2F5C) | the player leads and the target is a member |
| make / remove raid assistant | (PA, 0x086689BC) / (PA, 0x0DABB9EC) | raid, the player leads |

The `FellowAssist` button sets the player's target to the member's target (an assist action of
the session; BIN shows only the hand-off).

<a id="s4-6"></a>
### 4.6 Leader change, removal, disbanding (BIN element changes; Probable notices)

**Elements.** On a membership change the panel rebuilds: it destroys its rows, then creates one
row per member other than the player (§4.3) and sets each row's leader marker from whether
that member leads; `PlayerLeaderIcon` follows the same rule for the player (BIN). A removed
member's row disappears with the rebuild. When the group ends the rows are destroyed and
panels 2–4 hidden; panel 1 stays with an empty list, so nothing of it is seen in normal play
(its drag-box frame is hidden, GP §4); when a group forms the display is shown (BIN: the show
and clear handlers; the events that call them Probable).

**Notices** (SP7, SP8: `Fellowship`-table entries the original receives as session lines;
argument 0x52615 is a name):

| Role | (table, token) |
|---|---|
| you created a fellowship | (FT, 0x024583E7) |
| you joined a fellowship | (FT, 0x0CDFFC77) |
| *name* joined your fellowship | (FT, 0x047E6917) |
| *name* left your fellowship | (FT, 0x0C75F607) |
| *name* was dismissed from your fellowship | (FT, 0x09284BE7) |
| you dismissed *name* | (FT, 0x08AD4673) |
| you were dismissed | (FT, 0x0210C594) |
| you left your fellowship | (FT, 0x03073954) |
| your fellowship was disbanded | (FT, 0x018CCD74) |
| you are now the leader | (FT, 0x043F25A6) |
| *name* is now the leader | (FT, 0x03FD5682) |
| *name* is no longer the leader (gendered) | (FT, 0x08D678F2) |
| *name* went link-dead | (FT, 0x051B5697) |
| *name* was defeated | (FT, 0x024FA334) |
| *name* asks you to join a fellowship | (FT, 0x089728CE) |
| you accepted / declined an invitation | (FT, 0x0DA3CC14) / (FT, 0x071D2A95) |
| *name* declined to join | (FT, 0x0960A613) |
| the invitation expired | (FT, 0x030A0D94) |
| attempting to invite *name* | (FT, 0x078A39C5) |
| the fellowship is full | (FT, 0x09866C97) |

The table holds raid and monster-play ("group") variants of most rows and refusal lines
(leader required, not in a fellowship, already in one, …); 144 entries in all.

> **OPEN (Q14):** which of these lines the original shows for the player's own actions (for
> example both "you dismissed" and "was dismissed"), and their order relative to the panel
> update. **Tessera rule:** §9 TR4.

---

<a id="s5"></a>
## 5. Fellowship invitation prompt

<a id="s5-1"></a>
### 5.1 The window (Confirmed, SP15)

`FellowInvite` (0x1000168B, layout `fellowinvite`): `UIScaler`, design rectangle
(62, 86, 354 × 431), scaling group `Toolbar` (6), origin (177, 215), saved location, size not
saved, activatable, blocks clicks, auto-fade 0.1. Children:

| Element | Rect | Content |
|---|---|---|
| `FellowInvite_TitleBar` (0x100016A2) and its child Text `FellowInvite_TitleBar_Label` (0x100016A4) | (57, −7, 240 × 45) / (0, 15, 240 × 20) | the label holds the title; layout default (UI, 0x03045B45) |
| `FellowInvite_InviteText` | (20, 38, 316 × 63) | the invitation text |
| `FellowInvite_FellowList` (0x10001695) | ListBox (21, 104, 313 × 220) with `FellowInvite_Scrollbar` | one `FellowInvite_Fellow` row per member |
| `FellowInvite_WarningText` (0x10002EA7) | (20, 328, 316 × 67) | a warning, when given |
| `FellowInvite_AcceptButton` (0x100016A6) | text button (67, 401, 100 × 20), text (UI, 0x0F70211E) | accept |
| `FellowInvite_DeclineButton` (0x100016A7) | (187, 401), (UI, 0x0A76F86E) | decline |
| `FellowInvite_CloseButton` (0x1000168E) | (334, 25, 16 × 16), tooltip (UI, 0x08921EF4) | decline |
| `dragbar` | (49, 6, 255 × 24), tooltip (UI, 0x06D77F5C) | move |

Row template `FellowInvite_Fellow` (0x10001696, top level of the same layout, 296 × 39):
`FellowInvite_FellowClass` (ClassDisplay 25 × 25: class icon), `FellowInvite_FellowLevel`
(0x10001698: the level, (UI, 0x038B9CDC) with argument 0x50AA9C = the level, SP28),
`FellowInvite_FellowName` (0x10001699: name), `FellowInvite_FellowArea` (0x1000169A: the
member's area name; cleared when unknown). The layout's own entries of these three texts
(table 0x25033E35) are design-time texts and are replaced (cleared when there is no value).

> **Answered (revision 1, 2026-10-11; was an implementer OPEN):** the title label is
> `FellowInvite_TitleBar_Label` (0x100016A4), a child of `FellowInvite_TitleBar`.

<a id="s5-2"></a>
### 5.2 Behaviour (BIN, SP15)

- **Invitation.** When an invitation arrives and no other invitation or request is pending,
  the window takes the inviting group's members, one row each in the group's order, sets the
  title and the invitation text from the texts that come with the invitation (each only when
  non-empty), sets or clears the warning text, and shows itself. While one invitation is
  pending a second is not shown. The invitation text the original receives is a
  `Fellowship`-table line such as (FT, 0x089728CE), or, when joining would close a private
  encounter, one of the variants (FT, 0x0EA457D5), (FT, 0x0A458BA5) (Probable: the text is
  server-supplied).
- **Request to join.** The same window serves the reverse case, a player asking to join the
  player's group: one row (the requester), title and text from the request.
- **Accept** answers the pending invitation (or request) with yes, clears it, empties the list
  and hides the window. **Decline** and **close** answer no and hide it.
- **Timeout:** the window has no timer of its own. Expiry is the server's: the original
  receives (FT, 0x030A0D94) and the window is closed by the session (Probable).

> **OPEN (Q13):** whether the expiry closes the window through a session message or through
> another client path (not traced). **Tessera rule:** no timeout (§9 TR7).

<a id="s5-3"></a>
### 5.3 Keys

The window is an ordinary activatable panel, not an engine confirmation dialog, so the dialog
keys of UB §5.5 do not apply to it (BIN: its handler acts only on the three button clicks).

<a id="s5-4"></a>
### 5.4 The `Dialog_Fellowship*` markers (BIN, SP16)

GC §3.2 lists `Dialog_FellowRecruitConfirmation`, `Dialog_FellowshipMakerConfirmationDialogCallback`
and `Dialog_FellowshipVoteDialogCallback`. None of them is the invitation prompt, which is the
window above. By their labels they belong to recruiting from inside an encounter, the
fellowship maker and a group vote; their use stays GC's OPEN Q2.

---

<a id="s6"></a>
## 6. Trade window

<a id="s6-1"></a>
### 6.1 Layout (Confirmed, SP17)

`SecureTradeField` (0x10000573, layout `securetrade`): `UIScaler`, design rectangle
(254, 118, 518 × 388), scaling group `Panels` (31), origin (259, 194), saved location,
activates on show, activatable, blocks clicks, auto-fade 0.1. Title (UST, 0x0DB7B2AC) on
`TradeWindow_MoveBar`; drag bar tooltip (UI, 0x040EA354); close button `SecureTradeCloseButton`
(494, 23), tooltip (UI, 0x08921EF4).

| Side | Element | Children |
|---|---|---|
| own (left) | `SelfTrade_Field` (7, 39, 252 × 388) | label `SelfTrade_Label` (UST, 0x0D63B6DC); `SelfTrade_TotalMoneyField` (0, 20): text (UST, 0x02251404) and `SelfTrade_TotalMoney` (0x10000581, PlayerMoneyDisplay (83, 5, 160 × 22): the player's money as coin elements, §6.3); `SelfTrade_TradeField` (0, 48, 252 × 264; states `normal` image 0x41007F84, `highlight` image 0x41007F83): `SelfTrade_ItemList` (ListBox 226 × 222, one column, container, item display type Medium) with `SelfTrade_ItemList_SB`, and `SelfTrade_MoneyDisplayField` (44, 235): three coin entries; buttons `SelfTrade_TradeButton` (31, 316, 96 × 20; text (UST, 0x0A307BC4); boolean button) and `SelfTrade_CancelButton` (130, 316; (UST, 0x0190ED84)) |
| peer (right) | `OtherTrade_Field` (258, 38, 252 × 388) | label `OtherTrade_Label` (no layout text); `OtherTrade_TradeField` (0, 48; the same two states and images): `OtherTrade_ItemList` (ListBox 228 × 225, one column, Medium) with its scroll bar, and `OtherTrade_MoneyDisplay` (0x1000058B, MoneyDisplay (45, 236, 160 × 22): the peer's offered coin as coin elements, §6.3) |
| both | `SecureTrade_WaitOverlay` (4, 85, 508 × 266; image 0x41007F82 and a dark colour layer; blocks clicks) | `SecureTrade_WaitOverlayText`: (UST, 0x07E3F5E4) |

**Coin entries** (`MoneyDisplay_Entry_GoldField`, `…SilverField`, `…CopperField`, at
x = 0, 58, 111 in that order): each a field tagged `UI_SecureTrade_Currency_Type` = the currency
game object (0x70000218 gold, 0x70000219 silver, 0x7000021A copper here; EC §7: the
`GoldCurrency`, `SilverCurrency`, `CopperCurrency` records), with an editable one-line text
of at most 5 characters (no IME) and a label with a tooltip (UI, 0x0135BA5C),
(UI, 0x01E4A5BC), (UI, 0x0BE07C7C). The editable texts are `MoneyDisplay_Entry_GoldEntry`
(0x10000127), `MoneyDisplay_Entry_SilverEntry` (0x10000123) and `MoneyDisplay_Entry_CopperEntry`
(0x1000011F); the labels carrying the tooltips are `…GoldLabel` (0x10000126), `…SilverLabel`
(0x10000122), `…CopperLabel` (0x1000011E); each field also holds a background field. The trade
code names exactly these three entry IDs (SP31).

**Money displays** (SP31). `SelfTrade_TotalMoney` and `OtherTrade_MoneyDisplay` have no layout
children and no text of their own: each is a horizontal list of one row and at most three
columns (only visible items laid out, filled in reverse) whose coin element is
`MoneyDisplay_CoinTemplate` (0x10000114) of layout `moneydisplay` (0x2200006C), with
`UIComponent_MoneyDisplay_ForceShowAll` true on both and `…_SuppressLeadingZeroes` true on the
peer's. They are the money display of `npc-windows.md` §9.2–§9.3 (Accepted on `main`; it
reaches this branch with the merge of `main`), summarised here so that the trade window can be
built on this branch: the display creates three coin elements from its template, with the
icon (`MoneyDisplay_Icon`, inside `MoneyDisplay_Icon_Holder`) in states `Copper`, `Silver`,
`Gold` in that order, appended, so that with the reverse fill they read gold, silver, copper
from left to right; each coin's count is the text `MoneyDisplay_Text` (0x10000115), set through
(UI, 0x07480C44) with argument 0x5242A9 = the count; the amount is split by `Currency_Value`
largest first (EC §7, npc-windows §9.3); which coins are visible follows npc-windows §9.2's
Tessera rule (with `ForceShowAll` every coin shows; with `SuppressLeadingZeroes` as well, a zero
coin shows only after a larger non-zero coin, and an amount of 0 shows only the copper coin with
0). Setting a display's own text shows nothing.

<a id="s6-2"></a>
### 6.2 Item entries (Confirmed layout; BIN)

There is no slot grid: each side is a one-column list of item displays. For each offered item
the game code creates, from layout `itemdisplay`, the template chosen by the list's
`UI_Item_DisplayType` (Medium → `ItemDisplay_MediumTemplate` 0x100000F4, 200 × 32: a 32 × 32
`ItemIconDisplay` and a 165-pixel name text; Long → `…LongTemplate`, Short →
`…ShortTemplate`), binds it to the item and its owner, marks whether it is the player's side,
and appends it; an item already in the list is not added again. The icon is drawn by IL
(item instance, §3.1 step 1 there) and the tooltip by IT, as for any item display (CP §5).
The number of entries per side is not limited by the data (Q8).

> **OPEN (Q8):** the limit of entries per side (a server rule; no layout or client limit was
> found). **Tessera rule:** the feed's limit of 32 entries per side (SF §6.3).

**Parts of an entry** (SP30). In `ItemDisplay_MediumTemplate` the parts are found by ID, never
by type order:

| Part | Element (ID) | Content |
|---|---|---|
| icon | `ItemDisplay_Icon` (0x100000E4, `ItemIconDisplay`, 32 × 32, inside `ItemDisplay_ShortIcon` 0x100000F3) | the item's icon (IL), its examination tooltip (IT), CP §5 |
| quantity | `ItemDisplay_QuantityOverlay` (0x100000E7, a Text inside the icon, hidden by the layout) | the stack quantity, by CP §5's Tessera rule (shown when greater than 1) |
| name | `ItemDisplay_Name` (0x100000F5, Text at (35, 0) 165 × 32, left, centred vertically, truncated with (UI, 0x037DDBC3)) | the item's name |

`ItemDisplay_QuantityOverlay` is a Text that comes **before** `ItemDisplay_Name` in tree order,
so "the entry's first Text element" is the quantity overlay, not the name.

> **Tessera rule (name colour):** the name text takes the state of `item-tooltips.md` §6.2 for
> the item's `Item_Quality` (`ItemQuality_Legendary_ColorName` … `ItemQuality_Common_ColorName`;
> any other value `Object_ColorName`); all six states exist on `ItemDisplay_Name` (SP30). Which
> state the original's item display requests was not traced (Probable: the same mapping). The
> colour drawn is the one colour of the state's `UICore_Text_font_colors` (SP36); how Tessera
> applies it is §9.1 (e).

> **Answered (revision 1, 2026-10-11; was an implementer OPEN, with GP §11's OPEN for these
> two types):** name `ItemDisplay_Name` (0x100000F5); coin entries 0x10000127, 0x10000123,
> 0x1000011F (§6.1); the money displays are coin-element lists (§6.1 "Money displays").

<a id="s6-3"></a>
### 6.3 Coin (BIN, SP18)

- The own wallet shows in `SelfTrade_TotalMoney`; the peer's offered coin in
  `OtherTrade_MoneyDisplay` (set from the peer's offer; 0 at opening); both by the money
  display of §6.1.
- When a coin entry loses focus or receives Enter (BIN, SP32), only **that** entry is tested
  and cleared: its text not a number posts (UST, 0x085F80B9) and clears it; otherwise a total
  of the three entries (Σ entry × the currency's `Currency_Value`, EC §7; Probable for the
  arithmetic) above the player's money posts (UST, 0x06195A09) and clears it; otherwise a
  non-zero total that a further client check refuses posts that check's client message and
  clears it. The notices go to chat as type 1. In every case the handler then reads the three
  entries and sends the amounts present as the coin offer (per currency), even when the other
  two entries alone still exceed the player's money (the session decides).

> **Answered (revision 1, 2026-10-11; was an implementer OPEN):** the reading is right: only
> the committed entry is cleared, and the amounts present are sent afterwards (SP32).

<a id="s6-4"></a>
### 6.4 Accept states and resets (BIN, SP18)

| Event | Display |
|---|---|
| the window opens | both side fields `normal`; trade button not highlighted; peer label set (below) |
| a side accepts | that side's `…TradeField` → `highlight` |
| a side withdraws its acceptance | that side's field → `normal` |
| any offer content changes (items or coin, either side) | both fields → `normal`; trade button not highlighted |
| both offers empty (no items, no coin, no coin typed) | trade button ghosted (`UICore_Button_ghosted`); otherwise not ghosted |

The trade button is a boolean button: clicking it toggles its highlight; turning it on accepts,
turning it off withdraws. The peer label: the template (UST, 0x0CFA21C2) with argument
0x52615 = the peer's name, or cleared when no name is known.

<a id="s6-5"></a>
### 6.5 Opening (BIN, SP18)

- A trade begun by the peer first shows an engine confirmation dialog (UB §5.5) with the
  question (UI, 0x07D4E2D4), argument 0x7C83B5 = the peer's name; "yes" accepts the request.
  A trade begun by the player opens the window directly.
- The player begins a trade from the selection's player menu; the label is
  (PA, 0x06EB2E9C) (Probable: that entry is built by game scripts, not by client code).
- No built-in command opens a trade: the `Commands` entries with the trade word are the trade
  chat channel (CW §6.5).

<a id="s6-6"></a>
### 6.6 Drops, cancel, close (BIN, SP18)

- **Dropping an item** onto `SelfTrade_ItemList`: accepted only for an item of the player's
  own containers; an equipped item is refused with one message, an item of another owner with
  another (client messages by ID, Q9); otherwise the item and its quantity are offered.
- **Cancel** and the **close button** cancel the trade; when the peer has already ended it,
  they only hide the window. **The trade ends** (completed or cancelled by either side): the
  window hides; when the peer ended it, it hides at the next frame.

> **OPEN (Q9):** the texts of the two refusal messages for drops (client message IDs
> 0x1000031B and 0x10000038 here, shown by a message routine that was not followed).
> **Tessera rule:** §9 TR9 (own words, type 1).

> **OPEN (Q11):** how an offered item is taken back (no handler on the own list other than the
> drop was found). **Tessera rule:** no removal; the player cancels the trade.

<a id="s6-7"></a>
### 6.7 Trade notices (Confirmed data, SP7; Probable, SP8)

| Role | (table, token) |
|---|---|
| *name* wants to trade with you | (TT, 0x0E9D97E5) |
| trading … (in progress) | (TT, 0x00189D23) |
| the trade finished | (TT, 0x05D3ED34) |
| you changed the offer | (TT, 0x0DBCCC74) |
| the partner changed the offer | (TT, 0x08370F54) |
| not enough room (you / the partner) | (TT, 0x0E143ABB) / (TT, 0x0E02B18B) |
| the trade was interrupted by a logout | (TT, 0x072E2414) |
| refusals (busy, too far, not a player, already trading, cannot trade that, …) | the other 26 entries |

---

<a id="s7"></a>
## 7. Commands and key-map commands

<a id="s7-1"></a>
### 7.1 Built-in command names (Confirmed data; SP21)

All are `Commands`-table name lists (CW §8.1), registered by game scripts in the original;
none is referenced by client code. Roles by their help entries in `CommandHelp` where one
exists:

| Role | `Commands` token(s) | Help (CH) | Arguments |
|---|---|---|---|
| friends list: add, remove, list | 0x03D8B7E4 | 0x0D8B7E41 (usage), 0x0D8B7E42 | a sub-word, then a name |
| ignore list: add, remove, list | 0x03FE1B25 | 0x0FE1B2B1, 0x0FE1B2B2 | a sub-word, then a name |
| invite to the fellowship (two names) | 0x0F3108C5 | 0x03108C71, 0x03108C72 | a name |
| leave the group | 0x05A63F75 | — | — |
| disband | 0x0F8E33D4, 0x05B57444 | — | — |
| dismiss a member | 0x0F8F9533 | — | a name |
| promote (make leader) | 0x094AD2D5, 0x03139345 | — | a name |
| raid (create, disband, invite, dismiss, promote, demote, locks, swap, leave) | 0x07B59EB4 | 0x0A487BA1, 0x0A487BA2 | sub-word |
| ready check | 0x067D60AB | 0x0743E381, 0x0743E382 | — |
| assist | 0x0E2F0C44 | — | — |

The sub-words "add", "remove" and "list" are `Commands` entries too (several tokens each:
"add" 0x07EBEF14, 0x0B2AC9B4, 0x0B530F84; "remove" 0x070A93F5, 0x09015875, 0x0B6B9465;
"list" 0x0531B214, 0x02ADD114, 0x06A88324, 0x0845C0E4, 0x0EBFBBD4).

> **OPEN (Q5):** which of each pair (disband, promote) is the top-level fellowship command and
> which a raid sub-word, and which sub-word token belongs to the friend and ignore commands
> (script-registered, CW Q15). **Tessera rule:** §9 TR11.

<a id="s7-2"></a>
### 7.2 Key-map commands (BIN, SP19; Confirmed, SP20)

| Input command (ID here) | KM §12.11 class | Default binding (0x1D000000, section `Main`) | Effect |
|---|---|---|---|
| `ToggleSocialPanel` (0x10000035) | 15 panels | O | Social panel (§3.1) |
| `ToggleSocialFriendsPanel` (0x100001C5), `…FellowingPanel` (0x100001C3), `…KinshipPanel` (0x100001C1), `…RaidPanel` (0x100001C4), `…GroupStagePanel` (0x100001C2) | 15 | none | Social panel at a page (§3.1) |
| `SelectFellowOne` … `SelectFellowSix` | 11 targeting | F1 … F6 | target group member 1 … 6 |
| `AssistFellowTwo` … `AssistFellowSix` | 11 | Shift + F2 … F6 (mask bit 1) | assist group member 2 … 6 |
| `ToggleAssistancePanel` | 15 | F7 (and F1 in `EditControls`) | not social |

There is no `AssistFellowOne`: member 1 is the player (Probable, Q6).

> **OPEN (Q6):** whether `SelectFellowOne` targets the player and `SelectFellowN` (N ≥ 2) the
> (N − 1)-th row in panel order (inferred from the missing `AssistFellowOne` and the F1
> binding). **Tessera rule:** that mapping (§8.1).

---

<a id="s8"></a>
## 8. Mapping to the session layer (SF §9.2, §9.4)

<a id="s8-1"></a>
### 8.1 User actions → commands (Tessera rule)

| Action (original element or command) | Command (SF §9.4) |
|---|---|
| Friends: Add dialog answered; `/friend add NAME`; player menu "add to friends" | `AddFriend { name }` |
| Friends: Remove button, row menu remove; `/friend remove NAME` | `RemoveFriend { name }` |
| Friends: Invite button, row menu invite; `/invite NAME`; player menu invite (§4.5) | `InviteToFellowship { name }` |
| Friends row menu: ignore; Ignore page Add dialog; `/ignore add NAME` | `Ignore { name }` |
| Ignore page Remove, row menu remove; `/ignore remove NAME` | `Unignore { name }` |
| Friends Tell button, double click, row menu tell | none; opens the chat entry in tell mode (CW §10 item 12); the typed line becomes `Chat` on `Tell` |
| Fellowship: player menu dismiss; `/dismiss NAME` | `KickFromFellowship { name }` |
| Fellowship: transfer leadership; `/promote NAME` | `SetFellowshipLeader { name }` |
| Fellowship: leave; `/leave` | `LeaveFellowship` |
| Fellowship: the disband names of §7.1 (TR11) | `DisbandFellowship` (decision 0035; SF "P4 social") |
| Fellowship row click | `SelectTarget { target: Some(member entity) }` when the member's entity is known (§9 TR3), else the §4.4 notice |
| `FellowAssist`, `AssistFellowN` | `SelectTarget { target }` with the member's last reported target (`TargetChanged`), if any |
| `SelectFellowN` | `SelectTarget`: N = 1 the player, N ≥ 2 the (N − 1)-th row |
| Invitation Accept | `AnswerInvite { accept: true }` |
| Invitation Decline, close | `AnswerInvite { accept: false }` |
| Player menu "start secure trade" on an entity | `OpenTrade { entity }` |
| Trade: item dropped on the own list | `OfferTrade { window, item: Some(id), quantity: the stack's quantity, coin: the entered coin total }` |
| Trade: coin entry committed (§6.3 valid) | `OfferTrade { window, item: None, quantity: 0, coin: total }` |
| Trade button on / off | `SetTradeReady { window, ready: true / false }` |
| Trade Cancel, close button | `CloseWindow { window }` |

> **Answered (Q12, revision 1):** decision 0035 and SF "P4 social" (both Accepted) add the
> command `DisbandFellowship`, answered only by scenario triggers; TR11 sends it.

<a id="s8-2"></a>
### 8.2 Events → displays (Tessera rule)

| Event (SF §9.2) | Display |
|---|---|
| `FriendList { entries }` | the friends rows replaced: one row per entry (name, online colour), §9 TR5 |
| `IgnoreList { names }` | the ignore rows replaced |
| `FellowshipChanged { members }` | §9 TR2–TR4: rows of panel 1, leader markers, `PlayerLeaderIcon`, panel shown or hidden, derived notices (TR4), plugin events (LPA §15.4) |
| `FellowshipInvite { from }` | §9 TR7: the invitation window |
| `TradeShown { window, peer }` | the trade window opens for `window` (§6.4 "opens") |
| `TradeOffer { window, side, entries, coin }` | that side's list replaced by the entries, its coin display (peer) or entries' shown coin (own) set; then the §6.4 content-change reset |
| `TradeReady { window, side, ready }` | that side's field `highlight` / `normal`; for `Own` also the trade button's highlight |
| `WindowClosed { window }` of the trade window | the window hides |
| `VitalChanged`, `LevelChanged`, `Defeated`, `Revived`, `EffectApplied`/`Removed`, `TargetChanged` of a member's entity | that member's row: meters (TR15), level, defeated field (TR14), effects (TR16), assist target |
| `FellowVitalChanged { member, vital, current, max }` | the row of the member of that name, with or without an entity: its meter (TR3, TR15) |
| `ChannelJoined`/`Left`, `ChatMessage` | chat (CW), not these panels |

---

<a id="s9"></a>
## 9. Tessera rules for the sandbox

These are Tessera's rules (DEC), not facts about the original.

- **TR1 Panels exist without data.** P8, the four fellowship panels, the invitation window and
  the trade window are built from the install's layouts by UL §17.1 whether or not the feed
  sends data; the fellowship panels and the invitation and trade windows start hidden.
- **TR2 Members.** The current fellowship is the member list of the last `FellowshipChanged`
  (initially empty). The player is the member whose `entity` equals the player's entity, or,
  when no member has it, the first member whose `name` equals the player's character name. A
  non-empty list that does not contain the player is treated as if the player were appended at
  its end (one warning per run). Rows: every member other than the player, in list order, at
  most 5 (further members are not shown, one warning); template `FellowMiniTemplate`. The
  leader is the first member with `leader = true` (none: no marker shown). `PlayerLeaderIcon`
  shows when the player leads; the marker state is `fellowship` (no raids or monster play in
  the sandbox). Panel 1 is shown while the list is non-empty and hidden otherwise; panels 2–4
  stay hidden.
- **TR3 Member data.** A member with an `entity` takes its name from the event, and level,
  defeat and effects from the last `LevelChanged`, `Defeated`/`Revived` and
  `EffectApplied`/`EffectRemoved` the feed sent for that entity (kept after the entity
  despawns, SF §5.4). **Morale and power** of every member, with or without an entity, are per
  vital the later of the member's last `FellowVitalChanged` (matched to the member by name,
  ignoring case) and, for a member with an entity, that entity's last `VitalChanged`
  (decision 0035; SF §6.7). Without any such event a meter shows nothing (position 0, TR15);
  without a level the level text is empty; no effects. A member without an entity has no event
  for level, defeat or effects (SF "P4 social" OPEN), so those stay empty. The class icon is
  shown only when an actor's class is known to the client half (the feed's `CharacterInfo`
  gives one for the player only), so it is normally empty. Link-dead is never shown. A click
  on a member without an entity, or whose entity is despawned, posts (FT, 0x0571E4C5) with
  argument 0x4AA1147 = the member's name (SP33) as a local line of chat type 1 (Tessera's
  choice; the original's chat type for it was not traced) and sends nothing.
- **TR4 Derived notices.** The sandbox has no server to send §4.6's lines, so the client half
  derives them from consecutive `FellowshipChanged` events, comparing members by name, and
  posts them as session-like lines of chat type `Fellowship` (11) in this order: (a) list
  becomes empty: (FT, 0x03073954) when the player sent `LeaveFellowship` since the previous
  event, else (FT, 0x018CCD74); stop. (b) list becomes non-empty: (FT, 0x024583E7) when the
  player leads, else (FT, 0x0CDFFC77); stop. (c) otherwise, for each removed member in old-list
  order (FT, 0x09284BE7) when the player sent `KickFromFellowship` for that name since the
  previous event, else (FT, 0x0C75F607); then for each added member in new-list order
  (FT, 0x047E6917); then, when the leader changed, (FT, 0x043F25A6) if the player is the new
  leader, else (FT, 0x03FD5682) with the new leader's name. The text is read from the install;
  without an install no line is posted. On the test-server backend no `FellowshipChanged`
  arrives (SF §9.5), so nothing is derived.
- **TR5 Friends rows.** From `FriendList`: name and online colour as §3.3; level, class, area:
  the unknown text; last online: (UF, 0x0FF052F5) through its template when online, the
  unknown text when offline; grouped marker `fellowship_fellowed` when online and the name is
  a member of the current fellowship, else `fellowship_unknown`; no note. Rows sort by §3.4
  (default Member, ascending); the selection is kept by name across a replacement. The Note
  buttons and the menu's note, auto-fellow and away entries are ghosted (no feed field
  carries them, Q10). Friends notices of §3.7 are not derived: they arrive as `system` steps
  of a scenario when wanted.
- **TR6 Ignore rows.** From `IgnoreList`, sorted by name ascending (Q7); notes as TR5.
- **TR7 Invitation.** `FellowshipInvite { from }` shows the window with title (UI, 0x03045B45),
  the invitation text (FT, 0x089728CE) with argument 0x52615 = `from`, no warning, and one row:
  `from` (no class, level or area), since the event names no members. A second
  `FellowshipInvite` while one is shown replaces its text and row. Accept and Decline send
  `AnswerInvite` and hide the window; the feed decides what follows (a `FellowshipChanged`, or
  nothing). There is no timeout.
- **TR8 Other pages.** Grouping, kinship, raid and raid-lock pages show their layout with empty
  lists; their buttons are ghosted; nothing is sent.
- **TR9 Trade.** One trade window at a time (SF §5.6). On `TradeShown` the window opens without
  the request dialog (the feed has no request event); the peer name is the name of the peer's
  entity as last spawned, else cleared. Own-side entries show `TradeOffer { side: Own }` content
  (the feed is the authority; a drop sends `OfferTrade` and shows nothing until the feed
  answers). Each entry's name goes into `ItemDisplay_Name` with the name-colour rule of §6.2,
  its icon into `ItemDisplay_Icon`, its quantity into `ItemDisplay_QuantityOverlay` (§6.2).
  **Literal entries** (SF §5.2): the literal text in `ItemDisplay_Name` in state
  `Object_ColorName`; **no icon**, meaning that `ItemDisplay_Icon` draws no item image (the
  "no icon" result of IL §3.1; the template's own art stays) and no stand-in image is drawn; the
  quantity overlay is hidden; there is no examination tooltip (no item to examine). The coin:
  `SelfTrade_TotalMoney` shows the player's money, the amount of the last
  `WalletChanged { currency: coin }` (0 before any); `OtherTrade_MoneyDisplay` shows the last
  peer `TradeOffer` coin (0 at opening); both by the money display of §6.1. The own side's
  three entries (`MoneyDisplay_Entry_GoldEntry`, `…SilverEntry`, `…CopperEntry`) show the last
  own `TradeOffer` coin, split by `Currency_Value` (EC §7) into gold, silver and copper, written
  as plain integers (an entry with 0 left empty). The §6.4 content-change reset runs on every
  `TradeOffer`. A drop of an item not in the player's backpack is refused locally with a type 1
  line in Tessera's own words. No item removal from an offer (Q11).
- **TR10 Ready state.** `TradeReady { side: Own, ready }` sets the button highlight to `ready`;
  clicking the button sends `SetTradeReady` with the new highlight and shows it at once
  (optimistic), a later `TradeReady` overriding it.
- **TR11 Commands.** The client dispatcher (CW §10 item 3) registers, besides the names CW §10
  already registers, every name of the `Commands` entries 0x03D8B7E4 (friend), 0x03FE1B25
  (ignore), 0x0F3108C5 (invite), 0x05A63F75 (leave), 0x0F8F9533 (dismiss), 0x094AD2D5 and
  0x03139345 (promote), read from the install, plus the Tessera names `friend`, `ignore`,
  `invite`, `leave`, `dismiss`, `promote`. Sub-words of `friend` and `ignore` are matched
  case-insensitively against `add`, `remove`, `list` and against every name of the sub-word
  tokens of §7.1. `list` prints the model: the empty line ((CT, 0x06577433) or
  (CT, 0x01785864)) or, for ignore, the header (CT, 0x043D716F) then one line per name; for
  friends one line per entry, the name followed by (US, 0x0BE609B3) or (US, 0x0F902393). The
  disband names (0x0F8E33D4, 0x05B57444, read from the install, plus the Tessera name
  `disband`) are registered and send `DisbandFellowship` (decision 0035; SF "P4 social"); the
  feed's triggers answer it, and nothing is shown until they do. A name argument missing gives
  a type 1 line. All lines are type 8 (lists) or type 1 (refusals).
- **TR12 Keys.** The client maps `ToggleSocialPanel` and the five page commands to §3.1's
  Tessera rule, `SelectFellowOne` … `Six` and `AssistFellowTwo` … `Six` to §8.1. Tessera's own
  default key map binds `toggle_social` to O and leaves `select_fellow_1` … `6` and
  `assist_fellow_2` … `6` unbound, because F3 is Tessera's debug key in the `global` context
  and a binding there would conflict; the install's bindings (F1 … F6, Shift + F2 … F6,
  SP20) arrive through the key-map import (KM §12.14) when the user imports them.
- **TR13 Other backends.** On the test-server backend (SF §9.5, §9.8): at protocol minor 2 or
  higher with `CHAT_CHANNELS`, `JoinChannel`, `LeaveChannel` and `Chat` on `Named` are
  accepted; at every minor, `Chat` on `Fellowship` and every other Social command
  (`AddFriend`, `RemoveFriend`, `Ignore`, `Unignore`, the fellowship commands,
  `DisbandFellowship`, `AnswerInvite`, the trade commands) is refused with `InvalidCommand`,
  so the panels of this spec stay empty there; the buttons are not ghosted for that reason
  (a refusal posts the session's error line). The plain sandbox without a scenario accepts
  and ignores them (SF §9.5, §9.8). Overlay runs keep their P1 subset (decision 0035 D3).
- **TR14 Voice, status and ready-check elements.** Tessera has no voice chat and no ready
  check. On every member row, `FellowTemplateVoiceChat`, `FellowTemplateVoiceChatSquelched`,
  `FellowTemplateSendingData` and `FellowTemplateVolumeControl` are hidden, and so are, on
  panel P4, `PlayerVoiceChat`, `PlayerVoiceChatSquelched`, `PlayerSendingData` and
  `PlayerAssistantIcon`, whenever the row or the panel is built (§4.2, §4.3; the original shows
  them only with voice active). `FellowTemplateVolumeControl` is kept hidden by setting
  `UICore_Scrollbar_disabled` true and `UICore_Scrollbar_hide_when_disabled` true on it when
  the row is built, so that every later layout of the bar keeps it hidden (`ui-widgets.md`
  §6.4; SP35; §9.1 (f)). `FellowReadyCheckTemplate` gets no state request.
  `FellowTemplateLinkDead` is hidden; `FellowTemplateDefeated` is shown while the member is
  defeated (TR3) and hidden otherwise; `FellowTemplateLeader` follows TR2.
- **TR15 Member meters.** For a member's morale (TR3), the meter named by the `Health` entry
  of `VitalHealthField`'s `UI_VitalDisplay_Array` (`VitalCurrentHealthMeter`) takes
  `UICore_Meter_position` = current ÷ max clamped to [0, 1] (0 when max is 0); for power, the
  `Power` entry's meter (`VitalCurrentPowerMeter`) likewise. The temporary and clamped (dread)
  meters stay at position 0, the vital texts stay hidden, and every other `Meter` below the
  displays (backgrounds, `VitalHealthParentField`) is left as the layout has it.
  `VitalHealthFieldBg` gets no state request and keeps its layout default state `normal` (the
  plain background, SP34): the feed carries no vital-display override (revision 2; review 2
  note (c)). The pip entry's
  field `RageMeter` stays hidden and the power field shown: `Resource` vitals are not shown on
  member rows in P4. This is the member form of `combat-ui.md` TR10; when `combat-ui.md` reaches
  this branch, its §4.3 rules apply to these displays except where this rule differs.
- **TR16 Member effects.** `FellowEffectDisplay` shows the member's effects (TR3) by
  `combat-ui.md` §8.1–§8.2 and TR29–TR31 (branch `p2`, Accepted), with this list's own limits:
  2 rows, at most 6 icons per row (`UIComponent_EffectDisplay_MaxIconColumns` 6), icon size the
  list's `EffectDisplayList_EffectSize` (20 when absent, as here); further icons are kept but
  not shown. (The interim of revision 1, no icons until `combat-ui.md` was on the same branch,
  ended when TR16 was built on `main`, 2026-10-14.) Revision 3 (2026-10-23) adds:
  - **TR16a Binding.** Each member row's `FellowEffectDisplay` is one effect list of the
    combat UI (`combat-ui.md` §8), bound to that row and to the member's entity, filled from
    the effects the client keeps for that entity and then following its `EffectApplied` and
    `EffectRemoved`. A member without an entity has no list (the display stays empty). When a
    row comes to show another member, its list is rebound; the binding may reach the lists
    one frame after the rows change.
  - **TR16b Icons per row.** A row shows `UIComponent_EffectDisplay_MaxIconColumns` icons
    (6 here), 6 when the list lacks the property. Rows and icons are placed by
    `combat-ui.md` TR30a: two rows *e* pixels apart from the list's top-left corner, so with
    *e* = 20 the second row reaches 3 pixels below the 37-pixel list box; it is drawn, not
    clipped (the original's own row layout inside the list was not traced, as TR30a says).
  - **TR16c Pulse.** The combat-only pulse of `combat-ui.md` §8.2 follows the **local
    player's** combat state (`Combat_Violent`), as on the player's and the target's effect
    bars (E8, E9): the member lists are drawn in the local player's interface.
  - **TR16d Tooltips.** Member icons carry the tooltip properties of `combat-ui.md` §8.1;
    hovering one shows the effect tooltip of `skill-tooltips.md` §12.2 for the member's
    effect instance.

<a id="s9-1"></a>
### 9.1 Interim client choices (Tessera rules)

Answers to the implementer's engineering-choices OPEN (revision 1; they agree with what was
built):

- **(a)** The client has no engine text-input dialog (UB §5.5) or context-menu widget yet:
  the add dialogs of §3.5 and §3.6 post a type 8 line naming `/friend add NAME` and
  `/ignore add NAME`, and the row menus (§3.5, §3.6, §4.5) are not shown; the fellowship
  commands `/dismiss`, `/promote`, `/leave` and the disband names are the player's way
  meanwhile. When the widgets exist, §3.5, §3.6 and §4.5 apply as written.
- **(b)** `--open-panel` takes `social`, `friends`, `fellowship` and `trade`: the first three
  show P8 at its last, friends or grouping page; the fellowship display and the trade window
  show only while the feed reports a fellowship or a trade (TR2, TR9).
- **(c)** Escape hides P8 before the game-menu rule of GP §16 (P8 is an on-demand panel there).
- **(d)** A trade drop offers the item's whole stack (§8.1 "the stack's quantity"), and only
  from the backpack container (TR9).

Decisions of revision 2 (2026-10-14) on the implementer's items (1)–(5) of the note below
(each confirms what was built; none changes implemented behaviour):

- **(e) Name colour (item (1)).** Tessera's text path takes a text element's colour from
  `UICore_Text_font_color`. After every state change of `ItemDisplay_Name` the client writes
  the first entry of the element's effective `UICore_Text_font_colors` into its
  `UICore_Text_font_color` as a run-time property. In the data that entry is the state's only
  colour, no block of the element carries `UICore_Text_font_color` itself, and only code
  requests change the element's state (SP36), so in the trade window the write follows the
  §6.2 request. `string-table.md` §4.8.3 takes the item-quality colours from `ColorNameText`
  the same way (the first entry of a state's array). The general order of the colour sources
  (`fonts.md` §8.4, "Not covered here") is not decided here.
- **(f) Volume control (item (2)).** Kept as built and stated in TR14: the bar is disabled with
  `hide_when_disabled` true. Reason: `ui-widgets.md` §6.4 (answered 2026-10-14) shows that the
  bar's layout writes the element's one visibility, so a plain hide would last only until the
  bar's next layout. The original instead writes the visibility after the bar's last layout
  (SP35); without voice both give a hidden bar.
- **(g) Health background (item (3)).** Confirmed as TR15: `VitalHealthFieldBg` keeps its layout
  default state `normal`.
- **(h) Money display fallback (item (4)).** When a money display's coin element cannot be
  created from the install (layout `moneydisplay` or its `MoneyDisplay_CoinTemplate` missing,
  which a complete install never is, SP31), the display uses Tessera's fallback coin element,
  the same one the NPC windows' money displays use (one shared fallback: an element of the coin
  template's 53 × 22 holding the count text and a coin icon with one state per coin kind, drawn
  with Tessera's own art). The coin rule of §6.1 (three coins, reverse fill, the options) is
  unchanged.
- **(i) Portrait elements (item (5)).** Confirmed: the TR14 hiding of `PlayerVoiceChat`,
  `PlayerVoiceChatSquelched`, `PlayerSendingData` and `PlayerAssistantIcon` runs each time the
  client builds P4 and binds it, so a rebuilt P4 is hidden again; the original hides them in the
  panel's set-up (SP27).

Items (6) and (7) concern the plugin views and are answered in `lua-plugin-api.md` §15.4
(further rules 7 and 8, revision of 2026-10-14): item (6) is replaced (a member's effect list
gets the effect events of the entity it follows), item (7) is confirmed.

> **Implementer note** (conformance to revision 1 on `p4`, 2026-10-11; kept as the record of the
> choices; items (1)–(5) are answered by (e)–(i) above and items (6)–(7) as just said) Engineering
> choices made while building revision 1, for the next revision to confirm or replace:
> (1) the client's text shaper reads only `UICore_Text_font_color`, so after the §6.2 name
> state is requested the first entry of the name text's effective `UICore_Text_font_colors`
> is copied into `UICore_Text_font_color` (the same colour `string-table.md` reads from
> `ColorNameText`); (2) `FellowTemplateVolumeControl` is a scroll bar, which `ui-widgets.md`
> §6.4 step 6 shows again at its first layout, so TR14 keeps it hidden by setting it disabled
> with `hide_when_disabled` (`ui-widgets.md` §6.4 OPEN); (3) `VitalHealthFieldBg` keeps the
> layout's default state (review 2 note (c)); (4) a money display whose coin template cannot
> be created gets the fallback coin element of `npc-windows.md`'s implementation; (5) the
> P4 portrait's elements are hidden again whenever the client binds a newly built P4.
> Not yet built on `p4` although `combat-ui.md` has reached the branch with the merge of
> `main`: TR16's effect icons on member rows (the interim, no icons, still applies).
> Built since (2026-10-11): the `PartyMember` getters `GetTarget` and `GetEffects` follow
> §15.2 for the member's entity (`lua-plugin-api.md` §15.4 table row). Engineering choices
> for the next revision to confirm: (6) the member's `EffectList` is its own object (one per
> `PartyMember`, §15.2 "one per actor object"), reads the member's entity at each call and
> receives no `EffectAdded`/`EffectRemoved`/`EffectsCleared` of its own (§15.4: "`PartyMember`
> objects fire no events of their own"; the entity object's list gets them); (7) a member who
> left keeps the last entity seen, so its `GetTarget()` and list still follow that entity
> while the client knows it.

> **Answered (spec writer, 2026-10-23; revision 3):** choices (8)–(11) are confirmed as built
> and are now TR16a–TR16d of §9. No implemented behaviour changes. The implementer's note is
> kept as the record.
>
> (Was **OPEN:** implementer note, `crates/ui` and `apps/client`, 2026-10-14.) TR16 is now built
> with `combat-ui.md` on `main`, so the interim (no icons) no longer applies. Engineering
> choices for the next revision to confirm: (8) the member lists are effect lists of the
> combat widgets (`tessera-ui` `game::combat::CombatUi::set_member_lists`). Each list is
> bound to a row's `FellowEffectDisplay` and the member's entity, filled from the effects
> the client keeps for that entity, and then follows `EffectApplied`/`EffectRemoved` of the
> entity. A member without an entity has no list. A list rebinds when its row shows another
> member, and the binding reaches the lists one frame after the rows change. (9) A row
> shows `UIComponent_EffectDisplay_MaxIconColumns` icons, 6 when the list lacks the property.
> Rows and icons are placed as `combat-ui.md` §8.1 OPEN (arrangement) says: two rows of *e*
> pixels in the 37-pixel list, so with *e* = 20 the second row overflows the list by 3
> pixels. (10) The pulse of §8.2 uses the local player's combat state, as on E8 and E9.
> (11) Member icons carry the tooltip properties of §8.1, and hovering one shows the effect
> tooltip of `skill-tooltips.md` §12.2 for the member's effect instance.

---

<a id="s10"></a>
## 10. Plugin views

`Turbine.Gameplay` `Party` and `PartyMember` over the fellowship model of TR2 are specified in
LPA §15.4 (API facts SP22, SP23; Tessera rules there).

---

<a id="s11"></a>
## 11. Invariants

- **SPI1** (data) The `social` layout resolves to 881 element instances; its `Social_Panel`
  page list has five entries, exactly one with `UICore_Panel_page_open` true (`GroupingPage`);
  `FriendsPagePanel` has two, `FriendsPage` open.
- **SPI2** (data) `FriendTemplate` has the children of §3.2 with those IDs; `FriendTemplateLabel`
  carries the three colour properties; `FriendTemplateFellowship` has exactly the three
  `fellowship_*` states.
- **SPI3** (data) `minifellow` has four `FellowMiniPanel_N` whose scaler groups are
  `FellowshipVitals1` … `4` in order; `FellowMiniTemplate` and `FellowMicroTemplate` contain
  the child IDs of §4.2, except that the mini template lacks `FellowTemplateAssistant` and the
  micro template lacks `VitalHealthFieldBg` (SP34); `FellowTemplateLeader` and
  `PlayerLeaderIcon` have the states `fellowship`, `raid`, `monsterplaygroup`.
- **SPI4** (data) Each of the three coin fields of `securetrade` carries a distinct
  `UI_SecureTrade_Currency_Type`, and the three DIDs are the currency records of EC §7.
- **SPI5** (data) Every (table, token) of this spec exists in the install's table of that key.
- **SPI6** (Tessera) A fellowship list never yields a row for the player and never more than
  5 rows; `PlayerLeaderIcon` is shown iff the player leads.
- **SPI7** (Tessera) Two `FellowshipChanged` with the same member names, leader and order
  derive no notice and no plugin event.
- **SPI8** (Tessera) After any `TradeOffer` both side fields are `normal`.
- **SPI9** (data) In `FellowMiniTemplate` and `FellowMicroTemplate`, none of
  `FellowTemplateVoiceChat`, `…VoiceChatSquelched`, `…SendingData`, `FellowTemplateLinkDead`,
  `FellowTemplateDefeated`, `FellowTemplateLeader` carries `UICore_Element_hide`; in `toolbar`,
  none of `PlayerVoiceChat`, `PlayerVoiceChatSquelched`, `PlayerSendingData` does.
- **SPI10** (data) In `ItemDisplay_MediumTemplate`, `ItemDisplay_QuantityOverlay` precedes
  `ItemDisplay_Name` in tree order, and both are Text elements.
- **SPI11** (data) In `securetrade`, each `MoneyDisplay_Entry_…Field` holds exactly one editable
  Text, with the IDs of §6.1; `SelfTrade_TotalMoney` and `OtherTrade_MoneyDisplay` have no
  children and name `MoneyDisplay_CoinTemplate` of layout `moneydisplay`.
- **SPI12** (data) (UI, 0x070F9F3C) declares exactly the argument 0x7C83B5; (UI, 0x038B9CDC) and
  (UF, 0x06CDDF0C) exactly 0x50AA9C; (UF, 0x019353F5) and (UF, 0x0803A974) exactly 0xA2D94C5.
- **SPI13** (Tessera) After any fellowship update, no member row and not panel P4 shows a voice,
  squelched, sending-data or volume element (TR14), and only the current meters of a row's
  vital displays have a non-zero position (TR15).
- **SPI14** (data) `ItemDisplay_Name` of `ItemDisplay_MediumTemplate` has exactly 32 states, among
  them the six of §6.2; its own block and every state carry a one-entry
  `UICore_Text_font_colors` and no `UICore_Text_font_color` (SP30, SP36).
- **SPI15** (Tessera) A member row's `FellowTemplateVolumeControl` stays hidden after the bar is
  laid out again (for example after its position or size changes), and `VitalHealthFieldBg`
  is in state `normal` (TR14, TR15).

---

<a id="s12"></a>
## 12. Worked synthetic examples

Names below are invented scenario strings; tokens are this spec's.

<a id="s12-1"></a>
### 12.1 Fellowship formation, leader change, removal, disbanding

Player "Tam" (entity P). Actors `$a` "Brin" (entity A), `$b` "Cael" (entity B).

| Step | Event | Rows of panel 1 | Leader shown on | Derived lines (TR4) |
|---|---|---|---|---|
| `fellowship "Tam" $a leader="Tam"` | members [Tam(P, leader), Brin(A)] | Brin | `PlayerLeaderIcon` | (FT, 0x024583E7) |
| `fellowship "Tam" $a $b leader="Tam"` | + Cael | Brin, Cael | `PlayerLeaderIcon` | (FT, 0x047E6917) Cael |
| `fellowship "Tam" $a $b leader=$a` | leader Brin | Brin, Cael | Brin's row | (FT, 0x03FD5682) Brin |
| input `kick "Cael"`, then `fellowship "Tam" $a leader=$a` | Cael gone | Brin | Brin's row | (FT, 0x09284BE7) Cael |
| `fellowship none` | empty | — (panel hidden) | — | (FT, 0x018CCD74) |

Plugin events for the same run: LPA §15.4 example.

<a id="s12-2"></a>
### 12.2 Trade

Actor `$t` "Dara" (entity T). `trade $t` → `TradeShown { window 1, peer T }`: window opens, peer
label (UST, 0x0CFA21C2) with "Dara", both fields `normal`, trade button ghosted (both empty).
`trade-offer peer items="Rope" coin=1234567` → peer list one entry "Rope" (a literal: name in
`ItemDisplay_Name`, no icon, TR9), peer money display three coin elements reading 12 (gold),
345 (silver), 67 (copper) from left to right (values 100,000 / 100 / 1, EC §7); reset; button
not ghosted. Before that offer the peer display showed only the copper coin with 0
(`SuppressLeadingZeroes`, §6.1). Input `trade-ready on` sends `SetTradeReady { window 1, ready: true }`; the button
shows highlighted. `trade-ready own on` → own field `highlight`. `trade-ready peer on` → peer
field `highlight`. `trade-offer peer items="Rope","Lamp"` → both fields `normal`, button not
highlighted. `close trade` → `WindowClosed { window 1 }`: hidden.

<a id="s12-3"></a>
### 12.3 Friends sort

`friends "bo":off "Al":on "cy":on` → rows by Member ascending: Al, bo, cy (case-insensitive).
A click on the Last-online header (new column, ascending): bo (offline) first, then Al, cy
(online, by name). A second click: descending, cy, Al, bo.

---

<a id="s13"></a>
## 13. Open questions (summary) and owner checks

| # | Section | Question | Tessera rule | Owner check |
|---|---|---|---|---|
| Q1 | §3.1 | Toggle semantics of the social commands on an open panel | §3.1 | O133 |
| Q2 | §3.3 | The record test behind "known"; when the deleted colour is used | TR5 | — |
| Q3 | §4.3 | Member order in the fellowship panel and in `Party:GetMember` | list order | O130 |
| Q4 | §4.1 | The option that splits a raid over panels 2–4; when the micro template is used | not used | — |
| Q5 | §7.1 | Which token of each pair is the fellowship command; sub-word tokens | TR11 | O130 |
| Q6 | §7.2 | Whether `SelectFellowOne` targets the player and N maps to panel order | §8.1 | O130 |
| Q7 | §3.6 | Initial ignore sort | TR6 | O133 |
| Q8 | §6.2 | Entry limit per trade side | SF's 32 | O132 |
| Q9 | §6.6 | Texts of the client messages for refused drops | own words | — |
| Q10 | §3.5 | Notes and auto-fellow in a future feed revision | ghosted | — |
| Q11 | §6.6 | How an item is taken back out of an offer | none | O132 |
| Q12 | §7.1 | A disband command in the feed | **answered** (revision 1): `DisbandFellowship`, decision 0035 | — |
| Q13 | §5.2 | Whether the invitation window closes on expiry by the session or a client path | no timeout | O131 |
| Q14 | §4.6 | Whether the original also posts the TR4 lines when the change is the player's own action, and their order relative to the panel update | TR4 | O130 |

Owner checks are the rows O130–O133 of `docs/checklists/owner-observations.md`: O130 fellowship
display (member order, leader marker, out-of-view rows, the lines on leader change, dismissal
and disband, F1 and F2, the disband and promote names, whether voice icons show without voice
chat), O131 invitation prompt (expiry), O132 trade window (entries per side, taking an item
back, the coin displays), O133 Social panel (O and a page command on an open panel, sorting,
the ignore page's default sort).

---

<a id="s14"></a>
## 14. Notes for other specs

| Spec | Note |
|---|---|
| UL §17.4 P8 | "Friend, guild, group and raid lists: yes, as empty lists" is refined by this spec (§3, TR5, TR6, TR8): the friends and ignore lists are filled from the feed. |
| GC §3.2, Q2 | The invitation prompt is the `fellowinvite` window, not a `Dialog_*` marker (§5.4). |
| CW §10 item 3 | TR11 registers further original names (friend, ignore, invite, leave, dismiss, promote, disband) under the same rules; CW's "no other original command name is registered" needs that exception at its next revision. |
| KM §12.11 | Classes 11 and 15 gain Tessera uses for the commands of §7.2 (TR12). |
| LPA §9.3 | `Party.MemberAdded`/`MemberRemoved` carry `Player` (SP22); applied in LPA. |
| SF §6.4, §9.2 | No change. TR2 states how a list without the player is read; Q10 asks for notes. Q12 is answered by SF "P4 social" (`DisbandFellowship`); TR3 reads `FellowVitalChanged` as SF §6.7 asks (its "P4 social" OPEN item (2) is taken up here). |
| GP §3, §11, §14 note | The P4 portrait's voice elements and the member rows' voice, status and ready-check elements are elements game code shows or hides (§4.2, §4.3, TR14); `MultiVitalDisplay` and `EffectDisplayList` on member rows follow TR15, TR16; `PlayerMoneyDisplay` and `MoneyDisplay` are coin-element lists (§6.1), not texts. Applied as a note in GP §14. |
| `combat-ui.md` (branch `p2`) §4.1 OPEN | That OPEN leaves the player frame's voice elements at the layout's visibility; SP27 answers it (BIN: hidden at set-up or by a property the sandbox never sets); TR14 hides them. For that spec's owner at the merge. |
| `npc-windows.md` §11 (on `main`) | Uses "placeholder icon" for literal entries without defining it; TR9's definition (no icon, no stand-in image) can serve there too. For that spec's owner. |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-11 | First version (spec writer, dirty room; OBS and BIN on client 4906.0071.1794.4035) for decision 0024 P4: Social panel P8 pages, friends and ignore lists (rows, colours, sorting, buttons, dialogs, menus, notices), fellowship panels (four `FellowshipVitals` panels, member rows, leader marker, out-of-view rule, menu, rebuild, notices), invitation window, trade window (sides, entries, coin, accept states, opening, closing, notices), command names and key-map commands, mapping to SF §9.2/§9.4, Tessera rules TR1–TR13, invariants SPI1–SPI8, examples §12.1–§12.3, Q1–Q14, owner checks O130–O136. |
| 2026-10-11 | Review 1 (spec reviewer, independent session): changes requested, blocking items B1–B7 and notes (a)–(d) in the header's review record. No text of the spec changed. |
| 2026-10-11 | **Revision 1** (spec writer, dirty room; a session that did not do review 1), answering review 1. New provenance key OBS-R1 and BIN questions (q8)–(q10); new rows SP25–SP33; DEC adds decision 0035 and SF "P4 social". **B1:** §4.2 lists every row child with its visibility (new table "Visibility of the status, voice and ready-check elements", anchor `s4-2-1`), §4.3 the P4 portrait's voice elements (SP25–SP27); new TR14 hides the voice, squelched, sending-data and volume elements on rows and the three voice elements on P4; GP §14 note extended. **B2:** §6.1 gives the coin-entry IDs and the money displays as coin-element lists (summary of `npc-windows.md` §9.2–§9.3), §6.2 the parts of an entry by ID with a name-colour Tessera rule, TR9 defines a literal entry's "no icon" and the coin displays (SP30, SP31). **B3:** level templates in §4.2 and §5.1 (SP28). **B4:** §3.5 heading argument 0xA2D94C5 (SP33). **B5:** §5.1 full title label `FellowInvite_TitleBar_Label` (0x100016A4). **B6:** §4.2 names the vital-display parts and the effect list (SP29); new TR15 (meters) and TR16 (effects, pointing to `combat-ui.md` §8 on `p2`, interim: no icons); `FellowTemplatePetVitals` and `FellowTemplateVolumeControl` recorded as hidden by the layout; `FellowTemplateAssistant` exists only in the micro template (the earlier "same child IDs" corrected). **B7:** TR3 reads `FellowVitalChanged` (morale and power per vital, the later of it and the entity's `VitalChanged`), §8.1 adds `DisbandFellowship`, §8.2 adds `FellowVitalChanged`, Q12 answered, TR11 sends `DisbandFellowship`, TR13 follows SF §9.8 (remote backend at minor 2), §14 SF row. **Notes:** (a) TR12 and the engineering choices are Tessera rules (new §9.1, anchor `s9-1`); (b) §6.3 answered by BIN (SP32: only the committed entry is cleared, the amounts present are sent); (c) TR3 and §4.4 state argument 0x4AA1147; (d) status Revised. §11 adds SPI9–SPI13; §12.2 example updated; §13 owner column mapped to the existing rows O130–O133. **Changes to behaviour already built on `p4`:** (1) TR14: the voice, squelched and sending-data icons (the red and green icon) and the volume control are hidden on member rows and P4; (2) §6.2: the entry name goes into `ItemDisplay_Name`, not the first Text (the quantity overlay); (3) §6.1, TR9: `SelfTrade_TotalMoney` and `OtherTrade_MoneyDisplay` are filled with coin elements instead of their own text; (4) §6.2: the name colour follows the item's quality; (5) SP28: the level texts use the two `UI` templates instead of a plain number; (6) §3.5: the row-menu heading passes 0xA2D94C5; (7) TR15: only the current morale and power meters take the value (background, temporary, dread and wrath meters no longer). Unchanged: `DisbandFellowship`, `FellowVitalChanged`, TR12, TR13, §9.1, §6.3, the literal entry's icon, TR16's interim (no icons), argument 0x4AA1147, the coin-entry elements found (now named by ID). |
| 2026-10-11 | **Review 2** (spec reviewer, independent session that wrote no part of this spec and did not do review 1): re-review of revision 1, **passed; status Accepted.** Spot checks and non-blocking notes (a)–(c) in the header's review record. No text of the spec changed other than the header. |
| 2026-10-14 | **Revision 2** (spec writer, dirty room; a session that wrote neither review; status Accepted → Revised, awaiting independent review). New provenance: BIN question (q11) on the same build (SHA-256 prefix d22363eb9aff0fb5, Ghidra 12.1.3, private copy deleted after use), key OBS-R2, rows SP34–SP36. **Review 2 notes:** (a) SP30: `ItemDisplay_Name` has 32 states (31 `…_ColorName` and `Floaty_CriticalDamage`), not 33; (b) §4.2 and SPI3: the micro template also lacks `VitalHealthFieldBg` (SP34); (c) TR15: `VitalHealthFieldBg` keeps its default state `normal`. **§9.1 implementer items:** (1)–(5) become Tessera decisions (e)–(i), each confirming what was built: (e) the name colour is written from the first entry of the effective `UICore_Text_font_colors` into `UICore_Text_font_color` after every state change of the name (SP36: one-entry arrays, only code changes its state); (f) the volume control stays disabled with `hide_when_disabled`, because the bar's layout writes the element's one visibility (`ui-widgets.md` §6.4, answered the same day; SP35: the original writes the visibility after the bar's last layout), also in TR14 and the §4.2 table; (g) = TR15 above; (h) the shared fallback coin element when the coin template cannot be created; (i) P4's portrait elements are hidden at every build of P4. The note's heading no longer says OPEN; its text is kept as the record, the TR16 sentence in it unchanged. Items (6)–(7) are answered in `lua-plugin-api.md` §15.4 further rules 7–8 (same date). New invariants SPI14 (data), SPI15 (Tessera). **Changes to behaviour already implemented on `main`:** none in this spec. The related change is in `lua-plugin-api.md` §15.4 further rule 7: a `PartyMember`'s `EffectList` now receives the effect events of the entity it follows (on `main` it receives none). |
| 2026-10-14 | **Review 3** (spec reviewer, independent session that wrote no part of this spec, its revisions or its reviews): review of revision 2 (commit 7c545cd), **passed; status Revised → Accepted.** Spot checks and one non-blocking note (name `UICore_Scrollbar_position` where SP35, §4.2 and SPI15 say "position") in the header's review record. No text of the spec changed other than the header and this row; implemented behaviour unchanged. |
| 2026-10-23 | **Revision 3** (spec writer, dirty room; no new probe: the facts are the layout values of SP29 and the §4.2 table and `combat-ui.md` §8). §9 TR16: the interim (no icons) is marked ended; new TR16a (binding of a row's list to the member's entity, rebinding, one-frame lag), TR16b (icons per row, TR30a placement, the second row 3 pixels past the 37-pixel list, drawn), TR16c (pulse by the local player's combat state), TR16d (member icon tooltips); the implementer note of 2026-10-14 (choices (8)–(11)) is marked Answered. Status Accepted → Revised, awaiting independent review. **No implemented behaviour changes.** |
| 2026-10-23 | Independent review (spec-review:spec-gameplay:2 (independent session)) of the 2026-10-23 revision: **passed**, CLEAN_ROOM.md §5.3 every item; status Revised → Accepted. Header status and review record only; no other text changed; no implemented behaviour changes beyond those the revision row states. |
