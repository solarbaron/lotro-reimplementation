// SPDX-License-Identifier: GPL-3.0-or-later
//! Spline-compressed clips: block layout, track masks, channels, quantisation, the 40-bit
//! quaternion and B-spline evaluation (`havok-data.md` §5, §7.3–§7.7).

use crate::error::{Channel, HavokError, Result};
use crate::transform::{Transform, quat_dot};

/// Position/scale quantisation code: 8-bit control points (Reported, §5.3).
pub const QUANT_8BIT: u8 = 0;
/// Position/scale quantisation code: 16-bit control points (every track observed).
pub const QUANT_16BIT: u8 = 1;
/// Rotation quantisation code: the 40-bit quaternion (every track observed, §5.6).
pub const ROT_QUANT_40BIT: u8 = 1;
/// Largest curve degree Tessera accepts (1 and 3 occur, HV11). A resource limit, like the
/// tagfile's: evaluation costs O(*p*²) per channel, so the u8 degree's full range would let a
/// small record make every sample thousands of times slower. A parsed curve of higher degree
/// is `spline.bad_curve` (§9.3); [`Curve::new`] and [`basis_functions`] refuse it too.
pub const MAX_DEGREE: usize = 7;

/// 1 / (2047 × √2), the 40-bit field scale (§5.6).
const QUAT40_SCALE: f32 = 1.0 / (2047.0 * std::f32::consts::SQRT_2);

/// Decodes a 40-bit quaternion (§5.6) into (x, y, z, w).
///
/// The result is unit length whenever the three stored components leave room for the
/// dropped one (always, in the observed data); otherwise the dropped component is 0 and the
/// length is at most √1.5. Bit 39 is ignored.
pub fn decode_quat40(b: [u8; 5]) -> [f32; 4] {
    let v = u64::from(b[0])
        | u64::from(b[1]) << 8
        | u64::from(b[2]) << 16
        | u64::from(b[3]) << 24
        | u64::from(b[4]) << 32;
    let field = |shift: u32| ((v >> shift) & 0xFFF) as f32;
    let c = [
        (field(0) - 2047.0) * QUAT40_SCALE,
        (field(12) - 2047.0) * QUAT40_SCALE,
        (field(24) - 2047.0) * QUAT40_SCALE,
    ];
    let dropped = ((v >> 36) & 3) as usize;
    let negative = (v >> 38) & 1 == 1;
    let mut d = (1.0 - c[0] * c[0] - c[1] * c[1] - c[2] * c[2])
        .max(0.0)
        .sqrt();
    if negative {
        d = -d;
    }
    let mut q = [0.0f32; 4];
    let mut k = 0;
    for (i, slot) in q.iter_mut().enumerate() {
        if i == dropped {
            *slot = d;
        } else {
            *slot = c[k];
            k += 1;
        }
    }
    q
}

/// Finds the knot span for local frame `u` (§7.5 step 1): `n` when `u ≥ U[n+1]`, else the
/// index `s` in `p ..= n` with `U[s] ≤ u < U[s+1]`.
///
/// `knots` must hold `n + p + 2` values; with fewer, the result is clamped into range.
pub fn find_span(knots: &[u8], n: usize, p: usize, u: f32) -> usize {
    if n.checked_add(1).is_none_or(|e| e >= knots.len()) || p > n {
        return p.min(n);
    }
    if u >= f32::from(knots[n + 1]) {
        return n;
    }
    let interior = &knots[p + 1..=n];
    p + interior.partition_point(|&k| f32::from(k) <= u)
}

/// Computes the `p + 1` non-zero basis functions `N[s−p] … N[s]` of degree `p` at `u`
/// (§7.5 step 2, Cox–de Boor); a fraction with a zero denominator counts as 0.
///
/// `out` receives the values; it must hold at least `p + 1` entries, `p ≤ MAX_DEGREE`, and
/// `span` must satisfy `p ≤ span` and `span + p < knots.len()`. Returns `false` (and
/// leaves `out` unspecified) if these do not hold.
pub fn basis_functions(knots: &[u8], span: usize, p: usize, u: f32, out: &mut [f32]) -> bool {
    if p > MAX_DEGREE
        || out.len() <= p
        || span < p
        || span.checked_add(p).is_none_or(|e| e >= knots.len())
    {
        return false;
    }
    let mut left = [0.0f32; MAX_DEGREE + 1];
    let mut right = [0.0f32; MAX_DEGREE + 1];
    out[0] = 1.0;
    for j in 1..=p {
        left[j] = u - f32::from(knots[span + 1 - j]);
        right[j] = f32::from(knots[span + j]) - u;
        let mut saved = 0.0f32;
        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            let temp = if denom == 0.0 { 0.0 } else { out[r] / denom };
            out[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        out[j] = saved;
    }
    true
}

/// A borrowed B-spline: degree, knots (u8 frame numbers) and control points with `dims`
/// components each (3 or fewer for vectors, 4 for quaternions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve<'b> {
    degree: usize,
    knots: &'b [u8],
    points: &'b [f32],
    dims: usize,
}

impl<'b> Curve<'b> {
    /// Wraps curve data; `None` unless `1 ≤ dims ≤ 4`, `points.len()` is a non-zero multiple
    /// of `dims` (n + 1 control points), `degree ≤ n`, `degree ≤ MAX_DEGREE` and
    /// `knots.len() = n + degree + 2`.
    pub fn new(degree: usize, knots: &'b [u8], points: &'b [f32], dims: usize) -> Option<Self> {
        if !(1..=4).contains(&dims) || points.is_empty() || !points.len().is_multiple_of(dims) {
            return None;
        }
        let n = points.len() / dims - 1;
        if degree > n || degree > MAX_DEGREE || knots.len() != n + degree + 2 {
            return None;
        }
        Some(Curve {
            degree,
            knots,
            points,
            dims,
        })
    }

    /// Degree *p*.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// The knot vector.
    pub fn knots(&self) -> &'b [u8] {
        self.knots
    }

    /// Control points, `dims` components each, point-major.
    pub fn control_points(&self) -> &'b [f32] {
        self.points
    }

    /// Components per control point.
    pub fn dims(&self) -> usize {
        self.dims
    }

    /// Number of control points (*n* + 1).
    pub fn control_point_count(&self) -> usize {
        self.points.len() / self.dims
    }

    fn weights(&self, u: f32, w: &mut [f32; MAX_DEGREE + 1]) -> usize {
        let n = self.control_point_count() - 1;
        let s = find_span(self.knots, n, self.degree, u);
        if !basis_functions(self.knots, s, self.degree, u, w) {
            // Unreachable for a curve built by `new`; keep a defined value regardless.
            w[..=self.degree].fill(0.0);
            w[0] = 1.0;
            return 0;
        }
        s - self.degree
    }

    /// Evaluates the curve at local frame `u` (§7.5) into `out[..dims]` (componentwise
    /// basis blend). Extra entries of `out` are left alone; a shorter `out` gets a prefix.
    ///
    /// The blend is a convex combination of the window's control points, but the `f32`
    /// basis weights can sum a hair above 1, so finite points near the `f32` range limit
    /// can sum to infinity between frames. When the `f32` sum is not finite, the component
    /// is recomputed in `f64` and limited to the range of the window's points, so finite
    /// control points always give a finite value (other values are bit-identical).
    pub fn evaluate(&self, u: f32, out: &mut [f32]) {
        let mut w = [0.0f32; MAX_DEGREE + 1];
        let first = self.weights(u, &mut w);
        let dims = self.dims;
        let window = || {
            w.iter()
                .take(self.degree + 1)
                .enumerate()
                .map(move |(k, &wk)| (first + k, wk))
        };
        for (d, o) in out.iter_mut().enumerate().take(dims) {
            let point = |k: usize| self.points[k * dims + d];
            let mut acc = 0.0f32;
            for (k, wk) in window() {
                acc += wk * point(k);
            }
            if !acc.is_finite() {
                let (mut sum, mut lo, mut hi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
                for (k, wk) in window() {
                    let p = f64::from(point(k));
                    sum += f64::from(wk) * p;
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
                // `clamp` keeps NaN (non-finite points); lo ≤ hi for finite points.
                if lo <= hi {
                    acc = sum.clamp(lo, hi) as f32;
                }
            }
            *o = acc;
        }
    }

    /// Evaluates a quaternion curve (`dims` = 4) at `u` per §7.6: sign-align the window,
    /// blend, normalise; falls back to the control point with the largest weight when the
    /// blend is shorter than 1e-6. For other `dims` the missing components read as 0.
    pub fn evaluate_rotation(&self, u: f32) -> [f32; 4] {
        let mut w = [0.0f32; MAX_DEGREE + 1];
        let first = self.weights(u, &mut w);
        let dims = self.dims;
        let point = |k: usize| {
            let mut q = [0.0f32; 4];
            let base = (first + k) * dims;
            for (i, c) in q.iter_mut().enumerate().take(dims) {
                *c = self.points[base + i];
            }
            q
        };
        let mut acc = [0.0f32; 4];
        let mut prev = point(0);
        let mut best = (0usize, f32::NEG_INFINITY);
        for (k, &wk) in w.iter().enumerate().take(self.degree + 1) {
            let mut q = point(k);
            if k > 0 && quat_dot(prev, q) < 0.0 {
                q = q.map(|c| -c);
            }
            for i in 0..4 {
                acc[i] += wk * q[i];
            }
            if wk > best.1 {
                best = (k, wk);
            }
            prev = q;
        }
        let len = quat_dot(acc, acc).sqrt();
        if len.is_finite() && len >= 1e-6 {
            acc.map(|c| c / len)
        } else {
            point(best.0)
        }
    }
}

/// Where a curve's data lives inside its [`SplineBlock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveRef {
    /// Degree *p*.
    pub degree: u8,
    /// Index of the last control point (*n*).
    pub last: u16,
    /// Start of the knots in [`SplineBlock::knots`].
    pub knot_start: u32,
    /// Start of the control points in [`SplineBlock::points`].
    pub point_start: u32,
    /// Components per control point (1–3 dynamic components, 4 for rotations).
    pub dims: u8,
}

/// A position or scale channel of one track in one block (§5.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VectorChannel {
    /// No dynamic component: static values and defaults (0 for position, 1 for scale).
    Constant([f32; 3]),
    /// At least one dynamic component.
    Spline {
        /// Values of the static and default components (dynamic ones hold the default).
        base: [f32; 3],
        /// Dynamic components: bit 0 = x, bit 1 = y, bit 2 = z.
        dynamic: u8,
        /// The curve over the dynamic components, in x, y, z order.
        curve: CurveRef,
    },
}

/// A rotation channel of one track in one block (§5.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RotationChannel {
    /// No data: (0, 0, 0, 1).
    Identity,
    /// One quaternion for the whole block.
    Static([f32; 4]),
    /// A quaternion curve.
    Dynamic(CurveRef),
}

/// The three channels of one transform track in one block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackChannels {
    /// Translation.
    pub position: VectorChannel,
    /// Rotation.
    pub rotation: RotationChannel,
    /// Scale.
    pub scale: VectorChannel,
    /// The track's quantisation byte (0x45 everywhere observed).
    pub quantisation: u8,
}

/// Numbers a block parse needs from the clip object (§5.1, §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockParams {
    /// Block index *b* (for error reports).
    pub block: usize,
    /// Transform tracks.
    pub tracks: usize,
    /// Float tracks (one mask byte each; any non-zero count is rejected by the clip).
    pub float_tracks: usize,
    /// Frames in this block, *F_b*.
    pub frames: u32,
    /// `floatBlockOffsets[b]`: where parsing must end, relative to the block start.
    pub transform_length: usize,
}

/// One decoded block: per-track channels plus the shared knot and control-point storage.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineBlock {
    /// Frames in the block, *F_b*.
    pub frames: u32,
    /// Channels, one entry per transform track.
    pub tracks: Vec<TrackChannels>,
    /// Knots of every curve of the block.
    pub knots: Vec<u8>,
    /// Dequantised control points of every curve of the block.
    pub points: Vec<f32>,
}

/// Block reader: offsets are relative to the block start, overruns are typed.
struct BlockReader<'a> {
    data: &'a [u8],
    pos: usize,
    block: usize,
    track: usize,
}

impl<'a> BlockReader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        match self.pos.checked_add(n) {
            Some(end) if end <= self.data.len() => {
                let s = &self.data[self.pos..end];
                self.pos = end;
                Ok(s)
            }
            _ => Err(HavokError::BlockOverrun {
                block: self.block,
                track: self.track,
                offset: self.pos,
            }),
        }
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn f32(&mut self) -> Result<f32> {
        let b = self.bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn pad4(&mut self) -> Result<()> {
        let aligned = self.pos.next_multiple_of(4);
        self.bytes(aligned - self.pos).map(|_| ())
    }
}

struct BlockParser<'a> {
    rd: BlockReader<'a>,
    frames: u32,
    knots: Vec<u8>,
    points: Vec<f32>,
}

impl BlockParser<'_> {
    fn curve_err(&self, channel: Channel, reason: &'static str) -> HavokError {
        HavokError::BadCurve {
            block: self.rd.block,
            track: self.rd.track,
            channel,
            reason,
        }
    }

    /// Reads `n`, `p` and the knot vector and checks them (§5.4 knot vectors, HV11).
    fn curve_header(&mut self, channel: Channel) -> Result<(u16, u8, u32)> {
        let n = self.rd.u16()?;
        let p = self.rd.u8()?;
        if p == 0 || u16::from(p) > n {
            return Err(self.curve_err(channel, "degree outside 1..n"));
        }
        if usize::from(p) > MAX_DEGREE {
            return Err(self.curve_err(channel, "degree above Tessera's limit"));
        }
        if u32::from(n) + 1 > self.frames {
            return Err(self.curve_err(channel, "more control points than block frames"));
        }
        let count = usize::from(n) + usize::from(p) + 2;
        let knots = self.rd.bytes(count)?;
        let last = self.frames - 1; // frames ≥ 2 here: n ≥ p ≥ 1 and n + 1 ≤ frames.
        let p = usize::from(p);
        if knots.windows(2).any(|w| w[0] > w[1]) {
            return Err(self.curve_err(channel, "knots decreasing"));
        }
        if knots.iter().any(|&k| u32::from(k) > last) {
            return Err(self.curve_err(channel, "knot beyond the block's last frame"));
        }
        let head_ok = knots[..=p].iter().all(|&k| k == 0);
        let tail_ok = knots[count - p - 1..].iter().all(|&k| u32::from(k) == last);
        let interior_ok = knots[p + 1..count - p - 1]
            .iter()
            .all(|&k| k > 0 && u32::from(k) < last);
        if !(head_ok && tail_ok && interior_ok) {
            return Err(self.curve_err(channel, "knot vector not clamped"));
        }
        // §5.4 / HV11: interior knots repeat at most p times, so the curve is continuous.
        // Interior knots lie strictly inside (0, last), so their runs never touch the clamped
        // ends.
        let interior = &knots[p + 1..count - p - 1];
        let mut run = 0usize;
        for (i, &k) in interior.iter().enumerate() {
            run = if i > 0 && interior[i - 1] == k {
                run + 1
            } else {
                1
            };
            if run > p {
                return Err(self.curve_err(channel, "interior knot multiplicity above degree"));
            }
        }
        let start = self.knots.len() as u32;
        self.knots.extend_from_slice(knots);
        Ok((n, p as u8, start))
    }

    fn vector(
        &mut self,
        mask: u8,
        qcode: u8,
        default: f32,
        channel: Channel,
    ) -> Result<VectorChannel> {
        let bad = || HavokError::BadMask {
            block: self.rd.block,
            track: self.rd.track,
            channel,
            mask,
        };
        if mask & 0x88 != 0 {
            return Err(bad());
        }
        let stat = mask & 0x07;
        let dynamic = (mask >> 4) & 0x07;
        if stat & dynamic != 0 {
            return Err(bad());
        }
        let mut base = [default; 3];
        if dynamic == 0 {
            for (c, v) in base.iter_mut().enumerate() {
                if stat & (1 << c) != 0 {
                    *v = self.rd.f32()?;
                }
            }
            return Ok(VectorChannel::Constant(base));
        }
        let eight_bit = match qcode {
            QUANT_8BIT => true,
            QUANT_16BIT => false,
            code => {
                return Err(HavokError::UnsupportedQuantisation {
                    block: self.rd.block,
                    track: self.rd.track,
                    channel,
                    code,
                });
            }
        };
        let (n, p, knot_start) = self.curve_header(channel)?;
        self.rd.pad4()?;
        let mut ranges = [(0.0f32, 0.0f32); 3];
        let mut dims = 0usize;
        for (c, v) in base.iter_mut().enumerate() {
            if stat & (1 << c) != 0 {
                *v = self.rd.f32()?;
            } else if dynamic & (1 << c) != 0 {
                let min = self.rd.f32()?;
                let max = self.rd.f32()?;
                ranges[dims] = (min, max);
                dims += 1;
            }
        }
        let count = (usize::from(n) + 1) * dims;
        let width = if eight_bit { 1 } else { 2 };
        let raw = self.rd.bytes(count * width)?;
        let point_start = self.points.len() as u32;
        self.points.reserve(count);
        for (i, q) in raw.chunks_exact(width).enumerate() {
            let (min, max) = ranges[i % dims];
            let frac = if eight_bit {
                f32::from(q[0]) / 255.0
            } else {
                f32::from(u16::from_le_bytes([q[0], q[1]])) / 65535.0
            };
            self.points.push(min + (max - min) * frac);
        }
        self.rd.pad4()?;
        Ok(VectorChannel::Spline {
            base,
            dynamic,
            curve: CurveRef {
                degree: p,
                last: n,
                knot_start,
                point_start,
                dims: dims as u8,
            },
        })
    }

    fn quat(&mut self) -> Result<[f32; 4]> {
        let b = self.rd.bytes(5)?;
        Ok(decode_quat40([b[0], b[1], b[2], b[3], b[4]]))
    }

    fn rotation(&mut self, mask: u8, qcode: u8) -> Result<RotationChannel> {
        let dynamic = mask >> 4 != 0;
        let stat = mask & 0x0F != 0;
        let ch = if !dynamic && !stat {
            RotationChannel::Identity
        } else {
            if qcode != ROT_QUANT_40BIT {
                return Err(HavokError::UnsupportedQuantisation {
                    block: self.rd.block,
                    track: self.rd.track,
                    channel: Channel::Rotation,
                    code: qcode,
                });
            }
            if dynamic {
                let (n, p, knot_start) = self.curve_header(Channel::Rotation)?;
                let count = usize::from(n) + 1;
                // Check the bytes before reserving.
                let start = self.rd.pos;
                self.rd.bytes(count * 5)?;
                self.rd.pos = start;
                let point_start = self.points.len() as u32;
                self.points.reserve(count * 4);
                for _ in 0..count {
                    let q = self.quat()?;
                    self.points.extend_from_slice(&q);
                }
                RotationChannel::Dynamic(CurveRef {
                    degree: p,
                    last: n,
                    knot_start,
                    point_start,
                    dims: 4,
                })
            } else {
                RotationChannel::Static(self.quat()?)
            }
        };
        self.rd.pad4()?;
        Ok(ch)
    }
}

impl SplineBlock {
    /// Parses one block (§7.3). `bytes` starts at `blockOffsets[b]` and extends to the next
    /// block offset (or the end of `data`); every read is bounds-checked against it, and
    /// parsing must end exactly at `params.transform_length`.
    pub fn parse(bytes: &[u8], params: &BlockParams) -> Result<Self> {
        let mut bp = BlockParser {
            rd: BlockReader {
                data: bytes,
                pos: 0,
                block: params.block,
                track: params.tracks,
            },
            frames: params.frames,
            knots: Vec::new(),
            points: Vec::new(),
        };
        let mask_len = params
            .tracks
            .checked_mul(4)
            .and_then(|m| m.checked_add(params.float_tracks))
            .unwrap_or(usize::MAX);
        let masks = bp.rd.bytes(mask_len)?;
        bp.rd.pad4()?;
        let mut tracks = Vec::with_capacity(params.tracks);
        for (j, m) in masks
            .as_chunks::<4>()
            .0
            .iter()
            .take(params.tracks)
            .enumerate()
        {
            bp.rd.track = j;
            let q = m[0];
            let position = bp.vector(m[1], q & 0x03, 0.0, Channel::Position)?;
            let rotation = bp.rotation(m[2], (q >> 2) & 0x0F)?;
            let scale = bp.vector(m[3], q >> 6, 1.0, Channel::Scale)?;
            tracks.push(TrackChannels {
                position,
                rotation,
                scale,
                quantisation: q,
            });
        }
        if bp.rd.pos != params.transform_length {
            return Err(HavokError::BlockLength {
                block: params.block,
                expected: params.transform_length,
                found: bp.rd.pos,
            });
        }
        Ok(SplineBlock {
            frames: params.frames,
            tracks,
            knots: bp.knots,
            points: bp.points,
        })
    }

    /// The curve a [`CurveRef`] of this block describes (`None` if the reference does not
    /// fit the block's storage, which never happens for a parsed block).
    pub fn curve(&self, r: &CurveRef) -> Option<Curve<'_>> {
        let n = usize::from(r.last);
        let p = usize::from(r.degree);
        let dims = usize::from(r.dims);
        let ks = r.knot_start as usize;
        let ps = r.point_start as usize;
        let knots = self.knots.get(ks..ks.checked_add(n + p + 2)?)?;
        let points = self.points.get(ps..ps.checked_add((n + 1) * dims)?)?;
        Curve::new(p, knots, points, dims)
    }

    fn vector_at(&self, ch: &VectorChannel, u: f32) -> [f32; 3] {
        match ch {
            VectorChannel::Constant(v) => *v,
            VectorChannel::Spline {
                base,
                dynamic,
                curve,
            } => {
                let mut out = *base;
                let mut vals = [0.0f32; 3];
                if let Some(c) = self.curve(curve) {
                    c.evaluate(u, &mut vals);
                }
                let mut k = 0;
                for (i, o) in out.iter_mut().enumerate() {
                    if dynamic & (1 << i) != 0 {
                        *o = vals[k.min(2)];
                        k += 1;
                    }
                }
                out
            }
        }
    }

    fn rotation_at(&self, ch: &RotationChannel, u: f32) -> [f32; 4] {
        match ch {
            RotationChannel::Identity => [0.0, 0.0, 0.0, 1.0],
            RotationChannel::Static(q) => *q,
            RotationChannel::Dynamic(r) => self
                .curve(r)
                .map_or([0.0, 0.0, 0.0, 1.0], |c| c.evaluate_rotation(u)),
        }
    }

    /// Samples one track at local frame `u` (§7.7). `u` is clamped to `0 ..= F_b − 1`
    /// (NaN reads as 0). `None` if `track` is out of range.
    pub fn sample_track(&self, track: usize, u: f32) -> Option<Transform> {
        let ch = self.tracks.get(track)?;
        let u = self.clamp_u(u);
        Some(Transform {
            translation: self.vector_at(&ch.position, u),
            rotation: self.rotation_at(&ch.rotation, u),
            scale: self.vector_at(&ch.scale, u),
        })
    }

    /// Samples every track at local frame `u` into `out` (as many as fit); returns the number
    /// of tracks written. Does not allocate.
    pub fn sample(&self, u: f32, out: &mut [Transform]) -> usize {
        let u = self.clamp_u(u);
        let n = self.tracks.len().min(out.len());
        for (ch, o) in self.tracks.iter().zip(out.iter_mut()) {
            *o = Transform {
                translation: self.vector_at(&ch.position, u),
                rotation: self.rotation_at(&ch.rotation, u),
                scale: self.vector_at(&ch.scale, u),
            };
        }
        n
    }

    fn clamp_u(&self, u: f32) -> f32 {
        let last = self.frames.saturating_sub(1) as f32;
        if u.is_nan() { 0.0 } else { u.clamp(0.0, last) }
    }
}

/// A decoded `hkaSplineCompressedAnimation` (§5.1): clip-level numbers and the blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineAnimation {
    /// Frames *N*.
    pub num_frames: u32,
    /// `maxFramesPerBlock` *F*.
    pub max_frames_per_block: u32,
    /// `blockDuration` (seconds).
    pub block_duration: f32,
    /// `blockInverseDuration`.
    pub block_inverse_duration: f32,
    /// `frameDuration` (seconds).
    pub frame_duration: f32,
    /// The animation's `duration` (seconds), which governs sampling.
    pub duration: f32,
    /// Transform tracks.
    pub tracks: usize,
    /// The decoded blocks; never empty in parsed values (hand-built values are not checked;
    /// sampling an empty list writes nothing).
    pub blocks: Vec<SplineBlock>,
}

/// Clamps a sample time per §7.4 step 1: non-finite or negative → 0, above `duration` →
/// `duration`. A non-finite or negative `duration` counts as 0.
pub fn clamp_time(t: f32, duration: f32) -> f32 {
    let d = if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        0.0
    };
    if !t.is_finite() || t < 0.0 {
        0.0
    } else {
        t.min(d)
    }
}

impl SplineAnimation {
    /// Block and local frame for time `t` (§7.4).
    pub fn locate(&self, t: f32) -> (usize, f32) {
        let t = clamp_time(t, self.duration);
        let last_block = self.blocks.len().saturating_sub(1);
        let b = (t * self.block_inverse_duration).floor();
        // `as` saturates; NaN becomes 0.
        let b = (b as usize).min(last_block);
        let frames = self.blocks.get(b).map_or(1, |bl| bl.frames);
        let u = (t - b as f32 * self.block_duration) / self.frame_duration;
        let u = if u.is_nan() {
            0.0
        } else {
            u.clamp(0.0, frames.saturating_sub(1) as f32)
        };
        (b, u)
    }

    /// Samples every track at time `t` into `out`; returns the number written.
    pub fn sample(&self, t: f32, out: &mut [Transform]) -> usize {
        let (b, u) = self.locate(t);
        self.blocks.get(b).map_or(0, |bl| bl.sample(u, out))
    }

    /// Samples clip frame `f` exactly (no time conversion): block `min(f / (F − 1), B − 1)`,
    /// local frame `f − b × (F − 1)`. Frames past the end read as the last frame.
    pub fn sample_frame(&self, f: u32, out: &mut [Transform]) -> usize {
        let step = self.max_frames_per_block.saturating_sub(1).max(1);
        let f = f.min(self.num_frames.saturating_sub(1));
        let b = ((f / step) as usize).min(self.blocks.len().saturating_sub(1));
        let local = f.saturating_sub(b as u32 * step);
        self.blocks
            .get(b)
            .map_or(0, |bl| bl.sample(local as f32, out))
    }
}
