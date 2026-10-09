// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI side of text elements: run-time text, shaping through the client
//! ([`crate::env::UiEnv::shape_text`]) and the truncated-text tooltip
//! (`ui-behaviour.md` §10).

use std::sync::Arc;

use tessera_formats::property::PropertyValue;

use crate::env::{PreparedText, TextRequest};
use crate::events::MsgArg;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::tessera_msg;

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
