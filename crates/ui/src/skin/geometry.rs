// SPDX-License-Identifier: GPL-3.0-or-later
//! Panel geometry overrides (`skin-definition.md` §8; D19, D28).
//!
//! The element tree calls [`apply_override`] on the **per-build resolved copy** of each
//! element's descriptor before it creates the element (§8.2): it finds the override with
//! [`super::SkinOverlay::find_override`] (or [`super::Definition::find_override`]) from
//! the element's ID and the IDs of the elements enclosing it within the current build, and
//! applies it. Step 5 of §8.4 reaches the children inside the copy, and each child is
//! matched and overridden again when it is itself built (D19). Shared or cached layout
//! data is never touched.
//!
//! The plugin API's game-styled controls take no top-level override (D28); the tree simply
//! does not look one up for them.

use tessera_formats::ui_layout::{EdgeMode, Edges, ElementDesc, Rect};
use tessera_formats::{PropertyEntry, PropertySet, PropertyValue};

use super::definition::ElementOverride;
use super::tables::GeometryPids;

fn is_true(set: &PropertySet, pid: Option<u32>) -> bool {
    pid.and_then(|p| set.get(p))
        .is_some_and(|v| matches!(v, PropertyValue::Bool(true)))
}

fn set_int(set: &mut PropertySet, pid: u32, value: i32) {
    if let Some(e) = set.entries.iter_mut().find(|e| e.pid == pid) {
        e.value = PropertyValue::Int(value);
    } else {
        set.entries.push(PropertyEntry {
            pid,
            value: PropertyValue::Int(value),
        });
        set.size.count = set.size.count.saturating_add(1);
    }
}

/// Applies `ov` to the descriptor `desc` (§8.4). Nothing happens unless their element IDs
/// are equal. Returns whether it applied.
///
/// 1. X and Y, unless `UISkin_CannotBeMoved` is true in `desc`'s own property set;
/// 2. Width and Height (also when 0), unless `UISkin_CannotBeResized` is true;
/// 3. all four edge modes to 0, unless `UISkin_CannotBeDetached` is true;
/// 4. the two UI-scaler origin properties, when the override has a scaler origin;
/// 5. recursively, each child of `desc` whose ID is a key of the override's children.
///
/// A property absent or not boolean counts as false. Nothing else is written: not
/// `has_position`, the z-level, the flags, the ID, the type, the default state, the states
/// or the media lists (SP15).
pub fn apply_override(desc: &mut ElementDesc, ov: &ElementOverride, pids: &GeometryPids) -> bool {
    if desc.element_id != ov.element_id {
        return false;
    }
    let props = &desc.block.properties;
    let moved = !is_true(props, pids.cannot_be_moved);
    let resized = !is_true(props, pids.cannot_be_resized);
    let detached = !is_true(props, pids.cannot_be_detached);
    let rect = &mut desc.block.rect;
    if moved {
        rect.x = ov.x;
        rect.y = ov.y;
    }
    if resized {
        rect.width = ov.width;
        rect.height = ov.height;
    }
    if detached {
        desc.edges = Edges {
            left: EdgeMode(0),
            top: EdgeMode(0),
            right: EdgeMode(0),
            bottom: EdgeMode(0),
        };
    }
    if let Some((ox, oy)) = ov.scale_origin {
        if let Some(p) = pids.scaler_origin_x {
            set_int(&mut desc.block.properties, p, ox);
        }
        if let Some(p) = pids.scaler_origin_y {
            set_int(&mut desc.block.properties, p, oy);
        }
    }
    if !ov.children().is_empty() {
        for child in &mut desc.children {
            if let Some(c) = ov.child(child.element_id) {
                apply_override(child, c, pids);
            }
        }
    }
    true
}

/// The rectangle an override gives a descriptor without protections (for callers that keep
/// geometry outside an [`ElementDesc`]).
#[must_use]
pub fn override_rect(ov: &ElementOverride) -> Rect {
    Rect {
        x: ov.x,
        y: ov.y,
        width: ov.width,
        height: ov.height,
    }
}

/// Inputs of a UI-scaler scale change (§8.4, effect of the scaler origin; §12.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleChange {
    /// Current position (x, y) of the element.
    pub position: (i32, i32),
    /// The scaler origin (`UIComponent_UIScaler_Origin_X/Y`, 0 when absent; (0, 0) when
    /// the caller asks for the origin to be ignored, OPEN-13).
    pub origin: (i32, i32),
    /// The cached scale *s*₀ (`UIComponent_UIScaler_Cached_Scale`, 1.0 when absent).
    pub old_scale: f32,
    /// The new scale *s*₁.
    pub new_scale: f32,
    /// The element's new (scaled, limited) size.
    pub new_size: (i32, i32),
    /// The screen size.
    pub screen: (i32, i32),
}

/// The element's new position after a scale change (§8.4 steps 3–4): the origin keeps its
/// screen position, products truncated toward zero in 32-bit floating point, then the
/// position is clamped to the screen (right/bottom first, then 0).
///
/// The scales come from data and settings, so any `f32` is accepted without panicking: a
/// product beyond the 64-bit range saturates, NaN counts as 0, and the sums saturate
/// before the clamp. What the original does with a non-finite or huge cached scale is
/// open (`skin-definition.md` §8.4, OPEN note).
#[must_use]
pub fn scale_about_origin(c: &ScaleChange) -> (i32, i32) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let t = |o: i32, s: f32| -> i64 { ((o as f32) * s).trunc() as i64 };
    let axis = |pos: i32, origin: i32, size: i32, screen: i32| -> i32 {
        let moved = i64::from(pos)
            .saturating_add(t(origin, c.old_scale))
            .saturating_sub(t(origin, c.new_scale));
        let clamped = moved.min(i64::from(screen) - i64::from(size)).max(0);
        i32::try_from(clamped).unwrap_or(i32::MAX)
    };
    (
        axis(c.position.0, c.origin.0, c.new_size.0, c.screen.0),
        axis(c.position.1, c.origin.1, c.new_size.1, c.screen.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worked_example_12_8() {
        let c = ScaleChange {
            position: (448, 980),
            origin: (512, 100),
            old_scale: 1.0,
            new_scale: 1.25,
            new_size: (1280, 125),
            screen: (1920, 1080),
        };
        assert_eq!(scale_about_origin(&c), (320, 955));
        let c = ScaleChange {
            position: (1000, 1000),
            origin: (0, 0),
            ..c
        };
        assert_eq!(scale_about_origin(&c), (640, 955));
    }

    #[test]
    fn hostile_scales_and_positions_do_not_overflow() {
        let base = ScaleChange {
            position: (10, 10),
            origin: (1000, 1000),
            old_scale: 1.0,
            new_scale: 1.0,
            new_size: (100, 100),
            screen: (1920, 1080),
        };
        let scales = [
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            1e30,
            -1e30,
            f32::MAX,
            f32::MIN,
            0.0,
            -1.0,
        ];
        let positions = [(i32::MIN, i32::MIN), (i32::MAX, i32::MAX), (0, 0)];
        let origins = [(i32::MIN, i32::MAX), (i32::MAX, i32::MIN), (1000, 1000)];
        let sizes = [(i32::MIN, i32::MIN), (i32::MAX, i32::MAX), (100, 100)];
        for &old_scale in &scales {
            for &new_scale in &scales {
                for &position in &positions {
                    for &origin in &origins {
                        for &new_size in &sizes {
                            let c = ScaleChange {
                                position,
                                origin,
                                old_scale,
                                new_scale,
                                new_size,
                                ..base
                            };
                            let (x, y) = scale_about_origin(&c);
                            assert!(x >= 0 && y >= 0, "{c:?}");
                        }
                    }
                }
            }
        }
        // The probe of the review: +inf cached scale, ordinary rest.
        let c = ScaleChange {
            old_scale: f32::INFINITY,
            ..base
        };
        assert_eq!(scale_about_origin(&c), (1820, 980));
        // NaN products count as 0: x' = 10 + 0 - 1000, clamped to 0.
        let c = ScaleChange {
            old_scale: f32::NAN,
            ..base
        };
        assert_eq!(scale_about_origin(&c), (0, 0));
    }
}
