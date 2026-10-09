// SPDX-License-Identifier: GPL-3.0-or-later
//! Strokes and chords: the digital half of a binding
//! (`docs/specs/keymap.md` §5).
//!
//! * A **stroke** is a set of modifiers plus a set of up to
//!   [`MAX_STROKE_BUTTONS`] buttons that are held together, written
//!   `Ctrl+Shift+K` or `MouseLeft+MouseRight`. Button order does not matter.
//!   A stroke with modifiers only (`Shift`) is a *modifier hold*.
//! * A **chord** is a sequence of 1 to [`MAX_CHORD_STROKES`] strokes typed
//!   one after the other, separated by whitespace: `G G`, `Ctrl+K Ctrl+C`.

use std::fmt;

use crate::device::{Button, Modifiers};
use crate::error::{InputError, Result};

/// Most buttons in one stroke.
pub const MAX_STROKE_BUTTONS: usize = 4;
/// Most strokes in one chord.
pub const MAX_CHORD_STROKES: usize = 4;
/// Longest binding text accepted.
pub const MAX_CHORD_TEXT: usize = 256;

/// Buttons held together plus required modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Stroke {
    mods: Modifiers,
    /// Sorted, deduplicated, at most [`MAX_STROKE_BUTTONS`].
    buttons: Vec<Button>,
}

impl Stroke {
    /// A stroke; buttons are sorted and deduplicated.
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`] if there are more than
    /// [`MAX_STROKE_BUTTONS`] distinct buttons, neither buttons nor
    /// modifiers, or a button without a keymap name
    /// ([`Button::name`] is `None`: such a binding could not be saved).
    pub fn new(mods: Modifiers, buttons: impl IntoIterator<Item = Button>) -> Result<Self> {
        let mut list = Vec::new();
        for b in buttons {
            if b.name().is_none() {
                return Err(bad("stroke", "button has no keymap name"));
            }
            if !list.contains(&b) {
                if list.len() == MAX_STROKE_BUTTONS {
                    return Err(bad("stroke", "too many buttons in one stroke"));
                }
                list.push(b);
            }
        }
        list.sort_unstable();
        if list.is_empty() && mods.is_empty() {
            return Err(bad("stroke", "empty stroke"));
        }
        Ok(Self {
            mods,
            buttons: list,
        })
    }

    /// A single button with no modifiers. A button without a keymap name
    /// is accepted here, but [`Keymap::set`](crate::Keymap::set) refuses a
    /// binding that holds one.
    #[must_use]
    pub fn button(button: Button) -> Self {
        Self {
            mods: Modifiers::empty(),
            buttons: vec![button],
        }
    }

    /// Required modifiers.
    #[must_use]
    pub fn mods(&self) -> Modifiers {
        self.mods
    }

    /// The buttons, sorted.
    #[must_use]
    pub fn buttons(&self) -> &[Button] {
        &self.buttons
    }

    /// Whether this stroke has modifiers but no buttons.
    #[must_use]
    pub fn is_modifier_hold(&self) -> bool {
        self.buttons.is_empty()
    }

    /// Modifiers contributed by modifier *keys* that are themselves buttons
    /// of this stroke (`LShift+A` does not also require `Shift`).
    #[must_use]
    pub fn own_modifiers(&self) -> Modifiers {
        self.buttons
            .iter()
            .fold(Modifiers::empty(), |m, b| m | b.modifier())
    }

    /// Parse one stroke (no whitespace).
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`] on an unknown token, an empty token, too many
    /// buttons, or an empty stroke.
    pub fn parse(text: &str) -> Result<Self> {
        if text.is_empty() {
            return Err(bad(text, "empty stroke"));
        }
        let mut mods = Modifiers::empty();
        let mut buttons = Vec::new();
        for token in text.split('+') {
            if token.is_empty() {
                return Err(bad(
                    text,
                    "empty token (use `Plus`-free names such as `Equal`)",
                ));
            }
            if let Some(m) = Modifiers::from_token(token) {
                mods |= m;
            } else if let Some(b) = Button::from_name(token) {
                if !buttons.contains(&b) {
                    if buttons.len() == MAX_STROKE_BUTTONS {
                        return Err(bad(text, "too many buttons in one stroke"));
                    }
                    buttons.push(b);
                }
            } else {
                return Err(bad(text, &format!("unknown key or button {}", clip(token))));
            }
        }
        Self::new(mods, buttons).map_err(|_| bad(text, "invalid stroke"))
    }
}

impl fmt::Display for Stroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for t in self.mods.tokens() {
            if !first {
                f.write_str("+")?;
            }
            f.write_str(t)?;
            first = false;
        }
        for b in &self.buttons {
            if !first {
                f.write_str("+")?;
            }
            write!(f, "{b}")?;
            first = false;
        }
        Ok(())
    }
}

/// The activation filter of a digital binding entry, the `on` field of the
/// keymap file (`keymap.md` §5.3, §7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Activation {
    /// A hold from every resolved press to the release (the default).
    #[default]
    Press,
    /// A pulse at every release of a held button.
    Release,
    /// A pulse at a release at most half(*T*) after the button's last
    /// recorded press.
    Tap,
    /// A hold from a double press (at most *T* after the previous recorded
    /// press of the same button) to its release.
    Double,
    /// As [`Activation::Double`], and the press is nearby (§7.7).
    DoubleNearby,
}

impl Activation {
    /// Every value, in the §7.2.1 order within a class.
    pub const ALL: [Self; 5] = [
        Self::Press,
        Self::Double,
        Self::DoubleNearby,
        Self::Release,
        Self::Tap,
    ];

    /// The keymap-file spelling (`press`, `release`, `tap`, `double`,
    /// `double_nearby`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::Release => "release",
            Self::Tap => "tap",
            Self::Double => "double",
            Self::DoubleNearby => "double_nearby",
        }
    }

    /// Parse a keymap-file spelling, matched exactly (§5.3).
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == text)
    }

    /// Whether it fires at a release (`release`, `tap`) rather than at a
    /// press (§7.7 "Class").
    #[must_use]
    pub const fn is_release_class(self) -> bool {
        matches!(self, Self::Release | Self::Tap)
    }

    /// Rank inside one context at one raw event (§7.2.1): `press` before
    /// `double` before `double_nearby`; `release` before `tap`.
    #[must_use]
    pub(crate) const fn rank(self) -> u8 {
        match self {
            Self::Press | Self::Release => 0,
            Self::Double | Self::Tap => 1,
            Self::DoubleNearby => 2,
        }
    }
}

impl fmt::Display for Activation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A sequence of strokes, plus the activation filter of the binding entry it
/// belongs to (§5.3; [`Activation::Press`] unless set with
/// [`Chord::with_activation`]).
///
/// The text form ([`Chord::parse`], `Display`) holds the strokes only; the
/// keymap file writes a non-`press` activation as the `on` field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    strokes: Vec<Stroke>,
    on: Activation,
}

impl Chord {
    /// A chord of one stroke.
    #[must_use]
    pub fn single(stroke: Stroke) -> Self {
        Self {
            strokes: vec![stroke],
            on: Activation::Press,
        }
    }

    /// The activation filter (§5.3).
    #[must_use]
    pub fn activation(&self) -> Activation {
        self.on
    }

    /// The same chord with another activation filter.
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`] when `on` is not [`Activation::Press`] and the
    /// chord is not a single stroke with exactly one button that is not a
    /// wheel click (§5.3: such an entry is invalid).
    pub fn with_activation(mut self, on: Activation) -> Result<Self> {
        if on != Activation::Press {
            let one_button = match self.strokes.as_slice() {
                [s] => match s.buttons() {
                    [b] => !matches!(b, Button::Wheel(_)),
                    _ => false,
                },
                _ => false,
            };
            if !one_button {
                return Err(bad(
                    &self.to_string(),
                    "`on` other than \"press\" needs a single stroke with exactly one button \
                     that is not a wheel click",
                ));
            }
        }
        self.on = on;
        Ok(self)
    }

    /// A chord from strokes.
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`] when empty, longer than
    /// [`MAX_CHORD_STROKES`], or when a sequence contains a modifier hold.
    pub fn new(strokes: Vec<Stroke>) -> Result<Self> {
        if strokes.is_empty() {
            return Err(bad("", "empty binding"));
        }
        if strokes.len() > MAX_CHORD_STROKES {
            return Err(bad("", "too many strokes in a sequence"));
        }
        if strokes.len() > 1 && strokes.iter().any(Stroke::is_modifier_hold) {
            return Err(bad(
                "",
                "a modifier-only stroke cannot be part of a sequence",
            ));
        }
        Ok(Self {
            strokes,
            on: Activation::Press,
        })
    }

    /// The strokes in order.
    #[must_use]
    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    /// The first stroke.
    #[must_use]
    pub fn first(&self) -> &Stroke {
        &self.strokes[0]
    }

    /// The last stroke (the one that stays held for hold actions).
    #[must_use]
    pub fn last(&self) -> &Stroke {
        &self.strokes[self.strokes.len() - 1]
    }

    /// Parse binding text: strokes separated by ASCII whitespace.
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`] on any malformed stroke, text longer than
    /// [`MAX_CHORD_TEXT`], or the [`Chord::new`] rules.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > MAX_CHORD_TEXT {
            return Err(bad(text, "binding text too long"));
        }
        let mut strokes = Vec::new();
        for part in text.split_ascii_whitespace() {
            if strokes.len() == MAX_CHORD_STROKES {
                return Err(bad(text, "too many strokes in a sequence"));
            }
            strokes.push(Stroke::parse(part)?);
        }
        Self::new(strokes).map_err(|e| match e {
            InputError::Binding { reason, .. } => bad(text, &reason),
            other => other,
        })
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, s) in self.strokes.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{s}")?;
        }
        Ok(())
    }
}

fn clip(s: &str) -> String {
    let mut end = s.len().min(64);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{:?}", &s[..end])
}

pub(crate) fn bad(text: &str, reason: &str) -> InputError {
    let mut end = text.len().min(64);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    InputError::Binding {
        text: text[..end].to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{KeyCode, MouseButton};

    #[test]
    fn parse_and_display_normalize() {
        let c = Chord::parse("shift+k+ctrl").unwrap();
        assert_eq!(c.to_string(), "Ctrl+Shift+K");
        let c = Chord::parse("MouseRight+MouseLeft").unwrap();
        assert_eq!(c.to_string(), "MouseLeft+MouseRight");
        assert_eq!(
            c.first().buttons(),
            [
                Button::Mouse(MouseButton::Left),
                Button::Mouse(MouseButton::Right)
            ]
        );
        let c = Chord::parse("  Ctrl+K\tCtrl+C ").unwrap();
        assert_eq!(c.strokes().len(), 2);
        assert_eq!(c.to_string(), "Ctrl+K Ctrl+C");
        let c = Chord::parse("Shift").unwrap();
        assert!(c.first().is_modifier_hold());
        let c = Chord::parse("W+W").unwrap();
        assert_eq!(c.first().buttons(), [Button::Key(KeyCode::W)]);
    }

    #[test]
    fn rejects_malformed() {
        for t in [
            "",
            "   ",
            "Ctrl+",
            "+K",
            "K++L",
            "Bogus",
            "A+B+C+D+E",
            "A B C D E",
            "Shift G",
            "MouseX",
        ] {
            assert!(Chord::parse(t).is_err(), "{t:?} should fail");
        }
        assert!(Chord::parse(&"A ".repeat(200)).is_err());
    }

    #[test]
    fn unnamed_buttons_rejected() {
        let unnamed = Button::Mouse(MouseButton::Other(2));
        assert!(Stroke::new(Modifiers::empty(), [unnamed]).is_err());
        let named = Button::Mouse(MouseButton::Other(40));
        let s = Stroke::new(Modifiers::empty(), [named]).unwrap();
        assert_eq!(s.to_string(), "Mouse40");
        assert_eq!(Stroke::parse("Mouse40").unwrap(), s);
    }

    #[test]
    fn own_modifiers() {
        let s = Stroke::parse("LShift+A").unwrap();
        assert_eq!(s.own_modifiers(), Modifiers::SHIFT);
        assert_eq!(s.mods(), Modifiers::empty());
    }
}
