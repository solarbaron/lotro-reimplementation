// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the runtime gets script tables (0x07) and scriptlets (0x0C).
//!
//! The runtime asks for each record once and keeps it ([`crate::Runtime`] caches decoded
//! records and the compiled graphs built from them).

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::property::PropertySchema;
use tessera_formats::{Did, ScriptTable, Scriptlet};

/// A supplier of decoded script records. `None` means "missing or undecodable"; the
/// runtime reports it once ([`crate::RuntimeIssue::MissingRecord`]) and treats the record as
/// absent.
pub trait ScriptSource {
    /// The script table `did`.
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>>;
    /// The scriptlet `did`.
    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>>;
}

/// Records held in memory (synthetic graphs, tests).
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    tables: HashMap<Did, Arc<ScriptTable>>,
    scriptlets: HashMap<Did, Arc<Scriptlet>>,
}

impl MemorySource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds (or replaces) a table under `did` (its `did` field is set to match).
    pub fn insert_table(&mut self, did: Did, mut table: ScriptTable) {
        table.did = did;
        self.tables.insert(did, Arc::new(table));
    }

    /// Adds (or replaces) a scriptlet under `did`.
    pub fn insert_scriptlet(&mut self, did: Did, mut scriptlet: Scriptlet) {
        scriptlet.did = did;
        self.scriptlets.insert(did, Arc::new(scriptlet));
    }
}

impl ScriptSource for MemorySource {
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        self.tables.get(&did).cloned()
    }

    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        self.scriptlets.get(&did).cloned()
    }
}

/// Decodes records from raw payloads: `fetch` returns a record's payload (or `None`), and
/// `schema` types the property values inside nodes (normally the property dictionary).
///
/// Decode failures are kept in [`FetchSource::failures`] (DID and error text) and the
/// record counts as missing.
pub struct FetchSource<'s, S: ?Sized, F> {
    schema: &'s S,
    fetch: F,
    failures: Vec<(Did, String)>,
}

impl<S: ?Sized, F> std::fmt::Debug for FetchSource<'_, S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchSource")
            .field("failures", &self.failures.len())
            .finish_non_exhaustive()
    }
}

impl<'s, S, F, B> FetchSource<'s, S, F>
where
    S: PropertySchema + ?Sized,
    F: FnMut(Did) -> Option<B>,
    B: AsRef<[u8]>,
{
    /// A source over `fetch`, decoding with `schema`.
    pub fn new(schema: &'s S, fetch: F) -> Self {
        Self {
            schema,
            fetch,
            failures: Vec::new(),
        }
    }

    /// Records that failed to decode, with the error text.
    #[must_use]
    pub fn failures(&self) -> &[(Did, String)] {
        &self.failures
    }
}

impl<S, F, B> ScriptSource for FetchSource<'_, S, F>
where
    S: PropertySchema + ?Sized,
    F: FnMut(Did) -> Option<B>,
    B: AsRef<[u8]>,
{
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        if did.prefix() != tessera_formats::script_table::SCRIPT_TABLE_PREFIX {
            return None;
        }
        let bytes = (self.fetch)(did)?;
        match ScriptTable::parse_expecting(bytes.as_ref(), did, self.schema) {
            Ok(t) => Some(Arc::new(t)),
            Err(e) => {
                tracing::warn!(%did, error = %e, "script table does not decode");
                self.failures.push((did, e.to_string()));
                None
            }
        }
    }

    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        if did.prefix() != tessera_formats::script_table::SCRIPTLET_PREFIX {
            return None;
        }
        let bytes = (self.fetch)(did)?;
        match Scriptlet::parse_expecting(bytes.as_ref(), did, self.schema) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                tracing::warn!(%did, error = %e, "scriptlet does not decode");
                self.failures.push((did, e.to_string()));
                None
            }
        }
    }
}
