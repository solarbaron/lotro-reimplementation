// SPDX-License-Identifier: GPL-3.0-or-later
//! The binding of an acceptance record (`official-entry.md` §5.6): the OS account, the
//! machine and the absolute, canonical path of the record's directory, read from the
//! operating system at check time and never written into the document.
//!
//! - **OS account.** The numeric user ID, as decimal text (Unix-like systems). Windows stores
//!   no record at all (§5.6 "Windows", see the consent module), so no binding is made there.
//! - **Machine.** Linux: the content of `/etc/machine-id` and of no other file, with one line
//!   feed at its end removed, when the file holds at most 128 bytes (read up to 128 bytes plus
//!   one) and is not empty without that line feed; when that file cannot be read, is empty or
//!   is longer, and on other Unix-like systems, the host name from `uname`.
//! - **Directory.** `std::fs::canonicalize` of the record directory, as the platform's
//!   encoded path bytes.
//!
//! The type has no constructor outside the crate (fuzzing and tests use
//! `consent::test_support`).

use std::path::Path;

/// The machine-ID file read on Linux (§5.6).
#[cfg(target_os = "linux")]
const MACHINE_ID_FILE: &str = "/etc/machine-id";

/// Most bytes of the machine-ID file read (it holds 33).
#[cfg(target_os = "linux")]
const MAX_MACHINE_ID: usize = 128;

/// The three binding values.
#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    user: Vec<u8>,
    machine: Vec<u8>,
    directory: Vec<u8>,
}

impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Binding")
            .field("user", &String::from_utf8_lossy(&self.user))
            .field("machine", &"<machine>")
            .field("directory", &String::from_utf8_lossy(&self.directory))
            .finish()
    }
}

/// Why no binding could be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    /// The record directory has no canonical path (it does not exist or cannot be read).
    #[error("the record directory has no canonical path")]
    Directory,
    /// This platform gives no OS-account identifier to Tessera (Windows, module docs).
    #[error("the OS account cannot be identified on this platform")]
    Account,
    /// Neither a machine identifier nor a host name could be read.
    #[error("the machine cannot be identified")]
    Machine,
}

impl Binding {
    /// A binding from given values (tests, fuzzing: binding values injected, OE-T5). The
    /// record check of the client always uses [`Binding::current`].
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub(crate) fn new(user: &[u8], machine: &[u8], directory: &[u8]) -> Self {
        Self {
            user: user.to_vec(),
            machine: machine.to_vec(),
            directory: directory.to_vec(),
        }
    }

    /// The binding of this OS user and machine for the record directory `dir`.
    ///
    /// # Errors
    ///
    /// [`BindingError`] when a value cannot be read.
    pub(crate) fn current(dir: &Path) -> Result<Self, BindingError> {
        let canonical = std::fs::canonicalize(dir).map_err(|_| BindingError::Directory)?;
        let directory = canonical.as_os_str().as_encoded_bytes().to_vec();
        let user = current_user()?;
        let machine = current_machine()?;
        Ok(Self {
            user,
            machine,
            directory,
        })
    }

    /// The same binding for another directory (the canonical path bytes `directory`).
    #[cfg(all(test, unix))] // only the Unix record tests use it
    #[must_use]
    pub(crate) fn with_directory(&self, directory: &[u8]) -> Self {
        Self {
            directory: directory.to_vec(),
            ..self.clone()
        }
    }

    pub(crate) fn user(&self) -> &[u8] {
        &self.user
    }

    pub(crate) fn machine(&self) -> &[u8] {
        &self.machine
    }

    pub(crate) fn directory(&self) -> &[u8] {
        &self.directory
    }
}

#[cfg(unix)]
fn current_user() -> Result<Vec<u8>, BindingError> {
    Ok(rustix::process::getuid().as_raw().to_string().into_bytes())
}

#[cfg(not(unix))]
fn current_user() -> Result<Vec<u8>, BindingError> {
    Err(BindingError::Account)
}

/// The machine identifier of the system whose root directory is `root` (`/` in the client;
/// tests inject another root, OE-T18): the content of `etc/machine-id` with one line feed at
/// its end removed, and of no other file; `None` when it cannot be read, is empty or the file
/// is longer than [`MAX_MACHINE_ID`] bytes (its line feed counted).
#[cfg(target_os = "linux")]
fn machine_id_in(root: &Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let file = std::fs::File::open(root.join(MACHINE_ID_FILE.trim_start_matches('/'))).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    // Up to the bound plus one byte: a file longer than the bound (its line feed counted)
    // falls back to the host name (§5.6, OE-T18).
    let mut buf = Vec::new();
    file.take(MAX_MACHINE_ID as u64 + 1)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.len() > MAX_MACHINE_ID {
        return None;
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
    }
    (!buf.is_empty()).then_some(buf)
}

#[cfg(target_os = "linux")]
fn machine_id() -> Option<Vec<u8>> {
    machine_id_in(Path::new("/"))
}

#[cfg(not(target_os = "linux"))]
fn machine_id() -> Option<Vec<u8>> {
    None
}

#[cfg(unix)]
fn host_name() -> Option<Vec<u8>> {
    let uname = rustix::system::uname();
    let name = uname.nodename().to_bytes();
    (!name.is_empty()).then(|| name.to_vec())
}

#[cfg(not(unix))]
fn host_name() -> Option<Vec<u8>> {
    None
}

fn current_machine() -> Result<Vec<u8>, BindingError> {
    machine_id().or_else(host_name).ok_or(BindingError::Machine)
}

/// The machine value of the binding for the system root `root` (tests, OE-T18).
#[cfg(all(test, target_os = "linux"))]
fn machine_for_root(root: &Path) -> Option<Vec<u8>> {
    machine_id_in(root).or_else(host_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn current_binding_is_readable_on_unix() {
        let dir = tempfile::tempdir().expect("temp dir");
        let b = Binding::current(dir.path()).expect("binding");
        assert!(!b.user().is_empty() && !b.machine().is_empty());
        let canon = std::fs::canonicalize(dir.path()).expect("canonical");
        assert_eq!(b.directory(), canon.as_os_str().as_encoded_bytes());
    }

    #[test]
    fn missing_directory_has_no_binding() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            Binding::current(&dir.path().join("absent")),
            Err(BindingError::Directory)
        );
    }

    /// OE-T18: the identifier comes from `etc/machine-id` only, else the host name.
    #[cfg(target_os = "linux")]
    #[test]
    fn oe_t18_linux_machine_identifier() {
        let root = tempfile::tempdir().expect("temp dir");
        let host = host_name().expect("host name");
        // Absent.
        assert_eq!(machine_for_root(root.path()), Some(host.clone()));
        std::fs::create_dir(root.path().join("etc")).expect("etc");
        // A file of another name is never read.
        std::fs::write(root.path().join("etc/hostid"), b"other\n").expect("write");
        std::fs::write(root.path().join("var-lib-dbus-machine-id"), b"other\n").expect("write");
        assert_eq!(machine_for_root(root.path()), Some(host.clone()));
        // Empty.
        std::fs::write(root.path().join("etc/machine-id"), b"").expect("write");
        assert_eq!(machine_for_root(root.path()), Some(host.clone()));
        std::fs::write(root.path().join("etc/machine-id"), b"\n").expect("write");
        assert_eq!(machine_for_root(root.path()), Some(host));
        // Present: its content with one final line feed removed.
        std::fs::write(root.path().join("etc/machine-id"), b"0123abcd\n").expect("write");
        assert_eq!(machine_for_root(root.path()), Some(b"0123abcd".to_vec()));
        std::fs::write(root.path().join("etc/machine-id"), b" id \n\n").expect("write");
        assert_eq!(machine_for_root(root.path()), Some(b" id \n".to_vec()));
        // A file of 128 bytes is used, one of 129 bytes is not (its line feed counted).
        let id = |n: usize, lf: bool| {
            let mut v = vec![b'a'; n];
            if lf {
                v.push(b'\n');
            }
            v
        };
        let p = root.path().join("etc/machine-id");
        std::fs::write(&p, id(128, false)).expect("write");
        assert_eq!(machine_for_root(root.path()), Some(id(128, false)));
        std::fs::write(&p, id(127, true)).expect("write");
        assert_eq!(machine_for_root(root.path()), Some(id(127, false)));
        std::fs::write(&p, id(129, false)).expect("write");
        assert_eq!(
            machine_for_root(root.path()),
            Some(host_name().expect("host"))
        );
        std::fs::write(&p, id(128, true)).expect("write");
        assert_eq!(
            machine_for_root(root.path()),
            Some(host_name().expect("host"))
        );
    }

    #[test]
    fn debug_hides_the_machine() {
        let b = Binding::new(b"1000", b"secret-machine", b"/x");
        assert!(!format!("{b:?}").contains("secret-machine"));
    }
}
