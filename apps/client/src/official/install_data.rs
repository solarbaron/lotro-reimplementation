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
//!   (step 1), a reference through the string tables with the arguments the wire text
//!   carries (`official-messages-world.md` §2.6) as its values (`wire_values`,
//!   `string-table.md` §9.0 steps 3–4); a message with a value slot no argument fills is
//!   shown without that value and counted ([`ArchiveData::texts_without_values`]); a
//!   mode-2 text shows its second authoring string like a literal (§2.6 row 1,
//!   `LocalizedTokenText::source`). Each text the session shows is resolved once (the
//!   entity feed renders a name at the creation only), so the count is one per text
//!   shown. A reference whose table DID
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
    EncodedPropertyValue, LocalizedTokenText, LocalizedTokenTextArg, LocalizedTokenTextArgPayload,
    LocalizedTokenTextContent, PortalStateSchema, PropertyType, TextSource,
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
    CacheLimits, DEFAULT_MAX_DEPTH, Diagnostic, Message, Miss, MissReason, RenderOptions, Strings,
    TableSource, Value, Values, decode_escapes, is_string_table_did, render,
};

/// Deepest nesting of kind-1 arguments (a text inside a text, `official-messages-world.md`
/// §2.6) given to the renderer; deeper ones render as absent with a diagnostic, so the
/// text is not shown (`string-table.md` §9.0 step 4). Engineering choice, spec silent: the
/// renderer's own nesting limit.
pub const MAX_ARGUMENT_DEPTH: usize = DEFAULT_MAX_DEPTH;

/// Most kind-1 argument texts looked up in the string tables for one wire text; past it,
/// the further ones render as absent (the text is not shown), so a hostile text cannot
/// make one resolve read many tables. Engineering choice, spec silent.
pub const MAX_ARGUMENT_LOOKUPS: usize = 8;

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

/// A wire text as the session shows it ([`resolve_wire_text`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedText {
    /// The display text, `None` when nothing is shown.
    text: Option<String>,
    /// The message has a value slot none of the arguments fills.
    unfilled: bool,
}

/// Resolves a wire `LocalizedTokenText` through `strings` (`string-table.md` §9.0): mode
/// 2 and literals as in step 1, a reference rendered in display mode with its arguments as
/// values ([`wire_values`]).
fn resolve_wire_text(strings: &Strings, text: &LocalizedTokenText) -> ResolvedText {
    let shown = |text| ResolvedText {
        text,
        unfilled: false,
    };
    // Mode 2: the second authoring string, shown like a literal; no table is read
    // (`official-messages-world.md` §2.6 row 1).
    if let TextSource::AuthoringSource(source) = text.source() {
        return shown(shown_literal(source));
    }
    let (token, table) = match &text.content {
        LocalizedTokenTextContent::Literal(s) => return shown(shown_literal(s)),
        LocalizedTokenTextContent::Reference {
            token_id,
            table_did,
            ..
        } => match lookup_key(*token_id, *table_did) {
            Some(key) => key,
            None => return shown(None),
        },
    };
    let Ok(msg) = strings.message(table, token) else {
        return shown(None);
    };
    let mut lookups = MAX_ARGUMENT_LOOKUPS;
    let values = wire_values(strings, &text.arguments, &msg, 1, &mut lookups);
    let unfilled = msg.arg_ids().iter().any(|id| values.get(*id).is_none());
    let rendered = render(&msg, &values, &RenderOptions::default());
    // A miss, or an argument text that rendered as absent (too deep, over the budget):
    // the text fails (`string-table.md` §9.0 step 4). A truncated output is still shown.
    let failed = rendered.diagnostics.iter().any(|d| {
        matches!(
            d,
            Diagnostic::Miss(_) | Diagnostic::DepthLimit { .. } | Diagnostic::WorkLimit { .. }
        )
    });
    let text = if failed {
        None
    } else {
        let s = rendered.plain_text();
        (!s.trim().is_empty()).then_some(s)
    };
    ResolvedText { text, unfilled }
}

/// The values of a wire text's `arguments` for `msg` (`string-table.md` §9.0 steps 3–4,
/// `official-messages-world.md` §2.6): each argument is bound by its entry's key (on the
/// wire the `var_id`), the last argument of a key wins, and only keys `msg` uses are
/// built. Kind 1 is the nested text rendered in raw mode ([`nested_value`], at most
/// [`MAX_ARGUMENT_DEPTH`] deep and [`MAX_ARGUMENT_LOOKUPS`] table lookups per text); kinds
/// 2 and 3 are the float, kinds 4 to 7 the integer (`i64` two's complement). The decimal
/// places, format flags and radix of kinds 2, 3, 5 and 7 (§9.4.2–§9.4.3) are not applied:
/// the renderer prints a number in its plain decimal form (as `string-table.md` §9.4 says
/// Tessera does without the locale record). An argument without a value (an undefined
/// kind) fills no slot (§9.0 step 4). `depth` is the level of `msg` (the outermost is 1).
fn wire_values(
    strings: &Strings,
    arguments: &[LocalizedTokenTextArg],
    msg: &Message,
    depth: usize,
    lookups: &mut usize,
) -> Values {
    let used: HashSet<u32> = msg.arg_ids().into_iter().collect();
    let mut values = Values::new();
    for arg in arguments.iter().rev() {
        if !used.contains(&arg.key) || values.get(arg.key).is_some() {
            continue;
        }
        let value = match &arg.payload {
            None => continue,
            Some(LocalizedTokenTextArgPayload::Nested(text)) => {
                nested_value(strings, text, depth + 1, lookups)
            }
            Some(
                LocalizedTokenTextArgPayload::Float { value, .. }
                | LocalizedTokenTextArgPayload::FloatFormatted { value, .. },
            ) => Value::Float(*value),
            Some(
                LocalizedTokenTextArgPayload::Int { value, .. }
                | LocalizedTokenTextArgPayload::IntFormatted { value, .. },
            ) => Value::Integer(i64::from_ne_bytes(value.to_ne_bytes())),
        };
        values.insert(arg.key, value);
    }
    values
}

/// A kind-1 argument's value (`string-table.md` §9.0 step 4): the nested text in raw mode
/// (a literal or mode-2 source as it is, tag group kept and escapes not decoded; a
/// reference as a message with its own argument values). A nested text that fails, is too
/// deep or over the lookup budget is an absent value with its diagnostic, which makes the
/// outer text fail (step 4).
fn nested_value(
    strings: &Strings,
    text: &LocalizedTokenText,
    depth: usize,
    lookups: &mut usize,
) -> Value {
    if depth > MAX_ARGUMENT_DEPTH {
        return Value::Unresolved(Diagnostic::DepthLimit {
            limit: MAX_ARGUMENT_DEPTH,
        });
    }
    let (token, table) = match text.source() {
        TextSource::Literal(s) | TextSource::AuthoringSource(s) => {
            return Value::Text(s.to_owned());
        }
        TextSource::Table {
            token_id,
            table_did,
        } => (token_id, table_did),
    };
    // `property-system.md` §4.8: a (table 0, token 0) text is "no text".
    if token == 0 && table == 0 {
        return Value::Text(String::new());
    }
    if !is_string_table_did(table) {
        return Value::Unresolved(Diagnostic::Miss(Miss {
            table,
            token,
            reason: MissReason::NotStringTable,
        }));
    }
    let Some(left) = lookups.checked_sub(1) else {
        return Value::Unresolved(Diagnostic::WorkLimit {
            limit: MAX_ARGUMENT_LOOKUPS,
        });
    };
    *lookups = left;
    match strings.message(table, token) {
        Ok(msg) => {
            let values = wire_values(strings, &text.arguments, &msg, depth, lookups);
            Value::message(msg, values)
        }
        Err(miss) => Value::Unresolved(Diagnostic::Miss(miss)),
    }
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

    /// Texts whose message has a value slot that none of the wire text's arguments fills
    /// (`official-messages-world.md` §2.6): the slot renders as absent.
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
    /// the selected language's tables and rendered (display mode) with the wire text's
    /// arguments as its values (`wire_values`). A miss, a reference whose table DID lacks
    /// the `0x25` prefix (refused before any read: `official-messages-world.md` §2.6 row
    /// 2b, `string-table.md` §9.1), an argument text that fails (§9.0 step 4), or a text
    /// that shows nothing (the emptiness test of §9.0 step 2, literals included) is
    /// `None`: nothing is shown and the caller counts it. A value slot no argument fills
    /// renders as absent ([`ArchiveData::texts_without_values`]).
    fn resolve_text(&self, text: &LocalizedTokenText) -> Option<String> {
        let resolved = resolve_wire_text(&self.strings, text);
        if resolved.unfilled {
            self.texts_without_values.fetch_add(1, Ordering::Relaxed);
            tracing::debug!("official text with a value slot no argument fills");
        }
        resolved.text
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
    /// The key being built reached a `PlayerPhysics` node (kind 47).
    physics: bool,
    /// The keys left out at a bound, in build order.
    left_out: Vec<u32>,
    /// The keys whose definitions reach no `PlayerPhysics` node, in build order.
    not_physics: Vec<u32>,
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
            physics: false,
            left_out: Vec::new(),
            not_physics: Vec::new(),
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
        if node.kind == NodeKind::PLAYER_PHYSICS {
            self.physics = true;
        }
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
    /// for the definitions [`Self::gather`] finds); a key is a schema only when a node it
    /// visits, its scriptlets' included, is a `PlayerPhysics` node (kind 47, step 3). Each
    /// key visits at most
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
            self.physics = false;
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
            // §4.3.1 step 3: only a script whose node graph has a `PlayerPhysics` node
            // (kind 47) is a player-physics schema.
            if !self.physics {
                self.not_physics.push(key);
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
/// 2–4 collect the condition PIDs of their Switch (12) and When (63) nodes. Only a key
/// whose definitions reach a `PlayerPhysics` node (kind 47) is a schema (step 3). A schema
/// that hits [`MAX_SCHEMA_NODES`], [`MAX_SCHEMA_TOTAL_NODES`] or [`MAX_SCHEMA_DEPTH`] is
/// left out. Empty when a table or the enum is missing.
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
    /// The script keys of the player-physics table whose definitions reach no
    /// `PlayerPhysics` node (general lifecycle scripts, not schemas; §4.3.1 step 3).
    pub not_physics: Vec<u32>,
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
        not_physics: b.not_physics,
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

    /// A leaf node: a `PlayerPhysics` node (kind 47) without data, so that a script with
    /// one is a schema (`official-messages-world.md` §4.3.1 step 3).
    fn leaf() -> Node {
        Node {
            kind: NodeKind::PLAYER_PHYSICS,
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

    /// A synthetic property dictionary (`property-system.md` §2.1, §2.2) defining `defs`
    /// (PID, type code), without names or defaults. Counts stay below 128, so every VLE is
    /// one byte.
    fn typed_registry(defs: &[(u32, u32)]) -> PropertyRegistry {
        let mut out = 0x3400_0000u32.to_le_bytes().to_vec();
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&[0, 0]); // no names
        out.extend_from_slice(&[0, 0]);
        out.push(0);
        out.push(u8::try_from(defs.len()).expect("few"));
        for &(pid, ty) in defs {
            for v in [pid, pid, ty, 0, 4, 0, 0, 0] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&0x3FC0_0000u32.to_le_bytes());
            out.extend_from_slice(&[2, 2, 4, 0, 1]);
            out.push(0); // children (VLE)
            out.extend_from_slice(&0u32.to_le_bytes()); // subset count
            out.extend_from_slice(&0u32.to_le_bytes()); // terminator
        }
        PropertyRegistry::parse(&out).expect("a synthetic dictionary")
    }

    /// A Switch node on `pid` whose one port runs `children`.
    fn switch_node(pid: u32, children: Vec<Node>) -> Node {
        Node {
            kind: NodeKind::SWITCH,
            data: NodeData::Switch(tessera_formats::script_table::SwitchNode {
                property: pid,
                ..Default::default()
            }),
            ports: vec![tessera_formats::script_table::Port {
                port_type: tessera_formats::script_table::PortType(1),
                payload: tessera_formats::script_table::PortPayload::None,
                children,
            }],
            ..leaf()
        }
    }

    /// One random node of at most 3 levels: a Switch on one of `pids` with up to 2
    /// children, a Scriptlet node naming one of the `count` scriptlets from `base`, or a
    /// leaf.
    fn random_node(
        depth: u32,
        next: &mut dyn FnMut(u32) -> u32,
        pids: &[(u32, u32)],
        base: u32,
        count: u32,
    ) -> Node {
        match next(4) {
            0 | 1 if depth < 3 => {
                let k = next(3);
                let children = (0..k)
                    .map(|_| random_node(depth + 1, next, pids, base, count))
                    .collect();
                let n = u32::try_from(pids.len()).expect("few");
                let (pid, _) = pids[next(n) as usize];
                switch_node(pid, children)
            }
            2 => scriptlet_node(base + next(count)),
            _ => leaf(),
        }
    }

    /// One to three random root nodes ([`random_node`]).
    fn random_nodes(
        next: &mut dyn FnMut(u32) -> u32,
        pids: &[(u32, u32)],
        base: u32,
        count: u32,
    ) -> Vec<Node> {
        let k = 1 + next(3);
        (0..k)
            .map(|_| random_node(0, next, pids, base, count))
            .collect()
    }

    /// The naive walk of steps 2–4: pre-order, a scriptlet entered on every path that names
    /// it (a scriptlet on the current path is not entered again), every PID appended on its
    /// first appearance. `None` when it visits more than `cap` nodes.
    fn naive_schema(
        roots: &[Node],
        scriptlets: &HashMap<u32, Vec<Node>>,
        types: &HashMap<u32, u32>,
        cap: usize,
    ) -> Option<PortalStateSchema> {
        fn walk(
            n: &Node,
            scriptlets: &HashMap<u32, Vec<Node>>,
            types: &HashMap<u32, u32>,
            path: &mut Vec<u32>,
            left: &mut usize,
            out: &mut PortalStateSchema,
        ) -> Option<()> {
            *left = left.checked_sub(1)?;
            match &n.data {
                NodeData::Switch(sw) => {
                    let list = match types.get(&sw.property) {
                        Some(21) => Some(&mut out.bool_pids),
                        Some(8) => Some(&mut out.enum_pids),
                        _ => None,
                    };
                    if let Some(list) = list
                        && !list.contains(&sw.property)
                    {
                        list.push(sw.property);
                    }
                }
                NodeData::Scriptlet { file, .. } if !path.contains(&file.0) => {
                    if let Some(rs) = scriptlets.get(&file.0) {
                        path.push(file.0);
                        for r in rs {
                            walk(r, scriptlets, types, path, left, out)?;
                        }
                        path.pop();
                    }
                }
                _ => {}
            }
            for p in &n.ports {
                for c in &p.children {
                    walk(c, scriptlets, types, path, left, out)?;
                }
            }
            Some(())
        }
        let mut out = PortalStateSchema::default();
        let mut left = cap;
        for r in roots {
            walk(r, scriptlets, types, &mut Vec::new(), &mut left, &mut out)?;
        }
        Some(out)
    }

    #[test]
    fn the_once_per_schema_walk_finds_the_pids_of_the_naive_walk_in_their_order() {
        // Small random scriptlet graphs (shared scriptlets, diamonds, cycles, nodes naming
        // scriptlets inside Switch ports, PIDs of both types and of neither): the schema of
        // the once-per-schema walk has the PIDs of the naive walk, in the same order.
        const BASE: u32 = 0x0C00_0200;
        const SCRIPTLETS: u32 = 7;
        let pids: [(u32, u32); 7] = [
            (0x100, 21),
            (0x101, 8),
            (0x102, 21),
            (0x103, 8),
            (0x104, 21),
            (0x105, 8),
            (0x106, 4), // neither type: never listed
        ];
        let registry = typed_registry(&pids);
        let types: HashMap<u32, u32> = pids.iter().copied().collect();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            u32::try_from(seed % u64::from(n)).expect("small")
        };
        let mut compared = 0;
        for case in 0..300 {
            let graph: HashMap<u32, Vec<Node>> = (0..SCRIPTLETS)
                .map(|i| (BASE + i, random_nodes(&mut next, &pids, BASE, SCRIPTLETS)))
                .collect();
            let roots = random_nodes(&mut next, &pids, BASE, SCRIPTLETS);
            let Some(expected) = naive_schema(&roots, &graph, &types, 200_000) else {
                continue;
            };
            let read = |_: Did| None;
            let mut b = SchemaBuilder::new(&read, &registry, None);
            for (did, rs) in &graph {
                b.scriptlets.insert(
                    *did,
                    Some(Arc::new(Scriptlet {
                        did: Did(*did),
                        roots: rs.clone(),
                        dependencies: Vec::new(),
                    })),
                );
            }
            let mut c = closure_of(&[9], 0);
            // A last root that is a `PlayerPhysics` leaf makes the key a schema; it holds no
            // PID, so the order of the PIDs found is the naive walk's.
            c.tables[0].scripts[0].roots = roots;
            c.tables[0].scripts[0].roots.push(leaf());
            let built = b.build(&c, &[0], &[9]);
            assert_eq!(built.get(&9), Some(&expected), "case {case}");
            compared += 1;
        }
        assert!(compared > 250, "only {compared} cases compared");
    }

    #[test]
    fn only_scripts_with_a_player_physics_node_are_schemas() {
        // §4.3.1 step 3: a script whose node graph has no `PlayerPhysics` node (kind 47) is
        // a lifecycle script, not a schema, whatever its conditions; one in a scriptlet the
        // script runs counts.
        let pids: [(u32, u32); 1] = [(0x200, 21)];
        let registry = typed_registry(&pids);
        let read = |_: Did| None;
        let mut b = SchemaBuilder::new(&read, &registry, None);
        let plain = || Node {
            kind: NodeKind(2),
            ..leaf()
        };
        b.scriptlets.insert(
            0x0C00_0300,
            Some(Arc::new(Scriptlet {
                did: Did(0x0C00_0300),
                roots: vec![leaf()],
                dependencies: Vec::new(),
            })),
        );
        let mut c = closure_of(&[1, 2, 3], 0);
        // Key 1: a Switch only. Key 2: a Switch and a PlayerPhysics node in its port.
        // Key 3: a Scriptlet node whose scriptlet holds the PlayerPhysics node.
        c.tables[0].scripts[0].roots = vec![switch_node(0x200, vec![plain()])];
        c.tables[0].scripts[1].roots = vec![switch_node(0x200, vec![leaf()])];
        c.tables[0].scripts[2].roots =
            vec![switch_node(0x200, vec![]), scriptlet_node(0x0C00_0300)];
        let built = b.build(&c, &[0], &[1, 2, 3]);
        assert_eq!(built.keys().copied().collect::<Vec<_>>(), [2, 3]);
        assert_eq!(built[&2].bool_pids, [0x200]);
        assert_eq!(b.not_physics, [1]);
        assert!(b.left_out.is_empty());
    }

    #[test]
    fn a_mode_2_text_is_its_source_string_shown_like_a_literal() {
        let mut t = LocalizedTokenText::reference(5, 0x2500_0001);
        if let LocalizedTokenTextContent::Reference { mode, .. } = &mut t.content {
            *mode = 2;
        }
        t.authoring_strings = Some(["tok".into(), r"Gate \[b\] [n]".into(), String::new()]);
        let TextSource::AuthoringSource(source) = t.source() else {
            panic!("mode 2");
        };
        assert_eq!(shown_literal(source).as_deref(), Some("Gate [b]"));
        t.authoring_strings = None;
        assert_eq!(t.source(), TextSource::AuthoringSource(""));
        assert_eq!(shown_literal(""), None);
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

    fn vle(out: &mut Vec<u8>, v: u32) {
        tessera_formats::reader::write_vle(out, v);
    }

    /// A string table (records.md §3) with one entry per (token, pieces, argument IDs).
    fn string_table(did: u32, entries: &[(u32, &[&str], &[u32])]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&did.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.push(0);
        vle(&mut b, entries.len() as u32);
        for (token, pieces, args) in entries {
            b.extend_from_slice(&token.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&(pieces.len() as u32).to_le_bytes());
            for p in *pieces {
                let units: Vec<u16> = p.encode_utf16().collect();
                vle(&mut b, units.len() as u32);
                for u in units {
                    b.extend_from_slice(&u.to_le_bytes());
                }
            }
            b.extend_from_slice(&(args.len() as u32).to_le_bytes());
            for a in *args {
                b.extend_from_slice(&a.to_le_bytes());
            }
            b.push(0);
        }
        b
    }

    const TABLE: u32 = 0x2500_0042;
    const NAME: u32 = 0x0000_1111;
    const COUNT: u32 = 0x0000_2222;

    /// Synthetic tables: token 1 "Hail, <NAME>!", token 2 "<COUNT> wolves", token 3
    /// "the <NAME>", token 4 "Bree".
    fn strings() -> Strings {
        let mut source = tessera_text::MemorySource::new();
        source.insert(
            TABLE,
            string_table(
                TABLE,
                &[
                    (1, &["Hail, ", "!"], &[NAME]),
                    (2, &["", " wolves"], &[COUNT]),
                    (3, &["the ", ""], &[NAME]),
                    (4, &["Bree"], &[]),
                ],
            ),
        );
        Strings::new(source)
    }

    fn with_args(token: u32, args: Vec<LocalizedTokenTextArg>) -> LocalizedTokenText {
        let mut t = LocalizedTokenText::reference(token, TABLE);
        t.arguments = args;
        t
    }

    #[test]
    fn wire_arguments_fill_the_message_as_values() {
        // `string-table.md` §9.0 steps 3-4, `official-messages-world.md` §2.6: the
        // arguments of a wire text are its message's values, bound by `var_id`.
        let strings = strings();
        let int = LocalizedTokenTextArg::new(COUNT, 4, LocalizedTokenTextArgPayload::int(12));
        let r = resolve_wire_text(&strings, &with_args(2, vec![int]));
        assert_eq!(r.text.as_deref(), Some("12 wolves"));
        assert!(!r.unfilled);
        // A negative `i64` (two's complement on the wire), kind 7 with format options.
        let negative = LocalizedTokenTextArg::new(
            COUNT,
            7,
            LocalizedTokenTextArgPayload::IntFormatted {
                is_32bit: false,
                value: (-3i64).cast_unsigned(),
                format_flags: 1,
                radix: 0xFFFF,
            },
        );
        let r = resolve_wire_text(&strings, &with_args(2, vec![negative]));
        assert_eq!(r.text.as_deref(), Some("-3 wolves"));
        // A float (kind 2).
        let float = LocalizedTokenTextArg::new(
            COUNT,
            2,
            LocalizedTokenTextArgPayload::Float {
                value: 2.5,
                decimal_places: 1,
            },
        );
        let r = resolve_wire_text(&strings, &with_args(2, vec![float]));
        assert_eq!(r.text.as_deref(), Some("2.5 wolves"));
        // A nested reference (kind 1), itself with an argument.
        let inner = with_args(
            3,
            vec![LocalizedTokenTextArg::new(
                NAME,
                1,
                LocalizedTokenTextArgPayload::Nested(Box::new(LocalizedTokenText::reference(
                    4, TABLE,
                ))),
            )],
        );
        let nested = LocalizedTokenTextArg::new(
            NAME,
            1,
            LocalizedTokenTextArgPayload::Nested(Box::new(inner)),
        );
        let r = resolve_wire_text(&strings, &with_args(1, vec![nested]));
        assert_eq!(r.text.as_deref(), Some("Hail, the Bree!"));
        // A nested literal is kept raw (its tag group too); the display cut of the outer
        // text then applies.
        let literal = LocalizedTokenTextArg::new(
            NAME,
            1,
            LocalizedTokenTextArgPayload::Nested(Box::new(LocalizedTokenText::literal("Aldo"))),
        );
        let r = resolve_wire_text(&strings, &with_args(1, vec![literal]));
        assert_eq!(r.text.as_deref(), Some("Hail, Aldo!"));
        // The last argument of a key wins; arguments the message does not use are ignored.
        let first = LocalizedTokenTextArg::new(COUNT, 4, LocalizedTokenTextArgPayload::int(1));
        let last = LocalizedTokenTextArg::new(COUNT, 4, LocalizedTokenTextArgPayload::int(2));
        let other = LocalizedTokenTextArg::new(0x9999, 4, LocalizedTokenTextArgPayload::int(3));
        let r = resolve_wire_text(&strings, &with_args(2, vec![first, last, other]));
        assert_eq!(r.text.as_deref(), Some("2 wolves"));
    }

    #[test]
    fn a_slot_without_an_argument_is_counted_and_a_failed_argument_fails_the_text() {
        let strings = strings();
        let r = resolve_wire_text(&strings, &with_args(2, Vec::new()));
        assert!(r.unfilled, "no argument for the slot");
        assert_eq!(r.text.as_deref(), Some("wolves"));
        // A nested reference that misses fails the outer text (§9.0 step 4).
        let missing = LocalizedTokenTextArg::new(
            NAME,
            1,
            LocalizedTokenTextArgPayload::Nested(Box::new(LocalizedTokenText::reference(
                99, TABLE,
            ))),
        );
        let r = resolve_wire_text(&strings, &with_args(1, vec![missing]));
        assert_eq!(r.text, None);
        // So does one whose table DID is no string table (refused unread).
        let foreign = LocalizedTokenTextArg::new(
            NAME,
            1,
            LocalizedTokenTextArgPayload::Nested(Box::new(LocalizedTokenText::reference(
                4,
                0x0700_0029,
            ))),
        );
        assert_eq!(
            resolve_wire_text(&strings, &with_args(1, vec![foreign])).text,
            None
        );
        // Nesting past the budgets fails the text instead of recursing on.
        let mut text = LocalizedTokenText::reference(4, TABLE);
        for _ in 0..=MAX_ARGUMENT_DEPTH {
            text = with_args(
                3,
                vec![LocalizedTokenTextArg::new(
                    NAME,
                    1,
                    LocalizedTokenTextArgPayload::Nested(Box::new(text)),
                )],
            );
        }
        assert_eq!(resolve_wire_text(&strings, &text).text, None);
    }

    #[test]
    fn nested_lookups_per_text_are_bounded() {
        // A chain of kind-1 references deeper than the lookup budget but within the depth
        // limit: the lookups past MAX_ARGUMENT_LOOKUPS are absent and fail the text.
        let strings = strings();
        let chain = |n: usize| {
            let mut text = LocalizedTokenText::reference(4, TABLE);
            for _ in 0..n {
                text = with_args(
                    3,
                    vec![LocalizedTokenTextArg::new(
                        NAME,
                        1,
                        LocalizedTokenTextArgPayload::Nested(Box::new(text)),
                    )],
                );
            }
            text
        };
        const { assert!(MAX_ARGUMENT_LOOKUPS < MAX_ARGUMENT_DEPTH) };
        assert!(
            resolve_wire_text(&strings, &chain(MAX_ARGUMENT_LOOKUPS))
                .text
                .is_some()
        );
        assert_eq!(
            resolve_wire_text(&strings, &chain(MAX_ARGUMENT_LOOKUPS + 1)).text,
            None
        );
    }
}
