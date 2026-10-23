// SPDX-License-Identifier: GPL-3.0-or-later
//! UI scaling: the geometry foundation of `docs/specs/ui-scaling.md` (decision 0040).
//!
//! Decision 0040 makes Tessera's UI sizing and placement its own: the layout records, element
//! trees and art still come from the user's install unchanged, but *how* they are sized and
//! placed on the screen is decided here, not copied from the original. This module is the
//! pure, engine-neutral core that the renderer, the plugin host and the client build on. It
//! holds no element tree, no canvas and no window; it is arithmetic over the spaces of
//! `ui-scaling.md` §3–§6. Nothing in it decodes hostile bytes: every entry point takes numbers
//! and clamps non-finite or out-of-range input to a safe value, so it never panics.
//!
//! | Submodule | Spec | What |
//! |---|---|---|
//! | [`screen`] | §3.2, §3.6, §4.2, §4.3 | the effective scale, the auto default, the UI screen *F*, the axis factors, the pixel-centre mapping |
//! | [`dbox`] | §3.4 | the displayed box, a half-open rectangle in UI units as real numbers |
//! | [`safe_area`](mod@safe_area) | §5.1 | the safe area *A* for wide screens |
//! | [`fit`] | §5.4, §5.6, §5.7 | fit containers (Types A/B/C), fit groups, letterbox bars, fit-to-screen, the degenerate-box guard |
//! | [`placement`] | §5.8 | keep-on-screen and anchored saved placements |
//! | [`glyph`] | §6.4, §6.5, §6.7 | the target glyph height, font ladders, derived metrics, the glyph cache, the cursor size |
//! | [`image`] | §6.1, §6.3 | canvas pixel sizes, per-axis image kind and scale, the sharp-bilinear and area filters |
//!
//! At `Se` = 1 with every node scale 1 the mappings here are the identity on integers
//! (`ui-scaling.md` invariants I2, I7): a displayed box equals its layout box made absolute,
//! the pixel-centre mapping is `x ↦ x`, and the image and glyph paths select the exact 1 : 1
//! copy. So the module can be adopted incrementally without changing any output at scale 1.

pub mod dbox;
pub mod fit;
pub mod glyph;
pub mod image;
pub mod placement;
pub mod safe_area;
pub mod screen;

pub use dbox::DBox;
pub use fit::{
    ComputedBox, ContentBox, Fit, FitContainer, FitMember, MAX_BAR_RECTS, RHO,
    VerticalAnchor as FitVerticalAnchor, degenerate_guard, fit_to_screen, fit_type_a, fit_type_b,
    fit_type_c, group_box, letterbox_rects, safe_area_in_parent,
};
pub use glyph::{
    DerivedGlyph, GlyphCache, GlyphKey, GlyphMetrics, GlyphSource, SourceKind, choose_glyph_source,
    cursor_factor, cursor_hotspot, cursor_size, derive_metrics, distance_coverage,
    downsample_coverage, target_height,
};
pub use image::{AxisKind, area_weights, axis_kind, axis_scale, canvas_px, sharp_bilinear};
pub use placement::{Anchor, RefBox, SavedPlacement, anchor_for, apply_anchor, keep_on_screen};
pub use safe_area::safe_area;
pub use screen::{UiScreen, edge_map, effective_scale, round_scale, s_auto};

/// The letterbox bars of one fit group in the live tree (`ui-scaling.md` §5.4 step 6), as
/// [`crate::Ui::letterbox_bars`] reports them: opaque black fills in **physical pixels**,
/// clipped to the window, drawn once, directly below the picture of the group's lowest shown
/// member `below` in the drawing order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetterboxBars {
    /// The group's lowest shown member: the bars are drawn directly below its picture.
    pub below: crate::ElementKey,
    /// Disjoint half-open rectangles in physical pixels ([`letterbox_rects`]).
    pub rects: Vec<crate::paint::Rect>,
}
