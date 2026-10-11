// SPDX-License-Identifier: GPL-3.0-or-later
//! Quest indicators over NPC figures: a small world-anchored overlay standing in for the
//! ring scripts of `quest-journal.md` §8.1–§8.2.
//!
//! The original runs a script key (an FX-mapper label such as `Quest_Available`) on the
//! NPC, whose particle system draws the ring at an attach point of the figure. The key
//! choice is `tessera-gamedata`'s ([`super::present::JournalView::ring`], §8.2 Tessera
//! rule). Until the walker starts script keys on entities, this module draws one glyph per
//! ringed NPC above its position ([`ANCHOR_HEIGHT`]), projected with the walker's camera:
//!
//! - [`indicators`]: the ringed entities with their key, kind and world anchor;
//! - [`project`]: a world point to a screen pixel through a view-projection matrix;
//! - [`IndicatorOverlay`]: the glyph textures (the walker HUD's bitmap font) and the draw.
//!
//! **Wiring.** [`crate::quests::QuestHub`] keeps the ring list (recomputed when the
//! journal frame reports `rings_changed`), anchors it each frame at the walker's
//! render-space entity positions and draws it after the walker's scene with the walker's
//! view-projection matrix (`app.rs`). P2 builds floating text over entities in parallel;
//! its world-anchored overlay can absorb [`IndicatorOverlay`] at merge (the anchor and the
//! projection are the same problem). Starting the ring's script key on the figure
//! (`particle-runtime.md`) replaces the glyph once the walker's effect layer takes keys for
//! entities.
//!
//! Rings show only while the overhead quest icons setting is on (§8.2; default on): the
//! client's `--quest-icons on|off` (not yet a saved setting, §8.2 OPEN).

use tessera_net::EntityId;
use tessera_render::glam::{Mat4, Vec3, Vec4};
use tessera_render::{Pass, Renderer};

use super::present::JournalView;
use crate::character::PlayerState;
use crate::walker::hud::TextPanel;

/// Height of the indicator above the entity's position, in metres (Tessera rule; the
/// original's attach point is the script's own).
pub const ANCHOR_HEIGHT: f32 = 2.4;
/// Most indicators drawn in one frame (Tessera bound).
pub const MAX_INDICATORS: usize = 256;

/// What a ring key marks (by its FX-mapper label, §8.1 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RingKind {
    /// `Quest_Available…`, `Quest_Crafting`, `Quest_Item_Advancement`: a quest to take.
    Available,
    /// `Quest_Advance…`: the quest's next step is with this NPC.
    Advance,
    /// `Quest_InProgress`: the quest is underway elsewhere.
    InProgress,
    /// Any other ring key.
    Other,
}

impl RingKind {
    /// The kind of a ring key label.
    #[must_use]
    pub fn of(key: &str) -> Self {
        if key.starts_with("Quest_Available")
            || key == "Quest_Crafting"
            || key == "Quest_Item_Advancement"
        {
            Self::Available
        } else if key.starts_with("Quest_Advance") {
            Self::Advance
        } else if key == "Quest_InProgress" {
            Self::InProgress
        } else {
            Self::Other
        }
    }

    /// The glyph and its colour (Tessera's own stand-in art).
    #[must_use]
    pub fn glyph(self) -> (&'static str, [u8; 3]) {
        match self {
            Self::Available => ("!", [255, 214, 64]),
            Self::Advance => ("?", [255, 214, 64]),
            Self::InProgress => ("?", [170, 170, 170]),
            Self::Other => ("*", [200, 200, 255]),
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Available => 0,
            Self::Advance => 1,
            Self::InProgress => 2,
            Self::Other => 3,
        }
    }
}

/// One ringed NPC.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Indicator {
    /// The entity.
    pub entity: EntityId,
    /// The ring's script key (FX-mapper label).
    pub key: &'static str,
    /// Its kind.
    pub kind: RingKind,
    /// The world anchor (the entity's position raised by [`ANCHOR_HEIGHT`]), when the
    /// entity's position is known.
    pub anchor: Option<Vec3>,
}

/// The ringed NPCs of `view` (§8.2), with the anchor from `position` (the entity's
/// position in the caller's render space), at most [`MAX_INDICATORS`].
#[must_use]
pub fn indicators(
    view: &JournalView<'_>,
    state: &PlayerState,
    position: &dyn Fn(EntityId) -> Option<Vec3>,
) -> Vec<Indicator> {
    anchored(&view.rings(state), position)
}

/// The indicators of `rings` (entity and ring key, [`JournalView::rings`]) anchored at
/// `position`, at most [`MAX_INDICATORS`].
#[must_use]
pub fn anchored(
    rings: &[(EntityId, &'static str)],
    position: &dyn Fn(EntityId) -> Option<Vec3>,
) -> Vec<Indicator> {
    rings
        .iter()
        .take(MAX_INDICATORS)
        .map(|&(entity, key)| Indicator {
            entity,
            key,
            kind: RingKind::of(key),
            anchor: position(entity).map(|p| p + Vec3::Z * ANCHOR_HEIGHT),
        })
        .collect()
}

/// The screen pixel of world point `p` (top-left origin), or `None` behind the camera or
/// outside the clip volume.
#[must_use]
pub fn project(view_proj: Mat4, p: Vec3, size: (u32, u32)) -> Option<(f32, f32)> {
    let c: Vec4 = view_proj * p.extend(1.0);
    if c.w <= 1e-6 {
        return None;
    }
    let n = c.truncate() / c.w;
    if !(-1.0..=1.0).contains(&n.x) || !(-1.0..=1.0).contains(&n.y) || !(0.0..=1.0).contains(&n.z) {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let (w, h) = (size.0 as f32, size.1 as f32);
    Some(((n.x + 1.0) * 0.5 * w, (1.0 - n.y) * 0.5 * h))
}

/// The glyph textures and their draw (module docs).
#[derive(Debug, Default)]
pub struct IndicatorOverlay {
    glyphs: [TextPanel; 4],
    scale: u32,
}

impl IndicatorOverlay {
    /// Uploads the glyphs at `scale` (1–4) when not yet uploaded at it.
    pub fn prepare(&mut self, renderer: &mut Renderer<'_>, scale: u32) {
        let scale = scale.clamp(1, 4);
        for k in [
            RingKind::Available,
            RingKind::Advance,
            RingKind::InProgress,
            RingKind::Other,
        ] {
            let (g, c) = k.glyph();
            self.glyphs[k.index()].set(renderer, vec![(g.to_owned(), c)], scale);
        }
        self.scale = scale;
    }

    /// Draws each indicator with an anchor at its projected pixel (centred above it).
    pub fn draw(&self, pass: &mut Pass, view_proj: Mat4, size: (u32, u32), list: &[Indicator]) {
        for i in list {
            let Some(a) = i.anchor else { continue };
            let Some((x, y)) = project(view_proj, a, size) else {
                continue;
            };
            let g = &self.glyphs[i.kind.index()];
            #[allow(clippy::cast_precision_loss)]
            if let Some((w, h)) = g.size() {
                g.draw(pass, x - w as f32 / 2.0, y - h as f32);
            }
        }
    }

    /// Frees the textures.
    pub fn release(&mut self, renderer: &mut Renderer<'_>) {
        for g in &mut self.glyphs {
            g.release(renderer);
        }
        self.scale = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_kinds_by_label() {
        assert_eq!(
            RingKind::of("Quest_Available_Epic_Hard"),
            RingKind::Available
        );
        assert_eq!(RingKind::of("Quest_Crafting"), RingKind::Available);
        assert_eq!(RingKind::of("Quest_Advance_Mission"), RingKind::Advance);
        assert_eq!(RingKind::of("Quest_InProgress"), RingKind::InProgress);
        assert_eq!(RingKind::of("Quest_BelowLevel"), RingKind::Other);
    }

    #[test]
    fn projection_centres_the_look_target_and_drops_points_behind() {
        let vp = tessera_render::Camera {
            eye: Vec3::ZERO,
            target: Vec3::Y,
            up: Vec3::Z,
            projection: tessera_render::Perspective {
                fov_y: 1.0,
                near: 0.1,
                far: 100.0,
            },
        }
        .view_proj(2.0);
        let (x, y) = project(vp, Vec3::new(0.0, 10.0, 0.0), (200, 100)).expect("in view");
        assert!((x - 100.0).abs() < 0.01 && (y - 50.0).abs() < 0.01);
        // Above the target is higher on the screen.
        let (_, y2) = project(vp, Vec3::new(0.0, 10.0, 1.0), (200, 100)).expect("in view");
        assert!(y2 < y);
        assert_eq!(project(vp, Vec3::new(0.0, -10.0, 0.0), (200, 100)), None);
        assert_eq!(project(vp, Vec3::new(1000.0, 10.0, 0.0), (200, 100)), None);
    }
}
