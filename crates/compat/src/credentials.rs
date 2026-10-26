// SPDX-License-Identifier: GPL-3.0-or-later
//! The account credentials handed to the compatibility backend (`official-entry.md` §7.3).
//!
//! [`Credentials`] has no `Clone`, no serialisation, and a `Debug` form that prints only
//! `<redacted>`; its bytes are overwritten with zeros when it is dropped (best effort, as the
//! test-server password type does). The backend takes it on its first use and drops it as
//! soon as the login reports success or failure, or the session ends.

/// Longest account name in bytes (§7.1).
pub const MAX_ACCOUNT_BYTES: usize = 128;
/// Longest password in bytes (§7.1).
pub const MAX_PASSWORD_BYTES: usize = 256;

/// Why credentials were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialsError {
    /// The account name is empty, longer than [`MAX_ACCOUNT_BYTES`], not UTF-8 or holds a
    /// control character.
    #[error("the account name is out of bounds")]
    Account,
    /// The password is empty, longer than [`MAX_PASSWORD_BYTES`] or not UTF-8.
    #[error("the password is out of bounds")]
    Password,
}

/// Bytes overwritten with zeros when dropped.
struct Secret(Box<[u8]>);

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&self.0);
    }
}

/// The user's account name and password, as typed (§7.1, §7.3).
pub struct Credentials {
    account: Secret,
    password: Secret,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl Credentials {
    /// Credentials from the account form's buffers (copied; the form overwrites its own).
    ///
    /// # Errors
    ///
    /// [`CredentialsError`] when a value is out of the bounds of §7.1.
    pub fn new(account: &[u8], password: &[u8]) -> Result<Self, CredentialsError> {
        let account_ok = (1..=MAX_ACCOUNT_BYTES).contains(&account.len())
            && std::str::from_utf8(account).is_ok_and(|s| !s.chars().any(char::is_control));
        if !account_ok {
            return Err(CredentialsError::Account);
        }
        let password_ok = (1..=MAX_PASSWORD_BYTES).contains(&password.len())
            && std::str::from_utf8(password).is_ok();
        if !password_ok {
            return Err(CredentialsError::Password);
        }
        Ok(Self {
            account: Secret(account.into()),
            password: Secret(password.into()),
        })
    }

    /// The lengths in bytes of the account name and the password (diagnostics never need
    /// more).
    #[must_use]
    pub fn lengths(&self) -> (usize, usize) {
        (self.account.0.len(), self.password.0.len())
    }

    /// The account name and the password as typed, for the one request of the live
    /// connector that carries them (`official-login.md` §3.4; §7.3 "One use"). Crate-private:
    /// nothing outside this crate reads them.
    pub(crate) fn expose(&self) -> (&str, &str) {
        // Both were checked to be UTF-8 when the value was made.
        (
            std::str::from_utf8(&self.account.0).unwrap_or_default(),
            std::str::from_utf8(&self.password.0).unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_and_redaction() {
        assert_eq!(
            Credentials::new(b"", b"pw").map(|_| ()),
            Err(CredentialsError::Account)
        );
        assert_eq!(
            Credentials::new(b"a\tb", b"pw").map(|_| ()),
            Err(CredentialsError::Account)
        );
        assert_eq!(
            Credentials::new(&[b'a'; 129], b"pw").map(|_| ()),
            Err(CredentialsError::Account)
        );
        assert_eq!(
            Credentials::new(b"name", &[b'p'; 257]).map(|_| ()),
            Err(CredentialsError::Password)
        );
        assert_eq!(
            Credentials::new(b"name", &[0xff]).map(|_| ()),
            Err(CredentialsError::Password)
        );
        let c = Credentials::new(b"canary-name", b"canary-pass").expect("valid");
        assert_eq!(format!("{c:?}"), "<redacted>");
        assert_eq!(c.lengths(), (11, 11));
    }
}
