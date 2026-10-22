<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Official Gameplay Wire Protocol Specification (P1–P6, Mail, Auction House, Housing, Cosmetics, Storage & WLib RPC Census)

- **Status**: Accepted
- **Last updated**: 2026-10-22
- **Author role**: Spec writer (dirty room)
- **Reviewer**: Clean-room reviewer

---

## 1. Scope, Purpose, Compliance Boundaries & Provenance <a id="scope-and-provenance"></a>

### 1.1 Purpose & Scope <a id="purpose-and-scope"></a>

This specification defines the complete wire-level gameplay protocol exchanged between a compatible client and an official-protocol world server after character entry (`official-messages-world.md` and `official-messages-play.md`). It covers:

1. **WLib RPC Framing, `WState` Object-Graph Serialization (`0x008E0001` / `0x008E0006`), Top-Level Gameplay `NetBlobEvent` Envelopes (`0x28`, `0x30`, `0x50`, `0x51`, `0x5F`, `0x64`, `0x81`, `0x8C`, `0xA0`, `0xC0`, `0xC9`, `0xCA`), and the Complete Census of All 120 WLib RPC Classes (`1,168` RPC methods: `549` S→C and `619` C→S) and 458 Native Packages** (§2).
2. **P1 — Character Properties, Level Updates, Inventory, Containers & Equipment** (§3).
3. **P2 — Target Selection, Skills, Quickslots, Inductions, Channels, Cooldowns, Effects, Vitals & Combat Deltas** (§4).
4. **P3 — NPC Interaction, Dialogue, Quests, Deeds, Vendors, Buyback, Repair, Barter, Container/Fellowship/Pending Loot & Wallet (`PaperItem`)** (§5).
5. **P4 — Fellowships, Raids, Votes, Target Marking, Secure Player Trade, Friends & Ignore Lists, `/who` Directory Queries, Kinship (`Guild`) Roster/Calendar & Player Music** (§6).
6. **P5 — Map Markers, Fog-of-War, Entity Tracking, Stable-Master Travel, Milestones, Recall/Summon, Mounts, War-Steeds & Instance Finder (`WorldJoin`, `FellowshipMaker`, `PrivateEncounter`, `Skirmish`)** (§7).
7. **P6 — Progression, Experience, Faction Reputation, Titles, Class Skill Trainers, Trait Trees, Vocations, Professions, Recipes & Crafting** (§8).
8. **Mail System (`0x0514`), Auction House (`0x04E5`), and Player & Kinship Housing (`0x07A7`, `0x169F`, `0x0A04`, `0x0C20`, `0x0F5C`)** (§9).
9. **Cosmetic Outfits (`0x081B`), Barber (`0x083A`), Account Wardrobe (`0x0A45`), Vault & Shared Storage (`0x0A04`), Carry-All Bags (`0x1055`), Player Inspection & Biography (`0x04FD`, `0x0537`, `0x0507`), Skirmish Soldier Traits (`0x0532`, `0x0E15`) & Hobbies (`0x0859`)** (§10).
10. **Complete Summary Index of `crates/net/src/domain.rs` Gameplay Events & Commands** (§11), **Consolidated Hostile-Input Rules, Invariants & Synthetic Test Vectors** (§12), and **Change Log** (§13).

### 1.2 Governance & Compliance Boundaries <a id="compliance-boundaries"></a>

- Per `CLAUDE.md` and `docs/CLEAN_ROOM.md`:
 - No proprietary binaries, `.dat` archives, keys, certificates, server URLs/endpoints, or copyrighted game strings may be embedded in the repository, releases, or documentation.
 - All WLib bytecode definitions (`client_gamelogic.dat` record `0x56000000` `wlib.bin`), Master Property Definitions (`client_gamelogic.dat` record `0x34000000`), Enumeration tables (`client_gamelogic.dat` records `0x2300xxxx`), and UI layouts (`client_highres.dat` records `0x2200xxxx`) are read at runtime from the user's local game installation.
 - Client implementations must respect all server-enforced access controls, account/subscription entitlements, and client-side rate-limiting cooldown properties (`Antispam_*` `0x1000xxxx`) without bypass or modification.

### 1.3 Provenance Legend <a id="provenance-legend"></a>

| Provenance Tag | Confidence | Method | Source Artifact | Question Answered & Why M1/M2 Alone Insufficient |
| :--- | :--- | :--- | :--- | :--- |
| `OBS-WLIB-BIN` (`DAT`) | Confirmed | `M1 (OBS)` | `client_gamelogic.dat` record `0x56000000` (`wlib.bin`) | Direct observation of all 120 WLib RPC proxy classes, 619 C→S pack stubs, 549 S→C unpack stubs, `WLibArgFrame` (`0x008E`) parameter sequences, `WState` script class slot schemas, and paired client system classes. |
| `OBS-MASTER-PROP` (`DAT`) | Confirmed | `M1 (OBS)` | `client_gamelogic.dat` record `0x34000000` (Master Property Definition, 29,219 properties) | Direct observation of all 32-bit `PropertyID` (`0x00000001..0x1000729E`) names, wire type tags, and container/element schemas. |
| `OBS-ENUM-UI` (`DAT`) | Confirmed | `M1 (OBS)` | `client_gamelogic.dat` `0x2300xxxx` `EnumMapper` records & `client_highres.dat` `0x2200xxxx` UI layout records | Direct observation of enum value mappings (`QuestStatus`, `QuestRoleAction`, `FellowshipLootType`, `BankRepositoryType`, `ChestType`) and UI element action IDs. |
| `BIN-CLIENT64` (`BIN`) | Confirmed | `M4 (BIN)` | `x64/lotroclient64.exe` (PE32+ x86-64, SHA-256 `b581fb970c0fd6bc8cbb2fdbeccfdce38ac8cd7de5baa8f5be4be6bc3ca6ac60`, static analysis via Ghidra 11.3.2 & Capstone) | Native C++ `WLibArgFrame` (`0x008E`) and `WState` object-graph serializers (`0x00E1` `Utf16String`, `0x00C7` `StringInfo`, `0x0027` `PropertyValue`, `0x00A6` `PropertyCollection`, `0x01C6` `Shortcut`, `0x0198` `Effect`, `0x0796` `BarterRequest`, `0x0804` `MusicNoteBatch`, `0x0A44` `WorldJoinRequest`, `0x0A39` `WorldJoinInvite`, `0x0188` `AuctionQuery`, `0x0189` `AuctionAdaptor`, `0x01B5` `MailLetter`, `0x07A8..0x0E9C` housing/bank structs), `NetBlobEvent` wire serializers (`0x28`, `0x30`, `0x50`, `0x5F`, `0xC0`, `0xCE`), and the 458 native package registrations exist only in compiled machine code. |

---

## 2. WLib RPC Architecture, `WState` Reference Wire Format, Top-Level Gameplay `NetBlobEvent`s & Full WLib Census <a id="wlib-rpc-architecture"></a>

### 2.1 Two-Class Dispatch Pattern (RPC Proxy Class vs. Client System Class) <a id="two-class-dispatch"></a>

In `wlib.bin`, WLib RPC methods are separated into dedicated **RPC proxy/stub classes** (`120` classes in total: `0x01F5`, `0x04D9..0x0537`, and `0x071A..0x169F`) distinct from the **client gameplay system classes** (`0x003C`, `0x00F6..0x01CE`, `0x06C1..0x06C5`, `0x071B..0x16A0`):

1. **Client -> Server (Pack / Send Path, 619 methods):**
 - C++ UI and gameplay code never invokes the 120 WLib RPC proxy classes directly. Instead, C++ invokes a method on the **paired client gameplay system class** (or `0x06C2` `CAdmin` / `0x06C3` `CPlayer`) via `WLib::CallClassMethod`, passing `(paired_class_id << 16) | fnum`.
 - The paired client system method performs client-side validation (e.g. antispam cooldown checks via `0x00E70001`, interaction lock checks, or argument formatting) and then executes a **static WLib call (`op 0x16`)** directly to the body offset of the corresponding pack stub in the RPC proxy class.
 - The RPC pack stub allocates a `WLibArgFrame` (`op 0x0E` `new.native(0x008E)`), pushes each parameter in order using `0x008E0001..0x008E0005`, pushes the 32-bit `wire_function_id` (`0x80000000 | (rpc_class_id << 16) | target_fnum`), and invokes one of the native RPC dispatchers `0x00D90001..0x00D9000D`.
2. **Server -> Client (Unpack / Receive Path, 549 methods):**
 - When the client receives a WLib RPC `NetBlobEvent` (`0x30`, `0x5F`, or `0xC0`), its `Dispatch` virtual method (`slot[1]`) extracts `wire_function_id = 0x80000000 | (rpc_class_id << 16) | fnum` and resolves the unpack stub in `rpc_class_id`.
 - Every unpack stub begins by calling `0x00D60007(self, iface_class_id)`, which verifies and casts the target entity reference (`self`) to the expected client entity/interface class (most commonly `0x06C3` `CPlayer`, `0x06C2` `CAdmin`, `0x06C1` `CAgent`, or `0x06C5` `CAvatar`). If the cast returns zero, the stub logs error property `0x003C` and returns immediately.
 - Next, the unpack stub calls `0x008E000D(arg_frame)` to reset the `WLibArgFrame` read cursor to byte offset `0`, pops each argument in wire order using `0x008E0006..0x008E000A`, and dispatches (`op 0x16` static call or `op 0x14` virtual call) to the client handler method on the paired gameplay system class.


### 2.2 `WLibArgFrame` (`0x008E`) Native Package Primitives <a id="wlib-arg-frame-primitives"></a>

Native class `0x008E` (`WLibArgFrame`, size `0x20` bytes) wraps an inner `SmartBuffer` (`+0x08..+0x18`) and a 32-bit byte read cursor (`+0x1C`). When serialized inside a `NetBlobEvent`, the frame is written on the wire as a **32-bit little-endian `byte_length` (`u32`)** followed immediately by `byte_length` raw bytes of the inner `SmartBuffer`.


| Native ID | Direction | Parameter Type | Wire Encoding inside `WLibArgFrame` |
| :--- | :--- | :--- | :--- |
| `0x008E0001` | Pack (Write) | `ref` (WLib Object / Native Instance Graph) | `WLib::PackObjectRef`: serializes reachable object graph; see subsection below |
| `0x008E0002` | Pack (Write) | `i32 / u32` (32-bit integer / DID / enum / bool) | 4-byte little-endian `u32` |
| `0x008E0003` | Pack (Write) | `i64 / u64` (64-bit integer / `InstanceID`) | 8-byte little-endian `u64` |
| `0x008E0004` | Pack (Write) | `f32` (32-bit IEEE-754 float) | 4-byte little-endian `f32` |
| `0x008E0005` | Pack (Write) | `f64` (64-bit IEEE-754 double / timestamp) | 8-byte little-endian `f64` |
| `0x008E0006` | Unpack (Read) | `ref` (WLib Object / Native Instance Graph) | `WLib::UnpackObjectRef`: all 279 unpack call sites pass `allow_flag = 0` (`op 0x05`); deserializes object graph and relinks handles |
| `0x008E0007` | Unpack (Read) | `i32 / u32` (32-bit integer / DID / enum / bool) | Reads 4-byte little-endian `u32` at `[frame + 0x1C]`, adds `4` to `[frame + 0x1C]` |
| `0x008E0008` | Unpack (Read) | `i64 / u64` (64-bit integer / `InstanceID`) | Reads 8-byte little-endian `u64` at `[frame + 0x1C]`, adds `8` to `[frame + 0x1C]` |
| `0x008E0009` | Unpack (Read) | `f32` (32-bit IEEE-754 float) | Reads 4-byte little-endian `f32` at `[frame + 0x1C]`, adds `4` to `[frame + 0x1C]` |
| `0x008E000A` | Unpack (Read) | `f64` (64-bit IEEE-754 double / timestamp) | Reads 8-byte little-endian `f64` at `[frame + 0x1C]`, adds `8` to `[frame + 0x1C]` |
| `0x008E000B` | Pack (Copy) | `frame` (`WLibArgFrame` append) | Appends another `WLibArgFrame` buffer |
| `0x008E000C` | Lifecycle | Destroy / Release `WLibArgFrame` | Releases inner `SmartBuffer` |
| `0x008E000D` | Unpack (Reset) | Reset read cursor | Sets `dword ptr [frame + 0x1C] = 0` before unpacking parameters |

#### `0x008E0001` / `0x008E0006` (`ref`) Object-Graph Wire Format <a id="wlib-ref-wire-format"></a>

1. **Graph Traversal**: Starting from `root_ref_id` (where `0xFFFFFFFF` denotes `null`), the serializer traverses all reachable WLib object and native package references, recording `N` unique 32-bit handle IDs in `node_ids[0 .. N-1]` (with `node_ids[0] = root_ref_id`, or `N = 0` if `root_ref_id == 0xFFFFFFFF`).
2. **Node Handle Table**: Writes/reads the packed count `N` (3-tier VLE, maximum `0x7FFF0000`) followed by `N` 4-byte little-endian `u32` handle IDs (`node_ids[0 .. N-1]`). If `N == 0`, no node bodies follow and `0x008E0006` returns `0xFFFFFFFF`.
3. **Per-Node Body**: For each node `i` in `0 .. N-1`:
 - Writes/reads the 32-bit node header word `u32 node_flags`.
 - If `bit 29` (`0x20000000`, external/singleton reference) is set in `node_flags`: writes/reads a 4-byte `u32` external identifier.
 - If `bit 29` (`0x20000000`) is clear: writes/reads the 16-bit WLib class or native package ID (`u16 class_id = word ptr [[node + 0x08]]`), preceded by singleton/entity binding metadata if `bit 27` (`0x08000000`) is set. Then:
 - **Pure WLib Class Instance (`bit 28` = `0x10000000` clear)**: `field_count = (node_flags >> 8) & 0xFFFF`; copies `field_count * 8` raw bytes of the object's 64-bit member slots (`[node + 0x10]`), and on unpack relinks reference-typed member slots (`[class_desc + 0x10]`).
 - **Native Package Instance (`bit 28` = `0x10000000` set)**: invokes the native package's registered `PackUnpack` callback at `qword ptr [[node + 0x08] + 0x68](node + 0x18, smart_buffer)` (e.g. `0x00E1` `WString`, `0x0027` `PropertyCollection`, `0x00A9` `WArray`, `0x0011`/`0x0019`/`0x00B0` containers).


### 2.3 WLib RPC Native Dispatchers (`0x00D9`, `0x00D6`, `0x00DF`) <a id="wlib-rpc-dispatchers"></a>


| Native ID | Role & Calling Convention |
| :--- | :--- |
| `0x00D90001..0x00D9000B` | Server/targeted RPC dispatchers (unused in client pack stubs except `0x00D9000C` / `0x00D9000D`) |
| `0x00D9000C` | **Client->Server RPC Dispatch (`self`)**: pops `(self_entity, wire_function_id, arg_frame_ref)` and queues outbound RPC event for `self_entity` (used by **567** of 619 pack stubs) |
| `0x00D9000D` | **Client->Server RPC Dispatch (`local_0`)**: identical C++ handler to `0x00D9000C`; used by **52** of 619 pack stubs (primarily `0x04DC` / `0x0508` / `0x0511` / `0x052D` / `0x0FF6` admin stubs) |
| `0x00D60007` | **Unpack Entity Interface Cast**: `CastEntityToClass(entity_ref, target_class_id)` — called at the entry of all **549** unpack stubs |
| `0x00DF0001` | **Entity Lookup by 64-bit `InstanceID`**: resolves a `u64` entity ID unpacked via `0x008E0008` into a live client entity reference |
| `0x00DF0005` | **Entity Lookup / Validation Helper**: resolves entity reference during unpack handlers |

### 2.4 WLib RPC & WState Envelope `NetBlobEvent` Wire Formats <a id="wlib-rpc-envelope-wire-formats"></a>

All gameplay `NetBlobEvent` classes inherit from `NetBlobEvent`. Every `NetBlobEvent::PackUnpackPayload` (`vtable[7]`, offset `+0x38`) first calls `NetBlobEvent::PackUnpackHeader`, which serializes the **low byte (`u8`) of `[this + 0x14]` (`flags & 0xFF`)**.

#### `RpcTransactionId` Wire Structure (25 bytes) <a id="rpc-transaction-id-layout"></a>

Transactional WLib RPC events (`0x28`, `0x50`, `0xA0`, `0xC0`, `0xC9`, `0xCA`) embed a 25-byte transaction identifier serialized:

| Wire Offset | Size (Bytes) | Type | Struct Offset | Field Meaning | Byte Order |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `0x00` | `1` | `u8` | `+0x00` | `tx_type` (transaction type / scope discriminator) | N/A |
| `0x01` | `8` | `u64` | `+0x08` | `source_entity_id` (64-bit `InstanceID` A) | Little-endian |
| `0x09` | `4` | `u32` | `+0x10` | `request_sequence_id` (32-bit transaction sequence number) | Little-endian |
| `0x0D` | `2` | `u16` | `+0x14` | `sub_id_a` (16-bit routing / channel qualifier A) | Little-endian |
| `0x0F` | `2` | `u16` | `+0x16` | `sub_id_b` (16-bit routing / channel qualifier B) | Little-endian |
| `0x11` | `8` | `u64` | `+0x18` | `target_entity_id` (64-bit `InstanceID` B) | Little-endian |

#### WLib RPC & WState Envelope Events Summary Table <a id="wlib-envelope-events-table"></a>


| Event ID | Dec | Object Size | Flags (`+0x14`) | Payload Layout (after 1-byte `PackUnpackHeader` `u8`) |
| :--- | :--- | :--- | :--- | :--- |
| `0x28` | `40` | `0x50` | `0x00010028` | **Transactional WLib RPC Status Reply**: `RpcTransactionId` (25 B at `+0x28`) + `u32 status_code` (`+0x4C`) + `u8 status_flag` (`+0x48`) |
| `0x30` | `48` | `0x38` | `0x00000030` | **Standard WLib RPC Call**: `u32 wire_function_id` (`+0x28`) + `WLibArgFrame` (`+0x30`: `u32 byte_len` + `byte_len` bytes) |
| `0x50` | `80` | `0x50` | `0x00000050` | **Transactional WLib RPC Return Frame Reply**: `RpcTransactionId` (25 B at `+0x28`) + `WLibArgFrame` (`+0x48`: `u32 byte_len` + `byte_len` bytes) |
| `0x51` | `81` | `0x30` | `0x00000051` | **WLib RPC 32-bit Control / Ack Event**: `u32 control_word` (`+0x28`) |
| `0x5F` | `95` | `0x40` | `0x0000005F` | **Timestamped WLib RPC Call** (inherits `0x30`): `u32 wire_function_id` (`+0x28`) + `WLibArgFrame` (`+0x30`) + `f64 timestamp` (`+0x38`) |
| `0x64` | `100` | `0x38` | `0x00010064` | **WState Property Delta Sync**: `u8 bool is_clear` (`+0x2C`) + `u8 bool flag_2d` (`+0x2D`) + `u32 seq` (`+0x28`) + if `!is_clear`: `WState` property hash table (`+0x30`) |
| `0x81` | `129` | `0x30` | `0x00000081` | **WState Property Table Sync A**: `u8 bool has_table` + if `has_table`: `WState` property hash table (`+0x28`) |
| `0x8C` | `140` | `0x30` | `0x0000008C` | **WState Property Table Sync B**: identical `Dispatch` and `PackUnpack` to `0x81` (`u8 bool has_table` + optional `WState` property hash table) |
| `0xA0` | `160` | `0x48` | `0x000000A0` | **Transactional WLib RPC Control A**: `RpcTransactionId` (25 B at `+0x28`) |
| `0xC0` | `192` | `0x58` | `0x000000C0` | **Transactional WLib RPC Request** (inherits `0x30`): `u32 wire_function_id` (`+0x28`) + `WLibArgFrame` (`+0x30`) + `RpcTransactionId` (25 B at `+0x38`) |
| `0xC9` | `201` | `0x48` | `0x000000C9` | **Transactional WLib RPC Control B**: `RpcTransactionId` (25 B at `+0x28`) |
| `0xCA` | `202` | `0x48` | `0x000100CA` | **Transactional WLib RPC Control C**: `RpcTransactionId` (25 B at `+0x28`) |

#### Canonical `WState` Native Class IDs Inside `0x008E0001` / `0x008E0006` (`ref`) (`BIN`, `DAT`) <a id="canonical-wstate-native-classes"></a>

Reconciling `docs/specs/official-messages-play.md` §5.3 with the 120 WLib RPC classes and 258 `NativePackageDescriptor` entries in `x64/lotroclient64.exe`, every `ref` argument packed by `0x008E0001` or unpacked by `0x008E0006` encodes a compact class-table-free `WState` object-graph bundle:

1. **`item_count` (`1..5` bytes, 3-tier VLE):** Number of unique object nodes `N` in the graph (`0x00` if the root reference is null / `0xFFFFFFFF`).
2. **`node_ids[0 .. N-1]` (`4 * N` bytes, `u32 LE` each):** 1-based local reference labels (`node_ids[0]` is the root object).
3. **`nodes[0 .. N-1]` (Variable):** For each node `i` in `0 .. N-1`:
 - `u32 LE node_flags`: Header flags word:
 - Bit 29 (`0x20000000`) set: External/singleton reference followed by a 4-byte `u32 LE` external identifier.
 - Bit 29 (`0x20000000`) clear, Bit 28 (`0x10000000`) clear: **WLib Script Class Instance** (`u16 LE class_id` followed by `field_count = (node_flags >> 8) & 0xFFFF` raw 8-byte tagged slot cells `[u32 value_or_low, u32 tag_or_high]`).
 - Bit 29 (`0x20000000`) clear, Bit 28 (`0x10000000`) set: **Native C++ Package Instance** (`u16 LE class_id` followed immediately by the native package's binary archive payload).

| Native `class_id` (`u16 LE`) | Decimal | Canonical Name | Binary Wire Payload Following `u16 LE class_id` |
| :--- | :--- | :--- | :--- |
| `0x000B` | `11` | `HashTable<u32, WLibRef / u32>` (`IntHash`) | `u8 bucket_size_index` (`< 23`, `<= 50,000` buckets) + `VLE entry_count` + `entry_count` key/value pairs. |
| `0x000D` | `13` | `HashTable<u64, WLibRef>` | `u8 bucket_size_index` + `VLE entry_count` + `entry_count` `(u64 key, ref value)` pairs. |
| `0x0011` | `17` | `SmartArray<WLibRef / u32>` (`IntArray`) | `u32 LE element_count` + `element_count` serialized elements. |
| `0x0012` | `18` | `HashTable<u32, u32>` | `u8 bucket_size_index` + `VLE entry_count` + `entry_count × (u32 LE key, u32 LE value)`. |
| `0x0017` | `23` | `HashTable<u64, u64>` | `u8 bucket_size_index` + `VLE entry_count` + `entry_count × (u64 LE key, u64 LE value)`. |
| `0x0019` | `25` | `DoublyLinkedList<WLibRef>` | `u32 LE count` + `count` node references. |
| `0x0023` | `35` | `HashTable<WStr / u32, WLibRef>` (`WHash`) | `u8 bucket_size_index` + `VLE entry_count` + `entry_count` keyed entries. |
| `0x0027` | `39` | `PropertyValue` (`Property`) | `u32 LE property_id` + schema-typed property value payload (`client_gamelogic.dat` `0x34000000`). |
| `0x0061` | `97` | `HashTable<u64 / Str, u32 / WLibRef>` (`LongHash`) | `u8 bucket_size_index` + `VLE entry_count` + `entry_count` keyed entries. |
| `0x0068` | `104` | `SmartArray<u32 / u64>` | `u32 LE element_count` + `element_count` scalar elements. |
| `0x006F` | `111` | `DoublyLinkedList / WArray` | `u32 LE count` + `count` elements. |
| `0x0075` | `117` | `HashTable<WStr / u64, u32 / WLibRef>` | `u8 bucket_size_index` + `VLE entry_count` + `entry_count` keyed entries. |
| `0x008E` | `142` | `WLibArgFrame` (`SmartBuffer`) | `u32 LE byte_length` + `u8[byte_length]` raw bytes. |
| `0x00A6` | `166` | `PropertyCollection` (`PropertySet`) | `u32 LE prop_count` (or `PropertyCollection` header) + `prop_count × (u32 LE property_id, typed_value)`. |
| `0x00B0` | `176` | `SmartArray<u64>` | `u32 LE element_count` + `element_count × u64 LE` values. |
| `0x00B6` | `182` | `DoublyLinkedList<WStr / WLibRef>` (`WArray`) | `u32 LE count` + `count` elements. |
| `0x00C7` | `199` | `StringInfo` | Localized `StringInfo` token (`u32 LE table_did`, `u32 LE token_id`, or literal string flag + UTF-16LE text and variable map; see `official-messages-play.md` §5.3.2). |
| `0x00D3` | `211` | `Position` | World `Position` (`u64 cell_or_block`, `f32 x`, `f32 y`, `f32 z`, quaternion orientation). |
| `0x00E1` | `225` | `Utf16String` (`WString`) | 3-tier VLE `char_count` followed by `char_count × u16 LE` UTF-16LE code units (`official-messages-play.md` §5.3.1). |

---

### 2.5 Complete Census of All 198 Registered Top-Level `NetBlobEvent` Factory IDs & 28 Outbound-Only Constructor IDs <a id="netblob-event-census"></a>

#### 2.5.1 All 198 Registered `NetBlobEvent` Factory IDs (`BIN`) <a id="registered-netblob-factories"></a>

| Event ID (Hex) | Event ID (Dec) | Object Size | Initial Flags (`+0x14`) |
| :--- | :--- | :--- | :--- |
| `0x0001` | `1` | `0x28` | `0x300400` |
| `0x000E` | `14` | `None` | `0x300400` |
| `0x0015` | `21` | `None` | `0x300400` |
| `0x0016` | `22` | `0x30` | `0x300400` |
| `0x0017` | `23` | `None` | `0x300400` |
| `0x0018` | `24` | `None` | `0x300400` |
| `0x001B` | `27` | `None` | `0x300400` |
| `0x001C` | `28` | `0x38` | `0x300400` |
| `0x001D` | `29` | `None` | `0x300400` |
| `0x001E` | `30` | `0x60` | `0x300400` |
| `0x0025` | `37` | `0x30` | `0x300400` |
| `0x0028` | `40` | `0x50` | `0x300400` |
| `0x002F` | `47` | `0x38` | `0x300400` |
| `0x0030` | `48` | `0x38` | `0x300400` |
| `0x0031` | `49` | `0x28` | `0x300400` |
| `0x0032` | `50` | `0x70` | `0x300400` |
| `0x0038` | `56` | `0x30` | `0x300400` |
| `0x0039` | `57` | `None` | `0x300400` |
| `0x003A` | `58` | `0x38` | `0x300400` |
| `0x003C` | `60` | `0x58` | `0x300400` |
| `0x003D` | `61` | `0x48` | `0x300400` |
| `0x003E` | `62` | `0x58` | `0x300400` |
| `0x0040` | `64` | `None` | `0x300400` |
| `0x0043` | `67` | `None` | `0x300400` |
| `0x0046` | `70` | `0x30` | `None` |
| `0x0046` | `70` | `0x30` | `None` |
| `0x0047` | `71` | `0x30` | `0x300400` |
| `0x0048` | `72` | `0x38` | `0x300400` |
| `0x0049` | `73` | `0x78` | `0x300400` |
| `0x004A` | `74` | `0x48` | `0x300400` |
| `0x004C` | `76` | `0x48` | `0x300400` |
| `0x0050` | `80` | `0x50` | `0x300400` |
| `0x0051` | `81` | `0x30` | `0x300400` |
| `0x0052` | `82` | `None` | `0x300400` |
| `0x0054` | `84` | `None` | `0x300400` |
| `0x0057` | `87` | `0x38` | `None` |
| `0x0058` | `88` | `0x48` | `0x300400` |
| `0x0059` | `89` | `0x38` | `0x300400` |
| `0x005A` | `90` | `None` | `0x300400` |
| `0x005E` | `94` | `0x40` | `None` |
| `0x005F` | `95` | `0x40` | `0x300400` |
| `0x0060` | `96` | `None` | `0x300400` |
| `0x0062` | `98` | `0x38` | `0x300400` |
| `0x0063` | `99` | `None` | `0x300400` |
| `0x0064` | `100` | `0x38` | `0x300400` |
| `0x0065` | `101` | `0x58` | `0x300400` |
| `0x0066` | `102` | `0x190` | `0x300400` |
| `0x006B` | `107` | `None` | `0x300400` |
| `0x006C` | `108` | `None` | `0x300400` |
| `0x006D` | `109` | `None` | `0x300400` |
| `0x0070` | `112` | `0x40` | `0x300400` |
| `0x0072` | `114` | `None` | `0x300400` |
| `0x0073` | `115` | `None` | `0x300400` |
| `0x0074` | `116` | `None` | `0x300400` |
| `0x0076` | `118` | `None` | `0x300400` |
| `0x0079` | `121` | `None` | `0x300400` |
| `0x007B` | `123` | `0x88` | `0x300400` |
| `0x007D` | `125` | `0x30` | `0x300400` |
| `0x0080` | `128` | `0x60` | `0x300400` |
| `0x0081` | `129` | `0x30` | `None` |
| `0x0086` | `134` | `None` | `0x300400` |
| `0x0088` | `136` | `None` | `0x300400` |
| `0x0089` | `137` | `None` | `0x300400` |
| `0x008B` | `139` | `0x88` | `0x300400` |
| `0x008C` | `140` | `0x30` | `None` |
| `0x008E` | `142` | `0x30` | `0x300400` |
| `0x008F` | `143` | `0xb8` | `0x300400` |
| `0x0090` | `144` | `None` | `0x300400` |
| `0x0092` | `146` | `None` | `0x300400` |
| `0x0093` | `147` | `None` | `0x300400` |
| `0x0096` | `150` | `0x28` | `0x300400` |
| `0x0098` | `152` | `0x40` | `0x300400` |
| `0x009F` | `159` | `None` | `0x300400` |
| `0x00A0` | `160` | `0x48` | `0x300400` |
| `0x00A1` | `161` | `0x48` | `0x300400` |
| `0x00A2` | `162` | `0x38` | `0x300400` |
| `0x00A4` | `164` | `0x28` | `0x300400` |
| `0x00A7` | `167` | `None` | `0x300400` |
| `0x00A8` | `168` | `None` | `0x300400` |
| `0x00AA` | `170` | `None` | `0x300400` |
| `0x00AD` | `173` | `0x30` | `0x300400` |
| `0x00AE` | `174` | `0xd0` | `0x300400` |
| `0x00B0` | `176` | `None` | `0x300400` |
| `0x00B3` | `179` | `None` | `0x300400` |
| `0x00B4` | `180` | `0x38` | `0x300400` |
| `0x00B5` | `181` | `0x58` | `0x300400` |
| `0x00B7` | `183` | `0x38` | `0x300400` |
| `0x00BD` | `189` | `None` | `0x300400` |
| `0x00BE` | `190` | `0xe8` | `None` |
| `0x00BF` | `191` | `0x90` | `0x300400` |
| `0x00C0` | `192` | `0x58` | `0x300400` |
| `0x00C1` | `193` | `0x60` | `0x300400` |
| `0x00C7` | `199` | `None` | `0x300400` |
| `0x00C9` | `201` | `0x48` | `0x300400` |
| `0x00CA` | `202` | `0x48` | `0x300400` |
| `0x00CE` | `206` | `0xb0` | `0x300400` |
| `0x00CF` | `207` | `None` | `0x300400` |
| `0x00D3` | `211` | `0x38` | `0x300400` |
| `0x00D4` | `212` | `0x38` | `0x300400` |
| `0x00DC` | `220` | `0xa0` | `None` |
| `0x00DD` | `221` | `0x40` | `0x300400` |
| `0x00E2` | `226` | `0x30` | `0x300400` |
| `0x00E3` | `227` | `0x38` | `0x300400` |
| `0x00E4` | `228` | `0x30` | `0x300400` |
| `0x00E5` | `229` | `0x38` | `0x300400` |
| `0x00E6` | `230` | `0x80` | `0x300400` |
| `0x00E7` | `231` | `None` | `0x300400` |
| `0x00ED` | `237` | `None` | `0x300400` |
| `0x00EF` | `239` | `0x38` | `0x300400` |
| `0x00F0` | `240` | `0x38` | `0x300400` |
| `0x00F1` | `241` | `0x38` | `0x300400` |
| `0x00F5` | `245` | `0x28` | `0x300400` |
| `0x00FA` | `250` | `None` | `0x300400` |
| `0x00FB` | `251` | `None` | `0x300400` |
| `0x00FC` | `252` | `None` | `0x300400` |
| `0x00FE` | `254` | `None` | `0x300400` |
| `0x0138` | `312` | `0x30` | `0x300400` |
| `0x013E` | `318` | `0x40` | `0x300400` |
| `0x0146` | `326` | `None` | `0x300400` |
| `0x0147` | `327` | `None` | `0x300400` |
| `0x0148` | `328` | `None` | `0x300400` |
| `0x0149` | `329` | `None` | `0x300400` |
| `0x014A` | `330` | `None` | `0x300400` |
| `0x0152` | `338` | `None` | `0x300400` |
| `0x0153` | `339` | `None` | `0x300400` |
| `0x0158` | `344` | `None` | `0x300400` |
| `0x015B` | `347` | `None` | `0x300400` |
| `0x015C` | `348` | `0x88` | `None` |
| `0x015D` | `349` | `0x50` | `0x300400` |
| `0x015E` | `350` | `0xa0` | `None` |
| `0x015F` | `351` | `0x48` | `None` |
| `0x0160` | `352` | `0x50` | `None` |
| `0x0161` | `353` | `0x78` | `None` |
| `0x0162` | `354` | `0x50` | `0x300400` |
| `0x0163` | `355` | `0x48` | `None` |
| `0x0164` | `356` | `0x48` | `None` |
| `0x0165` | `357` | `0x30` | `0x300400` |
| `0x0166` | `358` | `0x48` | `None` |
| `0x0167` | `359` | `0x78` | `None` |
| `0x0168` | `360` | `0x70` | `None` |
| `0x0169` | `361` | `0x58` | `None` |
| `0x016A` | `362` | `0x70` | `None` |
| `0x016B` | `363` | `0x78` | `None` |
| `0x016C` | `364` | `0x50` | `None` |
| `0x016D` | `365` | `0x70` | `None` |
| `0x016E` | `366` | `0xa0` | `None` |
| `0x016F` | `367` | `0x70` | `None` |
| `0x0170` | `368` | `0x50` | `None` |
| `0x0171` | `369` | `0x50` | `None` |
| `0x0172` | `370` | `0x40` | `0x300400` |
| `0x0173` | `371` | `0x38` | `0x300400` |
| `0x0174` | `372` | `0x48` | `None` |
| `0x0175` | `373` | `0x78` | `None` |
| `0x0176` | `374` | `0xa0` | `None` |
| `0x0177` | `375` | `0x80` | `None` |
| `0x0178` | `376` | `0x70` | `None` |
| `0x0179` | `377` | `0x88` | `0x300400` |
| `0x017A` | `378` | `0x58` | `None` |
| `0x017B` | `379` | `0x48` | `None` |
| `0x017C` | `380` | `0x70` | `None` |
| `0x017D` | `381` | `0xc0` | `None` |
| `0x017E` | `382` | `0x50` | `None` |
| `0x017F` | `383` | `0x60` | `None` |
| `0x0180` | `384` | `0x50` | `None` |
| `0x0181` | `385` | `0x40` | `0x300400` |
| `0x0182` | `386` | `0x50` | `None` |
| `0x0183` | `387` | `0x78` | `None` |
| `0x0184` | `388` | `0x48` | `None` |
| `0x0185` | `389` | `0x48` | `None` |
| `0x0186` | `390` | `0xa8` | `0x300400` |
| `0x0187` | `391` | `0x58` | `0x300400` |
| `0x0188` | `392` | `0x78` | `None` |
| `0x0189` | `393` | `0x78` | `None` |
| `0x018A` | `394` | `0x90` | `None` |
| `0x018B` | `395` | `0x50` | `0x300400` |
| `0x018C` | `396` | `0x50` | `None` |
| `0x018D` | `397` | `0x68` | `None` |
| `0x018E` | `398` | `0x70` | `None` |
| `0x018F` | `399` | `0x48` | `None` |
| `0x0190` | `400` | `0x58` | `None` |
| `0x0191` | `401` | `0x38` | `0x300400` |
| `0x0192` | `402` | `0x48` | `0x300400` |
| `0x0193` | `403` | `0x68` | `None` |
| `0x0194` | `404` | `0x90` | `None` |
| `0x0195` | `405` | `0x50` | `None` |
| `0x0196` | `406` | `0x28` | `0x300400` |
| `0x0197` | `407` | `None` | `0x300400` |
| `0x019A` | `410` | `None` | `0x300400` |
| `0x01A2` | `418` | `0x50` | `0x300400` |
| `0x30000024` | `805306404` | `None` | `0x300400` |
| `0x30000025` | `805306405` | `None` | `0x300400` |
| `0x30000026` | `805306406` | `None` | `0x300400` |
| `0x3000002A` | `805306410` | `0x50` | `0x300400` |
| `0x30000033` | `805306419` | `None` | `0x300400` |
| `0x30000034` | `805306420` | `None` | `0x300400` |
| `0x3000003D` | `805306429` | `None` | `0x300400` |
| `0x30000040` | `805306432` | `None` | `0x300400` |
| `0x30000045` | `805306437` | `None` | `0x300400` |

#### 2.5.2 Outbound-Only / Constructor-Initialized `NetBlobEvent` IDs (`BIN`) <a id="outbound-only-netblob-events"></a>

| Event ID (Hex) | Event ID (Dec) | Initial Flags (`+0x14`) | Direction |
| :--- | :--- | :--- | :--- |
| `0x0024` | `36` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0029` | `41` | `0x300400` | C -> S (Outbound Constructor) |
| `0x002C` | `44` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0044` | `68` | `0x300400` | C -> S (Outbound Constructor) |
| `0x006F` | `111` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0075` | `117` | `0x300400` | C -> S (Outbound Constructor) |
| `0x007A` | `122` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0095` | `149` | `0x300400` | C -> S (Outbound Constructor) |
| `0x00A3` | `163` | `0x300400` | C -> S (Outbound Constructor) |
| `0x00CC` | `204` | `0x300400` | C -> S (Outbound Constructor) |
| `0x00D0` | `208` | `0x300400` | C -> S (Outbound Constructor) |
| `0x00D1` | `209` | `0x300400` | C -> S (Outbound Constructor) |
| `0x00EC` | `236` | `0x300400` | C -> S (Outbound Constructor) |
| `0x013D` | `317` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0154` | `340` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0155` | `341` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0198` | `408` | `0x300400` | C -> S (Outbound Constructor) |
| `0x0199` | `409` | `0x300400` | C -> S (Outbound Constructor) |
| `0x019E` | `414` | `0x300400` | C -> S (Outbound Constructor) |
| `0x3000001B` | `805306395` | `0x300400` | C -> S (Outbound Constructor) |
| `0x3000001C` | `805306396` | `0x300400` | C -> S (Outbound Constructor) |
| `0x3000001D` | `805306397` | `0x300400` | C -> S (Outbound Constructor) |
| `0x3000001E` | `805306398` | `0x300400` | C -> S (Outbound Constructor) |
| `0x3000001F` | `805306399` | `0x300400` | C -> S (Outbound Constructor) |
| `0x30000035` | `805306421` | `0x300400` | C -> S (Outbound Constructor) |
| `0x30000036` | `805306422` | `0x300400` | C -> S (Outbound Constructor) |
| `0x30000037` | `805306423` | `0x300400` | C -> S (Outbound Constructor) |
| `0x30000038` | `805306424` | `0x300400` | C -> S (Outbound Constructor) |

---

### 2.6 Master Subsystem Summary of All 120 WLib RPC Classes (`0x01F5..0x169F`) <a id="master-rpc-class-summary"></a>

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

### 2.7 WLib Native Package Registration Architecture & Complete Census of All 458 Native Packages <a id="wlib-native-packages"></a>

### 1.1 Two-Table Registration Design (`NativePackageTable` and `[registrar + 0x08]`) <a id="s1-1-two-tables"></a>

The WLib Virtual Machine in `x64/lotroclient64.exe` bridges bytecode (`client_gamelogic.dat` record `0x56000000`) to native C++ subsystems through two distinct registration tables populated during engine and client initialization:

1. **Table A — `NativePackageDescriptor` Table (`NativePackageTable`, count `NativePackageTable`, capacity `NativePackageTable`):**
 - Populated via `RegisterNativePackageDescriptor` , which inserts a `0x70`-byte `NativePackageDescriptor` sorted by `pkg_id` (`u32` at `+0x00`) into the global array at `NativePackageTable` and looked up by binary search in `LookupNativePackageDescriptor` .
 - Used whenever WLib bytecode executes opcode `0x0E` (`new.native <u32 pkg_id>`, handler ), or when a C++ native function wraps/copies a native C++ object into a WLib heap reference ( `NewNativeByPkg` `WrapNativePtr` `CopyNativeByPkg`), or when `WLibArgFrame` (`0x008E0001` / `0x008E0006`) serializes/deserializes a native object across an RPC boundary ( -> `desc->fn_serialize` at `+0x68`).
 - Contains **`258` registered `NativePackageDescriptor` entries**: **`151` base descriptors** registered in `RegisterBaseNativePackageDescriptors` and **`107` client descriptors** registered in `RegisterClientNativePackageDescriptors` .
2. **Table B — `NativeFunction` Hash Table (`[registrar + 0x08]`, linked into `NativePackageTable`):**
 - Populated via `registrar->vtable[+0x50]` (`RegisterNativeFunc`), taking `(registrar_rcx, uint32_t fn_id_edx, NativeFuncPtr fn_ptr_r8)` where `fn_id = (pkg_id << 16) | func_num`.
 - During `WLibVM::LinkBytecode` , every opcode `0x15` (`call.native <u32 (pkg_id << 16) | func_num>`) in the loaded bytecode has its `32`-bit immediate operand replaced in-place with an index into the flat resolved pointer array `NativePackageTable` (`8` bytes per entry). At runtime, opcode `0x15` indexes `NativePackageTable`[idx]` and calls the C++ wrapper directly (`void __fastcall fn(uint32_t* stack_top_rcx, uint32_t* ret_slots_rdx)`).
 - Contains **`3,997` registered native functions** across **`449` function-bearing packages**: **`2,090` base functions** registered in `RegisterBaseNativeFunctions` (`192` packages) and **`1,907` client functions** registered in `RegisterClientNativeFunctions` (`257` packages).
 - Combined with `10` descriptor-only packages (`0x001B`, `0x0051`, `0x0052`, `0x0056`, `0x0058`, `0x005A`, `0x005C`, `0x005D`, `0x005F`, `0x01C0`), the binary defines **`458` total WLib native packages**: **`298` packages in `0x0001..0x01FF`** (`195` base in `0x0001..0x00E7` and `103` client in `0x0120..0x01FF`) and **`160` extended packages in `0x070A..0x16A4`** (`7` base and `153` client).

### 1.2 `NativePackageDescriptor` (`0x70` Bytes) and Heap Wrapper (`0x20` Bytes) Layouts <a id="s1-2-layouts"></a>

Every entry in `NativePackageTable` has the following `0x70`-byte (`112`-byte) layout:

| Offset | Size | Type | Field Name | Description |
|---|---|---|---|---|
| `+0x00` | `4` | `u32` | `pkg_id` | Package ID (`0x0004..0x16A4`, little-endian). Sort key for binary search. |
| `+0x04` | `4` | `u32` | `obj_size` | Byte size of the underlying C++ payload (`0` for static/stateless or external-pointer packages; `4..0x1044` for value/struct packages). |
| `+0x08` | `1` | `u8` | `flag_heap_alloc` | When `1` allocates the C++ object via `fn_heap_new` (`+0x10`); when `0` and `obj_size > 0`, allocates in-place at `[wrapper + 0x20]` (`flags = 0x10000000`) and calls `fn_inplace_ctor` (`+0x28`). |
| `+0x09` | `1` | `u8` | `flag_refcounted` | Indicates external ref-counted C++ object lifecycle management. |
| `+0x0A` | `1` | `u8` | `flag_handle_only` | When `1`, allocated wrapper is a pure `0x20`-byte handle (`flags = 0x50000000`) whose `+0x18` `cpp_ptr` is populated by the caller. |
| `+0x0B` | `5` | `u8[5]` | `padding` | Zero padding to `8`-byte alignment. |
| `+0x10` | `8` | `ptr` | `fn_heap_new` | Heap allocator callback (`void* (*)(void)`). |
| `+0x18` | `8` | `ptr` | `fn_heap_copy` | Heap copy-constructor callback (`void* (*)(const void*)`). |
| `+0x20` | `8` | `ptr` | `fn_heap_delete` | Heap destructor/deallocator callback (`void (*)(void*)`). |
| `+0x28` | `8` | `ptr` | `fn_inplace_ctor` | In-place placement constructor (`void* (*)(void* inplace_buf)` at `[wrapper + 0x20]`). |
| `+0x30` | `8` | `ptr` | `fn_inplace_copy` | In-place copy constructor (`void* (*)(void* dst_buf, const void* src_buf)`). |
| `+0x38` | `8` | `ptr` | `fn_inplace_dtor` | In-place destructor (`void (*)(void* inplace_buf)`), invoked when the WLib reference is garbage-collected or freed. |
| `+0x40` | `8` | `ptr` | `fn_equals` | Equality comparison callback (`bool (*)(const void*, const void*)`). |
| `+0x48` | `8` | `ptr` | `fn_hash` | Hash code callback (`uint32_t (*)(const void*)`). |
| `+0x50` | `8` | `ptr` | `fn_serialized_size` | Returns serialized byte size (`uint32_t (*)(const void*)`). |
| `+0x58` | `8` | `ptr` | `fn_visit_refs` | GC reference traversal callback (`void (*)(void*, RefVisitor*)`) for container packages (`HashTable`, `SmartArray`, `DoublyLinkedList`, `WLibArgFrame`). |
| `+0x60` | `8` | `ptr` | `fn_remap_refs` | Reference remapping callback (`void (*)(void*, RefRemapper*)`). |
| `+0x68` | `8` | `ptr` | `fn_serialize` | Archive serializer (`void (*)(void** cpp_ptr_slot, Archive* ar)`), invoked during `WLibArgFrame` (`0x008E0001` / `0x008E0006`) RPC marshaling. |

Every WLib object/native reference stored in the VM reference table (`NativePackageTable`, indexed by `24`-bit reference ID `ref_id & 0x00FFFFFF`) points to a `0x20`-byte header:

| Offset | Size | Type | Field Name | Description |
|---|---|---|---|---|
| `+0x00` | `4` | `u32` | `flags_and_refcnt` | High nibble encodes allocation kind: `0x10000000` = single allocation of `0x20 + obj_size` bytes (`+0x18` points to `+0x20`); `0x50000000` = pooled `0x20`-byte native wrapper ; `0x40000000` = WLib class instance (`+0x08` points to WLib class metadata). Low bits hold reference count / GC state. |
| `+0x04` | `4` | `u32` | `reserved` | Zero / alignment padding. |
| `+0x08` | `8` | `ptr` | `type_desc_ptr` | Pointer to `NativePackageDescriptor` (when native) or WLib class descriptor (when WLib class). |
| `+0x10` | `8` | `ptr` | `wlib_slots_ptr` | Pointer to WLib member variable slots (`NULL` / `0` for native package objects). |
| `+0x18` | `8` | `ptr` | `cpp_ptr` | Pointer to the underlying C++ object (`[wrapper + 0x20]` for in-place allocations, or external C++ pointer). Unwrapped by native functions via `*(void**)(*(uintptr_t*)(`NativePackageTable` + 8 * (ref_id & 0xFFFFFF)) + 0x18)`. |

### 1.3 Native Function Wrapper Calling Convention & Return Marshaling <a id="s1-3-calling-convention"></a>

Every registered `NativeFunction` has the x86-64 Microsoft `__fastcall` signature `void NativeWrapper(uint32_t* stack_top_rcx, uint32_t* ret_slots_rdx)`:

1. **Operand Stack Unpacking (`rcx`):** `rcx` points one `32`-bit slot past the top of the VM operand stack (`r12` in the VM loop). Arguments pushed left-to-right precede `rcx` at negative `4`-byte offsets (`[rcx - 0x04]`, `[rcx - 0x08]`, `[rcx - 0x0C]`, ...). Single-slot types (`int`, `float`, `bool`, `string` table ref, `object`/`native` ref) occupy `4` bytes (`1` slot); `64`-bit `long` (`ulong` / `EntityID`) occupies `8` bytes (`2` slots).
2. **Receiver Unwrapping (`NativePackageTable`):** Instance methods on native packages read the receiver reference ID from the first pushed slot (highest negative offset `[rcx - 4*N]`), mask `ref_id & 0x00FFFFFF`, index `NativePackageTable`, and dereference `+0x18` (`cpp_ptr`) to obtain the `this` pointer for the C++ call.
3. **Return Value Writing (`rdx`):** Void native functions leave `rdx` untouched. Value-returning functions write the return value into `[rdx + 0x00]` and the VM stack type tag into `[rdx + 0x04]`:
 - `tag = 0x02` (`int` / `bool` / `enum` `u32`): `[rdx] = eax`, `[rdx + 4] = 2`.
 - `tag = 0x01` (`float` `f32`): `[rdx] = xmm0`, `[rdx + 4] = 1`.
 - `tag = 0x04` (`long` / `EntityID` `u64`, `2` stack slots = `16` tagged bytes): `[rdx] = 0`, `[rdx + 4] = 4`, `[rdx + 8] = rax` (`64`-bit value), `[rdx + 12] = 0` (`high tag`).
 - `tag = 0x08` (`WLibString` / `StringInfo`): `[rdx] = str_ref`, `[rdx + 4] = 8`.
 - `tag = 0x10` (`Object` / `NativePackage` reference): wrapped(pkg_id)` (`NewNativeByPkg`)(pkg_id, cpp_ptr, ...)` (`WrapNativePtr`), or (pkg_id, &stack_struct)` (`CopyNativeByPkg`), then registered in `NativePackageTable`, storing `[rdx] = ref_id | 0x02000000`, `[rdx + 4] = 0x10`.

### 1.4 Client UI Notice Broadcasting (`SendNotice_*` Packages -> `gm*UI` Subscribers) <a id="s1-4-notices"></a>

When a Server -> Client (`S->C`) WLib RPC stub (`0x8XXXXXXX`) dispatches to a client subsystem implementation class (`CXxxSystem::RecvCEvt_*`), the WLib method updates client state and invokes a client-tier native package (`0x012E..0x0168`, `0x0174..0x01FF`, or `0x070A..0x16A4`). Across `543` native functions in `103` packages (`452` unique `0x10000xxx` UI Notice IDs), the native wrapper calls a C++ notice-emitter helper which:

1. Resolves the target `NoticeBroadcaster`: either the global UI notice broadcaster via `GetGlobalNoticeBroadcaster` (using `[rax + 0x40]`) or an entity-scoped broadcaster via `GetClientEntity(entity_id)` (using `[entity + 0x98]`).
2. Loads a statically initialized `32`-bit **UI Notice ID (`0x10000001..0x1000085F`)** from the `.data` section into `r8d`.
3. Packs any event parameters into a stack-allocated `UIEventNoticeData` payload struct (`r9`) and calls `NoticeBroadcaster::SendNotice(broadcaster_rcx, out_rdx, notice_id_r8d, payload_r9)` , which synchronously notifies all subscribed C++ `gm*UI` window controllers that reference the exact same `.data` `0x10000xxx` global variable (`100%` of emitted Notice IDs have their C++ subscriber/handler xrefs resolved in Section 2.3 and Section 3.2).


#### 2.7.1 Complete Table of All 298 WLib Native Packages in `0x0001..0x01FF` (`BIN`, `DAT`) <a id="packages-0001-01ff"></a>

| `pkg_id` | Dec | Tier | `obj_size` | Flags (`H/R/O`) | `fn` Cnt | `0x0E` New | `0x15` Call | Subsystem / Package Role | C++ Target Module(s) & UI Notice IDs | Top WLib Caller Classes / Methods |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x0001` | `1` | `base` | - | - | `28` | `0` | `46` | LegacyStubPkg01 | SharedGameSystems, ContainerHelpers | CCommunicationSystem, CAdmin, 0x0675#0x048C, 0x0675#0x04A6 |
| `0x0002` | `2` | `base` | `0x100` | `0/0/0` | `6` | `16` | `116` | EngineMathOrUtil02 | UIMainFramework, StringInfo/Taboo | PlayerActionHandler, MonsterActionHandler, ContextMenuSystem::HandleObjectClickFailed, ContextMenuSystem::HandleObjectClick |
| `0x0003` | `3` | `base` | `0xe8` | `0/0/0` | `9` | `42` | `299` | EngineUtil03 | SharedGameSystems, StringInfo/Taboo | PlayerActionHandler, MonsterActionHandler, CMultiUsageSystem::BuildMultiUsageContextMenuItem, 0x0736#0x0007 |
| `0x0005` | `5` | `base` | - | - | `1` | `0` | `0` | Matrix3x3OrQuat | ClientDirectory | - |
| `0x0007` | `7` | `base` | - | - | `3` | `0` | `59` | EnginePkg (CQuestSystem) | WLog/GlobalEvent | CQuestSystem, PlayerActionHandler, CCommunicationSystem#0x006D, 0x06CF#0x0001 |
| `0x0008` | `8` | `base` | - | - | `8` | `0` | `322` | CommandParser | CommandParser | CAdmin, Admin, 0x0675#0x02F1, 0x0675#0x0473 |
| `0x000A` | `10` | `base` | - | - | `2` | `0` | `3` | LandBlockOrCellId | UIMainFramework | ContextMenuSystem, Admin, ContextMenuSystem::HandleObjectClickFailed, ContextMenuSystem::HandleObjectClick |
| `0x000B` | `11` | `base` | `0x28` | `0/0/0` | `14` | `255` | `377` | HashTable<u32,WLibRef> | PropertySystem, WLibPkgTable/DB | ItemAdvancementRegistry, ItemAdvancementControl, CPlayer::InqPaperItemQuantity, Admin#0x0351 |
| `0x000C` | `12` | `base` | `0x20` | `0/0/0` | `7` | `0` | `199` | HashIterator<u32,WLibRef> | WLibPkgTable/DB | ItemAdvancementRegistry, CItemAdvancementSystem, EffectRegistry#0x0037, Skill#0x01BF |
| `0x000D` | `13` | `base` | `0x28` | `0/0/0` | `11` | `5` | `18` | HashTable<u64,WLibRef> | Core/String/Disk, PropertySystem | EffectRegistry, EffectRegistry#0x003F, EffectRegistry#0x004A |
| `0x000E` | `14` | `base` | `0x20` | `0/0/0` | `4` | `0` | `9` | HashIterator<u64,WLibRef> | - | EffectRegistry, EffectRegistry::GetEffectRecordListByCategory, EffectRegistry::RemoveEffectFromCategorization |
| `0x0011` | `17` | `base` | `0x10` | `0/0/0` | `26` | `191` | `421` | SmartArray<WLibRef> | WLibPkgTable/DB, Core/String/Disk | CQuestJournal, CTraitSystem, 0x01F9#0x0004, 0x01F9#0x0001 |
| `0x0012` | `18` | `base` | `0x28` | `0/0/0` | `11` | `88` | `154` | HashTable<u32,u32> | PropertySystem, WLibPkgTable/DB | CQuestJournal, CQuestSystem, 0x0294#0x001D, 0x02E0#0x001A |
| `0x0013` | `19` | `base` | `0x20` | `0/0/0` | `7` | `0` | `83` | HashIterator<u32,u32> | - | CTravelSystem, BarringTraitUsagePlugin, 0x02E0#0x001A, 0x0744#0x0006 |
| `0x0015` | `21` | `base` | `0xc0` | `0/0/0` | `40` | `0` | `15` | StringOrTokenUtil | Monitor/Surface, WLibPkgTable/DB | CCombatSystem, Admin, Admin#0x0364, CCombatSystem#0x004D |
| `0x0016` | `22` | `base` | - | - | `13` | `0` | `0` | Array16 | EntityDesc/Weenie, WLibVM/RefTable | - |
| `0x0017` | `23` | `base` | `0x28` | `0/0/0` | `14` | `10` | `29` | HashTable<u64,u64> | PropertySystem, WLibPkgTable/DB | ItemAdvancementRegistry, Admin, HouseKeeper#0x0177, HouseKeeper#0x000B |
| `0x0018` | `24` | `base` | `0x20` | `0/0/0` | `7` | `0` | `12` | HashIterator<u64,u64> | - | 0x0736#0x0003, 0x0736#0x0008 |
| `0x0019` | `25` | `base` | `0x20` | `0/0/0` | `49` | `189` | `479` | DoublyLinkedList<WLibRef> | WLibVM/RefTable, WLibPkgTable/DB | CQuestJournal, CCraftSystem, Effect#0x006D, Effect#0x0088 |
| `0x001A` | `26` | `base` | `0x20` | `0/0/0` | `9` | `0` | `541` | ArbitraryBitfield | WLibVM/RefTable, Core/String/Disk | CQuestJournal, CCraftSystem, Effect#0x006D, Effect#0x0088 |
| `0x001B` | `27` | `base` | `0x18` | `1/0/0` | `0` | `0` | `0` | DescriptorOnly1B | - | - |
| `0x001C` | `28` | `base` | `0x28` | `0/0/0` | `11` | `0` | `0` | HashTable<u32,u64> | Core/String/Disk, PropertySystem | - |
| `0x001D` | `29` | `base` | `0x20` | `0/0/0` | `4` | `0` | `0` | HashIterator<u32,u64> | - | - |
| `0x0020` | `32` | `base` | - | - | `3` | `0` | `0` | EngineHandle20 | VisualDesc | - |
| `0x0021` | `33` | `base` | `0x38` | `0/0/0` | `1` | `0` | `0` | EnginePkg_0021 | - | - |
| `0x0022` | `34` | `base` | `0x8` | `0/0/0` | `5` | `6` | `14` | EngineUtil22 | Monitor/Surface, WLibPkgTable/DB | Clothing, Weapon, Clothing::InqWornAppearanceInfoByRaceAndSex, Weapon::InqWornAppearanceInfoByRaceAndSex |
| `0x0023` | `35` | `base` | `0x28` | `0/0/0` | `14` | `200` | `453` | HashTable<WStr,WLibRef> | PropertySystem, WLibPkgTable/DB | CQuestJournal, EffectRegistry, 0x01F0#0x0002, 0x01F0#0x0010 |
| `0x0024` | `36` | `base` | `0x20` | `0/0/0` | `7` | `0` | `358` | HashIterator<WStr,WLibRef> | WLibPkgTable/DB | CQuestJournal, TraitRegistry, 0x01F0#0x0006, 0x0675#0x03C2 |
| `0x0025` | `37` | `base` | `0x28` | `0/0/0` | `11` | `2` | `0` | HashTable<u64,u32> | Core/String/Disk, WLibPkgTable/DB | Quest |
| `0x0026` | `38` | `base` | `0x20` | `0/0/0` | `6` | `0` | `0` | HashIterator<u64,u32> | - | - |
| `0x0027` | `39` | `base` | `0x10` | `0/0/0` | `104` | `1392` | `3115` | PropertyValue | PropertySystem, WLibPkgTable/DB | ModSystem, gmPropertyUtils, 0x01E3#0x002D, 0x01E3#0x0017 |
| `0x0028` | `40` | `base` | `0x88` | `1/0/0` | `1` | `0` | `0` | PropertyDescOrKey | PropertySystem | - |
| `0x0029` | `41` | `base` | `0x18` | `0/0/0` | `9` | `5` | `6` | PropertyHelper | CObjectMaint/MapNote, Physics/Movement | Admin, CAdmin, CPlayer::SetupClientsMonitor, ColorTextSystem::SetupColorTextMonitor |
| `0x002A` | `42` | `base` | `0x80` | `1/0/0` | `1` | `0` | `0` | PlacesTable | - | - |
| `0x002B` | `43` | `base` | `0x80` | `1/0/0` | `1` | `0` | `0` | SegmentTable | - | - |
| `0x002C` | `44` | `base` | `0x68` | `0/0/0` | `10` | `0` | `0` | AppearanceTable | EntityDesc/Weenie, Core/String/Disk | - |
| `0x002D` | `45` | `base` | `0x38` | `1/0/0` | `1` | `0` | `0` | EnginePkg_002D | Quest/CombatShared | - |
| `0x002F` | `47` | `base` | `0x88` | `0/0/0` | `8` | `0` | `1` | PhysicsComponentBridge | WLog/GlobalEvent, EntityDesc/Weenie | Admin, CAdmin, Admin#0x006E |
| `0x0030` | `48` | `base` | - | - | `30` | `0` | `387` | Weenie/WorldEntity | EntityDesc/Weenie, WLibPkgTable/DB | CItemAdvancementSystem, CUsageSystem, 0x0675#0x03B1, 0x0675#0x0465 |
| `0x0031` | `49` | `base` | `0x40` | `1/0/0` | `1` | `0` | `0` | GameComponentBridge | WLibVM/RefTable | - |
| `0x0032` | `50` | `base` | `0x10` | `0/0/0` | `24` | `0` | `24` | DamageReport | Monitor/Surface, Core/Math/Base | UIDamageControl, VitalSystem, 0x06CD#0x0010, 0x06CD#0x0016 |
| `0x0033` | `51` | `base` | - | - | `1` | `0` | `136` | DBCache/DIDResolver | Database/DBCache | PlayerActionHandler, CItemAdvancementSystem, 0x005B#0x004D, 0x005B#0x0051 |
| `0x0034` | `52` | `base` | `0x8` | `0/0/0` | `14` | `0` | `185` | DBProperties | WLibPkgTable/DB, PropertySystem | CCraftSystem, CUsageSystem, 0x0FE5#0x0001, 0x0FE5#0x0002 |
| `0x0035` | `53` | `base` | - | - | `10` | `0` | `22` | PlacementSystem | Core/Math/Base, PlacementSystem | CAdmin, CErrorSystem, 0x00C4#0x0001, 0x01F1#0x0001 |
| `0x0036` | `54` | `base` | `0x88` | `0/0/0` | `6` | `0` | `0` | WorldSpatialQuery | Quest/CombatShared, EntityDesc/Weenie | - |
| `0x0037` | `55` | `base` | - | - | `4` | `0` | `0` | EffectDesc/FX | Quest/CombatShared | - |
| `0x0038` | `56` | `base` | `0x58` | `0/0/0` | `6` | `3` | `6` | EffectInstance | Quest/CombatShared, EntityDesc/Weenie | Admin, CAdmin, Admin#0x0364, CCombatSystem#0x0049 |
| `0x0039` | `57` | `base` | `0x18` | `0/0/0` | `27` | `58` | `74` | Bitfield39 | Archive/CRT, Core/String/Disk | BillingSystem, Player, 0x01E8#0x0073, 0x01E8#0x008E |
| `0x003D` | `61` | `base` | `0x1b0` | `0/0/0` | `20` | `2` | `3` | EntityDesc | EntityDesc/Weenie, HavokPhysics | HouseKeeper, LogUtils, HouseKeeper::InitializeDecorationHookInfo, LogUtils#0x0010 |
| `0x003E` | `62` | `base` | `0xa8` | `0/0/0` | `8` | `0` | `8` | EntityDescBuilder | EntityDesc/Weenie, StringInfo/Taboo | HouseKeeper, 0x07D5#0x001A, 0x07C5#0x0006 |
| `0x003F` | `63` | `base` | `0x50` | `0/0/0` | `8` | `1` | `3` | ModelComponentBridge | WLibPkgTable/DB, WLibVM/RefTable | MultiUsageSystem, CMultiUsageSystem, MultiUsageSystem::CheckActivityRange, CUsageSystem#0x0065 |
| `0x0040` | `64` | `base` | `0x8` | `0/0/0` | `6` | `0` | `7` | EnginePkg (CCombatSystem) | Core/String/Disk, Archive/CRT | CCombatSystem, CombatSystem, 0x02CA#0x000C, CCombatSystem#0x00A4 |
| `0x0041` | `65` | `base` | `0x8` | `0/0/0` | `5` | `0` | `3` | SharedCombatUtil | Core/String/Disk, Archive/CRT | LogUtils, LogUtils#0x004F, 0x0C36#0x0004 |
| `0x0042` | `66` | `base` | - | - | `6` | `0` | `81` | EnumMapper | EnumMapper | CAdmin, Admin, EngineOptionSystem::SanityCheckOption, 0x01FD#0x0013 |
| `0x0043` | `67` | `base` | `0x80` | `1/0/0` | `1` | `1` | `1` | MasterProperty | - | 0x010A#0x0005 |
| `0x0044` | `68` | `base` | - | - | `2` | `0` | `6` | RegionDesc | Archive/CRT, Database/DBCache | CAdmin, CBigBattleSystem, 0x0675#0x005C, 0x0675#0x03C2 |
| `0x0045` | `69` | `base` | - | - | `4` | `0` | `9` | EnginePkg (CAdmin) | WLibVM/RefTable | CAdmin, TitleSystem, 0x005B#0x004C, 0x0675#0x0433 |
| `0x0046` | `70` | `base` | `0x88` | `1/0/0` | `27` | `0` | `0` | PFileNode/FileParser | Archive/CRT, WLibGlueBase | - |
| `0x0047` | `71` | `base` | `0x38` | `0/0/0` | `2` | `0` | `0` | NewParser | PlacementSystem | - |
| `0x0048` | `72` | `base` | `0x1c` | `0/0/0` | `9` | `4` | `2` | PerfTimer | Console/ScriptLog, HavokPhysics | CAdmin, ChannelingQuestEvent, 0x0675#0x04A1, LogUtils#0x0010 |
| `0x0049` | `73` | `base` | `0x14` | `0/0/0` | `5` | `1` | `5` | RealTimeClock | Monitor/Surface, Core/String/Disk | CCombatSystem, CCombatSystem::GetFXAndBehaviorInformation |
| `0x004A` | `74` | `base` | - | - | `4` | `0` | `2` | EnginePkg (CCombatSystem) | EntityDesc/Weenie, Core/String/Disk | CCombatSystem, CCombatSystem#0x0046, 0x011C#0x001C |
| `0x004B` | `75` | `base` | - | - | `4` | `0` | `2` | GameTime | GameTime | CAdmin, 0x0675#0x048D |
| `0x004C` | `76` | `base` | `0x4` | `0/0/0` | `6` | `0` | `1` | ConsoleOutput | Archive/CRT, CObjectMaint/MapNote | 0x0175#0x0015 |
| `0x0050` | `80` | `base` | `0x20` | `0/0/0` | `2` | `0` | `0` | EnginePkg_0050 | VisualDesc, EntityDesc/Weenie | - |
| `0x0051` | `81` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_0051 | - | - |
| `0x0052` | `82` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_0052 | - | - |
| `0x0055` | `85` | `base` | - | - | `10` | `0` | `57` | ScriptLog/CSVChannel | Console/ScriptLog | CAdmin, EntityNameInterface, ContextMenuSystem::HandleObjectClickFailed, ContextMenuSystem::HandleObjectClick |
| `0x0056` | `86` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_0056 | - | - |
| `0x0057` | `87` | `base` | `0x80` | `1/0/0` | `1` | `0` | `0` | EnginePkg_0057 | - | - |
| `0x0058` | `88` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_0058 | - | - |
| `0x005A` | `90` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_005A | - | - |
| `0x005C` | `92` | `base` | `0x10` | `1/1/0` | `0` | `0` | `0` | EnginePkg_005C | - | - |
| `0x005D` | `93` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_005D | - | - |
| `0x005E` | `94` | `base` | `0x88` | `1/1/0` | `10` | `0` | `4` | EnginePkg (CAdmin) | Entity/WLibEvent, PropertySystem | CAdmin, 0x0675#0x04A1 |
| `0x005F` | `95` | `base` | `0x8` | `1/1/0` | `0` | `0` | `0` | EnginePkg_005F | - | - |
| `0x0061` | `97` | `base` | `0x28` | `0/0/0` | `14` | `12` | `16` | HashTable<Str,WLibRef> | WLibPkgTable/DB, PropertySystem | CFellowshipMakerSystem, ItemAdvancementControl, 0x044F#0x0058, CFellowshipMakerSystem#0x0011 |
| `0x0062` | `98` | `base` | `0x20` | `0/0/0` | `7` | `0` | `12` | HashIterator<Str,WLibRef> | WLibPkgTable/DB | CFellowshipMakerSystem, ItemAdvancementControl, CFellowshipMakerSystem#0x0018, ItemAdvancementControl#0x0057 |
| `0x0063` | `99` | `base` | `0x28` | `0/0/0` | `11` | `0` | `0` | HashTable<u32,f32> | Core/String/Disk, WLibPkgTable/DB | - |
| `0x0064` | `100` | `base` | `0x20` | `0/0/0` | `4` | `0` | `0` | HashIterator<u32,f32> | - | - |
| `0x0067` | `103` | `base` | - | - | `6` | `0` | `6` | EnginePkg (HouseKeeper) | WLibPkgTable/DB, Cell/LandBlock | HouseKeeper, LogUtils, Agent#0x0165, 0x07D5#0x001A |
| `0x0068` | `104` | `base` | `0x10` | `0/0/0` | `22` | `15` | `95` | SmartArray<u32> | WLibPkgTable/DB, Core/String/Disk | ItemWearSystem, VendorSystem, 0x0675#0x041E, 0x0351#0x0007 |
| `0x0069` | `105` | `base` | `0x28` | `0/0/0` | `11` | `29` | `82` | HashTable<u64,f32> | WLibPkgTable/DB, PropertySystem | Admin, CPlayer, Admin#0x0334, Admin#0x0333 |
| `0x006A` | `106` | `base` | `0x20` | `0/0/0` | `7` | `0` | `14` | HashIterator<u64,f32> | - | CCommunicationSystem, ItemAdvancementRegistry, CCommunicationSystem#0x0025, 0x0454#0x0004 |
| `0x006C` | `108` | `base` | `0x30` | `1/0/0` | `4` | `0` | `0` | EnginePkg_006C | Quest/CombatShared, StringInfo/Taboo | - |
| `0x006D` | `109` | `base` | `0x28` | `0/0/0` | `14` | `0` | `0` | HashTable<WLibRef,WLibRef> | PropertySystem, WLibPkgTable/DB | - |
| `0x006E` | `110` | `base` | `0x20` | `0/0/0` | `7` | `0` | `0` | HashIterator<WLibRef,WLibRef> | - | - |
| `0x006F` | `111` | `base` | `0x20` | `0/0/0` | `49` | `16` | `51` | EnginePkg (CQuestJournal) | WLibPkgTable/DB, Archive/CRT | CQuestJournal, PetState, 0x01F0#0x0002, 0x01F0#0x0010 |
| `0x0070` | `112` | `base` | `0x20` | `0/0/0` | `9` | `0` | `38` | EnginePkg (CFellowshipMakerSystem) | WLibVM/RefTable, Core/String/Disk | CFellowshipMakerSystem, CCommunicationSystem, 0x0675#0x03E4, CCommunicationSystem#0x0023 |
| `0x0071` | `113` | `base` | `0x28` | `0/0/0` | `10` | `0` | `0` | HashTable<u32,WStr> | Core/String/Disk, WLibPkgTable/DB | - |
| `0x0072` | `114` | `base` | `0x20` | `0/0/0` | `4` | `0` | `0` | HashIterator<u32,WStr> | - | - |
| `0x0075` | `117` | `base` | `0x28` | `0/0/0` | `14` | `39` | `231` | HashTable<WStr,u32> | WLibPkgTable/DB, PropertySystem | Admin, CPlayer, 0x01F0#0x0002, 0x01F0#0x0010 |
| `0x0076` | `118` | `base` | `0x20` | `0/0/0` | `7` | `0` | `144` | HashIterator<WStr,u32> | WLibPkgTable/DB | CCommunicationSystem, ItemAdvancementRegistry, Admin#0x02C3, Admin#0x02C8 |
| `0x0077` | `119` | `base` | `0x28` | `0/0/0` | `10` | `1` | `7` | HashTable<u64,WStr> | Core/String/Disk, WLibPkgTable/DB | EffectRegistry, EffectRegistry#0x0033, EffectRegistry#0x0013 |
| `0x0078` | `120` | `base` | `0x20` | `0/0/0` | `5` | `0` | `6` | HashIterator<u64,WStr> | - | EffectRegistry, EffectRegistry#0x0033, EffectRegistry#0x0026 |
| `0x0079` | `121` | `base` | - | - | `4` | `0` | `9` | EnginePkg (gmPropertyUtils) | PropertySystem | gmPropertyUtils, AdminPropertySystem, EngineOptionSystem::SanityCheckOption, 0x01FD#0x0013 |
| `0x007A` | `122` | `base` | - | - | `37` | `0` | `201` | EnginePkg (Skill) | Archive/CRT, WLibVM/RefTable | Skill, PVPCombatControl, Effect#0x0096, Effect#0x0084 |
| `0x007B` | `123` | `base` | - | - | `3` | `0` | `0` | EnginePkg_007B | EntityDesc/Weenie | - |
| `0x007C` | `124` | `base` | - | - | `11` | `0` | `9` | EnginePkg (InvSupportSystem) | EntityDesc/Weenie, VisualDesc | InvSupportSystem, OutfitSystem, InvSupportSystem#0x005A, InvSupportSystem#0x0059 |
| `0x007E` | `126` | `base` | `0x20` | `0/0/0` | `4` | `5` | `9` | EnginePkg (Admin) | EntityDesc/Weenie | Admin, CAdmin, Admin#0x009C, Admin#0x0055 |
| `0x007F` | `127` | `base` | `0xa0` | `0/0/0` | `10` | `5` | `20` | EnginePkg (Admin) | EntityDesc/Weenie, Monitor/Surface | Admin, CAdmin, CPlayer::SetupClientsMonitor, ColorTextSystem::SetupColorTextMonitor |
| `0x0080` | `128` | `base` | - | - | `15` | `0` | `7` | EnginePkg (Admin) | EntityDesc/Weenie, WLibPkgTable/DB | Admin, CAdmin, Admin#0x0374, CPlayer::SetupClientsMonitor |
| `0x0081` | `129` | `base` | `0x30` | `0/0/0` | `7` | `0` | `3` | EnginePkg (CCombatSystem) | Model/Animation, WLibPkgTable/DB | CCombatSystem, CUsageSystem, CCombatSystem#0x0051, CUsageSystem#0x004C |
| `0x0082` | `130` | `base` | `0x58` | `0/0/0` | `3` | `0` | `0` | EnginePkg_0082 | Model/Animation | - |
| `0x0083` | `131` | `base` | `0x28` | `0/0/0` | `14` | `1` | `1` | EnginePkg (WebStoreItemDirectory) | WLibPkgTable/DB, PropertySystem | WebStoreItemDirectory, WebStoreItemDirectory#0x0024 |
| `0x0084` | `132` | `base` | `0x20` | `0/0/0` | `6` | `0` | `0` | EnginePkg_0084 | - | - |
| `0x0085` | `133` | `base` | - | - | `10` | `0` | `10` | EnginePkg (CCraftSystem) | NameFilter | CCraftSystem, CraftSystem, Admin#0x02AA, 0x00FE#0x0025 |
| `0x0086` | `134` | `base` | `0x28` | `0/0/0` | `11` | `3` | `14` | EnginePkg (ChannelingQuestEvent) | WLibPkgTable/DB, PropertySystem | ChannelingQuestEvent, 0x0F46#0x0004, ChannelingQuestEvent#0x0025 |
| `0x0087` | `135` | `base` | `0x20` | `0/0/0` | `7` | `0` | `0` | EnginePkg_0087 | - | - |
| `0x0088` | `136` | `base` | `0x28` | `0/0/0` | `14` | `0` | `0` | EnginePkg_0088 | WLibPkgTable/DB, PropertySystem | - |
| `0x0089` | `137` | `base` | `0x20` | `0/0/0` | `6` | `0` | `0` | EnginePkg_0089 | - | - |
| `0x008A` | `138` | `base` | `0x28` | `0/0/0` | `14` | `7` | `19` | QuestSystemBridge | WLibPkgTable/DB, PropertySystem | CAdmin, Admin, 0x0675#0x044A, Admin#0x02A1 |
| `0x008B` | `139` | `base` | `0x20` | `0/0/0` | `6` | `0` | `8` | EnginePkg (Admin) | - | Admin, CAdmin, Admin#0x02A1, Admin#0x034C |
| `0x008C` | `140` | `base` | `0x28` | `0/0/0` | `10` | `0` | `0` | EnginePkg_008C | WLibPkgTable/DB | - |
| `0x008D` | `141` | `base` | `0x20` | `0/0/0` | `4` | `0` | `0` | EnginePkg_008D | - | - |
| `0x008E` | `142` | `base` | `0x58` | `1/0/0` | `13` | `619` | `2377` | WLibArgFrame | WLibVM/RefTable, Archive/CRT | 0x01F5#0x0004, 0x01F5#0x0003 |
| `0x0095` | `149` | `base` | `0x90` | `1/0/0` | `5` | `114` | `450` | EnginePkg (MonsterLevelDirectory) | - | MonsterLevelDirectory, InventorySystem::MoveStuff, InventorySystem::CreateStuff |
| `0x0096` | `150` | `base` | `0x130` | `1/0/0` | `3` | `108` | `285` | EnginePkg (MonsterLevelDirectory) | Core/String/Disk, HavokPhysics | MonsterLevelDirectory, MonitorHandler, MonitorHandler::OnEnterMonitor, MonitorHandler::OnLeaveMonitor |
| `0x0098` | `152` | `base` | `0xf8` | `0/0/0` | `11` | `0` | `0` | EnginePkg_0098 | Monitor/Surface, Script/Channels | - |
| `0x0099` | `153` | `base` | - | - | `30` | `0` | `51` | NameFilterSystem | EntityDesc/Weenie, WLibVM/RefTable | AreaSystem, CAdmin, 0x0675#0x04A1, Admin#0x0364 |
| `0x009A` | `154` | `base` | `0x40` | `0/0/0` | `4` | `0` | `0` | EnginePkg_009A | PropertySystem, Places/Segments | - |
| `0x009B` | `155` | `base` | `0x60` | `0/0/0` | `14` | `1` | `5` | EnginePkg (CAdmin) | PropertySystem, Monitor/Surface | CAdmin, 0x0675#0x04A1 |
| `0x009C` | `156` | `base` | `0x68` | `0/0/0` | `4` | `0` | `0` | EnginePkg_009C | Places/Segments, WLibPkgTable/DB | - |
| `0x009D` | `157` | `base` | `0x10` | `0/0/0` | `4` | `0` | `0` | EnginePkg_009D | PropertySystem, WLibVM/RefTable | - |
| `0x009F` | `159` | `base` | `0xe0` | `0/0/0` | `11` | `4` | `19` | EnginePkg (CCombatSystem) | EntityDesc/Weenie, PropertySystem | CCombatSystem, Admin, Admin#0x0364, CCombatSystem#0x004D |
| `0x00A0` | `160` | `base` | `0x38` | `0/0/0` | `21` | `18` | `11` | EnginePkg (LogUtils) | Console/ScriptLog, Core/Math/Base | LogUtils, CBigBattleSystem, CBigBattleSystem#0x0003, LogUtils#0x000F |
| `0x00A1` | `161` | `base` | `0x18` | `0/0/0` | `24` | `0` | `44` | EnginePkg (EmoteQuestEvent) | WLibVM/RefTable, Archive/CRT | EmoteQuestEvent, IQuestEvent, Agent#0x0117, AreaSystem#0x0018 |
| `0x00A2` | `162` | `base` | `0x28` | `0/0/0` | `3` | `0` | `0` | EnginePkg_00A2 | WLog/GlobalEvent | - |
| `0x00A3` | `163` | `base` | - | - | `2` | `0` | `1` | EnginePkg (CBigBattleSystem) | WLibVM/RefTable | CBigBattleSystem, CBigBattleSystem#0x0003 |
| `0x00A4` | `164` | `base` | - | - | `8` | `0` | `6` | EnginePkg (CTravelSystem) | WLibVM/RefTable, Archive/CRT | CTravelSystem, CBigBattleSystem, 0x00FF#0x0013, CBigBattleSystem#0x0003 |
| `0x00A5` | `165` | `base` | - | - | `2` | `0` | `0` | EnginePkg_00A5 | Entity/WLibEvent, Core/String/Disk | - |
| `0x00A6` | `166` | `base` | `0x60` | `0/0/0` | `74` | `154` | `1012` | PropertyCollection | PropertySystem, WLibPkgTable/DB | CQuestSystem, SkillSystem, 0x01E3#0x0001, 0x01E3#0x0002 |
| `0x00A7` | `167` | `base` | - | - | `1` | `0` | `0` | EnginePkg_00A7 | PropertySystem | - |
| `0x00A8` | `168` | `base` | `0x78` | `1/1/0` | `3` | `0` | `1` | EnginePkg_00A8 | Quest/CombatShared | 0x010A#0x0005 |
| `0x00A9` | `169` | `base` | - | - | `69` | `0` | `319` | EnginePkg (CCombatSystem) | WLibVM/RefTable, Core/String/Disk | CCombatSystem, Admin, 0x0059#0x0041, 0x0059#0x0007 |
| `0x00AB` | `171` | `base` | - | - | `16` | `0` | `30` | EnginePkg (CAdmin) | Archive/CRT, WLibPkgTable/DB | CAdmin, TitleSystem, 0x0675#0x0463, 0x0675#0x0433 |
| `0x00AC` | `172` | `base` | `0x48` | `0/0/0` | `2` | `0` | `0` | EnginePkg_00AC | PlacementSystem, Core/String/Disk | - |
| `0x00AD` | `173` | `base` | `0x10` | `0/0/0` | `15` | `1` | `2` | EnginePkg (DistantUnitUtils) | Archive/CRT, Monitor/Surface | DistantUnitUtils, DistantUnitUtils#0x0008 |
| `0x00AE` | `174` | `base` | - | - | `6` | `0` | `16` | EnginePkg (DistantUnitUtils) | PropertySystem | DistantUnitUtils, CharacterGenSystem, Effect#0x0034, DistantUnitUtils::Postproc_BuildFormation |
| `0x00AF` | `175` | `base` | `0x20` | `0/0/0` | `10` | `4` | `2` | EnginePkg (RandomNameGenerationCategory) | WLibPkgTable/DB, WLibGlueBase | RandomNameGenerationCategory, RandomNameGenerationCategory::GetRandomName |
| `0x00B0` | `176` | `base` | `0x10` | `0/0/0` | `19` | `150` | `526` | SmartArray<u64> | WLibPkgTable/DB, Core/String/Disk | DistantUnitUtils, IAItemRegistryInfo, 0x01F1#0x0023, Admin#0x0347 |
| `0x00B1` | `177` | `base` | `0x1c` | `0/0/0` | `8` | `0` | `0` | EnginePkg_00B1 | WLibPkgTable/DB, StringInfo/Taboo | - |
| `0x00B2` | `178` | `base` | `0x120` | `0/0/0` | `7` | `0` | `0` | EnginePkg_00B2 | Quest/CombatShared, EntityDesc/Weenie | - |
| `0x00B3` | `179` | `base` | `0xb0` | `0/0/0` | `7` | `0` | `0` | EnginePkg_00B3 | Monitor/Surface, PropertySystem | - |
| `0x00B5` | `181` | `base` | `0x10` | `0/0/0` | `7` | `7` | `25` | EnginePkg (UIDamageControl) | Archive/CRT, Monitor/Surface | UIDamageControl, UIDamageControl#0x0006, UIDamageControl#0x000B |
| `0x00B6` | `182` | `base` | `0x20` | `0/0/0` | `46` | `201` | `1369` | DoublyLinkedList<WStr> | WLibVM/RefTable, WLibPkgTable/DB | CAdmin, Admin, 0x01F8#0x0005, 0x01F8#0x0030 |
| `0x00B7` | `183` | `base` | `0x20` | `0/0/0` | `9` | `0` | `919` | ArbitraryBitfieldB7 | WLibVM/RefTable, Core/String/Disk | CQuestJournal, IQuestJournal, 0x0675#0x042C, 0x0675#0x044C |
| `0x00B8` | `184` | `base` | `0xa0` | `0/0/0` | `25` | `37` | `44` | EnginePkg (CAdmin) | WLibVM/RefTable, PropertySystem | CAdmin, CItemAdvancementSystem, 0x0675#0x03F9, Admin#0x0375 |
| `0x00B9` | `185` | `base` | - | - | `13` | `0` | `57` | EntityDescData | EntityDesc/Weenie, WLibVM/RefTable | Admin, CAdmin, Admin#0x0274, Admin#0x0375 |
| `0x00BA` | `186` | `base` | `0x40` | `0/0/0` | `4` | `0` | `0` | EnginePkg_00BA | - | - |
| `0x00BB` | `187` | `base` | `0x20` | `0/0/0` | `5` | `0` | `0` | EnginePkg_00BB | - | - |
| `0x00BC` | `188` | `base` | `0x68` | `0/0/0` | `4` | `0` | `0` | EnginePkg_00BC | Places/Segments, WLibPkgTable/DB | - |
| `0x00BD` | `189` | `base` | `0x68` | `1/0/0` | `8` | `0` | `4` | EnginePkg (Admin) | Quest/CombatShared, WLibPkgTable/DB | Admin, CAdmin, CPlayer::SetupClientsMonitor, ColorTextSystem::SetupColorTextMonitor |
| `0x00BE` | `190` | `base` | - | - | `1` | `0` | `0` | EnginePkg_00BE | EntityDesc/Weenie | - |
| `0x00BF` | `191` | `base` | `0x10` | `0/0/0` | `5` | `0` | `0` | EnginePkg_00BF | CObjectMaint/MapNote, Monitor/Surface | - |
| `0x00C0` | `192` | `base` | `0x30` | `1/0/0` | `3` | `0` | `0` | EnginePkg_00C0 | CObjectMaint/MapNote | - |
| `0x00C1` | `193` | `base` | `0x48` | `1/0/0` | `7` | `0` | `0` | EnginePkg_00C1 | - | - |
| `0x00C2` | `194` | `base` | `0xb0` | `0/0/0` | `4` | `0` | `0` | EnginePkg_00C2 | PropertySystem | - |
| `0x00C3` | `195` | `base` | `0x38` | `1/0/0` | `4` | `0` | `0` | EnginePkg_00C3 | Quest/CombatShared, StringInfo/Taboo | - |
| `0x00C5` | `197` | `base` | `0x58` | `0/0/0` | `18` | `0` | `7` | EnginePkg (UIDamageControl) | Monitor/Surface, HavokPhysics | UIDamageControl, UIDamageControl#0x0001 |
| `0x00C6` | `198` | `base` | - | - | `2` | `0` | `0` | EnginePkg_00C6 | EntityDesc/Weenie | - |
| `0x00C7` | `199` | `base` | `0x58` | `0/0/0` | `40` | `1205` | `2691` | StringInfo/TabooTable | StringInfo/Taboo, Core/String/Disk | CAdmin, Admin, 0x01E8#0x0043, EngineOptionSystem::SanityCheckOption |
| `0x00C8` | `200` | `base` | - | - | `1` | `0` | `0` | EnginePkg_00C8 | StringInfo/Taboo | - |
| `0x00C9` | `201` | `base` | - | - | `1` | `0` | `0` | EnginePkg_00C9 | StringInfo/Taboo | - |
| `0x00CA` | `202` | `base` | `0x68` | `0/0/0` | `3` | `0` | `0` | EnginePkg_00CA | - | - |
| `0x00CC` | `204` | `base` | - | - | `3` | `0` | `83` | TimeConversions | Archive/CRT | QuestRecord, CAdmin, 0x0675#0x0390, Admin#0x02CE |
| `0x00CD` | `205` | `base` | - | - | `13` | `0` | `15` | RandomGenerator | Archive/CRT | CAdmin, gmTimeUtils, 0x0675#0x0390, Admin#0x02D7 |
| `0x00CE` | `206` | `base` | - | - | `1` | `0` | `1` | EnginePkg_00CE | - | 0x15E8#0x0001 |
| `0x00CF` | `207` | `base` | `0x50` | `1/0/0` | `2` | `0` | `0` | EnginePkg_00CF | Quest/CombatShared | - |
| `0x00D0` | `208` | `base` | `0x10` | `0/0/0` | `4` | `0` | `0` | EnginePkg_00D0 | WLibVM/RefTable, Archive/CRT | - |
| `0x00D2` | `210` | `base` | `0x28` | `0/0/0` | `1` | `0` | `0` | EnginePkg_00D2 | EnumMapper | - |
| `0x00D3` | `211` | `base` | `0xc` | `0/0/0` | `26` | `24` | `60` | Position | Archive/CRT, EntityDesc/Weenie | DistantUnitUtils, ScatterLayoutCalculator, 0x0675#0x04A1, CPlayer::SetupClientsMonitor |
| `0x00D4` | `212` | `base` | - | - | `4` | `0` | `0` | EnginePkg_00D4 | Archive/CRT | - |
| `0x00D5` | `213` | `base` | `0x110` | `0/0/0` | `13` | `0` | `0` | VisualDesc | VisualDesc, NameFilter | - |
| `0x00D6` | `214` | `base` | - | - | `25` | `0` | `728` | VMReflection | WLibVM/RefTable, Core/String/Disk | ModSystem, VitalSystem, 0x005B#0x004B, 0x005B#0x0050 |
| `0x00D7` | `215` | `base` | - | - | `2` | `0` | `0` | EnginePkg_00D7 | Entity/WLibEvent | - |
| `0x00D8` | `216` | `base` | `0x28` | `0/0/0` | `4` | `0` | `0` | ScriptChannelConfig | WLibVM/RefTable | - |
| `0x00D9` | `217` | `base` | - | - | `13` | `0` | `619` | WLibRPCTransmitter | Entity/WLibEvent, WLibVM/RefTable | 0x01F5#0x0004, 0x01F5#0x0003 |
| `0x00DA` | `218` | `base` | - | - | `5` | `0` | `1` | EnginePkg_00DA | Entity/WLibEvent | 0x083F#0x0002 |
| `0x00DB` | `219` | `base` | - | - | `7` | `0` | `230` | MonitorVolume | Entity/WLibEvent, Core/Math/Base | MonsterLevelDirectory, MonitorHandler, MonitorHandler::OnEnterMonitor, MonitorHandler::OnLeaveMonitor |
| `0x00DC` | `220` | `base` | - | - | `11` | `0` | `29` | SpatialMonitor | Entity/WLibEvent, WLibVM/RefTable | GuildSystem, CMountSystem, Admin#0x00A1, 0x1662#0x0003 |
| `0x00DD` | `221` | `base` | - | - | `19` | `0` | `4089` | StringConcat/Convert | WLibVM/RefTable | CAdmin, EffectRegistry, 0x01E8#0x0035, 0x01F1#0x0001 |
| `0x00DE` | `222` | `base` | - | - | `26` | `0` | `24` | WLogSystem | WLog/GlobalEvent, Archive/CRT | 0x01F1#0x0001, 0x01F1#0x0012 |
| `0x00DF` | `223` | `base` | - | - | `6` | `0` | `676` | EntityRef/WState | Entity/WLibEvent, EntityDesc/Weenie | CItemAdvancementSystem, CAdmin, 0x01F1#0x0001, 0x0675#0x0444 |
| `0x00E1` | `225` | `base` | `0x8` | `0/0/0` | `37` | `582` | `840` | WLibStringOps | Core/String/Disk, Archive/CRT | CAdmin, PhysicsSystem, 0x01F1#0x0023, 0x0675#0x005C |
| `0x00E7` | `231` | `base` | - | - | `9` | `0` | `1074` | DirectoryAndEntityLookup | ClientDirectory, EntityDesc/Weenie | PlayerActionHandler, CQuestSystem, 0x01F1#0x000B, 0x01F1#0x001E |
| `0x0120` | `288` | `cli` | `0x150` | `0/0/0` | `31` | `2` | `18` | ClientDataStruct (CCraftSystem) | ClientNoticeEmit, StringInfo/Taboo | CCraftSystem, CCraftSystem#0x0046, CCraftSystem#0x0017 |
| `0x0121` | `289` | `cli` | `0x280` | `0/0/0` | `56` | `1` | `35` | ClientDataStruct (CCraftSystem) | ClientNoticeEmit, StringInfo/Taboo | CCraftSystem, CCraftSystem#0x000F |
| `0x0122` | `290` | `cli` | `0x68` | `0/0/0` | `4` | `1` | `2` | ClientDataStruct (CCraftSystem) | Core/String/Disk, ClientNativePt1 | CCraftSystem, CCraftSystem#0x0005 |
| `0x012B` | `299` | `cli` | `0x70` | `0/0/0` | `10` | `1` | `5` | ClientDataStruct (CErrorSystem) | Render/SmartBox, ClientNativePt1 | CErrorSystem, CErrorSystem#0x0006 |
| `0x012C` | `300` | `cli` | `0xe8` | `0/0/0` | `12` | `1` | `12` | ClientDataStruct_012C | ClientGluePt2, ClientNoticeEmit | 0x044D#0x0001 |
| `0x012D` | `301` | `cli` | `0x58` | `0/0/0` | `5` | `1` | `8` | ClientDataStruct_012D | ClientNativePt1, HavokPhysics | 0x044F#0x0003, 0x0454#0x0002 |
| `0x012E` | `302` | `cli` | - | - | `2` | `0` | `3` | ClientNoticeBridge_012E | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000014A`, `0x10000019` | 0x00FC#0x0004, 0x00FC#0x0007 |
| `0x012F` | `303` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge (AdminPropertySystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x100000C5`, `0x10000141` | AdminPropertySystem, AdminPropertySystem#0x000D, AdminPropertySystem#0x000F |
| `0x0130` | `304` | `cli` | - | - | `5` | `0` | `1` | ClientNoticeBridge (AdvancementSystem) | ClientNoticeEmit; Notices: `0x10000146`, `0x10000053`, `0x10000054` (+2) | AdvancementSystem, AdvancementSystem#0x005C |
| `0x0131` | `305` | `cli` | - | - | `2` | `0` | `13` | ClientNoticeBridge (CItemAdvancementSystem) | ClientNoticeEmit; Notices: `0x10000214`, `0x10000077` | CItemAdvancementSystem, 0x06CB#0x0005, 0x06CB#0x0002 |
| `0x0133` | `307` | `cli` | - | - | `9` | `0` | `7` | ClientNoticeBridge (AuctionSystem) | ClientNativePt3, ClientNoticeEmit; Notices: `0x100000E4`, `0x10000017`, `0x1000002E` (+6) | AuctionSystem, AuctionSystem#0x001F, AuctionSystem#0x0020 |
| `0x0135` | `309` | `cli` | - | - | `41` | `0` | `45` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x10000069`, `0x1000009A`, `0x10000149` (+38) | Admin, CAdmin, Admin#0x0293, Admin#0x0277 |
| `0x0136` | `310` | `cli` | - | - | `3` | `0` | `0` | ClientNoticeBridge_0136 | ClientNoticeEmit; Notices: `0x100001B7`, `0x1000009F`, `0x100000EC` | - |
| `0x0137` | `311` | `cli` | - | - | `19` | `0` | `16` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x1000005B`, `0x10000111`, `0x100001AA` (+16) | Admin, CPlayer, Admin#0x005C, Admin#0x02E2 |
| `0x0138` | `312` | `cli` | - | - | `3` | `0` | `3` | ClientNoticeBridge_0138 | ClientNoticeEmit; Notices: `0x10000168`, `0x1000020A`, `0x1000011E` | 0x0103#0x0007, 0x0103#0x0006 |
| `0x0139` | `313` | `cli` | - | - | `2` | `0` | `4` | ClientNoticeBridge_0139 | ClientNoticeEmit; Notices: `0x10000063`, `0x10000131` | 0x0106#0x0004, 0x0106#0x0009 |
| `0x013A` | `314` | `cli` | - | - | `11` | `0` | `13` | ClientNoticeBridge (CCraftSystem) | ClientNoticeEmit; Notices: `0x10000380`, `0x100001DB`, `0x10000553` (+8) | CCraftSystem, CCraftSystem#0x0011, CCraftSystem#0x0016 |
| `0x013B` | `315` | `cli` | - | - | `1` | `0` | `1` | ClientPkg_013B | ClientNativePt3, Archive/CRT | 0x0109#0x0003 |
| `0x013C` | `316` | `cli` | - | - | `1` | `0` | `1` | ClientNoticeBridge_013C | ClientNativePt3, StringInfo/Taboo; Notices: `0x10000145` | 0x010A#0x0004 |
| `0x013D` | `317` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge (CErrorSystem) | ClientNoticeEmit; Notices: `0x10000102`, `0x10000082` | CErrorSystem, CErrorSystem::HandleClientError, CErrorSystem#0x0004 |
| `0x013E` | `318` | `cli` | - | - | `7` | `0` | `7` | ClientNoticeBridge (CExaminationSystem) | ClientNativePt3, ClientNoticeEmit; Notices: `0x100001FE`, `0x10000201`, `0x10000240` (+3) | CExaminationSystem, Admin, CPlayer::TextTag_PrepareItemLinkTooltip, CExaminationSystem#0x000B |
| `0x013F` | `319` | `cli` | - | - | `28` | `0` | `26` | ClientNoticeBridge (FellowshipSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x100002F6`, `0x1000005A`, `0x100000FE` (+25) | FellowshipSystem, 0x0123#0x0058, 0x0123#0x0056 |
| `0x0140` | `320` | `cli` | - | - | `6` | `0` | `10` | ClientNoticeBridge (CCommunicationSystem) | ClientNoticeEmit; Notices: `0x10000156`, `0x100000B7`, `0x100000C4` (+3) | CCommunicationSystem, CCommunicationSystem#0x001B, CCommunicationSystem#0x0014 |
| `0x0141` | `321` | `cli` | - | - | `21` | `0` | `23` | ClientNoticeBridge (GuildSystem) | ClientNoticeEmit, ClientNativePt1; Notices: `0x10000561`, `0x10000560`, `0x1000055F` (+17) | GuildSystem, GuildSystem#0x02EA, GuildSystem#0x005C |
| `0x0143` | `323` | `cli` | - | - | `17` | `0` | `20` | ClientNoticeBridge_0143 | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000027A`, `0x1000025C`, `0x10000261` (+14) | 0x07D5#0x0014, 0x07D5#0x0020 |
| `0x0144` | `324` | `cli` | - | - | `13` | `0` | `15` | ClientNoticeBridge (CUsageSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x10000521`, `0x100000A0`, `0x100003B1` (+10) | CUsageSystem, CMEOInventorySystem, Admin#0x0366, 0x0AF4#0x0012 |
| `0x0145` | `325` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge_0145 | ClientNoticeEmit; Notices: `0x10000162`, `0x100000AC` | 0x0171#0x0007, 0x0171#0x000C |
| `0x0146` | `326` | `cli` | - | - | `16` | `0` | `19` | ClientNoticeBridge_0146 | ClientNoticeEmit; Notices: `0x10000514`, `0x10000513`, `0x10000018` (+13) | 0x1021#0x0030, 0x1021#0x008F |
| `0x0147` | `327` | `cli` | - | - | `2` | `0` | `1` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x100001B2`, `0x10000382` | Admin, CAdmin, Admin#0x0342 |
| `0x0148` | `328` | `cli` | - | - | `10` | `0` | `9` | ClientNoticeBridge (MailSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000015F`, `0x1000012D`, `0x100000FF` (+7) | MailSystem, MailSystem#0x0003, MailSystem#0x0038 |
| `0x0149` | `329` | `cli` | - | - | `7` | `0` | `9` | ClientNoticeBridge (CAdmin) | ClientNoticeEmit, ClientNativePt3; Notices: `0x10000161`, `0x100001D8`, `0x10000203` (+4) | CAdmin, 0x0675#0x03F3, 0x0110#0x0005 |
| `0x014A` | `330` | `cli` | - | - | `2` | `0` | `1` | ClientNoticeBridge_014A | ClientNoticeEmit; Notices: `0x1000004D`, `0x10000065` | 0x0621#0x0004 |
| `0x014B` | `331` | `cli` | - | - | `4` | `0` | `5` | ClientNoticeBridge (CMEOInventorySystem) | ClientNoticeEmit; Notices: `0x100001EB`, `0x1000017E`, `0x1000014D` (+1) | CMEOInventorySystem, CMEOInventorySystem#0x000F, CMEOInventorySystem#0x0005 |
| `0x014C` | `332` | `cli` | - | - | `1` | `0` | `1` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x100000F7` | Admin, CAdmin, Admin#0x0253 |
| `0x014E` | `334` | `cli` | - | - | `1` | `0` | `1` | ClientNoticeBridge_014E | ClientNoticeEmit; Notices: `0x100001AE` | 0x0114#0x0003 |
| `0x014F` | `335` | `cli` | - | - | `3` | `0` | `3` | ClientNoticeBridge_014F | ClientNoticeEmit; Notices: `0x10000084`, `0x10000051`, `0x1000014B` | 0x0115#0x0009, 0x0115#0x000E |
| `0x0150` | `336` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge_0150 | ClientNoticeEmit; Notices: `0x1000018F`, `0x10000143` | 0x0116#0x0004, 0x0116#0x0008 |
| `0x0151` | `337` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge (PedigreeSystem) | ClientNoticeEmit; Notices: `0x1000012A`, `0x1000002D` | PedigreeSystem, 0x0119#0x0005, 0x0119#0x0015 |
| `0x0152` | `338` | `cli` | - | - | `7` | `0` | `7` | ClientNoticeBridge (NPCSystem) | ClientNoticeEmit; Notices: `0x100000D8`, `0x1000009B`, `0x10000568` (+4) | NPCSystem, NPCSystem#0x001F, NPCSystem#0x0021 |
| `0x0153` | `339` | `cli` | - | - | `4` | `0` | `4` | ClientNoticeBridge (PedigreeSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x10000078`, `0x1000018C`, `0x1000021B` (+1) | PedigreeSystem, 0x0119#0x0007, 0x0119#0x0013 |
| `0x0154` | `340` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge_0154 | ClientNoticeEmit; Notices: `0x100001E8`, `0x10000060` | 0x011A#0x0005, 0x011A#0x000B |
| `0x0155` | `341` | `cli` | - | - | `11` | `0` | `9` | ClientNoticeBridge_0155 | ClientNoticeEmit; Notices: `0x100000BF`, `0x100001B1`, `0x10000034` (+8) | 0x00FE#0x0019, 0x00FE#0x001E |
| `0x0156` | `342` | `cli` | - | - | `1` | `0` | `2` | ClientNoticeBridge (UIDamageControl) | ClientNoticeEmit; Notices: `0x100001D0` | UIDamageControl, UIDamageControl#0x0001 |
| `0x0157` | `343` | `cli` | - | - | `13` | `0` | `14` | ClientNoticeBridge (PrivateEncounterSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000013D`, `0x1000022A`, `0x100001FF` (+10) | PrivateEncounterSystem, 0x011C#0x0005, 0x011C#0x000C |
| `0x0158` | `344` | `cli` | - | - | `3` | `0` | `3` | ClientNoticeBridge_0158 | ClientNoticeEmit; Notices: `0x100001DC`, `0x1000009C`, `0x10000104` | 0x011D#0x000A, 0x011D#0x0002 |
| `0x0159` | `345` | `cli` | - | - | `20` | `0` | `26` | ClientNoticeBridge (CQuestJournal) | ClientNoticeEmit; Notices: `0x10000116`, `0x1000015C`, `0x100003E6` (+17) | CQuestJournal, CQuestSystem, Admin#0x00CC, 0x011C#0x000D |
| `0x015A` | `346` | `cli` | - | - | `2` | `0` | `3` | ClientNoticeBridge_015A | ClientNoticeEmit; Notices: `0x10000179`, `0x1000001D` | 0x011E#0x0003, 0x011E#0x0002 |
| `0x015B` | `347` | `cli` | - | - | `2` | `0` | `6` | ClientNoticeBridge (CShapeshiftSystem) | ClientNoticeEmit; Notices: `0x10000167`, `0x100004E9` | CShapeshiftSystem, ShortcutSystem, CShapeshiftSystem#0x0006, CShapeshiftSystem#0x0002 |
| `0x015C` | `348` | `cli` | - | - | `5` | `0` | `5` | ClientNoticeBridge_015C | ClientNoticeEmit; Notices: `0x10000202`, `0x1000023C`, `0x10000160` (+2) | 0x0123#0x0012, 0x0123#0x000B |
| `0x015D` | `349` | `cli` | - | - | `1` | `0` | `4` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x10000188` | Admin, CAdmin, Admin#0x0303, Admin#0x02A1 |
| `0x015E` | `350` | `cli` | - | - | `17` | `0` | `13` | ClientNoticeBridge (SkillSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000019B`, `0x100003AC`, `0x1000019A` (+13) | SkillSystem, 0x0340#0x002E, 0x0340#0x0047 |
| `0x015F` | `351` | `cli` | - | - | `1` | `0` | `1` | ClientNoticeBridge_015F | ClientNoticeEmit, ClientNativePt3; Notices: `0x100001D6` | 0x06DE#0x0001 |
| `0x0160` | `352` | `cli` | - | - | `4` | `0` | `8` | ClientNoticeBridge (CTextItemSystem) | ClientNoticeEmit; Notices: `0x100001F3`, `0x10000171`, `0x100001F4` (+1) | CTextItemSystem, TextItemActionHandler, CTextItemSystem#0x0007, CTextItemSystem::RecvCEvt_UpdateCustomContents |
| `0x0161` | `353` | `cli` | - | - | `2` | `0` | `2` | ClientNoticeBridge_0161 | ClientNoticeEmit; Notices: `0x100001E7`, `0x10000079` | 0x017D#0x000E, 0x017D#0x000B |
| `0x0162` | `354` | `cli` | - | - | `5` | `0` | `5` | ClientNoticeBridge (CTrackingSystem) | ClientNoticeEmit; Notices: `0x10000200`, `0x100000AB`, `0x10000247` (+2) | CTrackingSystem, CTrackingSystem::StartTracking, CTrackingSystem#0x0002 |
| `0x0163` | `355` | `cli` | - | - | `10` | `0` | `14` | ClientNoticeBridge_0163 | ClientNoticeEmit; Notices: `0x10000124`, `0x10000010`, `0x1000006E` (+7) | 0x017F#0x0062, 0x017F#0x006A |
| `0x0164` | `356` | `cli` | - | - | `10` | `0` | `28` | ClientNoticeBridge (CTraitSystem) | ClientNoticeEmit; Notices: `0x1000004C`, `0x100002E5`, `0x100001EF` (+7) | CTraitSystem, CTraitSystem#0x0020, CTraitSystem#0x001B |
| `0x0165` | `357` | `cli` | - | - | `5` | `0` | `5` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x1000011C`, `0x10000165`, `0x100001E6` (+2) | Admin, CAdmin, Admin#0x0354, CPlayer::RecvCEvt_ClearTravelRecord |
| `0x0166` | `358` | `cli` | - | - | `1` | `0` | `3` | ClientNoticeBridge_0166 | ClientNoticeEmit; Notices: `0x10000195` | 0x0126#0x000A, 0x0126#0x000C |
| `0x0167` | `359` | `cli` | - | - | `22` | `0` | `25` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x100001AF`, `0x1000004B`, `0x1000011B` (+19) | Admin, CAdmin, Admin#0x0299, Admin#0x02C6 |
| `0x0168` | `360` | `cli` | - | - | `2` | `0` | `1` | ClientNoticeBridge (VendorSystem) | ClientNoticeEmit; Notices: `0x100000C1`, `0x1000000E` | VendorSystem, VendorSystem#0x004C |
| `0x0169` | `361` | `cli` | `0x138` | `0/0/0` | `15` | `1` | `15` | ClientPkg_0169 | ClientNativePt1, CObjectMaint/MapNote | 0x048C#0x0059 |
| `0x016A` | `362` | `cli` | `0x108` | `0/0/0` | `16` | `1` | `16` | ClientPkg_016A | CObjectMaint/MapNote, ClientNativePt1 | 0x048F#0x0029 |
| `0x016B` | `363` | `cli` | `0x68` | `0/0/0` | `4` | `1` | `4` | ClientPkg_016B | Monitor/Surface, ClientNativePt1 | 0x0490#0x0002 |
| `0x017B` | `379` | `cli` | `0x30` | `0/0/0` | `20` | `8` | `41` | ClientBridge (AdvancementSystem) | Monitor/Surface, Core/Math/Base | AdvancementSystem, AdvancementSystem::GetUntrainedBuyableSkills, AdvancementSystem#0x0094 |
| `0x0181` | `385` | `cli` | - | - | `15` | `0` | `50` | ClientBridge (CCommunicationSystem) | ClientGameplayUI | CCommunicationSystem, CQuestJournal, 0x0675#0x0513, Admin#0x02FD |
| `0x0182` | `386` | `cli` | `0x28` | `0/0/0` | `14` | `3` | `11` | ClientBridge (SkillSystem) | ClientNativePt1, Monitor/Surface | SkillSystem, 0x0C96#0x0014, SkillSystem::Client_CanUseSkillBasedOnTargetVitals |
| `0x0184` | `388` | `cli` | `0xc0` | `0/0/0` | `2` | `1` | `2` | ClientPkg_0184 | ClientNativePt1, StringInfo/Taboo | 0x00FC#0x000F |
| `0x0188` | `392` | `cli` | `0x18` | `0/0/0` | `4` | `6` | `22` | ClientBridge (Clothing) | Core/String/Disk, VisualDesc | Clothing, Weapon, Clothing::InqWornAppearanceInfoByRaceAndSex, Weapon::InqWornAppearanceInfoByRaceAndSex |
| `0x0189` | `393` | `cli` | `0x78` | `0/0/0` | `41` | `0` | `32` | ClientBridge (AuctionSystem) | ClientNativePt1, PropertySystem | AuctionSystem, ItemAdvancementRegistry, AuctionSystem::AddAuction, AuctionSystem#0x000D |
| `0x018B` | `395` | `cli` | `0x18` | `0/0/0` | `2` | `0` | `2` | ClientBridge (CharacterGenSystem) | ClientNativePt1, VisualDesc | CharacterGenSystem, CharacterGenSystem#0x004F |
| `0x018C` | `396` | `cli` | `0x8` | `0/0/0` | `4` | `1` | `2` | ClientPkg_018C | Monitor/Surface, HavokPhysics | 0x03CC#0x0001 |
| `0x018E` | `398` | `cli` | `0xd8` | `1/0/0` | `5` | `0` | `0` | ClientPkg_018E | ClientNoticeEmit, ClientNativePt1 | - |
| `0x018F` | `399` | `cli` | - | - | `4` | `0` | `0` | ClientPkg_018F | - | - |
| `0x0191` | `401` | `cli` | `0x10` | `0/0/0` | `4` | `0` | `0` | ClientPkg_0191 | ClientNativePt1, Monitor/Surface | - |
| `0x0192` | `402` | `cli` | `0x18` | `0/0/0` | `2` | `0` | `2` | ClientPkg_0192 | ClientNativePt1, ClientNoticeEmit | 0x0103#0x0004 |
| `0x0193` | `403` | `cli` | `0x28` | `0/0/0` | `3` | `3` | `8` | ClientPkg_0193 | ClientNativePt1, Database/DBCache | 0x0108#0x0002, 0x0108#0x0003 |
| `0x0195` | `405` | `cli` | `0x30` | `0/0/0` | `14` | `4` | `20` | ClientBridge (UIDamageControl) | ClientNoticeEmit, ClientNativePt1 | UIDamageControl, UIDamageControl#0x0006, UIDamageControl#0x000B |
| `0x0196` | `406` | `cli` | `0x70` | `0/0/0` | `11` | `0` | `0` | ClientPkg_0196 | ClientNativePt1, ClientNoticeEmit | - |
| `0x0197` | `407` | `cli` | `0x8` | `0/0/0` | `4` | `0` | `4` | ClientBridge (CAdmin) | ClientNoticeEmit | CAdmin, Admin, 0x0675#0x03F3, Admin#0x0208 |
| `0x0198` | `408` | `cli` | `0x48` | `0/0/0` | `24` | `1` | `16` | ClientBridge (CEffectSystem) | ClientNoticeEmit, ClientNativePt1 | CEffectSystem, CEffectSystem#0x0005 |
| `0x019B` | `411` | `cli` | `0x18` | `0/0/0` | `12` | `0` | `0` | ClientPkg_019B | ClientNativePt1, WLibPkgTable/DB | - |
| `0x019E` | `414` | `cli` | `0x60` | `0/0/0` | `23` | `0` | `39` | ClientBridge (CCommunicationSystem) | ClientNativePt1, StringInfo/Taboo | CCommunicationSystem, Admin, Admin#0x02C3, CCommunicationSystem#0x001B |
| `0x019F` | `415` | `cli` | `0xb0` | `0/0/0` | `31` | `1` | `63` | ClientBridge (Admin) | ClientNativePt1, ClientNoticeEmit | Admin, CAdmin, Admin#0x00D4, Admin#0x0303 |
| `0x01A0` | `416` | `cli` | - | - | `4` | `0` | `8` | ClientBridge (VitalThresholdEffect) | ClientNativePt1, ClientNoticeEmit | VitalThresholdEffect, SkillProcEffect, Effect#0x004C, 0x02CF#0x001F |
| `0x01A1` | `417` | `cli` | `0x170` | `1/0/0` | `11` | `0` | `0` | ClientPkg_01A1 | ClientNoticeEmit, ClientNativePt1 | - |
| `0x01A4` | `420` | `cli` | `0xe0` | `1/0/0` | `4` | `0` | `0` | ClientPkg_01A4 | ClientNativePt1, ClientNoticeEmit | - |
| `0x01A5` | `421` | `cli` | `0x70` | `1/0/0` | `1` | `4` | `0` | ClientBridge (Admin) | - | Admin, CAdmin |
| `0x01A6` | `422` | `cli` | `0xa0` | `1/0/0` | `32` | `0` | `28` | ClientBridge (CharacterGenSystem) | ClientNativePt1, ClientNoticeEmit | CharacterGenSystem, CharacterGenSystem#0x000D, CharacterGenSystem#0x0022 |
| `0x01A7` | `423` | `cli` | `0x50` | `0/0/0` | `8` | `9` | `14` | ClientBridge (CCombatSystem) | ClientGluePt1, WLibVM/RefTable | CCombatSystem, EmoteSystem, CCombatSystem#0x0053, EmoteSystem#0x0023 |
| `0x01A8` | `424` | `cli` | `0x80` | `0/0/0` | `17` | `0` | `0` | ClientPkg_01A8 | ClientNoticeEmit, StringInfo/Taboo | - |
| `0x01A9` | `425` | `cli` | - | - | `4` | `0` | `2` | ClientBridge (ForwardedPropertySystem) | ClientNoticeEmit | ForwardedPropertySystem, ForwardedPropertySystem::AfterEntityPostproc, 0x0571#0x0004 |
| `0x01AA` | `426` | `cli` | - | - | `6` | `0` | `116` | ClientBridge (GuildSystem) | ClientNoticeEmit | GuildSystem, PlayerActionHandler, Admin#0x0297, 0x0301#0x0011 |
| `0x01AB` | `427` | `cli` | `0x80` | `0/0/0` | `12` | `1` | `0` | ClientPkg_01AB | ClientNoticeEmit, StringInfo/Taboo | - |
| `0x01AC` | `428` | `cli` | `0x20` | `0/0/0` | `4` | `0` | `0` | ClientPkg_01AC | ClientNativePt1, PropertySystem | - |
| `0x01B0` | `432` | `cli` | `0x10` | `0/0/0` | `10` | `3` | `45` | ClientBridge (CExaminationSystem) | ClientNativePt1, ClientNoticeEmit | CExaminationSystem, ExaminationSkillDataGrabber, CExaminationSystem#0x0001, CExaminationSystem#0x0008 |
| `0x01B1` | `433` | `cli` | `0x60` | `0/0/0` | `11` | `0` | `17` | ClientBridge (CCommunicationSystem) | ClientNativePt1, StringInfo/Taboo | CCommunicationSystem, Admin, Admin#0x02C8, CCommunicationSystem#0x001B |
| `0x01B5` | `437` | `cli` | `0x78` | `0/0/0` | `51` | `0` | `14` | ClientBridge (MailSystem) | ClientNoticeEmit, StringInfo/Taboo | MailSystem, ItemAdvancementRegistry, ItemAdvancementRegistry#0x005B, ItemAdvancementRegistry#0x005E |
| `0x01B7` | `439` | `cli` | `0xd0` | `0/0/0` | `17` | `0` | `4` | ClientBridge (PrivateEncounterSystem) | ClientNoticeEmit, HavokPhysics | PrivateEncounterSystem, 0x011C#0x0008, 0x011C#0x000A |
| `0x01B9` | `441` | `cli` | `0xd0` | `0/0/0` | `9` | `1` | `3` | ClientBridge (MonsterPlaySystem) | ClientNativePt1, StringInfo/Taboo | MonsterPlaySystem, 0x0113#0x0006 |
| `0x01BC` | `444` | `cli` | `0x18` | `0/0/0` | `6` | `1` | `3` | ClientPkg_01BC | Core/String/Disk, Monitor/Surface | 0x011A#0x000D |
| `0x01C0` | `448` | `cli` | `0x158` | `0/0/0` | `0` | `0` | `0` | ClientPkg_01C0 | - | - |
| `0x01C3` | `451` | `cli` | - | - | `1` | `0` | `3` | ClientBridge (ChannelingQuestEvent) | ClientNoticeEmit | ChannelingQuestEvent, EmoteQuestEvent, ChannelingQuestEvent#0x0065 |
| `0x01C5` | `453` | `cli` | `0x38` | `0/0/0` | `13` | `0` | `0` | ClientPkg_01C5 | ClientNativePt1, ClientNoticeEmit | - |
| `0x01C6` | `454` | `cli` | `0x20` | `0/0/0` | `12` | `12` | `43` | ClientBridge (ShortcutSystem) | PropertySystem, ClientNoticeEmit | ShortcutSystem, Admin, Admin#0x02A1, Admin#0x02A4 |
| `0x01C7` | `455` | `cli` | `0xe8` | `0/0/0` | `4` | `1` | `3` | ClientPkg_01C7 | ClientNoticeEmit, StringInfo/Taboo | 0x0337#0x0007, 0x033E#0x0007 |
| `0x01C9` | `457` | `cli` | `0x228` | `0/0/0` | `1` | `0` | `0` | ClientPkg_01C9 | - | - |
| `0x01CA` | `458` | `cli` | `0x210` | `0/0/0` | `31` | `1` | `31` | ClientBridge (CTraitSystem) | ClientNativePt1, ClientNoticeEmit | CTraitSystem, TraitSystem, CTraitSystem#0x0015, TraitSystem::GetTraitUIDataForTree |
| `0x01CB` | `459` | `cli` | `0x14` | `0/0/0` | `6` | `2` | `26` | ClientBridge (CTraitSystem) | ClientNoticeEmit, Core/Math/Base | CTraitSystem, CMountedCombatSystem, CMountedCombatSystem#0x0012, CTraitSystem#0x001B |
| `0x01CF` | `463` | `cli` | `0xd8` | `0/0/0` | `22` | `1` | `24` | ClientPkg_01CF | ClientNativePt1, ClientNoticeEmit | 0x1021#0x0079, 0x1021#0x0063 |
| `0x01D0` | `464` | `cli` | `0x80` | `1/0/0` | `1` | `0` | `0` | ClientPkg_01D0 | - | - |
| `0x01D1` | `465` | `cli` | `0x24` | `0/0/0` | `10` | `0` | `0` | ClientPkg_01D1 | ClientNoticeEmit, WLibPkgTable/DB | - |

#### 2.7.2 Complete Table of All 160 Extended WLib Native Packages in `0x070A..0x16A4` (`BIN`, `DAT`) <a id="packages-gt-01ff"></a>

| `pkg_id` | Dec | Tier | `obj_size` | `fn` Cnt | `0x0E` New | `0x15` Call | Subsystem / Package Role | UI Notice IDs & C++ Modules | Top WLib Caller Classes / Methods |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x070A` | `1802` | `cli` | `0x80` | `2` | `1` | `1` | ClientPkg_070A | ClientNativePt1 | 0x0454#0x0002 |
| `0x0730` | `1840` | `base` | - | `1` | `0` | `1` | EnginePkg (CAdmin) | - | CAdmin, 0x0675#0x048A |
| `0x0742` | `1858` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0742 | ClientNativePt3, ClientNoticeEmit; Notices: `0x10000232` | 0x071D#0x0007 |
| `0x0746` | `1862` | `cli` | - | `3` | `0` | `3` | ClientNoticeBridge_0746 | ClientNoticeEmit; Notices: `0x10000234`, `0x10000233`, `0x10000235` | 0x0745#0x0003, 0x0745#0x0002 |
| `0x074A` | `1866` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_074A | ClientNoticeEmit; Notices: `0x10000239` | 0x0123#0x001E |
| `0x075C` | `1884` | `cli` | - | `1` | `0` | `1` | ClientBridge (CCommunicationSystem) | ClientNoticeEmit | CCommunicationSystem, CCommunicationSystem#0x0055 |
| `0x0788` | `1928` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0788 | ClientNoticeEmit; Notices: `0x1000024E` | 0x0787#0x0003 |
| `0x0789` | `1929` | `cli` | - | `2` | `0` | `2` | ClientNoticeBridge_0789 | ClientNoticeEmit; Notices: `0x1000024D`, `0x1000024F` | 0x0786#0x0011, 0x0786#0x000C |
| `0x078F` | `1935` | `cli` | `0x128` | `16` | `1` | `8` | ClientPkg_078F | ClientNativePt1, StringInfo/Taboo | 0x0787#0x0002 |
| `0x0790` | `1936` | `cli` | `0xc0` | `8` | `1` | `6` | ClientPkg_0790 | Script/Channels, ClientNativePt1 | 0x0761#0x000A |
| `0x0791` | `1937` | `cli` | - | `2` | `0` | `1` | ClientNoticeBridge_0791 | ClientNoticeEmit; Notices: `0x10000250`, `0x10000251` | 0x0795#0x0001 |
| `0x0796` | `1942` | `cli` | `0x40` | `29` | `0` | `9` | ClientPkg_0796 | ClientNativePt1, ClientNoticeEmit | 0x077C#0x0010, 0x0795#0x0002 |
| `0x0798` | `1944` | `cli` | - | `4` | `0` | `4` | ClientNoticeBridge_0798 | ClientNoticeEmit; Notices: `0x10000252`, `0x100002C1`, `0x100002C2` (+1) | 0x0769#0x000D, 0x0769#0x0016 |
| `0x07A8` | `1960` | `cli` | `0x68` | `4` | `0` | `0` | ClientPkg_07A8 | Core/String/Disk, ClientNativePt1 | - |
| `0x07A9` | `1961` | `cli` | `0x18` | `5` | `0` | `3` | ClientPkg_07A9 | ClientNativePt1, StringInfo/Taboo | 0x07A6#0x0041 |
| `0x07AF` | `1967` | `cli` | `0x78` | `10` | `0` | `0` | ClientPkg_07AF | Core/String/Disk, ClientNativePt1 | - |
| `0x07B0` | `1968` | `cli` | `0x90` | `6` | `0` | `0` | ClientPkg_07B0 | ClientNativePt1, Core/String/Disk | - |
| `0x07B1` | `1969` | `cli` | `0x70` | `7` | `0` | `3` | ClientPkg_07B1 | ClientNativePt1, StringInfo/Taboo | 0x07A6#0x004B |
| `0x07B3` | `1971` | `base` | `0x18` | `4` | `0` | `0` | EnginePkg_07B3 | WLibPkgTable/DB, VisualDesc | - |
| `0x07B4` | `1972` | `base` | `0x18` | `4` | `0` | `0` | EnginePkg_07B4 | VisualDesc | - |
| `0x07B6` | `1974` | `base` | `0x18` | `4` | `2` | `10` | EnginePkg (GameSetSystem) | WLibPkgTable/DB, VisualDesc | GameSetSystem, DistantUnitUtils, DistantUnitUtils::Postproc_HarvestFormationData, GameSetSystem::GetPredictedAverageMemberLevel |
| `0x07B7` | `1975` | `base` | `0x18` | `4` | `1` | `3` | EnginePkg (ItemAdvancementRegistry) | VisualDesc | ItemAdvancementRegistry, ItemAdvancementRegistry#0x000E |
| `0x07BC` | `1980` | `cli` | `0x80` | `15` | `1` | `7` | ClientPkg_07BC | ClientNativePt1, StringInfo/Taboo | 0x07A1#0x0080 |
| `0x07BD` | `1981` | `cli` | `0xa8` | `4` | `0` | `0` | ClientPkg_07BD | ClientNativePt1, SharedGameSystems | - |
| `0x07C6` | `1990` | `cli` | `0x38` | `17` | `1` | `10` | ClientPkg_07C6 | ClientNativePt1, Core/Math/Base | 0x07A1#0x003D |
| `0x07D4` | `2004` | `base` | - | `2` | `0` | `0` | EnginePkg_07D4 | PlacementSystem | - |
| `0x07DE` | `2014` | `cli` | - | `9` | `0` | `18` | ClientPkg_07DE | Music/CLCache, ClientNativePt3 | 0x0116#0x0021, 0x0116#0x0020 |
| `0x07EC` | `2028` | `cli` | - | `3` | `0` | `3` | ClientNoticeBridge (CCombatSystem) | ClientNoticeEmit; Notices: `0x10000269`, `0x100003BF`, `0x100002C9` | CCombatSystem, CCombatSystem#0x0044, CCombatSystem#0x0017 |
| `0x07EF` | `2031` | `cli` | `0x18` | `3` | `0` | `0` | ClientPkg_07EF | ClientNativePt1, Core/Math/Base | - |
| `0x07F0` | `2032` | `cli` | `0x30` | `8` | `0` | `0` | ClientPkg_07F0 | Monitor/Surface, ClientNativePt1 | - |
| `0x0801` | `2049` | `cli` | `0x18` | `4` | `0` | `0` | ClientPkg_0801 | ClientNativePt1, Core/Math/Base | - |
| `0x0802` | `2050` | `cli` | `0x18` | `6` | `0` | `0` | ClientPkg_0802 | Core/Math/Base, Monitor/Surface | - |
| `0x0803` | `2051` | `cli` | `0x70` | `10` | `0` | `6` | ClientPkg_0803 | ClientNativePt1, StringInfo/Taboo | 0x07A6#0x004C |
| `0x0804` | `2052` | `cli` | `0x20` | `3` | `0` | `3` | ClientPkg_0804 | WLibVM/RefTable, ClientNativePt1 | 0x0116#0x0027 |
| `0x0822` | `2082` | `cli` | - | `2` | `0` | `4` | ClientNoticeBridge (OutfitSystem) | ClientNoticeEmit; Notices: `0x1000027D`, `0x10000299` | OutfitSystem, OutfitSystem::SelectOutfit, OutfitSystem#0x0053 |
| `0x0838` | `2104` | `cli` | - | `3` | `0` | `2` | ClientNoticeBridge_0838 | ClientNoticeEmit; Notices: `0x1000029B`, `0x10000280`, `0x1000027F` | 0x0837#0x0008, 0x0837#0x000F |
| `0x083B` | `2107` | `cli` | - | `4` | `0` | `1` | ClientPkg_083B | ClientNoticeEmit, PropertySystem | 0x0836#0x0005 |
| `0x0860` | `2144` | `base` | - | `5` | `0` | `4` | EnginePkg (CAdmin) | - | CAdmin, 0x0675#0x04B0 |
| `0x0866` | `2150` | `cli` | - | `29` | `0` | `39` | ClientNoticeBridge (CItemAdvancementSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x100002F0`, `0x100003DC`, `0x10000286` (+26) | CItemAdvancementSystem, ItemAdvancementRegistry, CItemAdvancementSystem#0x0017, CItemAdvancementSystem#0x000D |
| `0x0870` | `2160` | `cli` | `0x2c` | `18` | `6` | `29` | ClientBridge (CItemAdvancementSystem) | Monitor/Surface, Core/Math/Base | CItemAdvancementSystem, ItemAdvancementSystem, CItemAdvancementSystem#0x0016, CItemAdvancementSystem#0x0092 |
| `0x0872` | `2162` | `cli` | - | `5` | `0` | `4` | ClientNoticeBridge_0872 | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000028E`, `0x10000290`, `0x1000029C` (+2) | 0x084E#0x0006, 0x084E#0x0013 |
| `0x0875` | `2165` | `base` | `0x20` | `2` | `0` | `0` | EnginePkg_0875 | WLibVM/RefTable | - |
| `0x0876` | `2166` | `base` | `0xd0` | `4` | `0` | `0` | EnginePkg_0876 | Archive/CRT, Quest/CombatShared | - |
| `0x08A0` | `2208` | `base` | `0x50` | `1` | `0` | `0` | EnginePkg_08A0 | Quest/CombatShared | - |
| `0x08BE` | `2238` | `base` | `0x18` | `4` | `0` | `0` | EnginePkg_08BE | WLibPkgTable/DB, VisualDesc | - |
| `0x08CA` | `2250` | `cli` | - | `2` | `0` | `1` | ClientNoticeBridge_08CA | ClientNoticeEmit; Notices: `0x100002B2`, `0x100002B1` | 0x08CC#0x0001 |
| `0x08DF` | `2271` | `cli` | - | `2` | `0` | `2` | ClientNoticeBridge_08DF | ClientNoticeEmit; Notices: `0x100002B7`, `0x100002B6` | 0x08E0#0x0003 |
| `0x08E8` | `2280` | `cli` | - | `2` | `0` | `3` | ClientNoticeBridge_08E8 | ClientNoticeEmit; Notices: `0x100002DA`, `0x100002C5` | 0x0729#0x0003, 0x0729#0x0004 |
| `0x08EB` | `2283` | `cli` | `0x68` | `4` | `1` | `2` | ClientBridge (CCraftSystem) | Core/String/Disk, ClientNativePt1 | CCraftSystem, CCraftSystem#0x001D |
| `0x095D` | `2397` | `cli` | `0x118` | `32` | `2` | `68` | ClientBridge (ItemAdvancementRegistry) | ClientNativePt1, ClientNoticeEmit | ItemAdvancementRegistry, CItemAdvancementSystem, 0x0422#0x0006, 0x0423#0x0005 |
| `0x0980` | `2432` | `base` | `0x20` | `10` | `0` | `0` | EnginePkg_0980 | WLibPkgTable/DB, HavokPhysics | - |
| `0x0982` | `2434` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge (CUsageSystem) | ClientNoticeEmit; Notices: `0x100002F4` | CUsageSystem, CUsageSystem#0x0032 |
| `0x09AC` | `2476` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge (CMountSystem) | ClientNoticeEmit; Notices: `0x10000306` | CMountSystem, CMountSystem#0x000C |
| `0x09AE` | `2478` | `cli` | `0x40` | `2` | `0` | `0` | ClientPkg_09AE | ClientNoticeEmit, PropertySystem | - |
| `0x09AF` | `2479` | `cli` | `0x1a0` | `14` | `1` | `4` | ClientBridge (QuestEventTargetLocationControl) | ClientNativePt1, ClientNoticeEmit | QuestEventTargetLocationControl, QuestEventTargetLocationControl#0x0019, QuestEventTargetLocationControl#0x000C |
| `0x09B6` | `2486` | `base` | - | `1` | `0` | `1` | EnginePkg_09B6 | - | 0x0AE7#0x0004 |
| `0x09C0` | `2496` | `base` | - | `1` | `0` | `0` | EnginePkg_09C0 | WLibVM/RefTable, Core/Math/Base | - |
| `0x09CA` | `2506` | `base` | `0x8` | `5` | `0` | `2` | EnginePkg (WebStoreSystem) | Core/String/Disk, Archive/CRT | WebStoreSystem, CWebStoreSystem, 0x01F1#0x0023, WebStoreSystem#0x0051 |
| `0x0A06` | `2566` | `cli` | - | `2` | `0` | `0` | ClientPkg_0A06 | Core/Math/Base | - |
| `0x0A07` | `2567` | `cli` | `0x80` | `15` | `0` | `6` | ClientBridge (ItemAdvancementRegistry) | ClientNativePt1, HavokPhysics | ItemAdvancementRegistry, 0x0A02#0x001C, 0x0A02#0x001E |
| `0x0A0A` | `2570` | `cli` | - | `5` | `0` | `7` | ClientNoticeBridge_0A0A | ClientNativePt3, ClientNoticeEmit; Notices: `0x100003B9`, `0x100003C6`, `0x100003BC` (+2) | 0x0A02#0x0001, 0x0A02#0x001F |
| `0x0A0E` | `2574` | `cli` | - | `5` | `0` | `13` | ClientBridge (SkillProcEffect) | ClientNoticeEmit, WLibVM/RefTable | SkillProcEffect, FireSiegeWeaponScriptCallback, 0x0F02#0x000C, 0x0F02#0x000A |
| `0x0A1A` | `2586` | `cli` | - | `5` | `0` | `7` | ClientNoticeBridge (CWebStoreSystem) | ClientNoticeEmit; Notices: `0x1000040F`, `0x10000390`, `0x10000500` (+2) | CWebStoreSystem, CWebStoreSystem#0x0053, CWebStoreSystem#0x0004 |
| `0x0A2D` | `2605` | `cli` | - | `4` | `0` | `5` | ClientNoticeBridge_0A2D | ClientNoticeEmit, ClientNativePt3; Notices: `0x10000392`, `0x100003B7`, `0x100003B0` | 0x0A2B#0x0012, 0x0A2B#0x0025 |
| `0x0A39` | `2617` | `cli` | `0xe0` | `16` | `1` | `7` | ClientPkg_0A39 | ClientNoticeEmit, ClientNativePt1 | 0x0A2A#0x0013, 0x0A2A#0x0010 |
| `0x0A44` | `2628` | `cli` | `0xa8` | `25` | `2` | `34` | ClientBridge (PrivateEncounterSystem) | ClientNoticeEmit, ClientNativePt1 | PrivateEncounterSystem, SkirmishEncounterTemplate, 0x0123#0x0091, 0x044F#0x0037 |
| `0x0A46` | `2630` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0A46 | ClientNoticeEmit; Notices: `0x10000396` | 0x0A43#0x0002 |
| `0x0A51` | `2641` | `cli` | - | `3` | `0` | `4` | ClientNoticeBridge (CSkirmishSystem) | ClientNoticeEmit, ClientNativePt3; Notices: `0x1000039D`, `0x100003AE`, `0x1000039C` | CSkirmishSystem, CSkirmishSystem#0x0002, CSkirmishSystem::RecvCEvt_UpdateSkirmishStat |
| `0x0A64` | `2660` | `base` | - | `6` | `0` | `7` | EnginePkg (Admin) | SharedGameSystems | Admin, CAdmin, Admin#0x034A |
| `0x0A9B` | `2715` | `cli` | `0x78` | `8` | `0` | `0` | ClientPkg_0A9B | ClientNoticeEmit | - |
| `0x0AB9` | `2745` | `cli` | `0x88` | `9` | `0` | `0` | ClientPkg_0AB9 | ClientNoticeEmit, Monitor/Surface | - |
| `0x0AC8` | `2760` | `cli` | `0x50` | `6` | `2` | `8` | ClientBridge (ColorPackSystem) | ClientNoticeEmit | ColorPackSystem, CMountedCombatSystem, ColorPackSystem#0x0008, CMountedCombatSystem#0x001E |
| `0x0AE4` | `2788` | `base` | `0x20` | `2` | `0` | `0` | EnginePkg_0AE4 | WLibVM/RefTable | - |
| `0x0AFB` | `2811` | `base` | - | `1` | `0` | `0` | EnginePkg_0AFB | Places/Segments | - |
| `0x0BED` | `3053` | `cli` | `0x48` | `3` | `1` | `1` | ClientBridge (RandomNameGenerationDirectory) | ClientNativePt1, WLibVM/RefTable | RandomNameGenerationDirectory, RandomNameGenerationOption, RandomNameGenerationOption#0x0005 |
| `0x0C0B` | `3083` | `cli` | - | `7` | `0` | `0` | ClientPkg_0C0B | ClientNativePt1 | - |
| `0x0C15` | `3093` | `cli` | - | `5` | `0` | `10` | ClientNoticeBridge (TemporaryServiceEffect) | ClientNoticeEmit; Notices: `0x10000442`, `0x100004FB`, `0x100003C2` (+2) | TemporaryServiceEffect, TemporaryServiceEffect::AfterBeingAdded, TemporaryServiceEffect::BeforeBeingRemoved |
| `0x0C1F` | `3103` | `cli` | `0x30` | `2` | `0` | `0` | ClientPkg_0C1F | ClientNativePt1 | - |
| `0x0C26` | `3110` | `cli` | - | `10` | `0` | `5` | ClientPkg_0C26 | ClientNativePt1 | 0x0C0E#0x0014, 0x0C0E#0x0003 |
| `0x0C51` | `3153` | `cli` | - | `1` | `0` | `0` | ClientPkg_0C51 | Music/CLCache | - |
| `0x0C62` | `3170` | `cli` | - | `1` | `0` | `0` | ClientPkg_0C62 | ClientNativePt1, WLibVM/RefTable | - |
| `0x0C68` | `3176` | `cli` | - | `52` | `0` | `102` | ClientBridge (DistantUnitUtils) | ClientNativePt1, ClientNoticeEmit | DistantUnitUtils, CItemAdvancementSystem, DistantUnitUtils#0x0009, DistantUnitUtils::GetCosmeticEntityStruct |
| `0x0C6D` | `3181` | `cli` | `0x18` | `7` | `0` | `0` | ClientPkg_0C6D | Monitor/Surface, ClientNativePt1 | - |
| `0x0CA8` | `3240` | `cli` | - | `1` | `0` | `2` | ClientBridge (SkillSystem) | ClientNativePt3, Music/CLCache | SkillSystem, 0x0C96#0x0014, SkillSystem::Client_CanUseSkillBasedOnTargetVitals |
| `0x0CB0` | `3248` | `cli` | - | `3` | `0` | `3` | ClientBridge (MEOInventorySystem) | ClientNativePt1, ClientNoticeEmit | MEOInventorySystem, CMEOInventorySystem, MEOInventorySystem#0x0079, MEOInventorySystem#0x0072 |
| `0x0CBA` | `3258` | `cli` | - | `11` | `0` | `4` | ClientBridge (LogUtils) | ClientNativePt1, ClientNoticeEmit | LogUtils, 0x035E#0x000A, LogUtils::LogAndAssert |
| `0x0CBD` | `3261` | `base` | `0x28` | `18` | `5` | `21` | EnginePkg (EmoteQuestEvent) | WLibPkgTable/DB, PropertySystem | EmoteQuestEvent, IQuestEvent, CQuestSystem#0x007E, CQuestSystem#0x0077 |
| `0x0CBE` | `3262` | `base` | `0x20` | `6` | `0` | `18` | EnginePkg (ChannelingQuestEvent) | - | ChannelingQuestEvent, EmoteQuestEvent, CQuestSystem#0x007E, CQuestSystem#0x0077 |
| `0x0CC5` | `3269` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge (Admin) | ClientNoticeEmit; Notices: `0x10000410` | Admin, CAdmin, Admin#0x034A |
| `0x0CCF` | `3279` | `base` | - | `3` | `0` | `0` | EnginePkg_0CCF | EntityDesc/Weenie | - |
| `0x0CD0` | `3280` | `base` | - | `1` | `0` | `0` | EnginePkg_0CD0 | EntityDesc/Weenie | - |
| `0x0CD6` | `3286` | `cli` | - | `3` | `0` | `3` | ClientNoticeBridge (CFellowshipMakerSystem) | ClientNativePt3, ClientNoticeEmit; Notices: `0x10000433`, `0x1000043B`, `0x1000043A` | CFellowshipMakerSystem, CFellowshipMakerSystem::RecvCEvt_FellowConfirmEnterQueue, CFellowshipMakerSystem#0x0011 |
| `0x0CDC` | `3292` | `cli` | - | `2` | `0` | `1` | ClientNoticeBridge_0CDC | ClientNoticeEmit; Notices: `0x10000436`, `0x10000437` | 0x0CD5#0x0009 |
| `0x0CDF` | `3295` | `cli` | - | `10` | `0` | `18` | ClientBridge (CAdmin) | ClientNoticeEmit, ClientNativePt1 | CAdmin, Admin, 0x0675#0x005C, Admin#0x02C0 |
| `0x0CED` | `3309` | `base` | `0x18` | `4` | `0` | `0` | EnginePkg_0CED | WLibPkgTable/DB, VisualDesc | - |
| `0x0CFD` | `3325` | `cli` | - | `6` | `0` | `2` | ClientBridge (CQuestSystem) | ClientNativePt1, Core/String/Disk | CQuestSystem, QuestSystem, CQuestSystem#0x0088, CQuestSystem#0x008C |
| `0x0D12` | `3346` | `cli` | `0x20` | `9` | `0` | `0` | ClientPkg_0D12 | ClientNoticeEmit, Monitor/Surface | - |
| `0x0D3D` | `3389` | `cli` | - | `4` | `0` | `0` | ClientPkg_0D3D | ClientNoticeEmit, WLibVM/RefTable | - |
| `0x0D6C` | `3436` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0D6C | ClientNoticeEmit; Notices: `0x1000044D` | 0x0C23#0x0002 |
| `0x0D85` | `3461` | `cli` | `0x58` | `17` | `1` | `7` | ClientBridge (SkillSystem) | ClientNativePt1, PropertySystem | SkillSystem, SkillSystem#0x0128, SkillSystem#0x0149 |
| `0x0DA1` | `3489` | `cli` | - | `2` | `0` | `0` | ClientPkg_0DA1 | ClientNativePt1, Core/Math/Base | - |
| `0x0DEF` | `3567` | `cli` | - | `2` | `0` | `1` | ClientNoticeBridge_0DEF | ClientNativePt3, ClientNoticeEmit; Notices: `0x10000459`, `0x10000456` | 0x0DD0#0x0006 |
| `0x0E0E` | `3598` | `cli` | - | `2` | `0` | `2` | ClientNoticeBridge_0E0E | ClientNoticeEmit; Notices: `0x10000463`, `0x10000462` | 0x0124#0x000A, 0x0124#0x000C |
| `0x0E0F` | `3599` | `cli` | `0x170` | `15` | `0` | `15` | ClientBridge (CQuestSystem) | ClientSystems/Inv, ClientNativePt3 | CQuestSystem, CQuestSystem#0x00C1 |
| `0x0E10` | `3600` | `cli` | `0x18` | `3` | `0` | `0` | ClientPkg_0E10 | ClientNoticeEmit | - |
| `0x0E1F` | `3615` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge (CWebStoreSystem) | ClientNoticeEmit; Notices: `0x10000464` | CWebStoreSystem, CWebStoreSystem#0x0056 |
| `0x0E21` | `3617` | `cli` | `0xb0` | `2` | `1` | `2` | ClientPkg_0E21 | ClientNativePt1, Script/Channels | 0x0E0C#0x0003 |
| `0x0E22` | `3618` | `cli` | - | `3` | `0` | `3` | ClientNoticeBridge_0E22 | StringInfo/Taboo, ClientNativePt3; Notices: `0x1000046A`, `0x10000469` | 0x0E16#0x0009, 0x0E16#0x000C |
| `0x0E25` | `3621` | `cli` | `0x28` | `11` | `0` | `0` | ClientPkg_0E25 | ClientNativePt1, ClientNoticeEmit | - |
| `0x0E32` | `3634` | `cli` | `0x20` | `5` | `0` | `0` | ClientPkg_0E32 | ClientNoticeEmit, WLibPkgTable/DB | - |
| `0x0E39` | `3641` | `cli` | - | `7` | `0` | `7` | ClientPkg_0E39 | Core/Math/Base, ClientSystems/Inv | 0x0D29#0x00A1, 0x0D29#0x0094 |
| `0x0E5A` | `3674` | `cli` | - | `4` | `0` | `5` | ClientNoticeBridge (SkillSystem) | ClientSystems/Inv, ClientNativePt3; Notices: `0x10000444`, `0x10000443`, `0x100001A0` | SkillSystem, SkillSystem::Client_ExecutionRequest |
| `0x0E60` | `3680` | `cli` | `0x120` | `12` | `2` | `13` | ClientBridge (CTravelSystem) | ClientNoticeEmit, StringInfo/Taboo | CTravelSystem, TravelSystem, CTravelSystem#0x0028, CTravelSystem#0x0027 |
| `0x0E70` | `3696` | `cli` | - | `5` | `0` | `3` | ClientNoticeBridge_0E70 | ClientNoticeEmit; Notices: `0x1000048B`, `0x10000487`, `0x100004E2` (+2) | 0x0E75#0x0001, 0x0E75#0x0002 |
| `0x0E77` | `3703` | `cli` | - | `2` | `0` | `2` | ClientPkg_0E77 | EntityDesc/Weenie, PropertySystem | 0x0E71#0x0003, 0x0E71#0x0002 |
| `0x0E79` | `3705` | `cli` | - | `2` | `0` | `3` | ClientNoticeBridge (CRandomItemRewardSystem) | ClientNoticeEmit; Notices: `0x100004A9`, `0x10000497` | CRandomItemRewardSystem, CRandomItemRewardSystem::RecvCEvt_RandomItemRewardedFromTreasureTable, CRandomItemRewardSystem#0x0010 |
| `0x0E9C` | `3740` | `cli` | `0x18` | `5` | `3` | `11` | ClientBridge (BankRepositorySystem) | ClientNativePt1, Core/Math/Base | BankRepositorySystem, HousingBankPolicy, BankRepositorySystem#0x001B, BankRepositorySystem#0x0005 |
| `0x0EB0` | `3760` | `base` | - | `3` | `0` | `0` | EnginePkg_0EB0 | WLibPkgTable/DB, WLibVM/RefTable | - |
| `0x0EB3` | `3763` | `cli` | - | `2` | `0` | `2` | ClientNoticeBridge (CBigBattleSystem) | ClientNoticeEmit; Notices: `0x100004DF`, `0x100004A5` | CBigBattleSystem, CBigBattleSystem#0x000A, 0x0E8D#0x0180 |
| `0x0EC9` | `3785` | `cli` | `0x28` | `5` | `0` | `2` | ClientBridge (CBigBattleSystem) | ClientNativePt1, ClientNoticeEmit | CBigBattleSystem, CBigBattleSystem::RecvCEvt_InitializeMedalRecords |
| `0x0EE7` | `3815` | `cli` | - | `10` | `0` | `12` | ClientNoticeBridge (CMultiUsageSystem) | ClientNoticeEmit; Notices: `0x100004B3`, `0x100004B4`, `0x100004CD` (+7) | CMultiUsageSystem, Missile, Missile::StartCollision, FireSiegeWeaponScriptCallback::FireSiegeWeapon |
| `0x0EF8` | `3832` | `cli` | `0x28` | `3` | `0` | `2` | ClientPkg_0EF8 | ClientNativePt1, Core/String/Disk | 0x0EFC#0x0008, 0x0EFC#0x0009 |
| `0x0EFB` | `3835` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0EFB | ClientNoticeEmit; Notices: `0x100004B6` | 0x0EFC#0x0001 |
| `0x0F05` | `3845` | `cli` | `0xd0` | `9` | `0` | `8` | ClientBridge (CMultiUsageSystem) | ClientNoticeEmit, ClientNativePt3 | CMultiUsageSystem, CMultiUsageSystem#0x001A |
| `0x0F08` | `3848` | `base` | - | `3` | `0` | `1` | EnginePkg_0F08 | EntityDesc/Weenie, Core/Math/Base | 0x0F0A#0x0004 |
| `0x0F16` | `3862` | `base` | - | `2` | `0` | `2` | EnginePkg (CBigBattleSystem) | Render/SmartBox | CBigBattleSystem, CBigBattleSystem#0x0003, CBigBattleSystem#0x0004 |
| `0x0F23` | `3875` | `cli` | - | `3` | `0` | `3` | ClientNoticeBridge (CBigBattleSystem) | ClientNoticeEmit; Notices: `0x100004C2`, `0x100004BC`, `0x100004BE` | CBigBattleSystem, CBigBattleSystem::RecvCEvt_InitializeMedalRecords, CBigBattleSystem#0x0006 |
| `0x0F2D` | `3885` | `cli` | `0x38` | `3` | `0` | `0` | ClientPkg_0F2D | ClientNoticeEmit | - |
| `0x0F33` | `3891` | `base` | `0x18` | `4` | `0` | `0` | EnginePkg_0F33 | VisualDesc | - |
| `0x0F3C` | `3900` | `base` | - | `1` | `0` | `0` | EnginePkg_0F3C | EntityDesc/Weenie | - |
| `0x0F3F` | `3903` | `cli` | `0x98` | `16` | `0` | `0` | ClientPkg_0F3F | ClientNativePt1, ClientNoticeEmit | - |
| `0x0F41` | `3905` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0F41 | ClientNoticeEmit; Notices: `0x100004CB` | 0x0F40#0x0005 |
| `0x0F42` | `3906` | `cli` | `0x1c8` | `2` | `0` | `0` | ClientPkg_0F42 | ClientNoticeEmit | - |
| `0x0F44` | `3908` | `cli` | - | `2` | `0` | `4` | ClientNoticeBridge (CTravelSystem) | ClientNoticeEmit; Notices: `0x100004D2`, `0x100004D4` | CTravelSystem, MannequinUsageAction, 0x07A6#0x00AA, 0x07A6#0x0006 |
| `0x0F47` | `3911` | `cli` | `0x18` | `2` | `0` | `0` | ClientPkg_0F47 | ClientNativePt1, CObjectMaint/MapNote | - |
| `0x0F5F` | `3935` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_0F5F | ClientNoticeEmit; Notices: `0x100004DD` | 0x0F5D#0x0004 |
| `0x0F76` | `3958` | `cli` | - | `4` | `0` | `6` | ClientNoticeBridge_0F76 | ClientNoticeEmit; Notices: `0x10000512`, `0x10000539`, `0x100004E1` (+1) | 0x0F71#0x000C, 0x0F71#0x000B |
| `0x0FA2` | `4002` | `cli` | - | `2` | `0` | `2` | ClientNoticeBridge_0FA2 | ClientNoticeEmit; Notices: `0x100004E5`, `0x100004E4` | 0x0F9F#0x0002, 0x0F9F#0x0003 |
| `0x0FE0` | `4064` | `cli` | - | `1` | `0` | `1` | ClientBridge (CWebStoreSystem) | ClientNoticeEmit | CWebStoreSystem, CWebStoreSystem#0x0007 |
| `0x0FE3` | `4067` | `cli` | `0x10` | `1` | `1` | `22` | ClientBridge (WebStoreSystem) | ClientNoticeEmit | WebStoreSystem, CWebStoreSystem, CCraftSystem#0x0048, 0x05AE#0x0010 |
| `0x0FE4` | `4068` | `base` | - | `1` | `0` | `2` | EnginePkg (WebStoreSystem) | Archive/CRT | WebStoreSystem, CWebStoreSystem, WebStoreSystem#0x0052, WebStoreSystem#0x005A |
| `0x0FE9` | `4073` | `base` | - | `1` | `0` | `1` | EnginePkg (CUsageSystem) | MovieTheatre | CUsageSystem, CUsageSystem#0x004C |
| `0x0FEB` | `4075` | `cli` | - | `2` | `0` | `1` | ClientBridge (DistantUnitUtils) | ClientNativePt3, Music/CLCache | DistantUnitUtils, DistantUnitUtils::GetCosmeticEntityStruct |
| `0x0FED` | `4077` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge (EmoteSystem) | ClientNoticeEmit; Notices: `0x100004F5` | EmoteSystem, EmoteSystem::RecvCEvt_OnGrantEmoteAccess |
| `0x0FEE` | `4078` | `cli` | `0x38` | `5` | `0` | `1` | ClientBridge (ScatterLayoutCalculator) | ClientNativePt1, ClientNoticeEmit | ScatterLayoutCalculator, ScatterLayoutCalculator#0x0004 |
| `0x1002` | `4098` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge (CImageItemSystem) | ClientNoticeEmit; Notices: `0x100004FC` | CImageItemSystem, CImageItemSystem::RecvCEvt_ShowImageItem |
| `0x102B` | `4139` | `base` | - | `1` | `0` | `1` | EnginePkg (SkirmishEncounterTemplate) | Archive/CRT | SkirmishEncounterTemplate, SkirmishEncounterTemplate#0x0019 |
| `0x102F` | `4143` | `cli` | `0xd0` | `9` | `1` | `9` | ClientPkg_102F | ClientNativePt1, HavokPhysics | 0x1030#0x0003 |
| `0x1046` | `4166` | `base` | - | `1` | `0` | `1` | EnginePkg (LogUtils) | - | LogUtils, LogUtils#0x0046 |
| `0x1061` | `4193` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_1061 | ClientNoticeEmit; Notices: `0x10000527` | 0x1062#0x001E |
| `0x1066` | `4198` | `cli` | - | `6` | `0` | `7` | ClientNoticeBridge_1066 | ClientNoticeEmit; Notices: `0x10000536`, `0x1000052D`, `0x1000052C` (+3) | 0x1052#0x0027, 0x1054#0x0039 |
| `0x1070` | `4208` | `base` | `0x8` | `1` | `0` | `0` | EnginePkg_1070 | - | - |
| `0x1073` | `4211` | `cli` | - | `1` | `0` | `1` | ClientNoticeBridge_1073 | ClientNoticeEmit; Notices: `0x1000053A` | 0x1011#0x000F |
| `0x1075` | `4213` | `cli` | `0x28` | `14` | `1` | `24` | ClientPkg_1075 | Monitor/Surface, HavokPhysics | 0x0FA7#0x0001, 0x1078#0x0009 |
| `0x1089` | `4233` | `cli` | - | `5` | `0` | `4` | ClientNoticeBridge_1089 | ClientNoticeEmit, Core/String/Disk; Notices: `0x10000541`, `0x10000540`, `0x1000053F` (+2) | 0x1087#0x0005, 0x1087#0x0007 |
| `0x15EB` | `5611` | `base` | - | `2` | `0` | `0` | EnginePkg_15EB | ClientDirectory | - |
| `0x1669` | `5737` | `cli` | `0xb0` | `2` | `2` | `2` | ClientPkg_1669 | ClientNativePt1, Script/Channels | 0x1668#0x000A |
| `0x1683` | `5763` | `cli` | `0x40` | `15` | `1` | `15` | ClientPkg_1683 | HavokPhysics, Monitor/Surface | 0x1681#0x0016, 0x1681#0x0015 |
| `0x1686` | `5766` | `cli` | `0x38` | `14` | `1` | `14` | ClientPkg_1686 | ClientNativePt1, Core/Math/Base | 0x1682#0x0010, 0x1682#0x0008 |
| `0x16A4` | `5796` | `cli` | `0x90` | `18` | `0` | `9` | ClientBridge (NPCSystem) | ClientNoticeEmit, HavokPhysics | NPCSystem, RequiredNeighborRelationshipUsagePlugin, NPCSystem#0x0034, NPCSystem#0x001F |

---

### 2.8 Complete Census of All 549 Server -> Client (`S->C`) WLib RPC Stubs (`0x8XXXXXXX`) <a id="s2c-rpc-census"></a>

Every Server -> Client WLib RPC method in `client_gamelogic.dat` record `0x56000000` (`wlib.bin`) verifies the target entity interface via `0x00D60007`, rewinds the `WLibArgFrame` read cursor via `0x008E000D`, unpacks parameters in wire order via `0x008E0006..0x008E000A`, and dispatches to the paired client subsystem handler:

| RPC Method ID | Root `cid` | `fnum` | Unpacked `WLibArgFrame` Signature (`0x008E0006..0A`) | Target WLib Handler (`ClassName::RecvCEvt_*`) | Downstream Native `(pkg, func)` Calls | Emitted UI Notice IDs |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x84D90004` | `0x04D9` | `0x0004` | `(i64)` | `0x00FE#0x0024` | `0x01550003` | `0x10000086` |
| `0x84D9000F` | `0x04D9` | `0x000F` | `(i32)` | `0x00FE#0x001F` | `0x01550007` | `0x100000CB` |
| `0x84D90024` | `0x04D9` | `0x0024` | `(ref, i32, i32)` | `0x00FE#0x0015` | `0x01550002` | `0x100000BF` |
| `0x84D90026` | `0x04D9` | `0x0026` | `(f64, f64, i32)` | `0x00FE#0x0025` | `0x01550009` | `0x100001EA` |
| `0x84D90036` | `0x04D9` | `0x0036` | `(i32)` | `0x00FE#0x0017` | `0x01550008` | `0x100000CA` |
| `0x84D9004F` | `0x04D9` | `0x004F` | `` | `0x00FE#0x001B` | `0x0155000A` | `0x100001B1` |
| `0x84D90059` | `0x04D9` | `0x0059` | `(i32)` | `0x00FE#0x0014` | `0x01550006` | `0x1000007C` |
| `0x84D90080` | `0x04D9` | `0x0080` | `(i32)` | `0x00FE#0x002D` | - | - |
| `0x84DA0004` | `0x04DA` | `0x0004` | `(ref, ref)` | `0x00FC#0x0006` | `0x012E0001` | `0x1000014A` |
| `0x84DA0006` | `0x04DA` | `0x0006` | `` | `AccomplishmentSystem::GetAllAccomplishments` | `0x012E0001` | `0x1000014A` |
| `0x84DA000A` | `0x04DA` | `0x000A` | `(i32)` | `0x00FC#0x0007` | `0x012E0001` | `0x1000014A` |
| `0x84DC0033` | `0x04DC` | `0x0033` | `(ref, ref)` | - | - | - |
| `0x84DD0002` | `0x04DD` | `0x0002` | `(ref, ref, ref, ref, i64)` | `AdminPropertySystem#0x0010` | `0x00E70003`, `0x01810006`, `0x012F0001` | `0x100000C5` |
| `0x84DD0007` | `0x04DD` | `0x0007` | `(ref, ref)` | `AdminPropertySystem#0x000E` | `0x012F0002` | `0x10000141` |
| `0x84DE0018` | `0x04DE` | `0x0018` | `(ref)` | `PhysicsSystem#0x0014` | `0x0137000C` | `0x10000222` |
| `0x84DE0069` | `0x04DE` | `0x0069` | `(ref)` | `PhysicsSystem#0x0030` | `0x0137000F` | `0x10000229` |
| `0x84DE0099` | `0x04DE` | `0x0099` | `(ref)` | `PhysicsSystem#0x0048` | `0x01370017` | `0x10000278` |
| `0x84DE009B` | `0x04DE` | `0x009B` | `(ref)` | `PhysicsSystem#0x0049` | `0x01370016` | `0x10000277` |
| `0x84DE0131` | `0x04DE` | `0x0131` | `(ref)` | `PhysicsSystem#0x0088` | `0x00B0000B`, `0x00DD0003`, `0x00DD0001`, `0x00B00003`, `0x00E10002` (+2) | - |
| `0x84DF0003` | `0x04DF` | `0x0003` | `(ref, i32)` | - | - | - |
| `0x84DF0016` | `0x04DF` | `0x0016` | `(i32, i32)` | `AdvancementSystem#0x0061` | `0x00C7000C`, `0x00C70003` | - |
| `0x84DF001B` | `0x04DF` | `0x001B` | `(i64)` | `AdvancementSystem#0x005C` | `0x01300005` | `0x10000146` |
| `0x84DF001E` | `0x04DF` | `0x001E` | `(ref)` | - | - | - |
| `0x84E10006` | `0x04E1` | `0x0006` | `(i32)` | - | - | - |
| `0x84E10007` | `0x04E1` | `0x0007` | `(ref)` | - | - | - |
| `0x84E40001` | `0x04E4` | `0x0001` | `(ref)` | `0x00FF#0x0004` | `0x00390001` | - |
| `0x84E5000B` | `0x04E5` | `0x000B` | `(i32, ref)` | `AuctionSystem#0x0059` | `0x01890016`, `0x01330007` | `0x10000178` |
| `0x84E5000F` | `0x04E5` | `0x000F` | `(i32, i64)` | `AuctionSystem#0x0057` | `0x01330002` | `0x100000C6` |
| `0x84E50014` | `0x04E5` | `0x0014` | `(i32, ref)` | `AuctionSystem#0x0054` | `0x01330008` | `0x10000095` |
| `0x84E5001E` | `0x04E5` | `0x001E` | `(i32, i32, i32, ref)` | `AuctionSystem#0x0055` | `0x01330006` | `0x100000A9` |
| `0x84E50021` | `0x04E5` | `0x0021` | `(i32)` | `AuctionSystem#0x005A` | - | - |
| `0x84E50024` | `0x04E5` | `0x0024` | `(ref)` | `AuctionSystem#0x0053` | `0x01330003` | `0x100000E4` |
| `0x84E50027` | `0x04E5` | `0x0027` | `(ref)` | `AuctionSystem#0x0056` | `0x01890016`, `0x01330004` | `0x10000017` |
| `0x84E5002C` | `0x04E5` | `0x002C` | `` | `AuctionSystem#0x005B` | `0x01330001` | `0x1000002E` |
| `0x84E50043` | `0x04E5` | `0x0043` | `(i64)` | `AuctionSystem#0x0088` | - | - |
| `0x84E90001` | `0x04E9` | `0x0001` | `(i32, i64)` | `CCombatSystem#0x0012` | - | - |
| `0x84E9001B` | `0x04E9` | `0x001B` | `(i32)` | `CCombatSystem#0x000B` | `0x00B80004`, `0x00B8000B`, `0x00B80007`, `0x00B8000C`, `0x00B90002` | - |
| `0x84E9002D` | `0x04E9` | `0x002D` | `(i32)` | - | - | - |
| `0x84E9003D` | `0x04E9` | `0x003D` | `(ref)` | `CCombatSystem#0x0018` | `0x07EC0001` | `0x10000269` |
| `0x84E9004F` | `0x04E9` | `0x004F` | `(ref, i64)` | `CCombatSystem#0x0044` | `0x07EC0007` | `0x100003BF` |
| `0x84EB0009` | `0x04EB` | `0x0009` | `(i64, i32)` | `0x08DA#0x0001` | `0x00E70001`, `0x0030000B` | - |
| `0x84EC0005` | `0x04EC` | `0x0005` | `(i32, ref, i32, ref, i64)` | `CCommunicationSystem#0x0026` | `0x01810007`, `0x00E70003`, `0x019F001D`, `0x0137000B` | `0x10000001` |
| `0x84EC000A` | `0x04EC` | `0x000A` | `(ref, i64)` | - | - | - |
| `0x84EC001A` | `0x04EC` | `0x001A` | `(i32, i64, ref, ref)` | `CCommunicationSystem#0x0036` | `0x0137000A`, `0x01370009` | `0x100001C3`; `0x10000030` |
| `0x84EC001C` | `0x04EC` | `0x001C` | `(i32, ref, i32)` | `CCommunicationSystem#0x0024` | `0x01B10004`, `0x01B10001`, `0x01400004`, `0x01400005` | `0x10000155`; `0x10000015` |
| `0x84EC001D` | `0x04EC` | `0x001D` | `(ref)` | `CCommunicationSystem#0x0023` | `0x0075000A`, `0x00760006`, `0x00760002`, `0x006F000E`, `0x00760001` (+13) | - |
| `0x84EC001E` | `0x04EC` | `0x001E` | `(ref, i64)` | - | - | - |
| `0x84EC002C` | `0x04EC` | `0x002C` | `(ref, ref, i64)` | `CCommunicationSystem#0x0044` | `0x01AA0002`, `0x019F001D`, `0x01810007`, `0x01370008` | `0x100001D5` |
| `0x84EC003B` | `0x04EC` | `0x003B` | `(ref, i64, i64)` | `CCommunicationSystem#0x004E` | `0x01810007` | - |
| `0x84EC0040` | `0x04EC` | `0x0040` | `(ref)` | `CCommunicationSystem#0x0051` | - | - |
| `0x84EC0044` | `0x04EC` | `0x0044` | `(i32, ref, ref)` | `CCommunicationSystem#0x0052` | - | - |
| `0x84EC0049` | `0x04EC` | `0x0049` | `(ref, ref, i64)` | `CCommunicationSystem#0x0054` | `0x01AA0002`, `0x019F001D`, `0x01810007`, `0x01370008` | `0x100001D5` |
| `0x84ED0005` | `0x04ED` | `0x0005` | `(i64, i64)` | `0x0103#0x0009` | `0x01380003` | `0x1000011E` |
| `0x84ED0006` | `0x04ED` | `0x0006` | `(ref, i64)` | `0x0103#0x0007` | `0x01380001` | `0x10000168` |
| `0x84ED000C` | `0x04ED` | `0x000C` | `(ref, ref, i64)` | `0x0103#0x000A` | `0x00E70001`, `0x01AA0007` | - |
| `0x84ED001C` | `0x04ED` | `0x001C` | `(i64, i32, i64)` | `0x0103#0x0006` | `0x01380002` | `0x1000020A` |
| `0x84EE0001` | `0x04EE` | `0x0001` | `(ref)` | `0x0106#0x0008` | `0x006F002E`, `0x00700008`, `0x00700003`, `0x00700001` | - |
| `0x84EE0003` | `0x04EE` | `0x0003` | `` | `0x0106#0x0003` | `0x006F002E`, `0x00700008`, `0x00700003`, `0x00700001` | - |
| `0x84EE0008` | `0x04EE` | `0x0008` | `(ref, i64)` | `0x0106#0x0005` | `0x01390002`, `0x01390001` | `0x10000131`; `0x10000063` |
| `0x84EE000A` | `0x04EE` | `0x000A` | `(i64)` | `0x0106#0x000A` | `0x01390002`, `0x01390001` | `0x10000131`; `0x10000063` |
| `0x84EF0001` | `0x04EF` | `0x0001` | `(i32, i32)` | `CCraftSystem#0x000A` | `0x013A0003` | `0x1000007B` |
| `0x84EF000A` | `0x04EF` | `0x000A` | `(i32)` | `CCraftSystem#0x0010` | `0x00E70001` | - |
| `0x84EF000B` | `0x04EF` | `0x000B` | `(i32, i32)` | `CCraftSystem#0x0011` | `0x013A0001` | `0x100001DB` |
| `0x84EF000C` | `0x04EF` | `0x000C` | `(i32, i32)` | `CCraftSystem#0x0016` | `0x013A0002` | `0x100001FD` |
| `0x84EF000D` | `0x04EF` | `0x000D` | `(i32, i64)` | `CCraftSystem#0x000C` | `0x013A0006` | `0x1000011A` |
| `0x84EF0010` | `0x04EF` | `0x0010` | `(i32)` | `CCraftSystem#0x0012` | `0x00E70001` | - |
| `0x84EF0011` | `0x04EF` | `0x0011` | `(i32, i32)` | `CCraftSystem#0x0009` | `0x013A0004` | `0x1000001B` |
| `0x84EF0016` | `0x04EF` | `0x0016` | `(i32, i32, i64)` | `CCraftSystem#0x0014` | `0x013A0005`, `0x01810004`, `0x01810003` | `0x10000081` |
| `0x84EF003A` | `0x04EF` | `0x003A` | `(i32, i64)` | `CCraftSystem#0x001C` | `0x013A0009` | `0x100002C6` |
| `0x84EF0047` | `0x04EF` | `0x0047` | `(i32)` | `CCraftSystem#0x002B` | `0x013A0010` | `0x10000380` |
| `0x84EF0051` | `0x04EF` | `0x0051` | `(i32)` | `CCraftSystem#0x0041` | `0x013A0015` | `0x10000553` |
| `0x84EF0052` | `0x04EF` | `0x0052` | `(i32)` | `CCraftSystem#0x0042` | `0x013A0015` | `0x10000553` |
| `0x84EF0055` | `0x04EF` | `0x0055` | `` | `CCraftSystem#0x0044` | `0x013A0015` | `0x10000553` |
| `0x84EF0057` | `0x04EF` | `0x0057` | `(i32, i64)` | `CCraftSystem#0x0045` | `0x013A0017` | `0x10000556` |
| `0x84F10001` | `0x04F1` | `0x0001` | `(ref, ref)` | `0x0109#0x0003` | `0x013B0001` | - |
| `0x84F20003` | `0x04F2` | `0x0003` | `(i32)` | `0x0129#0x000D` | `0x0135000B` | `0x100001AB` |
| `0x84F20004` | `0x04F2` | `0x0004` | `(ref)` | `0x0129#0x0007` | `0x0135000C` | `0x1000005E` |
| `0x84F2000A` | `0x04F2` | `0x000A` | `` | `0x0129#0x0004` | `0x00E70001` | - |
| `0x84F20010` | `0x04F2` | `0x0010` | `` | `0x0129#0x000A` | `0x0135000D` | `0x100001F1` |
| `0x84F20012` | `0x04F2` | `0x0012` | `(ref, ref, i64)` | `0x0129#0x0009` | `0x00E70001`, `0x01AA0001` | - |
| `0x84F20013` | `0x04F2` | `0x0013` | `(ref, ref, i64)` | `0x0129#0x000C` | `0x00E70001`, `0x01AA0001` | - |
| `0x84F2001C` | `0x04F2` | `0x001C` | `` | `0x0129#0x000E` | `0x00E70001` | - |
| `0x84F2004C` | `0x04F2` | `0x004C` | `(f64)` | `0x0129#0x0022` | `0x0135004A` | `0x1000044C` |
| `0x84F30003` | `0x04F3` | `0x0003` | `(ref, ref)` | `0x010A#0x0004` | `0x013C0001` | `0x10000145` |
| `0x84F70010` | `0x04F7` | `0x0010` | `(i64, ref)` | `0x06D8#0x0001` | `0x00C7000C`, `0x00C70009`, `0x00C70003`, `0x00330001`, `0x00A6001F` (+6) | - |
| `0x84F70023` | `0x04F7` | `0x0023` | `` | `0x06D8#0x0004` | `0x0099001B` | - |
| `0x84F80003` | `0x04F8` | `0x0003` | `(i32, i32)` | - | - | - |
| `0x84F8000F` | `0x04F8` | `0x000F` | `(ref, i32)` | - | - | - |
| `0x84F80019` | `0x04F8` | `0x0019` | `(i32, ref, i32, i64)` | `CEffectListenerSystem#0x0002` | `0x01670010` | `0x1000011B` |
| `0x84F80020` | `0x04F8` | `0x0020` | `(ref, i32)` | `CEffectSystem#0x0007` | `0x00E7000A`, `0x01350006` | `0x10000097` |
| `0x84F80023` | `0x04F8` | `0x0023` | `(i64)` | `CEffectListenerSystem::RecvCEvt_Effect_CRemoveListener` | `0x01670010`, `0x0030000B`, `0x0CBA000D` | `0x1000011B` |
| `0x84F80025` | `0x04F8` | `0x0025` | `(ref, i64)` | `CEffectListenerSystem#0x0001` | `0x01670010` | `0x1000011B` |
| `0x84F80031` | `0x04F8` | `0x0031` | `(ref)` | - | - | - |
| `0x84F80084` | `0x04F8` | `0x0084` | `(ref)` | `CEffectListenerSystem#0x0008` | `0x01670010` | `0x1000011B` |
| `0x84F90002` | `0x04F9` | `0x0002` | `(ref, ref, i64, ref, i32)` | `EmoteSystem::RecvCEvt_SeeTargetedPrefabEmote` | `0x00E70001`, `0x0CBA000D` | - |
| `0x84F90003` | `0x04F9` | `0x0003` | `(ref, ref, i64, i32)` | `EmoteSystem::RecvCEvt_SeePrefabEmote` | `0x00E70001`, `0x0CBA000D` | - |
| `0x84F90006` | `0x04F9` | `0x0006` | `(ref, ref, i64)` | `EmoteSystem#0x0016` | `0x00E70001` | - |
| `0x84F90010` | `0x04F9` | `0x0010` | `(i32, i64)` | `EmoteSystem::RecvCEvt_OnGrantEmoteAccess` | `0x00E70001`, `0x0FED0001`, `0x0CBA000D` | `0x100004F5` |
| `0x84FC0003` | `0x04FC` | `0x0003` | `(ref, i32)` | `CErrorSystem#0x0005` | `0x013D0002` | `0x10000082` |
| `0x84FC0005` | `0x04FC` | `0x0005` | `(i32)` | `CErrorSystem::HandleClientError` | `0x013D0001` | `0x10000102` |
| `0x84FD0006` | `0x04FD` | `0x0006` | `(i32, ref, ref)` | `CExaminationSystem::PurgeCacheOfOldData` | `0x013E0003`, `0x01B00008`, `0x01B00004`, `0x01B0000A`, `0x01B00006` (+2) | `0x100001B4` |
| `0x84FD0011` | `0x04FD` | `0x0011` | `(ref, ref, ref, i32, i32, i32, ref, ref, ref, ref, ref, ref, i32, i64)` | `CExaminationSystem#0x001B` | `0x013E0007` | - |
| `0x84FD0016` | `0x04FD` | `0x0016` | `(i32, i64)` | `CExaminationSystem#0x000A` | `0x013E0006` | `0x100000C7` |
| `0x84FD0017` | `0x04FD` | `0x0017` | `(i32, ref, i64)` | `CErrorSystem::HandleClientError` | `0x01670014`, `0x013D0001` | `0x100001D4`; `0x10000102` |
| `0x84FD0028` | `0x04FD` | `0x0028` | `(ref, i64)` | `CExaminationSystem#0x001D` | `0x013E0009`, `0x00E70001` | `0x100002F2` |
| `0x84FE0005` | `0x04FE` | `0x0005` | `(i64, i64)` | `0x0452#0x0055` | - | - |
| `0x84FE001E` | `0x04FE` | `0x001E` | `(i64)` | `0x0452#0x0053` | `0x013F0004` | `0x1000005A` |
| `0x84FE0020` | `0x04FE` | `0x0020` | `(ref, i64)` | `0x0452#0x004E` | - | - |
| `0x84FE0022` | `0x04FE` | `0x0022` | `(ref, i64)` | `0x0452#0x004A` | `0x013F0003` | `0x10000212` |
| `0x84FE002E` | `0x04FE` | `0x002E` | `(i32, i64)` | `0x0452#0x0043` | `0x013F0006` | `0x10000016` |
| `0x84FE0032` | `0x04FE` | `0x0032` | `(i32, i32)` | `0x0452#0x0047` | - | - |
| `0x84FE0034` | `0x04FE` | `0x0034` | `` | `0x0452#0x0054` | `0x013F0002`, `0x013F000D` | `0x100000E0`; `0x100001EC` |
| `0x84FE0039` | `0x04FE` | `0x0039` | `(i32)` | `0x0452#0x0049` | - | - |
| `0x84FE0044` | `0x04FE` | `0x0044` | `(ref)` | `0x0452#0x0052` | `0x013F0001` | `0x10000072` |
| `0x84FE004F` | `0x04FE` | `0x004F` | `(i64)` | `0x0452#0x0045` | `0x013F000A` | `0x100001F9` |
| `0x84FE0063` | `0x04FE` | `0x0063` | `(i32, ref, i32, i32, ref, i64)` | `0x0452#0x0051` | `0x00C7000C`, `0x00C70009` | - |
| `0x84FE0073` | `0x04FE` | `0x0073` | `` | `0x0452#0x0046` | `0x013F000B` | `0x100001DE` |
| `0x84FE007E` | `0x04FE` | `0x007E` | `(i64)` | `0x0452#0x0044` | `0x00C7000C` | - |
| `0x84FE0098` | `0x04FE` | `0x0098` | `(i32, i64)` | `0x0452#0x004D` | `0x013F0005` | `0x100001A8` |
| `0x84FE009C` | `0x04FE` | `0x009C` | `(i64, i64)` | `0x0452#0x0050` | `0x013F000F` | `0x100000FE` |
| `0x84FE009D` | `0x04FE` | `0x009D` | `(i32, i64)` | `0x0452#0x004C` | `0x013F0007` | `0x100001D9` |
| `0x84FE00A0` | `0x04FE` | `0x00A0` | `(i32)` | `0x0452#0x004B` | `0x013F000D` | `0x100001EC` |
| `0x84FE00A1` | `0x04FE` | `0x00A1` | `(i32, i64, i64)` | `0x0452#0x004F` | `0x013F000E` | `0x100000CC` |
| `0x84FE00B8` | `0x04FE` | `0x00B8` | `(i32, i32, i64)` | `0x0452#0x005A` | `0x013F0011` | `0x1000022C` |
| `0x84FE00BA` | `0x04FE` | `0x00BA` | `(ref, i64)` | `0x0452#0x005B` | - | - |
| `0x84FE00C4` | `0x04FE` | `0x00C4` | `(i32, i64)` | `0x019D#0x002F` | `0x0030000B` | - |
| `0x84FE00C6` | `0x04FE` | `0x00C6` | `(i32, i64)` | `0x019D#0x0030` | `0x0030000B` | - |
| `0x84FE00D7` | `0x04FE` | `0x00D7` | `(i32, i64)` | `0x0452#0x006B` | `0x013F0013` | `0x1000023B` |
| `0x84FE00DB` | `0x04FE` | `0x00DB` | `` | `0x0452#0x006C` | `0x00C7000C`, `0x00330001`, `0x00A6001F`, `0x00A6002B`, `0x00A60015` (+3) | - |
| `0x84FE00E6` | `0x04FE` | `0x00E6` | `(i32, i64)` | `0x0452#0x0071` | `0x013F0014` | `0x10000246` |
| `0x84FE00FA` | `0x04FE` | `0x00FA` | `(i32, i32)` | `0x0452#0x007B` | - | - |
| `0x84FE0106` | `0x04FE` | `0x0106` | `` | `0x0452#0x0082` | - | - |
| `0x84FE0118` | `0x04FE` | `0x0118` | `(i32)` | `0x0452#0x008A` | `0x00DF0001` | - |
| `0x84FE0119` | `0x04FE` | `0x0119` | `(i32)` | `0x0452#0x008B` | `0x01AA0005` | - |
| `0x84FE011E` | `0x04FE` | `0x011E` | `(f32, i32, i32, i64)` | `0x0452#0x008C` | `0x00E70001`, `0x013F0017` | `0x10000293` |
| `0x84FE0133` | `0x04FE` | `0x0133` | `(ref, i64)` | `0x0452#0x0097` | `0x00E70001`, `0x00E70002`, `0x01810004`, `0x01810003` | - |
| `0x84FE0136` | `0x04FE` | `0x0136` | `(ref, i64)` | `0x0452#0x0099` | `0x00E70001`, `0x00E70002`, `0x01810004`, `0x01810003` | - |
| `0x84FE0138` | `0x04FE` | `0x0138` | `(i32, ref, i64)` | `0x0452#0x0098` | `0x00E70001`, `0x00E70002`, `0x01810004`, `0x01810003` | - |
| `0x84FE013E` | `0x04FE` | `0x013E` | `(i64)` | `0x0452#0x00A0` | `0x013F0021` | `0x100002F7` |
| `0x84FE0149` | `0x04FE` | `0x0149` | `(i64)` | `0x0452#0x00A1` | `0x013F0020` | `0x100002F6` |
| `0x84FE014E` | `0x04FE` | `0x014E` | `(i32, ref, i64)` | `0x0452#0x00A3` | `0x0A440002`, `0x013F0027` | `0x100003A7` |
| `0x84FE0153` | `0x04FE` | `0x0153` | `(i32, i32)` | `0x0452#0x00A5` | `0x013F0027` | `0x100003A7` |
| `0x84FE016A` | `0x04FE` | `0x016A` | `(i64)` | `FellowshipSystem::AddAutoFellow` | `0x00DF0001`, `0x00690001` | - |
| `0x84FE016B` | `0x04FE` | `0x016B` | `(i64)` | `0x019D#0x007C` | `0x00690002`, `0x00690006` | - |
| `0x85020002` | `0x0502` | `0x0002` | `(i32, i64)` | `GuildSystem#0x0121` | `0x0141000C` | `0x100000AD` |
| `0x85020008` | `0x0502` | `0x0008` | `(ref)` | `GuildSystem::UpdateGuildName` | `0x01410003` | `0x10000204` |
| `0x8502000C` | `0x0502` | `0x000C` | `(i64, i64)` | `GuildSystem#0x0136` | `0x0141000A` | `0x10000035` |
| `0x85020018` | `0x0502` | `0x0018` | `(ref)` | `GuildSystem#0x0112` | - | - |
| `0x8502001C` | `0x0502` | `0x001C` | `(i32, i64)` | `GuildSystem#0x0131` | `0x01410008` | `0x100000D2` |
| `0x85020030` | `0x0502` | `0x0030` | `(ref, i64)` | `GuildSystem::OnMotDUpdated` | `0x01AA0001`, `0x01410007` | `0x10000132` |
| `0x85020042` | `0x0502` | `0x0042` | `(i32, i32)` | `GuildSystem#0x0127` | - | - |
| `0x85020044` | `0x0502` | `0x0044` | `(i32, ref, ref, i64)` | `GuildSystem#0x013A` | `0x01AA0001`, `0x0141000E` | `0x10000006` |
| `0x85020055` | `0x0502` | `0x0055` | `(i32, ref, i64)` | `GuildSystem::OnGuildUpdated` | `0x01AA0001`, `0x01410001` | `0x10000064` |
| `0x85020064` | `0x0502` | `0x0064` | `(i32, ref, i64, ref, i64)` | `GuildSystem#0x011B` | `0x01AA0001`, `0x0141000D` | `0x10000163` |
| `0x85020066` | `0x0502` | `0x0066` | `(i32, ref, ref, i64)` | `GuildSystem#0x012A` | `0x0141000E` | `0x10000006` |
| `0x85020080` | `0x0502` | `0x0080` | `(ref, ref)` | `GuildSystem#0x011C` | `0x01AA0001`, `0x01410008` | `0x100000D2` |
| `0x85020091` | `0x0502` | `0x0091` | `(ref)` | `GuildSystem#0x0117` | - | - |
| `0x85020097` | `0x0502` | `0x0097` | `(i32, i64, i64, i64)` | `GuildSystem#0x012B` | `0x01AA0001`, `0x01410009` | `0x10000080` |
| `0x8502009B` | `0x0502` | `0x009B` | `(ref, i64)` | `GuildSystem::UpdateGuildMemberName` | `0x0141000B`, `0x01410008` | `0x10000007`; `0x100000D2` |
| `0x8502009C` | `0x0502` | `0x009C` | `(i32, ref, i64)` | `GuildSystem::OnMemberUpdated` | `0x01AA0001`, `0x01410008` | `0x100000D2` |
| `0x850200A8` | `0x0502` | `0x00A8` | `(i32, i64, i64)` | `GuildSystem#0x0119` | `0x01AA0001` | - |
| `0x850200DF` | `0x0502` | `0x00DF` | `(i32, i64)` | `GuildSystem#0x0152` | `0x01410008` | `0x100000D2` |
| `0x850200E2` | `0x0502` | `0x00E2` | `(i32)` | `GuildSystem#0x0155` | `0x01410015` | `0x10000230` |
| `0x850200F8` | `0x0502` | `0x00F8` | `(f64)` | `GuildSystem#0x0185` | - | - |
| `0x85020252` | `0x0502` | `0x0252` | `(ref)` | `GuildSystem#0x030F` | `0x01410027` | - |
| `0x85020256` | `0x0502` | `0x0256` | `(ref)` | `GuildSystem#0x0310` | `0x01410026` | `0x1000055F` |
| `0x85020258` | `0x0502` | `0x0258` | `(i64)` | `GuildSystem#0x0311` | `0x01410025` | `0x10000560` |
| `0x8502025C` | `0x0502` | `0x025C` | `(ref)` | `GuildSystem#0x0315` | `0x01410028` | `0x10000561` |
| `0x8502025F` | `0x0502` | `0x025F` | `(i32, i64)` | `GuildSystem#0x031D` | - | - |
| `0x850A0015` | `0x050A` | `0x0015` | `(i32, i64)` | `0x0170#0x0101` | - | - |
| `0x850A0037` | `0x050A` | `0x0037` | `(ref)` | `0x0AF4#0x001A` | `0x00E70001`, `0x0144000F` | `0x100004D9` |
| `0x850B0004` | `0x050B` | `0x0004` | `(i32, i64)` | `0x0171#0x0008` | `0x00340004`, `0x00340003` | - |
| `0x850B0005` | `0x050B` | `0x0005` | `` | `0x0171#0x0007` | `0x01450001` | `0x10000162` |
| `0x850B0006` | `0x050B` | `0x0006` | `` | `0x0171#0x000C` | `0x01450002` | `0x100000AC` |
| `0x850B000A` | `0x050B` | `0x000A` | `(i32, i64)` | `0x0171#0x0009` | `0x00340004`, `0x00340003` | - |
| `0x850C0011` | `0x050C` | `0x0011` | `(ref, i64)` | `0x1021#0x0032` | `0x01460006` | `0x100000CD` |
| `0x850C0017` | `0x050C` | `0x0017` | `(i32, i64)` | `0x1021#0x002F` | `0x01460003` | `0x10000018` |
| `0x850C0020` | `0x050C` | `0x0020` | `(i32, i64)` | `0x1021#0x0010` | `0x01460007` | `0x1000017C` |
| `0x850C005E` | `0x050C` | `0x005E` | `(i32, i64)` | `0x1021#0x001C` | `0x01460005` | `0x100001C4` |
| `0x850C007B` | `0x050C` | `0x007B` | `(i32, i64)` | `0x1021#0x0016` | `0x01460005` | `0x100001C4` |
| `0x850C007D` | `0x050C` | `0x007D` | `(ref, i64)` | `0x1021#0x0039` | `0x01460006` | `0x100000CD` |
| `0x850C008A` | `0x050C` | `0x008A` | `(i32, i64)` | `0x1021#0x0021` | `0x01460007` | `0x1000017C` |
| `0x850C00A3` | `0x050C` | `0x00A3` | `(ref, i64)` | `0x1021#0x004D` | - | - |
| `0x850C00A7` | `0x050C` | `0x00A7` | `(ref, i64)` | `0x1021#0x004E` | - | - |
| `0x850C00B4` | `0x050C` | `0x00B4` | `(i64)` | `0x1021#0x0054` | `0x0146000F` | `0x10000508` |
| `0x850C00B6` | `0x050C` | `0x00B6` | `(ref)` | `0x1021#0x0052` | `0x01460010` | `0x10000507` |
| `0x850C00BA` | `0x050C` | `0x00BA` | `(ref)` | `0x1021#0x0056` | `0x01460010` | `0x10000507` |
| `0x850C00BB` | `0x050C` | `0x00BB` | `(i64)` | `0x1021#0x0053` | `0x0146000F` | `0x10000508` |
| `0x850C00BD` | `0x050C` | `0x00BD` | `(ref)` | `0x1021#0x0058` | `0x01460010`, `0x102F0004`, `0x102F0001`, `0x102F0002`, `0x102F0003` (+5) | `0x10000507` |
| `0x850C00E4` | `0x050C` | `0x00E4` | `(i64)` | `0x1021#0x0075` | `0x01460013` | `0x10000514` |
| `0x850C00E6` | `0x050C` | `0x00E6` | `(ref)` | `0x1021#0x007B` | `0x01460018`, `0x01CF0016`, `0x01CF0001`, `0x01CF0003`, `0x01CF000F` (+7) | `0x10000515` |
| `0x850C00EA` | `0x050C` | `0x00EA` | `(i64)` | `0x1021#0x0065` | `0x01460014` | `0x10000513` |
| `0x850C00ED` | `0x050C` | `0x00ED` | `(ref)` | `0x1021#0x008D` | `0x01460018` | `0x10000515` |
| `0x850C00F4` | `0x050C` | `0x00F4` | `(i64)` | `0x1021#0x007F` | `0x01460014` | `0x10000513` |
| `0x850C00F7` | `0x050C` | `0x00F7` | `(i64)` | `0x1021#0x0066` | `0x01460017` | `0x10000517` |
| `0x850C00FE` | `0x050C` | `0x00FE` | `(i64)` | `0x1021#0x008B` | `0x01460013` | `0x10000514` |
| `0x850C00FF` | `0x050C` | `0x00FF` | `(ref)` | `0x1021#0x0061` | `0x01460018` | `0x10000515` |
| `0x850C0102` | `0x050C` | `0x0102` | `(ref)` | `0x1021#0x0062` | `0x01460016`, `0x01CF0016`, `0x01CF0001`, `0x01CF0003`, `0x01CF000F` (+7) | `0x10000518` |
| `0x850C0103` | `0x050C` | `0x0103` | `(i64)` | `0x1021#0x0070` | `0x01460015` | `0x10000516` |
| `0x850C0106` | `0x050C` | `0x0106` | `(ref)` | `0x1021#0x008E` | `0x01460016` | `0x10000518` |
| `0x850C010F` | `0x050C` | `0x010F` | `(ref)` | `0x1021#0x0077` | `0x01460016` | `0x10000518` |
| `0x850C0112` | `0x050C` | `0x0112` | `(i64)` | `0x1021#0x0071` | `0x01460017` | `0x10000517` |
| `0x850C0115` | `0x050C` | `0x0115` | `(i64)` | `0x1021#0x0082` | `0x01460015` | `0x10000516` |
| `0x850E0003` | `0x050E` | `0x0003` | `(i32, i32)` | `0x0110#0x0005` | `0x01490005` | `0x10000203` |
| `0x850E000B` | `0x050E` | `0x000B` | `(ref)` | `0x0110#0x0003` | `0x01490006` | `0x10000161` |
| `0x850F000F` | `0x050F` | `0x000F` | `(i64)` | `0x0745#0x0001` | `0x07460003` | `0x10000235` |
| `0x850F0010` | `0x050F` | `0x0010` | `(i32, ref, i64, i64)` | `0x0745#0x0002` | `0x0030000B`, `0x07460002` | `0x10000233` |
| `0x850F0012` | `0x050F` | `0x0012` | `(i32, ref, i64, i64)` | `0x0745#0x0003` | `0x07460001` | `0x10000234` |
| `0x85100007` | `0x0510` | `0x0007` | `` | `0x018D#0x0005` | `0x00270006`, `0x00330001`, `0x00270025`, `0x00070002` | - |
| `0x85100015` | `0x0510` | `0x0015` | `` | `0x018D#0x000D` | `0x00270006`, `0x00330001`, `0x00270025`, `0x00070002` | - |
| `0x85120007` | `0x0512` | `0x0007` | `(i64)` | `0x0112#0x0005` | `0x0030000B` | - |
| `0x85120009` | `0x0512` | `0x0009` | `(ref, i32, i64)` | `0x0112#0x0006` | `0x01670012` | `0x100001C2` |
| `0x85120019` | `0x0512` | `0x0019` | `(ref, i64)` | `0x0112#0x0008` | - | - |
| `0x8512001D` | `0x0512` | `0x001D` | `(i32, f32, f64, ref, i32, i64)` | `0x0112#0x0007` | `0x01670011` | `0x1000004B` |
| `0x8512001E` | `0x0512` | `0x001E` | `(i64)` | `0x0112#0x0004` | `0x0030000B` | - |
| `0x8512002B` | `0x0512` | `0x002B` | `(ref, i32, i64, i64)` | `0x0112#0x000A` | `0x00C7000C`, `0x00C70009`, `0x00330001`, `0x00A6001F`, `0x00A6002B` (+6) | - |
| `0x8512003D` | `0x0512` | `0x003D` | `(ref)` | `ItemMungingSystem::AddMungingPropertiesForItemDIDForLevel` | `0x0144000B` | `0x1000045B` |
| `0x85130004` | `0x0513` | `0x0004` | `(i32, i32, i64)` | `CMEOInventorySystem::RecvCEvt_HandleContainerStartViewingResult` | `0x0030000B`, `0x014B0001`, `0x013D0001` | `0x1000014D`; `0x10000102` |
| `0x85130014` | `0x0513` | `0x0014` | `(i32, i32, i64)` | `CMEOInventorySystem::RecvCEvt_HandleContainerStopViewingResult` | `0x0030000B`, `0x014B0002`, `0x013D0001` | `0x10000009`; `0x10000102` |
| `0x85140025` | `0x0514` | `0x0025` | `(i64)` | `0x0987#0x0002` | `0x01480006` | `0x100000BA` |
| `0x85140026` | `0x0514` | `0x0026` | `(i64, i64)` | `0x0987#0x0003` | `0x01480004` | `0x100000AA` |
| `0x85140035` | `0x0514` | `0x0035` | `` | `0x0987#0x0008` | `0x01480001` | `0x1000015F` |
| `0x85140036` | `0x0514` | `0x0036` | `(i64)` | `0x0987#0x0001` | `0x01480002` | `0x1000012D` |
| `0x8514007F` | `0x0514` | `0x007F` | `(ref)` | `0x0987#0x0005` | `0x01B5001C`, `0x01B50048`, `0x01480009` | `0x1000017B` |
| `0x85140081` | `0x0514` | `0x0081` | `(i64)` | `0x0987#0x0009` | `0x01480008` | `0x10000005` |
| `0x85140083` | `0x0514` | `0x0083` | `` | `0x0987#0x0004` | `0x0148000A` | `0x10000191` |
| `0x85140086` | `0x0514` | `0x0086` | `(i32)` | `0x0987#0x0007` | `0x0148000A` | `0x10000191` |
| `0x85140088` | `0x0514` | `0x0088` | `(ref)` | `0x0987#0x0006` | `0x01480007` | `0x10000192` |
| `0x85150004` | `0x0515` | `0x0004` | `(ref, i32)` | `0x01B6#0x0007` | `0x01490002` | `0x100000D5` |
| `0x85190005` | `0x0519` | `0x0005` | `` | `0x0116#0x0006` | `0x01500001` | `0x1000018F` |
| `0x85190008` | `0x0519` | `0x0008` | `` | `0x0116#0x0007` | `0x01500002` | `0x10000143` |
| `0x8519000F` | `0x0519` | `0x000F` | `(i32, f64, i32, i32, i32, i32)` | `0x0116#0x000E` | `0x00E70001`, `0x07DE0004` | - |
| `0x85190011` | `0x0519` | `0x0011` | `(i32, f64, i32)` | `0x0116#0x000C` | `0x00E70001`, `0x07DE0005` | - |
| `0x85190016` | `0x0519` | `0x0016` | `(ref, ref)` | `0x0116#0x0024` | `0x00C7001A`, `0x00C7000C`, `0x00C70009` | - |
| `0x8519001C` | `0x0519` | `0x001C` | `(f64, f64)` | `0x0116#0x0023` | `0x07DE0002` | - |
| `0x85190021` | `0x0519` | `0x0021` | `(ref)` | `0x0116#0x0026` | `0x00E70001`, `0x07DE000A` | - |
| `0x85190027` | `0x0519` | `0x0027` | `` | `0x0116#0x002A` | `0x00E70001`, `0x07DE0006` | - |
| `0x851B0005` | `0x051B` | `0x0005` | `(i32)` | `0x0119#0x0010` | `0x01530003` | `0x1000018C` |
| `0x851B0007` | `0x051B` | `0x0007` | `(ref, i32)` | `0x0119#0x000F` | `0x01530001` | `0x10000078` |
| `0x851B000C` | `0x051B` | `0x000C` | `(i32, ref, i64)` | `0x0119#0x000A` | `0x00C7000C`, `0x00C70009`, `0x00330001`, `0x00A6001F`, `0x00A6002B` (+5) | - |
| `0x851B0010` | `0x051B` | `0x0010` | `(i64)` | `0x0119#0x000B` | `0x01510002` | `0x1000002D` |
| `0x851B001C` | `0x051B` | `0x001C` | `` | `0x0119#0x0009` | `0x01510001` | `0x1000012A` |
| `0x851B0020` | `0x051B` | `0x0020` | `(ref)` | `0x0119#0x0007` | `0x01530002` | `0x1000018D` |
| `0x851B002A` | `0x051B` | `0x002A` | `` | `0x0119#0x0013` | `0x01530004` | `0x1000021B` |
| `0x851C0003` | `0x051C` | `0x0003` | `(f64, i32)` | `0x011A#0x000E` | `0x00170002` | - |
| `0x851C0006` | `0x051C` | `0x0006` | `(i64)` | `0x011A#0x0005` | `0x01540001` | `0x10000060` |
| `0x851C0007` | `0x051C` | `0x0007` | `(i32)` | `0x011A#0x000A` | `0x01540002` | `0x100001E8` |
| `0x851D0014` | `0x051D` | `0x0014` | `(i32, f32, i32, i32)` | - | - | - |
| `0x851E0003` | `0x051E` | `0x0003` | `` | - | - | - |
| `0x851E0007` | `0x051E` | `0x0007` | `(i32, i32, i32)` | - | - | - |
| `0x851E000F` | `0x051E` | `0x000F` | `(i32, ref)` | - | - | - |
| `0x851E0012` | `0x051E` | `0x0012` | `(i64)` | - | - | - |
| `0x851E0013` | `0x051E` | `0x0013` | `(ref, ref, ref, ref, ref, ref, ref, ref, ref, ref, ref, ref, ref)` | - | - | - |
| `0x851E0017` | `0x051E` | `0x0017` | `(i32)` | - | - | - |
| `0x851E002C` | `0x051E` | `0x002C` | `(ref, i32)` | - | - | - |
| `0x851E004A` | `0x051E` | `0x004A` | `(i64, i32)` | - | - | - |
| `0x851F0003` | `0x051F` | `0x0003` | `` | `0x01C1#0x0051` | `0x00270006`, `0x00270025`, `0x00070002` | - |
| `0x851F0009` | `0x051F` | `0x0009` | `(ref, i64)` | `0x01C1#0x005A` | `0x0157000B` | `0x100001FF` |
| `0x851F000C` | `0x051F` | `0x000C` | `(ref)` | `0x01C1#0x0058` | `0x01570001` | `0x10000210` |
| `0x851F000F` | `0x051F` | `0x000F` | `(i32)` | `0x01C1#0x0057` | `0x00C7000C`, `0x00C70003`, `0x00330001`, `0x00A6001F`, `0x00A6002B` (+3) | - |
| `0x851F0027` | `0x051F` | `0x0027` | `` | `0x01C1#0x0052` | `0x01570002` | `0x10000125` |
| `0x851F0028` | `0x051F` | `0x0028` | `` | `0x01C1#0x0050` | `0x00330001`, `0x00270006`, `0x00270025`, `0x00070002` | - |
| `0x851F003F` | `0x051F` | `0x003F` | `(ref)` | `0x01C1#0x004E` | `0x01B70002`, `0x01570006` | `0x10000043` |
| `0x851F0050` | `0x051F` | `0x0050` | `(ref)` | `0x01C1#0x0054` | `0x00330001`, `0x00A6001F`, `0x00A6002B`, `0x00A60015`, `0x00070001` | - |
| `0x851F0057` | `0x051F` | `0x0057` | `(ref)` | `0x01C1#0x0059` | `0x01B70002`, `0x01570005` | `0x100001A7` |
| `0x851F00A8` | `0x051F` | `0x00A8` | `(ref)` | `0x01C1#0x0053` | `0x01B70002`, `0x01570004` | `0x1000013D` |
| `0x851F00B4` | `0x051F` | `0x00B4` | `(f32)` | `0x01C1#0x0056` | `0x01570009` | `0x100001F6` |
| `0x851F00CD` | `0x051F` | `0x00CD` | `(ref)` | `0x011C#0x000B` | `0x0157000C` | `0x1000022A` |
| `0x851F00CF` | `0x051F` | `0x00CF` | `(i32, i32)` | `0x011C#0x000C` | `0x0157000D` | `0x1000022B` |
| `0x851F00D8` | `0x051F` | `0x00D8` | `(i32)` | `0x01C1#0x0065` | `0x0157000E` | `0x10000244` |
| `0x851F00D9` | `0x051F` | `0x00D9` | `` | `0x01C1#0x0064` | `0x0157000F` | `0x10000245` |
| `0x851F00F6` | `0x051F` | `0x00F6` | `(ref)` | `0x01C1#0x0073` | `0x01590007` | `0x100000D6` |
| `0x851F00F9` | `0x051F` | `0x00F9` | `(ref)` | `0x01C1#0x0076` | `0x00B80004`, `0x00B8000D`, `0x00B9000D` | - |
| `0x851F013E` | `0x051F` | `0x013E` | `(ref)` | `0x01C1#0x0096` | `0x01B70002`, `0x01570003` | `0x1000010D` |
| `0x85200002` | `0x0520` | `0x0002` | `(f32, f64)` | `0x011D#0x0008` | `0x01580003` | `0x10000104` |
| `0x85200004` | `0x0520` | `0x0004` | `(i32, ref)` | `0x011D#0x0007` | `0x00E70001`, `0x01580002` | `0x1000009C` |
| `0x85200007` | `0x0520` | `0x0007` | `(ref)` | `0x011D#0x0009` | `0x00E70001`, `0x01580001` | `0x100001DC` |
| `0x8521000E` | `0x0521` | `0x000E` | `(i64)` | `CQuestSystem#0x0028` | `0x01590009` | `0x1000015C` |
| `0x85210010` | `0x0521` | `0x0010` | `(i32, i32, i32)` | `CQuestSystem#0x0040` | `0x00B6002B`, `0x00B70008`, `0x00B70003`, `0x00B70001` | - |
| `0x8521001F` | `0x0521` | `0x001F` | `(i32, i32, i32)` | `CQuestSystem#0x0036` | - | - |
| `0x85210020` | `0x0521` | `0x0020` | `(i32)` | `CQuestSystem#0x003D` | `0x00DD0008`, `0x00230009`, `0x0023000A`, `0x00240006`, `0x00240003` (+2) | - |
| `0x85210027` | `0x0521` | `0x0027` | `(i64)` | `CQuestSystem#0x0029` | `0x0159000A` | `0x100000A8` |
| `0x85210041` | `0x0521` | `0x0041` | `(i64, i32)` | `CQuestSystem#0x0033` | `0x0030000B` | - |
| `0x85210047` | `0x0521` | `0x0047` | `(i32, i32, i32)` | `CQuestSystem#0x0030` | `0x0159000F` | `0x10000301` |
| `0x85210051` | `0x0521` | `0x0051` | `(ref, i32, i32)` | `CQuestSystem#0x003A` | - | - |
| `0x85210052` | `0x0521` | `0x0052` | `(ref, i32)` | `CQuestSystem#0x002F` | `0x00230001`, `0x00190006`, `0x00B6002B`, `0x00B70008`, `0x00B70003` (+1) | - |
| `0x85210053` | `0x0521` | `0x0053` | `(i32, i32)` | `CQuestSystem#0x002E` | `0x01810004`, `0x01810003` | - |
| `0x8521005C` | `0x0521` | `0x005C` | `(ref, ref, ref, ref, ref, ref)` | `CQuestSystem#0x003E` | `0x01590008`, `0x00E70001` | `0x100001B5` |
| `0x8521005E` | `0x0521` | `0x005E` | `` | `CQuestSystem#0x0038` | `0x00070003` | - |
| `0x85210063` | `0x0521` | `0x0063` | `(i32)` | `CQuestSystem#0x0034` | `0x01810004`, `0x01810003` | - |
| `0x85210067` | `0x0521` | `0x0067` | `(i32, i32, i32)` | `CQuestSystem#0x0037` | `0x0159000F` | `0x10000301` |
| `0x85210075` | `0x0521` | `0x0075` | `(ref, i32, i32)` | `CQuestSystem#0x002A` | - | - |
| `0x85210094` | `0x0521` | `0x0094` | `(i32)` | `CQuestSystem#0x004A` | `0x00230003`, `0x00190024` | - |
| `0x8521009C` | `0x0521` | `0x009C` | `` | `CQuestSystem#0x0053` | `0x0023000A`, `0x00240006`, `0x00240002`, `0x00B6002B`, `0x00B70008` (+3) | - |
| `0x852100AB` | `0x0521` | `0x00AB` | `(i64)` | `CQuestSystem#0x0055` | `0x00E70001`, `0x0159000D`, `0x0159000C` | `0x100002A5`; `0x10000288` |
| `0x852100B5` | `0x0521` | `0x00B5` | `(ref, i64)` | `CQuestSystem#0x005A` | `0x00E70001`, `0x0159000D`, `0x0159000B` | `0x100002A5`; `0x10000287` |
| `0x852100BD` | `0x0521` | `0x00BD` | `` | `CQuestSystem#0x0060` | `0x00750009`, `0x0075000A`, `0x00760006`, `0x00760002`, `0x00760003` (+16) | - |
| `0x852100C1` | `0x0521` | `0x00C1` | `(i32)` | `CQuestSystem#0x0062` | `0x01590049` | `0x100004F7` |
| `0x852100CA` | `0x0521` | `0x00CA` | `(i64, i32)` | `CQuestSystem#0x0069` | `0x0030000B` | - |
| `0x852100CE` | `0x0521` | `0x00CE` | `(i32, i32, i32)` | `CQuestSystem#0x007A` | `0x00230004`, `0x000B0004`, `0x000B0002`, `0x00230002` | - |
| `0x852100CF` | `0x0521` | `0x00CF` | `(i32, i32, i32)` | `CQuestSystem#0x007B` | `0x00230004`, `0x000B0004`, `0x000B0003`, `0x000B0007`, `0x00230003` (+3) | - |
| `0x852100D2` | `0x0521` | `0x00D2` | `(ref)` | `CQuestSystem#0x007C` | `0x0023000A`, `0x00240006`, `0x00240002`, `0x00240003`, `0x000B000A` (+9) | - |
| `0x852100D5` | `0x0521` | `0x00D5` | `` | `CQuestSystem#0x007F` | `0x0CBD0007`, `0x0CBD000A`, `0x0CBE0005`, `0x0CBE0002`, `0x0CBE0003` (+1) | - |
| `0x852100DF` | `0x0521` | `0x00DF` | `` | `CQuestSystem#0x008D` | `0x0159002F` | `0x100003AD` |
| `0x852100F7` | `0x0521` | `0x00F7` | `(i64)` | `CQuestSystem#0x00AE` | `0x01A70001` | - |
| `0x852100F8` | `0x0521` | `0x00F8` | `(i64)` | `CQuestSystem#0x00B0` | `0x01590005` | `0x1000012C` |
| `0x852100FA` | `0x0521` | `0x00FA` | `(i64)` | `0x0C0E#0x000C` | `0x01590044` | `0x100003D9` |
| `0x852100FC` | `0x0521` | `0x00FC` | `` | `0x0C0E#0x000F` | `0x01590044` | `0x100003D9` |
| `0x85210103` | `0x0521` | `0x0103` | `(ref)` | `CQuestSystem#0x00BC` | `0x01590046` | `0x10000453` |
| `0x85210106` | `0x0521` | `0x0106` | `(i32)` | `CQuestSystem#0x00BD` | `0x01590046` | `0x10000453` |
| `0x85210115` | `0x0521` | `0x0115` | `(i32)` | `CQuestSystem#0x00C8` | `0x01A70001` | - |
| `0x85210118` | `0x0521` | `0x0118` | `(ref)` | `CQuestSystem#0x00C7` | `0x01A70001` | - |
| `0x85210126` | `0x0521` | `0x0126` | `(ref, i32, i32)` | `CQuestSystem#0x00E9` | - | - |
| `0x85220002` | `0x0522` | `0x0002` | `(ref, i32, i64)` | `0x011E#0x0002` | `0x015A0002` | `0x1000001D` |
| `0x8522000A` | `0x0522` | `0x000A` | `(i32, i64)` | `0x011E#0x0003` | `0x015A0001` | `0x10000179` |
| `0x85230006` | `0x0523` | `0x0006` | `(ref, i64)` | `0x0123#0x0012` | `0x015C0003` | `0x10000147` |
| `0x85230009` | `0x0523` | `0x0009` | `(i64)` | `0x0123#0x0011` | `0x015C0004` | `0x10000202` |
| `0x85230013` | `0x0523` | `0x0013` | `(ref)` | `0x0123#0x000B` | `0x015C0001`, `0x00E70001` | `0x100001B3` |
| `0x8523001C` | `0x0523` | `0x001C` | `(i64)` | `0x0123#0x0010` | `0x015C0005` | `0x10000160` |
| `0x8523003B` | `0x0523` | `0x003B` | `(ref)` | `0x0123#0x001E` | `0x074A0001` | `0x10000239` |
| `0x85230040` | `0x0523` | `0x0040` | `(i32, i64)` | `0x0123#0x0022` | `0x015C0008`, `0x00E70001` | `0x1000023C` |
| `0x85240001` | `0x0524` | `0x0001` | `(i32)` | `0x06DB#0x000A` | - | - |
| `0x85240003` | `0x0524` | `0x0003` | `(i32, ref, i32)` | `0x06DB#0x0006` | `0x00C70009`, `0x00C7000C` | - |
| `0x85240009` | `0x0524` | `0x0009` | `` | `0x06DB#0x0004` | `0x00330001`, `0x00270006`, `0x00270025`, `0x00070002` | - |
| `0x85240018` | `0x0524` | `0x0018` | `(ref, i32)` | `0x06DB#0x000E` | - | - |
| `0x85270006` | `0x0527` | `0x0006` | `(ref)` | `0x037F#0x0006` | - | - |
| `0x8528000A` | `0x0528` | `0x000A` | `(i32, i32, i32)` | `SkillSystem#0x00B4` | `0x015E0004` | `0x1000013C` |
| `0x8528000D` | `0x0528` | `0x000D` | `(i32)` | `SkillSystem#0x00B1` | `0x015E0010` | `0x100000E9` |
| `0x8528000E` | `0x0528` | `0x000E` | `(i32, i32, i64)` | `SkillSystem#0x00AE` | `0x015E0004` | `0x1000013C` |
| `0x8528001A` | `0x0528` | `0x001A` | `(i32)` | `SkillSystem#0x00FC` | `0x015E0014` | `0x100003AC` |
| `0x852A0004` | `0x052A` | `0x0004` | `(i32, ref, i32, i32, ref, i64)` | `0x06DD#0x0007` | `0x00C70019`, `0x00C7000C` | - |
| `0x852B000F` | `0x052B` | `0x000F` | `(i64)` | `0x0124#0x000A` | `0x0E0E000A` | `0x10000463` |
| `0x852B0013` | `0x052B` | `0x0013` | `(i64)` | `0x0124#0x000C` | `0x0E0E000B` | `0x10000462` |
| `0x852C0002` | `0x052C` | `0x0002` | `(i64)` | `CTextItemSystem#0x0008` | `0x01600004` | `0x1000010A` |
| `0x852C0004` | `0x052C` | `0x0004` | `(i32, ref, i64)` | `CTextItemSystem::RecvCEvt_UpdateCustomContents` | `0x00E70003`, `0x01600001`, `0x0030000B`, `0x0CBA000D` | `0x100001F4` |
| `0x852C0009` | `0x052C` | `0x0009` | `(i64)` | `CTextItemSystem#0x0005` | `0x01600004` | `0x1000010A` |
| `0x852C000A` | `0x052C` | `0x000A` | `(i64)` | `CTextItemSystem::RecvCEvt_EditTextItem` | `0x0030000A`, `0x01600002`, `0x0030000B`, `0x0CBA000D` | `0x100001F3` |
| `0x852C000D` | `0x052C` | `0x000D` | `(i64)` | `CTextItemSystem::RecvCEvt_ReadTextItem` | `0x0030000A`, `0x01600003`, `0x0030000B`, `0x0CBA000D` | `0x10000171` |
| `0x852D0003` | `0x052D` | `0x0003` | `` | `0x017D#0x000E` | `0x01610002` | `0x100001E7` |
| `0x852D0007` | `0x052D` | `0x0007` | `(i32)` | `0x017D#0x000D` | `0x01610001` | `0x10000079` |
| `0x852E0005` | `0x052E` | `0x0005` | `(f64, i32, i32)` | `TitleSystem::RecvCEvt_TitleAwarded` | `0x0135001D`, `0x0CBA000D` | `0x1000015B` |
| `0x852E0013` | `0x052E` | `0x0013` | `(i32)` | `CErrorSystem::HandleClientError` | `0x0135001E`, `0x013D0001` | `0x10000049`; `0x10000102` |
| `0x852E0016` | `0x052E` | `0x0016` | `` | `TitleSystem#0x0027` | `0x0135001F` | `0x100001E5` |
| `0x852F0003` | `0x052F` | `0x0003` | `(i32)` | `0x06DC#0x0001` | `0x015E000F` | `0x1000019A` |
| `0x852F0004` | `0x052F` | `0x0004` | `(i32)` | `0x06DC#0x0003` | `0x015E000E` | `0x1000019B` |
| `0x852F0021` | `0x052F` | `0x0021` | `(i64, i32)` | `0x06DC#0x0006` | `0x0030000B` | - |
| `0x852F0024` | `0x052F` | `0x0024` | `(i64, i32)` | `0x06DC#0x0007` | `0x0030000B` | - |
| `0x85300006` | `0x0530` | `0x0006` | `(i32, ref, i64)` | `CTrackingSystem#0x0004` | `0x01620003` | `0x100001E9` |
| `0x8530000C` | `0x0530` | `0x000C` | `(i32, ref, i32, ref, i64)` | `CTrackingSystem::StartTracking` | `0x01620001` | `0x10000200` |
| `0x85300011` | `0x0530` | `0x0011` | `(i32, i32, i64)` | `CTrackingSystem#0x0001` | `0x01620002` | `0x100000AB` |
| `0x85300019` | `0x0530` | `0x0019` | `(ref, i64)` | `CTrackingSystem#0x000A` | `0x01620005` | `0x1000026A` |
| `0x8530001E` | `0x0530` | `0x001E` | `(i32, i64)` | `CTrackingSystem::UpdateTrackedEntity` | - | - |
| `0x8531000B` | `0x0531` | `0x000B` | `(i64, i64)` | `0x017F#0x0091` | - | - |
| `0x8531000E` | `0x0531` | `0x000E` | `(ref, i64)` | `0x017F#0x0081` | - | - |
| `0x8531000F` | `0x0531` | `0x000F` | `(ref, i64, i64)` | `0x017F#0x008B` | `0x01630001` | `0x10000170` |
| `0x85310016` | `0x0531` | `0x0016` | `(i64, i64)` | `0x017F#0x008F` | - | - |
| `0x85310035` | `0x0531` | `0x0035` | `(i32, i64)` | `0x017F#0x007D` | - | - |
| `0x85310046` | `0x0531` | `0x0046` | `` | `0x017F#0x007B` | `0x01630002` | `0x100001E2` |
| `0x8531004C` | `0x0531` | `0x004C` | `(i64)` | `0x017F#0x0090` | - | - |
| `0x8531004E` | `0x0531` | `0x004E` | `(i64)` | `0x017F#0x007E` | - | - |
| `0x85310052` | `0x0531` | `0x0052` | `(i32)` | `0x017F#0x008D` | - | - |
| `0x8531005F` | `0x0531` | `0x005F` | `` | `0x017F#0x0088` | - | - |
| `0x85310061` | `0x0531` | `0x0061` | `(i32, i64)` | `0x017F#0x008C` | - | - |
| `0x85310068` | `0x0531` | `0x0068` | `(i64)` | `0x017F#0x0083` | - | - |
| `0x85320002` | `0x0532` | `0x0002` | `` | `CTraitSystem::RecvCEvt_UnslotAllTraitsRequest` | `0x01640013`, `0x0CBA000D`, `0x01310001` | `0x100002E6`; `0x10000214` |
| `0x85320003` | `0x0532` | `0x0003` | `(i32)` | `CTraitSystem::RecvCEvt_UnslotTraitByDIDRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x85320009` | `0x0532` | `0x0009` | `(i32)` | `CTraitSystem::RecvCEvt_UnslotAllTraitsOfNatureRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x8532000B` | `0x0532` | `0x000B` | `(ref, i32)` | `CTraitSystem::RecvCEvt_ForcePlaceTraitRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x8532000F` | `0x0532` | `0x000F` | `(i32)` | `CTraitSystem#0x0018` | `0x01640006`, `0x01640013` | `0x1000004C`; `0x100002E6` |
| `0x85320010` | `0x0532` | `0x0010` | `(i32, i32)` | `CTraitSystem#0x001F` | `0x00A9000E`, `0x00270015`, `0x00B80002`, `0x00B90002`, `0x00C7000C` (+2) | - |
| `0x85320013` | `0x0532` | `0x0013` | `(ref, i32)` | `CTraitSystem::RecvCEvt_SlotTraitRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x85320019` | `0x0532` | `0x0019` | `(i32)` | `CTraitSystem#0x000C` | `0x01640007`, `0x01640013` | `0x100000BB`; `0x100002E6` |
| `0x8532001A` | `0x0532` | `0x001A` | `` | `CTraitSystem#0x0016` | `0x01640013` | `0x100002E6` |
| `0x8532001D` | `0x0532` | `0x001D` | `(i32, i32)` | `CTraitSystem::RecvCEvt_AddEarnedTraitRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x85320020` | `0x0532` | `0x0020` | `(i32)` | `CTraitSystem::RecvCEvt_RemoveEarnedTraitRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x8532002B` | `0x0532` | `0x002B` | `(i32)` | `CTraitSystem#0x0025` | `0x01640013` | `0x100002E6` |
| `0x8532002E` | `0x0532` | `0x002E` | `(i32)` | `CTraitSystem#0x0028` | `0x01640013` | `0x100002E6` |
| `0x85320030` | `0x0532` | `0x0030` | `` | `CTraitSystem#0x0034` | `0x01640013` | `0x100002E6` |
| `0x8532004D` | `0x0532` | `0x004D` | `(i32, i32)` | `CTraitSystem::RecvCEvt_SetSuppressionToggleState` | `0x0CBA000D` | - |
| `0x85320054` | `0x0532` | `0x0054` | `(ref)` | `CTraitSystem::RecvCEvt_RemoveEarnedTraitsRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x85320056` | `0x0532` | `0x0056` | `(i32, ref)` | `CTraitSystem::RecvCEvt_AddEarnedTraitsRequest` | `0x01640013`, `0x0CBA000D` | `0x100002E6` |
| `0x8532005E` | `0x0532` | `0x005E` | `` | `CTraitSystem#0x0081` | `0x00A9000E`, `0x00270013`, `0x0019002E`, `0x001A0008`, `0x001A0003` (+1) | - |
| `0x85330002` | `0x0533` | `0x0002` | `(i64)` | `CTravelSystem#0x0015` | `0x01650001` | `0x10000165` |
| `0x85330006` | `0x0533` | `0x0006` | `(ref)` | `CTravelSystem#0x000B` | `0x01650003` | `0x100001E6` |
| `0x8533000B` | `0x0533` | `0x000B` | `(i32, i32)` | `CTravelSystem#0x000D` | `0x01650003` | `0x100001E6` |
| `0x8533001B` | `0x0533` | `0x001B` | `(i32, i64)` | `CTravelSystem#0x0017` | `0x0030000B` | - |
| `0x85330025` | `0x0533` | `0x0025` | `(ref, i32)` | - | - | - |
| `0x85330026` | `0x0533` | `0x0026` | `(i32)` | - | - | - |
| `0x85340002` | `0x0534` | `0x0002` | `(ref)` | `0x0126#0x0007` | `0x00390001` | - |
| `0x85340004` | `0x0534` | `0x0004` | `(i32, i32)` | `0x0126#0x0008` | `0x019F001D`, `0x01660001` | `0x10000195` |
| `0x8535000C` | `0x0535` | `0x000C` | `(ref)` | `CUsageSystem#0x000F` | - | - |
| `0x85350027` | `0x0535` | `0x0027` | `(i32, i64)` | `CUsageSystem#0x000E` | - | - |
| `0x85360004` | `0x0536` | `0x0004` | `(i64)` | `VendorSystem#0x004B` | `0x01680001` | `0x1000000E` |
| `0x85370003` | `0x0537` | `0x0003` | `(ref)` | `VitalSystem#0x004B` | - | - |
| `0x871A0016` | `0x071A` | `0x0016` | `(ref, ref, ref)` | `0x071B#0x000D` | `0x07420001` | `0x10000232` |
| `0x87280005` | `0x0728` | `0x0005` | `(i32, i32)` | `0x0729#0x0005` | `0x08E80001` | `0x100002C5` |
| `0x87280006` | `0x0728` | `0x0006` | `(i32, i32)` | `0x0729#0x0001` | `0x08E80001` | `0x100002C5` |
| `0x87280008` | `0x0728` | `0x0008` | `(ref)` | `0x0729#0x0003` | `0x08E80002` | `0x100002DA` |
| `0x877B0007` | `0x077B` | `0x0007` | `(i32)` | `0x0769#0x000D` | `0x07980001`, `0x00E70001` | `0x10000252` |
| `0x877F0019` | `0x077F` | `0x0019` | `(i32)` | `CErrorSystem::HandleClientError` | `0x013D0001` | `0x10000102` |
| `0x877F001E` | `0x077F` | `0x001E` | `(i32, i32)` | `0x0786#0x0007` | `0x0167001D` | `0x1000024A` |
| `0x877F0020` | `0x077F` | `0x0020` | `(i32, i32, i32)` | `0x0786#0x000D` | `0x0167001B` | `0x1000024B` |
| `0x877F0026` | `0x077F` | `0x0026` | `(i32, i32, i32, i64)` | `0x0786#0x000C` | `0x07890001` | `0x1000024D` |
| `0x877F0039` | `0x077F` | `0x0039` | `(i32, i32)` | `0x0786#0x0011` | `0x07890002` | `0x1000024F` |
| `0x878E0004` | `0x078E` | `0x0004` | `(i32)` | `0x0787#0x0003` | `0x07880002` | `0x1000024E` |
| `0x87930002` | `0x0793` | `0x0002` | `(i64)` | `0x0794#0x0001` | `0x07910001` | `0x10000251` |
| `0x87A70001` | `0x07A7` | `0x0001` | `(i32, i64)` | `0x07A6#0x0004` | `0x01430011` | `0x10000257` |
| `0x87A70029` | `0x07A7` | `0x0029` | `(i32, i64)` | `0x07A6#0x0009` | `0x01430012` | `0x10000259` |
| `0x87A7002D` | `0x07A7` | `0x002D` | `(ref, i32)` | `0x07A6#0x000C` | `0x01430013`, `0x07B10001`, `0x07B10007`, `0x07B10006` | `0x1000025A` |
| `0x87A7003D` | `0x07A7` | `0x003D` | `(i32, i32)` | `0x07A6#0x000E` | `0x01430014` | `0x1000025B` |
| `0x87A70042` | `0x07A7` | `0x0042` | `(ref, i32, i32)` | `0x07A6#0x0011` | `0x01430015` | `0x1000025C` |
| `0x87A7004A` | `0x07A7` | `0x004A` | `(i32, i32)` | `0x07A6#0x0014` | `0x01430016` | `0x1000025E` |
| `0x87A7004B` | `0x07A7` | `0x004B` | `(ref, i64)` | `0x07A6#0x0015` | `0x01430017` | `0x1000025D` |
| `0x87A70052` | `0x07A7` | `0x0052` | `(i32, i32, i32)` | `0x07A6#0x0019` | `0x01430018` | `0x1000025F` |
| `0x87A70068` | `0x07A7` | `0x0068` | `(ref, i32, i32)` | `0x07A6#0x001E` | `0x0143001A` | `0x10000261` |
| `0x87A700A5` | `0x07A7` | `0x00A5` | `(i64, i64)` | `0x07A6#0x003E` | `0x0143001F` | `0x1000026F` |
| `0x87A700BD` | `0x07A7` | `0x00BD` | `(ref, i32)` | `0x07A6#0x0042` | `0x01430020`, `0x07A90001`, `0x07A90006`, `0x07A90005` | `0x10000270` |
| `0x87A700FE` | `0x07A7` | `0x00FE` | `(ref, i32)` | `0x07A6#0x004A` | `0x01430022`, `0x08030003`, `0x0803000A`, `0x08030006`, `0x08030007` (+2) | `0x10000273` |
| `0x87A7011A` | `0x07A7` | `0x011A` | `(i32)` | `0x07A6#0x005A` | `0x01310001`, `0x01310002` | `0x10000214`; `0x10000077` |
| `0x87A70132` | `0x07A7` | `0x0132` | `` | `0x07A6#0x0067` | `0x01430025` | `0x1000027A` |
| `0x87A70163` | `0x07A7` | `0x0163` | `(i32)` | `0x07A6#0x0072` | `0x01310001`, `0x01310002` | `0x10000214`; `0x10000077` |
| `0x87A701B5` | `0x07A7` | `0x01B5` | `(i32)` | `0x07A6#0x0090` | `0x01310001`, `0x01310002` | `0x10000214`; `0x10000077` |
| `0x87A701B7` | `0x07A7` | `0x01B7` | `(i32)` | `0x07A6#0x008F` | `0x01310001`, `0x01310002` | `0x10000214`; `0x10000077` |
| `0x881B000A` | `0x081B` | `0x000A` | `(i32)` | `OutfitSystem#0x0023` | `0x08220001` | `0x1000027D` |
| `0x881B0010` | `0x081B` | `0x0010` | `(i32, i32, i32)` | `OutfitSystem#0x005E` | `0x08220002` | `0x10000299` |
| `0x883A0003` | `0x083A` | `0x0003` | `(i64)` | `0x0837#0x0008` | `0x08380001` | `0x1000027F` |
| `0x883A0005` | `0x083A` | `0x0005` | `` | `0x0837#0x000F` | `0x08380003` | `0x1000029B` |
| `0x8849000C` | `0x0849` | `0x000C` | `(ref)` | `0x084B#0x0005` | `0x08660001` | `0x10000286` |
| `0x88490011` | `0x0849` | `0x0011` | `(i64)` | `0x084B#0x0008` | `0x08660003` | `0x1000028C` |
| `0x88490013` | `0x0849` | `0x0013` | `(i64)` | `0x084B#0x0009` | `0x08660004` | `0x10000292` |
| `0x88490019` | `0x0849` | `0x0019` | `(i64)` | `0x084B#0x000C` | `0x08660009` | `0x100002C7` |
| `0x8849001F` | `0x0849` | `0x001F` | `(i32, i32)` | `0x084B#0x000E` | `0x08660006`, `0x08660015` | `0x100002A7`; `0x100002F1` |
| `0x88490022` | `0x0849` | `0x0022` | `(ref, ref, i32, i64)` | `0x084B#0x0010` | `0x0030000A`, `0x08660009` | `0x100002C7` |
| `0x88490025` | `0x0849` | `0x0025` | `(i32, i32, i64)` | `0x084B#0x0012` | `0x0030000A`, `0x08660008` | `0x100002AB` |
| `0x88490026` | `0x0849` | `0x0026` | `(i32, i32, i64)` | `0x084B#0x0011` | `0x0030000A`, `0x08660007` | `0x100002AC` |
| `0x8849002B` | `0x0849` | `0x002B` | `(i64)` | `0x084B#0x0015` | `0x0866000A` | `0x100002C8` |
| `0x8849002E` | `0x0849` | `0x002E` | `(i64)` | `0x084B#0x0016` | `0x0866000B` | `0x100002CA` |
| `0x88490031` | `0x0849` | `0x0031` | `(i32, i64)` | `0x084B#0x0018` | `0x0030000A`, `0x0866000F` | `0x100002E7` |
| `0x88490033` | `0x0849` | `0x0033` | `(i32, i64)` | `0x084B#0x0019` | `0x00E70003`, `0x01310001`, `0x08660009` | `0x10000214`; `0x100002C7` |
| `0x88490037` | `0x0849` | `0x0037` | `(i32, i32, i32, i64)` | `0x084B#0x001B` | `0x0030000A` | - |
| `0x8849003B` | `0x0849` | `0x003B` | `(i32, i64)` | `0x084B#0x001C` | `0x0030000A` | - |
| `0x88490040` | `0x0849` | `0x0040` | `(ref, ref, i64)` | `0x084B#0x001E` | `0x08660012` | `0x100002EE` |
| `0x88490042` | `0x0849` | `0x0042` | `(i32, ref, i32, i32, i32, i32, i64)` | `0x084B#0x001F` | `0x0030000A`, `0x08660011`, `0x0866001B` | `0x100002ED`; `0x100003AA` |
| `0x88490044` | `0x0849` | `0x0044` | `(i32, ref)` | `0x084B#0x0020` | `0x08660014` | `0x100002F0` |
| `0x88490045` | `0x0849` | `0x0045` | `(i64, i64)` | `0x084B#0x0021` | `0x08660009` | `0x100002C7` |
| `0x88490046` | `0x0849` | `0x0046` | `(i32, i64)` | `0x084B#0x0022` | `0x08660009` | `0x100002C7` |
| `0x88490051` | `0x0849` | `0x0051` | `(i64)` | `0x084B#0x0029` | `0x0030000A`, `0x0866001B` | `0x100003AA` |
| `0x88490052` | `0x0849` | `0x0052` | `(i64)` | `0x084B#0x0028` | - | - |
| `0x88490053` | `0x0849` | `0x0053` | `(i32)` | `0x084B#0x002A` | `0x00C7000C` | - |
| `0x8849005D` | `0x0849` | `0x005D` | `(i32)` | `0x084B#0x002B` | - | - |
| `0x88490064` | `0x0849` | `0x0064` | `(i32, ref, i64, i64)` | `0x084B#0x002E` | `0x0030000A`, `0x08660018` | `0x100002FE` |
| `0x88490068` | `0x0849` | `0x0068` | `(i32, i64)` | `0x084B#0x0030` | `0x0030000A`, `0x0866001A` | `0x100003A3` |
| `0x8849006A` | `0x0849` | `0x006A` | `(i64)` | `0x084B#0x0032` | `0x0030000A` | - |
| `0x8849006E` | `0x0849` | `0x006E` | `(i32, i64)` | `0x084B#0x0033` | `0x0030000A` | - |
| `0x8849006F` | `0x0849` | `0x006F` | `(ref, i32)` | `0x084B#0x0034` | `0x0866001C` | `0x100003DA` |
| `0x88490071` | `0x0849` | `0x0071` | `` | `0x084B#0x0035` | `0x0866001D` | `0x100003DC` |
| `0x88490073` | `0x0849` | `0x0073` | `(i32)` | `0x084B#0x0036` | `0x0866001E` | `0x100003E2` |
| `0x88490075` | `0x0849` | `0x0075` | `(i32, ref, ref)` | `0x084B#0x0037` | `0x0866001F` | `0x100003E3` |
| `0x88490077` | `0x0849` | `0x0077` | `(i64)` | `0x084B#0x0038` | `0x08660020` | `0x100004E6` |
| `0x88490080` | `0x0849` | `0x0080` | `(i32, i32, i64)` | `0x084B#0x003D` | `0x0030000A` | - |
| `0x88490082` | `0x0849` | `0x0082` | `(i32, i64)` | `0x084B#0x003C` | `0x0030000A` | - |
| `0x88490083` | `0x0849` | `0x0083` | `(i32, i64, i64)` | `0x084B#0x003E` | `0x0030000A`, `0x08660021` | `0x100004E7` |
| `0x88490085` | `0x0849` | `0x0085` | `(i64)` | `0x084B#0x003F` | `0x0030000A` | - |
| `0x88490087` | `0x0849` | `0x0087` | `(i32, i32, i64)` | `0x084B#0x0040` | `0x0030000A`, `0x08660021` | `0x100004E7` |
| `0x8849008B` | `0x0849` | `0x008B` | `(i32, i32, i64)` | `0x084B#0x0042` | `0x0030000A` | - |
| `0x8849008D` | `0x0849` | `0x008D` | `(i64)` | `0x084B#0x0043` | `0x0030000A`, `0x08660024` | `0x100004EA` |
| `0x88490090` | `0x0849` | `0x0090` | `(i64)` | `0x084B#0x0044` | `0x0030000A`, `0x08660021` | `0x100004E7` |
| `0x88490098` | `0x0849` | `0x0098` | `(ref)` | `0x084B#0x0048` | `0x08660025` | `0x10000503` |
| `0x8859000B` | `0x0859` | `0x000B` | `(i32, i64)` | `0x084E#0x0007` | `0x08720004` | `0x1000029C` |
| `0x88590011` | `0x0859` | `0x0011` | `(i32)` | `0x084E#0x0008` | `0x08720002`, `0x00E70001` | `0x1000028E` |
| `0x88590026` | `0x0859` | `0x0026` | `(i32)` | `0x084E#0x001C` | `0x000B0002` | - |
| `0x88590028` | `0x0859` | `0x0028` | `(i32)` | `0x084E#0x001D` | `0x00270006`, `0x00230002` | - |
| `0x88CB0001` | `0x08CB` | `0x0001` | `(i32, i64)` | `0x08CC#0x0001` | `0x08CA0001` | `0x100002B1` |
| `0x89580001` | `0x0958` | `0x0001` | `(ref, i32, i32, i32)` | `0x0959#0x0001` | `0x015B0001` | `0x10000167` |
| `0x89580005` | `0x0958` | `0x0005` | `(ref, i32, i32)` | `0x0959#0x0006` | `0x019F0008` | - |
| `0x8958000B` | `0x0958` | `0x000B` | `(i32)` | `0x0959#0x000A` | `0x019F002A`, `0x015B0001` | `0x10000167` |
| `0x8958000D` | `0x0958` | `0x000D` | `(i32, i32)` | `0x0959#0x000C` | `0x01C60006`, `0x01C60008`, `0x019F002A`, `0x015B0001` | `0x10000167` |
| `0x89BB0003` | `0x09BB` | `0x0003` | `(ref)` | `0x09BD#0x0002` | - | - |
| `0x89EB000B` | `0x09EB` | `0x000B` | `(ref, i32)` | `CWebStoreSystem#0x0008` | `0x00E70001`, `0x0A1A0002`, `0x0A1A0007`, `0x0FE00002` | `0x10000390`; `0x10000500` |
| `0x89EB000E` | `0x09EB` | `0x000E` | `` | `CWebStoreSystem#0x004D` | `0x019F001D` | - |
| `0x89EB0017` | `0x09EB` | `0x0017` | `` | `CWebStoreSystem#0x0064` | `0x0A1A0006` | `0x100004DE` |
| `0x89EC0007` | `0x09EC` | `0x0007` | `(ref, i32)` | `CSkirmishSystem::RecvCEvt_UpdateSkirmishStat` | `0x0A510002`, `0x0A510001`, `0x0CBA000D` | `0x1000039C`; `0x1000039D` |
| `0x89EC000A` | `0x09EC` | `0x000A` | `(ref)` | `CSkirmishSystem#0x0002` | `0x0A510001` | `0x1000039D` |
| `0x89EC000E` | `0x09EC` | `0x000E` | `(ref, i32)` | `CSkirmishSystem::RecvCEvt_IncrementSkirmishStat` | `0x0A510002`, `0x0A510001`, `0x0CBA000D` | `0x1000039C`; `0x1000039D` |
| `0x89EC0021` | `0x09EC` | `0x0021` | `(i32)` | `CSkirmishSystem#0x0007` | `0x0A510003` | `0x100003AE` |
| `0x8A04001D` | `0x0A04` | `0x001D` | `(i32, ref)` | `0x0A02#0x0016` | `0x0A0A0003` | `0x100003B9` |
| `0x8A040023` | `0x0A04` | `0x0023` | `(i64, i32, i32, ref)` | `0x0A02#0x001B` | `0x0A0A0005` | `0x100003BD` |
| `0x8A040026` | `0x0A04` | `0x0026` | `(ref, i32, i32, ref)` | `0x0A02#0x001A` | `0x0A0A0004`, `0x0A070004` | `0x100003BC` |
| `0x8A040028` | `0x0A04` | `0x0028` | `(ref, i32, i32, ref, ref)` | `0x0A02#0x001F` | `0x0A0A0008`, `0x0E9C0005` | `0x100003C6` |
| `0x8A2C0003` | `0x0A2C` | `0x0003` | `(ref)` | `0x0A2B#0x0003` | `0x0A2D0001` | - |
| `0x8A2C000B` | `0x0A2C` | `0x000B` | `(ref)` | `0x0A2B#0x000B` | `0x0A2D0002` | `0x10000392` |
| `0x8A2C0031` | `0x0A2C` | `0x0031` | `(ref)` | `0x0A2B#0x0011` | `0x0A44000E` | - |
| `0x8A2C0049` | `0x0A2C` | `0x0049` | `(i32)` | `0x0A2B#0x0016` | `0x0A2D0003` | `0x100003B0` |
| `0x8A2C0053` | `0x0A2C` | `0x0053` | `(ref)` | `0x0A2B#0x0020` | `0x00070001`, `0x00C7000C`, `0x00C70003`, `0x00C70009`, `0x00330001` (+4) | - |
| `0x8A2C0062` | `0x0A2C` | `0x0062` | `(i32)` | `0x0A2B#0x0025` | `0x0A2D0005` | `0x100003B7` |
| `0x8A450001` | `0x0A45` | `0x0001` | `(i64)` | `0x0A43#0x0002` | `0x0A460001` | `0x10000396` |
| `0x8A8400FB` | `0x0A84` | `0x00FB` | `(ref)` | `0x1062#0x001E` | `0x10610001` | `0x10000527` |
| `0x8AE60003` | `0x0AE6` | `0x0003` | `(ref, ref, ref, i64)` | `0x0AE7#0x0005` | `0x00E70003`, `0x09B60001` | - |
| `0x8C0D0001` | `0x0C0D` | `0x0001` | `(i32, i32)` | `0x0C0C#0x0002` | `0x0C150001`, `0x0C150003` | `0x100003C2`; `0x10000442` |
| `0x8C0D0004` | `0x0C0D` | `0x0004` | `(i32, i32, i32)` | `0x0C0C#0x0003` | `0x0C150001`, `0x0C150003`, `0x0C150002` | `0x100003C2`; `0x10000442`; `0x1000043E` |
| `0x8C0D0006` | `0x0C0D` | `0x0006` | `(ref)` | `0x0C0C#0x0004` | `0x0C150001`, `0x0C150003` | `0x100003C2`; `0x10000442` |
| `0x8C0D001C` | `0x0C0D` | `0x001C` | `(i32)` | `0x0C0C#0x0016` | `0x0C150006` | `0x100004FB` |
| `0x8C410028` | `0x0C41` | `0x0028` | `(i32, i64)` | `CFellowshipMakerSystem::RecvCEvt_FellowConfirmEnterQueue` | `0x0CD6001D`, `0x0030000B`, `0x0CBA000D` | `0x10000433` |
| `0x8C410034` | `0x0C41` | `0x0034` | `(ref, ref)` | `CFellowshipMakerSystem#0x0011` | `0x0CD6001F` | `0x1000043A` |
| `0x8C410037` | `0x0C41` | `0x0037` | `(i32, i64)` | `CFellowshipMakerSystem#0x0012` | `0x0CD60020` | `0x1000043B` |
| `0x8C410049` | `0x0C41` | `0x0049` | `(ref)` | `CFellowshipMakerSystem#0x0018` | `0x00C7000C`, `0x0061000A`, `0x00620006`, `0x00620002`, `0x00620003` (+7) | - |
| `0x8CFE0008` | `0x0CFE` | `0x0008` | `(ref)` | `FellowshipVoteSystem::GetVoteTextFromHolder` | `0x00C7000C`, `0x00C70019`, `0x00C70009`, `0x00330001`, `0x00A6001F` (+11) | - |
| `0x8CFE0010` | `0x0CFE` | `0x0010` | `(i32, ref)` | `FellowshipVoteSystem::GetVoteTextFromHolder` | `0x00C70019`, `0x00C7000C`, `0x00C70009`, `0x00330001`, `0x00070002` (+5) | - |
| `0x8DA70011` | `0x0DA7` | `0x0011` | `(i32)` | `CMountedCombatSystem#0x0029` | `0x00DC0007` | - |
| `0x8DD10047` | `0x0DD1` | `0x0047` | `(i32, ref)` | `0x0DD0#0x0006` | `0x0DEF0001` | `0x10000456` |
| `0x8E15000E` | `0x0E15` | `0x000E` | `(ref, i32)` | `0x0E16#0x0009` | `0x0E220001` | - |
| `0x8E150011` | `0x0E15` | `0x0011` | `(i32)` | `0x0E16#0x000C` | `0x0E220002` | `0x1000046A` |
| `0x8E150013` | `0x0E15` | `0x0013` | `(i32)` | `CMountedCombatSystem::ConstructCosmeticOverrideProperty` | `0x0030000B`, `0x0C680005`, `0x0E220003`, `0x00E70001` | `0x10000469` |
| `0x8E7C0003` | `0x0E7C` | `0x0003` | `(i64, i32, i32, i32, i32, i32)` | `CRandomItemRewardSystem::RecvCEvt_RandomItemRewardedFromTreasureTable` | `0x0E790008`, `0x0CBA000D` | `0x10000497` |
| `0x8E7C000E` | `0x0E7C` | `0x000E` | `` | `CRandomItemRewardSystem#0x0010` | `0x0E790009` | `0x100004A9` |
| `0x8EB6000C` | `0x0EB6` | `0x000C` | `(ref)` | `CBigBattleSystem::RecvCEvt_InitializeMedalRecords` | `0x0EC90003`, `0x0EC90005`, `0x0F230001`, `0x0CBA000D` | `0x100004BC` |
| `0x8EB6000E` | `0x0EB6` | `0x000E` | `(i32, i32)` | `CBigBattleSystem#0x0006` | `0x0F230002` | `0x100004BE` |
| `0x8EB60010` | `0x0EB6` | `0x0010` | `(ref, i32)` | `CBigBattleSystem#0x0007` | `0x0F230003` | `0x100004C2` |
| `0x8EB60017` | `0x0EB6` | `0x0017` | `` | `CBigBattleSystem#0x0008` | `0x01810004`, `0x01810003` | - |
| `0x8EB6001B` | `0x0EB6` | `0x001B` | `(ref, ref, ref, ref, i64)` | `CBigBattleSystem#0x000A` | `0x0030000B`, `0x0EB30002` | `0x100004DF` |
| `0x8ED50010` | `0x0ED5` | `0x0010` | `(i64)` | `CMultiUsageSystem#0x0002` | `0x0EE70002` | `0x100004B0` |
| `0x8ED50011` | `0x0ED5` | `0x0011` | `(i32, i64)` | `CMultiUsageSystem#0x0003` | `0x0EE70002` | `0x100004B0` |
| `0x8ED50016` | `0x0ED5` | `0x0016` | `(ref, i64)` | `CMultiUsageSystem#0x0001` | `0x0EE70003` | `0x100004B2` |
| `0x8ED50021` | `0x0ED5` | `0x0021` | `(i32, i32, i64)` | `CMultiUsageSystem#0x0009` | `0x0EE70004` | `0x100004B3` |
| `0x8ED50027` | `0x0ED5` | `0x0027` | `(i32, i32, i64)` | `CMultiUsageSystem#0x000D` | `0x0EE70005` | `0x100004B4` |
| `0x8ED50032` | `0x0ED5` | `0x0032` | `(ref, i64, i64)` | `CMultiUsageSystem#0x0018` | `0x0EE70008`, `0x0EE70009` | `0x100004B9`; `0x100004BA` |
| `0x8EF0000B` | `0x0EF0` | `0x000B` | `(ref, i64)` | `0x0EFC#0x0001` | `0x0EFB0001` | `0x100004B6` |
| `0x8FA00002` | `0x0FA0` | `0x0002` | `(i32)` | `0x0F9F#0x0002` | `0x0FA20002` | `0x100004E5` |
| `0x8FA00004` | `0x0FA0` | `0x0004` | `(ref)` | `0x0F9F#0x0003` | `0x0FA20001` | `0x100004E4` |
| `0x8FBC0003` | `0x0FBC` | `0x0003` | `(i64)` | `0x0F71#0x000B` | `0x01810004`, `0x01810003`, `0x0F760002` | `0x10000512` |
| `0x8FBC0004` | `0x0FBC` | `0x0004` | `(i32, i64)` | `0x0F71#0x000C` | `0x01810004`, `0x01810003`, `0x0F760002` | `0x10000512` |
| `0x90010002` | `0x1001` | `0x0002` | `(i64)` | `CImageItemSystem::RecvCEvt_ShowImageItem` | `0x0030000B`, `0x10020001`, `0x0CBA000D` | `0x100004FC` |
| `0x90120013` | `0x1012` | `0x0013` | `(i32)` | `0x1011#0x0010` | `0x10730002` | `0x1000053A` |
| `0x9055000D` | `0x1055` | `0x000D` | `(ref, i64)` | `0x1054#0x0034` | `0x10660003` | `0x1000052D` |
| `0x90550010` | `0x1055` | `0x0010` | `(i32, i64)` | `0x1054#0x0035` | `0x10660008` | `0x1000052C` |
| `0x90550015` | `0x1055` | `0x0015` | `(ref, i64)` | `0x1054#0x0039` | `0x1066000F` | `0x10000536` |
| `0x90550020` | `0x1055` | `0x0020` | `(i64)` | `0x1054#0x004D` | `0x10660010` | `0x10000557` |
| `0x90860002` | `0x1086` | `0x0002` | `(i32)` | `0x1087#0x0002` | - | - |
| `0x90860004` | `0x1086` | `0x0004` | `(i32)` | `0x1087#0x0003` | - | - |
| `0x90860006` | `0x1086` | `0x0006` | `(i32)` | `0x1087#0x0001` | - | - |
| `0x90860009` | `0x1086` | `0x0009` | `(ref, ref)` | `0x1087#0x0005` | `0x10890001` | `0x1000053F` |
| `0x9086000C` | `0x1086` | `0x000C` | `(i32)` | `0x1087#0x0006` | - | - |
| `0x9086000D` | `0x1086` | `0x000D` | `(f32, i32, i32)` | `0x1087#0x0007` | `0x10890003` | `0x10000540` |
| `0x9086000F` | `0x1086` | `0x000F` | `(i32, i32)` | `0x1087#0x000D` | `0x10890002` | `0x10000541` |
| `0x90860011` | `0x1086` | `0x0011` | `(f32, i32, i32)` | `0x1087#0x000E` | `0x10890004` | `0x10000542` |
| `0x969F0016` | `0x169F` | `0x0016` | `(i64)` | `NPCSystem#0x0013` | `0x01520008` | `0x10000569` |
| `0x969F001E` | `0x169F` | `0x001E` | `(ref)` | `NPCSystem#0x0021` | `0x16A40004`, `0x01520006` | `0x10000567` |
| `0x969F0020` | `0x169F` | `0x0020` | `(ref)` | `NPCSystem#0x0020` | `0x01520007`, `0x16A40004` | `0x10000566` |
| `0x969F0023` | `0x169F` | `0x0023` | `(ref)` | `NPCSystem#0x001F` | `0x16A40004`, `0x01520007`, `0x01520004` | `0x10000566`; `0x10000568` |

---

### 2.9 Complete Census of All 619 Client -> Server (`C->S`) WLib RPC Stubs (`0x8XXXXXXX`) <a id="c2s-rpc-census"></a>

Every Client -> Server WLib RPC method in `client_gamelogic.dat` record `0x56000000` (`wlib.bin`) allocates a `WLibArgFrame` (`0x008E`), packs arguments sequentially via `0x008E0001..0x008E0005`, and transmits the RPC via `0x00D9000C` or `0x00D9000D`:

| RPC Method ID | Root `cid` | `fnum` | Dispatcher (`0x00D9`) | Packed `WLibArgFrame` Signature (`0x008E0001..05`) | WLib Caller Method(s) (`0x16` Callers) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `0x81F50002` | `0x01F5` | `0x0003` | `0x00D9000D` | `(ref)` | `EngineOptionSystem#0x0001` |
| `0x81F50001` | `0x01F5` | `0x0004` | `0x00D9000D` | `(i32)` | `EngineOptionSystem#0x0002` |
| `0x84D90055` | `0x04D9` | `0x0062` | `0x00D9000D` | `(i32)` | `0x0675#0x0445` |
| `0x84D90049` | `0x04D9` | `0x0063` | `0x00D9000C` | `(i32, i32)` | `0x00FE#0x001E` |
| `0x84D90027` | `0x04D9` | `0x0064` | `0x00D9000C` | `(i64, i32)` | `0x00FE#0x001C`, `0x00FE#0x0018` |
| `0x84D9004A` | `0x04D9` | `0x0065` | `0x00D9000C` | `(i64, ref)` | `0x06CA#0x0002` |
| `0x84D9000C` | `0x04D9` | `0x0066` | `0x00D9000C` | `(ref, i64)` | `0x00FE#0x000A` |
| `0x84D900B3` | `0x04D9` | `0x00B4` | `0x00D9000C` | `(ref, i32)` | `0x0E75#0x0004` |
| `0x84DA0007` | `0x04DA` | `0x0008` | `0x00D9000C` | `(i64, ref)` | `0x00FC#0x0002` |
| `0x84DC0037` | `0x04DC` | `0x004C` | `0x00D9000D` | `(ref, ref)` | `0x0675#0x045F` |
| `0x84DC0021` | `0x04DC` | `0x004D` | `0x00D9000D` | `(i32, i32, i64)` | `0x0675#0x03DE` |
| `0x84DC003F` | `0x04DC` | `0x004E` | `0x00D9000D` | `` | `0x0675#0x03C9` |
| `0x84DC0024` | `0x04DC` | `0x004F` | `0x00D9000D` | `(i32, ref, i64)` | `0x0675#0x0465` |
| `0x84DC002E` | `0x04DC` | `0x0050` | `0x00D9000D` | `(i32, i32, i32, i64)` | `0x0675#0x03B5`, `0x0675#0x03F4` |
| `0x84DC0017` | `0x04DC` | `0x0055` | `0x00D9000D` | `(i64)` | `0x0675#0x03DE`, `0x0675#0x03C4` |
| `0x84DC002B` | `0x04DC` | `0x0056` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x03A9` |
| `0x84DC0012` | `0x04DC` | `0x0057` | `0x00D9000D` | `(f32)` | `0x0675#0x0453` |
| `0x84DC0034` | `0x04DC` | `0x0058` | `0x00D9000D` | `(ref, i64)` | `0x0675#0x045B` |
| `0x84DC0013` | `0x04DC` | `0x0059` | `0x00D9000D` | `(i32)` | `0x0675#0x03AE` |
| `0x84DC0048` | `0x04DC` | `0x005A` | `0x00D9000D` | `(ref, ref)` | `0x0675#0x044A` |
| `0x84DC0049` | `0x04DC` | `0x005B` | `0x00D9000D` | `(i32, i32, i64)` | `0x0675#0x03DE` |
| `0x84DC0011` | `0x04DC` | `0x005C` | `0x00D9000D` | `(f32, ref, i64, i64)` | `0x0675#0x0449`, `0x0675#0x04B2`, `0x0675#0x04B3` |
| `0x84DC000A` | `0x04DC` | `0x0060` | `0x00D9000D` | `(i64)` | `0x0675#0x0446` |
| `0x84DC000D` | `0x04DC` | `0x0061` | `0x00D9000D` | `(i64, ref)` | `0x0675#0x03CB` |
| `0x84DC0020` | `0x04DC` | `0x0062` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x03E7` |
| `0x84DC000B` | `0x04DC` | `0x0063` | `0x00D9000D` | `(i32, f32, i32, i32, i64)` | `0x0675#0x0433` |
| `0x84DC000C` | `0x04DC` | `0x0065` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x03FD` |
| `0x84DC0029` | `0x04DC` | `0x0067` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x03FC` |
| `0x84DC0046` | `0x04DC` | `0x0068` | `0x00D9000D` | `(i32, i32, i64)` | `0x0675#0x03DE` |
| `0x84DC0005` | `0x04DC` | `0x0069` | `0x00D9000D` | `(i32, i32, i64)` | `0x0675#0x0463` |
| `0x84DC003B` | `0x04DC` | `0x006B` | `0x00D9000D` | `(i64)` | `0x0675#0x03DE` |
| `0x84DC001A` | `0x04DC` | `0x006C` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x0421` |
| `0x84DC0030` | `0x04DC` | `0x006D` | `0x00D9000D` | `(ref, ref)` | `0x0675#0x038A` |
| `0x84DC0001` | `0x04DC` | `0x006E` | `0x00D9000D` | `(ref, i64)` | `0x0675#0x03D3` |
| `0x84DC0008` | `0x04DC` | `0x006F` | `0x00D9000D` | `(i64)` | `0x0675#0x0457` |
| `0x84DC0007` | `0x04DC` | `0x0070` | `0x00D9000D` | `(i64)` | `0x0675#0x038D` |
| `0x84DC0025` | `0x04DC` | `0x0073` | `0x00D9000D` | `(i64)` | `0x0675#0x0439` |
| `0x84DC0019` | `0x04DC` | `0x0074` | `0x00D9000D` | `(i32, i32)` | `0x0675#0x0417` |
| `0x84DC0039` | `0x04DC` | `0x0075` | `0x00D9000D` | `(i64)` | `0x0675#0x045C` |
| `0x84DC003D` | `0x04DC` | `0x0076` | `0x00D9000D` | `(i64)` | `0x0675#0x03B3` |
| `0x84DC0016` | `0x04DC` | `0x0077` | `0x00D9000D` | `(i64)` | `0x0675#0x03ED` |
| `0x84DC0031` | `0x04DC` | `0x0078` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x0463` |
| `0x84DC0040` | `0x04DC` | `0x0079` | `0x00D9000D` | `(ref, i64)` | `0x0675#0x0393` |
| `0x84DC0026` | `0x04DC` | `0x007A` | `0x00D9000D` | `(ref, i32)` | `0x0675#0x0423` |
| `0x84DC003C` | `0x04DC` | `0x007B` | `0x00D9000D` | `(i64, ref)` | `0x0675#0x046A` |
| `0x84DC000E` | `0x04DC` | `0x007D` | `0x00D9000D` | `(i64)` | `0x0675#0x0463` |
| `0x84DC003E` | `0x04DC` | `0x0081` | `0x00D9000D` | `(i64, ref)` | `0x0675#0x042B` |
| `0x84DC0004` | `0x04DC` | `0x0082` | `0x00D9000D` | `(ref, i32, i64)` | `0x0675#0x03F9` |
| `0x84DC0035` | `0x04DC` | `0x0083` | `0x00D9000D` | `(ref)` | `0x0675#0x03BC` |
| `0x84DC0045` | `0x04DC` | `0x0084` | `0x00D9000D` | `(i32)` | `0x0675#0x03D4` |
| `0x84DC0041` | `0x04DC` | `0x0085` | `0x00D9000D` | `(ref, i32, i64)` | `0x0675#0x03CD` |
| `0x84DC0009` | `0x04DC` | `0x0087` | `0x00D9000D` | `(i64)` | `0x0675#0x0395` |
| `0x84DC0090` | `0x04DC` | `0x0091` | `0x00D9000D` | `(ref)` | `0x0675#0x04DD` |
| `0x84DC0096` | `0x04DC` | `0x0097` | `0x00D9000D` | `(i32, i32, i64)` | `0x0675#0x03DE` |
| `0x84DD000E` | `0x04DD` | `0x0010` | `0x00D9000D` | `(ref)` | `AdminPropertySystem#0x0001` |
| `0x84DD0008` | `0x04DD` | `0x0011` | `0x00D9000D` | `(i32, i64)` | `0x0675#0x03CC` |
| `0x84DD0003` | `0x04DD` | `0x0012` | `0x00D9000D` | `(i32, ref)` | `0x0675#0x03F5` |
| `0x84DD0004` | `0x04DD` | `0x0013` | `0x00D9000D` | `(ref, ref, i64)` | `0x0675#0x0384`, `AdminPropertySystem#0x0015` |
| `0x84DD0016` | `0x04DD` | `0x0017` | `0x00D9000D` | `(ref, i64)` | `0x0675#0x0384`, `AdminPropertySystem#0x0016` |
| `0x84DD0018` | `0x04DD` | `0x0019` | `0x00D9000D` | `(ref, ref, i32, ref)` | `0x0675#0x0384` |
| `0x84DD001A` | `0x04DD` | `0x001B` | `0x00D9000D` | `(ref, i32, ref)` | `0x0675#0x0384` |
| `0x84DD001D` | `0x04DD` | `0x001F` | `0x00D9000D` | `(ref, i64)` | `0x0675#0x0538` |
| `0x84DE0003` | `0x04DE` | `0x001B` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0002`, `CTravelSystem#0x0016` |
| `0x84DE000F` | `0x04DE` | `0x001C` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x000A`, `PhysicsSystem#0x0006` |
| `0x84DE0013` | `0x04DE` | `0x001D` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x000A` |
| `0x84DE0017` | `0x04DE` | `0x001E` | `0x00D9000D` | `(ref)` | `0x0AF4#0x000B` |
| `0x84DE0014` | `0x04DE` | `0x001F` | `0x00D9000D` | `(ref, ref)` | `CQuestSystem#0x0035` |
| `0x84DE0005` | `0x04DE` | `0x0020` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x000F` |
| `0x84DE000B` | `0x04DE` | `0x0021` | `0x00D9000D` | `(ref)` | `0x011A#0x000D` |
| `0x84DE0015` | `0x04DE` | `0x0022` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0028`, `CCommunicationSystem#0x003A` |
| `0x84DE000D` | `0x04DE` | `0x0023` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0026`, `0x0129#0x0011` |
| `0x84DE0011` | `0x04DE` | `0x0024` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0003`, `CTravelSystem#0x000C` |
| `0x84DE0016` | `0x04DE` | `0x0025` | `0x00D9000D` | `(ref)` | `0x0129#0x0008` |
| `0x84DE0010` | `0x04DE` | `0x0026` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0011` |
| `0x84DE000E` | `0x04DE` | `0x0027` | `0x00D9000D` | `(ref)` | `MailSystem#0x0030` |
| `0x84DE0008` | `0x04DE` | `0x0028` | `0x00D9000D` | `(ref)` | `0x010E#0x0007` |
| `0x84DE0019` | `0x04DE` | `0x0029` | `0x00D9000D` | `(ref)` | `CCommunicationSystem#0x0037` |
| `0x84DE000A` | `0x04DE` | `0x002A` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x000E`, `PhysicsSystem#0x0001` |
| `0x84DE0009` | `0x04DE` | `0x002B` | `0x00D9000D` | `(ref)` | `CCommunicationSystem#0x002B` |
| `0x84DE0012` | `0x04DE` | `0x002C` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0012` |
| `0x84DE000C` | `0x04DE` | `0x002D` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0009` |
| `0x84DE002E` | `0x04DE` | `0x0030` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0018` |
| `0x84DE002F` | `0x04DE` | `0x0031` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0015` |
| `0x84DE0033` | `0x04DE` | `0x0039` | `0x00D9000D` | `(ref)` | `GuildSystem#0x013B` |
| `0x84DE0038` | `0x04DE` | `0x003D` | `0x00D9000D` | `(ref)` | `0x016F#0x0014` |
| `0x84DE0036` | `0x04DE` | `0x003E` | `0x00D9000D` | `(ref)` | `0x00FE#0x002B` |
| `0x84DE0037` | `0x04DE` | `0x003F` | `0x00D9000D` | `(ref)` | `0x0119#0x0011` |
| `0x84DE0040` | `0x04DE` | `0x0041` | `0x00D9000D` | `(ref)` | `0x016F#0x0017` |
| `0x84DE0043` | `0x04DE` | `0x0042` | `0x00D9000D` | `(ref)` | `CCommunicationSystem#0x0049` |
| `0x84DE0045` | `0x04DE` | `0x0044` | `0x00D9000D` | `(ref, ref)` | `AdvancementSystem#0x007B` |
| `0x84DE0046` | `0x04DE` | `0x0047` | `0x00D9000D` | `(i64, ref)` | `AdvancementSystem#0x0082` |
| `0x84DE0048` | `0x04DE` | `0x0049` | `0x00D9000D` | `(i64, ref)` | `SkillSystem#0x00C7` |
| `0x84DE004A` | `0x04DE` | `0x004B` | `0x00D9000D` | `(i64, ref)` | `CTraitSystem#0x0010` |
| `0x84DE004C` | `0x04DE` | `0x004D` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0019` |
| `0x84DE004F` | `0x04DE` | `0x004E` | `0x00D9000D` | `(ref)` | `GuildSystem#0x0135` |
| `0x84DE0051` | `0x04DE` | `0x0050` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x001B` |
| `0x84DE0052` | `0x04DE` | `0x0054` | `0x00D9000D` | `(i64, ref)` | `PhysicsSystem#0x001E` |
| `0x84DE0053` | `0x04DE` | `0x0055` | `0x00D9000D` | `(i64, ref)` | `PhysicsSystem#0x001F` |
| `0x84DE0056` | `0x04DE` | `0x0057` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0021`, `PhysicsSystem#0x0024` |
| `0x84DE0058` | `0x04DE` | `0x0059` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x0029` |
| `0x84DE005B` | `0x04DE` | `0x005A` | `0x00D9000D` | `(ref)` | `0x071D#0x0001` |
| `0x84DE005C` | `0x04DE` | `0x0062` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x002A` |
| `0x84DE0061` | `0x04DE` | `0x0063` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x002C` |
| `0x84DE005E` | `0x04DE` | `0x0064` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x002B` |
| `0x84DE0060` | `0x04DE` | `0x0065` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x002D` |
| `0x84DE005F` | `0x04DE` | `0x0066` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x002F` |
| `0x84DE005D` | `0x04DE` | `0x0067` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x002E` |
| `0x84DE006D` | `0x04DE` | `0x006E` | `0x00D9000D` | `(ref)` | `0x0757#0x0001` |
| `0x84DE006C` | `0x04DE` | `0x006F` | `0x00D9000D` | `(ref)` | `0x0756#0x0001` |
| `0x84DE0070` | `0x04DE` | `0x0071` | `0x00D9000D` | `(i64, ref)` | `0x0113#0x001C` |
| `0x84DE0072` | `0x04DE` | `0x0073` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0037` |
| `0x84DE0074` | `0x04DE` | `0x0075` | `0x00D9000D` | `(ref)` | `0x0769#0x0001` |
| `0x84DE0078` | `0x04DE` | `0x0079` | `0x00D9000D` | `(i32, i64)` | `PhysicsSystem#0x003A` |
| `0x84DE007A` | `0x04DE` | `0x007B` | `0x00D9000D` | `(i64, ref)` | `AuctionSystem#0x0071` |
| `0x84DE007C` | `0x04DE` | `0x007D` | `0x00D9000D` | `(ref)` | `0x0786#0x0001` |
| `0x84DE007E` | `0x04DE` | `0x007F` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x003B` |
| `0x84DE0080` | `0x04DE` | `0x0081` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x003D` |
| `0x84DE0085` | `0x04DE` | `0x0086` | `0x00D9000D` | `(ref)` | `0x011C#0x0013` |
| `0x84DE0087` | `0x04DE` | `0x0088` | `0x00D9000D` | `(ref)` | `0x07A6#0x0001` |
| `0x84DE0089` | `0x04DE` | `0x008A` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0040` |
| `0x84DE008D` | `0x04DE` | `0x008E` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0042` |
| `0x84DE008C` | `0x04DE` | `0x008F` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0044` |
| `0x84DE008B` | `0x04DE` | `0x0090` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0043` |
| `0x84DE0091` | `0x04DE` | `0x0092` | `0x00D9000D` | `(ref)` | `0x0675#0x04A6` |
| `0x84DE0093` | `0x04DE` | `0x0094` | `0x00D9000D` | `(ref)` | `GuildSystem#0x0171` |
| `0x84DE0095` | `0x04DE` | `0x0096` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x0045` |
| `0x84DE009A` | `0x04DE` | `0x00A3` | `0x00D9000D` | `(i32, i32)` | `PhysicsSystem#0x0047` |
| `0x84DE009E` | `0x04DE` | `0x00A4` | `0x00D9000D` | `(i32)` | `PhysicsSystem#0x0046` |
| `0x84DE00A5` | `0x04DE` | `0x00A6` | `0x00D9000D` | `(ref)` | `AuctionSystem#0x0016` |
| `0x84DE00A8` | `0x04DE` | `0x00A7` | `0x00D9000D` | `(ref)` | `0x0129#0x0015` |
| `0x84DE00AB` | `0x04DE` | `0x00AC` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x004A` |
| `0x84DE00AD` | `0x04DE` | `0x00AE` | `0x00D9000D` | `(ref)` | `0x084E#0x0001` |
| `0x84DE00AF` | `0x04DE` | `0x00B2` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x004C` |
| `0x84DE00B6` | `0x04DE` | `0x00B7` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x004E` |
| `0x84DE00B9` | `0x04DE` | `0x00B8` | `0x00D9000D` | `(ref)` | `0x0114#0x0006` |
| `0x84DE00BB` | `0x04DE` | `0x00BA` | `0x00D9000D` | `(ref)` | `CEffectListenerSystem#0x0004` |
| `0x84DE00BC` | `0x04DE` | `0x00BD` | `0x00D9000D` | `(ref)` | `0x0923#0x0001` |
| `0x84DE00BF` | `0x04DE` | `0x00BE` | `0x00D9000D` | `(ref)` | `CMEOInventorySystem#0x0010` |
| `0x84DE00C2` | `0x04DE` | `0x00C3` | `0x00D9000D` | `(i32, i32, i32)` | `PhysicsSystem#0x004F` |
| `0x84DE00C4` | `0x04DE` | `0x00C5` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0050` |
| `0x84DE00C7` | `0x04DE` | `0x00C8` | `0x00D9000D` | `(i32, ref)` | `PhysicsSystem#0x0054` |
| `0x84DE00C6` | `0x04DE` | `0x00C9` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0053` |
| `0x84DE00CC` | `0x04DE` | `0x00CD` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0055` |
| `0x84DE00CE` | `0x04DE` | `0x00CF` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0056` |
| `0x84DE00D1` | `0x04DE` | `0x00D0` | `0x00D9000D` | `(i64, ref)` | `PhysicsSystem#0x0058` |
| `0x84DE00D3` | `0x04DE` | `0x00D2` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x005A` |
| `0x84DE00D4` | `0x04DE` | `0x00D5` | `0x00D9000D` | `(ref)` | `CMountSystem#0x0006` |
| `0x84DE00D6` | `0x04DE` | `0x00D7` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x005D` |
| `0x84DE00DA` | `0x04DE` | `0x00DB` | `0x00D9000D` | `(ref)` | `CWebStoreSystem#0x0002` |
| `0x84DE00DC` | `0x04DE` | `0x00DD` | `0x00D9000D` | `(ref)` | `0x0A02#0x0001` |
| `0x84DE00DE` | `0x04DE` | `0x00DF` | `0x00D9000D` | `(ref)` | `0x0A2B#0x000C` |
| `0x84DE00E2` | `0x04DE` | `0x00E3` | `0x00D9000D` | `(ref)` | `0x0A43#0x0001` |
| `0x84DE00E4` | `0x04DE` | `0x00E5` | `0x00D9000D` | `(ref)` | `CCommunicationSystem#0x0061` |
| `0x84DE00E9` | `0x04DE` | `0x00EA` | `0x00D9000D` | `(ref)` | `CSkirmishSystem#0x0006` |
| `0x84DE00EB` | `0x04DE` | `0x00EC` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x005E` |
| `0x84DE00ED` | `0x04DE` | `0x00EE` | `0x00D9000D` | `(ref)` | `CCombatSystem#0x005E` |
| `0x84DE00F0` | `0x04DE` | `0x00F1` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x0061` |
| `0x84DE00EF` | `0x04DE` | `0x00F2` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x0062` |
| `0x84DE00F3` | `0x04DE` | `0x00F4` | `0x00D9000D` | `(ref)` | `CAdmin::DoCmdDataID` |
| `0x84DE00F5` | `0x04DE` | `0x00F6` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0064` |
| `0x84DE00F9` | `0x04DE` | `0x00FA` | `0x00D9000D` | `(i32)` | `PhysicsSystem#0x0066` |
| `0x84DE0107` | `0x04DE` | `0x0108` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x006A` |
| `0x84DE0109` | `0x04DE` | `0x010A` | `0x00D9000D` | `(i64)` | `PhysicsSystem#0x006B` |
| `0x84DE010B` | `0x04DE` | `0x010C` | `0x00D9000D` | `(ref)` | `0x0BF7#0x0002` |
| `0x84DE010D` | `0x04DE` | `0x0110` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x006D` |
| `0x84DE0111` | `0x04DE` | `0x0112` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0070` |
| `0x84DE0113` | `0x04DE` | `0x0114` | `0x00D9000D` | `(ref)` | `CFellowshipMakerSystem#0x0001` |
| `0x84DE0115` | `0x04DE` | `0x0116` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0073` |
| `0x84DE0117` | `0x04DE` | `0x0118` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0074` |
| `0x84DE0119` | `0x04DE` | `0x011A` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0076` |
| `0x84DE011F` | `0x04DE` | `0x0120` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x007D` |
| `0x84DE0121` | `0x04DE` | `0x0122` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x007F` |
| `0x84DE0123` | `0x04DE` | `0x0124` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0080` |
| `0x84DE0125` | `0x04DE` | `0x0126` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0083` |
| `0x84DE012D` | `0x04DE` | `0x012E` | `0x00D9000D` | `(ref)` | `0x0675#0x04FD` |
| `0x84DE012F` | `0x04DE` | `0x0130` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0087` |
| `0x84DE0133` | `0x04DE` | `0x0134` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0089` |
| `0x84DE0135` | `0x04DE` | `0x0136` | `0x00D9000D` | `(ref)` | `CTrackingSystem#0x000E` |
| `0x84DE0137` | `0x04DE` | `0x0138` | `0x00D9000D` | `(i64, ref)` | `0x0124#0x0006` |
| `0x84DE0139` | `0x04DE` | `0x013A` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x008B` |
| `0x84DE013B` | `0x04DE` | `0x013C` | `0x00D9000D` | `(ref)` | `CTravelSystem#0x0024` |
| `0x84DE013D` | `0x04DE` | `0x013E` | `0x00D9000D` | `` | `0x0675#0x050D` |
| `0x84DE013F` | `0x04DE` | `0x0140` | `0x00D9000D` | `(ref)` | `0x06CB#0x000F` |
| `0x84DE0141` | `0x04DE` | `0x0142` | `0x00D9000D` | `(ref)` | `0x0106#0x000E` |
| `0x84DE0143` | `0x04DE` | `0x0144` | `0x00D9000D` | `(ref)` | `CEffectSystem#0x0013` |
| `0x84DE0145` | `0x04DE` | `0x0146` | `0x00D9000D` | `(ref)` | `CBigBattleSystem#0x000B` |
| `0x84DE0147` | `0x04DE` | `0x0148` | `0x00D9000D` | `(ref)` | `CMultiUsageSystem#0x0017` |
| `0x84DE0149` | `0x04DE` | `0x014A` | `0x00D9000D` | `(ref)` | `CWebStoreSystem#0x0057` |
| `0x84DE014B` | `0x04DE` | `0x014C` | `0x00D9000D` | `(ref)` | `0x0675#0x051E` |
| `0x84DE014D` | `0x04DE` | `0x014E` | `0x00D9000D` | `(ref)` | `0x0F71#0x0001` |
| `0x84DE014F` | `0x04DE` | `0x0150` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x008D` |
| `0x84DE0151` | `0x04DE` | `0x0152` | `0x00D9000D` | `(i64, ref)` | `0x0675#0x0523` |
| `0x84DE0153` | `0x04DE` | `0x0154` | `0x00D9000D` | `(ref)` | `0x0729#0x000C` |
| `0x84DE0155` | `0x04DE` | `0x0156` | `0x00D9000D` | `(ref)` | `0x0F9F#0x0006` |
| `0x84DE0157` | `0x04DE` | `0x0158` | `0x00D9000D` | `(ref)` | `0x0675#0x0529` |
| `0x84DE0159` | `0x04DE` | `0x015A` | `0x00D9000D` | `(i64, ref)` | `CWebStoreSystem#0x0065` |
| `0x84DE015B` | `0x04DE` | `0x015C` | `0x00D9000D` | `(ref)` | `0x1008#0x0001` |
| `0x84DE015D` | `0x04DE` | `0x015E` | `0x00D9000D` | `(ref)` | `0x1011#0x0002` |
| `0x84DE015F` | `0x04DE` | `0x0160` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x008F` |
| `0x84DE0161` | `0x04DE` | `0x0162` | `0x00D9000D` | `(ref)` | `PhysicsSystem#0x0091` |
| `0x84DE0209` | `0x04DE` | `0x020A` | `0x00D9000D` | `(i64, ref)` | `PhysicsSystem#0x00E4` |
| `0x84DE020B` | `0x04DE` | `0x020C` | `0x00D9000D` | `(ref)` | `0x1054#0x004B` |
| `0x84DF001A` | `0x04DF` | `0x0021` | `0x00D9000C` | `(i32, i64)` | `AdvancementSystem#0x0010` |
| `0x84DF0005` | `0x04DF` | `0x0023` | `0x00D9000C` | `(i32, i64)` | `AdvancementSystem#0x000C` |
| `0x84DF002D` | `0x04DF` | `0x002E` | `0x00D9000C` | `(i32, i64)` | `AdvancementSystem#0x00AC` |
| `0x84E10004` | `0x04E1` | `0x0009` | `0x00D9000C` | `(i32)` | `0x06CB#0x0003` |
| `0x84E30001` | `0x04E3` | `0x0002` | `0x00D9000C` | `(ref)` | `0x00FF#0x0008` |
| `0x84E50017` | `0x04E5` | `0x002E` | `0x00D9000D` | `(i64, i64, i64)` | `AuctionSystem#0x0005` |
| `0x84E50003` | `0x04E5` | `0x002F` | `0x00D9000D` | `(i32, i64)` | `AuctionSystem#0x0007` |
| `0x84E5001D` | `0x04E5` | `0x0030` | `0x00D9000D` | `(i32, i64)` | `AuctionSystem#0x0008` |
| `0x84E50015` | `0x04E5` | `0x0031` | `0x00D9000D` | `(i64, i64)` | `AuctionSystem#0x0004` |
| `0x84E5001A` | `0x04E5` | `0x0032` | `0x00D9000D` | `(i64, i32, ref)` | `AuctionSystem::BidOnAuction` |
| `0x84E50018` | `0x04E5` | `0x0033` | `0x00D9000D` | `(i64, ref)` | `AuctionSystem::AddAuction` |
| `0x84E50026` | `0x04E5` | `0x0034` | `0x00D9000D` | `(i32, i64, i32, i32, ref)` | `AuctionSystem#0x0006` |
| `0x84E50035` | `0x04E5` | `0x0037` | `0x00D9000D` | `(i32, i64)` | `AuctionSystem#0x0065` |
| `0x84E50036` | `0x04E5` | `0x0038` | `0x00D9000D` | `(i32, i64)` | `AuctionSystem#0x0066` |
| `0x84E50049` | `0x04E5` | `0x004A` | `0x00D9000D` | `(i32)` | `AuctionSystem#0x00A8` |
| `0x84E80005` | `0x04E8` | `0x0006` | `0x00D9000C` | `` | `0x06E0#0x0001` |
| `0x84E90008` | `0x04E9` | `0x0032` | `0x00D9000C` | `(i32, i64)` | `CCombatSystem#0x0002` |
| `0x84E9002B` | `0x04E9` | `0x0033` | `0x00D9000C` | `(i64)` | `CCombatSystem#0x000D` |
| `0x84E90020` | `0x04E9` | `0x0034` | `0x00D9000C` | `(i64)` | `CCombatSystem#0x000D` |
| `0x84E9004D` | `0x04E9` | `0x0050` | `0x00D9000C` | `(i64, i64)` | `CCombatSystem#0x0045` |
| `0x84EC000E` | `0x04EC` | `0x002E` | `0x00D9000C` | `(ref, ref)` | `CCommunicationSystem#0x0055` |
| `0x84EC0021` | `0x04EC` | `0x002F` | `0x00D9000C` | `(ref)` | `CCommunicationSystem#0x001C` |
| `0x84EC002A` | `0x04EC` | `0x0030` | `0x00D9000C` | `(i32)` | `CCommunicationSystem#0x001F`, `CCommunicationSystem#0x0020` |
| `0x84EC001B` | `0x04EC` | `0x0031` | `0x00D9000C` | `(i32, ref, i32)` | `CCommunicationSystem::Say` |
| `0x84EC0042` | `0x04EC` | `0x0045` | `0x00D9000C` | `(ref, ref)` | `CCommunicationSystem#0x0050` |
| `0x84EC003F` | `0x04EC` | `0x0046` | `0x00D9000C` | `(ref)` | `CCommunicationSystem#0x0053` |
| `0x84EC004D` | `0x04EC` | `0x004C` | `0x00D9000C` | `(i64)` | `CCommunicationSystem#0x0022` |
| `0x84EC004F` | `0x04EC` | `0x004E` | `0x00D9000C` | `(i64, ref)` | `CCommunicationSystem#0x001D` |
| `0x84EC005A` | `0x04EC` | `0x005B` | `0x00D9000C` | `` | `CCommunicationSystem#0x0071` |
| `0x84EC005C` | `0x04EC` | `0x005D` | `0x00D9000C` | `(i32, i64)` | `CCommunicationSystem#0x006D` |
| `0x84EC0088` | `0x04EC` | `0x0089` | `0x00D9000C` | `` | `CCommunicationSystem#0x00BE` |
| `0x84ED0023` | `0x04ED` | `0x0026` | `0x00D9000D` | `(ref)` | `0x0103#0x000C` |
| `0x84EE0002` | `0x04EE` | `0x000C` | `0x00D9000D` | `(i64)` | `0x0106#0x000C` |
| `0x84EF000E` | `0x04EF` | `0x001C` | `0x00D9000C` | `(i32, ref, i32)` | `CCraftSystem#0x000B` |
| `0x84EF0019` | `0x04EF` | `0x001D` | `0x00D9000C` | `(i32, i64)` | `CCraftSystem#0x0007` |
| `0x84EF0008` | `0x04EF` | `0x001E` | `0x00D9000D` | `(ref)` | `CCraftSystem#0x000D` |
| `0x84EF0006` | `0x04EF` | `0x001F` | `0x00D9000C` | `(ref, i64)` | `CCraftSystem#0x0008` |
| `0x84EF003C` | `0x04EF` | `0x0040` | `0x00D9000C` | `(i32, i64)` | `CCraftSystem#0x001F` |
| `0x84EF005B` | `0x04EF` | `0x005C` | `0x00D9000C` | `(i32, i64)` | `CCraftSystem#0x004D` |
| `0x84EF005D` | `0x04EF` | `0x005E` | `0x00D9000C` | `(i32)` | `CCraftSystem#0x004F` |
| `0x84F20001` | `0x04F2` | `0x002D` | `0x00D9000C` | `` | `0x0129#0x0003` |
| `0x84F20026` | `0x04F2` | `0x002E` | `0x00D9000C` | `` | `0x0129#0x0003` |
| `0x84F2000B` | `0x04F2` | `0x002F` | `0x00D9000C` | `(f32)` | `0x0675#0x0399` |
| `0x84F20008` | `0x04F2` | `0x0030` | `0x00D9000C` | `(i32, i32)` | `0x0129#0x0001` |
| `0x84F2001F` | `0x04F2` | `0x0032` | `0x00D9000C` | `` | `0x0129#0x0002` |
| `0x84F30008` | `0x04F3` | `0x0009` | `0x00D9000C` | `` | `0x010A#0x0001` |
| `0x84F30001` | `0x04F3` | `0x000A` | `0x00D9000C` | `(i64)` | `0x010A#0x0002` |
| `0x84F60001` | `0x04F6` | `0x0004` | `0x00D9000C` | `(i64, ref)` | `0x06CE#0x0001` |
| `0x84F70007` | `0x04F7` | `0x0020` | `0x00D9000C` | `` | `0x06D8#0x0003` |
| `0x84F7000D` | `0x04F7` | `0x0021` | `0x00D9000C` | `(i32, i64)` | `0x06E3#0x0001` |
| `0x84F7001B` | `0x04F7` | `0x0022` | `0x00D9000C` | `(i64)` | `0x06D8#0x0002` |
| `0x84F9000A` | `0x04F9` | `0x000B` | `0x00D9000C` | `(ref)` | `EmoteSystem#0x0012` |
| `0x84F90008` | `0x04F9` | `0x000C` | `0x00D9000C` | `(i64, i32)` | `EmoteSystem#0x0024` |
| `0x84F90007` | `0x04F9` | `0x000D` | `0x00D9000C` | `(i32)` | `EmoteSystem#0x0005` |
| `0x84F90005` | `0x04F9` | `0x000E` | `0x00D9000C` | `(ref, i32)` | `EmoteSystem#0x0024` |
| `0x84FD000A` | `0x04FD` | `0x001F` | `0x00D9000C` | `(i64)` | `CExaminationSystem#0x0002` |
| `0x84FD0010` | `0x04FD` | `0x0020` | `0x00D9000C` | `(i64)` | `CExaminationSystem#0x0020` |
| `0x84FD001B` | `0x04FD` | `0x0021` | `0x00D9000C` | `(i32, ref, i32, ref)` | `CExaminationSystem#0x0001` |
| `0x84FD0025` | `0x04FD` | `0x0029` | `0x00D9000C` | `(i64)` | `CExaminationSystem#0x0020`, `CExaminationSystem#0x001F` |
| `0x84FE002B` | `0x04FE` | `0x00A3` | `0x00D9000C` | `(i32)` | `0x0123#0x004C` |
| `0x84FE0049` | `0x04FE` | `0x00A5` | `0x00D9000C` | `(i64)` | `0x0123#0x0045` |
| `0x84FE0042` | `0x04FE` | `0x00A7` | `0x00D9000C` | `(i64)` | `0x0123#0x0047` |
| `0x84FE006F` | `0x04FE` | `0x00A8` | `0x00D9000C` | `(i32)` | `0x0123#0x004D` |
| `0x84FE008E` | `0x04FE` | `0x00A9` | `0x00D9000C` | `` | `0x0123#0x0042` |
| `0x84FE0062` | `0x04FE` | `0x00AA` | `0x00D9000C` | `(i64)` | `0x0123#0x0043` |
| `0x84FE000D` | `0x04FE` | `0x00AB` | `0x00D9000C` | `` | `0x0123#0x0041` |
| `0x84FE0078` | `0x04FE` | `0x00AC` | `0x00D9000C` | `(i32, i64)` | `0x0123#0x0048` |
| `0x84FE0080` | `0x04FE` | `0x00AD` | `0x00D9000C` | `(i32, ref)` | `0x0123#0x007F`, `0x0123#0x0044` |
| `0x84FE007F` | `0x04FE` | `0x00AF` | `0x00D9000C` | `(i64)` | `0x0123#0x0046` |
| `0x84FE00B2` | `0x04FE` | `0x00B3` | `0x00D9000C` | `(i32)` | `0x0123#0x0062` |
| `0x84FE00D9` | `0x04FE` | `0x00DC` | `0x00D9000C` | `` | `0x0123#0x0067` |
| `0x84FE00D5` | `0x04FE` | `0x00DD` | `0x00D9000C` | `(i32)` | `0x074D#0x0001` |
| `0x84FE010D` | `0x04FE` | `0x010E` | `0x00D9000C` | `(i32)` | `0x0123#0x0072` |
| `0x84FE010C` | `0x04FE` | `0x010F` | `0x00D9000C` | `(i32)` | `0x0123#0x0073` |
| `0x84FE011C` | `0x04FE` | `0x011D` | `0x00D9000C` | `(i32, i32)` | `0x0123#0x007B` |
| `0x84FE013F` | `0x04FE` | `0x014B` | `0x00D9000C` | `(i64)` | `0x0123#0x0088` |
| `0x84FE0147` | `0x04FE` | `0x014C` | `0x00D9000C` | `(i64)` | `0x0123#0x0087` |
| `0x84FE0169` | `0x04FE` | `0x016E` | `0x00D9000C` | `(i64)` | `0x0123#0x0098` |
| `0x84FE0168` | `0x04FE` | `0x016F` | `0x00D9000C` | `(i64)` | `CCommunicationSystem#0x0018`, `CCommunicationSystem#0x0025`, `0x0123#0x0099` |
| `0x8502001A` | `0x0502` | `0x00B0` | `0x00D9000C` | `` | `GuildSystem#0x0008` |
| `0x850200A3` | `0x0502` | `0x00B4` | `0x00D9000C` | `(ref)` | `GuildSystem#0x0003` |
| `0x8502000B` | `0x0502` | `0x00B7` | `0x00D9000C` | `` | `GuildSystem#0x0001` |
| `0x85020045` | `0x0502` | `0x00B8` | `0x00D9000C` | `(i64)` | `GuildSystem#0x0010` |
| `0x85020011` | `0x0502` | `0x00B9` | `0x00D9000C` | `(i32, i32)` | `GuildSystem#0x0013` |
| `0x85020012` | `0x0502` | `0x00BB` | `0x00D9000C` | `(ref)` | `GuildSystem#0x008C` |
| `0x85020033` | `0x0502` | `0x00BD` | `0x00D9000C` | `` | `GuildSystem#0x007B` |
| `0x8502002F` | `0x0502` | `0x00BE` | `0x00D9000C` | `` | `GuildSystem#0x0011` |
| `0x85020031` | `0x0502` | `0x00BF` | `0x00D9000C` | `(ref)` | `GuildSystem#0x0005` |
| `0x850200A5` | `0x0502` | `0x00C0` | `0x00D9000C` | `(i64)` | `GuildSystem#0x000A` |
| `0x85020056` | `0x0502` | `0x00C1` | `0x00D9000C` | `(i64)` | `GuildSystem#0x0006` |
| `0x85020065` | `0x0502` | `0x00C3` | `0x00D9000C` | `(ref)` | `GuildSystem#0x0007` |
| `0x8502007C` | `0x0502` | `0x00C5` | `0x00D9000C` | `(i64)` | `GuildSystem#0x0009` |
| `0x8502005B` | `0x0502` | `0x00C7` | `0x00D9000C` | `(i64)` | `GuildSystem#0x000B` |
| `0x85020083` | `0x0502` | `0x00C9` | `0x00D9000C` | `(i32)` | `GuildSystem#0x0012` |
| `0x850200E6` | `0x0502` | `0x00E8` | `0x00D9000C` | `(ref)` | `GuildSystem::UpdatePlayerNote` |
| `0x85020231` | `0x0502` | `0x0232` | `0x00D9000D` | `(ref)` | `GuildSystem#0x02EE` |
| `0x85020230` | `0x0502` | `0x0233` | `0x00D9000D` | `(i64)` | `GuildSystem#0x02ED` |
| `0x8502022F` | `0x0502` | `0x0234` | `0x00D9000D` | `(ref)` | `GuildSystem#0x02EC` |
| `0x85020243` | `0x0502` | `0x0244` | `0x00D9000D` | `(ref)` | `GuildSystem#0x0303` |
| `0x85060009` | `0x0506` | `0x000A` | `0x00D9000C` | `` | `SkillSystem#0x00D3` |
| `0x8507000F` | `0x0507` | `0x0012` | `0x00D9000C` | `(ref, i64)` | `0x016F#0x0001` |
| `0x85080008` | `0x0508` | `0x000C` | `0x00D9000D` | `(i64)` | `0x0675#0x03B0` |
| `0x85080007` | `0x0508` | `0x000D` | `0x00D9000D` | `(i32)` | `0x0675#0x0415` |
| `0x850A000B` | `0x050A` | `0x0021` | `0x00D9000C` | `(i32, i32, i64, i64)` | `0x0AF4#0x0005` |
| `0x850A0001` | `0x050A` | `0x0022` | `0x00D9000C` | `(i32, i64)` | `0x0AF4#0x0014` |
| `0x850A000C` | `0x050A` | `0x0023` | `0x00D9000C` | `(i32, i64)` | `0x0AF4#0x0002` |
| `0x850A0016` | `0x050A` | `0x0024` | `0x00D9000C` | `(i32, i64)` | `0x0AF4#0x000C` |
| `0x850A001D` | `0x050A` | `0x0025` | `0x00D9000C` | `(i32, i32, i64)` | `0x0AF4#0x0006` |
| `0x850A0019` | `0x050A` | `0x0027` | `0x00D9000C` | `(i32, i64)` | `0x0AF4#0x0004` |
| `0x850A0034` | `0x050A` | `0x0035` | `0x00D9000C` | `(i64)` | `0x0AF4#0x0017` |
| `0x850A0038` | `0x050A` | `0x0039` | `0x00D9000C` | `(ref)` | `0x0AF4#0x0022` |
| `0x850B0002` | `0x050B` | `0x000D` | `0x00D9000C` | `(i64, i64)` | `0x0171#0x0006` |
| `0x850B000B` | `0x050B` | `0x000E` | `0x00D9000C` | `(i64, ref)` | `0x0171#0x000B` |
| `0x850C0059` | `0x050C` | `0x0064` | `0x00D9000C` | `(i32)` | `0x1021#0x0006` |
| `0x850C00BC` | `0x050C` | `0x00C0` | `0x00D9000C` | `` | `0x1021#0x0057` |
| `0x850C00EC` | `0x050C` | `0x011F` | `0x00D9000C` | `` | `0x1021#0x0076` |
| `0x850C011A` | `0x050C` | `0x0120` | `0x00D9000C` | `(ref)` | `0x1021#0x0087` |
| `0x850C0110` | `0x050C` | `0x0121` | `0x00D9000C` | `(ref)` | `0x1021#0x0084` |
| `0x850C00E0` | `0x050C` | `0x0122` | `0x00D9000C` | `(i64)` | `0x1021#0x0086` |
| `0x850C00F9` | `0x050C` | `0x0123` | `0x00D9000C` | `(i64)` | `0x1021#0x0069` |
| `0x850C00E9` | `0x050C` | `0x0124` | `0x00D9000C` | `` | `0x1021#0x0067` |
| `0x850C0132` | `0x050C` | `0x0133` | `0x00D9000C` | `(i32, i64)` | `0x1021#0x0092` |
| `0x850C012F` | `0x050C` | `0x0134` | `0x00D9000C` | `(i32, i64)` | `0x1021#0x0093` |
| `0x85100001` | `0x0510` | `0x0009` | `0x00D9000C` | `` | `0x018D#0x0004` |
| `0x85100006` | `0x0510` | `0x000A` | `0x00D9000C` | `` | `0x06EA#0x0001` |
| `0x85100018` | `0x0510` | `0x0019` | `0x00D9000C` | `` | `0x018D#0x000C` |
| `0x85100017` | `0x0510` | `0x001A` | `0x00D9000C` | `` | `0x0701#0x0001` |
| `0x85110001` | `0x0511` | `0x0006` | `0x00D9000D` | `(ref)` | `0x0675#0x0404` |
| `0x85120014` | `0x0512` | `0x0023` | `0x00D9000C` | `(i64, i64)` | `0x0112#0x0003` |
| `0x8512001B` | `0x0512` | `0x0024` | `0x00D9000C` | `(i32, i64)` | `0x0112#0x0007`, `0x0112#0x0002` |
| `0x85120025` | `0x0512` | `0x0026` | `0x00D9000C` | `(i64)` | `0x0112#0x0009` |
| `0x85120029` | `0x0512` | `0x002C` | `0x00D9000C` | `(i64, i64)` | `0x0751#0x0001` |
| `0x8512003F` | `0x0512` | `0x0040` | `0x00D9000C` | `` | `0x0112#0x0011` |
| `0x85120042` | `0x0512` | `0x0043` | `0x00D9000C` | `(i32, i32, i32)` | `0x0112#0x0014` |
| `0x85120041` | `0x0512` | `0x0044` | `0x00D9000C` | `(i32, i32, i32)` | `0x0112#0x0015` |
| `0x85120053` | `0x0512` | `0x0054` | `0x00D9000C` | `` | `0x0112#0x001B` |
| `0x85130016` | `0x0513` | `0x0028` | `0x00D9000C` | `(i64)` | `CMEOInventorySystem#0x0001` |
| `0x8513002D` | `0x0513` | `0x002E` | `0x00D9000C` | `(i64)` | `CMEOInventorySystem#0x0002` |
| `0x8514003B` | `0x0514` | `0x004C` | `0x00D9000D` | `(i64, i64)` | `MailSystem#0x0005` |
| `0x85140048` | `0x0514` | `0x004D` | `0x00D9000D` | `(i32, ref, i32, i32, ref, ref, ref, i64)` | `MailSystem#0x0009` |
| `0x85140040` | `0x0514` | `0x004E` | `0x00D9000D` | `(i64, i64)` | `MailSystem#0x0004` |
| `0x85140012` | `0x0514` | `0x0050` | `0x00D9000D` | `(i64, i64)` | `MailSystem#0x00C8` |
| `0x85140015` | `0x0514` | `0x0051` | `0x00D9000D` | `(i64, i64)` | `MailSystem#0x0007` |
| `0x8514006F` | `0x0514` | `0x0070` | `0x00D9000D` | `(i64, i64)` | `MailSystem#0x007E` |
| `0x85140074` | `0x0514` | `0x0075` | `0x00D9000D` | `(i64)` | `0x0627#0x0005` |
| `0x85140092` | `0x0514` | `0x0093` | `0x00D9000D` | `(i64, i64)` | `MailSystem#0x00A6` |
| `0x85150002` | `0x0515` | `0x0005` | `0x00D9000C` | `(i32)` | `0x01B6#0x0001` |
| `0x85150001` | `0x0515` | `0x0006` | `0x00D9000C` | `(ref, i32)` | `0x01B6#0x0002` |
| `0x85180004` | `0x0518` | `0x0007` | `0x00D9000C` | `(i32)` | `0x0175#0x0002`, `0x0175#0x0003`, `0x0175#0x0004`, (+1) |
| `0x85180006` | `0x0518` | `0x0009` | `0x00D9000C` | `(ref)` | `0x0175#0x0001` |
| `0x85190001` | `0x0519` | `0x000A` | `0x00D9000C` | `` | `0x0116#0x0009` |
| `0x85190004` | `0x0519` | `0x000B` | `0x00D9000C` | `` | `0x0116#0x0009` |
| `0x8519000E` | `0x0519` | `0x0013` | `0x00D9000C` | `(i32, f64, i32)` | `0x0116#0x0014` |
| `0x85190010` | `0x0519` | `0x0014` | `0x00D9000C` | `(i32, f64, i32, i32, i32)` | `0x0116#0x0015` |
| `0x85190018` | `0x0519` | `0x001F` | `0x00D9000C` | `` | `0x0116#0x001F` |
| `0x8519001E` | `0x0519` | `0x0020` | `0x00D9000C` | `(ref)` | `0x0116#0x0022` |
| `0x85190023` | `0x0519` | `0x0024` | `0x00D9000C` | `(ref)` | `0x0116#0x0027` |
| `0x85190025` | `0x0519` | `0x0028` | `0x00D9000C` | `` | `0x0116#0x0029` |
| `0x851B0001` | `0x051B` | `0x0021` | `0x00D9000D` | `(i32)` | `0x0119#0x0002` |
| `0x851B0016` | `0x051B` | `0x0022` | `0x00D9000D` | `` | `0x0119#0x000D` |
| `0x851B000F` | `0x051B` | `0x0023` | `0x00D9000D` | `(i32)` | `0x0119#0x000E` |
| `0x851B0015` | `0x051B` | `0x0024` | `0x00D9000D` | `(i64, ref)` | `0x0119#0x0003` |
| `0x851B0013` | `0x051B` | `0x0025` | `0x00D9000D` | `(i32, i32, i64)` | `0x06E2#0x0001` |
| `0x851B0004` | `0x051B` | `0x0026` | `0x00D9000D` | `(i32, i64)` | `0x0119#0x0008` |
| `0x851C0004` | `0x051C` | `0x000D` | `0x00D9000C` | `(i32, i64)` | `0x011A#0x0006` |
| `0x851E0020` | `0x051E` | `0x0022` | `0x00D9000C` | `(ref, i32)` | `ShortcutSystem#0x0011` |
| `0x851E0019` | `0x051E` | `0x0024` | `0x00D9000C` | `(i32, ref, ref)` | `Admin#0x02A1`, `0x0176#0x0011` |
| `0x851E0006` | `0x051E` | `0x0025` | `0x00D9000C` | `` | `Admin#0x0288` |
| `0x851E0004` | `0x051E` | `0x0026` | `0x00D9000C` | `(i32)` | `Admin#0x0248` |
| `0x851E000B` | `0x051E` | `0x0027` | `0x00D9000C` | `` | `Admin#0x0251` |
| `0x851E001E` | `0x051E` | `0x0028` | `0x00D9000C` | `(i64)` | `Admin#0x0277` |
| `0x851E0005` | `0x051E` | `0x0029` | `0x00D9000C` | `(ref, ref)` | `Admin#0x02CC` |
| `0x851E0016` | `0x051E` | `0x002A` | `0x00D9000C` | `(ref)` | `Admin#0x034B` |
| `0x851E002F` | `0x051E` | `0x0030` | `0x00D9000C` | `` | `Admin#0x02D5` |
| `0x851E0031` | `0x051E` | `0x0032` | `0x00D9000C` | `(ref, i32)` | `Admin#0x0318` |
| `0x851E0037` | `0x051E` | `0x0038` | `0x00D9000C` | `` | `Admin#0x0335` |
| `0x851F0085` | `0x051F` | `0x00BC` | `0x00D9000C` | `` | `0x06E9#0x0001` |
| `0x851F002C` | `0x051F` | `0x00BF` | `0x00D9000C` | `(ref, i64)` | `0x011C#0x0002` |
| `0x851F0136` | `0x051F` | `0x0137` | `0x00D9000C` | `` | `0x011C#0x0014` |
| `0x851F0162` | `0x051F` | `0x0163` | `0x00D9000C` | `` | `0x011C#0x0017` |
| `0x851F016C` | `0x051F` | `0x016D` | `0x00D9000C` | `` | `0x011C#0x0019` |
| `0x85210013` | `0x0521` | `0x0083` | `0x00D9000C` | `(i32)` | `CQuestSystem#0x002D` |
| `0x8521006B` | `0x0521` | `0x0084` | `0x00D9000C` | `(i32, i64, i32)` | `CQuestSystem#0x0003` |
| `0x8521002F` | `0x0521` | `0x0085` | `0x00D9000C` | `(i32, i32)` | `CQuestSystem#0x0032` |
| `0x85210082` | `0x0521` | `0x0086` | `0x00D9000C` | `` | `CQuestSystem#0x0027` |
| `0x8521004B` | `0x0521` | `0x0087` | `0x00D9000C` | `(i32)` | `CQuestSystem#0x0002` |
| `0x8521003D` | `0x0521` | `0x0088` | `0x00D9000C` | `(i32)` | `CQuestSystem#0x0042` |
| `0x85210057` | `0x0521` | `0x0089` | `0x00D9000C` | `(i32)` | `CQuestSystem#0x0025` |
| `0x85210008` | `0x0521` | `0x008A` | `0x00D9000C` | `(i32)` | `CQuestSystem#0x0024` |
| `0x852100B1` | `0x0521` | `0x00B2` | `0x00D9000C` | `(i32, i64)` | `CQuestSystem#0x001E` |
| `0x852100D6` | `0x0521` | `0x00D7` | `0x00D9000C` | `(i32, i32)` | `CQuestSystem#0x0080` |
| `0x852100F7` | `0x0521` | `0x00F6` | `0x00D90003` | `(i64)` | `CQuestJournal#0x0062` |
| `0x852100F8` | `0x0521` | `0x00F9` | `0x00D90003` | `(i64)` | `CQuestJournal#0x0093`, `CQuestJournal#0x00A7` |
| `0x852100FA` | `0x0521` | `0x00FB` | `0x00D90003` | `(i64)` | `0x0C0E#0x000E` |
| `0x85210111` | `0x0521` | `0x0112` | `0x00D9000C` | `(i32)` | `CQuestSystem#0x00C6` |
| `0x85220006` | `0x0522` | `0x000B` | `0x00D9000D` | `(ref, i32, i64)` | `0x011E#0x0001` |
| `0x85220005` | `0x0522` | `0x000C` | `0x00D9000D` | `(i64)` | `CAdmin::DoCmdGetQuestRoleData` |
| `0x85230010` | `0x0523` | `0x0023` | `0x00D9000C` | `` | `0x0123#0x0007` |
| `0x85230022` | `0x0523` | `0x0024` | `0x00D9000C` | `(i64)` | `0x0123#0x0001` |
| `0x85230016` | `0x0523` | `0x0025` | `0x00D9000C` | `(i64, i64)` | `0x0123#0x0014` |
| `0x8523001A` | `0x0523` | `0x0026` | `0x00D9000C` | `(i64)` | `0x0123#0x0002` |
| `0x85230008` | `0x0523` | `0x0027` | `0x00D9000C` | `` | `0x0123#0x000E` |
| `0x85230017` | `0x0523` | `0x0028` | `0x00D9000C` | `(i64, i64)` | `0x0123#0x0013` |
| `0x85230039` | `0x0523` | `0x003E` | `0x00D9000C` | `(ref)` | `0x0123#0x001D` |
| `0x85230045` | `0x0523` | `0x0046` | `0x00D9000C` | `(i64)` | `0x0123#0x0021` |
| `0x8524000C` | `0x0524` | `0x0010` | `0x00D9000C` | `(i32)` | `0x06DB#0x0003` |
| `0x8524000E` | `0x0524` | `0x0011` | `0x00D9000C` | `(i32)` | `0x0C47#0x0005`, `0x0628#0x0006` |
| `0x85240002` | `0x0524` | `0x0012` | `0x00D9000C` | `(i32)` | `0x06DB#0x0009` |
| `0x85240015` | `0x0524` | `0x0016` | `0x00D9000C` | `` | `0x06DB#0x000C` |
| `0x85280008` | `0x0528` | `0x0012` | `0x00D9000C` | `` | `SkillSystem#0x00A2` |
| `0x85280002` | `0x0528` | `0x0013` | `0x00D9000C` | `` | `SkillSystem#0x00B9` |
| `0x85280005` | `0x0528` | `0x0014` | `0x00D9000C` | `(ref)` | `SkillSystem#0x0149` |
| `0x85280006` | `0x0528` | `0x0015` | `0x00D9000C` | `(i32, i64)` | `CUsageSystem#0x0020` |
| `0x85280007` | `0x0528` | `0x0016` | `0x00D9000C` | `` | `SkillSystem#0x0018` |
| `0x852A0003` | `0x052A` | `0x0006` | `0x00D9000C` | `(i32, ref, i64)` | `0x06DD#0x0003`, `0x06DD#0x0002` |
| `0x852C000C` | `0x052C` | `0x000E` | `0x00D9000C` | `(i64)` | `CTextItemSystem#0x0007` |
| `0x852C0006` | `0x052C` | `0x000F` | `0x00D9000C` | `(i64, i64)` | `TextItemActionHandler::HandleAction` |
| `0x852C0005` | `0x052C` | `0x0010` | `0x00D9000C` | `(ref, ref, i64)` | `CTextItemSystem::WriteCustomTextItem` |
| `0x852D0008` | `0x052D` | `0x0009` | `0x00D9000C` | `` | `0x017D#0x0003` |
| `0x852D0005` | `0x052D` | `0x000A` | `0x00D9000C` | `` | `0x017D#0x0002` |
| `0x852D0002` | `0x052D` | `0x000B` | `0x00D9000D` | `(ref)` | `0x0675#0x0405` |
| `0x852D0006` | `0x052D` | `0x000C` | `0x00D9000C` | `` | `0x017D#0x0004` |
| `0x852E0009` | `0x052E` | `0x0018` | `0x00D9000C` | `(ref, i64)` | `TitleSystem#0x0023` |
| `0x852E0012` | `0x052E` | `0x0019` | `0x00D9000C` | `(i64)` | `TitleSystem#0x0021` |
| `0x852E0015` | `0x052E` | `0x001A` | `0x00D9000C` | `(ref, i64)` | `TitleSystem#0x0024` |
| `0x852E0001` | `0x052E` | `0x001B` | `0x00D9000C` | `(i64)` | `TitleSystem#0x0020` |
| `0x852E000E` | `0x052E` | `0x001C` | `0x00D9000C` | `(i32)` | `TitleSystem#0x0005` |
| `0x852E0010` | `0x052E` | `0x001D` | `0x00D9000C` | `` | `TitleSystem#0x0006` |
| `0x852E0014` | `0x052E` | `0x001E` | `0x00D9000C` | `(ref, i64)` | `TitleSystem#0x0028` |
| `0x852E0021` | `0x052E` | `0x0023` | `0x00D9000D` | `(ref)` | `TitleSystem#0x002F` |
| `0x85310050` | `0x0531` | `0x0069` | `0x00D9000C` | `(ref)` | `0x017F#0x0003` |
| `0x8531002E` | `0x0531` | `0x006A` | `0x00D9000C` | `(ref)` | `0x017F#0x0006` |
| `0x85310042` | `0x0531` | `0x006B` | `0x00D9000C` | `` | `0x0675#0x044C` |
| `0x85310043` | `0x0531` | `0x006C` | `0x00D9000C` | `(i64)` | `0x017F#0x0001` |
| `0x85310060` | `0x0531` | `0x006D` | `0x00D9000C` | `(i64)` | `0x017F#0x0007` |
| `0x85310024` | `0x0531` | `0x006E` | `0x00D9000C` | `(i64)` | `0x017F#0x0005` |
| `0x85310025` | `0x0531` | `0x006F` | `0x00D9000C` | `(i64)` | `0x0675#0x044C` |
| `0x8531005D` | `0x0531` | `0x0070` | `0x00D9000C` | `` | `0x017F#0x0002` |
| `0x85310034` | `0x0531` | `0x0071` | `0x00D9000C` | `` | `0x017F#0x0004` |
| `0x8532001F` | `0x0532` | `0x0023` | `0x00D9000C` | `` | `CTraitSystem#0x0009` |
| `0x85320021` | `0x0532` | `0x0024` | `0x00D9000C` | `` | `CTraitSystem#0x0008` |
| `0x85320008` | `0x0532` | `0x0025` | `0x00D9000C` | `` | `CTraitSystem#0x0007` |
| `0x85320016` | `0x0532` | `0x0026` | `0x00D9000C` | `(ref, i32)` | `CTraitSystem#0x0033` |
| `0x8532000E` | `0x0532` | `0x0027` | `0x00D9000C` | `(i32)` | `CTraitSystem#0x0004` |
| `0x8532003D` | `0x0532` | `0x003E` | `0x00D9000C` | `(ref, i32)` | `CTraitSystem#0x0050` |
| `0x85320045` | `0x0532` | `0x0046` | `0x00D9000C` | `` | `CTraitSystem#0x0059` |
| `0x85320049` | `0x0532` | `0x004A` | `0x00D9000C` | `(i32)` | `CTraitSystem#0x0061` |
| `0x8532004B` | `0x0532` | `0x004E` | `0x00D9000C` | `(i32, ref, i32)` | `CTraitSystem#0x006A` |
| `0x8532004F` | `0x0532` | `0x0050` | `0x00D9000C` | `(i32, i32)` | `CTraitSystem#0x0067` |
| `0x85320057` | `0x0532` | `0x0058` | `0x00D9000C` | `(i32)` | `CTraitSystem#0x003C` |
| `0x85330005` | `0x0533` | `0x0012` | `0x00D9000C` | `` | `CTravelSystem#0x0009` |
| `0x85330004` | `0x0533` | `0x0013` | `0x00D9000C` | `(i32)` | `CTravelSystem#0x000E` |
| `0x85330008` | `0x0533` | `0x0014` | `0x00D9000C` | `(i32, i32, i64)` | `CTravelSystem#0x0014` |
| `0x85330003` | `0x0533` | `0x0015` | `0x00D9000C` | `(ref)` | `CTravelSystem#0x000E` |
| `0x8533000D` | `0x0533` | `0x0016` | `0x00D9000C` | `(i32, i64)` | `CTravelSystem#0x0013` |
| `0x85330022` | `0x0533` | `0x0023` | `0x00D9000C` | `(i64)` | `CTravelSystem#0x001E` |
| `0x85340005` | `0x0534` | `0x0007` | `0x00D9000C` | `(i32)` | `0x0126#0x0006` |
| `0x85340006` | `0x0534` | `0x0008` | `0x00D9000C` | `` | `0x0126#0x0004` |
| `0x85350002` | `0x0535` | `0x002D` | `0x00D9000C` | `(ref)` | `CUsageSystem#0x001D` |
| `0x8535001D` | `0x0535` | `0x002E` | `0x00D9000C` | `(ref)` | `CUsageSystem#0x004B`, `CUsageSystem#0x0001`, `CUsageSystem#0x0002` |
| `0x8535003A` | `0x0535` | `0x003B` | `0x00D9000C` | `(i32)` | `CUsageSystem#0x003E` |
| `0x8536000A` | `0x0536` | `0x0011` | `0x00D9000C` | `(ref, i64)` | `VendorSystem#0x004D`, `VendorSystem#0x0048` |
| `0x8536000E` | `0x0536` | `0x0012` | `0x00D9000C` | `(i32, i64)` | `VendorSystem#0x004F` |
| `0x85360007` | `0x0536` | `0x0013` | `0x00D9000D` | `(i64)` | `0x0675#0x0466` |
| `0x85360002` | `0x0536` | `0x0014` | `0x00D9000C` | `(i32, i64)` | `VendorSystem#0x0049` |
| `0x85360001` | `0x0536` | `0x0015` | `0x00D9000C` | `(i32, i32, i64)` | `VendorSystem#0x0047` |
| `0x85360016` | `0x0536` | `0x0017` | `0x00D9000C` | `(i32, i64)` | `VendorSystem#0x0057` |
| `0x85370017` | `0x0537` | `0x0018` | `0x00D9000C` | `(i64)` | `VitalSystem#0x0051` |
| `0x8537001E` | `0x0537` | `0x001F` | `0x00D9000C` | `(i32)` | `PlayerActionHandler#0x0056` |
| `0x871A0017` | `0x071A` | `0x001A` | `0x00D9000C` | `(i32)` | `0x071D#0x0006` |
| `0x87390011` | `0x0739` | `0x0014` | `0x00D9000C` | `(i32, i64)` | `0x0736#0x0005` |
| `0x87390015` | `0x0739` | `0x0016` | `0x00D9000C` | `(i32, i64)` | `0x0736#0x0010` |
| `0x877F0025` | `0x077F` | `0x0029` | `0x00D9000C` | `` | `0x0786#0x0007` |
| `0x877F0028` | `0x077F` | `0x002B` | `0x00D9000C` | `(i32, i64)` | `0x0786#0x0003` |
| `0x877F0032` | `0x077F` | `0x003C` | `0x00D9000C` | `(i32)` | `0x0786#0x0010` |
| `0x877F003B` | `0x077F` | `0x003D` | `0x00D9000C` | `(i32)` | `0x0786#0x000F` |
| `0x878E0003` | `0x078E` | `0x0006` | `0x00D9000C` | `` | `0x0787#0x0002` |
| `0x87930006` | `0x0793` | `0x0005` | `0x00D9000C` | `(ref, i64)` | `0x0795#0x0002`, `0x0795#0x0005` |
| `0x87A70004` | `0x07A7` | `0x0003` | `0x00D9000C` | `(i32, i32, i64)` | `0x07A6#0x0005` |
| `0x87A7002F` | `0x07A7` | `0x0030` | `0x00D9000C` | `(i32)` | `0x07A6#0x000B` |
| `0x87A70031` | `0x07A7` | `0x0033` | `0x00D9000C` | `(i32, i32, i32, i32, i32, i64)` | `0x07A6#0x0006`, `0x0FF4#0x0001` |
| `0x87A70032` | `0x07A7` | `0x0034` | `0x00D9000C` | `(i32, i32, i32, i64)` | `0x07A6#0x0007` |
| `0x87A70043` | `0x07A7` | `0x0046` | `0x00D9000C` | `(i32, i32, i64)` | `0x07A6#0x0010` |
| `0x87A7004F` | `0x07A7` | `0x0054` | `0x00D9000C` | `(ref, i32, i32, i64)` | `0x07A6#0x0018` |
| `0x87A70060` | `0x07A7` | `0x0063` | `0x00D9000C` | `(i32, i32, i32, i64)` | `0x07A6#0x008D` |
| `0x87A70064` | `0x07A7` | `0x0069` | `0x00D9000C` | `(i32, i32, i64)` | `0x07A6#0x001D` |
| `0x87A70070` | `0x07A7` | `0x0072` | `0x00D9000C` | `(i32, i64, i32)` | `0x07A6#0x0029` |
| `0x87A70077` | `0x07A7` | `0x007E` | `0x00D9000C` | `(i32, i32)` | `0x07A6#0x002D` |
| `0x87A7008C` | `0x07A7` | `0x008D` | `0x00D9000C` | `` | `0x07A6#0x0035` |
| `0x87A70090` | `0x07A7` | `0x0091` | `0x00D9000C` | `(i64)` | `0x07A6#0x0038` |
| `0x87A700B3` | `0x07A7` | `0x00B4` | `0x00D9000C` | `(i64)` | `0x07A6#0x003F` |
| `0x87A700BB` | `0x07A7` | `0x00BE` | `0x00D9000C` | `(i32)` | `0x07A6#0x0040` |
| `0x87A700FF` | `0x07A7` | `0x0103` | `0x00D9000C` | `(i32)` | `0x07A6#0x0049` |
| `0x87A70123` | `0x07A7` | `0x0124` | `0x00D9000C` | `(i32)` | `0x07A6#0x0060` |
| `0x87A7014C` | `0x07A7` | `0x014D` | `0x00D9000C` | `(i64, ref)` | `0x07A6#0x006A` |
| `0x87A70169` | `0x07A7` | `0x016A` | `0x00D9000C` | `(i32, i32)` | `0x07A6#0x0073` |
| `0x87A7016F` | `0x07A7` | `0x0170` | `0x00D9000C` | `(f32, i64, i32)` | `0x07A6#0x0075` |
| `0x87A70177` | `0x07A7` | `0x0178` | `0x00D9000C` | `(i32)` | `0x07A6#0x0082` |
| `0x87A70186` | `0x07A7` | `0x0187` | `0x00D9000C` | `` | `0x07A6#0x0089` |
| `0x87A701A8` | `0x07A7` | `0x01A9` | `0x00D9000C` | `(i32, i32)` | `0x07A6#0x008C` |
| `0x87A701F0` | `0x07A7` | `0x01F1` | `0x00D9000C` | `(i64)` | `0x07A6#0x009C` |
| `0x87A701F2` | `0x07A7` | `0x01F5` | `0x00D9000C` | `(ref, i64, i32)` | `0x07A6#0x00A3` |
| `0x87A7020F` | `0x07A7` | `0x0211` | `0x00D9000C` | `(i64)` | `0x07A6#0x00AF` |
| `0x87A70210` | `0x07A7` | `0x0212` | `0x00D9000C` | `(i64)` | `0x07A6#0x00AE` |
| `0x88000006` | `0x0800` | `0x0007` | `0x00D9000C` | `` | `CMountSystem#0x0001`, `CMountSystem#0x0010` |
| `0x88000008` | `0x0800` | `0x0009` | `0x00D9000C` | `(i32, ref)` | `CMountSystem#0x0004` |
| `0x881B0005` | `0x081B` | `0x0006` | `0x00D9000C` | `(i32)` | `OutfitSystem::SelectOutfit` |
| `0x881B0007` | `0x081B` | `0x0008` | `0x00D9000C` | `(ref, i64, i32, i32)` | `OutfitSystem::OutfitEquip` |
| `0x881B000B` | `0x081B` | `0x000C` | `0x00D9000C` | `(i32, i32)` | `OutfitSystem::OutfitUnEquip` |
| `0x881B000D` | `0x081B` | `0x000E` | `0x00D9000C` | `(i32, i32, i32)` | `OutfitSystem#0x0053` |
| `0x883A0001` | `0x083A` | `0x0004` | `0x00D9000C` | `(ref)` | `0x0837#0x000D` |
| `0x8849000D` | `0x0849` | `0x000E` | `0x00D9000C` | `(i32, i64, i64)` | `CItemAdvancementSystem#0x0010` |
| `0x88490015` | `0x0849` | `0x0016` | `0x00D9000C` | `(ref, i64, i64)` | `CItemAdvancementSystem#0x0013` |
| `0x88490017` | `0x0849` | `0x0018` | `0x00D9000C` | `(i64)` | `CItemAdvancementSystem#0x0015` |
| `0x8849001B` | `0x0849` | `0x001C` | `0x00D9000C` | `(i32, i32, i64, i64)` | `CItemAdvancementSystem#0x001A` |
| `0x88490027` | `0x0849` | `0x0028` | `0x00D9000C` | `(i32, i32, i64)` | `CItemAdvancementSystem#0x006D` |
| `0x88490029` | `0x0849` | `0x002A` | `0x00D9000C` | `(i32, i64)` | `CItemAdvancementSystem#0x0027`, `0x08F1#0x0001` |
| `0x8849002F` | `0x0849` | `0x0030` | `0x00D9000C` | `(i32, i32, i64, ref)` | `CItemAdvancementSystem#0x002F` |
| `0x88490035` | `0x0849` | `0x0036` | `0x00D9000C` | `(i32, i64)` | `CItemAdvancementSystem#0x0039` |
| `0x8849003D` | `0x0849` | `0x003E` | `0x00D9000C` | `(i64)` | `CItemAdvancementSystem#0x0045` |
| `0x88490061` | `0x0849` | `0x0062` | `0x00D9000C` | `(i32, i64)` | `CItemAdvancementSystem#0x007D` |
| `0x88490069` | `0x0849` | `0x006C` | `0x00D9000C` | `(i64)` | `CItemAdvancementSystem#0x0093` |
| `0x88490079` | `0x0849` | `0x007A` | `0x00D9000C` | `(i64, i64)` | `CItemAdvancementSystem#0x00AE` |
| `0x88490091` | `0x0849` | `0x0092` | `0x00D9000C` | `(ref, i64)` | `CItemAdvancementSystem#0x00C7` |
| `0x88490093` | `0x0849` | `0x0094` | `0x00D9000C` | `(i32, i64, ref)` | `CItemAdvancementSystem#0x00C9` |
| `0x88490095` | `0x0849` | `0x0096` | `0x00D9000C` | `(i32, i64)` | `CItemAdvancementSystem#0x00CA` |
| `0x88490099` | `0x0849` | `0x009A` | `0x00D9000C` | `(i64)` | `CItemAdvancementSystem#0x00CD` |
| `0x88590001` | `0x0859` | `0x0002` | `0x00D9000C` | `(i32)` | `0x084E#0x0003` |
| `0x88590003` | `0x0859` | `0x0004` | `0x00D9000C` | `(i32)` | `0x084E#0x0004` |
| `0x88CB0003` | `0x08CB` | `0x0004` | `0x00D9000C` | `(i32, i64)` | `0x08CC#0x0008` |
| `0x89080001` | `0x0908` | `0x0002` | `0x00D9000C` | `` | `0x0909#0x0002` |
| `0x89580003` | `0x0958` | `0x0004` | `0x00D9000C` | `(ref)` | `0x0675#0x0502` |
| `0x89580008` | `0x0958` | `0x0009` | `0x00D9000C` | `` | `0x0959#0x0008` |
| `0x89580007` | `0x0958` | `0x000A` | `0x00D9000C` | `` | `0x0959#0x0007` |
| `0x896D0004` | `0x096D` | `0x0003` | `0x00D9000D` | `(ref)` | `0x096F#0x0001` |
| `0x89EB0004` | `0x09EB` | `0x0006` | `0x00D9000D` | `` | `CWebStoreSystem#0x0001` |
| `0x8A04000B` | `0x0A04` | `0x000C` | `0x00D9000C` | `(i32, i32, ref, i64, i64)` | `0x0A02#0x0003` |
| `0x8A040013` | `0x0A04` | `0x0014` | `0x00D9000C` | `(i32, i32, i64, ref, i64, i64)` | `0x0A02#0x0006` |
| `0x8A04001B` | `0x0A04` | `0x001C` | `0x00D9000C` | `(ref)` | `0x0A02#0x0015` |
| `0x8A04001F` | `0x0A04` | `0x0020` | `0x00D9000C` | `(i32, i32, ref, ref, i64, i64)` | `0x0A02#0x0017` |
| `0x8A040021` | `0x0A04` | `0x0022` | `0x00D9000C` | `(i32, ref, i64, i64)` | `0x0A02#0x0019` |
| `0x8A040029` | `0x0A04` | `0x002A` | `0x00D9000C` | `(ref, i64)` | `0x0A02#0x0020` |
| `0x8A04002D` | `0x0A04` | `0x0032` | `0x00D9000C` | `` | `0x0A02#0x0025` |
| `0x8A04003B` | `0x0A04` | `0x003C` | `0x00D9000C` | `(ref, i64, i64)` | `0x0A02#0x0029` |
| `0x8A04003F` | `0x0A04` | `0x0040` | `0x00D9000C` | `(ref, i64, i64)` | `0x0A02#0x002D` |
| `0x8A2C0001` | `0x0A2C` | `0x0002` | `0x00D9000C` | `` | `0x0A2B#0x0002` |
| `0x8A2C0005` | `0x0A2C` | `0x0006` | `0x00D9000C` | `(ref)` | `0x0A2B#0x0018` |
| `0x8A2C0007` | `0x0A2C` | `0x0008` | `0x00D9000C` | `` | `0x0A2B#0x0008` |
| `0x8A2C0009` | `0x0A2C` | `0x000A` | `0x00D9000C` | `(ref)` | `0x0A2B#0x000A` |
| `0x8A2C002F` | `0x0A2C` | `0x0030` | `0x00D9000C` | `(ref)` | `0x0A2B#0x000F` |
| `0x8A2C0045` | `0x0A2C` | `0x0048` | `0x00D9000C` | `(ref)` | `0x0A2B#0x0013` |
| `0x8A2C0051` | `0x0A2C` | `0x0052` | `0x00D9000C` | `` | `0x0A2B#0x001A` |
| `0x8A2C0059` | `0x0A2C` | `0x005E` | `0x00D9000C` | `` | `0x0A2B#0x0021` |
| `0x8A450003` | `0x0A45` | `0x0004` | `0x00D9000C` | `` | `0x0A43#0x0004` |
| `0x8A450007` | `0x0A45` | `0x0008` | `0x00D9000C` | `(i64, i32)` | `0x0A6A#0x0001` |
| `0x8A450009` | `0x0A45` | `0x000A` | `0x00D9000C` | `(i64, i64)` | `0x0A43#0x0009` |
| `0x8A45000B` | `0x0A45` | `0x000C` | `0x00D9000C` | `(i64, i64, ref)` | `0x0A43#0x000B` |
| `0x8A45000D` | `0x0A45` | `0x000E` | `0x00D9000C` | `(i64, i32, i64)` | `0x0A73#0x0001` |
| `0x8A45000F` | `0x0A45` | `0x0010` | `0x00D9000C` | `(i64)` | `0x0A43#0x0018` |
| `0x8A8400F3` | `0x0A84` | `0x00F4` | `0x00D9000C` | `(i32)` | `CPVPVirtueSystem::UI_IncrementVirtue` |
| `0x8C0D0017` | `0x0C0D` | `0x0018` | `0x00D9000C` | `(ref, i32)` | `0x0C0C#0x0015`, `CRandomItemRewardSystem#0x0014` |
| `0x8C170001` | `0x0C17` | `0x0002` | `0x00D9000C` | `(ref, ref)` | `0x0C16#0x0001` |
| `0x8C200005` | `0x0C20` | `0x0006` | `0x00D9000C` | `(ref, i32, ref)` | `0x0C23#0x0002` |
| `0x8C41000D` | `0x0C41` | `0x000E` | `0x00D9000C` | `` | `CFellowshipMakerSystem#0x0003` |
| `0x8C41000F` | `0x0C41` | `0x0010` | `0x00D9000C` | `(i32)` | `CFellowshipMakerSystem#0x0004` |
| `0x8C410019` | `0x0C41` | `0x001A` | `0x00D9000C` | `(i32)` | `CFellowshipMakerSystem#0x000A` |
| `0x8C41004F` | `0x0C41` | `0x0052` | `0x00D9000C` | `` | `CFellowshipMakerSystem#0x001B` |
| `0x8C410056` | `0x0C41` | `0x0057` | `0x00D9000C` | `(i32, i32)` | `CFellowshipMakerSystem#0x001C` |
| `0x8C410055` | `0x0C41` | `0x0058` | `0x00D9000C` | `(ref, i32, i32)` | `CFellowshipMakerSystem#0x001D` |
| `0x8C4F0001` | `0x0C4F` | `0x0002` | `0x00D9000C` | `(i64, i32, i32)` | `0x0C4E#0x0001` |
| `0x8C4F0003` | `0x0C4F` | `0x0004` | `0x00D9000C` | `(ref, i32)` | `0x0C4E#0x0002` |
| `0x8CFE0001` | `0x0CFE` | `0x0002` | `0x00D9000C` | `(ref)` | `0x0D00#0x0009` |
| `0x8CFE0009` | `0x0CFE` | `0x000A` | `0x00D9000C` | `(i32, ref)` | `0x0D00#0x0003` |
| `0x8DA70007` | `0x0DA7` | `0x0008` | `0x00D9000C` | `(i32)` | `CMountedCombatSystem#0x0003` |
| `0x8DA70009` | `0x0DA7` | `0x000A` | `0x00D9000C` | `(i64)` | `CMountedCombatSystem#0x0015` |
| `0x8DA7000B` | `0x0DA7` | `0x000C` | `0x00D9000C` | `` | `CMountedCombatSystem#0x0014` |
| `0x8DA7000D` | `0x0DA7` | `0x000E` | `0x00D9000C` | `(ref)` | `CMountedCombatSystem#0x001B` |
| `0x8DA7000F` | `0x0DA7` | `0x0010` | `0x00D9000C` | `` | `CMountedCombatSystem#0x001F` |
| `0x8DA70013` | `0x0DA7` | `0x0014` | `0x00D9000C` | `(i32, f32, i64)` | `CMountedCombatSystem#0x002A` |
| `0x8DD1003B` | `0x0DD1` | `0x003F` | `0x00D9000C` | `(i32)` | `0x0DD0#0x0002` |
| `0x8DD1003E` | `0x0DD1` | `0x0040` | `0x00D9000C` | `` | `0x0DD0#0x0001` |
| `0x8DD1004B` | `0x0DD1` | `0x004C` | `0x00D9000C` | `(i32)` | `0x0DD0#0x0008` |
| `0x8DD1004F` | `0x0DD1` | `0x0050` | `0x00D9000C` | `(ref)` | `0x0DD0#0x000C` |
| `0x8E150001` | `0x0E15` | `0x0002` | `0x00D9000C` | `(i32, i32)` | `0x0E16#0x0001` |
| `0x8E150005` | `0x0E15` | `0x0006` | `0x00D9000C` | `(ref, i32, i32, i32)` | `0x0E16#0x0006` |
| `0x8E150009` | `0x0E15` | `0x000A` | `0x00D9000C` | `(i32)` | `0x0E16#0x0007` |
| `0x8E15000B` | `0x0E15` | `0x000C` | `0x00D9000C` | `(ref, i32, i32)` | `0x0E16#0x0008` |
| `0x8E15000F` | `0x0E15` | `0x0010` | `0x00D9000C` | `(ref, ref, i32, i32, i32)` | `0x0E16#0x000A` |
| `0x8E430001` | `0x0E43` | `0x0002` | `0x00D9000C` | `(i32)` | `0x0E41#0x0001` |
| `0x8E7C000F` | `0x0E7C` | `0x0010` | `0x00D9000C` | `(ref)` | `CRandomItemRewardSystem::UI_GetSecondsUntilNextRewardReset` |
| `0x8EB60008` | `0x0EB6` | `0x0009` | `0x00D9000C` | `` | `CBigBattleSystem#0x0001` |
| `0x8ED50014` | `0x0ED5` | `0x0018` | `0x00D9000C` | `(i64)` | `CMultiUsageSystem#0x0005` |
| `0x8ED50017` | `0x0ED5` | `0x0019` | `0x00D9000C` | `(i64)` | `CMultiUsageSystem#0x0006` |
| `0x8ED5001E` | `0x0ED5` | `0x001F` | `0x00D9000C` | `(i32, i64)` | `CMultiUsageSystem#0x0029` |
| `0x8ED5002A` | `0x0ED5` | `0x002B` | `0x00D9000C` | `(i32, i64)` | `CMultiUsageSystem#0x0028` |
| `0x8F5C0003` | `0x0F5C` | `0x0004` | `0x00D9000C` | `(i32, i32, i64)` | `0x0F60#0x0002` |
| `0x8F5C0007` | `0x0F5C` | `0x0008` | `0x00D9000C` | `(i64)` | `0x0F60#0x0006` |
| `0x8FBC000B` | `0x0FBC` | `0x000C` | `0x00D9000C` | `(ref, i32)` | `0x0F71#0x0017` |
| `0x8FF60001` | `0x0FF6` | `0x0002` | `0x00D9000D` | `(ref)` | `0x0FFA#0x0001` |
| `0x90120001` | `0x1012` | `0x0002` | `0x00D9000C` | `(i32)` | `0x1011#0x0004` |
| `0x9012000D` | `0x1012` | `0x000E` | `0x00D9000C` | `(i32, i32)` | `0x1011#0x0009` |
| `0x9012000F` | `0x1012` | `0x0010` | `0x00D9000C` | `(i32)` | `0x1011#0x000C` |
| `0x901B000C` | `0x101B` | `0x000E` | `0x00D9000C` | `(i32, i32, i32)` | `0x101C#0x0005` |
| `0x901B0006` | `0x101B` | `0x0010` | `0x00D9000C` | `(i32, i32, i32)` | `0x101C#0x000B` |
| `0x901B0007` | `0x101B` | `0x0014` | `0x00D9000C` | `(i32, i32, i32)` | `0x101C#0x0009` |
| `0x901B001B` | `0x101B` | `0x001C` | `0x00D9000C` | `(i32, i32, i32)` | `0x101C#0x0010` |
| `0x90550002` | `0x1055` | `0x0006` | `0x00D9000C` | `(i32, i32, i32, i64)` | `0x1054#0x001A` |
| `0x90550003` | `0x1055` | `0x0007` | `0x00D9000C` | `(i32, i64)` | `0x1054#0x0026` |
| `0x90550001` | `0x1055` | `0x0008` | `0x00D9000C` | `(i64, i64)` | `0x1054#0x001D` |
| `0x90550005` | `0x1055` | `0x0009` | `0x00D9000C` | `(i32, i32, i64)` | `0x1054#0x001B` |
| `0x90550004` | `0x1055` | `0x000A` | `0x00D9000C` | `(i64)` | `0x1054#0x0020` |
| `0x90550014` | `0x1055` | `0x0016` | `0x00D9000C` | `(i64)` | `0x1054#0x003A` |
| `0x9055001A` | `0x1055` | `0x001F` | `0x00D9000C` | `(ref, i64)` | `0x1054#0x004F` |
| `0x90860007` | `0x1086` | `0x0008` | `0x00D9000C` | `(i32, i32)` | `0x1087#0x0004` |
| `0x969F000C` | `0x169F` | `0x000D` | `0x00D9000D` | `(ref)` | `NPCSystem#0x0007` |
| `0x969F0018` | `0x169F` | `0x0019` | `0x00D9000D` | `(i64, i64)` | `NPCSystem::GiveNeighborGift` |
| `0x969F004C` | `0x169F` | `0x004D` | `0x00D9000D` | `(i32, i32)` | `NPCSystem#0x0049` |

---

## 3. P1 — Character Properties, Inventory, Containers & Equipment <a id="p1-inventory-character"></a>



### 3.1 Entity Containment Model and Slot Bitmask Encoding (`BIN`, `DAT`)

In the official protocol, every item instance in a character's Backpack, Equipped Paperdoll, or open World Container is represented as a full entity (`64-bit InstanceID` and `32-bit` template `DataID` in Property `0x00000001`) attached to a parent container entity via the entity containment component.

#### Containment Types (`u32`)
| Value (Hex) | Value (Dec) | Meaning | Provenance |
|---|---|---|---|
| `0x00000010` | `16` | Attached / wielded visual child entity | `BIN` |
| `0x00000100` | `256` | Equipped paperdoll item | `BIN` |
| `0x00001000` | `4096` | Contained inventory / bag item | `BIN` |

#### Container Slot Property (`PID 0x0000034E`, `Bitfield32`) (`BIN`, `DAT`)
Each contained item entity specifies its position within its parent container via Property `0x0000034E` (`846`, type `0x12` `Bitfield32`). The parent container observes Property `0x0000034E` on all contained child entities and automatically re-indexes a child entity's slot whenever Property `0x0000034E` changes.

| High Nibble (`bits 28..31`) | Low Bits (`bits 0..27`) | Meaning | Provenance |
|---|---|---|---|
| `0x40000000` (`bit 30`) | `slot_index` (`0 .. capacity - 1`) | **Backpack / Container Slot:** `0x40000000 \| slot_index` (with `containment_type = 0x1000`). | `BIN` |
| `0x20000000` (`bit 29`) | `1 << (equip_index - 1)` (`bits 0..21`) | **Equipped Paperdoll Slot:** `0x20000000 \| equip_bit` (with `containment_type = 0x0100`). | `BIN` |
| `0x80000000` (`bit 31`) | `index` | **Overflow / Pending Slot.** | `BIN` |
| `0x10000000` (`bit 28`) | `sub_slot` | **Auxiliary Container Slot.** | `BIN` |

#### Paperdoll Equipment Slot Bitflags (`bits 0..21` of `PID 0x0000034E`, `PID 0x10000D88`, and `PID 0x10001516`) (`BIN`, `DAT`)
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

### 3.2 Initial Container & Item Delivery in Entity Creation (`BIN`)

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

### 3.3 Runtime Containment & Placement Update (`NetBlob` Event `0x00CE`) (`BIN`)

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

### 3.4 Item & Container Property IDs (`BIN`, `DAT`)

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

### 3.5 Client <-> Server Inventory, Equipment, Loot & Storage WLib RPCs (`NetBlobEvent` `0x30` (`0x30` / `0x5F` / `0xC0`)) (`BIN`, `DAT`)

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
| `0x850A0042` | C -> S | `` | **ClaimAllPendingLoot:** Claims all items from Pending Loot into backpack. | `BIN`, `DAT` |
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

### 3.6 In-World Character Property & Level Updates (`CharacterEvent`) (`BIN`, `DAT`)

#### Live Property Update `NetBlob` Events (`BIN`)
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

#### Character Level Update (`PID 0x10001011` and WLib RPC `0x84DF0016`) (`BIN`, `DAT`)
- **Effective Level Property:** `PID 0x10001011` (`268439569`, `Advancement_Level`, `Int32`).
- **Unscaled Real Level Property:** `PID 0x1000493B` (`268454203`, `Advancement_RealLevel`, `Int32`; falls back to `0x10001011` when `0`).
- **Quiet Update (`CharacterEvent::LevelChanged { new_level, quiet: true }`):** A `0x0088` or `0x0052` property update changing `PID 0x10001011` without an accompanying `0x84DF0016` RPC updates the character's level silently.
- **Level-Up Presentation (`CharacterEvent::LevelChanged { new_level, quiet: false }`):** When a character gains a level in-world, the server updates `PID 0x10001011` and sends S->C WLib RPC **`0x84DF0016`** (0 arguments) on `NetBlobEvent` `0x30` (`0x30` / `0x5F` / `0xC0`), which reads `PID 0x10001011`, announces the new level in chat (`0x25008B2C`), and plays the level-up fanfare.

---

---

### 3.7 Native Package `0x0030` (`CInventoryPackage`) Method Table (`BIN`) <a id="p1-native-pkg-0030"></a>

| `fnum` | Function ID | Signature & Behavior |
| --- | --- | --- |
| `1` | `0x00300001` | `AddItemToContainer(container: Entity, item: Entity, containment_type: u32) -> bool` (calls `vslot +0xc0` ) |
| `2` | `0x00300002` | `AttachEntityWithSlot(parent: Entity, child: Entity, attach_id: u32, slot: u32) -> bool` (calls `vslot +0xe8` and sets `PID 0x00000642` = `attach_id`) |
| `3` | `0x00300003` | `TransferEntityContainment(entity_a: Entity, entity_b: Entity, flags: u32) -> bool` |
| `4` | `0x00300004` | `GetAllContainedItemIIDs(container: Entity) -> Array<u64>` (iterates `[container + 0x18]`) |
| `5` | `0x00300005` | `GetAllContainedItemIIDsRecursive(container: Entity) -> Array<u64>` |
| `6` | `0x00300006` | `GetEquippedItemIIDs(container: Entity) -> Array<u64>` (filters `[container + 0x18]` where `containment_type == 0x0100`) |
| `7` | `0x00300007` | `GetContainedEntity(container: Entity, item_iid: u64) -> Entity` (`vslot +0xb8` ) |
| `8` | `0x00300008` | `EquipBitmaskToSlotIndex(slot_mask: u32) -> u32` (maps `1 << 0..21` to `1..22`, else `0`) |
| `9` | `0x00300009` | `ContainsItem(container: Entity, item_iid: u64) -> bool` |
| `10` | `0x0030000A` | `GetItemIfInSameRootContainer(entity: Entity, item_iid: u64) -> Entity` |
| `11` | `0x0030000B` | `LookupEntityByIID(item_iid: u64) -> Entity` |
| `12` | `0x0030000C` | `GetDirectParentContainer(item: Entity) -> Entity` (returns `[containment + 0x78]`) |
| `13` | `0x0030000D` | `GetRootParentContainer(item: Entity) -> Entity` |
| `14` | `0x0030000E` | `HaveSameRootParentContainer(entity: Entity, item_iid: u64) -> bool` |
| `15` | `0x0030000F` | `HasFreeSlotForItem(container: Entity, item_iid: u64) -> bool` |
| `16` | `0x00300010` | `DemoteAllEquippedToInventory(container: Entity)` (`0x0100` -> `0x1000`) |
| `17` | `0x00300011` | `PromoteAllInventoryToEquipped(container: Entity)` (`0x1000` -> `0x0100`) |
| `18` | `0x00300012` | `SetContainmentType(container: Entity, item_iid: u64, containment_type: u32) -> bool` |
| `19` | `0x00300013` | `GetItemAtExactSlot(container: Entity, slot: u32) -> Entity` (looks up `slot` in `[container + 0x40]`) |
| `20` | `0x00300014` | `GetItemsMatchingSlotMask(container: Entity, slot_mask: u32, out_array: Array<u64>) -> bool` (`(item_slot & slot_mask) != 0`) |
| `21` | `0x00300015` | `GetItemsMatchingSlotMaskSorted(container: Entity, slot_mask: u32, out_array: Array<u64>) -> bool` |
| `22` | `0x00300016` | `IsSlotMaskEmpty(container: Entity, slot_mask: u32) -> bool` |
| `23` | `0x00300017` | `CountItemsMatchingSlotMask(container: Entity, slot_mask: u32) -> u32` |
| `24` | `0x00300018` | `FindFirstFreeSlotInRange(container: Entity, start_slot: u32, end_slot: u32) -> u32` |
| `25` | `0x00300019` | `FindNextFreeSlotWrapping(container: Entity, start_slot: u32, min_slot: u32, max_slot: u32) -> u32` |
| `26` | `0x0030001A` | `GetItemsInSlotRange(container: Entity, mask: u32, min_slot: u32, max_slot: u32, out_array: Array<u64>) -> bool` |
| `27` | `0x0030001B` | `GetRootContainerIID(item: Entity) -> u64` |
| `28` | `0x0030001C` | `GetAllItemsAtExactSlot(container: Entity, slot: u32, out_array: Array<u64>) -> bool` |
| `29` | `0x0030001D` | `GetSingleItemAtSlot(container: Entity, slot: u32) -> Entity` |
| `30` | `0x0030001E` | `GetItemsWithSlotMask(container: Entity, slot_mask: u32, out_array: Array<u64>) -> bool` |

---

---

### 3.8 Complete Inventory, Equipment, Legendary Item, Relic, Socket & Lockbox WLib RPC Breakdown (`BIN`, `DAT`) <a id="p1-extended-rpcs"></a>

#### A. `CInventorySystem` (Class `1290` / `0x050A`, Base Class `368` / `0x0170`, RPC Prefix `0x850AXXXX`)
Disassembled from `wlib.bin` (`class_1290.txt`, `class_368.txt`, `class_2804.txt`, `class_1201.txt`, `class_1202.txt`):

| Function ID | Dir | Handler Offset | `WLibArgFrame` Parameters (Push Order -> Wire Order) | Purpose & Client Behavior |
|---|---|---|---|---|
| `0x850A0001` | C->S | `368` (`0xca9c8`) | `(dest_slot: u32, dest_container_iid: u64, item_iid: u64)` | **Unequip Item:** Sent by `2804 fnum 12` (`0xcc644`) when unequipping an item to `dest_container_iid` (`avatar_iid` for backpack) at `dest_slot` (`0` = auto-find first free backpack slot, or `0x40000000 \| bag_slot`). |
| `0x850A000B` | C->S | `368` (`0xcaa7c`) | `(quantity: i32, dest_slot: u32, dest_container_iid: u64, item_iid: u64)` | **Move Item / Split Stack:** Sent by `2804 fnum 5` (`0xcc834`) / `2350 fnum 6` (`0xcd058`). `quantity == 0` (or full stack size) moves the entire stack; `0 < quantity < current_stack` splits `quantity` items into `dest_slot` (`0x40000000 \| bag_slot`) in `dest_container_iid`. |
| `0x850A000C` | C->S | `368` (`0xcaab4`) | `(item_iid: u64)` | **Drop Item to Ground / World:** Sent by `2804 fnum 2` (`0xcc5d8`). |
| `0x850A0016` | C->S | `368` (`0xca954`) | `(item_iid: u64)` | **Take / Pickup Item into Inventory:** Sent by `2804 fnum 1` (`0xcc59c`). |
| `0x850A0019` | C->S | `368` (`0xca918`) | `(quantity_or_flags: i32, item_iid: u64)` | **Destroy Item:** Sent by `2804 fnum 4` (`0xcc708`, called from C++ ). Verified not in `0x10001718` (`Usage_ItemsInUse`). |
| `0x850A001A` | C->S | `368` (`0xca8dc`) | `(target_iid: u64, item_iid: u64)` | **Give Item to Entity/NPC:** Sent by `2804 fnum 3` (`0xcc6ac`). |
| `0x850A001D` | C->S | `368` (`0xcaa04`) | `(target_slot: u32, flags: u32, item_iid: u64)` | **Equip Item (or Auto-Use/Equip from Inventory):** Sent by `2804 fnum 6` (`0xcc784`) / `2350 fnum 10` (`0xcd3a4`, called from C++ ). `target_slot` is `0x20000000 \| (1 << equip_bit)` for a specific paperdoll slot or `0` for default slot. |
| `0x850A001E` | C->S | `368` (`0xcaa40`) | `(flag: bool, item_iid: u64)` | **Toggle Item State / Lock:** Sent by `2804 fnum 15` (`0xcc7dc`). |
| `0x850A0021` | S->C | `1290` (`0xcbe8c`) | `(err_code: u32, item_iid: u64)` | **Inventory Error Notification:** Displays inventory error via `pkg 22 fn 3` and fires UI event `0x00020066` (`131174`). |
| `0x850A0024` | S->C | `1290` (`0xcbe20`) | `(item_iid: u64)` | **OnItemRemovedFromInventory:** Removes `item_iid` from `0x1000147B` (`Inv_NewItemsList`), clears cache `0x100006E8`, and fires UI event `0x00020066` (`131174`). |
| `0x850A002A` | S->C | `1290` (`0xcbdd4`) | `(rc: u32)` | **OnInventoryOperationResult:** Fires UI event `0x000200CA` (`131274`, `rc == 1` success). |
| `0x850A002B` | C->S | `368` (`0xca86c`) | `` | **Query Inventory / Refresh:** No-arg request. |
| `0x850A002C` | S->C | `1290` (`0xcbd50`) | `(item_did: u32, quantity: i32)` | **OnItemTransmuted / Converted Notification:** Displays chat message (`0x250091FF`). |
| `0x850A002F` | C->S | `368` (`0xca830`) | `(item_iid: u64, target_iid: u64)` | **Apply Item to Target Item** (e.g. dye/essence/scroll application). |
| `0x850A0033` | S->C | `1290` (`0xcba68`) | `(item_iid: u64)` | **OnNewItemAdded:** Appends `item_iid` to `0x1000147B` (`Inv_NewItemsList`), fires UI event `0x00020066` (`131174`), and notifies quest/tutorial systems. |
| `0x850A0034` | C->S | `368` (`0xca7f4`) | `(item_iid: u64)` | **ClearNewItemFlag:** Sent by `1290 fnum 53` (`0xcb9a4`) when player inspects/clears a new item (`0` = clear all). |
| `0x850A0037` | S->C | `1290` (`0xcb960`) | `(success: bool)` | **OnExpandCapacityComplete:** Fires UI event `0x00020168` (`131432`). |
| `0x850A0038` | C->S | `368` (`0xca7bc`) | `(bag_slot_map: HashTable)` | **SaveBagSlotLayout:** Persists `0x10002E4F` (`Inventory_BagSlotMap` bag-to-slot assignment). |
| `0x850A003D` | C->S | `368` (`0xca74c`) | `(item_iid: u64, arg2: u32)` | **Pending Loot / Item Action.** |
| `0x850A003E` | C->S | `368` (`0xca714`) | `(item_iid: u64)` | **ClaimPendingLootItem:** Sent by `1290 fnum 61` (`0xcb850`) to move one item from `0x10003684` (`Loot_PendingLootList`) into backpack. |
| `0x850A0042` | C->S | `368` (`0xca6a4`) | `` | **ClaimAllPendingLoot:** Sent by `1290 fnum 65` (`0xcb7b4`) to claim all items from `0x10003684`. |
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
| `0x85130015` | C->S | `372` (`0xb7088`) | `` | **Close / Query MEO Inventory.** |
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

---

### 3.9 Extended Item, Equipment & Container Master Property IDs (`0x34000000`) (`DAT`) <a id="p1-extended-pids"></a>

Verified from `wlib.bin` (`Class 1201`, `1208`, `1290`, `1291`, `1298`, `1299`, `2075`, `2350`, `2564`, `2804`, `4181`) and `lotroclient64.exe` :

| Property ID (Hex) | Property ID (Dec) | Property Type | Engine Name / Role | Description |
|---|---|---|---|---|
| `0x00000001` | `1` | `DataID` (`0x14`) | `Entity_DataID` | Item template `DID` (`0x70XXXXXX`) |
| `0x0000034E` | `846` | `Bitfield32` (`0x12`) | `Inventory_Slot` | Container slot bitmask (`0x40000000 \| bag_slot` for backpack, `0x20000000 \| (1 << equip_bit)` for paperdoll equipment). Observed by `[container + 0x48 + 0x08]` . |
| `0x00000642` | `1602` | `Int32` (`0x02`) | `Attachment_SlotID` | Visual attachment slot ID set by `pkg 48 fn 2` |
| `0x10000421` | `268436513` | `DataID` (`0x14`) | `Item_PaperdollVisual` | Visual appearance `DID` used to render equipped items on the 3D avatar |
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
| `0x10001516` | `268440854` | `Bitfield32` (`0x12`) | `Item_UnavailableEquipmentSlots` | Equipment slot bitmask blocked while this item is equipped (e.g. `0x00010000` off-hand blocked by a two-handed weapon; checked and `1202 vslot 36`) |
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

---

### 3.10 P1 Synthetic Test Vectors (`BIN`) <a id="p1-test-vectors"></a>

#### Test Vector 1: `NetBlob` Event `0x00CE` — Place Item `0x0300000000112233` into Avatar `0x0200000000000001` Backpack (`containment_type = 0x1000`)
- `event_id`: `CE 00` (`0x00CE`)
- `flags1`: `08` (`has_seq = 1`, no velocity or tail overrides)
- `entity_iid`: `33 22 11 00 00 00 00 03` (`0x0300000000112233` LE)
- `mode_flags`: `04` (`is_contained = 1`, `has_attachment = 0`)
- `sequence_num`: `01`
- `parent_container_iid`: `01 00 00 00 00 00 00 02` (`0x0200000000000001` LE)
- `containment_type`: `90 00` (`0x1000` = `4096` encoded as 2-byte 3-tier VLE: `0x80 | (0x1000 >> 8) = 0x90`, `0x00`)
- **Complete Hex Payload (22 bytes):**
 `CE 00 08 33 22 11 00 00 00 00 03 04 01 01 00 00 00 00 00 00 02 90 00`

#### Test Vector 2: C→S WLib RPC `0x850A000B` (`MoveItem`) on `NetBlobEvent` `0x30` — Move Full Stack of Item `0x0300000000112233` to Backpack Slot 5 (`0x40000005`) on Avatar `0x0200000000000001`
- `event_header` (`u8`): `30` (`NetBlobEvent` `0x30`)
- `wire_function_id` (`u32 LE`): `0B 00 0A 85` (`0x850A000B`)
- `arg_frame_byte_length` (`u32 LE`): `18 00 00 00` (`24` bytes)
- `arg[0] quantity` (`0x008E0002` `i32 LE`): `00 00 00 00` (`0` = move full stack)
- `arg[1] dest_slot` (`0x008E0002` `u32 LE`): `05 00 00 40` (`0x40000005`)
- `arg[2] dest_container_iid` (`0x008E0003` `u64 LE`): `01 00 00 00 00 00 00 02` (`0x0200000000000001`)
- `arg[3] item_iid` (`0x008E0003` `u64 LE`): `33 22 11 00 00 00 00 03` (`0x0300000000112233`)
- **Complete Event `0x30` Payload (33 bytes):**
 `30 0B 00 0A 85 18 00 00 00 00 00 00 00 05 00 00 40 01 00 00 00 00 00 00 02 33 22 11 00 00 00 00 03`

#### Test Vector 3: C→S WLib RPC `0x850A001D` (`EquipItem`) on `NetBlobEvent` `0x30` — Equip Item `0x0300000000112233` into Main-Hand Weapon Slot (`0x20008000`, Slot Index 16 / Bit 15)
- `event_header` (`u8`): `30` (`NetBlobEvent` `0x30`)
- `wire_function_id` (`u32 LE`): `1D 00 0A 85` (`0x850A001D`)
- `arg_frame_byte_length` (`u32 LE`): `10 00 00 00` (`16` bytes)
- `arg[0] target_slot` (`0x008E0002` `u32 LE`): `00 80 00 20` (`0x20008000`)
- `arg[1] flags` (`0x008E0002` `u32 LE`): `00 00 00 00` (`0`)
- `arg[2] item_iid` (`0x008E0003` `u64 LE`): `33 22 11 00 00 00 00 03` (`0x0300000000112233`)
- **Complete Event `0x30` Payload (25 bytes):**
 `30 1D 00 0A 85 10 00 00 00 00 80 00 20 00 00 00 00 33 22 11 00 00 00 00 03`

---

## 4. P2 — Targeting, Skills, Quickslots, Inductions, Cooldowns, Effects, Vitals & Combat Deltas <a id="p2-combat-skills"></a>

### 4.1 WLib RPC Envelope (`0x30`) and `WLibArgFrame` (`0x008E`) Serialization <a id="wlib-rpc-envelope"></a>

All P2 gameplay commands (`C→S`) and non-property events (`S→C`) travel inside game-action event `0x30` (`Weenie_WLibRpc`), which wraps a 32-bit wire RPC identifier and a length-prefixed `WLibArgFrame` (`0x008E`) payload buffer.

#### Table 4.1 — `Weenie_WLibRpc` (`0x30`) Envelope Layout

| Offset | Size | Type | Name | Meaning | Byte Order |
|---|---|---|---|---|---|
| `0x00` | 1 | `u8` (VLE) | `ev_id` | Always `0x30` for standard WLib RPC events. | — |
| `0x01` | 0 or 8 | `u64` | `entity_id` | Present **only** if base event header bit 5 (`0x20`) is set. Omitted (`0 bytes`) on standard player-scoped gameplay RPCs. | Little-endian |
| `+0x00` | 4 | `u32` | `rpc_id` | Wire RPC identifier (`(class_id << 16) \| 0x80000000 \| method_wire_index`). | Little-endian |
| `+0x04` | 4 | `u32` | `arg_frame_len` | Byte length of the following `WLibArgFrame` payload (`0` if the RPC takes no arguments). | Little-endian |
| `+0x08` | `arg_frame_len` | `u8[]` | `arg_frame` | Sequential `WLibArgFrame` encoded arguments. | Little-endian |

#### Table 4.2 — `WLibArgFrame` (`0x008E`) Primitive and Object Graph Encodings

| Primitive / Type | Size | Wire Format |
|---|---|---|
| `pack_32` / `unpack_32` (`u32`, `i32`, `u8` bool) | 4 bytes | `u32` / `i32` little-endian (`u8` booleans are carried in the low byte of a 4-byte `u32`). |
| `pack_64` / `unpack_64` (`u64`) | 8 bytes | `u64` little-endian (used for 64-bit entity `InstanceID`s and 64-bit cooldown keys). |
| `pack_f32` / `unpack_f32` (`f32`) | 4 bytes | IEEE-754 binary32 little-endian. |
| `pack_f64` / `unpack_f64` (`f64`) | 8 bytes | IEEE-754 binary64 little-endian (used for server timestamps in seconds). |
| `pack_ref` / `unpack_ref` (Single non-null object) | `10 + N` bytes | `u32 ref_count = 1` (`4B LE`), `u32 ref_id = 0` (`4B LE`), `u16 class_id` (`2B LE`), followed by `N` bytes of class-specific payload. (If null: `u32 ref_count = 0` or `ref_id = 0xFFFFFFFF`). |

---

### 4.2 Target Selection (`CombatCommand::SelectTarget`, `CombatEvent::TargetChanged`) <a id="p2-targeting"></a>

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

### 4.3 Skill Execution, Inductions/Channels, and Cooldowns <a id="p2-skills-inductions-cooldowns"></a>

#### Client-to-Server Skill Execution (`0x85280005`) and Class `0x0D85` (`SkillRequest`)
To execute a skill (`CombatCommand::UseSkill`), the client increments a 32-bit sequence token, stores it in property `0x10002B84` (`Skill_QueueToken`), and sends wire RPC **`0x85280005`** containing a single `pack_ref` of native class **`0x0D85`**.

##### Table 4.3 — Native Class `0x0D85` (`SkillRequest`) Payload Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x0D85`)

| Offset | Size | Type | Name | Presence Condition & Meaning | Byte Order |
|---|---|---|---|---|---|
| `+0x00` | 4 | `u32` | `flags` | Bitmask of optional fields:<br>• Bit 0 (`0x01`): `sub_skill_did` present<br>• Bit 1 (`0x02`): `target_sub_index` present<br>• Bit 2 (`0x04`): boolean skill modifier flag (inline in bit 2; 0 extra bytes)<br>• Bit 3 (`0x08`): `target_entity_id` present<br>• Bit 4 (`0x10`): `ground_target` position present | Little-endian |
| `+0x04` | 4 | `u32` | `skill_did` | Always present. Skill DataID (`0x7000xxxx`). | Little-endian |
| `+0x08` | 4 | `u32` | `seq_token` | Always present. Client skill sequence token (stored in `0x10002B84`). | Little-endian |
| opt | 0 or 4 | `u32` | `sub_skill_did` | Present iff `(flags & 0x01) != 0`. Secondary/sub-skill DataID. | Little-endian |
| opt | 0 or 4 | `i32` | `target_sub_index` | Present iff `(flags & 0x02) != 0` (`>= 0`). | Little-endian |
| opt | 0 or 8 | `u64` | `target_entity_id` | Present iff `(flags & 0x08) != 0`. Explicit target entity `InstanceID`. | Little-endian |
| opt | 0 or var | `Position` | `ground_target` | Present iff `(flags & 0x10) != 0`. Ground target `Position` (`0x00A0`). | Little-endian |

#### Table 4.4 — Skill Execution, Cancellation, and Result RPCs (`0x0528`)

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

### 4.4 Quickslots (`CombatCommand::SetQuickslot`, `CombatEvent::QuickslotChanged`) <a id="p2-quickslots"></a>

The client maintains `84` quickslots (`slot_index = 0..83`, grouped into `7` pages of `12` slots; active page stored in property `0x10004CAA` `Shortcut_Current_Page`). Quickslot contents are serialized using native class **`0x01C6` (`Shortcut`)**.

#### Table 4.5 — Native Class `0x01C6` (`Shortcut`) Payload Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x01C6`)

| `shortcut_type` (`u32 LE` at `+0x00`) | Category | Type-Specific Trailing Bytes (`+0x04`) |
|---|---|---|
| `0` | Empty / Cleared | `0 bytes` |
| `2` | Inventory Item | `u64 item_entity_id` (`8B LE`) + `u32 item_did` (`4B LE`) (`12 bytes` total) |
| `3`, `4` | Chat Alias / Command | `PString text` (`VLE char_count` + `u16[char_count]` UTF-16LE) |
| `5`, `6`, `9`, `12` | Skill (`6`) / Action DID | `u32 target_did` (`4B LE`) |
| `7` | Value / Preset | `u32 extra_val` (`4B LE`) |
| `1`, `8`, `10`, `11` | Reserved | `0 bytes` |

#### Table 4.6 — Quickslot RPCs (`0x051E` and `0x0958`)

| Wire RPC ID | Direction | Arguments (`WLibArgFrame` Order) | Meaning |
|---|---|---|---|
| `0x851E0020` | C→S | `ref<0x01C6> shortcut`, `u32 slot_index` | `CombatCommand::SetQuickslot` (`slot_index` in `0..83`; use `shortcut_type = 0` to clear). |
| `0x89580005` | S→C | `ref<0x01C6> shortcut`, `u32 slot_index`, `u32 page_index` | `CombatEvent::QuickslotChanged` for a single slot. |
| `0x89580001` | S→C | `ref<0x01C6> shortcut`, `u32 slot_index`, `u32 mode`, `u32 page_index` | Extended single-slot quickslot update. |
| `0x8958000B` | S→C | `u32 target_val` | Remove matching shortcuts from quickslot bars. |
| `0x8958000D` | S→C | `u32 skill_did`, `u32 flags` | Remove/update skill shortcuts (`shortcut_type == 6`) matching `skill_did`. |
| `0x851E002C` | S→C | `ref page_data`, `u32 page_index` | Bulk quickslot page synchronization. |

---

### 4.5 Vitals, Combat State, Effects, Combat Deltas, and Defeat/Revive <a id="p2-vitals-effects-defeat"></a>

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

##### Table 4.7 — Native Class `0x0198` (`Effect`) Payload Layout (after `ref_count=1`, `ref_id=0`, `class_id=0x0198`)

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

##### Table 4.8 — Effect System RPCs (`0x04F8`)

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

### 4.6 Synthetic Test Vectors <a id="p2-test-vectors"></a>

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

---

## 5. P3 — NPC Interaction, Dialogue, Quests, Deeds, Vendors, Barter, Buyback, Repair, Loot & Wallet <a id="p3-npc-quests-economy"></a>

### 5.0 P3 Subsystem to WLib Manager & RPC Class Mapping (`BIN`, `DAT`) <a id="p3-class-map"></a>

| **Entity Selection & Use** | `0x0535` (1333) | `0x8535xxxx` | 3 | 2 | `0x0127` (295 `Selection/Usage`), `0x01D6` (470) |
| **Targeted Action & Combo / Role Use** | `0x0528` (1320) | `0x8528xxxx` | 5 | 4 | `0x0127` (295), `0x017A` (378) |
| **Targeted Drama / Recall / Action Dialog** | `0x0524` (1316) | `0x8524xxxx` | 4 | 4 | `0x0628` (1576), `0x06DB` (1755), `0x0C47` (3143) |
| **Multi-Usage Activity** | `0x0ED5` (3797) | `0x8ED5xxxx` | 4 | 6 | `0x0EDD` (3805 `MultiUsageSystem`) |
| **NPC Action Picker & Session** | `0x0523` (1315) | `0x8523xxxx` | 8 | 6 | `0x0123` (291 `NPCActionPicker / Session`) |
| **NPC / Vendor / Buyback / Service** | `0x04FE` (1278) | `0x84FExxxx` | 20 | 39 | `0x0102` (258), `0x0123` (291), `0x019D` (413 `Buyback`), `0x0452` (1106) |
| **Quest Dispenser (NPC Quest Offer/Bestow/Turn-In)** | `0x0522` (1314) | `0x8522xxxx` | 2 | 2 | `0x011E` (286 `QuestDispenserSystem`) |
| **NPC Dialog Text / Billboard** | `0x0527` (1319) | `0x8527xxxx` | 0 | 1 | `0x037F` (895 `DialogText`) |
| **NPC Comment Drama** | `0x08CB` (2251) | `0x88CBxxxx` | 1 | 1 | `0x08CC` (2252 `NPCCommentSystem`) |
| **Drama Performance** | `0x04F6` (1270) | `0x84F6xxxx` | 1 | 0 | `0x06CE` (1742 `DramaSystem`) |
| **Quests (Journal, Progress, Tracker, Share)** | `0x0521` (1313) | `0x8521xxxx` | 14 | 36 | `0x011F` (287 `QuestSystem`), `0x06D9` (1753), `0x0C0E` (3086) |
| **Deeds / Accomplishments** | `0x04DA` (1242) | `0x84DAxxxx` | 1 | 3 | `0x00FC` (252 `AccomplishmentSystem`) |
| **Titles** | `0x052E` (1326) | `0x852Exxxx` | 8 | 3 | `0x017E` (382 `TitleSystem`) |
| **Vendor Buy / Sell / SellAll** | `0x0536` (1334) | `0x8536xxxx` | 6 | 1 | `0x01CE` (462 `VendorSystem`) |
| **Item Repair** | `0x050B` (1291) | `0x850Bxxxx` | 2 | 4 | `0x0171` (369 `RepairSystem`), `0x0651` (1617 `Discount`) |
| **Barter** | `0x0793` (1939) | `0x8793xxxx` | 1 | 1 | `0x0794` (1940), `0x0795` (1941 `BarterSystem`) |
| **Container / Corpse / Chest Open & Close** | `0x0513` (1299) | `0x8513xxxx` | 2 | 2 | `0x092E` (2350 `CMEOInventorySystem`) |
| **Loot Rules, Roll/Pass, Master & Pending Loot** | `0x0512` (1298) | `0x8512xxxx` | 8 | 7 | `0x0112` (274 `LootSystem`), `0x0751` (1873) |
| **Inventory & Container Item Operations** | `0x04EC` (1260) | `0x84ECxxxx` | 11 | 11 | `0x0102` (258 `InventorySystem`) |
| **Inventory Transaction Support** | `0x050A` (1290) | `0x850Axxxx` | 8 | 2 | `0x0170` (368 `InvSupportSystem`), `0x0AF4` (2804) |
| **Wallet (`PaperItem`)** | `0x0C0D` (3085) | `0x8C0Dxxxx` | 1 | 4 | `0x0C0C` (3084 `PaperItemSystem`), `0x0E7D` (3709) |

---

---

### 5.1 Overview and Wire Framing <a id="spec-overview"></a>

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

### 5.2 Entity Selection, Targeted Use, and NPC Session Initiation <a id="spec-npc-interaction"></a>

#### 5.2.1 Entity Selection and Use (`0x8535xxxx`, `0x8528xxxx`, `0x8524xxxx`) <a id="spec-entity-use"></a>

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

#### 5.2.2 NPC Action Picker and Service Session (`0x8523xxxx`) <a id="spec-npc-session"></a>

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

#### 5.2.3 NPC Dialogue, Quest Dispenser Dialog, and Comments (`0x8522xxxx`, `0x8527xxxx`, `0x88CBxxxx`, `0x84F6xxxx`) <a id="spec-npc-dialogue"></a>

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

### 5.3 Quests and Deeds (Accomplishments) <a id="spec-quests-deeds"></a>

#### 5.3.1 Quest Lifecycle State Machine <a id="spec-quest-state-machine"></a>

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

#### 5.3.2 Quest Wire Messages (`0x8521xxxx`) <a id="spec-quest-messages"></a>

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

#### 5.3.3 Deeds / Accomplishments (`0x84DAxxxx`) <a id="spec-deeds"></a>

Deeds are represented as Accomplishment records that share objective tracking with `0x8521xxxx` and use `0x84DAxxxx` for Accomplishment table synchronization (`BIN-WLIB`, `DAT-LOGIC`):

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x84DA0007` | C→S | `target_iid: InstanceID`, `params: WState` | Client accomplishment query or action for `target_iid`. |
| `0x84DA0004` | S→C | `active_deeds: WState`, `completed_deeds: WState` | Synchronizes active and completed deed tables. |
| `0x84DA0006` | S→C | *(empty)* | Signals completion of accomplishment directory initialization/reset. |
| `0x84DA000A` | S→C | `deed_did: u32` | Grants or updates accomplishment `deed_did`. |

---

### 5.4 Vendors, Buyback, Item Repair, and Barter <a id="spec-vendors-barter"></a>

#### 5.4.1 Vendor Buy, Sell, and Sell-All (`0x8536xxxx`, `0x84FE008E`) <a id="spec-vendors"></a>

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

#### 5.4.2 Item Repair (`0x850Bxxxx`) <a id="spec-repair"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x850B0002` | C→S | `item_iid: InstanceID`, `vendor_iid: InstanceID` | Requests repair of a single item `item_iid` at vendor `vendor_iid`. |
| `0x850B000B` | C→S | `vendor_iid: InstanceID`, `item_list: WState` | Requests batch/all repair of items in `item_list` at vendor `vendor_iid`. |
| `0x850B0004` | S→C | `result_code: u32`, `item_iid: InstanceID` | Reports repair result `result_code` for single item `item_iid`. |
| `0x850B0005` | S→C | *(empty)* | Confirms completion of a repair-all operation. |
| `0x850B0006` | S→C | *(empty)* | Triggers client repair UI refresh. |
| `0x850B000A` | S→C | `result_code: u32`, `vendor_iid: InstanceID` | Reports batch repair result `result_code` at `vendor_iid`. |

#### 5.4.3 Barter Exchange (`0x8793xxxx`) <a id="spec-barter"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x87930006` | C→S | `barter_req: WState`, `vendor_iid: InstanceID` | Executes a barter exchange described by `barter_req` (`Barter_Profile` `0x100017FA`, item selection, and `CounterBox_CurrentCount` `0x100002BD`) with `vendor_iid`. |
| `0x87930002` | S→C | `vendor_iid: InstanceID` | Server confirmation that the barter exchange with `vendor_iid` completed. |

---

### 5.5 Corpse/Chest Container Loot, Fellowship Roll/Master Loot, and Pending Loot <a id="spec-loot"></a>

#### 5.5.1 Corpse and Chest Viewing Lifecycle (`0x8513xxxx`) <a id="spec-container-loot"></a>

Provenance: `BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`.

| Opcode | Direction | Payload Fields (in wire order) | Meaning |
|---|---|---|---|
| `0x8513002D` | C→S | `container_iid: InstanceID` | Requests to open and view loot container/corpse `container_iid`. |
| `0x85130004` | S→C | `result_code: u32`, `flags: u32`, `container_iid: InstanceID` | Reports result of opening `container_iid`; on success (`result_code == 0`), the client opens the container loot window (honoring `Inventory_DisableCloseOnEmpty` `0x10001514`). |
| `0x85130016` | C→S | `container_iid: InstanceID` | Notifies server that the player closed container/corpse `container_iid`. |
| `0x85130014` | S→C | `result_code: u32`, `flags: u32`, `container_iid: InstanceID` | Confirms closure of `container_iid`. |

#### 5.5.2 Loot Acquisition, Roll/Pass, Master Loot, and Pending Loot (`0x8512xxxx`) <a id="spec-fellowship-loot"></a>

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

### 5.6 Wallet (`PaperItem`) and Coin Currency <a id="spec-wallet"></a>

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

### 5.7 Testable Invariants and Synthetic Test Vectors <a id="spec-test-vectors"></a>

#### 5.7.1 Invariants <a id="spec-invariants"></a>

1. **Opcode Prefix Invariant:** Every P3 gameplay RPC payload starts with a 4-byte little-endian `u32` whose high bit is `1` (`(opcode & 0x80000000) != 0`).
2. **Antispam Property Cooldowns:** Before transmitting C→S requests for Vendor (`0x8536xxxx`), Repair (`0x850Bxxxx`), or Barter (`0x87930006`), the client checks the corresponding timestamp property (`Antispam_VendorRequest` `0x10000CF5`, `Antispam_RepairItemRequest` `0x10000EED`, `Antispam_BarterRequest` `0x10006959`) (`BIN-WLIB`, `DAT-LOGIC`).
3. **Sell-Lock Protection:** `0x8536000A` (Sell All) must exclude any inventory item whose `Inventory_SellLockedByID` (`0x1000056D`) or `Item_ForceNotSellable` (`0x10003D3E`) property is set (`BIN-WLIB`, `DAT-LOGIC`).

#### 5.7.2 Synthetic Test Vectors <a id="spec-vectors"></a>

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

### 5.8 Inner `WState` Wire Layouts (`0x008E0001` / `0x008E0006`) <a id="spec-wstate-layouts"></a>

Provenance: `BIN-CLIENT` , `BIN-WLIB` (`wlib.bin`, `wlib.bin`, `wlib.bin`, `wlib.bin`, `wlib.bin`), `DAT-LOGIC`.

#### 5.8.1 Core `WState` Envelope and Container Primitives <a id="spec-wstate-envelope"></a>

Every `WState` argument serialized via `0x008E0001` or deserialized via `0x008E0006` begins with a 4-byte little-endian object reference/header word (`0xFFFFFFFF` indicates a null object reference; any other value is followed by a 2-byte little-endian `class_id: u16` at offset `+0x04`):

| `class_id` (`u16`) | Kind | Wire Body Layout Following `class_id: u16` |
| --- | --- | --- |
| `0x0011` (`WIntSet` / `BitSet`) | Native C++ | `count: u32` followed by `count × u32` little-endian integer elements (`DataID`s or IDs). |
| `0x0023` (`WHash`) | Native C++ | `count: u32` followed by `count` entries, each serialized as `key: u32`, `val_tag: u32`, `flag: u8`, and (when `val_tag != 0xFFFFFFFF`) a recursively serialized `WState` value. |
| `0x0027` / `0x00A6` (`PropertyCollection`) | Native C++ | Property set mapping 32-bit `PropertyID` (`0x1000xxxx`) keys to schema-typed values (`u32`, `u64`, `f32`, `f64`, `StringInfo`, or nested `WArray`). |
| `0x006F` / `0x00B6` (`WArray`) | Native C++ | `count: u32` followed by `count` typed elements (either primitive values or nested `WState` objects). |
| `0x00C7` (`StringInfo`) | Native C++ | Localized string token (`table_did: u32`, `token_id: u32`, and variable substitution map). |
| WLib Script Class (descriptor bit 28 set) | WLib Bytecode Class | Writes `slot_count × 8` bytes of raw 64-bit slot cells (`[value_u32, type_tag_u32]`), followed by recursive `WState` serialization of any slot whose type tag is an object reference . |

#### 5.8.2 `0x8521005C` Full Quest Journal Synchronization (6 `WState` Arguments) <a id="spec-wstate-quest-sync"></a>

When `0x8521005C` (`S→C`) is unpacked (`287::62` in `wlib.bin` → `1753::99` in `wlib.bin`), the 6 `WState` arguments populate `Class 0x06D9` (`1753`, `QuestManager` state) in the following exact wire order (`BIN-WLIB`):

| Arg # | Target Slot in `Class 0x06D9` (`1753`) | Container `class_id` | Key → Value / Element Layout |
|---|---|---|---|
| 1 | `this.slot[6]` (`active_quests`) | `WHash` (`0x0023`) | Maps active quest `DataID` (`u32`) → `WLib Class 0x057D` (`1405`, active quest state record containing objective progress and role states). |
| 2 | `this.slot[10]` (`active_accomplishments`) | `WHash` (`0x0023`) | Maps active accomplishment/deed `DataID` (`u32`) → `WLib Class 0x057D` (`1405`). |
| 3 | `arg -4` → `this.slot[10]` (`completed_quests`) | `WIntSet` (`0x0011`) | `count: u32` followed by `count × u32` completed quest `DataID`s (iterated via `0x0011000E` count and `0x00110004` element getter; each `DataID` is wrapped into a 2-slot `WLib Class 0x09D8` / `2520` completed entry). |
| 4 | `this.slot[16]` (`cooldown_table`) | `WHash` (`0x0023`) | Maps repeatable quest `DataID` (`u32`) → cooldown expiration timestamp / timer entry. |
| 5 | `this.slot[18]` (`category_limiter_table`) | `WHash` (`0x0023`) | Maps quest category ID (`u32`) → limiter counter and reset timestamp (`QuestCategoryLimiter_ResetTime` `0x10003B0B`). |
| 6 | `this.slot[2]` (`autobestow_or_history_table`) | `WState` container | Pending auto-bestow / quest history table stored into `this.slot[2]`. |

#### 5.8.3 `0x87930006` Barter Request (`Native Class 0x0796`, 49-Byte Fixed Body) <a id="spec-wstate-barter"></a>

In `0x87930006` (`C→S`, callers `1941::2` in `wlib.bin` and `1941::5` in `wlib.bin`), the `barter_req: WState` parameter is an instance of native C++ `WState` class `0x0796` (`BarterRequest`). Following the `WState` header and `class_id = 0x0796`, the serializer writes and reads a fixed 49-byte (`0x31`-byte) little-endian payload (`BIN-CLIENT`, `BIN-WLIB`):

| Byte Offset (after `0x0796` tag) | Size (Bytes) | Wire Type | Struct Offset | Native Getter / Setter (`0x0796xxxx`) | Field Name & Meaning |
|---|---|---|---|---|---|
| `+0x00` | 4 | `u32` (`DataID`) | `[rbx+0x08]` | `0x07960015` / `0x07960016` | `source_did`: Optional source item/context `DataID` (`0` for standard NPC/wallet barter). |
| `+0x04` | 4 | `u32` (`DataID`) | `[rbx+0x0C]` | `0x07960001` / `0x07960002` | `vendor_barter_did`: Vendor barter table `DataID` (validated in `1941::5` via `wlib.bin`). |
| `+0x08` | 4 | `u32` | `[rbx+0x10]` | `0x07960003` / `0x07960004` | `context_param`: Secondary barter context parameter (default `0`). |
| `+0x0C` | 1 | `u8` (`bool`) | `[rbx+0x14]` | `0x07960026` / `0x07960025` | `is_special_flag`: Boolean byte (`0` or `1`; deserializer rejects values `>= 2`). |
| `+0x0D` | 4 | `u32` (`DataID`) | `[rbx+0x18]` | `0x07960011` / `0x07960012` | `alt_profile_did`: Optional alternate profile `DataID` (default `0`). |
| `+0x11` | 4 | `u32` | `[rbx+0x1C]` | `0x07960013` / `0x07960014` | `alt_entry_index`: Optional alternate entry index (default `0`). |
| `+0x15` | 4 | `u32` (`DataID`) | `[rbx+0x20]` | `0x07960005` / `0x07960006` | `barter_item_did`: Selected barter item/offer `DataID`. |
| `+0x19` | 4 | `u32` (`DataID`) | `[rbx+0x24]` | `0x07960007` / `0x07960008` | `barter_profile_did`: Active `Barter_Profile` (`0x100017FA`) `DataID`. |
| `+0x1D` | 4 | `u32` (`DataID`) | `[rbx+0x28]` | `0x07960009` / `0x0796000A` | `barter_merchant_did`: Barter merchant/service `DataID`. |
| `+0x21` | 4 | `u32` | `[rbx+0x2C]` | `0x0796000B` / `0x0796000C` | `barter_entry_index`: Zero-based offer index within the barter profile. |
| `+0x25` | 4 | `u32` | `[rbx+0x30]` | `0x0796000D` / `0x0796000E` | `quantity`: Requested exchange count (`CounterBox_CurrentCount` `0x100002BD`). |
| `+0x29` | 4 | `u32` | `[rbx+0x34]` | `0x0796000F` / `0x07960010` | `reputation_or_tier`: Tier/requirement value (`BarterReputationRewardTier` `0x2300026F`). |
| `+0x2D` | 4 | `u32` | `[rbx+0x38]` | `0x07960021` / `0x07960022` | `extra_param`: Reserved/extra parameter (`0`). |

#### 5.8.4 `0x85220006` Quest Dispenser Selection `WState` (`PropertyCollection` `0x00A6` / `0x0027`) <a id="spec-wstate-quest-dispenser"></a>

When the player responds to an NPC or item Quest Dispenser dialog ( → `286::1` in `wlib.bin` → `0x85220006`), the 3 wire arguments are (`BIN-CLIENT`, `BIN-WLIB`, `DAT-LOGIC`):
1. `selection_data: WState` (`PropertyCollection` `0x00A6` / `0x0027`) populated with:
 - `0x10000BDC` (`QuestDispenser_Action`, `u32` enum from `QuestRoleAction` `0x2300001D`)
 - `0x10000208` (`QuestDispenser_RoleName`, string/token copied from the selected picker entry)
 - `0x10001270` (`QuestDispenser_RoleConstraint`, `u32` constraint copied from the selected picker entry)
 - `0x1000146E` (`QuestDispenser_Quest`, `u32` quest `DataID` copied from `0x10000F13` `UI_QuestPicker_DataID`)
 - `0x100000BC` (`QuestTreasure_SelectedItem`, `u32` item `DataID` of the player's chosen selectable reward when completing a quest via `CompleteNowButton` `0x10000562` / `FinishQuestButton` `0x10000CA9`)
2. `dispenser_did: u32`: The selected quest `DataID` (`0x10000F13` `UI_QuestPicker_DataID`).
3. `source_iid: InstanceID`: The 64-bit `InstanceID` of the dispenser NPC or item.

#### 5.8.5 `0x8512003D` Pending Loot Entry (`WLib Class 0x04CD` / `1229`) <a id="spec-wstate-pending-loot"></a>

`0x8512003D` (`S→C`, unpacked by `274::25` in `wlib.bin`) delivers a `WArray` (`0x00B6`) of `WLib Class 0x04CD` (`1229`) objects. Each `Class 0x04CD` instance has 19 slots (`152` bytes of slot cells plus nested `PropertyCollection` objects) mapped by `274::25` into the UI `PropertyCollection` (`0x10004E68` `UI_Inventory_PendingLoot_Array`) (`BIN-WLIB`, `DAT-LOGIC`):

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

#### 5.8.6 `0x8535001D` Entity Use Request (`WLib Class 0x063E` / `1598`) <a id="spec-wstate-entity-use"></a>

`0x8535001D` (`C→S`) serializes an instance of `WLib Class 0x063E` (`1598`), which inherits from `WLib Class 0x01E6` (`486`, base `PropertyCollection` wrapper) and adds 3 script slots (`BIN-WLIB`):
- `slot[0]`: `StringInfo` (`0x00C7`) interaction label/context token.
- `slot[1]`: `u32` interaction flags / action code.
- `slot[2]`: `PropertyCollection` (`0x00A6`) containing target entity `InstanceID` and usage parameters.

---

### 5.9 Enumeration Constants (`client_gamelogic.dat` `0x2300xxxx` & C++ UI Handlers) <a id="spec-enums"></a>

Provenance: `DAT-ENUM` (`0x23000017`, `0x23000018`, `0x2300001D`, `0x2300006B`, `0x230000D1`, `0x23000292`), `BIN-CLIENT` (–), `BIN-WLIB` (`wlib.bin`, `wlib.bin`).

#### 5.9.1 `QuestRoleAction` / `QuestDispenser_Action` (`EnumMapper 0x2300001D`, Property `0x10000BDC`) <a id="spec-enum-quest-role-action"></a>

Used in `0x85220002` (`S→C`), `0x85220006` (`C→S`), and switched (`BIN-CLIENT`, `DAT-ENUM`):

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

#### 5.9.2 `QuestUpdateType` (`EnumMapper 0x23000018`) <a id="spec-enum-quest-update-type"></a>

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

#### 5.9.3 `QuestStatus` (`EnumMapper 0x23000017`) and `FellowshipQuestResponseType` (`EnumMapper 0x23000292`) <a id="spec-enum-quest-status"></a>

- **`QuestStatus` (`0x23000017`):** Bitmask where `0xF0000000` is `RawStatusMask` and `0x0FFFFFFF` is `ObjectiveNumberMask`:
 - `0x00000000` = `Undef`
 - `0x10000000` = `Underway` (`0x10000001`..`0x10000023` = `Underway_Objective01`..`Underway_Objective35`)
 - `0x20000000` = `Failed`
 - `0x30000000` = `Completed`
 - `0x40000000` = `Abandoned`
 - `0x50000000` = `Tapped`
- **`FellowshipQuestResponseType` (`0x23000292`, used in `0x852100BD`):**
 - `0` = `Undef`, `1` = `ReadyToTurnIn`, `2` = `Underway`, `3` = `Completed`, `4` = `Eligible`, `5` = `NotEligible`.

#### 5.9.4 Fellowship Loot Vote Choice (`0x8512001B`) and `FellowshipLootType` (`EnumMapper 0x2300006B`) <a id="spec-enum-loot"></a>

1. **Fellowship Loot `vote_choice` (`u32` in `0x8512001B`, verified in C++ UI button handler , auto-pass handler , `274::35` in `wlib.bin`, and `UIElementID` `0x230000D1`):**

| `vote_choice` (`u32`) | Meaning | Triggering UI Button (`0x230000D1`) |
| --- | --- | --- |
| `1` | **Greed** | `ItemGreedButton` (`0x10000FF4`) |
| `2` | **Need** | `ItemNeedButton` (`0x10000FF3`) |
| `3` | **Pass** | `ItemNeedGreedPassButton` (`0x100012E7`), `ItemRollPassButton` (`0x100012E8`), or timer expiration / auto-pass |
| `4` | **Roll** | `ItemRollButton` (`0x10000105`) |
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


---

## 6. P4 — Fellowships, Raids, Secure Player Trade, Friends/Ignore, `/who`, Kinship Roster & Player Music <a id="p4-social-music"></a>

### 6.1 Social Panels Overview and WLib RPC Transport <a id="spec-social-overview"></a>

Social gameplay interactions on the World Server—Fellowships, Raids, Player-to-Player Trade, Friends & Ignore Lists, `/who` Directory Queries, Kinship (`Guild`) Management, and Player Music—are transported inside WLib entity RPC messages (`0x00000030`) over the `"ClientNet"` world connection (`BIN-WLIB-P4`, `OBS-DAT-P4`).

Each WLib RPC frame carries a 32-bit method identifier (`0x8PPPSSSS`, where `0xPPP` identifies the WLib package/class and `0xSSSS` identifies the static method index) followed by a sequential payload of little-endian primitive values (`u32`, `u64`, `f32`, `f64`) and typed object references (`ref(Obj)`). A `ref(Obj)` begins with a 32-bit `class_id` (`0xFFFFFFFF` for null, `0x00E1` for UTF-16LE `Utf16String` / `WString`, `0x00C7` for `StringInfo`, `0x0027` for `PropertyValue`, `0x00A6` for `PropertyCollection`, or a WState native/script class ID followed by its field stream).

---

### 6.2 Fellowships, Raids, Ready Checks, Votes, and Target Marking <a id="spec-fellowships-raids"></a>

#### 6.2.1 Fellowship Client-to-Server Commands (`0x04FE`)

| Method ID | Command / Action | Wire Payload (Sequential Little-Endian) | Preconditions & State Rules |
|---|---|---|---|
| `0x84FE0048` | `InviteToFellowship` (by IID) | `u64 target_iid` | Invites character `target_iid` by entity ID. |
| `0x84FE0057` | `InviteToFellowship` (by Name) | `ref(WString) target_name` | Invites character by display name (`/invite <name>`). |
| `0x84FE0058` | `InviteToFellowship` (Selected) | `u64 target_iid` | Invites currently selected target `target_iid` (`/fellowship recruit`). |
| `0x84FE0070` | `AnswerInvite` | `u32 response` (`1`=Accept, `0`=Decline), `u32 token`, `u64 recruiter_iid` | Responds to an invitation using `token` (`0x100020CF`) and `recruiter_iid` (`0x100020CA`), then clears `0x100020CC`. |
| `0x84FE0059` | `LeaveFellowship` | *(empty)* | Local player leaves the current fellowship. |
| `0x84FE0042` | `KickFromFellowship` | `u64 target_iid` | Leader dismisses member `target_iid` from the fellowship. |
| `0x84FE0041` | `SetFellowshipLeader` | `u64 target_iid` | Leader transfers fellowship leadership to member `target_iid`. |
| `0x84FE0056` | `DisbandFellowship` | *(empty)* | Leader disbands the fellowship. |
| `0x84FE0062` | `SetLootRule` | `u32 loot_rule` (`2..8`) | Leader sets loot rule (`2`=Round Robin, `3`=Free For All, `4`=Need/Greed, `5`=Master Looter, `7`=Roll/Pass). |
| `0x84FE0065` | `SetLootQuality` | `u32 loot_quality` (`2..5`) | Leader sets loot quality threshold (`2`=Uncommon, `3`=Rare, `4`=Incomparable, `5`=Epic). |
| `0x84FE007C` | `ConvertToRaid` | *(empty)* | Leader converts a standalone fellowship into a Raid. |
| `0x84FE00A7` | `StartReadyCheck` | *(empty)* | Leader initiates a fellowship ready check (throttled by `0x100018BF`). |
| `0x84FE00AA` | `AnswerReadyCheck` | `u32 ready_state` (`1`=Ready, `0`=Not Ready) | Member responds to an active ready check. |
| `0x84FE00EB` | `SetConjunctionSkill` | `u32 skill_did`, `u64 fellow_iid` | Sets desired Fellowship Manoeuvre skill (`0x10002BF2`). |
| `0x84FE00EC` | `ClearConjunctionSkill` | `u64 fellow_iid` | Clears desired Fellowship Manoeuvre skill (`0x10002BF2`). |
| `0x84FE0147` | `AddTargetAssist` | `u64 target_iid` | Leader adds `target_iid` to the Fellowship Target Assist list. |
| `0x84FE0149` | `RemoveTargetAssist` | `u64 target_iid` | Leader removes `target_iid` from the Fellowship Target Assist list. |
| `0x84FE0161` | `AddAutoFellow` | `u64 target_iid` | Adds `target_iid` to the open/auto-fellowship list. |
| `0x84FE0162` | `RemoveAutoFellow` | `u64 target_iid` | Removes `target_iid` from the open/auto-fellowship list. |

#### 6.2.2 Fellowship Server-to-Client Notifications (`0x04FE`)

| Method ID | Event / Notification | Wire Payload (Sequential Little-Endian) | Client State Mutation |
|---|---|---|---|
| `0x84FE004F` | `FellowshipInvite` | `u32 token`, `ref(WString) recruiter_name`, `u32 is_raid`, `u64 recruiter_iid` | Sets `0x100020CA = recruiter_iid`, `0x100020CB = recruiter_name`, `0x100020CC = 1`, `0x100020CD = is_raid`, `0x100020CF = token`, and displays invite prompt. |
| `0x84FE0052` | `FellowshipChanged` (Full Sync) | `ref(0x044F) fellowship` | Replaces the active `Fellowship` (`0x044F`) state and synchronizes target markers. |
| `0x84FE0044` | `FellowshipChanged` (Member Joined) | `ref(0x044D) fellow` | Inserts `Fellow` (`0x044D`) into `Fellowship.members`. |
| `0x84FE0043` | `FellowshipChanged` (Member Left/Kicked) | `u32 reason`, `u64 fellow_iid` | Removes `fellow_iid` from `Fellowship.members` (`1`=left, `2`=dismissed). |
| `0x84FE0053` | `FellowshipChanged` (Member Departed) | `u64 fellow_iid` | Removes `fellow_iid` from `Fellowship.members`. |
| `0x84FE0045` | `FellowshipChanged` (Leader Changed) | `u64 new_leader_iid` | Updates `Fellowship.leader_iid`. |
| `0x84FE0046` | `FellowshipChanged` (Disbanded) | *(empty)* | Clears active `Fellowship` state (`disbanded`). |
| `0x84FE0054` | `FellowshipChanged` (Self Left) | *(empty)* | Clears active `Fellowship` state (`left`). |
| `0x84FE0047` | `FellowVitalChanged` (Full Record) | `ref(0x044D) fellow` | Updates full `Fellow` (`0x044D`) record for `fellow.fellow_iid`. |
| `0x84FE008C` | `FellowVitalChanged` (Fast Vitals) | `u32 mask`, `u32 cur_morale`, `u32 max_morale`, `u32 cur_power`, `u32 max_power`, `u64 fellow_iid` | Updates morale and power fields on `fellow_iid` in place. |
| `0x84FE004E` | `FellowLinkDeadChanged` | `u32 is_linkdead`, `u64 fellow_iid` | Updates online/link-dead flag for `fellow_iid`. |
| `0x84FE004C` | `LootRuleChanged` | `u32 loot_rule`, `u64 actor_iid` | Updates `Fellowship.loot_rule`. |
| `0x84FE004D` | `LootQualityChanged` | `u32 loot_quality`, `u64 actor_iid` | Updates `Fellowship.loot_quality`. |
| `0x84FE0055` | `FellowPetAdded` | `u32 pet_type`, `u64 pet_iid`, `u64 owner_iid` | Attaches `FellowPet` (`0x044E`) to `owner_iid`. |
| `0x84FE005B` | `FellowPetRemoved` | `u64 pet_iid`, `u64 owner_iid` | Clears `FellowPet` on `owner_iid`. |
| `0x84FE006C` | `ReadyCheckStarted` | *(empty)* | Opens the ready check prompt for the local player. |
| `0x84FE006B` | `ReadyCheckResponse` | `u32 ready_state`, `u64 fellow_iid` | Updates ready state (`1`=Ready, `0`=Not Ready) for `fellow_iid`. |
| `0x84FE0089` | `TargetMarkSet` | `u32 marker_id`, `u64 target_iid` | Assigns marker `marker_id` (`0..9`) to `target_iid`. |
| `0x84FE0087` | `TargetMarkCleared` | `u32 marker_id`, `u64 target_iid` | Clears marker `marker_id` from `target_iid`. |
| `0x84FE00A1` | `TargetAssistAdded` | `u64 target_iid` | Appends `target_iid` to `Fellowship.target_assists`. |
| `0x84FE00A0` | `TargetAssistRemoved` | `u64 target_iid` | Removes `target_iid` from `Fellowship.target_assists`. |

#### 6.2.3 Raid Commands (`0x0523`), Votes (`0x0CFE`), Target Marking (`0x0739`), and LFF Maker (`0x0C41`)

- **Raid (`0x0523`):** Supports C→S `0x85230001` (`Disband`), `0x85230002` (`Leave`), `0x85230003` (`DismissByName`: `ref(WString)`), `0x85230004` (`DismissByIID`: `u64`), `0x85230005` (`TransferLeadership`: `u64`), `0x85230006` (`PromoteAssistant`: `u64`), `0x85230007` (`DemoteAssistant`: `u64`), `0x85230008` (`MoveMember`: `u32 group_idx, u64 target_iid`), `0x85230009` (`SwapMembers`: `u64 b_iid, u64 a_iid`), `0x8523000A` (`SetLootRule`: `u32`), `0x8523000B` (`SetLootQuality`: `u32`), `0x85230025` (`QueryRaidLocks`); and S→C `0x8523000C` (`FullSync`: `ref(0x0454)`), `0x8523000D`–`0x85230020`, `0x85230026`, `0x85230033` (`FastVitals`).
- **Target Marking (`0x0739`):** C→S `0x87390001` (`u32 marker_id, u64 target_iid`).
- **Fellowship Vote (`0x0CFE`):** C→S `0x8CFE0002` (`u32 response, u32 vote_id`); S→C `0x8CFE0003` (`ref(0x0D06) vote_data`), `0x8CFE0004` (`u32 reason, u32 vote_id`), `0x8CFE0005` (`u32 error, u32 vote_id`).
- **LFF Maker (`0x0C41`):** C→S `0x8C410002` (`Post`: `ref(0x0C3B)`), `0x8C410003` (`Remove`), `0x8C410004` (`Search`: `ref(0x0C36)`), `0x8C410009` (`RequestJoin`: `u64 leader_iid`); S→C `0x8C410005` (`SearchResults`), `0x8C410006` (`PostUpdated`), `0x8C410007` (`Status`), `0x8C41000A` (`JoinRequestReceived`).

---

### 6.3 Player-to-Player Trade (`0x0531`) <a id="spec-player-trade"></a>

#### 6.3.1 Trade State Machine

The client tracks player-to-player trade state in 5 states:
1. **`0` (Idle):** No active or pending trade. `OpenTrade` (`0x85310043`) transitions `0 → 1`.
2. **`1` (Pending):** Outgoing or incoming trade request waiting for session creation. `OnTradeOpened` (`0x8531000F`) transitions `1 → 2`; `DeclineTrade` (`0x85310034`) or `CloseTrade` (`0x8531005D`) transitions `1 → 0`.
3. **`2` (Open / Modifying Offer):** Both players can add items (`0x85310024`), remove items (`0x85310060`), or set offered coinage (`0x8531002E`). Any modification to items or coinage on either side automatically resets both players' ready/acceptance flags to `0`.
4. **`3` (Local Offer Locked / Accepted):** Entered when the local player sends `AcceptTrade` (`0x85310050`) with a full `0x05C3` (`TradeOfferData`) snapshot. If either player modifies the offer or sends `ResetTrade` (`0x85310042`), `OnTradeReset` (`0x85310046`) transitions `3 → 2`.
5. **`4` (Completed):** Entered on `OnTradeCompleted` (`0x8531005F`), followed immediately by `OnTradeFinalized` (`0x85310052`), which logs the exchange and resets to `0`.

#### 6.3.2 Trade Client-to-Server Commands (`0x0531`)

| Method ID | Command / Action | Wire Payload (Sequential Little-Endian) | Description |
|---|---|---|---|
| `0x85310043` | `OpenTrade` | `u64 target_iid` | Initiates a trade request with player `target_iid`. |
| `0x85310034` | `DeclineTrade` | *(empty)* | Declines an incoming trade request. |
| `0x8531005D` | `CloseTrade` | *(empty)* | Cancels/closes an active trade session. |
| `0x85310024` | `OfferTradeItem` | `u64 item_iid` | Adds inventory item `item_iid` to the local player's trade offer. |
| `0x85310060` | `RevokeTradeItem` | `u64 item_iid` | Removes `item_iid` from the local player's trade offer. |
| `0x8531002E` | `OfferTradeMoney` | `ref(0x0193) money` | Sets the local player's coin offer (`Money` map: `1`=copper, `2`=silver, `3`=gold, `4`=platinum). |
| `0x85310050` | `SetTradeReady` (`AcceptTrade`) | `ref(0x05C3) offer_snapshot` | Locks the local offer and submits the full `0x05C3` (`TradeOfferData`) snapshot for server verification. |
| `0x85310042` | `ResetTradeOffer` | *(empty)* | Unlocks and resets the trade offer. |

#### 6.3.3 Trade Server-to-Client Notifications (`0x0531`)

| Method ID | Event / Notification | Wire Payload (Sequential Little-Endian) | Description |
|---|---|---|---|
| `0x8531000F` | `TradeShown` (`OnTradeOpened`) | `ref(Obj) extra`, `u64 slave_iid`, `u64 master_iid` | Opens the trade window between initiator `master_iid` and recipient `slave_iid`. |
| `0x8531004E` | `TradeOffer` (`OnMyItemAdded`) | `u64 item_iid` | Confirms `item_iid` added to local offer and clears ready locks. |
| `0x8531004C` | `TradeOffer` (`OnPartnerItemAdded`) | `u64 item_iid` | Adds `item_iid` to partner offer and clears ready locks. |
| `0x85310068` | `TradeOffer` (`OnItemRevoked`) | `u64 item_iid` | Removes `item_iid` from whichever offer list contains it and clears ready locks. |
| `0x8531000B` | `TradeOffer` (`OnCopperChanged`) | `u64 copper_amount`, `u64 player_iid` | Updates scalar copper amount offered by `player_iid` and clears ready locks. |
| `0x8531000E` | `TradeOffer` (`OnMoneyChanged`) | `ref(0x0193) money`, `u64 player_iid` | Updates `Money` (`0x0193`) object offered by `player_iid` and clears ready locks. |
| `0x85310061` | `TradeReady` (`OnAcceptanceChanged`) | `u32 is_accepted`, `u64 player_iid` | Updates ready/lock state (`1`=Locked, `0`=Unlocked) for `player_iid`. |
| `0x85310046` | `OnTradeReset` | *(empty)* | Unlocks both offers and returns session to state `2`. |
| `0x8531005F` | `OnTradeCompleted` | *(empty)* | Marks trade session as completed (`state = 4`). |
| `0x85310052` | `OnTradeFinalized` | `u32 result_code` | Prints trade summary, closes trade window, and resets state to `0`. |
| `0x85310035` | `OnTradeCancelled` | `u32 reason_code`, `u64 actor_iid` | Closes trade window and displays cancellation reason. |
| `0x85310016` | `OnTradeError` | `u64 arg1`, `u64 target_iid` | Reports trade initiation failure. |

---

### 6.4 Friends & Ignore Lists (`0x04EC`), `/who` (`0x050C`), and Kinships (`0x0502`) <a id="spec-friends-who-kinships"></a>

#### 6.4.1 Friends & Ignore Commands and Notifications (`0x04EC`)

| Method ID | Dir | Command / Event | Wire Payload (Sequential Little-Endian) | Description |
|---|---|---|---|---|
| `0x84EC0003` | C→S | `AddFriend` | `ref(WString) name` | Adds character `name` to Friends list (`0x100018BB`). |
| `0x84EC0004` | C→S | `RemoveFriend` | `ref(WString) name` | Removes character `name` from Friends list (`0x100018BC`). |
| `0x84EC0007` | C→S | `SetFriendNote` | `ref(WString) note`, `ref(WString) name` | Sets `PID:Annotation_Text` (`0x1000305D`) on friend `name`. |
| `0x84EC0005` | C→S | `Ignore` | `u32 reason`, `ref(WString) name` | Adds character `name` to Ignore list with `reason` (`0x10003086`). |
| `0x84EC0006` | C→S | `Unignore` | `ref(WString) name` | Removes character `name` from Ignore list (`0x100018BE`). |
| `0x84EC002C` | C→S | `SetIgnoreNote` | `ref(WString) note`, `ref(WString) name` | Sets `PID:Annotation_Text` (`0x1000305D`) on ignored player `name`. |
| `0x84EC0008` | C→S | `SetAnonymous` | `u32 is_anonymous` (`1`=Anon, `0`=Public) | Toggles `/anon` (`PID:Communication_CharacterPresence` `0x10000DF0`). |
| `0x84EC0009` | C→S | `SetAFK` | `ref(WString) message`, `u32 is_afk` | Toggles `/afk` status and message. |
| `0x84EC000A` | S→C | `FriendList` (Full Sync) | `ref(HashTable<WString, ref(0x02B5)>) friends` | Synchronizes complete Friends list (`0x02B5` entries). |
| `0x84EC000C` | S→C | `FriendList` (Entry Updated) | `u32 is_login_transition`, `ref(0x02B5) entry` | Adds/updates a single friend entry (`is_login_transition != 0` prints login/logout notice). |
| `0x84EC000D` | S→C | `FriendList` (Entry Removed) | `ref(WString) name` | Removes `name` from Friends list. |
| `0x84EC000B` | S→C | `IgnoreList` (Full Sync) | `ref(HashTable<WString, ref(0x02B5)>) ignored` | Synchronizes complete Ignore list (`0x02B5` entries). |
| `0x84EC000F` | S→C | `IgnoreList` (Entry Updated) | `u32 is_update`, `ref(0x02B5) entry` | Adds (`0`) or updates (`1`) a single ignored entry. |
| `0x84EC0010` | S→C | `IgnoreList` (Entry Removed) | `ref(WString) name` | Removes `name` from Ignore list. |

#### 6.4.2 `/who` Directory Queries (`0x050C`) and Kinship Roster & Calendar (`0x0502`)

- **`/who` Queries (`0x050C`):** C→S `0x850C0001` (`ref(0x04BE) filter`) submits a directory search; C→S `0x850C0002` cancels it; S→C `0x850C0003` (`u32 total_count, ref(Array<ref(0x1030)>) results`) returns matching player records.
- **Kinships (`0x0502`):** C→S `0x85020003` (`Disband`), `0x85020004` (`RecruitByIID`), `0x85020005` (`RecruitByName`), `0x85020006` (`AnswerInvite`), `0x85020007` (`Quit`), `0x85020008` (`Expel`), `0x85020009` (`Promote`), `0x8502000A` (`Demote`), `0x8502000B` (`SetMOTD`), `0x8502000C` (`SetPlayerNote`), `0x8502000D` (`SetOfficerNote`), `0x8502000E` (`SetSuccessor`), `0x8502000F` (`Usurp`), `0x85020029` (`RequestRoster`), `0x8502002F` (`SetRankTitle`), `0x85020030` (`SetRankPermissions`), and `0x8502004E`–`0x85020053` (`Calendar` operations). S→C `0x85020010` (`FullGuildSync`: `ref(0x048C)`), `0x8502002A` (`FullRosterSync`: `HashTable<u64, ref(0x048F)>`), `0x85020013`–`0x8502001B` (member/rank/MOTD deltas), and `0x8502005B`–`0x8502005E` (`Calendar` sync).

---

### 6.5 Player Music (Solo & Synchronized) (`0x0519`) <a id="spec-player-music"></a>

#### 6.5.1 Player Music Commands and Notifications (`0x0519`)

| Method ID | Dir | Command / Event | Wire Payload (Sequential Little-Endian) | Description |
|---|---|---|---|---|
| `0x85190001` | C→S | `ToggleMusicMode` | `u32 enter_music_mode` (`1`=Enter, `0`=Exit) | Enters or exits `/music` stance (requires an instrument equipped in the ranged slot). |
| `0x85190002` | C→S | `PlayLiveNote` | `u32 velocity`, `u32 note_pitch` (`1..37`) | Plays an immediate interactive note in `/music` mode. |
| `0x85190003` | C→S | `PlayTimestampedNote` | `u32 velocity`, `f64 timestamp`, `u32 note_pitch` | Sends a single timestamped Note-On event. |
| `0x85190015` | C→S | `SendNoteBatch` | `ref(0x0804) note_batch` | Streams a batch of timestamped Note-On/Note-Off events (`MusicNoteBatch` `0x0804`). |
| `0x85190007` | C→S | `ReadyPerformance` | `u32 is_sync` (`1`=Sync, `0`=Solo), `ref(WString) song_title` | Readies an ABC song for solo (`0`) or fellowship-synchronized (`1`) playback. |
| `0x85190008` | C→S | `StartPerformance` | *(empty)* | Starts a synchronized performance (`/playstart`) for all readied fellowship members. |
| `0x85190009` | C→S | `StopPerformance` | *(empty)* | Stops current playback or withdraws from a readied synchronized performance. |
| `0x85190004` | S→C | `PerformanceStarted` | `u32 is_sync`, `u32 instrument_did`, `u64 performer_iid` | Performer entered music mode or started playing on `instrument_did`. |
| `0x85190005` | S→C | `OnLiveNote` | `u32 velocity`, `u32 instrument_did`, `u32 note_pitch`, `u64 performer_iid` | Immediate live note played by `performer_iid`. |
| `0x85190006` | S→C | `OnTimestampedNote` | `u32 flag_b`, `u32 flag_a`, `u32 velocity`, `u32 instrument_did`, `u64 performer_iid`, `f64 timestamp`, `u32 note_pitch` | Single timestamped Note-On event from `performer_iid`. |
| `0x8519001A` | S→C | `OnNoteBatch` | `ref(0x0804) note_batch` | Batched Note-On/Note-Off events (`MusicNoteBatch` `0x0804`). |
| `0x8519000A` | S→C | `PerformanceReady` | `u32 is_sync`, `ref(WString) song_title`, `u32 instrument_did`, `u64 performer_iid` | Performer `performer_iid` is ready to play `song_title` (`is_sync`: `1`=Sync, `0`=Solo). |
| `0x8519000B` | S→C | `PerformanceSyncStart` | `u64 initiator_iid` | Synchronized performance started by `initiator_iid`. |
| `0x8519000C` | S→C | `PerformanceStopped` / `PerformanceWithdrawn` | `u64 performer_iid` | Performer `performer_iid` stopped playing or withdrew from sync readiness. |

---

### 6.6 Synthetic Test Vectors and Invariants <a id="spec-test-vectors"></a>

#### Vector 1: Fellowship AnswerInvite (`0x84FE0070`, C→S)
- **Scenario:** Player accepts (`response = 1`) a fellowship invite with confirmation `token = 0x000004D2` (`1234`) from `recruiter_iid = 0x0200000000ABCDEF`.
- **Parameter Wire Bytes (16 bytes):**
 - `+0x00`: `01 00 00 00` (`u32 response = 1`)
 - `+0x04`: `D2 04 00 00` (`u32 token = 1234`)
 - `+0x08`: `EF CD AB 00 00 00 00 02` (`u64 recruiter_iid = 0x0200000000ABCDEF`)

#### Vector 2: Fellowship Fast Vitals Update (`0x84FE008C`, S→C)
- **Scenario:** Server sends a fast vitals delta (`mask = 0x0F`, `cur_morale = 1850`, `max_morale = 2000`, `cur_power = 900`, `max_power = 1000`) for `fellow_iid = 0x0200000000112233`.
- **Parameter Wire Bytes (28 bytes):**
 - `+0x00`: `0F 00 00 00` (`u32 mask = 15`)
 - `+0x04`: `3A 07 00 00` (`u32 cur_morale = 1850`)
 - `+0x08`: `D0 07 00 00` (`u32 max_morale = 2000`)
 - `+0x0C`: `84 03 00 00` (`u32 cur_power = 900`)
 - `+0x10`: `E8 03 00 00` (`u32 max_power = 1000`)
 - `+0x14`: `33 22 11 00 00 00 00 02` (`u64 fellow_iid = 0x0200000000112233`)

#### Vector 3: `MusicNoteBatch` (`0x0804`) Payload (C→S `0x85190015` / S→C `0x8519001A`)
- **Scenario:** A batch of `note_count = 1` Note-On event (`is_note_on = 1`, `timestamp = 1.5` s = `0x3FF8000000000000`, `note_pitch = 13`, `velocity = 100`) played by `performer_iid = 0x0200000000112233` with `flag_b = 1`, `flag_a = 0`, `instrument_did = 0x70001234`.
- **Body Wire Bytes (38 bytes):**
 - `+0x00`: `01 00 00 00` (`u32 note_count = 1`)
 - `+0x04`: `01 00 00 00` (`u32 events[0].is_note_on = 1`)
 - `+0x08`: `00 00 00 00 00 00 F8 3F` (`f64 events[0].timestamp = 1.5`)
 - `+0x10`: `0D 00 00 00` (`u32 events[0].note_pitch = 13`)
 - `+0x14`: `64 00 00 00` (`u32 events[0].velocity = 100`)
 - `+0x18`: `33 22 11 00 00 00 00 02` (`u64 performer_iid = 0x0200000000112233`)
 - `+0x20`: `01` (`u8 flag_b = 1`)
 - `+0x21`: `00` (`u8 flag_a = 0`)
 - `+0x22`: `34 12 00 70` (`u32 instrument_did = 0x70001234`)

---


---

### 6.7 Detailed `WState` Layouts for Fellowship (`0x044D`, `0x044E`, `0x044F`) & Raid (`0x0454`) (`BIN`, `DAT`) <a id="p4-wstate-fellowship-raid"></a>

All WState classes in Domain 1 serialize their fields in `vslot 2` (`Serialize(WState*)`) in the exact order below (`BIN-WLIB-P4`):

#### 1.5.1 `Fellow` (`0x044D`, decimal `1101`, `vslot 2 = 0x844D0007`)

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | *(base)* | `u32` | `schema_version` | Class version header (`4`). |
| 2 | `+0x18` | `u64` | `fellow_iid` | Character `InstanceID` (`EntityID`). |
| 3 | `+0x20` | `ref(WString)` | `name` | Character display name (`0x00000010`). |
| 4 | `+0x28` | `u32` | `level` | Character level. |
| 5 | `+0x2C` | `u32` | `class_id` | Character class ID (`Agent_Class`). |
| 6 | `+0x30` | `u32` | `max_morale` | Maximum morale (`Health`). |
| 7 | `+0x34` | `u32` | `cur_morale` | Current morale. |
| 8 | `+0x38` | `u32` | `max_power` | Maximum power. |
| 9 | `+0x3C` | `u32` | `cur_power` | Current power. |
| 10 | `+0x58` | `u64` | `voice_id` | Voice chat room/member flags. |
| 11 | `+0x60` | `bool` (`u8`) | `is_linkdead` | `1` if disconnected/link-dead, `0` if online. |
| 12 | `+0x40` | `ref(Obj)` | `pet` | Optional `0x044E` (`FellowPet`) object reference (`0` if no pet). |
| 13 | `+0x64` | `u32` | `conjunction_did` | Desired conjunction skill DataID (`0` if none). |
| 14 | `+0x68` | `u32` | `region_or_zone_did` | Current region/area DataID. |
| 15 | `+0x70` | `u32` | `instance_status` | Private encounter / instance eligibility state. |
| 16 | `+0x74` | `u32` | `race_or_sex_id` | Character race/species identifier. |
| 17 | `+0x78` | `bool` (`u8`) | `is_mentor_or_guest` | Mentoring / scaling flag. |

#### 1.5.2 `FellowPet` (`0x044E`, decimal `1102`, `vslot 2 = 0x844E0002`)

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | *(base)* | `u32` | `schema_version` | Class version header (`1`). |
| 2 | `+0x10` | `u64` | `pet_iid` | Pet entity `InstanceID`. |
| 3 | `+0x18` | `u32` | `pet_type` | Pet classification / summon type. |

#### 1.5.3 `Fellowship` (`0x044F`, decimal `1103`, `vslot 2 = 0x844F0005`)

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | *(base)* | `u32` | `schema_version` | Class version header (`5`). |
| 2 | `+0x20` | `HashTable<u64, ref(0x044D)>` | `members` | Map of `fellow_iid` → `Fellow` (`0x044D`) objects (up to 6 members). |
| 3 | `+0x28` | `u64` | `leader_iid` | Fellowship leader `InstanceID`. |
| 4 | `+0x30` | `u32` | `loot_rule` | Active loot distribution rule (`2..8`). |
| 5 | `+0x34` | `u32` | `loot_quality` | Active loot quality threshold (`2..5`). |
| 6 | `+0x38` | `u64` | `chat_room_id` | Fellowship chat/voice room identifier. |
| 7 | `+0x40` | `Array<u64>` | `target_marked_iids` | 10-slot array (`marker_id 0..9`) of marked target `InstanceID`s. |
| 8 | `+0x48` | `bool` (`u8`) | `is_raid_subgroup` | `1` if this fellowship is a subgroup within a Raid, `0` if standalone. |
| 9 | `+0x50` | `List<u64>` | `target_assists` | Ordered list of Fellowship Target Assist `InstanceID`s. |
| 10 | `+0x58` | `ref(Obj)` | `instance_info` | Optional active instance/encounter descriptor (`0` if none). |

#### 1.5.4 `Raid` (`0x0454`, decimal `1108`, `vslot 2 = 0x84540002`)

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | *(base)* | `u32` | `schema_version` | Class version header (`2`). |
| 2 | `+0x18` | `Array<ref(0x044F)>` | `subgroups` | Array of 4 `Fellowship` (`0x044F`) subgroup objects (up to 24 total members). |
| 3 | `+0x20` | `HashTable<u64, u32>` | `member_to_group` | Map of `fellow_iid` → `subgroup_index` (`0..3`). |
| 4 | `+0x28` | `Set<u64>` | `assistants` | Set of Raid Assistant `InstanceID`s. |
| 5 | `+0x30` | `u64` | `raid_leader_iid` | Overall Raid Leader `InstanceID`. |
| 6 | `+0x38` | `u32` | `loot_rule` | Raid loot distribution rule. |
| 7 | `+0x3C` | `u32` | `loot_quality` | Raid loot quality threshold. |
| 8 | `+0x40` | `u64` | `raid_chat_room_id` | Raid chat/voice room identifier. |
| 9 | `+0x48` | `Array<u64>` | `target_marked_iids` | 10-slot array of Raid target markers (`0..9`). |

---

---

### 6.8 Detailed `WState` Layouts for `TradeOfferData` (`0x05C3`) & `Money` (`0x0193`) (`BIN`, `DAT`) <a id="p4-wstate-trade"></a>

#### 2.4.1 `TradeOfferData` (`0x05C3`, decimal `1475`, `vslot 2 = 0x85C30021`)

Sent C→S in `0x85310050` (`AcceptTrade`) so the server can verify that the client's view of both offers matches the server's authoritative state before executing the atomic exchange (`BIN-WLIB-P4`):

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | `+0x10` | `u64` | `master_iid` | Trade initiator `InstanceID`. |
| 2 | `+0x18` | `u64` | `slave_iid` | Trade recipient `InstanceID`. |
| 3 | `+0x20` | `List<u64>` | `master_items` | Ordered list of item `InstanceID`s offered by `master_iid`. |
| 4 | `+0x28` | `List<u64>` | `slave_items` | Ordered list of item `InstanceID`s offered by `slave_iid`. |
| 5 | `+0x30` | `u64` | `master_copper` | Legacy scalar copper amount offered by `master_iid`. |
| 6 | `+0x38` | `u64` | `slave_copper` | Legacy scalar copper amount offered by `slave_iid`. |
| 7 | `+0x40` | `ref(0x0193)` | `master_money` | `Money` (`0x0193`) denomination map offered by `master_iid`. |
| 8 | `+0x48` | `ref(0x0193)` | `slave_money` | `Money` (`0x0193`) denomination map offered by `slave_iid`. |

*(Note: `+0x50` `master_accepted` and `+0x54` `slave_accepted` are runtime-only booleans on `0x05C3` and are not written by `0x85C30021`.)*

#### 2.4.2 `Money` (`0x0193`, decimal `403`, Native Serializer )

When `Pack_ObjRef` serializes a `0x0193` (`Money`) object, it writes `u32 class_id = 0x00000193` followed by the native `IntrusiveHashTable<u32, u32>` payload (`BIN-TRADE-NAT`):

| Offset | Size | Type | Field Name | Meaning |
|---|---|---|---|---|
| `+0x00` | `1` | `u8` | `bucket_index` | Index into the prime bucket-count table (`0` for small maps). |
| `+0x01` | `1..4` | `PackedCount` | `entry_count` | Number of non-zero currency denomination entries (`0..4`), encoded via `PackCompactCount` (: `1` byte if `< 0x80`). |
| `+0x02` | `8 * N` | `(u32, u32)[N]` | `entries` | `entry_count` pairs of `(u32 denomination_id, u32 count)` where `denomination_id`: `1` = Copper (`0..99`), `2` = Silver (`0..999`), `3` = Gold (`0..999`), `4` = Platinum. |

---

---

### 6.9 Complete Kinship (`0x0502`), Friend/Ignore (`0x02B5`), `/who` (`0x04BE`/`0x1030`) & `MusicNoteBatch` (`0x0804`) Reference Tables <a id="p4-detailed-wstate-and-kinship"></a>

#### 6.9.1 `FriendIgnoreEntry` (`0x02B5`, decimal `693`, `vslot 2 = 0x82B50002`) WState Layout

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | *(base)* | `u32` | `schema_version` | Class version header (`4`). |
| 2 | `+0x10` | `ref(WString)` | `name` | Character name (`0x00E1` `Utf16String`). |
| 3 | `+0x18` | `u32` | `level` | Character level (`0` if anonymous or offline). |
| 4 | `+0x1C` | `u32` | `class_id` | Character class ID (`0` if anonymous or offline). |
| 5 | `+0x20` | `u32` | `area_did` | Current region/area DataID (`0` if anonymous or offline). |
| 6 | `+0x24` | `bool` (`u8`) | `is_online` | `1` if currently online, `0` if offline. |
| 7 | `+0x28` | `f64` | `last_online_timestamp` | Server timestamp of last logout. |
| 8 | `+0x30` | `u32` | `vocation_id` | Crafting vocation ID. |
| 9 | `+0x38` | `ref(0x00A6)` | `properties` | `PropertyCollection` containing `PID:Annotation_Text` (`0x1000305D` note string), `PID:Annotation_IgnoreReason` (`0x10003086` u32 reason), and `PID:Communication_CharacterPresence` (`0x10000DF0`). |
| 10 | `+0x40` | `bool` (`u8`) | `is_anonymous` | `1` if the friend has `/anon` enabled. |


#### 6.9.2 `WhoQueryFilter` (`0x04BE` / `0x01CF`, `vslot 2 = 0x84BE0004`) & `WhoResultPlayer` (`0x1030` / `0x102F`, `vslot 2 = 0x90300004`)

- **`WhoQueryFilter` (`0x04BE`) WState fields:** `u32 schema_version (1)`, `ref(WString) name_substring`, `u32 class_mask`, `u32 race_mask`, `u32 min_level`, `u32 max_level`, `u32 area_did`, `bool lfg_only`, `bool fellow_only`, `u32 max_results`.
- **`WhoResultPlayer` (`0x1030`) WState fields:** `u32 schema_version (2)`, `u64 player_iid`, `ref(WString) name`, `u32 level`, `u32 class_id`, `u32 race_id`, `u32 area_did`, `ref(WString) guild_name`, `bool is_lfg`, `bool is_in_fellowship`, `u32 fellow_count`, `ref(WString) lfg_note`.

---

### 3.3 Kinship (`Guild`) Roster, Ranks, Notes, MOTD, and Calendar (`0x0502` ↔ `0x016C`)

Kinships are managed by `ServerGuild` (`0x0502`, decimal `1282`) and `ClientGuild` (`0x016C`, decimal `364`), backed by WState classes `Guild` (`0x048C` / `0x0169`), `GuildMember` (`0x048F` / `0x016A`), `GuildRank` (`0x0490` / `0x016B`), and `GuildCalendar` (`0x1681` / `0x1682` / `0x1685`) (`BIN-WLIB-P4`).

#### 6.9.3 `ServerGuild` (`0x0502`) Client-to-Server RPCs (23 Methods)

| RPC Method ID | Wrapper Method | Client Caller (`0x016C`) | Wire Parameters (`send_msg` order) | Kinship Action Semantics |
|---|---|---|---|---|
| `0x85020003` | `0x85020031` | `0x816C0004` (`/kinship disband`) | *(void)* | Disband kinship (leader only). |
| `0x85020004` | `0x85020032` | `0x816C0005` (`/kinship recruit`) | `u64 target_iid` | Invite player `target_iid` to kinship. |
| `0x85020005` | `0x85020033` | `0x816C0006` (`/kinship recruit <name>`) | `ref(Obj) name` (`WString`) | Invite player `name` to kinship by character name. |
| `0x85020006` | `0x85020034` | `0x816C0007` (Accept) / `0x816C0008` (Decline) | `u32 response`, `u32 token`, `u64 recruiter_iid` | Respond to kinship recruitment invite (`1` = Accept, `0` = Decline). |
| `0x85020007` | `0x85020035` | `0x816C0009` (`/kinship quit`) | *(void)* | Leave kinship voluntarily. |
| `0x85020008` | `0x85020036` | `0x816C000A` (`/kinship expel`) | `u64 member_iid` | Expel/kick `member_iid` from kinship. |
| `0x85020009` | `0x85020037` | `0x816C000B` (`/kinship promote`) | `u64 member_iid` | Promote `member_iid` one kinship rank. |
| `0x8502000A` | `0x85020038` | `0x816C000C` (`/kinship demote`) | `u64 member_iid` | Demote `member_iid` one kinship rank. |
| `0x8502000B` | `0x85020039` | `0x816C000D` (`/kinship motd`) | `ref(Obj) motd` (`WString`) | Set Kinship Message of the Day (`MOTD`). |
| `0x8502000C` | `0x8502003A` | `0x816C000E` (`/kinship playercomment`) | `ref(Obj) note` (`WString`) | Set local member's public kinship note (`player_note`). |
| `0x8502000D` | `0x8502003B` | `0x816C000F` (`/kinship officercomment`) | `ref(Obj) note` (`WString`), `u64 member_iid` | Set officer note on `member_iid`. |
| `0x8502000E` | `0x8502003C` | `0x816C0010` (`/kinship leader`) | `u64 successor_iid` | Designate successor / transfer kinship leadership. |
| `0x8502000F` | `0x8502003D` | `0x816C0011` (`/kinship usurp`) | *(void)* | Usurp leadership from inactive kinship leader. |
| `0x85020029` | `0x8502003E` | `0x816C0012` | *(void)* | Request full kinship roster sync. |
| `0x8502002F` | `0x8502003F` | `0x816C0013` | `u32 rank_id`, `ref(Obj) rank_title` (`WString`) | Rename kinship rank title. |
| `0x85020030` | `0x85020040` | `0x816C0014` | `u32 permissions_mask`, `u32 rank_id` | Update kinship rank permissions bitmask. |
| `0x85020047` | `0x85020048` | `0x816C002E` | `u64 house_iid` | Visit/travel to kinship house. |
| `0x8502004E` | `0x85020055` | `0x816C0031` | *(void)* | Request Kinship Calendar events (`0x1681`). |
| `0x8502004F` | `0x85020056` | `0x816C0032` | `ref(Obj) event` (`0x1682` `GuildCalendarEvent`) | Create Kinship Calendar event. |
| `0x85020050` | `0x85020057` | `0x816C0033` | `ref(Obj) event` (`0x1682` `GuildCalendarEvent`) | Edit Kinship Calendar event. |
| `0x85020051` | `0x85020058` | `0x816C0034` | `u64 event_id` | Delete Kinship Calendar event. |
| `0x85020052` | `0x85020059` | `0x816C0035` | `u64 event_id` | Request Kinship Calendar event signups (`0x1685`). |
| `0x85020053` | `0x8502005A` | `0x816C0036` | `ref(Obj) signup` (`0x1685`), `u64 event_id` | Sign up / update attendance for Kinship Calendar event `event_id`. |

#### 6.9.4 `ServerGuild` (`0x0502` → `0x016C`) Server-to-Client RPCs (28 Methods)

| RPC Method ID | Client Handler (`0x016C`) | Wire Parameters (`send_msg` order) | Notification Semantics |
|---|---|---|---|
| `0x85020010` | `0x816C0015` | `ref(Obj) guild` (`0x048C` `Guild`) | Full Kinship header & rank table synchronized (`0x01690001`). |
| `0x85020011` | `0x816C0016` | *(void)* | Kinship disbanded (`0x01690002`). |
| `0x85020012` | `0x816C0017` | `u32 token`, `ref(Obj) guild_name`, `ref(Obj) recruiter_name`, `u64 recruiter_iid` | Incoming Kinship recruitment invitation (`0x01690003`). |
| `0x85020013` | `0x816C0018` | `ref(Obj) member` (`0x048F` `GuildMember`) | Member joined kinship (`0x01690004`). |
| `0x85020014` | `0x816C0019` | `u32 reason`, `u64 actor_iid`, `u64 member_iid` | Member left or was expelled from kinship (`0x01690005`). |
| `0x85020015` | `0x816C001A` | `ref(Obj) member` (`0x048F` `GuildMember`) | Member record updated (rank, level, area, note, online state) (`0x01690006`). |
| `0x85020016` | `0x816C001B` | `u32 new_rank`, `u64 actor_iid`, `u64 member_iid` | Member promoted/demoted to `new_rank` (`0x01690007`). |
| `0x85020017` | `0x816C001C` | `ref(Obj) motd` (`WString`) | Kinship MOTD updated (`0x01690008`). |
| `0x85020018` | `0x816C001D` | `u64 new_leader_iid` | Kinship leader changed (`0x01690009`). |
| `0x85020019` | `0x816C001E` | `u64 successor_iid` | Kinship successor changed (`0x0169000A`). |
| `0x8502001A` | `0x816C001F` | `u32 error_code` | Kinship error message (`0x0169000B`). |
| `0x8502001B` | `0x816C0020` | `u32 is_online`, `u64 member_iid` | Member logged in (`1`) or out (`0`) (`0x0169000C`). |
| `0x8502002A` | `0x816C0025` | `ref(Obj) members_map` (`HashTable<u64, ref(0x048F)>`) | Full Kinship roster batch synchronized (`0x01690010`). |
| `0x8502002C` | `0x816C0027` | `ref(Obj) rank` (`0x0490` `GuildRank`) | Kinship rank title/permissions updated (`0x01690012`). |
| `0x8502005B` | `0x816C0037` | `ref(Obj) calendar` (`0x1681` `GuildCalendar`) | Full Kinship Calendar synchronized (`0x16830001`). |
| `0x8502005C` | `0x816C0038` | `ref(Obj) event` (`0x1682` `GuildCalendarEvent`) | Kinship Calendar event added/updated (`0x16830002`). |
| `0x8502005D` | `0x816C0039` | `u64 event_id` | Kinship Calendar event deleted (`0x16830003`). |
| `0x8502005E` | `0x816C003A` | `ref(Obj) signups_map`, `u64 event_id` | Kinship Calendar event signups synchronized (`0x16830004`). |

#### 6.9.5 WState Layouts for `Guild` (`0x048C`), `GuildMember` (`0x048F`), and `GuildRank` (`0x0490`)

- **`Guild` (`0x048C` / `0x0169`, `vslot 2 = 0x848C0006`):** `u32 schema_version (7)`, `u64 guild_iid`, `ref(WString) name`, `u64 leader_iid`, `u64 successor_iid`, `f64 creation_timestamp`, `u32 lifespan_rank`, `u32 governance_type`, `ref(WString) motd`, `HashTable<u32, ref(0x0490)> ranks`, `HashTable<u64, ref(0x048F)> members`, `u64 kinship_house_iid`, `u32 member_count`.
- **`GuildMember` (`0x048F` / `0x016A`, `vslot 2 = 0x848F0005`):** `u32 schema_version (4)`, `u64 member_iid`, `ref(WString) name`, `u32 rank_id`, `u32 level`, `u32 class_id`, `u32 area_did`, `bool is_online`, `f64 last_online_timestamp`, `ref(WString) player_note`, `ref(WString) officer_note`, `u32 vocation_id`, `bool is_anonymous`.
- **`GuildRank` (`0x0490` / `0x016B`, `vslot 2 = 0x84900002`):** `u32 schema_version (1)`, `u32 rank_id`, `ref(WString) title`, `u32 permissions_mask`.


#### 6.9.6 `MusicNoteBatch` (`0x0804`, decimal `2052`) Native WState Wire Layout 

Used in both `0x85190015` (C→S) and `0x8519001A` (S→C). Inside `Pack_ObjRef` (`u32 class_id = 0x00000804` and WState class header), the serializer writes the note array followed by the performer metadata trailer (`BIN-MUSIC-NAT`):

| Offset | Size | Type | Field Name | Meaning |
|---|---|---|---|---|
| `+0x00` | `4` | `u32 LE` | `note_count` | Number of note events in this batch (`N`). Unpacker validates `N <= remaining_bytes`. |
| `+0x04` | `20 * N` | `NoteEvent[N]` | `events` | Array of `N` 20-byte note events (see sub-table below). |
| `+0x04 + 20*N` | `8` | `u64 LE` | `performer_iid` | Performer character `InstanceID` (`[batch + 0x10]`). |
| `+0x0C + 20*N` | `1` | `u8` | `flag_b` | Boolean flag `< 2` (`[batch + 0x18]`; unpacker rejects values `>= 2` with `0x80004005`). |
| `+0x0D + 20*N` | `1` | `u8` | `flag_a` | Boolean flag `< 2` (`[batch + 0x19]`; unpacker rejects values `>= 2` with `0x80004005`). |
| `+0x0E + 20*N` | `4` | `u32 LE` | `instrument_did` | Equipped instrument item/sound DataID (`[batch + 0x1C]`). |

#### `NoteEvent` Entry Layout (`20` bytes on wire; `24` bytes in memory at `[array_base + i * 24]`)

| Entry Offset | Size | Type | Memory Offset | Field Name | Meaning |
|---|---|---|---|---|---|
| `+0x00` | `4` | `u32 LE` | `+0x00` | `is_note_on` | `1` = Note-On event, `0` = Note-Off (release) event. |
| `+0x04` | `8` | `f64 LE` | `+0x08` | `timestamp` | Note event timestamp in seconds (IEEE-754 double). |
| `+0x0C` | `4` | `u32 LE` | `+0x10` | `note_pitch` | Note pitch index (`1..37` for interactive 3-octave lutings, or MIDI note pitch). |
| `+0x10` | `4` | `u32 LE` | `+0x14` | `velocity_or_part` | Note velocity / dynamic level / channel parameter. |

---

---

## 7. P5 — Map Markers, Fog-of-War, Stable-Master Travel, Mounts/War-Steeds & Instance Finder <a id="p5-world-travel-instances"></a>

### 7.1 Map Markers, Waypoints, Fellowship Positions, Entity Tracking, & Fog-of-War (`WorldEvent::MarkerAdded`, `MarkerRemoved`) <a id="p5-map-markers"></a>

#### 7.1.1 Overview & Provenance

Map and radar markers (`WorldEvent::MarkerAdded`, `WorldEvent::MarkerRemoved`), Point-of-Interest (POI) discovery, and map fog-of-war bitmaps are synchronized using WLib RPC classes `0x0530` (Entity Tracking), `0x050E` (Landmark Discovery), `0x0515` (Map Fog-of-War), `0x04E3`/`0x04E4` (Area Query), `0x04FE`/`0x0739` (Fellowship Positions & Target Marks), and `0x0521` (Quest Objective Events & Tracker), together with entity property updates (`BIN`, `DAT`, `OBS`).

#### 7.1.2 WLib RPC Summary Table — Map, Tracking, Landmark, & Waypoint Messages

| `function_id` | Class ID | Method ID | Direction | Domain Mapping | Wire Arguments (FIFO Order) | Provenance |
|---|---|---|---|---|---|---|
| `0x8530000C` | `0x0530` | `0x000C` (`12`) | S->C | `WorldEvent::MarkerAdded` | 1. `tracking_state: u32` (`!= 0`)<br>2. `entity_name: StringInfo` (`ObjRef`)<br>3. `category_or_icon: u32`<br>4. `position: Position` (`ObjRef`)<br>5. `tracked_iid: u64` |Confirmed (`BIN`) |
| `0x85300011` | `0x0530` | `0x0011` (`17`) | S->C | `WorldEvent::MarkerRemoved` | 1. `stop_reason: u32`<br>2. `tracking_state: u32`<br>3. `tracked_iid: u64` | Confirmed (`BIN`) |
| `0x85300006` | `0x0530` | `0x0006` (`6`) | S->C | `WorldEvent::MarkerAdded` (update) | 1. `tracking_flags: u32`<br>2. `position: Position` (`ObjRef`)<br>3. `tracked_iid: u64` | Confirmed (`BIN`) |
| `0x85300019` | `0x0530` | `0x0019` (`25`) | S->C | `WorldEvent::MarkerAdded` (position) | 1. `position: Position` (`ObjRef`)<br>2. `tracked_iid: u64` | Confirmed (`BIN`) |
| `0x8530001E` | `0x0530` | `0x001E` (`30`) | S->C | `WorldEvent::MarkerAdded` (state) | 1. `tracking_state: u32`<br>2. `tracked_iid: u64` | Confirmed (`BIN`) |
| `0x850E0003` | `0x050E` | `0x0003` (`3`) | S->C | `WorldEvent::MarkerAdded` (POI) | 1. `discovered_did: u32`<br>2. `landmark_did: u32` | Confirmed (`BIN`) |
| `0x850E000B` | `0x050E` | `0x000B` (`11`) | S->C | Bulk POI discovery sync | 1. `discovered_landmarks: ObjRef` | Confirmed (`BIN`) |
| `0x85150001` | `0x0515` | `0x0001` (`1`) | C->S | Save fog-of-war bitmap | 1. `fog_of_war_data: SmartBuffer` (`ObjRef` `0x008E`)<br>2. `map_did: u32` | Confirmed (`BIN`) |
| `0x85150002` | `0x0515` | `0x0002` (`2`) | C->S | Request fog-of-war bitmap | 1. `map_did: u32` | Confirmed (`BIN`) |
| `0x85150004` | `0x0515` | `0x0004` (`4`) | S->C | Receive fog-of-war bitmap | 1. `fog_of_war_data: SmartBuffer` (`ObjRef` `0x008E`)<br>2. `map_did: u32` | Confirmed (`BIN`) |
| `0x84FE0020` | `0x04FE` | `0x0020` (`32`) | S->C | `WorldEvent::MarkerAdded` (fellow) | 1. `position: Position` (`ObjRef`)<br>2. `member_iid: u64` | Confirmed (`BIN`) |
| `0x84FE00C6` | `0x04FE` | `0x00C6` (`198`) | S->C | Set fellowship target mark | 1. `mark_did: u32`<br>2. `target_iid: u64` | Confirmed (`BIN`) |
| `0x84FE00C4` | `0x04FE` | `0x00C4` (`196`) | S->C | Clear fellowship target mark | 1. `mark_did: u32`<br>2. `target_iid: u64` | Confirmed (`BIN`) |
| `0x87390015` | `0x0739` | `0x0015` (`21`) | C->S | Request set target mark | 1. `mark_did: u32`<br>2. `target_iid: u64` | Confirmed (`BIN`) |
| `0x87390011` | `0x0739` | `0x0011` (`17`) | C->S | Request clear target mark | 1. `mark_did: u32`<br>2. `target_iid: u64` | Confirmed (`BIN`) |
| `0x85210051` | `0x0521` | `0x0051` (`81`) | S->C | Update quest objective event | 1. `event_data: ObjRef`<br>2. `objective_idx: u32`<br>3. `quest_did: u32` | Confirmed (`BIN`) |
| `0x852100B5` | `0x0521` | `0x00B5` (`181`) | S->C | Update entity quest event map | 1. `event_counts_map: ObjRef` (`IntHash` `0x000B`)<br>2. `entity_iid: u64` | Confirmed (`BIN`) |
| `0x852100CE` | `0x0521` | `0x00CE` (`206`) | S->C | Increment quest event count | 1. `delta: u32`<br>2. `event_did: u32`<br>3. `quest_did: u32` | Confirmed (`BIN`) |
| `0x852100CF` | `0x0521` | `0x00CF` (`207`) | S->C | Decrement quest event count | 1. `delta: u32`<br>2. `event_did: u32`<br>3. `quest_did: u32` | Confirmed (`BIN`) |
| `0x852100D5` | `0x0521` | `0x00D5` (`213`) | S->C | Refresh quest tracker arrays | *(0 arguments)* | Confirmed (`BIN`) |
| `0x852100D6` | `0x0521` | `0x00D6` (`214`) | C->S | Update quest tracker entry | 1. `quest_did: u32`<br>2. `tracker_action_or_slot: u32` | Confirmed (`BIN`) |

#### 7.1.3 Key Properties — Area, Landmark, & Quest Waypoint Discovery

| Property ID | Property Name | Type | Entity Scope | Meaning | Provenance |
|---|---|---|---|---|---|
| `0x00000092` | `Area_DID` | `DataID` (`u32`) | Player | Current world area DataID; change triggers area discovery check (`0x10000E00`). | Confirmed (`BIN`, `DAT`) |
| `0x00000625` | `Dungeon_DID` | `DataID` (`u32`) | Player | Current dungeon DataID; change updates active dungeon map. | Confirmed (`BIN`, `DAT`) |
| `0x0000053B` | `Area_Override_DID` | `DataID` (`u32`) | Player | Sub-area override DataID. | Confirmed (`BIN`) |
| `0x10000856` | `Landmark_DiscoveredDID` | `DataID` (`u32`) | Event / Player | Newly discovered landmark DataID in `0x850E0003`. | Confirmed (`BIN`, `DAT`) |
| `0x100001E6` | `Landmark_DataID` | `DataID` (`u32`) | Landmark | Static DataID of a world landmark / POI. | Confirmed (`BIN`, `DAT`) |
| `0x1000038E` | `QuestEvent_LandmarkDID` | `DataID` (`u32`) | Quest Event | Target landmark DataID for a quest objective waypoint. | Confirmed (`BIN`, `DAT`) |
| `0x10001139` | `QuestEvent_LandmarkRegion` | `u32` / `Array` | Quest Event | Target region overlay for a quest objective. | Confirmed (`BIN`, `DAT`) |
| `0x10000E6B` | `Quest_TrackerArray` | `Array` | Player | Array of active quest DataIDs pinned to the Quest Tracker and Map Quest Guide. | Confirmed (`BIN`, `DAT`) |
| `0x10000291` | `Quest_TrackerArrayLocked` | `bool` | Player | Whether the player's Quest Tracker list is locked. | Confirmed (`BIN`, `DAT`) |

#### 7.1.4 Byte Layout — Fog-of-War `SmartBuffer` (`0x008E`) Native `ObjRef` Payload (`0x85150001` / `0x85150004`)

In `0x85150001` (C->S) and `0x85150004` (S->C), the `fog_of_war_data` `ObjRef` argument is an instance of WLib native class `0x008E` (`SmartBuffer`). Following the standard WLib `ObjRef` object-table header (`VLE` `object_count`, `u32[object_count]` `object_ids`) and per-object header (`u32` `object_flags` masked with `0xBB000000` where bit 28 `0x10000000` is `0` for native classes, followed by `u16` `class_id = 0x008E`), the native binary serializer writes/reads:

| Offset | Size (Bytes) | Type | Field | Meaning | Byte Order | Provenance |
|---|---|---|---|---|---|---|
| `0x00` | `4` | `u32` | `byte_length` | Number of raw fog-of-war bitmap bytes in `bitmap_bytes`. | Little-endian | Confirmed (`BIN`) |
| `0x04` | `byte_length` | `u8[byte_length]` | `bitmap_bytes` | Raw per-map fog-of-war exploration bitmap payload. | Byte array | Confirmed (`BIN`) |

---

### 7.2 Stable-Master Travel, Milestone, Recall, & Summon (`WorldEvent::TravelShown`, `TravelStarted`, `TravelEnded`, `WorldCommand::ChooseTravel`) <a id="p5-travel"></a>

#### 7.2.1 Overview & Provenance

Stable-Master travel uses WLib RPC class `0x0533` (`1331`). Interacting with a Stable-Master NPC sends C->S `0x85330008`; the server replies with S->C `0x85330002` (`npc_iid`) to open the travel window (`WorldEvent::TravelShown`). Route definitions (`TravelWeb`, `TravelDestination`, `TravelRoute`) are resolved from the NPC's `TravelWebWC` (`0x10000F66`) property in the client DAT files and combined with the player's discovered location set (`0x85330006`, `0x8533000B`, `0x8533001B`) and discount properties (`0x1000064A`, `0x100027EE`). Selecting a route (`WorldCommand::ChooseTravel`) sends C->S `0x8533000D` (`route_did`, `npc_iid`). Starting and ending a travel route (`WorldEvent::TravelStarted`, `WorldEvent::TravelEnded`) are driven by player entity property `Travel_IsTraveling` (`0x100009E8`), and early dismount sends C->S `0x85330005` (`BIN`, `DAT`, `OBS`).

#### 7.2.2 WLib RPC Summary Table — Travel, Recall, & Summon

| `function_id` | Class ID | Method ID | Direction | Domain Mapping | Wire Arguments (FIFO Order) | Provenance |
|---|---|---|---|---|---|---|
| `0x85330008` | `0x0533` | `0x0008` (`8`) | C->S | Interact with Stable-Master | 1. `flags: u32`<br>2. `is_admin_discover: u32` (`0` normal, `1` admin)<br>3. `npc_iid: u64` | Confirmed (`BIN`) |
| `0x85330002` | `0x0533` | `0x0002` (`2`) | S->C | `WorldEvent::TravelShown` | 1. `npc_iid: u64` | Confirmed (`BIN`) |
| `0x8533000D` | `0x0533` | `0x000D` (`13`) | C->S | `WorldCommand::ChooseTravel` | 1. `route_did: u32`<br>2. `npc_iid: u64` | Confirmed (`BIN`) |
| `0x85330005` | `0x0533` | `0x0005` (`5`) | C->S | Dismount / end travel early | *(0 arguments)* | Confirmed (`BIN`) |
| `0x85330006` | `0x0533` | `0x0006` (`6`) | S->C | Sync discovered locations | 1. `discovered_locations: ObjRef` (`IntSet`) | Confirmed (`BIN`) |
| `0x8533000B` | `0x0533` | `0x000B` (`11`) | S->C | Update discovered location | 1. `is_added: u32` (`1` add, `0` remove)<br>2. `location_did: u32` | Confirmed (`BIN`) |
| `0x8533001B` | `0x0533` | `0x001B` (`27`) | S->C | Discover NPC travel location | 1. `location_did: u32`<br>2. `npc_iid: u64` | Confirmed (`BIN`) |
| `0x85330022` | `0x0533` | `0x0022` (`34`) | C->S | Travel to fellowship leader | 1. `leader_iid: u64` | Confirmed (`BIN`) |
| `0x85330025` | `0x0533` | `0x0025` (`37`) | S->C | Update travel state slot 852 | 1. `travel_obj: ObjRef`<br>2. `key_u32: u32` | Confirmed (`BIN`) |
| `0x85330026` | `0x0533` | `0x0026` (`38`) | S->C | Update travel state slot 851 | 1. `val_u32: u32` | Confirmed (`BIN`) |
| `0x85240002` | `0x0524` | `0x0002` (`2`) | C->S | Bind milestone skill | 1. `milestone_skill_did: u32` | Confirmed (`BIN`) |
| `0x8524000E` | `0x0524` | `0x000E` (`14`) | C->S | Discover milestone landmark | 1. `landmark_did: u32` | Confirmed (`BIN`) |
| `0x852A0003` | `0x052A` | `0x0003` (`3`) | S->C | Receive player summon prompt | 1. `is_combat_summon: u32`<br>2. `destination_name: StringInfo` (`ObjRef`)<br>3. `summoner_iid: u64` | Confirmed (`BIN`) |
| `0x852A0004` | `0x052A` | `0x0004` (`4`) | C->S | Respond to player summon | 1. `accepted_bool: u32` | Confirmed (`BIN`) |

#### 7.2.3 Byte Layout — `ChooseTravel` (`0x8533000D`) `WLibArgFrame` Payload

| Offset | Size (Bytes) | Type | Field | Meaning | Byte Order | Provenance |
|---|---|---|---|---|---|---|
| `0x00` | `4` | `u32` | `route_did` | DataID of the selected `TravelRoute` (`0x05E1`). | Little-endian | Confirmed (`BIN`) |
| `0x04` | `8` | `u64` | `npc_iid` | InstanceID of the Stable-Master NPC. | Little-endian | Confirmed (`BIN`) |

#### 7.2.4 Travel State Machine & Properties

1. **Opening the Travel Window (`TravelShown`):**
 - Client sends `0x85330008` (`flags = 0`, `is_admin_discover = 0`, `npc_iid`).
 - If the NPC's `TravelHomeLocation` (`0x10001502`) was not previously discovered, the server sends `0x8533001B` (`location_did`, `npc_iid`).
 - Server sends `0x85330002` (`npc_iid`), triggering `WorldEvent::TravelShown`.
 - Client resolves all routes in the NPC's `TravelWebWC` (`0x10000F66`), computing discounted copper coin cost (`TravelRouteCost` `0x10001503` * `TravelWebSellMultiplier` `0x10001419` * `(1.0 - discount)`), swift travel flag (`TravelDestinationIsSwiftTravel` `0x10005257`), discovered status, and usage requirement status.
2. **Selecting a Route (`ChooseTravel` -> `TravelStarted`):**
 - If `Mount_Is_Mounted` (`0x10000AAF`) is `1`, the client first sends `0x88000006` (Dismount).
 - Client sends `0x8533000D` (`route_did`, `npc_iid`).
 - Server deducts coin cost, mounts the player on the route's travel steed (`Mount_Is_Mounted` `0x10000AAF` = `1`), and sets `Travel_RouteDID` (`0x10001B2B`) = `route_did`, `Travel_Allow_Dismount` (`0x10004031`), and `Travel_IsTraveling` (`0x100009E8`) = `1`, triggering `WorldEvent::TravelStarted`.
3. **Arriving or Dismounting Early (`TravelEnded`):**
 - If `Travel_Allow_Dismount` (`0x10004031`) is `1` and the player clicks Dismount during travel, the client sends `0x85330005` (0 args).
 - Upon arrival or early termination, the server sets `Travel_IsTraveling` (`0x100009E8`) = `0` and `Mount_Is_Mounted` (`0x10000AAF`) = `0`, triggering `WorldEvent::TravelEnded`.

---

### 7.3 Mounts, War-Steeds, Mounted Combat, & Mount Outfits (`WorldEvent::MountChanged`, `MountVitalChanged`, `WorldCommand::Dismount`) <a id="p5-mounts"></a>

#### 7.3.1 Overview & Provenance

Summoning a mount or war-steed uses standard skill execution (`WorldCommand::UseSkill`). Dismounting (`WorldCommand::Dismount`) sends the 0-argument WLib RPC `0x88000006`. All mount state changes (`WorldEvent::MountChanged`) and standard/war-steed vital changes (`WorldEvent::MountVitalChanged`) are delivered via property updates on the rider entity (`BIN`, `DAT`, `OBS`).

#### 7.3.2 WLib RPC Summary Table — Mounts, Mounted Combat, & Mount Outfits

| `function_id` | Class ID | Method ID | Direction | Domain Mapping | Wire Arguments (FIFO Order) | Provenance |
|---|---|---|---|---|---|---|
| `0x88000006` | `0x0800` | `0x0006` (`6`) | C->S | `WorldCommand::Dismount` | *(0 arguments)* | Confirmed (`BIN`) |
| `0x88000008` | `0x0800` | `0x0008` (`8`) | C->S | Rename mount / war-steed | 1. `mount_category: u32` (`1` normal, `2` war-steed)<br>2. `new_name: WString` (`ObjRef`) | Confirmed (`BIN`) |
| `0x8DA70007` | `0x0DA7` | `0x0007` (`7`) | C->S | Refund war-steed traits | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8DA7000A` | `0x0DA7` | `0x000A` (`10`) | C->S | Spend war-steed trait points | 1. `trait_rank_map: ObjRef` (`IntHash`) | Confirmed (`BIN`) |
| `0x8DA70010` | `0x0DA7` | `0x0010` (`16`) | C->S | Respec war-steed trait tree | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8DA70013` | `0x0DA7` | `0x0013` (`19`) | C->S | Select war-steed specialization | 1. `spec_tree_index: u32` | Confirmed (`BIN`) |
| `0x8E150001` | `0x0E15` | `0x0001` (`1`) | C->S | Select war-steed outfit slot | 1. `slot_index: u32` | Confirmed (`BIN`) |
| `0x8E150003` | `0x0E15` | `0x0003` (`3`) | C->S | Save war-steed outfit slot | 1. `appearance_map: ObjRef` (`IntHash`)<br>2. `slot_index: u32` | Confirmed (`BIN`) |
| `0x8E15000E` | `0x0E15` | `0x000E` (`14`) | C->S | Toggle war-steed piece visibility | 1. `is_hidden: u32`<br>2. `piece_slot: u32`<br>3. `outfit_slot_index: u32` | Confirmed (`BIN`) |
| `0x8E15000F` | `0x0E15` | `0x000F` (`15`) | C->S | Query war-steed outfit slots | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8E150012` | `0x0E15` | `0x0012` (`18`) | C->S | Purchase war-steed outfit slot | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8E150007` | `0x0E15` | `0x0007` (`7`) | S->C | Update war-steed outfit slot | 1. `slot_data: ObjRef`<br>2. `slot_index: u32` | Confirmed (`BIN`) |
| `0x8E150009` | `0x0E15` | `0x0009` (`9`) | S->C | Sync all war-steed outfit slots | 1. `all_slots_data: ObjRef` | Confirmed (`BIN`) |
| `0x8E15000B` | `0x0E15` | `0x000B` (`11`) | S->C | Confirm war-steed outfit slot | 1. `slot_index: u32` | Confirmed (`BIN`) |

#### 7.3.3 Rider Entity Properties — `MountChanged` & `MountVitalChanged`

| Property ID | Property Name | Type | Domain Event | Meaning & Units | Provenance |
|---|---|---|---|---|---|
| `0x10000AAF` | `Mount_Is_Mounted` | `bool` | `MountChanged` | `1` when mounted, `0` when unmounted. | Confirmed (`BIN`, `DAT`) |
| `0x10002A90` | `Mount_CurrentMountType` | `u32` | `MountChanged` | Active mount type bitmask; `(val & 0x38) != 0` indicates a combat mount (war-steed: `0x08` Light, `0x10` Medium, `0x20` Heavy). | Confirmed (`BIN`, `DAT`) |
| `0x10002A79` | `Mount_Cache_MountEffectDID` | `DataID` (`u32`) | `MountChanged` | DataID of the active mount effect (`Mount_DataID` `0x10001715`). | Confirmed (`BIN`, `DAT`) |
| `0x10001716` | `Mount_MountIID` | `InstanceID` (`u64`) | `MountChanged` | InstanceID of the mount entity. | Confirmed (`BIN`, `DAT`) |
| `0x10002A62` | `Mount_Durability_Current_Value` | `f32` | `MountVitalChanged` | Current durability (morale) of a standard mount. | Confirmed (`BIN`, `DAT`) |
| `0x10002A64` | `Mount_Durability_Current_Max` | `f32` | `MountVitalChanged` | Maximum durability of a standard mount. | Confirmed (`BIN`, `DAT`) |
| `0x10004939` | `MountEndurance_CurrentLevel` | `f32` | `MountVitalChanged` | Current War-Steed Endurance (morale). | Confirmed (`BIN`, `DAT`) |
| `0x10004935` | `MountEndurance_MaxLevel` | `f32` | `MountVitalChanged` | Maximum War-Steed Endurance. | Confirmed (`BIN`, `DAT`) |
| `0x1000493E` | `MountEndurance_Temporary` | `f32` | `MountVitalChanged` | Temporary War-Steed Endurance bubble. | Confirmed (`BIN`, `DAT`) |
| `0x1000493B` | `MountEndurance_Temporary_Max` | `f32` | `MountVitalChanged` | Maximum temporary War-Steed Endurance. | Confirmed (`BIN`, `DAT`) |
| `0x10004933` | `MountPower_CurrentLevel` | `f32` | `MountVitalChanged` | Current War-Steed Power. | Confirmed (`BIN`, `DAT`) |
| `0x10004937` | `MountPower_MaxLevel` | `f32` | `MountVitalChanged` | Maximum War-Steed Power. | Confirmed (`BIN`, `DAT`) |
| `0x10004938` | `MountPower_Temporary` | `f32` | `MountVitalChanged` | Temporary War-Steed Power bubble. | Confirmed (`BIN`, `DAT`) |
| `0x10004932` | `MountPower_Temporary_Max` | `f32` | `MountVitalChanged` | Maximum temporary War-Steed Power. | Confirmed (`BIN`, `DAT`) |
| `0x100037C2` | `Pip_Momentum` | `i32` / `f32` | `MountVitalChanged` | War-Steed Fury / Momentum level. | Confirmed (`BIN`, `DAT`) |
| `0x10004BE8` | `Mount_MountGear` | `i32` | `MountVitalChanged` | War-Steed gait gear (`0..4`). | Confirmed (`BIN`, `DAT`) |
| `0x10004120` | `MountMovement_CurrentSpeed` | `f32` | `MountVitalChanged` | Current War-Steed speed (`m/s`). | Confirmed (`BIN`, `DAT`) |
| `0x10004953` | `Stat_MountAgility` | `f32` | `MountVitalChanged` | War-Steed Agility rating. | Confirmed (`BIN`, `DAT`) |
| `0x10004954` | `Stat_MountStrength` | `f32` | `MountVitalChanged` | War-Steed Strength rating. | Confirmed (`BIN`, `DAT`) |
| `0x10004963` | `Combat_Agent_MountArmor_Value_Float` | `f32` | `MountVitalChanged` | War-Steed Armor rating. | Confirmed (`BIN`, `DAT`) |

---

### 7.4 Instance Finder (`WorldJoin` & `FellowshipMaker`), Private Encounters, & Skirmishes (`WorldEvent::InstanceListShown`, `InstanceChanged`, `InstanceQueueChanged`, `WorldCommand::JoinInstance`, `LeaveInstance`, `LeaveQueue`) <a id="p5-instances"></a>

#### 7.4.1 Overview & Provenance

The Instance Finder and Private Encounter lifecycle spans four WLib RPC classes (`BIN`, `DAT`, `OBS`):
- **World Join (`0x0A2C`):** Direct instance selection and launch (`0x8A2C0001` query list -> `0x8A2C0003` `WorldEvent::InstanceListShown`; `0x8A2C0005` `WorldCommand::JoinInstance`; `0x8A2C0007` cancel; `0x8A2C000B` invite -> `0x8A2C0009` response).
- **Fellowship Maker (`0x0C41`):** Automated matchmaking queue (`0x8C410055` queue specific; `0x8C410056` queue random; `0x8C41000D` `WorldCommand::LeaveQueue`; `0x8C41000F` confirm fellowship queue entry; `0x8C41004F` confirm pre-fellowship match; `0x8C410028`/`0x8C410034`/`0x8C410037` `WorldEvent::InstanceQueueChanged`).
- **Private Encounter (`0x051F`):** Active instance entry (`0x851F000C` `WorldEvent::InstanceChanged`), instance exit notification (`0x851F0027` `WorldEvent::InstanceChanged`), and leaving an active instance (`0x851F0136` `WorldCommand::LeaveInstance`).
- **Skirmish (`0x09EC`):** Skirmish statistics updates (`0x89EC0007`, `0x89EC000A`, `0x89EC000E`, `0x89EC0021`).

#### 7.4.2 WLib RPC Summary Table — Instance Finder & Private Encounters

| `function_id` | Class ID | Method ID | Direction | Domain Mapping | Wire Arguments (FIFO Order) | Provenance |
|---|---|---|---|---|---|---|
| `0x8A2C0001` | `0x0A2C` | `0x0001` (`1`) | C->S | Request available instances | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8A2C0003` | `0x0A2C` | `0x0003` (`3`) | S->C | `WorldEvent::InstanceListShown` | 1. `available_encounters: ObjRef` | Confirmed (`BIN`) |
| `0x8A2C0005` | `0x0A2C` | `0x0005` (`5`) | C->S | `WorldCommand::JoinInstance` | 1. `request: WorldJoinRequest` (`ObjRef` `0x0A44`) | Confirmed (`BIN`) |
| `0x8A2C0007` | `0x0A2C` | `0x0007` (`7`) | C->S | `WorldCommand::LeaveQueue` (WorldJoin) | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8A2C0009` | `0x0A2C` | `0x0009` (`9`) | C->S | Respond to WorldJoin invite | 1. `invite: WorldJoinInvite` (`ObjRef` `0x0A39`) | Confirmed (`BIN`) |
| `0x8A2C000B` | `0x0A2C` | `0x000B` (`11`) | S->C | `WorldEvent::InstanceQueueChanged` (invite) | 1. `invite: WorldJoinInvite` (`ObjRef` `0x0A39`) | Confirmed (`BIN`) |
| `0x8A2C002F` | `0x0A2C` | `0x002F` (`47`) | C->S | Query raid/instance lock status | 1. `request: WorldJoinRequest` (`ObjRef` `0x0A44`) | Confirmed (`BIN`) |
| `0x8A2C0031` | `0x0A2C` | `0x0031` (`49`) | S->C | Receive raid/instance lock status | 1. `request_with_locks: WorldJoinRequest` (`ObjRef` `0x0A44`) | Confirmed (`BIN`) |
| `0x8A2C0045` | `0x0A2C` | `0x0045` (`69`) | C->S | Cancel WorldJoin invite | 1. `invite: WorldJoinInvite` (`ObjRef` `0x0A39`) | Confirmed (`BIN`) |
| `0x8A2C0049` | `0x0A2C` | `0x0049` (`73`) | S->C | `WorldEvent::InstanceQueueChanged` (status) | 1. `status_code: u32` | Confirmed (`BIN`) |
| `0x8A2C0051` | `0x0A2C` | `0x0051` (`81`) | C->S | Query active WorldJoin request | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8A2C0059` | `0x0A2C` | `0x0059` (`89`) | C->S | Dismiss WorldJoin prompt | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8A2C0062` | `0x0A2C` | `0x0062` (`98`) | S->C | `WorldEvent::InstanceQueueChanged` (queue) | 1. `queue_enabled_status: u32` | Confirmed (`BIN`) |
| `0x8C410055` | `0x0C41` | `0x0055` (`85`) | C->S | `WorldCommand::JoinInstance` (queue specific) | 1. `encounter_config_map: ObjRef` (`IntHash`)<br>2. `config_flags: u32`<br>3. `roles_mask: u32` | Confirmed (`BIN`) |
| `0x8C410056` | `0x0C41` | `0x0056` (`86`) | C->S | `WorldCommand::JoinInstance` (queue random) | 1. `config_flags: u32`<br>2. `roles_mask: u32` | Confirmed (`BIN`) |
| `0x8C41000D` | `0x0C41` | `0x000D` (`13`) | C->S | `WorldCommand::LeaveQueue` | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8C41000F` | `0x0C41` | `0x000F` (`15`) | C->S | Confirm fellowship queue entry | 1. `confirm_roles_or_bool: u32` | Confirmed (`BIN`) |
| `0x8C410019` | `0x0C41` | `0x0019` (`25`) | C->S | Set matchmaking role mask | 1. `roles_mask: u32` | Confirmed (`BIN`) |
| `0x8C41004F` | `0x0C41` | `0x004F` (`79`) | C->S | Confirm pre-fellowship ready check | *(0 arguments)* | Confirmed (`BIN`) |
| `0x8C410028` | `0x0C41` | `0x0028` (`40`) | S->C | `WorldEvent::InstanceQueueChanged` | 1. `confirmed_bool: u32`<br>2. `fellow_iid: u64` | Confirmed (`BIN`) |
| `0x8C410034` | `0x0C41` | `0x0034` (`52`) | S->C | `WorldEvent::InstanceQueueChanged` | 1. `assigned_roles_list: ObjRef` (`IntArray`)<br>2. `member_roles_map: ObjRef` (`LongHash`) | Confirmed (`BIN`) |
| `0x8C410037` | `0x0C41` | `0x0037` (`55`) | S->C | `WorldEvent::InstanceQueueChanged` | 1. `status_or_role: u32`<br>2. `member_iid: u64` | Confirmed (`BIN`) |
| `0x8C410049` | `0x0C41` | `0x0049` (`73`) | S->C | `WorldEvent::InstanceQueueChanged` (error) | 1. `cooldown_or_error_map: ObjRef` (`LongHash`) | Confirmed (`BIN`) |
| `0x851F000C` | `0x051F` | `0x000C` (`12`) | S->C | `WorldEvent::InstanceChanged` (enter) | 1. `template_did: u32` | Confirmed (`BIN`) |
| `0x851F0027` | `0x051F` | `0x0027` (`39`) | S->C | `WorldEvent::InstanceChanged` (leave) | *(0 arguments)* | Confirmed (`BIN`) |
| `0x851F0136` | `0x051F` | `0x0136` (`310`) | C->S | `WorldCommand::LeaveInstance` | *(0 arguments)* | Confirmed (`BIN`) |
| `0x851F002C` | `0x051F` | `0x002C` (`44`) | C->S | Launch encounter via NPC/door | 1. `request: WorldJoinRequest` (`ObjRef` `0x0A44`)<br>2. `npc_or_door_iid: u64` | Confirmed (`BIN`) |
| `0x851F00BA` | `0x051F` | `0x00BA` (`186`) | S->C | Update encounter timer | 1. `time_remaining_sec: f32` | Confirmed (`BIN`) |
| `0x851F011D` | `0x051F` | `0x011D` (`285`) | S->C | Display encounter banner | 1. `banner_text: StringInfo` (`ObjRef`) | Confirmed (`BIN`) |
| `0x851F0150` | `0x051F` | `0x0150` (`336`) | S->C | Display encounter error alert | 1. `error_text: StringInfo` (`ObjRef`) | Confirmed (`BIN`) |
| `0x851F015B` | `0x051F` | `0x015B` (`347`) | S->C | Add encounter quest | 1. `quest_did: u32` | Confirmed (`BIN`) |
| `0x851F015E` | `0x051F` | `0x015E` (`350`) | S->C | Remove encounter quest | 1. `quest_did: u32` | Confirmed (`BIN`) |

#### 7.4.3 Key Properties — Instance Finder & Private Encounters

| Property ID | Property Name | Type | Entity Scope | Meaning | Provenance |
|---|---|---|---|---|---|
| `0x100007AB` | `PrivateEncounter_Template_DataID` | `DataID` (`u32`) | Player | Active `PrivateEncounterTemplate` DataID while inside an instance (`0` when on landscape). | Confirmed (`BIN`, `DAT`) |
| `0x10003DCA` | `FellowshipMaker_PlayerState` | `u32` | Player | Matchmaking queue state (`0` = Idle, `1` = Confirming Fellowship, `2` = Queued, `4` = Pre-Fellowship Ready Check, `7` = In Encounter). | Confirmed (`BIN`, `DAT`) |
| `0x10004166` | `FellowshipMaker_PlayerRole` | `u32` | Player | Bitmask of roles selected for Fellowship Maker matchmaking. | Confirmed (`BIN`, `DAT`) |
| `0x100041BE` | `FellowshipMaker_PlayerConfiguration` | `u32` | Player | Bitmask of group sizes (`0x10` Solo, `0x20` Duo, `0x40` Small Fellowship, `0x80` Fellowship), tiers, and encounter types selected for matchmaking. | Confirmed (`BIN`, `DAT`) |
| `0x100037F4` | `Skirmish_PlayerLevelAtSkirmishStart` | `i32` | Player | Player level recorded at the start of a skirmish. | Confirmed (`BIN`, `DAT`) |

#### 7.4.4 Byte Layouts — Native `ObjRef` Payloads (`0x0A44`, `0x0A39`, `0x000B`, `0x0011`, `0x0061`)

##### WLib `ObjRef` Stream Envelope

Every `Pack_ObjRef` / `Unpack_ObjRef` argument in a `WLibArgFrame` starts with an object-table header (`VLE` `object_count` followed by `u32[object_count]` `object_ids`). Each serialized object in the table begins with a 4-byte `u32` `object_flags` field (masked with `0xBB000000`, where bit 28 `0x10000000` is `0` for native classes and `1` for WLib script classes) and a 2-byte little-endian `u16` `class_id`, followed immediately by the class's binary payload (`BIN`).

##### Native Class `0x0A44` (`WorldJoinRequest`) Binary Payload

| Sequence | Size (Bytes) | Type | Field | Meaning | Byte Order | Provenance |
|---|---|---|---|---|---|---|
| `1` | `4` | `u32` | `encounter_did` | `PrivateEncounterTemplate` DataID (`WorldJoin_Encounter` `0x10003752`). | Little-endian | Confirmed (`BIN`, `DAT`) |
| `2` | `1` | `u8` | `param_bucket_idx` | Prime hash-bucket table index (`<= 50,000` buckets) for the `parameters` `HashTable<u32, u32>`. | Byte | Confirmed (`BIN`) |
| `3` | `1..5` | `VLE` | `param_count` | Number of `(key, value)` entries in `parameters`. | Little-endian VLE | Confirmed (`BIN`) |
| `4` | `8 * param_count` | `(u32, u32)[param_count]` | `parameters` | Key-value pairs storing `WorldJoin_Level` (`0x10003751`), `WorldJoin_DifficultyTier` (`0x10002ECA`), and `WorldJoin_GroupSize` (`0x10002EC9`). | Little-endian | Confirmed (`BIN`, `DAT`) |
| `5` | `1` | `u8` (`bool`) | `has_extra` | `1` when the versioned extension block is present (`1` on write). | Byte | Confirmed (`BIN`) |
| `6` | `4` (if `has_extra != 0`) | `u32` | `extra_version` | Extension version (`1`). | Little-endian | Confirmed (`BIN`) |
| `7` | `4` (if `extra_version >= 1`) | `u32` | `extra_field` | Secondary encounter flags / modifier value. | Little-endian | Confirmed (`BIN`) |
| `8` | `8` (if stream bit 14 == `0`) | `u64` | `pre_fellowship_id` | Matchmade pre-fellowship ID (`WorldJoin_PreFellowshipID` `0x1000422A`). | Little-endian | Confirmed (`BIN`, `DAT`) |
| `9` | `1` (if stream bit 14 == `0`) | `u8` | `pe_effect_bucket_idx` | Prime hash-bucket table index for `player_to_pe_effect_hash` (`HashTable<u64, u32>`). | Byte | Confirmed (`BIN`) |
| `10` | `1..5` (if stream bit 14 == `0`) | `VLE` | `pe_effect_count` | Number of `(player_iid, pe_effect_did)` entries (`WorldJoin_Encounter_PlayerToPEEffectHash_Array` `0x1000480D`). | Little-endian VLE | Confirmed (`BIN`, `DAT`) |
| `11` | `12 * pe_effect_count` | `(u64, u32)[pe_effect_count]` | `player_to_pe_effect_hash` | Per-player Private Encounter effect/lock pairs (`u64` `player_iid`, `u32` `pe_effect_did`). | Little-endian | Confirmed (`BIN`, `DAT`) |
| `12` | `4` (if stream bit 14 == `0`) | `u32` | `group_number` | Group number / raid lock flag (`WorldJoin_GroupNumber` `0x10003AD0`). | Little-endian | Confirmed (`BIN`, `DAT`) |

##### Native Class `0x0A39` (`WorldJoinInvite`) Binary Payload

| Sequence | Size (Bytes) | Type | Field | Meaning | Byte Order | Provenance |
|---|---|---|---|---|---|---|
| `1` | `8` | `u64` | `requester_iid` | InstanceID of the player who initiated the World Join request (`WorldJoin_RequesterIID` `0x10003756`). | Little-endian | Confirmed (`BIN`, `DAT`) |
| `2` | `4` | `u32` | `invitation_type` | Invitation state (`WorldJoin_InvitationType` `0x10003757`: `1` = pending confirmation, `2` = ready to travel). | Little-endian | Confirmed (`BIN`, `DAT`) |
| `3` | Variable | `WorldJoinRequest` (`0x0A44`) | `request` | Embedded `WorldJoinRequest` payload serialized inline using the `0x0A44` binary layout above. | Little-endian | Confirmed (`BIN`) |
| `4` | `4` | `u32` | `member_count` | Number of `MemberData` records in the invitation roster. | Little-endian | Confirmed (`BIN`) |
| `5` | Variable | `MemberData[member_count]` | `members` | Each `MemberData` record serializes in order: `u64` `member_iid`, `StringInfo` `member_name`, `u32` `class_or_role`, `u16` `level`, `u16` `rank_or_flags`, `u32` `kinship_or_title_id`, `u16` `status_flags`, `StringInfo` `secondary_string`, `u32` `response_state`, and `u8` `confirmed_bool`. | Little-endian | Confirmed (`BIN`) |
| `6` | `8` | `f64` | `invitation_create_time` | Invitation creation/expiration timestamp (`WorldJoin_InvitationCreateTime` `0x100037A4`). | IEEE-754 LE | Confirmed (`BIN`, `DAT`) |

##### Supporting Container Native Classes (`0x000B`, `0x0011`, `0x0061`)

| Class ID | Class Name | Binary Payload Layout | Provenance |
|---|---|---|---|
| `0x000B` | `IntHash` (`HashTable<u32, u32>`) | `u8` `bucket_size_index` + `VLE` `entry_count` + `entry_count` pairs of `(u32 key, u32 value)` (8 bytes LE per entry). | Confirmed (`BIN`) |
| `0x0011` | `IntArray` (`Array<u32>`) | `u32` `element_count` (4 bytes LE) + `element_count` `u32` values (4 bytes LE each). | Confirmed (`BIN`) |
| `0x0061` | `LongHash` (`HashTable<u64, u32>`) | `u8` `bucket_size_index` + `VLE` `entry_count` + `entry_count` pairs of `(u64 key, u32 value)` (12 bytes LE per entry). | Confirmed (`BIN`) |

---

### 7.5 Testable Invariants & Synthetic Test Vectors <a id="p5-test-vectors"></a>

#### Testable Invariants

1. **FIFO Argument Ordering in `WLibArgFrame`:** For every multi-argument P5 RPC (`0x8530000C`, `0x85300011`, `0x850E0003`, `0x85150001`, `0x85150004`, `0x85330008`, `0x8533000D`, `0x8C410055`, `0x8C410056`), the receiver pops fields in the exact order pushed by the sender.
2. **Pre-Travel Dismount Invariant:** When `WorldCommand::ChooseTravel` is executed while `Mount_Is_Mounted` (`0x10000AAF`) is non-zero, the client emits `0x88000006` (Dismount) prior to `0x8533000D` (`ChooseTravel`).
3. **War-Steed Type Bitmask Invariant:** A mounted player (`Mount_Is_Mounted` `0x10000AAF` != 0) is riding a War-Steed (combat mount) if and only if `(Mount_CurrentMountType & 0x38) != 0`.
4. **Entity Tracking Non-Zero State Invariant:** `CTrackingSystem::StartTracking` (`0x8530000C`) and `UpdateTrackedEntity` (`0x8530001E`) reject `tracking_state == 0` as invalid and only insert non-zero states into the per-entity tracking set.

#### Synthetic Test Vectors

1. **Vector 1 — `WorldCommand::ChooseTravel` (`0x8533000D`, C->S):**
 - `function_id`: `0x8533000D` (`0D 00 33 85` LE)
 - `route_did`: `0x18701234` (`34 12 70 18` LE)
 - `npc_iid`: `0x0200000000ABCDEF` (`EF CD AB 00 00 00 00 02` LE)
 - Serialized `WLibArgFrame` scalar payload (12 bytes): `34 12 70 18 EF CD AB 00 00 00 00 02`

2. **Vector 2 — `WorldCommand::Dismount` (`0x88000006`, C->S):**
 - `function_id`: `0x88000006` (`06 00 00 88` LE)
 - Serialized `WLibArgFrame` payload (0 bytes): `""` (empty argument frame)

3. **Vector 3 — `WorldEvent::TravelShown` (`0x85330002`, S->C):**
 - `function_id`: `0x85330002` (`02 00 33 85` LE)
 - `npc_iid`: `0x0200000000ABCDEF` (`EF CD AB 00 00 00 00 02` LE)
 - Serialized `WLibArgFrame` scalar payload (8 bytes): `EF CD AB 00 00 00 00 02`

4. **Vector 4 — `WorldEvent::InstanceChanged` Enter (`0x851F000C`, S->C) & `WorldCommand::LeaveInstance` (`0x851F0136`, C->S):**
 - Enter (`0x851F000C` S->C): `function_id` = `0C 00 1F 85`, `template_did` = `0x18705678` (`78 56 70 18` LE).
 - Leave (`0x851F0136` C->S): `function_id` = `36 01 1F 85`, 0-byte argument frame.

5. **Vector 5 — `WorldJoinRequest` (`0x0A44`) & `SmartBuffer` (`0x008E`) Native Binary Payloads:**
 - `SmartBuffer` (`0x008E`) body with `byte_length = 4` and bitmap bytes `AA BB CC DD`: `04 00 00 00 AA BB CC DD` (8 bytes).
 - `WorldJoinRequest` (`0x0A44`) body with `encounter_did = 0x18705678`, `param_bucket_idx = 0`, `param_count = 3` (`(0x10003751, 20)`, `(0x10002ECA, 1)`, `(0x10002EC9, 6)`), `has_extra = 1`, `extra_version = 1`, `extra_field = 0`, `pre_fellowship_id = 0`, `pe_effect_bucket_idx = 0`, `pe_effect_count = 0`, `group_number = 0`:
 `78 56 70 18 00 03 51 37 00 10 14 00 00 00 CA 2E 00 10 01 00 00 00 C9 2E 00 10 06 00 00 00 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00` (53 bytes).

---

### 7.6 Open Questions <a id="p5-open"></a>

None. All P5 map, travel, mount, war-steed, and instance finder wire messages, opcodes, argument layouts, and native object serializations are confirmed from binary and data analysis.

---

---

### 7.7 Extended Mount & War-Steed Entity Property Table (`DAT`, `BIN`) <a id="p5-extended-mount-props"></a>

All mount state and vital changes are delivered via property updates on the rider entity:

| Property ID | Name | Type | Domain Event | Semantics |
|---|---|---|---|---|
| `0x10000AAF` | `Mount_Is_Mounted` | `bool` | `MountChanged` | `1` when mounted, `0` when unmounted (`0x89A00013`). |
| `0x10002A90` | `Mount_CurrentMountType` | `u32` | `MountChanged` | Active mount type bitmask (`(val & 0x38) != 0` = combat War-Steed: `0x08` Light, `0x10` Medium, `0x20` Heavy; `(val & 0x47) != 0` = standard mount). |
| `0x10002A79` | `Mount_Cache_MountEffectDID` | `DataID` (`u32`) | `MountChanged` | DataID of the active mount effect (`0x099B`), whose static DAT record contains `Mount_DataID` (`0x10001715`), `Mount_Species` (`0x100022B9`), `Mount_MountAppearance` (`0x100040B5`), and `Mount_RiderType` (`0x1000406B`). |
| `0x10002A39` | `Mount_Cache_MountEffectID` | `u32` | `MountChanged` | Active mount effect instance ID on the rider. |
| `0x10002A3A` | `Mount_CurrentMountStatus` | `u32` | `MountChanged` | Current mount status state. |
| `0x10001716` | `Mount_MountIID` | `InstanceID` (`u64`) | `MountChanged` | InstanceID of the mount entity. |
| `0x10004C45` | `Mount_ClientUI_Entity` | `InstanceID` (`u64`) | `MountChanged` | Client UI mount entity IID (`0x89A50008`). |
| `0x10002A62` | `Mount_Durability_Current_Value` | `f32` | `MountVitalChanged` | Current durability (morale) of a standard (non-combat) mount (`0x89A00002`). |
| `0x10002A64` | `Mount_Durability_Current_Max` | `f32` | `MountVitalChanged` | Maximum durability of a standard mount (`0x89A00003`). |
| `0x10002A63` | `Mount_Durability_Base_Max` | `f32` | `MountVitalChanged` | Base maximum durability of a standard mount. |
| `0x10002A95` | `Mount_Durability_Current_RegenRate` | `f32` | `MountVitalChanged` | Current durability regeneration rate of a standard mount. |
| `0x10004939` | `MountEndurance_CurrentLevel` | `f32` | `MountVitalChanged` | Current War-Steed Endurance (morale). |
| `0x10004935` | `MountEndurance_MaxLevel` | `f32` | `MountVitalChanged` | Maximum War-Steed Endurance. |
| `0x1000493E` | `MountEndurance_Temporary` | `f32` | `MountVitalChanged` | Temporary War-Steed Endurance bubble. |
| `0x1000493B` | `MountEndurance_Temporary_Max` | `f32` | `MountVitalChanged` | Maximum temporary War-Steed Endurance. |
| `0x10004965` | `Vital_MountEndurancePercentage` | `f32` | `MountVitalChanged` | War-Steed Endurance percentage (`0.0..1.0`). |
| `0x1000494E` | `Vital_MountEnduranceCombatCurrentRegen` | `f32` | `MountVitalChanged` | In-combat War-Steed Endurance regen rate. |
| `0x1000494F` | `Vital_MountEndurancePeaceCurrentRegen` | `f32` | `MountVitalChanged` | Out-of-combat War-Steed Endurance regen rate. |
| `0x10004933` | `MountPower_CurrentLevel` | `f32` | `MountVitalChanged` | Current War-Steed Power. |
| `0x10004937` | `MountPower_MaxLevel` | `f32` | `MountVitalChanged` | Maximum War-Steed Power. |
| `0x10004938` | `MountPower_Temporary` | `f32` | `MountVitalChanged` | Temporary War-Steed Power bubble. |
| `0x10004932` | `MountPower_Temporary_Max` | `f32` | `MountVitalChanged` | Maximum temporary War-Steed Power. |
| `0x10004964` | `Vital_MountPowerPercentage` | `f32` | `MountVitalChanged` | War-Steed Power percentage (`0.0..1.0`). |
| `0x10004945` | `Vital_MountPowerCombatCurrentRegen` | `f32` | `MountVitalChanged` | In-combat War-Steed Power regen rate. |
| `0x1000494A` | `Vital_MountPowerPeaceCurrentRegen` | `f32` | `MountVitalChanged` | Out-of-combat War-Steed Power regen rate. |
| `0x100037C2` | `Pip_Momentum` | `i32` / `f32` | `MountVitalChanged` | War-Steed Fury / Momentum pip level. |
| `0x10004BEB` | `Pip_Momentum_MaximumAdditiveModifier` | `i32` / `f32` | `MountVitalChanged` | Additive modifier to maximum War-Steed Fury / Momentum. |
| `0x10004BE9` | `Mount_PipMaxModifier` | `i32` / `f32` | `MountVitalChanged` | Mount pip maximum modifier. |
| `0x10004BE8` | `Mount_MountGear` | `i32` | `MountVitalChanged` | Current War-Steed movement gear (`0` = Stop, `1` = Walk, `2` = Trot, `3` = Canter, `4` = Gallop). |
| `0x10004120` | `MountMovement_CurrentSpeed` | `f32` | `MountVitalChanged` | Current War-Steed movement speed (`m/s`). |
| `0x1000495F` | `Mount_Movement_Max_Speed` | `f32` | `MountVitalChanged` | War-Steed maximum speed (`m/s`). |
| `0x10004959` | `Mount_Movement_Acceleration` | `f32` | `MountVitalChanged` | War-Steed acceleration (`m/s^2`). |
| `0x10004969` | `Mount_Movement_Turn_Rate` | `f32` | `MountVitalChanged` | War-Steed turn rate (`rad/s` or `deg/s`). |
| `0x10004953` | `Stat_MountAgility` | `f32` | `MountVitalChanged` | War-Steed Agility stat. |
| `0x10004954` | `Stat_MountStrength` | `f32` | `MountVitalChanged` | War-Steed Strength stat. |
| `0x10004963` | `Combat_Agent_MountArmor_Value_Float` | `f32` | `MountVitalChanged` | War-Steed Armor value. |
| `0x1000497B` | `Mount_Level` | `i32` | `MountVitalChanged` | War-Steed level. |
| `0x100049AD` | `Mount_SteedXP` | `i64` / `i32` | `MountVitalChanged` | War-Steed experience points. |


---

## 8. P6 — Progression, Experience, Reputation, Titles, Trainers, Trait Trees, Professions, Recipes & Crafting <a id="p6-progression-crafting"></a>



### 8.1 Overview and Subsystem Interfaces

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

### 8.2 Experience, Leveling, Skill Pools, and Class Trainers (`0x04DF`)

#### 8.2.1 Experience and Level Properties (`ExperienceChanged`)

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

#### 8.2.2 Skill Pools and Skill Trainer Wire Messages (`0x84DFxxxx`)

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

### 8.3 Faction Reputation (`0x077B`)

#### 8.3.1 Faction Discovery and Action RPCs (`0x877Bxxxx`)

| Direction | Wire `function_id` | Stream Payload Fields (in wire order) | Description |
|---|---|---|---|
| S -> C | `0x877B0007` | `faction_did: u32` | Initializes faction `faction_did` (`Class 0x0761`) on the client and subscribes to the faction's dynamic player properties. |
| C -> S | `0x877B0008` | `action_id: u32`, `faction_did: u32` | Sends a faction action request for `faction_did`. |

#### 8.3.2 Dynamic Per-Faction Reputation Properties (`ReputationChanged`)

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

### 8.4 Titles (`0x052E`)

#### 8.4.1 Title Wire Messages (`0x852Exxxx`) and Properties

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

### 8.5 Traits, Virtues, Slotting, and Trait Trees (`0x0532`, `0x0E15`)

#### 8.5.1 Trait Slot Descriptor (`Class 0x05D1`)

When a `ClassRef(0x05D1)` instance is serialized into a `WLibArgFrame`, its body after the class-ref header consists of four little-endian `u32` fields (`OBS-gamelogic`):

| Offset (in `0x05D1` payload) | Size | Type | Field Name | Meaning |
|---|---|---|---|---|
| `+0x00` | 4 | `u32` | `tier` | Trait tier (`0` when `has_tier == 0`). |
| `+0x04` | 4 | `u32` | `has_tier` | Boolean flag (`0` or `1`) indicating whether `tier` is specified. |
| `+0x08` | 4 | `u32` | `index` | 0-based slot index within the trait nature category. |
| `+0x0C` | 4 | `u32` | `has_index` | Boolean flag (`0` or `1`) indicating whether `index` is specified (`1` when slotting a trait by index). |

#### 8.5.2 Trait Slotting and Virtue Wire Messages (`0x8532xxxx`)

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

#### 8.5.3 Point-Based Trait Trees and Configurations (`0x8E15xxxx`)

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

#### 8.5.4 Trait Point and Specialization Properties (`TraitPoints`, `TraitTreeChanged`)

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

### 8.6 Crafting, Professions, Vocations, Recipes, and Craft Trainers (`0x04EF`, `0x0528`)

#### 8.6.1 Craft Execution Descriptor (`Class 0x0A4A`)

When the player initiates crafting (`0x85280005`), the client serializes a `ClassRef(0x0A4A)` instance containing four fields in wire order (`OBS-gamelogic`, `BIN-client64`):

| Field Order | Type | Field Name | Meaning |
|---|---|---|---|
| 1 | `u32` | `multi_output_index` | `0` for single-output recipes; `1+` (1-based index) for multi-output recipes. |
| 2 | `ClassRef(Array<u32>)` | `optional_ingredients` | Array of `u32` boolean flags (`0` or `1`) corresponding 1-to-1 with the recipe's optional ingredient slots (e.g., critical chance journals/scrolls). |
| 3 | `u32` | `use_ingredient_pack` | `0` to consume normal inventory ingredients; `1` to consume universal/account ingredient packs. |
| 4 | `u32` | `tier` | Crafting tier identifier for the recipe being executed. |

*Note on active recipe and batch quantity:* The recipe being crafted is stored in the player property `Craft_CurrentRecipe` (`0x10001476`, `u32` DID). Multi-item batch crafting executes one craft induction cycle per `0x85280005` request, decrementing the client batch counter on each completion (`0x84EF002E`) and issuing the next `0x85280005` until the batch counter reaches zero or is interrupted (`0x84EF003C`).

#### 8.6.2 Crafting, Profession, and Trainer Wire Messages (`0x85280005`, `0x84EFxxxx`)

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

#### 8.6.3 Vocation, Profession, and Recipe Properties (`ProfessionChanged`, `RecipeForgotten`)

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

### 8.7 Synthetic Test Vectors

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

---

## 9. Mail System, Auction House & Player/Kinship Housing <a id="mail-auction-housing"></a>

### 9.1 Purpose & Scope <a id="spec-purpose"></a>

This specification defines the clean-room wire protocol for the Mail System, Auction House, and Player & Kinship Housing (including Housing Chest and Escrow storage) over the WLib RPC framing layer. All multi-byte integers and IEEE-754 floating-point values are **little-endian**.

- **Provenance tags used in this section:**
 - `[OBS]`: Observed directly in the read-only game data archives (`0x56000000` `wlib.bin` in `client_gamelogic.dat` and `0x34000000` Master Property Definition).
 - `[BIN]`: Confirmed from read-only static analysis of WState object serializers in the client executable.

### 9.2 WLib RPC Framing & Argument Encoding <a id="spec-framing"></a>

Every RPC invocation is carried inside a `NetBlob` payload using one of two WLib RPC envelopes `[OBS]`:

1. **Entity-Targeted RPC (`0x00D9:0x000D`):**
 - `u64 target_entity_iid`: 64-bit entity instance ID (`Mail` `0x0514`, `Auction` `0x04E5`, `PremiumHousing` `0x169F`, and `StorageLink` `0x0F5C`).
 - `u32 function_id`: `0x80000000 | (interface_class_id << 16) | method_index`.
 - `u32 arg_byte_length`: Byte length of the serialized argument payload.
 - `u8[arg_byte_length] arg_payload`: Arguments serialized sequentially with no per-argument type tags for primitive types.
2. **Broadcast / Service RPC (`0x00D9:0x000C`):**
 - `u32 function_id`: `0x80000000 | (interface_class_id << 16) | method_index` (`Housing` `0x07A7`, `BankStorage` `0x0A04`, and `ChestConfig` `0x0C20`).
 - `u32 arg_byte_length`: Byte length of the serialized argument payload.
 - `u8[arg_byte_length] arg_payload`: Arguments serialized sequentially.

When an argument is a **WState Object Reference** (`ref`), the wire stream encodes a `u32 class_id` (`0x00000000` for a null reference) followed immediately by the class-specific binary payload `[BIN]`.

---

### 9.3 Mail System Protocol (`0x0514`) <a id="spec-mail"></a>

#### 9.3.1 Mail RPCs (`0x0514`, Entity-Targeted) `[OBS]`

| `function_id` | Dir | Target Entity | Wire Arguments (Sequential Little-Endian Layout) | Semantics |
| --- | --- | --- | --- | --- |
| `0x85140074` | C→S | Mailbox IID | `u64 entity_iid` | Interact with world mailbox object (`Mail_ContainerID` `0x10000674`). |
| `0x85140048` | C→S | Avatar IID | `u32 is_cod_flag`, `ref<0x0068> attached_item_iids` (`Array<u64>`, max 10), `u32 cod_copper`, `u32 enclosed_copper`, `ref<0x00C7> body`, `ref<0x00C7> subject`, `ref<0x00C7> recipient_name`, `u64 mailbox_iid` | Send mail message. Subject and body are limited to `1024` characters each. |
| `0x85140040` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Open/read mail letter `mail_id`; requests full message body. |
| `0x8514003B` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Detach all attachments (items and coin; pays COD if `Mail_IsCOD`). |
| `0x8514006F` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Detach item attachment(s) only. |
| `0x85140092` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Detach currency attachment only. |
| `0x85140015` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Return mail letter `mail_id` to sender. |
| `0x85140012` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Delete mail letter `mail_id` (requires no remaining attachments). |
| `0x85140025` | S→C | Avatar IID | `u64 mailbox_iid` | Open Mailbox UI panel. |
| `0x85140035` | S→C | Avatar IID | *(0 bytes)* | Close Mailbox UI panel. |
| `0x85140088` | S→C | Avatar IID | `ref<0x00B6> inbox_table` (`HashTable<u64, MailLetter(0x01B5)>`) | Full inbox header table; sets `Mail_Count` (`0x100002B0`). |
| `0x8514007F` | S→C | Avatar IID | `ref<0x01B5> mail_letter` | Single `MailLetter` added or updated (includes body after `0x85140040`). |
| `0x85140026` | S→C | Avatar IID | `u64 detached_item_or_flag`, `u64 mail_id` | Attachment/currency detached from `mail_id`. |
| `0x85140036` | S→C | Avatar IID | `u64 mail_id` | Unlock `mail_id` after failed/cancelled operation. |
| `0x85140081` | S→C | Avatar IID | `u64 mail_id` | Remove `mail_id` from inbox; decrements `Mail_Count` (`0x100002B0`). |
| `0x85140083` | S→C | Avatar IID | *(0 bytes)* | Mail send succeeded. |
| `0x85140086` | S→C | Avatar IID | `u32 error_pid` | Mail operation failed with error `PropertyID`. |

#### 9.3.2 WState `0x01B5` (`MailLetter`) Wire Layout `[BIN, OBS]`

| Offset | Size | Type | Field | Description |
| --- | --- | --- | --- | --- |
| `+0x00` | `4` | `u32` | `flags` | Bitfield of boolean letter flags `[BIN]`:<br>- `0x00000001` (bit 0): `HasBeenOpened` / `IsRead` (`0x01B5:0x001C` get, `0x01B5:0x0019` set)<br>- `0x00000002` (bit 1): `IsReturned` (`0x01B5:0x001D` get, `0x01B5:0x001A` set)<br>- `0x00000004` (bit 2): `IsCOD` (`0x01B5:0x0021` get, `0x01B5:0x0020` set)<br>- `0x00000008` (bit 3): `IsAdmin` / `IsSystemMail` (`0x01B5:0x0024` get, `0x01B5:0x0025` set)<br>- `0x00000010` (bit 4): `IsSpam` (`0x01B5:0x0032` get, `0x01B5:0x0033` set)<br>- `0x00000020` (bit 5): `IsFaux` / `IsStoreMail` (`0x01B5:0x0034` get, `0x01B5:0x0035` set)<br>- `0x00000040` (bit 6): `CannotReturn` / `NoReturn` (`0x01B5:0x003A` get, `0x01B5:0x0037` set; `0x01B5:0x001F` `IsReturnable` evaluates `(flags & 0x40) == 0 && Mail_SenderIID != 0`) |
| `+0x04` | var | `PropertySet` (`0x00A6`) | `properties` | Keyed property map containing `0x100026F7` (`Mail_MailIID`, `u64`), `0x100003A6` (`Mail_ID`, `u64`), `0x10001233` (`Mail_SenderIID`, `u64`), `0x10000F90` (`Mail_SenderName`, `StringInfo`), `0x10000925` (`Mail_RecipientIID`, `u64`), `0x1000115A` (`Mail_RecipientName`, `StringInfo`), `0x10000926` (`Mail_Subject`, `StringInfo`), `0x10000E8A` (`Mail_Message`, `StringInfo`), `0x10001532` (`Mail_MailTimestamp`, `f64`), `0x1000057A` (`Mail_CurrencyAttachment`, `u32`), `0x10000673` (`Mail_CurrencyOnDelivery`, `u32`), `0x10000AD7` (`Mail_ItemIID`, `u64`), `0x100010EF` (`UI_Examination_Tooltip_DID`, `Struct`), and `0x10005E77` (`Mail_AdditionalItemAttachmentArray`, `Array` of `0x10005E76`) `[BIN, OBS]`. |

---

### 9.4 Auction House Protocol (`0x04E5`) <a id="spec-auction"></a>

#### 9.4.1 Auction House RPCs (`0x04E5`, Entity-Targeted on Avatar IID) `[OBS]`

| `function_id` | Dir | Wire Arguments (Sequential Little-Endian Layout) | Semantics |
| --- | --- | --- | --- |
| `0x84E50018` | C→S | `u64 auctioneer_iid`, `ref<0x0189> auction_adaptor` | Post a new auction (`Auction_Duration` `0x100003A3` in `{14400, 28800, 86400, 172800}` seconds). |
| `0x84E5001A` | C→S | `u64 auctioneer_iid`, `u32 bid_or_buyout_amount`, `ref<0x0189> auction_adaptor` | Place a bid or buyout (`bid_or_buyout_amount >= Auction_BuyoutPrice`). |
| `0x84E50015` | C→S | `u64 auctioneer_iid`, `u64 auction_id` | Cancel own posted auction `auction_id`. |
| `0x84E50017` | C→S | `u64 auctioneer_iid`, `u64 item_iid`, `u64 auction_id` | Request item examination tooltip (`0x10000421`) for `auction_id`. |
| `0x84E50026` | C→S | `u32 ah_currency_type` (`1`=Coin, `2`=Mithril), `u64 auctioneer_iid`, `u32 page_number`, `u32 sort_descending_flag`, `ref<0x0188> auction_query` | Execute Auction House search query. |
| `0x84E50003` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Request own posted auctions list. |
| `0x84E5001D` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Request own active bids list. |
| `0x84E50035` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Refresh own posted auctions when dirty flag (`0x10001720` / `0x1000508C`) is set. |
| `0x84E50036` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Refresh own active bids when dirty flag (`0x10001721` / `0x1000508B`) is set. |
| `0x84E50049` | C→S | `u32 close_or_mode_flag` | Notify server of Auction House UI close / mode change. |
| `0x84E5000F` | S→C | `u32 ah_currency_type`, `u64 auctioneer_iid` | Open Auction House UI panel. |
| `0x84E5002C` | S→C | *(0 bytes)* | Close Auction House UI panel. |
| `0x84E5001E` | S→C | `u32 ah_currency_type`, `u32 current_page`, `u32 total_pages_or_count`, `ref<0x00B6> results_table` (`HashTable<u64, 0x0189>`) | Search query results page. |
| `0x84E5000B` | S→C | `u32 ah_currency_type`, `ref<0x00B6> posted_table` (`HashTable<u64, 0x0189>`) | Own posted auctions list. |
| `0x84E50014` | S→C | `u32 ah_currency_type`, `ref<0x00B6> bids_table` (`HashTable<u64, 0x0189>`) | Own active bids list. |
| `0x84E50021` | S→C | `u32 status_or_error_pid` | Auction status or error notification `PropertyID`. |
| `0x84E50024` | S→C | `ref<0x0189> auction_adaptor` | Single auction listing added or updated. |
| `0x84E50027` | S→C | `ref<0x0189> auction_adaptor` | Single auction listing completed or removed. |
| `0x84E50043` | S→C | `u64 auction_id` | Auction listing removed by ID. |

#### 9.4.2 WState `0x0188` (`AuctionQuery`) & `0x0189` (`AuctionAdaptor`) Wire Layouts `[BIN, OBS]`

**WState `0x0188` (`AuctionQuery`):**

| Offset | Size | Type | Field | Description |
| --- | --- | --- | --- | --- |
| `+0x00` | `4` | `u32` | `sort_column_pid` | Sort property ID (`0x100026A5`, `0x100026CD`, `0x100026CE`, `0x100041CD`, `0x100041CB`, `0x100003A2`, `0x10001CB2`) `[BIN, OBS]`. |
| `+0x04` | `4` | `u32` | `array_header_flags` | Standard WState array capacity/flags word `[BIN]`. |
| `+0x08` | `4` | `u32` | `filter_count` | Number of filter `Property` elements (`<= 0x3FFFFFFF`) `[BIN]`. |
| `+0x0C` | var | `Property[filter_count]` | `filters` | Sequential `Property` (`0x0027`) elements (`0x10001533` `Auction_CategoryType`, `0x10000D95` `Auction_LevelRange`, `0x10000F8B` `Auction_StringSearch`, `0x10001593` `Auction_KinshipSearch`, `0x10001BB6` `Auction_SearchableProperty_Set`, `0x10002684` `Auction_IASearch`) `[BIN, OBS]`. |

**WState `0x0189` (`AuctionAdaptor`):**

| Offset | Size | Type | Field | Description |
| --- | --- | --- | --- | --- |
| `+0x00` | `8` | `u64` | `item_iid` | 64-bit instance ID of the auctioned item `[BIN]`. |
| `+0x08` | var | `PropertySet` (`0x00A6`) | `auction_properties` | Contains `0x10000002` (`Auction_ID`, `u64`), `0x1000507C` (`Auction_AuctionType`, `u32`), `0x10000AD0` (`Auction_Poster`, `u64`), `0x10000A07` (`Auction_PosterName`, `StringInfo`), `0x10000BAA` (`Auction_KinshipID`, `u64`), `0x1000074C` (`Auction_KinshipName`, `StringInfo`), `0x10000AD1` (`Auction_InitialPrice`, `u32`), `0x10000E84` (`Auction_BuyoutPrice`, `u32`), `0x10000D96` (`Auction_CurrentBid`, `u32`), `0x10000F8C` (`Auction_CurrentBidder`, `u64`), `0x100002AC` (`Auction_AuctionBids`, `u32`), `0x100003A3` (`Auction_Duration`, `u32`), `0x100003A2` (`Auction_ItemLevel`, `u32`), `0x100041CC` (`Auction_ItemQuantity`, `u32`), `0x10000D94` (`Auction_TimeReceived`, `f64`) `[BIN, OBS]`. |
| `var` | var | `Property` (`0x0027`) | `tooltip_property` | Serialized `0x10000421` (`UI_Examination_Tooltip_DID`) property for item inspection `[BIN, OBS]`. |

---

### 9.5 Player & Kinship Housing Protocol (`0x07A7`, `0x169F`, `0x0A04`) <a id="spec-housing"></a>

#### 9.5.1 Packed 64-Bit `house_id` Invariant `[OBS]`

Every `u64 house_id` transmitted in `0x07A7` RPC arguments or WState payloads (`0x07A8`, `0x0801`) satisfies:
- `neighborhood_did = u32(house_id >> 32)` (`0x10001A49` `HouseKeeper_NeighborhoodDID`)
- `house_did = u32(house_id & 0xFFFFFFFF)` (`0x1000190A` `HouseKeeper_HouseDID`)

#### 9.5.2 Summary of Housing & Storage RPCs `[OBS]`

- **Purchase / Abandon / Directory (`0x07A7` Broadcast):**
 - Purchase house: `0x87A7000B` (`u32 alt_currency_flag, u32 house_did, u32 neighborhood_did, u32 template_did, u32 vendor_did, u64 vendor_iid`) -> S→C `0x87A7002A` / `0x87A700AF`.
 - Abandon house: `0x87A70022` (`u64 house_id`) -> S→C `0x87A700AF` (`u64 status_or_owner_iid, u64 house_id`).
 - Query own/kinship houses: `0x87A7001E` (`0 bytes`).
 - Browse neighborhoods/houses: `0x87A70008` (`u32 template_did`) -> `0x87A70012` (`ref<0x07B1>, u32 status`); `0x87A7003B` (`u32 neighborhood_did`) -> `0x87A700B9` (`ref<0x07A9>, u32 status`); `0x87A70040` (`u32 neighborhood_did`) -> `0x87A700C2` (`ref<0x0803>, u32 status`).
- **Upkeep / Maintenance (`0x07A7` Broadcast):**
 - Query upkeep: `0x87A70019` (`u32 house_did, u32 neighborhood_did, u64 housekeeper_iid`) -> S→C `0x87A70062` (`ref<0x07C6>, u32 house_did, u32 neighborhood_did`).
 - Pay upkeep (coin): `0x87A7002D` (`u32 upkeep_amount, u64 house_id`) -> S→C `0x87A70051` (`u32 status, u32 house_did, u32 neighborhood_did`).
 - Pay upkeep (Mithril): `0x87A701DE` (`u32 periods, u64 house_id`) -> S→C `0x87A701E4` (`u32 status`).
- **Permissions (`0x07A7` Broadcast):**
 - Query permissions: `0x87A70006` (`u32 house_did, u32 neighborhood_did, u64 housekeeper_iid`) -> S→C `0x87A70042` (`ref<0x07BC>, u32 house_did, u32 neighborhood_did`).
 - Update group permission bitmask: `0x87A70016` (`u32 bitmask, u32 group_index, u32 house_did, u64 housekeeper_iid`) -> S→C `0x87A70049`.
 - Add/modify/remove individual guest: `0x87A70071` (add by IID), `0x87A70077` (add by `StringInfo` name), `0x87A70116` (modify bitmask), `0x87A700A5` (remove by IID) -> S→C `0x87A7004E` (`u32 status, u64 target_char_iid`).
- **Decoration Hooks (`0x07A7` Broadcast):**
 - Toggle decoration mode: `0x87A701B1` (`u32 enable_flag, u64 housekeeper_iid`).
 - Place item in hook: `0x87A701B5` (`u32 hook_id, u64 item_iid, u64 housekeeper_iid`) -> S→C `0x87A701AE` (`u32 hook_id, u64 item_iid, u32 status`).
 - Remove item from hook: `0x87A701B7` (`u32 hook_id, u64 housekeeper_iid`) -> S→C `0x87A701AE`.
 - Rotate item in hook: `0x87A701BF` (`f32 heading_degrees, u32 hook_id, u64 housekeeper_iid`) -> S→C `0x87A701BB` (`u32 hook_id, f32 heading_degrees, u32 status`).
 - Translate/offset item in hook: `0x87A701F4` (`ref<0x00D3> offset_vector, u64 housekeeper_iid, u32 hook_id`).
- **Housing Chest & Escrow Storage (`0x0A04` Broadcast):**
 - Open chest/escrow: `0x8A040004` (`u64 chest_or_npc_iid, ref<0x0E9C> bank_address`) -> S→C `0x8A04000C` (`ref<0x0E9C>, ref<Array<0x0A07>>, ref<Array<0x0C1F>>, u32 max_capacity, u32 used_capacity, u64 chest_iid`).
 - Deposit item: `0x8A040014` (`u32 quantity, u32 dest_chest_type, u64 item_iid, ref<0x0E9C> bank_address`) -> S→C `0x8A04001B`.
 - Withdraw item (chest or escrow): `0x8A040017` (`u32 quantity, u64 dest_container_iid, u64 item_iid, ref<0x0E9C> bank_address`) -> S→C `0x8A040020`.
 - Move item between chest tabs: `0x8A040018` (`u32 dest_chest_type, u64 item_iid, ref<0x0E9C> bank_address`).
 - Close chest/escrow: `0x8A040007` (`ref<0x0E9C> bank_address`).

---

### 9.6 Synthetic Test Vectors <a id="spec-test-vectors"></a>

#### 9.6.1 Mail Open Letter Request (`0x85140040`, C→S)
- **Scenario:** Player Avatar `0x00000001000000AA` opens Mail Letter `0x00000002000000BB` at Mailbox `0x00000003000000CC`.
- **Wire Bytes (Entity-Targeted `0x00D9:0x000D` payload):**
 - `target_entity_iid` (`u64` LE): `AA 00 00 00 01 00 00 00`
 - `function_id` (`u32` LE): `40 00 14 85` (`0x85140040`)
 - `arg_byte_length` (`u32` LE): `10 00 00 00` (`16` bytes)
 - `mail_id` (`u64` LE): `BB 00 00 00 02 00 00 00`
 - `mailbox_iid` (`u64` LE): `CC 00 00 00 03 00 00 00`

#### 9.6.2 Housing Decoration Rotate Request (`0x87A701BF`, C→S)
- **Scenario:** Player rotates decoration in Hook ID `12` (`0x0000000C`) to `90.0` degrees (`0x42B40000`) on HouseKeeper `0x00000004000000DD`.
- **Wire Bytes (Broadcast `0x00D9:0x000C` payload):**
 - `function_id` (`u32` LE): `BF 01 A7 87` (`0x87A701BF`)
 - `arg_byte_length` (`u32` LE): `10 00 00 00` (`16` bytes)
 - `heading_degrees` (`f32` LE): `00 00 B4 42` (`90.0f`)
 - `hook_id` (`u32` LE): `0C 00 00 00` (`12`)
 - `housekeeper_iid` (`u64` LE): `DD 00 00 00 04 00 00 00`

---

---

### 9.7 Client-to-Server (C→S) Housing RPCs (`0x07A7` & `0x169F`) <a id="dirty-housing-c2s"></a>

| `function_id` | Stub / Pack | Caller (`0x07A6` / `0x16A0`) | `WLibArgFrame` Wire Arguments (in serialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x87A70002` | `v2` (`fnum 2`) / `v3` (`0x87A70003`) | `0x07A6:v139` | 1. `u32 house_did` (`0x008E:0x0002`)<br>2. `u32 neighborhood_did` (`0x008E:0x0002`)<br>3. `u64 gate_iid` (`0x008E:0x0003`) | Enter / visit a neighborhood or specific house via neighborhood gate (`gate_iid`). |
| `0x87A7000B` | `v10` (`fnum 11`) / `v9` (`0x87A70033`) | `0x07A6:v140` | 1. `u32 writ_or_alt_currency_flag` (`0x008E:0x0002`)<br>2. `u32 house_did` (`0x008E:0x0002`)<br>3. `u32 neighborhood_did` (`0x008E:0x0002`)<br>4. `u32 template_did` (`0x008E:0x0002`)<br>5. `u32 vendor_did` (`0x008E:0x0002`)<br>6. `u64 vendor_iid` (`0x008E:0x0003`) | Purchase a house with coin or currency (`0x07A6:v140`; Note: when purchasing via Mithril/Writ paper item `fp-6 != 0`, client also invokes PaperItemService `0x0F44:0x0003` service `29` `0x001D` with `0x10005E36`, `0x10005E34`, `0x10005E35`, `0x10001A49`). |
| `0x87A7000A` | `v8` (`fnum 10`) / `v11` (`0x87A70034`) | `0x07A6:v141` | 1. `u32 action_flag` (`0x008E:0x0002`)<br>2. `u32 house_did` (`0x008E:0x0002`)<br>3. `u32 neighborhood_did` (`0x008E:0x0002`)<br>4. `u64 vendor_iid` (`0x008E:0x0003`) | Secondary house purchase / transfer action via vendor. |
| `0x87A70008` | `v6` (`fnum 8`) / `v7` (`0x87A70030`) | `0x07A6:v143` | `u32 template_did` (`0x008E:0x0002`) | Request neighborhood directory list (`0x07B1`) for homestead `template_did`. |
| `0x87A7003B` | `v37` (`fnum 59`) / `v40` (`0x87A700BD`) | `0x07A6:v180` | `u32 neighborhood_did` (`0x008E:0x0002`) | Request list of houses (`0x07A9`) in `neighborhood_did`. |
| `0x87A7003C` | `v34` (`fnum 60`) / `v41` (`0x87A700BE`) | `0x07A6:v181` | `u32 template_did` (`0x008E:0x0002`) | Request available houses for sale in homestead `template_did`. |
| `0x87A70040` | `v43` (`fnum 64`) / `v44` (`0x87A70103`) | `0x07A6:v185` | `u32 neighborhood_did` (`0x008E:0x0002`) | Request detailed house purchase listings (`0x0803`) for `neighborhood_did`. |
| `0x87A7001E` | `v30` (`fnum 30`) / `v31` (`0x87A7008D`) | `0x07A6:v174` | *(empty — 0 bytes)* | Request player's own personal/kinship house information. |
| `0x87A70022` | `v32` (`fnum 34`) / `v33` (`0x87A70091`) | `0x07A6:v176` | `u64 house_id` (`0x008E:0x0003`) | Abandon / relinquish owned house (`house_id`). |
| `0x87A70006` | `v14` (`fnum 6`) / `v15` (`0x87A70046`) | `0x07A6:v147` | 1. `u32 house_did` (`0x008E:0x0002`)<br>2. `u32 neighborhood_did` (`0x008E:0x0002`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Request housing permissions (`0x07BC`) for a house. |
| `0x87A70019` | `v23` (`fnum 25`) / `v25` (`0x87A70069`) | `0x07A6:v156` | 1. `u32 house_did` (`0x008E:0x0002`)<br>2. `u32 neighborhood_did` (`0x008E:0x0002`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Request housing upkeep/maintenance info (`0x07C6`) for a house. |
| `0x87A7002D` | `v26` (`fnum 45`) / `v29` (`0x87A7007E`) | `0x07A6:v166` | 1. `u32 upkeep_amount_or_credits` (`0x008E:0x0001`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Pay house upkeep/maintenance with standard coin. |
| `0x87A701DE` | `v59` (`fnum 478`) / `v60` (`0x87A701DF`) | `0x07A6:v229` | 1. `u32 upkeep_periods` (`0x008E:0x0001`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Pay house upkeep/maintenance with Mithril Coins. |
| `0x87A70016` | `v18` (`fnum 22`) / `v20` (`0x87A70054`) | `0x07A6:v153` | 1. `u32 permission_bitmask` (`0x008E:0x0001`)<br>2. `u32 permission_group_index` (`0x008E:0x0001`)<br>3. `u32 house_did` (`0x008E:0x0002`)<br>4. `u64 housekeeper_iid` (`0x008E:0x0003`) | Update housing permission bitmask for a permission group/class (`0x100019B0` `HousingSystem_Permissions`). |
| `0x87A7003A` | `v21` (`fnum 58`) / `v38` (`0x87A700B4`) | `0x07A6:v180` | `u64 target_char_iid` (`0x008E:0x0003`) | Request character display name resolution for a permission entry IID (`0x87A70177`). |
| `0x87A70071` | `v22` (`fnum 113`) / `v27` (`0x87A70072`) | `0x07A6:v162` | 1. `u32 permission_bitmask` (`0x008E:0x0001`)<br>2. `u64 target_char_iid` (`0x008E:0x0003`)<br>3. `u64 house_id` (`0x008E:0x0003`) | Add an individual character (`target_char_iid`) to the house guest permission list. |
| `0x87A70077` | `v47` (`fnum 119`) / `v28` (`0x87A70078`) | `0x07A6:v205` | 1. `ref target_char_name` (`0x008E:0x0004`, WState `0x00C7` `StringInfo`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Add an individual character by display name to the house guest permission list. |
| `0x87A700A5` | `v49` (`fnum 165`) / `v35` (`0x87A700A6`) | `0x07A6:v206` | 1. `u64 target_char_iid` (`0x008E:0x0003`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Remove an individual character (`target_char_iid`) from the house guest permission list. |
| `0x87A70116` | `v57` (`fnum 278`) / `v45` (`0x87A70117`) | `0x07A6:v203` | 1. `u32 permission_bitmask` (`0x008E:0x0001`)<br>2. `u64 target_char_iid` (`0x008E:0x0003`)<br>3. `u64 house_id` (`0x008E:0x0003`) | Update permission bitmask for an existing guest character (`target_char_iid`). |
| `0x87A70138` | `v58` (`fnum 312`) / `v46` (`0x87A70139`) | `0x07A6:v210` | 1. `u32 panel_type_flag` (`0x008E:0x0001`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Notify server of closing housing permissions/upkeep UI panel. |
| `0x87A701B1` | `v61` (`fnum 433`) / `v52` (`0x87A701B2`) | `0x07A6:v216` | 1. `u32 enable_flag` (`0x008E:0x0001`)<br>2. `u64 housekeeper_iid` (`0x008E:0x0003`) | Toggle Housing Decoration Mode (`HousingSystem_PlayerInDecorationMode` `0x100019B2`). |
| `0x87A701B5` | `v63` (`fnum 437`) / `v53` (`0x87A701B6`) | `0x07A6:v218` | 1. `u32 hook_id` (`0x008E:0x0002`)<br>2. `u64 item_iid` (`0x008E:0x0003`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Place / slot a decoration item (`item_iid`) into decoration hook `hook_id` (`HousingHook_LandmarkHookID` `0x100019FD`). |
| `0x87A701B7` | `v64` (`fnum 439`) / `v54` (`0x87A701B8`) | `0x07A6:v219` | 1. `u32 hook_id` (`0x008E:0x0002`)<br>2. `u64 housekeeper_iid` (`0x008E:0x0003`) | Remove / unslot the decoration item from decoration hook `hook_id`. |
| `0x87A701BF` | `v66` (`fnum 447`) / `v56` (`0x87A701C0`) | `0x07A6:v221` | 1. `f32 heading_degrees` (`0x008E:0x0005`)<br>2. `u32 hook_id` (`0x008E:0x0002`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Rotate the decoration item in `hook_id` to `heading_degrees`. |
| `0x87A701F4` | `v68` (`fnum 500`) / `v67` (`0x87A701F5`) | `0x07A6:v240` | 1. `ref offset_vector` (`0x008E:0x0004`, WState `0x00D3` `Vector` + rotation)<br>2. `u64 housekeeper_iid` (`0x008E:0x0003`)<br>3. `u32 hook_id` (`0x008E:0x0002`) | Adjust 3D translation offset and rotation of the decoration in `hook_id` (`HousingHook_TranslationOffsetVector` `0x10005A05`). |
| `0x87A701F0` | `v69` (`fnum 496`) / `v65` (`0x87A701F1`) | `0x07A6:v233` | `u64 housekeeper_iid` (`0x008E:0x0003`) | Reset / commit decoration hook state on `housekeeper_iid`. |
| `0x87A70210` | `v70` (`fnum 528`) / `v72` (`0x87A70211`) | `0x07A6:v249` | `u64 housekeeper_iid` (`0x008E:0x0003`) | Reset ambient house music / dayfile hook 1 (`0x07A6:v249`). |
| `0x87A70209` | `v71` (`fnum 521`) / `v73` (`0x87A70212`) | `0x07A6:v248` | `u64 housekeeper_iid` (`0x008E:0x0003`) | Reset ambient house music / dayfile hook 2 (`0x07A6:v248`). |
| `0x969F000C` | `v1` (`fnum 12`) / `v2` (`0x969F000D`) | `0x16A0:v1` | `u32 query_flag` (`0x008E:0x0002`) | Query primary residence / premium housing status (targeted at Player Avatar IID). |
| `0x969F0018` | `v4` (`fnum 24`) / `v5` (`0x969F0019`) | `0x16A0:v2` | 1. `u64 target_house_address` (`0x008E:0x0003`)<br>2. `u64 secondary_id` (`0x008E:0x0003`) | Set primary residence / premium house target address (`0x100068AD` `HousingSystem_PrimaryResidenceTargetAddress`). |
| `0x969F004C` | `v9` (`fnum 76`) / `v10` (`0x969F004D`) | `0x16A0:v16` | 1. `u32 arg0` (`0x008E:0x0002`)<br>2. `u32 arg1` (`0x008E:0x0002`) | Secondary premium housing action. |

### 9.8 Server-to-Client (S→C) Housing RPCs (`0x07A7` & `0x169F`) <a id="dirty-housing-s2c"></a>

| `function_id` | Stub | Handler Chain (`0x07A6` / `0x16A0`) | `WLibArgFrame` Wire Arguments (in deserialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x87A70001` | `v1` (`fnum 1`) | `0x07A6:v138` -> `0x0143:0x0011` | 1. `u32 template_did` (`0x008E:0x0007`)<br>2. `u64 gate_iid` (`0x008E:0x0008`) | Open Neighborhood Gate UI (`0x0143:0x0011`). |
| `0x87A70007` | `v4` (`fnum 7`) | `0x07A6:v142` -> `0x0143:0x0012` | 1. `u32 template_did` (`0x008E:0x0007`)<br>2. `u64 vendor_iid` (`0x008E:0x0008`) | Open House Broker / Purchase Vendor UI (`0x0143:0x0012`). |
| `0x87A70012` | `v5` (`fnum 18`) | `0x07A6:v144` -> `0x0143:0x0013` | 1. `ref neighborhood_list` (`0x008E:0x0009`, WState `0x07B1`)<br>2. `u32 status` (`0x008E:0x0006`) | Deliver neighborhood directory list (`0x07B1` containing `Array<0x07AF>`). |
| `0x87A7002A` | `v12` (`fnum 42`) | `0x07A6:v145` -> `0x0143:0x0014` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u32 house_or_neighborhood_did` (`0x008E:0x0007`) | House purchase / gate entry status response. |
| `0x87A70042` | `v13` (`fnum 66`) | `0x07A6:v148` -> `0x0143:0x0015` | 1. `ref permissions` (`0x008E:0x0009`, WState `0x07BC` `HousingPermissions`)<br>2. `u32 house_did` (`0x008E:0x0007`)<br>3. `u32 neighborhood_did` (`0x008E:0x0007`) | Deliver housing permissions (`0x07BC`). |
| `0x87A70049` | `v16` (`fnum 73`) | `0x07A6:v150` -> `0x0143:0x0016` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u32 arg1` (`0x008E:0x0006`) | Group permission update status response. |
| `0x87A7004E` | `v17` (`fnum 78`) | `0x07A6:v151` -> `0x0143:0x0017` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u64 target_char_iid` (`0x008E:0x0008`) | Individual guest permission update status response. |
| `0x87A70051` | `v19` (`fnum 81`) | `0x07A6:v154` -> `0x0143:0x0018` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u32 house_did` (`0x008E:0x0007`)<br>3. `u32 neighborhood_did` (`0x008E:0x0007`) | Upkeep payment / permission status response. |
| `0x87A70062` | `v24` (`fnum 98`) | `0x07A6:v157` -> `0x0143:0x001A` | 1. `ref upkeep_info` (`0x008E:0x0009`, WState `0x07C6` `HousingUpkeepInfo`)<br>2. `u32 house_did` (`0x008E:0x0007`)<br>3. `u32 neighborhood_did` (`0x008E:0x0007`) | Deliver house upkeep/maintenance info (`0x07C6`). |
| `0x87A700AF` | `v36` (`fnum 175`) | `0x07A6:v179` -> `0x0143:0x001F` | 1. `u64 status_or_owner_iid` (`0x008E:0x0008`)<br>2. `u64 house_id` (`0x008E:0x0008`) | House ownership / abandon confirmation. |
| `0x87A700B9` | `v39` (`fnum 185`) | `0x07A6:v183` -> `0x0143:0x0020` | 1. `ref house_list` (`0x008E:0x0009`, WState `0x07A9`)<br>2. `u32 status` (`0x008E:0x0006`) | Deliver list of houses in a neighborhood (`0x07A9` containing `Array<0x07A8>`). |
| `0x87A700C2` | `v42` (`fnum 194`) | `0x07A6:v186` -> `0x0143:0x0022` | 1. `ref purchase_list` (`0x008E:0x0009`, WState `0x0803`)<br>2. `u32 status` (`0x008E:0x0006`) | Deliver available houses for sale (`0x0803` containing `Array<0x0801>`). |
| `0x87A70174` | `v48` (`fnum 372`) | `0x07A6:v212` -> `0x0143:0x0019` | 1. `u32 arg0` (`0x008E:0x0006`)<br>2. `u32 arg1` (`0x008E:0x0006`) | Housing decoration/upkeep callback. |
| `0x87A70177` | `v50` (`fnum 375`) | `0x07A6:v214` -> `0x0143:0x0021` | 1. `u64 char_iid` (`0x008E:0x0008`)<br>2. `ref char_name` (`0x008E:0x0009`, WState `0x00C7` `StringInfo`) | Deliver resolved character display name for housing permission list. |
| `0x87A701AE` | `v51` (`fnum 430`) | `0x07A6:v215` -> `0x0143:0x001C` | 1. `u32 hook_id` (`0x008E:0x0007`)<br>2. `u64 item_iid` (`0x008E:0x0008`)<br>3. `u32 status` (`0x008E:0x0006`) | Decoration hook slot/unslot result. |
| `0x87A701BB` | `v55` (`fnum 443`) | `0x07A6:v220` -> `0x0143:0x001D` | 1. `u32 hook_id` (`0x008E:0x0007`)<br>2. `f32 heading_degrees` (`0x008E:0x000A`)<br>3. `u32 status` (`0x008E:0x0006`) | Decoration hook rotation result. |
| `0x87A701E4` | `v62` (`fnum 484`) | `0x07A6:v230` -> `0x0131:0x0001` | `u32 mithril_upkeep_status` (`0x008E:0x0006`) | Mithril Coin upkeep payment status response. |
| `0x969F0016` | `v3` (`fnum 22`) | `0x16A0:v6` | `u64 house_address` (`0x008E:0x0008`) | Primary residence / premium house address update. |
| `0x969F001E` | `v6` (`fnum 30`) | `0x16A0:v13` | `u32 status` (`0x008E:0x0006`) | Primary residence status callback 1. |
| `0x969F0020` | `v7` (`fnum 32`) | `0x16A0:v12` | `u32 status` (`0x008E:0x0006`) | Primary residence status callback 2. |
| `0x969F0023` | `v8` (`fnum 35`) | `0x16A0:v11` | `u32 status` (`0x008E:0x0006`) | Primary residence status callback 3. |

### 9.9 Housing Chest, Vault & Escrow Storage RPCs (`0x0A04`, `0x0C20`, `0x0F5C`) <a id="dirty-housing-storage"></a>

Housing Chests and Escrow brokers share the unified Bank/Vault RPC protocol (`0x0A04`, `2564`, implemented by `0x0A02` on `0x06C3`), discriminated by WState class `0x0E9C` (`3740`, `BankAddress`, 16 bytes: `u64 owner_or_house_iid`, `u32 bank_repository_type`, `u32 chest_type`).

| `function_id` | Direction | Transport | `WLibArgFrame` Wire Arguments (in serialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x8A040004` | C→S (`v1`, pack `v3`/`0x8A040008`) | Broadcast (`0x00D9:0x000C`) | 1. `u64 chest_or_npc_iid` (`0x008E:0x0003`)<br>2. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C` `BankAddress`) | Request to open a Housing Chest, Escrow, Kinship Vault, or Personal Vault container (`0x0A02:v131`). |
| `0x8A040007` | C→S (`v2`, pack `v4`/`0x8A040009`) | Broadcast (`0x00D9:0x000C`) | `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Close the open storage container (`0x0A02:v132`). |
| `0x8A040014` | C→S (`v7`, pack `v10`/`0x8A040019`) | Broadcast (`0x00D9:0x000C`) | 1. `u32 quantity` (`0x008E:0x0001`)<br>2. `u32 dest_chest_type` (`0x008E:0x0002`, `Enum_ChestType` `0x230001CD`)<br>3. `u64 item_iid` (`0x008E:0x0003`)<br>4. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Deposit item (`item_iid`) into storage chest/vault (`0x0A02:v135`). |
| `0x8A040017` | C→S (`v8`, pack `v11`/`0x8A04001A`) | Broadcast (`0x00D9:0x000C`) | 1. `u32 quantity` (`0x008E:0x0001`)<br>2. `u64 dest_container_iid` (`0x008E:0x0003`, e.g. player backpack IID)<br>3. `u64 item_iid` (`0x008E:0x0003`)<br>4. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Withdraw item (`item_iid`) from Housing Chest, Escrow, or Vault into `dest_container_iid` (`0x0A02:v136`). |
| `0x8A040018` | C→S (`v9`, pack `v12`/`0x8A040021`) | Broadcast (`0x00D9:0x000C`) | 1. `u32 dest_chest_type` (`0x008E:0x0002`)<br>2. `u64 item_iid` (`0x008E:0x0003`)<br>3. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Move item (`item_iid`) to another chest tab (`dest_chest_type`) within `bank_address` (`0x0A02:v137`). |
| `0x8A04000C` | S→C (`v5`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `ref items_array` (`0x008E:0x0009`, WState `0x0022` `Array<VaultItem(0x0A07)>`)<br>3. `ref chest_info_array` (`0x008E:0x0009`, WState `0x0022` `Array<BankChestInfo(0x0C1F)>`)<br>4. `u32 max_capacity` (`0x008E:0x0006`)<br>5. `u32 used_capacity_or_flags` (`0x008E:0x0006`)<br>6. `u64 npc_or_chest_iid` (`0x008E:0x0008`) | Open storage UI and deliver full chest/escrow item list (`0x0A02:v133`). |
| `0x8A040010` | S→C (`v6`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `u32 error_or_close_pid` (`0x008E:0x0006`) | Close storage container or report error (`0x0A02:v134`). |
| `0x8A04001B` | S→C (`v13`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `ref vault_item` (`0x008E:0x0009`, WState `0x0A07`)<br>3. `u32 update_flag` (`0x008E:0x0006`) | Single item added or updated in storage (`0x0A02:v138`). |
| `0x8A040020` | S→C (`v14`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `u32 chest_type` (`0x008E:0x0007`)<br>3. `u32 arg2` (`0x008E:0x0006`)<br>4. `u32 arg3` (`0x008E:0x0006`)<br>5. `u32 arg4` (`0x008E:0x0006`)<br>6. `u64 item_iid` (`0x008E:0x0008`) | Item removed or moved from storage (`0x0A02:v139`). |


### 9.10 Native C++ Housing & Storage WState Classes <a id="dirty-housing-wstate"></a>

Verified from `lotroclient64.exe` native `Serialize` / `Deserialize` implementations:


| Class ID | Name | Wire Layout Summary |
| --- | --- | --- |
| `0x07A8` (`1960`) | `NeighborhoodHouseInfo` | 1. `u64 house_id` (`+0x10`)<br>2. `u32 house_template_did` (`+0x18`)<br>3. `StringInfo owner_name` (`+0x20`)<br>4. `u32 price` (`+0x60`)<br>5. `u32 upkeep_cost` (`+0x64`)<br>6. `u8 flags_or_type` (`+0x68`)<br>7. `u64 owner_iid` (`+0x70`)<br>8. `StringInfo kinship_or_secondary_name` (`+0x78`)<br>9. `u8 is_kinship_or_locked` (`+0xB8`) |
| `0x07A9` (`1961`) | `NeighborhoodHouseList` | 1. `u32 neighborhood_did` (`+0x08`)<br>2. `u32 template_did` (`+0x0C`)<br>3. `Array<0x07A8>` (`+0x10`: `2 × u32` array header + `N` serialized `0x07A8` entries) |
| `0x07AF` (`1967`) | `NeighborhoodInstanceInfo` | 1. `u32 neighborhood_did` (`+0x10`)<br>2. `u32 neighborhood_name_token_or_id` (`+0x14`)<br>3. `u32 template_did` (`+0x18`)<br>4. `u8 status_flags` (`+0x1C`)<br>5. `Array<u32>` (`+0x20`: `2 × u32` header + `N × u32` available house counts by type)<br>6. `Array<u64>` (`+0x30`: `2 × u32` header + `N × u64` friend/kinship house IDs) |
| `0x07B1` (`1969`) | `NeighborhoodDirectoryList` | 1. `u32 template_did` (`+0x08`)<br>2. `Array<0x07AF>` (`+0x10`: `2 × u32` header + `N` serialized `0x07AF` entries) |
| `0x07BC` (`1980`) | `HousingPermissions` | 1. `10 × u32` group permission bitmasks (`+0x08..+0x2C`, 40 bytes)<br>2. `HashTable<u64, u32>` (`+0x30`: `u32 flags`, `u32 bucket_idx`, `u32 count`, followed by `count` pairs of `[u64 char_iid, u32 permission_bitmask]`)<br>3. `u64 owner_iid` (`+0x50`)<br>4. `u8 is_anonymous_or_flag` (`+0x58`) |
| `0x07BD` (`1981`) | `HousingDecorationInfo` | 1. `u32 item_did` (`+0x10`)<br>2. `u64 item_iid` (`+0x18`)<br>3. `u32 hook_id` (`+0x20`)<br>4. `u32 hook_category` (`+0x24`)<br>5. `f32 heading_degrees` (`+0x28`)<br>6. `StringInfo item_name` (`+0x30`)<br>7. `u64 owner_iid` (`+0x70`)<br>8. `StringInfo owner_name` (`+0x78`)<br>9. `u8 is_bound_flag` (`+0xB8`)<br>10. `u8 return_to_owner_flag` (`+0xB9`) |
| `0x07C6` (`1990`) | `HousingUpkeepInfo` | 1. `u32 upkeep_base_cost` (`+0x10`)<br>2. `u32 upkeep_adjusted_cost` (`+0x14`)<br>3. `u32 max_upkeep_periods` (`+0x18`)<br>4. `f64 next_upkeep_due_time` (`+0x20`)<br>5. `f64 grace_period_end_time` (`+0x28`)<br>6. `f64 paid_until_time` (`+0x30`)<br>7. `u32 upkeep_status_flags` (`+0x38`)<br>8. `u8 is_foreclosed_or_locked` (`+0x3C`)<br>9. `u32 mithril_upkeep_cost` (`+0x40`) |
| `0x0801` (`2049`) | `HousingPurchaseEntry` | 1. `u64 house_id` (`+0x10`)<br>2. `u32 house_template_did` (`+0x18`)<br>3. `u32 coin_price` (`+0x1C`)<br>4. `u32 upkeep_cost` (`+0x20`)<br>5. `u32 mithril_or_writ_price` (`+0x24`)<br>6. `u32 house_category_flags` (`+0x28`) |
| `0x0803` (`2051`) | `HousingPurchaseList` | `Array<0x0801>` (`+0x08`: `2 × u32` array header + `N` serialized `0x0801` entries) |
| `0x0E9C` (`3740`) | `BankAddress` | 1. `u64 owner_or_house_iid` (`+0x10`)<br>2. `u32 bank_repository_type` (`+0x18`, `Enum_BankRepositoryType` `0x230002B8`)<br>3. `u32 chest_type` (`+0x1C`, `Enum_ChestType` `0x230001CD`) |
| `0x0C1F` (`3103`) | `BankChestInfo` | 1. `u32 chest_type` (`+0x10`, `Enum_ChestType` `0x230001CD`)<br>2. `StringInfo chest_name` (`+0x18`)<br>3. `u32 permission_or_capacity_flag` (`+0x58`) |
| `0x0A07` (`2567`) | `VaultItem` | 1. `u64 item_iid` (`+0x10`)<br>2. `u64 container_or_owner_iid` (`+0x18`)<br>3. `PropertySet` (`+0x20`, class `0x00A6`)<br>4. `Property` (`+0x80`, class `0x0027`, `UI_Examination_Tooltip_DID`)<br>5. `u8 is_bound_or_flag` (`+0xB0`) |

---

## 10. Cosmetic Outfits, Wardrobe, Barber, Vault/Shared Storage, Carry-Alls, Inspection & Hobbies <a id="cosmetics-vault-hobbies"></a>

### 10.1 Common Entity RPC Framing <a id="spec-rpc-framing"></a>

All messages specified below are transmitted as entity-targeted RPC events (`NetBlob` event `0x30`) addressed to the local player avatar's 64-bit entity `InstanceID` (`IID`). Each RPC payload begins with a 32-bit little-endian `rpc_id` followed by a serialized argument frame whose fields are written and read strictly in the order listed in the tables below.

* Integer fields (`u32`, `u64`) and floating-point fields (`f32`) are little-endian.
* Boolean flags are encoded as 32-bit unsigned integers (`0` = false, `1` = true).
* `BankAddressRef` represents a serialized bank address object containing a 32-bit `bank_type` (`1` = Character Vault, `2` = Account Shared Storage, `3` = Shared Housing Storage, `4` = Standard Housing Chest) and a 64-bit `owner_iid`.

### 10.2 Cosmetic Outfits, Visibility, Barber, & Wardrobe <a id="spec-cosmetics-wardrobe"></a>

#### 10.2.1 Cosmetic Outfit & Slot Visibility RPCs <a id="spec-outfit-rpcs"></a>

Active outfit index is tracked on the avatar via property `0x10001C64` (`0` = Equipment Outfit, `1..N` = Cosmetic Outfit `1..N`). Per-outfit slot visibility bitmasks are mapped via property `0x10006B0E`, with default all-visible mask `0x01FFFFFE` (`0x10001C5D`). Client requests are gated by cooldown property `0x10001C4C`.

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x881B0005` | C→S | Select Active Outfit | 1. `outfit_type: u32` (`0` = Equipment, `1..N` = Cosmetic `1..N`) |
| `0x881B000A` | S→C | Select Outfit Failed | 1. `reverted_outfit_type: u32` |
| `0x881B0007` | C→S | Equip Cosmetic Item | 1. `wardrobe_item_props: PropertyBagRef` (null if equipping from inventory; `0x10002F15` bag with `0x10002F13` `u32` DID, `0x10002F12` `f32` color, and `0x10003AAD` `u64` banker IID if equipping from Wardrobe)<br>2. `item_iid: u64` (non-zero inventory IID, or `0` if from Wardrobe)<br>3. `slot_bitmask: u32`<br>4. `outfit_type: u32` |
| `0x881B000B` | C→S | Unequip Cosmetic Slot | 1. `slot_bitmask: u32`<br>2. `outfit_type: u32` |
| `0x881B000D` | C→S | Set Slot Visibility | 1. `b_visible: u32` (`0` = hidden, `1` = visible)<br>2. `slot_bitmask: u32`<br>3. `outfit_type: u32` |
| `0x881B0010` | S→C | Slot Visibility Result | 1. `b_visible: u32`<br>2. `slot_bitmask: u32`<br>3. `outfit_type: u32` |

#### 10.2.2 Barber (`AppearanceUI`) RPCs <a id="spec-barber-rpcs"></a>

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x883A0003` | S→C | Open Barber UI | 1. `barber_npc_iid: u64` |
| `0x883A0001` | C→S | Commit Appearance Changes | 1. `appearance_props: PropertyBagRef` |
| `0x883A0005` | S→C | Close Barber UI | *(empty)* |

#### 10.2.3 Account Wardrobe RPCs <a id="spec-wardrobe-rpcs"></a>

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

#### 10.2.4 Mounted Combat Cosmetic RPCs <a id="spec-mount-cosmetic-rpcs"></a>

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x8DA70007` | C→S | Apply Mount Cosmetic Dye | 1. `slot_type: u32`<br>2. `color_id: u32`<br>3. `trait_did: u32`<br>4. `dye_item_iid: u64` |
| `0x8DA70009` | C→S | Select Mount Appearance Trait | 1. `slot_type: u32`<br>2. `trait_did: u32` |
| `0x8DA7000B` | C→S | Select Mount Appearance Color | 1. `color_id: u32`<br>2. `trait_did: u32` |
| `0x8DA7000D` | C→S | Set Mount Slot Visibility | 1. `b_visible: u32`<br>2. `slot_type: u32` |
| `0x8DA7000F` | C→S | Save Mount Name | 1. `mount_name: WStringRef` |
| `0x8DA70011` | S→C | Mount Cosmetic Updated | *(empty)* |
| `0x8DA70013` | C→S | Purchase Mount Cosmetic Slot | *(empty)* |

### 10.3 Vault, Shared Storage, & Carry-All Bags <a id="spec-vault-carryall"></a>

#### 10.3.1 Vault & Shared Storage (`BankRepositorySystem`) RPCs <a id="spec-bank-rpcs"></a>

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

#### 10.3.2 Carry-All Bag (`BoS`) RPCs <a id="spec-carryall-rpcs"></a>

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

### 10.4 Player Inspection, Traits, Skirmish Soldiers, & Hobbies <a id="spec-inspect-traits-hobbies"></a>

#### 10.4.1 Player Inspection & Biography RPCs <a id="spec-inspect-rpcs"></a>

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

#### 10.4.2 Trait Slotting, Skirmish Soldier Traits, & Preset Configurations <a id="spec-trait-rpcs"></a>

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

#### 10.4.3 Hobby System (Fishing & Birding) RPCs & Execution Flow <a id="spec-hobby-rpcs"></a>

Hobby training and proficiency tracking use the `0x8859xxxx` RPC family below. Active execution in the world (casting/reeling a Fishing line or using Birding) is performed by activating the hobby's toggle skill (`0x10001EF5` `Hobby_Skill`) while an implement matching `0x10001DD5` (`Hobby_ToolCategory`) is equipped in slot `0x20000000` and `0x10001EF6` (`Hobby_Status`) is `0`.

| RPC ID | Direction | Operation | Argument Frame Fields (In Wire Order) |
|---|---|---|---|
| `0x88590001` | C→S | Untrain / Abandon Hobby | 1. `hobby_did: u32` |
| `0x88590003` | C→S | Train Hobby | 1. `hobby_did: u32` |
| `0x8859000B` | S→C | Offer Hobby Training | 1. `hobby_did: u32`<br>2. `trainer_npc_iid: u64` |
| `0x88590011` | S→C | Hobby Trained | 1. `hobby_did: u32` |
| `0x88590026` | S→C | Reset Daily Hobby Proficiency | 1. `hobby_did: u32` |
| `0x88590028` | S→C | Record Territory Proficiency Gain | 1. `territory_did: u32` |

### 10.5 Testable Invariants & Synthetic Test Vectors <a id="spec-invariants-vectors"></a>

#### 10.5.1 Invariants
1. **Outfit Equip Source Exclusivity:** In `0x881B0007` (`OutfitEquip`), either `item_iid != 0` and `wardrobe_item_props` is null (equipping from inventory), or `item_iid == 0` and `wardrobe_item_props` is a non-null `0x10002F15` property collection containing `0x10002F13` (`Wardrobe_ItemDID`).
2. **Slot Visibility Mask Preservation:** Toggling visibility of `slot_bitmask` via `0x881B000D` updates only the bits present in `slot_bitmask` within the outfit's visibility property (`0x10006B0E`), defaulting to `0x01FFFFFE` (`0x10001C5D`) when unset.
3. **Carry-All Bound Quantity Partition:** For every `0x1000677B` (`BoS_ItemStruct`) entry in `0x10006782`, `BoS_Bound_To_Account_Amount (0x10006775) + BoS_Bound_To_Character_Amount (0x10006788) <= BoS_Quantity (0x10006789)`.
4. **Hobby Execution Precondition:** A client must not dispatch the toggle skill in `0x10001EF5` (`Hobby_Skill`) unless `0x10001EF6` (`Hobby_Status`) equals `0` and the equipped item in slot `0x20000000` has category matching `0x10001DD5` (`Hobby_ToolCategory`).

#### 10.5.2 Synthetic Test Vectors (Little-Endian Argument Payloads)
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

### 10.6 Implementer Notes <a id="spec-implementer-notes"></a>

* **Runtime Data Loading and Server Entitlements:** All property definitions (`0x34000000`) and hobby/trait/item descriptors must be loaded at runtime from the user's local installation `.dat` files (`CLAUDE.md`, `docs/CLEAN_ROOM.md`). Never embed proprietary assets, keys, or server endpoints in source code or binaries, and respect all server-enforced cooldown (`Antispam_*`) and account entitlement (`Account_Outfit_System_Available`, `Wardrobe_MaxCapacity`, `BankRepository_MaxCapacity`) properties without modification or bypass.


---

### 10.7 Master Property IDs for Outfits, Wardrobe, Bank Repositories, Carry-Alls, Inspection & Hobbies (`0x34000000`) <a id="cvh-master-pids"></a>

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

## 11. Complete Summary Index of `crates/net/src/domain.rs` Gameplay Events & Commands <a id="domain-rs-summary-index"></a>

The tables below map every domain event and command variant in `crates/net/src/domain.rs` (using exact Rust variant and field names) to its authoritative wire representation in §3–§8 (`BIN`, `DAT`).

### 11.1 Character Domain (`CharacterEvent` & `CharacterCommand`) <a id="domain-character-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / Property ID) | Spec Section |
| :--- | :--- | :--- | :--- |
| `CharacterEvent::CharacterInfo { name, body, class, level, look }` | S→C | Pre-world character persona record (`official-messages-world.md` §3) and in-world entity creation `0x0098` / `0x0052` (`Advancement_Level` `0x10001011`). | §3.6 |
| `CharacterEvent::CharacterRemoved { name }` | S→C | Pre-world character deletion acknowledgement (`official-messages-world.md` §3). | §3.6 |
| `CharacterEvent::PropertyChanged { entity, property, value }` | S→C | Live property update `NetBlobEvent`s: `0x0052` (`PropertyCollection`), `0x0088` (`Int32` set), `0x0092` (`Int32` delta), `0x00FE` (`Float` set), `0x00B0` (`Float` delta), `0x0149` (`Int64` set), `0x000E` (`Bool` set), `0x00E7` (`InstanceID` set), `0x0076` (`DataID` set), `0x014A` (`Position` set), `0x008E` (remove `PID`), `0x00B7` (typed property update). | §3.6 |
| `CharacterEvent::CharacterRefused { name, reason }` | S→C | Pre-world character creation/entry refusal (`official-messages-world.md` §3). | §3.6 |
| `CharacterEvent::LevelChanged { entity, level, quiet }` | S→C | Property `0x10001011` (`Advancement_Level`, `Int32`) via `0x0088`/`0x0052` (`quiet = true`), paired with WLib RPC `0x84DF0016` on `NetBlobEvent` `0x30` (`quiet = false` level-up fanfare). | §3.6, §8.1 |
| `CharacterCommand::CreateCharacter { name, appearance, class, nationality }` | C→S | Pre-world character creation request (`official-messages-world.md` §3). | §3.6 |
| `CharacterCommand::DeleteCharacter { name }` | C→S | Pre-world character deletion request (`official-messages-world.md` §3). | §3.6 |

### 11.2 Inventory & Equipment Domain (`InventoryEvent` & `InventoryCommand`) <a id="domain-inventory-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / Property ID) | Spec Section |
| :--- | :--- | :--- | :--- |
| `InventoryEvent::ContainerSize { container, slots }` | S→C | Property `0x10000915` (`Inventory_MaxQuantity`, `Int32`) on `container` via `0x0052` / `0x0088`. | §3.4, §3.6 |
| `InventoryEvent::ItemAdded { item, what, at, quantity }` | S→C | Entity creation (`0x0098` / `0x0052`) or `NetBlobEvent` `0x00CE` (`containment_type = 0x1000` or `0x0100`) + `PID 0x00000001` (`what` DID), `PID 0x0000034E` (`at` slot bitmask), `PID 0x10000914` (`quantity`), and WLib RPC `0x850A0033` / `0x8513000D`. | §3.1–§3.5 |
| `InventoryEvent::ItemMoved { item, at }` | S→C | `NetBlobEvent` `0x00CE` (re-parenting / `containment_type` change) and/or `0x00B7` update of `PID 0x0000034E` (`Inventory_Slot`: `0x40000000 \| bag_slot` or `0x20000000 \| equip_bit`). | §3.1, §3.3, §3.6 |
| `InventoryEvent::ItemQuantity { item, quantity }` | S→C | `NetBlobEvent` `0x0088` / `0x0092` updating `PID 0x10000914` (`Inventory_Quantity`, `Int32`) on `item`. | §3.4, §3.6 |
| `InventoryEvent::ItemRemoved { item }` | S→C | Entity destruction (`0x0147` / `0x0148`), `NetBlobEvent` `0x00CE` (`is_contained = 0`), or WLib RPC `0x850A0024` / `0x8513000E`. | §3.3, §3.5 |
| `InventoryEvent::ItemCondition { item, current, max }` | S→C | `NetBlobEvent` `0x0088` / `0x0052` updating `PID 0x10000D86` (`Item_StructurePoints` = `current`) and `PID 0x10000A35` (`Item_MaxStructurePoints` = `max`), plus `PID 0x10000D8B` (`Item_WearState`). | §3.4, §3.6 |
| `InventoryEvent::WalletChanged { currency, amount }` | S→C | Coin: `PID 0x1000012E` (`Currency_Value`, copper) via `0x0088` / `0x0092`. Wallet `PaperItem` tokens: WLib RPCs `0x8C0D0006` (full `0x10003B2B` array), `0x8C0D0001` (`paper_item_did: u32, quantity: u32`), and `0x8C0D0004` (`flags: u32, quantity: u32, paper_item_did: u32`). | §3.4, §5.6 |
| `InventoryEvent::ItemProperty { item, property, value }` | S→C | `NetBlobEvent` `0x0052` / `0x0088` / `0x00FE` / `0x00B7` updating `property` on `item`. | §3.4, §3.6 |
| `InventoryCommand::MoveItem { item, to }` | C→S | WLib RPC `0x850A000B` (`quantity: i32 = 0, dest_slot: u32 = 0x40000000 \| slot, dest_container_iid: u64, item_iid: u64`). | §3.5 |
| `InventoryCommand::EquipItem { item, slot }` | C→S | WLib RPC `0x850A001D` (`target_slot: u32 = 0x20000000 \| (1 << (equip_index - 1)), flags: u32 = 0, item_iid: u64`). | §3.1, §3.5 |
| `InventoryCommand::UnequipItem { item }` | C→S | WLib RPC `0x850A0001` (`dest_slot: u32 = 0, dest_container_iid: u64 = avatar_iid, item_iid: u64`). | §3.5 |
| `InventoryCommand::SplitStack { item, quantity }` | C→S | WLib RPC `0x850A000B` (`quantity: i32 = quantity, dest_slot: u32 = 0x40000000 \| slot, dest_container_iid: u64, item_iid: u64`). | §3.5 |
| `InventoryCommand::DestroyItem { item }` | C→S | WLib RPC `0x850A0019` (`quantity_or_flags: i32 = 0, item_iid: u64`). | §3.5 |
| `InventoryCommand::UseItem { item, target }` | C→S | Skill-bearing items (`PID 0x10000426` != 0): WLib RPC `0x85280005` (`ref<0x0D85>` with `flags \| 0x04` and `item_entity_id`). Direct-use items: WLib RPC `0x8535001D` (`ref<0x063E>` / `action_flags, item_iid`). | §3.5, §4.3, §5.2 |

### 11.3 Combat, Skills, Quickslots, Effects & Vitals Domain (`CombatEvent` & `CombatCommand`) <a id="domain-combat-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / Property ID) | Spec Section |
| :--- | :--- | :--- | :--- |
| `CombatEvent::TargetChanged { entity, target }` | S→C | WLib RPC `0x851E0012` (`u64 target_entity_id` for local player) and replicated property `0x0000034F` (`Entity_Selection`, `u64 InstanceID`) on any `entity`. | §4.2 |
| `CombatEvent::VitalChanged { entity, vital, current, max }` | S→C | Property updates on `entity`: Morale (`0x100012D2` `Health_CurrentLevel`, `0x100008BB` `Health_MaxLevel`, `0x10003F81` `Health_Temporary`) and Power (`0x10000340` `Power_CurrentLevel`, `0x10000520` `Power_MaxLevel`, `0x10003F82` `Power_Temporary`). | §4.5 |
| `CombatEvent::CombatState { entity, in_combat }` | S→C | Property `0x10000EC9` (`CombatState_CurrentState`, `u32` bitmask: `0x00040000` = Peace `in_combat = false`; combat bits = `true`). | §4.5 |
| `CombatEvent::QuickslotChanged { slot, content }` | S→C | WLib RPCs `0x89580005` (`ref<0x01C6> shortcut, u32 slot_index, u32 page_index`), `0x89580001`, `0x8958000B`, `0x8958000D`, and `0x851E002C`. | §4.4 |
| `CombatEvent::CastStarted { entity, cast, what, target, duration_ms }` | S→C | Property `0x100067E7` (`Skill_InductionAction_Struct`: `0x100067D8` `StartTime` `f64`, `0x100067D9` `EndTime` `f64`, `0x10000729` `Skill_DefaultSkill` `u32`, `0x10000C5C` `Skill_InductionAction` `u32`) and `0x100011FA` (`Skill_CurrentChanneledSkillTarget` `u64`). | §4.3 |
| `CombatEvent::CastDelayed { entity, added_ms }` | S→C | Property `0x100067E7` (`Skill_InductionAction_Struct`) update while active with increased `EndTime` (`0x100067D9`). | §4.3 |
| `CombatEvent::CastEnded { entity, outcome }` | S→C | Property `0x100067E7` (`StartTime == 0.0, EndTime == 0.0`), `0x100003C8` (`Induction_CurrentInductionState` -> `0`), or WLib RPCs `0x85200005` / `0x85200006` / `0x85200007` and `0x85280036`. | §4.3 |
| `CombatEvent::CooldownStarted { what, duration_ms }` | S→C | WLib RPC `0x84EE0008` (`ref<0x03F8> timer` `[f64 start_timestamp, f32 duration_s]`, `u64 cooldown_key` where upper bits `0x00010000` = skill DID, `0x00020000` = channel ID). | §4.3 |
| `CombatEvent::CooldownEnded { what }` | S→C | WLib RPCs `0x84EE000A` (`u64 cooldown_key`), `0x84EE0001` (list of keys), and `0x84EE0003` (reset all). | §4.3 |
| `CombatEvent::EffectApplied { entity, effect, what, duration_ms, stacks, source }` | S→C | WLib RPCs `0x84F8000F` / `0x84F80020` (local player `ref<0x0198> Effect`) and `0x84F80025` (remote entity `ref<0x0198> effect, u64 target_entity_id`). | §4.5 |
| `CombatEvent::EffectRemoved { entity, effect }` | S→C | WLib RPCs `0x84F80003` (`u32 removal_flags, u32 effect_instance_id`), `0x84F8000A` (`u32 effect_instance_id`), `0x84F80023` (remote entity), and `0x84F80019` (clear all). | §4.5 |
| `CombatEvent::VitalDelta { source, target, vital, kind, amount, result, what, damage_type }` | S→C | WLib RPCs `0x84E9003D` (`ref<0x00A6> prop_collection`), `0x84E9004F` (`ref<0x00A6> prop_collection, u64 entity_id`), and `0x85370003` (`ref<0x0662> vital_update`). | §4.5 |
| `CombatEvent::Defeated { entity, by }` | S→C | Property `0x100001AF` (`Death_Incapacitated`, `u8` bool `0 -> 1`) + WLib RPC `0x84F20012` (`f32 release_timeout_s`). | §4.5 |
| `CombatEvent::Revived { entity }` | S→C | Property `0x100001AF` (`Death_Incapacitated`, `u8` bool `1 -> 0`) + WLib RPC `0x84F20004`. | §4.5 |
| `CombatCommand::SelectTarget { target }` | C→S | WLib RPC `0x851E001E` (`u64 target_entity_id`, `0` = clear) and combat target sync `0x84E90003` (`u64 target_entity_id, u32 context_type`). | §4.2 |
| `CombatCommand::UseSkill { what, target }` | C→S | WLib RPC `0x85280005` (`ref<0x0D85> SkillExecutionRequest` containing `u32 flags`, `u32 skill_did`, `u32 seq_token`, optional `u64 target_entity_id`). | §4.3 |
| `CombatCommand::CancelCast` | C→S | WLib RPC `0x85280007` (`0` arguments: interrupt/cancel active induction or channel) or `0x85280034` (`u32 skill_did` untoggle). | §4.3 |
| `CombatCommand::SetQuickslot { slot, content }` | C→S | WLib RPC `0x851E0020` (`ref<0x01C6> shortcut, u32 slot_index` in `0..83`). | §4.4 |

### 11.4 NPC Interaction, Quests, Vendors & Loot Domain (`NpcEvent` & `NpcCommand`) <a id="domain-npc-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / Property ID) | Spec Section |
| :--- | :--- | :--- | :--- |
| `NpcEvent::DialogueShown { window, speaker, text, options }` | S→C | WLib RPCs `0x85230006` (`npc_iid: u64, flags_a: u32, flags_b: u32, options: WState`), `0x85220002` (`dialog_data: WState, dispenser_action: u32, source_iid: u64`), and `0x85270006` (`text_payload: WState`). | §5.2 |
| `NpcEvent::QuestOffered { window, quest, giver }` | S→C | WLib RPC `0x85220002` (`dispenser_action = 6` `Bestow` in `0x2300001D`, `0x1000146E` `QuestDispenser_Quest`) or auto-bestow `0x852100CE` (`quest_did: u32`). | §5.2, §5.3 |
| `NpcEvent::QuestState { quest, state }` | S→C | WLib RPCs `0x8521005C` (login 6-table sync), `0x8521005E` (bestow/update), `0x852100B5` (completed), `0x85210052` (removed/abandoned), `0x8521001F` (status/failed). | §5.3 |
| `NpcEvent::QuestObjective { quest, objective, count, total, done }` | S→C | WLib RPCs `0x85210075` (`quest_did: u32, objective_index: u32, count: u32`), `0x852100DF` (`quest_did: u32, objective_index: u32, state: u32`), and `0x85210010` (`quest_did: u32, role_index: u32, progress: WState`). | §5.3 |
| `NpcEvent::QuestTracked { quest, tracked }` | S→C | WLib RPCs `0x852100D5` (`locked: u32, tracker_array: WState` `0x10000E6B`) and `0x85210106` (`quest_did: u32, slot: u32`). | §5.3 |
| `NpcEvent::VendorShown { window, vendor, entries }` | S→C | WLib RPC `0x85230013` (`service_type: u32, npc_iid: u64, service_payload: WState` populating `Vendor_Inventory` `0x10000059`) and `0x85360004`. | §5.2, §5.4 |
| `NpcEvent::LootShown { window, source, entries, coin }` | S→C | WLib RPCs `0x85130004` (`result_code: u32, flags: u32, container_iid: u64`) and `0x85120019` (`loot_items: WState, container_iid: u64`). | §5.5 |
| `NpcEvent::LootTaken { window, entry }` | S→C | WLib RPCs `0x8513000E` (`item_iid: u64, slot: u32, container_iid: u64`) and `0x85120019` (updated container loot list). | §3.5, §5.5 |
| `NpcEvent::WindowClosed { window }` | S→C | WLib RPCs `0x8523001C` (`status: u32`), `0x8523003B` (`npc_iid: u64`), `0x8522000A` (`result_code: u32, source_iid: u64`), and `0x85130014` (`result_code: u32, flags: u32, container_iid: u64`). | §5.2, §5.5 |
| `NpcEvent::QuestRewards { window, entries }` | S→C | WLib RPC `0x85220002` (`dialog_data: WState` containing fixed and selectable reward tables for `source_iid`). | §5.2, §5.8 |
| `NpcEvent::BuybackShown { window, entries }` | S→C | Player/vendor property `Buyback_Inventory` (`Buyback_BuybackID` `0x1000056B`) synchronized during vendor session `0x85230013` / `0x85360004`. | §5.4 |
| `NpcCommand::UseEntity { entity }` | C→S | WLib RPC `0x8535001D` (`usage_req: WState` `0x063E`) or `0x85230016` (`npc_iid: u64` NPC interaction session open). | §5.2 |
| `NpcCommand::ChooseOption { window, option }` | C→S | WLib RPC `0x85230008` (`option_id: u32, npc_iid: u64`). | §5.2 |
| `NpcCommand::CloseWindow { window }` | C→S | WLib RPC `0x85230017` (close NPC session) or `0x85130016` (`container_iid: u64` close loot container). | §5.2, §5.5 |
| `NpcCommand::AnswerQuest { window, accept }` | C→S | WLib RPC `0x85220006` (`selection_data: WState` `0x00A6`, `dispenser_did: u32`, `source_iid: u64`) or `0x8521004B` (`quest_did: u32` auto-bestow accept). | §5.2, §5.3, §5.8 |
| `NpcCommand::AbandonQuest { quest }` | C→S | WLib RPC `0x85210008` (`quest_did: u32`). | §5.3 |
| `NpcCommand::TrackQuest { quest, tracked }` | C→S | WLib RPC `0x852100D6` (`quest_did: u32, tracker_op: u32`) or `0x8521002F` (`quest_did: u32, slot_or_op: u32`). | §5.3 |
| `NpcCommand::Buy { window, entry, quantity }` | C→S | WLib RPC `0x85360001` (`quantity: u32, item_did: u32, vendor_iid: u64`). | §5.4 |
| `NpcCommand::Sell { window, item, quantity }` | C→S | WLib RPC `0x8536000A` (`items_to_sell: WState` `0x100013AC`, `vendor_iid: u64`) or `0x85360016` (`slot_or_param: u32, vendor_iid: u64`). | §5.4 |
| `NpcCommand::TakeLoot { window, entry }` | C→S | WLib RPC `0x85120014` (`item_iid: u64, container_iid: u64`) or `0x85120025` (`target_iid: u64` take all). | §5.5 |
| `NpcCommand::BuyBack { window, entry }` | C→S | WLib RPC `0x84FE008E` (`quantity: u32, buyback_id: u32`). | §5.4 |

### 11.5 Social, Fellowship, Trade & Player Music Domain (`SocialEvent` & `SocialCommand`) <a id="domain-social-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / ChatNet Opcode) | Spec Section |
| :--- | :--- | :--- | :--- |
| `SocialEvent::ChannelJoined { channel }` | S→C | `ChatNet` `0x00000178` (`ChatRoomEnterResponseEvent`) / `0x0000015C` (`official-messages-chat.md`) or World Chat `0x84EC000E`. | §6.4 |
| `SocialEvent::ChannelLeft { channel }` | S→C | `ChatNet` `0x0000018F` (`ChatRoomLeaveResponseEvent`) (`official-messages-chat.md`) or World Chat `0x84EC000E`. | §6.4 |
| `SocialEvent::FellowshipChanged { members }` | S→C | WLib RPCs `0x84FE0052` (full `0x044F` `Fellowship` sync), `0x84FE0044` (member joined `0x044D`), `0x84FE0043` / `0x84FE0053` (member left/kicked), `0x84FE0045` (leader changed), `0x84FE0046` / `0x84FE0054` (disbanded/left). | §6.2 |
| `SocialEvent::FellowshipInvite { from }` | S→C | WLib RPC `0x84FE004F` (`u32 token, ref(WString) recruiter_name, u32 is_raid, u64 recruiter_iid`). | §6.2 |
| `SocialEvent::TradeShown { window, peer }` | S→C | WLib RPC `0x8531000F` (`ref(Obj) extra, u64 slave_iid, u64 master_iid`). | §6.3 |
| `SocialEvent::TradeOffer { window, side, entries, coin }` | S→C | WLib RPCs `0x8531004E` (my item added), `0x8531004C` (partner item added), `0x85310068` (item revoked), `0x8531000B` (copper changed), and `0x8531000E` (`ref(0x0193) money, u64 player_iid`). | §6.3 |
| `SocialEvent::TradeReady { window, side, ready }` | S→C | WLib RPCs `0x85310061` (`u32 is_accepted, u64 player_iid`), `0x85310046` (reset), `0x8531005F` (completed), and `0x85310052` (finalized). | §6.3 |
| `SocialEvent::FriendList { entries }` | S→C | WLib RPCs `0x84EC000A` (full `HashTable<WString, ref(0x02B5)>`), `0x84EC000C` (entry added/updated), and `0x84EC000D` (entry removed). | §6.4 |
| `SocialEvent::IgnoreList { names }` | S→C | WLib RPCs `0x84EC000B` (full `HashTable<WString, ref(0x02B5)>`), `0x84EC000F` (entry added/updated), and `0x84EC0010` (entry removed). | §6.4 |
| `SocialEvent::PerformanceStarted { entity, tune, delay_ms }` | S→C | WLib RPC `0x85190004` (`u32 is_sync, u32 instrument_did, u64 performer_iid`) + note streams `0x85190005` / `0x85190006` / `0x8519001A` (`ref(0x0804) MusicNoteBatch`). | §6.5 |
| `SocialEvent::PerformanceStopped { entity }` | S→C | WLib RPC `0x8519000C` (`u64 performer_iid`). | §6.5 |
| `SocialEvent::PerformanceReady { member, entity, tune }` | S→C | WLib RPC `0x8519000A` (`u32 is_sync, ref(WString) song_title, u32 instrument_did, u64 performer_iid`). | §6.5 |
| `SocialEvent::PerformanceSyncStart { by, at_ms, lead_ms }` | S→C | WLib RPC `0x8519000B` (`u64 initiator_iid`). | §6.5 |
| `SocialEvent::PerformanceWithdrawn { member }` | S→C | WLib RPC `0x8519000C` (`u64 performer_iid`). | §6.5 |
| `SocialEvent::FellowVitalChanged { member, vital, current, max }` | S→C | WLib RPCs `0x84FE008C` (`u32 mask, u32 cur_morale, u32 max_morale, u32 cur_power, u32 max_power, u64 fellow_iid`) and `0x84FE0047` (`ref(0x044D) fellow`). | §6.2 |
| `SocialCommand::JoinChannel { name }` | C→S | `ChatNet` `0x00000177` (`ChatRoomEnterRequestEvent`) (`official-messages-chat.md`) or World Chat `0x84EC000E`. | §6.4 |
| `SocialCommand::LeaveChannel { name }` | C→S | `ChatNet` `0x0000018E` (`ChatRoomLeaveRequestEvent`) (`official-messages-chat.md`) or World Chat `0x84EC000E`. | §6.4 |
| `SocialCommand::InviteToFellowship { name }` | C→S | WLib RPC `0x84FE0057` (`ref(WString) target_name`) or `0x84FE0048` (`u64 target_iid`). | §6.2 |
| `SocialCommand::KickFromFellowship { name }` | C→S | WLib RPC `0x84FE0042` (`u64 target_iid`). | §6.2 |
| `SocialCommand::SetFellowshipLeader { name }` | C→S | WLib RPC `0x84FE0041` (`u64 target_iid`). | §6.2 |
| `SocialCommand::AnswerInvite { accept }` | C→S | WLib RPC `0x84FE0070` (`u32 response, u32 token, u64 recruiter_iid`). | §6.2 |
| `SocialCommand::LeaveFellowship` | C→S | WLib RPC `0x84FE0059` (`0` arguments). | §6.2 |
| `SocialCommand::DisbandFellowship` | C→S | WLib RPC `0x84FE0056` (`0` arguments). | §6.2 |
| `SocialCommand::OpenTrade { entity }` | C→S | WLib RPC `0x85310043` (`u64 target_iid`). | §6.3 |
| `SocialCommand::OfferTrade { window, item, quantity, coin }` | C→S | WLib RPCs `0x85310024` (`u64 item_iid` add), `0x85310060` (`u64 item_iid` revoke), and `0x8531002E` (`ref(0x0193) money`). | §6.3 |
| `SocialCommand::SetTradeReady { window, ready }` | C→S | WLib RPC `0x85310050` (`ref(0x05C3) offer_snapshot` when `ready == true`) or `0x85310042` (`0` args when `ready == false`). | §6.3 |
| `SocialCommand::AddFriend { name }` | C→S | WLib RPC `0x84EC0003` (`ref(WString) name`). | §6.4 |
| `SocialCommand::RemoveFriend { name }` | C→S | WLib RPC `0x84EC0004` (`ref(WString) name`). | §6.4 |
| `SocialCommand::Ignore { name }` | C→S | WLib RPC `0x84EC0005` (`u32 reason, ref(WString) name`). | §6.4 |
| `SocialCommand::Unignore { name }` | C→S | WLib RPC `0x84EC0006` (`ref(WString) name`). | §6.4 |
| `SocialCommand::ReadyPerformance { tune }` | C→S | WLib RPC `0x85190001` (`u32 enter_music_mode = 1`) + `0x85190007` (`u32 is_sync, ref(WString) song_title`). | §6.5 |
| `SocialCommand::StartPerformance` | C→S | WLib RPC `0x85190008` (`0` arguments) + note batch streaming `0x85190015` (`ref(0x0804) MusicNoteBatch`). | §6.5 |

### 11.6 World, Map Markers, Travel, Mounts & Instance Finder Domain (`WorldEvent` & `WorldCommand`) <a id="domain-world-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / Property ID) | Spec Section |
| :--- | :--- | :--- | :--- |
| `WorldEvent::MarkerAdded { marker, location, label, icon, quest, note_type }` | S→C | WLib RPCs `0x8530000C` (entity tracking start), `0x85300006` / `0x85300019` / `0x8530001E` (tracking update), `0x850E0003` / `0x850E000B` (POI landmark discovery), `0x84FE0020` (fellow position), and `0x85210051` (quest guide). | §7.1 |
| `WorldEvent::MarkerRemoved { marker }` | S→C | WLib RPC `0x85300011` (`stop_reason: u32, tracking_state: u32, tracked_iid: u64`). | §7.1 |
| `WorldEvent::TravelShown { window, source, entries }` | S→C | WLib RPC `0x85330002` (`npc_iid: u64`, resolving routes from `TravelWebWC` `0x10000F66` and discovered set `0x85330006` / `0x8533001B`). | §7.2 |
| `WorldEvent::TravelStarted { destination, duration_ms, route }` | S→C | Player properties `0x10001B2B` (`Travel_RouteDID` = `route`), `0x10004031` (`Travel_Allow_Dismount`), and `0x100009E8` (`Travel_IsTraveling` = `1`). | §7.2 |
| `WorldEvent::TravelEnded` | S→C | Player property `0x100009E8` (`Travel_IsTraveling` = `0`). | §7.2 |
| `WorldEvent::MountChanged { entity, mount, combat }` | S→C | Rider properties `0x10000AAF` (`Mount_Is_Mounted`), `0x10002A79` (`Mount_Cache_MountEffectDID`), and `0x10002A90` (`Mount_CurrentMountType`: `(val & 0x38) != 0` -> `combat = true`). | §7.3 |
| `WorldEvent::MountVitalChanged { entity, vital, current, max }` | S→C | Rider properties: standard mount durability (`0x10002A62` / `0x10002A64`), War-Steed Endurance (`0x10004939` / `0x10004935`), War-Steed Power (`0x10004933` / `0x10004937`), and Fury (`0x100037C2` `Pip_Momentum`). | §7.3, §7.7 |
| `WorldEvent::InstanceListShown { window, entries, source, featured }` | S→C | WLib RPC `0x8A2C0003` (`available_encounters: ObjRef`). | §7.4 |
| `WorldEvent::InstanceChanged { instance }` | S→C | WLib RPCs `0x851F000C` (`template_did: u32` enter) and `0x851F0027` (`0` args leave), plus player property `0x100007AB` (`PrivateEncounter_Template_DataID`). | §7.4 |
| `WorldEvent::InstanceQueueChanged { state, instance, seconds }` | S→C | Player property `0x10003DCA` (`FellowshipMaker_PlayerState`) and WLib RPCs `0x8A2C000B` (`WorldJoinInvite` `0x0A39`), `0x8A2C0049`, `0x8A2C0062`, `0x8C410028`, `0x8C410034`, `0x8C410037`, `0x8C410049`. | §7.4 |
| `WorldCommand::ChooseTravel { window, entry }` | C→S | WLib RPC `0x8533000D` (`route_did: u32, npc_iid: u64`; preceded by `0x88000006` if currently mounted). | §7.2 |
| `WorldCommand::JoinInstance { window, entry, size, tier, level }` | C→S | Specific WorldJoin: WLib RPC `0x8A2C0005` (`request: WorldJoinRequest` `0x0A44`). Matchmaking queue: `0x8C410055` (specific encounters) or `0x8C410056` (random encounter). | §7.4 |
| `WorldCommand::LeaveInstance` | C→S | WLib RPC `0x851F0136` (`0` arguments). | §7.4 |
| `WorldCommand::Dismount` | C→S | WLib RPC `0x88000006` (`0` arguments; or `0x85330005` during early travel dismount). | §7.2, §7.3 |
| `WorldCommand::LeaveQueue` | C→S | WLib RPC `0x8C41000D` (`0` arguments, FellowshipMaker queue) or `0x8A2C0007` (`0` arguments, WorldJoin request). | §7.4 |

### 11.7 Progression, Traits, Trainers & Crafting Domain (`ProgressionEvent` & `ProgressionCommand`) <a id="domain-progression-index"></a>

| Domain Enum & Variant (`crates/net/src/domain.rs`) | Dir | Official Wire Transport (`NetBlobEvent` / WLib RPC / Property ID) | Spec Section |
| :--- | :--- | :--- | :--- |
| `ProgressionEvent::ExperienceChanged { current, needed }` | S→C | Player properties `0x10000009` (`Advancement_XP`, `i64`), `0x100006A4` (`Advancement_NextLevelXP`, `i64`), and `0x100006A5` (`Advancement_PrevLevelXP`, `i64`). | §8.1 |
| `ProgressionEvent::SkillLearned { what }` | S→C | WLib RPC `0x84DF0003` (`trained_flag: u32, skill_ref: ClassRef(0x0353)`). | §8.1 |
| `ProgressionEvent::SkillForgotten { what }` | S→C | WLib RPC `0x84DF001E` (`skill_ref: ClassRef(0x0353)`). | §8.1 |
| `ProgressionEvent::TrainerShown { window, trainer, entries, offers }` | S→C | WLib RPC `0x84DF001B` (`trainer_iid: u64`, reading `Advancement_TrainerClass` `0x10000008` on `trainer_iid`). | §8.1 |
| `ProgressionEvent::UntrainedSkills { entries }` | S→C | Derived client-side by subtracting `SkillRegistry` (`0x84DF0003`) from the class's `AdvancementTable` (`0x10000006` in `0x3A000000`). | §8.1 |
| `ProgressionEvent::TraitPoints { available, total, tree }` | S→C | Player properties `0x10002B91` (`Trait_PointBasedTraitTree_PointsSpent`), `0x10002B92` (`Trait_PointBasedTraitTree_PointsTotal`), `0x10002B93` (`PointsEarned`), `0x10002B94` (`PointsBonus`). | §8.5 |
| `ProgressionEvent::TraitSlotted { slot, what, rank }` | S→C | WLib RPCs `0x85320003` (`slot_desc: ClassRef(0x05D1), trait_did: u32`) and `0x85320009` (`trait_did: u32` unslotted). | §8.5 |
| `ProgressionEvent::TraitRank { tree, what, rank }` | S→C | WLib RPCs `0x8532001D` / `0x85320056` (trait granted/ranked), `0x8E15000E` (`config_data: ClassRef, config_slot: u32`), and property `0x10002B8F` (`Trait_PointBasedTraitTree_RankArray`). | §8.5 |
| `ProgressionEvent::TraitTreeChanged { tree, branch }` | S→C | WLib RPC `0x8E15000E` (`config_data: ClassRef, config_slot: u32`) and player properties `0x1000201C` (`Trait_ClassSpecialization_ActiveBranch`) / `0x10002B90` (`Trait_PointBasedTraitTree_Branch`). | §8.5 |
| `ProgressionEvent::VocationChanged { vocation, slots }` | S→C | WLib RPC `0x84EF0017` (`vocation_did: u32`) and player property `0x10001481` (`Craft_Vocation`). | §8.6 |
| `ProgressionEvent::ProfessionChanged { profession, tier, proficiency, proficiency_max, mastery, mastery_max, mastery_tier }` | S→C | Player property `0x10006F93` (`Craft_ProfessionArray`) and per-profession dynamic properties resolved via `0x1000079B` (`Enabled`), `0x10000CF1` (`ProficiencyLevel`), `0x10000B32` (`MasteryLevel`), `0x100010BC` (`ProficiencyXP`), `0x10001296` (`MasteryXP`), `0x100026E9` (`OpenLevel`). | §8.6 |
| `ProgressionEvent::RecipeLearned { what, profession, tier, category }` | S→C | WLib RPC `0x84EF0014` (`recipe_did: u32`) and per-profession property `0x100006C6` (`CraftProfession_ExtraRecipeArray_PropertyName`). | §8.6 |
| `ProgressionEvent::RecipeForgotten { what }` | S→C | Removal of `what` from per-profession property `0x100006C6` (`CraftProfession_ExtraRecipeArray_PropertyName`) or profession abandonment `0x84EF005D`. | §8.6 |
| `ProgressionEvent::CraftTrainerShown { window, kind, what, npc }` | S→C | WLib RPCs `0x84EF001A` (`trainer_iid: u64` Vocation Trainer, `0x10000047`), `0x84EF0057` (`trainer_iid: u64` Profession Trainer, `0x10006FE9`), and `0x84EF0041` (`trainer_iid: u64` Craft Guild Trainer). | §8.6 |
| `ProgressionEvent::CraftResult { recipe, outcome, item, batch }` | S→C | WLib RPC `0x84EF0003` (`result_type: u32, recipe_did: u32, profession_did: u32`) + induction lifecycle `0x84EF000E` / `0x84EF002E` / `0x84EF003C`. | §8.6 |
| `ProgressionEvent::ReputationChanged { faction, points, tier }` | S→C | WLib RPC `0x877B000A` (`faction_did: u32`) and per-faction dynamic properties resolved from `ReputationFaction` (`0x0760`): `0x10001C89` (`CurrentTier`), `0x10001C8A` (`CurrentRep`), `0x10001C8E` (`MaxUnlockedTier`). | §8.2 |
| `ProgressionEvent::TitleGranted { what }` | S→C | WLib RPCs `0x852E0005` (`timestamp: f64, new_title_did: u32, replaced_title_did: u32`) and `0x852E000C` (initial title map sync). | §8.3 |
| `ProgressionCommand::TrainSkill { window, entry }` | C→S | WLib RPC `0x84DF0005` (`skill_did: u32, trainer_iid: u64`). | §8.1 |
| `ProgressionCommand::SlotTrait { slot, what }` | C→S | WLib RPC `0x85320016` (`slot_desc: ClassRef(0x05D1), trait_did: u32`) or `0x8532004B` (`config_slot: u32, slot_desc: ClassRef(0x05D1), trait_did: u32`). | §8.5 |
| `ProgressionCommand::SetTraitRank { tree, what, rank }` | C→S | WLib RPC `0x8E15000F` (`rank_map: ClassRef(HashTable<u32, u32>), slotted_map: ClassRef(HashTable), branch: u32, tree_type: u32, config_slot: u32`). | §8.5 |
| `ProgressionCommand::ChooseTraitBranch { tree, branch }` | C→S | WLib RPC `0x8E15000F` (or `0x8E150005` `slotted_map, branch: u32, tree_type: u32, config_slot: u32`). | §8.5 |
| `ProgressionCommand::Craft { recipe, quantity, tier, output, optional, pack }` | C→S | WLib RPC `0x85280005` (`craft_desc: ClassRef(0x0A4A)` `[multi_output_index: u32, optional_ingredients: Array<u32>, use_ingredient_pack: u32, tier: u32]` with `Craft_CurrentRecipe` `0x10001476`). | §8.6 |
| `ProgressionCommand::TrainCraft { window, replace }` | C→S | WLib RPC `0x84EF0006` (`vocation_did: u32, trainer_iid: u64`) or `0x84EF005B` (`profession_did: u32, trainer_iid: u64`). | §8.6 |

---

## 12. Consolidated Hostile-Input Rules & Wire Invariants <a id="hostile-input-and-invariants"></a>

### 12.1 Hostile-Input Validation Rules (`BIN`) <a id="hostile-input-rules"></a>

A clean-room parser handling incoming S→C gameplay `NetBlobEvent`s (`0x28`, `0x30`, `0x50`, `0x5F`, `0xC0`, `0xCE`, `0x52`, `0x88`, `0xB7`, etc.) and `WLibArgFrame` (`0x008E`) payloads must enforce the following exact bounds observed in the client deserializers:

1. **`WLibArgFrame` (`0x008E`) Cursor Bounds:**
 - Every `WLibArgFrame` starts with a 4-byte `u32 LE byte_length` that must not exceed the remaining bytes in the enclosing `NetBlobEvent`.
 - Every primitive unpack (`0x008E0007` `4` bytes, `0x008E0008` `8` bytes, `0x008E0009` `4` bytes, `0x008E000A` `8` bytes) must verify `cursor + size <= byte_length` before reading.
 - Upon completion of an S→C unpack stub (`0x008E000B`), any unconsumed trailing bytes or premature EOF invalidates the RPC dispatch.
2. **`WState` Object-Graph (`0x008E0001` / `0x008E0006`) Bounds:**
 - The 3-tier VLE `item_count` (`N`) must satisfy `N <= 0x7FFF0000` and `4 * N <= remaining_bytes`.
 - For WLib script classes (`(node_flags & 0x10000000) == 0`), `field_count = (node_flags >> 8) & 0xFFFF` requires `8 * field_count <= remaining_bytes`.
 - For native hash tables (`0x000B`, `0x0012`, `0x0017`, `0x0023`, `0x0061`, `0x0075`), `u8 bucket_size_index` must index a prime bucket count `<= 50,000` (`bucket_size_index < 23`).
3. **Entity Creation & Containment (`0x00CE`, `0x0098`, `0x0052`) Bounds:**
 - In `ContainmentLink`, `has_attachment` (`u8` at `+0x0C`) and `attach_flag` (`u8` at `+0x12`) are strict boolean bytes (`0` or `1`; values `>= 2` are rejected).
4. **Native `Shortcut` (`0x01C6`), `BarterRequest` (`0x0796`), `MusicNoteBatch` (`0x0804`), and `AuctionQuery` (`0x0188`) Bounds:**
 - `Shortcut` (`0x01C6`): `shortcut_type` (`u32 LE`) must satisfy `shortcut_type <= 12` (`0x0C`).
 - `BarterRequest` (`0x0796`): Fixed 49-byte (`0x31`-byte) body; byte at `+0x0C` (`is_special_flag`) must be `0` or `1` (values `>= 2` are rejected).
 - `MusicNoteBatch` (`0x0804`): `note_count` (`u32 LE`) must satisfy `20 * note_count + 14 <= remaining_bytes`, and trailer flags `flag_b` and `flag_a` must each be `< 2`.
 - `AuctionQuery` (`0x0188`): `filter_count` (`u32 LE` at `+0x08`) must satisfy `filter_count <= 0x3FFFFFFF`.

### 12.2 Structural Wire Invariants (`BIN`, `DAT`) <a id="structural-wire-invariants"></a>

1. **WLib RPC Census Partition Invariant:** In `client_gamelogic.dat` record `0x56000000` (`wlib.bin`), the `1,168` WLib RPC stubs across `120` RPC classes partition strictly into `619` C→S pack stubs (`0x008E0001..05` + `0x00D9000C`/`0x00D9000D`) and `549` S→C unpack stubs (`0x00D60007` + `0x008E000D` + `0x008E0006..0A`).
2. **Method Identifier Bit-Layout Invariant:** Every WLib RPC `wire_function_id` is a 32-bit little-endian unsigned integer matching `0x80000000 | (rpc_class_id << 16) | fnum`, where bit 31 is always `1`.
3. **FIFO Argument Ordering Invariant:** Every C→S pack stub writes parameters into `WLibArgFrame` (`0x008E`) in the exact FIFO byte order read by the corresponding S→C unpack stub.
4. **Zero Open Questions Invariant:** All `0` `OPEN` callouts remain across P1–P6, Mail, Auction House, Housing, Cosmetics, Wardrobe, Vault/Shared Storage, Carry-Alls, Inspection, Hobbies, and the WLib RPC / Native Package census.

---

## 13. Change Log <a id="change-log"></a>

| Date | Author | Summary |
| :--- | :--- | :--- |
| 2026-10-20 | Spec writer (dirty room) | Initial draft of `official-messages-gameplay.md`. |
| 2026-10-22 | Spec writer (dirty room, `m8-gameplay-assembler`) | Complete rewrite and assembly from the 10 verified M8 dirty-room research notes (`m8-wlib-rpc-census`, `m8-wlib-native-packages`, `m8-gameplay-p1`..`p6`, `m8-gameplay-mail-auction-housing`, `m8-gameplay-cosmetics-vault-hobbies`), reconciled with `official-messages-world.md`, `official-messages-play.md` §5, and `crates/net/src/domain.rs`. Normalized all DIDs (`0x56000000` `wlib.bin` and `0x34000000` Master Property Definition in `client_gamelogic.dat`) and `WState` native class IDs (`0x00E1` `Utf16String`, `0x00C7` `StringInfo`, `0x0027` `PropertyValue`, `0x00A6` `PropertyCollection`). 0 OPEN callouts. |
| 2026-10-22 | Reviewer (clean room) | Audited against `docs/CLEAN_ROOM.md` §5.3, `docs/specs/official-messages-world.md`, `docs/specs/official-messages-play.md`, and `crates/net/src/domain.rs`; marked `Accepted`. |
