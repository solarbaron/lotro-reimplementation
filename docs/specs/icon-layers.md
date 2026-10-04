<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Layered icons: properties, layer order and compositing

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/material` (`icon` module: layered icon composition), explorer icon view |
| **Companion specs** | `property-system.md` §5–§6 (property sets; the 0x70 → 0x79 rule). `records.md` §5 (render surfaces). `texture-list.md` §6.1 (icons are single surfaces, never texture lists). |

---

<a id="s0"></a>
## 0. Purpose

Item icons, and a few other icon families, are not stored as finished images. A game
object's property set names several **layer** images (a background, an underlay, a drop
shadow, the picture itself, an overlay), and the UI stacks them. This spec defines:

- which properties carry the layer references, and what they point to (§2);
- the layer order and the alpha rule needed to rebuild the icon (§3);
- where an item's quality colour comes from (§4);
- the other icon families (skills, effects, traits, sockets, large images, overrides) (§5);
- the evidence behind the order, which was checked by compositing real icons and viewing
  them (§6).

All counts are observations from one install on one date. Tests assert the rules and
invariants, not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = checked on the user's install on 2026-10-04 by throwaway probes in
the dirty-room scratchpad. **Reported** = from third-party material only.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04. All 384,948 property sets decoded: 383,332 with prefix 0x79 and 1,616 with prefix 0x78, every one parsed to the last byte. Every image-valued property, top-level or nested, was resolved against the 0x41 headers of all 1,408,052 surfaces. The alpha channel of every distinct surface used as an icon layer was classified. About 60 item icons were composited and inspected by eye, including variants with different layer orders and size anchoring. |
| LC | LotroCompanion `lotro-companion` (GPL-3.0) and `lotro-tools` sources: the layer property names an item reader uses, the order in which it stacks them, and its alpha rule. Facts only. |
| DS | `research/notes/data-semantics.md` §7.4 (list of `Icon_Layer_*` names). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| I1 | Item icon layers are DID-valued properties named `Icon_Layer_*` (§2.2) | LC, DS; OBS | Confirmed |
| I2 | Every icon-layer value is a 0x41 surface DID, or 0 meaning "no layer" (§2.3) | OBS (all 550,690 layer values, 521,824 of them non-zero) | Confirmed |
| I3 | Layer order: Background, Underlay, Shadow, Image, Overlay (§3.2) | LC (Background, Underlay, Shadow, Image); OBS (Background bottom, Image above Shadow, Overlay top: §6.2) | Confirmed except the relative order of Underlay and Shadow, which is Reported |
| I4 | Layers combine with straight-alpha source-over (§3.3) | LC; OBS (alpha statistics, visual check) | Confirmed (straight alpha); colour space is OPEN |
| I5 | Canvas and anchoring for layers of unequal size (§3.4) | LC (largest size, top-left) | Reported; OPEN for the client |
| I6 | Item quality is expressed by which background surface the item names; it is not a separate layer (§4) | OBS (correlation over 150,605 items) | Confirmed |
| I7 | Skill, effect and most other icons are single surfaces (§5) | LC; OBS | Confirmed |
| I8 | Trait icons add a static overlay frame on top (§5.3) | LC; OBS (1,258 traits) | Confirmed. The rank overlay is Reported. |

---

<a id="s2"></a>
## 2. Where the layer references live

<a id="s2-1"></a>
### 2.1 Records

An item, skill, effect or trait is a script-state object (prefix 0x70). Its static
properties are in the property set with the same low 24 bits and prefix 0x79
(`property-system.md` §6.2). Some stand-alone UI tables use prefix-0x78 property sets.
Icon properties may sit at the top level of a set, or inside struct and array values
(for example, socket-type tables list background and overlay pairs, §5.4).

Property values are resolved **by name** through the master property dictionary
(`property-system.md` §2). The names below are interoperability identifiers. The property
IDs observed in this install are given for test convenience only, and code must look
them up by name.

<a id="s2-2"></a>
### 2.2 The item layer properties (Confirmed)

All have property type 20 (DID reference).

| Name | Observed PID | Top-level sets that have it | Non-zero values | Role |
|---|---|---|---|---|
| `Icon_Layer_BackgroundDID` | 0x10000F9F | 151,977 | 151,977 | Bottom layer: tile plus frame, colour per quality (§4) |
| `Icon_Layer_UnderlayDID` | 0x10000685 | 107,598 | 81,465 | Coloured glow behind the picture |
| `Icon_Layer_ShadowDID` | 0x10001070 | 121,101 | 121,101 | Dark drop shadow behind the picture |
| `Icon_Layer_ImageDID` | 0x10000E9C | 151,557 | 151,557 | The picture |
| `Icon_Layer_OverlayDID` | 0x100030C9 | 15,422 | 15,422 | Top marker (for example a small tier numeral in a corner) |
| `Icon_Layer_LargeImageDID` | 0x10006C7A | 2,936 | 203 | Separate 64×64 picture for large displays (§5.5). Not part of the 32×32 stack. |
| `Icon_Layer_ImageOverrideDID` | 0x1000144F | 26 | 26 | Stands alone in sets with no other layer (§5.6) |

These names are defined in the dictionary but are **never set** in any static property set
of this install: `Icon_Layer_BackgroundOverrideDID`, `Icon_Layer_UnderlayOverrideDID`,
`Icon_Layer_ShadowOverrideDID`, `Icon_Layer_ImageOverrideDID_Checkpointed`,
`Icon_Layer_LargeImageGlowDID` (§5.6).

Layer combinations among the 151,977 top-level sets that have a background (a value
counts as present when non-zero):

| Background | Underlay | Shadow | Image | Overlay | Sets |
|---|---|---|---|---|---|
| ✓ | ✓ | ✓ | ✓ | | 81,424 |
| ✓ | | ✓ | ✓ | | 39,671 |
| ✓ | | | ✓ | ✓ | 15,416 |
| ✓ | | | ✓ | | 14,999 |
| ✓ | | | | | 420 |
| ✓ | ✓ | | ✓ | | 41 |
| ✓ | | ✓ | ✓ | ✓ | 6 |

Overlay and Underlay never occur together. Every set with an Image also has a Background.

<a id="s2-3"></a>
### 2.3 What the values point to (Confirmed)

- Every non-zero layer value names an existing **0x41 surface**: a single image with no
  mip chain and no texture list (`texture-list.md` §6.1). No layer value names a 0x40 list
  or any other prefix.
- **0 means "no layer"**. Explicit zeros occur: 26,133 Underlay values and 2,733
  LargeImage values are 0.
- Every layer surface has surface category 15 (UI image).
- One fully transparent 32×32 A8R8G8B8 surface (alpha 0 at every pixel) is used as a
  placeholder: it is the Underlay value of 23,402 sets and the Shadow value of 38,396.
  Compositing it changes nothing, so no special case is needed. A cache may still
  recognise it and skip the work.
- Shadow and Underlay name the same surface in 23,324 sets. In 23,222 of them that
  surface is the transparent placeholder.

Layer surfaces by format and size (counts of references):

| Layer | Distinct surfaces | Formats (references) | Sizes |
|---|---|---|---|
| Background | 52 | R8G8B8 145,339; A8R8G8B8 6,662 | 32×32 always |
| Underlay | 2,206 | A8R8G8B8 81,419; R8G8B8 46 | 32×32 |
| Shadow | 5,087 | A8R8G8B8 120,926; R8G8B8 175 | 32×32, a few 36×36, 32×30, 128×128 |
| Image | 15,487 | A8R8G8B8 145,254; R8G8B8 6,303 | 32×32 in 99.8%; the rest 16×8 to 36×36 |
| Overlay | 16 | A8R8G8B8 | 32×32 |

An R8G8B8 surface is fully opaque (`records.md` §5.3). An R8G8B8 layer above the background
hides everything below it.

<a id="s2-4"></a>
### 2.4 Example (synthetic)

An item whose 0x79 set contains:

| Property | Value |
|---|---|
| `Icon_Layer_BackgroundDID` | 0x41AAAA01 (32×32 R8G8B8: framed tile in the quality colour) |
| `Icon_Layer_UnderlayDID` | 0x41AAAA02 (32×32 A8R8G8B8: soft blue glow, alpha high around the object outline) |
| `Icon_Layer_ShadowDID` | 0x41AAAA03 (32×32 A8R8G8B8: black, alpha follows the object silhouette with a soft edge) |
| `Icon_Layer_ImageDID` | 0x41AAAA04 (32×32 A8R8G8B8: the object, alpha 0 around it) |
| `Icon_Layer_OverlayDID` | 0 |

The icon is the 32×32 result of stacking 01, 02, 03, 04 in that order (§3).

---

<a id="s3"></a>
## 3. Compositing

<a id="s3-1"></a>
### 3.1 Inputs

For each of the five stack layers (Background, Underlay, Shadow, Image, Overlay), read the
property value. Take it as absent if the property is missing, 0, or names a surface that
cannot be found or decoded. Decode each present surface to straight-alpha RGBA8
(`records.md` §5.3: R8G8B8 gets alpha 255, A8R8G8B8 keeps its alpha).

A missing or undecodable layer is logged, and composition goes on without it. An icon with
no present layers is "no icon".

<a id="s3-2"></a>
### 3.2 Order (bottom to top)

1. **Background**
2. **Underlay**
3. **Shadow**
4. **Image**
5. **Overlay**

How each position was established is in §6.2. Background at the bottom, Image above
Shadow, and Overlay on top are Confirmed from data. Underlay below Shadow is Reported (LC)
only.

<a id="s3-3"></a>
### 3.3 Alpha rule

Start from a canvas that is fully transparent (all channels 0). Draw each present layer, in
order, with the **source-over** operator on **straight** (non-premultiplied) alpha.

For a destination pixel (colour *D*, alpha *d*) and a source pixel (colour *S*, alpha *s*),
with all values in [0, 1]:

- result alpha *r* = *s* + *d*·(1 − *s*);
- if *r* > 0, result colour = (*S*·*s* + *D*·*d*·(1 − *s*)) / *r*; otherwise the result
  colour is 0.

The result keeps straight alpha. The output alpha is below 1 only where every layer is
partly transparent. That happens with the 10 backgrounds that have transparent pixels, or
in an icon without a background.

Integer implementations should round to nearest, and may differ from this formula by ±1 per
channel. Comparisons in tests use a tolerance of 1.

> **OPEN:** Whether the original client blends in sRGB-encoded values (as written above,
> and as typical for 2D UI) or in linear light. The difference is visible only in soft
> edges.

<a id="s3-4"></a>
### 3.4 Canvas size and anchoring

In 151,759 of 151,977 item sets (99.86%), all present layers have the same size, in practice
32×32. The remaining 218 have an Image or Shadow of another size: smaller (for example
28×24 or 16×8) or larger (35×35, 36×36).

Tessera rule (provisional until the client is observed):

- Canvas size = the Background's size when a Background is present. Otherwise it is the
  largest width and the largest height among the present layers.
- Each layer is **centred** on the canvas. The offset is ⌊(canvas − layer) / 2⌋ on each
  axis, and the parts of a layer that fall outside the canvas are clipped.

LC instead makes the canvas the largest layer size and anchors every layer at the top-left.
Composites checked by eye: centring looks correct for every smaller image tried, while
top-left anchoring pushed small pictures visibly into a corner. For the larger 35×35 and
36×36 images, neither placement is clearly right.

> **OPEN:** The client's anchoring and clipping for unequal layer sizes. It affects 0.14% of
> item icons.

---

<a id="s4"></a>
## 4. Quality colour (Confirmed)

No layer property is derived from item quality at run time. Each item names a background
surface, and those surfaces differ in colour by quality. Over the 150,605 sets that have
both `Item_Quality` (an enum value) and a Background:

| Quality value | Items | Distinct backgrounds | Share of the most common background |
|---|---|---|---|
| 0 | 1,839 | 2 | 96.1% |
| 1 | 8,238 | 14 | 74.8% |
| 2 | 45,577 | 24 | 80.6% |
| 3 | 29,227 | 20 | 83.0% |
| 4 | 33,424 | 15 | 85.9% |
| 5 | 32,300 | 21 | 33.3% |

Each quality value's most common background is a different surface (6 values, 6 surfaces).
The other backgrounds are category-specific tiles (for example wood-grain tiles), and those
are shared across qualities. So:

- a renderer draws the Background the item names, and never substitutes or tints it by
  quality;
- a tool that wants a quality border for an object with no Background has to supply its own.

> **OPEN:** Whether the client UI draws any extra quality border over item slots, outside
> the composed icon. Nothing in the property data indicates one.

---

<a id="s5"></a>
## 5. Other icon families

All values below are type-20 properties naming single 0x41 surfaces, with category 15. They
compose with the §3 rules.

<a id="s5-1"></a>
### 5.1 Skills (Confirmed)

| Property | Sets | Sizes |
|---|---|---|
| `Skill_Icon` | 13,247 | 32×32 (13,008); 71×69 (239) |
| `Skill_LargeIcon` | 13,247 | Same DID as `Skill_Icon` in all 13,247 |
| `Skill_SmallIcon` | 13,247 | Same DID as `Skill_Icon` in 13,008. In the 239 with a 71×69 `Skill_Icon` it is a separate 32×32 surface. |

A skill icon is a single layer, with no stack. Of 2,701 distinct `Skill_Icon` surfaces,
1,900 have binary alpha, 335 are fully opaque, and 466 have soft alpha.

<a id="s5-2"></a>
### 5.2 Effects (Confirmed)

`Effect_Icon`: 22,804 sets. Single layer, 32×32 in 99.9%; a few range from 30×29 to 50×50.

<a id="s5-3"></a>
### 5.3 Traits

| Property | Sets | Notes | Mark |
|---|---|---|---|
| `Trait_Icon` | 4,318 (70 are 0) | Base picture, 32×32 | Confirmed |
| `Trait_Static_Icon_Overlay` | 1,258 | 32×32 frame: opaque border, transparent centre. Drawn **above** `Trait_Icon` (Confirmed visually: the reverse order would hide the picture). | Confirmed |
| `Trait_Rank_Overlay_Progression` | 1,260 (171 are 0) | Type 20. The 1,089 non-zero values name 0x70 objects (progressions), not surfaces. LC treats it as a progression that maps a rank to an overlay surface, drawn above the static overlay. | Reported |
| `Trait_Icon_Progression` | 13 | Values name 0x70 objects (progressions), not surfaces | OPEN |

Trait order: `Trait_Icon`, then `Trait_Static_Icon_Overlay`, then the rank overlay.

> **OPEN:** The progression object layout and how a rank selects an overlay surface.

<a id="s5-4"></a>
### 5.4 Socket and nested layers (Confirmed)

24 Background values and 49 Overlay values occur inside array or struct values, not at
the top level of a set: for example per-socket-type entries with a Background/Overlay pair,
and level-keyed overlay entries. Where a Background and an Overlay appear in the same
entry, they compose in the §3.2 order (Background below Overlay).

<a id="s5-5"></a>
### 5.5 Large item image (Confirmed)

`Icon_Layer_LargeImageDID`: 203 non-zero values, all 64×64 A8R8G8B8 with soft alpha. It is
a separate single-layer picture for large displays, never stacked with the 32×32 layers.

<a id="s5-6"></a>
### 5.6 Overrides (partly Reported)

`Icon_Layer_ImageOverrideDID` appears in 26 sets that have no other icon layer. Each value
is a single 32×32 A8R8G8B8 surface, used as the whole icon.

The other `…OverrideDID` names (§2.2), and `…LargeImageGlowDID`, never occur in static data.
They are probably set at run time, by the server or by game state.

Tessera rule (provisional): if an override property for a layer is present and non-zero,
use it in place of the base layer of the same role. Then compose as in §3.

> **OPEN:** Override semantics in the client, and whether `…_Checkpointed` differs from the
> plain override.

<a id="s5-7"></a>
### 5.7 Other single-image properties (Confirmed, informative)

About 100 further DID properties whose names contain "Icon" or "Image" point directly at
category-15 surfaces. Examples: title, set, area, emote, hobby, perk, race and class icons;
map-note images (normal, highlighted, selected, menu); map images (JPEG); tutorial and
pop-up pictures (up to 384×512). Each is one surface, shown as is. When a family has
explicit small and large variants, each variant is its own property and surface. Nothing is
scaled from a single source.

---

<a id="s6"></a>
## 6. Evidence for the order and the alpha rule

<a id="s6-1"></a>
### 6.1 Alpha statistics per layer (Confirmed, every distinct layer surface)

| Layer | A8R8G8B8 surfaces | Of those: soft alpha | Fully transparent | Notes |
|---|---|---|---|---|
| Background | 18 | 10 | 0 | The other 34 backgrounds are R8G8B8 (opaque) |
| Underlay | 2,205 | 2,203 | 1 | Saturated colour where alpha > 0 |
| Shadow | 5,082 | 5,055 | 7 | 4,832 have black RGB wherever alpha > 0 |
| Image | 14,965 | 14,467 | 2 | |
| Overlay | 16 | 16 | 0 | |

Straight (not premultiplied) alpha: in 2,123 of the 2,203 soft-alpha underlays, and in all
86 large images, some pixel has a colour channel above its alpha. Premultiplied data cannot
do that.

<a id="s6-2"></a>
### 6.2 Order (each position separately)

| Claim | Evidence | Mark |
|---|---|---|
| Background is the bottom layer | 145,339 of 151,977 backgrounds are R8G8B8, which is opaque everywhere. Any layer drawn under one would never be visible. | Confirmed |
| Image is above Shadow | In 2,000 sampled items, 81% of the image's fully opaque pixels lie under shadow pixels with alpha > 0.5. Shadows are black, so drawing the shadow on top would blacken most of the picture. Shadow alpha extends about 26% beyond the image silhouette, which is the visible drop shadow. | Confirmed |
| Overlay is the top layer | Over 15,422 items, 77% of the overlay's opaque pixels (the numeral) lie over fully opaque image pixels. Below the image, the numeral would be invisible. | Confirmed |
| Underlay is below Shadow | LC's order. The two orders change about 23% of pixels in icons that have both (300 sampled), and both looked plausible by eye. Underlay alpha extends about 31% beyond the shadow (an outer glow). | Reported |

> **OPEN:** Confirm the Underlay/Shadow order by comparing with a screenshot of the original
> client showing an item that has both layers.

<a id="s6-3"></a>
### 6.3 Visual check

About 60 icons from every layer combination of §2.2 were composited with §3 and inspected
by eye at 4–5× zoom. Every result looked like a finished icon: frame visible, picture
centred, no dark halo over the picture, overlay numerals legible. Background colour also
matched the item's quality across samples. The images were kept in the scratchpad only.

---

<a id="s7"></a>
## 7. Invariants

| # | Invariant | Status |
|---|---|---|
| IL1 | Every non-zero `Icon_Layer_*` value names an existing 0x41 surface | Confirmed (521,824 non-zero values) |
| IL2 | Every icon-layer surface has category 15 | Confirmed |
| IL3 | Every set with an Image layer also has a Background layer | Confirmed |
| IL4 | Overlay and Underlay never both non-zero in one set | Confirmed (observation, not a rule; readers must not depend on it) |
| IL5 | Background surfaces are 32×32 | Confirmed (observation; §3.4 handles other sizes) |

---

<a id="s8"></a>
## 8. Synthetic fixtures for implementers

Generate these with our own code. No game images.

- A 2×2 opaque R8G8B8 background plus a 2×2 A8R8G8B8 image whose pixels have alpha 0, 128
  and 255: check the §3.3 formula, including the alpha-0 pixel taking the background colour.
- A four-layer stack where each layer is a single opaque pixel at a different position:
  check order and placement.
- A soft black shadow under an opaque image: the image's opaque pixels must come out
  unchanged.
- A layer of value 0, a missing layer DID, and a fully transparent layer: all three give the
  same result as leaving the layer out.
- Unequal sizes: a 4×4 background and a 2×2 image (centred at offset 1,1), and a 6×6 image
  on a 4×4 background (clipped, offset −1).
- A trait base picture plus a frame overlay with a transparent centre.

---

<a id="s9"></a>
## 9. Open questions

1. sRGB versus linear blending (§3.3).
2. Client anchoring and clipping for unequal layer sizes (§3.4).
3. Any quality border drawn by the UI outside the icon (§4).
4. Trait rank-overlay progression layout (§5.3).
5. Override and glow properties at run time (§5.6).
6. The relative order of Underlay and Shadow (§6.2).

---

<a id="s10"></a>
## 10. Open items raised by the `material` implementation (2026-10-04)

These came up while implementing `crates/material` (`icon` module) from this spec alone.
The real-data test (`crates/material/tests/real_data.rs`, 200 random item sets plus skill,
effect, trait and large-image samples) composes every sampled icon with no layer left out,
so none of these choices is exercised by the sampled data.

> **OPEN:** §5.6 names no override property for the Overlay layer, so Overlay is always
> taken from `Icon_Layer_OverlayDID`. `Icon_Layer_ImageOverrideDID_Checkpointed` and
> `Icon_Layer_LargeImageGlowDID` are ignored until their meaning is known.

> **OPEN:** §2.3 says a fully transparent layer changes nothing, and §8 that it gives the
> same result as leaving the layer out. In a stack without a Background a transparent
> layer larger than the others would still widen the §3.4 canvas if it counted for the
> size. The implementation follows §8: a fully transparent layer is treated as absent. It
> does not count for the canvas size, and a stack whose only layers are transparent is
> "no icon". No fully transparent Background exists in the data (§6.1), so item icons are
> unaffected.

> **OPEN:** When a stack holds more than one Background-role entry (possible only for
> caller-built stacks, never from one property set), the first one sets the canvas size.

> **OPEN:** The spec sets no size limit for a layer. The implementation refuses (and
> leaves out) layers larger than 2048 px on a side by default, checked before decoding
> (for JPEG, from the stream's frame header). Item layers are at most 128×128 and the
> tutorial and pop-up pictures of §5.7 at most 384×512, but a review scan found JPEG
> surfaces up to 4800×4800 (map images, §5.7, are JPEG; which properties name those
> surfaces was not checked). A caller that shows such pictures raises the limit, up to the
> JPEG decoder's 8192.

> **OPEN:** A layer surface with width or height 0 is not described. The implementation
> takes it as absent (§3.1: it cannot be decoded into an image), so it neither sets the
> canvas size nor is reported as drawn. None occurs in the data.

> **OPEN:** §5.6 says a present override is used in place of the base layer, and §3.1 that
> a value naming a surface that cannot be found or decoded is absent. The implementation
> reads them together: an override that is absent in that sense gives way to the base
> layer, as if the override were not set. No static data has both (§5.6).

> **OPEN:** `Icon_Layer_ImageOverrideDID` alone (§5.6) gives a stack whose only layer has
> the Image role and no Background, so IL3 is about `Icon_Layer_ImageDID` only. One such
> set appeared in the 200-set sample.
