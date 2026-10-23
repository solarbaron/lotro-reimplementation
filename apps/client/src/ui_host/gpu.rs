// SPDX-License-Identifier: GPL-3.0-or-later
//! The GPU side of the game UI (decision 0015 §1, `render.md` §6): `tessera-ui`'s
//! [`GpuOp`]s applied to `tessera-render`'s UI textures in order, and its
//! [`CompositionList`] turned into the entries of the frame's UI pass. Untextured fill
//! entries (the letterbox bars of `ui-scaling.md` §5.4 step 6) become the UI pass's
//! [`UiFill`]s in their place, so the host draws exactly the bars `Ui::letterbox_bars`
//! reports, below the picture of each group's lowest shown member.
//!
//! A UI texture covers a tile box of its canvas (`render.md` §6.2), so canvas coordinates
//! are shifted by the box's top-left corner before every write. A texture the renderer
//! refuses (device limit, allocation failure) is left out: its writes are skipped and its
//! entries not drawn; each refusal is counted and logged once per kind.
//!
//! External content (the portraits of `ui-widgets.md` §12, `render.md` §6.10) draws from
//! the textures registered in the [`ExternalTextures`] table, by the content id the UI
//! composes. The table is shared ([`UiTextures::external_textures`]), so `crate::portrait`
//! registers its targets without holding the host (the character panel and the character
//! screens register theirs through [`UiTextures::set_external`]). A texture registered with
//! its size is sampled whole: the entry's canvas is the target's size, so a 62 × 62 portrait
//! target fills its element's displayed box at any UI scale, and when the target's size
//! differs from the displayed box it is sampled with linear filtering, whatever the list's
//! filter and the `ui_scale_filter` override say (`ui-widgets.md` §12.3 step 3).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tessera_render::{
    Frame, Renderer, TextureId, UiAddressing, UiCapCounters, UiClip, UiEntry, UiFill, UiFilter,
    UiRect, UiRotation, UiSource,
};
use tessera_ui::paint::{
    Addressing, CapCounters, CompositionList, EntryTexture, Filter, GpuOp, Rect, Slot, Source,
    TexKey,
};

/// Counters of [`UiTextures`] since its creation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuOpStats {
    /// Textures created.
    pub created: u64,
    /// Textures the renderer refused to create.
    pub create_failures: u64,
    /// Writes (uploads, copies, clears) the renderer refused or that named no texture.
    pub write_failures: u64,
    /// Entries left out because their texture does not exist.
    pub missing_textures: u64,
}

/// One external-content texture ([`ExternalTextures`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalTexture {
    /// The registered render target.
    pub texture: TextureId,
    /// The target's size in pixels: the entry samples the whole target into its displayed
    /// box. `None` keeps the composition list's canvas (a target of the layout-box size).
    pub size: Option<(u32, u32)>,
}

/// The external-content textures by content id, shared between [`UiTextures`] and the code
/// that renders the targets (module docs). Cloning shares the table.
#[derive(Debug, Clone, Default)]
pub struct ExternalTextures(Rc<RefCell<BTreeMap<u64, ExternalTexture>>>);

impl ExternalTextures {
    /// Registers the texture content `id` draws from, or removes it (`None`: its entries
    /// are then not drawn).
    pub fn set(&self, id: u64, texture: Option<ExternalTexture>) {
        let mut m = self.0.borrow_mut();
        match texture {
            Some(t) => {
                m.insert(id, t);
            }
            None => {
                m.remove(&id);
            }
        }
    }

    /// The texture of content `id`.
    #[must_use]
    pub fn get(&self, id: u64) -> Option<ExternalTexture> {
        self.0.borrow().get(&id).copied()
    }

    /// Registered contents.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

#[derive(Debug, Clone, Copy)]
struct Tex {
    id: TextureId,
    tile_box: Rect,
}

/// The UI textures of one renderer, by `tessera-ui` texture key.
#[derive(Debug, Default)]
pub struct UiTextures {
    map: BTreeMap<TexKey, Tex>,
    stats: GpuOpStats,
    warned: [bool; 4],
    filter_override: Option<Filter>,
    /// Textures of external content (portrait viewports, `render.md` §6.10) by the
    /// content id the UI composes: render targets the client draws (module docs).
    external: ExternalTextures,
}

fn dims(r: &Rect) -> (u32, u32) {
    (r.width(), r.height())
}

impl UiTextures {
    /// No textures.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Samples every entry with `filter` instead of the composition list's own filter
    /// (`[UI] ui_scale_filter`, decision 0015 §2.2); `None` keeps the list's filter.
    pub fn set_filter_override(&mut self, filter: Option<Filter>) {
        self.filter_override = filter;
    }

    /// The filter override in use.
    #[must_use]
    pub fn filter_override(&self) -> Option<Filter> {
        self.filter_override
    }

    /// The filter an entry is drawn with.
    #[must_use]
    pub fn entry_filter(&self, list_filter: Filter) -> UiFilter {
        match self.filter_override.unwrap_or(list_filter) {
            Filter::Linear => UiFilter::Linear,
            Filter::Nearest => UiFilter::Nearest,
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> GpuOpStats {
        self.stats
    }

    /// Live textures.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no texture is live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The renderer texture of a key (tests and diagnostics).
    #[must_use]
    pub fn texture(&self, key: &TexKey) -> Option<TextureId> {
        self.map.get(key).map(|t| t.id)
    }

    fn warn_once(&mut self, kind: usize, msg: &str) {
        if let Some(w) = self.warned.get_mut(kind)
            && !*w
        {
            *w = true;
            tracing::warn!("UI textures: {msg} (further ones counted only)");
        }
    }

    fn write_failed(&mut self, what: &str) {
        self.stats.write_failures = self.stats.write_failures.saturating_add(1);
        self.warn_once(1, what);
    }

    /// Applies `ops` in order (`render.md` §6.3).
    pub fn apply(&mut self, renderer: &mut Renderer<'_>, ops: &[GpuOp]) {
        for op in ops {
            match op {
                GpuOp::Create { tex, tile_box } => self.create(renderer, *tex, *tile_box),
                GpuOp::Copy { from, to } => self.copy(renderer, from, to),
                GpuOp::Upload { tex, rect, bytes } => self.upload(renderer, tex, rect, bytes),
                GpuOp::Clear { tex, rect } => self.clear(renderer, tex, rect),
                GpuOp::Promote { surface, layer } => {
                    let back = TexKey {
                        surface: *surface,
                        layer: *layer,
                        slot: Slot::Back,
                    };
                    let front = TexKey {
                        slot: Slot::Front,
                        ..back
                    };
                    if let Some(old) = self.map.remove(&front) {
                        renderer.destroy_texture(old.id);
                    }
                    if let Some(t) = self.map.remove(&back) {
                        self.map.insert(front, t);
                    }
                }
                GpuOp::Destroy { tex } => {
                    if let Some(t) = self.map.remove(tex) {
                        renderer.destroy_texture(t.id);
                    }
                }
            }
        }
    }

    fn create(&mut self, renderer: &mut Renderer<'_>, tex: TexKey, tile_box: Rect) {
        if let Some(old) = self.map.remove(&tex) {
            renderer.destroy_texture(old.id);
        }
        let (w, h) = dims(&tile_box);
        match renderer.create_ui_texture(w, h, Some("ui-surface")) {
            Ok(id) => {
                self.stats.created = self.stats.created.saturating_add(1);
                self.map.insert(tex, Tex { id, tile_box });
            }
            Err(e) => {
                self.stats.create_failures = self.stats.create_failures.saturating_add(1);
                self.warn_once(0, &format!("a {w}x{h} texture was refused: {e}"));
            }
        }
    }

    fn copy(&mut self, renderer: &mut Renderer<'_>, from: &TexKey, to: &TexKey) {
        let (Some(s), Some(d)) = (self.map.get(from).copied(), self.map.get(to).copied()) else {
            self.write_failed("a copy names a missing texture");
            return;
        };
        let o = s.tile_box.intersect(&d.tile_box);
        let (w, h) = dims(&o);
        if w == 0 || h == 0 {
            return;
        }
        let (Some(sx), Some(sy), Some(dx), Some(dy)) = (
            offset(o.x0, s.tile_box.x0),
            offset(o.y0, s.tile_box.y0),
            offset(o.x0, d.tile_box.x0),
            offset(o.y0, d.tile_box.y0),
        ) else {
            self.write_failed("a copy outside its texture");
            return;
        };
        if renderer
            .copy_texture_rect(s.id, sx, sy, d.id, dx, dy, w, h)
            .is_err()
        {
            self.write_failed("a copy was refused");
        }
    }

    fn upload(&mut self, renderer: &mut Renderer<'_>, tex: &TexKey, rect: &Rect, bytes: &[u8]) {
        let Some(t) = self.map.get(tex).copied() else {
            self.write_failed("an upload names a missing texture");
            return;
        };
        let (w, h) = dims(rect);
        let (Some(x), Some(y)) = (
            offset(rect.x0, t.tile_box.x0),
            offset(rect.y0, t.tile_box.y0),
        ) else {
            self.write_failed("an upload outside its texture");
            return;
        };
        let stride = (w as usize).saturating_mul(4);
        if renderer
            .update_texture_rect(t.id, x, y, w, h, bytes, stride)
            .is_err()
        {
            self.write_failed("an upload was refused");
        }
    }

    fn clear(&mut self, renderer: &mut Renderer<'_>, tex: &TexKey, rect: &Rect) {
        let Some(t) = self.map.get(tex).copied() else {
            self.write_failed("a clear names a missing texture");
            return;
        };
        let (w, h) = dims(rect);
        let (Some(x), Some(y)) = (
            offset(rect.x0, t.tile_box.x0),
            offset(rect.y0, t.tile_box.y0),
        ) else {
            self.write_failed("a clear outside its texture");
            return;
        };
        if renderer.clear_texture_rect(t.id, x, y, w, h).is_err() {
            self.write_failed("a clear was refused");
        }
    }

    /// Releases every texture (UI teardown). External-content textures belong to their
    /// owners ([`Self::set_external`]) and are only forgotten.
    pub fn release(&mut self, renderer: &mut Renderer<'_>) {
        for (_, t) in std::mem::take(&mut self.map) {
            renderer.destroy_texture(t.id);
        }
        self.external.clear();
    }

    /// The texture that external content `id` draws from (a registered render target of
    /// the layout-box size), or none: its entries are then not drawn.
    pub fn set_external(&mut self, id: u64, texture: Option<TextureId>) {
        self.external.set(
            id,
            texture.map(|texture| ExternalTexture {
                texture,
                size: None,
            }),
        );
    }

    /// The shared table of external-content textures (module docs).
    #[must_use]
    pub fn external_textures(&self) -> ExternalTextures {
        self.external.clone()
    }

    /// Records the composition list as the frame's UI pass (`render.md` §6.5, §6.9). An
    /// entry of external content draws from the texture [`Self::set_external`] gave its id
    /// (the paperdoll's portrait, decision 0024 P1), and is left out without one. A fill
    /// entry ([`EntryTexture::Fill`], the letterbox bars) is recorded as a [`UiFill`] of its
    /// colour in its place. Returns the number of entries recorded.
    pub fn record(
        &mut self,
        frame: &mut Frame,
        list: &CompositionList,
        caps: &CapCounters,
        entry_cap: u32,
    ) -> usize {
        let pass = frame.ui_pass();
        pass.cap_counters(cap_counters(caps, entry_cap));
        let mut n = 0;
        let mut rotations: Vec<UiRotation> = Vec::new();
        let mut clips: Vec<UiClip> = Vec::new();
        for e in &list.entries {
            let mut external_size: Option<(u32, u32)> = None;
            let texture = match e.texture {
                EntryTexture::Surface { surface, layer } => {
                    let key = TexKey {
                        surface,
                        layer,
                        slot: Slot::Front,
                    };
                    let Some(t) = self.map.get(&key) else {
                        self.stats.missing_textures = self.stats.missing_textures.saturating_add(1);
                        continue;
                    };
                    t.id
                }
                // A portrait the client rendered (`render.md` §6.10).
                EntryTexture::External(id) => match self.external.get(id) {
                    Some(t) => {
                        external_size = t.size;
                        t.texture
                    }
                    None => continue,
                },
                // `ui-scaling.md` §5.4 step 6: an untextured fill (screen space, unclipped).
                EntryTexture::Fill { colour } => {
                    pass.fill(&UiFill::new(ui_rect(&e.dest), fill_colour(colour)));
                    n += 1;
                    continue;
                }
            };
            rotations.clear();
            rotations.extend(list.rotations_of(e).iter().map(|r| UiRotation {
                centre: r.centre,
                angles: r.angles,
            }));
            clips.clear();
            clips.extend(list.clips_of(e).iter().map(|c| UiClip {
                rect: c.rect,
                space: c.space,
            }));
            // A sized external target is sampled whole, linearly when it is scaled
            // (module docs).
            let (canvas, tex_offset, filter) = match external_size {
                Some((w, h)) if w > 0 && h > 0 => (
                    [w, h],
                    [0, 0],
                    external_filter((w, h), &e.dest, self.entry_filter(e.filter)),
                ),
                _ => (e.canvas, e.tex_offset, self.entry_filter(e.filter)),
            };
            let entry = UiEntry::new(texture, canvas, ui_rect(&e.dest))
                .with_source(match e.source {
                    Source::Straight => UiSource::Straight,
                    Source::Premultiplied => UiSource::Premultiplied,
                })
                .with_tex_offset(tex_offset)
                .with_addressing(match e.addressing {
                    Addressing::Fit => UiAddressing::Fit,
                    Addressing::Tile { node_scale } => UiAddressing::Tile { node_scale },
                })
                .with_filter(filter)
                .with_opacity(e.opacity)
                .with_alpha_threshold(e.alpha_threshold)
                .with_rotations(&rotations)
                .with_clips(&clips);
            pass.entry(&entry);
            n += 1;
        }
        n
    }
}

/// The filter of a sized external target of `size` pixels drawn into `dest`: linear when
/// the sizes differ (`ui-widgets.md` §12.3 step 3), otherwise `listed` (the list's filter
/// with the scale-filter override; at 1 : 1 the filters agree).
#[must_use]
pub fn external_filter(size: (u32, u32), dest: &Rect, listed: UiFilter) -> UiFilter {
    if (dest.width(), dest.height()) == size {
        listed
    } else {
        UiFilter::Linear
    }
}

/// A straight-alpha 0xAARRGGBB colour as the renderer's straight RGBA in 0–1.
#[must_use]
pub fn fill_colour(c: u32) -> [f32; 4] {
    let ch = |shift: u32| f32::from(((c >> shift) & 0xFF) as u8) / 255.0;
    [ch(16), ch(8), ch(0), ch(24)]
}

/// `x − origin` as a texture coordinate, or `None` when negative.
fn offset(x: i32, origin: i32) -> Option<u32> {
    u32::try_from(i64::from(x) - i64::from(origin)).ok()
}

/// A paint rectangle as a render rectangle.
#[must_use]
pub fn ui_rect(r: &Rect) -> UiRect {
    UiRect::new(r.x0, r.y0, r.width(), r.height())
}

fn sat(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

/// The cap counters of decision 0015 §7 in the renderer's form.
#[must_use]
pub fn cap_counters(c: &CapCounters, entry_cap: u32) -> UiCapCounters {
    UiCapCounters {
        apartment_cap: sat(c.apartment_surfaces),
        total_cap: sat(c.total_surfaces),
        depth_cap: sat(c.depth),
        memory_cap: sat(c.memory),
        tile_fallbacks: 0,
        cropped_canvases: sat(c.cropped),
        entry_cap: entry_cap.saturating_add(sat(c.entries)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_filter_setting_overrides_the_list() {
        let mut t = UiTextures::new();
        assert_eq!(t.entry_filter(Filter::Linear), UiFilter::Linear);
        assert_eq!(t.entry_filter(Filter::Nearest), UiFilter::Nearest);
        t.set_filter_override(Some(Filter::Nearest));
        assert_eq!(t.entry_filter(Filter::Linear), UiFilter::Nearest);
        assert_eq!(t.filter_override(), Some(Filter::Nearest));
    }

    #[test]
    fn a_scaled_external_target_is_sampled_linearly() {
        let at_size = Rect::from_xywh(10, 10, 62, 62);
        let scaled = Rect::from_xywh(10, 10, 93, 93);
        assert_eq!(
            external_filter((62, 62), &at_size, UiFilter::Nearest),
            UiFilter::Nearest
        );
        assert_eq!(
            external_filter((62, 62), &scaled, UiFilter::Nearest),
            UiFilter::Linear
        );
        assert_eq!(
            external_filter((62, 62), &Rect::from_xywh(0, 0, 62, 61), UiFilter::Nearest),
            UiFilter::Linear
        );
    }

    #[test]
    fn offsets_never_go_negative() {
        assert_eq!(offset(5, 3), Some(2));
        assert_eq!(offset(3, 5), None);
        assert_eq!(offset(i32::MIN, i32::MAX), None);
        let r = Rect::from_xywh(-4, 7, 10, 3);
        assert_eq!(ui_rect(&r), UiRect::new(-4, 7, 10, 3));
    }

    #[test]
    fn fill_colours_are_straight_rgba() {
        assert_eq!(fill_colour(0xFF00_0000), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(fill_colour(0x80FF_0000), [1.0, 0.0, 0.0, 128.0 / 255.0]);
        assert_eq!(fill_colour(0x0000_FF00), [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn counters_saturate() {
        let c = CapCounters {
            total_surfaces: u64::MAX,
            entries: 3,
            ..CapCounters::default()
        };
        let u = cap_counters(&c, u32::MAX);
        assert_eq!(u.total_cap, u32::MAX);
        assert_eq!(u.entry_cap, u32::MAX);
    }
}
