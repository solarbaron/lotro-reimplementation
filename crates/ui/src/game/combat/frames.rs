// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit frames: the player frame E5 (P4 `AvatarPanel`), the target frame E6 (P5
//! `OpponentPanel`) and the selection's target E7 (`GamePlay_SelectionTarget`)
//! (`combat-ui.md` §4, §5, §13.1–§13.2), and the class-resource pip displays of P3 (§4.4).

use tessera_formats::property::PropertyValue;

use crate::geom::IBox;
use crate::props::{array_elements, as_int, as_key};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::data::{ClassResource, FrameView, HealthColour, LevelShown, NameColour, Vital};
use super::look::{self, Bar, TextStyle};
use super::vocab::CombatVocab;
use crate::game::{find_below, subtree};

/// Which frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameKind {
    /// E5, the local player.
    Player,
    /// E6, the selection.
    Target,
    /// E7, the selection's own target.
    SelectionTarget,
}

/// The fallback name colour of §13.2.
#[must_use]
pub fn fallback_name_colour(c: NameColour) -> u32 {
    use super::data::ConRank as R;
    match c {
        NameColour::Con(r) => match r {
            R::Trivial => 0xFF8C_8C8C,
            R::InsanelyEasy => 0xFF20_C020,
            R::VeryEasy => 0xFF20_D0D0,
            R::Easy => 0xFF30_90F0,
            R::Even => 0xFFF0_F0F0,
            R::Tough => 0xFFE0_D020,
            R::VeryTough => 0xFFF0_8020,
            R::InsanelyTough => 0xFFE0_2020,
            R::Impossible => 0xFFB0_30C0,
        },
        NameColour::Npc => 0xFFE0_E040,
        NameColour::Player => 0xFFC0_C060,
        NameColour::Object | NameColour::Corpse => 0xFFF0_F0F0,
        // §13.2 gives no tapped colour; Tessera uses the trivial grey (the original's
        // tapped and trivial states share one colour, §5.4).
        NameColour::Tapped => 0xFF8C_8C8C,
    }
}

/// The fallback health fill of §13.2.
#[must_use]
pub fn fallback_health_colour(c: HealthColour) -> u32 {
    match c {
        HealthColour::Red => 0xFFD0_3030,
        HealthColour::Green => 0xFF30_C030,
        HealthColour::Grey => 0xFF80_8080,
        HealthColour::Yellow => 0xFFD0_C030,
    }
}

/// The state ID of a name colour.
#[must_use]
pub fn name_state(cv: &CombatVocab, c: NameColour) -> u32 {
    let s = &cv.states;
    match c {
        NameColour::Con(r) => cv.con_states()[r.index()],
        NameColour::Npc => s.npc_name,
        NameColour::Player => s.player_name,
        NameColour::Object => s.object_name,
        NameColour::Corpse => s.corpse_name,
        NameColour::Tapped => s.tapped_name,
    }
}

/// The state ID of a health colour.
#[must_use]
pub fn health_state(cv: &CombatVocab, c: HealthColour) -> u32 {
    let s = &cv.states;
    match c {
        HealthColour::Red => s.meter_red,
        HealthColour::Green => s.meter_green,
        HealthColour::Grey => s.meter_grey,
        HealthColour::Yellow => s.meter_yellow,
    }
}

/// The level text of TR16.
#[must_use]
pub fn level_text(l: LevelShown) -> Option<String> {
    match l {
        LevelShown::Level(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The parts of an install frame.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallParts {
    root: ElementKey,
    name: Option<ElementKey>,
    alt_name: Option<ElementKey>,
    level: Option<ElementKey>,
    skull: Option<ElementKey>,
    health: Option<ElementKey>,
    power: Option<ElementKey>,
    power_field: Option<ElementKey>,
    rage_field: Option<ElementKey>,
    rage: Option<ElementKey>,
    zero_meters: Vec<ElementKey>,
    texts: Vec<(ElementKey, bool)>,
    vital_area: Option<ElementKey>,
    vital_field: Option<ElementKey>,
    nonvital_field: Option<ElementKey>,
    decoration: Vec<ElementKey>,
    class: Option<ElementKey>,
    portrait: Option<ElementKey>,
}

/// The parts of a fallback frame.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FallbackParts {
    root: ElementKey,
    vital: ElementKey,
    name: Option<ElementKey>,
    level: Option<ElementKey>,
    health: Option<Bar>,
    power: Option<Bar>,
    resource: Option<Bar>,
    nonvital: Option<ElementKey>,
    nonvital_name: Option<ElementKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FrameLook {
    Install(Box<InstallParts>),
    Fallback(Box<FallbackParts>),
}

/// One unit frame (E5, E6 or E7) in its install or fallback look (TR41).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitFrame {
    kind: FrameKind,
    cv: CombatVocab,
    look: FrameLook,
    shown: Option<FrameView>,
    /// The external content last given to the portrait
    /// ([`UnitFrame::set_portrait_content`]); `None` before the first call.
    pub(super) portrait_content: std::cell::Cell<Option<Option<u64>>>,
}

fn first_meter_below(ui: &Ui, k: Option<ElementKey>) -> Option<ElementKey> {
    let k = k?;
    subtree(ui, k)
        .into_iter()
        .find(|c| ui.class(*c) == Some(WidgetClass::Meter))
}

impl UnitFrame {
    /// The frame of an opened install panel `root` (`AvatarPanel`, `OpponentPanel` or
    /// `GamePlay_SelectionTarget`). `None` when the root lacks the frame's name element
    /// (the caller then uses [`UnitFrame::fallback`]).
    #[must_use]
    pub fn install(
        ui: &mut Ui,
        cv: &CombatVocab,
        kind: FrameKind,
        root: ElementKey,
    ) -> Option<Self> {
        let i = &cv.ids;
        let f = |id: u32| find_below(ui, root, id);
        let mut p = InstallParts {
            root,
            name: None,
            alt_name: None,
            level: None,
            skull: None,
            health: None,
            power: None,
            power_field: None,
            rage_field: None,
            rage: None,
            zero_meters: Vec::new(),
            texts: Vec::new(),
            vital_area: None,
            vital_field: None,
            nonvital_field: None,
            decoration: Vec::new(),
            class: None,
            portrait: None,
        };
        let mut hidden: Vec<ElementKey> = Vec::new();
        match kind {
            FrameKind::Player => {
                p.name = Some(f(i.player_name)?);
                p.level = f(i.player_level);
                p.health = f(i.health_meter);
                p.power = f(i.power_meter);
                p.power_field = f(i.power_field);
                p.rage_field = f(i.rage_field);
                p.rage = f(i.rage_meter);
                p.zero_meters = [i.temp_health_meter, i.temp_power_meter, i.dread_meter]
                    .into_iter()
                    .filter_map(f)
                    .collect();
                p.texts = [(i.health_text, true), (i.power_text, false)]
                    .into_iter()
                    .filter_map(|(id, h)| f(id).map(|k| (k, h)))
                    .collect();
                p.vital_area = f(i.vitals_field);
                p.decoration = [
                    i.avatar_bg,
                    i.player_animations,
                    i.player_level_bg,
                    i.avatar_cover,
                ]
                .into_iter()
                .filter_map(f)
                .collect();
                p.portrait = f(i.player_portrait);
                // `social-panels.md` TR14 (§4.3, SP27): Tessera has no voice chat, so the
                // frame's voice, squelched, sending-data and assistant icons, which the
                // layout shows at (71, 11) over the start of `PlayerName`, are hidden
                // whenever the frame is built (the `combat-ui.md` §4.1 OPEN).
                hidden = [
                    i.player_voice,
                    i.player_voice_squelched,
                    i.player_sending,
                    i.player_assistant,
                ]
                .into_iter()
                .filter_map(f)
                .collect();
            }
            FrameKind::Target => {
                p.name = Some(f(i.opponent_name)?);
                p.alt_name = f(i.inanimate_name);
                p.level = f(i.opponent_level);
                p.skull = f(i.opponent_skull);
                p.health = f(i.opponent_health_meter);
                p.power_field = f(i.opponent_power_field);
                p.power =
                    f(i.opponent_power_meter).or_else(|| first_meter_below(ui, p.power_field));
                p.zero_meters = [i.opponent_temp_health, i.opponent_temp_power]
                    .into_iter()
                    .filter_map(f)
                    .collect();
                p.texts = [
                    (i.opponent_health_text, true),
                    (i.opponent_power_text, false),
                ]
                .into_iter()
                .filter_map(|(id, h)| f(id).map(|k| (k, h)))
                .collect();
                p.vital_area = f(i.opponent_vitals);
                p.vital_field = f(i.opponent_display);
                p.nonvital_field = f(i.inanimate_field);
                p.decoration = [i.opponent_bg, i.opponent_animations, i.opponent_level_bg]
                    .into_iter()
                    .filter_map(f)
                    .collect();
                p.class = f(i.opponent_class);
                p.portrait = f(i.opponent_portrait);
            }
            FrameKind::SelectionTarget => {
                p.name = Some(f(i.selection_target_name)?);
                p.vital_field = f(i.selection_target_vitals);
                // `MicroVitalFieldBase` opens with background meters that have no fill;
                // the current-health meter is found by its label (§5.7 OPEN), else the
                // first meter with a child (a fill).
                let below = |id: u32| p.vital_field.and_then(|v| find_below(ui, v, id));
                p.health = below(i.selection_target_health).or_else(|| {
                    p.vital_field.and_then(|v| {
                        subtree(ui, v).into_iter().find(|c| {
                            ui.class(*c) == Some(WidgetClass::Meter) && !ui.children(*c).is_empty()
                        })
                    })
                });
                p.zero_meters = [i.selection_target_temp_health, i.selection_target_dread]
                    .into_iter()
                    .filter_map(below)
                    .collect();
                p.nonvital_field = f(i.selection_target_nonvital);
            }
        }
        for k in hidden {
            ui.set_shown(k, false);
        }
        // `ui-widgets.md` §12.4, `ui-layout.md` §17.4: the portrait shows the figure when the
        // client draws a scene into it, otherwise nothing; so the element (with its own
        // image and `Target_Image`) starts hidden until the client gives it the scene as
        // external content ([`UnitFrame::set_portrait_content`], driven from
        // [`super::CombatUi::portraits`]).
        if let Some(k) = p.portrait {
            ui.set_shown(k, false);
        }
        // Until the frame has something to show, its texts are empty rather than the
        // layout's design-time samples (TR16; the name and level are set by `show`).
        for k in [p.name, p.alt_name, p.level].into_iter().flatten() {
            ui.set_text(k, Some(String::new()));
        }
        Some(Self {
            kind,
            cv: *cv,
            look: FrameLook::Install(Box::new(p)),
            shown: None,
            portrait_content: std::cell::Cell::new(None),
        })
    }

    /// The fallback frame of §13.1, created under `hud` (the [`look::Hud`] root, reference
    /// pixels). `None` when the elements cannot be created (element cap).
    pub fn fallback(
        ui: &mut Ui,
        cv: &CombatVocab,
        kind: FrameKind,
        hud: ElementKey,
    ) -> Option<Self> {
        let tp = cv.text;
        const PANEL_FILL: u32 = 0xC020_2020;
        const PANEL_BORDER: u32 = 0xFF80_8080;
        let (origin, size) = match kind {
            FrameKind::Player => ((8, 8), (240, 64)),
            FrameKind::Target => ((264, 8), (240, 64)),
            FrameKind::SelectionTarget => ((264, 124), (120, 32)),
        };
        let root = look::element(
            ui,
            hud,
            WidgetClass::Field,
            IBox::from_xywh(origin.0, origin.1, size.0, size.1),
            look::LOOK_ELEMENT_ID,
        )?;
        look::boxed(
            ui,
            root,
            IBox::from_xywh(0, 0, size.0, size.1),
            Some(PANEL_FILL),
            Some(PANEL_BORDER),
        );
        let vital = look::element(
            ui,
            root,
            WidgetClass::Field,
            IBox::from_xywh(0, 0, size.0, size.1),
            look::LOOK_ELEMENT_ID,
        )?;
        // Reference positions relative to the panel.
        let at = |x: i32, y: i32, w: i32, h: i32| IBox::from_xywh(x - origin.0, y - origin.1, w, h);
        let white = 0xFFFF_FFFF;
        let mut p = FallbackParts {
            root,
            vital,
            name: None,
            level: None,
            health: None,
            power: None,
            resource: None,
            nonvital: None,
            nonvital_name: None,
        };
        match kind {
            FrameKind::Player | FrameKind::Target => {
                let (lx, nx) = if kind == FrameKind::Player {
                    (12, 46)
                } else {
                    (472, 270)
                };
                look::boxed(ui, vital, at(lx, 12, 28, 28), None, Some(0xFFC0_A060));
                p.level = look::text(
                    ui,
                    &tp,
                    vital,
                    at(lx, 12, 28, 28),
                    "",
                    TextStyle::new(14, white).centred(),
                );
                p.name = look::text(
                    ui,
                    &tp,
                    vital,
                    at(nx, 12, 190, 16),
                    "",
                    TextStyle::new(14, white),
                );
                let from_right = kind == FrameKind::Target;
                p.health = Bar::new(
                    ui,
                    vital,
                    at(nx, 32, 190, 10),
                    0xFF40_1010,
                    0xFF30_C030,
                    from_right,
                );
                p.power = Bar::new(
                    ui,
                    vital,
                    at(nx, 46, 190, 8),
                    0xFF10_1040,
                    0xFF30_60E0,
                    from_right,
                );
                if kind == FrameKind::Player {
                    p.resource = Bar::new(
                        ui,
                        vital,
                        at(46, 58, 190, 4),
                        0xFF30_3010,
                        0xFFE0_C030,
                        false,
                    );
                } else {
                    let nv = look::element(
                        ui,
                        root,
                        WidgetClass::Field,
                        IBox::from_xywh(0, 0, size.0, size.1),
                        look::LOOK_ELEMENT_ID,
                    )?;
                    p.nonvital_name = look::text(
                        ui,
                        &tp,
                        nv,
                        at(264, 23, 240, 18),
                        "",
                        TextStyle::new(16, white).centred(),
                    );
                    ui.set_shown(nv, false);
                    p.nonvital = Some(nv);
                }
            }
            FrameKind::SelectionTarget => {
                p.name = look::text(
                    ui,
                    &tp,
                    vital,
                    at(268, 126, 112, 14),
                    "",
                    TextStyle::new(12, white),
                );
                p.health = Bar::new(
                    ui,
                    vital,
                    at(268, 144, 112, 6),
                    0xFF40_1010,
                    0xFF30_C030,
                    false,
                );
                // Without vitals (TR12, TR18): the panel box and the name at the same
                // place, no health bar (Tessera's look, §13.1 OPEN).
                let nv = look::element(
                    ui,
                    root,
                    WidgetClass::Field,
                    IBox::from_xywh(0, 0, size.0, size.1),
                    look::LOOK_ELEMENT_ID,
                )?;
                p.nonvital_name = look::text(
                    ui,
                    &tp,
                    nv,
                    at(268, 126, 112, 14),
                    "",
                    TextStyle::new(12, white),
                );
                ui.set_shown(nv, false);
                p.nonvital = Some(nv);
            }
        }
        if kind != FrameKind::Player {
            ui.set_shown(root, false);
        }
        Some(Self {
            kind,
            cv: *cv,
            look: FrameLook::Fallback(Box::new(p)),
            shown: None,
            portrait_content: std::cell::Cell::new(None),
        })
    }

    /// Which frame this is.
    #[must_use]
    pub fn kind(&self) -> FrameKind {
        self.kind
    }

    /// Whether the frame uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, FrameLook::Fallback(_))
    }

    /// The frame's root element.
    #[must_use]
    pub fn root(&self) -> ElementKey {
        match &self.look {
            FrameLook::Install(p) => p.root,
            FrameLook::Fallback(p) => p.root,
        }
    }

    /// The portrait element (`PlayerPortrait` of E5, `OpponentPortrait` of E6), install
    /// look only.
    #[must_use]
    pub fn portrait(&self) -> Option<ElementKey> {
        match &self.look {
            FrameLook::Install(p) => p.portrait,
            FrameLook::Fallback(_) => None,
        }
    }

    /// Whether the client draws a portrait scene into the portrait (`ui-widgets.md` §12.4).
    /// Superseded by [`UnitFrame::set_portrait_content`], which gives the element its scene
    /// as external content; this one sets no content. `false` is
    /// `set_portrait_content(None)` (hidden with its media and children). `true` keeps the
    /// content last given and shows the element; called before any content was given it
    /// still shows the element (kept for the widget tests), which composes the element's own
    /// media without a scene (`combat-ui.md` §5.1), so the client never calls it.
    pub fn set_portrait_scene(&self, ui: &mut Ui, drawn: bool) {
        match (drawn, self.portrait_content.get()) {
            (false, _) => self.set_portrait_content(ui, None),
            (true, Some(Some(id))) => self.set_portrait_content(ui, Some(id)),
            (true, _) => {
                if let Some(k) = self.portrait() {
                    ui.set_shown(k, true);
                }
            }
        }
    }

    /// The fallback look's class-resource bar (its back element; §13.1, §13.4), `None` in
    /// the install look and for frames other than the player's.
    #[must_use]
    pub fn fallback_resource(&self) -> Option<ElementKey> {
        match &self.look {
            FrameLook::Fallback(p) => p.resource.as_ref().map(|r| r.back),
            FrameLook::Install(_) => None,
        }
    }

    /// The text elements that show the frame's name and level now (`combat-ui.md` §4.1,
    /// §5.1, §13.1): the name of the vital or the non-vital field, by the shown view; the
    /// level element (E5, E6; `None` for E7 and in the non-vital field).
    #[must_use]
    pub fn name_and_level(&self) -> (Option<ElementKey>, Option<ElementKey>) {
        let vitals = self
            .shown
            .as_ref()
            .is_none_or(|v| v.has_vitals || self.kind == FrameKind::Player);
        match &self.look {
            FrameLook::Install(p) => {
                if vitals {
                    (p.name, p.level)
                } else {
                    (p.alt_name.or(p.name), None)
                }
            }
            FrameLook::Fallback(p) => {
                if vitals {
                    (p.name, p.level)
                } else {
                    (p.nonvital_name, None)
                }
            }
        }
    }

    /// What the frame shows (`None` while hidden).
    #[must_use]
    pub fn view(&self) -> Option<&FrameView> {
        self.shown.as_ref()
    }

    /// Shows `view` (or hides the frame for `None`; the player frame is never hidden by
    /// this, TR17). Applies TR9–TR16 and §5.2's field choice.
    pub fn show(&mut self, ui: &mut Ui, view: Option<&FrameView>) {
        let Some(v) = view else {
            if self.kind != FrameKind::Player {
                ui.set_shown(self.root(), false);
            }
            self.shown = None;
            return;
        };
        ui.set_shown(self.root(), true);
        match &self.look {
            FrameLook::Install(p) => self.show_install(ui, p, v),
            FrameLook::Fallback(p) => self.show_fallback(ui, p, v),
        }
        self.shown = Some(v.clone());
    }

    fn show_install(&self, ui: &mut Ui, p: &InstallParts, v: &FrameView) {
        let cv = &self.cv;
        let vitals = v.has_vitals || self.kind == FrameKind::Player;
        crate::game::show(ui, p.vital_field, vitals);
        crate::game::show(ui, p.nonvital_field, !vitals);
        let name_el = if vitals {
            p.name
        } else {
            p.alt_name.or(p.name)
        };
        if let Some(n) = name_el {
            ui.set_text(n, Some(super::data::cap_label(&v.name)));
            if self.kind != FrameKind::Player
                && let Some(c) = v.name_colour
            {
                crate::game::switch_if_defined(ui, n, name_state(cv, c));
            }
        }
        // Level (TR16, §5.2 step 4).
        let (level_on, skull_on) = match v.level {
            LevelShown::Level(_) | LevelShown::Nothing => (true, false),
            LevelShown::Skull => (false, true),
            LevelShown::Hidden => (false, false),
        };
        if let Some(l) = p.level {
            // Without a level the element shows nothing (TR16), not the layout's
            // design-time text.
            ui.set_text(l, Some(level_text(v.level).unwrap_or_default()));
            ui.set_shown(l, level_on);
        }
        crate::game::show(ui, p.skull, skull_on);
        // Decoration (TR9, TR13).
        let deco = v.decoration.unwrap_or(match self.kind {
            FrameKind::Player => cv.states.normal,
            _ => cv.states.monster_normal,
        });
        for d in &p.decoration {
            crate::game::switch_if_defined(ui, *d, deco);
        }
        // Class (§5.2 step 5).
        if let Some(c) = p.class {
            match v.class_display {
                Some(t) => {
                    ui.set_property(c, cv.pids.class_display, PropertyValue::Enum(t as u32));
                    ui.set_shown(c, true);
                }
                None => ui.set_shown(c, false),
            }
        }
        // Meters (TR10, TR14).
        let meter = |ui: &mut Ui, k: Option<ElementKey>, p: f32| {
            if let Some(k) = k {
                let pid = ui.vocab().pids.meter_position;
                ui.set_property(k, pid, PropertyValue::Float(p));
            }
        };
        meter(ui, p.health, v.health.map_or(0.0, |h| h.fraction()));
        if let Some(h) = p.health
            && self.kind != FrameKind::Player
        {
            crate::game::switch_if_defined(ui, h, health_state(cv, v.health_colour));
        }
        for z in &p.zero_meters {
            meter(ui, Some(*z), 0.0);
        }
        let wrath = matches!(v.resource, Some(ClassResource::Wrath(_)));
        if self.kind == FrameKind::Player {
            crate::game::show(ui, p.power_field, !wrath);
            crate::game::show(ui, p.rage_field, wrath);
        }
        meter(ui, p.power, v.power.map_or(0.0, |h| h.fraction()));
        if let Some(ClassResource::Wrath(w)) = v.resource {
            meter(ui, p.rage, w.fraction());
        }
        for (t, health) in &p.texts {
            let val = if *health { v.health } else { v.power };
            ui.set_text(*t, Some(val.map(|x| x.text()).unwrap_or_default()));
        }
    }

    fn show_fallback(&self, ui: &mut Ui, p: &FallbackParts, v: &FrameView) {
        let vitals = v.has_vitals || self.kind == FrameKind::Player;
        ui.set_shown(p.vital, vitals);
        crate::game::show(ui, p.nonvital, !vitals);
        let colour = match (self.kind, v.name_colour) {
            (FrameKind::Player, _) | (_, None) => 0xFFFF_FFFF,
            (_, Some(c)) => fallback_name_colour(c),
        };
        let tp = self.cv.text;
        let name_el = if vitals { p.name } else { p.nonvital_name };
        if let Some(n) = name_el {
            ui.set_text(n, Some(super::data::cap_label(&v.name)));
            ui.set_property(
                n,
                tp.font_color,
                PropertyValue::Colour(colour.to_le_bytes()),
            );
        }
        if let Some(l) = p.level {
            ui.set_text(l, Some(level_text(v.level).unwrap_or_default()));
        }
        if let Some(h) = &p.health {
            h.set(ui, v.health.map_or(0.0, |x| x.fraction()));
            if self.kind != FrameKind::Player {
                h.set_colour(ui, fallback_health_colour(v.health_colour));
            }
        }
        if let Some(pw) = &p.power {
            pw.set(ui, v.power.map_or(0.0, |x| x.fraction()));
        }
        if let Some(r) = &p.resource {
            let val: Option<Vital> = match v.resource {
                Some(ClassResource::Wrath(x) | ClassResource::Plain(x)) => Some(x),
                Some(ClassResource::Pips { value, .. }) => Some(value),
                None => None,
            };
            ui.set_shown(r.back, val.is_some());
            r.set(ui, val.map_or(0.0, |x| x.fraction()));
        }
    }

    /// Per-frame work: the vital texts show while the pointer is over the vital field
    /// (TR10; install look only).
    pub fn update(&self, ui: &mut Ui) {
        let FrameLook::Install(p) = &self.look else {
            return;
        };
        let pointer = ui.pointer();
        let over = self.shown.is_some()
            && p.vital_area.is_some_and(|area| {
                ui.is_displayed(area)
                    && pointer.is_some_and(|(x, y)| ui.displayed_box(area).contains(x, y))
            });
        for (t, _) in &p.texts {
            ui.set_shown(*t, over);
        }
    }
}

/// Shows the P3 pip display of the resource (TR11): the `PipDisplay` element below
/// `toolbar` whose `UIComponent_PipDisplay_Type` equals the pip type is shown, the others
/// are hidden, and the shown one switches to the state of its list entry whose value
/// equals round(current) (the nearest lower value when none is equal; its default state
/// when none is lower). `pip_display_type` is the game type `PipDisplay`. Returns the shown
/// display.
///
/// The members of a `UIComponent_PipDisplay_List` entry are not named in the spec
/// (`combat-ui.md` §4.4 OPEN): Tessera reads an entry's first Int member as the value and
/// its first Enum member as the state.
pub fn show_pips(
    ui: &mut Ui,
    cv: &CombatVocab,
    toolbar: ElementKey,
    pip_display_type: u32,
    resource: Option<&ClassResource>,
) -> Option<ElementKey> {
    let want = match resource {
        Some(ClassResource::Pips { pip_type, value }) => Some((*pip_type, *value)),
        _ => None,
    };
    let mut shown = None;
    for k in subtree(ui, toolbar) {
        if ui.element_type(k) != Some(pip_display_type) {
            continue;
        }
        let ty = ui.prop(k, cv.pids.pip_type).key();
        match want {
            Some((t, value)) if t == ty && shown.is_none() => {
                ui.set_shown(k, true);
                if let Some(s) = pip_state(ui, cv, k, value.current) {
                    crate::game::switch_if_defined(ui, k, s);
                }
                shown = Some(k);
            }
            _ => ui.set_shown(k, false),
        }
    }
    shown
}

/// The state of a pip display's list entry for `current` (TR11).
fn pip_state(ui: &Ui, cv: &CombatVocab, k: ElementKey, current: f64) -> Option<u32> {
    let target = super::data::round_half_away(current);
    let list = ui.property(k, cv.pids.pip_list)?;
    let mut best: Option<(i64, u32)> = None;
    for e in array_elements(list) {
        let PropertyValue::Struct(s) = &e.value else {
            continue;
        };
        let mut value = None;
        let mut state = None;
        for m in &s.fields {
            match &m.value {
                PropertyValue::Int(_) if value.is_none() => {
                    value = Some(i64::from(as_int(&m.value)))
                }
                PropertyValue::Enum(_) if state.is_none() => state = Some(as_key(&m.value)),
                _ => {}
            }
        }
        let (Some(v), Some(st)) = (value, state) else {
            continue;
        };
        if v == target {
            return Some(st);
        }
        if v < target && best.is_none_or(|(b, _)| v > b) {
            best = Some((v, st));
        }
    }
    best.map(|(_, s)| s)
}
