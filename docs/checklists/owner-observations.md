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
| [O8](#o8) | Standing heights per species and sex (whole-body scale) | `appearance.md` §6.1, `skeleton-anim.md` §2.2 | Look, side-by-side screenshots | open (M4; informal look at Tessera fine, 2026-10-05) |
| [O9](#o9) | Height slider and facial bone sliders in character creation | `appearance.md` §6.2, §6.3 | Character-creation screen | open (M4) |
| [O10](#o10) | Worn-item layering and equal-priority ties | `appearance.md` §4.6, §4.7 | Look (wardrobe preview) | open (M4) |
| [O11](#o11) | Updated-avatar option and default style files | `appearance.md` §3.3 | Options screen, look | open (M4) |
| [O12](#o12) | Look of random-appearance NPC figures | `appearance.md` §7.4 | Look at two spots, two visits | open (M4) |
| [O13](#o13) | Where weapons, instruments and class items rest, drawn and stowed | `appearance.md` §5.3–§5.4, `skeleton-anim.md` §8.5 | Look | open (M4) |
| [O14](#o14) | Animation playback: blending, layering, turn direction, speeds, scale inheritance, cloth | `skeleton-anim.md` §5.4, §6.5, §6.6, §7.3, §7.4, §13 | Look, `/loc`, short videos | open (M4) |
| [O15](#o15) | Skin, hair and eye colour tinting; face layers | `appearance.md` §3.3 | Frame capture | open (M4, low priority) |
| [O16](#o16) | Which idle, walk and run clips play; idle variants and fidgets | `script-table.md` §8, §16 | Look, explorer side by side | open (M4) |
| [O17](#o17) | Static NPCs: position, facing and presence at the N spots | `m4-visual.md` §N, `appearance.md` §7 | In-game `/loc`, look | open (M4; informal look at Tessera fine, 2026-10-05) |

M4 exit criteria that wait on these items ([ROADMAP M4 status](../ROADMAP.md#m4-status)):
criterion 2 (bodies play idle, walk and run): O8, O9, O14, O16; criterion 3 (worn and held
items): O10, O13; the sandbox's static NPCs: O12, O17. Criterion 4 (performance) passed
on the reference machine and needs no observation.

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

<a id="o16"></a>
## O16. Which idle, walk and run clips play (M4)

**Spec:** `script-table.md` §8 (how the scripts choose clips; §8.3 lists the clips Tessera
resolves per body) and §16 (OPEN: the resolver also reaches random idle fidgets that §8.6
says avatars do not have). [ROADMAP](../ROADMAP.md#m4) M4 criterion 2.

**First, see ours:** `tessera-explorer --character <species>,<m|f> --clip idle` (then
`walk`, `run`); the inspector lists the chosen clip and the other candidates.

**What to do:** for one character of each playable species and sex you can reach (new
characters at the starting area are fine), stand still for about a minute, then walk and
run in a straight line, and compare with the explorer's figure of the same species and sex.

**Record:** per species and sex: whether idle shows variants or fidgets (and roughly how
often, for example "every 10–20 s"); for idle, walk and run, `same` / `different` from
Tessera's clip (describe a difference in your own words, never by a name from the game).

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
