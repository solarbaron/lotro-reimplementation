<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Extended scripting API, slice 1b: Camera, Overlay, Sandbox, hot reload, console and generators

| Field | Value |
|---|---|
| Status | **Accepted** (2026-10-07) |
| Last updated | 2026-10-07 |
| Author role | Design (Tessera's own design under decision 0017; clean room, no dirty-room input), AI-assisted |
| Reviewer | Independent clean-room review |
| Review record | Review of revision 6, 2026-10-07: **accepted**, with minor follow-ups for the next revision. Full record moved to the review log (decision 0018). |
| Implements into | `crates/ext` (registry entries, validation, leases, overlay lists, hand-off serialisation), `crates/ext-host` (Camera, Overlay and Sandbox ports, the clock port, the file watcher, the reload sequence), `crates/render` (filled world boxes with back faces culled and a world style without depth test, §5.4), `apps/client` (feature `ext`: lease application in step 3, pointer picking under a lease, overlay encoding and coverage counting in step 10, the lease indicator, the developer console), `apps/scripttool` (`dump-api`, EM §11.2), `crates/net` (one additive event, §7.5), `tools/` (the reference check, §10.6) |
| Related | Decision [0017](../decisions/0017-extended-scripting.md) (**D17**, accepted 2026-10-07); [`ext-threat-model.md`](ext-threat-model.md) (**TM**, normative for every limit, every rule of its §7–§9 and the escape catalogue); [`ext-api-1a.md`](ext-api-1a.md) (**EA1a**, the slice-1a modules and the general rules of the API); [`ext-manifest.md`](ext-manifest.md) (**EM**, manifest, grants, backend gate, the script tool and the walk file); [0014](../decisions/0014-ui-input-bridge.md) (**IB**), [0015](../decisions/0015-ui-rendering.md) (**UR**), [`keymap.md`](keymap.md) (**KM**), `docs/ARCHITECTURE.md` (**ARCH**), `docs/LEGAL.md` §18 |

> **Clean-room and naming note.** Everything here is Tessera's own design, written from the
> accepted decision D17, Tessera's other decisions and specs, and Tessera's own code
> (`tessera-net`'s command model, `tessera-world`'s game clock, `tessera-render`'s pass
> primitives, the client's streaming focus). No dirty-room material was read. LEGAL §18.3
> rule 1 applies: no fact that rests on disassembly is part of this contract. In particular
> the camera behaviour that `walker.md` §11 records from the original client (its field of
> view, its look-at heights, its activation rules) is **not** the behaviour of the `Camera`
> module. The module names, function names and every string in this document are neutral
> (LEGAL §10, §18.3 rule 4) and none is an interop name of the plugin API.

---

<a id="s1"></a>
## 1. Purpose and scope

D17 splits the first extended-scripting release into slice 1a (M5.5: the core, `Core`,
`Store`, `Commands`, `UI`, `Input`, `World`, `Entity`, the panic chord, `check`, `test`) and
slice 1b, which ships right after it. EA1a and this document together are what D17 calls
`ext-api.md` for slices 1a and 1b (TM header). This document is the normative reference for
what slice 1b adds:

| Part | Section | D17 source |
|---|---|---|
| `Camera` with leases and the lease indicator | §4 | §3 table, §5.4 |
| `Overlay` | §5 | §3 table, §8 table |
| GPU coverage of ext overlays | §6 | open security and design item 1, through TM §7.1 |
| `Sandbox` with the clock port and its rate limits | §7 | §3 table, §7 "Chat", §8 table |
| Hot reload, the file watcher and the hand-off table | §8 | §10; open security and design item 6 (through TM §7.6); "Other improvements", partially loaded apartments at a backend switch (through TM §8.21) |
| The developer console | §9 | §10 "Console", §5.3; "Other improvements", script log (through TM §8.1, §8.2) |
| The generators: API reference, editor annotations, runtime validation | §10 | §1.3, §10 "One source of truth" |

**Not in this document.** The command lines of `tessera-scripttool` (`dump-api`, `init` and
`pack` among them) and the walk-file grammar are EM's (§11.2, §11.5); §10 states only what
`dump-api` generates. The general rules every module follows (`Ext.Use`, `Ext.Has`, value
types, the three ways to fail, costs, timing, handles, handlers, capability checks) are
EA1a's (§2, §3). Every limit, security rule and escape case is TM's. This document restates
a rule only where a slice-1b module needs its script-visible form, and then cites the
source.

**Precedence.**
1. **TM** wins on every limit (TM §10, cited "TM L*n*"), on every rule of TM §7–§9 and on
   the escape cases (TM §11, cited "TM X*n*"), as TM §0.1 says. A number repeated here next
   to its L-number is for the reader; if it differs from TM §10, TM wins and this document
   has a defect.
2. **EA1a** wins on the general rules of the API (EA1a §2–§3).
3. For the slice-1b modules, their names, arguments, results, notifications and the
   generators' outputs, this document is the contract, and the generated reference (§10.4)
   must agree with it.

<a id="s2"></a>
## 2. Provenance

| Facts | Source | Mark |
|---|---|---|
| Module list, capabilities, groups, backends, apply steps, the lease lifetime and end conditions, the overlay caps, the Sandbox rate limits, hot-reload rules, the console's limits, the generators' outputs | D17 (accepted 2026-10-07), sections cited per row | Decision |
| Every limit number (and which numbers are limits: TM §0.1 rule 6), the coverage rules, the view-dependent overlay statistics reserved to `world.read`, the frame-budget row and E9, read-your-writes, the log format, reserved-layer text, the declaration set of a hot reload, uncharged host memory, the lease end events (the unload request included), no lease or intent from an `Unload` handler, picking under a lease, revocation (the grant-store re-read and handle checks by minting module included), split turns, the teleport slot, its per-connection correlation and the all-scripts clock-change rate, the outstanding-request count per (kind, id) shared by both apartments of a hot reload (L27), the leased eye's reference point (L82), a full reload's load after the old close item and after the old cap has left the sum (L84), the watcher's fixed flag table as a design narrowing of D17 §10 (L84, TM §13), the lease end reasons as a stated residual (TM §6.8 item (6)), the escape cases | TM (revision 6, 2026-10-07), sections and L-, X-numbers cited per use | Decision |
| General API rules: value types (colour, position, `DID`, `table`), angles (yaw and pitch in radians, `heading` only as a result), failures and the order of checks, costs and the element-counting rule, timing, handles, the handler pattern, the `Ext` root and `Ext.Version`, `Core.Log` and `print`, the `Core` notifications `"Tick"` and `"Unload"` (`Core.On`) | EA1a §2.3–§2.10, §3, §4.1, §4.3 | Decision |
| Script-tool commands, the walk file, the backend gate states (the generation rising when a switch or session end is queued), manifest fields and validation, the developer-mode reload rules and notices | EM §4, §9.8, §10, §11 | Decision |
| Every API shape, name, range, default, formula and rule not found in the sources above | This document, original design, 2026-10-07 | Decision (design) |
| Region frame, `f64` world positions | ARCH §9.1 | Decision |
| Teleport destination forms, the teleport answer rule (refusal and supersession answered only by a system chat line today), coordinate and place-name limits (`MAX_LOCATION_COORDINATE` = 10⁷, `MAX_PLACE_NAME_LEN` = 256 bytes, a place name not only white space and with no control character), landblock indices 0–254 (`MAX_LANDBLOCK`), an anchor's ownership (same region, block-local `x` and `y` within ±1,280 m, `MAX_LOCAL_OFFSET`); `Session::send` accepts `Teleport` and `QueryLocation` only in the `InWorld` state (`NetError::InvalidState` otherwise) and passes on the backend's errors; `tessera-net`'s `heading` convention (`f32` radians, 0 = +X, counter-clockwise), which is EA1a §2.10's yaw convention; `RegionId` is the world source's region number (`coordinates.md` §4 with the user's data), and `RegionId(0)` is the abstract region of a sandbox that has no world data | `crates/net/src/model.rs`, `crates/net/src/session.rs`, read 2026-10-07 | Confirmed (code) |
| The clock setters and their clamps (`MAX_GAME_SECONDS` = 10¹²) | `crates/world/src/environment/clock.rs`, read 2026-10-07 | Confirmed (code) |
| A pass offers `line`, `aabb` (outline only) and `quad` primitives with linear RGBA colours; world passes use reversed depth. There is no filled box and no world style without depth test | `crates/render/src/frame.rs`, `crates/render/src/camera.rs`, read 2026-10-07 | Confirmed (code) |
| The client's streaming focus is computed from the player's controls (`stream_focus` over `controls.focus()`), not from the rendered camera | `apps/client/src/walker/mod.rs`, read 2026-10-07 | Confirmed (code) |
| The reference machine's GPU (RTX 2070 Super Max-Q) and display (3840 × 2160) | `docs/perf/reference-machine.md` | Confirmed |
| The LuaLS annotation syntax (`---@meta`, `---@class`, `---@field`, `---@param`, `---@return`, `---@alias`, `---@overload`) | The public documentation of the lua-language-server project (MIT) | Reported (public format) |
| The `bad argument #n to 'f' (…)` message form | The Lua 5.1 reference implementation's auxiliary library (MIT), public behaviour | Reported (public format) |
| The sRGB transfer function (IEC 61966-2-1) | Public standard | Public mathematics |
| The JSON Canonicalization Scheme (RFC 8785) and SHA-256 (FIPS 180-4), used for the registry's canonical form (§10.1) | Public standards | Public format |

<a id="s3"></a>
## 3. Conventions

<a id="s3-1"></a>
### 3.1 Units, frames and value forms

The value forms are EA1a's (§2.3, §2.10), so a `yaw` or a position read from `World` or
`Entity` passes to `Camera` and `Sandbox` unchanged. Every angle a slice-1b function takes
is in **radians**; no argument is ever a heading (EA1a §2.10).

| Quantity | Ext form |
|---|---|
| World position | `x`, `y`, `z`: numbers, metres, in the frame of the player's **current region** (EA1a §2.1 "Region frame", ARCH §9.1), `+Z` up. Every coordinate a script passes must be finite with an absolute value of at most 10⁷ (EA1a §2.3 `position`), or the call raises a Lua error. |
| Region | **In an argument:** integer 1–255; 0 is a Lua error, because 0 means "no region" (`coordinates.md` §4), as for EA1a's `region` (EA1a §7.9). **In a result:** the number of the player's current region, 1–255 with the user's data; 0 ("no region") only while the `Offline` session reports `tessera-net`'s `RegionId(0)`, the abstract region of a sandbox that has no world data (§2), where positions are in that sandbox's one frame. A script there cannot name its region in a destination (§7.2) and teleports only by a named place. The synthetic worlds of this document's tests use a region from 1 to 255 (§13; for `scripttool test`'s world, EM §11.5 is asked to state the number, an OPEN in §13). |
| Yaw | EA1a §2.10 `yaw`: **radians**, counter-clockwise from `+X` seen from above, 0 facing `+X` and π/2 facing `+Y` (KM §9.1). Any finite value is accepted (read modulo 2π); values the host returns are wrapped into (−π, π]. An offset about `+Z` relative to a yaw (`Follow`'s `yaw` option, §4.7) is in the same form (EA1a §2.10). |
| Heading | EA1a §2.10 `heading`: a **result only**, the compass bearing in degrees in [0, 360) of the same direction as the `yaw` beside it, for display. Where a table the host returned carries `heading` and a script passes that table back, the field is ignored (§4.2). |
| Pitch | EA1a §2.10 pitch: **radians** above the horizontal, with \|pitch\| ≤ 88 · π / 180 (≈ 1.53589). Outside: Lua error. |
| Field of view | vertical, **radians**, in [20 · π / 180, 100 · π / 180] (≈ 0.34907 to 1.74533). Outside: Lua error. (A field of view is an extent, whose unit EA1a §2.10 leaves to the module; `Camera` takes radians, like every other angle of this document.) |
| Screen position | UI units of the game window (EA1a §2.1 "UI unit"): origin top-left, `x` right, `y` down. |
| Physical pixels | pixels of the swap-chain surface: UI units times the UI scale. Coverage (§6) counts physical pixels. |
| Colour | EA1a §2.3 `colour`: a string `#RRGGBB` or `#RRGGBBAA`, sRGB-encoded channels and straight (not premultiplied) alpha, alpha `FF` when omitted. `Overlay.Rgba` (§5.3) builds one. |
| Time | seconds, numbers. |
| Direction of view | for yaw ψ and pitch θ, the unit vector d(ψ, θ) = (cos θ · cos ψ, cos θ · sin ψ, sin θ). |

**Domains and limits.** The ranges of single scalar arguments and fields in this document
(coordinate magnitudes, the pitch range of ±88° and the field-of-view range of 20°–100°
above, `Follow`'s distance of 0.5–50 m and height of −10 to 10 m, a quad's position and
size of at most 16,384 UI units, region, landblock and cell numbers) are argument-validity
domains in the sense of TM §0.1 rule 6 (a): each bounds no cost by itself, stays in this
document and has no L-number. Every count, size, rate, time and per-call cap that bounds
work, memory or the user's control is TM's and is cited by its L-number: among them the
`Overlay.Lines` per-call cap (TM L90), the 32 entries of a REPL result description and
the one overlay drop record per 10 s (both TM L91). Where a domain bounds something TM
relies on, TM registers it all the same (the 2,000 m box extent, TM L83).

<a id="s3-2"></a>
### 3.2 Results, reasons and errors

Every function of this document ends in one of EA1a §2.4's three ways: a result, a soft
failure (`nil` or `false` and a **reason**), or a Lua error (misuse, a stale handle, a
revoked grant, a cost over the call's remainder, a refused allocation). An abort ends the
call as EA1a §2.4 and TM §8.11 say. Refusals are expected outcomes and never count as
aborted calls (D17 §8). Slice 1b uses EA1a's reasons and adds these:

| Reason | Meaning | Used by |
|---|---|---|
| `"busy"` (EA1a) | A per-frame or per-second cap is reached | `Overlay` adds, `Sandbox` commands, camera intents past the pending-intent cap |
| `"taken"` | Another script holds the camera lease, or the calling apartment is unloading (§4.5, TM §8.17: "refused as if taken") | `Camera.Take` |
| `"unloading"` | The calling apartment's unload has been requested (§4.5); this includes every call of its `Unload` handler. Nothing is added or queued (TM §8.17) | `Overlay` adds, `Sandbox` commands |
| `"unfocused"` | The game window does not have keyboard focus, so no lease may start (D17 §5.4) | `Camera.Take` |
| `"ended"` | The lease has ended; `lease:EndReason()` says why | lease methods |
| `"pending"` | A teleport is pending, of any origin (D17 §7 "Chat") | `Sandbox.Teleport` |
| `"not in world"` | The session is not in the world | `Sandbox` commands |
| `"too large"`, `"cyclic"`, `"invalid"` (EA1a) | The bounded value model refused a value (TM L33–L35); `"too large"` also when the hand-off would not fit the new apartment's cap (TM §8.10) | `Core.SetHandOff` |
| `"not a hot reload"` | The hand-off table is accepted only in the `Unload` handler of a hot reload | `Core.SetHandOff` |
| `"refused"`, `"cancelled"`, `"failed"` | Not soft failures: the values of `TeleportDone`'s `reason` field (§7.5) | `Sandbox` notifications |

<a id="s3-3"></a>
### 3.3 Notifications and handlers

Handlers follow EA1a §2.8 (one handler per event per object, `nil` clears, one call per
notification with its conversion charged first, the queue cap of TM L30, nothing accrues
while suspended). Each slice-1b module that produces notifications sets its handlers with
one module function, EA1a §2.8's handler form 1 (module notifications, `Module.On(name,
fn)`):

- `Module.On(name, fn)`: `name` is one of the module's notification names (§4.10, §7.6);
  any other string raises a Lua error. `fn` is a function or `nil`.
- A notification is **queued only if a handler is set when it arises**. It is delivered in
  the script's next turn (EA1a §2.6 "next turn") with one argument: a table of the
  notification's fields.

<a id="s3-4"></a>
### 3.4 How the reference entries read

Function tables use these columns:

- **Results:** the values of a normal result; refusals and their reasons follow `|`.
- **Apply:** where the effect takes place: EA1a §2.6's *read* and *at once*, and *step 3
  next*, *step 2 next* (that step of the next frame, ARCH §20.3; D17 §4 "Intents") or
  *step 10* (this frame).
- **Cost:** EA1a §2.5's symbols (`B`, `b(n)`, `e(n)`, `V`), with *n* in `e(n)` counted by
  EA1a §2.5's rule (every key and every value: a record of *k* fields is 2*k*, an array of
  *m* entries 2*m*); "Rsv" marks a reservation of
  the cap's cost before the work, the unused part refunded (D17 §8 "Work units"). The
  figures are provisional like EA1a's (EA1a OPEN-1): E7 calibrates them and the registry
  holds them.
- **Since:** the module minor version that added the entry. Every module of this document
  starts at major 1, minor 0; `Core`'s two additions (§8.4) are `Core` 1.1, its first minor
  after the slice-1a release.

---

<a id="s4"></a>
## 4. `Camera`

<a id="s4-1"></a>
### 4.1 Module card

| Field | Value |
|---|---|
| Name and version | `Camera`, major 1, minor 0 |
| Capability | `camera` |
| Group | **A** (act for you: a lease replaces the user's view, D17 §3) |
| Backends | `Offline`. `TesseraProtocol` only from slice 3, and only when the server's policy lists `camera` (D17 §6.5); until then the gate admits `Offline` alone (EM §10.2) |
| Apply | reads at once; lease intents at step 3 of the next frame |
| Slice | 1b |
| Grant warnings | `camera` + `entity.read`; `camera` on a server (D17 §5.2, EM §6.2) |

<a id="s4-2"></a>
### 4.2 The pose

A **pose** is a table with these fields. The host returns new tables; a script passes a
table with the same fields, read by raw access only (EA1a §2.3 `table`), so metatables and
`__index` are ignored.

| Field | Type | In a returned pose | In a pose a script passes |
|---|---|---|---|
| `x`, `y`, `z` | number | the eye position of the view; **only when the script holds `world.read`** (below) | required, §3.1 ranges |
| `yaw` | number | the view's yaw, radians, in (−π, π] | required, finite (§3.1) |
| `pitch` | number | the view's pitch, radians | required, \|pitch\| ≤ 88 · π / 180 (§3.1) |
| `fov` | number | the vertical field of view in use, radians | optional; absent means "keep the lease's field of view" (§4.7); §3.1 range |
| `heading` | number | the compass bearing of `yaw` in degrees, [0, 360) (EA1a §2.10; display only) | ignored (never an argument, EA1a §2.10) |
| `region` | integer | the region of the frame the position is in; **only with `world.read`** | ignored (positions are always in the current region's frame) |
| `leased` | boolean | `true` when a lease decided this view | ignored |

So a pose returned by `Camera.GetPose` to a script that holds `world.read` passes to
`lease:Hold` unchanged, and the ignored fields are the only fields a passed pose may carry
besides the required and optional ones (any other field name is a Lua error, §10.2). A
passed pose is read with the bounded walk of §10.2 after the reservation (§10.3): at most
its 9 fields plus one unknown field, 10 keys and their values, `e(20)`.

**Positions need `world.read`.** The eye of the view is always near the player (the user's
camera, or a leased eye within 500 m, §4.6), so it tells where the player is. The `camera`
grant's dialog text (EM §6.1, "Take over the camera while it runs") does not say that, and
`world.read`'s does ("Read where you are", EM §6.1). A returned pose therefore carries `x`,
`y`, `z` and `region` only when `world.read` is in the apartment's effective set at the
call (TM §8.23); otherwise those four fields are absent, and the pose cannot be passed back
to `Hold` without the script supplying a position of its own. The same rule covers every
position `Sandbox` reports (§7.5). Direction and field of view are not positions and are
always reported. A pose has a **fixed shape** of 9 fields for costing (EA1a §2.5), so a
returned pose is charged `e(18)` whether or not its position fields are present. (This is
this document's own rule, which TM §0.1 rule 2 allows as stricter than TM; with it the
dialog texts of `camera` and `session.sandbox` in EM §6.1 stay accurate as they stand, and
EM §6.1 and §6.2 need no change.)

**The free-fly eye (EA1a OPEN-7).** In free fly without a character (EA1a §10.2) the eye
of the user's view is the free-fly viewpoint, and `GetPose` and `GetUserPose` report it
under `world.read` like any other eye. In that mode the viewpoint is where the user is, so
it is exactly what `world.read`'s dialog text ("Read where you are") grants, and what
`camera` alone never reveals. This document thereby settles EA1a OPEN-7 in favour of
reporting: `World.Here` may report the free-fly viewpoint (`mode = "fly"`) under
`world.read`, since `Camera` reports the same point under the same grant; a script
without `world.read` learns it from neither module (§14).

<a id="s4-3"></a>
### 4.3 Module functions

| Function | Arguments | Results | Apply | Cost | Since |
|---|---|---|---|---|---|
| `Camera.GetPose()` | — | pose: the view of this frame's step 3 (*V* of §4.6), the one the renderer presents; its position fields only with `world.read` (§4.2) | read | `B` + `e(18)` | 1.0 |
| `Camera.GetUserPose()` | — | pose: the view the user's own camera controller produced in this frame's step 3 (*V*ᵤ), with or without a lease; `leased` is `false`; its position fields only with `world.read` (§4.2) | read | `B` + `e(18)` | 1.0 |
| `Camera.Take()` | — | lease \| `nil`, `"taken"` \| `nil`, `"unfocused"` | at once (the lease exists at once; its view applies from step 3 next) | `B` | 1.0 |
| `Camera.On(name, fn)` | name, function or `nil` | — | at once | `B` | 1.0 |

**`Camera.Take` in detail.**
- One camera lease exists at a time **for all scripts** (TM L82). While another script's
  lease is active, `Take` answers `nil, "taken"`. This tells the asking script one bit
  (some script holds the camera), the same kind of shared-pool signal that D17 §5.2 "What
  remains" records for the global caps.
- If the calling script already holds the active camera lease, `Take` renews it and returns
  **the same handle**.
- While the game window does not have keyboard focus, `Take` answers `nil, "unfocused"`
  (D17 §5.4: no lease runs while the panic chord cannot reach the window).
- While the calling apartment is **unloading** (from the frame its unload is requested,
  §4.5, its `Unload` handler included), `Take` answers `nil, "taken"` and creates nothing
  (TM §8.17: a lease request from an `Unload` handler is refused as if taken). So the old
  apartment of a hot reload never holds the global lease slot (TM L82) against the new
  one.
- The order of the answers is: `"taken"` for an unloading apartment, then the renewal of
  the caller's own active lease, then `"taken"` for another script's lease, then
  `"unfocused"`.
- A new lease is active, has **no mode** (so the view stays the user's until a mode intent
  applies, §4.6) and expires 350 ms after the call (TM L63, §4.5).

<a id="s4-4"></a>
### 4.4 Lease methods

A lease is a host userdata with a sealed metatable (EA1a §2.7). A lease handle never becomes
stale while its apartment lives: an ended lease is an ordinary handle in the state *ended*,
so ending under starvation never turns a script's renewal into an aborting error.

| Method | Arguments | Results | Apply | Cost | Since |
|---|---|---|---|---|---|
| `lease:Renew()` | — | `true` \| `nil`, `"ended"` | at once (sets the expiry) | `B` | 1.0 |
| `lease:Release()` | — | `true` (also on an ended lease) | at once; the user's view returns at step 3 next | `B` | 1.0 |
| `lease:IsActive()` | — | boolean | read | `B` | 1.0 |
| `lease:EndReason()` | — | `nil` while active, else a reason of §4.5 | read | `B` | 1.0 |
| `lease:ExpiresIn()` | — | seconds until expiry, ≥ 0; 0 when ended | read | `B` | 1.0 |
| `lease:Exists()` | — | `true` (the uniform handle probe of D17 §3 "API rules") | read | `B` | 1.0 |
| `lease:Kind()` | — | `"CameraLease"` (EA1a §2.7) | read | `B` | 1.0 |
| `lease:Follow(target [, opts])` | `"player"` or an `Entity` handle (a union, §10.2); options table | `true` \| `nil`, `"ended"` | step 3 next | `B` + Rsv `e(12)` | 1.0 |
| `lease:Hold(pose)` | pose (§4.2) | `true` \| `nil`, `"ended"` | step 3 next | `B` + Rsv `e(20)` | 1.0 |
| `lease:Play(keyframes [, opts])` | array of keyframes (§4.8); options table | `true` \| `nil`, `"ended"` | step 3 next | `B` + Rsv `e(6)` + Rsv `e(24 · 256)` | 1.0 |
| `lease:SetFov(fov)` | `fov`: a **required** parameter of type number (radians, in the §3.1 range) or `nil` (the user's field of view again); `SetFov()` with no argument raises (EA1a §2.4 step 2, a missing required argument), `SetFov(nil)` does not | `true` \| `nil`, `"ended"` | step 3 next | `B` | 1.0 |

- `Follow`, `Hold`, `Play` and `SetFov` are **intents**: each also renews the lease, as
  D17 §5.4 says of "issuing an intent through it".
- **Coalescing.** The camera intents one lease makes in one frame are coalesced: the last
  mode intent (`Follow`, `Hold` or `Play`) and the last `SetFov` win. Together they count as
  **one** pending intent against TM L30's 256 per script per frame, so a script that
  re-issues `Hold` every frame never reaches `"busy"` through the camera.
- **Costs.** Elements are counted by EA1a §2.5's rule (every key and every value). Each
  table argument is walked only after its reservation (§10.3), by the bounded walk of
  §10.2, which reads at most the declared fields plus one unknown field (whose name
  raises): `Follow`'s options have 5 fields, so `e(2 · 6)` = `e(12)`; a pose 9 fields,
  `e(2 · 10)` = `e(20)`; `Play`'s options 2 fields, `e(2 · 3)` = `e(6)`. For the key
  frames `Play` reserves, per array entry, its index and its table (2) plus at most 11
  fields of the key frame (its 10 accepted fields, §4.8, plus one unknown), 2 · 11 = 22,
  so `e(24 · 256)` = `e(6,144)` = 122,880 units for the 256-entry cap. Every reservation
  is refunded down to what the walk read (EA1a §2.5).
- **`Follow` with an `Entity` handle needs `entity.read`** (TM §8.23 "Handles carry their
  module's capability"; D17 §5.2; EA1a §2.7). `entity.read`, the capability of the module
  that minted the handle, must be in the apartment's **effective set** at the call, or the
  call raises `"not granted"`; only then must the handle be a live handle of the script's
  own apartment, and a stale handle raises as every stale handle does (EA1a §2.7). This is
  EA1a §2.4's step 2 for a handle argument, applied to the `Entity` alternative of
  `Follow`'s union argument even though `Follow` is a `Camera` native: the union selects
  that alternative by the value's discriminator (a host handle of kind `Entity`) and then
  runs its checks in full (§10.2, §10.3), so a revoked minting capability is never
  reported as "matches no alternative". The grant dialog
  warns about `camera` + `entity.read` for this reason. A kept entity handle therefore
  cannot start a `Follow` after an optional `entity.read` grant was revoked and the script
  resumed (EA1a §2.7: the handle stays a valid object, but every native that takes it
  raises; EA1a §2.9 for the revocation). Step 3 checks the same condition again at every apply (§4.7).
- **While unloading** (§4.5) the lease has already ended, so every intent answers
  `nil, "ended"` and queues nothing.

<a id="s4-5"></a>
### 4.5 Lifetime and end reasons

**The lease clock.** In the client, "now" is the host's monotonic clock read at the moment
of the call (for `Take`, `Renew` and intents) or at the start of step 3 (for the expiry
check). In deterministic mode (`scripttool test`) it is the harness's simulated frame time,
so expiry repeats exactly (D17 §8 "Deterministic mode").

**Expiry.** `Take`, `Renew` and every intent set the expiry to now + **350 ms** (TM L63).
At the **start of every step 3**, before any controller steps (D17 §5.4: "at the start of
the step that applies the lease's intents"), the client ends a lease whose expiry has
passed. TM §7.4 rule 7.4.3: expiry is native and runs in every frame, gated or not.

A lease ends at the first of these events, which are TM §8.17's, with the reason
`lease:EndReason()` then returns. "That frame" is the frame of the event; when the event
comes after step 3 of its frame, the view returns at step 3 of the next frame.

| Reason | Cause | The view returns to the user | Notification |
|---|---|---|---|
| `"expired"` | No renewal or intent for 350 ms (a stalled, looping, throttled or starved script; D17 §5.4) | at the step 3 that finds it expired | `LeaseEnded` |
| `"released"` | `lease:Release()` | at step 3 of the next frame | none (the script did it) |
| `"focus"` | The game window lost keyboard focus (the user switched away; from slice 2 also a `Files.Pick` dialog opening), in the frame the client learns of it | at step 3 of that frame | `LeaseEnded` |
| `"region"` | The player's region changed (a `LocationReport` with a new region applied in step 2), because every position the lease holds is in the old region's frame | at step 3 of that frame | `LeaseEnded` |
| `"suspended"` | The script was suspended: watchdog, aborts, or the revocation of **any** of its grants, required or optional, by any route, a grant-store re-read that removes, invalidates or narrows its record included (TM §8.23) | at step 3 of that frame | none while suspended; readable after the user resumes it |
| `"panic"` | The panic chord (D17 §5.5) | at step 3 of the same frame (the chord is tested in step 1) | none while suspended; readable after resume |
| `"switch"` | A backend switch or session end is **queued** (EM §10.3 `Open` → `Draining`, TM rule 7.4.2), not the later frame of the apartment's unload item | at step 3 of that frame | none (6b does not run while `Draining`, EM §10.3, and the apartment then unloads) |
| `"unload"` | The apartment's unload is **requested** for any other cause (TM §8.17, L63), in the frame of the request, not the later frame of its unload item: the user's unload in the script manager, `Core.Unload`, developer mode turned off (EM §9.8 unloads every developer script), a full reload (§8.1), the client's exit; for the old apartment of a hot reload, the start of item 4 (§8.3), the moment the sequence commits to replacing it (until then a failed reload keeps it running, §8.7) | at step 3 of that frame (of the next frame when the request comes after step 3, for example `Core.Unload` in 6b) | none (the apartment is unloading) |

**Unloading.** From the frame its unload is requested (the `"switch"` and `"unload"` rows)
until its close item, an apartment is **unloading**. In that frame its lease ends, its
pending camera, overlay and `Sandbox` intents are discarded and its overlay lists are
cleared (§5.2). While it is unloading, whatever call runs (a turn before its unload item,
if the cause leaves it one, and its `Unload` handler): `Camera.Take` answers
`nil, "taken"`, every lease intent answers `nil, "ended"`, every `Overlay` add answers
`nil, "unloading"`, and every `Sandbox` command answers `nil, "unloading"`; nothing is
created or queued. Any intent that is queued all the same (a defect) is discarded when the
call returns (TM §8.17, TM invariant 25), and the apply steps check it again: step 2 and
step 3 apply no intent of an unloading apartment (§7.4), so nothing a handler queues can
take effect after the user's unload, `Core.Unload` or a hot reload's item 4. Reads
(`GetPose`, `IsActive`, `GetStats`) keep working, and `Core.SetHandOff` is unaffected
(§8.4).

After any end, the script may call `Camera.Take` again in a later call (while it is not
unloading); nothing carries over from the ended lease. After a revoked optional `camera`
grant and a resume, `Camera.Take` and every lease method raise `"not granted"` (EA1a
§2.9, TM §8.23).

<a id="s4-6"></a>
### 4.6 How step 3 applies a lease

Step 3 of every frame:

1. **Expiry** (at the start, §4.5). An active lease whose expiry has passed ends with
   `"expired"`.
2. **The user's view.** The user's camera controller runs exactly as without a lease: it
   receives the user's look, zoom and reset input and produces the user's view *V*ᵤ
   (eye, yaw, pitch, field of view). `Camera.GetUserPose` reports *V*ᵤ. Movement keys
   move the character relative to *V*ᵤ, never relative to the leased view, so W keeps
   walking where the user's camera faces.
3. **The leased view.** If an active lease has a mode, the view *V* is computed from the
   mode (§4.7–§4.8); otherwise *V* = *V*ᵤ.
4. **Distance clamp.** If the eye of *V* lies more than **500 m** (TM L82; 3D distance) from
   the **reference point** *P*, it is moved along the line from *P* to the eye to 500 m
   from *P*; yaw and pitch stay. (The same 500 m as the `World` ray limit, TM L69.) *P* is
   the player character's interpolated position of this frame when a character is placed;
   when none is (before placement, or in free fly without a character, which EA1a §10.2
   allows), *P* is the eye of the user's own view *V*ᵤ of this frame. Either way *P* is
   where the user's own controls put the view, so the clamp keeps a leased eye near what
   the client streams (item 7).
5. **Field of view.** *V* uses the lease's field of view when `SetFov` (or a pose's `fov`,
   or a path's `fov`) set one, otherwise the user's.
6. **Who uses *V*.** The renderer's view and projection in step 10, the projection of ext
   world labels and of overlays (step 8, step 10), `Camera.GetPose`, and the audio
   listener in step 9.
7. **Who keeps *V*ᵤ.** Character movement (item 2); the streamer's focus, which the client
   computes from the player's controls and never from the rendered camera (§2 provenance;
   the 500 m clamp keeps *V* near that focus); and the terrain and object level-of-detail
   selection (`LodSelector::from_fov`), which uses the **user's** field of view, so a
   script cannot change the client's detail work from frame to frame. A leased field of
   view narrower than the user's therefore shows distant terrain at the user's detail; this
   is accepted.
8. **Pointer picking** (TM §8.17). Every pick the user's own pointer makes (clicks,
   selection, hover, IB's pointer actions) uses the view and projection of the frame being
   presented, the one the user sees, so a click selects what is under the pointer on
   screen. Native UI anchored in the world follows the same presented view.
9. **No collision.** A leased eye is used as given (after the clamp): it may sit inside
   terrain or geometry. This is a view, not a body.

When a lease ends, the next step 3 uses *V*ᵤ at once, with no blend: control returns in one
frame (D17 §5.4).

<a id="s4-7"></a>
### 4.7 Modes

**Follow** — `lease:Follow(target, opts)`. Let *T* be the target's interpolated position
(the player's, or the entity's render transform of this frame) and ψₜ its facing as a yaw
(EA1a §2.10 "Facing of an object": `World.Here().yaw` for the player, EA1a §10.1 `yaw` for
an entity). With the options below, the look-at point is *L* = *T* + (0, 0, `height`), the
view's yaw is ψ = (`relative` ? ψₜ : 0) + `yaw` (wrapped into (−π, π]), its pitch
θ = `pitch`, and the eye is *L* − `distance` · d(ψ, θ) (§3.1). So with the defaults the
camera sits behind the target, looking the way the target faces, slightly down.

| Option | Type and range | Default |
|---|---|---|
| `distance` | number, [0.5, 50] m | 6 |
| `height` | number, [−10, 10] m | 1.8 |
| `yaw` | finite number, radians, counter-clockwise (§3.1): an offset added to the target's yaw when `relative`, the view's yaw itself otherwise | 0 |
| `pitch` | number, radians, §3.1 range | −0.35 (about −20°) |
| `relative` | boolean | `true` |

The follow is rigid (no smoothing). If the target entity despawns or its handle becomes
stale while the lease follows it, the lease switches to **Hold** with the last pose applied,
and the script gets `FollowLost` (§4.10).

**`Follow("player")` without a character.** While no character is placed (before
placement, or in free fly without a character, EA1a §10.2: `Entity.Self()` answers
`nil`), a `"player"` follow has no target: step 3 uses *V*ᵤ as *V*, exactly as for a lease
with no mode, and the lease stays in its `Follow` mode. From the first step 3 with a placed
character it follows that character. Following `"player"` never ends and never produces
`FollowLost`.

**`entity.read` at every apply.** An entity follow is accepted only with `entity.read` in
the effective set (§4.4), and step 3 checks it again every frame before it reads the
target's transform (TM §8.23: an intent that holds a handle is re-checked when it is
applied). A revoked `entity.read` grant suspends the script in that frame (TM §8.23), which
ends the lease with `"suspended"`; a resume never restarts the follow (a resumed script
has no lease, and its kept entity handles raise `"not granted"` in every native that takes
them, EA1a §2.7, §4.4). So no `Follow` made under `entity.read` survives its revocation
(TM §8.23). Should step 3 ever find the capability missing while an entity follow is the
lease's mode (a defect, since the revocation suspended the script first), it applies the
last pose as a **Hold** without reading the target, and logs the event.

**Hold** — `lease:Hold(pose)`. The view is the pose as given (its `fov`, when present, acts
as `SetFov`).

**Play** — `lease:Play(keyframes, opts)`. A keyframed path the client interpolates
(§4.8).

A new mode intent replaces the previous mode at the step 3 where it applies.

<a id="s4-8"></a>
### 4.8 Paths

**Keyframes.** An array (raw indices 1…*n*, 2 ≤ *n* ≤ **256**, TM L82) of tables with the
fields `t` (seconds), `x`, `y`, `z`, `yaw`, `pitch` and optional `fov`, each angle in
radians with the §3.1 ranges. The fields `heading`, `region` and `leased` are ignored,
exactly as in a passed pose (§4.2), so a pose that `GetPose` returned plus a `t` is a key
frame; any other field name is a Lua error. The first `t` is 0, each next `t` is strictly
greater, the last is at most **300 s** (TM L82). A key frame with `fov` anywhere means
every key frame must have it. The array's raw length is read before any element (2 ≤ *n*
≤ 256, else a Lua error before the reservation); the elements are walked only after `Play`'s
reservation (§4.4, §10.3). Any violation raises a Lua error naming the key frame's index.

**Options.**

| Option | Values | Default |
|---|---|---|
| `interpolation` | `"linear"` or `"smooth"` | `"smooth"` |
| `loop` | boolean | `false` |

**Evaluation at playback time τ.** τ is the lease clock (§4.5) minus the time of the step 3
that first applied this `Play`.

1. **Period.** Without `loop`, τ is used as it is. With `loop`, the period is the last key
   frame's time tₙ and τ is taken modulo tₙ, so key frame *n* and key frame 1 are the same
   instant of consecutive periods: the view jumps from key frame *n* to key frame 1 at the
   wrap. A script that wants a seamless loop makes its last key frame equal to its first.
2. **Segment.** Find the segment *i* (1 ≤ *i* < *n*) with tᵢ ≤ τ < tᵢ₊₁ and
   u = (τ − tᵢ) / (tᵢ₊₁ − tᵢ).
3. **Neighbours** (for `"smooth"`). Without `loop`: p₀ := p₁ and pₙ₊₁ := pₙ. With `loop`,
   cyclically over the period, skipping the shared instant: p₀ := pₙ₋₁ and pₙ₊₁ := p₂ (for
   *n* = 2 that gives p₀ := p₁ and p₃ := p₂).
4. **Yaw unwrapping.** Before evaluation the host replaces each yaw from the second key
   frame on by the value that differs from the previous key frame's (already unwrapped)
   yaw by an amount in (−π, π] (the yaw plus a whole multiple of 2π), so every segment
   turns along the shorter arc. A cyclic neighbour is unwrapped against the key frame it
   stands beside: p₀ against p₁, and pₙ₊₁ against pₙ.
5. **Interpolation.** `"linear"`: every field is pᵢ + u · (pᵢ₊₁ − pᵢ). `"smooth"`: every
   field follows the uniform Catmull-Rom spline through pᵢ₋₁, pᵢ, pᵢ₊₁, pᵢ₊₂ at parameter
   u. The yaw is then wrapped into (−π, π], and the pitch and the field of view are
   clamped to their §3.1 ranges.
6. **End.** Past the last key frame without `loop`, the view holds the last key frame, and
   the script gets `PathFinished` once.

A path does **not** keep its lease alive: the script still renews every 350 ms (D17 §5.4).

<a id="s4-9"></a>
### 4.9 The lease indicator

The reserved layer (D17 §5.3) shows one line per lease at the top centre of the screen in
every frame in which that lease is **active** (from `Take` to its end) and in every frame
whose presented view that lease decided; it disappears with the first presented frame whose
step 3 returned the view to the user after the end:

- Tessera's own text, for example "Camera controlled by ⟨name⟩ (⟨id⟩ · ⟨kind⟩) — ⟨chord⟩
  stops all scripts", where ⟨name⟩ is the manifest's `name` drawn as plain text by TM §8.2
  (with the `name` cap of TM L66), ⟨id⟩ · ⟨kind⟩ is the fixed-style id and folder kind
  ("installed", "developer"), and ⟨chord⟩ is the panic chord's current binding as Tessera's
  settings name it (TM L64).
- It is **hit- and hover-transparent** and takes no activation or focus (D17 §5.3); a
  script cannot style, move or hide it.
- It stays drawn while a consent prompt shows.

<a id="s4-10"></a>
### 4.10 Notifications

| Name | Fields | When |
|---|---|---|
| `LeaseEnded` | `reason` (`"expired"`, `"focus"` or `"region"`) | the lease ended for that reason (§4.5) |
| `PathFinished` | — | a `Play` without `loop` reached its last key frame |
| `FollowLost` | — | the followed entity despawned or its handle became stale (§4.7); never for `"player"` |

**What the end reasons tell a `camera`-only script.** From `LeaseEnded{reason = "region"}`
or `"focus"` a script that holds `camera` and not `world.read` learns that the player's
region changed or that the game window lost keyboard focus, at frame granularity, at most
once per lease and only while it holds one; it learns no region number, position or other
application. This is a stated residual, TM §6.8 item (6), accepted there: each is one
event per lease carrying no number, the lease ends at these events so that it fails safe
(TM §8.17, D17 §5.4), and hiding the reason would not hide the end, which a renewing
script observes as its lease ending early.

<a id="s4-11"></a>
### 4.11 Caps and costs

| Resource | Cap | When exceeded |
|---|---|---|
| Camera leases | 1 for all scripts (TM L82) | `nil, "taken"` |
| Camera intents | coalesced per lease per frame; count as 1 pending intent (TM L30) | — |
| Path length | 256 key frames, 300 s (TM L82, L34) | Lua error |
| Leased eye | 500 m from *P*, the player or, with no placed character, the user's eye (TM L82, which states the same *P*; §4.6) | eye clamped |
| Lease memory | the lease's declared size plus its current path, charged to the script's cap (TM L23) | allocation refused (EA1a §2.4) |

---

<a id="s5"></a>
## 5. `Overlay`

<a id="s5-1"></a>
### 5.1 Module card

| Field | Value |
|---|---|
| Name and version | `Overlay`, major 1, minor 0 |
| Capability | `overlay` |
| Group | **O** |
| Backends | `Offline`; `TesseraProtocol` from slice 3, subject to the server's policy (D17 §6.5) |
| Apply | additions at once (to the draft list); drawn in step 10 |
| Slice | 1b |
| Grant warnings | `entity.read` + `overlay` (D17 §5.2) |

<a id="s5-2"></a>
### 5.2 Draft list and shown list

Each apartment has two primitive lists:

- the **draft list**, which every add call appends to;
- the **shown list**, which the client draws.

**Swap points.** The draft list **becomes** the shown list, and a new, empty draft list
starts, at these points and only there:
- the normal return of the top-level run;
- the normal return of the `"Tick"` handler (EA1a §4.1, set with `Core.On("Tick", fn)`);
- for a script with **no** `Tick` handler, the end of each completed turn (TM §8.13: a turn
  split by the 2 ms cap completes in a later frame) in which at least one call ran, when the
  turn's last call returned normally.

So the overlay is rebuilt every tick (D17 §3): what the script added since its last swap is
what is drawn. A swap of an empty draft makes the overlay empty.

**No swap.** If the call at a swap point aborts or raises (including the last call of a
turn of a script without `Tick`), the draft list is discarded and the shown list stays. A
frame in which the script reaches no swap point (throttled, not reached under the 2 ms cap,
skipped by the heavy-frame gate) draws the shown list of its last swap, so overlays never
flicker (D17 §3). Additions made in calls of a turn that ends before its swap point (a split
turn) stay in the draft list for the next swap.

**Cleared.** The shown and draft lists are emptied:
- at suspension (any cause, TM §8.23) and at the panic chord, in the same frame (D17 §5.5,
  §8 "Suspension");
- in the frame the apartment's unload is requested (§4.5 "Unloading": the user's unload,
  `Core.Unload`, developer mode turned off, a full reload, the client's exit), and in the
  frame a backend switch or session end is queued (6b does not run while `Draining`, EM
  §10.3); for the old apartment of a hot reload at the start of item 4 (§8.3). An
  unloading apartment adds nothing afterwards: its adds answer `nil, "unloading"`.

**Not drawn.** The shown list is kept but not drawn:
- while a consent prompt shows (D17 §4);
- for its world primitives (lines, boxes), after a region change, until the next swap,
  because their positions are in the old region's frame. Screen quads are still drawn.

<a id="s5-3"></a>
### 5.3 Functions

| Function | Arguments | Results | Apply | Cost | Since |
|---|---|---|---|---|---|
| `Overlay.Line(x1, y1, z1, x2, y2, z2, colour [, style])` | two world points, colour, style | `true` \| `nil`, `"busy"` \| `nil`, `"unloading"` | at once (draft) | `B` | 1.0 |
| `Overlay.Lines(coords, colour [, style])` | flat array of *m* = 6·*n* numbers (1 ≤ *n* ≤ **2,048**, TM L90, so *m* ≤ 12,288): x1, y1, z1, x2, y2, z2 of each line, in raw indices 1…*m*; colour; style | `true` \| `nil`, `"busy"` \| `nil`, `"unloading"` | at once (draft) | `B` + `e(2m)`, *m* the actual length (EA1a §2.5: an array of *m* entries is 2*m* elements; at most 491,720 units) | 1.0 |
| `Overlay.Box(minx, miny, minz, maxx, maxy, maxz, colour [, style [, filled]])` | an axis-aligned box in world coordinates; colour; style; `filled` boolean (default `false`) | `true` \| `nil`, `"busy"` \| `nil`, `"unloading"` | at once (draft) | `B` | 1.0 |
| `Overlay.Quad(x, y, w, h, colour)` | a screen rectangle in UI units; colour | `true` \| `nil`, `"busy"` \| `nil`, `"unloading"` | at once (draft) | `B` | 1.0 |
| `Overlay.Clear()` | — | — (empties the draft list) | at once | `B` | 1.0 |
| `Overlay.Count()` | — | number of primitives in the draft list | read | `B` | 1.0 |
| `Overlay.GetStats()` | — | table of the last encoded frame (below); its view-dependent fields only with `world.read` | read | `B` + `e(14)` (a fixed shape of 7 fields, EA1a §2.5) | 1.0 |
| `Overlay.Rgba(r, g, b [, a])` | numbers in [0, 1]; `a` defaults to 1 | colour string `#RRGGBBAA` (§3.1): each channel is ⌊255 · *c* + 0.5⌋, in upper-case hexadecimal digits | read | `B` + `b(9)` | 1.0 |

**Argument rules.**
- `style` is `"depth"` (the default for lines and boxes: depth-tested against the world) or
  `"top"` (drawn after the world with no depth test). Screen quads are always on top.
- A box needs `min ≤ max` on every axis and an extent of at most **2,000 m** per axis (TM
  L83).
- A quad needs `w` and `h` in [0, 16,384] and `x`, `y` in [−16,384, 16,384] (argument
  domains, §3.1; a quad's cost is bounded by its coverage count, TM rule 7.1.2, not by
  these ranges).
- **`Lines` in detail.** Its length *m* is the array's raw length (Lua 5.1's `#` without
  metamethods), read **before any element**. In §10.3's order: a length that is 0, not a
  multiple of 6 or over 12,288 (2,048 lines, TM L90) raises a Lua error before any element
  is read (step 3); a call that would take the draft past TM L53's 10,000 primitives
  answers `nil, "busy"` (step 4); then the cost `B` + `e(2m)` is charged (step 5). At its
  largest that is 200 + 20 · 2 · 12,288 = 491,720 units, under two thirds of one call's
  800,000 (TM L4) and about 2.5 ms at 5 ns per unit, inside the 4 ms deadline (TM L3), so
  a `Lines` call of any allowed length can succeed in a call that has that much left
  (EA1a §2.5). As §10.3 requires of every walk, the elements are read only now (step 6),
  after the charge, by raw access into a host buffer; an element that is not a finite
  number with \|v\| ≤ 10⁷ raises a Lua error naming its index, and nothing is added (the
  charge stays, as for any error after step 5). Only a wholly valid array is appended. A
  script that wants more than 2,048 lines in one draft makes several `Lines` calls, across
  the calls of one turn (below).
- A primitive whose colour has alpha `00` is accepted and counted toward the primitive cap,
  but it is never drawn and counts no coverage (TM rule 7.1.2).

TM L90 (revision 5) states the same cost, `B` + e(2*m*), at most 491,720 units, under
EA1a §2.5's counting rule; revision 4's figure of 245,960 counted one element per number
(the OPEN of revision 4 is closed, §14).

**Never a silent truncation (D17 §3 "API rules").** An add that would take the draft list
past **10,000** primitives (TM L53) adds **nothing** and answers `nil, "busy"`; for `Lines`
that holds for the whole array, so a call either adds all its lines or none.

**Building a large draft takes several calls.** Each single add costs at least `B`
(200 units), so one call, with its 800,000 units (TM L4), makes at most 4,000 adds, fewer
after the interpreter's own units for the loop; `Lines` adds at most 2,048 lines per
native call (TM L90), and one script call has the units for one such maximal `Lines`.
A draft of 10,000 primitives is therefore built over several calls of one turn, before
its swap point (§5.2): for example several timer calls that run before the turn's `Tick`,
or, for a script without a `Tick` handler, all the calls of the turn. The 2 ms cap may
split such a turn across frames (TM L5, TM §8.13); the additions wait in the draft list, and
the shown list stays drawn, until the swap.

**`GetStats` fields** (all of the last frame in which step 10 encoded this script's shown
list; zeros before the first). These are TM rule 7.1.4's per-frame overlay statistics.
The fields that depend on the view (which primitives lie in the frustum and how large they
project) are present **only when `world.read` is in the apartment's effective set** at the
call; without it they are absent. A script with `overlay` alone could otherwise compare
them over frames and find the camera's position and facing, and so the player's location
(TM rule 7.1.4, TM §6.8). The other fields depend only on the script's own list and on
*n*, the number of overlay scripts, which is a stated residual (TM §6.8 item 5).

| Field | Meaning | Present |
|---|---|---|
| `listed` | primitives in the shown list | always |
| `dropped_count` | primitives dropped by the count share (TM rule 7.1.4 step 1) | always |
| `coverage_share` | this script's coverage share *A* in that frame, in screens | always |
| `drawn` | primitives drawn | with `world.read` |
| `culled` | primitives counted 0 by TM rule 7.1.2 (outside the view frustum, alpha `00`, or a filled box with the eye inside it, §5.4), and world primitives not drawn after a region change (§5.2): skipped, neither drawn nor dropped | with `world.read` |
| `dropped_coverage` | primitives dropped by the coverage share (TM rule 7.1.4 step 2) | with `world.read` |
| `coverage` | coverage of the drawn primitives, in screens (TM §2 "Coverage") | with `world.read` |

<a id="s5-4"></a>
### 5.4 Drawing (step 10)

- Shown lists are frozen at the end of 6b; step 10 of the same frame encodes them, after
  the camera of this frame's step 3 is known.
- World positions are converted to the renderer's camera-anchored `f32` space (ARCH §9.1)
  by the client.
- **Lines** are one physical pixel wide. **Outline boxes** are their 12 edges as lines.
  **Filled boxes** are the six faces, drawn with **back faces culled**, so a filled box is
  seen from outside only: with the eye inside it, it draws nothing. TM rule 7.1.2 counts a
  filled box with the eye inside as 0 only under this culling.
- **Colour.** The client decodes each 8-bit sRGB channel with the sRGB transfer function
  (IEC 61966-2-1) to linear, keeps alpha as given (straight), and passes the result as the
  pass's linear RGBA. Blending is source-over with straight alpha in linear space.
- **Order:** "depth" primitives are drawn with the world's depth at the end of the world
  passes, "top" primitives and screen quads right after the world passes; both lie under
  the walker's overlays and under the whole UI pass (UR as amended by D17). Within one
  script's list, primitives are drawn in list order; scripts are drawn in the order of
  their load sequence numbers.
- **Culling** and coverage are TM rule 7.1.2's (§6).
- Step 10's encoding of ext overlays is native work: it runs no Lua, is measured apart
  from U1, and has the budget row of TM rule 7.1.6 (§6.5).

**What `crates/render` must add.** A pass today offers `line`, an outline `aabb` and screen
`quad` (§2). Slice 1b adds: (1) a filled axis-aligned box in a world pass, drawn with back
faces culled; (2) a world-pass style without depth test and without depth writes, drawn
after the depth-tested world primitives ("top"). Depth-tested overlay primitives do not
write depth either, so overlays never hide each other through the depth buffer.

<a id="s5-5"></a>
### 5.5 Caps and costs

| Resource | Cap | When exceeded |
|---|---|---|
| Draft list | 10,000 primitives per script (TM L53) | the add answers `nil, "busy"` and adds nothing |
| One `Lines` call | 2,048 lines (12,288 numbers), so its cost fits TM L3 and L4 (TM L90) | Lua error before any element is read |
| An unloading apartment | no add (§4.5) | `nil, "unloading"`, nothing added |
| Encoded per frame | the count share of TM L53 | the newest of a script's list beyond its share are dropped |
| Coverage per frame | the coverage share of TM L54 | that primitive is dropped and the visit goes on |
| Memory | each primitive in either list is charged its declared size (E5) to the script's cap (TM L23) | allocation refused (EA1a §2.4) |

---

<a id="s6"></a>
## 6. GPU coverage of overlays

D17's open item 1 found that the primitive caps bound the **CPU** cost of overlays but not
their **GPU** cost. TM §7.1 closes it for overlays and for ext UI composition; its rules
7.1.1–7.1.6 are normative and are not restated here. This section says only how they apply
to the `Overlay` module and how a script observes them.

<a id="s6-1"></a>
### 6.1 Terms

**Screen** and **coverage** are TM §2's. In deterministic mode and in CI the screen is the
harness's virtual surface (1920 × 1080 unless the walk file's `[screen]` table sets
another, EM §11.5, within TM L86's ranges; TM §2), so every count repeats.

<a id="s6-2"></a>
### 6.2 Coverage of each primitive

Each `Overlay` primitive is counted by the row of TM rule 7.1.2 that names it, in physical
pixels, before depth testing, divided by one screen:

| `Overlay` primitive | TM rule 7.1.2 row |
|---|---|
| `Quad` (its rectangle in UI units times the UI scale) | Screen quad, counted outward |
| `Line`, each line of `Lines` | Line, width *w* = 1 |
| `Box`, not filled | World box, drawn as edges |
| `Box`, filled | World box, drawn filled (eye inside, counted 0 and reported as `culled`; eye outside with all corners in front; some corner on or behind the near plane) |
| Any primitive with alpha `00`, any primitive outside the view frustum, a filled `Box` with the eye inside it | 0, skipped (`culled`) |

<a id="s6-3"></a>
### 6.3 The bounds

The overlay bounds are TM L54 (0.5 screen per script, 1.0 screen for all ext scripts, per
frame) together with the count share of TM L53. The composition bounds (TM L55) apply to
slice 1a's `UI` module (TM rule 7.1.5, EA1a §7.10); slice 1b adds nothing to them.

<a id="s6-4"></a>
### 6.4 Counting and dropping (step 10)

Step 10 applies TM rule 7.1.4 as it stands: *n* is the number of ext scripts whose shown
list is **non-empty** in this frame, **whether or not it is drawn** (a list kept but not
drawn under §5.2's "Not drawn", wholly while a consent prompt shows or in part after a
region change, still counts). Then count share and coverage share from *n*, list order, a
primitive that does not fit dropped alone and the visit going on, never a partial
primitive. Within a list that is drawn in part, a world primitive that §5.2 keeps from
being drawn is treated like a primitive counted 0 by TM rule 7.1.2: it keeps its place
among the first *c* for the count share, is skipped, and adds nothing to Σ. A list that is
not drawn at all is not visited. Counting such lists in *n* can only make *n* larger than
the number of lists that draw, so every share, and with it every bound of TM L53 and L54,
is the same or smaller than with *n* counted over drawn lists only; nothing is ever drawn
past TM rule 7.1.4's bounds.

**What the script observes.** `Overlay.GetStats` (§5.3) reports the frame's counts, its
view-dependent fields only to a script with `world.read`. The profiler line (§9.3) shows
them all to the user, and the script log gets at most one record per 10 s (TM L91) with the
counts ("overlay coverage: *k* primitives dropped"), written in TM §8.1's format. Drops are
not aborts: they never throttle or suspend the script, because the bound already caps the
cost (TM rule 7.1.4).

<a id="s6-5"></a>
### 6.5 The budget row

The frame-budget row for ext overlays and composition is TM rule 7.1.6 (TM L56): 0.6 ms of
CPU encoding of up to 20,000 overlay primitives (E4 at twice its 10,000) and 1.0 ms of GPU
time at the coverage bounds, measured by **E9** at the reference display's **native
3840 × 2160**, the worst case because coverage is counted in screens. If E9 measures more,
the coverage bounds shrink in proportion (TM rule 7.1.6, TM OPEN O-1). How the row is
funded, and the interim ext ceilings until the owner decides, are TM OPEN O-2 and TM L12.

---

<a id="s7"></a>
## 7. `Sandbox`

<a id="s7-1"></a>
### 7.1 Module card

| Field | Value |
|---|---|
| Name and version | `Sandbox`, major 1, minor 0 |
| Capability | `session.sandbox` |
| Group | **A** |
| Backends | **`Offline` only.** On any other backend `Ext.Use("Sandbox", 1)` answers `nil, "unavailable on this backend"` (EA1a §3, D17 §3), whatever a server policy says (D17 §6.6) |
| Apply | commands and clock changes at step 2 of the next frame, before `Session::poll` |
| Slice | 1b |

<a id="s7-2"></a>
### 7.2 Destinations

`Sandbox.Teleport` takes one destination table, a table of several forms (§10.2 `forms`).
Its form is chosen by its fields: a table matches a form when it has every field the form
requires and no field the form does not list; exactly one form must match, and any other
combination raises a Lua error. The forms are disjoint by construction (`place`, `anchor`
and `ground` each belong to one form only).

| Form | Fields | `tessera-net` destination |
|---|---|---|
| Exact | `region`, `x`, `y`, `z`, `yaw` | `Destination::Location` |
| On the ground | `region`, `x`, `y`, `z`, `yaw`, `ground = true` (`z` is a fallback height; `ground` must be `true`) | `Destination::Ground` |
| Anchored (an interior position) | `region`, `x`, `y`, `z`, `yaw`, `anchor = { bx = …, by = …, cell = … }`: a **record** (§10.2) with the three named fields `bx`, `by` (integers 0–254, the landblock indices) and `cell` (an integer 0–65,535), never a three-entry array; an array such as `{ 3, 4, 256 }` raises, because its keys 1, 2, 3 are unknown fields | `Destination::Anchored` |
| Named place | `place` (a string, rules below) | `Destination::Named` |

**Validation before anything is queued**, so that every destination the script passes is
one `tessera-net` accepts (its `Destination::is_valid`, §2):

- coordinates finite with absolute value at most 10⁷; `region` an integer 1–255 (0,
  "no region", is a Lua error, §3.1; `tessera-net` itself would accept `RegionId(0)`, so
  this check is this document's own); `yaw` finite (radians, §3.1);
- **anchored form:** the anchor owns the location: with the landblock's origin at
  (160 · `bx`, 160 · `by`) in the region frame, both `x` − 160 · `bx` and `y` − 160 · `by`
  have an absolute value of at most **1,280 m** (the position lies at most 1,280 m outside
  or inside the landblock's square, which covers every interior position); the anchor's
  region is the destination's `region`;
- **named form:** the host first reads `place` by EA1a §2.3's rule, replacing each invalid
  UTF-8 sequence by U+FFFD (3 bytes each). Every check applies to that **replaced** text,
  which is exactly the text the command carries: 1 to 256 bytes (`MAX_PLACE_NAME_LEN`), at
  least one code point that is not white space (Unicode `White_Space`), and no control
  character (general category Cc). A 256-byte string with invalid bytes can grow to up to
  768 bytes when replaced; such a string raises here. When the walk meets `place`, the
  host checks its raw length first (at most 256 bytes, else a Lua error), so the
  replacement never reads more than 256 bytes, and `b(place)` is charged on those raw
  bytes.

A destination that `tessera-net` would reject as invalid is therefore a Lua error here, so
a refusal from the sandbox (§7.5) always means the sandbox chose to refuse, never that the
script passed garbage. A destination is walked after `Teleport`'s reservation and after
its soft answers (§7.3, §10.3), so a call that answers `"unloading"`, `"not in world"`,
`"busy"` or `"pending"` does not walk or check its destination.

**Yaw.** `tessera-net`'s `heading` is in radians, 0 = `+X`, counter-clockwise (§2), which
is EA1a §2.10's yaw convention. The port therefore passes the script's `yaw` through with
no conversion: it wraps it into (−π, π] and narrows it to `f32`. Reported positions give
`yaw` from the model's `heading` (widened to a number) and, for display, `heading` in
compass degrees by EA1a §2.10's formula. No compass converter of `tessera-net` is used on
this path, and a destination table never takes a `heading` field (EA1a §2.10: no argument
is a heading), so a stray `heading` field is a Lua error like any unknown field.

<a id="s7-3"></a>
### 7.3 Functions

| Function | Arguments | Results | Apply | Cost | Since |
|---|---|---|---|---|---|
| `Sandbox.Teleport(destination)` | §7.2 | `true` \| `nil`, `"unloading"` \| `nil`, `"not in world"` \| `nil`, `"busy"` \| `nil`, `"pending"` | step 2 next (`Command::Teleport`) | `B` + Rsv `e(26)` + Rsv `b(256)` | 1.0 |
| `Sandbox.QueryLocation()` | — | `true` \| `nil`, `"unloading"` \| `nil`, `"not in world"` \| `nil`, `"busy"` | step 2 next (`Command::QueryLocation`) | `B` | 1.0 |
| `Sandbox.SetTimeOfDay(t)` | finite number; wrapped into [0, 1) | `true` \| `nil`, `"unloading"` \| `nil`, `"busy"` | step 2 next (clock port, `set_time_of_day`) | `B` | 1.0 |
| `Sandbox.SetGameSeconds(g)` | finite number with \|g\| ≤ 10¹² | `true` \| `nil`, `"unloading"` \| `nil`, `"busy"` | step 2 next (clock port, `set_game_seconds`) | `B` | 1.0 |
| `Sandbox.On(name, fn)` | name, function or `nil` | — | at once | `B` | 1.0 |

`Teleport`'s reservation covers the bounded walk of §10.2, counted by EA1a §2.5's rule:
the destination's 8 possible fields plus one unknown, 2 · 9 = 18, and the anchor record's
3 named fields plus one unknown, 2 · 4 = 8 (§7.2: the walk stops at the first unknown
key, so an array-shaped anchor is read no further), so `e(26)`; and `b(256)` for `place`'s raw bytes. Both are refunded down
to what the walk read (`b(place)` for a named destination, nothing for the others).

`QueryLocation` leaves one pending-query flag per script and nothing else; §7.5 states
when it is set and cleared.

`SetTimeOfDay` keeps the game day number and moves the time of day; `SetGameSeconds` sets
the game clock's seconds. The clock is not a `Command`: the clock port of
`tessera-ext-host` applies both over `tessera-world`'s `GameClock` (D17 §3). Neither changes
the clock's rate.

**Order of the answers** (EA1a §2.4 step 4 as §10.3 applies it: after the scalar
arguments and the destination's Lua type are checked, before the destination is walked):
`"unloading"`
(§4.5), then `"not in world"` (teleport and query only: the session is not in the
`InWorld` state), then `"busy"` (the per-script rate, and for clock changes the
all-scripts rate, §7.4), then `"pending"` (teleport only: any teleport pending, the user's
or another script's ext teleport holding the global slot, §7.4). So a script over its own
rate gets `"busy"` even while a teleport is pending, and a script within its rate gets
`"pending"` whenever the global slot is held, whoever holds it. A refused call is charged
`B` only and consumes nothing else: no rate token, no slot.

<a id="s7-4"></a>
### 7.4 Rate limits and the pending-teleport rule

The limits are TM L75; this is their script-visible form.

- **The windows.** Both rates use **sliding** one-second windows over **accepted** calls,
  on the lease clock of §4.5 (the host's monotonic clock read at the call; in
  deterministic mode the harness's simulated frame time, so every call of one frame has
  the same time). A call at time *t* looks back over the half-open interval
  (*t* − 1 s, *t*]: an accepted call at *t* − 1 s exactly no longer counts. A refused call
  is not recorded and moves no window (§7.3).
- **One command per script per second**, counting all four commands together: a command
  at time *t* answers `nil, "busy"` if the same script had a command accepted in
  (*t* − 1 s, *t*]. "Script" is the (kind, id), so a hot reload does not reset the window.
- **Clock changes for all scripts together** (TM L75): a `SetTimeOfDay` or
  `SetGameSeconds` at time *t* answers `nil, "busy"` if **4** clock changes, of any
  scripts, were accepted in (*t* − 1 s, *t*]. Every interval of one second therefore
  holds at most 4 accepted clock changes, so the sky and the game time, and the plugin
  events they cause (§7.5), change at most 4 times per second whatever the number of
  scripts. Calls of one frame are counted in the order they run; five scripts that each
  call once in the same frame get four `true` and, for the call that runs fifth,
  `nil, "busy"`.
- **One outstanding ext teleport for all scripts** (TM L75). The global slot is taken when
  `Teleport` accepts a call, and it is released at the **first** of:
  1. the sandbox's **outcome arrives** in step 2 (the location report of cause
     *teleported*, or the typed outcome event of §7.5) and is matched to the ext request
     (below), whatever has become of the script meanwhile;
  2. the intent is **discarded** at apply time, or **not submitted** because another
     teleport is pending or the session has left the world (below);
  3. its **submission fails** (below);
  4. the **session generation** the request was accepted under **rises**: in the frame a
     backend switch or session end is **queued** (EM §10.3 `Open` → `Draining`, D17 §4),
     not the later frame in which it is applied. No ext intent of that generation can be
     applied after the rise (the generation check discards it, and 6b does not run while
     `Draining`), so nothing the slot guarded can happen any more. A switch to a new
     `Offline` session therefore starts with the slot free.

  So a suspended or unloaded script never holds the slot past one of these, and the slot
  is never carried into the next session.
- **Correlation** (TM L75). The client numbers the teleports it **successfully submits**
  to a `Session`, the user's and ext ones alike, 1, 2, 3, … **per `Session` connection**:
  the count restarts at 1 when `Command::Connect` is submitted (EM §10.3, at the end of
  `Draining`), not when the generation rises. It numbers the teleport outcomes that
  connection reports in the same way. Every teleport the sandbox accepts gets exactly one
  outcome, in submission order (a superseded request is answered "cancelled" before the
  request that superseded it is resolved), so outcome *k* of a connection belongs to
  request *k* of that connection. The old `Session` keeps running through `Draining`
  (up to about 6.25 s, TM L11), and a user teleport sent to it then is numbered in the old
  connection, so the numbering never shifts. The ext request is matched by (connection,
  request number) recorded at its submission; no request token in `tessera-net` is
  needed. An outcome of an earlier connection, or one that arrives after its ext
  request's slot was already released (items 2–4), releases nothing, answers nothing and
  is dropped; it is logged.
- **Never supersede.** `Teleport` answers `nil, "pending"` while any teleport is pending,
  the user's own included. At apply time (step 2) the client checks again: if a teleport of
  another origin became pending since the call, or is submitted in the same step 2, the ext
  intent is **not submitted** and its outcome is `"cancelled"`. So an ext teleport never
  supersedes another request (D17 §7 "Chat"). A user teleport issued while the ext teleport
  is pending in the sandbox does supersede it, and the sandbox then answers the ext request
  with its ordinary cancellation line and the typed outcome (D17 §7).
- **Intent checks at apply time** (D17 §4, EA1a §2.9), for every `Sandbox` intent, in this
  order: the apartment is loaded, **not unloading** (§4.5) and not suspended; the
  intent's session generation is the current one; the backend class is `Offline`; the
  grant is held. An intent that fails any of these is **discarded**: a discarded teleport
  releases the slot; no outcome is delivered (the script is suspended, unloading or gone),
  and the discard is logged. Then, for a teleport or query only, the session must be in
  the `InWorld` state; if it is not (it left the world between the call and step 2), the
  intent is not submitted: a teleport releases the slot and its outcome is
  `"cancelled"`; a query produces no `Location`.
- **Submission failure.** `Session::send` returns an error for a command it does not take
  (`NetError::InvalidState` outside `InWorld`, which the check above already excludes)
  and passes on the backend's errors (§2). A teleport whose `send` returns an error was
  never submitted: its request number is **not consumed** (the next submitted teleport
  gets that number), the slot is released, and its outcome is `"failed"`; the error is
  logged. A query whose `send` fails produces no `Location`. Clock changes are not
  `Command`s and cannot fail this way (§7.3).

<a id="s7-5"></a>
### 7.5 Outcomes

Each accepted `Teleport` gets exactly one `TeleportDone` notification, and each accepted
`QueryLocation` at most one `Location` notification, **when a handler for that
notification is set** at the moment the outcome arises (§3.3, EA1a §2.8: a module
notification is queued only then; a script that wants every outcome sets its handlers
before it calls), and unless the script is suspended or unloading before the outcome
arrives (then it is dropped and counted as EA1a §2.8 says) or the slot was already
released by the generation's rise (§7.4 item 4; the script is then unloaded by the switch
anyway):

| Notification | Fields | When |
|---|---|---|
| `TeleportDone` | `ok` (boolean); `reason` when not ok: `"refused"`, `"cancelled"` or `"failed"`; when ok: `yaw` and `heading` (§3.1), and `region`, `x`, `y`, `z` **only with `world.read`** (§4.2) | the sandbox answered the request: with a location report of cause *teleported* (ok), a refusal (`"refused"`), or a cancellation by a later request (`"cancelled"`); or the client did not submit it because another teleport was pending or the session had left the world (`"cancelled"`, §7.4); or its submission failed (`"failed"`, §7.4) |
| `Location` | `yaw` and `heading` (§3.1); `region`, `x`, `y`, `z` and `anchor = { bx, by, cell }` (or absent for a position outside an interior) **only with `world.read`** (§4.2) | the first location report of cause *requested* after the query was submitted |

**The pending query** (bounded state). The host keeps **one flag per script** ((kind, id))
for `QueryLocation`, and nothing else: no queue and no count. The flag is **set** when a
query of that script is submitted in step 2 (a query that is discarded or not submitted,
§7.4, sets nothing). It is **cleared** at the first of: (1) the first location report of
cause *requested* that arrives in a step 2 after the submission, which answers it (a
`Location` when the conditions above hold, else nothing); (2) the rise of the session
generation the query was submitted under (the frame a backend switch or session end is
queued, §7.4 item 4); (3) the frame the apartment's unload is requested (§4.5; for a hot
reload, the start of item 4, so the new apartment never receives the old one's answer). A
query submitted while the flag is already
set leaves it set, and the one report that clears it answers both with at most one
`Location`, which is why §7.5 promises at most one, not exactly one; the per-script rate of
§7.4 (one command per second) bounds how often that happens. A suspension does not clear
the flag; a report that arrives while the script is suspended clears it and is dropped as
above. So the state `QueryLocation` leaves behind is one boolean per script, at most 16
(TM L26), whatever the script calls.

`world.read` is checked against the effective set when the notification's table is built
(at delivery, EA1a §2.8). Without it the script learns the player's facing, not where the
player is; a successful teleport tells it only that the player now stands at the
destination it chose itself.

**What `tessera-net` must add.** Today the sandbox answers a refused or cancelled teleport
**only** with a system chat line (`Command::Teleport` in `crates/net/src/session.rs`), and
the client must never parse chat text to learn an outcome. Slice 1b therefore adds one
variant to the `#[non_exhaustive]` `Event` enum: a typed teleport outcome (refused or
cancelled), emitted by the sandbox **alongside** the same system line it posts today, so
what plugins observe is unchanged. Successful teleports keep their location report. This is
an additive change to `tessera-net`'s model (§14).

**What plugins observe** is exactly D17 §7's list for `Sandbox`: the world-state changes (a
teleport moves the player; a clock change moves the game time and the sky), delivered to
plugins as game-raised events in step 7 item 5 (IB D8), and the sandbox's ordinary system
lines (teleport cancelled or failed, a place description after a location query), bounded by
the rate limits above. No `Sandbox` call posts any other chat line. Plugin code that such
changes cause is subject to TM §8.20 (the inherited 0014 item).

<a id="s7-6"></a>
### 7.6 Notifications

| Name | Fields |
|---|---|
| `TeleportDone` | §7.5 |
| `Location` | §7.5 |

A teleport that changes the region also ends a camera lease with `"region"` (§4.5) and stops
drawing the world primitives of the shown overlay list until the next swap (§5.2).

---

<a id="s8"></a>
## 8. Hot reload and the hand-off table

<a id="s8-1"></a>
### 8.1 Which reloads are hot

A **hot** reload replaces a running apartment by a new one built from a new snapshot, and
may pass a **hand-off table** between them. A **full** reload is an unload, a content check
with whatever re-asks D17 §5.2 and EM §9.2 require, and a load, with **no** hand-off table.
The load of a full reload (EM §7.3, its phase A with its memory reservation included)
starts only **after the old apartment's close item has run and the old cap has left the
512 MiB sum** at the completion of its final flush (TM L24, rule 7.6.5), exactly as a hot
reload's item 6 waits for that flush before its new cap moves into the sum (§8.3). Until
then the load **waits**; it is never refused for the old copy's memory. So with the sum
full (16 scripts of 32 MiB), a full reload of one of them waits for the old final flush
and then fits, where an immediate phase A would have been refused under EM §7.3. A full
reload therefore never has two apartments of its script at once and never adds an
apartment to the 16 loaded ones. That is why TM L84 leaves full reloads uncounted and why
TM L26's 17 apartments (16 plus the one new apartment of a hot reload) still hold. TM L84
(revision 6) states the same wait, and **reads** D17 §8's and §10's "at most one reload at
a time" as the limit on hot reloads, the only reloads that hold two apartments, rather
than changing an accepted figure (TM §13, by status line).

**Validation first** (TM §8.24). Before comparing anything, the host validates the new
manifest exactly as a load would (EM §4.14, including the registry check of required module
majors and minors). A manifest a load would refuse ends the reload as a failure (§8.7): the
old apartment keeps running.

**Declarations** are the manifest fields whose change makes a reload full: TM §8.24's set,
which is `manifest`, `author`, `api`, `capabilities`, `optional_capabilities`,
`memory_mib`, `[[actions]]`, `[[contexts]]`, `modules`, `optional_modules` and `[server]`
(EM §4.2). An `author` change also empties the store before the new apartment opens it
(TM §8.24, D17 §5.2).

| Folder kind | Developer mode | Declarations changed | `/ext:reload <id>` | File watcher |
|---|---|---|---|---|
| `scripts-dev` | on | no | **hot** | **hot** |
| `scripts-dev` | on | yes | full; the grant rows of EM §9.2 (every grant asked again for a capability or `author` change, D17 §10) | full, as for the command |
| `scripts` (installed) | on | either | full; group-A grants asked again if the content hash changed (D17 §5.2) | no watcher |
| any | off | — | refused with a reserved-layer notice "Developer mode is off" (developer scripts are not loaded at all with developer mode off, EM §9.8) | no watcher |

Changes to `name`, `version`, `description`, `license`, `entry`, `autoload` and
`[[settings]]` keep a reload hot. After a `[[settings]]` change the host validates the
stored values against the new schema (EM §4.12) and hands the new apartment the result.

<a id="s8-2"></a>
### 8.2 Triggers and the watcher

- **The command.** `/ext:reload <id>` (D17 §3 `Commands`) follows EM §9.8's rules and
  notices exactly, its cases checked in the order listed; each notice is drawn in the reserved layer with the typed id as plain
  text by TM §8.2 and rate-limited (TM L67):
  - with developer mode off it is refused with the notice "Developer mode is off" (§8.1's
    last row) and queues nothing, in every gate state;
  - while the backend gate is not `Open` (EM §10.3) it queues nothing and shows "No loaded
    extended script '⟨id⟩'", whatever the id names (EM §10.5: in `Draining` no script
    takes lines, even one still loaded, and in `Off` and `Starting` none is loaded);
  - for an id that names no **loaded** script (not loaded, unloaded after a suspension,
    only listed, or unknown) it queues nothing and shows the notice "No loaded extended
    script '⟨id⟩'"; loading is the script manager's (EM §9.7), never this command's;
  - for a loaded script that is **suspended** it queues nothing and shows the notice
    "⟨id⟩ is suspended: resume it first";
  - otherwise it queues a reload request for that script.
- **The watcher** (developer mode only) watches `data/scripts-dev/`. It is a non-main thread
  or the platform's notification callback; it never reads file content (D17 §10). It
  sends **no** queue of events: it holds a fixed table of at most **16 slots** (TM L26),
  one per loaded developer script, which the main thread fills at each load and empties at
  each unload. An event under the folder of a slot sets that slot's **dirty flag** and its
  last-event time; any other event (a folder whose script is not loaded, a path outside
  the scripts' folders) is dropped in the watcher itself. So a directory that changes
  without pause costs at most 16 flags, never a growing queue, whatever its event rate.
  If the platform reports an overflow of its own event buffer, the watcher sets
  every slot's flag. This fixed table **replaces** D17 §10's "events … over a channel to
  the main thread": a design narrowing (the watcher still never reads file content, and
  its memory no longer depends on the event rate), recorded by TM in L84 and in its §13
  row "D17 §10 watcher (by status line)", which also restates ARCH §21's watcher row.
- **Quiet period.** Once per frame the main thread reads the slots. It queues a reload
  request for a slot whose flag is set once **300 ms** (TM L84) have passed since its last
  event, and clears the flag, so an editor's save of several files becomes one reload,
  and half-written files are not snapshotted in the common case. (A file that changes
  during the snapshot read refuses the snapshot anyway, D17 §2, and §8.7 handles that.)
- **Outside `Open`.** In the frame the gate leaves `Open` (a backend switch or session end
  queued, EM §10.3), every waiting reload request is dropped and every watcher flag is
  cleared. While the gate is not `Open`, the main thread still reads the slots once per
  frame but queues nothing: it clears every flag it finds set, with no notice. A change
  made then is therefore not replayed when the gate opens; the next change after that
  triggers the reload. (A reload already running when the switch is queued is cancelled,
  §8.7.)
- **The watcher follows the command's rules** (EM §9.8). A slot whose script is
  suspended when its quiet period ends queues nothing; its flag is cleared and the console
  shows "⟨id⟩ is suspended: resume it first" (rate-limited, TM L67). The user's resume does
  not replay it; the next change does.
- **One hot reload at a time** (TM L84). At most one **hot** reload runs, from its item 1
  until the reload allowance is free again (§8.3 item 6), or until it ends as a failure,
  is cancelled (§8.7), or turns full at item 2 (which releases the allowance at once). A
  **full** reload is an unload and a load, its load starting only after the old close
  item has run and the old cap has left the sum (§8.1), and is not counted: it neither waits for a hot reload nor makes one wait,
  so a full reload waiting on a grant dialog blocks no other script's reload. A reload request whose kind is not yet known (a developer script's
  request before item 2) waits for the allowance like a hot one; an installed script's
  request is full from the start (§8.1) and never waits for it.
- **Requests are coalesced per id.** While a reload of an id runs or waits, hot or full,
  further requests for it collapse into at most one pending request, which starts after
  the current one ends (a full reload ends when its load has finished, failed or been
  refused, its grant dialog answered included). Requests of other ids that wait for the
  allowance are served in arrival order. A waiting request is dropped when its script is
  suspended or unloaded before it starts (§8.7).

<a id="s8-3"></a>
### 8.3 The hot sequence

The sequence uses only D17 §4's 6a item kinds, so it adds nothing to the worst-frame bound.
Ext work obeys the 6a slice (TM L6), the 2 ms cap (TM L5), the heavy-frame gate and its
floor (TM L8, L9) throughout, so the sequence spans frames.

| # | Where | What happens | The old apartment | The new apartment |
|---|---|---|---|---|
| 1 | main thread, then worker | The new apartment's cap (the running manifest's `memory_mib`) is reserved from the **64 MiB reload allowance** (TM L24, §8.10). The folder is walked into a snapshot (D17 §2) under that reservation; the manifest is parsed first. No `Store` open is needed (same (kind, id) and, if the reload stays hot, same declarations, so the store stays open and its content unchanged); from item 2 its file index counts in full toward the new apartment's cap as well (TM rule 7.6.1, §8.8) | runs | — |
| 2 | 6a: snapshot result | The new manifest is validated (§8.1); a refused manifest ends the reload as a failure. The declarations are compared. Changed → the reservation is released, the hot sequence is abandoned and a full reload starts. Unchanged → the new apartment is created inside the reserved cap, charged from now on with the store's file index (TM rule 7.6.1) | runs | created, empty |
| 3 | 6a: one compilation item per `.lua` file | each file compiled from the snapshot; afterwards only the data files and the content hash are kept, charged to the new cap (TM §8.10) | runs, ticks as usual | compiling |
| 4 | 6a: `Unload` handler of the old apartment | The old apartment's `"Unload"` handler (EA1a §4.1, set with `Core.On("Unload", fn)`) runs with its 4 ms deadline (TM L3) and the reason `"reload"`; `Core.SetHandOff` (§8.4) serialises the table inside this call. **The start of this item is the old apartment's unload request** (§4.5): from then on the old apartment gets no further turn and is unloading, so inside its handler `Camera.Take` answers `nil, "taken"`, lease intents `nil, "ended"`, `Overlay` adds and `Sandbox` commands `nil, "unloading"`, and anything it queues is discarded when the handler returns; the new apartment's `Take` in item 5 therefore never meets the old lease. At that start its pending intents are discarded, its leases end (`"unload"`), its overlay lists are cleared, its windows and world labels are destroyed, its timers and subscriptions are dropped, its commands are unregistered and its active contexts are deactivated at the next item-7 drain (D17 §10). Its pending `Store` writes stay in the shared pending-write map, charged to the old cap (TM rule 7.6.5) | `Unload` runs, then inert | compiled |
| 5 | 6a: top-level run of the new apartment | the entry file runs with its 4 ms deadline; `Core.TakeHandOff` (§8.4) returns the table, built inside this call with its cost charged first | inert | runs; from its next turn on it is the script |
| 6 | 6a: state close of the old apartment | `lua_close` (D17 §8); the old apartment's pending entries go to its final flush. When the close item has run **and** that flush has completed, the old cap leaves the 512 MiB sum, the new cap moves from the reload allowance into it, and the allowance is free (TM rule 7.6.5, §8.10) | closed | the script |

Between items 4 and 5 the script has **no running apartment**. Notifications that arise for
it in that gap (its action starts and ends, `/ext:` lines, `Store` results) are dropped,
and the script log records their counts per kind. The actions and contexts stay registered
throughout (they belong to the (kind, id)), so the user's bindings never flicker.

<a id="s8-4"></a>
### 8.4 The hand-off table

Two functions are added to `Core` (no grant, always present):

| Function | Arguments | Results | Apply | Cost | Since |
|---|---|---|---|---|---|
| `Core.SetHandOff(value)` | a value of the bounded value model (TM L33–L35: at most 32 levels, 8,192 elements, 1 MiB serialised, no cycles) | `true` \| `false`, `"not a hot reload"` \| `nil`, `"too large"` \| `nil`, `"cyclic"` \| `nil`, `"invalid"` | at once (serialised at once) | `B`, then Rsv `V` (EA1a §2.5) | 1.1 |
| `Core.TakeHandOff()` | — | the value \| `nil` | at once (built at once) | `B` + `e(elements)` + `b(bytes)` of the stored value | 1.1 |

**`SetHandOff`.**
- It is honoured only inside the old apartment's `Unload` handler during item 4 of a hot
  reload. Anywhere else (another call, an ordinary unload, a full reload, whose `Unload`
  reason is also `"reload"`, EA1a §4.1) it answers `false, "not a hot reload"` **before**
  walking the value and charges only `B`, so a script may call it in every `Unload` handler
  without paying for a walk that nobody receives, and it tells a hot reload from a full one
  by this answer.
- The value is walked with raw access only (no metamethod runs) and serialised at once, its
  maximum cost reserved before the walk (D17 §8). A later `SetHandOff` in the same handler
  replaces the earlier one. A refusal leaves any earlier accepted value in place.
- The serialised bytes are charged **inside the new apartment's cap**, never on top of it
  and never to the 512 MiB sum (TM §8.10). A value whose bytes would not fit what is left of
  the new cap answers `nil, "too large"`; the reload continues without it.
- An accepted value **stands** even if the handler later errors or passes its deadline.

**`TakeHandOff`.**
- In the new apartment's top-level run it returns the value (built inside that metered
  call, its cost charged first) on its first call, and `nil` on every later call. Outside
  the top-level run it returns `nil`.
- The host frees the bytes at the end of item 5, whether the script took them or not.

So the hand-off table is converted only inside metered calls, as D17 §10 requires, and it
never crosses a change of grants or of `author`, because a reload with a change of
declarations is full (§8.1; D17 §5.2 rule 4; TM §8.24).

<a id="s8-5"></a>
### 8.5 What carries over

| State | Hot reload | Full reload |
|---|---|---|
| Grants | kept (developer folder, developer mode, D17 §5.2) | as D17 §5.2 and EM §9.2 say |
| `Store` values, written and pending | kept, with the read rule of §8.6 | kept (flushed at unload); emptied on an `author` change (TM §8.24) |
| Hand-off table | passed (§8.4) | none |
| Registered actions and contexts | kept, counted once (D17 §10) | unregistered and registered again |
| Active contexts | deactivated after the old `Unload`; the new apartment starts with none active (D17 §10) | — |
| The user's bindings | kept (they live in `keymap-ext.toml`, TM §8.19) | kept |
| `Commands` registrations | dropped at item 4; the new top-level run registers its own | dropped |
| Windows, world labels, overlay lists, leases, timers, subscriptions | ended at item 4 | ended at unload |
| Queued notifications, and those arising in the gap | dropped and counted in the log | dropped |
| Outstanding worker requests of the old apartment | their results are dropped on arrival. Each still counts toward the script's **one** TM L27 count of 32, which is per (kind, id) and shared by both apartments, until its result is discarded **and** any worker job it started has ended (TM L27), also after item 4 and after the old close item; so the new apartment's requests are refused with `"busy"` while the old ones fill the 32. They count toward the 256 for all scripts the same way | the same (the new load's count starts with whatever the old apartment's requests still hold) |
| Random generator, `tostring` numbering, `Ext.Load` cache | new (D17 §1.2) | new |
| `[[settings]]` values | re-validated and handed to the new apartment | read at load |
| Profiler history, abort count, throttle state | reset for the new apartment | reset |

<a id="s8-6"></a>
### 8.6 `Store` across the swap (read-your-writes)

Hot reload depends on `Store` read-your-writes (D17 open item 6). The rule is TM §7.6
(rules 7.6.1–7.6.5), carried for scripts by EA1a §5.2; it is not restated here. What it
means for a hot reload:

- Both apartments share the one pending-write map of the (kind, id) (TM rule 7.6.2).
- A `Store.Get` in the new top-level run of a key the old `Unload` handler wrote returns
  that value, from a copy taken at the `Get` (program order); a key whose pending entry is a
  deletion answers absent (TM rule 7.6.3).
- Such a read is an ordinary read in every way the script can observe: delivered in the
  script's next turn, counted against the read rate (TM L42) and as an outstanding request
  (TM L27) until delivered, with its result reserved (TM §8.14).
- The old apartment's pending entries stay charged to its cap until its final flush
  completes (TM rule 7.6.5, §8.3 item 6); the new apartment reads them without being
  charged for them.

<a id="s8-7"></a>
### 8.7 Failures and interruptions

| Event | Effect |
|---|---|
| The snapshot is refused (a folder rule broken, a file that grew during the read, D17 §2), or the new manifest fails validation (§8.1) | the reload ends; the old apartment keeps running; the reservation is released; the error goes to the script log, the console and the script manager, and a reserved-layer notice "Reload failed: ⟨id⟩ · developer" is shown (rate-limited, TM L67). A later watcher event retries |
| A compile error in item 3 | the same: the old apartment keeps running and the error is shown (D17 §10); the new apartment is closed (one close item) and the reservation is released |
| The old `Unload` handler errors or passes its deadline (item 4) | the sequence continues; any value `SetHandOff` already accepted stands; the error is reported as for any call. If the abort suspends the script (TM L18), the row "between items 4 and 5" below applies instead |
| The new top-level run errors or passes its deadline (item 5) | the new apartment is closed (a close item) after the old one's close; the script ends **unloaded**, shown as "failed" in the script manager with the error; the hand-off bytes are freed. A further `/ext:reload` or watcher event is an ordinary load with no hand-off |
| The panic chord, a watchdog stop or the revocation of an **optional** grant (TM §8.23) before item 4 | the script is suspended (D17 §5.5); the reload is **cancelled**: the new apartment's unrun items are dropped and it is closed (one close item, no `Unload`), the reservation is released when that close item has run, the old one stays loaded and suspended, and no `Unload` handler runs |
| The revocation of a **required** grant (TM §8.23), by any route, before item 4 | the script is suspended in that frame and then **unloaded**, because a revoked required capability cannot be resumed (TM §8.23, EA1a §2.9): the reload is cancelled as in the row above, and the old apartment gets its close item with no `Unload` handler (as for any suspended script); the script ends unloaded, with the revocation shown in the script manager |
| An event of either row above between items 4 and 5 | the new apartment's top-level run does not start while the script is suspended (D17 §5.5); the reload is cancelled: the new apartment is closed, and the script is unloaded (the old apartment is already inert), shown as suspended-and-unloaded in the script manager. This row also covers a **watchdog suspension caused by the old `Unload` handler itself** in item 4 (an abort that is the script's third in 10 s, TM L18): the suspension takes effect when the handler returns, after item 4, so the row "The old `Unload` handler errors or passes its deadline" does not continue the sequence then; this row ends it |
| A backend switch or session end queued during a reload | as EM §10.4 and TM §8.21: the gate enters `Draining`; the reload is cancelled; the new apartment's unrun items are dropped, its snapshot result (if not yet delivered) is discarded, and its state, if created, gets one close item with no `Unload` (it counts among the at most 17 apartments, TM L26, from its reservation); the old apartment unloads as a loaded script (its `Unload` handler and close item) unless item 4 has already run, in which case it gets only its close item. The switch timeout of TM rule 7.4.2 (TM L10) applies to the old apartment's `Unload` like any other; the hand-off bytes are freed. Every waiting reload request is dropped and every watcher flag cleared in the frame of the switch, and none is queued until the gate is `Open` again (§8.2) |
| A second reload request for the same id | coalesced (§8.2) |
| A waiting reload request (§8.2) whose script is suspended or unloaded before the request starts | the request is dropped; for a suspension the console shows "⟨id⟩ is suspended: resume it first" (rate-limited, TM L67) |
| During items 1–3: the old apartment's unload is requested by the user (script manager) or by its own `Core.Unload` | the reload is **cancelled**: the new apartment's unrun items are dropped, its snapshot result (if not yet delivered) is discarded, and its state, if created, gets one close item with no `Unload`; the reservation is released when that close item has run (or at once if no state exists). The old apartment unloads as requested: its leases, intents and overlay lists end in the frame of the request (§4.5), then its ordinary `Unload` handler (not a hot reload: `SetHandOff` answers `false, "not a hot reload"`) and close item |
| Between items 4 and 5: the user unloads the script | the new apartment's top-level run does not start; the new apartment is closed (one close item); the script ends unloaded (the old apartment is already inert and gets its close item, item 6); the hand-off bytes are freed |
| Developer mode is turned off during a reload (EM §9.8 then unloads every developer script) | for a hot reload, as the two rows above, by the item the reload has reached; for a full reload whose load is in progress or waiting (§8.1), that load is cancelled or refused as EM §9.8 does for every developer load (a showing dialog closes as "Not now", a waiting load is refused with "Developer mode is off"; TM §8.23, X90); every waiting reload request is dropped and the watcher stops (§8.2) |
| The client exits during a reload | as developer mode turned off: no further item of the new apartment runs code; the new apartment is closed with no `Unload`; the old apartment gets the exit's unload (its `Unload` handler only if item 4 has not run) |
| The new top-level run calls `Core.Unload` (item 5) | the reload completes (item 6 closes the old apartment as usual), then the new apartment unloads as requested, like any loaded script |

<a id="s8-8"></a>
### 8.8 Limits

- At most one **hot** reload at a time (full reloads are not counted, TM L84); at most 17
  apartments exist (16 loaded plus one new apartment in the reload allowance, TM L26).
- The new apartment's cap is reserved from the 64 MiB reload allowance (TM L24); its
  snapshot, retained data files, the hand-off bytes and, from item 2, the store's **file
  index** are charged **inside** that cap (TM §8.10, TM rule 7.6.1: during a hot reload
  the file index counts in full toward both apartments' caps, conservatively twice in the
  sum, until the old cap leaves the sum at item 6). So a reload always fits its
  allowance, whatever the cap (D17 §8), and `SetHandOff`'s `"too large"` (§8.4) is
  measured against what is left of the new cap after all of these.
- Every item obeys the 6a slice, the heavy-frame gate and its floor (TM L6, L8, L9).
- In `scripttool test`, the walk file's `reload` input (`{ script, from }`, EM §11.5)
  requests a reload of a script from a second folder version, so the hand-off path is
  testable headless.

---

<a id="s9"></a>
## 9. The developer console

<a id="s9-1"></a>
### 9.1 Availability and layering

- The console exists **only with developer mode on** (EM §9.8). It is part of the developer
  overlay (the egui pass of UR Decision 1.6; tool UI, never game UI, ARCH §15), drawn above
  the UI pass and **below the reserved layer** (D17 §5.3).
- **Opening.** Only the user opens or closes it: by Tessera's developer-overlay toggle or by
  the script manager's "Console" button. No script call opens, closes, focuses or writes
  into its input line.
- **While a consent prompt shows** the console is hidden and takes no input (D17 §5.3).
- **Panes:** the log view (§9.2), the profiler lines (§9.3) and the REPL (§9.4).

<a id="s9-2"></a>
### 9.2 Log view

- Shows the records of every loaded script's log (`print`, `Core` log, error reports) and
  the ext host's own reports, newest last, with a filter by script and by level.
- **The log file** is written in TM §8.1's one format (one line per record, the
  `<time> <level> <source>` host prefix, `\xNN` escapes, the 4,096-byte record cap with
  `…[cut]`, TM L45). The console never changes the file; its display cut does not apply to
  it.
- **The display.** Every displayed record is prefixed by Tessera's own fixed-style frame:
  level, ⟨id⟩ · ⟨kind⟩, and for errors "Error in ⟨id⟩ · ⟨kind⟩:". Script-supplied text
  (the record's text and its chunk name) is drawn as reserved-layer plain text: TM §8.2
  steps 1–5 with the "error or log line" cap (TM L66), so the console and the reserved layer
  show script text the same way (D17 §5.3).
- Display is rate-limited per script and message (D17 §8 "Rate-limited reports", TM L67);
  the log file's own caps are TM L45's.

<a id="s9-3"></a>
### 9.3 Profiler lines

One line per loaded script, also shown in the script manager (D17 §8 "Profiler line"):
tick time of the last turn, the rolling mean, memory used of its cap, units of the last
turn, pending intents, active leases, limit hits by kind, and from slice 1b the overlay
fields of `GetStats` (drawn, dropped by count, dropped by coverage, coverage against its
share). When Tessera is built with its Tracy feature (ARCH §22), each ext call and each 6a
item is a zone named ⟨id⟩ and the kind of call.

<a id="s9-4"></a>
### 9.4 REPL

**Binding.** The REPL binds to **one** loaded apartment of the `scripts-dev` kind, chosen
by the user in the console. It never binds to an installed script (D17 §10, §5.2), so for
every script of `data/scripts/` the code that runs is exactly the hashed snapshot. With no
developer script loaded, the REPL is disabled. After a hot reload the binding follows the
id to the new apartment; when the script unloads, the REPL becomes unbound.

**Input.** One submission is at most **8 KiB** of UTF-8 text (TM L85; one or more lines;
Enter submits, Shift+Enter adds a line). History is kept in memory for the session only,
never written to disk, so nothing a script can influence persists (D17 §5.2).

**Submission.** A submission is queued for the bound script only while the backend gate
is `Open` (EM §10.3), the script is loaded and not suspended, and no earlier submission
for it is still waiting. Otherwise it is refused at once with a console line and nothing
is queued; the text stays in the input line. The cases are checked in this order: with
the gate not `Open` (6b, where a REPL call would run, does not run in `Draining`, and in
`Off` and `Starting` no script is loaded), the line is "No loaded extended script
'⟨id⟩'", as EM §10.5 shows for `/ext:reload` in `Draining`; for a suspended script, "⟨id⟩
is suspended: resume it first"; with a submission waiting, "a submission is waiting". So
a submission made during `Draining` is refused at once, just as one waiting when the
switch is queued is dropped. A waiting submission is dropped, with a console line, when
its script is suspended or unloads before it runs, or when a backend switch or session
end is queued.

**Compilation.** The **host** compiles the submission as a developer tool, never through any
function a script can reach (D17 §10), as text only (binary chunks refused), with the chunk
name `repl`. If the submission starts with `=`, the `=` is replaced by `return `. Otherwise
the host first tries `return ` followed by the submission and, if that does not compile,
the submission as written. Compilation happens inside the call below, at its start, and
each compilation attempt is charged **before** it runs `B` + 4 · *n* units, where *n* is
the byte length of the text that attempt compiles (the submission, plus the 7 bytes of
`return ` when added). A submission that needs both attempts is charged both. At 8 KiB
(8,192 bytes) the largest charge is that of a submission needing both attempts:
(200 + 4 · 8,199) + (200 + 4 · 8,192) = 32,996 + 32,968 = 65,964 units, about 0.33 ms
at 5 ns per unit, far below K (TM L15) and inside one call (TM L3, L4). (A submission
starting with `=` makes one attempt of at most 8,198 bytes.) The coefficient is provisional like every cost
(§3.4, EA1a OPEN-1): E7 measures the compiler and the registry holds the figure.

**Running.** The compiled chunk runs as one ext call in the bound script's next turn, after
its queued notifications and before its timers and `Tick`, in the apartment's own global
environment, under the same limits as every call (TM L3–L5, L23). A REPL call that aborts or
raises an error counts toward the abort-count watchdog like any call (TM L18); a compile
error runs nothing and counts nothing. `print` inside it goes to the script log and the
console as usual.

**Results.** The results are described **inside the same metered call**, after the chunk
returns, by host code that runs no script code (D17 §1.2 "The host never runs script code
to describe a value") and charges `e(n)` + `b(bytes)` for what it reads: `nil`, booleans
and numbers in their Lua text form, strings as plain text (§9.2's display rule), and every
other value by its type name and `tostring` number (`table: #17`). A table result also
shows, read by raw access (no metamethod runs), its first 32 entries (TM L91) in the order
of a raw traversal, one level deep, each described the same way. If the call's units or deadline
run out during the description, the description ends with "…" and no abort is counted for
that part.

<a id="s9-5"></a>
### 9.5 Keyboard

- The console takes keyboard focus only by the user's click on its input line or the
  user's toggle, never by a script and never from a consent prompt.
- While the REPL input line has focus, the client pushes `ui.eat_keyboard` (`opaque_keys`,
  IB D6) owned by the console, so keys go to the console and no game-band or ext binding
  fires; the panic chord still works, because its tier is tested before every band (D17
  §5.5). The entry is popped when the input line loses focus or the console closes.

> **OPEN:** the developer overlay's own input routing (how egui and the input mapper share
> raw events) is not yet specified in ARCH §13 or UR Decision 1.6. §9.5's band entry is
> this document's requirement on it; the overlay's spec states the mechanism.

---

<a id="s10"></a>
## 10. The generators

D17 §1.3 makes the **module registry** the single source of truth: each module is a Rust
port trait plus a **binding description**, and three outputs are generated from the
description: the API reference, a LuaLS annotation file, and the runtime argument
validation. None can drift from the others, because none is written by hand.

<a id="s10-1"></a>
### 10.1 The binding description

One description per module major, held in `tessera-ext` as static data (no I/O, no
generation at run time), plus one description of the **root**: the `Ext` table of
`api` major 1 (EA1a §3) and the globals the ext environment adds or replaces (`print`,
EA1a §2.11, §4.3).

| Item | Content |
|---|---|
| Module | name (PascalCase), major, minor, capability (or none), group (O, A, —), backends (`Offline`, `TesseraProtocol`), slice, a one-paragraph description in Tessera's own words |
| Root | the `Ext` table's functions (`Use`, `Has`, `Load`) and fields (`Version`), and the global functions (`print`), each described as a function or a field below, with the owner `Ext` or `global` |
| Function or method | name; owner (the module, a handle type for methods, `Ext` or `global`); parameters, in order; results; refusal reasons; apply step; cost (§3.4); per-frame caps it consumes, by TM L-number; the minor that added it; a deprecation note if any; a description |
| Field | name; owner; type from §10.2; whether constant for the session; a description. A field has no cost, no validation and no apply step (`Ext.Version`, EA1a §3) |
| Parameter | name; type from §10.2; constraints (range, length, enum set, element cap); optional with a default, or required. Only the **last** parameter may be `variadic` |
| Result | type from §10.2; for refusals, the reason strings (each must be in the reason table) |
| Handle type | name (`CameraLease`), its methods, sealed (always) |
| Notification | name; fields with types (and, per field, the capability that must be held for the field to be present, for example `world.read`, §4.2); when it arises |
| Option table, record, forms | name; fields with types, ranges, defaults and presence conditions (for example `Follow`'s options, a pose); for `forms`, each form's name and field set (§10.2) |
| Reason table | every reason string with its meaning (EA1a §2.4, §3.2) |

All descriptions, names and reasons are Tessera's own words and carry no BIN fact (LEGAL
§18.3 rule 1; D17 §12).

<a id="s10-2"></a>
### 10.2 The type vocabulary

The runtime checks are EA1a §2.3's value types; the vocabulary below names each of them
and adds the composite forms the slice-1a and slice-1b entries need, so that every
signature of EA1a and of this document can be described.

| Type | Runtime validation | LuaLS annotation | Reference text |
|---|---|---|---|
| `number(range)` | a Lua number, finite, inside the range | `number` | "number in [a, b]" |
| `integer(range)` | a Lua number with an integral value inside the range | `integer` | "integer a–b" |
| `DID` | EA1a §2.3 `DID`: an integer in 0 … 4,294,967,295 | `integer` (`---@alias Ext.DID integer`) | "DID" |
| `string(max)` | a Lua string of at most *max* raw bytes; invalid UTF-8 replaced by U+FFFD when the host reads it (EA1a §2.3); a constraint that the host checks on the text (length in bytes, white space, control characters) applies to the **replaced** text, as §7.2 does for `place` | `string` | "text, at most *max* bytes" |
| `enum{…}` | a Lua string equal to one member, compared byte-wise; a one-member enum is a **literal** (`"player"`) | an `---@alias` of the string literals (a literal is written as itself) | "one of …" |
| `boolean` | `true` or `false` | `boolean` | "boolean" |
| `colour` | EA1a §2.3 `colour`: `#RRGGBB` or `#RRGGBBAA` | `string` (`---@alias Ext.Colour string`) | "colour `#RRGGBB[AA]`" |
| `position` | EA1a §2.3 `position`: three `number`s as separate arguments, each \|v\| ≤ 10⁷ | `number` ×3 | "world position" |
| `record` (a pose, a key frame, option tables) | a table read by raw access only, walked after the reservation (§10.3); declared fields, each with its type, required or optional, and **ignored** fields (accepted and not read, such as a pose's `heading`); any other field name is a Lua error; at most the declared fields plus one are read | a `---@class` | a field table |
| `forms{F₁, …}` | a table that is exactly one of several records (§7.2's destination): a table matches form F when it has every field F requires and no field F does not list; exactly one form must match, else a Lua error. The forms must be **disjoint**: every pair of forms differs in a required field that the other does not list (the generator checks this) | one `---@class` per form and an `---@alias` of their union | one field table per form, with the rule above |
| `array(T, max)` | raw indices 1…*n*, *n* ≤ *max*, each element of type T; the raw length (`#` without metamethods) is read before the reservation, the elements after it (§10.3); holes are a Lua error | `T[]` | "array of T, at most *max*" |
| `table(caps)` | EA1a §2.3 `table`: any table walked with raw access only under the declared element, depth and byte caps | `table` | "table (at most …)" |
| `value` | the bounded value model (TM L33–L35) | `any` | "plain value (bounded)" |
| `any` | any Lua value, never walked by the host; how the native uses it is the entry's rule (`Core.Log` converts each with `tostring`, EA1a §4.3) | `any` | "any value" |
| `handle(K)` | a host userdata of kind K belonging to the calling apartment, checked at step 2 in this order: (a) the module that minted it still has its capability in the effective set, else `"not granted"`, whatever the handle's kind (EA1a §2.7, which raises `"not granted"` even from `Exists` and `Kind`, so no answer tells whether the object is gone); (b) its kind is K; (c) it is not stale, else a Lua error, except for `Exists` and `Kind` (EA1a §2.7) | `Ext.K` | "K handle" |
| `union(T₁ \| T₂ …)` | **selected by discriminator, then checked in full.** Each alternative's **discriminator** is the Lua type it accepts and, for a `handle(K)` alternative, the kind K. The discriminators of one union are pairwise different (the generator checks this): never two alternatives of one Lua type, except handle alternatives of different kinds, and never a range, a literal's value or a field. At step 2 a host handle first passes check (a) of `handle(K)` (`"not granted"` when its minting capability is gone); then the value's discriminator selects at most one alternative. A value that selects none is a Lua error naming every alternative. The selected alternative's checks then decide, in EA1a §2.4's order, exactly as if it were the parameter's only type, and no other alternative is tried: in `Follow`'s `"player"` \| `handle(Entity)`, a string that is not `"player"` raises naming the alternatives, a stale entity handle raises as stale, and an entity handle whose `entity.read` is gone raises `"not granted"` (§4.4) | `T₁\|T₂` | "T₁ or T₂" |
| `function` | a Lua function (a callable table is refused, EA1a §2.3) | `fun(...)` with the declared parameters | "function" |
| `variadic(T)` | zero or more trailing arguments, each of type T (`any` for `Core.Log` and `print`); EA1a §2.4's "no extra argument" rule does not apply to a `variadic` parameter, and a `variadic` parameter has no element cap of its own (the entry states its cost per argument, for example `b(n)` of the text) | `---@param ... T` | "any number of T" |
| `nil` | as an **optional** parameter's marking: absent or `nil`. As an alternative of a `union` of a **required** parameter (`lease:SetFov`'s `number(range) \| nil`, §4.4): an explicit `nil` that must be passed, so the argument counts toward the arity and leaving it out raises (EA1a §2.4 step 2); its discriminator is the Lua type `nil` | `?` suffix (optional); `\|nil` in the union (required) | "optional"; "or nil" |

<a id="s10-3"></a>
### 10.3 Runtime validation

The order of checks is EA1a §2.4's general rule (grant and backend, arity and types,
constraints, caps, charge, operation, with steps 1–4 paid by `B`); every native of every
slice follows it, and the runtime validation is generated from the description to
implement it. This section adds only what the generator needs and how slice 1b's entries
fit the order.

- **What is checked before the charge.** Steps 1–4 read only the arguments themselves:
  scalar arguments, the Lua type of a table argument, a handle's kind, staleness and
  minting capability, an array's **raw length**, and a string's raw byte length. They
  never read a table's fields or elements.
- **Every walk after the charge** (EA1a §2.5: a native that walks a Lua value reserves
  the cost of its caps before the walk). Every read of a table's fields or elements (a
  pose, `Follow`'s and `Play`'s options, the key frames, a `Sandbox` destination and its
  `anchor`, the numbers of `Lines`, a `SetHandOff` value) happens in step 6, after step 5
  has charged the entry's cost or reserved it at the cap. Every check that needs those
  fields (unknown field names, field types and ranges, the order of key-frame times, the
  match of a `forms` table, the `place` text after replacement) runs during that walk; a
  failure there is a Lua error, nothing is applied or queued, and the units of the walk
  stay charged (the unused reservation is refunded). §4.4, §4.8, §5.3 and §7.2 state the
  reservation of each such entry.
- **Slice-1b soft answers are step 4.** `"ended"` (lease intents), `"busy"`,
  `"unloading"`, `"not in world"`, `"pending"` (`Sandbox`, in §7.3's order) and the draft
  cap of `Overlay` adds are step-4 caps, so they are answered before the walk and cost
  `B` only. `Camera.Take`'s `"taken"` and `"unfocused"` are step 4 as well.
- **Variadic and union parameters.** A `variadic` parameter is exempt from the "no extra
  argument" rule; every other parameter list is closed. A `union` is never checked by
  trying its alternatives in turn: step 2 applies the minting-capability check to a host
  handle, selects the one alternative the value's discriminator names (§10.2), and runs
  that alternative's full checks (step 2, and its constraints at step 3), so the error a
  script gets is the selected alternative's own: `"not granted"` for a handle whose
  minting module lost its capability, the stale-handle error for a stale one, and a
  "matches no alternative" error only for a value whose Lua type (or handle kind) no
  alternative accepts. This is what §4.4 and TM X91 require of `lease:Follow`.
- **Handles.** Every handle argument, and `self`, is checked in the order of §10.2's
  `handle(K)` row: minting capability, kind, staleness.

**Field checks in the walk (answered by EA1a).** EA1a revision 5 makes it the general
rule (EA1a §2.4 steps 3 and 6, its OPEN-12(f)): step 3 checks only what is known without
reading a table argument's contents (scalar arguments, an unknown name passed as a scalar
argument, a table argument's type), and step 6 starts with the walk of the table
arguments, which raises for unknown fields, field types and ranges and element caps
before any effect. This section and invariant 23 follow it; part (a) of the request this
document made in revision 5 is done.

> **OPEN (request to EA1a, TM §0.1 rule 5's form), the order of a handle's step-2
> checks:** EA1a §2.4 step 2 lists a handle's kind, staleness and minting capability
> without an order, and EA1a §2.7 does not say whether its minting rule or its
> stale-handle rule wins for a stale handle whose minting capability is gone. This
> document checks the minting capability first, then the kind, then staleness (§10.2),
> because EA1a §2.7 makes even `Exists` and `Kind` raise `"not granted"` so that no answer
> tells whether the object is gone. EA1a is asked to state that order for every handle
> argument, for `self` and for handles inside table arguments at step 6. Until EA1a
> answers, this document's order applies to its own entries; it is stricter and changes
> no outcome a script may rely on, only which Lua error a value with several faults
> raises first.

**Message form.** The generated messages read `⟨chunk⟩:⟨line⟩: bad argument #⟨n⟩ to
'⟨Module.Function⟩' (⟨expected⟩, got ⟨actual⟩)`, where ⟨chunk⟩:⟨line⟩ is the position of the
innermost **Lua** function that called the native (a relative path, D17 §1.2), ⟨expected⟩
is the reference text of §10.2 and ⟨actual⟩ a Lua type name or "out of range". For methods,
`self` is not numbered (`lease:Hold(p)` reports a bad pose as argument #1), and a bad `self`
reads "bad self to 'CameraLease:Hold' (CameraLease expected, got table)". Errors about
elements of tables add the index or field name: "bad argument #1 to 'Overlay.Lines'
(element 37: number expected, got string)". As EA1a §2.4 says, the exact text is not part
of the contract; scripts must not parse it.

**Fuzzing.** The fuzz target for Lua-side argument validation (D17 M5.5 deliverables) drives
every registry entry, slice-1b entries included, with arbitrary arguments and checks that
each call ends in a result, a refusal or a Lua error, never a panic.

<a id="s10-4"></a>
### 10.4 The reference generator

- `tessera-scripttool dump-api --format reference [-o FILE]` (EM §11.2) writes the reference
  as Markdown, to standard output without `-o`. D17 §10 spells the generator
  `--ext-dump-api`; the subcommand form is EM's design addition to D17's command table,
  and this document only states what it generates.
- **Where it lives.** The committed reference is one generated file,
  `docs/specs/ext-api-reference.md`, part of what D17 calls `ext-api.md` (with EA1a and this
  document). The whole file is generator output, its first line the SPDX header line
  `<!-- SPDX-License-Identifier: GPL-3.0-or-later -->` and its second a fixed "generated by
  `tessera-scripttool dump-api`; do not edit" note. EA1a and this document link to it and
  keep their prose; nothing in the file is edited by hand.
- **Content**, in this order: first the **root** (§10.1), as one section headed
  `Ext` (the `api` major of EA1a §3): the `Ext` table's functions in a function table
  (§3.4's columns), then its fields in a field table (name, type, whether constant for
  the session, description), then the global functions the ext environment adds or
  replaces (`print`) in a function table of owner `global`, each group in byte-wise
  order of names; then each module, in byte-wise order of module names: the module card
  (§4.1's fields), the function table (§3.4's columns), each handle type's method table,
  the option-table and notification tables, and each entry's description; then the
  reason table. Within a module, functions are in byte-wise order of their names, and
  methods under their handle type in the same order.
- **Deterministic output:** LF line endings, no timestamps, no paths, no build-dependent
  text; Markdown table cells escape `|`; the same registry gives the same bytes on every
  platform.

<a id="s10-5"></a>
### 10.5 The annotation generator

- `tessera-scripttool dump-api --format luals [-o FILE]` (EM §11.2) writes a LuaLS-style
  annotation file, `ext.lua` by convention, that editors load as a library so they complete
  and check `Ext.*` (D17 §10).
- **Content:** a `---@meta` header; one comment line with `Ext.Version` and a SHA-256 of
  the registry's canonical form; a `---@class` per module table (`Ext.Camera`) and per
  handle type (`Ext.CameraLease`) with a `---@field` or function stub per entry carrying
  `---@param` and `---@return` lines (a refusal is annotated as `nil` plus `string`); a
  `---@alias` per enum; a `---@class` per option table and notification table; and
  `Ext.Use` with one `---@overload` per module major, so that
  `Ext.Use("Camera", 1)` is typed as `Ext.Camera?, string?`. Descriptions are copied as
  doc comments.
- The file is generated on demand and shipped with releases; it is not committed, so no CI
  check compares it. `tessera-scripttool init` (EM §11.2) may place a copy in a new
  script's folder.
- Deterministic as §10.4.

**The canonical form of the registry** (used for the SHA-256 above and the fixture of
§10.6 step 4). It is one JSON text (RFC 8259) serialised by the JSON Canonicalization
Scheme (RFC 8785): UTF-8, no insignificant white space, object members sorted by RFC 8785's
rule, numbers and strings in RFC 8785's forms. `tessera-ext` computes it from the static
registry with one library function, which the generators and the check of §10.6 both call.
Its data model:

- **Top level:** an object with `canonical` (the integer 1, the version of this form),
  `root` (the root description, §10.1), `modules` and `reasons`.
- **Order.** Arrays whose order means something keep it: parameters in signature order,
  a record's fields and a form's fields in declaration order, a union's alternatives and
  a `forms` type's forms in declared order. Every other collection is an array sorted
  byte-wise by name: modules by (name, major), and within a module its functions,
  handle types (each with its methods), fields, notifications, option tables and records;
  enum members; reasons.
- **Entries.** Each function, method or field is an object with `kind` (`"function"`,
  `"method"` or `"field"`), `name`, `owner`, `params` (each `{name, type, optional}` plus
  `default` when it has one), `results`, `reasons` (sorted), `apply`, `cost`, `caps`
  (TM L-numbers as integers, sorted), `since` (the minor, an integer), `deprecated` (a
  string, only when present) and `description`. A field has only `kind`, `name`, `owner`,
  `type`, `constant` and `description`.
- **Types** are objects whose `t` member names the §10.2 type (`"number"`, `"integer"`,
  `"DID"`, `"string"`, `"enum"`, `"boolean"`, `"colour"`, `"position"`, `"record"`,
  `"forms"`, `"array"`, `"table"`, `"value"`, `"any"`, `"handle"`, `"union"`,
  `"function"`, `"variadic"`) and whose other members are its parameters (`min`, `max`,
  `members`, `fields`, `forms`, `of`, `alternatives`, `kind`, `caps`). An angle bound is
  written as the number the registry holds (its `f64` value, printed by RFC 8785).
- **Costs** are arrays of terms in the entry's order, each an object with these members:
  - `sym`: one of EA1a §2.5's symbols, `"B"`, `"b"`, `"e"`, `"h"`, `"N"`, `"P"`, `"Q"`,
    `"V"`. The fixed symbols `B`, `N`, `Q` and `V` carry no `n` and no `k`; the counted
    symbols `b`, `e`, `h` and `P` always carry `n`.
  - `n`: either an **integer**, a fixed count, written as its value (a product of
    constants is multiplied out: `Play`'s `e(24 · 256)` is `"n": 6144`), or a **measure**,
    an object `{"m": …, "of": …}` with `m` one of `"length"` (the raw length of an array
    argument, read before the walk, §10.3), `"bytes"` (the byte length of a string, or of
    a serialised value) and `"elements"` (elements counted by EA1a §2.5's rule), and
    `of` the name of the parameter it is measured on, or `"stored"` for a value the entry
    reads from host state rather than from an argument (`Core.TakeHandOff`'s stored
    hand-off value, §8.4).
  - `k`: an integer factor greater than 1 that multiplies a measure; present only with a
    measure, and omitted when the factor is 1.
  - `rsv`: `true` on a reservation, omitted otherwise.

  So `Overlay.Lines`' cost `B` + `e(2m)` is `[{"sym": "B"}, {"sym": "e", "k": 2, "n":
  {"m": "length", "of": "coords"}}]`, `Core.TakeHandOff`'s `B` + `e(elements)` +
  `b(bytes)` is `[{"sym": "B"}, {"sym": "e", "n": {"m": "elements", "of": "stored"}},
  {"sym": "b", "n": {"m": "bytes", "of": "stored"}}]`, and `Sandbox.Teleport`'s is
  `[{"sym": "B"}, {"sym": "e", "n": 26, "rsv": true}, {"sym": "b", "n": 256, "rsv":
  true}]` (a refund down to what the walk read is part of every reservation, EA1a §2.5,
  and is not written). Each cost has exactly one such form, so two implementations write
  the same terms.
- **Absent members are omitted**, never written as `null`. No member holds a path, a
  time or anything that depends on the build.

So two implementations of this section produce the same bytes from the same registry, and
the SHA-256 (FIPS 180-4) of those bytes, written in lower-case hexadecimal, identifies it.

> **OPEN:** the licence of the generated annotation file. It is derived from GPL-licensed
> registry data, but scripts of any licence should be able to use it in their editors. The
> owner decides whether it carries `GPL-3.0-or-later` or a permissive licence such as
> CC0-1.0, and `REUSE.toml` records the choice if a copy is ever committed.

<a id="s10-6"></a>
### 10.6 The CI check

A check in `tools/` (run in CI, D17 §10 "A CI check fails if the committed reference differs
from the generated one"):

1. runs `dump-api --format reference` and fails if the output differs, byte for byte,
   from `docs/specs/ext-api-reference.md`, printing a diff and the command that updates it;
2. runs `dump-api --format luals` twice and fails if the two outputs differ (determinism);
3. fails if any registry entry lacks a description, a cost, an apply step or a `since`
   minor, if a refusal reason is missing from the reason table, if a module's capability
   is not in EM §6.1's catalogue, or if an entry's `since` is above its module's minor;
4. fails if the registry removes or changes an entry of an existing major (D17 §2
   "Versioning", EM §12.3: within a major only additions). This part is a test of
   `tessera-ext` (run by CI's `cargo test`), which compares the current canonical form
   (§10.5) with the previous release's, committed as a test fixture (our own registry
   data, no game data). Both are parsed, and for each module major of the fixture every
   entry must still exist with the same `kind`, `owner`, `params`, `results`, `reasons`,
   `apply` and `since`, and every type it uses must be unchanged; `description`, `cost`
   and `caps` may change (costs are calibrated by E7, EA1a OPEN-1). New entries must have
   a `since` above the fixture's minor of their module. A change of the form's own
   `canonical` version comes with a converter from the previous version in the same
   test, so the comparison never lapses.

---

<a id="s11"></a>
## 11. Invariants

TM §12's invariants hold; these are the slice-1b modules' own.

1. At most one camera lease is active at any time, for all scripts.
2. Every lease has ended by step 3 of the frame of each event of §4.5 that ends it (at most
   350 ms after its last `Take`, `Renew` or intent, measured on the lease clock; focus loss;
   suspension for any cause, any grant revocation included; the panic chord; a queued
   backend switch or session end; an unload request, in the frame it is made), and from
   that step 3 on the view is *V*ᵤ (TM invariant 25). An unloading apartment, its `Unload`
   handler included, starts no lease and has no lease intent, overlay addition or
   `Sandbox` command applied (§4.5, TM §8.17).
3. No lease is active while the game window lacks keyboard focus.
4. While no lease is active, the view is the user's view *V*ᵤ, exactly as with no ext script
   loaded.
5. Character movement, the streaming focus and level-of-detail selection always use the
   user's camera *V*ᵤ and the player's controls; the user's pointer always picks from the
   presented frame (TM §8.17).
6. A leased eye is never more than 500 m (TM L82) from the reference point *P*: the
   player's interpolated position, or the user's own eye when no character is placed
   (§4.6). An entity `Follow` reads its target only while `entity.read` is in the
   effective set (§4.7, TM §8.23).
7. The lease indicator is drawn in every frame in which a lease is active or whose
   presented view a lease decided, names its script with id and kind, and never catches
   the pointer.
8. A lease handle never raises a stale-handle error while its apartment lives.
9. An overlay add either adds everything it was given or nothing, and every `Overlay`
   call's largest cost fits one call's units and deadline (TM L3, L4).
10. Overlay coverage and primitive counts drawn per frame stay within TM rule 7.1.4's shares
    (TM L53, L54; TM invariant 5), counted conservatively by TM rule 7.1.2.
11. Overlays and windows of a script vanish in the frame it is suspended or the panic chord
    is pressed, and are not drawn while a consent prompt shows.
12. `Sandbox` is never usable on a backend other than `Offline`; no `Sandbox` intent is ever
    submitted to another backend or under a newer session generation.
13. At most one ext teleport is outstanding; none is submitted while another teleport is
    pending; the slot is released at the first of its outcome's arrival, the intent's
    discard or non-submission, a failed submission and the rise of its session generation
    (the frame a switch or session end is queued), so it is never carried into the next
    session; teleports and outcomes are numbered per `Session` connection (reset when
    `Connect` is submitted), a failed submission consumes no number, an outcome is
    matched by (connection, request number), and an outcome of an earlier connection or
    of an already released request releases and answers nothing (TM L75). Clock changes
    of all scripts together stay within TM L75's 4 in every one-second interval, and each
    script's commands within 1 (sliding windows over accepted calls, §7.4).
14. Every accepted `Teleport` produces exactly one `TeleportDone` when a `TeleportDone`
    handler is set as its outcome arises, unless the script is suspended or unloading, or
    its slot was released by the generation's rise, before its outcome arrives.
    `QueryLocation` leaves at most one pending flag per script, cleared by the first
    report of cause *requested*, the generation's rise or the unload request (§7.5). No
    position field reaches a script without `world.read` (§4.2, §7.5).
15. A hot reload happens only for a `scripts-dev` script with developer mode on, a new
    manifest that a load would accept, and unchanged declarations (TM §8.24); every other
    reload is full and passes no hand-off table. A reload is started only for a loaded,
    unsuspended script while the backend gate is `Open` (EM §9.8, §10.5); at most one hot
    reload runs at a time, and full reloads never wait for one another or for it (TM
    L84); a full reload's load starts only after the old close item has run and the old
    cap has left the 512 MiB sum, waiting for that and never refused for it, so a script
    never has two apartments outside a hot reload (TM L24, L26); the watcher holds at most one flag
    per loaded developer script, never a queue, and queues nothing outside `Open`.
16. The hand-off table is serialised only inside the old `Unload` call, charged inside the
    new apartment's cap, and converted only inside the new top-level run.
17. During a hot reload, a script never has two apartments that run code in the same frame:
    the old one runs nothing but its `Unload` handler from item 4, the new one runs from
    item 5.
18. A failed reload (snapshot, validation or compile error) leaves the old apartment running
    exactly as before.
19. A `Store.Get` returns the latest value written before it in program order, across a hot
    reload too, and such a read is counted and delivered like any read (TM invariant 13).
20. The REPL binds only to `scripts-dev` apartments, compiles only what the user typed, and
    runs it, the description of its results included, under every limit of an ordinary
    call.
21. No script text in the console draws a character that TM §8.2 removes or more than two
    marks per base, and the log file holds only TM §8.1 records (TM invariants 16, 17).
22. `docs/specs/ext-api-reference.md` equals the registry's output byte for byte at every
    commit.
23. Every native validates its arguments in EA1a §2.4's order as §10.3 applies it, checks
    a handle's minting capability before its kind and staleness, selects a `union`'s
    alternative by its discriminator and then checks that alternative in full (so a kept
    handle whose minting capability is gone always raises `"not granted"`), reads no
    table field or element before its charge or reservation, and never panics.
24. No argument names region 0; a result reports region 0 only for a sandbox without
    world data (§3.1).

<a id="s12"></a>
## 12. Worked examples

**A camera-follow script under starvation.** A reference script takes the camera lease in its
top-level run and calls `lease:Follow("player", { distance = 8, pitch = -0.3 })` in every
`Tick`. At 60 fps each tick renews the lease to now + 350 ms, and step 3 of the next frame
places the eye 8 m behind and above the player's interpolated position. The user then hits
a severe hitch in a debug build: from frame 1 on, every frame takes 190 ms and reaches
step 7 long after 22 ms, so the heavy-frame gate skips ext (D17 §4). The last renewal was
in frame 0's 6b, 20 ms after frame 0 began, so the lease expires 370 ms after that start.
Frame 1 (starting at 190 ms) is gated. Frame 2 starts at 380 ms; it is a floor frame (TM
rule 7.4.1: 250 ms have passed since the start of frame 0, the last frame in which ext
ran), but expiry is checked at the start of its
step 3, before its 6b: the lease has expired, the view snaps back to the user's camera in
that frame and the indicator disappears with it. In the script's next turn (that floor
frame's 6b or a later one) it gets `LeaseEnded { reason = "expired" }`; its handler calls
`Camera.Take()` and gets a new lease; the follow resumes from the next frame. With frames
of 30 ms the floor frames come at most about 280 ms apart and lease holders are ticked
first (D17 §4), so the lease survives; throttling alone never drops it either (a half-rate
tick at 30 fps comes every 66 ms).

**The panic chord during a path.** A script plays a 20-key-frame path. The user presses the
chord. In step 1 the script is suspended, its lease ends with `"panic"`, its intents are
dropped and its overlay is cleared; step 3 of the same frame uses the user's camera. No
`LeaseEnded` is queued (the script is suspended). When the user resumes the script, its
first turn gets `Resumed`, and `lease:EndReason()` answers `"panic"`; `lease:Renew()`
answers `nil, "ended"`.

**A backend switch under a lease.** A script holds a `Hold` lease when the user starts a
switch to another backend. The gate enters `Draining` in that frame; step 3 of that frame
ends the lease with `"switch"` and the user's view returns, although the script's unload
item may run many frames later (TM rule 7.4.4).

**A quad flood.** A hostile script with `overlay` and `world.read` and no `Tick` handler
has five timers (`Core.Every(0, …)`, EA1a §4.2) that fire in every turn, with the UI scale
at 1.0 and the screen 1920 × 1080. The first four each add 2,000 quads of 960 × 540 at
integer positions in a loop (about 400,000 units of `B` plus the loop's own, inside one
call's 800,000, TM L4); the fifth tries 2,001. The 2 ms cap splits the turn over several
frames (TM L5, TM §8.13), and the draft grows across them. The fifth timer's 2,001st add,
the 10,001st of the draft, answers `nil, "busy"`, so the draft holds 10,000; at the end
of the turn they become the shown list (§5.2). It is the only overlay script (*n* = 1),
so *c* = 10,000 and *A* = min(0.5, 1.0) = 0.5 screen (TM rule 7.1.4). Each quad counts
518,400 pixels, 0.25 screen. The first two are drawn (Σ = 0.5); every later one would
pass *A* and is dropped alone. `GetStats` reports `listed = 10000`, `dropped_count = 0`,
`coverage_share = 0.5`, and, because the script holds `world.read`, `drawn = 2`,
`culled = 0`, `dropped_coverage = 9998`, `coverage = 0.5`. Without `world.read` it would
get only the first three. A full-screen quad (1.0 screen) never fits the 0.5 share and is
never drawn.

**Two more scripts and the near-camera box flood.** A second hostile script adds 10,000
filled boxes around the camera over several calls of one turn in the same way, each with a corner behind the near plane but the eye
outside the box, and a third draws 500 small outline boxes. Now *n* = 3: *c* = 6,666 and
*A* = 1/3 screen each. The quad script draws one quarter-screen quad (0.25); a second
would make 0.5 > 1/3. Each box of the box script counts one full screen (TM rule 7.1.2), so
none fits and all 6,666 considered boxes are dropped, the remaining 3,334 by the count
share; a box with the eye inside it counts 0 and draws nothing, so placing boxes around the
eye gains nothing. The outline boxes fit and are drawn. In total at most 1.0 screen is
filled.

**A waypoint overlay.** The reference waypoint script draws 500 primitives: 400 outline boxes
of 2 m around waypoints and 100 lines between them. At 3840 × 2160 a distant box's twelve
edges count a few hundred pixels together and a 1,600-pixel line counts
⌈1,601 + 1,601 + 1 + 1⌉ = 3,204 pixels (TM rule 7.1.2, *w* = 1), so the total is about half a
megapixel, about 0.06 screen, far inside its share: nothing is dropped, and E4's
10,000-primitive measurement covers its encoding.

**A teleport bookmark.** A key-bound script saves the player's position and yaw in
`Store` on one action and teleports there on another. The user presses the teleport key:
the script calls `Sandbox.Teleport({ region = r, x = x, y = y, z = z, yaw = w, ground =
true })`, with `w` the `yaw` that `World.Here()` returned (EA1a §2.10), which answers `true`
and takes the global teleport slot. In the same second the user presses it again:
`nil, "busy"` (one command per script per second). In step 2 of the next frame the client
finds no other teleport pending and the session in the world, and submits the command,
with `w` narrowed to `f32` as the model's heading (§7.2), as request number *k* of the
current `Session` connection. The sandbox moves the player; the location report of cause
*teleported* arrives in a later step 2 as that connection's outcome *k*, which releases
the slot; the script's next turn gets `TeleportDone { ok = true, yaw = …, heading = … }`
(with `region`, `x`, `y`, `z` only if it holds `world.read`). Had the user started a
switch to a new `Offline` session before the outcome arrived, the slot would have been
released in the frame the switch was queued, an outcome the old session still sent during
`Draining` dropped, and the new connection's numbering started again at 1 (§7.4). Had
`Session::send` returned a backend error, the number *k* would not have been consumed and
the script would have got `TeleportDone { ok = false, reason = "failed" }`.
Had the user typed their own teleport command in the frame between the call and step 2, the
ext intent would not have been submitted, and the script would have got
`TeleportDone { ok = false, reason = "cancelled" }` with no chat line, since nothing reached
the sandbox.

**A hot reload of a UI script.** A developer edits `ui/panel.lua` of `data/scripts-dev/notes/`
and saves. 300 ms later the watcher's quiet period ends and a reload is queued. Its cap is
reserved from the reload allowance; a worker snapshots the folder; the snapshot result
validates the new manifest, finds the declarations unchanged and creates the new apartment.
Three compilation items run over three frames while the old window keeps working. The old
`Unload` handler (reason `"reload"`) saves its draft note with `Store.Set` and calls
`Core.SetHandOff({ x = 400, y = 220, w = 320, h = 240, scroll = 12 })`, which answers
`true`. From that item the old window is gone. One frame later the new top-level run calls
`Core.TakeHandOff()`, gets the table, creates the window at the same place, scrolls the list
and calls `Store.Get("draft")`; its first turn receives the value from the pending write
(TM rule 7.6.3), so the draft is the one just saved, although the write-behind has not
flushed yet. The next item closes the old state; when its final flush completes, the
allowance is free. The user saw the window disappear for one frame and come back where it
was.

**A reload that changes a declaration.** The developer adds `camera` to the manifest's
capabilities and saves. The snapshot result sees a changed capability list, so the reload is
full: the old apartment unloads (its `Unload` handler's `SetHandOff` answers
`false, "not a hot reload"`), the grant dialog asks again for every capability, and only
after the user's answer is the new snapshot loaded, with no hand-off table.

**A compile error.** The developer saves a file with a syntax error. The compilation item
fails; the old apartment never stops running, the console shows
"Error in notes · developer: ui/panel.lua:14: …", and the reservation is released. Fixing
the file triggers the next reload.

**The REPL.** With the console open and bound to `notes · developer`, the developer types
`=#state.items`. The host replaces `=` with `return `, compiles it (a few hundred bytes,
charged before compiling), and runs it in the script's next turn: the console shows `42`.
Typing `while true do end` runs until the 4 ms deadline aborts it; the console shows the
abort, and the abort counts toward the watchdog like any call. While the script is
suspended, a submission is refused with "notes is suspended: resume it first".

**A new registry entry.** An implementer adds `Overlay.Arrow` to the registry. CI's reference
check fails with a diff and the command that regenerates `ext-api-reference.md`; once it is
regenerated and committed, the check passes. The CI also fails until the new entry has a
description, a cost, an apply step and `since` = the module's new minor; and it would fail
outright if the change removed or renamed `Overlay.Line` within major 1.

<a id="s13"></a>
## 13. Tests

Slice 1b's exit check (D17 "Later slices") runs E4, E6 and E9 with the slice-1b reference
scripts, the TM escape cases of slice 1b, and this document's functional tests.

**Budgets.**
- **E4** (D17 §9): 10,000 primitives, CPU encode ≤ 0.3 ms, GPU ≤ 0.3 ms.
- **E9** (TM rule 7.1.6, TM L56): the overlay part, at the coverage bounds, at the
  reference display's native 3840 × 2160, with ext composition at its bounds: GPU ≤ 1.0 ms;
  encoding of 20,000 primitives ≤ 0.6 ms. If it does not hold, TM rule 7.1.6's shrinking
  applies.
- **E6** additions: TM X29 and X30, each alone and both together, with ext windows at the
  composition bounds; the frame rule holds in the M4.5 walking run.

**Escape cases** (TM §11, named only by their X-numbers, TM §0.1 rule 3; this document
keeps no escape catalogue of its own). Each case's setup and "fails if" are TM's; a case
with parts under several harnesses reports each part as TM §11 says. Slice 1b runs the
slice-1b cases and the slice-1b parts of earlier cases:

| X | Slice-1b part |
|---|---|
| X16 | D17 E6's two backend-switch cases with a hot reload's new apartment as the 17th apartment (RM, E6) |
| X20 | the maximal hand-off table (with hot reload) |
| X27 | its hot-reload part (the `memory_mib = 64` reload with 8 MiB of data files and a 1 MiB hand-off, the file index counted in the new cap, §8.8) |
| X28 | each load case plus a hot reload |
| X29, X30 | overlay coverage (CI counting; RM in E6 and E9); X29 builds its lists over several calls and frames, as §5.3 requires |
| X44 | its slice-1b part (TM tags it 1b, 2; its `Files.Pick` part runs in slice 2): lease ends at every event, a user unload and `Core.Unload` included, each in the frame of its request; `Camera.Take`, a lease intent, an `Overlay` add and a `Sandbox` command inside an `Unload` handler during a user unload and during a hot reload whose new apartment then calls `Take` and gets a lease; picking under a `Hold` |
| X50 | the lease indicator over a plugin button |
| X64 | a hot reload that changes the capability list passes no hand-off |
| X80 | a `Sandbox` teleport before a switch to `Other`; the slot after a suspended script's outcome; a teleport outstanding when a switch to a new `Offline` session is queued (slot free in that frame), then a teleport in the new session numbered from 1; a late outcome of the old connection; a teleport whose submission fails (number not consumed) |
| X84 | `Sandbox` on a `TesseraProtocol` stub with a permissive policy |
| X85 | its slice-1b part: a switch with a hot reload in progress (17 apartments) within 392 frames, and a lease held when the switch is queued ends in that frame |
| X87 | a hot reload in progress when a switch is queued |
| X88 | its hot-reload part (§8.6) |
| X91 | revocation with a lease, a path and a `Follow` active; a grant-store re-read that narrows a lease holder's record; a revoke-resume-widen of optional `camera`; a kept entity handle passed to `lease:Follow` after an `entity.read` revocation and resume raises `"not granted"`, also when its entity has despawned meanwhile (stale), never "matches no alternative" (§10.2) |
| X92 | an `author`-only hot reload; an invalid new manifest at a hot reload; a full reload across an `author` change, its dialog answered as EM §11.4 says |

X51 covers the log file (TM §8.1) and X47 part (b) the display filter (TM §8.2) that the
console shares with the reserved layer; the console's own use of that filter is a
functional test below.

**Functional tests** (this document's own, not escape cases). They run in `scripttool
test` (EM §11.4) unless marked **(CI)**: a client integration test, headless, on a
synthetic world with test hooks (TM §2 "Harness codes"). The walk file's `command` input
passes only `/ext:` lines to the client (EM §11.5), so every test that needs a teleport,
a location query or a clock change **of the user**, or a change of region, is a client
integration test. The synthetic world of a client integration test uses regions from 1 to
255 (§3.1), so its destinations can name them.

> **OPEN (request to EM, TM §0.1 rule 5's form):** EM §11.5 does not state the region
> number of `scripttool test`'s synthetic world. §3.1 and this section rely on it being
> from 1 to 255 (a result reports 0 only for a sandbox without world data, invariant 24),
> and the `region` that `GetPose` reports there to a script with `world.read` must be
> that number. EM is asked to state the synthetic world's region number in §11.5's `[world]`
> row (for example a fixed region 1 that the harness prints with its centre landblock).
> A region-change input in the walk file is **not** requested: the one test that needs it
> runs in the client integration harness (**(CI)** above). Until EM answers, the
> `scripttool test` cases of this section do not depend on the region's value, only on
> its being the same for the whole run.
- *Leases.* A second script's `Take` answers `"taken"`; `Take` while unfocused answers
  `"unfocused"`. W moves the character relative to the user's camera during a `Hold` that
  faces the other way. A `Hold` 2 km from the player is drawn 500 m from the player; with
  no placed character (free fly, EA1a §10.2) it is drawn 500 m from the user's eye, and a
  `Follow("player")` shows the user's view until a character is placed, then follows it;
  in free fly without a character, `GetPose` reports the free-fly eye to a script with
  `world.read` (§4.2).
  **(CI)** A region change ends the lease with `"region"` (a client integration test on a
  synthetic world of two regions: `scripttool test`'s synthetic world has one region and
  its walk file has no region-change input, EM §11.5). A `yaw` read with `World.Here()` and
  passed to `Hold` (with pitch 0) gives a view facing the character's facing, and, for a
  script with `world.read`, a pose returned by `Camera.GetPose()` passed back to `Hold`
  reproduces the same view (its `heading`, `region` and `leased` ignored, as in a key
  frame); for a script without `world.read`, `GetPose` and `GetUserPose` return no `x`,
  `y`, `z` or `region`. A `Sandbox` destination with a `heading` field raises (§7.2). A
  looped path of three key frames evaluates as §4.8 says at the wrap, including the cyclic
  neighbours. The streaming focus and the level-of-detail selection are unchanged by a
  `Hold` far from the player. `lease:Follow(entity)` raises `"not granted"` for a script
  without `entity.read`, and also with a kept entity handle after an `entity.read`
  revocation and resume (TM X91), whether that handle is live or stale; with
  `entity.read` held, a stale entity handle raises the stale-handle error, and
  `Follow("players")` raises naming both alternatives (§10.2). `lease:SetFov()` with no
  argument raises, and `lease:SetFov(nil)` answers `true` (§4.4). A user unload and `Core.Unload` (in 6b) each end the
  lease with `"unload"` in the frame of the request (step 3 of that frame or of the next
  one, §4.5), however late the unload item runs. Inside an `Unload` handler `Camera.Take`
  answers `nil, "taken"` and creates nothing, a lease intent answers `nil, "ended"`, and
  an intent queued anyway (a test hook) is discarded at the handler's return; during a hot
  reload, the new apartment's `Take` in its top-level run gets a lease (TM X44).
- *Overlay.* A `Lines` call of 2,048 lines succeeds in a fresh call, and its charge is
  `B` + `e(24,576)`; one of 2,049 lines, or of a length not a multiple of 6, raises before
  any element is read and is charged `B` only; one of 2,048 lines whose last element is a
  string raises naming index 12,288 and adds nothing; one that would take the draft past
  10,000 answers `nil, "busy"` and adds nothing. A draft of 10,000 primitives built over
  five calls of one turn (three of them `Lines`) is shown whole at the turn's swap. §12's
  flood examples, built over several calls as they say, give exactly their `GetStats`
  counts in `scripttool test` (virtual surface 1920 × 1080, EM §11.5 `[screen]`), and a
  script without `world.read` gets only `listed`, `dropped_count` and `coverage_share`. A
  script with no `Tick` handler swaps at the end of each completed turn. A filled box with
  the eye inside draws nothing, counts 0 and is reported under `culled`. `Overlay.Rgba(1, 0.4, 0)` answers
  `"#FF6600FF"`. An `Overlay` add inside an `Unload` handler answers `nil, "unloading"`.
- *Sandbox* **(CI)**. An ext teleport is refused while a user teleport is pending, and is
  not submitted when a user teleport becomes pending between the call and step 2. A
  refused and a cancelled teleport each produce one `TeleportDone` with the right reason,
  from the typed outcome event, and the plugin-visible chat lines are exactly those of the
  same commands typed by the user. A teleport whose script is suspended before the outcome
  arrives releases the slot at the arrival, and another script's teleport is then
  accepted. A teleport outstanding when a switch to a new `Offline` session is **queued**
  releases the slot in that frame; a user teleport sent to the old session during
  `Draining` is numbered in the old connection; a teleport in the new session is accepted
  as its request 1; and a late outcome of the old connection is dropped without a
  `TeleportDone`. A teleport whose `Session::send` fails (a test hook in the backend)
  answers `TeleportDone { ok = false, reason = "failed" }`, releases the slot and consumes
  no request number, so the next teleport's outcome is still matched. A teleport called in
  the world whose session leaves the world before step 2 answers `"cancelled"`. A
  `Sandbox` command inside an `Unload` handler answers `nil, "unloading"`, and a teleport
  the handler's test hook queues anyway is never submitted. A destination's `yaw` reaches
  the model's heading unchanged except for `f32` narrowing and wrapping; a `place` of 256
  bytes with invalid UTF-8 that grows past 256 bytes when replaced raises, and one with a
  control character raises. A destination with `region = 0` raises (§3.1), and one whose
  `anchor` is the array `{ 3, 4, 256 }` raises (§7.2). Two `QueryLocation` calls of one
  script whose reports are delayed (a test hook in the backend) leave one pending flag and
  produce at most one `Location`; a switch queued while the flag is set clears it, and no
  `Location` follows (§7.5). Five scripts
  that each call `SetTimeOfDay` once in the same frame get four `true` and, for the call
  that runs fifth, `nil, "busy"` (TM L75); one of them calling again exactly 1 s after its
  accepted call is accepted, and a call refused with `"busy"` does not delay the next
  (sliding windows over accepted calls, §7.4). Without `world.read`, `TeleportDone` and
  `Location` carry no `region`, `x`, `y`, `z` or `anchor`. A script with no `TeleportDone`
  handler set gets no `TeleportDone`, and the slot is still released at the outcome.
- *Hot reload.* A hot reload of a UI script keeps its window state through the hand-off
  table and keeps the frame rule (D17 slice 1b exit). A reload that changes each
  declaration of §8.1 in turn is full, with no hand-off. `/ext:reload` of an installed
  script is full and re-asks group A after a content change. A compile error and a
  snapshot refusal leave the old apartment running. The panic chord before and after
  item 4, a backend switch during a reload, a user unload and `Core.Unload` during items
  1–3, a user unload between items 4 and 5, and developer mode turned off during a reload
  each end as §8.7 says. A maximal hand-off table, a cyclic one, and one larger than what
  is left of the new cap after its snapshot, data files and the store's file index
  (`"too large"`). A hot reload of a script with `memory_mib = 64`, 8 MiB of data files
  and a 1 MiB hand-off stays within the allowance at every item (TM X27, above).
  `/ext:reload` with developer mode off, of an id that is not loaded, and of a loaded but
  suspended script queues nothing and shows EM §9.8's notice ("Developer mode is off",
  "No loaded extended script '⟨id⟩'", "⟨id⟩ is suspended: resume it first"); a watcher
  event for a suspended script queues nothing. In `Draining`, `/ext:reload` of a loaded
  script queues nothing and shows "No loaded extended script '⟨id⟩'" (EM §10.5); a
  file changed while the gate is not `Open` queues no reload, then or when the gate opens
  again, and every waiting request is dropped when a switch is queued. A **required**
  grant revoked before item 4 cancels the reload and unloads the script; an optional one
  leaves it loaded and suspended. A full reload's load takes its reservation only after
  the old close item has run (16 loaded scripts and a full reload of one of them never
  make 17 apartments, TM L26). With the sum full (16 scripts with `memory_mib = 32`) and
  a worker latency of several frames (`--latency`, EM §11.4), so that the old
  apartment's final flush completes some frames after its close item, a full reload of
  one of them waits after the close item, is never refused, and starts its phase A only
  once that flush has completed (TM L24, L84, §8.1). With 32 worker requests of the old apartment outstanding
  at item 4, the new apartment's first request answers `nil, "busy"` until one of them
  has ended (TM L27). A full reload of one script held at its grant dialog does not
  delay a hot reload of another (TM L84). A watcher fed 100,000
  change events in one second under one script's folder, and as many under a folder of no
  loaded script, holds at most 16 flags and queues exactly one reload, 300 ms after the
  last event.
- *Console.* The REPL cannot bind to an installed script; a submission for a suspended
  script is refused; a submission made while the gate is `Draining` is refused at once
  with "No loaded extended script '⟨id⟩'" (§9.4); the console is hidden while a consent prompt shows; a REPL call
  returning a table with 10,000 entries shows 32 and stays inside its call's limits; an
  8 KiB submission that needs both compilation attempts is charged both, 65,964 units
  (§9.4); a
  `print` of X47's hostile string is drawn in the console with none of TM §8.2's removed
  characters, at most two marks per base and the log-line cap, while the log file holds
  its full escaped record (TM §8.1).
- *Generators.* The CI check fails on a hand edit of `ext-api-reference.md` and on a
  registry entry without a description. The canonical form (§10.5) of a registry built in
  two different orders is the same bytes, and the fixture test of §10.6 step 4 fails when
  an existing entry's parameter is renamed. EA1a's `Core.Log` and `print` (variadic),
  `Follow`'s union target and `Sandbox.Teleport`'s destination forms are described by
  §10.2's vocabulary. The validation fuzz target covers every slice-1b entry.

<a id="s14"></a>
## 14. Open items and changes other documents need

> **OPEN:** the developer overlay's input routing, which §9.5's `ui.eat_keyboard` entry
> relies on (ARCH §13, UR Decision 1.6).

> **OPEN:** the licence of the generated annotation file (§10.5).

> **OPEN:** (request to EA1a) the order of a handle's step-2 checks (§10.3). The other
> part of revision 5's request, the field checks of table arguments in the walk after the
> reservation, is answered by EA1a revision 5 (§10.3).

> **OPEN:** (request to EM) the region number of `scripttool test`'s synthetic world
> (§13).

The owner's decisions on the budget row and the interim ceilings (TM OPEN O-2) and E9's
measurement (TM OPEN O-1) are TM's open items, not this document's. Every limit this
document uses is TM's (L3–L12, L15, L18, L23–L35, L42, L45, L53–L56, L63, L64, L66, L67,
L69, L75, L82–L86, L90, L91, as TM revision 6 states them);
a change goes through TM §0.1 rule 1. The ranges of single arguments (§3.1 "Domains and
limits") are argument-validity domains under TM §0.1 rule 6 and stay here.

**Changes other documents need** (this document edits no other file):

| Document | Change | State |
|---|---|---|
| TM §10.6 | Register the per-call cap of `Overlay.Lines`: at most 2,048 lines (12,288 numbers) per call, so its cost fits L3 and L4 (§5.3) | **done** (TM L90) |
| TM L90 | Restate the cost of a maximal `Lines` call under EA1a §2.5's counting rule: `B` + e(2*m*), at most 491,720 units, not 245,960 | **done** (TM revision 5); the OPEN of revision 4 is closed (§5.3) |
| TM L82 | Measure the leased eye's 500 m from §4.6's reference point *P* (the placed character, otherwise the user's own eye), not "the player" (TM §0.1 rule 5) | **done** (TM revision 5); §4.6 and §4.11 cite it |
| TM L84, §13 | A full reload's load starts only after the old close item, so it adds no apartment (L26); record that L84 narrows D17 §10's "at most one reload at a time" to hot reloads | **done** (TM revision 5, L84 and its §13 row for D17 §10; TM revision 6 states it as a **reading** of D17 §8 and §10, not a change of an accepted figure); §8.1 and §8.2 state the same |
| TM L84 | A full reload's load also waits until the old cap has left L24's sum at its final flush (as a hot reload's item 6 does), so with the sum full it waits instead of being refused under EM §7.3 | **done** (TM revision 6, L84 and its §13 row "EA1b §8.1, §8.2"); §8.1, §8.2, invariant 15 and a hot-reload test state the same |
| TM L84, §13 | Record the watcher's fixed table of dirty flags as a design narrowing of D17 §10's event channel | **done** (TM revision 6, L84 and its §13 row "D17 §10 watcher"); §8.2 cites it |
| TM §6.8 | The lease end reasons `"region"` and `"focus"` as a stated residual for a `camera`-only script | **done** (TM revision 6, residual (6), and its §13 row "EA1b §4.10"); §4.10 cites it |
| EM §11.5 | State the region number of `scripttool test`'s synthetic world (1–255) | **requested** (the OPEN in §13); the region-change test is a client integration test **(CI)**, so no region-change input is requested |
| TM L27 | Say whose count the old apartment's outstanding requests occupy during and after a hot reload | **done** (TM revision 5: per (kind, id), shared by both apartments, until each result is discarded and its job has ended); §8.5 follows |
| EA1a OPEN-12(a) | Refuse region 0 for a destination or say what 0 means | **answered here** (§3.1, §7.2, revision 5): every argument refuses 0; a result reports 0 ("no region") only for `tessera-net`'s `RegionId(0)`, a sandbox with no world data. EA1a revision 5 still lists the item as open; EA1a can close it (its re-review asks the same, F2) |
| EA1a OPEN-12 (b), (c) | The handle rule cited as EA1a §2.7 | **done** (§4.4, §4.7) |
| EA1a OPEN-7 | Whether the free-fly viewpoint may be reported under `world.read` | **answered here** (§4.2): `Camera` reports the free-fly eye under `world.read`, so `World.Here` may report it in fly mode under the same grant. EA1a may close the item |
| EA1a §2.4 step 3 (and step 6) | Field checks of table arguments in the walk after the reservation | **done** (EA1a revision 5, §2.4 steps 3 and 6, OPEN-12(f)); §10.3 follows |
| EA1a §2.4 step 2, §2.7 | A handle's step-2 checks in the order minting capability, kind, staleness, for every handle argument, `self` and handles inside tables at step 6 (so the minting rule wins for a stale handle whose minting capability is gone) | **requested** (the OPEN in §10.3) |
| TM §10.6 | Register the REPL result description's 32 entries and the overlay drop record's interval of 10 s | **done** (TM L91) |
| TM §11 slice column | X27 and X85 tagged for their slice-1b parts; X44's `Files.Pick` part in slice 2 | **done** (TM revision 4: X27, X85 and X16 "1a, 1b", X44 "1b, 2"); §13 lists the parts |
| TM L75, X80 | Teleports numbered per `Session` connection (reset when `Connect` is submitted); the slot released in the frame a switch or session end is queued and at a failed submission (number not consumed); an all-scripts rate for clock changes | **done** (TM revision 4); §7.4 states the same moments |
| TM §8.17, L63, L84, rule 7.1.4, rule 7.6.1 | The "unload" end at the request; no lease, intent or `Sandbox` command from an `Unload` handler; L84 counting hot reloads only; view-dependent overlay statistics only with `world.read`; the file index in the new apartment's cap | **done** (TM revision 4); §4.5, §5.3, §8.2, §8.8 follow |
| EA1a §2.4, §2.5 | The order of checks as a general rule; the element-counting rule | **done** (EA1a revision 4); §10.3 applies the order, and every cost of this document counts elements by the rule |
| EM §11.5 | Walk-file keys for the virtual surface size, a reload from a second folder version and a simulated loss of window keyboard focus | **done** in EM revision 2 (`[screen]`, `[[input]]` `reload` and `focus`; TM §13, ranges TM L86); cited in §6.1, §8.8 and §13 |
| EA1a §4.1 | The `"Unload"` reason `"reload"` covers both full and hot reloads; the hand-off table is passed with `Core.SetHandOff` (§8.4), not by the handler's return value | **done** (EA1a §4.1) |
| EA1a §0, §2.5 | The generated reference is `docs/specs/ext-api-reference.md` (§10.4); EA1a and EA1b together are D17's `ext-api.md` | **done** (EA1a §0, §2.5) |
| EA1a §2.10 | One angle form for every API spec: yaw and pitch in radians, `heading` only as a result | **done** (EA1a §2.10); this document follows it (§3.1, §4, §7.2) |
| `tessera-net` (`crates/net`, its model documentation and the sandbox) | The typed teleport-outcome event of §7.5: an additive variant of the non-exhaustive `Event`, emitted beside the existing system line; a test hook that makes `Session::send` fail and one that delays the sandbox's location reports (§13) | for the slice-1b implementation |
| `docs/specs/ext-api-reference.md` | New, generated (§10.4); created when the registry exists | for the slice-1b implementation |

<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First draft (design role), from accepted decision 0017: `Camera` with leases, modes, paths and the lease indicator; `Overlay` with draft and shown lists; the GPU-area bound for ext overlays and ext UI composition with its budget row (D17 open item 1); `Sandbox` with destinations, rate limits, the pending-teleport rule and a typed teleport outcome; hot reload with the watcher, the item sequence, the hand-off table, the read-your-writes rule (D17 open item 6) and the reload cases of a backend switch; the developer console with its log filter (D17 "Other improvements", log sanitising) and REPL; the generators with the binding description, type vocabulary, validation order, reference and annotation outputs and the CI check. Status Review. |
| 2026-10-07 | Revision 2 (Review), answering the independent review and aligning with TM revision 2, EA1a and EM. **Authority:** TM cited as normative for limits, rules and escape cases; EA1a (not the non-existent `ext-api.md`) for general rules; limits cited as TM L-numbers. **§3:** heading in compass degrees, pitch and field of view in degrees, colours as EA1a `#RRGGBB[AA]` strings, costs in EA1a's symbols, handlers as EA1a §2.8; the angle-form and handler-pattern OPENs closed. **§4:** expiry checked at the start of step 3 (D17 §5.4); new `"switch"` end reason at a queued switch; revocation of any grant ends the lease through suspension (TM §8.23); picking uses the presented view (TM §8.17); streaming focus and level of detail follow the user (both OPENs closed); the indicator's frames stated; looped paths' period, cyclic neighbours and heading unwrapping defined; `lease:Kind()`. **§5–§6:** the own a(p) table, 2·S/3·S bounds, composition rule, 1.8 ms row and E4b removed in favour of TM §7.1 (L53–L56, E9 at native 3840 × 2160); `GetStats` in screens; a swap point for scripts without `Tick`; the `crates/render` additions and the sRGB-to-linear conversion stated. **§7:** teleport slot released when the outcome arrives (L75); correlation rule; heading converted with `tessera-net`'s converter. **§8:** TM §8.24's declaration set and full validation first; cap reserved from the allowance before the snapshot, snapshot and hand-off charged inside the new cap (TM §8.10), allowance freed after the old final flush (TM rule 7.6.5); §8.6 replaced by TM §7.6; backend switch during a reload as EM §10.4 and TM §8.21 with the switch timeout; `Unload` is EA1a's `Core.OnUnload` with reason `"reload"` (OPEN closed). **§9:** log file in TM §8.1's format and console display by TM §8.2 (the own filter removed); REPL submissions for suspended scripts refused; result description inside the metered call. **§10:** `dump-api --format` subcommand of EM §11.2; the reference is the generated `ext-api-reference.md`. **§11–§13:** invariants restated (2, 5, 7, 10, 13–16, 19, 21); examples recomputed; tests split into TM escape cases by X-number and this document's functional tests; E4b replaced by E9. **§14:** closed OPENs removed; changes requested of EM §11.5, EA1a §4.1 and `tessera-net` listed. |
| 2026-10-07 | Revision 3 (Review), answering the re-review of revision 2 and aligning with TM revision 3 and EA1a revision 3. **Angles (blocking):** EA1a §2.10's form throughout: poses, `Follow` (its `bearing` option became a `yaw` offset in radians, default pitch −0.35), key frames and `Sandbox` destinations take `yaw` and `pitch` in radians, the field of view in radians, the view direction built from yaw; `heading` appears only in results and is ignored in a passed pose or key frame and refused in a destination; yaw unwrapping in (−π, π]; the `Sandbox` port passes yaw through with `f32` narrowing (no compass converter); the non-existent "EA1a §9 'Heading'" citation removed. **`Overlay.Lines` (blocking):** cost `B` + `e(m)` on the actual length read before any element, at most 2,048 lines per call (≤ 245,960 units, inside TM L3 and L4), validated whole before anything is appended; "building a large draft takes several calls"; the quad and box floods and the functional tests restated over several calls of one turn; invariant 9 extended; the per-call cap's registration requested of TM. **Names:** `Core.On("Tick")` and `Core.On("Unload")` replace `Core.OnTick` and `Core.OnUnload` (§2, §5.2, §8.3, §14). **Teleport slot:** released at the first of the outcome's arrival, the intent's discard and the end of its session generation; outcomes matched by (session generation, request number), late outcomes of an ended generation dropped (TM L75, X80); invariants 13 and 14, the bookmark example and the Sandbox tests updated. **Tests:** the escape list is a table of X-numbers with their slice-1b parts, adding X20 (maximal hand-off), X27 and X28 (hot reload), X87 (a hot reload in progress) and X91's re-read part; retagging of X27 and X85 requested of TM; a console display test. **Walk file:** the `[screen]`, `reload` and `focus` requests marked done (EM §11.5), cited in §6.1, §8.8 and §13. **Smaller:** `/ext:reload` with developer mode off or for an id that is not loaded (§8.2); a grant-store re-read that narrows ends a lease through suspension (TM §8.23); the floor frame counted from the start of the last frame in which ext ran (§12); the `dump-api` spelling against D17 §10 noted (§10.4); §14's changes table gains a state column. |
| 2026-10-07 | Revision 4 (Review), answering the re-review of revision 3 and the follow-up review of the same day, and aligning with TM revision 4 and EA1a revision 4. **`Follow` and `entity.read` (blocking, §4.4, §4.7):** an entity target raises `"not granted"` unless `entity.read` is in the effective set at the call (EA1a §2.4 step 2 for the handle), step 3 re-checks it at every apply; the §4.7 citation of EA1a §2.9 corrected (kept handles stay objects that raise); X91's slice-1b part and a functional test added. **Teleport correlation (blocking, §7.4):** requests and outcomes numbered per `Session` connection, reset when `Connect` is submitted; the slot released in the frame the generation rises (a switch or session end queued), as TM L75 and X80 now say; the impossible "teleport submitted while a switch is queued" sentence removed. **Submission (blocking, §7.2, §7.4, §7.5):** apply-time checks include "not unloading" and `InWorld` (a teleport that finds the session out of the world is `"cancelled"`); a failed `Session::send` releases the slot, consumes no request number and answers `TeleportDone{ok = false, reason = "failed"}`; `place` validated on the text after U+FFFD replacement, its raw length checked first. **`Unload` handlers (blocking, §3.2, §4.3, §4.5, §5.2, §8.3):** the "unload" end, the discard of intents and the clearing of overlay lists in the frame the unload is requested; while unloading (its handler included) `Camera.Take` answers `"taken"`, lease intents `"ended"`, `Overlay` adds and `Sandbox` commands the new reason `"unloading"`, intents queued anyway are discarded at return and never applied. **Capability separation (§4.2, §5.3, §7.5):** `GetStats`' view-dependent fields (`drawn`, `culled`, `dropped_coverage`, `coverage`) and every player position (`Camera` poses, `TeleportDone`, `Location`) only with `world.read`. **EM §9.8 (§8.2, §8.7):** the notices "No loaded extended script '⟨id⟩'" and "⟨id⟩ is suspended: resume it first"; a suspended script is not reloaded by the command or the watcher. **Register (§3.1, §14):** argument-validity domains named under TM §0.1 rule 6; L90 and L91 cited; TM's answers recorded as done; X16 and X44's parts in §13. **Generators (§10):** the order of checks is EA1a §2.4's, every walk of table fields or elements after the charge or reservation, slice-1b soft answers at step 4; the vocabulary gains `DID`, `table`, `any`, `record`, `forms`, `union`, `variadic`, the root and fields; the registry's canonical form defined (RFC 8785 JSON, ordering, members) and the fixture comparison stated. **Hot reload (§8.2, §8.3, §8.7, §8.8):** L84 counts hot reloads only, full reloads never wait; the watcher keeps one flag per loaded developer script instead of an event channel; rows for a user unload, `Core.Unload`, developer mode turned off and client exit during a reload, and for waiting requests; the file index counts toward the new apartment's cap (TM rule 7.6.1). **No character (§4.6, §4.7, invariant 6):** *P* is the user's eye, and `Follow("player")` shows the user's view, while no character is placed. **§6.4:** *n* counts non-empty shown lists, drawn or not, as TM rule 7.1.4. **Costs (§4.2–§4.4, §5.3, §7.3):** elements counted by EA1a §2.5's rule (poses `e(18)`, `Hold` Rsv `e(20)`, `Follow` Rsv `e(12)`, `Play` Rsv `e(6)` + `e(24 · 256)`, `Teleport` Rsv `e(26)` + `b(256)`, `GetStats` `e(14)`, `Lines` `B` + `e(2m)`), with an OPEN to TM on L90's cost figure. **Smaller:** the order of `"busy"` and `"pending"` (§7.3); the REPL compile charge `B` + 4 units per byte per attempt (§9.4); the all-scripts clock rate of 4 per second (TM L75); `TeleportDone` only when a handler is set (§7.5, invariant 14); the anchor rule stated (§7.2); the quad-flood example adds 2,001 in its fifth timer; the `Sandbox` tests run in the client integration harness (§13); invariants 2, 6, 13–15, 17 and 23 restated. |
| 2026-10-07 | Revision 5 (Review), answering the re-review of revision 4 and aligning with TM revision 5. **`union` validation (blocking, §4.4, §10.2, §10.3, §13, TM X91):** an alternative is selected by its discriminator (its Lua type, and a handle's kind), never by trying alternatives in turn, and the selected alternative's checks then decide in full; a host handle's step-2 checks run in the order minting capability, kind, staleness (EA1a §2.7), so a kept entity handle after an `entity.read` revocation raises `"not granted"` whether live or stale, a stale one with `entity.read` held raises as stale, and "matches no alternative" is left for a value no alternative's type accepts; invariant 23 and the X91 row and leases tests extended. **Region 0 (blocking, §2, §3.1, §7.2; EA1a OPEN-12(a)):** every argument takes `region` 1–255 and refuses 0 ("no region", `coordinates.md` §4); a result reports 0 only for `tessera-net`'s `RegionId(0)`, a sandbox with no world data; the tests' synthetic worlds use 1–255; new invariant 24 and a destination test. **EA1a requests:** an OPEN in §10.3 (and §14) asking EA1a §2.4 to run the field checks of table arguments in the walk after the reservation and to order a handle's step-2 checks; EA1a OPEN-7 answered (§4.2: the free-fly eye is reported under `world.read`, so `World.Here` may report it); the handle rule cited as EA1a §2.7 (§4.4, §4.7). **TM revision 5:** the L90 OPEN closed (TM states `B` + e(2*m*), 491,720 units; §5.3, §14); L82's reference point *P*, L84's full-reload timing and its D17 §10 narrowing, and L27 per (kind, id) recorded as done in §14. **Rates (§7.4):** sliding one-second windows over accepted calls on the lease clock, (*t* − 1 s, *t*], per (kind, id) and for all scripts; refused calls not recorded; the five-script test fixed to one frame. **Hot reload (§8.1, §8.2, §8.5, §8.7):** a full reload's load (EM phase A) starts only after the old close item, so it adds no apartment (TM L26, L84); `/ext:reload` outside `Open` queues nothing and shows "No loaded extended script '⟨id⟩'" (EM §10.5), its cases checked in order; waiting requests dropped and watcher flags cleared when the gate leaves `Open`, no flag queued until it is `Open` again; a required grant revoked before item 4 cancels the reload and unloads the script; the old apartment's outstanding requests count in the script's one L27 count, shared by both apartments, until each has ended. **Smaller:** `culled` names a filled box with the eye inside (§5.3, §6.2); the REPL's largest compile charge is 65,964 units, not 66,392 (§9.4); invariants 13, 15, 23 and 24; tests for all of the above. |
| 2026-10-07 | Revision 6 (Accepted), answering the follow-ups of the re-review of revision 5 and aligning with TM revision 6. **Hot reload (§8.1, §8.2, §8.7, invariant 15):** a full reload's load starts only after the old close item has run **and** the old cap has left the 512 MiB sum at its final flush, waiting for it and never refused for the old copy's memory, as TM L84 now says; the watcher's fixed flag table cited as the design narrowing of D17 §10 that TM L84 and TM §13 record; a watchdog suspension caused by the old `Unload` handler in item 4 falls under the "between items 4 and 5" row; developer mode turned off cancels a full reload's load in progress or waiting as EM §9.8 does (TM §8.23). **EA1a (§10.3, §14):** the field-check part of revision 5's request is done (EA1a revision 5, §2.4 steps 3 and 6, OPEN-12(f)) and the stale sentence about EA1a step 3 removed; the order of a handle's step-2 checks stays an OPEN; the EA1a OPEN-12(a) row notes that EA1a still lists the answered item. **Tests (§13, §14):** the region-change lease test is **(CI)**; an OPEN asks EM §11.5 to state the synthetic world's region number; a test of a full reload waiting at a full sum. **Generators (§10.2, §10.4, §10.5):** canonical cost terms gain a factor `k` and named measures (`length`, `bytes`, `elements` of a parameter or of the stored value), with `Lines`, `TakeHandOff` and `Teleport` as examples; the reference starts with the root (`Ext` and `print`); a required parameter may take `nil` as a `union` alternative. **Smaller:** the pending-query flag of `QueryLocation` (§7.3, §7.5, invariant 14); `anchor` is a record, not an array (§7.2, §7.3); the lease end reasons cite TM §6.8 residual (6) (§4.10); `Module.On` cited as EA1a §2.8 form 1 (§3.3); `SetFov`'s `fov` a required number or `nil` (§4.4); REPL submissions refused at once while the gate is not `Open` (§9.4); §14's list of limits adds L15, L18, L42, L64 and L66, and its changes table records TM revision 6's answers. |
