<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# P4 Social Panels Wire Protocol: Fellowships & Raids, Player-to-Player Trade, Friends/Ignore & Kinships, and Player Music

- **Status:** Draft
- **Last updated:** 2026-10-20
- **Author role:** Spec writer (dirty room)
- **Reviewer:** pending

---

## Purpose

This research note documents the complete official LOTRO World Server WLib RPC and WState wire protocol for the four **P4 Social Panels** domains:

1. **Fellowships & Raids** — `ServerFellowship` (`0x04FE`, decimal `1278`) ↔ `ClientFellowship` (`0x019D` / `0x0452`), `ServerRaid` (`0x0523`, decimal `1315`) ↔ `ClientRaid` (`0x0123`), `ServerFellowshipVote` (`0x0CFE`, decimal `3326`) ↔ `ClientFellowshipVote` (`0x0D00`), `ServerTargetMarking` (`0x0739`, decimal `1849`) ↔ `ClientTargetMarking` (`0x0736`), and `ServerFellowshipMaker` (`0x0C41`, decimal `3137`) ↔ `ClientFellowshipMaker` (`0x0C3D`), including WState layouts for `Fellow` (`0x044D`), `FellowPet` (`0x044E`), `Fellowship` (`0x044F`), and `Raid` (`0x0454`).
2. **Player-to-Player Trade (`SecureTrade`)** — `ServerTrade` (`0x0531`, decimal `1329`) ↔ `ClientTrade` (`0x017F`, decimal `383`), including the `TradeOfferData` (`0x05C3`, decimal `1475`) and `Money` (`0x0193`, decimal `403`) WState serializers, item instance ID transfers, copper/coin denomination tables, and trade state transitions (`OpenTrade`, `OfferTrade`, `SetTradeReady`, `TradeShown`, `TradeOffer`, `TradeReady`, cancel, and completion).
3. **Friends & Ignore Lists, `/who` & LFG, User Chat Channels, and Kinship (`Guild`) Roster & Calendar** — `ServerChat` (`0x04EC`, decimal `1260`) ↔ `ClientChat` (`0x0102`, decimal `258`), `ServerWho` (`0x050C`, decimal `1292`) ↔ `ClientWho` (`0x1021`, decimal `4129`), `ServerGuild` (`0x0502`, decimal `1282`) ↔ `ClientGuild` (`0x016C`, decimal `364`), and their coordination with the UDP `ChatNet` Buddy List (`0x0170`, `0x018D`, `0x0193`, `0x015E`, `0x01A2`) and User Chat Channel (`0x016D`–`0x0195`) messages.
4. **Player Music (Solo & Synchronized)** — `ServerMusic` (`0x0519`, decimal `1305`) ↔ `ClientMusic` (`0x0116`, decimal `278`), `/music` mode, `/play <song> [sync]`, `/playstart`, real-time NoteOn/NoteOff streaming, and the native `MusicNoteBatch` (`0x0804`, decimal `2052`) WState serializer.
5. **Clean-Room Specification Section** — Self-contained clean-room prose, byte-layout tables, state machines, and synthetic test vectors (free of binary virtual addresses and internal C++ symbol names) ready for direct integration into `docs/specs/official-messages-gameplay.md`.

All findings comply with Decision 0034 (G4 ban-risk warning requirement; G5 permanent boundaries: no cryptographic keys, server endpoints, packet captures, or player/account data).

---

## Provenance

| ID | Class | Method | Source / Binary | Build / Version | SHA-256 | Tool & Version | Question Answered | Why M1–M2 Were Insufficient |
|---|---|---|---|---|---|---|---|---|
| `BIN-WLIB-P4` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (embedded WLib bytecode image at file offset `0x01AE1EB0` and native package dispatch tables) | `4906.0071.1794.4035` | `b532ecd4d4dc2fc2db2fe1a246cfc2f7fa0bb7ffebfe3a8dc8ceec2bedf0fd4d` | Python WLib bytecode disassembler & `objdump 2.42` | Exact method indices (`0x8PPPSSSS`), parameter stack push/pop order, `Pack_ObjRef` (`0x14114E5F0`) / `Unpack_ObjRef` (`0x14114E910`) wire framing, and WState `vslot 2` serializers for `0x04FE`, `0x0523`, `0x0CFE`, `0x0739`, `0x0C41`, `0x0531`, `0x04EC`, `0x050C`, `0x0502`, and `0x0519`. | WLib RPC method dispatchers and WState object serializers are compiled into the client binary's embedded WLib VM image and native C++ packages; they do not appear in `.dat` files or public documentation. |
| `BIN-TRADE-NAT` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (`0x140B43E60`–`0x140B5F2B6`, `0x140A57EB0`–`0x140A584D0`) | `4906.0071.1794.4035` | `b532ecd4d4dc2fc2db2fe1a246cfc2f7fa0bb7ffebfe3a8dc8ceec2bedf0fd4d` | `objdump 2.42` | Exact C++ callers of `ClientTrade` (`0x017F`), UI event hooks in package `0x0163`, and the native `IntrusiveHashTable<u32, u32>` WState serializer for `Money` (`0x0193`, `0x140A58410` → `0x140A57EB0`). | SecureTrade native UI glue and the `Money` native class serializer exist only in executable code. |
| `BIN-MUSIC-NAT` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (`0x140BC00B0`–`0x140BC0630`, `0x140B8FD80`–`0x140B902B0`, `0x140AAFBD0`–`0x140AAFE70`) | `4906.0071.1794.4035` | `b532ecd4d4dc2fc2db2fe1a246cfc2f7fa0bb7ffebfe3a8dc8ceec2bedf0fd4d` | `objdump 2.42` | Exact C++ callers and handlers for `ClientMusic` (`0x0116`), native package `0x0804` (`MusicNoteBatch`), and its WState serializer (`0x140AAFD60` → `0x140AAFBD0`). | Player music note-streaming and `MusicNoteBatch` (`0x0804`) serialization are implemented in native C++ code. |
| `OBS-DAT-P4` | Confirmed | M1 (`OBS`) | `client_gamelogic.dat` (`0x34000000` `MasterProperty`) | Steam install (read-only) | — | Python `.dat` reader | Property IDs (`PID`) referenced by P4 WLib methods (`0x100002B8`–`0x10004DF1`, `0x10000DF0` `Communication_CharacterPresence`, `0x1000305D` `Annotation_Text`, `0x10003086` `Annotation_IgnoreReason`, `0x100020CA`–`0x100020CF` Fellowship properties). | Used M1 directly to resolve property IDs observed in WLib bytecode. |

---

## 0. Common WLib RPC & WState Wire Framing (`0x00000030`) <a id="common-wlib-framing"></a>

All four P4 domains communicate with the World Server over `"ClientNet"` using WLib entity RPC messages (`NetMessage` event opcode `0x00000030`) dispatched on either:
- The local player's avatar entity (`Avatar` / `Player`, via `0x02000009` `GetLocalPlayer()`), or
- A dedicated singleton manager entity (such as `ClientGuild` `0x016C` ↔ `ServerGuild` `0x0502`, or `ClientFellowshipMaker` `0x0C3D` ↔ `ServerFellowshipMaker` `0x0C41`).

### 0.1 WLib RPC Call Header and Parameter Stack Order

In every `send_msg` WLib instruction (`opcode 0x71`, `u32 method_id = 0x8PPPSSSS` where `PPP` is the 12-bit package/class ID and `SSSS` is the 16-bit static method index):

1. **Method Identifier:** The RPC frame identifies the target package (`0x04FE`, `0x0523`, `0x0CFE`, `0x0739`, `0x0C41`, `0x0531`, `0x04EC`, `0x050C`, `0x0502`, `0x0519`) and method index (`SSSS`), or its dense vtable slot index on the synchronized entity interface.
2. **Parameter Serialization Order:** In the WLib VM, the caller pushes arguments onto the evaluation stack before executing `call_static` (`0x4B`). Inside the RPC stub, the prologue pops arguments from the stack in **LIFO order** (`pop_local_0` pops the last-pushed argument; `pop_local_N` pops the first-pushed argument) and then executes `push_local_0 .. push_local_N` followed by `send_msg`. Because `send_msg` serializes the re-pushed stack slots sequentially, the **wire byte order of parameters in every WLib RPC payload matches the stub's `push_local_0, push_local_1, ...` order** documented in the tables below.
3. **Primitive Parameter Wire Types:**
   - `u32` / `i32` / `bool` (WLib stack type `2`): 4-byte little-endian integer (`u32 LE`).
   - `u64` / `EntityID` (`InstanceID` / `IID`, WLib stack type `3`): 8-byte little-endian unsigned integer (`u64 LE`).
   - `f32` (WLib stack type `4`): 4-byte IEEE-754 little-endian float (`f32 LE`).
   - `f64` (WLib stack type `5`): 8-byte IEEE-754 little-endian double (`f64 LE`).
   - `ref(Obj)` (WLib stack type `1`, serialized by `WLibArgFrame::Pack_ObjRef` at `0x14114E5F0` and deserialized by `WLibArgFrame::Unpack_ObjRef` at `0x14114E910`):
     - First writes a 32-bit `class_id` (`u32 LE`):
       - `0x00000000`: Null object reference (no further bytes).
       - `0x00000002`: `PString` (8-bit ANSI/UTF-8 string: packed length prefix followed by raw bytes).
       - `0x00000010`: `WString` (UTF-16LE string: packed character count followed by `count * 2` bytes of UTF-16LE code units).
       - `0x0000000E`: `PropertyCollection` (`CPropertyCollection` WState container).
       - Any WState-serializable WLib or native class ID (e.g., `0x0000044D` `Fellow`, `0x0000044F` `Fellowship`, `0x00000454` `Raid`, `0x000005C3` `TradeOfferData`, `0x00000193` `Money`, `0x000002B5` `FriendIgnoreEntry`, `0x000001CF`/`0x000004BE` `WhoQueryFilter`, `0x0000102F`/`0x00001030` `WhoResultPlayer`, `0x00000169`/`0x0000048C` `Guild`, `0x0000016A`/`0x0000048F` `GuildMember`, `0x0000016B`/`0x00000490` `GuildRank`, `0x00001681`/`0x00001682`/`0x00001685` `GuildCalendar`, `0x00000804` `MusicNoteBatch`).
     - When `class_id` is a WState class (not `0`, `2`, `0x10`, or `0x0E`), `Pack_ObjRef` invokes `WState::PackClass` (`0x14114DFD0`), which writes the class schema/version header and invokes virtual slot `2` (`Serialize(WState*)`) on the object instance.

---

## 1. Domain 1: Fellowships & Raids <a id="domain-1-fellowships-raids"></a>

### 1.1 Architecture & WLib Class Hierarchy

The Fellowship and Raid subsystem spans five WLib client/server class pairs and six WState data classes (`BIN-WLIB-P4`):

| Subsystem | Server Class (`send_msg` Target) | Client Handler Class | Client Base / Helper Class | Native UI Bridge Package |
|---|---|---|---|---|
| **Fellowship** | `0x04FE` (`1278`, `ServerFellowship`) | `0x0452` (`1106`) → `0x019D` (`413`, `ClientFellowship`) | `0x0453` (`1107`), `0x074D` (`1869`, ReadyCheck) | `0x013F` (`319`, `UIFellowship`) |
| **Raid** | `0x0523` (`1315`, `ServerRaid`) | `0x0123` (`291`, `ClientRaid`) | `0x0454` (`1108`, `Raid` data) | `0x012C` (`300`), `0x012D` (`301`) |
| **Fellowship Vote** | `0x0CFE` (`3326`, `ServerFellowshipVote`) | `0x0D00` (`3328`, `ClientFellowshipVote`) | `0x0D06` (`3334`, `VoteData`) | `0x0CD6` (`3286`) |
| **Target Marking** | `0x0739` (`1849`, `ServerTargetMarking`) | `0x0736` (`1846`, `ClientTargetMarking`) | `0x0734` (`1844`), `0x0735` (`1845`) | `0x070A` (`1802`), `0x074A` (`1866`) |
| **Fellowship Maker (LFF)** | `0x0C41` (`3137`, `ServerFellowshipMaker`) | `0x0C3D` (`3133`, `ClientFellowshipMaker`) | `0x0C3B` (`3131`), `0x0C36` (`3126`), `0x0FC7` (`4039`) | — |

Slash commands (`/fellowship` at `0x0089A0`, `/invite` via `0x819D002C`, `/readycheck` via `0x819D0031`, `/raid` at `0x008FA0`) and C++ UI callbacks (`0x140A9333E`, `0x140A9C6C6`, `0x140AA0500`, `0x140AA0696`) invoke `0x019D` (`ClientFellowship`) methods, which validate state against the local player's `Fellowship` object (`0x044F`, stored at `ClientFellowship+0x20`) and dispatch `0x04FE` RPCs to the server.

### 1.2 `ServerFellowship` (`0x04FE`) Client-to-Server RPCs (21 Methods)

| RPC Method ID | Wrapper Method | Client Caller (`0x019D` / `0x074D`) | Wire Parameters (`send_msg` order) | `SocialCommand` / Action Semantics |
|---|---|---|---|---|
| `0x84FE0041` | `0x84FE00B7` | `0x819D0008` (`/fellowship transferleadership`) | `u64 target_iid` | **`SocialCommand::SetFellowshipLeader`** — Transfer fellowship leadership to member `target_iid`. Requires caller IID == `Fellowship.leader_iid`. |
| `0x84FE0042` | `0x84FE00B4` | `0x819D0006` / `0x819D0007` (`/fellowship dismiss`) | `u64 target_iid` | **`SocialCommand::KickFromFellowship`** — Dismiss/kick member `target_iid` from the fellowship. |
| `0x84FE0048` | `0x84FE00AD` | `0x819D0051` (C++ UI `0x140A9333E`) | `u64 target_iid` | **`SocialCommand::InviteToFellowship` (by IID)** — Invite player `target_iid` to fellowship from UI context menu / unit frame. |
| `0x84FE0056` | `0x84FE00AE` | `0x819D0003` (`/fellowship disband`) | *(void)* | **`SocialCommand::DisbandFellowship`** — Disband the fellowship (leader only). |
| `0x84FE0057` | `0x84FE00B3` | `0x819D002C` (`/invite <name>`) | `ref(Obj) target_name` (`WString`) | **`SocialCommand::InviteToFellowship` (by Name)** — Recruit/invite player by character name. |
| `0x84FE0058` | `0x84FE00AF` | `0x819D0005` (`/fellowship recruit`) | `u64 target_iid` | **`SocialCommand::InviteToFellowship` (Selected Target)** — Recruit currently selected target `target_iid`. |
| `0x84FE0059` | `0x84FE00B0` | `0x819D0004` (`/fellowship leave`) | *(void)* | **`SocialCommand::LeaveFellowship`** — Leave the current fellowship. |
| `0x84FE0062` | `0x84FE00B8` | `0x819D0012` (`/fellowship lootrule`) | `u32 loot_rule` | **Set Loot Rule** — Leader sets fellowship loot distribution rule (`2` = Round Robin, `3` = Free For All, `4` = Need/Greed, `5` = Master Looter, `7` = Roll/Pass). |
| `0x84FE0065` | `0x84FE00B9` | `0x819D0013` (`/fellowship lootquality`) | `u32 loot_quality` | **Set Loot Quality Threshold** — Leader sets minimum item quality for roll/master-loot (`2` = Uncommon, `3` = Rare, `4` = Incomparable, `5` = Epic/Legendary). |
| `0x84FE0070` | `0x84FE00B5` | `0x819D0009` (Accept) / `0x819D000A` (Decline) | `u32 response`, `u32 token`, `u64 recruiter_iid` | **`SocialCommand::AnswerInvite`** — Respond to pending fellowship/raid invitation. `response`: `1` = Accept, `0` = Decline. `token` is read from `PID:Fellowship_ConfirmationToken` (`0x100020CF`) and `recruiter_iid` from `PID:Fellowship_Recruiter` (`0x100020CA`); clears `PID:Fellowship_Recruitment_Pending` (`0x100020CC`). |
| `0x84FE007C` | `0x84FE00DA` | `0x819D0010` (`/fellowship raid`) | *(void)* | **Convert Fellowship to Raid** — Converts the current fellowship into a Raid (leader only, requires `is_raid == 0`). |
| `0x84FE007D` | `0x84FE00DB` | `0x819D0011` | `u64 target_iid` | **Promote / Assist Member** — Leader member management action on `target_iid`. |
| `0x84FE00A7` | `0x84FE00DC` | `0x819D0031` (`/readycheck`) | *(void)* | **Start Fellowship Ready Check** — Initiates a ready check (rate-limited by `PID:Antispam_FellowshipReadyCheckRequest` `0x100018BF`). |
| `0x84FE00AA` | `0x84FE00DD` | `0x874D0001` (`ReadyCheckDialog`) | `u32 ready_state` | **Respond to Ready Check** — `1` = Ready, `0` = Not Ready. |
| `0x84FE00EB` | `0x84FE010E` | `0x819D003F` | `u32 skill_did`, `u64 fellow_iid` | **Set Desired Conjunction (Fellowship Manoeuvre) Skill** — Sets `PID:Fellowship_DesiredConjunctionSkill_DataID` (`0x10002BF2`). |
| `0x84FE00EC` | `0x84FE010F` | `0x819D0040` | `u64 fellow_iid` | **Clear Desired Conjunction Skill** — Clears `PID:Fellowship_DesiredConjunctionSkill_DataID` (`0x10002BF2`). |
| `0x84FE0114` | `0x84FE011D` | `0x819D0048` / `0x819D004A` (`0x140AA0500`) | `u64 target_iid` | **Toggle Fellowship Voice / Member Flag** — Toggles member state flag on `target_iid`. |
| `0x84FE0147` | `0x84FE014B` | `0x819D0067` | `u64 target_iid` | **Add Target Assist** — Adds `target_iid` as a Fellowship Target Assist (leader only). |
| `0x84FE0149` | `0x84FE014C` | `0x819D0066` | `u64 target_iid` | **Remove Target Assist** — Removes `target_iid` from Fellowship Target Assist list (leader only). |
| `0x84FE0161` | `0x84FE016E` | `0x819D007F` (`0x140A9C6C6`) | `u64 target_iid` | **Add Auto-Fellow Target** — Adds `target_iid` to the open/auto-fellowship list. |
| `0x84FE0162` | `0x84FE016F` | `0x819D0080` (`0x140AA0696`) | `u64 target_iid` | **Remove Auto-Fellow Target** — Removes `target_iid` from the open/auto-fellowship list. |

### 1.3 `ServerFellowship` (`0x04FE` → `0x0452` → `0x019D`) Server-to-Client RPCs (35 Methods)

| RPC Method ID | Client Handler (`0x0452` → `0x019D`) | Native UI Hook (`0x013F`) | Wire Parameters (`send_msg` order) | `SocialEvent` / Notification Semantics |
|---|---|---|---|---|
| `0x84FE0043` | `0x84520043` → `0x819D0026` | `0x013F0004` | `u32 reason`, `u64 fellow_iid` | **`SocialEvent::FellowshipChanged` (Member Removed/Dismissed)** — Removes `fellow_iid` from `Fellowship.members` (`reason`: `1` = voluntary leave, `2` = dismissed/kicked). |
| `0x84FE0044` | `0x84520044` → `0x819D0016` | `0x013F0003` | `ref(Obj) fellow` (`0x044D` `Fellow`) | **`SocialEvent::FellowshipChanged` (Member Joined)** — Inserts `Fellow` record into `Fellowship.members` hash table (`0x844F0002`) and updates UI. |
| `0x84FE0045` | `0x84520045` → `0x819D001A` | `0x013F0006` | `u64 new_leader_iid` | **`SocialEvent::FellowshipChanged` (Leader Changed)** — Updates `Fellowship.leader_iid` (`+0x28`) and notifies UI. |
| `0x84FE0046` | `0x84520046` | `0x013F000B` | *(void)* | **`SocialEvent::FellowshipChanged` (Disbanded)** — Clears `ClientFellowship+0x20` (`0x8453000D`) and notifies UI that the fellowship disbanded. |
| `0x84FE0047` | `0x84520047` → `0x819D0023` | `0x013F0007` | `ref(Obj) fellow` (`0x044D` `Fellow`) | **`SocialEvent::FellowVitalChanged` (Full Member Update)** — Replaces `Fellow` entry in `Fellowship.members` and refreshes member vitals/class/level in UI. |
| `0x84FE0049` | `0x84520049` → `0x819D003E` | — | `u32 error_code` | **Fellowship Error** — Displays localized error message for `error_code`. |
| `0x84FE004A` | `0x8452004A` → `0x819D001F` | — | `u32 reason`, `u64 target_iid` | **Recruit/Action Failure** — Displays recruitment or member action failure reason for `target_iid`. |
| `0x84FE004B` | `0x8452004B` → `0x819D003D` | `0x013F000D` | `u32 status_code` | **Fellowship Status Update** — General fellowship state notification. |
| `0x84FE004C` | `0x8452004C` → `0x819D0028` | `0x013F000F` | `u32 loot_rule`, `u64 actor_iid` | **Loot Rule Changed** — Updates `Fellowship.loot_rule` (`+0x30`) and logs change. |
| `0x84FE004D` | `0x8452004D` → `0x819D0017` | `0x013F0010` | `u32 loot_quality`, `u64 actor_iid` | **Loot Quality Changed** — Updates `Fellowship.loot_quality` (`+0x34`) and logs change. |
| `0x84FE004E` | `0x8452004E` → `0x819D001B` | `0x013F0008` | `u32 is_linkdead`, `u64 fellow_iid` | **Member Link-Dead / Online Status Changed** — Updates `Fellow.is_linkdead` (`+0x60`) for `fellow_iid`. |
| `0x84FE004F` | `0x8452004F` → `0x819D0024` | — | `u32 token`, `ref(Obj) recruiter_name` (`WString`), `u32 is_raid`, `u64 recruiter_iid` | **`SocialEvent::FellowshipInvite`** — Incoming fellowship (`is_raid == 0`) or raid (`is_raid != 0`) invitation! Stores `recruiter_iid` in `PID:0x100020CA`, `recruiter_name` in `PID:0x100020CB`, `1` in `PID:0x100020CC`, `is_raid` in `PID:0x100020CD`, and `token` in `PID:0x100020CF`. If `PID:Fellowship_AutoDeclineInvites` is set, immediately replies with `0x84FE0070(0, token, recruiter_iid)`. |
| `0x84FE0050` | `0x84520050` → `0x819D0027` | — | `u32 response_code`, `ref(Obj) invitee_name` (`WString`), `u64 invitee_iid` | **Invitation Response Notice** — Notifies the recruiter whether `invitee_name` accepted, declined, or was busy/already grouped. |
| `0x84FE0051` | `0x84520051` → `0x819D0022` | `0x013F0009` | `u32 code`, `u32 arg2`, `ref(Obj) name`, `u64 iid1`, `u64 iid2` | **Fellowship Recruitment Broadcast** — Notifies fellowship members of an invitation or recruitment event. |
| `0x84FE0052` | `0x84520052` → `0x819D0020` | `0x013F0001` | `ref(Obj) fellowship` (`0x044F` `Fellowship`) | **`SocialEvent::FellowshipChanged` (Full State Sync)** — Replaces the client's `0x044F` `Fellowship` object (`+0x20`), synchronizes target markers (`0x8736000F`), and refreshes the entire Fellowship UI (`0x013F0001`). |
| `0x84FE0053` | `0x84520053` → `0x819D001D` | `0x013F0005` | `u64 fellow_iid` | **Member Departed** — Removes `fellow_iid` from `Fellowship.members` (`0x844F0003`) and fires `0x013F0005`. |
| `0x84FE0054` | `0x84520054` | `0x013F0002` | *(void)* | **Local Player Left Fellowship** — Clears `ClientFellowship+0x20` (`0x8453000D`) and fires `0x013F0002`. |
| `0x84FE0055` | `0x84520055` → `0x819D001C` | `0x013F000C` | `u32 pet_type`, `u64 pet_iid`, `u64 owner_iid` | **Fellowship Pet Added** — Constructs `0x044E` (`FellowPet`) for `owner_iid` (`0x844D0022`) and fires `0x013F000C`. |
| `0x84FE005A` | `0x8452005A` → `0x819D002D` | `0x013F0011` | `ref(Obj) pos_data`, `u64 arg2`, `u64 fellow_iid` | **Member Map Position Update** — Updates radar/map position for `fellow_iid`. |
| `0x84FE005B` | `0x8452005B` → `0x819D002E` | `0x013F000C` | `u64 pet_iid`, `u64 owner_iid` | **Fellowship Pet Removed** — Removes `pet_iid` from `owner_iid`'s `Fellow` record (`0x844D0024`). |
| `0x84FE006B` | `0x8452006B` → `0x819D0033` | `0x013F0013` | `u32 ready_state`, `u64 fellow_iid` | **Ready Check Member Response** — Updates ready check icon (`1` = Ready, `0` = Not Ready) for `fellow_iid`. |
| `0x84FE006C` | `0x8452006C` → `0x819D0032` | — | *(void)* | **Ready Check Initiated** — Opens the `0x074D` / `0x0D03` Ready Check prompt for the local player. |
| `0x84FE0071` | `0x84520071` → `0x819D0034` | `0x013F0014` | `u64 voice_flags`, `u64 fellow_iid` | **Member Voice Status Updated** — Updates voice-chat state on `fellow_iid` (`0x844D0008`). |
| `0x84FE007B` | `0x8452007B` → `0x819D003A` | — | `ref(Obj) pe_data` | **Private Encounter Fellowship Update** — Updates instance/encounter state. |
| `0x84FE0082` | `0x84520082` → `0x819D003B` | — | *(void)* | **Purge Stale Fellowship State** — Calls `0x8453003B`. |
| `0x84FE0087` | `0x819D002F` | `0x070A0002` | `u32 marker_id`, `u64 target_iid` | **Target Marker Cleared** — Forwards to `ClientTargetMarking::ClearTargetMark` (`0x8736000D`). |
| `0x84FE0089` | `0x819D0030` | `0x070A0001` | `u32 marker_id`, `u64 target_iid` | **Target Marker Assigned** — Forwards to `ClientTargetMarking::SetTargetMark` (`0x87360011`). |
| `0x84FE008A` | `0x8452008A` → `0x819D0042` | — | `u32 skill_did` | **Conjunction Skill Cleared Notice** — Notification that desired Fellowship Manoeuvre skill was cleared. |
| `0x84FE008B` | `0x8452008B` → `0x819D0041` | — | `u32 skill_did` | **Conjunction Skill Selected Notice** — Notification that desired Fellowship Manoeuvre skill was set. |
| `0x84FE008C` | `0x8452008C` → `0x819D0049` | `0x013F0017` | `u32 mask`, `u32 cur_morale`, `u32 max_morale`, `u32 cur_power`, `u32 max_power`, `u64 fellow_iid` | **`SocialEvent::FellowVitalChanged` (Fast Vitals Delta)** — Compact 28-byte update of `fellow_iid`'s morale and power without re-sending the full `0x044D` `Fellow` object (`0x844D0027`). |
| `0x84FE0097` | `0x84520097` → `0x819D0056` | — | `u32 vote_type`, `u64 target_iid`, `u64 initiator_iid` | **Fellowship Vote Started** — Notification of fellowship vote. |
| `0x84FE0098` | `0x84520098` → `0x819D0057` | — | `ref(Obj) name`, `u32 vote_type`, `u64 target_iid`, `u64 initiator_iid` | **Fellowship Vote Cast / Progress** — Notification of vote progress. |
| `0x84FE0099` | `0x84520099` → `0x819D0058` | — | `u64 target_iid`, `u64 initiator_iid` | **Fellowship Vote Ended** — Notification of vote completion. |
| `0x84FE00A0` | `0x845200A0` → `0x819D006F` | `0x013F0021` | `u64 target_iid` | **Target Assist Removed** — Removes `target_iid` from `Fellowship.target_assists` (`0x844F000F`). |
| `0x84FE00A1` | `0x845200A1` → `0x819D006E` | `0x013F0020` | `u64 target_iid` | **Target Assist Added** — Adds `target_iid` to `Fellowship.target_assists` (`0x844F000E`). |
| `0x84FE00A3` | `0x845200A3` → `0x819D0074` | `0x013F0027` | `u32 status`, `u64 arg2`, `u64 fellow_iid` | **Member Instance Eligibility Updated** — Updates instance join state (`0x844D002D`). |
| `0x84FE00A5` | `0x845200A5` → `0x819D0075` | — | `ref(Obj) data` | **Fellowship Instance Data Updated** — Updates fellowship instance state (`0x844F0010`). |
| `0x84FE0164` | `0x819D007C` | — | `u64 target_iid` | **Auto-Fellow Removed** — Removes `target_iid` from auto-fellow set (`0x819D0079`). |
| `0x84FE0165` | `0x819D007D` | — | `u64 target_iid` | **Auto-Fellow Added** — Adds `target_iid` to auto-fellow set (`0x819D007B`). |

### 1.4 `ServerRaid` (`0x0523` ↔ `0x0123`), `ServerFellowshipVote` (`0x0CFE` ↔ `0x0D00`), `ServerTargetMarking` (`0x0739` ↔ `0x0736`), and `ServerFellowshipMaker` (`0x0C41` ↔ `0x0C3D`)

#### 1.4.1 Raid RPCs (`0x0523` ↔ `0x0123`)

| RPC Method ID | Dir | Client Caller / Handler | Wire Parameters (`send_msg` order) | Meaning |
|---|---|---|---|---|
| `0x85230001` | C→S | `0x85230037` ← `0x81230003` (`/raid disband`) | *(void)* | Disband the entire Raid (raid leader only). |
| `0x85230002` | C→S | `0x85230038` ← `0x81230004` (`/raid leave`) | *(void)* | Leave the Raid. |
| `0x85230003` | C→S | `0x85230039` ← `0x81230006` (`/raid dismiss <name>`) | `ref(Obj) name` (`WString`) | Dismiss/kick member from Raid by character name. |
| `0x85230004` | C→S | `0x8523003A` ← `0x81230005` (`/raid dismiss`) | `u64 target_iid` | Dismiss/kick member from Raid by `target_iid`. |
| `0x85230005` | C→S | `0x8523003C` ← `0x81230007` (`/raid transferleadership`) | `u64 target_iid` | Transfer Raid leadership to `target_iid`. |
| `0x85230006` | C→S | `0x8523003E` ← `0x8123000C` (`/raid promote`) | `u64 target_iid` | Promote Raid member to Raid Assistant (`0x84540014`). |
| `0x85230007` | C→S | `0x8523003F` ← `0x8123000D` (`/raid demote`) | `u64 target_iid` | Demote Raid Assistant (`0x84540015`). |
| `0x85230008` | C→S | `0x8523003D` ← `0x8123000B` (`/raid move`) | `u32 dest_group_idx`, `u64 target_iid` | Move member `target_iid` to subgroup `dest_group_idx` (`0..3`). |
| `0x85230009` | C→S | `0x8523003B` ← `0x8123000A` (`/raid swap`) | `u64 member_b_iid`, `u64 member_a_iid` | Swap two members between Raid subgroups. |
| `0x8523000A` | C→S | `0x85230040` ← `0x81230010` | `u32 loot_rule` | Set Raid loot rule (`2..8`). |
| `0x8523000B` | C→S | `0x85230041` ← `0x81230011` | `u32 loot_quality` | Set Raid loot quality threshold (`2..5`). |
| `0x85230025` | C→S | `0x85230042` ← `0x81230029` | *(void)* | Query Raid Locks (`/raid locks`). |
| `0x8523000C` | S→C | `0x81230013` | `ref(Obj) raid` (`0x0454` `Raid`) | Full Raid state synchronized (`0x012C0001`). |
| `0x8523000D` | S→C | `0x81230014` | *(void)* | Local player left Raid (`0x012C0002`). |
| `0x8523000E` | S→C | `0x81230015` | *(void)* | Raid disbanded (`0x012C000B`). |
| `0x8523000F` | S→C | `0x81230016` | `u32 group_idx`, `ref(Obj) fellow` (`0x044D`) | Member joined Raid subgroup `group_idx` (`0x012C0003`). |
| `0x85230010` | S→C | `0x8123001E` | `ref(Obj) fellow` (`0x044D`) | Raid member info updated (`0x012C0007`). |
| `0x85230011` | S→C | `0x81230021` | `u32 reason`, `u64 fellow_iid` | Member dismissed/left Raid (`0x012C0004`). |
| `0x85230012` | S→C | `0x8123001C` | `u64 fellow_iid` | Member departed Raid (`0x012C0005`). |
| `0x85230013` | S→C | `0x81230023` | `u32 dest_group_idx`, `u64 fellow_iid` | Member moved to subgroup `dest_group_idx` (`0x012C000E`). |
| `0x85230014` | S→C | `0x81230024` | `u64 member_b_iid`, `u64 member_a_iid` | Two Raid members swapped subgroups (`0x012C000F`). |
| `0x85230015` | S→C | `0x81230019` | `u64 new_leader_iid` | Raid leader changed (`0x012C0006`). |
| `0x85230016` | S→C | `0x8123001A` | `u32 is_assistant`, `u64 fellow_iid` | Raid assistant status changed (`0x012C0009` / `0x012C000A`). |
| `0x85230017` | S→C | `0x8123001B` | `u32 is_linkdead`, `u64 fellow_iid` | Raid member link-dead status changed (`0x012C0008`). |
| `0x85230018` | S→C | `0x8123001D` | `u32 code`, `u32 arg2`, `ref(Obj) name`, `u64 iid1`, `u64 iid2` | Raid recruitment notification. |
| `0x85230019` | S→C | `0x81230020` | `u32 response_code`, `ref(Obj) name`, `u64 iid` | Raid invite response notice. |
| `0x8523001A` | S→C | `0x81230025` | `u32 error_code` | Raid error notification. |
| `0x8523001B` | S→C | `0x8123001F` | `u32 reason`, `u64 target_iid` | Raid action failure notice. |
| `0x8523001C` | S→C | `0x81230026` | `u32 status_code` | Raid status notification (`0x012C000D`). |
| `0x8523001D` | S→C | `0x81230022` | `u32 loot_rule`, `u64 actor_iid` | Raid loot rule changed (`0x012C0010`). |
| `0x8523001E` | S→C | `0x81230017` | `u32 loot_quality`, `u64 actor_iid` | Raid loot quality changed (`0x012C0011`). |
| `0x8523001F` | S→C | `0x81230027` | `u32 group_idx`, `u64 leader_iid` | Subgroup leader changed (`0x012C0012`). |
| `0x85230020` | S→C | `0x81230028` | `u32 group_idx`, `u64 voice_id` | Subgroup voice room updated (`0x012C0013`). |
| `0x85230026` | S→C | `0x8123002A` | `ref(Obj) locks_list` | Raid locks query response (`0x012D0001`). |
| `0x85230029` | S→C | `0x8123002C` | *(void)* | Purge stale Raid state (`0x8454001B`). |
| `0x85230033` | S→C | `0x8123002D` | `u32 mask`, `u32 cur_m`, `u32 max_m`, `u32 cur_p`, `u32 max_p`, `u64 fellow_iid` | Raid member fast vitals delta (`0x012C0014`). |

#### 1.4.2 Fellowship Vote (`0x0CFE` ↔ `0x0D00`), Target Marking (`0x0739` ↔ `0x0736`), and Fellowship Maker (`0x0C41` ↔ `0x0C3D`)

| Subsystem | RPC Method ID | Dir | Client Method | Wire Parameters (`send_msg` order) | Meaning |
|---|---|---|---|---|---|
| **Vote** | `0x8CFE0002` | C→S | `0x8CFE000D` ← `0x8D000001` | `u32 response`, `u32 vote_id` | Cast ballot (`response`: `1`=Yes, `0`=No) on active fellowship vote `vote_id`. |
| **Vote** | `0x8CFE0003` | S→C | `0x8D000002` | `ref(Obj) vote_data` (`0x0D06`) | Fellowship vote started (`0x0CD60001`). |
| **Vote** | `0x8CFE0004` | S→C | `0x8D000003` | `u32 reason`, `u32 vote_id` | Fellowship vote ended/cancelled (`0x0CD60002`). |
| **Vote** | `0x8CFE0005` | S→C | `0x8D000004` | `u32 error_code`, `u32 vote_id` | Fellowship vote error (`0x0CD60003`). |
| **TargetMark** | `0x87390001` | C→S | `0x87390003` ← `0x87360002` | `u32 marker_id`, `u64 target_iid` | Assign fellowship/raid target marker `marker_id` (`0..9`) to `target_iid` (`0` clears). |
| **LFF Maker** | `0x8C410002` | C→S | `0x8C410011` ← `0x8C3D0001` | `ref(Obj) entry` (`0x0C3B`) | Create/update LFF Fellowship Maker posting. |
| **LFF Maker** | `0x8C410003` | C→S | `0x8C410012` ← `0x8C3D0002` | *(void)* | Remove LFF Fellowship Maker posting. |
| **LFF Maker** | `0x8C410004` | C→S | `0x8C410013` ← `0x8C3D0003` | `ref(Obj) filter` (`0x0C36`) | Search LFF Fellowship Maker directory. |
| **LFF Maker** | `0x8C410009` | C→S | `0x8C410014` ← `0x8C3D0009` | `u64 leader_iid` | Request to join LFF group led by `leader_iid`. |
| **LFF Maker** | `0x8C410005` | S→C | `0x8C3D0004` | `ref(Obj) results_array` (`Array<0x0C3B>`) | LFF directory search results. |
| **LFF Maker** | `0x8C410006` | S→C | `0x8C3D0005` | `ref(Obj) entry` (`0x0C3B`) | LFF posting updated. |
| **LFF Maker** | `0x8C410007` | S→C | `0x8C3D0006` | `u32 status_code` | LFF status/error notification. |
| **LFF Maker** | `0x8C41000A` | S→C | `0x8C3D000A` | `ref(Obj) applicant_info` (`0x0FC7`) | Incoming LFF join request to leader. |

### 1.5 WState Data Class Layouts (`0x044D` `Fellow`, `0x044E` `FellowPet`, `0x044F` `Fellowship`, `0x0454` `Raid`)

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

## 2. Domain 2: Player-to-Player Trade (`SecureTrade`) <a id="domain-2-trade"></a>

### 2.1 Architecture & WLib Class Hierarchy

Player-to-player trading is implemented by `ServerTrade` (`0x0531`, decimal `1329`) and `ClientTrade` (`0x017F`, decimal `383`), backed by the WState class `TradeOfferData` (`0x05C3`, decimal `1475`), the native currency container `Money` (`0x0193`, decimal `403`), and the native UI notification package `UITrade` (`0x0163`, decimal `355`) (`BIN-WLIB-P4`, `BIN-TRADE-NAT`).

Slash commands (`/trade` at `0x009708`: `open`, `close`, `accept`, `decline`, `offer`, `revoke`, `reset`, `dump`, `query`) and C++ UI trade window callbacks (`0x140B5D646`, `0x140B4E8DF`, `0x140B4D2CF`, `0x140B4EC5F`, `0x140B5B236`, `0x140B5B106`, `0x140B5F2B6`) invoke `0x017F` (`ClientTrade`) methods.

### 2.2 `ServerTrade` (`0x0531`) Client-to-Server RPCs (9 Methods)

| RPC Method ID | Wrapper Method | Client Caller (`0x017F` & C++ VA) | Wire Parameters (`send_msg` order) | `SocialCommand` / Action Semantics |
|---|---|---|---|---|
| `0x85310043` | `0x8531006C` | `0x817F0001` (`/trade open`, `0x140B5D646`) | `u64 target_iid` | **`SocialCommand::OpenTrade`** — Request a secure trade session with player `target_iid`. Checks that local player is not already in a trade (`ClientTrade+0x28 == 0`), is within trade distance (`0x817F0050`), and `target_iid != local_player_iid`. Sets local trade state `+0x28 = 1` (Pending) and `+0x10 = target_iid`. |
| `0x8531005D` | `0x85310070` | `0x817F0002` (`/trade close`, `0x140B4E8DF`) | *(void)* | **Cancel / Close Trade** — Cancels an active or pending trade session (`ClientTrade+0x28` in `1..3`) and resets local trade state (`0x817F006B`). |
| `0x85310050` | `0x85310069` | `0x817F0003` (`/trade accept`, `0x140B4D2CF`) | `ref(Obj) offer_snapshot` (`0x05C3` `TradeOfferData`) | **`SocialCommand::SetTradeReady` (Accept / Lock Trade)** — Sends the client's complete `0x05C3` `TradeOfferData` snapshot (`ClientTrade+0x20`, containing both master and slave item lists and `0x0193` `Money` offers) so the server can verify exact offer equality before committing the trade. Sets local player's acceptance state to `1` (`0x85C3002D`) and state `+0x28 = 3`. |
| `0x85310034` | `0x85310071` | `0x817F0004` (`/trade decline`, `0x140B4EC5F`) | *(void)* | **Decline Trade** — Declines an incoming trade request (`ClientTrade+0x28 == 1`) and resets state (`0x817F006B`). |
| `0x85310024` | `0x8531006E` | `0x817F0005` (`/trade offer`, `0x140B5B236`) | `u64 item_iid` | **`SocialCommand::OfferTrade` (Add Item)** — Adds inventory item `item_iid` to the local player's trade offer. Requires trade state `+0x28 == 2`, item not already in offer (`0x85C30019`), and item passing `CanTradeItem` check (`0x817F005A`: not bound, not locked, in local inventory). |
| `0x8531002E` | `0x8531006A` | `0x817F0006` (`0x140B5B106`) | `ref(Obj) money` (`0x0193` `Money`) | **`SocialCommand::OfferTrade` (Set Coin)** — Sets the local player's offered coin amount. `0x817F0006` takes a `u64 copper_amount`, decomposes it into denominations (`1` = copper `% 100`, `2` = silver `(copper / 100) % 1000`, `3` = gold `(copper / 100000) % 1000`, `4` = platinum `copper / 100000000`) inside a `0x0193` `Money` object via `0x01930001`, updates `TradeOfferData` (`0x85C30005`), and sends the `0x0193` object to the server. |
| `0x85310060` | `0x8531006D` | `0x817F0007` (`/trade revoke`, `0x140B5F2B6`) | `u64 item_iid` | **`SocialCommand::OfferTrade` (Remove Item)** — Removes `item_iid` from the local player's trade offer (`0x85C30019` must be true). |
| `0x85310042` | `0x8531006B` | `/trade reset` (`0x009708`) | *(void)* | **Reset Trade Offer** — Resets the current trade offer and unlocks acceptance. |
| `0x85310025` | `0x8531006F` | `/trade query srv` (`0x009708`) | `u64 target_iid` | **Admin Query Server Trade State** — Debug/admin command. |

### 2.3 `ServerTrade` (`0x0531` → `0x017F`) Server-to-Client RPCs (12 Methods)

| RPC Method ID | Client Handler (`0x017F` → Impl) | Native UI Hook (`0x0163`) | Wire Parameters (`send_msg` order) | `SocialEvent` / Notification Semantics |
|---|---|---|---|---|
| `0x8531000F` | `0x817F008B` → `0x817F007C` | `0x01630001` (`OnTradeOpened`) | `ref(Obj) extra`, `u64 slave_iid`, `u64 master_iid` | **`SocialEvent::TradeShown`** — Trade session opened between `master_iid` (initiator) and `slave_iid` (recipient). Allocates `0x05C3` (`TradeOfferData` at `ClientTrade+0x20`), records `master_iid` (`0x85C30016`) and `slave_iid` (`0x85C30012`), sets local state `+0x28 = 2` (Open), and opens the UI Trade window (`0x01630001`). If `PID:Trade_AutoDeclineRequests` is enabled on the recipient, automatically calls `0x817F0004` (`DeclineTrade`). |
| `0x85310046` | `0x817F007B` → `0x817F008A` | `0x01630002` (`OnTradeReset`) | *(void)* | **Trade Session Reset** — Clears both players' acceptance locks (`0x85C3002B`) and sets local state `+0x28 = 2` (`0x01630002`). |
| `0x85310061` | `0x817F008C` → `0x817F0062` | `0x01630003` (`OnTradeAccepted`) | `u32 is_accepted`, `u64 player_iid` | **`SocialEvent::TradeReady`** — Updates the trade lock/ready indicator (`is_accepted`: `1` = Ready/Locked, `0` = Unready) for `player_iid` (`0x85C3002D`) and fires `0x01630003`. |
| `0x8531004E` | `0x817F007E` → `0x817F0064` | `0x01630004` (`OnMyItemsChanged`) | `u64 item_iid` | **`SocialEvent::TradeOffer` (Local Item Added)** — Confirms `item_iid` added to the local player's offer list (`0x85C30001`), resets both ready locks (`0x85C3002C(0)`), and refreshes the local offer pane (`0x01630004`). |
| `0x8531004C` | `0x817F0090` → `0x817F003B` | `0x01630005` (`OnPartnerItemsChanged`) | `u64 item_iid` | **`SocialEvent::TradeOffer` (Partner Item Added)** — Adds `item_iid` to the trade partner's offer list (`0x85C30023`), resets both ready locks (`0x85C3002C(0)`), and refreshes the partner offer pane (`0x01630005`). |
| `0x85310068` | `0x817F0083` → `0x817F000E` | `0x01630004` & `0x01630005` | `u64 item_iid` | **`SocialEvent::TradeOffer` (Item Revoked)** — Removes `item_iid` from whichever offer list contains it (`0x85C30025`), resets ready locks (`0x85C3002C(0)`), and refreshes both offer panes. |
| `0x8531000B` | `0x817F0091` → `0x817F006A` | `0x01630008` (`OnMoneyCopperChanged`) | `u64 copper_amount`, `u64 player_iid` | **`SocialEvent::TradeOffer` (Coin Copper Updated)** — Updates offered copper for `player_iid` (`0x85C30009`), resets ready locks (`0x85C3002C(0)`), and notifies UI (`0x01630008`). |
| `0x8531000E` | `0x817F0081` → `0x817F0030` | `0x01630009` (`OnMoneyObjChanged`) | `ref(Obj) money` (`0x0193`), `u64 player_iid` | **`SocialEvent::TradeOffer` (Coin Object Updated)** — Updates `0x0193` `Money` object for `player_iid` (`0x85C30005`), resets ready locks (`0x85C3002C(0)`), and notifies UI (`0x01630009`). |
| `0x8531005F` | `0x817F0088` → `0x817F002A` | `0x01630006` (`OnTradeCompleted`) | *(void)* | **Trade Completed (State Transition)** — Sets local trade state `+0x28 = 4` (Completed) and fires `0x01630006`. |
| `0x85310052` | `0x817F008D` → `0x817F0048` | `0x01630007` (`OnTradeClosed`) | `u32 result_code` | **Trade Finalized & Closed** — Prints chat summary of coins given/received (`0x817F0010`), closes the trade UI (`0x01630007`), and resets `ClientTrade` (`0x817F006B`). |
| `0x85310035` | `0x817F007D` → `0x817F0034` | `0x01630007` (`OnTradeClosed`) | `u32 cancel_reason`, `u64 actor_iid` | **Trade Cancelled / Aborted** — Closes the trade UI (`0x01630007`), displays cancellation reason (`cancel_reason`: `2` = cancelled, `3` = declined, `10` = busy, `17` = out of range), and resets `ClientTrade`. |
| `0x85310016` | `0x817F008F` → `0x817F000A` | `0x0163000A` (`OnTradeError`) | `u64 arg1`, `u64 target_iid` | **Trade Request Error** — Fires `0x0163000A` and resets `ClientTrade` if pending with `target_iid`. |

### 2.4 WState Layouts for `TradeOfferData` (`0x05C3`) and `Money` (`0x0193`)

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

#### 2.4.2 `Money` (`0x0193`, decimal `403`, Native Serializer `0x140A58410` → `0x140A57EB0`)

When `Pack_ObjRef` serializes a `0x0193` (`Money`) object, it writes `u32 class_id = 0x00000193` followed by the native `IntrusiveHashTable<u32, u32>` payload (`BIN-TRADE-NAT`):

| Offset | Size | Type | Field Name | Meaning |
|---|---|---|---|---|
| `+0x00` | `1` | `u8` | `bucket_index` | Index into the prime bucket-count table (`0x141A53688`; `0` for small maps). |
| `+0x01` | `1..4` | `PackedCount` | `entry_count` | Number of non-zero currency denomination entries (`0..4`), encoded via `PackCompactCount` (`0x140304D00`: `1` byte if `< 0x80`). |
| `+0x02` | `8 * N` | `(u32, u32)[N]` | `entries` | `entry_count` pairs of `(u32 denomination_id, u32 count)` where `denomination_id`: `1` = Copper (`0..99`), `2` = Silver (`0..999`), `3` = Gold (`0..999`), `4` = Platinum. |

---

## 3. Domain 3: Friends & Ignore Lists, `/who` & LFG, User Chat Channels, and Kinship (`Guild`) Roster & Calendar <a id="domain-3-friends-who-kinship"></a>

### 3.1 Friends & Ignore Lists and World Chat RPCs (`0x04EC` ↔ `0x0102`)

The World Server side of Friends, Ignore, `/anon`, `/afk`, and spatial/regional chat is handled by `ServerChat` (`0x04EC`, decimal `1260`) and `ClientChat` (`0x0102`, decimal `258`), using data class `0x02B5` (`FriendIgnoreEntry`, decimal `693`) (`BIN-WLIB-P4`).

#### 3.1.1 `ServerChat` (`0x04EC`) Client-to-Server RPCs (21 Methods)

| RPC Method ID | Wrapper Method | Client Caller (`0x0102`) | Wire Parameters (`send_msg` order) | `SocialCommand` / Action Semantics |
|---|---|---|---|---|
| `0x84EC0001` | `0x84EC001D` | `0x81020036` (`/say`, `/tell`) | `u32 chat_type`, `ref(Obj) text` (`WString`), `ref(Obj) target_name` (`WString`) | Send world-routed chat message or tell. |
| `0x84EC0002` | `0x84EC001E` | `0x8102003D` | `u32 channel_id`, `ref(Obj) text` (`WString`) | Send channel message via World Server. |
| `0x84EC0003` | `0x84EC001F` | `0x81020007` (`/friend add`) | `ref(Obj) name` (`WString`) | **`SocialCommand::AddFriend`** — Add character `name` to the player's Friends list (rate-limited by `PID:Antispam_AddFriendRequest` `0x100018BB`). |
| `0x84EC0004` | `0x84EC0020` | `0x81020008` (`/friend remove`) | `ref(Obj) name` (`WString`) | **`SocialCommand::RemoveFriend`** — Remove character `name` from the player's Friends list (`PID:Antispam_RemoveFriendRequest` `0x100018BC`). |
| `0x84EC0005` | `0x84EC0021` | `0x81020010` (`/ignore add`) | `u32 ignore_reason`, `ref(Obj) name` (`WString`) | **`SocialCommand::Ignore`** — Add character `name` to the Ignore list with reason code (`PID:Annotation_IgnoreReason` `0x10003086`; rate-limited by `PID:Antispam_AddIgnoreRequest` `0x100018BD`). |
| `0x84EC0006` | `0x84EC0022` | `0x81020011` (`/ignore remove`) | `ref(Obj) name` (`WString`) | **`SocialCommand::Unignore`** — Remove character `name` from the Ignore list (`PID:Antispam_RemoveIgnoreRequest` `0x100018BE`). |
| `0x84EC0007` | `0x84EC0023` | `0x81020009` (`/friend note`) | `ref(Obj) note` (`WString`), `ref(Obj) name` (`WString`) | **Set Friend Note** — Updates `PID:Annotation_Text` (`0x1000305D`) on friend `name`. |
| `0x84EC0008` | `0x84EC0024` | `0x81020016` (`/anon`) | `u32 is_anonymous` | **Set Anonymous Flag** — `1` = Anonymous (`PID:Communication_CharacterPresence` `0x10000DF0` set to `2`), `0` = Public (`1`). |
| `0x84EC0009` | `0x84EC0025` | `0x81020018` (`/afk`) | `ref(Obj) afk_msg` (`WString`), `u32 is_afk` | **Set AFK Status & Message** — Toggles AFK mode and optional auto-reply text. |
| `0x84EC000E` | `0x84EC0026` | `0x8102001F` | `u32 channel_type`, `u32 enable` | Opt in/out of world-managed regional/system chat channel. |
| `0x84EC0012` | `0x84EC0027` | `0x81020025` | `u32 filter_type`, `u32 enable` | Update server-side chat filter setting. |
| `0x84EC0015` | `0x84EC0028` | `0x81020028` | *(void)* | Request full Friends & Ignore list refresh. |
| `0x84EC0018` | `0x84EC002A` | `0x8102002C` | `ref(Obj) text` (`WString`), `u32 channel_id` | Send localized/formatted chat message. |
| `0x84EC001B` | `0x84EC0029` | `0x81020030` | `u64 target_iid`, `u32 reason` | Report spammer / chat abuse. |
| `0x84EC002C` | `0x84EC002F` | `0x81020042` (`/ignore note`) | `ref(Obj) note` (`WString`), `ref(Obj) name` (`WString`) | **Set Ignore Note** — Updates `PID:Annotation_Text` (`0x1000305D`) on ignored player `name`. |

#### 3.1.2 `ServerChat` (`0x04EC` → `0x0102`) Server-to-Client RPCs (12 Methods)

| RPC Method ID | Client Handler (`0x0102`) | Native UI Hook (`0x0140`) | Wire Parameters (`send_msg` order) | `SocialEvent` / Notification Semantics |
|---|---|---|---|---|
| `0x84EC000A` | `0x81020037` (vslot 54) | `0x01400001` | `ref(Obj) friend_map` (`HashTable<WString, ref(0x02B5)>`) | **`SocialEvent::FriendList` (Full Sync)** — Replaces the client's Friends hash table (`ClientChat+0x10`) and refreshes the Friends UI panel (`0x01400001`). |
| `0x84EC000B` | `0x8102003A` (vslot 57) | `0x01400005` | `ref(Obj) ignore_map` (`HashTable<WString, ref(0x02B5)>`) | **`SocialEvent::IgnoreList` (Full Sync)** — Replaces the client's Ignore hash table (`ClientChat+0x18`) and refreshes the Ignore UI panel (`0x01400005`). |
| `0x84EC000C` | `0x81020038` (vslot 55) | `0x01400002` / `0x01400004` | `u32 is_login_transition`, `ref(Obj) entry` (`0x02B5`) | **`SocialEvent::FriendList` (Single Friend Added/Updated)** — Inserts or updates `0x02B5` (`FriendIgnoreEntry`) in `ClientChat+0x10`. If `is_login_transition != 0`, prints the friend online/offline notification in chat (`0x01400004`). |
| `0x84EC000D` | `0x81020039` (vslot 56) | `0x01400003` | `ref(Obj) name` (`WString`) | **`SocialEvent::FriendList` (Friend Removed)** — Removes `name` from `ClientChat+0x10` and fires `0x01400003`. |
| `0x84EC000F` | `0x8102003B` (vslot 58) | `0x01400006` / `0x01400008` | `u32 is_update`, `ref(Obj) entry` (`0x02B5`) | **`SocialEvent::IgnoreList` (Single Ignore Added/Updated)** — Inserts or updates `0x02B5` in `ClientChat+0x18` and fires `0x01400006` (added) or `0x01400008` (updated). |
| `0x84EC0010` | `0x8102003C` (vslot 59) | `0x01400007` | `ref(Obj) name` (`WString`) | **`SocialEvent::IgnoreList` (Ignore Removed)** — Removes `name` from `ClientChat+0x18` and fires `0x01400007`. |
| `0x84EC0011` | `0x8102003E` (vslot 61) | `0x01400009` | `u32 error_code`, `ref(Obj) name` (`WString`) | **Friend/Ignore Error Notice** — Displays error message (e.g., player not found, list full). |
| `0x84EC0013` | `0x8102003F` (vslot 62) | — | `u32 is_anonymous` | **Anonymous Status Sync** — Updates `PID:Communication_CharacterPresence` (`0x10000DF0`). |
| `0x84EC0014` | `0x81020040` (vslot 63) | — | `u32 is_afk`, `ref(Obj) afk_msg` (`WString`) | **AFK Status Sync** — Updates local AFK state. |

#### 3.1.3 `FriendIgnoreEntry` (`0x02B5`, decimal `693`, `vslot 2 = 0x82B50002`) WState Layout

| Order | Member Offset | WState Type | Field Name | Meaning |
|---|---|---|---|---|
| 1 | *(base)* | `u32` | `schema_version` | Class version header (`4`). |
| 2 | `+0x10` | `ref(WString)` | `name` | Character name (`0x00000010`). |
| 3 | `+0x18` | `u32` | `level` | Character level (`0` if anonymous or offline). |
| 4 | `+0x1C` | `u32` | `class_id` | Character class ID (`0` if anonymous or offline). |
| 5 | `+0x20` | `u32` | `area_did` | Current region/area DataID (`0` if anonymous or offline). |
| 6 | `+0x24` | `bool` (`u8`) | `is_online` | `1` if currently online, `0` if offline. |
| 7 | `+0x28` | `f64` | `last_online_timestamp` | Server timestamp of last logout. |
| 8 | `+0x30` | `u32` | `vocation_id` | Crafting vocation ID. |
| 9 | `+0x38` | `ref(0x000E)` | `properties` | `PropertyCollection` containing `PID:Annotation_Text` (`0x1000305D` note string), `PID:Annotation_IgnoreReason` (`0x10003086` u32 reason), and `PID:Communication_CharacterPresence` (`0x10000DF0`). |
| 10 | `+0x40` | `bool` (`u8`) | `is_anonymous` | `1` if the friend has `/anon` enabled. |

#### 3.1.4 Coordination with `ChatNet` Buddy List and User Chat Channels

As documented in `research/notes/m8-chat-server-protocol.md` §4–§5, the UDP Chat Server (`"ChatNet"`) provides parallel room and buddy-tracking opcodes:
- **Buddy List Opcodes on `ChatNet`:** `0x00000170` (`ChatQueryBuddyListRequestEvent` C→S) ↔ `0x0000018D` (`ChatQueryBuddyListResponseEvent` S→C), `0x00000193` (`ChatQueryTrackerRequestEvent` C→S) ↔ `0x0000015E` (`ChatQueryTrackerResponseEvent` S→C), and `0x000001A2` (`ChatBuddyStatusNoticeEvent` S→C). However, the LOTRO Social Panel Friends & Ignore tabs are driven authoritatively by the World Server WLib `0x04EC` ↔ `0x0102` RPCs (`0x84EC0003`–`0x84EC0010` and `0x84EC002C`) documented in §3.1.1–§3.1.2 above.
- **User Chat Channels (`/join`, `/leave`, `/1`–`/8`, `/chatwho`, `/invite`, `/kick`, `/leader`, `/passwd`):** Routed over `"ChatNet"` using `ChatRoomDescriptor` (`desc_type = 2`) with C→S opcodes `0x016E` (`CreateRoom`), `0x016F` (`DestroyRoom`), `0x0177` (`RoomEnter`), `0x018E` (`RoomLeave`), `0x0188` (`SetLeader`), `0x0183` (`ChangePassword`), `0x017F` (`QueryRoom`), `0x016D` (`Invite`), `0x0189` (`Kick`), and `0x016A` (`SendDataToRoom`).

---

### 3.2 `/who` Directory Queries and LFG State (`0x050C` ↔ `0x1021`)

The `/who` player search and LFG ("Looking for Fellowship") status flags are handled by `ServerWho` (`0x050C`, decimal `1292`) and `ClientWho` (`0x1021`, decimal `4129`), using filter class `0x04BE` (`WhoQueryFilter`, native base `0x01CF`) and result entry class `0x1030` (`WhoResultPlayer`, native base `0x102F`) (`BIN-WLIB-P4`).

| RPC Method ID | Dir | Client Method (`0x1021`) | Wire Parameters (`send_msg` order) | Meaning |
|---|---|---|---|---|
| `0x850C0001` | C→S | `0x850C0005` ← `0x90210001` (`/who`) | `ref(Obj) filter` (`0x04BE` `WhoQueryFilter`) | Submit a `/who` search query (rate-limited by `PID:Antispam_WhoQueryRequest` `0x100018B8`). |
| `0x850C0002` | C→S | `0x850C0006` ← `0x90210002` | *(void)* | Cancel active `/who` subscription/query. |
| `0x850C0003` | S→C | `0x90210003` | `u32 total_matches`, `ref(Obj) players` (`Array<ref(0x1030)>`) | `/who` search results batch (`0x075C0001`). |
| `0x850C0004` | S→C | `0x90210004` | `u32 status_code` | `/who` query status/throttled notice (`0x075C0002`). |

#### 3.2.1 `WhoQueryFilter` (`0x04BE` / `0x01CF`, `vslot 2 = 0x84BE0004`) & `WhoResultPlayer` (`0x1030` / `0x102F`, `vslot 2 = 0x90300004`)

- **`WhoQueryFilter` (`0x04BE`) WState fields:** `u32 schema_version (1)`, `ref(WString) name_substring`, `u32 class_mask`, `u32 race_mask`, `u32 min_level`, `u32 max_level`, `u32 area_did`, `bool lfg_only`, `bool fellow_only`, `u32 max_results`.
- **`WhoResultPlayer` (`0x1030`) WState fields:** `u32 schema_version (2)`, `u64 player_iid`, `ref(WString) name`, `u32 level`, `u32 class_id`, `u32 race_id`, `u32 area_did`, `ref(WString) guild_name`, `bool is_lfg`, `bool is_in_fellowship`, `u32 fellow_count`, `ref(WString) lfg_note`.

---

### 3.3 Kinship (`Guild`) Roster, Ranks, Notes, MOTD, and Calendar (`0x0502` ↔ `0x016C`)

Kinships are managed by `ServerGuild` (`0x0502`, decimal `1282`) and `ClientGuild` (`0x016C`, decimal `364`), backed by WState classes `Guild` (`0x048C` / `0x0169`), `GuildMember` (`0x048F` / `0x016A`), `GuildRank` (`0x0490` / `0x016B`), and `GuildCalendar` (`0x1681` / `0x1682` / `0x1685`) (`BIN-WLIB-P4`).

#### 3.3.1 `ServerGuild` (`0x0502`) Client-to-Server RPCs (23 Methods)

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

#### 3.3.2 `ServerGuild` (`0x0502` → `0x016C`) Server-to-Client RPCs (28 Methods)

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

#### 3.3.3 WState Layouts for `Guild` (`0x048C`), `GuildMember` (`0x048F`), and `GuildRank` (`0x0490`)

- **`Guild` (`0x048C` / `0x0169`, `vslot 2 = 0x848C0006`):** `u32 schema_version (7)`, `u64 guild_iid`, `ref(WString) name`, `u64 leader_iid`, `u64 successor_iid`, `f64 creation_timestamp`, `u32 lifespan_rank`, `u32 governance_type`, `ref(WString) motd`, `HashTable<u32, ref(0x0490)> ranks`, `HashTable<u64, ref(0x048F)> members`, `u64 kinship_house_iid`, `u32 member_count`.
- **`GuildMember` (`0x048F` / `0x016A`, `vslot 2 = 0x848F0005`):** `u32 schema_version (4)`, `u64 member_iid`, `ref(WString) name`, `u32 rank_id`, `u32 level`, `u32 class_id`, `u32 area_did`, `bool is_online`, `f64 last_online_timestamp`, `ref(WString) player_note`, `ref(WString) officer_note`, `u32 vocation_id`, `bool is_anonymous`.
- **`GuildRank` (`0x0490` / `0x016B`, `vslot 2 = 0x84900002`):** `u32 schema_version (1)`, `u32 rank_id`, `ref(WString) title`, `u32 permissions_mask`.

---

## 4. Domain 4: Player Music (Solo & Synchronized) <a id="domain-4-player-music"></a>

### 4.1 Architecture & WLib Class Hierarchy

The LOTRO Player Music system is implemented by `ServerMusic` (`0x0519`, decimal `1305`) and `ClientMusic` (`0x0116`, decimal `278`), with native C++ audio/ABC playback glue in `0x140BC00B0`–`0x140BC0630` (`ClientMusic` native package `0x0116`) and batched note streaming via native WState class `0x0804` (`MusicNoteBatch`, decimal `2052`) (`BIN-WLIB-P4`, `BIN-MUSIC-NAT`).

Slash commands (`/music` at `0x140B8FE80`, `/play <filename> [sync]` at `0x140B8FF10`, `/playstart` at `0x140B90080`) and interactive keyboard/keybind note actions (`PlayerActionHandler::HandleAction` `0x1919A4` for action IDs `0x1000037C`–`0x1000039F`, or C++ note hooks `0x140B8FD80` / `0x140B90110` / `0x140B901F0`) invoke `0x0116` (`ClientMusic`) methods.

### 4.2 `ServerMusic` (`0x0519`) Client-to-Server RPCs (7 Methods)

| RPC Method ID | Wrapper Method | Client Caller (`0x0116` & C++ VA) | Wire Parameters (`send_msg` order) | `SocialCommand` / Music Semantics |
|---|---|---|---|---|
| `0x85190001` | `0x8519000E` | `0x81160004` (`/music`, `0x140B8FE80`) | `u32 enter_music_mode` | **Toggle `/music` Mode** — `enter_music_mode`: `1` = enter player music stance and equip instrument (`0x81160016` checks `PID:Inventory_SlotCache_RangedItem` for an equipped instrument); `0` = exit music mode and stop any active performance. |
| `0x85190002` | `0x8519000F` | `0x81160005` (Keybind `0x1919A4`) | `u32 channel_or_velocity`, `u32 note_pitch` | **Interactive Live Note-On** — Plays a single interactive note (`note_pitch` `1..37`, mapped from action IDs `0x1000037C`–`0x1000039F`) during live manual performance. |
| `0x85190003` | `0x85190010` | `0x81160006` (`0x140B8FD80`) | `u32 channel_or_velocity`, `f64 timestamp`, `u32 note_pitch` | **Timestamped Note-On** — Sends a single timestamped Note-On event (`timestamp` in seconds relative to song start). |
| `0x85190015` | `0x8519001B` | `0x81160015` (`0x140B901F0`) | `ref(Obj) batch` (`0x0804` `MusicNoteBatch`) | **Stream Note Batch (`MusicNoteBatch`)** — Sends a batch of timestamped Note-On and Note-Off events (`0x0804` WState object) for ABC song playback or low-latency live performance. |
| `0x85190007` | `0x85190011` | `0x81160008` (`/play`, `0x140B8FF10`) | `u32 is_sync`, `ref(Obj) song_title` (`WString`) | **`SocialCommand::ReadyPerformance`** — Prepares ABC song `song_title` for solo (`is_sync == 0`) or fellowship-synchronized (`is_sync == 1`) playback. Rate-limited by `PID:Antispam_MusicPlayRequest` (`0x10001EA6`). |
| `0x85190008` | `0x85190012` | `0x81160009` (`/playstart`, `0x140B90080`) | *(void)* | **`SocialCommand::StartPerformance`** — Starts a synchronized performance once fellowship members have readied via `0x85190007` (`is_sync == 1`). |
| `0x85190009` | `0x85190013` | `0x8116000A` (`0x140B90110`) | *(void)* | **Stop / Withdraw Performance** — Stops current ABC song playback or withdraws from a readied synchronized performance. |

### 4.3 `ServerMusic` (`0x0519` → `0x0116`) Server-to-Client RPCs (7 Methods)

| RPC Method ID | Client Handler (`0x0116`) | Native C++ Handler (`0x0116` Package) | Wire Parameters (`send_msg` order) | `SocialEvent` / Notification Semantics |
|---|---|---|---|---|
| `0x85190004` | `0x81160003` (vslot 11) | `0x01160001` (`0x140BC00B0`) | `u32 is_sync`, `u32 instrument_did`, `u64 performer_iid` | **`SocialEvent::PerformanceStarted` / Mode Entered** — Notifies clients that `performer_iid` entered music mode or started performing on `instrument_did` (`0x140BBF4E0`). |
| `0x85190005` | `0x81160007` (vslot 12) | `0x0116000F` (`0x140BC05B0`) | `u32 channel_or_velocity`, `u32 instrument_did`, `u32 note_pitch`, `u64 performer_iid` | **Live Note-On Broadcast** — Broadcasts an immediate interactive note played by `performer_iid` (`0x140BBF9F0`). |
| `0x85190006` | `0x81160013` (vslot 13) | `0x01160010` (`0x140BC0470`) | `u32 flag_b`, `u32 flag_a`, `u32 channel_or_velocity`, `u32 instrument_did`, `u64 performer_iid`, `f64 timestamp`, `u32 note_pitch` | **Timestamped Note-On Broadcast** — Schedules a single timestamped note (`is_note_on = 1`) for `performer_iid` at `timestamp` (`0x140BB5B80`). |
| `0x8519001A` | `0x81160017` (vslot 14) | `0x0116000D` (`0x140BC0370`) | `ref(Obj) batch` (`0x0804` `MusicNoteBatch`) | **Batched Notes Broadcast (`MusicNoteBatch`)** — Iterates through all `N` note events in `0x0804` (`0x140AAFA10`) and schedules each Note-On (`1`) or Note-Off (`0`) on the client's audio synthesizer (`0x140BB5B80`). |
| `0x8519000A` | `0x8116000B` (vslot 15) | `0x01160005` (`0x140BC0210`) | `u32 is_sync`, `ref(Obj) song_title` (`WString`), `u32 instrument_did`, `u64 performer_iid` | **`SocialEvent::PerformanceReady`** — Notifies the player/fellowship that `performer_iid` is ready to play `song_title` on `instrument_did` (`is_sync`: `1` = sync, `0` = solo; calls `0x140BBF5D0`). |
| `0x8519000B` | `0x8116000C` (vslot 16) | `0x01160006` (`0x140BC0310`) | `u64 initiator_iid` | **`SocialEvent::PerformanceSyncStart`** — Synchronized performance started by `initiator_iid` (`0x140BBCF70`). All readied performers begin streaming note batches (`0x85190015`). |
| `0x8519000C` | `0x8116000D` (vslot 17) | `0x0116000E` (`0x140BC0550`) | `u64 performer_iid` | **`SocialEvent::PerformanceStopped` / `PerformanceWithdrawn`** — `performer_iid` stopped playing or withdrew from a synchronized performance (`0x140BBF7B0`). |

### 4.4 `MusicNoteBatch` (`0x0804`, decimal `2052`) Native WState Wire Layout (`0x140AAFD60` → `0x140AAFBD0`)

Used in both `0x85190015` (C→S) and `0x8519001A` (S→C). Inside `Pack_ObjRef` (`u32 class_id = 0x00000804` and WState class header), the serializer at `0x140AAFD60` writes the note array (`0x140AAFBD0`) followed by the performer metadata trailer (`BIN-MUSIC-NAT`):

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

## 5. Clean-Room Specification Section (Ready for `docs/specs/official-messages-gameplay.md`) <a id="clean-room-spec"></a>

> The following section is written in self-contained clean-room prose and byte-layout tables, free of binary virtual addresses and internal symbol names, for direct inclusion in `docs/specs/official-messages-gameplay.md`.

### 5.1 Social Panels Overview and WLib RPC Transport <a id="spec-social-overview"></a>

Social gameplay interactions on the World Server—Fellowships, Raids, Player-to-Player Trade, Friends & Ignore Lists, `/who` Directory Queries, Kinship (`Guild`) Management, and Player Music—are transported inside WLib entity RPC messages (`0x00000030`) over the `"ClientNet"` world connection (`BIN-WLIB-P4`, `OBS-DAT-P4`).

Each WLib RPC frame carries a 32-bit method identifier (`0x8PPPSSSS`, where `0xPPP` identifies the WLib package/class and `0xSSSS` identifies the static method index) followed by a sequential payload of little-endian primitive values (`u32`, `u64`, `f32`, `f64`) and typed object references (`ref(Obj)`). A `ref(Obj)` begins with a 32-bit `class_id` (`0x00000000` for null, `0x00000002` for 8-bit `PString`, `0x00000010` for UTF-16LE `WString`, `0x0000000E` for `PropertyCollection`, or a WState class ID followed by its versioned field stream).

---

### 5.2 Fellowships, Raids, Ready Checks, Votes, and Target Marking <a id="spec-fellowships-raids"></a>

#### 5.2.1 Fellowship Client-to-Server Commands (`0x04FE`)

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

#### 5.2.2 Fellowship Server-to-Client Notifications (`0x04FE`)

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

#### 5.2.3 Raid Commands (`0x0523`), Votes (`0x0CFE`), Target Marking (`0x0739`), and LFF Maker (`0x0C41`)

- **Raid (`0x0523`):** Supports C→S `0x85230001` (`Disband`), `0x85230002` (`Leave`), `0x85230003` (`DismissByName`: `ref(WString)`), `0x85230004` (`DismissByIID`: `u64`), `0x85230005` (`TransferLeadership`: `u64`), `0x85230006` (`PromoteAssistant`: `u64`), `0x85230007` (`DemoteAssistant`: `u64`), `0x85230008` (`MoveMember`: `u32 group_idx, u64 target_iid`), `0x85230009` (`SwapMembers`: `u64 b_iid, u64 a_iid`), `0x8523000A` (`SetLootRule`: `u32`), `0x8523000B` (`SetLootQuality`: `u32`), `0x85230025` (`QueryRaidLocks`); and S→C `0x8523000C` (`FullSync`: `ref(0x0454)`), `0x8523000D`–`0x85230020`, `0x85230026`, `0x85230033` (`FastVitals`).
- **Target Marking (`0x0739`):** C→S `0x87390001` (`u32 marker_id, u64 target_iid`).
- **Fellowship Vote (`0x0CFE`):** C→S `0x8CFE0002` (`u32 response, u32 vote_id`); S→C `0x8CFE0003` (`ref(0x0D06) vote_data`), `0x8CFE0004` (`u32 reason, u32 vote_id`), `0x8CFE0005` (`u32 error, u32 vote_id`).
- **LFF Maker (`0x0C41`):** C→S `0x8C410002` (`Post`: `ref(0x0C3B)`), `0x8C410003` (`Remove`), `0x8C410004` (`Search`: `ref(0x0C36)`), `0x8C410009` (`RequestJoin`: `u64 leader_iid`); S→C `0x8C410005` (`SearchResults`), `0x8C410006` (`PostUpdated`), `0x8C410007` (`Status`), `0x8C41000A` (`JoinRequestReceived`).

---

### 5.3 Player-to-Player Trade (`0x0531`) <a id="spec-player-trade"></a>

#### 5.3.1 Trade State Machine

The client tracks player-to-player trade state in 5 states:
1. **`0` (Idle):** No active or pending trade. `OpenTrade` (`0x85310043`) transitions `0 → 1`.
2. **`1` (Pending):** Outgoing or incoming trade request waiting for session creation. `OnTradeOpened` (`0x8531000F`) transitions `1 → 2`; `DeclineTrade` (`0x85310034`) or `CloseTrade` (`0x8531005D`) transitions `1 → 0`.
3. **`2` (Open / Modifying Offer):** Both players can add items (`0x85310024`), remove items (`0x85310060`), or set offered coinage (`0x8531002E`). Any modification to items or coinage on either side automatically resets both players' ready/acceptance flags to `0`.
4. **`3` (Local Offer Locked / Accepted):** Entered when the local player sends `AcceptTrade` (`0x85310050`) with a full `0x05C3` (`TradeOfferData`) snapshot. If either player modifies the offer or sends `ResetTrade` (`0x85310042`), `OnTradeReset` (`0x85310046`) transitions `3 → 2`.
5. **`4` (Completed):** Entered on `OnTradeCompleted` (`0x8531005F`), followed immediately by `OnTradeFinalized` (`0x85310052`), which logs the exchange and resets to `0`.

#### 5.3.2 Trade Client-to-Server Commands (`0x0531`)

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

#### 5.3.3 Trade Server-to-Client Notifications (`0x0531`)

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

### 5.4 Friends & Ignore Lists (`0x04EC`), `/who` (`0x050C`), and Kinships (`0x0502`) <a id="spec-friends-who-kinships"></a>

#### 5.4.1 Friends & Ignore Commands and Notifications (`0x04EC`)

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

#### 5.4.2 `/who` Directory Queries (`0x050C`) and Kinship Roster & Calendar (`0x0502`)

- **`/who` Queries (`0x050C`):** C→S `0x850C0001` (`ref(0x04BE) filter`) submits a directory search; C→S `0x850C0002` cancels it; S→C `0x850C0003` (`u32 total_count, ref(Array<ref(0x1030)>) results`) returns matching player records.
- **Kinships (`0x0502`):** C→S `0x85020003` (`Disband`), `0x85020004` (`RecruitByIID`), `0x85020005` (`RecruitByName`), `0x85020006` (`AnswerInvite`), `0x85020007` (`Quit`), `0x85020008` (`Expel`), `0x85020009` (`Promote`), `0x8502000A` (`Demote`), `0x8502000B` (`SetMOTD`), `0x8502000C` (`SetPlayerNote`), `0x8502000D` (`SetOfficerNote`), `0x8502000E` (`SetSuccessor`), `0x8502000F` (`Usurp`), `0x85020029` (`RequestRoster`), `0x8502002F` (`SetRankTitle`), `0x85020030` (`SetRankPermissions`), and `0x8502004E`–`0x85020053` (`Calendar` operations). S→C `0x85020010` (`FullGuildSync`: `ref(0x048C)`), `0x8502002A` (`FullRosterSync`: `HashTable<u64, ref(0x048F)>`), `0x85020013`–`0x8502001B` (member/rank/MOTD deltas), and `0x8502005B`–`0x8502005E` (`Calendar` sync).

---

### 5.5 Player Music (Solo & Synchronized) (`0x0519`) <a id="spec-player-music"></a>

#### 5.5.1 Player Music Commands and Notifications (`0x0519`)

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

### 5.6 Synthetic Test Vectors and Invariants <a id="spec-test-vectors"></a>

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

## Change Log

- **2026-10-20:** Initial draft completing P4 Social Panels wire specification (`0x04FE`, `0x0523`, `0x0CFE`, `0x0739`, `0x0C41`, `0x0531`, `0x04EC`, `0x050C`, `0x0502`, `0x0519`, and `0x0804` `MusicNoteBatch`).
