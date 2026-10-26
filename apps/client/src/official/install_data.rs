// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's records for the live official connector (`official-login.md` §2.4.3,
//! `official-session.md` §4.1.3, `official-messages-world.md` §2.7): the archives opened
//! read-only and the master property dictionary (`0x34000000`) as the wire property schema.
//!
//! [`loader`] runs on the connector's thread (opening the archives and decoding the
//! dictionary take a while; the frame thread never waits for them, ARCH §21).

use std::path::PathBuf;
use std::sync::Arc;

use tessera_compat::connector::{InstallData, InstallDataLoader, InstallLocator};
use tessera_compat::world::PropertyType;
use tessera_dat::{ArchiveSet, SetOptions};
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

impl InstallData for ArchiveData {
    fn record(&self, did: u32) -> Option<Vec<u8>> {
        self.set.read(did).ok()
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        let t = self.registry.property_type(pid)?;
        PropertyType::from_code(t.code()).ok()
    }
}

/// Opens the install at `root` on the calling thread.
///
/// # Errors
///
/// What could not be read, without paths.
fn open(root: &std::path::Path) -> Result<ArchiveData, String> {
    let set = ArchiveSet::open(root, SetOptions::default())
        .map_err(|e| format!("the archives cannot be opened ({e})"))?;
    let dict = set
        .read(PROPERTY_DICTIONARY_DID.0)
        .map_err(|e| format!("the property dictionary cannot be read ({e})"))?;
    let registry = PropertyRegistry::parse(&dict)
        .map_err(|e| format!("the property dictionary cannot be decoded ({e})"))?;
    Ok(ArchiveData { set, registry })
}

/// The connector's loader of the install's records.
pub fn loader() -> InstallDataLoader {
    Box::new(|root| open(root).map(|d| Arc::new(d) as Arc<dyn InstallData>))
}

/// The connector's locator: `--game-dir`, else `TESSERA_GAME_DIR`, else discovery
/// ([`crate::data::find_install`]).
pub fn locator(explicit: Option<PathBuf>) -> InstallLocator {
    Box::new(move || crate::data::find_install(explicit.as_deref()).map(|i| i.root))
}
