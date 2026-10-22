<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 Gameplay P2 — Targeting, Skills, Quickslots, Inductions, Cooldowns, Effects, Vitals, Combat Deltas, and Defeat/Revive

- **Status:** Draft
- **Last updated:** 2026-10-20
- **Author role:** Spec writer (dirty room, `m8-gameplay-p2`)
- **Provenance (`BIN`):** `lotroclient64.exe` (x86_64 PE, build `4100.0061.8168.4126`, SHA-256 `b3022fb93dc6dca52bcd2fb3bdb3fbabb3cbfab3dbfcfbd8769f7b65b739c3db`, tool: Ghidra 11.x + `capstone` x86_64 disassembly; M1/M2 were insufficient because the official server's WLib RPC wire IDs, native class serializers, and property schemas are compiled into the binary and embedded WLib bytecode)
- **Provenance (`DAT`):** `client_general.dat` (`0x11000000` embedded WLib bytecode image and `0x34000000` Master Property Definition Table)

---

## Dirty-Room Architecture Overview: WLib RPC Wire Framing (`0x30`, `0x5F`, `0xC0`) & `WLibArgFrame` (`0x008E`)

Before detailing the four P2 gameplay subsystems, all client-to-server (`C→S`) and server-to-client (`S→C`) WLib RPCs in P2 share the same outer notice/event framing and `WLibArgFrame` (`0x008E`) payload encoding:

### 1. Outer Event Envelope (`Weenie_WLibRpc` `0x30`)
- **Serializer:** `0x140522950` (base class constructor `0x140522190`, subclasses `0x00D9000C` / `0x00D9000D` at `0x140522200` / `0x140522250`).
- **Handler:** `0x14051C160` (dispatches to WLib VM `0x1403AB340`), or `0x5F` (`0x140B96FC0`) / `0xC0` (`0x140B73340` for remote entity RPCs).
- **Wire layout inside the game-action sub-message (`0x0000000E` `GameEvent` / `WeenieEvent`):**
  1. `ev_id` (`VLE`, `1B` for `0x30`).
  2. Optional `u64 target_entity_id` (`8B LE`): present **only** if base event flags bit 5 (`0x20`) is set (`([rcx+0x08] & 0x20) != 0`). For `0x00D9000C` and `0x00D9000D` constructed via `0x140522190`, `[rcx+0x08]` is initialized to `0`, so the `u64` prefix is omitted (`0x30` directly precedes `rpc_id`). In `0xC0` (`0x140B73340`), `u64 entity_id` precedes `u32 rpc_id`.
  3. `u32 rpc_id` (`4B LE`, e.g., `0x851E001E`). Note: in WLib bytecode, each C→S stub has a method ID (`(class_id << 16) | 0x80000000 | stub_index`), and its body pushes a distinct **wire RPC ID** (`(class_id << 16) | 0x80000000 | wire_index`) into `send_rpc`. Both are documented below; **only the wire RPC ID appears on the wire**.
  4. `u32 arg_frame_byte_len` (`4B LE`, written by `0x140352750`).
  5. `u8[arg_frame_byte_len]` (`WLibArgFrame` buffer).

### 2. `WLibArgFrame` (`0x008E`) Scalar Primitives & Object Graph (`pack_ref` / `unpack_ref`)
All arguments inside `WLibArgFrame` are packed in exact call order of the WLib stub's `call_native` instructions and unpacked in the exact call order of the receiver's `call_native` instructions:
- `0x008E0002` (`pack_32` @ `0x14039FA30`) / `0x008E0007` (`unpack_32` @ `0x1403AEBF0`): `4B LE` `u32` / `i32` / `u8` (masked with `& 0xFF` on unpack when used as `bool`).
- `0x008E0003` (`pack_64` @ `0x14039FBA0`) / `0x008E0008` (`unpack_64` @ `0x1403AED50`): `8B LE` `u64` (used for 64-bit `InstanceID` / `EntityId` and 64-bit cooldown keys).
- `0x008E0004` (`pack_f32` @ `0x14039FB20`) / `0x008E0009` (`unpack_f32` @ `0x1403AECD0`): `4B LE` IEEE-754 `f32`.
- `0x008E0005` (`pack_f64` @ `0x14039FA90`) / `0x008E000A` (`unpack_f64` @ `0x1403AEC50`): `8B LE` IEEE-754 `f64` (used for server timestamps).
- `0x008E0001` (`pack_ref` @ `0x14039F9B0` -> `0x140361630` -> `0x14035FCF0` -> `0x140363240`) / `0x008E0006` (`unpack_ref` @ `0x1403AEB50` -> `0x140368460` -> `0x140363320`):
  - Serializes a self-contained WLib object graph into the `WLibArgFrame` stream (`0x140363520` / `0x140363A00` on write, `0x140363520` / `0x140363B80` on read):
    1. `u32 ref_count` (`4B LE`): number of object reference slots in the graph (`0` if root is null, `1` for a single non-null leaf object such as `0x0D85`, `0x01C6`, `0x0198`, `0x03F8`).
    2. `u32[ref_count] ref_ids` (`4B LE` each): object table indices (`0x00000000` for the first object, `0xFFFFFFFF` for null).
    3. For each non-null object `i` in `0 .. ref_count`:
       - `u16 class_id` (`2B LE`, e.g., `0x0D85`, `0x01C6`, `0x0198`, `0x03F8`, `0x006F`, `0x0075`).
       - If the class is a **WLib native class** (`bit 28 == 1` in object header, registered in `0x14081B530` base or `0x140B293D0` client): invokes `descriptor->Serialize(obj + 0x18, Archive*)` (`[rax+0x68]`).
       - If the class is a **WLib script class** (`bit 28 == 0` in object header, e.g., `0x03F8` cooldown timer): writes raw `8B * attr_count` attribute cells (`[obj + 0x10]`), followed by any nested reference indices.

---

## 1. Target Selection (`CombatCommand::SelectTarget`, `CombatEvent::TargetChanged`)

### 1.1 Local Player Target Selection (`0x06C3` vslot 631 -> `0x051E`)
- **WLib Handler:** `0x06C3` vslot `631` (`0x277`, `Body 0x015228`, `code_off=0x03BC48`), invoked when the player clicks an entity, presses Tab/nearest-target, or clears selection (`Esc`).
- **Step-by-step behavior in `Body 0x015228`:**
  1. Reads the local player's current selection from property `0x0000034F` (`Entity_Selection`, `u64` InstanceID) and `0x000004B3` (`World_Selection`, `u64` InstanceID).
  2. If the new `target_entity_id` differs from the current selection:
     - Updates local properties `0x0000034F` (`Entity_Selection`) and `0x000004B3` (`World_Selection`) to `target_entity_id`.
     - Checks whether the target is a **client-only entity** by testing `(target_entity_id >> 48) & 0x03FF == 0x03FF` (mask `0x03FF0000_00000000`).
     - If client-only (`0x03FF`), sends `0` (`0x00000000_00000000`) to the server; otherwise sends `target_entity_id`.
     - Invokes C→S stub `0x851E0028` (`body=0x1098FC`), which sends **wire RPC ID `0x851E001E`** with `pack_64(u64 target_entity_id)` (`8B LE`).
     - Calls `0x0729E8` (`0x0100` fn 232/20) to update auto-attack target tracking.

### 1.2 Server-Forced Selection Update (`0x051E` S→C `0x851E0012`)
- **WLib Receiver:** `0x851E0012` (`body=0x1093F4` on class `0x051E`).
- **Payload:** `unpack_64(u64 target_entity_id)` (`8B LE`).
- **Handler:** Calls `this->vslot[710](target_entity_id)` (`Body 0x017254`), which calls `this->vslot[631](target_entity_id)` to update `Entity_Selection` (`0x0000034F`) and `World_Selection` (`0x000004B3`), and notifies C++ native `0x01670009` (`0x140B3A890` -> `0x140B0C8B0`).

### 1.3 Combat / Assisted Target Request (`0x04E9` `CCombatSystem`)
- **C→S Stub `0x84E90032` (`body=0x0FB170`, wire RPC ID `0x84E90008`):**
  - Invoked by `0x072CAC` (`0x0100` fn 239, C++ `0x140B5ED45`), rate-limited by property `0x10001B9A` (`Antispam_TargetRequest`).
  - Increments sequence counter at `this->attr[7]` (`u32 seq_token`) and sends **wire RPC ID `0x84E90008`** with:
    1. `pack_32(u32 seq_token)` (`4B LE`)
    2. `pack_64(u64 target_entity_id)` (`8B LE`).
- **S→C Receiver `0x84E90001` (`body=0x0FB1D8`):**
  - Unpacks:
    1. `unpack_32(u32 seq_token)` (`4B LE`)
    2. `unpack_64(u64 target_entity_id)` (`8B LE`).
  - Calls `0x072D18` (`0x0100` fn 240): if `seq_token == this->vslot[634]()`, calls `this->vslot[631](target_entity_id)` to apply the target change.

### 1.4 Replicated Target Property on Any Entity (`0x0000034F` `Entity_Selection`)
- Every entity (local player, remote players, NPCs/monsters) replicates its current target via property **`0x0000034F` (`Entity_Selection`, `u64` InstanceID)** in `Weenie_PropertyUpdate` (`0x5B`) or `0x19`.
- C++ handler `FUN_1409f4c40` / `FUN_140b44e00` (`d_sel1.c`) listens for changes to `0x0000034F` to drive Target-of-Target UI and remote entity facing/selection indicators (`CombatEvent::TargetChanged { entity, target }`).

### 1.5 Fellowship / Raid Target Marking (`0x0739`)
- **Set Target Mark:** C→S stub `0x87390016` (`body=0x13D12C`) sends **wire RPC ID `0x87390015`** with `pack_32(u32 mark_index)`, `pack_64(u64 target_entity_id)` (`12B`).
- **Clear Target Mark:** C→S stub `0x87390014` (`body=0x13D180`) sends **wire RPC ID `0x87390011`** with `pack_32(u32 mark_index)`, `pack_64(u64 target_entity_id)` (`12B`).

---

## 2. Skill Execution, Casts/Inductions/Channels, and Cooldowns

### 2.1 Skill Execution (`0x0528` `SkillSystem`, Handler `0x017A`)

#### C→S `UseSkill` (`0x85280005`) & Native Class `0x0D85` (`SkillRequest`)
- **Entry points:** C++ `0x140B4E45C` (`0x017A0027`, `Body 0x16F92C`) and `0x140B4E314` (`0x017A0025`, `Body 0x16F5D0`).
- **Action:** Allocates native class `0x0D85` (`size = 0x58`, registered at `0x140B2D817` with descriptor `0x141CDFA70`, serializer `0x140BB19B0` -> `0x140ADD850`), populates it via native `0x0D85` methods (`0x140B41DD0`, `0x140B41EA0`, `0x140B41F80`, `0x140B42050`, `0x140B42120`, `0x140B421E0`), stores `seq_token` in local property `0x10002B84` (`Skill_QueueToken`), and calls C→S stub `0x85280014` (`body=0x10D2AC`), which sends **wire RPC ID `0x85280005`** with `pack_ref(skill_req_0x0D85)`.
- **Exact Wire Layout of Native Class `0x0D85` (`0x140ADD850`):**
  Inside the `WLibArgFrame` object record (after `u32 ref_count = 1`, `u32 ref_id = 0`, `u16 class_id = 0x0D85`):
  1. `u32 flags` (`4B LE` bitmask):
     - Bit 0 (`0x01`): `sub_skill_did` is present (`[rbx+0x04] != 0 && [rbx+0x04] != [rbx+0x00]`).
     - Bit 1 (`0x02`): `target_sub_index` is present (`(i32)[rbx+0x0C] >= 0`; initialized to `-1` when unused).
     - Bit 2 (`0x04`): boolean flag (`[rbx+0x10] != 0`, set by `0x0D850007` @ `0x140B41EA0`; encoded directly in bit 2 of `flags` with **0 extra wire bytes**).
     - Bit 3 (`0x08`): `target_entity_id` is present (`[rbx+0x18] != 0`).
     - Bit 4 (`0x10`): ground-target `Position` (`0x00A0`) is present (`Position::IsValid(rbx+0x20)` @ `0x14036DD80` returns non-zero).
  2. `u32 skill_did` (`4B LE`, `[rbx+0x00]`, DataID of the executed skill, e.g., `0x7000xxxx`).
  3. `u32 seq_token` (`4B LE`, `[rbx+0x08]`, monotonically incrementing sequence token from `this->vslot[740]()`).
  4. Optional `u32 sub_skill_did` (`4B LE`, `[rbx+0x04]`, present iff `(flags & 0x01) != 0`).
  5. Optional `i32 target_sub_index` (`4B LE`, `[rbx+0x0C]`, present iff `(flags & 0x02) != 0`).
  6. Optional `u64 target_entity_id` (`8B LE`, `[rbx+0x18]`, present iff `(flags & 0x08) != 0`).
  7. Optional `Position ground_target` (`[rbx+0x20]`, serialized via `Position::Serialize` @ `0x140372D60`, present iff `(flags & 0x10) != 0`).

#### C→S `UseItemSkill` (`0x85280006`)
- **WLib Stub:** `0x85280015` (`body=0x10D38C`, called by `0x19A210` on item use).
- **Wire RPC ID:** **`0x85280006`**.
- **Payload:**
  1. `pack_32(u32 seq_token)` (`4B LE`)
  2. `pack_64(u64 item_entity_id)` (`8B LE`).

#### C→S `CancelCast` / Clear Skill Queue (`0x85280008`, `0x85280002`, `0x85280007`)
- **Interrupt / Cancel Active Skill (`0x85280008`):** Stub `0x85280012` (`body=0x10D204`, called by `0x16F50C` `0x017A` fn 11) sends **wire RPC ID `0x85280008`** with **0 arguments** (`arg_frame_byte_len = 0`).
- **Clear Queued Skill (`0x85280002`):** Stub `0x85280013` (`body=0x10D1BC`, called by `0x16F5A4` `0x017A` fn 12) sends **wire RPC ID `0x85280002`** with **0 arguments**.
- **Abort / Reset Skill State (`0x85280007`):** Stub `0x85280016` (`body=0x10D174`, called by `0x16FD94` `0x017A0018` from C++ `0x140B4E752`) sends **wire RPC ID `0x85280007`** with **0 arguments**.

#### S→C Skill Execution Responses (`0x0528`)
- **`0x8528000A` (`body=0x10D304`, Skill Execution Result):**
  - Unpacks:
    1. `unpack_32(u32 result_code)` (`4B LE`, `0` = success, non-zero = error enum)
    2. `unpack_32(u32 seq_token)` (`4B LE`)
    3. `unpack_32(u32 skill_did)` (`4B LE`).
  - Handler `0x16FC38` (`0x017A` fn 16): compares `seq_token` against `0x10002B84` (`Skill_QueueToken`), updates `0x10001307` (`Skill_LastSuccessTimestamp`) on `result_code == 0` or `0x10001410` (`Skill_LastErrorTimestamp`) on error, and calls C++ native `0x015E0004` (`0x140B35110` -> `0x140AFC030`).
- **`0x8528000E` (`body=0x10D3F4`, Item Skill Execution Result):**
  - Unpacks `unpack_32(u32 result_code)`, `unpack_32(u32 seq_token)`, `unpack_64(u64 item_entity_id)` -> `0x16FD08` (`0x017A` fn 17) -> native `0x015E0009` (`0x140B35210` -> `0x140AFC0A0`).
- **`0x8528000D` (`body=0x10D47C`, Skill Error Notification):**
  - Unpacks `unpack_32(u32 err_code)` -> `0x1708A4` -> native `0x015E0010` (`0x140B33D40` -> `0x140AF99F0`).
- **`0x8528001A` (`body=0x10D24C`, Skill State Notification):**
  - Unpacks `unpack_32(u32 val)` -> `0x16FE20` -> native `0x015E0014` (`0x140B31670` -> `0x140AF5820`).

### 2.2 Casts, Inductions, and Channels (`0x100067E7` `Skill_InductionAction_Struct`)
- **Primary Discovery (`FUN_140dd3b20` in `d_prog2.c` & `0x140DD9940`):**
  Inductions and channeled skills are driven by **replicated property updates (`Weenie_PropertyUpdate` `0x5B` / `0x19`)** on the casting entity (both local player and any target/focus entity), specifically compound property **`0x100067E7` (`Skill_InductionAction_Struct`, type `0x0B` SubPropertyBag)**.
- **Sub-properties inside `0x100067E7` (`Skill_InductionAction_Struct`) read by `FUN_140dd3b20`:**
  - `0x100067D8` (`Skill_InductionAction_StartTime`, `f64` IEEE-754 server timestamp in seconds).
  - `0x100067D9` (`Skill_InductionAction_EndTime`, `f64` IEEE-754 server timestamp in seconds).
  - `0x10000729` (`Skill_DefaultSkill`, `u32` DataID of the parent skill being executed).
  - `0x10000C5C` (`Skill_InductionAction`, `u32` DataID of the induction/channel action record).
- **Static DID Properties looked up on `Skill_InductionAction` (`0x10000C5C`) in `FUN_140dd3b20`:**
  - `0x10000B0F` (`Induction_Interruptable`, `u8` bool) / `0x10001360` (`Channeling_Interruptable`, `u8` bool).
  - `0x1000125E` (`Induction_TitleString`, `StringInfo`) / `0x100000F2` (`Channeling_TitleString`, `StringInfo`).
- **Companion Replicated Properties on the Caster Entity:**
  - `0x100003C8` (`Induction_CurrentInductionState`, `i32`, monitored by WLib `0x04A7` `Body 0x0C8E80`).
  - `0x1000331D` (`Induction_CurrentInductionSkill`, `u32` Skill DataID).
  - `0x10000BC3` (`Channeling_CurrentChannelingDID`, `u32` Action DataID, monitored by WLib `0x03C2` `Body 0x06DCCC`).
  - `0x1000112C` (`Skill_CurrentChanneledSkill`, `u32` Skill DataID).
  - `0x100011FA` (`Skill_CurrentChanneledSkillTarget`, `u64` Target InstanceID).
- **State Machine in `0x140DD9940(bar, start_time_f64, end_time_f64)`:**
  - **`CombatEvent::CastStarted`:** `0x100067E7` arrives with `0.0 < start_time < end_time` and `Skill_InductionAction != 0`. Duration is `(end_time - start_time)` seconds.
  - **`CombatEvent::CastDelayed` (Induction Setback on Hit):** While an induction is active, a new `0x100067E7` update arrives with an increased `start_time` and `end_time`, shifting the progress bar backward.
  - **`CombatEvent::CastEnded`:** `0x100067E7` arrives with `start_time == 0.0` and `end_time == 0.0` (or `end_time <= start_time`), or `Induction_CurrentInductionState` (`0x100003C8`) transitions to `0`, immediately hiding the induction bar.

### 2.3 Cooldowns (`0x04EE` `CCooldownSystem`, Handler `0x0106`, Timer `0x03F8`)
- **64-bit `cooldown_key` (`u64`) Encoding (`Body 0x07E1A8` & `Body 0x07DF30`):**
  - Upper 16 bits (`(cooldown_key & 0xFFFF0000_00000000) >> 32`):
    - `0x00010000` (`0x0001` in bits 48..63): **Per-Skill DataID Cooldown**. Lower 32 bits (`cooldown_key & 0xFFFFFFFF`) = `u32 skill_did`. Dispatches to C++ native `0x01390001` (`0x140B46960` -> `0x140B1AFA0(ui_mgr, skill_did, start_time_f64, duration_f32)`).
    - `0x00020000` (`0x0002` in bits 48..63): **Cooldown Channel** (shared cooldown group across skills/items, e.g., potion or shared skill channel). Lower 32 bits = `u32 cooldown_channel_id` (matching property `0x1000090C` `Skill_CooldownChannel`). Dispatches to C++ native `0x01390002` (`0x140B46A30` -> `0x140B1B110(ui_mgr, channel_id, start_time_f64, duration_f32)`).
    - `0x00040000` (`0x0004` in bits 48..63): Secondary category handled in `0x07DF30`.
- **WLib Script Class `0x03F8` (`CooldownTimer`):**
  - Script class (`bit 28 == 0` in object header) with 2 attribute cells (`16 bytes` following `u16 class_id = 0x03F8`):
    - `attr64[0]` (`+0x00`, `8B LE`): `f64 start_timestamp` (server frame clock timestamp in seconds when the cooldown started).
    - `attr[1]` (`+0x08`, `8B` cell containing `4B LE` `f32 duration` in seconds in the low 32 bits + `4B` zero padding).
- **S→C Cooldown RPCs on `0x04EE`:**
  1. **`CooldownStarted` (`0x84EE0008`, `body=0x0FC0D4`):**
     - `unpack_ref(timer_0x03F8)` (WLib script class `0x03F8` with `f64 start_timestamp`, `f32 duration`)
     - `unpack_64(u64 cooldown_key)` (`8B LE`).
     - Calls `0x07D774` -> `0x07DF30`, which invokes `0x01390001` (for `0x00010000` skill DID) or `0x01390002` (for `0x00020000` channel ID).
  2. **`CooldownEnded` (`0x84EE000A`, `body=0x0FC074`):**
     - `unpack_64(u64 cooldown_key)` (`8B LE`).
     - Calls `0x07D854`, which removes the entry from `this->attr[3]` (`0x0075` hash table) and calls `0x01390001` / `0x01390002` with `start_time = -1.0` (`0xBFF0000000000000`) and `duration = 0.0` to clear the UI sweep.
  3. **`ResetAllCooldowns` (`0x84EE0003`, `body=0x0FC028`):**
     - `0 arguments`. Iterates all entries in `this->attr[3]` and clears every active cooldown (`0x07DD08`).
  4. **`RemoveCooldownList` (`0x84EE0001`, `body=0x0FBFC4`):**
     - `unpack_ref(list_0x006F)` (list of `u64 cooldown_key` values) -> calls `0x07D854` for each key (`0x07DDAC`).

---

## 3. Quickslots (`CombatCommand::SetQuickslot`, `CombatEvent::QuickslotChanged`)

### 3.1 Quickslot Indexing & Native Class `0x01C6` (`Shortcut`)
- **Grid Geometry (`0x16B3E0` / `0x16BE2C`):**
  - `84` total quickslots (`0x54`, indices `0..83`), arranged as `7` pages of `12` slots (`page_index = 0..6`, `slot_in_page = 0..11`).
  - Active main bar page is stored in replicated property **`0x10004CAA` (`Shortcut_Current_Page`, `i32`)**.
- **Exact Wire Layout of Native Class `0x01C6` (`Shortcut`, `size = 0x20`, registered at `0x140B29805`, descriptor `0x141CDF9F0`, serializer `0x140BB1970` -> `0x140ADD5F0`):**
  Inside the `WLibArgFrame` object record (after `u32 ref_count = 1`, `u32 ref_id = 0`, `u16 class_id = 0x01C6`):
  1. `u32 shortcut_type` (`4B LE`, `[rbx+0x00]`; deserializer rejects values `> 12` / `0x0C`).
  2. Type-specific payload (dispatched via jump table at `0x140ADD6C0`):
     - **`shortcut_type == 0` (Empty / Cleared Slot):** `0 extra bytes`.
     - **`shortcut_type == 2` (Inventory Item Shortcut):**
       - `u64 item_entity_id` (`8B LE`, `[rbx+0x10]`, InstanceID of the item)
       - `u32 item_did` (`4B LE`, `[rbx+0x18]`, DataID of the item, serialized via `0x14031C890`).
     - **`shortcut_type == 3` or `4` (Chat Alias / Text Command Shortcut):**
       - `PString command_text` (`[rbx+0x08]`, serialized via `0x140320A00`: `VLE char_count` followed by `u16[char_count]` UTF-16LE code units).
     - **`shortcut_type == 5`, `6` (Skill Shortcut), `9`, or `12` (Pet / Hobby / DataID Shortcut):**
       - `u32 target_did` (`4B LE`, `[rbx+0x18]`, DataID of the skill/action, serialized via `0x14031C890`).
     - **`shortcut_type == 7` (Value / Preset Shortcut):**
       - `u32 extra_val` (`4B LE`, `[rbx+0x1C]`, serialized via `0x14031C890`).
     - **`shortcut_type == 1`, `8`, `10`, `11`:** `0 extra bytes` (default branch of jump table).

### 3.2 C→S Quickslot RPCs (`0x051E` & `0x0958`)
- **`SetQuickslot` (`0x851E0020`, stub `0x851E0022` @ `body=0x109954`):**
  - Invoked by `0x16BD44` (`0x01790013` @ `0x140B99B57`) and `0x16B26C` when assigning or clearing a quickslot (`slot_index < 84`). To clear a slot, the client constructs a `0x01C6` object with `shortcut_type = 0` (`0x16B3E0`) and sends it via `0x851E0020`.
  - **Payload:**
    1. `pack_ref(shortcut_0x01C6)`
    2. `pack_32(u32 slot_index)` (`4B LE`, `0..83`).
- **Secondary C→S Quickslot RPCs:**
  - `0x851E0031` (stub `0x851E0023` @ `body=0x10973C`): `pack_ref(shortcut_0x01C6)`, `pack_32(u32 index)`.
  - `0x89580003` (stub `0x8958000A` @ `body=0x10CFC8`): `pack_ref(obj)`.
  - `0x89580007` (stub `0x89580009` @ `body=0x10CEF8`): `0 args`.
  - `0x89580008` (stub `0x8958000C` @ `body=0x10D00C`): `0 args`.

### 3.3 S→C Quickslot RPCs (`0x0958` & `0x051E`)
- **`QuickslotChanged` (`0x89580005`, `body=0x10CF34`):**
  - `unpack_ref(shortcut_0x01C6)`
  - `unpack_32(u32 slot_index)` (`4B LE`)
  - `unpack_32(u32 page_index)` (`4B LE`).
  - Calls `0x16C798` -> `0x16BE2C`, updating local slot storage and notifying C++ UI via `0x015F0005` (`0x140B997C0`).
- **`QuickslotChangedEx` (`0x89580001`, `body=0x10D050`):**
  - `unpack_ref(shortcut_0x01C6)`, `unpack_32(u32 slot_index)`, `unpack_32(u32 flags_or_mode)`, `unpack_32(u32 page_index)` -> calls `0x16C580`.
- **Clear Matching Shortcuts (`0x8958000B`, `body=0x10CE7C`):**
  - `unpack_32(u32 target_val)` -> calls `0x16C6D8` (removes matching shortcuts and sends `0x851E0020` with empty `0x01C6`).
- **Clear / Update Skill Shortcuts (`0x8958000D`, `body=0x10D0F0`):**
  - `unpack_32(u32 skill_did)`, `unpack_32(u32 arg2)` -> calls `0x16C61C` (iterates `84` slots for `shortcut_type == 6` matching `skill_did`).
- **Bulk Page Update (`0x851E002C`, `body=0x1096C8`):**
  - `unpack_ref(page_container)`, `unpack_32(u32 page_index)`.

---

## 4. Vitals, Combat State, Effects, Combat Hit/Damage/Heal Deltas, and Defeat/Revive

### 4.1 Vitals (`Weenie_PropertyUpdate` `0x5B` / `0x19` -> `0x065B` / `0x065D` / `0x065E`)
- **Wire Mechanism:** All entity vitals (Morale/Health, Power, and War-steed Mount Power) are synchronized via `Weenie_PropertyUpdate` (`0x5B`) and `0x19` property updates on the entity's property bag (`0x00B1`).
- **Morale / Health (`vital_type = 1`, WLib class `0x065D`, `Body 0x1BA330`):**
  - `0x100012D2` (`Health_CurrentLevel`, `f32` current morale)
  - `0x100008BB` (`Health_MaxLevel`, `f32` maximum morale)
  - `0x100015FF` (`Health_MaxLevel_Cached`, `f32`)
  - `0x10003F81` (`Health_Temporary`, `f32` temporary morale bubble)
  - `0x10004064` (`Health_Temporary_Max`, `f32` maximum temporary morale bubble)
  - `0x10001604` (`Health_UpdateThresholdPercent`, `f32`)
  - `0x10004300` (`Health_Starting_Level`, `f32`)
  - `0x100008BA` (`Health_RegenRate`, `f32` morale regeneration rate per second).
  - **Special Clamping Rule (`Body 0x1BA330`):** If `Health_CurrentLevel == 0.0` and property `0x1000148A` (`Death_NeverSayDie`, `u8`) is non-zero, the client clamps displayed current morale to `1.0` (`0x3F800000`).
- **Power (`vital_type = 2`, WLib class `0x065E`, `Body 0x1BA6D8`):**
  - `0x10000340` (`Power_CurrentLevel`, `f32` current power)
  - `0x10000520` (`Power_MaxLevel`, `f32` maximum power)
  - `0x100015FE` (`Power_MaxLevel_Cached`, `f32`)
  - `0x10003F82` (`Power_Temporary`, `f32` temporary power bubble)
  - `0x10004065` (`Power_Temporary_Max`, `f32` maximum temporary power bubble)
  - `0x10001605` (`Power_UpdateThresholdPercent`, `f32`)
  - `0x10004301` (`Power_Starting_Level`, `f32`)
  - `0x100014C5` (`Power_RegenRate`, `f32` power regeneration rate per second).
- **War-Steed Mount Power:**
  - `0x10004933` (`MountPower_CurrentLevel`, `f32`), `0x10004937` (`MountPower_MaxLevel`, `f32`), `0x10004938` (`MountPower_Temporary`, `f32`), `0x10004932` (`MountPower_Temporary_Max`, `f32`).

### 4.2 Combat State (`CombatEvent::CombatState`, WLib Class `0x03E3`)
- **Wire Mechanism (`Body 0x076A9C`):** Synchronized via replicated properties on any entity:
  - **`0x10000EC9` (`CombatState_CurrentState`, `u32` bitmask):**
    - Default / Peace (out of combat): `0x00040000` (`in_combat == false`).
    - When `CombatState_CurrentState` has combat or crowd-control bits set (e.g., tested against masks `0x253A57F7`, `0x08040000`, `0x10000000`, `0x00010000` in `0x076B94` / `0x076E98`), the entity is in active combat or crowd-control state (`in_combat == true`).
  - `0x10000EC8` (`CombatState_CurrentContext`, `u32`).
  - `0x10001384` (`CombatState_CurrentStateStartTime`, `f64` server timestamp).
  - `0x1000232E` (`CombatState_CurrentStateInducer`, `u64` InstanceID of the entity that induced the state).

### 4.3 Effects — Buffs & Debuffs (`0x04F8` `CEffectSystem` & Native Class `0x0198`)

#### Exact Wire Layout of Native Class `0x0198` (`Effect`, `size = 0x48`, registered at `0x140B296AD`, descriptor `0x141CC4FD0`, serializer `0x140BB0F20` -> `0x140A58FB0`)
Inside the `WLibArgFrame` object record (after `u32 ref_count`, `u32 ref_id`, `u16 class_id = 0x0198`), `0x140A58FB0` serializes 15 fields in exact sequential order:
1. `u32 effect_instance_id` (`4B LE`, `[rdi+0x00]`, unique runtime instance ID on the target entity).
2. `u32 effect_did` (`4B LE`, `[rdi+0x04]`, DataID of the effect definition in `client_gamelogic.dat`).
3. `f32 duration` (`4B LE` IEEE-754 `f32`, `[rdi+0x08]`, total duration in seconds; `< 0.0` or `0.0` for permanent/passive effects).
4. `u8 flag_0c` (`1B` boolean `< 2`, `[rdi+0x0C]`).
5. `u8 flag_0d` (`1B` boolean `< 2`, `[rdi+0x0D]`).
6. `u8 flag_0e` (`1B` boolean `< 2`, `[rdi+0x0E]`).
7. `PString custom_name` (`[rdi+0x10]`, serialized via `0x140320A00`: `VLE char_count` + `u16[char_count]` UTF-16LE).
8. `PString custom_description` (`[rdi+0x18]`, serialized via `0x140320A00`: `VLE char_count` + `u16[char_count]` UTF-16LE).
9. `u32 category_or_flags` (`4B LE`, `[rdi+0x20]`).
10. `f64 application_time` (`8B LE` IEEE-754 `f64`, `[rdi+0x28]`, server timestamp when the effect was applied).
11. `u64 caster_entity_id` (`8B LE`, `[rdi+0x30]`, InstanceID of the caster).
12. `u64 source_id_or_context` (`8B LE`, `[rdi+0x38]`).
13. `u32 stack_count` (`4B LE`, `[rdi+0x40]`, current stack/tier count).
14. `u8 flag_0f` (`1B` boolean `< 2`, `[rdi+0x0F]`).
15. `u8 flag_44` (`1B` boolean `< 2`, `[rdi+0x44]`).

#### Local Player Effect RPCs (`0x04F8` `CEffectSystem`)
- **S→C `EffectApplied` (`0x84F8000F`, `body=0x0FE610`):**
  - `unpack_ref(effect_0x0198)` -> calls `0x082F88`, inserts into local hash table (`0x0075`) keyed by `effect_instance_id` (`0x0198000F`), and calls C++ native `0x0135001E` (`0x140B48DA0`).
- **S→C `EffectUpdated` (`0x84F80020`, `body=0x0FE6F0`):**
  - `unpack_ref(effect_0x0198)` -> calls `0x08312C` -> C++ native `0x01350026`.
- **S→C `EffectRemoved` (`0x84F80003`, `body=0x0FE538`):**
  - `unpack_32(u32 removal_flags)` (`4B LE`)
  - `unpack_32(u32 effect_instance_id)` (`4B LE`).
  - Calls `0x083010` -> removes `effect_instance_id` from table and calls C++ native `0x0135001F` (`0x140B48EA0`).
- **S→C `EffectRemovedSimple` (`0x84F8000A`, `body=0x0FE5B0`):**
  - `unpack_32(u32 effect_instance_id)` (`4B LE`).
- **S→C Bulk Effect Table Sync (`0x84F80031`, `body=0x0FE680`):**
  - `unpack_ref(hash_table_0x0075)` (maps `u32 effect_instance_id` -> `ref<0x0198>`).
- **C→S Cancel / Dismiss Dispellable Effect (`0x04F8`):**
  - Stub `0x84F80029` (`body=0x0FE760`, wire RPC ID **`0x84F8002A`**): `pack_32(u32 effect_instance_id)`.
  - Stub `0x84F8002B` (`body=0x0FE7B0`, wire RPC ID **`0x84F80006`**): `pack_32(u32 effect_instance_id)`.

#### Remote Entity / Target Effect Subscription & RPCs (`0x04F8` `CEffectListenerSystem`)
- **C→S Subscribe to Entity Effects:** Stub `0x84F80028` (`body=0x0FEACC`, wire RPC ID **`0x84F80024`**): `pack_64(u64 target_entity_id)`.
- **C→S Unsubscribe from Entity Effects:** Stub `0x84F8002A` (`body=0x0FEB1C`, wire RPC ID **`0x84F8001E`**): `pack_64(u64 target_entity_id)`.
- **S→C Remote `EffectApplied` (`0x84F80025`, `body=0x0FE9D0`):** `unpack_ref(effect_0x0198)`, `unpack_64(u64 target_entity_id)` -> `0x083B88` -> native `0x01350021`.
- **S→C Remote `EffectRemoved` (`0x84F80023`, `body=0x0FE8CC`):** `unpack_32(u32 removal_flags)`, `unpack_32(u32 effect_instance_id)`, `unpack_64(u64 target_entity_id)` -> `0x083BF4` -> native `0x01350022`.
- **S→C Remote `EffectsCleared` (`0x84F80019`, `body=0x0FE858`):** `unpack_64(u64 target_entity_id)` -> `0x083C5C` -> native `0x01350023`.
- **S→C Remote Bulk Effect Table (`0x84F80084`, `body=0x0FEA4C`):** `unpack_ref(hash_table_0x0075)`, `unpack_64(u64 target_entity_id)` -> `0x083B04` -> native `0x01350020`.
- **C++ Direct Effect Event (`ev_id = 0xFC`):** Handled at `0x140B9AD80` -> `0x140B02390` (reads `u64 entity_id` + `u32 code` and notifies UI effect listeners).

### 4.4 Combat Hit / Damage / Heal Deltas (`CombatEvent::VitalDelta`, `0x04E9`, `0x0537`, `0x065B` & `0x0662`)
- **S→C Combat Event RPCs on `0x04E9` (`CCombatSystem`):**
  1. **`0x84E9003D` (`body=0x0FAFD4`, Direct Combat Property Event):**
     - `unpack_ref(prop_collection_0x00A6)` -> calls `0x07361C` -> C++ native `0x07EC0001` (`0x140B3CA60` -> `0x140B0A4C0`, broadcasting event `*(u32*)0x141A30854` -> `0x141A30878` via `listener->vtable[0]`).
     - Registered listener `0x1416B1E08` (`vtable[0] = 0x140E66750` -> `0x140CDAFE0` -> `0x14042D310` / `0x140CDB130` / `0x140CDBD80`) copies the unpacked object as a **WLib Native `PropertyCollection`** (`Class 0x00A6` / `0x019E` / `0x01B1`, `size = 0x60`, vtable `0x1414F0630`, inner `HashTable<u32, PropertyValue>` at `+0x08`) and queries combat/event properties (`0x354`, `0x10001011`, `0x10000CBE`, `0x1000018E` `Combat_DamageType`, `0x1000051B` `Combat_HitType`, `0x1000019A` `Combat_FloatTextMagnitude`).
     - **Wire Serialization of `PropertyCollection` (`0x00A6` @ `0x1408384D0`, `0x019E` / `0x01B1` @ `0x140BB1020` -> `0x140A5CA70` -> `0x14043A0F0`):**
       1. Byte-align stream (`0x141168D50`).
       2. `u32 entry_count` (`4B LE`, `[r15+0x2C]`).
       3. `entry_count` pairs of:
          - `u32 property_id` (`4B LE`)
          - `PropertyValue value` (serialized via `0x140439770` according to `property_id`'s master property definition).
  2. **`0x84E9004F` (`body=0x0FB0F8`, Entity-Targeted Combat Property Event):**
     - `unpack_ref(prop_collection_0x00A6)`, `unpack_64(u64 entity_id)` -> calls `0x072D58` -> C++ native `0x07EC0007` (`0x140B3E330` -> `0x140B0C930`, broadcasting event `*(u32*)0x141A30874` -> `0x141A30880` via `listener->vtable[1](listener, entity_id, prop_collection)`).
- **S→C Vital Modification / Delta RPC (`0x85370003` on `0x0537` `CVitalSystem`, `body=0x110A60`) & WLib Script Class `0x0662`:**
  - `0x85370003` (`body=0x110A60`) calls `unpack_ref(vital_update_0x0662)` and invokes `0x1BAC58` -> `0x1BA9EC` -> `0x1BA020` (`Class 0x065B` `Vital`, `vslot 101`).
  - **Exact Wire Layout of WLib Script Class `0x0662` (`S = 9` scalar cells = `72` bytes, `0` ref cells, serialized via standard WLib script-object serializer `0x140376AA0` -> `0x140375EA0` -> `0x1403A2490`):**
    After `u32 ref_count = 1`, `u32 ref_id = 0`, `u16 class_id = 0x0662`, `u32 attr_count = 9`:
    - Cell `0` (`type = 3`, `f32`, `4B LE`): `max_value` (read by `vslot 7` `0x1BADF0`, passed to `0x01820001`).
    - Cell `1` (`type = 3`, `f32`, `4B LE`): `current_value` (read by `vslot 16` `0x1BAE40`, passed to `0x01820006`).
    - Cell `2` (`type = 3`, `f32`, `4B LE`): `regen_modifier_or_base` (read by `vslot 9` `0x1BAE30`, passed to `0x01820005`).
    - Cell `3` (`type = 3`, `f32`, `4B LE`): `regen_rate` (read by `vslot 11` `0x1BAE10`).
    - Cells `4..5` (`type = 130`, `u64`, `8B LE`): `source_or_context_id` (read by `vslot 2` `0x1BAE20`, passed to `0x01820003`).
    - Cell `6` (`type = 3`, `f32`, `4B LE`): `secondary_or_temp_value` (read by `vslot 17` `0x1BAE50`, passed to `0x01820007`).
    - Cell `7` (`type = 2`, `u32`, `4B LE`): `vital_type` (read by `vslot 5` `0x1BADE0`, passed to `0x01820004`: `1` = Morale, `2` = Power).
    - Cell `8` (`type = 3`, `f32`, `4B LE`): `base_max_value` (read by `vslot 6` `0x1BAE00`, passed to `0x01820002`).
  - When `Body 0x1BA020` processes a `0x0662` record whose `source_or_context_id != 0` and `current_value != old_value`, it computes `delta = old_value - current_value`, formats the combat log `StringInfo` (`0x00C7` with token `0x03FC4A23`, vital name `0x005AE85C`, and delta `0x04624A34`), updates the `Vital` (`vslot 103`, `109`, `94`, `99`), converts the `0x0662` record into a native `0x0182` struct (`0x1BAE60`), and calls C++ native `0x01350029` (`0x140B49730` -> `0x140B1DF40`) to emit `CombatEvent::VitalDelta`.

### 4.5 Defeat & Revive (`CombatEvent::Defeated`, `CombatEvent::Revived`, `0x04F2` & `0x083A`)
- **Replicated Defeat State Property (`Weenie_PropertyUpdate` `0x5B` / `0x19`):**
  - **`0x100001AF` (`Death_Incapacitated`, `u8` boolean):**
    - Monitored on every entity by WLib class `0x040A` (`Body 0x087144`).
    - Transition `0 -> 1` emits **`CombatEvent::Defeated { entity, ... }`**.
    - Transition `1 -> 0` emits **`CombatEvent::Revived { entity }`**.
  - Companion properties: `0x10003AD8` (`Death_ReviverId`, `u64` InstanceID), `0x10000CA2` (`Death_ShowDreadNoticeOnRevive`, `u8`).
- **S→C Death System RPCs (`0x04F2` `CDeathSystem`):**
  - `0x84F20012` (`body=0x0FC87C`): `unpack_f32(f32 release_timeout_seconds)` -> `0x07ECC8` -> native `0x01350007` (shows defeat/release dialog with countdown timer).
  - `0x84F20013` (`body=0x0FCA34`): `unpack_32`, `unpack_32`, `unpack_32` -> `0x07EE20` -> native `0x01350009`.
  - `0x84F2001C` (`body=0x0FC9B4`): `unpack_32`, `unpack_32` -> `0x07EDE0` -> native `0x01350008`.
  - `0x84F20003` (`body=0x0FCAD8`): `unpack_32(u8 flag)`, `unpack_32(u32 val)`, `unpack_f32(f32 timeout)` -> `0x07EEA8` -> native `0x0135000A`.
  - `0x84F20010` (`body=0x0FC950`): `unpack_32(u8 flag)` -> `0x07EDA0` -> native `0x01350016`.
  - `0x84F20004` (`body=0x0FCB6C`): `0 args` -> `0x07EF3C` -> native `0x01350011` (revive/clear death UI).
- **C→S Death System RPCs (`0x04F2`):**
  - **Release / Retreat to Rally Circle (`0x84F20008`):** Stub `0x84F20016` (`body=0x0FC838`, called by `0x07EC34` `0x01090002` @ `0x140B4CD43`) sends **wire RPC ID `0x84F20008`** with `0 arguments`.
  - **Accept / Decline Revive (`0x84F20001`):** Stub `0x84F20019` (`body=0x0FCBA8`, called by `0x07EF50` `0x0109000D` @ `0x140B4D0BF`) sends **wire RPC ID `0x84F20001`** with `pack_32(u8 accept_flag)`.
  - **Select Revive Option (`0x84F20026`):** Stub `0x84F2001A` (`body=0x0FCC00`, called by `0x07EF94` `0x0109000F` @ `0x140B4CE17`) sends **wire RPC ID `0x84F20026`** with `pack_32(u32 option_id)`.
- **Revive Prompt System (`0x083A`):**
  - S→C `0x883A0003` (`body=0x1C3AC4`): `unpack_f32(f32 morale_pct)`, `unpack_f32(f32 power_pct)`, `unpack_32(u32 skill_or_item_did)`, `unpack_64(u64 reviver_entity_id)`, `unpack_32(u32 prompt_id)`.
  - S→C `0x883A0005` (`body=0x1C3B68`): `unpack_64(u64 reviver_entity_id)` (cancels revive prompt).
  - C→S `0x883A0001` (stub `0x883A0006` @ `body=0x1C3BD8`): `pack_32(u8 accepted)`, `pack_64(u64 reviver_entity_id)`.

---

## 5. Clean-Room Spec Section (Ready for `docs/specs/official-messages-gameplay.md`)

### Provenance Table

| Method | Source | Scope | Confidence |
|---|---|---|---|
| `BIN` | `lotroclient64.exe` (build `4100.0061.8168.4126`, SHA-256 `b3022fb93dc6dca52bcd2fb3bdb3fbabb3cbfab3dbfcfbd8769f7b65b739c3db`) | WLib RPC wire framing (`0x30`), `WLibArgFrame` (`0x008E`), native class serializers (`0x0D85` SkillRequest, `0x01C6` Shortcut, `0x0198` Effect), induction progress bar handler | Confirmed |
| `DAT` | `client_general.dat` (`0x11000000` WLib bytecode image & `0x34000000` Master Property Definition Table) | WLib RPC wire IDs and argument pack/unpack sequences for `0x051E`, `0x04E9`, `0x0528`, `0x04EE`, `0x0958`, `0x04F8`, `0x04F2`, `0x083A`, and property IDs for selection, inductions, vitals, combat state, and incapacity | Confirmed |

---

### 5.1 WLib RPC Envelope (`0x30`) and `WLibArgFrame` (`0x008E`) Serialization <a id="wlib-rpc-envelope"></a>

All P2 gameplay commands (`C→S`) and non-property events (`S→C`) travel inside game-action event `0x30` (`Weenie_WLibRpc`), which wraps a 32-bit wire RPC identifier and a length-prefixed `WLibArgFrame` (`0x008E`) payload buffer.

#### Table 5.1 — `Weenie_WLibRpc` (`0x30`) Envelope Layout

| Offset | Size | Type | Name | Meaning | Byte Order |
|---|---|---|---|---|---|
| `0x00` | 1 | `u8` (VLE) | `ev_id` | Always `0x30` for standard WLib RPC events. | — |
| `0x01` | 0 or 8 | `u64` | `entity_id` | Present **only** if base event header bit 5 (`0x20`) is set. Omitted (`0 bytes`) on standard player-scoped gameplay RPCs. | Little-endian |
| `+0x00` | 4 | `u32` | `rpc_id` | Wire RPC identifier (`(class_id << 16) \| 0x80000000 \| method_wire_index`). | Little-endian |
| `+0x04` | 4 | `u32` | `arg_frame_len` | Byte length of the following `WLibArgFrame` payload (`0` if the RPC takes no arguments). | Little-endian |
| `+0x08` | `arg_frame_len` | `u8[]` | `arg_frame` | Sequential `WLibArgFrame` encoded arguments. | Little-endian |

#### Table 5.2 — `WLibArgFrame` (`0x008E`) Primitive and Object Graph Encodings

| Primitive / Type | Size | Wire Format |
|---|---|---|
| `pack_32` / `unpack_32` (`u32`, `i32`, `u8` bool) | 4 bytes | `u32` / `i32` little-endian (`u8` booleans are carried in the low byte of a 4-byte `u32`). |
| `pack_64` / `unpack_64` (`u64`) | 8 bytes | `u64` little-endian (used for 64-bit entity `InstanceID`s and 64-bit cooldown keys). |
| `pack_f32` / `unpack_f32` (`f32`) | 4 bytes | IEEE-754 binary32 little-endian. |
| `pack_f64` / `unpack_f64` (`f64`) | 8 bytes | IEEE-754 binary64 little-endian (used for server timestamps in seconds). |
| `pack_ref` / `unpack_ref` (Single non-null object) | `10 + N` bytes | `u32 ref_count = 1` (`4B LE`), `u32 ref_id = 0` (`4B LE`), `u16 class_id` (`2B LE`), followed by `N` bytes of class-specific payload. (If null: `u32 ref_count = 0` or `ref_id = 0xFFFFFFFF`). |

---

### 5.2 Target Selection (`CombatCommand::SelectTarget`, `CombatEvent::TargetChanged`) <a id="p2-targeting"></a>

#### Client-to-Server Target Selection (`0x851E001E` and `0x84E90008`)
1. **Direct Selection Change (`0x851E001E`):**
   - When the local player selects an entity or clears their selection, the client updates local properties `0x0000034F` (`Entity_Selection`, `u64`) and `0x000004B3` (`World_Selection`, `u64`) and sends wire RPC **`0x851E001E`**.
   - Client-only entities (where `(target_entity_id >> 48) & 0x03FF == 0x03FF`) must be sent as `0x0000000000000000` on the wire.
2. **Assisted / Validated Combat Target Request (`0x84E90008`):**
   - Rate-limited by property `0x10001B9A` (`Antispam_TargetRequest`). Sends a monotonically incrementing `u32 seq_token` and `u64 target_entity_id`.

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Payload Size | Meaning |
|---|---|---|---|---|
| `0x851E001E` | C→S | `u64 target_entity_id` | 8 bytes | Set local player's current target (`0` to clear selection). |
| `0x851E0012` | S→C | `u64 target_entity_id` | 8 bytes | Server-forced target selection change on the local player. |
| `0x84E90008` | C→S | `u32 seq_token`, `u64 target_entity_id` | 12 bytes | Request validated/assisted combat target selection. |
| `0x84E90001` | S→C | `u32 seq_token`, `u64 target_entity_id` | 12 bytes | Server response to `0x84E90008`; applied iff `seq_token` matches the latest request. |
| `0x87390015` | C→S | `u32 mark_index`, `u64 target_entity_id` | 12 bytes | Assign fellowship/raid target mark `mark_index` to `target_entity_id`. |
| `0x87390011` | C→S | `u32 mark_index`, `u64 target_entity_id` | 12 bytes | Clear fellowship/raid target mark `mark_index` from `target_entity_id`. |

#### Replicated Entity Target (`CombatEvent::TargetChanged`)
- Any entity's current target (used for Target-of-Target UI and remote entity facing) is replicated via property **`0x0000034F` (`Entity_Selection`, `u64` InstanceID)** in `Weenie_PropertyUpdate` (`0x5B`) or `0x19`. A value of `0` indicates no target.

---

### 5.3 Skill Execution, Inductions/Channels, and Cooldowns <a id="p2-skills-inductions-cooldowns"></a>

#### Client-to-Server Skill Execution (`0x85280005`) and Class `0x0D85` (`SkillRequest`)
To execute a skill (`CombatCommand::UseSkill`), the client increments a 32-bit sequence token, stores it in property `0x10002B84` (`Skill_QueueToken`), and sends wire RPC **`0x85280005`** containing a single `pack_ref` of native class **`0x0D85`**.

##### Table 5.3 — Native Class `0x0D85` (`SkillRequest`) Payload Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x0D85`)

| Offset | Size | Type | Name | Presence Condition & Meaning | Byte Order |
|---|---|---|---|---|---|
| `+0x00` | 4 | `u32` | `flags` | Bitmask of optional fields:<br>• Bit 0 (`0x01`): `sub_skill_did` present<br>• Bit 1 (`0x02`): `target_sub_index` present<br>• Bit 2 (`0x04`): boolean skill modifier flag (inline in bit 2; 0 extra bytes)<br>• Bit 3 (`0x08`): `target_entity_id` present<br>• Bit 4 (`0x10`): `ground_target` position present | Little-endian |
| `+0x04` | 4 | `u32` | `skill_did` | Always present. Skill DataID (`0x7000xxxx`). | Little-endian |
| `+0x08` | 4 | `u32` | `seq_token` | Always present. Client skill sequence token (stored in `0x10002B84`). | Little-endian |
| opt | 0 or 4 | `u32` | `sub_skill_did` | Present iff `(flags & 0x01) != 0`. Secondary/sub-skill DataID. | Little-endian |
| opt | 0 or 4 | `i32` | `target_sub_index` | Present iff `(flags & 0x02) != 0` (`>= 0`). | Little-endian |
| opt | 0 or 8 | `u64` | `target_entity_id` | Present iff `(flags & 0x08) != 0`. Explicit target entity `InstanceID`. | Little-endian |
| opt | 0 or var | `Position` | `ground_target` | Present iff `(flags & 0x10) != 0`. Ground target `Position` (`0x00A0`). | Little-endian |

#### Table 5.4 — Skill Execution, Cancellation, and Result RPCs (`0x0528`)

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Meaning |
|---|---|---|---|
| `0x85280005` | C→S | `ref<0x0D85> skill_request` | Execute skill (`CombatCommand::UseSkill`). |
| `0x85280006` | C→S | `u32 seq_token`, `u64 item_entity_id` | Execute an item's bound skill. |
| `0x85280008` | C→S | *(0 arguments)* | Cancel active induction/channel (`CombatCommand::CancelCast`). |
| `0x85280002` | C→S | *(0 arguments)* | Clear queued skill. |
| `0x85280007` | C→S | *(0 arguments)* | Reset/abort local skill execution state. |
| `0x8528000A` | S→C | `u32 result_code`, `u32 seq_token`, `u32 skill_did` | Skill execution result (`0` = success, updates `0x10001307`; non-zero = error, updates `0x10001410`). |
| `0x8528000E` | S→C | `u32 result_code`, `u32 seq_token`, `u64 item_entity_id` | Item skill execution result. |
| `0x8528000D` | S→C | `u32 err_code` | General skill failure notification. |
| `0x8528001A` | S→C | `u32 state_val` | Skill system state notification. |

#### Casts, Inductions, and Channeled Skills (`0x100067E7` `Skill_InductionAction_Struct`)
Inductions and channeled casts on any entity are driven by replicated property updates (`Weenie_PropertyUpdate` `0x5B` / `0x19`) carrying compound property **`0x100067E7` (`Skill_InductionAction_Struct`, type `0x0B` SubPropertyBag)**:

| Property ID | Name | Type | Role in Induction / Channel State Machine |
|---|---|---|---|
| `0x100067E7` | `Skill_InductionAction_Struct` | `SubPropertyBag` (`0x0B`) | Container property on the casting entity. |
| `0x100067D8` | `Skill_InductionAction_StartTime` | `f64` | Sub-property of `0x100067E7`: server timestamp (seconds) when the cast started. |
| `0x100067D9` | `Skill_InductionAction_EndTime` | `f64` | Sub-property of `0x100067E7`: server timestamp (seconds) when the cast finishes. |
| `0x10000729` | `Skill_DefaultSkill` | `u32` (DID) | Sub-property of `0x100067E7`: DataID of the outer skill being cast. |
| `0x10000C5C` | `Skill_InductionAction` | `u32` (DID) | Sub-property of `0x100067E7`: DataID of the induction/channel action record. |
| `0x100003C8` | `Induction_CurrentInductionState` | `i32` | Entity property: current induction state (`0` = idle). |
| `0x1000331D` | `Induction_CurrentInductionSkill` | `u32` (DID) | Entity property: DataID of the active induction skill. |
| `0x10000BC3` | `Channeling_CurrentChannelingDID` | `u32` (DID) | Entity property: DataID of the active channeling action. |
| `0x1000112C` | `Skill_CurrentChanneledSkill` | `u32` (DID) | Entity property: DataID of the active channeled skill. |
| `0x100011FA` | `Skill_CurrentChanneledSkillTarget` | `u64` (IID) | Entity property: target `InstanceID` of the active channeled skill. |

- **`CombatEvent::CastStarted`:** Emitted when `0x100067E7` updates on an entity (while no cast is active) with `0.0 < start_time < end_time` and `Skill_InductionAction != 0`. Total duration is `(end_time - start_time)` seconds.
- **`CombatEvent::CastDelayed`:** Emitted when `0x100067E7` updates on an entity that already has an active induction, shifting `start_time` and `end_time` forward by `delay_s = new_end_time - old_end_time`.
- **`CombatEvent::CastEnded`:** Emitted when `0x100067E7` updates with `start_time == 0.0` and `end_time == 0.0` (or `end_time <= start_time`), or when `0x100003C8` (`Induction_CurrentInductionState`) resets to `0`.

#### Cooldowns (`0x04EE` `CCooldownSystem` and Script Class `0x03F8`)
- **64-bit `cooldown_key` (`u64`) Format:**
  - `0x0001_0000_0000_0000 | (u64)skill_did`: Per-skill DataID cooldown.
  - `0x0002_0000_0000_0000 | (u64)cooldown_channel_id`: Shared cooldown channel (`0x1000090C` `Skill_CooldownChannel`).
- **Script Class `0x03F8` (`CooldownTimer`) Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x03F8`):**
  - `+0x00`: `f64 start_timestamp` (`8B LE` server timestamp in seconds).
  - `+0x08`: `f32 duration_s` (`4B LE` duration in seconds) + `u32 reserved_pad` (`4B LE`, `0`).

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Domain Event / Meaning |
|---|---|---|---|
| `0x84EE0008` | S→C | `ref<0x03F8> timer`, `u64 cooldown_key` | `CombatEvent::CooldownStarted { skill_id, duration_s }` |
| `0x84EE000A` | S→C | `u64 cooldown_key` | `CombatEvent::CooldownEnded { skill_id }` |
| `0x84EE0003` | S→C | *(0 arguments)* | Reset all active cooldowns on the local player. |
| `0x84EE0001` | S→C | `ref<0x006F> key_list` | Clear a list of `u64` cooldown keys. |

---

### 5.4 Quickslots (`CombatCommand::SetQuickslot`, `CombatEvent::QuickslotChanged`) <a id="p2-quickslots"></a>

The client maintains `84` quickslots (`slot_index = 0..83`, grouped into `7` pages of `12` slots; active page stored in property `0x10004CAA` `Shortcut_Current_Page`). Quickslot contents are serialized using native class **`0x01C6` (`Shortcut`)**.

#### Table 5.5 — Native Class `0x01C6` (`Shortcut`) Payload Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x01C6`)

| `shortcut_type` (`u32 LE` at `+0x00`) | Category | Type-Specific Trailing Bytes (`+0x04`) |
|---|---|---|
| `0` | Empty / Cleared | `0 bytes` |
| `2` | Inventory Item | `u64 item_entity_id` (`8B LE`) + `u32 item_did` (`4B LE`) (`12 bytes` total) |
| `3`, `4` | Chat Alias / Command | `PString text` (`VLE char_count` + `u16[char_count]` UTF-16LE) |
| `5`, `6`, `9`, `12` | Skill (`6`) / Action DID | `u32 target_did` (`4B LE`) |
| `7` | Value / Preset | `u32 extra_val` (`4B LE`) |
| `1`, `8`, `10`, `11` | Reserved | `0 bytes` |

#### Table 5.6 — Quickslot RPCs (`0x051E` and `0x0958`)

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Meaning |
|---|---|---|---|
| `0x851E0020` | C→S | `ref<0x01C6> shortcut`, `u32 slot_index` | `CombatCommand::SetQuickslot` (`slot_index` in `0..83`; use `shortcut_type = 0` to clear). |
| `0x89580005` | S→C | `ref<0x01C6> shortcut`, `u32 slot_index`, `u32 page_index` | `CombatEvent::QuickslotChanged` for a single slot. |
| `0x89580001` | S→C | `ref<0x01C6> shortcut`, `u32 slot_index`, `u32 mode`, `u32 page_index` | Extended single-slot quickslot update. |
| `0x8958000B` | S→C | `u32 target_val` | Remove matching shortcuts from quickslot bars. |
| `0x8958000D` | S→C | `u32 skill_did`, `u32 flags` | Remove/update skill shortcuts (`shortcut_type == 6`) matching `skill_did`. |
| `0x851E002C` | S→C | `ref page_data`, `u32 page_index` | Bulk quickslot page synchronization. |

---

### 5.5 Vitals, Combat State, Effects, Combat Deltas, and Defeat/Revive <a id="p2-vitals-effects-defeat"></a>

#### Vitals (`CombatEvent::VitalChanged`) and Combat State (`CombatEvent::CombatState`)
Vitals and combat state on all entities are updated via `Weenie_PropertyUpdate` (`0x5B`) and `0x19`:

| Subsystem | Property ID | Name | Wire Type | Notes |
|---|---|---|---|---|
| **Morale (`vital_type = 1`)** | `0x100012D2` | `Health_CurrentLevel` | `f32` | Clamped to `1.0` if `0.0` while `0x1000148A` (`Death_NeverSayDie`) is non-zero. |
| | `0x100008BB` | `Health_MaxLevel` | `f32` | Maximum morale. |
| | `0x10003F81` | `Health_Temporary` | `f32` | Temporary morale bubble (`0x10004064` = max temporary morale). |
| | `0x100008BA` | `Health_RegenRate` | `f32` | Morale regeneration rate (points/s). |
| **Power (`vital_type = 2`)** | `0x10000340` | `Power_CurrentLevel` | `f32` | Current power. |
| | `0x10000520` | `Power_MaxLevel` | `f32` | Maximum power. |
| | `0x10003F82` | `Power_Temporary` | `f32` | Temporary power bubble (`0x10004065` = max temporary power). |
| | `0x100014C5` | `Power_RegenRate` | `f32` | Power regeneration rate (points/s). |
| **Mount Power** | `0x10004933` | `MountPower_CurrentLevel` | `f32` | War-steed power (`0x10004937` = max). |
| **Combat State** | `0x10000EC9` | `CombatState_CurrentState` | `u32` | Bitmask: `0x00040000` = Peace (`in_combat = false`); combat/CC bits set = `in_combat = true`. |
| **Defeat State** | `0x100001AF` | `Death_Incapacitated` | `u8` (bool) | `0 -> 1` = `CombatEvent::Defeated`; `1 -> 0` = `CombatEvent::Revived`. |

#### Effects (`CombatEvent::EffectApplied`, `CombatEvent::EffectRemoved`) and Native Class `0x0198` (`Effect`)

##### Table 5.7 — Native Class `0x0198` (`Effect`) Payload Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x0198`)

| Order | Size | Type | Name | Meaning | Byte Order |
|---|---|---|---|---|---|
| 1 | 4 | `u32` | `effect_instance_id` | Runtime effect instance ID on the target entity. | Little-endian |
| 2 | 4 | `u32` | `effect_did` | Effect DataID in `client_gamelogic.dat`. | Little-endian |
| 3 | 4 | `f32` | `duration_s` | Total duration in seconds (`<= 0.0` if permanent). | Little-endian |
| 4 | 1 | `u8` | `flag_0c` | Boolean flag (`0` or `1`). | — |
| 5 | 1 | `u8` | `flag_0d` | Boolean flag (`0` or `1`). | — |
| 6 | 1 | `u8` | `flag_0e` | Boolean flag (`0` or `1`). | — |
| 7 | var | `PString` | `custom_name` | `VLE char_count` + `u16[char_count]` UTF-16LE (`0x00` if empty). | Little-endian |
| 8 | var | `PString` | `custom_desc` | `VLE char_count` + `u16[char_count]` UTF-16LE (`0x00` if empty). | Little-endian |
| 9 | 4 | `u32` | `category_flags` | Effect category/display flags. | Little-endian |
| 10 | 8 | `f64` | `application_time` | Server timestamp (seconds) when applied. | Little-endian |
| 11 | 8 | `u64` | `caster_entity_id` | Caster `InstanceID`. | Little-endian |
| 12 | 8 | `u64` | `source_context` | Source/context 64-bit ID. | Little-endian |
| 13 | 4 | `u32` | `stack_count` | Stack / tier count (`>= 1`). | Little-endian |
| 14 | 1 | `u8` | `flag_0f` | Boolean flag (`0` or `1`). | — |
| 15 | 1 | `u8` | `flag_44` | Boolean flag (`0` or `1`). | — |

##### Table 5.8 — Effect System RPCs (`0x04F8`)

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Meaning |
|---|---|---|---|
| `0x84F8000F` | S→C | `ref<0x0198> effect` | `CombatEvent::EffectApplied` on local player. |
| `0x84F80020` | S→C | `ref<0x0198> effect` | Update existing effect on local player. |
| `0x84F80003` | S→C | `u32 removal_flags`, `u32 effect_instance_id` | `CombatEvent::EffectRemoved` on local player. |
| `0x84F8000A` | S→C | `u32 effect_instance_id` | Remove effect on local player (no flags). |
| `0x84F80031` | S→C | `ref<0x0075> effect_table` | Bulk effect table sync on local player. |
| `0x84F8002A` | C→S | `u32 effect_instance_id` | Request cancellation of a dispellable buff on local player. |
| `0x84F80024` | C→S | `u64 target_entity_id` | Subscribe to remote entity effect updates. |
| `0x84F8001E` | C→S | `u64 target_entity_id` | Unsubscribe from remote entity effect updates. |
| `0x84F80025` | S→C | `ref<0x0198> effect`, `u64 target_entity_id` | `CombatEvent::EffectApplied` on subscribed remote entity. |
| `0x84F80023` | S→C | `u32 removal_flags`, `u32 effect_instance_id`, `u64 target_entity_id` | `CombatEvent::EffectRemoved` on subscribed remote entity. |
| `0x84F80019` | S→C | `u64 target_entity_id` | Clear all effects on subscribed remote entity. |
| `0x84F80084` | S→C | `ref<0x0075> effect_table`, `u64 target_entity_id` | Bulk effect table sync on subscribed remote entity. |

#### Combat Hit / Damage / Heal Events (`0x04E9`, `0x0537`, `0x0662`) and Defeat / Revive RPCs (`0x04F2`, `0x083A`)

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Meaning |
|---|---|---|---|
| `0x84E9003D` | S→C | `ref<0x00A6> prop_collection` | Direct combat property event (`PropertyCollection` `0x00A6` / `0x019E` / `0x01B1` serialized as `u32 entry_count` + `entry_count × (u32 property_id, PropertyValue)`). |
| `0x84E9004F` | S→C | `ref<0x00A6> prop_collection`, `u64 entity_id` | Entity-scoped combat property event (`PropertyCollection` `0x00A6` / `0x019E` / `0x01B1` + `u64 entity_id`). |
| `0x85370003` | S→C | `ref<0x0662> vital_update` | Vital modification / delta record (`CombatEvent::VitalDelta`); `Class 0x0662` has `u32 attr_count = 9` followed by `f32 max_value` (`cell 0`), `f32 current_value` (`cell 1`), `f32 regen_mod` (`cell 2`), `f32 regen_rate` (`cell 3`), `u64 source_or_context_id` (`cells 4..5`), `f32 secondary_value` (`cell 6`), `u32 vital_type` (`cell 7`: `1` = Morale, `2` = Power), `f32 base_max_value` (`cell 8`). |
| `0x84F20012` | S→C | `f32 release_timeout_s` | Defeat release countdown timer started. |
| `0x84F20004` | S→C | *(0 arguments)* | Clear defeat dialog on revive. |
| `0x84F20008` | C→S | *(0 arguments)* | Release / retreat to rally circle after defeat. |
| `0x84F20001` | C→S | `u8 accepted` (in `u32`) | Accept (`1`) or decline (`0`) revive prompt. |
| `0x84F20026` | C→S | `u32 option_id` | Select defeat/revive option. |
| `0x883A0003` | S→C | `f32 morale_pct`, `f32 power_pct`, `u32 did`, `u64 reviver_id`, `u32 prompt_id` | Display player-to-player revive prompt. |
| `0x883A0005` | S→C | `u64 reviver_id` | Cancel player-to-player revive prompt. |
| `0x883A0001` | C→S | `u8 accepted` (in `u32`), `u64 reviver_id` | Respond to player-to-player revive prompt. |

---

### 5.6 Synthetic Test Vectors <a id="p2-test-vectors"></a>

1. **C→S `SelectTarget` (`0x851E001E`) — Select Entity `0x0200000000001234`:**
   - Outer `0x30` payload (`17 bytes`):
     `30` (`ev_id = 0x30`)
     `1E 00 1E 85` (`rpc_id = 0x851E001E`)
     `08 00 00 00` (`arg_frame_len = 8`)
     `34 12 00 00 00 00 00 02` (`u64 target_entity_id = 0x0200000000001234`).

2. **C→S `UseSkill` (`0x85280005`) — Execute Skill `0x70003F10` with `seq_token = 7` on Target `0x0200000000001234`:**
   - Outer `0x30` payload (`39 bytes`):
     `30` (`ev_id = 0x30`)
     `05 00 28 85` (`rpc_id = 0x85280005`)
     `1E 00 00 00` (`arg_frame_len = 30`)
     `01 00 00 00` (`ref_count = 1`)
     `00 00 00 00` (`ref_id = 0`)
     `85 0D` (`class_id = 0x0D85`)
     `08 00 00 00` (`flags = 0x08`, `target_entity_id` present)
     `10 3F 00 70` (`skill_did = 0x70003F10`)
     `07 00 00 00` (`seq_token = 7`)
     `34 12 00 00 00 00 00 02` (`target_entity_id = 0x0200000000001234`).

3. **C→S `SetQuickslot` (`0x851E0020`) — Assign Skill `0x70003F10` (`shortcut_type = 6`) to Slot `0`:**
   - Outer `0x30` payload (`31 bytes`):
     `30` (`ev_id = 0x30`)
     `20 00 1E 85` (`rpc_id = 0x851E0020`)
     `16 00 00 00` (`arg_frame_len = 22`)
     `01 00 00 00` (`ref_count = 1`)
     `00 00 00 00` (`ref_id = 0`)
     `C6 01` (`class_id = 0x01C6`)
     `06 00 00 00` (`shortcut_type = 6`)
     `10 3F 00 70` (`target_did = 0x70003F10`)
     `00 00 00 00` (`slot_index = 0`).
