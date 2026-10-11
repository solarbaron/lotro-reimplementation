// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat-state display E17 with P3's root state, and the defeat panel E18
//! (`combat-ui.md` §10, TR37–TR39, §13.7).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::data::CombatAction;
use super::look::{self, TextStyle};
use super::vocab::CombatVocab;
use crate::game::find_below;

/// Tessera's own statement of the defeat panel (TR38).
pub const DEFEAT_STATEMENT: &str = "Your character was defeated.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum StateLook {
    Install {
        toolbar: ElementKey,
        indicator: Option<ElementKey>,
    },
    Fallback {
        indicator: ElementKey,
    },
}

/// The combat-state display (TR37).
#[derive(Debug, Clone, PartialEq)]
pub struct CombatStateDisplay {
    cv: CombatVocab,
    look: StateLook,
    in_combat: bool,
}

impl CombatStateDisplay {
    /// The install look: `toolbar` is P3's root (`ToolbarField`) or an element above it.
    /// `None` when `toolbar` is not and holds no `ToolbarField`.
    #[must_use]
    pub fn install(ui: &Ui, cv: &CombatVocab, toolbar: ElementKey) -> Option<Self> {
        let root = if ui.element_id(toolbar) == Some(cv.ids.toolbar_field) {
            toolbar
        } else {
            find_below(ui, toolbar, cv.ids.toolbar_field)?
        };
        Some(Self {
            cv: *cv,
            look: StateLook::Install {
                toolbar: root,
                indicator: find_below(ui, root, cv.ids.violent_indicator),
            },
            in_combat: false,
        })
    }

    /// The fallback look of §13.7: a 24 × 24 box at (500, 700) with a red border while in
    /// combat.
    pub fn fallback(ui: &mut Ui, cv: &CombatVocab, hud: ElementKey) -> Option<Self> {
        let indicator = look::boxed(
            ui,
            hud,
            IBox::from_xywh(500, 700, 24, 24),
            None,
            Some(0xFFFF_4040),
        )?;
        ui.set_shown(indicator, false);
        Some(Self {
            cv: *cv,
            look: StateLook::Fallback { indicator },
            in_combat: false,
        })
    }

    /// Whether the display uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, StateLook::Fallback { .. })
    }

    /// The indicator element, when it exists.
    #[must_use]
    pub fn indicator(&self) -> Option<ElementKey> {
        match &self.look {
            StateLook::Install { indicator, .. } => *indicator,
            StateLook::Fallback { indicator } => Some(*indicator),
        }
    }

    /// The last combat state.
    #[must_use]
    pub fn in_combat(&self) -> bool {
        self.in_combat
    }

    /// `CombatState { entity: player, in_combat }` (TR37, §10.1).
    pub fn set(&mut self, ui: &mut Ui, in_combat: bool) {
        self.in_combat = in_combat;
        let s = &self.cv.states;
        match &self.look {
            StateLook::Install { toolbar, indicator } => {
                let ts = if in_combat {
                    s.toolbar_combat
                } else {
                    s.toolbar_noncombat
                };
                crate::game::switch_if_defined(ui, *toolbar, ts);
                if let Some(i) = indicator {
                    let st = if in_combat {
                        s.combat_violent
                    } else {
                        s.combat_normal
                    };
                    crate::game::switch_if_defined(ui, *i, st);
                }
            }
            StateLook::Fallback { indicator } => ui.set_shown(*indicator, in_combat),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DefeatLook {
    Install { layout: Did },
    Fallback { hud: ElementKey },
}

/// The defeat panel E18 (TR38).
#[derive(Debug, Clone, PartialEq)]
pub struct DefeatPanel {
    cv: CombatVocab,
    look: DefeatLook,
    open: Option<ElementKey>,
    ok: Option<ElementKey>,
    ok_id: u32,
    regs: Vec<(u32, u32)>,
}

impl DefeatPanel {
    /// The install look: `Awaken_DialogMessageField` of the `Dialog` layout `layout`,
    /// opened on defeat.
    #[must_use]
    pub fn install(cv: &CombatVocab, layout: Did) -> Self {
        Self::with(cv, DefeatLook::Install { layout })
    }

    /// The fallback look of §13.7 under the HUD root.
    #[must_use]
    pub fn fallback(cv: &CombatVocab, hud: ElementKey) -> Self {
        Self::with(cv, DefeatLook::Fallback { hud })
    }

    fn with(cv: &CombatVocab, look: DefeatLook) -> Self {
        Self {
            cv: *cv,
            look,
            open: None,
            ok: None,
            ok_id: 0,
            regs: Vec::new(),
        }
    }

    /// Whether the panel uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, DefeatLook::Fallback { .. })
    }

    /// The open panel.
    #[must_use]
    pub fn panel(&self) -> Option<ElementKey> {
        self.open
    }

    /// `Defeated { entity: player }`: opens the panel (TR38). Without the install element
    /// the call opens nothing and returns `false`.
    pub fn open(&mut self, ui: &mut Ui) -> bool {
        if self.open.is_some_and(|k| ui.exists(k)) {
            return true;
        }
        let ok = match &self.look {
            DefeatLook::Install { layout } => self.open_install(ui, *layout),
            DefeatLook::Fallback { hud } => self.open_fallback(ui, *hud),
        };
        ok.is_some()
    }

    fn open_install(&mut self, ui: &mut Ui, layout: Did) -> Option<()> {
        let ids = self.cv.ids;
        let k = match ui.open_panel(layout, ids.awaken_dialog) {
            Ok(k) => k,
            Err(e) => {
                tracing::debug!("defeat panel not opened: {e}");
                return None;
            }
        };
        if let Some(t) = find_below(ui, k, ids.dialog_statement) {
            ui.set_text(t, Some(DEFEAT_STATEMENT.to_string()));
        }
        if let Some(r) = find_below(ui, k, ids.dialog_revive) {
            let g = ui.vocab().pids.button_ghosted;
            ui.set_property(r, g, PropertyValue::Bool(true));
        }
        for id in [ids.dialog_pay, ids.dialog_buy] {
            crate::game::show(ui, find_below(ui, k, id), false);
        }
        self.ok = find_below(ui, k, ids.dialog_ok);
        self.ok_id = ids.dialog_ok;
        let click = ui.vocab().msgs.button_click;
        self.listen(ui, ids.dialog_ok, click);
        self.open = Some(k);
        Some(())
    }

    fn open_fallback(&mut self, ui: &mut Ui, hud: ElementKey) -> Option<()> {
        let tp = self.cv.text;
        let root = look::boxed(
            ui,
            hud,
            IBox::from_xywh(256, 274, 512, 220),
            Some(0xE010_1828),
            Some(0xFF80_8080),
        )?;
        look::text(
            ui,
            &tp,
            root,
            IBox::from_xywh(16, 26, 480, 20),
            DEFEAT_STATEMENT,
            TextStyle::new(16, 0xFFFF_FFFF),
        );
        let button = |ui: &mut Ui, x: i32, label: &str, colour: u32, id: u32| {
            let b = look::element(
                ui,
                root,
                WidgetClass::Field,
                IBox::from_xywh(x - 256, 437 - 274, 170, 36),
                id,
            )?;
            look::boxed(
                ui,
                b,
                IBox::from_xywh(0, 0, 170, 36),
                Some(0xFF30_3848),
                Some(0xFF80_8080),
            );
            look::text(
                ui,
                &tp,
                b,
                IBox::from_xywh(0, 0, 170, 36),
                label,
                TextStyle::new(14, colour).centred(),
            );
            Some(b)
        };
        button(ui, 393, "Revive", 0xFF80_8080, look::LOOK_ELEMENT_ID);
        let ok_id = look::next_look_id();
        self.ok = button(ui, 577, "OK", 0xFFFF_FFFF, ok_id);
        self.ok_id = ok_id;
        let click = ui.vocab().msgs.mouse_click;
        self.listen(ui, ok_id, click);
        self.open = Some(root);
        Some(())
    }

    fn listen(&mut self, ui: &mut Ui, id: u32, msg: u32) {
        ui.register_handler(id, msg);
        self.regs.push((id, msg));
    }

    /// Closes the panel (`Revived`, or OK).
    pub fn close(&mut self, ui: &mut Ui) {
        for (id, m) in self.regs.drain(..) {
            ui.unregister_handler(id, m);
        }
        if let Some(k) = self.open.take() {
            ui.destroy(k);
        }
        self.ok = None;
    }

    /// The OK button closes the panel and gives [`CombatAction::DefeatAcknowledged`].
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<CombatAction> {
        let UiEvent::Game { message } = ev else {
            return None;
        };
        let (Some(_), Some(ok)) = (self.open, self.ok) else {
            return None;
        };
        if message.source_id != self.ok_id || !ui.is_ancestor_or_self(ok, message.source) {
            return None;
        }
        let ms = ui.vocab().msgs;
        if message.msg != ms.button_click && message.msg != ms.mouse_click {
            return None;
        }
        self.close(ui);
        Some(CombatAction::DefeatAcknowledged)
    }
}
