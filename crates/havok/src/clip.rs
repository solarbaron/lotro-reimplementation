// SPDX-License-Identifier: GPL-3.0-or-later
//! Animation clips: the 0x05 record envelope (`havok-data.md` §2.3), the animation objects
//! (§4.3, §5, §6), annotation tracks (§4.4), extracted-motion reference frames (§4.5), and
//! sampling (§7.4–§7.8).

use crate::error::{HavokError, Result};
use crate::skeleton::Skeleton;
use crate::spline::{BlockParams, SplineAnimation, SplineBlock, clamp_time};
use std::sync::Arc;

use crate::tagfile::{Object, SharedStrings, StrId, Tagfile, Value, latin1};
use crate::transform::{Transform, quat_nlerp};
use crate::{KNOWN_CLASSES, class_error};

/// Class name of a spline-compressed animation.
pub const SPLINE_CLASS: &str = "hkaSplineCompressedAnimation";
/// Class name of an uncompressed interleaved animation.
pub const INTERLEAVED_CLASS: &str = "hkaInterleavedUncompressedAnimation";
/// Class name of an extracted-motion reference frame.
pub const REFERENCE_FRAME_CLASS: &str = "hkaDefaultAnimatedReferenceFrame";

/// Bytes of a record's envelope, split into its fields (§2.3). Tagfiles are not decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope<'a> {
    /// Field 1: clip length in seconds as stored in the envelope.
    pub duration: f32,
    /// Field 2: transform track count.
    pub track_count: u32,
    /// Field 4: the leading reference-frame tagfile, when field 3 is 1.
    pub reference_frame: Option<&'a [u8]>,
    /// Field 6: the clip tagfile.
    pub tagfile: &'a [u8],
    /// Field 8: per-track weights (empty or one per track).
    pub track_weights: Vec<f32>,
}

struct EnvReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> EnvReader<'a> {
    fn bytes(&mut self, n: usize, what: &'static str) -> Result<&'a [u8]> {
        if n > self.data.len() - self.pos {
            return Err(HavokError::EnvelopeTruncated {
                offset: self.pos,
                what,
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u32(&mut self, what: &'static str) -> Result<u32> {
        let b = self.bytes(4, what)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self, what: &'static str) -> Result<f32> {
        Ok(f32::from_bits(self.u32(what)?))
    }

    /// VLE (`records.md` §1.2).
    fn vle(&mut self, what: &'static str) -> Result<u32> {
        let a = self.bytes(1, what)?[0];
        Ok(match a {
            0x00..=0x7F => u32::from(a),
            0x80..=0xBF => (u32::from(a & 0x3F) << 8) | u32::from(self.bytes(1, what)?[0]),
            0xE0 => self.u32(what)?,
            _ => {
                let b = self.bytes(3, what)?;
                (u32::from(a & 0x3F) << 24)
                    | (u32::from(b[0]) << 16)
                    | u32::from(u16::from_le_bytes([b[1], b[2]]))
            }
        })
    }
}

impl<'a> Envelope<'a> {
    /// Splits a 0x05 record into its envelope fields; the record must end exactly after the
    /// weight array.
    pub fn parse(record: &'a [u8]) -> Result<Self> {
        let mut r = EnvReader {
            data: record,
            pos: 0,
        };
        let duration = r.f32("duration")?;
        let track_count = r.u32("track count")?;
        let flag = r.bytes(1, "reference-frame flag")?[0];
        let reference_frame = match flag {
            0 => None,
            1 => {
                let len = r.u32("reference-frame length")? as usize;
                Some(r.bytes(len, "reference-frame tagfile")?)
            }
            value => return Err(HavokError::EnvelopeBadFlag { value }),
        };
        let len = r.u32("tagfile length")? as usize;
        let tagfile = r.bytes(len, "clip tagfile")?;
        let w = r.vle("weight count")?;
        if w != 0 && w != track_count {
            return Err(HavokError::EnvelopeWeightCount {
                count: w,
                tracks: track_count,
            });
        }
        let raw = r.bytes((w as usize).saturating_mul(4), "track weights")?;
        let track_weights = raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        if r.pos != record.len() {
            return Err(HavokError::EnvelopeTrailingBytes {
                offset: r.pos,
                count: record.len() - r.pos,
            });
        }
        Ok(Envelope {
            duration,
            track_count,
            reference_frame,
            tagfile,
            track_weights,
        })
    }
}

/// One timed annotation (never present in this install, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// Time in seconds.
    pub time: f32,
    /// Text bytes, if present.
    pub text: Option<Vec<u8>>,
}

/// An annotation track: a name (belonging to the transform track of the same index) and its
/// annotations (§4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationTrack {
    /// Track name bytes, if present. Tracks naming the same pool string share one allocation.
    pub name: Option<Arc<[u8]>>,
    /// Timed annotations; always empty in the observed data.
    pub annotations: Vec<Annotation>,
}

/// An `hkaDefaultAnimatedReferenceFrame` (§4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceFrame {
    /// Up axis (x, y, z, w).
    pub up: [f32; 4],
    /// Forward axis (x, y, z, w).
    pub forward: [f32; 4],
    /// Duration in seconds.
    pub duration: f32,
    /// Stored component count of the samples (3 or 4; 0 when the array is absent).
    pub components: u8,
    /// One sample per frame: x, y, z translation and w (missing components read 0).
    pub samples: Vec<[f32; 4]>,
}

impl ReferenceFrame {
    /// Builds the reference frame from an object of class
    /// `hkaDefaultAnimatedReferenceFrame`.
    pub fn from_object(tf: &Tagfile<'_>, obj: &Object<'_>) -> Result<Self> {
        let c = obj.class;
        let v = &obj.values;
        let l = REFERENCE_FRAME_CLASS;
        let vec4 = |name| -> Result<[f32; 4]> {
            Ok(match tf.typed(c, v, l, name, 0x04)? {
                Some(Value::Vec4(x)) => *x,
                _ => [0.0; 4],
            })
        };
        let up = vec4("up")?;
        let forward = vec4("forward")?;
        let duration = match tf.typed(c, v, l, "duration", 0x03)? {
            Some(Value::Real(x)) => *x,
            _ => 0.0,
        };
        let (components, samples) = match tf.typed(c, v, l, "referenceFrameSamples", 0x14)? {
            Some(Value::Vec4Array { components, values }) => (*components, values.clone()),
            _ => (0, Vec::new()),
        };
        Ok(ReferenceFrame {
            up,
            forward,
            duration,
            components,
            samples,
        })
    }

    /// Parses a leading reference-frame tagfile (envelope field 4); object 1 must be an
    /// `hkaDefaultAnimatedReferenceFrame`.
    pub fn parse(tagfile: &[u8]) -> Result<Self> {
        let tf = Tagfile::parse(tagfile)?;
        let Some(root) = tf.root() else {
            return Err(HavokError::ObjectMissing {
                expected: REFERENCE_FRAME_CLASS,
                found: None,
            });
        };
        let name = tf.class_name(root.class);
        if name != REFERENCE_FRAME_CLASS.as_bytes() {
            return Err(class_error(name, REFERENCE_FRAME_CLASS, KNOWN_CLASSES));
        }
        Self::from_object(&tf, root)
    }

    /// Samples the reference frame at time `t` as `havok-data.md` §7.8 words it: the frame
    /// grid has one frame per *sample* over `duration`, so a short sample array is stretched
    /// over the whole clip; x, y, z and w are interpolated linearly. No samples gives zeros;
    /// one sample is constant.
    ///
    /// For root motion use [`ReferenceFrame::sample_on_grid`], which follows `havok-data.md`
    /// §4.5 and `skeleton-anim.md` §7.3 (clip frame grid, hold the last sample). The two
    /// agree whenever the sample count equals the clip's frame count (211 of 214 objects).
    pub fn sample(&self, t: f32) -> [f32; 4] {
        match self.samples.len() {
            0 => [0.0; 4],
            1 => self.samples[0],
            n => {
                let (f, alpha) = frame_and_alpha(t, self.duration, n);
                let a = self.samples[f];
                let b = self.samples[f + 1];
                std::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha)
            }
        }
    }

    /// Samples the reference frame at time `t` on the clip's frame grid (`havok-data.md`
    /// §4.5, `skeleton-anim.md` §7.3): sample *k* belongs to time *k* × `duration` /
    /// (`frames` − 1), and times past the last stored sample hold that sample. `frames` is
    /// the clip's frame count ([`Animation::frame_count`]) and `duration` the clip's sampling
    /// duration; `t` is clamped as in §7.4. No samples gives zeros; one sample, or
    /// `frames` < 2, gives the first sample.
    pub fn sample_on_grid(&self, t: f32, frames: usize, duration: f32) -> [f32; 4] {
        let Some(&first) = self.samples.first() else {
            return [0.0; 4];
        };
        if frames < 2 || self.samples.len() < 2 {
            return first;
        }
        let (f, alpha) = frame_and_alpha(t, duration, frames);
        let last = self.samples.len() - 1;
        let a = self.samples[f.min(last)];
        let b = self.samples[(f + 1).min(last)];
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha)
    }
}

/// Frame index `f ≤ n − 2` and blend factor for time `t` on an `n`-frame grid over
/// `duration` (§7.8). `n` must be at least 2.
fn frame_and_alpha(t: f32, duration: f32, n: usize) -> (usize, f32) {
    let t = clamp_time(t, duration);
    let delta = duration / (n - 1) as f32;
    let x = t / delta;
    let x = if x.is_nan() { 0.0 } else { x.max(0.0) };
    let f = (x.floor() as usize).min(n - 2);
    let alpha = (x - f as f32).clamp(0.0, 1.0);
    (f, alpha)
}

/// The `tracks` transforms of frame `f`, if present (checked arithmetic for hand-built
/// values).
fn frame_slice(transforms: &[Transform], f: usize, tracks: usize) -> Option<&[Transform]> {
    let start = f.checked_mul(tracks)?;
    transforms.get(start..start.checked_add(tracks)?)
}

/// A decoded `hkaInterleavedUncompressedAnimation` (§6).
#[derive(Debug, Clone, PartialEq)]
pub struct InterleavedAnimation {
    /// Transform tracks *T*.
    pub tracks: usize,
    /// Frames *N* (`transforms.len() / T`).
    pub frames: usize,
    /// The animation's `duration` (seconds).
    pub duration: f32,
    /// Frame-major transforms: element `f × T + j` is track `j` at frame `f`.
    pub transforms: Vec<Transform>,
}

impl InterleavedAnimation {
    /// Samples every track at time `t` into `out` (§7.8): linear translation and scale,
    /// normalised-linear rotation after sign alignment. Returns the number written.
    pub fn sample(&self, t: f32, out: &mut [Transform]) -> usize {
        let n = self.tracks.min(out.len());
        if self.frames < 2 {
            return self.sample_frame(0, out);
        }
        let (f, alpha) = frame_and_alpha(t, self.duration, self.frames);
        let t = self.tracks;
        let (Some(a), Some(b)) = (
            frame_slice(&self.transforms, f, t),
            frame_slice(&self.transforms, f + 1, t),
        ) else {
            return 0;
        };
        for ((o, x), y) in out.iter_mut().zip(a).zip(b) {
            *o = Transform {
                translation: std::array::from_fn(|i| {
                    x.translation[i] + (y.translation[i] - x.translation[i]) * alpha
                }),
                rotation: quat_nlerp(x.rotation, y.rotation, alpha),
                scale: std::array::from_fn(|i| x.scale[i] + (y.scale[i] - x.scale[i]) * alpha),
            };
        }
        n
    }

    /// Copies frame `f` (clamped to the last frame), rotations normalised. Returns the
    /// number of tracks written.
    pub fn sample_frame(&self, f: usize, out: &mut [Transform]) -> usize {
        let f = f.min(self.frames.saturating_sub(1));
        let frame = frame_slice(&self.transforms, f, self.tracks).unwrap_or(&[]);
        let n = frame.len().min(out.len());
        for (o, x) in out.iter_mut().zip(frame) {
            *o = *x;
            o.rotation = quat_nlerp(x.rotation, x.rotation, 0.0);
        }
        n
    }
}

/// The kind-specific part of an animation.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationData {
    /// `hkaSplineCompressedAnimation` (§5).
    Spline(SplineAnimation),
    /// `hkaInterleavedUncompressedAnimation` (§6).
    Interleaved(InterleavedAnimation),
}

/// A decoded animation object (object 1 of a clip tagfile, §4.3).
///
/// The fields are public so tests and tools can build models by hand. The invariants stated
/// on them (and on [`SplineAnimation`] and [`InterleavedAnimation`]) are guaranteed for
/// values returned by the parsers only; sampling and [`Animation::check_samples_finite`]
/// tolerate hand-built values that break them without panicking or allocating beyond the
/// sample data.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// The `type` member (3 = spline, 1 = interleaved where observed; informational).
    pub type_code: i32,
    /// `duration` in seconds; sampling uses this value.
    pub duration: f32,
    /// `numberOfTransformTracks`; parsed values equal the track count of `data`.
    pub tracks: usize,
    /// Kind-specific data.
    pub data: AnimationData,
    /// Annotation tracks (empty when absent). At most [`Animation::tracks`] entries: track
    /// *i* belongs to transform track *i* (§4.4), and elements past the last transform track
    /// are not kept.
    pub annotation_tracks: Vec<AnnotationTrack>,
    /// The embedded reference frame referenced by `extractedMotion`, if any.
    pub extracted_motion: Option<ReferenceFrame>,
}

fn int(tf: &Tagfile<'_>, obj: &Object<'_>, class: &'static str, name: &'static str) -> Result<i32> {
    Ok(match tf.typed(obj.class, &obj.values, class, name, 0x02)? {
        Some(Value::Int(x)) => *x,
        _ => 0,
    })
}

fn real(
    tf: &Tagfile<'_>,
    obj: &Object<'_>,
    class: &'static str,
    name: &'static str,
) -> Result<f32> {
    Ok(match tf.typed(obj.class, &obj.values, class, name, 0x03)? {
        Some(Value::Real(x)) => *x,
        _ => 0.0,
    })
}

fn ints<'v, 'a>(
    tf: &Tagfile<'a>,
    obj: &'v Object<'a>,
    class: &'static str,
    name: &'static str,
) -> Result<&'v [i32]> {
    Ok(match tf.typed(obj.class, &obj.values, class, name, 0x12)? {
        Some(Value::IntArray { values, .. }) => values,
        _ => &[],
    })
}

impl Animation {
    /// Parses a clip tagfile (envelope field 6) into its animation.
    pub fn parse(tagfile: &[u8]) -> Result<Self> {
        let tf = Tagfile::parse(tagfile)?;
        Self::from_tagfile(&tf)
    }

    /// Builds the animation from object 1 of a parsed clip tagfile (§7.2 step 2).
    pub fn from_tagfile(tf: &Tagfile<'_>) -> Result<Self> {
        const EXPECTED: &str =
            "hkaSplineCompressedAnimation or hkaInterleavedUncompressedAnimation";
        let Some(obj) = tf.root() else {
            return Err(HavokError::ObjectMissing {
                expected: EXPECTED,
                found: None,
            });
        };
        let cname = tf.class_name(obj.class);
        let label: &'static str = if cname == SPLINE_CLASS.as_bytes() {
            SPLINE_CLASS
        } else if cname == INTERLEAVED_CLASS.as_bytes() {
            INTERLEAVED_CLASS
        } else {
            return Err(class_error(cname, EXPECTED, KNOWN_CLASSES));
        };
        let type_code = int(tf, obj, label, "type")?;
        let duration = real(tf, obj, label, "duration")?;
        let tracks = int(tf, obj, label, "numberOfTransformTracks")?;
        let float_tracks = int(tf, obj, label, "numberOfFloatTracks")?;
        if float_tracks > 0 {
            return Err(HavokError::FloatTracks {
                count: float_tracks,
            });
        }
        if float_tracks < 0 || tracks < 0 {
            return Err(HavokError::inconsistent(format!(
                "negative track counts ({tracks} transform, {float_tracks} float)"
            )));
        }
        // Declared types are checked here; the tracks are built once the kind-specific data
        // has bounded the track count by the input.
        let annotations = annotation_columns(tf, obj, label)?;
        let extracted_motion = match tf.typed(
            obj.class,
            &obj.values,
            label,
            "extractedMotion",
            0x08,
        )? {
            Some(Value::Object(r)) if *r != 0 => {
                // The container already checked that the reference resolves.
                let target = tf
                    .object(*r)
                    .ok_or(HavokError::UnresolvedRef { index: *r as i32 })?;
                if tf.class_name(target.class) != REFERENCE_FRAME_CLASS.as_bytes() {
                    return Err(HavokError::inconsistent(
                        "extractedMotion does not reference an hkaDefaultAnimatedReferenceFrame",
                    ));
                }
                Some(ReferenceFrame::from_object(tf, target)?)
            }
            _ => None,
        };
        let data = if label == SPLINE_CLASS {
            AnimationData::Spline(spline(tf, obj, duration, tracks as usize)?)
        } else {
            AnimationData::Interleaved(interleaved(tf, obj, duration, tracks)?)
        };
        let annotation_tracks = annotations.map_or_else(Vec::new, |cols| {
            build_annotation_tracks(tf, &cols, tracks as usize)
        });
        match (&data, type_code) {
            (AnimationData::Spline(_), 3) | (AnimationData::Interleaved(_), 1) => {}
            _ => tracing::debug!(
                type_code,
                class = label,
                "animation type code differs from class"
            ),
        }
        Ok(Animation {
            type_code,
            duration,
            tracks: tracks as usize,
            data,
            annotation_tracks,
            extracted_motion,
        })
    }

    /// Number of frames *N*.
    pub fn frame_count(&self) -> usize {
        match &self.data {
            AnimationData::Spline(s) => s.num_frames as usize,
            AnimationData::Interleaved(i) => i.frames,
        }
    }

    /// Samples every track at time `t` (seconds, clamped to `0 ..= duration`) into `out`, as
    /// many tracks as fit; returns the number written. Does not allocate.
    pub fn sample(&self, t: f32, out: &mut [Transform]) -> usize {
        match &self.data {
            AnimationData::Spline(s) => s.sample(t, out),
            AnimationData::Interleaved(i) => i.sample(t, out),
        }
    }

    /// Samples one track at time `t`; `None` if `track` is out of range.
    pub fn sample_track(&self, track: usize, t: f32) -> Option<Transform> {
        match &self.data {
            AnimationData::Spline(s) => {
                let (b, u) = s.locate(t);
                s.blocks.get(b)?.sample_track(track, u)
            }
            AnimationData::Interleaved(i) => {
                if track >= i.tracks {
                    return None;
                }
                // Sampling all tracks would need a buffer; interpolate this one directly.
                if i.frames < 2 {
                    return i.transforms.get(track).copied();
                }
                let (f, alpha) = frame_and_alpha(t, i.duration, i.frames);
                let x = frame_slice(&i.transforms, f, i.tracks)?.get(track)?;
                let y = frame_slice(&i.transforms, f + 1, i.tracks)?.get(track)?;
                Some(Transform {
                    translation: std::array::from_fn(|k| {
                        x.translation[k] + (y.translation[k] - x.translation[k]) * alpha
                    }),
                    rotation: quat_nlerp(x.rotation, y.rotation, alpha),
                    scale: std::array::from_fn(|k| x.scale[k] + (y.scale[k] - x.scale[k]) * alpha),
                })
            }
        }
    }

    /// Samples frame `f` exactly (clamped to the last frame); returns the number written.
    pub fn sample_frame(&self, f: usize, out: &mut [Transform]) -> usize {
        match &self.data {
            AnimationData::Spline(s) => s.sample_frame(u32::try_from(f).unwrap_or(u32::MAX), out),
            AnimationData::Interleaved(i) => i.sample_frame(f, out),
        }
    }

    /// Samples every frame of every track and checks that every value is finite (the last
    /// condition of §9.2). `scratch` is reused between frames and grown to the number of
    /// tracks the data holds.
    ///
    /// The work is bounded by the data, not by the declared frame counts: a spline block
    /// without curves has the same value at every local frame and is sampled once, and a
    /// block with curves has at most 256 frames in parsed data (its last knot, a byte, equals
    /// *F_b* − 1, §5.4). Hand-built blocks with curves are sampled at their first 256 local
    /// frames and at their last one.
    pub fn check_samples_finite(&self, scratch: &mut Vec<Transform>) -> Result<()> {
        /// Distinct local frames a curve can address: knots are bytes.
        const MAX_CURVE_FRAMES: u32 = 256;
        let width = match &self.data {
            AnimationData::Spline(s) => s.blocks.iter().map(|b| b.tracks.len()).max(),
            AnimationData::Interleaved(i) => Some(i.tracks.min(i.transforms.len())),
        };
        scratch.clear();
        scratch.resize(width.unwrap_or(0), Transform::IDENTITY);
        let check = |s: &[Transform]| -> Result<()> {
            match s.iter().position(|t| !t.is_finite()) {
                Some(track) => Err(HavokError::NonFinite {
                    what: "sampled transform",
                    track,
                }),
                None => Ok(()),
            }
        };
        match &self.data {
            AnimationData::Spline(s) => {
                for b in &s.blocks {
                    let frames = if b.knots.is_empty() {
                        // No curve: every channel is constant over the block.
                        1
                    } else {
                        b.frames.min(MAX_CURVE_FRAMES)
                    };
                    for u in 0..frames {
                        let n = b.sample(u as f32, scratch);
                        check(&scratch[..n])?;
                    }
                    if frames < b.frames {
                        let n = b.sample((b.frames - 1) as f32, scratch);
                        check(&scratch[..n])?;
                    }
                }
            }
            AnimationData::Interleaved(i) => {
                // Frames past the stored transforms read as nothing; stop at the data.
                let stored = i.transforms.len() / i.tracks.max(1);
                for f in 0..i.frames.min(stored) {
                    let n = i.sample_frame(f, scratch);
                    check(&scratch[..n])?;
                }
            }
        }
        Ok(())
    }
}

/// The `annotationTracks` struct array of an animation object: its length and its name
/// column, if present.
struct AnnotationColumns<'v> {
    len: usize,
    names: Option<&'v [StrId]>,
}

/// The type-checked `annotationTracks` columns, or `None` when the member is absent.
fn annotation_columns<'v, 'a>(
    tf: &Tagfile<'a>,
    obj: &'v Object<'a>,
    label: &'static str,
) -> Result<Option<AnnotationColumns<'v>>> {
    let Some(Value::StructArray(sa)) =
        tf.typed(obj.class, &obj.values, label, "annotationTracks", 0x19)?
    else {
        return Ok(None);
    };
    let names = match tf.typed(
        sa.class,
        &sa.columns,
        "hkaAnnotationTrack",
        "trackName",
        0x0A,
    )? {
        Some(Value::CStringArray(ids)) => Some(ids.as_slice()),
        _ => None,
    };
    // An `annotations` column would be an array column, rejected by the container (§3.9);
    // its declared type is still checked.
    tf.typed(
        sa.class,
        &sa.columns,
        "hkaAnnotationTrack",
        "annotations",
        0x19,
    )?;
    Ok(Some(AnnotationColumns { len: sa.len, names }))
}

/// Builds the annotation tracks of an array of `cols.len` elements for `tracks` transform
/// tracks.
/// Only the first `tracks` elements are kept (track *i* belongs to transform track *i*,
/// §4.4); an element costs no bytes when no column is present, so the array length alone
/// does not bound memory. Names share one copy per pool string.
fn build_annotation_tracks(
    tf: &Tagfile<'_>,
    cols: &AnnotationColumns<'_>,
    tracks: usize,
) -> Vec<AnnotationTrack> {
    let AnnotationColumns { len, names } = *cols;
    if len != tracks {
        tracing::debug!(
            annotation_tracks = len,
            transform_tracks = tracks,
            "annotation track count differs from the transform track count"
        );
    }
    let mut shared = SharedStrings::default();
    (0..len.min(tracks))
        .map(|i| AnnotationTrack {
            name: names
                .and_then(|ids| ids.get(i))
                .map(|&id| shared.get(tf, id)),
            annotations: Vec::new(),
        })
        .collect()
}

fn interleaved(
    tf: &Tagfile<'_>,
    obj: &Object<'_>,
    duration: f32,
    tracks: i32,
) -> Result<InterleavedAnimation> {
    let l = INTERLEAVED_CLASS;
    let elems: &[[f32; 12]] = match tf.typed(obj.class, &obj.values, l, "transforms", 0x16)? {
        Some(Value::Vec12Array(v)) => v,
        _ => &[],
    };
    if let Some(Value::RealArray(f)) = tf.typed(obj.class, &obj.values, l, "floats", 0x13)?
        && !f.is_empty()
    {
        tracing::debug!(
            count = f.len(),
            "interleaved clip has float samples; ignored"
        );
    }
    if tracks <= 0 || elems.is_empty() || !elems.len().is_multiple_of(tracks as usize) {
        return Err(HavokError::InterleavedBadShape {
            transforms: elems.len(),
            tracks: i64::from(tracks),
        });
    }
    let tracks = tracks as usize;
    Ok(InterleavedAnimation {
        tracks,
        frames: elems.len() / tracks,
        duration,
        transforms: elems.iter().map(Transform::from_vec12).collect(),
    })
}

fn spline(
    tf: &Tagfile<'_>,
    obj: &Object<'_>,
    duration: f32,
    tracks: usize,
) -> Result<SplineAnimation> {
    let l = SPLINE_CLASS;
    let n = int(tf, obj, l, "numFrames")?;
    let b = int(tf, obj, l, "numBlocks")?;
    let f = int(tf, obj, l, "maxFramesPerBlock")?;
    let mask_size = int(tf, obj, l, "maskAndQuantizationSize")?;
    let block_duration = real(tf, obj, l, "blockDuration")?;
    let block_inverse_duration = real(tf, obj, l, "blockInverseDuration")?;
    let frame_duration = real(tf, obj, l, "frameDuration")?;
    let offsets = ints(tf, obj, l, "blockOffsets")?;
    let fbo = ints(tf, obj, l, "floatBlockOffsets")?;
    // Declared types are checked even though the values are unused.
    ints(tf, obj, l, "transformOffsets")?;
    ints(tf, obj, l, "floatOffsets")?;
    let endian = int(tf, obj, l, "endian")?;
    let data: &[u8] = match tf.typed(obj.class, &obj.values, l, "data", 0x11)? {
        Some(Value::Bytes(d)) => d,
        _ => &[],
    };
    if endian != 0 {
        return Err(HavokError::inconsistent(format!(
            "endian {endian}, expected 0"
        )));
    }
    if tracks == 0 {
        return Err(HavokError::inconsistent(
            "spline clip with no transform tracks",
        ));
    }
    if n < 1 || f < 2 || b < 1 {
        return Err(HavokError::inconsistent(format!(
            "numFrames {n}, maxFramesPerBlock {f}, numBlocks {b}"
        )));
    }
    let (n, f, b) = (n as u64, f as u64, b as u64);
    let expected_blocks = (n - 1).div_ceil(f - 1).max(1);
    if b != expected_blocks {
        return Err(HavokError::inconsistent(format!(
            "numBlocks {b}, expected {expected_blocks} for {n} frames"
        )));
    }
    if offsets.len() as u64 != b || fbo.len() as u64 != b {
        return Err(HavokError::inconsistent(format!(
            "{} block offsets and {} float block offsets for {b} blocks",
            offsets.len(),
            fbo.len()
        )));
    }
    if i64::from(mask_size) != 4 * tracks as i64 {
        return Err(HavokError::inconsistent(format!(
            "maskAndQuantizationSize {mask_size} for {tracks} tracks"
        )));
    }
    // HV9: block table.
    let b = b as usize;
    let align16 = |x: i64| (x + 15) & !15;
    if offsets[0] != 0 {
        return Err(HavokError::inconsistent("blockOffsets[0] is not 0"));
    }
    for i in 0..b {
        let off = i64::from(offsets[i]);
        let len = i64::from(fbo[i]);
        if off % 16 != 0 || len < 0 {
            return Err(HavokError::inconsistent(format!(
                "block {i}: offset {off}, transform length {len}"
            )));
        }
        let next = if i + 1 < b {
            i64::from(offsets[i + 1])
        } else {
            data.len() as i64
        };
        if next != align16(off + len) {
            return Err(HavokError::inconsistent(format!(
                "block {i}: next block at {next}, expected {}",
                align16(off + len)
            )));
        }
    }
    let mut blocks = Vec::with_capacity(b);
    for i in 0..b {
        let start = offsets[i] as usize;
        let end = if i + 1 < b {
            offsets[i + 1] as usize
        } else {
            data.len()
        };
        let frames = f.min(n - i as u64 * (f - 1));
        let params = BlockParams {
            block: i,
            tracks,
            float_tracks: 0,
            frames: u32::try_from(frames).unwrap_or(u32::MAX),
            transform_length: fbo[i] as usize,
        };
        blocks.push(SplineBlock::parse(&data[start..end], &params)?);
    }
    Ok(SplineAnimation {
        num_frames: u32::try_from(n).unwrap_or(u32::MAX),
        max_frames_per_block: u32::try_from(f).unwrap_or(u32::MAX),
        block_duration,
        block_inverse_duration,
        frame_duration,
        duration,
        tracks,
        blocks,
    })
}

/// Where a clip's reference frame came from (§4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceFrameSource {
    /// Embedded in the clip tagfile and referenced by `extractedMotion`.
    Embedded,
    /// The leading tagfile of the envelope.
    Leading,
}

/// A decoded 0x05 clip record (§7.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Envelope field 1. May differ from [`Animation::duration`] (§8.4); sampling uses the
    /// animation's value.
    pub envelope_duration: f32,
    /// Envelope field 2; equals the animation's track count.
    pub envelope_track_count: u32,
    /// The leading reference frame (envelope field 4), if present.
    pub leading_reference_frame: Option<ReferenceFrame>,
    /// The animation.
    pub animation: Animation,
    /// Per-track weights (empty, or one per track; meaning OPEN, §2.3).
    pub track_weights: Vec<f32>,
}

/// Track-to-bone binding of a clip. No binding object exists in the data (§4.6); tracks
/// bind to bones by position (`skeleton-anim.md` §3.2): track *j* drives bone *j*.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackBinding<'c> {
    tracks: usize,
    annotations: &'c [AnnotationTrack],
}

impl<'c> TrackBinding<'c> {
    /// Number of transform tracks.
    pub fn track_count(&self) -> usize {
        self.tracks
    }

    /// The bone driven by `track` (positional), `None` if the track does not exist.
    pub fn bone(&self, track: usize) -> Option<usize> {
        (track < self.tracks).then_some(track)
    }

    /// The annotation track name of `track`, a diagnostic only (never used to reorder).
    pub fn track_name(&self, track: usize) -> Option<&'c [u8]> {
        self.annotations.get(track)?.name.as_deref()
    }

    /// Whether the clip fits `skeleton`: the track count equals the bone count.
    pub fn fits(&self, skeleton: &Skeleton) -> bool {
        self.tracks == skeleton.bone_count()
    }
}

impl Clip {
    /// Parses a 0x05 record (§7.2).
    pub fn parse(record: &[u8]) -> Result<Self> {
        let env = Envelope::parse(record)?;
        let leading_reference_frame = env.reference_frame.map(ReferenceFrame::parse).transpose()?;
        let animation = Animation::parse(env.tagfile)?;
        if u64::from(env.track_count) != animation.tracks as u64 {
            return Err(HavokError::inconsistent(format!(
                "envelope track count {} differs from numberOfTransformTracks {}",
                env.track_count, animation.tracks
            )));
        }
        if env.duration.to_bits() != animation.duration.to_bits() {
            tracing::debug!(
                envelope = env.duration,
                tagfile = animation.duration,
                "clip duration mismatch"
            );
        }
        Ok(Clip {
            envelope_duration: env.duration,
            envelope_track_count: env.track_count,
            leading_reference_frame,
            animation,
            track_weights: env.track_weights,
        })
    }

    /// The reference frame: the embedded one if present, else the leading one (§7.2).
    pub fn reference_frame(&self) -> Option<(&ReferenceFrame, ReferenceFrameSource)> {
        if let Some(r) = &self.animation.extracted_motion {
            return Some((r, ReferenceFrameSource::Embedded));
        }
        self.leading_reference_frame
            .as_ref()
            .map(|r| (r, ReferenceFrameSource::Leading))
    }

    /// Whether the envelope duration differs (bitwise) from the animation's (§8.4).
    pub fn duration_mismatch(&self) -> bool {
        self.envelope_duration.to_bits() != self.animation.duration.to_bits()
    }

    /// Duration used for sampling (the animation's, §2.3).
    pub fn duration(&self) -> f32 {
        self.animation.duration
    }

    /// Number of transform tracks.
    pub fn track_count(&self) -> usize {
        self.animation.tracks
    }

    /// The positional track-to-bone binding.
    pub fn binding(&self) -> TrackBinding<'_> {
        TrackBinding {
            tracks: self.animation.tracks,
            annotations: &self.animation.annotation_tracks,
        }
    }

    /// Samples every track at time `t` into `out` (see [`Animation::sample`]).
    pub fn sample(&self, t: f32, out: &mut [Transform]) -> usize {
        self.animation.sample(t, out)
    }

    /// Annotation track names decoded as Latin-1 (diagnostics).
    pub fn track_names_lossy(&self) -> Vec<Option<String>> {
        self.animation
            .annotation_tracks
            .iter()
            .map(|a| a.name.as_deref().map(latin1))
            .collect()
    }
}
