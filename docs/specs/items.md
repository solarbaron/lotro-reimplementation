<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Items: item objects, their property sets, lookup tables and auxiliary records

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted), not the author |
| **Review record** | Re-review 2026-10-07 (a separate dirty-room review session, not the author): **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/wstate` / `crates/props` (item identification and typed accessors), a future gameplay-data crate (value, DPS, armour and durability evaluation; set and legacy tables), `apps/explorer` (item card), `crates/net` sandbox backend (inventory, vendors, loot) |
| **Companion specs** | [`wlib.md`](wlib.md) (class library, ancestor chains, §9 parent map); [`wstate.md`](wstate.md) (object-state records, §3–§8); [`property-system.md`](property-system.md) (dictionary §2, value encodings §4, property sets §5, the +0x09000000 rule §6.2, enum resolution §7, DID maps §8); [`string-table.md`](string-table.md) §9 (rendering StringInfo); [`icon-layers.md`](icon-layers.md) (item icon composition); [`appearance.md`](appearance.md) §4–§5 (worn and held visuals, the only part of items specified before this spec); [`ui-layout.md`](ui-layout.md) (tooltip layouts); [`combat-data.md`](combat-data.md) (progressions §4, `Mod_Array` §5, effects §6); [`economy.md`](economy.md) (loot §4, recipes §5, vendors and barter §6). This spec starts from decoded property sets and decoded object-state records. |

---

<a id="purpose"></a>
## Purpose

Items are the largest gameplay family in the data: 151,230 of the 383,332 game-object records
(prefix 0x70), 39 %. Inventories, loot tables, vendors, barter lists, crafting recipes and quest
rewards all point at them. This spec lets an implementer:

- recognise item records by their class-library ancestry and property signature, without
  hard-coding class indices (§2);
- know what the object-state record and the property set of an item contain, and which
  properties carry information versus which are inherited stamps, mirrored copies or values
  derived from shared tables (§3, §4);
- read the gameplay values a server needs (level, quality, value, stack size, binding,
  durability, weapon damage, armour, stat modifiers, on-use and on-equip effects, sockets,
  legendary data) and understand their encodings (§5);
- read the shared lookup and control tables that those values depend on, and recompute the
  derived values exactly as the data does (§6, §9);
- follow the auxiliary records that hang off items: item sets, relics, legacies and the
  directories that list them (§7);
- navigate the DID graph into and out of items (§8);
- build the explorer's item card (§10).

Item **visuals** (worn and held appearance) stay in `appearance.md`; item **icons** stay in
`icon-layers.md`. Effects, skills, traits, progressions and stat modifiers are specified in
[`combat-data.md`](combat-data.md); loot tables, recipes, vendors, barter and currency in
[`economy.md`](economy.md); quests and deeds in [`quests.md`](quests.md). This spec describes
only how items point at them. (Those three specs were written in parallel with this one and
are in Review; section references to them are to their 2026-10-07 versions.)

---

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam, client build 4906.0071.1794.4035) on 2026-10-06 and 2026-10-07 with throwaway probes in the dirty-room scratch area. The probes used the repository's own decoders (`tessera-dat`, `tessera-props`, `tessera-wstate`) as tools. **Every** object record was decoded: all 383,332 records with prefix 0x70 and their 383,332 property sets (0x79), all 1,616 stand-alone property sets (0x78), the property dictionary (0x34000000), the class library (0x56000000), all 735 enum mappers (0x23) and the DID maps 0x2800000E (WEENIECONTENT) and 0x28000001 (EMAPPER). Every count in this spec covers all records of the kind named, unless it says otherwise. |
| OBS-SB | Part of OBS: a count of how often each item property's PID occurs as a 4-byte little-endian constant inside the bytecode chunk (chunk type 1, `wlib.md` §4.1) of the script library. This is a statistical observation about the data file. It says nothing about what the code does with the value. |
| LC | LotroCompanion `lotro-tools` (item, item-set, value-table, DPS-table, weapon-speed and legendary loaders) and `lotro-core` (money and value-table classes), read for facts only. No code, comment or non-interoperability identifier was copied. |
| DS | `research/notes/data-semantics.md` §4 (WEENIECONTENT labels) and §12 (item property naming). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, file version read 2026-10-07), analysed headless in Ghidra on 2026-10-07 from a private copy of the shared project. **Question:** does the client's native code apply the value, DPS, damage, armour or set rules of §9 itself (so that they are client behaviour and not only relations in the data), and does it read item properties by PID? **Method:** a search of all instructions for immediate operands equal to 38 item-related PIDs, a search of the string data for property names, and decompilation of the functions that use the hits. **Why M1–M3 were not enough:** the data shows that stored values agree with the tables (§9), but not who computes them; no published source says; the running client does not show it. **Result:** no immediate operand equals the PIDs of `Item_Value`, `Item_ValueLookupTable`, `Combat_DamageVariance`, `Combat_Damage`, `Item_Armor_Value`, `Item_Armor_Value_Lookup_Table`, `Item_BaseActionDuration`, `LinearInterpolatingProgression_Array`, `Set_ActiveCount` or `Set_MemberList`, and no property name occurs as a string. The other PIDs do occur, but every decompiled use treats the number as a UI-element identifier, which shares the 0x1000xxxx number space. So BIN does **not** settle the question; the rules of §9 are stated as data relations, and who applies them stays OPEN (§14). Nothing from network, login, launcher or anti-tamper code was read. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| F1 | Item records are the 151,230 object records whose root class descends from class 1734 here; 15 classes have records (§2.1) | OBS (all 383,332 headers + `wlib.md` parent map) | Confirmed 2026-10-07 |
| F2 | Ancestor chain above the item base, and the sibling carry-all classes 4178/4179 (41 records) under the same parent (§2.2) | OBS | Confirmed |
| F3 | `WeenieType` low byte 0x81 marks 151,257 records: 151,216 items and the 41 carry-alls; 14 items have low byte 0x01 (§2.3) | OBS (all 383,332 sets) | Confirmed. The bit meaning is OPEN. |
| F4 | The class indices of the item kinds match those named by LC's current item loader | LC; OBS | Reported (stability), Confirmed (use here) |
| F5 | Object-state shape of item records: root attributes, the per-item scaffold object, links and available-property lists (§3.2) | OBS (151,230 records) | Confirmed layout; meaning OPEN |
| F6 | For all 41,881 records of class 795, the object state holds a map keyed by species + sex whose values equal the `Item_WornAppearanceMapList` entries (§3.3) | OBS | Confirmed |
| F7 | Property-set sizes: 41 to 117 top-level properties, mean 85.5; 527 distinct top-level names (§4.1) | OBS | Confirmed |
| F8 | Sixteen properties are present with the same value in all 195,930 records under ancestor 1733 and in no other record (§4.2) | OBS (all 383,332 sets) | Confirmed |
| F9 | Ten mirrored copies (`Forward_Cached_*`, `ForwardSource_*`, `*_Cached`, `*_Shadow`) and `Item_CurStructurePoints` equal their source in every item (§4.3) | OBS | Confirmed |
| F10 | None of the eight stamps and only two of the twelve copy labels occur in the script bytecode; 322 of the 507 other labels do (§4.6) | OBS-SB | Confirmed (count only) |
| F11 | `Item_Value` = value-table base value at `Item_Level` × quality factor, single-precision product, rounded half up; 145,518 of 145,518 records with a table (145,477 items, 41 carry-alls); null table ⇒ value 0 (5,087 of 5,087) (§9.2) | OBS; LC (formula) | Confirmed |
| F12 | `Combat_BaseDPS` = DPS-table base at (`ItemAdvancement_CombatDPSLevel` if present, else `Item_Level`) × quality factor as a single-precision product, exactly equal in 23,204 of 23,204 (a double-precision product is exact in only 1,115) (§9.3) | OBS; LC | Confirmed |
| F13 | `Combat_Damage` = `Combat_BaseDPS` × `Item_BaseActionDuration` × 2 ÷ (2 − `Combat_DamageVariance`), within 1.2 × 10⁻⁷ relative, 23,204 of 23,204 (§9.4) | OBS; LC (min/max reading) | Confirmed relation; min/max reading Reported |
| F14 | `Item_Armor_Value` = the armour progression evaluated at `Item_Level` (`combat-data.md` §4.10), truncated, 60,880 of 60,880 items that have both (§9.5) | OBS | Confirmed |
| F15 | `Item_MaxStructurePoints` and `Item_DurabilityValue` equal the wear-control tables at their enum keys (87,020 and 90,932 of all) (§9.6) | OBS | Confirmed |
| F16 | Weapon timing values equal the weapon-speed tables at (equipment category, speed) for 23,204 of 23,204 weapons (§9.7) | OBS; LC | Confirmed |
| F17 | Cooldown and decay enums used by items all have rows in the cooldown and decay control maps (§9.8) | OBS | Confirmed |
| F18 | Every non-null DID reference in the item property sets (3,607,800) resolves to an existing record; distribution by prefix and property (§8.1) | OBS (every archive's directory) | Confirmed |
| F19 | Inbound references to items: 129,560 distinct items are named by other records; sources by property (§8.2) | OBS (all 383,332 sets) | Confirmed |
| F20 | Every `Mod_Array` entry carries `Mod_Modified` and `Mod_Op`, and either `Mod_Progression` (289,349) or a field whose PID equals `Mod_Modified` (21,136); 12 also carry one unrelated field (§5.8) | OBS (310,497 entries) | Confirmed |
| F21 | The item level governs an item's stat modifiers (progression index and floor/ceiling test) | LC; `combat-data.md` §5.5 | Reported |
| F22 | Effect references: struct layout of the four `EffectGenerator_*` lists and their target classes (§5.9) | OBS | Confirmed layout; semantics of spellcraft Reported (LC) / OPEN |
| F23 | Behaviour objects named by `Usage_Action`, `Usage_Permission`, `Usage_TakePermission`, `CSM_ActionHandler` (§5.10) | OBS; WEENIECONTENT labels | Confirmed |
| F24 | Item sets: layout of class 1824, directory 0x70009869 (label `GameSetDirectory`), membership through `Set_MemberList` or through `Item_PropertySet` (§7.1) | OBS; LC | Confirmed layout; bonus-activation rule Reported |
| F25 | Relic, transmutation, runic-list, legacy, title and imbued-legacy records and how items reach them (§7.2–§7.4) | OBS | Confirmed layout; semantics mostly Reported/OPEN |
| F26 | Control records named in WEENIECONTENT that items depend on (§6) | OBS; DS (labels) | Confirmed |
| F27 | Binding combinations of `Inventory_BindOnAcquire` / `_BindToAccount` / `_BindOnEquip` (§5.4); their reading as binding kinds | OBS; LC | Combinations Confirmed; reading Reported |
| F28 | Money unit of `Item_Value` (copper; 100 copper = 1 silver; 1,000 silver = 1 gold) | LC | Reported |
| F29 | Enum mappers used by item properties and their EMAPPER labels (§11) | OBS | Confirmed |
| F30 | The native client code does not settle §9 (BIN result above) | BIN | BIN only (negative result) |

---

<a id="s1"></a>
## 1. Concepts and terms

| Term | Meaning in this spec |
|---|---|
| **Item record** | An object record (prefix 0x70, `wstate.md`) whose root class (offset 4) descends from the **item base class** *I* (§2). Its static properties are the property set at DID + 0x09000000 (prefix 0x79, `property-system.md` §6.2). |
| **Item kind** | The root class of an item record. Class names are not in the data (`wlib.md` §13); this spec uses its own descriptive labels. |
| **Property label** | The dictionary name of a PID (`property-system.md` §2). Labels are interoperability identifiers: implementations look PIDs up by label at run time and never hard-code PID numbers. The PIDs printed here are for test convenience only. |
| **Stamp** | A property present in every record of a class subtree with one value (§4.2, §4.4). |
| **Copy** | A property whose value equals another property of the same set in every record (§4.3). |
| **Derived value** | A stored value that equals a function of other stored values and shared tables (§4.5, §9). |
| **Authored value** | Any property that is none of the above: it carries per-item information (§4.6). |
| **Behaviour object** | A 0x70 record with an empty or near-empty property set, named by an item property, whose root class's script code implements a behaviour (§5.10). |
| **Control record** | A 0x70 record reached through a WEENIECONTENT label (`property-system.md` §8.1) that holds a game-wide table (§6). |

Throughout, "class *n*" is a class index observed in this install. Indices are build-specific
(`wlib.md` §1); §2.4 gives the data-driven way to find them.

---

<a id="s2"></a>
## 2. Identifying item records

<a id="s2-1"></a>
### 2.1 The item kinds (Confirmed, all 151,230 item records)

The item base class here is **1734**. It has 27 descendant classes; 15 of them are root classes
of records, the others (and 1734 itself) have none: 791, 793, 800, 811, 812, 813, 814, 816,
1721, 1735, 1736, 1737.

| Root class | Records | Path below 1734 | `WeenieType` (label: records) | `Item_Class` values | Distinct property labels | Signature (properties present in every record of the class, exclusive to it among item classes, or characteristic) | Descriptive label (this spec's) |
|---|---|---|---|---|---|---|---|
| 796 | 52,753 | direct | Item 52,719; PopupImage 17; Entity 10; Hotspot 4; Money 3 | 87 | 426 | Heterogeneous; 218 labels occur only in this class (sockets for essences and traceries, crafting trinkets, instruments, decorations, hotspots) | General items |
| 795 | 41,881 | 791 | Clothing 41,871; Item 10 | 16 | 136 | `Item_WornAppearanceMapList` (all 41,881), `Item_DurabilityEnum`, `Item_ClothingColorSecondary` (exclusive) | Worn apparel (armour and cosmetic clothing) |
| 799 | 23,204 | 816 → 1737 | Weapon 23,204 | 15 | 157 | `Combat_DPS_LUT`, `Combat_BaseDPS`, `Combat_Damage`, `Combat_DamageVariance`, `Combat_DamageType`, `Combat_WeaponSpeed` (all, exclusive) | Weapons |
| 802 | 19,720 | direct | Jewelry 19,720 | 3 | 126 | `Inventory_DefaultSlot` in a jewellery slot | Jewellery |
| 805 | 6,055 | 796 | Item 6,055 | 134 | 83 | `RecipeItem_Recipe`, `Usage_BarringCraftRecipe` (all, exclusive) | Recipe scrolls |
| 2814 | 3,634 | direct | PackageItem 3,634 | 22 | 120 | `PackageItem_TrophyListTemplate` (all, exclusive) and 18 other `PackageItem_*` / web-store labels | Packages (items with contents) |
| 797 | 2,902 | 813 → 1735 | Shield 2,902 | 1 | 119 | `Item_Class` = the shield key in all | Shields |
| 798 | 301 | 814 → 1736 | Letter 301 | 3 | 74 | `TextItem_Title`, `TextItem_PrefabContentList` (all, exclusive) | Readable text items |
| 1722 | 254 | 793 | Door 254 | 1 | 68 | `Usage_PlayersCanOpen`, `Usage_PlayersCanClose` (exclusive) | In-world doors |
| 815 | 228 | direct | Waypoint 228 | 1 | 78 | 13 exclusive labels (travel destination, layer lists, tracking) | In-world travel points |
| 2097 | 167 | direct | Door 166; Item 1 | 2 | 94 | 13 exclusive labels (`BigBattle_Interactive_*` extras, activity collections) | In-world interactive objects |
| 3663 | 71 | 796 | Item 71 | 1 | 73 | A usage effect whose data list carries `Effect_Crafting_RecipeList` | Multi-recipe scrolls |
| 803 | 40 | direct | Key 40 | 1 | 79 | `Key_CanBePlacedOnKeyRing` (exclusive) | Keys |
| 804 | 18 | direct | Milestone 18 | 1 | 55 | `Milestone_*`, `Travel_TravelFlags` (exclusive) | In-world milestones |
| 3924 | 2 | 796 | Item 2 | 1 | 82 | 11 exclusive `SPM_*` / `Usage_ModPropertyUsageData_*` labels | Property-modifying tokens |

`WeenieType` labels come from enum mapper 0x230000C9 (EMAPPER label `WeenieType`, §11). Four
kinds (1722, 815, 2097, 804) are fixtures placed in the world: they have `Item_Class` =
`NonInventory`, no `Item_Value` and no `Item_Level` (§5.2).

Byte totals (inflated payloads): 0x70 records 164,216,619 bytes (largest 2,856), 0x79 sets
205,932,378 bytes (largest 2,655).

<a id="s2-2"></a>
### 2.2 Ancestor chain (Confirmed)

The ancestor chain of the item base class, from the class upwards, is
1734 → 801 → 1733 → 787 → 1726 → 488 → 223 → 89 → 485 → 484.

- **801** has exactly two record-bearing subtrees: 1734 (items) and **4178** (39 records) with
  its child **4179** (2 records). The 41 records of 4178/4179 carry the same item signature
  (`Item_Class`, `Item_Value`, `Item_ValueLookupTable`, `Item_Level`, `Inventory_MaxQuantity`,
  `Inventory_CompatibleSlot`, icon layers) and, per LC, are carry-all containers
  (`WeenieType` label `BagOfSingletons`; their sets hold `BoS_*` properties naming the items
  they accept). A server should treat them as items; the item rules of this spec apply to them
  unchanged (they are included in F11). They are **not** in the 151,230 count.
- **1733** is the root of every placeable world entity: its subtree holds 195,930 records
  (items, creatures, NPCs, containers, world objects). The entity stamps of §4.2 come from it.
- Class **794** (1,722 records, under 1732 → 695 → 1733) and classes 1989, 1113, 1210, 1724
  carry `Item_Class` in 2,585 records but are not items: they are world containers, resource
  nodes and creatures. Do not use `Item_Class` alone to recognise items.

<a id="s2-3"></a>
### 2.3 Property signature (Confirmed)

Over all 383,332 property sets:

| Test | Items (151,230) | Other records |
|---|---|---|
| Has `Item_Class` | 151,230 | 2,585 (§2.2) |
| Has `Icon_Layer_BackgroundDID` | 151,230 | 747 (relics 706, carry-alls 41) |
| Has `Item_Value` | 150,564 | 41 (carry-alls) |
| Has `Item_ValueLookupTable` | 150,564 | 58 |
| Has `Inventory_MaxQuantity` | 148,895 | 54 |
| `WeenieType` low byte = 0x81 | 151,216 | 41 (carry-alls) |
| `WeenieType` low byte = 0x01 | 14 (10 `Entity`, 4 `Hotspot`, all class 796) | 6,007 |

> **OPEN:** The bit structure of `WeenieType` values (for example `Weapon` = 0x20081,
> `Clothing` = 0x30081, `Jewelry` = 0x200081). The low byte 0x81 tracks the item and carry-all
> subtrees almost exactly, but no rule is confirmed; use the label, not the bits.

<a id="s2-4"></a>
### 2.4 Rule for implementers (normative)

1. Find the classes at run time, without hard-coding 1734 or 801:
   - take every DID listed by vendor, barter and treasure-group tables (`Vendor_Item` elements
     of `Vendor_ItemList`, `Barter_Item`, `TreasureGroupProfile_ItemTable_Item`; §8.2) and read
     each record's root class;
   - let *P* be the deepest class that is an ancestor-or-self of all of those root classes
     (`wlib.md` §9 gives the parent map). Here *P* = 801: all 308,799 references fall in its
     subtree (`Vendor_Item` 91,274, all from class 1724; `Barter_Item` 100,761, all from class
     1943; `TreasureGroupProfile_ItemTable_Item` 116,764, of which 116,706 from class 1235 and
     58 from class 2372). No reference is null or names a missing record;
   - let *I* be the child of *P* whose subtree holds the most of those references. Here
     *I* = 1734 with 308,778 (all vendor and treasure references and 100,740 barter
     references); the other 21 are barter references that name carry-alls (4178);
   - a reader may instead use 1734 and 801 as fast hints after checking that the record named
     by the WEENIECONTENT label `BankFillItem` (0x70000EF3 here, class 799) descends from 1734.
2. A record is an **item** if its root class is *I* or descends from *I*.
3. A record is **item-like** (treated as an item by inventory code) if its root class descends
   from *P*. This adds the carry-alls of §2.2.
4. The **kind** is the root class. Code that needs a kind uses the signatures of §2.1 or the
   `WeenieType` label, never the class index alone.

> **Decision (Tessera):** the explorer and the sandbox server classify items by rule 2–3. A
> record whose root class descends from *I* but whose property set lacks `Item_Class` would be
> reported as malformed; none exists.

> **OPEN (implementer, 2026-10-07):** Step 1 does not say what happens when two children of
> *P* hold the same number of references. `crates/formats/src/gameplay/items.rs`
> (`ItemClasses::discover`) picks the lower class index and counts references whose class is
> *P* itself for no child. Please confirm or state a rule.

> **OPEN (implementer, 2026-10-07):** Rule 4 cannot separate two kinds of §2.1. The in-world
> interactive objects (2097) carry `WeenieType` `Door` like the doors (1722), and their
> signature ("13 exclusive labels (`BigBattle_Interactive_*` extras, activity collections)")
> names no label present in every record; the property-modifying tokens (3924) carry `Item`
> like the general items, and their "11 exclusive `SPM_*` / `Usage_ModPropertyUsageData_*`
> labels" are not listed. Please name, for each of the two, labels present in every record
> of the class and absent from the others. Until then `ItemView::kind` reads them as Door
> and General. Engineering choice recorded here: the kind is decided by, in order, the
> signatures that §2.1 marks "all" or "exclusive" (weapon, recipe scroll, multi-recipe
> scroll, package, text item, door, key), then the `WeenieType` label (`Weapon`, `Clothing`,
> `Jewelry`, `Shield`, `PackageItem`, `Letter`, `Door`, `Key`, `Waypoint`, `Milestone`; the
> other labels give no kind), then `Item_WornAppearanceMapList`, else General.

---

<a id="s3"></a>
## 3. The record pair

<a id="s3-1"></a>
### 3.1 Where the values are (Confirmed)

Each item is two records with the same serial: the object-state record (0x70) and the
property set (0x79). All 151,230 pairs exist and decode exactly. **Every gameplay value in this
spec is in the property set.** The object-state record holds a script-object scaffold whose
attributes are almost all null references, empty containers or zero; its only data that
duplicates a property is the worn-appearance map of §3.3.

<a id="s3-2"></a>
### 3.2 Object-state shape (Confirmed layout, meaning OPEN)

Decoded with `wstate.md`. Attributes are named by their name hash (`wlib.md` §8.2).

| Kind | Root attributes (hash: type) | Items in the bundle | Links (`wstate.md` §6) |
|---|---|---|---|
| 796, 802, 797, 805, 798, 815, 2097, 3663, 804, 3924 | 0x035B97AF, 0x04343517, 0x0BCFA314 (references) | 2 (root + scaffold) up to 55 (796), 34 (802), 21 (797); 8 in every 805 and 3663 record | One link of type 8 (limit 1) in most. 13,874 general items add type 13 (limit 99, both directions) and type 12 (limit 250, both directions), and 11,145 more add a type 0x10000019 link as well; recipe scrolls have type 8 and two type-13 links; travel points, doors and interactive objects have a type 0x1000000B link; milestones have none |
| 795, 799 | the three above + 0x0905B9D8 (reference) | 18–38 (795), 3–38 (799) | One link, type 8 |
| 2814 | nine attributes (five integers, four references) | 3–7 | Five links (types 8, 13, 13, 12, 12) |
| 1722 | the three + 0x0516AFF9 | 2 | Six to eight links |
| 803 | the three + 0x0F9CB3A4 | 2–6 | One link |

- Attribute 0x04343517 references an object of **class 701** in all 151,230 records (13
  attributes; most null). In records with 5 or more items, that object leads through a
  `multimap_u32` (class 13) and a `refmap_u32` (class 35) to an object of class 700 and
  further objects of classes 732, 1342, 1626, 1339, 699 and native classes 166 (empty property
  set), 182, 39 and 199.
- Every item record has a non-empty available-property list (`wstate.md` §7) except 182
  general items; the list depends on the kind (one to 66 distinct lists per class) and only
  22 % of its entries are labels that the item's property set has.
- No item record contains a DID item. Imports (`wstate.md` §3.2) occur only in class 795 (7 in
  39,542 records, 8 in 2,339: appearance records, among them the wardrobe records of §3.3) and
  in 200 weapons (7 each: the worn battle gauntlets).

> **OPEN:** What the class-701 scaffold, the class-700 object and their integer fields
> represent at run time. Nothing in this spec depends on them; an offline reader may skip the
> object-state record of an item entirely.

<a id="s3-3"></a>
### 3.3 Worn-appearance mirror (Confirmed)

In all 41,881 records of class 795, root attribute 0x0905B9D8 references a `refmap_u32`
(class 35). Its keys are **species key | sex key** (for example 23 | 0x1000 = 4,119) and each
value is an object of class 2065 with two integers: the `Item_AppearanceKey` value and the
wardrobe record DID (0x20…). For every one of the 41,881 records this map equals the
`Item_WornAppearanceMapList` of the property set (same keys, same pairs). The 205 other items
with that list (200 weapons, 5 general items) have no such map. `appearance.md` §4 reads the
property list; readers need not read the mirror.

> **OPEN (implementer, 2026-10-07):** The sentence "The 205 other items with that list (200
> weapons, 5 general items) have no such map" is not reproduced. Running
> `crates/formats/tests/gameplay_items_real_data.rs` (it decodes the object-state record of
> every item that has `Item_WornAppearanceMapList`) finds root attribute 0x0905B9D8 resolving
> to a non-empty `refmap_u32` in all 200 weapons with the list as well, and in each of them
> the map equals the list (keys and the two integers); only the 5 general items have none.
> Is the sentence wrong, or does it describe something other than a non-null reference to a
> non-empty map? Should IT12 cover the weapons too? The test asserts IT12 for every record
> that has a mirror and reports the classes; it does not assert which classes those are.

---

<a id="s4"></a>
## 4. What an item's property set contains

<a id="s4-1"></a>
### 4.1 Census (Confirmed)

| Measure | Value |
|---|---|
| Top-level properties per item | min 41, max 117, mean 85.5, median 84 |
| Total top-level values | 12,925,452 |
| Distinct top-level labels | 527 |
| Largest groups by label prefix (labels / values) | `Item` 75 / 2,285,749; `Usage` 103 / 1,953,330; `Inventory` 17 / 1,140,841; `UI` 9 / 924,512; `BigBattle` 15 / 907,497; `MC` 5 / 756,150; `Forward` 5 / 756,150; `Icon` 6 / 549,015; `Entity` 7 / 303,676; `ForwardSource` 3 / 302,462; `Combat` 12 / 238,062 |

Every property is classified below into exactly one of five categories. Per item, on average:
8.0 entity stamps (§4.2), 10.6 copies (§4.3), 3.2 derived values (§4.5), 19.7 class-uniform
values (§4.4) and **44.0 authored values** (§4.6).

| Kind | Mean properties | Stamps | Copies | Derived | Class-uniform | Authored |
|---|---|---|---|---|---|---|
| 796 | 76.1 | 8 | 10.1 | 1.1 | 4 | 52.9 |
| 795 | 89.4 | 8 | 10.9 | 3.9 | 22 | 44.6 |
| 799 | 102.1 | 8 | 11.0 | 7.0 | 34 | 42.2 |
| 802 | 86.5 | 8 | 11.0 | 4.0 | 28 | 35.5 |
| 805 | 76.4 | 8 | 10 | 1 | 40 | 17.4 |
| 2814 | 82.2 | 8 | 10 | 1 | 34 | 29.2 |
| 797 | 92.8 | 8 | 11.0 | 4.0 | 38 | 31.8 |
| 798 | 65.0 | 8 | 10 | 1 | 31 | 15.0 |
| 1722 | 55.2 | 8 | 10 | 0 | 22 | 15.2 |
| 815 | 47.5 | 8 | 10 | 0 | 19 | 10.5 |
| 2097 | 45.0 | 8 | 10 | 0.0 | 9 | 18.0 |
| 3663 | 73 | 8 | 10 | 1 | 46 | 8 |
| 803 | 65.7 | 8 | 10 | 1 | 27 | 19.6 |
| 804 | 54.2 | 8 | 10 | 0 | 27 | 9.2 |
| 3924 | 82 | 8 | 10 | 1 | 55 | 8 |

"Class-uniform" in a small class is partly coincidence (3924 has two records); the column is an
observation, not a schema.

<a id="s4-2"></a>
### 4.2 Entity stamps (Confirmed)

These eight labels, and their eight copies of §4.3, are present in **all 195,930 records under
ancestor 1733** (items, creatures, NPCs and other world entities) with one value each, and in
no record outside that subtree. Each value equals the dictionary default of its label
(`property-system.md` §2.2).

| Label | Type | Value everywhere |
|---|---|---|
| `MC_Trait_Loremaster_BredForWar_AttackSpeed`, `…_BleedDamage`, `…_CritChance`, `…_FlankChance`, `…_Threat` | float | 0.0 |
| `BigBattle_OfficerBannerCanHeal` | boolean | false |
| `Boss_Zone_1` | int | 0 |
| `Skirmish_RoleCombo` | enum | key 40 |

They carry **no item information**. A reader may drop them; a writer producing item data for
Tessera's own server does not need them. None of these sixteen PIDs (eight stamps, eight
copies) occurs in the script bytecode (F10).

Four more labels are nearly the same: `BigBattle_Interactive_Impulse_Min`, `…_Max`,
`…_Min_Base`, `…_Max_Base` are 0.0 in all items except one class-2097 record (10.0 and 40.0).

<a id="s4-3"></a>
### 4.3 Copies (Confirmed)

Each label in the first column equals the label in the second column in **every** item that
has it (all counts are of items):

| Copy | Source | Items |
|---|---|---|
| `Forward_Cached_MC_Trait_Loremaster_BredForWar_*` (5 labels) | the five `MC_Trait_*` stamps | 151,230 each |
| `ForwardSource_Skirmish_RoleCombo` | `Skirmish_RoleCombo` | 151,230 |
| `ForwardSource_Boss_Zone_1` | `Boss_Zone_1` | 151,230 |
| `BigBattle_OfficerBannerCanHeal_Cached` | `BigBattle_OfficerBannerCanHeal` | 151,230 |
| `Usage_Action_Shadow` | `Usage_Action` | 151,230 |
| `Usage_MinUsageDist_Shadow` | `Usage_MinUsageDist` | 151,230 |
| `Item_CurStructurePoints` | `Item_MaxStructurePoints` | 87,020 |

`ForwardSource_Movement_SpeedMultiplier` occurs in two items without its source. Readers keep
copies when round-tripping data and ignore them otherwise; on a live item, the current
structure points start equal to the maximum (§5.5).

<a id="s4-4"></a>
### 4.4 Class-uniform values (Confirmed per class; not a schema)

A property is class-uniform for kind *K* when every record of *K* has it with one value.
Examples that matter to gameplay (each holds in all records of the kind):

| Kind | Label | Value |
|---|---|---|
| 799 (23,204) | `Combat_DamageVariance` | 0.25 |
| 799 | `Combat_WeaponSpeed` | key 1 (`Normal`, mapper 0x23000066) |
| 799 | `Item_BaseActionDuration` | 1.0 |
| 799 | `Item_BaseAnimDurationMultiplierMod` | −0.1 |
| 799 | `Combat_WeaponHarmless` | false |
| 795, 799, 797, 802 | `Inventory_MaxQuantity` | 1 |
| 805 | `Usage_DestroyOnUse` | true |
| 797 | `Item_Class` | the shield key (33) |

These values are game rules expressed as data. **They must be read from each item, not
hard-coded**, because a later client build may change them; for weapons they are also
derived from shared tables (§9.7).

<a id="s4-5"></a>
### 4.5 Derived values (Confirmed relations)

| Stored label | Items | Derived from | § |
|---|---|---|---|
| `Item_Value` | 150,564 | `Item_ValueLookupTable`, `Item_Level`, `Item_Quality` | §9.2 |
| `Combat_BaseDPS` | 23,204 | `Combat_DPS_LUT`, level, `Item_Quality` | §9.3 |
| `Combat_Damage` | 23,204 | `Combat_BaseDPS`, `Item_BaseActionDuration`, `Combat_DamageVariance` | §9.4 |
| `Item_Armor_Value` | 60,880 of 64,322 | `Item_Armor_Value_Lookup_Table`, `Item_Level` | §9.5 |
| `Item_MaxStructurePoints` | 87,020 | `Item_MaxStructurePointsEnum` + wear control | §9.6 |
| `Item_DurabilityValue` | 90,932 | `Item_DurabilityEnum` + wear control | §9.6 |
| `Item_BaseActionDuration`, `Item_BaseAnimDurationMultiplierMod` | 23,204 | equipment category, `Combat_WeaponSpeed` + speed tables | §9.7 |

The stored value and the recomputed value agree in every record. A server may use either; a
Tessera writer that changes an input must recompute the derived value.

The other 3,442 `Item_Armor_Value` values (3,412 of class 795, 30 of class 797) have no lookup
table and are all 0.

<a id="s4-6"></a>
### 4.6 Authored values

Everything else carries per-item information: 507 of the 527 labels occur in at least one item
outside §4.2–§4.3. 322 of those 507 PIDs occur as constants in the script bytecode (OBS-SB), for
example `Item_Level` (18 occurrences), `Combat_BaseDPS` (18), `Item_DurabilityValue` (13),
`Item_Quality` (10), `Item_Value` (9), `Combat_DPS_LUT` (8), `Mod_Array` (5),
`Inventory_MaxQuantity` (4). The sixteen labels of §4.2 occur zero times. This supports §4.2 but is a
count, not a statement of what the scripts do.

The property groups of §5 list the authored labels that matter for gameplay. Labels not listed
(UI tweaks, physics and render flags, housing, hobbies, monster play, web-store flags) are
decoded generically (`property-system.md`) and shown raw by the explorer.

---

<a id="s5"></a>
## 5. Property groups

Columns: **Items** = number of item property sets with the label at top level; **Type** =
property type code name (`property-system.md` §3). "Meaning" is Confirmed only where a relation
in §9 or a census proves it; otherwise it is marked Reported (LC), "label" (the plain reading of
the interoperability label, not confirmed by any observation) or OPEN.

<a id="s5-1"></a>
### 5.1 Identity and display

| Label | Type | Items | Observed values | Meaning |
|---|---|---|---|---|
| `Name` | StringInfo | 151,230 | 134,004 distinct | Item name (render per `string-table.md` §9) |
| `PluralName` | StringInfo | 48,923 | | Plural form for stacks (Reported LC) |
| `Description` | StringInfo | 150,592 | | Description text |
| `Item_Class` | enum (0x23000036) | 151,230 | 260 of the mapper's 279 keys | Item category; the mapper has display texts (278) |
| `Item_Quality` | enum (0x23000067) | 150,564 | keys 0–5: 1,839 / 8,238 / 45,577 / 29,227 / 33,424 / 32,259 | Quality; indexes the quality factors of §9.2–§9.3 (Confirmed) |
| `Item_EquipmentCategory` | bitfield 64 (0x2300004C) | 91,190 | one bit set in 91,154, zero in 36; 39 distinct values | Equipment category; bit *i* = key *i* + 1 (`property-system.md` §7.3) |
| `WeenieType` | enum (0x230000C9) | 151,230 | 15 values (§2.1) | Object behaviour kind |
| `Auction_CategoryType` | enum (0x230001D1) | 109,148 | 46 | Auction category (label) |
| `Item_Client_Sort_Type` | enum (0x23000317) | 28,546 | 7 | Sort group (label); `InventoryControl_SmartSortInfo_Array` lists property IDs per group (§6.4) |
| `Item_MaterialType` | enum (0x230000F3) | 87,982 | 13 | Material (OPEN use) |
| `Item_ImplementType` | enum (0x23000004) | 29,220 | 16 | OPEN |
| `Icon_Layer_*` | DID → 0x41 | see `icon-layers.md` §2.2 | | Icon layers |
| `PhysObj` | DID → 0x47 | 151,230 | 10,295 distinct | Ground / held look (`appearance.md` §5) |
| `ExternalAppearanceID` | DID → 0x20 | 138,462 | 0x20000004 in 138,428, 0x200020F1 in 34 | OPEN |
| `Item_WornAppearanceMapList` | array | 42,086 | | Worn look (`appearance.md` §4) |
| `UI_Examination_Tooltip_0Click_Layout`, `…_1Click_Layout`, `…_Compare_Layout` | DID → 0x22 | 149,695 / 148,979 / 107,479 | 0x22000000 in all but 11, 11 and 20 | Tooltip layout record (`ui-layout.md`) |
| `UI_Examination_Tooltip_0Click_Element`, `…_1Click_Element`, `…_Compare_Element` | enum (0x230000D1) | 149,695 / 148,979 / 107,479 | 25 / 22 / 10 | Element of that layout used as the tooltip |
| `UI_Examination_Tooltip_Compare_Type` | enum | 110,672 | 6 | OPEN |
| `Entity_AdditionalScriptTables` | array of `ScriptTable_DBScriptTable` (DID → 0x07) | 149,924 | 1,163 distinct lists | Script tables attached to the item's entity (`script-table.md`) |

<a id="s5-2"></a>
### 5.2 Level, scaling and distribution

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Item_Level` | int | 150,478 | 1–580, 325 distinct; 1 in 29,639 | Item level; indexes value, DPS and armour tables (Confirmed, §9). Absent in 752 items: all 254 doors, 228 travel points, 166 interactive objects, 18 milestones, 86 general items. Dictionary default 0. |
| `Usage_MinLevel` | int | 100,735 | 2–160 | Minimum character level to use (Reported LC). Equal to `Item_Level` in 40,703 items, lower in 51,520, higher in 8,512. Dictionary default 1. |
| `Usage_MaxLevel` | int | 15,109 | 44 distinct | Maximum character level to use (Reported LC) |
| `Item_DistributionType` | enum (0x2300028C) | 83,031 | Loot 74,544; Quest 3,696; Craft 1,552; Instance 1,261; Raid 746; … (13 keys) | Source category; `InventoryControl_DistributionToLevelOffsetList` maps each key to a level offset (0 for all 15 keys here; §6.4) |
| `Item_iLevel_Crafting_Offset` | int | 30,250 | 6 distinct; 0 in 60 % | OPEN |
| `Item_iLevel_Bonus_Limit`, `Item_iLevel_Bonus_Chance` | int, float | 27,983 each | limit 0–5, chance 0–0.5 | Chance and cap of a random item-level bonus (Reported LC) |
| `ItemMunging_MinMungeLevel`, `ItemMunging_MaxMungeLevel` | int | 29,733 / 8,740 | | Level-scaling window (Reported LC) |
| `ItemMunging_ItemLevelOverrideProgression` | DID → progression (class 1389) | 20,798 | 54 distinct | Progression that maps a driving property to the item level (Reported LC) |
| `ItemMunging_ItemLevelOverrideProperty` | property ID | 32,486 | `Item_Level` 16,517; `Advancement_Level` 9,204; `ze_skirmish_level` 6,765 | The driving property (Reported LC) |
| `Item_LevelScaling_OnLevelForPlayersBelowLevel` | int | 126 | 6 distinct | OPEN |
| `SpellcraftCalculator` | DID (class 3177) | 56 | 6 distinct | OPEN |

Combinations of the munging labels over all items: none (110,855); only the override property
(9,321); min + progression + property (`ze_skirmish_level` 6,063, `Advancement_Level` 4,798);
min + property (4,595); min + progression (4,539); and nine rarer combinations.

> **OPEN:** The scaling rule (how min, max, override progression and override property combine
> into a live item level) is not in the data; LC applies "if `Item_Level` < min, use min" for
> display. Tessera's offline viewer shows the stored `Item_Level` and the munging fields raw.

<a id="s5-3"></a>
### 5.3 Economy

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Item_Value` | int | 150,564 | 4,278 distinct; 0 in 14,873 | Base value in copper coins (unit Reported, F28); derived (§9.2) |
| `Item_ValueLookupTable` | DID → class 1688 | 150,564 | 100 distinct; null in 5,087 | Value table (§6.1) |
| `Barter_IsBarterItem` | boolean | 41,888 | true 5,366 | OPEN |
| `Item_ForceNotSellable` | boolean | 3,933 | true in all | Not sellable (label) |
| `Item_Disenchant_Value` | int | 62,162 | −1 (25,668), 0 (10,149), positive (26,345) | OPEN |
| `Item_Disenchant_Component_Result` | DID → item | 26,345 | 45 distinct | An item (class 796 or 2814): the disenchanting result (label) |
| `Item_Disenchant_Trophy_List` | DID → class 1237 (loot list) | 992 | 26 | A loot list: the disenchanting result (label) |
| `Item_MithrilCost` | int | 308 | 8 distinct | OPEN |
| `Item_WearState` | enum (0x230000FB) | 87,020 | key 2 (`Pristine`) in all | Wear state; wear control maps it to a value factor (§6.3) |

<a id="s5-4"></a>
### 5.4 Inventory, slots and binding

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Inventory_MaxQuantity` | int | 148,895 | 1 (114,461), 50 (18,924), 100, 10, 0 (2,979), 25, 500, 5, … up to 999,999,999; 38 distinct | Stack size limit (Reported LC). 0 occurs; its meaning is OPEN. |
| `Inventory_Unique` | boolean | 5,067 | true in all | One per inventory (Reported LC) |
| `Inventory_UniqueEquipped` | boolean | 15,417 | true 12,629 | OPEN |
| `Item_UniquenessChannel` | enum (0x2300043B) | 15,416 | 363 distinct | Group within which uniqueness applies (socketables) (OPEN) |
| `Inventory_CompatibleSlot` | bitfield 32 (0x230000EE) | 148,895 | 27 distinct | Slots the item may occupy (`appearance.md` §4.1, §5.2) |
| `Inventory_DefaultSlot` | bitfield 32 | 128,084 | 24 distinct; `Backpack` 36,894; equipment slot + `Equipment` bit in 91,154 | Slot it goes to by default |
| `Inventory_PrecludedSlot` | bitfield 32 | 128,078 | `Equipment` 81,026; none 40,151; `Weapon_Secondary`+`Equipment` 6,901 | Slots blocked while equipped (Reported LC) |
| `Inventory_CanBeContained` | boolean | 149,447 | true 148,235 | OPEN |
| `Inventory_BindOnAcquire`, `Inventory_BindToAccount`, `Inventory_BindOnEquip` | boolean | 148,891 / 81,821 / 148,295 | table below | Binding (reading Reported LC) |
| `Inventory_UseOnAcquire` | boolean | 2,230 | true in all | OPEN |
| `Inventory_Indestructible` | boolean | 278 | true in all | OPEN |
| `Inventory_QuestItem`, `Usage_IsQuestItem`, `Quest_IsQuestToken` | boolean | 743 / 10,995 / 5,112 | | Quest-item flags (OPEN) |
| `Inventory_ParentingLocation`, `Inventory_PaperdollParentingLocation`, `Inventory_ParentingOrientation` | enum | 4,734 / 3,794 / 26,164 | | Held-item placement (`appearance.md` §5.3) |

Binding combinations (acquire, account, equip; "–" = label absent), all 151,230 items:

| BindOnAcquire | BindToAccount | BindOnEquip | Items | Reading (Reported LC) |
|---|---|---|---|---|
| true | true | false or – | 37,971 | Bound to the account when acquired |
| true | false or – | false or – | 33,098 | Bound to the character when acquired |
| false | false or – | true | 46,521 | Bound to the character when equipped |
| false | true | true | 30 | OPEN (account-bound on equip?) |
| false or – | false or – | false or – | 33,610 | Not bound |

Dictionary defaults of all three are false.

<a id="s5-5"></a>
### 5.5 Durability and wear

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Item_DurabilityEnum` | enum (0x230000FA) | 90,932 | keys 3 (78,417), 4 (8,551), 6 (3,912), 1 (34), 2 (18) | Sturdiness class |
| `Item_DurabilityValue` | float | 90,932 | 0.75 / 0.8 / 1.0 / 0.85 / 0.0 for those keys | Derived (§9.6) |
| `Item_MaxStructurePointsEnum` | enum (0x230000F9) | 87,020 | 48 distinct | Structure-points class |
| `Item_MaxStructurePoints` | int | 87,020 | 12 distinct (28–100) | Derived (§9.6) |
| `Item_CurStructurePoints` | int | 87,020 | = max | Copy (§4.3) |
| `Item_RepairCostPerPointProgression` | DID → class 1385 (float progression) | 87,020 | 8 distinct | Repair cost per point (label); evaluation OPEN |

Durability labels occur on all of 795, 797, 799, on 19,520 of 802 and on 3,425 of 796.

<a id="s5-6"></a>
### 5.6 Weapons (kind 799; all 23,204 have every label in this table except where noted)

| Label | Type | Observed | Meaning |
|---|---|---|---|
| `Combat_DPS_LUT` | DID → class 1660 | 6 distinct | DPS table (§6.2) |
| `Combat_BaseDPS` | float | 801 distinct | Damage per second; derived (§9.3) |
| `Combat_Damage` | float | 801 distinct | Maximum damage per hit; derived (§9.4) |
| `Combat_DamageVariance` | float | 0.25 in all | Spread: minimum = maximum × (1 − variance) (Reported LC) |
| `Combat_DamageType` | enum (0x23000028) | 9 keys, all powers of two | Damage type |
| `Combat_WeaponSpeed` | enum (0x23000066) | key 1 in all | Speed class; with the equipment category selects §9.7 |
| `Item_BaseActionDuration`, `Item_BaseAnimDurationMultiplierMod` | float | 1.0 and −0.1 in all | Attack duration in seconds (Reported LC) and animation-speed modifier; derived (§9.7) |
| `Combat_SingleWeaponStance`, `Combat_DualWieldStance`, `Combat_WithShieldStance` | enum (0x23000092) | 7 / 7 / 6 | Combat stance per wield mode (OPEN) |
| `Combat_WeaponSlayerGenus`, `Combat_WeaponSlayerAddMod` | bitfield 32, float | 3,011 items | Bonus against creature genera (Reported LC) |
| `ItemAdvancement_CombatDPSLevel` | int | 3,766 items | Level used for the DPS lookup instead of `Item_Level` (Confirmed, §9.3) |
| `EffectGenerator_TargetEffectList` | array | 3,084 items | Effects applied to the struck target (§5.9) |

`InventoryControl_DefaultEquipperEffectList` and `…_DefaultTargetEffectList` (§6.4) add effects
by equipment category to **every** weapon or shield of that category; they are not in the item.

<a id="s5-7"></a>
### 5.7 Armour

| Label | Type | Items | Meaning |
|---|---|---|---|
| `Item_Armor_Value` | int | 64,322 (classes 795, 797, 796) | Armour; derived where a table exists (§9.5), else 0 |
| `Item_Armor_Value_Lookup_Table` | DID → class 3918 (linear-interpolating progression) | 60,916 | Armour progression; 126 distinct. 36 jewellery items have the table but no armour value; the table gives 0 for them. |

The armour stat may also appear inside `Mod_Array` (§5.8); LC reports three shields with both.

<a id="s5-8"></a>
### 5.8 Stat modifiers (`Mod_Array`)

`Mod_Array` (92,396 items; also on 20,106 non-item records such as effects, traits and relics)
is an array of `Mod_Entry` structs. Census over all 310,497 entries in items:

| Field | Type | Entries | Meaning |
|---|---|---|---|
| `Mod_Modified` | property ID | 310,497 | The modified property (a stat) |
| `Mod_Op` | enum (0x23000046) | 310,497 | Operation: key 7 in 308,948, 5 in 724, 6 in 672, 3 in 108, 8 in 45. The mapper labels the keys (for example 7 = add, 5 = set, 6 = subtract, 8 = multiply, 3 = bitwise or). |
| `Mod_Progression` | DID → progression (class 3918 in 287,637, class 1389 in 1,724) | 289,361 | Value as a function of level |
| *the field whose PID equals `Mod_Modified`* | the type of that property | 21,136 | Constant value |
| `Mod_ProgressionFloor`, `Mod_ProgressionCeiling` | int | 19,466 / 19,401 | Level range for the entry (Reported LC) |
| `Mod_DescriptionOverride` | StringInfo | 3,596 | Tooltip text replacing the stat line (Reported LC) |

Every entry has either `Mod_Progression` or the self-named constant field (12 entries also carry
one further field whose PID is not `Mod_Modified`; `combat-data.md` §5.2 treats these as
anomalies to ignore). Readers find the constant by looking up the PID named in
`Mod_Modified` among the struct's fields. Operators, level ranges (inclusive at both ends) and
the evaluation of one entry are specified in `combat-data.md` §5.3–§5.6.

> **OPEN (F21):** Which level governs an item's entries. LC uses the item level; no stored
> item value depends on it, so the data cannot confirm it. How entries from several sources
> combine on the wearer is `combat-data.md` §5.3's OPEN. Progressions are evaluated per
> `combat-data.md` §4.10; §9.5 confirms that evaluation against the stored armour values.

<a id="s5-9"></a>
### 5.9 Effects and granted abilities

Four arrays name effects. Each element is an `EffectGenerator_EffectStruct`:

| Field | Type | Observed | Meaning |
|---|---|---|---|
| `EffectGenerator_EffectID` | DID → effect record (0x70, `combat-data.md` §6) | every struct | The effect |
| `EffectGenerator_EffectSpellcraft` | float | −1.0 in 25,846 of the 26,659 structs of the first three lists; positive in 812; 0.0 in 1 | Effect strength (LC calls it spellcraft); what −1 means is OPEN |
| `EffectGenerator_EffectSpellcraftFloor`, `…Ceiling` | int | in 2,876 structs; (−1, −1) in 2,786 | Range (OPEN) |
| `EffectGenerator_EffectDataList` | array | in 9,361 usage structs | Arguments for the effect: `Effect_Crafting_Recipe` (6,055), `Effect_GrantTrait_Trait_To_Grant` (1,385), `Effect_GrantSkill_Skill_To_Grant` (1,263), `Effect_GrantTraitRank_Grant_List` (587), `Effect_Crafting_Profession` (71), `Effect_Crafting_RecipeList` (71) |

| List | Items | Elements | Element count per item | Main target classes |
|---|---|---|---|---|
| `EffectGenerator_UsageEffectList` | 13,480 | 14,599 | 1 (12,631) to 7 | 770 (6,055, recipe learning), 722, 734, 721, 5724, … |
| `EffectGenerator_EquipperEffectList` | 8,568 | 8,925 | 1 (8,267) to 15 | 718 (5,173), 734 (2,772), 736, 3686, 3124 |
| `EffectGenerator_TargetEffectList` | 3,084 (weapons) | 3,135 | 1–2 | 756, 734, 755 |
| `EffectGenerator_HotspotEffectList` | 62 | 137 | 1–4 | 718 |

Reported (LC): usage effects apply when the item is used, equipper effects while it is
equipped, target effects on the target of a weapon hit. Equipper and target lists occur only
on items; usage lists also on 90 non-item records (classes 794, 1724, 3826) and hotspot lists
on 2,246 records of class 1170.

Related labels: `Item_GrantedSkill` (1,262, → skill class 827), `Item_GrantedTrait` (1,557, →
trait classes 1477, 3509, 2525, 1483), `Usage_SkillToExecute` + `Usage_SkillLevel` (1,194 /
1,098), `Effect_IsEnchantable` (149,611; true 119,174; OPEN).

<a id="s5-10"></a>
### 5.10 Usage: behaviour objects and requirements

**Behaviour objects (Confirmed).** Four labels name records whose property sets are small (the
24 permission and handler targets hold at most six properties, mostly menu texts or a
mouse-over type; most are empty) and whose behaviour lives in the script code of their root
class:

| Label | Items | Distinct targets | Most common targets (records: WEENIECONTENT label where one exists) |
|---|---|---|---|
| `Usage_Action` | 151,230 | 99 | 0x7000044D (91,154, no label); 0x70000000 (29,939, `UsageAction_DoNothing`); 0x7003EF28 (15,411); 0x7000D95C (4,805); 0x700225C2 (2,988) |
| `Usage_Permission` | 149,760 | 15 | 0x7000044C (91,154); 0x70000427 (58,024, `UsagePermission_Default`); 0x7000024B (130, `UsagePermission_AlwaysAllow`) |
| `Usage_TakePermission` | 151,188 | 3 | 0x70000441 (98,545); 0x70000426 (42,441); 0x7000024C (10,202, `TakePermission_CantTake`) |
| `CSM_ActionHandler` | 105,092 | 6 | all labelled: `EquippableActionHandler` (87,869), `DefaultActionHandler` (16,114), `ConsumableActionHandler` (553), `TextItemActionHandler` (301), `DoorActionHandler` (253), `NPCActionHandler` (2) |

The targets of `Usage_Permission`, `Usage_TakePermission` and `CSM_ActionHandler`, and 59 of
the 99 `Usage_Action` targets (named by 121,162 items), are each the only record of their root
class (for example class 1567 for 0x7000044D). The other 40 `Usage_Action` targets belong to
classes with several records: class 1556 (2 records, one of them `UsageAction_DoNothing`,
named by 30,029 items), 3904 (30 records, 30 items) and 3859 (8 records, 9 items).
WEENIECONTENT labels are read at run time (`property-system.md` §8.1); unlabelled targets are
identified by DID or class. The same 91,154 items (the equippable ones) use 0x7000044D and
0x7000044C.

> **Decision (Tessera sandbox server):** dispatch "use" by the `Usage_Action` target DID with a
> table of Tessera's own behaviours, and treat unknown targets as "do nothing".

**Requirements and limits** (authored; semantics Reported LC unless marked):

| Label | Type | Items | Notes |
|---|---|---|---|
| `Usage_RequiredClassList` | array of `Usage_ClassType` (enum 0x2300000E) | 20,000 | 41 distinct lists |
| `Usage_RequiredRaces` | array of `Usage_RequiredRaceEntry` | 322 | |
| `Usage_RequiredFaction` | struct (`Usage_RequiredFaction_DataID` → class 1889, `Usage_RequiredFaction_Tier`) | 4,569 | |
| `Usage_RequiredTrait`, `Usage_RequiredTrait_Array` (struct `Usage_RequiredTrait` + `Usage_RequiredVirtueRank`) | DID / array | 2,577 / 1,024 | |
| `Usage_BarringTrait`, `Usage_BarringSkill`, `Usage_BarringCraftRecipe`, `Usage_BarringEmote` (and `_Array` forms) | DID | 1,647 / 1,261 / 6,055 / 96 | Barring condition (label) |
| `Usage_MinGloryRank` | int | 1,084 | |
| `Usage_RequiredCraftProfession` (→ class 1023), `Usage_RequiredCraftTier`, `Usage_RequiredCraftMastery` | DID / enum | 6,152 / 6,126 / 149 | |
| `Usage_QuestRequirements` | array of (`Usage_Operator`, `Usage_QuestID`, `Usage_QuestStatus`) | 397 | |
| `Usage_RestrictedEffect`, `Usage_RequiredEffect` | DID → effect | 5,522 / 236 | |
| `Usage_CooldownDuration` | enum (0x2300006D) | 7,229 | 46 keys; seconds from the cooldown control (§9.8) |
| `Usage_CooldownChannel` | enum (0x2300006C) | 5,432 | Shared-cooldown group (187 keys used) |
| `Usage_EquipCooldownDuration` | enum (0x2300006D) | 85,754 | key 0 in 85,266 |
| `Usage_DestroyOnUse` | boolean | 15,589 | true in all |
| `Usage_AllowedInCombat` | boolean | 16,156 | false in all (dictionary default true) |
| `Usage_AllowedWhileMounted` | boolean | 139,591 | true in all (dictionary default false) |
| `Usage_MinUsageDist` | float | 151,230 | 2.0 in 95.6 % (25 distinct) |
| `Usage_TargetType`, `Usage_ValidTargetTypes` | bitfield 32 | 71,389 / 15,569 | |
| `ItemDecay_Duration` | enum (0x2300030C) | 249 | Lifetime; seconds from the decay control (§9.8) |

> **OPEN (implementer, 2026-10-07):** Field labels and types missing from the requirement
> table: the fields of a `Usage_RequiredRaceEntry` (the implementation returns the raw
> elements); the type of `Usage_RequiredVirtueRank` (read as an int or an enum key); the
> types of `Usage_RequiredCraftTier` and `Usage_RequiredCraftMastery` (the row says
> "DID / enum" for three labels; both are read as enum keys); the types of `Usage_Operator`
> and `Usage_QuestStatus` (read as enum keys, as `map-data.md` §8.4.1 gives them); the
> `_Array` forms of the barring labels (not read). Please state them.

Where a label's value is the same in every item that has it but differs from the dictionary
default (the last three booleans), the **presence** of the label is the information; an
absent label stands for the dictionary default (`property-system.md` §2.2) only if the game
applies dictionary defaults to absent item properties, which is OPEN (§14; Tessera decides to
apply them).

<a id="s5-11"></a>
### 5.11 Sockets, essences and traceries

**Sockets on equipment.** `Item_Socket_Array` (9,938 items) is an array of
`Item_Socket_Array_Entry` structs with exactly two fields (19,953 entries):
`Item_Socket_Type` (bitfield 64, mapper 0x23000432 `ItemSocketType`, one bit set in every entry)
and `Item_Socket_Unlock_ILevel` (int). Sockets per item: 1 (4,835), 2 (3,083), 3 (688),
4 (1,078), 5 (46), 11 (150), 12 (58). Socket types used: `Classic` 16,155, then
`PvP_Essence`, `Essence_Mainstat`, `Legacy_Power`, `Legacy_Craft`, … (22 types).

**Socketables** (15,445 general items, `Item_Class` = `Essence`): `Item_Socket_Type` at top level
(one bit set in 15,405; zero in 40) says which socket accepts them. With it (15,416 items):
`Item_Socket_GemMinLevel` / `Item_Socket_GemMaxLevel` (int), `Item_Socket_LevelupRuneIncrement`,
`Item_Socket_Operation` (DID → class 4216 in 15,376, class 4215 in 40; each the only record of
its class, with one property `Item_Socket_DragOverlay`), `Item_TraceryUpgrade_Value` (1 in all),
`Item_TraceryUpgrade_Component_Result` (10,208 items, each naming a different item of class 796),
`Item_UniquenessChannel` and `Item_PropertySet` (DID → item set, 516 items, §7.1).

The socket control record (§6.4) maps socket types to frame images, socket types to level-up
cost properties, and minimum levels to essence overlays.

> **OPEN:** How `Item_Socket_GemMinLevel` / `GemMaxLevel` (for example 500 / 499 on the most
> common value) constrain the host item, and what `Item_Socket_Unlock_ILevel` unlocks at.

<a id="s5-12"></a>
### 5.12 Legendary and item-advancement data on items

| Label | Type | Items | Notes |
|---|---|---|---|
| `ItemAdvancement_Item` | boolean | 4,902 | true in all; marks advancing ("legendary") items |
| `ItemAdvancement_ProgressionGroupOverride` | DID → class 2139 | 4,902 | Weighted lists of legacy pools (§7.3) |
| `ItemAdvancement_StaticEffectGroupOverride`, `…Group2Override` | DID → class 2139 | 1,399 / 98 | |
| `ItemAdvancement_CombatDPSLevel` | int | 3,766 | DPS level (§9.3) |
| `ItemAdvancement_CombatPropertyModDID`, `…ModLevel`, `…ModLargeIconDID`, `…ModSmallIconDID`, `Item_RequiredCombatPropertyType` | DID → effect 734 / int / 0x41 / 0x41 / enum | 1,136 each | Non-DPS main legacy (Reported LC). Every advancing item has exactly one of `ItemAdvancement_CombatDPSLevel` (3,766) and this group (1,136) (Confirmed) |
| `ItemAdvancement_CanBeDeconstructed`, `…DeconstructionCostOverride`, `…RelicDeconstructionCostOverride` | boolean / int / int | 5,393 / 527 / 527 | |
| `ItemAdvancement_Title` | DID → class 2338 | 421 (general items) | Title-granting scrolls |
| `ItemAdvancement_RunicToCreate` | DID → class 2219 | 391 | Relic-creating items |
| `ItemAdvancement_RunicLootListOverride` | DID → class 2229 | 136 | |
| `ItemAdvancement_ReplacementLegacy`, `…AllowedLegacyToReplace`, `…LegacyType`, `…NewLegacyToAdd` | DID → effect / DID / enum (0x23000323) / DID | 405 / 405 / 404 / 7 | Legacy-replacement scrolls |
| `Item_MaxLevelUpgrades` | int | 4,905 | 3 in most |
| `Item_Reforgable`, `ItemAdvancement_ReforgeTable` (→ class 2327) | boolean / DID | 448 / 27 | |
| `Item_PILI_*` (10 labels: base weapon item, title, four relic DIDs, legacy entry list → class 4015, XP to apply, two tier counts) | | 181 each | A group of 181 general items that each describe a fully specified advancing weapon (OPEN) |

<a id="s5-13"></a>
### 5.13 Kind-specific groups

| Kind | Labels (items) | Notes |
|---|---|---|
| Recipe scrolls (805) | `RecipeItem_Recipe` (6,055 → recipe class 1024), `Usage_BarringCraftRecipe` (6,055, same target), usage effect with `Effect_Crafting_Recipe` | The recipe is named three times (`RecipeItem_Recipe`, `Usage_BarringCraftRecipe`, the `Effect_Crafting_Recipe` argument of the single usage effect); all three agree in all 6,055 |
| Multi-recipe scrolls (3663) | usage effect data `Effect_Crafting_RecipeList`, `Effect_Crafting_Profession` (71 each) | |
| Packages (2814) | `PackageItem_TrophyListTemplate` (3,634; → loot list class 1237 in 3,390, null in 243), `PackageItem_FilteredTrophyTableTemplate` (3,582; → class 3666 in 275), `PackageItem_Freep_WeightedTreasureTableID` / `…Creep_…` (→ class 3307), `PackageItem_PreviewList` (429 items, 1,366 preview item DIDs), `PackageItem_BindAllItemsToAccount` / `…ToCharacter`, `PackageItem_LockBox` and lock IDs, `PackageItem_UseCachedMungeLevel`, `PackageItem_UsePlayerAsContainerForMunging` | Contents come from loot tables (loot family) |
| Text items (798) | `TextItem_Title`, `TextItem_PrefabContentList`, `TextItem_AuthorName` (301 / 301 / 284) | |
| Doors (1722), interactive objects (2097) | `Usage_PlayersCanOpen`, `Usage_PlayersCanClose`, `Door_*` (animation scriptlets 0x0C), `BigBattle_Interactive_*`, `MultiUsage_ActivityCollection` | World fixtures |
| Travel points (815), milestones (804) | `PortalDestination`, `Usage_LayerArray`, `Waypoint_Cost`, `Milestone_TravelLink`, `Travel_TravelFlags` | World fixtures |
| Keys (803) | `Key_CanBePlacedOnKeyRing`, `Usage_KeyID` (enum, 81 items) | |
| Instruments (796) | `Usage_IsMusicalInstrument`, `Item_Music_Instrument_Type` (enum 0x230000FC), `Item_Music_Channeling_State` (→ class 961) (1,077 each) | |
| Crafting tools and trinkets (796) | `Item_CraftToolCategory` (478), `CraftTrinket_Profession` (→ class 1023), `CraftTrinket_Tier` (1,248) | |
| Decorations (796) | `Item_Decoration_Category` (5,047), `Item_Decoration_PropertyHook_Name` / `_Value` (635) | Housing |
| Paper items (796) | `PaperItem_Category` (610), `PaperItem_SmallIcon` (674), `PaperItemControl_CurrencyBonusPropertyArray` (697) | Wallet currencies, listed by the paper-item control (§6.4) |
| Reputation items (796) | `Reputation_IsReputationItem`, `Reputation_Faction` (→ class 1889), `Reputation_ReputationGain` (int64) (268–269) | |
| Mounts (796) | `Mount_SkillToGrantTall`, `Mount_SkillToGrantShort` (377), `Mount_Type` (52) | |
| Hobby items (796) | `Hobby_*` (552) | |

---

<a id="s6"></a>
## 6. Shared lookup and control records

All of these are ordinary 0x70 records with property sets. Control records are found through
their WEENIECONTENT label (`property-system.md` §8.1); the DIDs are this install's and must not
be hard-coded.

<a id="s6-1"></a>
### 6.1 Value tables (class 1688, 99 records)

Each has exactly two properties:

| Label | Type | Content |
|---|---|---|
| `Item_BaseValArray` | array of `Item_BaseValStruct` { `Item_BaseVal`: float } | 601 elements in every table; element *k* (1-based) is the base value at item level *k* |
| `Item_QualityArray` | array of `Item_QualityStruct` { `Item_Quality`: enum, `Item_QualityModVal`: float } | 5 elements, one per quality key 1–5 (every table) |

Referenced by `Item_ValueLookupTable` of 145,477 items (+41 carry-alls).

<a id="s6-2"></a>
### 6.2 DPS tables (class 1660, 8 records)

| Label | Type | Content |
|---|---|---|
| `Combat_BaseDPSArray` | array of `Combat_BaseDPS` (float) | 701 elements; element *k* is the base DPS at level *k* |
| `Combat_QualityModArray` | array of `Combat_QualityModData` { `Combat_Quality`: enum, `Combat_DPSMod`: float } | 5 elements, quality keys 1–5 |

Six of the eight are used by weapons (`Combat_DPS_LUT`); the other two only by advancement
widgets (`ItemAdvancement_AdvanceableWidget_DPSLUT`, class 4024).

<a id="s6-3"></a>
### 6.3 Weapon timing and wear control

- **Weapon timing tables** (class 1691, 24 records), listed by the record labelled
  `WeaponActionDurationTable` (0x70000254 here, class 1692, property
  `Item_WeaponActionDurationTable`, an array of DIDs). Each table has `Item_EquipmentCategory`
  (one category) and `Item_WeaponActionDurationDataArray`: entries { `Item_WeaponSpeed`: enum
  0x23000066, `Item_BaseActionDuration`: float, `Item_BaseAnimDurationMultiplierMod`: float },
  one per speed key 1–6. All 144 entries are (1.0, −0.1) on this install.
- **Wear control** (label `ItemWearControl`, 0x70000224 here, class 1672):

| Label | Entries | Entry fields |
|---|---|---|
| `ItemWearControl_MaxStructurePointsArray` | 50 | `…_MaxStructurePointsEnum` (key of 0x230000F9), `…_MaxStructurePointsValue` (int) |
| `ItemWearControl_DurabilityArray` | 7 | `…_DurabilityEnum` (key of 0x230000FA), `…_DurabilityValue` (float) |
| `ItemWearControl_RepairCostQualityModArray` | 5 | quality key, float factor |
| `ItemWearControl_ItemWearStateValueModArray` | 4 | wear-state key (0x230000FB), float factor (1.0 for the key every item has) |
| `ItemWearControl_SkillGateArray` | 5 | key, float |

> **OPEN:** The roles of the repair-cost, wear-state and skill-gate tables. Only their keys are
> known (quality, wear state, an unnamed key); no stored item value depends on them.

> **OPEN (implementer, 2026-10-07):** The entry field labels are abbreviated ("`…_MaxStructurePointsEnum`")
> or not given. The implementation reads `ItemWearControl_MaxStructurePointsEnum`,
> `ItemWearControl_MaxStructurePointsValue`, `ItemWearControl_DurabilityEnum` and
> `ItemWearControl_DurabilityValue` (all four are defined in the dictionary and reproduce
> §9.6 on every item). The entries of the repair-cost, wear-state and skill-gate arrays are
> read by field type (first enum field = key, first float or int field = value), because
> their labels are not given. Please spell out every entry label and value type.

<a id="s6-4"></a>
### 6.4 Other control records

| WEENIECONTENT label | DID here (class) | Item-relevant content |
|---|---|---|
| `InventoryControl` | 0x70000230 (1202) | Slot counts and limits; `InventoryControl_DistributionToLevelOffsetList` (15 entries: distribution key → level offset, all 0 here); `…_DefaultEquipperEffectList` (13 equipment categories → effect lists) and `…_DefaultTargetEffectList` (9 categories); `…_EquipmentCategoryCachedSlotList` (3 slots → a cached-category property); `…_SlotToPaperdollLocation_MappingList`; `…_ExamProperty_Array` (38 property IDs: the per-instance properties of an item, for example `Inventory_BoundToID`, `Item_CurStructurePoints`); `…_MergeProperty_Array` (6 property IDs including `Item_Value` and `Item_Level`); `InventoryControl_SmartSortInfo_Array` (8 entries: a sort type and a list of property IDs) |
| `CooldownControl` | 0x70000264 (1664) | `CooldownControl_DurationMapList`: 63 entries (cooldown key → seconds, float) |
| `ItemDecayControl` | 0x70021EB3 (2809) | `ItemDecayControl_DurationMapList`: 20 entries (decay key → seconds) |
| `ItemMungingControl` | 0x70000252 (1671) | Dye colour and pattern lookup lists (`appearance.md` §4.9) |
| `ItemSocketControl` | 0x70040DEE (4005) | Socket images per type, level-up cost property per socket type, essence overlays by level |
| `ItemAdvancementControl` | 0x7000EAA6 (2152) | 42 game-wide legendary settings (§7.3) |
| `GameSetDirectory` | 0x70009869 (1827) | `Set_Directory_GameSetList`: all 2,224 set DIDs (§7.1) |
| `RelicTransmutationDirectory` | 0x70025633 (3139) | 506 transmutation recipes (§7.2) |
| `PaperItemControl` | 0x70024867 (3105) | Paper-item lists (443 + 167 items), version upgrades (84 old/new pairs), services |
| `CosmeticPetDirectory` | 0x7004832E (4091) | Cosmetic companions (§7.4) |
| `WardrobeControl` | 0x7001BE4C (2639) | Wardrobe (appearance family) |

> **OPEN:** What `InventoryControl_ExamProperty_Array` and `…_MergeProperty_Array` are used for
> (no behaviour that uses them was observed).

> **OPEN (implementer, 2026-10-07):** The entry layout of `CooldownControl_DurationMapList` and
> `ItemDecayControl_DurationMapList` is not given (labels of the key and seconds fields, and
> the type of the decay seconds). The implementation reads each entry struct by field type:
> first enum field = key, first float (or int) field = seconds. This reproduces §9.8 coverage
> on every item. Please state the labels and types.

---

<a id="s7"></a>
## 7. Auxiliary item families (ancestor 486)

These records are not items (their root classes descend from 486 → 89 → 485 → 484, not from
1733) but exist only to serve items.

<a id="s7-1"></a>
### 7.1 Item sets (class 1824, 2,217 records)

Class 1824's parent is 1817, which also roots **class 2078** (7 records): those use the same
labels, but their members are traits (class 1477), not items. Both are listed in the
`GameSetDirectory` record (2,217 + 7 = 2,224 DIDs).

| Label | Type | Records | Observed | Meaning |
|---|---|---|---|---|
| `Set_Name`, `Set_Description` | StringInfo | 2,217 | | |
| `Set_Icon` | DID → 0x41 | 2,217 | one surface in all | |
| `Set_Level` | int | 2,217 | 65 distinct | Set level (Reported LC: level used for bonus progressions) |
| `Set_LevelRequired` | **boolean** | 2,217 | true in all | OPEN (LC reads it as an integer; it is a boolean) |
| `Set_MaxPlayerLevel` | int | 2,217 | 52 distinct | Maximum character level for the bonuses (Reported LC) |
| `ItemGameSet_UseAverageItemLevelForSetLevel` | boolean | 100 | true | OPEN (name only) |
| `Set_MemberList` | array of `Set_MemberDID` (DID → item) | 2,191 | 2–18 members; 6 in 1,184 sets | Members |
| `Set_ActiveCountDataList` | array of `Set_ActiveCountData` | 2,217 | 1–7 entries | Bonus tiers |
| `Set_MinActiveMembers`, `Set_MaxActiveMembers`, `Set_ActiveCountProperty`, `Set_ActiveLevelProperty` | int / int / property ID / property ID | 26 | | Sets without a member list (below) |

`Set_ActiveCountData` fields: `Set_ActiveCount` (int; the lists of all 2,224 item and trait
sets are strictly ascending), `Mod_Array` (3,929 tiers; §5.8), `Set_EffectDataList` (1,449; an array of
`EffectGenerator_EffectStruct`, §5.9), `Set_StartScriptID` (1,746; enum 0x23000097),
`Set_ShortDescriptionForCount` (21).

**Membership (Confirmed).** 10,607 distinct items are named by the member lists of the item
sets; no item is in two lists; every member is an item. The 26 sets without a member list are named instead by
the `Item_PropertySet` of 516 socketables (22 of the 26 sets, 4 to 44 socketables each); the
other 4 are named by nothing. So **the members of a set are the items in its member list plus
the items whose `Item_PropertySet` names it.** A reverse index (item → set) must be built by
scanning; the member items themselves do not name their set.

Nine sets have two members but tiers up to 5 (`Set_ActiveCount` 2–5).

> **OPEN:** The bonus rule. Reported (LC): a tier is active when the number of equipped members
> is at least its `Set_ActiveCount`, and its modifiers are evaluated at `Set_Level`. How the
> nine two-member sets reach counts above 2, and what the 26 member-less sets count (each has a
> `Set_ActiveCountProperty` and a `Set_ActiveLevelProperty`, property IDs whose role is not shown), are not shown by
> the data.

<a id="s7-2"></a>
### 7.2 Relics and transmutation

- **Relics** (class 2219, 706 records): `Runic_Name`, `Runic_Type` (bitfield, 6 values),
  `Runic_Level`, `Runic_Tier`, `Runic_RequiredItemLevel` (423), `Relic_ValidContainerSlots`,
  `Mod_Array` (all), `Runic_Deconstruction_Value` and `…_TrophyListTemplate`, icons and usage
  labels. They carry icon layers and `Usage_*` labels like items but are not in the item
  subtree. Reached from items (`ItemAdvancement_RunicToCreate` 391, `Item_PILI_Runic_Type1–4`
  181 each), from runic lists (469), runic treasure groups (class 2239, 450) and transmutation
  outputs (343).
- **Transmutation recipes** (class 3138, 506 records): `Relic_Transmutation_Recipe_RelicCurrencyCost`,
  `…_TransmutationCategory`, `…_Output_List` (outputs: a relic or an item, with a weight).
  Listed by `RelicTransmutationDirectory`.
- **Runic lists** (class 2229, 169 records): one property, `RunicList_Array` of
  { `RunicList_DropFrequency`: enum, `RunicList_RunicOrTreasureGroup`: DID → relic or class
  2239 }. Named by runic level loot tables (class 2227, 1,050), by items (136) and by the runic
  combination control. Their use in loot is specified in `economy.md` §4.9.

> **OPEN (implementer, 2026-10-07):** The value types of `Runic_Tier` (§7.2) and
> `ItemAdvancement_Mod_Weight` (§7.3) are not given. The implementation accepts an int or an
> enum key for the tier and any numeric value for the weight. Please state them.

<a id="s7-3"></a>
### 7.3 Legacies and advancement tables

| Class | Records | Content | Named by |
|---|---|---|---|
| 2134 | 359 | `ItemAdvancement_Effect_Array`: { `ItemAdvancement_Effect` (→ effect 734), `ItemAdvancement_Mod_Weight` } | 2139 (1,519) |
| 2139 | 224 | `ItemAdvancement_ProgressionListArray` { → 2134, weight } and `ItemAdvancement_ProgressionPullCountList` { amount, weight } (4 entries in all) | items (6,399 via the three override labels), reforge tables (class 2204, 350) |
| 2338 | 421 | Legendary titles: `Name`, `ItemAdvancement_IATitle_Category`, `…_Tier`, `ItemAdvancement_Title_SlayerGenusType`, `…_DamageType`; an effect list in 344 and a `Mod_Array` in 105 | items (421 + 181) |
| 4015 | 329 | Imbued legacies: `ItemAdvancement_ImbuedLegacy_Effect` (→ effect), initial and absolute max levels (46 and 83 in all), icon, tooltip layout, `…_ClassicLegacyTransform` (struct) | `Item_PILI_Legacy_Entry_List` (1,267), the advancement control (333) |
| 2152 | 1 | The advancement control (label `ItemAdvancementControl`): 42 settings, including level tables, reforge levels, legacy name lists, relic and XP currencies (items) | WEENIECONTENT |

> **OPEN:** The legendary rules (legacy rolls by weight, pull counts, imbuement, reforging).
> The tables are fully decodable with `property-system.md`; their use is server behaviour and
> is not specified here.

> **OPEN (implementer, 2026-10-07):** The records of classes 2139, 2338, 4015 (§7.3) and 3138
> (§7.2) are not given typed readers because their labels are abbreviated or missing:
> the field labels and types of the `ItemAdvancement_ProgressionListArray` entries
> ("{ → 2134, weight }") and of the `ItemAdvancement_ProgressionPullCountList` entries
> ("{ amount, weight }"); the full labels written "`…_Tier`" and "`…_DamageType`" (2338),
> "initial and absolute max levels", "icon", "tooltip layout" and "`…_ClassicLegacyTransform`"
> (4015), "`…_TransmutationCategory`", "`…_Output_List`" and the output entry fields (3138).
> They stay reachable through the generic property access. Please spell out every label and
> its type.

<a id="s7-4"></a>
### 7.4 Cosmetic companions (class 3698, 563 records)

Each has `CosmeticEntity_CosmeticEntity` (DID → the creature shown), `CosmeticEntity_Name_Initial`,
distance, movement type, following speed, movement thresholds and `Item_Quality` (562, all key 5).
They are reached from effects (`Mod_Array` → `CosmeticEntity_CosmeticPet_ForwardSource` in 562
effect records of class 734) and from the `CosmeticPetDirectory`, not from items directly; an
item summons a companion through its usage effect.

---

<a id="s8"></a>
## 8. The DID graph

<a id="s8-1"></a>
### 8.1 Out of items (Confirmed, all 151,230 property sets, nested values included)

3,607,800 non-null DID values; every one names an existing record (F18).

| Target prefix | References | Main properties (references) |
|---|---|---|
| 0x70 | 1,505,975 | `Mod_Progression` (289,361), `Usage_Action` (151,230), `Usage_Action_Shadow` (151,230), `Usage_TakePermission` (151,188), `Item_ValueLookupTable` (145,477), `Usage_Permission` (149,760), `CSM_ActionHandler` (105,092), `Item_RepairCostPerPointProgression` (87,020), `Item_Armor_Value_Lookup_Table` (60,916), `EffectGenerator_EffectID` (26,796), `Item_Disenchant_Component_Result` (26,345), `Combat_DPS_LUT` (23,204), `ItemMunging_ItemLevelOverrideProgression` (20,798), `Induction_ActionDID` (17,344), `Item_Socket_Operation` (15,416), `Item_TraceryUpgrade_Component_Result` (10,208), and about 70 smaller ones (§5) |
| 0x20 | 773,250 | `Item_WornAppearance` (634,655), `ExternalAppearanceID` (138,462), `Appearance_AprFile` (133) |
| 0x41 | 524,175 | icon layers (`icon-layers.md`), `ItemAdvancement_CombatPropertyMod*IconDID`, `PaperItem_SmallIcon`, `Hobby_Reward_Icon*`, `Item_Popup_Image` |
| 0x22 | 406,133 | the three tooltip layouts (§5.1) |
| 0x07 | 235,387 | `ScriptTable_DBScriptTable` inside `Entity_AdditionalScriptTables` (46 more are null) |
| 0x47 | 151,230 | `PhysObj` |
| 0x2A | 11,618 | `Usage_Sound` (11,148), `Hobby_Birding_BirdCall` (470) |
| 0x0C | 32 | `Door_Animation_*` |

Null DIDs are frequent and meaningful as "none" (for example `Induction_ActionDID` 107,732 nulls,
`Icon_Layer_UnderlayDID` 26,092, `Item_ValueLookupTable` 5,087).

Item-to-item references: `Item_Disenchant_Component_Result` (26,345), `Item_TraceryUpgrade_Component_Result`
(10,208), `PackageItem_PreviewItem` (1,366), `Item_PILI_Base_Weapon_WC` (181).

<a id="s8-2"></a>
### 8.2 Into items (Confirmed, all 383,332 property sets)

129,560 distinct items are named by some record. Largest sources (references; source root class):

| Source property path | Source class | References | Target kinds |
|---|---|---|---|
| `TreasureGroupProfile_ItemTable` → `…_Item` | 1235 (treasure groups) | 116,706 (class 1235 only; another 58 come from class 2372, 116,764 in all, §2.4) | 795, 802, 796, 799, 2814 |
| `Barter_ItemListArray` → `Barter_Item` | 1943 (barter lists) | 100,761 | 796, 795, 802, 805, 799 |
| `Vendor_ItemList` | 1724 (NPCs) | 91,274 | 796, 805, 795, 799, 2814 |
| `LootGen_TrophyList` → `…_ItemOrProfile` | 1237 (loot lists) | 55,074 | 796, 795, 802, 2814, 799 |
| `CraftRecipe_IngredientList`, `…_OptionalIngredientList`, `…_ResultItem`, `…_CriticalResultItem`, `…_MultiOutputArray`, `…_IngredientPack` | 1024 (recipes) | 58,493 in all | 796 mostly; results also 795, 799, 802, 797 |
| `VendorInventory_Items` | 1619 | 14,014 | |
| `Quest_ObjectiveArray` (several nested paths), `Quest_LootTable`, `Quest_GiveItemArray`, `Quest_RoleArray`, `Quest_BestowalRoles`, `Quest_PaperItemAwards` | 1398 (quests and deeds) | 24,548 | 796, 798 mostly |
| `Set_MemberList` | 1824 | 10,607 | 795, 802, 799, 796, 797 |
| `QuestTreasure_FixedItemArray`, `…_SelectableItemArray` | 1406 | 14,537 | |
| `HobbyRewardProfile_ItemArray` | 2153 | 2,689 | |
| `RewardTrack_Array` → `…_Reward` | 4224 | 2,466 | 2814 |
| `PrivateEncounterTemplate_PaperItemAwardArray` | 2651 | 1,823 | |
| `WebStoreInventoryItem_DataID` | 2541, 3054, 3052, 2532 | 1,335 | |
| `AdvTable_StartingInventory_List` | 3335, 853 | 327 | |

About 60 further source paths name items fewer than 600 times each (generators, monster play,
mail effects, housing control, guild control, paper-item control, skill material components,
legendary control, starting inventories). The loot, crafting, vendor and barter sources are
specified in `economy.md` §4–§6 (where loot tables are attached: §4.13); the quest sources in
`quests.md` §3.2.

---

<a id="s9"></a>
## 9. Algorithms

All arithmetic below is in IEEE-754 single precision unless a step says otherwise; tables are
read at run time.

<a id="s9-1"></a>
### 9.1 Classify a record

1. Read the root class (object-state offset 4).
2. Walk its ancestors (`wlib.md` §9, stop after the class count). If *I* (§2.4) is met, the
   record is an item; if only the parent of *I* is met, it is item-like.
3. Load the property set (+0x09000000). Kind-specific code keys off §2.1's signatures.

<a id="s9-2"></a>
### 9.2 Item value (Confirmed, F11)

Inputs: `Item_ValueLookupTable` *T*, `Item_Level` *L*, `Item_Quality` *Q*.

1. If *T* is absent or null, the value is 0. (All 5,087 null-table items store 0; the 666 items
   without the label store no value.)
2. Let *b* = the `Item_BaseVal` of element *L* of *T*'s `Item_BaseValArray` (1-based). *L* is
   between 1 and the array length (601) in all items.
3. Let *m* = the `Item_QualityModVal` of the `Item_QualityArray` element whose `Item_Quality`
   is *Q*. Every table has keys 1–5 and every item with a table has one of them.
4. *x* = *b* × *m*, computed as a single-precision product.
5. The value is *x* rounded to the nearest integer, **halves rounded up** (for example 6.5 → 7,
   687.5 → 688; 1,353 records have an exact half and all 1,353 round up).

Single precision matters: if the product is computed in double precision from the same
single-precision inputs and rounded half up, 533 items get a different integer, and the stored
value matches the single-precision result in all 533. Double precision with halves rounded to
even fails in 624 items. (An earlier draft gave 93 here; that count did not reproduce and is
withdrawn.)

<a id="s9-3"></a>
### 9.3 Weapon DPS (Confirmed, F12)

Inputs: `Combat_DPS_LUT` *T*, level *L* = `ItemAdvancement_CombatDPSLevel` if present
(3,766 weapons) else `Item_Level`, quality *Q*.

1. *b* = element *L* (1-based) of *T*'s `Combat_BaseDPSArray`.
2. *m* = `Combat_DPSMod` of the `Combat_QualityModArray` element whose `Combat_Quality` is *Q*.
3. DPS = *b* × *m*, computed as a single-precision product. Stored `Combat_BaseDPS` equals
   this **exactly** in all 23,204 weapons. A double-precision product (not rounded to single
   precision) is exact in only 1,115, so a reader must round the product to single precision
   before comparing or storing.

Using `Item_Level` for the 3,766 weapons with a DPS level gives wrong values in all of them.

<a id="s9-4"></a>
### 9.4 Weapon damage (Confirmed relation, F13)

With DPS *d*, `Item_BaseActionDuration` *t* and `Combat_DamageVariance` *v*:

- stored `Combat_Damage` = *d* × *t* × 2 ÷ (2 − *v*) (within 1.2 × 10⁻⁷ relative, all 23,204);
- Reported (LC): this is the **maximum** damage per hit; the minimum is maximum × (1 − *v*).
  Then the mean of minimum and maximum is *d* × *t*: the damage per attack implied by the DPS.

<a id="s9-5"></a>
### 9.5 Armour value (Confirmed, F14)

Inputs: `Item_Armor_Value_Lookup_Table` *P* (class 3918 record) and `Item_Level` *L*.

1. Read `LinearInterpolatingProgression_Array`: control points
   { `LinearInterpolatingProgression_Key`: int, `LinearInterpolatingProgression_Value`: float },
   keys strictly ascending (1,814 of 1,814 records; 2 to 22 points).
2. Evaluate the progression at *L* as `combat-data.md` §4.4 and §4.10 describe (single
   precision, stored-slope form). Inside the keys this is linear interpolation between the two
   neighbouring points.
3. Truncate toward zero. All 60,880 stored values match, whether step 2 is computed in single
   precision with the slope form or in double precision (the values are non-negative, so
   truncation equals floor; rounding to nearest would fail in 18,324 items).

Among the 60,880 items checked above, the item level lies inside the keys (first key ≤ *L* ≤
last key) in 60,211, above the last key in 669 and below the first key in none. (Counting all
60,916 items that have a table, including the 36 without a stored armour value, gives 60,247
inside and the same 669 above.) In all 669 items above the last key the last segment is
flat, so clamping and extrapolation give the same value and the
data does not settle `combat-data.md` §4.8's OPEN.
Other uses of class-3918 progressions (`Mod_Progression`) are not confirmed to truncate
(§5.8 OPEN).

<a id="s9-6"></a>
### 9.6 Durability and structure points (Confirmed, F15)

- `Item_MaxStructurePoints` = the `…_MaxStructurePointsValue` of the wear-control entry whose
  enum key equals `Item_MaxStructurePointsEnum` (87,020 of 87,020).
- `Item_DurabilityValue` = the `…_DurabilityValue` of the entry whose key equals
  `Item_DurabilityEnum` (90,932 of 90,932).
- A new item instance starts with current structure points = maximum (the static copy, §4.3).

<a id="s9-7"></a>
### 9.7 Weapon timing (Confirmed, F16)

Find the timing table (§6.3) whose `Item_EquipmentCategory` equals the weapon's; in it, the
entry whose `Item_WeaponSpeed` equals `Combat_WeaponSpeed`. Its duration and animation modifier
equal the weapon's stored `Item_BaseActionDuration` and `Item_BaseAnimDurationMultiplierMod`
(23,204 of 23,204).

<a id="s9-8"></a>
### 9.8 Cooldown and lifetime in seconds (Confirmed coverage, F17)

Every key of `Usage_CooldownDuration` (7,229 items) and `Usage_EquipCooldownDuration`
(85,754; key 0 = none) used by an item is either 0 or a row of the cooldown control map;
every `ItemDecay_Duration` key used (249) is 0 or a row of the decay control map. The
duration is the row's float, in seconds (unit Reported: the enum labels name the same spans,
for example a key labelled one second maps to 1.0).

<a id="s9-9"></a>
### 9.9 Set membership (Confirmed)

1. Read the `GameSetDirectory` list.
2. For each set of the item-set class (1824 here; not the trait-set sibling), record its
   `Set_MemberList` DIDs.
3. Add every item whose `Item_PropertySet` names the set.
4. The reverse index item → set is a function: no item belongs to two sets.

---

<a id="s10"></a>
## 10. Explorer item card

The item card shows one item record. Rows appear only when their sources exist.

| Row | Source | Notes |
|---|---|---|
| Header | `Name` (StringInfo, `string-table.md` §9); composed icon (`icon-layers.md` §3); DID, root class, kind label (§2.1) | |
| Category | `Item_Class` display text (mapper 0x23000036 display list, `property-system.md` §7.1); `Item_EquipmentCategory` names | |
| Quality | `Item_Quality` display text | No colour is derived (`icon-layers.md` §4) |
| Level | `Item_Level`; `Usage_MinLevel`–`Usage_MaxLevel`; munging fields raw (§5.2) | |
| Binding and limits | §5.4 table reading; `Inventory_Unique`, `Inventory_UniqueEquipped`; stack size `Inventory_MaxQuantity` | |
| Value | `Item_Value` as gold/silver/copper (F28) and the recomputed value (§9.2), flagged if they differ | |
| Durability | `Item_CurStructurePoints` / `Item_MaxStructurePoints`; `Item_DurabilityEnum` display text | |
| Armour | `Item_Armor_Value` (and recomputed, §9.5) | |
| Weapon | DPS (§9.3), damage range (§9.4, marked Reported), `Combat_DamageType` display text, speed | |
| Stats | Each `Mod_Entry`: modified property label, operation label, constant value or progression DID (evaluation marked OPEN, §5.8) | |
| Effects | Each effect struct of the four lists: list name, effect DID and the effect record's `Effect_Name` | `combat-data.md` §6 |
| Sockets | `Item_Socket_Array` types and unlock levels; for socketables, the accepted socket type and set | |
| Set | Set name (via §9.9), tier counts, member count | |
| Legendary | §5.12 labels present | |
| Description | `Description` | |
| Visuals | worn/held preview (`appearance.md` §8) | |
| Links | Every outbound DID of §8.1 (clickable) and the inbound references of §8.2 (a reverse index built once per session) | |
| Raw | Every property, grouped as in §4 (stamps and copies collapsed by default) | |

---

<a id="s11"></a>
## 11. Enum mappers used by item properties (Confirmed)

Labels in the third column are the EMAPPER DID-map labels (`property-system.md` §8.1). Mapper
DIDs are this install's; resolve them through each property definition's `data` field.

| Property | Mapper (here) | EMAPPER label | Names / display texts |
|---|---|---|---|
| `Item_Class` | 0x23000036 | `ItemClass` | 279 / 278 |
| `Item_EquipmentCategory` (bitfield) | 0x2300004C | `EquipmentCategory` | 42 / 41 |
| `Item_Quality` | 0x23000067 | `ItemQualities` | 6 / 5 (key 0 has no text) |
| `Item_Socket_Type` (bitfield) | 0x23000432 | `ItemSocketType` | 25 / 24 |
| `Combat_WeaponSpeed`, `Item_WeaponSpeed` | 0x23000066 | `WeaponSpeedType` | 7 / 0 |
| `Combat_DamageType` | 0x23000028 | `DamageType` | 19 / 15 |
| `WeenieType` | 0x230000C9 (base 0x230001AB) | `WeenieType` | 38 own / 0 |
| `Inventory_CompatibleSlot`, `_DefaultSlot`, `_PrecludedSlot` (bitfields) | 0x230000EE (base 0x23000101) | `ContainerSlot` | 30 own / 21 |
| `Item_DistributionType` | 0x2300028C | `ItemDistributionType` | 16 / 0 |
| `Item_DurabilityEnum` | 0x230000FA | | 8 / 7 |
| `Item_MaxStructurePointsEnum` | 0x230000F9 | | 51 / 0 |
| `Item_WearState` | 0x230000FB | | 5 / 0 |
| `Usage_CooldownDuration` | 0x2300006D | `CooldownDuration` | 64 / 0 |
| `Usage_CooldownChannel` | 0x2300006C | `CooldownChannel` | |
| `Mod_Op` | 0x23000046 | | 12 / 0 |
| `Auction_CategoryType` | 0x230001D1 | | 53 / 52 |
| `Item_Client_Sort_Type` | 0x23000317 | | 9 / 0 |
| `Item_MaterialType` | 0x230000F3 | | 54 / 0 |

The EMAPPER label `ClothingClassType` names mapper 0x23000015 (6 names), but **no property
definition uses it**; it plays no part in items on this install.

---

<a id="s12"></a>
## 12. Invariants (all Confirmed on this install)

| # | Invariant |
|---|---|
| IT1 | Every record whose root class descends from *I* has a property set that decodes, and that set has `Item_Class`, `WeenieType`, `Name`, `PhysObj`, `Usage_Action` and `Icon_Layer_BackgroundDID`. |
| IT2 | The sixteen labels of §4.2 are present, with one value each, in exactly the records under ancestor 1733. |
| IT3 | Each copy of §4.3 equals its source. |
| IT4 | Non-null `Item_ValueLookupTable` ⇒ 1 ≤ `Item_Level` ≤ length of the base array, the table has the item's quality key, and `Item_Value` follows §9.2. Null table ⇒ `Item_Value` = 0. |
| IT5 | Every weapon has `Combat_DPS_LUT`; §9.3 holds exactly (single-precision product equals the stored `Combat_BaseDPS` bit for bit), and §9.4 holds within 3 × 10⁻⁷ relative. |
| IT6 | `Item_Armor_Value` with a lookup table follows §9.5 exactly. |
| IT7 | §9.6 and §9.7 hold exactly. |
| IT8 | Every non-null DID in an item property set names an existing record. |
| IT9 | Every `Mod_Entry` has `Mod_Modified` and `Mod_Op`, and has `Mod_Progression` or a field whose PID equals `Mod_Modified`. |
| IT10 | `Item_Socket_Array` entries have exactly the two fields of §5.11 and one socket-type bit. |
| IT11 | `Set_ActiveCount` values within a set are strictly ascending; no item appears in two `Set_MemberList`s; every member of a class-1824 set is an item. |
| IT12 | For class-795 records, the object-state worn map equals `Item_WornAppearanceMapList` (§3.3). |
| IT13 | The targets of `Usage_Permission`, `Usage_TakePermission` and `CSM_ActionHandler` are each the only record of their root class, and their property sets hold at most six properties. |

> **OPEN (implementer, 2026-10-07):** IT13 holds only when "record" means a 0x70 record. Counted
> over all object DIDs (0x70–0x77), the classes of 2 of the 24 targets also hold a 0x76
> singleton record. The real-data test counts 0x70 records. Please say which count IT13 and
> §5.10 mean.

A real-data test asserts IT1–IT13 and reports counts; it never asserts the counts themselves,
which change with client updates.

---

<a id="s13"></a>
## 13. Worked synthetic examples (for tests)

All numbers are invented for the examples; none is taken from the install.

**13.1 Classification.** A synthetic library has classes 10 (no parent), 20 (parent 10),
25 (parent 20), 30 (parent 25), 31 (parent 25), 40 (parent 30) and 50 (parent 20). Vendor,
barter and treasure lists name 900 records of class 40, 95 of class 31 and 5 of class 50. The
deepest common ancestor-or-self of {40, 31, 50} is *P* = 20. Its children are 25 (995
references in its subtree) and 50 (5), so *I* = 25. A record of class 30 is an item; a record
of class 50 is item-like but not an item; a record of class 10 is neither.

**13.2 Value.** Table: base values 2.0, 4.0, 6.5 at levels 1–3; quality factors key 5 → 1.0,
key 4 → 1.25. An item at level 3, quality 5: 6.5 × 1.0 = 6.5 → **7** (half rounds up). Level
3, quality 4: 6.5 × 1.25 = 8.125 → **8**. Level 2, quality 4: 4.0 × 1.25 = 5.0 → **5**. A null
table → **0**.

**13.3 DPS and damage.** DPS table: level 10 → 20.0; quality key 2 → 1.04. A weapon at item
level 12 with `ItemAdvancement_CombatDPSLevel` 10, quality 2: DPS = 20.0 × 1.04 = **20.8** (stored as the single-precision value nearest 20.8).
With duration 1.0 and variance 0.25: `Combat_Damage` = 20.8 × 1.0 × 2 ÷ 1.75 = **23.771429**;
Reported reading: minimum = 23.771429 × 0.75 = 17.828571; mean = 20.8.

**13.4 Armour.** Control points (1, 10.0), (11, 30.0), (21, 30.0). Level 6 → 20.0 → **20**;
level 7 → 22.0 → **22**; level 4 → 16.0 → **16**; level 25 → **30**. With points (1, 2.0),
(5, 5.0): level 2 → 2.75 → **2**.

**13.5 Mod entry.** A struct { `Mod_Modified` = PID 0x10000123 (a synthetic stat),
`Mod_Op` = 7, PID 0x10000123 = 15 }: a constant +15 (operation label from the mapper). A
struct { `Mod_Modified` = 0x10000123, `Mod_Op` = 7, `Mod_Progression` = 0x70001000 }: value
from that progression (§5.8 OPEN).

**13.6 Binding.** (true, true, false) → account-bound on acquire; (false, absent, true) →
bind on equip; all absent → unbound (§5.4).

**13.7 Set membership.** Set A lists items 1, 2, 3. Set B has no list; items 7 and 8 have
`Item_PropertySet` = B. Members: A = {1, 2, 3}, B = {7, 8}. Reverse index: 1, 2, 3 → A;
7, 8 → B.

---

<a id="s14"></a>
## 14. Open questions

> **OPEN:** Who applies the rules of §9 (client, server or the build pipeline that wrote the
> data). BIN found no native use (F30), which agrees with `combat-data.md`'s finding that the
> native client does not evaluate progressions; the script bytecode mentions the PIDs (§4.6) but was
> not interpreted. For Tessera the rules are data relations: a reader may recompute or trust
> the stored values; they agree everywhere.

> **OPEN:** Whether the client applies dictionary defaults to absent item properties
> (`Usage_AllowedInCombat` default true is absent in 135,074 items; `Usage_MinLevel` default 1
> absent in 50,495). Same question as `mesh.md` §8.5 OPEN. **Decision (Tessera, until settled):**
> use the dictionary default for an absent property.

> **OPEN (F21):** Level input, floor/ceiling semantics and operation combination for
> `Mod_Array` entries (§5.8); `EffectGenerator_EffectSpellcraft` = −1 and the floor/ceiling pair
> (§5.9).

> **OPEN:** Item-level scaling rule (munging, §5.2) and the meaning of `Item_iLevel_Crafting_Offset`.

> **OPEN:** Set bonus activation (§7.1), legendary rules (§7.3).

> **OPEN:** `Inventory_MaxQuantity` = 0 (2,979 items); `Item_Disenchant_Value` −1 versus 0;
> `Barter_IsBarterItem`; `Inventory_CanBeContained`; `Inventory_UseOnAcquire`;
> `Inventory_Indestructible`; the (false, true, true) binding combination.

> **OPEN:** Roles of the repair-cost, wear-state and skill-gate tables (§6.3); of the exam and
> merge property lists (§6.4).

> **OPEN:** `WeenieType` bit structure (§2.3); object-state scaffold (§3.2);
> `ExternalAppearanceID`, `Item_ImplementType`, `Item_MaterialType`, `UI_Examination_Tooltip_Compare_Type`.

> **OPEN:** Socketable level window and socket unlock (§5.11); the `Item_PILI_*` group (§5.12).

---

<a id="s15"></a>
## 15. Notes for other specs (not edited here)

- `appearance.md` §4.1: the worn item counts by `WeenieType` (41,871 / 200 / 15) agree with §2.1
  and §3.3 here; the object-state worn map (§3.3) is a mirror and need not be read.
- `wstate.md` §11.1 lists identification by property signature for areas; items follow the same
  principle with the class-ancestry rule of §2.4.
- `icon-layers.md` §2.2 counts 151,977 sets with a background: 151,230 items + 706 relics + 41
  carry-alls (§2.3 here).
- `mesh.md` §8.5 OPEN (dictionary defaults of absent properties) is the same question as the
  second OPEN of §14.

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Census over all 151,230 item records and all 383,332 property sets; derived-value relations verified against every item; one narrow BIN pass with a negative result. |
| 2026-10-07 | Review fixes, every count re-measured over all records: §9.2 double-precision disagreement is 533 items (half up; 624 half to even), not 93; F12, §9.3 step 3, IT5 and §13.3 state exact equality of the single-precision DPS product (23,204 of 23,204; double precision exact in 1,115); §2.4 gives references per source (91,274 / 100,761 / 116,764 = 308,799; 308,778 under 1734, 21 barter references to carry-alls); §8.2 treasure row says it counts class 1235 only (58 more from class 2372); §9.5 counts the 60,880 checked items (60,211 inside, 669 above, 0 below) and gives the 60,916-item counts separately. Status stays Review. |
