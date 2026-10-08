// SPDX-License-Identifier: GPL-3.0-or-later
//! Accounts (`test-server.md` §3.4, §6): the accounts file, Argon2id password hashes in PHC
//! string form, the password policy, generated passwords, and the shared in-memory store
//! with reload-on-change.
//!
//! Passwords, hashes and salts are never logged (§11.2). Hash computations run only on the
//! hash workers and the command line, never on the network or simulation thread (§14).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use argon2::{Algorithm, Argon2, Params, Version};
use base64ct::{Base64Unpadded, Encoding};

use crate::datadir::{self, DataDir};
use crate::error::ServerError;
use crate::names::canonical_account;
use crate::tomlx::{self, Reader, Rule, Source, View, quote};

/// Argon2id parameters of one hash (§3.4, §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashParams {
    /// Memory, KiB.
    pub memory_kib: u32,
    /// Passes.
    pub iterations: u32,
    /// Lanes.
    pub lanes: u32,
}

impl HashParams {
    /// The configured parameters for new hashes.
    #[must_use]
    pub fn from_config(c: &crate::config::AccountsSection) -> Self {
        Self {
            memory_kib: c.argon2_memory_kib,
            iterations: c.argon2_iterations,
            lanes: c.argon2_lanes,
        }
    }

    /// Whether `self` is below `other` in any of memory, iterations or lanes (re-hash rule).
    #[must_use]
    pub fn below(&self, other: &Self) -> bool {
        self.memory_kib < other.memory_kib
            || self.iterations < other.iterations
            || self.lanes < other.lanes
    }
}

/// Salt length of new hashes, bytes (128 bits).
pub const SALT_LEN: usize = 16;
/// Tag length of new hashes, bytes (256 bits).
pub const TAG_LEN: usize = 32;
/// Longest password, bytes (`PASSWORD`, `tessera-protocol.md` §5.2).
pub const MAX_PASSWORD: usize = 256;
/// Most memory a stored hash may ask for, KiB (the configuration's maximum): a hostile
/// accounts file cannot make a verification allocate more.
const MAX_STORED_MEMORY_KIB: u32 = 1_048_576;
/// Most passes a stored hash may ask for.
const MAX_STORED_ITERATIONS: u32 = 64;
/// Most lanes a stored hash may ask for.
const MAX_STORED_LANES: u32 = 64;

/// A parsed PHC string of an Argon2id (v=19) hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhcHash {
    /// Parameters.
    pub params: HashParams,
    salt: Vec<u8>,
    tag: Vec<u8>,
}

impl PhcHash {
    /// Parses `$argon2id$v=19$m=<KiB>,t=<iterations>,p=<lanes>$<salt>$<tag>` (unpadded
    /// standard base64). Bounded: refuses parameters that would make a verification
    /// unreasonably expensive.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        if s.len() > 512 {
            return None;
        }
        let mut parts = s.split('$');
        if !parts.next()?.is_empty() || parts.next()? != "argon2id" || parts.next()? != "v=19" {
            return None;
        }
        let params = parts.next()?;
        let salt = parts.next()?;
        let tag = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let mut m = None;
        let mut t = None;
        let mut p = None;
        for kv in params.split(',') {
            let (k, v) = kv.split_once('=')?;
            if v.is_empty() || v.len() > 10 || !v.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: u32 = v.parse().ok()?;
            let slot = match k {
                "m" => &mut m,
                "t" => &mut t,
                "p" => &mut p,
                _ => return None,
            };
            if slot.replace(n).is_some() {
                return None;
            }
        }
        let params = HashParams {
            memory_kib: m?,
            iterations: t?,
            lanes: p?,
        };
        if params.memory_kib > MAX_STORED_MEMORY_KIB
            || params.iterations > MAX_STORED_ITERATIONS
            || params.lanes > MAX_STORED_LANES
            || Params::new(
                params.memory_kib,
                params.iterations,
                params.lanes,
                Some(TAG_LEN),
            )
            .is_err()
        {
            return None;
        }
        let salt = Base64Unpadded::decode_vec(salt).ok()?;
        let tag = Base64Unpadded::decode_vec(tag).ok()?;
        if !(8..=64).contains(&salt.len()) || !(16..=64).contains(&tag.len()) {
            return None;
        }
        Some(Self { params, salt, tag })
    }

    /// The PHC string.
    #[must_use]
    pub fn to_phc(&self) -> String {
        format!(
            "$argon2id$v=19$m={},t={},p={}${}${}",
            self.params.memory_kib,
            self.params.iterations,
            self.params.lanes,
            Base64Unpadded::encode_string(&self.salt),
            Base64Unpadded::encode_string(&self.tag)
        )
    }

    /// Verifies a password (constant-time tag comparison).
    #[must_use]
    pub fn verify(&self, password: &[u8]) -> bool {
        let Ok(mut out) = compute(password, &self.salt, self.params, self.tag.len()) else {
            return false;
        };
        let equal = out.len() == self.tag.len()
            && out
                .iter()
                .zip(&self.tag)
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0;
        out.fill(0);
        equal
    }
}

fn compute(
    password: &[u8],
    salt: &[u8],
    p: HashParams,
    tag_len: usize,
) -> Result<Vec<u8>, ServerError> {
    let params = Params::new(p.memory_kib, p.iterations, p.lanes, Some(tag_len))
        .map_err(|e| ServerError::Runtime(format!("Argon2 parameters: {e}")))?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = vec![0u8; tag_len];
    a.hash_password_into(password, salt, &mut out)
        .map_err(|e| ServerError::Runtime(format!("Argon2: {e}")))?;
    Ok(out)
}

/// Fills `buf` from the operating system's CSPRNG.
pub fn random_bytes(buf: &mut [u8]) -> Result<(), ServerError> {
    getrandom::fill(buf).map_err(|e| ServerError::Runtime(format!("random number source: {e}")))
}

/// Hashes a password with a fresh 16-byte salt and a 32-byte tag (§3.4); returns the PHC
/// string.
pub fn hash_password(password: &[u8], params: HashParams) -> Result<String, ServerError> {
    let mut salt = [0u8; SALT_LEN];
    random_bytes(&mut salt)?;
    let tag = compute(password, &salt, params, TAG_LEN)?;
    Ok(PhcHash {
        params,
        salt: salt.to_vec(),
        tag,
    }
    .to_phc())
}

/// The 57-character alphabet of generated passwords: `A–Z a–z 2–9` without `I O l` (§2.4).
pub const PASSWORD_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
/// Length of generated passwords.
pub const GENERATED_PASSWORD_LEN: usize = 20;

/// A random password of 20 characters from [`PASSWORD_ALPHABET`] (rejection sampling, no
/// bias).
pub fn generate_password() -> Result<String, ServerError> {
    let n = PASSWORD_ALPHABET.len();
    let limit = 256 - (256 % n);
    let mut out = String::with_capacity(GENERATED_PASSWORD_LEN);
    let mut buf = [0u8; 64];
    while out.len() < GENERATED_PASSWORD_LEN {
        random_bytes(&mut buf)?;
        for &b in &buf {
            if usize::from(b) < limit && out.len() < GENERATED_PASSWORD_LEN {
                out.push(char::from(PASSWORD_ALPHABET[usize::from(b) % n]));
            }
        }
    }
    Ok(out)
}

/// The password policy for new passwords (§6.5): `min_bytes`–256 bytes of UTF-8, not equal
/// to the account name ignoring ASCII case.
#[must_use]
pub fn password_acceptable(password: &str, account: &str, min_bytes: u32) -> bool {
    let len = password.len();
    len >= usize::try_from(min_bytes).unwrap_or(usize::MAX)
        && len <= MAX_PASSWORD
        && !password.eq_ignore_ascii_case(account)
}

/// Overwrites a password buffer (§6.2 step 6).
pub fn wipe(password: String) {
    let mut bytes = password.into_bytes();
    bytes.fill(0);
    std::hint::black_box(&bytes);
}

/// One account (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Canonical name.
    pub name: String,
    /// PHC string.
    pub hash: String,
    /// Creation time, RFC 3339 UTC.
    pub created: String,
    /// Login refused with `ACCOUNT_DISABLED`.
    pub disabled: bool,
    /// Chat refused with `MUTED`.
    pub muted: bool,
}

/// The accounts file, keyed and ordered by canonical name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountsFile {
    /// Accounts by canonical name.
    pub accounts: BTreeMap<String, Account>,
}

impl AccountsFile {
    /// Parses the file. Any unknown key, duplicate or non-canonical name or invalid hash
    /// makes the whole file invalid (§3.4); the message names the line.
    pub fn parse(text: &str) -> Result<Self, String> {
        let src = Source::new(text);
        let doc = tomlx::parse(&src).map_err(|f| format!("line {}: {}", f.line, f.message))?;
        let mut r = Reader::new(&src, Rule::Format, Rule::Range);
        let root = View {
            table: doc.get_ref(),
            line: 1,
            path: "",
        };
        r.check_keys(root, &["format", "account"]);
        if r.int(root, "format", true, 1, 1).is_none() && r.findings.is_empty() {
            r.push(1, Rule::Format, "unknown format");
        }
        let mut out = Self::default();
        for (t, line) in r.tables(root, "account") {
            let v = View {
                table: t,
                line,
                path: "account",
            };
            r.check_keys(v, &["name", "hash", "created", "disabled", "muted"]);
            let name = r.string(v, "name", true);
            let hash = r.string(v, "hash", true);
            let created = r.utc_datetime(v, "created", true);
            let disabled = r.bool(v, "disabled", false).is_some_and(|(b, _)| b);
            let muted = r.bool(v, "muted", false).is_some_and(|(b, _)| b);
            let (Some((name, nline)), Some((hash, hline)), Some((created, _))) =
                (name, hash, created)
            else {
                continue;
            };
            if canonical_account(&name).as_deref() != Some(name.as_str()) {
                r.push(
                    nline,
                    Rule::Format,
                    "account name is not canonical (ACCOUNT, lowercase)",
                );
                continue;
            }
            if PhcHash::parse(&hash).is_none() {
                r.push(
                    hline,
                    Rule::Format,
                    "hash is not a valid Argon2id PHC string",
                );
                continue;
            }
            if out.accounts.contains_key(&name) {
                r.push(nline, Rule::Format, "duplicate account name");
                continue;
            }
            out.accounts.insert(
                name.clone(),
                Account {
                    name,
                    hash,
                    created,
                    disabled,
                    muted,
                },
            );
        }
        if let Some(f) = r.findings.first() {
            return Err(format!("line {}: {}", f.line, f.message));
        }
        Ok(out)
    }

    /// The file's text, accounts in canonical name order.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut s = String::from(
            "# Tessera test server accounts (private operator state; never publish or commit)\nformat = 1\n",
        );
        for a in self.accounts.values() {
            s.push_str(&format!(
                "\n[[account]]\nname = {}\nhash = {}\ncreated = {}\n",
                quote(&a.name),
                quote(&a.hash),
                a.created
            ));
            if a.disabled {
                s.push_str("disabled = true\n");
            }
            if a.muted {
                s.push_str("muted = true\n");
            }
        }
        s
    }
}

/// Reads the accounts file (empty when missing).
pub fn read_accounts(dir: &DataDir) -> Result<AccountsFile, ServerError> {
    let path = dir.accounts_path();
    match tomlx::read_text(&path) {
        Ok(Ok(text)) => AccountsFile::parse(&text).map_err(|message| ServerError::InvalidFile {
            path: path.clone(),
            message,
        }),
        Ok(Err(f)) => Err(ServerError::InvalidFile {
            path,
            message: f.message,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AccountsFile::default()),
        Err(e) => Err(ServerError::io(path, e)),
    }
}

/// Writes the accounts file atomically with mode 0600 (§3.7). Refuses (and writes nothing)
/// when the text would be larger than every reader accepts ([`tomlx::MAX_TOML_FILE`]): such
/// a file would make the server refuse to start.
pub fn write_accounts(dir: &DataDir, file: &AccountsFile) -> Result<(), ServerError> {
    let path = dir.accounts_path();
    let text = file.to_toml();
    if text.len() as u64 > tomlx::MAX_TOML_FILE {
        return Err(ServerError::InvalidFile {
            path,
            message: format!(
                "the accounts file would exceed {} bytes; not written",
                tomlx::MAX_TOML_FILE
            ),
        });
    }
    datadir::atomic_write(&path, text.as_bytes(), 0o600).map_err(|e| ServerError::io(path, e))
}

/// Whether the accounts file's text fits in what every reader accepts.
#[must_use]
pub fn fits(file: &AccountsFile) -> bool {
    file.to_toml().len() as u64 <= tomlx::MAX_TOML_FILE
}

/// Read-modify-write of the accounts file under the accounts lock (§3.7).
pub fn modify_accounts<T>(
    dir: &DataDir,
    f: impl FnOnce(&mut AccountsFile) -> Result<T, ServerError>,
) -> Result<(T, AccountsFile), ServerError> {
    datadir::with_accounts_lock(dir, || {
        let mut file = read_accounts(dir)?;
        let out = f(&mut file)?;
        write_accounts(dir, &file)?;
        Ok((out, file))
    })
}

/// Creates accounts with the given passwords in one read-modify-write of the accounts file
/// (under the accounts lock), hashing with `params`. Existing names are replaced. For load
/// tests and in-process test servers (`test-server.md` §13.1); runs on the calling thread.
pub fn add_accounts(
    dir: &DataDir,
    params: HashParams,
    entries: &[(&str, &str)],
) -> Result<(), ServerError> {
    let mut hashed = Vec::with_capacity(entries.len());
    for (name, password) in entries {
        let canonical = canonical_account(name)
            .ok_or_else(|| ServerError::Usage(format!("\"{name}\" is not an account name")))?;
        hashed.push((canonical, hash_password(password.as_bytes(), params)?));
    }
    datadir::create_private_dir(dir.path())?;
    modify_accounts(dir, |f| {
        for (name, hash) in hashed {
            f.accounts.insert(name.clone(), new_account(name, hash));
        }
        Ok(())
    })?;
    Ok(())
}

/// A new account record with the current time.
#[must_use]
pub fn new_account(name: String, hash: String) -> Account {
    Account {
        name,
        hash,
        created: datadir::utc_rfc3339(SystemTime::now()),
        disabled: false,
        muted: false,
    }
}

fn file_stamp(path: &std::path::Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

#[derive(Debug, Default)]
struct StoreInner {
    file: AccountsFile,
    stamp: Option<(SystemTime, u64)>,
}

/// The accounts as the running server sees them: the last good copy of `accounts.toml`,
/// re-read when its modification time or size changes (§3.7). Shared by the simulation
/// (in-memory reads only), the hash workers and the writer (which do the file I/O).
#[derive(Debug)]
pub struct AccountStore {
    dir: DataDir,
    inner: RwLock<StoreInner>,
    generation: AtomicU64,
}

impl AccountStore {
    /// Loads the accounts; an invalid file is an error (`run` refuses to start, §3.4).
    pub fn load(dir: &DataDir) -> Result<Self, ServerError> {
        let stamp = file_stamp(&dir.accounts_path());
        let file = read_accounts(dir)?;
        Ok(Self {
            dir: dir.clone(),
            inner: RwLock::new(StoreInner { file, stamp }),
            generation: AtomicU64::new(1),
        })
    }

    /// An empty store that never touches a file (for fuzzing and unit tests of the
    /// simulation).
    #[must_use]
    pub fn in_memory(file: AccountsFile) -> Self {
        Self {
            dir: DataDir::new(PathBuf::new()),
            inner: RwLock::new(StoreInner { file, stamp: None }),
            generation: AtomicU64::new(1),
        }
    }

    fn file_backed(&self) -> bool {
        !self.dir.path().as_os_str().is_empty()
    }

    /// Increases on every change of the in-memory copy.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// One account by canonical name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Account> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.file.accounts.get(name).cloned())
    }

    /// Every account's canonical name with its disabled and muted flags.
    #[must_use]
    pub fn flags(&self) -> Vec<(String, bool, bool)> {
        self.inner.read().map_or_else(
            |_| Vec::new(),
            |g| {
                g.file
                    .accounts
                    .values()
                    .map(|a| (a.name.clone(), a.disabled, a.muted))
                    .collect()
            },
        )
    }

    /// Number of accounts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().map_or(0, |g| g.file.accounts.len())
    }

    /// Whether there are no accounts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn replace(&self, file: AccountsFile, stamp: Option<(SystemTime, u64)>) {
        if let Ok(mut g) = self.inner.write() {
            g.file = file;
            g.stamp = stamp;
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Re-reads the file if its modification time or size changed. A file that became
    /// invalid is logged and the last good copy kept. File I/O: never call this on the
    /// network or simulation thread. Returns true when the copy changed.
    pub fn refresh_if_changed(&self) -> bool {
        if !self.file_backed() {
            return false;
        }
        let path = self.dir.accounts_path();
        let stamp = file_stamp(&path);
        let same = self.inner.read().is_ok_and(|g| g.stamp == stamp);
        if same {
            return false;
        }
        // Read and install under the accounts lock, so a concurrent `modify` (registration,
        // re-hash) can never be overwritten by this older copy.
        let r = datadir::with_accounts_lock(&self.dir, || {
            let stamp = file_stamp(&path);
            let file = read_accounts(&self.dir)?;
            self.replace(file, stamp);
            Ok(())
        });
        match r {
            Ok(()) => {
                tracing::info!("accounts file reloaded");
                true
            }
            Err(e) => {
                tracing::error!(error = %e, "accounts file invalid; keeping the last good copy");
                if let Ok(mut g) = self.inner.write() {
                    g.stamp = stamp;
                }
                false
            }
        }
    }

    /// Applies a change under the accounts lock and updates the in-memory copy. For an
    /// in-memory store only the copy changes.
    pub fn modify<T>(
        &self,
        f: impl FnOnce(&mut AccountsFile) -> Result<T, ServerError>,
    ) -> Result<T, ServerError> {
        if !self.file_backed() {
            let mut file = self
                .inner
                .read()
                .map(|g| g.file.clone())
                .unwrap_or_default();
            let out = f(&mut file)?;
            self.replace(file, None);
            return Ok(out);
        }
        // The in-memory copy is replaced while the accounts lock is still held (see
        // `refresh_if_changed`).
        datadir::with_accounts_lock(&self.dir, || {
            let mut file = read_accounts(&self.dir)?;
            let out = f(&mut file)?;
            write_accounts(&self.dir, &file)?;
            let stamp = file_stamp(&self.dir.accounts_path());
            self.replace(file, stamp);
            Ok(out)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAST: HashParams = HashParams {
        memory_kib: 64,
        iterations: 1,
        lanes: 1,
    };

    #[test]
    fn phc_round_trip() {
        let h = hash_password(b"correct horse", FAST).unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=64,t=1,p=1$"));
        let p = PhcHash::parse(&h).unwrap();
        assert_eq!(p.to_phc(), h);
        assert!(p.verify(b"correct horse"));
        assert!(!p.verify(b"correct horsE"));
        assert!(
            PhcHash::parse("$argon2i$v=19$m=64,t=1,p=1$AAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAA")
                .is_none()
        );
        assert!(
            PhcHash::parse("$argon2id$v=19$m=99999999,t=1,p=1$AAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAA")
                .is_none()
        );
        assert!(PhcHash::parse("").is_none());
        assert!(
            PhcHash::parse("$argon2id$v=19$m=64,t=1,p=1,p=1$AAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAA")
                .is_none()
        );
    }

    #[test]
    fn generated_passwords() {
        let p = generate_password().unwrap();
        assert_eq!(p.len(), 20);
        assert!(p.bytes().all(|b| PASSWORD_ALPHABET.contains(&b)));
        assert_eq!(PASSWORD_ALPHABET.len(), 57);
        assert!(!PASSWORD_ALPHABET.contains(&b'I') && !PASSWORD_ALPHABET.contains(&b'l'));
    }

    #[test]
    fn policy() {
        assert!(password_acceptable("12345678", "bob", 8));
        assert!(!password_acceptable("1234567", "bob", 8));
        assert!(!password_acceptable("BobBobBob", "bobbobbob", 8));
        assert!(!password_acceptable(&"x".repeat(257), "bob", 8));
    }

    #[test]
    fn file_round_trip() {
        let mut f = AccountsFile::default();
        let h = hash_password(b"pw", FAST).unwrap();
        let mut a = new_account("alice".into(), h);
        a.muted = true;
        f.accounts.insert("alice".into(), a);
        let text = f.to_toml();
        assert_eq!(AccountsFile::parse(&text).unwrap(), f);
        assert!(AccountsFile::parse(&text.replace("alice", "Alice")).is_err());
        assert!(AccountsFile::parse(&format!("{text}bogus = 1\n")).is_err());
        assert!(AccountsFile::parse(&text.replace("$argon2id", "$argon2d")).is_err());
        let dup = format!(
            "{text}{}",
            text.split_once("\n\n").unwrap().1.replace("\n[[", "\n\n[[")
        );
        assert!(
            AccountsFile::parse(&format!(
                "{dup}\n[[account]]\nname = \"alice\"\nhash = {}\ncreated = 2026-01-01T00:00:00Z\n",
                quote(&f.accounts["alice"].hash)
            ))
            .is_err()
        );
    }
}
