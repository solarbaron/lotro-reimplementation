// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's records for the live official connector (`official-login.md` §2.4.3,
//! `official-session.md` §4.1.3, `official-messages-world.md` §2.7): the archives opened
//! read-only and the master property dictionary (`0x34000000`) as the wire property schema,
//! and from them the tables the world session reads at run time:
//!
//! - the `0xCC` state schemas, built from the player-physics script table `0x07000029` and
//!   its `0x0C` scriptlets by the construction algorithm of `official-messages-world.md`
//!   §4.3.1 ([`portal_state_schemas`]);
//! - the texts of `LocalizedTokenText` references, through the string tables of the
//!   language the login selected (`string-table.md` §9). A message with value slots is
//!   shown with its literal parts and counted ([`ArchiveData::texts_without_values`]): the
//!   wire form of arguments is OPEN (`official-messages-world.md` §2.6), so no value can
//!   fill them;
//! - the baseline properties of an entity's template game object (`property-system.md`
//!   §6.2; `official-messages-world.md` §6 row `0x98 + 0x52`);
//! - the keys of the chat text category enum `0x230002A8` and its base chain
//!   (`official-messages-play.md` §6.7).
//!
//! [`loader`] and [`locator`] run on the connector's thread (finding the install, opening
//! the archives, decoding the dictionary and building the schemas take a while; the frame
//! thread never waits for them, ARCH §21). Their errors are kinds ([`InstallProblem`]): no
//! path of the user reaches a log line from here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_compat::connector::{
    InstallData, InstallDataLoader, InstallLocator, InstallProblem, LocatedInstall,
};
use tessera_compat::world::{
    EncodedPropertyValue, LocalizedTokenText, LocalizedTokenTextContent, PortalStateSchema,
    PropertyType,
};
use tessera_dat::{ArchiveSet, SetOptions, inflate_record};
use tessera_formats::property::{PropertyDictionary, PropertyValue};
use tessera_formats::property_set::{PropertySet, PropertySetRecord, object_property_set_did};
use tessera_formats::script_table::{Node, NodeData, NodeKind, ScriptTable, Scriptlet};
use tessera_formats::{Did, EnumMapper, Record as _, StringSource};
use tessera_props::{PROPERTY_DICTIONARY_DID, PropertyRegistry};
use tessera_text::{RenderOptions, Strings, TableSource, Values};

/// The player-physics script table whose scripts define the `0xCC` state schemas
/// (`official-messages-world.md` §4.3.1, W3).
pub const PLAYER_PHYSICS_TABLE: u32 = 0x0700_0029;

/// The chat text category enum (`official-messages-play.md` §6.7; its base `0x230002A9` is
/// found through the record's own base link).
pub const CHAT_CATEGORY_ENUM: u32 = 0x2300_02A8;

/// Longest record read for the session's tables (script tables, scriptlets, property sets,
/// string tables, enum mappers). Engineering choice.
pub const MAX_TABLE_RECORD: usize = 16 * 1024 * 1024;

/// Most tables of the player-physics closure visited. Engineering choice (an avatar closure
/// has about 150, `script-table.md` §8.2).
pub const MAX_CLOSURE_TABLES: usize = 512;

/// Most nodes visited while building all schemas. Engineering choice.
pub const MAX_SCHEMA_NODES: usize = 2_000_000;

/// Deepest nesting followed while building a schema (nodes inside ports and scriptlets).
/// Engineering choice: the data stays far below it (`script-table.md` §4.5).
pub const MAX_SCHEMA_DEPTH: usize = 1024;

/// Most template property sets kept decoded (the cache is emptied when full). Engineering
/// choice.
pub const MAX_CACHED_TEMPLATES: usize = 1024;

/// The 4-byte inflated-length prefix a compressed record's stored `length` includes
/// (`dat-container.md` §5.2).
const COMPRESSED_PREFIX: u64 = 4;

/// The payload of record `did`, at most `max_bytes` long (a larger record is not read).
fn bounded_record(set: &ArchiveSet, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
    let located = set.locate(did).ok()??;
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
    let member = set.archive_of(&located)?;
    let stored = member.archive.read_raw(entry).ok()?;
    if !entry.is_compressed() {
        return (stored.len() <= max_bytes).then_some(stored);
    }
    // Inflation is bounded by the same limit.
    inflate_record(&stored, max).ok()
}

/// The string tables of the set (its language member serves the `0x25` records).
struct SetTables(Arc<ArchiveSet>);

impl TableSource for SetTables {
    fn read_table(&self, did: u32) -> tessera_text::Result<Option<Vec<u8>>> {
        Ok(bounded_record(&self.0, did, MAX_TABLE_RECORD))
    }

    /// Not needed by the session (it looks tables up by DID only): none listed.
    fn table_dids(&self) -> tessera_text::Result<Vec<u32>> {
        Ok(Vec::new())
    }
}

/// The archives, the property dictionary and the session's tables of one install.
pub struct ArchiveData {
    set: Arc<ArchiveSet>,
    registry: PropertyRegistry,
    strings: Strings,
    schemas: BTreeMap<u32, PortalStateSchema>,
    chat_keys: Option<Vec<u32>>,
    templates: Mutex<HashMap<u32, Option<Arc<PropertySet>>>>,
    texts_without_values: AtomicU64,
}

impl std::fmt::Debug for ArchiveData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveData")
            .field("archives", &self.set.members().len())
            .field("properties", &self.registry.len())
            .field("schemas", &self.schemas.len())
            .field("chat_keys", &self.chat_keys.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

impl ArchiveData {
    /// The `0xCC` state schemas built from the install, by `state_schema_id`.
    #[must_use]
    pub fn schemas(&self) -> &BTreeMap<u32, PortalStateSchema> {
        &self.schemas
    }

    /// Texts shown with only their literal parts: messages with value slots, which no
    /// wire argument can fill yet (`official-messages-world.md` §2.6 OPEN).
    #[must_use]
    pub fn texts_without_values(&self) -> u64 {
        self.texts_without_values.load(Ordering::Relaxed)
    }

    /// The baseline property set of the game object `template`, decoded once.
    fn template_set(&self, template: u32) -> Option<Arc<PropertySet>> {
        let mut cache = self
            .templates
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(s) = cache.get(&template) {
            return s.clone();
        }
        let set = object_property_set_did(template).and_then(|did| {
            let bytes = bounded_record(&self.set, did.0, MAX_TABLE_RECORD)?;
            PropertySetRecord::parse_expecting(&bytes, did, self.registry.dictionary())
                .ok()
                .map(|r| Arc::new(r.set))
        });
        if cache.len() >= MAX_CACHED_TEMPLATES {
            cache.clear();
        }
        cache.insert(template, set.clone());
        set
    }
}

/// A stored property value in the wire value form, for the kinds the session reads
/// (`official-messages-world.md` §2.7.1: the same value in either form); `None` for the
/// others and for texts with arguments.
fn wire_value(v: &PropertyValue) -> Option<EncodedPropertyValue> {
    Some(match v {
        PropertyValue::String(s) => EncodedPropertyValue::String(s.clone()),
        PropertyValue::StringToken(t) => EncodedPropertyValue::StringToken(*t),
        PropertyValue::Enum(e) => EncodedPropertyValue::Enum(*e),
        PropertyValue::Float(f) => EncodedPropertyValue::Float(*f),
        PropertyValue::Int(i) => EncodedPropertyValue::Int(*i),
        PropertyValue::Int64(i) => EncodedPropertyValue::Int64(*i),
        PropertyValue::Bool(b) => EncodedPropertyValue::Boolean(*b),
        PropertyValue::DidRef(d) => EncodedPropertyValue::DidRef(d.0),
        PropertyValue::Bitfield32(b) => EncodedPropertyValue::Bitfield32(*b),
        PropertyValue::Bitfield64(b) => EncodedPropertyValue::Bitfield64(*b),
        PropertyValue::InstanceId(i) => EncodedPropertyValue::InstanceId(*i),
        PropertyValue::StringInfo(info) => {
            if info.args.as_ref().is_some_and(|a| !a.values.is_empty()) {
                return None;
            }
            EncodedPropertyValue::LocalizedTokenText(match &info.source {
                StringSource::Literal(s) => LocalizedTokenText::literal(s.to_string_lossy()),
                StringSource::Table { token, table } => {
                    LocalizedTokenText::reference(*token, table.0)
                }
            })
        }
        _ => return None,
    })
}

impl InstallData for ArchiveData {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        bounded_record(&self.set, did, max_bytes)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        let t = self.registry.property_type(pid)?;
        PropertyType::from_code(t.code()).ok()
    }

    fn portal_state_schema(&self, id: u32) -> Option<&PortalStateSchema> {
        self.schemas.get(&id)
    }

    /// `string-table.md` §9: a literal is its text; a reference is looked up in the
    /// selected language's tables and rendered (display mode). A miss, or a text that
    /// shows nothing (the emptiness test of §9.0 step 2), is `None`: nothing is shown and
    /// the caller counts it. A message with value slots renders its literal parts
    /// ([`ArchiveData::texts_without_values`]).
    fn resolve_text(&self, text: &LocalizedTokenText) -> Option<String> {
        let (token, table) = match &text.content {
            LocalizedTokenTextContent::Literal(s) => return Some(s.clone()),
            LocalizedTokenTextContent::Reference {
                token_id,
                table_did,
                ..
            } => (*token_id, *table_did),
        };
        if token == 0 || table == 0 {
            return None;
        }
        let msg = self.strings.message(table, token).ok()?;
        if !msg.slots().is_empty() {
            self.texts_without_values.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(
                table = format_args!("{table:#010x}"),
                "official text with value slots shown without its values"
            );
        }
        let rendered =
            self.strings
                .render_entry(table, token, &Values::new(), &RenderOptions::default());
        if rendered.miss().is_some() {
            return None;
        }
        let s = rendered.plain_text();
        (!s.trim().is_empty()).then_some(s)
    }

    fn template_value(&self, template_did: u32, pid: u32) -> Option<EncodedPropertyValue> {
        let set = self.template_set(template_did)?;
        wire_value(set.get(pid)?)
    }

    fn chat_category_keys(&self) -> Option<Vec<u32>> {
        self.chat_keys.clone()
    }
}

/// Every key of the enum mapper `start` and its base chain (`records.md` §7), `None` when
/// `start` is missing or does not decode.
fn enum_keys(set: &ArchiveSet, start: u32) -> Option<Vec<u32>> {
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    let mut did = Did(start);
    for _ in 0..tessera_formats::enum_mapper::ENUM_BASE_CHAIN_LIMIT {
        if did.is_null() || !seen.insert(did) {
            break;
        }
        let bytes = bounded_record(set, did.0, MAX_TABLE_RECORD);
        let mapper = bytes.and_then(|b| EnumMapper::parse_expecting(&b, did).ok());
        let Some(m) = mapper else {
            if did.0 == start {
                return None;
            }
            break;
        };
        keys.extend(m.names().iter().map(|(k, _)| *k));
        did = m.base;
    }
    keys.sort_unstable();
    keys.dedup();
    Some(keys)
}

/// The builder of the `0xCC` state schemas (`official-messages-world.md` §4.3.1).
struct SchemaBuilder<'a> {
    set: &'a ArchiveSet,
    dict: &'a PropertyDictionary,
    registry: &'a PropertyRegistry,
    scriptlets: HashMap<u32, Option<Arc<Scriptlet>>>,
    visited: usize,
    truncated: bool,
}

impl SchemaBuilder<'_> {
    fn table(&self, did: Did) -> Option<ScriptTable> {
        let bytes = bounded_record(self.set, did.0, MAX_TABLE_RECORD)?;
        ScriptTable::parse_expecting(&bytes, did, self.dict).ok()
    }

    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        if let Some(s) = self.scriptlets.get(&did.0) {
            return s.clone();
        }
        let s = bounded_record(self.set, did.0, MAX_TABLE_RECORD)
            .and_then(|b| Scriptlet::parse_expecting(&b, did, self.dict).ok())
            .map(Arc::new);
        self.scriptlets.insert(did.0, s.clone());
        s
    }

    /// The closure of `root`: the table and every table reachable through its includes,
    /// in pre-order, depth first, in stored include order, each once (`script-table.md`
    /// §8.2).
    fn closure(&self, root: Did) -> Vec<ScriptTable> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![root];
        while let Some(did) = stack.pop() {
            if out.len() >= MAX_CLOSURE_TABLES || did.is_null() || !seen.insert(did) {
                continue;
            }
            let Some(t) = self.table(did) else {
                continue;
            };
            // Reversed, so the first include is visited first.
            stack.extend(t.includes.iter().rev().copied());
            out.push(t);
        }
        out
    }

    /// Step 4: a condition PID goes to the booleans (type 21) or the enums (type 8), once.
    fn condition(&self, pid: u32, schema: &mut PortalStateSchema) {
        let Some(t) = self.registry.property_type(pid) else {
            return;
        };
        let list = match PropertyType::from_code(t.code()) {
            Ok(PropertyType::Boolean) => &mut schema.bool_pids,
            Ok(PropertyType::Enum) => &mut schema.enum_pids,
            _ => return,
        };
        if !list.contains(&pid) {
            list.push(pid);
        }
    }

    /// Steps 2–3: `node` in pre-order: the node, then (for a Scriptlet node) the roots of
    /// the scriptlet it names, then the children of its ports.
    fn visit(
        &mut self,
        node: &Node,
        depth: usize,
        stack: &mut Vec<u32>,
        out: &mut PortalStateSchema,
    ) {
        if self.visited >= MAX_SCHEMA_NODES || depth >= MAX_SCHEMA_DEPTH {
            self.truncated = true;
            return;
        }
        self.visited += 1;
        match &node.data {
            NodeData::Switch(sw)
                if node.kind == NodeKind::SWITCH || node.kind == NodeKind::WHEN =>
            {
                self.condition(sw.property, out);
            }
            NodeData::Scriptlet { file, .. } if !file.is_null() => {
                // A scriptlet that invokes itself (directly or through others) is not
                // entered again.
                if !stack.contains(&file.0)
                    && let Some(s) = self.scriptlet(*file)
                {
                    stack.push(file.0);
                    for r in &s.roots {
                        self.visit(r, depth + 1, stack, out);
                    }
                    stack.pop();
                }
            }
            _ => {}
        }
        for port in &node.ports {
            for child in &port.children {
                self.visit(child, depth + 1, stack, out);
            }
        }
    }
}

/// The `0xCC` state schemas of the install (`official-messages-world.md` §4.3.1): for each
/// script key of the player-physics table [`PLAYER_PHYSICS_TABLE`], the condition PIDs of
/// the Switch (12) and When (63) nodes of every definition of that key in the table's
/// closure (in closure order, `script-table.md` §8.2), visited in pre-order through ports
/// and the scriptlets they name. Empty when the table is missing or does not decode.
#[must_use]
pub fn portal_state_schemas(
    set: &ArchiveSet,
    registry: &PropertyRegistry,
) -> BTreeMap<u32, PortalStateSchema> {
    let mut b = SchemaBuilder {
        set,
        dict: registry.dictionary(),
        registry,
        scriptlets: HashMap::new(),
        visited: 0,
        truncated: false,
    };
    let closure = b.closure(Did(PLAYER_PHYSICS_TABLE));
    let Some(root) = closure.first() else {
        tracing::warn!("official session: the player-physics script table is missing");
        return BTreeMap::new();
    };
    let keys: Vec<u32> = root.scripts.iter().map(|s| s.key).collect();
    let mut out = BTreeMap::new();
    for key in keys {
        let mut schema = PortalStateSchema::default();
        let mut stack = Vec::new();
        for table in &closure {
            if let Some(script) = table.script(key) {
                for r in &script.roots {
                    b.visit(r, 0, &mut stack, &mut schema);
                }
            }
        }
        out.insert(key, schema);
    }
    if b.truncated {
        tracing::warn!("official session: a state schema hit its traversal bound");
    }
    tracing::debug!(schemas = out.len(), "official session: state schemas built");
    out
}

/// Opens the install at `root` on the calling thread, with the string tables of `language`.
fn open(root: &Path, language: &str) -> Result<ArchiveData, InstallProblem> {
    let options = SetOptions {
        language: Some(language.to_owned()),
        ..SetOptions::default()
    };
    let set = Arc::new(ArchiveSet::open(root, options).map_err(|_| InstallProblem::Archives)?);
    let dict = set
        .read(PROPERTY_DICTIONARY_DID.0)
        .map_err(|_| InstallProblem::Record)?;
    let registry = PropertyRegistry::parse(&dict).map_err(|_| InstallProblem::Properties)?;
    let schemas = portal_state_schemas(&set, &registry);
    let chat_keys = enum_keys(&set, CHAT_CATEGORY_ENUM);
    if chat_keys.is_none() {
        tracing::warn!("official session: the chat category enum is missing");
    }
    let strings = Strings::new(SetTables(Arc::clone(&set)));
    Ok(ArchiveData {
        set,
        registry,
        strings,
        schemas,
        chat_keys,
        templates: Mutex::new(HashMap::new()),
        texts_without_values: AtomicU64::new(0),
    })
}

/// Opens the install at `root` with the string tables of `language` (tests and tools; the
/// connector uses [`loader`]).
///
/// # Errors
///
/// [`InstallProblem`] when the archives or the property dictionary cannot be used.
pub fn install_data_at(root: &Path, language: &str) -> Result<ArchiveData, InstallProblem> {
    open(root, language)
}

/// The connector's loader of the install's records.
pub fn loader() -> InstallDataLoader {
    Box::new(|root, language| open(root, language).map(|d| Arc::new(d) as Arc<dyn InstallData>))
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
