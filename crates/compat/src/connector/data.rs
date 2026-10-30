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
    DICT_KEY_CLIENT_TO_SERVER, DICT_KEY_SERVER_TO_CLIENT_DEFERRED, DataIdMap, LauncherConfig,
    LoginError, NetPreferences, NetworkVersionDiscovery, NetworkVersionQuery, UserPreferences,
    VersionInfo, compression_dictionary_map_did, discover_network_version,
    extract_didver_from_dat_file, resolve_bit_tree_table, resolve_install_language,
    root_did_map_from_superblock,
};
use crate::play::chat::{CATEGORY_SAY, CATEGORY_TELL, ChatCategories, LiteralTexts, TextResolver};
use crate::session::{BitTreeCodec, BitTreeTable};
use crate::world::{
    EncodedPropertyValue, LocalizedTokenText, PortalStateSchema, PortalStateSchemaLookup,
    PropertySchemaLookup, PropertyType,
};
use tessera_net::ChatChannel;

/// The archive whose superblock names the Master DID Map (`official-login.md` §2.4.3,
/// `official-session.md` §4.1.3 item 1) and whose header carries `didver` (§2.4.1 item 2).
pub const GENERAL_ARCHIVE: &str = "client_general.dat";

/// The fallback archive for `didver` (§2.4.1 item 2).
pub const GAMELOGIC_ARCHIVE: &str = "client_gamelogic.dat";

/// Longest archive record the connector takes from [`InstallData`] (the dictionaries and DID
/// maps are far smaller). Engineering choice, spec silent.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

/// Whether `did` names a string table: the `0x25` prefix that a `LocalizedTokenText`
/// reference's `table_did` carries (`official-messages-world.md` §2.6 row 2b), which a
/// consumer checks before reading the table (`string-table.md` §9.1).
#[must_use]
pub const fn is_string_table_did(did: u32) -> bool {
    did >> 24 == 0x25
}

/// What went wrong while finding or opening the install: a kind only, never a path of
/// the user (`official-entry.md` §7.3 "Never on disk or in logs" applies to logs of the
/// install's location too: engineering choice, spec silent (question with the spec
/// writers)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InstallProblem {
    /// No usable install was found.
    #[error("no usable install was found")]
    NotFound,
    /// The archives could not be opened.
    #[error("the install's archives cannot be opened")]
    Archives,
    /// A record could not be read.
    #[error("a record of the install cannot be read")]
    Record,
    /// The property dictionary could not be decoded.
    #[error("the install's property dictionary cannot be decoded")]
    Properties,
}

/// The install the client found: its root and, when the client knows it, the directory
/// of the original's `UserPreferences.ini` (inside a Wine, Proton or CrossOver prefix for
/// an install there, `install-layout.md` §8.1).
#[derive(Debug, Clone, Default)]
pub struct LocatedInstall {
    /// The install root.
    pub root: PathBuf,
    /// The settings directory, if known.
    pub user_settings_dir: Option<PathBuf>,
}

/// Read-only records and tables of the user's install, supplied by the client.
///
/// Everything here is data of the user's own install, read at run time (decision 0034 §3,
/// `ARCHITECTURE.md` §19.3); nothing is built in.
pub trait InstallData: Send + Sync + std::fmt::Debug {
    /// The payload of the archive record `did`, or `None` when the install has none or it
    /// is longer than `max_bytes`. The bound is applied while reading (a larger record is
    /// never read in full).
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>>;

    /// The property type of `pid` in the master property dictionary (`0x34000000`,
    /// `official-messages-world.md` §2.7.1), or `None` when the dictionary has none.
    fn property_type(&self, pid: u32) -> Option<PropertyType>;

    /// The `0xCC` state schema `id` (`official-messages-world.md` §4.3.1), when the client
    /// built it from the install. The default knows none: `0xCC` events then fail to
    /// decode and are counted.
    fn portal_state_schema(&self, _id: u32) -> Option<&PortalStateSchema> {
        None
    }

    /// The display text of a `LocalizedTokenText`. The default resolves literal texts only;
    /// the client's resolves references through the install's string tables
    /// (`string-table.md` §9).
    fn resolve_text(&self, text: &LocalizedTokenText) -> Option<String> {
        LiteralTexts.resolve(text)
    }

    /// The value of `pid` in the baseline property set of the game object `template_did`
    /// (`template_did` + `0x09000000`, `property-system.md` §6.2;
    /// `official-messages-world.md` §6 row `0x98 + 0x52`), in the wire value form, or
    /// `None` when the install has none (or the value has no wire form the caller reads:
    /// texts with arguments, nested tables). The default knows none.
    fn template_value(&self, _template_did: u32, _pid: u32) -> Option<EncodedPropertyValue> {
        None
    }

    /// Whether [`Self::template_value`] for `template_did` answers without reading a record
    /// (its baseline set is decoded and kept). The entity feed reads at most a few uncached
    /// templates per pass ([`super::entities::MAX_TEMPLATE_LOADS_PER_PASS`]), so a server that
    /// cycles templates cannot hold the session thread. The default (no records behind the
    /// lookups) is `true`.
    fn template_cached(&self, _template_did: u32) -> bool {
        true
    }

    /// Whether resolving a reference into the string table `table_did` reads nothing from
    /// the archives (the table is decoded and kept, or the DID is not a string table and is
    /// refused unread, [`is_string_table_did`]). The entity feed resolves at most a few names
    /// in uncached tables per pass ([`super::entities::MAX_TEXT_LOADS_PER_PASS`]). The default
    /// (no tables behind the lookups) is `true`.
    fn text_cached(&self, _table_did: u32) -> bool {
        true
    }

    /// Every key of the chat text category enum (`0x230002A8` with its base chain,
    /// `official-messages-play.md` §6.7), or `None` when the client did not read it (the
    /// default). [`chat_categories`] keeps only the categories the spec names that are
    /// among these keys.
    fn chat_category_keys(&self) -> Option<Vec<u32>> {
        None
    }
}

/// The chat categories of the session (`official-messages-play.md` §6.7): the numbers the
/// spec names (Say `5`, Tell `6`), each kept only when the install's chat text category enum
/// has it ([`InstallData::chat_category_keys`]). Without the install's enum the spec's numbers
/// are used as they are. Yell and Fellowship stay unmapped (their numbers are OPEN, §6.7):
/// lines in other categories are counted by the play layer, and the user channel slots are
/// the chat server's rooms, which carry their names (`crate::chat_session`).
#[must_use]
pub fn chat_categories(data: &dyn InstallData) -> ChatCategories {
    let Some(keys) = data.chat_category_keys() else {
        tracing::debug!("official session: no chat category enum from the install; spec numbers");
        return ChatCategories::default();
    };
    let mut c = ChatCategories::empty();
    if keys.contains(&CATEGORY_SAY) {
        c.set(CATEGORY_SAY, ChatChannel::Say);
    }
    if keys.contains(&CATEGORY_TELL) {
        c.set(
            CATEGORY_TELL,
            ChatChannel::Tell {
                peer: String::new(),
            },
        );
    }
    c
}

/// Makes the [`InstallData`] of the install root it is given, for the `<Lang>` token the
/// login selected (`official-login.md` §2.2: the language whose string tables texts resolve
/// through), on the connector's thread (opening the archives can take a while; it never
/// runs on the frame thread).
pub type InstallDataLoader =
    Box<dyn FnOnce(&Path, &str) -> Result<Arc<dyn InstallData>, InstallProblem> + Send>;

/// Finds the install on the connector's thread (install discovery can scan the disk).
pub type InstallLocator = Box<dyn FnOnce() -> Result<LocatedInstall, InstallProblem> + Send>;

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

/// Where the connector looks on this machine, and the values the user supplied (all given
/// by the client; nothing found here is written to).
#[derive(Debug, Clone, Default)]
pub struct LocalPaths {
    /// The install root (`<InstallRoot>`, `official-login.md` §2).
    pub install_root: PathBuf,
    /// The directory of `UserPreferences*.ini`, when the client knows it (§2.3); without
    /// one, `<Documents>/<Product.DocumentFolder>/` of this machine is tried.
    pub user_settings_dir: Option<PathBuf>,
    /// A network version string the user supplied (§2.4.1 step 1), from the client's
    /// settings or command line.
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
    /// The `[Net]` keys of `UserPreferences.ini` (`official-session.md` §3.2 item 1), the
    /// defaults without a preferences file.
    pub net: NetPreferences,
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
    let bytes = data.record(did, MAX_RECORD_BYTES).ok_or_else(|| {
        invalid("a record the session needs is missing from the install or too large")
    })?;
    // The supplier promised the bound; a larger answer is refused all the same.
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(invalid("a record the session needs is too large"));
    }
    Ok(bytes)
}

/// One dictionary resolved through Master DID Map `3` with at most one redirect
/// (`official-session.md` §4.1.3 item 1; invariant `I16`).
fn dictionary(
    data: &dyn InstallData,
    map: &DataIdMap,
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
    let map = DataIdMap::parse(&sub)?;
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
    if prefs.is_none() {
        // Once per login attempt; no path is named.
        tracing::debug!("official login: no preferences file; the install's language is used");
    }
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
    let data = loader(root, &language).map_err(|why| {
        tracing::warn!(problem = %why, "official login: the install's records cannot be read");
        invalid("the install's records cannot be read")
    })?;
    let (codecs, codec_uuids) = load_codecs(root, data.as_ref())?;
    let net = prefs.map(|p| p.net).unwrap_or_default();
    Ok(LocalInputs {
        config,
        language,
        version,
        codecs,
        codec_uuids,
        net,
        data,
    })
}
