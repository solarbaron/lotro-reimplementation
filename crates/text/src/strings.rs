// SPDX-License-Identifier: GPL-3.0-or-later
//! String access by DID and StringInfo resolution (`docs/specs/string-table.md` §9).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use tessera_formats::{
    DataIdMap, Record, StringArgValue, StringInfo, StringSource, StringTable, StringTableEntry,
};

use crate::error::{Result, TextError};
use crate::locale::{Locale, open_locale};
use crate::markup::{RichText, Style};
use crate::message::Message;
use crate::render::{
    DEFAULT_MAX_DEPTH, Diagnostic, Miss, MissReason, RenderMode, RenderOptions, Rendered, Renderer,
};
use crate::source::{CacheLimits, CacheStats, TableCache, TableSource, is_string_table_did};
use crate::value::{Value, Values};

/// DID of the root DID map (`records.md` §8.2).
pub const ROOT_DID_MAP: u32 = 0x2800_0000;
/// Root-map label of the named string-table map (`records.md` §8.2); an interoperability
/// identifier looked up at run time.
pub const STRING_TABLE_MAP_LABEL: &str = "STRINGTABLE";

/// One named string table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTable {
    /// Key in the DID map.
    pub key: u32,
    /// Label.
    pub label: String,
    /// Table DID (always prefix 0x25).
    pub did: u32,
}

/// Named string tables (spec §9.1): the DID map reached from the root map's
/// [`STRING_TABLE_MAP_LABEL`] label. Only values with the 0x25 prefix are kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamedTables {
    tables: Vec<NamedTable>,
}

impl NamedTables {
    /// Builds the set from a decoded DID map, skipping the null sentinel and any value
    /// that is not a string table.
    #[must_use]
    pub fn from_did_map(map: &DataIdMap) -> Self {
        let mut tables = Vec::new();
        for e in map {
            let did = e.value.raw();
            if is_string_table_did(did) {
                tables.push(NamedTable {
                    key: e.key,
                    label: e.label.to_string(),
                    did,
                });
            } else if did != 0 {
                tracing::debug!(label = %e.label, did = format_args!("{did:#010x}"), "named table is not a string table");
            }
        }
        Self { tables }
    }

    /// Resolves the named-table map through the root DID map and decodes it. `source`
    /// reads general-archive records (any DID, despite the trait's name).
    pub fn load(source: &dyn TableSource, root_did: u32) -> Result<Self> {
        let read = |did: u32| -> Result<DataIdMap> {
            let bytes = source
                .read_table(did)?
                .ok_or_else(|| TextError::NamedTables(format!("DID map {did:#010x} not found")))?;
            DataIdMap::parse_expecting(&bytes, did)
                .map_err(|source| TextError::Decode { did, source })
        };
        let root = read(root_did)?;
        let map_did = root.by_label(STRING_TABLE_MAP_LABEL).ok_or_else(|| {
            TextError::NamedTables(format!("root map has no {STRING_TABLE_MAP_LABEL} label"))
        })?;
        Ok(Self::from_did_map(&read(map_did.raw())?))
    }

    /// The table DID for a label.
    #[must_use]
    pub fn by_label(&self, label: &str) -> Option<u32> {
        self.tables.iter().find(|t| t.label == label).map(|t| t.did)
    }

    /// The table DID for a key.
    #[must_use]
    pub fn by_key(&self, key: u32) -> Option<u32> {
        self.tables.iter().find(|t| t.key == key).map(|t| t.did)
    }

    /// All named tables in map order.
    #[must_use]
    pub fn tables(&self) -> &[NamedTable] {
        &self.tables
    }
}

/// String access for one locale: tables by DID (cached), messages by (table, token),
/// rendering and StringInfo resolution. Thread-safe (`Send + Sync`).
pub struct Strings {
    source: Box<dyn TableSource>,
    cache: TableCache,
    locale: Option<Locale>,
    named: NamedTables,
}

impl fmt::Debug for Strings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Strings")
            .field("locale", &self.locale)
            .field("cache", &self.cache.stats())
            .field("named", &self.named.tables.len())
            .finish_non_exhaustive()
    }
}

impl Strings {
    /// String access over a table source with default cache limits.
    pub fn new(source: impl TableSource + 'static) -> Self {
        Self {
            source: Box::new(source),
            cache: TableCache::new(CacheLimits::default()),
            locale: None,
            named: NamedTables::default(),
        }
    }

    /// Selects and opens (read-only) the language archive of an install directory
    /// (spec §10.1) and gives string access over it.
    pub fn open_install(dir: impl AsRef<Path>, preferred_language: Option<&str>) -> Result<Self> {
        let (locale, archive) = open_locale(dir, preferred_language)?;
        Ok(Self::new(archive).with_locale(locale))
    }

    /// Sets the locale description.
    #[must_use]
    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    /// Replaces the cache with one using `limits`.
    #[must_use]
    pub fn with_cache_limits(mut self, limits: CacheLimits) -> Self {
        self.cache = TableCache::new(limits);
        self
    }

    /// Sets the named tables.
    #[must_use]
    pub fn with_named_tables(mut self, named: NamedTables) -> Self {
        self.named = named;
        self
    }

    /// The selected locale, if known.
    #[must_use]
    pub fn locale(&self) -> Option<&Locale> {
        self.locale.as_ref()
    }

    /// The named tables.
    #[must_use]
    pub fn named_tables(&self) -> &NamedTables {
        &self.named
    }

    /// Cache statistics.
    #[must_use]
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// Drops every cached table.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Every string-table DID in the locale, ascending.
    pub fn table_dids(&self) -> Result<Vec<u32>> {
        self.source.table_dids()
    }

    /// The table `did` (cached). `Ok(None)` when the DID is not a string-table DID or the
    /// locale has no such table.
    pub fn table(&self, did: u32) -> Result<Option<Arc<StringTable>>> {
        if !is_string_table_did(did) {
            return Ok(None);
        }
        self.cache.get_or_load(did, self.source.as_ref())
    }

    /// The parsed message for (table, token), or why it missed (spec §9 step 2).
    pub fn message(&self, table: u32, token: u32) -> Result<Message, Miss> {
        self.with_entry(table, token, Message::from_entry)
    }

    /// Runs `f` on the stored entry for (table, token), or reports the miss.
    pub fn with_entry<T>(
        &self,
        table: u32,
        token: u32,
        f: impl FnOnce(&StringTableEntry) -> T,
    ) -> Result<T, Miss> {
        let miss = |reason| Miss {
            table,
            token,
            reason,
        };
        if !is_string_table_did(table) {
            return Err(miss(MissReason::NotStringTable));
        }
        match self.table(table) {
            Err(e) => Err(miss(MissReason::Unreadable(e.to_string()))),
            Ok(None) => Err(miss(MissReason::NoTable)),
            Ok(Some(t)) => t.get(token).map(f).ok_or_else(|| miss(MissReason::NoToken)),
        }
    }

    /// Renders the entry (table, token) with `values`. A miss renders as empty text
    /// (plain) or a placeholder (explorer) and is reported in the diagnostics.
    #[must_use]
    pub fn render_entry(
        &self,
        table: u32,
        token: u32,
        values: &Values,
        opts: &RenderOptions<'_>,
    ) -> Rendered {
        let mut r = Renderer::new(opts);
        match self.message(table, token) {
            Ok(msg) => {
                let text = r.render_top(&msg, values);
                r.finish(text)
            }
            Err(miss) => miss_rendered(r, opts, miss),
        }
    }

    /// Resolves and renders a StringInfo (spec §9).
    ///
    /// The value map is `values` overlaid with the StringInfo's own argument block (its
    /// arguments win for the IDs they set). Only arguments the resolved message uses are
    /// built (recursively for nested StringInfos), and each referenced entry is parsed
    /// once per call. A nested StringInfo that misses fills its slots as an absent value
    /// (even if `values` had one) and the miss is reported.
    #[must_use]
    pub fn resolve(
        &self,
        info: &StringInfo,
        values: &Values,
        opts: &RenderOptions<'_>,
    ) -> Rendered {
        let mut r = Renderer::new(opts);
        let mut b = ArgBuilder::new(self, opts.max_depth);
        let msg = match b.message(&info.source) {
            Ok(m) => m,
            Err(miss) => return miss_rendered(r, opts, miss),
        };
        let mut merged = values.clone();
        b.add_args(info, &msg, &mut merged, 1);
        let text = r.render_top(&msg, &merged);
        r.finish(text)
    }

    /// Converts a StringInfo into a nested message value (for filling another message's
    /// slot). Fails only when the StringInfo itself misses. A nested argument that misses
    /// (or nests deeper than [`DEFAULT_MAX_DEPTH`]) becomes a [`Value::Unresolved`], which
    /// renders as absent and reports the miss in every render that uses it.
    pub fn to_value(&self, info: &StringInfo) -> Result<Value, Miss> {
        let mut b = ArgBuilder::new(self, DEFAULT_MAX_DEPTH);
        let msg = b.message(&info.source)?;
        let mut values = Values::new();
        b.add_args(info, &msg, &mut values, 1);
        Ok(Value::shared_message(msg, values))
    }
}

/// Builds StringInfo argument values for one [`Strings::resolve`] / [`Strings::to_value`]
/// call. Lookups are memoised per (table, token), misses included, so a StringInfo whose
/// arguments all name one entry parses it once.
struct ArgBuilder<'s> {
    strings: &'s Strings,
    max_depth: usize,
    memo: HashMap<(u32, u32), Result<Arc<Message>, Miss>>,
}

impl<'s> ArgBuilder<'s> {
    fn new(strings: &'s Strings, max_depth: usize) -> Self {
        Self {
            strings,
            max_depth,
            memo: HashMap::new(),
        }
    }

    fn message(&mut self, source: &StringSource) -> Result<Arc<Message>, Miss> {
        match source {
            StringSource::Literal(text) => Ok(Arc::new(Message::literal(&text.to_string_lossy()))),
            StringSource::Table { token, table } => {
                let strings = self.strings;
                self.memo
                    .entry((table.raw(), *token))
                    .or_insert_with(|| strings.message(table.raw(), *token).map(Arc::new))
                    .clone()
            }
        }
    }

    /// Adds the arguments of `info` that `msg` uses (spec §9 step 3) to `values`. `depth`
    /// is the level of `msg` (the outermost is 1). For repeated `var_id`s the last
    /// argument wins.
    fn add_args(&mut self, info: &StringInfo, msg: &Message, values: &mut Values, depth: usize) {
        let Some(args) = &info.args else { return };
        let used: HashSet<u32> = msg.arg_ids().into_iter().collect();
        let mut done: HashSet<u32> = HashSet::new();
        for arg in args.values.iter().rev() {
            if !used.contains(&arg.var_id) || !done.insert(arg.var_id) {
                continue;
            }
            let value = match &arg.value {
                StringArgValue::Integer(n) => Value::Integer(i64::from(*n)),
                StringArgValue::Float(f) => Value::from(*f),
                StringArgValue::Nested(nested) => self.nested_value(nested, depth + 1),
            };
            values.insert(arg.var_id, value);
        }
    }

    fn nested_value(&mut self, info: &StringInfo, depth: usize) -> Value {
        if depth > self.max_depth {
            return Value::Unresolved(Diagnostic::DepthLimit {
                limit: self.max_depth,
            });
        }
        match self.message(&info.source) {
            Err(miss) => Value::Unresolved(Diagnostic::Miss(miss)),
            Ok(msg) => {
                let mut values = Values::new();
                self.add_args(info, &msg, &mut values, depth);
                Value::shared_message(msg, values)
            }
        }
    }
}

fn miss_rendered(mut r: Renderer<'_>, opts: &RenderOptions<'_>, miss: Miss) -> Rendered {
    let mut text = RichText::new();
    if opts.mode == RenderMode::Explorer {
        text.push_str(
            &miss.placeholder(),
            &Style {
                placeholder: true,
                ..Style::default()
            },
        );
    }
    tracing::debug!(
        table = format_args!("{:#010x}", miss.table),
        token = format_args!("{:#010x}", miss.token),
        reason = %miss.reason,
        "string lookup miss"
    );
    r.diag(Diagnostic::Miss(miss));
    r.finish(text)
}
