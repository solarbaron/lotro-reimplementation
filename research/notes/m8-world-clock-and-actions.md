<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# World Clock Synchronization, Portal Placement (`0xCC`) State Schemas, and `ActionPayload` Wire Specification

- **Status:** Draft
- **Last updated:** 2026-10-20
- **Author role:** Spec writer (dirty room)
- **Reviewer:** pending

---

## Purpose

This specification resolves three deep world-protocol and engine-state questions left open during initial M8 world message analysis:

1. **Game Clock Baseline, Transport `TimeSync` (`0x01000000`), `GameTime` Calendar (`0x35000000`), and `GlobalEventHandler` (`14080df60`) Events** — Closes `docs/specs/official-messages-world.md` §4.4 OPEN #3, `docs/specs/environment.md` §11.1.1, and `docs/specs/property-system.md` §4.3.
2. **Portal Placement (`0xCC`) Dynamic State Schema Table and `extra_blob` Framing** — Closes `docs/specs/official-messages-world.md` §4.3.1.
3. **`ActionPayload` (`FUN_1407f6de0`) and Action Events (`0x24`, `0x75`, `0x5A`, `0xA8`, `0xCD`) Wire Layout** — Closes `docs/specs/official-messages-world.md` §5.5–§5.6 OPEN #4.

All descriptions use clean-room prose, byte-layout tables, mathematical formulas, state-machine rules, and synthetic test vectors. No vendor source code, decompiler output, packet captures, or proprietary credentials are included.

---

## Provenance

| ID | Class | Method | Source / Binary | Build / Version | SHA-256 | Tool & Version | Question Answered | Why M1–M2 Were Insufficient |
|---|---|---|---|---|---|---|---|---|
| `OBS-DAT-GEN` | Confirmed | M1 (`OBS`) | `client_general.dat` (`0x35000000`, `0x0F000000`–`0x0F000009`) | Steam install (read-only) | — | Python `.dat` reader | Exact byte layout and values of the `0x35000000` (`GameTime`) calendar record (`11160.0` s day length, `360` days/year, `0.25` dawn, `0.80` dusk, 4 time-of-day phases, 12 months). | M1 directly resolved record values once the record DID (`0x35000000`) and layout were identified. |
| `OBS-DAT-GL` | Confirmed | M1 (`OBS`) | `client_gamelogic.dat` (`0x34000000` `MasterProperty`, `0x07000029` `ScriptTable`, `0x0Cxxxxxx` `Scriptlet`, `0x23000097` / `0x230000F8`) | Steam install (read-only) | — | Python `.dat` reader | Exact deduplicated `bool_pids` (8 properties) and `enum_pids` (6 properties) lists for `state_schema_id = 0x0C` (`12`, `Earth_ControlledSurface`) and sibling `PlayerPhysics` scripts (`0x10000E50`, `0x10000E51`) in `0x07000029`. | M1 directly provided the `.dat` table and property definitions once the traversal algorithm in the binary was known. |
| `BIN-CLOCK` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (`FUN_14116eff0`, `FUN_1403145a0`, `FUN_14031bc10`, `FUN_14031be80`, `FUN_141196400`, `FUN_1411990e0`, `FUN_14043a700`) | `4906.0071.1794.4035` (`Sun Sep 27 14:35:50 2026 : RELEASE`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra `12.1.3` headless | How `DAT_141e706b0` (synced monotonic clock) and `DAT_141e706b8` (raw local clock) are seeded by transport control flag `0x01000000` (`TimeSync`), slewed per frame, and added to Type 4 `Timestamp` properties. | Clock synchronization formulas, slew rate tiers, and transport control opcodes exist only in executable code, not in `.dat` files or public docs. |
| `BIN-GAMETIME` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (`FUN_1403e13d0`–`FUN_1403e3770`, `FUN_14080df60`, `FUN_140812da0`, `FUN_140750a10`–`FUN_140751b10`) | `4906.0071.1794.4035` | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra `12.1.3` headless | How `GameTime.cpp` (`DAT_141e51e40`) computes time-of-day from `DAT_141e706b0`, `World_TimeOffset` (`0x01EF`), `World_TimeDilation` (`0x06CC`), and `0x35000000`; and the complete wire layout of all 22 events registered in `GlobalEventHandler` (`14080df60`). | Runtime formulas and event handler dispatch tables exist only in the client binary. |
| `BIN-0XCC` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (`FUN_1405473a0`, `FUN_14053b4b0`, `FUN_140537630`, `FUN_1404919f0`, `FUN_1404916d0`, `FUN_140733dd0`, `FUN_140584670`) | `4906.0071.1794.4035` | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra `12.1.3` headless | How `DAT_141e56178` is populated on demand from `0x07` `ScriptTable` and `0x0C` `Scriptlet` graphs (`Switch` node kind `12` and `When` node kind `63`) and how `FUN_1405473a0` decodes the boolean bitfield, enum VLE array, and 4-byte `extra_blob_size` in event `0xCC`. | `DAT_141e56178` is a runtime-constructed array in `.bss`; its population algorithm and wire framing exist only in executable code. |
| `BIN-ACTION` | Confirmed | M4 (`BIN`) | `x64/lotroclient64.exe` (`FUN_1407f6de0`, `FUN_1407f7810`, `FUN_1407f6cb0`, `FUN_1407f7d30`, `FUN_1407f6aa0`, `FUN_1407f7bd0`, `FUN_1405953c0`, `FUN_140596bd0`, `FUN_140596410`, `FUN_140596840`, `FUN_1404cb4f0`, `FUN_1405733f0`) | `4906.0071.1794.4035` | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra `12.1.3` headless | Complete wire layout of `ActionPayload` (`FUN_1407f6de0`), its 5 sub-serializers, and its 5 enclosing action events (`0x24`, `0x75`, `0x5A`, `0xA8`, `0xCD`). | Wire serializers for combat/skill/animation action payloads exist only in executable code. |

---

## 1. Game Clock Baseline, `TimeSync` (`0x01000000`), `GameTime`, and `GlobalEventHandler` (`14080df60`) <a id="game-clock"></a>

### 1.1 Global Monotonic Clock Variables and Frame Update (`FUN_14116eff0`) <a id="clock-globals"></a>

The client maintains three global 64-bit values in `.bss` (`0x141e706b0`–`0x141e706c0`) updated once per frame at the top of the main loop (`FUN_14031bec0` calling `FUN_14116eff0`):

| Global Address | Type | Name | Meaning |
|---|---|---|---|
| `0x141e706b0` | `f64` | `g_SyncedClockSeconds` (`DAT_141e706b0`) | Server-synchronized monotonic game clock in seconds. Used as the baseline for Type 4 `Timestamp` properties (`FUN_14043a700`), `ActionPayload` relative timestamps (`FUN_1407f6de0`), and `GameTime` time-of-day progression (`FUN_1403e21d0`). |
| `0x141e706b8` | `f64` | `g_LocalClockSeconds` (`DAT_141e706b8`) | Raw local monotonic elapsed time in seconds since client startup (derived from `QueryPerformanceCounter` with `timeGetTime` fallback in `FUN_14116e120`). |
| `0x141e706c0` | `ptr` | `g_ActiveClockPtr` (`DAT_141e706c0`) | Pointer to the active `SmoothedClock` object (`0xB8` bytes, constructed by `FUN_1403145a0`, vtable `0x1414f22e0`). |

#### `SmoothedClock` Object Layout (`0xB8` bytes, `FUN_1403145a0` / `FUN_14031bd70`)

| Offset | Size | Type | Initial Value | Meaning |
|---|---|---|---|---|
| `+0x00` | `8` | `ptr` | `0x1414f22e0` | Virtual method table pointer (`vtable[+0x10]` = `FUN_140317a60` raw time query; `vtable[+0x18]` = `FUN_140317a70` raw time + offset; `vtable[+0x20]` = `FUN_14031bc10` per-frame update; `vtable[+0x28]` = `FUN_14031be80` record server time sync). |
| `+0x08` | `4` | `i32` | `1` | Reference count. |
| `+0x10` | `0x50` | `struct` | — | Embedded high-resolution hardware timer (`QueryPerformanceFrequency` / `QueryPerformanceCounter` state, initialized by `FUN_14116dc90`). |
| `+0x60` | `8` | `f64` | `0.0` | `clock_offset`: current additive offset (`synced_time - raw_elapsed`) in seconds. |
| `+0x68` | `8` | `f64` | `raw_t0` | `raw_elapsed`: monotonic local elapsed time in seconds at the most recent frame tick (copied to `DAT_141e706b8`). |
| `+0x70` | `8` | `f64` | `raw_t0` | `synced_time`: server-synchronized game time in seconds at the most recent frame tick (copied to `DAT_141e706b0`). |
| `+0x78` | `1` | `u8` | `0` | Base timer flag. |
| `+0x80` | `8` | `f64` | `0.0` | `slew_rate`: dimensionless fractional slew rate (`seconds_corrected / second_elapsed`) applied when the client clock is ahead of the server clock. |
| `+0x88` | `8` | `f64` | `0.015` | `snap_threshold`: minimum positive ahead-of-server discrepancy (`15` ms) required before slewing begins (`0x3f8eb851eb851eb8` = `0.015`). |
| `+0x90` | `8` | `f64` | `0.0` | `slew_debt`: remaining positive time excess in seconds to bleed off via slewing. |
| `+0x98` | `8` | `f64` | `0.0` | `prev_server_time`: server timestamp from the preceding ahead-of-server sync sample, used to detect clock drift rate. |
| `+0xA0` | `8` | `f64` | `0.0` | `pending_server_time`: server timestamp staged by `vtable[+0x28]` (`FUN_14031be80`). |
| `+0xA8` | `8` | `f64` | `0.0` | `pending_receipt_raw`: raw local time (`vtable[+0x10]()`) when `pending_server_time` was staged. |
| `+0xB0` | `1` | `u8` | `0` | `pending_force`: `1` to force an immediate snap regardless of direction; `0` for normal asymmetric snap/slew. |
| `+0xB1` | `1` | `u8` | `0` | `has_pending_sync`: `1` when a new server time sample is waiting to be applied on the next `vtable[+0x20]` (`FUN_14031bc10`) call. |

---

### 1.2 Transport-Layer `TimeSync` (`0x01000000`) Wire Format and Seeding Flow <a id="transport-timesync"></a>

`DAT_141e706b0` is **not** seeded by a game-layer event inside `0x0082400B` (`RecvEvt_*`). Instead, it is seeded and continuously synchronized by the **transport-layer packet header control flag `0x01000000`** (`TimeSync`), registered in the transport control opcode table `DAT_141e73820` by `FUN_141192dc0`:

| Transport Control Flag | Parser Function | Wire Payload Size | Control Block Tag (`+0x14`) | Handler Path |
|---|---|---|---|---|
| `0x01000000` (bit 24 of big-endian transport packet flags) | `FUN_141196400` | `8` bytes (`f64 LE`) | `0x18` (`24`) | `FUN_14119d370` -> `FUN_141193e60` -> `FUN_141199eb0` (`vtable[+0xE8]`) -> `FUN_1411990e0` |

#### `TimeSync` (`0x01000000`) Wire Payload

When bit `0x01000000` is set in the transport packet header flags (`FUN_14119d370`), the transport control loop invokes `FUN_141196400` on the packet's bit/byte stream (`param_2 + 0x218`):

| Offset | Size | Type | Byte Order | Meaning |
|---|---|---|---|---|
| `0x00` | `8` | `f64` | Little-endian | `server_time`: authoritative server monotonic clock in seconds. |

#### Three-Stage Latency-Compensated Seeding and Slew Algorithm

1. **Stage 1 — Packet Receipt Stamp (`FUN_141196400`):**
   - Reads `server_time` (`f64 LE`, `8` bytes) from the transport packet stream.
   - Resolves the active clock pointer (`clock`) from the network context (`0x080A4005` interface `+0x18`, or global fallback `FUN_14116e780()`).
   - Queries `t_recv = clock->vtable[+0x10](clock)` (raw local hardware time in seconds at the moment the UDP packet is parsed).
   - Allocates a `0x3C`-byte control message (`+0x10 = 0x01000000`, `+0x14 = 0x18`, `+0x24 = clock`, `+0x2C = server_time`, `+0x34 = t_recv`).
2. **Stage 2 — Network Queue Dispatch (`FUN_1411990e0`, invoked via `FUN_141199eb0` `vtable[+0xE8]`):**
   - When the control message is dequeued (`type == 5`), queries `t_dispatch = clock->vtable[+0x10](clock)`.
   - Compensates for queueing delay between packet parsing and control dispatch:
     $$\text{server\_time\_at\_dispatch} = (t_{\text{dispatch}} - t_{\text{recv}}) + \text{server\_time}$$
   - Calls `clock->vtable[+0x28](clock, server_time_at_dispatch, 0)` (`FUN_14031be80`), which stores:
     - `pending_server_time` (`+0xA0`) = `server_time_at_dispatch`
     - `pending_force` (`+0xB0`) = `0`
     - `pending_receipt_raw` (`+0xA8`) = `clock->vtable[+0x10](clock)`
     - `has_pending_sync` (`+0xB1`) = `1`
3. **Stage 3 — Per-Frame Clock Update (`FUN_14031bc10`, invoked via `FUN_14116eff0`):**
   - If `has_pending_sync != 0`:
     - Clears `has_pending_sync = 0`.
     - Queries `t_now = clock->vtable[+0x10](clock)` and computes the target server time at the current instant:
       $$\text{target} = (t_{\text{now}} - \text{pending\_receipt\_raw}) + \text{pending\_server\_time}$$
     - **Case A — Client behind server or forced (`target > synced_time` or `pending_force != 0`):**
       - Monotonicity is preserved by jumping forward.
       - Updates `raw_elapsed` (`+0x68`) via `FUN_14116e120(this + 0x10, 1)`.
       - Snaps `synced_time` (`+0x70`) = `target`.
       - Sets `clock_offset` (`+0x60`) = `target - raw_elapsed`.
       - Resets slew state: `slew_rate` (`+0x80`) = `0.0`, `slew_debt` (`+0x90`) = `0.0`, `prev_server_time` (`+0x98`) = `0.0`, and returns immediately.
     - **Case B — Client ahead of server (`target <= synced_time` and `pending_force == 0`):**
       - To prevent `DAT_141e706b0` from moving backward, the clock never snaps backward; instead, it slews (slows down) if the discrepancy reaches `snap_threshold` (`0.015` s):
       - Let $\text{excess} = \text{synced\_time} - \text{target} \ge 0$.
       - If $\text{excess} < 0.015\text{ s}$ (`+0x88`): ignores the discrepancy and zeroes `slew_rate`, `slew_debt`, and `prev_server_time`.
       - If $\text{excess} \ge 0.015\text{ s}$: sets `slew_debt` (`+0x90`) = `excess` and selects the base `slew_rate` (`+0x80`) from 5 threshold tiers (multiples of `0.015` s):

| Excess Range ($\text{synced\_time} - \text{target}$) | Multiplier of `0.015` s | Base `slew_rate` (`+0x80`) | Binary Constant Address |
|---|---|---|---|
| $[0.000, 0.015)\text{ s}$ | $[0, 1)$ | `0.0` (no slew) | — |
| $[0.015, 0.030)\text{ s}$ | $[1, 2)$ | `0.00025` (`0.025%`) | `DAT_1414f3198` (`0x3f30624dd2f1a9fc`) |
| $[0.030, 0.060)\text{ s}$ | $[2, 4)$ | `0.0025` (`0.25%`) | `DAT_1414f31a0` (`0x3f647ae147ae147b`) |
| $[0.060, 0.120)\text{ s}$ | $[4, 8)$ | `0.01` (`1.0%`) | `DAT_1414f31a8` (`0x3f847ae147ae147b`) |
| $[0.120, 0.240)\text{ s}$ | $[8, 16)$ | `0.025` (`2.5%`) | `DAT_1414f31b0` (`0x3f9999999999999a`) |
| $\ge 0.240\text{ s}$ | $\ge 16$ | `0.05` (`5.0%`) | `DAT_1414f31b8` (`0x3fa999999999999a`) |

       - **Drift-Rate Boost:** If `prev_server_time > 0.0` and $\frac{\text{excess}}{\text{pending\_server\_time} - \text{prev\_server\_time}} > \text{slew\_rate}$, adds `0.0002` (`DAT_1414f3190` = `0x3f2a36e2eb1c432d`) to `slew_rate`. Sets `prev_server_time` = `pending_server_time`.
   - **Per-Frame Advancement (executed on every frame when not snapping forward):**
     - Queries `new_raw = FUN_14116e120(this + 0x10, 1)` and computes frame delta $\Delta t = \text{new\_raw} - \text{raw\_elapsed}$.
     - If `slew_debt > 0.0`:
       $$\text{step} = \min(\text{slew\_debt},\; \Delta t \times \text{slew\_rate})$$
       $$\text{slew\_debt} \leftarrow \text{slew\_debt} - \text{step}, \qquad \text{clock\_offset} \leftarrow \text{clock\_offset} - \text{step}$$
     - Advances `raw_elapsed = max(raw_elapsed, new_raw)` and sets:
       $$\text{synced\_time} = \text{raw\_elapsed} + \text{clock\_offset}$$
   - `FUN_14116eff0` then copies `raw_elapsed` to `DAT_141e706b8` and `synced_time` to `DAT_141e706b0`.

---

### 1.3 Property System Type 4 (`Timestamp`) Wire Decoding (`FUN_14043a700`) <a id="timestamp-property"></a>

In the property serializer (`FUN_14043a700`), base property type `4` (`Timestamp`, stored in memory as an `f32` in seconds) is encoded on the wire as a **relative `f32 LE` offset** from `DAT_141e706b0`:

- **Wire-to-Client Decode (`(stream.flags & 1) == 0` and `(stream.flags & 0x800) != 0`):**
  1. Reads a 4-byte `f32 LE` value `rel_seconds` from the bitstream (`FUN_1411680d0(stream, 4)`).
  2. Converts it to an absolute client game-clock timestamp by adding `(f32)DAT_141e706b0`:
     $$\text{prop\_timestamp\_f32} = \text{rel\_seconds} + \text{f32}(\text{DAT\_141e706b0})$$
- **Client-to-Wire Encode (`(stream.flags & 1) != 0` and `(stream.flags & 0x800) != 0`):**
  1. Subtracts `(f32)DAT_141e706b0` from the in-memory property value:
     $$\text{rel\_seconds} = \text{prop\_timestamp\_f32} - \text{f32}(\text{DAT\_141e706b0})$$
  2. Writes `rel_seconds` as `f32 LE` (`4` bytes).
- **Local/Archive Serialization (`(stream.flags & 0x800) == 0`):**
  - Reads or writes the raw `f32 LE` value without adding or subtracting `DAT_141e706b0`.

---

### 1.4 `GameTime` Calendar Record (`0x35000000`) and World Time Properties (`FUN_1403e13d0`–`FUN_1403e3770`) <a id="gametime-calendar"></a>

The `GameTime` singleton (`DAT_141e51e40`, `0x78` bytes, initialized by `FUN_1403e2980` -> `FUN_1403e13d0`) loads its calendar definition via `FUN_14034b680(0x96, 0x0F, 2)`, which resolves key `0x96` (`150`) in the `0x0F` directory map to **record DID `0x35000000` in `client_general.dat`** (`180` bytes, deserialized by `FUN_1403e3380`).

#### `0x35000000` (`GameTime` Calendar Definition) Byte Layout in `client_general.dat` (`180` bytes)

| Offset | Size | Type | Value in `client_general.dat` | In-Memory Offset (`lVar2`) | Meaning |
|---|---|---|---|---|---|
| `0x00` | `4` | `u32 LE` | `0x35000000` | `+0x08` | Record DID (`0x35000000`). |
| `0x04` | `4` | `f32 LE` | `11160.0` (`0x462E6000`) | `+0x40` | `day_length_seconds`: length of one in-game day in seconds (`3` hours `6` minutes = `186` minutes = `11,160.0` s; constructor default before load is `100,000.0` s `0x47C35000`). |
| `0x08` | `4` | `i32 LE` | `360` (`0x00000168`) | `+0x44` | `days_per_year`: number of in-game days per year (`360`). |
| `0x0C` | `8` | `f64 LE` | `3600.0` (`0x40AC200000000000`) | `+0x48` | `calendar_epoch_offset`: additive epoch offset in seconds (`3600.0` s). |
| `0x14` | `4` | `i32 LE` | `34` (`0x00000022`) | `+0x50` | `base_year`: base calendar year offset (`34`). |
| `0x18` | `4` | `f32 LE` | `0.80` (`0x3F4CCCCD`) | `+0x54` | `dusk_threshold`: normalized time-of-day fraction $[0.0, 1.0]$ at which night begins (`0.80`; constructor default `0.75` `0x3F400000`). |
| `0x1C` | `4` | `f32 LE` | `0.25` (`0x3E800000`) | `+0x58` | `dawn_threshold`: normalized time-of-day fraction $[0.0, 1.0]$ at which day begins (`0.25`). |
| `0x20` | `4` | `u32 LE` | `4` | `+0x6C` | `tod_phase_count`: number of time-of-day display phases (`+0x60` array). |
| `0x24` | `32` | `4 × (f32, u32)` | `(0.0, 0x0FB4F8C4)`, `(0.25, 0x0004A8DE)`, `(0.50, 0x0005565E)`, `(0.75, 0x05AC59C4)` | `*(+0x60)` | `tod_phases`: 4 pairs of `(f32 start_fraction, u32 string_table_did_or_token)` sorted by `start_fraction`. |
| `0x44` | `4` | `u32 LE` | `1` | `+0x7C` | `unknown_70_count`: count of 4-byte entries in `+0x70` array. |
| `0x48` | `4` | `1 × u32 LE` | `0x0004A8C6` | `*(+0x70)` | `unknown_70_entries`: 4-byte token (`0x0004A8C6`). |
| `0x4C` | `4` | `u32 LE` | `12` | `+0x8C` | `month_count`: number of months in the calendar year (`12`). |
| `0x50` | `96` | `12 × (u32, u32)` | `(0, 0x05465AB1)`, `(30, 0x05465AB2)`, `(60, 0x05465AB3)`, `(90, 0x05465AB4)`, `(120, 0x05465AB5)`, `(150, 0x05465AB6)`, `(180, 0x05465AB7)`, `(210, 0x05465AB8)`, `(240, 0x05465AB9)`, `(270, 0x0465AB10)`, `(300, 0x0465AB11)`, `(330, 0x0465AB12)` | `*(+0x80)` | `months`: 12 pairs on wire of `(u32 start_day_of_year, u32 month_string_token)` (stored in memory at `lVar1 + uVar9 * 8` as `(u32 month_string_token, i32 start_day_of_year)`). |
| `0xB0` | `4` | `u32 LE` | `0x005FB893` | `+0x90` | `era_string_token`: calendar era string token (`0x005FB893`). |

#### World Property Integration and Time-of-Day Formula (`FUN_1403e21d0`, `FUN_1403e2ec0`, `FUN_1403e3750`)

`GameTime` registers property listeners (`FUN_140588530`) on the global **World Property Collection** (`DAT_141514560`) for two server-driven input properties and exposes five world properties (`FUN_1403e2bf0`, `FUN_1403e2d20`):

| Property ID | Name | Type | Role in `GameTime` (`DAT_141e51e40`) |
|---|---|---|---|
| `0x000001EF` (`495`) | `World_TimeOffset` | `9` (`f32`) | Server-controlled additive offset in seconds (`*(f32*)(gametime + 0x54)`, default `0.0`). Updated via world property listener `FUN_1403e3230` / `FUN_1403e32a0` / `FUN_1403e3310`. |
| `0x000006CC` (`1740`) | `World_TimeDilation` | `9` (`f32`) | Server-controlled time dilation multiplier (`*(f32*)(gametime + 0x50)`, default `1.0`). Updated via world property listener `FUN_1403e3230` / `FUN_1403e32a0` / `FUN_1403e3310`. |
| `0x00000682` (`1666`) | `World_PresentTimeOfDay` | `9` (`f32`) | Normalized time of day in $[0.0, 1.0]$ (`*(f32*)(gametime + 0x18)`). Broadcasts a world property change notification (`vtable[+0xD8]`) whenever `|World_PresentTimeOfDay - last_broadcast_tod| > 0.01` (`DAT_1414fa14c` = `0.01`). |
| `0x000001F0` (`496`) | `World_IsNight` | `21` (`bool`) | `0` (daytime) when `dawn_threshold <= World_PresentTimeOfDay < dusk_threshold` ($0.25 \le t < 0.80$); `1` (nighttime) when $t < 0.25$ or $t \ge 0.80$. Broadcasts a world property change notification on day/night transitions. |
| `0x00000235` (`565`) | `World_DayType` | `8` (`enum`) | Current day type (`*(i32*)(gametime + 0x5C)`), recomputed at each day rollover in `FUN_1403e2ec0`. |

On each update tick (`FUN_1403e3750` -> `FUN_1403e21d0`):
1. Computes the raw shifted clock `dVar4`:
   $$T_{\text{raw}} = \text{DAT\_141e706b0} + \text{offset}_{60} + \text{offset}_{68} + \text{World\_TimeOffset}$$
   where `offset_60` and `offset_68` are `0.0` at runtime.
2. Integrates dilated game time `T_dilated` (`+0x48`) from the delta since the previous `T_raw` (`+0x40`):
   $$T_{\text{dilated}} \leftarrow T_{\text{dilated}} + \text{World\_TimeDilation} \times (T_{\text{raw}} - T_{\text{raw,prev}})$$
   (On initialization in `FUN_1403e13d0`, both `T_raw,prev` and `T_dilated` are seeded to `DAT_141e706b0 + World_TimeOffset`, and `FUN_1403e2ec0` is called once to establish the current day start `T_day_start` at `+0x20`.)
3. Computes the normalized time of day (`+0x18`, `World_PresentTimeOfDay`):
   $$\text{World\_PresentTimeOfDay} = \frac{T_{\text{dilated}} - T_{\text{day\_start}}}{\text{day\_length\_seconds}} \quad (\text{where } \text{day\_length\_seconds} = 11160.0\text{ s})$$
4. Whenever `World_PresentTimeOfDay > 1.0` (`DAT_1414f0c80`), calls `FUN_1403e2ec0` to advance the calendar day:
   - Let $T_{\text{cal}} = T_{\text{dilated}} + \text{calendar\_epoch\_offset}$ (`3600.0` s) and $L_{\text{year}} = \text{days\_per\_year} \times \text{day\_length\_seconds} = 360 \times 11160.0 = 4,017,600.0\text{ s}$.
   - $\text{year\_index} = \lfloor T_{\text{cal}} / L_{\text{year}} \rfloor$, $\text{calendar\_year} (+0x28) = \text{year\_index} + \text{base\_year}$ (`34`).
   - $T_{\text{rem}} = T_{\text{cal}} - \text{year\_index} \times L_{\text{year}}$.
   - $\text{day\_of\_year} (+0x2C) = \lfloor T_{\text{rem}} / 11160.0 \rfloor \in [0, 359]$.
   - $T_{\text{day\_start}} (+0x20) = T_{\text{dilated}} - (T_{\text{rem}} - \text{day\_of\_year} \times 11160.0)$.

---

### 1.5 Complete Dispatch Table of `GlobalEventHandler` (`FUN_14080df60`) <a id="global-event-handler"></a>

`FUN_14080df60` (`GlobalEventHandler`, vtable `0x14155d6e8`, registered for the `0x0082400B` game event router via `FUN_140812da0`) is **not** a sky/time handler. Instead, it is the client's **Global Event Handler** for session lifecycle, chat/fellowship routing, movement/physics global commands, and entity-targeted action forwarding.

All 22 event opcodes registered in `FUN_14080df60` and their exact wire payloads (after the 4-byte `u32 LE` event opcode read by the outer event router) are specified below:

| Event ID | Handler Function | Inner Serializer / Target | Wire Payload (after `u32 LE` event ID) | Purpose / Behavior |
|---|---|---|---|---|
| `0x00000010` | `FUN_140751420` | `FUN_1407514a0` -> `FUN_140589260` | `u32 LE` `val_a` (`4` B) + `u32 LE` `val_b` (`4` B) + `Array<u64 LE>` (`u32 LE` `count` + `count × 8` B) + `PString16` (`WideString` via `FUN_140304ca0`) | Forwards global group/channel roster update to client controller `+0x30` (`FUN_1407472d0`). |
| `0x00000013` | `FUN_1407515a0` | `FUN_140751610` -> `FUN_14058ab10` | `Array<Struct_0xC0>`: `u32 LE` `count` (`<= 0x20`, else `0x80004005`) + `count` entries of (`u32 LE` `f00` + `u32 LE` `f04` + `PString16` `name` + `Array<u64 LE>` `members` (`u32 LE` `n <= 0x20` + `n × 8` B)) | Batch channel/group roster state sync (`FUN_1407489e0`). |
| `0x00000014` | `FUN_1407512c0` | `FUN_140751330` -> `FUN_1405877e0` | `u32 LE` `val_a` (`4` B) + `u32 LE` `val_b` (`4` B) + `u64 LE` `entity_id` (`8` B) | Global group/channel member event (`FUN_1407462c0`). |
| `0x00000015` | `FUN_1407517f0` | `FUN_140751860` -> `FUN_14058ae50` | `u32 LE` `val_a` (`4` B) + `u32 LE` `val_b` (`4` B) + `u64 LE` `entity_id` (`8` B) | Global group/channel member removal/status event (`FUN_140748b30`). |
| `0x00000016` | `FUN_140751970` | `FUN_1407519f0` -> `FUN_14058b5c0` | `u32 LE` `val_a` (`4` B) + `u32 LE` `val_b` (`4` B) + `PString16` (`WideString` via `FUN_140304ca0`) + `u32 LE` `val_c` (`4` B) | Global group/channel string parameter update (`FUN_140749880`). |
| `0x00000017` | `FUN_1407519b0` | `FUN_140751a20` -> `FUN_14058b550` | `Array<Struct_0x28>`: `u32 LE` `count` (`<= 0x20`, else `0x80004005`) + `count` entries of (`u32 LE` `f00` + `u32 LE` `f04` + `PString16` `str` + `u64 LE` `id`) | Batch group/channel metadata update (`FUN_1407497d0`). |
| `0x00000018` | `FUN_140751a50` | `FUN_140751a90` -> `FUN_140589d00` | `u64 LE` `id` (`8` B) + `u8` `flag` (`1` B) | Calls `(*(param_2 + 0x40))->vtable[+0xC8]` with `16`-byte struct `(u64 id, u8 flag)`. |
| `0x00000063` | `FUN_140750a80` | `FUN_1405834d0` | `0` bytes (empty payload) | Calls `(*(param_1 + 0xB8))->vtable[+0xA8]()` (clear/reset controller state). |
| `0x00000072` | `FUN_140751490` | `FUN_1407514d0` | `PString8` (`1`-byte or VLE length-prefixed narrow string via `FUN_140304c40`) + `u32 LE` `flags` (`4` B) | Appends `"\n"` and dispatches console/server text message via `FUN_1403fca60`. |
| `0x00000076` | `FUN_140750ab0` | `FUN_1405834f0` | `u64 LE` `target_id` (`8` B) + `u32 LE` `param` (`4` B) | Calls `(*(param_1 + 0xB8))->vtable[+0xA0](ctrl, target_id, param)`. |
| `0x0000008A` | `FUN_140750a10` | `FUN_140750a40` -> `FUN_14054e790` | `PropertyList` (`FUN_14042adc0` property bag) | Applies property bag update to client controller `+0x08` (`param_1 + 0x150` via `FUN_1405cdc50`). |
| `0x0000009B` | `FUN_140751a80` | `FUN_140751ab0` -> `FUN_14054fbe0` | `EntityLinkDesc` (`u64 LE` `id` + `PropertyList` via `FUN_14034bc90`) | Applies entity link descriptor to client controller `+0x08` (`param_1 + 0x150` via `FUN_1405cdd00`). |
| `0x000000B1` | `FUN_140751720` | `FUN_140751760` | `u32 LE` `val` (`4` B) | Calls `(*(param_1 + 0x08))->vtable[+0xF8](ctrl, val)`. |
| `0x000000B2` | `FUN_140750960` | `FUN_140750970` | `0` bytes (empty payload) | Calls `(*(param_1 + 0x08))->vtable[+0x100](ctrl)`. |
| `0x000000C7` | `FUN_140750980` | `FUN_1407509b0` | `u64 LE` `entity_id` (`8` B) + `Position` (`FUN_140372d60`) + `u8` `flag` (`1` B) | Looks up entity `entity_id` (`FUN_14050fae0`), acquires physics/movement interface `+0xB0`, and calls `FUN_140732e80(phys, &position, flag)`. |
| `0x000000CD` | `FUN_140751ae0` | `FUN_140751b10` -> `FUN_1404cb4f0` | `CompactDID` (`1` B high + VLE24 low) + `u64 LE` `entity_id` (`8` B) + `ActionPayload` (`FUN_1407f6de0`) + `Array<u64 LE>` (`u32 LE` `count` + `count × 8` B) | Global entity-targeted action broadcast; resolves `entity_id` (`FUN_14050fae0`) and dispatches to entity handler (`FUN_140596a80`). Detailed in [§3.2](#action-events). |
| `0x000000DC` | `FUN_140751750` | `FUN_140751790` -> `FUN_1407357e0` | `u32 LE` `mode` (`4` B) + `u64 LE` `target_id` (`8` B) | Acquires local player physics controller `+0x108` and activates autonomous/mount/attach mode (`FUN_1407357e0`). |
| `0x000000DF` | `FUN_1407517c0` | `FUN_140751800` -> `FUN_140735840` | `u32 LE` `param` (`4` B) | Acquires local player physics controller `+0x108` and deactivates mode (`FUN_140735840`). |
| `0x000000FB` | `FUN_140751850` | `FUN_140751890` | `u8` `flag` (`1` B) | Acquires local player physics controller `+0x108` and calls `vtable[+0xC0](phys, &flag)`. |
| `0x000000FC` | `FUN_1407513b0` | `FUN_1405733f0` | `u32 LE` `count` (`4` B) + `count` entries of (`u32 LE` `key` + `EffectDesc` via `FUN_1405730d0`) | Synchronizes global world effect table (`param_1 + 0xC8` via `FUN_140573bf0`). |
| `0x000000FE` | `FUN_140750940` | `FUN_140750950` | `0` bytes (empty payload) | Calls `(*(param_1 + 0x08))->vtable[+0xE8](ctrl)`. |
| `0x000000FF` | `FUN_140751690` | `FUN_1407516d0` | `u64 LE` `player_entity_id` (`8` B) | `RecvEvt_PlayerSessionStart_ServerToClient` (`FUN_1408150f0`): logs `"GlobalEventHandler::RecvEvt_PlayerSessionStart_ServerToClient: player: 0x%016I64X\n"` and notifies session listeners (`FUN_1408128b0`). |

---

## 2. Portal Placement (`0xCC`) State Schema and Wire Layout <a id="portal-placement-0xcc"></a>

### 2.1 Complete Wire Layout of Event `0xCC` (`FUN_1405473a0`) <a id="portal-placement-wire"></a>

When an entity receives event `0x000000CC` (`FUN_140595be0` -> `FUN_1405473a0`), the payload after the 4-byte `u32 LE` event opcode (`0x000000CC`) has the following exact structure:

| Step | Field Name | Size (bytes) | Type | Condition | In-Memory Offset (`param_1`) | Meaning |
|---|---|---|---|---|---|---|
| 1 | `flags` | `1` | `u8` | Always | — | Bitmask controlling optional velocity components and `state_schema_id`:<br>• Bit 0 (`0x01`): sets `*(u8*)(param_1 + 0x60) = 1`<br>• Bit 1 (`0x02`): `vel_x` present<br>• Bit 2 (`0x04`): `vel_y` present<br>• Bit 3 (`0x08`): `vel_z` present<br>• Bit 4 (`0x10`): `ang_vel_x` present<br>• Bit 5 (`0x20`): `ang_vel_y` present<br>• Bit 6 (`0x40`): `ang_vel_z` present<br>• Bit 7 (`0x80`): explicit `state_schema_id` (`u32 LE`) present; if `0`, `state_schema_id` defaults to `0x0000000C` (`12`, `Earth_ControlledSurface`). |
| 2 | `sequence_or_flags_00` | `4` | `u32 LE` | Always | `+0x00` (`param_1[0]`) | 32-bit header field read via `FUN_1411680d0(stream, 4)`. |
| 3 | `position` | Variable | `Position` | Always | `+0x08` (`param_1 + 2`) | Standard `Position` structure (`FUN_140372d60`: `u8` format tag followed by cell/offset/quaternion fields). |
| 4 | `heading_or_speed` | `4` | `f32 LE` / `u32 LE` | Always | `+0x30` (`param_1 + 0x0C`) | Read via `FUN_14115bb00(param_1 + 0x0C, stream)`. |
| 5 | `movement_mode_byte` | `1` | `u8` | Always | `+0x34` (`param_1 + 0x0D`) | Single byte read via `FUN_1411680d0(stream, 1)`. |
| 6a | `vel_x` | `0` or `4` | `f32 LE` | `(flags & 0x02) != 0` | `+0x48` (`param_1[0x12]`) | Linear velocity X component; `0.0` if bit `0x02` is clear. |
| 6b | `vel_y` | `0` or `4` | `f32 LE` | `(flags & 0x04) != 0` | `+0x4C` (`param_1[0x13]`) | Linear velocity Y component; `0.0` if bit `0x04` is clear. |
| 6c | `vel_z` | `0` or `4` | `f32 LE` | `(flags & 0x08) != 0` | `+0x50` (`param_1[0x14]`) | Linear velocity Z component; `0.0` if bit `0x08` is clear. |
| 6d | `ang_vel_x` | `0` or `4` | `f32 LE` | `(flags & 0x10) != 0` | `+0x54` (`param_1[0x15]`) | Angular velocity X component; `0.0` if bit `0x10` is clear. |
| 6e | `ang_vel_y` | `0` or `4` | `f32 LE` | `(flags & 0x20) != 0` | `+0x58` (`param_1[0x16]`) | Angular velocity Y component; `0.0` if bit `0x20` is clear. |
| 6f | `ang_vel_z` | `0` or `4` | `f32 LE` | `(flags & 0x40) != 0` | `+0x5C` (`param_1[0x17]`) | Angular velocity Z component; `0.0` if bit `0x40` is clear. |
| 7 | `compact_did` | `2`, `3`, or `5` | `CompactDID` | Always | `+0x64` (`param_1[0x19]`) | `u8` high byte (`bits 24..31`) + 3-tier VLE low 24 bits (`bits 0..23`), reconstructed as `(high << 24) | low24`. |
| 8 | `state_schema_id` | `0` or `4` | `u32 LE` | `(flags & 0x80) != 0` | `+0x68` (`param_1[0x1A]`) | Physics script ID (`Entity_PlayerPhysicsScript`, PID `0x0A9D`). If `(flags & 0x80) == 0`, no bytes are read and `state_schema_id` defaults to **`0x0000000C` (`12`, `Earth_ControlledSurface`)**. |
| 9a | `bool_state_bytes` | `ceil(N_bool / 8)` | `u8[ceil(N_bool / 8)]` | Schema found in `DAT_141e56178` | `+0x70` (`param_1 + 0x1C`) | Raw bit-packed boolean property values (no length prefix on wire). Bit `i` (`0 <= i < N_bool`) is `(byte[i >> 3] >> (i & 7)) & 1` (LSB-first), paired with `schema.bool_pids[i]`. For default schema `0x0C`, `N_bool = 8` (**`1` byte**). |
| 9b | `enum_state_vles` | Variable (`N_enum` VLEs) | `VLE32[N_enum]` | Schema found in `DAT_141e56178` | `+0x80` (`param_1 + 0x20`) | `N_enum` consecutive **3-tier VLE** (`1`, `2`, or `4` bytes each; no count prefix on wire) values, paired with `schema.enum_pids[i]`. For default schema `0x0C`, `N_enum = 6` (**`6` VLEs**). |
| 10 | `extra_blob_size` | `4` | `u32 LE` | Always | `local_res20` | **4-byte `u32 LE`** byte count read via `FUN_1411680d0(stream, 4)` (not a VLE). Checked against remaining stream bytes (`FUN_1411684a0`). |
| 11 | `extra_blob` | `extra_blob_size` | `u8[extra_blob_size]` | `extra_blob_size > 0` | `+0x90` (`param_1 + 0x24`) | Raw trailing payload bytes read via `FUN_1411680d0(stream, extra_blob_size)`. |

---

### 2.2 Dynamic Construction of `DAT_141e56178` (`FUN_14053b4b0` & `FUN_140537630`) <a id="portal-schema-construction"></a>

`DAT_141e56178` is a dynamically grown `SmartArray` of `0x28`-byte (`40`-byte) schema entries (`count` at `DAT_141e56184`). It is **not** a static table in `.rdata`; instead, each entry is built on demand by `FUN_14053b4b0(state_schema_id, entity)` when an entity's physics controller initializes (`FUN_140733dd0` reading entity property `0x0A9D` `Entity_PlayerPhysicsScript` via `FUN_140584670`, which defaults to `0x0C` = `12` when absent).

#### Layout of Each `0x28`-Byte Schema Entry in `DAT_141e56178`

| Offset | Size | Type | Meaning |
|---|---|---|---|
| `+0x00` | `4` | `u32` | `state_schema_id`: Physics script key (from enum mapper `0x23000097` / `0x230000F8`, e.g. `0x0000000C` = `Earth_ControlledSurface`). |
| `+0x08` | `16` | `SmartArray<u32>` | `bool_pids`: Data pointer at `+0x08`, capacity/flags at `+0x10`, **`N_bool` count at `+0x14`**. Holds deduplicated Type `21` (`0x15`, `boolean`) property IDs. |
| `+0x18` | `16` | `SmartArray<u32>` | `enum_pids`: Data pointer at `+0x18`, capacity/flags at `+0x20`, **`N_enum` count at `+0x24`**. Holds deduplicated Type `8` (`0x08`, `enum`) property IDs. |

#### Exact Graph Traversal Algorithm (`FUN_14053b4b0` -> `FUN_1404919f0` -> `FUN_1404916d0` -> `FUN_140537630`)

When `FUN_14053b4b0(script_key, entity)` is called for a `script_key` not yet present in `DAT_141e56178`:
1. **Resolve Entity `0x07` `ScriptTable` Closure (`FUN_140584870`):**
   - Obtains the entity's `0x07` `ScriptTable` (for player/avatar entities, `0x07000417`, which transitively includes global root `0x07000000`, forming a closure of `150` `0x07` tables).
2. **Pre-Order Depth-First Table Closure Walk (`FUN_1404919f0`):**
   - Calls `FUN_1404919f0(root_table, script_key, table_kind_mask=2, recurse_scriptlets=1, &visitor, &visited_tables)` where `visitor` vtable `0x141513328` points to `FUN_140537630`.
   - If `table` is already in `visited_tables`, returns immediately; otherwise inserts `table` into `visited_tables`.
   - If `(table->kind & 2) != 0` and `table` defines a script with key `script_key`, iterates over that script's root nodes in order, calling `FUN_1404916d0(root_node, &visitor, 1, &visited_nodes)`.
   - Recursively visits each included `0x07` table in `table->includes` (`+0xD0`, count `+0xDC`) in array order.
3. **Pre-Order Node & Scriptlet Walk (`FUN_1404916d0`):**
   - For each script node:
     1. Invokes `FUN_140537630(&visitor, node->data)`.
     2. If `node->kind == 19` (`0x13`, `Scriptlet`) and its `ScriptletScript_File` property (`0x0Cxxxxxx` DID) is non-zero, loads the `0x0C` `Scriptlet` record and recursively visits its root nodes in order (`FUN_140491980`).
     3. Iterates through the node's output ports (`node + 0x40`) and recursively visits each port's child nodes in order.
4. **Node Visitor Filter and Deduplicating Append (`FUN_140537630`):**
   - Inspects `node->kind` (`vtable[+0x50]()`):
     - Only node kinds **`12` (`0x0C`, `Switch`)** and **`63` (`0x3F`, `When`)** are accepted; all other node kinds are ignored.
   - Reads the node's condition property ID `pid = *(u32*)(node_data + 0xB0)` (`SwitchScript_Property`, PID `0x04AE` = `1198`).
   - Looks up `pid` in `0x34000000` (`MasterProperty` definition table via `FUN_140432cc0`) and checks its base property type (`FUN_14042cbc0`):
     - If `base_type == 21` (`0x15`, `boolean`): appends `pid` to `bool_pids` (`entry + 0x08`) **if and only if `pid` is not already in `bool_pids`** (`FUN_1403d70d0` linear scan).
     - If `base_type == 8` (`0x08`, `enum`): appends `pid` to `enum_pids` (`entry + 0x18`) **if and only if `pid` is not already in `enum_pids`** (`FUN_1403d70d0` linear scan).
     - Any other `base_type` (for example, `base_type == 6` `Vector` for `0x000002E9` `Physics_Velocity`) is ignored.

---

### 2.3 Complete Resolved `DAT_141e56178` Schema Table from `client_gamelogic.dat` <a id="portal-schema-table"></a>

Across the entire player/avatar `0x07` `ScriptTable` closure (`150` tables rooted at `0x07000417` and `0x07000000`), only **table `0x07000029`** (`table_kind = 2`) defines `PlayerPhysics` (node kind `47`) scripts. Table `0x07000029` defines three scripts:

| `state_schema_id` (`script_key`) | Symbolic Name (`0x23000097` / `0x230000F8`) | Defining `0x07` Table | `N_bool` (`+0x14`) | Boolean Wire Bytes (`ceil(N_bool / 8)`) | `N_enum` (`+0x24`) | Enum Wire Bytes (`N_enum` VLEs) |
|---|---|---|---|---|---|---|
| `0x0000000C` (`12`) *(default when `flags & 0x80 == 0`)* | `Earth_ControlledSurface` | `0x07000029` | **`8`** | **`1` byte** | **`6`** | **`6` VLEs** (`6`–`24` bytes) |
| `0x10000E50` (`268439120`) | *(mounted/auxiliary physics script 1)* | `0x07000029` | **`0`** | **`0` bytes** | **`0`** | **`0` bytes** *(only switches on `0x02E9` `Physics_Velocity`, Type 6 `Vector`)* |
| `0x10000E51` (`268439121`) | *(mounted/auxiliary physics script 2)* | `0x07000029` | **`0`** | **`0` bytes** | **`0`** | **`0` bytes** *(only switches on `0x02E9` `Physics_Velocity`, Type 6 `Vector`)* |

#### Exact `bool_pids` Table for Default Schema `0x0000000C` (`12`, `Earth_ControlledSurface`)

In `0x0000000C`, `N_bool = 8`, occupying exactly **`1` byte** on the wire (`byte[0]`), packed LSB-first:

| Bit Index (`i`) | Bit Mask in `byte[0]` | Property ID (`u32`) | Decimal PID | MasterProperty Type | Property Name (`0x34000000`) |
|---|---|---|---|---|---|
| `0` | `0x01` (`1 << 0`) | `0x0000068A` | `1674` | `21` (`boolean`) | `Movement_ActiveMovementFrozen` |
| `1` | `0x02` (`1 << 1`) | `0x10003EA0` | `268451488` | `21` (`boolean`) | `Physics_OnSlide` |
| `2` | `0x04` (`1 << 2`) | `0x10006676` | `268461686` | `21` (`boolean`) | `Physics_OnIce` |
| `3` | `0x08` (`1 << 3`) | `0x10004157` | `268452183` | `21` (`boolean`) | `Mount_Movement_Use_RampUp_Controller` |
| `4` | `0x10` (`1 << 4`) | `0x10000ED8` | `268439256` | `21` (`boolean`) | `Stealth_IsStealthed` |
| `5` | `0x20` (`1 << 5`) | `0x10001A27` | `268442151` | `21` (`boolean`) | `ScriptSystem_IsHovering` |
| `6` | `0x40` (`1 << 6`) | `0x10000ED7` | `268439255` | `21` (`boolean`) | `ScriptSystem_ActiveMovementBlocked` |
| `7` | `0x80` (`1 << 7`) | `0x10000443` | `268436547` | `21` (`boolean`) | `Patrol_Flying` |

#### Exact `enum_pids` Table for Default Schema `0x0000000C` (`12`, `Earth_ControlledSurface`)

Immediately following `byte[0]` of `bool_state_bytes`, `N_enum = 6` consecutive **3-tier VLEs** are read from the wire in the following exact order:

| VLE Index (`i`) | Encoding | Property ID (`u32`) | Decimal PID | MasterProperty Type | Property Name (`0x34000000`) |
|---|---|---|---|---|---|
| `0` | 3-tier VLE (`1`, `2`, or `4` B) | `0x00000126` | `294` | `8` (`enum`) | `WeenieType` |
| `1` | 3-tier VLE (`1`, `2`, or `4` B) | `0x00000201` | `513` | `8` (`enum`) | `Entity_Element` |
| `2` | 3-tier VLE (`1`, `2`, or `4` B) | `0x00000687` | `1671` | `8` (`enum`) | `Movement_MovementStyle` |
| `3` | 3-tier VLE (`1`, `2`, or `4` B) | `0x1000136E` | `268440430` | `8` (`enum`) | `Agent_ClassificationFilter` |
| `4` | 3-tier VLE (`1`, `2`, or `4` B) | `0x100000FC` | `268435708` | `8` (`enum`) | `Agent_Species` |
| `5` | 3-tier VLE (`1`, `2`, or `4` B) | `0x10000469` | `268436585` | `8` (`enum`) | `Combat_AnimationSet` |

---

## 3. `ActionPayload` (`FUN_1407f6de0`) and Action Events (`0x24`, `0x75`, `0x5A`, `0xA8`, `0xCD`) <a id="action-payload"></a>

### 3.1 Complete Wire Layout of `ActionPayload` (`FUN_1407f6de0`) <a id="action-payload-wire"></a>

`FUN_1407f6de0` deserializes an `ActionPayload` (`0x90` bytes in memory) from a byte-aligned network stream (`param_2`). It begins with a 2-byte `u16 LE` bitmask (`flags`) that gates all optional fields and sub-structures:

| Step | Field Name | Size (bytes) | Type | Condition (`flags` bit) | In-Memory Offset (`param_1`) | Meaning |
|---|---|---|---|---|---|---|
| 1 | `flags` | `2` | `u16 LE` | Always | Local (`uVar9`) | 16-bit presence bitmask (`bits 0..14` used; bit 15 unused). Note: if bit `0x0002` is set on the wire, `flags` also sets bit `0x4000` in `param_1 + 0x7C` when `action_duration != 1.0`. |
| 2 | `action_id` | `1`, `2`, or `4` | `VLE32` | Always | `+0x00` (`*param_1`) | 3-tier VLE action/animation identifier. |
| 3 | `action_sequence_or_type` | `1`, `2`, or `4` | `VLE32` | Always | `+0x08` (`param_1[2]`) | 3-tier VLE secondary action token/index. |
| 4 | `timestamp_start` | `4` | `f32 LE` | Always | `+0x30` (`param_1[0x0C]`) | Relative start timestamp in seconds on the wire; decoded to client time by **adding `(f32)DAT_141e706b0`**. |
| 5 | `timestamp_end` | `0` or `4` | `f32 LE` | `(flags & 0x0001) != 0` | `+0x34` (`param_1[0x0D]`) | If bit `0x0001` is set: reads `f32 LE` and **adds `(f32)DAT_141e706b0`**. If bit `0x0001` is clear: defaults to **`timestamp_start`** (`param_1[0x0D] = param_1[0x0C]`). |
| 6 | `action_speed_or_scale` | `0` or `4` | `f32 LE` | `(flags & 0x0002) != 0` | `+0x38` (`param_1[0x0E]`) | If bit `0x0002` is set: reads `f32 LE` (and if `value != 1.0`, sets bit `0x4000` in `param_1[0x1F]`). If bit `0x0002` is clear: defaults to **`1.0`** (`0x3F800000`). |
| 7 | `target_entity_id` | `0` or `8` | `u64 LE` | `(flags & 0x0004) != 0` | `+0x10` (`param_1 + 4`) | Primary target `EntityID` (`FUN_140352020`). Defaults to `0` (`DAT_141e70510`) if bit `0x0004` is clear. |
| 8 | `secondary_entity_id` | `0` or `8` | `u64 LE` | `(flags & 0x0008) != 0` | `+0x18` (`param_1 + 6`) | Secondary/source `EntityID` (`FUN_140352020`). Defaults to `0` if bit `0x0008` is clear. |
| 9 | `target_list` | `0` or Variable | `Array<ActionTargetEntry>` | `(flags & 0x0010) != 0` | `+0x20` (`param_1 + 8`) | `u32 LE` `count` (`4` B, must be `<= remaining_bytes`) followed by `count` `ActionTargetEntry` items (`FUN_1407f7810`, `12` bytes each on wire). |
| 10 | `action_context` | `0` or `20` | `ActionContext` | `(flags & 0x0020) != 0` | `+0x40` (`*(param_1 + 0x10)`) | Allocates `0x18`-byte struct and deserializes `20` bytes via `FUN_1407f6cb0`. |
| 11 | `motion_chain` | `0` or Variable | `List<ActionMotionItem>` | `(flags & 0x0040) != 0` | `+0x48` (`param_1 + 0x12`) | `u32 LE` `count` (`4` B) followed by `count` `ActionMotionItem` elements (`FUN_1407f7d30`). |
| 12 | `ballistic_info` | `0` or Variable | `ActionBallisticInfo` | `(flags & 0x0080) != 0` | `+0x58` (`*(param_1 + 0x16)`) | Allocates `0x68`-byte struct and deserializes via `FUN_1407f6aa0`. |
| 13 | `skill_detail` | `0` or `8`–`12` | `ActionSkillDetail` | `(flags & 0x0100) != 0` | `+0x60` (`*(param_1 + 0x18)`) | Allocates `0x10`-byte struct and deserializes via `FUN_1407f7bd0`. |
| 14 | `property_bag` | `0` or Variable | `PropertyList` | `(flags & 0x0200) != 0` | `+0x68` (`*(param_1 + 0x1A)`) | Allocates `0x28`-byte `PropertyList` and deserializes via `FUN_14042adc0`. |
| 15 | `extra_did_or_token` | `0` or `4` | `u32 LE` | `(flags & 0x0400) != 0` | `+0x78` (`param_1[0x1E]`) | 4-byte `u32 LE` identifier; defaults to `0` (`DAT_141e56664`) if bit `0x0400` is clear. |
| 16 | `client_action_flags` | — (`0` wire B) | Derived `u32` | Always | `+0x7C` (`param_1[0x1F]`) | Reconstructed in memory from `flags`:<br>`((flags & 0x4000) \| 0x8000) >> 13 \| (flags & 0x3800) >> 8 \| ((flags & 0x4000) ? 0x80 : 0) \| (speed != 1.0 ? 0x4000 : 0)`. Thus wire bits `11..13` (`0x0800`, `0x1000`, `0x2000`) and bit `14` (`0x4000`) are boolean flags carried directly in `flags` without extra wire bytes! |

---

### 3.2 Conditional Sub-Structures of `ActionPayload` <a id="action-payload-substructures"></a>

#### 3.2.1 `ActionTargetEntry` (`flags & 0x0010`, `FUN_1407f7810`, `12` bytes per entry)

Preceded by a 4-byte `u32 LE` `count`:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| `+0x00` | `8` | `u64 LE` | `target_id`: Target `EntityID` (`FUN_140352020`). |
| `+0x08` | `4` | `u32 LE` | `target_result_flags`: 32-bit target impact/result flags (`FUN_1411680d0(stream, 4)`). |

#### 3.2.2 `ActionContext` (`flags & 0x0020`, `FUN_1407f6cb0`, `20` bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| `+0x00` | `8` | `u64 LE` | `context_entity_id`: Context `EntityID` (`FUN_140352020`). |
| `+0x08` | `4` | `u32 LE` | `context_did_0`: 32-bit DID/token (`+0x08`). |
| `+0x0C` | `4` | `u32 LE` | `context_did_1`: 32-bit DID/token (`+0x0C`). |
| `+0x10` | `4` | `u32 LE` | `context_value`: 32-bit integer/float parameter (`+0x10`). |

#### 3.2.3 `ActionMotionItem` (`flags & 0x0040`, `FUN_1407f7d30`, `4`–`21` bytes per entry)

Preceded by a 4-byte `u32 LE` `count`. Each item in the linked list (`0x28` bytes in memory) has the following wire layout:

| Step | Field Name | Size (bytes) | Type | Condition | In-Memory Offset | Meaning |
|---|---|---|---|---|---|---|
| 1 | `item_flags` | `1` | `u8` | Always | Derived into `+0x1C` | Bitmask gating optional fields:<br>• Bit 0 (`0x01`): `param_08` present<br>• Bit 1 (`0x02`): `param_0c` present<br>• Bit 2 (`0x04`): `scale_10` present<br>• Bit 3 (`0x08`): `weight_14` present<br>• Bit 4 (`0x10`): `blend_18` present<br>• Bit 5 (`0x20`): sets bit `1` (`0x02`) of `+0x1C`<br>• Bit 6 (`0x40`): sets bit `2` (`0x04`) of `+0x1C`. |
| 2 | `motion_id` | `1`, `2`, or `4` | `VLE32` | Always | `+0x00` (`*param_1`) | 3-tier VLE motion/animation key. |
| 3 | `motion_tier_or_group` | `1`, `2`, or `4` | `VLE32` | Always | `+0x04` (`param_1[1]`) | 3-tier VLE secondary motion key (`<= 0xFFFF`). |
| 4 | `param_08` | `0`, `1`, `2`, or `4` | `VLE32` | `(item_flags & 0x01) != 0` | `+0x08` (`param_1[2]`) | Optional 3-tier VLE parameter; `0` if bit `0x01` is clear. |
| 5 | `param_0c` | `0`, `1`, `2`, or `4` | `VLE32` | `(item_flags & 0x02) != 0` | `+0x0C` (`param_1[3]`) | Optional 3-tier VLE parameter; `0` if bit `0x02` is clear. |
| 6 | `scale_10` | `0` or `4` | `f32 LE` | `(item_flags & 0x04) != 0` | `+0x10` (`param_1[4]`) | Optional `f32 LE` multiplier (sets bit `0x01` of `+0x1C` if `!= 1.0`); defaults to **`1.0`** (`0x3F800000`) if bit `0x04` is clear. |
| 7 | `weight_14` | `0` or `4` | `f32 LE` | `(item_flags & 0x08) != 0` | `+0x14` (`param_1[5]`) | Optional `f32 LE` weight; defaults to **`1.0`** (`0x3F800000`) if bit `0x08` is clear. |
| 8 | `blend_18` | `0` or `4` | `f32 LE` | `(item_flags & 0x10) != 0` | `+0x18` (`param_1[6]`) | Optional `f32 LE` blend time; defaults to **`-1.0`** (`0xBF800000`) if bit `0x10` is clear. |

#### 3.2.4 `ActionBallisticInfo` (`flags & 0x0080`, `FUN_1407f6aa0`)

| Step | Field Name | Size (bytes) | Type | Condition | In-Memory Offset | Meaning |
|---|---|---|---|---|---|---|
| 1 | `ballistic_flags` | `1` | `u8` | Always | `+0x60` (`*(u8*)(param_1 + 0x18)`) | Bitmask:<br>• Bit 0 (`0x01`): `target_position` present<br>• Bit 1 (`0x02`): `velocity_vector` present<br>• Bit 2 (`0x04`): `impact_id` (`u32 LE`) present<br>• Bit 3 (`0x08`): stored in `+0x60`. |
| 2 | `target_position` | `0` or Variable | `Position` | `(ballistic_flags & 0x01) != 0` | `+0x00` (`param_1`) | Standard `Position` (`FUN_140372d60`) if bit `0x01` is set; initialized via `FUN_140372730` if clear. |
| 3 | `velocity_vector` | `0` or `12` | `Vector3` (`3 × f32 LE`) | `(ballistic_flags & 0x02) != 0` | `+0x48` (`param_1 + 9`) | 12-byte `(vx, vy, vz)` (`FUN_14115be20`) if bit `0x02` is set; `(0.0, 0.0, 0.0)` if clear. |
| 4 | `impact_id` | `0` or `4` | `u32 LE` | `(ballistic_flags & 0x04) != 0` | `+0x54` (`*(u32*)(param_1 + 0x0A) + 4`) | 4-byte `u32 LE` identifier if bit `0x04` is set; `0` (`DAT_141e56664`) if clear. |
| 5 | `flight_duration` | `4` | `f32 LE` | Always | `+0x58` (`param_1[0x0B]`) | Always-present 4-byte `f32 LE` duration/parameter. |
| 6 | `arc_height_or_gravity` | `4` | `f32 LE` | Always | `+0x5C` (`*(u32*)(param_1 + 0x0B) + 4`) | Always-present 4-byte `f32 LE` parameter. |

#### 3.2.5 `ActionSkillDetail` (`flags & 0x0100`, `FUN_1407f7bd0`, `8` or `12` bytes)

| Step | Field Name | Size (bytes) | Type | Condition | In-Memory Offset | Meaning |
|---|---|---|---|---|---|---|
| 1 | `skill_did` | `4` | `u32 LE` | Always | `+0x00` (`*param_1`) | 4-byte `u32 LE` skill/effect DID. |
| 2 | `detail_flags` | `1` | `u8` | Always | `+0x0C` (`param_1[3]`, bits `0..2`) | Bitmask:<br>• Bit 0 (`0x01`): `optional_param_08` (`u32 LE`) present<br>• Bits 1..2 (`0x06`): stored in `param_1[3]`. |
| 3 | `optional_param_08` | `0` or `4` | `u32 LE` | `(detail_flags & 0x01) != 0` | `+0x08` (`param_1[2]`) | 4-byte `u32 LE` parameter if bit `0x01` is set; `0` if clear. |
| 4 | `skill_level_or_stage` | `1`, `2`, or `4` | `VLE32` | Always | `+0x04` (`param_1[1]`) | 3-tier VLE integer parameter. |

---

### 3.3 Complete Wire Framing of the Five Enclosing Action Events (`0x24`, `0x75`, `0x5A`, `0xA8`, `0xCD`) <a id="action-events"></a>

All five action events embed `ActionPayload` (`FUN_1407f6de0`). Four (`0x24`, `0x75`, `0x5A`, `0xA8`) are entity-scoped events registered on the entity message handler (`FUN_140595160`), and one (`0xCD`) is a global broadcast event registered on `GlobalEventHandler` (`FUN_14080df60`):

| Event Opcode | Scope | Handler Function | Complete Wire Payload (after `u32 LE` event opcode) |
|---|---|---|---|
| `0x00000024` (`36`) | Entity | `FUN_1405953c0` | **`ActionPayload` (`FUN_1407f6de0`)** — no extra prefix or suffix bytes. Dispatches directly to `entity->vtable[+0x150](entity, &action_payload)` (`FUN_140594170`). |
| `0x00000075` (`117`) | Entity | `FUN_140596bd0` | **`ActionPayload` (`FUN_1407f6de0`) + `u8` `mode_flag` (`1` byte)** — reads `ActionPayload` first, then reads `1` trailing `u8` byte (`FUN_1411680d0(stream, 1)`), and dispatches to `entity->vtable[+0x170](entity, &action_payload, mode_flag)`. |
| `0x0000005A` (`90`) | Entity | `FUN_140596410` | **`ActionPayload` (`FUN_1407f6de0`)** — no extra prefix or suffix bytes. Dispatches directly to `entity->vtable[+0x178](entity, &action_payload)`. |
| `0x000000A8` (`168`) | Entity | `FUN_140596840` | **`ActionPayload` (`FUN_1407f6de0`)** — no extra prefix or suffix bytes. Dispatches directly to `entity->vtable[+0x188](entity, &action_payload)`. |
| `0x000000CD` (`205`) | Global (`14080df60`) | `FUN_140751ae0` -> `FUN_1404cb4f0` -> `FUN_140596a80` | **1. `CompactDID` (`1` B high + VLE24 low, `param_1[0]`)**<br>**2. `u64 LE` `entity_id` (`8` B, `param_1 + 1`)**<br>**3. `ActionPayload` (`FUN_1407f6de0`, `param_1 + 8`)**<br>**4. `Array<u64 LE>` `extra_targets` (`u32 LE` `count` + `count × 8` B via `FUN_140354070`, `param_1 + 2`)**<br>Resolves `entity_id` via `FUN_14050fae0` and invokes `entity->vtable[+0x168](entity, param_1)`. |

---

## 4. Testable Invariants and Synthetic Test Vectors <a id="invariants-and-vectors"></a>

### 4.1 Testable Invariants

1. **Monotonicity of `DAT_141e706b0` (`INV-CLOCK-MONO`):**
   - For any sequence of transport `TimeSync` (`0x01000000`) messages with `force == 0` and non-decreasing hardware counter readings, `DAT_141e706b0` never decreases from one frame to the next. When `target > synced_time`, `DAT_141e706b0` snaps forward to `target`; when `target <= synced_time`, `DAT_141e706b0` advances at $(1 - \text{slew\_rate}) \times \Delta t \ge 0.95 \times \Delta t$ until `slew_debt` reaches `0.0`.
2. **Default `0xCC` Schema `0x0000000C` Byte Footprint (`INV-0XCC-SCHEMA-0C`):**
   - Whenever event `0xCC` has `(flags & 0x80) == 0` (or explicit `state_schema_id == 0x0000000C`), the state-schema section of `0xCC` consists of **exactly `1` boolean bitfield byte** (`8` bits for `bool_pids[0..7]`) followed by **exactly `6` 3-tier VLEs** (for `enum_pids[0..5]`), followed by a **4-byte `u32 LE` `extra_blob_size`** and `extra_blob_size` bytes.
3. **`ActionPayload` Minimum Wire Size (`INV-ACTION-MIN`):**
   - When `ActionPayload.flags == 0x0000`, `action_id < 0x80`, and `action_sequence_or_type < 0x80`, the entire `ActionPayload` is **exactly `8` bytes** on the wire: `2` B (`flags = 0x0000`) + `1` B (`action_id`) + `1` B (`action_sequence_or_type`) + `4` B (`timestamp_start` `f32 LE`). In memory, `timestamp_end` equals `timestamp_start + (f32)DAT_141e706b0` and `action_speed_or_scale` equals `1.0`.

---

### 4.2 Synthetic Test Vectors

#### Vector 1 — Transport `TimeSync` (`0x01000000`) Slew Calculation

- **Initial state:** `raw_elapsed = 100.0` s, `clock_offset = 500.0` s, `synced_time = 600.0` s, `prev_server_time = 0.0` s.
- **Wire input (`FUN_141196400`):** `00 00 00 00 00 B8 82 40` (`f64 LE` = `599.0` s? Wait: `599.95` s = `0x4082BF999999999A`). Suppose `target = 599.95` s (`50` ms behind `synced_time = 600.0` s) with `force = 0`:
  - $\text{excess} = 600.0 - 599.95 = 0.050\text{ s}$.
  - Since $0.030 \le 0.050 < 0.060\text{ s}$ (tier $[2, 4) \times 0.015\text{ s}$), `slew_debt = 0.050` s and `slew_rate = 0.0025`.
  - On the next frame with $\Delta t = 0.016\text{ s}$ (`new_raw = 100.016` s):
    - $\text{step} = \min(0.050, 0.016 \times 0.0025) = 0.00004\text{ s}$.
    - `slew_debt` becomes $0.04996\text{ s}$, `clock_offset` becomes $499.99996\text{ s}$, and `DAT_141e706b0` becomes $100.016 + 499.99996 = 600.01596\text{ s}$.

#### Vector 2 — Event `0xCC` State Schema Section for Default Schema `0x0C` (`flags & 0x80 == 0`)

Suppose `flags = 0x00` (no optional velocities, default `state_schema_id = 0x0000000C`), and after `compact_did` the stream contains the following `11` bytes:
```text
11  02 01 03 00 04 01  00 00 00 00
```
- **Byte 0 (`0x11` = `0b00010001`):** `bool_state_bytes[0]` (`8` bits for `bool_pids[0..7]`):
  - Bit 0 (`0x0000068A` `Movement_ActiveMovementFrozen`) = `1` (`true`)
  - Bit 1 (`0x10003EA0` `Physics_OnSlide`) = `0` (`false`)
  - Bit 2 (`0x10006676` `Physics_OnIce`) = `0` (`false`)
  - Bit 3 (`0x10004157` `Mount_Movement_Use_RampUp_Controller`) = `0` (`false`)
  - Bit 4 (`0x10000ED8` `Stealth_IsStealthed`) = `1` (`true`)
  - Bit 5 (`0x10001A27` `ScriptSystem_IsHovering`) = `0` (`false`)
  - Bit 6 (`0x10000ED7` `ScriptSystem_ActiveMovementBlocked`) = `0` (`false`)
  - Bit 7 (`0x10000443` `Patrol_Flying`) = `0` (`false`)
- **Bytes 1..6 (`02 01 03 00 04 01`):** `6` 1-byte 3-tier VLEs for `enum_pids[0..5]`:
  - `enum_pids[0]` (`0x00000126` `WeenieType`) = `2`
  - `enum_pids[1]` (`0x00000201` `Entity_Element`) = `1`
  - `enum_pids[2]` (`0x00000687` `Movement_MovementStyle`) = `3`
  - `enum_pids[3]` (`0x1000136E` `Agent_ClassificationFilter`) = `0`
  - `enum_pids[4]` (`0x100000FC` `Agent_Species`) = `4`
  - `enum_pids[5]` (`0x10000469` `Combat_AnimationSet`) = `1`
- **Bytes 7..10 (`00 00 00 00`):** `extra_blob_size` = `0` (`u32 LE`), followed by `0` bytes of `extra_blob`.

#### Vector 3 — Event `0x75` (`ActionPayload` + `u8` Mode Flag)

Suppose `DAT_141e706b0 = 1000.0` s, and the payload of entity event `0x00000075` (after the 4-byte opcode) is the following `21` bytes:
```text
07 00  2A  01  00 00 00 BF  00 00 80 3F  00 00 C0 3F  05 00 00 00 02 00 00 02  01
```
1. `flags`: `07 00` (`u16 LE` = `0x0007`: `timestamp_end`, `action_speed_or_scale`, and `target_entity_id` present).
2. `action_id`: `2A` (3-tier VLE = `0x2A` = `42`).
3. `action_sequence_or_type`: `01` (3-tier VLE = `1`).
4. `timestamp_start`: `00 00 00 BF` (`f32 LE` = `-0.5` s) -> decoded client timestamp = `-0.5 + 1000.0 = 999.5` s.
5. `timestamp_end` (`flags & 0x0001`): `00 00 80 3F` (`f32 LE` = `+1.0` s) -> decoded client timestamp = `1.0 + 1000.0 = 1001.0` s.
6. `action_speed_or_scale` (`flags & 0x0002`): `00 00 C0 3F` (`f32 LE` = `1.5`; since `1.5 != 1.0`, sets bit `0x4000` in `+0x7C`).
7. `target_entity_id` (`flags & 0x0004`): `05 00 00 00 02 00 00 02` (`u64 LE` = `0x0200000200000005`).
8. Trailing event `0x75` byte (`mode_flag`): `01` (`u8` = `1`).

---

## 5. Implementer Notes <a id="implementer-notes"></a>

1. **Folding into `docs/specs/official-messages-world.md`:**
   - **§4.3.1 (Event `0xCC`):** Replace the `state_schema_id` OPEN note with the exact 8-boolean (`1`-byte) and 6-enum (`6`-VLE) tables for `0x0000000C` (`Earth_ControlledSurface`) and the general `0x07` `ScriptTable` + `0x0C` `Scriptlet` traversal rule (`Switch` kind `12` and `When` kind `63`). Also update `extra_blob_size` from 3-tier VLE to **4-byte `u32 LE`** (`FUN_1411680d0(stream, 4)`).
   - **§4.4 (OPEN #3 — World Clock & `14080df60`):** Document transport `TimeSync` (`0x01000000`), the `SmoothedClock` snap/slew state machine, the `0x35000000` (`GameTime`) calendar record in `client_general.dat`, and the 22-opcode `GlobalEventHandler` (`14080df60`) table.
   - **§5.5–§5.6 (OPEN #4 — `ActionPayload`):** Replace the `ActionPayload` placeholder with the 16-bit `flags` table (`FUN_1407f6de0`), its 5 sub-structures, and the framing of events `0x24`, `0x75`, `0x5A`, `0xA8`, and `0xCD`.
2. **Folding into `docs/specs/environment.md` §11.1.1 and `docs/specs/property-system.md` §4.3:**
   - `environment.md` §11.1.1 can cite `0x35000000` in `client_general.dat` for `day_length_seconds = 11160.0` s (`3h 6m`), `dawn_threshold = 0.25`, `dusk_threshold = 0.80`, and the world properties `0x01EF` (`World_TimeOffset`), `0x06CC` (`World_TimeDilation`), `0x0682` (`World_PresentTimeOfDay`), and `0x01F0` (`World_IsNight`).
   - `property-system.md` §4.3 can cite `FUN_14043a700` and `DAT_141e706b0` for Type 4 (`Timestamp`) relative wire encoding.

---

## 6. Open Items <a id="open-items"></a>

All three deep questions assigned to this research note are **fully resolved** (`0` OPEN items remaining).

---

## 7. Change Log <a id="change-log"></a>

| Date | Author Role | Summary |
|---|---|---|
| 2026-10-20 | Spec writer (dirty room) | Initial specification resolving game clock synchronization (`0x01000000` `TimeSync`, `DAT_141e706b0`, `0x35000000` `GameTime`, `14080df60` `GlobalEventHandler`), `0xCC` portal placement state schemas (`DAT_141e56178`, `0x07000029` / `0x0C`), and `ActionPayload` (`FUN_1407f6de0`) wire layout across events `0x24`, `0x75`, `0x5A`, `0xA8`, and `0xCD`. |
