// SPDX-License-Identifier: GPL-3.0-or-later
//! The account form (`docs/specs/official-entry.md` §7).
//!
//! The account name and the password live only in [`SecretBuffer`]s of fixed capacity
//! (128 and 256 bytes), allocated when the form opens, so typing never reallocates and
//! leaves no copies behind. Both are overwritten with zeros (best effort) when the form is
//! left by Back, Escape or closing the window, and right after Log in has moved their
//! contents into a [`Credentials`] value for the compatibility backend. The masked entry's
//! text, layout and drawing see only bullets (U+2022, one per character); copy and cut do
//! nothing and there is no "show password" control; the form keeps no undo history; and
//! input-method composition is off for the whole screen (the window handler never enables
//! it).
//!
//! Tessera engineering note: the client has no clipboard access (no clipboard crate), so
//! paste into the masked entry (§7.2, for password managers) is not available yet; managers
//! that type the password work, since typed text goes straight into the secret buffer.

use std::time::Instant;

use tessera_compat::Credentials;
use tessera_compat::credentials::{MAX_ACCOUNT_BYTES, MAX_PASSWORD_BYTES};

use super::input::{DeviceInput, DeviceInputKind, Key};

/// The bullet the masked entry shows per character (§7.1).
pub const BULLET: char = '\u{2022}';

/// A UTF-8 text in a fixed-capacity buffer that is overwritten with zeros when cleared or
/// dropped. Its `Debug` form prints nothing of the text.
pub struct SecretBuffer<const N: usize> {
    bytes: Box<[u8; N]>,
    len: usize,
}

impl<const N: usize> std::fmt::Debug for SecretBuffer<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretBuffer(<redacted>)")
    }
}

impl<const N: usize> Default for SecretBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> SecretBuffer<N> {
    /// An empty buffer of capacity `N` bytes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes: Box::new([0u8; N]),
            len: 0,
        }
    }

    /// The text's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Characters in the text.
    #[must_use]
    pub fn char_count(&self) -> usize {
        std::str::from_utf8(self.as_bytes()).map_or(0, |s| s.chars().count())
    }

    /// Appends the characters of `text` that fit, whole characters only; characters `keep`
    /// refuses are skipped. Returns whether anything was added.
    pub fn push_str(&mut self, text: &str, keep: impl Fn(char) -> bool) -> bool {
        let mut added = false;
        for c in text.chars().filter(|c| keep(*c)) {
            let mut tmp = [0u8; 4];
            let enc = c.encode_utf8(&mut tmp).as_bytes();
            let end = self.len + enc.len();
            let Some(dst) = self.bytes.get_mut(self.len..end) else {
                break;
            };
            dst.copy_from_slice(enc);
            self.len = end;
            added = true;
            tmp.fill(0);
        }
        added
    }

    /// Removes the last character.
    pub fn pop_char(&mut self) {
        while self.len > 0 {
            self.len -= 1;
            let b = self.bytes.get(self.len).copied().unwrap_or(0);
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = 0;
            }
            if b & 0xC0 != 0x80 {
                break;
            }
        }
    }

    /// Overwrites the whole buffer with zeros and empties it.
    pub fn clear(&mut self) {
        self.bytes.fill(0);
        std::hint::black_box(&self.bytes);
        self.len = 0;
    }

    /// A copy in another buffer of the same capacity.
    #[must_use]
    pub fn copy(&self) -> Self {
        let mut c = Self::new();
        if let (Some(dst), Some(src)) = (c.bytes.get_mut(..self.len), self.bytes.get(..self.len)) {
            dst.copy_from_slice(src);
            c.len = self.len;
        }
        c
    }

    /// Every byte of the buffer, used or not (tests: the buffer reads back as zeros).
    #[cfg(test)]
    pub(crate) fn raw(&self) -> &[u8; N] {
        &self.bytes
    }
}

impl<const N: usize> Drop for SecretBuffer<N> {
    fn drop(&mut self) {
        self.clear();
    }
}

/// The account name buffer.
pub type AccountBuffer = SecretBuffer<MAX_ACCOUNT_BYTES>;
/// The password buffer.
pub type PasswordBuffer = SecretBuffer<MAX_PASSWORD_BYTES>;

/// A field of the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormField {
    /// "Account name".
    Account,
    /// "Password" (masked).
    Password,
}

/// A control of the form under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormControl {
    /// A field.
    Field(FormField),
    /// "Log in".
    LogIn,
    /// "Back".
    Back,
}

/// What the form asks the flow to do.
#[derive(Debug)]
pub enum FormAction {
    /// Log in with these credentials; `account` is a copy of the account name, kept only to
    /// refill the form after `BadCredentials` (§8.6).
    LogIn {
        /// The credentials for the backend.
        credentials: Credentials,
        /// The account name, for `BadCredentials`.
        account: AccountBuffer,
    },
    /// Back to the start screen (both fields cleared).
    Back,
}

/// The account form's state (module docs).
#[derive(Debug)]
pub struct AccountForm {
    account: AccountBuffer,
    password: PasswordBuffer,
    focus: FormField,
    /// Log in stays disabled until then (§8.6 retry waits).
    blocked_until: Option<Instant>,
    /// The wait is a `RateLimited` one (the form shows the countdown).
    rate_limited: bool,
    pressed_on: Option<FormControl>,
    pointer: (f64, f64),
}

impl AccountForm {
    /// A form with both fields empty (§7.1: every time).
    #[must_use]
    pub fn new() -> Self {
        Self {
            account: AccountBuffer::new(),
            password: PasswordBuffer::new(),
            focus: FormField::Account,
            blocked_until: None,
            rate_limited: false,
            pressed_on: None,
            pointer: (-1.0, -1.0),
        }
    }

    /// A form refilled with `account` (after `BadCredentials`), password empty and focused.
    #[must_use]
    pub fn with_account(account: &AccountBuffer) -> Self {
        let mut f = Self::new();
        f.account = account.copy();
        f.focus = FormField::Password;
        f
    }

    /// Log in stays disabled until `until`; `rate_limited` shows the countdown.
    pub fn block_until(&mut self, until: Instant, rate_limited: bool) {
        self.blocked_until = Some(until);
        self.rate_limited = rate_limited;
    }

    /// The focused field.
    #[must_use]
    pub fn focus(&self) -> FormField {
        self.focus
    }

    /// The account name as shown (it is not masked).
    #[must_use]
    pub fn account_text(&self) -> &str {
        std::str::from_utf8(self.account.as_bytes()).unwrap_or("")
    }

    /// The masked entry's text: one bullet per character, nothing else (§7.2).
    #[must_use]
    pub fn password_text(&self) -> String {
        std::iter::repeat_n(BULLET, self.password.char_count()).collect()
    }

    /// Seconds left of a `RateLimited` wait at `now` (for the countdown), if one runs.
    #[must_use]
    pub fn rate_limit_left(&self, now: Instant) -> Option<u64> {
        let until = self.blocked_until?;
        (self.rate_limited && until > now).then(|| {
            let left = until.duration_since(now);
            left.as_secs() + u64::from(left.subsec_nanos() > 0)
        })
    }

    /// Whether Log in is enabled at `now` (§7.1: both fields non-empty, no retry wait).
    #[must_use]
    pub fn login_enabled(&self, now: Instant) -> bool {
        !self.account.is_empty()
            && !self.password.is_empty()
            && self.blocked_until.is_none_or(|t| now >= t)
    }

    /// Clears both fields (Back, Escape, closing the window).
    pub fn clear(&mut self) {
        self.account.clear();
        self.password.clear();
    }

    fn type_text(&mut self, text: &str) {
        match self.focus {
            FormField::Account => {
                self.account.push_str(text, |c| !c.is_control());
            }
            FormField::Password => {
                self.password.push_str(text, |c| !c.is_control());
            }
        }
    }

    fn log_in(&mut self, now: Instant) -> Option<FormAction> {
        if !self.login_enabled(now) {
            return None;
        }
        let credentials = Credentials::new(self.account.as_bytes(), self.password.as_bytes());
        let account = self.account.copy();
        // The contents moved on: both buffers are overwritten now (§7.3).
        self.clear();
        match credentials {
            Ok(credentials) => Some(FormAction::LogIn {
                credentials,
                account,
            }),
            Err(e) => {
                tracing::info!("official login: {e}");
                None
            }
        }
    }

    /// One device-input event; `hit` names the form control under a window position.
    pub fn input(
        &mut self,
        ev: &DeviceInput,
        hit: &dyn Fn(f64, f64) -> Option<FormControl>,
    ) -> Option<FormAction> {
        match ev.kind() {
            DeviceInputKind::CloseRequested => {
                self.clear();
                return Some(FormAction::Back);
            }
            DeviceInputKind::Key {
                key,
                pressed: true,
                held_before: false,
                modifiers,
                text,
                ..
            } => match key {
                Key::Escape => {
                    self.clear();
                    return Some(FormAction::Back);
                }
                Key::Enter => return self.log_in(ev.at()),
                Key::Tab => {
                    self.focus = match self.focus {
                        FormField::Account => FormField::Password,
                        FormField::Password => FormField::Account,
                    };
                }
                Key::Backspace => match self.focus {
                    FormField::Account => self.account.pop_char(),
                    FormField::Password => self.password.pop_char(),
                },
                // Shortcuts (Control without Alt, or the logo key: copy, cut, paste, ...) type
                // nothing (module docs); Control with Alt is AltGr on Windows, whose
                // characters (`@`, `€`, ...) are typed.
                _ if modifiers.logo || (modifiers.ctrl && !modifiers.alt) => {}
                _ => {
                    if let Some(t) = text {
                        self.type_text(t.as_str());
                    }
                }
            },
            DeviceInputKind::PointerMoved { x, y } => self.pointer = (*x, *y),
            DeviceInputKind::PointerButton { pressed: true } => {
                self.pressed_on = hit(self.pointer.0, self.pointer.1);
                if let Some(FormControl::Field(f)) = self.pressed_on {
                    self.focus = f;
                }
            }
            DeviceInputKind::PointerButton { pressed: false } => {
                let started = self.pressed_on.take();
                if started.is_some() && started == hit(self.pointer.0, self.pointer.1) {
                    match started {
                        Some(FormControl::LogIn) => return self.log_in(ev.at()),
                        Some(FormControl::Back) => {
                            self.clear();
                            return Some(FormAction::Back);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Both buffers' bytes, used or not (tests).
    #[cfg(test)]
    pub(crate) fn raw_buffers(&self) -> (&[u8], &[u8]) {
        (self.account.raw(), self.password.raw())
    }
}

impl Default for AccountForm {
    fn default() -> Self {
        Self::new()
    }
}
