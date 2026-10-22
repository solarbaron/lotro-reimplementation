<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 Gameplay P1 Research Note: Inventory, Containers, Equipment & Character Property/Level Updates

- **Date:** 2026-10-21
- **Author:** Spec writer (dirty room)
- **Target Binary (`BIN`):** `x64/lotroclient64.exe` (PE32+ x86-64, FileVersion `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) and embedded WLib bytecode image (`client_local_English.dat` DID `0x34000000` + native package `48` / `0x0030`)
- **Scope:** Official server wire protocol for P1 Inventory, Containers, Equipment (`InventoryEvent`, `InventoryCommand`), and In-World Character Property & Level Updates (`CharacterEvent::PropertyChanged`, `CharacterEvent::LevelChanged`).

---

## 1. Inventory & Equipment Container Delivery (`InventoryEvent`)

### 1.1 Architectural Overview: Items as Contained Entities vs. Repository Records

In the LOTRO client (`lotroclient64.exe` + WLib `0x34000000`), inventory and storage are split into two distinct wire models:

1. **Entity-Backed Containers (Player Backpack, Equipped Paperdoll, World/Corpse Loot Containers):**
   - Every item instance in the player's Backpack or Equipment is a full **WState Entity (`CBaseEntity`)** identified by a 64-bit `InstanceID` (`u64`, typically with type prefix `0x03XXXXXXXXXXXXXX`) and a 32-bit item template `DataID` (`DID`, prefix `0x70XXXXXX`, stored in entity `m_did` and Property `0x00000001`).
   - Item entities carry WState control classes **`2804` (`0x0AF4`, `CInventoryControl`)**, **`2350` (`0x092E`, `CMEOInventoryControl`)**, and **`1208` (`0x04B8`, `CItemWearControl`)**, and store all per-instance state (slot bitmask, stack quantity, current/max durability, wear state, bound status, paperdoll visual DID, dye color, item level, quality) in their entity `PropertyCollection`.
   - Parent-child membership between a container entity (the player's avatar entity for Backpack and Equipment, or a corpse/chest entity for world container loot) and its contained item entities is maintained by the **C++ Entity Containment Component** at `[entity + 0x48]` (`vtable 0x14150cbc0`, `176` bytes = `0xB0`), exposed to WLib scripts via **Native Package `48` (`0x0030`, `CInventoryPackage`)**.
2. **Property/RPC-Backed Repositories (Vault, Shared Storage, Wardrobe, Carry-Alls, Pending Loot):**
   - **Vault & Shared Storage** are managed by WLib **`CBankRepositorySystem` (`Class 2564` / `0x0A04`, RPC prefix `0x8A04XXXX`)** and **`CMEOBankRepositorySystem` (`Class 2562` / `0x0A02`, RPC prefix `0x8A02XXXX`)**.
   - **Carry-All Bags (`BagOfSingletons`)** are managed by WLib **`CBagOfSingletonsSystem` (`Class 4181` / `0x1055`, RPC prefix `0x9055XXXX`)**.
   - **Wardrobe** is managed by WLib **`CWardrobeSystem` (`Class 2629` / `0x0A45`, RPC prefix `0x8A45XXXX`)**.
   - **Cosmetic Outfits** are managed by WLib **`COutfitSystem` (`Class 2075` / `0x081B`, RPC prefix `0x881BXXXX`)**.
   - **Pending Loot (Overflow Loot List)** is stored in avatar entity properties (`0x10003684: Loot_PendingLootList`, `0x10003685: Loot_PendingLootTimers`) and claimed via **`CInventorySystem` (`Class 1290` / `0x050A`, RPCs `0x850A003E` / `0x850A0042`)** and **`CLootSystem` (`Class 1298` / `0x0512`, RPC prefix `0x8512XXXX`)**.

---

### 1.2 C++ Entity Containment Component (`[entity + 0x48]`, `vt 0x14150cbc0`) & Automatic Slot Indexing (`PID 0x0000034E`)

Every `CBaseEntity` allocates a containment component at `[entity + 0x48]` (`0x140517ad0`, called from `CBaseEntity` init `0x140710d08`, primary vtable `0x14150cbc0`, secondary property-observer vtable `0x14150cd38` at `+0x08`).

#### Internal Layout of `[entity + 0x48]` (`0xB0` = `176` bytes)
| Offset | Size | Type | Description |
|---|---|---|---|
| `+0x00` | `8` | `void*` | Primary vtable `0x14150cbc0` (`36` virtual methods) |
| `+0x08` | `8` | `void*` | Secondary `IPropertyObserver` vtable `0x14150cd38` (`vslot 2` = `0x1405186f0`, observes `PID 0x0000034E` on contained children) |
| `+0x10` | `8` | `CBaseEntity*` | `owner_entity` (back-pointer to owning `CBaseEntity`) |
| `+0x18..+0x34` | `32` | `IntrusiveHashTable<u64, ContainedNode>` | Primary hash table of contained child entities keyed by `u64 child_iid` (`+0x20` buckets array, `+0x30` bucket count, `+0x34` element count). Each `0x28`-byte (`40`-byte) `ContainedNode` holds: `+0x00` `u64 child_iid`, `+0x08` `ContainedNode* next`, `+0x10` `u32 containment_type`, `+0x18` `CBaseEntity* child_entity`, `+0x20` `AttachmentInfo* attachment` (`vt 0x14150cea8`, `NULL` unless attached) |
| `+0x40..+0x5C` | `32` | `IntrusiveHashTable<u32, SlotNode>` | Secondary hash table of contained child entities keyed by **`u32 slot_bitmask` (`PID 0x0000034E`)** (`+0x48` buckets array, `+0x58` bucket count, `+0x5C` element count). Each `0x18`-byte (`24`-byte) `SlotNode` holds: `+0x00` `u32 slot_bitmask`, `+0x08` `SlotNode* next`, `+0x10` `CBaseEntity* child_entity` |
| `+0x60` | `8` | `void*` | Attachment list/state |
| `+0x6C` | `4` | `u32` | `attachment_count` |
| `+0x70` | `8` | `u64` | `parent_container_iid` (`0` if uncontained / in world) |
| `+0x78` | `8` | `CBaseEntity*` | `parent_container_entity` (`NULL` if uncontained) |
| `+0x80..+0xAC` | `48` | `Transform` | Local attachment transform & flags |

#### Containment Type Constants (`u32 containment_type` at `ContainedNode + 0x10`)
- **`0x00000010` (`16`)**: Attached / Wielded visual child entity (`AttachEntity`, `pkg 48 fn 2`, `0x140517a40`).
- **`0x00000100` (`256`)**: Equipped item (`GetEquippedItemIIDs`, `pkg 48 fn 6`, `0x1405166c0`; `PromoteInventoryToEquipped`, `pkg 48 fn 17`, `0x140586d40`).
- **`0x00001000` (`4096`)**: Inventory / Contained item (`AddItemToContainer`, `pkg 48 fn 1`, `0x140516d40`; `DemoteEquippedToInventory`, `pkg 48 fn 16`, `0x140586e30`).

#### Automatic Slot Indexing via Property `0x0000034E` (`846`, `Inventory_Slot`, `Type 0x12` = `Bitfield32`)
- When a child entity is added to a container (`vslot 24 (+0xc0)` `0x140516d40` or `vslot 34 (+0x110)` `0x140517cf0`), the container calls `0x140583c50(child_entity)` (`containment_ser.txt` lines 407–423), which:
  1. Queries **Property `0x0000034E` (`846`)** on `child_entity` (`vslot +0x80` on `CBaseEntity`, returns `u32` when property type is `0x12` `Bitfield32`).
  2. Inserts `(slot_bitmask, child_entity)` into the container's secondary slot hash table at `[container + 0x40]`.
  3. Registers `[container + 0x08]` (`vt 0x14150cd38`, `vslot 2` = `0x1405186f0`) as a **property observer for `PID 0x0000034E`** on `child_entity` (`vslot +0x240` on `CBaseEntity`).
- Whenever the server sends any property update (`0x88`, `0x92`, `0x52`, or `0xB7`) that modifies **`PID 0x0000034E`** on a contained item entity, `0x1405186f0` invokes `0x140519560(container, child_entity, new_slot_bitmask)`, which removes the old `SlotNode` for `child_entity` (`0x140518e40`) and inserts a new `SlotNode` under `new_slot_bitmask` in `[container + 0x40]`.
- When an item entity is destroyed (`NetBlob` Event `0x9A`), its containment component calls `vslot 30 (+0xf0)` (`0x140518e20` -> `0x140588b80` -> `vslot 25 (+0xc8)` `0x1405173d0`), which unregisters the `0x0000034E` property observer and removes the item from both `[parent + 0x18]` and `[parent + 0x40]`.

#### Encoding of `PID 0x0000034E` (`Inventory_Slot`, `u32` Bitmask)
Verified from `pkg 48` (`0x140b5ff10..0x140b60800`), `0x140b601e0` (bitmask-to-index helper), `Class 1201` (`InvSupportSystem`), and `Class 1299` (`CMEOInventorySystem` `fnum 20` at `0xb5d38`):

| High Tag Bits (`bits 28..31`) | Low Bits (`bits 0..27`) | Meaning |
|---|---|---|
| `0x40000000` (`bit 30`) | `u32 bag_slot_index` (`0 .. max_slots - 1`) | **Backpack / Container Slot:** `0x40000000 \| slot_index` (e.g. `0x40000000` = Backpack slot 0, `0x40000001` = Backpack slot 1, `0x4000002D` = Backpack slot 45). Queried via `pkg 48 fn 19` (`GetItemAtExactSlot`) and `fn 26` (`GetItemsInSlotRange(container, 0x40000000, min_slot, max_slot)`). |
| `0x20000000` (`bit 29`) | `1 << (equip_slot_index - 1)` (`bits 0..21`, mask `0x003FFFFF`) | **Equipped Paperdoll Slot:** `0x20000000 \| equip_bit` (with `containment_type == 0x0100`). Helper `0x140b601e0` maps the 22 equipment bitflags (`1 << 0` .. `1 << 21`) to 1-based equipment slot indices `1..22`. |
| `0x80000000` (`bit 31`) | `u32 index` | **Overflow / Pending / Unslotted:** temporary/overflow container slot. |
| `0x10000000` (`bit 28`) | `u32 sub_slot` | **Auxiliary / Special Container Slot:** checked in `Class 1299` `fnum 20` (`0xb5d38`: `slot & 0x10000000`). |

#### Equipment Paperdoll Slot Bitflags (`PID 0x0000034E` / `PID 0x10000D88` `Item_EquipmentSlot` / `PID 0x10001516` `Item_UnavailableEquipmentSlots`)
Verified from `0x140b601e0` (`pkg48_dis.txt` lines 1630–1685), `0x140c47330` (paperdoll visual renderer), and `Class 2075` (`COutfitSystem`):

| 1-Based Index (`0x140b601e0`) | Bit Position | Bitflag (`u32`) | Full `0x0000034E` Equipped Value | Paperdoll Slot |
|---|---|---|---|---|
| `1` | Bit 0 | `0x00000001` | `0x20000001` | Head |
| `2` | Bit 1 | `0x00000002` | `0x20000002` | Chest |
| `3` | Bit 2 | `0x00000004` | `0x20000004` | Legs |
| `4` | Bit 3 | `0x00000008` | `0x20000008` | Hands (Gloves) |
| `5` | Bit 4 | `0x00000010` | `0x20000010` | Feet (Boots) |
| `6` | Bit 5 | `0x00000020` | `0x20000020` | Shoulders |
| `7` | Bit 6 | `0x00000040` | `0x20000040` | Back (Cloak) |
| `8` | Bit 7 | `0x00000080` | `0x20000080` | Left Wrist (Bracelet 1) |
| `9` | Bit 8 | `0x00000100` | `0x20000100` | Right Wrist (Bracelet 2) |
| `10` | Bit 9 | `0x00000200` | `0x20000200` | Neck (Necklace) |
| `11` | Bit 10 | `0x00000400` | `0x20000400` | Left Finger (Ring 1) |
| `12` | Bit 11 | `0x00000800` | `0x20000800` | Right Finger (Ring 2) |
| `13` | Bit 12 | `0x00001000` | `0x20001000` | Left Ear (Earring 1) |
| `14` | Bit 13 | `0x00002000` | `0x20002000` | Right Ear (Earring 2) |
| `15` | Bit 14 | `0x00004000` | `0x20004000` | Pocket |
| `16` | Bit 15 | `0x00008000` | `0x20008000` | Primary Weapon (Main-hand) |
| `17` | Bit 16 | `0x00010000` | `0x20010000` | Secondary Weapon / Shield (Off-hand) |
| `18` | Bit 17 | `0x00020000` | `0x20020000` | Ranged Weapon |
| `19` | Bit 18 | `0x00040000` | `0x20040000` | Craft Tool |
| `20` | Bit 19 | `0x00080000` | `0x20080000` | Class Slot |
| `21` | Bit 20 | `0x00100000` | `0x20100000` | Bridle / Mount Slot |
| `22` | Bit 21 | `0x00200000` | `0x20200000` | Aura / Extra Weapon Slot |

---

### 1.3 Wire Serialization of Entity Containment (`[entity + 0x48]` & Top-Level `NetBlob` Event `0xCE`)

#### A. Initial Containment Archive in Entity Creation (`0x140518fc0`, `vslot 7` of `0x14150cbc0`)
During entity creation (`NetBlob` Event `0x98` / `0x52`), `CBaseEntity::Serialize` invokes `vslot 7 (+0x38)` (`0x140518fc0`) on `[entity + 0x48]`, which executes three sequential steps:

1. **`vslot 31 (+0xf8)` (`0x1405192d0`) — Parent Container IID:**
   - Reads `u64 LE parent_container_iid` (`8` bytes) into `[containment + 0x70]`. (`0x0000000000000000` for an uncontained root entity such as the player avatar).
2. **`vslot 32 (+0x100)` (`0x1405190f0`) — Contained Children Table & Child Entity Archives:**
   - Reads `u32 LE attachment_count` (`4` bytes) into `[containment + 0x6c]`.
   - Calls `0x140516080` to deserialize the `[containment + 0x18]` hash table header and link records:
     - `u8 bucket_size_index` (`1` byte, index `< 23` into prime table `0x141a53688`: `[7, 13, 31, 61, 127, 251, 509, 1021, 2039, 4093, ...]`, validated `<= 50000` buckets).
     - `VLE contained_count` (`1..5` bytes, standard 3-tier VLE via `0x140304d00`).
     - `contained_count` link records, each consisting of:
       - `u64 LE child_iid` (`8` bytes)
       - `u32 LE containment_type` (`4` bytes: `0x0010` = attached, `0x0100` = equipped, `0x1000` = inventory)
       - `bool8 has_attachment_data` (`1` byte, `0x140515e50`), and if `has_attachment_data == 1`:
         - `u32 LE attach_id` (`4` bytes)
         - `u8 attach_sub` (`1` byte)
         - `bool8 attach_flag` (`1` byte, validated `< 2`)
   - Loops `contained_count` times (`0x140519220..0x140519283`), calling `IID_EntitySerializer` (`0x14150cb50`, `vslot +0x90`) to deserialize each contained child entity's full `CBaseEntity` archive (entity `InstanceID`, `DID`, WState class, containment component, and `PropertyCollection`).
3. **`vslot 33 (+0x108)` (`0x140519010`) — Post-Deserialization Linking & Slot Indexing:**
   - Iterates all `contained_count` nodes in `[containment + 0x18]` and calls `vslot 34 (+0x110)` (`0x140517cf0`), which resolves `child_entity = LookupEntity(child_iid)` (`0x1415384c0` `vslot +0xa8`), sets `[child_containment + 0x70] = owner_iid` and `[child_containment + 0x78] = owner_entity`, reads `PID 0x0000034E` on `child_entity` via `0x140583c50`, inserts `(slot_bitmask, child_entity)` into `[containment + 0x40]`, and attaches the `0x0000034E` property observer (`0x1405186f0`).

#### B. Runtime Containment & Placement Update: Top-Level `NetBlob` Event `0xCE` (`206`)
Verified from constructor `0x140750010` (`vtable 0x141510908`, `event_id = 0xCE`), serializer `0x1405481e0` (`vslot 7`), and handler `0x1407501b0` -> `0x140750de0` (`vslot 1`):

When an existing entity is moved into a container, moved between containers, promoted/demoted between inventory (`0x1000`) and equipped (`0x0100`), attached (`0x0010`), or dropped from a container into the world, the server sends **Top-Level `NetBlob` Event `0xCE` (`206`)**:

| Order | Field | Size | Type | Condition | Description |
|---|---|---|---|---|---|
| 1 | `event_id` | `2` | `u16 LE` | Always | `0x00CE` (`206`) |
| 2 | `flags1` | `1` | `u8` | Always | Optional field presence bitmask (`0x01` = `has_field_84`, `0x02` = `has_field_a8`, `0x04` = `has_field_ac`, `0x08` = `has_sequence_num`, `0x10` = `has_vel_x`, `0x20` = `has_vel_y`, `0x40` = `has_vel_z`) |
| 3 | `entity_iid` | `8` | `u64 LE` | Always | Target child/moving entity `InstanceID` (`[event + 0x38]`) |
| 4 | `mode_flags` | `1` | `u8` | Always | Placement mode bitmask (`[event + 0x88]`). Bit `0x04` (`(mode_flags >> 2) & 1`): **`is_contained`**. Bit `0x08` (`(mode_flags >> 3) & 1`): **`has_attachment_info`**. Bit `0x10` (`(mode_flags >> 4) & 1`): **`position_only_no_quat`** |
| 5 | `sequence_num` | `1` | `u8` | `flags1 & 0x08` | Sequence counter (`[event + 0x8c]`), checked modulo 256 (`delta <= 0x7f`) against `[entity + 0x140]` |
| 6 | `vel_x` | `4` | `f32 LE` | `flags1 & 0x10` | Linear velocity X (`[event + 0x78]`, default `0.0`) |
| 7 | `vel_y` | `4` | `f32 LE` | `flags1 & 0x20` | Linear velocity Y (`[event + 0x7c]`, default `0.0`) |
| 8 | `vel_z` | `4` | `f32 LE` | `flags1 & 0x40` | Linear velocity Z (`[event + 0x80]`, default `0.0`) |
| 9 | `field_84` | `1..5` | `VLE u32` | `flags1 & 0x01` | 3-tier VLE (`[event + 0x84]`, default `0`) |
| 10a | `parent_container_iid` | `8` | `u64 LE` | `(mode_flags & 0x04) != 0` | `InstanceID` of parent container entity (`[event + 0x90]`) |
| 10b | `containment_type` | `1..5` | `VLE u32` | `(mode_flags & 0x04) != 0` | 3-tier VLE (`[event + 0x98]`): `0x1000` (inventory), `0x0100` (equipped), or `0x0010` (attached) |
| 10c | `attachment_info` | `6` | `struct` | `(mode_flags & 0x04) != 0 && (mode_flags & 0x08) != 0` | Deserialized by `0x140518f00`: `u32 LE attach_id` (`4B`) + `u8 attach_sub` (`1B`) + `bool8 attach_flag` (`1B`, `< 2`) |
| 11a | `world_pos` | var | `Position` | `(mode_flags & 0x04) == 0 && (mode_flags & 0x10) != 0` | World `Position` (`0x140372d60`) without orientation |
| 11b | `world_pos_rot` | var | `Position + Quat` | `(mode_flags & 0x04) == 0 && (mode_flags & 0x10) == 0` | World `Position` (`0x140372d60`) + `Quaternion` (`0x14115bb00`) |
| 12 | `field_a8` | `1` | `u8` | `flags1 & 0x02` | `[event + 0xa8]` (default `5`) |
| 13 | `field_ac` | `1` | `u8` | `flags1 & 0x04` | `[event + 0xac]` (default `2`) |

**Handler Execution (`0x140750de0`):**
- If `parent_container_iid != 0` (`(mode_flags & 0x04) != 0`):
  - Looks up `parent_entity = LookupEntity(parent_container_iid)`.
  - If `parent_entity` is present in the client entity table:
    - If `attachment_info == NULL`: calls `0x140581610(parent_entity, child_entity, containment_type, 0)` -> `vslot +0xc0` (`0x140516d40`), which adds `child_entity` to `parent_entity` (or updates its `containment_type` via `0x140517830` if already contained in `parent_entity`) and indexes its `PID 0x0000034E` slot in `[parent + 0x48 + 0x40]`.
    - If `attachment_info != NULL`: calls `0x1405814a0` -> `vslot +0xe8` (`0x140517a40`) to attach `child_entity` to `parent_entity` at `attach_id`.
  - If `parent_entity` has not yet arrived on the client: queues `(child_entity, event_0xce)` in the pending-containment table (`0x140769ee0` on `[0x141e5a0f8]`), which is drained by `0x140751f30` as soon as `parent_entity` is registered.
- If `parent_container_iid == 0` (`(mode_flags & 0x04) == 0`):
  - Removes `child_entity` from its previous parent container (`0x14076a850`, `0x140586d20` / `0x140588bf0`) and applies its world `Position` via `0x14058a6a0`.

---

### 1.4 Native Package `48` (`0x0030`, `CInventoryPackage`) Method Table (`0x14191e6e0`)

Registered at `0x140bd1210` (`package_id = 48` = `0x0030`, 30 functions):

| `fnum` | Function ID | Implementation VA | Signature & Behavior |
|---|---|---|---|
| `1` | `0x00300001` | `0x140b5ff10` -> `0x140581610` | `AddItemToContainer(container: Entity, item: Entity, containment_type: u32) -> bool` (calls `vslot +0xc0` `0x140516d40`) |
| `2` | `0x00300002` | `0x140b5ffa0` -> `0x140583570` | `AttachEntityWithSlot(parent: Entity, child: Entity, attach_id: u32, slot: u32) -> bool` (calls `vslot +0xe8` and sets `PID 0x00000642` = `attach_id`) |
| `3` | `0x00300003` | `0x140b60040` -> `0x1405837b0` | `TransferEntityContainment(entity_a: Entity, entity_b: Entity, flags: u32) -> bool` |
| `4` | `0x00300004` | `0x140b600d0` -> `0x140584c00` | `GetAllContainedItemIIDs(container: Entity) -> Array<u64>` (iterates `[container + 0x18]`) |
| `5` | `0x00300005` | `0x140b60110` -> `0x140584bc0` | `GetAllContainedItemIIDsRecursive(container: Entity) -> Array<u64>` (`0x140516930`) |
| `6` | `0x00300006` | `0x140b60150` -> `0x140584e00` | `GetEquippedItemIIDs(container: Entity) -> Array<u64>` (filters `[container + 0x18]` where `containment_type == 0x0100`) |
| `7` | `0x00300007` | `0x140b60190` -> `0x1405850c0` | `GetContainedEntity(container: Entity, item_iid: u64) -> Entity` (`vslot +0xb8` `0x140517dd0`) |
| `8` | `0x00300008` | `0x140b601e0` | `EquipBitmaskToSlotIndex(slot_mask: u32) -> u32` (maps `1 << 0..21` to `1..22`, else `0`) |
| `9` | `0x00300009` | `0x140b60360` -> `0x1405868b0` | `ContainsItem(container: Entity, item_iid: u64) -> bool` (`0x140518410`) |
| `10` | `0x0030000A` | `0x140b603b0` -> `0x140586b70` | `GetItemIfInSameRootContainer(entity: Entity, item_iid: u64) -> Entity` |
| `11` | `0x0030000B` | `0x140b60400` -> `0x14050fae0` | `LookupEntityByIID(item_iid: u64) -> Entity` |
| `12` | `0x0030000C` | `0x140b60430` -> `0x140584a10` | `GetDirectParentContainer(item: Entity) -> Entity` (returns `[containment + 0x78]`) |
| `13` | `0x0030000D` | `0x140b60470` -> `0x140585110` | `GetRootParentContainer(item: Entity) -> Entity` (`0x140518120`) |
| `14` | `0x0030000E` | `0x140b604b0` -> `0x1405847f0` | `HaveSameRootParentContainer(entity: Entity, item_iid: u64) -> bool` |
| `15` | `0x0030000F` | `0x140b60500` -> `0x140584840` | `HasFreeSlotForItem(container: Entity, item_iid: u64) -> bool` (`0x140517ea0`) |
| `16` | `0x00300010` | `0x140b60550` -> `0x140586e30` | `DemoteAllEquippedToInventory(container: Entity)` (`0x0100` -> `0x1000`) |
| `17` | `0x00300011` | `0x140b60580` -> `0x140586d40` | `PromoteAllInventoryToEquipped(container: Entity)` (`0x1000` -> `0x0100`) |
| `18` | `0x00300012` | `0x140b605b0` -> `0x140582260` | `SetContainmentType(container: Entity, item_iid: u64, containment_type: u32) -> bool` (`0x140517830`) |
| `19` | `0x00300013` | `0x140b60600` -> `0x140584d80` | `GetItemAtExactSlot(container: Entity, slot: u32) -> Entity` (`0x140517f10`, looks up `slot` in `[container + 0x40]`) |
| `20` | `0x00300014` | `0x140b60650` -> `0x140585040` | `GetItemsMatchingSlotMask(container: Entity, slot_mask: u32, out_array: Array<u64>) -> bool` (`0x140517f80`, `(item_slot & slot_mask) != 0`) |
| `21` | `0x00300015` | `0x140b606a0` -> `0x140584ec0` | `GetItemsMatchingSlotMaskSorted(container: Entity, slot_mask: u32, out_array: Array<u64>) -> bool` (`0x1405174c0`) |
| `22` | `0x00300016` | `0x140b606f0` -> `0x140586970` | `IsSlotMaskEmpty(container: Entity, slot_mask: u32) -> bool` (`0x140518390`) |
| `23` | `0x00300017` | `0x140b60740` -> `0x140586ed0` | `CountItemsMatchingSlotMask(container: Entity, slot_mask: u32) -> u32` (`0x140518670`) |
| `24` | `0x00300018` | `0x140b60790` -> `0x140584ac0` | `FindFirstFreeSlotInRange(container: Entity, start_slot: u32, end_slot: u32) -> u32` (`0x1405172d0`) |
| `25` | `0x00300019` | `0x140b607e0` -> `0x140585990` | `FindNextFreeSlotWrapping(container: Entity, start_slot: u32, min_slot: u32, max_slot: u32) -> u32` (`0x140518010`) |
| `26` | `0x0030001A` | `0x140b60840` -> `0x140584f40` | `GetItemsInSlotRange(container: Entity, mask: u32, min_slot: u32, max_slot: u32, out_array: Array<u64>) -> bool` (`0x140517650`) |
| `27` | `0x0030001B` | `0x140b608a0` -> `0x140585150` | `GetRootContainerIID(item: Entity) -> u64` |
| `28` | `0x0030001C` | `0x140b608f0` -> `0x140584fc0` | `GetAllItemsAtExactSlot(container: Entity, slot: u32, out_array: Array<u64>) -> bool` (`0x140517730`) |
| `29` | `0x0030001D` | `0x140b60940` -> `0x140585850` | `GetSingleItemAtSlot(container: Entity, slot: u32) -> Entity` (`0x140517ec0`) |
| `30` | `0x0030001E` | `0x140b60990` -> `0x140585040` | `GetItemsWithSlotMask(container: Entity, slot_mask: u32, out_array: Array<u64>) -> bool` |

---

### 1.5 WLib Inventory, Container, Equipment & Storage Systems (S->C & C->S RPCs)

#### A. `CInventorySystem` (Class `1290` / `0x050A`, Base Class `368` / `0x0170`, RPC Prefix `0x850AXXXX`)
Disassembled from `wlib.bin` (`class_1290.txt`, `class_368.txt`, `class_2804.txt`, `class_1201.txt`, `class_1202.txt`):

| Function ID | Dir | Handler Offset | `WLibArgFrame` Parameters (Push Order -> Wire Order) | Purpose & Client Behavior |
|---|---|---|---|---|
| `0x850A0001` | C->S | `368` (`0xca9c8`) | `(dest_slot: u32, dest_container_iid: u64, item_iid: u64)` | **Unequip Item:** Sent by `2804 fnum 12` (`0xcc644`) when unequipping an item to `dest_container_iid` (`avatar_iid` for backpack) at `dest_slot` (`0` = auto-find first free backpack slot, or `0x40000000 \| bag_slot`). |
| `0x850A000B` | C->S | `368` (`0xcaa7c`) | `(quantity: i32, dest_slot: u32, dest_container_iid: u64, item_iid: u64)` | **Move Item / Split Stack:** Sent by `2804 fnum 5` (`0xcc834`) / `2350 fnum 6` (`0xcd058`). `quantity == 0` (or full stack size) moves the entire stack; `0 < quantity < current_stack` splits `quantity` items into `dest_slot` (`0x40000000 \| bag_slot`) in `dest_container_iid`. |
| `0x850A000C` | C->S | `368` (`0xcaab4`) | `(item_iid: u64)` | **Drop Item to Ground / World:** Sent by `2804 fnum 2` (`0xcc5d8`). |
| `0x850A0016` | C->S | `368` (`0xca954`) | `(item_iid: u64)` | **Take / Pickup Item into Inventory:** Sent by `2804 fnum 1` (`0xcc59c`). |
| `0x850A0019` | C->S | `368` (`0xca918`) | `(quantity_or_flags: i32, item_iid: u64)` | **Destroy Item:** Sent by `2804 fnum 4` (`0xcc708`, called from C++ `0x140b66a2e`). Verified not in `0x10001718` (`Usage_ItemsInUse`). |
| `0x850A001A` | C->S | `368` (`0xca8dc`) | `(target_iid: u64, item_iid: u64)` | **Give Item to Entity/NPC:** Sent by `2804 fnum 3` (`0xcc6ac`). |
| `0x850A001D` | C->S | `368` (`0xcaa04`) | `(target_slot: u32, flags: u32, item_iid: u64)` | **Equip Item (or Auto-Use/Equip from Inventory):** Sent by `2804 fnum 6` (`0xcc784`) / `2350 fnum 10` (`0xcd3a4`, called from C++ `0x140b602af`). `target_slot` is `0x20000000 \| (1 << equip_bit)` for a specific paperdoll slot or `0` for default slot. |
| `0x850A001E` | C->S | `368` (`0xcaa40`) | `(flag: bool, item_iid: u64)` | **Toggle Item State / Lock:** Sent by `2804 fnum 15` (`0xcc7dc`). |
| `0x850A0021` | S->C | `1290` (`0xcbe8c`) | `(err_code: u32, item_iid: u64)` | **Inventory Error Notification:** Displays inventory error via `pkg 22 fn 3` and fires UI event `0x00020066` (`131174`). |
| `0x850A0024` | S->C | `1290` (`0xcbe20`) | `(item_iid: u64)` | **OnItemRemovedFromInventory:** Removes `item_iid` from `0x1000147B` (`Inv_NewItemsList`), clears cache `0x100006E8`, and fires UI event `0x00020066` (`131174`). |
| `0x850A002A` | S->C | `1290` (`0xcbdd4`) | `(rc: u32)` | **OnInventoryOperationResult:** Fires UI event `0x000200CA` (`131274`, `rc == 1` success). |
| `0x850A002B` | C->S | `368` (`0xca86c`) | `()` | **Query Inventory / Refresh:** No-arg request. |
| `0x850A002C` | S->C | `1290` (`0xcbd50`) | `(item_did: u32, quantity: i32)` | **OnItemTransmuted / Converted Notification:** Displays chat message (`0x250091FF`). |
| `0x850A002F` | C->S | `368` (`0xca830`) | `(item_iid: u64, target_iid: u64)` | **Apply Item to Target Item** (e.g. dye/essence/scroll application). |
| `0x850A0033` | S->C | `1290` (`0xcba68`) | `(item_iid: u64)` | **OnNewItemAdded:** Appends `item_iid` to `0x1000147B` (`Inv_NewItemsList`), fires UI event `0x00020066` (`131174`), and notifies quest/tutorial systems. |
| `0x850A0034` | C->S | `368` (`0xca7f4`) | `(item_iid: u64)` | **ClearNewItemFlag:** Sent by `1290 fnum 53` (`0xcb9a4`) when player inspects/clears a new item (`0` = clear all). |
| `0x850A0037` | S->C | `1290` (`0xcb960`) | `(success: bool)` | **OnExpandCapacityComplete:** Fires UI event `0x00020168` (`131432`). |
| `0x850A0038` | C->S | `368` (`0xca7bc`) | `(bag_slot_map: HashTable)` | **SaveBagSlotLayout:** Persists `0x10002E4F` (`Inventory_BagSlotMap` bag-to-slot assignment). |
| `0x850A003D` | C->S | `368` (`0xca74c`) | `(item_iid: u64, arg2: u32)` | **Pending Loot / Item Action.** |
| `0x850A003E` | C->S | `368` (`0xca714`) | `(item_iid: u64)` | **ClaimPendingLootItem:** Sent by `1290 fnum 61` (`0xcb850`) to move one item from `0x10003684` (`Loot_PendingLootList`) into backpack. |
| `0x850A0042` | C->S | `368` (`0xca6a4`) | `()` | **ClaimAllPendingLoot:** Sent by `1290 fnum 65` (`0xcb7b4`) to claim all items from `0x10003684`. |
| `0x850A0043` | C->S | `368` (`0xca66c`) | `(item_iid: u64)` | **DiscardPendingLootItem:** Sent by `1290 fnum 66` (`0xcb750`). |
| `0x850A0048` | S->C | `1290` (`0xcb554`) | `(item_did: u32, quantity: i32, reason: u32)` | **OnItemAutoRouted / Filtered:** Notifies chat when an item is auto-routed to Pending Loot or Carry-All. |

#### B. `CMEOInventorySystem` (Class `1299` / `0x0513`, Base Class `372` / `0x0174`, RPC Prefix `0x8513XXXX`)
Disassembled from `wlib.bin` (`class_1299.txt`, `class_372.txt`, `class_2350.txt`):

| Function ID | Dir | Handler Offset | `WLibArgFrame` Parameters | Purpose & Client Behavior |
|---|---|---|---|---|
| `0x85130001` | S->C | `1299` (`0xb6c40`) | `(container_iid: u64, flag: bool)` | **OnOpenContainerLoot:** Opens corpse/chest container loot UI (`0x00020073` / `131187`), sets `0x10000CC0` (`Inventory_OpenContainerID`) on avatar. |
| `0x85130008` | S->C | `1299` (`0xb6b24`) | `(container_iid: u64, err_code: u32)` | **OnContainerLootError:** Clears `0x10000CC0`, fires `0x00020076` (`131190`). |
| `0x8513000C` | S->C | `1299` (`0xb6954`) | `(item_iid: u64, err_code: u32)` | **OnItemOperationFailed:** Fires UI event `0x00020066` (`131174`) and `0x0002006B` (`131179`) to revert optimistic UI drag/drop. |
| `0x8513000D` | S->C | `1299` (`0xb6604`) | `(item_iid: u64)` | **OnContainerItemAdded:** Registers property observer for `PID 0x0000034E` on `item_iid`, inserts into `0x100006E8` (`Inventory_ItemCache`), and fires `0x00020066` (`131174`) or `0x00020074` (`131188`, container item added). |
| `0x8513000E` | S->C | `1299` (`0xb6278`) | `(item_iid: u64, slot: u32, container_iid: u64)` | **OnContainerItemRemoved:** Unregisters `0x0000034E` observer, removes from `0x100006E8`, and fires `0x00020066` (`131174`), `0x00020069` (`131177`, equipment changed if `(slot & 0x003FFFFF) != 0`), or `0x00020075` (`131189`, container item removed). |
| `0x85130010` | C->S | `372` (`0xb70f4`) | `(arg1: u32, item_iid: u64)` | **Container Item Action.** |
| `0x85130015` | C->S | `372` (`0xb7088`) | `()` | **Close / Query MEO Inventory.** |
| `0x85130016` | S->C | `1299` (`0xb5b74`) | `(rc: u32, err_did: u32)` | **OnItemActionError:** Displays error string for `rc` (`1..31`) or `err_did`. |

#### C. `CLootSystem` (Class `1298` / `0x0512`, Base Class `274` / `0x0112`, RPC Prefix `0x8512XXXX`)
Disassembled from `wlib.bin` (`class_1298.txt`, `class_274.txt`):
- **`0x85120001` (C->S):** `LootItemsFromContainer(loot_all: bool, item_iid_array: Array<u64>, container_iid: u64)` — loots specific item `InstanceID`s (or all items when `loot_all == true`) from an open world/corpse container `container_iid`.
- **`0x85120002` (C->S):** `NotifyContainerLootClosed(container_iid: u64)` — notifies the server that the player closed container `container_iid`.
- **`0x85120003` (C->S):** `RequestDestroyContainerItem(item_iid_array: Array<u64>, container_iid: u64)`.
- **`0x85120004` (S->C):** `OnCloseContainerLoot(container_iid: u64)` — closes container loot UI (`0x00020076` / `131190`).
- **`0x85120007`..`0x8512001D`:** Fellowship roll/pass/master-looter RPCs (`0x8512000B` C->S Roll/Pass, `0x8512000D` C->S MasterLooterAssign).

#### D. `CBankRepositorySystem` (Vault & Shared Storage: Class `2564` / `0x0A04` & Class `2562` / `0x0A02`)
Disassembled from `wlib.bin` (`class_2564.txt`, `class_2562.txt`, `class_2560.txt`):
- **`0x8A04000A` (S->C):** `OnOpenBankRepository(repo_type: u32, npc_or_bank_iid: u64, capacity: u32, ...)`
- **`0x8A04000B` (C->S):** `CloseBankRepository(npc_or_bank_iid: u64)`
- **`0x8A04000C` (C->S):** `DepositItemToBank(quantity: i32, chest_id: u32, item_iid: u64, bank_iid: u64)`
- **`0x8A04000D` (C->S):** `WithdrawItemFromBank(dest_slot: u32, quantity: i32, item_iid: u64, bank_iid: u64)`
- **`0x8A04000E` (S->C):** `OnBankCapacitiesUpdated(max_cap: u32, cur_cap: u32, bank_iid: u64)`
- **`0x8A040014` (S->C):** `OnBankItemAdded(item_entity_or_props, bank_iid: u64)`
- **`0x8A040015` (S->C):** `OnBankItemRemoved(item_iid: u64, bank_iid: u64)`
- **`0x8A040018` (S->C):** `OnBankItemUpdated(item_iid: u64, bank_iid: u64)`
- **`0x8A040019` (C->S):** `MoveBankItemToChest(chest_id: u32, item_iid: u64, bank_iid: u64)`
- **`0x8A04001F` (S->C):** `OnBankOperationResult(rc: u32, bank_iid: u64)`

#### E. `CBagOfSingletonsSystem` (Carry-All Bags: Class `4181` / `0x1055`, Base Class `4180` / `0x1054`, RPC Prefix `0x9055XXXX`)
Disassembled from `wlib.bin` (`class_4181.txt`, `class_4180.txt`):
- **`0x90550002` (C->S):** `SetCarryAllAutoGather(enabled: bool, carry_all_iid: u64)`
- **`0x90550003` (C->S):** `DepositItemIntoCarryAll(item_iid: u64, carry_all_iid: u64)`
- **`0x90550004` (C->S):** `WithdrawFromCarryAll(dest_slot: u32, quantity: i32, item_did: u32, carry_all_iid: u64)`
- **`0x90550005` (S->C):** `OnCarryAllOperationComplete(op_type: u32, success: bool)`
- **`0x90550006` (C->S):** `GatherAllFromBackpackIntoCarryAll(carry_all_iid: u64)`
- **`0x90550008` (C->S):** `DeleteEntryFromCarryAll(quantity: i32, item_did: u32, carry_all_iid: u64)`

---

### 1.6 Complete Item & Container Property ID Table (`0x34000000`)

Verified from `wlib.bin` (`Class 1201`, `1208`, `1290`, `1291`, `1298`, `1299`, `2075`, `2350`, `2564`, `2804`, `4181`) and `lotroclient64.exe` (`0x140583c50`, `0x140c47330`):

| Property ID (Hex) | Property ID (Dec) | Property Type | Engine Name / Role | Description |
|---|---|---|---|---|
| `0x00000001` | `1` | `DataID` (`0x14`) | `Entity_DataID` | Item template `DID` (`0x70XXXXXX`) |
| `0x0000034E` | `846` | `Bitfield32` (`0x12`) | `Inventory_Slot` | Container slot bitmask (`0x40000000 \| bag_slot` for backpack, `0x20000000 \| (1 << equip_bit)` for paperdoll equipment). Observed by `[container + 0x48 + 0x08]` (`0x1405186f0`). |
| `0x00000642` | `1602` | `Int32` (`0x02`) | `Attachment_SlotID` | Visual attachment slot ID set by `pkg 48 fn 2` (`0x140583570`) |
| `0x10000421` | `268436513` | `DataID` (`0x14`) | `Item_PaperdollVisual` | Visual appearance `DID` used by `0x140c47330` to render equipped items on the 3D avatar |
| `0x10000682` | `268437122` | `Int32` (`0x02`) | `Item_Level` | Item level |
| `0x100006E8` | `268437224` | `HashTable` (`0x09`) | `Inventory_ItemCache` | Client-side cached map of contained item `InstanceID`s on the container entity |
| `0x10000914` | `268437780` | `Int32` (`0x02`) | `Inventory_Quantity` | Current stack count of the item instance (default `1` if absent, `1201 vslot 11` `0xccd58`) |
| `0x10000915` | `268437781` | `Int32` (`0x02`) | `Inventory_MaxQuantity` | Container slot capacity (number of backpack slots on avatar; read by `1201 vslot 22` `0xccbf0`) |
| `0x10000916` | `268437782` | `Int32` (`0x02`) | `Inventory_Weight` | Item weight |
| `0x10000917` | `268437783` | `Int32` (`0x02`) | `Inventory_MaxWeight` | Container maximum weight capacity |
| `0x10000918` | `268437784` | `Int32` (`0x02`) | `Inventory_MaxStackSize` | Maximum stack size for this item (`1201 vslot 12` `0xccd18`) |
| `0x1000091A` | `268437786` | `InstanceID` (`0x07`) | `Inventory_BindToID` | `InstanceID` of the character to whom this item is bound (`0` if unbound) |
| `0x1000091B` | `268437787` | `Bool` (`0x05`) | `Inventory_BindOnAcquire` | `true` if item binds on acquire |
| `0x1000091C` | `268437788` | `Bool` (`0x05`) | `Inventory_BindOnEquip` | `true` if item binds on equip |
| `0x10000A35` | `268438069` | `Int32` (`0x02`) | `Item_MaxStructurePoints` | Maximum durability points (`1208 vslot 7` `0xbe6d4`) |
| `0x10000CC0` | `268438720` | `InstanceID` (`0x07`) | `Inventory_OpenContainerID` | `InstanceID` of currently open world/corpse loot container |
| `0x10000CF8` | `268438776` | `Enum`/`Int32` | `Item_ClothingColor` | Base clothing/dye color |
| `0x10000D86` | `268438918` | `Int32` (`0x02`) | `Item_StructurePoints` | Current durability points (`1208 vslot 6` `0xbe72c`) |
| `0x10000D87` | `268438919` | `Enum` (`0x15`) | `Item_Category` | Item category enum |
| `0x10000D88` | `268438920` | `Bitfield32` (`0x12`) | `Item_EquipmentSlot` | Allowed equipment paperdoll slot bitmask (`0x003FFFFF`) |
| `0x10000D89` | `268438921` | `Enum` (`0x15`) | `Item_Quality` | Item quality/rarity tier |
| `0x10000D8A` | `268438922` | `Int32` (`0x02`) | `Item_Value` | Base vendor value in copper |
| `0x10000D8B` | `268438923` | `Enum` (`0x15`) | `Item_WearState` | Discrete durability wear state (`CItemWearSystem` class `1291` / `1208 vslot 2`) |
| `0x10001053` | `268439635` | `Bool` (`0x05`) | `Inventory_BoundToAccount` | `true` if bound to account |
| `0x10001126` | `268439846` | `Bool` (`0x05`) | `Inventory_TempItem` | `true` if temporary item |
| `0x1000147B` | `268440699` | `Array` (`0x08`) | `Inv_NewItemsList` | Array of newly acquired item `InstanceID`s (`0x850A0033` / `0x850A0034`) |
| `0x10001516` | `268440854` | `Bitfield32` (`0x12`) | `Item_UnavailableEquipmentSlots` | Equipment slot bitmask blocked while this item is equipped (e.g. `0x00010000` off-hand blocked by a two-handed weapon; checked in `0x140c47442` and `1202 vslot 36`) |
| `0x10001718` | `268441368` | `Array` (`0x08`) | `Usage_ItemsInUse` | Array of item `InstanceID`s currently being used/channeled (`CUsageSystem` class `1333`) |
| `0x10001D35` | `268442933` | `Int32` (`0x02`) | `Inventory_MaxSizeBonus` | Bonus inventory capacity slots |
| `0x10001D47` | `268442951` | `Int32` (`0x02`) | `Item_DyeColor` | Applied dye color |
| `0x10001E56` | `268443222` | `Bool` (`0x05`) | `Inventory_Locked` | `true` if item is locked against sale/destruction |
| `0x10002E4F` | `268447311` | `HashTable` (`0x09`) | `Inventory_BagSlotMap` | Client bag-to-slot layout mapping (`0x850A0038`) |
| `0x10003684` | `268449412` | `Array` (`0x08`) | `Loot_PendingLootList` | Array of `InstanceID`s in Pending Loot overflow |
| `0x10003685` | `268449413` | `HashTable` (`0x09`) | `Loot_PendingLootTimers` | Map of `item_iid -> expiration_timestamp` for Pending Loot |
| `0x10003F0E` | `268451598` | `Int32` (`0x02`) | `Inventory_AccountMaxSizeBonus` | Account-wide inventory slot bonus |
| `0x100044EE` | `268453102` | `Int32` (`0x02`) | `Inventory_CharacterMaxSizeBonus` | Character-specific inventory slot bonus |

---

## 2. Client -> Server Inventory & Equipment Commands (`InventoryCommand`)

All client-to-server inventory and equipment commands are sent as **Top-Level `NetBlob` Event `0x0115` (`277`)** carrying an entity-targeted **`WLibMessage` (`0x8XXXXXXX`)** addressed to the player's avatar entity (`InstanceID` at `NetBlob` event offset `+0x28`), except direct world-entity interaction which uses **Top-Level `NetBlob` Event `0x0075` (`117`)**.

### 2.1 `WLibArgFrame` Wire Encoding Refresher
Every WLib RPC payload starts with `u32 LE function_id` (`0x8XXXXXXX`), followed by the serialized `WLibArgFrame` (`0x140b8e6e0`):
- **Push Order vs. Serialization Order:** The WLib bytecode VM (and C++ callers in `0x140b4f85f..0x140b66a2e`) pushes arguments onto the VM stack before pushing `self` (the target entity WState reference) and invoking `CALLMETHOD` (`0x44`). `WLibArgFrame` serializes the pushed arguments in the exact order they sit in the call frame:
  - `i32` / `u32` / `bool`: `u8 type_tag = 0x02` (`1` byte) + `u32 LE value` (`4` bytes) = `5` bytes.
  - `u64` (`InstanceID`, pushed as two 32-bit stack slots `[low_u32, high_u32]` with high-slot tag `0x82` and low-slot tag `0x02`): serialized as an 8-byte `InstanceID` (`u8 tag = 0x82` + `u32 LE high` + `u8 tag = 0x02` + `u32 LE low`, or via `WLibArgFrame` pair encoding).

### 2.2 Exact Command Mapping (`InventoryCommand`)

#### 1. `InventoryCommand::MoveItem { item, to }`
- **C++ Wrapper:** `0x140b4f9bf` / `0x140b6041f` -> calls `CMEOInventoryControl` (`Class 2350`) `fnum 6` (`0x092E0006` at `0xcd058`) -> calls `CInventoryControl` (`Class 2804`) `fnum 5` (`0x0AF40005` at `0xcc834`).
- **Preconditions Checked by Client (`2804 fnum 5`):**
  1. `1201 vslot 8` (`0xcce00`): `item` is not currently in `0x10001718` (`Usage_ItemsInUse`).
  2. `dest_slot != 0` and `(dest_slot & 0x40000000) != 0` (must have backpack/container slot bit `0x40000000` set).
- **WLib RPC Function ID:** **`0x850A000B`** (`Class 368` `fnum 11`, `0xcaa7c`).
- **`WLibArgFrame` Parameters (in frame order):**
  1. `quantity: i32` = `0` (move entire stack; or full stack quantity from `PID 0x10000914`)
  2. `dest_slot: u32` = `0x40000000 | (to_slot_index & 0x0FFFFFFF)`
  3. `dest_container_iid: u64` = `avatar_iid` (player's own `InstanceID` for backpack)
  4. `item_iid: u64` = `item` (`InstanceID` of the item being moved)
- **Server Response:**
  - If `dest_slot` is empty: Server sends a Property Update (`0x88` / `0xB7`) on `item_iid` setting `PID 0x0000034E` = `dest_slot`.
  - If `dest_slot` contains another item of the same stackable template `DID`: Server updates `PID 0x10000914` (`Inventory_Quantity`) on the target stack and either updates `PID 0x10000914` or destroys (`0x9A` + `0x850A0024`) the source stack.
  - If `dest_slot` contains a different item: Server sends two Property Updates (`0x88` / `0xB7`) swapping `PID 0x0000034E` between the two item entities.
  - If rejected: Server sends S->C WLib RPC `0x8513000C(item_iid, err_code)` or `0x850A0021(err_code, item_iid)`.

#### 2. `InventoryCommand::SplitStack { item, quantity, to }`
- **C++ Wrapper & WLib Path:** Identical to `MoveItem`: `2350 fnum 6` (`0xcd058`) -> `2804 fnum 5` (`0xcc834`).
- **WLib RPC Function ID:** **`0x850A000B`** (`Class 368` `fnum 11`, `0xcaa7c`).
- **`WLibArgFrame` Parameters (in frame order):**
  1. `quantity: i32` = `quantity` (`1 <= quantity < current_stack_size`)
  2. `dest_slot: u32` = `0x40000000 | (to_slot_index & 0x0FFFFFFF)`
  3. `dest_container_iid: u64` = `avatar_iid`
  4. `item_iid: u64` = `item` (`InstanceID` of the source stack)
- **Server Response:**
  - Server decrements `PID 0x10000914` (`Inventory_Quantity`) on `item_iid` via Property Update (`0x88` / `0xB7`), spawns a new item entity (`0x98`/`0x52`) with `PID 0x10000914 = quantity` and `PID 0x0000034E = dest_slot`, and links it to `avatar_iid` via `NetBlob` Event `0xCE` (`containment_type = 0x1000`) + S->C RPC `0x8513000D`.

#### 3. `InventoryCommand::EquipItem { item, slot }`
- **C++ Wrapper:** `0x140b4f85f` / `0x140b602af` -> calls `CMEOInventoryControl` (`Class 2350`) `fnum 10` (`0x092E000A` at `0xcd3a4`) -> calls `CInventoryControl` (`Class 2804`) `fnum 6` (`0x0AF40006` at `0xcc784`).
- **Preconditions Checked by Client (`2350 fnum 10` & `2804 fnum 6`):**
  1. `1201 vslot 8` (`0xcce00`): `item` is not currently in `0x10001718` (`Usage_ItemsInUse`).
  2. `1202 vslot 36` (`0xbc07c`): Target equipment slot is allowed by `PID 0x10000D88` (`Item_EquipmentSlot`) and not blocked by `PID 0x10001516` (`Item_UnavailableEquipmentSlots`) on currently equipped items.
- **WLib RPC Function ID:** **`0x850A001D`** (`Class 368` `fnum 29`, `0xcaa04`).
- **`WLibArgFrame` Parameters (in frame order):**
  1. `target_slot: u32` = `0x20000000 | (1 << (equip_slot_index - 1))` (or `0` to let the server pick the default valid slot from `PID 0x10000D88`)
  2. `flags: u32` = `0` (`0` = normal equip from inventory)
  3. `item_iid: u64` = `item` (`InstanceID` of the item to equip)
- **Server Response:**
  - If an item is already equipped in `target_slot`, the server demotes the old item (`PID 0x0000034E` -> `0x40000000 | old_bag_slot`, `Event 0xCE` `containment_type = 0x1000`).
  - Server updates `PID 0x0000034E` on `item_iid` to `0x20000000 | (1 << (equip_slot_index - 1))` via Property Update (`0x88` / `0xB7`) and sends `NetBlob` Event `0xCE` (`parent_container_iid = avatar_iid`, `containment_type = 0x0100`).

#### 4. `InventoryCommand::UnequipItem { item, to }`
- **WLib Path:** `CMEOInventoryControl` (`Class 2350`) `fnum 4` (`0x092E0004` at `0xccfc0`) -> `CInventoryControl` (`Class 2804`) `fnum 12` (`0x0AF4000C` at `0xcc644`).
- **Preconditions Checked by Client:**
  1. `1201 vslot 8`: `item` is not in `0x10001718` (`Usage_ItemsInUse`).
  2. `pkg 48 fn 6` (`GetEquippedItemIIDs`): `item` is currently equipped (`containment_type == 0x0100`).
- **WLib RPC Function ID:** **`0x850A0001`** (`Class 368` `fnum 1`, `0xca9c8`).
- **`WLibArgFrame` Parameters (in frame order):**
  1. `dest_slot: u32` = `0x40000000 | (to_slot_index & 0x0FFFFFFF)` (or `0` to auto-place in the first free backpack slot)
  2. `dest_container_iid: u64` = `avatar_iid`
  3. `item_iid: u64` = `item` (`InstanceID` of the equipped item)
- **Server Response:**
  - Server updates `PID 0x0000034E` on `item_iid` to `0x40000000 | free_bag_slot` via Property Update (`0x88` / `0xB7`) and sends `NetBlob` Event `0xCE` (`parent_container_iid = avatar_iid`, `containment_type = 0x1000`).

#### 5. `InventoryCommand::DestroyItem { item }`
- **C++ Wrapper:** `0x140b66a2e` -> calls `CInventoryControl` (`Class 2804`) `fnum 4` (`0x0AF40004` at `0xcc708`).
- **Preconditions Checked by Client (`2804 fnum 4`):**
  1. `1201 vslot 8`: `item` is not in `0x10001718` (`Usage_ItemsInUse`).
  2. `PID 0x10001E56` (`Inventory_Locked`) is not `true`.
- **WLib RPC Function ID:** **`0x850A0019`** (`Class 368` `fnum 25`, `0xca918`).
- **`WLibArgFrame` Parameters (in frame order):**
  1. `quantity_or_flags: i32` = `0` (destroy entire item/stack; or specific quantity to destroy)
  2. `item_iid: u64` = `item` (`InstanceID` of the item to destroy)
- **Server Response:**
  - Server sends `NetBlob` Event `0x9A` (`DestroyEntity`) for `item_iid` and S->C WLib RPC `0x850A0024(item_iid)` + `0x8513000E(item_iid, old_slot, avatar_iid)`.

#### 6. `InventoryCommand::UseItem { item, target }` — All 3 Client Execution Paths
Disassembled from `CUsageSystem` (`Class 1333` / `0x0535`, `Class 295` / `0x0127`, `Class 1545` / `0x0609`, `Class 1598` / `0x063E`) and `CMEOInventoryControl` (`Class 2350` `fnum 9` / `10` at `0xcd35c` / `0xcd3a4`):

When the player double-clicks or activates an item in inventory or on a quickslot bar, the client inspects the item's properties and dispatches one of three wire commands:

| Path | Condition on Item Entity | Wire Command Sent by Client | Parameters |
|---|---|---|---|
| **Path A: Skill-Executing Item** (potions, food, scrolls, clicky gear, mounts) | Item has `PID 0x10000426` (`Usage_SkillToExecute`, `DataID` `0x70XXXXXX`) | **WLib RPC `0x85280006`** (`CSkillSystem`, via `NetBlob` `0x0115`) | Same skill execution RPC documented in P0 (`skill_did = PID 0x10000426`, `item_iid = item`, `target_iid = target`). |
| **Path B: Generic Usable Inventory Item** (containers, quest starters, reputation items, recipes, tomes without `0x10000426`) | Item is in inventory/equipped (`HaveSameRootParentContainer == true`) and has no `PID 0x10000426` (or is used via `CUsageSystem` `Class 1598` `fnum 1` `0xb2738` -> `0x140b4ec64`) | **WLib RPC `0x8535001D`** (`CUsageSystem` `Class 295` `fnum 29`, `0xb01a4`, via `NetBlob` `0x0115`) or **`0x850A001D`** (`target_slot = 0, flags = 0, item_iid`) | For **`0x8535001D`**: `(action_flags: i32, item_iid: u64)` where `action_flags = 0` (or usage verb). Server responds with **`0x8535001E`** (`OnUseItemResult(err_code: u32, action_flags: i32, item_iid: u64)` at `1333` `0xb0754`) and adds `item_iid` to `0x10001718` (`Usage_ItemsInUse`). |
| **Path C: World Entity / Ground Item Interaction** (corpses, chests, doors, crafting facilities, resource nodes, ground items) | Target entity is in the world (not contained in avatar's inventory) | **Top-Level `NetBlob` Event `0x0075` (`117`, `ClientActionCommand`)** | Raw `NetBlob` `0x0075` payload (`target_iid: u64`, `action_id: u32`, `position`, `flags`). For a lootable corpse/chest, the server responds with S->C WLib RPC **`0x85130001`** (`OnOpenContainerLoot(container_iid, flag)`). |

---

## 3. In-World Character Property & Level Updates (`CharacterEvent`)

### 3.1 Live Property Update Events (`CharacterEvent::PropertyChanged`)

As established in the entity property baseline (`0x141511120`..`0x1415117b0`) and confirmed across `CAdvancementSystem` (`Class 1247`), `CInventorySystem` (`Class 1290`), and `CMEOInventorySystem` (`Class 1299`), the server updates entity properties (on the player avatar, other characters/NPCs, and contained item entities) using twelve top-level `NetBlob` events:

| Event ID (Hex) | Event ID (Dec) | Vtable VA | Handler / Serializer VA | Wire Role & Payload Summary |
|---|---|---|---|---|
| `0x0052` | `82` | `0x141510e10` | `0x140759bf0` / `0x140548da0` | **Full `PropertyCollection` Snapshot / Bulk Update:** Carries target `entity_iid: u64`, sequence/timestamp header, and a full `PropertyCollection` sub-stream (`0x14116df40`). |
| `0x0088` | `136` | `0x141511120` | `0x14076bc80` | **Single `Int32` (`Type 2`) Property Set:** `[entity_iid: u64, pid: u32, val: i32, ts_opt]`. Primary live update for `Advancement_Level` (`0x10001011`), `Inventory_Quantity` (`0x10000914`), `Inventory_MaxQuantity` (`0x10000915`), `Item_StructurePoints` (`0x10000D86`), `Item_MaxStructurePoints` (`0x10000A35`), vitals, and integer stats. |
| `0x0092` | `146` | `0x1415111a0` | `0x14076bda0` | **Single `Int32` (`Type 2`) Property Delta:** `[entity_iid: u64, pid: u32, delta: i32, ts_opt]`. Adds `delta` to current `Int32` property value (used for stack quantity deltas, currency/XP deltas, durability wear deltas). |
| `0x00FE` | `254` | `0x141511220` | `0x14076bec0` | **Single `Float` (`Type 3`) Property Set:** `[entity_iid: u64, pid: u32, val: f32, ts_opt]`. |
| `0x00B0` | `176` | `0x1415112a0` | `0x14076bff0` | **Single `Float` (`Type 3`) Property Delta:** `[entity_iid: u64, pid: u32, delta: f32, ts_opt]`. |
| `0x0149` | `329` | `0x1415113a0` | `0x14076c230` | **Single `Int64` (`Type 4`) Property Set:** `[entity_iid: u64, pid: u32, val: i64, ts_opt]` (used for 64-bit XP totals and timestamps). |
| `0x000E` | `14` | `0x141511420` | `0x14076c350` | **Single `Bool` (`Type 5`) Property Set:** `[entity_iid: u64, pid: u32, val: u8, ts_opt]` (used for `Inventory_BoundToAccount` `0x10001053`, `Inventory_Locked` `0x10001E56`, etc.). |
| `0x00E7` | `231` | `0x141511520` | `0x14076c5b0` | **Single `InstanceID` (`Type 7`) Property Set:** `[entity_iid: u64, pid: u32, val: u64, ts_opt]` (used for `Inventory_BindToID` `0x1000091A`, `Inventory_OpenContainerID` `0x10000CC0`, target selection). |
| `0x0076` | `118` | `0x141511620` | `0x14076c810` | **Single `DataID` (`Type 0x14`) Property Set:** `[entity_iid: u64, pid: u32, val: u32, ts_opt]` (used for `Item_PaperdollVisual` `0x10000421`). |
| `0x014A` | `330` | `0x1415116a0` | `0x14076c930` | **Single `Position` (`Type 0x10`) Property Set.** |
| `0x008E` | `142` | `0x141511720` | `0x14076ca60` | **Remove Property:** `[entity_iid: u64, pid: u32, ts_opt]`. Removes `pid` from the entity's `PropertyCollection`. |
| `0x00B7` | `183` | `0x1415117b0` | `0x14076cb50` | **Generic Single Property Update (Any Type):** `[entity_iid: u64, pid: u32, property_ descriptor + value]`. Used for `Bitfield32` (`Type 0x12`, e.g. `PID 0x0000034E` `Inventory_Slot`, `PID 0x10000D88`, `PID 0x10001516`), `Enum` (`Type 0x15`, e.g. `PID 0x10000D8B` `Item_WearState`), `Array` (`Type 0x08`, e.g. `PID 0x10003684` `Loot_PendingLootList`), and `HashTable` (`Type 0x09`). |

---

### 3.2 Character Level Updates & Advancement (`CharacterEvent::LevelChanged`)

Disassembled from `CAdvancementSystem` (`Class 1247` / `0x04DF`, Base Class `389` / `0x0185`, RPC prefix `0x84DFXXXX` in `class_1247.txt` and `class_389.txt`):

#### Key Advancement & Level Property IDs (`0x34000000`)
| Property ID (Hex) | Property ID (Dec) | Type | Engine Name / Role | Description |
|---|---|---|---|---|
| `0x10001011` | `268439569` | `Int32` (`0x02`) | `Advancement_Level` | Character's current effective level (`Class 1247` `fnum 21` `0xd619c`, `fnum 22` `0xd60f8`) |
| `0x1000493B` | `268454203` | `Int32` (`0x02`) | `Advancement_RealLevel` | Character's unscaled/real level (when level-scaled in instances/missions; falls back to `0x10001011` if `0`, `Class 1247` `fnum 24` `0xd6080`) |
| `0x10000019` | `268435481` | `Int64` / `Int32` | `Advancement_XP` | Current experience points |
| `0x1000072E` | `268437294` | `Int32` (`0x02`) | `Wallet_Money` / `Currency_Copper` | Character coin balance in copper (100 copper = 1 silver, 100,000 copper = 1 gold) |
| `0x100019E4` | `268442084` | `HashTable` (`0x09`) | `Advancement_SkillTrainPriceTable` | Map of `skill_did -> price` at trainer (`0x84DF0009`) |

#### Quiet Level Update vs. Level-Up Presentation (`0x84DF0016`)
The client distinguishes between a **quiet level property update** (initial login, inspecting another entity, or server-side level scaling) and an **in-world level-up event** (`CharacterEvent::LevelChanged { new_level, quiet: false }`) via `CAdvancementSystem` (`Class 1247` `fnum 22`, `0x04DF0016` at `0xd60f8`):

1. **Property Update (`NetBlob` Event `0x88` or `0x52`):**
   - Updates **`PID 0x10001011` (`Advancement_Level`, `Int32`)** (and optionally **`PID 0x1000493B` (`Advancement_RealLevel`, `Int32`)**) on the character entity.
   - If no `0x84DF0016` RPC accompanies the property update, the UI updates the displayed level number quietly without playing the level-up fanfare or chat announcement (`quiet = true`).
2. **S->C WLib RPC `0x84DF0016` (`OnPlayerLevelUp` on `CAdvancementSystem` class `1247`, `fnum 22` at `0xd60f8`):**
   - **Wire Payload:** `function_id = 0x84DF0016`, `WLibArgFrame = ()` (0 arguments).
   - **Client Handler (`0xd60f8`):**
     1. Reads `new_level = entity.GetProperty(0x10001011)` (`Advancement_Level`).
     2. Formats and displays the localized chat announcement `"You have attained level {new_level}!"` (`string_did = 0x25008B2C`, chat channel `26`) via `pkg 22 fn 3`.
     3. Invokes `1247 fnum 31` (`0xd5ee0`), which plays the level-up visual/audio effect and triggers level-gated tutorial/UI notifications (`0x0002016F` / `131439`).

#### Complete `CAdvancementSystem` (`Class 1247` / `389`, `0x84DFXXXX`) RPC Table
| Function ID | Dir | Handler Offset | `WLibArgFrame` Parameters | Purpose |
|---|---|---|---|---|
| `0x84DF0004` | C->S | `389` (`0xd6748`) | `(skill_did: u32)` | Request to purchase/train `skill_did` |
| `0x84DF0005` | C->S | `389` (`0xd6784`) | `(skill_did: u32)` | Request to untrain `skill_did` |
| `0x84DF0009` | S->C | `1247` (`0xd65cc`) | `(price_table: HashTable)` | Stores `0x100019E4` (`Advancement_SkillTrainPriceTable`) |
| `0x84DF000A` | C->S | `389` (`0xd66dc`) | `()` | Query skill training prices |
| `0x84DF000C` | S->C | `1247` (`0xd6360`) | `(trainer_iid: u64, arg2: u32)` | Opens class trainer UI (`0x00020023` / `131107`) |
| `0x84DF000E` | S->C | `1247` (`0xd643c`) | `(skill_did: u32, err_code: u32)` | Skill training result/error notification |
| `0x84DF000F` | C->S | `389` (`0xd66a0`) | `(skill_did: u32, trainer_iid: u64)` | Buy skill from trainer NPC `trainer_iid` |
| `0x84DF0016` | S->C | `1247` (`0xd60f8`) | `()` | **OnPlayerLevelUp:** Reads `PID 0x10001011`, prints level-up chat message (`0x25008B2C`), triggers level-up presentation (`quiet = false`). |

---

## 4. Clean-Room Specification Section (Ready for `docs/specs/official-messages-gameplay.md`)

> The following section contains no Ghidra addresses, internal symbol names, or decompiler output, uses `BIN` and `DAT` provenance tags, and includes synthetic test vectors suitable for direct inclusion in `docs/specs/official-messages-gameplay.md`.

---

### P1. Inventory, Containers, Equipment, and Character Property/Level Updates

#### P1.1 Entity Containment Model and Slot Bitmask Encoding (`BIN`, `DAT`)

In the official protocol, every item instance in a character's Backpack, Equipped Paperdoll, or open World Container is represented as a full entity (`64-bit InstanceID` and `32-bit` template `DataID` in Property `0x00000001`) attached to a parent container entity via the entity containment component.

##### Containment Types (`u32`)
| Value (Hex) | Value (Dec) | Meaning | Provenance |
|---|---|---|---|
| `0x00000010` | `16` | Attached / wielded visual child entity | `BIN` |
| `0x00000100` | `256` | Equipped paperdoll item | `BIN` |
| `0x00001000` | `4096` | Contained inventory / bag item | `BIN` |

##### Container Slot Property (`PID 0x0000034E`, `Bitfield32`) (`BIN`, `DAT`)
Each contained item entity specifies its position within its parent container via Property `0x0000034E` (`846`, type `0x12` `Bitfield32`). The parent container observes Property `0x0000034E` on all contained child entities and automatically re-indexes a child entity's slot whenever Property `0x0000034E` changes.

| High Nibble (`bits 28..31`) | Low Bits (`bits 0..27`) | Meaning | Provenance |
|---|---|---|---|
| `0x40000000` (`bit 30`) | `slot_index` (`0 .. capacity - 1`) | **Backpack / Container Slot:** `0x40000000 \| slot_index` (with `containment_type = 0x1000`). | `BIN` |
| `0x20000000` (`bit 29`) | `1 << (equip_index - 1)` (`bits 0..21`) | **Equipped Paperdoll Slot:** `0x20000000 \| equip_bit` (with `containment_type = 0x0100`). | `BIN` |
| `0x80000000` (`bit 31`) | `index` | **Overflow / Pending Slot.** | `BIN` |
| `0x10000000` (`bit 28`) | `sub_slot` | **Auxiliary Container Slot.** | `BIN` |

##### Paperdoll Equipment Slot Bitflags (`bits 0..21` of `PID 0x0000034E`, `PID 0x10000D88`, and `PID 0x10001516`) (`BIN`, `DAT`)
| Slot Index | Bit | Bitmask (`u32`) | Equipped `PID 0x0000034E` Value | Paperdoll Slot | Provenance |
|---|---|---|---|---|---|
| `1` | `0` | `0x00000001` | `0x20000001` | Head | `BIN` |
| `2` | `1` | `0x00000002` | `0x20000002` | Chest | `BIN` |
| `3` | `2` | `0x00000004` | `0x20000004` | Legs | `BIN` |
| `4` | `3` | `0x00000008` | `0x20000008` | Hands | `BIN` |
| `5` | `4` | `0x00000010` | `0x20000010` | Feet | `BIN` |
| `6` | `5` | `0x00000020` | `0x20000020` | Shoulders | `BIN` |
| `7` | `6` | `0x00000040` | `0x20000040` | Back (Cloak) | `BIN` |
| `8` | `7` | `0x00000080` | `0x20000080` | Left Wrist (Bracelet 1) | `BIN` |
| `9` | `8` | `0x00000100` | `0x20000100` | Right Wrist (Bracelet 2) | `BIN` |
| `10` | `9` | `0x00000200` | `0x20000200` | Neck (Necklace) | `BIN` |
| `11` | `10` | `0x00000400` | `0x20000400` | Left Finger (Ring 1) | `BIN` |
| `12` | `11` | `0x00000800` | `0x20000800` | Right Finger (Ring 2) | `BIN` |
| `13` | `12` | `0x00001000` | `0x20001000` | Left Ear (Earring 1) | `BIN` |
| `14` | `13` | `0x00002000` | `0x20002000` | Right Ear (Earring 2) | `BIN` |
| `15` | `14` | `0x00004000` | `0x20004000` | Pocket | `BIN` |
| `16` | `15` | `0x00008000` | `0x20008000` | Primary Weapon (Main-hand) | `BIN` |
| `17` | `16` | `0x00010000` | `0x20010000` | Secondary Weapon / Shield (Off-hand) | `BIN` |
| `18` | `17` | `0x00020000` | `0x20020000` | Ranged Weapon | `BIN` |
| `19` | `18` | `0x00040000` | `0x20040000` | Craft Tool | `BIN` |
| `20` | `19` | `0x00080000` | `0x20080000` | Class Slot | `BIN` |
| `21` | `20` | `0x00100000` | `0x20100000` | Bridle / Mount Slot | `BIN` |
| `22` | `21` | `0x00200000` | `0x20200000` | Aura / Extra Weapon Slot | `BIN` |

---

#### P1.2 Initial Container & Item Delivery in Entity Creation (`BIN`)

During entity creation (`NetBlob` Event `0x0098` / `0x0052`), an entity's containment component is serialized immediately following the base entity header using the following byte layout:

| Order | Field | Size (Bytes) | Type | Byte Order | Description | Provenance |
|---|---|---|---|---|---|---|
| 1 | `parent_container_iid` | `8` | `u64` | Little-endian | `InstanceID` of parent container (`0` if uncontained). | `BIN` |
| 2 | `attachment_count` | `4` | `u32` | Little-endian | Number of active visual attachments. | `BIN` |
| 3 | `bucket_size_index` | `1` | `u8` | — | Prime bucket table index (`< 23`, corresponding bucket count `<= 50000`). | `BIN` |
| 4 | `contained_count` | `1..5` | `VLE u32` | Little-endian | 3-tier variable-length integer count of contained child entities. | `BIN` |
| 5 | `links[contained_count]` | `13` or `19` each | `ContainmentLink` | Little-endian | Per-child containment link record (see below). | `BIN` |
| 6 | `children[contained_count]` | variable | `EntityArchive[]` | Little-endian | `contained_count` serialized child entity archives in link order. | `BIN` |

Each `ContainmentLink` entry in `links[contained_count]` has the layout:

| Offset | Field | Size (Bytes) | Type | Condition | Description | Provenance |
|---|---|---|---|---|---|---|
| `+0x00` | `child_iid` | `8` | `u64 LE` | Always | Contained child entity `InstanceID`. | `BIN` |
| `+0x08` | `containment_type` | `4` | `u32 LE` | Always | `0x0010` (attached), `0x0100` (equipped), or `0x1000` (inventory). | `BIN` |
| `+0x0C` | `has_attachment` | `1` | `u8` (`bool`) | Always | `0` = no attachment struct, `1` = 6-byte attachment struct follows. | `BIN` |
| `+0x0D` | `attach_id` | `4` | `u32 LE` | `has_attachment == 1` | Bone or attachment point ID. | `BIN` |
| `+0x11` | `attach_sub` | `1` | `u8` | `has_attachment == 1` | Sub-slot index. | `BIN` |
| `+0x12` | `attach_flag` | `1` | `u8` (`bool`) | `has_attachment == 1` | Boolean flag (`0` or `1`; values `>= 2` are invalid). | `BIN` |

---

#### P1.3 Runtime Containment & Placement Update (`NetBlob` Event `0x00CE`) (`BIN`)

When an item entity is added to a container, moved between containers, promoted/demoted between inventory (`0x1000`) and equipped (`0x0100`), or dropped into the world, the server sends top-level **`NetBlob` Event `0x00CE` (`206`)**:

| Order | Field | Size (Bytes) | Type | Condition | Description | Provenance |
|---|---|---|---|---|---|---|
| 1 | `event_id` | `2` | `u16 LE` | Always | `0x00CE` (`206`). | `BIN` |
| 2 | `flags1` | `1` | `u8` | Always | Optional field bitmask (`0x01` = `has_aux_vle`, `0x02` = `has_tail_a8`, `0x04` = `has_tail_ac`, `0x08` = `has_seq`, `0x10` = `has_vel_x`, `0x20` = `has_vel_y`, `0x40` = `has_vel_z`). | `BIN` |
| 3 | `entity_iid` | `8` | `u64 LE` | Always | `InstanceID` of the entity being placed or re-parented. | `BIN` |
| 4 | `mode_flags` | `1` | `u8` | Always | Bit `0x04`: `is_contained` (`1` = inside container, `0` = in world). Bit `0x08`: `has_attachment` (when `is_contained == 1`). Bit `0x10`: `pos_without_orientation` (when `is_contained == 0`). | `BIN` |
| 5 | `sequence_num` | `1` | `u8` | `flags1 & 0x08` | Sequence counter (`u8`, wrap-around window `127`). | `BIN` |
| 6 | `vel_x` | `4` | `f32 LE` | `flags1 & 0x10` | Velocity X (default `0.0`). | `BIN` |
| 7 | `vel_y` | `4` | `f32 LE` | `flags1 & 0x20` | Velocity Y (default `0.0`). | `BIN` |
| 8 | `vel_z` | `4` | `f32 LE` | `flags1 & 0x40` | Velocity Z (default `0.0`). | `BIN` |
| 9 | `aux_vle` | `1..5` | `VLE u32` | `flags1 & 0x01` | Auxiliary 3-tier VLE (default `0`). | `BIN` |
| 10a | `parent_container_iid` | `8` | `u64 LE` | `(mode_flags & 0x04) != 0` | `InstanceID` of the parent container entity. | `BIN` |
| 10b | `containment_type` | `1..5` | `VLE u32` | `(mode_flags & 0x04) != 0` | 3-tier VLE (`0x1000` inventory, `0x0100` equipped, `0x0010` attached). | `BIN` |
| 10c | `attachment_info` | `6` | `struct` | `(mode_flags & 0x0C) == 0x0C` | `u32 LE attach_id` (`4B`) + `u8 attach_sub` (`1B`) + `u8 attach_flag` (`1B`). | `BIN` |
| 11 | `world_transform` | variable | `Position [+ Quat]` | `(mode_flags & 0x04) == 0` | World position (plus quaternion orientation if `(mode_flags & 0x10) == 0`). | `BIN` |
| 12 | `tail_a8` | `1` | `u8` | `flags1 & 0x02` | Trailing mode byte A (default `5`). | `BIN` |
| 13 | `tail_ac` | `1` | `u8` | `flags1 & 0x04` | Trailing mode byte B (default `2`). | `BIN` |

---

#### P1.4 Item & Container Property IDs (`BIN`, `DAT`)

| Property ID (Hex) | Property ID (Dec) | Wire Type | Semantic Role | Provenance |
|---|---|---|---|---|
| `0x00000001` | `1` | `DataID` (`0x14`) | Item template `DID` (`0x70XXXXXX`). | `BIN`, `DAT` |
| `0x0000034E` | `846` | `Bitfield32` (`0x12`) | Container slot bitmask (`0x40000000 \| bag_slot` or `0x20000000 \| equip_bit`). | `BIN`, `DAT` |
| `0x10000421` | `268436513` | `DataID` (`0x14`) | Paperdoll visual appearance `DID`. | `BIN`, `DAT` |
| `0x10000426` | `268436518` | `DataID` (`0x14`) | Skill `DID` executed when the item is used. | `BIN`, `DAT` |
| `0x10000682` | `268437122` | `Int32` (`0x02`) | Item level. | `BIN`, `DAT` |
| `0x10000914` | `268437780` | `Int32` (`0x02`) | Stack quantity (default `1` if omitted). | `BIN`, `DAT` |
| `0x10000915` | `268437781` | `Int32` (`0x02`) | Container slot capacity (total backpack slots on avatar). | `BIN`, `DAT` |
| `0x10000918` | `268437784` | `Int32` (`0x02`) | Maximum stack size. | `BIN`, `DAT` |
| `0x1000091A` | `268437786` | `InstanceID` (`0x07`) | Bound character `InstanceID` (`0` if unbound). | `BIN`, `DAT` |
| `0x1000091B` | `268437787` | `Bool` (`0x05`) | Bind-on-acquire flag. | `BIN`, `DAT` |
| `0x1000091C` | `268437788` | `Bool` (`0x05`) | Bind-on-equip flag. | `BIN`, `DAT` |
| `0x10000A35` | `268438069` | `Int32` (`0x02`) | Maximum durability (structure points). | `BIN`, `DAT` |
| `0x10000CC0` | `268438720` | `InstanceID` (`0x07`) | Currently open world loot container `InstanceID`. | `BIN`, `DAT` |
| `0x10000CF8` | `268438776` | `Int32`/`Enum` | Base clothing color. | `BIN`, `DAT` |
| `0x10000D86` | `268438918` | `Int32` (`0x02`) | Current durability (structure points). | `BIN`, `DAT` |
| `0x10000D88` | `268438920` | `Bitfield32` (`0x12`) | Allowed equipment paperdoll slots mask (`0x003FFFFF`). | `BIN`, `DAT` |
| `0x10000D89` | `268438921` | `Enum` (`0x15`) | Item quality / rarity tier. | `BIN`, `DAT` |
| `0x10000D8A` | `268438922` | `Int32` (`0x02`) | Vendor value in copper coins. | `BIN`, `DAT` |
| `0x10000D8B` | `268438923` | `Enum` (`0x15`) | Discrete durability wear state. | `BIN`, `DAT` |
| `0x10001053` | `268439635` | `Bool` (`0x05`) | Bound-to-account flag. | `BIN`, `DAT` |
| `0x1000147B` | `268440699` | `Array` (`0x08`) | Newly acquired item `InstanceID` list. | `BIN`, `DAT` |
| `0x10001516` | `268440854` | `Bitfield32` (`0x12`) | Equipment slots blocked while this item is equipped. | `BIN`, `DAT` |
| `0x10001718` | `268441368` | `Array` (`0x08`) | Item `InstanceID`s currently in use. | `BIN`, `DAT` |
| `0x10001D47` | `268442951` | `Int32` (`0x02`) | Applied dye color. | `BIN`, `DAT` |
| `0x10001E56` | `268443222` | `Bool` (`0x05`) | Item locked flag (prevents destruction/sale). | `BIN`, `DAT` |
| `0x10003684` | `268449412` | `Array` (`0x08`) | Pending Loot overflow item `InstanceID` list. | `BIN`, `DAT` |

---

#### P1.5 Client <-> Server Inventory, Equipment, Loot & Storage WLib RPCs (`NetBlob` `0x0115`) (`BIN`, `DAT`)

| Function ID | Direction | `WLibArgFrame` Parameters (Wire Order) | Semantic Action | Provenance |
|---|---|---|---|---|
| `0x850A0001` | C -> S | `(dest_slot: u32, dest_container_iid: u64, item_iid: u64)` | **UnequipItem:** Unequips `item_iid` into `dest_container_iid` (`avatar_iid`) at `dest_slot` (`0` = first free slot, or `0x40000000 \| slot_index`). | `BIN`, `DAT` |
| `0x850A000B` | C -> S | `(quantity: i32, dest_slot: u32, dest_container_iid: u64, item_iid: u64)` | **MoveItem / SplitStack:** Moves `item_iid` to `dest_slot` (`0x40000000 \| slot_index`) in `dest_container_iid`. `quantity == 0` moves the full stack; `0 < quantity < stack_size` splits the stack. | `BIN`, `DAT` |
| `0x850A000C` | C -> S | `(item_iid: u64)` | **DropItem:** Drops `item_iid` from inventory into the world. | `BIN`, `DAT` |
| `0x850A0016` | C -> S | `(item_iid: u64)` | **PickupItem:** Picks up `item_iid` into inventory. | `BIN`, `DAT` |
| `0x850A0019` | C -> S | `(quantity_or_flags: i32, item_iid: u64)` | **DestroyItem:** Destroys `item_iid` (`quantity_or_flags = 0` for full stack). | `BIN`, `DAT` |
| `0x850A001D` | C -> S | `(target_slot: u32, flags: u32, item_iid: u64)` | **EquipItem:** Equips `item_iid` into `target_slot` (`0x20000000 \| (1 << (equip_index - 1))`, or `0` for default paperdoll slot; `flags = 0`). | `BIN`, `DAT` |
| `0x850A0021` | S -> C | `(err_code: u32, item_iid: u64)` | **InventoryError:** Notifies client of an inventory operation failure. | `BIN`, `DAT` |
| `0x850A0024` | S -> C | `(item_iid: u64)` | **ItemRemovedNotification:** Clears `item_iid` from new-item list and UI cache. | `BIN`, `DAT` |
| `0x850A0033` | S -> C | `(item_iid: u64)` | **NewItemAddedNotification:** Marks `item_iid` as newly acquired in backpack. | `BIN`, `DAT` |
| `0x850A0034` | C -> S | `(item_iid: u64)` | **ClearNewItemFlag:** Clears new-item highlight for `item_iid` (`0` = clear all). | `BIN`, `DAT` |
| `0x850A003E` | C -> S | `(item_iid: u64)` | **ClaimPendingLootItem:** Claims `item_iid` from Pending Loot (`0x10003684`) into backpack. | `BIN`, `DAT` |
| `0x850A0042` | C -> S | `()` | **ClaimAllPendingLoot:** Claims all items from Pending Loot into backpack. | `BIN`, `DAT` |
| `0x850A0043` | C -> S | `(item_iid: u64)` | **DiscardPendingLootItem:** Permanently discards `item_iid` from Pending Loot. | `BIN`, `DAT` |
| `0x85130001` | S -> C | `(container_iid: u64, flag: bool)` | **OpenContainerLoot:** Opens corpse/chest loot window for `container_iid`. | `BIN`, `DAT` |
| `0x8513000C` | S -> C | `(item_iid: u64, err_code: u32)` | **ItemMoveFailed:** Reverts optimistic UI drag/drop for `item_iid`. | `BIN`, `DAT` |
| `0x8513000D` | S -> C | `(item_iid: u64)` | **ContainerItemAdded:** Notifies UI that `item_iid` was added to an active container. | `BIN`, `DAT` |
| `0x8513000E` | S -> C | `(item_iid: u64, slot: u32, container_iid: u64)` | **ContainerItemRemoved:** Notifies UI that `item_iid` at `slot` was removed from `container_iid`. | `BIN`, `DAT` |
| `0x85120001` | C -> S | `(loot_all: bool, item_iids: Array<u64>, container_iid: u64)` | **LootItemsFromContainer:** Loots specified items (or all items if `loot_all == true`) from `container_iid`. | `BIN`, `DAT` |
| `0x85120002` | C -> S | `(container_iid: u64)` | **CloseContainerLoot:** Notifies server that the player closed `container_iid`. | `BIN`, `DAT` |
| `0x8535001D` | C -> S | `(action_flags: i32, item_iid: u64)` | **UseItem:** Activates a non-skill usable item `item_iid` (`action_flags = 0`). (Items with `PID 0x10000426` execute their skill via `0x85280006` instead.) | `BIN`, `DAT` |
| `0x8535001E` | S -> C | `(err_code: u32, action_flags: i32, item_iid: u64)` | **UseItemResult:** Result of `0x8535001D`. | `BIN`, `DAT` |
| `0x8A04000C` | C -> S | `(quantity: i32, chest_id: u32, item_iid: u64, bank_iid: u64)` | **DepositToVault:** Deposits `item_iid` into Vault / Shared Storage `bank_iid`. | `BIN`, `DAT` |
| `0x8A04000D` | C -> S | `(dest_slot: u32, quantity: i32, item_iid: u64, bank_iid: u64)` | **WithdrawFromVault:** Withdraws `item_iid` from `bank_iid` to backpack `dest_slot`. | `BIN`, `DAT` |
| `0x90550003` | C -> S | `(item_iid: u64, carry_all_iid: u64)` | **DepositIntoCarryAll:** Deposits `item_iid` into Carry-All bag `carry_all_iid`. | `BIN`, `DAT` |
| `0x90550004` | C -> S | `(dest_slot: u32, quantity: i32, item_did: u32, carry_all_iid: u64)` | **WithdrawFromCarryAll:** Withdraws `quantity` of template `item_did` from `carry_all_iid` to `dest_slot`. | `BIN`, `DAT` |

---

#### P1.6 In-World Character Property & Level Updates (`CharacterEvent`) (`BIN`, `DAT`)

##### Live Property Update `NetBlob` Events (`BIN`)
| Event ID (Hex) | Event ID (Dec) | Property Value Type | Semantics | Provenance |
|---|---|---|---|---|
| `0x0052` | `82` | `PropertyCollection` | Full or multi-property collection update for `entity_iid`. | `BIN` |
| `0x0088` | `136` | `Int32` (`Type 0x02`) | Sets `PID` to absolute `i32` value (used for `Advancement_Level` `0x10001011`, `Inventory_Quantity` `0x10000914`, `Inventory_MaxQuantity` `0x10000915`, `Item_StructurePoints` `0x10000D86`, `Wallet_Money` `0x1000072E`, and integer stats). | `BIN` |
| `0x0092` | `146` | `Int32` (`Type 0x02`) | Adds `i32` delta to current `PID` value. | `BIN` |
| `0x00FE` | `254` | `Float` (`Type 0x03`) | Sets `PID` to absolute `f32` value. | `BIN` |
| `0x00B0` | `176` | `Float` (`Type 0x03`) | Adds `f32` delta to current `PID` value. | `BIN` |
| `0x0149` | `329` | `Int64` (`Type 0x04`) | Sets `PID` to absolute `i64` value (e.g. XP total). | `BIN` |
| `0x000E` | `14` | `Bool` (`Type 0x05`) | Sets `PID` to boolean (`0` or `1`). | `BIN` |
| `0x00E7` | `231` | `InstanceID` (`Type 0x07`) | Sets `PID` to `u64` `InstanceID`. | `BIN` |
| `0x0076` | `118` | `DataID` (`Type 0x14`) | Sets `PID` to `u32` `DataID`. | `BIN` |
| `0x014A` | `330` | `Position` (`Type 0x10`) | Sets `PID` to `Position` struct. | `BIN` |
| `0x008E` | `142` | — | Removes `PID` from `entity_iid`. | `BIN` |
| `0x00B7` | `183` | Any (`Type`-prefixed) | Updates any typed property (including `Bitfield32` `PID 0x0000034E` `Inventory_Slot`, `Enum` `PID 0x10000D8B` `Item_WearState`, `Array`, and `HashTable`). | `BIN` |

##### Character Level Update (`PID 0x10001011` and WLib RPC `0x84DF0016`) (`BIN`, `DAT`)
- **Effective Level Property:** `PID 0x10001011` (`268439569`, `Advancement_Level`, `Int32`).
- **Unscaled Real Level Property:** `PID 0x1000493B` (`268454203`, `Advancement_RealLevel`, `Int32`; falls back to `0x10001011` when `0`).
- **Quiet Update (`CharacterEvent::LevelChanged { new_level, quiet: true }`):** A `0x0088` or `0x0052` property update changing `PID 0x10001011` without an accompanying `0x84DF0016` RPC updates the character's level silently.
- **Level-Up Presentation (`CharacterEvent::LevelChanged { new_level, quiet: false }`):** When a character gains a level in-world, the server updates `PID 0x10001011` and sends S->C WLib RPC **`0x84DF0016`** (0 arguments) on `NetBlob` `0x0115`, which reads `PID 0x10001011`, announces the new level in chat (`0x25008B2C`), and plays the level-up fanfare.

---

#### P1.7 Synthetic Test Vectors (`BIN`)

##### Test Vector 1: `NetBlob` Event `0x00CE` — Place Item `0x0300000000112233` into Avatar `0x0200000000000001` Backpack (`containment_type = 0x1000`)
- `event_id`: `CE 00` (`0x00CE`)
- `flags1`: `08` (`has_seq = 1`, no velocity or tail overrides)
- `entity_iid`: `33 22 11 00 00 00 00 03` (`0x0300000000112233` LE)
- `mode_flags`: `04` (`is_contained = 1`, `has_attachment = 0`)
- `sequence_num`: `01`
- `parent_container_iid`: `01 00 00 00 00 00 00 02` (`0x0200000000000001` LE)
- `containment_type`: `90 00` (`0x1000` = `4096` encoded as 2-byte 3-tier VLE: `0x80 | (0x1000 >> 8) = 0x90`, `0x00`)
- **Complete Hex Payload (22 bytes):**
  `CE 00 08 33 22 11 00 00 00 00 03 04 01 01 00 00 00 00 00 00 02 90 00`

##### Test Vector 2: C->S WLib RPC `0x850A000B` (`MoveItem`) — Move Full Stack of Item `0x0300000000112233` to Backpack Slot 5 (`0x40000005`) on Avatar `0x0200000000000001`
- `function_id`: `0B 00 0A 85` (`0x850A000B` LE)
- `arg[0] quantity`: `02 00 00 00 00` (`tag = 0x02`, `i32 = 0` LE)
- `arg[1] dest_slot`: `02 05 00 00 40` (`tag = 0x02`, `u32 = 0x40000005` LE)
- `arg[2] dest_container_iid`: `82 00 00 00 02 02 01 00 00 00` (`u64 = 0x0200000000000001` in 2-slot `WLibArgFrame` encoding)
- `arg[3] item_iid`: `82 00 00 00 03 02 33 22 11 00` (`u64 = 0x0300000000112233` in 2-slot `WLibArgFrame` encoding)
- **Complete WLib Payload (34 bytes):**
  `0B 00 0A 85 02 00 00 00 00 02 05 00 00 40 82 00 00 00 02 02 01 00 00 00 82 00 00 00 03 02 33 22 11 00`

##### Test Vector 3: C->S WLib RPC `0x850A001D` (`EquipItem`) — Equip Item `0x0300000000112233` into Main-Hand Weapon Slot (`0x20008000`, Slot Index 16 / Bit 15)
- `function_id`: `1D 00 0A 85` (`0x850A001D` LE)
- `arg[0] target_slot`: `02 00 80 00 20` (`tag = 0x02`, `u32 = 0x20008000` LE)
- `arg[1] flags`: `02 00 00 00 00` (`tag = 0x02`, `u32 = 0` LE)
- `arg[2] item_iid`: `82 00 00 00 03 02 33 22 11 00` (`u64 = 0x0300000000112233`)
- **Complete WLib Payload (24 bytes):**
  `1D 00 0A 85 02 00 80 00 20 02 00 00 00 00 82 00 00 00 03 02 33 22 11 00`
