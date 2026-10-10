<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Character selection and creation screens

| | |
|---|---|
| **Status** | Accepted (2026-10-10, second re-review passed; first version 2026-10-09, review and re-review of 2026-10-09 changes requested, revisions the same day; see the review record). (e) stands until branch `p0` is merged |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Independent spec-review sessions that wrote no part of this spec and no code (Clean-Room-Role: review): review and re-review 2026-10-09, second re-review 2026-10-10 |
| **Review record** | 2026-10-09, independent spec review (Clean-Room-Role: review; a session that wrote no part of this spec and no code), whole spec (new). Checked against `docs/CLEAN_ROOM.md` §5.3, decision 0024 P1 and every cited section of `player-character.md`, `appearance.md`, `combat-data.md` §9.1, `ui-layout.md` §3 and §5.8, `tessera-protocol.md` §5.3, §6.2.7–§6.2.9, §14.4 and `scenario-feed.md` (Accepted text on branch `p0`: §5.1, §6.1, §7.5, §7.7, §9.2, §9.4, §9.5, §16). Spot-checked read-only in the install with a throwaway probe: the seven layout records and their `UILAYOUT` labels, the stage mapper 0x23000132 (keys 1, 3, 4, 6, 7 as in §2.2), the full element trees of `chargen`, `chargen_characterselect`, `chargen_raceclass` and `chargen_appearance` (every label named in §3–§5 exists where stated; sizes 1440 × 900 z 250, 338 × 71, 261 × 45, 253 × 40/52/74/80 match; the row template is based on `Chargen_Character_Slot`), the `chargen_viewport` properties of §2.1 (all six values match), the class-info property names, and the presence of token 0x0BB533CE and the three variable IDs in table 0x250001BD (no text read). Checklist: functional facts only; no code or third-party structure (the `Appearance::new(Body::Character …)` form is Tessera's own API, as in `scenario-feed.md`); no game text; no gates touched. Result: **changes requested**. Blocking: (1) §2.1 describes `chargen` incompletely: its `ControlsParent` (edge modes 6, 5, 6, 5) holds `CharGen_Back_Button`, `CharGen_Continue_Button`, `CharGen_Exit_Button`, the turntable group `Chargen_Controls` (rotate left/right, zoom in/out buttons) and `CharGen_AvatarUpdate_Toggle` (label and check-box button); §6 uses Continue and back and §5.2 the avatar-update check box without naming any of these elements, so the flow cannot be wired from the layout without guessing; name them, say which stages show which, and give a P1 rule for the turntable and Exit. (2) §5.3 calls `CharGen_Name_Entry_Field` "a `ChargenNameTextEntry`", which reads as a widget type; in the data the field is a `Text` element (editable, selectable, one line) whose base reference is the element labelled `ChargenNameTextEntry` in layout 0x22000002 (no element type of that name exists); state it so. (3) §7.2 gives the deletion answer for the scenario feed only: `tessera-protocol.md` has no deletion message and the test-server backend refuses `DeleteCharacter` with `NetError::InvalidCommand` (`scenario-feed.md` §9.5), yet §3 shows the delete button for every backend; give the screen's rule per backend (as §7.1 does for creation), at least for M7's server (button hidden or disabled, or the refusal shown). Non-blocking: (a) §7.1 "rule 5, `server-content.md`": rule 5 is LEGAL.md §11.1 rule 5. (b) §2.2 "each has a scaling parent … that keeps the side panels at the screen edges": the side panels are siblings of the scaling parent and anchor by their own edge modes (5, 5, 6, 5) left and (6, 5, 5, 5) right; the scaling parent (6, 5, 6, 5) holds the centre parts. (c) The "Probable" reading of the description format in §3 has no provenance row; add one (OBS token IDs, meaning a reading). (d) Elements present but not given a P1 rule: `CharGen_Name_Admin_Button`, `CharGen_RandomName_Debug_Field`, `CharGen_ContinueLocked_BuyNowButton`, the `CharGen_Avatar_Loading_*` blockers, `CharGen_Appearance_Nationality_Text`, the choose-available-characters dialog of `chargen_characterselect`; say hidden or how used. (e) `scenario-feed.md` is not on this branch (Accepted on `p0`); links resolve only after `p0` is merged, which implementation of §7–§9 needs anyway. **Re-review 2026-10-09** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec and no code), whole spec (still new on this branch), with emphasis on the revision. Checked against `docs/CLEAN_ROOM.md` §5.3, `tessera-protocol.md` §6.2.2 and §6.2.7–§6.2.9, `scenario-feed.md` §9.4–§9.5 (on `p0`), `appearance.md` §3.3 and `LEGAL.md` §11.1. Spot-checked read-only in the install (archives byte-identical to an earlier independent decode of layouts 0x22000001, 0x22000002, 0x22000004, 0x2200000C, 0x2200000D, 0x22000010, 0x2200083E, re-decoded with a throwaway value-reading probe): every box, base element, edge mode, z-level and button property in the §2.3 table (including `UICore_Button_broadcast_when_ghosted` on Continue only, the hot-button flags and first interval 0.05 on the two turn buttons, Exit sharing Back's box); the six viewport values of §2.1; §2.2 edge modes of the side panels and scaling parents; §4.3 boxes, z −10, `UICore_Element_blocks_clicks` and the 0xBF000000 fill of both loading blockers; §5.3 name field (base `ChargenNameTextEntry` in `text`, a `Text` 391 × 51, field 268 × 28 at 8, 0 with editable, selectable and one-line true); `CharGen_Name_Admin_Button` hidden; the create and race-change buttons in the Continue box; the row template 338 × 71 on `Chargen_Character_Slot`; the dialog and its entry templates top-level in `chargen_characterselect`. First-review items (1)–(3), (a)–(d): each answered as the change log says. Checklist: functional facts only; no code (the struct-like command forms are Tessera's own `tessera-net` API); no game text; provenance rows CS8–CS13 present; invariants testable; no gate touched. Result: **changes requested**. Blocking: (R2-1) §7.1 says that on M7's server the client sends `CharacterCreate { name, appearance }`, but the command the screen issues is `tessera-net`'s `CreateCharacter`, and `scenario-feed.md` §9.5 (the same sentence CS13 cites for deletion) makes the test-server backend refuse every new command of §9.4, `CreateCharacter` included, with `NetError::InvalidCommand`; the `remote` backend also has no event for `CharacterList` or `CharacterCreateFailed` (it surfaces only `Connected` with names). As written, creation on M7's server — needed by P1 exit criterion 1 — cannot be built without guessing. Either (i) state the rule for the `remote` backend (`CreateCharacter` with an appearance becomes `CharacterCreate`, class dropped; which session event a `CharacterList` and a `CharacterCreateFailed` become, e.g. `CharacterInfo` for the new name and `CharacterRefused` with Tessera's text per §14.4 code; the New button ghosted when `HelloAck.server_caps` lacks `CHARACTER_CREATION`, §6.2.2) and record it as a required revision of `scenario-feed.md` §9.5 with an OPEN and a provenance row, or (ii) treat creation on M7's server as OPEN with a per-backend rule for the New button (as §7.2 does for Delete) and say that P1 criterion 1's M7 part waits on it; in both cases make CS13, §7.1's answer table and §11 consistent. Non-blocking: (f) `CharGen_ContinueLocked_BuyNowButton` is also hidden by its own data (`UICore_Element_hide` true); add it to CS12 and §4.3. (g) §2.2 selection row: `CharGen_Notice_FinishingLogout` is a direct child of `CharGen_CharacterSelect_Base`, not of the scaling parent (only `CharGen_Temp_TooManyPlayers` is inside it). (h) Elements without a P1 rule: the top-level `ECommBrowser` (a `Browser`) of `chargen_characterselect` and `GamePlay_ExaminationTooltip_Activatable` under `CharGen_Base_Field` of `chargen`; say never created or unused. (i) The mark **Probable** (CS11) is not defined in the provenance key; define it (as `fonts.md` does) or reword. (j) §4.3 loading blockers are based on `Chargen_Silver_Border` (0x22000001, a nine-part border) as well as the fill; mention the border. (e) stands. **Second re-review 2026-10-10** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec and no code), the revision of 2026-10-09 (§1 key, CS12, CS14, CS15, §2.1, §2.2, §3, §4.3, §7.1, §7.1.1, §9 step 5, §11 Q11), checked against `docs/CLEAN_ROOM.md` §5.3, `tessera-protocol.md` §5.3 (`CharacterSummary` carries a body), §6.1, §6.2.2, §6.2.5–§6.2.10, §12.1, §13.1, §14.4 and Appendix A, `scenario-feed.md` §9.2, §9.4 and §9.5 (Accepted text on branch `p0`) and `test-server.md` §6.4 and its `character_creation` and `max_characters` settings. Spot-checked read-only in the install with an own throwaway probe (layouts 0x2200000C, 0x2200000D, 0x2200083E and 0x22000001 decoded in full): `CharGen_Notice_FinishingLogout` a direct child of `CharGen_CharacterSelect_Base` and `CharGen_Temp_TooManyPlayers` the only notice in the scaling parent; `ECommBrowser` a top-level `Browser` 100 × 100; `GamePlay_ExaminationTooltip_Activatable` a `Field` 192 × 128 at 571, 12 under `CharGen_Base_Field`; `CharGen_ContinueLocked_BuyNowButton` 241 × 41 at 600, 666 on `CharGen_Class_BuyNow` of 0x22000837 with `UICore_Element_hide` true; both loading blockers based on `Chargen_Silver_Border` of 0x22000001, which has eight edge pieces and a background; boxes, z-levels and `blocks_clicks` as in §4.3. All match. R2-1: answered by option (i). §7.1 now has one screen-level command and two events on every backend, a per-backend New-button rule and the Create, refusal and time-out rules; §7.1.1 gives a complete, testable rule for the `remote` backend (capability bit, acceptance conditions that match the protocol's preconditions, class dropped under LEGAL.md §11.1 rule 5, `CharacterList` → `CharacterInfo` with the summary's body, `CharacterCreateFailed` → `CharacterRefused` with Tessera's own text for each §14.4 code, unknown codes as 6, states); it is stated as a narrowing of `scenario-feed.md` §9.5 with an OPEN and §11 Q11 for the matching revision there and in Appendix A, and CS13, CS14, the answer table and §11 agree. Non-blocking (f)–(j): each answered as the change log says. Checklist: functional facts only; no code (struct-like forms are Tessera's own `tessera-net` and protocol names); no copied prose; no game text (no string read); provenance rows CS14, CS15 present with marks; invariants testable (§9 step 5); unknowns are OPEN; no gate touched (M7's own protocol only). Result: **passed**, status Accepted. Non-blocking notes for a later revision: (k) `test-server.md` §6.4 stores the NFC form of the name, so §7.1.1 step 3 ("an entry with the outstanding name") and §7.1's success rule ("name equals the requested name") miss a successful creation typed in a non-NFC form and report `SERVER_ERROR`; compare the NFC forms (or send the NFC form) and say so. (l) After the screen's 10 s time-out the backend's creation is still outstanding; say that the `remote` backend also refuses `EnterWorld` while it is (`tessera-protocol.md` §6.2.10, §13.1 make a second request fatal), or that the selection screen ghosts Enter meanwhile. (m) §9 step 5 creates one character per race on one account: name a fresh account and a `max_characters` (default 8) at least the number of playable races, so no creation of that step fails with `TOO_MANY` (the duplicate check comes first, `test-server.md` §6.4, so its code 1 is not affected). (e) stands. |
| **Implements into** | `apps/client` (the screens before world entry, decision 0024 P1 exit criterion 1), `crates/gamedata` (race, gender, class, nationality and appearance-control lists from the install; name generation and checks), `crates/ui` (fallback widgets of the creation screens' game widget types) |
| **Companion specs** | [`player-character.md`](player-character.md) (race tables §5, nationalities §6, class data §7, names §8, character slots §11.1), [`appearance.md`](appearance.md) (bodies §2, creation controls §3.4, face and colours §3.5, sliders §6), [`combat-data.md`](combat-data.md) §9.1 (class tables), [`ui-layout.md`](ui-layout.md), [`misc-records.md`](misc-records.md) (UI scenes, movie library), [`scenario-feed.md`](scenario-feed.md) (Accepted on branch `p0`, 2026-10-09: character list, `CharacterInfo`, `CreateCharacter`, character triggers; §3.3, §5.1, §6.1, §7.5, §7.7, §9), [`tessera-protocol.md`](tessera-protocol.md) §6.2.7–§6.2.9 and [`test-server.md`](test-server.md) (M7's character creation), [`character-panels.md`](character-panels.md) |

---

<a id="s0"></a>
## 0. Purpose

Decision 0024 P1 exit criterion 1: "The creation screen offers every race, class and body
choice of the install; a made character enters the sandbox, and on M7's server other clients
see its look." This spec gives the original's screens as the install's layout data shows
them, the records that fill every list and control, the flow between screens, and how the
scenario feed and M7's server answer the screens' commands. The original's screen logic is
game code that was not traced; where the data does not settle a behaviour the spec gives a
**Tessera rule** and an owner check.

Values (DIDs, labels, counts) are observations of one install; read them at run time.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (client build 4906.0071.1794.4035), read-only, archives unchanged since 2026-10-04 (re-checked 2026-10-09): the UI layout records `chargen` (0x2200000C), `chargen_characterselect` (0x2200000D), `chargen_raceclass` (0x2200083E), `chargen_appearance` (0x22000010), `chargen_movielibrary` (0x220001A8), `chargen_movieplayer` (0x220001B9), `renamecharacterdialog` (0x22000845) and the appearance-control templates they are based on (dump of 2026-10-06, re-read with a throwaway value-decoding probe 2026-10-09); the mappers `CharGen_Stage` (0x23000132) and the camera mapper (0x230000B9); the race, nationality and class records (`player-character.md` OBS); string table 0x250001BD (character screens) read for tokens and variable IDs only. |
| **PC**, **AP** | `player-character.md` and `appearance.md` (Accepted), for record layouts and rules cited by section. |
| **Design** | Tessera rules, marked **Tessera rule**. |
| **Probable** (mark) | A reading of observed data that fits every record checked but whose meaning was not traced to the original's behaviour; an implementer may rely on it, and a contrary observation turns it into an OPEN. |
| **Spec** | Accepted text of another Tessera spec, cited by section (`tessera-protocol.md`, `scenario-feed.md`). |

| # | Fact | Source | Mark |
|---|---|---|---|
| CS1 | The creation screens are four layouts over one full-screen base with a 3D viewport (§2) | OBS | Confirmed 2026-10-09 |
| CS2 | The stage mapper names `CharacterSelect`, `RaceClass`, `ClassSelect`, `Appearance`, `Name` (§2.2) | OBS | Confirmed |
| CS3 | Character selection screen parts and the character row template (§3) | OBS | Confirmed |
| CS4 | Race-and-class screen parts: race list, gender buttons, class list, race and class info, racial traits (§4) | OBS | Confirmed |
| CS5 | Appearance-and-name screen parts: nationality list and info, appearance controls list, randomise button, name entry, random-name button, tutorial choice, create buttons (§5) | OBS | Confirmed |
| CS6 | The records that fill each list (§4, §5) | PC, AP; OBS | Confirmed (records), Decision (display order where untraced) |
| CS7 | The flow between screens and the commands sent (§6, §7) | Design | Decision |
| CS8 | The shared controls of `chargen` under `ControlsParent`: back, continue and exit buttons, the turntable group, the avatar-update toggle; their positions, base elements and button properties (§2.3) | OBS (layout 0x2200000C read in full with the value-decoding probe, 2026-10-09) | Confirmed 2026-10-09 (data); Decision (which stage shows which) |
| CS9 | `CharGen_Name_Entry_Field` is an element with a base reference to the `Text` element `ChargenNameTextEntry` of layout 0x22000002 (`text`), with editable, selectable and one-line set (§5.3) | OBS | Confirmed 2026-10-09 |
| CS10 | Side panels anchor by their own edge modes; the scaling parent holds the centre parts (§2.2) | OBS (the three screen layouts' top-level elements) | Confirmed 2026-10-09 |
| CS11 | The description line of a character row uses the three-variable format token 0x0BB533CE of table 0x250001BD with variables 0x50AA9C, 0x56575, 0x480683, and shorter formats exist (§3) | OBS (token and variable IDs present; no text read) | Confirmed (IDs); **Probable** (that the three variables are level, race name and class name: a reading of the variable roles, not traced) |
| CS12 | Elements hidden by their own data (`UICore_Element_hide` true): `CharGen_Name_Admin_Button`, `CharGen_ContinueLocked_BuyNowButton`, `CharGen_Avatar_Loading_CenterScreen`, `CharGen_Avatar_Loading_RaceBlocker` (the latter also blocks clicks) (§3–§5) | OBS | Confirmed 2026-10-09 |
| CS13 | No backend except the scenario feed can answer a deletion (creation: CS14): `tessera-protocol.md` §6.2 has no deletion message; the test-server backend refuses `DeleteCharacter` with `NetError::InvalidCommand` and the plain sandbox ignores it (`scenario-feed.md` §9.5) (§7.2) | `tessera-protocol.md` §6.2; `scenario-feed.md` §9.5 (Accepted on `p0`) | Confirmed (spec text, 2026-10-09) |
| CS14 | On M7's server the screens' `CreateCharacter` is carried by `CharacterCreate` (§6.2.7) and answered by `CharacterList` or `CharacterCreateFailed` (§6.2.8–§6.2.9, codes §14.4), accepted only when `HelloAck.server_caps` has `CHARACTER_CREATION` (§6.2.2) and only one request is outstanding; the `remote` backend has no event for either answer yet (Appendix A) and `scenario-feed.md` §9.5 makes it refuse `CreateCharacter`; the mapping of §7.1.1 is a Tessera rule that narrows that refusal | Spec (`tessera-protocol.md` §6.2.2, §6.2.7–§6.2.9, §13.1, §14.4, Appendix A; `scenario-feed.md` §9.4–§9.5 on `p0`); Design | Confirmed (spec text, 2026-10-09); Decision (the mapping) |
| CS15 | `CharGen_Notice_FinishingLogout` is a direct child of `CharGen_CharacterSelect_Base` (only `CharGen_Temp_TooManyPlayers` is inside the scaling parent); `ECommBrowser` (a `Browser`, 100 × 100) is a top-level element of `chargen_characterselect`; `GamePlay_ExaminationTooltip_Activatable` (a `Field`, 192 × 128) is a child of `CharGen_Base_Field`; both loading blockers are based on `Chargen_Silver_Border` of layout 0x22000001 (eight edge pieces and a background) and carry the translucent fill (§2, §3, §4.3) | OBS (layouts 0x2200000C, 0x2200000D, 0x2200083E, 0x22000001, value-decoding probe 2026-10-09) | Confirmed 2026-10-09 |

---

<a id="s2"></a>
## 2. Screens

<a id="s2-1"></a>
### 2.1 Base and viewport (Confirmed, CS1)

Layout `chargen`: `CharGen_Base_Field` (1440 × 900 design size, z 250) holds
`chargen_viewport` (a full-screen `Viewport` with shadows on, bloom 0.05, ambient light
0xFF281E19, fake shadow colour 0xFF61617D, deshade 0.08, rim lighting 1.0, particles on) behind
`chargen_foreground` (the stage field `CharGen_Stage_Field` where the screens below are
placed; `ControlsParent`, the controls shared by the creation stages, §2.3; and an error-text
list `CharGen_ErrorTextList` built from `CharGenErrorTextTemplate`). The viewport shows the
creation scene: the race's `RaceTable_UIScene` (a 0x21 record, `misc-records.md`) with the
cameras of `RaceTable_Chargen_Camera` / `RaceTable_Character_Select_Camera` and their zoom
variants in the gender entry (`player-character.md` §5.2–§5.3).

`CharGen_Base_Field` also holds `GamePlay_ExaminationTooltip_Activatable` (a `Field`,
192 × 128 at 571, 12, CS15), by its label the in-world examination tooltip's anchor. **Tessera
rule (P1):** never created on these screens (they have no examinable objects).

<a id="s2-2"></a>
### 2.2 Screens and stages (Confirmed, CS2)

| Stage (mapper key, name) | Layout | Root |
|---|---|---|
| 4 `CharacterSelect` | `chargen_characterselect` | `CharGen_CharacterSelect_Base` |
| 7 `RaceClass` | `chargen_raceclass` | `CharGen_RaceClass_Base` |
| 6 `Appearance` (with the name entry) | `chargen_appearance` | `CharGen_Appearance_Base` |
| 1 `ClassSelect`, 3 `Name` | — (no separate layout of this build) | — |

All screens scale with the window (`ui-layout.md` §5.8; CS10). Under each screen's root, the
left side panel (edge modes 5, 5, 6, 5) and the right side panel (6, 5, 5, 5) are siblings of
a scaling parent (6, 5, 6, 5) and anchor to the screen edges by their own edge modes; the
scaling parent holds the centre parts:

| Screen | Left panel | Right panel | Scaling parent and its centre parts |
|---|---|---|---|
| Selection | `CharGen_CharacterSelect_Panel` | `CharGen_Menu_Panel` | `CharGen_CharacterSelect_ScalingParent`: logo, the `CharGen_Temp_TooManyPlayers` notice, PvP lobby field, rename buttons (the other notice, `CharGen_Notice_FinishingLogout`, is a direct child of the root, CS15) |
| Race and class | `CharGen_RaceSelect_Panel` | `CharGen_ClassSelect_Panel` | `CharGen_RaceClass_ScaleParent`: logo, loading notice, locked-continue store button |
| Appearance and name | `CharGen_Appearance_Panel_Left` | `CharGen_Appearance_Panel_Right` | `ControlsParent` (this layout's own): create, race-change, advanced-start and name-entry panels |

> **OPEN:** the original's order of stages and its back navigation. **Tessera rule:** §6.

<a id="s2-3"></a>
### 2.3 Shared controls (Confirmed data, CS8; stage rules are Tessera rules)

`ControlsParent` of layout `chargen` (a child of `chargen_foreground`, full design size,
edge modes 6, 5, 6, 5, object mode stretch) holds the controls that stay on screen across the
stages. Positions are in design units (1440 × 900) relative to `ControlsParent`.

| Element | Base element (layout) | Box (x, y, w × h) | Data | Function |
|---|---|---|---|---|
| `CharGen_Back_Button` (label child `CharGen_Back_Button_Label`) | `CharGen_Button_ScreenNav_Left` (0x22000004) | 386, 810, 261 × 45 | rollover; tooltip on; activate on show | back one stage |
| `CharGen_Exit_Button` (label `CharGen_Exit_Button_Label`) | `CharGen_Button_ScreenNav_Left` (0x22000004) | 386, 810, 261 × 45 (the same box as Back) | rollover | leave creation |
| `CharGen_Continue_Button` (label `CharGen_Continue_button_Label`) | `Chargen_Button_ScreenNav_Right` (0x22000004) | 790, 810, 261 × 45 | rollover; tooltip on; `UICore_Button_broadcast_when_ghosted` true | next stage |
| `Chargen_Controls` (field, edge modes 1, 0, 1, 0) | — | 0, 780, 100 × 100 | one image item | turntable group |
| — `CharGen_Turntable_Left_Button` | `Chargen_Right_Button` (0x22000004) | 9, 23, 28 × 48 in the group | hot button, first repeat interval 0.05 s | turn the figure (left-hand button) |
| — `CharGen_Turntable_Right_Button` | `Chargen_Left_Button` (0x22000004) | 62, 23, 28 × 48 | hot button, first repeat interval 0.05 s | turn the figure (right-hand button) |
| — `CharGen_Turntable_Zoom_In_Button` | `Chargen_Plus_Button` (0x22000004) | 37, 12, 25 × 25 | tooltip on | zoom in |
| — `CharGen_Turntable_Zoom_Out_Button` | `Chargen_Minus_Button` (0x22000004) | 37, 56, 25 × 25 | tooltip on | zoom out |
| `CharGen_AvatarUpdate_Toggle` (field, z −10) | — | 420, 782, 200 × 20 | — | the `GOP_AvatarUpdateVisible` check box (§5.2) |
| — `CharGen_AvatarUpdate_Toggle_Label` | `fieldlabelsmall` (`text`) | 40, 0, 172 × 20 | text from table 0x250001BD | label |
| — `CharGen_AvatarUpdate_Toggle_Button` | `Button_Checkbox_03` (0x22000004) | 19, 2, 16 × 16 | — | check box |

The two turntable buttons are based on the opposite-named base elements (the left-hand button
on `Chargen_Right_Button`); the data, not the base name, decides the position. The layout
`chargen_appearance` has its own field labelled `ControlsParent` (same element ID, a different
layout) whose `CharGen_Name_Create_Button` and `CharGen_Name_RaceChange_Button` occupy the
Continue box (790, 810, 261 × 45, base `Chargen_Button_ScreenNav_Right`).

Which stage shows which control is game code, not data. **Tessera rule (P1):**

| Control | Selection (stage 4) | Race and class (7) | Appearance and name (6) |
|---|---|---|---|
| Back | hidden | shown (→ selection) | shown (→ race and class) |
| Exit | hidden | hidden | hidden |
| Continue | hidden | shown (→ appearance and name) | hidden (the create button takes its box, §5.4) |
| Turntable group | shown | shown | shown |
| Avatar-update toggle | hidden | shown while the chosen species is 0x49 or 0x51 | shown while the chosen species is 0x49 or 0x51 |

- **Exit** is hidden in P1 because Back and the selection screen's quit button cover leaving;
  it shares Back's box, so the two are never shown together.
- **Continue** is ghosted (disabled look) while the race, gender and class choice is not valid
  by `player-character.md` §5.4. Because the data sets `UICore_Button_broadcast_when_ghosted`,
  a click on the ghosted button still reaches the screen, which then adds an error line to
  `CharGen_ErrorTextList` saying that the choice is incomplete (Tessera's own text) and stays.
- **Turntable:** while a turn button is held, the figure turns about its vertical axis at
  90° per second (left-hand button: the figure's front turns to the viewer's left; right-hand
  button: to the right). Both are hot buttons in the data; Tessera reads the hold as
  continuous turning that starts once the button has been held for the element's
  `UICore_Button_hot_click_first_interval` (read from the data, 0.05 s here), and a press
  released earlier turns the figure 15°. (No spec yet defines hot-button repeat in general;
  this rule covers only these two buttons.) Zoom in switches to the zoom variant of the current camera
  (`RaceTable_Chargen_Camera` or `RaceTable_Character_Select_Camera`, §2.1); zoom out switches
  back; repeated clicks in the same direction do nothing. The turn angle resets to 0 when the
  stage, race or gender changes.
- **Avatar-update toggle:** the check box shows and writes `GOP_AvatarUpdateVisible`
  (`appearance.md` §3.3: a graphics option kept in Tessera's settings, default on; the figure
  rebuilds when it changes). The species condition is the data-backed part (`appearance.md`
  §3.3, BIN2 there); the stages it appears on are the Tessera rule above.

> **OPEN:** the original's stage-by-stage visibility of these controls, its turn speed and
> step, and when it offers Exit (owner check O85).

---

<a id="s3"></a>
## 3. Character selection (Confirmed, CS3)

| Part (element label) | Content | P1 (Tessera rule) |
|---|---|---|
| `CharGen_CharacterSelect_Panel` (left) | title box (`CharGen_CharacterSelect_Label`, the account type and slots-available labels), the character list `CharGen_CharacterSelect_Listbox` with a page control and a sort menu | shown; slots label: "used / allowed" from the session's list and Tessera's limit (64, `scenario-feed.md` §5.1) |
| `CharGen_CharacterSelect_New_Button` | new character → §4 | shown; enabled or ghosted per backend (§7.1) |
| `NewCharacterSlot_Button` | buy a slot (store) | hidden |
| `CharGen_CharacterSelect_Delete_Button` | delete the selected character (§7.2) | shown; enabled only on a backend that can answer a deletion (§7.2) |
| `CharGen_CharacterSelect_RaceChange_Button`, `CharGen_RenameCharacter_Button`, `CharGen_RenameCharacter_BuyNow` | race change, rename (store or account services) | hidden |
| `CharGen_Menu_Panel` (right) | store button, monster-play button, enter button (`CharGen_CharacterSelect_Enter_Button`, large), credits, movie library, options, quit, plugin manager buttons | enter, options, quit, plugin manager shown; others hidden |
| `CharGen_PVP_Lobby_HideWhenField` | PvP lobby | hidden |
| `CharGen_Notice_FinishingLogout`, `CharGen_Temp_TooManyPlayers` | notices | shown when applicable (Tessera's own texts) |
| `ECommBrowser` (top-level element of the layout, a `Browser`, 100 × 100, CS15) | by its label an embedded web view for the store (reading of the label, not traced) | never created in P1 (Tessera embeds no web content) |
| `CharSelect_ChooseAvailableCharacters_Dialog` (top-level element of the layout, with its entry template `CharSelect_ChooseAvailableCharacters_Entry`: a check box and a name, an Ok entry and a text line) | a dialog with a scrolling list of characters, each with a check box, and an Ok button; by its labels an account-services choice of which characters stay available (reading of the labels, not traced) | never created in P1 |

**Character row** (`CharGen_CharacterSelect_Character_Template`, based on
`Chargen_Character_Slot`, 338 × 71; `CharGen_CharacterSelect_AdvancedCharacter_Template` for
advanced-start characters):

| Child | Shows |
|---|---|
| `CharGen_CharacterSelect_Character_Name` | the character's name (truncated with a tooltip) |
| `CharGen_CharacterSelect_Character_Description` | level, race and class (CS11, Probable: the three-variable format 0x0BB533CE of table 0x250001BD, variables 0x50AA9C level, 0x56575 race name, 0x480683 class name; formats with level only and level and race exist for unknown race or class) |
| `CharGen_CharacterSelect_Character_LastLocation` | the last location's name (markup allowed) |
| `CharGen_CharacterSelect_Character_Class` | the class icon (48 × 48), `AdvTable_ClassInfo` / `AdvTable_ClassIcon` |

The selected character stands in the viewport: its race's scene and
`RaceTable_Character_Select_Camera`, in its appearance (`appearance.md` §8).

> **OPEN:** list order, sort-menu choices and which character is selected first. **Tessera
> rule:** the session's order; the first entry, or the one entered last in this run, is
> selected; a click selects, a double click enters.

> **OPEN:** (implementer note, `apps/client/src/screens`, 2026-10-10) The description
> format CS11 is Probable only and its variable names are not tied to the row. As
> implemented: the row's `CharGen_CharacterSelect_Character_Description` shows Tessera's
> own line "Level *n* *race* *class*" with the parts the session told (race: the
> `RaceTable_Gender_Race_Name` of the row's body; class: `AdvTable_ClassName`). Please
> confirm the format and its variables, or give the rule.

---

<a id="s4"></a>
## 4. Race and class (Confirmed, CS4, CS6)

<a id="s4-1"></a>
### 4.1 Race list and gender

`CharGen_Race_Select_Panel` (left): the title, the gender group (`CharGen_Male_Button`,
`CharGen_Female_Button`) and the race list `CharGen_Race_Select_Listbox`, one
`CharGen_Race_Select_Race_Template` button (261 × 45: icon 48 × 48, name, a store coin button,
a disabled overlay) per race.

| Item | Record (`player-character.md`) |
|---|---|
| Races | the race tables of the race directory's `RaceTable_RaceTableList` (§5.1), in stored order (**Tessera rule**: this order) |
| Race button icon and name | the selected gender entry's `RaceTable_RaceSelect_Icon` and `RaceTable_Gender_Race_Name` (§5.3) |
| Gender buttons | one per entry of the race's `RaceTable_GenderList` by `RaceTable_GenderType`; a race with one entry offers one gender (the other button disabled) |
| Race information (`Chargen_Race_Info_Panel`) | `RaceTable_Description`; racial traits from `AdvTable_RaceCharacteristic_List` (§5.2) as `CharGen_Race_Select_Racial_Trait_Template` rows (`TraitDisplay`: trait icon and name). Which traits the screen lists (all, or only those of a low `AdvTable_Trait_Level`) was not traced; **Tessera rule:** all, by ascending level, then stored order |
| Race movie button | `RaceTable_RaceSelect_Movie_DID` (P1: hidden) |
| Disabled overlay, store coin | `RaceTable_CharGen_ClassSelect_WebstoreItem` (store) — **Tessera rule:** every race enabled |
| Viewport | `RaceTable_RaceSelect_Entity` of the gender entry in the race's scene, camera `RaceTable_Chargen_Camera` |

<a id="s4-2"></a>
### 4.2 Class list

`CharGen_ClassSelect_Panel` (right): the class list `CharGen_Class_Select_Listbox` of
`CharGen_Class_Select_Class_Template` buttons (icon, name, store coin, disabled overlay) and
`CharGen_Class_Info_Panel` (icon, title, description, movie button).

| Item | Record |
|---|---|
| Classes | the race's `RaceTable_ClassList` (§5.2), ordered by ascending `CharGen_ClassPriority_PriorityValue` (§7.3, Decision D3) |
| Button icon and name | the class table's `AdvTable_ClassInfo`: `AdvTable_ClassIcon`, `AdvTable_ClassName` (`combat-data.md` §9.1) |
| Information | `AdvTable_ClassName`, `AdvTable_ClassDesc`, `AdvTable_ClassIcon`; background `AdvTable_CharGenBackground` (§7.4) |
| Movie | `AdvTable_CharGen_ClassSelect_Movie_DID` (P1: hidden) |
| Disabled overlay | classes gated by store items or account tokens (§7.4) — **Tessera rule:** every class of the race enabled |

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) the element labels of the race and class
> buttons' parts. §4.1 and §4.2 say the buttons show an icon and a name but do not name the
> parts; a review probe of the install reported `CharGen_Race_Select_Race_Name` and
> `CharGen_Race_Select_Race_Icon` in `CharGen_Race_Select_Race_Template`, and
> `CharGen_Class_Select_Class_Name` and `CharGen_Class_Select_Class_Icon` in
> `CharGen_Class_Select_Class_Template` (the template roots are Buttons with a full-size
> background and frame pieces). The parts of `CharGen_Nationality_Entry_Template` (§5.1)
> are not named either. As implemented: the parts are looked up by those labels; when a
> label does not resolve, **Tessera rule:** the label goes to the first `Text` element
> below the entry (never the entry's own Button) and the icon to the first square element
> below it that is not of the entry's own size. Please confirm the labels and name the
> nationality entry's parts.

Changing race keeps the class if the new race allows it, else selects the first class of the
list (**Tessera rule**). A choice is valid by `player-character.md` §5.4.

> **OPEN:** (implementer note, `apps/client/src/screens`, 2026-10-10) The parts of
> `Chargen_Race_Info_Panel` and `CharGen_Class_Info_Panel` are not named, nor the list that
> holds the `CharGen_Race_Select_Racial_Trait_Template` rows, nor where a trait's icon and
> name come from (the race table's `AdvTable_Trait_WC` names a trait record; its name and
> icon properties are not stated). As implemented: the texts go to the `Text` elements below
> the panel that the layout gives no text, in tree order (race panel: the description;
> class panel: the name, then the description); the class icon, the movie button and the
> trait rows are not shown. Please name the parts and the trait record's fields.

<a id="s4-3"></a>
### 4.3 Other elements of the race-and-class screen

| Element | Data | P1 (Tessera rule) |
|---|---|---|
| `CharGen_ContinueLocked_BuyNowButton` (in the scaling parent, 241 × 41 at 600, 666, based on `CharGen_Class_BuyNow` of layout 0x22000837; label and coin children) | a store button in place of Continue for a locked choice; hidden by its own data (CS12) | never shown (every choice is enabled, §4.1–§4.2) |
| `CharGen_Avatar_Loading_CenterScreen` (160 × 100 at 640, 399, based on `Chargen_Silver_Border` of layout 0x22000001 — a nine-part border of eight edge pieces and a background, CS15 — with a translucent dark fill over it, label child `AvatarLoading_Label`) | hidden by its own data (CS12) | shown while the figure for the current choice is still loading (`appearance.md` §8: assets not yet resident), hidden when it is drawn |
| `CharGen_Avatar_Loading_RaceBlocker` (276 × 361 at 39, 70 in the left panel, z −10, based on the same `Chargen_Silver_Border` with the same translucent dark fill, CS15) | hidden by its own data, blocks clicks (CS12) | shown over the race list for the same time as the notice, so no new race is chosen while a figure loads |

---

<a id="s5"></a>
## 5. Appearance and name (Confirmed, CS5, CS6)

<a id="s5-1"></a>
### 5.1 Nationality

`CharGen_Nationality_Select_Panel` (left): list of `CharGen_Nationality_Entry_Template` buttons
(one per key of the race's `RaceTable_NationalityList`, in stored order); the information
panel shows the selected nationality's `NationalityTable_Name`, `NationalityTable_Icon`,
`NationalityTable_Desc` and its naming guideline for the chosen sex
(`NationalityTable_Naming_Guideline_Array`, `player-character.md` §6.2). The nationality's
dye-map overrides apply to the colour controls (§6.2 there, OPEN Q14; **Tessera rule:** the
override replaces the control's palette image).

<a id="s5-2"></a>
### 5.2 Appearance controls

`CharGen_Appearance_Panel` (right): the title, the list `CharGen_Appearance_Controls_Listbox`
and the randomise button. The list holds the gender's player avatar's `AppearanceUI_Controls`
(`appearance.md` §3.4) whose `AppearanceUI_Availability` has the `CharGen` bit, in stored
order, grouped under `AppearanceUIGroup` headers by `AppearanceUI_Group` (`Skin`, `Head`,
`Face`, `Eyes`, `Hair`, `Mouth`). Each control is shown by the template of its kind:

| Control struct (`appearance.md` §3.4) | Template element of `chargen_appearance` | Writes |
|---|---|---|
| `AppearanceUI_APRControl` | `CharGen_APRAppearanceChoice_BaseField` (253 × 40) | a style index for the APR key (head: `AC_Mesh_Head`) |
| `AppearanceUI_CompositorControl` | `CharGen_CompositorAppearanceChoice_BaseField` | a face layer choice (`appearance.md` §3.5) |
| `AppearanceUI_DyeMapControl` | `CharGen_DyeMapAppearanceChoice_BaseField` (253 × 80) | a colour into the control's `AC_Color_*` property, picked from the palette image |
| `AppearanceUI_BoneScalingControl` | `CharGen_BoneScalingAppearanceChoice_BaseField` (253 × 74) | a float in [`…MinFloatControlValue`, `…MaxFloatControlValue`] into `AC_BoneScale_Test` (and `AC_Avatar_Height` where named) |
| `AppearanceUI_BoneSliderControl` | `CharGen_BoneSliderSystemChoice_BaseField`, `…GroupChoice…`, `…AppearanceChoice…` | slider bytes into `AC_BoneSlider_Value_Array` (`appearance.md` §6.3) |
| `AppearanceUI_PresetsControl` | `CharGen_PresetsAppearanceChoice_BaseField` (253 × 52) | a whole preset from `Chargen_AvatarPresetsArray` |

The `GOP_AvatarUpdateVisible` check box (`CharGen_AvatarUpdate_Toggle` of `chargen`, §2.3)
appears only for species 0x49 and 0x51 (`appearance.md` §3.3, BIN2 there).

`CharGen_Appearance_Nationality_Text` (a text line at the top of `CharGen_Appearance_Panel`,
edge modes 1, 5, 1, 5): **Tessera rule:** shows the selected nationality's
`NationalityTable_Name` (§5.1).

> **OPEN:** the randomise rule and each template's inner controls (arrows, swatches, slider
> ranges). **Tessera rule:** randomise picks, per control, a uniformly random choice (styles:
> any index; colours: any palette pixel; floats: uniform in range; sliders: the preset of a
> random preset entry if the avatar has presets, else unchanged), from a generator seeded by
> the time; each template is a labelled row with previous/next arrows (styles, presets), a
> swatch grid (colours) or a slider (floats).

> **OPEN:** (implementer note, `crates/gamedata/src/chargen`, 2026-10-10) "In stored order,
> grouped under `AppearanceUIGroup` headers" does not say how the headers are ordered when a
> group's controls are not adjacent. As implemented: groups in the order of their first
> control, controls in stored order within a group, controls without `AppearanceUI_Group`
> under one header-less group. Likewise §4.2: a class key without a
> `CharGen_ClassPriority_Entry` (none in this install) is listed after the prioritised
> classes, in stored order. Two more Tessera rules are implemented without a spec line:
> (a) when the install's `AppearanceUI_Availability` mapper has no `CharGen` label, every
> control is offered at creation (logged), including controls available only for appearance
> modification; (b) after a race change (§4.1, §6) the gender is kept when the new race has a
> gender entry of it, else the new race's first gender entry is selected (as for classes in
> §4.2). Please confirm both.

> **OPEN:** (implementer note, `apps/client/src/screens`, 2026-10-10) The "Writes" column
> does not settle every property a look entry can name (decision 0025 D2 carries only
> property values): (1) a style control without `AppearanceUI_ControlScriptPropertyName`
> (hair, beard, ears) names no property, so its choice cannot travel in a look; (2) "and
> `AC_Avatar_Height` where named" does not say where the second property is named; (3) the
> number of slider bytes of a slider control is not stated. As implemented: (1) such a
> style is applied to the creating client's own figure only and is not kept in the
> settings either (they store look entries); (2) a body-scale control writes the property
> its control struct names in `AppearanceUI_ControlPropertyName`, else `AC_BoneScale_Test`;
> (3) one byte per pair of the avatar's own `AC_BoneSlider_Value_Array` (or the property's
> dictionary default), written as upper-case hex pairs. Colours travel as RGBA (the data's
> B, G, R, A order is converted). Please give the rule.

> **OPEN:** (implementer note, `apps/client/src/screens`, 2026-10-10, after review) (4) The
> head control's `AppearanceUI_ControlScriptPropertyName` is `AC_Mesh_Head`, which the
> dictionary types as a float, and the block variants of style entries are not 0.00,
> 0.01, … (for example 0.00–0.04, 0.30–0.34, 0.40–0.44 for one head entry; hair lists
> 0.30, 0.301, 0.302, …). As implemented: style choice *i* is the *i*-th block of the APR
> entry (blocks of equal variant merged, NaN dropped); the figure's modifier is that
> block's own `variant`, so `appearance.md` §3.2.1 rule 2 selects exactly that block; a
> float style property (`AC_Mesh_Head`) is written as that `variant`, an integer one as
> the index; a received value is mapped back to the block with exactly that variant, else
> to the block rule 2 selects. Please confirm the value written to `AC_Mesh_Head` and the
> order of the choices (the `AppearanceUI_EndOldOptionsControlValue` /
> `AppearanceUI_OldOptionsWeightValue` ordering of `appearance.md` §3.4 is not applied).

> **OPEN:** (implementer note, `apps/client/src/screens`, 2026-10-10) Element labels the
> screens need but §3–§5 do not name: on `CharGen_Menu_Panel` the quit, options and plugin
> manager buttons (§3 "shown"), the account-type and slots-available labels of the title
> box, the randomise button of `CharGen_Appearance_Panel`, the `AppearanceUIGroup` header
> element of the control list, the parts of `CharGen_Nationality_Entry_Template` and of the
> nationality information panel, and the inner parts of the six control templates. As
> implemented: those elements are left as the layout creates them (quit is the client's
> Escape), nationality entries get the fallback parts rule of the §4.2 OPEN, controls get
> the fallback widgets of the §5.2 OPEN, and Tessera's delete confirmation (§7.2) is an
> error-list line answered by a second click on Delete. Please name the labels.

<a id="s5-3"></a>
### 5.3 Name

`CharGen_Name_Entry_Panel` (top): the name field, the random-name button
(`CharGen_Name_RandomNameGenerator`) and the tutorial choice (`CharGen_Name_ChangeTutorial`
with its label and current choice).

The name field `CharGen_Name_Entry_Field` (268 × 28 at 8, 0 in its parent, edge modes 1, 0,
1, 0) has no widget type of its own: it carries a **base reference** (`ui-layout.md` §5.6) to
the element labelled `ChargenNameTextEntry` in layout 0x22000002 (`UILAYOUT` label `text`),
which is a `Text` element (391 × 51, its own font, outline on, six text states). The field
overrides its base with `UICore_Text_editable`, `UICore_Text_selectable` and
`UICore_Text_one_line` set to true and left/centre justification (CS9). So the field is an
editable one-line `Text` element; `ChargenNameTextEntry` is the base element's label, not an
element type (none of that name exists).

Other elements of the panel: `CharGen_Name_Admin_Button` is hidden by its own data
(`UICore_Element_hide` true, CS12) and stays hidden; `CharGen_RandomName_Debug_Field` (based
on `RandomName_Debug_Field_Base` of layout 0x22000851) is a debugging aid of the name
generator and is never created in P1 (**Tessera rule**).

| Function | Rule |
|---|---|
| Random name | `player-character.md` §8.5 for the chosen race, sex and nationality |
| Name check before sending | `player-character.md` §8.6 and the name-rule control §8.1; the session's own check decides (§7.1) |
| Tutorial choice | offered when the race's `RaceTable_AllowSkipTutorial` is true (reading of the name); **Tessera rule:** hidden in P1 (the sandbox has no tutorial) |

> **OPEN:** (implementer note, `crates/gamedata/src/chargen`, 2026-10-10) The name check needs
> a rule-set key and a context bit for character names and an order of the checks
> (`misc-records.md` §9.3 OPEN (1), which lets Tessera choose). As implemented: the rule set
> of the first key of the generator's `RandomNameGeneration_ValidNameRules` that the
> name-filter table holds, the lowest bit of its `letter_classes` as context, the string
> check (§9.1 there) on the name as typed, then the name check (§9.2 there). The taboo check
> (§10.1 there) is not applied (its tokenizer is OPEN and the workspace has no
> PCRE-compatible matcher). Letter, digit and printable are approximated by the standard
> library's Unicode properties (alphabetic; numeric and not alphabetic; not a control and not
> a common format character). Please confirm the choice or give the rule.

<a id="s5-4"></a>
### 5.4 Create buttons

`CharGen_Name_Create_Button` creates the character (§7.1). `CharGen_Name_AdvancedStart_Panel`
offers a level-one or an advanced start (`player-character.md` §7.2, store-gated); **Tessera
rule:** hidden, every character starts at level one. `CharGen_Name_RaceChange_Button` replaces
the create button in the race-change service; hidden.

---

<a id="s6"></a>
## 6. Flow (Tessera rule, CS7)

1. **Select** (§3). New → 2. Enter → world entry (§7.3). Delete → §7.2.
2. **Race and class** (§4). Default selection: the race and gender of the selected character
   if any, else the first race and its first gender; the first class. Continue (`CharGen_Continue_Button`, §2.3) → 3; back
   (`CharGen_Back_Button`) → 1.
3. **Appearance and name** (§5). Default appearance: the avatar's own values (`appearance.md`
   §3.4: "the avatar entity's own values are the defaults"); default nationality: the first of
   the list; empty name. Create (`CharGen_Name_Create_Button`) → §7.1; back (`CharGen_Back_Button`) → 2 (the appearance choices are kept while the
   race and gender stay the same, else reset).

The viewport follows the selection at once (figure rebuilt by `appearance.md` §8 on every
change of a control).

> **OPEN:** (implementer note, `apps/client/src/screens`, 2026-10-10, after review; not a
> spec gap, an implementation gap recorded so that P1 criterion 1 is not claimed early)
> (1) The client's figure path (`walker::characters::figure`) takes only the
> `AC_Avatar_Height` and `AC_BoneScale_Test` values of a creation figure, so styles,
> colours, face layers and slider strings reach the assembler's `CharacterSpec` (the
> screens build it) but not the drawn viewport figure. (2) The client's start-up does not
> open these screens yet: they run headless (tests, the scenario driver) only, so no
> screenshot of them exists. Both are to be closed in the client before P1 criterion 1 is
> claimed. (3) The §7.1 settings key of a test server is its connect string, which the
> start-up must pass; without it (the display name is the same for every server by
> default) the choices are neither kept nor applied.

> **OPEN:** (implementer, `apps/client/src/pre_world.rs`, 2026-10-10; follow-up of the note
> above) Item (2): `tessera-client --character-select` (windowed, with the game UI) now opens
> these screens at start-up: the world session does not enter by itself and is polled by the
> screens until `WorldEntered`, then the walker continues; Escape on the screens quits the
> client. Item (3): the start-up passes `CONNECT#ACCOUNT` (the `--server` connect string and
> the `--account` name) as the settings key. Item (1) stays open: the viewport's figure is not
> drawn. Not specified, as implemented: the screens are not offered in headless runs, with
> `--scenario` or with `--character`.

---

<a id="s7"></a>
## 7. Commands and answers

<a id="s7-1"></a>
### 7.1 Creating

On every backend the screen issues one command, `tessera-net`'s `CreateCharacter { name,
appearance, class }` (`scenario-feed.md` §9.4): `appearance` =
`Appearance::new(Body::Character { species, sex })` with the chosen race table's species and
the gender entry's sex value, `class` = the class key. The backend carries it (on M7's server
as `CharacterCreate`, §7.1.1) and turns the answer into the session events `CharacterInfo`
(success) or `CharacterRefused` (failure); the screen sees only these events.

| Backend | New button (`CharGen_CharacterSelect_New_Button`) | Success | Failure |
|---|---|---|---|
| Scenario feed | enabled | `CharacterInfo` with that name (an `accept-character` step of an `on create-character` trigger) | `CharacterRefused { name, reason }` (a `refuse-character` step) |
| Plain sandbox (no scenario) | enabled | none (§8 OPEN) | none |
| M7 server (`remote` backend) | enabled when the server advertised `CHARACTER_CREATION`, else ghosted (§7.1.1 step 1) | `CharacterInfo` for the new name (§7.1.1 step 3) | `CharacterRefused` with Tessera's own text per code (§7.1.1 step 4) |

Screen rules (**Tessera rule**, CS7): Create (`CharGen_Name_Create_Button`) is ghosted from
the moment it sends until the answer or the time-out below. A `CharacterInfo` whose name
equals the requested name is success: the screen returns to the selection screen, adds the
row (if new) and selects it. A `CharacterRefused` for that name is failure: its `reason` is
shown in `CharGen_ErrorTextList` and the screen stays with every choice kept. A refusal of the
command itself (`NetError::InvalidCommand`, for example an invalid name or appearance under
`scenario-feed.md` §9.5) is shown the same way with Tessera's own text. With no answer within
10 s an error line is shown, Create is enabled again and the screen stays. A ghosted New button
keeps its tooltip, which says (Tessera's own text) that this server does not allow creating
characters.

<a id="s7-1-1"></a>
#### 7.1.1 Creation on M7's server (`remote` backend) (Tessera rule, CS14)

This is the normative rule for `CreateCharacter` on the test-server (`remote`) backend. It
narrows `scenario-feed.md` §9.5, whose blanket refusal of the new commands still holds for
`DeleteCharacter` and every other command of its §9.4 (OPEN below for the matching revision
there).

1. **Capability.** The backend keeps the `server_caps` of the server's `HelloAck`
   (`tessera-protocol.md` §6.2.2) and lets the screen read whether bit 1
   `CHARACTER_CREATION` is set (with the other server information it already exposes). The
   screen ghosts the New button when it is clear.
2. **Command.** The backend accepts `CreateCharacter` only when all of these hold: the session
   is `Connected` (the wire state CharacterSelect); `CHARACTER_CREATION` is set; `appearance`
   is present (the wire message requires one); `scenario-feed.md` §9.5's validation passes;
   and no `CharacterCreate` or `EnterWorld` of this connection is outstanding (a second one
   is fatal, `tessera-protocol.md` §13.1 "Outstanding requests"). Otherwise it refuses with
   `NetError::InvalidCommand` and sends nothing. When it accepts, it sends
   `CharacterCreate { name, appearance }` (§6.2.7) with the appearance in its §5.3 wire form,
   drops `class` (the protocol carries none: `LEGAL.md` §11.1 rule 5, no gameplay
   reproduction; `server-content.md` gives the checkable form) and records the name as the
   outstanding creation.
3. **`CharacterList` answer** (§6.2.8). The backend clears the outstanding creation. If the
   list holds an entry with the outstanding name, it emits one
   `CharacterInfo { name, body: Some(that entry's body), class: None, level: None }`. If it
   does not, it emits `CharacterRefused { name: the outstanding name, reason }` with the
   reason text Tessera uses for code 6 `SERVER_ERROR`. Other entries of the list emit nothing
   (in v1.0 a list changes only through this connection's own request; a difference is logged
   at debug level).
4. **`CharacterCreateFailed { code, text }` answer** (§6.2.9). The backend clears the
   outstanding creation and emits `CharacterRefused { name: the outstanding name, reason }`,
   `reason` being Tessera's own text for the code of `tessera-protocol.md` §14.4: one text
   each for 1 `NAME_TAKEN`, 2 `NAME_INVALID`, 3 `TOO_MANY`, 4 `APPEARANCE_REFUSED`,
   5 `DISABLED` and 6 `SERVER_ERROR`; any other code is treated as 6. The server's `text` is
   logged (info level), not shown.
5. **States.** Both events are character events and are surfaced while the session is
   `Connected` (`scenario-feed.md` §9.5 "States"). An answer that arrives after the session
   left `Connected` is dropped with a debug log; an answer with no outstanding creation is a
   message outside its state, handled as `tessera-protocol.md` §6.1 and §13 say.

> **OPEN:** (implementer note, `crates/net/src/remote`, 2026-10-10; review note (k)) Step 3
> does not say how "an entry with the outstanding name" is compared, and §7.1's success rule
> compares with the requested name. `test-server.md` §6.4 stores and lists the NFC form, so a
> name sent in another normal form is created yet missed by a byte comparison (reported as
> `SERVER_ERROR`; every retry then gets `NAME_TAKEN`). As implemented: the backend sends the
> name as given, looks for an entry with exactly that name, else for one whose NFC form equals
> the NFC form of the outstanding name, and emits `CharacterInfo` under the requested
> (outstanding) name, so the screen's rule holds; the server finds the character by its name
> key on `EnterWorld`. Also as implemented (step 2): the name is recorded as outstanding only
> once the command is queued for the network side; a `CharacterCreate` that the network side
> then cannot send is answered locally as `SERVER_ERROR`, and the end of the session clears
> the outstanding creation. Spec owner to confirm or choose another rule (for example: the
> backend sends the NFC form), here and in `scenario-feed.md` §9.7.

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D4):**
> `scenario-feed.md` §9.5 and §9.7 and `tessera-protocol.md` Appendix A now point to §7.1.1
> and map `CreateCharacter` → `CharacterCreate`, `CharacterList` → `CharacterInfo` /
> `CharacterRefused` and `CharacterCreateFailed` → `CharacterRefused`. The note as raised:
>
> **OPEN (required revision of `scenario-feed.md` §9.5 and `tessera-protocol.md` Appendix A,
> for their owner; does not block this spec):** §9.5 "Other backends" still says the
> test-server backend refuses every new command, `CreateCharacter` included, and Appendix A
> lists no `Event` for `CharacterList` and no `Command` for `CharacterCreate`. Both should
> point to §7.1.1 here: §9.5 excepting `CreateCharacter` under these conditions, Appendix A
> mapping `CreateCharacter` → `CharacterCreate`, `CharacterList` → `CharacterInfo` and
> `CharacterCreateFailed` → `CharacterRefused`. Until they are revised, §7.1.1 governs
> `CreateCharacter` on the `remote` backend.

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D2):**
> protocol 1.1 carries a **look** (up to 48 opaque property-ID and value pairs, at most 768
> bytes) beside the appearance: `tessera-protocol.md` §3.1, §5.3, §6.2.7, §7.4 (with the
> receiving client's rule), `test-server.md` §3.5 and §6.4 (storage), `server-content.md`
> §4.1 (R3, R7); `tessera-net`'s `Appearance.look` and `CreateCharacter.nationality`,
> `scenario-feed.md` §5.8, §9.1, §9.4, §9.7. The Tessera rule below still applies where no
> look is stored: on a minor-0 connection and across restarts of the plain sandbox. The
> note as raised:
>
> **OPEN (design, blocks the "look" part of P1 exit criterion 1 on M7's server):** the
> `Appearance` model carries a body (species and sex), a scale, worn items and an idle, but
> not the creation choices (styles, face layers, colours, height, sliders: the values of the
> properties the controls write, §5.2). So other clients, and the sandbox after a restart, see
> the race's default look. Proposal for a revision of `tessera-protocol.md` (a minor version)
> and the net model: an optional list of (property ID, value) pairs for the
> `AC_*` properties of §5.2, opaque to the server (`server-content.md` §4.1 R2 still applies),
> with limits stated there. **Tessera rule until then:** the client keeps the choices of the
> characters it created in its own settings, keyed by server and character name, and applies
> them to its own figure only.

> **OPEN:** (implementer note, `apps/client/src/walker/characters`, 2026-10-10) The walker's
> figures apply a received look under the receiver rule of `tessera-protocol.md` §7.4 (the
> player's own figure and every observed character): the accepted entries set the creation
> controls of §5.2 as the selection screen does, and the figure gets the heights
> (`AC_Avatar_Height`, `AC_BoneScale_Test`), the chosen styles and the slider string.
> The figure path now takes styles and the slider string too (`FigureLook`), which also
> serves item (1) of the §6 OPEN. Colour controls and face layers are not drawn:
> `tessera-mesh`'s `CharacterSpec` has no field for them. Until the assembler takes them,
> the "look" part of P1 exit criterion 1 holds for heights, styles and sliders only.

<a id="s7-2"></a>
### 7.2 Deleting

Only the scenario feed takes a deletion command (CS13). The delete button
(`CharGen_CharacterSelect_Delete_Button`, §3) is always shown; whether it is enabled, and
what a click does, depends on the session's backend (**Tessera rule**):

| Backend | Delete button | On click | Answer and screen effect |
|---|---|---|---|
| Scenario feed | enabled while a character is selected | Tessera's own confirmation dialog, then `DeleteCharacter { name }` | an `on delete-character` trigger fires (`scenario-feed.md` §7.7); the row is removed only on a removal event (OPEN below); with none the row stays and no error is shown |
| Plain sandbox (no scenario) | ghosted (disabled look), never sends | nothing (no dialog) | — (the sandbox would accept and ignore the command, `scenario-feed.md` §9.5) |
| M7 server (`remote` backend) | ghosted, never sends | nothing (no dialog) | — (`tessera-protocol.md` has no deletion message; the backend would refuse `DeleteCharacter` with `NetError::InvalidCommand`, `scenario-feed.md` §9.5) |

The ghosted button keeps its tooltip, which says (Tessera's own text) that this server does
not support deleting characters. If a `DeleteCharacter` is nevertheless refused by any backend
with `NetError::InvalidCommand` (for example a backend added later without a rule here), the
screen adds an error line to `CharGen_ErrorTextList` (Tessera's own text), keeps the row and
treats the button as ghosted for the rest of the session.

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D5):**
> `scenario-feed.md` §6.1 `remove-character` (in `on create-character` and `on
> delete-character` bodies) emits `CharacterRemoved { name }` (§9.2 there). The note as
> raised:
>
> **OPEN (for `scenario-feed.md`, non-blocking):** the feed has no step that removes a name
> from the character list, so a scenario cannot answer a deletion (`scenario-feed.md` §7.5;
> its §16 OPEN (3)). Proposal: a `remove-character STRING` step allowed in `on
> delete-character` bodies, emitting a new `CharacterRemoved { name }`. **Tessera rule until
> then:** the screen removes the row only on such an event; with none it keeps the row.

> **OPEN (for `tessera-protocol.md`, non-blocking):** M7's protocol has no message that
> deletes a character, so on Tessera's own server a created character cannot be removed from
> the client. Proposal for a minor version: a C→S deletion request by name answered by a new
> `CharacterList` (success) or a failure with a code (as `CharacterCreateFailed`). **Tessera
> rule until then:** the table above (button ghosted).

<a id="s7-3"></a>
### 7.3 Entering

Enter sends `EnterWorld` with the selected name; the screens close on `WorldEntered`.

---

<a id="s8"></a>
## 8. The scenario feed

| Feed (`scenario-feed.md`) | Screen effect |
|---|---|
| `Connected` with the character list (§5.1 there) | The selection list (names) |
| `CharacterInfo { name, body, class, level }` (pre-world) | The row's description (level, race from `body`, class), class icon, and the selected character's figure (body; default appearance values) |
| `CharacterRefused { name, reason }` | Error line (`reason` as `Text`, `item-tooltips.md` §11 for text kinds) |
| input `at pre create-character …`, `delete-character …` (§7.7 there) | Headless tests of the screens: the driver sends the same commands the screens send |
| `on create-character` with `accept-character` / `refuse-character` (§7.5, §6.1 there) | The scripted answers of §7.1 |

A scenario for P1 criterion 1 declares no characters, scripts `on create-character` →
`accept-character` with the same name and its class, species and sex, and enters with it
(`enter` setting); the test asserts that the screen sent a `CreateCharacter` whose appearance
resolves (§9).

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D5):**
> the plain sandbox answers `CreateCharacter` itself (`scenario-feed.md` §9.6, cases §14.10):
> success is a `CharacterInfo` with the command's body, class and look, so the §7.1 table's
> "none" for the plain sandbox is superseded by that rule. The note as raised:
>
> **OPEN (for `scenario-feed.md` §9.5, non-blocking):** the plain sandbox (no scenario)
> ignores `CreateCharacter`, so creating a character in an ordinary offline session does
> nothing. Proposal: the sandbox's generic answer adds the name (if valid and new) to its list
> and emits `CharacterInfo` with the command's body and class — Tessera's own acknowledgement,
> no game rule (decision 0024 §2.4). Until then offline creation needs a scenario.

---

<a id="s9"></a>
## 9. Real-data test (decision 0024 P1 exit criterion 1)

For the install at `TESSERA_GAME_DIR`:

1. Build the race list (§4.1). For every race table, every gender entry and every class key of
   the race's class list (`player-character.md` §5.2: 8 races, 15 gender entries, 69 class
   entries here), the screen model offers the choice, and `player-character.md` §5.4 accepts
   it.
2. For each (race, gender) the appearance-control list (§5.2) builds without error, and the
   default figure assembles (`appearance.md` §8; categories of `appearance.md` §10 for
   failures).
3. For each (race, gender, nationality) a random name is generated (`player-character.md`
   §8.5) or the generator's empty result is reported.
4. A headless scenario run creates one character per race and gender with the screen's
   command and enters the world with it.
5. Against Tessera's test server run in-process (`test-server.md`, `character_creation`
   true), the screen's `CreateCharacter` for one (race, gender) per race yields a
   `CharacterInfo` with that name and the command's body (§7.1.1 step 3), and `EnterWorld`
   with it succeeds; a second creation with the same name yields `CharacterRefused` (code 1);
   with `character_creation` false the New button is ghosted and nothing is sent. The look
   seen by other clients is limited by the §7.1 OPEN (Q7).

Each step reports per-choice results; a failure is a panic or an unreported error.

---

<a id="s10"></a>
## 10. Invariants

| ID | Invariant | Status |
|---|---|---|
| CSI1 | The four creation layouts and their root elements of §2.2 exist | Confirmed 2026-10-09 |
| CSI2 | Every race of the directory has at least one gender entry and one class (`player-character.md` §5.2) | Confirmed (PC) |
| CSI3 | Every class key of every race's list has a class table with `AdvTable_ClassInfo` icon and name | Confirmed (PC P10, `combat-data.md` §9.1) |
| CSI4 | Every gender entry's player avatar has `AppearanceUI_Controls` with at least one control available at creation | Confirmed (AP §3.4: 11–20 controls per avatar) |
| CSI5 | A created character's `Appearance` body is `Body::Character` with the species and sex of a gender entry | Design check |

---

<a id="s11"></a>
## 11. Open questions (summary) and owner checks

| # | Question | Section | Settles it |
|---|---|---|---|
| Q1 | Stage order and back navigation | §2.2, §6 | owner (O85) |
| Q2 | Selection list order and default | §3 | owner (O85) |
| Q3 | Race order; traits shown | §4.1 | owner (O85) |
| Q4 | Class order (`player-character.md` Q2) | §4.2 | owner (O85) |
| Q5 | Randomise and control widgets | §5.2 | owner (O86) |
| Q6 | Nationality dye override (`player-character.md` Q14) | §5.1 | owner (O86) |
| Q7 | Creation choices in `Appearance` | §7.1 | design decision (protocol minor); answered by decision 0025 D2 |
| Q8 | Deletion in scenarios; plain-sandbox creation | §7.2, §8 | `scenario-feed.md` owner; answered by decision 0025 D5 |
| Q9 | Deletion on M7's server | §7.2 | design decision (protocol minor) |
| Q10 | Stage visibility of back, continue, exit, turntable and avatar-update toggle; turn speed | §2.3 | owner (O85) |
| Q11 | `scenario-feed.md` §9.5 and `tessera-protocol.md` Appendix A to point to the `remote` creation rule | §7.1.1 | their owners (revision; §7.1.1 governs meanwhile); answered by decision 0025 D4 |

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (Review), for decision 0024 P1: screens and stages, selection row, race/gender/class lists and their records, nationality, appearance controls, name, create, flow, commands and answers per backend, scenario-feed driving, real-data test, open design gaps (creation choices in `Appearance`, deletion in the feed, plain-sandbox creation). |
| 2026-10-09 | Revision after the independent review of 2026-10-09 (changes requested); status stays Review. Blocking (1): new §2.3 names the shared controls of `chargen` under `ControlsParent` (back, exit, continue, the turntable group, the avatar-update toggle) with boxes, base elements and button data, and gives the P1 rule per stage, Exit hidden, the ghosted Continue, the turntable and the toggle; §2.1 and §5.2 point there. Blocking (2): §5.3 states that `CharGen_Name_Entry_Field` is an element with a base reference to the `Text` element `ChargenNameTextEntry` of layout 0x22000002, overridden to editable, selectable and one line. Blocking (3): §7.2 gives the delete rule per backend (scenario feed enabled; plain sandbox and M7 server ghosted, never send), the handling of an `InvalidCommand` refusal and a new protocol OPEN; §3 delete row follows. Non-blocking (a) §7.1 cites `LEGAL.md` §11.1 rule 5; (b) §2.2 corrected (side panels anchor by their own edge modes; per-screen table); (c) provenance row CS11 for the description format; (d) P1 rules for the admin and debug name elements (§5.3), the locked-continue store button and the loading blockers (new §4.3), the nationality text (§5.2), the choose-available-characters dialog (§3). Provenance rows CS8–CS13 added; §11 Q9, Q10; owner check O85 extended. (e) unchanged: `scenario-feed.md` links resolve after branch `p0` is merged. |
| 2026-10-09 | Revision after the independent re-review of 2026-10-09 (changes requested); status stays Review. Blocking R2-1, answered by option (i): §7.1 now states that the screens issue `CreateCharacter` on every backend and see only `CharacterInfo` / `CharacterRefused`, with the New button rule per backend and the Create, refusal and time-out rules; new §7.1.1 gives the `remote` backend's rule (capability `CHARACTER_CREATION` from `HelloAck`, acceptance conditions, `CharacterCreate` with the class dropped, `CharacterList` → `CharacterInfo`, `CharacterCreateFailed` → `CharacterRefused` with Tessera's text per §14.4 code, states) as the rule that narrows `scenario-feed.md` §9.5, with an OPEN for the matching revision of that section and of `tessera-protocol.md` Appendix A; provenance row CS14; §3 New row, §9 step 5 (M7 part of P1 criterion 1) and §11 Q11 follow. Non-blocking: (f) `CharGen_ContinueLocked_BuyNowButton` added to CS12 and §4.3; (g) §2.2 selection row: `CharGen_Notice_FinishingLogout` is a direct child of the root; (h) `ECommBrowser` (§3) and `GamePlay_ExaminationTooltip_Activatable` (§2.1) never created in P1; (i) the **Probable** mark and the **Spec** source defined in the §1 key; (j) §4.3 loading blockers named as based on `Chargen_Silver_Border`. Provenance row CS15 (OBS, re-checked with the value-decoding probe 2026-10-09). (e) unchanged. |
| 2026-10-10 | Pointers only (no fact changed): the OPENs of §7.1 (look), §7.1.1 (Q11), §7.2 (feed removal) and §8 (plain-sandbox creation) and rows Q7, Q8, Q11 point to their answers in decision 0025 (`scenario-feed.md` revision 4, `tessera-protocol.md` v1.1). The §7.2 protocol-deletion OPEN (Q9) stays open. |
