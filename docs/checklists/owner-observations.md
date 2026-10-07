<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Observations only the project owner can make

| | |
|---|---|
| **Status** | Open list; started 2026-10-04; last updated 2026-10-07 (items answered or narrowed from the client) |
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
| [O1](#o1) | `/loc`, map coordinates and area at 20 reference locations | `docs/checklists/m3-reference-locations.md` | In-game `/loc` | partly answered (`coordinates.md` §4, §8–§9, §14: formula, rounding, landcells and block edges from the client); left for the owner: the avatar's height above the terrain (`oz`), the area-name column, and whether map coordinates show indoors |
| [O2](#o2) | Compass bearing of a placed object versus `/loc` heading | `entity-descriptor.md` §4.4 | In-game `/loc` | answered from the client (`coordinates.md` §11.3–§11.4: h = (360° − yaw) mod 360, bearing of model +Y); optional confirmation |
| [O3](#o3) | Which content layer the world shows by default | `entity-descriptor.md` §4.5.1 | In-game look | answered from the client (`entity-descriptor.md` §4.5.1, §9.3: layers 1 and 2 are both shown, for every kind of placed object including drawn Physics objects; only named layers depend on story progress); optional confirmation spot: region 1, block 0xA15B |
| [O4](#o4) | Are one-triangle placeholder parts drawn? | `setup.md` §8.5 | Look, or frame capture | answered from the client (`setup.md`: never drawn in objects with more than one part); no observation needed |
| [O5](#o5) | Exact dye blend | `material.md` §12.3 | Look, or frame capture | open |
| [O6](#o6) | Other material and render-state questions | `material.md` §17 and §19 | Frame capture | open |
| [O7](#o7) | Command-line option leniency | `install-layout.md` §10 | Launch test | answered from the client (`install-layout.md` §10.1); no observation needed |
| [O8](#o8) | Standing heights per species and sex (whole-body scale) | `appearance.md` §6.1, §17, `skeleton-anim.md` §2.2 | Look, side-by-side screenshots | partly answered (`appearance.md` §6.1, §17); left for the owner: one screenshot of a High-elf female beside a Human female |
| [O9](#o9) | Height slider and facial bone sliders in character creation | `appearance.md` §6.2, §6.3 | Character-creation screen | partly answered (`appearance.md` §6.2–§6.5, §17: slider rules); left for the owner, as confirmation only: heights at the ends of the height controls, River-hobbit feet on the ground |
| [O10](#o10) | Worn-item layering and equal-priority ties | `appearance.md` §4.6, §4.7 | Look (wardrobe preview) | partly answered (`appearance.md` §4.7, §17: the client's tie rule); left for the owner: items 1–8, mainly item 5 and whether the order of equipping matters |
| [O11](#o11) | Updated-avatar option and default style files | `appearance.md` §3.3 | Options screen, look | answered from the client (`appearance.md` §3.3, §3.5, §17); optional glance at the options screen to confirm the default |
| [O12](#o12) | Look of random-appearance NPC figures | `appearance.md` §7.4 | Look at two spots, two visits | partly answered (`appearance.md` §7.4, §17: where outfits come from); left for the owner: two visits to one figure (same look or not) |
| [O13](#o13) | Where weapons, instruments and class items rest, drawn and stowed | `appearance.md` §5.3–§5.4, `skeleton-anim.md` §8.5 | Look | partly answered (`appearance.md` §5.6, §17: weapons, shields, ranged items); left for the owner: in-hand direction of long weapons (two side screenshots) and whether class items are ever visible |
| [O14](#o14) | Animation playback: blending, layering, turn direction, speeds, scale inheritance, cloth | `skeleton-anim.md` §5.4, §6.5, §6.6, §7.3, §7.4, §13 | Look, `/loc`, short videos | partly answered (`skeleton-anim.md` §15.1: O14.1–O14.3, O14.5–O14.7 answered); left for the owner: O14.4 walking speeds by `/loc` timing; visual checks of O14.1, O14.2, O14.7 optional |
| [O15](#o15) | Skin, hair and eye colour tinting; face layers | `appearance.md` §3.3, §3.5 | Frame capture | partly answered (`appearance.md` §3.5, §17: one composited face texture, tints through the dye colour; colour byte order B, G, R, A, `property-system.md` §4.7); left for the owner: one face frame capture for the texture slot (low priority) |
| [O16](#o16) | Which idle, walk and run clips play; idle variants and fidgets | `script-table.md` §8, §15.1, §16 | Look, explorer side by side | partly answered (`script-table.md` §8.6, §15.1: avatars fidget, timing predicted; mood idles answered); left for the owner: the side-by-side clip comparison and about 5 minutes standing still |
| [O17](#o17) | Static NPCs: position, facing and presence at the N spots | `m4-visual.md` §N, `appearance.md` §7, `entity-descriptor.md` §9.4 | In-game `/loc`, look | partly answered (`entity-descriptor.md` §9.4: presence rule, position and facing from the client); left for the owner: whether N4 and N6 are there for your character, and each figure's look |
| [O18](#o18)–[O27](#o27) | World fidelity: movement, scripted walks, visual rows, clock, draw distances, shadows and lights, water, frills, sky and fog, terrain bands | see [M4.5 items](#m4-5-items) | `/loc`, look, short videos | O21, O22, O26 partly answered (2026-10-07); the others open (M4.5) |

M4 exit criteria that wait on these items ([ROADMAP M4 status](../ROADMAP.md#m4-status)):
criterion 2 (bodies play idle, walk and run): O8, O9, O14, O16; criterion 3 (worn and held
items): O10, O13; the sandbox's static NPCs: O12, O17. Criterion 4 (performance) passed
on the reference machine and needs no observation.

M4.5 exit criteria that wait on these items ([ROADMAP M4.5 status](../ROADMAP.md#m4-5-status)):
criterion 4 (walking on collision): O18, O19; criterion 6 (visual checklist): O20 (terrain,
scenery and frill rows), O23 (lighting rows), O24 (water rows), O25 (frills), O26 (sky and
fog rows), O27; the default-settings profile behind criteria 5 and 6: O21, O22.

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
| [O18](#o18) | Movement: jump, speeds, highest step, steepest slope, wading and swimming depth | `collision.md` §10–§12 (Q9, Q11), `water.md` §7.4 | In-game `/loc`, stopwatch, short video | open |
| [O19](#o19) | Outcome of the 32 scripted walks in the original | `m4-5-visual.md` §2 | In-game `/loc`, look | open |
| [O20](#o20) | Side-by-side look at the terrain, scenery and frill rows | `m4-5-visual.md` §4.1–§4.3 | Look, private screenshots | open |
| [O21](#o21) | Game clock: time of day, day length, epoch | `environment.md` §6, §10.1, §11.1 | Clock in the UI or `/loc`, stopwatch | partly answered (`environment.md` §10.1, §11.1, §11.5: rate, date, night flag, day-type rule); left for the owner: two (in-game time, UTC) readings for the epoch, and the weather on several days |
| [O22](#o22) | Default draw distances: landscape extent, object pop-in, fog clipping | `walker.md` §11.3–§11.4, `mesh.md` §8, `environment.md` §11.7 | Look, `/loc` | partly answered (`environment.md` §10.1, §11.7: fog-distance limits; the distance rules were already from the client); left for the owner: steps 1–6 as a visual check |
| [O23](#o23) | Sun shadows and point lights | `lighting.md` §8–§9, `m4-5-visual.md` §4.6 | Look at a set time, frame capture optional | open |
| [O24](#o24) | Water look: surface, lava, placed water, distant water | `water.md` §8–§9, `m4-5-visual.md` §4.4 | Look, frame capture optional | open |
| [O25](#o25) | Frills: radius, fade, density, sway, terrain tint | `scenery.md` §8.3–§8.4 | Look, short video | open |
| [O26](#o26) | Sky and fog at four times of day; the day type; interior fog | `environment.md` §10.1, §11.2–§12, `lighting.md` §10 | Look at set times | partly answered (`environment.md` §10.1: day-type rule, interior fog rules, sun and moon path); left for the owner: the Y1–Y4 look at set times (after O21), sun and moon size, fog on the sky |
| [O27](#o27) | Terrain near and far bands, and the seam between them | `terrain-texturing.md` §12 | Look, short video | open |

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
