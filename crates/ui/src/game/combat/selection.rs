// SPDX-License-Identifier: GPL-3.0-or-later
//! The on-screen selection display: the selection indicator E1 and the selection's name
//! block E3 with its mini vitals and mini induction bar (`combat-ui.md` §3.5, TR7, TR8,
//! §13.7), placed at the selection's anchor.

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::data::{HealthColour, NameColour, ScreenAnchor, Vital, cap_label};
use super::floaty::{FloatySettings, LAYER_Z, distance_scale, layer_root, place, put};
use super::frames::{fallback_health_colour, fallback_name_colour, health_state, name_state};
use super::look::{self, Bar, TextStyle};
use super::vocab::CombatVocab;
use crate::game::{find_below, subtree};

/// The indicator's vertical offset, design pixels (§3.5 step 2).
pub const INDICATOR_OFFSET: f32 = -75.0;
/// The indicator's near distance scale (§3.5 step 2).
pub const INDICATOR_NEAR_SCALE: f32 = 0.25;
/// The indicator's far distance scale (§3.5 step 2).
pub const INDICATOR_FAR_SCALE: f32 = 0.2;

/// The fallback name block's size (§13.7: name 14 px, a 60 × 5 health bar and a 60 × 4
/// cast bar under it; Tessera's arrangement).
pub const FALLBACK_BLOCK: (i32, i32) = (120, 29);
/// The fallback indicator triangle (§13.7).
pub const FALLBACK_TRIANGLE: (u32, u32) = (16, 12);
/// The fallback indicator's colour in peace (§13.7).
pub const TRIANGLE_PEACE: u32 = 0xFFFF_E060;
/// The fallback indicator's colour in combat (§13.7).
pub const TRIANGLE_COMBAT: u32 = 0xFFFF_6040;

/// What the selection display shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectionView {
    /// The selection's name.
    pub name: String,
    /// Its name colour (TR15).
    pub colour: Option<NameColour>,
    /// Health, when the selection has vitals (TR8).
    pub health: Option<Vital>,
    /// Power, when it has vitals.
    pub power: Option<Vital>,
    /// The mini health meter's colour state (TR8 by §5.3, TR14).
    pub health_colour: HealthColour,
    /// The selection's cast position *p*, while it casts (TR26).
    pub cast: Option<f32>,
    /// Whether the selection's cast is interruptible (§7.2: the mini induction meter takes
    /// `vitalmeter_yellow`, else `vitalmeter_grey`).
    pub cast_interruptible: bool,
    /// Whether the player is in combat (TR7).
    pub in_combat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkerLook {
    Install {
        block: Option<ElementKey>,
        name: Option<ElementKey>,
        title: Option<ElementKey>,
        vitals: Option<ElementKey>,
        health: Option<ElementKey>,
        power: Option<ElementKey>,
        zero_meters: Vec<ElementKey>,
        induction: Option<ElementKey>,
        induction_meter: Option<ElementKey>,
        indicator: Option<ElementKey>,
    },
    Fallback {
        block: ElementKey,
        name: Option<ElementKey>,
        health: Option<Bar>,
        cast: Option<Bar>,
        indicator: Option<ElementKey>,
    },
}

/// The selection indicator and name block.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionMarker {
    cv: CombatVocab,
    settings: FloatySettings,
    look: MarkerLook,
    roots: [ElementKey; 2],
    view: Option<SelectionView>,
}

fn meters_below(ui: &Ui, k: Option<ElementKey>) -> Vec<ElementKey> {
    k.map(|k| {
        subtree(ui, k)
            .into_iter()
            .filter(|c| *c != k && ui.class(*c) == Some(WidgetClass::Meter))
            .collect()
    })
    .unwrap_or_default()
}

impl SelectionMarker {
    /// The install look: `SelectionNameTextTemplate` and `EntitySelectionIndicator` of the
    /// `floatytext` layout `layout`. Parts that cannot be created are left out.
    pub fn install(
        ui: &mut Ui,
        cv: &CombatVocab,
        layout: Did,
        settings: FloatySettings,
    ) -> Option<Self> {
        let blocks = layer_root(ui, LAYER_Z + 1)?;
        let marks = layer_root(ui, LAYER_Z - 1)?;
        let ids = &cv.ids;
        let make = |ui: &mut Ui, parent: ElementKey, id: u32| match ui.create_element(
            Some(parent),
            layout,
            id,
        ) {
            Ok(k) => {
                look::own_surface(ui, k);
                ui.set_shown(k, false);
                Some(k)
            }
            Err(e) => {
                tracing::debug!("selection element {id:#x} not created: {e}");
                None
            }
        };
        let block = make(ui, blocks, ids.selection_name_block);
        let indicator = make(ui, marks, ids.selection_indicator);
        let f = |ui: &Ui, id: u32| block.and_then(|b| find_below(ui, b, id));
        let name = f(ui, ids.entity_name);
        let title = f(ui, ids.entity_title);
        let vitals = f(ui, ids.entity_mini_vitals);
        let induction = f(ui, ids.entity_mini_induction);
        // The mini vitals hold the target frame's meters (`SelectionMiniVitals` mirrors
        // `RealOpponentVitalsField`, §5.3): health and power by element ID, the temporary
        // meters kept at 0 (TR10). Without those IDs, the first two meters.
        let vm = meters_below(ui, vitals);
        let g = |id: u32| vitals.and_then(|v| find_below(ui, v, id));
        let health = g(ids.opponent_health_meter).or_else(|| vm.first().copied());
        let power = g(ids.opponent_power_meter)
            .or_else(|| {
                g(ids.opponent_power_field).and_then(|f| meters_below(ui, Some(f)).first().copied())
            })
            .or_else(|| {
                vm.get(1)
                    .copied()
                    .filter(|_| g(ids.opponent_health_meter).is_none())
            });
        let zero_meters: Vec<ElementKey> = [ids.opponent_temp_health, ids.opponent_temp_power]
            .into_iter()
            .filter_map(g)
            .collect();
        let induction_meter = meters_below(ui, induction)
            .first()
            .copied()
            .or(induction.filter(|k| ui.class(*k) == Some(WidgetClass::Meter)));
        Some(Self {
            cv: *cv,
            settings: settings.sanitized(),
            look: MarkerLook::Install {
                block,
                name,
                title,
                vitals,
                health,
                power,
                zero_meters,
                induction,
                induction_meter,
                indicator,
            },
            roots: [blocks, marks],
            view: None,
        })
    }

    /// The fallback look of §13.7.
    pub fn fallback(ui: &mut Ui, cv: &CombatVocab) -> Option<Self> {
        let blocks = layer_root(ui, LAYER_Z + 1)?;
        let marks = layer_root(ui, LAYER_Z - 1)?;
        let (bw, bh) = FALLBACK_BLOCK;
        let block = look::element(
            ui,
            blocks,
            WidgetClass::Field,
            IBox::from_xywh(0, 0, bw, bh),
            look::LOOK_ELEMENT_ID,
        )?;
        look::own_surface(ui, block);
        let name = look::text(
            ui,
            &cv.text,
            block,
            IBox::from_xywh(0, 0, bw, 16),
            "",
            TextStyle::new(14, 0xFFFF_FFFF)
                .centred()
                .outlined(0xFF00_0000),
        );
        let x = (bw - 60) / 2;
        let health = Bar::new(
            ui,
            block,
            IBox::from_xywh(x, 18, 60, 5),
            0xFF40_1010,
            0xFF30_C030,
            false,
        );
        let cast = Bar::new(
            ui,
            block,
            IBox::from_xywh(x, 25, 60, 4),
            0xFF30_3030,
            0xFFE0_C040,
            false,
        );
        let (tw, th) = FALLBACK_TRIANGLE;
        let indicator = look::element(
            ui,
            marks,
            WidgetClass::Field,
            IBox::from_xywh(0, 0, tw as i32, th as i32),
            look::LOOK_ELEMENT_ID,
        );
        ui.set_shown(block, false);
        if let Some(i) = indicator {
            ui.set_shown(i, false);
        }
        Some(Self {
            cv: *cv,
            settings: FloatySettings::default(),
            look: MarkerLook::Fallback {
                block,
                name,
                health,
                cast,
                indicator,
            },
            roots: [blocks, marks],
            view: None,
        })
    }

    /// Whether the marker uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, MarkerLook::Fallback { .. })
    }

    /// The name block element and the indicator element, when they exist.
    #[must_use]
    pub fn elements(&self) -> (Option<ElementKey>, Option<ElementKey>) {
        match &self.look {
            MarkerLook::Install {
                block, indicator, ..
            } => (*block, *indicator),
            MarkerLook::Fallback {
                block, indicator, ..
            } => (Some(*block), *indicator),
        }
    }

    /// The layer roots (name blocks, indicator).
    #[must_use]
    pub fn roots(&self) -> [ElementKey; 2] {
        self.roots
    }

    /// Sets what is shown (`None`: no selection, everything hides).
    pub fn set(&mut self, ui: &mut Ui, view: Option<SelectionView>) {
        let Some(mut v) = view else {
            self.hide(ui);
            self.view = None;
            return;
        };
        v.name = cap_label(&v.name);
        match &self.look {
            MarkerLook::Install {
                name,
                title,
                vitals,
                health,
                power,
                zero_meters,
                induction,
                induction_meter,
                indicator,
                ..
            } => {
                if let Some(n) = name {
                    ui.set_text(*n, Some(v.name.clone()));
                    if let Some(c) = v.colour {
                        crate::game::switch_if_defined(ui, *n, name_state(&self.cv, c));
                    }
                }
                if let Some(t) = title {
                    ui.set_text(*t, None);
                }
                crate::game::show(ui, *vitals, v.health.is_some());
                let mp = ui.vocab().pids.meter_position;
                for (m, val) in [(*health, v.health), (*power, v.power)] {
                    if let Some(m) = m {
                        let p = val.map_or(0.0, |x| x.fraction());
                        ui.set_property(m, mp, PropertyValue::Float(p));
                    }
                }
                for z in zero_meters {
                    ui.set_property(*z, mp, PropertyValue::Float(0.0));
                }
                if let Some(h) = health {
                    crate::game::switch_if_defined(ui, *h, health_state(&self.cv, v.health_colour));
                }
                crate::game::show(ui, *induction, v.cast.is_some());
                if let (Some(m), Some(p)) = (induction_meter, v.cast) {
                    ui.set_property(*m, mp, PropertyValue::Float(p.clamp(0.0, 1.0)));
                    let st = if v.cast_interruptible {
                        self.cv.states.meter_yellow
                    } else {
                        self.cv.states.meter_grey
                    };
                    crate::game::switch_if_defined(ui, *m, st);
                }
                if let Some(i) = indicator {
                    let s = if v.in_combat {
                        self.cv.states.floaty_combat
                    } else {
                        self.cv.states.floaty_peace
                    };
                    crate::game::switch_if_defined(ui, *i, s);
                }
            }
            MarkerLook::Fallback {
                name, health, cast, ..
            } => {
                if let Some(n) = name {
                    ui.set_text(*n, Some(v.name.clone()));
                    let c = v.colour.map_or(0xFFFF_FFFF, fallback_name_colour);
                    ui.set_property(
                        *n,
                        self.cv.text.font_color,
                        PropertyValue::Colour(c.to_le_bytes()),
                    );
                }
                if let Some(h) = health {
                    ui.set_shown(h.back, v.health.is_some());
                    h.set(ui, v.health.map_or(0.0, |x| x.fraction()));
                    h.set_colour(ui, fallback_health_colour(v.health_colour));
                }
                if let Some(c) = cast {
                    ui.set_shown(c.back, v.cast.is_some());
                    c.set(ui, v.cast.unwrap_or(0.0));
                }
            }
        }
        self.view = Some(v);
    }

    /// Updates the mini induction bar only (the selection's cast position, TR26, and
    /// whether that cast is interruptible, §7.2), and the indicator's combat state (TR7).
    pub fn set_cast(
        &mut self,
        ui: &mut Ui,
        cast: Option<f32>,
        interruptible: bool,
        in_combat: bool,
    ) {
        let Some(v) = &self.view else { return };
        if v.cast == cast && v.cast_interruptible == interruptible && v.in_combat == in_combat {
            return;
        }
        let mut v = v.clone();
        v.cast = cast;
        v.cast_interruptible = interruptible;
        v.in_combat = in_combat;
        self.set(ui, Some(v));
    }

    /// What is shown.
    #[must_use]
    pub fn view(&self) -> Option<&SelectionView> {
        self.view.as_ref()
    }

    fn hide(&self, ui: &mut Ui) {
        let (b, i) = self.elements();
        crate::game::show(ui, b, false);
        crate::game::show(ui, i, false);
    }

    /// The name block's height in design pixels (the floating texts' *y*₀, §6.3), when
    /// shown.
    #[must_use]
    pub fn block_height(&self, ui: &Ui) -> f32 {
        let (b, _) = self.elements();
        match b {
            Some(b) if self.view.is_some() && ui.is_visible(b) => {
                ui.layout_box(b).map_or(0.0, |x| x.h() as f32)
            }
            _ => 0.0,
        }
    }

    /// Per-frame placement at the selection's anchor (§3.5 steps 2–3, TR8, §13.7).
    pub fn update(&self, ui: &mut Ui, anchor: Option<ScreenAnchor>) {
        let Some(v) = &self.view else {
            self.hide(ui);
            return;
        };
        let screen = ui.screen_size();
        let (block, indicator) = self.elements();
        let Some(a) = anchor else {
            self.hide(ui);
            return;
        };
        let st = self.settings;
        let sigma = distance_scale(
            a.distance,
            st.near_range,
            st.far_range,
            st.near_scale,
            st.far_scale,
        );
        // Name block: offset 0, the settings' scales (TR8).
        let mut block_top = a.y;
        if let Some(b) = block {
            if a.on_screen && a.drawn {
                let d = ui
                    .layout_box(b)
                    .map_or((0.0, 0.0), |x| (x.w() as f32, x.h() as f32));
                let bx = place((a.x, a.y), d, 1.0, sigma, (0.0, 0.0), screen);
                block_top = bx.1;
                put(ui, b, bx, d, 1.0);
                ui.set_shown(b, true);
            } else {
                ui.set_shown(b, false);
            }
        }
        let Some(i) = indicator else { return };
        match &self.look {
            MarkerLook::Install { .. } => {
                let s2 = distance_scale(
                    a.distance,
                    st.near_range,
                    st.far_range,
                    INDICATOR_NEAR_SCALE,
                    INDICATOR_FAR_SCALE,
                );
                let d = ui
                    .layout_box(i)
                    .map_or((0.0, 0.0), |x| (x.w() as f32, x.h() as f32));
                let bx = place((a.x, a.y), d, 1.0, s2, (0.0, INDICATOR_OFFSET), screen);
                let bx = if a.on_screen {
                    bx
                } else {
                    clamp_to_screen(bx, screen).0
                };
                put(ui, i, bx, d, 1.0);
                ui.set_shown(i, true);
            }
            MarkerLook::Fallback { .. } => {
                let (tw, th) = FALLBACK_TRIANGLE;
                let (tw, th) = (tw as f32, th as f32);
                let colour = if v.in_combat {
                    TRIANGLE_COMBAT
                } else {
                    TRIANGLE_PEACE
                };
                // Centred 12 px above the name block (§13.7).
                let cy = block_top - 12.0;
                let bx = (a.x - tw / 2.0, cy - th / 2.0, tw, th);
                let (bx, deg) = if a.on_screen && a.drawn {
                    (bx, 0)
                } else {
                    let (c, _) = clamp_to_screen(bx, screen);
                    let (mx, my) = (c.0 + tw / 2.0, c.1 + th / 2.0);
                    // 0° points down; clockwise.
                    let ang = (a.x - mx).atan2(a.y - my).to_degrees();
                    (c, (-ang).round() as i32)
                };
                let r = look::triangle_ref(colour, tw as u32, th as u32, deg);
                look::set_procedural(ui, i, &r);
                put(ui, i, bx, (tw, th), 1.0);
                ui.set_shown(i, true);
            }
        }
    }
}

/// Clamps a box to the screen, keeping a margin of three quarters of its height at the
/// top (§3.5 step 3). Returns the box and whether it moved. Never panics: a non-finite
/// size counts as 0 and a non-finite position goes to the bound it passed.
#[must_use]
pub fn clamp_to_screen(
    bx: (f32, f32, f32, f32),
    screen: (i32, i32),
) -> ((f32, f32, f32, f32), bool) {
    let (l, t, w, h) = bx;
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    let (fw, fh) = (finite(w), finite(h));
    let (sw, sh) = (screen.0 as f32, screen.1 as f32);
    // `max`/`min` rather than `clamp`: the bounds are finite here, and a NaN position
    // takes the lower bound.
    let nl = l.max(0.0).min((sw - fw).max(0.0));
    let top = 0.75 * fh;
    let nt = t.max(top).min((sh - fh).max(top));
    ((nl, nt, w, h), nl != l || nt != t)
}
