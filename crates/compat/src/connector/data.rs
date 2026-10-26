// SPDX-License-Identifier: GPL-3.0-or-later
//! What the live connector reads from the user's install before any network operation
//! (`official-login.md` §2, §5.1 `S1_LOCAL_DISCOVERY`, invariant `I1`): the launcher
//! configuration, the language, the client and data version, and the two compression
//! dictionaries of `official-session.md` §4.1.2–§4.1.3.
//!
//! The archive records (the DID maps, the dictionaries, the property dictionary) come from
//! an [`InstallData`] the client supplies: this crate has no archive reader of its own.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::chat_session::ChatCodecs;
use crate::login::{
    DICT_KEY_CLIENT_TO_SERVER, DICT_KEY_SERVER_TO_CLIENT_DEFERRED, DidMap3Table, LauncherConfig,
    LoginError, NetworkVersionDiscovery, NetworkVersionQuery, UserPreferences, VersionInfo,
    compression_dictionary_map_did, discover_network_version, extract_didver_from_dat_file,
    resolve_bit_tree_table, resolve_install_language, root_did_map_from_superblock,
};
use crate::play::chat::{LiteralTexts, TextResolver};
use crate::session::{BitTreeCodec, BitTreeTable};
use crate::world::{
    LocalizedTokenText, PortalStateSchema, PortalStateSchemaLookup, PropertySchemaLookup,
    PropertyType,
};

/// The archive whose superblock names the Master DID Map (`official-login.md` §2.4.3,
/// `official-session.md` §4.1.3 item 1) and whose header carries `didver` (§2.4.1 item 2).
pub const GENERAL_ARCHIVE: &str = "client_general.dat";

/// The fallback archive for `didver` (§2.4.1 item 2).
pub const GAMELOGIC_ARCHIVE: &str = "client_gamelogic.dat";

/// Longest archive record the connector takes from [`InstallData`] (the dictionaries and DID
/// maps are far smaller). Engineering choice, spec silent.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

/// Read-only records and tables of the user's install, supplied by the client.
///
/// Everything here is data of the user's own install, read at run time (decision 0034 §3,
/// `ARCHITECTURE.md` §19.3); nothing is built in.
pub trait InstallData: Send + Sync + std::fmt::Debug {
    /// The payload of the archive record `did`, or `None` when the install has none.
    fn record(&self, did: u32) -> Option<Vec<u8>>;

    /// The property type of `pid` in the master property dictionary (`0x34000000`,
    /// `official-messages-world.md` §2.7.1), or `None` when the dictionary has none.
    fn property_type(&self, pid: u32) -> Option<PropertyType>;

    /// The `0xCC` state schema `id` (`official-messages-world.md` §4.3.1), when the client
    /// built it from the install. The default knows none: `0xCC` events then fail to
    /// decode and are counted.
    fn portal_state_schema(&self, _id: u32) -> Option<&PortalStateSchema> {
        None
    }

    /// The display text of a `LocalizedTokenText`. The default resolves literal texts only.
    fn resolve_text(&self, text: &LocalizedTokenText) -> Option<String> {
        LiteralTexts.resolve(text)
    }
}

/// Makes the [`InstallData`] of the install root it is given, on the connector's thread
/// (opening the archives can take a while; it never runs on the frame thread). An error
/// names what is missing, without paths of the user.
pub type InstallDataLoader = Box<dyn FnOnce(&Path) -> Result<Arc<dyn InstallData>, String> + Send>;

/// Finds the install root on the connector's thread (install discovery can scan the disk).
pub type InstallLocator = Box<dyn FnOnce() -> Result<PathBuf, String> + Send>;

/// The [`InstallData`] seen as the world module's property schema.
#[derive(Debug, Clone)]
pub struct SchemaView(pub Arc<dyn InstallData>);

impl PropertySchemaLookup for SchemaView {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.0.property_type(pid)
    }
}

impl PortalStateSchemaLookup for SchemaView {
    fn portal_state_schema(&self, state_schema_id: u32) -> Option<&PortalStateSchema> {
        self.0.portal_state_schema(state_schema_id)
    }
}

impl TextResolver for SchemaView {
    fn resolve(&self, text: &LocalizedTokenText) -> Option<String> {
        self.0.resolve_text(text)
    }
}

/// Where the connector looks on this machine (all given by the client; nothing found
/// here is written to).
#[derive(Debug, Clone, Default)]
pub struct LocalPaths {
    /// The install root (`<InstallRoot>`, `official-login.md` §2).
    pub install_root: PathBuf,
    /// The directory of `UserPreferences*.ini`, when the client knows it (§2.3).
    pub user_settings_dir: Option<PathBuf>,
    /// A network version string the user supplied (§2.4.1 step 1).
    pub network_version: Option<String>,
}

/// Everything `S1_LOCAL_DISCOVERY` found (§5.1).
#[derive(Debug, Clone)]
pub struct LocalInputs {
    /// The launcher configuration (§2.1).
    pub config: LauncherConfig,
    /// The `<Lang>` token (§2.2).
    pub language: String,
    /// `netver` and `didver` (§2.4.1).
    pub version: VersionInfo,
    /// The two wire codecs (`official-session.md` §4.1.2).
    pub codecs: ChatCodecs,
    /// The `codecUuid`s of the loaded dictionaries (`official-session.md` §6.4 step 3).
    pub codec_uuids: Vec<[u8; 16]>,
    /// The install's records and tables.
    pub data: Arc<dyn InstallData>,
}

fn invalid(reason: &str) -> LoginError {
    LoginError::ConfigInvalid {
        reason: reason.to_owned(),
    }
}

/// The first `0x1A8` bytes of an archive (its superblock, `official-login.md` §2.4.1 item 2).
fn archive_header(path: &Path) -> Result<Vec<u8>, LoginError> {
    let file = std::fs::File::open(path).map_err(|_| invalid("an archive cannot be opened"))?;
    let mut header = Vec::with_capacity(0x1A8);
    file.take(0x1A8)
        .read_to_end(&mut header)
        .map_err(|_| invalid("an archive header cannot be read"))?;
    if header.len() < 0x1A8 {
        return Err(invalid("an archive header is too short"));
    }
    Ok(header)
}

fn record(data: &dyn InstallData, did: u32) -> Result<Vec<u8>, LoginError> {
    let bytes = data
        .record(did)
        .ok_or_else(|| invalid("a record the session needs is missing from the install"))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(invalid("a record the session needs is too large"));
    }
    Ok(bytes)
}

/// One dictionary resolved through Master DID Map `3` with at most one redirect
/// (`official-session.md` §4.1.3 item 1; invariant `I16`).
fn dictionary(
    data: &dyn InstallData,
    map: &DidMap3Table,
    key: u32,
) -> Result<(Arc<dyn BitTreeCodec>, [u8; 16]), LoginError> {
    let initial = map
        .resolve_enum_key(key)
        .ok_or_else(|| invalid("the compression dictionary map lacks a dictionary"))?;
    let mut cache: HashMap<u32, Vec<u8>> = HashMap::new();
    let first = record(data, initial)?;
    let header = crate::login::BitTreeTableHeader::parse(&first, initial)?;
    cache.insert(initial, first);
    if header.redirects {
        let target = header.redirect_did;
        cache.insert(target, record(data, target)?);
    }
    let resolved = resolve_bit_tree_table(map, key, |did| cache.get(&did).map(Vec::as_slice))?;
    let bytes = cache
        .get(&resolved.resolved_did)
        .ok_or_else(|| invalid("a compression dictionary record is missing"))?;
    let table = BitTreeTable::from_dat_bytes(bytes)
        .map_err(|_| invalid("a compression dictionary record is malformed"))?;
    Ok((Arc::new(table), resolved.header.codec_uuid))
}

/// The two wire codecs from the install (`official-login.md` §2.4.3; `official-session.md`
/// §4.1.2–§4.1.3): the root DID map from `client_general.dat`'s superblock, its compression
/// dictionary sub-map, then `0x10000002` (client to server) and `0x10000001` (server to
/// client, through its deferred redirect).
///
/// # Errors
///
/// [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) when a record is missing or malformed.
pub(crate) fn load_codecs(
    install_root: &Path,
    data: &dyn InstallData,
) -> Result<(ChatCodecs, Vec<[u8; 16]>), LoginError> {
    let header = archive_header(&install_root.join(GENERAL_ARCHIVE))?;
    let root_did = root_did_map_from_superblock(&header)?;
    let root = record(data, root_did)?;
    let sub_did = compression_dictionary_map_did(&root)?;
    let sub = record(data, sub_did)?;
    let map = DidMap3Table::parse(&sub)?;
    let (outgoing, out_uuid) = dictionary(data, &map, DICT_KEY_CLIENT_TO_SERVER)?;
    let (incoming, in_uuid) = dictionary(data, &map, DICT_KEY_SERVER_TO_CLIENT_DEFERRED)?;
    let mut uuids = vec![out_uuid];
    if in_uuid != out_uuid {
        uuids.push(in_uuid);
    }
    Ok((
        ChatCodecs {
            incoming: Some(incoming),
            outgoing: Some(outgoing),
        },
        uuids,
    ))
}

/// `S1_LOCAL_DISCOVERY` (`official-login.md` §5.1): every local input of the login, read
/// before any network operation (invariant `I1`).
///
/// # Errors
///
/// [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) for a missing or malformed launcher
/// configuration, language archive, network version (none supplied and none in the
/// version resource: Tessera has no prompt for it, §2.4.1 step 3), archive header or
/// dictionary, or when the client's [`InstallDataLoader`] fails.
pub fn discover_local(
    paths: &LocalPaths,
    loader: InstallDataLoader,
) -> Result<LocalInputs, LoginError> {
    let root = paths.install_root.as_path();
    let (_, config) = LauncherConfig::discover(root)?;
    // §2.3: `<Documents>/<Product.DocumentFolder>/` unless the client named the directory.
    let settings_dir = paths.user_settings_dir.clone().or_else(|| {
        let folder = config.document_folder.as_deref()?;
        Some(dirs::document_dir()?.join(folder))
    });
    let prefs = match settings_dir.as_deref() {
        Some(dir) => UserPreferences::discover(dir)
            .ok()
            .flatten()
            .map(|(_, p)| p),
        None => None,
    };
    let language =
        resolve_install_language(root, prefs.as_ref().and_then(|p| p.last_locale.as_deref()))?;
    let netver = match discover_network_version(&NetworkVersionQuery {
        user_value: paths.network_version.as_deref(),
        install_root: Some(root),
        client_executable: None,
        resource_key: None,
    }) {
        NetworkVersionDiscovery::Found { version, .. } => version,
        NetworkVersionDiscovery::AskUser(reason) => {
            tracing::debug!(?reason, "official login: no network version string");
            return Err(invalid("no client network version string is available"));
        }
    };
    let (guid, archive_version) = extract_didver_from_dat_file(&root.join(GENERAL_ARCHIVE))
        .or_else(|_| extract_didver_from_dat_file(&root.join(GAMELOGIC_ARCHIVE)))?;
    let version = VersionInfo::new(netver, guid, archive_version)?;
    let data = loader(root).map_err(|why| {
        tracing::warn!("official login: the install's records cannot be read ({why})");
        invalid("the install's records cannot be read")
    })?;
    let (codecs, codec_uuids) = load_codecs(root, data.as_ref())?;
    Ok(LocalInputs {
        config,
        language,
        version,
        codecs,
        codec_uuids,
        data,
    })
}
