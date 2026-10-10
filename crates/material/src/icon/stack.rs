// SPDX-License-Identifier: GPL-3.0-or-later
//! Which surfaces make up an icon, and in what order (`icon-layers.md` §2, §3.2, §5).

use std::fmt;

use tessera_formats::Did;

use crate::error::{MaterialError, Result};

/// Property names that carry icon layers (`icon-layers.md` §2.2, §5). These are
/// interoperability identifiers; values are looked up by name through the property
/// dictionary, never by PID.
pub mod names {
    /// Item layer: bottom tile and frame (§2.2). The tiles differ in colour by quality in
    /// the data; composition draws the named tile as it is and never tints or substitutes
    /// it by quality (§4).
    pub const BACKGROUND: &str = "Icon_Layer_BackgroundDID";
    /// Item layer: coloured glow behind the picture (§2.2).
    pub const UNDERLAY: &str = "Icon_Layer_UnderlayDID";
    /// Item layer: dark drop shadow behind the picture (§2.2).
    pub const SHADOW: &str = "Icon_Layer_ShadowDID";
    /// Item layer: the picture (§2.2).
    pub const IMAGE: &str = "Icon_Layer_ImageDID";
    /// Item layer: top marker such as a tier numeral (§2.2).
    pub const OVERLAY: &str = "Icon_Layer_OverlayDID";
    /// Separate 64×64 picture for large displays, never stacked (§5.5).
    pub const LARGE_IMAGE: &str = "Icon_Layer_LargeImageDID";
    /// Not read by the client (§5.6; never set in static data).
    pub const BACKGROUND_OVERRIDE: &str = "Icon_Layer_BackgroundOverrideDID";
    /// Not read by the client (§5.6; never set in static data).
    pub const UNDERLAY_OVERRIDE: &str = "Icon_Layer_UnderlayOverrideDID";
    /// Not read by the client (§5.6; never set in static data).
    pub const SHADOW_OVERRIDE: &str = "Icon_Layer_ShadowOverrideDID";
    /// Not read by the client's icon composition (§5.6): the 26 static sets that carry it
    /// have no Image or Background, so they get no icon.
    pub const IMAGE_OVERRIDE: &str = "Icon_Layer_ImageOverrideDID";
    /// Replaces the Image layer of an item **instance** whenever present, whatever its
    /// value (§3.1 step 1, §5.6); never present in static data.
    pub const IMAGE_OVERRIDE_CHECKPOINTED: &str = "Icon_Layer_ImageOverrideDID_Checkpointed";
    /// Skill icon (§5.1).
    pub const SKILL_ICON: &str = "Skill_Icon";
    /// Skill large icon (§5.1).
    pub const SKILL_LARGE_ICON: &str = "Skill_LargeIcon";
    /// Skill small icon (§5.1).
    pub const SKILL_SMALL_ICON: &str = "Skill_SmallIcon";
    /// Effect icon (§5.2).
    pub const EFFECT_ICON: &str = "Effect_Icon";
    /// Trait base picture (§5.3).
    pub const TRAIT_ICON: &str = "Trait_Icon";
    /// Trait frame drawn above the base picture (§5.3).
    pub const TRAIT_STATIC_OVERLAY: &str = "Trait_Static_Icon_Overlay";
}

/// The role a surface plays in a stack. Only [`LayerRole::Background`] affects layout: it
/// fixes the canvas size (`icon-layers.md` §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LayerRole {
    /// Item background (bottom).
    Background,
    /// Item underlay (glow).
    Underlay,
    /// Item drop shadow.
    Shadow,
    /// Item picture.
    Image,
    /// Item overlay (top).
    Overlay,
    /// Trait base picture.
    TraitIcon,
    /// Trait frame overlay.
    TraitFrame,
    /// A single-surface icon (skill, effect, large image, …).
    Single,
}

impl LayerRole {
    /// The five item stack layers, bottom to top (`icon-layers.md` §3.2).
    pub const ITEM_ORDER: [LayerRole; 5] = [
        LayerRole::Background,
        LayerRole::Underlay,
        LayerRole::Shadow,
        LayerRole::Image,
        LayerRole::Overlay,
    ];

    /// The base property of an item layer role, `None` for the other roles.
    #[must_use]
    pub fn item_property(self) -> Option<&'static str> {
        Some(match self {
            Self::Background => names::BACKGROUND,
            Self::Underlay => names::UNDERLAY,
            Self::Shadow => names::SHADOW,
            Self::Image => names::IMAGE,
            Self::Overlay => names::OVERLAY,
            _ => return None,
        })
    }

    /// The override property of an item layer role (`icon-layers.md` §5.6), if the
    /// dictionary defines one. The Overlay has none.
    #[must_use]
    pub fn override_property(self) -> Option<&'static str> {
        Some(match self {
            Self::Background => names::BACKGROUND_OVERRIDE,
            Self::Underlay => names::UNDERLAY_OVERRIDE,
            Self::Shadow => names::SHADOW_OVERRIDE,
            Self::Image => names::IMAGE_OVERRIDE,
            _ => return None,
        })
    }

    /// A short lower-case name for logs and UIs.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Underlay => "underlay",
            Self::Shadow => "shadow",
            Self::Image => "image",
            Self::Overlay => "overlay",
            Self::TraitIcon => "trait icon",
            Self::TraitFrame => "trait frame",
            Self::Single => "single",
        }
    }
}

impl fmt::Display for LayerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One surface of a stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackEntry {
    /// What the surface is.
    pub role: LayerRole,
    /// The surface DID (non-zero).
    pub did: Did,
    /// The property the DID came from, when known.
    pub property: Option<&'static str>,
    /// The base-layer DID to fall back on when `did` (an override, `icon-layers.md`
    /// §5.6) is absent: not found, unreadable or undecodable (§3.1). `None` when there is
    /// nothing to fall back on. The fallback's property is the role's
    /// [`LayerRole::item_property`].
    pub fallback: Option<Did>,
}

impl StackEntry {
    /// An entry with no fallback.
    #[must_use]
    pub fn new(role: LayerRole, did: Did, property: Option<&'static str>) -> Self {
        Self {
            role,
            did,
            property,
            fallback: None,
        }
    }

    /// The entry to try when this one is absent: the fallback DID with the role's base
    /// property, and no further fallback. `None` without a (non-zero) fallback.
    #[must_use]
    pub fn fallback_entry(&self) -> Option<StackEntry> {
        let did = self.fallback.filter(|d| d.0 != 0 && *d != self.did)?;
        Some(StackEntry {
            role: self.role,
            did,
            property: self.role.item_property(),
            fallback: None,
        })
    }
}

/// Maximum number of layers in one [`IconStack`]. Real stacks have at most five.
pub const MAX_STACK_LAYERS: usize = 16;

/// The most input entries [`IconStack::from_entries`] reads, kept or dropped (an entry
/// with a zero DID and no fallback is dropped). Bounds the work on an endless input of
/// dropped entries; real stacks are built from five entries.
pub const MAX_STACK_INPUT: usize = MAX_STACK_LAYERS * 4;

/// The surfaces of one icon, bottom to top.
///
/// Built from a property lookup (`name → DID`), which for a property set from
/// `tessera-props` is typically
/// `|name| set.get_by_name(registry, name).and_then(|v| v.as_did())`. A value that is
/// missing or 0 means "no layer" (`icon-layers.md` §2.3). An empty stack is "no icon".
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct IconStack {
    entries: Vec<StackEntry>,
}

/// A layer's DID and the property it came from.
type Layer = (Did, Option<&'static str>);

/// Looks a property up by name. Missing and null values are both "absent".
fn present(lookup: &impl Fn(&str) -> Option<Did>, name: &'static str) -> Option<Did> {
    lookup(name).filter(|d| d.0 != 0)
}

impl IconStack {
    /// An empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A stack of the given entries, drawn in the given order (first = bottom). An entry
    /// with a zero DID is replaced by its fallback entry if it has one, else dropped.
    ///
    /// The input is read only until `MAX_STACK_LAYERS + 1` entries are kept or
    /// [`MAX_STACK_INPUT`] + 1 entries are read, whichever comes first, so an endless
    /// iterator is safe. Either fails with [`MaterialError::TooManyLayers`], whose count
    /// is then a lower bound (of kept entries, or of entries read).
    pub fn from_entries(entries: impl IntoIterator<Item = StackEntry>) -> Result<Self> {
        let mut kept: Vec<StackEntry> = Vec::new();
        for (read, e) in entries.into_iter().enumerate() {
            if read >= MAX_STACK_INPUT {
                return Err(MaterialError::TooManyLayers {
                    count: read.saturating_add(1),
                    limit: MAX_STACK_LAYERS,
                });
            }
            let e = StackEntry {
                fallback: e.fallback.filter(|d| d.0 != 0),
                ..e
            };
            let Some(e) = (if e.did.0 != 0 {
                Some(e)
            } else {
                e.fallback_entry()
            }) else {
                continue;
            };
            kept.push(e);
            if kept.len() > MAX_STACK_LAYERS {
                return Err(MaterialError::TooManyLayers {
                    count: kept.len(),
                    limit: MAX_STACK_LAYERS,
                });
            }
        }
        Ok(Self { entries: kept })
    }

    /// An item icon (`icon-layers.md` §3.1, §3.2, §5.6): Background, Underlay, Shadow,
    /// Image, Overlay, bottom to top, as the client builds it.
    ///
    /// `lookup` returns `None` for an absent property and `Some(Did(0))` for a property
    /// that is present with value 0 (or present with a type other than a DID, §3.1 "Read
    /// *P*"). The rules:
    ///
    /// 1. **Image:** `Icon_Layer_ImageOverrideDID_Checkpointed` when present (item
    ///    instances only), else `Icon_Layer_ImageDID`; absent → no icon (an empty stack).
    /// 2. **Shadow**, 3. **Underlay**, 5. **Overlay:** a layer only when present and non-zero.
    /// 4. **Background:** `Icon_Layer_BackgroundDID`; absent → no icon.
    ///
    /// A layer whose DID is 0 is left out, except the bottom one: a bottom layer of 0 (for
    /// example a Background of 0) means no icon. `Icon_Layer_ImageOverrideDID` and the
    /// other override names are not read (§5.6). `Icon_Layer_LargeImageDID` is a separate
    /// picture, see [`IconStack::large_item_image`].
    ///
    /// Also works for nested socket entries (§5.4) when `lookup` reads the struct value.
    pub fn item(lookup: impl Fn(&str) -> Option<Did>) -> Self {
        Self::item_in_display(lookup, None)
    }

    /// [`IconStack::item`] for a UI element that carries `UI_ItemIconDisplay_OverrideBackground`
    /// (§3.1 step 4, §5.6): with `background_override` `Some`, the item's Background
    /// property is not read; a non-zero override is the Background layer and an override of
    /// 0 adds none (the next layer up becomes the bottom). `None` reads the item's own
    /// Background as [`IconStack::item`] does.
    pub fn item_in_display(
        lookup: impl Fn(&str) -> Option<Did>,
        background_override: Option<Did>,
    ) -> Self {
        let Some(image) = lookup(names::IMAGE_OVERRIDE_CHECKPOINTED)
            .map(|d| (d, names::IMAGE_OVERRIDE_CHECKPOINTED))
            .or_else(|| lookup(names::IMAGE).map(|d| (d, names::IMAGE)))
        else {
            return Self::default();
        };
        let background = match background_override {
            Some(d) if d.0 == 0 => None,
            Some(d) => Some((d, None)),
            None => match lookup(names::BACKGROUND) {
                Some(d) => Some((d, Some(names::BACKGROUND))),
                None => return Self::default(),
            },
        };
        let optional = |name: &'static str| present(&lookup, name).map(|d| (d, Some(name)));
        let layers: [(LayerRole, Option<Layer>); 5] = [
            (LayerRole::Background, background),
            (LayerRole::Underlay, optional(names::UNDERLAY)),
            (LayerRole::Shadow, optional(names::SHADOW)),
            (LayerRole::Image, Some((image.0, Some(image.1)))),
            (LayerRole::Overlay, optional(names::OVERLAY)),
        ];
        let mut entries = Vec::with_capacity(layers.len());
        for (role, layer) in layers {
            let Some((did, property)) = layer else {
                continue;
            };
            if did.0 == 0 {
                if entries.is_empty() {
                    // The bottom layer is 0: no icon (§3.1).
                    return Self::default();
                }
                continue;
            }
            entries.push(StackEntry::new(role, did, property));
        }
        Self { entries }
    }

    /// A trait icon (`icon-layers.md` §5.3): `Trait_Icon`, then
    /// `Trait_Static_Icon_Overlay` above it.
    ///
    /// The rank overlay (`Trait_Rank_Overlay_Progression`) names a progression object,
    /// not a surface; its layout is OPEN, so it is not drawn.
    pub fn trait_icon(lookup: impl Fn(&str) -> Option<Did>) -> Self {
        let mut entries = Vec::with_capacity(2);
        for (role, name) in [
            (LayerRole::TraitIcon, names::TRAIT_ICON),
            (LayerRole::TraitFrame, names::TRAIT_STATIC_OVERLAY),
        ] {
            if let Some(did) = present(&lookup, name) {
                entries.push(StackEntry::new(role, did, Some(name)));
            }
        }
        Self { entries }
    }

    /// A single-surface icon read from one property, such as [`names::SKILL_ICON`] or
    /// [`names::EFFECT_ICON`] (`icon-layers.md` §5.1, §5.2, §5.7).
    pub fn single_property(lookup: impl Fn(&str) -> Option<Did>, name: &'static str) -> Self {
        let entries = present(&lookup, name)
            .map(|did| StackEntry::new(LayerRole::Single, did, Some(name)))
            .into_iter()
            .collect();
        Self { entries }
    }

    /// The large item picture (`Icon_Layer_LargeImageDID`, `icon-layers.md` §5.5), a
    /// single 64×64 layer.
    pub fn large_item_image(lookup: impl Fn(&str) -> Option<Did>) -> Self {
        Self::single_property(lookup, names::LARGE_IMAGE)
    }

    /// A single surface, shown as is. A zero DID gives an empty stack.
    #[must_use]
    pub fn single(did: Did) -> Self {
        let entries = (did.0 != 0)
            .then_some(StackEntry::new(LayerRole::Single, did, None))
            .into_iter()
            .collect();
        Self { entries }
    }

    /// The entries, bottom to top.
    #[must_use]
    pub fn entries(&self) -> &[StackEntry] {
        &self.entries
    }

    /// Whether the stack has no layer ("no icon").
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of layers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The DID of the first entry with `role`.
    #[must_use]
    pub fn did_of(&self, role: LayerRole) -> Option<Did> {
        self.entries.iter().find(|e| e.role == role).map(|e| e.did)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(map: &[(&'static str, u32)]) -> impl Fn(&str) -> Option<Did> + use<> {
        let map: HashMap<&'static str, u32> = map.iter().copied().collect();
        move |n| map.get(n).map(|&v| Did(v))
    }

    #[test]
    fn item_order_and_zero_values() {
        let s = IconStack::item(lookup(&[
            (names::OVERLAY, 0x4100_0005),
            (names::IMAGE, 0x4100_0004),
            (names::UNDERLAY, 0),
            (names::SHADOW, 0x4100_0003),
            (names::BACKGROUND, 0x4100_0001),
            (names::LARGE_IMAGE, 0x4100_0009),
        ]));
        let roles: Vec<_> = s.entries().iter().map(|e| e.role).collect();
        assert_eq!(
            roles,
            [
                LayerRole::Background,
                LayerRole::Shadow,
                LayerRole::Image,
                LayerRole::Overlay
            ]
        );
        assert_eq!(s.did_of(LayerRole::Image), Some(Did(0x4100_0004)));
        assert_eq!(s.did_of(LayerRole::Underlay), None);
    }

    #[test]
    fn client_layer_rules() {
        // §3.1 step 1: the checkpointed override wins whenever present.
        let s = IconStack::item(lookup(&[
            (names::IMAGE, 0x4100_0004),
            (names::IMAGE_OVERRIDE_CHECKPOINTED, 0x4100_00AA),
            (names::BACKGROUND, 0x4100_0001),
        ]));
        assert_eq!(s.did_of(LayerRole::Image), Some(Did(0x4100_00AA)));
        assert_eq!(
            s.entries()[1].property,
            Some(names::IMAGE_OVERRIDE_CHECKPOINTED)
        );
        // An override of 0 gives an icon without a picture.
        let no_pic = IconStack::item(lookup(&[
            (names::IMAGE, 0x4100_0004),
            (names::IMAGE_OVERRIDE_CHECKPOINTED, 0),
            (names::BACKGROUND, 0x4100_0001),
        ]));
        assert_eq!(no_pic.len(), 1);
        assert_eq!(no_pic.did_of(LayerRole::Image), None);
        // §8 missing properties: no Image → no icon; no Background → no icon; a
        // Background of 0 → no icon; an Image of 0 → the other layers only.
        assert!(IconStack::item(lookup(&[(names::BACKGROUND, 0x4100_0001)])).is_empty());
        assert!(IconStack::item(lookup(&[(names::IMAGE, 0x4100_0004)])).is_empty());
        assert!(
            IconStack::item(lookup(&[
                (names::IMAGE, 0x4100_0004),
                (names::BACKGROUND, 0)
            ]))
            .is_empty()
        );
        let zero_image = IconStack::item(lookup(&[
            (names::IMAGE, 0),
            (names::SHADOW, 0x4100_0003),
            (names::BACKGROUND, 0x4100_0001),
        ]));
        assert_eq!(zero_image.len(), 2);
        // §5.6: `Icon_Layer_ImageOverrideDID` and the other overrides are not read.
        assert!(IconStack::item(lookup(&[(names::IMAGE_OVERRIDE, 0x4100_00AB)])).is_empty());
        let ignored = IconStack::item(lookup(&[
            (names::IMAGE, 0x4100_0004),
            (names::IMAGE_OVERRIDE, 0x4100_00AB),
            (names::BACKGROUND, 0x4100_0001),
            (names::BACKGROUND_OVERRIDE, 0x4100_00AC),
        ]));
        assert_eq!(ignored.did_of(LayerRole::Image), Some(Did(0x4100_0004)));
        assert_eq!(
            ignored.did_of(LayerRole::Background),
            Some(Did(0x4100_0001))
        );
        assert!(ignored.entries().iter().all(|e| e.fallback.is_none()));
    }

    #[test]
    fn display_background_override() {
        let item = [
            (names::IMAGE, 0x4100_0004),
            (names::SHADOW, 0x4100_0003),
            (names::BACKGROUND, 0x4100_0001),
        ];
        // A non-zero override replaces the item's Background, which is not read.
        let s = IconStack::item_in_display(lookup(&item), Some(Did(0x4100_0099)));
        assert_eq!(s.did_of(LayerRole::Background), Some(Did(0x4100_0099)));
        let no_bg = IconStack::item_in_display(lookup(&item[..2]), Some(Did(0x4100_0099)));
        assert_eq!(no_bg.len(), 3);
        // An override of 0 adds no Background: the Shadow becomes the bottom.
        let s = IconStack::item_in_display(lookup(&item), Some(Did(0)));
        assert_eq!(s.entries()[0].role, LayerRole::Shadow);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn traits_and_singles() {
        let t = IconStack::trait_icon(lookup(&[
            (names::TRAIT_STATIC_OVERLAY, 0x4100_0002),
            (names::TRAIT_ICON, 0x4100_0001),
        ]));
        assert_eq!(t.entries()[0].role, LayerRole::TraitIcon);
        assert_eq!(t.entries()[1].role, LayerRole::TraitFrame);
        assert!(IconStack::single(Did(0)).is_empty());
        assert_eq!(IconStack::single(Did(0x4100_0001)).len(), 1);
        let sk = IconStack::single_property(lookup(&[(names::SKILL_ICON, 7)]), names::SKILL_ICON);
        assert_eq!(sk.entries()[0].did, Did(7));
        assert!(IconStack::large_item_image(lookup(&[])).is_empty());
        assert!(IconStack::item(lookup(&[])).is_empty());
    }

    #[test]
    fn from_entries_bounds() {
        let e = StackEntry::new(LayerRole::Single, Did(0x4100_0001), None);
        assert_eq!(IconStack::from_entries([e; 3]).unwrap().len(), 3);
        let zero = StackEntry { did: Did(0), ..e };
        assert!(IconStack::from_entries([zero; 40]).unwrap().is_empty());
        // A zero DID with a fallback is replaced by the fallback; a zero fallback is
        // dropped.
        let promoted = StackEntry {
            role: LayerRole::Image,
            did: Did(0),
            property: Some(names::IMAGE_OVERRIDE),
            fallback: Some(Did(0x4100_0002)),
        };
        let zero_fb = StackEntry {
            fallback: Some(Did(0)),
            ..e
        };
        let s = IconStack::from_entries([promoted, zero_fb]).unwrap();
        assert_eq!(s.entries()[0].did, Did(0x4100_0002));
        assert_eq!(s.entries()[0].property, Some(names::IMAGE));
        assert_eq!(s.entries()[1].fallback, None);
        // An endless input is cut off; the count is a lower bound.
        assert!(matches!(
            IconStack::from_entries(std::iter::repeat(e)),
            Err(MaterialError::TooManyLayers { count, limit: MAX_STACK_LAYERS })
                if count == MAX_STACK_LAYERS + 1
        ));
        let msg = IconStack::from_entries([e; 40]).unwrap_err().to_string();
        assert!(msg.contains("at least 17"), "{msg}");
        // An endless input of dropped entries (zero DID, no fallback) returns too.
        let dropped = StackEntry::new(LayerRole::Single, Did(0), None);
        assert!(matches!(
            IconStack::from_entries(std::iter::repeat(dropped)),
            Err(MaterialError::TooManyLayers { count, limit: MAX_STACK_LAYERS })
                if count == MAX_STACK_INPUT + 1
        ));
        // Up to the input bound, dropped entries are only skipped.
        let mut some: Vec<StackEntry> = vec![dropped; MAX_STACK_INPUT - 1];
        some.push(e);
        assert_eq!(IconStack::from_entries(some).unwrap().entries().len(), 1);
    }
}
