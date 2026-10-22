<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 Gameplay Wire Protocol — Part 5: Map Markers, Stable-Master Travel, Mounts & War-Steeds, and Instance Finder / Private Encounters

- **Status:** Draft
- **Last updated:** 2026-10-21
- **Author role:** Spec writer (dirty room)
- **Reviewer:** pending

---

## Purpose

This research note documents the official server wire protocol for the four Part 5 (P5) gameplay subsystems in `crates/net/src/domain.rs`:

1. **Map Markers, Waypoints, Fellowship Positions, Entity Tracking, & Fog-of-War** (`WorldEvent::MarkerAdded`, `WorldEvent::MarkerRemoved`)
2. **Stable-Master Travel, Milestone, Recall, & Summon** (`WorldEvent::TravelShown`, `WorldEvent::TravelStarted`, `WorldEvent::TravelEnded`, `WorldCommand::ChooseTravel`)
3. **Mounts, War-Steeds, Mounted Combat, & Mount Outfits** (`WorldEvent::MountChanged`, `WorldEvent::MountVitalChanged`, `WorldCommand::Dismount`)
4. **Instance Finder (`WorldJoin` & `FellowshipMaker`), Private Encounters, & Skirmishes** (`WorldEvent::InstanceListShown`, `WorldEvent::InstanceChanged`, `WorldEvent::InstanceQueueChanged`, `WorldCommand::JoinInstance`, `WorldCommand::LeaveInstance`, `WorldCommand::LeaveQueue`)

Section 5 provides a **Clean-Room Spec Section** (free of binary virtual addresses, internal symbol names, and decompiler output, with `BIN`/`DAT`/`OBS` provenance tags and synthetic test vectors) ready to be merged directly into `docs/specs/official-messages-gameplay.md`.

---

## Provenance

| Tag | Method | Source | Details |
|---|---|---|---|
| `BIN` | M4 | `x64/lotroclient64.exe` | Version `4102.0056.1824.4022`, SHA-256 `b62b97dbc9dc0f5afd8da9fb1fb8dc93bd9bbd26dc6fb7ebdcabe2fb4d3b0b83`, analysed via static disassembly (`capstone`) and embedded WLib bytecode (`0x141748D50`, 2,000,722 bytes; 3,023 classes; 1,508 native functions) to extract RPC `function_id` values, `WLibArgFrame` (`0x008E`) argument pack/unpack sequences, native C++ serializer layouts, and property callbacks. Static binary analysis was required because M1–M2 sources do not document the WLib RPC wire framing for these subsystems. |
| `DAT` | M1 | `client_highres.dat`, `client_surface.dat`, `client_gamelogic.dat` | Master Property Definition Table (`0x34000000`, 26,645 property definitions) inspected read-only to confirm property IDs (`PID`) and data types for map/landmark discovery, travel webs/routes, mount/war-steed vitals, and instance finder / private encounters. |
| `OBS` | M1 | `/home/solarbaron/git/lotro-reimplementation/crates/net/src/domain.rs` | Clean-room domain event and command definitions for `MarkerAdded`, `MarkerRemoved`, `TravelShown`, `TravelStarted`, `TravelEnded`, `ChooseTravel`, `MountChanged`, `MountVitalChanged`, `Dismount`, `InstanceListShown`, `InstanceChanged`, `InstanceQueueChanged`, `JoinInstance`, `LeaveInstance`, and `LeaveQueue`. |

---

## Framing Recap: `WLibArgFrame` (`0x008E`) RPC Dispatch

All gameplay RPCs in this note are carried inside `GameEvent` (`0x00000003`) or `ClientEvent` (`0x00000004`) entity/avatar messages and dispatched through the WLib virtual machine:

- **32-bit `function_id`:** `0x80000000 | (class_id << 16) | method_id`.
- **Client-to-Server (C->S):** A WLib packer method allocates a `WLibArgFrame` (`new_native(0x008E)`), pushes arguments in order via `Pack_*` methods, and calls `native 0x00D9000C` (`SendServerEvent`, VA `0x1403A6A80`) or `native 0x00D90003` (`SendDirectedEvent`, VA `0x1403A6B40`) with the target `function_id`.
- **Server-to-Client (S->C):** The WLib handler verifies the context via `native 0x00D60007(1731)` (or `1730`), calls `CheckFrame` (`0x008E000D`), and pops arguments in exact FIFO wire order via `Unpack_*` methods before invoking the implementation class method.

| `0x008E` Method ID | Direction | Operation | Wire Type |
|---|---|---|---|
| `0x008E0001` | Pack | `Pack_ObjRef` | Polymorphic/typed WLib object or native class (`0x140367D20`) |
| `0x008E0002` | Pack | `Pack_u32` | `u32` / `i32` / `bool` / `DataID` (4 bytes LE) |
| `0x008E0003` | Pack | `Pack_u64` | `u64` `InstanceID` (8 bytes LE) |
| `0x008E0004` | Pack | `Pack_f32` | `f32` IEEE-754 float (4 bytes LE) |
| `0x008E0005` | Pack | `Pack_f64` | `f64` IEEE-754 double (8 bytes LE) |
| `0x008E0006` | Unpack | `Unpack_ObjRef` | Polymorphic/typed WLib object or native class (`0x140368460`) |
| `0x008E0007` | Unpack | `Unpack_u32` | `u32` / `i32` / `bool` / `DataID` (4 bytes LE) |
| `0x008E0008` | Unpack | `Unpack_u64` | `u64` `InstanceID` (8 bytes LE) |
| `0x008E0009` | Unpack | `Unpack_f32` | `f32` IEEE-754 float (4 bytes LE) |
| `0x008E000A` | Unpack | `Unpack_f64` | `f64` IEEE-754 double (8 bytes LE) |
| `0x008E000D` | Check | `CheckFrame` | Frame cursor validation prior to unpacking |

---

## 1. Map Markers, Waypoints, Fellowship Positions, Entity Tracking, & Fog-of-War

### 1.1 Subsystem Architecture & Domain Mapping

In the LOTRO client, world map and radar markers (`WorldEvent::MarkerAdded`, `WorldEvent::MarkerRemoved`) and map fog-of-war / POI discovery are driven by five distinct mechanisms:

1. **Entity Tracking System (`0x0530` RPC <-> `0x06DF` `CTrackingSystem`):** Dynamic radar/map tracking pins created by Hunter/warden/racial tracking skills (`Track Mines`, `Track Wood`, `Passage of Nature`, `Passage of Foes`, etc.) or admin `/track`.
2. **Landmark / POI Discovery (`0x050E` RPC <-> `0x0110` `CLandmarkSystem`):** Point-of-Interest (POI) / Landmark discovery notifications and initial discovered-landmark list synchronization.
3. **Map Fog-of-War System (`0x0515` RPC <-> `0x01B6` `CMapSystem`):** Per-map fog-of-war bitmap synchronization between client and server.
4. **Area & Dungeon Discovery (`0x04E3` / `0x04E4` RPC <-> `0x00FF` / `0x00F8` `CAreaSystem`):** Area/dungeon transitions driven by player entity property updates (`0x00000092`, `0x00000625`, `0x0000053B`), plus admin `/area query`.
5. **Fellowship Member Map Positions & Target Marks (`0x04FE` / `0x0739` RPC <-> `0x0452` / `0x019D` / `0x0453` / `0x0736`):** Live world-map `Position` updates for fellowship members (`0x84FE0020`) and overhead/radar target marks (`0x0739` C->S, `0x84FE00C4`/`0x84FE00C6` S->C).
6. **Quest Guide / Quest Tracker Waypoints (`0x0521` RPC <-> `0x011F` / `0x06D9` / `0x0575`):** Quest objective event progression and DAT-resolved landmark/region waypoints (`QuestEvent_LandmarkDID` `0x1000038E`, `QuestEvent_LandmarkRegion` `0x10001139`).

---

### 1.2 Entity Tracking System (`0x0530` <-> `0x06DF` `CTrackingSystem`)

RPC class `0x0530` (1328) is purely Server-to-Client (5 S->C handlers; initiating a track uses standard skill execution or item usage). It dispatches into `0x06DF` (`CTrackingSystem`), which stores tracked entity records in helper class `0x05C2` (wrapping native `LongHash` `0x0075` mapping `tracked_iid: u64` -> `IntSet` `0x0019` of active tracking states) and notifies the radar/map UI via native class `0x0162` (`0x01620001`..`0x01620005`).

#### Complete RPC Table — Class `0x0530`

| `function_id` | `fn` | Dir | Bytecode Off | Target Handler (`0x06DF`) | Native UI Call (`0x0162`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|---|
| `0x85300006` | `6` (`0x06`) | S->C | `0x10E7AC` | `0x86DF0004` (`0x1780A8`) -> `0x86DF0003` (`0x177FBC`) | `0x01620003` (`0x140B493E0` -> `0x140B1EF20`) | 1. `Unpack_u32` (`tracking_type_or_flags: u32`)<br>2. `Unpack_ObjRef` (`position: Position` `0x01A7`)<br>3. `Unpack_u64` (`tracked_iid: u64`) | **Update Tracked Entity Position & Flags:** Updates the world `Position` and tracking state/flags for `tracked_iid` in `0x05C2` (`0x85C20009`) and broadcasts `(tracked_iid, position, tracking_type_or_flags)` to UI radar/map listeners (`vtable+0x20`). |
| `0x8530000C` | `12` (`0x0C`) | S->C | `0x10E580` | `0x86DF0006` (`0x177EF4`) -> `0x86DF0005` (`0x177DA0`, `CTrackingSystem::StartTracking`) | `0x01620001` (`0x140B42D30` -> `0x140B14320`) | 1. `Unpack_u32` (`tracking_state: u32`, must be `!= 0`)<br>2. `Unpack_ObjRef` (`entity_name: StringInfo` `0x00C7`)<br>3. `Unpack_u32` (`category_or_icon: u32`)<br>4. `Unpack_ObjRef` (`position: Position` `0x01A7`)<br>5. `Unpack_u64` (`tracked_iid: u64`) | **Start Tracking Entity (`MarkerAdded`):** Validates `tracking_state != 0` (logs `"Invalid tracking state added for entity ... in CTrackingSystem::StartTracking"` if 0), inserts `(tracked_iid, tracking_state)` into `0x05C2` (`0x85C20003`), updates quest monster script (`0x86D90003` at `0x13F2A8`), and broadcasts `(tracked_iid, position, entity_name, category_or_icon, tracking_state)` to UI radar/map listeners (`vtable+0x00`). |
| `0x85300011` | `17` (`0x11`) | S->C | `0x10E638` | `0x86DF0001` (`0x177CB0`) -> `0x86DF0002` (`0x177E58`, `StopTracking`) | `0x01620002` (`0x140B42FE0` -> `0x140B14890`) | 1. `Unpack_u32` (`stop_reason: u32`)<br>2. `Unpack_u32` (`tracking_state: u32`)<br>3. `Unpack_u64` (`tracked_iid: u64`) | **Stop Tracking Entity (`MarkerRemoved`):** Removes `tracking_state` from `tracked_iid` in `0x05C2` (`0x85C20005`). If `stop_reason` is `2` or `3`, displays chat notification string (`0xDFBB3FC` or `0x4C6B42D`) with the entity name (`0x86DF000B`). If no tracking states remain for `tracked_iid`, calls `native 0x01620002(tracked_iid)` (`vtable+0x08`) to remove the radar/map marker and refreshes quest monster scripts. |
| `0x85300019` | `25` (`0x19`) | S->C | `0x10E734` | `0x86DF000A` (`0x178218`) | `0x01620005` (`0x140B49330` -> `0x140B1EE20`) | 1. `Unpack_ObjRef` (`position: Position` `0x01A7`)<br>2. `Unpack_u64` (`tracked_iid: u64`) | **Update Tracked Entity Position:** Broadcasts `(tracked_iid, position)` to UI radar/map listeners (`vtable+0x18`). |
| `0x8530001E` | `30` (`0x1E`) | S->C | `0x10E6C0` | `0x86DF000D` (`0x1781D0`) -> `0x86DF000C` (`0x178134`, `CTrackingSystem::UpdateTrackedEntity`) | `0x01620004` (`0x140B492A0` -> `0x140B1ED20`) | 1. `Unpack_u32` (`tracking_state: u32`)<br>2. `Unpack_u64` (`tracked_iid: u64`) | **Update Tracked Entity State:** Validates `tracked_iid != 0` and `tracking_state != 0`, adds `tracking_state` to `tracked_iid` in `0x05C2`, and calls `native 0x01620004(tracked_iid, primary_effect_did)` (`vtable+0x10`) where `primary_effect_did` is resolved via `0x85C20008` -> `0x877A0005`. |

---

### 1.3 Landmark / POI Discovery (`0x050E` <-> `0x0110` `CLandmarkSystem`)

RPC class `0x050E` (1294) handles Point-of-Interest (POI) / Landmark discovery on the world map and radar.

#### Complete RPC Table — Class `0x050E`

| `function_id` | `fn` | Dir | Bytecode Off | Target Handler (`0x0110`) | Native UI Call (`0x0149`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|---|
| `0x850E0003` | `3` (`0x03`) | S->C | `0x105AB8` | `0x81100005` (`0x0EA360`) | `0x01490005` (`0x140B38A90` -> `0x140B02B90`) | 1. `Unpack_u32` (`discovered_did: u32`)<br>2. `Unpack_u32` (`landmark_did: u32`) | **Discover Single Landmark (`MarkerAdded`):** Checks `vcall(slot=501)` on the player with `landmark_did`. If newly discovered, creates a `PropertyBag` (`0x00A6`) with `Landmark_DiscoveredDID` (`0x10000856`) = `discovered_did`, fires client event `1796` (`vcall(slot=395)`), calls `vcall(slot=520)` with `landmark_did`, and invokes `native 0x01490005(player_entity, landmark_did)` (`Landmark::OnDiscover`, `vtable+0x10`) to reveal the POI icon on the map and radar. |
| `0x850E000B` | `11` (`0x0B`) | S->C | `0x105A54` | `0x81100003` (`0x0EA410`) | `0x01490006` (`0x140B38070` -> `0x140B01820`) | 1. `Unpack_ObjRef` (`discovered_landmarks: IntSet/Bitfield`) | **Sync Discovered Landmarks List:** Stores the player's full discovered-landmarks collection via `vcall(slot=462)` and calls `native 0x01490006` (`Landmark::OnRecvDiscoveredList`, `vtable+0x00`) to refresh all POI markers on the world map. |

> **Note:** Interacting with a Milestone (`0x0628` / `0x0C47`) also discovers its associated landmark if not already known: `0x86280006` (`0x1A2098`) and `0x8C470005` (`0x1A0254`) check `vcall(slot=501)` and, if newly discovered, call `0x85240011` (`0x10C464`, sending C->S `0x8524000E` with `Pack_u32(landmark_did)`) and `native 0x01490005(player_entity, landmark_did)`.

---

### 1.4 Map / Fog-of-War System (`0x0515` <-> `0x01B6` `CMapSystem`)

RPC class `0x0515` (1301) synchronizes the per-map Fog-of-War exploration bitmap between the client (`0x01B6`) and server.

#### Complete RPC Table — Class `0x0515`

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x01B6`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x85150001` | `1` (`0x01`) | C->S | Packer `0x85150006` (`0x106F9C`) | Called by `0x81B60002` (`0x0F381C`) from `0x140B5FB30` | 1. `Pack_ObjRef` (`fog_of_war_data: SmartBuffer` `0x008E`)<br>2. `Pack_u32` (`map_did: u32`) | **Save Fog-of-War Data:** Sends updated fog-of-war exploration bitmap (`SmartBuffer` `0x008E`) for `map_did` to the server. |
| `0x85150002` | `2` (`0x02`) | C->S | Packer `0x85150005` (`0x106ECC`) | Called by `0x81B60001` (`0x0F37F8`) from `0x81B60008` (`0x0F38D4`) | 1. `Pack_u32` (`map_did: u32`) | **Request Fog-of-War Data:** Requests the fog-of-war exploration bitmap for `map_did` when switching areas/dungeons (`0x81B60008` also calls `native 0x01490003` or `0x01490004` with `Dungeon_MiniMapData` `0x10000DFF` / `Dungeon_MapData` `0x100011A0` / `Area_MapData` `0x100010C8`). |
| `0x85150004` | `4` (`0x04`) | S->C | Handler `0x85150004` (`0x106F24`) | Calls `0x81B60007` (`0x0F383C`) -> `native 0x01490002` (`0x140B40D90` -> `0x140B10350`) | 1. `Unpack_ObjRef` (`fog_of_war_data: SmartBuffer` `0x008E`)<br>2. `Unpack_u32` (`map_did: u32`) | **Receive Fog-of-War Data:** Delivers the fog-of-war `SmartBuffer` (`0x008E`) for `map_did` to `MapSystem::OnRecvFogOfWar` (`vtable+0x40`). |

#### Native `ObjRef` Wire Sub-Framing for `0x85150001` / `0x85150004` (`SmartBuffer` `0x008E`)

Confirmed from `0x140B5FB30` (`0x140B5FB97: mov ecx, 0x8e; call 0x140359e40`), `0x14081E9C9` (`RegisterNativeClassDesc` for `0x008E`), and `[WLibClassDesc + 0x68]` at `0x140838280` -> `0x140367E00` -> `0x140352750`:
- `Pack_ObjRef` (`0x140361630`) / `Unpack_ObjRef` (`0x140364CD0`) writes/reads the WLib object table header (`0x140363520`: `VLE` object count followed by `count` `u32` object IDs) and, for each object (`0x140363A00` / `0x140363B80`), writes/reads the per-object flags header (`0x1403630B0`: masked `u32` flags `obj->flags & 0xBB000000`, with bit 28 `0x10000000` clear for native classes), `u16` `class_id = 0x008E` (LE), and invokes `[WLibClassDesc + 0x68]`:
  1. `u32` `byte_length` (4 bytes LE)
  2. `u8[byte_length]` raw fog-of-war exploration bitmap bytes.


---

### 1.5 Area & Dungeon Discovery (`0x04E3` / `0x04E4` <-> `0x00FF` / `0x00F8` `CAreaSystem`)

#### Complete RPC Table — Classes `0x04E3` & `0x04E4`

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x00FF`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x84E30001` | `1` (`0x01`) | C->S | Packer `0x84E30002` (`0x0F95C4`) | Called by `0x80FF0001` (`0x05B1D8`) | 1. `Pack_ObjRef` (`area_wc_name: WString` `0x00E1`) | **Admin Area Query:** Sent by `/area query <area.wc>`. |
| `0x84E40001` | `1` (`0x01`) | S->C | Handler `0x84E40001` (`0x0F961C`) | Calls `0x80FF0004` (`0x05AC6C`) | 1. `Unpack_ObjRef` (`response_text: WString` `0x00E1`) | **Admin Area Query Response:** Logs response via `native 0x00390001`. |

#### Property-Driven Area & Dungeon Transitions (`0x80F8001A` / `0x80FF0009`)

Normal area and dungeon transitions (and "Area Discovered" banners) do **not** use `0x04E3`/`0x04E4`; they are driven by entity property updates on the local player entity (`0x80F8001A` at `0x059DB0` registers callback `0x80FF0009` at `0x05AE5C`):

| Property ID | Name | Type | Role in `0x80FF0009` (`0x05AE5C`) |
|---|---|---|---|
| `0x00000092` (`146`) | `Area_DID` | `DataID` (`u32`) | Server-updated current Area DID on the player entity. |
| `0x00000625` (`1573`) | `Dungeon_DID` | `DataID` (`u32`) | Server-updated current Dungeon DID on the player entity. |
| `0x0000053B` (`1339`) | `Area_Override_DID` | `DataID` (`u32`) | Server-updated sub-area/override DID on the player entity. |
| `0x10000983` | `Area_CurrentArea` | `DataID` (`u32`) | Client-cached current Area DID updated by `0x80FF0009`. |
| `0x100007AC` | `Dungeon_CurrentDungeon` | `DataID` (`u32`) | Client-cached current Dungeon DID updated by `0x80FF0009`. |
| `0x10000E00` | `Area_DiscoveredDID` | `DataID` (`u32`) | Populated in event `1796` when `0x80FF0003` (`0x05AC98`) fires on entering a new area; `0x80FF0003` also calls `0x81B60006` (`CMapSystem`) and `native 0x01350018`. |

---

### 1.6 Fellowship Member Map Positions & Target Marks (`0x04FE` / `0x0739` <-> `0x0452` / `0x019D` / `0x0453` / `0x0736`)

When the player is in a Fellowship, member world-map positions, dungeon/area locations, and overhead/radar target marks are synchronized via RPC classes `0x04FE` (1278) and `0x0739` (1849):

| `function_id` | `fn` | Dir | Packer / Handler Off | Target (`0x0452` / `0x019D` / `0x0736`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x84FE0020` | `32` (`0x20`) | S->C | `0x0FEB04` | `0x8452004E` (`0x0AFA30`) -> `0x819D001B` (`0x0AB104`) | 1. `Unpack_ObjRef` (`position: Position` `0x01A7`)<br>2. `Unpack_u64` (`member_iid: u64`) | **Fellowship Member Position Update (`MarkerAdded`):** Looks up fellowship member record (`0x84530026`) by `member_iid` and updates its world `Position` (`vcall(slot=5)`), driving the fellowship member pin on the world map and radar. |
| `0x84FE00BA` | `186` (`0xBA`) | S->C | `0x0FF4C4` | `0x8452005B` (`0x0AF908`) -> `0x819D002E` (`0x0AA9B4`) | 1. `Unpack_ObjRef` (`area_or_string_obj`)<br>2. `Unpack_u64` (`member_iid: u64`) | **Fellowship Member Location/Status Update:** Updates slot 3 on the fellowship member record (`0x8453001E`). |
| `0x84FE00B8` | `184` (`0xB8`) | S->C | `0x0FF738` | `0x8452005A` (`0x0AF8E4`) -> `0x819D002D` (`0x0AA964`) | 1. `Unpack_u32` (`key_u32: u32`)<br>2. `Unpack_u32` (`val_u32: u32`)<br>3. `Unpack_u64` (`member_iid: u64`) | **Fellowship Member IntHash Attribute Update:** Stores `(key_u32, val_u32)` in the member's `IntHash` (`0x84530038`) and calls `native 0x013F0011(member_iid, key_u32, val_u32)` (`0x140B472C0` -> `0x140B1C060`). |
| `0x84FE00C6` | `198` (`0xC6`) | S->C | `0x0FE344` | `0x819D0030` (`0x0AC058`) -> `0x87360011` (`0x17453C`) | 1. `Unpack_u32` (`mark_did: u32`)<br>2. `Unpack_u64` (`target_iid: u64`) | **Set Fellowship Target Mark:** Assigns target mark `mark_did` (`0x0735`, `TargetMarking_Icon_DataID` `0x1000169E`, `TargetMarking_ScriptID` `0x100016A4`) to `target_iid` in `0x0741` and plays the overhead target mark FX (`native 0x00B90002`). |
| `0x84FE00C4` | `196` (`0xC4`) | S->C | `0x0FE418` | `0x819D002F` (`0x0AC094`) -> `0x8736000D` (`0x1746CC`) | 1. `Unpack_u32` (`mark_did: u32`)<br>2. `Unpack_u64` (`target_iid: u64`) | **Clear Fellowship Target Mark:** Removes target mark `mark_did` from `target_iid` in `0x0741` and stops the overhead target mark FX (`native 0x00B90004`). |
| `0x87390015` | `21` (`0x15`) | C->S | Packer `0x87390016` (`0x10D894`) | Called by `0x87360010` (`0x1744E4`) | 1. `Pack_u32` (`mark_did: u32`)<br>2. `Pack_u64` (`target_iid: u64`) | **Request Set Fellowship Target Mark:** Requests setting mark `mark_did` (from `0x0734` array, indices `0..9`) on `target_iid` (rate-limited by `Antispam_TargetMarkingRequest` `0x10001C56`). |
| `0x87390011` | `17` (`0x11`) | C->S | Packer `0x87390014` (`0x10D8FC`) | Called by `0x87360005` (`0x174674`) | 1. `Pack_u32` (`mark_did: u32`)<br>2. `Pack_u64` (`target_iid: u64`) | **Request Clear Fellowship Target Mark:** Requests clearing mark `mark_did` from `target_iid`. |

---

### 1.7 Quest Guide & Quest Tracker Waypoints (`0x0521` <-> `0x011F` / `0x06D9` / `0x0575`)

Quest Guide map/radar markers (objective rings, point markers, and region highlights) are computed by `CQuestJournal` (`0x06D9`, parent `0x0575`) and `CQuestSystem` (`0x011F`, parent `0x057D`) from active quest objectives and dynamic quest event updates:

| `function_id` | `fn` | Dir | Packer / Handler Off | Target (`0x011F` / `0x06D9` / `0x0575`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x85210051` | `81` (`0x51`) | S->C | `0x10B85C` | `0x144080` -> `0x86D9004D` (`0x13F318`) | 1. `Unpack_ObjRef` (`event_data`)<br>2. `Unpack_u32` (`objective_idx: u32`)<br>3. `Unpack_u32` (`quest_did: u32`) | **Update Quest Objective Dynamic Event:** Updates dynamic event state on `(quest_did, objective_idx)` (`vcall(slot=72)`) and refreshes Quest Guide map/radar markers. |
| `0x85210075` | `117` (`0x75`) | S->C | `0x10B744` | `0x1440B4` -> `0x86D9001F` (`0x13F34C`) | 1. `Unpack_ObjRef` (`event_data`)<br>2. `Unpack_u32` (`objective_idx: u32`)<br>3. `Unpack_u32` (`quest_did: u32`) | **Update Quest Objective Event List (Slot 32):** Updates `(quest_did, objective_idx)` (`vcall(slot=32)`). |
| `0x85210126` | `294` (`0x126`) | S->C | `0x10B7D0` | `0x1440E8` -> `0x86D900A3` (`0x13F380`) | 1. `Unpack_ObjRef` (`event_data`)<br>2. `Unpack_u32` (`objective_idx: u32`)<br>3. `Unpack_u32` (`quest_did: u32`) | **Update Quest Objective Event List (Slot 99):** Updates `(quest_did, objective_idx)` (`vcall(slot=99)`). |
| `0x852100B5` | `181` (`0xB5`) | S->C | `0x10A954` | `0x143FFC` -> `0x86D90063` (`0x13999C`) | 1. `Unpack_ObjRef` (`event_counts_map: IntHash` `0x000B`)<br>2. `Unpack_u64` (`entity_iid: u64`) | **Update Entity Quest Event Map:** Associates `entity_iid` with `event_counts_map` in `0x06D9` `attr(7)` (`LongHash` `0x0075`) and notifies UI Quest Guide listeners via `native 0x0159000D` / `0x0159000B`. |
| `0x852100CE` | `206` (`0xCE`) | S->C | `0x10A5B0` | `0x144AA4` -> `0x85750026` (`0x1471D4`) | 1. `Unpack_u32` (`delta: u32`)<br>2. `Unpack_u32` (`event_did: u32`)<br>3. `Unpack_u32` (`quest_did: u32`) | **Increment Quest Event Counter:** Adds `delta` to `(quest_did, event_did)` in `0x0575` `attr(0)` and plays sound `122`. |
| `0x852100CF` | `207` (`0xCF`) | S->C | `0x10AE3C` | `0x144AE8` -> `0x85750025` (`0x147280`) | 1. `Unpack_u32` (`delta: u32`)<br>2. `Unpack_u32` (`event_did: u32`)<br>3. `Unpack_u32` (`quest_did: u32`) | **Decrement Quest Event Counter:** Subtracts `delta` from `(quest_did, event_did)` in `0x0575` `attr(0)`. |
| `0x852100D2` | `210` (`0xD2`) | S->C | `0x10A898` | `0x1449D0` -> `0x85750026` (`0x1471D4`) | 1. `Unpack_ObjRef` (`quest_events_hash: IntHash` `0x0023`) | **Sync All Quest Event Counters:** Iterates outer `IntHash` (`quest_did` -> inner `IntHash` of `event_did` -> `count`) and populates `0x0575` `attr(0)`. |
| `0x852100D5` | `213` (`0xD5`) | S->C | `0x10B2C4` | `0x811F007F` (`0x1433F4`) | *(0 arguments)* | **Refresh Quest Tracker Arrays:** Re-reads player properties `Quest_TrackerArray` (`0x10000E6B`) and `Quest_TrackerArrayLocked` (`0x10000291`), updating tracked quest waypoints on the map and radar. |
| `0x852100D6` | `214` (`0xD6`) | C->S | Packer `0x852100D7` (`0x10B8E8`) | Called by Quest Tracker UI | 1. `Pack_u32` (`quest_did: u32`)<br>2. `Pack_u32` (`tracker_action_or_slot: u32`) | **Update Quest Tracker Entry:** Adds, removes, or locks a quest in the server-persisted Quest Tracker array. |

Key DAT/entity properties for Quest Waypoints and Map Notes:
- `0x1000038E` (`QuestEvent_LandmarkDID`): `DataID` of the target landmark (`0x0110`) for a quest objective event.
- `0x10001139` (`QuestEvent_LandmarkRegion`): Region polygon/area identifier for a quest objective event.
- `0x100001E6` (`Landmark_DataID`): `DataID` on a landmark entity.
- `0x100002C9` (`Landmark_QuestOnly`): `bool` indicating a landmark is only displayed when referenced by an active quest objective.
- `0x10000E6B` (`Quest_TrackerArray`): `Array` on the player entity listing currently tracked quest DIDs.
- `0x10000291` (`Quest_TrackerArrayLocked`): `bool` on the player entity indicating whether the quest tracker is locked.
- `0x10000071` (`UI_Map_MapNoteImage`): Icon DID for a map note.
- `0x10000C37` (`EffectGenerator_EffectData_MapNoteAdd`): Map note effect data (`0x81CC0017` at `0x189844`).

---

## 2. Stable-Master Travel, Milestone, Recall, & Summon

### 2.1 Subsystem Architecture & Domain Mapping

- **`WorldEvent::TravelShown`:** Triggered when the client interacts with a Stable-Master NPC (sending C->S `0x85330008` with `npc_iid`) and receives S->C `0x85330002` (`npc_iid`), which invokes `0x81CC0015` -> `native 0x01650001` (`Travel::OpenWindow`, VA `0x140B08460`). The UI then queries `0x81CC0001` (`TravelInqDestinations`, `0x189C18`) and `0x81CC0003` (`0x189CD0`), which resolve the NPC's `TravelWebWC` (`0x10000F66`) against client DAT records (`TravelWeb` `0x05E4`, `TravelDestination` `0x05DB`, `TravelRoute` `0x05E1`) and the player's discovered locations (`0x85330006` / `0x8533000B` / `0x8533001B`) to populate native route info objects (`0x0E60`).
- **`WorldCommand::ChooseTravel`:** Sends C->S `0x8533000D` (`Pack_u32(route_did)`, `Pack_u64(npc_iid)`) via `0x85330016` (`0x1102C8`), called by `0x81CC0013` (`0x1892A0`) / `0x81CC0004` (`0x189D10`).
- **`WorldEvent::TravelStarted` & `WorldEvent::TravelEnded`:** Driven by player entity property updates on `Travel_IsTraveling` (`0x100009E8`), `Travel_RouteDID` (`0x10001B2B`), `Travel_Allow_Dismount` (`0x10004031`), and `Mount_Is_Mounted` (`0x10000AAF`).
- **Dismount / End Travel Early:** Sends C->S `0x85330005` (0 arguments) via `0x85330012` (`0x110228`), called by `0x81CC0009` (`0x1895E4`).

---

### 2.2 Travel System (`0x0533` <-> `0x01CC` / `0x05E3` `CTravelSystem`)

#### Complete RPC Table — Class `0x0533` (1331)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x01CC`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x85330002` | `2` (`0x02`) | S->C | Handler `0x85330002` (`0x1101C8`) | Calls `0x81CC0015` (`0x189994`) -> `native 0x01650001` (`0x140B3BBF0` -> `0x140B08460`) | 1. `Unpack_u64` (`npc_iid: u64`) | **Open Stable-Master Travel Window (`TravelShown`):** Opens `Travel_Window` for the Stable-Master NPC `npc_iid`. |
| `0x85330005` | `5` (`0x05`) | C->S | Packer `0x85330012` (`0x110228`) | Called by `0x81CC0009` (`0x1895E4`) | *(0 arguments)* | **End Travel Early (Dismount Travel Route):** Requests immediate termination of an active ridden travel route when `Travel_IsTraveling` (`0x100009E8`) and `Travel_Allow_Dismount` (`0x10004031`) are true (rate-limited by `Antispam_TravelRequest` `0x10001C52`). |
| `0x85330006` | `6` (`0x06`) | S->C | Handler `0x85330006` (`0x1100F0`) | Calls `0x81CC000B` (`0x189598`) -> `0x81CC0010` (`0x189930`) -> `native 0x01650003` (`0x140B447C0`) | 1. `Unpack_ObjRef` (`discovered_locations: IntSet/List` `0x0019`) | **Sync Discovered Travel Locations:** Replaces the player's known Stable-Master location DIDs (`vcall(slot=526)`) and notifies the Travel UI (`native 0x01650003`). |
| `0x85330008` | `8` (`0x08`) | C->S | Packer `0x85330014` (`0x110078`) | Called by `0x81CC0014` (`0x1897D8`) from `0x81CC0002` (`0x189C68`) and `0x81CC000F` (`0x189710`) | 1. `Pack_u32` (`flags_or_mode: u32`)<br>2. `Pack_u32` (`is_admin_discover: u32`)<br>3. `Pack_u64` (`npc_iid: u64`) | **Interact with Stable-Master / Discover NPC Locations:** In normal NPC interaction (`0x81CC0002`), sends `(arg0, 0, npc_iid)`. In admin `/travel discoverNPClocations` (`0x81CC000F`), sends `(1, 1, target_iid)`. |
| `0x8533000B` | `11` (`0x0B`) | S->C | Handler `0x8533000B` (`0x110004`) | Calls `0x81CC000D` (`0x189948`) -> `0x81CC0010` (`0x189930`) -> `native 0x01650003` | 1. `Unpack_u32` (`is_added: u32`)<br>2. `Unpack_u32` (`location_did: u32`) | **Add or Remove Discovered Travel Location:** If `is_added != 0`, adds `location_did` to the player's discovered locations (`0x85E3001D`); if `0`, removes it (`0x85E3001E`); then refreshes the Travel UI (`native 0x01650003`). |
| `0x8533000D` | `13` (`0x0D`) | C->S | Packer `0x85330016` (`0x1102C8`) | Called by `0x81CC0013` (`0x1892A0`) from `0x81CC0004` (`0x189D10`) | 1. `Pack_u32` (`route_did: u32`)<br>2. `Pack_u64` (`npc_iid: u64`) | **Purchase / Select Travel Route (`ChooseTravel`):** Validates route requirements (`0x85E3000C` at `0x18AEE8`). If the player is currently mounted (`Mount_Is_Mounted` `0x10000AAF`), `0x81CC0013` first calls `0x89A50010` (dismount) before sending `0x8533000D(route_did, npc_iid)`. Note: if purchasing via Mithril Coins (`use_paper_item != 0` in `0x81CC0004`), constructs a `PaperItemService` (`0x0F62`) with `service_type = 2`, `route_did`, and `npc_iid` and invokes `0x8C0C0015` (`0x093560`). |
| `0x8533001B` | `27` (`0x1B`) | S->C | Handler `0x8533001B` (`0x110154`) | Calls `0x81CC0017` (`0x189844`) | 1. `Unpack_u32` (`location_did: u32`)<br>2. `Unpack_u64` (`npc_iid: u64`) | **Discover Stable-Master Location on NPC Interaction:** Adds `location_did` to the player's discovered travel locations (`0x85E3001D`), displays the chat discovery message for `TravelDisplayName` (`0x85DB0005`, `0x10001418`), and updates the NPC's overhead indicator (`EffectGenerator_EffectData_MapNoteAdd` `0x10000C37`). |
| `0x85330022` | `34` (`0x22`) | C->S | Packer `0x85330023` (`0x110270`) | Called by `0x81CC001E` (`0x189640`) | 1. `Pack_u64` (`leader_iid: u64`) | **Travel to Fellowship Leader:** Requests teleportation to the fellowship leader `leader_iid` (checked via `0x85E30023`). |
| `0x85330025` | `37` (`0x25`) | S->C | Handler `0x85330025` (`0x10FEDC`) | Calls `vcall(slot=852)` on player | 1. `Unpack_ObjRef` (`arg0_obj`)<br>2. `Unpack_u32` (`arg1_u32: u32`) | **Update Player Travel Slot 852:** Stores travel state data on player slot 852. |
| `0x85330026` | `38` (`0x26`) | S->C | Handler `0x85330026` (`0x10FE7C`) | Calls `vcall(slot=851)` on player | 1. `Unpack_u32` (`arg0_u32: u32`) | **Update Player Travel Slot 851:** Stores travel state `u32` on player slot 851. |

#### Route Discovery, Requirements, & Cost Calculation (`0x05E3` / `0x05E4` / `0x05DB` / `0x05E1` -> Native `0x0E60`)

When `TravelShown` (`0x85330002`) fires, the UI calls `0x81CC0001` (`TravelInqDestinations`, `0x189C18` -> `0x85E30015` at `0x18B73C`) and `0x81CC0003` (`0x189CD0` -> `0x85E30028` at `0x18BC7C`):
1. Reads `TravelWebWC` (`0x10000F66`) on the Stable-Master NPC entity (`npc_iid`) to obtain the `TravelWeb` (`0x05E4`, class 1508) and its `TravelHomeLocation` (`0x10001502`, a `TravelDestination` `0x05DB`, class 1499).
2. Iterates the `TravelRoute` (`0x05E1`, class 1505) entries connected to the NPC's `TravelHomeLocation`:
   - Route destination: `TravelRouteDestination` (`0x10001501`) -> `TravelDestination` (`0x05DB`).
   - Route base cost: `TravelRouteCost` (`0x10001503`, `i32` copper coins) multiplied by the NPC's `TravelWebSellMultiplier` (`0x10001419`, `f32`, default `1.0`) and discounted by the player's `Travel_DiscountArray` (`0x1000064A`) / `Discount_Travel` (`0x100027EE`) / faction-specific `Discount_Travel_*` properties (`0x85E30005` at `0x18B2E8`).
   - Route actions/hops: `TravelRouteActionArray` (`0x10001508`), containing `TravelRouteAction` (`0x10001505`) and `TravelSegment` (`0x10001509`).
   - Swift Travel flag: `TravelDestinationIsSwiftTravel` (`0x10005257`, `bool`).
   - Mithril Coin / Paper Item cost: `TravelDestinationAllowPaperItems` (`0x1000511C`, `bool`) and `TravelRoute_NonSubscriberTemporaryServiceRequired` (`0x10005EC5`, `i32`).
   - Route eligibility (`0x85E3000C` at `0x18AEE8` and `0x85E30028` at `0x18BC7C`):
     - Checks whether the destination `location_did` is in the player's discovered locations set (`vcall(slot=525)`), unless `Travel_BypassUsageChecks` (`0x10003919`) is set.
     - Checks usage requirements on the `TravelRoute` (`0x05E1`) and `TravelDestination` (`0x05DB`) (min level `Usage_MinLevel` `0x100000C4`, quest/deed requirements, subscriber/allegiance checks).
   - Populates native `0x0E60` (`TravelRouteInfo`, 88 bytes, VA `0x140A85E20`..`0x140A83A30`) via setters `0x0E600001`..`0x0E60000E` (`route_did`, `dest_did`, `display_name`, `is_swift_travel`, `area_name`, `territory_name`, `coin_cost`, `requirements_met`, `unmet_requirements_tooltip`, `mithril_cost`, etc.).

#### Travel State Properties on the Player Entity

| Property ID | Name | Type | Semantics |
|---|---|---|---|
| `0x100009E8` | `Travel_IsTraveling` | `bool` | Set to `1` when a travel route starts (`TravelStarted`) and `0` when the player arrives or dismounts early (`TravelEnded`). |
| `0x10001B2B` | `Travel_RouteDID` | `DataID` (`u32`) | DataID of the active `TravelRoute` (`0x05E1`) while traveling. |
| `0x10004031` | `Travel_Allow_Dismount` | `bool` | Whether the active travel route allows early dismount via C->S `0x85330005` (`0x81CC000C` at `0x189D68`). |
| `0x1000064A` | `Travel_DiscountArray` | `Array` | Array of active travel discount property IDs on the player. |
| `0x100027EE` | `Discount_Travel` | `f32` | Global travel cost discount multiplier on the player. |

---

### 2.3 Recall / Milestone System (`0x0524` <-> `0x06DB` / `0x0628` / `0x0C47`) & Summon System (`0x052A` <-> `0x06DD`)

#### Complete RPC Table — Class `0x0524` (1316, Recall & Milestone)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x06DB` / `0x0628` / `0x0C47`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x85240001` | `1` (`0x01`) | C->S | Packer `0x85240008` (`0x10C584`) | Called by `0x86DB0005` (`0x162BE4`) | 1. `Pack_u64` (`target_iid: u64`) | **Admin Recall Query:** Sent by `/recall query`. |
| `0x85240002` | `2` (`0x02`) | C->S | Packer `0x85240006` (`0x10C3A4`) | Called by `0x86DB0001` (`0x162FE8`) | 1. `Pack_u32` (`milestone_skill_did: u32`) | **Bind Milestone:** Confirms binding a milestone skill (`Recall_MilestoneSkill` `0x10003C9E` or `0` for primary milestone) to the currently interacted milestone (`0x0628` / `0x0C47`) after accepting the confirmation dialog (`0x86EC0001` at `0x162AD0`). |
| `0x85240007` | `7` (`0x07`) | S->C | Handler `0x85240007` (`0x10C524`) | Calls `0x86DB0007` (`0x162C28`) | 1. `Unpack_ObjRef` (`msg_text: WString`) | **Recall System Message:** Displays chat message on channel `4`. |
| `0x8524000C` | `12` (`0x0C`) | C->S | Packer `0x8524000D` (`0x10C3FC`) | Called by `0x86DB000C` (`0x163168`) | 1. `Pack_u32` (`telepad_did: u32`)<br>2. `Pack_u32` (`slot_index: u32`) | **Set Recall Telepad Slot:** Sets a recall telepad DID (`Recall_Proficiency` `0x100013C4` / `Recall_LinkArray` `0x10003681`) at `slot_index`. |
| `0x8524000E` | `14` (`0x0E`) | C->S | Packer `0x85240011` (`0x10C464`) | Called by `0x86280006` (`0x1A2098`) and `0x8C470005` (`0x1A0254`) | 1. `Pack_u32` (`landmark_did: u32`) | **Discover Milestone Landmark:** Notifies the server that the player discovered the milestone's landmark (`landmark_did`) upon interacting with the milestone object. |
| `0x85240012` | `18` (`0x12`) | C->S | Packer `0x85240013` (`0x10C4BC`) | Called by `0x86DB000F` (`0x163284`) | 1. `Pack_u32` (`link_did: u32`)<br>2. `Pack_u32` (`arg1_u32: u32`) | **Update Milestone Link:** Updates milestone link configuration (`Recall_LinkArray` `0x10003681`). |
| `0x85240018` | `24` (`0x18`) | S->C | Handler `0x85240018` (`0x10C340`) | Calls `0x86DB0010` (`0x1632D8`) | 1. `Unpack_ObjRef` (`link_array_obj`) | **Sync Milestone Link Array:** Updates the player's milestone link array (`vcall(slot=821)`). |

#### Complete RPC Table — Class `0x052A` (1322, Player Summon / Horn of Gondor / Muster)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x06DD`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x852A0003` | `3` (`0x03`) | S->C | Handler `0x852A0003` (`0x10CFD8`) | Calls `0x86DD0005` (`0x164E64`) | 1. `Unpack_u32` (`is_combat_summon: u32`)<br>2. `Unpack_ObjRef` (`destination_name: StringInfo` `0x00C7`)<br>3. `Unpack_u64` (`summoner_iid: u64`) | **Receive Summon Invitation:** Opens the 30-second Summon confirmation dialog (`0x00070001`). |
| `0x852A0004` | `4` (`0x04`) | C->S | Packer `0x852A0006` (`0x10CF80`) | Called by `0x86DD0006` (`0x165160`) | 1. `Pack_u32` (`accepted_bool: u32`) | **Respond to Summon Invitation:** Sends `1` to accept the summon or `0` to decline. |

---

## 3. Mounts, War-Steeds, Mounted Combat, & Mount Outfits

### 3.1 Subsystem Architecture & Domain Mapping

- **Summoning a Mount or War-Steed:** Mounts in LOTRO are skills (`Skill_Category` = Mount / War-Steed). Summoning a mount uses the standard skill execution command (`WorldCommand::UseSkill`).
- **Dismounting (`WorldCommand::Dismount`):** Sends C->S `0x88000006` (0 arguments) via packer `0x88000007` (`0x10705C`), called by `0x89A50001` (`0x117E38`) and `0x89A50010` (`0x117BF8`).
- **`WorldEvent::MountChanged`:** Driven by property updates on the rider entity (`0x89A0000D` at `0x118BD0` registers callback `0x89A5000E` at `0x117EC8`):
  - `Mount_Is_Mounted` (`0x10000AAF`, `bool`): `1` when mounted, `0` when unmounted.
  - `Mount_CurrentMountType` (`0x10002A90`, `u32` bitmask): bitmask of the active mount type. Bitmask `(mount_type & 0x38) != 0` (`56`: Light `0x08`, Medium `0x10`, Heavy `0x20`) identifies a **Combat Mount (War-Steed)** (`0x89A00014` at `0x118FB8`); bit `0x01`/`0x02`/`0x04`/`0x40` identifies a standard/session/goat mount.
  - `Mount_Cache_MountEffectDID` (`0x10002A79`, `DataID` `u32`): DataID of the active mount effect (`0x099B`, which carries `Mount_DataID` `0x10001715`, `Mount_Species` `0x100022B9`, `Mount_MountAppearance` `0x100040B5`, `Mount_RiderType` `0x1000406B`).
  - `Mount_Cache_MountEffectID` (`0x10002A39`, `u32`): Active effect instance ID on the rider.
  - `Mount_CurrentMountStatus` (`0x10002A3A`, `u32`): Current mount state enum.
  - `Mount_MountIID` (`0x10001716`, `u64` `InstanceID`) / `Mount_ClientUI_Entity` (`0x10004C45`, `u64` `InstanceID`).
- **`WorldEvent::MountVitalChanged`:** Driven by property updates on the rider entity for both standard mounts (Durability) and War-Steeds (Endurance, Power, Momentum/Fury, Speed/Gear, Agility/Strength/Armor).

---

### 3.2 Mount System (`0x0800` <-> `0x09A5` / `0x054E` / `0x09A0` `CMountSystem`)

#### Complete RPC Table — Class `0x0800` (2048)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller (`0x09A5`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x88000006` | `6` (`0x06`) | C->S | Packer `0x88000007` (`0x10705C`) | Called by `0x89A50001` (`0x117E38`) and `0x89A50010` (`0x117BF8`) | *(0 arguments)* | **Dismount (`WorldCommand::Dismount`):** Requests dismounting from the current standard mount or war-steed (`0x89A50010` first checks `0x89A00013` / `Mount_Is_Mounted` `0x10000AAF` and rate-limiter `0x89A00001` `Antispam_MountRequest` `0x10001C50`). |
| `0x88000008` | `8` (`0x08`) | C->S | Packer `0x88000009` (`0x1070A4`) | Called by `0x89A50004` (`0x1179D4`) | 1. `Pack_u32` (`mount_category_or_type: u32`)<br>2. `Pack_ObjRef` (`new_name: WString` `0x00E1`) | **Rename Mount:** Renames the player's mount (`mount_category_or_type`: `1` = standard mount `Mount_PetName` `0x10002B8A`, `2` = war-steed `Mount_CombatPetName` `0x10004E09`, validated via `native 0x00320001`). |

---

### 3.3 Mounted Combat System (`0x0DA7` <-> `0x0D8E` / `0x0D6F` `CMountedCombatSystem`)

RPC class `0x0DA7` (3495) manages War-Steed trait trees, trait point allocation/refunds, and admin mounted combat commands.

#### Complete RPC Table — Class `0x0DA7` (3495)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller (`0x0D8E`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x8DA70001` | `1` (`0x01`) | C->S | Packer `0x8DA70003` (`0x0FA9B8`) | Called by `0x8D8E000A` (`0x11A0E8`) | 1. `Pack_ObjRef` (`cmd_args: WString`) | **Admin Mounted Combat Command:** Sent by `/mc` admin subcommands. |
| `0x8DA70007` | `7` (`0x07`) | C->S | Packer `0x8DA70009` (`0x0FAA10`) | Called by `0x8D8E0001` (`0x119AD4`) | *(0 arguments)* | **Refund / Clear War-Steed Traits:** Requests resetting/refunding spent War-Steed trait points (checks `0x8D6F0007`, coin cost via `0x8D6F0012` / `MountedCombat_Control_CoinRefundMultiplier` `0x10004DF1`, and `Antispam_MountedCombatRequest` `0x10004DE5`). |
| `0x8DA7000A` | `10` (`0x0A`) | C->S | Packer `0x8DA7000B` (`0x0FAA58`) | Called by `0x8D8E0002` (`0x1197E4`) | 1. `Pack_ObjRef` (`trait_rank_map: IntHash` `0x000B`) | **Spend War-Steed Trait Points:** Submits a map of `(trait_property_id -> desired_rank)` to spend War-Steed trait points (`Trait_MountedCombat_TotalPoints` `0x1000497F`, `Trait_MountedCombat_TotalSpentPoints` `0x1000497E`). |
| `0x8DA70010` | `16` (`0x10`) | C->S | Packer `0x8DA70012` (`0x0FAAB0`) | Called by `0x8D8E0005` (`0x119C0C`) | *(0 arguments)* | **Request War-Steed Trait Tree Respec (Mithril / Store):** Requests trait tree respec. |
| `0x8DA70013` | `19` (`0x13`) | C->S | Packer `0x8DA70014` (`0x0FAAF8`) | Called by `0x8D8E0006` (`0x119BEC`) | 1. `Pack_u32` (`spec_tree_index: u32`) | **Select War-Steed Trait Specialization:** Selects or unlocks a War-Steed specialization tab. |

---

### 3.4 Mount Saves / War-Steed Cosmetic Outfits (`0x0E15` <-> `0x0E16`)

RPC class `0x0E15` (3605) manages War-Steed cosmetic appearance outfit slots (`0x0E16`).

#### Complete RPC Table — Class `0x0E15` (3605)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x0E16`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x8E150001` | `1` (`0x01`) | C->S | Packer `0x8E150002` (`0x0FFEDC`) | Called by `0x8E160004` (`0x17AA78`) | 1. `Pack_u32` (`slot_index: u32`) | **Select War-Steed Cosmetic Outfit Slot:** Switches active War-Steed appearance to `slot_index` (`Mount_Saves_CurrentSelection` `0x10004D48`, rate-limited by `Antispam_MountSavesRequest` `0x10004EC9`). |
| `0x8E150003` | `3` (`0x03`) | C->S | Packer `0x8E150004` (`0x0FFE14`) | Called by `0x8E160005` (`0x17AAA0`) | 1. `Pack_ObjRef` (`appearance_map: IntHash`)<br>2. `Pack_u32` (`slot_index: u32`) | **Save War-Steed Cosmetic Outfit Slot:** Saves War-Steed cosmetic appearance and dye selections (`0x10006C1D`..`0x10006C25`) into `slot_index`. |
| `0x8E150007` | `7` (`0x07`) | S->C | Handler `0x8E150007` (`0x0FFCC0`) | Calls `0x8E160009` (`0x17A960`) -> `native 0x0E220001` | 1. `Unpack_ObjRef` (`slot_data_obj`)<br>2. `Unpack_u32` (`slot_index: u32`) | **Update War-Steed Outfit Slot:** Updates saved War-Steed outfit slot `slot_index` (`vcall(slot=848)`) and notifies the UI (`native 0x0E220001`). |
| `0x8E150009` | `9` (`0x09`) | S->C | Handler `0x8E150009` (`0x0FFD9C`) | Calls `0x8E16000B` (`0x17A99C`) -> `native 0x0E220003` | 1. `Unpack_ObjRef` (`all_slots_obj`) | **Sync All War-Steed Outfit Slots:** Replaces all saved War-Steed outfit slots (`vcall(slot=846)`) and notifies the UI (`native 0x0E220003`). |
| `0x8E15000B` | `11` (`0x0B`) | S->C | Handler `0x8E15000B` (`0x0FFD3C`) | Calls `0x8E16000C` (`0x17A980`) -> `native 0x0E220002` | 1. `Unpack_u32` (`slot_index: u32`) | **Confirm Active War-Steed Outfit Slot:** Updates `Mount_Saves_CurrentSelection` (`vcall(slot=850)`) and notifies the UI (`native 0x0E220002`). |
| `0x8E15000E` | `14` (`0x0E`) | C->S | Packer `0x8E150011` (`0x0FFE7C`) | Called by `0x8E160008` (`0x17AB28`) | 1. `Pack_u32` (`is_hidden_bool: u32`)<br>2. `Pack_u32` (`piece_slot: u32`)<br>3. `Pack_u32` (`outfit_slot_index: u32`) | **Set War-Steed Cosmetic Piece Visibility:** Toggles visibility of War-Steed gear piece `piece_slot` (`1..6`) in `outfit_slot_index`. |
| `0x8E15000F` | `15` (`0x0F`) | C->S | Packer `0x8E150010` (`0x0FFF34`) | Called by `0x8E160002` (`0x17AA08`) | *(0 arguments)* | **Query War-Steed Outfit Slots:** Requests synchronization of War-Steed cosmetic outfit slots. |
| `0x8E150012` | `18` (`0x12`) | C->S | Packer `0x8E150013` (`0x0FFF7C`) | Called by `0x8E16000D` (`0x17A9B8`) | *(0 arguments)* | **Purchase Additional War-Steed Outfit Slot:** Opens/initiates store unlock for an extra War-Steed outfit slot (`Mount_Saves_NumAvailable` `0x10004D49` vs `Mount_Saves_Control_MaxSlots` `0x10004E2A`). |

---

### 3.5 Complete Mount & War-Steed Entity Property Table (`MountChanged` & `MountVitalChanged`)

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

## 4. Instance Finder (`WorldJoin` & `FellowshipMaker`), Private Encounters, & Skirmishes

### 4.1 Subsystem Architecture & Domain Mapping

LOTRO's Instance Finder panel integrates four cooperating WLib RPC systems:

1. **World Join (`0x0A2C` RPC <-> `0x0A2B` / `0x0A2A` `CWorldJoinSystem`):** The "Specific" / direct Instance Join tab where a solo player or fellowship leader selects a specific `PrivateEncounterTemplate` DID, group size, difficulty tier, and level, and launches or responds to a World Join ready-check invitation (`WorldJoinRequest` native class `0x0A44`, `WorldJoinInvite` native class `0x0A39`).
   - Opening the Instance Finder panel sends C->S `0x8A2C0001` (0 args); the server replies with S->C `0x8A2C0003` (`Unpack_ObjRef`), which invokes `0x8A2B0003` -> `native 0x0A2D0001` (`WorldJoin::OnRecvAvailableEncounters`, VA `0x140B45DD0`) -> **`WorldEvent::InstanceListShown`**.
   - Launching a specific instance (`WorldCommand::JoinInstance`) constructs a `WorldJoinRequest` (`0x0A44`), populates it via `0x0A440001` (`template_did`), `0x0A440002` (`group_size`), `0x0A440003` (`level`), and `0x0A440004` (`difficulty_tier`), and sends C->S `0x8A2C0005` (`Pack_ObjRef(request)`).
2. **Fellowship Maker (`0x0C41` RPC <-> `0x0C3D` / `0x0C3B` `CFellowshipMakerSystem`):** The matchmaking queue system for random or filtered multi-encounter queues.
   - Joining a specific-encounter matchmaking queue sends C->S `0x8C410055` (`Pack_ObjRef(encounter_config_map)`, `Pack_u32(config_flags)`, `Pack_u32(roles_mask)`).
   - Joining a random matchmaking queue sends C->S `0x8C410056` (`Pack_u32(config_flags)`, `Pack_u32(roles_mask)`).
   - Leaving the matchmaking queue (**`WorldCommand::LeaveQueue`**) sends C->S `0x8C41000D` (0 args) (or C->S `0x8A2C0007` to cancel a pending WorldJoin request).
   - Queue state transitions and ready checks (**`WorldEvent::InstanceQueueChanged`**) are driven by player property `FellowshipMaker_PlayerState` (`0x10003DCA`) and S->C RPCs `0x8C410028`, `0x8C410034`, `0x8C410037`, `0x8C410049`, and `0x8A2C000B` (`WorldJoinInvite`).
3. **Private Encounter System (`0x051F` RPC <-> `0x011C` / `0x01C1` / `0x01C2` / `0x06E9`):** Active instance lifecycle, objectives, quests, and instance exit.
   - Entering and leaving an active instance (**`WorldEvent::InstanceChanged`**) are signalled by S->C `0x851F000C` (`Unpack_u32(template_did)` -> `0x811C002D` -> `native 0x01570001` `OnEnterEncounter`) and S->C `0x851F0027` (0 args -> `0x811C0071` -> `native 0x01570002` `OnLeaveEncounter`), alongside player property `PrivateEncounter_Template_DataID` (`0x100007AB`).
   - Leaving an active instance (**`WorldCommand::LeaveInstance`**) calls `0x811C0013` (`0x127644`, which opens confirmation dialog `0x06E9` if in an active instance) -> `0x811C0014` (`0x1276B8`), which sends C->S `0x851F0136` (0 args, via packer `0x851F0137` at `0x109FE0`).
4. **Skirmish System (`0x09EC` RPC <-> `0x0A52` / `0x0A6C` `CSkirmishSystem`) & Instance Milestones (`0x083A` <-> `0x0837`):** Skirmish statistics and instance checkpoint milestones.

---

### 4.2 World Join System (`0x0A2C` <-> `0x0A2B` / `0x0A2A` `CWorldJoinSystem`)

#### Complete RPC Table — Class `0x0A2C` (2604)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x0A2B`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x8A2C0001` | `1` (`0x01`) | C->S | Packer `0x8A2C0002` (`0x105BA0`) | Called by `0x8A2B0001` (`0x1C2840`) | *(0 arguments)* | **Query Available World Join Encounters:** Sent when opening the Instance Join panel (rate-limited by `Antispam_WorldJoinRequest` `0x10002ED1`). |
| `0x8A2C0003` | `3` (`0x03`) | S->C | Handler `0x8A2C0003` (`0x105EE0`) | Calls `0x8A2B0003` (`0x1C2A18`) -> `native 0x0A2D0001` (`0x140B45DD0`) | 1. `Unpack_ObjRef` (`available_encounters_list`) | **Receive Available World Join Encounters (`InstanceListShown`):** Delivers the list of available encounters (each entry 136 bytes in `0x140B45E65`, unpacked via `0x140AB4770`: `template_did: u32`, `category_or_flags: u32`, valid group sizes, valid difficulty tiers, min/max level, lock flags) to `WorldJoin::OnRecvAvailableEncounters`. |
| `0x8A2C0005` | `5` (`0x05`) | C->S | Packer `0x8A2C0006` (`0x105EC8`) | Called by `0x8A2B0004` (`0x1C2880`) | 1. `Pack_ObjRef` (`request: WorldJoinRequest` `0x0A44`) | **Submit World Join Request (`JoinInstance`):** Requests launching or queuing for the configured `WorldJoinRequest` (`0x0A44`: `template_did`, `group_size`, `level`, `difficulty_tier`). |
| `0x8A2C0007` | `7` (`0x07`) | C->S | Packer `0x8A2C0008` (`0x105DF8`) | Called by `0x8A2B0006` (`0x1C291C`) | *(0 arguments)* | **Cancel World Join Request (`LeaveQueue`):** Cancels the player's pending World Join request. |
| `0x8A2C0009` | `9` (`0x09`) | C->S | Packer `0x8A2C000A` (`0x105D50`) | Called by `0x8A2B0007` (`0x1C295C`) | 1. `Pack_ObjRef` (`invite: WorldJoinInvite` `0x0A39`) | **Respond to World Join Invitation:** Sends the player's response (`0x0A390005(accepted_bool)` on `0x0A39`, where `1` = accept/travel now, `0` = decline/cancel) to a pending `WorldJoinInvite`. |
| `0x8A2C000B` | `11` (`0x0B`) | S->C | Handler `0x8A2C000B` (`0x105CF4`) | Calls `0x8A2B000B` (`0x1C2A30`) -> `native 0x0A2D0002` (`0x140B347D0` -> `0x140AFA9A0`) | 1. `Unpack_ObjRef` (`invite: WorldJoinInvite` `0x0A39`) | **Receive World Join Invitation (`InstanceQueueChanged`):** Delivers a `WorldJoinInvite` (`0x0A39`) to the client (`WorldJoin::OnRecvInvite`, `vtable+0x00`), prompting the player to confirm entry into the instance. |
| `0x8A2C002F` | `47` (`0x2F`) | C->S | Packer `0x8A2C0030` (`0x105BE8`) | Called by `0x8A2B0010` (`0x1C2A80`) | 1. `Pack_ObjRef` (`request: WorldJoinRequest` `0x0A44`) | **Query Encounter Lock / Raid Lock Status:** Requests lock/save status for `request` (`0x0A44`). |
| `0x8A2C0031` | `49` (`0x31`) | S->C | Handler `0x8A2C0031` (`0x105C40`) | Calls `0x8A2B0011` (`0x1C2ABC`) | 1. `Unpack_ObjRef` (`request_with_locks: WorldJoinRequest` `0x0A44`) | **Receive Encounter Lock Status:** Reads lock status via `native 0x0A44000E` (`0x140A8EEB0`); if locked, opens raid-lock confirmation dialog (`0x0AC1` / `0x0AC5`). |
| `0x8A2C0045` | `69` (`0x45`) | C->S | Packer `0x8A2C0046` (`0x105E98`) | Called by `0x8A2B0015` (`0x1C2BC8`) | 1. `Pack_ObjRef` (`invite: WorldJoinInvite` `0x0A39`) | **Leader Cancel / Modify World Join Invitation:** Sends updated/cancelled `0x0A39` invite (`0x0A390005(0)`). |
| `0x8A2C0049` | `73` (`0x49`) | S->C | Handler `0x8A2C0049` (`0x105E40`) | Calls `0x8A2B0016` (`0x1C2A4C`) -> `native 0x0A2D0003` (`0x140B366E0` -> `0x140AFEAF0`) | 1. `Unpack_u32` (`status_code: u32`) | **World Join Status Update (49):** Notifies UI of World Join status change (`status_code`). |
| `0x8A2C0051` | `81` (`0x51`) | C->S | Packer `0x8A2C0052` (`0x105DA8`) | Called by `0x8A2B0018` (`0x1C2CC0`) | *(0 arguments)* | **Query Active World Join Request:** Requests the current active World Join request state. |
| `0x8A2C0053` | `83` (`0x53`) | S->C | Handler `0x8A2C0053` (`0x105B3C`) | Calls `0x8A2B001A` (`0x1C2CF8`) | 1. `Unpack_ObjRef` (`status_text: WString` `0x00E1`) | **World Join Admin Dump Response:** Displays `/worldjoin` admin dump text on channel `4`. |
| `0x8A2C0059` | `89` (`0x59`) | C->S | Packer `0x8A2C005A` (`0x105DF0`) | Called by `0x8A2B0019` (`0x1C2CD8`) | *(0 arguments)* | **Dismiss World Join UI / Notification:** Notifies server that World Join prompt was dismissed. |
| `0x8A2C0062` | `98` (`0x62`) | S->C | Handler `0x8A2C0062` (`0x105C9C`) | Calls `0x8A2B0025` (`0x1C2A00`) -> `native 0x0A2D0005` (`0x140B358F0` -> `0x140AFD0E0`) | 1. `Unpack_u32` (`queue_status: u32`) | **World Join Queue Status Update (62):** Updates global World Join queue enabled/status flag (`queue_status`). |

#### Native Class `0x0A44` (`WorldJoinRequest`) & Native Class `0x0A39` (`WorldJoinInvite`)

Confirmed from `p5_natives_disasm.txt`, `p5_serializers_and_extras.txt`, `RegisterNativeClassDesc` (`0x14037A770`), and `[WLibClassDesc + 0x68]` binary `Archive` functions:
- **Native Class `0x0A44` (`WorldJoinRequest`, 168 bytes = `0xA8`, registered at `0x140B2B7D3`, `[WLibClassDesc + 0x68]` at `0x140BB1730` -> `0x140AB8AF0`):**
  - Constructed in `0x8A2B0002` (`0x1C3108`) and `0x8A2B0013` (`0x1C2FF4`):
    1. `0x0A440001(template_did: u32)` (VA `0x140A866F0` -> `0x140AA3830`): Sets the `PrivateEncounterTemplate` DataID (`0x0A5B`, stored at `[obj + 0x04]`) and initializes the encounter's parameter map (`[obj + 0x18]`).
    2. `0x0A440002() -> u32` (VA `0x140A702A0`): Getter returning `template_did: u32`.
    3. `0x0A440003(level: u32)` (VA `0x140A88E90`) / `0x0A440004() -> u32` (VA `0x140A72E90`): Setter/getter for `encounter_level: u32` (stored in `[obj + 0x18]` under `WorldJoin_Level` `0x10003751`).
    4. `0x0A440005(group_size_flag: u32)` (VA `0x140A85EA0`) / `0x0A440006() -> u32` (VA `0x140A6FD40`): Setter/getter for `group_size: u32` (stored in `[obj + 0x18]` under `WorldJoin_GroupSize` `0x10002EC9`).
    5. `0x0A440025(difficulty_tier: u32)` (VA `0x140A8AE80`) / `0x0A440026() -> u32` (VA `0x140A71100`): Setter/getter for `difficulty_tier: u32` (stored in `[obj + 0x18]` under `WorldJoin_DifficultyTier` `0x10002ECA`).
    6. `0x0A44000D` / `0x0A44000E`: Raid lock / group number field (`[obj + 0xA0]`, `WorldJoin_GroupNumber` `0x10003AD0`).
  - **Binary `Archive` (`0x140BB1730` -> `0x140AB8AF0`) Wire Layout:**
    1. `u32` `encounter_did` (`[obj + 0x04]`, `WorldJoin_Encounter` `0x10003752`, 4 bytes LE).
    2. `HashTable<u32, u32>` `parameters` (`[obj + 0x18]`, via `0x140A57EB0`):
       - `u8` `bucket_size_index` (index into prime bucket table `0x141A53688`, validated `<= 50,000` buckets),
       - `VLE` `entry_count` (`0x140304D00`),
       - `entry_count` pairs of `(u32 key, u32 value)` (8 bytes LE each), carrying `0x10003751` (`WorldJoin_Level`), `0x10002ECA` (`WorldJoin_DifficultyTier`), and `0x10002EC9` (`WorldJoin_GroupSize`).
    3. `u8` `has_extra` (`bool`, `1` on write): when `1`, followed by `u32` `extra_version` (`1`, 4 bytes LE) and (when `extra_version >= 1`) `u32` `extra_field` (`[obj + 0x08]`, 4 bytes LE).
    4. When stream client-only filter bit 14 (`0x141168D50`: `(stream->flags >> 14) & 1`) is `0`:
       - `u64` `pre_fellowship_id` (`[obj + 0x98]`, `WorldJoin_PreFellowshipID` `0x1000422A`, 8 bytes LE),
       - `HashTable<u64, u32>` `player_to_pe_effect_hash` (`[obj + 0x78]`, `WorldJoin_Encounter_PlayerToPEEffectHash_Array` `0x1000480D`, via `0x140AB3D10`): `u8` `bucket_size_index`, `VLE` `entry_count`, and `entry_count` pairs of `(u64 player_iid, u32 pe_effect_did)` (12 bytes LE each),
       - `u32` `group_number` (`[obj + 0xA0]`, `WorldJoin_GroupNumber` `0x10003AD0`, 4 bytes LE).
- **Native Class `0x0A39` (`WorldJoinInvite`, 224 bytes = `0xE0`, registered at `0x140B2C23E`, `[WLibClassDesc + 0x68]` at `0x140BB1BF0` -> `0x140AE25A0`):**
  - Wraps an inner `WorldJoinRequest` (`0x0A44` at `[obj + 0x10]`, accessed via `0x0A39000F` at VA `0x140A76540`), an invitation type/state (`[obj + 0x08]`, `0x0A390012` at VA `0x14038ACF0`: `1` = pending leader/fellowship confirmation, `2` = ready to travel), an invitation creation/expiration timestamp (`[obj + 0xD8]`, `0x0A390017` at VA `0x140A6EEF0`, `f64`), a member response list (`[obj + 0xB8]`), and the player's accept/decline response setter (`0x0A390005` at VA `0x140A83540`, `bool`).
  - Also broadcast to fellowship members via S->C `0x84FE014E` (`fn=334=0x14E`, `0x0FF5C4`), which unpacks `Unpack_u32(status)`, `Unpack_ObjRef(WorldJoinRequest_0x0A44)`, `Unpack_u64(fellow_iid)` and calls `0x819D0074` (`0x0AAA58`) -> `native 0x013F0027(template_did, status)` (`0x140B47430`).
  - **Binary `Archive` (`0x140BB1BF0` -> `0x140AE25A0`) Wire Layout:**
    1. `u64` `requester_iid` (`[obj + 0x00]`, `WorldJoin_RequesterIID` `0x10003756`, 8 bytes LE).
    2. `u32` `invitation_type` (`[obj + 0x08]`, `WorldJoin_InvitationType` `0x10003757`, 4 bytes LE).
    3. Embedded `WorldJoinRequest` (`0x0A44` at `[obj + 0x10]`, serialized inline via `0x140AB8AF0`).
    4. `List<MemberData>` (`[obj + 0xB8]`, serialized via `0x140AB82C0` -> `0x140AB28E0` using reflection schema `0x140AB6340`):
       - `u32` `member_count` (4 bytes LE),
       - `member_count` `MemberData` records, each serialized in order: `u64` `member_iid` (`+0x00`), `StringInfo` `member_name` (`+0x08`), `u32` `class_or_role` (`+0x60`), `u16` `level` (`+0x64`), `u16` `rank_or_flags` (`+0x66`), `u32` `kinship_or_title_id` (`+0x68`), `u16` `status_flags` (`+0x6C`), `StringInfo` `secondary_string` (`+0x70`), `u32` `response_state` (`+0xC8`), and `u8` `confirmed_bool` (`+0xCC`).
    5. `f64` `invitation_create_time` (`[obj + 0xD8]`, `WorldJoin_InvitationCreateTime` `0x100037A4`, 8 bytes IEEE-754 LE).
- **Supporting Native Container Classes (`0x000B`, `0x0011`, `0x0061`):**
  - `0x000B` (`IntHash` / `HashTable<u32, u32>`, registered at `0x14081B5C8`, `Archive` at `0x140836DF0` -> `0x1406EE9A0`): `u8` `bucket_size_index` + `VLE` `entry_count` + `entry_count * (u32 key, u32 value)`.
  - `0x0011` (`IntArray` / `Array<u32>`, registered at `0x14081B882`, `Archive` at `0x140836FF0` -> `0x140413880`): `u32` `element_count` + `element_count * u32` elements.
  - `0x0061` (`LongHash` / `HashTable<u64, u32>`, registered at `0x14081D564`, `Archive` at `0x140837AC0` -> `0x140818DA0`): `u8` `bucket_size_index` + `VLE` `entry_count` + `entry_count * (u64 key, u32 value)`.

---

### 4.3 Fellowship Maker System (`0x0C41` <-> `0x0C3D` / `0x0C3B` `CFellowshipMakerSystem`)

#### Complete RPC Table — Class `0x0C41` (3137)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x0C3D`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x8C41000D` | `13` (`0x0D`) | C->S | Packer `0x8C41000E` (`0x0FFBCC`) | Called by `0x8C3D0003` (`0x0B205C`) | *(0 arguments)* | **Leave Matchmaking Queue (`WorldCommand::LeaveQueue`):** Removes the player or fellowship from the Fellowship Maker queue (rate-limited by `Antispam_FellowshipMakerRequest` `0x10003C5B`). |
| `0x8C41000F` | `15` (`0x0F`) | C->S | Packer `0x8C410010` (`0x0FFA94`) | Called by `0x8C3D0004` (`0x0B20FC`) | 1. `Pack_u32` (`confirm_roles_or_bool: u32`) | **Confirm Enter Queue / Ready Check:** Confirms the player's role selection / pre-fellowship queue entry (`0x8C3B0013` validates `(confirm_roles & available_roles) == confirm_roles`). |
| `0x8C410019` | `25` (`0x19`) | C->S | Packer `0x8C41001A` (`0x0FFC14`) | Called by `0x8C3D000A` (`0x0B2184`) | 1. `Pack_u32` (`roles_mask: u32`) | **Set Player Role Mask:** Updates `FellowshipMaker_PlayerRole` (`0x10004166`) on the server (`roles_mask` bitmask: Damage, Healer, Support, Defense). |
| `0x8C410028` | `40` (`0x28`) | S->C | Handler `0x8C410028` (`0x0FF884`) | Calls `0x8C3D000C` (`0x0B2294`, `RecvCEvt_FellowConfirmEnterQueue`) | 1. `Unpack_u32` (`confirmed_bool: u32`)<br>2. `Unpack_u64` (`fellow_iid: u64`) | **Fellow Confirmed Enter Queue (`InstanceQueueChanged`):** Displays chat notification that `fellow_iid` confirmed (`0xA124415`) or declined (`0x6959ED5`) entering the queue, and if `confirmed_bool != 0` calls `native 0x0CD6001D(fellow_iid)` (`0x140B35660`). |
| `0x8C410034` | `52` (`0x34`) | S->C | Handler `0x8C410034` (`0x0FF96C`) | Calls `0x8C3D0011` (`0x0B2844`) -> `native 0x0CD6001F` (`0x140B3CD10`) | 1. `Unpack_ObjRef` (`assigned_roles_list: IntArray` `0x0011`)<br>2. `Unpack_ObjRef` (`member_roles_map: LongHash` `0x0061`) | **Pre-Fellowship Match Progress (`InstanceQueueChanged`):** Delivers matched roles (`assigned_roles_list` + `member_roles_map`, total count = `list.size + map.size`) and notifies the UI (`native 0x0CD6001F`). |
| `0x8C410037` | `55` (`0x37`) | S->C | Handler `0x8C410037` (`0x0FF8F8`) | Calls `0x8C3D0012` (`0x0B28E0`) -> `native 0x0CD60020` (`0x140B3CC60`) | 1. `Unpack_u32` (`status_or_role: u32`)<br>2. `Unpack_u64` (`member_iid: u64`) | **Pre-Fellowship Member Response (`InstanceQueueChanged`):** Updates pre-fellowship member `member_iid` confirmation status (`status_or_role`) via `native 0x0CD60020`. |
| `0x8C410049` | `73` (`0x49`) | S->C | Handler `0x8C410049` (`0x0FF9E8`) | Calls `0x8C3D0018` (`0x0B2A64`) | 1. `Unpack_ObjRef` (`cooldown_or_error_map: LongHash` `0x0061`) | **Fellowship Maker Queue Failure / Cooldown Map:** Iterates `LongHash` (`fellow_iid: u64` -> `error_or_timestamp: u32`) and displays formatted chat error (`0x149894E`). |
| `0x8C41004F` | `79` (`0x4F`) | C->S | Packer `0x8C410052` (`0x0FFA4C`) | Called by `0x8C3D001B` (`0x0B2D74`) | *(0 arguments)* | **Confirm Pre-Fellowship Ready Check:** Confirms readiness when matched into a pre-fellowship (`World_FellowshipMaker_PreFellowshipTimer` `0x10003CC0`). |
| `0x8C410055` | `85` (`0x55`) | C->S | Packer `0x8C410058` (`0x0FFAEC`) | Called by `0x8C3D001D` (`0x0B1FE8`) | 1. `Pack_ObjRef` (`encounter_config_map: IntHash` `0x000B`)<br>2. `Pack_u32` (`config_flags: u32`)<br>3. `Pack_u32` (`roles_mask: u32`) | **Join Matchmaking Queue for Specific Encounters:** Queues for a set of selected encounters (`encounter_config_map` mapping `template_did: u32` -> `config_flags: u32`) with `config_flags` and `roles_mask`. |
| `0x8C410056` | `86` (`0x56`) | C->S | Packer `0x8C410057` (`0x0FFB64`) | Called by `0x8C3D001C` (`0x0B1F78`) | 1. `Pack_u32` (`config_flags: u32`)<br>2. `Pack_u32` (`roles_mask: u32`) | **Join Matchmaking Queue for Random Encounter:** Queues for a random encounter matching `config_flags` (bitmask of sizes `0x10`/`0x20`/`0x40`/`0x80` in `0x8C3B001E`, tiers, and encounter types) and `roles_mask`. |

#### Fellowship Maker Player Properties (`InstanceQueueChanged`)

| Property ID | Name | Type | Semantics |
|---|---|---|---|
| `0x10003DCA` | `FellowshipMaker_PlayerState` | `u32` | Current queue state on the player entity (`0x8C3B0007` / `0x8C3B0018`): `0` = None/Idle, `1` = Waiting for Fellowship Confirmation, `2` = Queued, `4` = Pre-Fellowship Ready Check, `7` = In Matchmade Encounter. |
| `0x10004166` | `FellowshipMaker_PlayerRole` | `u32` | Selected role bitmask on the player entity (`0x8C3B000E`). |
| `0x100041BE` | `FellowshipMaker_PlayerConfiguration` | `u32` | Selected configuration bitmask (group sizes, tiers, types) on the player entity (`0x8C3B001D`). |
| `0x1000419C` | `FellowshipMaker_EnterQueueConfirmedIIDArray` | `Array` | Array of fellowship member IIDs who have confirmed queue entry. |
| `0x100047CB` | `FellowshipMaker_PlayerSelectedEncounters` | `Array` | Array of selected `FellowshipMaker_EncounterDataID` (`0x100047C7`) values. |

---

### 4.4 Private Encounter System (`0x051F` <-> `0x011C` / `0x01C1` / `0x01C2` / `0x06E9`)

RPC class `0x051F` (1311) handles active Private Encounter (instance) lifecycle, instance quests/objectives, boss/skirmish announcements, and leaving an instance.

#### Complete RPC Table — Class `0x051F` (1311)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target (`0x011C` / `0x01C1` / `0x01C2`) | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x851F000C` | `12` (`0x0C`) | S->C | Handler `0x851F000C` (`0x109DB0`) | Calls `0x81C2002D` (`0x12874C`) -> `native 0x01570001` (`0x140B35220` -> `0x140AFC280`) | 1. `Unpack_u32` (`template_did: u32`) | **Enter Private Encounter (`InstanceChanged`):** Sets the player's active `PrivateEncounterTemplate` (`0x0A5B`) to `template_did` (`vcall(slot=464)`) and invokes `PrivateEncounter::OnEnterEncounter(player, template_did)` (`vtable+0x20`). |
| `0x851F0027` | `39` (`0x27`) | S->C | Handler `0x851F0027` (`0x10A150`) | Calls `0x81C20071` (`0x1287B8`) -> `native 0x01570002` (`0x140B381B0` -> `0x140B01AD0`) | *(0 arguments)* | **Leave Private Encounter (`InstanceChanged`):** Clears active encounter quests (`vcall(slot=600) -> vcall(slot=30)`), sets active template to `0` (`vcall(slot=464)`), and invokes `PrivateEncounter::OnLeaveEncounter(player)` (`vtable+0x38`). |
| `0x851F002C` | `44` (`0x2C`) | C->S | Packer `0x851F0067` (`0x109C70`) | Called by `0x811C000A` (`0x1278A8`) | 1. `Pack_ObjRef` (`request: WorldJoinRequest` `0x0A44`)<br>2. `Pack_u64` (`npc_or_door_iid: u64`) | **Launch Private Encounter via NPC / Door:** Requests launching the `WorldJoinRequest` (`0x0A44`) through an in-world NPC or instance door `npc_or_door_iid`. |
| `0x851F0085` | `133` (`0x85`) | C->S | Packer `0x851F00BC` (`0x109BD0`) | Called by `0x86E90001` (`0x128248`) | *(0 arguments)* | **Confirm Instance Prompt (`0x06E9`):** Sent when confirming dialog `0x06E9`. |
| `0x851F0095` | `149` (`0x95`) | S->C | Handler `0x851F0095` (`0x109D44`) | Calls `0x81C200FE` (`0x128B50`) -> `native 0x01570003` (`0x140B3FE80`) | 1. `Unpack_ObjRef` (`encounter_info: 0x01B7`) | **Add Available Encounter at NPC/Door (`0x01B7`):** Registers `0x01B7` encounter descriptor (`0x01B70002` = `template_did`) in `vcall(slot=528)` and notifies UI (`native 0x01570003`). |
| `0x851F009C` | `156` (`0x9C`) | S->C | Handler `0x851F009C` (`0x109CD8`) | Calls `0x81C2003F` (`0x128A20`) -> `native 0x01570004` (`0x140B305B0`) | 1. `Unpack_ObjRef` (`encounter_info: 0x01B7`) | **Update Available Encounter at NPC/Door (`0x01B7`):** Updates `0x01B7` in `vcall(slot=528)` and notifies UI (`native 0x01570004`). |
| `0x851F009D` | `157` (`0x9D`) | S->C | Handler `0x851F009D` (`0x10A0EC`) | Calls `0x81C20049` (`0x128A74`) -> `native 0x01570005` (`0x140B3F7F0`) | 1. `Unpack_ObjRef` (`encounter_info: 0x01B7`) | **Remove Available Encounter at NPC/Door (`0x01B7`):** Removes `0x01B7` from `vcall(slot=528)` and notifies UI (`native 0x01570005`). |
| `0x851F00A3` | `163` (`0xA3`) | S->C | Handler `0x851F00A3` (`0x10A1AC`) | Calls `0x81C20028` (`0x128AE0`) -> `native 0x01570006` (`0x140B47C00`) | 1. `Unpack_ObjRef` (`encounter_info: 0x01B7`) | **Select Available Encounter at NPC/Door (`0x01B7`):** Updates `vcall(slot=528)` and notifies UI (`native 0x01570006`). |
| `0x851F00BA` | `186` (`0xBA`) | S->C | Handler `0x851F00BA` (`0x10A210`) | Calls `0x81C20073` (`0x128C00`) -> `native 0x01570009` (`0x140B41650`) | 1. `Unpack_f32` (`time_remaining_sec: f32`) | **Update Encounter Timer:** Sets encounter shutdown/expiration timer (`vcall(slot=533)`) and notifies UI (`native 0x01570009`). |
| `0x851F00D8` | `216` (`0xD8`) | S->C | Handler `0x851F00D8` (`0x109E8C`) | Calls `0x811C0005` (`0x127484`) -> `native 0x0157000B` (`0x140B344F0`) | 1. `Unpack_ObjRef` (`quest_list_or_info`)<br>2. `Unpack_ObjRef` (`encounter_data`) | **Open NPC/Door Private Encounter Dialog:** Opens the local NPC/door Private Encounter selection dialog (`native 0x0157000B`). |
| `0x851F00EE` | `238` (`0xEE`) | S->C | Handler `0x851F00EE` (`0x10A33C`) | Calls `0x811C000B` (`0x12762C`) -> `native 0x0157000C` (`0x140B31140`) | 1. `Unpack_u32` (`arg0_u32: u32`)<br>2. `Unpack_u32` (`arg1_u32: u32`) | **Update Private Encounter State (`0x0157000C`):** Notifies UI (`vtable+0x40`). |
| `0x851F0113` | `275` (`0x113`) | S->C | Handler `0x851F0113` (`0x10A2E0`) | Calls `0x811C000C` (`0x12760C`) -> `native 0x0157000D` (`0x140B3C360`) | *(0 arguments)* | **Close Private Encounter Dialog (`0x0157000D`):** Closes the NPC/door Private Encounter dialog (`vtable+0x38`). |
| `0x851F011D` | `285` (`0x11D`) | S->C | Handler `0x851F011D` (`0x10A270`) | Calls `0x811C000D` (`0x127644`) -> `native 0x01590007` (`0x140B485D0`) | 1. `Unpack_ObjRef` (`banner_text: StringInfo` `0x00C7`) | **Display Encounter Billboard / Objective Banner:** Displays `banner_text` on screen (`native 0x01590007`) and on chat channel `0x100000`. |
| `0x851F0136` | `310` (`0x136`) | C->S | Packer `0x851F0137` (`0x109FE0`) | Called by `0x811C0014` (`0x1276B8`) | *(0 arguments)* | **Leave Active Instance (`WorldCommand::LeaveInstance`):** Requests teleporting out of the current Private Encounter / instance (`Travel_LeavePrivateInstance` `0x10004054`). |
| `0x851F0150` | `336` (`0x150`) | S->C | Handler `0x851F0150` (`0x109F18`) | Calls `0x811C0019` (`0x127514`) | 1. `Unpack_ObjRef` (`error_text: StringInfo` `0x00C7`) | **Private Encounter Error Notification:** Displays `error_text` in a modal alert dialog (`0x00070001`) and on chat channel `1024`. |
| `0x851F015A` | `346` (`0x15A`) | S->C | Handler `0x851F015A` (`0x109EF8`) | No-op (`ret`) | *(0 arguments)* | **No-op Handler:** Returns `0` immediately. |
| `0x851F015B` | `347` (`0x15B`) | S->C | Handler `0x851F015B` (`0x10A088`) | Calls `0x81C10064` (`0x1280E4`) -> `native 0x0157000F` (`0x140B38240`) | 1. `Unpack_u32` (`quest_did: u32`) | **Add Private Encounter Quest:** Adds `quest_did` to the active encounter quest list (`0x0157000F`). |
| `0x851F015E` | `350` (`0x15E`) | S->C | Handler `0x851F015E` (`0x10A028`) | Calls `0x81C10065` (`0x1280C8`) -> `native 0x0157000E` (`0x140B352C0`) | 1. `Unpack_u32` (`quest_did: u32`) | **Remove Private Encounter Quest:** Removes `quest_did` from the active encounter quest list (`0x0157000E`). |
| `0x851F0160` | `352` (`0x160`) | S->C | Handler `0x851F0160` (`0x10A3AC`) | Calls `0x811C001F` (`0x127858`) | 1. `Unpack_ObjRef` (`pe_obj`)<br>2. `Unpack_u32` (`arg1_u32: u32`) | **Update Private Encounter Slot 296:** Calls `vcall(slot=296)` on `0x011C`. |
| `0x851F0162` | `354` (`0x162`) | C->S | Packer `0x851F0163` (`0x109F7C`) | Called by `0x81C10067` (`0x1280A8`) | *(0 arguments)* | **Request Private Encounter Quest Sync:** Requests current encounter quest state. |
| `0x851F0165` | `357` (`0x165`) | S->C | Handler `0x851F0165` (`0x109C18`) | Calls `0x81C2012C` (`0x128800`) | *(0 arguments)* | **Notify Encounter Ready (`0x01570010`):** Calls `vcall(slot=300)` (`native 0x01570010`). |
| `0x851F0168` | `360` (`0x168`) | S->C | Handler `0x851F0168` (`0x10A420`) | Calls `0x81C2012D` (`0x128814`) -> `native 0x01570011(1)` | *(0 arguments)* | **Enable Encounter Feature Flag (`0x01570011`):** Calls `native 0x01570011(true)`. |
| `0x851F0169` | `361` (`0x169`) | S->C | Handler `0x851F0169` (`0x109E2C`) | Calls `0x81C2012E` (`0x128834`) -> `native 0x01570011(0)` | *(0 arguments)* | **Disable Encounter Feature Flag (`0x01570011`):** Calls `native 0x01570011(false)`. |
| `0x851F016C` | `364` (`0x16C`) | C->S | Packer `0x851F016D` (`0x10A460`) | Called by `0x81C2012F` (`0x128854`) | *(0 arguments)* | **Trigger Encounter Action (`0x851F016C`):** Sends 0-arg C->S event to active encounter. |

---

### 4.5 Skirmish System (`0x09EC` <-> `0x0A52`) & Instance Milestones (`0x083A` <-> `0x0837`)

#### Complete RPC Table — Class `0x09EC` (2540, Skirmish Stats) & Class `0x083A` (2106, Instance Milestones)

| `function_id` | `fn` | Dir | Packer / Handler Off | Caller / Target | Wire Arguments (FIFO Order) | Semantics |
|---|---|---|---|---|---|---|
| `0x89EC0007` | `7` (`0x07`) | S->C | Handler `0x89EC0007` (`0x10D618`) | `0x8A520001` (`0x170D88`, `RecvCEvt_UpdateSkirmishStat`) | 1. `Unpack_ObjRef` (`stat_prop: Property` `0x0027`)<br>2. `Unpack_u32` (`template_did: u32`) | **Update Skirmish Stat:** Sets `stat_prop` in the player's skirmish stat `PropertyBag` for `template_did` and calls `native 0x0A510002`. |
| `0x89EC000A` | `10` (`0x0A`) | S->C | Handler `0x89EC000A` (`0x10D554`) | `0x8A520002` (`0x170D50`) | 1. `Unpack_ObjRef` (`all_stats_hash: IntHash` `0x0023`) | **Sync All Skirmish Stats:** Stores the full skirmish stat `IntHash` (`vcall(slot=838)`) and calls `native 0x0A510001`. |
| `0x89EC000E` | `14` (`0x0E`) | S->C | Handler `0x89EC000E` (`0x10D4DC`) | `0x8A520003` (`0x170ECC`, `RecvCEvt_IncrementSkirmishStat`) | 1. `Unpack_ObjRef` (`delta_prop: Property` `0x0027`)<br>2. `Unpack_u32` (`template_did: u32`) | **Increment Skirmish Stat:** Adds `delta_prop` (`i32` or `f32`) to the existing skirmish stat for `template_did` and calls `native 0x0A510002`. |
| `0x89EC0021` | `33` (`0x21`) | S->C | Handler `0x89EC0021` (`0x10D5B8`) | `0x8A520007` (`0x1712F4`) | 1. `Unpack_u32` (`status_or_did: u32`) | **Skirmish UI Notification:** Calls `native 0x0A510003(status_or_did)`. |
| `0x883A0001` | `1` (`0x01`) | C->S | Packer `0x883A0004` (`0x0F950C`) | Called by `0x8837000D` (`0x0476E8`) | 1. `Pack_ObjRef` (`appearance_or_milestone_array: Array` `0x00B0`) | **Use Instance Milestone / Appearance Control (`0x083A`):** Sends filtered `Array` (`0x00B0`) to server. |
| `0x883A0003` | `3` (`0x03`) | S->C | Handler `0x883A0003` (`0x0F9564`) | `0x88370008` (`0x0477B0`) -> `native 0x08380001` | 1. `Unpack_u64` (`entity_iid: u64`) | **Open Instance Milestone / Appearance UI:** Calls `native 0x08380001(entity_iid)`. |
| `0x883A0005` | `5` (`0x05`) | S->C | Handler `0x883A0005` (`0x0F94C0`) | `0x8837000F` (`0x0477C8`) -> `native 0x08380003` | *(0 arguments)* | **Close Instance Milestone / Appearance UI:** Calls `native 0x08380003()`. |

---

## 5. Clean-Room Spec Section (Ready to Merge into `docs/specs/official-messages-gameplay.md`)

### 5.1 Map Markers, Waypoints, Fellowship Positions, Entity Tracking, & Fog-of-War (`WorldEvent::MarkerAdded`, `MarkerRemoved`) <a id="p5-map-markers"></a>

#### 5.1.1 Overview & Provenance

Map and radar markers (`WorldEvent::MarkerAdded`, `WorldEvent::MarkerRemoved`), Point-of-Interest (POI) discovery, and map fog-of-war bitmaps are synchronized using WLib RPC classes `0x0530` (Entity Tracking), `0x050E` (Landmark Discovery), `0x0515` (Map Fog-of-War), `0x04E3`/`0x04E4` (Area Query), `0x04FE`/`0x0739` (Fellowship Positions & Target Marks), and `0x0521` (Quest Objective Events & Tracker), together with entity property updates (`BIN`, `DAT`, `OBS`).

#### 5.1.2 WLib RPC Summary Table — Map, Tracking, Landmark, & Waypoint Messages

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

#### 5.1.3 Key Properties — Area, Landmark, & Quest Waypoint Discovery

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

#### 5.1.4 Byte Layout — Fog-of-War `SmartBuffer` (`0x008E`) Native `ObjRef` Payload (`0x85150001` / `0x85150004`)

In `0x85150001` (C->S) and `0x85150004` (S->C), the `fog_of_war_data` `ObjRef` argument is an instance of WLib native class `0x008E` (`SmartBuffer`). Following the standard WLib `ObjRef` object-table header (`VLE` `object_count`, `u32[object_count]` `object_ids`) and per-object header (`u32` `object_flags` masked with `0xBB000000` where bit 28 `0x10000000` is `0` for native classes, followed by `u16` `class_id = 0x008E`), the native binary serializer writes/reads:

| Offset | Size (Bytes) | Type | Field | Meaning | Byte Order | Provenance |
|---|---|---|---|---|---|---|
| `0x00` | `4` | `u32` | `byte_length` | Number of raw fog-of-war bitmap bytes in `bitmap_bytes`. | Little-endian | Confirmed (`BIN`) |
| `0x04` | `byte_length` | `u8[byte_length]` | `bitmap_bytes` | Raw per-map fog-of-war exploration bitmap payload. | Byte array | Confirmed (`BIN`) |

---

### 5.2 Stable-Master Travel, Milestone, Recall, & Summon (`WorldEvent::TravelShown`, `TravelStarted`, `TravelEnded`, `WorldCommand::ChooseTravel`) <a id="p5-travel"></a>

#### 5.2.1 Overview & Provenance

Stable-Master travel uses WLib RPC class `0x0533` (`1331`). Interacting with a Stable-Master NPC sends C->S `0x85330008`; the server replies with S->C `0x85330002` (`npc_iid`) to open the travel window (`WorldEvent::TravelShown`). Route definitions (`TravelWeb`, `TravelDestination`, `TravelRoute`) are resolved from the NPC's `TravelWebWC` (`0x10000F66`) property in the client DAT files and combined with the player's discovered location set (`0x85330006`, `0x8533000B`, `0x8533001B`) and discount properties (`0x1000064A`, `0x100027EE`). Selecting a route (`WorldCommand::ChooseTravel`) sends C->S `0x8533000D` (`route_did`, `npc_iid`). Starting and ending a travel route (`WorldEvent::TravelStarted`, `WorldEvent::TravelEnded`) are driven by player entity property `Travel_IsTraveling` (`0x100009E8`), and early dismount sends C->S `0x85330005` (`BIN`, `DAT`, `OBS`).

#### 5.2.2 WLib RPC Summary Table — Travel, Recall, & Summon

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

#### 5.2.3 Byte Layout — `ChooseTravel` (`0x8533000D`) `WLibArgFrame` Payload

| Offset | Size (Bytes) | Type | Field | Meaning | Byte Order | Provenance |
|---|---|---|---|---|---|---|
| `0x00` | `4` | `u32` | `route_did` | DataID of the selected `TravelRoute` (`0x05E1`). | Little-endian | Confirmed (`BIN`) |
| `0x04` | `8` | `u64` | `npc_iid` | InstanceID of the Stable-Master NPC. | Little-endian | Confirmed (`BIN`) |

#### 5.2.4 Travel State Machine & Properties

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

### 5.3 Mounts, War-Steeds, Mounted Combat, & Mount Outfits (`WorldEvent::MountChanged`, `MountVitalChanged`, `WorldCommand::Dismount`) <a id="p5-mounts"></a>

#### 5.3.1 Overview & Provenance

Summoning a mount or war-steed uses standard skill execution (`WorldCommand::UseSkill`). Dismounting (`WorldCommand::Dismount`) sends the 0-argument WLib RPC `0x88000006`. All mount state changes (`WorldEvent::MountChanged`) and standard/war-steed vital changes (`WorldEvent::MountVitalChanged`) are delivered via property updates on the rider entity (`BIN`, `DAT`, `OBS`).

#### 5.3.2 WLib RPC Summary Table — Mounts, Mounted Combat, & Mount Outfits

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

#### 5.3.3 Rider Entity Properties — `MountChanged` & `MountVitalChanged`

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

### 5.4 Instance Finder (`WorldJoin` & `FellowshipMaker`), Private Encounters, & Skirmishes (`WorldEvent::InstanceListShown`, `InstanceChanged`, `InstanceQueueChanged`, `WorldCommand::JoinInstance`, `LeaveInstance`, `LeaveQueue`) <a id="p5-instances"></a>

#### 5.4.1 Overview & Provenance

The Instance Finder and Private Encounter lifecycle spans four WLib RPC classes (`BIN`, `DAT`, `OBS`):
- **World Join (`0x0A2C`):** Direct instance selection and launch (`0x8A2C0001` query list -> `0x8A2C0003` `WorldEvent::InstanceListShown`; `0x8A2C0005` `WorldCommand::JoinInstance`; `0x8A2C0007` cancel; `0x8A2C000B` invite -> `0x8A2C0009` response).
- **Fellowship Maker (`0x0C41`):** Automated matchmaking queue (`0x8C410055` queue specific; `0x8C410056` queue random; `0x8C41000D` `WorldCommand::LeaveQueue`; `0x8C41000F` confirm fellowship queue entry; `0x8C41004F` confirm pre-fellowship match; `0x8C410028`/`0x8C410034`/`0x8C410037` `WorldEvent::InstanceQueueChanged`).
- **Private Encounter (`0x051F`):** Active instance entry (`0x851F000C` `WorldEvent::InstanceChanged`), instance exit notification (`0x851F0027` `WorldEvent::InstanceChanged`), and leaving an active instance (`0x851F0136` `WorldCommand::LeaveInstance`).
- **Skirmish (`0x09EC`):** Skirmish statistics updates (`0x89EC0007`, `0x89EC000A`, `0x89EC000E`, `0x89EC0021`).

#### 5.4.2 WLib RPC Summary Table — Instance Finder & Private Encounters

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

#### 5.4.3 Key Properties — Instance Finder & Private Encounters

| Property ID | Property Name | Type | Entity Scope | Meaning | Provenance |
|---|---|---|---|---|---|
| `0x100007AB` | `PrivateEncounter_Template_DataID` | `DataID` (`u32`) | Player | Active `PrivateEncounterTemplate` DataID while inside an instance (`0` when on landscape). | Confirmed (`BIN`, `DAT`) |
| `0x10003DCA` | `FellowshipMaker_PlayerState` | `u32` | Player | Matchmaking queue state (`0` = Idle, `1` = Confirming Fellowship, `2` = Queued, `4` = Pre-Fellowship Ready Check, `7` = In Encounter). | Confirmed (`BIN`, `DAT`) |
| `0x10004166` | `FellowshipMaker_PlayerRole` | `u32` | Player | Bitmask of roles selected for Fellowship Maker matchmaking. | Confirmed (`BIN`, `DAT`) |
| `0x100041BE` | `FellowshipMaker_PlayerConfiguration` | `u32` | Player | Bitmask of group sizes (`0x10` Solo, `0x20` Duo, `0x40` Small Fellowship, `0x80` Fellowship), tiers, and encounter types selected for matchmaking. | Confirmed (`BIN`, `DAT`) |
| `0x100037F4` | `Skirmish_PlayerLevelAtSkirmishStart` | `i32` | Player | Player level recorded at the start of a skirmish. | Confirmed (`BIN`, `DAT`) |

#### 5.4.4 Byte Layouts — Native `ObjRef` Payloads (`0x0A44`, `0x0A39`, `0x000B`, `0x0011`, `0x0061`)

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

### 5.5 Testable Invariants & Synthetic Test Vectors <a id="p5-test-vectors"></a>

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

### 5.6 Open Questions <a id="p5-open"></a>

None. All P5 map, travel, mount, war-steed, and instance finder wire messages, opcodes, argument layouts, and native object serializations are confirmed from binary and data analysis.

---

## Change Log

| Date | Author | Summary |
|---|---|---|
| 2026-10-21 | Spec writer (dirty room) | Initial complete P5 wire protocol note and clean-room spec section covering Map Markers & Fog-of-War, Stable-Master Travel, Mounts & War-Steeds, and Instance Finder / Private Encounters. |
| 2026-10-22 | Spec writer (dirty room) | Resolved `P5-1` by confirming `WLibClassDesc` native binary `Archive` wire layouts for `0x008E` (`SmartBuffer`), `0x0A44` (`WorldJoinRequest`), `0x0A39` (`WorldJoinInvite`), `0x000B` (`IntHash`), `0x0011` (`IntArray`), and `0x0061` (`LongHash`). |

