<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Observations only the project owner can make

| | |
|---|---|
| **Status** | Open list; started 2026-10-04; last updated 2026-10-10 (O24 extended to the water rows W10–W17 of `m4-5-visual.md` §4.8, with the W13 and W17 questions); 2026-10-09 (merge of main: the M5 items O39–O49 joined the summary next to the 2026-10-08 statuses); 2026-10-08 (every pending item checked against the data and the offline client: O5, O14, O70 answered; O6, O18, O28 narrowed; the rest marked as needing the original) |
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
| [O1](#o1) | `/loc`, map coordinates and area at 20 reference locations | `docs/checklists/m3-reference-locations.md` | In-game `/loc` | partly answered (`coordinates.md` §4, §8–§9, §14: formula, rounding, landcells and block edges from the client); left for the owner (2026-10-08: not in the data or the client): `oz` on flat ground at two or three rows (expected terrain + 0.00 or + 0.05 m), the area-name column, and whether map coordinates show indoors |
| [O2](#o2) | Compass bearing of a placed object versus `/loc` heading | `entity-descriptor.md` §4.4 | In-game `/loc` | answered from the client (`coordinates.md` §11.3–§11.4: h = (360° − yaw) mod 360, bearing of model +Y); optional confirmation |
| [O3](#o3) | Which content layer the world shows by default | `entity-descriptor.md` §4.5.1 | In-game look | answered from the client (`entity-descriptor.md` §4.5.1, §9.3: layers 1 and 2 are both shown, for every kind of placed object including drawn Physics objects; only named layers depend on story progress); optional confirmation spot: region 1, block 0xA15B |
| [O4](#o4) | Are one-triangle placeholder parts drawn? | `setup.md` §8.5 | Look, or frame capture | answered from the client (`setup.md`: never drawn in objects with more than one part); no observation needed |
| [O5](#o5) | Exact dye blend | `material.md` §12.3 | — | **Answered from the client (2026-10-08)**: smooth blend on stored values, no threshold, dye alpha unused, no sRGB (`material.md` §12.3, MA24, MA33); no observation needed |
| [O6](#o6) | Other material and render-state questions | `material.md` §17 and §19 | Look, toggling options | narrowed (2026-10-08): O6.1–O6.9 answered from the client (`material.md` §17, §19, §20.1); left for the owner, without a frame capture: which graphics option turns on the soft edges of cut-out foliage (`material.md` §8.5 OPEN) |
| [O7](#o7) | Command-line option leniency | `install-layout.md` §10 | Launch test | answered from the client (`install-layout.md` §10.1); no observation needed |
| [O8](#o8) | Standing heights per species and sex (whole-body scale) | `appearance.md` §6.1, §17, `skeleton-anim.md` §2.2 | Look, side-by-side screenshots | partly answered (`appearance.md` §6.1, §17); left for the owner: one screenshot of a High-elf female beside a Human female; unchanged 2026-10-08 (whether the template scale enters the render scale was not traced; the one screenshot pair settles it) |
| [O9](#o9) | Height slider and facial bone sliders in character creation | `appearance.md` §6.2, §6.3 | Character-creation screen | partly answered (`appearance.md` §6.2–§6.5, §17: slider rules); left for the owner, as confirmation only: heights at the ends of the height controls, River-hobbit feet on the ground; unchanged 2026-10-08 (confirmation by look only) |
| [O10](#o10) | Worn-item layering and equal-priority ties | `appearance.md` §4.6, §4.7 | Look (wardrobe preview) | partly answered (`appearance.md` §4.7, §17: the client's tie rule); left for the owner: items 1–8, mainly item 5 and whether the order of equipping matters; unchanged 2026-10-08 (item 5 depends on the order the server sends; not in the client) |
| [O11](#o11) | Updated-avatar option and default style files | `appearance.md` §3.3 | Options screen, look | answered from the client (`appearance.md` §3.3, §3.5, §17); optional glance at the options screen to confirm the default |
| [O12](#o12) | Look of random-appearance NPC figures | `appearance.md` §7.4 | Look at two spots, two visits | partly answered (`appearance.md` §7.4, §17: where outfits come from); left for the owner: two visits to one figure (same look or not); narrowed 2026-10-08: the picks come from the client's clock-seeded global generator with no per-object seed, so a different look per visit is expected (`script-runtime.md` §8, R40); the two visits confirm it |
| [O13](#o13) | Where weapons, instruments and class items rest, drawn and stowed | `appearance.md` §5.3–§5.4, `skeleton-anim.md` §8.5 | Look | partly answered (`appearance.md` §5.6, §17: weapons, shields, ranged items); left for the owner: in-hand direction of long weapons (two side screenshots) and whether class items are ever visible; unchanged 2026-10-08 (look) |
| [O14](#o14) | Animation playback: blending, layering, turn direction, speeds, scale inheritance, cloth | `skeleton-anim.md` §5.4, §6.5, §6.6, §7.3, §7.4, §13 | — | **Answered from the client (2026-10-08)**: all rows (`skeleton-anim.md` §15.1; O14.4 walking speeds 2.5 / 2.25 / 2.57 m/s from `collision.md` §11.8); `/loc` timing and visual checks optional |
| [O15](#o15) | Skin, hair and eye colour tinting; face layers | `appearance.md` §3.3, §3.5 | Frame capture | partly answered (`appearance.md` §3.5, §17: one composited face texture, tints through the dye colour; colour byte order B, G, R, A, `property-system.md` §4.7); left for the owner: one face frame capture for the texture slot (low priority); unchanged 2026-10-08 (frame capture, low priority) |
| [O16](#o16) | Which idle, walk and run clips play; idle variants and fidgets | `script-table.md` §8, §15.1, §16 | Look, explorer side by side | partly answered (`script-table.md` §8.6, §15.1: avatars fidget, timing predicted; mood idles answered); left for the owner: the side-by-side clip comparison and about 5 minutes standing still; unchanged 2026-10-08 (side-by-side look) |
| [O17](#o17) | Static NPCs: position, facing and presence at the N spots | `m4-visual.md` §N, `appearance.md` §7, `entity-descriptor.md` §9.4 | In-game `/loc`, look | partly answered (`entity-descriptor.md` §9.4: presence rule, position and facing from the client); left for the owner: whether N4 and N6 are there for your character, and each figure's look; unchanged 2026-10-08 (server state and look) |
| [O18](#o18)–[O27](#o27) | World fidelity: movement, scripted walks, visual rows, clock, draw distances, shadows and lights, water, frills, sky and fog, terrain bands | see [M4.5 items](#m4-5-items) | `/loc`, look, short videos | O18 mostly answered (2026-10-08; only the jump measurement left); O21, O22, O26 partly answered (2026-10-07); O19, O20, O23–O25, O27 need the original (visual) |
| [O28](#o28) | Audio and player music: area-music cross-fade, gaps, falloff, day/night and indoor beds, combat fade, ABC checks, live play, instance limit | `sound.md` §16, `script-runtime.md` §26, `abc-dialect.md` §11 and §17, `instruments.md` §16 | Listening, `/loc`, a recording of the client's output, small original ABC files | narrowed (2026-10-08): O28.3, O28.4, O28.7, O28.9 answered from the data and the client; O28.1–O28.2 reduced to where the music changes; the others need listening (M6) |
| [O39](#o39) | Screenshots of at least 10 native panels in the original client | `ui-layout.md` §17, `m5-visual.md` | Screenshots (private) | open (M5 criterion 2) |
| [O40](#o40) | The 14 plugins of the usage census installed and run in Tessera | `plugin-api-usage.md` §9, `lua-plugin-api.md` | Plugin install, look | open (M5 criterion 4) |
| [O41](#o41) | A published user skin in the original client versus Tessera | `skin-definition.md` | Look (optional) | open (M5 criterion 3, optional) |
| [O42](#o42) | Second right or left press during mouse-look | decision 0014 (Known difference 14) | Look | open (M5) |
| [O43](#o43) | Wheel over a plain plugin window: does the camera zoom? | decision 0014 (D5 OPEN) | Look | open (M5) |
| [O44](#o44) | Right press during an item drag | decision 0014 (Known difference 20) | Look | open (M5) |
| [O45](#o45) | Escape with panels open: what closes, in what order | `game-panels.md` §16 | Look | open (M5) |
| [O46](#o46) | Chat window at UI scales 1.0, 1.5, 0.5; auto-fade look; semi-transparent edge blend | decision 0015 (observations 1–3) | Look, frame capture | open (M5) |
| [O47](#o47) | Owner-written nested-surface test plugin | decision 0015 (observation 4) | Test plugin, screenshot | open (M5) |
| [O48](#o48) | Cursor during and after mouse-look; OS cursor size at scale 1.333; minimap marker edges | decision 0015 (observations 5–7) | Look | open (M5) |
| [O49](#o49) | U1–U5 scripted UI measurement on the idle reference laptop | decision 0015 §6, decision 0010 | Measurement run | open (M5 criterion 7) |
| [O70](#o70) | Player movement speeds and turn rate (test-server movement limits) | `test-server.md` §4.5, §8.3; `tessera-protocol.md` §8.3 | — | **Answered from the client (2026-10-08)**: run 6.5 / 3.25 / 5.0 m/s, walk 2.5 / 2.25 / 2.57 m/s (forward / backward / strafe), keyboard turn 150°/s (`[Input] TurnKeySpeed`), no species difference (`collision.md` §11.8–§11.9) |
| [O71](#o71) | How other players' characters move when watched (interpolation feel) | `tessera-protocol.md` §8.6 | Look, optional private video | open (M7); not answerable from the client (a felt comparison; remote updates belong to the excluded network layer) |
| [O72](#o72) | Distance at which other player characters appear and disappear | `tessera-protocol.md` §9, `test-server.md` §8.2 | Second account or a friend, `/loc` | open (M7); not answerable from the client (the server decides) |
| [O73](#o73) | Range of local "say" chat (optional) | `test-server.md` §4.6, §9 | Two characters, `/loc` | open (M7, optional); not answerable from the client (the server decides) |

M4 exit criteria that wait on these items ([ROADMAP M4 status](../ROADMAP.md#m4-status)):
criterion 2 (bodies play idle, walk and run): O8, O9, O16 (O14 answered from the client on 2026-10-08); criterion 3 (worn and held
items): O10, O13; the sandbox's static NPCs: O12, O17. Criterion 4 (performance) passed
on the reference machine and needs no observation.

M4.5 exit criteria that wait on these items ([ROADMAP M4.5 status](../ROADMAP.md#m4-5-status)):
criterion 4 (walking on collision): O18 (only the jump measurement left since 2026-10-08), O19; criterion 6 (visual checklist): O20 (terrain,
scenery and frill rows), O23 (lighting rows), O24 (water rows), O25 (frills), O26 (sky and
fog rows), O27; the default-settings profile behind criteria 5 and 6: O21, O22.

M5 exit criteria that wait on these items ([ROADMAP M5 status](../ROADMAP.md#m5-status)):
criterion 2 (native panels): O39; criterion 3 (user skin, optional cross-check): O41;
criterion 4 (third-party plugins): O40; criterion 7 (UI budgets): O49. O42–O48 settle OPEN
behaviour of decisions 0014 and 0015 and do not block the criteria.

M6 exit criteria that wait on these items ([ROADMAP M6 status](../ROADMAP.md#m6-status)):
criterion 2 (area music cross-fades at area borders): O28.1 (row O1 of
[`m6-audio.md`](m6-audio.md)). The other O28 rows check facts the M6 specs leave OPEN or
mark as owner observations; criteria 1, 3 and 4 need no observation in the original client
(criterion 4 still needs an idle re-measurement on the reference machine, decision 0010).

M7 items O70–O73 tune defaults of Tessera's own protocol and test server so that movement
feels like the original (O70 answered from the client on 2026-10-08; O71–O73 depend on the
server or on feel and stay with the owner); no M7 exit criterion waits on them ([ROADMAP M7
status](../ROADMAP.md#m7-status)). They are numbered from O70 because other milestone
branches added items at the same time (M4.5 uses O18–O27, M6 uses O28); they keep their
numbers.

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

**2026-10-07 (from the client):** `coordinates.md` §4, §8–§9 and §14 now take from the
client the map-coordinate formula and its per-region constants, the rounding (exact ties
round to even), the letters, the `/loc` landcell and offset rules, the meaning of `r`, and
which block a point exactly on a block edge belongs to. Those columns are now only a
confirmation. Still for the owner:

- **Avatar height above the ground:** the `oz` column of the rows you can reach, on flat open
  ground. The client code that keeps the avatar on the terrain was not traced, so only these
  readings show whether `oz` sits exactly on the terrain or at a fixed distance above it.
- **Area names:** the area-name column (`= walker` / `≠ walker` / `seen`) for every row you
  can reach.
- **Indoors (new):** stand inside one building or dungeon and note whether any panel shows
  your map coordinates there, where it shows them, and what `/loc` prints (numbers and
  layout only, no names). `coordinates.md` §8.3 OPEN.

**2026-10-08 (check against the data and the client):** none of the three columns can be read
from the install or the offline client. The area name and the indoor display are what the
original shows on screen, and the avatar's height above the ground depends on where the
original places the player's capsule, which was not traced (`collision.md` §11.7 OPEN, §16
Q18). To keep the `oz` part short:

- **`oz` (reduced):** at two or three rows on flat open ground, record `oz`. The spec writer
  compares it with the walker's terrain height there; the client's rules allow only about
  0.00 m (capsule bottom at the feet) or about 0.05 m (the terrain's 0.05 m convex radius)
  above the terrain (`collision.md` §11.7 table). The other rows' `oz` are not needed.
- The area-name column and the indoors check stay as above.

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

**2026-10-07 (from the client):** answered for every kind of placed object, now including
the drawn Physics objects (`entity-descriptor.md` §4.5.1, §9.3): objects whose lists name
layer 1 or layer 2 are both shown in the shared world, and objects whose lists name only
named layers are hidden until the server makes such a layer active. No observation is
needed; the spot (region 1, block 0xA15B) stays available as an optional confirmation.

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

**Answered from the client (2026-10-08).** No observation is needed. `material.md` §12.3
(MA24 from the compiled shader programs, BIN-S; MA33 from the graphics back ends, BIN-T (g))
already holds every answer: the blend is smooth, dyed colour = base × (1 − *w*) + base × dye
× *w* with *w* = 1 − mask alpha, applied to the sampled texture values before lighting; there
is no threshold and no brightness boost; the dye colour's alpha is not used; no back end uses
sRGB textures or frame buffers, so the multiply works on the stored values. Steps 1 and 2
remain an optional confirmation.

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

**2026-10-08 (from the client, narrowed).** Rows O6.1–O6.9 are answered in `material.md` from
the compiled shader programs and the client (§17 items 1–8, §19, §20.1; MA22–MA49): blend
and depth state per pass (§7.6, §11.3), two-sided = flag 4003 (§11.4), no alpha test without
an opacity source and discard at or below the threshold (§11.2), opacity of unlit blended
effects (§20.1), lighting and specular (`lighting.md` §5; §8.3, §11.5), normal-map axes
(§10.2), texture matrices (§8.4), float curves (§5.3.1), vertex colours and colour 2000
(§19). The frame captures above are no longer needed. Left for the owner, by look only:

- **Soft foliage edges** (`material.md` §8.5 OPEN): with a tree close in view, switch each
  graphics option that could plausibly affect edges (anti-aliasing, alpha-to-coverage,
  material or texture detail) off and on once, and record which one turns the leaf edges from
  hard to soft.
- **Optional** (`material.md` §11.2 OPEN): whether cut-out foliage on an object that is
  fading in at the draw distance loses its hard edges during the fade.

<a id="o7"></a>
## O7. Command-line option leniency (low priority)

**Spec:** `install-layout.md` §10 (OPEN): whether the original client accepts
`--opt=value` and treats long option names case-insensitively. Tessera's parser accepts
both.

**What to do:** only if a launcher lets you add client arguments easily: start the client
once with a harmless option written as `--option=value`, once with its name in a different
case. Never touch or record login tickets or account options.

**Record:** accepted / ignored / error, for each form.

**2026-10-07 (from the client):** still answered by `install-layout.md` §10.1, which now also
settles which of repeated options wins (the last) and how `--debug` reads its number. No
launch test is needed for O7.

<a id="o8"></a>
## O8. Standing heights per species and sex (M4)

**Spec:** `appearance.md` §6.1 (OPEN), `skeleton-anim.md` §2.2 (OPEN). The data gives two
whole-body scales: the visual template's uniform `vec3` and the avatar entity's
`Render_AdjustableScale`. The River-hobbit entities carry 0.7 (their setups are human-sized),
Beorning entities 1.03, Elf and High-elf female entities 1.02, the Elf male template 1.03,
the High-elf templates 1.05 (male) and 1.03 (female). Tessera multiplies the two. Expected
heights at default sliders: Hobbit about 1.16/1.14 m, Human 1.81/1.77, Elf 1.85/1.80, Dwarf
1.36, Beorning 1.86/1.82, High-elf 1.90/1.84 (female 1.82 if the entity value replaces the
template value), River-hobbit 1.27/1.24 (male/female).

**What to do:** with characters of different species at the default height slider (new
characters are fine, or the character-creation screen), stand two at a time side by side on
flat ground, facing the camera, and take a screenshot each: Human with Elf, Human with
High-elf (both sexes), Human with Beorning, Human with River-hobbit, River-hobbit with
Hobbit. Measure the top of the head of each in pixels.

**Record:** for each pair, the height ratio (to two decimals), and whether a High-elf
female stands about 3 % or about 5 % taller than a Human female.

**2026-10-05 (informal):** the owner looked at characters in Tessera on the laptop and found
their heights fine by eye. No ratios were measured, so the record above is still wanted.

**2026-10-07 (from the client, partly):** `appearance.md` §6.1 and §17 now state that template
`vec3` values multiply along the template chain and that the render scale is a per-axis
product of the entity's `Render_AdjustableScale` and other object factors. Whether the
template `vec3` is one of those factors was not traced. Only that one question is left, and
one screenshot pair settles it:

- **What to do (reduced):** a High-elf female beside a Human female, default height slider,
  side by side on flat ground, facing the camera. Measure the head tops in pixels.
- **Expected:** Human female about 1.77 m. High-elf female about **1.84 m** (ratio about
  1.04) if the scales multiply, about **1.82 m** (ratio about 1.03) if the template value
  does not enter. If a High-elf male and a Human male are at hand, that pair shows the same
  thing more clearly: about 1.90 m against 1.81 m (ratio about 1.05) if they multiply, about
  the same height (ratio about 1.00) if not.
- **Record:** the ratio (to two decimals) for the pair or pairs taken. The other pairs above
  are welcome as a confirmation but no longer needed.

<a id="o9"></a>
## O9. Height slider and facial bone sliders (M4)

**Spec:** `appearance.md` §6.2 and §6.3 (OPEN). The height control drives
`AC_BoneScale_Test` from 0 to 1 (default 0.5); River-hobbits have a second height control.
The 35 facial sliders store one byte each (0x80 default) between stored limits; which byte
gives the limit, and whether the effect is in the bone's own frame, is not in the data.

**What to do** (character-creation screen, no character needs to be saved):

1. Human male: move the height slider fully left, centre, fully right; screenshot each from
   the side at the same zoom. Repeat for a River-hobbit (both height controls if shown).
2. Human male: for the nose-bridge scale slider (the strongest: 0.7 to 1.3) and the eye
   separation slider, screenshot the face from the front at the left end, the centre and the
   right end.
3. Note whether the centre position of a slider looks exactly like the default face.

**Record:** for 1, the head-top height ratio left : centre : right; for 2, whether the left
end is the smaller (narrower) one, and whether eye separation moves both eyes symmetrically;
for 3, yes/no.

4. If time allows, repeat step 1 (and the body sliders the creation screen shows, such as
   build or weight) for one body of every other playable species and sex, and note the
   numbers the UI shows at each end, if any.

**Also record (M4 River-hobbit rows):** whether a River-hobbit's feet rest on the ground at
the default height slider, in idle, walk and run. Tessera's figures float about 0.09 m when
the two hidden leg-scale sliders are applied as `appearance.md` §6.3 decides (rows B37–B42
of [`m4-visual.md`](m4-visual.md), note 1); this observation and the slider frame settle it.

**2026-10-07 (from the client, partly):** the rules are answered (`appearance.md` §6.2–§6.5,
§17). The left end of a slider gives the stored minimum (the smaller value, for example the
narrower nose bridge); eye separation moves both eyes symmetrically; the default value
(0x80) lies a hair off the slider's centre value (0.502 of the range instead of 0.5);
sliders deform the mesh once about each bone's own joint, not the animated pose. The slider
frame question above is settled, and steps 2 and 3 need no observation (they remain
welcome as a confirmation). Still for the owner, as confirmation only:

- **Step 1:** the head-top heights at the left end, centre and right end of the height
  control for a Human male and a River-hobbit (both controls if shown), compared with
  `appearance.md` §6.5.
- **River-hobbit feet:** whether they rest on the ground at the default height slider in
  idle, walk and run (Tessera computes them within 1 cm of the ground in idle; walking is
  the open part).
- Step 4 stays optional.

<a id="o10"></a>
## O10. Worn-item layering and ties (M4)

**Spec:** `appearance.md` §4.4 (normative rule: per body slot the highest part priority
wins), §4.6 (cases) and §4.7 (ties, OPEN). Check items are rows W and L of
[`m4-visual.md`](m4-visual.md); use similar items you own if not those exact ones (the
wardrobe preview or the dressing-room view is enough).

**What to do:** check these situations on a Man or Elf character:

1. A floor-length robe over leggings: are the leggings visible anywhere?
2. A long robe that shows its own shoes, with tall boots, then with low shoes: which
   footwear shows?
3. A long-sleeved chest piece with gloves: are the gloves visible?
4. A hooded cloak with a helmet: hood or helmet? With a very large full helm?
5. Battle gauntlets (or any gloves) with a chest piece that hides gloves: which hands show?
6. A hat that shows "hair under the hat": is it the character's own hairstyle or a generic
   one (compare two characters with different hairstyles)?

7. A helmet with the client's "hide helm" option off, then on: is the hair (and the beard)
   the character's own style when the helmet is hidden?
8. If you have the exact items of rows W1–W22 and L1–L8 of [`m4-visual.md`](m4-visual.md)
   (or close look-alikes), compare each with the explorer's figure
   (`tessera-explorer --character human,m --wear <DID> --clip idle`).

**Record:** one short answer per item (what shows). For 5, which one wins when both are
equipped, and whether the order of equipping matters. For 7, own style / generic / bald. For
8, `same` / `different` per row (what differs, in your own words).

**2026-10-07 (from the client, partly):** `appearance.md` §4.7 and §17 settle the client's
rule: on equal priority the part applied later wins, in a fixed order of records and keys,
and the cases of §4.6 follow from the priorities. The order between **worn items** comes to
the original client from its server and is not in the client, so the owner is still
needed:

- **Item 5** (battle gauntlets or gloves with a chest piece that hides gloves) matters most:
  which hands show, and whether the order of equipping changes it.
- Items 1–4 and 6–8 remain useful as a confirmation of the priority rule.

<a id="o11"></a>
## O11. Updated-avatar option (M4)

**Spec:** `appearance.md` §3.3 (OPEN). Head, hair and beard lists name a plain style record
and a "toggle" record, switched by a boolean property that the avatar data sets to true. The
toggle record holds additional, later styles.

**What to do:** in the client's options, look for a setting that switches between original
and updated character models or appearances (its exact wording does not matter; do not
record it). Note its default state. If convenient, toggle it and see whether existing
characters' heads or hair change.

**Record:** setting present yes/no; default on/off; whether toggling changes faces or hair.

**2026-10-07 (from the client):** answered (`appearance.md` §3.3, §3.5, §17). The setting is a
graphics option, on by default. With it on, the client uses the toggle (newer) style
records and the newer face compositor; turning it off rebuilds the figures with the older
records. Character creation offers the check box only for Hobbit and Dwarf. Nothing is
required; a glance at the options screen to confirm the default (on) is welcome.

<a id="o12"></a>
## O12. Random-appearance NPC figures (M4)

**Spec:** `appearance.md` §7.4 (OPEN). About 43,000 placed NPC figures and most server NPCs
name only a "random appearance type"; no table mapping types to clothes was found in the
data. Tessera draws such figures with their rig's default body. Spot: row N5 of
`m4-visual.md` (landblock 0x80214F79; the figure holds a tool in its right hand).

**First, see ours:** `tessera-client --spawn 1,79,121,151.07,145.72 --heading 0 --pitch -10`
(the sandbox backend, standing 5 m south of the N5 figure and facing it; the walker places
it at the data's position, row N5 of [`m4-visual.md`](m4-visual.md)).

**What to do:** visit the spot (the landblock is in region 1; ask the spec writer for the map
area if needed) and look at the figure; log out and back in (or come back another day) and
look again. Do the same at one more figure of the same kind nearby.

**Record:** "same look both visits" or "different look"; whether the two nearby figures of
the same kind look alike; whether the figure wears more than a plain default outfit (for
example a hat, a cloak, a distinct shirt and trousers).

**2026-10-07 (from the data and the client, partly):** where the outfits come from is answered
(`appearance.md` §7.4, §17): the figures' start-up scripts pick one random piece per group,
and 316 of the 318 placed random-appearance types have outfits. So these figures wear more
than a default outfit. Whether the original picks per object (same look on every visit) or
anew each time was not traced; Tessera keeps one look per figure (seeded from the instance).
Still for the owner:

- **What to do (reduced):** visit one such figure (the N5 figure is fine) twice, logging out
  and back in or on another day.
- **Record:** "same look both visits" or "different look". The second nearby figure and the
  outfit description are now optional.

**2026-10-08 (from the client, narrowed):** the client draws these random picks from its single
global generator, which it seeds once from the wall-clock time at start-up; nothing in the
pick seeds it per figure (`script-runtime.md` §8, R40; `appearance.md` §7.4). So a **different
look on each visit is expected**. A game-logic reseed before the figure's start-up script was
not ruled out, so the two visits above remain the confirmation; nothing else is needed.

<a id="o13"></a>
## O13. Where held items rest (M4)

**Spec:** `appearance.md` §5.3–§5.4 and `skeleton-anim.md` §8.5 (OPEN). The data names
in-hand and stowing locations (hips, back) on every player body but nothing selects between
them; Tessera's interim rule is in `appearance.md` §5.4.

**What to do:** on one character, equip in turn a one-handed sword (and a second one-handed
weapon in the off hand), a two-handed sword, a two-handed axe or hammer, a staff, a spear, a
bow, a crossbow, a shield, a musical instrument, and a class item if your class has one.
For each, note where it rests out of combat and where it is in combat stance.

Also note whether a shield or other off-hand item sits on the forearm or in the hand, and
whether a long weapon held in one hand points forward (along the line of sight) or along
the forearm (Tessera follows `skeleton-anim.md` §8.3: the item's +Y forward; the checklist
asks which is meant, [`m4-visual.md`](m4-visual.md) note 3). Rows H1–H25 of
[`m4-visual.md`](m4-visual.md) list the items and Tessera's locations
(`tessera-explorer --character human,m --wear <DID>[@sheathed] --clip idle`).

**Record:** a two-column table per item: resting place (right hip, left hip, back diagonal,
back straight, in hand, hidden) and combat place (right hand, left hand, forearm, both
hands); and per H row `same` / `different` from Tessera, wielded and stowed.

**2026-10-07 (from the data and the client, partly):** answered for weapons, shields and
ranged items (`appearance.md` §5.6, §17): stowed and wielded locations, offsets, rotations
and per-species scales come from placement scripts by equipment category. Class items and
instruments have no placement script (§5.3); instruments appear while they are played.
Still for the owner:

- **In-hand direction:** one side-view screenshot of a stowed staff and one of a drawn sword
  (does the long weapon point along the line of sight or along the forearm, and does the
  stowed staff lie as Tessera shows it).
- **Class items:** whether a class item is ever visible on the figure (yes/no, and when).
- The per-item table and the H rows above remain useful as a confirmation.

<a id="o14"></a>
## O14. Animation playback (M4)

**Spec:** `skeleton-anim.md` §15 lists six owner observations. One session covers them:

| # | Question | Spec | What to do | Record |
|---|---|---|---|---|
| O14.1 | Blend method and cross-fade time between clips | §6.5; `script-table.md` §8 (0.2 s walk/run cross-fade in the data) | Short video (60 fps) of a character going from standing to walking, walking to running, running to standing | Approximate cross-fade time in frames for idle→walk, walk→run and run→idle (instant, under about 0.3 s, longer); any visible pop |
| O14.2 | Upper-body clips layered over locomotion | §6.6 | Attack or use a skill while running | Do the legs keep running during the attack? |
| O14.3 | Turn direction of turn-in-place clips | §7.3 | Watch a four-legged creature turn on the spot (or an NPC that turns in place) | Which way it ends up facing (left/right of start) |
| O14.4 | Walk, run and backward speeds; foot sliding | §7.4; `script-table.md` §15 (ground speed) | On flat ground, type `/loc`, move straight for a timed 10 s (forward walk, forward run, backward), type `/loc` again; repeat per species if they seem to differ | Metres per second per gait (and per species if different); feet slide yes/no |
| O14.5 | Scale inheritance from parent bones | §5.4 | Watch a creature attack that visibly inflates or shrinks a body part | Do the children (for example a hand on a growing arm) grow too? |
| O14.6 | Runtime cloth on capes and skirts | §13 | Stand still in wind, then turn quickly on the spot | Does the cape swing on its own, or only follow the animation? |
| O14.7 | Turning in place | `script-table.md` §8 (turn branches) | Turn a standing character slowly with the keyboard, then quickly | Does a turn animation play (feet step), or does the body rotate rigidly? |

**2026-10-07 (from the client, partly):** `skeleton-anim.md` §15.1 gives the status per row.

- **Answered, no observation needed:** O14.3 (the turn comes from the movement system, not
  from the clips), O14.5 (children grow too, but their joints stay where the unscaled parent
  puts them), O14.6 (there is no cloth: capes and skirts move only with their clips). The
  clip crop is answered too (`havok-data.md` §2.3).
- **Answered, a visual check is welcome:** O14.1 (linear blending; walk ↔ run is a 0.2 s
  cross-fade; idle ↔ walk and idle ↔ run follow the player's movement ramp, about 0.13 s
  running and 0.25 s walking, so about 8 or 15 frames at 60 fps; no pop), O14.2 (legs keep
  running during an attack while moving; a standing attack moves the whole body), O14.7 (a
  standing turn plays stepping turn clips while the body rotates).
- **Still for the owner, O14.4:** the running speeds are now known from the data (forward
  6.5 m/s, backward 3.25 m/s, strafe 5.0 m/s), but the walking speed depends on an input
  value that was not traced. Do the 10 s `/loc` timing for **walk forward** (walk toggle on),
  **walk backward** and **walk strafe**; run forward, backward and strafe only as a
  confirmation. Note whether the feet slide (the client does not match the clip rate to the
  speed, so some sliding is expected). This is the same measurement as O18 step 1; do it once
  for both.

**Answered from the client (2026-10-08).** O14.4 is answered too, so every row of O14 is
answered and nothing is required (`collision.md` §11.8, BIN; `skeleton-anim.md` §7.4,
§15.1). Walk mode halves the key values, which selects the Walking layer: walking moves at
**2.5 m/s** forward, **2.25 m/s** backward and **2.57 m/s** sideways; running at 6.5, 3.25 and
5.0 m/s (no speed effects active). The walk clips cover about 2 m/s, so the feet slide a
little when walking. The `/loc` timing and the visual checks of O14.1, O14.2 and O14.7 are
optional confirmations.

<a id="o15"></a>
## O15. Skin, hair and eye colours; face layers (M4, frame capture, low priority)

**Spec:** `appearance.md` §3.3 (OPEN, out of scope for M4). Characters store skin, hair, eye,
lip and war-paint colours as RGBA properties, and face details (complexion, eyebrows, mouth,
scars, wrinkles, stubble) as texture-layer indices composited by 0x33 records.

**What to do:** capture one frame of a character's face close up. In the head draw call,
find the shader constants that hold colours, and the textures bound.

**Record:** whether the skin colour is a constant multiplied into the texture or baked into a
generated texture; whether the face texture is one composited image (generated at run time)
or several layered textures; the same for hair (constant tint on a grey texture, or baked).

**2026-10-07 (from the data and the client, partly):** the mechanism is answered
(`appearance.md` §3.5, §17). The face is one texture composited at run time from the face
layers and the character's colours; hair, beard and the skin surfaces are tinted through
their dye colour, set from the hair and skin colours; a second graphics option, on by
default, turns face compositing off. Colours are stored B, G, R, A (`property-system.md`
§4.7). Still for the owner (low priority):

- **What to do (reduced):** the same one frame capture of a face close up.
- **Record:** which texture slot of the face-skin draw holds the composed face image. The
  other questions above are now only a confirmation.

<a id="o16"></a>
## O16. Which idle, walk and run clips play (M4)

**Spec:** `script-table.md` §8 (how the scripts choose clips; §8.3 lists the clips Tessera
resolves per body) and §16 (OPEN: the resolver also reaches random idle fidgets that §8.6
says avatars do not have). [ROADMAP](../ROADMAP.md#m4) M4 criterion 2.

**First, see ours:** `tessera-explorer --character <species>,<m|f> --clip idle` (then
`walk`, `run`); the inspector lists the chosen clip and the other candidates.

**What to do:** for one character of each playable species and sex you can reach (new
characters at the starting area are fine), stand still for about 5 minutes (was "about a
minute"; see the note of 2026-10-07 below), then walk and run in a straight line, and
compare with the explorer's figure of the same species and sex.

**Record:** per species and sex: whether idle shows variants or fidgets (and roughly how
often, for example "every 10–20 s"); for idle, walk and run, `same` / `different` from
Tessera's clip (describe a difference in your own words, never by a name from the game).

**2026-10-07 (from the data and the client, partly):** `script-table.md` §15.1.

- **Fidgets:** avatars do fidget (§8.6). The predicted timing: the first pick comes after
  about 3.6 minutes of standing still (idle, on the ground, out of combat, not emoting); it
  is a fidget or a short pause; further picks follow about every 10 s while the character
  stays idle. One minute is too short to see any, so **stand still for about 5 minutes** and
  note when the first fidget comes and how often they follow. Still open in the spec:
  whether something restarts the long first wait.
- **Mood idles:** answered from the data; no observation needed.
- **Which idle, walk and run clips play:** still for the owner. The data path is complete;
  only the side-by-side look can say `same` or `different`. One character per species and
  sex you can reach, as above.

<a id="o17"></a>
## O17. Static NPCs at the N spots (M4)

**Spec:** rows N1–N7 of [`m4-visual.md`](m4-visual.md) (positions and headings from the
data) and `appearance.md` §7. The walker places these figures exactly where the data
places them (`apps/client/tests/npc_spots_real_data.rs`).

**First, see ours:** `tessera-client --place "<the spot's /loc line>"` (for N4 and N6 add
`--content-layers 1,0x10000019` or `--content-layers 1,0x10000536`).

**What to do:** at each N spot you can reach (ask the spec writer for the map area if
needed), stand next to the figure, type `/loc`, and note the compass direction it faces.
Note whether it is there at all for a character with no special progress (N4 and N6 are on
content layers; see also [O3](#o3)).

**Record:** per row: present yes/no; the `/loc` numbers; the facing as a compass bearing
(±10° is enough); `same` / `different` from the walker's figure (position within about a
metre, facing, pose such as sitting or standing, held item).

**2026-10-05 (informal):** the owner looked at characters in Tessera on the laptop and found
their positions fine by eye. No `/loc` readings or facings were taken, so the record above
is still wanted.

**2026-10-07 (from the client, partly):** presence, position and facing are answered
(`entity-descriptor.md` §9.4). The client places these figures like any other object, at
their stored position and facing, without ground snapping; the expected facings of N1–N7
are in that section's table. N1, N2, N3 and N5 are present in the shared world; N7 is
present but is a hidden marker, so no figure is expected there; N4 and N6 sit on named
content layers and appear only while the server makes their layer active. Still for the
owner:

- **N4 and N6:** whether each figure is there for your character (yes/no, and the
  character's level band). This is server state, not client code.
- **Look:** at each spot you reach, `same` / `different` from the walker's figure for pose,
  dress and held item (governed by `appearance.md` and the animation specs).
- The `/loc` numbers and the facing are now only a confirmation.

---

<a id="m4-5-summary"></a>
<a id="m4-5-items"></a>
## M4.5 items (world fidelity, added 2026-10-06)

The spots, walks and rows these items refer to are in
[`m4-5-visual.md`](m4-5-visual.md). Every item compares the original client at its default
graphics settings (record any setting you changed) with the expected facts listed there.

| ID | Question | Spec | Needs | Status |
|---|---|---|---|---|
| [O18](#o18) | Movement: jump, speeds, highest step, steepest slope, wading and swimming depth | `collision.md` §10–§12 (Q9, Q11), `water.md` §7.4 | Stopwatch or short video | mostly answered from the client (2026-10-08: speeds, highest step, steepest slope, swimming depth, `collision.md` §10.5, §11.3, §11.5–§11.6, §11.8); left for the owner: the jump's height and airtime (predicted 1.2 m, 0.8 s, §11.10) |
| [O19](#o19) | Outcome of the 32 scripted walks in the original | `m4-5-visual.md` §2 | In-game `/loc`, look | open; not answerable from the client (outcome in the original; needs `/loc` and look) |
| [O20](#o20) | Side-by-side look at the terrain, scenery and frill rows | `m4-5-visual.md` §4.1–§4.3 | Look, private screenshots | open; not answerable from the client (visual comparison) |
| [O21](#o21) | Game clock: time of day, day length, epoch | `environment.md` §6, §10.1, §11.1 | Clock in the UI or `/loc`, stopwatch | partly answered (`environment.md` §10.1, §11.1, §11.5: rate, date, night flag, day-type rule); left for the owner: two (in-game time, UTC) readings for the epoch, and the weather on several days; unchanged 2026-10-08 (the epoch is not in the client) |
| [O22](#o22) | Default draw distances: landscape extent, object pop-in, fog clipping | `walker.md` §11.3–§11.4, `mesh.md` §8, `environment.md` §11.7 | Look, `/loc` | partly answered (`environment.md` §10.1, §11.7: fog-distance limits; the distance rules were already from the client); left for the owner: steps 1–6 as a visual check; unchanged 2026-10-08 (visual check) |
| [O23](#o23) | Sun shadows and point lights | `lighting.md` §8–§9, `m4-5-visual.md` §4.6 | Look at a set time, frame capture optional | open; not answered from the client in this pass (shadow colour, cascades and fade stay `lighting.md` §9 OPEN; the rest is visual) |
| [O24](#o24) | Water look: surface, lava, placed water, distant water, puddles cut by the ground (W13), river bed and ripples (W17) | `water.md` §8–§9, `m4-5-visual.md` §4.4, §4.8 | Look, frame capture optional | open; not answerable from the client (visual comparison; reflection, flow and foam stay `water.md` §8 OPEN) |
| [O25](#o25) | Frills: radius, fade, density, sway, terrain tint | `scenery.md` §8.3–§8.4 | Look, short video | open; not answerable from the client (visual; radius, fade and density are already the client's, `scenery.md` §8.3–§8.4) |
| [O26](#o26) | Sky and fog at four times of day; the day type; interior fog | `environment.md` §10.1, §11.2–§12, `lighting.md` §10 | Look at set times | partly answered (`environment.md` §10.1: day-type rule, interior fog rules, sun and moon path); left for the owner: the Y1–Y4 look at set times (after O21), sun and moon size, fog on the sky; unchanged 2026-10-08 (look at set times) |
| [O27](#o27) | Terrain near and far bands, and the seam between them | `terrain-texturing.md` §12 | Look, short video | open; not answerable from the client (visual comparison) |

<a id="o18"></a>
## O18. Movement measurements (M4.5)

**Spec:** `collision.md` §10.3 (movement records), §11 (layering, walkable 45°), §12.3
(Tessera's controller, built where the original's integration is OPEN, Q9 and Q11);
`water.md` §7.4 (Tessera's wade/swim rule, OPEN in the original). Tessera's values for a
Human male: run 6.5 m/s, walk 5 m/s, backward 3.25 m/s, strafe 5 m/s, swim 3.2 m/s; step-up
0.5 m; jump about 0.9 m high and 0.7 s in the air (a Tessera Decision); wading until the
water surface is 1.42 m above the feet, swimming beyond.

**First, see ours:** `tessera-client --walk-path` with walks ST1, SL1 and WA1 of
`m4-5-visual.md` §2.3, and `--spawn 1,161,151,127.81,89.59` (walk WL1's start) for open
ground.

**What to do** (a Human male character, no mount, no speed buffs; flat open ground such as
the start of walk WL1):

1. **Speeds.** Type `/loc`, hold one movement key for a timed 10 s, type `/loc` again; once
   each for run forward, walk forward (walk toggle on), backward, strafe left, and swimming
   forward in deep water (WA1, after the swim point). If a character of another species is
   at hand, repeat run and walk with it and note whether the speeds differ.
2. **Jump.** Jump on flat ground next to a wall with known height (any SED of the walk
   table) and film it from the side at 60 fps; or jump onto ever higher steps until it
   fails. Count the frames from take-off to landing for the airtime.
3. **Highest step.** At ST1–ST3 walk up the platform steps; then find a ledge you cannot
   walk onto without jumping and one you can, and type `/loc` on the ground before each and
   on top (the difference in `oz` is the step height).
4. **Steepest slope.** At SL1–SL3 run straight uphill: note whether the character climbs,
   stops or slides back, and type `/loc` at the highest point reached.
5. **Wading and swimming.** At WA1–WA4 walk slowly into the water and type `/loc` at the
   moment the swimming animation starts.

**Record:** metres per second per gait (one decimal); jump height (to 0.1 m) and airtime
(to 0.1 s); the `oz` values of step 3; for each SL walk the highest `oz` and whether the
character slid; for each WA walk the `/loc` where swimming started (compare with the
`swim=` point of the row; the depth there is the original's swim depth).

**2026-10-07 (note):** step 1 overlaps O14.4. `skeleton-anim.md` §7.4 now takes the running
speeds from the data (they match Tessera's values above); the walking speed depends on an
input value that was not traced, so the walking timings are the ones that matter. Steps
2–5 are unchanged and still open.

**2026-10-08 (from the client, mostly answered).** Steps 1, 3, 4 and 5 are answered from the
client's code and the data; the measurements above are optional confirmations
(`collision.md` §10.5, §11.3, §11.5–§11.6, §11.8):

- **Step 1, speeds:** running 6.5 m/s forward, 3.25 m/s backward, 5.0 m/s sideways; walking
  2.5, 2.25 and 2.57 m/s; swimming 3.2 m/s forward. The same for every playable species
  (only server-set speed effects change them). Tessera's walk of 5 m/s above is not the
  original's.
- **Step 3, highest step:** the client lifts the character by a fixed 0.55 m (0.5502 m) when
  a ledge blocks it and keeps the step only when it lands on walkable ground, so ledges up to
  about 0.55 m are climbed without jumping.
- **Step 4, steepest slope:** players can walk on ground up to 45°; on steeper ground they
  cannot climb and slide down (no control; the Slope layer's gravity of 10 m/s²).
- **Step 5, swimming depth:** the character swims once the water at its feet is deeper than
  its float height: Human, Elf, Beorning, High-elf 1.42 m (male) and 1.40 m (female),
  Hobbit 0.92 m, Dwarf 1.05 m, Stout-axe dwarf 1.12 m, River-hobbit 0.985 m (male) and 0.97 m (female); shallower
  water is wading.

**Still for the owner, step 2 only (reduced):** the jump's push is now known from the client
and the data (6.0 m/s upwards on land, `collision.md` §11.10), which predicts a jump about
**1.2 m** high and about **0.8 s** in the air; the take-off itself was not traced end to end.
Jump once on flat ground beside something of known height and film it from the side (or count
frames); record the height (to 0.1 m) and the airtime (to 0.1 s).

<a id="o19"></a>
## O19. Scripted walks in the original (M4.5)

**Spec:** `m4-5-visual.md` §2.3 (32 walks with expected outcomes) and §2.2 (pass
conditions).

**First, see ours:** `tessera-client --walk-path <id>` for any row (the table form of the
checklist is accepted directly).

**What to do:** for every walk you can reach (`n/a` otherwise; interiors IN1–IN4 lie in
instanced spaces), go to the start (`loc` column), face the heading, and do what the
`inputs` column says by hand (hold forward, or walk to the listed end point). Type `/loc`
at the end.

**Record:** per walk: the final `/loc` numbers (no server name, no time stamp); `blocked`
/ `reached` / `slid back` / `wading then swimming` in your own words; for WL, UB and GN
whether the character stopped at the object (and roughly how far from it, that is how
deep the body seems to sink into the wall or how wide a gap it keeps); for UB whether
anything is visible there; for GN whether the tree or rock is there at all (it is
generated from the data, so its absence would mean a generation difference).

<a id="o20"></a>
## O20. Terrain, scenery and frill rows side by side (M4.5)

**Spec:** `m4-5-visual.md` §4.1 (terrain layers, T1–T16), §4.2 (generated scenery,
S1–S11), §4.3 (frills, F1–F6).

**First, see ours:** `tessera-client --screenshot ours.png --spawn R,X,Y,x,y --heading H
--pitch P --fly` with each row's eye; the explorer's terrain and scenery views list the
layer stack and the generated objects.

**What to do:** stand at each row's eye position in the original (time of day around
noon), look in the row's direction, and compare with Tessera's screenshot of the same
row. For S rows count the trees and rocks in view; for T rows look at the ground texture
under the target (roads, snow, paving, normal-mapped relief, highlights); for F rows look
at the grass and ground cover near the target.

**Record:** per row `same` / `different` (pass / fail); a difference in your own words (for
example "road edge softer", "extra rock at the left", "grass sparser"). Screenshots stay
private. The other rows of `m4-5-visual.md` are compared the same way under O24 (water,
W1–W9), O26 (sky and fog, Y1–Y5) and O23 (lighting, L1–L7), so that together these items
give a pass or fail for every row of criterion 6.

<a id="o21"></a>
## O21. Game clock and epoch (M4.5)

**Spec:** `environment.md` §6 (day length 11,160 s, 360 days per year, zero offset
3,600 s, zero year 34) and §11.1 (the clock); the epoch of the clock is OPEN, so Tessera's
clock has a settable start. The `/loc` reply names the game time (game text; record only
numbers).

**What to do:**

1. Note the in-game time the client shows (clock or the `/loc` sentence, or the sun's
   position if nothing else shows it) and the real date and time (UTC), twice, at least an
   hour apart.
2. Note the in-game date if the client shows one (day of the month, month number, year
   number, as numbers only).
3. If possible, note the real time at which dusk visibly starts (the light turns orange)
   and at which night starts.
4. Day type: at one fixed spot (row Y1), note the weather (clear, overcast, rain, fog) on
   several in-game days, together with the in-game date of each (see also O26).

**Record:** the pairs (in-game time, UTC) with the date; the in-game date numbers. From
two pairs the spec writer derives the rate (expected: one game day per 11,160 s, 3.1 h) and
the epoch.

**2026-10-07 (from the client, partly):** `environment.md` §10.1, §11.1 and §11.5.

- **Answered:** the rate (one game day per 11,160 real seconds at the default rate), the
  360-day year of 12 thirty-day months and the date formulas. There is no single dusk event:
  light and fog follow the time-of-day keyframes, and the night flag switches on at time of
  day 0.8 and off at 0.25. The day type is computed by the client at each game-day change
  by a fixed rule (§11.5).
- **Not in the client:** the epoch. The client's clock has no wall-clock start of its own;
  it is set from outside the game-time code (probably the connection layer, which this
  project does not analyse). So the owner is still needed.
- **What to do (reduced):** step 1, two (in-game time, UTC) readings at least an hour apart,
  with the real date. Step 2 (the in-game date numbers) helps check the year and month.
  Step 3 is now optional: once the epoch is known, the night times follow from it. Step 4
  (the weather at row Y1 on several in-game days, with each in-game date) checks the
  day-type rule against the live game, which also depends on the server; do it if
  convenient, together with O26.

<a id="o22"></a>
## O22. Default draw distances (M4.5)

**Spec:** `walker.md` §11.3 (landscape extent 2,080 m at the built-in High setting; the
global limit *G* is the fog far distance when that is shorter), §11.4 (object distances by
LOD class, 1.5 s fades), §11.8 (default profile).

**What to do** (default graphics settings; record them):

1. At perf spot P5 (`m4-5-visual.md` §3), look along heading 190: is the sea visible out to
   the horizon, and where does the landscape end?
2. Walk slowly toward a lone building from 400 m away (any WL row's building) and note the
   distance (from `/loc`) at which it appears, and whether it fades in or pops.
3. Do the same with a small object (a barrel or crate) and with a tree.
4. At night in the town of row L2 (fog far 300 m in the data), look along a long street:
   do buildings beyond about 300 m disappear?
5. Do the same as in 2 with another character (an NPC or another player): the distance at
   which it appears and finishes fading in.
6. How far grass and other frills reach and how dense they look (details under O25).

**Record:** the graphics preset in use; distances in metres (to 10 m); fade or pop; for 4,
yes/no and the distance; the farthest visible landscape (from 1).

**2026-10-07 (from the client, partly):** the distance rules were already from the client
(`walker.md` §11.3–§11.4, `mesh.md` §8.5–§8.7); `environment.md` §10.1 and §11.7 now add
that the outdoor fog far distance, which can shorten every draw distance, never exceeds
10,000 and never falls below 20 (in the same distance units as the 300 m of step 4). So steps 1–6 now check Tessera's picture against
predicted values rather than discover them: do them as described, as a visual check.
Step 4 (buildings vanishing at the fog distance at night) is the one this revision
touches.

<a id="o23"></a>
## O23. Sun shadows and point lights (M4.5)

**Spec:** `lighting.md` §8.1 (sun from the day description), §8.3 (point lights and their
attenuation), §9 (sun shadows; colour, cascades and fade OPEN); `m4-5-visual.md` §4.6.

**What to do:**

1. Row L1 around noon: note the direction the barn's shadow points (compass) and roughly
   how far it reaches from the wall; the shadow's colour (black, blue-grey, tinted);
   whether the shadow edge is sharp or soft; at what distance from the camera shadows stop
   being drawn, and whether they fade out or end at a line.
2. Row L2 at night: are the lamps lit; do they light the walls and the ground around them;
   roughly how far does the light reach (the data says range 10 m); is the colour orange?
3. Rows L3 and L4: do the fire and the crystals glow (bright regardless of the darkness,
   with a halo or bloom)?
4. Optional frame capture at L1: the shadow-map size and cascade count, and the shadow
   colour constants (`lighting.md` §9 OPEN).

**Record:** compass bearing (±10°) and length (±5 m) of the shadow; sharp/soft; shadow
distance; per lamp question yes/no and the reach; glow yes/no and halo yes/no; capture
values if taken.

<a id="o24"></a>
## O24. Water look (M4.5)

**Spec:** `water.md` §8 (drawing water; reflection, flow, foam and depth OPEN), §9
(distant water), §7.3 (placed water); `m4-5-visual.md` §4.4 rows W1–W9.

**What to do:** at each W row (daytime), look at the water and note: colour and
transparency (can you see the bed?), reflections (sky, shore), waves or flow direction,
foam at the shore, and any edge where two water heights meet (W6). At W7 note whether lava
glows and moves. At W8 note whether the pool surface is see-through. At W9 note whether
distant water looks like the near water. Optionally dive under at W4 and note the
under-water look (fog colour, distance).

**Record:** per row a few words per aspect; `n/a` where not reachable.

**Added 2026-10-10 (spec writer; `m4-5-visual.md` §4.8 rows W10–W17, `water.md` §8.5–§8.7):**
at each row's `/loc`, note what the rows' "Expected" column asks (waterfall streaming and soft
edges, fountain water, flow speeds). Two answers settle open points:

- **W13 (puddle):** is the puddle cut by the ground (its outline following the terrain where
  the ground rises above the water patch, as Tessera draws it), or does it show a rounded soft
  outline all round, as if drawn on top of the ground? The data predict the first
  (`water.md` §8.5.4); a different look would mean the original raises, biases or projects
  puddles in a way the data do not show. Note the graphics settings (reflections on or off).
- **W17 (river):** do moving ripples show (`water.md` §8.6.3 OPEN), and is the river bed
  visible through the water, or is the water dark at a few tens of metres? Note whether
  water reflections are enabled and which DirectX version the client runs.

<a id="o25"></a>
## O25. Frills (M4.5)

**Spec:** `scenery.md` §8.3–§8.4 (frill radius 97 m at the default level 2, fade from
67 m, reduction from 40 m, fade-in 1.5 s, terrain tint; shader formulas OPEN);
`m4-5-visual.md` §4.3 and walks FR1–FR3.

**What to do:** at rows F2 and F4 (dense grass), stand still and look along the ground:
estimate where grass ends and whether it fades out or shrinks; walk forward and watch new
grass appear (fade or pop); watch it in wind (does it sway, how much). Compare the colour
of grass with the ground under it (tinted toward the ground?). At walk FR1 note whether
the character walks through the ground cover without stopping.

**Record:** end distance (to 10 m), fade/shrink/pop, sway yes/no and strength in your own
words, tint yes/no; FR walks pass/fail.

<a id="o26"></a>
## O26. Sky and fog; day type; interior fog (M4.5)

**Spec:** `environment.md` §11.2 (the day type *D* is OPEN; Tessera starts with 2, which
selects only the local default table, `m4-5-visual.md` §1.1 item 3), §11.3 (blending),
§12 (sky); `lighting.md` §10 (fog); rows Y1–Y5.

**What to do:** at rows Y1–Y4 (or any spot with the same sky key) at the four times of day
of the rows (night 0.0, dawn 0.25, noon 0.5, dusk 0.78; use O21 to know when), look at the
sky, the sun or moon (its path and apparent size), clouds and stars, the fog colour and the
fog distance (where distant hills fade out). Do this under at least three sky keys
(regions). On two different real days note whether the weather at Y1 differs (rain,
overcast). At Y5 (IN1's dungeon) and in one cave, note the fog colour, its density and
the distance at which walls fade.

**Record:** per row: sky colour and fog colour in your own words, fog distance (to 50 m),
sun/moon position (compass and height above the horizon); weather per day; interior fog
distance and colour.

**2026-10-07 (from the client, partly):** `environment.md` §10.1.

- **Day type:** answered as a rule (§11.5): the client computes it at each game-day change.
  The "Tessera starts with 2" above was the default sky key, not a day type (§11.2). The
  weather on a given real day still depends on the epoch and on the server, so the weather
  at Y1 on several days (shared with O21 step 4) remains a useful check.
- **Interior fog (Y5 and a cave):** answered as rules (§7.2, §11.7); the colours and
  distances are data. Optional confirmation only.
- **Sun and moon path:** the rotation is answered (§11.4); their **apparent size** is still
  open, so note it (for example compared with a nearby object or the screen width).
- **Fog on the sky:** in dense fog the sky objects are not drawn at all; how lighter fog
  tints the sky is open. Note whether the sky shows through heavy fog and whether it looks
  tinted in light fog.
- **Still for the owner:** the Y1–Y4 look at the four times of day, as described above.
  Do O21 step 1 first: the real time at which to stand there follows from the epoch.

<a id="o27"></a>
## O27. Terrain near and far bands (M4.5)

**Spec:** `terrain-texturing.md` §12 (distance bands and the near/far seam OPEN; Reported:
full detail to about 14 blocks, medium to about 24, then distant imposters).

**What to do:** at perf spot P5 and at row T3 (a high cliff top), look over a wide
landscape: note the distance at which ground textures lose detail or change colour, whether
a line or band is visible where they change, and whether it moves with the camera. A short
video while walking forward helps. At rows T1–T16 also note how sharp road edges are,
whether wet or rocky ground shows a specular sheen, and whether the landscape itself casts
shadows onto the terrain (hills shading valleys).

**Record:** distances (to 100 m) of each visible change; visible seam yes/no; moves with the
camera yes/no; road edge sharp/soft; sheen yes/no per row; landscape shadow yes/no.

---

<a id="m6-items"></a>
<a id="o28"></a>
## O28. Audio and player music (M6)

**Specs:** `sound.md` §16 (S-1 to S-5), `script-runtime.md` §26 (RT-1 to RT-5),
`abc-dialect.md` §11 (OPEN on the commands) and §17 (OC1 to OC10), `instruments.md` §16
(Q1, Q2, Q4, Q5, Q6). [ROADMAP](../ROADMAP.md#m6) M6 criterion 2 waits on O28.1.

**First, hear ours:** device output is opt-in (decision 0019):
`cargo run --release -p tessera-client --features audio-device -- --audio device --spawn
<region>,<block x>,<block y>,<x>,<y>` (the walker's HUD shows the audio line). Player music:
put the test file in the settings folder's `Music` folder and add
`--music-command "/play <name>"` (repeatable; `/playstart`, `/playstop`, `/playlist`).

**What to do:** in the original client, at places you choose (record them by `/loc` only,
never by name), make the observations below. For ABC checks write each small test file
yourself (original tunes only, as in the table of `abc-dialect.md` §17) and put it in the
original client's `Music` folder. **Do not try digit runs of 128 characters or more** in an
ABC file (`abc-dialect.md` §4.1: undefined behaviour in the original).

| # | Question | Spec | What to do | Record |
|---|---|---|---|---|
| O28.1 | Area-border music change (S-1, RT-1; M6 criterion 2) | `sound.md` §10.5, `script-runtime.md` §6.6, §24.1 | Walk across an area border with music playing | Does the old track fade out over roughly 1–4 s while the new area's music starts at once (or a silence begins)? Are both audible for a moment? The border by `/loc` and the approximate fade length |
| O28.2 | Crossing back and forth (RT-2) | `script-runtime.md` §6.6 | Cross a border and walk straight back within one track | Does the music restart (the track beginning again) each time? |
| O28.3 | Silence between tracks (S-2) | `sound.md` §10.4 | Stand in one area for several tracks with a stopwatch | Gap lengths in seconds (expected 5–30 s) |
| O28.4 | Positional falloff (S-3) | `sound.md` §6.2, §11 | Walk away from a positional emitter (waterfall, fire, torch), typing `/loc` at the emitter and where it falls silent | The distance in metres; whether loudness falls smoothly or steps |
| O28.5 | Day/night and indoor variants (S-4) | `sound.md` §10.3.5, §10.7 | At one place compare the ambient bed and the reverb at day and at night, and outdoors versus inside a building or cave | Does each change? When: on entering, or at a time of day? |
| O28.6 | Ogg played range (S-5) | `sound.md` §3.3.2 | Record the client's output while a short one-shot plays (ask the maintainer to name the sound info) | Its length in seconds, to compare with the spec's *M* and *N* |
| O28.7 | Combat music fade (RT-3) | `sound.md` §10.6, `script-runtime.md` §4.4 | Enter and leave combat in an area with music; a recording of the client's output helps | Does area music fade out over about 3 s while combat music plays, and fade back in over about 8 s afterwards? Loudness at the end of the fade (about −52 dB below the track expected) |
| O28.8 | Quest-dependent music (RT-4) | `script-runtime.md` §18 (OPEN-11) | In an area whose music depends on a quest, compare with the quest done and not done (the spec writer can name a place by `/loc`) | Same or different music |
| O28.9 | Pacing of rapid ambient loops (RT-5) | `script-runtime.md` §10.4, §11.2 | Listen close to a dense emitter such as a fire | Any burst faster than about ten per second? |
| O28.10 | ABC dialect checks OC1–OC10 | `abc-dialect.md` §17 | Put each OC file in `Music` and `/play` it | Per row: plays / refused (with the message, in your own words) / truncated, and the approximate timing |
| O28.11 | ABC commands | `abc-dialect.md` §11 (OPEN) | `/play` a file with several parts and no part number; try `/playlist` with a filter and with songs in a sub-folder; time `/playstart` after a synced `/play` and a solo `/play` | Which part plays; the filter syntax and whether sub-folders are listed; the delay to the first note in both cases (about 0.7 s expected) |
| O28.12 | Re-attack with a different volume | `abc-dialect.md` §9.3, `instruments.md` §15 (OC6) | Play a file that strikes the same pitch again with a changed `+dynamic+` mark | Does the re-struck note play or go silent? |
| O28.13 | Live play and release (Q1) | `instruments.md` §5.1, §9.2 | Hold a sustained instrument's note key for 2 s and for 10 s, then release; compare live loudness with `+mf+` and `+ffff+` song notes; play live with the quantize option on and off against a metronome | A short (about 0.2 s) release fade yes/no; live loudness relative to the two song marks; whether quantize moves notes onto a grid, and its step |
| O28.14 | Fast passages and the instance limit (Q2) | `instruments.md` §8.6 | Play a lute run of more than 10 notes within one second | Any dropped notes? |
| O28.15 | Quantize and from-song flags (Q4) | `instruments.md` §5.8 | `/play` a horn part that uses the top C with the quantize option on | Is the top C missing? Do loudness marks have no effect? |
| O28.16 | Several performers (Q6) | `instruments.md` §9.3, `sound.md` §6.6 | With several characters playing at once (a group, or several of your own clients) | Dropped notes once many voices sound? |
| O28.17 | Handknells octave (Q5, optional) | `instruments.md` §7.4 | Compare one Handknells note against a reference tone (a tuner app) | The octave heard |

**2026-10-08 (from the data and the client, narrowed).** Several rows are predicted exactly by
the original's scripts (data) and its script runtime (BIN); for those, listening is only a
confirmation:

- **O28.3, answered from the data:** the silences between tracks are one-shot timers of the
  music programs, mostly 5–30 s (15 s commonest, then 10, 20 and 30 s; `sound.md` §10.4).
- **O28.4, answered from the client:** loudness falls linearly from the sound's minimum to its
  maximum distance and is zero beyond; the distances are stored per sound (`sound.md` §6.2).
- **O28.7, answered from the data and the client:** entering combat runs a 3 s fade-out to
  0.05 of the music level (applied twice, so about −52 dB, `script-runtime.md` §4.4) and the
  combat music; leaving combat fades the area music back in over 8 s (`sound.md` §10.6).
  When combat starts is server state.
- **O28.9, answered from the client:** a looping sequence runs at most ten passes per second
  and a looping timer's cycle is at least 0.1 s (`script-runtime.md` §10.4, §11.2), so no
  burst is faster than ten per second.
- **O28.1 and O28.2, narrowed:** when the area music changes, the old track fades out
  linearly over its own ramp-out time (1–4 s, mostly 4 s) while the new area's programme
  starts at once; crossing back starts the first area's programme again from its first step
  (`script-runtime.md` §6.6, `sound.md` §10.5). Not in the data or the client code read so
  far: **where** the music changes (landblock or finer, and whether dungeon, landmark or area
  music wins; `sound.md` §10.3.1 OPEN). Still for the owner: cross one area border with music
  playing and record the `/loc` where the music changes and whether the old track fades for
  a few seconds; O28.2 is then optional.
- **Unchanged (listening needed):** O28.5 (which beds change at night and indoors; the night
  switch itself is at time of day 0.8 and 0.25, `environment.md` §6, §10.1), O28.6, O28.8
  (quest-dependent music: the quest state comes from the server and the game callback that
  reads it was not decoded, `script-runtime.md` §18 OPEN-11), O28.10–O28.17 (ABC and instrument
  checks: OC1–OC8 confirm `abc-dialect.md`'s predictions, OC9–OC10 settle its §4.1 OPEN).

---

<a id="m5-items"></a>
<a id="o39"></a>
## O39. Native panels in the original client (M5 criterion 2)

**Spec:** `ui-layout.md` §17 (panels P1–P15, their G, A, T and S rows) and the method of
[`m5-visual.md`](m5-visual.md). [ROADMAP](../ROADMAP.md#m5) M5 criterion 2.

**First, see ours:** the commands in the "Tessera-side self-check" section of
[`m5-visual.md`](m5-visual.md) (one run per panel with `--open-panel P<n>`).

**What to do:** at a known screen size (record it), interface scale 1.0, no user skin and
the default panel positions, take a screenshot of at least 10 of the panels P1–P15, plus the
button states listed in each panel's S row.

**Record:** the screenshots stay private (dirty-room scratch area). In `m5-visual.md` record
only pass / fail / n/a per row, the screen size and the date.

<a id="o40"></a>
## O40. The 14 census plugins in Tessera (M5 criterion 4)

**Spec:** `plugin-api-usage.md` §9 (the 14 plugins), `lua-plugin-api.md`. [ROADMAP](../ROADMAP.md#m5)
M5 criterion 4. Tessera's synthetic plugin suite passes; the real plugins have never been
run because the owner has no `Plugins` folder yet.

**What to do:** install the 14 plugins into a `Plugins` folder (optionally also the
publisher's 2010 sample package, to check that a user copy takes precedence over Tessera's
built-in class helper). Run `cargo test --release -p tessera-plugins --test
compat_real_plugins` with that folder in place (it prints a table per plugin and writes
nothing into the install), then start Tessera with the folder and try each plugin's windows.
For reference, also note in the original client what each plugin's windows look like and do.

**Record:** per plugin: loads yes/no, creates its windows yes/no, responds to input yes/no,
and any difference in your own words. The incompatibility list goes into the ROADMAP M5
status.

<a id="o41"></a>
## O41. A user skin in both clients (M5 criterion 3, optional)

**Spec:** `skin-definition.md`. Tessera's side passes with a generated skin
(`apps/client/tests/ui_skin_real_data.rs`).

**What to do:** if you use a published user skin, compare how the original client shows the
panels it maps (art and geometry) with Tessera started with `--skin <name> --skins-root
<folder>` on the same skin folder.

**Record:** per mapped panel: same / different, in your own words.

<a id="o42"></a>
## O42. Second press during mouse-look (decision 0014, Known difference 14)

**What to do:** in the original client, during mouse-look over the world, make a quick
second right press; then, while right is held, make a quick second left press.

**Record:** does the camera keep turning; does a double-click or tap fire; does the run chord
start.

<a id="o43"></a>
## O43. Wheel over a plain plugin window (decision 0014, D5 OPEN)

**What to do:** with a plugin window open that has no scrollable content, turn the mouse
wheel with the pointer over it.

**Record:** does the camera zoom, yes/no.

<a id="o44"></a>
## O44. Right press during an item drag (decision 0014, Known difference 20)

**What to do:** start dragging an item from a bag or a quickslot, then press the right mouse
button; repeat while trying the run chord.

**Record:** does the camera turn; is the drag cancelled; where does the item land; does the
run chord work during the drag.

<a id="o45"></a>
## O45. Escape with panels open (`game-panels.md` §16)

**What to do:** open several panels (for example the social, options and world-map panels),
then press Escape repeatedly.

**Record:** which panel closes first and in what order, and when the main menu opens.

<a id="o46"></a>
## O46. UI scale, auto-fade and edge blending (decision 0015, observations 1–3)

**What to do:** look at the chat window at interface scale 1.0, 1.5 and 0.5; watch a panel
fade below its top level; take a frame capture of a semi-transparent panel edge.

**Record:** text crispness and size at each scale; the look of the fade; the blend of the
edge pixels (numbers from the capture).

<a id="o47"></a>
## O47. Nested-surface test plugin (decision 0015, observation 4)

**What to do:** write a small test plugin for the original client: a window at opacity 0.5
holding a stretch-mode child, an overlapping sibling, and a child window with a stretch
control, at z = 30. Take a screenshot (kept private).

**Record:** how opacity, overlap and clipping combine, in your own words.

<a id="o48"></a>
## O48. Cursor and minimap marker (decision 0015, observations 5–7)

**What to do:** watch the cursor during mouse-look and when it ends; look at the OS cursor
size at compositor scale factor 1.333; look at the minimap marker pointing north, east and
at 45 degrees.

**Record:** cursor visible during mouse-look yes/no and where it reappears; cursor size; the
look of the marker's edges at 45 degrees.

<a id="o49"></a>
## O49. UI budgets on the idle reference laptop (M5 criterion 7)

**Spec:** decision 0015 §6 (U1–U5), decision 0010, [`docs/perf/m5-ui-budgets.md`](../perf/m5-ui-budgets.md).
The values recorded during M5 are indicative only (the machine was loaded).

**What to do:** on the idle reference laptop, run the scripted UI measurement of
`m5-ui-budgets.md` at 3840 × 2160 and at the window size the compositor gives.

**Record:** the two reports, added to `m5-ui-budgets.md`.

---

<a id="m7-items"></a>
<a id="o70"></a>
## O70. Player movement speeds and turn rate (M7)

**Spec:** `test-server.md` §4.5 and §8.3 (movement validation and its tolerances),
`tessera-protocol.md` §8.3, and the load tool's bot speeds (`test-server.md` §13.1). The specs
currently use the walker's 2 m/s walk and 6 m/s run (`walker.md`). Overlaps with
[O14](#o14) item O14.4 and [O18](#o18) step 1 (M4.5); one set of readings answers all three.

**What to do:** on flat open ground, type `/loc`, move straight for a timed 10 s, type `/loc`
again. Do this for walking forward, running forward, walking backward, and strafing if the
client has a strafe move. Then turn in place with the keyboard for one full turn (360°) and
time it.

**Record:** metres per second per gait (forward walk, forward run, backward, strafe or "no
strafe"); degrees per second for the keyboard turn; whether the speeds seem to differ by
species (if so, per species).

**Answered from the client (2026-10-08).** No measurement is needed (`collision.md`
§11.8–§11.9, BIN; values from the data):

- **Speeds** (no speed effects active): running 6.5 m/s forward, 3.25 m/s backward, 5.0 m/s
  strafing (the client has strafe); walking (walk mode) 2.5 m/s forward, 2.25 m/s backward,
  2.57 m/s strafing. The 2 m/s walk and 6 m/s run that the M7 specs use are therefore not the
  original's.
- **Keyboard turn on foot:** the `[Input] TurnKeySpeed` preference, 150 degrees per second by
  default (a full turn in 2.4 s); walk mode does not slow it.
- **Species:** no difference; only speed effects set by the server change the speeds.

The stopwatch runs above are an optional confirmation.

<a id="o71"></a>
## O71. How other players' characters move when watched (M7)

**Spec:** `tessera-protocol.md` §8.6 (snapshot interpolation and its recommended delay, now
about 167 ms at the default rates) and the client's remote-character placement
(`walker.md` §10.8).

**What to do:** watch another player's character for a few minutes in a busy place: while it
walks, runs, stops, turns sharply and turns on the spot. An informal look is enough; a short
video may be kept privately, never in the repository.

**Record:** yes/no answers: does it trail its true position smoothly, or snap after stopping
or turning; does it turn on the spot with a stepping animation or rotate rigidly; does it
slide or stutter at a normal latency. Rough delays in milliseconds where you can judge them.

**2026-10-08 (check):** not answerable from the data or the offline client. It is a felt
comparison, and the path that applies other players' updates belongs to the network layer,
which decision 0011 excludes. Unchanged.

<a id="o72"></a>
## O72. Distance at which other player characters appear and disappear (M7)

**Spec:** `tessera-protocol.md` §9 (interest management by landblock: radius 2 blocks with a
one-block margin before an entity leaves) and `test-server.md` §8.2.

**What to do:** in open terrain, with a second account or a friend, stand still and let the
other character walk straight away from you until it vanishes, then walk back until it
appears again. Take `/loc` readings of both characters at the moment it vanishes and at the
moment it appears.

**Record:** the distance in metres (or landblocks, from the two `/loc` readings) for
disappearing and for appearing again; whether the two differ (hysteresis).

**2026-10-08 (check):** not answerable from the data or the offline client: the server
decides which characters a client is told about. Unchanged.

<a id="o73"></a>
## O73. Range of local "say" chat (M7, optional)

**Spec:** `test-server.md` §4.6 (`[chat] say_range_m`) and §9 (chat relay).

**What to do:** with two characters, one speaks on the local "say" channel while the other
moves away in steps; note where the lines stop arriving. Repeat for an emote if emote text
seems to have its own range.

**Record:** metres only (from `/loc`), for say and, if different, for emote text. **Do not
record any chat text.**

**2026-10-08 (check):** not answerable from the data or the offline client: the server
decides who receives a chat line. Unchanged (optional).
