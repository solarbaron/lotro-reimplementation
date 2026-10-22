<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# M8 Gameplay P6: Progression, Experience, Trait Trees, Skill/Craft Trainers, Professions, Vocations, Recipes, Crafting Execution, Reputation, and Titles

- **Status:** Draft
- **Last updated:** 2026-10-21
- **Author role:** Spec writer (dirty room)
- **Reviewer:** pending

---

## Provenance

| Tag | Method | Source | Details / Scope |
|---|---|---|---|
| `OBS-gamelogic` | M1 (Observe install files read-only) | `client_gamelogic.dat` (SHA-256 `f67cd9ca40f4ff40edfd7ffd6ecc8bc0fdff0fb6d6b0f8bb62f0f1cf18f3d28f`) | WLib bytecode image (`0x31000000`, 5536 classes, 26397 method bodies) and Property Master Table (`0x34000000`) |
| `BIN-client64` | M4 (Static disassembly / decompilation) | `lotroclient64.exe` (Update 41+, SHA-256 `ac65f80f2dc892fd96ebc6fde5dc90ff2da23aa2fc7ad4ebf5f7dc44bfe5fb6d`), Ghidra 11.x / `objdump` | C++ callers of `EntityDesc::CallMethod` (`0x14074d610`), WLib native function bridge tables (`0x0130`, `0x0135`, `0x013A`, `0x015D`, `0x015E`, `0x0798`, `0x0E22`), and progression/crafting UI dispatchers |

---

## 1. Dirty-Room Analysis: Experience, Advancement, Skill Registry, and Skill Trainers

### 1.1 WLib Class Hierarchy & RPC Interface (`0x04DF` <-> `0x0185` / `0x0353`)

In `client_gamelogic.dat` (`0x31000000`), player advancement, experience, skill pools, and class trainers are governed by:
- **Class `0x04DF`**: Network RPC interface for advancement and skill training (`0x84DFxxxx` wire function IDs).
- **Class `0x0185`** (`AdvancementSystem`): Client-side advancement and trainer controller (`EntityDesc::GetSystem(0x185)`).
- **Class `0x0353`** (`ExperienceSystem`): Experience point and level-table calculation system (`EntityDesc::GetSystem(0x353)`).
- **Class `0x0355`** (`LevelTable`): Per-class advancement table WeenieContent record, retrieved via `0x0353::Fn_0x0041` (`GetLevelTableForClass(class_code)`).
- **Class `0x0351`** (`AdvancementTable`): Level-to-cumulative-XP lookup table WeenieContent record, referenced from `LevelTable` via property `XP_EarnedPointsToLevel_Table` (`0x10000A89`).
- **Class `0x0340`** (`SkillRegistry`): Attached to `CPlayer` (`0x06C3`, retrieved via virtual slot `0x0213`). Holds 4 `SkillPool` (`Class 0x033F`) references in its instance attributes:
  - Pool type `1` (`attr[4]`): **Trained Skill Pool** (normal character skills).
  - Pool type `2` (`attr[8]`): **Conjunction Skill Pool** (fellowship manoeuvre skills).
  - Pool type `3` (`attr[5]`): **Combo Skill Pool** (chain/response skills).
  - Pool type `4` (`attr[7]`): **Gambit Skill Pool** (Warden gambits).

### 1.2 Experience & Level Properties (`ProgressionEvent::ExperienceChanged`)

There is no dedicated standalone S->C "XP gained" RPC packet; instead, player experience and level state are synchronized through standard entity property updates (`PropertyBag` / `PropertyResponse`) on the local player entity (`CPlayer` `0x06C3`) and global world properties:

| Property ID | Name (`0x34000000`) | Type | Reader Method | Meaning |
|---|---|---|---|---|
| `0x10001011` | `Advancement_Level` | `i32` | `0x0E71::Fn_0x0003` (body `0x006584`) | Current character level (defaults to `0` if unset). |
| `0x10000D33` | `XP_EarnedXP` | `i64` | `0x0353::Fn_0x001C` (body `0x067C90`) | Cumulative total earned character XP (`0x01E4::Fn_0x002B` 64-bit integer property getter). |
| `0x10000CC8` | `XP_BonusXP` | `i64` | `PropertyBag` | Remaining Rest / Bonus XP pool on the player entity. |
| `0x1000370B` | `XP_DisableXP` | `bool` (`u32`) | `0x0353::Fn_0x0043` (body `0x067CDC`) | Non-zero when character XP gain is disabled (e.g., Stone of the Tortoise equipped). |
| `0x10001631` | `World_NonAdminPlayerLevelCap` | `i32` | `0x0353::Fn_0x0037` (body `0x068B50`) | Global server level cap for non-admin players (read via world property helper `0x04EE::Fn_0x0001`). |
| `0x10000A89` | `XP_EarnedPointsToLevel_Table` | `u32` (DID) | `0x0355::Fn_0x0006` (body `0x069648`) | Property on the class `LevelTable` (`0x0355`) giving the DID of the `AdvancementTable` (`0x0351`) that maps character level to cumulative `i64` XP required. |
| `0x1000017A` | `Advancement_MonsterPlayXP` | `i64` | `0x0353::Fn_0x0017` (body `0x067D08`) | Monster Play earned XP. |
| `0x10003715` | `MonsterPlay_CurrentFreePeepXP` | `i64` | `0x0353::Fn_0x0012` (body `0x067CB4`) | Free Peoples PvMP XP. |

**Within-level XP progress calculation (`0x0353::Fn_0x0045` & `Fn_0x0046`):**
1. `0x0353::Fn_0x0042` (`GetXPAdvancementTableForPlayer`, body `0x0688B0`) fetches the player's class via `0x0E71::Fn_0x0001`, looks up the class `LevelTable` (`0x0355`), and resolves the `AdvancementTable` (`0x0351`) from `XP_EarnedPointsToLevel_Table` (`0x10000A89`).
2. `0x0353::Fn_0x0045` (body `0x0689D4`) computes **XP into current level**: `XP_EarnedXP(player) - AdvancementTable.GetPointsForLevel(current_level)`.
3. `0x0353::Fn_0x0046` (body `0x068A54`) computes **XP required for next level**: `AdvancementTable.GetPointsForLevel(current_level + 1) - AdvancementTable.GetPointsForLevel(current_level)`.

### 1.3 Advancement & Skill Trainer Wire RPCs (`0x04DF`)

| Direction | Wire `function_id` | `0x04DF` Method | Handler / Caller | `WLibArgFrame` Layout (Stream Order) | Semantics |
|---|---|---|---|---|---|
| S -> C | `0x84DF001E` | `Fn_0x001E` (`0x0C3ED0`) | `0x06C3::Fn_0x0049` (`0x0EB914`) -> `0x0340::Fn_0x002D` (`0x063AB0`) | `[unpack_ref(0x033F)]` (`skill_pool: Class 0x033F`) | **Full SkillPool replacement (`SkillLearned` / `SkillForgotten`).** Reads `pool_type` (`0x033F::Fn_0x000B`), diffs against existing pool (`0x033F::Fn_0x0018`), replaces the pool on `SkillRegistry`, fires native `0x015E0018` (`VA=0x140b42540`) for removed skills and `0x015E0007` (`VA=0x140b47ec0`) for updated/added skills. |
| S -> C | `0x84DF0003` | `Fn_0x0003` (`0x0C3AA4`) | `0x06C3::Fn_0x00AC` (`0x0EC890`) -> `0x0340::Fn_0x002C` (`0x064BA4`) | `[unpack_ref, unpack_32]` (`skill_entry: ref, pool_type: u32`) | **Single skill entry update in pool (`SkillLearned`).** Inserts/updates `skill_entry` in `pool_type` (`1`..`4`) and calls native `0x015D000A` (`VA=0x140b47e10`). |
| S -> C | `0x84DF0016` | `Fn_0x0016` (`0x0C3CE8`) | `0x0185::Fn_0x0061` (`0x02658C`) -> `0x0185::Fn_0x0021` (`0x02561C`) | `[unpack_32, unpack_32]` (`old_level: u32, new_level: u32`) | **Level-up notification.** Formats chat notification (`0x0ADCCCA5`) and notifies `CSkirmishSystem` (`0x0B8D`), `GuildSystem` (`0x03D3`), and `CTraitSystem` (`0x0125`). |
| S -> C | `0x84DF001B` | `Fn_0x001B` (`0x0C3DC8`) | `0x0185::Fn_0x005C` (`0x0255FC`) | `[unpack_64]` (`trainer_iid: u64`) | **Open Class/Skill Trainer UI (`TrainerShown`).** Invokes native `0x01300005` (`VA=0x140b3bb70`) with `trainer_iid`. |
| S -> C | `0x84DF0010` | `Fn_0x0010` (`0x0C3BCC`) | `0x0185::Fn_0x0060` (`0x02642C`) | `[unpack_32]` (`skill_did: u32`) | **Skill training error / notification.** Formats skill name into string `0x0ADCCC9F` and displays an alert. |
| C -> S | `0x84DF0005` | `Fn_0x0023` (`0x0C3F30`) | `0x0185::Fn_0x000C` (`0x023CD8`), C++ `0x140aa0972` | `[pack_32(skill_did), pack_64(trainer_iid)]` | **Train Skill at Trainer (`ProgressionCommand::TrainSkill`).** C++ caller passes `rcx=trainer_iid`, `edx=skill_did`; checks `CanTrainSkill` (`0x0185::Fn_0x0053`), then packs `skill_did: u32` followed by `trainer_iid: u64`. |
| C -> S | `0x84DF001A` | `Fn_0x0021` (`0x0C3BF8`) | `0x0185::Fn_0x0010` (`0x023DB8`), C++ `0x140aa0b12` | `[pack_32(trait_did), pack_64(trainer_iid)]` | **Train Characteristic/Trait at Trainer.** Checks `CanTrainCharacteristic` (`0x0185::Fn_0x0049`), then packs `trait_did: u32` followed by `trainer_iid: u64`. |
| C -> S | `0x84DF002D` | `Fn_0x002E` (`0x0C3FD8`) | `0x0185::Fn_0x00AC` (`0x023D48`), C++ `0x140aa0a42` | `[pack_32(did), pack_64(trainer_iid)]` | **Train secondary advancement item at Trainer.** Checks `0x0185::Fn_0x00AD`, then packs `did: u32` followed by `trainer_iid: u64`. |

### 1.4 Untrained Skills Derivation (`ProgressionEvent::UntrainedSkills`)

`UntrainedSkills` is computed locally by `AdvancementSystem`:
- `0x0185::Fn_0x0062` (`GetUntrainedBuyableSkills`, body `0x024720`, C++ caller `0x140a9f4cc`) and `0x0185::Fn_0x005D` (`GetUntrainedBuyableCharacteristics`, body `0x024A80`, C++ caller `0x140a9f37c`).
- It retrieves the player's class `LevelTable` (`0x0355`) via `0x0353::Fn_0x0041`, iterates the class's trainable skill/characteristic entries (`0x0354`), filters out skills already present in `CPlayer`'s `SkillRegistry` (`0x0340::Fn_0x0022`) or `TraitRegistry` (`0x05D0::Fn_0x002D`), and returns an `IntrusiveList` (`nat_cls=0x0019`) of untrained DIDs.

---

## 2. Dirty-Room Analysis: Faction Reputation (`ProgressionEvent::ReputationChanged`)

### 2.1 WLib Class Hierarchy & RPC Interface (`0x077B` <-> `0x0769` / `0x0761`)

- **Class `0x077B`**: Reputation network RPC interface (`0x877Bxxxx`).
- **Class `0x0769`** (derived `0x076B`): Client `ReputationSystem` (`EntityDesc::GetSystem(0x769)`).
- **Class `0x0761`** (`Faction`): WeenieContent data record describing a reputation faction.

### 2.2 Faction Discovery RPC & Dynamic Per-Faction Property Architecture

| Direction | Wire `function_id` | `0x077B` Method | Handler | `WLibArgFrame` Layout (Stream Order) | Semantics |
|---|---|---|---|---|---|
| S -> C | `0x877B0007` | `Fn_0x0007` (`0x1A68EC`) | `0x0769::Fn_0x000D` (`0x1A59EC`) | `[unpack_32]` (`faction_did: u32`) | **Initialize / Discover Faction.** Registers property listeners (`0x0769::Fn_0x0013`) on the player entity for the faction's dynamic properties and fires native `0x07980001` (`VA=0x140b35580`). |
| C -> S | `0x877B0008` | `Fn_0x0009` (`0x1A6918`) | `0x0769::Fn_0x0018` (`0x1A5A84`) | `[pack_32(arg0), pack_32(faction_did)]` | **Faction action / reward request.** Packs `arg0: u32` and `faction_did: u32`. |

**How `ReputationChanged` works on the wire:**
Each `Faction` (`Class 0x0761`) WeenieContent record stores the **Property IDs** used on the player entity for that specific faction (`0x0769::Fn_0x0013`, body `0x1A5DC8`):

| Faction Weenie Property ID | Name (`0x34000000`) | Type | Faction Getter | Meaning |
|---|---|---|---|---|
| `0x1000177D` | `Reputation_Faction_CurrentTier_PropertyName` | `u32` (Property ID) | `0x0761::Fn_0x0005` (`0x1A49D4`) | Returns the player entity Property ID (`u32`) that stores the player's current standing tier (`u32`) with this faction. |
| `0x1000177E` | `Reputation_Faction_EarnedReputation_PropertyName` | `u32` (Property ID) | `0x0761::Fn_0x000B` (`0x1A49F0`) | Returns the player entity Property ID (`u32`) that stores the player's cumulative earned reputation points (`i64`) with this faction. |
| `0x1000189D` | `Reputation_Faction_GlobalCap_PropertyName` | `u32` (Property ID) | `0x0761::Fn_0x000A` (`0x1A4A0C`) | Returns the world Property ID (`u32`) for the faction's global cap. |
| `0x10001762` | `Reputation_Faction_AdvancementTable` | `u32` (DID) | `0x0761::Fn_0x0002` (`0x1A4C58`) | DID of the `AdvancementTable` (`0x0351`) mapping tier (`u32`) to cumulative reputation points (`i64`). |
| `0x10001757` | `Reputation_Faction_DefaultTier` | `u32` | `0x0761::Fn_0x0006` (`0x1A4A60`) | Default starting tier when the faction is initialized. |
| `0x1000175B` | `Reputation_LowestTier` | `u32` | `0x0761::Fn_0x000E` (`0x1A4AB4`) | Minimum standing tier for this faction. |
| `0x1000175C` | `Reputation_HighestTier` | `u32` | `0x0761::Fn_0x000C` (`0x1A4AD0`) | Maximum standing tier for this faction. |
| `0x10003E94` | `Reputation_BonusRep_BonusRemaining` | `i64` | `0x0769::Fn_0x001E` (`0x1A5248`) | Player entity property holding remaining bonus reputation points. |

When `0x0769::Fn_0x0013` runs for a `Faction`:
1. It registers a property-change callback (`vslot 0x0041`) on the player for `Reputation_Faction_CurrentTier_PropertyName` -> `0x0769::Fn_0x001A` (`0x1A5FA8`), which invokes native `0x07980007` (`VA=0x140b434f0`) when the standing tier changes.
2. It registers a property-change callback (`vslot 0x0041`) on the player for `Reputation_Faction_EarnedReputation_PropertyName` -> `0x0769::Fn_0x0016` (`0x1A5EC0`), which invokes native `0x07980006` (`VA=0x140b3fbf0`) when reputation points change, and a property-removal callback (`vslot 0x0045`) -> `0x0769::Fn_0x0015` (`0x1A5E68`) -> native `0x07980005` (`VA=0x140b355f0`).

---

## 3. Dirty-Room Analysis: Titles (`ProgressionEvent::TitleGranted`)

### 3.1 WLib Class Hierarchy & RPC Interface (`0x052E` <-> `0x017E` / `0x05C0` / `0x05BE`)

- **Class `0x052E`**: Title network RPC interface (`0x852Exxxx`).
- **Class `0x017E`** (`TitleSystem`): Client title manager (`EntityDesc::GetSystem(0x17E)`).
- **Class `0x05C0`** (`TitleRegistry`): Player title collection stored on `CPlayer`.
- **Class `0x05BE`** (`Title`): WeenieContent record for a title (includes `Title_Equivalence_Group` `0x100010A4` and `Title_Equivalence_Rank` `0x100010A3`).

### 3.2 Title Wire RPCs (`0x052E`) & Properties

| Direction | Wire `function_id` | `0x052E` Method | Handler / Caller | `WLibArgFrame` Layout (Stream Order) | Semantics |
|---|---|---|---|---|---|
| S -> C | `0x852E0005` | `Fn_0x0005` (`0x10AECC`) | `0x017E::Fn_0x0022` (`0x0229C4`, `RecvCEvt_TitleAwarded`) | `[unpack_f64, unpack_32, unpack_32]` (`timestamp: f64, new_title_did: u32, replaced_title_did: u32`) | **Title Granted (`ProgressionEvent::TitleGranted`).** Adds `new_title_did` to `TitleRegistry` (`0x05C0::Fn_0x000A`, replacing any lower-ranked title in the same equivalence group) and invokes native `0x0135001D` (`VA=0x140b43560`). |
| S -> C | `0x852E0013` | `Fn_0x0013` (`0x10AFDC`) | `0x017E::Fn_0x0029` (`0x022A88`) | `[unpack_32]` (`title_did: u32`) | **Title Revoked.** Removes `title_did` from `TitleRegistry` (`0x05C0::Fn_0x0005`) and invokes native `0x0135001E` (`VA=0x140b37940`). |
| S -> C | `0x852E0016` | `Fn_0x0016` (`0x10B074`) | `0x017E::Fn_0x0027` (`0x022A3C`) | `[]` (empty) | **Clear All Titles.** Reinitializes `TitleRegistry` (`0x05C0::Fn_0x0006`) and invokes native `0x0135001F` (`VA=0x140b3b970`). |
| C -> S | `0x852E000E` | `Fn_0x001C` (`0x10AF70`) | `0x017E::Fn_0x0005` (`0x022474`), C++ `0x140b50574` | `[pack_32(title_did)]` | **Set Active Title.** Verifies title ownership (`0x017E::Fn_0x001A`), then packs `title_did: u32`. |
| C -> S | `0x852E0002` | `Fn_0x001D` (`0x10AE5C`) | `0x017E::Fn_0x0006` (`0x0224CC`), C++ `0x140b4f642` | `[]` (empty) | **Clear Active Title.** Sends empty frame to remove the player's active title. |

**Active Title Properties on Player Entity:**
- `Title_ActiveTitleDID` (`0x1000088B`, `u32`): Read by `0x017E::Fn_0x0012` (`0x022554`) to determine the player's currently displayed title DID.
- `Title_ActiveTitleString` (`0x1000119C`, `StringInfo`): Read by `0x017E::Fn_0x0017` (`0x022570`) for the localized title string on any entity.

---

## 4. Dirty-Room Analysis: Traits, Virtues, Slotting, and Trait Trees

### 4.1 WLib Class Hierarchy & RPC Interfaces (`0x0532` & `0x0E15`)

- **Class `0x0532`**: Trait and Virtue network RPC interface (`0x8532xxxx`).
- **Class `0x0125`** (`CTraitSystem`, base `0x05D3` `TraitSystem`): Client trait manager (`EntityDesc::GetSystem(0x125)`).
- **Class `0x05D0`** (`TraitRegistry`): Attached to `CPlayer` (`vslot 0x0212`), stores earned traits, trait ranks, and slotted traits.
- **Class `0x05D1`** (`TraitSlotDescriptor`, 4 `u32` attributes): Serialized via `pack_ref`/`unpack_ref` (`0x05D1::Fn_0x000A` pack, `0x05D1::Fn_0x000E` unpack):
  - `attr[0]` (`pack_32`/`unpack_32`): `tier: u32`
  - `attr[1]` (`pack_32`/`unpack_32`): `has_tier: u32` (`0` or `1`)
  - `attr[2]` (`pack_32`/`unpack_32`): `index: u32` (slot index)
  - `attr[3]` (`pack_32`/`unpack_32`): `has_index: u32` (`0` or `1`)
- **Class `0x0E15`**: Trait Configuration and Point-Based Trait Tree network RPC interface (`0x8E15xxxx`).
- **Class `0x0E16`** (base `0x0E05` `TraitConfigurationSystem`): Client trait tree and multi-spec configuration manager (`EntityDesc::GetSystem(0xE16)`).

### 4.2 Trait & Virtue Wire RPCs (`0x0532`)

| Direction | Wire `function_id` | `0x0532` Method | Handler / Caller | `WLibArgFrame` Layout (Stream Order) | Semantics |
|---|---|---|---|---|---|
| S -> C | `0x85320013` | `Fn_0x0013` (`0x10DCBC`) | `0x0125::Fn_0x000D` (`0x011DFC`, `RecvCEvt_SlotTraitRequest`) | `[unpack_ref(0x05D1), unpack_32]` (`slot_desc: Class 0x05D1, trait_did: u32`) | **Trait Slotted (`ProgressionEvent::TraitSlotted`).** Validates and places `trait_did` into `slot_desc` (`0x05D3::Fn_0x0037`). |
| S -> C | `0x8532000B` | `Fn_0x000B` (`0x10DBA8`) | `0x0125::Fn_0x0015` (`0x012110`, `RecvCEvt_ForcePlaceTraitRequest`) | `[unpack_ref(0x05D1), unpack_32]` (`slot_desc: Class 0x05D1, trait_did: u32`) | **Force Place Trait (`ProgressionEvent::TraitSlotted`).** Unconditionally slots `trait_did` into `slot_desc` (`0x05D3::Fn_0x0012`). |
| S -> C | `0x85320003` | `Fn_0x0003` (`0x10DA70`) | `0x0125::Fn_0x0011` (`0x011FA8`, `RecvCEvt_UnslotTraitByDIDRequest`) | `[unpack_32]` (`trait_did: u32`) | **Trait Unslotted.** Removes `trait_did` from its slot (`0x05D3::Fn_0x003D`). |
| S -> C | `0x85320009` | `Fn_0x0009` (`0x10DB40`) | `0x0125::Fn_0x000F` (`0x011EDC`, `RecvCEvt_UnslotAllTraitsOfNatureRequest`) | `[unpack_32]` (`trait_nature: u32`) | **Unslot All Traits of Nature.** Clears all slotted traits matching `trait_nature` (`0x05D3::Fn_0x0039`). |
| S -> C | `0x85320002` | `Fn_0x0002` (`0x10DA14`) | `0x0125::Fn_0x0014` (`0x012078`, `RecvCEvt_UnslotAllTraitsRequest`) | `[]` (empty) | **Unslot All Traits.** Clears all slotted traits across all natures (`0x05D3::Fn_0x0038`). |
| S -> C | `0x85320010` | `Fn_0x0010` (`0x10DC50`) | `0x0125::Fn_0x001F` (`0x0125F0`) | `[unpack_32, unpack_32]` (`new_rank: u32, trait_did: u32`) | **Trait / Virtue Rank Changed (`ProgressionEvent::TraitRank`).** Notifies the client that `trait_did` reached `new_rank`. |
| S -> C | `0x8532001D` | `Fn_0x001D` (`0x10DDDC`) | `0x0125::Fn_0x0017` (`0x0121C8`, `RecvCEvt_AddEarnedTraitRequest`) | `[unpack_32, unpack_32]` (`notify_flag: u32, trait_did: u32`) | **Add Earned Trait.** Adds `trait_did` to `TraitRegistry` (`0x05D3::Fn_0x001C`). |
| S -> C | `0x85320020` | `Fn_0x0020` (`0x10DE48`) | `0x0125::Fn_0x0019` (`0x0122A4`, `RecvCEvt_RemoveEarnedTraitRequest`) | `[unpack_32]` (`trait_did: u32`) | **Remove Earned Trait.** Removes `trait_did` from `TraitRegistry` (`0x05D3::Fn_0x0031`). |
| S -> C | `0x85320056` | `Fn_0x0056` (`0x10E364`) | `0x0125::Fn_0x0076` (`0x012DFC`, `RecvCEvt_AddEarnedTraitsRequest`) | `[unpack_32, unpack_ref(Array<u32>)]` (`notify_flag: u32, trait_dids: Array<u32>`) | **Batch Add Earned Traits.** Adds an array of `trait_did`s (`0x05D3::Fn_0x006A`). |
| S -> C | `0x85320054` | `Fn_0x0054` (`0x10E2FC`) | `0x0125::Fn_0x0075` (`0x012D48`, `RecvCEvt_RemoveEarnedTraitsRequest`) | `[unpack_ref(Array<u32>)]` (`trait_dids: Array<u32>`) | **Batch Remove Earned Traits.** Removes an array of `trait_did`s (`0x05D3::Fn_0x006B`). |
| C -> S | `0x85320016` | `Fn_0x0026` (`0x10DD28`) | `0x0125::Fn_0x0003` (`0x011A50`), C++ `0x140b9a3ac` | `[pack_ref(0x05D1), pack_32(trait_did)]` | **Slot Trait (`ProgressionCommand::SlotTrait`).** Constructs `Class 0x05D1` with `index=slot_index, has_index=1`, checks `CanSlotTrait` (`0x05D3::Fn_0x0009`), and packs `slot_desc` followed by `trait_did: u32`. |
| C -> S | `0x8532000E` | `Fn_0x0027` (`0x10DC14`) | `0x0125::Fn_0x0004` (`0x011AE0`), C++ `0x140b9b872` | `[pack_32(trait_did)]` | **Unslot Trait.** Checks `CanUnslotTrait` (`0x05D3::Fn_0x000C`), then packs `trait_did: u32`. |
| C -> S | `0x8532003D` | `Fn_0x003E` (`0x10E018`) | `0x0125::Fn_0x0050` (`0x011C8C`) | `[pack_ref(HashTable<u32, 0x05D1>), pack_32(trait_nature)]` | **Commit Slotted Traits for Nature.** Packs a map of `trait_did -> Class 0x05D1` and `trait_nature: u32`. |
| C -> S | `0x85320045` | `Fn_0x0046` (`0x10E0CC`) | `0x0125::Fn_0x0059` (`0x011CF4`), C++ `0x140b91ec0` | `[]` (empty) | **Reset / Clear Slotted Traits.** |
| C -> S | `0x85320049` | `Fn_0x004A` (`0x10E138`) | `0x0125::Fn_0x0061` (`0x011D3C`), C++ `0x140b9972b` | `[pack_32(arg0)]` | **Unlock / Purchase Trait Slot.** |
| C -> S | `0x8532004B` | `Fn_0x004E` (`0x10E184`) | `0x0125::Fn_0x0066` (`0x012B20`), C++ `0x140b9a4cf` | `[pack_32(config_slot), pack_ref(0x05D1), pack_32(trait_did)]` | **Slot Trait in Configuration Slot.** |
| C -> S | `0x8532004F` | `Fn_0x0050` (`0x10E210`) | `0x0125::Fn_0x0067` (`0x012BC0`), C++ `0x140b9b966` | `[pack_32(config_slot), pack_32(trait_did)]` | **Unslot Trait in Configuration Slot.** |
| C -> S | `0x85320057` | `Fn_0x0058` (`0x10E3AC`) | `0x0125::Fn_0x003C` (`0x011BD0`), C++ `0x140b65f95` | `[pack_32(virtue_trait_did)]` | **Set Active Earning Virtue.** Selects which Virtue trait receives Virtue XP (`0x05D3::Fn_0x006D`). |

### 4.3 Point-Based Trait Trees & Trait Configurations (`0x0E15`)

| Direction | Wire `function_id` | `0x0E15` Method | Handler / Caller | `WLibArgFrame` Layout (Stream Order) | Semantics |
|---|---|---|---|---|---|
| C -> S | `0x8E15000F` | `Fn_0x0010` (`0x28FA44`) | `0x0E16::Fn_0x000A` (`0x28FEB4`), C++ `0x140b9a9d4` | `[pack_ref(rank_map), pack_ref(slotted_map), pack_32(branch), pack_32(tree_type), pack_32(config_slot)]` | **Commit Point-Based Trait Tree (`ChooseTraitBranch` / `SetTraitRank`).** Validates via `0x0E05::Fn_0x0030`, then packs: (1) `rank_map: HashTable<u32, u32>` mapping `trait_did -> rank`, (2) `slotted_map: HashTable` of slotted traits, (3) `branch: u32` specialization branch enum, (4) `tree_type: u32`, (5) `config_slot: u32`. |
| C -> S | `0x8E150001` | `Fn_0x0002` (`0x28F6EC`) | `0x0E16::Fn_0x0001` (`0x28FD48`), C++ `0x140b611f1` | `[pack_32(tree_type), pack_32(config_slot)]` | **Switch Active Trait Configuration Slot.** |
| C -> S | `0x8E150005` | `Fn_0x0006` (`0x28F804`) | `0x0E16::Fn_0x0006` (`0x28FDC8`), C++ `0x140b9ab15` | `[pack_ref(slotted_map), pack_32(branch), pack_32(tree_type), pack_32(config_slot)]` | **Commit Trait Slot Configuration.** |
| C -> S | `0x8E150009` | `Fn_0x000A` (`0x28F90C`) | `0x0E16::Fn_0x0007` (`0x28FE2C`), C++ `0x140b92e40` | `[pack_32(config_slot)]` | **Purchase / Unlock Additional Trait Configuration Slot.** |
| C -> S | `0x8E15000B` | `Fn_0x000C` (`0x28F96C`) | `0x0E16::Fn_0x0008` (`0x28FE60`), C++ `0x140b99d38` | `[pack_ref(StringInfo), pack_32(tree_type), pack_32(config_slot)]` | **Rename Trait Configuration Slot.** |
| S -> C | `0x8E15000E` | `Fn_0x000E` (`0x28FA04`) | `0x0E16::Fn_0x0009` (`0x28FE9C`) | `[unpack_ref, unpack_32]` (`config_data: ref, config_slot: u32`) | **Trait Tree / Configuration Updated (`ProgressionEvent::TraitTreeChanged`).** Invokes native `0x0E220001` (`VA=0x140b441d0`). |
| S -> C | `0x8E150011` | `Fn_0x0011` (`0x28FAC4`) | `0x0E16::Fn_0x000C` (`0x28FF18`) | `[unpack_32]` (`status_or_slot: u32`) | **Trait Configuration Commit Response.** Invokes native `0x0E220002` (`VA=0x140b44090`). |
| S -> C | `0x8E150013` | `Fn_0x0013` (`0x28FB24`) | `0x0E16::Fn_0x000B` (`0x28FF00`) | `[unpack_32]` (`status_or_slot: u32`) | **Trait Configuration Reset / Notification.** Invokes native `0x0E220003` (`VA=0x140b44130`). |

### 4.4 Trait Point & Trait Tree Properties (`ProgressionEvent::TraitPoints`, `TraitTreeChanged`)

| Property ID | Name (`0x34000000`) | Type | Meaning |
|---|---|---|---|
| `0x1000201C` | `Trait_ClassSpecialization_ActiveBranch` | `u32` (Enum) | Active class specialization branch ID. |
| `0x10002B90` | `Trait_PointBasedTraitTree_Branch` | `u32` (Enum) | Selected branch in the point-based trait tree (`0x0E05::Fn_0x001A`). |
| `0x10002B8F` | `Trait_PointBasedTraitTree_RankArray` | `Array` | Array of allocated trait ranks in the active point-based trait tree (`0x0E05::Fn_0x0015`). |
| `0x10002B91` | `Trait_PointBasedTraitTree_PointsSpent` | `i32` | Trait points currently spent in the active configuration (`0x0E05::Fn_0x0012`). |
| `0x10002B92` | `Trait_PointBasedTraitTree_PointsTotal` | `i32` | Total trait points available (`PointsEarned + PointsBonus`, `0x0E05::Fn_0x0010`). |
| `0x10002B93` | `Trait_PointBasedTraitTree_PointsEarned` | `i32` | Trait points earned from character level and deeds. |
| `0x10002B94` | `Trait_PointBasedTraitTree_PointsBonus` | `i32` | Bonus trait points granted by promotions/items. |
| `0x1000399C` | `Trait_Virtue_ActiveTrait` | `u32` (DID) | Currently active Virtue trait DID earning Virtue XP (`0x05D3::Fn_0x006C`). |
| `0x10003999` | `Trait_Virtue_BonusXP` | `i32` | Bonus Virtue XP pool (`0x05D3::Fn_0x006F`). |

---

## 5. Dirty-Room Analysis: Crafting, Professions, Vocations, Recipes, and Craft Trainers

### 5.1 WLib Class Hierarchy & RPC Interfaces (`0x04EF` & `0x0528`)

- **Class `0x04EF`**: Crafting trainer, recipe learning, vocation/profession management, and crafting state S->C / C->S RPC interface (`0x84EFxxxx`).
- **Class `0x0528`**: Player action RPC interface (`0x8528xxxx`), whose `Fn_0x001D` (`0x85280005`) carries the **Execute Craft** C->S command.
- **Class `0x0107`** (`CCraftSystem`, base `0x0401` `CraftSystem`): Client crafting system (`EntityDesc::GetSystem(0x107)`).
- **Class `0x0A4A`** (`CraftExecutionDescriptor`, 4 attributes): Serialized via `0x0A4A::Fn_0x000A` (pack) and `0x0A4A::Fn_0x0006` (unpack):
  - `attr[0]` (`pack_32`/`unpack_32`): `multi_output_index: u32` (`0` = single/default output, `1+` = 1-based index into `CraftRecipe`'s multi-output array `attr[3]`).
  - `attr[1]` (`pack_ref`/`unpack_ref` of `Array<u32>`): `optional_ingredients: Array<u32>` (1-to-1 boolean `0`/`1` array matching the recipe's optional ingredient list `CraftRecipe.attr[4]`).
  - `attr[2]` (`pack_32`/`unpack_32`): `use_ingredient_pack: u32` (`0` = normal ingredients, `1` = consume ingredient pack via `CraftRecipe::Fn_0x002B`).
  - `attr[3]` (`pack_32`/`unpack_32`): `tier: u32` (crafting tier ID for the recipe).
- **Class `0x03FF`** (`CraftProfession`): WeenieContent record for a profession (e.g., Prospector, Metalsmith, Scholar).
- **Class `0x0402`** (`CraftVocation`): WeenieContent record for a 3-profession vocation (e.g., Armourer, Explorer, Historian).
- **Class `0x0400`** (`CraftRecipe`): WeenieContent record for a crafting recipe.
- **Class `0x08E6`** (`CraftGuild`): WeenieContent record for a crafting guild.

### 5.2 Vocation, Profession, and Recipe Properties (`VocationChanged`, `ProfessionChanged`)

On the player entity, vocation and profession state are driven by static properties plus per-profession dynamic properties defined on each `CraftProfession` (`0x03FF`) WeenieContent record:

| Property ID | Name (`0x34000000`) | Owner | Type | Getter | Meaning |
|---|---|---|---|---|---|
| `0x10001481` | `Craft_Vocation` | Player | `u32` (DID) | `0x0401::Fn_0x002D` (`0x082F84`) | Player's active `CraftVocation` (`0x0402`) DID (`0` if using standalone professions). |
| `0x10006F93` | `Craft_ProfessionArray` | Player | `Array<u32>` | `0x0401::Fn_0x0053` (`0x083D78`) | Array of trained `CraftProfession` (`0x03FF`) DIDs on the player (used in U38+ decoupled profession mode when `Craft_Vocation == 0`). |
| `0x100067BF` | `Craft_CraftGuild_Membership_Array` | Player | `Array<u32>` | `0x0401::Fn_0x0052` (`0x085150`) | Array of `CraftGuild` (`0x08E6`) DIDs the player belongs to. |
| `0x1000079B` | `CraftProfession_Enabled_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x000B` (`0x082454`) | Player Property ID (`bool`) indicating whether this profession is trained/enabled (`0x0401::Fn_0x000B`). |
| `0x10000CF1` | `CraftProfession_ProficiencyLevel_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x0006` (`0x082508`) | Player Property ID (`u32`) holding current **Proficiency Tier** (`0x0401::Fn_0x000D`). |
| `0x10000B32` | `CraftProfession_MasteryLevel_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x0008` (`0x0824E4`) | Player Property ID (`u32`) holding current **Mastery Tier** (`0x0401::Fn_0x0009`). |
| `0x100010BC` | `CraftProfession_ProficiencyXP_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x0013` (`0x0824C0`) | Player Property ID (`i32`) holding earned **Proficiency XP** in the current proficiency tier (`0x0401::Fn_0x0011`). |
| `0x10001296` | `CraftProfession_MasteryXP_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x001B` (`0x08249C`) | Player Property ID (`i32`) holding earned **Mastery XP** in the current mastery tier (`0x0401::Fn_0x0007`). |
| `0x100026E9` | `CraftProfession_OpenLevel_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x0024` (`0x082598`) | Player Property ID (`u32`) holding the highest unlocked crafting tier for this profession (`0x0401::Fn_0x0037`). |
| `0x100006C6` | `CraftProfession_ExtraRecipeArray_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x000F` (`0x082478`) | Player Property ID (`Array<u32>`) holding learned non-default recipe DIDs for this profession (`0x0401::Fn_0x001C`). |
| `0x1000004B` | `CraftProfession_CriticalChanceAddModifier_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | `0x03FF::Fn_0x0014` (`0x08252C`) | Player Property ID (`f32`) holding the additive critical chance modifier (`0x0401::Fn_0x0006`). |
| `0x10001476` | `Craft_CurrentRecipe` | Player | `u32` (DID) | `0x0401::Fn_0x002B` (`0x082DB8`) | Currently selected/active `CraftRecipe` (`0x0400`) DID on the player (`0x10001476`). |

**Known Recipes (`RecipeLearned` / `RecipeForgotten`):**
`0x0401::Fn_0x001B` (`GetKnownRecipesForProfession`, body `0x0848D0`) constructs the complete known recipe list for a profession by:
1. Iterating all tiers `1 .. OpenLevel` and unioning the tier's automatic `DefaultRecipeArray` (`0x03FF::Fn_0x0015`).
2. Unioning the player's learned extra recipes from the profession's `CraftProfession_ExtraRecipeArray_PropertyName` (`0x100006C6`) property array (`0x0401::Fn_0x001C`).
When a single-use recipe is consumed or a recipe is unlearned, the server updates the profession's `ExtraRecipeArray` property on the player entity (`ProgressionEvent::RecipeForgotten`).

### 5.3 Crafting Execution & Trainer Wire RPCs (`0x0528` & `0x04EF`)

| Direction | Wire `function_id` | Class / Method | Handler / Caller | `WLibArgFrame` Layout (Stream Order) | Semantics |
|---|---|---|---|---|---|
| C -> S | `0x85280005` | `0x0528::Fn_0x001D` (`0x108CF8`) | `0x0107::Fn_0x000B` (`0x00B8B0`), C++ `0x140b678fe` | `[pack_ref(0x0A4A)]` (`craft_desc: Class 0x0A4A`) | **Execute Craft (`ProgressionCommand::Craft`).** C++ `0x140b678fe` sets/checks the active recipe, builds `Class 0x0A4A` (`multi_output_index`, `optional_ingredients`, `use_ingredient_pack`, `tier`), validates via `0x0401::Fn_0x0005`, and sends `0x85280005`. Batch crafting (quantity > 1) is managed by repeating execution per induction cycle while the client batch counter remains non-zero. |
| C -> S | `0x84EF0006` | `0x04EF::Fn_0x001D` (`0x0CDCD0`) | `0x0107::Fn_0x0007` (`0x00B6F0`), C++ `0x140b9b302` | `[pack_32(vocation_did), pack_64(trainer_iid)]` | **Train Vocation (`ProgressionCommand::TrainCraft`).** Validates via `0x0401::Fn_0x0008`, then packs `vocation_did: u32` followed by `trainer_iid: u64`. |
| C -> S | `0x84EF000D` | `0x04EF::Fn_0x001F` (`0x0CDD54`) | `0x0107::Fn_0x0008` (`0x00B77C`), C++ `0x140b8dc08` | `[pack_ref(StringInfo), pack_64(trainer_iid)]` | **Untrain / Abandon Vocation.** Packs confirmation `StringInfo` and `trainer_iid: u64`. |
| C -> S | `0x84EF005B` | `0x04EF::Fn_0x005C` (`0x0CE2BC`) | `0x0107::Fn_0x004D` (`0x00CBA4`), C++ `0x140b9b232` | `[pack_32(profession_did), pack_64(trainer_iid)]` | **Train Individual Profession (`ProgressionCommand::TrainCraft`).** Validates via `0x0401::Fn_0x0042`, then packs `profession_did: u32` followed by `trainer_iid: u64`. |
| C -> S | `0x84EF005D` | `0x04EF::Fn_0x005E` (`0x0CE308`) | `0x0107::Fn_0x004F` (`0x00CC84`), C++ `0x140b9b9eb` | `[pack_32(profession_did)]` | **Abandon Individual Profession.** Validates via `0x0401::Fn_0x0048`, then packs `profession_did: u32`. |
| C -> S | `0x84EF0042` | `0x04EF::Fn_0x0040` (`0x0CE054`) | `0x0107::Fn_0x001F` (`0x00BDEC`), C++ `0x140b9b0a2` | `[pack_32(craft_guild_did), pack_64(trainer_iid)]` | **Join / Change Craft Guild.** Packs `craft_guild_did: u32` followed by `trainer_iid: u64`. |
| C -> S | `0x84EF0032` | `0x04EF::Fn_0x0033` (`0x0CDEE0`) | `0x0107::Fn_0x0004` (`0x00B5D8`), C++ `0x140b9ba30` | `[pack_ref(StringInfo), pack_64(item_iid)]` | **Inscribe Crafted Item.** Validates `Craft_ItemCanBeInscribed` (`0x10000883`), then packs inscription `StringInfo` and `item_iid: u64`. |
| S -> C | `0x84EF0003` | `0x04EF::Fn_0x0003` (`0x0CDC40`) | `0x0107::Fn_0x002B` (`0x00C004`) | `[unpack_32, unpack_32, unpack_32]` (`result_type: u32, recipe_did: u32, profession_did: u32`) | **Craft / Recipe Result Notification (`ProgressionEvent::CraftResult`).** Invokes native `0x013A0006` (`VA=0x140b37c20`) with `(profession_did, recipe_did, result_type)`. |
| S -> C | `0x84EF0014` | `0x04EF::Fn_0x0014` (`0x0CDDC4`) | `0x0107::Fn_0x002C` (`0x00C024`) | `[unpack_32]` (`recipe_did: u32`) | **Recipe Learned (`ProgressionEvent::RecipeLearned`).** Invokes native `0x013A0008` (`VA=0x140b438b0`) when a recipe scroll is learned. |
| S -> C | `0x84EF0017` | `0x04EF::Fn_0x0017` (`0x0CDE0C`) | `0x0107::Fn_0x0032` (`0x00C090`) | `[unpack_32]` (`vocation_did: u32`) | **Vocation Changed (`ProgressionEvent::VocationChanged`).** Invokes native `0x013A000A` (`VA=0x140b43920`). |
| S -> C | `0x84EF001A` | `0x04EF::Fn_0x001A` (`0x0CDE54`) | `0x0107::Fn_0x0030` (`0x00C05C`) | `[unpack_64]` (`trainer_iid: u64`) | **Open Vocation / Craft Trainer UI (`ProgressionEvent::CraftTrainerShown`).** Invokes native `0x013A0012` (`VA=0x140b37c90`), reading `Craft_OfferedVocationArray` (`0x10000047`) on the trainer entity. |
| S -> C | `0x84EF0041` | `0x04EF::Fn_0x0041` (`0x0CE008`) | `0x0107::Fn_0x0039` (`0x00C0DC`) | `[unpack_64]` (`trainer_iid: u64`) | **Open Craft Guild Trainer UI.** Invokes native `0x013A0013` (`VA=0x140b37d00`). |
| S -> C | `0x84EF0057` | `0x04EF::Fn_0x0057` (`0x0CE204`) | `0x0107::Fn_0x0049` (`0x00CAD0`) | `[unpack_64]` (`trainer_iid: u64`) | **Open Individual Profession Trainer UI (`ProgressionEvent::CraftTrainerShown`).** Invokes native `0x013A0017` (`VA=0x140b37d70`), reading `Craft_OfferedProfessionsArray` (`0x10006FE9`) on the trainer entity. |
| S -> C | `0x84EF0022` | `0x04EF::Fn_0x0022` (`0x0CDE9C`) | `0x0107::Fn_0x0029` (`0x00BFCC`) | `[unpack_32]` (`active_flag: u32`) | **Craft Panel / Facility State Notification.** Invokes native `0x013A0001` (`VA=0x140b43720`). |
| S -> C | `0x84EF000B` | `0x04EF::Fn_0x000B` (`0x0CDD14`) | `0x0107::Fn_0x002A` (`0x00BFE8`) | `[unpack_32]` (`state_flag: u32`) | **Craft Execution Ready / State Flag.** Invokes native `0x013A0005` (`VA=0x140b43790`). |
| S -> C | `0x84EF000E` | `0x04EF::Fn_0x000E` (`0x0CDD84`) | `0x0107::Fn_0x002F` (`0x00C040`) | `[unpack_32]` (`induction_flag: u32`) | **Craft Induction State (`0` = stopped, non-zero = active).** Invokes native `0x013A0007` (`VA=0x140b43800`). |
| S -> C | `0x84EF002E` | `0x04EF::Fn_0x002E` (`0x0CDF54`) | `0x0107::Fn_0x0036` (`0x00C0AC`) | `[]` (empty) | **Craft Induction Completed.** Invokes native `0x013A0010` (`VA=0x140b3fc60`). |
| S -> C | `0x84EF003C` | `0x04EF::Fn_0x003C` (`0x0CDFD4`) | `0x0107::Fn_0x0037` (`0x00C0C4`) | `[]` (empty) | **Craft Induction Interrupted / Failed.** Invokes native `0x013A0011` (`VA=0x140b3fcd0`). |

---

## 6. Clean-Room Specification Section (Ready for `docs/specs/official-messages-gameplay.md`)

<a id="progression-traits-crafting-reputation-titles"></a>
### Progression, Experience, Trait Trees, Trainers, Crafting, Reputation, and Titles

#### 1. Overview and Subsystem Interfaces

Character progression, skill and trait acquisition, crafting, faction standing, and titles operate over six WLib RPC interfaces and entity property bags (`OBS-gamelogic`, `BIN-client64`):

| Subsystem | WLib RPC Interface ID | Wire `function_id` Prefix | Primary Client State / Data Classes |
|---|---|---|---|
| Advancement, Leveling & Skill Trainers | `0x04DF` | `0x84DFxxxx` | Skill registry (`0x0340`), Skill pool (`0x033F`), Class level table (`0x0355`), Advancement table (`0x0351`) |
| Faction Reputation | `0x077B` | `0x877Bxxxx` | Faction definition (`0x0761`), Advancement table (`0x0351`) |
| Titles | `0x052E` | `0x852Exxxx` | Title registry (`0x05C0`), Title definition (`0x05BE`) |
| Traits, Virtues & Trait Slotting | `0x0532` | `0x8532xxxx` | Trait registry (`0x05D0`), Trait slot descriptor (`0x05D1`) |
| Point-Based Trait Trees & Configurations | `0x0E15` | `0x8E15xxxx` | Trait configuration system (`0x0E05` / `0x0E16`) |
| Crafting, Professions, Vocations & Recipes | `0x04EF`, `0x0528` | `0x84EFxxxx`, `0x85280005` | Craft execution descriptor (`0x0A4A`), Profession (`0x03FF`), Vocation (`0x0402`), Recipe (`0x0400`), Craft Guild (`0x08E6`) |

---

#### 2. Experience, Leveling, Skill Pools, and Class Trainers (`0x04DF`)

##### 2.1 Experience and Level Properties (`ExperienceChanged`)

Character experience, rest bonus XP, and level caps are synchronized through standard entity property updates on the local player entity and world property state (`OBS-gamelogic`, `BIN-client64`):

| Property ID | Property Name | Value Type | Scope | Meaning |
|---|---|---|---|---|
| `0x10001011` | `Advancement_Level` | `i32` | Player entity | Current character level. |
| `0x10000D33` | `XP_EarnedXP` | `i64` | Player entity | Cumulative total earned character experience points. |
| `0x10000CC8` | `XP_BonusXP` | `i64` | Player entity | Remaining rest / bonus experience pool. |
| `0x1000370B` | `XP_DisableXP` | `u32` (bool) | Player entity | Non-zero when character XP gain is disabled. |
| `0x10001631` | `World_NonAdminPlayerLevelCap` | `i32` | World | Maximum attainable player level on the server. |
| `0x10000A89` | `XP_EarnedPointsToLevel_Table` | `u32` (DID) | Class `LevelTable` (`0x0355`) | Data ID of the `AdvancementTable` (`0x0351`) mapping character level (`u32`) to cumulative required XP (`i64`). |
| `0x1000017A` | `Advancement_MonsterPlayXP` | `i64` | Player entity | Cumulative Monster Play experience points. |
| `0x10003715` | `MonsterPlay_CurrentFreePeepXP` | `i64` | Player entity | Free Peoples PvMP experience points. |

**Within-level XP calculation:**
To compute within-level progress from `XP_EarnedXP` (`0x10000D33`) and `Advancement_Level` (`0x10001011`):
1. Look up the class `LevelTable` (`0x0355`) for the player's class and read `XP_EarnedPointsToLevel_Table` (`0x10000A89`) to obtain the `AdvancementTable` (`0x0351`).
2. `xp_into_current_level = XP_EarnedXP - table[current_level]`.
3. `xp_for_next_level = table[current_level + 1] - table[current_level]`.

##### 2.2 Skill Pools and Skill Trainer Wire Messages (`0x84DFxxxx`)

The player's `SkillRegistry` (`0x0340`) maintains four `SkillPool` (`0x033F`) containers indexed by `pool_type: u32` (`OBS-gamelogic`):
- `1`: Trained skills (`SkillLearned` / `SkillForgotten`)
- `2`: Conjunction skills (fellowship manoeuvres)
- `3`: Combo / response skills
- `4`: Gambit skills

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| S -> C | `0x84DF001E` | `skill_pool: ClassRef(0x033F)` | Replaces the player's `SkillPool` for `skill_pool.pool_type`. The client diffs the new pool against the previous pool of that type, emitting `SkillForgotten` for removed skill DIDs and `SkillLearned` for newly added skill DIDs. |
| S -> C | `0x84DF0003` | `skill_entry: ClassRef`, `pool_type: u32` | Inserts or updates a single skill entry inside `pool_type` (`1..4`), emitting `SkillLearned`. |
| S -> C | `0x84DF0016` | `old_level: u32`, `new_level: u32` | Notifies the client that the player's character level changed from `old_level` to `new_level`. |
| S -> C | `0x84DF001B` | `trainer_iid: u64` | Opens the Class / Skill Trainer dialog for the NPC identified by `trainer_iid` (`TrainerShown`). |
| S -> C | `0x84DF0010` | `skill_did: u32` | Displays a skill-training failure/notification alert for `skill_did`. |
| C -> S | `0x84DF0005` | `skill_did: u32`, `trainer_iid: u64` | Requests training/purchase of `skill_did` from the trainer NPC `trainer_iid` (`TrainSkill`). |
| C -> S | `0x84DF001A` | `trait_did: u32`, `trainer_iid: u64` | Requests training/purchase of characteristic/trait `trait_did` from trainer NPC `trainer_iid`. |
| C -> S | `0x84DF002D` | `item_or_skill_did: u32`, `trainer_iid: u64` | Requests secondary trainer advancement purchase `item_or_skill_did` from `trainer_iid`. |

**Untrained Buyable Skills (`UntrainedSkills`):**
The list of skills available for purchase at a trainer is derived client-side by enumerating the trainable entries (`0x0354`) in the player's class `LevelTable` (`0x0355`) and excluding skills already present in the player's `SkillRegistry` (`0x0340`).

---

#### 3. Faction Reputation (`0x077B`)

##### 3.1 Faction Discovery and Action RPCs (`0x877Bxxxx`)

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| S -> C | `0x877B0007` | `faction_did: u32` | Initializes faction `faction_did` (`Class 0x0761`) on the client and subscribes to the faction's dynamic player properties. |
| C -> S | `0x877B0008` | `action_id: u32`, `faction_did: u32` | Sends a faction action request for `faction_did`. |

##### 3.2 Dynamic Per-Faction Reputation Properties (`ReputationChanged`)

Each `Faction` (`0x0761`) data record defines the specific player property IDs used to track standing and earned points for that faction (`OBS-gamelogic`, `BIN-client64`):

| Faction Record Property ID | Property Name | Value Type | Meaning |
|---|---|---|---|
| `0x1000177D` | `Reputation_Faction_CurrentTier_PropertyName` | `u32` (Property ID) | The Property ID on the player entity (`u32` value) storing the player's current standing tier for this faction. |
| `0x1000177E` | `Reputation_Faction_EarnedReputation_PropertyName` | `u32` (Property ID) | The Property ID on the player entity (`i64` value) storing the player's cumulative earned reputation points for this faction. |
| `0x1000189D` | `Reputation_Faction_GlobalCap_PropertyName` | `u32` (Property ID) | The world Property ID (`u32` value) storing the server-wide tier cap for this faction. |
| `0x10001762` | `Reputation_Faction_AdvancementTable` | `u32` (DID) | Data ID of the `AdvancementTable` (`0x0351`) mapping standing tier (`u32`) to cumulative reputation points (`i64`). |
| `0x10001757` | `Reputation_Faction_DefaultTier` | `u32` | Default standing tier when the faction is first discovered. |
| `0x1000175B` | `Reputation_LowestTier` | `u32` | Minimum standing tier for the faction. |
| `0x1000175C` | `Reputation_HighestTier` | `u32` | Maximum standing tier for the faction. |
| `0x10003E94` | `Reputation_BonusRep_BonusRemaining` | `i64` | Player entity property holding remaining bonus reputation points. |

---

#### 4. Titles (`0x052E`)

##### 4.1 Title Wire Messages (`0x852Exxxx`) and Properties

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| S -> C | `0x852E0005` | `timestamp: f64`, `new_title_did: u32`, `replaced_title_did: u32` | Grants `new_title_did` (`Class 0x05BE`) to the player (`TitleGranted`). If `new_title_did` belongs to an equivalence group (`0x100010A4`), any lower-ranked title (`0x100010A3`) in that group is replaced in `TitleRegistry` (`0x05C0`). |
| S -> C | `0x852E0013` | `title_did: u32` | Revokes `title_did` from the player's `TitleRegistry`. |
| S -> C | `0x852E0016` | *(empty)* | Clears all titles from the player's `TitleRegistry`. |
| C -> S | `0x852E000E` | `title_did: u32` | Selects `title_did` as the player's active displayed title. |
| C -> S | `0x852E0002` | *(empty)* | Clears the player's active displayed title. |

**Entity Properties:**
- `Title_ActiveTitleDID` (`0x1000088B`, `u32`): Active title Data ID on the player entity (`0` when no title is active).
- `Title_ActiveTitleString` (`0x1000119C`, `StringInfo`): Resolved active title string on character entities.

---

#### 5. Traits, Virtues, Slotting, and Trait Trees (`0x0532`, `0x0E15`)

##### 5.1 Trait Slot Descriptor (`Class 0x05D1`)

When a `ClassRef(0x05D1)` instance is serialized into a `WLibArgFrame`, its body after the class-ref header consists of four little-endian `u32` fields (`OBS-gamelogic`):

| Offset (in `0x05D1` payload) | Size | Type | Field Name | Meaning |
|---|---|---|---|---|
| `+0x00` | 4 | `u32` | `tier` | Trait tier (`0` when `has_tier == 0`). |
| `+0x04` | 4 | `u32` | `has_tier` | Boolean flag (`0` or `1`) indicating whether `tier` is specified. |
| `+0x08` | 4 | `u32` | `index` | 0-based slot index within the trait nature category. |
| `+0x0C` | 4 | `u32` | `has_index` | Boolean flag (`0` or `1`) indicating whether `index` is specified (`1` when slotting a trait by index). |

##### 5.2 Trait Slotting and Virtue Wire Messages (`0x8532xxxx`)

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| S -> C | `0x85320013` | `slot_desc: ClassRef(0x05D1)`, `trait_did: u32` | Slots `trait_did` into `slot_desc` (`TraitSlotted`). |
| S -> C | `0x8532000B` | `slot_desc: ClassRef(0x05D1)`, `trait_did: u32` | Force-places `trait_did` into `slot_desc` (`TraitSlotted`). |
| S -> C | `0x85320003` | `trait_did: u32` | Unslots `trait_did`. |
| S -> C | `0x85320009` | `trait_nature: u32` | Unslots all traits belonging to `trait_nature`. |
| S -> C | `0x85320002` | *(empty)* | Unslots all traits across all natures. |
| S -> C | `0x85320010` | `new_rank: u32`, `trait_did: u32` | Updates the rank of `trait_did` (e.g., Virtue rank-up) to `new_rank` (`TraitRank`). |
| S -> C | `0x8532001D` | `notify_flag: u32`, `trait_did: u32` | Grants earned trait `trait_did` to the player's `TraitRegistry`. |
| S -> C | `0x85320020` | `trait_did: u32` | Removes earned trait `trait_did` from the player's `TraitRegistry`. |
| S -> C | `0x85320056` | `notify_flag: u32`, `trait_dids: ClassRef(Array<u32>)` | Grants a batch of earned traits to the player's `TraitRegistry`. |
| S -> C | `0x85320054` | `trait_dids: ClassRef(Array<u32>)` | Removes a batch of earned traits from the player's `TraitRegistry`. |
| C -> S | `0x85320016` | `slot_desc: ClassRef(0x05D1)`, `trait_did: u32` | Requests slotting `trait_did` into `slot_desc` (`SlotTrait`). |
| C -> S | `0x8532000E` | `trait_did: u32` | Requests unslotting `trait_did`. |
| C -> S | `0x8532003D` | `slot_map: ClassRef(HashTable<u32, 0x05D1>)`, `trait_nature: u32` | Commits a batch of slotted traits for `trait_nature`. |
| C -> S | `0x85320045` | *(empty)* | Requests clearing/resetting all slotted traits. |
| C -> S | `0x85320049` | `slot_arg: u32` | Requests unlocking/purchasing a trait slot. |
| C -> S | `0x8532004B` | `config_slot: u32`, `slot_desc: ClassRef(0x05D1)`, `trait_did: u32` | Requests slotting `trait_did` into `slot_desc` within configuration `config_slot`. |
| C -> S | `0x8532004F` | `config_slot: u32`, `trait_did: u32` | Requests unslotting `trait_did` from configuration `config_slot`. |
| C -> S | `0x85320057` | `virtue_trait_did: u32` | Sets `virtue_trait_did` as the active earning Virtue. |

##### 5.3 Point-Based Trait Trees and Configurations (`0x8E15xxxx`)

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| C -> S | `0x8E15000F` | `rank_map: ClassRef(HashTable<u32, u32>)`, `slotted_map: ClassRef(HashTable)`, `branch: u32`, `tree_type: u32`, `config_slot: u32` | Commits a point-based specialization branch (`ChooseTraitBranch`) and cell rank allocations (`SetTraitRank`, mapping `trait_did -> rank`) for `config_slot`. |
| C -> S | `0x8E150001` | `tree_type: u32`, `config_slot: u32` | Switches the active trait configuration to `config_slot`. |
| C -> S | `0x8E150005` | `slotted_map: ClassRef(HashTable)`, `branch: u32`, `tree_type: u32`, `config_slot: u32` | Commits slotted traits and specialization branch for `config_slot`. |
| C -> S | `0x8E150009` | `config_slot: u32` | Purchases/unlocks trait configuration slot `config_slot`. |
| C -> S | `0x8E15000B` | `name: ClassRef(StringInfo)`, `tree_type: u32`, `config_slot: u32` | Renames trait configuration slot `config_slot`. |
| S -> C | `0x8E15000E` | `config_data: ClassRef`, `config_slot: u32` | Updates client trait tree configuration state for `config_slot` (`TraitTreeChanged`). |
| S -> C | `0x8E150011` | `status_or_slot: u32` | Acknowledges trait configuration commit. |
| S -> C | `0x8E150013` | `status_or_slot: u32` | Notifies trait configuration reset or status change. |

##### 5.4 Trait Point and Specialization Properties (`TraitPoints`, `TraitTreeChanged`)

| Property ID | Property Name | Value Type | Meaning |
|---|---|---|---|
| `0x1000201C` | `Trait_ClassSpecialization_ActiveBranch` | `u32` (Enum) | Active class specialization branch ID. |
| `0x10002B90` | `Trait_PointBasedTraitTree_Branch` | `u32` (Enum) | Specialization branch ID in the active point-based trait tree. |
| `0x10002B8F` | `Trait_PointBasedTraitTree_RankArray` | `Array` | Allocated trait ranks in the active point-based trait tree. |
| `0x10002B91` | `Trait_PointBasedTraitTree_PointsSpent` | `i32` | Trait points currently spent. |
| `0x10002B92` | `Trait_PointBasedTraitTree_PointsTotal` | `i32` | Total available trait points (`PointsEarned + PointsBonus`). |
| `0x10002B93` | `Trait_PointBasedTraitTree_PointsEarned` | `i32` | Earned trait points. |
| `0x10002B94` | `Trait_PointBasedTraitTree_PointsBonus` | `i32` | Bonus trait points. |
| `0x1000399C` | `Trait_Virtue_ActiveTrait` | `u32` (DID) | Active Virtue trait DID currently earning Virtue XP. |
| `0x10003999` | `Trait_Virtue_BonusXP` | `i32` | Bonus Virtue XP pool. |

---

#### 6. Crafting, Professions, Vocations, Recipes, and Craft Trainers (`0x04EF`, `0x0528`)

##### 6.1 Craft Execution Descriptor (`Class 0x0A4A`)

When the player initiates crafting (`0x85280005`), the client serializes a `ClassRef(0x0A4A)` instance containing four fields in wire order (`OBS-gamelogic`, `BIN-client64`):

| Field Order | Type | Field Name | Meaning |
|---|---|---|---|
| 1 | `u32` | `multi_output_index` | `0` for single-output recipes; `1+` (1-based index) for multi-output recipes. |
| 2 | `ClassRef(Array<u32>)` | `optional_ingredients` | Array of `u32` boolean flags (`0` or `1`) corresponding 1-to-1 with the recipe's optional ingredient slots (e.g., critical chance journals/scrolls). |
| 3 | `u32` | `use_ingredient_pack` | `0` to consume normal inventory ingredients; `1` to consume universal/account ingredient packs. |
| 4 | `u32` | `tier` | Crafting tier identifier for the recipe being executed. |

*Note on active recipe and batch quantity:* The recipe being crafted is stored in the player property `Craft_CurrentRecipe` (`0x10001476`, `u32` DID). Multi-item batch crafting executes one craft induction cycle per `0x85280005` request, decrementing the client batch counter on each completion (`0x84EF002E`) and issuing the next `0x85280005` until the batch counter reaches zero or is interrupted (`0x84EF003C`).

##### 6.2 Crafting, Profession, and Trainer Wire Messages (`0x85280005`, `0x84EFxxxx`)

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| C -> S | `0x85280005` | `craft_desc: ClassRef(0x0A4A)` | Starts crafting execution for the current recipe (`Craft`). |
| C -> S | `0x84EF0006` | `vocation_did: u32`, `trainer_iid: u64` | Trains/adopts vocation `vocation_did` at craft trainer `trainer_iid` (`TrainCraft`). |
| C -> S | `0x84EF000D` | `confirm_text: ClassRef(StringInfo)`, `trainer_iid: u64` | Abandons current vocation at craft trainer `trainer_iid`. |
| C -> S | `0x84EF005B` | `profession_did: u32`, `trainer_iid: u64` | Trains standalone profession `profession_did` at craft trainer `trainer_iid` (`TrainCraft`). |
| C -> S | `0x84EF005D` | `profession_did: u32` | Abandons standalone profession `profession_did`. |
| C -> S | `0x84EF0042` | `craft_guild_did: u32`, `trainer_iid: u64` | Joins crafting guild `craft_guild_did` at guild trainer `trainer_iid`. |
| C -> S | `0x84EF0032` | `inscription: ClassRef(StringInfo)`, `item_iid: u64` | Inscribes a custom name onto crafted item `item_iid`. |
| S -> C | `0x84EF0003` | `result_type: u32`, `recipe_did: u32`, `profession_did: u32` | Reports crafting outcome (`CraftResult`: success, critical success, or failure) for `recipe_did` under `profession_did`. |
| S -> C | `0x84EF0014` | `recipe_did: u32` | Notifies the client that `recipe_did` was learned (`RecipeLearned`). |
| S -> C | `0x84EF0017` | `vocation_did: u32` | Notifies the client that the player's vocation changed to `vocation_did` (`VocationChanged`). |
| S -> C | `0x84EF001A` | `trainer_iid: u64` | Opens the Vocation Trainer UI for `trainer_iid` (`CraftTrainerShown`). Offered vocations are read from `Craft_OfferedVocationArray` (`0x10000047`) on the trainer entity. |
| S -> C | `0x84EF0057` | `trainer_iid: u64` | Opens the Profession Trainer UI for `trainer_iid` (`CraftTrainerShown`). Offered professions are read from `Craft_OfferedProfessionsArray` (`0x10006FE9`) on the trainer entity. |
| S -> C | `0x84EF0041` | `trainer_iid: u64` | Opens the Craft Guild Trainer UI for `trainer_iid`. |
| S -> C | `0x84EF0022` | `active_flag: u32` | Updates crafting facility / panel active state (`0` or `1`). |
| S -> C | `0x84EF000B` | `state_flag: u32` | Updates crafting execution state flag (`0` or `1`). |
| S -> C | `0x84EF000E` | `induction_flag: u32` | Starts (`!= 0`) or stops (`0`) the crafting induction bar. |
| S -> C | `0x84EF002E` | *(empty)* | Signals completion of the current crafting induction cycle. |
| S -> C | `0x84EF003C` | *(empty)* | Signals cancellation or interruption of the current crafting induction cycle. |

##### 6.3 Vocation, Profession, and Recipe Properties (`ProfessionChanged`, `RecipeForgotten`)

| Property ID | Property Name | Owner | Value Type | Meaning |
|---|---|---|---|---|
| `0x10001481` | `Craft_Vocation` | Player | `u32` (DID) | Active `CraftVocation` (`0x0402`) DID (`0` when using standalone professions). |
| `0x10006F93` | `Craft_ProfessionArray` | Player | `Array<u32>` | Trained `CraftProfession` (`0x03FF`) DIDs on the player. |
| `0x100067BF` | `Craft_CraftGuild_Membership_Array` | Player | `Array<u32>` | `CraftGuild` (`0x08E6`) DIDs the player has joined. |
| `0x10001476` | `Craft_CurrentRecipe` | Player | `u32` (DID) | Active `CraftRecipe` (`0x0400`) DID selected for crafting. |
| `0x1000079B` | `CraftProfession_Enabled_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`bool`) indicating whether this profession is trained. |
| `0x10000CF1` | `CraftProfession_ProficiencyLevel_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`u32`) for the profession's current proficiency tier. |
| `0x10000B32` | `CraftProfession_MasteryLevel_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`u32`) for the profession's current mastery tier. |
| `0x100010BC` | `CraftProfession_ProficiencyXP_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`i32`) for earned proficiency XP in the current tier. |
| `0x10001296` | `CraftProfession_MasteryXP_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`i32`) for earned mastery XP in the current tier. |
| `0x100026E9` | `CraftProfession_OpenLevel_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`u32`) for the highest unlocked crafting tier. |
| `0x100006C6` | `CraftProfession_ExtraRecipeArray_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`Array<u32>`) holding learned extra recipe DIDs (`RecipeLearned` / `RecipeForgotten`). |
| `0x1000004B` | `CraftProfession_CriticalChanceAddModifier_PropertyName` | `CraftProfession` (`0x03FF`) | `u32` (PID) | Player Property ID (`f32`) for additive crafting critical chance. |

---

#### 7. Synthetic Test Vectors

##### Vector 1: C->S `TrainSkill` (`0x84DF0005`)
Training skill DID `0x70001234` at trainer entity IID `0x020000000000ABCD`:
- `function_id`: `0x84DF0005` (`u32` LE: `05 00 DF 84`)
- `arg_data`:
  - `skill_did`: `0x70001234` (`34 12 00 70`)
  - `trainer_iid`: `0x020000000000ABCD` (`CD AB 00 00 00 00 00 02`)
- `arg_size`: `12` (`0x0000000C`)

##### Vector 2: S->C `TitleAwarded` (`0x852E0005`)
Granting title DID `0x700055AA` (replacing `0x00000000`) at timestamp `1000.0` (`0x408F400000000000`):
- `function_id`: `0x852E0005` (`u32` LE: `05 00 2E 85`)
- `arg_data`:
  - `timestamp`: `1000.0` (`f64` LE: `00 00 00 00 00 40 8F 40`)
  - `new_title_did`: `0x700055AA` (`u32` LE: `AA 55 00 70`)
  - `replaced_title_did`: `0x00000000` (`u32` LE: `00 00 00 00`)
- `arg_size`: `16` (`0x00000010`)

##### Vector 3: S->C `CraftResult` (`0x84EF0003`)
Reporting `result_type = 2` (Critical Success) for `recipe_did = 0x70008899` in `profession_did = 0x70000101`:
- `function_id`: `0x84EF0003` (`u32` LE: `03 00 EF 84`)
- `arg_data`:
  - `result_type`: `2` (`u32` LE: `02 00 00 00`)
  - `recipe_did`: `0x70008899` (`u32` LE: `99 88 00 70`)
  - `profession_did`: `0x70000101` (`u32` LE: `01 01 00 70`)
- `arg_size`: `12` (`0x0000000C`)

---

## 7. Change Log

| Date | Author | Summary |
|---|---|---|
| 2026-10-21 | Spec writer (dirty room) | Initial P6 progression, experience, traits, trainers, crafting, reputation, and titles specification from `client_gamelogic.dat` and `lotroclient64.exe`. |
