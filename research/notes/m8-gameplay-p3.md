<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 Gameplay Protocol Research Note — P3: NPC Interaction, Quests, Deeds, Vendors, Barter, Loot, and Wallet

- **Status:** Draft
- **Last updated:** 2026-10-22
- **Author role:** Spec writer (dirty room, `m8-gameplay-p3`)
- **Reviewer:** pending

---

## 1. Provenance Table <a id="provenance"></a>

| Tag | Method | Source | Identifier / SHA-256 | Question Answered | Why M1–M2 Alone Were Insufficient |
|---|---|---|---|---|---|
| `BIN-CLIENT` | M4 (`BIN`) | `lotroclient64.exe` (x86-64 PE32+, Live build) | `b3939821242343ad2bb6cd7dc72ebdc7b3fe9fd2f03c4eb41f1cbfdc2cb0cf32` | C++ UI element handlers (`0x140d00000`–`0x140fa0000`), WLib native bindings (`0x008E`, `0x00D9`, `0x011E`–`0x0C21`), WState wire serializers (`0x140363a00`–`0x140368460`), native `0x0796` `BarterRequest` wire serializer (`0x140a4a640`), Loot Roll/Pass UI button dispatch (`0x140d74e31`–`0x140d75068`), and Quest Dispenser UI action dispatch (`0x140e55800`–`0x140e5d4e0`). | Official wire opcodes, native `WState` C++ serializers, and WLib native bridge signatures exist only in the compiled client binary and embedded WLib bytecode. |
| `BIN-WLIB` | M4 (`BIN`) | Embedded WLib bytecode archive (`0x40000000` inside `client_gamelogic.dat` / `wlib.bin`) | `client_gamelogic.dat` record `0x40000000` (`14,485,809` bytes, 4,946 WLib classes, 120 RPC classes `0x8xxxxxxx`) | Exact C→S (`packs`) and S→C (`unpacks`) RPC method IDs (`0x8xxxxxxx`), parameter serialization order (`0x008E0001`–`0x008E000A`), dispatch targets (`0x00D90003`, `0x00D9000C`, `0x00D9000D`), and WLib class slot layouts (`0x04CD`, `0x057D`, `0x063E`, `0x06D9`, `0x09D8`). | All gameplay C→S and S→C messages in LOTRO are serialized and deserialized by WLib bytecode methods in `0x40000000`. |
| `DAT-LOGIC` | M1 (`DAT`) | `client_gamelogic.dat` master property registry (`0x34000000`) | `client_gamelogic.dat` record `0x34000000` | Property IDs (`0x1000xxxx` / `0x0000xxxx`), property names, and data types for Quests, Deeds, NPCs, Vendors, Barter, Repair, Loot, and Wallet (`PaperItem`). | Needed to map property IDs read/written by WLib RPC handlers and C++ UI elements to their exact semantic meanings. |
| `DAT-ENUM` | M1 (`DAT`) | `client_gamelogic.dat` `EnumMapper` records (`0x2300xxxx`) | `0x23000017` (`QuestStatus`), `0x23000018` (`QuestUpdateType`), `0x2300001D` (`QuestRoleAction`), `0x2300001E` (`UIQuestScope`), `0x2300004F` (`QuestEventType`), `0x2300006B` (`FellowshipLootType`), `0x230000D1` (`UIElementID`), `0x2300026F` (`BarterReputationRewardTier`), `0x23000292` (`FellowshipQuestResponseType`) | Exact integer enumeration constants for Quest Dispenser actions, quest statuses, fellowship quest share responses, fellowship loot rules, and UI button IDs. | Enumeration integer-to-symbol mappings are stored in `0x2300xxxx` `EnumMapper` records inside `client_gamelogic.dat`. |
| `DAT-HIGHRES` | M1 (`DAT`) | `client_highres.dat` UI layout records (`0x2200xxxx`) | `0x22000067` (`questdisplay`), `0x22000044` (`questhistory`), `0x22000028` (`questtracker`) | UI action element IDs (`0x1000xxxx`) and layout hierarchy for Quest Offer/Bestowal, Quest Journal, and Quest Tracker panels. | Confirms which C++ UI vtable handlers correspond to Accept, Decline, Complete, Cancel, Share, and Reward Selection buttons. |

---

## 2. Dirty-Room Reverse Engineering Findings <a id="dirty-room-findings"></a>

### 2.1 WLib RPC Serialization & Transport Primitives (`0x008E` & `0x00D9`) <a id="dirty-wlib-primitives"></a>

All P3 gameplay messages are implemented as WLib RPC methods (`0x8xxxxxxx`, where bits `16..30` are the WLib RPC class ID and bits `0..15` are the method index). Each RPC method operates on a WState serialization stream (`Package` / `Buffer`) using native class `0x008E` (`va=0x14039f980`–`0x1403b4109`) and dispatches via native class `0x00D9` (`va=0x1403a6230`–`0x1403a6c51`):

| Native ID | C++ VA | Direction | Wire Type | Stack Slots | Description |
|---|---|---|---|---|---|
| `0x008E0001` | `0x14039fc60` | Write (C→S) | `WState` object | 1 (`[rcx-0x10]`) | Calls `0x140367d20` to serialize a WState object/struct or `WArray` (class ID + typed fields) into the RPC buffer. |
| `0x008E0002` | `0x14039fa30` | Write (C→S) | `u32` / `i32` / `enum` / `DataID` / `bool` | 1 (`[rcx-0x10]`) | Writes a 32-bit little-endian integer, enum, `DataID`, or boolean flag to the RPC buffer. |
| `0x008E0003` | `0x14039fba0` | Write (C→S) | `InstanceID` (`u64`) | 2 (`[rcx-0x18..-0x10]`) | Writes a 64-bit little-endian `InstanceID` (`u64`) to the RPC buffer. |
| `0x008E0004` | `0x14039f980` | Write (C→S) | `f32` | 1 (`[r9-0x10]`) | Writes a 32-bit IEEE-754 float (`edx=4`) to the RPC buffer. |
| `0x008E0005` | `0x14039fae0` | Write (C→S) | `f64` / `u64` | 2 (`[r9-0x18..-0x10]`) | Writes a 64-bit value (`shl rcx, 0x20; or rcx, rax`) to the RPC buffer. |
| `0x008E0006` | `0x1403b4070` | Read (S→C) | `WState` object | 1 (`bool` arg) | Calls `0x140368460` (`dl = allow_null_or_polymorphic`) to deserialize a WState object/struct or `WArray` (`StringInfo`, `Array`, item/quest struct) from the RPC buffer. |
| `0x008E0007` | `0x1403b3dc0` | Read (S→C) | `u32` / `i32` / `enum` / `DataID` / `bool` | 0 → 1 | Reads a 32-bit little-endian integer, enum, `DataID`, or boolean from the RPC buffer. |
| `0x008E0008` | `0x1403b3f90` | Read (S→C) | `InstanceID` (`u64`) | 0 → 2 | Reads a 64-bit little-endian `InstanceID` (`u64`) from the RPC buffer. |
| `0x008E0009` | `0x1403b3ce0` | Read (S→C) | `f32` | 0 → 1 | Reads a 32-bit IEEE-754 float from the RPC buffer. |
| `0x008E000A` | `0x1403b3e90` | Read (S→C) | `f64` / `u64` | 0 → 2 | Reads a 64-bit value from the RPC buffer. |
| `0x00D9000C` | `0x1403a6a80` | Send (C→S) | Entity RPC (`0x140522190`) | `(target_iid, opcode_u32, pkg)` | Primary C→S entity-targeted RPC send (used by Selection, Quests, Deeds, Vendors, Repair, Barter, Loot, Wallet). |
| `0x00D9000D` | `0x1403a6a80` | Send (C→S) | Entity RPC (`0x140522190`) | `(target_iid, opcode_u32, pkg)` | Secondary C→S entity-targeted RPC send (shares native implementation `0x1403a6a80` with `0x00D9000C`; used by `QuestDispenser` `0x8522xxxx` and Vendor `0x85360007`). |
| `0x00D90003` | `0x1403a6b40` | Send (C→S) | Routed RPC (`0x140522350`) | `(target_iid, src_iid, opcode_u32, pkg, flags)` | Bidirectional / routed RPC send (used by Quest Category Limiter `0x852100F7`, `0x852100F8`, `0x852100FA`). |

---

### 2.2 Complete P3 Subsystem → WLib Manager & RPC Class Mapping <a id="dirty-class-map"></a>

| Subsystem | RPC Class | Opcode Prefix | C→S Packs | S→C Unpacks | Paired WLib Manager Class(es) | C++ Dispatch Sites |
|---|---|---|---|---|---|---|
| **Entity Selection & Use** | `0x0535` (1333) | `0x8535xxxx` | 3 | 2 | `0x0127` (295 `Selection/Usage`), `0x01D6` (470) | `0x140b54671`–`0x140b651e7` (20 sites) |
| **Targeted Action & Combo / Role Use** | `0x0528` (1320) | `0x8528xxxx` | 5 | 4 | `0x0127` (295), `0x017A` (378) | `0x140b54671`–`0x140b651e7` (45 sites) |
| **Targeted Drama / Recall / Action Dialog** | `0x0524` (1316) | `0x8524xxxx` | 4 | 4 | `0x0628` (1576), `0x06DB` (1755), `0x0C47` (3143) | WLib internal |
| **Multi-Usage Activity** | `0x0ED5` (3797) | `0x8ED5xxxx` | 4 | 6 | `0x0EDD` (3805 `MultiUsageSystem`) | `0x140b64143`–`0x140b7fb16` (9 sites) |
| **NPC Action Picker & Session** | `0x0523` (1315) | `0x8523xxxx` | 8 | 6 | `0x0123` (291 `NPCActionPicker / Session`) | `0x140b66c95`–`0x140b8e54c` (12 sites) |
| **NPC / Vendor / Buyback / Service** | `0x04FE` (1278) | `0x84FExxxx` | 20 | 39 | `0x0102` (258), `0x0123` (291), `0x019D` (413 `Buyback`), `0x0452` (1106) | `0x140b4d5b0`–`0x140b5719b` (84 sites) |
| **Quest Dispenser (NPC Quest Offer/Bestow/Turn-In)** | `0x0522` (1314) | `0x8522xxxx` | 2 | 2 | `0x011E` (286 `QuestDispenserSystem`) | `0x140b67740` (UI `0x140e5c6b0`–`0x140e5eb90`) |
| **NPC Dialog Text / Billboard** | `0x0527` (1319) | `0x8527xxxx` | 0 | 1 | `0x037F` (895 `DialogText`) | WLib internal |
| **NPC Comment Drama** | `0x08CB` (2251) | `0x88CBxxxx` | 1 | 1 | `0x08CC` (2252 `NPCCommentSystem`) | `0x140b66037`–`0x140b7b78e` (7 sites) |
| **Drama Performance** | `0x04F6` (1270) | `0x84F6xxxx` | 1 | 0 | `0x06CE` (1742 `DramaSystem`) | WLib internal |
| **Quests (Journal, Progress, Tracker, Share)** | `0x0521` (1313) | `0x8521xxxx` | 14 | 36 | `0x011F` (287 `QuestSystem`), `0x06D9` (1753), `0x0C0E` (3086) | `0x140b4e111`–`0x140b642d7` (77 sites; UI `0x140e4f960`–`0x140e6b4f0`) |
| **Deeds / Accomplishments** | `0x04DA` (1242) | `0x84DAxxxx` | 1 | 3 | `0x00FC` (252 `AccomplishmentSystem`) | `0x140b67c88`–`0x140b8722d` (7 sites; UI `0x140d0c350`–`0x140d13720`) |
| **Titles** | `0x052E` (1326) | `0x852Exxxx` | 8 | 3 | `0x017E` (382 `TitleSystem`) | `0x140b4f642`–`0x140b5a26a` (6 sites) |
| **Vendor Buy / Sell / SellAll** | `0x0536` (1334) | `0x8536xxxx` | 6 | 1 | `0x01CE` (462 `VendorSystem`) | `0x140a98fd7`–`0x140aa123b` (13 sites; UI `0x140fa2070`) |
| **Item Repair** | `0x050B` (1291) | `0x850Bxxxx` | 2 | 4 | `0x0171` (369 `RepairSystem`), `0x0651` (1617 `Discount`) | `0x140b808c6`–`0x140b92c1a` (5 sites) |
| **Barter** | `0x0793` (1939) | `0x8793xxxx` | 1 | 1 | `0x0794` (1940), `0x0795` (1941 `BarterSystem`) | `0x140b61c28`–`0x140b8dadd` (7 sites; UI `0x140fa4ab0`) |
| **Container / Corpse / Chest Open & Close** | `0x0513` (1299) | `0x8513xxxx` | 2 | 2 | `0x092E` (2350 `CMEOInventorySystem`) | `0x140b4f910`–`0x140b90ce1` (9 sites; UI `0x140e0d410`–`0x140e122e0`) |
| **Loot Rules, Roll/Pass, Master & Pending Loot** | `0x0512` (1298) | `0x8512xxxx` | 8 | 7 | `0x0112` (274 `LootSystem`), `0x0751` (1873) | `0x140b647ff`–`0x140b922b5` (11 sites) |
| **Inventory & Container Item Operations** | `0x04EC` (1260) | `0x84ECxxxx` | 11 | 11 | `0x0102` (258 `InventorySystem`) | `0x140b4d5b0`–`0x140b5719b` (41 sites) |
| **Inventory Transaction Support** | `0x050A` (1290) | `0x850Axxxx` | 8 | 2 | `0x0170` (368 `InvSupportSystem`), `0x0AF4` (2804) | `0x140b52008`–`0x140b5aa82` (26 sites) |
| **Wallet (`PaperItem`)** | `0x0C0D` (3085) | `0x8C0Dxxxx` | 1 | 4 | `0x0C0C` (3084 `PaperItemSystem`), `0x0E7D` (3709) | `0x140b64cb7`–`0x140b83f37` (17 sites; UI `0x140cff190`–`0x140d04a20`) |

---

### 2.3 Detailed Dirty-Room Analysis by Subsystem <a id="dirty-subsystems"></a>

#### 2.3.1 Entity Selection, Interaction Use &NPC Dialogue (`0x8535`, `0x8528`, `0x8524`, `0x8522`, `0x8523`, `0x8527`, `0x88CB`, `0x84F6`) <a id="dirty-npc-dialogue"></a>

1. **Selection & Use (`RPC Class 0x0535` / 1333, paired with `WLib Class 0x0127` / 295):**
   - `0x85350002` (C→S, `writes=[1 (WState)]`, `send=0x00D9000C`, caller `295::29` at `wlib+0x19bb7c`): Sends an entity interaction/selection state struct (`WState`).
   - `0x8535001D` (C→S, `writes=[1 (WState)]`, `send=0x00D9000C`, callers `295::75`, `295::1`, `295::2` at `wlib+0x19d414`–`0x19ef8c`): Instantiates WState `Class 0x063E` (1598) containing the target entity `InstanceID` and interaction/use parameters, and sends it to the player's entity on the server.
   - `0x8535003A` (C→S, `writes=[2 (u32)]`, `send=0x00D9000C`, caller `295::62` at `wlib+0x19c9dc`): Sends a `u32` selection/usage command or action code.
   - `0x8535000C` (S→C, `reads=[6 (WState, 0)]`, target `295::15` at `wlib+0x19ea64`): Server response/update carrying an interaction `WState` object.
   - `0x85350027` (S→C, `reads=[7 (u32), 8 (InstanceID)]`, target `295::14` at `wlib+0x19ed00`): Server notification of entity use/selection result (`u32` status/action, `u64` target `InstanceID`).

2. **Targeted Action & Role Use (`RPC Class 0x0528` / 1320, paired with `WLib Class 0x0127` / 295 & `0x017A` / 378):**
   - `0x85280002` (C→S, `writes=[]`, caller `378::296`): Zero-argument action cancel/acknowledge.
   - `0x85280005` (C→S, `writes=[1 (WState)]`, caller `378::264`): Sends a `WState` action request.
   - `0x85280006` (C→S, `writes=[2 (u32), 3 (InstanceID)]`, caller `295::32` at `wlib+0x19a210`, which checks `0x10000035 QuestDispenser_RoleSuccessText`): Performs a role/quest action (`u32` action/role type, `u64` target `InstanceID`).
   - `0x85280007` (C→S, `writes=[]`, caller `378::265`) and `0x85280008` (C→S, `writes=[]`, caller `378::276`).
   - S→C: `0x8528000A` (`reads=[7 (u32), 7 (u32), 7 (u32)]`), `0x8528000D` (`reads=[7 (u32)]`), `0x8528000E` (`reads=[7 (u32), 7 (u32), 8 (InstanceID)]`), `0x8528001A` (`reads=[7 (u32)]`).

3. **Targeted Drama / Action Dialog (`RPC Class 0x0524` / 1316, paired with `WLib Class 0x06DB` / 1755 & `0x0C47` / 3143):**
   - Associated properties: `Mouseover_TargetedActionType` (`0x10000043`), `Mouseover_DramaActionType` (`0x1000079A`), `Usage_TargetActionWithDialog` (`0x100041E6`).
   - C→S: `0x85240002` (`writes=[2 (u32)]`), `0x8524000C` (`writes=[2 (u32)]`), `0x8524000E` (`writes=[2 (u32)]`), `0x85240015` (`writes=[]`).
   - S→C: `0x85240001` (`reads=[7 (u32)]`), `0x85240003` (`reads=[7 (u32), 6 (WState, 0), 7 (u32)]`), `0x85240009` (`reads=[]`), `0x85240018` (`reads=[6 (WState, 0), 7 (u32)]`).

4. **NPC Action Picker & Service Session (`RPC Class 0x0523` / 1315, paired with `WLib Class 0x0123` / 291):**
   - Associated properties on `Class 291` (`0x0123`): `NPC_ActionPicker_QuestScript` (`0x1000018D`), `NPC_ActionPicker_SkillTrainerScript` (`0x10000270`), `NPC_ActionPicker_SelectionScript` (`0x10000271`), `NPC_ActionPicker_MPMissionControllerString` (`0x10000451`), `Vendor_Inventory` (`0x10000059`), `UI_Vendor_FilterType` (`0x10000065`).
   - C→S RPCs (`send=0x00D9000C`):
     - `0x85230008`: `writes=[2 (u32), 3 (InstanceID)]` (caller `291::26` at `wlib+0x149f88`) — Select NPC action / service option (`u32` action/option ID, `u64` NPC `InstanceID`).
     - `0x85230010`: `writes=[2 (u32), 2 (u32)]` (caller `291::33` at `wlib+0x14aa70`).
     - `0x85230016`: `writes=[3 (InstanceID)]` (caller `291::11` at `wlib+0x149ec0`) — Initiate / open NPC interaction session with `npc_iid` (`u64`).
     - `0x85230017`: `writes=[]` (caller `291::34` at `wlib+0x14bd20`) — Close / end current NPC interaction session.
     - `0x8523001A`: `writes=[2 (u32)]` (caller `291::14` at `wlib+0x14ac64`).
     - `0x85230022`: `writes=[3 (InstanceID), 3 (InstanceID)]` (caller `291::12` at `wlib+0x149d58`).
     - `0x85230039`: `writes=[2 (u32)]` (caller `291::13` at `wlib+0x14acc0`).
     - `0x85230045`: `writes=[2 (u32)]` (caller `291::28` at `wlib+0x14a0c4`).
   - S→C RPCs:
     - `0x85230006`: `reads=[8 (InstanceID), 7 (u32), 7 (u32), 6 (WState, 0)]` (target `291::19` at `wlib+0x14c93c`) — Opens NPC Action Picker / dialogue menu for `npc_iid` (`u64`) with two `u32` flags/counts and a `WState` action/quest array.
     - `0x85230009`: `reads=[6 (WState, 0), 7 (u32)]` (target `291::21` at `wlib+0x14c610`).
     - `0x85230013`: `reads=[7 (u32), 8 (InstanceID), 6 (WState, 0)]` (target `291::17` at `wlib+0x14cc34`, which invokes `Vendor_Inventory` handler `wlib+0xab400` and Quest Manager `287::35` at `wlib+0x142644`) — Opens/populates NPC service session (Vendor inventory / Quest list) for `npc_iid` (`u64`).
     - `0x8523001C`: `reads=[7 (u32)]` (target `291::18` at `wlib+0x14cad0`) — Session close / error status (`u32`).
     - `0x8523003B`: `reads=[8 (InstanceID)]` (target `291::16` at `wlib+0x14ce5c`) — Terminate / clear NPC interaction with `npc_iid` (`u64`).
     - `0x85230040`: `reads=[7 (u32), 7 (u32)]` (target `291::20` at `wlib+0x14c890`).

5. **Quest Dispenser — NPC Quest Offer / Bestowal / Turn-In Dialog (`RPC Class 0x0522` / 1314, paired with `WLib Class 0x011E` / 286):**
   - C++ UI class (`va=0x140e5c6b0`–`0x140e5eb90`, layout `0x22000067` `questdisplay`) calls `0x011E0001` (`va=0x140b67740` -> `286::1` at `wlib+0x1363c8`) when the player clicks Accept, Decline, Continue, or Complete in the NPC Quest Dialog, after reading `QuestTreasure_SelectedItem` (`0x100000BC`) and `QuestDispenser_Action` (`0x10000BDC`).
   - C→S RPCs (`send=0x00D9000D`):
     - `0x85220005`: `writes=[3 (InstanceID)]` (caller `1757::2` at `wlib+0x3e62f4`) — Queries quest role / dispenser data for `npc_iid` (`u64`) (used by `/getquestroledata` / `DoCmdGetQuestRoleData`).
     - `0x85220006`: `writes=[1 (WState), 2 (u32), 3 (InstanceID)]` (caller `286::1` at `wlib+0x1363c8`) — Sends the player's Quest Dispenser action to the server:
       1. `WState` (`0x008E0001`): Action parameters / selected reward item (`QuestTreasure_SelectedItem` `0x100000BC`, quest `DataID`).
       2. `u32` (`0x008E0002`): `QuestDispenser_Action` (`0x10000BDC` enum: e.g., accept/bestow, advance/complete, cancel/decline).
       3. `InstanceID` (`0x008E0003`): Dispenser NPC or item `InstanceID` (`u64`).
   - S→C RPCs:
     - `0x85220002`: `reads=[6 (WState, 0), 7 (u32), 8 (InstanceID)]` (target `286::2` at `wlib+0x1369c8`) — Server opens the NPC Quest Offer / Turn-in dialog (`questdisplay` `0x22000067`):
       1. `WState` (`0x008E0006`): Quest dialog payload (quest `DataID`, dialog text `StringInfo`, reward items/currencies).
       2. `u32` (`0x008E0007`): `QuestDispenser_Action` / dialog mode enum.
       3. `InstanceID` (`0x008E0008`): Dispenser NPC/item `InstanceID` (`u64`).
     - `0x8522000A`: `reads=[7 (u32), 8 (InstanceID)]` (target `286::7` at `wlib+0x136154`) — Server closes or reports result (`u32` status/error code) for Quest Dispenser interaction with `npc_iid` (`u64`).

6. **NPC Dialog Text, Comments & Drama (`0x8527`, `0x88CB`, `0x84F6`):**
   - `0x85270006` (S→C, `reads=[6 (WState, 0)]`, target `895::1` at `wlib+0x2959d0`): Delivers a `WState` containing `GCParam_DialogText` (`0x10000ED2`), `GCParam_DisplayText_IsBillboardText` (`0x10000CDF`), and `GCParam_DialogText_InitiatorIID` (`0x10003C2D`).
   - `0x88CB0003` (C→S, `writes=[2 (u32), 3 (InstanceID)]`, `send=0x00D9000C`, caller `2252::7` at `wlib+0x4a7f6c`): Requests NPC comment drama (`u32` comment `DataID` from `NPCComment_DIDCommentArray` `0x10002128`, `u64` `npc_iid`).
   - `0x88CB0001` (S→C, `reads=[7 (u32), 8 (InstanceID)]`, target `2252::1` at `wlib+0x4a826c`): Server triggers NPC comment (`u32` comment `DataID` `NPCComment_PickedComment` `0x1000212A`, `u64` `npc_iid`).
   - `0x84F60001` (C→S, `writes=[3 (InstanceID), 1 (WState)]`, `send=0x00D9000C`, caller `1742::2` at `wlib+0x3e0d90`): Sends drama interaction/skip request during `Drama_Performing` (`0x1000076E`).

---

#### 2.3.2 Quests: Journal, Objective Progress, Rewards, Sharing & Tracker (`RPC Class 0x0521` / 1313, `0x8521xxxx`) <a id="dirty-quests"></a>

Paired with **`WLib Class 0x011F` (287 `QuestSystem`)**, **`Class 0x06D9` (1753)**, and **`Class 0x0C0E` (3086 `QuestCategoryLimiter`)**.
C++ UI classes:
- **Quest Offer / Bestowal / Turn-In (`questdisplay`, layout `0x22000067`):** `0x140e5c6b0`–`0x140e5eb90`
  - UI Element IDs: `0x100007DE` (`AcceptButton`), `0x100007DF` (`DeclineButton`), `0x10000909` (`CancelButton`), `0x10000908` (`OKButton`), `0x10000CA8` (`AcceptNextQuestButton`), `0x10000CA9` (`FinishQuestButton`), `0x10000CAA` (`RepeatQuestButton`), `0x10000837` (`QuestRewardsField`), `0x10000838` (`QuestOptionalRewardsField`), `0x1000083C` (`SelectOneNote`).
- **Quest Journal (`questhistory`, layout `0x22000044`):** `0x140e4f960`–`0x140e5c400`
  - UI Element IDs: `0x100007EE` (`AbandonButton`), `0x10000CE4` (`ShareQuestButton`), `0x10000ED0` (`AddToTrackerButton`), `0x100018E7` (`CompleteButton`), `0x10000898` (`ShowCompletedQuestsButton`), `0x10005116` (`QuestHistoryFilterField`).
- **Quest Tracker (`questtracker`, layout `0x22000028`):** `0x140e60000`–`0x140e6b4f0`
  - Properties: `Quest_TrackerArray` (`0x10000E6B`), `Quest_TrackerArrayLocked` (`0x10000291`), `Quest_TrackerQuestDID` (`0x100001AB`).

##### C→S Quest RPCs (`0x8521xxxx`, 14 methods):

| Opcode | Writes (`0x008E`) | Send Native | WLib Caller | Semantic Meaning |
|---|---|---|---|---|
| `0x85210008` | `[2 (u32)]` | `0x00D9000C` | `287::208` (`0x14155c`), `287::211` (`0x1409f4`) | Abandon / cancel active quest (`u32` quest `DataID`). |
| `0x85210013` | `[2 (u32)]` | `0x00D9000C` | `287::174` (`0x13fa10`) | Share quest with fellowship (`u32` quest `DataID`). |
| `0x8521002F` | `[2 (u32), 2 (u32)]` | `0x00D9000C` | `287::185` (`0x142228`) | Update quest tracker slot / state (`u32` quest `DataID`, `u32` slot/action). |
| `0x8521003D` | `[2 (u32)]` | `0x00D9000C` | `287::42` (`0x1416ec`) | Request quest details / bestow or advance quest by `DataID` (`u32`). |
| `0x8521004B` | `[2 (u32)]` | `0x00D9000C` | `287::232` (`0x136bf4`) | Trigger / accept auto-bestowed or remote quest (`u32` quest `DataID`). |
| `0x85210057` | `[2 (u32)]` | `0x00D9000C` | `287::179` (`0x13ec20`) | Request quest guide / map location data (`u32` quest `DataID`). |
| `0x8521006B` | `[2 (u32), 3 (InstanceID), 2 (u32)]` | `0x00D9000C` | `1753::2` (`0x3e4c30`) | Respond to fellowship quest share dialog (`u32` response bool/enum, `u64` `Quest_ShareDialog_SharingPlayerIID` `0x10000D76`, `u32` `Quest_DataID` `0x10001426`). |
| `0x85210082` | `[]` | `0x00D9000C` | `287::192` (`0x140d0c`) | Clear / sync quest state (zero-argument request). |
| `0x852100B1` | `[2 (u32), 3 (InstanceID)]` | `0x00D9000C` | `287::209` (`0x1410dc`) | Interact with quest dispenser / item (`u32` quest `DataID`, `u64` source `InstanceID`). |
| `0x852100D6` | `[2 (u32), 2 (u32)]` | `0x00D9000C` | `287::189` (`0x141d64`) | Modify quest tracker (`u32` quest `DataID`, `u32` tracker operation/flag). |
| `0x852100F7` | `[3 (InstanceID)]` | `0x00D90003` | `3086::3` (`0x5c3648`) | Query quest category limiter status for `player_iid` (`u64`). |
| `0x852100F8` | `[3 (InstanceID)]` | `0x00D90003` | `3086::4` (`0x5c3580`) | Reset / update quest category limiter for `player_iid` (`u64`). |
| `0x852100FA` | `[3 (InstanceID)]` | `0x00D90003` | `3086::5` (`0x5c341c`) | Query `QuestCategoryLimiter_ResetTime` (`0x10003B0B`) for `player_iid` (`u64`). |
| `0x85210111` | `[2 (u32)]` | `0x00D9000C` | `287::238` (`0x14191c`) | Complete remote / auto-complete quest (`u32` quest `DataID`, triggered by `CompleteButton` `0x100018E7`). |

##### S→C Quest RPCs (`0x8521xxxx`, 36 methods):

| Opcode | Reads (`0x008E`) | WLib Target | Semantic Meaning |
|---|---|---|---|
| `0x8521000E` | `[7 (u32)]` | `287::69` (`0x13ac78`) | Quest notification / status update for quest `DataID` (`u32`). |
| `0x85210010` | `[7 (u32), 7 (u32), 6 (WState, 0)]` | `287::81` (`0x13e518`) | Objective / role progress update (`u32` quest `DataID`, `u32` objective/role index, `WState` progress data). |
| `0x8521001F` | `[7 (u32), 7 (u32)]` | `287::52` (`0x13fa70`) | Quest state transition (`u32` quest `DataID`, `u32` status/error code). |
| `0x85210020` | `[6 (WState, 0), 7 (u32)]` | `287::88` (`0x13c9dc`) | Quest data / objective struct update (`WState` quest record, `u32` quest `DataID`). |
| `0x85210027` | `[7 (u32), 7 (u32)]` | `287::76` (`0x13b4a0`) | Quest counter / timer update (`u32` quest `DataID`, `u32` value). |
| `0x85210041` | `[7 (u32), 8 (InstanceID), 7 (u32)]` | `287::55` (`0x141748`) | Fellowship quest share prompt (`u32` status/flag, `u64` sharing player `InstanceID`, `u32` quest `DataID`; instantiates `Class 1753`). |
| `0x85210047` | `[6 (WState, 0), 7 (u32)]` | `287::87` (`0x13cd34`) | Quest objective array / state sync (`WState`, `u32` quest `DataID`). |
| `0x85210051` | `[6 (WState, 0)]` | `287::66` (`0x13ee4c`) | Quest radar / map position list response (`WState`). |
| `0x85210052` | `[7 (u32)]` | `287::70` (`0x13ab74`) | Quest removed / abandoned notification (`u32` quest `DataID`). |
| `0x85210053` | `[7 (u32), 7 (u32)]` | `287::84` (`0x13db9c`) | Quest repeat / cooldown status (`u32` quest `DataID`, `u32` timestamp/status). |
| `0x8521005C` | `[6 (WState, 0) × 6]` | `287::62` (`0x13ffe0`) | Full Quest Journal synchronization on login (6 `WState` tables: active quests, completed quests, cooldown timers, category limiters, tracker state, pending auto-bestows). |
| `0x8521005E` | `[6 (WState, 0), 7 (u32)]` | `287::79` (`0x13e884`) | Quest bestow / update with `WState` state and `u32` quest `DataID`. |
| `0x85210063` | `[6 (WState, 0)]` | `287::65` (`0x13ef9c`) | Batch quest list / history update (`WState`). |
| `0x85210067` | `[6 (WState, 0), 6 (WState, 0)]` | `287::67` (`0x13ec7c`) | Quest guide / location data update (2 `WState` objects). |
| `0x85210075` | `[7 (u32), 7 (u32), 7 (u32)]` | `287::80` (`0x13e7a4`) | Objective counter increment (`u32` quest `DataID`, `u32` objective index, `u32` current count). |
| `0x85210094` | `[7 (u32), 7 (u32)]` | `287::64` (`0x13f470`) | Quest share result (`u32` quest `DataID`, `u32` result code). |
| `0x8521009C` | `[7 (u32), 7 (u32)]` | `287::73` (`0x13b978`) | Quest status / error notification (`u32` quest `DataID`, `u32` code). |
| `0x852100AB` | `[7 (u32), 6 (WState, 0), 7 (u32)]` | `287::89` (`0x13c90c`) | Quest objective override / event update (`u32`, `WState`, `u32`). |
| `0x852100B5` | `[7 (u32)]` | `287::72` (`0x13baec`) | Single quest completed / advanced (`u32` quest `DataID`). |
| `0x852100BD` | `[7 (u32), 7 (u32), 8 (InstanceID), 7 (u32)]` | `287::63` (`0x13f5d4`) | Fellowship member quest share status (`u32` error/result, `u32` quest `DataID`, `u64` target player `InstanceID`, `u32` flag). |
| `0x852100C1` | `[7 (u32), 7 (u32)]` | `287::85` (`0x13d558`) | Quest timer / lock update (`u32` quest `DataID`, `u32` seconds). |
| `0x852100CA` | `[7 (u32), 7 (u32)]` | `287::74` (`0x13b868`) | Quest state flag update (`u32` quest `DataID`, `u32` flag). |
| `0x852100CE` | `[7 (u32)]` | `287::75` (`0x13b5c4`) | Auto-bestow quest available notification (`u32` quest `DataID`). |
| `0x852100CF` | `[6 (WState, 0), 7 (u32)]` | `287::86` (`0x13d0ec`) | Quest data sync (`WState`, `u32` quest `DataID`). |
| `0x852100D2` | `[7 (u32), 10 (f64/u64)]` | `287::83` (`0x13dfe8`) | Quest timestamp / expiration update (`u32` quest `DataID`, `f64`/`u64` timestamp). |
| `0x852100D5` | `[7 (u32), 6 (WState, 0)]` | `287::58` (`0x141cb4`) | Quest Tracker array sync (`u32` lock flag `Quest_TrackerArrayLocked` `0x10000291`, `WState` `Quest_TrackerArray` `0x10000E6B`). |
| `0x852100DF` | `[7 (u32), 7 (u32), 7 (u32)]` | `287::82` (`0x13e2a0`) | Quest objective state change (`u32` quest `DataID`, `u32` objective index, `u32` state). |
| `0x852100F7` | `[8 (InstanceID)]` | `3086::3` (`0x5c3648`) | Category limiter sync (`u64` `InstanceID`). |
| `0x852100F8` | `[8 (InstanceID)]` | `3086::4` (`0x5c3580`) | Category limiter reset (`u64` `InstanceID`). |
| `0x852100FA` | `[8 (InstanceID)]` | `3086::5` (`0x5c341c`) | Category limiter reset time (`u64` `InstanceID`). |
| `0x852100FC` | `[7 (u32)]` | `287::68` (`0x13ad64`) | Quest notification (`u32` quest `DataID`). |
| `0x85210103` | `[7 (u32), 7 (u32), 7 (u32)]` | `287::77` (`0x13b0a4`) | Quest medal / points update (`u32`, `u32`, `u32`). |
| `0x85210106` | `[7 (u32), 7 (u32)]` | `287::59` (`0x140d5c`) | Quest tracker slot response (`u32` quest `DataID`, `u32` slot). |
| `0x85210115` | `[7 (u32), 7 (u32)]` | `287::78` (`0x13ae68`) | Remote quest completion result (`u32` quest `DataID`, `u32` result). |
| `0x85210118` | `[7 (u32)]` | `287::71` (`0x13a9f4`) | Quest event notification (`u32` quest `DataID`). |
| `0x85210126` | `[7 (u32), 7 (u32)]` | `287::61` (`0x1407c0`) | Quest bestow/abandon status (`u32` quest `DataID`, `u32` status). |

---

#### 2.3.3 Deeds / Accomplishments (`RPC Class 0x04DA` / 1242, `0x84DAxxxx`) <a id="dirty-deeds"></a>

Paired with **`WLib Class 0x00FC` (252 `AccomplishmentSystem`)**.
In LOTRO's engine, Deeds are implemented as specialized Quests managed by `AccomplishmentSystem` (`Class 252`), which references Global, Class, and Race Accomplishment Directories in `client_gamelogic.dat` (`AccomplishmentSystem::GetAllAccomplishments`). Because Deeds are Quest records at the data layer, granular objective progress and completion notifications share `QuestSystem` (`0x8521xxxx`), while `0x84DAxxxx` handles Accomplishment-specific initialization and UI actions (C++ UI `0x140d0c350`–`0x140d13720`):

| Opcode | Direction | Signature (`0x008E`) | WLib Handler / Caller | Semantic Meaning |
|---|---|---|---|---|
| `0x84DA0007` | C→S | `writes=[3 (InstanceID), 1 (WState)]`, `send=0x00D9000C` | Caller `252::2` (`wlib+0xfaed8`, C++ `0x140b8722d`) | Client accomplishment action / query (`u64` target/player `InstanceID`, `WState` parameters). |
| `0x84DA0004` | S→C | `reads=[6 (WState, 0), 6 (WState, 0)]` | Target `252::5` (`wlib+0xfc00c`) | Synchronizes active and completed Accomplishment (Deed) tables (2 `WState` objects). |
| `0x84DA0006` | S→C | `reads=[]` | Target `252::6` (`wlib+0xfbf88`) | Signals Accomplishment directory / state reset or initialization complete. |
| `0x84DA000A` | S→C | `reads=[7 (u32)]` | Target `252::7` (`wlib+0xfbdb8`) | Grants / updates single Accomplishment by `DataID` (`u32`). |

---

#### 2.3.4 Vendors, Buyback, Repair & Barter (`0x8536`, `0x84FE`, `0x850B`, `0x8793`) <a id="dirty-vendors-barter"></a>

1. **Vendor Buy / Sell / SellAll (`RPC Class 0x0536` / 1334, `0x8536xxxx`, paired with `WLib Class 0x01CE` / 462 `VendorSystem`):**
   - C++ UI handler: `0x140fa2070`.
   - Properties on `Class 462` (`0x01CE`): `Vendor_ItemList` (`0x1000013C`), `Vendor_BuysItems` (`0x1000129E`), `Antispam_VendorRequest` (`0x10000CF5`), `Vendor_SellAll_VendorID` (`0x10000306`), `Vendor_SellAll_ItemsToSell` (`0x100013AC`), `Vendor_SellAll_ItemToSell` (`0x1000013A`), `Inventory_SellLockedByID` (`0x1000056D`), `Item_ForceNotSellable` (`0x10003D3E`).
   - **C→S RPCs:**
     - `0x85360001`: `writes=[2 (u32), 2 (u32), 3 (InstanceID)]`, `send=0x00D9000C` (callers `462::15`, `462::1` at `wlib+0x1dcdf8`–`0x1de8d8`, C++ `0x140aa123b`) — **Buy Item (`CanPlayerBuyItemLatent`)**: `u32` quantity (`CounterBox_CurrentCount` `0x100002BD`), `u32` item `DataID` (or vendor slot index), `u64` vendor `InstanceID`.
     - `0x85360002`: `writes=[2 (u32), 3 (InstanceID)]`, `send=0x00D9000C` (callers `462::17`, `462::2` at `wlib+0x1dc7fc`–`0x1de354`, C++ `0x140a99ade`) — **Buy / Fill Stack (`CanPlayerFillStackLatent`)**: `u32` item `DataID`, `u64` vendor `InstanceID`.
     - `0x85360007`: `writes=[3 (InstanceID)]`, `send=0x00D9000D` (caller `1757::2` at `wlib+0x3e62f4`) — Admin / debug dump vendor (`u64` vendor `InstanceID`).
     - `0x8536000A`: `writes=[1 (WState), 3 (InstanceID)]`, `send=0x00D9000C` (caller `462::3` at `wlib+0x1ddd9c`, C++ `0x140a9ff7c`) — **Sell All / Sell Items**: `WState` array (`Vendor_SellAll_ItemsToSell` `0x100013AC` containing `Vendor_SellAll_ItemToSell` `0x1000013A` entries), `u64` vendor `InstanceID` (`Vendor_SellAll_VendorID` `0x10000306`).
     - `0x8536000E`: `writes=[2 (u32), 3 (InstanceID)]`, `send=0x00D9000C` (callers `462::16`, `462::5` at `wlib+0x1dcaf4`–`0x1dd718`, C++ `0x140a99400`) — **Buy Item Variant (by Currency / PaperItem / Perk)**: `u32` item `DataID`, `u64` vendor `InstanceID`.
     - `0x85360016`: `writes=[2 (u32), 3 (InstanceID)]`, `send=0x00D9000C` (callers `462::18`, `462::7` at `wlib+0x1dc518`–`0x1dd140`, C++ `0x140aa0e20`) — **Sell Single Item / Buyback**: `u32` parameter/index, `u64` target `InstanceID`.
   - **S→C RPC:**
     - `0x85360004`: `reads=[8 (InstanceID)]` (target `462::75` at `wlib+0x1d9300`) — Server acknowledgment / refresh for vendor `InstanceID` (`u64`).

2. **Buyback & Extended NPC/Service Operations (`RPC Class 0x04FE` / 1278, `0x84FExxxx`, paired with `Class 0x019D` / 413 & `Class 0x0123` / 291):**
   - `0x84FE008E` (C→S, `writes=[2 (u32), 2 (u32)]`, `send=0x00D9000C`, caller `413::4` at `wlib+0x1c7560`, which reads `0x1000056B Buyback_BuybackID` and `0x10000399 Buyback_BuybackListItem`): **Buyback Item Request** (`u32` quantity, `u32` `Buyback_BuybackID`).
   - `0x84FE00D5` (C→S, `writes=[3 (InstanceID)]`, `send=0x00D9000C`, caller `1106::5` at `wlib+0x31df14`, paired with `Vendor_Inventory` `0x10000059`).
   - `0x84FE011C` (S→C, `reads=[6 (WState, 0)]`, target `1106::2` at `wlib+0x31dfd8`, updates `Vendor_Inventory` `0x10000059`).

3. **Item Repair (`RPC Class 0x050B` / 1291, `0x850Bxxxx`, paired with `WLib Class 0x0171` / 369 `RepairSystem`):**
   - Properties: `Antispam_RepairItemRequest` (`0x10000EED`), `Vendor_DiscountList` (`0x100006D6`), `Vendor_SellModifier` (`0x10001491`).
   - **C→S RPCs:**
     - `0x850B0002`: `writes=[3 (InstanceID), 3 (InstanceID)]`, `send=0x00D9000C` (caller `369::1` at `wlib+0x191ec8`, C++ `0x140b8d88b`) — **Repair Single Item**: `u64` item `InstanceID`, `u64` vendor `InstanceID`.
     - `0x850B000B`: `writes=[3 (InstanceID), 1 (WState)]`, `send=0x00D9000C` (caller `369::2` at `wlib+0x191adc`, C++ `0x140b8f01c`) — **Repair Multiple / All Items**: `u64` vendor `InstanceID`, `WState` array of item `InstanceID`s.
   - **S→C RPCs:**
     - `0x850B0004`: `reads=[7 (u32), 8 (InstanceID)]` (target `369::13` at `wlib+0x1915cc`) — Single item repair result (`u32` status/error code, `u64` item `InstanceID`).
     - `0x850B0005`: `reads=[]` (target `369::11` at `wlib+0x191848`) — Repair all succeeded notification.
     - `0x850B0006`: `reads=[]` (target `369::12` at `wlib+0x19175c`) — Repair state/UI refresh notification.
     - `0x850B000A`: `reads=[7 (u32), 8 (InstanceID)]` (target `369::10` at `wlib+0x191934`) — Batch repair result (`u32` status/error code, `u64` vendor `InstanceID`).

4. **Barter (`RPC Class 0x0793` / 1939, `0x8793xxxx`, paired with `WLib Class 0x0795` / 1941 `BarterSystem`):**
   - C++ UI handler: `0x140fa4ab0`.
   - Properties on `Class 1941` (`0x0795`): `Barter_IsBarterItem` (`0x10001795`), `Barter_ProfileArray` (`0x100017F9`), `Barter_Profile` (`0x100017FA`), `Antispam_BarterRequest` (`0x10006959`), `CounterBox_CurrentCount` (`0x100002BD`).
   - **C→S RPC:**
     - `0x87930006`: `writes=[1 (WState), 3 (InstanceID)]`, `send=0x00D9000C` (callers `1941::2` at `wlib+0x40ec80` and `1941::5` at `wlib+0x40e4fc`, C++ `0x140b61c28` and `0x140b61d18`) — **Execute Barter Exchange**:
       1. `WState` (`0x008E0001`): Barter transaction descriptor (profile `DataID`, reward index/item `DataID`, quantity `CounterBox_CurrentCount` `0x100002BD`).
       2. `InstanceID` (`0x008E0003`): Barter NPC or item `InstanceID` (`u64`).
   - **S→C RPC:**
     - `0x87930002`: `reads=[8 (InstanceID)]` (target `1940::1` at `wlib+0x40daec` -> `1941::1`) — Server acknowledgment / completion of barter exchange with `npc_iid` (`u64`).

---

#### 2.3.5 Corpse/Chest Container Loot, Roll/Pass, Master Loot & Pending Loot (`0x8513`, `0x8512`, `0x84EC`) <a id="dirty-loot"></a>

1. **Container / Corpse / Chest Open & Close (`RPC Class 0x0513` / 1299, `0x8513xxxx`, paired with `WLib Class 0x092E` / 2350 `CMEOInventorySystem`):**
   - C++ Corpse/Chest Loot UI (`0x140e0d410`–`0x140e122e0`) invokes `Class 2350` (`0x092E`) methods (`0x140b4f910`–`0x140b90ce1`).
   - **C→S RPCs (`send=0x00D9000C`):**
     - `0x85130016`: `writes=[3 (InstanceID)]` (caller `2350::10` at `wlib+0x4ca93c`, C++ `0x140b4f910`) — **Stop Viewing / Close Container** (`u64` container `InstanceID`).
     - `0x8513002D`: `writes=[3 (InstanceID)]` (caller `2350::6` at `wlib+0x4cadec`, C++ `0x140b4fa9f`) — **Start Viewing / Open Container** (`u64` container `InstanceID`).
   - **S→C RPCs:**
     - `0x85130004`: `reads=[7 (u32), 7 (u32), 8 (InstanceID)]` (target `2350::11` `CMEOInventorySystem::RecvCEvt_HandleContainerStartViewingResult` at `wlib+0x4ca728`) — Result of opening container (`u32` error/result code, `u32` flags/slot count, `u64` container `InstanceID`; checks `Inventory_DisableCloseOnEmpty` `0x10001514`).
     - `0x85130014`: `reads=[7 (u32), 7 (u32), 8 (InstanceID)]` (target `2350::12` `CMEOInventorySystem::RecvCEvt_HandleContainerStopViewingResult` at `wlib+0x4ca3dc`) — Result of closing container (`u32` error/result code, `u32` flags, `u64` container `InstanceID`).

2. **Loot Permissions, Roll/Pass, Master Loot & Pending Loot (`RPC Class 0x0512` / 1298, `0x8512xxxx`, paired with `WLib Class 0x0112` / 274 `LootSystem`):**
   - Properties on `Class 274` (`0x0112`):
     - `Loot_ShouldSparkle` (`0x10000A33`)
     - `Loot_UI_MasterLootDialog_AssigneeIID` (`0x100016FB`), `Loot_UI_MasterLootDialog_ItemIID` (`0x100016FC`), `Loot_UI_MasterLootDialog_ItemDID` (`0x100016FD`)
     - `UI_Inventory_PendingLoot_Array` (`0x10004E68`), `UI_Inventory_PendingLoot_ItemDID` (`0x10004E64`), `UI_Inventory_PendingLoot_ItemQuantity` (`0x10004E61`), `UI_Inventory_PendingLoot_ItemEarnTimestamp` (`0x10004E87`).
   - **C→S RPCs (`send=0x00D9000C`):**
     - `0x85120014`: `writes=[3 (InstanceID), 3 (InstanceID)]` (caller `274::1` at `wlib+0x127f00`, C++ `0x140b647ff`) — **Loot Item from Corpse/Container** (`u64` item `InstanceID`, `u64` container `InstanceID`).
     - `0x8512001B`: `writes=[2 (u32), 3 (InstanceID)]` (caller `274::9` at `wlib+0x1279b4`, C++ `0x140b90d71`) — **Roll / Greed / Need / Pass on Loot Item** (`u32` decision enum: Roll/Need/Greed/Pass, `u64` loot item/rule `InstanceID`).
     - `0x85120025`: `writes=[3 (InstanceID)]` (caller `274::11` at `wlib+0x1277ec`, C++ `0x140b8ebfb`) — **Loot All / Claim Pending Loot Item** (`u64` container or pending loot item `InstanceID`).
     - `0x85120029`: `writes=[3 (InstanceID), 3 (InstanceID)]` (caller `274::14` at `wlib+0x1274ac`, reads `0x100016FC` & `0x100016FB`) — **Master Looter Assign Item** (`u64` `Loot_UI_MasterLootDialog_AssigneeIID`, `u64` `Loot_UI_MasterLootDialog_ItemIID`).
     - `0x8512003F`: `writes=[]` (caller `274::26` at `wlib+0x125a3c`, C++ `0x140b8ec9c`) — **Claim All Pending Loot / Clear Pending Loot**.
     - `0x85120041`: `writes=[2 (u32), 2 (u32), 2 (u32)]` (caller `274::27` at `wlib+0x1258e4`, C++ `0x140b921a5`) — **Pending Loot Item Action A** (`u32`, `u32`, `u32`: e.g. take/move pending loot entry by `DataID`/index).
     - `0x85120042`: `writes=[2 (u32), 2 (u32), 2 (u32)]` (caller `274::21` at `wlib+0x125b78`, C++ `0x140b92256`) — **Pending Loot Item Action B** (`u32`, `u32`, `u32`: e.g. destroy/discard pending loot entry).
     - `0x85120053`: `writes=[]` (caller `274::17` at `wlib+0x125738`, C++ `0x140b922b5`) — **Query / Refresh Pending Loot List**.
   - **S→C RPCs:**
     - `0x85120007`: `reads=[8 (InstanceID)]` (target `274::2` at `wlib+0x127de4`) — Enables corpse sparkle (`Loot_ShouldSparkle` `0x10000A33` = true) on `corpse_iid` (`u64`).
     - `0x85120009`: `reads=[6 (WState, 0), 7 (u32), 8 (InstanceID)]` (target `274::6` at `wlib+0x127b10`) — Opens Master Loot / Fellowship Loot prompt (`WState` candidate/item info, `u32` `DataID`/rule, `u64` item `InstanceID`).
     - `0x85120019`: `reads=[6 (WState, 0), 8 (InstanceID)]` (target `274::5` at `wlib+0x127ba4`) — Updates container/corpse loot item list (`WState` item array, `u64` container `InstanceID`).
     - `0x8512001D`: `reads=[7 (u32), 9 (f32), 10 (f64/u64), 6 (WState, 0), 7 (u32), 8 (InstanceID)]` (target `274::8` at `wlib+0x127a4c`) — Starts a Fellowship Roll/Pass loot timer dialog (`u32` rule/type, `f32` duration seconds, `u64` timestamp/ID, `WState` item property collection, `u32` item `DataID`, `u64` loot item `InstanceID`).
     - `0x8512001E`: `reads=[8 (InstanceID)]` (target `274::3` at `wlib+0x127cf0`) — Disables corpse sparkle (`Loot_ShouldSparkle` `0x10000A33` = false) on `corpse_iid` (`u64`).
     - `0x8512002B`: `reads=[6 (WState, 0), 7 (u32), 8 (InstanceID), 8 (InstanceID)]` (target `274::13` at `wlib+0x127670`) — Master Loot assignment notification (`WState`, `u32` item `DataID`, `u64` assignee `InstanceID`, `u64` item `InstanceID`).
     - `0x8512003D`: `reads=[6 (WState, 0)]` (target `274::25` at `wlib+0x125cc0`) — Synchronizes `UI_Inventory_PendingLoot_Array` (`0x10004E68`) (`WState` array of pending loot items).

---

#### 2.3.6 Wallet (`PaperItem`) & Currency (`RPC Class 0x0C0D` / 3085, `0x8C0Dxxxx`) <a id="dirty-wallet"></a>

Paired with **`WLib Class 0x0C0C` (3084 `PaperItemSystem`)** and **`Class 0x0C21` (3105)**.
- **Coin (Copper / Silver / Gold):** Stored directly as an integer property in copper units (`100` copper = `1` silver, `100,000` copper = `1` gold) on the player entity via `Currency_Value` (`0x1000012E`) / `UI_Inventory_Currency` (`0x1000050D`), synchronized via standard property updates.
- **Wallet Currencies / Barter Tokens (`PaperItem`):** Managed by `PaperItemSystem` (`Class 3084`, C++ UI `0x140cff190`–`0x140d04a20`).
  - Properties: `PaperItem_Category` (`0x10003B0F`), `PaperItemControl_PaperItemArray` (`0x10003B2B`), `PaperItem_SmallIcon` (`0x10004798`), `PaperItemService_AssociatedDID` (`0x10005E35`), `PaperItemService_SourceIID` (`0x10005E36`).
- **RPC Methods (`0x8C0Dxxxx`):**

| Opcode | Direction | Signature (`0x008E`) | WLib Handler / Caller | Semantic Meaning |
|---|---|---|---|---|
| `0x8C0D0017` | C→S | `writes=[1 (WState), 2 (u32)]`, `send=0x00D9000C` | Callers `3084::21` (`wlib+0x5c11bc`) & `3709::20` (`wlib+0x6280d8`) | Spend / use / convert a Wallet `PaperItem` (`WState` parameters, `u32` `PaperItem` `DataID`). |
| `0x8C0D0001` | S→C | `reads=[7 (u32), 7 (u32)]` | Target `3084::2` (`wlib+0x5c3070`) | Updates or removes a Wallet `PaperItem` entry (`u32` `PaperItem` `DataID`, `u32` quantity/delta). |
| `0x8C0D0004` | S→C | `reads=[7 (u32), 7 (u32), 7 (u32)]` | Target `3084::3` (`wlib+0x5c2dc8`) | Updates a Wallet `PaperItem` balance with flags (`u32` flag/status, `u32` quantity, `u32` `PaperItem` `DataID`). |
| `0x8C0D0006` | S→C | `reads=[6 (WState, 0)]` | Target `3084::4` (`wlib+0x5c2b90`) | Full Wallet (`PaperItemControl_PaperItemArray` `0x10003B2B`) synchronization (`WState` array). |
| `0x8C0D001C` | S→C | `reads=[7 (u32)]` | Target `3084::22` (`wlib+0x5c1028`) | Result notification (`u32` status/error code) for `0x8C0D0017`. |

---

## 3. Clean-Room Specification Section (Ready to Merge into `docs/specs/official-messages-gameplay.md`) <a id="clean-room-spec"></a>

### 3.1 Overview and Wire Framing <a id="spec-overview"></a>

All P3 gameplay subsystems—**Entity Selection and Use**, **NPC Interaction and Dialogue**, **Quests**, **Deeds (Accomplishments)**, **Vendors, Buyback, Repair, and Barter**, **Corpse/Chest and Fellowship Loot**, and **Wallet (`PaperItem`)**—exchange messages over the official server's entity-targeted Remote Procedure Call (RPC) framing (`BIN-CLIENT`, `BIN-WLIB`).

Every RPC message payload begins with a 32-bit little-endian method identifier (`0x8xxxxxxx`), where:
- **Bit 31** is always `1` (`0x80000000`).
- **Bits 16..30** identify the 15-bit RPC subsystem class (`0x04DA`–`0x0C0D`).
- **Bits 0..15** identify the 16-bit method index within that class.

Following the 32-bit RPC identifier, arguments are serialized sequentially in the order listed in the tables below using five primitive wire types (`BIN-CLIENT`, `BIN-WLIB`):

| Wire Type Token | Size (Bytes) | Byte Order | Description |
|---|---|---|---|
| `u32` / `i32` / `DataID` / `bool` | 4 | Little-endian | 32-bit integer, enumeration code, resource `DataID`, or boolean (`0` = false, non-zero = true). |
| `InstanceID` (`u64`) | 8 | Little-endian | 64-bit entity or item instance identifier. |
| `f32` | 4 | Little-endian | IEEE-754 single-precision floating-point number. |
| `f64` / `u64` | 8 | Little-endian | IEEE-754 double-precision float or 64-bit timestamp/value. |
| `WState` | Variable | Little-endian | Self-describing typed state container (16-bit/32-bit class tag followed by tagged fields, `StringInfo` localization tokens, or `WArray` element lists). |

---

### 3.2 Entity Selection, Targeted Use, and NPC Session Initiation <a id="spec-npc-interaction"></a>

#### 3.2.1 Entity Selection and Use (`0x8535xxxx`, `0x8528xxxx`, `0x8524xxxx`) <a id="spec-entity-use"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x85350002` | C→S | `state: WState` | Synchronizes client entity selection/interaction state with the server. |
| `0x8535001D` | C→S | `use_req: WState` (Class `0x063E`) | Requests interaction/use of a target world entity (`InstanceID` and interaction flags packed in `use_req`). |
| `0x8535003A` | C→S | `action_code: u32` | Sends a discrete selection/usage command code. |
| `0x8535000C` | S→C | `state: WState` | Server response updating entity interaction state. |
| `0x85350027` | S→C | `status: u32`, `target_iid: InstanceID` | Server notification of entity use completion or failure for `target_iid`. |
| `0x85280006` | C→S | `role_action: u32`, `target_iid: InstanceID` | Executes a targeted quest/role action (`QuestDispenser_RoleSuccessText` `0x10000035`) on `target_iid`. |
| `0x8528000E` | S→C | `status: u32`, `action_id: u32`, `target_iid: InstanceID` | Server result for a targeted action on `target_iid`. |
| `0x85240003` | S→C | `action_type: u32`, `dialog_data: WState`, `flags: u32` | Triggers a targeted action confirmation dialog (`Usage_TargetActionWithDialog` `0x100041E6`). |

#### 3.2.2 NPC Action Picker and Service Session (`0x8523xxxx`) <a id="spec-npc-session"></a>

When a player interacts with an NPC that offers multiple quests or services (such as Vendor, Repair, Trainer, or Quest Dispenser), the session is managed via `0x8523xxxx` (`BIN-WLIB`, `DAT-LOGIC`):

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x85230016` | C→S | `npc_iid: InstanceID` | Opens an interaction session with `npc_iid`. |
| `0x85230006` | S→C | `npc_iid: InstanceID`, `flags_a: u32`, `flags_b: u32`, `options: WState` | Opens the NPC Action Picker menu for `npc_iid`, providing available quest scripts (`NPC_ActionPicker_QuestScript` `0x1000018D`) and service scripts (`NPC_ActionPicker_SelectionScript` `0x10000271`). |
| `0x85230008` | C→S | `option_id: u32`, `npc_iid: InstanceID` | Selects action/service `option_id` on `npc_iid`. |
| `0x85230013` | S→C | `service_type: u32`, `npc_iid: InstanceID`, `service_payload: WState` | Opens the selected NPC service panel (e.g. populates `Vendor_Inventory` `0x10000059` or quest list) for `npc_iid`. |
| `0x85230017` | C→S | *(empty)* | Closes the active NPC interaction session. |
| `0x8523001C` | S→C | `status: u32` | Server notification of NPC session closure or error status. |
| `0x8523003B` | S→C | `npc_iid: InstanceID` | Server directive terminating the interaction session with `npc_iid`. |

#### 3.2.3 NPC Dialogue, Quest Dispenser Dialog, and Comments (`0x8522xxxx`, `0x8527xxxx`, `0x88CBxxxx`, `0x84F6xxxx`) <a id="spec-npc-dialogue"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`, `DAT-HIGHRES`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x85220002` | S→C | `dialog_data: WState`, `dispenser_action: u32`, `source_iid: InstanceID` | Opens the Quest Dispenser dialog (`questdisplay` layout `0x22000067`) for `source_iid` with `QuestDispenser_Action` (`0x10000BDC`) and quest offer/reward details in `dialog_data`. |
| `0x85220006` | C→S | `selection_data: WState`, `dispenser_action: u32`, `source_iid: InstanceID` | Submits the player's Quest Dispenser response (Accept `0x100007DE`, Decline `0x100007DF`, Complete/Finish `0x10000CA9`) and optional reward choice (`QuestTreasure_SelectedItem` `0x100000BC` inside `selection_data`) for `source_iid`. |
| `0x85220005` | C→S | `npc_iid: InstanceID` | Requests quest role/dispenser state for `npc_iid`. |
| `0x8522000A` | S→C | `result_code: u32`, `source_iid: InstanceID` | Closes the Quest Dispenser dialog or reports result `result_code` for `source_iid`. |
| `0x85270006` | S→C | `text_payload: WState` | Displays NPC chat/billboard text (`GCParam_DialogText` `0x10000ED2`, `GCParam_DisplayText_IsBillboardText` `0x10000CDF`, `GCParam_DialogText_InitiatorIID` `0x10003C2D`). |
| `0x88CB0003` | C→S | `comment_did: u32`, `npc_iid: InstanceID` | Requests playback of NPC comment `comment_did` (`NPCComment_DIDCommentArray` `0x10002128`) on `npc_iid`. |
| `0x88CB0001` | S→C | `comment_did: u32`, `npc_iid: InstanceID` | Triggers NPC comment `comment_did` (`NPCComment_PickedComment` `0x1000212A`) on `npc_iid`. |
| `0x84F60001` | C→S | `target_iid: InstanceID`, `drama_state: WState` | Sends player interaction/skip input during a drama sequence (`Drama_Performing` `0x1000076E`). |

---

### 3.3 Quests and Deeds (Accomplishments) <a id="spec-quests-deeds"></a>

#### 3.3.1 Quest Lifecycle State Machine <a id="spec-quest-state-machine"></a>

1. **Login Synchronization:** Upon world entry, the server sends `0x8521005C` containing 6 `WState` structures that populate the client's active quest journal, completed quest history, repeat cooldowns, category limiters, quest tracker (`Quest_TrackerArray` `0x10000E6B`), and pending auto-bestow quests (`BIN-WLIB`).
2. **Offer & Bestowal:**
   - **NPC/Item Dispenser:** The server sends `0x85220002` to open the offer dialog; the client accepts by sending `0x85220006` (`BIN-WLIB`, `DAT-HIGHRES`).
   - **Auto-Bestow / Remote Bestow:** The server notifies the client via `0x852100CE` (`quest_did: u32`); the client accepts via `0x8521004B` (`quest_did: u32`) or `0x8521003D` (`quest_did: u32`) (`BIN-WLIB`).
   - **Fellowship Share:** A player shares an active quest via `0x85210013` (`quest_did: u32`). Fellowship members receive `0x85210041` (`status: u32`, `sharer_iid: InstanceID`, `quest_did: u32`) and respond via `0x8521006B` (`accept_code: u32`, `sharer_iid: InstanceID`, `quest_did: u32`) (`BIN-WLIB`).
3. **Objective Progress:** As objectives advance, the server sends `0x85210075` (`quest_did: u32`, `objective_index: u32`, `current_count: u32`), `0x852100DF` (`quest_did: u32`, `objective_index: u32`, `state: u32`), or `0x85210010` (`quest_did: u32`, `objective_index: u32`, `progress_wstate: WState`) (`BIN-WLIB`).
4. **Completion & Rewards:**
   - For NPC turn-in quests, the player opens the NPC (`0x85230016` → `0x85220002`), selects an optional reward (`QuestTreasure_SelectedItem` `0x100000BC`) if required, and sends `0x85220006` (`BIN-WLIB`, `DAT-HIGHRES`).
   - For remote-completable quests (`CompleteButton` `0x100018E7` in `questhistory` `0x22000044`), the client sends `0x85210111` (`quest_did: u32`), and the server replies with `0x85210115` (`quest_did: u32`, `result_code: u32`) and `0x852100B5` (`quest_did: u32`) (`BIN-CLIENT`, `BIN-WLIB`).
5. **Abandonment:** Clicking Abandon (`0x100007EE` in `questhistory` `0x22000044`) sends `0x85210008` (`quest_did: u32`); the server confirms removal with `0x85210052` (`quest_did: u32`) (`BIN-CLIENT`, `BIN-WLIB`).

#### 3.3.2 Quest Wire Messages (`0x8521xxxx`) <a id="spec-quest-messages"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`, `DAT-HIGHRES`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x85210008` | C→S | `quest_did: u32` | Abandon active quest `quest_did`. |
| `0x85210013` | C→S | `quest_did: u32` | Share quest `quest_did` with fellowship. |
| `0x8521002F` | C→S | `quest_did: u32`, `slot_or_op: u32` | Update quest tracker slot/state for `quest_did`. |
| `0x8521003D` | C→S | `quest_did: u32` | Request/advance quest `quest_did`. |
| `0x8521004B` | C→S | `quest_did: u32` | Accept pending auto-bestow quest `quest_did`. |
| `0x85210057` | C→S | `quest_did: u32` | Query map/radar guide locations for `quest_did`. |
| `0x8521006B` | C→S | `response: u32`, `sharer_iid: InstanceID`, `quest_did: u32` | Accept or decline fellowship quest share from `sharer_iid` for `quest_did`. |
| `0x85210082` | C→S | *(empty)* | Request quest state synchronization. |
| `0x852100B1` | C→S | `quest_did: u32`, `source_iid: InstanceID` | Interact with quest dispenser `source_iid` for `quest_did`. |
| `0x852100D6` | C→S | `quest_did: u32`, `tracker_op: u32` | Add/remove/lock `quest_did` in the quest tracker. |
| `0x852100F7` | C→S / S→C | `entity_iid: InstanceID` | Query/sync quest category limiter for `entity_iid`. |
| `0x852100F8` | C→S / S→C | `entity_iid: InstanceID` | Reset/update quest category limiter for `entity_iid`. |
| `0x852100FA` | C→S / S→C | `entity_iid: InstanceID` | Query/sync `QuestCategoryLimiter_ResetTime` (`0x10003B0B`) for `entity_iid`. |
| `0x85210111` | C→S | `quest_did: u32` | Complete remote-completable quest `quest_did`. |
| `0x8521000E` | S→C | `quest_did: u32` | Quest notification for `quest_did`. |
| `0x85210010` | S→C | `quest_did: u32`, `role_index: u32`, `progress: WState` | Detailed objective/role progress update for `quest_did`. |
| `0x8521001F` | S→C | `quest_did: u32`, `status_code: u32` | Quest state/error update for `quest_did`. |
| `0x85210020` | S→C | `quest_data: WState`, `quest_did: u32` | Updates quest record `quest_data` for `quest_did`. |
| `0x85210027` | S→C | `quest_did: u32`, `counter_val: u32` | Updates counter/timer value for `quest_did`. |
| `0x85210041` | S→C | `status: u32`, `sharer_iid: InstanceID`, `quest_did: u32` | Prompts player with a fellowship quest share offer from `sharer_iid`. |
| `0x85210047` | S→C | `objectives: WState`, `quest_did: u32` | Updates objective list state for `quest_did`. |
| `0x85210051` | S→C | `guide_locations: WState` | Delivers quest radar/map guide coordinates. |
| `0x85210052` | S→C | `quest_did: u32` | Removes abandoned/canceled quest `quest_did` from journal. |
| `0x85210053` | S→C | `quest_did: u32`, `cooldown_info: u32` | Updates repeat cooldown state for `quest_did`. |
| `0x8521005C` | S→C | `active: WState`, `completed: WState`, `cooldowns: WState`, `limiters: WState`, `tracker: WState`, `autobestow: WState` | Full 6-table quest journal synchronization. |
| `0x8521005E` | S→C | `quest_state: WState`, `quest_did: u32` | Bestows or updates active quest `quest_did`. |
| `0x85210063` | S→C | `quest_list: WState` | Batch quest history/journal update. |
| `0x85210067` | S→C | `guide_a: WState`, `guide_b: WState` | Updates quest guide location tables. |
| `0x85210075` | S→C | `quest_did: u32`, `objective_index: u32`, `count: u32` | Updates numeric progress count for `objective_index` of `quest_did`. |
| `0x85210094` | S→C | `quest_did: u32`, `result_code: u32` | Fellowship quest share result for `quest_did`. |
| `0x8521009C` | S→C | `quest_did: u32`, `error_code: u32` | Quest error/status notification. |
| `0x852100AB` | S→C | `param_a: u32`, `override_data: WState`, `quest_did: u32` | Objective progress override update. |
| `0x852100B5` | S→C | `quest_did: u32` | Marks `quest_did` as completed. |
| `0x852100BD` | S→C | `result_code: u32`, `quest_did: u32`, `member_iid: InstanceID`, `flags: u32` | Reports fellowship member `member_iid`'s quest share response for `quest_did`. |
| `0x852100C1` | S→C | `quest_did: u32`, `duration_sec: u32` | Updates timer for `quest_did`. |
| `0x852100CA` | S→C | `quest_did: u32`, `flags: u32` | Updates state flags for `quest_did`. |
| `0x852100CE` | S→C | `quest_did: u32` | Notifies client that auto-bestow quest `quest_did` is available. |
| `0x852100CF` | S→C | `quest_data: WState`, `quest_did: u32` | Synchronizes quest state record for `quest_did`. |
| `0x852100D2` | S→C | `quest_did: u32`, `timestamp: f64` | Updates expiration/reset timestamp for `quest_did`. |
| `0x852100D5` | S→C | `locked: u32`, `tracker_array: WState` | Synchronizes `Quest_TrackerArrayLocked` (`0x10000291`) and `Quest_TrackerArray` (`0x10000E6B`). |
| `0x852100DF` | S→C | `quest_did: u32`, `objective_index: u32`, `state: u32` | Updates state of `objective_index` in `quest_did`. |
| `0x852100FC` | S→C | `quest_did: u32` | Quest state notification for `quest_did`. |
| `0x85210103` | S→C | `quest_did: u32`, `medal: u32`, `points: u32` | Updates `Quest_Medal` (`0x100056BD`) and `Quest_Points` (`0x100056B1`) for `quest_did`. |
| `0x85210106` | S→C | `quest_did: u32`, `slot: u32` | Confirms quest tracker slot assignment for `quest_did`. |
| `0x85210115` | S→C | `quest_did: u32`, `result_code: u32` | Result of remote quest completion (`0x85210111`). |
| `0x85210118` | S→C | `quest_did: u32` | Quest event trigger for `quest_did`. |
| `0x85210126` | S→C | `quest_did: u32`, `status_code: u32` | Bestow/abandon status update for `quest_did`. |

#### 3.3.3 Deeds / Accomplishments (`0x84DAxxxx`) <a id="spec-deeds"></a>

Deeds are represented as Accomplishment records that share objective tracking with `0x8521xxxx` and use `0x84DAxxxx` for Accomplishment table synchronization (`BIN-WLIB`, `DAT-LOGIC`):

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x84DA0007` | C→S | `target_iid: InstanceID`, `params: WState` | Client accomplishment query or action for `target_iid`. |
| `0x84DA0004` | S→C | `active_deeds: WState`, `completed_deeds: WState` | Synchronizes active and completed deed tables. |
| `0x84DA0006` | S→C | *(empty)* | Signals completion of accomplishment directory initialization/reset. |
| `0x84DA000A` | S→C | `deed_did: u32` | Grants or updates accomplishment `deed_did`. |

---

### 3.4 Vendors, Buyback, Item Repair, and Barter <a id="spec-vendors-barter"></a>

#### 3.4.1 Vendor Buy, Sell, and Sell-All (`0x8536xxxx`, `0x84FE008E`) <a id="spec-vendors"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x85360001` | C→S | `quantity: u32`, `item_did: u32`, `vendor_iid: InstanceID` | Purchases `quantity` (`CounterBox_CurrentCount` `0x100002BD`) of `item_did` from `vendor_iid`. |
| `0x85360002` | C→S | `item_did: u32`, `vendor_iid: InstanceID` | Purchases enough of `item_did` from `vendor_iid` to fill an existing partial stack in the player's inventory. |
| `0x8536000E` | C→S | `item_did: u32`, `vendor_iid: InstanceID` | Purchases `item_did` from `vendor_iid` using alternate/perk currency rules. |
| `0x8536000A` | C→S | `items_to_sell: WState`, `vendor_iid: InstanceID` | Sells one or more inventory items (`Vendor_SellAll_ItemsToSell` `0x100013AC` containing `Vendor_SellAll_ItemToSell` `0x1000013A` entries, excluding items with `Inventory_SellLockedByID` `0x1000056D` or `Item_ForceNotSellable` `0x10003D3E`) to `vendor_iid` (`Vendor_SellAll_VendorID` `0x10000306`). |
| `0x85360016` | C→S | `slot_or_param: u32`, `vendor_iid: InstanceID` | Single-item vendor transaction on `vendor_iid`. |
| `0x85360007` | C→S | `vendor_iid: InstanceID` | Queries/dumps vendor state for `vendor_iid`. |
| `0x85360004` | S→C | `vendor_iid: InstanceID` | Server acknowledgment / inventory refresh for `vendor_iid`. |
| `0x84FE008E` | C→S | `quantity: u32`, `buyback_id: u32` | Repurchases `quantity` of a previously sold item identified by `Buyback_BuybackID` (`0x1000056B`). |

#### 3.4.2 Item Repair (`0x850Bxxxx`) <a id="spec-repair"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x850B0002` | C→S | `item_iid: InstanceID`, `vendor_iid: InstanceID` | Requests repair of a single item `item_iid` at vendor `vendor_iid`. |
| `0x850B000B` | C→S | `vendor_iid: InstanceID`, `item_list: WState` | Requests batch/all repair of items in `item_list` at vendor `vendor_iid`. |
| `0x850B0004` | S→C | `result_code: u32`, `item_iid: InstanceID` | Reports repair result `result_code` for single item `item_iid`. |
| `0x850B0005` | S→C | *(empty)* | Confirms completion of a repair-all operation. |
| `0x850B0006` | S→C | *(empty)* | Triggers client repair UI refresh. |
| `0x850B000A` | S→C | `result_code: u32`, `vendor_iid: InstanceID` | Reports batch repair result `result_code` at `vendor_iid`. |

#### 3.4.3 Barter Exchange (`0x8793xxxx`) <a id="spec-barter"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x87930006` | C→S | `barter_req: WState`, `vendor_iid: InstanceID` | Executes a barter exchange described by `barter_req` (`Barter_Profile` `0x100017FA`, item selection, and `CounterBox_CurrentCount` `0x100002BD`) with `vendor_iid`. |
| `0x87930002` | S→C | `vendor_iid: InstanceID` | Server confirmation that the barter exchange with `vendor_iid` completed. |

---

### 3.5 Corpse/Chest Container Loot, Fellowship Roll/Master Loot, and Pending Loot <a id="spec-loot"></a>

#### 3.5.1 Corpse and Chest Viewing Lifecycle (`0x8513xxxx`) <a id="spec-container-loot"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x8513002D` | C→S | `container_iid: InstanceID` | Requests to open and view loot container/corpse `container_iid`. |
| `0x85130004` | S→C | `result_code: u32`, `flags: u32`, `container_iid: InstanceID` | Reports result of opening `container_iid`; on success (`result_code == 0`), the client opens the container loot window (honoring `Inventory_DisableCloseOnEmpty` `0x10001514`). |
| `0x85130016` | C→S | `container_iid: InstanceID` | Notifies server that the player closed container/corpse `container_iid`. |
| `0x85130014` | S→C | `result_code: u32`, `flags: u32`, `container_iid: InstanceID` | Confirms closure of `container_iid`. |

#### 3.5.2 Loot Acquisition, Roll/Pass, Master Loot, and Pending Loot (`0x8512xxxx`) <a id="spec-fellowship-loot"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x85120007` | S→C | `corpse_iid: InstanceID` | Enables lootable sparkle VFX (`Loot_ShouldSparkle` `0x10000A33` = true) on `corpse_iid`. |
| `0x8512001E` | S→C | `corpse_iid: InstanceID` | Disables lootable sparkle VFX (`Loot_ShouldSparkle` `0x10000A33` = false) on `corpse_iid`. |
| `0x85120019` | S→C | `loot_items: WState`, `container_iid: InstanceID` | Populates or updates the lootable item list (`loot_items`) inside `container_iid`. |
| `0x85120014` | C→S | `item_iid: InstanceID`, `container_iid: InstanceID` | Takes single loot item `item_iid` from `container_iid`. |
| `0x85120025` | C→S | `target_iid: InstanceID` | Takes all loot from container `target_iid` (or claims a single loot entry). |
| `0x8512001D` | S→C | `rule_type: u32`, `timeout_sec: f32`, `roll_id: f64`, `item_props: WState`, `item_did: u32`, `item_iid: InstanceID` | Opens a fellowship Roll/Need/Greed/Pass timer dialog (`timeout_sec` seconds, `rule_type` from `FellowshipLootType` `0x2300006B`) for `item_iid` (`item_did`). |
| `0x8512001B` | C→S | `vote_choice: u32`, `item_iid: InstanceID` | Submits player's Roll/Need/Greed/Pass choice (`1` = Greed, `2` = Need, `3` = Pass, `4` = Roll) for `item_iid`. |
| `0x85120009` | S→C | `candidates: WState`, `item_did: u32`, `item_iid: InstanceID` | Opens the Master Looter assignment dialog for `item_iid` (`Loot_UI_MasterLootDialog_ItemDID` `0x100016FD`). |
| `0x85120029` | C→S | `assignee_iid: InstanceID`, `item_iid: InstanceID` | Master Looter assigns `item_iid` (`Loot_UI_MasterLootDialog_ItemIID` `0x100016FC`) to fellowship member `assignee_iid` (`Loot_UI_MasterLootDialog_AssigneeIID` `0x100016FB`). |
| `0x8512002B` | S→C | `item_info: WState`, `item_did: u32`, `assignee_iid: InstanceID`, `item_iid: InstanceID` | Notifies fellowship of Master Loot assignment of `item_iid` to `assignee_iid`. |
| `0x8512003D` | S→C | `pending_array: WState` | Synchronizes the player's overflow `UI_Inventory_PendingLoot_Array` (`0x10004E68`, `WArray` of `Class 0x04CD` entries). |
| `0x8512003F` | C→S | *(empty)* | Claims/clears all items in the Pending Loot list. |
| `0x85120041` | C→S | `earn_timestamp: u32`, `quantity: u32`, `item_did: u32` | Claims/moves a specific Pending Loot entry (`UI_Inventory_PendingLoot_ItemEarnTimestamp` `0x10004E87`, `UI_Inventory_PendingLoot_ItemQuantity` `0x10004E61`, `UI_Inventory_PendingLoot_ItemDID` `0x10004E64`) into inventory. |
| `0x85120042` | C→S | `earn_timestamp: u32`, `quantity: u32`, `item_did: u32` | Discards/destroys a specific Pending Loot entry (`UI_Inventory_PendingLoot_ItemEarnTimestamp` `0x10004E87`, `UI_Inventory_PendingLoot_ItemQuantity` `0x10004E61`, `UI_Inventory_PendingLoot_ItemDID` `0x10004E64`). |
| `0x85120053` | C→S | *(empty)* | Requests a refresh of the Pending Loot list. |

---

### 3.6 Wallet (`PaperItem`) and Coin Currency <a id="spec-wallet"></a>

1. **Standard Coin (`Currency_Value` `0x1000012E` / `UI_Inventory_Currency` `0x1000050D`):**
   - Represented as a 32-bit/64-bit integer count of **Copper** coins (`1` Silver = `100` Copper; `1` Gold = `1,000` Silver = `100,000` Copper) synchronized via entity property updates (`DAT-LOGIC`, `BIN-CLIENT`).
2. **Wallet Tokens (`PaperItem`, `0x8C0Dxxxx`):**
   - Account-bound and character-bound barter currencies, festival tokens, shards, and marks are stored as `PaperItem` entries (`PaperItemControl_PaperItemArray` `0x10003B2B`, categorized by `PaperItem_Category` `0x10003B0F`) (`BIN-WLIB`, `DAT-LOGIC`):

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x8C0D0006` | S→C | `wallet_array: WState` | Synchronizes the full `PaperItemControl_PaperItemArray` (`0x10003B2B`). |
| `0x8C0D0001` | S→C | `paper_item_did: u32`, `quantity: u32` | Updates or removes a Wallet token (`paper_item_did`, new/delta `quantity`). |
| `0x8C0D0004` | S→C | `flags: u32`, `quantity: u32`, `paper_item_did: u32` | Updates Wallet token `paper_item_did` to `quantity` with display `flags`. |
| `0x8C0D0017` | C→S | `use_params: WState`, `paper_item_did: u32` | Uses, converts, or spends Wallet token `paper_item_did` with `use_params`. |
| `0x8C0D001C` | S→C | `result_code: u32` | Server result code for `0x8C0D0017`. |

---

### 3.7 Testable Invariants and Synthetic Test Vectors <a id="spec-test-vectors"></a>

#### 3.7.1 Invariants <a id="spec-invariants"></a>

1. **Opcode Prefix Invariant:** Every P3 gameplay RPC payload starts with a 4-byte little-endian `u32` whose high bit is `1` (`(opcode & 0x80000000) != 0`).
2. **Antispam Property Cooldowns:** Before transmitting C→S requests for Vendor (`0x8536xxxx`), Repair (`0x850Bxxxx`), or Barter (`0x87930006`), the client checks the corresponding timestamp property (`Antispam_VendorRequest` `0x10000CF5`, `Antispam_RepairItemRequest` `0x10000EED`, `Antispam_BarterRequest` `0x10006959`) (`BIN-WLIB`, `DAT-LOGIC`).
3. **Sell-Lock Protection:** `0x8536000A` (Sell All) must exclude any inventory item whose `Inventory_SellLockedByID` (`0x1000056D`) or `Item_ForceNotSellable` (`0x10003D3E`) property is set (`BIN-WLIB`, `DAT-LOGIC`).

#### 3.7.2 Synthetic Test Vectors <a id="spec-vectors"></a>

1. **Abandon Quest (`0x85210008`, C→S):**
   - Input: `quest_did = 0x700003E8` (`1879049192`)
   - Wire bytes (8 bytes, hex): `08 00 21 85 e8 03 00 70`
2. **Open Corpse/Chest Container (`0x8513002D`, C→S):**
   - Input: `container_iid = 0x0200000000001234`
   - Wire bytes (12 bytes, hex): `2d 00 13 85 34 12 00 00 00 00 00 02`
3. **Loot Single Item from Container (`0x85120014`, C→S):**
   - Input: `item_iid = 0x0300000000005678`, `container_iid = 0x0200000000001234`
   - Wire bytes (20 bytes, hex): `14 00 12 85 78 56 00 00 00 00 00 03 34 12 00 00 00 00 00 02`
4. **Vendor Buy Item (`0x85360001`, C→S):**
   - Input: `quantity = 5`, `item_did = 0x70001000`, `vendor_iid = 0x010000000000ABCD`
   - Wire bytes (20 bytes, hex): `01 00 36 85 05 00 00 00 00 10 00 70 cd ab 00 00 00 00 00 01`
5. **Wallet Token Balance Update (`0x8C0D0004`, S→C):**
   - Input: `flags = 1`, `quantity = 250` (`0x000000FA`), `paper_item_did = 0x7002A1B0`
   - Wire bytes (16 bytes, hex): `04 00 0d 8c 01 00 00 00 fa 00 00 00 b0 a1 02 70`

---

### 3.8 Inner `WState` Wire Layouts (`0x008E0001` / `0x008E0006`) <a id="spec-wstate-layouts"></a>

Provenance: `BIN-CLIENT` (`0x140363a00`–`0x140368460`, `0x140837e60`, `0x140a4a640`, `0x140e5ca50`), `BIN-WLIB` (`wlib+0x060d18`, `wlib+0x060e38`, `wlib+0x0ef250`–`0x0ef65c`, `wlib+0x137f50`, `wlib+0x1519a8`), `DAT-LOGIC`.

#### 3.8.1 Core `WState` Envelope and Container Primitives <a id="spec-wstate-envelope"></a>

Every `WState` argument serialized via `0x008E0001` (`0x14039fc60` → `0x140367d20` → `0x140363a00`) or deserialized via `0x008E0006` (`0x1403b4070` → `0x140368460` → `0x140363b80`) begins with a 4-byte little-endian object reference/header word (`0xFFFFFFFF` indicates a null object reference; any other value is followed by a 2-byte little-endian `class_id: u16` at offset `+0x04`):

| `class_id` (`u16`) | Kind | Serializer (`BIN-CLIENT`) | Wire Body Layout Following `class_id: u16` |
|---|---|---|---|
| `0x0011` (`WIntSet` / `BitSet`) | Native C++ | `0x140363520` | `count: u32` followed by `count × u32` little-endian integer elements (`DataID`s or IDs). |
| `0x0023` (`WHash`) | Native C++ | `0x140367e90` | `count: u32` followed by `count` entries, each serialized as `key: u32`, `val_tag: u32`, `flag: u8`, and (when `val_tag != 0xFFFFFFFF`) a recursively serialized `WState` value. |
| `0x0027` / `0x00A6` (`PropertyCollection`) | Native C++ | Property serializer | Property set mapping 32-bit `PropertyID` (`0x1000xxxx`) keys to schema-typed values (`u32`, `u64`, `f32`, `f64`, `StringInfo`, or nested `WArray`). |
| `0x006F` / `0x00B6` (`WArray`) | Native C++ | `0x140837e60` (`0x1408365b0`) | `count: u32` followed by `count` typed elements (either primitive values or nested `WState` objects). |
| `0x00C7` (`StringInfo`) | Native C++ | StringInfo serializer | Localized string token (`table_did: u32`, `token_id: u32`, and variable substitution map). |
| WLib Script Class (descriptor bit 28 set) | WLib Bytecode Class | `0x140363aea`–`0x140365380` | Writes `slot_count × 8` bytes of raw 64-bit slot cells (`[value_u32, type_tag_u32]`), followed by recursive `WState` serialization of any slot whose type tag is an object reference (`0x140365380`). |

#### 3.8.2 `0x8521005C` Full Quest Journal Synchronization (6 `WState` Arguments) <a id="spec-wstate-quest-sync"></a>

When `0x8521005C` (`S→C`) is unpacked (`287::62` at `wlib+0x13ffe0` → `1753::99` at `wlib+0x137f50`), the 6 `WState` arguments populate `Class 0x06D9` (`1753`, `QuestManager` state) in the following exact wire order (`BIN-WLIB`):

| Arg # | Target Slot in `Class 0x06D9` (`1753`) | Container `class_id` | Key → Value / Element Layout |
|---|---|---|---|
| 1 | `this.slot[6]` (`active_quests`) | `WHash` (`0x0023`) | Maps active quest `DataID` (`u32`) → `WLib Class 0x057D` (`1405`, active quest state record containing objective progress and role states). |
| 2 | `this.slot[10]` (`active_accomplishments`) | `WHash` (`0x0023`) | Maps active accomplishment/deed `DataID` (`u32`) → `WLib Class 0x057D` (`1405`). |
| 3 | `arg -4` → `this.slot[10]` (`completed_quests`) | `WIntSet` (`0x0011`) | `count: u32` followed by `count × u32` completed quest `DataID`s (iterated via `0x0011000E` count and `0x00110004` element getter; each `DataID` is wrapped into a 2-slot `WLib Class 0x09D8` / `2520` completed entry). |
| 4 | `this.slot[16]` (`cooldown_table`) | `WHash` (`0x0023`) | Maps repeatable quest `DataID` (`u32`) → cooldown expiration timestamp / timer entry. |
| 5 | `this.slot[18]` (`category_limiter_table`) | `WHash` (`0x0023`) | Maps quest category ID (`u32`) → limiter counter and reset timestamp (`QuestCategoryLimiter_ResetTime` `0x10003B0B`). |
| 6 | `this.slot[2]` (`autobestow_or_history_table`) | `WState` container | Pending auto-bestow / quest history table stored into `this.slot[2]`. |

#### 3.8.3 `0x87930006` Barter Request (`Native Class 0x0796`, 49-Byte Fixed Body) <a id="spec-wstate-barter"></a>

In `0x87930006` (`C→S`, callers `1941::2` at `wlib+0x060e38` and `1941::5` at `wlib+0x060d18`), the `barter_req: WState` parameter is an instance of native C++ `WState` class `0x0796` (`BarterRequest`, vtable `0x1417d9300`, wire serializer/deserializer `0x140a4a640`). Following the `WState` header and `class_id = 0x0796`, the serializer writes and reads a fixed 49-byte (`0x31`-byte) little-endian payload (`BIN-CLIENT`, `BIN-WLIB`):

| Byte Offset (after `0x0796` tag) | Size (Bytes) | Wire Type | Struct Offset | Native Getter / Setter (`0x0796xxxx`) | Field Name & Meaning |
|---|---|---|---|---|---|
| `+0x00` | 4 | `u32` (`DataID`) | `[rbx+0x08]` | `0x07960015` / `0x07960016` | `source_did`: Optional source item/context `DataID` (`0` for standard NPC/wallet barter). |
| `+0x04` | 4 | `u32` (`DataID`) | `[rbx+0x0C]` | `0x07960001` / `0x07960002` | `vendor_barter_did`: Vendor barter table `DataID` (validated in `1941::5` via `wlib+0x060ae0`). |
| `+0x08` | 4 | `u32` | `[rbx+0x10]` | `0x07960003` / `0x07960004` | `context_param`: Secondary barter context parameter (default `0`). |
| `+0x0C` | 1 | `u8` (`bool`) | `[rbx+0x14]` | `0x07960026` / `0x07960025` | `is_special_flag`: Boolean byte (`0` or `1`; deserializer `0x140a4a6eb` rejects values `>= 2`). |
| `+0x0D` | 4 | `u32` (`DataID`) | `[rbx+0x18]` | `0x07960011` / `0x07960012` | `alt_profile_did`: Optional alternate profile `DataID` (default `0`). |
| `+0x11` | 4 | `u32` | `[rbx+0x1C]` | `0x07960013` / `0x07960014` | `alt_entry_index`: Optional alternate entry index (default `0`). |
| `+0x15` | 4 | `u32` (`DataID`) | `[rbx+0x20]` | `0x07960005` / `0x07960006` | `barter_item_did`: Selected barter item/offer `DataID`. |
| `+0x19` | 4 | `u32` (`DataID`) | `[rbx+0x24]` | `0x07960007` / `0x07960008` | `barter_profile_did`: Active `Barter_Profile` (`0x100017FA`) `DataID`. |
| `+0x1D` | 4 | `u32` (`DataID`) | `[rbx+0x28]` | `0x07960009` / `0x0796000A` | `barter_merchant_did`: Barter merchant/service `DataID`. |
| `+0x21` | 4 | `u32` | `[rbx+0x2C]` | `0x0796000B` / `0x0796000C` | `barter_entry_index`: Zero-based offer index within the barter profile. |
| `+0x25` | 4 | `u32` | `[rbx+0x30]` | `0x0796000D` / `0x0796000E` | `quantity`: Requested exchange count (`CounterBox_CurrentCount` `0x100002BD`). |
| `+0x29` | 4 | `u32` | `[rbx+0x34]` | `0x0796000F` / `0x07960010` | `reputation_or_tier`: Tier/requirement value (`BarterReputationRewardTier` `0x2300026F`). |
| `+0x2D` | 4 | `u32` | `[rbx+0x38]` | `0x07960021` / `0x07960022` | `extra_param`: Reserved/extra parameter (`0`). |

#### 3.8.4 `0x85220006` Quest Dispenser Selection `WState` (`PropertyCollection` `0x00A6` / `0x0027`) <a id="spec-wstate-quest-dispenser"></a>

When the player responds to an NPC or item Quest Dispenser dialog (`0x140e5ca50`–`0x140e5cc8a` → `0x140b67680` → `286::1` at `wlib+0x161564` → `0x85220006`), the 3 wire arguments are (`BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`):
1. `selection_data: WState` (`PropertyCollection` `0x00A6` / `0x0027`) populated with:
   - `0x10000BDC` (`QuestDispenser_Action`, `u32` enum from `QuestRoleAction` `0x2300001D`)
   - `0x10000208` (`QuestDispenser_RoleName`, string/token copied from the selected picker entry)
   - `0x10001270` (`QuestDispenser_RoleConstraint`, `u32` constraint copied from the selected picker entry)
   - `0x1000146E` (`QuestDispenser_Quest`, `u32` quest `DataID` copied from `0x10000F13` `UI_QuestPicker_DataID`)
   - `0x100000BC` (`QuestTreasure_SelectedItem`, `u32` item `DataID` of the player's chosen selectable reward when completing a quest via `CompleteNowButton` `0x10000562` / `FinishQuestButton` `0x10000CA9`)
2. `dispenser_did: u32`: The selected quest `DataID` (`0x10000F13` `UI_QuestPicker_DataID`).
3. `source_iid: InstanceID`: The 64-bit `InstanceID` of the dispenser NPC or item.

#### 3.8.5 `0x8512003D` Pending Loot Entry (`WLib Class 0x04CD` / `1229`) <a id="spec-wstate-pending-loot"></a>

`0x8512003D` (`S→C`, unpacked by `274::25` at `wlib+0x0ef250`) delivers a `WArray` (`0x00B6`) of `WLib Class 0x04CD` (`1229`) objects. Each `Class 0x04CD` instance has 19 slots (`152` bytes of slot cells plus nested `PropertyCollection` objects) mapped by `274::25` into the UI `PropertyCollection` (`0x10004E68` `UI_Inventory_PendingLoot_Array`) (`BIN-WLIB`, `DAT-LOGIC`):

| `Class 0x04CD` Slot | Wire Slot Type | Target UI Property ID | Property Name & Meaning |
|---|---|---|---|
| `slot[4..5]` | `f64` | `0x10004E63` | `UI_Inventory_PendingLoot_ItemExpirationTime` (64-bit expiration timestamp). |
| `slot[6]` | `PropertyCollection` (`0x00A6`) | *(nested)* | Item instance properties (queried for `0x10000669` `Item_Level`). |
| `slot[7..8]` | `InstanceID` (`u64`) | — | Source container / entity `InstanceID`. |
| `slot[11]` | `u32` | `0x10004E61` | `UI_Inventory_PendingLoot_ItemQuantity` (stack count; passed as Arg 2 of `0x85120041` / `0x85120042`). |
| `slot[12]` | `u32` (`DataID`) | `0x10004E64` | `UI_Inventory_PendingLoot_ItemDID` (item `DataID`; passed as Arg 3 of `0x85120041` / `0x85120042`). |
| `slot[13]` | `u32` | `0x10000669` | Fallback `Item_Level` when `slot[6]` is null. |
| `slot[15..16]` | `InstanceID` (`u64`) | — | Pending item `InstanceID`. |
| `slot[17]` | `u32` | `0x10004E87` | `UI_Inventory_PendingLoot_ItemEarnTimestamp` (earn timestamp; passed as Arg 1 of `0x85120041` / `0x85120042`). |
| `slot[18]` | `PropertyCollection` (`0x00A6`) | *(nested)* | Optional secondary item property collection. |

#### 3.8.6 `0x8535001D` Entity Use Request (`WLib Class 0x063E` / `1598`) <a id="spec-wstate-entity-use"></a>

`0x8535001D` (`C→S`) serializes an instance of `WLib Class 0x063E` (`1598`), which inherits from `WLib Class 0x01E6` (`486`, base `PropertyCollection` wrapper) and adds 3 script slots (`BIN-WLIB`):
- `slot[0]`: `StringInfo` (`0x00C7`) interaction label/context token.
- `slot[1]`: `u32` interaction flags / action code.
- `slot[2]`: `PropertyCollection` (`0x00A6`) containing target entity `InstanceID` and usage parameters.

---

### 3.9 Enumeration Constants (`client_gamelogic.dat` `0x2300xxxx` & C++ UI Handlers) <a id="spec-enums"></a>

Provenance: `DAT-ENUM` (`0x23000017`, `0x23000018`, `0x2300001D`, `0x2300006B`, `0x230000D1`, `0x23000292`), `BIN-CLIENT` (`0x140d74e31`–`0x140d75068`, `0x140e55800`–`0x140e55b50`), `BIN-WLIB` (`wlib+0x0eec54`, `wlib+0x1519a8`).

#### 3.9.1 `QuestRoleAction` / `QuestDispenser_Action` (`EnumMapper 0x2300001D`, Property `0x10000BDC`) <a id="spec-enum-quest-role-action"></a>

Used in `0x85220002` (`S→C`), `0x85220006` (`C→S`), and switched at `0x140e55959` (`BIN-CLIENT`, `DAT-ENUM`):

| Value (`u32`) | Symbolic Name (`0x2300001D`) | Meaning |
|---|---|---|
| `0` | `Undef` | Undefined / no action. |
| `1` | `NPCComment` | Plays an NPC comment (`0x88CBxxxx`). |
| `2` | `NPCTalk` | Advances a quest dialogue objective (`NPCTalk`) with an NPC. |
| `3` | `Teleport` | Executes a quest-driven teleport. |
| `4` | `ItemTalk` | Advances a quest objective via an interactive item. |
| `5` | `LeaveInstance` | Leaves the current private encounter/instance. |
| `6` | `Bestow` | Offers/bestows a new quest in the Quest Dispenser dialog. |
| `7` | `Detect` | Triggers a quest detection role. |
| `8` | `Failure` | Displays quest failure dialog. |
| `9` | `Session` | Launches a Session Play quest. |
| `10` | `ResourceSetWait` | Waits for resource set availability. |
| `12` | `OpenUIMenu` | Opens an associated service UI menu. |
| `13` | `OpenInstanceUI` | Opens the Instance Finder / World Join UI. |
| `14` | `MissionTeleport` | Teleports into a Mission instance. |

#### 3.9.2 `QuestUpdateType` (`EnumMapper 0x23000018`) <a id="spec-enum-quest-update-type"></a>

Used by quest dispenser ring/icon state and `UI_QuestPicker_RoleAction` (`0x10000B0B`) (`DAT-ENUM`):

| Value (`u32`) | Symbolic Name (`0x23000018`) | Value (`u32`) | Symbolic Name (`0x23000018`) |
|---|---|---|---|
| `0` | `Undef` | `10` | `Session` |
| `1` | `Teleport` | `13` | `AlwaysShowBestowRings` |
| `2` | `Complete` | `14` | `ResourceSetWait` |
| `3` | `FutureBestow` | `16` | `Requires_XP1` |
| `4` | `Bestow` | `18` | `Requires_ExpansionPack` |
| `5` | `Advance` | `19` | `OpenUIMenu` |
| `6` | `TooLow` | `20` | `Requires_WebstoreItem` |
| `7` | `Failure` | `29` | `OpenInstanceUI` |
| `8` | `Comment` | | |
| `9` | `Pending` | | |

#### 3.9.3 `QuestStatus` (`EnumMapper 0x23000017`) and `FellowshipQuestResponseType` (`EnumMapper 0x23000292`) <a id="spec-enum-quest-status"></a>

- **`QuestStatus` (`0x23000017`):** Bitmask where `0xF0000000` is `RawStatusMask` and `0x0FFFFFFF` is `ObjectiveNumberMask`:
  - `0x00000000` = `Undef`
  - `0x10000000` = `Underway` (`0x10000001`..`0x10000023` = `Underway_Objective01`..`Underway_Objective35`)
  - `0x20000000` = `Failed`
  - `0x30000000` = `Completed`
  - `0x40000000` = `Abandoned`
  - `0x50000000` = `Tapped`
- **`FellowshipQuestResponseType` (`0x23000292`, used in `0x852100BD`):**
  - `0` = `Undef`, `1` = `ReadyToTurnIn`, `2` = `Underway`, `3` = `Completed`, `4` = `Eligible`, `5` = `NotEligible`.

#### 3.9.4 Fellowship Loot Vote Choice (`0x8512001B`) and `FellowshipLootType` (`EnumMapper 0x2300006B`) <a id="spec-enum-loot"></a>

1. **Fellowship Loot `vote_choice` (`u32` in `0x8512001B`, verified in C++ UI button handler `0x140d74e31`–`0x140d74eac`, auto-pass handler `0x140d75052`, `274::35` at `wlib+0x0eec54`, and `UIElementID` `0x230000D1`):**

| `vote_choice` (`u32`) | Meaning | Triggering UI Button (`0x230000D1`) | C++ Handler Address |
|---|---|---|---|
| `1` | **Greed** | `ItemGreedButton` (`0x10000FF4`) | `0x140d74e92` (`mov edx, 1`) |
| `2` | **Need** | `ItemNeedButton` (`0x10000FF3`) | `0x140d74e79` (`mov edx, 2`) |
| `3` | **Pass** | `ItemNeedGreedPassButton` (`0x100012E7`), `ItemRollPassButton` (`0x100012E8`), or timer expiration / auto-pass | `0x140d74ea7` / `0x140d75052` (`mov edx, 3`) / `wlib+0x0eec84` |
| `4` | **Roll** | `ItemRollButton` (`0x10000105`) | `0x140d74e5e` (`mov edx, 4`) |

2. **`FellowshipLootType` (`EnumMapper 0x2300006B`, `rule_type: u32` in `0x8512001D`):**
   - `0` = `Undef`
   - `2` = `FreeForAll`
   - `3` = `MasterLooter`
   - `4` = `RoundRobin_NeedGreed`
   - `5` = `RoundRobin_RollPass`
   - `6` = `FreeForAll_NeedGreed`
   - `7` = `MasterLooter_FreeForAll`
   - `8` = `FreeForAll_RollPass`

---

## 4. Change Log <a id="changelog"></a>

| Date | Author | Summary |
|---|---|---|
| 2026-10-21 | Spec writer (dirty room, `m8-gameplay-p3`) | Initial complete dirty-room analysis and clean-room specification section for P3 gameplay messages (NPC Interaction, Dialogue, Quests, Deeds, Vendors, Buyback, Repair, Barter, Container/Fellowship/Pending Loot, and Wallet `PaperItem`). |
| 2026-10-22 | Spec writer (dirty room, `m8-gameplay-p3`) | Resolved `OPEN (P3-1)` and `OPEN (P3-2)` by reverse-engineering the exact inner `WState` wire layouts (`0x8521005C` 6 journal tables, `0x87930006` 49-byte `0x0796` `BarterRequest` payload, `0x85220006` `PropertyCollection`, `0x8512003D` `Class 0x04CD` pending loot slots, `0x85120041`/`0x85120042` wire argument order, and `0x8535001D` `Class 0x063E`) and extracting all exact enumeration constants (`0x23000017`, `0x23000018`, `0x2300001D`, `0x2300006B`, `0x23000292`, and `0x8512001B` `vote_choice`). 0 OPEN callouts remain. |
