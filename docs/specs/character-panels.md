<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Character sheet, bags and equipment: panels, slots, item displays, worn look

| | |
|---|---|
| **Status** | Accepted (2026-10-09: re-review of the revision answering the first-version review passed; implementers may build from it) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Independent spec-review session (Claude Code), which wrote no part of this spec |
| **Review record** | **Re-review of the revision of 2026-10-09 (commit 9bced79), 2026-10-09, independent review session that wrote no part of this spec: passed, Accepted.** Whole spec re-checked against CLEAN_ROOM.md §5.3 (the file is new relative to `main`): every statement is a layout, value, constraint, behaviour or a marked **Tessera rule**; no code or pseudo-code (the `EquipItem { … }` forms are the feed's message notation, `scenario-feed.md` §9.4); element, state, property, mapper and input-action labels and API names are interoperability identifiers; no game text (tooltip and title texts referenced by StringInfo only), assets, keys, hostnames or traffic; each fact has a provenance row (CP1–CP11) with a mark and, for Confirmed, a date; invariants CPI1–CPI5 are testable; every unknown is an OPEN with a Tessera rule and is listed in §11 (Q1–Q8) with an owner check (O82–O84 present in `owner-observations.md`); no closed gate touched. Blocking items of the first review: (1) fixed: §4.3 is one numbered rule (bags 1–5 by range, bag 6 shows 75 … *N* − 1, *N* grows only before the first `ContainerSize` and never shrinks without one, growth fires `SizeChanged` before the item's `ItemAdded`), and it agrees with `lua-plugin-api.md` §15.1 (`Backpack.SizeChanged` row) and with `scenario-feed.md` §5.5 on branch `p0` (an out-of-size `give` is skipped once a size is set, so growth after the first `ContainerSize` cannot occur); (2) fixed: §8 keeps only the API facts and defers event derivation and getters to `lua-plugin-api.md` §15.1, whose `Backpack` events row includes moves between containers and whose `Equipment` events row covers keys 2 … 22; (3) fixed: `QUICKSLOT_BACKPACK` is an OPEN with a Tessera rule, Q8, O83. Non-blocking (a)–(d) and (f) fixed as the change log says. Spot-checks (read-only, 2026-10-09, fresh extraction with the release `tessera-dattool` and a throwaway probe): DID map 0x28000003 (UILAYOUT, `records.md` §8) holds key 0x10000021 with label `journal` and target 0x2200003B (CP11, §2); the input-action mapper 0x230000AF names `QUICKSLOT_BACKPACK` and `QUICKSLOT_BACKPACK1` … `6` with distinct keys, layout 0x22000074 references each numbered key exactly once and the un-numbered key not at all (§4.1, CPI4, the §4.1 OPEN premise); mapper 0x230000EE keys 2 … 25 carry the slot names of §3.3 and key 0x1E is `Equipment` (§6 default-slot rule); an independently decoded dump of `InventoryControl` (0x70000230) shows `InventoryControl_SmartSortInfo_Array` with 8 entries of a sort type and a property-ID list, and `InventoryControl_MutuallyExclusiveLocation_MappingList` with exactly two pairs, right hand with right off-hand and left hand with left off-hand (CP11, §4.4, §6). All agree. **Non-blocking notes** (for a later revision; none blocks implementation): (a) §4.3: say explicitly that items left at slots ≥ *N* after a shrinking `ContainerSize` (the feed keeps them, `scenario-feed.md` §5.5) are shown by no bag and are not visible to plugins; (b) §4.3 rule 3 lets bag 6 hold up to 65,461 slots before the first `ContainerSize` (the feed's default size is 65,536); a display bound or a scroll requirement would help implementers; (c) `lua-plugin-api.md` §15.1 still speaks of the "highest occupied slot + 1 term" of §4.3, which §4.3 now words as growth on arrival; same effect, align the wording at the next revision of that spec; (d) §11 lists Q8 between Q5 and Q6; (e) the Status of a never-Accepted spec read "Revised"; Review would have been the §5.2 state. Earlier: **Review of the first version, 2026-10-09: changes requested.** Checked against CLEAN_ROOM.md §5.3: every statement is a layout, value, constraint or behaviour; no code or pseudo-code; element, state, property and mapper labels and the API names are interoperability identifiers taken from the data and the API; the empty-slot tooltip texts and titles are referenced by StringInfo only (no game text); the class of the wrath row is given by key only; no assets, keys, hostnames or traffic; no closed gate touched (no M4 method, no official-server traffic). Spot-checks (read-only, 2026-10-09; layout records 0x2200084B, 0x2200003B, 0x22000074, 0x22000051 and 0x22000849 re-extracted from the install and byte-identical to the 2026-10-06 dump): `Journal_Window` widget type, scaling group 29, origin (357, 200), rect, auto-fade 0.1, object mode, the two pages and tabs (§2); the 23 slot elements, their one-bit `Container_Slot` values, the 23-entry `UI_Inventory_EquipmentSlotList`, the 24 parent positions (44 × 44) and the slotless `EssenceControlSlotParent` (§3.3, CPI1, CPI2); mapper 0x230000EE keys 2–25 and their names (bit *b* ↔ key *b* + 1); the three `CharacterEquipmentPage` variants and the paperdoll part properties (§3.1, §3.2); stat-row type values, the three state colours and the power/wrath `HideWhenField` pair on key 214 (§3.4); the six bags' input actions and column settings (§4.1, CPI4); the five `InventoryControl` counts 45, 15, 6, 5, 21 (§4.2, from a decoded dump of 0x70000230); `ItemDisplay_SlottedTemplate` 35 × 35 with tooltip layout 0x22000003 and `ItemDisplay_SlottedIcon` 32 × 32 at (3, 3) (§5); API `Equipment` values 1–21 against `lua-plugin-api.md` Appendix A (§8.2, CPI3). All agree. **Blocking:** (1) §4.3 Tessera rule is self-contradictory at its upper limit: the range rule gives bag 6 the slots 75 … min(90, *N*) − 1, then "slots beyond 90 are shown by bag 6" overrides it without saying so, and "beyond 90" is ambiguous for 0-based slot indices (is slot 90 included?). State one rule (for example: bags 1–5 by the range rule, bag 6 shows 75 … *N* − 1). In the same rule, *N* grows before the first `ContainerSize` when an item arrives at a high slot; say whether that growth fires the plugin `SizeChanged` (§8.1 fires it only on `ContainerSize`). (2) §8 contradicts `lua-plugin-api.md` §15.1 on the plugin events, which both specs define for `crates/plugins`: §15.1 fires `ItemAdded` for an `ItemMoved` from another container into the backpack, §8.1 does not; §8.2 gives no derivation of `ItemEquipped`/`ItemUnequipped` from the feed (§15.1: an item entering or leaving `Equipment` at key 2 … 22, nothing for keys 23 … 25). Make the two specs say the same thing, or reduce §8 to a pointer to §15.1 so only one spec owns the rule. (3) §4.1 states "`QUICKSLOT_BACKPACK` (no number) toggles all bags (reading of the name)": an inference from a label, presented as a fact with no OPEN. Make it an OPEN with a **Tessera rule**, list it in §11 and point it at an owner check (O83 fits). **Non-blocking:** (a) CP2–CP7 and CPI2–CPI4 say "Confirmed" without a date (§5.1 asks for one; CP1 and CPI1 have it); (b) the §2 rect "(183, 98, 713, 648)" reads like two corners; write it as position (183, 98), size 713 × 648 as §3.3 does; (c) §6 split rule: say it applies only to a stack of 2 or more (half of 1 rounds to 0); (d) §5 unusable overlay: name the class property (`Usage_RequiredClassList`, `items.md`) instead of "its required classes"; (e) `scenario-feed.md` is Accepted only on branch `p0`, so its links dangle on this branch until `p0` is merged (the spec says so; no change needed); (f) the facts of §2 "Opening" (the `UILAYOUT` key), §4.4 (`SmartSortInfo`) and §6 (the two mutually exclusive pairs) have no own provenance row; add them to CP1, CP6 or a new row. |
| **Implements into** | `apps/client` (the character panel and the six bag windows of decision 0024 P1, their commands to the session), `crates/ui` (fallback widgets of the game widget types named here), `crates/gamedata` (inventory model over the scenario feed's events, slot ↔ key mapping), `crates/plugins` (`Turbine.Gameplay` `Backpack` and `Equipment`, with `lua-plugin-api.md` §15) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (layouts, §17.2 P13 and P15 rows), [`ui-behaviour.md`](ui-behaviour.md) (§7 drag and drop, §8 window manager), [`ui-widgets.md`](ui-widgets.md) and [`game-panels.md`](game-panels.md) (`PropertyDisplay`, `HideWhenField`, `UIScaler`), [`items.md`](items.md) (slots, stack sizes, `InventoryControl`), [`appearance.md`](appearance.md) (worn look, §4, §8), [`item-tooltips.md`](item-tooltips.md), [`icon-layers.md`](icon-layers.md), [`player-character.md`](player-character.md) (avatar slot counts §4.4, class data §7), [`scenario-feed.md`](scenario-feed.md) (Accepted on branch `p0`, 2026-10-09: containers, items, equip commands, §5.5, §6.1, §7.6, §9), [`lua-plugin-api.md`](lua-plugin-api.md) §15, [`keymap.md`](keymap.md) (commands) |

---

<a id="s0"></a>
## 0. Purpose

Decision 0024 P1 needs the original's character sheet (the character panel's equipment page
with the paperdoll, the header and the stat list), the six bag windows, the item slots they
show, and the worn look that follows equipment. This spec gives, from the install's layout
data, the panels' structure and slot tables; the behaviour that is game code and was not
traced is marked OPEN with a Tessera rule; and it says how the scenario feed drives each part.

All values (DIDs, element and state labels, positions, counts) are observations of one
install; implementations read them at run time.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (client build 4906.0071.1794.4035), read-only, archives unchanged since 2026-10-04 (re-checked 2026-10-09). All 302 UI layout records (dump of 2026-10-06, decoded per `ui-layout.md` §4, re-read with a throwaway value-decoding probe 2026-10-09): layouts `journal` (0x2200003B), `characterequipmentpage` (0x2200084B), `characterappearancepage` (0x2200084A), `stattree` (0x22000849), `containerdisplay` (0x22000074), `inventory` (0x22000036), `itemdisplay` (0x22000051), `moneydisplay` (0x2200006C); the mappers `UIElementID`, `ContainerSlot` (0x230000EE), the input-action mapper (0x230000AF); the `InventoryControl` record (0x70000230) and the race avatar templates (`player-character.md` §4.4). |
| **S2** | The official Lua API reference (beta documentation of `Turbine.Gameplay` `Backpack`, `Equipment`, `LocalPlayer`, as kept in the research clone `lunarwtr_lotro-api-docs`, U25 pages), read for facts only (`lua-plugin-api.md` key S2). |
| **Design** | Tessera rules, marked **Tessera rule**. |

| # | Fact | Source | Mark |
|---|---|---|---|
| CP1 | The character panel is `Journal_Window` of layout `journal`: a scaled window with two tab pages, equipment (open) and appearance (§2) | OBS | Confirmed 2026-10-09 |
| CP2 | The equipment page: header (level, class icon, name, title button, race-and-class line), a paperdoll viewport with rotate buttons, 23 equipment slots, a stat list, a row of buttons (§3) | OBS | Confirmed 2026-10-09 |
| CP3 | Slot table: element, `Container_Slot` bit, position, empty-slot tooltip (§3.3) | OBS | Confirmed 2026-10-09 |
| CP4 | Stat rows: display type, value properties, states `normal`, `buffed`, `debuffed` with colours (§3.4) | OBS | Confirmed 2026-10-09 |
| CP5 | Six bag windows `PlayerBag1_Template` … `PlayerBag6_Template`, each with an item list of 5 columns by default and a per-bag column setting; each names its toggle command (§4) | OBS | Confirmed 2026-10-09 |
| CP6 | `InventoryControl`: base backpack slots 45, slots per bag 15, at most 6 bags, slots per pouch 5, 21 equipment slots; avatar templates: 75 backpack, 21 equipment, 20 overflow slots (§4.2) | OBS | Confirmed 2026-10-09 |
| CP7 | Item-slot display: icon, quantity text, and overlays by state (§5) | OBS | Confirmed 2026-10-09 (layout); conditions OPEN |
| CP8 | `Backpack:GetItem(index)` takes 1 … `GetSize()`; `Equipment:GetItem(slot)` takes the API slot enumeration; events `ItemAdded`, `ItemRemoved`, `ItemMoved`, `SizeChanged`, `ItemEquipped`, `ItemUnequipped` (§8) | S2 | Reported |
| CP9 | The API's `Equipment` values equal the data's `ContainerSlot` key minus one for keys 2–22 (`Head` … `Mounted`/`Bridle`) (§8.2) | S2 (API enum, `lua-plugin-api.md` Appendix A); OBS (mapper names) | Reported (API values) + Confirmed (data keys) |
| CP10 | Interactions, slot-to-bag mapping, the all-bags command, overlay conditions, stat values (§6, §4.3, §4.1, §5, §3.4) | Design | Decision |
| CP11 | The `UILAYOUT` map (0x28000003 here) holds key 0x10000021, label `journal`, target 0x2200003B (§2 Opening); `InventoryControl_SmartSortInfo_Array` has 8 entries, each a sort type and a list of property IDs (§4.4, `items.md` §6.4); `InventoryControl_MutuallyExclusiveLocation_MappingList` has two pairs of holding locations: right hand with right off-hand, left hand with left off-hand (§6) | OBS (DID map 0x28000003 re-read 2026-10-09 per `records.md` §8; decoded dump of 0x70000230 of 2026-10-07) | Confirmed 2026-10-09 |

---

<a id="s2"></a>
## 2. The character panel (Confirmed, CP1)

| Part | Data |
|---|---|
| Root | `Journal_Window`, widget type `UIScaler`, scaling group `Character` (29), origin (357, 200), design rect at position (183, 98), size 713 × 648; saves its location; activatable; auto-fade 0.1; object mode Stretch (`ui-layout.md` §9.9, `ui-behaviour.md` §9.2, §11.1) |
| Chrome | background `Box_Silver`, title bar with a title text (StringInfo of table 0x25000023), a drag bar with a tooltip, a close button |
| `Journal_Panel` | widget type `Panel`, pages (in order): `Equipment_Page` (open at start, tab `Equipment_Tab`) and `Appearance_Page` (tab `Appearance_Tab`); each page is based on the page element of its own layout (`CharacterEquipmentPage_Page`, `CharacterAppearancePage_Page`) |

Opening (CP11): `UILAYOUT` entry `journal` (key 0x10000021 → 0x2200003B here), root `Journal_Window`.

> **OPEN:** which input command toggles the character panel (by its name, `ToggleJournalPanel`
> of the input-action mapper; the binding comes from the key map, `keymap.md`). **Tessera
> rule:** `ToggleJournalPanel` toggles `Journal_Window`.

---

<a id="s3"></a>
## 3. Equipment page (Confirmed, CP2)

<a id="s3-1"></a>
### 3.1 Variants

`RealPaperdollField` has three children of widget type `CharacterEquipmentPage`:
`RealPaperdollField_Player` (free peoples), `RealPaperdollField_MonsterPlayer` and
`RealPaperdollField_SessionPlayer`. **Tessera rule:** P1 shows the player variant only.

<a id="s3-2"></a>
### 3.2 Header and buttons (player variant)

| Element | Widget | Shows |
|---|---|---|
| `PlayerLevel` | `PropertyDisplay`, `UI_PropertyDisplay_UsePlayerAsTarget` true | `Advancement_Level` of the local character (`game-panels.md` property displays) |
| `PlayerName` | `PropertyDisplay` | `Name` of the local character; truncated with a tooltip when too long (`ui-behaviour.md` §10.2) |
| `PlayerClassIcon` | based on `ClassDisplay_50Template_WithTooltip` (layout 0x22000193) | the class icon |
| `PlayerTitleButton` | `Button`, click action `TITLE_VIEW` | the active title |
| `PlayerRaceClassDisplay` | `Text` | a one-line race-and-class text |
| `Player_Skills_Button`, `_Title_`, `_Bio_`, `_Wallet_`, `_House_`, `_Hobby_`, `_Reputation_`, `_War_` | `Button` | open other panels (P2–P6) |
| `CharacterEquipmentPage_Player_EnhanceCharacter_Button` and its flyout | store | **Tessera rule:** hidden |

The paperdoll (`CharacterEquipmentPage_AvatarInfoElement`, widget `AvatarInfo`) names its
parts by properties: `UIComponent_AvatarInfo_Viewport` (`CharacterEquipmentPage_Viewport`, a
`TargetViewport` of viewport type `Paperdoll`, UI scene 0x21000005, camera `Camera_1`),
`UIComponent_AvatarInfo_LeftButton` / `_RightButton` (rotate buttons; hot buttons repeating
every 0.1 s while held), `UIComponent_AvatarInfo_SlotField` (§3.3) and
`UIComponent_AvatarInfo_EnableMouseDragRotate` true (dragging in the viewport turns the figure).

> **OPEN:** the rotation step per repeat and per pixel of drag, and the figure's pose. **Tessera
> rule:** 15° per repeat, 0.5° per pixel, the idle pose (`appearance.md`, `script-table.md`).

> **OPEN:** (implementer, `apps/client/src/character/panel.rs`, 2026-10-10) Three details of
> the header and the paperdoll that this section does not give. (1) The format of
> `PlayerRaceClassDisplay`: as implemented, the race's and the class's display texts (enum
> texts of `Usage_RequiredRaceEntry` and `Usage_ClassType`) joined by one space; an unknown
> value shows an empty text, not the layout's placeholder. (2) `PlayerName` and `PlayerLevel`
> are `PropertyDisplay` elements whose fallback widget draws no text: the client creates that
> type as a text element when the panel opens and sets the text itself (name of the entered
> character, level of the latest `LevelChanged` or `CharacterInfo`). (3) The paperdoll's
> figure in `CharacterEquipmentPage_Viewport`: as implemented, the player's walker figure
> (worn look of the latest `AppearanceChanged`, current pose) drawn alone, unlit with a fixed
> light, at the origin turned by the paperdoll's yaw, seen from its front by a camera of
> Tessera's own (vertical field of view 30°, the figure's bounding sphere filling the
> height); the UI scene 0x21000005 and its camera `Camera_1` are not read. Owner check O82.

> **OPEN:** (implementer, `apps/client/src/character/panel.rs`, 2026-10-10, after review)
> (4) `PlayerClassIcon` (based on `ClassDisplay_50Template_WithTooltip`): §9 lists the class
> icon as a header effect of `CharacterInfo`, but neither this spec nor `item-tooltips.md`
> names where a class's icon image comes from (a property of the class table, or of the
> template's own elements). As implemented: the element keeps the layout's content (no
> class icon is set). Please name the image source.

<a id="s3-3"></a>
### 3.3 Equipment slots (Confirmed, CP3)

`CharacterEquipmentPage_SlotField` (at (4, 72), 440 × 453) lists its slots in
`UI_Inventory_EquipmentSlotList` and holds one parent field (44 × 44) per slot, each with a
slot element (35 × 35 at (3, 3), based on `SlotTemplate` → `ItemDisplay_SlottedTemplate` of
`itemdisplay`, §5). Each slot element has `Container_Slot` (a one-bit `ContainerSlot` value; bit
*b* ↔ key *b* + 1, `property-system.md` §7.3) and `UI_Item_DisplayEmptyOverride`, the text of
its tooltip while empty (a StringInfo of table 0x250001BB; not reproduced). Positions are the
parent fields' within the slot field:

| Slot element | `Container_Slot` | Key and mapper name | Parent position |
|---|---|---|---|
| `Earring1Slot` | 0x2000 | 14 `Earring1` | (13, 10) |
| `Earring2Slot` | 0x4000 | 15 `Earring2` | (13, 55) |
| `NecklaceSlot` | 0x400 | 11 `Necklace` | (13, 100) |
| `Bracelet1Slot` | 0x100 | 9 `Bracelet1` | (13, 145) |
| `Bracelet2Slot` | 0x200 | 10 `Bracelet2` | (13, 190) |
| `Ring1Slot` | 0x800 | 12 `Ring1` | (13, 235) |
| `Ring2Slot` | 0x1000 | 13 `Ring2` | (13, 280) |
| `Pocket1Slot` | 0x8000 | 16 `Pocket1` | (13, 325) |
| `HeadSlot` | 0x2 | 2 `Head` | (383, 10) |
| `ShoulderSlot` | 0x40 | 7 `Shoulder` | (383, 55) |
| `BackSlot` | 0x80 | 8 `Back` | (383, 100) |
| `ChestSlot` | 0x4 | 3 `Chest` | (383, 145) |
| `GlovesSlot` | 0x10 | 5 `Gloves` | (383, 190) |
| `LegsSlot` | 0x8 | 4 `Legs` | (383, 235) |
| `BootsSlot` | 0x20 | 6 `Boots` | (383, 280) |
| `Weapon1Slot` | 0x10000 | 17 `Weapon_Primary` | (30, 405) |
| `Weapon1FXSlot` | 0x400000 | 23 `Weapon_Primary_FX` | (71, 405) |
| `Weapon2Slot` | 0x20000 | 18 `Weapon_Secondary` | (122, 405) |
| `Weapon2FXSlot` | 0x800000 | 24 `Weapon_Secondary_FX` | (163, 405) |
| `WeaponRangedSlot` | 0x40000 | 19 `Weapon_Ranged` | (215, 405) |
| `WeaponRangedFXSlot` | 0x1000000 | 25 `Weapon_Ranged_FX` | (256, 405) |
| `CraftToolSlot` | 0x80000 | 20 `CraftTool` | (311, 405) |
| `ClassSlot` | 0x100000 | 21 `Class` | (361, 405) |

A 24th parent, `EssenceControlSlotParent` at (383, 325), has no slot (a tooltip-only field).
The FX slots hold cosmetic weapon effects; the `Mounted` slot (key 22) is not on this page.

<a id="s3-4"></a>
### 3.4 Stat list (Confirmed, CP4)

`CharacterStats` is based on `StatTreeUIElement` of `stattree` (widget `StatTreeDisplay`: a
`StatTree` list with a scroll bar and a class-filter check box). Its row elements:

| Row kind (template) | Widget | Value source in the data | States (colour here) |
|---|---|---|---|
| Vitals (`Stats_VitalTemplate`): morale, power; mount endurance and power | `VitalDisplay` | `UIComponent_VitalDisplay_Type` (`Health` 1, `Power` 2, `MountEndurance` 3, `MountPower` 4), a description StringInfo | `normal` 0xFFB8B9B8, `buffed` 0xFF22B922, `debuffed` 0xFFB92222 |
| Main stats (`Stats_StatTemplate`): might, agility, vitality, will, fate; mount strength, mount agility | `StatDisplay` | `UIComponent_StatDisplay_Type` (`Vitality` 1, `Agility` 2, `Fate` 3, `Might` 4, `Will` 5, `MountAgility` 6, `MountStrength` 7) | as above |
| Armour (`Stats_ArmorClassText`) | `ArmorClassDisplay` | `Combat_ArmorDefense_CurrentPoints_Common` | 3 states |
| Rating rows (critical, finesse, physical and tactical mastery, block, evade, parry, outgoing and incoming healing, critical defence, mitigations, resistances, light) | templates of `stattree` | `UIComponent_CombatPointsDisplay_StatProperty`, `…_CacheProperty`, `…_ClassProperty`, `UIComponent_CombatPercentDisplay_CacheProperty` (actor property labels, for example `Combat_Stats_CriticalPoints_Unified`) | as above |
| Headers | `Field` with a label | — | — |

Each value row names its label element (`UIComponent_StatDisplay_LabelElement`), whose
tooltip is an examination tooltip of layout `examination_stat` (0x220008C2) through the
override properties of `item-tooltips.md` §3.3. The power row is hidden for the class whose
key is 214 and a wrath row shown instead (`HideWhenField`, `game-panels.md` §6).

> **OPEN:** how the original computes the shown values and chooses `buffed` / `debuffed`
> (server properties and the class library's stat code). **Tessera rule:** each row shows the
> value the session last reported for its property (§9) as an integer (percent rows with one
> decimal and a percent sign), in state `normal`; a row without a reported value shows an
> empty value text.

> **OPEN:** (implementer, `apps/client/src/character/panel.rs`, 2026-10-10) The rule above
> needs, per row, which property it shows and which element shows the value. As implemented:
> a row element with `UIComponent_VitalDisplay_Type` 1 or 2 shows "current / max" of the
> reported morale or power (`VitalChanged`); one with
> `UIComponent_CombatPointsDisplay_StatProperty` (integer) or
> `UIComponent_CombatPercentDisplay_CacheProperty` (percent) shows the player's last
> `PropertyChanged` value of the property that names; the text goes on the row element
> itself. `StatDisplay` rows (`UIComponent_StatDisplay_Type`) are left empty: no mapping from
> their type values to actor properties is given. On this install the `StatTree` list of the
> equipment page holds no row elements when the page opens (the rows are made by game code
> from the `stattree` templates, not traced), so every row stays empty. Please give the row
> creation rule, the stat-type → property mapping and the value element.

---

<a id="s4"></a>
## 4. Bags (Confirmed, CP5, CP6)

<a id="s4-1"></a>
### 4.1 Windows

The six bag windows are the top-level elements `PlayerBag1_Template` … `PlayerBag6_Template`
of `containerdisplay` (`ui-layout.md` §17.2 P15; bag 1 based on `InventoryBag_Template`, bags
2–6 on `PlayerBagOver1_Template`): a `UIScaler` frame with a background, a drag bar, a resize
bar, a close button, a title bar and the item list `Backpack_ItemList` (based on
`InventoryDisplayListbox`, widget `InventoryDisplay`: a horizontal list, `max_columns` 5, minimum
height 36). Bag 1 also has a toolbar (`InventoryToolbar`, a search entry).

| Bag | `UIComponent_ContainerDisplay_InputAction` | `UI_UserSetting_NumItemsInRow` |
|---|---|---|
| 1 | `QUICKSLOT_BACKPACK1` | `UI.Bag1Columns` |
| 2 | `QUICKSLOT_BACKPACK2` | `UI.Bag2Columns` |
| 3–6 | `QUICKSLOT_BACKPACK3` … `6` | `UI.Bag3Columns` … `UI.Bag6Columns` |

The input action is the command that toggles that bag. The column setting is a user setting overriding the
5 columns; the resize bar changes it (`ui-behaviour.md` §8.2).

> **OPEN:** whether the input action `QUICKSLOT_BACKPACK` (the label without a number) toggles
> all bags, and how it behaves when some bags are open and others closed; no bag window names
> it, and only its label suggests the meaning. Owner check O83. **Tessera rule:**
> `QUICKSLOT_BACKPACK` acts on every offered bag (§4.3): if at least one of them is closed it
> opens all of them, otherwise it closes all of them.

<a id="s4-2"></a>
### 4.2 Sizes

| Value | Source | Here |
|---|---|---|
| Base backpack slots | `InventoryControl_BaseBackpackSlots` | 45 |
| Slots per bag | `InventoryControl_SlotsPerBag` | 15 |
| Bags at most | `InventoryControl_MaxNumBags` | 6 |
| Slots per pouch (store extension) | `InventoryControl_SlotsPerPouch` | 5 |
| Equipment slots | `InventoryControl_NumEquipmentSlots` | 21 |
| Avatar template maxima | `Inventory_MaxBackpackSlots`, `…MaxEquipmentSlots`, `…MaxOverflowSlots` (`player-character.md` §4.4) | 75, 21, 20 |

<a id="s4-3"></a>
### 4.3 Which backpack slots a bag shows

> **OPEN:** how the original maps backpack slot indices to the six windows (base slots, bags
> bought later, pouches). Owner check O83. **Tessera rule** (slot indices are 0-based, as in
> the feed):
>
> 1. **Size *N*.** After the session's first `ContainerSize` for `Backpack`, *N* is the *n* of
>    the last one. Before it (a scenario without a `bag` step, `scenario-feed.md` §5.5), *N*
>    starts at `InventoryControl_BaseBackpackSlots` and, whenever an item arrives at backpack
>    slot *s* ≥ *N*, becomes *s* + 1; it never shrinks without a `ContainerSize`.
> 2. **Bags 1–5.** Bag *k* (*k* = 1 … 5) shows slots 15 × (*k* − 1) through
>    min(15 × *k*, *N*) − 1.
> 3. **Bag 6.** Bag 6 shows slots 75 through *N* − 1, however large *N* is (so every slot from
>    75 upwards, including 90 and above, is in bag 6).
> 4. A bag whose range is empty (*N* ≤ its first slot) is not offered. Each bag lists its
>    slots in ascending order, row by row with the bag's column count.
> 5. **Plugins.** The growth of rule 1 takes effect before the arriving item is reported to
>    plugins, so its index lies in 1 … `GetSize()`. Like every change of *N*, it fires the
>    plugin `SizeChanged`, queued before that item's `ItemAdded`; the event rule itself is
>    owned by `lua-plugin-api.md` §15.1 (`Backpack.SizeChanged` row).

<a id="s4-4"></a>
### 4.4 Sorting

`InventoryControl_SmartSortInfo_Array` (CP11) lists, per `Item_Client_Sort_Type`, the properties a
sort compares (`items.md` §6.4). **Tessera rule:** the bag sort (when offered) groups by
`Item_Client_Sort_Type` ascending (absent last), then compares the listed properties in order,
then the record DID; it only proposes moves, sent as `MoveItem` commands (§6).

---

<a id="s5"></a>
## 5. Item slot display (Confirmed layout, CP7)

Every item slot (bags, paperdoll, vendor and loot lists) is an `ItemDisplay_SlottedTemplate`
(widget `ItemDisplay`, 35 × 35; tooltip layout 0x22000003 and tooltip element `ToolTip_01`, no
tooltip text) holding `ItemDisplay_SlottedIcon` (based on `ItemDisplay_IconTemplate`, 32 × 32
at (3, 3)) and an animated overlay `item_changed`. The icon template stacks, over the
composed icon (`ItemDisplay_Icon`, widget `ItemIconDisplay`, `icon-layers.md` §3), these
overlays (all hidden by default; each switched by states):

| Overlay | States (image) | Reading of the name; Tessera rule |
|---|---|---|
| `ItemDisplay_QuantityOverlay` | text, right-justified, one line, at (0, 15) 32 × 12 | the stack quantity; **Tessera rule:** shown when the quantity is greater than 1 |
| `ItemDisplay_BrokenOverlay` | `broken`, `almostBroken`, `notBroken` | wear state; **Tessera rule:** as the durability row of `item-tooltips.md` §6.10 and §11 |
| `ItemDisplay_UnusableOverlay` | `overlay_on`/`off` | the viewer cannot use the item; **Tessera rule:** on when the item's `Usage_MinLevel` exceeds the viewer's level or its `Usage_RequiredClassList` (`items.md`) is present and does not contain the viewer's class |
| `ItemDisplay_CooldownOverlay` | `Meter` with 60 frames, draw mode Multiply | cooldown sweep; **Tessera rule:** follows `CooldownStarted`/`CooldownEnded` for the item's record (P2) |
| `ItemDisplay_SelectedOverlay`, `…_ElligibleOverlay`, `…_PendingOverlay`, `…_LockedOverlay`, `…_GhostedOverlay`, `…_NewItemOverlay`, `…_DarkenOverlay`, `…_OutOfRangeOverlay`, the legendary and essence overlays | `overlay_on`/`off` and others | **Tessera rule:** off in P1 |
| `ItemDisplay_RolloverOverlay` | rollover highlight | on while the pointer is over the slot |

There is no quality-dependent element in the slot template; the icon's background tile is the
only quality cue (`icon-layers.md` §4).

> **OPEN:** the original's conditions for each overlay. Owner check O83.

> **OPEN:** (implementer, `apps/client/src/character/panel.rs`, 2026-10-10, after review)
> The rule above does not say what an unknown viewer level or class does (a scenario player
> without `class=`, the plain sandbox). As implemented: an unknown level or class never
> makes an item unusable, as `tessera-gamedata`'s tooltip engine shows an unknown viewer
> value as meeting a requirement row (`item-tooltips.md` §6.15 does not say either; the
> same question applies there).

---

<a id="s6"></a>
## 6. Interactions (Tessera rules, CP10)

The original's item handling is game code not traced here; drag and drop mechanics are
`ui-behaviour.md` §7. Tessera sends the scenario feed's commands (`scenario-feed.md` §9.4) and
changes nothing locally until the matching event arrives:

| User action | Command |
|---|---|
| Drop a backpack item on a paperdoll slot whose `Container_Slot` bit is set in the item's `Inventory_CompatibleSlot` | `EquipItem { item, slot: key of that slot }` |
| Drop a backpack item on a paperdoll slot it does not fit | nothing (the drop target refuses it, `ui-behaviour.md` §7.3) |
| Activate a backpack item (the use command of the slot, right button by default) that has an `Inventory_DefaultSlot` with the `Equipment` bit | `EquipItem { item, slot: the lowest equipment key in its default slot }` |
| Activate any other backpack item | `UseItem { item, target: current target }` |
| Drop an equipped item on a backpack slot | `MoveItem { item, to: Backpack slot }` (or `UnequipItem` when dropped on the bag window outside a slot) |
| Activate an equipped item | `UnequipItem { item }` |
| Drop an item on another slot of the same or another bag | `MoveItem` (the feed swaps an occupant, `scenario-feed.md` §7.6) |
| Shift-drop a stack of 2 or more onto an empty slot | `SplitStack` with half the quantity rounded down (a stack of 1 is moved as an ordinary drop, `MoveItem`) |
| Drag an item out of every window and confirm | `DestroyItem` |

> **OPEN:** the original's bindings for use, split and destroy, and whether activating an
> equipped item unequips it. Owner check O84.

> **OPEN:** (implementer, `apps/client/src/character/panel.rs`, 2026-10-10) The last row's
> "confirm" is not specified. As implemented (Tessera rule): the first drag of an item out of
> every window only asks (a system line: drag it out again to destroy it); a second drag of
> the same item out of every window within 10 s sends `DestroyItem`. Owner check O84.

> **OPEN:** (implementer note, `crates/gamedata/src/inventory`, 2026-10-10) (1) The
> shift-drop row sends `SplitStack` for a drop on an empty slot, but `SplitStack` carries no
> target (`scenario-feed.md` §9.4) and the feed puts the new stack at the first free slot of
> the item's own container (§7.6 there). The slot the player chose is therefore ignored, also
> when it is in another bag or another container. Which rule applies: a `SplitStack` with a
> target, a follow-up `MoveItem` once the new item's `ItemAdded` arrives, or "split only
> within the same container, target ignored"? As implemented: `SplitStack` as the row says,
> target ignored. (2) "The lowest equipment key in its default slot" is read as the lowest key
> of the `Equipment` container: a key of the page (§3.3) or one with an API value (2 … 22,
> §8.2), so a default slot `Mounted` + `Equipment` equips to key 22 although the page has no
> `Mounted` slot. A drop on a place of the `Equipment` container is handled as a drop on the
> paperdoll slot of that key (fit check, `EquipItem`), never as a `MoveItem`. Please confirm.

`InventoryControl_MutuallyExclusiveLocation_MappingList` lists holding locations that cannot
be used together (two pairs here, CP11); the feed does not enforce it (no rules, decision 0024 §2.4).

---

<a id="s7"></a>
## 7. Worn look

The equipment container drives the player's appearance: the feed emits `AppearanceChanged`
with one `Equipped` per equipped item record, in ascending slot-key order
(`scenario-feed.md` §5.5), and the walker rebuilds the figure by `appearance.md` §8 (worn
items in ascending `ContainerSlot` order, held items in their slots). On M7's server the same
`Appearance` travels as opaque references (`tessera-protocol.md` §5.3), so observers see the
change (decision 0024 P1 exit criterion 3).

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D3):** `Equipped` gains an optional `slot`, the
> equipment slot key that holds the item: `scenario-feed.md` §5.5 and §9.1 (the feed fills it
> from the equipment slot), `tessera-protocol.md` §5.3 (flag bit 3, minor 1) and §7.4. When
> `slot` is present the walker places the held item in that slot; the Tessera rule below
> applies only when it is absent (minor-0 servers, other sources). The note as raised:
>
> **OPEN (for `scenario-feed.md` and `tessera-protocol.md`, non-blocking for armour):**
> `Equipped` carries no slot, so an item compatible with both hands cannot say which hand it is
> in. **Tessera rule until a revision adds the slot:** a held item goes to its
> `Inventory_DefaultSlot` hand; if an earlier entry of the list already took that hand and the
> item's `Inventory_CompatibleSlot` has `Weapon_Secondary`, it goes to the secondary hand.

---

<a id="s8"></a>
## 8. Plugin views (`lua-plugin-api.md` §15)

This section gives the API facts the panels share with the plugin views. **Which plugin
events fire for which feed event, and what the views' getters return, is owned by
`lua-plugin-api.md` §15.1 alone**; this spec does not restate it. The inventory model that
§15.1 reads is the one of §4.3 and §9 here.

<a id="s8-1"></a>
### 8.1 Backpack (Reported, CP8)

`LocalPlayer:GetBackpack()` returns the backpack object; `GetSize()` its slot count;
`GetItem(index)` the item at index 1 … `GetSize()` or nil (empty slot or index out of range).
Events: `ItemAdded`, `ItemRemoved` (arguments `Index`, `Item`), `ItemMoved` (`OldIndex`,
`NewIndex`, `Item`), `SizeChanged`.

Index *i* is the feed's backpack slot *i* − 1, and `GetSize()` is the backpack size *N* of
§4.3. The derivation of the four events from the feed: `lua-plugin-api.md` §15.1
(`Backpack` events row).

<a id="s8-2"></a>
### 8.2 Equipment (Reported + Confirmed, CP9)

`LocalPlayer:GetEquipment()` returns the equipment object; `GetItem(slot)` takes a value of
the `Turbine.Gameplay.Equipment` enumeration. The API values are the data's `ContainerSlot`
keys minus one: `Head` 1 ↔ key 2 `Head`, … , `Pocket` 15 ↔ key 16 `Pocket1`, `PrimaryWeapon`
16 ↔ 17, `SecondaryWeapon`/`Shield` 17 ↔ 18, `RangedWeapon`/`Instrument` 18 ↔ 19,
`CraftTool` 19 ↔ 20, `Class` 20 ↔ 21, `Bridle` 21 ↔ 22 `Mounted`. The FX slots (keys 23–25)
have no API value, so FX-slot items are not visible to plugins. Events `ItemEquipped`,
`ItemUnequipped` carry `Index` (the API value) and `Item`. `GetSize()`, `GetItem` and the
derivation of the two events from the feed: `lua-plugin-api.md` §15.1 (`Equipment` rows).

---

<a id="s9"></a>
## 9. The scenario feed

| Feed event (`scenario-feed.md` §9.2) | Panel effect |
|---|---|
| `ContainerSize { Backpack, n }` | Backpack size *N* (§4.3); bags shown accordingly |
| `ItemAdded`, `ItemMoved`, `ItemRemoved`, `ItemQuantity`, `ItemCondition` | Slot contents, quantity overlay, broken overlay, paperdoll slots for the `Equipment` container (slot = data key, §3.3); tooltips (`item-tooltips.md` §11) |
| `AppearanceChanged` of the player | The paperdoll viewport's figure and the walker's avatar (§7) |
| `CharacterInfo`, `LevelChanged` | Header: level, class icon, race-and-class line; the viewer's class and level for §5 |
| `VitalChanged` (health, power) | Morale and power rows (`current` / `max`) |
| `WalletChanged { Coin }` | The wallet view behind `Player_Wallet_Button` (money display of `item-tooltips.md` §6.11) |

Plugin events for these feed events: `lua-plugin-api.md` §15.1 (§8).

Commands from §6 go to the session; with `defaults on` the feed answers equip, unequip, move,
split and destroy generically (`scenario-feed.md` §7.6), so a scenario with a few `give` steps
is enough to test every interaction. A scenario that checks the worn look scripts `equip` steps
(feed §6.1) or `equip` input lines (feed §7.7).

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D5):** `scenario-feed.md` §6.1 `stat ENTITY PROPERTY=VALUE`
> (property by dictionary label or `pid:N:KIND`, §5.8 there) emits `PropertyChanged { entity,
> property, value }` (§9.2 there; Character group, surfaced in world, §9.5); values are
> scripted, never derived. The §3.4 rule reads the last reported value. The note as raised:
>
> **OPEN (for `scenario-feed.md`, non-blocking):** the stat rows (§3.4) need scripted values.
> Proposal for a revision of the feed: a step `stat ENTITY @Label VALUE` resolving the label
> through the property dictionary at resolution time (`enum_value`-like) and an event
> `PropertyChanged { entity, property: u32, value: f64 }` in the `Character` group; values
> scripted, never derived (decision 0024 §4). Until then the rows stay empty (§3.4 rule).

---

<a id="s10"></a>
## 10. Invariants

| ID | Invariant | Status |
|---|---|---|
| CPI1 | Every slot element of `CharacterEquipmentPage_SlotField` has a `Container_Slot` with exactly one bit set, and the 23 bits are distinct | Confirmed 2026-10-09 |
| CPI2 | Every element label in `UI_Inventory_EquipmentSlotList` names a slot element of the slot field (23 of 23) | Confirmed 2026-10-09 |
| CPI3 | The API `Equipment` value + 1 is a `ContainerSlot` key whose mapper name matches the API name (21 of 21, with `Pocket`↔`Pocket1`, `PrimaryWeapon`↔`Weapon_Primary`, `SecondaryWeapon`↔`Weapon_Secondary`, `RangedWeapon`↔`Weapon_Ranged`, `Bridle`↔`Mounted`) | Confirmed 2026-10-09 (names) |
| CPI4 | Each `PlayerBagN_Template` (N = 1 … 6) has a `Backpack_ItemList` whose input action is `QUICKSLOT_BACKPACKN` | Confirmed 2026-10-09 |
| CPI5 | Backpack index ↔ slot: `GetItem(i)` returns the item at slot *i* − 1 for every *i* in 1 … `GetSize()` | Design check (scenario) |

---

<a id="s11"></a>
## 11. Open questions (summary) and owner checks

| # | Question | Section | Settles it |
|---|---|---|---|
| Q1 | Command that toggles the character panel | §2 | owner (O82) |
| Q2 | Paperdoll rotation and pose | §3.2 | owner (O82) |
| Q3 | Stat values and buffed/debuffed | §3.4 | server data; feed step answered by decision 0025 D5 (§9) |
| Q4 | Backpack slots per window | §4.3 | owner (O83) |
| Q5 | Overlay conditions | §5 | owner (O83) |
| Q8 | Whether `QUICKSLOT_BACKPACK` toggles all bags | §4.1 | owner (O83) |
| Q6 | Use, split, destroy, unequip inputs | §6 | owner (O84) |
| Q7 | Held-item hand in `Equipped` | §7 | design (feed and protocol revision); answered by decision 0025 D3 |

---

<a id="s12"></a>
## 12. Decision 0024 P1 spec list: check of 2026-10-09

| Spec | What P1 needs from it | Result |
|---|---|---|
| `player-character.md` | Race, gender, nationality and class lists, names, slot limits, avatar templates | Complete for P1; the screens that use it are in `character-screens.md`; unchanged |
| `appearance.md` | Bodies, creation controls, worn and held items | Complete; the held-item hand gap of `Equipped` is §7 OPEN here; unchanged |
| `items.md` | Item properties, slots, value, binding, sets; tooltip properties | Revised 2026-10-09 (tooltip properties point to `item-tooltips.md`, compare kind's reader) |
| `icon-layers.md` | Composed icons; quality cue | Revised 2026-10-09 (§4 OPEN answered) |
| `game-panels.md` | Property displays, hide-when fields, drag boxes of the new panels | Its rules apply unchanged |
| `ui-behaviour.md` | Hover, drag and drop, window manager, tooltips | Revised 2026-10-09 (notes pointing to `item-tooltips.md`) |
| `lua-plugin-api.md` §15 | `GetBackpack`, `GetEquipment`, `Item`, `ItemInfo` | Revised 2026-10-09 (§15.1, checks 66–69) |
| `scenario-feed.md` | Containers, items, equip commands, character commands | Sufficient for P1 except the non-blocking gaps raised as OPENs in this spec (§7 hand, §9 stat values), `item-tooltips.md` §11 (instance properties) and `character-screens.md` §7.2, §8 (deletion, plain-sandbox creation); the file lives on branch `p0` and is not edited here |
| (new) | Item tooltips; selection and creation screens; character sheet and bags | `item-tooltips.md`, `character-screens.md`, this spec |

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (Review), for decision 0024 P1: character panel, equipment page, slot table, stat list, bag windows and sizes, item-slot overlays, interactions as Tessera rules, worn look, plugin `Backpack` and `Equipment` mapping, scenario-feed driving. |
| 2026-10-09 | Revised after the first-version review (blocking 1–3, non-blocking a–d, f). §4.3: one Tessera rule with numbered steps (bags 1–5 by range, bag 6 shows 75 … *N* − 1; *N* grows on arrival before the first `ContainerSize` and never shrinks without one; that growth fires the plugin `SizeChanged` before the item's `ItemAdded`, as `lua-plugin-api.md` §15.1 states). §8: plugin event derivation and the views' getters removed; `lua-plugin-api.md` §15.1 is their only owner (§9 points there). §4.1: `QUICKSLOT_BACKPACK` is an OPEN with a Tessera rule (Q8, owner check O83). Provenance: dates on CP2–CP7 and CPI2–CPI4; new row CP11 (the `UILAYOUT` entry of `journal`, re-read from the install 2026-10-09; `SmartSortInfo` count and the two mutually exclusive location pairs, from the decoded `InventoryControl` dump). §2 rect written as position and size; §6 split only for stacks of 2 or more; §5 unusable overlay names `Usage_RequiredClassList`. |
| 2026-10-10 | Pointers only (no fact changed): the OPENs of §7 (held-item slot) and §9 (stat values) and rows Q3, Q7 point to their answers in decision 0025 (`scenario-feed.md` revision 4, `tessera-protocol.md` v1.1). |
