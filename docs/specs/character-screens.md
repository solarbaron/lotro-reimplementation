<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Character selection and creation screens

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review by spec-review:spec-world-render:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer: the two implementer notes of §2.1.4 dated 2026-10-23 answered by pointers to `script-table.md` §5.2 and §6.10 as revised the same day; no fact of this spec changed; awaiting independent review). Before: **Accepted** (2026-10-10, independent review of the revision CS28–CS34, commit 303de21, passed; see the review record). Before: Revised (2026-10-10, spec writer: the implementer OPENs of commit dc06296 answered from the install and the client (OBS3, BIN2; rows CS28–CS34): §2.1.4 the `Chargen` start-up script the screens add to every stage figure, what it plays per stage and class, one-shot stage clips, height and framing; §2.1.1 `Render_Color` and `Render_Opacity` of scene elements; §2.3 wheel direction, hover and click on the figure, field-of-view axis; §5.2.2 the group template `CharGen_AppearanceUIGroupChoice_BaseField`; §10 CSI9–CSI10; §11 Q15 narrowed, Q16–Q17; no section number or anchor changed; awaiting review). Before: Accepted (2026-10-10, independent review of the revision CS19–CS27, commit be3b129, passed; see the review record). Before: Revised (2026-10-10, spec writer: the P1 creation-screen defects answered from the install and the client (OBS2, WLB, BIN; rows CS19–CS27): §2.1 scene, figure placement, camera and stage figures (new §2.1.1–§2.1.4), §2.3 turntable and zoom, §4.1 gender group and stage figure, §5.2 control labels, current-choice text, group headers, starting values (with `appearance.md` §3.4), §6 stage keys, §10 CSI6–CSI8, §11 Q12–Q15; implementer OPENs of §2.1, §5.2 and §6 answered or narrowed; no section number or anchor changed; awaiting review). Before: Accepted (2026-10-10, third re-review of the revision of 2026-10-10, commit 72d3f84, passed; see the review record). Merged into branch `p1` (2026-10-10): P1's pointer rows to [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) (no fact changed) and the P1 implementer OPENs are not covered by that review and await the P1 review (see the change log). Before: Revised (2026-10-10, spec writer: the second re-review's non-blocking notes answered, (k) names compared in NFC form in §7.1 and §7.1.1 step 3, (l) new §7.1.1 step 6 and §7.3 refusal of `EnterWorld` while a creation is outstanding, (m) §9 step 5 names a fresh account and `max_characters`; (e) settled, `scenario-feed.md` is on main; provenance rows CS16–CS18; awaiting review, see the change log). Before: Accepted (2026-10-10, second re-review passed; first version 2026-10-09, review and re-review of 2026-10-09 changes requested, revisions the same day; see the review record) |
| **Last updated** | 2026-10-23 (spec writer: §2.1.4 answers). Before: 2026-10-10 (spec writer, revision CS28–CS34) |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Independent spec-review sessions that wrote no part of this spec and no code (Clean-Room-Role: review): review and re-review 2026-10-09, second and third re-reviews 2026-10-10; review of the revision CS19–CS27 (commit be3b129) 2026-10-10; review of the revision CS28–CS34 (commit 303de21) 2026-10-10 |
| **Review record** | **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-world-render:1 (independent session), a session that wrote no part of it): **passed** CLEAN_ROOM.md §5.3, every item; contamination screen clean. BIN facts were checked for consistency with the cited sections and install artefacts, not re-traced. Spot checks: the two pointers agree with `script-table.md` §5.2 (T47) and §6.10 step 1 (T48). Non-blocking: the statement that key 6 is the model origin on every playable body should carry "(Inferred)" as `script-table.md` §5.2 does. Before: 2026-10-09, independent spec review (Clean-Room-Role: review; a session that wrote no part of this spec and no code), whole spec (new). Checked against `docs/CLEAN_ROOM.md` §5.3, decision 0024 P1 and every cited section of `player-character.md`, `appearance.md`, `combat-data.md` §9.1, `ui-layout.md` §3 and §5.8, `tessera-protocol.md` §5.3, §6.2.7–§6.2.9, §14.4 and `scenario-feed.md` (Accepted text on branch `p0`: §5.1, §6.1, §7.5, §7.7, §9.2, §9.4, §9.5, §16). Spot-checked read-only in the install with a throwaway probe: the seven layout records and their `UILAYOUT` labels, the stage mapper 0x23000132 (keys 1, 3, 4, 6, 7 as in §2.2), the full element trees of `chargen`, `chargen_characterselect`, `chargen_raceclass` and `chargen_appearance` (every label named in §3–§5 exists where stated; sizes 1440 × 900 z 250, 338 × 71, 261 × 45, 253 × 40/52/74/80 match; the row template is based on `Chargen_Character_Slot`), the `chargen_viewport` properties of §2.1 (all six values match), the class-info property names, and the presence of token 0x0BB533CE and the three variable IDs in table 0x250001BD (no text read). Checklist: functional facts only; no code or third-party structure (the `Appearance::new(Body::Character …)` form is Tessera's own API, as in `scenario-feed.md`); no game text; no gates touched. Result: **changes requested**. Blocking: (1) §2.1 describes `chargen` incompletely: its `ControlsParent` (edge modes 6, 5, 6, 5) holds `CharGen_Back_Button`, `CharGen_Continue_Button`, `CharGen_Exit_Button`, the turntable group `Chargen_Controls` (rotate left/right, zoom in/out buttons) and `CharGen_AvatarUpdate_Toggle` (label and check-box button); §6 uses Continue and back and §5.2 the avatar-update check box without naming any of these elements, so the flow cannot be wired from the layout without guessing; name them, say which stages show which, and give a P1 rule for the turntable and Exit. (2) §5.3 calls `CharGen_Name_Entry_Field` "a `ChargenNameTextEntry`", which reads as a widget type; in the data the field is a `Text` element (editable, selectable, one line) whose base reference is the element labelled `ChargenNameTextEntry` in layout 0x22000002 (no element type of that name exists); state it so. (3) §7.2 gives the deletion answer for the scenario feed only: `tessera-protocol.md` has no deletion message and the test-server backend refuses `DeleteCharacter` with `NetError::InvalidCommand` (`scenario-feed.md` §9.5), yet §3 shows the delete button for every backend; give the screen's rule per backend (as §7.1 does for creation), at least for M7's server (button hidden or disabled, or the refusal shown). Non-blocking: (a) §7.1 "rule 5, `server-content.md`": cite `server-content.md` §2 directly. (b) §2.2 "each has a scaling parent … that keeps the side panels at the screen edges": the side panels are siblings of the scaling parent and anchor by their own edge modes (5, 5, 6, 5) left and (6, 5, 5, 5) right; the scaling parent (6, 5, 6, 5) holds the centre parts. (c) The "Probable" reading of the description format in §3 has no provenance row; add one (OBS token IDs, meaning a reading). (d) Elements present but not given a P1 rule: `CharGen_Name_Admin_Button`, `CharGen_RandomName_Debug_Field`, `CharGen_ContinueLocked_BuyNowButton`, the `CharGen_Avatar_Loading_*` blockers, `CharGen_Appearance_Nationality_Text`, the choose-available-characters dialog of `chargen_characterselect`; say hidden or how used. (e) `scenario-feed.md` is not on this branch (Accepted on `p0`); links resolve only after `p0` is merged, which implementation of §7–§9 needs anyway. **Re-review 2026-10-09** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec and no code), whole spec (still new on this branch), with emphasis on the revision. Checked against `docs/CLEAN_ROOM.md` §5.3, `tessera-protocol.md` §6.2.2 and §6.2.7–§6.2.9, `scenario-feed.md` §9.4–§9.5 (on `p0`), `appearance.md` §3.3 and `server-content.md`. Spot-checked read-only in the install (archives byte-identical to an earlier independent decode of layouts 0x22000001, 0x22000002, 0x22000004, 0x2200000C, 0x2200000D, 0x22000010, 0x2200083E, re-decoded with a throwaway value-reading probe): every box, base element, edge mode, z-level and button property in the §2.3 table (including `UICore_Button_broadcast_when_ghosted` on Continue only, the hot-button flags and first interval 0.05 on the two turn buttons, Exit sharing Back's box); the six viewport values of §2.1; §2.2 edge modes of the side panels and scaling parents; §4.3 boxes, z −10, `UICore_Element_blocks_clicks` and the 0xBF000000 fill of both loading blockers; §5.3 name field (base `ChargenNameTextEntry` in `text`, a `Text` 391 × 51, field 268 × 28 at 8, 0 with editable, selectable and one-line true); `CharGen_Name_Admin_Button` hidden; the create and race-change buttons in the Continue box; the row template 338 × 71 on `Chargen_Character_Slot`; the dialog and its entry templates top-level in `chargen_characterselect`. First-review items (1)–(3), (a)–(d): each answered as the change log says. Checklist: functional facts only; no code (the struct-like command forms are Tessera's own `tessera-net` API); no game text; provenance rows CS8–CS13 present; invariants testable; no gate touched. Result: **changes requested**. Blocking: (R2-1) §7.1 says that on M7's server the client sends `CharacterCreate { name, appearance }`, but the command the screen issues is `tessera-net`'s `CreateCharacter`, and `scenario-feed.md` §9.5 (the same sentence CS13 cites for deletion) makes the test-server backend refuse every new command of §9.4, `CreateCharacter` included, with `NetError::InvalidCommand`; the `remote` backend also has no event for `CharacterList` or `CharacterCreateFailed` (it surfaces only `Connected` with names). As written, creation on M7's server — needed by P1 exit criterion 1 — cannot be built without guessing. Either (i) state the rule for the `remote` backend (`CreateCharacter` with an appearance becomes `CharacterCreate`, class dropped; which session event a `CharacterList` and a `CharacterCreateFailed` become, e.g. `CharacterInfo` for the new name and `CharacterRefused` with Tessera's text per §14.4 code; the New button ghosted when `HelloAck.server_caps` lacks `CHARACTER_CREATION`, §6.2.2) and record it as a required revision of `scenario-feed.md` §9.5 with an OPEN and a provenance row, or (ii) treat creation on M7's server as OPEN with a per-backend rule for the New button (as §7.2 does for Delete) and say that P1 criterion 1's M7 part waits on it; in both cases make CS13, §7.1's answer table and §11 consistent. Non-blocking: (f) `CharGen_ContinueLocked_BuyNowButton` is also hidden by its own data (`UICore_Element_hide` true); add it to CS12 and §4.3. (g) §2.2 selection row: `CharGen_Notice_FinishingLogout` is a direct child of `CharGen_CharacterSelect_Base`, not of the scaling parent (only `CharGen_Temp_TooManyPlayers` is inside it). (h) Elements without a P1 rule: the top-level `ECommBrowser` (a `Browser`) of `chargen_characterselect` and `GamePlay_ExaminationTooltip_Activatable` under `CharGen_Base_Field` of `chargen`; say never created or unused. (i) The mark **Probable** (CS11) is not defined in the provenance key; define it (as `fonts.md` does) or reword. (j) §4.3 loading blockers are based on `Chargen_Silver_Border` (0x22000001, a nine-part border) as well as the fill; mention the border. (e) stands. **Second re-review 2026-10-10** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec and no code), the revision of 2026-10-09 (§1 key, CS12, CS14, CS15, §2.1, §2.2, §3, §4.3, §7.1, §7.1.1, §9 step 5, §11 Q11), checked against `docs/CLEAN_ROOM.md` §5.3, `tessera-protocol.md` §5.3 (`CharacterSummary` carries a body), §6.1, §6.2.2, §6.2.5–§6.2.10, §12.1, §13.1, §14.4 and Appendix A, `scenario-feed.md` §9.2, §9.4 and §9.5 (Accepted text on branch `p0`) and `test-server.md` §6.4 and its `character_creation` and `max_characters` settings. Spot-checked read-only in the install with an own throwaway probe (layouts 0x2200000C, 0x2200000D, 0x2200083E and 0x22000001 decoded in full): `CharGen_Notice_FinishingLogout` a direct child of `CharGen_CharacterSelect_Base` and `CharGen_Temp_TooManyPlayers` the only notice in the scaling parent; `ECommBrowser` a top-level `Browser` 100 × 100; `GamePlay_ExaminationTooltip_Activatable` a `Field` 192 × 128 at 571, 12 under `CharGen_Base_Field`; `CharGen_ContinueLocked_BuyNowButton` 241 × 41 at 600, 666 on `CharGen_Class_BuyNow` of 0x22000837 with `UICore_Element_hide` true; both loading blockers based on `Chargen_Silver_Border` of 0x22000001, which has eight edge pieces and a background; boxes, z-levels and `blocks_clicks` as in §4.3. All match. R2-1: answered by option (i). §7.1 now has one screen-level command and two events on every backend, a per-backend New-button rule and the Create, refusal and time-out rules; §7.1.1 gives a complete, testable rule for the `remote` backend (capability bit, acceptance conditions that match the protocol's preconditions, class dropped per `server-content.md` §2, `CharacterList` → `CharacterInfo` with the summary's body, `CharacterCreateFailed` → `CharacterRefused` with Tessera's own text for each §14.4 code, unknown codes as 6, states); it is stated as a narrowing of `scenario-feed.md` §9.5 with an OPEN and §11 Q11 for the matching revision there and in Appendix A, and CS13, CS14, the answer table and §11 agree. Non-blocking (f)–(j): each answered as the change log says. Checklist: functional facts only; no code (struct-like forms are Tessera's own `tessera-net` and protocol names); no copied prose; no game text (no string read); provenance rows CS14, CS15 present with marks; invariants testable (§9 step 5); unknowns are OPEN; no gate touched (M7's own protocol only). Result: **passed**, status Accepted. Non-blocking notes for a later revision: (k) `test-server.md` §6.4 stores the NFC form of the name, so §7.1.1 step 3 ("an entry with the outstanding name") and §7.1's success rule ("name equals the requested name") miss a successful creation typed in a non-NFC form and report `SERVER_ERROR`; compare the NFC forms (or send the NFC form) and say so. (l) After the screen's 10 s time-out the backend's creation is still outstanding; say that the `remote` backend also refuses `EnterWorld` while it is (`tessera-protocol.md` §6.2.10, §13.1 make a second request fatal), or that the selection screen ghosts Enter meanwhile. (m) §9 step 5 creates one character per race on one account: name a fresh account and a `max_characters` (default 8) at least the number of playable races, so no creation of that step fails with `TOO_MANY` (the duplicate check comes first, `test-server.md` §6.4, so its code 1 is not affected). (e) stands. **Third re-review 2026-10-10** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec and no code), the revision of 2026-10-10 (commit 72d3f84: header, §1 key and rows CS13, CS14, CS16–CS18, companion row, §7.1, §7.1.1 steps 3 and 6, §7.3, §9 step 5, change-log row), checked against `docs/CLEAN_ROOM.md` §5.3, `test-server.md` §3.5 (stored `name` is the NFC form), §4.3 (`max_characters` default 8, range 1–64) and §6.4 (NFC name rule; creation checks in the order enabled, name rule, name taken, `max_characters`, appearance), `tessera-protocol.md` §6.2.10 (`EnterWorld` pre: CharacterSelect, nothing outstanding), §13.1 "Outstanding requests" (one `CharacterCreate`/`EnterWorld`, a second is fatal) and §14.4 (code 1 `NAME_TAKEN`, 3 `TOO_MANY`), and `scenario-feed.md` on main: no commit touches it after the merge a1fb29f, and the only change after its accepted revision 3 (eafefc9) is 674ffb1, which adds three OPEN callouts in §11.1, §12.4 and §13 and leaves the cited sections unchanged. All agree. No new install fact was added (the race count of §9 step 5 refers to step 1's observation, read at run time), so no install probe was needed. (k), (l), (m): each answered as the change log says; the success and failure rules of §7.1 and step 3 of §7.1.1 use the same NFC comparison; step 6 is consistent with step 2 and §7.3; (e) settled. Checklist: functional facts only; no code (struct-like forms are Tessera's own `tessera-net` and protocol names); no copied prose; no game text; provenance rows CS16–CS18 present with marks, new key entry **Standard** for UAX #15; invariants testable (§9 step 5 now fixes its account and limit); Q11 and the §7.1.1 OPEN kept; no gate touched. Result: **passed**, status Accepted. No new notes. **Review of the revision CS19–CS27 2026-10-10** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec, no code and none of the earlier reviews), commit be3b129 on branch specs-22: header, §1 keys OBS2, WLB, BIN and rows CS19–CS27, §2.1–§2.1.4, §2.3, §4.1, §5.2–§5.2.2, §6, §10 CSI6–CSI8, §11 Q12–Q15, change-log row; with the companion `appearance.md` §3.4 hunk and owner items O85, O86. Checked against `docs/CLEAN_ROOM.md` §5.3 and the cited anchors of `ui-widgets.md` (§3.4, §7.1 default hot-click interval 0.05 s, §7.3, §12), `misc-records.md` §3, §4.1 U3 and §4.2 U4 rules 1–5, `setup.md` §3.1 and §6.3, `lighting.md` §5.3, §8.3, §8.5, `coordinates.md` §11.3, `mesh.md` §11.1, `walker.md` W17 and `script-table.md` (`CharGen_Stage` switch). Spot-checked read-only in the install with an own throwaway probe on the repository decoders (dirty scratch area): all 8 race tables with their 15 gender entries and 14 distinct player avatars; every race scene decoded in full: 21–732 elements, Physics 13–686, Light 3–41, Camera 4–7, Decal elements in two scenes (2 and 5), 1–4 `KeyLight` lights per scene with ranges 10–800, 0–3 `ShadowCaster` lights; in each scene exactly one Physics element whose setup chain ends in a setup with holding location key 0x10000042 (`Holding_Loc_Entity_00` of mapper 0x23000068; frame identity at the origin), always the entity 0x4700061C with `Render_Draw` false, no placement in 7 scenes and a −90° turn about +Z at the origin in 0x21000012; `chargen_Camera` = key 0x10000008 of 0x230000B9, an element of that name in every scene; `RaceTable_Chargen_Camera` equal to `RaceTable_Character_Select_Camera` for every race; every base and zoom camera the race tables name is an element of the race's scene, on the +Y side rotated about 180° about +Z (on +X rotated +90° in 0x21000012); mappers 0x23000132 (stage keys 1, 3, 4, 6, 7) and 0x23000133 (`CharGen_Action` 1–3, 2 `Idle`, 3 `Hover`); layout 0x2200083E (`CharGen_Gender_GroupBox` default button `CharGen_Male_Button`; both buttons boolean, on `Chargen_Race_Select_Gender` of 0x22000004, `UI_RaceSelection_Gender` 4096 and 8192); 221 of 221 controls with `AppearanceUI_ControlStringName`, all in 0x250001BD; the label of every colour template variant in 0x22000088 carries token 0x08E35CB2, which is also the name of one colour control (Beorning female); token 0x0E5568D5 of 0x250001BB has exactly the argument IDs 0x047D3D75, 0x005662B5 (no text read); 0x2200009F `AppearanceUIElement_BaseField` with label 161 × 23 and current choice 48 × 23, and the three style, face-layer and presets templates holding it; group mapper 0x23000491 keys 1–9 with the nine names and display texts; 37 style controls offered at creation, each naming its key in its record and its toggle record, every first block `variant` 0.00; the 8 presets controls each the first control of its avatar; 2 controls with `AppearanceUI_ControlDefaultLockState`; the Beorning female and High-elf male examples of §5.2.1 (records, toggle records and head mesh 0x0600D8C2 in both Beorning head records). All match, except the lower bound noted in (n2). BIN and WLB facts (fields of view 65 and 40, 20° steps, drag factor, half-lives, zoom-move timing, ghosting, stage keys set by the client, gender-entry slot 14) were not re-derived (no M4 session in this review); they are stated with their BIN key under decision 0011, restated in words, with no address, listing or symbol. Checklist: functional facts only; no code or pseudo-code (the zoom timing and easing are formulas in words; decision 0012 constants); no copied prose; element, property, mapper and class names are interoperability identifiers; no game text (tokens and IDs only; the group names are mapper names); provenance rows CS19–CS27 with marks; invariants CSI6–CSI8 testable and hold on this install; unknowns OPEN (§2.1.1, §2.1.3, §2.1.4, §5.2) with Tessera rules; no gate touched (offline UI code only). Result: **passed**, status Accepted. Non-blocking notes for a later revision: (n1) §2.3 says which wheel direction zooms in "was not traced" and Q15 lists it, but there is no `> **OPEN:**` callout and no Tessera rule, so an implementer must choose; add the callout with a rule. (n2) §2.1.3 gives the race cameras as 0.99–1.74 m above the scene origin; this probe finds 1.02 (hobbit and River-hobbit base cameras, 1.015) to 1.74; correct the lower bound or say what was measured. (n3) CS19 names only the element and light ranges; add the other §2.1.1 counts (Physics 13–686, cameras 4–7, decals in 2 scenes, `KeyLight` 1–4 with ranges 10–800, `ShadowCaster` 0–3) to the row. (n4) §5.2.2 rule 4 marks the lock's effect OPEN inline; make it a callout like the others. **Review of the revision CS28–CS34 2026-10-10** (independent spec review, Clean-Room-Role: review; a session that wrote no part of this spec, no code and none of the earlier reviews), commit 303de21 on branch specs-23: header, §1 keys OBS3 and BIN2 and rows CS28–CS34, §2.1.1 (colour and opacity of scene objects, alpha OPEN), §2.1.4 (the `Chargen` start-up script, what it plays, one-shot clips, height and framing, removal OPEN, the implementer OPEN answered), §2.3 (wheel direction in the Zoom item, the answer to the implementer OPEN), §5.2.2 (group template, the implementer OPEN answered), §10 CSI9–CSI10, §11 Q15–Q17, change-log row. Checked against `docs/CLEAN_ROOM.md` §5.3 and the cited anchors: `instance-scripts.md` §3 and §4.2 (start-up order I7), `script-runtime.md` §12.2 (every defining table of the closure contributes roots), `script-table.md` §4.3 (port type 28), §6.7, §6.8, §6.10 steps 3, 4 and 8 (one-shot end and stop), §8.3 (both idle clips in its table), §8.4 step 1, §11.3, `appearance.md` §7.4 ("Applying an Appearance node"), `material.md` §8.3 (per-object tint), `environment.md` §11.4 step 5 and §11.7 (sky path to `Render_Color`/`Render_Opacity`), `ui-widgets.md` §2.4 (`ScrollUp` 0x82, `ScrollDown` 0x10), `keymap.md` K42, `ui-layout.md` §5.6 and `walker.md` W17 (Tessera Decision: vertical). Spot-checked read-only in the install with an own throwaway probe on the repository decoders (dirty scratch area; every 0x07 and 0x0C record decoded, scriptlets followed): key 0x100004AB of 0x23000097 is `Chargen`; 19 tables define it; the table closure of each of the 14 player-avatar entities holds exactly two of them, 0x0700042D (root: persistent Switch on `Entity_Mode`) and one body table, 14 distinct; each body definition has a persistent Switch on `CharGen_Stage` with cases 4, 1, 6, a type-28 port and a Default; under case 1 a persistent Switch on `Agent_Class` with 9 to 11 class cases and a Default (148 cases over the 14 avatars, 159 with the shared avatar of the 15th entry counted twice); every class case holds an Appearance node, some MeshFX nodes, and reaches through its Scriptlet nodes a looping clip on layer 80 (78 in 7 of 11 cases of the Dwarf and the Stout-axe bodies); all 155 looping clips under `Agent_Class` cases have no track weights; a few cases add one-shot layer-90 gestures under a looping Sequence of Random nodes; the stage-6 branch of the Stout-axe body loops 0x05007A13 and of the River-hobbit female 0x05008EE7; no typed node (Switch, ScalarBlend, While, driven or scriptlet-driven field, SetProperty, When test) refers to `CharGen_Action`, and every raw occurrence of its ID in a script record is the numerically equal script key of a definition or Run node; no Run node names `Chargen`; the text `chargen_class_idle` occurs in table 0x07001820 and in two 0x0C records (the template and one other scriptlet) only; the 8 race scenes hold 2,830 Physics elements, 207 with an own `Render_Color` (10 with a fourth byte below 255: 216, 112, 178) and 6 with an own `Render_Opacity`; `chargen_appearance` (0x22000010 by `UILAYOUT` map 0x28000003, which has no entry for 0x2200090A) holds `CharGen_AppearanceUIGroupChoice_BaseField` 265 × 80 at 830, 231, edge modes 1, 0, 1, 0, based on `AppearanceUIGroupUIElement_BaseField` of 0x2200090A (typed, 261 × 40) with every part, position, size and edge mode as §5.2.2 states. The runtime census of CS30 (189 of 189, 0 of 159) was not re-run; the static structure above agrees with it and with the implementer's observation. The object-chain counts of CS34 and the lock button's boolean flag were not re-checked. BIN2 facts (the appended start-up key and `Render_AlwaysSelectable`, wheel commands and step, hover and click handling) were not re-derived (no M4 session in this review); they carry their BIN key under decision 0011, restated in words, with no address, listing or symbol. Checklist: functional facts only; no code or pseudo-code; no copied prose (the quoted "too tall" and "waist-deep" are Tessera's own implementer note); property, mapper, element and script-key names are interoperability identifiers; no game text (IDs only); provenance rows CS28–CS34 with marks and the new keys OBS3, BIN2; invariants CSI9 (on the script runtime) and CSI10 testable; unknowns OPEN with Tessera rules (§2.1.1 alpha byte, §2.1.4 removal of node pieces, §2.3 wheel during a zoom move, §5.2.2 collapse and group lock) and listed as Q15–Q17; no gate touched (offline UI code and data only). Result: **passed**, status Accepted. Non-blocking notes for a later revision: (o1) §2.1.4 lists the start-up order as `Startup`, class-table names, `Chargen`; `instance-scripts.md` §4.2 puts `Entity_PlayerPhysicsScript` after `Startup` when it is set (none of the 14 avatars sets it, so the order holds on this install); say so. (o2) The class cases start their stance and gesture clips through Scriptlet nodes (0x0C records), not through Animation nodes of the body table; a sentence would spare an implementer who inspects the table directly. (o3) The stage-6 branch of every body and the stage-4 branch of two bodies also hold an `Agent_Class` Switch (one to three ports, mostly MeshFX props); item 2 of "What `Chargen` plays" does not mention them, and the removal OPEN (Q16) applies to them as well. |
| **Implements into** | `apps/client` (the screens before world entry, decision 0024 P1 exit criterion 1), `crates/gamedata` (race, gender, class, nationality and appearance-control lists from the install; name generation and checks), `crates/ui` (fallback widgets of the creation screens' game widget types) |
| **Companion specs** | [`player-character.md`](player-character.md) (race tables §5, nationalities §6, class data §7, names §8, character slots §11.1), [`appearance.md`](appearance.md) (bodies §2, creation controls §3.4, face and colours §3.5, sliders §6), [`combat-data.md`](combat-data.md) §9.1 (class tables), [`ui-layout.md`](ui-layout.md), [`ui-widgets.md`](ui-widgets.md) (GroupBox §3.4, hot buttons §7.1, state requests §7.3, viewport §12), [`misc-records.md`](misc-records.md) (UI scenes §4, compositors §3, movie library), [`setup.md`](setup.md) §6.3 (holding locations), [`lighting.md`](lighting.md) §5.3, §8.3, §8.5 (scene lights), [`scenario-feed.md`](scenario-feed.md) (Accepted 2026-10-09, revision 3; on main since the merge of branch `p0`, commit a1fb29f: character list, `CharacterInfo`, `CreateCharacter`, character triggers; §3.3, §5.1, §6.1, §7.5, §7.7, §9), [`tessera-protocol.md`](tessera-protocol.md) §6.2.7–§6.2.9 and [`test-server.md`](test-server.md) (M7's character creation), [`character-panels.md`](character-panels.md) |

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
| **Spec** | Accepted text of another Tessera spec, cited by section (`tessera-protocol.md`, `scenario-feed.md`, `test-server.md`). |
| **Standard** | A published standard, cited by name (Unicode Standard Annex #15 for NFC). |
| **OBS2** | The same install, read-only, 2026-10-10 (archives unchanged since 2026-10-04), with throwaway probes in the dirty scratch area on the repository's decoders: all 8 race UI scenes (`RaceTable_UIScene` of every race table: 0x21000006–0x21000009, 0x21000012, 0x2100001A, 0x2100001D, 0x2100001F) element by element, with the setup chain and holding locations of every Physics element's object; the UISceneEntity mapper 0x230000B9, the HoldingLocation mapper 0x23000068, the `AppearanceUI_Group` mapper 0x23000491 and the `CharGen_Action` mapper 0x23000133; the `AppearanceUI_Controls` of all 14 avatar entities (221 controls); the base layouts of the control templates (0x22000085–0x22000088, 0x2200009F, 0x220008EB–0x220008ED, 0x22000908, 0x2200090A) and of the gender group (0x2200083E); the presence of two text tokens and their argument IDs in the string tables (no text reproduced). |
| **WLB** | Class-library script code of the install (record 0x56000000, method M1), disassembled 2026-10-10 in the dirty room with the rules of [`wlib-bytecode.md`](wlib-bytecode.md): the class-394 functions the client calls for the stage figure and for the scene and cameras, and the slot layout (class 859) and values of every race table's gender entries. Restated in words; nothing of the code is reproduced. |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md): the client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035, read on 2026-10-10 in the dirty room with Ghidra (headless, private read-only copy of the shared project, deleted afterwards). **Questions:** (a) which values a fresh appearance starts with on the appearance screen; (b) where the figure stands in the UI scene, which camera frames it, with which field of view, and what the turntable and zoom controls do; (c) where the labels of the appearance controls, their current-choice texts and the group headers come from; (d) which stage keys and other properties the figures of the stages carry; (e) how the gender buttons behave. **Why M1–M3 were not enough:** the data holds the scenes, their camera and marker elements, the controls and their name texts, but not which element places the figure, how the camera is chosen and moved, or which values a fresh appearance takes; the original client cannot be observed running in this session. Only offline UI code was read (the creation screens, their appearance-control widgets and the scene viewport); nothing about network, login, keys, launcher, patcher or anti-tamper. Numeric constants are functional facts ([decision 0012](../decisions/0012-numeric-constants.md)). No address, listing or symbol appears in this spec. |
| **OBS3** | The same install, read-only, 2026-10-10 (archives unchanged since 2026-10-04), with throwaway probes in the dirty scratch area on the repository's decoders and on its script runtime (`tessera-scriptgraph`, built from `script-runtime.md`): the table closures of the 15 gender entries' player avatars (which tables define the script key `Chargen`, their `CharGen_Stage` and `Agent_Class` cases); 204 runs of a figure's `Startup`, start-up names and, in a second pass, `Chargen` (each body with `CharGen_Stage` 1 and each class key of its `Agent_Class` cases or none, with 6, and with 4; 20 s simulated), recording every clip started with its layer, loop flags, `UseBoneOpacity`, track count and track weights; every 0x07 and 0x0C record searched for Switch, When and driven fields on `CharGen_Stage` and `CharGen_Action` and for Run nodes naming `Chargen`; the Physics elements of the 8 race scenes and their object chains for `Render_Color`, `Render_Opacity` and `Render_Draw`; every 0x22 layout for elements based on layout 0x2200090A; the `UILAYOUT` DID map. Clip and record IDs only; no text read. |
| **BIN2** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md): the same client build (4906.0071.1794.4035), read on 2026-10-10 in the dirty room with Ghidra (headless, private read-only copy of the shared project, deleted afterwards). **Questions:** (f) what else than the stage keys the screens give the stage figures, so that they take a stance (implementer OPEN of §2.1.4); (g) which wheel direction zooms in; (h) how the pointer's hover over a figure and a click on it are found. **Why M1–M3 were not enough:** the data hold the `Chargen` script definitions, but no data node starts that script (OBS3), so only the client can say who starts it; the viewport's input mapping and the hover test are code; the original client cannot be observed running in this session. Only offline UI code was read: the two routines that create the selection and the race-and-class figure, the routine that appends a script key to a property set's instance script table, the viewport's input handler and its per-frame hover update. Nothing about network, login, keys, launcher, patcher or anti-tamper. No address, listing or symbol appears in this spec. |

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
| CS13 | No backend except the scenario feed can answer a deletion (creation: CS14): `tessera-protocol.md` §6.2 has no deletion message; the test-server backend refuses `DeleteCharacter` with `NetError::InvalidCommand` and the plain sandbox ignores it (`scenario-feed.md` §9.5) (§7.2) | `tessera-protocol.md` §6.2; `scenario-feed.md` §9.5 (Accepted; on main since a1fb29f, text unchanged from the reviewed one, re-checked 2026-10-10) | Confirmed (spec text, 2026-10-09; re-checked on main 2026-10-10) |
| CS14 | On M7's server the screens' `CreateCharacter` is carried by `CharacterCreate` (§6.2.7) and answered by `CharacterList` or `CharacterCreateFailed` (§6.2.8–§6.2.9, codes §14.4), accepted only when `HelloAck.server_caps` has `CHARACTER_CREATION` (§6.2.2) and only one request is outstanding; the `remote` backend has no event for either answer yet (Appendix A) and `scenario-feed.md` §9.5 makes it refuse `CreateCharacter`; the mapping of §7.1.1 is a Tessera rule that narrows that refusal | Spec (`tessera-protocol.md` §6.2.2, §6.2.7–§6.2.9, §13.1, §14.4, Appendix A; `scenario-feed.md` §9.4–§9.5, on main since a1fb29f, text unchanged from the reviewed one, re-checked 2026-10-10); Design | Confirmed (spec text, 2026-10-09; re-checked on main 2026-10-10); Decision (the mapping) |
| CS15 | `CharGen_Notice_FinishingLogout` is a direct child of `CharGen_CharacterSelect_Base` (only `CharGen_Temp_TooManyPlayers` is inside the scaling parent); `ECommBrowser` (a `Browser`, 100 × 100) is a top-level element of `chargen_characterselect`; `GamePlay_ExaminationTooltip_Activatable` (a `Field`, 192 × 128) is a child of `CharGen_Base_Field`; both loading blockers are based on `Chargen_Silver_Border` of layout 0x22000001 (eight edge pieces and a background) and carry the translucent fill (§2, §3, §4.3) | OBS (layouts 0x2200000C, 0x2200000D, 0x2200083E, 0x22000001, value-decoding probe 2026-10-09) | Confirmed 2026-10-09 |
| CS16 | Tessera's test server normalises a created name to Unicode NFC, stores and lists that form (`CharacterList` entries carry it), so the screens and the `remote` backend compare names by their NFC forms (§7.1, §7.1.1 step 3) | Spec (`test-server.md` §6.4 name rule, §3.5 `name` field); Standard (Unicode Standard Annex #15, Normalization Form C); Design (the comparison) | Confirmed (spec text, 2026-10-10); Decision (the comparison) |
| CS17 | `EnterWorld` requires that nothing be outstanding and a second outstanding `CharacterCreate`/`EnterWorld` is fatal, so the `remote` backend refuses `EnterWorld` while a creation is outstanding, also after the screen's time-out; the selection screen shows the refusal and stays (§7.1.1 step 6, §7.3) | Spec (`tessera-protocol.md` §6.2.10, §13.1 "Outstanding requests"); Design (the refusal and the screen's handling) | Confirmed (spec text, 2026-10-10); Decision (the rule) |
| CS18 | The test server checks a creation in the order enabled, name rule, name taken, `max_characters`, appearance; `max_characters` defaults to 8 with range 1–64, so §9 step 5 runs on a fresh account with `max_characters` at least the race count and its duplicate creation still yields code 1 (§9) | Spec (`test-server.md` §4.3, §6.4); Design (the test setup) | Confirmed (spec text, 2026-10-10); Decision (the setup) |
| CS19 | Every race scene (8 of 8) has exactly one Physics element whose object's setup has the holding location `Holding_Loc_Entity_00` (key 0x10000042): the entity 0x4700061C (`Render_Draw` false), at the scene origin, rotation identity in 7 scenes and −90° about +Z in 0x21000012; every race scene has an element named `chargen_Camera`, and the camera elements the race tables name (`RaceTable_Chargen_Camera`, `RaceTable_Character_Select_Camera` and the gender entries' zoom cameras) exist in the race's scene and look at the scene origin (on +Y with the +Z rotation 180°, or on +X with +90° in 0x21000012); 21–732 elements and 3–41 lights per scene (§2.1.1–§2.1.3) | OBS2 | Confirmed 2026-10-10 |
| CS20 | The viewport's scene and camera value set per stage; the figure placed at the frame of the first Physics element whose setup has `Holding_Loc_Entity_00`; the camera element `chargen_Camera` moved to the stage camera's frame, the zoom frame from the zoom camera, field of view 65 and 40, view along local +Y (§2.1.1–§2.1.3) | BIN; OBS2 (consistency of every race scene) | BIN |
| CS21 | Turntable and zoom: the turn angle moves the camera around the figure; 20° per hot click, −0.5° per pixel of drag, half-life 0.15 s; zoom buttons with a timed eased camera move, wheel steps of 0.15 with half-life 0.1 s; ghosting of the zoom and turn buttons; angle and zoom reset with the stage camera (§2.1.3, §2.3) | BIN | BIN |
| CS22 | Stage figures: selection figure with `CharGen_Stage` 4 and `CharGen_Action` 2 or 3; the race-and-class figure is the entity in slot 14 of the gender entry (in all 15 entries the `PhysObj` of `RaceTable_Gender_PlayerAvatar`) with `CharGen_Stage` 1, `CharGen_Action` 2 and `Agent_Class`; the appearance stage keeps it and sets `CharGen_Stage` 6 (§2.1.4) | BIN; WLB; OBS2 (slot values of all 15 gender entries) | BIN; Confirmed (slot values) 2026-10-10 |
| CS23 | Every appearance control (221 of 221 of the 14 avatar entities) has `AppearanceUI_ControlStringName` (all in table 0x250001BD); the client writes it into the template's label element; the colour template's label carries a fixed layout text (token 0x08E35CB2 of 0x250001BD), the same token as one colour control's own name, which the control's name replaces (§5.2.2) | OBS2; BIN | Confirmed 2026-10-10 (data); BIN (use) |
| CS24 | The current-choice text is token 0x0E5568D5 of table 0x250001BB with the two arguments 0x047D3D75 (number of the current choice) and 0x005662B5 (number of choices) (§5.2.2) | OBS2 (token and argument IDs); BIN (use and argument values) | Confirmed 2026-10-10 (IDs); BIN |
| CS25 | Group headers: label = the display text of the group key (mapper 0x23000491, keys 1–9 with display texts); ungrouped controls first, then groups 1–9 in ascending key; groups start expanded (§5.2.2) | OBS2; BIN | Confirmed (mapper) 2026-10-10; BIN |
| CS26 | Starting values of a fresh appearance: controls built in stored order, each reading its value from the figure and writing it back; style controls write an appearance-list element from the control's own record (§5.2.1, `appearance.md` §3.4) | BIN; OBS2 (37 style controls of 14 avatars, every first block `variant` 0.00; every presets control is its avatar's first control, 8 of 8) | BIN; Confirmed (census) 2026-10-10 |
| CS27 | Gender group: `CharGen_Gender_GroupBox` with `UICore_GroupBox_default_button` = `CharGen_Male_Button`, both buttons `UICore_Button_boolean_button` true and carrying `UI_RaceSelection_Gender`; the client sets each race entry's `UI_RaceSelection_AllowGenderSelect` to "the race has more than one gender entry", hides the group and selects the male button when it is false, and reads the gender from the selected button (§4.1) | OBS2 (layout 0x2200083E); BIN | Confirmed 2026-10-10 (data); BIN (behaviour) |
| CS28 | The screens append the script key `Chargen` (0x100004AB of the script-key mapper 0x23000097) to `ScriptTable_StartupScriptNames` of the `Entity_InstanceScriptTable` in the property set they create the selection figure and the race-and-class figure with; the race-and-class figure also gets `Render_AlwaysSelectable` true (§2.1.4) | BIN2 | BIN |
| CS29 | Every avatar closure (15 of 15 gender entries) defines `Chargen` in two tables: 0x0700042D (shared by all) and one creation table of the body (14 distinct, one per avatar entity); the body's definition switches persistently on `CharGen_Stage` (cases 1, 4, 6, a transition port, a Default) and under case 1 on `Agent_Class` (9 to 11 class keys per body), the class cases starting a looping stance clip and holding Appearance (58) nodes, some also MeshFX (41) nodes; no Run node of any 0x07 or 0x0C record names `Chargen`; no node of any 0x07 or 0x0C record switches on or is driven by `CharGen_Action`; the template parameter `chargen_class_idle` (`script-table.md` §8.4 step 1) is bound only in table 0x07001820, which no avatar closure contains (§2.1.4, §2.3) | OBS3 | Confirmed 2026-10-10 |
| CS30 | Stance census: with `Chargen` running, 189 of 189 runs (15 bodies × stage 1 with each class key of their cases (159), stage 6 (15), stage 4 (15)) keep at least one looping clip on a layer below 110 that covers the whole skeleton (no track weights, or `UseBoneOpacity` 0) with the body's track count; without it, 0 of the 159 stage-1 runs, 11 of 15 stage-6 runs (not: both Stout-axe entries, both River-hobbit entries) and 15 of 15 stage-4 runs (§2.1.4) | OBS3 (runtime of `script-runtime.md`) | Confirmed 2026-10-10 |
| CS31 | Wheel over the viewport: `ScrollUp` (0x82) moves the target of the zoom fraction by +0.15 (towards the zoom frame), `ScrollDown` (0x10) by −0.15, clamped to [0, 1], half-life 0.1 s; the wheel and drag handlers do not test whether a zoom button's camera move runs (§2.3) | BIN2 | BIN |
| CS32 | Hover and click: each frame the screen takes the first object the viewport's 3D view reports under the pointer; a figure of the screen that is not the clicked one gets `CharGen_Action` 3 when it becomes the hovered object and 2 when it stops being it; a `SelectLeft` command on the viewport makes the figure under the pointer the clicked figure (§2.3) | BIN2 | BIN |
| CS33 | `chargen_appearance` holds the group template `CharGen_AppearanceUIGroupChoice_BaseField` (top level, 265 × 80 at 830, 231, edge modes 1, 0, 1, 0), based on `AppearanceUIGroupUIElement_BaseField` of layout 0x2200090A (no `UILAYOUT` label; type `AppearanceUIGroup`, 261 × 40) with label, expand and collapse buttons, lock button, the list `CharGen_AppearanceUI_Controls_Listbox` and a three-part frame (§5.2.2) | OBS3 | Confirmed 2026-10-10 |
| CS34 | Of the 2,830 Physics elements of the 8 race scenes, 207 carry their own `Render_Color` (10 with an alpha below 255) and 6 their own `Render_Opacity`; the object chains of a few more carry `Render_Color` (4), `Render_Opacity` (10) or `Render_Draw` (17) (§2.1.1) | OBS3 | Confirmed 2026-10-10 |

---

<a id="s2"></a>
## 2. Screens

<a id="s2-1"></a>
### 2.1 Base and viewport (Confirmed, CS1; scene, figure and camera BIN, CS19–CS22)

Layout `chargen`: `CharGen_Base_Field` (1440 × 900 design size, z 250) holds
`chargen_viewport` (a full-screen `Viewport` with shadows on, bloom 0.05, ambient light
0xFF281E19, fake shadow colour 0xFF61617D, deshade 0.08, rim lighting 1.0, particles on) behind
`chargen_foreground` (the stage field `CharGen_Stage_Field` where the screens below are
placed; `ControlsParent`, the controls shared by the creation stages, §2.3; and an error-text
list `CharGen_ErrorTextList` built from `CharGenErrorTextTemplate`). The viewport shows the
race's `RaceTable_UIScene` (a 0x21 record, `misc-records.md` §4) with the figure of the stage
standing in it (§2.1.1–§2.1.4), seen through the cameras of `RaceTable_Chargen_Camera` /
`RaceTable_Character_Select_Camera` and their zoom variants in the gender entry
(`player-character.md` §5.2–§5.3).

`CharGen_Base_Field` also holds `GamePlay_ExaminationTooltip_Activatable` (a `Field`,
192 × 128 at 571, 12, CS15), by its label the in-world examination tooltip's anchor. **Tessera
rule (P1):** never created on these screens (they have no examinable objects).

> **Answered (2026-10-10, §2.1.1–§2.1.4, BIN):** the figure stands at the scene's
> `Holding_Loc_Entity_00` marker with that marker's rotation (§2.1.2); the view is the scene's
> `chargen_Camera` element moved to the stage camera's frame (§2.1.3); the turntable moves the
> camera, not the figure (§2.3); the scene's objects, lights and decals are drawn around the
> figure (§2.1.1); the race-and-class figure carries stage key 1, not 7 (§2.1.4). The viewport's
> lighting values: ambient by the Tessera rule of §2.1.1, the others OPEN there. The note as
> raised:
>
> **OPEN:** (implementer, `apps/client/src/screens/viewport.rs`, `apps/client/src/walker`,
> 2026-10-10) Where the figure stands in the race's `RaceTable_UIScene` and how the scene's
> camera element (U4 of `misc-records.md` §4.2) frames it are not stated: the scene's
> elements carry placements, but which element (or which point) the screens' figure is
> placed at, and with which heading, is not given; nor whether the six `chargen_viewport`
> values above (shadows, bloom, ambient light, fake shadow colour, deshade, rim lighting)
> apply to the figure. The figures of the creation stages also play other idle clips than in
> the world (the species tables' scripts choose them by `CharGen_Stage`), and these clips
> turn some bodies away from a camera on the figure's front. As implemented: the viewport
> hosts external content (`render.md` §6.10) into which the client draws the figure alone,
> as the paperdoll's portrait: at the origin, turned by the turntable, seen from its front
> at the height of its bounding sphere's centre with a fixed light, on a transparent
> background; the zoom (§2.3) halves the distance and aims a quarter of the sphere's radius
> higher. The scene's objects, lights and cameras are not drawn. Please give the figure's
> place and heading in the scene and the rule for the viewport's lighting values.

<a id="s2-1-1"></a>
#### 2.1.1 Scene (data Confirmed, CS19; scene and camera value BIN, CS20)

On every stage the screen sets the viewport's `UICore_Viewport_uiscene` to the scene of the
race in view (`RaceTable_UIScene`: on the selection screen the selected character's race, on
the creation stages the chosen race) and its `UICore_Viewport_camera` to the UISceneEntity key
`chargen_Camera` (0x10000008 of mapper 0x230000B9) (BIN). A different scene replaces every
element of the old one; the figure is added again (§2.1.2). With the camera value set, the
camera rule of `misc-records.md` §4.2 U4 selects the scene's first element named
`chargen_Camera`; every race scene has one (CS19). §2.1.3 says where that camera is put.

What a race scene holds (OBS2, all 8): 21 to 732 elements, created as `misc-records.md` §4.2 U4
rule 1 says. Physics elements (13 to 686 per scene) are the set: ground, rocks and buildings,
an object under the figure's place, sky domes and horizon objects scaled up to 100 times, water
planes, and effect entities whose start-up scripts run particle effects; one of them is the
not-drawn marker of §2.1.2. Light elements: 3 to 41 per scene, with `LightRange`,
`LightDiffuse`, `LightDiffuseIntensity`, `LightFalloff`, `KeyLight` and `ShadowCaster` in their
property sets; 1 to 4 per scene have `KeyLight` true (ranges 10–800 m), and 0 to 3 have
`ShadowCaster` true. Decal elements in 2 scenes (2 and 5). Camera elements (4 to 7 per scene)
named by UISceneEntity labels.

**Presentation (Tessera rule; the original's viewport drawing is OPEN in `ui-widgets.md` §12):**

1. Draw every Physics element as a static object at its placement (a descriptor without a
   placement stands at the origin with identity rotation), with its descriptor's property set
   applied as on landblock statics (`entity-descriptor.md`; for example
   `Render_AdjustableScale`, `Render_Color`, `Render_Opacity`, `Render_Draw`) and its start-up
   scripts run, so effect entities start their particle systems (`particle-runtime.md`).
2. Every Light element is a point light at its placement with the light properties of its
   descriptor (`lighting.md` §8.3); the objects of the scene, the figure included, receive the
   scene's lights by `lighting.md` §8.5 (KeyLight first).
3. Decal elements are drawn as placed decals (`decals.md`); Camera elements are not drawn.
4. The scene has no environment of its own: no sun or moon, sky description, game time,
   landblock terrain or water body; its sky and ground are its own objects. The ambient input
   *A₀* of `lighting.md` §5.3 is the viewport's `UICore_Viewport_AmbientLight` (0xFF281E19 on
   `chargen_viewport`, read from the layout), the hemispheric input is zero. Fog follows
   `misc-records.md` §4.1 U3 (off for every race scene of this install).
5. The figure is one more object of the scene: it is lit, shadowed and fogged like the others.

**Colour and opacity of scene objects (OBS3, CS34; rule 1).** Yes, rule 1 includes them: of
the 2,830 Physics elements of the 8 race scenes, 207 carry their own `Render_Color` and 6 their
own `Render_Opacity`, and a few object chains carry them too; the element's own value wins over its chain's, as for
`Render_AdjustableScale`. **Tessera rule** (the client's path from these two properties to the
drawn object was traced only for sky objects, `environment.md` §11.4 step 5 and §11.7): an
object's per-object tint of `material.md` §8.3 is (R, G, B of `Render_Color` as fractions of
255, `Render_Opacity`), white and 1 where a property is absent; the tint multiplies the material
colours as §8.3 there says and its fourth value is the object's opacity.

> **OPEN:** what the alpha byte of an element's own `Render_Color` does (below 255 on 10
> elements, for example 112 and 178); the sky path sets it to 255 and carries opacity in
> `Render_Opacity`. **Tessera rule:** ignored.

> **OPEN:** how the original applies the other viewport values (`UICore_Viewport_UseShadows`,
> `…BloomIntensity`, `…BloomCompensationAmount`, `…FakeShadowColor`, `…DeshadeStrength`,
> `…RimLightingIntensity`) and whether the viewport's ambient replaces or adds to an ambient of
> the scene's lights. **Tessera rule until then:** the values are read and kept; ambient as in
> step 4; the others are not applied until `lighting.md` or `render.md` defines bloom,
> deshading, rim lighting and fake shadows; `UseParticles` true lets effect entities run.

> **OPEN:** (implementer, `apps/client/src/walker/stage.rs`, 2026-10-10; not a spec gap, an
> implementation gap recorded so that the rule above is not claimed complete) As implemented:
> Physics elements are drawn through the object layer's instanced models at their placements
> and `Render_AdjustableScale` (the element's own draw values winning over its object chain's),
> lit by the scene's Light elements (range in force by day, intensity curve at the material
> clock, *σ* = 1) and the viewport's ambient, with no sun, no hemispheric input and no fog;
> the figure is lit the same way. Not drawn yet: Decal elements (the walker has no drawing of
> placed decals), the particle systems of effect entities' start-up scripts, and parts whose
> surfaces cannot be drawn as instanced models (records with programs of their own,
> `material.md` §11.6; left out and logged).

<a id="s2-1-2"></a>
#### 2.1.2 Where the figure stands (BIN, CS20; data CS19)

The figure of the stage (§2.1.4) is added to the scene and placed at the position and rotation
of the **first** Physics element of the scene (descriptor type 7, stored order) whose object's
setup has the holding location `Holding_Loc_Entity_00` (key 0x10000042 of the HoldingLocation
mapper, 0x23000068 here; frame list A, `setup.md` §6.3; the setup is reached from the element's
object DID by the chain of `setup.md` §3.1). The holding location's own frame is not added on
top (in this install it is the identity at the origin, so both readings give the same place).

In this install every race scene has exactly one such element, the marker entity 0x4700061C
(`Render_Draw` false, so it draws nothing) at the scene's origin (CS19):

| Scenes | Marker rotation | Figure faces | The race cameras stand |
|---|---|---|---|
| 0x21000006–0x21000009, 0x2100001A, 0x2100001D, 0x2100001F | identity (no placement stored) | +Y (an object faces +Y, `coordinates.md` §11.3, `mesh.md` §11.1) | near the +Y axis, looking toward −Y |
| 0x21000012 (Beorning) | −90° about +Z | +X | on the +X axis, looking toward −X |

So with the cameras of §2.1.3 every figure is seen from its front at the start of a stage,
whatever its clip does afterwards (the stage clips, chosen by the stage keys of §2.1.4, are
part of the figure's own animation). A scene without such an element: **Tessera rule** (none
in this install): the origin with identity rotation.

<a id="s2-1-3"></a>
#### 2.1.3 Camera (BIN, CS20, CS21)

The view is the scene element `chargen_Camera` (§2.1.1). Its own placement and `CameraFOV`
are not used: on each stage the screen keeps two frames and puts the camera between them.

- **Base frame:** the position and rotation of the scene's first element whose UISceneEntity
  identifier (`misc-records.md` §4.2 U4 rule 2) equals the stage camera: the race table's
  `RaceTable_Character_Select_Camera` on the selection screen, `RaceTable_Chargen_Camera` on the
  creation stages (in this install the two are equal for every race).
- **Zoom frame:** likewise for the gender entry's `RaceTable_Character_Select_Zoom_Camera` or
  `RaceTable_Chargen_Zoom_Camera`.
- **Field of view:** 65 at the base frame, 40 at the zoom frame (degrees; the axis is the one of
  the other cameras, `walker.md` W17 OPEN); a value outside 1–100 is not applied.
- **Orientation convention:** a camera looks along its local +Y axis with local +Z up, the
  convention of objects (`coordinates.md` §11.3, `mesh.md` §11.1). Every camera element the
  race tables name lies near the +Y axis (on +X in 0x21000012), 0.87–2.8 m horizontally from
  the scene origin and 0.99–1.74 m above it, and looks toward the figure (CS19).

The camera frame for a zoom fraction *t* (0 = base, 1 = zoom) and a turn angle *θ* (§2.3):

1. Interpolate between the base and zoom frames by *t*: the position linearly, the rotation
   from the base rotation to the zoom rotation by the same fraction (**Tessera rule:**
   spherical linear interpolation; the client's frame blend was not traced in detail).
2. Turn the result about the vertical axis through the figure's position by *θ* degrees: the
   camera's horizontal offset from the figure is rotated, and its view direction turns with
   it. A positive *θ* moves the camera counter-clockwise seen from above (from +X towards +Y).
3. The field of view is (1 − *t*) × 65 + *t* × 40.

When the scene or the stage camera is set (a new stage, race or gender, or a new selected
character), *t* and *θ* are set to 0 at once. The turntable and zoom controls of §2.3 change *t*
and *θ*.

> **OPEN:** a race whose camera element is missing from its scene (none in this install).
> **Tessera rule:** keep the previous frames; log it.

<a id="s2-1-4"></a>
#### 2.1.4 The figure of each stage (BIN, WLB, CS22; `Chargen` script BIN2, OBS3, CS28–CS30)

| Stage | Figure | `CharGen_Stage` | Other properties the screen sets |
|---|---|---|---|
| Selection (layout stage 4) | the selected character: its body and its stored appearance (`appearance.md` §8; §8 below for the scenario feed) | 4 (`CharacterSelect`) | `CharGen_Action` 2 (`Idle`, mapper 0x23000133) at creation; the screen sets 3 (`Hover`) when the pointer moves onto the figure and 2 when it moves off, except on a figure that has been clicked (the clicked figure keeps its value; §2.3 says how hover and click are found); the start-up script `Chargen` (below) |
| Race and class (layout stage 7) | a new figure of the chosen race and gender, made from the entity the class library returns for the gender entry: slot 14 of the gender-entry object (class 859), which in all 15 entries of this install holds the `PhysObj` of the entry's `RaceTable_Gender_PlayerAvatar` (WLB, OBS2); it is created with the property set the screen keeps for the character being made | 1 (`ClassSelect`) | `CharGen_Action` 2; `Agent_Class` = the chosen class key; `Render_AlwaysSelectable` true; the start-up script `Chargen` (below) |
| Appearance and name (layout stage 6) | the same figure, kept from the race-and-class stage | 6 (`Appearance`), also while the name is typed | `CharGen_Action` 2 (the figure keeps its running `Chargen` script) |

The race-and-class figure is made again when the race or gender changes; on the creation
stages it carries its stage key from the start of its scripts (the key is in the property set
it is created with); entering the appearance stage changes the key of the existing figure.
The client sets no figure's `CharGen_Stage` to 7 (`RaceClass`) or 3 (`Name`): those keys name
layout stages only (§2.2). The figure's scripts choose the stage clips by these keys, so the
key decides the figure's pose; the script that does so is `Chargen`.

**The `Chargen` start-up script (BIN2, CS28).** Before it creates the selection figure or the
race-and-class figure, the screen adds one script to the figure's start-up list: in the
property set the figure is created with, it appends the script key `Chargen` (key 0x100004AB
of the script-key mapper, 0x23000097 in this install; read the key by its label at run time)
to the `ScriptTable_StartupScriptNames` list of the set's `Entity_InstanceScriptTable`
(`instance-scripts.md` §3), keeping what the list already holds and creating the table and the
list when the set has none. When the figure is activated it therefore starts `Startup`, then
the class table's start-up names, then `Chargen` (`instance-scripts.md` §4.2), and it runs all
of them by the ordinary rules of `script-runtime.md`; nothing else in the screens chooses a
clip. The appearance stage keeps the figure and with it the running script, whose Switches
react when the stage key changes.

**What `Chargen` plays (OBS3, CS29, all 15 gender entries; read at run time):**

1. Every avatar's table closure defines `Chargen` in two tables, both of which start
   (`script-runtime.md` §12.2): one shared by all avatars (0x0700042D here; a persistent
   Switch on `Entity_Mode` whose cases run other scripts) and one creation table of the body
   (14 distinct tables for the 14 avatar entities).
2. The body's definition holds a persistent Switch on `CharGen_Stage` with cases 4, 1 and 6,
   a transition port (type 28, `script-table.md` §4.3) and a Default port:
   - **4** (selection): the body's selection idle;
   - **1** (race and class): a persistent Switch on `Agent_Class` with one case per class key
     (9 to 11 keys per body) and a Default; each class case starts a looping stance clip that
     covers the whole skeleton (layer 80; layer 78 for 7 of the 11 classes of the Dwarf and
     the Stout-axe bodies) and holds Appearance nodes (kind 58: the class's costume pieces,
     applied as `appearance.md` §7.4 "Applying an Appearance node" says) and for some classes
     MeshFX nodes (kind 41: props); a few classes add a Sequence of Random choices among one-shot gestures on
     layer 90 over the stance;
   - **6** (appearance and name): the body's appearance-stage idle;
   - the transition port and the Default start one-shot or idle clips of their own.
3. The clips the other start-up scripts start keep running underneath: the idle-emote on
   layer 15 or 18 (`script-table.md` §8.3), the layer-110 proportion clip of the River-hobbit
   and Stout-axe bodies (`script-table.md` §6.8) and, where a body's start-up scripts switch on
   `CharGen_Stage` themselves, their clips. The locomotion template's own stage branch
   (`script-table.md` §8.4 step 1: parameter `chargen_class_idle` for key 1) is bound by no
   avatar's binding table in this install, so it plays nothing.

Result (CS30, testable as CSI9): with `Chargen` running, each of the 15 bodies, with
`CharGen_Stage` 1 and any class key its cases name, with 6 and with 4, keeps a looping clip on
a layer below 110 that covers its whole skeleton (no track weights, or `UseBoneOpacity` 0):
189 of 189 runs. Without `Chargen`, no stage-1 figure has one (0 of 159), and on stage 6 the
two Stout-axe and the two River-hobbit entries have none: the bind poses, the Stout-axe figure
"too tall" and the River-hobbit female "waist-deep" of the implementer note below.

**One-shot stage clips.** A clip that a stage branch starts with `LoopForever` 0 and
`ManualUpdate` 0 plays as `script-table.md` §6.10 steps 3, 4 and 8 say: once from its start
fraction (`LoopCount` times its duration, less the part after `EndTime`), with its fade-out over
`RampOut` ending at its end time, and is then removed. It is not held at its last frame (only
`ManualUpdate` clips are held, `script-table.md` §6.7) and not looped. What follows is the
graph's: a Sequence or Timer may start the next gesture; the looping stance underneath shows
again meanwhile.

**Height and framing.** The figure's own position is the marker's (§2.1.2, BIN, CS20): no step
of the screens raises or lowers it, scales it for the camera or measures it, and the cameras of
§2.1.3 do not depend on the figure. A figure that stands below the marker or does not fit the
camera lacks its stance clip. With `Chargen`, the River-hobbit female's appearance-stage stance
is her in-world idle (0x05008EE7 here) and the Stout-axe body's is its in-world idle
(0x05007A13 here; both in the table of `script-table.md` §8.3).

> **OPEN:** whether the pieces an Appearance node added (and the props of a MeshFX node) are
> taken off when the node stops, that is when the class or the stage key changes. **Tessera
> rule:** they are: a stopped node's element is removed from the figure's appearance and its
> prop detached, so the figure wears only the pieces of the branches that are running.

> **OPEN:** what `RaceTable_RaceSelect_Entity` (§4.1) is used for; it is not the figure of any
> stage traced here. **Tessera rule:** not used by the screens.

> **OPEN:** (implementer, `apps/client/src/walker/characters/live.rs`, 2026-10-23; an
> implementation note so that the rules above are not claimed complete, not a spec gap; it
> replaces the note of 2026-10-10) The screens' figure now runs its scripts live with
> `tessera-scriptgraph` (`script-runtime.md`): `Chargen` is appended to its start-up names
> (after `Startup` and the class table's names) and `Render_AlwaysSelectable` is set on
> the race-and-class figure as before; once the figure is on the GPU its `Startup`,
> start-up names and `Chargen` run over time, so Random nodes play their gestures, later
> Sequence and Timer steps run, and Switch transition ports run. Animation nodes play on
> the figure's animator by `script-table.md` §6.10 (the held layer-110 proportion clips
> stay with the shape path and are not played twice); a looping clip that is still being
> decoded on the workers when its node starts begins once it is decoded (the frame thread
> reads no clip), a one-shot one is skipped that time. Clips started while the figure's
> start-up scripts are being started take `RampIn` 0 (`script-table.md` §6.10 step 1);
> **question:** §6.10 says "while the entity is running one of its start-up scripts", and
> a start-up script keeps running (its Random gestures, later Sequence steps); Tessera
> reads it as the start of those scripts only (the clips started when the figure is
> spawned), so later gestures ramp in by their own `RampIn`, until the spec says which.
> MeshFX nodes of the class branches show their props, with their driven fields applied
> (each prop entity is built with the figure, hidden while its node does not run; the
> Tessera rule above); a node finds its prop by entity, holding location, bone type,
> offset, rotation and scale, so two nodes that attach the same entity at different
> anchors show two props. A change of
> `CharGen_Stage` or `CharGen_Action` is a property change of the running figure: nothing
> is built again and the clips that keep running (the idle) are not restarted; the
> figure's status then reports the keys the scripts run with and the clips they play (what
> the CSI9 stance check reads). The figure job evaluates the scripts with every branch and
> resolves each stage value with the figure's class on a worker, keeps the script records
> they reach for the runtime and decodes the clips they reach. Pieces of Appearance nodes:
> the job resolves the start-up for each case value of the `CharGen_Stage` Switch of
> `Chargen`, assembles each distinct piece set once and draws the set of the current stage
> key (the Tessera rule above, for stage changes). Not done: pieces of Appearance nodes
> and colours of EditMaterial nodes that change with anything but the stage key (they keep
> the build's resolution; a class change builds the figure again, as before); a prop's
> fade, life and destroy times and `AttachToParent` false (a prop is shown while its node
> runs, attached); sounds and particle nodes of the screens' figure (the viewport draws
> neither). Without the install's script vocabulary the figure keeps the static resolution
> of `script-table.md` §11.3 and is built again on a key change. Real data: the stance
> check of CSI9 on Tessera's side passes on all 15 bodies on both stages, and the class
> props attach (`chargen_bodies_real_data`).

> **Answered (spec writer, 2026-10-23):** the question on `RampIn` is settled by
> `script-table.md` §6.10 step 1 as revised (row T48, BIN): the replacement by 0 covers only the
> clip requests made while a start-up script is being started; later gestures, Sequence steps,
> Random choices and Switch changes of the same script ramp in by their own `RampIn`. Tessera's
> reading is the client's. The rest of the note is a build status: the items listed as not done
> stay open for the implementer (`script-table.md` §5.2 now gives the prop anchor; fade, life
> and destroy times and `AttachToParent` 0 are `appearance.md` §5.7 and `script-table.md` §5.2).

> **OPEN:** (implementer, `apps/client/src/walker/characters/figure.rs`, 2026-10-23) Where a
> MeshFX node attaches its entity: `script-table.md` §5.2 lists `HoldingLocation`,
> `BoneType` and `BoneOffset` but no rule, and several MeshFX nodes the screens' figures
> reach have `HoldingLocation` 0. **Tessera rule** (taken from the particle
> attachment of `particle-runtime.md` §6.1, not stated for MeshFX): `BoneType` ≠ 0 attaches
> to the bone of that bone type (`setup.md` §6.2) at an identity frame; else `HoldingLocation`
> ≠ 0 attaches at that holding location's frame; both 0 attach at the holder's model origin;
> in each case `BoneOffset`, the offset rotation and `Scale` are applied as for held items.

> **Answered (spec writer, 2026-10-23):** `script-table.md` §5.2 "Where a MeshFX node attaches
> its entity" (row T47, BIN). The Tessera rule above is the client's for unattached nodes
> (`AttachToParent` 0). For attached nodes (`AttachToParent` 1) `HoldingLocation` wins when it is
> not 0, then `BoneType`; with both 0 the node uses the holding location with key 6, which on
> every playable body is the holder's model origin (no playable body's setup holds a key-6
> element), so the "both 0" case keeps its result. A holder that lacks the named location or
> bone shows no prop. `BoneOffset`, the offset rotation and `Scale` apply as `appearance.md` §5.7
> says.

> **Answered (2026-10-10, BIN2, OBS3, CS28–CS30):** the stance comes from the start-up script
> `Chargen`, which the screens add to every stage figure (above); a one-shot stage clip plays
> once and is removed; the figure stands on the marker and the cameras ignore its size. The
> walker's idle selection (`script-table.md` §11.3) does not run `Chargen`; run the figure's
> scripts as above. The note as raised:
>
> **OPEN:** (implementer, 2026-10-10, real data, `tessera-client` with the keys of the table
> above) With `CharGen_Stage` 1, `CharGen_Action` 2 and `Agent_Class` set, the species tables
> of 13 of the 15 race-and-class figures (all but the two River-hobbit bodies) start only
> clips on layer 18 that drive part of the skeleton (`UseBoneOpacity`), so Tessera shows these
> figures in their bind pose (arms stretched out); with key 6 the same bodies also start a
> looping layer-10 clip and stand. The Stout-axe body (species 0x78, both gender entries)
> shows its bind pose on the appearance stage too (arms out, taller than the dwarf body, so
> the stage camera does not hold the whole figure), and the River-hobbit female's only
> appearance-stage clip (layer 30, not looping) leaves her standing waist-deep below the
> marker. Not stated: which clip (or which further property of the figure) gives the
> race-and-class figure its stance, and how a stage clip that does not loop is played (once,
> held at its last frame, or looped). As implemented: the walker's idle selection
> (`script-table.md` §11.3; looping clips one per layer, else the most likely candidate).

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

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) how the side panels fit a window whose
> aspect differs from the 1440 × 900 design. In this install both side panels of the
> appearance screen have object mode Stretch and edge modes
> (5, 5, 6, 5) and (6, 5, 5, 5); laid out by `ui-layout.md` §8.4 they take the window's
> height and a width of height × their design aspect (at 1280 × 720: 400 × 720 from
> 500 × 900), but their children keep their design boxes, so the right panel's controls and
> current-choice texts reach about 65 pixels beyond the window's right edge and both panels'
> lower parts beyond its bottom. Whether the original scales a Stretch panel's content to the
> panel's new size here (`ui-behaviour.md` §9.2.1 element resize: the layout box kept and a
> scale vector set) or lays its children out again is the question raised in
> `ui-layout.md` §8.4 (OPEN of 2026-10-10). Not changed in Tessera: the defect stays until it
> is answered.
>
> **Note (2026-10-16, decision 0040):** answered for Tessera by
> [`ui-scaling.md`](ui-scaling.md) §5.3–§5.4 (the side panels are Type B fit containers in
> the safe box, their content scaled uniformly with the panel) with numbers in its §10.2;
> takes effect when that spec is Accepted and built.

<a id="s2-3"></a>
### 2.3 Shared controls (Confirmed data, CS8; turntable and zoom BIN, CS21; stage rules are Tessera rules)

`ControlsParent` of layout `chargen` (a child of `chargen_foreground`, full design size,
edge modes 6, 5, 6, 5, object mode stretch) holds the controls that stay on screen across the
stages. Positions are in design units (1440 × 900) relative to `ControlsParent`.

| Element | Base element (layout) | Box (x, y, w × h) | Data | Function |
|---|---|---|---|---|
| `CharGen_Back_Button` (label child `CharGen_Back_Button_Label`) | `CharGen_Button_ScreenNav_Left` (0x22000004) | 386, 810, 261 × 45 | rollover; tooltip on; activate on show | back one stage |
| `CharGen_Exit_Button` (label `CharGen_Exit_Button_Label`) | `CharGen_Button_ScreenNav_Left` (0x22000004) | 386, 810, 261 × 45 (the same box as Back) | rollover | leave creation |
| `CharGen_Continue_Button` (label `CharGen_Continue_button_Label`) | `Chargen_Button_ScreenNav_Right` (0x22000004) | 790, 810, 261 × 45 | rollover; tooltip on; `UICore_Button_broadcast_when_ghosted` true | next stage |
| `Chargen_Controls` (field, edge modes 1, 0, 1, 0) | — | 0, 780, 100 × 100 | one image item | turntable group |
| — `CharGen_Turntable_Left_Button` | `Chargen_Right_Button` (0x22000004) | 9, 23, 28 × 48 in the group | hot button, first repeat interval 0.05 s | turn the view (left-hand button; §2.1.3 *θ* − 20°) |
| — `CharGen_Turntable_Right_Button` | `Chargen_Left_Button` (0x22000004) | 62, 23, 28 × 48 | hot button, first repeat interval 0.05 s | turn the view (right-hand button; *θ* + 20°) |
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
- **Turntable (BIN, CS21):** the turn buttons do not turn the figure; they change the turn
  angle *θ* of §2.1.3, which moves the camera around the figure. Each `Button_hot_click` of
  `CharGen_Turntable_Left_Button` (`ui-widgets.md` §7.1: one on the press, the next after the
  element's `UICore_Button_hot_click_first_interval`, 0.05 s here, then one every
  `UICore_Button_hot_click_interval`, default 0.05 s, while the button is held) sets the
  target of *θ* to the current *θ* − 20°; each one of `CharGen_Turntable_Right_Button` to the
  current *θ* + 20°. *θ* eases towards its target: from its value when the target was set, the
  remaining difference halves every 0.15 s, and the target counts as reached within 0.0002.
  Dragging across the viewport with the pointer button held sets *θ* at once (no easing) to
  its value at the start of the drag + (*x* − *x*₀) × (−0.5)°, *x* and *x*₀ being the pointer's
  current and starting horizontal positions in the viewport's pixels. Both turn buttons are
  ghosted while a zoom button's camera move (next item) runs. *θ* returns to 0 when the stage
  camera is set (§2.1.3).
- **Zoom (BIN, CS21):** the zoom fraction *t* of §2.1.3 lies in [0, 1].
  `CharGen_Turntable_Zoom_In_Button` sets *t* to 1, `CharGen_Turntable_Zoom_Out_Button` to 0,
  and either moves the camera from where it is to the frame of §2.1.3 for the new *t* (and the
  current *θ*) in a timed move: duration *T* = ½ × (*d* / 3 + ½ × (2 − *c*)) seconds, *d* being
  the distance in metres between the start and end camera positions and *c* the cosine of the
  angle between the start and end view directions; a progress *u* grows from 0 to 1 at the rate
  1 / *T*; the camera is placed between the start and end frames at the fraction
  ½ × sin((*u* − ½) × π) + ½ (eased at both ends) and its field of view at the fraction *u*
  between the start and end fields of view. The mouse wheel over the viewport moves the target
  of *t* by 0.15 per notch (`ScrollUp` +0.15, towards the zoom frame; `ScrollDown` −0.15,
  towards the base frame; BIN2, CS31), clamped to [0, 1]; *t* eases with a
  half-life of 0.1 s as *θ* does. The zoom-in button is ghosted while the target of *t* is at least
  1 − 0.0002, the zoom-out button while it is at most 0.0002. *t* returns to 0 when the stage camera is set.
- **Avatar-update toggle:** the check box shows and writes `GOP_AvatarUpdateVisible`
  (`appearance.md` §3.3: a graphics option kept in Tessera's settings, default on; the figure
  rebuilds when it changes). The species condition is the data-backed part (`appearance.md`
  §3.3, BIN2 there); the stages it appears on are the Tessera rule above.

> **OPEN:** the original's stage-by-stage visibility of these controls and when it offers
> Exit (owner check O85). (The turn step, speed and zoom are answered above, BIN.)

> **Answered (2026-10-10, BIN2, CS31, CS32; OBS3, CS29):**
> 1. **Wheel:** a wheel notch over the viewport arrives as `ScrollUp` (0x82) or `ScrollDown`
>    (0x10) (`ui-widgets.md` §2.4; `keymap.md` K42 binds `ScrollUp` to the wheel's positive
>    axis, `ScrollDown` to its negative axis); `ScrollUp` moves the target of *t* by +0.15,
>    towards the zoom frame, `ScrollDown` by −0.15, clamped to [0, 1], with the half-life of
>    0.1 s above (the Zoom item now says so). As implemented.
> 2. **Wheel and drag during a zoom button's move:** the original's wheel and drag handlers do
>    not test whether that move runs; what the view shows while both act was not traced.
>    Tessera's choice (ignore them during the move) stays a **Tessera rule**.
> 3. **Field-of-view axis:** not traced (Q15); the vertical reading as implemented follows the
>    Tessera Decision of `walker.md` W17.
> 4. **Hover and click:** each frame the screen asks the viewport's 3D view which objects lie
>    under the pointer and takes the first one it reports. When that object changes, the
>    figure it was (if it is a figure of the screen and not the clicked one) gets
>    `CharGen_Action` 2 and the new one (likewise) gets 3. A `SelectLeft` command on the
>    viewport makes the figure under the pointer the clicked figure. **Tessera rule** for "under
>    the pointer" (the view's picking was not traced): the nearest figure hit by the ray from
>    the camera through the pointer, tested against the figure's drawn triangles or, failing
>    that, its bounding box. No node of any script record of this install reads
>    `CharGen_Action` (CS29), so on this install the hover value changes no clip or effect,
>    and a selection figure that keeps 2 looks the same.
>
> The note as raised:
>
> **OPEN:** (implementer, 2026-10-10; review note (n1)) which wheel direction zooms in.
> **Tessera rule as implemented:** a notch away from the player (`ScrollUp`) moves the target
> of *t* by +0.15 (towards the zoom frame), `ScrollDown` by −0.15. Also as implemented
> (Tessera rules, not stated above): while a zoom button's camera move runs, drags and wheel
> notches are ignored as well as the ghosted turn buttons; the field of view is applied as the
> vertical one (`walker.md` W17 OPEN); the hover of the selection figure (`CharGen_Action` 3,
> §2.1.4) is not detected, since the viewport has no picking of its figure yet, so the
> selection figure carries 2.

> **OPEN:** (implementer, `apps/client/src/pre_world.rs`, 2026-10-10; engineering choices of
> the Tessera rule for "under the pointer") As implemented: "the figure's drawn triangles"
> are not tested; the box is the posed figure's joints (the pose's model-space bone
> positions) widened by 0.15 m on every side, and the ray is tested only while the viewport
> is the UI element under the pointer (a panel over the viewport hides the figure from it).
> The press that starts a drag over the viewport is the `SelectLeft`: with the figure under
> the pointer it becomes the clicked figure, without it there is none; a new selected
> character or another stage clears it. A change of `CharGen_Action` builds the figure again
> (§2.1.4 implementer OPEN), so its idle restarts when the pointer moves onto or off it.

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
| Gender buttons | `CharGen_Male_Button` and `CharGen_Female_Button` in `CharGen_Gender_GroupBox`; each carries its sex in `UI_RaceSelection_Gender` (Male 4096, Female 8192, layout data); a race with one gender entry hides the whole group (rule below, CS27) |
| Race information (`Chargen_Race_Info_Panel`) | `RaceTable_Description`; racial traits from `AdvTable_RaceCharacteristic_List` (§5.2) as `CharGen_Race_Select_Racial_Trait_Template` rows (`TraitDisplay`: trait icon and name). Which traits the screen lists (all, or only those of a low `AdvTable_Trait_Level`) was not traced; **Tessera rule:** all, by ascending level, then stored order |
| Race movie button | `RaceTable_RaceSelect_Movie_DID` (P1: hidden) |
| Disabled overlay, store coin | `RaceTable_CharGen_ClassSelect_WebstoreItem` (store) — **Tessera rule:** every race enabled |
| Viewport | the race-and-class figure of §2.1.4 (the gender entry's avatar entity, `CharGen_Stage` 1), at the marker of §2.1.2 in the race's scene, camera of §2.1.3 (`RaceTable_Chargen_Camera`); `RaceTable_RaceSelect_Entity` is not used (§2.1.4 OPEN; corrected 2026-10-10) |

**Gender group (Confirmed data, BIN rule, CS27).** `CharGen_Gender_GroupBox` is a GroupBox
(`ui-widgets.md` §3.4) whose `UICore_GroupBox_default_button` is `CharGen_Male_Button`; both
buttons have `UICore_Button_boolean_button` true and the `Chargen_Race_Select_Gender` base
(0x22000004), whose three image parts carry their own images for the `highlight` states. So:

1. The **selected** gender button is the GroupBox's selection and is in the `highlight` state;
   the other is in `normal` (`ui-widgets.md` §3.4 selection change, §7.3). When the tree is
   complete the GroupBox selects its default button, the male button. A click on the other
   button moves the selection and the highlight; a click on the selected button keeps it lit
   (`ui-widgets.md` §3.4 "Clicks").
2. The screen reads the chosen sex from the selected button's `UI_RaceSelection_Gender`
   whenever the race or the selection changes, and makes the stage figure for (race, sex)
   (§2.1.4).
3. Each race entry of the race list carries `UI_RaceSelection_Race` (its species) and
   `UI_RaceSelection_AllowGenderSelect`, which the screen sets to true exactly when the race
   table has more than one gender entry. When the selected race's value is false, the screen
   hides `CharGen_Gender_GroupBox` and sets its selection to the male button; when true, the
   group is shown and keeps its selection, so the sex chosen for one race is kept for the next
   (this answers item (b) of the §5.2 implementer note). In this install every race has a male
   entry, and the races with one entry (Dwarf) have only a male one.
4. To show a character's sex on entering the race and class screen (§6 step 2), the screen
   sets the GroupBox's `UICore_GroupBox_selected_button` to that sex's button (**Tessera
   rule**; the original's entry default is O85).

> **OPEN:** (implementer, `apps/client/src/screens/ui.rs`, 2026-10-10) how the race icon fills
> its part. With the part looked up as the §4.2 OPEN says (`CharGen_Race_Select_Race_Icon`),
> the gender entries' `RaceTable_RaceSelect_Icon` images are smaller than that part, so
> drawn as element images (`ui-layout.md` §9.6: native size, tiled over the element) each
> row shows one and a half icons across and down, reaching into the next row. **Tessera rule
> as implemented:** the icon is drawn once at its own size from the part's top-left corner
> (a child field of the image's size, as for layered item icons), not tiled. Please name the
> part that shows the icon and say whether the image is tiled, scaled or drawn once.

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

> **OPEN:** (implementer, `apps/client/src/screens/ui.rs`, 2026-10-10) the list that holds the
> nationality entries is not named: `CharGen_Nationality_Select_Panel` is a panel, not a list
> box, so entries added to it are refused and the list showed nothing. **Tessera rule as
> implemented:** the entries go to the first list box below the panel (tree order); a click
> on an entry or a selection in that list chooses the nationality, and the chosen one is the
> list's selection. Please name the list element.

<a id="s5-2"></a>
### 5.2 Appearance controls

`CharGen_Appearance_Panel` (right): the title, the list `CharGen_Appearance_Controls_Listbox`
(one column, scroll bar `CharGen_Appearance_Controls_Scrollbar`) and the randomise button
`CharGen_Appearance_Randomize_Button`. The list holds the gender's player avatar's
`AppearanceUI_Controls` (`appearance.md` §3.4) whose `AppearanceUI_Availability` has the
`CharGen` bit: first the controls without `AppearanceUI_Group` (or with group 0) in stored
order, then one group header per group key 1 to 9 that has controls, in ascending key order,
each holding its controls in stored order (BIN, CS25; §5.2.2). Each control is shown by the
template of its kind:

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

> **Answered (2026-10-10, BIN, CS25, CS27):** headers in ascending group key after the ungrouped
> controls (the list rule above), not in the order of their first control; (b) the sex is kept
> when the new race offers a choice of sex, else the male button is selected (§4.1 gender
> group, rule 3). The class-order part and (a) stay as implemented (Tessera rules; the
> `CharGen` label exists in this install). The note as raised:
>
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

> **Answered in part (2026-10-10, BIN, CS26; §5.2.1):** a style control writes an
> appearance-list element (key, the control's own record, the chosen block's `variant`, the
> control's toggle record and property) into the figure's `Appearance_InstanceList`, and, when it
> names `AppearanceUI_ControlScriptPropertyName`, sets that property to the same `variant`; so
> `AC_Mesh_Head` takes the block's `variant`, as implemented. The choices are the blocks of
> the control's record in stored order. Still OPEN: whether blocks of equal `variant` count as
> one choice, and what `AppearanceUI_EndOldOptionsControlValue` and
> `AppearanceUI_OldOptionsWeightValue` change (the client reads both into the control; their
> use was not traced). The note as raised:
>
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

> **Answered in part (2026-10-10, OBS2):** the randomise button is
> `CharGen_Appearance_Randomize_Button` (label child `CharGen_Appearance_Randomize_Label`); the
> group header element and the inner parts of the six control templates are named in §5.2.2.
> The menu-panel buttons, the title-box labels and the nationality entry parts stay OPEN.
> The note as raised:
>
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

<a id="s5-2-1"></a>
#### 5.2.1 Starting values (BIN, CS26)

When the appearance stage is entered (and whenever its list is built again for a new race or
sex), the screen builds the controls in the list order above from the stage figure (§2.1.4) as
it stands. Each control reads its starting value from the figure and at once writes that value
back to the figure, before the next control is built, so a later control sees what the earlier
ones wrote. The values are those of `appearance.md` §3.4 "Starting values on the creation
screen": in short, every style starts at the block of the control's own record that the
figure's instance list names (in this install always the first block, `variant` 0.00), and
every other control at the figure's own value. A presets control starts at its first preset but
does not apply it at build time (in this install each presets control is its avatar's first
control). Nothing is randomised at build time; the randomise button acts only when clicked.

Examples (OBS2): a fresh Beorning female gets head block 0.00 of 0x200029B9 (the control's
toggle record while `GOP_AvatarUpdateVisible` is on, else 0x20002AC6; both give the head mesh
0x0600D8C2) and hair block 0.00 of 0x200029C0 (or 0x20002AFB); a fresh High-elf male gets head
block 0.00 of 0x20002B4D, which replaces the setup's default head, and hair block 0.00 of
0x20002B3D (not the 0.03 block its entity list names, since the class list is not consulted).

<a id="s5-2-2"></a>
#### 5.2.2 Labels, current choice and group headers (BIN, OBS2, CS23–CS25)

The templates of §5.2 are based on control widgets of other layouts (OBS2); their parts:

| Template (base element, layout) | Parts the screen fills |
|---|---|
| style, face layer, presets (`APRAppearanceUIElement_BaseField` 0x22000086, `CompositorAppearanceUIElement_BaseField` 0x22000085, `PresetsAppearanceUIElement_BaseField` 0x22000908; each holds `AppearanceUIElement_BaseField` of 0x2200009F) | `AppearanceUIElement_Label` (161 × 23), `AppearanceUIElement_CurrentChoice` (48 × 23), `AppearanceUIElement_LeftButton` and `AppearanceUIElement_RightButton` (arrows), `AppearanceUIElement_LockButton` |
| colour (`DyeMapAppearanceUIElement_BaseField`, 0x22000088) | `AppearanceUIElement_Label`, `ColorChoice` (a `ColorPicker`, 241 × 32) over `ColorChoice_Bkg`, `AppearanceUIElement_LockButton` |
| body scale (`BoneScalingAppearanceUIElement_BaseField`, 0x22000087) | `BoneScalingAppearanceUIElement_Label`, the slider `BoneScalingAppearanceUIElement_Slider` with its `…_Scrollbar` and the arrow buttons `…_ButtonLeft`, `…_ButtonRight`, `AppearanceUIElement_LockButton` |
| body sliders (`BoneSliderSystemUIElement_BaseField` 0x220008EB, `BoneSliderGroupUIElement_BaseField` 0x220008ED, `BoneSliderAppearanceUIElement_BaseField` 0x220008EC) | their `…_Label` parts; the system and group fields hold the lists `CharGen_BoneSliderGroup_Listbox` and `CharGen_BoneSlider_Controls_Listbox`; each slider has `BoneSliderAppearanceUIElement_Slider` with scroll bar and arrows |
| group header (`AppearanceUIGroupUIElement_BaseField`, 0x2200090A) | `AppearanceUIGroupUIElement_Label`, the list `CharGen_AppearanceUI_Controls_Listbox` holding the group's controls, the buttons `AppearanceUIGroupUIElement_ButtonExpand` and `…_ButtonCollapse` |

Rules (BIN):

1. **Control label.** The label part of every style, face-layer, presets, colour and
   body-scale control shows the text of the control's own `AppearanceUI_ControlStringName` (a
   StringInfo; every one of the 221 controls of the 14 avatar entities has one, all in table
   0x250001BD, CS23); for the slider templates this was not traced (**Tessera rule:** the
   same). The colour template's label carries a fixed text in the layout (token 0x08E35CB2 of
   table 0x250001BD, the name of one particular colour control); it is a placeholder and is
   always replaced by the control's name. So no label shows a number or the placeholder.
2. **Current choice.** For style, face-layer and presets controls,
   `AppearanceUIElement_CurrentChoice` shows the format token 0x0E5568D5 of table
   0x250001BB with argument 0x047D3D75 = the number of the current choice and argument
   0x005662B5 = the number of choices (CS24). Style: choice *i* of *n* blocks is shown as
   *i* (counting from 1) of *n*. Face layer: values run from `AppearanceUI_MinControlValue`
   (default 0) to *m*, the number of options of the control's layer in its compositor
   (`misc-records.md` §3; in this install the options of each of the 120 layers the controls
   name are numbered 1 to *m* without gaps, OBS2); with minimum 0 the value *v* is shown as
   *v* + 1 of *m* + 1, otherwise as *v* of *m*.
   Presets: preset *i* (from 1) of the array's count. The colour and slider templates have no
   current-choice part.
3. **Group header.** The header's label shows the display text of its group key in the
   `AppearanceUI_Group` mapper (0x23000491 here: keys 1–9 `Skin`, `Head`, `Face`, `Eyes`,
   `Hair`, `Nose`, `Jaw`, `Mouth`, `Ears`, each with a display text), never the key itself
   (CS25). A header starts expanded: its expand button hidden, its collapse button shown.
4. **Lock buttons.** A control whose struct has `AppearanceUI_ControlDefaultLockState` true
   starts with its lock button in the `highlight` state, otherwise `normal` (2 controls of this
   install carry the field). What a lock changes is OPEN (Tessera rule: a locked control is
   skipped by randomise).

> **Answered (2026-10-10, OBS3, CS33):** `chargen_appearance` does hold the group template:
> the top-level element `CharGen_AppearanceUIGroupChoice_BaseField` (265 × 80 at 830, 231, edge
> modes 1, 0, 1, 0, like the other `CharGen_…Choice_BaseField` templates beside it) has a base
> reference (`ui-layout.md` §5.6) to `AppearanceUIGroupUIElement_BaseField` of layout
> 0x2200090A, which the base reference names by DID (that layout has no `UILAYOUT` label; none
> is needed). The base element (type `AppearanceUIGroup`, 261 × 40) holds, in its own
> coordinates: `AppearanceUIGroupUIElement_Label` (16, 10, 180 × 17), the lock button
> `AppearanceUIElement_LockButton` (208, 5, 24 × 24, boolean), the buttons
> `AppearanceUIGroupUIElement_ButtonExpand` and `…_ButtonCollapse` (both 234, 5, 24 × 24),
> the list `CharGen_AppearanceUI_Controls_Listbox` (3, 33, 250 × 11, edge modes 5, 5, 5, 5)
> and the frame `AppearanceUIGroupUIElement_Frame_Large` (0, 0, 261 × 40, with top, middle
> and bottom parts). So a group is **one** row of the control list, made from this template,
> that holds its own controls in its inner list box (rule 3 and the table above); the controls
> are not rows of the outer list after a header. Build the group row from this template as
> the control rows are built from theirs. **Tessera rule** (not traced): the collapse button
> hides the inner list and the row shrinks to its top part; the expand button shows it again;
> each hides itself and shows the other. The group's lock button: what it changes is OPEN with rule 4
> (**Tessera rule:** it toggles its `highlight` state, and randomise skips every control of a
> locked group). The note as raised:
>
> **OPEN:** (implementer, 2026-10-10) The group header's element is named by its layout's DID
> only (0x2200090A, rule 3 and the table above); the client finds layouts by their `UILAYOUT`
> labels, and `chargen_appearance` holds no header template of its own. **Tessera rule as
> implemented:** a header row is an instance of the style template (`CharGen_APRAppearanceChoice_BaseField`)
> with only its label part shown, holding the group's display text, placed in the control
> list before the group's controls; groups stay expanded (no expand or collapse buttons).
> Also as implemented: a click on a lock button toggles the lock (its `highlight` state).
> Please name the header layout's label (or another run-time way to find it).

> **OPEN:** (implementer, `apps/client/src/screens/ui.rs`, 2026-10-10) how tall a group row
> is. The template's inner list (edge modes 5, 5, 5, 5) is 11 high and holds no fixed number
> of controls; the rule that sizes the row to its controls was not given. **Tessera rule as
> implemented:** an expanded row is the template's height plus the summed heights of its
> controls less the inner list's own height (the list stretches with the row); a collapsed
> row is as high as the inner list's top, with the list hidden.
>
> **OPEN:** (implementer, `apps/client/src/screens/controls.rs`, 2026-10-10; an
> implementation gap, not a spec gap) Rule 2 lets a face-layer control with
> `AppearanceUI_MinControlValue` 0 take the value 0, and `appearance.md` §3.4 starts it at the
> figure's own value, else the dictionary default, which is 0 for several controls of this
> install. As implemented: the choices are the layer's option numbers at or above the minimum
> (1 to *m*); a starting value that is not among them, 0 included, starts at the first choice.
> Offering 0 made the Elf male's face composition exceed the decoded-image budget of
> `tessera-mesh` (`LookLimits`, 64 MiB per compositor, 128 MiB per figure) on this install
> (no face composed); with twice those budgets it composed.
>
> **Answered (owner, 2026-10-16): offer value 0, and raise the budget.** A face-layer control
> whose minimum is 0 offers 0 as a choice (choices 0 … *m*), and a starting value of 0 starts
> there. The `tessera-mesh` `LookLimits` decode budget is doubled to 128 MiB per compositor
> and 256 MiB per figure (`appearance.md` §9 bounds (6)), which the data shows is enough to
> compose the Elf male's face with value 0 on this install; it stays a bounded guard. This
> changes implemented behaviour: `tessera-mesh` `LookLimits` and
> `apps/client/src/screens/controls.rs` are conformed to it.

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
3. **Appearance and name** (§5). Default appearance: the starting values of §5.2.1
   (`appearance.md` §3.4 "Starting values on the creation screen"; BIN, corrected 2026-10-10:
   styles come from the controls' own records, the rest from the avatar's own values); default
   nationality: the first of the list; empty name. Create (`CharGen_Name_Create_Button`) → §7.1; back (`CharGen_Back_Button`) → 2 (the appearance choices are kept while the
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

> **Answered (2026-10-10, BIN, §2.1.4):** the figure keeps stage key 6 while the name is typed
> (the client sets 6 on entering the appearance stage and no other key until the stage is
> left). Correction: the race-and-class figure carries key **1** (`ClassSelect`), not 7; the
> selection figure 4. The note as raised:
>
> **OPEN:** (implementer, `apps/client/src/screens`, `apps/client/src/walker`, 2026-10-10;
> follow-up of the two notes above and below) Item (1) is closed in the client: the
> viewport's figure is the walker's, built through the shared figure path
> (`tessera-mesh` assembly and `assembly::look`) with every creation control applied: styles,
> slider string, the two heights and, as the character's own appearance values, every value
> a set control writes (colours, face layers, the head value), so the species tables compose
> the face and tint hair and skin from them (`appearance.md` §3.5); it is rebuilt on every
> change (the old figure stays drawn until the new one is ready). Figures on these screens
> carry the stage's `CharGen_Stage` key (§2.2: 4 on the selection screen, 7 for the
> race-select entity, 6 on the appearance screen, read from the mapper by label) from the
> start of their scripts and are not placed in a world (`appearance.md` §3.5 Decision). Not
> specified, as implemented: before world entry nothing streams around a viewer, so the
> client loads only the textures of the screens' figure. Question: the stage key of a figure
> shown on the appearance stage while the name is typed (stage 3 `Name` has no layout of its
> own in this build, §2.2): 6 is used; please confirm.

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
the moment it sends until the answer or the time-out below. Names are compared in Unicode
Normalization Form C (NFC): two names are the same when their NFC forms are equal, code point
for code point (Tessera's test server stores and lists the NFC form of a created name,
`test-server.md` §6.4 and §3.5, so a name typed in another form comes back changed; CS16). A
`CharacterInfo` whose name is the same as the requested name is success: the screen returns
to the selection screen, adds the row (if new; with the name as the event carries it) and
selects it. A `CharacterRefused` whose name is the same as the requested name is failure: its
`reason` is shown in `CharGen_ErrorTextList` and the screen stays with every choice kept. A
refusal of the command itself (`NetError::InvalidCommand`, for example an invalid name or
appearance under `scenario-feed.md` §9.5) is shown the same way with Tessera's own text.
With no answer within 10 s an error line is shown, Create is enabled again and the screen
stays. The time-out is the screen's only: the backend may still hold the request (on M7's
server it stays outstanding until the server answers or the session ends, §7.1.1 step 6). An
answer that arrives after the time-out is handled as above (success returns to the selection
screen with the new row selected; a failure's `reason` is shown). A ghosted New button keeps
its tooltip, which says (Tessera's own text) that this server does not allow creating
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
   drops `class` (the protocol carries none: no gameplay reproduction, `server-content.md`
   §2) and records the name as the
   outstanding creation.
3. **`CharacterList` answer** (§6.2.8). The backend clears the outstanding creation. If the
   list holds an entry whose name is the same as the outstanding name in NFC form (both
   names normalised to Unicode NFC and compared code point for code point; the server lists
   the NFC form it stored, `test-server.md` §6.4, §3.5), it emits one
   `CharacterInfo { name: that entry's name, body: Some(that entry's body), class: None,
   level: None }`. If it does not, it emits
   `CharacterRefused { name: the outstanding name, reason }` with the reason text Tessera
   uses for code 6 `SERVER_ERROR`. Other entries of the list emit nothing
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
6. **Outstanding creation and `EnterWorld`.** A creation is outstanding from the moment the
   backend sends `CharacterCreate` until step 3 or step 4 clears it or the session ends. The
   screen's 10 s time-out (§7.1) does not end it. While it is outstanding the backend refuses
   the `EnterWorld` command with `NetError::InvalidCommand` and sends nothing, because
   `EnterWorld` requires that nothing be outstanding (`tessera-protocol.md` §6.2.10) and a
   second outstanding request is fatal (§13.1 "Outstanding requests"); likewise step 2
   refuses a further `CreateCharacter`. The screen shows such a refusal as §7.3 says. (CS17)

> **Answered (2026-10-10, revision (k) and (l) on main, commit 72d3f84, Accepted; merged into
> branch `p1`):** step 3 compares the list entry's name with the outstanding name in NFC form
> and emits `CharacterInfo` with **the entry's name** (the NFC form the server stored), not the
> requested name; §7.1 compares the event's name with the requested name in NFC form, so the
> screen's success rule holds and the row shows the name as the event carries it. The name is
> sent as given (step 2). Step 6: a creation is outstanding from the moment the backend sends
> `CharacterCreate` until step 3 or step 4 clears it or the session ends. Still open from the
> note: the local `SERVER_ERROR` answer to a `CharacterCreate` the network side cannot send
> is not stated in §7.1.1 (spec owner). The note as raised:
>
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

> **OPEN:** (implementer, `apps/client/src/walker/characters`, 2026-10-10; follow-up of the
> note above) The assembler now takes the character's own appearance values
> (`CharacterSpec::appearance_values`), so a received look draws colours and face layers too:
> every entry the receiver rule accepts sets its creation control as on the screens, and
> every set control's written property and value become the figure's own property values,
> set in its script state after its property sets (look colours, R, G, B, A, converted to the
> data's B, G, R, A order). The figure then composes its face and tints hair and skin from
> them like any figure (`appearance.md` §3.5). Not specified, as implemented: an accepted
> entry whose value no control of the receiving install accepts (a face-layer option the
> compositor lacks, a value outside a float range) is dropped with its control, so the
> property keeps the avatar's own value; please confirm, or say whether the value should
> be set as received.

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

Enter sends `EnterWorld` with the selected name; the screens close on `WorldEntered`. If the
backend refuses the command with `NetError::InvalidCommand` (on M7's server while a creation
is still outstanding, §7.1.1 step 6), the selection screen stays, adds an error line to
`CharGen_ErrorTextList` (Tessera's own text, saying that a character creation is still
waiting for the server's answer) and keeps Enter enabled, so the player can try again once
the answer has arrived (**Tessera rule**, CS17).

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
5. Against Tessera's test server run in-process (`test-server.md`) with a fresh data
   directory and one fresh account (no characters), `character_creation` true and
   `max_characters` (`test-server.md` §4.3, default 8, range 1–64) at least the number of
   races of step 1 (8 here; the count is read from the install at run time, not hard-coded),
   so that no creation of this step fails with `TOO_MANY` (CS18), the screen's
   `CreateCharacter` for one (race, gender) per race yields a
   `CharacterInfo` with that name and the command's body (§7.1.1 step 3), and `EnterWorld`
   with it succeeds; a second creation with the same name yields `CharacterRefused` (code 1
   `NAME_TAKEN`: the server checks the name before the character count, `test-server.md`
   §6.4, so this holds even when the account is full);
   with `character_creation` false the New button is ghosted and nothing is sent. The look
   seen by other clients is limited by the §7.1 OPEN (Q7; answered by decision 0025 D2).

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
| CSI6 | Every race table's `RaceTable_UIScene` holds exactly one Physics element whose object's setup has the holding location `Holding_Loc_Entity_00` and at least one element named `chargen_Camera`, and every camera key of the race table and of its gender entries names an element of that scene (§2.1.1–§2.1.3) | Confirmed 2026-10-10 (8 scenes, CS19) |
| CSI7 | Every appearance control of every avatar entity has an `AppearanceUI_ControlStringName` (§5.2.2) | Confirmed 2026-10-10 (221 of 221, CS23) |
| CSI8 | Every style control offered at creation names an entry for its `AppearanceUI_APRKey` in its `AppearanceUI_APRFile`, so §5.2.1 gives it a first block (§5.2.1) | Confirmed 2026-10-10 (37 of 37; every first block has `variant` 0.00, CS26) |
| CSI9 | Every gender entry's avatar closure defines the script key `Chargen`; a figure of each body that runs `Startup`, its start-up names and `Chargen` (§2.1.4), with `CharGen_Stage` 1 and any class key of its `Agent_Class` cases, with 6 and with 4, keeps a looping clip on a layer below 110 with the body's track count that has no track weights or plays with `UseBoneOpacity` 0 | Confirmed 2026-10-10 (15 of 15 closures; 189 of 189 runs, CS29, CS30) |
| CSI10 | `chargen_appearance` has a top-level element `CharGen_AppearanceUIGroupChoice_BaseField` whose base element is of type `AppearanceUIGroup` and holds `CharGen_AppearanceUI_Controls_Listbox` (§5.2.2) | Confirmed 2026-10-10 (CS33) |

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
| Q12 | How the viewport applies shadows, bloom, fake shadow colour, deshading and rim lighting; ambient of the viewport against the scene's lights | §2.1.1 | `lighting.md` / `render.md` writers (further BIN); owner check O85 |
| Q13 | What `RaceTable_RaceSelect_Entity` is used for | §2.1.4 | further BIN |
| Q14 | Blocks of equal `variant` as one style choice; the effect of `AppearanceUI_EndOldOptionsControlValue` and `AppearanceUI_OldOptionsWeightValue` | §5.2 | further BIN |
| Q15 | What a control's (and a group's) lock changes; the camera's field-of-view axis (the wheel direction is answered, CS31) | §5.2.2, §2.3, §2.1.3 | further BIN; owner check O85 |
| Q16 | Whether the pieces of a stopped Appearance node and the props of a stopped MeshFX node are taken off (class or stage change on the creation figure) | §2.1.4 | further BIN (`appearance.md` §7.4 owner); owner check O85 |
| Q17 | What the alpha byte of a scene element's own `Render_Color` does; the client's path from `Render_Color` and `Render_Opacity` to the per-object tint for scene objects | §2.1.1 | further BIN (`material.md` §8.3, `environment.md` §11.4) |

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (Review), for decision 0024 P1: screens and stages, selection row, race/gender/class lists and their records, nationality, appearance controls, name, create, flow, commands and answers per backend, scenario-feed driving, real-data test, open design gaps (creation choices in `Appearance`, deletion in the feed, plain-sandbox creation). |
| 2026-10-09 | Revision after the independent review of 2026-10-09 (changes requested); status stays Review. Blocking (1): new §2.3 names the shared controls of `chargen` under `ControlsParent` (back, exit, continue, the turntable group, the avatar-update toggle) with boxes, base elements and button data, and gives the P1 rule per stage, Exit hidden, the ghosted Continue, the turntable and the toggle; §2.1 and §5.2 point there. Blocking (2): §5.3 states that `CharGen_Name_Entry_Field` is an element with a base reference to the `Text` element `ChargenNameTextEntry` of layout 0x22000002, overridden to editable, selectable and one line. Blocking (3): §7.2 gives the delete rule per backend (scenario feed enabled; plain sandbox and M7 server ghosted, never send), the handling of an `InvalidCommand` refusal and a new protocol OPEN; §3 delete row follows. Non-blocking (a) §7.1 cites `server-content.md` §2; (b) §2.2 corrected (side panels anchor by their own edge modes; per-screen table); (c) provenance row CS11 for the description format; (d) P1 rules for the admin and debug name elements (§5.3), the locked-continue store button and the loading blockers (new §4.3), the nationality text (§5.2), the choose-available-characters dialog (§3). Provenance rows CS8–CS13 added; §11 Q9, Q10; owner check O85 extended. (e) unchanged: `scenario-feed.md` links resolve after branch `p0` is merged. |
| 2026-10-09 | Revision after the independent re-review of 2026-10-09 (changes requested); status stays Review. Blocking R2-1, answered by option (i): §7.1 now states that the screens issue `CreateCharacter` on every backend and see only `CharacterInfo` / `CharacterRefused`, with the New button rule per backend and the Create, refusal and time-out rules; new §7.1.1 gives the `remote` backend's rule (capability `CHARACTER_CREATION` from `HelloAck`, acceptance conditions, `CharacterCreate` with the class dropped, `CharacterList` → `CharacterInfo`, `CharacterCreateFailed` → `CharacterRefused` with Tessera's text per §14.4 code, states) as the rule that narrows `scenario-feed.md` §9.5, with an OPEN for the matching revision of that section and of `tessera-protocol.md` Appendix A; provenance row CS14; §3 New row, §9 step 5 (M7 part of P1 criterion 1) and §11 Q11 follow. Non-blocking: (f) `CharGen_ContinueLocked_BuyNowButton` added to CS12 and §4.3; (g) §2.2 selection row: `CharGen_Notice_FinishingLogout` is a direct child of the root; (h) `ECommBrowser` (§3) and `GamePlay_ExaminationTooltip_Activatable` (§2.1) never created in P1; (i) the **Probable** mark and the **Spec** source defined in the §1 key; (j) §4.3 loading blockers named as based on `Chargen_Silver_Border`. Provenance row CS15 (OBS, re-checked with the value-decoding probe 2026-10-09). (e) unchanged. |
| 2026-10-10 | Revision (spec writer) after the second re-review's non-blocking notes; status Accepted → Revised, awaiting review. (k) §7.1: names are compared in NFC form (Tessera's test server stores and lists the NFC form, `test-server.md` §6.4), for success and for failure; §7.1.1 step 3 matches the list entry by NFC form and emits `CharacterInfo` with the entry's name. (l) New §7.1.1 step 6: a creation stays outstanding after the screen's 10 s time-out until the server answers or the session ends, and the `remote` backend refuses `EnterWorld` (and a further `CreateCharacter`, step 2) with `NetError::InvalidCommand` meanwhile (`tessera-protocol.md` §6.2.10, §13.1); §7.1 says the time-out is the screen's only and how a late answer is handled; §7.3 gives the selection screen's handling of the refusal (error line, stays, Enter kept enabled). (m) §9 step 5 runs on a fresh data directory and a fresh account with `max_characters` at least the race count read at run time, and states why the duplicate creation still yields code 1. (e) settled: branch `p0` is merged (a1fb29f) and `scenario-feed.md` is on main; its cited sections (§3.3, §5.1, §6.1, §7.5, §7.7, §9.2, §9.4, §9.5) are unchanged from the reviewed text (later commits add only OPEN callouts in §11.1, §12.4 and §13); the companion row, CS13 and CS14 now cite main. Provenance rows CS16–CS18. Q11 stays open (`scenario-feed.md` §9.5 still refuses `CreateCharacter` on the test-server backend). |
| 2026-10-10 | Pointers only (no fact changed): the OPENs of §7.1 (look), §7.1.1 (Q11), §7.2 (feed removal) and §8 (plain-sandbox creation) and rows Q7, Q8, Q11 point to their answers in decision 0025 (`scenario-feed.md` revision 4, `tessera-protocol.md` v1.1). The §7.2 protocol-deletion OPEN (Q9) stays open. Merged with main's revision (k)–(m) above (branch `p1`, 2026-10-10): main's "Q11 stays open" is superseded by decision 0025 D4 (`scenario-feed.md` revision 4 revises §9.5); the §7.1.1 implementer OPEN on name comparison is marked answered by (k) and (l), except the local `SERVER_ERROR` for an unsendable command; §9 step 5 points Q7 to decision 0025 D2. Status: main's acceptance covers the text reviewed at 72d3f84; P1's pointer rows and implementer OPENs await the P1 review. |
| 2026-10-10 | Revision (spec writer; status Accepted → Revised, awaiting review) for the defects seen on the P1 creation screen. New provenance keys OBS2, WLB, BIN and rows CS19–CS27. §2.1: the viewport's scene and camera value per stage; new §2.1.1 (what a race scene holds; Tessera presentation rule: all Physics, Light and Decal elements drawn, the figure lit by the scene's lights, viewport ambient; OPEN on the other viewport values), §2.1.2 (the figure stands at the scene's `Holding_Loc_Entity_00` marker with its rotation; table of the 8 scenes), §2.1.3 (the `chargen_Camera` element moved between the base and zoom camera frames; field of view 65 / 40; turn about the figure), §2.1.4 (stage figures: selection key 4 with idle/hover action; race-and-class figure = the avatar entity from slot 14 of the gender entry, key 1, `Agent_Class`; appearance stage key 6); the implementer OPEN of §2.1 marked answered. §2.3: the turntable moves the camera (20° per hot click, drag −0.5° per pixel, half-life 0.15 s), zoom buttons with a timed eased move, wheel steps 0.15 with half-life 0.1 s, ghosting; the former Tessera turn rule (90° per second, 15° tap) withdrawn; the §2.3 OPEN narrowed. §4.1: gender buttons row and the viewport row corrected (`RaceTable_RaceSelect_Entity` is not the figure); new gender-group rule (selected button highlighted by the GroupBox, default male, group hidden with male selected for one-gender races, sex kept across races). §5.2: list order (ungrouped controls, then groups by ascending key), part names; new §5.2.1 starting values (with `appearance.md` §3.4) and §5.2.2 labels (each control's own `AppearanceUI_ControlStringName`; the colour template's layout text is a placeholder), current-choice text (token and arguments), group headers (display text of the group key), lock buttons; implementer OPENs on group order, gender after a race change, the head value and element labels answered or narrowed. §6: default appearance per §5.2.1; stage keys answered (6 while the name is typed; race-and-class 1, not 7). §10 CSI6–CSI8; §11 Q12–Q15. No section number or anchor changed; new anchors `s2-1-1` to `s2-1-4`, `s5-2-1`, `s5-2-2`. |
| 2026-10-10 | Revision (spec writer; status Accepted → Revised, awaiting review) answering the implementer OPENs of commit dc06296. New provenance keys OBS3 and BIN2, rows CS28–CS34. §2.1.4: the screens add the start-up script `Chargen` to the selection and the race-and-class figure (and `Render_AlwaysSelectable` to the latter); what `Chargen` plays per stage key and class (looping whole-skeleton stance, class costume through Appearance and MeshFX nodes, gestures); stance census 189 of 189 with the script, 0 of 159 stage-1 runs without it; one-shot stage clips play once and are removed (`script-table.md` §6.10); the figure stands on the marker and the cameras ignore its size; new OPEN on removing the pieces of stopped Appearance and MeshFX nodes (Tessera rule: removed); the implementer OPEN marked answered. §2.1.1: scene objects apply `Render_Color` and `Render_Opacity` as the per-object tint (Tessera rule for the mapping), new OPEN on the colour's alpha byte. §2.3: wheel direction (`ScrollUp` zooms in), hover and click by the view's pick (Tessera rule for the pick), wheel and drag during a zoom move, field-of-view axis; the Zoom item states the wheel direction; the implementer OPEN marked answered. §5.2.2: the group template `CharGen_AppearanceUIGroupChoice_BaseField` of `chargen_appearance` and its parts; a group is one row holding its controls; Tessera rules for collapse and the group lock; the implementer OPEN marked answered. §10 CSI9, CSI10; §11 Q15 narrowed, Q16, Q17. No section number or anchor changed. |
| 2026-10-16 | Note only (no fact changed), decision 0040: the §2.2 implementer OPEN points to `ui-scaling.md` §5.3–§5.4 and §10.2. Status unchanged. |
| 2026-10-23 | Spec writer (dirty room); status Accepted → Revised, awaiting independent review. §2.1.4: the implementer's build-status note (question on the start-up `RampIn` rule) and the OPEN on where a MeshFX node attaches its entity answered by pointers to `script-table.md` §6.10 step 1 (T48) and §5.2 (T47), revised the same day from a new BIN pass. No fact of this spec changed. |
