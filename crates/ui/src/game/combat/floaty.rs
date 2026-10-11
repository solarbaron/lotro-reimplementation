// SPDX-License-Identifier: GPL-3.0-or-later
//! The floating-text layer E13 (`combat-ui.md` §6, §13.6; TR19–TR24): per-entity lists of
//! active and pending texts, their life, growth, rise, fade, critical placement and
//! crowding, placed each frame from screen-space anchors the client gives
//! ([`super::data::AnchorSource`]), within bounded counts (CUI3, CUI4).

use std::collections::{BTreeMap, VecDeque};

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::data::{AnchorSource, EntityId, ScreenAnchor, cap_label, round_half_away};
use super::look::{self, TextStyle};
use super::vocab::CombatVocab;

/// The release spacing of §6.5 (a pending text is released once the newest active one has
/// risen more than (1 − 0.35) of its height).
pub const RELEASE_SPACING: f32 = 0.35;

/// A new text is refused while more than this many texts of the entity are pending (§6.5).
pub const MAX_PENDING: usize = 50;

/// Critical slots per entity (§6.3).
pub const CRIT_SLOTS: usize = 6;

/// The slot offsets (*a*, *b*) of §6.3.
pub const CRIT_SLOT_OFFSETS: [(f32, f32); CRIT_SLOTS] = [
    (0.0, 0.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-1.0, -1.0),
    (1.0, -1.0),
    (0.0, -1.0),
];

/// A text is hidden at *t* ≥ 1 and removed at *t* ≥ this (§6.3, CUI4).
pub const REMOVE_AT: f32 = 1.05;

/// Most entities with texts (Tessera cap; the oldest list is dropped).
pub const MAX_ENTITIES: usize = 512;

/// Most active texts per entity (Tessera cap beyond the release rule).
pub const MAX_ACTIVE: usize = 64;

/// Most pooled elements kept for reuse (Tessera cap).
pub const MAX_POOL: usize = 128;

/// The time constant of the rise-speed smoothing, seconds (§6.5: not read; Tessera's
/// choice).
pub const RISE_SMOOTHING: f64 = 0.25;

/// Life of the generic texts and of TR20's, seconds.
pub const DEFAULT_LIFE: f32 = 2.0;

/// Scale *s*₁ of the generic texts and of TR20's.
pub const DEFAULT_SCALE: f32 = 1.5;

/// Damage on another entity (TR20: the `FloatyDamageFont` colour).
pub const COLOUR_DAMAGE: u32 = 0xFFF7_E4A9;
/// Damage on the local player (TR20).
pub const COLOUR_DAMAGE_TAKEN: u32 = 0xFFFF_4040;
/// Heals (TR20).
pub const COLOUR_HEAL: u32 = 0xFF40_FF40;
/// Avoidance words (TR20).
pub const COLOUR_AVOID: u32 = 0xFFFF_FFFF;
/// The critical state's colour (§6.3, `Floaty_CriticalDamage`).
pub const COLOUR_CRITICAL: u32 = 0xFFFF_0000;
/// The critical colour of the fallback look (§13.6).
pub const COLOUR_CRITICAL_FALLBACK: u32 = 0xFFFF_2020;

/// Pixel size of the fallback font (§13.6).
pub const FALLBACK_PX: i32 = 18;
/// Design size of a fallback text element (Tessera: wide enough for a centred line).
pub const FALLBACK_SIZE: (i32, i32) = (256, 22);

/// The floating-text settings (§6.2), defaulting to the column "0x78000004" (which §13.6
/// takes for the fallback look).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatySettings {
    /// `UI_FloatyText_NearRange` (m).
    pub near_range: f32,
    /// `UI_FloatyText_FarRange` (m).
    pub far_range: f32,
    /// `UI_FloatyText_NearRangeScale`.
    pub near_scale: f32,
    /// `UI_FloatyText_FarRangeScale`.
    pub far_scale: f32,
    /// `UI_FloatyText_CriticalDamage_SpeedMod`.
    pub crit_speed_mod: f32,
    /// `UI_FloatyText_CriticalDamage_InitialScale`.
    pub crit_initial_scale: f32,
    /// `UI_FloatyText_CriticalDamage_ShrinkTime`.
    pub crit_shrink_time: f32,
    /// `UI_FloatyText_CriticalDamage_HorizontalOffset`.
    pub crit_h_offset: f32,
    /// `UI_FloatyText_CriticalDamage_VerticalOffset`.
    pub crit_v_offset: f32,
}

impl Default for FloatySettings {
    fn default() -> Self {
        Self {
            near_range: 4.0,
            far_range: 30.0,
            near_scale: 1.2,
            far_scale: 0.5,
            crit_speed_mod: 0.0,
            crit_initial_scale: 1.7,
            crit_shrink_time: 0.06,
            crit_h_offset: 60.0,
            crit_v_offset: 50.0,
        }
    }
}

impl FloatySettings {
    /// The settings with every non-finite value replaced by its default (§6.2 accepts any
    /// stored float; NaN or infinite ranges and scales would make the placement
    /// undefined).
    #[must_use]
    pub fn sanitized(self) -> Self {
        let d = Self::default();
        let f = |v: f32, def: f32| if v.is_finite() { v } else { def };
        Self {
            near_range: f(self.near_range, d.near_range),
            far_range: f(self.far_range, d.far_range),
            near_scale: f(self.near_scale, d.near_scale),
            far_scale: f(self.far_scale, d.far_scale),
            crit_speed_mod: f(self.crit_speed_mod, d.crit_speed_mod),
            crit_initial_scale: f(self.crit_initial_scale, d.crit_initial_scale),
            crit_shrink_time: f(self.crit_shrink_time, d.crit_shrink_time),
            crit_h_offset: f(self.crit_h_offset, d.crit_h_offset),
            crit_v_offset: f(self.crit_v_offset, d.crit_v_offset),
        }
    }
}

/// The distance scale *σ* of §6.4 for distance `d` with near and far ranges and scales.
/// Always finite: non-finite inputs give the far scale (or 1 when that is not finite).
#[must_use]
pub fn distance_scale(d: f32, near: f32, far: f32, near_scale: f32, far_scale: f32) -> f32 {
    let span = far - near;
    let u = if span.abs() <= f32::EPSILON || !d.is_finite() {
        if d.is_finite() && d <= near { 0.0 } else { 1.0 }
    } else {
        ((d - near) / span).clamp(0.0, 1.0)
    };
    let u = if u.is_nan() { 1.0 } else { u };
    let q = (1.0 - u) * (1.0 - u);
    let r = (1.0 - q) * far_scale + q * near_scale;
    if r.is_finite() {
        r
    } else if far_scale.is_finite() {
        far_scale
    } else {
        1.0
    }
}

/// The base rise speed of §6.2 for a screen height *H*: *H* × 0.125 ÷ 1.5 pixels per
/// second.
#[must_use]
pub fn base_rise_speed(h: f32) -> f32 {
    h * 0.125 / 1.5
}

/// The growth *k* of §6.3 step 2 at *t*.
#[must_use]
pub fn text_scale(t: f32, s1: f32, critical: bool, c: f32, i: f32) -> f32 {
    if critical {
        if c > 0.0 && t < c {
            s1 * (1.0 + (c - t) / c * (i - 1.0))
        } else {
            s1
        }
    } else if t < 0.5 {
        1.0 + 2.0 * t * (s1 - 1.0)
    } else {
        s1
    }
}

/// The opacity of §6.3 step 4 at *t*.
#[must_use]
pub fn text_opacity(t: f32) -> f32 {
    if t <= 0.5 {
        1.0
    } else {
        (1.0 - 2.0 * (t - 0.5)).clamp(0.0, 1.0)
    }
}

/// The placement of §6.4: the screen box (left, top, width, height) of an element of design
/// size `design` with growth `k`, distance scale `sigma`, offsets `(dx, dy)` (design pixels,
/// multiplied by *σ*), centred horizontally on the anchor with its bottom edge at the
/// anchor plus the vertical offset, on a screen of `screen` pixels.
#[must_use]
pub fn place(
    anchor: (f32, f32),
    design: (f32, f32),
    k: f32,
    sigma: f32,
    offset: (f32, f32),
    screen: (i32, i32),
) -> (f32, f32, f32, f32) {
    let (sw, sh) = (screen.0.max(1) as f32, screen.1.max(1) as f32);
    let aspect = sw / sh;
    let w = design.0 * k * sigma * (sw / 1024.0) * ((4.0 / 3.0) / aspect);
    let h = design.1 * k * sigma * (sh / 768.0);
    let cx = anchor.0 + offset.0 * sigma;
    let bottom = anchor.1 + offset.1 * sigma;
    (cx - w / 2.0, bottom - h, w, h)
}

/// One floating text as the client asks for it.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatyText {
    /// The text.
    pub text: String,
    /// Its colour.
    pub colour: u32,
    /// The critical flag.
    pub critical: bool,
    /// Life *L*, seconds.
    pub life: f32,
    /// Scale *s*₁.
    pub scale: f32,
}

impl FloatyText {
    /// A text with TR20's life and scale.
    #[must_use]
    pub fn new(text: impl Into<String>, colour: u32, critical: bool) -> Self {
        Self {
            text: text.into(),
            colour,
            critical,
            life: DEFAULT_LIFE,
            scale: DEFAULT_SCALE,
        }
    }

    /// The damage text of TR19–TR20: "−*n*" with *n* the amount rounded half away from
    /// zero; `None` for an amount that rounds to 0 or less. `on_local_player` picks the
    /// damage-taken colour.
    #[must_use]
    pub fn damage(amount: f64, critical: bool, on_local_player: bool) -> Option<Self> {
        let n = round_half_away(amount);
        (n > 0).then(|| {
            let c = if on_local_player {
                COLOUR_DAMAGE_TAKEN
            } else {
                COLOUR_DAMAGE
            };
            Self::new(format!("\u{2212}{n}"), c, critical)
        })
    }

    /// The heal text of TR19–TR20: "+*n*"; `None` for an amount that rounds to 0 or less.
    #[must_use]
    pub fn heal(amount: f64, critical: bool) -> Option<Self> {
        let n = round_half_away(amount);
        (n > 0).then(|| Self::new(format!("+{n}"), COLOUR_HEAL, critical))
    }

    /// An avoidance word (TR19: the resolved short-form token, or the fallback word).
    #[must_use]
    pub fn avoidance(word: impl Into<String>) -> Self {
        Self::new(word, COLOUR_AVOID, false)
    }
}

/// One text of a list.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    id: u64,
    text: FloatyText,
    /// Critical on an entity other than the local player (§6.3).
    crit: bool,
    start: Option<f64>,
    slot: Option<usize>,
    height: f32,
    element: Option<ElementKey>,
    /// The rise so far, design pixels (before *σ*).
    rise: f32,
}

/// The texts of one entity (§6.5).
#[derive(Debug, Clone, PartialEq, Default)]
struct EntityTexts {
    active: Vec<Entry>,
    pending: VecDeque<Entry>,
    slots: [bool; CRIT_SLOTS],
    speed: Option<f32>,
    last: Option<f64>,
    touched: u64,
}

/// What one shown text looks like this frame (for tests and the client's checks).
#[derive(Debug, Clone, PartialEq)]
pub struct ShownText {
    /// The entity.
    pub entity: EntityId,
    /// The text.
    pub text: String,
    /// Screen box (left, top, width, height).
    pub bx: (f32, f32, f32, f32),
    /// Opacity.
    pub opacity: f32,
    /// Growth *k*.
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FloatyLook {
    Install { root: ElementKey, layout: Did },
    Fallback { root: ElementKey },
}

/// The floating-text layer.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingTexts {
    cv: CombatVocab,
    settings: FloatySettings,
    look: FloatyLook,
    lists: BTreeMap<EntityId, EntityTexts>,
    pool: Vec<ElementKey>,
    /// Elements of lists evicted by [`FloatingTexts::push`] (which has no `Ui`): hidden
    /// and pooled at the start of the next update or drop.
    stale: Vec<ElementKey>,
    next_id: u64,
    clock: u64,
    shown: Vec<ShownText>,
}

/// The z level of the floating-text layer (TR40: after the name blocks, before the
/// selection indicator, behind every panel). Higher z-levels draw behind lower ones
/// (`ui-layout.md` §9.3), so the layers take high levels: the name blocks
/// `LAYER_Z + 1`, the texts `LAYER_Z`, the selection indicator `LAYER_Z - 1`.
pub const LAYER_Z: i32 = 1_000_000;

impl FloatingTexts {
    /// The layer in the install look: texts are `DamageTextTemplate` instances of the
    /// `floatytext` layout `layout`. `None` when the layer root cannot be created.
    pub fn install(
        ui: &mut Ui,
        cv: &CombatVocab,
        layout: Did,
        settings: FloatySettings,
    ) -> Option<Self> {
        let root = layer_root(ui, LAYER_Z)?;
        Some(Self::with(
            cv,
            settings,
            FloatyLook::Install { root, layout },
        ))
    }

    /// The layer in the fallback look of §13.6 (settings of the column "0x78000004").
    pub fn fallback(ui: &mut Ui, cv: &CombatVocab) -> Option<Self> {
        let root = layer_root(ui, LAYER_Z)?;
        Some(Self::with(
            cv,
            FloatySettings::default(),
            FloatyLook::Fallback { root },
        ))
    }

    fn with(cv: &CombatVocab, settings: FloatySettings, look: FloatyLook) -> Self {
        Self {
            cv: *cv,
            settings: settings.sanitized(),
            look,
            lists: BTreeMap::new(),
            pool: Vec::new(),
            stale: Vec::new(),
            next_id: 0,
            clock: 0,
            shown: Vec::new(),
        }
    }

    /// Whether the layer uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, FloatyLook::Fallback { .. })
    }

    /// The layer's root.
    #[must_use]
    pub fn root(&self) -> ElementKey {
        match &self.look {
            FloatyLook::Install { root, .. } | FloatyLook::Fallback { root } => *root,
        }
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> &FloatySettings {
        &self.settings
    }

    /// Adds a text over `entity` (to its pending queue, §6.5). `local_player` says whether
    /// the entity is the local player (a critical text on it is shown like a normal one).
    /// Returns `false` when refused (more than [`MAX_PENDING`] pending, or empty text).
    pub fn push(&mut self, entity: EntityId, text: FloatyText, local_player: bool) -> bool {
        if text.text.is_empty() {
            return false;
        }
        if !self.lists.contains_key(&entity) && self.lists.len() >= MAX_ENTITIES {
            // Drop the least recently touched list.
            if let Some(old) = self
                .lists
                .iter()
                .min_by_key(|(_, l)| l.touched)
                .map(|(e, _)| *e)
                && let Some(l) = self.lists.remove(&old)
            {
                self.recycle_list(l);
            }
        }
        self.clock += 1;
        let clock = self.clock;
        let list = self.lists.entry(entity).or_default();
        list.touched = clock;
        if list.pending.len() > MAX_PENDING {
            return false;
        }
        let mut t = text;
        t.text = cap_label(&t.text);
        if !t.life.is_finite() || t.life <= 0.0 {
            t.life = DEFAULT_LIFE;
        }
        if !t.scale.is_finite() || t.scale <= 0.0 {
            t.scale = DEFAULT_SCALE;
        }
        let crit = t.critical && !local_player;
        self.next_id += 1;
        list.pending.push_back(Entry {
            id: self.next_id,
            text: t,
            crit,
            start: None,
            slot: None,
            height: 0.0,
            element: None,
            rise: 0.0,
        });
        true
    }

    /// Queues a dropped list's elements for hiding and reuse ([`Self::flush_stale`]).
    fn recycle_list(&mut self, l: EntityTexts) {
        for e in l.active.into_iter().chain(l.pending) {
            if let Some(k) = e.element {
                self.stale.push(k);
            }
        }
    }

    /// Hides the elements of dropped lists and moves them to the pool (CUI4: an evicted
    /// list's texts never stay on screen).
    fn flush_stale(&mut self, ui: &mut Ui) {
        for k in std::mem::take(&mut self.stale) {
            if ui.exists(k) {
                ui.set_shown(k, false);
                self.pool.push(k);
            }
        }
        self.trim_pool(ui);
    }

    /// Drops an entity's texts (despawn).
    pub fn drop_entity(&mut self, ui: &mut Ui, entity: EntityId) {
        if let Some(l) = self.lists.remove(&entity) {
            self.recycle_list(l);
        }
        self.flush_stale(ui);
    }

    /// The active and pending counts of an entity.
    #[must_use]
    pub fn counts(&self, entity: EntityId) -> (usize, usize) {
        self.lists
            .get(&entity)
            .map_or((0, 0), |l| (l.active.len(), l.pending.len()))
    }

    /// The critical slots taken over an entity.
    #[must_use]
    pub fn slots_taken(&self, entity: EntityId) -> usize {
        self.lists
            .get(&entity)
            .map_or(0, |l| l.slots.iter().filter(|s| **s).count())
    }

    /// The texts shown by the last [`FloatingTexts::update`], older first.
    #[must_use]
    pub fn shown(&self) -> &[ShownText] {
        &self.shown
    }

    /// Per-frame work (§6.3–§6.5): smooths each list's rise speed, releases at most one
    /// pending text per list, ages, scales, raises, fades and places the active texts at
    /// their entities' anchors, and removes the expired ones. `name_height` gives the
    /// height of an entity's name block when it shows one (the initial vertical offset
    /// *y*₀ = −height), else 0.
    pub fn update(
        &mut self,
        ui: &mut Ui,
        now: f64,
        anchors: &dyn AnchorSource,
        name_height: &dyn Fn(EntityId) -> f32,
    ) {
        self.flush_stale(ui);
        let screen = ui.screen_size();
        let base = base_rise_speed(screen.1 as f32);
        let st = self.settings;
        let mut shown = Vec::new();
        let mut freed: Vec<ElementKey> = Vec::new();
        let entities: Vec<EntityId> = self.lists.keys().copied().collect();
        for ent in entities {
            let anchor = anchors.anchor(ent);
            let y0 = -name_height(ent).max(0.0);
            // Rise speed (§6.5).
            let (n, dt) = {
                let l = &self.lists[&ent];
                let dt = l.last.map_or(0.0, |t| (now - t).max(0.0));
                (l.active.len() + l.pending.len(), dt)
            };
            let target = if n > 2 {
                base * (1.0 + (n as f32 / 10.0).min(1.0))
            } else {
                base
            };
            // Release (§6.5): at most one per frame.
            let release = {
                let l = &self.lists[&ent];
                !l.pending.is_empty()
                    && l.active.len() < MAX_ACTIVE
                    && match l.active.iter().rev().find(|e| !e.crit) {
                        None => true,
                        Some(e) => e.rise > (1.0 - RELEASE_SPACING) * e.height,
                    }
            };
            if release {
                let entry = self.lists.get_mut(&ent).and_then(|l| l.pending.pop_front());
                if let Some(mut e) = entry {
                    let el = self.take_element(ui);
                    if let Some(k) = el {
                        self.style(ui, k, &e);
                        crate::game::move_to_front(ui, k);
                        e.height = ui.layout_box(k).map_or(0.0, |b| b.h() as f32);
                    }
                    e.element = el;
                    e.start = Some(now);
                    if let Some(l) = self.lists.get_mut(&ent) {
                        if e.crit {
                            let free = l.slots.iter().position(|s| !s).unwrap_or(0);
                            l.slots[free] = true;
                            e.slot = Some(free);
                        }
                        l.active.push(e);
                    }
                }
            }
            let Some(l) = self.lists.get_mut(&ent) else {
                continue;
            };
            let v = l.speed.unwrap_or(base);
            let a = if dt > 0.0 {
                1.0 - (-dt / RISE_SMOOTHING).exp()
            } else {
                0.0
            };
            let v = v + (target - v) * a as f32;
            l.speed = Some(v);
            l.last = Some(now);
            let mut keep = Vec::with_capacity(l.active.len());
            for mut e in l.active.drain(..) {
                let start = e.start.unwrap_or(now);
                let el = (now - start).max(0.0);
                let t = ((el / f64::from(e.text.life)) as f32).clamp(0.0, 1.1);
                if t >= REMOVE_AT {
                    if let Some(s) = e.slot {
                        l.slots[s] = false;
                    }
                    if let Some(k) = e.element {
                        ui.set_shown(k, false);
                        freed.push(k);
                    }
                    continue;
                }
                let m = if e.crit { st.crit_speed_mod } else { 1.0 };
                e.rise = v * el as f32 * m;
                let k = text_scale(
                    t,
                    e.text.scale,
                    e.crit,
                    st.crit_shrink_time,
                    st.crit_initial_scale,
                );
                let (ox, oy) = e.slot.map_or((0.0, 0.0), |s| {
                    let (a, b) = CRIT_SLOT_OFFSETS[s];
                    (st.crit_h_offset * a, st.crit_v_offset * b)
                });
                let visible =
                    t < 1.0 && anchor.is_some_and(|a: ScreenAnchor| a.on_screen && a.drawn);
                if let (Some(elk), Some(a)) = (e.element, anchor) {
                    if visible {
                        let sigma = distance_scale(
                            a.distance,
                            st.near_range,
                            st.far_range,
                            st.near_scale,
                            st.far_scale,
                        );
                        let d = ui
                            .layout_box(elk)
                            .map_or((0.0, 0.0), |b| (b.w() as f32, b.h() as f32));
                        let bx = place((a.x, a.y), d, k, sigma, (ox, y0 + oy - e.rise), screen);
                        let o = text_opacity(t);
                        put(ui, elk, bx, d, o);
                        ui.set_shown(elk, true);
                        shown.push(ShownText {
                            entity: ent,
                            text: e.text.text.clone(),
                            bx,
                            opacity: o,
                            scale: k,
                        });
                    } else {
                        ui.set_shown(elk, false);
                    }
                } else if let Some(elk) = e.element {
                    ui.set_shown(elk, false);
                }
                keep.push(e);
            }
            l.active = keep;
            if l.active.is_empty() && l.pending.is_empty() {
                self.lists.remove(&ent);
            }
        }
        self.pool.extend(freed);
        self.trim_pool(ui);
        self.shown = shown;
    }

    fn trim_pool(&mut self, ui: &mut Ui) {
        while self.pool.len() > MAX_POOL {
            if let Some(k) = self.pool.pop() {
                ui.destroy(k);
            }
        }
    }

    /// An element for a released text: from the pool, or newly made.
    fn take_element(&mut self, ui: &mut Ui) -> Option<ElementKey> {
        while let Some(k) = self.pool.pop() {
            if ui.exists(k) {
                return Some(k);
            }
        }
        let root = self.root();
        let k = match &self.look {
            FloatyLook::Install { layout, .. } => {
                match ui.create_element(Some(root), *layout, self.cv.ids.damage_text) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::debug!("floating text not created: {e}");
                        return None;
                    }
                }
            }
            FloatyLook::Fallback { .. } => {
                let (w, h) = FALLBACK_SIZE;
                look::element(
                    ui,
                    root,
                    WidgetClass::Text,
                    IBox::from_xywh(0, 0, w, h),
                    look::LOOK_ELEMENT_ID,
                )?
            }
        };
        look::own_surface(ui, k);
        ui.set_shown(k, false);
        Some(k)
    }

    /// Sets an element's text and colours for an entry (§6.3, §13.6).
    fn style(&self, ui: &mut Ui, k: ElementKey, e: &Entry) {
        let tp = self.cv.text;
        let colour = |c: u32| PropertyValue::Colour(c.to_le_bytes());
        match &self.look {
            FloatyLook::Install { .. } => {
                if e.crit {
                    crate::game::switch_if_defined(ui, k, self.cv.states.floaty_critical);
                    ui.set_property(k, tp.font_color, colour(COLOUR_CRITICAL));
                    ui.set_property(k, tp.outline_color, colour(0xFFFF_FFFF));
                } else {
                    ui.switch_state(k, 0);
                    ui.set_property(k, tp.font_color, colour(e.text.colour));
                    ui.clear_property(k, tp.outline_color);
                }
                if let Some(el) = ui.el_mut(k) {
                    el.text.dirty = true;
                }
            }
            FloatyLook::Fallback { .. } => {
                let (c, o) = if e.crit {
                    (COLOUR_CRITICAL_FALLBACK, 0xFFFF_FFFF)
                } else {
                    (e.text.colour, 0xFF00_0000)
                };
                look::style_text(
                    ui,
                    &tp,
                    k,
                    TextStyle::new(FALLBACK_PX, c).centred().outlined(o),
                );
            }
        }
        ui.set_text(k, Some(e.text.text.clone()));
    }
}

/// Places an element of design size `d` at screen box `bx` through its node scale, with
/// opacity `o`.
pub(crate) fn put(ui: &mut Ui, k: ElementKey, bx: (f32, f32, f32, f32), d: (f32, f32), o: f32) {
    let (l, t, w, h) = bx;
    let (dw, dh) = (d.0.max(1.0), d.1.max(1.0));
    let sx = ((w + 1.0) / (dw + 1.0)).max(0.001);
    let sy = ((h + 1.0) / (dh + 1.0)).max(0.001);
    let clampc = |v: f32| v.clamp(-1.0e6, 1.0e6) as i32;
    ui.set_layout_box(
        k,
        IBox::from_xywh(clampc(l), clampc(t), dw as i32, dh as i32),
    );
    let scale = if (sx - 1.0).abs() < 1.0e-4 && (sy - 1.0).abs() < 1.0e-4 {
        None
    } else if sx.is_finite() && sy.is_finite() {
        Some([sx, sy, 1.0])
    } else {
        None
    };
    if ui.el(k).is_some_and(|e| e.scale != scale) {
        ui.set_node_scale(k, scale);
    }
    look::set_opacity(ui, k, o);
}

/// A full-screen, click-through layer root at z level `z`.
pub(crate) fn layer_root(ui: &mut Ui, z: i32) -> Option<ElementKey> {
    let (w, h) = ui.screen_size();
    look::screen_root(ui, IBox::from_xywh(0, 0, w, h), z)
}
