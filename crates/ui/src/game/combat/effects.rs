// SPDX-License-Identifier: GPL-3.0-or-later
//! Effect bars: the player's effect rows E8, the target's E9 and their icons E10
//! (`combat-ui.md` §8, §13.5; TR29–TR31): rows by harm, order by start time, the pulse of
//! icons near their end, removal by the session.

use std::collections::BTreeMap;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::data::{EffectView, cap_label};
use super::look::{self, TextStyle};
use super::vocab::CombatVocab;
use crate::game::find_below;

/// An icon pulses when its timeout is less than this many seconds away (§8.2).
pub const PULSE_LEAD: f64 = 15.0;

/// One pulse cycle, seconds (§8.2: 1 s down to 0.3, 1 s back to 1).
pub const PULSE_PERIOD: f64 = 2.0;

/// The lowest opacity of a pulse (§8.2).
pub const PULSE_LOW: f32 = 0.3;

/// Icons a row shows (`UIComponent_EffectDisplay_MaxIconColumns`, 8 in P4 and P5; TR30).
pub const DEFAULT_MAX_COLUMNS: usize = 8;

/// The icon size when the list has no `EffectDisplayList_EffectSize` (§8.1).
pub const DEFAULT_ICON_SIZE: i32 = 20;

/// Most effects kept per list (Tessera cap, hostile input).
pub const MAX_EFFECTS: usize = 256;

/// Rows of a list (§8.1: `max_rows` 2).
pub const ROWS: usize = 2;

/// The pulse opacity `t` seconds after the pulse began (§8.2: 1 → 0.3 over 1 s, then
/// 0.3 → 1 over 1 s, repeating).
#[must_use]
pub fn pulse_opacity(t: f64) -> f32 {
    if !t.is_finite() || t < 0.0 {
        return 1.0;
    }
    let ph = (t % PULSE_PERIOD) as f32;
    if ph < 1.0 {
        1.0 - (1.0 - PULSE_LOW) * ph
    } else {
        PULSE_LOW + (1.0 - PULSE_LOW) * (ph - 1.0)
    }
}

/// Whether an icon pulses at `now` (§8.2): a combat-only icon while the player is out of
/// combat; any other icon whose timeout is less than [`PULSE_LEAD`] away.
#[must_use]
pub fn pulses(e: &EffectView, now: f64, in_combat: bool) -> bool {
    if e.combat_only {
        return !in_combat;
    }
    e.timeout().is_some_and(|t| t - now < PULSE_LEAD)
}

/// The ordered rows of one effect list (§8.2, TR30), independent of any element.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectRows {
    rows: [Vec<EffectView>; ROWS],
}

impl EffectRows {
    /// The row of an effect (TR30): beneficial in row 0 (the first), harmful in row 1.
    #[must_use]
    pub fn row_of(e: &EffectView) -> usize {
        usize::from(e.harmful)
    }

    /// The sort key of §8.2: the start time, or the largest value for an icon without a
    /// duration.
    fn key(e: &EffectView) -> f64 {
        if e.timeout().is_some() {
            e.start
        } else {
            f64::MAX
        }
    }

    /// Adds an effect (replacing an icon of the same instance) by §8.2: an untimed
    /// effect goes to the front; a timed one goes after the last icon (from the end) whose
    /// start is later than its own (untimed icons count as the latest), else to the front.
    ///
    /// §8.2's "an icon whose effect ID is already in the row goes to the front" conflicts
    /// with CUI7 for timed icons (`combat-ui.md` §8.2 OPEN). Tessera keeps CUI7: the rule
    /// moves an icon to the front of its own group only, which is where the two placements
    /// above already put it (untimed icons at position 0, a timed icon first among the
    /// icons that started no later than it). Returns `false` when the list is full
    /// ([`MAX_EFFECTS`]).
    pub fn insert(&mut self, e: EffectView) -> bool {
        self.remove(e.instance);
        if self.len() >= MAX_EFFECTS {
            return false;
        }
        let r = Self::row_of(&e);
        let row = &mut self.rows[r];
        let untimed = e.timeout().is_none();
        let at = if untimed {
            0
        } else {
            let k = Self::key(&e);
            row.iter()
                .rposition(|x| Self::key(x) > k)
                .map_or(0, |j| j + 1)
        };
        row.insert(at.min(row.len()), e);
        true
    }

    /// Removes the icon of an instance; whether one was removed.
    pub fn remove(&mut self, instance: u64) -> bool {
        let mut hit = false;
        for row in &mut self.rows {
            let n = row.len();
            row.retain(|x| x.instance != instance);
            hit |= row.len() != n;
        }
        hit
    }

    /// Removes every icon.
    pub fn clear(&mut self) {
        for r in &mut self.rows {
            r.clear();
        }
    }

    /// The number of effects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.iter().map(Vec::len).sum()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Row `r` in order (empty for `r` ≥ [`ROWS`]).
    #[must_use]
    pub fn row(&self, r: usize) -> &[EffectView] {
        self.rows.get(r).map_or(&[], Vec::as_slice)
    }

    /// The icons of row `r` that are shown: the first `max_columns` (TR30; the others are
    /// kept until earlier ones are removed).
    #[must_use]
    pub fn shown(&self, r: usize, max_columns: usize) -> &[EffectView] {
        let row = self.row(r);
        &row[..row.len().min(max_columns)]
    }

    /// Whether the rows satisfy CUI7: untimed icons precede timed ones and timed icons are
    /// in non-increasing order of start time.
    #[must_use]
    pub fn satisfies_cui7(&self) -> bool {
        self.rows.iter().all(|row| {
            row.windows(2).all(|w| {
                let (a, b) = (&w[0], &w[1]);
                match (a.timeout().is_some(), b.timeout().is_some()) {
                    (true, false) => false,
                    (true, true) => a.start >= b.start,
                    _ => true,
                }
            })
        })
    }
}

/// Which list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectOwner {
    /// E8, the player's effects.
    Player,
    /// E9, the selection's effects.
    Target,
}

/// One shown icon.
#[derive(Debug, Clone, PartialEq)]
struct Icon {
    root: ElementKey,
    image: ElementKey,
    pulse_since: Option<f64>,
    /// The view the icon was made from (a new `EffectApplied` of the instance remakes it).
    view: EffectView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EffectLook {
    Install {
        display: ElementKey,
        layout: Did,
        rows: [ElementKey; ROWS],
    },
    Fallback {
        rows: [ElementKey; ROWS],
    },
}

/// One effect list in its install or fallback look.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectBar {
    owner: EffectOwner,
    cv: CombatVocab,
    look: EffectLook,
    model: EffectRows,
    icons: BTreeMap<u64, Icon>,
    size: i32,
    gap: i32,
    max_columns: usize,
    dirty: bool,
}

impl EffectBar {
    /// The list of an opened install panel: `root` is the frame (`AvatarPanel`,
    /// `OpponentPanel`) or the `EffectDisplayList` itself; `effects` is the `effects`
    /// layout (`TimedEffectTemplate`). `None` without the list element. The two rows are
    /// code-made fields one under the other inside the list, and icons are placed side by
    /// side without spacing (Tessera's arrangement of the horizontal row list boxes).
    pub fn install(
        ui: &mut Ui,
        cv: &CombatVocab,
        owner: EffectOwner,
        root: ElementKey,
        effects: Did,
    ) -> Option<Self> {
        let id = match owner {
            EffectOwner::Player => cv.ids.player_effects,
            EffectOwner::Target => cv.ids.opponent_effects,
        };
        let display = if ui.element_id(root) == Some(id) {
            root
        } else {
            find_below(ui, root, id)?
        };
        let size = {
            let p = ui.prop(display, cv.pids.effect_size);
            if p.is_set() && p.int() > 0 {
                p.int().min(256)
            } else {
                DEFAULT_ICON_SIZE
            }
        };
        let max_columns = {
            let p = ui.prop(display, cv.pids.effect_max_columns);
            if p.is_set() && p.int() > 0 {
                usize::try_from(p.int())
                    .unwrap_or(DEFAULT_MAX_COLUMNS)
                    .min(64)
            } else {
                DEFAULT_MAX_COLUMNS
            }
        };
        let w = size.saturating_mul(max_columns as i32);
        let r0 = look::element(
            ui,
            display,
            WidgetClass::Field,
            IBox::from_xywh(0, 0, w, size),
            look::LOOK_ELEMENT_ID,
        )?;
        let r1 = look::element(
            ui,
            display,
            WidgetClass::Field,
            IBox::from_xywh(0, size, w, size),
            look::LOOK_ELEMENT_ID,
        )?;
        Some(Self {
            owner,
            cv: *cv,
            look: EffectLook::Install {
                display,
                layout: effects,
                rows: [r0, r1],
            },
            model: EffectRows::default(),
            icons: BTreeMap::new(),
            size,
            gap: 0,
            max_columns,
            dirty: false,
        })
    }

    /// The fallback rows of §13.1 / §13.5 under the HUD root.
    pub fn fallback(
        ui: &mut Ui,
        cv: &CombatVocab,
        owner: EffectOwner,
        hud: ElementKey,
    ) -> Option<Self> {
        let x = match owner {
            EffectOwner::Player => 46,
            EffectOwner::Target => 270,
        };
        let size = DEFAULT_ICON_SIZE;
        let gap = 2;
        let w = (size + gap) * DEFAULT_MAX_COLUMNS as i32;
        let r0 = look::element(
            ui,
            hud,
            WidgetClass::Field,
            IBox::from_xywh(x, 76, w, size),
            look::LOOK_ELEMENT_ID,
        )?;
        let r1 = look::element(
            ui,
            hud,
            WidgetClass::Field,
            IBox::from_xywh(x, 98, w, size),
            look::LOOK_ELEMENT_ID,
        )?;
        Some(Self {
            owner,
            cv: *cv,
            look: EffectLook::Fallback { rows: [r0, r1] },
            model: EffectRows::default(),
            icons: BTreeMap::new(),
            size,
            gap,
            max_columns: DEFAULT_MAX_COLUMNS,
            dirty: false,
        })
    }

    /// Which list.
    #[must_use]
    pub fn owner(&self) -> EffectOwner {
        self.owner
    }

    /// Whether the list uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, EffectLook::Fallback { .. })
    }

    /// The model.
    #[must_use]
    pub fn rows(&self) -> &EffectRows {
        &self.model
    }

    /// The row elements (shown while they hold icons).
    #[must_use]
    pub fn row_elements(&self) -> [ElementKey; ROWS] {
        match &self.look {
            EffectLook::Install { rows, .. } | EffectLook::Fallback { rows } => *rows,
        }
    }

    /// The icon element of an instance, while shown.
    #[must_use]
    pub fn icon(&self, instance: u64) -> Option<ElementKey> {
        self.icons.get(&instance).map(|i| i.root)
    }

    /// The instance whose icon (or one of its parts) is `k` (for the examination hover,
    /// `skill-tooltips.md` §12.2).
    #[must_use]
    pub fn effect_at(&self, ui: &Ui, k: ElementKey) -> Option<u64> {
        self.icons
            .iter()
            .find(|(_, i)| ui.is_ancestor_or_self(i.root, k))
            .map(|(id, _)| *id)
    }

    /// `EffectApplied` (TR30): adds the icon. The elements change at the next
    /// [`EffectBar::update`].
    pub fn apply(&mut self, view: EffectView) {
        let mut v = view;
        v.label = cap_label(&v.label);
        if self.model.insert(v) {
            self.dirty = true;
        }
    }

    /// `EffectRemoved` (TR30): removes the icon; whether there was one.
    pub fn remove(&mut self, instance: u64) -> bool {
        let hit = self.model.remove(instance);
        self.dirty |= hit;
        hit
    }

    /// Replaces every effect (E9 rebuilt on a selection change).
    pub fn set_all(&mut self, views: &[EffectView]) {
        self.model.clear();
        for v in views.iter().take(MAX_EFFECTS) {
            let mut v = v.clone();
            v.label = cap_label(&v.label);
            self.model.insert(v);
        }
        self.dirty = true;
    }

    /// Per-frame work: rebuilds the shown icons after a change and runs the pulse
    /// (§8.2; `in_combat` is the player's combat state).
    pub fn update(&mut self, ui: &mut Ui, now: f64, in_combat: bool) {
        if self.dirty {
            self.rebuild(ui);
            self.dirty = false;
        }
        let pulse = self.cv.states.pulse;
        let fallback = self.is_fallback();
        for r in 0..ROWS {
            for e in self.model.shown(r, self.max_columns) {
                let Some(icon) = self.icons.get_mut(&e.instance) else {
                    continue;
                };
                let on = pulses(e, now, in_combat);
                match (on, icon.pulse_since) {
                    (true, None) => {
                        icon.pulse_since = Some(now);
                        if !fallback {
                            crate::game::switch_if_defined(ui, icon.image, pulse);
                        }
                    }
                    (false, Some(_)) => {
                        icon.pulse_since = None;
                        if !fallback {
                            ui.switch_state(icon.image, 0);
                        }
                    }
                    _ => {}
                }
                if fallback {
                    let o = icon.pulse_since.map_or(1.0, |t0| pulse_opacity(now - t0));
                    look::set_opacity(ui, icon.root, o);
                }
            }
        }
    }

    /// Removes the code-made rows and their icons (before dropping the bar or rebuilding
    /// the combat widgets over panels that stay open).
    pub fn detach(&mut self, ui: &mut Ui) {
        for r in self.row_elements() {
            ui.destroy(r);
        }
        self.icons.clear();
        self.model.clear();
    }

    fn rebuild(&mut self, ui: &mut Ui) {
        let mut old = std::mem::take(&mut self.icons);
        let mut keep: BTreeMap<u64, Icon> = BTreeMap::new();
        let rows = self.row_elements();
        // §8.1: a row is an auto-resizing list box that resizes to zero when empty, inside
        // a vertical list: in the install look the second row moves up when the first is
        // empty. The fallback rows keep their §13.1 places.
        if let EffectLook::Install { .. } = &self.look {
            let first_empty = self.model.shown(0, self.max_columns).is_empty();
            let y = if first_empty { 0 } else { self.size };
            if let Some(b) = ui.layout_box(rows[1]) {
                ui.set_layout_box(rows[1], IBox::from_xywh(b.l, y, b.w(), b.h()));
            }
        }
        for (r, row_el) in rows.iter().enumerate() {
            let shown: Vec<EffectView> = self.model.shown(r, self.max_columns).to_vec();
            ui.set_shown(*row_el, !shown.is_empty());
            for (j, e) in shown.iter().enumerate() {
                let x = (self.size + self.gap) * j as i32;
                let bx = IBox::from_xywh(x, 0, self.size, self.size);
                let icon = match old.remove(&e.instance) {
                    Some(i) if ui.parent(i.root) == Some(*row_el) && i.view == *e => {
                        ui.set_layout_box(i.root, bx);
                        Some(i)
                    }
                    Some(i) => {
                        ui.destroy(i.root);
                        self.make_icon(ui, *row_el, bx, e)
                    }
                    None => self.make_icon(ui, *row_el, bx, e),
                };
                if let Some(i) = icon {
                    keep.insert(e.instance, i);
                }
            }
        }
        for i in old.into_values() {
            ui.destroy(i.root);
        }
        self.icons = keep;
    }

    fn make_icon(&self, ui: &mut Ui, row: ElementKey, bx: IBox, e: &EffectView) -> Option<Icon> {
        let icon = match &self.look {
            // TR41: the fallback look applies element by element, so an effect without
            // an icon (a literal, §13.5) takes it in the install look too.
            EffectLook::Install { layout, .. } if e.icon.is_some() => {
                self.install_icon(ui, *layout, row, bx, e)?
            }
            EffectLook::Install { .. } | EffectLook::Fallback { .. } => {
                self.fallback_icon(ui, row, bx, e)?
            }
        };
        let p = &self.cv.pids;
        let timeout = e.timeout();
        ui.set_property(
            icon.root,
            p.effect_id,
            PropertyValue::Int(e.effect_id as i32),
        );
        let start = if timeout.is_some() {
            e.start as f32
        } else {
            f32::MAX
        };
        ui.set_property(icon.root, p.effect_duration, PropertyValue::Float(start));
        ui.set_property(
            icon.root,
            p.effect_timeout,
            PropertyValue::Float(timeout.map_or(f32::MAX, |t| t as f32)),
        );
        ui.set_property(
            icon.root,
            p.effect_combat_only,
            PropertyValue::Bool(e.combat_only),
        );
        if let Some(t) = e.tooltip {
            ui.set_property(icon.image, p.tooltip_did, PropertyValue::DidRef(Did(t)));
        }
        ui.set_property(
            icon.image,
            p.tooltip_effect_id,
            PropertyValue::Int(e.effect_id as i32),
        );
        ui.set_property(
            icon.image,
            p.tooltip_effect_instance,
            PropertyValue::Int64(e.instance as i64),
        );
        ui.set_examine(icon.image, true);
        Some(icon)
    }

    fn install_icon(
        &self,
        ui: &mut Ui,
        layout: Did,
        row: ElementKey,
        bx: IBox,
        e: &EffectView,
    ) -> Option<Icon> {
        let ids = &self.cv.ids;
        let root = match ui.create_element(Some(row), layout, ids.effect_icon) {
            Ok(k) => k,
            Err(err) => {
                tracing::debug!("effect icon not created: {err}");
                return None;
            }
        };
        ui.set_layout_box(root, bx);
        ui.resize(root, bx.w(), bx.h());
        ui.move_to(root, bx.l, bx.t);
        let image = find_below(ui, root, ids.effect_image).unwrap_or(root);
        crate::game::set_icon(ui, image, e.icon.as_ref());
        if let Some(border) = find_below(ui, root, ids.effect_border) {
            match e.border {
                Some(s) => {
                    ui.set_shown(border, true);
                    crate::game::switch_if_defined(ui, border, s);
                }
                None => ui.set_shown(border, false),
            }
        }
        Some(Icon {
            root,
            image,
            pulse_since: None,
            view: e.clone(),
        })
    }

    fn fallback_icon(
        &self,
        ui: &mut Ui,
        row: ElementKey,
        bx: IBox,
        e: &EffectView,
    ) -> Option<Icon> {
        let fill = if e.harmful { 0xFF40_3030 } else { 0xFF30_4030 };
        let root = look::boxed(ui, row, bx, Some(fill), Some(0xFF80_8080))?;
        look::own_surface(ui, root);
        let first: String = e
            .label
            .chars()
            .next()
            .map_or_else(|| "?".to_string(), |c| c.to_string());
        look::text(
            ui,
            &self.cv.text,
            root,
            IBox::from_xywh(0, 0, bx.w(), bx.h()),
            &first,
            TextStyle::new(12, 0xFFFF_FFFF).centred(),
        );
        Some(Icon {
            root,
            image: root,
            pulse_since: None,
            view: e.clone(),
        })
    }
}
