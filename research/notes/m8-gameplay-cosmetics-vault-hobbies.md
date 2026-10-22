<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 Dirty-Room Research Note: Cosmetic Outfits, Wardrobe, Vault/Shared Storage, Carry-Alls, Player Inspection, Skirmish Traits/Soldiers, & Hobbies

* **Date:** 2026-10-21
* **Author:** Spec writer (dirty room)
* **Scope:** Wire-level WLib RPCs (`0x00D9000C` / Entity `0x30` RPC), `WLibArgFrame` (`0x008E`) serialization layouts, native `BankAddress` (`0x0E9C`) framing, and Master Property Definition (`0x34000000`) Property IDs for:
  1. Cosmetic Outfits (`OutfitSystem`), Equipment Slot Visibility, Barber (`AppearanceUI`), Wardrobe (`WardrobeSystem`), and Mounted Combat Cosmetics (`MountCosmetics`).
  2. Character Vault, Account Shared Storage, Housing Chests (`BankRepositorySystem`), Carry-All Bags (`BagOfSorts` / `BoS`), and Inventory/Container operations (`CInventorySystem` / `CMEOInventorySystem`).
  3. Player Inspection (`Inspect`, `CExaminationSystem`, `Bio`), Trait & Skirmish Soldier Configuration (`CTraitSystem`, `TraitConfiguration`, `CSkirmishSystem`, `WorldJoin`), and Hobbies — Fishing & Birding (`HobbySystem`).
* **Sources:**
  * **OBS (M1):** Read-only extraction of `client_highres.dat` Master Property Definition (`0x34000000`, 30,278 property definitions) and Embedded WLib Bytecode (`0x30000000`, 5,320 classes).
  * **BIN (M4):** Static analysis of `lotroclient64.exe` (SHA-256: `b08fb0ab2fb67ccb9acd2dc9ddeac1ebdc6fd6bbdb63fc3dc7adbfebff6d67a5`, Ghidra 11.3.2) for native C++ UI-to-WLib dispatch (`WLib::CallStatic`) and native serializer packages (`0x008E`, `0x00D6`, `0x00D9`, `0x0E9C`).

---

## Part I — Dirty-Room Reverse Engineering Findings

### 0. Common Wire Transport & `WLibArgFrame` Serialization

Every client-to-server (C→S) and server-to-client (S→C) RPC across the systems in this note uses the entity-targeted WLib RPC transport (`NetBlob` event `0x30`, dispatched on the local player avatar entity retrieved via `CALL.NATIVE 0x00E70001`):

* **C→S Dispatch:**
  1. Construct an argument frame via `NEW.NATIVE class=142 (0x008E)` (`WLibArgFrame`).
  2. Push arguments in exact bytecode execution order using `0x008E` serializer natives:
     * `0x008E0001` (`WriteRef`): Serializes a WLib object reference, `PropertyCollection` (`class=39 / 0x0027`), `WString` (`class=199 / 0x00C7`), `Array` (`class=35 / 0x0023`), or `BankAddress` (`class=3740 / 0x0E9C`).
     * `0x008E0002` (`WriteI32`): Appends a 32-bit unsigned/signed integer (`u32` / `i32`, little-endian) — used for DIDs, PIDs, slot bitmasks, enum values, quantities, and boolean flags (`0` or `1`).
     * `0x008E0003` (`WriteF32`): Appends a 32-bit IEEE-754 float (`f32`, little-endian).
     * `0x008E0004` (`WriteI64`): Appends a 64-bit unsigned integer (`u64`, little-endian) — used for `InstanceID` (`IID`).
     * `0x008E000C` (`WriteProperty`): Appends a single typed `Property` value.
  3. Invoke `CALL.NATIVE 0x00D9000C (this_entity, rpc_id, arg_frame)` (`SendRPC_C`), where `rpc_id = 0x80000000 | (wlib_class_id << 16) | method_idx`.

* **S→C Dispatch:**
  1. Server delivers an entity-targeted `0x30` RPC carrying `rpc_id = 0x80000000 | (wlib_class_id << 16) | method_idx`.
  2. The WLib handler validates the message context via `CALL.NATIVE 0x00D60007 (this, 0x06C3)` (`GetRPCArgFrame`), returning status `60 (0x003C)` on mismatch.
  3. If arguments are present, initializes reading via `CALL.NATIVE 0x008E000D (arg_frame)` (`InitUnpackFrame`) and reads fields sequentially using:
     * `0x008E0006` (`ReadRef`): Deserializes a reference/object (`PropertyCollection`, `WString`, `Array`, `Map`, or `BankAddress`).
     * `0x008E0007` (`ReadI32`): Reads a 32-bit integer (`u32` / `i32`, little-endian).
     * `0x008E0008` (`ReadF32`): Reads a 32-bit float (`f32`, little-endian).
     * `0x008E0009` (`ReadI64`): Reads a 64-bit integer (`u64` `IID`, little-endian).
     * `0x008E000B` (`ReadProperty`): Reads a single typed `Property`.

---

### 1. Domain 1: Cosmetic Outfits, Equipment Visibility, Barber (`AppearanceUI`), Mounted Combat Cosmetics, & Wardrobe

#### 1.1 `OutfitSystem` (`RPC Class 2075 / 0x081B` ↔ `System Class 2073 / 0x0819`)

* **WLib Class Hierarchy:**
  * `Class 2075 (0x081B)`: Network RPC interface (`parent=2071 / 0x0817`).
  * `Class 2073 (0x0819)`: Client `OutfitSystem` (`parent=2075 / 0x081B`), invoked directly by C++ UI glue (`OutfitControl`, `Class 2085 / 0x0825`).
  * `Class 2070 (0x0816)`: Static outfit slot & visibility helper methods (`boff=0x16BFA0..0x16C718`).
* **Native C++ UI Glue (`lotroclient64.exe`):**
  * `0x140b990b6`: Calls `WLib::CallStatic(0x08190010)` (`SelectOutfit`).
  * `0x140b9110f`: Calls `WLib::CallStatic(0x08190016)` (`OutfitEquip` from inventory).
  * `0x140b911ad`: Calls `WLib::CallStatic(0x08190063)` (`OutfitEquip` from Wardrobe).
  * `0x140b91262`: Calls `WLib::CallStatic(0x08190037)` (`OutfitUnEquip`).
  * `0x140b9988a`: Calls `WLib::CallStatic(0x08190058)` (`SetSlotVisibility`).
  * `0x140B3A290` (`0x08220001`): Native UI callback for `RecvSelectOutfitFailed`.
  * `0x140B3A330` (`0x08220002`): Native UI callback for `RecvSlotVisibilityResult`.
* **Client Gating & Antispam Checks (`Class 2073 / 0x0819`):**
  * Before sending any outfit RPC, `0x08190049` checks antispam cooldown property `0x10001C4C` (`Antispam_OutfitRequest`) via `0x04CE0002`.
  * `0x0819003E` (`IsOutfitSystemEnabled`) checks `0x100028FA` (`Account_Outfit_System_Available`), `0x10001C65` (`Outfit_AreaEnabled`), and player level (`0x10000049` `Advancement_Level`) against `0x10001C52` (`Outfit_MinLevel`).
  * Active outfit index is stored on the avatar in `0x10001C64` (`Outfit_ActiveOutfitType`), where `0` = Equipment Outfit (`0x10001C61` `Outfit_EquipOutfit`) and `1..N` = Cosmetic Outfit `1..N` (`0x10001C5C` `Outfit_Outfit1`, `0x10001C5E` `Outfit_Outfit2`, etc., mapped in `0x10006B0C` `OutfitControl_OutfitTypeList`).
  * Per-outfit slot visibility bitmasks are mapped via `0x10006B0E` (`OutfitControl_OutfitTypeVisibilityMap`, mapping `outfit_type` → visibility PID such as `0x10001C69` `Outfit_EquipOutfitSlotVisibility`, `0x10001C55` `Outfit_Outfit1SlotVisibility`, `0x10001C4A` `Outfit_Outfit2SlotVisibility`, `0x10003AD5..0x10003AD9` `Outfit_Outfit3..7SlotVisibility`, `0x10004FC5` `Outfit_Outfit8SlotVisibility`). The default all-visible mask is `0x01FFFFFE` (`268435454`, stored in `0x10001C5D` `Outfit_AllSlotVisibilityFlags`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes / Validation |
|---|---|---|---|---|---|
| `0x881B0005` | C→S | `0x081B0006` (`boff=0x108480`) ← `0x08190013` | `SelectOutfit` | 1. `outfit_type: u32` (`WriteI32`) | Switches active outfit (`0` = Equipment, `1..N` = Cosmetic Outfit `1..N`). Optimistically sets `0x10001C64` (`Outfit_ActiveOutfitType`). |
| `0x881B000A` | S→C | `0x081B000A` (`boff=0x108420`) → `0x08190023` | `RecvSelectOutfitFailed` | 1. `reverted_outfit_type: u32` (`ReadI32`) | Reverts `0x10001C64` (`Outfit_ActiveOutfitType`) to `reverted_outfit_type` and notifies UI (`0x08220001`). |
| `0x881B0007` | C→S | `0x081B0008` (`boff=0x108330`) ← `0x08190019` | `OutfitEquip` | 1. `wardrobe_item_props: PropertyBagRef` (`WriteRef`)<br>2. `item_iid: u64` (`WriteI64`)<br>3. `slot_bitmask: u32` (`WriteI32`)<br>4. `outfit_type: u32` (`WriteI32`) | **Inventory item:** `item_iid != 0`, `wardrobe_item_props = null`.<br>**Wardrobe item (`0x08190063`):** `item_iid = 0`, `wardrobe_item_props` = `PropertyCollection` (`0x10002F15` `Wardrobe_ItemStruct`) containing `0x10002F13` (`Wardrobe_ItemDID`: `u32`), `0x10002F12` (`Wardrobe_ItemClothingColor`: `f32`), and `0x10003AAD` (`Wardrobe_BankerIID`: `u64`). |
| `0x881B000B` | C→S | `0x081B000C` (`boff=0x1083B8`) ← `0x08190039` | `OutfitUnEquip` | 1. `slot_bitmask: u32` (`WriteI32`)<br>2. `outfit_type: u32` (`WriteI32`) | Clears the cosmetic slot identified by `slot_bitmask` in `outfit_type` (`1..N`). |
| `0x881B000D` | C→S | `0x081B000E` (`boff=0x1084D8`) ← `0x08190053` | `SetSlotVisibility` | 1. `b_visible: u32` (`WriteI32`)<br>2. `slot_bitmask: u32` (`WriteI32`)<br>3. `outfit_type: u32` (`WriteI32`) | Toggles visibility (`b_visible`: `0` = hidden, `1` = visible) for `slot_bitmask` (head, cloak/back, shoulders, gloves, boots, chest, leggings, weapons) on `outfit_type` (`0` = all outfits / equipment synchronization per `0x0816000D`). |
| `0x881B0010` | S→C | `0x081B0010` (`boff=0x1082A8`) → `0x0819005E` | `RecvSlotVisibilityResult` | 1. `b_visible: u32` (`ReadI32`)<br>2. `slot_bitmask: u32` (`ReadI32`)<br>3. `outfit_type: u32` (`ReadI32`) | Confirms or rolls back the slot visibility bitmask in `0x10006B0E` and triggers `0x08220002`. |

#### 1.2 Barber / `AppearanceUI` (`RPC Class 2106 / 0x083A` ↔ `System Class 2103 / 0x0837`)

* **Native C++ UI Glue (`lotroclient64.exe`):**
  * `0x140b621c9`: Calls `WLib::CallStatic(0x0837000D)` (`CommitAppearanceChanges` in `"AppearanceUI"`).
  * `0x08380001..0x08380003`: Native UI handlers for opening/updating/closing the Barber UI.
* **Cost Calculation (`0x0837000A`):** Compares requested properties in `appearance_props` against the avatar's current properties via `0x1000253A` (`AppearanceUI_ModifiableProperties`), multiplying avatar level by per-category cost modifiers (`0x10002538` `AppearanceUI_Haircut_Modifier`, `0x10002539` `AppearanceUI_Detail_Modifier`, `0x10002537` `AppearanceUI_Color_Modifier`) and `0x10002536` (`AppearanceUI_Level_Cost`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x883A0003` | S→C | `0x083A0003` (`boff=0x104FD0`) → `0x08370008` | `OpenBarberUI` | 1. `barber_npc_iid: u64` (`ReadI64`) | Opens Barber UI bound to `barber_npc_iid`. |
| `0x883A0001` | C→S | `0x083A0004` (`boff=0x105088`) ← `0x0837000D` | `CommitAppearanceChanges` | 1. `appearance_props: PropertyBagRef` (`WriteRef`) | Submits `PropertyCollection` (`0x0027`) of modified avatar appearance properties. |
| `0x883A0005` | S→C | `0x083A0005` (`boff=0x105030`) → `0x0837000F` | `CloseBarberUI` | *(0 arguments)* | Closes the Barber UI (`0x08380003`). |

#### 1.3 `WardrobeSystem` (`RPC Class 2629 / 0x0A45` ↔ `System Class 2627 / 0x0A43`)

* **WLib Class Hierarchy:**
  * `Class 2629 (0x0A45)`: Network RPC interface (`parent=2626 / 0x0A42`).
  * `Class 2627 (0x0A43)`: Client `WardrobeSystem` (`parent=2629 / 0x0A45`).
  * `Class 2666 (0x0A6A)` & `Class 2675 (0x0A73)`: Wardrobe item removal (`RemoveWardrobeItem`) and dye application (`DyeWardrobeItem`) confirmation dialog callbacks.
* **Antispam & Item Storage:**
  * Antispam cooldown PID: `0x100030AA` (`Antispam_WardrobeRequest`).
  * Account Wardrobe items are cached on the client in `0x10002FF2` (`Wardrobe_MergedItemCache`), an array of `0x10002F15` (`Wardrobe_ItemStruct`) bags where each bag holds:
    * `0x10002F13` (`Wardrobe_ItemDID`: `u32`) — prototype DataID of the cosmetic item.
    * `0x10002F16` (`Wardrobe_ItemClothingColors`: `u32`) & `0x10006392` (`Wardrobe_ItemClothingColorsExt`: `u64` / `i64`) — bitmask of unlocked dye colors for that Wardrobe entry (`0x0A420013` tests `(colors_ext & (1ULL << color_bit)) != 0`).
* **Dyeing Items (`Inventory` vs `Wardrobe`):**
  * **Dyeing an Inventory/Cosmetic Item:** Performed via standard targeted item usage (`Usage_PendingTargetedUsageItem` `0x10001218`, where the dye bottle has `0x1000007E` `Item_Class == 41 / 0x0029` and carries `0x1000085A` `Item_ClothingColor`).
  * **Dyeing a Wardrobe Entry:** When a dye item (`Item_Class == 41`) is active in `0x10001218` (`Usage_PendingTargetedUsageItem`) and the player clicks a Wardrobe entry (`0x0A430016` at `boff=0x16B488`), the client verifies the color bit is not already unlocked (`0x0A420013`), spawns confirmation dialog `Class 2675 (0x0A73)`, and invokes `0x0A430011` → `0x8A45000D` (`DyeWardrobeItem`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x8A45000F` | C→S | `0x0A450010` (`boff=0x110CE4`) ← `0x0A430001` | `RequestOpenWardrobe` | 1. `banker_iid: u64` (`WriteI64`) | Requests opening Wardrobe via Vault-Keeper (`banker_iid`) or remote access (`0`). |
| `0x8A450001` | S→C | `0x0A450001` (`boff=0x110C1C`) → `0x0A430002` | `RecvOpenWardrobe` | 1. `banker_iid: u64` (`ReadI64`) | Opens Wardrobe UI (`0x0A460001`) and records active `banker_iid` in `0x10003AAD` (`Wardrobe_BankerIID`). |
| `0x8A450003` | C→S | `0x0A450004` (`boff=0x110D3C`) ← `0x0A430004` | `CloseWardrobe` | *(0 arguments)* | Closes the active Wardrobe session and clears `0x10003AAD`. |
| `0x8A450007` | C→S | `0x0A450008` (`boff=0x110C7C`) ← `0x0A43000D` | `RemoveWardrobeItem` | 1. `banker_iid: u64` (`WriteI64`)<br>2. `item_did: u32` (`WriteI32`) | Removes `item_did` and all unlocked colors from the Wardrobe. |
| `0x8A450009` | C→S | `0x0A45000A` (`boff=0x110BB4`) ← `0x0A43000F` | `AddInventoryItemToWardrobe` | 1. `banker_iid: u64` (`WriteI64`)<br>2. `item_iid: u64` (`WriteI64`) | Copies cosmetic appearance & color of backpack item `item_iid` into the Wardrobe. |
| `0x8A45000B` | C→S | `0x0A45000C` (`boff=0x110AC4`) ← `0x0A430010` | `AddBankItemToWardrobe` | 1. `banker_iid: u64` (`WriteI64`)<br>2. `bank_item_iid: u64` (`WriteI64`)<br>3. `bank_address: BankAddressRef` (`WriteRef`) | Copies cosmetic appearance & color of an item stored in Vault/Shared Storage (`bank_address`, class `0x0E9C`) into the Wardrobe. |
| `0x8A45000D` | C→S | `0x0A45000E` (`boff=0x110B3C`) ← `0x0A430011` | `DyeWardrobeItem` | 1. `banker_iid: u64` (`WriteI64`)<br>2. `item_did: u32` (`WriteI32`)<br>3. `dye_iid: u64` (`WriteI64`) | Consumes `dye_iid` to unlock its clothing color bit on Wardrobe entry `item_did`. |

#### 1.4 Mounted Combat Cosmetics (`RPC Class 3495 / 0x0DA7` ↔ `System Class 3470 / 0x0D8E`)

* **Mount Cosmetic Slot & Color Management:**
  * Uses `Class 3495 (0x0DA7)` for war-steed cosmetic trait/color/visibility/name changes, and `TraitConfiguration` (`Class 3605 / 0x0E15` with `config_type == 2`) for war-steed cosmetic outfit preset slots (`0x10004E8F` `Mount_Appearances_Slot_Colourization`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x8DA70007` | C→S | `0x0DA70008` (`boff=0x10EFF0`) ← `0x0D8E0027` | `ApplyMountCosmeticDye` | 1. `slot_type: u32` (`WriteI32`)<br>2. `color_id: u32` (`WriteI32`)<br>3. `trait_did: u32` (`WriteI32`)<br>4. `dye_item_iid: u64` (`WriteI64`) | Unlocks/applies war-steed dye `dye_item_iid`. |
| `0x8DA70009` | C→S | `0x0DA7000A` (`boff=0x10F184`) ← `0x0D8E0028` | `SelectMountAppearanceTrait` | 1. `slot_type: u32` (`WriteI32`)<br>2. `trait_did: u32` (`WriteI32`) | Equips war-steed cosmetic piece `trait_did` in `slot_type`. |
| `0x8DA7000B` | C→S | `0x0DA7000C` (`boff=0x10F1EC`) ← `0x0D8E0029` | `SelectMountAppearanceColor` | 1. `color_id: u32` (`WriteI32`)<br>2. `trait_did: u32` (`WriteI32`) | Selects unlocked color `color_id` for `trait_did`. |
| `0x8DA7000D` | C→S | `0x0DA7000E` (`boff=0x10F254`) ← `0x0D8E002A` | `SetMountCosmeticSlotVisibility` | 1. `b_visible: u32` (`WriteI32`)<br>2. `slot_type: u32` (`WriteI32`) | Toggles visibility of war-steed cosmetic slot (`0x10004FA8`). |
| `0x8DA7000F` | C→S | `0x0DA70010` (`boff=0x10F0CC`) ← `0x0D8E002C` | `SaveMountName` | 1. `mount_name: WStringRef` (`WriteRef`) | Renames war-steed (antispam `0x100050BD` `Antispam_MountSaveName`). |
| `0x8DA70011` | S→C | `0x0DA70011` (`boff=0x10F074`) → `0x0D8E002F` | `RecvMountCosmeticUpdate` | *(0 arguments)* | Notifies UI (`0x0DA90001`). |
| `0x8DA70013` | C→S | `0x0DA70014` (`boff=0x10F12C`) ← `0x0D8E0030` | `PurchaseMountCosmeticSlot` | *(0 arguments)* | Opens Store/purchase flow for an extra mount cosmetic slot. |

---

### 2. Domain 2: Vault, Shared Storage (`BankRepositorySystem`), Carry-All Bags (`BoS`), & Inventory Management

#### 2.1 `BankRepositorySystem` — Vault, Shared Storage, & Housing Chests (`RPC Class 2564 / 0x0A04` ↔ `System Class 2562 / 0x0A02`)

* **WLib Class Hierarchy & `BankAddress` (`Class 3740 / 0x0E9C`):**
  * `Class 2564 (0x0A04)`: Network RPC interface (`parent=2560 / 0x0A00`).
  * `Class 2562 (0x0A02)`: Client `BankRepositorySystem` (`parent=2564 / 0x0A04`).
  * `Class 3740 (0x0E9C)`: Native `BankAddress` object constructed in `0x0A00001B` (`boff=0x17E3F0`):
    * `0x0E9C0004 (bank_addr, bank_type: u32)`: Sets the 32-bit `BankType` enum (`0x10002DAB` `BankRepository_BankType`):
      * `1` = `Character` (Character Vault)
      * `2` = `Account` (Account Shared Storage)
      * `3` = `SharedHousing` (Shared Housing Storage)
      * `4` = `StandardHousing` (Standard Housing Chest)
    * `0x0E9C0002 (bank_addr, owner_iid: u64)`: Sets the 64-bit owner/container `IID` (`0x10002DAA` `BankRepository_AccountBankID` / `0x10002DDE` `BankRepository_BankerID`).
  * Antispam cooldown PID: `0x10002DF3` (`Antispam_BankRequest`), checked via `0x0A00001C`.
  * Chest organization: Every item in a bank repository has `0x10002DDD` (`BankRepository_ChestType`: `u32`, `0` = Main Vault, `1..10` = Chests 1..10). Custom chest names are stored on the avatar in `0x100039FD..0x10003A06` (`BankRepository_ChestName1..10`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x8A040029` | C→S | `0x0A04002A` (`boff=0x110588`) ← `0x0A020020` | `OpenBank` | 1. `bank_addr: BankAddressRef` (`WriteRef`) | Requests opening Character Vault (`bank_type=1`) or Shared Storage (`bank_type=2`). |
| `0x8A04002D` | C→S | `0x0A04002E` (`boff=0x110744`) ← `0x0A020025` | `CloseBank` | *(0 arguments)* | Closes active bank session. |
| `0x8A04000B` | C→S | `0x0A04000C` (`boff=0x110854`) ← `0x0A020003` | `DepositItemToBank` | 1. `quantity: u32` (`WriteI32`)<br>2. `chest_type: u32` (`WriteI32`)<br>3. `dest_merge_item_iid: u64` (`WriteI64`)<br>4. `src_item_iid: u64` (`WriteI64`)<br>5. `dest_bank_addr: BankAddressRef` (`WriteRef`) | Moves `quantity` of `src_item_iid` from Backpack/Equipment into `dest_bank_addr` at tab `chest_type` (merging onto `dest_merge_item_iid` if non-zero). |
| `0x8A040013` | C→S | `0x0A040014` (`boff=0x11079C`) ← `0x0A020006` | `WithdrawItemFromBank` | 1. `b_to_equipped: u32` (`WriteI32`)<br>2. `quantity: u32` (`WriteI32`)<br>3. `dest_slot: u32` (`WriteI32`)<br>4. `dest_merge_item_iid: u64` (`WriteI64`)<br>5. `src_item_iid: u64` (`WriteI64`)<br>6. `src_bank_addr: BankAddressRef` (`WriteRef`) | Withdraws `quantity` of `src_item_iid` from `src_bank_addr` to Backpack (`b_to_equipped=0`, `dest_slot` index) or directly to an Equipment slot (`b_to_equipped=1`, `dest_slot` bitmask). |
| `0x8A04001F` | C→S | `0x0A040020` (`boff=0x1104E0`) ← `0x0A020017` | `TransferItemBetweenBanks` | 1. `quantity: u32` (`WriteI32`)<br>2. `dest_chest_type: u32` (`WriteI32`)<br>3. `dest_merge_item_iid: u64` (`WriteI64`)<br>4. `dest_bank_addr: BankAddressRef` (`WriteRef`)<br>5. `src_item_iid: u64` (`WriteI64`)<br>6. `src_bank_addr: BankAddressRef` (`WriteRef`) | Transfers `quantity` of `src_item_iid` directly between two open banks (e.g., Character Vault ↔ Shared Storage). |
| `0x8A040021` | C→S | `0x0A040022` (`boff=0x1103F4`) ← `0x0A020019` | `MoveBankItemToChest` | 1. `dest_chest_type: u32` (`WriteI32`)<br>2. `item_iid: u64` (`WriteI64`)<br>3. `bank_addr: BankAddressRef` (`WriteRef`) | Reassigns `item_iid` inside `bank_addr` to chest tab `dest_chest_type` (`0..10`). |
| `0x8A04001B` | C→S | `0x0A04001C` (`boff=0x11046C`) ← `0x0A020015` | `PurchaseBankUpgrade` | 1. `banker_iid: u64` (`WriteI64`)<br>2. `bank_addr: BankAddressRef` (`WriteRef`) | Purchases next in-game currency capacity upgrade tier for `bank_addr` at `banker_iid`. |
| `0x8A04003B` | C→S | `0x0A04003C` (`boff=0x1108F8`) ← `0x0A020029` | `RenameBankChest` | 1. `new_name: WStringRef` (`WriteRef`)<br>2. `chest_type: u32` (`WriteI32`)<br>3. `bank_addr: BankAddressRef` (`WriteRef`) | Renames chest tab `chest_type` (`1..10`) in `bank_addr`. |
| `0x8A04003F` | C→S | `0x0A040040` (`boff=0x110970`) ← `0x0A02002D` | `DepositHousingDecorationToBank` | 1. `hook_index: u32` (`WriteI32`)<br>2. `chest_type: u32` (`WriteI32`)<br>3. `decoration_iid: u64` (`WriteI64`)<br>4. `house_iid: u64` (`WriteI64`)<br>5. `dest_bank_addr: BankAddressRef` (`WriteRef`) | Returns a slotted housing decoration directly to `dest_bank_addr`. |
| `0x8A04001D` | S→C | `0x0A04001D` (`boff=0x1105E0`) → `0x0A020016` | `RecvBankOperationResult` | 1. `result_pid: u32` (`ReadI32`)<br>2. `bank_addr: BankAddressRef` (`ReadRef`) | Reports bank operation completion or error PID (`0x0A0A0003`). |
| `0x8A040026` | S→C | `0x0A040026` (`boff=0x1106B0`) → `0x0A02001A` | `RecvBankOpened` | 1. `b_read_only: u32` (`ReadI32`)<br>2. `max_capacity: u32` (`ReadI32`)<br>3. `bank_addr: BankAddressRef` (`ReadRef`) | Marks bank `bank_addr` open with `max_capacity` (`0x10002DC5`) and notifies UI (`0x0A0A0004`). |
| `0x8A040023` | S→C | `0x0A040023` (`boff=0x110650`) → `0x0A02001B` | `RecvBankClosed` | 1. `bank_addr: BankAddressRef` (`ReadRef`) | Marks bank `bank_addr` closed and notifies UI (`0x0A0A0005`). |
| `0x8A040028` | S→C | `0x0A040028` (`boff=0x11036C`) → `0x0A02001F` | `RecvBankUpgradeAvailable` | 1. `upgrade_tier: u32` (`ReadI32`)<br>2. `banker_iid: u64` (`ReadI64`)<br>3. `bank_addr: BankAddressRef` (`ReadRef`) | Prompts client with available in-game capacity upgrade (`0x0A0A0008`). |

#### 2.2 Carry-All Bags — `BagOfSorts` (`BoS`, `RPC Class 4181 / 0x1055` ↔ `System Class 4180 / 0x1054`)

* **WLib Class Hierarchy & Properties:**
  * `Class 4181 (0x1055)`: Network RPC interface (`parent=4178 / 0x1052`).
  * `Class 4180 (0x1054)`: Client `BagOfSortsSystem` (`parent=4181 / 0x1055`).
  * `Class 4179 (0x1053)`: Carry-All item entity wrapper (`boff=0x0BDDFC`).
  * Carry-All contents are stored in `0x10006782` (`BoS_ItemArray`), an array of `0x1000677B` (`BoS_ItemStruct`) `PropertyCollection` bags containing:
    * `0x10006780` (`BoS_ItemDID`: `u32`) — prototype DataID of the stacked resource/item.
    * `0x10006789` (`BoS_Quantity`: `u32`) — total stacked quantity.
    * `0x10006775` (`BoS_Bound_To_Account_Amount`: `u32`) — portion bound to account.
    * `0x10006788` (`BoS_Bound_To_Character_Amount`: `u32`) — portion bound to character.
    * `0x10006786` (`BoS_ItemName`: `WString`) — cached localized item name.
  * Bag-level configuration properties: `0x1000677F` (`BoS_MaxSlots`), `0x1000677D` (`BoS_MaxStackSize`), `0x1000677E` (`BoS_AutoGather`), `0x1000677A` (`BoS_MatchingCategories`), `0x100071B0` (`BoS_Pseudonym`).
  * Antispam cooldown PID: `0x100068CA` (`Antispam_BoSRequest`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x90550001` | C→S | `0x10550008` (`boff=0x0F9EA8`) ← `0x1054001D` | `DepositItemToCarryAll` | 1. `item_iid: u64` (`WriteI64`)<br>2. `carry_all_iid: u64` (`WriteI64`) | Deposits inventory stack `item_iid` into Carry-All bag `carry_all_iid`. |
| `0x90550002` | C→S | `0x10550006` (`boff=0x0FA2CC`) ← `0x1054001A` | `WithdrawItemFromCarryAll` | 1. `bound_filter_type: u32` (`WriteI32`)<br>2. `quantity: u32` (`WriteI32`)<br>3. `item_did: u32` (`WriteI32`)<br>4. `carry_all_iid: u64` (`WriteI64`) | Withdraws `quantity` of `item_did` matching `bound_filter_type` from `carry_all_iid` into the player's Backpack. |
| `0x90550003` | C→S | `0x10550007` (`boff=0x0FA194`) ← `0x10540026` | `DeleteCarryAllItem` | 1. `item_did: u32` (`WriteI32`)<br>2. `carry_all_iid: u64` (`WriteI64`) | Permanently destroys stack `item_did` inside `carry_all_iid`. |
| `0x90550004` | C→S | `0x1055000A` (`boff=0x0F9F10`) ← `0x10540020` | `GatherItemsIntoCarryAll` | 1. `carry_all_iid: u64` (`WriteI64`) | Sweeps all matching items from Backpack into `carry_all_iid`. |
| `0x90550005` | C→S | `0x10550009` (`boff=0x0FA054`) ← `0x1054001B` | `SetCarryAllOption` | 1. `b_enabled: u32` (`WriteI32`)<br>2. `option_type: u32` (`WriteI32`)<br>3. `carry_all_iid: u64` (`WriteI64`) | Toggles `BoS_AutoGather` (`0x1000677E`) or related option on `carry_all_iid`. |
| `0x90550014` | C→S | `0x10550016` (`boff=0x0FA274`) ← `0x1054003A` | `RequestCarryAllContents` | 1. `carry_all_iid: u64` (`WriteI64`) | Requests full item array sync when opening `carry_all_iid` for the first time in a session. |
| `0x9055001A` | C→S | `0x1055001F` (`boff=0x0FA12C`) ← `0x1054004F` | `SetCarryAllPseudonym` | 1. `pseudonym: WStringRef` (`WriteRef`)<br>2. `carry_all_iid: u64` (`WriteI64`) | Sets user-defined custom label (`0x100071B0` `BoS_Pseudonym`) on `carry_all_iid`. |
| `0x9055000D` | S→C | `0x1055000D` (`boff=0x0F9FD8`) → `0x10540034` | `RecvCarryAllItemUpdated` | 1. `bos_item_struct: PropertyBagRef` (`ReadRef`)<br>2. `carry_all_iid: u64` (`ReadI64`) | Updates or inserts a `BoS_ItemStruct` (`0x1000677B`) in `carry_all_iid` (`0x10660003`). |
| `0x90550010` | S→C | `0x10550010` (`boff=0x0FA0BC`) → `0x10540035` | `RecvCarryAllItemRemoved` | 1. `item_did: u32` (`ReadI32`)<br>2. `carry_all_iid: u64` (`ReadI64`) | Removes `item_did` entry from `carry_all_iid` (`0x10660008`). |
| `0x90550015` | S→C | `0x10550015` (`boff=0x0FA1FC`) → `0x10540039` | `RecvCarryAllItemArrayChunk` | 1. `bos_item_array_bag: PropertyBagRef` (`ReadRef`)<br>2. `carry_all_iid: u64` (`ReadI64`) | Streams a chunk of `BoS_ItemArray` (`0x10006782`) for `carry_all_iid` (`0x1066000F`). |
| `0x90550020` | S→C | `0x10550020` (`boff=0x0F9F68`) → `0x1054004D` | `RecvCarryAllSyncComplete` | 1. `carry_all_iid: u64` (`ReadI64`) | Marks `carry_all_iid` contents synchronized and notifies UI (`0x10660010`). |

#### 2.3 Backpack Inventory & Container Viewing (`RPC Class 1290 / 0x050A` & `RPC Class 1299 / 0x0513`)

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x850A0001` | C→S | `0x050A0010` (`boff=0x103DA8`) | `DestroyItem` | 1. `quantity: u32` (`WriteI32`)<br>2. `item_iid: u64` (`WriteI64`) | Destroys `quantity` of backpack item `item_iid`. |
| `0x850A000B` | C→S | `0x050A0011` (`boff=0x103D00`) | `MoveOrStackItem` | 1. `quantity: u32` (`WriteI32`)<br>2. `dest_slot: u32` (`WriteI32`)<br>3. `dest_iid: u64` (`WriteI64`)<br>4. `src_iid: u64` (`WriteI64`) | Moves or merges `src_iid` onto `dest_iid` / `dest_slot`. |
| `0x850A000C` | C→S | `0x050A0012` (`boff=0x103F98`) | `SetItemLocked` | 1. `b_locked: u32` (`WriteI32`)<br>2. `item_iid: u64` (`WriteI64`) | Toggles `Inventory_ItemLocked` on `item_iid`. |
| `0x850A0016` | C→S | `0x050A0028` (`boff=0x103C88`) | `ClaimPendingLootItem` | 1. `dest_slot: u32` (`WriteI32`)<br>2. `item_iid: u64` (`WriteI64`) | Moves an overflow/pending loot item into backpack `dest_slot`. |
| `0x850A0019` | C→S | `0x050A0026` (`boff=0x1040D8`) | `SplitItemStack` | 1. `quantity: u32` (`WriteI32`)<br>2. `item_iid: u64` (`WriteI64`) | Splits `quantity` from stack `item_iid` into a free backpack slot. |
| `0x850A001D` | C→S | `0x050A0027` (`boff=0x104050`) | `MoveItemToSlot` | 1. `quantity: u32` (`WriteI32`)<br>2. `dest_slot: u32` (`WriteI32`)<br>3. `item_iid: u64` (`WriteI64`) | Moves `item_iid` to backpack/equipment `dest_slot`. |
| `0x850A0034` | C→S | `0x050A0035` (`boff=0x104140`) | `ClearNewItemFlag` | 1. `item_iid: u64` (`WriteI64`) | Clears the UI "New Item" highlight on `item_iid`. |
| `0x850A0038` | C→S | `0x050A003B` (`boff=0x103EB8`) | `SaveBagSlotLayout` | 1. `slot_array: ArrayRef` (`WriteRef`) | Saves custom inventory bag-to-slot mapping (`0x10004FA9`). |
| `0x850A0015` | S→C | `0x050A0015` (`boff=0x103E20`) | `RecvInventoryMoveError` | 1. `error_pid: u32` (`ReadI32`)<br>2. `item_iid: u64` (`ReadI64`) | Reports inventory move failure. |
| `0x850A0037` | S→C | `0x050A0037` (`boff=0x103F20`) | `RecvBagSlotLayout` | 1. `slot_array: ArrayRef` (`ReadRef`) | Synchronizes custom inventory bag-to-slot mapping. |
| `0x8513002D` | C→S | `0x05130037` (`boff=0x106BC8`) | `StartViewingContainer` | 1. `container_iid: u64` (`WriteI64`) | Opens an external world container (`CMEOInventorySystem`). |
| `0x85130016` | C→S | `0x05130023` (`boff=0x106DD8`) | `StopViewingContainer` | 1. `container_iid: u64` (`WriteI64`) | Closes an external world container. |
| `0x85130004` | S→C | `0x05130004` (`boff=0x106C30`) | `RecvStartViewingResult` | 1. `result_pid: u32` (`ReadI32`)<br>2. `flags: u32` (`ReadI32`)<br>3. `container_iid: u64` (`ReadI64`) | Confirms or rejects opening `container_iid`. |
| `0x85130014` | S→C | `0x05130014` (`boff=0x106CE0`) | `RecvStopViewingResult` | 1. `result_pid: u32` (`ReadI32`)<br>2. `flags: u32` (`ReadI32`)<br>3. `container_iid: u64` (`ReadI64`) | Confirms closing `container_iid`. |

---

### 3. Domain 3: Player Inspection (`Inspect`, `CExaminationSystem`, `Bio`), Skirmish Traits/Soldiers, & Hobbies (Fishing/Birding)

#### 3.1 Player Inspection, Examination, & Biography (`RPC Class 1277 / 0x04FD`, `RPC Class 1335 / 0x0537`, `RPC Class 1287 / 0x0507`)

* **Two Complementary Inspection Channels:**
  1. **`CExaminationSystem` (`RPC Class 1277 / 0x04FD` ↔ `System Class 269 / 0x010D`):**
     * Native C++ UI glue at `0x140b63d2d` (in `"SocialPanelPageElements"` / `"Inspect"`) calls `WLib::CallStatic(0x010D0002, target_iid)` (`RequestInspectTarget`).
     * `0x010D0002` (`boff=0x08F610`) sends `0x84FD000A` (`RequestInspectTarget`) and `0x010D0020` (`boff=0x08F68C`) sends `0x84FD0010` (`RequestExamineBio`) and `0x84FD0025` (`RequestExamineTraitsAndEffects`).
     * Server responds with `0x84FD0011` (`RecvInspectEquipmentAndCosmetics`), `0x84FD0017` (`RecvPlayerBio`), and `0x84FD0028` (`RecvInspectTraitsAndEffects`), or `0x84FD0016` (`RecvInspectError`, e.g., when target has `Anonymous` privacy enabled).
  2. **`Inspect` / `VitalDisplayOverride` (`RPC Class 1335 / 0x0537` ↔ `System Class 387 / 0x0183` & `Class 1540 / 0x0604`):**
     * `0x01830051` (`boff=0x1BABAC`) sends `0x85370017` (`RequestInspectPropertyBag`: `[target_iid: u64]`), and server responds with `0x85370003` (`RecvInspectPropertyBag`: `[inspect_props: PropertyBagRef]`), which populates `0x10002B8B` (`Inspect_Data`).
     * `0x06040056` (`boff=0x195128`) sends `0x8537001E` (`SetVitalDisplayOverride`: `[override_id: u32]`, antispam `0x10004A0D`) to select the player's portrait frame (`0x10004A0A` `UI_VitalDisplayOverride`).
  3. **`Bio` / Item Inscription (`RPC Class 1287 / 0x0507` ↔ `System Class 367 / 0x016F`):**
     * Native C++ UI glue at `0x140b99043` calls `WLib::CallStatic(0x016F0001)` → `0x8507000F` (`SetPlayerBioOrItemInscription`).
     * Antispam cooldown PID: `0x1000143A` (`Antispam_BioRequest`); maximum length governed by `0x10000A76` (`Bio_MaxStringLength`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x84FD000A` | C→S | `0x04FD001F` (`boff=0x0FDBF4`) ← `0x010D0002` | `RequestInspectTarget` | 1. `target_iid: u64` (`WriteI64`) | Requests equipment and active cosmetic outfit inspection of `target_iid`. |
| `0x84FD0010` | C→S | `0x04FD0020` (`boff=0x0FDC4C`) ← `0x010D0020` | `RequestExamineBio` | 1. `target_iid: u64` (`WriteI64`) | Requests player biography (`0x10000A75` `Bio_PlayerBiography`) of `target_iid`. |
| `0x84FD0025` | C→S | `0x04FD0029` (`boff=0x0FDCA4`) ← `0x010D001F` | `RequestExamineTraitsAndEffects` | 1. `target_iid: u64` (`WriteI64`) | Requests slotted traits and active effects of `target_iid`. |
| `0x84FD001B` | C→S | `0x04FD0021` (`boff=0x0FDB6C`) ← `0x010D0001` | `RequestItemExamination` | 1. `context_id: u32` (`WriteI32`)<br>2. `exam_key: Ref` (`WriteRef`)<br>3. `flags: u32` (`WriteI32`)<br>4. `exam_req: Ref` (`WriteRef`) | Requests tooltip examination data for an item link or non-resident item. |
| `0x84FD0006` | S→C | `0x04FD0006` (`boff=0x0FDF4C`) → `0x010D000B` | `RecvItemExaminationData` | 1. `context_id: u32` (`ReadI32`)<br>2. `item_props: PropertyBagRef` (`ReadRef`)<br>3. `exam_key: Ref` (`ReadRef`) | Delivers item tooltip property bag (`0x013E0003`). |
| `0x84FD0011` | S→C | `0x04FD0011` (`boff=0x0FDE18`) → `0x010D001B` | `RecvInspectEquipmentAndCosmetics` | 1. `sub_element_props_map: MapRef<u32, PropertyBagRef>` (`ReadRef`, `0x140B29040`)<br>2. `secondary_player_info_props: PropertyBagRef` (`ReadRef`)<br>3. `primary_player_info_props: PropertyBagRef` (`ReadRef`)<br>4. `quest_category_did: u32` (`ReadI32`, `0x1000006D`)<br>5. `raid_image_did: u32` (`ReadI32`, `0x1000006C`)<br>6. `vendor_filter_val: u32` (`ReadI32`, `0x10000065` / `0x0050AA9C`)<br>7. `extra_player_info_props: PropertyBagRef` (`ReadRef`)<br>8. `phys_obj_info_props: PropertyBagRef` (`ReadRef`)<br>9. `item_exam_array: ArrayRef` (`ReadRef`, `0x140BB01D0`)<br>10. `outfit_tooltip_array: ArrayRef<PropertyBagRef>` (`ReadRef`, `0x10001C67` + `0x10001C49`)<br>11. `avatar_visual_props: PropertyBagRef` (`ReadRef`)<br>12. `header_player_info_props: PropertyBagRef` (`ReadRef`)<br>13. `active_outfit_type: u32` (`ReadI32`)<br>14. `target_iid: u64` (`ReadI64`) | Dispatched via `0x013E0007` (`0x140B401C0` → `0x140B0FFA0` → `0x140C95BE0` → `0x140CA6360` & `0x140CAC220`) to populate `UI_InspectDisplay_AvatarInfoElement` (`0x100007C8`) and `UI_InspectDisplay_PlayerInfoElement` (`0x100007C7`). |
| `0x84FD0016` | S→C | `0x04FD0016` (`boff=0x0FDD80`) → `0x010D000A` | `RecvInspectError` | 1. `error_pid: u32` (`ReadI32`)<br>2. `target_iid: u64` (`ReadI64`) | Reports inspection failure (e.g., target anonymous or out of range) to `0x013E0006`. |
| `0x84FD0017` | S→C | `0x04FD0017` (`boff=0x0FDDF0`) → `0x010D0017` | `RecvPlayerBio` | 1. `error_pid: u32` (`ReadI32`)<br>2. `bio_text: WStringRef` (`ReadRef`)<br>3. `target_iid: u64` (`ReadI64`) | Delivers inspected player's biography text (`0x01670014`). |
| `0x84FD0028` | S→C | `0x04FD0028` (`boff=0x0FDCFC`) → `0x010D001D` | `RecvInspectTraitsAndEffects` | 1. `traits_props: PropertyBagRef` (`ReadRef`)<br>2. `target_iid: u64` (`ReadI64`) | Delivers inspected player's slotted traits (`0x013E0009`). |
| `0x85370017` | C→S | `0x05370018` (`boff=0x110A08`) ← `0x01830051` | `RequestInspectPropertyBag` | 1. `target_iid: u64` (`WriteI64`) | Requests `Inspect_Data` (`0x10002B8B`) property bag for `target_iid`. |
| `0x85370003` | S→C | `0x05370003` (`boff=0x110A60`) → `0x0183004B` | `RecvInspectPropertyBag` | 1. `inspect_props: PropertyBagRef` (`ReadRef`) | Delivers `Inspect_Data` (`0x10002B8B`) property bag. |
| `0x8537001E` | C→S | `0x0537001F` (`boff=0x1109B0`) ← `0x06040056` | `SetVitalDisplayOverride` | 1. `override_id: u32` (`WriteI32`) | Sets avatar portrait frame (`0x10004A0A` `UI_VitalDisplayOverride`). |
| `0x8507000F` | C→S | `0x05070012` (`boff=0x10291C`) ← `0x016F0001` | `SetPlayerBioOrItemInscription` | 1. `text: WStringRef` (`WriteRef`)<br>2. `target_entity_iid: u64` (`WriteI64`) | Sets `Bio_PlayerBiography` (`0x10000A75`) when `target_entity_iid` is the avatar IID, or `Bio_ItemInscription` (`0x10000A74`) when `target_entity_iid` is a crafted item IID. |

#### 3.2 Trait Slotting, Skirmish Soldier Traits, & Specializations (`CTraitSystem` `0x0532`, `TraitConfiguration` `0x0E15`, `CSkirmishSystem` `0x09EC`)

* **How Skirmish Soldier Traits & Cosmetics Work:**
  * Skirmish Soldiers do **not** use a separate soldier-specific slotting RPC class. Instead, Skirmish Soldier Roles, Attributes, Skills, Training, and Personal/Cosmetic Appearance Traits are standard Traits with `SkirmishTraitDescriptor` (`Class 2525 / 0x09DD`, `parent=308 / 0x0134` `TraitDescriptor`) and Skirmish `Trait_Nature` (`0x100007C1`) categories!
  * Upgrading a Skirmish Soldier trait rank at a Skirmish Bard/Trainer uses `CTraitSystem::AcquireTraitRank` (`0x8532003D`) or `BarterForTrait` (`0x85320057`), deducting Skirmish Marks (`0x10002EB8` `Skirmish_TraitCostProgression` / `0x10002EBA` `Trait_Custom_Skirmish_TraitRankCap`).
  * Slotting/unslotting Skirmish Soldier traits (including Soldier cosmetic appearance traits) at a Skirmish Bard uses `CTraitSystem::SlotTrait` (`0x85320016`) / `SlotTraitAtPosition` (`0x8532004B`), `UnslotTraitByDID` (`0x8532000E`), and `CommitPendingTraits` (`0x8532001F`).
* **`CTraitSystem` (`RPC Class 1330 / 0x0532` ↔ `System Class 293 / 0x0125`):**

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x85320008` | C→S | `0x0532000D` (`boff=0x107BF4`) | `UnslotAllTraits` | *(0 arguments)* | Clears all pending slotted traits. |
| `0x8532000E` | C→S | `0x0532001B` (`boff=0x107B9C`) | `UnslotTraitByDID` | 1. `trait_did: u32` (`WriteI32`) | Unslots `trait_did` (Virtue, Racial, Class, or Skirmish Soldier trait). |
| `0x85320016` | C→S | `0x0532001C` (`boff=0x1075F4`) | `SlotTrait` | 1. `slot_descriptor: Ref` (`WriteRef`)<br>2. `trait_did: u32` (`WriteI32`) | Places `trait_did` into `slot_descriptor` (`Class 309 / 0x0135`). |
| `0x8532001F` | C→S | `0x05320031` (`boff=0x107AD8`) | `CommitPendingTraits` | *(0 arguments)* | Commits pending trait changes (retraiting / bard / soldier configuration). |
| `0x85320021` | C→S | `0x05320032` (`boff=0x107B20`) | `CancelPendingTraits` | *(0 arguments)* | Discards uncommitted pending trait changes. |
| `0x8532003D` | C→S | `0x05320041` (`boff=0x107C3C`) | `AcquireTraitRank` | 1. `slot_descriptor: Ref` (`WriteRef`)<br>2. `trait_did: u32` (`WriteI32`) | Purchases/trains next rank of `trait_did` (including Skirmish Soldier trait ranks). |
| `0x85320045` | C→S | `0x05320046` (`boff=0x107FC4`) | `RequestTraitRespec` | *(0 arguments)* | Requests paid trait tree respec (`0x10004D48` `Trait_PointBasedTraitResetCost`). |
| `0x85320049` | C→S | `0x0532004A` (`boff=0x107F6C`) | `SetActiveTraitNature` | 1. `trait_nature: u32` (`WriteI32`) | Sets active trait category/set (`0x100007C1` `Trait_Nature`). |
| `0x8532004B` | C→S | `0x0532004C` (`boff=0x10800C`) | `SlotTraitWithFlags` | 1. `flags: u32` (`WriteI32`)<br>2. `slot_descriptor: Ref` (`WriteRef`)<br>3. `trait_did: u32` (`WriteI32`) | Slots `trait_did` into `slot_descriptor` with explicit flags. |
| `0x8532004F` | C→S | `0x05320053` (`boff=0x107CA4`) | `SetTraitSuppressionToggle` | 1. `b_suppressed: u32` (`WriteI32`)<br>2. `trait_did: u32` (`WriteI32`) | Toggles suppression of a passive/racial trait (`0x100051F3`). |
| `0x85320057` | C→S | `0x0532005C` (`boff=0x107F14`) | `BarterForTrait` | 1. `trait_did: u32` (`WriteI32`) | Barters currency/marks for `trait_did`. |

* **`TraitConfiguration` (`RPC Class 3605 / 0x0E15` ↔ `System Class 3606 / 0x0E16`):**
  * Manages saved multi-slot configurations (`config_type = 1` for Class/Mounted Specialization Trees, `config_type = 2` for Mounted Combat Cosmetic Outfit Presets). Antispam PID: `0x10004F9E` (`Antispam_TraitConfigurationRequest`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x8E150001` | C→S | `0x0E150002` (`boff=0x10FAEC`) ← `0x0E160001` | `SelectTraitConfiguration` | 1. `slot_idx: u32` (`WriteI32`)<br>2. `config_type: u32` (`WriteI32`) | Activates saved specialization/cosmetic preset `slot_idx` for `config_type`. |
| `0x8E150005` | C→S | `0x0E150006` (`boff=0x10FCBC`) ← `0x0E160006` | `SaveTraitConfiguration` | 1. `config_props: PropertyBagRef` (`WriteRef`)<br>2. `param3: u32` (`WriteI32`)<br>3. `slot_idx: u32` (`WriteI32`)<br>4. `config_type: u32` (`WriteI32`) | Saves configuration bag `config_props` into `slot_idx`. |
| `0x8E150009` | C→S | `0x0E15000A` (`boff=0x10FBCC`) ← `0x0E160007` | `PurchaseTraitConfigurationSlot` | 1. `config_type: u32` (`WriteI32`) | Purchases an additional preset slot for `config_type`. |
| `0x8E15000B` | C→S | `0x0E15000C` (`boff=0x10FB54`) ← `0x0E160008` | `RenameTraitConfigurationSlot` | 1. `name: WStringRef` (`WriteRef`)<br>2. `slot_idx: u32` (`WriteI32`)<br>3. `config_type: u32` (`WriteI32`) | Renames preset `slot_idx` (`0x10004E91` `Trait_Config_Specialization_UserSetName`). |
| `0x8E15000F` | C→S | `0x0E150010` (`boff=0x10FC24`) ← `0x0E16000A` | `SaveFullTraitConfiguration` | 1. `ref_arg1: Ref` (`WriteRef`)<br>2. `ref_arg2: Ref` (`WriteRef`)<br>3. `param3: u32` (`WriteI32`)<br>4. `slot_idx: u32` (`WriteI32`)<br>5. `config_type: u32` (`WriteI32`) | Saves full tree + bar configuration into `slot_idx`. |
| `0x8E15000E` | S→C | `0x0E15000E` (`boff=0x10FD38`) → `0x0E160009` | `RecvTraitConfigurationUpdated` | 1. `config_props: PropertyBagRef` (`ReadRef`)<br>2. `config_type: u32` (`ReadI32`) | Delivers updated configuration preset (`0x0E220001`). |
| `0x8E150011` | S→C | `0x0E150011` (`boff=0x10FD44`) → `0x0E16000C` | `RecvTraitConfigSlotPurchased` | 1. `config_type: u32` (`ReadI32`) | Notifies UI of slot purchase result (`0x0E220002`). |
| `0x8E150013` | S→C | `0x0E150013` (`boff=0x10FDA4`) → `0x0E16000B` | `RecvTraitConfigurationApplied` | 1. `config_type: u32` (`ReadI32`) | Applies active configuration (updating `0x10004E8F` on mount when `config_type == 2`). |

* **`CSkirmishSystem` (`RPC Class 2540 / 0x09EC`) & `WorldJoin` (`RPC Class 2604 / 0x0A2C`):**
  * `CSkirmishSystem` (`0x09EC`) is S→C only: `0x89EC0007` (`RecvUpdateSkirmishStat`: `[stat_prop: Property, encounter_did: u32]`), `0x89EC000A` (`RecvFullSkirmishStats`: `[stats_map: MapRef]`), `0x89EC000E` (`RecvIncrementSkirmishStat`: `[stat_prop: Property, encounter_did: u32]`), and `0x89EC0021` (`RecvSkirmishPointsUpdated`: `[points: u32]`).
  * `WorldJoin` (`0x0A2C`, Instance Finder / Skirmish Join): C→S `0x8A2C0001` (0 args), `0x8A2C0005` (`CreateEncounter`: `[wj_request: Ref]`), `0x8A2C0007` (`LeaveQueue`: 0 args), `0x8A2C0009` (`JoinEncounter`: `[wj_request: Ref]`), `0x8A2C002F` (`TravelToEncounter`: `[wj_request: Ref]`), `0x8A2C0045` (`VoteReady`: `[wj_response: Ref]`).

#### 3.3 Hobbies — Fishing & Birding (`RPC Class 2137 / 0x0859` ↔ `System Class 2126 / 0x084E`)

* **WLib Class Hierarchy:**
  * `Class 2137 (0x0859)`: Network RPC interface (`parent=2124 / 0x084C`).
  * `Class 2126 (0x084E)`: Client `HobbySystem` (`parent=2137 / 0x0859`).
  * `Class 2131 (0x0853)`: `HobbyDescriptor` (`boff=0x16B73C`), exposing metadata from `0x34000000`:
    * `0x10001D84` (`Hobby_Type`: `u32` enum — `1` = Fishing, `2` = Birding).
    * `0x10001DC1` (`Hobby_Proficiency_PropertyName`: `u32` PID — e.g. `0x1000201E` `Hobby_Fishing_Proficiency`).
    * `0x10001DC4` (`Hobby_Proficiency_Modifier_PropertyName`: `u32` PID — e.g. `0x1000201F` `Hobby_Fishing_Proficiency_Modifier`, granted by fishing poles/birding kit items).
    * `0x10001DD5` (`Hobby_ToolCategory`: `u32` — required implement category equipped in the Ranged/Tool slot `0x20000000`, checked by `0x084C0009` via `0x017000B9`).
    * `0x10001EF5` (`Hobby_Skill`: `u32` Skill DID — the toggle skill used to perform the hobby, e.g. Fishing skill or Birding skill).
    * `0x10001EF6` (`Hobby_Status`: `u32` — active hobby state on the avatar).
    * `0x10001DDF` (`Hobby_DailyProficiencyLimit`: `u32`) & `0x10001DC2` (`Hobby_MaxProficiency`: `u32`).
* **Native C++ UI Glue (`lotroclient64.exe`):**
  * `0x140b39eb5`: Calls `WLib::CallStatic(0x084E000D, hobby_did)` (`TrainHobby` confirmation dialog `Class 2088 / 0x0828` → `0x084E0004` → `0x88590003`).
* **How Fishing & Birding Actions Work on the Wire:**
  * `HobbySystem` (`Class 2137 / 0x0859`) handles **training, untraining, daily proficiency reset, and territory proficiency tracking**.
  * **Casting & Reeling In (Fishing) / Spotting (Birding):** Executing a hobby in the world does **not** use a `0x0859` RPC. Instead, the player activates the hobby's `Hobby_Skill` (`0x10001EF5`, a toggle skill with `0x10001F0F` `Skill_Toggle_Fishing_SubType`) via the standard Skill execution pipeline (`CSkillControlSystem`).
  * `0x084C0008` (`CanUseHobbySkill`, `boff=0x16B924`) verifies:
    1. The player has trained a hobby (`0x08580005`).
    2. `0x10001EF6` (`Hobby_Status`) is `0` (idle).
    3. The item equipped in slot `0x20000000` (implement/tool slot) matches `0x10001DD5` (`Hobby_ToolCategory`, e.g. Fishing Pole or Birding Spyglass).
  * During Fishing:
    * Toggling `Hobby_Skill` ON casts the line and sets `0x10001EF6` (`Hobby_Status`) to active.
    * When a fish bites, the server applies the hooked effect (`0x10001DD7` `Hobby_AvatarHookedEffect`, `0x10001DD3` `Hobby_FloatHookedEffect`, and `0x10001DC3` `Effect_HookedFish_Quality`) to the avatar/bobber.
    * Toggling `Hobby_Skill` OFF while hooked reels in the catch; the server resolves the loot table (`0x10001E56` `Hobby_TreasureControl` / `0x1000729B` `Hobby_TerritoryTreasureDirectory`) and increments the avatar's `Hobby_Proficiency_PropertyName` (`0x1000201E` `Hobby_Fishing_Proficiency`), which triggers the client property listener `0x884E0015` (`boff=0x0BEE3C`) to increment the daily gain counter in `HobbyRegistry` (`0x0858`).

| RPC ID | Dir | WLib Stub / Handler | Name | `WLibArgFrame` Serialization Order | Notes |
|---|---|---|---|---|---|
| `0x88590001` | C→S | `0x08590002` (`boff=0x101490`) ← `0x084E0003` | `UntrainHobby` | 1. `hobby_did: u32` (`WriteI32`) | Abandons/untrains `hobby_did` (after confirmation dialog `Class 2167 / 0x0877`). |
| `0x88590003` | C→S | `0x08590004` (`boff=0x1015BC`) ← `0x084E0004` | `TrainHobby` | 1. `hobby_did: u32` (`WriteI32`) | Trains `hobby_did` at a Hobby Master NPC. |
| `0x8859000B` | S→C | `0x0859000B` (`boff=0x101548`) → `0x084E0007` | `RecvOfferHobbyTraining` | 1. `hobby_did: u32` (`ReadI32`)<br>2. `trainer_npc_iid: u64` (`ReadI64`) | Prompts the player to train `hobby_did` at Hobby Master `trainer_npc_iid` (`0x08720004`). |
| `0x88590011` | S→C | `0x08590011` (`boff=0x1014E8`) → `0x084E0008` | `RecvHobbyTrained` | 1. `hobby_did: u32` (`ReadI32`) | Registers `hobby_did` in `HobbyRegistry` (`0x0858`), plays sound `99`, and attaches listeners to `0x10001DC1` (`Hobby_Proficiency_PropertyName`). |
| `0x88590026` | S→C | `0x08590026` (`boff=0x1013D0`) → `0x084E001C` | `RecvResetDailyHobbyProficiency` | 1. `hobby_did: u32` (`ReadI32`) | Resets daily proficiency gain counter for `hobby_did` to `0`. |
| `0x88590028` | S→C | `0x08590028` (`boff=0x101430`) → `0x084E001D` | `RecvHobbyTerritoryProficiencyGain` | 1. `territory_did: u32` (`ReadI32`) | Records territory proficiency gain (`0x1000729E` `Hobby_TerritoryProficiencyGainList`, used by Birding/Fishing regional caps). |

---

### 4. Key Property IDs (`0x34000000` Master Property Definition)

| Property ID | Hex | Name | Usage |
|---|---|---|---|
| `268442698` | `0x10001C4A` | `Outfit_Outfit2SlotVisibility` | Bitmask of visible slots on Cosmetic Outfit 2 |
| `268442700` | `0x10001C4C` | `Antispam_OutfitRequest` | Cooldown timestamp for `OutfitSystem` RPCs |
| `268442706` | `0x10001C52` | `Outfit_MinLevel` | Minimum avatar level required to use Cosmetic Outfits |
| `268442709` | `0x10001C55` | `Outfit_Outfit1SlotVisibility` | Bitmask of visible slots on Cosmetic Outfit 1 |
| `268442716` | `0x10001C5C` | `Outfit_Outfit1` | Container/link for Cosmetic Outfit 1 |
| `268442717` | `0x10001C5D` | `Outfit_AllSlotVisibilityFlags` | Default all-visible slot mask (`0x01FFFFFE`) |
| `268442718` | `0x10001C5E` | `Outfit_Outfit2` | Container/link for Cosmetic Outfit 2 |
| `268442721` | `0x10001C61` | `Outfit_EquipOutfit` | Container/link for Equipment Outfit (`0`) |
| `268442724` | `0x10001C64` | `Outfit_ActiveOutfitType` | Active outfit index (`0` = Equipment, `1..N` = Cosmetic `1..N`) |
| `268442725` | `0x10001C65` | `Outfit_AreaEnabled` | Whether cosmetic outfits are allowed in the current region |
| `268442729` | `0x10001C69` | `Outfit_EquipOutfitSlotVisibility` | Bitmask of visible slots on Equipment Outfit (`0`) |
| `268447474` | `0x10002EF2` | `Wardrobe_MaxCapacity` | Maximum account Wardrobe capacity |
| `268447506` | `0x10002F12` | `Wardrobe_ItemClothingColor` | Selected dye color (`f32`) when equipping from Wardrobe |
| `268447507` | `0x10002F13` | `Wardrobe_ItemDID` | Prototype DataID (`u32`) of a Wardrobe item |
| `268447509` | `0x10002F15` | `Wardrobe_ItemStruct` | `PropertyCollection` representing one Wardrobe entry |
| `268447510` | `0x10002F16` | `Wardrobe_ItemClothingColors` | 32-bit bitmask of unlocked dyes for a Wardrobe item |
| `268447730` | `0x10002FF2` | `Wardrobe_MergedItemCache` | Client array of `Wardrobe_ItemStruct` entries |
| `268447914` | `0x100030AA` | `Antispam_WardrobeRequest` | Cooldown timestamp for `WardrobeSystem` RPCs |
| `268450477` | `0x10003AAD` | `Wardrobe_BankerIID` | Active Vault-Keeper `IID` for Wardrobe operations |
| `268460946` | `0x10006392` | `Wardrobe_ItemClothingColorsExt` | 64-bit bitmask of unlocked dyes for a Wardrobe item |
| `268462860` | `0x10006B0C` | `OutfitControl_OutfitTypeList` | Array mapping `outfit_type` to outfit container PIDs |
| `268462862` | `0x10006B0E` | `OutfitControl_OutfitTypeVisibilityMap` | Map of `outfit_type` → slot visibility bitmask PID |
| `268447146` | `0x10002DAA` | `BankRepository_AccountBankID` | Account Shared Storage `IID` |
| `268447147` | `0x10002DAB` | `BankRepository_BankType` | `BankType` enum (`1`=Character, `2`=Account, `3`=SharedHousing, `4`=StandardHousing) |
| `268447173` | `0x10002DC5` | `BankRepository_MaxCapacity` | Maximum slot capacity of a bank repository |
| `268447197` | `0x10002DDD` | `BankRepository_ChestType` | Chest tab index (`0` = Main, `1..10` = Chests 1..10) |
| `268447198` | `0x10002DDE` | `BankRepository_BankerID` | Active Vault-Keeper `IID` for bank operations |
| `268447219` | `0x10002DF3` | `Antispam_BankRequest` | Cooldown timestamp for `BankRepositorySystem` RPCs |
| `268450301` | `0x100039FD` | `BankRepository_ChestName1` | Custom name for Vault Chest 1 (`..0x10003A06` for Chest 10) |
| `268461941` | `0x10006775` | `BoS_Bound_To_Account_Amount` | Account-bound quantity within a Carry-All stack |
| `268461947` | `0x1000677B` | `BoS_ItemStruct` | `PropertyCollection` for one item stack inside a Carry-All |
| `268461949` | `0x1000677D` | `BoS_MaxStackSize` | Maximum stack size per slot in a Carry-All |
| `268461950` | `0x1000677E` | `BoS_AutoGather` | Auto-gather boolean toggle on a Carry-All |
| `268461951` | `0x1000677F` | `BoS_MaxSlots` | Maximum distinct item DIDs a Carry-All can hold |
| `268461952` | `0x10006780` | `BoS_ItemDID` | Prototype DataID (`u32`) of an item stack in a Carry-All |
| `268461954` | `0x10006782` | `BoS_ItemArray` | Array of `BoS_ItemStruct` inside a Carry-All |
| `268461960` | `0x10006788` | `BoS_Bound_To_Character_Amount` | Character-bound quantity within a Carry-All stack |
| `268461961` | `0x10006789` | `BoS_Quantity` | Total quantity of an item stack in a Carry-All |
| `268462282` | `0x100068CA` | `Antispam_BoSRequest` | Cooldown timestamp for Carry-All RPCs |
| `268464560` | `0x100071B0` | `BoS_Pseudonym` | Custom user label (`WString`) on a Carry-All |
| `268438132` | `0x10000A74` | `Bio_ItemInscription` | Crafted item inscription string |
| `268438133` | `0x10000A75` | `Bio_PlayerBiography` | Player biography string |
| `268438134` | `0x10000A76` | `Bio_MaxStringLength` | Maximum character length for player biography |
| `268446603` | `0x10002B8B` | `Inspect_Data` | `PropertyCollection` returned by `0x85370003` |
| `268454410` | `0x10004A0A` | `UI_VitalDisplayOverride` | Selected portrait frame ID |
| `268447416` | `0x10002EB8` | `Skirmish_TraitCostProgression` | Skirmish Mark cost progression array for soldier traits |
| `268447418` | `0x10002EBA` | `Trait_Custom_Skirmish_TraitRankCap` | Maximum trainable rank for a Skirmish Soldier trait |
| `268455567` | `0x10004E8F` | `Mount_Appearances_Slot_Colourization` | Mounted combat cosmetic appearance configuration |
| `268443012` | `0x10001D84` | `Hobby_Type` | Hobby enum (`1` = Fishing, `2` = Birding) |
| `268443073` | `0x10001DC1` | `Hobby_Proficiency_PropertyName` | Avatar property ID tracking proficiency for a hobby |
| `268443075` | `0x10001DC3` | `Effect_HookedFish_Quality` | Hooked fish quality property on fishing bobber/avatar effect |
| `268443093` | `0x10001DD5` | `Hobby_ToolCategory` | Required tool category in slot `0x20000000` (Fishing Pole / Spyglass) |
| `268443381` | `0x10001EF5` | `Hobby_Skill` | Toggle Skill DID used to execute the hobby |
| `268443382` | `0x10001EF6` | `Hobby_Status` | Current hobby execution state on the avatar (`0` = idle) |
| `268443678` | `0x1000201E` | `Hobby_Fishing_Proficiency` | Avatar's current Fishing proficiency rank |

---

## Part II — Clean-Room Specification: Cosmetics, Wardrobe, Vault/Shared Storage, Carry-Alls, Inspection, Skirmish Traits, & Hobbies

* **Status:** Draft
* **Last Updated:** 2026-10-21
* **Author:** Spec writer (dirty room)
* **Reviewer:** pending

### 1. Purpose <a id="spec-purpose"></a>

This specification defines the wire-level entity RPC identifiers, argument frame field orders, property identifiers, and state transitions required for a clean-room client to interoperate with Cosmetic Outfits, Equipment Slot Visibility, the Barber interface, the Account Wardrobe, Mounted Combat Cosmetics, Character Vault and Account Shared Storage, Carry-All resource bags, Player Inspection and Biography, Trait and Skirmish Soldier configuration, and Hobbies (Fishing and Birding).

### 2. Provenance <a id="spec-provenance"></a>

| Tag | Method | Source | Scope |
|---|---|---|---|
| `OBS` | M1 (Install Observation) | Read-only inspection of `client_highres.dat` Master Property Definition (`0x34000000`) and embedded bytecode (`0x30000000`) | RPC IDs, argument types and order, property IDs, enum values, and state machine conditions |
| `BIN` | M4 (Binary Static Analysis) | `lotroclient64.exe` (SHA-256 `b08fb0ab2fb67ccb9acd2dc9ddeac1ebdc6fd6bbdb63fc3dc7adbfebff6d67a5`), analysed with Ghidra 11.3.2 | Native argument-frame serializer bindings and UI-to-script entry points; required because native serializer primitives are not defined in `.dat` files |

### 3. Common Entity RPC Framing <a id="spec-rpc-framing"></a>

All messages specified below are transmitted as entity-targeted RPC events (`NetBlob` event `0x30`) addressed to the local player avatar's 64-bit entity `InstanceID` (`IID`). Each RPC payload begins with a 32-bit little-endian `rpc_id` followed by a serialized argument frame whose fields are written and read strictly in the order listed in the tables below.

* Integer fields (`u32`, `u64`) and floating-point fields (`f32`) are little-endian.
* Boolean flags are encoded as 32-bit unsigned integers (`0` = false, `1` = true).
* `BankAddressRef` represents a serialized bank address object containing a 32-bit `bank_type` (`1` = Character Vault, `2` = Account Shared Storage, `3` = Shared Housing Storage, `4` = Standard Housing Chest) and a 64-bit `owner_iid`.

### 4. Cosmetic Outfits, Visibility, Barber, & Wardrobe <a id="spec-cosmetics-wardrobe"></a>

#### 4.1 Cosmetic Outfit & Slot Visibility RPCs <a id="spec-outfit-rpcs"></a>

Active outfit index is tracked on the avatar via property `0x10001C64` (`0` = Equipment Outfit, `1..N` = Cosmetic Outfit `1..N`). Per-outfit slot visibility bitmasks are mapped via property `0x10006B0E`, with default all-visible mask `0x01FFFFFE` (`0x10001C5D`). Client requests are gated by cooldown property `0x10001C4C`.

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x881B0005` | C→S | Select Active Outfit | 1. `outfit_type: u32` (`0` = Equipment, `1..N` = Cosmetic `1..N`) |
| `0x881B000A` | S→C | Select Outfit Failed | 1. `reverted_outfit_type: u32` |
| `0x881B0007` | C→S | Equip Cosmetic Item | 1. `wardrobe_item_props: PropertyBagRef` (null if equipping from inventory; `0x10002F15` bag with `0x10002F13` `u32` DID, `0x10002F12` `f32` color, and `0x10003AAD` `u64` banker IID if equipping from Wardrobe)<br>2. `item_iid: u64` (non-zero inventory IID, or `0` if from Wardrobe)<br>3. `slot_bitmask: u32`<br>4. `outfit_type: u32` |
| `0x881B000B` | C→S | Unequip Cosmetic Slot | 1. `slot_bitmask: u32`<br>2. `outfit_type: u32` |
| `0x881B000D` | C→S | Set Slot Visibility | 1. `b_visible: u32` (`0` = hidden, `1` = visible)<br>2. `slot_bitmask: u32`<br>3. `outfit_type: u32` |
| `0x881B0010` | S→C | Slot Visibility Result | 1. `b_visible: u32`<br>2. `slot_bitmask: u32`<br>3. `outfit_type: u32` |

#### 4.2 Barber (`AppearanceUI`) RPCs <a id="spec-barber-rpcs"></a>

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x883A0003` | S→C | Open Barber UI | 1. `barber_npc_iid: u64` |
| `0x883A0001` | C→S | Commit Appearance Changes | 1. `appearance_props: PropertyBagRef` |
| `0x883A0005` | S→C | Close Barber UI | *(empty)* |

#### 4.3 Account Wardrobe RPCs <a id="spec-wardrobe-rpcs"></a>

Account Wardrobe entries are stored on the client in `0x10002FF2` as an array of `0x10002F15` property bags containing item prototype DataID `0x10002F13` (`u32`) and unlocked dye bitmasks `0x10002F16` (`u32`) / `0x10006392` (`u64`). Requests are gated by cooldown property `0x100030AA`.

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x8A45000F` | C→S | Request Open Wardrobe | 1. `banker_iid: u64` |
| `0x8A450001` | S→C | Open Wardrobe | 1. `banker_iid: u64` |
| `0x8A450003` | C→S | Close Wardrobe | *(empty)* |
| `0x8A450007` | C→S | Remove Wardrobe Item | 1. `banker_iid: u64`<br>2. `item_did: u32` |
| `0x8A450009` | C→S | Add Inventory Item to Wardrobe | 1. `banker_iid: u64`<br>2. `item_iid: u64` |
| `0x8A45000B` | C→S | Add Bank Item to Wardrobe | 1. `banker_iid: u64`<br>2. `bank_item_iid: u64`<br>3. `bank_address: BankAddressRef` |
| `0x8A45000D` | C→S | Dye Wardrobe Item | 1. `banker_iid: u64`<br>2. `item_did: u32`<br>3. `dye_iid: u64` |

#### 4.4 Mounted Combat Cosmetic RPCs <a id="spec-mount-cosmetic-rpcs"></a>

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x8DA70007` | C→S | Apply Mount Cosmetic Dye | 1. `slot_type: u32`<br>2. `color_id: u32`<br>3. `trait_did: u32`<br>4. `dye_item_iid: u64` |
| `0x8DA70009` | C→S | Select Mount Appearance Trait | 1. `slot_type: u32`<br>2. `trait_did: u32` |
| `0x8DA7000B` | C→S | Select Mount Appearance Color | 1. `color_id: u32`<br>2. `trait_did: u32` |
| `0x8DA7000D` | C→S | Set Mount Slot Visibility | 1. `b_visible: u32`<br>2. `slot_type: u32` |
| `0x8DA7000F` | C→S | Save Mount Name | 1. `mount_name: WStringRef` |
| `0x8DA70011` | S→C | Mount Cosmetic Updated | *(empty)* |
| `0x8DA70013` | C→S | Purchase Mount Cosmetic Slot | *(empty)* |

### 5. Vault, Shared Storage, & Carry-All Bags <a id="spec-vault-carryall"></a>

#### 5.1 Vault & Shared Storage (`BankRepositorySystem`) RPCs <a id="spec-bank-rpcs"></a>

Requests are gated by cooldown property `0x10002DF3`. Each stored item carries chest assignment property `0x10002DDD` (`0` = Main Vault, `1..10` = Chests 1..10).

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x8A040029` | C→S | Open Bank | 1. `bank_addr: BankAddressRef` |
| `0x8A04002D` | C→S | Close Bank | *(empty)* |
| `0x8A04000B` | C→S | Deposit Item to Bank | 1. `quantity: u32`<br>2. `chest_type: u32`<br>3. `dest_merge_item_iid: u64`<br>4. `src_item_iid: u64`<br>5. `dest_bank_addr: BankAddressRef` |
| `0x8A040013` | C→S | Withdraw Item from Bank | 1. `b_to_equipped: u32`<br>2. `quantity: u32`<br>3. `dest_slot: u32`<br>4. `dest_merge_item_iid: u64`<br>5. `src_item_iid: u64`<br>6. `src_bank_addr: BankAddressRef` |
| `0x8A04001F` | C→S | Transfer Item Between Banks | 1. `quantity: u32`<br>2. `dest_chest_type: u32`<br>3. `dest_merge_item_iid: u64`<br>4. `dest_bank_addr: BankAddressRef`<br>5. `src_item_iid: u64`<br>6. `src_bank_addr: BankAddressRef` |
| `0x8A040021` | C→S | Move Bank Item to Chest Tab | 1. `dest_chest_type: u32`<br>2. `item_iid: u64`<br>3. `bank_addr: BankAddressRef` |
| `0x8A04001B` | C→S | Purchase Bank Upgrade | 1. `banker_iid: u64`<br>2. `bank_addr: BankAddressRef` |
| `0x8A04003B` | C→S | Rename Bank Chest Tab | 1. `new_name: WStringRef`<br>2. `chest_type: u32` (`1..10`)<br>3. `bank_addr: BankAddressRef` |
| `0x8A04003F` | C→S | Deposit Housing Decoration to Bank | 1. `hook_index: u32`<br>2. `chest_type: u32`<br>3. `decoration_iid: u64`<br>4. `house_iid: u64`<br>5. `dest_bank_addr: BankAddressRef` |
| `0x8A04001D` | S→C | Bank Operation Result | 1. `result_pid: u32`<br>2. `bank_addr: BankAddressRef` |
| `0x8A040026` | S→C | Bank Opened | 1. `b_read_only: u32`<br>2. `max_capacity: u32`<br>3. `bank_addr: BankAddressRef` |
| `0x8A040023` | S→C | Bank Closed | 1. `bank_addr: BankAddressRef` |
| `0x8A040028` | S→C | Bank Upgrade Available | 1. `upgrade_tier: u32`<br>2. `banker_iid: u64`<br>3. `bank_addr: BankAddressRef` |

#### 5.2 Carry-All Bag (`BoS`) RPCs <a id="spec-carryall-rpcs"></a>

Carry-All bags store stacks in `0x10006782` as an array of `0x1000677B` property bags (`0x10006780` `u32` DID, `0x10006789` `u32` total quantity, `0x10006775` `u32` account-bound quantity, `0x10006788` `u32` character-bound quantity). Requests are gated by cooldown property `0x100068CA`.

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x90550001` | C→S | Deposit Item to Carry-All | 1. `item_iid: u64`<br>2. `carry_all_iid: u64` |
| `0x90550002` | C→S | Withdraw Item from Carry-All | 1. `bound_filter_type: u32`<br>2. `quantity: u32`<br>3. `item_did: u32`<br>4. `carry_all_iid: u64` |
| `0x90550003` | C→S | Delete Item from Carry-All | 1. `item_did: u32`<br>2. `carry_all_iid: u64` |
| `0x90550004` | C→S | Gather Backpack Items into Carry-All | 1. `carry_all_iid: u64` |
| `0x90550005` | C→S | Set Carry-All Option | 1. `b_enabled: u32`<br>2. `option_type: u32`<br>3. `carry_all_iid: u64` |
| `0x90550014` | C→S | Request Carry-All Contents | 1. `carry_all_iid: u64` |
| `0x9055001A` | C→S | Set Carry-All Pseudonym | 1. `pseudonym: WStringRef`<br>2. `carry_all_iid: u64` |
| `0x9055000D` | S→C | Carry-All Item Updated | 1. `bos_item_struct: PropertyBagRef`<br>2. `carry_all_iid: u64` |
| `0x90550010` | S→C | Carry-All Item Removed | 1. `item_did: u32`<br>2. `carry_all_iid: u64` |
| `0x90550015` | S→C | Carry-All Item Array Chunk | 1. `bos_item_array_bag: PropertyBagRef`<br>2. `carry_all_iid: u64` |
| `0x90550020` | S→C | Carry-All Sync Complete | 1. `carry_all_iid: u64` |

### 6. Player Inspection, Traits, Skirmish Soldiers, & Hobbies <a id="spec-inspect-traits-hobbies"></a>

#### 6.1 Player Inspection & Biography RPCs <a id="spec-inspect-rpcs"></a>

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x84FD000A` | C→S | Request Inspect Equipment & Cosmetics | 1. `target_iid: u64` |
| `0x84FD0010` | C→S | Request Inspect Biography | 1. `target_iid: u64` |
| `0x84FD0025` | C→S | Request Inspect Traits & Effects | 1. `target_iid: u64` |
| `0x84FD0011` | S→C | Inspect Equipment & Cosmetics Data | 1. `sub_element_props_map: MapRef<u32, PropertyBagRef>` (keyed by UI sub-element ID)<br>2. `secondary_player_info_props: PropertyBagRef`<br>3. `primary_player_info_props: PropertyBagRef`<br>4. `quest_category_did: u32` (`0x1000006D`)<br>5. `raid_image_did: u32` (`0x1000006C`)<br>6. `vendor_filter_val: u32` (`0x10000065` / `0x0050AA9C`)<br>7. `extra_player_info_props: PropertyBagRef`<br>8. `phys_obj_info_props: PropertyBagRef`<br>9. `item_exam_array: ArrayRef` (item examination records)<br>10. `outfit_tooltip_array: ArrayRef<PropertyBagRef>` (each bag containing `0x10001C67` `u32` outfit type and `0x10001C49` `Array` of per-slot tooltip bags with `0x10000421` `u32` DID and `0x100012C5` `Array` containing `0x0000034E` `u32` slot mask)<br>11. `avatar_visual_props: PropertyBagRef`<br>12. `header_player_info_props: PropertyBagRef`<br>13. `active_outfit_type: u32` (`0` = Equipment, `1..N` = Cosmetic `1..N`)<br>14. `target_iid: u64` |
| `0x84FD0016` | S→C | Inspect Error | 1. `error_pid: u32`<br>2. `target_iid: u64` |
| `0x84FD0017` | S→C | Inspect Biography Data | 1. `error_pid: u32`<br>2. `bio_text: WStringRef`<br>3. `target_iid: u64` |
| `0x84FD0028` | S→C | Inspect Traits & Effects Data | 1. `traits_props: PropertyBagRef`<br>2. `target_iid: u64` |
| `0x85370017` | C→S | Request Inspect Property Bag | 1. `target_iid: u64` |
| `0x85370003` | S→C | Inspect Property Bag Data | 1. `inspect_props: PropertyBagRef` |
| `0x8537001E` | C→S | Set Vital Display Portrait Override | 1. `override_id: u32` |
| `0x8507000F` | C→S | Set Player Bio or Item Inscription | 1. `text: WStringRef`<br>2. `target_entity_iid: u64` (avatar IID for `0x10000A75` biography; item IID for `0x10000A74` inscription) |

#### 6.2 Trait Slotting, Skirmish Soldier Traits, & Preset Configurations <a id="spec-trait-rpcs"></a>

Skirmish Soldier traits (role, attribute, training, personal, and soldier cosmetic appearance traits) use the same trait slotting and rank-acquisition RPCs as character traits, distinguished by their `Trait_Nature` (`0x100007C1`) and cost progression (`0x10002EB8`).

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x85320008` | C→S | Unslot All Pending Traits | *(empty)* |
| `0x8532000E` | C→S | Unslot Trait by DID | 1. `trait_did: u32` |
| `0x85320016` | C→S | Slot Trait | 1. `slot_descriptor: Ref`<br>2. `trait_did: u32` |
| `0x8532001F` | C→S | Commit Pending Traits | *(empty)* |
| `0x85320021` | C→S | Cancel Pending Traits | *(empty)* |
| `0x8532003D` | C→S | Acquire / Upgrade Trait Rank | 1. `slot_descriptor: Ref`<br>2. `trait_did: u32` |
| `0x85320045` | C→S | Request Trait Tree Respec | *(empty)* |
| `0x85320049` | C→S | Set Active Trait Nature | 1. `trait_nature: u32` |
| `0x8532004B` | C→S | Slot Trait with Flags | 1. `flags: u32`<br>2. `slot_descriptor: Ref`<br>3. `trait_did: u32` |
| `0x8532004F` | C→S | Set Trait Suppression Toggle | 1. `b_suppressed: u32`<br>2. `trait_did: u32` |
| `0x85320057` | C→S | Barter for Trait | 1. `trait_did: u32` |
| `0x8E150001` | C→S | Select Trait/Mount Configuration Preset | 1. `slot_idx: u32`<br>2. `config_type: u32` (`1` = Specialization, `2` = Mount Cosmetics) |
| `0x8E150005` | C→S | Save Trait/Mount Configuration Preset | 1. `config_props: PropertyBagRef`<br>2. `flags: u32`<br>3. `slot_idx: u32`<br>4. `config_type: u32` |
| `0x8E150009` | C→S | Purchase Configuration Preset Slot | 1. `config_type: u32` |
| `0x8E15000B` | C→S | Rename Configuration Preset Slot | 1. `name: WStringRef`<br>2. `slot_idx: u32`<br>3. `config_type: u32` |
| `0x8E15000E` | S→C | Configuration Preset Updated | 1. `config_props: PropertyBagRef`<br>2. `config_type: u32` |
| `0x8E150013` | S→C | Configuration Preset Applied | 1. `config_type: u32` |

#### 6.3 Hobby System (Fishing & Birding) RPCs & Execution Flow <a id="spec-hobby-rpcs"></a>

Hobby training and proficiency tracking use the `0x8859xxxx` RPC family below. Active execution in the world (casting/reeling a Fishing line or using Birding) is performed by activating the hobby's toggle skill (`0x10001EF5` `Hobby_Skill`) while an implement matching `0x10001DD5` (`Hobby_ToolCategory`) is equipped in slot `0x20000000` and `0x10001EF6` (`Hobby_Status`) is `0`.

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x88590001` | C→S | Untrain / Abandon Hobby | 1. `hobby_did: u32` |
| `0x88590003` | C→S | Train Hobby | 1. `hobby_did: u32` |
| `0x8859000B` | S→C | Offer Hobby Training | 1. `hobby_did: u32`<br>2. `trainer_npc_iid: u64` |
| `0x88590011` | S→C | Hobby Trained | 1. `hobby_did: u32` |
| `0x88590026` | S→C | Reset Daily Hobby Proficiency | 1. `hobby_did: u32` |
| `0x88590028` | S→C | Record Territory Proficiency Gain | 1. `territory_did: u32` |

### 7. Testable Invariants & Synthetic Test Vectors <a id="spec-invariants-vectors"></a>

#### 7.1 Invariants
1. **Outfit Equip Source Exclusivity:** In `0x881B0007` (`OutfitEquip`), either `item_iid != 0` and `wardrobe_item_props` is null (equipping from inventory), or `item_iid == 0` and `wardrobe_item_props` is a non-null `0x10002F15` property collection containing `0x10002F13` (`Wardrobe_ItemDID`).
2. **Slot Visibility Mask Preservation:** Toggling visibility of `slot_bitmask` via `0x881B000D` updates only the bits present in `slot_bitmask` within the outfit's visibility property (`0x10006B0E`), defaulting to `0x01FFFFFE` (`0x10001C5D`) when unset.
3. **Carry-All Bound Quantity Partition:** For every `0x1000677B` (`BoS_ItemStruct`) entry in `0x10006782`, `BoS_Bound_To_Account_Amount (0x10006775) + BoS_Bound_To_Character_Amount (0x10006788) <= BoS_Quantity (0x10006789)`.
4. **Hobby Execution Precondition:** A client must not dispatch the toggle skill in `0x10001EF5` (`Hobby_Skill`) unless `0x10001EF6` (`Hobby_Status`) equals `0` and the equipped item in slot `0x20000000` has category matching `0x10001DD5` (`Hobby_ToolCategory`).

#### 7.2 Synthetic Test Vectors (Little-Endian Argument Payloads)
* **Vector 1 — Select Cosmetic Outfit 2 (`0x881B0005`):**
  * `rpc_id`: `05 00 1B 88` (`0x881B0005`)
  * `outfit_type` (`u32`): `02 00 00 00` (`2`)
* **Vector 2 — Hide Head Slot (`0x00000004`) on Cosmetic Outfit 1 (`0x881B000D`):**
  * `rpc_id`: `0D 00 1B 88` (`0x881B000D`)
  * `b_visible` (`u32`): `00 00 00 00` (`0` = hidden)
  * `slot_bitmask` (`u32`): `04 00 00 00` (`0x00000004`)
  * `outfit_type` (`u32`): `01 00 00 00` (`1`)
* **Vector 3 — Withdraw 50 Unbound Items (`DID = 0x70012345`) from Carry-All (`IID = 0x0102030405060708`) (`0x90550002`):**
  * `rpc_id`: `02 00 55 90` (`0x90550002`)
  * `bound_filter_type` (`u32`): `00 00 00 00` (`0`)
  * `quantity` (`u32`): `32 00 00 00` (`50`)
  * `item_did` (`u32`): `45 23 01 70` (`0x70012345`)
  * `carry_all_iid` (`u64`): `08 07 06 05 04 03 02 01` (`0x0102030405060708`)
* **Vector 4 — Train Fishing Hobby (`DID = 0x7000A1B2`) (`0x88590003`):**
  * `rpc_id`: `03 00 59 88` (`0x88590003`)
  * `hobby_did` (`u32`): `B2 A1 00 70` (`0x7000A1B2`)

### 8. Implementer Notes (G4 & G5 Compliance) <a id="spec-implementer-notes"></a>

* **G4 (Ban-Risk Warning):** Before any network connection to an official server can be initiated, the client UI must present a prominent ban-risk warning that requires explicit user confirmation in the client's own interface.
* **G5 (No Secrets, Keys, or Bypass):** All property definitions (`0x34000000`) and hobby/trait/item descriptors must be loaded at runtime from the user's local installation `.dat` files. Never embed proprietary assets, keys, or server endpoints in source code or binaries, and respect all server-enforced cooldown (`Antispam_*`) and account entitlement (`Account_Outfit_System_Available`, `Wardrobe_MaxCapacity`, `BankRepository_MaxCapacity`) properties without modification or bypass.

### 9. Change Log <a id="spec-changelog"></a>

| Date | Author | Summary |
|---|---|---|
| 2026-10-21 | Spec writer (dirty room) | Initial draft covering Cosmetics, Wardrobe, Vault/Shared Storage, Carry-Alls, Inspection, Skirmish Traits, and Hobbies |
