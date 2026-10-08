// SPDX-License-Identifier: GPL-3.0-or-later
//! A figure's shape inputs bound to its skeleton: the one path from
//! [`tessera_mesh::assembly::shape::FigureShape`] (held proportion clips, body-shape
//! deformer clips, bone sliders; `appearance.md` §6.2–§6.4, `script-table.md` §6.7–§6.9) to
//! held playbacks and a [`BindDeformation`]. The client and the explorer both call
//! [`bind_shape`], so they show the same figure; only how a clip DID becomes a
//! [`BoundClip`] (their clip caches) differs, and that is the `bind` closure.

use std::fmt::Display;
use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::script_table::resolve::held_clip_time;
use tessera_mesh::assembly::shape::{FigureShape, HeldClip};

use crate::clip::BoundClip;
use crate::deform::{BindDeformation, DeformClip};
use crate::skeleton::Skeleton;
use crate::slider::BoneSliders;

/// Most held clips bound per figure: a Tessera bound, not a client rule (`script-table.md`
/// §6.7: the client has no limit; §16 answer T40). The install has at most one per avatar,
/// on layer 110; a held clip beyond the bound is dropped and logged.
pub const MAX_HELD_CLIPS: usize = 4;

/// Most notes [`bind_shape`] reports.
const MAX_NOTES: usize = 8;

/// A held proportion clip bound to the figure's skeleton (`script-table.md` §6.7, §6.8).
#[derive(Debug, Clone)]
pub struct BoundHeldClip {
    /// What the shape inputs say: clip, layer, held fraction, track opacity, driver.
    pub held: HeldClip,
    /// The clip, bound.
    pub clip: Arc<BoundClip>,
    /// The fraction of the clip's whole duration it is shown at: the clip time of
    /// `script-table.md` §6.7 step 2 (the held fraction × the played length, duration −
    /// end crop, limited to that length) divided by the duration; 0 for a clip without a
    /// usable duration. [`crate::Playback::held`] takes it.
    pub clip_fraction: f32,
}

/// The fraction of `clip`'s duration at which held clip `h` is shown (`script-table.md`
/// §6.7 step 2; see [`BoundHeldClip::clip_fraction`]).
#[must_use]
pub fn held_clip_fraction(h: &HeldClip, clip: &BoundClip) -> f32 {
    let d = clip.duration();
    if !(d.is_finite() && d > 0.0) {
        return 0.0;
    }
    (held_clip_time(h.fraction, d, h.end_crop) / d).clamp(0.0, 1.0)
}

/// [`bind_shape`]'s result.
#[derive(Debug, Clone, Default)]
pub struct BoundShape {
    /// Held clips in the order of [`FigureShape::held`] (lowest layer first), at most
    /// [`MAX_HELD_CLIPS`].
    pub held: Vec<BoundHeldClip>,
    /// The bind-space deformation of the skinned parts (`appearance.md` §6.4); `None` when
    /// the figure has no body-shape clip and no slider, nothing fits, or it changes no bone
    /// by more than 1e-6.
    pub deformation: Option<BindDeformation>,
    /// Clips that did not bind, sliders that did not fit, a deformation that failed.
    pub notes: Vec<String>,
}

/// Binds the shape inputs of a figure to `skeleton` (`appearance.md` §6.4,
/// `script-table.md` §6.7–§6.9):
///
/// - **Held clips**: each of [`FigureShape::held`] (at most [`MAX_HELD_CLIPS`]; the others
///   are dropped and logged) bound with `bind`, shown at [`held_clip_fraction`].
/// - **Body shape**: each of [`FigureShape::body_shape`] bound with `bind` as a
///   [`DeformClip`] at its fraction (the deformation limits it to [0, 1]).
/// - **Sliders**: every definition of [`FigureShape::sliders`] at its position
///   ([`tessera_mesh::assembly::shape::FigureSliders::positions`], the A16 rule), built into
///   [`BoneSliders`] ([`BoneSliders::build`]: a bone outside the skeleton loses only its
///   own entry, a slider with values that are not finite none; both are noted).
///
/// The deformation is built from the body-shape clips that bound and the sliders.
pub fn bind_shape<E: Display>(
    shape: &FigureShape,
    skeleton: &Arc<Skeleton>,
    mut bind: impl FnMut(Did) -> Result<Arc<BoundClip>, E>,
) -> BoundShape {
    let mut out = BoundShape::default();
    if shape.held.len() > MAX_HELD_CLIPS {
        let dropped = shape.held.len() - MAX_HELD_CLIPS;
        tracing::warn!(
            dropped,
            kept = MAX_HELD_CLIPS,
            "held clips beyond the Tessera bound dropped"
        );
        note(
            &mut out.notes,
            format!("{dropped} held clips beyond {MAX_HELD_CLIPS} dropped"),
        );
    }
    for h in shape.held.iter().take(MAX_HELD_CLIPS) {
        match bind(h.clip) {
            Ok(clip) => out.held.push(BoundHeldClip {
                held: h.clone(),
                clip_fraction: held_clip_fraction(h, &clip),
                clip,
            }),
            Err(e) => note(&mut out.notes, format!("held clip {}: {e}", h.clip)),
        }
    }
    if shape.body_shape.len() > BindDeformation::MAX_CLIPS {
        let dropped = shape.body_shape.len() - BindDeformation::MAX_CLIPS;
        tracing::warn!(
            dropped,
            kept = BindDeformation::MAX_CLIPS,
            "body-shape clips beyond the Tessera bound dropped"
        );
        note(
            &mut out.notes,
            format!(
                "{dropped} body-shape clips beyond {} dropped",
                BindDeformation::MAX_CLIPS
            ),
        );
    }
    let mut clips = Vec::new();
    for b in shape.body_shape.iter().take(BindDeformation::MAX_CLIPS) {
        match bind(b.clip) {
            Ok(clip) => clips.push(DeformClip {
                clip,
                fraction: b.fraction,
                use_bone_weights: b.use_bone_weights,
            }),
            Err(e) => note(&mut out.notes, format!("body shape {}: {e}", b.clip)),
        }
    }
    let sliders = shape.sliders.as_ref().map(|s| {
        let (sliders, errors) = BoneSliders::build(
            skeleton,
            s.table
                .sliders
                .iter()
                .zip(&s.positions)
                .map(|(d, &t)| (d.channels, d.min, d.max, t, d.bones.as_slice())),
        );
        if let Some(e) = errors.first() {
            note(
                &mut out.notes,
                format!("{} slider problems (first: {e})", errors.len()),
            );
        }
        sliders
    });
    let sliders = sliders.filter(|s| !s.is_empty());
    if clips.is_empty() && sliders.is_none() {
        return out;
    }
    match BindDeformation::new(skeleton, &clips, sliders.as_ref()) {
        Ok(d) => out.deformation = (!d.is_identity(1e-6)).then_some(d),
        Err(e) => note(&mut out.notes, format!("bind-space deformation: {e}")),
    }
    out
}

/// Adds a note while fewer than [`MAX_NOTES`] are kept.
fn note(notes: &mut Vec<String>, n: String) {
    if notes.len() < MAX_NOTES {
        notes.push(n);
    }
}

#[cfg(test)]
mod tests {
    use tessera_formats::Did;
    use tessera_havok::Transform;
    use tessera_mesh::assembly::shape::{
        FigureSliders, ShapeDriver, SliderSpec, SliderTable, channel,
    };

    use super::*;

    fn leg() -> Arc<Skeleton> {
        let t = |z: f32| Transform {
            translation: [0.0, 0.0, z],
            ..Transform::IDENTITY
        };
        Arc::new(Skeleton::from_parts(&[-1, 0, 1], &[t(1.0), t(-0.45), t(-0.41)]).unwrap())
    }

    fn fixed_scale(z: f32, bone: i64) -> SliderSpec {
        SliderSpec {
            name: 0,
            group: 0,
            visible: false,
            channels: channel::SCALE_Z,
            min: [1.0, 1.0, z],
            max: [1.0, 1.0, z],
            bones: vec![(bone, false)],
        }
    }

    /// `appearance.md` §12.5 through the shared path: the two fixed leg sliders deform the
    /// sole vertex to 0.0112; a held clip that cannot be bound is noted, not fatal.
    #[test]
    fn shape_inputs_become_the_worked_deformation() {
        let s = leg();
        let shape = FigureShape {
            held: vec![HeldClip {
                clip: Did(0x0500_0001),
                layer: 110,
                fraction: 0.0,
                end_crop: 0.0,
                use_bone_opacity: true,
                driver: Some(ShapeDriver::AvatarHeight),
            }],
            sliders: Some(FigureSliders {
                table: SliderTable {
                    record: Did(0x7800_0001),
                    sliders: vec![fixed_scale(0.8, 0), fixed_scale(1.15, 1)],
                    unknown_channel_bits: 0,
                },
                values: Vec::new(),
                positions: vec![0.5, 0.5],
            }),
            ..FigureShape::default()
        };
        let bound = bind_shape(&shape, &s, |d| Err::<Arc<BoundClip>, _>(format!("no {d}")));
        assert!(bound.held.is_empty());
        assert_eq!(bound.notes.len(), 1, "{:?}", bound.notes);
        let d = bound.deformation.expect("deformation");
        let mut pos = [[0.0, 0.0, 0.0]];
        d.deform(
            &mut pos,
            None,
            None,
            &[[2, 0, 0, 0]],
            &[[1.0, 0.0, 0.0, 0.0]],
        );
        assert!((pos[0][2] - 0.0112).abs() < 1e-5, "{pos:?}");
    }

    /// No inputs, or sliders that change nothing: no deformation.
    #[test]
    fn identity_shapes_have_no_deformation() {
        let s = leg();
        let none = bind_shape(&FigureShape::default(), &s, |_| {
            Err::<Arc<BoundClip>, _>("unused")
        });
        assert!(none.deformation.is_none() && none.notes.is_empty());
        let shape = FigureShape {
            sliders: Some(FigureSliders {
                table: SliderTable {
                    record: Did(0x7800_0001),
                    sliders: vec![fixed_scale(1.0, 0), fixed_scale(1.2, 9)],
                    unknown_channel_bits: 0,
                },
                values: Vec::new(),
                positions: vec![0.5, 0.5],
            }),
            ..FigureShape::default()
        };
        let b = bind_shape(&shape, &s, |_| Err::<Arc<BoundClip>, _>("unused"));
        assert!(b.deformation.is_none());
        assert_eq!(b.notes.len(), 1, "bone 9 is outside the skeleton");
    }
}
