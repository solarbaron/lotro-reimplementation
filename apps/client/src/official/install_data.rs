// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's records for the live official connector (`official-login.md` §2.4.3,
//! `official-session.md` §4.1.3, `official-messages-world.md` §2.7): the archives opened
//! read-only and the master property dictionary (`0x34000000`) as the wire property schema.
//!
//! [`loader`] and [`locator`] run on the connector's thread (finding the install, opening
//! the archives and decoding the dictionary take a while; the frame thread never waits for
//! them, ARCH §21). Their errors are kinds ([`InstallProblem`]): no path of the user reaches
//! a log line from here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tessera_compat::connector::{
    InstallData, InstallDataLoader, InstallLocator, InstallProblem, LocatedInstall,
};
use tessera_compat::world::PropertyType;
use tessera_dat::{ArchiveSet, SetOptions, inflate_record};
use tessera_props::{PROPERTY_DICTIONARY_DID, PropertyRegistry};

/// The archives and the property dictionary of one install.
pub struct ArchiveData {
    set: ArchiveSet,
    registry: PropertyRegistry,
}

impl std::fmt::Debug for ArchiveData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveData")
            .field("archives", &self.set.members().len())
            .field("properties", &self.registry.len())
            .finish()
    }
}

/// The 4-byte inflated-length prefix a compressed record's stored `length` includes
/// (`dat-container.md` §5.2).
const COMPRESSED_PREFIX: u64 = 4;

impl InstallData for ArchiveData {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        let located = self.set.locate(did).ok()??;
        let max = u64::try_from(max_bytes).ok()?;
        let entry = &located.entry;
        // The stored size is known before anything is read: a larger record is not read.
        let stored_limit = if entry.is_compressed() {
            max.saturating_add(COMPRESSED_PREFIX)
        } else {
            max
        };
        if u64::from(entry.length) > stored_limit {
            return None;
        }
        let member = self.set.archive_of(&located)?;
        let stored = member.archive.read_raw(entry).ok()?;
        if !entry.is_compressed() {
            return (stored.len() <= max_bytes).then_some(stored);
        }
        // Inflation is bounded by the same limit.
        inflate_record(&stored, max).ok()
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        let t = self.registry.property_type(pid)?;
        PropertyType::from_code(t.code()).ok()
    }
}

/// Opens the install at `root` on the calling thread.
fn open(root: &Path) -> Result<ArchiveData, InstallProblem> {
    let set =
        ArchiveSet::open(root, SetOptions::default()).map_err(|_| InstallProblem::Archives)?;
    let dict = set
        .read(PROPERTY_DICTIONARY_DID.0)
        .map_err(|_| InstallProblem::Record)?;
    let registry = PropertyRegistry::parse(&dict).map_err(|_| InstallProblem::Properties)?;
    Ok(ArchiveData { set, registry })
}

/// The connector's loader of the install's records.
pub fn loader() -> InstallDataLoader {
    Box::new(|root| open(root).map(|d| Arc::new(d) as Arc<dyn InstallData>))
}

/// The `Product.DocumentFolder` of the install's launcher configuration, if it has one.
fn document_folder(root: &Path) -> Option<String> {
    let (_, config) = tessera_compat::login::LauncherConfig::discover(root).ok()?;
    config.document_folder
}

/// The original's settings directory for the install (`install-layout.md` §8.1): inside the
/// install's Wine, Proton or CrossOver prefix when it has one, else the platform's
/// Documents folder, with the launcher configuration's `Product.DocumentFolder`.
fn settings_dir(install: &tessera_install::GameInstall) -> Option<PathBuf> {
    let folder = document_folder(&install.root);
    let dir = install.settings_dir(
        &tessera_install::SettingsQuery {
            prefs_file: None,
            document_folder: folder.as_deref(),
        },
        &tessera_install::SearchEnv::from_process(),
    );
    dir.exists.then_some(dir.path)
}

/// The connector's locator: `--game-dir`, else `TESSERA_GAME_DIR`, else discovery, with
/// the settings directory of the install found.
pub fn locator(explicit: Option<PathBuf>) -> InstallLocator {
    Box::new(move || {
        let env = tessera_install::SearchEnv::from_process();
        let discovery = tessera_install::discover_with(&env, explicit.as_deref())
            .map_err(|_| InstallProblem::NotFound)?;
        let install = discovery
            .installs
            .into_iter()
            .next()
            .ok_or(InstallProblem::NotFound)?;
        Ok(LocatedInstall {
            user_settings_dir: settings_dir(&install),
            root: install.root,
        })
    })
}
