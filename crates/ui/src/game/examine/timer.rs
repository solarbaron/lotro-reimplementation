// SPDX-License-Identifier: GPL-3.0-or-later
//! Live timer rows of skill and effect tooltips (`skill-tooltips.md` §10.2, §12.2): while a
//! tooltip shows, its recovery-time and effect-time rows count down; only their text
//! changes (the tooltip is not rebuilt and does not follow the pointer). The text comes
//! from the client's tooltip model (`tessera-gamedata`'s timer rows), through
//! [`TimerText`]; this module only writes it into the row elements.

use crate::tree::ElementKey;
use crate::ui::Ui;

use super::panel::{BuiltTooltip, ExamTooltips};

/// Most timer rows one tooltip tracks (Tessera cap; the skill layout has one, the effect
/// layout one).
pub const MAX_TIMERS: usize = 16;

/// The client's timer texts.
pub trait TimerText {
    /// The text of timer `timer` (the client's index) at `now`; `None` empties the row.
    fn timer_text(&mut self, timer: usize, now: f64) -> Option<String>;
}

impl<F: FnMut(usize, f64) -> Option<String>> TimerText for F {
    fn timer_text(&mut self, timer: usize, now: f64) -> Option<String> {
        self(timer, now)
    }
}

/// The timer rows of one shown tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTimers {
    frame: ElementKey,
    subject: Option<u64>,
    rows: Vec<(usize, ElementKey)>,
}

impl LiveTimers {
    /// Tracks the timer rows of `built`: `timers` pairs a row template index (as
    /// [`super::ExamTooltip::rows`]) with the client's timer index. A row that §5 removed
    /// is skipped. `subject` names what the tooltip examines (an effect instance), for
    /// [`LiveTimers::hide_if`].
    #[must_use]
    pub fn new(
        ui: &Ui,
        built: &BuiltTooltip,
        timers: &[(usize, usize)],
        subject: Option<u64>,
    ) -> Self {
        let rows = timers
            .iter()
            .take(MAX_TIMERS)
            .filter_map(|(row, timer)| {
                let k = built.by_template.get(*row).copied().flatten()?;
                let t = ExamTooltips::text_target(ui, k)?;
                Some((*timer, t))
            })
            .collect();
        Self {
            frame: built.frame,
            subject,
            rows,
        }
    }

    /// The tooltip frame.
    #[must_use]
    pub fn frame(&self) -> ElementKey {
        self.frame
    }

    /// The number of rows tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no row is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Whether the tooltip still exists.
    #[must_use]
    pub fn alive(&self, ui: &Ui) -> bool {
        ui.exists(self.frame)
    }

    /// Writes each row's text at `now`. Returns whether the tooltip still exists (the
    /// client drops the tracker otherwise).
    pub fn update(&self, ui: &mut Ui, now: f64, src: &mut dyn TimerText) -> bool {
        if !self.alive(ui) {
            return false;
        }
        for (timer, k) in &self.rows {
            let text = src.timer_text(*timer, now);
            ui.set_text(*k, text.map(|t| super::super::combat::data::cap_label(&t)));
        }
        true
    }

    /// Hides the tooltip when it examines `subject` and is the UI's showing tooltip (an
    /// `EffectRemoved` hides an open tooltip of the instance with its icon, §13). Returns
    /// whether it was hidden.
    pub fn hide_if(&self, ui: &mut Ui, subject: u64) -> bool {
        if self.subject == Some(subject) && ui.tooltip_panel() == Some(self.frame) {
            ui.hide_tooltip();
            return true;
        }
        false
    }
}
