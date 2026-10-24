// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI side of text elements: run-time text, shaping through the client
//! ([`crate::env::UiEnv::shape_text`]) and the truncated-text tooltip
//! (`ui-behaviour.md` §10).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::property::PropertyValue;

use crate::env::{PreparedText, TextRequest};
use crate::events::MsgArg;
use crate::paint::PaintOp;
use crate::scaling::{GlyphCache, target_height};
use crate::text::{
    DEFAULT_GLYPH_CACHE_BYTES, FontFace, FontId, FontLadder, LayoutParams, MAX_FALLBACK_PX,
    MIN_FALLBACK_PX, ScaleCtx, TEXT_SCALE_RANGE, TesseraFont, TextLayout, TextParamsReader,
    UiTextSource, ceil_div_scale, layout_scaled, physical_params, relayout_lines,
};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::tessera_msg;

/// Most Tessera-font faces kept rasterised at once (one per pixel size, so every size of
/// [`MIN_FALLBACK_PX`]..=[`MAX_FALLBACK_PX`] fits; Tessera bound).
const MAX_TESSERA_FACES: usize = (MAX_FALLBACK_PX - MIN_FALLBACK_PX + 1) as usize;

/// Most bytes of rasterised Tessera-font faces kept at once, whatever the glyph share (Tessera
/// bound). Within it the faces take at most half of the glyph share of `ui-scaling.md` §8 S6
/// (revision 11 answer, SC18): past that the least recently used face is dropped, one at a
/// time, but the face in use is always kept ([`TextScaling::set_glyph_share`]).
const MAX_TESSERA_FACE_BYTES: usize = 48 << 20;

/// The text-scaling state of the painter (`ui-scaling.md` §6.4, §6.5, §7.1, §7.5): the
/// derived-glyph cache, the registered font ladders, the layout-parameter reader, the text
/// options, Tessera's own font at the sizes in use, the elements the text factor reaches, and
/// the physical text drawn for each element.
pub(crate) struct TextScaling {
    pub cache: GlyphCache,
    /// The glyph caches' share of the S6 total (`ui-scaling.md` §8): the derived-glyph cache's
    /// budget ([`TextScaling::set_glyph_share`]).
    glyph_share: usize,
    /// Ladder of each registered face, by face id (§6.4.2).
    pub ladders: HashMap<FontId, FontLadder>,
    pub params: Option<TextParamsReader>,
    /// `ui_text_scale` *x* (§7.5).
    pub factor: f64,
    /// `ui_text_source` (§6.4.3 item 4).
    pub source: UiTextSource,
    /// The bundled font, parsed on first use (`Some(None)`: it did not parse).
    tessera: Option<Option<TesseraFont>>,
    /// Tessera's font rasterised at each pixel size in use, with the tick of its last use.
    tessera_faces: HashMap<u32, (Arc<FontFace>, u64)>,
    /// Use counter of [`TextScaling::tessera_faces`] and the bytes they hold.
    tessera_tick: u64,
    tessera_bytes: usize,
    /// Tessera-font faces rasterised so far (each cache miss).
    tessera_rasterised: u64,
    /// Elements whose text takes the text factor although they are not tooltips or scrollable
    /// multi-line text: the chat log lines and the entry line (§7.5).
    pub reflow: HashSet<ElementKey>,
    /// Physical text drawn per element, reused while its key is unchanged.
    pub runs: HashMap<ElementKey, ScaledRuns>,
    /// Bumped by every option or registration change; part of each run key.
    pub epoch: u64,
    /// Glyphs derived and time spent on scaled text since the last frame's figures (S7).
    pub derived: u64,
    pub derive_ns: u64,
    /// Ladder steps accepted since the last frame's figures: count, worker time and the
    /// longest step, nanoseconds (S7).
    pub ladder_steps: u32,
    pub ladder_step_ns: u64,
    pub ladder_step_max_ns: u64,
}

impl std::fmt::Debug for TextScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextScaling")
            .field("cache", &self.cache)
            .field("ladders", &self.ladders.len())
            .field("params", &self.params.is_some())
            .field("factor", &self.factor)
            .field("source", &self.source)
            .field("tessera_faces", &self.tessera_faces.len())
            .field("reflow", &self.reflow.len())
            .field("runs", &self.runs.len())
            .finish_non_exhaustive()
    }
}

impl Default for TextScaling {
    fn default() -> Self {
        Self {
            cache: GlyphCache::new(DEFAULT_GLYPH_CACHE_BYTES),
            glyph_share: DEFAULT_GLYPH_CACHE_BYTES,
            ladders: HashMap::new(),
            params: None,
            factor: 1.0,
            source: UiTextSource::Game,
            tessera: None,
            tessera_faces: HashMap::new(),
            tessera_tick: 0,
            tessera_bytes: 0,
            tessera_rasterised: 0,
            reflow: HashSet::new(),
            runs: HashMap::new(),
            epoch: 0,
            derived: 0,
            derive_ns: 0,
            ladder_steps: 0,
            ladder_step_ns: 0,
            ladder_step_max_ns: 0,
        }
    }
}

/// The inputs of the physical **layout** of one element's text: when any changes, the text is
/// laid out again. (The prepared text itself is compared by address.)
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutKey {
    pub c: u64,
    pub x: u64,
    /// The stretch vector *σ* the text is stretched by (`ui-scaling.md` §6.1), as bits.
    pub sigma: [u64; 2],
    pub params: Option<LayoutParams>,
    pub box_phys: [i32; 2],
    /// [`TextScaling::epoch`]: the options, ladders and reader the fonts are resolved with.
    pub epoch: u64,
}

/// The inputs of the **draw** only: when only these change (a scroll step, a move), the cached
/// physical layout is drawn again without laying the text out again.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DrawKey {
    pub origin: [i32; 2],
    pub clip: crate::paint::Rect,
    pub outline: bool,
    pub outline_colour: u32,
}

/// The physical text of one element (`ui-scaling.md` §6.4.5): its layout in physical pixels,
/// the paint operations in canvas pixels and the window-physical position of its text origin
/// (for the hit test of §6.4.6).
#[derive(Debug)]
pub(crate) struct ScaledRuns {
    /// The prepared text the runs were drawn from (kept so that its address stays unique).
    pub prepared: Arc<PreparedText>,
    pub layout_key: LayoutKey,
    pub draw_key: DrawKey,
    pub layout: Arc<TextLayout>,
    pub ops: Option<Vec<PaintOp>>,
    /// Canvas offset of text coordinate (0, 0) from the element's canvas box origin (minus
    /// the scroll offsets in canvas pixels), for the hit test of §6.4.6.
    pub text_offset: [i32; 2],
    /// The stretch vector the drawn text was stretched by ((1, 1) outside stretched surfaces);
    /// the hit test divides by it (§6.4.6).
    pub sigma: [f64; 2],
}

/// Fonts and ladders resolved for one scaled draw (see [`TextScaling::resolve_fonts`]).
pub(crate) struct ResolvedFonts {
    pub fonts: Vec<Option<Arc<FontFace>>>,
    pub ladders: Vec<FontLadder>,
    /// The fonts were replaced by Tessera's font (§6.4.3 item 4).
    pub replaced: bool,
    /// The replacement faces are rasterised at the target heights, so the text is laid out and
    /// drawn at the identity scale with them ([`ResolvedFonts::ctx`]).
    identity_ctx: bool,
}

impl ResolvedFonts {
    /// The scale context to lay out and draw with at content scale `c` and text factor `x`.
    pub fn ctx(&self, c: f64, x: f64) -> ScaleCtx<'_> {
        if self.identity_ctx {
            ScaleCtx {
                c: 1.0,
                x: 1.0,
                ladders: &[],
            }
        } else {
            ScaleCtx {
                c,
                x,
                ladders: &self.ladders,
            }
        }
    }
}

impl TextScaling {
    /// Tessera's bundled font rasterised at `px` (clamped to the supported sizes), cached. The
    /// cache holds faces up to half the S6 glyph share ([`Self::face_cap`]); past that the
    /// least recently used face is dropped, one at a time. The face asked for is the face in
    /// use and is always kept, even when it alone is larger than its half; the derived-glyph
    /// cache gets the rest of the share (`ui-scaling.md` §8 S6 revision 11 answer).
    fn tessera_face(&mut self, px: i32) -> Option<Arc<FontFace>> {
        let px = u32::try_from(px.clamp(MIN_FALLBACK_PX as i32, MAX_FALLBACK_PX as i32)).ok()?;
        self.tessera_tick = self.tessera_tick.wrapping_add(1);
        let tick = self.tessera_tick;
        if let Some((f, used)) = self.tessera_faces.get_mut(&px) {
            *used = tick;
            return Some(Arc::clone(f));
        }
        let font = self
            .tessera
            .get_or_insert_with(|| match TesseraFont::bundled() {
                Ok(f) => Some(f),
                Err(e) => {
                    tracing::warn!(error = %e, "bundled font does not parse");
                    None
                }
            })
            .as_ref()?;
        self.tessera_rasterised = self.tessera_rasterised.saturating_add(1);
        let face = match font.rasterise(px, crate::text::DEFAULT_RANGES) {
            Ok(f) => Arc::new(f),
            Err(e) => {
                tracing::warn!(px, error = %e, "bundled font does not rasterise");
                return None;
            }
        };
        let bytes = face.memory_bytes();
        while !self.tessera_faces.is_empty()
            && (self.tessera_faces.len() >= MAX_TESSERA_FACES
                || self.tessera_bytes.saturating_add(bytes) > self.face_cap())
        {
            let Some(old) = self
                .tessera_faces
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some((f, _)) = self.tessera_faces.remove(&old) {
                self.tessera_bytes = self.tessera_bytes.saturating_sub(f.memory_bytes());
            }
        }
        self.tessera_bytes = self.tessera_bytes.saturating_add(bytes);
        self.tessera_faces.insert(px, (Arc::clone(&face), tick));
        self.sync_glyph_budget();
        Some(face)
    }

    /// The faces' part of the glyph share: half of it (`ui-scaling.md` §8 S6 revision 11
    /// answer), at most [`MAX_TESSERA_FACE_BYTES`].
    fn face_cap(&self) -> usize {
        (self.glyph_share / 2).min(MAX_TESSERA_FACE_BYTES)
    }

    /// The derived-glyph cache's budget: the glyph share minus the faces' bytes (0 when a
    /// face in use alone takes more than its half and the rest).
    fn sync_glyph_budget(&mut self) {
        self.cache
            .set_budget(self.glyph_share.saturating_sub(self.tessera_bytes));
    }

    /// Sets the glyph caches' share of the S6 total (`ui-scaling.md` §8 S6, revision 11
    /// answer): Tessera's rasterised faces take at most half of it (the least recently used
    /// are dropped down to it, but the most recently used face, the face in use, is always
    /// kept), and the derived-glyph cache the rest, which it evicts down to.
    pub(crate) fn set_glyph_share(&mut self, bytes: usize) {
        self.glyph_share = bytes;
        let cap = self.face_cap();
        while self.tessera_faces.len() > 1 && self.tessera_bytes > cap {
            let Some(old) = self
                .tessera_faces
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some((f, _)) = self.tessera_faces.remove(&old) {
                self.tessera_bytes = self.tessera_bytes.saturating_sub(f.memory_bytes());
            }
        }
        self.sync_glyph_budget();
    }

    /// The glyph share of [`Self::set_glyph_share`].
    pub(crate) fn glyph_share(&self) -> usize {
        self.glyph_share
    }

    /// Bytes of Tessera's rasterised faces held.
    pub(crate) fn tessera_bytes(&self) -> usize {
        self.tessera_bytes
    }

    /// The number of Tessera-font faces held and the number rasterised so far (tests).
    #[cfg(test)]
    pub(crate) fn tessera_face_counts(&self) -> (usize, u64) {
        (self.tessera_faces.len(), self.tessera_rasterised)
    }

    /// Whether a face is Tessera's bundled font (`fonts.md` §11 item 2), which is always
    /// rasterised directly at the target height (`ui-scaling.md` §6.4.3).
    fn is_bundled(&mut self, face: &FontFace) -> bool {
        let FontId::Tessera { font, .. } = face.id() else {
            return false;
        };
        let bundled = self
            .tessera
            .get_or_insert_with(|| TesseraFont::bundled().ok())
            .as_ref()
            .map(TesseraFont::key);
        bundled == Some(font)
    }

    /// The fonts and ladders a text is drawn with at content scale `c` and text factor `x`
    /// (`ui-scaling.md` §6.4.2, §6.4.3): with `ui_text_source = "tessera"`, or when every font
    /// of the text is Tessera's own and the scale is not the identity, each font is replaced by
    /// Tessera's font rasterised directly at the target height (§6.4.3 item 4 and the last
    /// paragraph); otherwise each slot keeps its font with its registered ladder (a font with no
    /// registered ladder is a ladder of its own).
    pub fn resolve_fonts(
        &mut self,
        fonts: &[Option<Arc<FontFace>>],
        c: f64,
        x: f64,
    ) -> ResolvedFonts {
        let identity = ScaleCtx { c, x, ladders: &[] }.is_identity();
        let mut any = false;
        let mut all_bundled = true;
        for f in fonts.iter().flatten() {
            any = true;
            if !self.is_bundled(f) {
                all_bundled = false;
            }
        }
        let replace = any && (self.source == UiTextSource::Tessera || (!identity && all_bundled));
        let max = MAX_FALLBACK_PX as i32;
        let over = fonts
            .iter()
            .flatten()
            .any(|f| target_height(f.line_height_i32(), c, x) > max);
        if replace && over && !identity {
            // A target taller than the rasteriser's bound: that slot is derived from the
            // tallest rasterised size by the distance method (§6.4.4, a one-record ladder);
            // every other slot keeps its face made for its target height.
            let mut ladders = Vec::with_capacity(fonts.len());
            let mut ok = true;
            for f in fonts {
                ladders.push(match f {
                    Some(face) => {
                        let t = target_height(face.line_height_i32(), c, x);
                        match self.tessera_face(t.min(max)) {
                            Some(r) if t > max => FontLadder::single(r),
                            Some(r) => FontLadder::direct(t, r),
                            None => {
                                ok = false;
                                FontLadder::default()
                            }
                        }
                    }
                    None => FontLadder::default(),
                });
            }
            if ok {
                return ResolvedFonts {
                    fonts: fonts.to_vec(),
                    ladders,
                    replaced: true,
                    identity_ctx: false,
                };
            }
        } else if replace {
            let mut out = Vec::with_capacity(fonts.len());
            let mut ok = true;
            for f in fonts {
                out.push(match f {
                    Some(face) => {
                        let t = target_height(face.line_height_i32(), c, x);
                        match self.tessera_face(t) {
                            Some(r) => Some(r),
                            None => {
                                ok = false;
                                Some(Arc::clone(face))
                            }
                        }
                    }
                    None => None,
                });
            }
            if ok {
                return ResolvedFonts {
                    fonts: out,
                    ladders: Vec::new(),
                    replaced: true,
                    identity_ctx: true,
                };
            }
        }
        let ladders = fonts
            .iter()
            .map(|f| {
                f.as_ref()
                    .and_then(|face| self.ladders.get(&face.id()).cloned())
                    .unwrap_or_default()
            })
            .collect();
        ResolvedFonts {
            fonts: fonts.to_vec(),
            ladders,
            replaced: false,
            identity_ctx: false,
        }
    }
}

impl Ui {
    /// The text an element shows: its run-time text, or its resolved `UICore_Text_entry`.
    #[must_use]
    pub fn text_of(&mut self, k: ElementKey) -> String {
        if let Some(t) = self.el(k).and_then(|e| e.text.runtime.clone()) {
            return t;
        }
        match self.property(k, self.v.pids.text_entry).cloned() {
            Some(PropertyValue::StringInfo(si)) => self.env.resolve_string(&si).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Sets (or clears) the run-time text of an element.
    pub fn set_text(&mut self, k: ElementKey, text: Option<String>) {
        let text = text.map(|t| {
            if t.encode_utf16().count() > crate::text::MAX_TEXT_UNITS {
                t.chars().take(crate::text::MAX_TEXT_UNITS / 2).collect()
            } else {
                t
            }
        });
        let Some(e) = self.el_mut(k) else { return };
        if e.text.runtime == text {
            return;
        }
        e.text.runtime = text;
        e.text.dirty = true;
        self.note_paint_dirty(k);
        self.painter.touched = true;
    }

    /// Text changed by editing: sets it and posts the text-changed notification.
    pub(crate) fn set_text_edited(&mut self, k: ElementKey, text: String) {
        self.set_text(k, Some(text));
        self.post(k, tessera_msg::TEXT_CHANGED, MsgArg::default());
    }

    /// The text prepared for drawing, shaping it again when needed. Applies the
    /// truncated-text tooltip (§10.2) when the cut state changes.
    pub(crate) fn prepared_text(&mut self, k: ElementKey) -> Option<Arc<PreparedText>> {
        let e = self.el(k)?;
        if !e.class.is_text() && e.plugin.is_none() {
            return None;
        }
        if !e.text.dirty {
            return e.text.prepared.clone();
        }
        let text = self.text_of(k);
        let props = self.properties(k);
        let b = self.layout_box(k)?;
        let req = TextRequest {
            text: &text,
            props: &props,
            size: (b.w(), b.h()),
            wrap_width: None,
        };
        let prepared = if text.is_empty() {
            None
        } else {
            self.env.shape_text(&req)
        };
        let cut = prepared.as_ref().is_some_and(|p| p.truncated);
        let was_cut = self.el(k).is_some_and(|e| e.text.cut);
        if let Some(e) = self.el_mut(k) {
            e.text.prepared = prepared.clone();
            // Shaping that waits for fonts is tried again on the next paint.
            e.text.dirty = prepared.is_none() && !text.is_empty();
            e.text.cut = cut;
        }
        if cut != was_cut || cut {
            self.truncation_tooltip(k, &text, cut);
        }
        prepared
    }

    /// `ui-behaviour.md` §10.2.
    fn truncation_tooltip(&mut self, k: ElementKey, full: &str, cut: bool) {
        let p = self.v.pids;
        if !self.prop(k, p.text_auto_tooltip).bool() {
            return;
        }
        let own = match self.property(k, p.tooltip_entry).cloned() {
            Some(PropertyValue::StringInfo(si)) => {
                self.env.resolve_string(&si).filter(|s| !s.is_empty())
            }
            _ => None,
        };
        if cut {
            let t = match &own {
                Some(o) => self.env.truncated_tooltip(full, o),
                None => full.to_string(),
            };
            self.set_tooltip_text(k, Some(t));
            self.set_property(k, p.tooltips_on, PropertyValue::Bool(true));
        } else if let Some(o) = own {
            self.set_tooltip_text(k, Some(o));
            self.set_property(k, p.tooltips_on, PropertyValue::Bool(true));
        } else {
            self.set_tooltip_text(k, None);
            self.set_property(k, p.tooltips_on, PropertyValue::Bool(false));
        }
        self.recompute_mouse_visible(k);
    }

    /// Re-shapes every text element (after fonts arrive).
    pub fn invalidate_text(&mut self) {
        for k in self.arena.keys() {
            if let Some(e) = self.el_mut(k)
                && (e.class.is_text() || e.plugin.is_some())
            {
                e.text.dirty = true;
                self.note_paint_dirty(k);
            }
        }
        self.painter.touched = true;
    }
}

impl Ui {
    /// Registers the layout-parameter reader of the crisp text path (`ui-scaling.md` §6.4.5,
    /// [`crate::text::TextParamsReader`]), or removes it. With a reader, text drawn at a content
    /// scale other than 1 (or with the text factor of §7.5, or Tessera's font as the text
    /// source) is laid out again in physical pixels; without one, the shaper's line breaks are
    /// kept ([`crate::text::relayout_lines`]) and **the text factor of [`Ui::set_text_scale`] is
    /// not applied** (it needs a new layout). A client that offers `ui_text_scale` must
    /// register a reader.
    pub fn set_text_layout_params(&mut self, reader: Option<TextParamsReader>) {
        self.painter.text.params = reader;
        self.text_scaling_changed();
    }

    /// Registers font ladders (`ui-scaling.md` §6.4.2): the sibling records of each logical
    /// font, built by the client from the plugin font enum and the records present. Each face
    /// of a ladder is looked up by its [`FontId`]; a face of no registered ladder is a ladder
    /// of its own. Replaces the ladders registered before.
    pub fn set_font_ladders(&mut self, ladders: Vec<FontLadder>) {
        let mut map = HashMap::new();
        for l in ladders {
            for h in l.heights() {
                if let Some(f) = l.face_at(h) {
                    map.insert(f.id(), l.clone());
                }
            }
        }
        self.painter.text.ladders = map;
        // The glyph cache is kept: its keys name the record a glyph is derived from and the
        // source kind (`ui-scaling.md` §6.5), so new ladders only change which keys are looked
        // up. The texts are drawn again with the new ladders.
        self.text_scaling_changed();
    }

    /// Sets the text factor `ui_text_scale` *x* (`ui-scaling.md` §7.1, §7.5): 0.80–1.50 in
    /// steps of 0.05 (rounded to the nearest step and clamped; a non-finite value is 1.0). It
    /// enlarges the plain tooltip's text, the chat window's log and entry line, and multi-line
    /// Text elements inside a Scrollable; all other text keeps *x* = 1. Applied live.
    ///
    /// Takes effect only while a layout-parameter reader is registered
    /// ([`Ui::set_text_layout_params`]); without one the factor is stored but no text takes it.
    pub fn set_text_scale(&mut self, x: f64) {
        let x = if x.is_finite() {
            ((x / 0.05).round() * 0.05).clamp(TEXT_SCALE_RANGE.0, TEXT_SCALE_RANGE.1)
        } else {
            1.0
        };
        if (self.painter.text.factor - x).abs() < 1e-12 {
            return;
        }
        self.painter.text.factor = x;
        self.text_scaling_changed();
    }

    /// The text factor of [`Ui::set_text_scale`].
    #[must_use]
    pub fn text_scale(&self) -> f64 {
        self.painter.text.factor
    }

    /// Sets `ui_text_source` (`ui-scaling.md` §6.4.3 item 4, §7.1). Applied live.
    pub fn set_text_source(&mut self, source: UiTextSource) {
        if self.painter.text.source == source {
            return;
        }
        self.painter.text.source = source;
        self.text_scaling_changed();
    }

    /// The text source of [`Ui::set_text_source`].
    #[must_use]
    pub fn text_source(&self) -> UiTextSource {
        self.painter.text.source
    }

    /// Sets the glyph caches' share (`ui-scaling.md` §6.5; their share of the S6 total, §8):
    /// Tessera's rasterised faces take at most half of it and the derived-glyph cache the
    /// rest (revision 11 answer). The caches evict down to it. [`Self::set_image_memory`] sets
    /// it from the S6 rule; this sets it directly.
    pub fn set_glyph_cache_budget(&mut self, bytes: usize) {
        self.painter.text.set_glyph_share(bytes);
    }

    /// The glyph caches' share in bytes, and the bytes they hold now: (share, derived glyphs,
    /// Tessera's faces).
    #[must_use]
    pub fn glyph_memory(&self) -> (usize, usize, usize) {
        let t = &self.painter.text;
        (t.glyph_share(), t.cache.bytes(), t.tessera_bytes())
    }

    /// Takes the glyphs of a ladder step derived on a worker ([`crate::text::derive_ladder_step`],
    /// `ui-scaling.md` §6.5) into the derived-glyph cache, and records its worker time for the
    /// next frame's figures (`PaintFrameStats::ladder_step_ns`, budget S7). The glyphs count
    /// toward the cache's budget like any other; text already drawn is not repainted.
    pub fn accept_ladder_step(&mut self, step: crate::text::LadderStep) {
        let t = &mut self.painter.text;
        for (k, g) in step.glyphs {
            t.cache.insert(k, g);
        }
        let ns = u64::try_from(step.elapsed.as_nanos()).unwrap_or(u64::MAX);
        t.ladder_steps = t.ladder_steps.saturating_add(1);
        t.ladder_step_ns = t.ladder_step_ns.saturating_add(ns);
        t.ladder_step_max_ns = t.ladder_step_max_ns.max(ns);
    }

    /// The derived-glyph cache's figures: (glyphs held, bytes held, glyphs derived so far).
    #[must_use]
    pub fn glyph_cache_stats(&self) -> (usize, usize, u64) {
        let c = &self.painter.text.cache;
        (c.len(), c.bytes(), c.insertions())
    }

    /// Marks an element whose text takes the text factor of `ui-scaling.md` §7.5 although it is
    /// neither a tooltip nor a scrollable multi-line text (the chat window's log lines and entry
    /// line), or clears the mark.
    pub fn set_text_reflow(&mut self, k: ElementKey, on: bool) {
        let changed = if on {
            self.painter.text.reflow.insert(k)
        } else {
            self.painter.text.reflow.remove(&k)
        };
        if changed {
            self.mark_dirty(k);
        }
    }

    /// The text factor that reaches element `k` (`ui-scaling.md` §7.5): *x* for the plain
    /// tooltip's text (the one [`Ui::show_tooltip`] builds and sizes from its scaled text),
    /// marked elements ([`Ui::set_text_reflow`]) and multi-line Text elements inside a
    /// Scrollable (a linked scroll bar of its own or a Scrollable ancestor; multi-line read
    /// through the layout-parameter reader); 1.0 for all other text and whenever no
    /// layout-parameter reader is registered (the factor needs a new layout). Tooltip panels
    /// built by game code ([`Ui::set_tooltip_panels`]) size their rows from unscaled extents,
    /// so their text keeps *x* = 1 until they measure through [`Ui::measure_text_scaled`].
    pub(crate) fn text_factor_for(&self, k: ElementKey, params: Option<&LayoutParams>) -> f64 {
        let x = self.painter.text.factor;
        if (x - 1.0).abs() < 1e-12 || self.painter.text.params.is_none() {
            return 1.0;
        }
        if self.factor_marked(k) {
            return x;
        }
        let multi_line = params.is_some_and(|p| !p.one_line);
        if multi_line && self.text_in_scrollable(k) {
            return x;
        }
        1.0
    }

    /// The cheap part of [`Ui::text_factor_for`]: the plain tooltip's text or a marked element.
    fn factor_marked(&self, k: ElementKey) -> bool {
        (self.tooltip.text == Some(k) && self.tooltip.panel.is_some())
            || self.painter.text.reflow.contains(&k)
    }

    /// Whether `k` is a Text element inside a Scrollable (§7.5; see
    /// [`Ui::scrolls_at_or_above`]).
    fn text_in_scrollable(&self, k: ElementKey) -> bool {
        self.el(k)
            .is_some_and(|e| e.class == crate::widgets::WidgetClass::Text)
            && self.scrolls_at_or_above(k)
    }

    /// Whether text at the identity mapping (no UI screen) still takes the scaled path, and
    /// with which layout parameters: the text source is Tessera's font, or the text factor
    /// reaches the element (§7.1, §7.5). The cheap conditions are checked before the reader
    /// runs, and the reader runs at most once (`None`: the identity path).
    pub(crate) fn text_scaling_params(&self, k: ElementKey) -> Option<Option<LayoutParams>> {
        if self.painter.text.source == UiTextSource::Tessera {
            return Some(self.text_params_of(k, None));
        }
        let t = &self.painter.text;
        if (t.factor - 1.0).abs() < 1e-12 || t.params.is_none() {
            return None;
        }
        if self.factor_marked(k) {
            return Some(self.text_params_of(k, None));
        }
        if !self.text_in_scrollable(k) {
            return None;
        }
        let p = self.text_params_of(k, None)?;
        (!p.one_line).then_some(Some(p))
    }

    /// Whether text element `k` is "inside a Scrollable" for `ui-scaling.md` §7.5: it has a
    /// linked scroll bar of its own (`ui-widgets.md` §5.1), or an ancestor is a Scrollable
    /// (ListBox, Tree or Text, §5). Every Text element is itself of a Scrollable class, so the
    /// element's own class alone does not count: a plain multi-line label keeps *x* = 1. This
    /// conservative reading is a Tessera choice the spec does not settle (reported for an
    /// implementer OPEN of §7.5).
    fn scrolls_at_or_above(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        if e.widget
            .scroll
            .as_ref()
            .is_some_and(|s| s.vbar.is_some() || s.hbar.is_some())
        {
            return true;
        }
        let mut cur = e.parent;
        let mut n = 0;
        while let Some(c) = cur {
            let Some(e) = self.el(c) else { return false };
            if e.widget.scroll.is_some() {
                return true;
            }
            cur = e.parent;
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 4 {
                return false;
            }
        }
        false
    }

    /// The layout parameters of element `k` from the registered reader, at its layout-box size.
    pub(crate) fn text_params_of(&self, k: ElementKey, wrap: Option<i32>) -> Option<LayoutParams> {
        let reader = self.painter.text.params.as_ref()?;
        let b = self.layout_box(k)?;
        let props = self.properties(k);
        Some(reader(&props, (b.w(), b.h()), wrap))
    }

    /// Something every scaled text reads changed: draws all text again. The runs drawn so far
    /// are kept (their keys carry the old epoch, so none is reused) and answer hit tests until
    /// the next paint replaces them (§6.4.6).
    fn text_scaling_changed(&mut self) {
        self.painter.text.epoch = self.painter.text.epoch.wrapping_add(1);
        self.invalidate_paint();
    }

    /// The text extent of `text` in element `k` in UI units, measured as the element shows it
    /// (`ui-scaling.md` §6.4.5): shaped by the client, laid out at the element's content scale
    /// *c* with the text factor that reaches it (§7.5) and taken back to UI units (÷ *c*,
    /// rounded up). `size` and `wrap` are those of the measuring request. `None` while the
    /// shaper cannot lay the text out (its fonts are still loading). For game code that sizes
    /// rows or panels from their text (tooltips, list rows, text scroll ranges); at the
    /// identity it is the shaped extent.
    pub(crate) fn measure_text_scaled(
        &mut self,
        k: ElementKey,
        text: &str,
        size: (i32, i32),
        wrap: Option<i32>,
    ) -> Option<(i32, i32)> {
        let props = self.properties(k);
        let req = TextRequest {
            text,
            props: &props,
            size,
            wrap_width: wrap,
        };
        let shaped = self.env.shape_text(&req)?;
        let c = self.text_content_scale(k);
        let x = if self.painter.text.params.is_some() {
            let reader = self.painter.text.params.as_ref()?;
            let p = reader(&props, size, wrap);
            self.text_factor_for(k, Some(&p))
        } else {
            1.0
        };
        Some(self.scaled_text_extent(&shaped, &props, size, wrap, c, x))
    }

    /// The vertical content scale *c* of element `k` (`ui-scaling.md` §3.4: *Ky* times the
    /// node scale on or above it), 1 with no UI screen. The text of a stretched surface is laid
    /// out at its uniform part (§6.1): a node scale that differs between the axes is a stretch
    /// vector, so only *Ky* counts then.
    pub(crate) fn text_content_scale(&self, k: ElementKey) -> f64 {
        let Some(f) = self.ui_screen else { return 1.0 };
        let (mx, my, _) = self.node_scale_at_or_above(k);
        if (mx - my).abs() > 1e-6 {
            f.ky()
        } else {
            f.ky() * f64::from(my)
        }
    }

    /// The text extent of a prepared text in UI units at content scale `c` with text factor
    /// `x` (`ui-scaling.md` §6.4.5: the physical extent ÷ `c`, rounded up), for elements sized
    /// from their text (tooltips, `ui-behaviour.md` §6.4). `props`, `size` and `wrap` are those
    /// of the measuring request. At the identity (`c × x` = 1, game fonts) it is the prepared
    /// extent unchanged.
    pub(crate) fn scaled_text_extent(
        &mut self,
        pt: &PreparedText,
        props: &[(u32, PropertyValue)],
        size: (i32, i32),
        wrap: Option<i32>,
        c: f64,
        x: f64,
    ) -> (i32, i32) {
        let c = if c.is_finite() && c > 0.0 { c } else { 1.0 };
        let rf = self.painter.text.resolve_fonts(&pt.fonts, c, x);
        let identity = ScaleCtx { c, x, ladders: &[] }.is_identity();
        if identity && !rf.replaced && (c - 1.0).abs() < 1e-12 {
            return pt.extent;
        }
        let ctx = rf.ctx(c, x);
        let map = |v: i32| -> i32 {
            (f64::from(v) * c)
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        };
        let suffix = pt.layout.truncation.as_ref().map(|t| t.suffix.as_slice());
        let lay = match self.painter.text.params.as_ref() {
            Some(reader) => {
                let p = reader(props, size, wrap);
                let pp = physical_params(&p, map, map, c);
                layout_scaled(&pt.chars, &rf.fonts, &pp, suffix, &ctx)
            }
            None => relayout_lines(&pt.layout, &pt.chars, &rf.fonts, &ctx, map, map, c),
        };
        match lay {
            Ok(l) => (
                ceil_div_scale(l.content_width, c),
                ceil_div_scale(l.content_height, c),
            ),
            Err(_) => pt.extent,
        }
    }

    /// The character of a text element under a pointer at window-physical pixel
    /// (`px`, `py`) (`ui-scaling.md` §6.4.6): the pointer minus the element's physical origin
    /// plus its scroll offsets in physical pixels, looked up in the physical layout the element
    /// was last drawn with ([`TextLayout::hit`]). In a stretched surface (§6.1) the pointer is
    /// first taken into the unstretched text (`unstretch_point`): the centre of its pixel
    /// relative to the text origin, divided by the stretch vector *σ*, since the text was laid
    /// out unstretched at the screen's factor and stretched by *σ* when drawn. Without a
    /// UI screen and before the element was drawn on the scaled path, the shaper's layout is
    /// used (layout units are physical pixels then). `None` when the element has no text, the
    /// point is on no character, or a UI screen is set and the element was not drawn yet.
    ///
    /// **During a geometry preview** of the surface the text is drawn into (§6.1; §6.4.6
    /// revision 15, anchor `s6-4-6-preview`) the canvas shows the text laid out at the held
    /// geometry, so the lookup uses that **held layout**: the pointer is taken into the held
    /// canvas by the inverse of its composition (`Ui::held_canvas_point`: 1 : 1 at the
    /// surface's current top-left while the node scales are the held ones, scaled by held
    /// canvas size ÷ current physical box size otherwise), then the text's drawn origin in
    /// held-canvas pixels is subtracted and, in a stretched surface, the held *σ* taken back.
    /// No layout at the current content scale is made for it.
    #[must_use]
    pub fn text_hit_test(&self, k: ElementKey, px: i32, py: i32) -> Option<usize> {
        if let Some(r) = self.painter.text.runs.get(&k) {
            let held = self
                .text_surface_owner(k)
                .and_then(|o| self.held_canvas_point(o, px, py));
            if let Some(point) = held {
                let [hx, hy] = point?;
                let x = hx.saturating_sub(r.draw_key.origin[0]);
                let y = hy.saturating_sub(r.draw_key.origin[1]);
                return r.layout.hit(
                    unstretch_point(x, r.sigma[0]),
                    unstretch_point(y, r.sigma[1]),
                );
            }
            let b = self.physical_box(k);
            let origin = [
                b.x0.saturating_add(r.text_offset[0]),
                b.y0.saturating_add(r.text_offset[1]),
            ];
            let [x, y] = crate::text::mouse_to_text_physical([px, py], origin, [0, 0]);
            return r.layout.hit(
                unstretch_point(x, r.sigma[0]),
                unstretch_point(y, r.sigma[1]),
            );
        }
        if self.ui_screen.is_some() {
            // The shaper's layout is in layout units, not physical pixels.
            return None;
        }
        let e = self.el(k)?;
        let pt = e.text.prepared.as_ref()?;
        let (sx, sy) = e.widget.scroll.as_ref().map_or((0, 0), |s| s.offset);
        let b = self.physical_box(k);
        let [x, y] = crate::text::mouse_to_text_physical([px, py], [b.x0, b.y0], [sx, sy]);
        pt.layout.hit(x, y)
    }
}

/// The stretched-surface step of the text hit test (`ui-scaling.md` §6.4.6) on one axis: `v`
/// is the pointer's physical pixel relative to the drawn text origin (in the stretched
/// picture), `s` the stretch vector's component *σ*. The pointer's position is the centre of
/// its pixel (§3.6), so the point in the unstretched text is ⌊(*v* + ½) ÷ *σ*⌋; the factor
/// *Se* of §6.4.6 is already in `v` and in the layout, both in physical pixels at the screen's
/// factor. At *σ* = 1 (and for a non-finite or non-positive *σ*) this is `v` itself, the
/// unstretched rule.
#[must_use]
pub(crate) fn unstretch_point(v: i32, s: f64) -> i32 {
    if (s - 1.0).abs() < 1e-12 || !s.is_finite() || s <= 0.0 {
        return v;
    }
    ((f64::from(v) + 0.5) / s)
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::TextScaling;
    use crate::ControlKind;
    use crate::env::{MemoryEnv, NullEnv, PreparedText};
    use crate::paint::Unlimited;
    use crate::scaling::{UiScreen, safe_area, target_height};
    use crate::text::{
        CharListBuilder, DEFAULT_RANGES, FontFace, FontId, LayoutParams, TesseraFont, UiTextSource,
        layout, layout_scaled,
    };
    use crate::ui::Ui;
    use crate::vocab::Vocab;

    fn params(one_line: bool) -> LayoutParams {
        LayoutParams {
            one_line,
            ..LayoutParams::default()
        }
    }

    /// The bundled font at `px`, relabelled as a data font record (invented DID).
    fn data_face(px: u32) -> Arc<FontFace> {
        let f = TesseraFont::bundled()
            .expect("bundled font")
            .rasterise(px, DEFAULT_RANGES)
            .expect("rasterise");
        Arc::new(
            FontFace::from_parts(
                FontId::Data(0x4200_0C00 + px),
                f.line_height(),
                f.baseline(),
                f.borders(),
                f.glyphs().to_vec(),
                Arc::clone(f.foreground()),
                None,
            )
            .expect("face"),
        )
    }

    /// `ui-scaling.md` §6.4.6, §3.6: in a stretched surface the pointer's pixel centre is taken
    /// back by σ; at σ = 1 the pixel itself is looked up. At σ = 1.5 pixel 1 (centre 1.5) lies
    /// in unstretched column 1, which its left edge alone (1 ÷ 1.5) would put in column 0.
    #[test]
    fn stretched_hit_test_uses_the_pixel_centre() {
        use super::unstretch_point;
        for v in [-3, 0, 1, 7, 100] {
            assert_eq!(unstretch_point(v, 1.0), v);
            assert_eq!(unstretch_point(v, f64::NAN), v);
            assert_eq!(unstretch_point(v, 0.0), v);
        }
        assert_eq!(unstretch_point(1, 1.5), 1);
        assert_eq!(unstretch_point(0, 1.5), 0);
        assert_eq!(unstretch_point(2, 1.5), 1);
        assert_eq!(unstretch_point(3, 1.5), 2);
        assert_eq!(unstretch_point(3, 2.0), 1);
        assert_eq!(unstretch_point(-1, 2.0), -1);
        assert_eq!(unstretch_point(i32::MAX, 0.5), i32::MAX);
        // Every pixel the stretched picture of unstretched column n covers maps back to n.
        for s in [1.5f64, 2.0, 2.5, 0.75] {
            for n in 0..20i32 {
                let (lo, hi) = (f64::from(n) * s, f64::from(n + 1) * s);
                for px in (lo.ceil() as i32 - 1)..(hi.ceil() as i32 + 1) {
                    let centre = f64::from(px) + 0.5;
                    if centre >= lo && centre < hi {
                        assert_eq!(unstretch_point(px, s), n, "σ {s}, pixel {px}");
                    }
                }
            }
        }
    }

    /// `ui-scaling.md` §7.5: the text factor reaches multi-line Text inside a Scrollable and
    /// marked elements only; plain labels and one-line text keep *x* = 1, and without a
    /// layout-parameter reader nothing takes it.
    #[test]
    fn text_factor_reaches_only_reflowing_text() {
        let mut ui = Ui::new(Box::new(NullEnv), Vocab::default(), (1024, 768));
        let window = ui.create_control(ControlKind::Window, 1).unwrap();
        let plain = ui.create_control(ControlKind::Label, 1).unwrap();
        ui.control_set_parent(plain, Some(window)).unwrap();
        let list = ui.create_control(ControlKind::ListBox, 1).unwrap();
        ui.control_set_parent(list, Some(window)).unwrap();
        let row = ui.create_control(ControlKind::Label, 1).unwrap();
        ui.control_set_parent(row, Some(list)).unwrap();
        ui.set_text_scale(1.25);
        // No reader: the factor needs a new layout, so nothing takes it.
        assert_eq!(ui.text_factor_for(row, Some(&params(false))), 1.0);
        ui.set_text_layout_params(Some(Box::new(|_, _, _| LayoutParams::default())));
        assert_eq!(ui.text_factor_for(row, Some(&params(false))), 1.25);
        assert_eq!(
            ui.text_factor_for(row, Some(&params(true))),
            1.0,
            "one line"
        );
        assert_eq!(
            ui.text_factor_for(plain, Some(&params(false))),
            1.0,
            "plain"
        );
        ui.set_text_reflow(plain, true);
        assert_eq!(
            ui.text_factor_for(plain, Some(&params(true))),
            1.25,
            "marked"
        );
        ui.set_text_scale(1.0);
        assert_eq!(ui.text_factor_for(row, Some(&params(false))), 1.0);
    }

    /// §7.5: the plain tooltip's text takes the factor (it is sized from its scaled text);
    /// the text of tooltip panels built by game code does not (they size their rows from
    /// unscaled extents, so the text would be clipped).
    #[test]
    fn text_factor_skips_game_built_tooltip_panels() {
        let mut ui = Ui::new(Box::new(NullEnv), Vocab::default(), (1024, 768));
        ui.set_text_layout_params(Some(Box::new(|_, _, _| LayoutParams::default())));
        ui.set_text_scale(1.5);
        let main = ui.create_control(ControlKind::Window, 1).unwrap();
        let row = ui.create_control(ControlKind::Label, 1).unwrap();
        ui.control_set_parent(row, Some(main)).unwrap();
        ui.set_tooltip_panels(main, Vec::new());
        assert_eq!(ui.tooltip_panel(), Some(main));
        assert_eq!(ui.text_factor_for(row, Some(&params(false))), 1.0);
        assert_eq!(ui.text_factor_for(main, Some(&params(false))), 1.0);
        // The plain tooltip's text element.
        ui.tooltip.text = Some(row);
        assert_eq!(ui.text_factor_for(row, Some(&params(true))), 1.5);
        ui.hide_tooltip();
        assert_eq!(ui.tooltip.text, None);
        assert_eq!(ui.text_factor_for(row, Some(&params(true))), 1.0);
    }

    /// §6.4.3 item 4: Tessera's font is rasterised once per size in use; a window cycling
    /// through more sizes than the old 16-face cache held no longer rasterises them again.
    #[test]
    fn tessera_faces_are_rasterised_once_per_size() {
        let mut t = TextScaling::default();
        for px in 6..=23 {
            assert!(t.tessera_face(px).is_some());
        }
        assert_eq!(t.tessera_face_counts(), (18, 18));
        for _ in 0..3 {
            for px in 6..=23 {
                let _ = t.tessera_face(px);
            }
        }
        assert_eq!(t.tessera_face_counts(), (18, 18), "every lookup is a hit");
    }

    /// `ui-scaling.md` §8 S6 (revision 11 answer): Tessera's faces take at most half of the
    /// glyph share and the derived-glyph cache the rest; past the half the least recently used
    /// face goes, and the face in use is kept even when it alone exceeds its half (the
    /// derived-glyph part is then empty).
    #[test]
    fn tessera_faces_take_at_most_half_the_glyph_share() {
        let mut t = TextScaling::default();
        let share = crate::text::DEFAULT_GLYPH_CACHE_BYTES;
        for px in 30..=64 {
            assert!(t.tessera_face(px).is_some());
            let (faces, bytes) = (t.tessera_face_counts().0, t.tessera_bytes());
            assert!(
                bytes <= share / 2 || faces == 1,
                "{px}: {bytes} bytes in {faces} faces"
            );
            assert_eq!(
                t.cache.budget(),
                share - bytes,
                "{px}: the rest is the glyphs'"
            );
        }
        assert!(t.tessera_face_counts().0 < 35, "some faces were dropped");
        // A share whose half no face fits in: only the face in use stays.
        t.set_glyph_share(64 << 10);
        assert_eq!(t.tessera_face_counts().0, 1, "the face in use is kept");
        assert!(t.tessera_bytes() > 32 << 10);
        assert_eq!(t.cache.budget(), 0, "nothing left for derived glyphs");
        assert!(t.tessera_face(20).is_some());
        assert_eq!(
            t.tessera_face_counts().0,
            1,
            "the new face in use replaces it"
        );
        // Back to the default share: the sizes one screen uses at once stay rasterised.
        t.set_glyph_share(share);
        let before = t.tessera_face_counts().1;
        for _ in 0..4 {
            for px in 11..=14 {
                let _ = t.tessera_face(px);
            }
        }
        assert_eq!(
            t.tessera_face_counts().1 - before,
            4,
            "11 to 14 px fit in the half: rasterised once each"
        );
        assert!(t.tessera_bytes() <= share / 2);
    }

    /// §6.4.3 item 4 and §6.4.4: a target height above the rasteriser's bound is derived from
    /// the tallest rasterised size by the distance method, so the text keeps its size.
    #[test]
    fn tessera_source_above_the_rasteriser_bound_keeps_its_size() {
        let face = data_face(28);
        let lh = face.line_height_i32();
        let fonts = vec![Some(face)];
        let mut b = CharListBuilder::new();
        b.colour(0xFFFF_FFFF).push_str("Hg Hg");
        let chars = b.finish();
        let p = LayoutParams {
            width: 100_000,
            height: 10_000,
            ..LayoutParams::default()
        };
        let mut t = TextScaling {
            source: UiTextSource::Tessera,
            ..TextScaling::default()
        };
        for c in [2.0, 3.0, 4.0] {
            let want = target_height(lh, c, 1.0);
            let rf = t.resolve_fonts(&fonts, c, 1.0);
            assert!(rf.replaced, "c {c}");
            let l = layout_scaled(&chars, &rf.fonts, &p, None, &rf.ctx(c, 1.0)).expect("layout");
            let got = l.lines[0].height;
            if want <= 64 {
                // Rasterised at the target height: that face's own line height.
                assert!((got - want).abs() <= 3, "c {c}: {got} for {want}");
            } else {
                assert_eq!(got, want, "c {c}: derived at the target height");
            }
            assert!(got > 50, "c {c}: {got}");
        }
    }

    fn label_scene(screen: Option<f64>) -> (Ui, crate::tree::ElementKey) {
        const TEXT: &str = "Scroll me";
        let fonts = vec![Some(data_face(12))];
        let mut b = CharListBuilder::new();
        b.colour(0xFFFF_FFFF).push_str(TEXT);
        let chars = b.finish();
        let p = LayoutParams {
            width: 200,
            height: 40,
            ..LayoutParams::default()
        };
        let l = layout(&chars, &fonts, &p, None).expect("layout");
        let extent = (l.content_width, l.content_height);
        let mut env = MemoryEnv::default();
        env.texts.insert(
            TEXT.to_string(),
            Arc::new(PreparedText {
                layout: l,
                chars,
                fonts,
                outline: false,
                outline_colour: 0xFF00_0000,
                truncated: false,
                extent,
            }),
        );
        let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
        let window = ui.create_control(ControlKind::Window, 1).unwrap();
        ui.control_set_position(window, 20, 20);
        ui.control_set_size(window, 240, 80);
        ui.control_set_visible(window, true);
        let label = ui.create_control(ControlKind::Label, 1).unwrap();
        ui.control_set_parent(label, Some(window)).unwrap();
        ui.control_set_position(label, 10, 10);
        ui.control_set_size(label, 200, 40);
        ui.control_set_visible(label, true);
        ui.set_text(label, Some(TEXT.to_string()));
        if let Some(c) = screen {
            let f =
                UiScreen::new((1024.0 * c).round() as i32, (768.0 * c).round() as i32, c).unwrap();
            ui.set_ui_screen(f, safe_area(&f, None));
        }
        (ui, label)
    }

    fn settle(ui: &mut Ui) {
        let mut budget = Unlimited;
        for t in 0..8u64 {
            ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
            if ui.paint_settled() {
                break;
            }
        }
    }

    /// A move of a text element inside its surface (origin only) draws the cached physical
    /// layout again; it is not laid out again.
    #[test]
    fn a_move_reuses_the_physical_layout() {
        let (mut ui, label) = label_scene(Some(1.5));
        settle(&mut ui);
        let r = ui.painter.text.runs.get(&label).expect("runs");
        let (layout, origin) = (Arc::clone(&r.layout), r.draw_key.origin);
        ui.control_set_position(label, 14, 12);
        settle(&mut ui);
        let r = ui.painter.text.runs.get(&label).expect("runs");
        assert_ne!(r.draw_key.origin, origin, "drawn at the new place");
        assert!(Arc::ptr_eq(&r.layout, &layout), "the layout was reused");
    }

    /// §6.4.6: before an element is drawn under a UI screen the hit test has no physical
    /// layout and answers nothing (not a character at the wrong scale); after an option
    /// change the last drawn layout keeps answering until the next paint.
    #[test]
    fn hit_test_without_a_physical_layout() {
        let (mut ui, label) = label_scene(Some(1.5));
        let b = ui.physical_box(label);
        let y = b.y0 + 6;
        assert_eq!(ui.text_hit_test(label, b.x0 + 1, y), None, "not drawn yet");
        settle(&mut ui);
        assert_eq!(ui.text_hit_test(label, b.x0 + 1, y), Some(0));
        ui.set_text_source(UiTextSource::Tessera);
        assert_eq!(
            ui.text_hit_test(label, b.x0 + 1, y),
            Some(0),
            "kept until repainted"
        );
        // Without a UI screen the shaper's layout is in physical pixels and answers.
        let (mut ui, label) = label_scene(None);
        settle(&mut ui);
        assert!(!ui.painter.text.runs.contains_key(&label), "identity path");
        let b = ui.physical_box(label);
        assert_eq!(ui.text_hit_test(label, b.x0 + 1, b.y0 + 4), Some(0));
    }
}
