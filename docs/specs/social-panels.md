<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Social panels: friends and ignore lists, fellowship display, fellowship invitation, trade window

| | |
|---|---|
| **Status** | Draft (2026-10-11), awaiting an independent review |
| **Last updated** | 2026-10-11 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Pending (a session that wrote no part of this spec) |
| **Review record** | None yet |
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
only (LEGAL.md §11.1 rule 5 unchanged: Tessera's server runs no gameplay rules). This spec
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
chat, pets, mail and auction.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The owner's install (Steam; archives and `x64/lotroclient64.exe` unchanged since 2026-10-04), read in place and read-only on 2026-10-11 with throwaway probes in the dirty scratch area: the 302 UI layout records (the dump of 2026-10-06, re-resolved with a probe that merges by UL §7: derived properties replace base properties by PID; the P8 instance count 881, text elements 61 and string references 185 reproduce UL §17.2); the layouts `social` (0x2200004E), `friends` (0x22000061), `ignore` (0x22000062), the friends-page base 0x2200009B, `grouping` (0x2200004F), `minifellow` (0x2200005C), `fellowinvite` (0x22000604), `securetrade` (0x22000024), `itemdisplay` (0x22000051), `toolbar` (0x22000033); the STRINGTABLE DID map 0x28000004 (179 keys) and the string tables it names below (every entry of `Commands`, `CommandHelp`, `Fellowship`, `Trade`, `UI_SecureTrade`, `UI_Friends`, `UI_Social`, `PlayerActionHandler` read; text compared, not reproduced); the enum mappers `UIElementID`, `UIStateID`, `UIElementType`, `InputActions` (0x230000AF, base 0x23000176), `UIScalingGroupType`; the four default key-map records 0x1D000000, 0x1D000001, 0x1D000004, 0x1D000009 (parsed by KM §12.9 with no bytes left over). |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`), the shared auto-analysed Ghidra project (a private copy, headless, read-only), 2026-10-11. Only the routines that reference the element IDs, string tokens and event names below, and their direct callees, were read, by address range; no listing or decompiler output is kept. **Questions:** (q1) what the friends and ignore pages do on header, button, row and menu messages; which elements a friend row fills and from what; sort keys and default; (q2) how the fellowship panels are created, which members get a row, which template, which children are set from what, and how the leader marker is chosen; (q3) what the invitation window shows and what its buttons do; (q4) how the trade window is wired: sides, coin entries, item drops, the trade button, accept states, resets, closing, the request dialog; (q5) which input commands open which Social page; (q6) whether client code posts the fellowship, friends and trade notices or command names itself; (q7) the argument tables of `Party.MemberAdded`/`MemberRemoved` and the semantics of the `Party` getters (LPA §9.3 OPEN). **Why M1–M3 were not enough:** the layout data name elements, states and templates but not which code fills them, which rows are created, which states are chosen, or what the buttons send; the original client cannot be observed in this session; S2 and S3 document the plugin API only. The network protocol, login, keys, launcher and anti-tamper code were not read (decision 0004): where a routine hands an action to the session layer, only the fact that it does so is recorded. |
| **S2** | The official Lua API reference (U25 pages in the research clone `lunarwtr_lotro-api-docs`), read for facts only (LPA key S2). |
| **S3** | `lunarwtr_vscode-lotro-api`: `TurbineEventsDump.json` (event argument tables captured in game), read for facts only (LPA key S3). |
| **DEC** | Accepted Tessera decisions [0024](../decisions/0024-parity-track.md) (parity track, P4), [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge), [0015](../decisions/0015-ui-rendering.md) (UI rendering), [0020](../decisions/0020-chat-command-routing.md) (one command dispatcher), and the Accepted SF. Statements taken from them are **Tessera rules**. |

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

**Row menu** (in this order): a heading (UF, 0x019353F5) with the friend's name; send tell
(UF, 0x0743846D), enabled when online or away; invite to fellowship (UF, 0x04529325),
enabled when the player is in no group or leads one that may grow and the friend is online
or away; remove friend (UF, 0x033D8545); ignore (UF, 0x044B1505); edit note
(UF, 0x038601E5); and a toggle between "auto-fellow" (UF, 0x065BE227) and "no auto-fellow"
(UF, 0x00CB5BA7).

> **OPEN (Q10):** notes and auto-fellow have no field in the feed or protocol v1.1.
> **Tessera rule:** §9 TR5 (ghosted).

> **OPEN (implementation, P4 clean room, 2026-10-11):** the row menu's heading
> (UF, 0x019353F5) is given "with the friend's name" but no argument ID. `tessera-ui`
> (`game::social::friends`) passes the name as argument 0x52615 (the usual name argument);
> please give the entry's argument ID.

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
`ClassDisplay_RaidVitalBackTemplate`); both have the same child IDs. The game code fills them
from the member's group record (BIN):

| Child (ID) | Mini rect | Shows |
|---|---|---|
| `FellowTemplateNameText` (0x10000438) | (56, 2, 133 × 14) | the member's name |
| `FellowTemplateClassField` (0x10000439) | ClassDisplay (5, 3, 50 × 50) | class icon (22-entry table) |
| `FellowTemplateLevelText` (0x10000440) | (12, 42, 32 × 19) | level, through a `UI`-table template with the level as argument |
| `FellowTemplateVitalField` (0x10000437) | (3, −3, 170 × 80) | `VitalHealthField` (MultiVitalDisplay, (52, 25, 99 × 6)): morale meter with temporary-morale and dread overlays; `VitalPowerField` (MultiVitalDisplay, (51, 32, 99 × 6)): power meter, replaced by a pip display of type `Wrath` for that class resource; both bound to the member; their numeric texts hidden by the layout; `VitalHealthFieldBg` (200 × 80) takes the member's vital-display override state |
| `FellowTemplateLeader` (0x1000043A) | (39, 34, 16 × 16) | leader marker (§4.3) |
| `FellowTemplateLinkDead` (0x1000043B) | (55, 20, 112 × 16), text (UI, 0x00B8F659) | shown while the member is link-dead |
| `FellowTemplateDefeated` (0x100010C9) | (55, 20, 112 × 16), text (UI, 0x08FAF389) | shown while the member is defeated |
| `FellowEffectDisplay` (0x1000043E) | (54, 37, 150 × 37); 2 rows × 6 columns of effect icons | the member's effects (player-only display for the local player) |
| `FellowAssist` (0x1000043F) | Button (153, 21, 16 × 16), tooltip (UI, 0x0C110D2E) | assist the member |
| `FellowTemplatePetVitals` (0x10000441) | hidden by the layout | the member's pet |
| `FellowTemplateVoiceChat`, `…Squelched`, `…SendingData`, `FellowTemplateVolumeControl`, `FellowReadyCheckTemplate`, `FellowTemplateAssistant` | — | voice chat, ready check, raid assistant: out of scope (§0) |

Meter colours follow the meters' own states (`vitalmeter_green`, `_red`, `_yellow`, `_blue`,
`_grey`, UW §4); the game code's choice among them is P2's (`combat-ui.md`, if written) and is
not changed here.

> **OPEN (implementation, P4 clean room, 2026-10-11):** (a) the `UI`-table template of
> `FellowTemplateLevelText` (and of `FellowInvite_FellowLevel`, §5.1) is not named by token
> or argument ID; `tessera-ui` (`game::social::fellowship`, `invite`) shows the level as a
> plain number until it is. (b) `VitalHealthField` and `VitalPowerField` are
> `MultiVitalDisplay` game types (GP §11 fallback); Tessera sets `UICore_Meter_position` on
> every `Meter` below them to current ÷ max and draws no temporary-morale, dread or pip
> overlay. Please confirm or give the children game code fills.

> **OPEN (implementation, P4 clean room, 2026-10-11, review):** `FellowEffectDisplay`
> (2 × 6 effect icons) is not filled. `tessera-ui` (`game::social::fellowship`) keeps each
> member's effects from `EffectApplied`/`EffectRemoved` (TR3, `MemberRow::effects`) but
> leaves the display as the layout has it: the spec does not say which children the game
> code creates or fills (icon element, layout, order of the 12 places, what happens past
> 12), nor which icon an effect record gives. Please give them, or point to the effect-icon
> path of the combat UI to reuse.

<a id="s4-3"></a>
### 4.3 Who gets a row, order, limit, leader marker (BIN, SP11; Probable, SP12)

- **The local player gets no row** (BIN): the member list routine skips the player's own
  character. The player's vitals are panel P4; its child `PlayerLeaderIcon` (0x10000B4E,
  hidden by the layout, image 0x41007DCE) is the player's leader marker.
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
(FT, 0x0571E4C5) (out of range), argument 0x4AA1147 = the member's name, and clears the
target.

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
| `FellowInvite_TitleBar` / `_Label` | (57, −7, 240 × 45) / (0, 15, 240 × 20) | layout default title (UI, 0x03045B45) |
| `FellowInvite_InviteText` | (20, 38, 316 × 63) | the invitation text |
| `FellowInvite_FellowList` (0x10001695) | ListBox (21, 104, 313 × 220) with `FellowInvite_Scrollbar` | one `FellowInvite_Fellow` row per member |
| `FellowInvite_WarningText` (0x10002EA7) | (20, 328, 316 × 67) | a warning, when given |
| `FellowInvite_AcceptButton` (0x100016A6) | text button (67, 401, 100 × 20), text (UI, 0x0F70211E) | accept |
| `FellowInvite_DeclineButton` (0x100016A7) | (187, 401), (UI, 0x0A76F86E) | decline |
| `FellowInvite_CloseButton` (0x1000168E) | (334, 25, 16 × 16), tooltip (UI, 0x08921EF4) | decline |
| `dragbar` | (49, 6, 255 × 24), tooltip (UI, 0x06D77F5C) | move |

Row template `FellowInvite_Fellow` (0x10001696, top level of the same layout, 296 × 39):
`FellowInvite_FellowClass` (ClassDisplay 25 × 25: class icon), `FellowInvite_FellowLevel`
(level, through the `UI`-table level template), `FellowInvite_FellowName` (name),
`FellowInvite_FellowArea` (the member's area name; cleared when unknown).

> **OPEN (implementation, P4 clean room, 2026-10-11):** the title label is written
> "`FellowInvite_TitleBar` / `_Label`"; `tessera-ui` looks it up by the label
> `FellowInvite_TitleBar_Label`. Please confirm the full label.

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
| own (left) | `SelfTrade_Field` (7, 39, 252 × 388) | label `SelfTrade_Label` (UST, 0x0D63B6DC); `SelfTrade_TotalMoneyField` (0, 20): text (UST, 0x02251404) and `SelfTrade_TotalMoney` (PlayerMoneyDisplay: the player's wallet, three coin columns, all shown); `SelfTrade_TradeField` (0, 48, 252 × 264; states `normal` image 0x41007F84, `highlight` image 0x41007F83): `SelfTrade_ItemList` (ListBox 226 × 222, one column, container, item display type Medium) with `SelfTrade_ItemList_SB`, and `SelfTrade_MoneyDisplayField` (44, 235): three coin entries; buttons `SelfTrade_TradeButton` (31, 316, 96 × 20; text (UST, 0x0A307BC4); boolean button) and `SelfTrade_CancelButton` (130, 316; (UST, 0x0190ED84)) |
| peer (right) | `OtherTrade_Field` (258, 38, 252 × 388) | label `OtherTrade_Label` (no layout text); `OtherTrade_TradeField` (0, 48; the same two states and images): `OtherTrade_ItemList` (ListBox 228 × 225, one column, Medium) with its scroll bar, and `OtherTrade_MoneyDisplay` (MoneyDisplay (45, 236, 160 × 22): three coin columns, all shown, leading zeros suppressed) |
| both | `SecureTrade_WaitOverlay` (4, 85, 508 × 266; image 0x41007F82 and a dark colour layer; blocks clicks) | `SecureTrade_WaitOverlayText`: (UST, 0x07E3F5E4) |

**Coin entries** (`MoneyDisplay_Entry_GoldField`, `…SilverField`, `…CopperField`, at
x = 0, 58, 111 in that order): each a field tagged `UI_SecureTrade_Currency_Type` = the currency
game object (0x70000218 gold, 0x70000219 silver, 0x7000021A copper here; EC §7: the
`GoldCurrency`, `SilverCurrency`, `CopperCurrency` records), with an editable one-line text
of at most 5 characters (no IME) and a label with a tooltip (UI, 0x0135BA5C),
(UI, 0x01E4A5BC), (UI, 0x0BE07C7C).

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

> **OPEN (implementation, P4 clean room, 2026-10-11):** the element IDs of the parts the
> trade window's code fills are not given: the medium item display's 165-pixel name text,
> the editable text of each `MoneyDisplay_Entry_…Field`, and the children of
> `PlayerMoneyDisplay` and `MoneyDisplay` (GP §11 OPEN). `tessera-ui` (`game::social::trade`)
> takes the entry's first `Text` element as the name, its `ItemIconDisplay` (by type) as the
> item slot (icon, quantity, examination tooltip), the first editable text below a coin
> field as its entry, and sets the money displays' own text to the gold, silver and copper
> amounts as numbers (leading zero denominations left out). Please give the IDs or confirm.

<a id="s6-3"></a>
### 6.3 Coin (BIN, SP18)

- The own wallet shows in `SelfTrade_TotalMoney`; the peer's offered coin in
  `OtherTrade_MoneyDisplay` (set from the peer's offer; 0 at opening).
- When a coin entry loses focus or receives Enter (BIN): the three texts are read; a text
  that is not a number posts (UST, 0x085F80B9) and clears that entry; a total
  (Σ entry × the currency's `Currency_Value`, EC §7; Probable for the arithmetic) above the
  player's money posts (UST, 0x06195A09) and clears the entry; a non-zero total that a
  further client check refuses posts that check's client message and clears the entry; in
  every case the amounts then present are sent as the coin offer (per currency).

> **OPEN (implementation, P4 clean room, 2026-10-11, review):** "clears the entry" is read
> as the entry that lost focus or received Enter. When the total of the other two entries
> is still above the player's money after that, the amounts present are sent anyway (the
> feed decides), as this section says; `tessera-ui` does that. Please confirm, or say
> whether the original clears more entries or sends nothing in that case.

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

> **OPEN (Q12):** SF §9.4 has no disband command; the disband names and the leader's
> disband action have no command to send. **Tessera rule:** §9 TR11 (a refusal line).

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
| `VitalChanged`, `LevelChanged`, `Defeated`, `Revived`, `EffectApplied`/`Removed`, `TargetChanged` of a member's entity | that member's row: meters, level, defeated field, effects, assist target |
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
- **TR3 Member data.** A member with an `entity` takes name from the event, and level, morale,
  power, defeat and effects from the last `LevelChanged`, `VitalChanged`, `Defeated`/`Revived`,
  `EffectApplied`/`EffectRemoved` the feed sent for that entity (kept after the entity
  despawns, SF §5.4). Without an entity (or before any such event): meters empty, level
  empty, no effects, class icon none. The class icon is shown only when an actor's class is
  known to the client half (the feed's `CharacterInfo` gives one for the player only), so it is
  normally empty. Link-dead is never shown. A click on a member without an entity, or whose
  entity is despawned, posts (FT, 0x0571E4C5) as a local line of chat type 1 (Tessera's
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
  answers). Literal refs (SF §5.2) show their text with a placeholder icon. Coin shown on the
  own side is the last own `TradeOffer` coin, split into gold, silver and copper by
  `Currency_Value` (EC §7), written into the three entries. The §6.4 content-change reset runs
  on every `TradeOffer`. A drop of an item not in the player's backpack is refused locally with
  a type 1 line in Tessera's own words. No item removal from an offer (Q11).
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
  disband names are registered and answer with a type 1 line in Tessera's words (no disband
  command in SF §9.4, Q12). A name argument missing gives a type 1 line. All lines are
  type 8 (lists) or type 1 (refusals).
  > **OPEN (implementation, P4 client, 2026-10-11):** the disband names send
  > `DisbandFellowship` (decision 0035 D4, `scenario-feed.md` §7.5, which answers Q12), not
  > TR11's refusal line; the feed's triggers answer it. Please align TR11 with decision 0035.
- **TR12 Keys.** The client maps `ToggleSocialPanel` and the five page commands to §3.1's
  Tessera rule, `SelectFellowOne` … `Six` and `AssistFellowTwo` … `Six` to §8.1, with the
  install's default bindings (KM §12.14 import).
  > **OPEN (implementation, P4 client, 2026-10-11):** Tessera's own default key map binds
  > `toggle_social` to O; `select_fellow_1` … `6` and `assist_fellow_2` … `6` stay unbound
  > there because F3 is Tessera's debug key (a binding would conflict in the `global`
  > context); the install's keys arrive through the key-map import (KM §12.14). Please
  > confirm, or name the Tessera keys.
- **TR13 Other backends.** On the test-server backend every Social command except `Chat` is
  refused with `InvalidCommand` (SF §9.5); the panels stay empty; the buttons are not ghosted
  for that reason (a refusal posts the session's error line). The sandbox without a scenario
  accepts and ignores them (SF §9.5).

---

> **OPEN (implementation, P4 client, 2026-10-11; engineering choices):** (a) The client has no
> engine text-input dialog (UB §5.5) or context-menu widget yet: the add dialogs of §3.5 and
> §3.6 post a type 8 line naming `/friend add NAME` and `/ignore add NAME`, and a row menu
> (§3.5, §3.6, §4.5) is kept for a later menu widget (the fellowship commands `/dismiss`,
> `/promote`, `/leave` are the player's way meanwhile). (b) `--open-panel` takes `social`,
> `friends`, `fellowship` and `trade`: the first three show P8 at its last, friends or
> grouping page; the fellowship display and the trade window still show only while the feed
> reports a fellowship or a trade (TR2, TR9). (c) Escape hides P8 before the game menu rule
> (`game-panels.md` §16). (d) A trade drop takes the item's whole stack (TR9 "the stack's
> quantity") and is offered only from the backpack container. Please confirm.

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
  the child IDs of §4.2; `FellowTemplateLeader` and `PlayerLeaderIcon` have the states
  `fellowship`, `raid`, `monsterplaygroup`.
- **SPI4** (data) Each of the three coin fields of `securetrade` carries a distinct
  `UI_SecureTrade_Currency_Type`, and the three DIDs are the currency records of EC §7.
- **SPI5** (data) Every (table, token) of this spec exists in the install's table of that key.
- **SPI6** (Tessera) A fellowship list never yields a row for the player and never more than
  5 rows; `PlayerLeaderIcon` is shown iff the player leads.
- **SPI7** (Tessera) Two `FellowshipChanged` with the same member names, leader and order
  derive no notice and no plugin event.
- **SPI8** (Tessera) After any `TradeOffer` both side fields are `normal`.

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
`trade-offer peer items="Rope" coin=1234567` → peer list one entry "Rope" (placeholder icon),
peer coin display 12 gold, 345 silver, 67 copper (values 100,000 / 100 / 1, EC §7); reset; button not
ghosted. Input `trade-ready on` sends `SetTradeReady { window 1, ready: true }`; the button
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
| Q1 | §3.1 | Toggle semantics of the social commands on an open panel | §3.1 | O130 |
| Q2 | §3.3 | The record test behind "known"; when the deleted colour is used | TR5 | — |
| Q3 | §4.3 | Member order in the fellowship panel and in `Party:GetMember` | list order | O131 |
| Q4 | §4.1 | The option that splits a raid over panels 2–4; when the micro template is used | not used | — |
| Q5 | §7.1 | Which token of each pair is the fellowship command; sub-word tokens | TR11 | O132 |
| Q6 | §7.2 | Whether `SelectFellowOne` targets the player and N maps to panel order | §8.1 | O133 |
| Q7 | §3.6 | Initial ignore sort | TR6 | — |
| Q8 | §6.2 | Entry limit per trade side | SF's 32 | O134 |
| Q9 | §6.6 | Texts of the client messages for refused drops | own words | — |
| Q10 | §3.5 | Notes and auto-fellow in a future feed revision | ghosted | — |
| Q11 | §6.6 | How an item is taken back out of an offer | none | O134 |
| Q12 | §7.1 | A disband command in the feed | refused | — |
| Q13 | §5.2 | Whether the invitation window closes on expiry by the session or a client path | no timeout | O135 |
| Q14 | §4.6 | Whether the original also posts the TR4 lines when the change is the player's own action, and their order relative to the panel update | TR4 | O136 |

Owner checks (to be added to `docs/checklists/owner-observations.md`, rows from O130):
O130 press O and a page command on an open Social panel; O131 compare fellowship panel order
with join order; O132 type the disband and promote names in a fellowship and in a raid; O133
press F1 and F2 in a fellowship; O134 offer many items in one trade and try to take one back;
O135 let a fellowship invitation lapse; O136 note the chat lines and their order on join,
leader change, dismissal and disband.

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
| SF §6.4, §9.2 | No change. TR2 states how a list without the player is read; Q12 asks for a disband command; Q10 for notes. |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-11 | First version (spec writer, dirty room; OBS and BIN on client 4906.0071.1794.4035) for decision 0024 P4: Social panel P8 pages, friends and ignore lists (rows, colours, sorting, buttons, dialogs, menus, notices), fellowship panels (four `FellowshipVitals` panels, member rows, leader marker, out-of-view rule, menu, rebuild, notices), invitation window, trade window (sides, entries, coin, accept states, opening, closing, notices), command names and key-map commands, mapping to SF §9.2/§9.4, Tessera rules TR1–TR13, invariants SPI1–SPI8, examples §12.1–§12.3, Q1–Q14, owner checks O130–O136. |
