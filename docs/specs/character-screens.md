<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Character selection and creation screens

| | |
|---|---|
| **Status** | Review (first version, 2026-10-09) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Pending (an independent spec-review session that wrote no part of this spec) |
| **Review record** | — |
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

| # | Fact | Source | Mark |
|---|---|---|---|
| CS1 | The creation screens are four layouts over one full-screen base with a 3D viewport (§2) | OBS | Confirmed 2026-10-09 |
| CS2 | The stage mapper names `CharacterSelect`, `RaceClass`, `ClassSelect`, `Appearance`, `Name` (§2.2) | OBS | Confirmed |
| CS3 | Character selection screen parts and the character row template (§3) | OBS | Confirmed |
| CS4 | Race-and-class screen parts: race list, gender buttons, class list, race and class info, racial traits (§4) | OBS | Confirmed |
| CS5 | Appearance-and-name screen parts: nationality list and info, appearance controls list, randomise button, name entry, random-name button, tutorial choice, create buttons (§5) | OBS | Confirmed |
| CS6 | The records that fill each list (§4, §5) | PC, AP; OBS | Confirmed (records), Decision (display order where untraced) |
| CS7 | The flow between screens and the commands sent (§6, §7) | Design | Decision |

---

<a id="s2"></a>
## 2. Screens

<a id="s2-1"></a>
### 2.1 Base and viewport (Confirmed, CS1)

Layout `chargen`: `CharGen_Base_Field` (1440 × 900 design size, z 250) holds
`chargen_viewport` (a full-screen `Viewport` with shadows on, bloom 0.05, ambient light
0xFF281E19, fake shadow colour 0xFF61617D, deshade 0.08, rim lighting 1.0, particles on) behind
`chargen_foreground` (the stage field where the screens below are placed, and an error-text
list `CharGen_ErrorTextList` built from `CharGenErrorTextTemplate`). The viewport shows the
creation scene: the race's `RaceTable_UIScene` (a 0x21 record, `misc-records.md`) with the
cameras of `RaceTable_Chargen_Camera` / `RaceTable_Character_Select_Camera` and their zoom
variants in the gender entry (`player-character.md` §5.2–§5.3).

<a id="s2-2"></a>
### 2.2 Screens and stages (Confirmed, CS2)

| Stage (mapper key, name) | Layout | Root |
|---|---|---|
| 4 `CharacterSelect` | `chargen_characterselect` | `CharGen_CharacterSelect_Base` |
| 7 `RaceClass` | `chargen_raceclass` | `CharGen_RaceClass_Base` |
| 6 `Appearance` (with the name entry) | `chargen_appearance` | `CharGen_Appearance_Base` |
| 1 `ClassSelect`, 3 `Name` | — (no separate layout of this build) | — |

All screens scale with the window: each has a scaling parent field with edge modes (6, 5, 6, 5)
that keeps the side panels at the screen edges (`ui-layout.md` §5.8).

> **OPEN:** the original's order of stages and its back navigation. **Tessera rule:** §6.

---

<a id="s3"></a>
## 3. Character selection (Confirmed, CS3)

| Part (element label) | Content | P1 (Tessera rule) |
|---|---|---|
| `CharGen_CharacterSelect_Panel` (left) | title box (`CharGen_CharacterSelect_Label`, the account type and slots-available labels), the character list `CharGen_CharacterSelect_Listbox` with a page control and a sort menu | shown; slots label: "used / allowed" from the session's list and Tessera's limit (64, `scenario-feed.md` §5.1) |
| `CharGen_CharacterSelect_New_Button` | new character → §4 | shown |
| `NewCharacterSlot_Button` | buy a slot (store) | hidden |
| `CharGen_CharacterSelect_Delete_Button` | delete the selected character (§7.2) | shown |
| `CharGen_CharacterSelect_RaceChange_Button`, `CharGen_RenameCharacter_Button`, `CharGen_RenameCharacter_BuyNow` | race change, rename (store or account services) | hidden |
| `CharGen_Menu_Panel` (right) | store button, monster-play button, enter button (`CharGen_CharacterSelect_Enter_Button`, large), credits, movie library, options, quit, plugin manager buttons | enter, options, quit, plugin manager shown; others hidden |
| `CharGen_PVP_Lobby_HideWhenField` | PvP lobby | hidden |
| `CharGen_Notice_FinishingLogout`, `CharGen_Temp_TooManyPlayers` | notices | shown when applicable (Tessera's own texts) |

**Character row** (`CharGen_CharacterSelect_Character_Template`, based on
`Chargen_Character_Slot`, 338 × 71; `CharGen_CharacterSelect_AdvancedCharacter_Template` for
advanced-start characters):

| Child | Shows |
|---|---|
| `CharGen_CharacterSelect_Character_Name` | the character's name (truncated with a tooltip) |
| `CharGen_CharacterSelect_Character_Description` | level, race and class (Probable: the three-variable format 0x0BB533CE of table 0x250001BD, variables 0x50AA9C level, 0x56575 race name, 0x480683 class name; formats with level only and level and race exist for unknown race or class) |
| `CharGen_CharacterSelect_Character_LastLocation` | the last location's name (markup allowed) |
| `CharGen_CharacterSelect_Character_Class` | the class icon (48 × 48), `AdvTable_ClassInfo` / `AdvTable_ClassIcon` |

The selected character stands in the viewport: its race's scene and
`RaceTable_Character_Select_Camera`, in its appearance (`appearance.md` §8).

> **OPEN:** list order, sort-menu choices and which character is selected first. **Tessera
> rule:** the session's order; the first entry, or the one entered last in this run, is
> selected; a click selects, a double click enters.

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

Changing race keeps the class if the new race allows it, else selects the first class of the
list (**Tessera rule**). A choice is valid by `player-character.md` §5.4.

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

The `GOP_AvatarUpdateVisible` check box appears only for species 0x49 and 0x51
(`appearance.md` §3.3, BIN2 there).

> **OPEN:** the randomise rule and each template's inner controls (arrows, swatches, slider
> ranges). **Tessera rule:** randomise picks, per control, a uniformly random choice (styles:
> any index; colours: any palette pixel; floats: uniform in range; sliders: the preset of a
> random preset entry if the avatar has presets, else unchanged), from a generator seeded by
> the time; each template is a labelled row with previous/next arrows (styles, presets), a
> swatch grid (colours) or a slider (floats).

<a id="s5-3"></a>
### 5.3 Name

`CharGen_Name_Entry_Panel` (top): the name field (`CharGen_Name_Entry_Field`, a
`ChargenNameTextEntry`), the random-name button (`CharGen_Name_RandomNameGenerator`) and the
tutorial choice (`CharGen_Name_ChangeTutorial` with its label and current choice).

| Function | Rule |
|---|---|
| Random name | `player-character.md` §8.5 for the chosen race, sex and nationality |
| Name check before sending | `player-character.md` §8.6 and the name-rule control §8.1; the session's own check decides (§7.1) |
| Tutorial choice | offered when the race's `RaceTable_AllowSkipTutorial` is true (reading of the name); **Tessera rule:** hidden in P1 (the sandbox has no tutorial) |

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
   if any, else the first race and its first gender; the first class. Continue → 3; back → 1.
3. **Appearance and name** (§5). Default appearance: the avatar's own values (`appearance.md`
   §3.4: "the avatar entity's own values are the defaults"); default nationality: the first of
   the list; empty name. Create → §7.1; back → 2 (the appearance choices are kept while the
   race and gender stay the same, else reset).

The viewport follows the selection at once (figure rebuilt by `appearance.md` §8 on every
change of a control).

---

<a id="s7"></a>
## 7. Commands and answers

<a id="s7-1"></a>
### 7.1 Creating

The client sends `CreateCharacter { name, appearance, class }` (`scenario-feed.md` §9.4):
`appearance` = `Appearance::new(Body::Character { species, sex })` with the chosen race table's
species and the gender entry's sex value, `class` = the class key. On M7's server the client
sends `CharacterCreate { name, appearance }` (`tessera-protocol.md` §6.2.7); the class is not
carried (rule 5, `server-content.md`).

Answers:

| Backend | Success | Failure |
|---|---|---|
| Scenario feed | `CharacterInfo` with that name (an `accept-character` step of an `on create-character` trigger) | `CharacterRefused { name, reason }`; the reason text is shown in the error list and the screen stays |
| Plain sandbox (no scenario) | none (§8 OPEN) | none |
| M7 server | `CharacterList` containing the name | `CharacterCreateFailed { code, text }`; Tessera shows its own text per code (`tessera-protocol.md` §14.4) |

On success the screen returns to the selection screen with the new character selected.
**Tessera rule:** with no answer within 10 s, an error line is shown and the screen stays.

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

<a id="s7-2"></a>
### 7.2 Deleting

A confirmation (Tessera's own dialog) then `DeleteCharacter { name }`.

> **OPEN (for `scenario-feed.md`, non-blocking):** the feed has no step that removes a name
> from the character list, so a scenario cannot answer a deletion (`scenario-feed.md` §7.5;
> its §16 OPEN (3)). Proposal: a `remove-character STRING` step allowed in `on
> delete-character` bodies, emitting a new `CharacterRemoved { name }`. **Tessera rule until
> then:** the screen removes the row only on such an event; with none it keeps the row.

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
| Q7 | Creation choices in `Appearance` | §7.1 | design decision (protocol minor) |
| Q8 | Deletion in scenarios; plain-sandbox creation | §7.2, §8 | `scenario-feed.md` owner |

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (Review), for decision 0024 P1: screens and stages, selection row, race/gender/class lists and their records, nationality, appearance controls, name, create, flow, commands and answers per backend, scenario-feed driving, real-data test, open design gaps (creation choices in `Appearance`, deletion in the feed, plain-sandbox creation). |
