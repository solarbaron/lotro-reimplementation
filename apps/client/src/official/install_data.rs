// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's records for the live official connector (`official-login.md` §2.4.3,
//! `official-session.md` §4.1.3, `official-messages-world.md` §2.7): the archives opened
//! read-only and the master property dictionary (`0x34000000`) as the wire property schema,
//! and from them the tables the world session reads at run time:
//!
//! - the `0xCC` state schemas, built by the construction algorithm of
//!   `official-messages-world.md` §4.3.1 over the avatar closure that step 1 names
//!   ([`portal_state_schemas`]);
//! - the texts of `LocalizedTokenText` values in the language the login selected
//!   (`string-table.md` §9.0): a literal with the display cut and its escapes decoded
//!   (step 1), a reference through the string tables. A message with value slots is shown
//!   with its literal parts and counted ([`ArchiveData::texts_without_values`]): the wire
//!   form of arguments is OPEN (`official-messages-world.md` §2.6), so no value can fill
//!   them. Each text the session shows is resolved once (the entity feed renders a name at
//!   the creation only), so the count is one per text shown. A reference whose table DID
//!   lacks the `0x25` prefix is refused before anything is read (§2.6 row 2b,
//!   `string-table.md` §9.1), and [`InstallData::text_cached`] tells the entity feed which
//!   tables are decoded already, so it can bound the table reads of one pass;
//! - the baseline properties of an entity's template game object (`property-system.md`
//!   §6.2; `official-messages-world.md` §6 row `0x98 + 0x52`), decoded on demand and kept
//!   for the [`MAX_CACHED_TEMPLATES`] used last;
//! - the keys of the chat text category enum `0x230002A8` and its base chain
//!   (`official-messages-play.md` §6.7).
//!
//! The in-game calendar (`official-messages-world.md` §4.4.2) is not read here: the spec's
//! lookup (key `0x96` of directory `0x0F000002`) contradicts `environment.md` §3, which
//! makes `0x0F000002` region 3's descriptor (question in `docs/m8-remediation.md` §15). A
//! caller of `tessera_compat::world::WorldCalendarConfig` keeps supplying the record DID
//! itself until the spec says how to find it (deferred).
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
use tessera_formats::script_table::resolve::{
    COMBINATION_OVERRIDING, RUN_LIST_KINDS, ScriptVocabulary,
};
use tessera_formats::script_table::{Node, NodeData, NodeKind, ScriptTable, Scriptlet};
use tessera_formats::{Did, EnumMapper, Record as _, StringSource};
use tessera_props::{PROPERTY_DICTIONARY_DID, PropertyRegistry};
use tessera_text::{
    CacheLimits, RenderOptions, Strings, TableSource, Values, decode_escapes, is_string_table_did,
};

/// The player-physics script table whose scripts define the `0xCC` state schemas
/// (`official-messages-world.md` §4.3.1, W3): its script keys are the schema IDs.
pub const PLAYER_PHYSICS_TABLE: u32 = 0x0700_0029;

/// The roots of the script-table closure the schemas are gathered over: the avatar table
/// and the global root (`official-messages-world.md` §4.3.1 step 1).
pub const SCHEMA_CLOSURE_ROOTS: [u32; 2] = [0x0700_0417, 0x0700_0000];

/// The script-key enum mapper (its base chain followed) whose keys a `state_schema_id` is
/// one of (`official-messages-world.md` §4.3.1: mapper `0x23000097` / `0x230000F8`).
///
/// Tessera's reading of §4.3.1: a script key of [`PLAYER_PHYSICS_TABLE`] is a schema only
/// when it is a key of `0x23000097`, whose base chain reaches `0x230000F8` (the spec names
/// both mappers but not how they filter the keys; question in `docs/m8-remediation.md` §15
/// (W §4.3.1)).
pub const SCRIPT_KEY_ENUM: u32 = 0x2300_0097;

/// The chat text category enum (`official-messages-play.md` §6.7; its base `0x230002A9` is
/// found through the record's own base link).
pub const CHAT_CATEGORY_ENUM: u32 = 0x2300_02A8;

/// Longest record read for the session's tables (script tables, scriptlets, property sets,
/// string tables, enum mappers). Engineering choice.
pub const MAX_TABLE_RECORD: usize = 16 * 1024 * 1024;

/// Most tables of the player-physics closure visited. Engineering choice (an avatar closure
/// has about 150, `script-table.md` §8.2).
pub const MAX_CLOSURE_TABLES: usize = 512;

/// Most nodes visited while building one schema; a schema that hits it is left out, so a
/// `0xCC` naming it is refused rather than misframed. Engineering choice.
pub const MAX_SCHEMA_NODES: usize = 1_000_000;

/// Most nodes visited while building every schema of the install, over all keys, so a
/// crafted install costs at most this many visits rather than keys × [`MAX_SCHEMA_NODES`].
/// A key whose build would pass it is left out (a `0xCC` naming it is refused), and so is
/// every key after it. Engineering choice.
pub const MAX_SCHEMA_TOTAL_NODES: usize = 4_000_000;

/// Deepest nesting followed while building a schema (nodes inside ports and scriptlets).
/// Engineering choice: the data stays far below it (`script-table.md` §4.5).
pub const MAX_SCHEMA_DEPTH: usize = 1024;

/// Most template property sets kept decoded; beyond it the one used longest ago is
/// dropped. Engineering choice.
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

/// The display cut of `string-table.md` §7.6: the text before its first `[` not directly
/// preceded by a backslash, without trailing whitespace (space, LF, CR, TAB, form feed);
/// the whole text when it has no such `[`.
#[must_use]
pub fn display_cut(text: &str) -> &str {
    let bytes = text.as_bytes();
    let first = bytes
        .iter()
        .enumerate()
        .find(|&(i, &b)| b == b'[' && (i == 0 || bytes.get(i - 1) != Some(&b'\\')))
        .map(|(i, _)| i);
    match first.and_then(|i| text.get(..i)) {
        Some(head) => head.trim_end_matches([' ', '\n', '\r', '\t', '\u{0C}']),
        None => text,
    }
}

/// The string tables the [`Strings`] cache has read from the archives (every read is a
/// cache miss), for [`InstallData::text_cached`].
#[derive(Debug, Default)]
struct ServedTables {
    /// The DIDs read, at most [`CacheLimits::max_tables`] (the cache holds no more).
    dids: HashSet<u32>,
    /// Reads made: more than `dids.len()` once a table was read twice (it had been
    /// evicted) or the set was full.
    reads: u64,
}

impl ServedTables {
    /// Whether `did` is in a cache that now holds `cached` tables: it was read, and the
    /// cache never evicted one (it holds as many tables as were read, each read once).
    fn holds(&self, did: u32, cached: usize) -> bool {
        self.dids.contains(&did)
            && u64::try_from(self.dids.len()).is_ok_and(|n| n == self.reads)
            && cached == self.dids.len()
    }
}

/// A literal's text as shown (`string-table.md` §9.0 step 1: the display cut, then the
/// escapes decoded), `None` when it shows nothing (the emptiness test of step 2).
fn shown_literal(s: &str) -> Option<String> {
    let shown = decode_escapes(display_cut(s));
    (!shown.trim().is_empty()).then_some(shown)
}

/// The `(token, table)` to look a reference up by, `None` when nothing is to be read: a
/// zero token, or a table DID without the `0x25` prefix (`official-messages-world.md` §2.6
/// row 2b, `string-table.md` §9.1), the null DID included.
fn lookup_key(token: u32, table: u32) -> Option<(u32, u32)> {
    (token != 0 && is_string_table_did(table)).then_some((token, table))
}

/// The string tables of the set (its language member serves the `0x25` records).
struct SetTables {
    set: Arc<ArchiveSet>,
    served: Arc<Mutex<ServedTables>>,
}

impl TableSource for SetTables {
    fn read_table(&self, did: u32) -> tessera_text::Result<Option<Vec<u8>>> {
        {
            let mut served = self.served.lock().unwrap_or_else(PoisonError::into_inner);
            served.reads += 1;
            if served.dids.len() < CacheLimits::default().max_tables {
                served.dids.insert(did);
            }
        }
        Ok(bounded_record(&self.set, did, MAX_TABLE_RECORD))
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
    served: Arc<Mutex<ServedTables>>,
    schemas: BTreeMap<u32, PortalStateSchema>,
    chat_keys: Option<Vec<u32>>,
    templates: Mutex<TemplateCache>,
    texts_without_values: AtomicU64,
}

/// The decoded template sets, the [`MAX_CACHED_TEMPLATES`] used last.
#[derive(Debug, Default)]
struct TemplateCache {
    sets: HashMap<u32, (Option<Arc<PropertySet>>, u64)>,
    /// Use stamp to template.
    order: BTreeMap<u64, u32>,
    clock: u64,
}

impl TemplateCache {
    fn get(&mut self, template: u32) -> Option<Option<Arc<PropertySet>>> {
        let (set, stamp) = self.sets.get_mut(&template)?;
        self.order.remove(stamp);
        self.clock += 1;
        *stamp = self.clock;
        self.order.insert(self.clock, template);
        Some(set.clone())
    }

    fn insert(&mut self, template: u32, set: Option<Arc<PropertySet>>) {
        if !self.sets.contains_key(&template)
            && self.sets.len() >= MAX_CACHED_TEMPLATES
            && let Some((_, oldest)) = self.order.pop_first()
        {
            self.sets.remove(&oldest);
        }
        self.clock += 1;
        if let Some((_, old)) = self.sets.insert(template, (set, self.clock)) {
            self.order.remove(&old);
        }
        self.order.insert(self.clock, template);
    }
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

    /// The baseline property set of the game object `template`, decoded once while it
    /// stays among the [`MAX_CACHED_TEMPLATES`] used last.
    fn template_set(&self, template: u32) -> Option<Arc<PropertySet>> {
        let mut cache = self
            .templates
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(s) = cache.get(template) {
            return s;
        }
        let set = object_property_set_did(template).and_then(|did| {
            let bytes = bounded_record(&self.set, did.0, MAX_TABLE_RECORD)?;
            PropertySetRecord::parse_expecting(&bytes, did, self.registry.dictionary())
                .ok()
                .map(|r| Arc::new(r.set))
        });
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

    /// `string-table.md` §9: a literal is its text in display mode (§9.0 step 1: the
    /// display cut of §7.6, then the escapes of §7.7 decoded); a reference is looked up in
    /// the selected language's tables and rendered (display mode). A miss, a reference
    /// whose table DID lacks the `0x25` prefix (refused before any read:
    /// `official-messages-world.md` §2.6 row 2b, `string-table.md` §9.1), or a text that
    /// shows nothing (the emptiness test of §9.0 step 2, literals included) is `None`:
    /// nothing is shown and the caller counts it. A message with value slots renders its
    /// literal parts ([`ArchiveData::texts_without_values`]).
    fn resolve_text(&self, text: &LocalizedTokenText) -> Option<String> {
        let (token, table) = match &text.content {
            LocalizedTokenTextContent::Literal(s) => return shown_literal(s),
            LocalizedTokenTextContent::Reference {
                token_id,
                table_did,
                ..
            } => lookup_key(*token_id, *table_did)?,
        };
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

    fn template_cached(&self, template_did: u32) -> bool {
        self.templates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .sets
            .contains_key(&template_did)
    }

    fn chat_category_keys(&self) -> Option<Vec<u32>> {
        self.chat_keys.clone()
    }

    /// `true` when looking up `table_did` reads nothing: it is not a string-table DID (it is
    /// refused unread), or the cache holds it. The cache is known to hold every table it
    /// read as long as it never evicted one: then it holds exactly as many tables as were
    /// read, each read once. After an eviction (or once the record of reads is full) this
    /// answers `false` for every string table, so each lookup counts against the caller's
    /// budget (conservative).
    fn text_cached(&self, table_did: u32) -> bool {
        if !is_string_table_did(table_did) {
            return true;
        }
        let served = self.served.lock().unwrap_or_else(PoisonError::into_inner);
        served.holds(table_did, self.strings.cache_stats().tables)
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

/// Reads one record of the install, bounded by [`MAX_TABLE_RECORD`].
type RecordReader<'a> = &'a dyn Fn(Did) -> Option<Vec<u8>>;

/// The builder of the `0xCC` state schemas (`official-messages-world.md` §4.3.1).
struct SchemaBuilder<'a> {
    read: RecordReader<'a>,
    dict: &'a PropertyDictionary,
    registry: &'a PropertyRegistry,
    scriptlets: HashMap<u32, Option<Arc<Scriptlet>>>,
    /// `Script_CombinationType` and its `Overriding` key (`script-table.md` §8.2), when
    /// the install's vocabulary has them; without them every definition is additive.
    combination: Option<(u32, u32)>,
    /// Most nodes one key may visit ([`MAX_SCHEMA_NODES`]).
    per_key: usize,
    /// Nodes the remaining keys may visit together ([`MAX_SCHEMA_TOTAL_NODES`]).
    total_left: usize,
    /// The bound of the key being built: the smaller of the two above.
    limit: usize,
    visited: usize,
    truncated: bool,
    /// The keys left out at a bound, in build order.
    left_out: Vec<u32>,
}

/// A script-table closure: its tables in pre-order and their positions by DID.
#[derive(Default)]
struct Closure {
    tables: Vec<ScriptTable>,
    index: HashMap<Did, usize>,
}

impl<'a> SchemaBuilder<'a> {
    fn new(
        read: RecordReader<'a>,
        registry: &'a PropertyRegistry,
        combination: Option<(u32, u32)>,
    ) -> Self {
        Self {
            read,
            dict: registry.dictionary(),
            registry,
            scriptlets: HashMap::new(),
            combination,
            per_key: MAX_SCHEMA_NODES,
            total_left: MAX_SCHEMA_TOTAL_NODES,
            limit: 0,
            visited: 0,
            truncated: false,
            left_out: Vec::new(),
        }
    }

    fn table(&self, did: Did) -> Option<ScriptTable> {
        let bytes = (self.read)(did)?;
        ScriptTable::parse_expecting(&bytes, did, self.dict).ok()
    }

    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        if let Some(s) = self.scriptlets.get(&did.0) {
            return s.clone();
        }
        let s = (self.read)(did)
            .and_then(|b| Scriptlet::parse_expecting(&b, did, self.dict).ok())
            .map(Arc::new);
        self.scriptlets.insert(did.0, s.clone());
        s
    }

    /// The closure of `roots`: the tables and every table reachable through their
    /// includes, in pre-order, depth first, in stored include order, each once; null
    /// includes and includes naming their own table are skipped (`script-table.md` §8.2).
    fn closure(&self, roots: &[Did]) -> Closure {
        let mut out = Closure::default();
        let mut stack: Vec<Did> = roots.iter().rev().copied().collect();
        while let Some(did) = stack.pop() {
            if out.tables.len() >= MAX_CLOSURE_TABLES
                || did.is_null()
                || out.index.contains_key(&did)
            {
                continue;
            }
            let Some(t) = self.table(did) else {
                continue;
            };
            // Reversed, so the first include is visited first.
            stack.extend(
                t.includes
                    .iter()
                    .rev()
                    .copied()
                    .filter(|inc| !inc.is_null() && *inc != did),
            );
            out.index.insert(did, out.tables.len());
            out.tables.push(t);
        }
        out
    }

    /// Whether `t`'s definition of `key` is overriding (`script-table.md` §8.2 step 2).
    fn overriding(&self, t: &ScriptTable, key: u32) -> bool {
        let Some((pid, overriding)) = self.combination else {
            return false;
        };
        t.script_properties(key)
            .and_then(|set| set.get(pid))
            .and_then(PropertyValue::as_f64)
            .is_some_and(|v| v == f64::from(overriding))
    }

    /// The tables whose definitions of `key` run, in run order (`script-table.md` §8.2
    /// steps 2–3): for the kind-1, then the kind-2, then the kind-4 list, the closure from
    /// `roots` in pre-order; a table whose kind shares a bit with the list's contributes
    /// its definition, and an overriding definition stops the descent into that table's
    /// includes.
    ///
    /// A definition is taken once over all three lists, while §8.2 step 3 de-duplicates
    /// per list only. For a schema the two are the same: step 4 of
    /// `official-messages-world.md` §4.3.1 appends a condition PID only when it is not in
    /// its list yet, so visiting a definition a second time (in a later list) adds nothing
    /// and cannot change the order of the PIDs found first.
    fn gather(&self, c: &Closure, roots: &[usize], key: u32) -> Vec<usize> {
        let mut out = Vec::new();
        let mut taken = vec![false; c.tables.len()];
        let mut seen = vec![false; c.tables.len()];
        for list in RUN_LIST_KINDS {
            seen.fill(false);
            let mut stack: Vec<usize> = roots.iter().rev().copied().collect();
            while let Some(ti) = stack.pop() {
                match seen.get_mut(ti) {
                    Some(v) if !*v => *v = true,
                    _ => continue,
                }
                let Some(t) = c.tables.get(ti) else {
                    continue;
                };
                let mut descend = true;
                if t.kind & list != 0 && t.script(key).is_some() {
                    if let Some(tk) = taken.get_mut(ti)
                        && !*tk
                    {
                        *tk = true;
                        out.push(ti);
                    }
                    descend = !self.overriding(t, key);
                }
                if descend {
                    for inc in t.includes.iter().rev() {
                        if let Some(&j) = c.index.get(inc)
                            && !seen.get(j).copied().unwrap_or(true)
                        {
                            stack.push(j);
                        }
                    }
                }
            }
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
    ///
    /// A scriptlet is entered at most once per schema (`entered`, the scriptlets entered
    /// so far for this key), however many nodes name it: the scriptlets of a closure form a
    /// graph that many paths share, and walking it once per path grows with the number of
    /// paths, not of nodes (the reference install's key `0x05` spent the whole per-key
    /// budget that way). It does not change the schema: step 4 appends a PID only when it is
    /// not listed yet, and by the time a scriptlet is named again every node its first
    /// entry reaches has been visited, or is being visited by a scriptlet still being
    /// entered further up, which a second entry would skip as well; so a second entry
    /// finds no PID that is not listed already, and the order of first appearance stays
    /// that of the full pre-order walk. The same set stops a scriptlet that invokes itself
    /// (directly or through others).
    ///
    /// Two readings of step 2, recorded (question in `docs/m8-remediation.md` §15): the
    /// scriptlet's roots are visited before the node's own port children, and every port of
    /// a node counts as an output port (the decoded node keeps no port direction). The
    /// verification tables of §4.3.1 come out the same on the reference install.
    fn visit(
        &mut self,
        node: &Node,
        depth: usize,
        entered: &mut HashSet<u32>,
        out: &mut PortalStateSchema,
    ) {
        if self.visited >= self.limit || depth >= MAX_SCHEMA_DEPTH {
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
                // Entered once per schema (see above).
                if entered.insert(file.0)
                    && let Some(s) = self.scriptlet(*file)
                {
                    for r in &s.roots {
                        self.visit(r, depth + 1, entered, out);
                    }
                }
            }
            _ => {}
        }
        for port in &node.ports {
            for child in &port.children {
                self.visit(child, depth + 1, entered, out);
            }
        }
    }

    /// The schemas of `keys` over the closure `c` from the tables at `roots` (steps 2–4
    /// for the definitions [`Self::gather`] finds). Each key visits at most
    /// [`MAX_SCHEMA_NODES`], all keys together at most [`MAX_SCHEMA_TOTAL_NODES`]; a key
    /// that hits either bound (or [`MAX_SCHEMA_DEPTH`]) is left out, and its visits still
    /// count against the total.
    fn build(
        &mut self,
        c: &Closure,
        roots: &[usize],
        keys: &[u32],
    ) -> BTreeMap<u32, PortalStateSchema> {
        let mut out = BTreeMap::new();
        for &key in keys {
            self.visited = 0;
            self.truncated = false;
            self.limit = self.per_key.min(self.total_left);
            let mut schema = PortalStateSchema::default();
            let mut entered = HashSet::new();
            for ti in self.gather(c, roots, key) {
                let Some(script) = c.tables.get(ti).and_then(|t| t.script(key)) else {
                    continue;
                };
                for r in &script.roots {
                    self.visit(r, 0, &mut entered, &mut schema);
                }
                if self.truncated {
                    break;
                }
            }
            self.total_left = self.total_left.saturating_sub(self.visited);
            if self.truncated {
                tracing::warn!(
                    key = format_args!("{key:#010x}"),
                    total_left = self.total_left,
                    "official session: a state schema hit its traversal bound and is left out"
                );
                self.left_out.push(key);
                continue;
            }
            out.insert(key, schema);
        }
        out
    }
}

/// `Script_CombinationType` and its `Overriding` key from the install's vocabulary
/// (`script-table.md` §8.2), `None` when the install lacks it.
fn combination_vocabulary(set: &ArchiveSet, dict: &PropertyDictionary) -> Option<(u32, u32)> {
    let mappers = |did: Did| {
        bounded_record(set, did.0, MAX_TABLE_RECORD)
            .and_then(|b| EnumMapper::parse_expecting(&b, did).ok())
    };
    let vocab = ScriptVocabulary::from_data(dict, mappers).ok()?;
    let pid = vocab.combination_type?;
    Some((
        pid,
        vocab
            .combination_overriding
            .unwrap_or(COMBINATION_OVERRIDING),
    ))
}

/// The `0xCC` state schemas of the install (`official-messages-world.md` §4.3.1).
///
/// Step 1: the closure of the avatar table and the global root
/// ([`SCHEMA_CLOSURE_ROOTS`]). The schema IDs are the script keys of the player-physics
/// table [`PLAYER_PHYSICS_TABLE`] (the table §4.3.1 names as defining them, which must be
/// in that closure) that are keys of the script-key enum [`SCRIPT_KEY_ENUM`] (Tessera's
/// reading, question in `docs/m8-remediation.md` §15 (W §4.3.1)). For each, the
/// definitions of the key in the closure are gathered as `script-table.md` §8.2 runs them
/// (kind lists 1, 2, 4; overriding definitions prune their tables' includes), and steps
/// 2–4 collect the condition PIDs of their Switch (12) and When (63) nodes. A schema that
/// hits [`MAX_SCHEMA_NODES`], [`MAX_SCHEMA_TOTAL_NODES`] or [`MAX_SCHEMA_DEPTH`] is left
/// out. Empty when a table or the enum is missing.
#[must_use]
pub fn portal_state_schemas(
    set: &ArchiveSet,
    registry: &PropertyRegistry,
) -> BTreeMap<u32, PortalStateSchema> {
    portal_state_schema_build(set, registry).schemas
}

/// What [`portal_state_schema_build`] found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SchemaBuild {
    /// The schemas built, by `state_schema_id`.
    pub schemas: BTreeMap<u32, PortalStateSchema>,
    /// The schema IDs left out at a traversal bound, in build order.
    pub left_out: Vec<u32>,
    /// Nodes visited over all keys.
    pub visited: usize,
}

/// [`portal_state_schemas`] with the keys left out and the nodes visited (tests and
/// tools).
#[must_use]
pub fn portal_state_schema_build(set: &ArchiveSet, registry: &PropertyRegistry) -> SchemaBuild {
    let read = |did: Did| bounded_record(set, did.0, MAX_TABLE_RECORD);
    let mut b = SchemaBuilder::new(
        &read,
        registry,
        combination_vocabulary(set, registry.dictionary()),
    );
    let roots = SCHEMA_CLOSURE_ROOTS.map(Did);
    let closure = b.closure(&roots);
    let Some(physics) = closure
        .index
        .get(&Did(PLAYER_PHYSICS_TABLE))
        .and_then(|&i| closure.tables.get(i))
    else {
        tracing::warn!("official session: the player-physics script table is not in the closure");
        return SchemaBuild::default();
    };
    // Tessera's reading (question in `docs/m8-remediation.md` §15 (W §4.3.1)): the keys of
    // `0x07000029` are kept only when they are keys of `0x23000097`, which reaches
    // `0x230000F8` through its base chain ([`SCRIPT_KEY_ENUM`]).
    let Some(script_keys) = enum_keys(set, SCRIPT_KEY_ENUM) else {
        tracing::warn!("official session: the script-key enum is missing");
        return SchemaBuild::default();
    };
    let keys: Vec<u32> = physics
        .scripts
        .iter()
        .map(|s| s.key)
        .filter(|k| script_keys.binary_search(k).is_ok())
        .collect();
    let root_positions: Vec<usize> = roots
        .iter()
        .filter_map(|d| closure.index.get(d).copied())
        .collect();
    let schemas = b.build(&closure, &root_positions, &keys);
    let visited = MAX_SCHEMA_TOTAL_NODES.saturating_sub(b.total_left);
    tracing::debug!(
        schemas = schemas.len(),
        tables = closure.tables.len(),
        visited,
        "official session: state schemas built"
    );
    SchemaBuild {
        schemas,
        left_out: b.left_out,
        visited,
    }
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
    let served = Arc::new(Mutex::new(ServedTables::default()));
    let strings = Strings::new(SetTables {
        set: Arc::clone(&set),
        served: Arc::clone(&served),
    });
    Ok(ArchiveData {
        set,
        registry,
        strings,
        served,
        schemas,
        chat_keys,
        templates: Mutex::new(TemplateCache::default()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_display_cut_drops_from_the_first_unescaped_bracket() {
        assert_eq!(display_cut("Watcher [m]"), "Watcher");
        assert_eq!(display_cut("Watcher\t \n[[x]] more"), "Watcher");
        assert_eq!(display_cut(r"A \[b\] c [n]"), r"A \[b\] c");
        assert_eq!(display_cut("No group  "), "No group  ");
        assert_eq!(display_cut("[all]"), "");
        // Escapes are decoded after the cut (§9.0 step 1).
        assert_eq!(decode_escapes(display_cut(r"A \[b\]\q [n]")), "A [b]\"");
    }

    /// The smallest valid property dictionary: no names, no definitions
    /// (`property-system.md` §2).
    fn empty_registry() -> PropertyRegistry {
        let mut out = 0x3400_0000u32.to_le_bytes().to_vec();
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        PropertyRegistry::parse(&out).expect("an empty dictionary")
    }

    /// A leaf node of no particular kind.
    fn leaf() -> Node {
        Node {
            kind: NodeKind(2),
            data: NodeData::None,
            base: tessera_formats::script_table::BaseBlock::default(),
            particle_flag: None,
            ports: Vec::new(),
            targets: Vec::new(),
        }
    }

    /// One kind-1 table defining `keys`, each with `nodes` leaf roots.
    fn closure_of(keys: &[u32], nodes: usize) -> Closure {
        let table = ScriptTable {
            did: Did(PLAYER_PHYSICS_TABLE),
            kind: 1,
            scripts: keys
                .iter()
                .map(|&key| tessera_formats::script_table::Script {
                    key,
                    roots: vec![leaf(); nodes],
                })
                .collect(),
            ..ScriptTable::default()
        };
        let mut c = Closure::default();
        c.index.insert(table.did, 0);
        c.tables.push(table);
        c
    }

    #[test]
    fn the_schema_build_keeps_to_a_total_node_budget_over_all_keys() {
        let registry = empty_registry();
        let read = |_: Did| None;
        let keys = [1, 2, 3, 4];
        let c = closure_of(&keys, 10);

        // Every key within both budgets: all built.
        let mut b = SchemaBuilder::new(&read, &registry, None);
        assert_eq!(b.build(&c, &[0], &keys).len(), 4);
        assert_eq!(b.total_left, MAX_SCHEMA_TOTAL_NODES - 40);

        // A total of 25 nodes: keys 1 and 2 (10 each) are built, key 3 passes the total
        // after 5 nodes and is left out, key 4 has nothing left.
        let mut b = SchemaBuilder::new(&read, &registry, None);
        b.total_left = 25;
        let built = b.build(&c, &[0], &keys);
        assert_eq!(built.keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(b.total_left, 0);

        // The per-key budget alone: a key over it is left out, the others are built, and
        // the visits of the key left out count against the total.
        let c = ScriptTable {
            scripts: vec![
                tessera_formats::script_table::Script {
                    key: 1,
                    roots: vec![leaf(); 3],
                },
                tessera_formats::script_table::Script {
                    key: 2,
                    roots: vec![leaf(); 12],
                },
            ],
            ..closure_of(&[], 0).tables.remove(0)
        };
        let mut closure = Closure::default();
        closure.index.insert(c.did, 0);
        closure.tables.push(c);
        let mut b = SchemaBuilder::new(&read, &registry, None);
        b.per_key = 5;
        b.total_left = 100;
        let built = b.build(&closure, &[0], &[1, 2]);
        assert_eq!(built.keys().copied().collect::<Vec<_>>(), [1]);
        assert_eq!(b.total_left, 100 - 3 - 5);
    }

    /// A Scriptlet node naming `file`.
    fn scriptlet_node(file: u32) -> Node {
        Node {
            data: NodeData::Scriptlet {
                file: Did(file),
                parameters: PropertySet::default(),
            },
            ..leaf()
        }
    }

    #[test]
    fn a_scriptlet_named_on_many_paths_is_entered_once_per_schema() {
        // 40 scriptlets, each naming the next twice: walked once per path that is 2^40
        // entries of the last; entered once per schema it is 80 nodes and one leaf.
        const N: u32 = 40;
        const BASE: u32 = 0x0C00_0100;
        let registry = empty_registry();
        let read = |_: Did| None;
        let mut b = SchemaBuilder::new(&read, &registry, None);
        for i in 0..N {
            let roots = if i + 1 < N {
                vec![scriptlet_node(BASE + i + 1), scriptlet_node(BASE + i + 1)]
            } else {
                vec![leaf()]
            };
            b.scriptlets.insert(
                BASE + i,
                Some(Arc::new(Scriptlet {
                    did: Did(BASE + i),
                    roots,
                    dependencies: Vec::new(),
                })),
            );
        }
        let mut c = closure_of(&[1, 2], 0);
        for script in &mut c.tables[0].scripts {
            script.roots = vec![scriptlet_node(BASE), scriptlet_node(BASE)];
        }
        let built = b.build(&c, &[0], &[1, 2]);
        assert_eq!(built.keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert!(b.left_out.is_empty());
        // Per key: the two root nodes, two nodes in each of the first 39 scriptlets and the
        // last one's leaf; each scriptlet is entered again for the next key.
        let per_key = 2 + 2 * (N as usize - 1) + 1;
        assert_eq!(b.total_left, MAX_SCHEMA_TOTAL_NODES - 2 * per_key);
    }

    #[test]
    fn a_scriptlet_that_names_itself_is_not_entered_again() {
        let registry = empty_registry();
        let read = |_: Did| None;
        let mut b = SchemaBuilder::new(&read, &registry, None);
        for (did, next) in [(0x0C00_0001, 0x0C00_0002), (0x0C00_0002, 0x0C00_0001)] {
            b.scriptlets.insert(
                did,
                Some(Arc::new(Scriptlet {
                    did: Did(did),
                    roots: vec![scriptlet_node(next), scriptlet_node(did)],
                    dependencies: Vec::new(),
                })),
            );
        }
        let mut c = closure_of(&[7], 0);
        c.tables[0].scripts[0].roots = vec![scriptlet_node(0x0C00_0001)];
        assert_eq!(b.build(&c, &[0], &[7]).len(), 1);
        // The root, then two nodes in each scriptlet.
        assert_eq!(b.total_left, MAX_SCHEMA_TOTAL_NODES - 5);
    }

    #[test]
    fn a_literal_that_shows_nothing_is_no_text() {
        assert_eq!(shown_literal(r"A \[b\] [n]").as_deref(), Some("A [b]"));
        assert_eq!(shown_literal("[all]"), None);
        assert_eq!(shown_literal("  \t"), None);
        assert_eq!(shown_literal(""), None);
    }

    #[test]
    fn only_string_table_dids_are_looked_up() {
        assert_eq!(lookup_key(7, 0x2500_0001), Some((7, 0x2500_0001)));
        assert_eq!(lookup_key(0, 0x2500_0001), None);
        for table in [0, 0x0700_0029, 0x2600_0001, 0x2400_FFFF, 0xFFFF_FFFF] {
            assert_eq!(lookup_key(7, table), None, "{table:#x}");
        }
    }

    #[test]
    fn a_table_counts_as_cached_only_while_the_cache_never_evicted_one() {
        let mut s = ServedTables::default();
        for did in [0x2500_0001, 0x2500_0002] {
            s.reads += 1;
            s.dids.insert(did);
        }
        assert!(s.holds(0x2500_0001, 2));
        assert!(!s.holds(0x2500_0003, 2), "never read");
        // The cache holds fewer than were read: one was evicted, none is trusted.
        assert!(!s.holds(0x2500_0001, 1));
        // A table read twice was evicted before: none is trusted from then on.
        s.reads += 1;
        assert!(!s.holds(0x2500_0001, 2));
    }

    #[test]
    fn the_template_cache_drops_the_set_used_longest_ago_one_at_a_time() {
        let mut c = TemplateCache::default();
        for t in 0..MAX_CACHED_TEMPLATES as u32 {
            c.insert(t, None);
        }
        // Template 0 is used again: template 1 is now the oldest.
        assert!(c.get(0).is_some());
        c.insert(9_999, None);
        assert_eq!(c.sets.len(), MAX_CACHED_TEMPLATES);
        assert!(c.sets.contains_key(&0));
        assert!(!c.sets.contains_key(&1));
        assert!(c.sets.contains_key(&2));
        assert_eq!(c.order.len(), c.sets.len());
        // Inserting a template already there evicts nothing.
        c.insert(9_999, None);
        assert_eq!(c.sets.len(), MAX_CACHED_TEMPLATES);
        assert!(c.sets.contains_key(&2));
    }
}
