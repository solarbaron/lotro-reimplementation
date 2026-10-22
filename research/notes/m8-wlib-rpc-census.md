<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 WLib RPC & Gameplay NetBlobEvent Census

- **Status**: Draft
- **Last updated**: 2026-10-21
- **Author role**: Spec writer (dirty room)
- **Reviewer**: pending

## 1. Purpose & Provenance <a id="purpose-and-provenance"></a>

This dirty-room research note provides a complete, automated census of:
1. All **1,168 WLib RPC methods** (**619 Client->Server pack stubs** and **549 Server->Client unpack stubs**) across all **120 WLib RPC classes** in `client_gamelogic.dat` record `0x56000000` (`wlib.bin`).
2. The **Two-Class WLib RPC Dispatch Architecture** pairing each WLib RPC proxy/stub class with its client gameplay system class (`0x003C..0x16A0`), including all C++->WLib call sites (`0x1403cf2b0`), WLib->C++ native package handlers (`0x1403cfc30`), `0x0F` message strings, and `0x01`/`0x03` property IDs resolved against the master property directory (`0x34000000` / `pids.txt`).
3. All **198 registered top-level `NetBlobEvent` gameplay event factory registrations** (`0x1411a2f40`) and **172 distinct `NetBlobEvent` constructor IDs** (`0x141165b90`) in `x64/lotroclient64.exe`, together with the exact wire layouts of the WLib RPC and WState replication envelope events (`0x28`, `0x30`, `0x50`, `0x51`, `0x5F`, `0x64`, `0x81`, `0x8C`, `0xA0`, `0xC0`, `0xC9`, `0xCA`).

### Provenance Table <a id="provenance-table"></a>

| Method | Source Artifact | Version / SHA-256 | Tool | Question Answered | Why M1/M2 Alone Insufficient |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `M1 (OBS)` | `client_gamelogic.dat` record `0x56000000` (`wlib.bin`) | 5,976,473 bytes; 1,924 classes, 19,212 unique method bodies, 10,676 `MSG` strings | Custom Python bytecode parser (`analyze_all_rpc.py`) | Enumerate all 120 WLib RPC classes, 619 pack bodies, 549 unpack bodies, `WLibArgFrame` parameter sequences, `op 0x16` static call graph, `0x0F` strings, and `0x01`/`0x03` property IDs | Direct observation of on-disk bytecode record `0x56000000` |
| `M1 (OBS)` | `client_gamelogic.dat` record `0x34000000` (`pids.txt`) | 29,219 property definitions | Python parser | Resolve 32-bit property IDs (`0x00000000..0x10006FA1`) referenced in WLib bodies and C++ functions to canonical property names and types | Direct observation of on-disk MasterProperty definition record `0x34000000` |
| `M4 (BIN)` | `x64/lotroclient64.exe` | SHA-256 `b581fb970c0fd6bc8cbb2fdbeccfdce38ac8cd7de5baa8f5be4be6bc3ca6ac60` | Capstone / Python PE static analyzer (`deep_cpp_xref.py`) | Extract `0x008E`/`0x00D9`/`0x00D6` native handlers, C++->WLib call sites (`0x1403cf2b0`), `NetBlobEvent` factory registrations (`0x1411a2f40`), and `slot[7]` wire serializers | Native C++ handlers, C++->WLib call sites, and `NetBlobEvent` wire serializers exist only in compiled machine code |

---

## 2. WLib RPC Architecture & Wire Primitives <a id="wlib-rpc-architecture"></a>

### 2.1 Two-Class Dispatch Pattern (RPC Proxy Class vs. Client System Class) <a id="two-class-dispatch"></a>

In `wlib.bin`, WLib RPC methods are separated into dedicated **RPC proxy/stub classes** (`120` classes in total: `0x01F5`, `0x04D9..0x0537`, and `0x071A..0x169F`) distinct from the **client gameplay system classes** (`0x003C`, `0x00F6..0x01CE`, `0x06C1..0x06C5`, `0x071B..0x16A0`):

1. **Client -> Server (Pack / Send Path, 619 methods):**
   - C++ UI and gameplay code never invokes the 120 WLib RPC proxy classes directly. Instead, C++ invokes a method on the **paired client gameplay system class** (or `0x06C2` `CAdmin` / `0x06C3` `CPlayer`) via `WLib::CallClassMethod` (`0x1403cf2b0`), passing `(paired_class_id << 16) | fnum`.
   - The paired client system method performs client-side validation (e.g. antispam cooldown checks via `0x00E70001`, interaction lock checks, or argument formatting) and then executes a **static WLib call (`op 0x16`)** directly to the body offset of the corresponding pack stub in the RPC proxy class.
   - The RPC pack stub allocates a `WLibArgFrame` (`op 0x0E` `new.native(0x008E)`), pushes each parameter in order using `0x008E0001..0x008E0005`, pushes the 32-bit `wire_function_id` (`0x80000000 | (rpc_class_id << 16) | target_fnum`), and invokes one of the native RPC dispatchers `0x00D90001..0x00D9000D` (`0x1403a6a80` -> `0x140520df0`).
2. **Server -> Client (Unpack / Receive Path, 549 methods):**
   - When the client receives a WLib RPC `NetBlobEvent` (`0x30`, `0x5F`, or `0xC0`), its `Dispatch` virtual method (`slot[1]`: `0x14051c160`, `0x14051c420`, or `0x14051bc60`) extracts `wire_function_id = 0x80000000 | (rpc_class_id << 16) | fnum` and resolves the unpack stub in `rpc_class_id`.
   - Every unpack stub begins by calling `0x00D60007(self, iface_class_id)` (`0x14039b570`), which verifies and casts the target entity reference (`self`) to the expected client entity/interface class (most commonly `0x06C3` `CPlayer`, `0x06C2` `CAdmin`, `0x06C1` `CAgent`, or `0x06C5` `CAvatar`). If the cast returns zero, the stub logs error property `0x003C` and returns immediately.
   - Next, the unpack stub calls `0x008E000D(arg_frame)` (`0x14038ef80`) to reset the `WLibArgFrame` read cursor to byte offset `0`, pops each argument in wire order using `0x008E0006..0x008E000A`, and dispatches (`op 0x16` static call or `op 0x14` virtual call) to the client handler method on the paired gameplay system class.

### 2.2 `WLibArgFrame` (`0x008E`) Native Package Primitives <a id="wlib-arg-frame-primitives"></a>

Native class `0x008E` (`WLibArgFrame`, size `0x20` bytes, vtable `0x1414db6b8`, registered at `0x1403a6830`) wraps an inner `SmartBuffer` (`+0x08..+0x18`) and a 32-bit byte read cursor (`+0x1C`). When serialized inside a `NetBlobEvent` (`0x140367e00` -> `0x140352750`), the frame is written on the wire as a **32-bit little-endian `byte_length` (`u32`)** followed immediately by `byte_length` raw bytes of the inner `SmartBuffer`.

| Native ID | C++ Handler VA | Direction | Parameter Type | Wire Encoding inside `WLibArgFrame` |
| :--- | :--- | :--- | :--- | :--- |
| `0x008E0001` | `0x14039fc60` | Pack (Write) | `ref` (WLib Object / Native Instance Graph) | `WLib::PackObjectRef` (`0x140367d20` -> `0x140361630`): serializes reachable object graph (`0x14035fcf0` -> `0x140363520` -> `0x140363a00`); see subsection below |
| `0x008E0002` | `0x14039fa30` | Pack (Write) | `i32 / u32` (32-bit integer / DID / enum / bool) | 4-byte little-endian `u32` |
| `0x008E0003` | `0x14039fba0` | Pack (Write) | `i64 / u64` (64-bit integer / `InstanceID`) | 8-byte little-endian `u64` |
| `0x008E0004` | `0x14039fae0` | Pack (Write) | `f32` (32-bit IEEE-754 float) | 4-byte little-endian `f32` |
| `0x008E0005` | `0x14039fa90` | Pack (Write) | `f64` (64-bit IEEE-754 double / timestamp) | 8-byte little-endian `f64` |
| `0x008E0006` | `0x1403b4070` | Unpack (Read) | `ref` (WLib Object / Native Instance Graph) | `WLib::UnpackObjectRef` (`0x140368460` -> `0x140364cd0`): all 279 unpack call sites pass `allow_flag = 0` (`op 0x05`); deserializes object graph (`0x140363520` -> `0x140363b80`) and relinks handles (`0x140364fe0` / `0x1403651b0`) |
| `0x008E0007` | `0x1403b3dc0` | Unpack (Read) | `i32 / u32` (32-bit integer / DID / enum / bool) | Reads 4-byte little-endian `u32` at `[frame + 0x1C]`, adds `4` to `[frame + 0x1C]` |
| `0x008E0008` | `0x1403b3f90` | Unpack (Read) | `i64 / u64` (64-bit integer / `InstanceID`) | Reads 8-byte little-endian `u64` at `[frame + 0x1C]`, adds `8` to `[frame + 0x1C]` |
| `0x008E0009` | `0x1403b3ce0` | Unpack (Read) | `f32` (32-bit IEEE-754 float) | Reads 4-byte little-endian `f32` at `[frame + 0x1C]`, adds `4` to `[frame + 0x1C]` |
| `0x008E000A` | `0x1403b3b90` | Unpack (Read) | `f64` (64-bit IEEE-754 double / timestamp) | Reads 8-byte little-endian `f64` at `[frame + 0x1C]`, adds `8` to `[frame + 0x1C]` |
| `0x008E000B` | `0x14039fb30` | Pack (Copy) | `frame` (`WLibArgFrame` append) | Appends another `WLibArgFrame` buffer |
| `0x008E000C` | `0x14038ec30` | Lifecycle | Destroy / Release `WLibArgFrame` | Releases inner `SmartBuffer` |
| `0x008E000D` | `0x14038ef80` | Unpack (Reset) | Reset read cursor | Sets `dword ptr [frame + 0x1C] = 0` before unpacking parameters |

#### `0x008E0001` / `0x008E0006` (`ref`) Object-Graph Wire Format (`0x140361630` / `0x140364cd0`) <a id="wlib-ref-wire-format"></a>

1. **Graph Traversal (`0x14035fcf0`)**: Starting from `root_ref_id` (where `0xFFFFFFFF` denotes `null`), the serializer traverses all reachable WLib object and native package references, recording `N` unique 32-bit handle IDs in `node_ids[0 .. N-1]` (with `node_ids[0] = root_ref_id`, or `N = 0` if `root_ref_id == 0xFFFFFFFF`).
2. **Node Handle Table (`0x140363520`)**: Writes/reads the packed count `N` (`0x140304d00`, maximum `0x7FFF0000`) followed by `N` 4-byte little-endian `u32` handle IDs (`node_ids[0 .. N-1]`). If `N == 0`, no node bodies follow and `0x008E0006` returns `0xFFFFFFFF`.
3. **Per-Node Body (`0x140363a00` / `0x140363b80` -> `0x140363910`)**: For each node `i` in `0 .. N-1`:
   - Writes/reads the 32-bit node header word `u32 node_flags` (`0x1403630b0` / `0x1403573e0`).
   - If `bit 29` (`0x20000000`, external/singleton reference) is set in `node_flags`: writes/reads a 4-byte `u32` external identifier (`0x1403698e0` if `bit 28` is set, or sentinel `[0x141e51d00]`).
   - If `bit 29` (`0x20000000`) is clear: writes/reads the 16-bit WLib class or native package ID (`u16 class_id = word ptr [[node + 0x08]]`), preceded by singleton/entity binding metadata (`0x1403638b0` / `0x140363150`) if `bit 27` (`0x08000000`) is set. Then:
     - **Pure WLib Class Instance (`bit 28` = `0x10000000` clear)**: `field_count = (node_flags >> 8) & 0xFFFF`; copies `field_count * 8` raw bytes of the object's 64-bit member slots (`[node + 0x10]`), and on unpack relinks reference-typed member slots (`[class_desc + 0x10]`) via `0x140364fe0` / `0x1403651b0`.
     - **Native Package Instance (`bit 28` = `0x10000000` set)**: invokes the native package's registered `PackUnpack` callback at `qword ptr [[node + 0x08] + 0x68](node + 0x18, smart_buffer)` (e.g. `0x00E1` `WString`, `0x0027` `PropertyCollection`, `0x00A9` `WArray`, `0x0011`/`0x0019`/`0x00B0` containers).

### 2.3 WLib RPC Native Dispatchers (`0x00D9`, `0x00D6`, `0x00DF`) <a id="wlib-rpc-dispatchers"></a>

| Native ID | C++ Handler VA | Inner Target VA | Role & Calling Convention |
| :--- | :--- | :--- | :--- |
| `0x00D90001..0x00D9000B` | `0x1403a6b00` (`0x1403a6a80` for `0x00D90005`) | `0x140520eb0` / `0x140520df0` | Server/targeted RPC dispatchers (unused in client pack stubs except `0x00D9000C` / `0x00D9000D`) |
| `0x00D9000C` | `0x1403a6a80` | `0x140520df0` | **Client->Server RPC Dispatch (`self`)**: pops `(self_entity, wire_function_id, arg_frame_ref)` and queues outbound RPC event for `self_entity` (used by **567** of 619 pack stubs) |
| `0x00D9000D` | `0x1403a6a80` | `0x140520df0` | **Client->Server RPC Dispatch (`local_0`)**: identical C++ handler to `0x00D9000C`; used by **52** of 619 pack stubs (primarily `0x04DC` / `0x0508` / `0x0511` / `0x052D` / `0x0FF6` admin stubs) |
| `0x00D60007` | `0x14039b570` | `0x14039b570` | **Unpack Entity Interface Cast**: `CastEntityToClass(entity_ref, target_class_id)` — called at the entry of all **549** unpack stubs |
| `0x00DF0001` | `0x1403867e0` | `0x1403867e0` | **Entity Lookup by 64-bit `InstanceID`**: resolves a `u64` entity ID unpacked via `0x008E0008` into a live client entity reference |
| `0x00DF0005` | `0x14039b0b0` | `0x14039b0b0` | **Entity Lookup / Validation Helper**: resolves entity reference during unpack handlers |

---

## 3. Top-Level `NetBlobEvent` Gameplay Event Census <a id="netblob-event-census"></a>

### 3.1 WLib RPC & WState Envelope `NetBlobEvent` Wire Formats <a id="wlib-rpc-envelope-wire-formats"></a>

All gameplay `NetBlobEvent` classes inherit from `NetBlobEvent` (`0x141165b90`, base vtable `0x1417d9a70`). Every `NetBlobEvent::PackUnpackPayload` (`vtable[7]`, offset `+0x38`) first calls `NetBlobEvent::PackUnpackHeader` (`0x1411a3840`), which serializes the **low byte (`u8`) of `[this + 0x14]` (`flags & 0xFF`)**.

#### `RpcTransactionId` Wire Structure (`0x1405197c0`, 25 bytes) <a id="rpc-transaction-id-layout"></a>

Transactional WLib RPC events (`0x28`, `0x50`, `0xA0`, `0xC0`, `0xC9`, `0xCA`) embed a 25-byte transaction identifier serialized by `0x1405197c0`:

| Wire Offset | Size (Bytes) | Type | Struct Offset | Field Meaning | Byte Order |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `0x00` | `1` | `u8` | `+0x00` | `tx_type` (transaction type / scope discriminator) | N/A |
| `0x01` | `8` | `u64` | `+0x08` | `source_entity_id` (64-bit `InstanceID` A) | Little-endian |
| `0x09` | `4` | `u32` | `+0x10` | `request_sequence_id` (32-bit transaction sequence number) | Little-endian |
| `0x0D` | `2` | `u16` | `+0x14` | `sub_id_a` (16-bit routing / channel qualifier A) | Little-endian |
| `0x0F` | `2` | `u16` | `+0x16` | `sub_id_b` (16-bit routing / channel qualifier B) | Little-endian |
| `0x11` | `8` | `u64` | `+0x18` | `target_entity_id` (64-bit `InstanceID` B) | Little-endian |

#### WLib RPC & WState Envelope Events Summary Table <a id="wlib-envelope-events-table"></a>

| Event ID | Dec | Object Size | Flags (`+0x14`) | Vtable VA | `Dispatch` (`slot[1]`) | `PackUnpack` (`slot[7]`) | Payload Layout (after 1-byte `PackUnpackHeader` `u8`) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x28` | `40` | `0x50` | `0x00010028` | `0x14150d848` | `0x14051c8a0` | `0x140522b60` | **Transactional WLib RPC Status Reply**: `RpcTransactionId` (25 B at `+0x28`) + `u32 status_code` (`+0x4C`) + `u8 status_flag` (`+0x48`) |
| `0x30` | `48` | `0x38` | `0x00000030` | `0x14150d598` | `0x14051c160` | `0x140522950` | **Standard WLib RPC Call**: `u32 wire_function_id` (`+0x28`) + `WLibArgFrame` (`+0x30`: `u32 byte_len` + `byte_len` bytes) |
| `0x50` | `80` | `0x50` | `0x00000050` | `0x14150d658` | `0x14051ba50` | `0x1405227e0` | **Transactional WLib RPC Return Frame Reply**: `RpcTransactionId` (25 B at `+0x28`) + `WLibArgFrame` (`+0x48`: `u32 byte_len` + `byte_len` bytes) |
| `0x51` | `81` | `0x30` | `0x00000051` | `0x14150d808` | `0x14051c5f0` | `0x140522af0` | **WLib RPC 32-bit Control / Ack Event**: `u32 control_word` (`+0x28`) |
| `0x5F` | `95` | `0x40` | `0x0000005F` | `0x14150d5d8` | `0x14051c420` | `0x140522a90` | **Timestamped WLib RPC Call** (inherits `0x30`): `u32 wire_function_id` (`+0x28`) + `WLibArgFrame` (`+0x30`) + `f64 timestamp` (`+0x38`) |
| `0x64` | `100` | `0x38` | `0x00010064` | `0x14150da30` | `0x14051b850` | `0x140522590` | **WState Property Delta Sync**: `u8 bool is_clear` (`+0x2C`) + `u8 bool flag_2d` (`+0x2D`) + `u32 seq` (`+0x28`) + if `!is_clear`: `WState` property hash table (`+0x30`, via `0x140367e90`) |
| `0x81` | `129` | `0x30` | `0x00000081` | `0x14152fee0` | `0x14051b980` | `0x1405226e0` | **WState Property Table Sync A**: `u8 bool has_table` + if `has_table`: `WState` property hash table (`+0x28`, via `0x140367e90`) |
| `0x8C` | `140` | `0x30` | `0x0000008C` | `0x14152ff28` | `0x14051b980` | `0x1405226e0` | **WState Property Table Sync B**: identical `Dispatch` and `PackUnpack` to `0x81` (`u8 bool has_table` + optional `WState` property hash table) |
| `0xA0` | `160` | `0x48` | `0x000000A0` | `0x1414f7bd8` | `0x14051c6f0` | `0x1405228b0` | **Transactional WLib RPC Control A**: `RpcTransactionId` (25 B at `+0x28`) |
| `0xC0` | `192` | `0x58` | `0x000000C0` | `0x14150d618` | `0x14051bc60` | `0x140522900` | **Transactional WLib RPC Request** (inherits `0x30`): `u32 wire_function_id` (`+0x28`) + `WLibArgFrame` (`+0x30`) + `RpcTransactionId` (25 B at `+0x38`) |
| `0xC9` | `201` | `0x48` | `0x000000C9` | `0x14150d698` | `0x14051bb60` | `0x1405228b0` | **Transactional WLib RPC Control B**: `RpcTransactionId` (25 B at `+0x28`) |
| `0xCA` | `202` | `0x48` | `0x000100CA` | `0x14150d7c8` | `0x14051caf0` | `0x1405228b0` | **Transactional WLib RPC Control C**: `RpcTransactionId` (25 B at `+0x28`) |

### 3.2 Complete Census of All 198 Registered `NetBlobEvent` Factory Calls (`0x1411a2f40`) <a id="registered-netblob-factories"></a>

The following table lists all **198** registrations calling `NetBlobEventFactory::Register` (`0x1411a2f40`) in `x64/lotroclient64.exe`, ordered by `evt_id` (`blobHeaderWord`):

| Event ID (Hex) | Event ID (Dec) | Object Size | Initial Flags (`+0x14`) | Registration Site VA | Factory Thunk VA | Vtable VA | `Dispatch` (`slot[1]`) | `PackUnpack` (`slot[7]`) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x0001` | `1` | `0x28` | `0x300400` | `0x140228470` | `0x140529650` | `0x14150ea78` | `0x14052a130` | `0x140355120` |
| `0x000E` | `14` | `None` | `0x300400` | `0x14080eb85` | `None` | `0x141549c20` | `0x14080c210` | `0x140354480` |
| `0x0015` | `21` | `None` | `0x300400` | `0x14080d7e0` | `None` | `0x141549000` | `0x14080afc0` | `0x140354540` |
| `0x0016` | `22` | `0x30` | `0x300400` | `0x140229fb0` | `0x140581aa0` | `0x141515670` | `0x1405830b0` | `0x140522af0` |
| `0x0017` | `23` | `None` | `0x300400` | `0x14080e653` | `None` | `0x1415498a0` | `0x14080cb30` | `0x140355340` |
| `0x0018` | `24` | `None` | `0x300400` | `0x14080e4eb` | `None` | `0x141549760` | `0x14080c770` | `0x140355210` |
| `0x001B` | `27` | `None` | `0x300400` | `0x14080e57b` | `None` | `0x1415497e0` | `0x14080c8f0` | `0x140354480` |
| `0x001C` | `28` | `0x38` | `0x300400` | `0x140304490` | `0x1411aa2b0` | `0x141709338` | `0x1411aa340` | `0x1411aa810` |
| `0x001D` | `29` | `None` | `0x300400` | `0x14080e0fa` | `None` | `0x1415499a0` | `0x14080cef0` | `0x140355440` |
| `0x001E` | `30` | `0x60` | `0x300400` | `0x14022ad00` | `0x14059a910` | `0x141516818` | `0x14059df30` | `0x1405a5eb0` |
| `0x0025` | `37` | `0x30` | `0x300400` | `0x1402a8b30` | `0x140bc7b20` | `0x14159b548` | `0x140068cc0` | `0x1411a3840` |
| `0x0028` | `40` | `0x50` | `0x300400` | `0x1402280c0` | `0x14051abc0` | `0x14150d848` | `0x14051c8a0` | `0x140522b60` |
| `0x002F` | `47` | `0x38` | `0x300400` | `0x140229820` | `0x14056ed80` | `0x141513788` | `0x14056ef40` | `0x14056f740` |
| `0x0030` | `48` | `0x38` | `0x300400` | `0x140227fa0` | `0x14051ac60` | `0x14150d598` | `0x14051c160` | `0x140522950` |
| `0x0031` | `49` | `0x28` | `0x300400` | `0x14022ad60` | `0x14059a9b0` | `0x141516898` | `0x14059e110` | `0x1405a60b0` |
| `0x0032` | `50` | `0x70` | `0x300400` | `0x14022ac70` | `0x14059aa00` | `0x1414f05a0` | `0x140372fb0` | `0x140373700` |
| `0x0038` | `56` | `0x30` | `0x300400` | `0x140226c90` | `0x1404f3390` | `0x14150a198` | `0x1404f3750` | `0x1404f3d60` |
| `0x0039` | `57` | `None` | `0x300400` | `0x14080e1d3` | `None` | `0x141549490` | `0x14080b360` | `0x1403546d0` |
| `0x003A` | `58` | `0x38` | `0x300400` | `0x140229480` | `0x1405639d0` | `0x141512730` | `0x140564f80` | `0x14056c760` |
| `0x003C` | `60` | `0x58` | `0x300400` | `0x14022abb0` | `0x14059aae0` | `0x141516858` | `0x14059d320` | `0x1405a5570` |
| `0x003D` | `61` | `0x48` | `0x300400` | `0x140229f80` | `0x140581b20` | `0x1415153f0` | `0x140583030` | `0x140589530` |
| `0x003E` | `62` | `0x58` | `0x300400` | `0x14022a0d0` | `0x140581bc0` | `0x141515470` | `0x1405833f0` | `0x140589b60` |
| `0x0040` | `64` | `None` | `0x300400` | `0x14080e2ab` | `None` | `0x141549550` | `0x14080bcb0` | `0x1411a3840` |
| `0x0043` | `67` | `None` | `0x300400` | `0x14080d6b8` | `None` | `0x141548f80` | `0x14080cfb0` | `0x140355140` |
| `0x0046` | `70` | `0x30` | `None` | `0x1402262c0` | `0x140484860` | `0x1415046d0` | `0x1404f3750` | `0x1404f3d60` |
| `0x0046` | `70` | `0x30` | `None` | `0x1402500b0` | `0x140484860` | `0x1415046d0` | `0x1404f3750` | `0x1404f3d60` |
| `0x0047` | `71` | `0x30` | `0x300400` | `0x140226c60` | `0x1404f3430` | `0x14150a148` | `0x1404f3750` | `0x1404f3d60` |
| `0x0048` | `72` | `0x38` | `0x300400` | `0x140229390` | `0x140563a60` | `0x1415127b0` | `0x140564a90` | `0x14056c710` |
| `0x0049` | `73` | `0x78` | `0x300400` | `0x14022abe0` | `0x14059ab90` | `0x1414f05a0` | `0x140372fb0` | `0x140373700` |
| `0x004A` | `74` | `0x48` | `0x300400` | `0x14022ac40` | `0x14059ac70` | `0x141516798` | `0x14059da50` | `0x1405a5760` |
| `0x004C` | `76` | `0x48` | `0x300400` | `0x14024d060` | `0x140765d70` | `0x14153d280` | `0x140068cc0` | `0x140355470` |
| `0x0050` | `80` | `0x50` | `0x300400` | `0x140227f10` | `0x14051acf0` | `0x14150d658` | `0x14051ba50` | `0x1405227e0` |
| `0x0051` | `81` | `0x30` | `0x300400` | `0x140228060` | `0x14051ad90` | `0x14150d808` | `0x14051c5f0` | `0x140522af0` |
| `0x0052` | `82` | `None` | `0x300400` | `0x14080ebcd` | `None` | `0x141549c60` | `0x14080c5f0` | `0x1403549e0` |
| `0x0054` | `84` | `None` | `0x300400` | `0x14080dd38` | `None` | `0x1415492c0` | `0x14080bd30` | `0x140354c30` |
| `0x0057` | `87` | `0x38` | `None` | `0x1402500e0` | `0x1407aaee0` | `0x141542770` | `0x1404f3750` | `0x1407f6d80` |
| `0x0058` | `88` | `0x48` | `0x300400` | `0x1402293f0` | `0x140563af0` | `0x141512770` | `0x140564cd0` | `0x14056c7b0` |
| `0x0059` | `89` | `0x38` | `0x300400` | `0x14022a7b0` | `0x140593480` | `0x141516000` | `0x1405943b0` | `0x140594f90` |
| `0x005A` | `90` | `None` | `0x300400` | `0x14080e383` | `None` | `0x141549620` | `0x14080c070` | `0x140354ed0` |
| `0x005E` | `94` | `0x40` | `None` | `0x140226320` | `0x1404848c0` | `0x141504558` | `0x1404f3750` | `0x1404cc3d0` |
| `0x005F` | `95` | `0x40` | `0x300400` | `0x140228030` | `0x14051ae20` | `0x14150d5d8` | `0x14051c420` | `0x140522a90` |
| `0x0060` | `96` | `None` | `0x300400` | `0x14080e21b` | `None` | `0x1415494d0` | `0x14080b420` | `0x1403546d0` |
| `0x0062` | `98` | `0x38` | `0x300400` | `0x140250140` | `0x1407aaf40` | `0x141545a08` | `0x1407b1500` | `0x1407f6970` |
| `0x0063` | `99` | `None` | `0x300400` | `0x14080e6e3` | `None` | `0x141549920` | `0x14080cd70` | `0x1403553e0` |
| `0x0064` | `100` | `0x38` | `0x300400` | `0x140227ee0` | `0x14051aeb0` | `0x14150da30` | `0x14051b850` | `0x140522590` |
| `0x0065` | `101` | `0x58` | `0x300400` | `0x140229360` | `0x140563b90` | `0x141513008` | `0x140564970` | `0x14056c610` |
| `0x0066` | `102` | `0x190` | `0x300400` | `0x14022ac10` | `0x14059ad10` | `0x141516990` | `0x14059d590` | `0x1405a5670` |
| `0x006B` | `107` | `None` | `0x300400` | `0x14080df31` | `None` | `0x141549240` | `0x14080b1f0` | `0x1403545c0` |
| `0x006C` | `108` | `None` | `0x300400` | `0x14080dee9` | `None` | `0x141549200` | `0x14080af10` | `0x140354500` |
| `0x006D` | `109` | `None` | `0x300400` | `0x14080de11` | `None` | `0x141549140` | `0x14080ad00` | `0x140354480` |
| `0x0070` | `112` | `0x40` | `0x300400` | `0x1402a8a40` | `0x140bc7ba0` | `0x14159b508` | `0x140068cc0` | `0x140bcef10` |
| `0x0072` | `114` | `None` | `0x300400` | `0x14080e263` | `None` | `0x141549510` | `0x14080b4e0` | `0x140354840` |
| `0x0073` | `115` | `None` | `0x300400` | `0x14080e45b` | `None` | `0x1415496e0` | `0x14080c530` | `0x140355170` |
| `0x0074` | `116` | `None` | `0x300400` | `0x14080e413` | `None` | `0x1415496a0` | `0x14080c3a0` | `0x140354f20` |
| `0x0076` | `118` | `None` | `0x300400` | `0x14080e9d5` | `None` | `0x141549ae0` | `0x14080b860` | `0x140354480` |
| `0x0079` | `121` | `None` | `0x300400` | `0x14080de59` | `None` | `0x141549180` | `0x14080adb0` | `0x140354400` |
| `0x007B` | `123` | `0x88` | `0x300400` | `0x14022ad30` | `0x14059ada0` | `0x141516f08` | `0x14059e070` | `0x1405a5fb0` |
| `0x007D` | `125` | `0x30` | `0x300400` | `0x140226cf0` | `0x1404f34d0` | `0x14150a1e8` | `0x1404f3750` | `0x1404f3d60` |
| `0x0080` | `128` | `0x60` | `0x300400` | `0x14022b840` | `0x1405c1580` | `0x141519898` | `0x1405c24f0` | `0x1405c7910` |
| `0x0081` | `129` | `0x30` | `None` | `0x140240720` | `0x140712240` | `0x14152fee0` | `0x14051b980` | `0x1405226e0` |
| `0x0086` | `134` | `None` | `0x300400` | `0x14080e69b` | `None` | `0x1415498e0` | `0x14080ccb0` | `0x140355370` |
| `0x0088` | `136` | `None` | `0x300400` | `0x14080ec15` | `None` | `0x141549ca0` | `0x14080cbf0` | `0x140354a70` |
| `0x0089` | `137` | `None` | `0x300400` | `0x14080e72b` | `None` | `0x141549960` | `0x14080ce30` | `0x140355410` |
| `0x008B` | `139` | `0x88` | `0x300400` | `0x14022a040` | `0x140581c70` | `0x1414f0640` | `0x140311fa0` | `0x1400015c0` |
| `0x008C` | `140` | `0x30` | `None` | `0x1402406f0` | `0x1407122a0` | `0x14152ff28` | `0x14051b980` | `0x1405226e0` |
| `0x008E` | `142` | `0x30` | `0x300400` | `0x140253920` | `0x140809d30` | `0x141548de0` | `0x14080bf50` | `0x140354400` |
| `0x008F` | `143` | `0xb8` | `0x300400` | `0x14022acd0` | `0x14059ae60` | `0x1414f05a0` | `0x140372fb0` | `0x140373700` |
| `0x0090` | `144` | `None` | `0x300400` | `0x14080e33b` | `None` | `0x1415495e0` | `0x14080bea0` | `0x140354740` |
| `0x0092` | `146` | `None` | `0x300400` | `0x14080e86c` | `None` | `0x141549ca0` | `0x14080cbf0` | `0x140354a70` |
| `0x0093` | `147` | `None` | `0x300400` | `0x14080dea1` | `None` | `0x1415491c0` | `0x14080ae60` | `0x1403544d0` |
| `0x0096` | `150` | `0x28` | `0x300400` | `0x140229420` | `0x140563c30` | `0x141512830` | `0x140564dc0` | `0x140355120` |
| `0x0098` | `152` | `0x40` | `0x300400` | `0x140253950` | `0x140809f90` | `0x141548da0` | `0x140068cc0` | `0x140355a80` |
| `0x009F` | `159` | `None` | `0x300400` | `0x14080eaf5` | `None` | `0x141549be0` | `0x14080c130` | `0x140354f20` |
| `0x00A0` | `160` | `0x48` | `0x300400` | `0x140228090` | `0x14051af50` | `0x1414f7bd8` | `0x14051c6f0` | `0x1405228b0` |
| `0x00A1` | `161` | `0x48` | `0x300400` | `0x14024c830` | `0x14075db80` | `0x14153b570` | `0x1405942e0` | `0x140594f50` |
| `0x00A2` | `162` | `0x38` | `0x300400` | `0x140229fe0` | `0x140581d20` | `0x141515430` | `0x140583100` | `0x1405896a0` |
| `0x00A4` | `164` | `0x28` | `0x300400` | `0x140224eb0` | `0x140353150` | `0x1414f7690` | `0x140353570` | `0x140355120` |
| `0x00A7` | `167` | `None` | `0x300400` | `0x14080e5c3` | `None` | `0x141549820` | `0x14080c9b0` | `0x140355290` |
| `0x00A8` | `168` | `None` | `0x300400` | `0x14080e3cb` | `None` | `0x141549660` | `0x14080c2d0` | `0x1403550c0` |
| `0x00AA` | `170` | `None` | `0x300400` | `0x14080d594` | `None` | `0x141548e40` | `0x14080b2a0` | `0x140354670` |
| `0x00AD` | `173` | `0x30` | `0x300400` | `0x14022a010` | `0x140581db0` | `0x1415155f0` | `0x140583170` | `0x1405896f0` |
| `0x00AE` | `174` | `0xd0` | `0x300400` | `0x140226350` | `0x140484930` | `0x1415060e8` | `0x14048e2f0` | `0x1404cce70` |
| `0x00B0` | `176` | `None` | `0x300400` | `0x14080eaad` | `None` | `0x141549ba0` | `0x14080ba70` | `0x140354a70` |
| `0x00B3` | `179` | `None` | `0x300400` | `0x14080ddc9` | `None` | `0x141549100` | `0x14080ab80` | `0x140354210` |
| `0x00B4` | `180` | `0x38` | `0x300400` | `0x1402a8ad0` | `0x140bc7c30` | `0x14159b440` | `0x140068cc0` | `0x140bcf0e0` |
| `0x00B5` | `181` | `0x58` | `0x300400` | `0x1402a8a70` | `0x140bc7cb0` | `0x14159b400` | `0x140068cc0` | `0x140bcf000` |
| `0x00B7` | `183` | `0x38` | `0x300400` | `0x14022a070` | `0x140581e40` | `0x141515630` | `0x1405832e0` | `0x14056c760` |
| `0x00BD` | `189` | `None` | `0x300400` | `0x14080e60b` | `None` | `0x141549860` | `0x14080ca70` | `0x140355310` |
| `0x00BE` | `190` | `0xe8` | `None` | `0x1402262f0` | `0x1404849c0` | `0x141507060` | `0x1404f3750` | `0x1404cb280` |
| `0x00BF` | `191` | `0x90` | `0x300400` | `0x140228960` | `0x140538980` | `0x14150fce8` | `0x14053bc00` | `0x140546a10` |
| `0x00C0` | `192` | `0x58` | `0x300400` | `0x140227f70` | `0x14051aff0` | `0x14150d618` | `0x14051bc60` | `0x140522900` |
| `0x00C1` | `193` | `0x60` | `0x300400` | `0x14022b870` | `0x1405c1630` | `0x141519978` | `0x1405c25b0` | `0x1405c79b0` |
| `0x00C7` | `199` | `None` | `0x300400` | `0x14080e143` | `None` | `0x141549340` | `0x14080b070` | `0x1411a3840` |
| `0x00C9` | `201` | `0x48` | `0x300400` | `0x140227f40` | `0x14051b080` | `0x14150d698` | `0x14051bb60` | `0x1405228b0` |
| `0x00CA` | `202` | `0x48` | `0x300400` | `0x1402280f0` | `0x14051b120` | `0x14150d7c8` | `0x14051caf0` | `0x1405228b0` |
| `0x00CE` | `206` | `0xb0` | `0x300400` | `0x140249760` | `0x140750010` | `0x141510908` | `0x1407501b0` | `0x1405481e0` |
| `0x00CF` | `207` | `None` | `0x300400` | `0x14080dd81` | `None` | `0x1415490c0` | `0x14080aad0` | `0x140354210` |
| `0x00D3` | `211` | `0x38` | `0x300400` | `0x1402293c0` | `0x140563c80` | `0x1415126f0` | `0x140564be0` | `0x14056c760` |
| `0x00D4` | `212` | `0x38` | `0x300400` | `0x140250110` | `0x1407aafd0` | `0x141545a88` | `0x1407b13d0` | `0x1407f68c0` |
| `0x00DC` | `220` | `0xa0` | `None` | `0x1402a3910` | `0x140a4db20` | `0x1414f0640` | `0x140311fa0` | `0x1400015c0` |
| `0x00DD` | `221` | `0x40` | `0x300400` | `0x1402a8b00` | `0x140bc7d50` | `0x14159b480` | `0x140068cc0` | `0x140bcf140` |
| `0x00E2` | `226` | `0x30` | `0x300400` | `0x140226cc0` | `0x1404f3570` | `0x14150a0f8` | `0x1404f3750` | `0x1404f3d60` |
| `0x00E3` | `227` | `0x38` | `0x300400` | `0x140229450` | `0x140563d10` | `0x1415127f0` | `0x140564ea0` | `0x14056c710` |
| `0x00E4` | `228` | `0x30` | `0x300400` | `0x140224e50` | `0x1403531a0` | `0x1414f76d0` | `0x140353230` | `0x1403547a0` |
| `0x00E5` | `229` | `0x38` | `0x300400` | `0x1402501a0` | `0x1407ab070` | `0x141545a48` | `0x1407b1700` | `0x1407f6c30` |
| `0x00E6` | `230` | `0x80` | `0x300400` | `0x14022aca0` | `0x14059af40` | `0x1415167d8` | `0x14059dcc0` | `0x1405a5d90` |
| `0x00E7` | `231` | `None` | `0x300400` | `0x14080eb3d` | `None` | `0x141549c20` | `0x14080c210` | `0x140354480` |
| `0x00ED` | `237` | `None` | `0x300400` | `0x14080e533` | `None` | `0x1415497a0` | `0x14080c830` | `0x140355240` |
| `0x00EF` | `239` | `0x38` | `0x300400` | `0x1402a8b60` | `0x140bc7de0` | `0x14159b588` | `0x140068cc0` | `0x140bcf290` |
| `0x00F0` | `240` | `0x38` | `0x300400` | `0x140250170` | `0x1407ab110` | `0x1415459c8` | `0x1407b1610` | `0x1407f6970` |
| `0x00F1` | `241` | `0x38` | `0x300400` | `0x14022a0a0` | `0x140581ed0` | `0x1415155b0` | `0x140583330` | `0x14056c760` |
| `0x00F5` | `245` | `0x28` | `0x300400` | `0x1402a8aa0` | `0x140bc7e70` | `0x1414f32b0` | `0x140068cc0` | `0x1411a3840` |
| `0x00FA` | `250` | `None` | `0x300400` | `0x14080d66f` | `None` | `0x141548f40` | `0x14080c480` | `0x140355140` |
| `0x00FB` | `251` | `None` | `0x300400` | `0x14080d773` | `None` | `0x141548fc0` | `0x14080ac40` | `0x140354260` |
| `0x00FC` | `252` | `None` | `0x300400` | `0x14080d729` | `None` | `0x141548fc0` | `0x14080ac40` | `0x140354260` |
| `0x00FE` | `254` | `None` | `0x300400` | `0x14080e8fd` | `None` | `0x141549a20` | `0x14080b650` | `0x1403549e0` |
| `0x0138` | `312` | `0x30` | `0x300400` | `0x1402a8b90` | `0x140bc7aa0` | `0x141527e00` | `0x140068cc0` | `0x140354480` |
| `0x013E` | `318` | `0x40` | `0x300400` | `0x140225380` | `0x140376b00` | `0x1414f8fa8` | `0x1403770d0` | `0x140377ca0` |
| `0x0146` | `326` | `None` | `0x300400` | `0x14080e8b5` | `None` | `0x1415499e0` | `0x14080b5a0` | `0x140354980` |
| `0x0147` | `327` | `None` | `0x300400` | `0x14080ea65` | `None` | `0x141549b60` | `0x14080b9c0` | `0x140354aa0` |
| `0x0148` | `328` | `None` | `0x300400` | `0x14080e98d` | `None` | `0x141549aa0` | `0x14080b7b0` | `0x140354a10` |
| `0x0149` | `329` | `None` | `0x300400` | `0x14080ea1d` | `None` | `0x141549b20` | `0x14080b910` | `0x140354a70` |
| `0x014A` | `330` | `None` | `0x300400` | `0x14080e945` | `None` | `0x141549a60` | `0x14080b700` | `0x140354480` |
| `0x0152` | `338` | `None` | `0x300400` | `0x14080e4a3` | `None` | `0x141549720` | `0x14080c6b0` | `0x140355170` |
| `0x0153` | `339` | `None` | `0x300400` | `0x14080e18b` | `None` | `0x141549380` | `0x14080b130` | `0x1411a3840` |
| `0x0158` | `344` | `None` | `0x300400` | `0x14080d626` | `None` | `0x141548ec0` | `0x14080bbe0` | `0x140354ae0` |
| `0x015B` | `347` | `None` | `0x300400` | `0x14080d5dd` | `None` | `0x141548e80` | `0x14080bb20` | `0x1411a3840` |
| `0x015C` | `348` | `0x88` | `None` | `0x1402a2310` | `0x140a16a00` | `0x141528218` | `0x140068cc0` | `0x140a1ded0` |
| `0x015D` | `349` | `0x50` | `0x300400` | `0x1402a22e0` | `0x140a16ac0` | `0x14158f548` | `0x14080acf0` | `0x140a1cd10` |
| `0x015E` | `350` | `0xa0` | `None` | `0x1402a1e00` | `0x140a16bb0` | `None` | `None` | `None` |
| `0x015F` | `351` | `0x48` | `None` | `0x1402a1f50` | `0x140a16c00` | `0x14158f298` | `0x140a18b50` | `0x140a1cad0` |
| `0x0160` | `352` | `0x50` | `None` | `0x1402a1da0` | `0x140a16c60` | `0x141528108` | `0x1406dbe50` | `0x140a1d480` |
| `0x0161` | `353` | `0x78` | `None` | `0x1402a23a0` | `0x140a16cc0` | `0x14158f650` | `0x140068cc0` | `0x140a1e340` |
| `0x0162` | `354` | `0x50` | `0x300400` | `0x1402a22b0` | `0x140a16d80` | `0x14158f4f8` | `0x140a18fd0` | `0x140a1cd10` |
| `0x0163` | `355` | `0x48` | `None` | `0x1402a2100` | `0x140a16e70` | `0x14158f5e8` | `0x140a18d20` | `0x140a1cad0` |
| `0x0164` | `356` | `0x48` | `None` | `0x1402a20a0` | `0x140a16ed0` | `0x14158eff8` | `0x140a18d20` | `0x1406e2690` |
| `0x0165` | `357` | `0x30` | `0x300400` | `0x1402a19b0` | `0x140a16f30` | `0x141527e00` | `0x140068cc0` | `0x140354480` |
| `0x0166` | `358` | `0x48` | `None` | `0x1402a1d10` | `0x140a16fc0` | `0x1415283c0` | `0x1406dbe50` | `0x1406e2690` |
| `0x0167` | `359` | `0x78` | `None` | `0x1402a2400` | `0x140a17020` | `0x14158f700` | `0x140068cc0` | `0x140a1e460` |
| `0x0168` | `360` | `0x70` | `None` | `0x1402a2340` | `0x140a170d0` | `0x14158f060` | `0x140068cc0` | `0x140a1cc60` |
| `0x0169` | `361` | `0x58` | `None` | `0x1402a1e30` | `0x140a17180` | `0x14158eec0` | `0x140a18d20` | `0x140a1d7f0` |
| `0x016A` | `362` | `0x70` | `None` | `0x1402a1ad0` | `0x140a171f0` | `0x1415285d0` | `0x140068cc0` | `0x140a1cc60` |
| `0x016B` | `363` | `0x78` | `None` | `0x1402a1d70` | `0x140a172a0` | `None` | `None` | `None` |
| `0x016C` | `364` | `0x50` | `None` | `0x1402a20d0` | `0x140a172f0` | `0x141528058` | `0x1406dbe50` | `0x140a1d480` |
| `0x016D` | `365` | `0x70` | `None` | `0x1402a1b60` | `0x140a17350` | `0x141528010` | `0x140068cc0` | `0x140a1cc60` |
| `0x016E` | `366` | `0xa0` | `None` | `0x1402a1980` | `0x140a17400` | `0x1415284d8` | `0x140068cc0` | `0x140a1cae0` |
| `0x016F` | `367` | `0x70` | `None` | `0x1402a1a10` | `0x140a17530` | `0x141528428` | `0x140068cc0` | `0x140a1cc60` |
| `0x0170` | `368` | `0x50` | `None` | `0x1402a1b90` | `0x140a175e0` | `0x1415286c8` | `0x140068cc0` | `0x140a1cd10` |
| `0x0171` | `369` | `0x50` | `None` | `0x1402a1b30` | `0x140a17640` | `0x141527ed0` | `0x140068cc0` | `0x1406e2680` |
| `0x0172` | `370` | `0x40` | `0x300400` | `0x1402a21c0` | `0x140a176a0` | `0x14158f910` | `0x140a191a0` | `0x140a1e0c0` |
| `0x0173` | `371` | `0x38` | `0x300400` | `0x1402a1f20` | `0x140a17740` | `0x14158f1a8` | `0x140a18fd0` | `0x140a1d220` |
| `0x0174` | `372` | `0x48` | `None` | `0x1402a1ef0` | `0x140a177e0` | `0x141528260` | `0x1406dbe50` | `0x1406e2690` |
| `0x0175` | `373` | `0x78` | `None` | `0x1402a1ec0` | `0x140a17840` | `None` | `None` | `None` |
| `0x0176` | `374` | `0xa0` | `None` | `0x1402a1c50` | `0x140a17890` | `0x14158f110` | `0x140068cc0` | `0x140a1cf90` |
| `0x0177` | `375` | `0x80` | `None` | `0x1402a19e0` | `0x140a17990` | `0x141528378` | `0x140068cc0` | `0x140a1cbb0` |
| `0x0178` | `376` | `0x70` | `None` | `0x1402a2460` | `0x140a17a50` | `0x14158f7b0` | `0x140068cc0` | `0x140a1cc60` |
| `0x0179` | `377` | `0x88` | `0x300400` | `0x1402a1fb0` | `0x140a17b00` | `0x14158f248` | `0x140a18fd0` | `0x140a1ded0` |
| `0x017A` | `378` | `0x58` | `None` | `0x1402a1dd0` | `0x140a17c60` | `0x14158f3f0` | `0x140a18d20` | `0x140a1d500` |
| `0x017B` | `379` | `0x48` | `None` | `0x1402a1f80` | `0x140a17cd0` | `0x14158f0a8` | `0x140a18d20` | `0x1406e2690` |
| `0x017C` | `380` | `0x70` | `None` | `0x1402a1ce0` | `0x140a17d30` | `None` | `None` | `None` |
| `0x017D` | `381` | `0xc0` | `None` | `0x1402a1c20` | `0x140a17d80` | `None` | `None` | `None` |
| `0x017E` | `382` | `0x50` | `None` | `0x1402a1bf0` | `0x140a17dd0` | `0x1415282c8` | `0x140068cc0` | `0x1406e2680` |
| `0x017F` | `383` | `0x60` | `None` | `0x1402a2370` | `0x140a17e30` | `0x1415280c0` | `0x140068cc0` | `0x140a1e250` |
| `0x0180` | `384` | `0x50` | `None` | `0x1402a2130` | `0x140a17e90` | `0x141528618` | `0x1406dbed0` | `0x140a1d480` |
| `0x0181` | `385` | `0x40` | `0x300400` | `0x1402a2010` | `0x140a17ef0` | `0x14158f458` | `0x140a18fd0` | `0x140a1df60` |
| `0x0182` | `386` | `0x50` | `None` | `0x1402a1a70` | `0x140a17f90` | `0x14158f8c8` | `0x140068cc0` | `0x140a1cd10` |
| `0x0183` | `387` | `0x78` | `None` | `0x1402a23d0` | `0x140a17ff0` | `0x141528680` | `0x140068cc0` | `0x140a1e3b0` |
| `0x0184` | `388` | `0x48` | `None` | `0x1402a1fe0` | `0x140a180a0` | `0x141528520` | `0x1406dbe50` | `0x1406e2690` |
| `0x0185` | `389` | `0x48` | `None` | `0x1402a2220` | `0x140a18100` | `0x14158f748` | `0x140a18da0` | `0x140a1cad0` |
| `0x0186` | `390` | `0xa8` | `0x300400` | `0x1402a2160` | `0x140a18160` | `0x141527e00` | `0x140068cc0` | `0x140354480` |
| `0x0187` | `391` | `0x58` | `0x300400` | `0x1402a1d40` | `0x140a181b0` | `0x14158f1f8` | `0x140a18fd0` | `0x140a1d2b0` |
| `0x0188` | `392` | `0x78` | `None` | `0x1402a2430` | `0x140a182a0` | `0x141527f80` | `0x140068cc0` | `0x140a1e3b0` |
| `0x0189` | `393` | `0x78` | `None` | `0x1402a1bc0` | `0x140a18350` | `0x141527fc8` | `0x140068cc0` | `0x140a1ce40` |
| `0x018A` | `394` | `0x90` | `None` | `0x1402a2280` | `0x140a18400` | `0x141528588` | `0x140068cc0` | `0x140a1e1e0` |
| `0x018B` | `395` | `0x50` | `0x300400` | `0x1402a2250` | `0x140a18510` | `0x14158f958` | `0x140a191a0` | `0x140a1cd10` |
| `0x018C` | `396` | `0x50` | `None` | `0x1402a21f0` | `0x140a18600` | `0x141527f18` | `0x1406dbe50` | `0x140a1d480` |
| `0x018D` | `397` | `0x68` | `None` | `0x1402a1e90` | `0x140a18660` | `None` | `None` | `None` |
| `0x018E` | `398` | `0x70` | `None` | `0x1402a1aa0` | `0x140a186b0` | `0x141528170` | `0x140068cc0` | `0x140a1cc60` |
| `0x018F` | `399` | `0x48` | `None` | `0x1402a2190` | `0x140a18760` | `0x14158f698` | `0x140a18da0` | `0x140a1cad0` |
| `0x0190` | `400` | `0x58` | `None` | `0x1402a1e60` | `0x140a187c0` | `0x14158ef90` | `0x140a18d20` | `0x140a1d9a0` |
| `0x0191` | `401` | `0x38` | `0x300400` | `0x1402a1cb0` | `0x140a18830` | `0x14158f158` | `0x140a18fd0` | `0x140a1d220` |
| `0x0192` | `402` | `0x48` | `0x300400` | `0x1402a2040` | `0x140a188d0` | `0x14158f4a8` | `0x140a18fd0` | `0x140a1dfc0` |
| `0x0193` | `403` | `0x68` | `None` | `0x1402a1b00` | `0x140a18970` | `0x14158f3a8` | `0x140068cc0` | `0x140a1cd60` |
| `0x0194` | `404` | `0x90` | `None` | `0x1402a2070` | `0x140a189e0` | `None` | `None` | `None` |
| `0x0195` | `405` | `0x50` | `None` | `0x1402a1c80` | `0x140a18a30` | `0x141528470` | `0x1406dbe50` | `0x140a1d1f0` |
| `0x0196` | `406` | `0x28` | `0x300400` | `0x140224e80` | `0x140353100` | `0x1414f7710` | `0x1403534a0` | `0x1411a3840` |
| `0x0197` | `407` | `None` | `0x300400` | `0x1405e6443` | `None` | `0x14151ec10` | `0x1405e5890` | `0x140354400` |
| `0x019A` | `410` | `None` | `0x300400` | `0x14031e6ff` | `None` | `0x1414f33e8` | `0x1411fad2a` | `0x1411a3840` |
| `0x01A2` | `418` | `0x50` | `0x300400` | `0x1402a1a40` | `0x140a18a90` | `0x1414f99a8` | `0x14037a3d0` | `0x403030203020201` |
| `0x30000024` | `805306404` | `None` | `0x300400` | `0x140ae9161` | `None` | `0x141597b20` | `0x140ae8780` | `0x1403544d0` |
| `0x30000025` | `805306405` | `None` | `0x300400` | `0x140ae9119` | `None` | `0x141597ae0` | `0x140ae86d0` | `0x140354480` |
| `0x30000026` | `805306406` | `None` | `0x300400` | `0x140ae91a9` | `None` | `0x141597b60` | `0x140ae8990` | `0x1411a3840` |
| `0x3000002A` | `805306410` | `0x50` | `0x300400` | `0x1402a46e0` | `0x140ad0680` | `0x1415954b0` | `0x140ad0b10` | `0x140ad4ab0` |
| `0x30000033` | `805306419` | `None` | `0x300400` | `0x140ae9089` | `None` | `0x1415979b0` | `0x140ae8af0` | `0x1411a3840` |
| `0x30000034` | `805306420` | `None` | `0x300400` | `0x140ae90d1` | `None` | `0x1415979f0` | `0x140ae8bb0` | `0x1411a3840` |
| `0x3000003D` | `805306429` | `None` | `0x300400` | `0x140ae9037` | `None` | `0x141597c70` | `0x140ae8a40` | `0x140354f20` |
| `0x30000040` | `805306432` | `None` | `0x300400` | `0x140ae9239` | `None` | `0x141597c30` | `0x140ae88e0` | `0x1411a3840` |
| `0x30000045` | `805306437` | `None` | `0x300400` | `0x140ae91f1` | `None` | `0x141597bf0` | `0x140ae8830` | `0x140355140` |

### 3.3 Outbound-Only / Constructor-Initialized `NetBlobEvent` IDs (`0x141165b90`) <a id="outbound-only-netblob-events"></a>

In addition to the 198 factory registrations above (which cover `197` distinct event IDs), the client binary contains **28** additional `NetBlobEvent` subclasses that call `NetBlobEvent::NetBlobEvent` (`0x141165b90`) directly for outbound construction without registering a receive factory:

| Event ID (Hex) | Event ID (Dec) | Initial Flags (`+0x14`) | Vtable VA | Constructor Site VA | `Dispatch` (`slot[1]`) | `PackUnpack` (`slot[7]`) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x0024` | `36` | `0x300400` | `0x1415493c0` | `0x14080ed29` | `0x14080acf0` | `0x140354360` |
| `0x0029` | `41` | `0x300400` | `0x141516858` | `0x14059ab30` | `0x14059d320` | `0x1405a5570` |
| `0x002C` | `44` | `0x300400` | `0x14159b588` | `0x140bc7e30` | `0x140068cc0` | `0x140bcf290` |
| `0x0044` | `68` | `0x300400` | `0x1415168d8` | `0x1405a6f95` | `0x14059e250` | `0x1405a60c0` |
| `0x006F` | `111` | `0x300400` | `0x141549280` | `0x14080f4f5` | `0x14080acf0` | `0x140354740` |
| `0x0075` | `117` | `0x300400` | `0x141549410` | `0x14080ef8b` | `0x14080acf0` | `0x140354390` |
| `0x007A` | `122` | `0x300400` | `0x141549300` | `0x14080fc73` | `0x14080acf0` | `0x140355060` |
| `0x0095` | `149` | `0x300400` | `0x141549450` | `0x14080f173` | `0x14080acf0` | `0x140354400` |
| `0x00A3` | `163` | `0x300400` | `0x141548f00` | `0x14080f8d4` | `0x14080acf0` | `0x140354f50` |
| `0x00CC` | `204` | `0x300400` | `0x141549590` | `0x14080a51b` | `0x14080bde0` | `0x140354ea0` |
| `0x00D0` | `208` | `0x300400` | `0x141549300` | `0x14080fe33` | `0x14080acf0` | `0x140355060` |
| `0x00D1` | `209` | `0x300400` | `0x141549300` | `0x14080fab3` | `0x14080acf0` | `0x140355060` |
| `0x00EC` | `236` | `0x300400` | `0x14159b4c8` | `0x140bc6703` | `0x140068cc0` | `0x140bcf1c0` |
| `0x013D` | `317` | `0x300400` | `0x14159b608` | `0x140bce942` | `0x140068cc0` | `0x140bcf3e0` |
| `0x0154` | `340` | `0x300400` | `0x141549080` | `0x14080f6d6` | `0x14080acf0` | `0x140354950` |
| `0x0155` | `341` | `0x300400` | `0x141549ce0` | `0x14080fff3` | `0x14080acf0` | `0x1403551a0` |
| `0x0198` | `408` | `0x300400` | `0x1414f3360` | `0x14031c990` | `0x14031cd60` | `0x14031cf50` |
| `0x0199` | `409` | `0x300400` | `0x1414f33a8` | `0x14031c920` | `0x14031ccb0` | `0x14031ceb0` |
| `0x019E` | `414` | `0x300400` | `0x141549300` | `0x1408101b3` | `0x14080acf0` | `0x140355060` |
| `0x3000001B` | `805306395` | `0x300400` | `0x141593178` | `0x140a59638` | `0x14080acf0` | `0x140354a70` |
| `0x3000001C` | `805306396` | `0x300400` | `0x141549040` | `0x140aefce7` | `0x14080acf0` | `0x1411a3840` |
| `0x3000001D` | `805306397` | `0x300400` | `0x1415931b8` | `0x140a59517` | `0x14080acf0` | `0x140354480` |
| `0x3000001E` | `805306398` | `0x300400` | `0x1415931f8` | `0x140a5948f` | `0x14080acf0` | `0x140a59f60` |
| `0x3000001F` | `805306399` | `0x300400` | `0x141593238` | `0x140a593f8` | `0x14080acf0` | `0x140a59ed0` |
| `0x30000035` | `805306421` | `0x300400` | `0x141593278` | `0x140a59318` | `0x14080acf0` | `0x140a59cd0` |
| `0x30000036` | `805306422` | `0x300400` | `0x1415978e0` | `0x140aef8af` | `0x14080acf0` | `0x140a59a60` |
| `0x30000037` | `805306423` | `0x300400` | `0x1415978a0` | `0x140aef5df` | `0x14080acf0` | `0x140a597b0` |
| `0x30000038` | `805306424` | `0x300400` | `0x1415932b8` | `0x140a59596` | `0x14080acf0` | `0x140a59fc0` |

---

## 4. Master Subsystem Summary of All 120 WLib RPC Classes <a id="master-rpc-class-summary"></a>

| RPC Class ID | Dec | Parent Class | Unpack `iface_id` | Paired Client Class(es) | Pack Count (C->S) | Unpack Count (S->C) | Total RPC Methods | C++ Call Sites | Identified Gameplay Subsystem |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x01F5` | `501` | None | None | `0x003C` | `2` | `0` | `2` | `2` | EngineOptionSystem (Gameplay Options Sanity Check & Property Sync) |
| `0x04D9` | `1241` | None | `0x06C3` | `0x00FE`, `0x06CA`, `0x0E75` | `6` | `8` | `14` | `16` | AIPetSystem / CosmeticPetSystem (Combat Pets & Cosmetic Pets) |
| `0x04DA` | `1242` | None | `0x06C2`, `0x06C3` | `0x00FC` | `1` | `3` | `4` | `7` | AccomplishmentSystem (Deeds & Accomplishment Directory) |
| `0x04DC` | `1244` | None | `0x06C2` | `0x06C2` | `45` | `1` | `46` | `0` | CAdmin Core Slash Commands (Appearance, Effects, Dispel, Admin Utilities) |
| `0x04DD` | `1245` | None | `0x06C2` | `0x00F6` | `8` | `2` | `10` | `1` | AdminPropertySystem (Live Property Edit, Remove & Singleton Dump) |
| `0x04DE` | `1246` | None | `0x06C2` | `0x00FA`, `0x00FE`, `0x0100`, `0x0102`, `0x0106`, `0x010B` (+45 more) | `140` | `5` | `145` | `670` | CAdmin / Gameplay Systems Master Admin RPC Dispatcher (140 Admin Commands) |
| `0x04DF` | `1247` | None | `0x06C3` | `0x0185` | `3` | `4` | `7` | `25` | AdvancementSystem (Character Level, XP, Class Skill Training) |
| `0x04E1` | `1249` | None | `0x06C3` | `0x06CB` | `1` | `2` | `3` | `1` | AlertSystem (Client/Server Alert Registry) |
| `0x04E3` | `1251` | None | None | `0x00FF` | `1` | `0` | `1` | `2` | AreaSystem Client->Server (Area Query & WorldJoin Area Check) |
| `0x04E4` | `1252` | None | `0x06C3` | `0x00FF` | `0` | `1` | `1` | `2` | AreaSystem Server->Client (Area Discovery & Region Sync) |
| `0x04E5` | `1253` | None | `0x06C3` | `0x00FA` | `10` | `9` | `19` | `22` | AuctionSystem (Auction House Post, Bid, Buyout, Cancel, Search) |
| `0x04E8` | `1256` | None | None | `0x06E0` | `1` | `0` | `1` | `0` | AuctionSystem / Dialog Confirmation Callback (0x06E0) |
| `0x04E9` | `1257` | None | `0x06C1`, `0x06C3` | `0x0100` | `4` | `5` | `9` | `20` | CCombatSystem (Auto-Attack, Target Selection, Combat Mode & Combat Events) |
| `0x04EB` | `1259` | None | `0x06C1` | `0x08DA` | `0` | `1` | `1` | `0` | CombatStateBreakNotice / Dismount Notification System (0x08DA) |
| `0x04EC` | `1260` | None | `0x06C3`, `0x06C5` | `0x0102` | `11` | `11` | `22` | `41` | CommunicationSystem / Chat & Gag System (Broadcast, Gag, Chat Channels) |
| `0x04ED` | `1261` | None | `0x06C1`, `0x06C3` | `0x0103` | `1` | `4` | `5` | `7` | ConjunctionSystem (Fellowship Manoeuvres & Conjunction Contributions) |
| `0x04EE` | `1262` | None | `0x06C3` | `0x0106` | `1` | `4` | `5` | `2` | CooldownSystem (Skill & Item Cooldown Timers & Reset) |
| `0x04EF` | `1263` | None | `0x06C3` | `0x0107` | `7` | `14` | `21` | `26` | CCraftSystem (Professions, Vocations, Recipes, Craft Guilds, Item Level) |
| `0x04F1` | `1265` | None | `0x06C3` | `0x0109` | `0` | `1` | `1` | `0` | GameTime / Scene Waveform / Client Notification System (0x0109 / 0x013B0001) |
| `0x04F2` | `1266` | None | `0x06C1`, `0x06C3` | `0x0129` | `5` | `8` | `13` | `3` | DeathSystem (LifeState, Revive, Retreat, Defeat/Smite, NeverSayDie) |
| `0x04F3` | `1267` | None | `0x06C3` | `0x010A` | `2` | `1` | `3` | `3` | DiscoverySystem (Resource Tracking & Monster Tracking) |
| `0x04F6` | `1270` | None | None | `0x06CE` | `1` | `0` | `1` | `0` | DramaSystem (AI Drama Sequence Performance & Dump) |
| `0x04F7` | `1271` | None | `0x06C3` | `0x06D8`, `0x06E3` | `3` | `2` | `5` | `0` | DuelingSystem (Sparring Invite, Accept, Decline & Result) |
| `0x04F8` | `1272` | None | `0x06C3` | `0x010B`, `0x06D0` | `0` | `8` | `8` | `6` | CEffectSystem / CEffectListenerSystem (Effect Add/Remove & Listener Sync) |
| `0x04F9` | `1273` | None | `0x06C5` | `0x012A` | `4` | `4` | `8` | `5` | EmoteSystem (Granted Emotes, Emote Commands & Execution) |
| `0x04FC` | `1276` | None | `0x06C3` | `0x010C` | `0` | `2` | `2` | `3` | CErrorSystem (Server->Client Error Code & String Table Notification) |
| `0x04FD` | `1277` | None | `0x06C3` | `0x010D` | `4` | `5` | `9` | `4` | CExaminationSystem (Tooltip Inspection Cache & Characteristic Structs) |
| `0x04FE` | `1278` | None | `0x06C3` | `0x0102`, `0x019D`, `0x0452`, `0x074D` | `20` | `39` | `59` | `72` | Social / Friends / Ignore / Who / Fellowship Lookup System (0x019D / 0x0452) |
| `0x0502` | `1282` | None | `0x06C3` | `0x016C` | `20` | `25` | `45` | `24` | GuildSystem (Kinship Management, Roster, MotD, Ranks, Lifespan) |
| `0x0506` | `1286` | None | None | `0x017A` | `1` | `0` | `1` | `25` | SkillSystem Auxiliary Request (paired with 0x017A / 0x0528) |
| `0x0507` | `1287` | None | None | `0x016F` | `1` | `0` | `1` | `1` | Inscription / PlayerBio System (/playerbio & /iteminscription) |
| `0x0508` | `1288` | None | None | `0x06C2` | `2` | `0` | `2` | `0` | InteractionPermission & PvPOverride Admin System (/showattackperm, /pvpoverride) |
| `0x050A` | `1290` | None | `0x06C3` | `0x0170`, `0x0AF4` | `8` | `2` | `10` | `26` | Inventory Item Creation & Container Operations (/create, Stack/Move/Equip) |
| `0x050B` | `1291` | None | `0x06C3` | `0x0171` | `2` | `4` | `6` | `5` | ItemWearSystem (Equipment Wear State & Item Repair) |
| `0x050C` | `1292` | None | `0x06C3` | `0x1021` | `10` | `28` | `38` | `13` | LFGSystem (Looking For Group / Social Panel Queue & Search) |
| `0x050E` | `1294` | None | `0x06C3` | `0x0110` | `0` | `2` | `2` | `1` | LandmarkSystem (Point-of-Interest & Landmark Discovery) |
| `0x050F` | `1295` | None | `0x06C3` | `0x0745` | `0` | `3` | `3` | `0` | Selection / Target-of-Target Display Sync System (0x0745 / 0x0746) |
| `0x0510` | `1296` | None | `0x06C3` | `0x018D`, `0x06EA`, `0x0701` | `4` | `2` | `6` | `4` | Logout / Session Termination Dialog System (Player_HasLoggingOutDialog) |
| `0x0511` | `1297` | None | None | `0x06C2` | `1` | `0` | `1` | `0` | LootGenSystem Admin Control (/lootgen) |
| `0x0512` | `1298` | None | `0x06C3` | `0x0112`, `0x0751` | `8` | `7` | `15` | `11` | LootSystem (Corpse/Chest Loot List, Roll/Pass, Master Looter, Pending Loot) |
| `0x0513` | `1299` | None | `0x06C3` | `0x092E` | `2` | `2` | `4` | `9` | CMEOInventorySystem (Container Start/Stop Viewing & Inventory Commands) |
| `0x0514` | `1300` | None | `0x06C3` | `0x0173`, `0x0627`, `0x0987` | `8` | `9` | `17` | `11` | MailSystem (Send, Read, Delete, Return, Take Attachment/Money, Admin Mail) |
| `0x0515` | `1301` | None | `0x06C3` | `0x01B6` | `2` | `1` | `3` | `2` | MapNote / Dungeon Map Data System (Dungeon_CurrentDungeon, Dungeon_MapData) |
| `0x0518` | `1304` | None | None | `0x0175` | `2` | `0` | `2` | `0` | Movement / Action Lock System (Movement Action Restriction) |
| `0x0519` | `1305` | None | `0x06C3` | `0x0116` | `8` | `8` | `16` | `6` | MusicSystem (Player Music Mode, ABC/Note Performance & Quantization) |
| `0x051B` | `1307` | None | `0x06C3` | `0x0119`, `0x055E`, `0x06E2` | `6` | `7` | `13` | `3` | PedigreeSystem (Character Surname, Pedigree & Generation) |
| `0x051C` | `1308` | None | `0x06C3` | `0x011A` | `1` | `3` | `4` | `3` | PerkSystem (Destiny Point Perks Purchase & Query) |
| `0x051D` | `1309` | None | `0x06C3` | `0x06C3` | `0` | `1` | `1` | `0` | CPlayer Virtual Slot 0x0361 State Sync (4-Field Entity/Value Update) |
| `0x051E` | `1310` | None | `0x06C3` | `0x0176`, `0x0179` | `11` | `8` | `19` | `11` | ShortcutSystem (Quickslot Bar Assignment & Shortcut Execution) |
| `0x051F` | `1311` | None | `0x06C3` | `0x011C`, `0x01C1`, `0x06E9` | `5` | `18` | `23` | `9` | PrivateEncounterSystem (Instance/Skirmish/Session Encounter Control) |
| `0x0520` | `1312` | None | `0x01FE` | `0x011D` | `0` | `3` | `3` | `1` | Induction / Channeling System (Skill Induction & Channeling Interrupt Sync) |
| `0x0521` | `1313` | None | `0x06C2`, `0x06C3` | `0x011F`, `0x06D9`, `0x0C0E` | `14` | `36` | `50` | `77` | CQuestSystem / CQuestJournal (Quest Bestowal, Advance, Complete, Cancel, Tracker) |
| `0x0522` | `1314` | None | `0x06C3` | `0x011E` | `2` | `2` | `4` | `1` | QuestRoleData / NPC Quest Role Inspection System (DoCmdGetQuestRoleData) |
| `0x0523` | `1315` | None | `0x06C3` | `0x0123` | `8` | `6` | `14` | `12` | FellowshipSystem (Create, Invite, Dismiss, Leave, Leader, Loot Rules) |
| `0x0524` | `1316` | None | `0x06C3` | `0x0628`, `0x06DB`, `0x0C47` | `4` | `4` | `8` | `0` | RecallSystem (Milestone bind, Return/Travel Link & Recall Confirmation) |
| `0x0527` | `1319` | None | `0x06C3` | `0x037F` | `0` | `1` | `1` | `0` | Script Dialog / WSL Callback Return System (GCParam_WSLReturnValue, 0x037F) |
| `0x0528` | `1320` | None | `0x06C3` | `0x0127`, `0x017A` | `5` | `4` | `9` | `45` | SkillSystem (Skill Execution, Toggle, Target Validation & Pip/Vital Checks) |
| `0x052A` | `1322` | None | `0x06C3` | `0x06DD` | `1` | `1` | `2` | `0` | SummonSystem (Horn/Muster Player Summoning & Confirmation Dialog) |
| `0x052B` | `1323` | None | `0x06C3` | `0x0124` | `0` | `2` | `2` | `5` | TappingSystem (Mob Tap Ownership & Open Tapping) |
| `0x052C` | `1324` | None | `0x06C3` | `0x017C`, `0x0606` | `3` | `5` | `8` | `4` | CTextItemSystem (Readable Parchments/Books & Custom Text Items) |
| `0x052D` | `1325` | None | `0x06C3` | `0x017D` | `4` | `2` | `6` | `4` | Scene / Cutscene Playback System (/scene play, Ready/Not Ready Sync) |
| `0x052E` | `1326` | None | `0x06C3` | `0x017E` | `8` | `3` | `11` | `6` | TitleSystem (Earned Titles, Active Title Selection & Title Display) |
| `0x052F` | `1327` | None | `0x06C3` | `0x06DC` | `0` | `4` | `4` | `0` | EquivalenceEffect / Cooldown Timer Notification System (0x06DC / 0x015E) |
| `0x0530` | `1328` | None | `0x06C3` | `0x06DF` | `0` | `5` | `5` | `1` | CTrackingSystem (Entity Radar/Minimap Tracking Update) |
| `0x0531` | `1329` | None | `0x06C3` | `0x017F` | `9` | `12` | `21` | `8` | TradeSystem (Player-to-Player Secure Trade Session, Offer, Lock, Accept) |
| `0x0532` | `1330` | None | `0x06C3` | `0x0125` | `11` | `18` | `29` | `53` | CTraitSystem (Earned Traits, Slotted Traits, Specialization Trees) |
| `0x0533` | `1331` | None | `0x06C3` | `0x01CC` | `6` | `6` | `12` | `20` | TravelSystem (Stable-Master Travel Web Routes & Teleportation) |
| `0x0534` | `1332` | None | `0x06C3` | `0x0126` | `2` | `2` | `4` | `3` | TutorialHintSystem (Tutorial Hint Display & Suppression) |
| `0x0535` | `1333` | None | `0x06C3` | `0x0127` | `3` | `2` | `5` | `20` | CUsageSystem (Interactive World Object Usage & Interaction Lock) |
| `0x0536` | `1334` | None | `0x06C3` | `0x01CE` | `6` | `1` | `7` | `13` | VendorSystem (NPC Merchant Buy, Sell, Buyback, Discount & Item Info) |
| `0x0537` | `1335` | None | `0x06C3` | `0x0183` | `2` | `1` | `3` | `4` | VitalSystem (Vital Display Override Request & Vital Sync) |
| `0x071A` | `1818` | None | `0x06C2` | `0x071B`, `0x071D` | `1` | `1` | `2` | `5` | WorldEventSystem (World/Zone Event Conditions, History & Admin Control) |
| `0x0728` | `1832` | None | `0x06C3` | `0x0729` | `0` | `3` | `3` | `4` | GameSetRegistry / Trait & Item Set Bonus System (0x0729 / 0x08E8) |
| `0x0739` | `1849` | None | None | `0x0736` | `2` | `0` | `2` | `3` | TargetMarking / Fellowship ReadyCheck System (0x0736) |
| `0x077B` | `1915` | None | `0x06C3` | `0x0769` | `0` | `1` | `1` | `12` | ReputationSystem (Faction Standing, Tiers & Reputation Rewards) |
| `0x077F` | `1919` | None | `0x06C3` | `0x0786` | `4` | `5` | `9` | `8` | SessionPlaySystem (MonsterPlay / Historical Session Play Control) |
| `0x078E` | `1934` | None | `0x06C3` | `0x0787` | `1` | `1` | `2` | `6` | MPMissionController / MonsterPlay Session Selection System (0x0787) |
| `0x0793` | `1939` | None | `0x06C3` | `0x0794`, `0x0795` | `1` | `1` | `2` | `7` | BarterSystem (NPC Barter Exchange & Barter Wallet) |
| `0x07A7` | `1959` | None | `0x06C3` | `0x07A6`, `0x0FF4` | `26` | `17` | `43` | `66` | HousingSystem (Purchase, Maintenance, Permissions, Decorations, Chests) |
| `0x0800` | `2048` | None | None | `0x09A5` | `2` | `0` | `2` | `4` | CMountSystem (Mount Summon, Dismount & Movement Controller Sync) |
| `0x081B` | `2075` | None | `0x06C3` | `0x0819` | `4` | `2` | `6` | `15` | OutfitSystem (Cosmetic Outfit Slot Equip, Unequip & Selection) |
| `0x083A` | `2106` | None | `0x06C3` | `0x0837` | `1` | `2` | `3` | `5` | AppearanceUI / Barber Character Customization System (0x0837) |
| `0x0849` | `2121` | None | `0x06C3` | `0x084A`, `0x084B`, `0x08F1` | `16` | `41` | `57` | `52` | CItemAdvancementSystem (Legendary Items, Reforge, Legacies, Imbuement) |
| `0x0859` | `2137` | None | `0x06C3` | `0x084E` | `2` | `4` | `6` | `11` | HobbySystem (Fishing & Hobby Proficiency, Execution & Rewards) |
| `0x08CB` | `2251` | None | `0x06C3` | `0x08CC` | `1` | `1` | `2` | `7` | NPCCommentSystem (Conditional NPC Bark / Drama Trigger) |
| `0x0908` | `2312` | None | None | `0x0909` | `1` | `0` | `1` | `1` | GameplayOptionsProfile XML Layout Import/Export Sync (0x0909) |
| `0x0958` | `2392` | None | `0x06C3` | `0x0959` | `3` | `4` | `7` | `3` | ShortcutPage / Quickslot Page Sync System (Shortcut_Current_Page, 0x0959) |
| `0x096D` | `2413` | None | None | `0x096F` | `1` | `0` | `1` | `0` | DRSSystem (Dynamic Resolution / Delayed Reward System Admin /drs) |
| `0x09BB` | `2491` | None | `0x06C3` | `0x09BD` | `0` | `1` | `1` | `0` | Dialog Callback Notification Stub (0x09BD slot 84) |
| `0x09EB` | `2539` | None | `0x06C3` | `0x09E8` | `1` | `3` | `4` | `14` | CWebStoreSystem (In-Game Store URL, Coupons & Try-On Preview) |
| `0x09EC` | `2540` | None | `0x06C3` | `0x0A52` | `0` | `4` | `4` | `4` | CSkirmishSystem (Skirmish Statistics & Skirmish Gating Sync) |
| `0x0A04` | `2564` | None | `0x06C3` | `0x0A02` | `9` | `4` | `13` | `14` | BankRepositorySystem (Vault, Shared Storage, Escrow & Chest Operations) |
| `0x0A2C` | `2604` | None | `0x06C3` | `0x0A2B` | `8` | `6` | `14` | `16` | WorldJoinSystem (Instance Finder / World Join Queue & Teleport) |
| `0x0A45` | `2629` | None | `0x06C3` | `0x0A43`, `0x0A6A`, `0x0A73` | `6` | `1` | `7` | `14` | WardrobeSystem (Cosmetic Wardrobe Storage, Dye & Slot Management) |
| `0x0A84` | `2692` | None | `0x06C3` | `0x0DD9`, `0x1062` | `1` | `1` | `2` | `1` | PVPLeaderboardSystem (/pvplb Leaderboard & PvP Virtue Selection) |
| `0x0AE6` | `2790` | None | `0x06C2` | `0x0AE7` | `0` | `1` | `1` | `0` | Tea / Telemetry / Effect Array Notification System (0x0AE7) |
| `0x0C0D` | `3085` | None | `0x06C3` | `0x0C0C`, `0x0E7D` | `1` | `4` | `5` | `17` | PaperItemSystem (Wallet Currencies / Account & Character Paper Items) |
| `0x0C17` | `3095` | None | None | `0x0C16` | `1` | `0` | `1` | `1` | Client Event 0xE1 Two-Argument Dispatch Stub (0x0C16) |
| `0x0C20` | `3104` | None | None | `0x0C23` | `1` | `0` | `1` | `1` | Bank Chest Rename System (chestinfo rename) |
| `0x0C41` | `3137` | None | `0x06C3` | `0x0C3D` | `6` | `4` | `10` | `16` | CFellowshipMakerSystem (Instance Finder Group Matchmaking Queue) |
| `0x0C4F` | `3151` | None | None | `0x0C4E` | `2` | `0` | `2` | `1` | RelicSystem (Legendary Item Relic Deconstruct & Transmute) |
| `0x0CFE` | `3326` | None | `0x06C3` | `0x0D00` | `2` | `2` | `4` | `0` | FellowshipVoteSystem (Fellowship Dismiss & Loot Rule Voting) |
| `0x0DA7` | `3495` | None | `0x06C3` | `0x0D8E` | `6` | `1` | `7` | `17` | CMountedCombatSystem (War-Steed Trait Trees & Cosmetic Appearances) |
| `0x0DD1` | `3537` | None | `0x06C3` | `0x0DD0` | `4` | `1` | `5` | `7` | PVPPreGameSystem (PvP Match Pre-Game Lobby & Team Chat) |
| `0x0E15` | `3605` | None | `0x06C3` | `0x0E16` | `5` | `3` | `8` | `6` | TraitConfigurationSystem (Trait Specialization Loadout Slots) |
| `0x0E43` | `3651` | None | None | `0x0E41` | `1` | `0` | `1` | `1` | MithrilCoinsSystem (Mithril Coin Fast-Purchase Request) |
| `0x0E7C` | `3708` | None | `0x06C3` | `0x0E7D` | `1` | `2` | `3` | `5` | CRandomItemRewardSystem (Daily/Weekly Login & Hobbit Gift Rewards) |
| `0x0EB6` | `3766` | None | `0x06C2`, `0x06C3` | `0x0F12` | `1` | `5` | `6` | `2` | CBigBattleSystem (Epic Battles Promotion Points, Medals & Siege Weapons) |
| `0x0ED5` | `3797` | None | `0x06C3` | `0x0EDD` | `4` | `6` | `10` | `9` | CMultiUsageSystem (Multi-Action Interactive Object Context Menu & Lock) |
| `0x0EF0` | `3824` | None | `0x06C3` | `0x0EFC` | `0` | `1` | `1` | `1` | DistributedChargingSystem (DistributedCharging_UserRate Sync) |
| `0x0F5C` | `3932` | None | None | `0x0F60` | `2` | `0` | `2` | `4` | Lockbox / Lootbox Disenchant & Inspection System (0x0F60) |
| `0x0FA0` | `4000` | None | `0x06C3` | `0x0F9F` | `0` | `2` | `2` | `3` | CollectionSystem (Mounts/Pets/Emotes Collection Fulfill & Refresh) |
| `0x0FBC` | `4028` | None | `0x06C3` | `0x0F71` | `1` | `2` | `3` | `13` | ItemSocketSystem (Essence Socket / Unsocket / Level Manipulation) |
| `0x0FF6` | `4086` | None | None | `0x0FFA` | `1` | `0` | `1` | `0` | CAdmin Slot 1326 Single-Integer RPC Command Stub (0x0FFA) |
| `0x1001` | `4097` | None | `0x06C3` | `0x0FFF` | `0` | `1` | `1` | `0` | CImageItemSystem (RecvCEvt_ShowImageItem UI Display) |
| `0x1012` | `4114` | None | `0x06C3` | `0x1011` | `3` | `1` | `4` | `19` | AllegianceSystem (Allegiance Faction Level, Points & Claim Rewards) |
| `0x101B` | `4123` | None | None | `0x101C` | `4` | `0` | `4` | `7` | AccountFilterSystem (Loot, Quest & Audio Filter Lists, 0x101C) |
| `0x1055` | `4181` | None | `0x06C3` | `0x1054` | `7` | `4` | `11` | `18` | CarryAllSystem (Bag of Holding / Carry-All Pseudonym & Item Management) |
| `0x1086` | `4230` | None | `0x06C3` | `0x1087` | `1` | `8` | `9` | `12` | Collection / Shortcut / Combat State UI Notification System (0x1087 / 0x1089) |
| `0x169F` | `5791` | None | `0x06C3` | `0x16A0` | `3` | `4` | `7` | `8` | NPCSystem / Neighbor NPC System (Housing NPNPC Outfits, Gifts & State) |

---

## 5. Complete Method-by-Method Census of All 1,168 WLib RPC Methods <a id="method-by-method-census"></a>

Each subsection below documents one of the 120 WLib RPC classes (`0x01F5..0x169F`), listing its class-level cross-references followed by every RPC method (`Pack (C->S)` and `Unpack (S->C)`) in virtual-slot order.

### Class `0x01F5` (`501`) — EngineOptionSystem (Gameplay Options Sanity Check & Property Sync) <a id="rpc-class-0x01f5"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x003C`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x00000068` (`Slider_HeadX`)
- **Referenced WLib Strings (`0x0F`, 5 total)**: `"NULL ENTITY"`, `"via the options codepath!  This may indicate a hacking attempt"`, `"in property group"`, `"is trying to set property"`, `"EngineOptionSystem::SanityCheckOption> Error: Entity"`
- **Referenced Native Functions (`0x15`, 10 total)**: `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00270001` (`0x14038a7e0`), `0x00270006` (`0x1403ae7b0`), `0x00270002` (`0x1403861e0`), `0x00C7001B` (`0x140387e70`), `0x00DD0008` (`0x1403b5b70`), `0x00420002` (`0x14038bc20`), `0x00790003` (`0x140396b20`), `0x00DD0001` (`0x1403b6150`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 2 total)**: `0x803C0001` @ `0x1403cf100` (fn `0x1403cf076`), `0x803C0002` @ `0x1403cefc4` (fn `0x1403cef5a`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x81F50003` | `3` | Pack (C->S) | `0x81F50002` | `(ref)` | `0x00D9000D` (`local_0`) | `0x803C0001` (`+13836`) [C++: `0x1403cf100`] | NAT: `0x00270001` (`0x14038a7e0`) |
| `0x81F50004` | `4` | Pack (C->S) | `0x81F50001` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x803C0002` (`+13728`) [C++: `0x1403cefc4`] | — |

### Class `0x04D9` (`1241`) — AIPetSystem / CosmeticPetSystem (Combat Pets & Cosmetic Pets) <a id="rpc-class-0x04d9"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x00FE`, `0x06CA`, `0x0E75`
- **Method Counts**: `6` Pack (C->S), `8` Unpack (S->C) (`14` total)
- **Referenced Properties (`pids.txt`, 37 total)**: `0x1000002B` (`MPAdvancementControl_ModStatTypeEntry`), `0x10000B70` (`AI_Pet_Master`), `0x100003FA` (`Death_EffectSpellcraft`), `0x1000074F` (`UI_VitalDisplay_Array`), `0x000000B9` (`UICore_Tree_indent_amount`), `0x1000074E` (`Bank`), `0x100019B7` (`AI_Pet_SavedPetAssist`), `0x100008DC` (`AI_Pet_SavedPetStance`), `0x00000100` (`MapNote_Level`), `0x000004B2` (`World_NameRule`), `0x10001AEC` (`GOP_DisplayPetActionSpew`), `0x1000003B` (`GCParam_AddEffect_CasterOverride`), `0x000004F0` (`LightDiffuseIntensity`), `0x10000254` (`Effect_StartupEffectID`), `0x10000526` (`Effect_StartupEffectSpellcraft`), `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x1000000F` (`Trait_Champion_DeepStrikes`), `0x100000A5` (`Skill_AttackHook_DamageQualifier`) *(+19 more)*
- **Referenced WLib Strings (`0x0F`, 17 total)**: `"normal"`, `"super"`, `"/petname <reset/view> <InstanceID>"`, `"/change petname <reset/view> <InstanceID>"`, `"reset - Resets all pet names this player owns\n  view  - Views all pet names thi"`, `"] failed to have a struct element... Either data is wrong or it failed to prune!"`, `"The PetApplyToMasterEffect at index ["`, `"AIPetSystem::BuildPetApplyToMasterEffectList"`, `"] failed to have an effect DataID... Either data is wrong or it failed to prune!"`, `"usage:  /ai [ dump \| goalqueue \| combat ]"` *(+7 more)*
- **Referenced Native Functions (`0x15`, 58 total)**: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x00230004` (`0x1403bc7d0`), `0x00750004` (`0x1403bd030`), `0x00270006` (`0x1403ae7b0`), `0x00270013` (`0x140391550`), `0x00550003` (`0x1403b4dc0`), `0x006F0027` (`0x140381e40`), `0x006F001E` (`0x1403c5b40`), `0x00750009` (`0x1403ba760`), `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760003` (`0x1403ba130`) *(+44 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 16 total)**: `0x80FE000A` @ `0x140b5035d` (fn `0x140b502ad`), `0x80FE000E` @ `0x140b4fb92` (fn `0x140b4fb44`), `0x80FE000D` @ `0x140b4fc92` (fn `0x140b4fc44`), `0x80FE0001` @ `0x140b4fdc5` (fn `0x140b4fd46`), `0x80FE0002` @ `0x140b4fec2` (fn `0x140b4fe74`), `0x80FE000B` @ `0x140b4ffc2` (fn `0x140b4ff74`), `0x80FE0009` @ `0x140b500c2` (fn `0x140b50074`), `0x80FE0007` @ `0x140b501f5` (fn `0x140b50176`), `0x80FE0003` @ `0x140b50452` (fn `0x140b50404`), `0x80FE0031` @ `0x140b5934a` (fn `0x140b59278`), `0x80FE002F` @ `0x140b73ab4` (fn `0x140b73a6b`), `0x80FE0010` @ `0x140b84807` (fn `0x140b84750`) *(+4 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84D90004` | `1` | Unpack (S->C) | `0x84D90004` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x80FE0024` (`+351732`) | NAT: `0x01550003` (`0x140b3f8a0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D9000F` | `3` | Unpack (S->C) | `0x84D9000F` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x80FE001F` (`+351824`) | NAT: `0x01550007` (`0x140b3c770`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D90024` | `4` | Unpack (S->C) | `0x84D90024` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x80FE0015` (`+351668`) | NAT: `0x01550002` (`0x140b30660`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D90026` | `5` | Unpack (S->C) | `0x84D90026` | `(f64, f64, i32/u32)` | `0x00D60007(0x06C3)` | `0x80FE0025` (`+351940`) | NAT: `0x01550009` (`0x140b3c550`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D90036` | `7` | Unpack (S->C) | `0x84D90036` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x80FE0017` (`+351868`) | NAT: `0x01550008` (`0x140b3c6d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D9004F` | `10` | Unpack (S->C) | `0x84D9004F` | `()` | `0x00D60007(0x06C3)` | `0x80FE001B` (`+352000`) | NAT: `0x0155000A` (`0x140b32d20`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D90059` | `12` | Unpack (S->C) | `0x84D90059` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x80FE0014` (`+351780`) | NAT: `0x01550006` (`0x140b3c630`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D90062` | `13` | Pack (C->S) | `0x84D90055` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x86C200E6` (`+61680`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`); MSG: `"normal"`, `"super"` |
| `0x84D90063` | `14` | Pack (C->S) | `0x84D90049` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x80FE001E` (`+351536`) | NAT: `0x0155000B` (`0x140b3c810`); PID: `0x0000005D` (`Server_World_DefaultMaxCharactersForType`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) |
| `0x84D90064` | `15` | Pack (C->S) | `0x84D90027` | `(i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x80FE001C` (`+350980`), `0x80FE0018` (`+351156`) | PID: `0x0000005D` (`Server_World_DefaultMaxCharactersForType`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10000182` (`AI_RemoteDetector_Activated`) |
| `0x84D90065` | `16` | Pack (C->S) | `0x84D9004A` | `(i64/u64, ref)` | `0x00D9000C` (`self`) | `0x86CA0002` (`+348772`) | — |
| `0x84D90066` | `17` | Pack (C->S) | `0x84D9000C` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x80FE000A` (`+350624`) [C++: `0x140b5035d`] | NAT: `0x00C70010` (`0x14038bee0`), `0x00E10003` (`0x1403c4630`); PID: `0x100000A5` (`Skill_AttackHook_DamageQualifier`), `0x10000264` (`PetChannel_Control_AlwaysSummon`), `0x100000EB` (`GRSEvent_Item_ItemClass`) |
| `0x84D90080` | `18` | Unpack (S->C) | `0x84D90080` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x80FE002D` (`+351912`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84D900B4` | `20` | Pack (C->S) | `0x84D900B3` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x8E750004` (`+518780`) | NAT: `0x00C70010` (`0x14038bee0`), `0x00E10003` (`0x1403c4630`); PID: `0x100000A5` (`Skill_AttackHook_DamageQualifier`), `0x10000264` (`PetChannel_Control_AlwaysSummon`), `0x100000EB` (`GRSEvent_Item_ItemClass`) |

### Class `0x04DA` (`1242`) — AccomplishmentSystem (Deeds & Accomplishment Directory) <a id="rpc-class-0x04da"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2`, `0x06C3` | **Paired Client Implementation Class(es)**: `0x00FC`
- **Method Counts**: `1` Pack (C->S), `3` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 32 total)**: `0x00000346` (`UICore_Element_tooltip_layout`), `0x00000256` (`PEDesc_RandomizeStartFrame`), `0x1000391A` (`Accomplishment_RequiresXP1AccessToken`), `0x1000391B` (`Accomplishment_RequiresXP2AccessToken`), `0x00000308` (`EntityVFXScript_Persistent`), `0x00000315` (`MeshFXScript_OffsetRotationYaw`), `0x00000323` (`UIB_MD_Color`), `0x000003B3` (`World_LQ_RateLimit`), `0x000003D1` (`UICore_Viewport_uiscene`), `0x000003EF` (`StreakInfoScript_Material`), `0x0000040E` (`Tools_VisibilityInheritance`), `0x000004CE` (`Entity_ContentLayers`), `0x000004EC` (`Humidity`), `0x0000050A` (`Server_World_UserLogonLimit`), `0x00000529` (`PEDesc_OriginX`), `0x00000149` (`ClearanceBBoxMax3`), `0x0000014A` (`ClearanceBBoxMin1`), `0x00000586` (`CameraFarDistance`) *(+14 more)*
- **Referenced WLib Strings (`0x0F`, 18 total)**: `"usage: [ events \| journalevents \| showall \| show \| clear ]"`, `"usage:\n/accomplishment events(prints the list of persistent events from the dir"`, `"!"`, `"> Unable to get quest singleton for:"`, `"AccomplishmentSystem::AccomplishmentIsAvailable"`, `"> ERROR: Failed to obtain the Global Accomplishment Directory!"`, `"AccomplishmentSystem::IsAutoBestowQuest"`, `"> ERROR: Failed to obtain the Class Accomplishment Directory!"`, `"> ERROR: Failed to obtain the Race Accomplishment Directory!"`, `"AccomplishmentSystem::AddAccomplishmentsForSkillMsg"` *(+8 more)*
- **Referenced Native Functions (`0x15`, 32 total)**: `0x00E10005` (`0x1403ca7d0`), `0x00120007` (`0x1403bee50`), `0x00130005` (`0x1403c50c0`), `0x00130007` (`0x1403b9f80`), `0x00130001` (`0x1403c4e10`), `0x00DD0001` (`0x1403b6150`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x001A0001` (`0x1403c4ed0`), `0x00190006` (`0x1403c2c90`), `0x00190028` (`0x140381e40`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`) *(+18 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 7 total)**: `0x80FC000F` @ `0x140b67c88` (fn `0x140b67ba0`), `0x80FC000A` @ `0x140b6ca4b` (fn `0x140b6ca00`), `0x80FC000B` @ `0x140b6cadb` (fn `0x140b6ca90`), `0x80FC000E` @ `0x140b73022` (fn `0x140b72fa0`), `0x80FC000D` @ `0x140b77bd2` (fn `0x140b77b50`), `0x80FC0008` @ `0x140b7b492` (fn `0x140b7b0cb`), `0x80FC0001` @ `0x140b8722d` (fn `0x140b87160`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84DA0004` | `1` | Unpack (S->C) | `0x84DA0004` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x80FC0006` (`+301856`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84DA0006` | `2` | Unpack (S->C) | `0x84DA0006` | `()` | `0x00D60007(0x06C2)` | `0x80FC0004` (`+302548`) | NAT: `0x012E0001` (`0x140b30050`); PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DA0008` | `4` | Pack (C->S) | `0x84DA0007` | `(i64/u64, ref)` | `0x00D9000C` (`self`) | `0x80FC0002` (`+301824`) | — |
| `0x84DA000A` | `5` | Unpack (S->C) | `0x84DA000A` | `(i32/u32)` | `0x00D60007(0x06C2)` | `0x80FC0007` (`+302624`) | NAT: `0x00110002` (`0x1403c68d0`), `0x012E0001` (`0x140b30050`); PID: `0x000006C2` (`MapNote_StringInfo`) |

### Class `0x04DC` (`1244`) — CAdmin Core Slash Commands (Appearance, Effects, Dispel, Admin Utilities) <a id="rpc-class-0x04dc"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2` | **Paired Client Implementation Class(es)**: `0x06C2`
- **Method Counts**: `45` Pack (C->S), `1` Unpack (S->C) (`46` total)
- **Referenced Properties (`pids.txt`, 9 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x0000033B` (`Render_AdjustableScale`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`), `0x10000025` (`Drama_ActionSequence`), `0x00000079` (`PEDesc_NumVFrames`), `0x0000006D` (`UICore_Resizebar_border_right`), `0x00000200` (`Slider_Fly`), `0x000000C1` (`UICore_Element_notify_on_create`), `0x000006C2` (`MapNote_StringInfo`)
- **Referenced WLib Strings (`0x0F`, 119 total)**: `"dump"`, `"client"`, `"CLIENT SKILL REGISTRY"`, `"server"`, `"Unrecognized argument:"`, `"Expecting "client" or "server"."`, `"acquire"`, `".wc"`, `"' doesn't exist."`, `"The skill '"` *(+109 more)*
- **Referenced Native Functions (`0x15`, 66 total)**: `0x00A90001` (`0x140384d60`), `0x00450002` (`0x140384ce0`), `0x0023000A` (`0x1403bf2a0`), `0x0023000B` (`0x1403c0c60`), `0x00240004` (`0x1403baae0`), `0x00240002` (`0x1403b9f80`), `0x0039001E` (`0x14039ada0`), `0x00240003` (`0x1403ba130`), `0x00440001` (`0x1403b5660`), `0x00240001` (`0x1403c4e10`), `0x00DD0001` (`0x1403b6150`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`) *(+52 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84DC0033` | `33` | Unpack (S->C) | `0x84DC0033` | `(ref, ref)` | `0x00D60007(0x06C2)` | `0x86C20056` (`vslot 0x03C2`, `+24656`) | NAT: `0x0023000A` (`0x1403bf2a0`), `0x0023000B` (`0x1403c0c60`), `0x00240004` (`0x1403baae0`); PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DC004C` | `49` | Pack (C->S) | `0x84DC0037` | `(ref, ref)` | `0x00D9000D` (`local_0`) | `0x86C20107` (`+48344`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `".wc"`, `"clientonly"` |
| `0x84DC004D` | `50` | Pack (C->S) | `0x84DC0021` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20076` (`+46364`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000025` (`Drama_ActionSequence`); MSG: `"list"`, `"reset"` |
| `0x84DC004E` | `51` | Pack (C->S) | `0x84DC003F` | `()` | `0x00D9000D` (`local_0`) | `0x86C2005F` (`+44188`) | — |
| `0x84DC004F` | `52` | Pack (C->S) | `0x84DC0024` | `(i32/u32, ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2010E` (`+43288`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002B` (`0x1403c0470`), `0x00B70001` (`0x1403c4ed0`); MSG: `"client"`, `"-v"` |
| `0x84DC0050` | `53` | Pack (C->S) | `0x84DC002E` | `(i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20042` (`+56068`), `0x86C2008D` (`+56736`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x00000079` (`PEDesc_NumVFrames`); MSG: `"/unlock> Error: You must select a lockee, or "`, `"/unlock> Error: Invalid IID format entered fo"` |
| `0x84DC0055` | `54` | Pack (C->S) | `0x84DC0017` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20076` (`+46364`), `0x86C20059` (`+47416`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000025` (`Drama_ActionSequence`); MSG: `"list"`, `"reset"` |
| `0x84DC0056` | `55` | Pack (C->S) | `0x84DC002B` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20032` (`+44596`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"/dispel> You must select a target."` |
| `0x84DC0057` | `56` | Pack (C->S) | `0x84DC0012` | `(f32)` | `0x00D9000D` (`local_0`) | `0x86C200F8` (`+61968`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`) |
| `0x84DC0058` | `57` | Pack (C->S) | `0x84DC0034` | `(ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20103` (`+45852`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"list"`, `"on"` |
| `0x84DC0059` | `58` | Pack (C->S) | `0x84DC0013` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x86C20038` (`+73692`) | NAT: `0x00B6002B` (`0x1403c0470`), `0x00B6002D` (`0x1403c1a60`), `0x00B70001` (`0x1403c4ed0`); MSG: `"pc"`, `"mpc"` |
| `0x84DC005A` | `59` | Pack (C->S) | `0x84DC0048` | `(ref, ref)` | `0x00D9000D` (`local_0`) | `0x86C200EC` (`+74304`) | NAT: `0x00DD0009` (`0x1403b59c0`), `0x00DD0008` (`0x1403b5b70`), `0x008A0001` (`0x1403b7220`); MSG: `"is the number"`, `"the payload is"` |
| `0x84DC005B` | `60` | Pack (C->S) | `0x84DC0049` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20076` (`+46364`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000025` (`Drama_ActionSequence`); MSG: `"list"`, `"reset"` |
| `0x84DC005C` | `61` | Pack (C->S) | `0x84DC0011` | `(f32, ref, i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C200EB` (`+41776`), `0x86C20178` (`+42280`), `0x86C20179` (`+42744`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002B` (`0x1403c0470`), `0x00B70001` (`0x1403c4ed0`); MSG: `".wc"`, `"/addeffectfrom> Error: Invalid IID format ent"` |
| `0x84DC0060` | `62` | Pack (C->S) | `0x84DC000A` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C200E7` (`+54796`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`); MSG: `">"`, `":"` |
| `0x84DC0061` | `63` | Pack (C->S) | `0x84DC000D` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x86C20061` (`+45104`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DC0062` | `64` | Pack (C->S) | `0x84DC0020` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20080` (`+63112`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); MSG: `"on"`, `"off"` |
| `0x84DC0063` | `65` | Pack (C->S) | `0x84DC000B` | `(i32/u32, f32, i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C200D1` (`+40312`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); MSG: `"Nothing selected to apply/remove appearance."`, `"apply"` |
| `0x84DC0065` | `67` | Pack (C->S) | `0x84DC000C` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20096` (`+67484`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60019` (`0x1403c6280`), `0x00E10016` (`0x1403ba9d0`); MSG: `"internal"`, `"That is unethical! Target may not be a player"` |
| `0x84DC0067` | `68` | Pack (C->S) | `0x84DC0029` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20095` (`+64864`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`); MSG: `"unknown effect"`, `") to server"` |
| `0x84DC0068` | `69` | Pack (C->S) | `0x84DC0046` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20076` (`+46364`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000025` (`Drama_ActionSequence`); MSG: `"list"`, `"reset"` |
| `0x84DC0069` | `70` | Pack (C->S) | `0x84DC0005` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2010C` (`+37164`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`); PID: `0x0000033B` (`Render_AdjustableScale`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`); MSG: `"dump"`, `"client"` |
| `0x84DC006B` | `72` | Pack (C->S) | `0x84DC003B` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20076` (`+46364`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000025` (`Drama_ActionSequence`); MSG: `"list"`, `"reset"` |
| `0x84DC006C` | `73` | Pack (C->S) | `0x84DC001A` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C200BB` (`+71896`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"true"`, `"t"` |
| `0x84DC006D` | `74` | Pack (C->S) | `0x84DC0030` | `(ref, ref)` | `0x00D9000D` (`local_0`) | `0x86C2000D` (`+62848`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); MSG: `"clienthotfix"`, `"Client Hotfix 5 confirmed!"` |
| `0x84DC006E` | `75` | Pack (C->S) | `0x84DC0001` | `(ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2006A` (`+41368`), `0x86C2006A` (`+41368`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); MSG: `"You must specify the name of something to tes"` |
| `0x84DC006F` | `76` | Pack (C->S) | `0x84DC0008` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C200FD` (`+57492`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"/lockstatus> Error: Invalid IID format entere"`, `"/lockstatus> Error: Need to specify a valid t"` |
| `0x84DC0070` | `77` | Pack (C->S) | `0x84DC0007` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20010` (`+44288`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`); MSG: `"/dispelall> Must select or specify the IID of"` |
| `0x84DC0073` | `78` | Pack (C->S) | `0x84DC0025` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C200D8` (`+54084`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"client"`, `"doesn't exist."` |
| `0x84DC0074` | `79` | Pack (C->S) | `0x84DC0019` | `(i32/u32, i32/u32)` | `0x00D9000D` (`local_0`) | `0x86C200B1` (`+41084`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E1001C` (`0x1403cbac0`) |
| `0x84DC0075` | `80` | Pack (C->S) | `0x84DC0039` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20104` (`+55244`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`); MSG: `">"`, `":"` |
| `0x84DC0076` | `81` | Pack (C->S) | `0x84DC003D` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2003F` (`+62192`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`); MSG: `"Invalid IID specified"`, `"No target specified!"` |
| `0x84DC0077` | `82` | Pack (C->S) | `0x84DC0016` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20086` (`+63876`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"is not a valid iid."`, `"/deleteobject> Error:"` |
| `0x84DC0078` | `83` | Pack (C->S) | `0x84DC0031` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2010C` (`+37164`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`); PID: `0x0000033B` (`Render_AdjustableScale`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`); MSG: `"dump"`, `"client"` |
| `0x84DC0079` | `84` | Pack (C->S) | `0x84DC0040` | `(ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20016` (`+61120`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"list"`, `"Invalid argument:"` |
| `0x84DC007A` | `85` | Pack (C->S) | `0x84DC0026` | `(ref, i32/u32)` | `0x00D9000D` (`local_0`) | `0x86C200BD` (`+67268`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00420003` (`0x140392ba0`); PID: `0x0000006D` (`UICore_Resizebar_border_right`) |
| `0x84DC007B` | `86` | Pack (C->S) | `0x84DC003C` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x86C20113` (`+45492`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DC007D` | `87` | Pack (C->S) | `0x84DC000E` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2010C` (`+37164`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`); PID: `0x0000033B` (`Render_AdjustableScale`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`); MSG: `"dump"`, `"client"` |
| `0x84DC0081` | `89` | Pack (C->S) | `0x84DC003E` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x86C200C6` (`+60612`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00C70010` (`0x14038bee0`); MSG: `"isn't valid!"`, `"The target IID"` |
| `0x84DC0082` | `90` | Pack (C->S) | `0x84DC0004` | `(ref, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20092` (`+64272`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`); MSG: `"unknown effect"`, `") to server"` |
| `0x84DC0083` | `91` | Pack (C->S) | `0x84DC0035` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C2004C` (`+74956`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`) |
| `0x84DC0084` | `92` | Pack (C->S) | `0x84DC0045` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x86C2006B` (`+70708`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`); MSG: `"on"`, `"off"` |
| `0x84DC0085` | `93` | Pack (C->S) | `0x84DC0041` | `(ref, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20063` (`+58304`) | NAT: `0x00E10002` (`0x1403ba7e0`) |
| `0x84DC0087` | `95` | Pack (C->S) | `0x84DC0009` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20018` (`+55692`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`); MSG: `">."`, `"Fumbling inventory and contents of <"` |
| `0x84DC0091` | `97` | Pack (C->S) | `0x84DC0090` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C201B3` (`+42252`) | — |
| `0x84DC0097` | `99` | Pack (C->S) | `0x84DC0096` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20076` (`+46364`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000025` (`Drama_ActionSequence`); MSG: `"list"`, `"reset"` |

### Class `0x04DD` (`1245`) — AdminPropertySystem (Live Property Edit, Remove & Singleton Dump) <a id="rpc-class-0x04dd"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2` | **Paired Client Implementation Class(es)**: `0x00F6`
- **Method Counts**: `8` Pack (C->S), `2` Unpack (S->C) (`10` total)
- **Referenced Properties (`pids.txt`, 10 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000001E6` (`PreprocOptions_FontGen_AllowColor`), `0x00000068` (`Slider_HeadX`), `0x000001E5` (`PreprocOptions_PlaceGen_Houses`), `0x000000C0` (`UICore_Element_ObjectOpacity`), `0x000000C9` (`UICore_Viewport_RenderToTextureHeight`), `0x000000E3` (`CinematicCameraScript_Yaw`), `0x00000200` (`Slider_Fly`), `0x000000C1` (`UICore_Element_notify_on_create`), `0x000006C2` (`MapNote_StringInfo`)
- **Referenced WLib Strings (`0x0F`, 47 total)**: `"client"`, `"server"`, `".wc"`, `"]"`, `"Could not find the client singleton named ["`, `"] to Admin Log:"`, `"Dumping client singleton ["`, `"Unrecognized property group ["`, `"...found target arg:"`, `"...first arg non-numeric:"` *(+37 more)*
- **Referenced Native Functions (`0x15`, 34 total)**: `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00450002` (`0x140384ce0`), `0x0039001C` (`0x1403aa5e0`), `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x00AB000D` (`0x1403c90b0`), `0x00DD0008` (`0x1403b5b70`), `0x00B60028` (`0x140381e40`), `0x00DD0001` (`0x1403b6150`), `0x00420003` (`0x140392ba0`), `0x00DD0012` (`0x1403b3140`) *(+20 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x80F60001` @ `0x140b91e20` (fn `0x140b91d96`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84DD0002` | `1` | Unpack (S->C) | `0x84DD0002` | `(ref, ref, ref, ref, i64/u64)` | `0x00D60007(0x06C2)` | `0x80F60010` (`+304560`) | PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DD0007` | `4` | Unpack (S->C) | `0x84DD0007` | `(ref, ref)` | `0x00D60007(0x06C2)` | `0x80F6000E` (`+304600`) | PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DD0010` | `7` | Pack (C->S) | `0x84DD000E` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80F60001` (`+304132`) [C++: `0x140b91e20`] | — |
| `0x84DD0011` | `8` | Pack (C->S) | `0x84DD0008` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20062` (`+49752`), `0x86C20062` (`+49752`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x00000068` (`Slider_HeadX`); MSG: `"client"`, `"server"` |
| `0x84DD0012` | `9` | Pack (C->S) | `0x84DD0003` | `(i32/u32, ref)` | `0x00D9000D` (`local_0`) | `0x86C2008E` (`+48732`), `0x86C2008E` (`+48732`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); PID: `0x000001E6` (`PreprocOptions_FontGen_AllowColor`), `0x00000068` (`Slider_HeadX`); MSG: `"client"`, `"server"` |
| `0x84DD0013` | `10` | Pack (C->S) | `0x84DD0004` | `(ref, ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20004` (`+50852`), `0x80F60015` (`+304492`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"remove"`, `"singleton"` |
| `0x84DD0017` | `12` | Pack (C->S) | `0x84DD0016` | `(ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20004` (`+50852`), `0x80F60016` (`+304528`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"remove"`, `"singleton"` |
| `0x84DD0019` | `14` | Pack (C->S) | `0x84DD0018` | `(ref, ref, i32/u32, ref)` | `0x00D9000D` (`local_0`) | `0x86C20004` (`+50852`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"remove"`, `"singleton"` |
| `0x84DD001B` | `16` | Pack (C->S) | `0x84DD001A` | `(ref, i32/u32, ref)` | `0x00D9000D` (`local_0`) | `0x86C20004` (`+50852`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"remove"`, `"singleton"` |
| `0x84DD001F` | `18` | Pack (C->S) | `0x84DD001D` | `(ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20232` (`+52652`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"client"`, `"server"` |

### Class `0x04DE` (`1246`) — CAdmin / Gameplay Systems Master Admin RPC Dispatcher (140 Admin Commands) <a id="rpc-class-0x04de"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2` | **Paired Client Implementation Class(es)**: `0x00FA`, `0x00FE`, `0x0100`, `0x0102`, `0x0106`, `0x010B` (+45 more)
- **Method Counts**: `140` Pack (C->S), `5` Unpack (S->C) (`145` total)
- **Referenced Properties (`pids.txt`, 1103 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x1000195C` (`Admin_PlayerTestMode`), `0x10000047` (`Craft_OfferedVocationArray`), `0x000006F6` (`BeamScript_SustainTime`), `0x000008C8` (`GeneratorTracker_Output_ContentLayers`), `0x0000097E` (`Preproc_Lbo2Lbi_MaxLinks`), `0x000009B3` (`UIPlugins_HasPluginsLoaded`), `0x000009C7` (`Kynapse_Map`), `0x10001011` (`Advancement_Level`), `0x1000006C` (`UI_QuestTree_RaidImageElement`), `0x00000BD2` (`WAL_QueryGameConfigTable_Res`), `0x10003FB9` (`Combat_ArmorDefense_ClassBased_CalcType_Cached`), `0x10006E2D` (`Combat_TacticalArmorDefense_ClassBased_CalcType_Cached`), `0x10003FBA` (`Combat_NonCommonArmorDefense_ClassBased_CalcType_Cached`), `0x10004056` (`NPC_ActionPicker_LevelBestower_LevelArray`), `0x10000169` (`Advancement_SkillBeingUntrained`), `0x10000273` (`Skill_AlwaysApplyUserEffects`), `0x100001D9` (`GRSEvent_Pedigree_IsDeleted`) *(+1085 more)*
- **Referenced WLib Strings (`0x0F`, 1065 total)**: `"CAdmin::DoCmdDataID> Past args check."`, `"Client:"`, `"send"`, `":"`, `"sending to"`, `"Send was successful"`, `"error"`, `"Failed to send to"`, `"dump"`, `"Emote commands only work on a player."` *(+1055 more)*
- **Referenced Native Functions (`0x15`, 846 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+832 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 670 total)**: `0x80FD0001` @ `0x140b93345` (fn `0x140b932db`), `0x80FD0002` @ `0x140b97720` (fn `0x140b97696`), `0x80FD0003` @ `0x140b97850` (fn `0x140b977c6`), `0x80FD0024` @ `0x140b93905` (fn `0x140b9389b`), `0x80FD0026` @ `0x140b96695` (fn `0x140b9662b`), `0x80FD0028` @ `0x140b97575` (fn `0x140b9750b`), `0x80FD0029` @ `0x140b94715` (fn `0x140b94696`), `0x80FD002A` @ `0x140b94cb5` (fn `0x140b94c36`), `0x80FD002B` @ `0x140b94a75` (fn `0x140b949f6`), `0x80FD002C` @ `0x140b94385` (fn `0x140b94306`), `0x80FD002D` @ `0x140b94265` (fn `0x140b941e6`), `0x80FD002E` @ `0x140b94ef5` (fn `0x140b94e76`) *(+658 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84DE0018` | `19` | Unpack (S->C) | `0x84DE0018` | `(ref)` | `0x00D60007(0x06C2)` | `0x80FD0014` (`+318736`) | NAT: `0x0137000C` (`0x140b45350`); PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DE001B` | `21` | Pack (C->S) | `0x84DE0003` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0002` (`+314596`) [C++: `0x140b97720`], `0x81CC0016` (`+1611336`) | NAT: `0x00B60006` (`0x1403c2c90`), `0x00B60029` (`0x1403c5260`); MSG: `"AdminUI: TeleChar"`, `"Attempting to teleport..."` |
| `0x84DE001C` | `22` | Pack (C->S) | `0x84DE000F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD000A` (`+308584`), `0x80FD0006` (`+308824`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`) |
| `0x84DE001D` | `23` | Pack (C->S) | `0x84DE0013` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD000A` (`+308584`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`) |
| `0x84DE001E` | `24` | Pack (C->S) | `0x84DE0017` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8AF4000B` (`+837124`) | — |
| `0x84DE001F` | `25` | Pack (C->S) | `0x84DE0014` | `(ref, ref)` | `0x00D9000D` (`local_0`) | `0x811F0035` (`+1322508`) | NAT: `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`), `0x00B60026` (`0x1403bbc50`); PID: `0x10000142` (`Hotspot_FX_OnTriggerIntensity`); MSG: `"on"`, `"nearbyitems"` |
| `0x84DE0020` | `26` | Pack (C->S) | `0x84DE0005` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD000F` (`+310352`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0021` | `27` | Pack (C->S) | `0x84DE000B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x811A000D` (`+1246816`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0022` | `28` | Pack (C->S) | `0x84DE0015` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0028` (`+314372`) [C++: `0x140b97575`], `0x8102003A` (`+487832`) | NAT: `0x00B60006` (`0x1403c2c90`), `0x00E10010` (`0x1403b7f00`), `0x00E10005` (`0x1403ca7d0`); MSG: `"AdminUI: Snoop"`, `"on"` |
| `0x84DE0023` | `29` | Pack (C->S) | `0x84DE000D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0026` (`+314032`) [C++: `0x140b96695`], `0x81290011` (`+550956`) | NAT: `0x00B60006` (`0x1403c2c90`), `0x00E10005` (`0x1403ca7d0`), `0x00B60029` (`0x1403c5260`); MSG: `"AdminUI: NeverSayDie"`, `"on"` |
| `0x84DE0024` | `30` | Pack (C->S) | `0x84DE0011` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0003` (`+314700`) [C++: `0x140b97850`], `0x81CC000C` (`+1611540`) | NAT: `0x00B60006` (`0x1403c2c90`); MSG: `"AdminUI: TeleTo"` |
| `0x84DE0025` | `31` | Pack (C->S) | `0x84DE0016` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81290008` (`+550292`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: Invalid Target."` |
| `0x84DE0026` | `32` | Pack (C->S) | `0x84DE0010` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0011` (`+310088`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0027` | `33` | Pack (C->S) | `0x84DE000E` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81730030` (`+991884`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`) |
| `0x84DE0028` | `34` | Pack (C->S) | `0x84DE0008` | `(ref)` | `0x00D9000D` (`local_0`) | `0x810E0007` (`+744232`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0029` | `35` | Pack (C->S) | `0x84DE0019` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81020037` (`+487304`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE002A` | `36` | Pack (C->S) | `0x84DE000A` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD000E` (`+308344`), `0x80FD0001` (`+313088`) [C++: `0x140b93345`] | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60006` (`0x1403c2c90`); MSG: `"AdminUI: Cloak"`, `"off"` |
| `0x84DE002B` | `37` | Pack (C->S) | `0x84DE0009` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8102002B` (`+487980`) | NAT: `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: You must select a target!"` |
| `0x84DE002C` | `38` | Pack (C->S) | `0x84DE0012` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0012` (`+308504`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE002D` | `39` | Pack (C->S) | `0x84DE000C` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0009` (`+310324`) | — |
| `0x84DE0030` | `42` | Pack (C->S) | `0x84DE002E` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0018` (`+309044`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0031` | `43` | Pack (C->S) | `0x84DE002F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0015` (`+309228`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0039` | `48` | Pack (C->S) | `0x84DE0033` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816C013B` (`+760996`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE003D` | `49` | Pack (C->S) | `0x84DE0038` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816F0014` (`+824768`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`); MSG: `"set"`, `"Error: Invalid Target."` |
| `0x84DE003E` | `50` | Pack (C->S) | `0x84DE0036` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FE002B` (`+350380`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: Invalid Target."` |
| `0x84DE003F` | `51` | Pack (C->S) | `0x84DE0037` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81190011` (`+1220232`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: Invalid Target."` |
| `0x84DE0041` | `53` | Pack (C->S) | `0x84DE0040` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816F0017` (`+824484`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: Invalid Target."` |
| `0x84DE0042` | `54` | Pack (C->S) | `0x84DE0043` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81020049` (`+487460`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0044` | `56` | Pack (C->S) | `0x84DE0045` | `(ref, ref)` | `0x00D9000D` (`local_0`) | `0x8185007B` (`+320676`) | NAT: `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`), `0x00B60026` (`0x1403bbc50`); MSG: `"on"` |
| `0x84DE0047` | `59` | Pack (C->S) | `0x84DE0046` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x81850082` (`+320920`) | — |
| `0x84DE0049` | `61` | Pack (C->S) | `0x84DE0048` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x817A00C7` (`+1509604`) | — |
| `0x84DE004B` | `63` | Pack (C->S) | `0x84DE004A` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x81250010` (`+1554200`) | — |
| `0x84DE004D` | `65` | Pack (C->S) | `0x84DE004C` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0019` (`+310432`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE004E` | `66` | Pack (C->S) | `0x84DE004F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816C0135` (`+760968`) | — |
| `0x84DE0050` | `68` | Pack (C->S) | `0x84DE0051` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD001B` (`+310512`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0054` | `72` | Pack (C->S) | `0x84DE0052` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x80FD001E` (`+310736`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00550003` (`0x1403b4dc0`); MSG: `"Error: Target must be a player!"` |
| `0x84DE0055` | `73` | Pack (C->S) | `0x84DE0053` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x80FD001F` (`+310592`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00550003` (`0x1403b4dc0`); MSG: `"Error: Target must be a player!"` |
| `0x84DE0057` | `75` | Pack (C->S) | `0x84DE0056` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0021` (`+310880`), `0x80FD0024` (`+313432`) [C++: `0x140b93905`] | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60006` (`0x1403c2c90`), `0x00E10005` (`0x1403ca7d0`); MSG: `"AdminUI: Deaf"`, `"on"` |
| `0x84DE0059` | `77` | Pack (C->S) | `0x84DE0058` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD0029` (`+314936`) [C++: `0x140b94715`] | — |
| `0x84DE005A` | `78` | Pack (C->S) | `0x84DE005B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x871D0001` (`+1836340`) | — |
| `0x84DE0062` | `86` | Pack (C->S) | `0x84DE005C` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD002A` (`+315068`) [C++: `0x140b94cb5`] | — |
| `0x84DE0063` | `87` | Pack (C->S) | `0x84DE0061` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD002C` (`+315024`) [C++: `0x140b94385`] | — |
| `0x84DE0064` | `88` | Pack (C->S) | `0x84DE005E` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD002B` (`+314980`) [C++: `0x140b94a75`] | — |
| `0x84DE0065` | `89` | Pack (C->S) | `0x84DE0060` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD002D` (`+315156`) [C++: `0x140b94265`] | — |
| `0x84DE0066` | `90` | Pack (C->S) | `0x84DE005F` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD002F` (`+315112`) [C++: `0x140b95015`] | — |
| `0x84DE0067` | `91` | Pack (C->S) | `0x84DE005D` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD002E` (`+315200`) [C++: `0x140b94ef5`] | — |
| `0x84DE0069` | `92` | Unpack (S->C) | `0x84DE0069` | `(ref)` | `0x00D60007(0x06C2)` | `0x80FD0030` (`+318772`) | NAT: `0x0137000F` (`0x140b44950`); PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DE006E` | `95` | Pack (C->S) | `0x84DE006D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x87570001` (`+1623856`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE006F` | `96` | Pack (C->S) | `0x84DE006C` | `(ref)` | `0x00D9000D` (`local_0`) | `0x87560001` (`+1455408`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0071` | `98` | Pack (C->S) | `0x84DE0070` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x8113001C` (`+1140612`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0073` | `100` | Pack (C->S) | `0x84DE0072` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0037` (`+310960`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0075` | `102` | Pack (C->S) | `0x84DE0074` | `(ref)` | `0x00D9000D` (`local_0`) | `0x87690001` (`+1456136`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0079` | `104` | Pack (C->S) | `0x84DE0078` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD003A` (`+315244`) [C++: `0x140b944c3`] | — |
| `0x84DE007B` | `106` | Pack (C->S) | `0x84DE007A` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x80FA0071` (`+377932`) | — |
| `0x84DE007D` | `108` | Pack (C->S) | `0x84DE007C` | `(ref)` | `0x00D9000D` (`local_0`) | `0x87860001` (`+1473548`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE007F` | `110` | Pack (C->S) | `0x84DE007E` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD003B` (`+314892`) [C++: `0x140b945f5`] | — |
| `0x84DE0081` | `112` | Pack (C->S) | `0x84DE0080` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD003D` (`+315292`) [C++: `0x140b94b95`] | — |
| `0x84DE0086` | `114` | Pack (C->S) | `0x84DE0085` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81C2009F` (`+1213792`) | — |
| `0x84DE0088` | `116` | Pack (C->S) | `0x84DE0087` | `(ref)` | `0x00D9000D` (`local_0`) | `0x87A60001` (`+791496`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE008A` | `118` | Pack (C->S) | `0x84DE0089` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0040` (`+311064`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"on"`, `"off"` |
| `0x84DE008E` | `122` | Pack (C->S) | `0x84DE008D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0042` (`+311344`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00550003` (`0x1403b4dc0`), `0x00E10010` (`0x1403b7f00`); MSG: `"Error: Must specify a player or target must b"` |
| `0x84DE008F` | `123` | Pack (C->S) | `0x84DE008C` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0044` (`+311612`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00550003` (`0x1403b4dc0`), `0x00E10010` (`0x1403b7f00`); MSG: `"Error: Must specify a player or target must b"` |
| `0x84DE0090` | `124` | Pack (C->S) | `0x84DE008B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0043` (`+311584`) | — |
| `0x84DE0092` | `126` | Pack (C->S) | `0x84DE0091` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C2015F` (`+81444`) | NAT: `0x00B6002B` (`0x1403c0470`), `0x00B6002D` (`0x1403c1a60`), `0x00B70001` (`0x1403c4ed0`); MSG: `"send"`, `":"` |
| `0x84DE0094` | `128` | Pack (C->S) | `0x84DE0093` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816C0171` (`+761104`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: Invalid Target."` |
| `0x84DE0096` | `130` | Pack (C->S) | `0x84DE0095` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD0045` (`+315336`) [C++: `0x140b94835`] | — |
| `0x84DE0099` | `131` | Unpack (S->C) | `0x84DE0099` | `(ref)` | `0x00D60007(0x06C2)` | `0x80FD0048` (`+318872`) | NAT: `0x01370017` (`0x140b45400`); PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DE009B` | `133` | Unpack (S->C) | `0x84DE009B` | `(ref)` | `0x00D60007(0x06C2)` | `0x80FD0049` (`+318836`) | NAT: `0x01370016` (`0x140b454b0`); PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DE00A3` | `135` | Pack (C->S) | `0x84DE009A` | `(i32/u32, i32/u32)` | `0x00D9000D` (`local_0`) | `0x80FD0047` (`+315540`) [C++: `0x140b95522`] | — |
| `0x84DE00A4` | `136` | Pack (C->S) | `0x84DE009E` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x80FD0046` (`+315512`) [C++: `0x140b963f4`] | — |
| `0x84DE00A6` | `138` | Pack (C->S) | `0x84DE00A5` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FA0016` (`+376560`), `0x80FA0016` (`+376560`), `0x80FA0016` (`+376560`), `0x80FA0016` (`+376560`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"openui"`, `"ui"` |
| `0x84DE00A7` | `139` | Pack (C->S) | `0x84DE00A8` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81290015` (`+550624`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00550003` (`0x1403b4dc0`), `0x00DD0006` (`0x1403b6370`); MSG: `"Error: Invalid Target."` |
| `0x84DE00AC` | `142` | Pack (C->S) | `0x84DE00AB` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD004A` (`+309892`) | — |
| `0x84DE00AE` | `144` | Pack (C->S) | `0x84DE00AD` | `(ref)` | `0x00D9000D` (`local_0`) | `0x884E0001` (`+778672`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00B2` | `146` | Pack (C->S) | `0x84DE00AF` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD004C` (`+315380`) [C++: `0x140b94955`] | — |
| `0x84DE00B7` | `148` | Pack (C->S) | `0x84DE00B6` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD004E` (`+309124`) | — |
| `0x84DE00B8` | `149` | Pack (C->S) | `0x84DE00B9` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81140006` (`+1143116`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00BA` | `151` | Pack (C->S) | `0x84DE00BB` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86D00004` (`+557608`) | — |
| `0x84DE00BD` | `154` | Pack (C->S) | `0x84DE00BC` | `(ref)` | `0x00D9000D` (`local_0`) | `0x89230001` (`+1256272`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00BE` | `155` | Pack (C->S) | `0x84DE00BF` | `(ref)` | `0x00D9000D` (`local_0`) | `0x892E0010` (`+840676`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002B` (`0x1403c0470`), `0x00B70001` (`0x1403c4ed0`); MSG: `"manage"`, `"swapslots"` |
| `0x84DE00C3` | `158` | Pack (C->S) | `0x84DE00C2` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000D` (`local_0`) | `0x80FD004F` (`+315572`) [C++: `0x140b97dcd`] | — |
| `0x84DE00C5` | `160` | Pack (C->S) | `0x84DE00C4` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0050` (`+311880`) | — |
| `0x84DE00C8` | `163` | Pack (C->S) | `0x84DE00C7` | `(i32/u32, ref)` | `0x00D9000D` (`local_0`) | `0x80FD0054` (`+308180`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00C9` | `164` | Pack (C->S) | `0x84DE00C6` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0053` (`+308264`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00CD` | `166` | Pack (C->S) | `0x84DE00CC` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0055` (`+309920`) | — |
| `0x84DE00CF` | `168` | Pack (C->S) | `0x84DE00CE` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0056` (`+309456`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00D0` | `169` | Pack (C->S) | `0x84DE00D1` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x80FD0058` (`+309948`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`) |
| `0x84DE00D2` | `171` | Pack (C->S) | `0x84DE00D3` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD005A` (`+311908`) | — |
| `0x84DE00D5` | `174` | Pack (C->S) | `0x84DE00D4` | `(ref)` | `0x00D9000D` (`local_0`) | `0x89A50006` (`+1146332`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00D7` | `176` | Pack (C->S) | `0x84DE00D6` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD005D` (`+311936`) | — |
| `0x84DE00DB` | `178` | Pack (C->S) | `0x84DE00DA` | `(ref)` | `0x00D9000D` (`local_0`) | `0x89E80002` (`+1819744`) | NAT: `0x00B60001` (`0x1403b8d80`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`); MSG: `"filtertokens"`, `"navigate"` |
| `0x84DE00DD` | `180` | Pack (C->S) | `0x84DE00DC` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8A020001` (`+388056`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`); MSG: `"ui"` |
| `0x84DE00DF` | `182` | Pack (C->S) | `0x84DE00DE` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8A2B000C` (`+1844504`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00E3` | `184` | Pack (C->S) | `0x84DE00E2` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8A430001` (`+1814392`) | — |
| `0x84DE00E5` | `186` | Pack (C->S) | `0x84DE00E4` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81020061` (`+487684`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE00EA` | `188` | Pack (C->S) | `0x84DE00E9` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8A520006` (`+1512204`) | — |
| `0x84DE00EC` | `190` | Pack (C->S) | `0x84DE00EB` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD005E` (`+315424`) [C++: `0x140b94dd5`] | — |
| `0x84DE00EE` | `192` | Pack (C->S) | `0x84DE00ED` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8100005E` (`+475404`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00B6001E` (`0x1403c5c30`); MSG: `"ERROR: Missing required sub-command parameter"`, `"pointstopercent"` |
| `0x84DE00F1` | `195` | Pack (C->S) | `0x84DE00F0` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD0061` (`+314804`) [C++: `0x140b93625`] | — |
| `0x84DE00F2` | `196` | Pack (C->S) | `0x84DE00EF` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD0062` (`+314848`) [C++: `0x140b93505`] | — |
| `0x84DE00F4` | `198` | Pack (C->S) | `0x84DE00F3` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C201B7` (`+53548`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"CAdmin::DoCmdDataID> Past args check."`, `"Client:"` |
| `0x84DE00F6` | `200` | Pack (C->S) | `0x84DE00F5` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0064` (`+311992`) | — |
| `0x84DE00FA` | `202` | Pack (C->S) | `0x84DE00F9` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x80FD0066` (`+315608`) | — |
| `0x84DE0108` | `204` | Pack (C->S) | `0x84DE0107` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD006A` (`+312020`) | — |
| `0x84DE010A` | `206` | Pack (C->S) | `0x84DE0109` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FD006B` (`+315468`) [C++: `0x140b95135`] | — |
| `0x84DE010C` | `208` | Pack (C->S) | `0x84DE010B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8BF70002` (`+1207856`) | — |
| `0x84DE0110` | `210` | Pack (C->S) | `0x84DE010D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD006D` (`+312048`) | — |
| `0x84DE0112` | `212` | Pack (C->S) | `0x84DE0111` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0070` (`+311964`) | — |
| `0x84DE0114` | `214` | Pack (C->S) | `0x84DE0113` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8C3D0001` (`+728872`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0116` | `216` | Pack (C->S) | `0x84DE0115` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0073` (`+312276`) | — |
| `0x84DE0118` | `218` | Pack (C->S) | `0x84DE0117` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0074` (`+312304`) | — |
| `0x84DE011A` | `220` | Pack (C->S) | `0x84DE0119` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0076` (`+312332`) | — |
| `0x84DE0120` | `222` | Pack (C->S) | `0x84DE011F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD007D` (`+312360`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60002` (`0x1403c2eb0`) |
| `0x84DE0122` | `224` | Pack (C->S) | `0x84DE0121` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD007F` (`+312580`) | — |
| `0x84DE0124` | `226` | Pack (C->S) | `0x84DE0123` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0080` (`+309704`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0126` | `228` | Pack (C->S) | `0x84DE0125` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0083` (`+312608`) | — |
| `0x84DE012E` | `230` | Pack (C->S) | `0x84DE012D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C201F1` (`+84252`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0130` | `232` | Pack (C->S) | `0x84DE012F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0087` (`+312636`) | — |
| `0x84DE0131` | `233` | Unpack (S->C) | `0x84DE0131` | `(ref)` | `0x00D60007(0x06C2)` | `0x80FD0088` (`+318808`) | PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x84DE0134` | `235` | Pack (C->S) | `0x84DE0133` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0089` (`+310296`) | — |
| `0x84DE0136` | `237` | Pack (C->S) | `0x84DE0135` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86DF000E` (`+1539460`) | — |
| `0x84DE0138` | `239` | Pack (C->S) | `0x84DE0137` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x81240006` (`+1523276`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE013A` | `241` | Pack (C->S) | `0x84DE0139` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD008B` (`+312664`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE013C` | `243` | Pack (C->S) | `0x84DE013B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x81CC0024` (`+1611644`) | — |
| `0x84DE013E` | `245` | Pack (C->S) | `0x84DE013D` | `()` | `0x00D9000D` (`local_0`) | `0x86C20204` (`+84720`) | NAT: `0x00DD0001` (`0x1403b6150`); MSG: `"Client Version:"` |
| `0x84DE0140` | `247` | Pack (C->S) | `0x84DE013F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86CB000F` (`+355652`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`); MSG: `"dump"`, `"client"` |
| `0x84DE0142` | `249` | Pack (C->S) | `0x84DE0141` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8106000E` (`+513672`) | — |
| `0x84DE0144` | `251` | Pack (C->S) | `0x84DE0143` | `(ref)` | `0x00D9000D` (`local_0`) | `0x810B0013` (`+561808`) | — |
| `0x84DE0146` | `253` | Pack (C->S) | `0x84DE0145` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8F12000B` (`+399336`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"dbuviz"`, `"Sending request to server..."` |
| `0x84DE0148` | `255` | Pack (C->S) | `0x84DE0147` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8EDD0017` (`+1164808`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`); MSG: `"dumpcache"`, `"Client MultiUsage Registry Cache:"` |
| `0x84DE014A` | `257` | Pack (C->S) | `0x84DE0149` | `(ref)` | `0x00D9000D` (`local_0`) | `0x89E80057` (`+1820668`) | — |
| `0x84DE014C` | `259` | Pack (C->S) | `0x84DE014B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C20218` (`+82772`) | — |
| `0x84DE014E` | `261` | Pack (C->S) | `0x84DE014D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8F710001` (`+941832`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`) |
| `0x84DE0150` | `263` | Pack (C->S) | `0x84DE014F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD008D` (`+311852`) | — |
| `0x84DE0152` | `265` | Pack (C->S) | `0x84DE0151` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x86C2021D` (`+82824`) | NAT: `0x00550003` (`0x1403b4dc0`), `0x00B60029` (`0x1403c5260`); MSG: `"Emote commands only work on a player."` |
| `0x84DE0154` | `267` | Pack (C->S) | `0x84DE0153` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8729000C` (`+1477408`) | NAT: `0x00B60001` (`0x1403b8d80`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`); MSG: `"dump"`, `"..."` |
| `0x84DE0156` | `269` | Pack (C->S) | `0x84DE0155` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8F9F0006` (`+463408`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`) |
| `0x84DE0158` | `271` | Pack (C->S) | `0x84DE0157` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C20223` (`+45440`) | — |
| `0x84DE015A` | `273` | Pack (C->S) | `0x84DE0159` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x89E80065` (`+1822128`) | — |
| `0x84DE015C` | `275` | Pack (C->S) | `0x84DE015B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x90080001` (`+969756`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE015E` | `277` | Pack (C->S) | `0x84DE015D` | `(ref)` | `0x00D9000D` (`local_0`) | `0x90110002` (`+359700`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x84DE0160` | `279` | Pack (C->S) | `0x84DE015F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD008F` (`+312776`) | NAT: `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x00DD0001` (`0x1403b6150`); MSG: `"client"`, `"server"` |
| `0x84DE0162` | `281` | Pack (C->S) | `0x84DE0161` | `(ref)` | `0x00D9000D` (`local_0`) | `0x80FD0091` (`+318972`) | — |
| `0x84DE020A` | `283` | Pack (C->S) | `0x84DE0209` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x80FD00E4` (`+319000`) | — |
| `0x84DE020C` | `285` | Pack (C->S) | `0x84DE020B` | `(ref)` | `0x00D9000D` (`local_0`) | `0x9054004B` (`+864720`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`); MSG: `"Error: Invalid Target."` |

### Class `0x04DF` (`1247`) — AdvancementSystem (Character Level, XP, Class Skill Training) <a id="rpc-class-0x04df"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0185`
- **Method Counts**: `3` Pack (C->S), `4` Unpack (S->C) (`7` total)
- **Referenced Properties (`pids.txt`, 23 total)**: `0x10000047` (`Craft_OfferedVocationArray`), `0x000006F6` (`BeamScript_SustainTime`), `0x000008C8` (`GeneratorTracker_Output_ContentLayers`), `0x0000097E` (`Preproc_Lbo2Lbi_MaxLinks`), `0x000009B3` (`UIPlugins_HasPluginsLoaded`), `0x000009C7` (`Kynapse_Map`), `0x10001011` (`Advancement_Level`), `0x1000006C` (`UI_QuestTree_RaidImageElement`), `0x00000BD2` (`WAL_QueryGameConfigTable_Res`), `0x10003FB9` (`Combat_ArmorDefense_ClassBased_CalcType_Cached`), `0x10006E2D` (`Combat_TacticalArmorDefense_ClassBased_CalcType_Cached`), `0x10003FBA` (`Combat_NonCommonArmorDefense_ClassBased_CalcType_Cached`), `0x10004056` (`NPC_ActionPicker_LevelBestower_LevelArray`), `0x10000169` (`Advancement_SkillBeingUntrained`), `0x10000273` (`Skill_AlwaysApplyUserEffects`), `0x100001D9` (`GRSEvent_Pedigree_IsDeleted`), `0x100002BD` (`CounterBox_CurrentCount`), `0x1000010B` (`Conjunction_ListenerTarget`) *(+5 more)*
- **Referenced WLib Strings (`0x0F`, 28 total)**: `"on"`, `"usage:   /xp award <points> [ <experienceawardflags> [<chllvl>] ] ]\n         /x"`, `"/advancement [grantskills \| levelup \| grantnovice]"`, `"grantskills <max_level> - Grants the Player Skills up to the given level.\n   le"`, `"!"`, `"ERROR: No level table for class"`, `"AdvancementSystem::GetUntrainedBuyableSkills"`, `"AdvancementSystem::GetUntrainedBuyableCharacteristics"`, `"AdvancementSystem::UI_InqCharacteristicTrainData"`, `"> ERROR: Undef character class type passed to this function."` *(+18 more)*
- **Referenced Native Functions (`0x15`, 45 total)**: `0x00C7000C` (`0x1403afdf0`), `0x00C70003` (`0x14037c2f0`), `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`), `0x00B60026` (`0x1403bbc50`), `0x00E10010` (`0x1403b7f00`), `0x00DD0008` (`0x1403b5b70`), `0x01300005` (`0x140b3bb70`), `0x00E70001` (`0x140821170`), `0x00DD0001` (`0x1403b6150`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`), `0x017B0002` (`0x140a82870`) *(+31 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 25 total)**: `0x8185000C` @ `0x140aa0972` (fn `0x140aa08f0`), `0x81850010` @ `0x140aa0b12` (fn `0x140aa0a90`), `0x818500AC` @ `0x140aa0a42` (fn `0x140aa09c0`), `0x81850004` @ `0x140a93ad4` (fn `0x140a93a50`), `0x818500E4` @ `0x140a93bf2` (fn `0x140a93ba4`), `0x81850005` @ `0x140a94372` (fn `0x140a94310`), `0x81850003` @ `0x140a94444` (fn `0x140a943c0`), `0x818500B4` @ `0x140a9ccce` (fn `0x140a9cc70`), `0x81850002` @ `0x140a9e104` (fn `0x140a9e080`), `0x81850009` @ `0x140a9e240` (fn `0x140a9e210`), `0x81850007` @ `0x140a9e403` (fn `0x140a9e3d0`), `0x818500E3` @ `0x140a9e480` (fn `0x140a9e450`) *(+13 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84DF0003` | `1` | Unpack (S->C) | `0x84DF0003` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C300AC` (`vslot 0x02C5`, `+127420`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84DF0016` | `6` | Unpack (S->C) | `0x84DF0016` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81850061` (`+321172`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84DF001B` | `8` | Unpack (S->C) | `0x84DF001B` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8185005C` (`+321148`) | NAT: `0x01300005` (`0x140b3bb70`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84DF001E` | `9` | Unpack (S->C) | `0x84DF001E` | `(ref)` | `0x00D60007(0x06C3)` | `0x86C30049` (`vslot 0x0276`, `+127376`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84DF0021` | `12` | Pack (C->S) | `0x84DF001A` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81850010` (`+323004`) [C++: `0x140aa0b12`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84DF0023` | `14` | Pack (C->S) | `0x84DF0005` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8185000C` (`+321744`) [C++: `0x140aa0972`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84DF002E` | `18` | Pack (C->S) | `0x84DF002D` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x818500AC` (`+323144`) [C++: `0x140aa0a42`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x04E1` (`1249`) — AlertSystem (Client/Server Alert Registry) <a id="rpc-class-0x04e1"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x06CB`
- **Method Counts**: `1` Pack (C->S), `2` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x10000077` (`EffectGenerator_EffectData_TokenID`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 8 total)**: `"usage: /alert [cmd]"`, `"/alerts dump <client\|server> : Displays the alerts in the alert registry on the"`, `"/alerts add <AlertType> : Attempts to add the given alert type to the player"`, `"/alerts remove <AlertType> : Attempts to remove the given alert type to the play"`, `"No alerts"`, `"Alerts:"`, `"dump"`, `"client"`
- **Referenced Native Functions (`0x15`, 14 total)**: `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`), `0x00B70001` (`0x1403c4ed0`), `0x00E10005` (`0x1403ca7d0`), `0x00DD0008` (`0x1403b5b70`), `0x00DD0001` (`0x1403b6150`), `0x00B60029` (`0x1403c5260`), `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`), `0x01310001` (`0x140b2ffb0`), `0x01310002` (`0x140b3f1f0`), `0x00B60001` (`0x1403b8d80`), `0x00E70001` (`0x140821170`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x86CB000C` @ `0x140b612dc` (fn `0x140b61240`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84E10006` | `2` | Unpack (S->C) | `0x84E10006` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86C3005E` (`vslot 0x0289`, `+129268`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E10007` | `3` | Unpack (S->C) | `0x84E10007` | `(ref)` | `0x00D60007(0x06C3)` | `0x86C300F0` (`vslot 0x02FC`, `+129240`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E10009` | `4` | Pack (C->S) | `0x84E10004` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x86CB0003` (`+356000`) | — |

### Class `0x04E3` (`1251`) — AreaSystem Client->Server (Area Query & WorldJoin Area Check) <a id="rpc-class-0x04e3"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x00FF`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 19 total)**: `0x00000092` (`Area_DID`), `0x00000625` (`Dungeon_DID`), `0x0000053B` (`Volume_IsOutside`), `0x10000983` (`Area_CurrentArea`), `0x1000013D` (`Dungeon_Name`), `0x00000198` (`Area_Name`), `0x100011A1` (`Area_Territory`), `0x10001494` (`Area_Region`), `0x100001E6` (`Landmark_DataID`), `0x00000314` (`MeshFXScript_Scale`), `0x00000320` (`WB_Acceleration`), `0x10000459` (`Skill_CanBeEvaded`), `0x10000E00` (`Area_DiscoveredDID`), `0x100000C0` (`Quest_CompletionConditionArray`), `0x100007AC` (`Dungeon_CurrentDungeon`), `0x10000039` (`GCParam_Properties_List`), `0x00000753` (`WCGenChecksum_Array`), `0x000006C7` (`PlaceGen_PlaceName`) *(+1 more)*
- **Referenced WLib Strings (`0x0F`, 14 total)**: `"]."`, `"Was not able to get the area data id based on the players position["`, `"AreaSystem::IsPlayerAllowedToWorldJoin"`, `"] from the players position["`, `"AreaSystem::IsPlayerAllowedToWorldJoin> Was not able to get the area based on th"`, `"query"`, `""."`, `"Invalid subcommand ""`, `"Missing required Area definition name parameter."`, `".wc"` *(+4 more)*
- **Referenced Native Functions (`0x15`, 36 total)**: `0x00E7000A` (`0x140821770`), `0x00A9003F` (`0x1403a42c0`), `0x00990004` (`0x140389fa0`), `0x00A10007` (`0x14039c1c0`), `0x00A90012` (`0x140391a60`), `0x00270024` (`0x140392100`), `0x00A1000B` (`0x14039c2f0`), `0x00A90015` (`0x1403919a0`), `0x00A1000E` (`0x1403b36b0`), `0x00DD0001` (`0x1403b6150`), `0x01AA0007` (`0x140a7db60`), `0x00390001` (`0x1403b97d0`), `0x00B80004` (`0x1403abbd0`), `0x00A60015` (`0x1403a9640`) *(+22 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 2 total)**: `0x80FF000F` @ `0x140b81110` (fn `0x140b81060`), `0x80FF0011` @ `0x140b819b0` (fn `0x140b81900`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84E30002` | `2` | Pack (C->S) | `0x84E30001` | `(ref)` | `0x00D9000C` (`self`) | `0x80FF0008` (`+372904`) | NAT: `0x00B70009` (`0x140381e40`), `0x00B70003` (`0x1403ba520`), `0x00AB000D` (`0x1403c90b0`); MSG: `"Missing required Area definition name paramet"`, `".wc"` |

### Class `0x04E4` (`1252`) — AreaSystem Server->Client (Area Discovery & Region Sync) <a id="rpc-class-0x04e4"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x00FF`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 20 total)**: `0x00000092` (`Area_DID`), `0x00000625` (`Dungeon_DID`), `0x0000053B` (`Volume_IsOutside`), `0x10000983` (`Area_CurrentArea`), `0x1000013D` (`Dungeon_Name`), `0x00000198` (`Area_Name`), `0x100011A1` (`Area_Territory`), `0x10001494` (`Area_Region`), `0x100001E6` (`Landmark_DataID`), `0x00000314` (`MeshFXScript_Scale`), `0x00000320` (`WB_Acceleration`), `0x10000459` (`Skill_CanBeEvaded`), `0x10000E00` (`Area_DiscoveredDID`), `0x100000C0` (`Quest_CompletionConditionArray`), `0x100007AC` (`Dungeon_CurrentDungeon`), `0x10000039` (`GCParam_Properties_List`), `0x00000753` (`WCGenChecksum_Array`), `0x000006C7` (`PlaceGen_PlaceName`) *(+2 more)*
- **Referenced WLib Strings (`0x0F`, 14 total)**: `"]."`, `"Was not able to get the area data id based on the players position["`, `"AreaSystem::IsPlayerAllowedToWorldJoin"`, `"] from the players position["`, `"AreaSystem::IsPlayerAllowedToWorldJoin> Was not able to get the area based on th"`, `"query"`, `""."`, `"Invalid subcommand ""`, `"Missing required Area definition name parameter."`, `".wc"` *(+4 more)*
- **Referenced Native Functions (`0x15`, 36 total)**: `0x00E7000A` (`0x140821770`), `0x00A9003F` (`0x1403a42c0`), `0x00990004` (`0x140389fa0`), `0x00A10007` (`0x14039c1c0`), `0x00A90012` (`0x140391a60`), `0x00270024` (`0x140392100`), `0x00A1000B` (`0x14039c2f0`), `0x00A90015` (`0x1403919a0`), `0x00A1000E` (`0x1403b36b0`), `0x00DD0001` (`0x1403b6150`), `0x01AA0007` (`0x140a7db60`), `0x00390001` (`0x1403b97d0`), `0x00B80004` (`0x1403abbd0`), `0x00A60015` (`0x1403a9640`) *(+22 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 2 total)**: `0x80FF000F` @ `0x140b81110` (fn `0x140b81060`), `0x80FF0011` @ `0x140b819b0` (fn `0x140b81900`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84E40001` | `1` | Unpack (S->C) | `0x84E40001` | `(ref)` | `0x00D60007(0x06C3)` | `0x80FF0004` (`+371820`) | NAT: `0x00390001` (`0x1403b97d0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x04E5` (`1253`) — AuctionSystem (Auction House Post, Bid, Buyout, Cancel, Search) <a id="rpc-class-0x04e5"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x00FA`
- **Method Counts**: `10` Pack (C->S), `9` Unpack (S->C) (`19` total)
- **Referenced Properties (`pids.txt`, 28 total)**: `0x10001392` (`World_DisableAuctionPosting`), `0x00000321` (`AnimConfigurableStruct`), `0x0000032B` (`PreprocOptions_KynGen_LBIList`), `0x0000045C` (`VoiceChat_MicMaxLevel`), `0x10001CB7` (`Antispam_AuctionRequest`), `0x100003FE` (`Area_RegionID`), `0x10000191` (`Skill_Toggle_MaxDistanceOverride`), `0x10000396` (`RefereeFilter_Score`), `0x10002984` (`Auction_OperationUnderway`), `0x10000490` (`Item_CraftToolCategory`), `0x1000074B` (`Auction_PostCount`), `0x100002E3` (`QuestDispenser_PortalDestination`), `0x1000073F` (`House_AssociatedKinshipID`), `0x1000013A` (`Vendor_SellAll_ItemToSell`), `0x10000312` (`UI_Element_IsActivatablePanel`), `0x1000035C` (`Effect_InstantVital_InitialChangeVariable`), `0x100000D9` (`Summon_SummonerPosition`), `0x100003E0` (`World_MPControl_Ettenmoors_Lumberyard`) *(+10 more)*
- **Referenced WLib Strings (`0x0F`, 42 total)**: `"openui"`, `"ui"`, `"mcopenui"`, `"closeui"`, `"cancelbyposter"`, `"mccancelbyposter"`, `"post"`, `"mcpost"`, `"bulkpost"`, `"mcbulkpost"` *(+32 more)*
- **Referenced Native Functions (`0x15`, 38 total)**: `0x00CC0003` (`0x1403c1d20`), `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`), `0x00B60002` (`0x1403c2eb0`), `0x00DD0001` (`0x1403b6150`), `0x00E10005` (`0x1403ca7d0`), `0x00DD0008` (`0x1403b5b70`), `0x00A9000E` (`0x14039aad0`), `0x00270013` (`0x140391550`), `0x01890018` (`0x140a70850`), `0x00DD0012` (`0x1403b3140`), `0x0189002C` (`0x140a6d8f0`), `0x01890019` (`0x140a729e0`) *(+24 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 22 total)**: `0x80FA0001` @ `0x140b4d47e` (fn `0x140b4d3ce`), `0x80FA0002` @ `0x140b4d9da` (fn `0x140b4d90f`), `0x80FA0005` @ `0x140b4eaae` (fn `0x140b4e9de`), `0x80FA0006` @ `0x140b5dd0a` (fn `0x140b5dbf4`), `0x80FA0007` @ `0x140b5de53` (fn `0x140b5ddbe`), `0x80FA0008` @ `0x140b5dfa3` (fn `0x140b5df0e`), `0x80FA0065` @ `0x140b5eac3` (fn `0x140b5ea2e`), `0x80FA0066` @ `0x140b5ec13` (fn `0x140b5eb7e`), `0x80FA00A8` @ `0x140b5b064` (fn `0x140b5affa`), `0x80FA0003` @ `0x140b4db70` (fn `0x140b4daa0`), `0x80FA0096` @ `0x140b52c1e` (fn `0x140b528f0`), `0x80FA0089` @ `0x140b53734` (fn `0x140b536ca`) *(+10 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84E5000B` | `2` | Unpack (S->C) | `0x84E5000B` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x80FA0059` (`+383260`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E5000F` | `3` | Unpack (S->C) | `0x84E5000F` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x80FA0057` (`+383188`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E50014` | `4` | Unpack (S->C) | `0x84E50014` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x80FA0054` (`+383292`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E5001E` | `10` | Unpack (S->C) | `0x84E5001E` | `(i32/u32, i32/u32, i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x80FA0055` (`+383220`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E50021` | `11` | Unpack (S->C) | `0x84E50021` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x80FA005A` (`+383132`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E50024` | `12` | Unpack (S->C) | `0x84E50024` | `(ref)` | `0x00D60007(0x06C3)` | `0x80FA0053` (`+383160`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E50027` | `14` | Unpack (S->C) | `0x84E50027` | `(ref)` | `0x00D60007(0x06C3)` | `0x80FA0056` (`+383324`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E5002C` | `15` | Unpack (S->C) | `0x84E5002C` | `()` | `0x00D60007(0x06C3)` | `0x80FA005B` (`+383108`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E5002E` | `16` | Pack (C->S) | `0x84E50017` | `(i64/u64, i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FA0005` (`+380992`) [C++: `0x140b4eaae`] | PID: `0x10000191` (`Skill_Toggle_MaxDistanceOverride`) |
| `0x84E5002F` | `17` | Pack (C->S) | `0x84E50003` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FA0007` (`+379332`) [C++: `0x140b5de53`] | PID: `0x10000191` (`Skill_Toggle_MaxDistanceOverride`) |
| `0x84E50030` | `18` | Pack (C->S) | `0x84E5001D` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FA0008` (`+379432`) [C++: `0x140b5dfa3`] | PID: `0x10000191` (`Skill_Toggle_MaxDistanceOverride`) |
| `0x84E50031` | `19` | Pack (C->S) | `0x84E50015` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FA0004` (`+378812`) | PID: `0x10001CB7` (`Antispam_AuctionRequest`), `0x100003FE` (`Area_RegionID`), `0x10000191` (`Skill_Toggle_MaxDistanceOverride`) |
| `0x84E50032` | `20` | Pack (C->S) | `0x84E5001A` | `(i64/u64, i32/u32, ref)` | `0x00D9000D` (`local_0`) | `0x80FA0002` (`+378528`) [C++: `0x140b4d9da`] | PID: `0x0000045C` (`VoiceChat_MicMaxLevel`); MSG: `"> ERROR: Given a null Auction Adaptor! Game S"`, `"AuctionSystem::BidOnAuction"` |
| `0x84E50033` | `21` | Pack (C->S) | `0x84E50018` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x80FA0001` (`+378084`) [C++: `0x140b4d47e`] | NAT: `0x01890018` (`0x140a70850`), `0x00DD0012` (`0x1403b3140`), `0x00DD0001` (`0x1403b6150`); PID: `0x00000321` (`AnimConfigurableStruct`), `0x0000032B` (`PreprocOptions_KynGen_LBIList`); MSG: `"> ERROR: Given a null Auction Adaptor! Game S"`, `"AuctionSystem::AddAuction"` |
| `0x84E50034` | `22` | Pack (C->S) | `0x84E50026` | `(i32/u32, i64/u64, i32/u32, i32/u32, ref)` | `0x00D9000D` (`local_0`) | `0x80FA0006` (`+379220`) [C++: `0x140b5dd0a`] | PID: `0x10000191` (`Skill_Toggle_MaxDistanceOverride`) |
| `0x84E50037` | `25` | Pack (C->S) | `0x84E50035` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FA0065` (`+379752`) [C++: `0x140b5eac3`] | — |
| `0x84E50038` | `26` | Pack (C->S) | `0x84E50036` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x80FA0066` (`+379812`) [C++: `0x140b5ec13`] | — |
| `0x84E50043` | `27` | Unpack (S->C) | `0x84E50043` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x80FA0088` (`+383352`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E5004A` | `29` | Pack (C->S) | `0x84E50049` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x80FA00A8` (`+380964`) [C++: `0x140b5b064`] | — |

### Class `0x04E8` (`1256`) — AuctionSystem / Dialog Confirmation Callback (0x06E0) <a id="rpc-class-0x04e8"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x06E0`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 3 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10000492` (`Auction_WinningFee_FortyEightHours`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 41 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+27 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84E80006` | `2` | Pack (C->S) | `0x84E80005` | `()` | `0x00D9000C` (`self`) | `0x86E00001` (`+1670764`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10000492` (`Auction_WinningFee_FortyEightHours`) |

### Class `0x04E9` (`1257`) — CCombatSystem (Auto-Attack, Target Selection, Combat Mode & Combat Events) <a id="rpc-class-0x04e9"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C1`, `0x06C3` | **Paired Client Implementation Class(es)**: `0x0100`
- **Method Counts**: `4` Pack (C->S), `5` Unpack (S->C) (`9` total)
- **Referenced Properties (`pids.txt`, 91 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x100001DE` (`Channeling_InterruptedByMovement`), `0x10000C6E` (`Combat_Violent`), `0x10003C3D` (`Combat_CurrentMoveToStamp`), `0x0000020B` (`PEDesc_ConstrainY`), `0x10001B9A` (`Antispam_TargetRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x100001B4` (`Inventory_SlotCache_Eq_Weapon_Primary`), `0x100001B3` (`Inventory_SlotCache_Eq_Weapon_Secondary`), `0x1000082A` (`Inventory_SlotCache_Eq_Weapon_Ranged`), `0x10000B87` (`Combat_DamageQualifier_Melee_Offense`), `0x10000C6B` (`Combat_DamageQualifier_Ranged_Offense`), `0x100064F0` (`LoE_Light_Modifier`), `0x10002169` (`Combat_Stats_EvadePoints`), `0x10002185` (`Combat_Class_EvadePoints`), `0x10002140` (`Combat_EvadePoints_Modifier`), `0x10002149` (`Combat_Current_EvadePoints`), `0x100021E8` (`Combat_PartialEvade_PercentageAddMod_Cached`) *(+73 more)*
- **Referenced WLib Strings (`0x0F`, 56 total)**: `"] from"`, `"> Couldn't get attack hook ["`, `"CCombatSystem::GetFXAndBehaviorInformation"`, `"dump"`, `"reset"`, `"usage: /combatmode <cmd>"`, `"<cmd> can be one of the following:"`, `"dump: Dumps data about combat mode on current selection."`, `"reset: Resets all Combat mode data on current selection."`, `"Could not get player."` *(+46 more)*
- **Referenced Native Functions (`0x15`, 115 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+101 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 20 total)**: `0x81000002` @ `0x140b5ed45` (fn `0x140b5ecc6`), `0x81000001` @ `0x140b524fe` (fn `0x140b52400`), `0x8100001A` @ `0x140b629ad` (fn `0x140b628f0`), `0x8100001C` @ `0x140b62acd` (fn `0x140b62a10`), `0x8100005A` @ `0x140b62bed` (fn `0x140b62b30`), `0x8100003E` @ `0x140b62d2f` (fn `0x140b62c50`), `0x8100003B` @ `0x140b62e6f` (fn `0x140b62d90`), `0x81000025` @ `0x140b62f8d` (fn `0x140b62ed0`), `0x8100001B` @ `0x140b6315d` (fn `0x140b630a0`), `0x81000026` @ `0x140b6327d` (fn `0x140b631c0`), `0x81000027` @ `0x140b6344d` (fn `0x140b63390`), `0x8100003F` @ `0x140b6356f` (fn `0x140b634b0`) *(+8 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84E90001` | `1` | Unpack (S->C) | `0x84E90001` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81000012` (`+470296`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E9001B` | `3` | Unpack (S->C) | `0x84E9001B` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8100000B` (`+470080`) | NAT: `0x00B80004` (`0x1403abbd0`), `0x00B8000B` (`0x1403af4f0`), `0x00B80007` (`0x1403a8b40`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E9002D` | `6` | Unpack (S->C) | `0x84E9002D` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86C30073` (`vslot 0x029A`, `+93600`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E90032` | `7` | Pack (C->S) | `0x84E90008` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81000002` (`+470188`) [C++: `0x140b5ed45`] | PID: `0x10001B9A` (`Antispam_TargetRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) |
| `0x84E90033` | `8` | Pack (C->S) | `0x84E9002B` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8100000D` (`+472108`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"dump"`, `"reset"` |
| `0x84E90034` | `9` | Pack (C->S) | `0x84E90020` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8100000D` (`+472108`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"dump"`, `"reset"` |
| `0x84E9003D` | `10` | Unpack (S->C) | `0x84E9003D` | `(ref)` | `0x00D60007(0x06C1)` | `0x81000018` (`+472604`) | NAT: `0x07EC0001` (`0x140b3ca60`) |
| `0x84E9004F` | `12` | Unpack (S->C) | `0x84E9004F` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x81000044` (`+470360`) | NAT: `0x07EC0007` (`0x140b3e330`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84E90050` | `13` | Pack (C->S) | `0x84E9004D` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x81000045` (`+475372`) | — |

### Class `0x04EB` (`1259`) — CombatStateBreakNotice / Dismount Notification System (0x08DA) <a id="rpc-class-0x04eb"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C1` | **Paired Client Implementation Class(es)**: `0x08DA`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x10002252` (`GOP_DisplayCombatStateBreakNotices`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 5 total)**: `0x00E70001` (`0x140821170`), `0x0030000B` (`0x140385ab0`), `0x00C70019` (`0x14039dba0`), `0x00C70009` (`0x14037c4c0`), `0x00230004` (`0x1403bc7d0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84EB0009` | `1` | Unpack (S->C) | `0x84EB0009` | `(i64/u64, i32/u32)` | `0x00D60007(0x06C1)` | `0x88DA0001` (`+485112`) | NAT: `0x00E70001` (`0x140821170`), `0x0030000B` (`0x140385ab0`), `0x00C70019` (`0x14039dba0`); PID: `0x10002252` (`GOP_DisplayCombatStateBreakNotices`) |

### Class `0x04EC` (`1260`) — CommunicationSystem / Chat & Gag System (Broadcast, Gag, Chat Channels) <a id="rpc-class-0x04ec"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3`, `0x06C5` | **Paired Client Implementation Class(es)**: `0x0102`
- **Method Counts**: `11` Pack (C->S), `11` Unpack (S->C) (`22` total)
- **Referenced Properties (`pids.txt`, 53 total)**: `0x1000195C` (`Admin_PlayerTestMode`), `0x10006616` (`Communication_AutoGagged`), `0x0000054A` (`Option_VoiceEnabled`), `0x10000068` (`UI_Pet_PetAggressiveButton`), `0x000001DC` (`Light_LODClass`), `0x1000004D` (`TextItem_Title`), `0x1000157F` (`Communication_Gagged`), `0x1000087A` (`GOP_FloatyInformation_Bitfield`), `0x00000080` (`BeamScript_UseTargetPosition`), `0x10000482` (`Usage_Locked`), `0x10001B6F` (`GOP_AcceptFriendsAndGuildMemberTellsOnly`), `0x10001CC0` (`Communication_LastSpamReportTime`), `0x10000DF0` (`Communication_CharacterPresence`), `0x00000064` (`LedgePhysicsController_LedgePlaneDistance`), `0x00000040` (`CameraShakeScript_OffsetY`), `0x1000036A` (`AI_KungFu_DramaTransition`), `0x1000305D` (`Annotation_Text`), `0x10003086` (`Annotation_IgnoreReason`) *(+35 more)*
- **Referenced WLib Strings (`0x0F`, 22 total)**: `"Usage: /broadcastglobal <message>"`, `"Broadcasts the message to all players logged into your server over the global ch"`, `"usage: /gag <name> <on \| off>"`, `"This command will prevent the specified player from communicating over any chat "`, `"usage: /gagtime <name> <Days> <Minutes> <Seconds>"`, `"This command will prevent the specified player from communicating over any chat "`, `"usage: /snoop <target name> <on \| off>"`, `"This command will place the user in all of the specified characters chat channel"`, `"Error: You must select a target!"`, `"Usage: /admindialog(message)"` *(+12 more)*
- **Referenced Native Functions (`0x15`, 128 total)**: `0x00A9002D` (`0x1403a8a20`), `0x01AA0002` (`0x140a7dae0`), `0x01310001` (`0x140b2ffb0`), `0x00B60029` (`0x1403c5260`), `0x00E10005` (`0x1403ca7d0`), `0x00DD0008` (`0x1403b5b70`), `0x00E10006` (`0x1403b8050`), `0x00E70001` (`0x140821170`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`), `0x00E70002` (`0x1408211d0`), `0x00A9003F` (`0x1403a42c0`), `0x00270001` (`0x14038a7e0`), `0x00270013` (`0x140391550`) *(+114 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 41 total)**: `0x8102001C` @ `0x140b57380` (fn `0x140b572f6`), `0x8102001D` @ `0x140b9cd80` (fn `0x140b9ccf6`), `0x8102001F` @ `0x140b5c5a2` (fn `0x140b5c54f`), `0x81020020` @ `0x140b5c692` (fn `0x140b5c63f`), `0x81020022` @ `0x140b5719b` (fn `0x140b5716f`), `0x81020071` @ `0x140b51535` (fn `0x140b51500`), `0x81020017` @ `0x140b4d5b0` (fn `0x140b4d526`), `0x81020058` @ `0x140b4e190` (fn `0x140b4e160`), `0x81020065` @ `0x140b507c8` (fn `0x140b50708`), `0x81020067` @ `0x140b50928` (fn `0x140b50868`), `0x8102004A` @ `0x140b56d2f` (fn `0x140b56d03`), `0x81020042` @ `0x140b56fb0` (fn `0x140b56f84`) *(+29 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84EC0005` | `1` | Unpack (S->C) | `0x84EC0005` | `(i32/u32, ref, i32/u32, ref, i64/u64)` | `0x00D60007(0x06C5)` | `0x81020026` (`+489808`) | PID: `0x000006C5` (`Tools_Map_FogOfWar`) |
| `0x84EC000A` | `2` | Unpack (S->C) | `0x84EC000A` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86C300C9` (`vslot 0x02DA`, `+93400`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC001A` | `4` | Unpack (S->C) | `0x84EC001A` | `(i32/u32, i64/u64, ref, ref)` | `0x00D60007(0x06C3)` | `0x81020036` (`+491980`) | NAT: `0x00C70010` (`0x14038bee0`), `0x0137000A` (`0x140b45280`), `0x01370009` (`0x140b45190`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC001C` | `6` | Unpack (S->C) | `0x84EC001C` | `(i32/u32, ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x81020024` (`+501492`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC001D` | `7` | Unpack (S->C) | `0x84EC001D` | `(ref)` | `0x00D60007(0x06C3)` | `0x81020023` (`+497016`) | NAT: `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760002` (`0x1403ba230`); PID: `0x1000004D` (`TextItem_Title`), `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC001E` | `8` | Unpack (S->C) | `0x84EC001E` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86C30044` (`vslot 0x0271`, `+93368`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC002C` | `11` | Unpack (S->C) | `0x84EC002C` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C5)` | `0x81020044` (`+490844`) | PID: `0x000006C5` (`Tools_Map_FogOfWar`) |
| `0x84EC002E` | `12` | Pack (C->S) | `0x84EC000E` | `(ref, ref)` | `0x00D9000C` (`self`) | `0x81020055` (`+489540`) | NAT: `0x00C70010` (`0x14038bee0`), `0x075C0001` (`0x140a6c4c0`), `0x00C70012` (`0x1403abd30`) |
| `0x84EC002F` | `13` | Pack (C->S) | `0x84EC0021` | `(ref)` | `0x00D9000C` (`self`) | `0x8102001C` (`+500452`) [C++: `0x140b57380`] | NAT: `0x00750007` (`0x140388ca0`), `0x00C70010` (`0x14038bee0`), `0x00E10015` (`0x1403b8e30`); PID: `0x0000012C` (`BeamScript_UntargetedBeamLength`), `0x100002C9` (`Landmark_QuestOnly`), `0x00000040` (`CameraShakeScript_OffsetY`) |
| `0x84EC0030` | `14` | Pack (C->S) | `0x84EC002A` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8102001F` (`+502504`) [C++: `0x140b5c5a2`], `0x81020020` (`+502632`) [C++: `0x140b5c692`] | NAT: `0x00C7000C` (`0x1403afdf0`); PID: `0x1000004D` (`TextItem_Title`) |
| `0x84EC0031` | `15` | Pack (C->S) | `0x84EC001B` | `(i32/u32, ref, i32/u32)` | `0x00D9000C` (`self`) | `0x81020047` (`+489236`) | NAT: `0x00C70010` (`0x14038bee0`), `0x00010020` (`0x140821650`); PID: `0x000001DC` (`Light_LODClass`), `0x1000004D` (`TextItem_Title`), `0x1000157F` (`Communication_Gagged`); MSG: `"> ERROR:  Players can't use narration!"`, `"CCommunicationSystem::Say"` |
| `0x84EC003B` | `16` | Unpack (S->C) | `0x84EC003B` | `(ref, i64/u64, i64/u64)` | `0x00D60007(0x06C5)` | `0x8102004E` (`+492124`) | PID: `0x000006C5` (`Tools_Map_FogOfWar`) |
| `0x84EC0040` | `18` | Unpack (S->C) | `0x84EC0040` | `(ref)` | `0x00D60007(0x06C3)` | `0x81020051` (`+503556`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC0044` | `20` | Unpack (S->C) | `0x84EC0044` | `(i32/u32, ref, ref)` | `0x00D60007(0x06C3)` | `0x81020052` (`+503520`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EC0045` | `21` | Pack (C->S) | `0x84EC0042` | `(ref, ref)` | `0x00D9000C` (`self`) | `0x81020050` (`+502732`) | PID: `0x10000463` (`Combat_CurrentCombo`), `0x1000047E` (`Usage_RequiredQuestStatus_Take`), `0x10000478` (`Quest_ObjectiveProgressOverride`) |
| `0x84EC0046` | `22` | Pack (C->S) | `0x84EC003F` | `(ref)` | `0x00D9000C` (`self`) | `0x81020053` (`+502928`) | PID: `0x10000478` (`Quest_ObjectiveProgressOverride`) |
| `0x84EC0049` | `23` | Unpack (S->C) | `0x84EC0049` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C5)` | `0x81020054` (`+490888`) | PID: `0x000006C5` (`Tools_Map_FogOfWar`) |
| `0x84EC004C` | `24` | Pack (C->S) | `0x84EC004D` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81020022` (`+490932`) [C++: `0x140b5719b`] | NAT: `0x01AA0002` (`0x140a7dae0`), `0x019F001D` (`0x140a7aaa0`), `0x00270013` (`0x140391550`); PID: `0x10000482` (`Usage_Locked`), `0x10001B6F` (`GOP_AcceptFriendsAndGuildMemberTellsOnly`) |
| `0x84EC004E` | `26` | Pack (C->S) | `0x84EC004F` | `(i64/u64, ref)` | `0x00D9000C` (`self`) | `0x8102001D` (`+501120`) [C++: `0x140b9cd80`] | NAT: `0x00270006` (`0x1403ae7b0`), `0x0001001B` (`0x140822cf0`); PID: `0x10003086` (`Annotation_IgnoreReason`) |
| `0x84EC005B` | `29` | Pack (C->S) | `0x84EC005A` | `()` | `0x00D9000C` (`self`) | `0x81020071` (`+487608`) [C++: `0x140b51535`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10006616` (`Communication_AutoGagged`) |
| `0x84EC005D` | `31` | Pack (C->S) | `0x84EC005C` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8102006D` (`+503584`) | NAT: `0x00A6002B` (`0x1403b0040`), `0x00A60017` (`0x1403ab3c0`), `0x00A6001F` (`0x1403a9c20`); PID: `0x10000400` (`RaceTable_MonsterPlayExaminationName`), `0x10001C91` (`Communication_SpammerName`), `0x10001C90` (`Communication_SpammerIID`) |
| `0x84EC0089` | `33` | Pack (C->S) | `0x84EC0088` | `()` | `0x00D9000C` (`self`) | `0x810200BE` (`+488444`) | — |

### Class `0x04ED` (`1261`) — ConjunctionSystem (Fellowship Manoeuvres & Conjunction Contributions) <a id="rpc-class-0x04ed"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C1`, `0x06C3` | **Paired Client Implementation Class(es)**: `0x0103`
- **Method Counts**: `1` Pack (C->S), `4` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x10000008` (`Skill_Pips_PipMinMod_Trait_ArrowStorm`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 21 total)**: `0x00E70001` (`0x140821170`), `0x0011000F` (`0x1403cb2b0`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x00190006` (`0x1403c2c90`), `0x0030000B` (`0x140385ab0`), `0x00190029` (`0x1403c5260`), `0x01380001` (`0x140b36470`), `0x01380002` (`0x140b36580`), `0x01380003` (`0x140b36630`), `0x00C70019` (`0x14039dba0`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x01920002` (`0x140a81b10`) *(+7 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 7 total)**: `0x81030003` @ `0x140b6bcba` (fn `0x140b6bb40`), `0x81030001` @ `0x140b6bde4` (fn `0x140b6bd60`), `0x81030002` @ `0x140b6bf3c` (fn `0x140b6be80`), `0x8103000E` @ `0x140b6c0b0` (fn `0x140b6bfed`), `0x81030005` @ `0x140b7d9f5` (fn `0x140b7d87b`), `0x8103000D` @ `0x140b7db87` (fn `0x140b7dad0`), `0x81030004` @ `0x140b89e80` (fn `0x140b89dd0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84ED0005` | `1` | Unpack (S->C) | `0x84ED0005` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x81030009` (`+509280`) | NAT: `0x01380003` (`0x140b36630`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84ED0006` | `2` | Unpack (S->C) | `0x84ED0006` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x81030007` (`+509220`) | NAT: `0x01380001` (`0x140b36470`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84ED000C` | `3` | Unpack (S->C) | `0x84ED000C` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C1)` | `0x8103000A` (`+509308`) | NAT: `0x00E70001` (`0x140821170`), `0x00C70019` (`0x14039dba0`), `0x00C7000C` (`0x1403afdf0`); PID: `0x10000008` (`Skill_Pips_PipMinMod_Trait_ArrowStorm`) |
| `0x84ED001C` | `4` | Unpack (S->C) | `0x84ED001C` | `(i64/u64, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81030006` (`+509248`) | NAT: `0x01380002` (`0x140b36580`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84ED0026` | `6` | Pack (C->S) | `0x84ED0023` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8103000C` (`+509884`) | — |

### Class `0x04EE` (`1262`) — CooldownSystem (Skill & Item Cooldown Timers & Reset) <a id="rpc-class-0x04ee"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0106`
- **Method Counts**: `1` Pack (C->S), `4` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 3 total)**: `0x00000200` (`Slider_Fly`), `0x000000C1` (`UICore_Element_notify_on_create`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 10 total)**: `"me"`, `"Unable to find 'me' as an admin!"`, `"AdminUtils::StringToInstanceID"`, `"0x"`, `"" is not an InstanceID!"`, `"Parameter ""`, `"..."`, `"Resetting all cooldown timers for"`, `"/cooldownreset <IID\|me>"`, `"Resets all cooldown timers on the given InstanceID, your selection or yourself i"`
- **Referenced Native Functions (`0x15`, 19 total)**: `0x00E10016` (`0x1403ba9d0`), `0x00E10003` (`0x1403c4630`), `0x00DD0001` (`0x1403b6150`), `0x00E10012` (`0x1403cb5e0`), `0x00AB0006` (`0x1403c4360`), `0x00E1001F` (`0x1403cbd20`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`), `0x00DD0012` (`0x1403b3140`), `0x00DD0008` (`0x1403b5b70`), `0x00E70001` (`0x140821170`), `0x00CC0002` (`0x1403c2050`), `0x01390002` (`0x140b46a30`) *(+5 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 2 total)**: `0x81060002` @ `0x140b7e044` (fn `0x140b7df30`), `0x81060001` @ `0x140b7e1d4` (fn `0x140b7e0c0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84EE0001` | `1` | Unpack (S->C) | `0x84EE0001` | `(ref)` | `0x00D60007(0x06C3)` | `0x81060008` (`+514684`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EE0003` | `3` | Unpack (S->C) | `0x84EE0003` | `()` | `0x00D60007(0x06C3)` | `0x81060003` (`+514660`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EE0008` | `4` | Unpack (S->C) | `0x84EE0008` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x81060005` (`+514600`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EE000A` | `5` | Unpack (S->C) | `0x84EE000A` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8106000A` (`+514632`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EE000C` | `6` | Pack (C->S) | `0x84EE0002` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x8106000C` (`+513252`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`); MSG: `"" is not an InstanceID!"`, `"Parameter ""` |

### Class `0x04EF` (`1263`) — CCraftSystem (Professions, Vocations, Recipes, Craft Guilds, Item Level) <a id="rpc-class-0x04ef"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0107`
- **Method Counts**: `7` Pack (C->S), `14` Unpack (S->C) (`21` total)
- **Referenced Properties (`pids.txt`, 56 total)**: `0x1000049A` (`Skill_InductionDuration_Trait_Fastpull_2`), `0x00000354` (`Name`), `0x000002FD` (`PluralName`), `0x10000441` (`Combat_DamageType_Additional_Base_Mitigation_Light`), `0x10000044` (`Craft_Prospector_ProficiencyLevel`), `0x10004D8A` (`CraftVocation_AdvancedCraftsmanTier`), `0x00000321` (`AnimConfigurableStruct`), `0x00000733` (`MovementCommands_EnumMapper`), `0x100064F5` (`Item_iLevel_Crafting_Offset`), `0x10000669` (`Item_Level`), `0x00000400` (`StringInfoRendering_VarType`), `0x000003FF` (`GlobalCamera_UnderWater`), `0x00000402` (`TerrainTypeDesc_UseGlossSpecular`), `0x10001481` (`Craft_Vocation`), `0x100067BF` (`Craft_CraftGuild_Membership_Array`), `0x10001729` (`World_DisabledCraftRecipe`), `0x1000172B` (`World_DisabledCraftRecipeArray`), `0x100001EA` (`ExaminationElement_Skill_Damage_Struct`) *(+38 more)*
- **Referenced WLib Strings (`0x0F`, 35 total)**: `"make"`, `"Not enough arguments."`, `"Unknown recipe."`, `"it was specified to be used in commmand line."`, `"found in recipe,"`, `"ERROR: No optional ingredient"`, `"has multiple tiers. For now no way to specify which one you mean, we'll pick one"`, `"Recipe"`, `"."`, `"Executing recipe:"` *(+25 more)*
- **Referenced Native Functions (`0x15`, 137 total)**: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`), `0x00B60014` (`0x1403c6650`), `0x00AB0004` (`0x1403c4250`), `0x00E1001D` (`0x1403cbc60`), `0x00110001` (`0x1403caaa0`), `0x00110006` (`0x1403c9bd0`), `0x0011000A` (`0x1403be350`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x00DD0003` (`0x1403b6270`), `0x00DD0001` (`0x1403b6150`) *(+123 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 26 total)**: `0x81070007` @ `0x140b9b302` (fn `0x140b9b280`), `0x81070008` @ `0x140b8dc08` (fn `0x140b8db60`), `0x8107001F` @ `0x140b9b0a2` (fn `0x140b9b020`), `0x8107004D` @ `0x140b9b232` (fn `0x140b9b1b0`), `0x8107004F` @ `0x140b9b9eb` (fn `0x140b9b9a0`), `0x81070035` @ `0x140b6402b` (fn `0x140b63fe0`), `0x81070004` @ `0x140b678fe` (fn `0x140b678cf`), `0x81070051` @ `0x140b703e0` (fn `0x140b703b0`), `0x8107004C` @ `0x140b739c2` (fn `0x140b73974`), `0x8107004B` @ `0x140b740f4` (fn `0x140b74070`), `0x8107004E` @ `0x140b74273` (fn `0x140b741e0`), `0x8107003C` @ `0x140b7cf11` (fn `0x140b7ce40`) *(+14 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84EF0001` | `1` | Unpack (S->C) | `0x84EF0001` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8107000A` (`+528076`) | NAT: `0x013A0003` (`0x140b33920`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF000A` | `4` | Unpack (S->C) | `0x84EF000A` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81070010` (`+527980`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF000B` | `5` | Unpack (S->C) | `0x84EF000B` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81070011` (`+528020`) | NAT: `0x013A0001` (`0x140b33850`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF000C` | `6` | Unpack (S->C) | `0x84EF000C` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81070016` (`+528048`) | NAT: `0x013A0002` (`0x140b33990`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF000D` | `7` | Unpack (S->C) | `0x84EF000D` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8107000C` (`+528336`) | NAT: `0x013A0006` (`0x140b3bcf0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0010` | `9` | Unpack (S->C) | `0x84EF0010` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81070012` (`+528000`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0011` | `10` | Unpack (S->C) | `0x84EF0011` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81070009` (`+528104`) | NAT: `0x013A0004` (`0x140b33a00`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0016` | `11` | Unpack (S->C) | `0x84EF0016` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81070014` (`+528300`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF001C` | `13` | Pack (C->S) | `0x84EF000E` | `(i32/u32, ref, i32/u32)` | `0x00D9000C` (`self`) | `0x8107000B` (`+522632`) | PID: `0x1000049A` (`Skill_InductionDuration_Trait_Fastpull_2`) |
| `0x84EF001D` | `14` | Pack (C->S) | `0x84EF0019` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81070007` (`+523932`) [C++: `0x140b9b302`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84EF001E` | `15` | Pack (C->S) | `0x84EF0008` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8107000D` (`+521488`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B6001E` (`0x1403c5c30`); MSG: `"make"` |
| `0x84EF001F` | `16` | Pack (C->S) | `0x84EF0006` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x81070008` (`+524652`) [C++: `0x140b8dc08`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84EF003A` | `17` | Unpack (S->C) | `0x84EF003A` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8107001C` (`+528392`) | NAT: `0x013A0009` (`0x140b3a710`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0040` | `19` | Pack (C->S) | `0x84EF003C` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8107001F` (`+524408`) [C++: `0x140b9b0a2`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84EF0047` | `20` | Unpack (S->C) | `0x84EF0047` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8107002B` (`+528280`) | NAT: `0x013A0010` (`0x140b337f0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0051` | `21` | Unpack (S->C) | `0x84EF0051` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81070041` (`+528216`) | NAT: `0x013A0015` (`0x140b338c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0052` | `22` | Unpack (S->C) | `0x84EF0052` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81070042` (`+528172`) | NAT: `0x013A0015` (`0x140b338c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0055` | `23` | Unpack (S->C) | `0x84EF0055` | `()` | `0x00D60007(0x06C3)` | `0x81070044` (`+528260`) | NAT: `0x013A0015` (`0x140b338c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF0057` | `24` | Unpack (S->C) | `0x84EF0057` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81070045` (`+528364`) | NAT: `0x013A0017` (`0x140b3b8f0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84EF005C` | `26` | Pack (C->S) | `0x84EF005B` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8107004D` (`+524784`) [C++: `0x140b9b232`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84EF005E` | `28` | Pack (C->S) | `0x84EF005D` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8107004F` (`+524896`) [C++: `0x140b9b9eb`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x04F1` (`1265`) — GameTime / Scene Waveform / Client Notification System (0x0109 / 0x013B0001) <a id="rpc-class-0x04f1"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0109`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 37 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+23 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F10001` | `1` | Unpack (S->C) | `0x84F10001` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x81090003` (`+549032`) | NAT: `0x013B0001` (`0x140b45850`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x04F2` (`1266`) — DeathSystem (LifeState, Revive, Retreat, Defeat/Smite, NeverSayDie) <a id="rpc-class-0x04f2"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C1`, `0x06C3` | **Paired Client Implementation Class(es)**: `0x0129`
- **Method Counts**: `5` Pack (C->S), `8` Unpack (S->C) (`13` total)
- **Referenced Properties (`pids.txt`, 12 total)**: `0x100005D6` (`LifeState`), `0x00000258` (`PEDesc_Inclusive`), `0x10006631` (`ZE_Boss_Engaged`), `0x10000054` (`Death_VitalPercentageData`), `0x10005E17` (`Death_SelfDefeat_AnnounceOverride`), `0x000000C8` (`UICore_Button_hot_click_interval`), `0x10002A85` (`Player_WasKilledBySmite`), `0x00000040` (`CameraShakeScript_OffsetY`), `0x10002CEF` (`Death_SelfReviveBlocked`), `0x00000080` (`BeamScript_UseTargetPosition`), `0x100000EE` (`GRSEvent_Quest_ChallengeLevel`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 14 total)**: `"Capping radius to 80."`, `"Error: Invalid Target."`, `"usage: /smite [<victim_iid>]"`, `"kills the victim dead in one fell swoop. If"`, `"no IID is given, it tries to kill your currently"`, `"selected object."`, `"usage: /revive [<victim_iid>]"`, `"revives the victim. If no IID is given, it tries"`, `"to revive your currently selected player."`, `"on"` *(+4 more)*
- **Referenced Native Functions (`0x15`, 23 total)**: `0x00A90025` (`0x140397860`), `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`), `0x00E10020` (`0x1403cba00`), `0x00DD0006` (`0x1403b6370`), `0x00B60006` (`0x1403c2c90`), `0x00DD0008` (`0x1403b5b70`), `0x00550003` (`0x1403b4dc0`), `0x00A9003F` (`0x1403a42c0`), `0x00C7000C` (`0x1403afdf0`), `0x00C70003` (`0x14037c2f0`), `0x0135000B` (`0x140b38550`), `0x0135004A` (`0x140b41700`) *(+9 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x81290001` @ `0x140b5e7e4` (fn `0x140b5e75f`), `0x81290002` @ `0x140b5e982` (fn `0x140b5e92f`), `0x81290003` @ `0x140b5f215` (fn `0x140b5f1ab`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F20003` | `2` | Unpack (S->C) | `0x84F20003` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8129000D` (`+551344`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70003` (`0x14037c2f0`), `0x0135000B` (`0x140b38550`); PID: `0x00000258` (`PEDesc_Inclusive`), `0x10006631` (`ZE_Boss_Engaged`), `0x10000054` (`Death_VitalPercentageData`) |
| `0x84F20004` | `3` | Unpack (S->C) | `0x84F20004` | `(ref)` | `0x00D60007(0x06C3)` | `0x81290007` (`+552832`) | NAT: `0x0135000C` (`0x140b38870`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F2000A` | `5` | Unpack (S->C) | `0x84F2000A` | `()` | `0x00D60007(0x06C3)` | `0x81290004` (`+552600`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000054` (`Death_VitalPercentageData`), `0x000006C3` (`Appearance_ClassList`) |
| `0x84F20010` | `8` | Unpack (S->C) | `0x84F20010` | `()` | `0x00D60007(0x06C3)` | `0x8129000A` (`+552904`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70003` (`0x14037c2f0`), `0x0135000D` (`0x140b38730`); PID: `0x10000054` (`Death_VitalPercentageData`), `0x000000C8` (`UICore_Button_hot_click_interval`), `0x000006C3` (`Appearance_ClassList`) |
| `0x84F20012` | `9` | Unpack (S->C) | `0x84F20012` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C1)` | `0x81290009` (`+551580`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10005E17` (`Death_SelfDefeat_AnnounceOverride`), `0x10000054` (`Death_VitalPercentageData`) |
| `0x84F20013` | `10` | Unpack (S->C) | `0x84F20013` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8129000C` (`+552016`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000054` (`Death_VitalPercentageData`), `0x000006C3` (`Appearance_ClassList`) |
| `0x84F2001C` | `11` | Unpack (S->C) | `0x84F2001C` | `()` | `0x00D60007(0x06C3)` | `0x8129000E` (`+552424`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000054` (`Death_VitalPercentageData`), `0x000006C3` (`Appearance_ClassList`) |
| `0x84F2002D` | `14` | Pack (C->S) | `0x84F20001` | `()` | `0x00D9000C` (`self`) | `0x81290003` (`+552856`) [C++: `0x140b5f215`] | — |
| `0x84F2002E` | `15` | Pack (C->S) | `0x84F20026` | `()` | `0x00D9000C` (`self`) | `0x81290003` (`+552856`) [C++: `0x140b5f215`] | — |
| `0x84F2002F` | `16` | Pack (C->S) | `0x84F2000B` | `(f32)` | `0x00D9000C` (`self`) | `0x86C2001C` (`+47908`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"Capping radius to 80."` |
| `0x84F20030` | `17` | Pack (C->S) | `0x84F20008` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x81290001` (`+552776`) [C++: `0x140b5e7e4`] | — |
| `0x84F20032` | `19` | Pack (C->S) | `0x84F2001F` | `()` | `0x00D9000C` (`self`) | `0x81290002` (`+552808`) [C++: `0x140b5e982`] | — |
| `0x84F2004C` | `20` | Unpack (S->C) | `0x84F2004C` | `(f64)` | `0x00D60007(0x06C3)` | `0x81290022` (`+551556`) | NAT: `0x0135004A` (`0x140b41700`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x04F3` (`1267`) — DiscoverySystem (Resource Tracking & Monster Tracking) <a id="rpc-class-0x04f3"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x010A`
- **Method Counts**: `2` Pack (C->S), `1` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 3 total)**: `0x10000F36` (`Discovery_Tracking_Resource`), `0x100003EF` (`Craft_Resource_Type`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 17 total)**: `0x00E70002` (`0x1408211d0`), `0x00A9003F` (`0x1403a42c0`), `0x0030000B` (`0x140385ab0`), `0x00990004` (`0x140389fa0`), `0x00E70001` (`0x140821170`), `0x013C0001` (`0x140b340e0`), `0x00D30007` (`0x1403a6da0`), `0x00290001` (`0x1403a8bc0`), `0x00290002` (`0x1403ac4a0`), `0x00A80001` (`0x1403a7040`), `0x00430001` (`0x1403a6ee0`), `0x007F0001` (`0x1403aa070`), `0x007F0002` (`0x1403abe60`), `0x007F0005` (`0x1403a7f60`) *(+3 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x810A0001` @ `0x140b65955` (fn `0x140b65920`), `0x810A0002` @ `0x140b60d61` (fn `0x140b60d00`), `0x810A000A` @ `0x140b7f14a` (fn `0x140b7f090`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F30003` | `2` | Unpack (S->C) | `0x84F30003` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x810A0004` (`+554296`) | NAT: `0x013C0001` (`0x140b340e0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F30009` | `4` | Pack (C->S) | `0x84F30008` | `()` | `0x00D9000C` (`self`) | `0x810A0001` (`+554260`) [C++: `0x140b65955`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x84F3000A` | `5` | Pack (C->S) | `0x84F30001` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x810A0002` (`+554220`) [C++: `0x140b60d61`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x04F6` (`1270`) — DramaSystem (AI Drama Sequence Performance & Dump) <a id="rpc-class-0x04f6"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x06CE`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x1000076E` (`Drama_Performing`), `0x1000388F` (`Drama_CanInteractDuringDrama`)
- **Referenced WLib Strings (`0x0F`, 1 total)**: `"usage:  /drama perform <drama name>\n        /drama dump"`
- **Referenced Native Functions (`0x15`, 1 total)**: `0x00E10005` (`0x1403ca7d0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F60004` | `2` | Pack (C->S) | `0x84F60001` | `(i64/u64, ref)` | `0x00D9000C` (`self`) | `0x86CE0001` (`+557008`) | — |

### Class `0x04F7` (`1271`) — DuelingSystem (Sparring Invite, Accept, Decline & Result) <a id="rpc-class-0x04f7"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x06D8`, `0x06E3`
- **Method Counts**: `3` Pack (C->S), `2` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 17 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x10000050` (`Emote.Sit@Player.i_bChairFound`), `0x100000BC` (`QuestTreasure_SelectedItem`), `0x10000035` (`QuestDispenser_RoleSuccessText`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x00000693` (`UICore_Dialog_confirmation_nobuttontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x10000BA6` (`Dueling_UI_Invite_InstanceID`), `0x00000644` (`UICore_Dialog_topmost`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x10000F89` (`Dueling_State`), `0x10000749` (`Dueling_OpponentID`), `0x10001154` (`Dueling_Invite_TimeStamp`), `0x000005A5` (`UICore_Dialog_confirmation_response`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 59 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+45 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F70010` | `3` | Unpack (S->C) | `0x84F70010` | `(i64/u64, ref)` | `0x00D60007(0x06C3)` | `0x86D80001` (`+1274944`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00C70003` (`0x14037c2f0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000050` (`Emote.Sit@Player.i_bChairFound`), `0x10000035` (`QuestDispenser_RoleSuccessText`) |
| `0x84F70020` | `5` | Pack (C->S) | `0x84F70007` | `()` | `0x00D9000C` (`self`) | `0x86D80003` (`+1274900`) | — |
| `0x84F70021` | `6` | Pack (C->S) | `0x84F7000D` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x86E30001` (`+1672056`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00C7000C` (`0x1403afdf0`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10000050` (`Emote.Sit@Player.i_bChairFound`), `0x10000BA6` (`Dueling_UI_Invite_InstanceID`) |
| `0x84F70022` | `7` | Pack (C->S) | `0x84F7001B` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x86D80002` (`+1274660`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00CC0002` (`0x1403c2050`); PID: `0x10000050` (`Emote.Sit@Player.i_bChairFound`), `0x100000BC` (`QuestTreasure_SelectedItem`) |
| `0x84F70023` | `8` | Unpack (S->C) | `0x84F70023` | `()` | `0x00D60007(0x06C3)` | `0x86D80004` (`+1275372`) | NAT: `0x0099001B` (`0x1403b29b0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x04F8` (`1272`) — CEffectSystem / CEffectListenerSystem (Effect Add/Remove & Listener Sync) <a id="rpc-class-0x04f8"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x010B`, `0x06D0`
- **Method Counts**: `0` Pack (C->S), `8` Unpack (S->C) (`8` total)
- **Referenced Properties (`pids.txt`, 6 total)**: `0x100000E9` (`GRSEvent_Mail_MailID`), `0x000000B8` (`Physics_NoShadowSolids`), `0x0000010A` (`WorldBuilderSounds`), `0x00000162` (`FlushRCP`), `0x000002BA` (`AnimationScript_ManualUpdate`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 21 total)**: `"usage: /efflistener <dump> [InstanceID]"`, `", but not found in the cache!"`, `"Told to remove"`, `"CEffectListenerSystem::RecvCEvt_Effect_CRemoveListener"`, `"in the registry!"`, `", it no longer appears"`, `"ERROR: Unable to handle AfterSuppression on old top-level Effect"`, `"CEffectSystem::AddEffect"`, `"was not in effect registry for this entity."`, `"> ERROR: Effect"` *(+11 more)*
- **Referenced Native Functions (`0x15`, 54 total)**: `0x00750003` (`0x1403bb3c0`), `0x00DD0001` (`0x1403b6150`), `0x01670010` (`0x140b34cc0`), `0x00750004` (`0x1403bd030`), `0x0023000C` (`0x1403b8940`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`), `0x00230001` (`0x1403b6580`), `0x00B70001` (`0x1403c4ed0`), `0x00750001` (`0x1403b6ce0`), `0x00230003` (`0x1403baec0`), `0x00E70001` (`0x140821170`), `0x0023000A` (`0x1403bf2a0`) *(+40 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 6 total)**: `0x810B0002` @ `0x140b580b1` (fn `0x140b57fde`), `0x810B000D` @ `0x140b5825a` (fn `0x140b58181`), `0x810B0001` @ `0x140b5a43a` (fn `0x140b5a373`), `0x810B000F` @ `0x140b671e1` (fn `0x140b67170`), `0x810B0012` @ `0x140b7bdcd` (fn `0x140b7bd00`), `0x810B0021` @ `0x140b7f257` (fn `0x140b7f1a0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F80003` | `1` | Unpack (S->C) | `0x84F80003` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C300F3` (`vslot 0x02FE`, `+93688`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F8000F` | `2` | Unpack (S->C) | `0x84F8000F` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C3004B` (`vslot 0x0278`, `+93628`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F80019` | `3` | Unpack (S->C) | `0x84F80019` | `(i32/u32, ref, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x86D00002` (`+558012`) | NAT: `0x00230003` (`0x1403baec0`), `0x00230001` (`0x1403b6580`), `0x01670010` (`0x140b34cc0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F80020` | `4` | Unpack (S->C) | `0x84F80020` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x810B0007` (`+561068`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F80023` | `5` | Unpack (S->C) | `0x84F80023` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x86D00003` (`+557668`) | NAT: `0x00750003` (`0x1403bb3c0`), `0x00DD0001` (`0x1403b6150`), `0x01670010` (`0x140b34cc0`); PID: `0x000006C3` (`Appearance_ClassList`); MSG: `", but not found in the cache!"`, `"Told to remove"` |
| `0x84F80025` | `6` | Unpack (S->C) | `0x84F80025` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86D00001` (`+557804`) | NAT: `0x00750004` (`0x1403bd030`), `0x0023000C` (`0x1403b8940`), `0x00B6002B` (`0x1403c0470`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F80031` | `7` | Unpack (S->C) | `0x84F80031` | `(ref)` | `0x00D60007(0x06C3)` | `0x86C3000E` (`vslot 0x024B`, `+93660`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84F80084` | `8` | Unpack (S->C) | `0x84F80084` | `(ref)` | `0x00D60007(0x06C3)` | `0x86D00008` (`+558144`) | NAT: `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x04F9` (`1273`) — EmoteSystem (Granted Emotes, Emote Commands & Execution) <a id="rpc-class-0x04f9"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C5` | **Paired Client Implementation Class(es)**: `0x012A`
- **Method Counts**: `4` Pack (C->S), `4` Unpack (S->C) (`8` total)
- **Referenced Properties (`pids.txt`, 37 total)**: `0x00000419` (`MoveToController_LeftSpeed`), `0x0000006F` (`UICore_Text_font_struct`), `0x00000079` (`PEDesc_NumVFrames`), `0x00000087` (`SwitchScriptPort_ToIsDefault`), `0x000000C7` (`UICore_Text_fit_to_text`), `0x000000D6` (`BeamScript_TailAnimStartTime`), `0x000000E1` (`DetectScript_OtherObjectID`), `0x10000001` (`Pedigree_Surname_LevelRestriction`), `0x000000EF` (`BlockY`), `0x1000004D` (`TextItem_Title`), `0x00000132` (`AfterEffectScript_OverlayOpacity`), `0x0000013C` (`CombinationTarget_Operator`), `0x0000014A` (`ClearanceBBoxMin1`), `0x10000021` (`Examination_Effect_Duration`), `0x000002E2` (`Camera_InMouseLookMode`), `0x1000049F` (`Trait_Captain_ImprovedBannerofHope_AppearanceA`), `0x000002F1` (`UICore_Scrollbar_stop_locations`), `0x00000318` (`Script_DrivenPropertyEntry`) *(+19 more)*
- **Referenced WLib Strings (`0x0F`, 39 total)**: `"Emote Commands:"`, `"listgrantedemotes, grantemote, revokeemote, revokegrantedemotes."`, `"-- listgrantedemotes [<IID>] - List the granted emotes for selected or given pla"`, `"-- grantemote <emote.wc> [<IID>] - Grants the specified emote for selected or gi"`, `"-- revokeemote <emote.wc> [<IID>] - Revokes the specified emote for selected or "`, `"-- revokegrantedemotes [<IID>] - Revokes all granted emotes for selected or give"`, `"ERROR: Failed to retrieve list of known Emotes from the EmoteDirectory!"`, `"EmoteSystem::InqValidEmotes"`, `"ERROR: Failed to retrieve list of granted Emotes from the Player!"`, `"!"` *(+29 more)*
- **Referenced Native Functions (`0x15`, 40 total)**: `0x00E10006` (`0x1403b8050`), `0x00120009` (`0x1403be3e0`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00DD000F` (`0x1403b3090`), `0x00DD0001` (`0x1403b6150`), `0x00120003` (`0x1403b8fa0`), `0x00120001` (`0x1403b6790`), `0x001A0001` (`0x1403c4ed0`), `0x0012000F` (`0x1403c5400`), `0x00C70019` (`0x14039dba0`), `0x00C70010` (`0x14038bee0`), `0x0008000A` (`0x140820520`) *(+26 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 5 total)**: `0x812A0005` @ `0x140b7ad74` (fn `0x140b7ad05`), `0x812A0004` @ `0x140b4dee2` (fn `0x140b4ddfe`), `0x812A0003` @ `0x140b4f7a8` (fn `0x140b4f6e8`), `0x812A0002` @ `0x140b5844c` (fn `0x140b58344`), `0x812A0001` @ `0x140b5a709` (fn `0x140b5a6de`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84F90002` | `1` | Unpack (S->C) | `0x84F90002` | `(ref, ref, i64/u64, ref, i32/u32)` | `0x00D60007(0x06C5)` | `0x812A0017` (`+579108`) | NAT: `0x00DD000F` (`0x1403b3090`), `0x00DD0001` (`0x1403b6150`), `0x00E70001` (`0x140821170`); PID: `0x0000068D` (`DecalVolume_RenderPhase`), `0x000006C5` (`Tools_Map_FogOfWar`); MSG: `") from the Server, this shouldn't happen..."`, `"ERROR: Received an invalid Emote definition D"` |
| `0x84F90003` | `2` | Unpack (S->C) | `0x84F90003` | `(ref, ref, i64/u64, i32/u32)` | `0x00D60007(0x06C5)` | `0x812A0018` (`+578788`) | NAT: `0x00DD000F` (`0x1403b3090`), `0x00DD0001` (`0x1403b6150`), `0x00E70001` (`0x140821170`); PID: `0x00000650` (`UICore_Text_font_color_struct`), `0x000006C5` (`Tools_Map_FogOfWar`); MSG: `") from the Server, this shouldn't happen..."`, `"ERROR: Received an invalid Emote definition D"` |
| `0x84F90006` | `4` | Unpack (S->C) | `0x84F90006` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C5)` | `0x812A0016` (`+578588`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000021` (`Examination_Effect_Duration`), `0x000006C5` (`Tools_Map_FogOfWar`) |
| `0x84F9000B` | `8` | Pack (C->S) | `0x84F9000A` | `(ref)` | `0x00D9000C` (`self`) | `0x812A0012` (`+577464`) | NAT: `0x00E10002` (`0x1403ba7e0`), `0x00010020` (`0x140821650`), `0x00E10003` (`0x1403c4630`); PID: `0x1000157F` (`Communication_Gagged`), `0x10006616` (`Communication_AutoGagged`), `0x1000378A` (`ForwardSource_Emotes_Disabled`) |
| `0x84F9000C` | `9` | Pack (C->S) | `0x84F90008` | `(i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x812A0024` (`+577924`) | NAT: `0x00E10002` (`0x1403ba7e0`), `0x0030000B` (`0x140385ab0`); PID: `0x1000157F` (`Communication_Gagged`), `0x10006616` (`Communication_AutoGagged`), `0x1000378A` (`ForwardSource_Emotes_Disabled`) |
| `0x84F9000D` | `10` | Pack (C->S) | `0x84F90007` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x812A0005` (`+575572`) [C++: `0x140b7ad74`] | PID: `0x1000004D` (`TextItem_Title`) |
| `0x84F9000E` | `11` | Pack (C->S) | `0x84F90005` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x812A0024` (`+577924`) | NAT: `0x00E10002` (`0x1403ba7e0`), `0x0030000B` (`0x140385ab0`); PID: `0x1000157F` (`Communication_Gagged`), `0x10006616` (`Communication_AutoGagged`), `0x1000378A` (`ForwardSource_Emotes_Disabled`) |
| `0x84F90010` | `12` | Unpack (S->C) | `0x84F90010` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C5)` | `0x812A001B` (`+578396`) | NAT: `0x00E70001` (`0x140821170`), `0x00DD000F` (`0x1403b3090`), `0x00DD0001` (`0x1403b6150`); PID: `0x00000603` (`UICore_Resizebar_border_top`), `0x000006C5` (`Tools_Map_FogOfWar`); MSG: `") from the Server, this shouldn't happen..."`, `"ERROR: Received an invalid Emote definition D"` |

### Class `0x04FC` (`1276`) — CErrorSystem (Server->Client Error Code & String Table Notification) <a id="rpc-class-0x04fc"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x010C`
- **Method Counts**: `0` Pack (C->S), `2` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 3 total)**: `0x000006D1` (`CursorPlacementMode`), `0x0000005B` (`AvatarEntity`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 14 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x013D0002` (`0x140b32e40`), `0x00DD000F` (`0x1403b3090`), `0x00C70002` (`0x14037c380`), `0x0001001E` (`0x140820ee0`), `0x00C70003` (`0x14037c2f0`), `0x012B0002` (`0x140a85fc0`), `0x012B0004` (`0x140b4a5e0`), `0x012B0006` (`0x140b4b940`), `0x012B0008` (`0x140a8dc00`), `0x012B000A` (`0x140a848b0`), `0x00E70001` (`0x140821170`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x810C0001` @ `0x140b79980` (fn `0x140b79930`), `0x810C0002` @ `0x140b79a4c` (fn `0x140b799b0`), `0x810C000F` @ `0x140b7f37d` (fn `0x140b7f2b0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84FC0003` | `1` | Unpack (S->C) | `0x84FC0003` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x810C0005` (`+583876`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FC0005` | `2` | Unpack (S->C) | `0x84FC0005` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x810C0003` (`+583848`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x04FD` (`1277`) — CExaminationSystem (Tooltip Inspection Cache & Characteristic Structs) <a id="rpc-class-0x04fd"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x010D`
- **Method Counts**: `4` Pack (C->S), `5` Unpack (S->C) (`9` total)
- **Referenced Properties (`pids.txt`, 15 total)**: `0x10000178` (`Effect_VitalTransfer_TargetVitalType`), `0x10000004` (`RandomBox@Entity.m_bCanFireOutput_6`), `0x10000258` (`Effect_ApplyOverTime_Applied_Effect_Array`), `0x1000018F` (`Aggro@NPC::CurrentTarget`), `0x10000255` (`Effect_TargetFilterPermissions`), `0x0000041D` (`Slider_Skid`), `0x100010EF` (`UI_Examination_Tooltip_CharacteristicStruct`), `0x10000421` (`UI_Examination_Tooltip_DID`), `0x10002897` (`UI_Examination_Tooltip_AlternateSourceIID`), `0x100012C5` (`UI_Examination_Tooltip_ArrayHelper`), `0x10000AC1` (`Inventory_BoundToID`), `0x10000ACD` (`Item_ClothingColor`), `0x0000034E` (`Container_Slot`), `0x0000012B` (`RigidBodyScript_AngularVelocity`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 4 total)**: `"entries."`, `"CExaminationSystem::PurgeCacheOfOldData removed"`, `"null i_rcPropColl passed in for item"`, `"ExaminationSystem::GetExaminationCharacteristicStructFromDIDAndPropertyCollectio"`
- **Referenced Native Functions (`0x15`, 56 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00E70001` (`0x140821170`), `0x01B00008` (`0x140a7f8c0`), `0x01B00004` (`0x140a7ebb0`), `0x01B00006` (`0x140386b70`), `0x0030000B` (`0x140385ab0`), `0x01B00005` (`0x140a7e1b0`), `0x01B00007` (`0x1403891f0`), `0x01B0000A` (`0x140a7de40`), `0x013E0003` (`0x140b46b90`), `0x00750004` (`0x1403bd030`), `0x00750001` (`0x1403b6ce0`) *(+42 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x810D0001` @ `0x140b52261` (fn `0x140b520b0`), `0x810D0002` @ `0x140b5e8bc` (fn `0x140b5e860`), `0x810D0020` @ `0x140b4d8a1` (fn `0x140b4d81e`), `0x810D0024` @ `0x140b585e3` (fn `0x140b584f0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84FD0006` | `1` | Unpack (S->C) | `0x84FD0006` | `(i32/u32, ref, ref)` | `0x00D60007(0x06C3)` | `0x810D000B` (`+585688`) | NAT: `0x013E0003` (`0x140b46b90`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FD0011` | `4` | Unpack (S->C) | `0x84FD0011` | `(ref, ref, ref, i32/u32, i32/u32, i32/u32, ref, ref, ref, ref, ref, ref, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x810D001B` (`+587612`) | NAT: `0x013E0007` (`0x140b401c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FD0016` | `5` | Unpack (S->C) | `0x84FD0016` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x810D000A` (`+587576`) | NAT: `0x013E0006` (`0x140b40d10`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FD0017` | `6` | Unpack (S->C) | `0x84FD0017` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x810D0017` (`+587340`) | NAT: `0x01670014` (`0x140b40110`); PID: `0x10000255` (`Effect_TargetFilterPermissions`), `0x000006C3` (`Appearance_ClassList`) |
| `0x84FD001F` | `8` | Pack (C->S) | `0x84FD000A` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x810D0002` (`+587280`) [C++: `0x140b5e8bc`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000018F` (`Aggro@NPC::CurrentTarget`) |
| `0x84FD0020` | `9` | Pack (C->S) | `0x84FD0010` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x810D0020` (`+587404`) [C++: `0x140b4d8a1`] | NAT: `0x00E70001` (`0x140821170`), `0x013E0005` (`0x140b31cd0`) |
| `0x84FD0021` | `10` | Pack (C->S) | `0x84FD001B` | `(i32/u32, ref, i32/u32, ref)` | `0x00D9000C` (`self`) | `0x810D0001` (`+585032`) [C++: `0x140b52261`] | NAT: `0x00E70001` (`0x140821170`), `0x01B00008` (`0x140a7f8c0`), `0x01B00004` (`0x140a7ebb0`); PID: `0x10000178` (`Effect_VitalTransfer_TargetVitalType`), `0x10000004` (`RandomBox@Entity.m_bCanFireOutput_6`), `0x10000258` (`Effect_ApplyOverTime_Applied_Effect_Array`) |
| `0x84FD0028` | `12` | Unpack (S->C) | `0x84FD0028` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x810D001D` (`+587524`) | NAT: `0x013E0009` (`0x140b40aa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FD0029` | `13` | Pack (C->S) | `0x84FD0025` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x810D0020` (`+587404`) [C++: `0x140b4d8a1`], `0x810D001F` (`+587484`) | NAT: `0x00E70001` (`0x140821170`), `0x013E0005` (`0x140b31cd0`) |

### Class `0x04FE` (`1278`) — Social / Friends / Ignore / Who / Fellowship Lookup System (0x019D / 0x0452) <a id="rpc-class-0x04fe"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0102`, `0x019D`, `0x0452`, `0x074D`
- **Method Counts**: `20` Pack (C->S), `39` Unpack (S->C) (`59` total)
- **Referenced Properties (`pids.txt`, 99 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x10006616` (`Communication_AutoGagged`), `0x0000054A` (`Option_VoiceEnabled`), `0x10000068` (`UI_Pet_PetAggressiveButton`), `0x000001DC` (`Light_LODClass`), `0x1000004D` (`TextItem_Title`), `0x1000157F` (`Communication_Gagged`), `0x1000087A` (`GOP_FloatyInformation_Bitfield`), `0x00000080` (`BeamScript_UseTargetPosition`), `0x10000482` (`Usage_Locked`), `0x10001B6F` (`GOP_AcceptFriendsAndGuildMemberTellsOnly`), `0x10001CC0` (`Communication_LastSpamReportTime`), `0x10000DF0` (`Communication_CharacterPresence`), `0x00000064` (`LedgePhysicsController_LedgePlaneDistance`), `0x00000040` (`CameraShakeScript_OffsetY`), `0x1000036A` (`AI_KungFu_DramaTransition`), `0x1000305D` (`Annotation_Text`), `0x10003086` (`Annotation_IgnoreReason`) *(+81 more)*
- **Referenced WLib Strings (`0x0F`, 28 total)**: `"Usage: /broadcastglobal <message>"`, `"Broadcasts the message to all players logged into your server over the global ch"`, `"usage: /gag <name> <on \| off>"`, `"This command will prevent the specified player from communicating over any chat "`, `"usage: /gagtime <name> <Days> <Minutes> <Seconds>"`, `"This command will prevent the specified player from communicating over any chat "`, `"usage: /snoop <target name> <on \| off>"`, `"This command will place the user in all of the specified characters chat channel"`, `"Error: You must select a target!"`, `"Usage: /admindialog(message)"` *(+18 more)*
- **Referenced Native Functions (`0x15`, 212 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+198 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 72 total)**: `0x81020018` @ `0x140b5e555` (fn `0x140b5e4d6`), `0x819D0004` @ `0x140a93042` (fn `0x140a92fef`), `0x819D0005` @ `0x140a93165` (fn `0x140a930e6`), `0x819D0006` @ `0x140a93290` (fn `0x140a93206`), `0x819D0007` @ `0x140a92f55` (fn `0x140a92ed6`), `0x819D0008` @ `0x140a93505` (fn `0x140a93486`), `0x819D003F` @ `0x140a9c660` (fn `0x140a9c610`), `0x819D0040` @ `0x140a9cad0` (fn `0x140a9ca80`), `0x819D0051` @ `0x140a933de` (fn `0x140a9333e`), `0x819D007F` @ `0x140a9c745` (fn `0x140a9c6c6`), `0x819D0080` @ `0x140aa0715` (fn `0x140aa0696`), `0x81020017` @ `0x140b4d5b0` (fn `0x140b4d526`) *(+60 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84FE0005` | `1` | Unpack (S->C) | `0x84FE0005` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520055` (`+719012`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE001E` | `3` | Unpack (S->C) | `0x84FE001E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x84520053` (`+718956`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0020` | `4` | Unpack (S->C) | `0x84FE0020` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452004E` (`+719408`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0022` | `5` | Unpack (S->C) | `0x84FE0022` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452004A` (`+718924`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE002E` | `7` | Unpack (S->C) | `0x84FE002E` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520043` (`+719144`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0032` | `8` | Unpack (S->C) | `0x84FE0032` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x84520047` (`+719440`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0034` | `9` | Unpack (S->C) | `0x84FE0034` | `()` | `0x00D60007(0x06C3)` | `0x84520054` (`+718900`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0039` | `10` | Unpack (S->C) | `0x84FE0039` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x84520049` (`+718872`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0044` | `12` | Unpack (S->C) | `0x84FE0044` | `(ref)` | `0x00D60007(0x06C3)` | `0x84520052` (`+718816`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE004F` | `14` | Unpack (S->C) | `0x84FE004F` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x84520045` (`+718984`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0063` | `16` | Unpack (S->C) | `0x84FE0063` | `(i32/u32, ref, i32/u32, i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520051` (`+719308`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0073` | `18` | Unpack (S->C) | `0x84FE0073` | `()` | `0x00D60007(0x06C3)` | `0x84520046` (`+719384`) | NAT: `0x013F000B` (`0x140b3ac20`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE007E` | `20` | Unpack (S->C) | `0x84FE007E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x84520044` (`+719356`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0098` | `24` | Unpack (S->C) | `0x84FE0098` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452004D` (`+719044`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE009C` | `25` | Unpack (S->C) | `0x84FE009C` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520050` (`+719508`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE009D` | `26` | Unpack (S->C) | `0x84FE009D` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452004C` (`+719176`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00A0` | `27` | Unpack (S->C) | `0x84FE00A0` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8452004B` (`+718844`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00A1` | `28` | Unpack (S->C) | `0x84FE00A1` | `(i32/u32, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452004F` (`+719472`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00A3` | `29` | Pack (C->S) | `0x84FE002B` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x819D0012` (`+701100`) | NAT: `0x00E70001` (`0x140821170`), `0x00E70002` (`0x1408211d0`); PID: `0x10000085` (`ErrorCategory_Script`) |
| `0x84FE00A5` | `30` | Pack (C->S) | `0x84FE0049` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D0007` (`+699872`) [C++: `0x140a92f55`] | PID: `0x10000451` (`NPC_ActionPicker_MPMissionControllerString`), `0x1000028B` (`QuestEvent_Skill_AttackResult`), `0x10000565` (`Signals@Usage::User`) |
| `0x84FE00A7` | `31` | Pack (C->S) | `0x84FE0042` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D0009` (`+700280`) | — |
| `0x84FE00A8` | `32` | Pack (C->S) | `0x84FE006F` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x819D0013` (`+701248`) | NAT: `0x00E70001` (`0x140821170`), `0x00E70002` (`0x1408211d0`); PID: `0x10000085` (`ErrorCategory_Script`), `0x10000565` (`Signals@Usage::User`) |
| `0x84FE00A9` | `33` | Pack (C->S) | `0x84FE008E` | `()` | `0x00D9000C` (`self`) | `0x819D0004` (`+699692`) [C++: `0x140a93042`] | PID: `0x10000451` (`NPC_ActionPicker_MPMissionControllerString`), `0x1000028B` (`QuestEvent_Skill_AttackResult`), `0x1000056B` (`Buyback_BuybackID`) |
| `0x84FE00AA` | `34` | Pack (C->S) | `0x84FE0062` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D0005` (`+699844`) [C++: `0x140a93165`] | — |
| `0x84FE00AB` | `35` | Pack (C->S) | `0x84FE000D` | `()` | `0x00D9000C` (`self`) | `0x819D0003` (`+699520`) | PID: `0x100001C4` (`RandomBox@Entity.m_bRepeatsAllowed`), `0x10000565` (`Signals@Usage::User`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`) |
| `0x84FE00AC` | `36` | Pack (C->S) | `0x84FE0078` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x819D000A` (`+700316`) | — |
| `0x84FE00AD` | `37` | Pack (C->S) | `0x84FE0080` | `(i32/u32, ref)` | `0x00D9000C` (`self`) | `0x819D0051` (`+700356`) [C++: `0x140a933de`], `0x819D0006` (`+700388`) [C++: `0x140a93290`] | — |
| `0x84FE00AF` | `38` | Pack (C->S) | `0x84FE007F` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D0008` (`+700112`) [C++: `0x140a93505`] | PID: `0x10000451` (`NPC_ActionPicker_MPMissionControllerString`), `0x1000028B` (`QuestEvent_Skill_AttackResult`), `0x10000015` (`GRSEvent_Crafting_IsCritical`) |
| `0x84FE00B3` | `40` | Pack (C->S) | `0x84FE00B2` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x819D002C` (`+704352`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000451` (`NPC_ActionPicker_MPMissionControllerString`), `0x1000028B` (`QuestEvent_Skill_AttackResult`), `0x00000064` (`LedgePhysicsController_LedgePlaneDistance`) |
| `0x84FE00B8` | `41` | Unpack (S->C) | `0x84FE00B8` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452005A` (`+719076`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00BA` | `42` | Unpack (S->C) | `0x84FE00BA` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452005B` (`+719112`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00C4` | `43` | Unpack (S->C) | `0x84FE00C4` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x819D002F` (`+704660`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00C6` | `44` | Unpack (S->C) | `0x84FE00C6` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x819D0030` (`+704600`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00D7` | `46` | Unpack (S->C) | `0x84FE00D7` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452006B` (`+719744`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00DB` | `48` | Unpack (S->C) | `0x84FE00DB` | `()` | `0x00D60007(0x06C3)` | `0x8452006C` (`+719720`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00DC` | `49` | Pack (C->S) | `0x84FE00D9` | `()` | `0x00D9000C` (`self`) | `0x819D0031` (`+704720`) | NAT: `0x013F0012` (`0x140b3d670`); PID: `0x10001D1F` (`Antispam_FellowshipReadyCheckRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) |
| `0x84FE00DD` | `50` | Pack (C->S) | `0x84FE00D5` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x874D0001` (`+1676200`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`) |
| `0x84FE00E6` | `51` | Unpack (S->C) | `0x84FE00E6` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520071` (`+719540`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE00FA` | `52` | Unpack (S->C) | `0x84FE00FA` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8452007B` (`+719244`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0106` | `53` | Unpack (S->C) | `0x84FE0106` | `()` | `0x00D60007(0x06C3)` | `0x84520082` (`+719572`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE010E` | `56` | Pack (C->S) | `0x84FE010D` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x819D003F` (`+705900`) [C++: `0x140a9c660`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10001C6F` (`Fellowship_DesiredConjunctionSkill_DataID`) |
| `0x84FE010F` | `57` | Pack (C->S) | `0x84FE010C` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x819D0040` (`+706144`) [C++: `0x140a9cad0`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10001C6F` (`Fellowship_DesiredConjunctionSkill_DataID`) |
| `0x84FE0118` | `58` | Unpack (S->C) | `0x84FE0118` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8452008A` (`+719804`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0119` | `59` | Unpack (S->C) | `0x84FE0119` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8452008B` (`+719776`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE011D` | `61` | Pack (C->S) | `0x84FE011C` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x819D0048` (`+703084`) | — |
| `0x84FE011E` | `62` | Unpack (S->C) | `0x84FE011E` | `(f32, i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8452008C` (`+719596`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0133` | `63` | Unpack (S->C) | `0x84FE0133` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520097` (`+719632`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0136` | `64` | Unpack (S->C) | `0x84FE0136` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520099` (`+719692`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0138` | `65` | Unpack (S->C) | `0x84FE0138` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x84520098` (`+719660`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE013E` | `66` | Unpack (S->C) | `0x84FE013E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x845200A0` (`+719860`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0149` | `69` | Unpack (S->C) | `0x84FE0149` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x845200A1` (`+719832`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE014B` | `70` | Pack (C->S) | `0x84FE013F` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D0067` (`+707488`) | — |
| `0x84FE014C` | `71` | Pack (C->S) | `0x84FE0147` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D0066` (`+707460`) | — |
| `0x84FE014E` | `72` | Unpack (S->C) | `0x84FE014E` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x845200A3` (`+719208`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE0153` | `73` | Unpack (S->C) | `0x84FE0153` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x845200A5` (`+719276`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE016A` | `76` | Unpack (S->C) | `0x84FE016A` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x819D007D` (`+708408`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE016B` | `77` | Unpack (S->C) | `0x84FE016B` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x819D007C` (`+708380`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x84FE016E` | `78` | Pack (C->S) | `0x84FE0169` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x819D007F` (`+708508`) [C++: `0x140a9c745`] | — |
| `0x84FE016F` | `79` | Pack (C->S) | `0x84FE0168` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81020018` (`+498996`) [C++: `0x140b5e555`], `0x81020025` (`+499772`), `0x819D0080` (`+708580`) [C++: `0x140aa0715`] | NAT: `0x00270006` (`0x1403ae7b0`), `0x0001001C` (`0x140822da0`), `0x00010006` (`0x140821d40`); PID: `0x1000305D` (`Annotation_Text`), `0x10000068` (`UI_Pet_PetAggressiveButton`) |

### Class `0x0502` (`1282`) — GuildSystem (Kinship Management, Roster, MotD, Ranks, Lifespan) <a id="rpc-class-0x0502"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x016C`
- **Method Counts**: `20` Pack (C->S), `25` Unpack (S->C) (`45` total)
- **Referenced Properties (`pids.txt`, 52 total)**: `0x100074E1` (`Guild_MaxGuildCalendarEvents`), `0x10001919` (`Guild_PlayerNoteMaxLength`), `0x1000281A` (`Guild_InfoLoaded`), `0x10000012` (`Guild_ID`), `0x1000058E` (`Guild_Name`), `0x1000144E` (`Guild_Theme`), `0x1000123E` (`Guild_PlayerRankID`), `0x10000AEB` (`Guild_LeaderID`), `0x10000E99` (`Guild_Recruiter`), `0x10000AEA` (`Guild_RecruiterName`), `0x10001162` (`Guild_RecruiterGuildName`), `0x10000DA6` (`Guild_ConfirmationToken`), `0x10000F9D` (`Guild_LifespanRank`), `0x10007429` (`Guild_GuildIDForRecruitment`), `0x00000680` (`NumCharactersAllowedStruct`), `0x10000046` (`Craft_Weaponsmith_ProficiencyLevel`), `0x00000040` (`CameraShakeScript_OffsetY`), `0x00000100` (`MapNote_Level`) *(+34 more)*
- **Referenced WLib Strings (`0x0F`, 31 total)**: `"> null String Info Passed in!"`, `"GuildSystem::UpdateGuildName"`, `"GuildSystem::OnMotDUpdated"`, `"An Admin"`, `"Error: Invalid Target."`, `"> null Guild Passed in!"`, `"GuildSystem::OnGuildUpdated"`, `"/kinshipmotd <reset/view> <Kinship InstanceID>"`, `"/change kinshipmotd <reset/view> <Kinship InstanceID>"`, `"Note: Also can get the kinship IID off of a selected player\n  reset - Resets th"` *(+21 more)*
- **Referenced Native Functions (`0x15`, 70 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00B0000C` (`0x140385130`), `0x00B00003` (`0x1403beb80`), `0x01410021` (`0x140b3acb0`), `0x01410002` (`0x140b36210`), `0x01AA0002` (`0x140a7dae0`), `0x00CC0003` (`0x1403c1d20`), `0x01410003` (`0x140b35df0`), `0x00C70012` (`0x1403abd30`), `0x01AA0001` (`0x140a7ef40`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`) *(+56 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 24 total)**: `0x816C0001` @ `0x140b5e1f2` (fn `0x140b5e1a4`), `0x816C0005` @ `0x140b9ceb0` (fn `0x140b9ce26`), `0x816C0007` @ `0x140b5e330` (fn `0x140b5e2a6`), `0x816C0008` @ `0x140b4f2f2` (fn `0x140b4f2a4`), `0x816C0009` @ `0x140b5db35` (fn `0x140b5dab6`), `0x816C000A` @ `0x140b4ef35` (fn `0x140b4eeb6`), `0x816C000B` @ `0x140b5ffa5` (fn `0x140b5ff26`), `0x816C0010` @ `0x140b50af5` (fn `0x140b50a76`), `0x816C0011` @ `0x140b5ef92` (fn `0x140b5ef44`), `0x816C0012` @ `0x140b60be2` (fn `0x140b60b8f`), `0x816C0013` @ `0x140b66984` (fn `0x140b6691a`), `0x816C007B` @ `0x140b9d2c2` (fn `0x140b9d274`) *(+12 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85020002` | `1` | Unpack (S->C) | `0x85020002` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0121` (`+758888`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020008` | `2` | Unpack (S->C) | `0x85020008` | `(ref)` | `0x00D60007(0x06C3)` | `0x816C012E` (`+758636`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8502000C` | `4` | Unpack (S->C) | `0x8502000C` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0136` (`+758824`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020018` | `7` | Unpack (S->C) | `0x85020018` | `(ref)` | `0x00D60007(0x06C3)` | `0x816C0112` (`+758580`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8502001C` | `9` | Unpack (S->C) | `0x8502001C` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0131` (`+758732`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020030` | `11` | Unpack (S->C) | `0x85020030` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0130` (`+758664`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020042` | `14` | Unpack (S->C) | `0x85020042` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x816C0127` (`+758792`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020044` | `15` | Unpack (S->C) | `0x85020044` | `(i32/u32, ref, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C013A` (`+758964`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020055` | `17` | Unpack (S->C) | `0x85020055` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0124` (`+758396`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020064` | `20` | Unpack (S->C) | `0x85020064` | `(i32/u32, ref, i64/u64, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C011B` (`+758920`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020066` | `22` | Unpack (S->C) | `0x85020066` | `(i32/u32, ref, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C012A` (`+759004`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020080` | `24` | Unpack (S->C) | `0x85020080` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x816C011C` (`+759084`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020091` | `26` | Unpack (S->C) | `0x85020091` | `(ref)` | `0x00D60007(0x06C3)` | `0x816C0117` (`+758608`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020097` | `27` | Unpack (S->C) | `0x85020097` | `(i32/u32, i64/u64, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C012B` (`+759044`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8502009B` | `28` | Unpack (S->C) | `0x8502009B` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0133` (`+758856`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8502009C` | `29` | Unpack (S->C) | `0x8502009C` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0122` (`+758696`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850200A8` | `32` | Unpack (S->C) | `0x850200A8` | `(i32/u32, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0119` (`+758544`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850200B0` | `34` | Pack (C->S) | `0x8502001A` | `()` | `0x00D9000C` (`self`) | `0x816C0008` (`+767264`) [C++: `0x140b4f2f2`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000481` (`Usage_RestrictedEffect`), `0x00000800` (`Render_IsFogVolume`) |
| `0x850200B4` | `35` | Pack (C->S) | `0x850200A3` | `(ref)` | `0x00D9000C` (`self`) | `0x816C0003` (`+767060`) | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000416` (`UI_SecureTrade_Currency_Type`), `0x100001F2` (`Drama_NamedDrama_Instant`) |
| `0x850200B7` | `36` | Pack (C->S) | `0x8502000B` | `()` | `0x00D9000C` (`self`) | `0x816C0001` (`+768900`) [C++: `0x140b5e1f2`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`) |
| `0x850200B8` | `37` | Pack (C->S) | `0x85020045` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x816C0010` (`+769024`) [C++: `0x140b50af5`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000336` (`UI_CharacterManagement_Reaction`), `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000092` (`Effect_Lootgen_DiscoveryFailureMessage`) |
| `0x850200B9` | `38` | Pack (C->S) | `0x85020011` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x816C0013` (`+760800`) [C++: `0x140b66984`] | — |
| `0x850200BB` | `40` | Pack (C->S) | `0x85020012` | `(ref)` | `0x00D9000C` (`self`) | `0x816C008C` (`+767664`) | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x00000200` (`Slider_Fly`), `0x10000092` (`Effect_Lootgen_DiscoveryFailureMessage`) |
| `0x850200BD` | `41` | Pack (C->S) | `0x85020033` | `()` | `0x00D9000C` (`self`) | `0x816C007B` (`+772308`) [C++: `0x140b9d2c2`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`) |
| `0x850200BE` | `42` | Pack (C->S) | `0x8502002F` | `()` | `0x00D9000C` (`self`) | `0x816C0011` (`+767480`) [C++: `0x140b5ef92`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000092` (`Effect_Lootgen_DiscoveryFailureMessage`) |
| `0x850200BF` | `43` | Pack (C->S) | `0x85020031` | `(ref)` | `0x00D9000C` (`self`) | `0x816C0005` (`+775688`) [C++: `0x140b9ceb0`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000327` (`UI_PrivateEncounter_SelectedVenue`), `0x10000092` (`Effect_Lootgen_DiscoveryFailureMessage`) |
| `0x850200C0` | `44` | Pack (C->S) | `0x850200A5` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x816C000A` (`+771996`) [C++: `0x140b4ef35`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x00000100` (`MapNote_Level`), `0x10000092` (`Effect_Lootgen_DiscoveryFailureMessage`) |
| `0x850200C1` | `45` | Pack (C->S) | `0x85020056` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x816C0006` (`+770488`) | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000416` (`UI_SecureTrade_Currency_Type`), `0x100001F2` (`Drama_NamedDrama_Instant`), `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`) |
| `0x850200C3` | `46` | Pack (C->S) | `0x85020065` | `(ref)` | `0x00D9000C` (`self`) | `0x816C0007` (`+770204`) [C++: `0x140b5e330`] | — |
| `0x850200C5` | `47` | Pack (C->S) | `0x8502007C` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x816C0009` (`+771684`) [C++: `0x140b5db35`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000339` (`EffectMod_Test_VOT_InitialChangeMult_AddMod2`), `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000092` (`Effect_Lootgen_DiscoveryFailureMessage`) |
| `0x850200C7` | `48` | Pack (C->S) | `0x8502005B` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x816C000B` (`+775976`) [C++: `0x140b5ffa5`] | NAT: `0x01AA0001` (`0x140a7ef40`); PID: `0x10000338` (`Alert_GenericInt`), `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000481` (`Usage_RestrictedEffect`) |
| `0x850200C9` | `49` | Pack (C->S) | `0x85020083` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x816C0012` (`+760744`) [C++: `0x140b60be2`] | — |
| `0x850200DF` | `50` | Unpack (S->C) | `0x850200DF` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0152` (`+759116`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850200E2` | `51` | Unpack (S->C) | `0x850200E2` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x816C0155` (`+758764`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850200E8` | `53` | Pack (C->S) | `0x850200E6` | `(ref)` | `0x00D9000C` (`self`) | `0x816C015A` (`+770240`) [C++: `0x140b9cff0`] | NAT: `0x01AA0001` (`0x140a7ef40`), `0x00E10003` (`0x1403c4630`); PID: `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x10000044` (`Craft_Prospector_ProficiencyLevel`); MSG: `"> null Note Passed in!"`, `"GuildSystem::UpdatePlayerNote"` |
| `0x850200F8` | `54` | Unpack (S->C) | `0x850200F8` | `(f64)` | `0x00D60007(0x06C3)` | `0x816C0185` (`+759148`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020232` | `58` | Pack (C->S) | `0x85020231` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816C02EE` (`+777120`) | — |
| `0x85020233` | `59` | Pack (C->S) | `0x85020230` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x816C02ED` (`+776992`) | — |
| `0x85020234` | `60` | Pack (C->S) | `0x8502022F` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816C02EC` (`+776788`) | — |
| `0x85020244` | `62` | Pack (C->S) | `0x85020243` | `(ref)` | `0x00D9000D` (`local_0`) | `0x816C0303` (`+777300`) | — |
| `0x85020252` | `63` | Unpack (S->C) | `0x85020252` | `(ref)` | `0x00D60007(0x06C3)` | `0x816C030F` (`+758432`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020256` | `64` | Unpack (S->C) | `0x85020256` | `(ref)` | `0x00D60007(0x06C3)` | `0x816C0310` (`+758488`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85020258` | `65` | Unpack (S->C) | `0x85020258` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x816C0311` (`+758516`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8502025C` | `66` | Unpack (S->C) | `0x8502025C` | `(ref)` | `0x00D60007(0x06C3)` | `0x816C0315` (`+758460`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8502025F` | `67` | Unpack (S->C) | `0x8502025F` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x816C031D` (`+758364`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0506` (`1286`) — SkillSystem Auxiliary Request (paired with 0x017A / 0x0528) <a id="rpc-class-0x0506"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x017A`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 107 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x0000017A` (`PEDesc_RotationVelocityZ`), `0x1000180A` (`GOP_EnableSkillTargetNearestEnemy`), `0x0000033B` (`Render_AdjustableScale`), `0x0000030E` (`PropertyTarget_PropertyName`), `0x1000002C` (`MPAdvancementControl_VirtueCostProgression`), `0x100002A8` (`EnvironmentType`), `0x10000C6E` (`Combat_Violent`), `0x1000009F` (`Signals@NPC::HintChange::Source`), `0x1000017F` (`Effect_Duration_ConstantPulseCount`), `0x10000281` (`Combat_ActionDurationMultiplierMod`), `0x100002D4` (`ConsiderationTable_RankPerDifferentialArray`), `0x10000219` (`Craft_Cook_CriticalChanceAddModifier`), `0x100000E6` (`UI_PropertyMeter_MinValuePropertyName`), `0x100003E8` (`Craft_Leatherworker_StartTime`), `0x100003E9` (`Craft_Jeweler_CompletedProficiencyLevel`), `0x00000599` (`GridSnapAngle`), `0x000005B9` (`ScriptTable_NodeOutputs`) *(+89 more)*
- **Referenced WLib Strings (`0x0F`, 23 total)**: `"."`, `"> ERROR: Got a request for a skill that does not exist in data(skillID:"`, `"SkillSystem::GetIconDID"`, `"> ERROR: Unable to lookup skill from did:"`, `"SkillSystem::IsSkillHarmful"`, `"Unable to get skill by DID:"`, `"SkillSystem::Client_CanUseSkillBasedOnTargetVitals"`, `"Failed to cast target to agent. Current target:"`, `"It must be between 0 and 100."`, `"Received invalid health percentage:"` *(+13 more)*
- **Referenced Native Functions (`0x15`, 119 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+105 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 25 total)**: `0x817A00D3` @ `0x140b61ab5` (fn `0x140b61a80`), `0x817A0016` @ `0x140b4e314` (fn `0x140b4e2a5`), `0x817A0149` @ `0x140b4e45c` (fn `0x140b4e3bc`), `0x817A0152` @ `0x140b4e677` (fn `0x140b4e5c0`), `0x817A0018` @ `0x140b4e752` (fn `0x140b4e6ff`), `0x817A000A` @ `0x140b565f4` (fn `0x140b5658a`), `0x817A0009` @ `0x140b56704` (fn `0x140b5669a`), `0x817A0102` @ `0x140b6531b` (fn `0x140b652d0`), `0x817A00EA` @ `0x140b65417` (fn `0x140b65360`), `0x817A0150` @ `0x140b69927` (fn `0x140b698fb`), `0x817A00E8` @ `0x140b70473` (fn `0x140b70420`), `0x817A0008` @ `0x140b73b9b` (fn `0x140b73b50`) *(+13 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8506000A` | `2` | Pack (C->S) | `0x85060009` | `()` | `0x00D9000C` (`self`) | `0x817A00D3` (`+1504268`) [C++: `0x140b61ab5`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0507` (`1287`) — Inscription / PlayerBio System (/playerbio & /iteminscription) <a id="rpc-class-0x0507"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x016F`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`)
- **Referenced WLib Strings (`0x0F`, 8 total)**: `"/playerbio <reset/view> <InstanceID>"`, `"/change playerbio <reset/view> <InstanceID>"`, `"reset - Resets the bio of a player\n  view  - Views the bio of a player"`, `"Error: Invalid Target."`, `"set"`, `"/iteminscription <set/reset/view> <newinscription> <InstanceID>"`, `"/change iteminscription <set/reset/view> <newinscription> <InstanceID>"`, `"set   - Sets the inscription on an item\n  reset - Resets the inscription on an "`
- **Referenced Native Functions (`0x15`, 42 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+28 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x816F0001` @ `0x140b5a880` (fn `0x140b5a7f6`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85070012` | `4` | Pack (C->S) | `0x8507000F` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x816F0001` (`+824728`) [C++: `0x140b5a880`] | — |

### Class `0x0508` (`1288`) — InteractionPermission & PvPOverride Admin System (/showattackperm, /pvpoverride) <a id="rpc-class-0x0508"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x06C2`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 4 total)**: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`), `0x00E1001D` (`0x1403cbc60`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8508000C` | `3` | Pack (C->S) | `0x85080008` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2003C` (`+58116`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x8508000D` | `4` | Pack (C->S) | `0x85080007` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x86C200AF` (`+57892`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`) |

### Class `0x050A` (`1290`) — Inventory Item Creation & Container Operations (/create, Stack/Move/Equip) <a id="rpc-class-0x050a"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0170`, `0x0AF4`
- **Method Counts**: `8` Pack (C->S), `2` Unpack (S->C) (`10` total)
- **Referenced Properties (`pids.txt`, 20 total)**: `0x10000E7B` (`Inventory_Quantity`), `0x0000034E` (`Container_Slot`), `0x100076F2` (`NPC_NeighborQualities`), `0x100077B0` (`NPC_Neighbor_OutfitType`), `0x10005EA5` (`Inventory_SlotAssignmentsNeedUpdate`), `0x1000035A` (`Effect_Reflective_RestrictAbsorbClass`), `0x10000218` (`Craft_Jeweller_Enabled`), `0x1000114B` (`Inventory_MaxBackpackSlots`), `0x1000029D` (`Inventory_MaxEquipmentSlots`), `0x100009FA` (`Inventory_MaxOverflowSlots`), `0x10000D88` (`Inventory_DefaultSlot`), `0x10000F82` (`Inventory_CompatibleSlot`), `0x10001516` (`Inventory_PrecludedSlot`), `0x1000029E` (`Inventory_CanBeContained`), `0x1000114D` (`Inventory_Unique`), `0x10000835` (`Item_Value`), `0x1000082D` (`Inventory_MaxQuantity`), `0x10000C8A` (`Inventory_ReservedForIID`) *(+2 more)*
- **Referenced WLib Strings (`0x0F`, 4 total)**: `"InvSupportSystem::UI_HandleTransactionStart"`, `"InvSupportSystem::UI_DisplayResult"`, `"usage: /create <item name>.wc [ distant \| halt \| wander \| neversaydie \| inte"`, `"An item of the listed type will be created\n distant\|d:        this will create"`
- **Referenced Native Functions (`0x15`, 32 total)**: `0x00A90003` (`0x1403a4ca0`), `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00E70001` (`0x140821170`), `0x01440001` (`0x140b37560`), `0x00A9003F` (`0x1403a42c0`), `0x0068000C` (`0x140385130`), `0x00680004` (`0x1403bead0`), `0x0030000A` (`0x140386f40`), `0x006F0006` (`0x1403c2da0`), `0x0144000F` (`0x140b49080`), `0x00B0000B` (`0x140385130`), `0x00B00003` (`0x1403beb80`) *(+18 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 26 total)**: `0x8AF40004` @ `0x140b66ac3` (fn `0x140b66a2e`), `0x8AF40017` @ `0x140b929e1` (fn `0x140b92980`), `0x81700081` @ `0x140b52008` (fn `0x140b51f9e`), `0x81700103` @ `0x140b523bb` (fn `0x140b52370`), `0x8170000B` @ `0x140b53a22` (fn `0x140b539d4`), `0x8170000C` @ `0x140b53b34` (fn `0x140b53aca`), `0x81700009` @ `0x140b55792` (fn `0x140b55744`), `0x8170000D` @ `0x140b55d62` (fn `0x140b55d14`), `0x8170000A` @ `0x140b561e2` (fn `0x140b56194`), `0x81700074` @ `0x140b5aa82` (fn `0x140b5aa34`), `0x817000A7` @ `0x140b5ab82` (fn `0x140b5ab34`), `0x81700120` @ `0x140b6019b` (fn `0x140b60150`) *(+14 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x850A0015` | `5` | Unpack (S->C) | `0x850A0015` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81700101` (`+848608`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850A0021` | `9` | Pack (C->S) | `0x850A000B` | `(i32/u32, i32/u32, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8AF40005` (`+837684`) | — |
| `0x850A0022` | `10` | Pack (C->S) | `0x850A0001` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8AF40014` (`+837628`) | — |
| `0x850A0023` | `11` | Pack (C->S) | `0x850A000C` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8AF40002` (`+837328`) | — |
| `0x850A0024` | `12` | Pack (C->S) | `0x850A0016` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8AF4000C` (`+837152`) | — |
| `0x850A0025` | `13` | Pack (C->S) | `0x850A001D` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8AF40006` (`+837508`) | — |
| `0x850A0027` | `15` | Pack (C->S) | `0x850A0019` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8AF40004` (`+837384`) [C++: `0x140b66ac3`] | — |
| `0x850A0035` | `17` | Pack (C->S) | `0x850A0034` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8AF40017` (`+838508`) [C++: `0x140b929e1`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x850A0037` | `18` | Unpack (S->C) | `0x850A0037` | `(ref)` | `0x00D60007(0x06C3)` | `0x8AF4001A` (`+838660`) | NAT: `0x00E70001` (`0x140821170`), `0x0144000F` (`0x140b49080`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850A0039` | `20` | Pack (C->S) | `0x850A0038` | `(ref)` | `0x00D9000C` (`self`) | `0x8AF40022` (`+839032`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x10005EA5` (`Inventory_SlotAssignmentsNeedUpdate`) |

### Class `0x050B` (`1291`) — ItemWearSystem (Equipment Wear State & Item Repair) <a id="rpc-class-0x050b"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0171`
- **Method Counts**: `2` Pack (C->S), `4` Unpack (S->C) (`6` total)
- **Referenced Properties (`pids.txt`, 15 total)**: `0x10000045` (`Craft_Weaponsmith_ProficiencyXP`), `0x00000354` (`Name`), `0x10000063` (`UI_SkillDisplay_IconSize`), `0x10000EED` (`Antispam_RepairItemRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x1000132C` (`Item_CurStructurePoints`), `0x10001151` (`Item_MaxStructurePoints`), `0x10000919` (`Item_WearState`), `0x10000ACC` (`Item_RepairCostPerPointProgression`), `0x1000039F` (`Item_DurabilityValue`), `0x100001B5` (`Inventory_SlotCache_Eq_Earring1`), `0x100001D1` (`Trait_Burglar_EyeForVulnerability`), `0x00000272` (`MusicScript_FireAndForget`), `0x0000027C` (`TerrainTypeDesc_TextureUOffset`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 18 total)**: `"usage: /itemwear [cmd]"`, `"event combathit: Executes a combat hit item wear event."`, `"event combatblock: Executes a combat block item wear event."`, `"event combatparry: Executes a combat parry item wear event."`, `"event craft: Executes a craft item wear event."`, `"event harvest: Executes a harvest item wear event."`, `"event death: Executes a death item wear event."`, `"event deathbyduel: Executes a death by duel item wear event."`, `"event resurrect: Executes a resurrect item wear event."`, `"repair: Repairs current item selection."` *(+8 more)*
- **Referenced Native Functions (`0x15`, 27 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`), `0x0030000B` (`0x140385ab0`), `0x00C7000C` (`0x1403afdf0`), `0x00340004` (`0x140382f70`), `0x00340003` (`0x140396120`), `0x00270028` (`0x140397600`), `0x00C70009` (`0x14037c4c0`), `0x01450001` (`0x140b310e0`), `0x01450002` (`0x140b42ad0`), `0x00E10005` (`0x1403ca7d0`) *(+13 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 5 total)**: `0x81710003` @ `0x140b808c6` (fn `0x140b806e1`), `0x81710001` @ `0x140b8d88b` (fn `0x140b8d7c0`), `0x81710002` @ `0x140b8f01c` (fn `0x140b8efc0`), `0x81710005` @ `0x140b92b28` (fn `0x140b92a90`), `0x81710004` @ `0x140b92c1a` (fn `0x140b92b60`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x850B0004` | `2` | Unpack (S->C) | `0x850B0004` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81710008` (`+955708`) | NAT: `0x00340004` (`0x140382f70`), `0x00340003` (`0x140396120`), `0x00270028` (`0x140397600`); PID: `0x00000354` (`Name`), `0x10000063` (`UI_SkillDisplay_IconSize`), `0x000006C3` (`Appearance_ClassList`) |
| `0x850B0005` | `3` | Unpack (S->C) | `0x850B0005` | `()` | `0x00D60007(0x06C3)` | `0x81710007` (`+955904`) | NAT: `0x01450001` (`0x140b310e0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850B0006` | `4` | Unpack (S->C) | `0x850B0006` | `()` | `0x00D60007(0x06C3)` | `0x8171000C` (`+955924`) | NAT: `0x01450002` (`0x140b42ad0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850B000A` | `5` | Unpack (S->C) | `0x850B000A` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81710009` (`+955512`) | NAT: `0x00340004` (`0x140382f70`), `0x00340003` (`0x140396120`), `0x00270028` (`0x140397600`); PID: `0x00000354` (`Name`), `0x10000063` (`UI_SkillDisplay_IconSize`), `0x000006C3` (`Appearance_ClassList`) |
| `0x850B000D` | `7` | Pack (C->S) | `0x850B0002` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x81710006` (`+954996`) | NAT: `0x0030000A` (`0x140386f40`), `0x0030000B` (`0x140385ab0`), `0x00C7000C` (`0x1403afdf0`); PID: `0x10000045` (`Craft_Weaponsmith_ProficiencyXP`) |
| `0x850B000E` | `8` | Pack (C->S) | `0x850B000B` | `(i64/u64, ref)` | `0x00D9000C` (`self`) | `0x8171000B` (`+955264`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00C7000C` (`0x1403afdf0`); PID: `0x10000045` (`Craft_Weaponsmith_ProficiencyXP`) |

### Class `0x050C` (`1292`) — LFGSystem (Looking For Group / Social Panel Queue & Search) <a id="rpc-class-0x050c"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x1021`
- **Method Counts**: `10` Pack (C->S), `28` Unpack (S->C) (`38` total)
- **Referenced Properties (`pids.txt`, 16 total)**: `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x1000004D` (`TextItem_Title`), `0x100066D0` (`LFG_WhoSemaphoreID`), `0x100066CF` (`LFG_LFQuestSemaphoreID`), `0x100066CE` (`LFG_LFInstanceSemaphoreID`), `0x100065EB` (`Antispam_LFGMembershipRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10001B2E` (`Antispam_LFGRequest`), `0x10000983` (`Area_CurrentArea`), `0x10006595` (`LFG_Quest_Array`), `0x10006596` (`LFG_Quest_Struct`), `0x10001E4B` (`LFG_QuestDID`), `0x100066CB` (`LFG_Instance_Array`), `0x100066CC` (`LFG_Instance_Struct`), `0x100066CD` (`LFG_InstanceDID`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 59 total)**: `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E1001C` (`0x1403cbac0`), `0x00E10002` (`0x1403ba7e0`), `0x00DD0001` (`0x1403b6150`), `0x00DD0008` (`0x1403b5b70`), `0x01460009` (`0x140b3a3e0`), `0x01460019` (`0x140b39cf0`), `0x0146001A` (`0x140b39e90`), `0x00C7000C` (`0x1403afdf0`), `0x00C70010` (`0x14038bee0`), `0x00E10016` (`0x1403ba9d0`), `0x01CF0015` (`0x140386b70`), `0x0146001C` (`0x140b3b400`) *(+45 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 13 total)**: `0x90210092` @ `0x140b5f710` (fn `0x140b5f67f`), `0x90210093` @ `0x140b5f5d0` (fn `0x140b5f53f`), `0x90210049` @ `0x140b60672` (fn `0x140b60624`), `0x90210060` @ `0x140b701a0` (fn `0x140b70170`), `0x90210073` @ `0x140b8f104` (fn `0x140b8f095`), `0x90210094` @ `0x140b90f9b` (fn `0x140b90f50`), `0x9021006A` @ `0x140b927cb` (fn `0x140b9275b`), `0x90210081` @ `0x140b928fb` (fn `0x140b9288b`), `0x9021006D` @ `0x140b96022` (fn `0x140b95fd4`), `0x90210078` @ `0x140b96122` (fn `0x140b960d4`), `0x90210050` @ `0x140b97f52` (fn `0x140b97f04`), `0x90210063` @ `0x140b9c090` (fn `0x140b9c006`) *(+1 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x850C0011` | `1` | Unpack (S->C) | `0x850C0011` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x90210032` (`+964368`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0017` | `2` | Unpack (S->C) | `0x850C0017` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x9021002F` (`+963920`) | NAT: `0x01460003` (`0x140b39aa0`), `0x0001000B` (`0x140822750`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0020` | `3` | Unpack (S->C) | `0x850C0020` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x90210010` (`+964496`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C005E` | `5` | Unpack (S->C) | `0x850C005E` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x9021001C` (`+964240`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0064` | `6` | Pack (C->S) | `0x850C0059` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x90210006` (`+965184`) | — |
| `0x850C007B` | `7` | Unpack (S->C) | `0x850C007B` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x90210016` (`+964272`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C007D` | `8` | Unpack (S->C) | `0x850C007D` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x90210039` (`+964400`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C008A` | `9` | Unpack (S->C) | `0x850C008A` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x90210021` (`+964528`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00A3` | `10` | Unpack (S->C) | `0x850C00A3` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x9021004D` (`+962356`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00A7` | `11` | Unpack (S->C) | `0x850C00A7` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x9021004E` (`+962344`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00B4` | `12` | Unpack (S->C) | `0x850C00B4` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210054` (`+964128`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00B6` | `13` | Unpack (S->C) | `0x850C00B6` | `(ref)` | `0x00D60007(0x06C3)` | `0x90210052` (`+963668`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00BA` | `14` | Unpack (S->C) | `0x850C00BA` | `(ref)` | `0x00D60007(0x06C3)` | `0x90210056` (`+963640`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00BB` | `15` | Unpack (S->C) | `0x850C00BB` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210053` (`+964100`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00BD` | `17` | Unpack (S->C) | `0x850C00BD` | `(ref)` | `0x00D60007(0x06C3)` | `0x90210058` (`+963772`) | NAT: `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760003` (`0x1403ba130`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00C0` | `18` | Pack (C->S) | `0x850C00BC` | `()` | `0x00D9000C` (`self`) | `0x90210057` (`+965604`) | — |
| `0x850C00E4` | `20` | Unpack (S->C) | `0x850C00E4` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210075` (`+963464`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00E6` | `21` | Unpack (S->C) | `0x850C00E6` | `(ref)` | `0x00D60007(0x06C3)` | `0x9021007B` (`+964756`) | NAT: `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760003` (`0x1403ba130`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00EA` | `23` | Unpack (S->C) | `0x850C00EA` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210065` (`+963232`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00ED` | `25` | Unpack (S->C) | `0x850C00ED` | `(ref)` | `0x00D60007(0x06C3)` | `0x9021008D` (`+964624`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00F4` | `26` | Unpack (S->C) | `0x850C00F4` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x9021007F` (`+963176`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00F7` | `27` | Unpack (S->C) | `0x850C00F7` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210066` (`+963260`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00FE` | `29` | Unpack (S->C) | `0x850C00FE` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x9021008B` (`+963408`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C00FF` | `30` | Unpack (S->C) | `0x850C00FF` | `(ref)` | `0x00D60007(0x06C3)` | `0x90210061` (`+964652`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0102` | `31` | Unpack (S->C) | `0x850C0102` | `(ref)` | `0x00D60007(0x06C3)` | `0x90210062` (`+965036`) | NAT: `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760003` (`0x1403ba130`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0103` | `32` | Unpack (S->C) | `0x850C0103` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210070` (`+963436`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0106` | `33` | Unpack (S->C) | `0x850C0106` | `(ref)` | `0x00D60007(0x06C3)` | `0x9021008E` (`+964932`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C010F` | `34` | Unpack (S->C) | `0x850C010F` | `(ref)` | `0x00D60007(0x06C3)` | `0x90210077` (`+964904`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0112` | `36` | Unpack (S->C) | `0x850C0112` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210071` (`+963204`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C0115` | `37` | Unpack (S->C) | `0x850C0115` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x90210082` (`+963492`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850C011F` | `39` | Pack (C->S) | `0x850C00EC` | `()` | `0x00D9000C` (`self`) | `0x90210076` (`+965392`) | — |
| `0x850C0120` | `40` | Pack (C->S) | `0x850C011A` | `(ref)` | `0x00D9000C` (`self`) | `0x90210087` (`+965244`) | — |
| `0x850C0121` | `41` | Pack (C->S) | `0x850C0110` | `(ref)` | `0x00D9000C` (`self`) | `0x90210084` (`+965424`) | — |
| `0x850C0122` | `42` | Pack (C->S) | `0x850C00E0` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x90210086` (`+965332`) | — |
| `0x850C0123` | `43` | Pack (C->S) | `0x850C00F9` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x90210069` (`+965512`) | — |
| `0x850C0124` | `44` | Pack (C->S) | `0x850C00E9` | `()` | `0x00D9000C` (`self`) | `0x90210067` (`+965572`) | — |
| `0x850C0133` | `47` | Pack (C->S) | `0x850C0132` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x90210092` (`+961772`) [C++: `0x140b5f710`] | — |
| `0x850C0134` | `48` | Pack (C->S) | `0x850C012F` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x90210093` (`+961824`) [C++: `0x140b5f5d0`] | — |

### Class `0x050E` (`1294`) — LandmarkSystem (Point-of-Interest & Landmark Discovery) <a id="rpc-class-0x050e"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0110`
- **Method Counts**: `0` Pack (C->S), `2` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 5 total)**: `0x10000065` (`UI_Vendor_FilterType`), `0x10000856` (`Landmark_DiscoveredDID`), `0x10000136` (`DeathCount`), `0x100001E6` (`Landmark_DataID`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 12 total)**: `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00270024` (`0x140392100`), `0x01350022` (`0x140b34350`), `0x01350023` (`0x140b380d0`), `0x00E70001` (`0x140821170`), `0x00B80004` (`0x1403abbd0`), `0x00A60015` (`0x1403a9640`), `0x00B8000D` (`0x1403ae510`), `0x00B90002` (`0x1403a5840`), `0x01490005` (`0x140b38a90`), `0x01490006` (`0x140b38070`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x81100001` @ `0x140b8f24b` (fn `0x140b8f200`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x850E0003` | `1` | Unpack (S->C) | `0x850E0003` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81100005` (`+959328`) | NAT: `0x00B80004` (`0x1403abbd0`), `0x00A60015` (`0x1403a9640`), `0x00B8000D` (`0x1403ae510`); PID: `0x10000856` (`Landmark_DiscoveredDID`), `0x10000136` (`DeathCount`), `0x000006C3` (`Appearance_ClassList`) |
| `0x850E000B` | `2` | Unpack (S->C) | `0x850E000B` | `(ref)` | `0x00D60007(0x06C3)` | `0x81100003` (`+959504`) | NAT: `0x01490006` (`0x140b38070`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x050F` (`1295`) — Selection / Target-of-Target Display Sync System (0x0745 / 0x0746) <a id="rpc-class-0x050f"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0745`
- **Method Counts**: `0` Pack (C->S), `3` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 4 total)**: `0x07460001` (`0x140b430f0`), `0x0030000B` (`0x140385ab0`), `0x07460002` (`0x140b431e0`), `0x07460003` (`0x140b432d0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x850F000F` | `1` | Unpack (S->C) | `0x850F000F` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x87450001` (`+969148`) | NAT: `0x07460003` (`0x140b432d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850F0010` | `2` | Unpack (S->C) | `0x850F0010` | `(i32/u32, ref, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x87450002` (`+969016`) | NAT: `0x0030000B` (`0x140385ab0`), `0x07460002` (`0x140b431e0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x850F0012` | `3` | Unpack (S->C) | `0x850F0012` | `(i32/u32, ref, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x87450003` (`+968976`) | NAT: `0x07460001` (`0x140b430f0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0510` (`1296`) — Logout / Session Termination Dialog System (Player_HasLoggingOutDialog) <a id="rpc-class-0x0510"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x018D`, `0x06EA`, `0x0701`
- **Method Counts**: `4` Pack (C->S), `2` Unpack (S->C) (`6` total)
- **Referenced Properties (`pids.txt`, 21 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x100002E8` (`ActivationCallback::SourceEntity`), `0x1000004B` (`CraftProfession_CriticalChanceAddModifier_PropertyName`), `0x1000006C` (`UI_QuestTree_RaidImageElement`), `0x00000517` (`UICore_Dialog_type`), `0x1000288A` (`Skill_VitalReturn_Captain_ValiantStrike`), `0x0000011B` (`UICore_Dialog_UIElement_Override`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x00000693` (`UICore_Dialog_confirmation_nobuttontext`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x00000644` (`UICore_Dialog_topmost`), `0x1000009A` (`AITargetBias_DesireThreshold`), `0x0000055F` (`UICore_Dialog_message_statementtext`), `0x0000033F` (`UICore_Dialog_message_okbuttontext`), `0x0000033E` (`UICore_Dialog_message_duration`), `0x100037D4` (`Player_HasLoggingOutDialog`) *(+3 more)*
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 52 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+38 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x818D000E` @ `0x140a9cc35` (fn `0x140a9cc00`), `0x818D000A` @ `0x140aa01d0` (fn `0x140aa01a0`), `0x818D0001` @ `0x140aa0835` (fn `0x140aa0800`), `0x818D0002` @ `0x140aa08a0` (fn `0x140aa0870`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85100007` | `3` | Unpack (S->C) | `0x85100007` | `()` | `0x00D60007(0x06C3)` | `0x818D0005` (`+977500`) | NAT: `0x00270006` (`0x1403ae7b0`), `0x00330001` (`0x140384e80`), `0x00270025` (`0x1403a9500`); PID: `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x1000006C` (`UI_QuestTree_RaidImageElement`), `0x000006C3` (`Appearance_ClassList`) |
| `0x85100009` | `4` | Pack (C->S) | `0x85100001` | `()` | `0x00D9000C` (`self`) | `0x818D0004` (`+976636`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x8510000A` | `5` | Pack (C->S) | `0x85100006` | `()` | `0x00D9000C` (`self`) | `0x86EA0001` (`+1675152`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x01670008` (`0x140b353c0`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`) |
| `0x85100015` | `6` | Unpack (S->C) | `0x85100015` | `()` | `0x00D60007(0x06C3)` | `0x818D000D` (`+977616`) | NAT: `0x00270006` (`0x1403ae7b0`), `0x00330001` (`0x140384e80`), `0x00270025` (`0x1403a9500`); PID: `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x1000009A` (`AITargetBias_DesireThreshold`), `0x000006C3` (`Appearance_ClassList`) |
| `0x85100019` | `9` | Pack (C->S) | `0x85100018` | `()` | `0x00D9000C` (`self`) | `0x818D000C` (`+976684`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x8510001A` | `10` | Pack (C->S) | `0x85100017` | `()` | `0x00D9000C` (`self`) | `0x87010001` (`+1674864`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x0167001C` (`0x140b411d0`); PID: `0x000000BC` (`UICore_Dialog_message_response`) |

### Class `0x0511` (`1297`) — LootGenSystem Admin Control (/lootgen) <a id="rpc-class-0x0511"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x06C2`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 1 total)**: `0x00B60029` (`0x1403c5260`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85110006` | `2` | Pack (C->S) | `0x85110001` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C2009D` (`+45280`) | NAT: `0x00B60029` (`0x1403c5260`) |

### Class `0x0512` (`1298`) — LootSystem (Corpse/Chest Loot List, Roll/Pass, Master Looter, Pending Loot) <a id="rpc-class-0x0512"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0112`, `0x0751`
- **Method Counts**: `8` Pack (C->S), `7` Unpack (S->C) (`15` total)
- **Referenced Properties (`pids.txt`, 44 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x100010EF` (`UI_Examination_Tooltip_CharacteristicStruct`), `0x10000421` (`UI_Examination_Tooltip_DID`), `0x0000012B` (`RigidBodyScript_AngularVelocity`), `0x100012C5` (`UI_Examination_Tooltip_ArrayHelper`), `0x10000A33` (`Loot_ShouldSparkle`), `0x10000010` (`Trait_Loremaster_KnowledgeOfThePast`), `0x100000AF` (`CombatMusicControl_ModEntry`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x00000693` (`UICore_Dialog_confirmation_nobuttontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x00000644` (`UICore_Dialog_topmost`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x100016FB` (`Loot_UI_MasterLootDialog_AssigneeIID`), `0x100016FC` (`Loot_UI_MasterLootDialog_ItemIID`), `0x100016FD` (`Loot_UI_MasterLootDialog_ItemDID`) *(+26 more)*
- **Referenced WLib Strings (`0x0F`, 11 total)**: `"null i_rcPropColl passed in for item"`, `"ExaminationSystem::GetExaminationCharacteristicStructFromDIDAndPropertyCollectio"`, `"Loot Commands:"`, `"p\|pending dump\|destroy\|loot\|lootall."`, `"-- p\|pending dump - Dumps information about items in the admin's pending loot."`, `"-- p\|pending destroy [Item DID] [Item Qty] [Item Time Setup] - Destroys the giv"`, `"-- p\|pending loot [Item DID] [Item Qty] [Item Time Setup] - Loots the given ite"`, `"-- p\|pending lootall - Loots all items in admin's pending loot."`, `"Unable to get item from"`, `"ItemMungingSystem::AddMungingPropertiesForItemDIDForLevel"` *(+1 more)*
- **Referenced Native Functions (`0x15`, 81 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+67 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 11 total)**: `0x81120002` @ `0x140b92da2` (fn `0x140b92d20`), `0x81120003` @ `0x140b92ce8` (fn `0x140b92c50`), `0x81120009` @ `0x140b90d71` (fn `0x140b90d10`), `0x81120011` @ `0x140b922b5` (fn `0x140b92280`), `0x81120014` @ `0x140b92366` (fn `0x140b922f0`), `0x81120015` @ `0x140b92256` (fn `0x140b921e0`), `0x8112001B` @ `0x140b921a5` (fn `0x140b92170`), `0x81120001` @ `0x140b647ff` (fn `0x140b64776`), `0x81120017` @ `0x140b70280` (fn `0x140b70250`), `0x8112000B` @ `0x140b8ebfb` (fn `0x140b8ebb0`), `0x8112001A` @ `0x140b8ec9c` (fn `0x140b8ec40`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85120007` | `1` | Unpack (S->C) | `0x85120007` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x81120005` (`+977836`) | NAT: `0x0030000B` (`0x140385ab0`); PID: `0x10000A33` (`Loot_ShouldSparkle`), `0x000006C3` (`Appearance_ClassList`) |
| `0x85120009` | `2` | Unpack (S->C) | `0x85120009` | `(ref, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81120006` (`+978088`) | NAT: `0x01670012` (`0x140b41d00`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85120019` | `4` | Unpack (S->C) | `0x85120019` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x81120008` (`+978120`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8512001D` | `6` | Unpack (S->C) | `0x8512001D` | `(i32/u32, f32, f64, ref, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81120007` (`+978004`) | NAT: `0x01670011` (`0x140b31d80`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8512001E` | `7` | Unpack (S->C) | `0x8512001E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x81120004` (`+977920`) | NAT: `0x0030000B` (`0x140385ab0`); PID: `0x10000A33` (`Loot_ShouldSparkle`), `0x000006C3` (`Appearance_ClassList`) |
| `0x85120023` | `8` | Pack (C->S) | `0x85120014` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x81120003` (`+979624`) [C++: `0x140b92ce8`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85120024` | `9` | Pack (C->S) | `0x8512001B` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81120007` (`+978004`), `0x81120002` (`+979536`) [C++: `0x140b92da2`] | NAT: `0x01670011` (`0x140b31d80`), `0x00E70001` (`0x140821170`) |
| `0x85120026` | `11` | Pack (C->S) | `0x85120025` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81120009` (`+979712`) [C++: `0x140b90d71`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8512002B` | `13` | Unpack (S->C) | `0x8512002B` | `(ref, i32/u32, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x8112000A` (`+978148`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00330001` (`0x140384e80`); PID: `0x10000010` (`Trait_Loremaster_KnowledgeOfThePast`), `0x100000AF` (`CombatMusicControl_ModEntry`), `0x00000517` (`UICore_Dialog_type`) |
| `0x8512002C` | `14` | Pack (C->S) | `0x85120029` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x87510001` (`+1675340`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00A60018` (`0x140393b30`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x100016FC` (`Loot_UI_MasterLootDialog_ItemIID`), `0x100016FB` (`Loot_UI_MasterLootDialog_AssigneeIID`) |
| `0x8512003D` | `15` | Unpack (S->C) | `0x8512003D` | `(ref)` | `0x00D60007(0x06C3)` | `0x81120013` (`+978696`) | NAT: `0x00270006` (`0x1403ae7b0`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`); PID: `0x10004E68` (`UI_Inventory_PendingLoot_Array`), `0x10004E62` (`UI_Inventory_PendingLoot_Info`), `0x10004E64` (`UI_Inventory_PendingLoot_ItemDID`) |
| `0x85120040` | `17` | Pack (C->S) | `0x8512003F` | `()` | `0x00D9000C` (`self`) | `0x81120011` (`+979860`) [C++: `0x140b922b5`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85120043` | `20` | Pack (C->S) | `0x85120042` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x81120014` (`+979908`) [C++: `0x140b92366`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85120044` | `21` | Pack (C->S) | `0x85120041` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x81120015` (`+980020`) [C++: `0x140b92256`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85120054` | `23` | Pack (C->S) | `0x85120053` | `()` | `0x00D9000C` (`self`) | `0x8112001B` (`+980132`) [C++: `0x140b921a5`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0513` (`1299`) — CMEOInventorySystem (Container Start/Stop Viewing & Inventory Commands) <a id="rpc-class-0x0513"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x092E`
- **Method Counts**: `2` Pack (C->S), `2` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 25 total)**: `0x10000AC2` (`Inventory_BindOnAcquire`), `0x1000011E` (`GOP_AcceptKinship`), `0x100001CF` (`Trait_Minstrel_LifeSinger`), `0x00000044` (`ParticleScript_BoneType`), `0x100000B7` (`Combat_BaseDPS`), `0x10000197` (`Skill_UsableWhileMoving`), `0x100006C7` (`CraftRecipe_OneTimeRecipe`), `0x100001F6` (`Induction_ActionDID`), `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`), `0x10000063` (`UI_SkillDisplay_IconSize`), `0x10000104` (`CombatState_Control_NoImmunityStates`), `0x10000156` (`EffectGenerator_EffectData_QuestPhaseRange`), `0x10001514` (`Inventory_DisableCloseOnEmpty`), `0x00000321` (`AnimConfigurableStruct`), `0x10000913` (`Inventory_ImplementTypePrimary`), `0x100000CF` (`Inventory_ImplementTypeSecondary`), `0x10000744` (`Inventory_ImplementTypeRanged`) *(+7 more)*
- **Referenced WLib Strings (`0x0F`, 16 total)**: `"manage"`, `"swapslots"`, `"ERROR! Source bag should be 1-6!"`, `"ERROR! Destination bag should be 1-6!"`, `"!"`, `"ERROR! Slots should be 1-"`, `"dumpslots"`, `"Slot Dump:"`, `"CMEOInventorySystem::RecvCEvt_HandleContainerStartViewingResult"`, `"CMEOInventorySystem::RecvCEvt_HandleContainerStopViewingResult"` *(+6 more)*
- **Referenced Native Functions (`0x15`, 42 total)**: `0x00A90003` (`0x1403a4ca0`), `0x00A9002D` (`0x1403a8a20`), `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x014B0004` (`0x140b336e0`), `0x014B0003` (`0x140b33600`), `0x01810004` (`0x140b2ef90`), `0x01810003` (`0x140b2f8a0`), `0x00B60029` (`0x1403c5260`), `0x00B6002B` (`0x1403c0470`), `0x00B70001` (`0x1403c4ed0`), `0x00B70003` (`0x1403ba520`), `0x00E10016` (`0x1403ba9d0`) *(+28 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 9 total)**: `0x892E0001` @ `0x140b9a705` (fn `0x140b9a686`), `0x892E0002` @ `0x140b90ce1` (fn `0x140b90c80`), `0x892E000A` @ `0x140b4f910` (fn `0x140b4f85f`), `0x892E0006` @ `0x140b4fa9f` (fn `0x140b4f9bf`), `0x892E0005` @ `0x140b60360` (fn `0x140b602af`), `0x892E000F` @ `0x140b604ff` (fn `0x140b6041f`), `0x892E0007` @ `0x140b6f9db` (fn `0x140b6f990`), `0x892E0015` @ `0x140b8eaa2` (fn `0x140b8ea54`), `0x892E0016` @ `0x140b8ed2b` (fn `0x140b8ece0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85130004` | `1` | Unpack (S->C) | `0x85130004` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x892E0003` (`+842064`) | NAT: `0x0030000B` (`0x140385ab0`), `0x014B0001` (`0x140b42e20`); PID: `0x10000104` (`CombatState_Control_NoImmunityStates`), `0x10000156` (`EffectGenerator_EffectData_QuestPhaseRange`), `0x10001514` (`Inventory_DisableCloseOnEmpty`); MSG: `"CMEOInventorySystem::RecvCEvt_HandleContainer"` |
| `0x85130014` | `4` | Unpack (S->C) | `0x85130014` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x892E0008` (`+842272`) | NAT: `0x0030000B` (`0x140385ab0`), `0x014B0002` (`0x140b43060`); PID: `0x10000104` (`CombatState_Control_NoImmunityStates`), `0x000006C3` (`Appearance_ClassList`); MSG: `"CMEOInventorySystem::RecvCEvt_HandleContainer"` |
| `0x85130028` | `10` | Pack (C->S) | `0x85130016` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x892E0001` (`+841676`) [C++: `0x140b9a705`] | — |
| `0x8513002E` | `12` | Pack (C->S) | `0x8513002D` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x892E0002` (`+841704`) [C++: `0x140b90ce1`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000B` (`0x140385ab0`), `0x0068000B` (`0x1403817b0`); PID: `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`), `0x10000063` (`UI_SkillDisplay_IconSize`) |

### Class `0x0514` (`1300`) — MailSystem (Send, Read, Delete, Return, Take Attachment/Money, Admin Mail) <a id="rpc-class-0x0514"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0173`, `0x0627`, `0x0987`
- **Method Counts**: `8` Pack (C->S), `9` Unpack (S->C) (`17` total)
- **Referenced Properties (`pids.txt`, 39 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x10001AFD` (`Antispam_MailRequest`), `0x10002730` (`World_Mail_Global_Lock`), `0x0000031E` (`ExecuteScript_IsScriptDefined_ScriptID`), `0x0000012F` (`EntityVFXScript_AmbientLightBoost_AmbientLight`), `0x00000351` (`Entity_DataID`), `0x10000464` (`Combat_Monster_Aura_Policy`), `0x10000212` (`Stealth_WaitingForAwareHeartbeat`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x100001A9` (`QuestEffect_ApplyRole`), `0x10000396` (`RefereeFilter_Score`), `0x100001CC` (`Skill_Duration_Trait_Guardian_ImprovedParryResponse`), `0x00000400` (`StringInfoRendering_VarType`), `0x100000B4` (`Combat_CanBlock`), `0x1000031F` (`UI_Shortcut_PetCommand`), `0x10000192` (`Skill_Toggle_VitalCostPerSecondList`), `0x00000580` (`StreakInfoScript_TextureScale`), `0x1000001F` (`Resist_Additional_Resistance_Cry`) *(+21 more)*
- **Referenced WLib Strings (`0x0F`, 30 total)**: `"/mail dump\|dumpdelayed\|cleanup\|purgefromsender\|return\|remove\|removeall\|re"`, `"/mail dump <Player Name> - Displays all of the letters currently associated with"`, `"/mail dumpdelayed <Player Name> - Displays all of the currently delayed letters "`, `"/mail cleanup <opened mail duration> <unopened mail duration> - Forces a cleanup"`, `"/mail purgefromsender <Player IID> - Deletes ALL mail in the system sent by the "`, `"/mail return <Player Name> <Mail ID> - Forces the specified letter to be returne"`, `"/mail remove <Player Name> <Mail ID> - Forces the specified letter to be removed"`, `"/mail removeall <Player Name> - Removes(deletes) ALL letters in the given Player"`, `"/mail resenditem <Player Name> <Item IID> - Attempts to send the given Item to t"`, `"/mail send <Player Name> <Subject> <Message> - Sends a letter to the specified p"` *(+20 more)*
- **Referenced Native Functions (`0x15`, 79 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+65 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 11 total)**: `0x81730004` @ `0x140b4ee03` (fn `0x140b4ed5e`), `0x81730005` @ `0x140b4f093` (fn `0x140b4efee`), `0x81730007` @ `0x140b5d593` (fn `0x140b5d4ee`), `0x81730009` @ `0x140b5fa41` (fn `0x140b5fa15`), `0x8173007E` @ `0x140b5f0f3` (fn `0x140b5f04e`), `0x817300A6` @ `0x140b5e6a3` (fn `0x140b5e5fe`), `0x817300C8` @ `0x140b4f1f3` (fn `0x140b4f14e`), `0x817300CF` @ `0x140b4dd24` (fn `0x140b4dc35`), `0x81730003` @ `0x140b4e842` (fn `0x140b4e7ef`), `0x817300D3` @ `0x140b63bec` (fn `0x140b63b25`), `0x817300D1` @ `0x140b85f60` (fn `0x140b85f38`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85140025` | `4` | Unpack (S->C) | `0x85140025` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x89870002` (`+990280`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140026` | `5` | Unpack (S->C) | `0x85140026` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x89870003` (`+990444`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140035` | `6` | Unpack (S->C) | `0x85140035` | `()` | `0x00D60007(0x06C3)` | `0x89870008` (`+990256`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140036` | `7` | Unpack (S->C) | `0x85140036` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x89870001` (`+990476`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8514004C` | `11` | Pack (C->S) | `0x8514003B` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x81730005` (`+993316`) [C++: `0x140b4f093`] | PID: `0x10000464` (`Combat_Monster_Aura_Policy`), `0x10000212` (`Stealth_WaitingForAwareHeartbeat`) |
| `0x8514004D` | `12` | Pack (C->S) | `0x85140048` | `(i32/u32, ref, i32/u32, i32/u32, ref, ref, ref, i64/u64)` | `0x00D9000D` (`local_0`) | `0x81730009` (`+996032`) [C++: `0x140b5fa41`] | PID: `0x100003C4` (`Classification_Control_Entry`) |
| `0x8514004E` | `13` | Pack (C->S) | `0x85140040` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x81730004` (`+993168`) [C++: `0x140b4ee03`] | PID: `0x10000464` (`Combat_Monster_Aura_Policy`), `0x10000212` (`Stealth_WaitingForAwareHeartbeat`) |
| `0x85140050` | `14` | Pack (C->S) | `0x85140012` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x817300C8` (`+993560`) [C++: `0x140b4f1f3`] | PID: `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x100001A9` (`QuestEffect_ApplyRole`) |
| `0x85140051` | `15` | Pack (C->S) | `0x85140015` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x81730007` (`+994244`) [C++: `0x140b5d593`] | PID: `0x10000464` (`Combat_Monster_Aura_Policy`), `0x10000212` (`Stealth_WaitingForAwareHeartbeat`) |
| `0x85140070` | `18` | Pack (C->S) | `0x8514006F` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x8173007E` (`+993812`) [C++: `0x140b5f0f3`] | PID: `0x10000464` (`Combat_Monster_Aura_Policy`), `0x10000212` (`Stealth_WaitingForAwareHeartbeat`) |
| `0x85140075` | `20` | Pack (C->S) | `0x85140074` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86270005` (`+1711728`) | — |
| `0x8514007F` | `21` | Unpack (S->C) | `0x8514007F` | `(ref)` | `0x00D60007(0x06C3)` | `0x89870005` (`+990416`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140081` | `22` | Unpack (S->C) | `0x85140081` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x89870009` (`+990388`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140083` | `23` | Unpack (S->C) | `0x85140083` | `()` | `0x00D60007(0x06C3)` | `0x89870004` (`+990336`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140086` | `24` | Unpack (S->C) | `0x85140086` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x89870007` (`+990308`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140088` | `25` | Unpack (S->C) | `0x85140088` | `(ref)` | `0x00D60007(0x06C3)` | `0x89870006` (`+990360`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85140093` | `27` | Pack (C->S) | `0x85140092` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x817300A6` (`+994392`) [C++: `0x140b5e6a3`] | PID: `0x10000464` (`Combat_Monster_Aura_Policy`), `0x10000212` (`Stealth_WaitingForAwareHeartbeat`) |

### Class `0x0515` (`1301`) — MapNote / Dungeon Map Data System (Dungeon_CurrentDungeon, Dungeon_MapData) <a id="rpc-class-0x0515"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x01B6`
- **Method Counts**: `2` Pack (C->S), `1` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 9 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x100007AC` (`Dungeon_CurrentDungeon`), `0x000001B6` (`UICore_Scrollbar_hide_when_disabled`), `0x100011A0` (`Dungeon_MapData`), `0x10000307` (`Dungeon_ParentDungeon`), `0x10000DFF` (`Dungeon_MiniMapData`), `0x10000983` (`Area_CurrentArea`), `0x100010C8` (`Area_MapData`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 40 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+26 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 2 total)**: `0x81B60002` @ `0x140b5fc22` (fn `0x140b5fb6c`), `0x81B60003` @ `0x140b58e36` (fn `0x140b58d3e`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85150004` | `3` | Unpack (S->C) | `0x85150004` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x81B60007` (`+997436`) | NAT: `0x01490002` (`0x140b40d90`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85150005` | `4` | Pack (C->S) | `0x85150002` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x81B60001` (`+997368`) | — |
| `0x85150006` | `5` | Pack (C->S) | `0x85150001` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x81B60002` (`+997404`) [C++: `0x140b5fc22`] | — |

### Class `0x0518` (`1304`) — Movement / Action Lock System (Movement Action Restriction) <a id="rpc-class-0x0518"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0175`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 5 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x0000055C` (`Slider_ForwardSpeed`), `0x0000041E` (`Slider_StrafeSpeed`), `0x00000245` (`Slider_FlySpeed`), `0x00000201` (`Entity_Element`)
- **Referenced WLib Strings (`0x0F`, 1 total)**: `"Action: Can't move until you have finished current action."`
- **Referenced Native Functions (`0x15`, 41 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+27 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85180007` | `4` | Pack (C->S) | `0x85180004` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x81750002` (`+1163312`), `0x81750003` (`+1163352`), `0x81750004` (`+1163392`), `0x81750005` (`+1163432`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x85180009` | `6` | Pack (C->S) | `0x85180006` | `(ref)` | `0x00D9000C` (`self`) | `0x81750001` (`+1163228`) | NAT: `0x00E70001` (`0x140821170`); MSG: `"Action: Can't move until you have finished cu"` |

### Class `0x0519` (`1305`) — MusicSystem (Player Music Mode, ABC/Note Performance & Quantization) <a id="rpc-class-0x0519"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0116`
- **Method Counts**: `8` Pack (C->S), `8` Unpack (S->C) (`16` total)
- **Referenced Properties (`pids.txt`, 18 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10000335` (`UI_CharacterManagement_UIElement`), `0x100003B0` (`Guild_MaxAccounts`), `0x10001612` (`GOP_PlayerMusicOnClient`), `0x10001616` (`GOP_PlayerMusicQuantize`), `0x10001640` (`Music_Cached_Preference_ClientSide`), `0x1000163F` (`Music_Cached_Preference_Quantize`), `0x10000069` (`UI_Pet_PetFollowButton`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10000BA1` (`Item_Music_Channeling_State`), `0x10000054` (`Death_VitalPercentageData`), `0x10000242` (`UI_Examination_Tooltip_Compare_Element`), `0x100002F6` (`Stealth_Cache_StealthDetectionLevel`), `0x10000078` (`Vital_HealthCombatCurrentRegen`), `0x10000188` (`AI_CombatOnly`), `0x10000FD3` (`Antispam_MusicPlayRequest`), `0x10001D21` (`Antispam_MusicChunkPlayRequest`), `0x10000C90` (`Item_Music_Instrument_Type`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 24 total)**: `0x07DE0001` (`0x140b2f810`), `0x07DE0003` (`0x140b2f970`), `0x00E70001` (`0x140821170`), `0x07DE0006` (`0x140b4d110`), `0x00E70003` (`0x140821030`), `0x01500001` (`0x140b39b20`), `0x01500002` (`0x140b39bb0`), `0x07DE0008` (`0x140b2f280`), `0x07DE0004` (`0x140b4d020`), `0x07DE0005` (`0x140b4d170`), `0x08040003` (`0x140386640`), `0x08040001` (`0x1403ab230`), `0x08040002` (`0x140a85360`), `0x07DE000A` (`0x140b2f9f0`) *(+10 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 6 total)**: `0x81160022` @ `0x140b915d6` (fn `0x140b91560`), `0x81160001` @ `0x140b71eeb` (fn `0x140b71ea0`), `0x81160025` @ `0x140b991a6` (fn `0x140b99130`), `0x8116001A` @ `0x140b9a5a5` (fn `0x140b9a570`), `0x81160028` @ `0x140b9a615` (fn `0x140b9a5e0`), `0x8116001D` @ `0x140b9aca5` (fn `0x140b9ac70`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85190005` | `3` | Unpack (S->C) | `0x85190005` | `()` | `0x00D60007(0x06C3)` | `0x81160006` (`+1179028`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85190008` | `4` | Unpack (S->C) | `0x85190008` | `()` | `0x00D60007(0x06C3)` | `0x81160007` (`+1179052`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8519000A` | `5` | Pack (C->S) | `0x85190001` | `()` | `0x00D9000C` (`self`) | `0x81160009` (`+1177444`) | — |
| `0x8519000B` | `6` | Pack (C->S) | `0x85190004` | `()` | `0x00D9000C` (`self`) | `0x81160009` (`+1177444`) | — |
| `0x8519000F` | `8` | Unpack (S->C) | `0x8519000F` | `(i32/u32, f64, i32/u32, i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8116000E` (`+1179076`) | NAT: `0x00E70001` (`0x140821170`), `0x07DE0004` (`0x140b4d020`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85190011` | `10` | Unpack (S->C) | `0x85190011` | `(i32/u32, f64, i32/u32)` | `0x00D60007(0x06C3)` | `0x8116000C` (`+1179192`) | NAT: `0x00E70001` (`0x140821170`), `0x07DE0005` (`0x140b4d170`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85190013` | `11` | Pack (C->S) | `0x8519000E` | `(i32/u32, f64, i32/u32)` | `0x00D9000C` (`self`) | `0x81160014` (`+1178216`) | NAT: `0x07DE0005` (`0x140b4d170`) |
| `0x85190014` | `12` | Pack (C->S) | `0x85190010` | `(i32/u32, f64, i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x81160015` (`+1177964`) | NAT: `0x07DE0008` (`0x140b2f280`), `0x07DE0004` (`0x140b4d020`); PID: `0x10000335` (`UI_CharacterManagement_UIElement`) |
| `0x85190016` | `13` | Unpack (S->C) | `0x85190016` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x81160024` (`+1179504`) | NAT: `0x00C7001A` (`0x14039b820`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000069` (`UI_Pet_PetFollowButton`) |
| `0x8519001C` | `15` | Unpack (S->C) | `0x8519001C` | `(f64, f64)` | `0x00D60007(0x06C3)` | `0x81160023` (`+1179672`) | NAT: `0x07DE0002` (`0x140b2f8f0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8519001F` | `17` | Pack (C->S) | `0x85190018` | `()` | `0x00D9000C` (`self`) | `0x8116001F` (`+1177664`) | PID: `0x100003B0` (`Guild_MaxAccounts`) |
| `0x85190020` | `18` | Pack (C->S) | `0x8519001E` | `(ref)` | `0x00D9000C` (`self`) | `0x81160022` (`+1180228`) [C++: `0x140b915d6`] | NAT: `0x00E70001` (`0x140821170`), `0x00C7001A` (`0x14039b820`), `0x00C7000C` (`0x1403afdf0`); PID: `0x10000069` (`UI_Pet_PetFollowButton`) |
| `0x85190021` | `19` | Unpack (S->C) | `0x85190021` | `(ref)` | `0x00D60007(0x06C3)` | `0x81160026` (`+1179388`) | NAT: `0x00E70001` (`0x140821170`), `0x07DE000A` (`0x140b2f9f0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85190024` | `21` | Pack (C->S) | `0x85190023` | `(ref)` | `0x00D9000C` (`self`) | `0x81160027` (`+1178300`) | NAT: `0x07DE0008` (`0x140b2f280`), `0x08040003` (`0x140386640`), `0x08040001` (`0x1403ab230`); PID: `0x10000335` (`UI_CharacterManagement_UIElement`) |
| `0x85190027` | `23` | Unpack (S->C) | `0x85190027` | `()` | `0x00D60007(0x06C3)` | `0x8116002A` (`+1179296`) | NAT: `0x00E70001` (`0x140821170`), `0x07DE0006` (`0x140b4d110`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85190028` | `24` | Pack (C->S) | `0x85190025` | `()` | `0x00D9000C` (`self`) | `0x81160029` (`+1178716`) | NAT: `0x07DE0008` (`0x140b2f280`), `0x07DE0006` (`0x140b4d110`) |

### Class `0x051B` (`1307`) — PedigreeSystem (Character Surname, Pedigree & Generation) <a id="rpc-class-0x051b"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0119`, `0x055E`, `0x06E2`
- **Method Counts**: `6` Pack (C->S), `7` Unpack (S->C) (`13` total)
- **Referenced Properties (`pids.txt`, 33 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x100001C9` (`UI_Social_IsInParty`), `0x10000051` (`Emote_GrantedList`), `0x1000006D` (`UI_QuestTree_QuestCategoryElement`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x00000693` (`UICore_Dialog_confirmation_nobuttontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x100000DA` (`Pedigree_Dialog_Requestor`), `0x100002AA` (`Pedigree_Relationship`), `0x00000644` (`UICore_Dialog_topmost`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x10000026` (`Drama_PatrolStart`), `0x100001EE` (`LootGenControl_DropFrequencyTable`), `0x10000190` (`Skill_Resist_Chance_Modifier`), `0x000004B2` (`World_NameRule`) *(+15 more)*
- **Referenced WLib Strings (`0x0F`, 23 total)**: `"openui"`, `"ui"`, `"abandon"`, `"divorce"`, `"dump"`, `"client"`, `"server"`, `"."`, `"Error: Unrecognized argument +"`, `"La La La"` *(+13 more)*
- **Referenced Native Functions (`0x15`, 82 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+68 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x81190002` @ `0x140b997c0` (fn `0x140b99770`), `0x81190003` @ `0x140b99c48` (fn `0x140b99ba0`), `0x81190001` @ `0x140b7ae30` (fn `0x140b7ade0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x851B0005` | `3` | Unpack (S->C) | `0x851B0005` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81190010` (`+1221048`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851B0007` | `4` | Unpack (S->C) | `0x851B0007` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x8119000F` (`+1221016`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851B000C` | `5` | Unpack (S->C) | `0x851B000C` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8119000A` (`+1221220`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000051` (`Emote_GrantedList`), `0x1000006D` (`UI_QuestTree_QuestCategoryElement`) |
| `0x851B0010` | `7` | Unpack (S->C) | `0x851B0010` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8119000B` (`+1221100`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851B001C` | `11` | Unpack (S->C) | `0x851B001C` | `()` | `0x00D60007(0x06C3)` | `0x81190009` (`+1221076`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851B0020` | `12` | Unpack (S->C) | `0x851B0020` | `(ref)` | `0x00D60007(0x06C3)` | `0x81190007` (`+1221128`) | NAT: `0x01530002` (`0x140b48910`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851B0021` | `13` | Pack (C->S) | `0x851B0001` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x81190002` (`+1220516`) [C++: `0x140b997c0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x851B0022` | `14` | Pack (C->S) | `0x851B0016` | `()` | `0x00D9000D` (`local_0`) | `0x855E000D` (`+1224224`) | — |
| `0x851B0023` | `15` | Pack (C->S) | `0x851B000F` | `(i32/u32)` | `0x00D9000D` (`local_0`) | `0x8119000E` (`+1220896`) | — |
| `0x851B0024` | `16` | Pack (C->S) | `0x851B0015` | `(i64/u64, ref)` | `0x00D9000D` (`local_0`) | `0x81190003` (`+1220604`) [C++: `0x140b99c48`] | NAT: `0x00E70001` (`0x140821170`), `0x00C70010` (`0x14038bee0`) |
| `0x851B0025` | `17` | Pack (C->S) | `0x851B0013` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x86E20001` (`+1671348`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00A60018` (`0x140393b30`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x100000DA` (`Pedigree_Dialog_Requestor`), `0x100002AA` (`Pedigree_Relationship`) |
| `0x851B0026` | `18` | Pack (C->S) | `0x851B0004` | `(i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x81190008` (`+1220772`) | — |
| `0x851B002A` | `19` | Unpack (S->C) | `0x851B002A` | `()` | `0x00D60007(0x06C3)` | `0x81190013` (`+1221200`) | NAT: `0x01530004` (`0x140b3fb90`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x051C` (`1308`) — PerkSystem (Destiny Point Perks Purchase & Query) <a id="rpc-class-0x051c"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x011A`
- **Method Counts**: `1` Pack (C->S), `3` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 19 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10000056` (`PrivateEncounterTemplate_SuccessTeleportLocationStruct`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x100002C6` (`PerkDialog_PerkDID`), `0x10000692` (`PerkDialog_VendorIID`), `0x0000055F` (`UICore_Dialog_message_statementtext`), `0x0000011C` (`UICore_Dialog_modal`), `0x00000561` (`UICore_Scrollable_preserve_percentage_on_resize`), `0x00000704` (`Volume_AreaLoad`), `0x100003F0` (`CraftControl_XPRewardEnum`), `0x1000050E` (`UI_Inventory_FullBackpackTab`), `0x100002FE` (`Emote_IconImage`), `0x100002BD` (`CounterBox_CurrentCount`), `0x1000005A` (`RaceTable_StartingLocation`), `0x1000037B` (`Skill_LastDefaultAttackTarget`), `0x100002B2` (`LFP_MaxLevel`) *(+1 more)*
- **Referenced WLib Strings (`0x0F`, 3 total)**: `"usage: /perk [cmd]"`, `"dump: display perk info for player."`, `"addcredit <perkchannel> <creditamount>: add perk credits to the given channel."`
- **Referenced Native Functions (`0x15`, 31 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x01540001` (`0x140b3b870`), `0x00E70001` (`0x140821170`), `0x0030000B` (`0x140385ab0`), `0x01540002` (`0x140b31f00`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00330001` (`0x140384e80`), `0x00A6001F` (`0x1403a9c20`), `0x00A6002B` (`0x1403b0040`), `0x00A60015` (`0x1403a9640`), `0x00A60017` (`0x1403ab3c0`) *(+17 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x811A0001` @ `0x140b622d2` (fn `0x140b62250`), `0x811A0003` @ `0x140b8440f` (fn `0x140b842a0`), `0x811A0002` @ `0x140b84585` (fn `0x140b84490`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x851C0003` | `1` | Unpack (S->C) | `0x851C0003` | `(f64, i32/u32)` | `0x00D60007(0x06C3)` | `0x811A000E` (`+1245340`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851C0006` | `3` | Unpack (S->C) | `0x851C0006` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x811A0005` (`+1245292`) | NAT: `0x01540001` (`0x140b3b870`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851C0007` | `4` | Unpack (S->C) | `0x851C0007` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x811A000A` (`+1245316`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851C000D` | `5` | Pack (C->S) | `0x851C0004` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x811A0006` (`+1245504`) | — |

### Class `0x051D` (`1309`) — CPlayer Virtual Slot 0x0361 State Sync (4-Field Entity/Value Update) <a id="rpc-class-0x051d"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x06C3`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`)**: Only `0x008E`/`0x00D9`/`0x00D6` RPC primitives
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x851D0014` | `1` | Unpack (S->C) | `0x851D0014` | `(i32/u32, f32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C3018D` (`vslot 0x0361`, `+134836`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x051E` (`1310`) — ShortcutSystem (Quickslot Bar Assignment & Shortcut Execution) <a id="rpc-class-0x051e"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0176`, `0x0179`
- **Method Counts**: `11` Pack (C->S), `8` Unpack (S->C) (`19` total)
- **Referenced Properties (`pids.txt`, 60 total)**: `0x0000034F` (`Entity_Selection`), `0x000004B3` (`World_Selection`), `0x10000511` (`UI_Examination_Tooltip_DisplayString`), `0x1000004D` (`TextItem_Title`), `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x10000039` (`GCParam_Properties_List`), `0x1000195C` (`Admin_PlayerTestMode`), `0x000000FD` (`PreprocOptions_FontGen_WantBackgroundImage`), `0x100001B4` (`Inventory_SlotCache_Eq_Weapon_Primary`), `0x100001B3` (`Inventory_SlotCache_Eq_Weapon_Secondary`), `0x1000082A` (`Inventory_SlotCache_Eq_Weapon_Ranged`), `0x10000B87` (`Combat_DamageQualifier_Melee_Offense`), `0x10000C6B` (`Combat_DamageQualifier_Ranged_Offense`), `0x100064F0` (`LoE_Light_Modifier`), `0x10002169` (`Combat_Stats_EvadePoints`), `0x10002185` (`Combat_Class_EvadePoints`), `0x10002140` (`Combat_EvadePoints_Modifier`), `0x10002149` (`Combat_Current_EvadePoints`) *(+42 more)*
- **Referenced WLib Strings (`0x0F`, 9 total)**: `"."`, `"> WARNING: Unable to create color text detection sphere on entity"`, `"ColorTextSystem::SetupColorTextMonitor"`, `".  This shorcut was at absolute index:"`, `"> ERROR: Can't use a shortcut of this type:"`, `"ShortcutSystem::UseShortcutByIndex"`, `"ShortcutSystem::MetaUseShortcutByIndex"`, `"> WARNING: Unable to create travel detection sphere on entity"`, `"CTravelSystem::SetupTravelMonitor"`
- **Referenced Native Functions (`0x15`, 125 total)**: `0x00A90037` (`0x1403ab570`), `0x00E70003` (`0x140821030`), `0x00270006` (`0x1403ae7b0`), `0x0027002B` (`0x1403ab310`), `0x00A9000D` (`0x1403b2260`), `0x0030000B` (`0x140385ab0`), `0x01350007` (`0x140b40f30`), `0x01670001` (`0x140b387d0`), `0x019F000B` (`0x140a8c2a0`), `0x015D0001` (`0x140b3f160`), `0x00750009` (`0x1403ba760`), `0x00C70012` (`0x1403abd30`), `0x00B90002` (`0x1403a5840`), `0x0135000F` (`0x140b3bef0`) *(+111 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 11 total)**: `0x81760009` @ `0x140b52602` (fn `0x140b525b4`), `0x81760001` @ `0x140b588ce` (fn `0x140b58813`), `0x8176000C` @ `0x140b5fd50` (fn `0x140b5fcc6`), `0x8179000B` @ `0x140b66860` (fn `0x140b667d6`), `0x81790004` @ `0x140b8a343` (fn `0x140b8a250`), `0x81790003` @ `0x140b8a4b2` (fn `0x140b8a3d0`), `0x81790009` @ `0x140b8a7d6` (fn `0x140b8a690`), `0x81790007` @ `0x140b90ea1` (fn `0x140b90e30`), `0x81790008` @ `0x140b99b57` (fn `0x140b99ac0`), `0x81790005` @ `0x140b9c7a5` (fn `0x140b9c730`), `0x81790006` @ `0x140b9c846` (fn `0x140b9c7d0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x851E0003` | `1` | Unpack (S->C) | `0x851E0003` | `()` | `0x00D60007(0x06C3)` | `0x86C300E7` (`vslot 0x02F3`, `+94872`) | NAT: `0x01670006` (`0x140b35360`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851E0007` | `5` | Unpack (S->C) | `0x851E0007` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C30010` (`vslot 0x024D`, `+94840`) | NAT: `0x01670005` (`0x140b351b0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851E000F` | `7` | Unpack (S->C) | `0x851E000F` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x86C30071` (`vslot 0x0299`, `+93432`) | NAT: `0x01670001` (`0x140b387d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851E0012` | `9` | Unpack (S->C) | `0x851E0012` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x86C300AD` (`vslot 0x02C6`, `+94804`) | NAT: `0x01670009` (`0x140b3a890`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851E0013` | `10` | Unpack (S->C) | `0x851E0013` | `(ref, ref, ref, ref, ref, ref, ref, ref, ref, ref, ref, ref, ref)` | `0x00D60007(0x06C3)` | `0x86C30047` (`vslot 0x0274`, `+93720`) | NAT: `0x00750009` (`0x1403ba760`), `0x00C70012` (`0x1403abd30`), `0x00B90002` (`0x1403a5840`); PID: `0x10000511` (`UI_Examination_Tooltip_DisplayString`), `0x000006C3` (`Appearance_ClassList`) |
| `0x851E0017` | `12` | Unpack (S->C) | `0x851E0017` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86C300C6` (`vslot 0x02D7`, `+131228`) | NAT: `0x00CD0006` (`0x1403a2e60`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000039` (`GCParam_Properties_List`), `0x000006C3` (`Appearance_ClassList`) |
| `0x851E0022` | `16` | Pack (C->S) | `0x851E0020` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x81790011` (`+1490244`) | — |
| `0x851E0024` | `18` | Pack (C->S) | `0x851E0019` | `(i32/u32, ref, ref)` | `0x00D9000C` (`self`) | `0x86C3007A` (`+110404`), `0x81760011` (`+1189664`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); PID: `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x10000039` (`GCParam_Properties_List`) |
| `0x851E0025` | `19` | Pack (C->S) | `0x851E0006` | `()` | `0x00D9000C` (`self`) | `0x86C3005C` (`+95624`) | — |
| `0x851E0026` | `20` | Pack (C->S) | `0x851E0004` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x86C30006` (`+108324`), `0x86C30006` (`+108324`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00C7000C` (`0x1403afdf0`); PID: `0x1000004D` (`TextItem_Title`), `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`) |
| `0x851E0027` | `21` | Pack (C->S) | `0x851E000B` | `()` | `0x00D9000C` (`self`) | `0x86C30016` (`+101204`) | — |
| `0x851E0028` | `22` | Pack (C->S) | `0x851E001E` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x86C3004A` (`+86568`) | NAT: `0x00E70003` (`0x140821030`), `0x00270006` (`0x1403ae7b0`), `0x0027002B` (`0x1403ab310`); PID: `0x0000034F` (`Entity_Selection`), `0x000004B3` (`World_Selection`) |
| `0x851E0029` | `23` | Pack (C->S) | `0x851E0005` | `(ref, ref)` | `0x00D9000C` (`self`) | `0x86C300B3` (`+126892`) | NAT: `0x019F001A` (`0x140a6e010`) |
| `0x851E002A` | `24` | Pack (C->S) | `0x851E0016` | `(ref)` | `0x00D9000C` (`self`) | `0x86C3015E` (`+127056`) | — |
| `0x851E002C` | `25` | Unpack (S->C) | `0x851E002C` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C300FB` (`vslot 0x0303`, `+93460`) | NAT: `0x019F000B` (`0x140a8c2a0`), `0x015D0001` (`0x140b3f160`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851E0030` | `27` | Pack (C->S) | `0x851E002F` | `()` | `0x00D9000C` (`self`) | `0x86C300C2` (`+95984`) | — |
| `0x851E0032` | `29` | Pack (C->S) | `0x851E0031` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x86C3011B` (`+132080`), `0x86C3011B` (`+132080`) | NAT: `0x00C70019` (`0x14039dba0`), `0x00C7001C` (`0x1403a5220`), `0x00C7001A` (`0x14039b820`); PID: `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`) |
| `0x851E0038` | `31` | Pack (C->S) | `0x851E0037` | `()` | `0x00D9000C` (`self`) | `0x86C30142` (`+122764`) | — |
| `0x851E004A` | `32` | Unpack (S->C) | `0x851E004A` | `(i64/u64, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C301A6` (`vslot 0x0375`, `+94892`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00B80004` (`0x1403abbd0`), `0x00B90002` (`0x1403a5840`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x051F` (`1311`) — PrivateEncounterSystem (Instance/Skirmish/Session Encounter Control) <a id="rpc-class-0x051f"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x011C`, `0x01C1`, `0x06E9`
- **Method Counts**: `5` Pack (C->S), `18` Unpack (S->C) (`23` total)
- **Referenced Properties (`pids.txt`, 40 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x100002E8` (`ActivationCallback::SourceEntity`), `0x000006C3` (`Appearance_ClassList`), `0x100007AB` (`PrivateEncounter_Template_DataID`), `0x10005001` (`PrivateEncounter_PreviousEncounterTemplate`), `0x10005003` (`PrivateEncounter_IntroVO`), `0x10002571` (`WE_Ettenmoors_HP_Monster_North`), `0x10000003` (`MiniGame.FreezeTag@Player.m_bIsPlaying`), `0x10000034` (`NPC_ColorText`), `0x00000517` (`UICore_Dialog_type`), `0x0000055F` (`UICore_Dialog_message_statementtext`), `0x0000033F` (`UICore_Dialog_message_okbuttontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x0000033E` (`UICore_Dialog_message_duration`), `0x10000B37` (`PrivateEncounter_TravelArray`), `0x1000129D` (`PrivateEncounter_Template`), `0x10000A45` (`QuestDispenser_PrivateEncounterTemplate`), `0x10000049` (`CraftProfession_Recipe`) *(+22 more)*
- **Referenced WLib Strings (`0x0F`, 16 total)**: `"> ERROR: INVALID_DID passed in."`, `"PrivateEncounterSystem::GetEncounterName"`, `"."`, `"> ERROR: Could not find private encounter template for encounter"`, `"> ERROR: failed to retrieve encounter template"`, `"PrivateEncounterSystem::IsPlayerInMPBattleEncounter"`, `"PrivateEncounterSystem::IsPlayerInCinematicEncounter"`, `"usage: /pe <option>"`, `"usage: /pe <option> \n Execute private encounter operations from command line.\n"`, `"PrivateEncounterSystem::GetMaxPlayers passed invalid template"` *(+6 more)*
- **Referenced Native Functions (`0x15`, 93 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+79 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 9 total)**: `0x811C0002` @ `0x140b9b48d` (fn `0x140b9b3e0`), `0x811C0017` @ `0x140b90b55` (fn `0x140b90b20`), `0x811C0015` @ `0x140b67121` (fn `0x140b670b0`), `0x811C000F` @ `0x140b7a5da` (fn `0x140b7a5b0`), `0x811C001B` @ `0x140b81bfe` (fn `0x140b81b20`), `0x811C0022` @ `0x140b81efa` (fn `0x140b81df0`), `0x811C0003` @ `0x140b865c1` (fn `0x140b864ec`), `0x811C0001` @ `0x140b8c7ae` (fn `0x140b8c6cd`), `0x811C0013` @ `0x140b90bc5` (fn `0x140b90b90`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x851F0003` | `1` | Unpack (S->C) | `0x851F0003` | `()` | `0x00D60007(0x06C3)` | `0x81C10051` (`+1212592`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0009` | `2` | Unpack (S->C) | `0x851F0009` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x81C1005A` (`+1212532`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F000C` | `3` | Unpack (S->C) | `0x851F000C` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10058` (`+1212372`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F000F` | `4` | Unpack (S->C) | `0x851F000F` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81C10057` (`+1212564`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0027` | `6` | Unpack (S->C) | `0x851F0027` | `()` | `0x00D60007(0x06C3)` | `0x81C10052` (`+1212424`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0028` | `7` | Unpack (S->C) | `0x851F0028` | `()` | `0x00D60007(0x06C3)` | `0x81C10050` (`+1212508`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F003F` | `10` | Unpack (S->C) | `0x851F003F` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C1004E` (`+1212288`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0050` | `11` | Unpack (S->C) | `0x851F0050` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10054` (`+1212480`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0057` | `12` | Unpack (S->C) | `0x851F0057` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10059` (`+1212260`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00A8` | `17` | Unpack (S->C) | `0x851F00A8` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10053` (`+1212232`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00B4` | `18` | Unpack (S->C) | `0x851F00B4` | `(f32)` | `0x00D60007(0x06C3)` | `0x81C10056` (`+1212344`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00BC` | `22` | Pack (C->S) | `0x851F0085` | `()` | `0x00D9000C` (`self`) | `0x86E90001` (`+1675044`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x851F00BF` | `24` | Pack (C->S) | `0x851F002C` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x811C0002` (`+1209508`) [C++: `0x140b9b48d`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x851F00CD` | `26` | Unpack (S->C) | `0x851F00CD` | `(ref)` | `0x00D60007(0x06C3)` | `0x811C000B` (`+1209900`) | NAT: `0x0157000C` (`0x140b31140`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00CF` | `27` | Unpack (S->C) | `0x851F00CF` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811C000C` (`+1209868`) | NAT: `0x0157000D` (`0x140b3c360`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00D8` | `28` | Unpack (S->C) | `0x851F00D8` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81C10065` (`+1212616`) | NAT: `0x0157000E` (`0x140b352c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00D9` | `29` | Unpack (S->C) | `0x851F00D9` | `()` | `0x00D60007(0x06C3)` | `0x81C10064` (`+1212644`) | NAT: `0x0157000F` (`0x140b38240`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00F6` | `32` | Unpack (S->C) | `0x851F00F6` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10073` (`+1212668`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F00F9` | `33` | Unpack (S->C) | `0x851F00F9` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10076` (`+1212696`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0137` | `37` | Pack (C->S) | `0x851F0136` | `()` | `0x00D9000C` (`self`) | `0x811C0014` (`+1210488`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x851F013E` | `38` | Unpack (S->C) | `0x851F013E` | `(ref)` | `0x00D60007(0x06C3)` | `0x81C10096` (`+1212316`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x851F0163` | `40` | Pack (C->S) | `0x851F0162` | `()` | `0x00D9000C` (`self`) | `0x811C0017` (`+1210660`) [C++: `0x140b90b55`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x851F016D` | `42` | Pack (C->S) | `0x851F016C` | `()` | `0x00D9000C` (`self`) | `0x811C0019` (`+1210784`) | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0520` (`1312`) — Induction / Channeling System (Skill Induction & Channeling Interrupt Sync) <a id="rpc-class-0x0520"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x01FE` | **Paired Client Implementation Class(es)**: `0x011D`
- **Method Counts**: `0` Pack (C->S), `3` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 5 total)**: `0x000001FE` (`Movement_SecondsBetweenVisualizations`), `0x10000091` (`Effect_Lootgen_EffectLevelPolicy`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 11 total)**: `0x00E70001` (`0x140821170`), `0x01580001` (`0x140b3cfb0`), `0x01580002` (`0x140b3d090`), `0x01580003` (`0x140b3d130`), `0x00A6001F` (`0x1403a9c20`), `0x00A6002B` (`0x1403b0040`), `0x00A60015` (`0x1403a9640`), `0x00070001` (`0x140822020`), `0x00270006` (`0x1403ae7b0`), `0x00270025` (`0x1403a9500`), `0x00070002` (`0x140821f90`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x811D0001` @ `0x140b90f16` (fn `0x140b90f01`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85200002` | `1` | Unpack (S->C) | `0x85200002` | `(f32, f64)` | `0x00D60007(0x01FE)` | `0x811D0008` (`+1257236`) | PID: `0x000001FE` (`Movement_SecondsBetweenVisualizations`) |
| `0x85200004` | `2` | Unpack (S->C) | `0x85200004` | `(i32/u32, ref)` | `0x00D60007(0x01FE)` | `0x811D0007` (`+1257024`) | PID: `0x000001FE` (`Movement_SecondsBetweenVisualizations`) |
| `0x85200007` | `3` | Unpack (S->C) | `0x85200007` | `(ref)` | `0x00D60007(0x01FE)` | `0x811D0009` (`+1256816`) | PID: `0x000001FE` (`Movement_SecondsBetweenVisualizations`) |

### Class `0x0521` (`1313`) — CQuestSystem / CQuestJournal (Quest Bestowal, Advance, Complete, Cancel, Tracker) <a id="rpc-class-0x0521"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2`, `0x06C3` | **Paired Client Implementation Class(es)**: `0x011F`, `0x06D9`, `0x0C0E`
- **Method Counts**: `14` Pack (C->S), `36` Unpack (S->C) (`50` total)
- **Referenced Properties (`pids.txt`, 209 total)**: `0x000006C3` (`Appearance_ClassList`), `0x000006C2` (`MapNote_StringInfo`), `0x10000182` (`AI_RemoteDetector_Activated`), `0x10000188` (`AI_CombatOnly`), `0x10000183` (`AI_IgnoreHealthSpeedModifier`), `0x10000184` (`AI_CallForHelp_CanRespond`), `0x10000185` (`Signals.MakeDecision@NPC::CombatMove::Target`), `0x10000187` (`Signals@NPC::SkillError::Error`), `0x10000186` (`Signals@NPC::Trigger::Name`), `0x1000018E` (`NPC_PlayingStartingIdle`), `0x1000018F` (`Aggro@NPC::CurrentTarget`), `0x10000190` (`Skill_Resist_Chance_Modifier`), `0x10000192` (`Skill_Toggle_VitalCostPerSecondList`), `0x1000018A` (`AIAction_Skill`), `0x10000189` (`AI_ActionTable_MaxLevel`), `0x1000018B` (`AIAction_DisallowedHints`), `0x1000018C` (`Behavior@NPC::Patrol::Velocity`), `0x1000018D` (`NPC_ActionPicker_QuestScript`) *(+191 more)*
- **Referenced WLib Strings (`0x0F`, 111 total)**: `"]"`, `"[DataID"`, `"<Unknown value>"`, `"["`, `"."`, `"> WARNING: Unable to create quest journal detection sphere on entity"`, `"CQuestJournal::SetupQuestJournalMonitor"`, `"Quest Scripts status for"`, `"Unable to find NPC:"`, `"Not tracking NPC:"` *(+101 more)*
- **Referenced Native Functions (`0x15`, 219 total)**: `0x00A9001F` (`0x140393e20`), `0x00DD000F` (`0x1403b3090`), `0x00DD0001` (`0x1403b6150`), `0x00DD0008` (`0x1403b5b70`), `0x00420001` (`0x140397330`), `0x01590007` (`0x140b485d0`), `0x00E70001` (`0x140821170`), `0x0011000E` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x00230001` (`0x1403b6580`), `0x0023000A` (`0x1403bf2a0`), `0x00240006` (`0x1403c50c0`), `0x00240002` (`0x1403b9f80`), `0x00190006` (`0x1403c2c90`) *(+205 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 77 total)**: `0x811F0002` @ `0x140b99ed0` (fn `0x140b99e80`), `0x811F0003` @ `0x140b61e10` (fn `0x140b61d60`), `0x811F001E` @ `0x140b99295` (fn `0x140b991d0`), `0x811F0024` @ `0x140b90c50` (fn `0x140b90c00`), `0x811F0025` @ `0x140b9b780` (fn `0x140b9b730`), `0x811F0027` @ `0x140b923c5` (fn `0x140b92390`), `0x811F0080` @ `0x140b9a227` (fn `0x140b9a1b0`), `0x811F00C6` @ `0x140b65860` (fn `0x140b65810`), `0x811F00CE` @ `0x140b4e111` (fn `0x140b4e060`), `0x811F0001` @ `0x140b60b12` (fn `0x140b60aa0`), `0x811F000D` @ `0x140b616db` (fn `0x140b61690`), `0x811F001C` @ `0x140b617cd` (fn `0x140b61720`) *(+65 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8521000E` | `2` | Unpack (S->C) | `0x8521000E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x811F0028` (`+1327020`) | NAT: `0x01590009` (`0x140b30490`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210010` | `3` | Unpack (S->C) | `0x85210010` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0040` (`+1316160`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8521001F` | `5` | Unpack (S->C) | `0x8521001F` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0036` (`+1316264`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210020` | `6` | Unpack (S->C) | `0x85210020` | `(i32/u32)` | `0x00D60007(0x06C2)` | `0x811F003D` (`+1323952`) | PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x85210027` | `7` | Unpack (S->C) | `0x85210027` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x811F0029` (`+1327048`) | NAT: `0x0159000A` (`0x140b3f440`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210041` | `10` | Unpack (S->C) | `0x85210041` | `(i64/u64, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0033` (`+1327388`) | NAT: `0x00330001` (`0x140384e80`), `0x0030000B` (`0x140385ab0`), `0x00C7000C` (`0x1403afdf0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000041` (`GOP_StaticPlacedTooltips`), `0x00000517` (`UICore_Dialog_type`) |
| `0x85210047` | `11` | Unpack (S->C) | `0x85210047` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0030` (`+1316316`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210051` | `13` | Unpack (S->C) | `0x85210051` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F003A` (`+1327232`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210052` | `14` | Unpack (S->C) | `0x85210052` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F002F` (`+1316624`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210053` | `15` | Unpack (S->C) | `0x85210053` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F002E` (`+1316368`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8521005C` | `17` | Unpack (S->C) | `0x8521005C` | `(ref, ref, ref, ref, ref, ref)` | `0x00D60007(0x06C3)` | `0x811F003E` (`+1315472`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8521005E` | `18` | Unpack (S->C) | `0x8521005E` | `()` | `0x00D60007(0x06C3)` | `0x811F0038` (`+1327780`) | NAT: `0x00070003` (`0x140821f30`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210063` | `19` | Unpack (S->C) | `0x85210063` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0034` (`+1316580`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210067` | `20` | Unpack (S->C) | `0x85210067` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0037` (`+1316212`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210075` | `22` | Unpack (S->C) | `0x85210075` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F002A` (`+1327284`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210083` | `24` | Pack (C->S) | `0x85210013` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x811F002D` (`+1328508`) | — |
| `0x85210084` | `25` | Pack (C->S) | `0x8521006B` | `(i32/u32, i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x811F0003` (`+1320604`) [C++: `0x140b61e10`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85210085` | `26` | Pack (C->S) | `0x8521002F` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x811F0032` (`+1328536`) | — |
| `0x85210086` | `27` | Pack (C->S) | `0x85210082` | `()` | `0x00D9000C` (`self`) | `0x811F0027` (`+1328336`) [C++: `0x140b923c5`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85210087` | `28` | Pack (C->S) | `0x8521004B` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x811F0002` (`+1316092`) [C++: `0x140b99ed0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85210088` | `29` | Pack (C->S) | `0x8521003D` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x811F0042` (`+1316132`) | — |
| `0x85210089` | `30` | Pack (C->S) | `0x85210057` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x811F0025` (`+1328248`) [C++: `0x140b9b780`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8521008A` | `31` | Pack (C->S) | `0x85210008` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x811F0024` (`+1328208`) [C++: `0x140b90c50`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85210094` | `32` | Unpack (S->C) | `0x85210094` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x811F004A` (`+1316416`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8521009C` | `33` | Unpack (S->C) | `0x8521009C` | `()` | `0x00D60007(0x06C3)` | `0x811F0053` (`+1316460`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100AB` | `34` | Unpack (S->C) | `0x852100AB` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x811F0055` (`+1327168`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100B2` | `36` | Pack (C->S) | `0x852100B1` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x811F001E` (`+1325584`) [C++: `0x140b99295`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x852100B5` | `37` | Unpack (S->C) | `0x852100B5` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x811F005A` (`+1327100`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100BD` | `38` | Unpack (S->C) | `0x852100BD` | `()` | `0x00D60007(0x06C2)` | `0x811F0060` (`+1324904`) | NAT: `0x00750009` (`0x1403ba760`), `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`); PID: `0x000006C2` (`MapNote_StringInfo`), `0x100000A6` (`Skill_AttackHookInfo`); MSG: `"You are not currently in a fellowship"`, `"You have no data on your fellow's quests"` |
| `0x852100C1` | `39` | Unpack (S->C) | `0x852100C1` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0062` (`+1316500`) | NAT: `0x01590049` (`0x140b3d480`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100CA` | `40` | Unpack (S->C) | `0x852100CA` | `(i64/u64, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F0069` (`+1329120`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00B80004` (`0x1403abbd0`), `0x00B9000D` (`0x1403a59c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100CE` | `41` | Unpack (S->C) | `0x852100CE` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F007A` (`+1329828`) | PID: `0x000006C3` (`Appearance_ClassList`), `0x0000007A` (`PEDesc_EmissionDistance`) |
| `0x852100CF` | `42` | Unpack (S->C) | `0x852100CF` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F007B` (`+1329896`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100D2` | `43` | Unpack (S->C) | `0x852100D2` | `(ref)` | `0x00D60007(0x06C3)` | `0x811F007C` (`+1329616`) | NAT: `0x0023000A` (`0x1403bf2a0`), `0x00240006` (`0x1403c50c0`), `0x00240002` (`0x1403b9f80`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100D5` | `44` | Unpack (S->C) | `0x852100D5` | `()` | `0x00D60007(0x06C2)` | `0x811F007F` (`+1324020`) | PID: `0x000006C2` (`MapNote_StringInfo`), `0x10000E6B` (`Quest_TrackerArray`), `0x10000291` (`Quest_TrackerArrayLocked`) |
| `0x852100D7` | `46` | Pack (C->S) | `0x852100D6` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x811F0080` (`+1328384`) [C++: `0x140b9a227`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x852100DF` | `47` | Unpack (S->C) | `0x852100DF` | `()` | `0x00D60007(0x06C3)` | `0x811F008D` (`+1327076`) | NAT: `0x0159002F` (`0x140b3d1c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100F6` | `48` | Pack (C->S) | `0x852100F7` | `(i64/u64)` | `0x00D90003` (`local_0`) | `0x86D90062` (`+1307240`) | — |
| `0x852100F7` | `49` | Unpack (S->C) | `0x852100F7` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x811F00AE` (`+1328624`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100F8` | `50` | Unpack (S->C) | `0x852100F8` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x811F00B0` (`+1328672`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100F9` | `51` | Pack (C->S) | `0x852100F8` | `(i64/u64)` | `0x00D90003` (`local_0`) | `0x86D90093` (`+1304464`), `0x86D900A7` (`+1304668`) | NAT: `0x006F0014` (`0x1403c6500`), `0x006F0027` (`0x140381e40`), `0x01590005` (`0x140b48700`) |
| `0x852100FA` | `52` | Unpack (S->C) | `0x852100FA` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8C0E000C` (`+1361352`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852100FB` | `53` | Pack (C->S) | `0x852100FA` | `(i64/u64)` | `0x00D90003` (`local_0`) | `0x8C0E000E` (`+1361936`) | NAT: `0x00CC0003` (`0x1403c1d20`), `0x01590044` (`0x140b3d2f0`), `0x007A0015` (`0x1403c49a0`); PID: `0x10003B0B` (`QuestCategoryLimiter_ResetTime`) |
| `0x852100FC` | `54` | Unpack (S->C) | `0x852100FC` | `()` | `0x00D60007(0x06C3)` | `0x8C0E000F` (`+1361376`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210103` | `55` | Unpack (S->C) | `0x85210103` | `(ref)` | `0x00D60007(0x06C3)` | `0x811F00BC` (`+1329948`) | NAT: `0x01590046` (`0x140b3c1f0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210106` | `56` | Unpack (S->C) | `0x85210106` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x811F00BD` (`+1330008`) | NAT: `0x01590046` (`0x140b3c1f0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000105` (`LockBlocksInPerforce`) |
| `0x85210112` | `58` | Pack (C->S) | `0x85210111` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x811F00C6` (`+1330872`) [C++: `0x140b65860`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85210115` | `59` | Unpack (S->C) | `0x85210115` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x811F00C8` (`+1330148`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210118` | `60` | Unpack (S->C) | `0x85210118` | `(ref)` | `0x00D60007(0x06C3)` | `0x811F00C7` (`+1330084`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85210126` | `61` | Unpack (S->C) | `0x85210126` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x811F00E9` (`+1327336`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0522` (`1314`) — QuestRoleData / NPC Quest Role Inspection System (DoCmdGetQuestRoleData) <a id="rpc-class-0x0522"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x011E`
- **Method Counts**: `2` Pack (C->S), `2` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 7 total)**: `0x00000126` (`WeenieType`), `0x000006C3` (`Appearance_ClassList`), `0x10000BDC` (`QuestDispenser_Action`), `0x1000055D` (`Quest_CanBestowInsideInstance`), `0x100000BC` (`QuestTreasure_SelectedItem`), `0x100001CF` (`Trait_Minstrel_LifeSinger`), `0x100001A9` (`QuestEffect_ApplyRole`)
- **Referenced WLib Strings (`0x0F`, 6 total)**: `"You have not selected an agent."`, `"CAdmin::DoCmdGetQuestRoleData> WARNING: Player did not select an agent."`, `"> ERROR: Entity could not be located."`, `"CAdmin::DoCmdGetQuestRoleData> ERROR: Entity could not be located."`, `"WARNING: You have not selected a valid Agent."`, `"CAdmin::DoCmdGetQuestRoleData> WARNING: Player did not select a valid Agent."`
- **Referenced Native Functions (`0x15`, 11 total)**: `0x0039001C` (`0x1403aa5e0`), `0x00DD0001` (`0x1403b6150`), `0x00E70003` (`0x140821030`), `0x0035000A` (`0x140384c30`), `0x013D0001` (`0x140b32db0`), `0x00A6000A` (`0x140395ca0`), `0x00270021` (`0x140392a50`), `0x00A60016` (`0x140392220`), `0x015A0001` (`0x140b345c0`), `0x015A0002` (`0x140b3b970`), `0x00E70001` (`0x140821170`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x811E0001` @ `0x140b67740` (fn `0x140b67680`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85220002` | `1` | Unpack (S->C) | `0x85220002` | `(ref, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x811E0002` (`+1447232`) | NAT: `0x015A0002` (`0x140b3b970`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8522000A` | `4` | Unpack (S->C) | `0x8522000A` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x811E0003` (`+1447200`) | NAT: `0x015A0001` (`0x140b345c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8522000B` | `5` | Pack (C->S) | `0x85220006` | `(ref, i32/u32, i64/u64)` | `0x00D9000D` (`local_0`) | `0x811E0001` (`+1447268`) [C++: `0x140b67740`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8522000C` | `6` | Pack (C->S) | `0x85220005` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C20058` (`+69668`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x00E70003` (`0x140821030`); MSG: `"You have not selected an agent."`, `"CAdmin::DoCmdGetQuestRoleData> WARNING: Playe"` |

### Class `0x0523` (`1315`) — FellowshipSystem (Create, Invite, Dismiss, Leave, Leader, Loot Rules) <a id="rpc-class-0x0523"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0123`
- **Method Counts**: `8` Pack (C->S), `6` Unpack (S->C) (`14` total)
- **Referenced Properties (`pids.txt`, 54 total)**: `0x10001C6F` (`Fellowship_DesiredConjunctionSkill_DataID`), `0x10000271` (`NPC_ActionPicker_SelectionScript`), `0x1000018D` (`NPC_ActionPicker_QuestScript`), `0x10000065` (`UI_Vendor_FilterType`), `0x1000028C` (`QuestEvent_TappedDramaName`), `0x10000017` (`GRSEvent_Death_MonsterDID`), `0x10000090` (`Effect_InstantVital_Multiplicative`), `0x100001C4` (`RandomBox@Entity.m_bRepeatsAllowed`), `0x10000565` (`Signals@Usage::User`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`), `0x10000451` (`NPC_ActionPicker_MPMissionControllerString`), `0x1000028B` (`QuestEvent_Skill_AttackResult`), `0x1000056B` (`Buyback_BuybackID`), `0x10000085` (`ErrorCategory_Script`), `0x10000015` (`GRSEvent_Crafting_IsCritical`), `0x1000001F` (`Resist_Additional_Resistance_Cry`), `0x0000005D` (`Server_World_DefaultMaxCharactersForType`), `0x0000005C` (`APC.ExceptGroups`) *(+36 more)*
- **Referenced WLib Strings (`0x0F`, 8 total)**: `"usage: /fellowship [cmd]"`, `"[disband]\n  [leave]\n  [recruit [selected player\|fellow iid]]\n  [dismiss [sel"`, `"Player is in a raid."`, `"Player is in a fellowship."`, `"Player reports still in fellowship; purging..."`, `"Player reports not in fellowship"`, `"Requesting self-autofellow. How did this happen?"`, `"FellowshipSystem::AddAutoFellow"`
- **Referenced Native Functions (`0x15`, 89 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x013F0001` (`0x140b47380`), `0x013F0004` (`0x140b32b10`), `0x013F000A` (`0x140b47a20`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00E70001` (`0x140821170`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`), `0x00B60006` (`0x1403c2c90`), `0x00B70001` (`0x1403c4ed0`) *(+75 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 12 total)**: `0x81230001` @ `0x140b66e55` (fn `0x140b66dd6`), `0x81230002` @ `0x140b66f75` (fn `0x140b66ef6`), `0x81230007` @ `0x140b66c95` (fn `0x140b66c60`), `0x81230021` @ `0x140b79f01` (fn `0x140b79ea0`), `0x81230008` @ `0x140b66d68` (fn `0x140b66cd0`), `0x81230009` @ `0x140b67078` (fn `0x140b66fe0`), `0x8123000A` @ `0x140b75146` (fn `0x140b75070`), `0x81230025` @ `0x140b8e54c` (fn `0x140b8e4f0`), `0x81230003` @ `0x140b8e9d0` (fn `0x140b8e9a0`), `0x81230005` @ `0x140b8fd9c` (fn `0x140b8fd40`), `0x81230004` @ `0x140b8fe3c` (fn `0x140b8fde0`), `0x81230023` @ `0x140b99da5` (fn `0x140b99d70`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85230006` | `1` | Unpack (S->C) | `0x85230006` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x81230012` (`+710868`) | NAT: `0x015C0003` (`0x140b48840`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85230009` | `3` | Unpack (S->C) | `0x85230009` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x81230011` (`+711096`) | NAT: `0x015C0004` (`0x140b30710`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85230013` | `5` | Unpack (S->C) | `0x85230013` | `(ref)` | `0x00D60007(0x06C3)` | `0x8123000B` (`+710940`) | NAT: `0x015C0001` (`0x140b48790`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8523001C` | `9` | Unpack (S->C) | `0x8523001C` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x81230010` (`+711196`) | NAT: `0x015C0005` (`0x140b3f950`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85230023` | `11` | Pack (C->S) | `0x85230010` | `()` | `0x00D9000C` (`self`) | `0x81230007` (`+710416`) [C++: `0x140b66c95`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000066` (`UI_Pet_PetCommandIconArray`) |
| `0x85230024` | `12` | Pack (C->S) | `0x85230022` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81230001` (`+709924`) [C++: `0x140b66e55`] | — |
| `0x85230025` | `13` | Pack (C->S) | `0x85230016` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x81230014` (`+711516`) | PID: `0x100001C4` (`RandomBox@Entity.m_bRepeatsAllowed`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`), `0x10000066` (`UI_Pet_PetCommandIconArray`) |
| `0x85230026` | `14` | Pack (C->S) | `0x8523001A` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81230002` (`+709968`) [C++: `0x140b66f75`] | — |
| `0x85230027` | `15` | Pack (C->S) | `0x85230008` | `()` | `0x00D9000C` (`self`) | `0x8123000E` (`+708744`) | NAT: `0x00B6002B` (`0x1403c0470`), `0x00B6002D` (`0x1403c1a60`), `0x00B70001` (`0x1403c4ed0`); PID: `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x10000565` (`Signals@Usage::User`) |
| `0x85230028` | `16` | Pack (C->S) | `0x85230017` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x81230013` (`+711296`) | PID: `0x100001C4` (`RandomBox@Entity.m_bRepeatsAllowed`), `0x10000016` (`GRSEvent_MonsterKill_DeathBlow`), `0x10000066` (`UI_Pet_PetCommandIconArray`) |
| `0x8523003B` | `18` | Unpack (S->C) | `0x8523003B` | `(ref)` | `0x00D60007(0x06C3)` | `0x8123001E` (`+712116`) | NAT: `0x074A0001` (`0x140b3c250`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000065` (`UI_Vendor_FilterType`), `0x000006C3` (`Appearance_ClassList`) |
| `0x8523003E` | `19` | Pack (C->S) | `0x85230039` | `(ref)` | `0x00D9000C` (`self`) | `0x8123001D` (`+711736`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); PID: `0x10000065` (`UI_Vendor_FilterType`) |
| `0x85230040` | `20` | Unpack (S->C) | `0x85230040` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81230022` (`+712328`) | NAT: `0x015C0008` (`0x140b3cb10`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85230046` | `22` | Pack (C->S) | `0x85230045` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81230021` (`+712228`) [C++: `0x140b79f01`] | NAT: `0x00E70001` (`0x140821170`), `0x00E70002` (`0x1408211d0`) |

### Class `0x0524` (`1316`) — RecallSystem (Milestone bind, Return/Travel Link & Recall Confirmation) <a id="rpc-class-0x0524"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0628`, `0x06DB`, `0x0C47`
- **Method Counts**: `4` Pack (C->S), `4` Unpack (S->C) (`8` total)
- **Referenced Properties (`pids.txt`, 34 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x10000A85` (`Milestone_TravelLink`), `0x000001EB` (`MapNote_Discoverable`), `0x10000A84` (`Milestone_Bind_Confirmation_Text`), `0x0000033B` (`Render_AdjustableScale`), `0x000006C3` (`Appearance_ClassList`), `0x10000065` (`UI_Vendor_FilterType`), `0x1000007F` (`Mood_Control_Dread_Decay_Rate`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x0000011C` (`UICore_Dialog_modal`), `0x00000420` (`UICore_Dialog_Range_Check_Entity`), `0x000004C5` (`UICore_Dialog_Range_Check_Distance`), `0x100003CF` (`Recall_Confirmation_Type`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x10000132` (`Death_EffectOnBehalfOf`), `0x00000152` (`Option_NumStops`) *(+16 more)*
- **Referenced WLib Strings (`0x0F`, 4 total)**: `"!"`, `"for"`, `"Clearing block instance from travel record"`, `"RecallSystem::InqTravelLinkPosition"`
- **Referenced Native Functions (`0x15`, 62 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+48 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85240001` | `1` | Unpack (S->C) | `0x85240001` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86DB000A` (`+1453708`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85240003` | `3` | Unpack (S->C) | `0x85240003` | `(i32/u32, ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x86DB0006` (`+1453768`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85240009` | `4` | Unpack (S->C) | `0x85240009` | `()` | `0x00D60007(0x06C3)` | `0x86DB0004` (`+1453804`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85240010` | `7` | Pack (C->S) | `0x8524000C` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x86DB0003` (`+1453104`) | — |
| `0x85240011` | `8` | Pack (C->S) | `0x8524000E` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8C470005` (`+1704532`), `0x86280006` (`+1712280`) | NAT: `0x01490005` (`0x140b38a90`), `0x00A60016` (`0x140392220`), `0x00C7000C` (`0x1403afdf0`); PID: `0x1000016B` (`TutorialHint_Name`), `0x10000145` (`ExecuteScript_GetContainingEntityIID_ContainerIID`), `0x10003C9E` (`Recall_MilestoneSkill`) |
| `0x85240012` | `9` | Pack (C->S) | `0x85240002` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x86DB0009` (`+1452836`) | — |
| `0x85240016` | `11` | Pack (C->S) | `0x85240015` | `()` | `0x00D9000C` (`self`) | `0x86DB000C` (`+1454072`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x01810002` (`0x140b2d4b0`), `0x00C70009` (`0x14037c4c0`); PID: `0x10000132` (`Death_EffectOnBehalfOf`), `0x10000065` (`UI_Vendor_FilterType`) |
| `0x85240018` | `12` | Unpack (S->C) | `0x85240018` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x86DB000E` (`+1453736`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0527` (`1319`) — Script Dialog / WSL Callback Return System (GCParam_WSLReturnValue, 0x037F) <a id="rpc-class-0x0527"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x037F`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 7 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x10000B20` (`GCParam_WSLReturnValue`), `0x10000ED2` (`GCParam_DialogText`), `0x10003C2D` (`GCParam_DialogText_InitiatorIID`), `0x10000CDF` (`GCParam_DisplayText_IsBillboardText`), `0x10002761` (`GCParam_DialogChatTextType`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 50 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+36 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85270006` | `1` | Unpack (S->C) | `0x85270006` | `(ref)` | `0x00D60007(0x06C3)` | `0x837F0006` (`+421340`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0528` (`1320`) — SkillSystem (Skill Execution, Toggle, Target Validation & Pip/Vital Checks) <a id="rpc-class-0x0528"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0127`, `0x017A`
- **Method Counts**: `5` Pack (C->S), `4` Unpack (S->C) (`9` total)
- **Referenced Properties (`pids.txt`, 233 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x0000033B` (`Render_AdjustableScale`), `0x100001DE` (`Channeling_InterruptedByMovement`), `0x10002545` (`Antispam_ConjunctionContributionRequest`), `0x000006C3` (`Appearance_ClassList`), `0x0000017A` (`PEDesc_RotationVelocityZ`), `0x1000180A` (`GOP_EnableSkillTargetNearestEnemy`), `0x0000030E` (`PropertyTarget_PropertyName`), `0x1000002C` (`MPAdvancementControl_VirtueCostProgression`), `0x100002A8` (`EnvironmentType`), `0x10000C6E` (`Combat_Violent`), `0x1000009F` (`Signals@NPC::HintChange::Source`), `0x1000017F` (`Effect_Duration_ConstantPulseCount`), `0x10000281` (`Combat_ActionDurationMultiplierMod`), `0x100002D4` (`ConsiderationTable_RankPerDifferentialArray`), `0x10000219` (`Craft_Cook_CriticalChanceAddModifier`), `0x100000E6` (`UI_PropertyMeter_MinValuePropertyName`), `0x100003E8` (`Craft_Leatherworker_StartTime`) *(+215 more)*
- **Referenced WLib Strings (`0x0F`, 35 total)**: `"in combo chain for skill:"`, `"Missing skill:"`, `"ComboSystem::GetComboSkillByAgent"`, `"A circular reference has been detected while processing the combo chain for skil"`, `"."`, `"> ERROR: Got a request for a skill that does not exist in data(skillID:"`, `"SkillSystem::GetIconDID"`, `"> ERROR: Unable to lookup skill from did:"`, `"SkillSystem::IsSkillHarmful"`, `"Unable to get skill by DID:"` *(+25 more)*
- **Referenced Native Functions (`0x15`, 171 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+157 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 45 total)**: `0x817A0018` @ `0x140b4e752` (fn `0x140b4e6ff`), `0x817A0149` @ `0x140b4e45c` (fn `0x140b4e3bc`), `0x8127003C` @ `0x140b54671` (fn `0x140b54340`), `0x8127003D` @ `0x140b54cef` (fn `0x140b549b0`), `0x81270052` @ `0x140b5adfc` (fn `0x140b5ada0`), `0x81270003` @ `0x140b60772` (fn `0x140b60724`), `0x81270002` @ `0x140b608a0` (fn `0x140b60816`), `0x8127003F` @ `0x140b609a2` (fn `0x140b6094f`), `0x81270005` @ `0x140b6510b` (fn `0x140b650c0`), `0x81270047` @ `0x140b651e7` (fn `0x140b65150`), `0x81270004` @ `0x140b6528c` (fn `0x140b65230`), `0x81270031` @ `0x140b65731` (fn `0x140b656d0`) *(+33 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8528000A` | `6` | Unpack (S->C) | `0x8528000A` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x817A00B4` (`+1508480`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8528000D` | `7` | Unpack (S->C) | `0x8528000D` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x817A00B1` (`+1508552`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8528000E` | `8` | Unpack (S->C) | `0x8528000E` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x817A00AE` (`+1508516`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85280012` | `11` | Pack (C->S) | `0x85280008` | `()` | `0x00D9000C` (`self`) | `0x817A00A2` (`+1504524`) | PID: `0x1000010B` (`Conjunction_ListenerTarget`) |
| `0x85280013` | `12` | Pack (C->S) | `0x85280002` | `()` | `0x00D9000C` (`self`) | `0x817A00B9` (`+1504676`) | — |
| `0x85280014` | `13` | Pack (C->S) | `0x85280005` | `(ref)` | `0x00D9000C` (`self`) | `0x817A0149` (`+1505580`) [C++: `0x140b4e45c`] | NAT: `0x0D850006` (`0x1403a7700`), `0x0D850008` (`0x1403afd80`), `0x0D850012` (`0x140a877a0`) |
| `0x85280015` | `14` | Pack (C->S) | `0x85280006` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81270020` (`+1679888`) | PID: `0x1000016B` (`TutorialHint_Name`), `0x10000035` (`QuestDispenser_RoleSuccessText`) |
| `0x85280016` | `15` | Pack (C->S) | `0x85280007` | `()` | `0x00D9000C` (`self`) | `0x817A0018` (`+1506708`) [C++: `0x140b4e752`] | — |
| `0x8528001A` | `16` | Unpack (S->C) | `0x8528001A` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x817A00FC` (`+1508580`) | NAT: `0x015E0014` (`0x140b31670`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x052A` (`1322`) — SummonSystem (Horn/Muster Player Summoning & Confirmation Dialog) <a id="rpc-class-0x052a"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x06DD`
- **Method Counts**: `1` Pack (C->S), `1` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 13 total)**: `0x00000198` (`Area_Name`), `0x000006C3` (`Appearance_ClassList`), `0x10000034` (`NPC_ColorText`), `0x100002C1` (`GRSObject_Crafting_MasteryLevel_1`), `0x1000091C` (`Summon_ToSummonID`), `0x10001EB5` (`Summon_ConfirmationMessage_Override`), `0x10001EB3` (`Summon_ConfirmationMessageWithMaterial_Override`), `0x1000001C` (`Perk_Effect`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x10000A04` (`Summon_SummonerID`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 16 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00C70019` (`0x14039dba0`), `0x00C7000C` (`0x1403afdf0`), `0x00A60017` (`0x1403ab3c0`), `0x00A60003` (`0x14037fc50`), `0x00A6002C` (`0x1403977a0`), `0x00C70009` (`0x14037c4c0`), `0x00A60005` (`0x14037ae30`), `0x00330001` (`0x140384e80`), `0x00A60015` (`0x1403a9640`), `0x00A60019` (`0x1403aa260`), `0x00A6001F` (`0x1403a9c20`) *(+2 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x852A0004` | `2` | Unpack (S->C) | `0x852A0004` | `(i32/u32, ref, i32/u32, i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86DD0007` (`+1522204`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852A0006` | `3` | Pack (C->S) | `0x852A0003` | `(i32/u32, ref, i64/u64)` | `0x00D9000C` (`self`) | `0x86DD0003` (`+1521288`), `0x86DD0002` (`+1521440`) | NAT: `0x00C70019` (`0x14039dba0`), `0x00C7000C` (`0x1403afdf0`); PID: `0x10000034` (`NPC_ColorText`), `0x100002C1` (`GRSObject_Crafting_MasteryLevel_1`) |

### Class `0x052B` (`1323`) — TappingSystem (Mob Tap Ownership & Open Tapping) <a id="rpc-class-0x052b"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0124`
- **Method Counts**: `0` Pack (C->S), `2` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 6 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10000BC4` (`Tapping_TappedToIID`), `0x10000EA5` (`Tapping_Untappable_State`), `0x100014EA` (`AI_KungFu_IsNPC`), `0x10004CD8` (`Tapping_OpenTapping`), `0x0000008C` (`SoundScript_SoundID`)
- **Referenced WLib Strings (`0x0F`, 1 total)**: `"To set the targeted mob to be open tapped, use "/opentap true""`
- **Referenced Native Functions (`0x15`, 4 total)**: `0x00E70001` (`0x140821170`), `0x00B60029` (`0x1403c5260`), `0x0E0E000A` (`0x140b30aa0`), `0x0E0E000B` (`0x140b3f5a0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 5 total)**: `0x8124000D` @ `0x140b8e1e2` (fn `0x140b8e194`), `0x81240009` @ `0x140b8f622` (fn `0x140b8f5d4`), `0x81240001` @ `0x140b8f8e2` (fn `0x140b8f894`), `0x81240002` @ `0x140b90422` (fn `0x140b903d4`), `0x81240003` @ `0x140b907c2` (fn `0x140b90774`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x852B000F` | `1` | Unpack (S->C) | `0x852B000F` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8124000A` (`+1523384`) | NAT: `0x0E0E000A` (`0x140b30aa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852B0013` | `2` | Unpack (S->C) | `0x852B0013` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8124000C` (`+1523428`) | NAT: `0x0E0E000B` (`0x140b3f5a0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x052C` (`1324`) — CTextItemSystem (Readable Parchments/Books & Custom Text Items) <a id="rpc-class-0x052c"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x017C`, `0x0606`
- **Method Counts**: `3` Pack (C->S), `5` Unpack (S->C) (`8` total)
- **Referenced Properties (`pids.txt`, 50 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x00000126` (`WeenieType`), `0x000001E8` (`PreprocOptions_TexGen_Directory`), `0x000006C3` (`Appearance_ClassList`), `0x10000173` (`Effect_InduceCombatState_StateToInduce`), `0x0000004F` (`Option_Description`), `0x00000063` (`Camera_Pitch`), `0x000000EB` (`RunScriptOutput_Source`), `0x00000120` (`UICore_ListBox_item_selected_state`), `0x0000013F` (`MeshFXScript_HoldingLocation`), `0x00000148` (`ClearanceCylRadius2`), `0x00000176` (`UICore_Viewport_RenderToTextureWidth`), `0x00000180` (`MeshReplacementScript_MeshType`), `0x00000197` (`UIB_MediaDesc_sound_file`), `0x000001A1` (`PreprocOptions_TexGen_ForceMaterials`), `0x10000CF3` (`TextItem_Counter_Letters`), `0x10000A58` (`TextItem_Counter_Parchments`), `0x100004EA` (`TextItem_Counter_Books`) *(+32 more)*
- **Referenced WLib Strings (`0x0F`, 60 total)**: `">"`, `":"`, `"LogUtils::LogAndAssert"`, `"INVALID_IID"`, `"(CLIENT)"`, `"]"`, `"[IID:"`, `"[Location:"`, `"the author!"`, `", it's either a prefabricated book or the current Player is not"` *(+50 more)*
- **Referenced Native Functions (`0x15`, 72 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+58 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x817C0004` @ `0x140b81ad8` (fn `0x140b81a40`), `0x817C0002` @ `0x140b8bbd5` (fn `0x140b8b8c0`), `0x817C0001` @ `0x140b9bcf1` (fn `0x140b9bc90`), `0x817C0003` @ `0x140b9cc5f` (fn `0x140b9cb31`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x852C0002` | `1` | Unpack (S->C) | `0x852C0002` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817C0008` (`+1529164`) | NAT: `0x01600004` (`0x140b43470`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852C0004` | `2` | Unpack (S->C) | `0x852C0004` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x817C0009` (`+1528836`) | NAT: `0x00E70003` (`0x140821030`), `0x00DD0001` (`0x1403b6150`), `0x01600001` (`0x140b433f0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x0000013F` (`MeshFXScript_HoldingLocation`), `0x00000148` (`ClearanceCylRadius2`); MSG: `", it may not be in our Inventory after all!"`, `"ERROR: Failed to get object for TextItem"` |
| `0x852C0009` | `5` | Unpack (S->C) | `0x852C0009` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817C0005` (`+1529684`) | NAT: `0x01600004` (`0x140b43470`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852C000A` | `6` | Unpack (S->C) | `0x852C000A` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817C000C` (`+1529188`) | NAT: `0x0030000A` (`0x140386f40`), `0x00DD0001` (`0x1403b6150`), `0x01600002` (`0x140b34c40`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000176` (`UICore_Viewport_RenderToTextureWidth`), `0x00000180` (`MeshReplacementScript_MeshType`); MSG: `"when it is supposed to be in my inventory!"`, `"ERROR: Failed to locate item with ID"` |
| `0x852C000D` | `8` | Unpack (S->C) | `0x852C000D` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817C000B` (`+1529436`) | NAT: `0x0030000A` (`0x140386f40`), `0x00DD0001` (`0x1403b6150`), `0x01600003` (`0x140b3d5f0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000197` (`UIB_MediaDesc_sound_file`), `0x000001A1` (`PreprocOptions_TexGen_ForceMaterials`); MSG: `"when it is supposed to be in my inventory!"`, `"ERROR: Failed to locate item with ID"` |
| `0x852C000E` | `9` | Pack (C->S) | `0x852C000C` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x817C0007` (`+1527376`) | NAT: `0x01600001` (`0x140b433f0`) |
| `0x852C000F` | `10` | Pack (C->S) | `0x852C0006` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x86060009` (`+1667832`) | NAT: `0x00E70003` (`0x140821030`), `0x00DD0012` (`0x1403b3140`), `0x00DD0001` (`0x1403b6150`); PID: `0x00000104` (`DungeonFlag`), `0x1000005F` (`HotSpot_InteractionPermissions_Enabled`), `0x10000898` (`CSM_TextItem_Option_Paste`); MSG: `"Invalid target ID!"`, `"TextItemActionHandler::HandleAction"` |
| `0x852C0010` | `11` | Pack (C->S) | `0x852C0005` | `(ref, ref, i64/u64)` | `0x00D9000C` (`self`) | `0x817C000A` (`+1527572`) | NAT: `0x00DD0001` (`0x1403b6150`); PID: `0x10000173` (`Effect_InduceCombatState_StateToInduce`), `0x0000004F` (`Option_Description`), `0x00000063` (`Camera_Pitch`); MSG: `"the author!"`, `", it's either a prefabricated book or the cur"` |

### Class `0x052D` (`1325`) — Scene / Cutscene Playback System (/scene play, Ready/Not Ready Sync) <a id="rpc-class-0x052d"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x017D`
- **Method Counts**: `4` Pack (C->S), `2` Unpack (S->C) (`6` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 1 total)**: `"play"`
- **Referenced Native Functions (`0x15`, 8 total)**: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x00B60028` (`0x140381e40`), `0x01610002` (`0x140b32910`), `0x01610001` (`0x140b3c8c0`), `0x00E70001` (`0x140821170`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x817D0002` @ `0x140b56515` (fn `0x140b564e0`), `0x817D0003` @ `0x140b5faf5` (fn `0x140b5fac0`), `0x817D0004` @ `0x140b5f7b5` (fn `0x140b5f780`), `0x817D0001` @ `0x140b5e0ed` (fn `0x140b5e020`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x852D0003` | `2` | Unpack (S->C) | `0x852D0003` | `()` | `0x00D60007(0x06C3)` | `0x817D000E` (`+1530896`) | NAT: `0x01610002` (`0x140b32910`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852D0007` | `5` | Unpack (S->C) | `0x852D0007` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x817D000D` (`+1530868`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852D0009` | `7` | Pack (C->S) | `0x852D0008` | `()` | `0x00D9000C` (`self`) | `0x817D0003` (`+1531032`) [C++: `0x140b5faf5`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x852D000A` | `8` | Pack (C->S) | `0x852D0005` | `()` | `0x00D9000C` (`self`) | `0x817D0002` (`+1530996`) [C++: `0x140b56515`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x852D000B` | `9` | Pack (C->S) | `0x852D0002` | `(ref)` | `0x00D9000D` (`local_0`) | `0x86C2009E` (`+70092`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B60014` (`0x1403c6650`); MSG: `"play"` |
| `0x852D000C` | `10` | Pack (C->S) | `0x852D0006` | `()` | `0x00D9000C` (`self`) | `0x817D0004` (`+1531068`) [C++: `0x140b5f7b5`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x052E` (`1326`) — TitleSystem (Earned Titles, Active Title Selection & Title Display) <a id="rpc-class-0x052e"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x017E`
- **Method Counts**: `8` Pack (C->S), `3` Unpack (S->C) (`11` total)
- **Referenced Properties (`pids.txt`, 17 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x000005BE` (`BeamScript_HeadAnimStartTime`), `0x10000BFE` (`Title_String`), `0x00000071` (`UICore_Button_hot_button`), `0x1000119C` (`Title_ActiveTitleString`), `0x1000088B` (`Title_ActiveTitleDID`), `0x00000183` (`FireMissileScript_Lifetime`), `0x0000018D` (`Environment_PlayerIndoorAmbianceOverride`), `0x000001CD` (`AnimationScript_DetachOnRampOut`), `0x000001D7` (`Account_ClientLanguage`), `0x000001E0` (`UI_Map_PixelOffsetX`), `0x000001F3` (`Server_World_AutoAdminAccounts`), `0x0000023D` (`GenericPhysicsController_FlyUpMoveSpeed`), `0x0000024A` (`UICore_ContextMenu_lvl`), `0x10000060` (`SCP_GetUpcomingHitResult_Result`), `0x100001FA` (`MonsterPlay_Name_OrcShaman`)
- **Referenced WLib Strings (`0x0F`, 52 total)**: `">"`, `":"`, `"LogUtils::LogAndAssert"`, `", unable to complete Title template!"`, `"ERROR: Failed to retrieve Title_String property from title"`, `"Title::InqTitleString"`, `", unable to validate active Title!"`, `"ERROR: Failed to retrieve Title instance for DataID"`, `"TitleSystem::InqTitleString"`, `"!"` *(+42 more)*
- **Referenced Native Functions (`0x15`, 70 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+56 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 6 total)**: `0x817E0005` @ `0x140b50574` (fn `0x140b50505`), `0x817E0006` @ `0x140b4f642` (fn `0x140b4f5ef`), `0x817E0002` @ `0x140b57941` (fn `0x140b57916`), `0x817E002B` @ `0x140b59f29` (fn `0x140b59e67`), `0x817E0004` @ `0x140b5a0b1` (fn `0x140b59fdc`), `0x817E0001` @ `0x140b5a26a` (fn `0x140b5a191`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x852E0005` | `2` | Unpack (S->C) | `0x852E0005` | `(f64, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x817E0022` (`+1534684`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x0135001D` (`0x140b43560`); PID: `0x000006C3` (`Appearance_ClassList`), `0x0000023D` (`GenericPhysicsController_FlyUpMoveSpeed`), `0x0000024A` (`UICore_ContextMenu_lvl`); MSG: `"ERROR: Failed to construct Title StringInfo f"`, `"TitleSystem::RecvCEvt_TitleAwarded"` |
| `0x852E0013` | `7` | Unpack (S->C) | `0x852E0013` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x817E0029` (`+1535180`) | NAT: `0x0135001E` (`0x140b43630`); PID: `0x000006C3` (`Appearance_ClassList`), `0x100001FA` (`MonsterPlay_Name_OrcShaman`) |
| `0x852E0016` | `10` | Unpack (S->C) | `0x852E0016` | `()` | `0x00D60007(0x06C3)` | `0x817E0027` (`+1535268`) | NAT: `0x0135001F` (`0x140b436d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852E0018` | `11` | Pack (C->S) | `0x852E0009` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x817E0023` (`+1537176`) | NAT: `0x00B70009` (`0x140381e40`), `0x00550003` (`0x1403b4dc0`), `0x00B70003` (`0x1403ba520`); MSG: `"Missing required Title definition file name."`, `"You have no target, you must target a Player "` |
| `0x852E0019` | `12` | Pack (C->S) | `0x852E0012` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x817E0021` (`+1536068`) | NAT: `0x00550003` (`0x1403b4dc0`); MSG: `"You have no target, you must target a Player "`, `"Your target is not a Player, you must target "` |
| `0x852E001A` | `13` | Pack (C->S) | `0x852E0015` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x817E0024` (`+1537540`) | NAT: `0x00B70009` (`0x140381e40`), `0x00550003` (`0x1403b4dc0`), `0x00B70003` (`0x1403ba520`); MSG: `"Missing required Title definition file name."`, `"You have no target, you must target a Player "` |
| `0x852E001B` | `14` | Pack (C->S) | `0x852E0001` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x817E0020` (`+1537904`) | NAT: `0x00550003` (`0x1403b4dc0`); MSG: `"You have no target, you must target a Player "`, `"Your target is not a Player, you must target "` |
| `0x852E001C` | `15` | Pack (C->S) | `0x852E000E` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x817E0005` (`+1534564`) [C++: `0x140b50574`] | — |
| `0x852E001D` | `16` | Pack (C->S) | `0x852E0010` | `()` | `0x00D9000C` (`self`) | `0x817E0006` (`+1534592`) [C++: `0x140b4f642`] | — |
| `0x852E001E` | `17` | Pack (C->S) | `0x852E0014` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x817E0028` (`+1536284`) | NAT: `0x00B70009` (`0x140381e40`), `0x00550003` (`0x1403b4dc0`), `0x00B70003` (`0x1403ba520`); MSG: `"Missing required Title definition file name."`, `"You have no target, you must target a Player "` |
| `0x852E0023` | `19` | Pack (C->S) | `0x852E0021` | `(ref)` | `0x00D9000D` (`local_0`) | `0x817E002F` (`+1536648`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B6002B` (`0x1403c0470`), `0x00B70003` (`0x1403ba520`); PID: `0x000005BE` (`BeamScript_HeadAnimStartTime`); MSG: `"Wrong number of arguments for this command."`, `"] is not a player!"` |

### Class `0x052F` (`1327`) — EquivalenceEffect / Cooldown Timer Notification System (0x06DC / 0x015E) <a id="rpc-class-0x052f"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x06DC`
- **Method Counts**: `0` Pack (C->S), `4` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x0000033B` (`Render_AdjustableScale`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 8 total)**: `0x00120003` (`0x1403b8fa0`), `0x015E000E` (`0x140b2fd90`), `0x015E000F` (`0x140b33ca0`), `0x00120001` (`0x1403b6790`), `0x00120002` (`0x1403bb160`), `0x0030000B` (`0x140385ab0`), `0x00B9000D` (`0x1403a59c0`), `0x00B90004` (`0x1403b2b90`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x852F0003` | `1` | Unpack (S->C) | `0x852F0003` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86DC0001` (`+1490684`) | NAT: `0x015E000F` (`0x140b33ca0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852F0004` | `2` | Unpack (S->C) | `0x852F0004` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86DC0003` (`+1490640`) | NAT: `0x015E000E` (`0x140b2fd90`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852F0021` | `3` | Unpack (S->C) | `0x852F0021` | `(i64/u64, i32/u32)` | `0x00D60007(0x06C3)` | `0x86DC0006` (`+1490820`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00B9000D` (`0x1403a59c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x852F0024` | `4` | Unpack (S->C) | `0x852F0024` | `(i64/u64, i32/u32)` | `0x00D60007(0x06C3)` | `0x86DC0007` (`+1490940`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00B90004` (`0x1403b2b90`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0530` (`1328`) — CTrackingSystem (Entity Radar/Minimap Tracking Update) <a id="rpc-class-0x0530"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x06DF`
- **Method Counts**: `0` Pack (C->S), `5` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 6 total)**: `0x000006C3` (`Appearance_ClassList`), `0x000000C8` (`UICore_Button_hot_click_interval`), `0x100014EA` (`AI_KungFu_IsNPC`), `0x000000C7` (`UICore_Text_fit_to_text`), `0x10000011` (`KinshipTheme_RankEntry`), `0x000001AB` (`MoveToController_DownSpeed`)
- **Referenced WLib Strings (`0x0F`, 9 total)**: `"Undef_Tracking state is not a valid argument!"`, `"CTrackingSystem::StartTracking"`, `"CTrackingSystem::UpdateTrackedEntity"`, `"usage: /track [cmd]"`, `"Toggles tracking for an entity."`, `"dump: lists all the tracked targets on the admin."`, `"manager dump [<IID>]: Dumps the tracking manager record for this IID(dumps all r"`, `"manager dumphash: Dumps the tracking manager nested hash(maps locations to recor"`, `"manager search <tracking state> <search distance> <num results>: Searches near t"`
- **Referenced Native Functions (`0x15`, 22 total)**: `0x00E70001` (`0x140821170`), `0x00990004` (`0x140389fa0`), `0x00A10007` (`0x14039c1c0`), `0x00A9003F` (`0x1403a42c0`), `0x01620001` (`0x140b42d30`), `0x01620002` (`0x140b42fe0`), `0x00C7000C` (`0x1403afdf0`), `0x00C7001A` (`0x14039b820`), `0x01620003` (`0x140b493e0`), `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760002` (`0x1403ba230`), `0x00760003` (`0x1403ba130`), `0x0019002E` (`0x1403bf1b0`) *(+8 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x86DF0009` @ `0x140b918ac` (fn `0x140b9179b`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85300006` | `1` | Unpack (S->C) | `0x85300006` | `(i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86DF0004` (`+1540368`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8530000C` | `2` | Unpack (S->C) | `0x8530000C` | `(i32/u32, ref, i32/u32, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86DF0006` (`+1540288`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85300011` | `3` | Unpack (S->C) | `0x85300011` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x86DF0001` (`+1540332`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85300019` | `4` | Unpack (S->C) | `0x85300019` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x86DF000A` (`+1540632`) | NAT: `0x01620005` (`0x140b49330`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8530001E` | `5` | Unpack (S->C) | `0x8530001E` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x86DF000B` (`+1540600`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0531` (`1329`) — TradeSystem (Player-to-Player Secure Trade Session, Offer, Lock, Accept) <a id="rpc-class-0x0531"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x017F`
- **Method Counts**: `9` Pack (C->S), `12` Unpack (S->C) (`21` total)
- **Referenced Properties (`pids.txt`, 9 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x0000017F` (`RigidBodyScript_Entity`), `0x10000148` (`HiddenDragBox_Title`), `0x10000147` (`UI_SkillDisplay_PlayingOverlayState`), `0x10000062` (`UI_SkillDisplay_UILayout`), `0x100003D3` (`ThreatInputModifier_Damage`), `0x1000007E` (`Effect_Summoner`), `0x10000705` (`TutorialHint_AutoShow`)
- **Referenced WLib Strings (`0x0F`, 21 total)**: `"query"`, `"open"`, `"..."`, `"Opening trade with:"`, `"close"`, `"Closing trade..."`, `"accept"`, `"Accepting trade..."`, `"[no trade!]"`, `"decline"` *(+11 more)*
- **Referenced Native Functions (`0x15`, 64 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+50 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 8 total)**: `0x817F0001` @ `0x140b5d6c5` (fn `0x140b5d646`), `0x817F0002` @ `0x140b4e932` (fn `0x140b4e8df`), `0x817F0003` @ `0x140b4d322` (fn `0x140b4d2cf`), `0x817F0004` @ `0x140b4ecb2` (fn `0x140b4ec5f`), `0x817F0005` @ `0x140b5b2b5` (fn `0x140b5b236`), `0x817F0006` @ `0x140b5b190` (fn `0x140b5b106`), `0x817F0007` @ `0x140b5f335` (fn `0x140b5f2b6`), `0x817F0008` @ `0x140b8ef68` (fn `0x140b8eea0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8531000B` | `1` | Unpack (S->C) | `0x8531000B` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x817F0091` (`+1548536`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8531000E` | `2` | Unpack (S->C) | `0x8531000E` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x817F0081` (`+1548568`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8531000F` | `3` | Unpack (S->C) | `0x8531000F` | `(ref, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x817F008B` (`+1548336`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310016` | `4` | Unpack (S->C) | `0x85310016` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x817F008F` (`+1548600`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310035` | `9` | Unpack (S->C) | `0x85310035` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x817F007D` (`+1548660`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310046` | `12` | Unpack (S->C) | `0x85310046` | `()` | `0x00D60007(0x06C3)` | `0x817F007B` (`+1548372`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8531004C` | `13` | Unpack (S->C) | `0x8531004C` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817F0090` (`+1548456`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8531004E` | `14` | Unpack (S->C) | `0x8531004E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817F007E` (`+1548428`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310052` | `16` | Unpack (S->C) | `0x85310052` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x817F008D` (`+1548632`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8531005F` | `18` | Unpack (S->C) | `0x8531005F` | `()` | `0x00D60007(0x06C3)` | `0x817F0088` (`+1548512`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310061` | `20` | Unpack (S->C) | `0x85310061` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x817F008C` (`+1548396`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310068` | `21` | Unpack (S->C) | `0x85310068` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x817F0083` (`+1548484`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85310069` | `22` | Pack (C->S) | `0x85310050` | `(ref)` | `0x00D9000C` (`self`) | `0x817F0003` (`+1548172`) [C++: `0x140b4d322`] | — |
| `0x8531006A` | `23` | Pack (C->S) | `0x8531002E` | `(ref)` | `0x00D9000C` (`self`) | `0x817F0006` (`+1548280`) [C++: `0x140b5b190`] | — |
| `0x8531006B` | `24` | Pack (C->S) | `0x85310042` | `()` | `0x00D9000C` (`self`) | `0x86C200EE` (`+38664`) | NAT: `0x00B6002B` (`0x1403c0470`), `0x00B6002D` (`0x1403c1a60`), `0x00B70001` (`0x1403c4ed0`); MSG: `"query"`, `"open"` |
| `0x8531006C` | `25` | Pack (C->S) | `0x85310043` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x817F0001` (`+1548120`) [C++: `0x140b5d6c5`] | — |
| `0x8531006D` | `26` | Pack (C->S) | `0x85310060` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x817F0007` (`+1548308`) [C++: `0x140b5f335`] | — |
| `0x8531006E` | `27` | Pack (C->S) | `0x85310024` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x817F0005` (`+1548252`) [C++: `0x140b5b2b5`] | — |
| `0x8531006F` | `28` | Pack (C->S) | `0x85310025` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x86C200EE` (`+38664`) | NAT: `0x00B6002B` (`0x1403c0470`), `0x00B6002D` (`0x1403c1a60`), `0x00B70001` (`0x1403c4ed0`); MSG: `"query"`, `"open"` |
| `0x85310070` | `29` | Pack (C->S) | `0x8531005D` | `()` | `0x00D9000C` (`self`) | `0x817F0002` (`+1548148`) [C++: `0x140b4e932`] | — |
| `0x85310071` | `30` | Pack (C->S) | `0x85310034` | `()` | `0x00D9000C` (`self`) | `0x817F0004` (`+1548228`) [C++: `0x140b4ecb2`] | — |

### Class `0x0532` (`1330`) — CTraitSystem (Earned Traits, Slotted Traits, Specialization Trees) <a id="rpc-class-0x0532"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0125`
- **Method Counts**: `11` Pack (C->S), `18` Unpack (S->C) (`29` total)
- **Referenced Properties (`pids.txt`, 75 total)**: `0x000006C3` (`Appearance_ClassList`), `0x000000C2` (`UICore_Element_notify_on_move`), `0x000000E3` (`CinematicCameraScript_Yaw`), `0x0000010E` (`DecalVolume_StickToWorldEntities`), `0x00000120` (`UICore_ListBox_item_selected_state`), `0x00000133` (`FireMissileScript_InitialSpeed`), `0x00000149` (`ClearanceBBoxMax3`), `0x1000003C` (`ForwardSource_Movement_MountSpeedMultiplier`), `0x0000015D` (`InvertMouseY`), `0x00000173` (`UICore_Element_tooltip_ID`), `0x10000012` (`Guild_ID`), `0x100039D4` (`Alert_Trait_DescriptionText`), `0x00000195` (`TerrainTypeDesc_NormalTexture`), `0x1000773A` (`World_EnableSurvivalInstincts`), `0x100001F3` (`Drama_SetIdleDrama`), `0x10000696` (`ExaminationElement_Skill_Materials_Struct`), `0x100005C5` (`GOP_CtrlBar_AlwaysShow`), `0x100000FE` (`ForwardSource_GameEvent_CombatEventTable`) *(+57 more)*
- **Referenced WLib Strings (`0x0F`, 86 total)**: `"]"`, `"[DataID"`, `"<Unknown value>"`, `"["`, `">"`, `":"`, `"LogUtils::LogAndAssert"`, `". The client and server are probably out of sync!"`, `">ERROR: Client failed to add the Trait"`, `"CTraitSystem::RecvCEvt_AddEarnedTraitRequest"` *(+76 more)*
- **Referenced Native Functions (`0x15`, 110 total)**: `0x01310001` (`0x140b2ffb0`), `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x013D0002` (`0x140b32e40`), `0x00DD000F` (`0x1403b3090`), `0x00DD0008` (`0x1403b5b70`), `0x00420001` (`0x140397330`), `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0003` (`0x1403b6270`), `0x00E70001` (`0x140821170`), `0x01640003` (`0x140b34b90`), `0x01CB0001` (`0x140a827d0`), `0x01640001` (`0x140b428c0`) *(+96 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 53 total)**: `0x81250004` @ `0x140b9b872` (fn `0x140b9b7ed`), `0x81250007` @ `0x140b92092` (fn `0x140b9203f`), `0x81250008` @ `0x140b60e12` (fn `0x140b60dbf`), `0x81250009` @ `0x140b98c02` (fn `0x140b98baf`), `0x8125003C` @ `0x140b65f95` (fn `0x140b65f65`), `0x81250059` @ `0x140b91ec0` (fn `0x140b91e90`), `0x81250061` @ `0x140b9972b` (fn `0x140b996e0`), `0x81250067` @ `0x140b9b966` (fn `0x140b9b8f0`), `0x8125006C` @ `0x140b560b0` (fn `0x140b56080`), `0x81250073` @ `0x140b56280` (fn `0x140b56250`), `0x81250046` @ `0x140b56322` (fn `0x140b562c0`), `0x8125007C` @ `0x140b6817b` (fn `0x140b68130`) *(+41 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85320002` | `1` | Unpack (S->C) | `0x85320002` | `()` | `0x00D60007(0x06C3)` | `0x81250014` (`+1552920`) | NAT: `0x01640013` (`0x140b34fa0`), `0x00C7000C` (`0x1403afdf0`), `0x00A6002B` (`0x1403b0040`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000173` (`UICore_Element_tooltip_ID`), `0x10000012` (`Guild_ID`); MSG: `">ERROR: Client failed to unslot all Traits. T"`, `"CTraitSystem::RecvCEvt_UnslotAllTraitsRequest"` |
| `0x85320003` | `2` | Unpack (S->C) | `0x85320003` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81250011` (`+1552608`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000149` (`ClearanceBBoxMax3`); MSG: `"The client and server are probably out of syn"`, `"!"` |
| `0x85320009` | `4` | Unpack (S->C) | `0x85320009` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8125000F` (`+1552768`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x1000003C` (`ForwardSource_Movement_MountSpeedMultiplier`), `0x0000015D` (`InvertMouseY`); MSG: `"The client and server are probably out of syn"`, `"."` |
| `0x8532000B` | `5` | Unpack (S->C) | `0x8532000B` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x81250015` (`+1553168`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000195` (`TerrainTypeDesc_NormalTexture`); MSG: `"The client and server are probably out of syn"`, `"!"` |
| `0x8532000F` | `7` | Unpack (S->C) | `0x8532000F` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81250018` (`+1559536`) | NAT: `0x01640006` (`0x140b30c40`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85320010` | `8` | Unpack (S->C) | `0x85320010` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8125001F` (`+1559688`) | NAT: `0x00A9000E` (`0x14039aad0`), `0x00270015` (`0x140393f20`), `0x00B80002` (`0x1403ab2a0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x100066E0` (`WE_Virtue_Tier_Cap`), `0x10000047` (`Craft_OfferedVocationArray`) |
| `0x85320013` | `9` | Unpack (S->C) | `0x85320013` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x8125000D` (`+1552372`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000133` (`FireMissileScript_InitialSpeed`); MSG: `"The client and server are probably out of syn"`, `"."` |
| `0x85320019` | `12` | Unpack (S->C) | `0x85320019` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8125000C` (`+1559576`) | NAT: `0x01640007` (`0x140b3faf0`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8532001A` | `13` | Unpack (S->C) | `0x8532001A` | `()` | `0x00D60007(0x06C3)` | `0x81250016` (`+1553316`) | NAT: `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8532001D` | `14` | Unpack (S->C) | `0x8532001D` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81250017` (`+1551728`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x000000C2` (`UICore_Element_notify_on_move`); MSG: `". The client and server are probably out of s"`, `">ERROR: Client failed to add the Trait"` |
| `0x85320020` | `16` | Unpack (S->C) | `0x85320020` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81250019` (`+1552156`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x0000010E` (`DecalVolume_StickToWorldEntities`); MSG: `". The client and server are probably out of s"`, `">ERROR: Client failed to remove the Trait"` |
| `0x85320023` | `18` | Pack (C->S) | `0x8532001F` | `()` | `0x00D9000C` (`self`) | `0x81250009` (`+1559512`) [C++: `0x140b98c02`] | — |
| `0x85320024` | `19` | Pack (C->S) | `0x85320021` | `()` | `0x00D9000C` (`self`) | `0x81250008` (`+1559488`) [C++: `0x140b60e12`] | — |
| `0x85320025` | `20` | Pack (C->S) | `0x85320008` | `()` | `0x00D9000C` (`self`) | `0x81250007` (`+1559464`) [C++: `0x140b92092`] | — |
| `0x85320026` | `21` | Pack (C->S) | `0x85320016` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x81250033` (`+1559252`) | NAT: `0x00C7001A` (`0x14039b820`) |
| `0x85320027` | `22` | Pack (C->S) | `0x8532000E` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x81250004` (`+1556912`) [C++: `0x140b9b872`] | NAT: `0x01640012` (`0x140b33f30`); PID: `0x100000FE` (`ForwardSource_GameEvent_CombatEventTable`), `0x100001F3` (`Drama_SetIdleDrama`) |
| `0x8532002B` | `24` | Unpack (S->C) | `0x8532002B` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81250025` (`+1559652`) | NAT: `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8532002E` | `25` | Unpack (S->C) | `0x8532002E` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x81250028` (`+1559616`) | NAT: `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85320030` | `26` | Unpack (S->C) | `0x85320030` | `()` | `0x00D60007(0x06C3)` | `0x81250034` (`+1560748`) | NAT: `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8532003E` | `28` | Pack (C->S) | `0x8532003D` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x81250050` (`+1556364`) | NAT: `0x0023000A` (`0x1403bf2a0`), `0x00240006` (`0x1403c50c0`), `0x00240002` (`0x1403b9f80`); PID: `0x100001F3` (`Drama_SetIdleDrama`), `0x100005C5` (`GOP_CtrlBar_AlwaysShow`) |
| `0x85320046` | `30` | Pack (C->S) | `0x85320045` | `()` | `0x00D9000C` (`self`) | `0x81250059` (`+1561796`) [C++: `0x140b91ec0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8532004A` | `32` | Pack (C->S) | `0x85320049` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x81250061` (`+1562296`) [C++: `0x140b9972b`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8532004D` | `34` | Unpack (S->C) | `0x8532004D` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81250065` (`+1562600`) | PID: `0x000006C3` (`Appearance_ClassList`); MSG: `"Failed to find trait registry!"`, `"CTraitSystem::RecvCEvt_SetSuppressionToggleSt"` |
| `0x8532004E` | `35` | Pack (C->S) | `0x8532004B` | `(i32/u32, ref, i32/u32)` | `0x00D9000C` (`self`) | `0x8125006A` (`+1556172`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7001A` (`0x14039b820`); PID: `0x10000696` (`ExaminationElement_Skill_Materials_Struct`) |
| `0x85320050` | `37` | Pack (C->S) | `0x8532004F` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x81250067` (`+1556752`) [C++: `0x140b9b966`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x100000FE` (`ForwardSource_GameEvent_CombatEventTable`) |
| `0x85320054` | `38` | Unpack (S->C) | `0x85320054` | `(ref)` | `0x00D60007(0x06C3)` | `0x81250075` (`+1552280`) | NAT: `0x01640013` (`0x140b34fa0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000120` (`UICore_ListBox_item_selected_state`); MSG: `">ERROR: Client failed to remove one or more r"`, `"CTraitSystem::RecvCEvt_RemoveEarnedTraitsRequ"` |
| `0x85320056` | `39` | Unpack (S->C) | `0x85320056` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x81250076` (`+1551916`) | NAT: `0x01640013` (`0x140b34fa0`), `0x0011000E` (`0x140385130`), `0x00110004` (`0x1403bea40`); PID: `0x000006C3` (`Appearance_ClassList`), `0x000000E3` (`CinematicCameraScript_Yaw`); MSG: `">ERROR: Client failed to add one or more requ"`, `"CTraitSystem::RecvCEvt_AddEarnedTraitsRequest"` |
| `0x85320058` | `41` | Pack (C->S) | `0x85320057` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8125003C` (`+1560808`) [C++: `0x140b65f95`] | NAT: `0x00E70001` (`0x140821170`), `0x00110009` (`0x1403b9080`) |
| `0x8532005E` | `42` | Unpack (S->C) | `0x8532005E` | `()` | `0x00D60007(0x06C3)` | `0x81250081` (`+1553716`) | NAT: `0x00A9000E` (`0x14039aad0`), `0x00270013` (`0x140391550`); PID: `0x000006C3` (`Appearance_ClassList`), `0x1000773A` (`World_EnableSurvivalInstincts`) |

### Class `0x0533` (`1331`) — TravelSystem (Stable-Master Travel Web Routes & Teleportation) <a id="rpc-class-0x0533"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x01CC`
- **Method Counts**: `6` Pack (C->S), `6` Unpack (S->C) (`12` total)
- **Referenced Properties (`pids.txt`, 33 total)**: `0x100000BC` (`QuestTreasure_SelectedItem`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x000006C3` (`Appearance_ClassList`), `0x10000AAF` (`Mount_Is_Mounted`), `0x10005E35` (`PaperItemService_AssociatedDID`), `0x10005E34` (`PaperItemService_SourceDID`), `0x10005E36` (`PaperItemService_SourceIID`), `0x10000C37` (`EffectGenerator_EffectData_MapNoteAdd`), `0x10000042` (`Mouseover_AllowMouseoverOverride`), `0x10000C36` (`EffectScratch_VOT_InitialChangeDone`), `0x10000F66` (`TravelWebWC`), `0x000001D7` (`Account_ClientLanguage`), `0x000002D9` (`World_GRSLevel`), `0x10000518` (`UI_FloatyText_DamageColumns`), `0x00000202` (`UICore_ListBox_max_columns`), `0x000005DB` (`Account_AdditionalInfoSectionData`), `0x10001418` (`TravelDisplayName`), `0x100001A1` (`TravelRoute_Destination`) *(+15 more)*
- **Referenced WLib Strings (`0x0F`, 58 total)**: `"] has no TravelRegistry! Update failed!"`, `"> ERROR: Player ["`, `"CPlayer::RecvCEvt_ClearTravelRecord"`, `">"`, `":"`, `"LogUtils::LogAndAssert"`, `"]"`, `"[IID:"`, `"[Name:"`, `"[WB ID:"` *(+48 more)*
- **Referenced Native Functions (`0x15`, 85 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x01650005` (`0x140b44870`), `0x00DD0001` (`0x1403b6150`), `0x00B60014` (`0x1403c6650`), `0x0035000A` (`0x140384c30`), `0x013D0001` (`0x140b32db0`), `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0003` (`0x1403b6270`), `0x00990004` (`0x140389fa0`), `0x00DD0010` (`0x1403b3140`), `0x00C7001B` (`0x140387e70`), `0x00DD0008` (`0x1403b5b70`), `0x00DD000F` (`0x1403b3090`) *(+71 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 20 total)**: `0x81CC0009` @ `0x140aa07b0` (fn `0x140aa0780`), `0x81CC001E` @ `0x140aa0b95` (fn `0x140aa0b60`), `0x81CC0003` @ `0x140a9c241` (fn `0x140a9c140`), `0x81CC0001` @ `0x140a9c47b` (fn `0x140a9c370`), `0x81CC0028` @ `0x140a9c882` (fn `0x140a9c7ee`), `0x81CC0005` @ `0x140a9cbab` (fn `0x140a9cb00`), `0x81CC002B` @ `0x140a9d12b` (fn `0x140a9ce00`), `0x81CC0021` @ `0x140a9e035` (fn `0x140a9dfd1`), `0x81CC002D` @ `0x140a9e8db` (fn `0x140a9e890`), `0x81CC0022` @ `0x140a9eb9d` (fn `0x140a9ead0`), `0x81CC0023` @ `0x140a9f0ed` (fn `0x140a9f020`), `0x81CC0006` @ `0x140a9fafb` (fn `0x140a9fab0`) *(+8 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85330002` | `1` | Unpack (S->C) | `0x85330002` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x81CC0015` (`+1612180`) | NAT: `0x01650001` (`0x140b3bbf0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85330006` | `5` | Unpack (S->C) | `0x85330006` | `(ref)` | `0x00D60007(0x06C3)` | `0x81CC000B` (`+1611748`) | NAT: `0x00120007` (`0x1403bee50`), `0x00130005` (`0x1403c50c0`), `0x00130002` (`0x1403b9f80`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8533000B` | `7` | Unpack (S->C) | `0x8533000B` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81CC000D` (`+1612048`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85330012` | `9` | Pack (C->S) | `0x85330005` | `()` | `0x00D9000C` (`self`) | `0x81CC0009` (`+1613956`) [C++: `0x140aa07b0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85330013` | `10` | Pack (C->S) | `0x85330004` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x81CC000E` (`+1610248`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`); MSG: `"dump"`, `"dumpreg"` |
| `0x85330014` | `11` | Pack (C->S) | `0x85330008` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81CC0014` (`+1612660`) | PID: `0x10000F66` (`TravelWebWC`) |
| `0x85330015` | `12` | Pack (C->S) | `0x85330003` | `(ref)` | `0x00D9000C` (`self`) | `0x81CC000E` (`+1610248`), `0x81CC000E` (`+1610248`), `0x81CC000E` (`+1610248`), `0x81CC000E` (`+1610248`) | NAT: `0x00B60029` (`0x1403c5260`), `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`); MSG: `"dump"`, `"dumpreg"` |
| `0x85330016` | `13` | Pack (C->S) | `0x8533000D` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81CC0013` (`+1613084`) | NAT: `0x0099001B` (`0x1403b29b0`), `0x00DC0001` (`0x1403b22f0`) |
| `0x8533001B` | `14` | Unpack (S->C) | `0x8533001B` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x81CC0017` (`+1611844`) | NAT: `0x00B90002` (`0x1403a5840`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000C37` (`EffectGenerator_EffectData_MapNoteAdd`), `0x10000042` (`Mouseover_AllowMouseoverOverride`) |
| `0x85330023` | `16` | Pack (C->S) | `0x85330022` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81CC001E` (`+1614780`) [C++: `0x140aa0b95`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x85330025` | `17` | Unpack (S->C) | `0x85330025` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x86C30173` (`vslot 0x0354`, `+133616`) | NAT: `0x01650005` (`0x140b44870`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85330026` | `18` | Unpack (S->C) | `0x85330026` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x86C30172` (`vslot 0x0353`, `+133672`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x01650005` (`0x140b44870`); PID: `0x000006C3` (`Appearance_ClassList`); MSG: `"] has no TravelRegistry! Update failed!"`, `"> ERROR: Player ["` |

### Class `0x0534` (`1332`) — TutorialHintSystem (Tutorial Hint Display & Suppression) <a id="rpc-class-0x0534"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0126`
- **Method Counts**: `2` Pack (C->S), `2` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 6 total)**: `0x000006C3` (`Appearance_ClassList`), `0x1000156E` (`GOP_ShowTutorialHints`), `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x1000004D` (`TextItem_Title`), `0x100000C6` (`Usage_CooldownDataID`), `0x000005F7` (`MoveToController_MoveToFrozen`)
- **Referenced WLib Strings (`0x0F`, 4 total)**: `"show"`, `"Unable to find string to show!"`, `"as a tutorial hint. Should be a value from TutorialHintType.emp or a tutorial hi"`, `"Unable to interpret"`
- **Referenced Native Functions (`0x15`, 20 total)**: `0x00390001` (`0x1403b97d0`), `0x00E70001` (`0x140821170`), `0x0039000A` (`0x1403cb870`), `0x00390005` (`0x1403c9e50`), `0x00390008` (`0x1403c94a0`), `0x00270006` (`0x1403ae7b0`), `0x00270014` (`0x1403a87f0`), `0x019F001E` (`0x140a8b300`), `0x019F001D` (`0x140a7aaa0`), `0x00270013` (`0x140391550`), `0x00A9000E` (`0x14039aad0`), `0x01660001` (`0x140b375f0`), `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`) *(+6 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x81260002` @ `0x140b66b9b` (fn `0x140b66b50`), `0x81260003` @ `0x140b8c24c` (fn `0x140b8c074`), `0x81260001` @ `0x140b8dc9b` (fn `0x140b8dc50`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85340002` | `1` | Unpack (S->C) | `0x85340002` | `(ref)` | `0x00D60007(0x06C3)` | `0x81260007` (`+1624340`) | NAT: `0x00390001` (`0x1403b97d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85340004` | `2` | Unpack (S->C) | `0x85340004` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x81260008` (`+1624384`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85340007` | `5` | Pack (C->S) | `0x85340005` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x81260006` (`+1624736`) | NAT: `0x0039000A` (`0x1403cb870`), `0x00390005` (`0x1403c9e50`) |
| `0x85340008` | `6` | Pack (C->S) | `0x85340006` | `()` | `0x00D9000C` (`self`) | `0x81260004` (`+1624824`) | NAT: `0x00390008` (`0x1403c94a0`), `0x00270006` (`0x1403ae7b0`), `0x00270014` (`0x1403a87f0`); PID: `0x1000156E` (`GOP_ShowTutorialHints`) |

### Class `0x0535` (`1333`) — CUsageSystem (Interactive World Object Usage & Interaction Lock) <a id="rpc-class-0x0535"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0127`
- **Method Counts**: `3` Pack (C->S), `2` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 126 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x1000016B` (`TutorialHint_Name`), `0x10000035` (`QuestDispenser_RoleSuccessText`), `0x1000049A` (`Skill_InductionDuration_Trait_Fastpull_2`), `0x00000127` (`ResetCountdownTime`), `0x10000037` (`GeneratorProfile_RegenerationDelay`), `0x10000043` (`Mouseover_TargetedActionType`), `0x1000090A` (`Usage_DoMetaUsage`), `0x00000617` (`DetectScript_BoxMin`), `0x00000040` (`CameraShakeScript_OffsetY`), `0x1000007B` (`Player_TeleportSemaphore`), `0x100000A7` (`Skill_RequiredLineOfSight`), `0x100004E0` (`Craft_Leatherworker_CompletedProficiencyLevel`), `0x10000244` (`UI_ChatFont_FontEnum`), `0x10000287` (`Act_Scene_Array`), `0x100050F9` (`GOP_MoveToUse`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`) *(+108 more)*
- **Referenced WLib Strings (`0x0F`, 8 total)**: `"CUsageSystem::TakeInteractionLock"`, `"CUsageSystem::ReleaseInteractionLock"`, `"> Could not\n                              calculate the skill level with the gi"`, `"UsageSystem::GetUsageSkillLevel"`, `"of"`, `"unable to query property at index"`, `"UsageSystem::BuildAllowedUsersAsHashSet"`, `"unable to query iid from AllowedUserList:"`
- **Referenced Native Functions (`0x15`, 92 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+78 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 20 total)**: `0x81270002` @ `0x140b608a0` (fn `0x140b60816`), `0x8127003E` @ `0x140b65a35` (fn `0x140b659cb`), `0x8127003C` @ `0x140b54671` (fn `0x140b54340`), `0x8127003D` @ `0x140b54cef` (fn `0x140b549b0`), `0x81270052` @ `0x140b5adfc` (fn `0x140b5ada0`), `0x81270003` @ `0x140b60772` (fn `0x140b60724`), `0x8127003F` @ `0x140b609a2` (fn `0x140b6094f`), `0x81270005` @ `0x140b6510b` (fn `0x140b650c0`), `0x81270047` @ `0x140b651e7` (fn `0x140b65150`), `0x81270004` @ `0x140b6528c` (fn `0x140b65230`), `0x81270031` @ `0x140b65731` (fn `0x140b656d0`), `0x81270044` @ `0x140b66343` (fn `0x140b662b6`) *(+8 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8535000C` | `2` | Unpack (S->C) | `0x8535000C` | `(ref)` | `0x00D60007(0x06C3)` | `0x8127000F` (`+1686676`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85350027` | `4` | Unpack (S->C) | `0x85350027` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8127000E` (`+1686704`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8535002D` | `5` | Pack (C->S) | `0x85350002` | `(ref)` | `0x00D9000C` (`self`) | `0x8127001D` (`+1686396`) | NAT: `0x0030000B` (`0x140385ab0`); PID: `0x100002B5` (`Skill_Duration_Trait_Captain_BuffLineBonus`), `0x10000145` (`ExecuteScript_GetContainingEntityIID_ContainerIID`) |
| `0x8535002E` | `6` | Pack (C->S) | `0x8535001D` | `(ref)` | `0x00D9000C` (`self`) | `0x8127004B` (`+1680456`), `0x81270001` (`+1680928`), `0x81270002` (`+1681192`) [C++: `0x140b608a0`] | NAT: `0x00A60004` (`0x1403b4530`); PID: `0x1000016B` (`TutorialHint_Name`) |
| `0x8535003B` | `8` | Pack (C->S) | `0x8535003A` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8127003E` (`+1689728`) [C++: `0x140b65a35`] | — |

### Class `0x0536` (`1334`) — VendorSystem (NPC Merchant Buy, Sell, Buyback, Discount & Item Info) <a id="rpc-class-0x0536"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x01CE`
- **Method Counts**: `6` Pack (C->S), `1` Unpack (S->C) (`7` total)
- **Referenced Properties (`pids.txt`, 36 total)**: `0x000006C3` (`Appearance_ClassList`), `0x1000013C` (`Vendor_ItemList`), `0x000001C4` (`WhileScript_PropertyName`), `0x00000321` (`AnimConfigurableStruct`), `0x10000A01` (`Item_Class`), `0x00000205` (`UICore_Menu_popup_layout`), `0x0000020C` (`PathTest_Result`), `0x00000354` (`Name`), `0x00000216` (`SwitchScript_IsNull`), `0x0000021E` (`ParticleScript_BoneOffset`), `0x1000082D` (`Inventory_MaxQuantity`), `0x00000228` (`UIB_MediaDesc_has_alpha`), `0x00000230` (`Tools_Map_CameraOffset`), `0x0000024D` (`UICore_Meter_smooth_movement`), `0x10003D3E` (`Item_ForceNotSellable`), `0x1000056D` (`Inventory_SellLockedByID`), `0x1000129E` (`Vendor_BuysItems`), `0x10000300` (`Antispam_DisbandFellowshipRequest`) *(+18 more)*
- **Referenced WLib Strings (`0x0F`, 33 total)**: `"dump"`, `"buy"`, `"sell"`, `"Unrecognized command."`, `"from discount array."`, `"> ERROR: Unable to get element:"`, `"VendorSystem::GetVendorDiscountList"`, `"> Invalid property name in discount list."`, `"]"`, `"At quantity ["` *(+23 more)*
- **Referenced Native Functions (`0x15`, 50 total)**: `0x0027002A` (`0x1403939f0`), `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x00CC0003` (`0x1403c1d20`), `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00270030` (`0x1403903e0`), `0x00270031` (`0x140390320`), `0x00DD0003` (`0x1403b6270`), `0x00270021` (`0x140392a50`), `0x00190006` (`0x1403c2c90`) *(+36 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 13 total)**: `0x81CE0053` @ `0x140a98fd7` (fn `0x140a98dd6`), `0x81CE0005` @ `0x140a99400` (fn `0x140a991e2`), `0x81CE0002` @ `0x140a99ade` (fn `0x140a999fd`), `0x81CE0003` @ `0x140a9ff7c` (fn `0x140a9ff20`), `0x81CE0007` @ `0x140aa0e20` (fn `0x140aa0d6f`), `0x81CE0009` @ `0x140aa0f63` (fn `0x140aa0ece`), `0x81CE0058` @ `0x140aa10b3` (fn `0x140aa101e`), `0x81CE0001` @ `0x140aa123b` (fn `0x140aa1130`), `0x81CE0004` @ `0x140aa1663` (fn `0x140aa1634`), `0x81CE0006` @ `0x140aa18cb` (fn `0x140aa17b1`), `0x81CE000A` @ `0x140aa1a15` (fn `0x140aa1996`), `0x81CE0008` @ `0x140aa1b86` (fn `0x140aa1b5b`) *(+1 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85360004` | `3` | Unpack (S->C) | `0x85360004` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x81CE004B` (`+1796984`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85360011` | `7` | Pack (C->S) | `0x8536000A` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x81CE004D` (`+1799016`), `0x81CE0048` (`+1799752`) | NAT: `0x0068000B` (`0x1403817b0`), `0x0068000C` (`0x140385130`), `0x00680002` (`0x1403c6a90`); PID: `0x10000CF5` (`Antispam_VendorRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10000306` (`Vendor_SellAll_VendorID`) |
| `0x85360012` | `8` | Pack (C->S) | `0x8536000E` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81CE004F` (`+1798872`) | NAT: `0x0030000B` (`0x140385ab0`); PID: `0x100002BD` (`CounterBox_CurrentCount`) |
| `0x85360013` | `9` | Pack (C->S) | `0x85360007` | `(i64/u64)` | `0x00D9000D` (`local_0`) | `0x86C2010F` (`+68924`) | NAT: `0x00B60020` (`0x1403bb9d0`), `0x00B60029` (`0x1403c5260`), `0x00B60014` (`0x1403c6650`); MSG: `"dump"`, `"buy"` |
| `0x85360014` | `10` | Pack (C->S) | `0x85360002` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81CE0049` (`+1799972`) | — |
| `0x85360015` | `11` | Pack (C->S) | `0x85360001` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81CE0047` (`+1798420`) | NAT: `0x0030000B` (`0x140385ab0`); PID: `0x10000CF5` (`Antispam_VendorRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x100002BD` (`CounterBox_CurrentCount`) |
| `0x85360017` | `13` | Pack (C->S) | `0x85360016` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x81CE0057` (`+1798644`) | NAT: `0x0030000B` (`0x140385ab0`); PID: `0x10000CF5` (`Antispam_VendorRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x100002BD` (`CounterBox_CurrentCount`) |

### Class `0x0537` (`1335`) — VitalSystem (Vital Display Override Request & Vital Sync) <a id="rpc-class-0x0537"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0183`
- **Method Counts**: `2` Pack (C->S), `1` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 10 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x10004A0D` (`Antispam_VitalDisplayOverrideChangeRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x0000065D` (`WhenRunningScript_Script`), `0x0000065E` (`AfterEffectScript_DepthFogFarDistance`), `0x00000183` (`FireMissileScript_Lifetime`), `0x1000010B` (`Conjunction_ListenerTarget`), `0x10000025` (`Drama_ActionSequence`), `0x00000435` (`PEDesc_MaxParticles`)
- **Referenced WLib Strings (`0x0F`, 4 total)**: `"]"`, `"["`, `"Passed a null player reference."`, `"VitalSystem::UI_RequestVDO_UIStateID"`
- **Referenced Native Functions (`0x15`, 48 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+34 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x81830051` @ `0x140b97ea1` (fn `0x140b97e40`), `0x81830002` @ `0x140b52865` (fn `0x140b527d6`), `0x81830003` @ `0x140b8cd83` (fn `0x140b8cba6`), `0x8183007F` @ `0x140b97c77` (fn `0x140b97c29`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x85370003` | `1` | Unpack (S->C) | `0x85370003` | `(ref)` | `0x00D60007(0x06C3)` | `0x8183004B` (`+1813592`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x85370018` | `3` | Pack (C->S) | `0x85370017` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x81830051` (`+1813420`) [C++: `0x140b97ea1`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8537001F` | `5` | Pack (C->S) | `0x8537001E` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x86040056` (`+1659176`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x10004A0D` (`Antispam_VitalDisplayOverrideChangeRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) |

### Class `0x071A` (`1818`) — WorldEventSystem (World/Zone Event Conditions, History & Admin Control) <a id="rpc-class-0x071a"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2` | **Paired Client Implementation Class(es)**: `0x071B`, `0x071D`
- **Method Counts**: `1` Pack (C->S), `1` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 16 total)**: `0x000006C2` (`MapNote_StringInfo`), `0x00000717` (`World_Kynapse_TimePerFrame`), `0x10001691` (`WorldEvent_AllConditionList`), `0x10001693` (`WorldEvent_AnyConditionList`), `0x100016D6` (`Usage_WorldEvent_AllConditionList`), `0x100016D7` (`Usage_WorldEvent_AnyConditionList`), `0x1000168F` (`WorldEvent_Condition`), `0x10001924` (`WorldEvent_Broadcast_ConditionStruct`), `0x10001654` (`WorldEvent_WorldEvent`), `0x10003DCF` (`WorldEvent_StateToTestValue`), `0x10001690` (`WorldEvent_Operator`), `0x10003A66` (`WorldEvent_Condition_WorldEventToCompareWith`), `0x10001692` (`WorldEvent_ConditionValue`), `0x10001760` (`WorldEvent_Condition_UsageString`), `0x100017AE` (`WorldEvent_CountConditionList`), `0x0000075F` (`MapNote_SelectIcon`)
- **Referenced WLib Strings (`0x0F`, 14 total)**: `"usage:  /worldevent print [zone] - prints a list of all world/zone events\n     "`, `">"`, `">="`, `"<"`, `"<="`, `"!="`, `"=="`, `"}"`, `"{"`, `"[ All"` *(+4 more)*
- **Referenced Native Functions (`0x15`, 14 total)**: `0x00E70001` (`0x140821170`), `0x07420001` (`0x140b41fd0`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00110002` (`0x1403c68d0`), `0x001A0001` (`0x1403c4ed0`), `0x00E10005` (`0x1403ca7d0`), `0x00270001` (`0x14038a7e0`), `0x00190006` (`0x1403c2c90`), `0x00450003` (`0x140385ca0`), `0x00DD0008` (`0x1403b5b70`), `0x00DD0001` (`0x1403b6150`), `0x00DD0002` (`0x1403b5e20`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 5 total)**: `0x871D0006` @ `0x140b982c1` (fn `0x140b98270`), `0x871D000B` @ `0x140b50a01` (fn `0x140b50990`), `0x871D000C` @ `0x140b51c23` (fn `0x140b51b10`), `0x871D0008` @ `0x140b9819b` (fn `0x140b98150`), `0x871D000A` @ `0x140b9822b` (fn `0x140b981e0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x871A0016` | `1` | Unpack (S->C) | `0x871A0016` | `(ref, ref, ref)` | `0x00D60007(0x06C2)` | `0x871B000D` (`+1837496`) | PID: `0x000006C2` (`MapNote_StringInfo`) |
| `0x871A001A` | `3` | Pack (C->S) | `0x871A0017` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x871D0006` (`+1836368`) [C++: `0x140b982c1`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0728` (`1832`) — GameSetRegistry / Trait & Item Set Bonus System (0x0729 / 0x08E8) <a id="rpc-class-0x0728"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0729`
- **Method Counts**: `0` Pack (C->S), `3` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10000012` (`Guild_ID`)
- **Referenced WLib Strings (`0x0F`, 5 total)**: `"dump"`, `"..."`, `"Displaying GameSetRegistry for"`, `"client"`, `"!!Can't get the GameSetRegistry on the client for someone who isn't you!!"`
- **Referenced Native Functions (`0x15`, 17 total)**: `0x00B60001` (`0x1403b8d80`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x00550003` (`0x1403b4dc0`), `0x00DD0001` (`0x1403b6150`), `0x08E80002` (`0x140b35960`), `0x08E80001` (`0x140b38e70`), `0x00E70001` (`0x140821170`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`), `0x00C7001A` (`0x14039b820`) *(+3 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x87290008` @ `0x140b6a880` (fn `0x140b6a7d0`), `0x87290007` @ `0x140b71fd4` (fn `0x140b71f65`), `0x8729000A` @ `0x140b7217b` (fn `0x140b72130`), `0x87290009` @ `0x140b77d24` (fn `0x140b77ca0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x87280005` | `1` | Unpack (S->C) | `0x87280005` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87290005` (`+1477872`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87280006` | `2` | Unpack (S->C) | `0x87280006` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87290001` (`+1477904`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87280008` | `3` | Unpack (S->C) | `0x87280008` | `(ref)` | `0x00D60007(0x06C3)` | `0x87290003` (`+1477936`) | NAT: `0x08E80002` (`0x140b35960`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0739` (`1849`) — TargetMarking / Fellowship ReadyCheck System (0x0736) <a id="rpc-class-0x0739"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0736`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 4 total)**: `0x10001C56` (`Antispam_TargetMarkingRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10000074` (`UI_CharacterManagement_Race`), `0x1000006B` (`UI_Selection_MaxSelectionDistance`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 26 total)**: `0x00E70001` (`0x140821170`), `0x00B90002` (`0x1403a5840`), `0x00B90004` (`0x1403b2b90`), `0x00170007` (`0x140388ca0`), `0x0017000A` (`0x1403bef70`), `0x00180005` (`0x1403c50c0`), `0x00180002` (`0x1403b9f80`), `0x00180003` (`0x1403ba000`), `0x0030000B` (`0x140385ab0`), `0x00180001` (`0x1403c4e10`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x00B80004` (`0x1403abbd0`), `0x0011000E` (`0x140385130`) *(+12 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x87360007` @ `0x140b4d767` (fn `0x140b4d64b`), `0x87360004` @ `0x140b56843` (fn `0x140b567ae`), `0x87360015` @ `0x140b56993` (fn `0x140b568fe`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x87390014` | `2` | Pack (C->S) | `0x87390011` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87360005` (`+1525364`) | — |
| `0x87390016` | `4` | Pack (C->S) | `0x87390015` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87360010` (`+1524964`) | — |

### Class `0x077B` (`1915`) — ReputationSystem (Faction Standing, Tiers & Reputation Rewards) <a id="rpc-class-0x077b"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0769`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 26 total)**: `0x000006C3` (`Appearance_ClassList`), `0x1000015F` (`MoveItemDistance`), `0x1000015E` (`Craft_ForgeEffectRadius`), `0x10000160` (`Mood_Control_Mood_Level`), `0x10001902` (`NPC_ReputationColorTextArray`), `0x10001901` (`NPC_ColorText_MinReputationTier`), `0x10001900` (`NPC_ColorText_MaxReputationTier`), `0x10001781` (`Reputation_NegativeFaction`), `0x00000074` (`UICore_Viewport_AmbientLight`), `0x00000085` (`ScriptletParameterScript_Name`), `0x000000B2` (`DecalVolume_BackfaceCullingAngle`), `0x10003E94` (`Reputation_BonusRep_BonusRemaining`), `0x00000761` (`UICore_Element_OpacityMaskTexture`), `0x00000110` (`DecalVolume_UseBackfaceCulling`), `0x00000122` (`UICore_Text_bullet_spacing`), `0x10001815` (`Usage_RequiredFaction`), `0x10001814` (`Usage_RequiredFaction_DataID`), `0x10001813` (`Usage_RequiredFaction_Tier`) *(+8 more)*
- **Referenced WLib Strings (`0x0F`, 29 total)**: `"Usage: /reputation < init \| remove \| increase \| decrease \| setrepcap \| remo"`, `"* Init: Initializes the targets standing with the specified faction."`, `"* Remove: Removes the targets standing with the specified faction."`, `"* Increase: Increases the targets standing with the specified faction by the giv"`, `"* Decrease: Decreases the targets standing with the specified faction by the giv"`, `"* SetRepCap: This command will set a global restriction on reputation tier for t"`, `"* RemoveRepCap: This command will remove the global restriction from the specifi"`, `"]."`, `"> ERROR: Failed to obtain Advancement Table for Faction DID: ["`, `"ReputationSystem::CalculateTier"` *(+19 more)*
- **Referenced Native Functions (`0x15`, 20 total)**: `0x00E70001` (`0x140821170`), `0x00B60029` (`0x1403c5260`), `0x00E10006` (`0x1403b8050`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x07980001` (`0x140b35580`), `0x00270001` (`0x14038a7e0`), `0x07980006` (`0x140b3fbf0`), `0x07980005` (`0x140b355f0`), `0x07980007` (`0x140b434f0`), `0x00B00002` (`0x1403c68d0`), `0x00DD0011` (`0x1403b2fe0`), `0x00DD0001` (`0x1403b6150`), `0x00A90002` (`0x14037fd40`) *(+6 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 12 total)**: `0x87690022` @ `0x140b6a950` (fn `0x140b6a920`), `0x87690007` @ `0x140b6cfeb` (fn `0x140b6cf10`), `0x87690003` @ `0x140b6d154` (fn `0x140b6d0d0`), `0x87690011` @ `0x140b6d2ae` (fn `0x140b6d1f0`), `0x87690009` @ `0x140b6d4c7` (fn `0x140b6d450`), `0x8769000E` @ `0x140b7057b` (fn `0x140b70530`), `0x87690008` @ `0x140b756f1` (fn `0x140b75680`), `0x87690020` @ `0x140b7795b` (fn `0x140b77910`), `0x8769000C` @ `0x140b7a1bb` (fn `0x140b7a170`), `0x8769001E` @ `0x140b822e3` (fn `0x140b821f0`), `0x87690024` @ `0x140b8e4ab` (fn `0x140b8e460`), `0x8769001C` @ `0x140b99fdb` (fn `0x140b99f90`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x877B0007` | `1` | Unpack (S->C) | `0x877B0007` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8769000D` (`+1456556`) | NAT: `0x07980001` (`0x140b35580`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x077F` (`1919`) — SessionPlaySystem (MonsterPlay / Historical Session Play Control) <a id="rpc-class-0x077f"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0786`
- **Method Counts**: `4` Pack (C->S), `5` Unpack (S->C) (`9` total)
- **Referenced Properties (`pids.txt`, 28 total)**: `0x100017FD` (`MonsterPlay_SessionFailureText`), `0x100017FF` (`MonsterPlay_SessionCompletionText`), `0x000006C3` (`Appearance_ClassList`), `0x10000041` (`GOP_StaticPlacedTooltips`), `0x000002EF` (`UICore_Dialog_wait_text`), `0x1000023F` (`UI_MonsterPlayEnd_FinishData`), `0x10000C22` (`UI_MonsterPlayEnd_PlayerIID`), `0x00000517` (`UICore_Dialog_type`), `0x0000011C` (`UICore_Dialog_modal`), `0x10000050` (`Emote.Sit@Player.i_bChairFound`), `0x0000055F` (`UICore_Dialog_message_statementtext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x1000081E` (`Quest_DialogUI_QuestStatus`), `0x0000033E` (`UICore_Dialog_message_duration`), `0x100005F7` (`UI_MonsterPlayInitiation_Data`), `0x100005F6` (`UI_MonsterPlayInitiation_PlayerIID`), `0x100012BE` (`UI_MonsterPlayInitiation_SessionDID`), `0x10001803` (`UI_MonsterPlayInitiation_LoginSceneID`) *(+10 more)*
- **Referenced WLib Strings (`0x0F`, 10 total)**: `"usage: /sessionplay [cmd]"`, `"start <session filename> ['nonadmin']: attempts to initiate the given session."`, `"dump: display session info for this shard."`, `"add <Session filename> <IID>: request access to a session(for testing only)."`, `"remove <Session filename> <IID>: remove access to a session."`, `"reset <Session filename>: remove all current access to a session."`, `"lock <Session filename>: lock access to a session."`, `"unlock <Session filename>: unlock access to a session."`, `"resetall: remove all current access to all sessions(will not affect people alrea"`, `"cleanup: Kick off a cleanup and restart cleanup loop."`
- **Referenced Native Functions (`0x15`, 32 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00B60029` (`0x1403c5260`), `0x00C7000C` (`0x1403afdf0`), `0x00270006` (`0x1403ae7b0`), `0x00270029` (`0x1403aff90`), `0x00070002` (`0x140821f90`), `0x07890002` (`0x140b41230`), `0x0167001B` (`0x140b41140`), `0x00C70009` (`0x14037c4c0`), `0x0167001D` (`0x140b373c0`), `0x07890001` (`0x140b3ba40`), `0x0027002B` (`0x1403ab310`) *(+18 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 8 total)**: `0x87860003` @ `0x140b7aee2` (fn `0x140b7ae60`), `0x8786000F` @ `0x140b9703b` (fn `0x140b96ff0`), `0x87860010` @ `0x140b658db` (fn `0x140b65890`), `0x87860013` @ `0x140b6206a` (fn `0x140b61f70`), `0x87860017` @ `0x140b64422` (fn `0x140b643a0`), `0x87860016` @ `0x140b78b42` (fn `0x140b78ae0`), `0x87860002` @ `0x140b89fdd` (fn `0x140b89f10`), `0x87860015` @ `0x140b8a1cf` (fn `0x140b8a060`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x877F0019` | `1` | Unpack (S->C) | `0x877F0019` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8786000A` (`+1473628`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00270006` (`0x1403ae7b0`), `0x00270029` (`0x1403aff90`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000041` (`GOP_StaticPlacedTooltips`), `0x000002EF` (`UICore_Dialog_wait_text`) |
| `0x877F001E` | `2` | Unpack (S->C) | `0x877F001E` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87860007` (`+1474120`) | NAT: `0x0167001D` (`0x140b373c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x877F0020` | `3` | Unpack (S->C) | `0x877F0020` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8786000D` (`+1473788`) | NAT: `0x0167001B` (`0x140b41140`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000041` (`GOP_StaticPlacedTooltips`) |
| `0x877F0026` | `6` | Unpack (S->C) | `0x877F0026` | `(i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8786000C` (`+1474184`) | NAT: `0x07890001` (`0x140b3ba40`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x877F0029` | `8` | Pack (C->S) | `0x877F0025` | `()` | `0x00D9000C` (`self`) | `0x87860007` (`+1474120`) | NAT: `0x0167001D` (`0x140b373c0`) |
| `0x877F002B` | `10` | Pack (C->S) | `0x877F0028` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87860003` (`+1476152`) [C++: `0x140b7aee2`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x877F0039` | `12` | Unpack (S->C) | `0x877F0039` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87860011` (`+1473760`) | NAT: `0x07890002` (`0x140b41230`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x877F003C` | `14` | Pack (C->S) | `0x877F0032` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x87860010` (`+1476404`) [C++: `0x140b658db`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x877F003D` | `15` | Pack (C->S) | `0x877F003B` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8786000F` (`+1476356`) [C++: `0x140b9703b`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x078E` (`1934`) — MPMissionController / MonsterPlay Session Selection System (0x0787) <a id="rpc-class-0x078e"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0787`
- **Method Counts**: `1` Pack (C->S), `1` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 5 total)**: `0x0000078A` (`Entity_Camera_Array`), `0x0000078C` (`Entity_Camera_MaxDistance`), `0x000006C3` (`Appearance_ClassList`), `0x0000054F` (`Appearance_InstanceList`), `0x10001376` (`MonsterPlay_CurrentSessionDID`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 15 total)**: `0x00270006` (`0x1403ae7b0`), `0x00270030` (`0x1403903e0`), `0x00270031` (`0x140390320`), `0x00270032` (`0x1403a7af0`), `0x078F0004` (`0x140a89d80`), `0x078F0006` (`0x140a85b30`), `0x078F000C` (`0x140a84c60`), `0x078F0008` (`0x140a83ab0`), `0x078F000A` (`0x140a83a30`), `0x078F0002` (`0x140a82870`), `0x078F000E` (`0x140a89b00`), `0x078F0010` (`0x140a84830`), `0x07880002` (`0x140b33db0`), `0x00A60001` (`0x14037bb20`) *(+1 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 6 total)**: `0x87870002` @ `0x140b94005` (fn `0x140b93fd0`), `0x87870006` @ `0x140b6951a` (fn `0x140b69470`), `0x87870001` @ `0x140b69600` (fn `0x140b695d0`), `0x87870005` @ `0x140b71b84` (fn `0x140b71b00`), `0x87870007` @ `0x140b73700` (fn `0x140b736d0`), `0x87870004` @ `0x140b81387` (fn `0x140b812d0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x878E0004` | `2` | Unpack (S->C) | `0x878E0004` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x87870003` (`+364668`) | NAT: `0x07880002` (`0x140b33db0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x878E0006` | `3` | Pack (C->S) | `0x878E0003` | `()` | `0x00D9000C` (`self`) | `0x87870002` (`+364872`) [C++: `0x140b94005`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0793` (`1939`) — BarterSystem (NPC Barter Exchange & Barter Wallet) <a id="rpc-class-0x0793"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0794`, `0x0795`
- **Method Counts**: `1` Pack (C->S), `1` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 11 total)**: `0x10006959` (`Antispam_BarterRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10001795` (`Barter_IsBarterItem`), `0x100001A9` (`QuestEffect_ApplyRole`), `0x100001CF` (`Trait_Minstrel_LifeSinger`), `0x100002BD` (`CounterBox_CurrentCount`), `0x100006F5` (`UI_BackgroundSelection_UpbringingID`), `0x1000037E` (`ForwardSource_Combat_TraitCombo3`), `0x100017F9` (`Barter_ProfileArray`), `0x100017FA` (`Barter_Profile`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 19 total)**: `0x07960007` (`0x140388ca0`), `0x07960005` (`0x1403c51e0`), `0x07960009` (`0x140387b60`), `0x0796000B` (`0x140388da0`), `0x0796000D` (`0x140a72450`), `0x0796000F` (`0x140388d20`), `0x07910001` (`0x140b3a690`), `0x00E70001` (`0x140821170`), `0x07960001` (`0x140385130`), `0x0030000B` (`0x140385ab0`), `0x00190006` (`0x1403c2c90`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`) *(+5 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 7 total)**: `0x87950002` @ `0x140b61c28` (fn `0x140b61b80`), `0x87950005` @ `0x140b61d18` (fn `0x140b61c70`), `0x87950004` @ `0x140b7c755` (fn `0x140b7c560`), `0x87950009` @ `0x140b7caa7` (fn `0x140b7c900`), `0x87950006` @ `0x140b8711b` (fn `0x140b870d0`), `0x8795000D` @ `0x140b8c60d` (fn `0x140b8c540`), `0x8795000A` @ `0x140b8dadd` (fn `0x140b8da10`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x87930002` | `1` | Unpack (S->C) | `0x87930002` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x87940001` (`+395256`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87930005` | `2` | Pack (C->S) | `0x87930006` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x87950002` (`+396568`) [C++: `0x140b61c28`], `0x87950005` (`+396856`) [C++: `0x140b61d18`] | NAT: `0x00E70001` (`0x140821170`), `0x07960001` (`0x140385130`); PID: `0x100006F5` (`UI_BackgroundSelection_UpbringingID`), `0x1000037E` (`ForwardSource_Combat_TraitCombo3`) |

### Class `0x07A7` (`1959`) — HousingSystem (Purchase, Maintenance, Permissions, Decorations, Chests) <a id="rpc-class-0x07a7"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x07A6`, `0x0FF4`
- **Method Counts**: `26` Pack (C->S), `17` Unpack (S->C) (`43` total)
- **Referenced Properties (`pids.txt`, 101 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x1000038D` (`QuestEvent_RunDramaOnAdvance`), `0x10000071` (`UI_Map_MapNoteImage`), `0x10000033` (`MonsterLevel_StatModifierArray`), `0x10000150` (`UI_Fellowship_IID`), `0x10005E34` (`PaperItemService_SourceDID`), `0x100019FE` (`HousingDecoration_HookID`), `0x000005A0` (`Model_ParticleScale`), `0x1000685A` (`HousingDecoration_IsSkillSwap`), `0x100003D8` (`Conjunction_Role`), `0x100003D5` (`CombatState_ChainProtection_Duration`), `0x10000194` (`Skill_AttackHook_DPSAddMod_Progression`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x00000517` (`UICore_Dialog_type`), `0x00000420` (`UICore_Dialog_Range_Check_Entity`), `0x000004C5` (`UICore_Dialog_Range_Check_Distance`), `0x1000193C` (`UI_HousingPurchase_TemplateDID`) *(+83 more)*
- **Referenced WLib Strings (`0x0F`, 49 total)**: `"usage: /housing [cmd]"`, `"dump authority: display housing authority data."`, `"dump templates: list all neighborhood templates and their .WCs."`, `"dump hood [<neighborhood wc>]: display data for the current or given neighborhoo"`, `"dump house [<neighborhood wc>] [<house wc>]: display housekeeper data for the cu"`, `"dump owner [<IID>]: Display house info for selected or given player."`, `"dump perm [<neighborhood wc>] [<house wc>]: Dumps permissions for the current or"`, `"dump upkeep [<neighborhood wc>] [<house wc>]: Dumps upkeep data for the current "`, `"dump decorations [<neighborhood wc>] [<house wc>]: Dumps decoration data for the"`, `"dump storage [<IID>]: Display housing bank info for selected or given player."` *(+39 more)*
- **Referenced Native Functions (`0x15`, 131 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+117 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 66 total)**: `0x87A60005` @ `0x140b9650a` (fn `0x140b96460`), `0x87A60006` @ `0x140b95781` (fn `0x140b956d0`), `0x87A60007` @ `0x140b95991` (fn `0x140b95900`), `0x87A6000B` @ `0x140b95a2b` (fn `0x140b959e0`), `0x87A60010` @ `0x140b9564a` (fn `0x140b955a0`), `0x87A60018` @ `0x140b96879` (fn `0x140b96790`), `0x87A6001D` @ `0x140b9587a` (fn `0x140b957d0`), `0x87A60029` @ `0x140b93c5b` (fn `0x140b93bb0`), `0x87A6002D` @ `0x140b93d21` (fn `0x140b93cb0`), `0x87A60035` @ `0x140b9ac20` (fn `0x140b9abf0`), `0x87A60038` @ `0x140b95412` (fn `0x140b953c0`), `0x87A6003F` @ `0x140b9340c` (fn `0x140b933b0`) *(+54 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x87A70001` | `1` | Unpack (S->C) | `0x87A70001` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x87A60004` (`+799232`) | NAT: `0x01430011` (`0x140b3b6c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70003` | `2` | Pack (C->S) | `0x87A70004` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A60005` (`+798920`) [C++: `0x140b9650a`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70029` | `4` | Unpack (S->C) | `0x87A70029` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x87A60009` (`+799260`) | NAT: `0x01430012` (`0x140b3ae80`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A7002D` | `5` | Unpack (S->C) | `0x87A7002D` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A6000C` (`+799436`) | NAT: `0x01430013` (`0x140b36c90`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70030` | `7` | Pack (C->S) | `0x87A7002F` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x87A6000B` (`+796540`) [C++: `0x140b95a2b`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A70033` | `10` | Pack (C->S) | `0x87A70031` | `(i32/u32, i32/u32, i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A60006` (`+797208`) [C++: `0x140b95781`], `0x8FF40001` (`+1674116`) | NAT: `0x00E70001` (`0x140821170`), `0x00C7000C` (`0x1403afdf0`), `0x00C70003` (`0x14037c2f0`); PID: `0x10000033` (`MonsterLevel_StatModifierArray`), `0x100003D8` (`Conjunction_Role`), `0x100003D5` (`CombatState_ChainProtection_Duration`) |
| `0x87A70034` | `11` | Pack (C->S) | `0x87A70032` | `(i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A60007` (`+798772`) [C++: `0x140b95991`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A7003D` | `12` | Unpack (S->C) | `0x87A7003D` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A6000E` (`+799288`) | NAT: `0x01430014` (`0x140b3ae10`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70042` | `13` | Unpack (S->C) | `0x87A70042` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A60011` (`+799572`) | NAT: `0x01430015` (`0x140b369c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70046` | `15` | Pack (C->S) | `0x87A70043` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A60010` (`+796724`) [C++: `0x140b9564a`] | NAT: `0x00E70001` (`0x140821170`), `0x01430015` (`0x140b369c0`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A7004A` | `16` | Unpack (S->C) | `0x87A7004A` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A60014` (`+799344`) | NAT: `0x01430016` (`0x140b3af90`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A7004B` | `17` | Unpack (S->C) | `0x87A7004B` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x87A60015` (`+799316`) | NAT: `0x01430017` (`0x140b3c930`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70052` | `19` | Unpack (S->C) | `0x87A70052` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A60019` (`+800024`) | NAT: `0x01430018` (`0x140b3c2e0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70054` | `20` | Pack (C->S) | `0x87A7004F` | `(ref, i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A60018` (`+796392`) [C++: `0x140b96879`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70063` | `22` | Pack (C->S) | `0x87A70060` | `(i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A6008D` (`+797100`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70068` | `24` | Unpack (S->C) | `0x87A70068` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A6001E` (`+799604`) | NAT: `0x0143001A` (`0x140b36a60`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70069` | `25` | Pack (C->S) | `0x87A70064` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x87A6001D` (`+796884`) [C++: `0x140b9587a`] | NAT: `0x00E70001` (`0x140821170`), `0x0143001A` (`0x140b36a60`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A70072` | `27` | Pack (C->S) | `0x87A70070` | `(i32/u32, i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x87A60029` (`+795604`) [C++: `0x140b93c5b`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`); PID: `0x10000150` (`UI_Fellowship_IID`) |
| `0x87A7007E` | `29` | Pack (C->S) | `0x87A70077` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x87A6002D` (`+795364`) [C++: `0x140b93d21`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A7008D` | `31` | Pack (C->S) | `0x87A7008C` | `()` | `0x00D9000C` (`self`) | `0x87A60035` (`+793644`) [C++: `0x140b9ac20`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70091` | `33` | Pack (C->S) | `0x87A70090` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x87A60038` (`+795240`) [C++: `0x140b95412`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A700A5` | `36` | Unpack (S->C) | `0x87A700A5` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x87A6003E` (`+799372`) | NAT: `0x0143001F` (`0x140b3af00`); PID: `0x00000058` (`Tools_VisibilityArray`), `0x000006C3` (`Appearance_ClassList`) |
| `0x87A700B4` | `38` | Pack (C->S) | `0x87A700B3` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x87A6003F` (`+795192`) [C++: `0x140b9340c`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A700BD` | `40` | Unpack (S->C) | `0x87A700BD` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A60042` (`+799532`) | NAT: `0x01430020` (`0x140b39610`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A700BE` | `41` | Pack (C->S) | `0x87A700BB` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x87A60040` (`+795100`) [C++: `0x140b965ab`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A700FE` | `42` | Unpack (S->C) | `0x87A700FE` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x87A6004A` (`+799484`) | NAT: `0x01430022` (`0x140b36d30`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70103` | `44` | Pack (C->S) | `0x87A700FF` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x87A60049` (`+796632`) [C++: `0x140b95abb`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A7011A` | `45` | Unpack (S->C) | `0x87A7011A` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x87A6005A` (`+799648`) | NAT: `0x01310001` (`0x140b2ffb0`), `0x01310002` (`0x140b3f1f0`); PID: `0x00000054` (`PreprocOptions_TexGen_NoMipMaps`), `0x000006C3` (`Appearance_ClassList`) |
| `0x87A70124` | `47` | Pack (C->S) | `0x87A70123` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x87A60060` (`+792388`) [C++: `0x140b9ad2b`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70132` | `48` | Unpack (S->C) | `0x87A70132` | `()` | `0x00D60007(0x06C3)` | `0x87A60067` (`+799416`) | NAT: `0x01430025` (`0x140b33120`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x87A7014D` | `50` | Pack (C->S) | `0x87A7014C` | `(i64/u64, ref)` | `0x00D9000C` (`self`) | `0x87A6006A` (`+796276`) [C++: `0x140b9697a`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A70163` | `51` | Unpack (S->C) | `0x87A70163` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x87A60072` (`+799724`) | NAT: `0x01310001` (`0x140b2ffb0`), `0x01310002` (`0x140b3f1f0`); PID: `0x00000054` (`PreprocOptions_TexGen_NoMipMaps`), `0x000006C3` (`Appearance_ClassList`) |
| `0x87A7016A` | `53` | Pack (C->S) | `0x87A70169` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x87A60073` (`+791576`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A70170` | `55` | Pack (C->S) | `0x87A7016F` | `(f32, i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x87A60075` (`+794676`) [C++: `0x140b98f04`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70178` | `57` | Pack (C->S) | `0x87A70177` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x87A60082` (`+795496`) [C++: `0x140b96eb0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70187` | `59` | Pack (C->S) | `0x87A70186` | `()` | `0x00D9000C` (`self`) | `0x87A60089` (`+792460`) [C++: `0x140b9ada0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A701A9` | `61` | Pack (C->S) | `0x87A701A8` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x87A6008C` (`+791732`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x1000038D` (`QuestEvent_RunDramaOnAdvance`) |
| `0x87A701B5` | `62` | Unpack (S->C) | `0x87A701B5` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x87A60090` (`+799912`) | NAT: `0x01310001` (`0x140b2ffb0`), `0x01310002` (`0x140b3f1f0`); PID: `0x000000CE` (`PEDesc_Name`), `0x0000010C` (`GenericPhysicsController_PhysicsTerminalVelocity`), `0x00000060` (`PhysicsMovement`) |
| `0x87A701B7` | `63` | Unpack (S->C) | `0x87A701B7` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x87A6008F` (`+799800`) | NAT: `0x01310001` (`0x140b2ffb0`), `0x01310002` (`0x140b3f1f0`); PID: `0x000000CE` (`PEDesc_Name`), `0x0000010C` (`GenericPhysicsController_PhysicsTerminalVelocity`), `0x00000060` (`PhysicsMovement`) |
| `0x87A701F1` | `65` | Pack (C->S) | `0x87A701F0` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x87A6009C` (`+801900`) [C++: `0x140b99932`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A701F5` | `67` | Pack (C->S) | `0x87A701F2` | `(ref, i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x87A600A3` (`+794112`) [C++: `0x140b98de7`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000B` (`0x140385ab0`), `0x00990004` (`0x140389fa0`) |
| `0x87A70211` | `72` | Pack (C->S) | `0x87A7020F` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x87A600AF` (`+802044`) [C++: `0x140b99a72`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x87A70212` | `73` | Pack (C->S) | `0x87A70210` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x87A600AE` (`+801972`) [C++: `0x140b999d2`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0800` (`2048`) — CMountSystem (Mount Summon, Dismount & Movement Controller Sync) <a id="rpc-class-0x0800"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x09A5`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 41 total)**: `0x10002A90` (`Mount_CurrentMountType`), `0x10004157` (`Mount_Movement_Use_RampUp_Controller`), `0x100042B6` (`Movement_ServerAuthoritative_Controller`), `0x1000007E` (`Effect_Summoner`), `0x100000F2` (`Channeling_TitleString`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x0000011C` (`UICore_Dialog_modal`), `0x10000AAF` (`Mount_Is_Mounted`), `0x10000497` (`UI_VitalDisplay_MeterID`), `0x100006CF` (`Death_VitalType`), `0x100004A0` (`Trait_Captain_ImprovedBannerofHope`), `0x100004A1` (`Trait_Guardian_ReactiveBlocks`), `0x100004A2` (`KinshipTheme_Rank_RankID`), `0x10000067` (`UI_Pet_PetGroupBoxElement`), `0x10000983` (`Area_CurrentArea`), `0x10001DAB` (`Dungeon_Allowed_Mount_Types`) *(+23 more)*
- **Referenced WLib Strings (`0x0F`, 9 total)**: `"CMountSystem::OnPropertyChanged> Unhandled property"`, `"CMountSystem::OnPropertyChanged"`, `"usage: /mount [cmd]"`, `"resetnames <iid>: Resets all saved names for all mounts for the specified player"`, `"dumpnames <iid>: Dumps all saved names for all mounts for the specified player."`, `"> The mount skill item did not specify a mount skill to grant to"`, `"MountSystem::GetMountItemSkillToGrant"`, `"> The mount skill item did not specify a mount skill to grant to the tall race!"`, `"> The mount skill item did not specify a mount skill to grant to the short race!"`
- **Referenced Native Functions (`0x15`, 42 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00A90041` (`0x1403a4360`), `0x00270001` (`0x14038a7e0`), `0x09AC0001` (`0x140b39180`), `0x00B60029` (`0x1403c5260`), `0x00C7000C` (`0x1403afdf0`), `0x00330001` (`0x140384e80`), `0x00A60015` (`0x1403a9640`), `0x00A6001F` (`0x1403a9c20`), `0x00A6002B` (`0x1403b0040`), `0x00A60023` (`0x1403a88c0`), `0x00070001` (`0x140822020`) *(+28 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x89A50001` @ `0x140b940c2` (fn `0x140b9406f`), `0x89A50004` @ `0x140b96c5e` (fn `0x140b96bbe`), `0x89A5000E` @ `0x140b4e542` (fn `0x140b4e4f4`), `0x89A5000F` @ `0x140b70d82` (fn `0x140b70d20`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x88000007` | `2` | Pack (C->S) | `0x88000006` | `()` | `0x00D9000C` (`self`) | `0x89A50001` (`+1146224`) [C++: `0x140b940c2`], `0x89A50010` (`+1146736`) | NAT: `0x00DC0006` (`0x1403a5d90`), `0x00A9003F` (`0x1403a42c0`), `0x00DC000B` (`0x1403a6180`); PID: `0x10000AAF` (`Mount_Is_Mounted`) |
| `0x88000009` | `4` | Pack (C->S) | `0x88000008` | `(i32/u32, ref)` | `0x00D9000C` (`self`) | `0x89A50004` (`+1146248`) [C++: `0x140b96c5e`] | — |

### Class `0x081B` (`2075`) — OutfitSystem (Cosmetic Outfit Slot Equip, Unequip & Selection) <a id="rpc-class-0x081b"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0819`
- **Method Counts**: `4` Pack (C->S), `2` Unpack (S->C) (`6` total)
- **Referenced Properties (`pids.txt`, 39 total)**: `0x1000195C` (`Admin_PlayerTestMode`), `0x100001B7` (`Inventory_BindOnEquip`), `0x1000655D` (`Inventory_BindOnRaid`), `0x00000313` (`AnimationScript_Speed`), `0x00000121` (`UICore_Element_disallow_drag_in`), `0x100000BC` (`QuestTreasure_SelectedItem`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x000006C3` (`Appearance_ClassList`), `0x10000ACD` (`Item_ClothingColor`), `0x10002F15` (`Wardrobe_ItemStruct`), `0x10002F13` (`Wardrobe_ItemDID`), `0x10002F12` (`Wardrobe_ItemClothingColor`), `0x10001C4C` (`Antispam_OutfitRequest`), `0x00000321` (`AnimConfigurableStruct`), `0x10000005` (`GloryControl_DiminishingReturnsModifierProgression`), `0x10000018` (`GRSEvent_XP_XPAwarded`), `0x10000099` (`Patrol_PatrolPoint_AlignToHeading`), `0x00000212` (`DetectScriptOutput_FoundObjects`) *(+21 more)*
- **Referenced WLib Strings (`0x0F`, 45 total)**: `"Item:"`, `"<unknown>"`, `"!"`, `", but it should be bound to"`, `"is bound to"`, `"BindingSystem::SanityCheckBinding"`, `"<Unknown value>"`, `"]"`, `"["`, `")"` *(+35 more)*
- **Referenced Native Functions (`0x15`, 56 total)**: `0x00A9002B` (`0x1403909c0`), `0x00A9003A` (`0x1403a8360`), `0x01AA0002` (`0x140a7dae0`), `0x00E10006` (`0x1403b8050`), `0x00DD0001` (`0x1403b6150`), `0x00420001` (`0x140397330`), `0x00DD000F` (`0x1403b3090`), `0x00DD0008` (`0x1403b5b70`), `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0003` (`0x1403b6270`), `0x00990004` (`0x140389fa0`), `0x00DD0010` (`0x1403b3140`), `0x00C7001B` (`0x140387e70`), `0x0030000C` (`0x140384920`) *(+42 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 15 total)**: `0x88190024` @ `0x140b53be0` (fn `0x140b53bb0`), `0x8819001B` @ `0x140b5ad24` (fn `0x140b5acba`), `0x88190020` @ `0x140b7256c` (fn `0x140b72417`), `0x8819004F` @ `0x140b72749` (fn `0x140b7268c`), `0x88190067` @ `0x140b728c2` (fn `0x140b727d0`), `0x88190027` @ `0x140b729a2` (fn `0x140b72954`), `0x88190060` @ `0x140b74002` (fn `0x140b73fb4`), `0x88190011` @ `0x140b75cd2` (fn `0x140b75c84`), `0x88190062` @ `0x140b7a312` (fn `0x140b7a2c4`), `0x88190069` @ `0x140b8e39e` (fn `0x140b8e2a6`), `0x88190056` @ `0x140b8f711` (fn `0x140b8f6a0`), `0x88190016` @ `0x140b9110f` (fn `0x140b91025`) *(+3 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x881B0006` | `2` | Pack (C->S) | `0x881B0005` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x88190013` (`+1195384`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x08220001` (`0x140b3a290`), `0x01810004` (`0x140b2ef90`); PID: `0x10000099` (`Patrol_PatrolPoint_AlignToHeading`), `0x000003A0` (`AppearanceScript_Modifier`); MSG: `"]"`, `"> Invalid outfit ["` |
| `0x881B0008` | `4` | Pack (C->S) | `0x881B0007` | `(ref, i64/u64, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x88190019` (`+1193188`) | NAT: `0x00DD0001` (`0x1403b6150`); PID: `0x10000099` (`Patrol_PatrolPoint_AlignToHeading`), `0x00000212` (`DetectScriptOutput_FoundObjects`), `0x0000021C` (`DecalScript_Lifespan`); MSG: `"]"`, `"> Invalid outfit ["` |
| `0x881B000A` | `5` | Unpack (S->C) | `0x881B000A` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x88190023` (`+1198124`) | NAT: `0x08220001` (`0x140b3a290`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x881B000C` | `7` | Pack (C->S) | `0x881B000B` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x88190039` (`+1194924`) | NAT: `0x00DD0001` (`0x1403b6150`); PID: `0x10000099` (`Patrol_PatrolPoint_AlignToHeading`), `0x00000338` (`CMoveKeys_Left`), `0x00000343` (`UICore_ListBox_layout_only_visible`); MSG: `"]"`, `"> Invalid outfit ["` |
| `0x881B000E` | `9` | Pack (C->S) | `0x881B000D` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x88190053` (`+1195672`) | NAT: `0x08220002` (`0x140b3a330`) |
| `0x881B0010` | `10` | Unpack (S->C) | `0x881B0010` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8819005E` (`+1198152`) | NAT: `0x08220002` (`0x140b3a330`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x083A` (`2106`) — AppearanceUI / Barber Character Customization System (0x0837) <a id="rpc-class-0x083a"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0837`
- **Method Counts**: `1` Pack (C->S), `2` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 12 total)**: `0x00000157` (`Appearance_KeyInfo`), `0x10006DB5` (`AC_Mesh_Head`), `0x000000A4` (`Appearance_AprFile`), `0x00000289` (`Appearance_Modifier`), `0x0000054F` (`Appearance_InstanceList`), `0x10000761` (`AppearanceUI_Controls`), `0x100004B2` (`AppearanceUI_ControlPropertyName`), `0x100001E3` (`AppearanceUI_DyeMapControl`), `0x10000A24` (`AppearanceUI_DyeMapImage`), `0x100003FC` (`Death_MPDefeatInfo`), `0x100003FD` (`PrivateEncounterTemplate_InstanceMilestone`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 20 total)**: `0x00B0000B` (`0x140385130`), `0x00B00003` (`0x1403beb80`), `0x00270001` (`0x14038a7e0`), `0x00270006` (`0x1403ae7b0`), `0x00270003` (`0x14038a870`), `0x0027003A` (`0x1403993f0`), `0x00270036` (`0x140399330`), `0x00270030` (`0x1403903e0`), `0x00270031` (`0x140390320`), `0x00270010` (`0x14039dc40`), `0x0027001D` (`0x140391c80`), `0x083B0002` (`0x140a6a180`), `0x00B00002` (`0x1403c68d0`), `0x00270021` (`0x140392a50`) *(+6 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 5 total)**: `0x8837000D` @ `0x140b621c9` (fn `0x140b6219d`), `0x88370009` @ `0x140b7dcde` (fn `0x140b7dc23`), `0x8837000B` @ `0x140b861c4` (fn `0x140b8615a`), `0x8837000E` @ `0x140b862e4` (fn `0x140b8627a`), `0x8837000A` @ `0x140b86439` (fn `0x140b8640d`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x883A0003` | `2` | Unpack (S->C) | `0x883A0003` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x88370008` (`+292784`) | NAT: `0x08380001` (`0x140b3a590`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x883A0004` | `3` | Pack (C->S) | `0x883A0001` | `(ref)` | `0x00D9000C` (`self`) | `0x8837000D` (`+292584`) [C++: `0x140b621c9`] | NAT: `0x00B0000D` (`0x140385130`); PID: `0x100003FC` (`Death_MPDefeatInfo`), `0x100003FD` (`PrivateEncounterTemplate_InstanceMilestone`) |
| `0x883A0005` | `4` | Unpack (S->C) | `0x883A0005` | `()` | `0x00D60007(0x06C3)` | `0x8837000F` (`+292808`) | NAT: `0x08380003` (`0x140b31510`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0849` (`2121`) — CItemAdvancementSystem (Legendary Items, Reforge, Legacies, Imbuement) <a id="rpc-class-0x0849"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x084A`, `0x084B`, `0x08F1`
- **Method Counts**: `16` Pack (C->S), `41` Unpack (S->C) (`57` total)
- **Referenced Properties (`pids.txt`, 125 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x1000195C` (`Admin_PlayerTestMode`), `0x100001B7` (`Inventory_BindOnEquip`), `0x1000655D` (`Inventory_BindOnRaid`), `0x100000A7` (`Skill_RequiredLineOfSight`), `0x10000480` (`Usage_ScrollSlotMergeCheckID`), `0x10000273` (`Skill_AlwaysApplyUserEffects`), `0x10000AAE` (`Combat_DPS_LUT`), `0x1000317A` (`Item_RequiredCombatPropertyType`), `0x00000354` (`Name`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`), `0x10000079` (`Vital_PowerPeaceRegenAddMod`), `0x10000447` (`Signals@NPC::CallForHelp::Source`), `0x10000485` (`Generator_MaxRegenDelay`), `0x100000DB` (`Auction_Cleanup_Delay`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`) *(+107 more)*
- **Referenced WLib Strings (`0x0F`, 44 total)**: `"Could not find The Player!"`, `"CItemAdvancementSystem::UI_GetLegacyDescription"`, `"CItemAdvancementSystem::UI_InqLegaciesToExtractInfo"`, `"Could not find item"`, `"Unable to get Effect List for Item"`, `"CItemAdvancementSystem::UI_InqFilteredImbuedLegacyInfoArray"`, `"Couldn't get the IA registry!"`, `"CItemAdvancementSystem::BuildImbuedLegacyInfoArray"`, `"]!"`, `"Unable to get singleton from ["` *(+34 more)*
- **Referenced Native Functions (`0x15`, 185 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+171 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 52 total)**: `0x884A0013` @ `0x140b96b2c` (fn `0x140b96a30`), `0x884A0015` @ `0x140b7aa0c` (fn `0x140b7a9b0`), `0x884A001A` @ `0x140b93b63` (fn `0x140b93ac2`), `0x884A0039` @ `0x140b7ab72` (fn `0x140b7aaf0`), `0x884A007D` @ `0x140b9c6f7` (fn `0x140b9c670`), `0x884A0093` @ `0x140b7aaac` (fn `0x140b7aa50`), `0x884A00AE` @ `0x140b95b93` (fn `0x140b95b00`), `0x884A00C7` @ `0x140b96e18` (fn `0x140b96d70`), `0x884A00C9` @ `0x140b93a43` (fn `0x140b93970`), `0x884A00CD` @ `0x140b93781` (fn `0x140b93720`), `0x884A0080` @ `0x140b4e241` (fn `0x140b4e1e0`), `0x884A0081` @ `0x140b57296` (fn `0x140b57220`) *(+40 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8849000C` | `1` | Unpack (S->C) | `0x8849000C` | `(ref)` | `0x00D60007(0x06C3)` | `0x884B0005` (`+905420`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849000E` | `3` | Pack (C->S) | `0x8849000D` | `(i32/u32, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x884A0010` (`+868528`) | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`); PID: `0x10000480` (`Usage_ScrollSlotMergeCheckID`), `0x10000273` (`Skill_AlwaysApplyUserEffects`) |
| `0x88490011` | `4` | Unpack (S->C) | `0x88490011` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0008` (`+905488`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490013` | `5` | Unpack (S->C) | `0x88490013` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0009` (`+905516`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490016` | `7` | Pack (C->S) | `0x88490015` | `(ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x884A0013` (`+872004`) [C++: `0x140b96b2c`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`), `0x00340004` (`0x140382f70`); PID: `0x10000480` (`Usage_ScrollSlotMergeCheckID`), `0x10000273` (`Skill_AlwaysApplyUserEffects`), `0x00000354` (`Name`) |
| `0x88490018` | `9` | Pack (C->S) | `0x88490017` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x884A0015` (`+871480`) [C++: `0x140b7aa0c`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x88490019` | `10` | Unpack (S->C) | `0x88490019` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B000C` (`+905600`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849001C` | `12` | Pack (C->S) | `0x8849001B` | `(i32/u32, i32/u32, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x884A001A` (`+872900`) [C++: `0x140b93b63`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`); PID: `0x10000480` (`Usage_ScrollSlotMergeCheckID`), `0x10000273` (`Skill_AlwaysApplyUserEffects`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`) |
| `0x8849001F` | `13` | Unpack (S->C) | `0x8849001F` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x884B000E` (`+905628`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490022` | `14` | Unpack (S->C) | `0x88490022` | `(ref, ref, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0010` (`+905448`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490025` | `15` | Unpack (S->C) | `0x88490025` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0012` (`+905696`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490026` | `16` | Unpack (S->C) | `0x88490026` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0011` (`+905660`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490028` | `18` | Pack (C->S) | `0x88490027` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x884A006D` (`+874652`) | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`); PID: `0x10000485` (`Generator_MaxRegenDelay`) |
| `0x8849002A` | `20` | Pack (C->S) | `0x88490029` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x884A0027` (`+867752`), `0x88F10001` (`+1674636`) | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`), `0x00A60024` (`0x140391670`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x1000238A` (`ItemAdvancement_ItemIID`), `0x1000238B` (`ItemAdvancement_ItemSlot`) |
| `0x8849002B` | `21` | Unpack (S->C) | `0x8849002B` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0015` (`+904928`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849002E` | `22` | Unpack (S->C) | `0x8849002E` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0016` (`+905572`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490030` | `24` | Pack (C->S) | `0x8849002F` | `(i32/u32, i32/u32, i64/u64, ref)` | `0x00D9000C` (`self`) | `0x884A002F` (`+875548`) | PID: `0x10000273` (`Skill_AlwaysApplyUserEffects`) |
| `0x88490031` | `25` | Unpack (S->C) | `0x88490031` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0018` (`+905732`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490033` | `26` | Unpack (S->C) | `0x88490033` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0019` (`+904896`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490036` | `28` | Pack (C->S) | `0x88490035` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x884A0039` (`+871344`) [C++: `0x140b7ab72`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x88490037` | `29` | Unpack (S->C) | `0x88490037` | `(i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B001B` (`+905316`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849003B` | `30` | Unpack (S->C) | `0x8849003B` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B001C` (`+905388`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849003E` | `32` | Pack (C->S) | `0x8849003D` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x884A0045` (`+879628`) | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`); PID: `0x100027CF` (`Antispam_ItemLevelingToggle`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x100026E1` (`ItemAdvancement_MustBeEquippedToGainXP`) |
| `0x88490040` | `33` | Unpack (S->C) | `0x88490040` | `(ref, ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B001E` (`+905184`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490042` | `34` | Unpack (S->C) | `0x88490042` | `(i32/u32, ref, i32/u32, i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B001F` (`+904984`) | NAT: `0x00C70019` (`0x14039dba0`), `0x00C70010` (`0x14038bee0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490044` | `35` | Unpack (S->C) | `0x88490044` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x884B0020` (`+905152`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490045` | `36` | Unpack (S->C) | `0x88490045` | `(i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0021` (`+905796`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490046` | `37` | Unpack (S->C) | `0x88490046` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0022` (`+905764`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490051` | `38` | Unpack (S->C) | `0x88490051` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0029` (`+905936`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490052` | `39` | Unpack (S->C) | `0x88490052` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0028` (`+905880`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490053` | `40` | Unpack (S->C) | `0x88490053` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x884B002A` (`+905964`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849005D` | `41` | Unpack (S->C) | `0x8849005D` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x884B002B` (`+905852`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490062` | `43` | Pack (C->S) | `0x88490061` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x884A007D` (`+881684`) [C++: `0x140b9c6f7`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x88490064` | `44` | Unpack (S->C) | `0x88490064` | `(i32/u32, ref, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B002E` (`+905112`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490068` | `45` | Unpack (S->C) | `0x88490068` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0030` (`+905080`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849006A` | `47` | Unpack (S->C) | `0x8849006A` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0032` (`+905908`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849006C` | `48` | Pack (C->S) | `0x88490069` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x884A0093` (`+871612`) [C++: `0x140b7aaac`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x8849006E` | `49` | Unpack (S->C) | `0x8849006E` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0033` (`+905356`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849006F` | `50` | Unpack (S->C) | `0x8849006F` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x884B0034` (`+905256`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490071` | `51` | Unpack (S->C) | `0x88490071` | `()` | `0x00D60007(0x06C3)` | `0x884B0035` (`+905828`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490073` | `52` | Unpack (S->C) | `0x88490073` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x884B0036` (`+905288`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490075` | `53` | Unpack (S->C) | `0x88490075` | `(i32/u32, ref, ref)` | `0x00D60007(0x06C3)` | `0x884B0037` (`+905220`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490077` | `54` | Unpack (S->C) | `0x88490077` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0038` (`+905544`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849007A` | `56` | Pack (C->S) | `0x88490079` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x884A00AE` (`+872640`) [C++: `0x140b95b93`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x88490080` | `57` | Unpack (S->C) | `0x88490080` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B003D` (`+905992`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490082` | `58` | Unpack (S->C) | `0x88490082` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B003C` (`+906028`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490083` | `59` | Unpack (S->C) | `0x88490083` | `(i32/u32, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B003E` (`+906124`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490085` | `60` | Unpack (S->C) | `0x88490085` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B003F` (`+906096`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490087` | `61` | Unpack (S->C) | `0x88490087` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0040` (`+906160`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849008B` | `62` | Unpack (S->C) | `0x8849008B` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0042` (`+906060`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849008D` | `63` | Unpack (S->C) | `0x8849008D` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0043` (`+906224`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490090` | `64` | Unpack (S->C) | `0x88490090` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x884B0044` (`+906196`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88490092` | `66` | Pack (C->S) | `0x88490091` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x884A00C7` (`+872416`) [C++: `0x140b96e18`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |
| `0x88490094` | `68` | Pack (C->S) | `0x88490093` | `(i32/u32, i64/u64, ref)` | `0x00D9000C` (`self`) | `0x884A00C9` (`+873232`) [C++: `0x140b93a43`] | NAT: `0x00E70001` (`0x140821170`), `0x0068000B` (`0x1403817b0`), `0x00680004` (`0x1403bead0`); PID: `0x10000273` (`Skill_AlwaysApplyUserEffects`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`) |
| `0x88490096` | `70` | Pack (C->S) | `0x88490095` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x884A00CA` (`+868776`) | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000480` (`Usage_ScrollSlotMergeCheckID`) |
| `0x88490098` | `71` | Unpack (S->C) | `0x88490098` | `(ref)` | `0x00D60007(0x06C3)` | `0x884B0048` (`+904956`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8849009A` | `73` | Pack (C->S) | `0x88490099` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x884A00CD` (`+866876`) [C++: `0x140b93781`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |

### Class `0x0859` (`2137`) — HobbySystem (Fishing & Hobby Proficiency, Execution & Rewards) <a id="rpc-class-0x0859"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x084E`
- **Method Counts**: `2` Pack (C->S), `4` Unpack (S->C) (`6` total)
- **Referenced Properties (`pids.txt`, 21 total)**: `0x10001D75` (`Hobby_Name`), `0x10001D7B` (`Hobby_Description`), `0x10001E6B` (`Hobby_SmallIcon`), `0x10001E69` (`Hobby_Icon`), `0x10001E6A` (`Hobby_LargeIcon`), `0x10001DC1` (`Hobby_Proficiency_PropertyName`), `0x10001DC6` (`Hobby_ProficiencyModifier_PropertyName`), `0x100072A4` (`Hobby_Birding_DiscoveredBirds`), `0x000001DB` (`ClearanceBBoxMin3`), `0x00000063` (`Camera_Pitch`), `0x1000729E` (`Hobby_TerritoryProficiencyGainList`), `0x10001EF6` (`Hobby_Status`), `0x10001DD5` (`Hobby_ToolCategory`), `0x10001F34` (`Hobby_OfferedHobbiesArray`), `0x10007313` (`Hobby_MetaScore`), `0x10000408` (`Hotspot_ActivateDelay`), `0x10000407` (`Tripwire_Broadcast_Radius`), `0x10000409` (`UI_Auction_ItemIID`) *(+3 more)*
- **Referenced WLib Strings (`0x0F`, 24 total)**: `"Usage: /hobby <dump\|dumpoutput\|execute\|train\|bestowitems\|setproficiency\|re"`, `"* bestowItems: usage: bestowitems <hobby did>"`, `"Re-bestows the items associated with the specified hobby to the admin."`, `"* dump: usage: dump [playername]"`, `"This command will display the contents of the players hobby registry. If no"`, `"name is specified, the command will default to the admin as the target."`, `"* dumpoutput: usage: dumpoutput <hobby did> [proficiency]"`, `"This command displays the potential rewards for successfully executing"`, `"the given Hobby at the given proficiency level(optional)."`, `"* Execute: usage: execute <hobby did>"` *(+14 more)*
- **Referenced Native Functions (`0x15`, 37 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00B60029` (`0x1403c5260`), `0x00E10005` (`0x1403ca7d0`), `0x00E10006` (`0x1403b8050`), `0x00120007` (`0x1403bee50`), `0x00130005` (`0x1403c50c0`), `0x00130002` (`0x1403b9f80`), `0x00110002` (`0x1403c68d0`), `0x00130001` (`0x1403c4e10`), `0x08720001` (`0x140b36fb0`), `0x00A6002B` (`0x1403b0040`), `0x00A60015` (`0x1403a9640`) *(+23 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 11 total)**: `0x884E000F` @ `0x140b6a47b` (fn `0x140b6a430`), `0x884E0005` @ `0x140b6e13d` (fn `0x140b6e070`), `0x884E000C` @ `0x140b6e244` (fn `0x140b6e1c0`), `0x884E0010` @ `0x140b6e310` (fn `0x140b6e2e0`), `0x884E001B` @ `0x140b6e3b2` (fn `0x140b6e350`), `0x884E0014` @ `0x140b6e484` (fn `0x140b6e400`), `0x884E0011` @ `0x140b7a24b` (fn `0x140b7a200`), `0x884E000A` @ `0x140b7c0aa` (fn `0x140b7bfe3`), `0x884E001A` @ `0x140b7effc` (fn `0x140b7ef40`), `0x884E000B` @ `0x140b8e0e2` (fn `0x140b8e094`), `0x884E000D` @ `0x140b9b177` (fn `0x140b9b0f0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x88590002` | `2` | Pack (C->S) | `0x88590001` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x884E0003` (`+780820`) | — |
| `0x88590004` | `4` | Pack (C->S) | `0x88590003` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x884E0004` (`+781188`) | — |
| `0x8859000B` | `5` | Unpack (S->C) | `0x8859000B` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x884E0007` (`+781512`) | NAT: `0x08720004` (`0x140b3ad10`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88590011` | `6` | Unpack (S->C) | `0x88590011` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x884E0008` (`+781292`) | NAT: `0x08720002` (`0x140b30540`); PID: `0x00000063` (`Camera_Pitch`), `0x000006C3` (`Appearance_ClassList`) |
| `0x88590026` | `7` | Unpack (S->C) | `0x88590026` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x884E001C` (`+781420`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88590028` | `8` | Unpack (S->C) | `0x88590028` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x884E001D` (`+781468`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x08CB` (`2251`) — NPCCommentSystem (Conditional NPC Bark / Drama Trigger) <a id="rpc-class-0x08cb"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x08CC`
- **Method Counts**: `1` Pack (C->S), `1` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 9 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10002128` (`NPCComment_DIDCommentArray`), `0x10000065` (`UI_Vendor_FilterType`), `0x000008C7` (`GeneratorTracker_Entity_Generator_PositionSetID_Property`), `0x100002BD` (`CounterBox_CurrentCount`), `0x00000052` (`PreprocOptions_FontGen_BackgroundVertBorderPixels`), `0x1000043B` (`Effect_Impulse_ScriptParam_ImpulseVector`), `0x0000005C` (`APC.ExceptGroups`), `0x0000007B` (`PEDesc_TimeLimit`)
- **Referenced WLib Strings (`0x0F`, 8 total)**: `"] without a comment array!"`, `"> being called on NPC ["`, `"NPCCommentSystem::CanPlayerDoNPCCommentDrama"`, `"] isn't on that NPC!"`, `"] but the comment ["`, `"]"`, `"Failed to get NPCComment ["`, `"NPCCommentSystem::IsCommentActiveForPlayer"`
- **Referenced Native Functions (`0x15`, 14 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x08CA0001` (`0x140b3b5c0`), `0x00E70001` (`0x140821170`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00110002` (`0x1403c68d0`), `0x001A0001` (`0x1403c4ed0`), `0x0011000B` (`0x1403817b0`), `0x00C7000C` (`0x1403afdf0`), `0x0030000B` (`0x140385ab0`), `0x00190025` (`0x1403b9150`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 7 total)**: `0x88CC0008` @ `0x140b66037` (fn `0x140b65fdc`), `0x88CC0007` @ `0x140b71674` (fn `0x140b715f0`), `0x88CC0004` @ `0x140b71794` (fn `0x140b71710`), `0x88CC0003` @ `0x140b718b4` (fn `0x140b71830`), `0x88CC0006` @ `0x140b7199b` (fn `0x140b71950`), `0x88CC0005` @ `0x140b71a64` (fn `0x140b719e0`), `0x88CC0002` @ `0x140b7b78e` (fn `0x140b7b6ad`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x88CB0001` | `1` | Unpack (S->C) | `0x88CB0001` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x88CC0001` (`+1187360`) | NAT: `0x08CA0001` (`0x140b3b5c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x88CB0004` | `3` | Pack (C->S) | `0x88CB0003` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x88CC0008` (`+1187952`) [C++: `0x140b66037`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0908` (`2312`) — GameplayOptionsProfile XML Layout Import/Export Sync (0x0909) <a id="rpc-class-0x0908"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0909`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 1 total)**: `0x00080009` (`0x140820490`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x89090007` @ `0x140b67e28` (fn `0x140b67d80`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x89080002` | `2` | Pack (C->S) | `0x89080001` | `()` | `0x00D9000C` (`self`) | `0x89090002` (`+304044`) | — |

### Class `0x0958` (`2392`) — ShortcutPage / Quickslot Page Sync System (Shortcut_Current_Page, 0x0959) <a id="rpc-class-0x0958"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0959`
- **Method Counts**: `3` Pack (C->S), `4` Unpack (S->C) (`7` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10004CAA` (`Shortcut_Current_Page`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 11 total)**: `0x00B60029` (`0x1403c5260`), `0x015B0001` (`0x140b3d4f0`), `0x019F002A` (`0x140a81990`), `0x0011000B` (`0x1403817b0`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x01C60006` (`0x1403a7700`), `0x01C60008` (`0x1403a6fd0`), `0x00E70001` (`0x140821170`), `0x01C60007` (`0x140a85540`), `0x019F0008` (`0x140a8c1e0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x89590007` @ `0x140b5af85` (fn `0x140b5af50`), `0x89590008` @ `0x140b5da45` (fn `0x140b5da10`), `0x89590005` @ `0x140b59c6d` (fn `0x140b59ba0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x89580001` | `1` | Unpack (S->C) | `0x89580001` | `(ref, i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x89590001` (`+1486288`) | NAT: `0x015B0001` (`0x140b3d4f0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x89580004` | `3` | Pack (C->S) | `0x89580003` | `(ref)` | `0x00D9000C` (`self`) | `0x86C201F6` (`+84464`) | NAT: `0x00B60029` (`0x1403c5260`) |
| `0x89580005` | `4` | Unpack (S->C) | `0x89580005` | `(ref, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x89590006` (`+1486672`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x89580009` | `7` | Pack (C->S) | `0x89580008` | `()` | `0x00D9000C` (`self`) | `0x89590008` (`+1487248`) [C++: `0x140b5da45`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8958000A` | `8` | Pack (C->S) | `0x89580007` | `()` | `0x00D9000C` (`self`) | `0x89590007` (`+1487200`) [C++: `0x140b5af85`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8958000B` | `9` | Unpack (S->C) | `0x8958000B` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8959000A` (`+1486344`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8958000D` | `10` | Unpack (S->C) | `0x8958000D` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8959000C` (`+1486596`) | NAT: `0x01C60006` (`0x1403a7700`), `0x01C60008` (`0x1403a6fd0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x096D` (`2413`) — DRSSystem (Dynamic Resolution / Delayed Reward System Admin /drs) <a id="rpc-class-0x096d"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x096F`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`, 2 total)**: `"usage: /drs [cmd]"`, `"doheartbeat: forces a heartbeat to happen now."`
- **Referenced Native Functions (`0x15`, 3 total)**: `0x00B60029` (`0x1403c5260`), `0x00E10005` (`0x1403ca7d0`), `0x00DD0008` (`0x1403b5b70`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x896D0003` | `1` | Pack (C->S) | `0x896D0004` | `(ref)` | `0x00D9000D` (`local_0`) | `0x896F0001` (`+1455848`) | NAT: `0x00B60029` (`0x1403c5260`) |

### Class `0x09BB` (`2491`) — Dialog Callback Notification Stub (0x09BD slot 84) <a id="rpc-class-0x09bb"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x09BD`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 36 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+22 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x89BB0003` | `2` | Unpack (S->C) | `0x89BB0003` | `(ref)` | `0x00D60007(0x06C3)` | `0x89BD0002` (`+1527352`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x09EB` (`2539`) — CWebStoreSystem (In-Game Store URL, Coupons & Try-On Preview) <a id="rpc-class-0x09eb"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x09E8`
- **Method Counts**: `1` Pack (C->S), `3` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 35 total)**: `0x000006C3` (`Appearance_ClassList`), `0x00000B9E` (`World_Webstore_Mode`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10006F5A` (`WebStore_Antispam_LastTimeCheck`), `0x10006F59` (`World_Webstore_Antispam_TimeBetweenStoreRequests_Sec`), `0x10003BC9` (`GOP_ToggleWebStoreCouponRedemptionScript`), `0x00000231` (`WB_VisualizedType_List`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x0000025D` (`BeamScript_HeadEntity`), `0x000001D7` (`Account_ClientLanguage`), `0x1000637E` (`World_Webstore_SandboxUrl`), `0x00000288` (`MapNote_Type`), `0x10002ED5` (`World_Webstore_BaseUrl`), `0x100067FF` (`World_Webstore_BaseUrl_SystemBrowser`), `0x1000393F` (`Antispam_AccountRefreshRequest`), `0x00000BA6` (`World_Webstore_SandboxEnvironment`), `0x1000389D` (`Item_IsPurchasedWebStoreItem`), `0x000009E4` (`PreprocOptions_ConvertMaterialGeometryToBillboards`) *(+17 more)*
- **Referenced WLib Strings (`0x0F`, 73 total)**: `"filtertokens"`, `"navigate"`, `"tryonmountappearancetrait"`, `"browserversion"`, `"decodefiltertokens"`, `"Usage: /webstore tryonmountappearancetrait webstore_it_trait_mtx_mountedapr_head"`, `"Please provide a URL."`, `"Filter Tokens:"`, `"Could not get filter tokens."`, `"Must specify one or more filter tokens to decode."` *(+63 more)*
- **Referenced Native Functions (`0x15`, 57 total)**: `0x00E70001` (`0x140821170`), `0x00B60001` (`0x1403b8d80`), `0x00B60014` (`0x1403c6650`), `0x00E10016` (`0x1403ba9d0`), `0x0E1F0001` (`0x140b448e0`), `0x0181001C` (`0x140b2dac0`), `0x0A1A0005` (`0x140b30ce0`), `0x00DD0001` (`0x1403b6150`), `0x0CDF0002` (`0x140a90ef0`), `0x00B0000B` (`0x140385130`), `0x00B00003` (`0x1403beb80`), `0x00A9003F` (`0x1403a42c0`), `0x00270001` (`0x14038a7e0`), `0x0A1A0001` (`0x140b43800`) *(+43 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 14 total)**: `0x89E80001` @ `0x140b5ede5` (fn `0x140b5edb0`), `0x89E80073` @ `0x140b51803` (fn `0x140b51760`), `0x89E80074` @ `0x140b51983` (fn `0x140b518e0`), `0x89E80019` @ `0x140b5ac30` (fn `0x140b5ac00`), `0x89E80007` @ `0x140b5d7f0` (fn `0x140b5d730`), `0x89E8000E` @ `0x140b5d870` (fn `0x140b5d820`), `0x89E8000D` @ `0x140b5d8f0` (fn `0x140b5d8a0`), `0x89E80039` @ `0x140b68573` (fn `0x140b68510`), `0x89E80051` @ `0x140b686b3` (fn `0x140b68650`), `0x89E80014` @ `0x140b7cdbd` (fn `0x140b7ccf0`), `0x89E80021` @ `0x140b8914d` (fn `0x140b89080`), `0x89E80012` @ `0x140b89287` (fn `0x140b891d0`) *(+2 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x89EB0006` | `2` | Pack (C->S) | `0x89EB0004` | `()` | `0x00D9000D` (`local_0`) | `0x89E80001` (`+1819676`) [C++: `0x140b5ede5`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x89EB000B` | `3` | Unpack (S->C) | `0x89EB000B` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x89E80008` (`+1820808`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x89EB000E` | `4` | Unpack (S->C) | `0x89EB000E` | `()` | `0x00D60007(0x06C3)` | `0x89E8004D` (`+1822200`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x89EB0017` | `5` | Unpack (S->C) | `0x89EB0017` | `()` | `0x00D60007(0x06C3)` | `0x89E80064` (`+1820840`) | NAT: `0x0A1A0006` (`0x140b3e8e0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x09EC` (`2540`) — CSkirmishSystem (Skirmish Statistics & Skirmish Gating Sync) <a id="rpc-class-0x09ec"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0A52`
- **Method Counts**: `0` Pack (C->S), `4` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 12 total)**: `0x000006C3` (`Appearance_ClassList`), `0x00000051` (`PreprocOptions_PlaceGen_Farview`), `0x00000082` (`AfterEffectScript_Priority`), `0x00000093` (`ClearanceCylHeight2`), `0x000000A7` (`ParsedPlaceGen_Places`), `0x000000C8` (`UICore_Button_hot_click_interval`), `0x00000A5B` (`MeshFXScript_AppearanceModifier`), `0x00000064` (`LedgePhysicsController_LedgePlaneDistance`), `0x10003180` (`SkirmishGating_SkirmishesCurrentlyRun`), `0x00000048` (`ClearanceCylHeight1`), `0x000000C4` (`UICore_Element_min_width`), `0x100037F4` (`Skirmish_PlayerLevelAtSkirmishStart`)
- **Referenced WLib Strings (`0x0F`, 35 total)**: `">"`, `"<Unknown Property"`, `":"`, `"LogUtils::LogAndAssert"`, `"] was not a skirmish stat property!"`, `"CSkirmishSystem::RecvCEvt_UpdateSkirmishStat> ERROR: Property ["`, `"CSkirmishSystem::RecvCEvt_UpdateSkirmishStat"`, `"]"`, `"CSkirmishSystem::RecvCEvt_IncrementSkirmishStat> ERROR: Could not get the Proper"`, `"CSkirmishSystem::RecvCEvt_IncrementSkirmishStat"` *(+25 more)*
- **Referenced Native Functions (`0x15`, 35 total)**: `0x00790003` (`0x140396b20`), `0x00DD000F` (`0x1403b3090`), `0x00DD0001` (`0x1403b6150`), `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0003` (`0x1403b6270`), `0x00230012` (`0x1403c5400`), `0x0A510001` (`0x140b36e30`), `0x00230004` (`0x1403bc7d0`), `0x00A60002` (`0x1403b4950`), `0x00230002` (`0x1403c9a70`), `0x00270001` (`0x14038a7e0`), `0x0A510002` (`0x140b48fc0`), `0x00270006` (`0x1403ae7b0`), `0x00270054` (`0x14038f2c0`) *(+21 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x8A520008` @ `0x140b56a5b` (fn `0x140b56a10`), `0x8A52000A` @ `0x140b62895` (fn `0x140b62730`), `0x8A52000D` @ `0x140b649b0` (fn `0x140b64980`), `0x8A52000B` @ `0x140b64a30` (fn `0x140b64a00`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x89EC0007` | `1` | Unpack (S->C) | `0x89EC0007` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x8A520001` (`+1510792`) | NAT: `0x00230004` (`0x1403bc7d0`), `0x00A60002` (`0x1403b4950`), `0x00230002` (`0x1403c9a70`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000051` (`PreprocOptions_PlaceGen_Farview`); MSG: `"] was not a skirmish stat property!"`, `"CSkirmishSystem::RecvCEvt_UpdateSkirmishStat>"` |
| `0x89EC000A` | `2` | Unpack (S->C) | `0x89EC000A` | `(ref)` | `0x00D60007(0x06C3)` | `0x8A520002` (`+1510736`) | NAT: `0x00230012` (`0x1403c5400`), `0x0A510001` (`0x140b36e30`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x89EC000E` | `3` | Unpack (S->C) | `0x89EC000E` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x8A520003` (`+1511116`) | NAT: `0x00230004` (`0x1403bc7d0`), `0x00270001` (`0x14038a7e0`), `0x00270006` (`0x1403ae7b0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000082` (`AfterEffectScript_Priority`), `0x00000093` (`ClearanceCylHeight2`); MSG: `"]"`, `"CSkirmishSystem::RecvCEvt_IncrementSkirmishSt"` |
| `0x89EC0021` | `4` | Unpack (S->C) | `0x89EC0021` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8A520007` (`+1512180`) | NAT: `0x0A510003` (`0x140b427d0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0A04` (`2564`) — BankRepositorySystem (Vault, Shared Storage, Escrow & Chest Operations) <a id="rpc-class-0x0a04"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0A02`
- **Method Counts**: `9` Pack (C->S), `4` Unpack (S->C) (`13` total)
- **Referenced Properties (`pids.txt`, 14 total)**: `0x10005E50` (`Bank_Type`), `0x10000132` (`Death_EffectOnBehalfOf`), `0x0000004E` (`PathMap_BlockingBoxMin`), `0x10000513` (`UI_Escort_EscortNameElement`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`), `0x00000137` (`MoveToScript_MaximumDistance`), `0x1000005E` (`Tripwire_DeactivateOnTrip`), `0x100004E2` (`Craft_Cook_CompletedMasteryLevel`), `0x100059BD` (`Item_IsVersioningToBankRepository`), `0x10003BEE` (`Antispam_BankRepositoryRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10000510` (`UI_Examination_Tooltip_DisplayDefault`), `0x10000512` (`UI_Examination_Tooltip_Override_Layout`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 16 total)**: `"> Failed to get the bank owner out of the given collection! Collection Info:"`, `"BankRepositorySystem::CreateBankAddressForServiceCollection"`, `"> Failed to get the bank type out of the given collection! Collection Info:"`, `"Failed to get a bank policy for"`, `"BankRepositorySystem::DetermineBankOwner"`, `"usage: /bankrepository [cmd]"`, `"Dump the contents of the desired bank repository.\n  dump <Character \| Account "`, `"Remove all contents of the desired bank repository.\n  flush <Character \| Accou"`, `"Fill the desired bank repository with the desired item.\n  fill <Character \| Ac"`, `"Fill the desired bank repository with pre-determine args. Defaults to character "` *(+6 more)*
- **Referenced Native Functions (`0x15`, 30 total)**: `0x0E9C0003` (`0x140a78a50`), `0x00DD0001` (`0x1403b6150`), `0x00A60040` (`0x140385860`), `0x00E10005` (`0x1403ca7d0`), `0x00DD0008` (`0x1403b5b70`), `0x0E9C0005` (`0x14038acf0`), `0x0E9C0006` (`0x140a6db80`), `0x00B60029` (`0x1403c5260`), `0x00B6002A` (`0x1403c26d0`), `0x00E10016` (`0x1403ba9d0`), `0x00B60020` (`0x1403bb9d0`), `0x00B60028` (`0x140381e40`), `0x0A0A0007` (`0x140b41780`), `0x0A0A0008` (`0x140b31870`) *(+16 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 14 total)**: `0x8A020003` @ `0x140b93f97` (fn `0x140b93eb7`), `0x8A020006` @ `0x140b98116` (fn `0x140b98007`), `0x8A020015` @ `0x140b97ac6` (fn `0x140b97a50`), `0x8A020017` @ `0x140b97a18` (fn `0x140b978c0`), `0x8A020019` @ `0x140b96318` (fn `0x140b96257`), `0x8A020020` @ `0x140b9326d` (fn `0x140b931c0`), `0x8A020025` @ `0x140b92135` (fn `0x140b92100`), `0x8A020029` @ `0x140b93e46` (fn `0x140b93d70`), `0x8A02002D` @ `0x140b96fb6` (fn `0x140b96ee0`), `0x8A02000B` @ `0x140b63a23` (fn `0x140b63980`), `0x8A020013` @ `0x140b65081` (fn `0x140b65010`), `0x8A020028` @ `0x140b71e61` (fn `0x140b71df0`) *(+2 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8A04000C` | `2` | Pack (C->S) | `0x8A04000B` | `(i32/u32, i32/u32, ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A020003` (`+388780`) [C++: `0x140b93f97`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A040014` | `4` | Pack (C->S) | `0x8A040013` | `(i32/u32, i32/u32, i64/u64, ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A020006` (`+389108`) [C++: `0x140b98116`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A04001C` | `6` | Pack (C->S) | `0x8A04001B` | `(ref)` | `0x00D9000C` (`self`) | `0x8A020015` (`+390276`) [C++: `0x140b97ac6`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000510` (`UI_Examination_Tooltip_DisplayDefault`), `0x10000512` (`UI_Examination_Tooltip_Override_Layout`) |
| `0x8A04001D` | `7` | Unpack (S->C) | `0x8A04001D` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x8A020016` (`+388548`) | NAT: `0x0A0A0003` (`0x140b317d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A040020` | `9` | Pack (C->S) | `0x8A04001F` | `(i32/u32, i32/u32, ref, ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A020017` (`+388636`) [C++: `0x140b97a18`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A040022` | `11` | Pack (C->S) | `0x8A040021` | `(i32/u32, ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A020019` (`+388972`) [C++: `0x140b96318`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A040023` | `12` | Unpack (S->C) | `0x8A040023` | `(i64/u64, i32/u32, i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x8A02001B` (`+388444`) | NAT: `0x0A0A0005` (`0x140b31c10`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A040026` | `13` | Unpack (S->C) | `0x8A040026` | `(ref, i32/u32, i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x8A02001A` (`+388496`) | NAT: `0x0A0A0004` (`0x140b31b50`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A040028` | `14` | Unpack (S->C) | `0x8A040028` | `(ref, i32/u32, i32/u32, ref, ref)` | `0x00D60007(0x06C3)` | `0x8A02001F` (`+388360`) | NAT: `0x0A0A0008` (`0x140b31870`), `0x0E9C0005` (`0x14038acf0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A04002A` | `16` | Pack (C->S) | `0x8A040029` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x8A020020` (`+389516`) [C++: `0x140b9326d`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A040032` | `18` | Pack (C->S) | `0x8A04002D` | `()` | `0x00D9000C` (`self`) | `0x8A020025` (`+390516`) [C++: `0x140b92135`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A04003C` | `20` | Pack (C->S) | `0x8A04003B` | `(ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A020029` (`+389252`) [C++: `0x140b93e46`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A040040` | `22` | Pack (C->S) | `0x8A04003F` | `(ref, i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A02002D` (`+389384`) [C++: `0x140b96fb6`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0A2C` (`2604`) — WorldJoinSystem (Instance Finder / World Join Queue & Teleport) <a id="rpc-class-0x0a2c"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0A2B`
- **Method Counts**: `8` Pack (C->S), `6` Unpack (S->C) (`14` total)
- **Referenced Properties (`pids.txt`, 37 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10000007` (`Skill_Pips_Mod_Trait_EnduringPrecision`), `0x100016D6` (`Usage_WorldEvent_AllConditionList`), `0x10000089` (`Effect_VitalOverTime_AllowPartialIntervalOnSuppression`), `0x10000112` (`ScriptLinks::Initiator`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x0000011C` (`UICore_Dialog_modal`), `0x10000113` (`ScriptSystem_SourceIID`), `0x10000114` (`ForwardSource_Movement_SpeedMultiplier`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x00000693` (`UICore_Dialog_confirmation_nobuttontext`), `0x0000055F` (`UICore_Dialog_message_statementtext`), `0x100001A1` (`TravelRoute_Destination`), `0x10000268` (`AIColorEventTable_Event_SayType`), `0x10003331` (`Skirmish_RestrictionDescription`), `0x10002ED1` (`Antispam_WorldJoinRequest`) *(+19 more)*
- **Referenced WLib Strings (`0x0F`, 21 total)**: `"usage: /worldjoin [cmd]"`, `"status: Display the status of the world join system."`, `"dump requests: Dump the world join requests."`, `"dump encounters: Dump the available world join encounters."`, `"dump invitations: Dump the open world join invitations."`, `"addrequest <player iid> <pe name> <min level> <max level> <difficulty tier> <gro"`, `"servicerequest <player iid>: Services the player's world join request(out of ord"`, `"removerequest <player iid>: Remove the player's world join request."`, `"removeallrequests: Remove all world join requests."`, `"setmaxencounters <max>: Sets the maximum number of concurrent WJ encounters for "` *(+11 more)*
- **Referenced Native Functions (`0x15`, 37 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00080009` (`0x140820490`), `0x00B60029` (`0x1403c5260`), `0x00E70001` (`0x140821170`), `0x0A2D0002` (`0x140b347d0`), `0x0A2D0005` (`0x140b358f0`), `0x0A2D0001` (`0x140b45dd0`), `0x0A2D0003` (`0x140b366e0`), `0x00C7000C` (`0x1403afdf0`), `0x0A44000E` (`0x140a8eeb0`), `0x00330001` (`0x140384e80`), `0x00A60015` (`0x1403a9640`) *(+23 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 16 total)**: `0x8A2B0002` @ `0x140b93030` (fn `0x140b93000`), `0x8A2B0008` @ `0x140b95d20` (fn `0x140b95cf0`), `0x8A2B000A` @ `0x140b60cc1` (fn `0x140b60c50`), `0x8A2B0013` @ `0x140b657d1` (fn `0x140b65760`), `0x8A2B000D` @ `0x140b63801` (fn `0x140b63790`), `0x8A2B0014` @ `0x140b63ce1` (fn `0x140b63c70`), `0x8A2B0005` @ `0x140b63fa1` (fn `0x140b63f30`), `0x8A2B000E` @ `0x140b64ab0` (fn `0x140b64a80`), `0x8A2B002B` @ `0x140b65dab` (fn `0x140b65d60`), `0x8A2B0023` @ `0x140b6a640` (fn `0x140b6a610`), `0x8A2B0015` @ `0x140b75589` (fn `0x140b75510`), `0x8A2B0027` @ `0x140b76f3b` (fn `0x140b76ef0`) *(+4 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8A2C0002` | `2` | Pack (C->S) | `0x8A2C0001` | `()` | `0x00D9000C` (`self`) | `0x8A2B0002` (`+1844584`) [C++: `0x140b93030`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C0003` | `3` | Unpack (S->C) | `0x8A2C0003` | `(ref)` | `0x00D60007(0x06C3)` | `0x8A2B0003` (`+1845784`) | NAT: `0x0A2D0001` (`0x140b45dd0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A2C0006` | `5` | Pack (C->S) | `0x8A2C0005` | `(ref)` | `0x00D9000C` (`self`) | `0x8A2B0018` (`+1844836`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C0008` | `7` | Pack (C->S) | `0x8A2C0007` | `()` | `0x00D9000C` (`self`) | `0x8A2B0008` (`+1845084`) [C++: `0x140b95d20`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C000A` | `9` | Pack (C->S) | `0x8A2C0009` | `(ref)` | `0x00D9000C` (`self`) | `0x8A2B000A` (`+1845336`) [C++: `0x140b60cc1`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C000B` | `10` | Unpack (S->C) | `0x8A2C000B` | `(ref)` | `0x00D60007(0x06C3)` | `0x8A2B000B` (`+1845808`) | NAT: `0x0A2D0002` (`0x140b347d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A2C0030` | `12` | Pack (C->S) | `0x8A2C002F` | `(ref)` | `0x00D9000C` (`self`) | `0x8A2B000F` (`+1844960`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C0031` | `13` | Unpack (S->C) | `0x8A2C0031` | `(ref)` | `0x00D60007(0x06C3)` | `0x8A2B0011` (`+1845948`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x0A44000E` (`0x140a8eeb0`), `0x00330001` (`0x140384e80`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10000089` (`Effect_VitalOverTime_AllowPartialIntervalOnSuppression`), `0x10000112` (`ScriptLinks::Initiator`) |
| `0x8A2C0048` | `15` | Pack (C->S) | `0x8A2C0045` | `(ref)` | `0x00D9000C` (`self`) | `0x8A2B0013` (`+1845460`) [C++: `0x140b657d1`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C0049` | `16` | Unpack (S->C) | `0x8A2C0049` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8A2B0016` (`+1845836`) | NAT: `0x0A2D0003` (`0x140b366e0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A2C0052` | `18` | Pack (C->S) | `0x8A2C0051` | `()` | `0x00D9000C` (`self`) | `0x8A2B001A` (`+1846540`) | — |
| `0x8A2C0053` | `19` | Unpack (S->C) | `0x8A2C0053` | `(ref)` | `0x00D60007(0x06C3)` | `0x8A2B0020` (`+1847048`) | NAT: `0x00070001` (`0x140822020`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8A2C005E` | `21` | Pack (C->S) | `0x8A2C0059` | `()` | `0x00D9000C` (`self`) | `0x8A2B0021` (`+1844768`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A2C0062` | `22` | Unpack (S->C) | `0x8A2C0062` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8A2B0025` (`+1845760`) | NAT: `0x0A2D0005` (`0x140b358f0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0A45` (`2629`) — WardrobeSystem (Cosmetic Wardrobe Storage, Dye & Slot Management) <a id="rpc-class-0x0a45"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0A43`, `0x0A6A`, `0x0A73`
- **Method Counts**: `6` Pack (C->S), `1` Unpack (S->C) (`7` total)
- **Referenced Properties (`pids.txt`, 34 total)**: `0x00000045` (`Script_ScriptLinkEnd_Persistent`), `0x000006C3` (`Appearance_ClassList`), `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10002F13` (`Wardrobe_ItemDID`), `0x10003AAD` (`Wardrobe_BankerIID`), `0x100030D5` (`Wardrobe_DyeIID`), `0x00000043` (`DecalScript_Size`), `0x00000313` (`AnimationScript_Speed`), `0x10000ACD` (`Item_ClothingColor`), `0x10000085` (`ErrorCategory_Script`), `0x10000105` (`CombatState_Control_StateDataList`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x10000106` (`MonsterLevel_ModToUseArray`), `0x0000055F` (`UICore_Dialog_message_statementtext`), `0x100030AA` (`Antispam_WardrobeRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) *(+16 more)*
- **Referenced WLib Strings (`0x0F`, 6 total)**: `"usage: /wardrobe [cmd]"`, `"/wardrobe ui -- Opens the wardrobe UI"`, `"/wardrobe slots -- Displays info about slots available"`, `"/wardrobe add -- Adds the selected item to the wardrobe"`, `"/wardrobe remove -- Removes the selected item from the wardrobe"`, `"/wardrobe dump <player_iid> -- Outputs the contents of the wardrobe to chat"`
- **Referenced Native Functions (`0x15`, 61 total)**: `0x00A90001` (`0x140384d60`), `0x00A90002` (`0x14037fd40`), `0x00A90048` (`0x140381cd0`), `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x00A9001E` (`0x140391780`), `0x00A9001F` (`0x140393e20`), `0x00A90020` (`0x140394390`), `0x00A90021` (`0x1403937e0`), `0x00A90022` (`0x140399fa0`), `0x00A90024` (`0x140397490`), `0x00A90025` (`0x140397860`), `0x00A90026` (`0x140393250`) *(+47 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 14 total)**: `0x8A430004` @ `0x140b673a5` (fn `0x140b67370`), `0x8A430009` @ `0x140b61658` (fn `0x140b615c0`), `0x8A43000B` @ `0x140b61456` (fn `0x140b61346`), `0x8A430018` @ `0x140b96761` (fn `0x140b96700`), `0x8A43001C` @ `0x140b618fc` (fn `0x140b61840`), `0x8A430017` @ `0x140b63a90` (fn `0x140b63a60`), `0x8A430006` @ `0x140b6b3dd` (fn `0x140b6b322`), `0x8A43000F` @ `0x140b6b51e` (fn `0x140b6b460`), `0x8A430014` @ `0x140b720f0` (fn `0x140b720c0`), `0x8A430010` @ `0x140b79b07` (fn `0x140b79a80`), `0x8A43000D` @ `0x140b79b75` (fn `0x140b79b40`), `0x8A430007` @ `0x140b926e7` (fn `0x140b92660`) *(+2 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8A450001` | `1` | Unpack (S->C) | `0x8A450001` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8A430002` (`+1814420`) | NAT: `0x0A460001` (`0x140b3bd70`); PID: `0x000006C3` (`Appearance_ClassList`), `0x00000043` (`DecalScript_Size`) |
| `0x8A450004` | `3` | Pack (C->S) | `0x8A450003` | `()` | `0x00D9000C` (`self`) | `0x8A430004` (`+1814932`) [C++: `0x140b673a5`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A450008` | `5` | Pack (C->S) | `0x8A450007` | `(i64/u64, i32/u32)` | `0x00D9000C` (`self`) | `0x8A6A0001` (`+1678324`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00A60016` (`0x140392220`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10002F13` (`Wardrobe_ItemDID`), `0x10003AAD` (`Wardrobe_BankerIID`) |
| `0x8A45000A` | `7` | Pack (C->S) | `0x8A450009` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x8A430009` (`+1814460`) [C++: `0x140b61658`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8A45000C` | `9` | Pack (C->S) | `0x8A45000B` | `(i64/u64, i64/u64, ref)` | `0x00D9000C` (`self`) | `0x8A43000B` (`+1814588`) [C++: `0x140b61456`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x00000313` (`AnimationScript_Speed`), `0x10000ACD` (`Item_ClothingColor`) |
| `0x8A45000E` | `11` | Pack (C->S) | `0x8A45000D` | `(i64/u64, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8A730001` (`+1678552`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00A60016` (`0x140392220`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10002F13` (`Wardrobe_ItemDID`), `0x100030D5` (`Wardrobe_DyeIID`) |
| `0x8A450010` | `13` | Pack (C->S) | `0x8A45000F` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8A430018` (`+1816352`) [C++: `0x140b96761`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0A84` (`2692`) — PVPLeaderboardSystem (/pvplb Leaderboard & PvP Virtue Selection) <a id="rpc-class-0x0a84"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0DD9`, `0x1062`
- **Method Counts**: `1` Pack (C->S), `1` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 4 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10004DCB` (`PVP_PSSS_SelectedPVPVirtueArray`), `0x10004DCA` (`PVP_VirtueDID`), `0x10004D91` (`PVP_Hero_AvailableVirtuePoints`)
- **Referenced WLib Strings (`0x0F`, 13 total)**: `">"`, `":"`, `"LogUtils::LogAndAssert"`, `"usage: /pvplb [ list \| show \| reset \| remove \| addme ]"`, `"usage: /pvplb list [ charname ]: List the contents of the leaderboard, sort by s"`, `"usage: /pvplb show: Open the player-visible UI"`, `"usage: /pvplb reset: Reset the entire leaderboard"`, `"usage: /pvplb remove <charname>: Remove the character from the leaderboard."`, `"Failed to send virtue increment request:"`, `"CPVPVirtueSystem::UI_IncrementVirtue"` *(+3 more)*
- **Referenced Native Functions (`0x15`, 15 total)**: `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0001` (`0x1403b6150`), `0x00DD0003` (`0x1403b6270`), `0x00DD0008` (`0x1403b5b70`), `0x10610001` (`0x140b3a030`), `0x00E70001` (`0x140821170`), `0x00270006` (`0x1403ae7b0`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00270025` (`0x1403a9500`), `0x00270035` (`0x1403a2b80`), `0x001A0001` (`0x1403c4ed0`), `0x00270001` (`0x14038a7e0`) *(+1 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x8DD90001` @ `0x140b7ac10` (fn `0x140b7abc0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8A8400F4` | `2` | Pack (C->S) | `0x8A8400F3` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8DD90001` (`+1276648`) [C++: `0x140b7ac10`] | NAT: `0x00E70001` (`0x140821170`), `0x00DD0001` (`0x1403b6150`); MSG: `"Failed to send virtue increment request:"`, `"CPVPVirtueSystem::UI_IncrementVirtue"` |
| `0x8A8400FB` | `3` | Unpack (S->C) | `0x8A8400FB` | `(ref)` | `0x00D60007(0x06C3)` | `0x9062001E` (`+1273532`) | NAT: `0x10610001` (`0x140b3a030`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0AE6` (`2790`) — Tea / Telemetry / Effect Array Notification System (0x0AE7) <a id="rpc-class-0x0ae6"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2` | **Paired Client Implementation Class(es)**: `0x0AE7`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 4 total)**: `0x000006C2` (`MapNote_StringInfo`), `0x000008C1` (`Tea_Int_Param`), `0x000008BE` (`Tea_Array_Param`), `0x1000000C` (`Skill_VitalCost_Book_SigilAnimalMod`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 11 total)**: `0x00E70003` (`0x140821030`), `0x00A90009` (`0x140393d70`), `0x00A90008` (`0x140391b20`), `0x00A9000A` (`0x14039a920`), `0x00270006` (`0x1403ae7b0`), `0x00270016` (`0x1403ab710`), `0x00A60007` (`0x140396240`), `0x00B0000B` (`0x140385130`), `0x00270035` (`0x1403a2b80`), `0x00B00003` (`0x1403beb80`), `0x09B60001` (`0x140821bd0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8AE60003` | `2` | Unpack (S->C) | `0x8AE60003` | `(ref, ref, ref, i64/u64)` | `0x00D60007(0x06C2)` | `0x8AE70005` (`+1851508`) | NAT: `0x00E70003` (`0x140821030`), `0x00A90009` (`0x140393d70`), `0x00A90008` (`0x140391b20`); PID: `0x000006C2` (`MapNote_StringInfo`) |

### Class `0x0C0D` (`3085`) — PaperItemSystem (Wallet Currencies / Account & Character Paper Items) <a id="rpc-class-0x0c0d"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0C0C`, `0x0E7D`
- **Method Counts**: `1` Pack (C->S), `4` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 25 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10000096` (`Effect_NotifyTargetOnRemovalMessage`), `0x100055FF` (`Antispam_TaskPurchase`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x000001E8` (`PreprocOptions_TexGen_Directory`), `0x000000A1` (`Test::TestSphereOffset_ParameterProvided`), `0x000000B8` (`Physics_NoShadowSolids`), `0x000000C0` (`UICore_Element_ObjectOpacity`), `0x10003B0F` (`PaperItem_Category`), `0x1000013A` (`Vendor_SellAll_ItemToSell`), `0x0000013B` (`SequenceScript_LoopForever`), `0x00000155` (`PreprocOptions_CrossModule_VisualPreproc`), `0x1000060C` (`Mood_Control_Mood_Effects`), `0x0000008D` (`Script_IntensityMultiplierVariance`), `0x0000009B` (`UIB_MediaDesc_image_file`), `0x000000B4` (`DecalVolume_ColorAndOpacity`), `0x00000072` (`UICore_Button_broadcast_when_ghosted`), `0x00000082` (`AfterEffectScript_Priority`) *(+7 more)*
- **Referenced WLib Strings (`0x0F`, 26 total)**: `"usage: /paperitem <option> \n        add <item.wc> q <number> <IID>\n        rem"`, `"]!"`, `"Unable to get entity for Item ["`, `"PaperItemSystem::ShouldTreatLikePaperItem"`, `"> Unable to load PaperItemControl!"`, `"PaperItemSystem::ShouldTreatLikePaperItemEntity"`, `"has a paper item category but is not in the PaperItemControl!"`, `"> Item"`, `"Could not determine cost for Paper Item Service:"`, `"PaperItemSystem::InqBasePaperItemServiceCost"` *(+16 more)*
- **Referenced Native Functions (`0x15`, 22 total)**: `0x000B000A` (`0x1403bec10`), `0x000C0006` (`0x140381830`), `0x000C0002` (`0x1403b9f80`), `0x00110002` (`0x1403c68d0`), `0x000C0001` (`0x1403c4e10`), `0x000B0012` (`0x1403c5400`), `0x000C0005` (`0x1403c50c0`), `0x000C0003` (`0x1403b9f00`), `0x0C150001` (`0x140b3c080`), `0x0C150003` (`0x140b3bfa0`), `0x00C7000C` (`0x1403afdf0`), `0x00C70005` (`0x14037c560`), `0x00C70009` (`0x14037c4c0`), `0x0C150002` (`0x140b3c0f0`) *(+8 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 17 total)**: `0x8C0C0015` @ `0x140b91717` (fn `0x140b91680`), `0x8E7D0014` @ `0x140b98ce6` (fn `0x140b98c70`), `0x8C0C000C` @ `0x140b64cb7` (fn `0x140b64c20`), `0x8C0C0007` @ `0x140b6dd60` (fn `0x140b6dd30`), `0x8C0C0009` @ `0x140b72bf4` (fn `0x140b72b60`), `0x8C0C0001` @ `0x140b83a65` (fn `0x140b8394f`), `0x8C0C0010` @ `0x140b83bc7` (fn `0x140b83b10`), `0x8C0C000A` @ `0x140b83d17` (fn `0x140b83c20`), `0x8C0C000F` @ `0x140b83e27` (fn `0x140b83d70`), `0x8C0C000D` @ `0x140b83f37` (fn `0x140b83e80`), `0x8C0C0014` @ `0x140b8405d` (fn `0x140b83f90`), `0x8C0C0008` @ `0x140b8eb6b` (fn `0x140b8eb20`) *(+5 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8C0D0001` | `1` | Unpack (S->C) | `0x8C0D0001` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8C0C0002` (`+1199008`) | NAT: `0x0C150001` (`0x140b3c080`), `0x0C150003` (`0x140b3bfa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8C0D0004` | `2` | Unpack (S->C) | `0x8C0D0004` | `(i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8C0C0003` (`+1198704`) | NAT: `0x0C150001` (`0x140b3c080`), `0x0C150003` (`0x140b3bfa0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8C0D0006` | `3` | Unpack (S->C) | `0x8C0D0006` | `(ref)` | `0x00D60007(0x06C3)` | `0x8C0C0004` (`+1198496`) | NAT: `0x000B0012` (`0x1403c5400`), `0x000B000A` (`0x1403bec10`), `0x000C0005` (`0x1403c50c0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8C0D0018` | `5` | Pack (C->S) | `0x8C0D0017` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x8C0C0015` (`+1199476`) [C++: `0x140b91717`], `0x8E7D0014` (`+1447936`) [C++: `0x140b98ce6`] | NAT: `0x00E70001` (`0x140821170`), `0x0E790009` (`0x140b38380`) |
| `0x8C0D001C` | `6` | Unpack (S->C) | `0x8C0D001C` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8C0C0016` (`+1199120`) | NAT: `0x0C150006` (`0x140b3c010`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0C17` (`3095`) — Client Event 0xE1 Two-Argument Dispatch Stub (0x0C16) <a id="rpc-class-0x0c17"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0C16`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`)**: Only `0x008E`/`0x00D9`/`0x00D6` RPC primitives
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x8C160001` @ `0x140b5f498` (fn `0x140b5f3d8`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8C170002` | `2` | Pack (C->S) | `0x8C170001` | `(ref, ref)` | `0x00D9000C` (`self`) | `0x8C160001` (`+990224`) [C++: `0x140b5f498`] | — |

### Class `0x0C20` (`3104`) — Bank Chest Rename System (chestinfo rename) <a id="rpc-class-0x0c20"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0C23`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`, 2 total)**: `"chestinfo dump <Character \| Account \| SharedHousing \| StandardHousing> [<play"`, `"chestinfo rename <Character \| Account \| SharedHousing \| StandardHousing> <pla"`
- **Referenced Native Functions (`0x15`, 3 total)**: `0x00DD0008` (`0x1403b5b70`), `0x00E70001` (`0x140821170`), `0x0D6C0001` (`0x140b31710`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x8C230002` @ `0x140b996b2` (fn `0x140b995e0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8C200006` | `2` | Pack (C->S) | `0x8C200005` | `(ref, i32/u32, ref)` | `0x00D9000C` (`self`) | `0x8C230002` (`+387868`) [C++: `0x140b996b2`] | NAT: `0x00E70001` (`0x140821170`), `0x0D6C0001` (`0x140b31710`) |

### Class `0x0C41` (`3137`) — CFellowshipMakerSystem (Instance Finder Group Matchmaking Queue) <a id="rpc-class-0x0c41"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0C3D`
- **Method Counts**: `6` Pack (C->S), `4` Unpack (S->C) (`10` total)
- **Referenced Properties (`pids.txt`, 21 total)**: `0x10000090` (`Effect_InstantVital_Multiplicative`), `0x000000D1` (`PSDesc_SortingPriority`), `0x100000F0` (`Time_LastLogoutTimeStamp`), `0x00000245` (`Slider_FlySpeed`), `0x10003C5B` (`Antispam_FellowshipMakerRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10003DCA` (`FellowshipMaker_PlayerState`), `0x10004166` (`FellowshipMaker_PlayerRole`), `0x1000006C` (`UI_QuestTree_RaidImageElement`), `0x0000017B` (`PEDesc_Exploding`), `0x100041BE` (`FellowshipMaker_PlayerConfiguration`), `0x0000025A` (`PSDesc_StopFadeDist`), `0x00000040` (`CameraShakeScript_OffsetY`), `0x00000080` (`BeamScript_UseTargetPosition`), `0x100042D7` (`FellowshipMaker_DisallowUse`), `0x10000209` (`GeneratorProfile_RepeatMaxTimes`), `0x1000016B` (`TutorialHint_Name`), `0x10000562` (`Quest_ObjectiveVolumeString`) *(+3 more)*
- **Referenced WLib Strings (`0x0F`, 32 total)**: `", who is not our fellow."`, `"Received a FellowConfirmEnterQueue message for player"`, `"CFellowshipMakerSystem::RecvCEvt_FellowConfirmEnterQueue"`, `"null fellow in fellowship:"`, `"CFellowshipMakerSystem::UI_ArePlayerAndFellowsAtCap"`, `"usage: /fellowshipmaker [cmd]"`, `"status: Report on the current status of the system - high level summary."`, `"runtimestats: Report on the runtime stats on the master server. Various internal"`, `"encounterstats: Report on the encounter stats since they last were dumped to GRS"`, `"dump queue: Dump the fellowship maker system queue."` *(+22 more)*
- **Referenced Native Functions (`0x15`, 43 total)**: `0x00B60029` (`0x1403c5260`), `0x00E70001` (`0x140821170`), `0x00DD0001` (`0x1403b6150`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x0CD6001D` (`0x140b35660`), `0x00610007` (`0x140388ca0`), `0x0011000C` (`0x140385130`), `0x00C70005` (`0x14037c560`), `0x0CD6001F` (`0x140b3cd10`), `0x0CD60020` (`0x140b3cc60`), `0x000B0004` (`0x1403bc340`), `0x00270006` (`0x1403ae7b0`), `0x00A9000E` (`0x14039aad0`) *(+29 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 16 total)**: `0x8C3D0003` @ `0x140b961d0` (fn `0x140b961a0`), `0x8C3D0004` @ `0x140b936db` (fn `0x140b93690`), `0x8C3D001B` @ `0x140b92fb0` (fn `0x140b92f80`), `0x8C3D001C` @ `0x140b95f51` (fn `0x140b95ee0`), `0x8C3D001D` @ `0x140b95e93` (fn `0x140b95de0`), `0x8C3D0010` @ `0x140b63ea0` (fn `0x140b63df0`), `0x8C3D000D` @ `0x140b64699` (fn `0x140b64590`), `0x8C3D0015` @ `0x140b648b0` (fn `0x140b64880`), `0x8C3D0008` @ `0x140b7c358` (fn `0x140b7c2c0`), `0x8C3D000B` @ `0x140b7cca8` (fn `0x140b7cc10`), `0x8C3D000F` @ `0x140b85268` (fn `0x140b851d0`), `0x8C3D0009` @ `0x140b85a98` (fn `0x140b85a00`) *(+4 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8C41000E` | `2` | Pack (C->S) | `0x8C41000D` | `()` | `0x00D9000C` (`self`) | `0x8C3D0003` (`+729180`) [C++: `0x140b961d0`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000090` (`Effect_InstantVital_Multiplicative`) |
| `0x8C410010` | `4` | Pack (C->S) | `0x8C41000F` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8C3D0004` (`+729340`) [C++: `0x140b936db`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000090` (`Effect_InstantVital_Multiplicative`) |
| `0x8C41001A` | `6` | Pack (C->S) | `0x8C410019` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8C3D000A` (`+729476`) | NAT: `0x00E70001` (`0x140821170`) |
| `0x8C410028` | `7` | Unpack (S->C) | `0x8C410028` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8C3D000C` (`+729748`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x000000D1` (`PSDesc_SortingPriority`), `0x10000090` (`Effect_InstantVital_Multiplicative`), `0x000006C3` (`Appearance_ClassList`); MSG: `", who is not our fellow."`, `"Received a FellowConfirmEnterQueue message fo"` |
| `0x8C410034` | `8` | Unpack (S->C) | `0x8C410034` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x8C3D0011` (`+731204`) | NAT: `0x00610007` (`0x140388ca0`), `0x0011000C` (`0x140385130`), `0x00C7000C` (`0x1403afdf0`); PID: `0x10000090` (`Effect_InstantVital_Multiplicative`), `0x000006C3` (`Appearance_ClassList`) |
| `0x8C410037` | `9` | Unpack (S->C) | `0x8C410037` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8C3D0012` (`+731360`) | NAT: `0x0CD60020` (`0x140b3cc60`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8C410049` | `10` | Unpack (S->C) | `0x8C410049` | `(ref)` | `0x00D60007(0x06C3)` | `0x8C3D0018` (`+731748`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x0061000A` (`0x1403bf600`), `0x00620006` (`0x1403c50c0`); PID: `0x10000090` (`Effect_InstantVital_Multiplicative`), `0x100000F0` (`Time_LastLogoutTimeStamp`), `0x000006C3` (`Appearance_ClassList`) |
| `0x8C410052` | `12` | Pack (C->S) | `0x8C41004F` | `()` | `0x00D9000C` (`self`) | `0x8C3D001B` (`+732532`) [C++: `0x140b92fb0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8C410057` | `15` | Pack (C->S) | `0x8C410056` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8C3D001C` (`+728952`) [C++: `0x140b95f51`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000090` (`Effect_InstantVital_Multiplicative`) |
| `0x8C410058` | `16` | Pack (C->S) | `0x8C410055` | `(ref, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8C3D001D` (`+729064`) [C++: `0x140b95e93`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000090` (`Effect_InstantVital_Multiplicative`) |

### Class `0x0C4F` (`3151`) — RelicSystem (Legendary Item Relic Deconstruct & Transmute) <a id="rpc-class-0x0c4f"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0C4E`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 9 total)**: `0x1000195C` (`Admin_PlayerTestMode`), `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`), `0x100001A9` (`QuestEffect_ApplyRole`), `0x10000549` (`Skill_AttackHook_PositionalDamageMultiplier_Mod_Array`), `0x10001E42` (`Runic_Tier`), `0x10002ED1` (`Antispam_WorldJoinRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10001729` (`World_DisabledCraftRecipe`), `0x1000172B` (`World_DisabledCraftRecipeArray`)
- **Referenced WLib Strings (`0x0F`, 6 total)**: `"usage: /relic <deconstruct\|transmute>"`, `"add <Relic DID> [Quantity] ~ adds the given quantity of the indicated relic to y"`, `"deconstruct <Relic DID> ~ breaks down the indicated relic."`, `"transmute <Recipe DID> [Optional Relic1] [Optional Relic2] [...] ~ executes the "`, `"shards <Quantity> ~ creates the given quantity of relic shards."`, `"disabledrecipelist \| drl <add\|remove\|clear\|list> [.wc file name] ~ manage gl"`
- **Referenced Native Functions (`0x15`, 14 total)**: `0x01AA0002` (`0x140a7dae0`), `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00E70001` (`0x140821170`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00230004` (`0x1403bc7d0`), `0x00230001` (`0x1403b6580`), `0x00190006` (`0x1403c2c90`), `0x001A0001` (`0x1403c4ed0`), `0x00E10006` (`0x1403b8050`), `0x0030000B` (`0x140385ab0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x8C4E0001` @ `0x140b675ce` (fn `0x140b67520`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8C4F0002` | `2` | Pack (C->S) | `0x8C4F0001` | `(i64/u64, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8C4E0001` (`+1469188`) [C++: `0x140b675ce`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x100002AF` (`RandomBox@Entity.m_bCanFireOutput_9`) |
| `0x8C4F0004` | `4` | Pack (C->S) | `0x8C4F0003` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x8C4E0002` (`+1469316`) | NAT: `0x00E70001` (`0x140821170`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`); PID: `0x100001A9` (`QuestEffect_ApplyRole`), `0x10000549` (`Skill_AttackHook_PositionalDamageMultiplier_Mod_Array`) |

### Class `0x0CFE` (`3326`) — FellowshipVoteSystem (Fellowship Dismiss & Loot Rule Voting) <a id="rpc-class-0x0cfe"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0D00`
- **Method Counts**: `2` Pack (C->S), `2` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 22 total)**: `0x10004784` (`FellowshipVote_VoteToDismissPlayer`), `0x10004785` (`FellowshipVote_VoteToChangeLootType`), `0x1000478E` (`FellowshipVote_VoteToChangeLootQuality`), `0x1000009F` (`Signals@NPC::HintChange::Source`), `0x10000145` (`ExecuteScript_GetContainingEntityIID_ContainerIID`), `0x00000517` (`UICore_Dialog_type`), `0x00000472` (`UICore_Dialog_confirmation_questiontext`), `0x00000249` (`UICore_Dialog_confirmation_yesbuttontext`), `0x00000693` (`UICore_Dialog_confirmation_nobuttontext`), `0x000001B5` (`UICore_Dialog_WSLCallback`), `0x0000071E` (`UICore_Dialog_StartInactive`), `0x000003C8` (`UICore_Dialog_confirmation_duration`), `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10004792` (`FellowshipVote_UnderwayVote`), `0x1000001F` (`Resist_Additional_Resistance_Cry`), `0x1000006A` (`UI_Pet_PetVitalDisplayElement`), `0x10000059` (`Vendor_Inventory`), `0x000000A0` (`Option_MaxIntValue`) *(+4 more)*
- **Referenced WLib Strings (`0x0F`, 2 total)**: `"Unknown vote type:"`, `"FellowshipVoteSystem::GetVoteTextFromHolder"`
- **Referenced Native Functions (`0x15`, 22 total)**: `0x00C7000C` (`0x1403afdf0`), `0x00C70019` (`0x14039dba0`), `0x00C70009` (`0x14037c4c0`), `0x00330001` (`0x140384e80`), `0x00A6001F` (`0x1403a9c20`), `0x00A6002B` (`0x1403b0040`), `0x00A60015` (`0x1403a9640`), `0x00A60023` (`0x1403a88c0`), `0x00A60019` (`0x1403aa260`), `0x00A60001` (`0x14037bb20`), `0x00070001` (`0x140822020`), `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00A6000A` (`0x140395ca0`) *(+8 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8CFE0002` | `2` | Pack (C->S) | `0x8CFE0001` | `(ref)` | `0x00D9000C` (`self`) | `0x8D000009` (`+736348`) | — |
| `0x8CFE0008` | `3` | Unpack (S->C) | `0x8CFE0008` | `(ref)` | `0x00D60007(0x06C3)` | `0x8D000002` (`+736396`) | NAT: `0x00C7000C` (`0x1403afdf0`), `0x00C70019` (`0x14039dba0`), `0x00C70009` (`0x14037c4c0`); PID: `0x1000009F` (`Signals@NPC::HintChange::Source`), `0x10000145` (`ExecuteScript_GetContainingEntityIID_ContainerIID`), `0x00000517` (`UICore_Dialog_type`) |
| `0x8CFE000A` | `5` | Pack (C->S) | `0x8CFE0009` | `(i32/u32, ref)` | `0x00D9000C` (`self`) | `0x8D000003` (`+736876`) | NAT: `0x00E70001` (`0x140821170`), `0x00A60024` (`0x140391670`), `0x00A6000A` (`0x140395ca0`); PID: `0x000005A5` (`UICore_Dialog_confirmation_response`), `0x10004792` (`FellowshipVote_UnderwayVote`) |
| `0x8CFE0010` | `6` | Unpack (S->C) | `0x8CFE0010` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x8D000005` (`+737012`) | NAT: `0x00C70019` (`0x14039dba0`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`); PID: `0x1000009F` (`Signals@NPC::HintChange::Source`), `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0DA7` (`3495`) — CMountedCombatSystem (War-Steed Trait Trees & Cosmetic Appearances) <a id="rpc-class-0x0da7"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0D8E`
- **Method Counts**: `6` Pack (C->S), `1` Unpack (S->C) (`7` total)
- **Referenced Properties (`pids.txt`, 37 total)**: `0x000006C3` (`Appearance_ClassList`), `0x100001E8` (`ExaminationElement_Genero_Struct`), `0x10004C45` (`Mount_ClientUI_Entity`), `0x10004CB3` (`Mount_Appearances_TraitsOverride`), `0x0000009C` (`UISkin_MappingStruct`), `0x10004CB4` (`Mount_Appearances_TraitDID`), `0x0000099B` (`WCGenDependencyListArray`), `0x10004E90` (`Mount_Appearances_Slot_Colourization_Override`), `0x10004E8F` (`Mount_Appearances_Slot_Colourization`), `0x10004C6F` (`Mount_MeshFX_Appearances`), `0x10004C6C` (`Mount_MeshFX_Entity`), `0x0000054F` (`Appearance_InstanceList`), `0x1000012E` (`Currency_Value`), `0x000000F4` (`ClearanceBBoxMax2`), `0x000000FC` (`PreprocOptions_PlaceGen_Places`), `0x10004A5B` (`Antispam_TraitRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x10004A98` (`MountedCombat_Landblock_Area`) *(+19 more)*
- **Referenced WLib Strings (`0x0F`, 19 total)**: `"Slotted trait in the MountedAppearances pool was not an appearance trait!"`, `"CMountedCombatSystem::ConstructCosmeticOverrideProperty"`, `"No number of points to refund cost progression found on the MountedCombatControl"`, `"MountedCombatControl::GetRefundCostForTraitPoints"`, `"addxp <#> - Adds the specified amount of SteedXP."`, `"levelup <#> - Adds SteedXP to bring you up to the specified level."`, `"getcosmetics - Learns all the cosmetic traits for warsteeds."`, `"Unable to get trait tree!"`, `"MountedCombatSystem::GetCurrentlyActiveTree"`, `"MountedCombatSystem::GetMountAppearancesProperty"` *(+9 more)*
- **Referenced Native Functions (`0x15`, 51 total)**: `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00E70001` (`0x140821170`), `0x00270006` (`0x1403ae7b0`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00270025` (`0x1403a9500`), `0x00270035` (`0x1403a2b80`), `0x001A0001` (`0x1403c4ed0`), `0x00B6002B` (`0x1403c0470`), `0x00B70008` (`0x1403c5150`), `0x00B70003` (`0x1403ba520`) *(+37 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 17 total)**: `0x8D8E0003` @ `0x140b91fe0` (fn `0x140b91f90`), `0x8D8E0014` @ `0x140b5d9a2` (fn `0x140b5d94f`), `0x8D8E001F` @ `0x140b4dff2` (fn `0x140b4df9f`), `0x8D8E0005` @ `0x140b5aee5` (fn `0x140b5ae7b`), `0x8D8E0016` @ `0x140b63048` (fn `0x140b62ff0`), `0x8D8E0017` @ `0x140b64930` (fn `0x140b64900`), `0x8D8E001E` @ `0x140b6b67c` (fn `0x140b6b580`), `0x8D8E0013` @ `0x140b6c870` (fn `0x140b6c840`), `0x8D8E000C` @ `0x140b70e92` (fn `0x140b70dd0`), `0x8D8E0002` @ `0x140b7871b` (fn `0x140b786d0`), `0x8D8E0018` @ `0x140b78fe1` (fn `0x140b78f60`), `0x8D8E0020` @ `0x140b793d2` (fn `0x140b79370`) *(+5 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8DA70008` | `2` | Pack (C->S) | `0x8DA70007` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8D8E0003` (`+1151224`) [C++: `0x140b91fe0`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x100001E8` (`ExaminationElement_Genero_Struct`) |
| `0x8DA7000A` | `4` | Pack (C->S) | `0x8DA70009` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8D8E0015` (`+1153760`) | — |
| `0x8DA7000C` | `6` | Pack (C->S) | `0x8DA7000B` | `()` | `0x00D9000C` (`self`) | `0x8D8E0014` (`+1153980`) [C++: `0x140b5d9a2`] | — |
| `0x8DA7000E` | `8` | Pack (C->S) | `0x8DA7000D` | `(ref)` | `0x00D9000C` (`self`) | `0x8D8E001B` (`+1154280`) | — |
| `0x8DA70010` | `10` | Pack (C->S) | `0x8DA7000F` | `()` | `0x00D9000C` (`self`) | `0x8D8E001F` (`+1154024`) [C++: `0x140b4dff2`] | — |
| `0x8DA70011` | `11` | Unpack (S->C) | `0x8DA70011` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8D8E0029` (`+1153940`) | NAT: `0x00DC0007` (`0x1403a5ea0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8DA70014` | `13` | Pack (C->S) | `0x8DA70013` | `(i32/u32, f32, i64/u64)` | `0x00D9000C` (`self`) | `0x8D8E002A` (`+1153808`) | NAT: `0x00DC0006` (`0x1403a5d90`), `0x00DC000B` (`0x1403a6180`) |

### Class `0x0DD1` (`3537`) — PVPPreGameSystem (PvP Match Pre-Game Lobby & Team Chat) <a id="rpc-class-0x0dd1"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0DD0`
- **Method Counts**: `4` Pack (C->S), `1` Unpack (S->C) (`5` total)
- **Referenced Properties (`pids.txt`, 8 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10004E3D` (`PVP_TeamChatRoomID`), `0x1000031A` (`UIComponent_Timer_PropertyToWatch`), `0x10007348` (`World_Chat_ProvideGaggedPlayerFeedback`), `0x1000004D` (`TextItem_Title`), `0x100000A0` (`AI_KungFu_TransitionList`), `0x10004DCC` (`PVP_PVPSessionSetupStruct`), `0x10004CB0` (`Antispam_PVPPreGameRequest`)
- **Referenced WLib Strings (`0x0F`, 2 total)**: `"FIXME: Spam handling errors not implemented yet!"`, `"PVPPreGameSystem::HandleSpam"`
- **Referenced Native Functions (`0x15`, 11 total)**: `0x00A9002E` (`0x1403ab690`), `0x00E70001` (`0x140821170`), `0x0DEF0001` (`0x140b38bc0`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x00230001` (`0x1403b6580`), `0x00010002` (`0x140822160`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00C70002` (`0x14037c380`), `0x00270006` (`0x1403ae7b0`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 7 total)**: `0x8DD00001` @ `0x140b95da5` (fn `0x140b95d70`), `0x8DD00002` @ `0x140b97630` (fn `0x140b975e0`), `0x8DD00008` @ `0x140b79f80` (fn `0x140b79f30`), `0x8DD0000C` @ `0x140b96d46` (fn `0x140b96cd0`), `0x8DD0000D` @ `0x140b61efd` (fn `0x140b61e50`), `0x8DD00005` @ `0x140b6ff43` (fn `0x140b6ff10`), `0x8DD00007` @ `0x140b759b5` (fn `0x140b758a0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8DD1003F` | `3` | Pack (C->S) | `0x8DD1003B` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8DD00002` (`+1272264`) [C++: `0x140b97630`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8DD10040` | `4` | Pack (C->S) | `0x8DD1003E` | `()` | `0x00D9000C` (`self`) | `0x8DD00001` (`+1272196`) [C++: `0x140b95da5`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8DD10047` | `5` | Unpack (S->C) | `0x8DD10047` | `(i32/u32, ref)` | `0x00D60007(0x06C3)` | `0x8DD00006` (`+1272356`) | NAT: `0x0DEF0001` (`0x140b38bc0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10004E3D` (`PVP_TeamChatRoomID`) |
| `0x8DD1004C` | `7` | Pack (C->S) | `0x8DD1004B` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8DD00008` (`+1272668`) [C++: `0x140b79f80`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8DD10050` | `9` | Pack (C->S) | `0x8DD1004F` | `(ref)` | `0x00D9000C` (`self`) | `0x8DD0000C` (`+1273272`) [C++: `0x140b96d46`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0E15` (`3605`) — TraitConfigurationSystem (Trait Specialization Loadout Slots) <a id="rpc-class-0x0e15"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0E16`
- **Method Counts**: `5` Pack (C->S), `3` Unpack (S->C) (`8` total)
- **Referenced Properties (`pids.txt`, 23 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10004CB3` (`Mount_Appearances_TraitsOverride`), `0x0000009C` (`UISkin_MappingStruct`), `0x10004CB4` (`Mount_Appearances_TraitDID`), `0x10004C45` (`Mount_ClientUI_Entity`), `0x00000289` (`Appearance_Modifier`), `0x000000A4` (`Appearance_AprFile`), `0x00000157` (`Appearance_KeyInfo`), `0x0000054F` (`Appearance_InstanceList`), `0x10004C6F` (`Mount_MeshFX_Appearances`), `0x10005028` (`Mount_Appearances_FXTraits`), `0x000001AB` (`MoveToController_DownSpeed`), `0x10000518` (`UI_FloatyText_DamageColumns`), `0x10004E8F` (`Mount_Appearances_Slot_Colourization`), `0x10000126` (`Craft_Metalsmith_StartTime`), `0x0000005D` (`Server_World_DefaultMaxCharactersForType`), `0x10004F9E` (`Antispam_TraitConfigurationRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) *(+5 more)*
- **Referenced WLib Strings (`0x0F`, 6 total)**: `"Slotted trait in the MountedAppearances pool was not an appearance trait!"`, `"CMountedCombatSystem::ConstructCosmeticOverrideProperty"`, `"MountedCombatSystem::WriteUpdatedMountAppearancesProperty"`, `"Failed to instantiate TraitConfigurationStruct for:"`, `"TraitConfigurationSystem::InqStateArray_TraitConfigurationSlotAccess"`, `"TraitConfigurationSystem::InqFirstSlotAvailableForMithril"`
- **Referenced Native Functions (`0x15`, 36 total)**: `0x00A90004` (`0x140395da0`), `0x00A90003` (`0x1403a4ca0`), `0x00A90005` (`0x1403ae5b0`), `0x0035000A` (`0x140384c30`), `0x00DD0001` (`0x1403b6150`), `0x013D0001` (`0x140b32db0`), `0x00E70001` (`0x140821170`), `0x00270006` (`0x1403ae7b0`), `0x0019002E` (`0x1403bf1b0`), `0x001A0008` (`0x1403c5150`), `0x001A0003` (`0x1403ba0b0`), `0x00270025` (`0x1403a9500`), `0x00270035` (`0x1403a2b80`), `0x001A0001` (`0x1403c4ed0`) *(+22 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 6 total)**: `0x8E160001` @ `0x140b611f1` (fn `0x140b61180`), `0x8E160006` @ `0x140b9ab15` (fn `0x140b9aa5b`), `0x8E160007` @ `0x140b92e40` (fn `0x140b92df0`), `0x8E160008` @ `0x140b99d38` (fn `0x140b99c80`), `0x8E16000A` @ `0x140b9a9d4` (fn `0x140b9a91f`), `0x8E160002` @ `0x140b8b1b2` (fn `0x140b8b0d0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8E150002` | `2` | Pack (C->S) | `0x8E150001` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8E160001` (`+1550128`) [C++: `0x140b611f1`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x10000518` (`UI_FloatyText_DamageColumns`) |
| `0x8E150006` | `4` | Pack (C->S) | `0x8E150005` | `(ref, i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8E160006` (`+1550276`) [C++: `0x140b9ab15`] | NAT: `0x00E70001` (`0x140821170`), `0x00230009` (`0x1403ba760`) |
| `0x8E15000A` | `6` | Pack (C->S) | `0x8E150009` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8E160007` (`+1550636`) [C++: `0x140b92e40`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8E15000C` | `8` | Pack (C->S) | `0x8E15000B` | `(ref, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8E160008` (`+1550548`) [C++: `0x140b99d38`] | NAT: `0x00E70001` (`0x140821170`), `0x00C70010` (`0x14038bee0`) |
| `0x8E15000E` | `9` | Unpack (S->C) | `0x8E15000E` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x8E160009` (`+1550688`) | NAT: `0x0E220001` (`0x140b441d0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8E150010` | `11` | Pack (C->S) | `0x8E15000F` | `(ref, ref, i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x8E16000A` (`+1550400`) [C++: `0x140b9a9d4`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8E150011` | `12` | Unpack (S->C) | `0x8E150011` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8E16000C` (`+1550720`) | NAT: `0x0E220002` (`0x140b44090`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8E150013` | `13` | Unpack (S->C) | `0x8E150013` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8E16000B` (`+1550748`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00270030` (`0x1403903e0`), `0x00270006` (`0x1403ae7b0`); PID: `0x000006C3` (`Appearance_ClassList`), `0x10004E8F` (`Mount_Appearances_Slot_Colourization`) |

### Class `0x0E43` (`3651`) — MithrilCoinsSystem (Mithril Coin Fast-Purchase Request) <a id="rpc-class-0x0e43"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0E41`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x100054C8` (`Item_MithrilCost`), `0x0000007D` (`PSDesc_DrawReflection`)
- **Referenced WLib Strings (`0x0F`, 2 total)**: `"Collection for MithrilPurchase Paper Item Service contained an invalid item to p"`, `"MithrilCoinsSystem::CanPlayerPurchaseItemForMithril"`
- **Referenced Native Functions (`0x15`, 4 total)**: `0x00E70001` (`0x140821170`), `0x00340004` (`0x140382f70`), `0x00340006` (`0x140394140`), `0x00DD0001` (`0x1403b6150`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x8E410001` @ `0x140b66770` (fn `0x140b66720`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8E430002` | `2` | Pack (C->S) | `0x8E430001` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x8E410001` (`+1119384`) [C++: `0x140b66770`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0E7C` (`3708`) — CRandomItemRewardSystem (Daily/Weekly Login & Hobbit Gift Rewards) <a id="rpc-class-0x0e7c"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0E7D`
- **Method Counts**: `1` Pack (C->S), `2` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 14 total)**: `0x000001E8` (`PreprocOptions_TexGen_Directory`), `0x000006C3` (`Appearance_ClassList`), `0x0000008D` (`Script_IntensityMultiplierVariance`), `0x0000009B` (`UIB_MediaDesc_image_file`), `0x000000B4` (`DecalVolume_ColorAndOpacity`), `0x00000072` (`UICore_Button_broadcast_when_ghosted`), `0x00000082` (`AfterEffectScript_Priority`), `0x0000008F` (`Volume_WaterSurface`), `0x10005657` (`LoginReward_AccountResetTime_Array`), `0x10005654` (`LoginReward_NextResetTime`), `0x10005656` (`LoginReward_ID`), `0x000000B7` (`Entity_CollisionStopScript`), `0x000000FF` (`PreprocOptions_TexGen_FastFilter`), `0x10000103` (`CombatStat_CombatStartTime`)
- **Referenced WLib Strings (`0x0F`, 20 total)**: `">"`, `":"`, `"LogUtils::LogAndAssert"`, `"]"`, `"[File:"`, `"(CLIENT)"`, `"Could not get a valid WTT from DID"`, `"CRandomItemRewardSystem::RecvCEvt_RandomItemRewardedFromTreasureTable"`, `"Could not get the player!"`, `"CRandomItemRewardSystem::UI_IsRewardOnCooldown"` *(+10 more)*
- **Referenced Native Functions (`0x15`, 9 total)**: `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0001` (`0x1403b6150`), `0x00DD0003` (`0x1403b6270`), `0x00DD0008` (`0x1403b5b70`), `0x0E790008` (`0x140b38410`), `0x0E790009` (`0x140b38380`), `0x00E70001` (`0x140821170`), `0x000B0004` (`0x1403bc340`), `0x00CC0003` (`0x1403c1d20`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 5 total)**: `0x8E7D000E` @ `0x140b75861` (fn `0x140b757d0`), `0x8E7D000C` @ `0x140b89667` (fn `0x140b895b0`), `0x8E7D000D` @ `0x140b8fecb` (fn `0x140b8fe80`), `0x8E7D0015` @ `0x140b90a66` (fn `0x140b909f0`), `0x8E7D0014` @ `0x140b98ce6` (fn `0x140b98c70`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8E7C0003` | `1` | Unpack (S->C) | `0x8E7C0003` | `(i64/u64, i32/u32, i32/u32, i32/u32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8E7D0007` (`+1447412`) | NAT: `0x00DD0001` (`0x1403b6150`), `0x0E790008` (`0x140b38410`); PID: `0x000006C3` (`Appearance_ClassList`); MSG: `"Could not get a valid WTT from DID"`, `"CRandomItemRewardSystem::RecvCEvt_RandomItemR"` |
| `0x8E7C000E` | `2` | Unpack (S->C) | `0x8E7C000E` | `()` | `0x00D60007(0x06C3)` | `0x8E7D0010` (`+1447540`) | NAT: `0x0E790009` (`0x140b38380`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8E7C0010` | `4` | Pack (C->S) | `0x8E7C000F` | `(ref)` | `0x00D9000C` (`self`) | `0x8E7D000E` (`+1447720`) [C++: `0x140b75861`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x0000008D` (`Script_IntensityMultiplierVariance`); MSG: `"Could not get the player!"`, `"CRandomItemRewardSystem::UI_GetSecondsUntilNe"` |

### Class `0x0EB6` (`3766`) — CBigBattleSystem (Epic Battles Promotion Points, Medals & Siege Weapons) <a id="rpc-class-0x0eb6"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C2`, `0x06C3` | **Paired Client Implementation Class(es)**: `0x0F12`
- **Method Counts**: `1` Pack (C->S), `5` Unpack (S->C) (`6` total)
- **Referenced Properties (`pids.txt`, 15 total)**: `0x10004A5B` (`Antispam_TraitRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x00000057` (`Tools_Map_WcTemplate`), `0x00000060` (`PhysicsMovement`), `0x00000069` (`Physics_EtherealToType`), `0x10005D81` (`BigBattle_IsSiegeWeapon`), `0x10000182` (`AI_RemoteDetector_Activated`), `0x000001E5` (`PreprocOptions_PlaceGen_Houses`), `0x10005B9E` (`BigBattle_Interactive_MaxRange`), `0x10005BA5` (`BigBattle_Interactive_NearViewLandBlockOverride_Array`), `0x10005BA4` (`BigBattle_Interactive_NearViewLandBlockOverride`), `0x00000104` (`DungeonFlag`), `0x0000007B` (`PEDesc_TimeLimit`), `0x000006C3` (`Appearance_ClassList`), `0x000006C2` (`MapNote_StringInfo`)
- **Referenced WLib Strings (`0x0F`, 25 total)**: `"BigBattles Commands:"`, `"dump, awardmedal, dbuinfo, dbuviz."`, `"-- dump - Dumps all the big battle information we have."`, `"-- awardmedal [QuestDID] [Medal] - Awards the given medal for the named quest."`, `"-- dbuinfo [DBU IID] - Dumps server properties for the specified or selected DBU"`, `"-- dbuviz <DBU DID> [file] - Dumps Worldbuilder visualization properties for the"`, `"Error: BigBattlesControl not found."`, `"BigBattleSystem::GetCostToRefundTraitPoints"`, `"Error: Trait refund cost progression not valid. Must be a float progression. Was"`, `"for Big Battles traits in progression"` *(+15 more)*
- **Referenced Native Functions (`0x15`, 37 total)**: `0x00E10006` (`0x1403b8050`), `0x00DD0001` (`0x1403b6150`), `0x00DD0003` (`0x1403b6270`), `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`), `0x00DD0008` (`0x1403b5b70`), `0x00B60014` (`0x1403c6650`), `0x00E10002` (`0x1403ba7e0`), `0x00A6002F` (`0x1403b37a0`), `0x00440001` (`0x1403b5660`), `0x00E70001` (`0x140821170`), `0x00990004` (`0x140389fa0`), `0x00D30007` (`0x1403a6da0`) *(+23 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 2 total)**: `0x8F120001` @ `0x140b91f40` (fn `0x140b91f10`), `0x8F120002` @ `0x140b6c2c0` (fn `0x140b6c290`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8EB60009` | `2` | Pack (C->S) | `0x8EB60008` | `()` | `0x00D9000C` (`self`) | `0x8F120001` (`+400108`) [C++: `0x140b91f40`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8EB6000C` | `3` | Unpack (S->C) | `0x8EB6000C` | `(ref)` | `0x00D60007(0x06C3)` | `0x8F120005` (`+400828`) | NAT: `0x0EC90003` (`0x140a6cff0`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`); PID: `0x00000104` (`DungeonFlag`), `0x000006C3` (`Appearance_ClassList`); MSG: `"Error: Couldn't get Big Battles Control!"`, `"CBigBattleSystem::RecvCEvt_InitializeMedalRec"` |
| `0x8EB6000E` | `4` | Unpack (S->C) | `0x8EB6000E` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x8F120006` (`+401064`) | NAT: `0x0F230002` (`0x140b3e400`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8EB60010` | `5` | Unpack (S->C) | `0x8EB60010` | `(ref, i32/u32)` | `0x00D60007(0x06C3)` | `0x8F120007` (`+401092`) | NAT: `0x0F230003` (`0x140b34040`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8EB60017` | `6` | Unpack (S->C) | `0x8EB60017` | `()` | `0x00D60007(0x06C3)` | `0x8F120008` (`+401120`) | NAT: `0x01810004` (`0x140b2ef90`), `0x01810003` (`0x140b2f8a0`); PID: `0x0000007B` (`PEDesc_TimeLimit`), `0x000006C3` (`Appearance_ClassList`) |
| `0x8EB6001B` | `7` | Unpack (S->C) | `0x8EB6001B` | `(ref, ref, ref, ref, i64/u64)` | `0x00D60007(0x06C2)` | `0x8F12000A` (`+401252`) | NAT: `0x0030000B` (`0x140385ab0`), `0x0EB30002` (`0x140b34a90`); PID: `0x000006C2` (`MapNote_StringInfo`) |

### Class `0x0ED5` (`3797`) — CMultiUsageSystem (Multi-Action Interactive Object Context Menu & Lock) <a id="rpc-class-0x0ed5"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0EDD`
- **Method Counts**: `4` Pack (C->S), `6` Unpack (S->C) (`10` total)
- **Referenced Properties (`pids.txt`, 37 total)**: `0x000006C3` (`Appearance_ClassList`), `0x10005D92` (`MultiUsage_TutorialHint`), `0x1000063F` (`Skill_AttackHook_fxAttackerHit`), `0x1000062E` (`Signals.MakeDecision@NPC::Combat::Duration`), `0x10000648` (`Discount_Eredluin`), `0x10000687` (`GRSEvent_Exploit_PositionString`), `0x100006CA` (`Communication_LocalizedChannel_List`), `0x100000AD` (`Combat_WeaponDamageMultiplier_2HAxe`), `0x1000062C` (`AI_CallForHelpFilter`), `0x100000D2` (`Movement_ActiveMovementFrozen_ByEffect`), `0x10005D84` (`CSM_Option_MultiUsage`), `0x0000037D` (`Slider_Alpha`), `0x100057AE` (`MultiUsage_ActivityCollection`), `0x10005BFB` (`MultiUsage_HostingEnabled`), `0x10005830` (`MultiUsage_CurrentUsageTarget`), `0x00000058` (`Tools_VisibilityArray`), `0x10005A13` (`Antispam_MultiUsage`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`) *(+19 more)*
- **Referenced WLib Strings (`0x0F`, 38 total)**: `"dumpcache"`, `"Client MultiUsage Registry Cache:"`, `"<uninitialized>"`, `"<empty>"`, `"]:"`, `"["`, `"Sending request to server..."`, `"CMultiUsageSystem::TakeInteractionLock"`, `"CMultiUsageSystem::ReleaseInteractionLock"`, `"Failed to retrieve CSM_Option_MultiUsage property!"` *(+28 more)*
- **Referenced Native Functions (`0x15`, 44 total)**: `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00E10016` (`0x1403ba9d0`), `0x00DD0008` (`0x1403b5b70`), `0x00750009` (`0x1403ba760`), `0x0075000A` (`0x1403bfc90`), `0x00760006` (`0x1403c50c0`), `0x00760002` (`0x1403ba230`), `0x00760003` (`0x1403ba130`), `0x00DD0001` (`0x1403b6150`), `0x0CDF0007` (`0x140a914d0`), `0x00760001` (`0x1403c4e10`), `0x0030000B` (`0x140385ab0`), `0x0EE70007` (`0x140b3b7c0`) *(+30 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 9 total)**: `0x8EDD0005` @ `0x140b99e3c` (fn `0x140b99de0`), `0x8EDD0006` @ `0x140b660cc` (fn `0x140b66070`), `0x8EDD001E` @ `0x140b64143` (fn `0x140b640ae`), `0x8EDD000F` @ `0x140b67302` (fn `0x140b6726e`), `0x8EDD0007` @ `0x140b674b2` (fn `0x140b6741e`), `0x8EDD0022` @ `0x140b70894` (fn `0x140b70825`), `0x8EDD001A` @ `0x140b7ba11` (fn `0x140b7b940`), `0x8EDD0021` @ `0x140b7e992` (fn `0x140b7e8a4`), `0x8EDD001B` @ `0x140b7fb16` (fn `0x140b7f9c1`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8ED50010` | `1` | Unpack (S->C) | `0x8ED50010` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8EDD0002` (`+1168416`) | NAT: `0x00750003` (`0x1403bb3c0`), `0x0EE70002` (`0x140b40fe0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8ED50011` | `2` | Unpack (S->C) | `0x8ED50011` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8EDD0003` (`+1168300`) | NAT: `0x0EE70002` (`0x140b40fe0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8ED50016` | `4` | Unpack (S->C) | `0x8ED50016` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8EDD0001` (`+1168328`) | NAT: `0x00750002` (`0x1403ca100`), `0x0EE70003` (`0x140b41090`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8ED50018` | `6` | Pack (C->S) | `0x8ED50014` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8EDD0005` (`+1165884`) [C++: `0x140b99e3c`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8ED50019` | `7` | Pack (C->S) | `0x8ED50017` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8EDD0006` (`+1166044`) [C++: `0x140b660cc`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8ED5001F` | `9` | Pack (C->S) | `0x8ED5001E` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8EDD0029` (`+1166552`) | — |
| `0x8ED50021` | `10` | Unpack (S->C) | `0x8ED50021` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8EDD0009` (`+1168492`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8ED50027` | `11` | Unpack (S->C) | `0x8ED50027` | `(i32/u32, i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8EDD000D` (`+1168528`) | NAT: `0x0EE70005` (`0x140b2fef0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8ED5002B` | `13` | Pack (C->S) | `0x8ED5002A` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8EDD0028` (`+1166960`) | — |
| `0x8ED50032` | `14` | Unpack (S->C) | `0x8ED50032` | `(ref, i64/u64, i64/u64)` | `0x00D60007(0x06C3)` | `0x8EDD0018` (`+1168616`) | NAT: `0x0EE70008` (`0x140b367e0`), `0x0EE70009` (`0x140b368d0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0EF0` (`3824`) — DistributedChargingSystem (DistributedCharging_UserRate Sync) <a id="rpc-class-0x0ef0"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0EFC`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x1000596C` (`DistributedCharging_UserRate`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 13 total)**: `0x0023000A` (`0x1403bf2a0`), `0x00240006` (`0x1403c50c0`), `0x00240002` (`0x1403b9f80`), `0x00240003` (`0x1403ba130`), `0x0EFB0001` (`0x140b32970`), `0x00240001` (`0x1403c4e10`), `0x00230004` (`0x1403bc7d0`), `0x0EF80002` (`0x140a7f110`), `0x00E70001` (`0x140821170`), `0x0EF80003` (`0x140a7dc70`), `0x00750004` (`0x1403bd030`), `0x00750002` (`0x1403ca100`), `0x00230002` (`0x1403c9a70`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 1 total)**: `0x8EFC0003` @ `0x140b7d3af` (fn `0x140b7d2a1`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8EF0000B` | `1` | Unpack (S->C) | `0x8EF0000B` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x8EFC0001` (`+555760`) | NAT: `0x0023000A` (`0x1403bf2a0`), `0x00240006` (`0x1403c50c0`), `0x00240002` (`0x1403b9f80`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0F5C` (`3932`) — Lockbox / Lootbox Disenchant & Inspection System (0x0F60) <a id="rpc-class-0x0f5c"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0F60`
- **Method Counts**: `2` Pack (C->S), `0` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 2 total)**: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 4 total)**: `0x8F600002` @ `0x140b98fde` (fn `0x140b98f68`), `0x8F600006` @ `0x140b65af6` (fn `0x140b65ab6`), `0x8F600001` @ `0x140b64ecc` (fn `0x140b64e70`), `0x8F600003` @ `0x140b7d1da` (fn `0x140b7d0e0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8F5C0004` | `2` | Pack (C->S) | `0x8F5C0003` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x8F600002` (`+865688`) [C++: `0x140b98fde`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x8F5C0008` | `4` | Pack (C->S) | `0x8F5C0007` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x8F600006` (`+865936`) [C++: `0x140b65af6`] | NAT: `0x00E70001` (`0x140821170`), `0x0030000A` (`0x140386f40`) |

### Class `0x0FA0` (`4000`) — CollectionSystem (Mounts/Pets/Emotes Collection Fulfill & Refresh) <a id="rpc-class-0x0fa0"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0F9F`
- **Method Counts**: `0` Pack (C->S), `2` Unpack (S->C) (`2` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 6 total)**: `"usage: collection [ clearcompleted \| refreshall \| fulfill ]"`, `"usage: collection"`, `"clearcompleted: clears the completed collection hash so you can earn the rewards"`, `"refreshall: refreshes all collections to flag them as completed if necessary."`, `"fulfill <collection.wc>: fulfills the specified collection."`, `"dump: dumps your completed collections hashset."`
- **Referenced Native Functions (`0x15`, 12 total)**: `0x00E70001` (`0x140821170`), `0x0011000C` (`0x140385130`), `0x00110004` (`0x1403bea40`), `0x00340004` (`0x140382f70`), `0x0034000F` (`0x14037eaa0`), `0x00110018` (`0x1403c6bd0`), `0x00120001` (`0x1403b6790`), `0x0FA20002` (`0x140b33530`), `0x0FA20001` (`0x140b335a0`), `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00DD0008` (`0x1403b5b70`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 3 total)**: `0x8F9F0005` @ `0x140b6aeaf` (fn `0x140b6ada0`), `0x8F9F0001` @ `0x140b6af95` (fn `0x140b6af00`), `0x8F9F0004` @ `0x140b73185` (fn `0x140b730f0`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8FA00002` | `1` | Unpack (S->C) | `0x8FA00002` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x8F9F0002` (`+463188`) | NAT: `0x00120001` (`0x1403b6790`), `0x0FA20002` (`0x140b33530`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x8FA00004` | `2` | Unpack (S->C) | `0x8FA00004` | `(ref)` | `0x00D60007(0x06C3)` | `0x8F9F0003` (`+463284`) | NAT: `0x0FA20001` (`0x140b335a0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x0FBC` (`4028`) — ItemSocketSystem (Essence Socket / Unsocket / Level Manipulation) <a id="rpc-class-0x0fbc"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0F71`
- **Method Counts**: `1` Pack (C->S), `2` Unpack (S->C) (`3` total)
- **Referenced Properties (`pids.txt`, 26 total)**: `0x100000E4` (`Trait_Burglar_DisabledAttacks`), `0x00000081` (`AfterEffectScript_OverlayTexture`), `0x00000080` (`BeamScript_UseTargetPosition`), `0x10005F32` (`Examination_ItemSocketInfo_Struct`), `0x00000321` (`AnimConfigurableStruct`), `0x10000BC8` (`Examination_EffectInfo_Array`), `0x10005F07` (`Item_Socket_Array`), `0x10006A88` (`Item_Socket_Unlock_ILevel`), `0x10006973` (`Item_Socket_Type`), `0x10006A43` (`Item_Socket_RequiredMinLevel`), `0x10006A44` (`Item_Socket_RequiredMaxLevel`), `0x10005F0E` (`Item_Socket_Gem_Array`), `0x00000268` (`ListenerScript_EventType`), `0x00000289` (`Appearance_Modifier`), `0x1000610D` (`Inventory_UniqueEquipped`), `0x1000015F` (`MoveItemDistance`), `0x100000B2` (`Combat_Monster_Aura_Radius`), `0x10000722` (`Skill_AttackHook_fxWeaponHit`) *(+8 more)*
- **Referenced WLib Strings (`0x0F`, 12 total)**: `"usage: itemsocket [ dump \| socket \| unsocket \| level ]"`, `"usage: itemsocket"`, `"dump [iid] -- dumps the socketing state of the selected item or supplied iid"`, `"extractall [iid] -- removes all gems from the selected item or supplied iid and "`, `"socket <gemDID> <index> [gemLevel] [iid] -- sockets the given gem into the speci"`, `"unsocket <gemDID> <index> [iid] -- removes the given gem at the specified zero-b"`, `"level <index> <value> [iid] -- sets a gem's socket level."`, `"Failed to get gem Item singleton from"`, `"ItemSocketSystem::GetGemItemSocketType"`, `"ItemSocketSystem::IsGemUniqueEquipped"` *(+2 more)*
- **Referenced Native Functions (`0x15`, 34 total)**: `0x00E70001` (`0x140821170`), `0x00B60020` (`0x1403bb9d0`), `0x00B6001E` (`0x1403c5c30`), `0x00330001` (`0x140384e80`), `0x0030000A` (`0x140386f40`), `0x01810004` (`0x140b2ef90`), `0x01810003` (`0x140b2f8a0`), `0x0F760002` (`0x140b42a50`), `0x0030000B` (`0x140385ab0`), `0x00270006` (`0x1403ae7b0`), `0x00270037` (`0x1403b0510`), `0x00DD0008` (`0x1403b5b70`), `0x00270031` (`0x140390320`), `0x00270036` (`0x140399330`) *(+20 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 13 total)**: `0x8F710017` @ `0x140b9138c` (fn `0x140b912f0`), `0x8F710011` @ `0x140b69713` (fn `0x140b69640`), `0x8F710019` @ `0x140b6cdfb` (fn `0x140b6cdb0`), `0x8F710010` @ `0x140b6dcec` (fn `0x140b6dc90`), `0x8F710013` @ `0x140b76c09` (fn `0x140b76ba0`), `0x8F71000E` @ `0x140b76cac` (fn `0x140b76c50`), `0x8F71001C` @ `0x140b76d59` (fn `0x140b76cf0`), `0x8F710014` @ `0x140b76e79` (fn `0x140b76da0`), `0x8F710015` @ `0x140b78eac` (fn `0x140b78e50`), `0x8F71001F` @ `0x140b8aec5` (fn `0x140b8ade0`), `0x8F71001B` @ `0x140b8b067` (fn `0x140b8af20`), `0x8F71000D` @ `0x140b9028c` (fn `0x140b90230`) *(+1 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8FBC0003` | `1` | Unpack (S->C) | `0x8FBC0003` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8F71000B` (`+942580`) | NAT: `0x01810004` (`0x140b2ef90`), `0x01810003` (`0x140b2f8a0`), `0x0F760002` (`0x140b42a50`); PID: `0x00000080` (`BeamScript_UseTargetPosition`), `0x000006C3` (`Appearance_ClassList`) |
| `0x8FBC0004` | `2` | Unpack (S->C) | `0x8FBC0004` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x8F71000C` (`+942528`) | NAT: `0x01810004` (`0x140b2ef90`), `0x01810003` (`0x140b2f8a0`), `0x0F760002` (`0x140b42a50`); PID: `0x00000081` (`AfterEffectScript_OverlayTexture`), `0x000006C3` (`Appearance_ClassList`) |
| `0x8FBC000C` | `4` | Pack (C->S) | `0x8FBC000B` | `(ref, i32/u32)` | `0x00D9000C` (`self`) | `0x8F710017` (`+943044`) [C++: `0x140b9138c`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x0FF6` (`4086`) — CAdmin Slot 1326 Single-Integer RPC Command Stub (0x0FFA) <a id="rpc-class-0x0ff6"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x0FFA`
- **Method Counts**: `1` Pack (C->S), `0` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`)**: None in immediate RPC/paired scope
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`)**: Only `0x008E`/`0x00D9`/`0x00D6` RPC primitives
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x8FF60002` | `2` | Pack (C->S) | `0x8FF60001` | `(ref)` | `0x00D9000D` (`local_0`) | `0x8FFA0001` (`+1809100`) | — |

### Class `0x1001` (`4097`) — CImageItemSystem (RecvCEvt_ShowImageItem UI Display) <a id="rpc-class-0x1001"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x0FFF`
- **Method Counts**: `0` Pack (C->S), `1` Unpack (S->C) (`1` total)
- **Referenced Properties (`pids.txt`, 2 total)**: `0x00000126` (`WeenieType`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 12 total)**: `"ERROR: Failed to locate item with ID"`, `"CImageItemSystem::RecvCEvt_ShowImageItem"`, `", however the item is not a PopupImageItem!"`, `"ERROR: Received directive to do a PopupImageItem read on item"`, `">"`, `":"`, `"LogUtils::LogAndAssert"`, `"INVALID_IID"`, `"(CLIENT)"`, `"]"` *(+2 more)*
- **Referenced Native Functions (`0x15`, 14 total)**: `0x0030000B` (`0x140385ab0`), `0x00DD0001` (`0x1403b6150`), `0x10020001` (`0x140b41b90`), `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0003` (`0x1403b6270`), `0x00DD0008` (`0x1403b5b70`), `0x00DD0010` (`0x1403b3140`), `0x00670002` (`0x1403a2fc0`), `0x003D0008` (`0x140389270`), `0x00480002` (`0x140384090`), `0x00D30011` (`0x14038e380`), `0x00D30010` (`0x140388040`), `0x00D3000F` (`0x1403869b0`), `0x00DD0004` (`0x1403b5d10`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`)**: Invoked via WLib internal/virtual dispatch

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x90010002` | `1` | Unpack (S->C) | `0x90010002` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x8FFF0002` (`+822220`) | NAT: `0x0030000B` (`0x140385ab0`), `0x00DD0001` (`0x1403b6150`), `0x10020001` (`0x140b41b90`); PID: `0x000006C3` (`Appearance_ClassList`); MSG: `"ERROR: Failed to locate item with ID"`, `"CImageItemSystem::RecvCEvt_ShowImageItem"` |

### Class `0x1012` (`4114`) — AllegianceSystem (Allegiance Faction Level, Points & Claim Rewards) <a id="rpc-class-0x1012"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x1011`
- **Method Counts**: `3` Pack (C->S), `1` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 8 total)**: `0x10006504` (`Allegiance_Member_Struct`), `0x10006505` (`Allegiance_Member_Array`), `0x100064FD` (`Allegiance_Type_Entry`), `0x100069BA` (`Allegiance_Group`), `0x10006507` (`Allegiance_Advancement_Progression_Entry`), `0x10006508` (`Allegiance_Points_Earned`), `0x100069C2` (`Allegiance_Group_Array`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 5 total)**: `"Usage: /allegiance <clear\|dump\|set <level <allegiance wc, level>> \| <rewardfl"`, `"clear: Removes all Allegiance information on the selected player."`, `"dump: Displays all Allegiance info for the selected player."`, `"set level: Modifies Allegiance points to be the minimum for the specified level."`, `"set rewardflags: Modifies Allegiance reward redemption flags."`
- **Referenced Native Functions (`0x15`, 19 total)**: `0x00E10006` (`0x1403b8050`), `0x000B0009` (`0x1403ba760`), `0x000B0004` (`0x1403bc340`), `0x00270001` (`0x14038a7e0`), `0x00270030` (`0x1403903e0`), `0x00270031` (`0x140390320`), `0x00270036` (`0x140399330`), `0x00270024` (`0x140392100`), `0x00270017` (`0x1403944b0`), `0x0023000A` (`0x1403bf2a0`), `0x00240006` (`0x1403c50c0`), `0x00240002` (`0x1403b9f80`), `0x00190006` (`0x1403c2c90`), `0x00240001` (`0x1403c4e10`) *(+5 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 19 total)**: `0x90110004` @ `0x140b90af0` (fn `0x140b90aa0`), `0x90110009` @ `0x140b65e66` (fn `0x140b65df0`), `0x9011000C` @ `0x140b99310` (fn `0x140b992c0`), `0x9011000B` @ `0x140b67f42` (fn `0x140b67ee0`), `0x90110021` @ `0x140b6890b` (fn `0x140b688c0`), `0x9011001C` @ `0x140b689da` (fn `0x140b68950`), `0x90110016` @ `0x140b68aee` (fn `0x140b68a30`), `0x90110007` @ `0x140b68cdb` (fn `0x140b68c90`), `0x90110012` @ `0x140b68d82` (fn `0x140b68d20`), `0x90110006` @ `0x140b68e1b` (fn `0x140b68dd0`), `0x90110008` @ `0x140b68ed1` (fn `0x140b68e60`), `0x90110014` @ `0x140b68f82` (fn `0x140b68f20`) *(+7 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x90120002` | `2` | Pack (C->S) | `0x90120001` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x90110004` (`+359892`) [C++: `0x140b90af0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x9012000E` | `4` | Pack (C->S) | `0x9012000D` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x90110009` (`+359820`) [C++: `0x140b65e66`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x90120010` | `6` | Pack (C->S) | `0x9012000F` | `(i32/u32)` | `0x00D9000C` (`self`) | `0x9011000C` (`+360000`) [C++: `0x140b99310`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x90120013` | `7` | Unpack (S->C) | `0x90120013` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x90110010` (`+360452`) | PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x101B` (`4123`) — AccountFilterSystem (Loot, Quest & Audio Filter Lists, 0x101C) <a id="rpc-class-0x101b"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: None | **Paired Client Implementation Class(es)**: `0x101C`
- **Method Counts**: `4` Pack (C->S), `0` Unpack (S->C) (`4` total)
- **Referenced Properties (`pids.txt`, 11 total)**: `0x000000C8` (`UICore_Button_hot_click_interval`), `0x100068C7` (`Antispam_FilterRequest`), `0x100000BD` (`QuestReward_SessionFailurePointsEntry`), `0x00000321` (`AnimConfigurableStruct`), `0x10006547` (`Account_Filter_ItemArray`), `0x10006541` (`Filter_ItemEntry`), `0x10006544` (`Filter_ItemArray`), `0x00000576` (`MoveToScript_TurnOffset`), `0x10006543` (`Account_Filter_QuestArray`), `0x10006546` (`Filter_QuestEntry`), `0x10006545` (`Filter_QuestArray`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 1 total)**: `0x00E70001` (`0x140821170`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 7 total)**: `0x901C0005` @ `0x140b50c10` (fn `0x140b50b70`), `0x901C0009` @ `0x140b50eb0` (fn `0x140b50e10`), `0x901C000B` @ `0x140b50dd0` (fn `0x140b50d30`), `0x901C0010` @ `0x140b50cf0` (fn `0x140b50c50`), `0x901C000D` @ `0x140b53870` (fn `0x140b53840`), `0x901C000E` @ `0x140b8edbb` (fn `0x140b8ed70`), `0x901C000F` @ `0x140b8fc7b` (fn `0x140b8fc30`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x901B000E` | `4` | Pack (C->S) | `0x901B000C` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x901C0005` (`+738428`) [C++: `0x140b50c10`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x901B0010` | `5` | Pack (C->S) | `0x901B0006` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x901C000B` (`+738644`) [C++: `0x140b50dd0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x901B0014` | `6` | Pack (C->S) | `0x901B0007` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x901C0009` (`+738752`) [C++: `0x140b50eb0`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x901B001C` | `8` | Pack (C->S) | `0x901B001B` | `(i32/u32, i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x901C0010` (`+738536`) [C++: `0x140b50cf0`] | NAT: `0x00E70001` (`0x140821170`) |

### Class `0x1055` (`4181`) — CarryAllSystem (Bag of Holding / Carry-All Pseudonym & Item Management) <a id="rpc-class-0x1055"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x1054`
- **Method Counts**: `7` Pack (C->S), `4` Unpack (S->C) (`11` total)
- **Referenced Properties (`pids.txt`, 15 total)**: `0x10006774` (`BoS_AllowInSingletonBag`), `0x0000017B` (`PEDesc_Exploding`), `0x00000354` (`Name`), `0x1000677B` (`BoS_ItemStruct`), `0x00000313` (`AnimationScript_Speed`), `0x10006775` (`BoS_Bound_To_Account_Amount`), `0x10000053` (`Title_Priority`), `0x10006788` (`BoS_Bound_To_Character_Amount`), `0x10006789` (`BoS_Quantity`), `0x10006778` (`BoS_TotalNeeded`), `0x10000065` (`UI_Vendor_FilterType`), `0x10006780` (`BoS_ItemDID`), `0x10006786` (`BoS_ItemName`), `0x10006782` (`BoS_ItemArray`), `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`, 3 total)**: `"/carryallpseudonym set <name>"`, `"set - Sets the pseudonym of selected carryall"`, `"Error: Invalid Target."`
- **Referenced Native Functions (`0x15`, 22 total)**: `0x00E70001` (`0x140821170`), `0x00270001` (`0x14038a7e0`), `0x00C70019` (`0x14039dba0`), `0x00270036` (`0x140399330`), `0x00270015` (`0x140393f20`), `0x00C7000C` (`0x1403afdf0`), `0x00C70009` (`0x14037c4c0`), `0x00C70003` (`0x14037c2f0`), `0x00C70005` (`0x14037c560`), `0x00270024` (`0x140392100`), `0x00270028` (`0x140397600`), `0x1066000F` (`0x140b421b0`), `0x10660010` (`0x140b424c0`), `0x10660003` (`0x140b42260`) *(+8 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 18 total)**: `0x9054001A` @ `0x140b9b6ef` (fn `0x140b9b60f`), `0x9054001D` @ `0x140b66203` (fn `0x140b6614e`), `0x90540020` @ `0x140b67a1f` (fn `0x140b67996`), `0x90540026` @ `0x140b99564` (fn `0x140b994bf`), `0x9054003A` @ `0x140b61b51` (fn `0x140b61af0`), `0x9054004F` @ `0x140b9716d` (fn `0x140b970bd`), `0x9054002F` @ `0x140b530cc` (fn `0x140b53040`), `0x9054002D` @ `0x140b533b4` (fn `0x140b53345`), `0x9054000E` @ `0x140b638ff` (fn `0x140b63876`), `0x9054004C` @ `0x140b6a2d0` (fn `0x140b6a240`), `0x90540027` @ `0x140b6cc4a` (fn `0x140b6cba0`), `0x9054001E` @ `0x140b6cec8` (fn `0x140b6ce40`) *(+6 more)*

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x90550006` | `6` | Pack (C->S) | `0x90550002` | `(i32/u32, i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x9054001A` (`+861684`) [C++: `0x140b9b6ef`] | — |
| `0x90550007` | `7` | Pack (C->S) | `0x90550003` | `(i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x90540026` (`+861768`) [C++: `0x140b99564`] | — |
| `0x90550008` | `8` | Pack (C->S) | `0x90550001` | `(i64/u64, i64/u64)` | `0x00D9000C` (`self`) | `0x9054001D` (`+861644`) [C++: `0x140b66203`] | — |
| `0x90550009` | `9` | Pack (C->S) | `0x90550005` | `(i32/u32, i32/u32, i64/u64)` | `0x00D9000C` (`self`) | `0x9054001B` (`+861840`) | — |
| `0x9055000A` | `10` | Pack (C->S) | `0x90550004` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x90540020` (`+861732`) [C++: `0x140b67a1f`] | — |
| `0x9055000D` | `11` | Unpack (S->C) | `0x9055000D` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x90540034` (`+863764`) | NAT: `0x10660003` (`0x140b42260`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90550010` | `12` | Unpack (S->C) | `0x90550010` | `(i32/u32, i64/u64)` | `0x00D60007(0x06C3)` | `0x90540035` (`+863836`) | NAT: `0x10660008` (`0x140b42310`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90550015` | `14` | Unpack (S->C) | `0x90550015` | `(ref, i64/u64)` | `0x00D60007(0x06C3)` | `0x90540039` (`+863668`) | NAT: `0x1066000F` (`0x140b421b0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90550016` | `15` | Pack (C->S) | `0x90550014` | `(i64/u64)` | `0x00D9000C` (`self`) | `0x9054003A` (`+861588`) [C++: `0x140b61b51`] | NAT: `0x00E70001` (`0x140821170`); PID: `0x0000017B` (`PEDesc_Exploding`) |
| `0x9055001F` | `17` | Pack (C->S) | `0x9055001A` | `(ref, i64/u64)` | `0x00D9000C` (`self`) | `0x9054004F` (`+861808`) [C++: `0x140b9716d`] | — |
| `0x90550020` | `18` | Unpack (S->C) | `0x90550020` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x9054004D` (`+863740`) | NAT: `0x10660010` (`0x140b424c0`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x1086` (`4230`) — Collection / Shortcut / Combat State UI Notification System (0x1087 / 0x1089) <a id="rpc-class-0x1086"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x1087`
- **Method Counts**: `1` Pack (C->S), `8` Unpack (S->C) (`9` total)
- **Referenced Properties (`pids.txt`, 1 total)**: `0x000006C3` (`Appearance_ClassList`)
- **Referenced WLib Strings (`0x0F`)**: None
- **Referenced Native Functions (`0x15`, 6 total)**: `0x10890001` (`0x140b464a0`), `0x10890003` (`0x140b38ee0`), `0x10890002` (`0x140b32a20`), `0x10890004` (`0x140b46c50`), `0x00E70001` (`0x140821170`), `0x0011000C` (`0x140385130`)
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 12 total)**: `0x90870004` @ `0x140b65f16` (fn `0x140b65ea0`), `0x90870008` @ `0x140b63da3` (fn `0x140b63d20`), `0x9087000F` @ `0x140b6ac0c` (fn `0x140b6abb0`), `0x9087000C` @ `0x140b6c35c` (fn `0x140b6c300`), `0x9087000B` @ `0x140b6c3fc` (fn `0x140b6c3a0`), `0x9087000A` @ `0x140b6c933` (fn `0x140b6c8b0`), `0x90870011` @ `0x140b6cd3c` (fn `0x140b6cce0`), `0x90870009` @ `0x140b7231c` (fn `0x140b722c0`), `0x90870015` @ `0x140b7353c` (fn `0x140b734e0`), `0x90870010` @ `0x140b77e1c` (fn `0x140b77dc0`), `0x90870012` @ `0x140b8e67c` (fn `0x140b8e620`), `0x90870016` @ `0x140b9c4dc` (fn `0x140b9c480`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x90860002` | `1` | Unpack (S->C) | `0x90860002` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x90870002` (`+1464096`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90860004` | `2` | Unpack (S->C) | `0x90860004` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x90870003` (`+1464084`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90860006` | `3` | Unpack (S->C) | `0x90860006` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x90870001` (`+1464108`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90860008` | `5` | Pack (C->S) | `0x90860007` | `(i32/u32, i32/u32)` | `0x00D9000C` (`self`) | `0x90870004` (`+1464252`) [C++: `0x140b65f16`] | NAT: `0x00E70001` (`0x140821170`) |
| `0x90860009` | `6` | Unpack (S->C) | `0x90860009` | `(ref, ref)` | `0x00D60007(0x06C3)` | `0x90870005` (`+1464132`) | NAT: `0x10890001` (`0x140b464a0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x9086000C` | `7` | Unpack (S->C) | `0x9086000C` | `(i32/u32)` | `0x00D60007(0x06C3)` | `0x90870006` (`+1464120`) | PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x9086000D` | `8` | Unpack (S->C) | `0x9086000D` | `(f32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x90870007` (`+1464160`) | NAT: `0x10890003` (`0x140b38ee0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x9086000F` | `9` | Unpack (S->C) | `0x9086000F` | `(i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x9087000D` (`+1464192`) | NAT: `0x10890002` (`0x140b32a20`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x90860011` | `10` | Unpack (S->C) | `0x90860011` | `(f32, i32/u32, i32/u32)` | `0x00D60007(0x06C3)` | `0x9087000E` (`+1464220`) | NAT: `0x10890004` (`0x140b46c50`); PID: `0x000006C3` (`Appearance_ClassList`) |

### Class `0x169F` (`5791`) — NPCSystem / Neighbor NPC System (Housing NPNPC Outfits, Gifts & State) <a id="rpc-class-0x169f"></a>

- **Parent Class**: None | **Unpack Target Interface(s)**: `0x06C3` | **Paired Client Implementation Class(es)**: `0x16A0`
- **Method Counts**: `3` Pack (C->S), `4` Unpack (S->C) (`7` total)
- **Referenced Properties (`pids.txt`, 9 total)**: `0x000006C3` (`Appearance_ClassList`), `0x1000016F` (`Effect_Revive_EffectList`), `0x100077B1` (`NPC_Neighbor_Outfits`), `0x10007794` (`World_Neighbors_NeighborsEnabled`), `0x10000753` (`Skill_RecoveryTime_Trait_Warshot`), `0x1000075D` (`GRSEvent_QuestFinish_NumberCompletions`), `0x100076F2` (`NPC_NeighborQualities`), `0x100077B0` (`NPC_Neighbor_OutfitType`), `0x10000756` (`Trait_Captain_ImprovedCourage`)
- **Referenced WLib Strings (`0x0F`, 9 total)**: `">"`, `":"`, `"LogUtils::LogAndAssert"`, `"> ERROR: Failed to obtain NPC_Neighbor_Outfits_Property!"`, `"NPCSystem::UI_InqOutfitAppearanceProperties"`, `"npnpc - neighborhood persistent npc diagnostic tools."`, `"usage:  /npnpc \n  [clearstate] - Resets the Persisted State of the targeted NPN"`, `"> ERROR: Unable to get internal reference on Item IID:"`, `"NPCSystem::GiveNeighborGift"`
- **Referenced Native Functions (`0x15`, 27 total)**: `0x0030000A` (`0x140386f40`), `0x0CBA000D` (`0x140a7d9f0`), `0x00DD0001` (`0x1403b6150`), `0x00DD0003` (`0x1403b6270`), `0x00B0000B` (`0x140385130`), `0x00B00003` (`0x1403beb80`), `0x00B00012` (`0x1403c6bd0`), `0x16A4001A` (`0x140a775a0`), `0x0039000D` (`0x1403b9b20`), `0x0039001E` (`0x14039ada0`), `0x00420006` (`0x140397700`), `0x00110018` (`0x1403c6bd0`), `0x00340004` (`0x140382f70`), `0x00340003` (`0x140396120`) *(+13 more)*
- **C++ -> WLib Call Sites (`0x1403cf2b0`, 8 total)**: `0x96A00014` @ `0x140b798b3` (fn `0x140b7980e`), `0x96A00044` @ `0x140b81d8a` (fn `0x140b81c80`), `0x96A0000C` @ `0x140b825cc` (fn `0x140b824e2`), `0x96A00034` @ `0x140b82932` (fn `0x140b827c3`), `0x96A0004A` @ `0x140b836d3` (fn `0x140b835e0`), `0x96A00043` @ `0x140b88e8a` (fn `0x140b88d80`), `0x96A00010` @ `0x140b8f492` (fn `0x140b8f444`), `0x96A00047` @ `0x140b99402` (fn `0x140b9937d`)

| `function_id` | `vslot` | Direction | `wire_function_id` | Ordered `WLibArgFrame` Parameter Types | Dispatch / Cast Primitive | Paired Caller / Handler `function_id` (Body) | Method-Level Natives, Strings & Properties |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x969F000D` | `2` | Pack (C->S) | `0x969F000C` | `(ref)` | `0x00D9000D` (`local_0`) | `0x96A00007` (`+1183008`) | — |
| `0x969F0016` | `3` | Unpack (S->C) | `0x969F0016` | `(i64/u64)` | `0x00D60007(0x06C3)` | `0x96A00013` (`+1184376`) | NAT: `0x01520008` (`0x140b3b640`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x969F0019` | `5` | Pack (C->S) | `0x969F0018` | `(i64/u64, i64/u64)` | `0x00D9000D` (`local_0`) | `0x96A00016` (`+1184580`) | NAT: `0x00DD0012` (`0x1403b3140`), `0x00DD0001` (`0x1403b6150`); MSG: `"> ERROR: Unable to get internal reference on "`, `"NPCSystem::GiveNeighborGift"` |
| `0x969F001E` | `6` | Unpack (S->C) | `0x969F001E` | `(ref)` | `0x00D60007(0x06C3)` | `0x96A00021` (`+1184120`) | NAT: `0x00B0000B` (`0x140385130`), `0x00B00003` (`0x1403beb80`), `0x16A40004` (`0x140a6cbe0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x969F0020` | `7` | Unpack (S->C) | `0x969F0020` | `(ref)` | `0x00D60007(0x06C3)` | `0x96A00020` (`+1184312`) | NAT: `0x01520007` (`0x140b397b0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x969F0023` | `8` | Unpack (S->C) | `0x969F0023` | `(ref)` | `0x00D60007(0x06C3)` | `0x96A0001F` (`+1183996`) | NAT: `0x16A40004` (`0x140a6cbe0`), `0x01520007` (`0x140b397b0`), `0x01520004` (`0x140b396b0`); PID: `0x000006C3` (`Appearance_ClassList`) |
| `0x969F004D` | `10` | Pack (C->S) | `0x969F004C` | `(i32/u32, i32/u32)` | `0x00D9000D` (`local_0`) | `0x96A00049` (`+1184752`) | — |

---

## 6. Testable Invariants & Synthetic Test Vectors <a id="invariants-and-test-vectors"></a>

### 6.1 Structural & Wire Invariants <a id="structural-invariants"></a>

1. **Total WLib RPC Census Invariant**: In `client_gamelogic.dat` record `0x56000000` (`wlib.bin`), exactly **120** classes contain methods referencing native package `0x008E` (`WLibArgFrame`), comprising **619** pack bodies (`op 0x0E` `new.native(0x008E)` + `0x00D9000C`/`0x00D9000D`) and **549** unpack bodies (`0x00D60007` + `0x008E000D`), for a total of **1,168** RPC stub bodies.
2. **`function_id` Wire Encoding Invariant**: Every WLib RPC `wire_function_id` has bit 31 set (`0x80000000`), bits 16..30 set to the 15-bit `rpc_class_id`, and bits 0..15 set to the 1-based method number (`fnum`). In all 619 pack stubs, the `wire_function_id` pushed before `0x00D9000C`/`0x00D9000D` belongs to the same `rpc_class_id` and targets the corresponding virtual slot on the remote peer.
3. **`NetBlobEvent` Envelope Header Invariant**: Every `NetBlobEvent::PackUnpackPayload` (`vtable[7]`) writes/reads a 1-byte header (`u8 (flags & 0xFF)`) via `0x1411a3840` before its event-specific payload.
4. **Event `0x30` Standard WLib RPC Wire Invariant**: After the 1-byte header (`0x30`), Event `0x30` (`0x140522950`) serializes `u32 wire_function_id` (4 bytes LE) followed by `u32 arg_frame_byte_length` (4 bytes LE) and `arg_frame_byte_length` raw bytes of the `WLibArgFrame` buffer.

### 6.2 Synthetic Test Vector (Event `0x30` Invoking `0x84E90003` with `(u64, i32)`) <a id="synthetic-test-vector"></a>

A synthetic Event `0x30` payload invoking `CCombatSystem` RPC `0x84E90003` (`class_id = 0x04E9`, `fnum = 3`, parameter signature `(i64/u64, i32/u32)`) with synthetic arguments `arg0 = 0x0200000000000001` (`u64`) and `arg1 = 1` (`u32`) has the following 21-byte payload layout:

| Byte Offset | Size | Value (Hex) | Field Meaning |
| :--- | :--- | :--- | :--- |
| `0x00` | `1` | `30` | `NetBlobEvent::PackUnpackHeader` (`flags & 0xFF = 0x30`) |
| `0x01` | `4` | `03 00 E9 84` | `wire_function_id = 0x84E90003` (little-endian `u32`) |
| `0x05` | `4` | `0C 00 00 00` | `WLibArgFrame` `byte_length = 12` (`0x0000000C`, little-endian `u32`) |
| `0x09` | `8` | `01 00 00 00 00 00 00 02` | `arg0 = 0x0200000000000001` (`0x008E0003` / `0x008E0008` `u64` LE) |
| `0x11` | `4` | `01 00 00 00` | `arg1 = 0x00000001` (`0x008E0002` / `0x008E0007` `u32` LE) |

---

## 7. Open Questions <a id="open-questions"></a>

None.

---

## 8. Change Log <a id="change-log"></a>

- **2026-10-22**: Added complete wire format specification of `0x008E0001` (`WLib::PackObjectRef`) and `0x008E0006` (`WLib::UnpackObjectRef`) object-graph serialization (`0x140361630`, `0x140364cd0`, `0x14035fcf0`, `0x140363520`, `0x140363a00`, `0x140363b80`, `0x140363910`), resolving the last open question (0 OPEN callouts).
- **2026-10-21**: Initial complete census of all 120 WLib RPC classes, 1,168 RPC methods, C++ cross-references, and 198 `NetBlobEvent` factory registrations.
