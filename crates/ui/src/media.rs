// SPDX-License-Identifier: GPL-3.0-or-later
//! The media player (`ui-layout.md` §6, §9.5) and the base-image setter with its skin hook
//! (§9.8, `skin-definition.md` §6.3, §6.6, §6.8).

use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::MediaItem;

use crate::env::ImageRef;
use crate::events::{MsgArg, UiEvent};
use crate::skin::ImageChoice;
use crate::tree::{BaseImage, ElementKey, Overlay};
use crate::ui::Ui;

/// Durations at or below this count as zero (`ui-layout.md` §6.4, §6.6).
const TINY: f64 = 0.0002;

/// Nested media runs before a newly started list waits for the next frame (Tessera cap
/// against State-item loops that would otherwise run without end in one frame).
const MAX_MEDIA_NESTING: u32 = 8;

/// The draw mode that clears a layer (`ui-layout.md` §6.2, §6.3).
const DRAW_NONE: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Done,
    Busy,
    StateChanged,
}

impl Ui {
    /// Replaces the running media list and starts it from the first item (§9.4 step 6,
    /// §9.5 step 1). Instant items apply at once; a list started from inside a media run
    /// deeper than a small cap runs on the next frame.
    pub(crate) fn start_media(&mut self, k: ElementKey, list: Arc<[MediaItem]>) {
        let Some(e) = self.el_mut(k) else { return };
        e.media.list = list;
        e.media.pos = 0;
        e.media.item_start = None;
        e.media.last_frame = None;
        e.media.complete = false;
        if self.media_depth >= MAX_MEDIA_NESTING {
            self.want_frame(k);
            return;
        }
        self.run_media(k);
    }

    /// Runs the element's list from its position (§9.5 step 2).
    pub(crate) fn run_media(&mut self, k: ElementKey) {
        self.media_depth += 1;
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 4096 {
                self.want_frame(k);
                break;
            }
            let Some(e) = self.el(k) else { break };
            if e.media.complete {
                break;
            }
            let pos = e.media.pos;
            let Some(item) = e.media.list.get(pos).cloned() else {
                // Step 3: complete.
                if let Some(e) = self.el_mut(k) {
                    e.media.complete = true;
                }
                if self.prop(k, self.v.pids.notify_on_media_done).bool() {
                    self.post(k, self.v.msgs.media_done, MsgArg::default());
                }
                break;
            };
            let list_ptr = Arc::as_ptr(&e.media.list) as *const () as usize;
            match self.run_item(k, &item) {
                Step::Done => {
                    if let Some(e) = self.el_mut(k)
                        && Arc::as_ptr(&e.media.list) as *const () as usize == list_ptr
                        && e.media.pos == pos
                    {
                        e.media.pos += 1;
                        e.media.item_start = None;
                        e.media.last_frame = None;
                    }
                }
                Step::Busy => {
                    self.want_frame(k);
                    break;
                }
                Step::StateChanged => {
                    // §9.5 step 2: stop and update next frame at the same position. When
                    // the State item caused no switch (§9.4 step 2) this list is still the
                    // running one, and the item runs again next frame.
                    self.want_frame(k);
                    break;
                }
            }
        }
        self.media_depth -= 1;
    }

    fn created(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.created)
    }

    fn item_clock(&mut self, k: ElementKey) -> f64 {
        let now = self.now;
        match self.el_mut(k) {
            Some(e) => *e.media.item_start.get_or_insert(now),
            None => now,
        }
    }

    fn run_item(&mut self, k: ElementKey, item: &MediaItem) -> Step {
        match item {
            MediaItem::Image {
                did,
                draw_mode,
                path,
            } => {
                if draw_mode.0 == DRAW_NONE {
                    self.clear_base(k);
                } else if did.0 == 0 && !path.is_empty() {
                    self.set_base(k, ImageRef::File(path.clone()), 0, false, draw_mode.0);
                } else if did.0 == 0 {
                    self.clear_base(k);
                } else {
                    self.set_base_did(k, did.0, draw_mode.0, true);
                }
                Step::Done
            }
            MediaItem::ColouredImage {
                did,
                draw_mode,
                colour,
            } => {
                let ov = if draw_mode.0 == DRAW_NONE {
                    None
                } else {
                    Some(Overlay {
                        image: (did.0 != 0).then_some(ImageRef::Did(did.0)),
                        colour: colour.0,
                        draw_mode: draw_mode.0,
                    })
                };
                if let Some(e) = self.el_mut(k)
                    && e.overlay != ov
                {
                    e.overlay = ov;
                    e.dirty = true;
                    self.painter.touched = true;
                }
                Step::Done
            }
            MediaItem::Animation {
                duration,
                draw_mode,
                looping,
                frames,
            } => {
                let start = self.item_clock(k);
                let d = f64::from(*duration);
                let t = if d.abs() <= TINY {
                    0.0
                } else {
                    ((self.now - start) / d).clamp(0.0, 1.0)
                };
                if !frames.is_empty() {
                    let n = frames.len() as f64;
                    let idx = ((n - 1.0) * t + 0.5).floor().clamp(0.0, n - 1.0) as usize;
                    let last = self.el(k).and_then(|e| e.media.last_frame);
                    if last != Some(idx) {
                        if let Some(e) = self.el_mut(k) {
                            e.media.last_frame = Some(idx);
                        }
                        self.set_base_did(k, frames[idx].0, draw_mode.0, true);
                    }
                }
                if t < 1.0 {
                    Step::Busy
                } else {
                    if let Some(e) = self.el_mut(k) {
                        e.media.item_start = None;
                        if *looping {
                            e.media.last_frame = None;
                        }
                    }
                    if *looping { Step::Busy } else { Step::Done }
                }
            }
            MediaItem::Cursor {
                did,
                hotspot_x,
                hotspot_y,
            } => {
                let c = (did.0 != 0).then_some((did.0, *hotspot_x, *hotspot_y));
                if let Some(e) = self.el_mut(k) {
                    e.cursor = c;
                }
                self.input.cursor_dirty = true;
                Step::Done
            }
            MediaItem::Sound { did } => {
                self.emit(UiEvent::Sound { did: did.0 });
                Step::Done
            }
            MediaItem::HitMask { did } => {
                if let Some(e) = self.el_mut(k) {
                    e.hit_mask = did.0;
                }
                Step::Done
            }
            MediaItem::State { state, probability } => {
                if !self.created(k) {
                    return Step::Busy;
                }
                if self.env.random() <= *probability {
                    self.switch_state(k, *state);
                    Step::StateChanged
                } else {
                    Step::Done
                }
            }
            MediaItem::Message {
                message,
                probability,
            } => {
                if !self.created(k) {
                    return Step::Busy;
                }
                if self.env.random() <= *probability {
                    self.post(k, *message, MsgArg::default());
                }
                Step::Done
            }
            MediaItem::Jump {
                target,
                probability,
            } => {
                if !self.created(k) {
                    return Step::Busy;
                }
                if self.env.random() <= *probability {
                    // OPEN (§6.8): the exact index arithmetic. Tessera: continue at `target`.
                    if let Some(e) = self.el_mut(k) {
                        e.media.pos = (*target as usize).min(e.media.list.len());
                        e.media.item_start = None;
                    }
                    return Step::Busy;
                }
                Step::Done
            }
            MediaItem::Fade {
                start,
                end,
                duration,
            } => {
                let pid = self.v.pids.object_opacity;
                let d = f64::from(*duration);
                if self.el(k).is_some_and(|e| e.media.item_start.is_none()) {
                    // Step 1: resume part-way when the opacity is already beyond `start`.
                    let c = self.prop(k, pid).float();
                    let (s, en) = (*start, *end);
                    let resume = (s > en && c < s) || (s < en && c > s);
                    let frac = if resume && (en - s).abs() > 0.0 {
                        (f64::from((c - s).abs()) / f64::from((en - s).abs())).min(1.0)
                    } else {
                        0.0
                    };
                    let now = self.now;
                    if let Some(e) = self.el_mut(k) {
                        e.media.item_start = Some(now - frac * d);
                    }
                }
                let st = self
                    .el(k)
                    .and_then(|e| e.media.item_start)
                    .unwrap_or(self.now);
                let t = if d.abs() <= TINY {
                    1.0
                } else {
                    ((self.now - st) / d).clamp(0.0, 1.0)
                };
                let o = *start + (*end - *start) * t as f32;
                self.set_property(k, pid, PropertyValue::Float(o));
                if t < 1.0 {
                    Step::Busy
                } else {
                    if let Some(e) = self.el_mut(k) {
                        e.media.item_start = None;
                    }
                    Step::Done
                }
            }
            MediaItem::Pause { min, max } => {
                if self.el(k).is_some_and(|e| e.media.item_start.is_none()) {
                    let r = f64::from(self.env.random().clamp(0.0, 1.0));
                    let (a, b) = (f64::from(*min), f64::from(*max));
                    let dur = a + (b - a) * r;
                    let now = self.now;
                    if let Some(e) = self.el_mut(k) {
                        e.media.item_start = Some(now);
                        e.media.pause = if dur.is_finite() { dur } else { 0.0 };
                    }
                }
                let (st, dur) = self
                    .el(k)
                    .map(|e| (e.media.item_start.unwrap_or(0.0), e.media.pause))
                    .unwrap_or((0.0, 0.0));
                if self.now - st < dur {
                    Step::Busy
                } else {
                    if let Some(e) = self.el_mut(k) {
                        e.media.item_start = None;
                    }
                    Step::Done
                }
            }
            MediaItem::Movie { did, path, .. } => {
                if !self.created(k) {
                    return Step::Busy;
                }
                // Tessera M5 plays no video (decision 0015 §1.7: Movie stays empty).
                self.emit(UiEvent::Movie {
                    element: k,
                    did: did.0,
                    path: path.clone(),
                });
                Step::Done
            }
        }
    }

    /// The base-image setter (`skin-definition.md` §6.8): sets the base image from a DID
    /// with a draw mode, letting the active user skin substitute the surface when
    /// `skinnable`.
    pub(crate) fn set_base_did(
        &mut self,
        k: ElementKey,
        did: u32,
        draw_mode: u32,
        skinnable: bool,
    ) {
        let image = self.skinned(did, skinnable);
        self.set_base(k, image, did, true, draw_mode);
    }

    /// What an image requested by DID resolves to under the active skin.
    pub(crate) fn skinned(&self, did: u32, skinnable: bool) -> ImageRef {
        match (&self.skin, skinnable) {
            (Some(s), true) => match s.resolve_image(did) {
                ImageChoice::Base(d) => ImageRef::Did(d),
                ImageChoice::GameSkin { did, .. } => ImageRef::Did(did),
                ImageChoice::SkinFile(r) => ImageRef::Skin {
                    art_asset: r.art_asset,
                    did,
                },
            },
            _ => ImageRef::Did(did),
        }
    }

    pub(crate) fn set_base(
        &mut self,
        k: ElementKey,
        image: ImageRef,
        did: u32,
        refreshable: bool,
        draw_mode: u32,
    ) {
        let b = Some(BaseImage {
            image,
            did,
            refreshable,
            draw_mode,
        });
        if let Some(e) = self.el_mut(k)
            && e.base != b
        {
            e.base = b;
            e.dirty = true;
            self.painter.touched = true;
        }
    }

    pub(crate) fn clear_base(&mut self, k: ElementKey) {
        if let Some(e) = self.el_mut(k)
            && e.base.is_some()
        {
            e.base = None;
            e.dirty = true;
            self.painter.touched = true;
        }
    }

    /// Activates a user skin overlay (or none) and runs the refresh of
    /// `skin-definition.md` §6.6: every base image made from a DID is set again from that
    /// original DID; file images, overlays, text, states and media are untouched.
    pub fn set_skin(&mut self, skin: Option<Arc<crate::skin::SkinOverlay>>) {
        self.skin = skin;
        for k in self.arena.keys() {
            let Some(b) = self.el(k).and_then(|e| e.base.clone()) else {
                continue;
            };
            if !b.refreshable || b.did == 0 {
                continue;
            }
            let image = self.skinned(b.did, true);
            self.set_base(k, image, b.did, true, b.draw_mode);
            self.mark_dirty(k);
        }
    }

    /// Sets the run-time image offset of an element (`ui-layout.md` §9.6).
    pub fn set_image_offset(&mut self, k: ElementKey, x: i32, y: i32) {
        if let Some(e) = self.el_mut(k)
            && e.image_offset != (x, y)
        {
            e.image_offset = (x, y);
            e.dirty = true;
            self.painter.touched = true;
        }
    }

    /// Gives an element a media list and starts it (how game code installs images, for
    /// example a meter's child images, `skin-definition.md` §6.8).
    pub fn play_media(&mut self, k: ElementKey, items: Vec<MediaItem>) {
        self.start_media(k, items.into());
    }
}
