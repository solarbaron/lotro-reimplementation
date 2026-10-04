<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Observations only the project owner can make

| | |
|---|---|
| **Status** | Open list; started 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Who fills it in** | The project owner, in the original client, on their own account and install |
| **Method** | Black-box observation of the unmodified official client: its own UI, `/loc`, and frame captures (RenderDoc or apitrace under Wine/DXVK). Allowed by `docs/CLEAN_ROOM.md` §4 (method M3) and [ROADMAP §2.1](../ROADMAP.md#method-allowed). |

Some facts cannot be read from the data files: they are decided by the original client's
code at run time. This file lists every such question that a spec currently leaves OPEN,
with what to do and what to record. When an item is answered, the spec writer moves the
fact into the spec (with this file as its source, tagged "owner observation, date") and
marks the item **done** here.

<a id="rules"></a>
## Rules for recording

- **No game text in the repository.** Do not write area, item, NPC or server names, quest
  text or chat output here. Describe things in your own words or by DID. Keep anything
  else in a private note outside the repository.
- **No captures or screenshots in the repository.** Frame captures and screenshots stay in
  the private dirty-room scratch area; only the facts read from them are written here
  (numbers, states, yes/no).
- Record for every observation: the date, the client build (as the launcher or the client's
  own about box shows it, numbers only), graphics settings that matter (detail levels,
  DirectX version), and whether the client ran under Wine/Proton + DXVK.
- Where an item has a Tessera command to show our current output, run it first so you know
  what to compare.

<a id="summary"></a>
## Summary

| ID | Question | Spec | Needs | Status |
|---|---|---|---|---|
| [O1](#o1) | `/loc`, map coordinates and area at 20 reference locations | `docs/checklists/m3-reference-locations.md` | In-game `/loc` | open |
| [O2](#o2) | Compass bearing of a placed object versus `/loc` heading | `entity-descriptor.md` §4.4 | In-game `/loc` | open |
| [O3](#o3) | Which content layer the world shows by default | `entity-descriptor.md` §4.5.1 | In-game look | open (spot to be supplied) |
| [O4](#o4) | Are one-triangle placeholder parts drawn? | `setup.md` §8.5 | Look, or frame capture | open |
| [O5](#o5) | Exact dye blend | `material.md` §12.3 | Look, or frame capture | open |
| [O6](#o6) | Other material and render-state questions | `material.md` §17 and §19 | Frame capture | open |
| [O7](#o7) | Command-line option leniency | `install-layout.md` §10 | Launch test | open, low priority |

---

<a id="o1"></a>
## O1. Twenty reference locations (M3 exit criteria 4 and 5)

**Spec:** [`m3-reference-locations.md`](m3-reference-locations.md) §2.2 (table with empty
columns) and §4.2 (procedure).

**What to do:** for each row you can reach, stand on open ground just outside the listed
corner of the anchor object, type `/loc`, and fill in the three empty columns as §4.2 says:
the numeric `/loc` fields and the map-coordinate string under the minimap; `= walker` /
`≠ walker` / `seen` for the area name (never the name itself); pass, fail or `n/a`.

**Why:** these readings check the height scale against `oz`, the `/loc` landcell and
offset convention, the map-coordinate formula and its rounding, the formula's use outside
region 1, and the block-map area assignment (that checklist's §3).

<a id="o2"></a>
## O2. Bearing of placed objects versus the `/loc` heading

**Spec:** `entity-descriptor.md` §4.4: an object's local +Y axis is derived to point at
compass bearing (360° − ψ) mod 360°, measured clockwise from north. Not yet checked against
the original.

**What to do:** at three long buildings from the O1 table, walk along the long wall and
stop with the character facing exactly along it (back or shoulder against the wall helps).
Type `/loc` and record `h`. Do it once in each direction if convenient.

| Row | Long wall runs along the object's | ψ (°) | Expected `h` if §4.4 is right | `h` if the turn direction were reversed | Observed `h` |
|---|---|---|---|---|---|
| M3-06 | local *y* (38.4 m side) | 307.3 | 52.7 or 232.7 | 127.3 or 307.3 | |
| M3-10 | local *y* (30.6 m side) | 75.0 | 285.0 or 105.0 | 255.0 or 75.0 | |
| M3-14 | local *y* (27.6 m side) | 225.0 | 135.0 or 315.0 | 45.0 or 225.0 | |

**Record:** the `h` values (±3° is enough). `/loc` omits `h` within about 1.3° of north
(research note); write "omitted" then.

<a id="o3"></a>
## O3. Default content layer

**Spec:** `entity-descriptor.md` §4.5.1. Placed objects can carry content-layer lists;
layers 1 and 2 are the two common alternatives. Which one the open world shows (for a
character with no special progress) is unknown. The walker draws objects without layers
plus those listing layer 1, with a debug toggle.

**What to do (once the entity-descriptor spec writer has added a spot here):** go to the
spot, note in your own words which of the two object sets is present (the walker's toggle
shows both sets), and whether it differs between characters at different stages of the
story.

**Record:** "layer 1 set present" / "layer 2 set present", and the character's level band.

<a id="o4"></a>
## O4. One-triangle placeholder parts

**Spec:** `setup.md` §8.5 (OPEN); found by `m2-visual.md` row C6. 259 mesh records draw a
single triangle; most use the plain white surface 0x310000E5. Tessera's viewer draws them,
so creature 0x79018D38 (a small humanoid creature; setup 0x04000110, part 2 = mesh
0x060068B9) shows a white triangle at its feet.

**First, see ours:** `tessera-explorer --select 0x79018D38 --view mesh --mesh-angle 20,10
--mesh-select 2,0` highlights the triangle.

**What to do:**

1. By eye: find a creature of the same kind in the original, approach it at the highest
   model detail and look at the ground near its feet from several angles. Record
   "triangle visible" or "not visible".
2. Frame capture (settles it): capture a frame with that creature in view. Look for a draw
   of 3 indices whose vertex positions match a single small triangle near the creature's
   feet, with a plain white texture. Record whether such a draw exists; if it does, its
   blend state, depth write, colour write mask and the alpha of the material colour it
   receives.

<a id="o5"></a>
## O5. Dye blend

**Spec:** `material.md` §12.3 (OPEN, frame capture): hard threshold on the mask
(alpha < 128 ⇒ multiply) or a smooth blend; linear or gamma space; the role of the dye
colour's alpha (1.0 in most entries, 0.45 in 49); any brightness boost of the grey base.
Tessera uses the smooth form.

**First, see ours:** `tessera-explorer --select 0x79002B7C --view mesh --mesh-choice 1
--dye d02020` (chest armour, red) and `tessera-explorer --select 0x790698CC --view mesh
--mesh-choice 1 --dye 2040d0` (cloak, blue), as in `m2-visual.md` rows A2 and A10.

**What to do:**

1. By eye: dye the same pieces in the original if you have them (otherwise any dyeable
   armour piece) with one strong dye, using the dye preview. Zoom in on the boundary
   between dyed and undyed areas. Record: hard edge or soft gradient; whether mid-tone
   areas of the mask look partly dyed; whether the dyed colour looks brighter or darker
   than ours with the same dye colour.
2. Frame capture (settles it): capture the dyed piece. In the pixel shader's constants
   find the dye colour and record its four values; record whether the shader compares
   the mask alpha against a constant (threshold) or interpolates with it; and whether the
   render target and texture views are sRGB (gamma-space or linear-space multiply).

<a id="o6"></a>
## O6. Other material and render-state questions (frame captures)

**Spec:** `material.md` §17 lists what only a capture can settle; §18 and §19 add the
items raised by the implementation. One capture session can answer several at once. For
each, record the per-draw pipeline state (blend enable and factors, depth test and write,
cull mode, alpha-to-coverage) and, where noted, shader constants.

| # | Question | Spec | What to capture | Record |
|---|---|---|---|---|
| O6.1 | Render-state bytes 4–6; does a lit base pass blend; is any lit surface translucent | §7.6, §11.3 | One lit opaque object, one additive effect (glow, fire), one alpha-blended effect (smoke) | Blend enable, factors, depth write for each draw |
| O6.2 | Is flag 4007 the two-sided switch | §11.4 | A leaf card or cloth edge seen from behind | Cull mode of that draw; whether the back is drawn |
| O6.3 | Alpha test with no opacity texture; alpha-to-coverage or soft edges | §11.2 | Foliage and a grille at close range, with anti-aliasing on | Whether a test happens, on which channel, at what threshold; coverage on/off |
| O6.4 | Opacity of unlit blended effects without opacity in alpha | §18 | One unlit blended effect card | Which texture channel drives its alpha |
| O6.5 | Lighting model: gloss, specular colour 2001, power 1000; environment map; specular power 0 | §10.2, §11.5, §19 | A shiny metal object near a light | Specular term constants; whether power 0 gives no highlight |
| O6.6 | Normal-map green channel (Y up or down) | §10.2 | Any normal-mapped wall lit from the side | Whether the shader flips green |
| O6.7 | Texture matrix: U/V order, composition order, coordinate set per slot | §8.4 | A surface with a rotated or offset detail texture | Texture-coordinate transform constants |
| O6.8 | Animated float curves (modes 2–10) | §5.3 | Scrolling water or a pulsing glow, as a short video or several captures over time | Value over time of the animated constant |
| O6.9 | Vertex colours and diffuse colour 2000 | §19 | Foliage cards and a snow-covered surface | Whether vertex colour or a material tint constant enters the colour |

<a id="o7"></a>
## O7. Command-line option leniency (low priority)

**Spec:** `install-layout.md` §10 (OPEN): whether the original client accepts
`--opt=value` and treats long option names case-insensitively. Tessera's parser accepts
both.

**What to do:** only if a launcher lets you add client arguments easily: start the client
once with a harmless option written as `--option=value`, once with its name in a different
case. Never touch or record login tickets or account options.

**Record:** accepted / ignored / error, for each form.
