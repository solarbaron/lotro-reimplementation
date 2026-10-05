// SPDX-License-Identifier: GPL-3.0-or-later
//! The 0x07 script-table record and the 0x0C scriptlet record.
//!
//! Spec: `docs/specs/script-table.md` §2 (table), §3 (scriptlet), §11.1 (algorithm), §12
//! (invariants).

use std::collections::HashSet;

use super::node::{MIN_NODE_SIZE, Node, NodeData, NodeDecoder, NodeKind, PortType};
use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::property::PropertySchema;
use crate::property_set::PropertySet;
use crate::reader::{Reader, TSize, keys_in_bucket_order};

/// Table kinds observed in the install (§2.1). Others decode but are reported as unusual.
pub const TABLE_KINDS: [u32; 3] = [1, 2, 4];

/// One script of a table: a key and its root nodes (§2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// Script key (enum 0x23000097).
    pub key: u32,
    /// Root nodes.
    pub roots: Vec<Node>,
}

/// Node statistics of a record, for censuses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeCensus {
    /// Nodes in all scripts (or roots), nested nodes included.
    pub nodes: u64,
    /// Ports, empty ones included.
    pub ports: u64,
    /// Top-level targets of all nodes (sub-targets not counted).
    pub targets: u64,
    /// Animation nodes.
    pub animations: u64,
    /// Nodes per kind code.
    pub by_kind: Vec<(NodeKind, u64)>,
    /// Deepest nesting (1 for a lone root).
    pub max_depth: usize,
}

impl NodeCensus {
    fn add(&mut self, roots: &[Node]) {
        for root in roots {
            root.walk(&mut |n, depth| {
                self.nodes += 1;
                self.ports += n.ports.len() as u64;
                self.targets += n.targets.len() as u64;
                self.animations += u64::from(n.kind == NodeKind::ANIMATION);
                self.max_depth = self.max_depth.max(depth + 1);
                match self.by_kind.iter_mut().find(|(k, _)| *k == n.kind) {
                    Some((_, c)) => *c += 1,
                    None => self.by_kind.push((n.kind, 1)),
                }
            });
        }
        self.by_kind.sort_unstable();
    }
}

/// The first violation of the node invariants SI6–SI8 in `roots`, or `None`.
fn first_unusual_node(roots: &[Node], in_scriptlet: bool) -> Option<&'static str> {
    let mut why = None;
    let mut ids = HashSet::new();
    let mut dup_ids = false;
    for root in roots {
        root.walk(&mut |n, _| {
            if why.is_some() {
                return;
            }
            if n.base.node_id == 0 {
                why = Some("node ID 0");
            }
            if !ids.insert(n.base.node_id) {
                dup_ids = true;
            }
            if let NodeData::Animation(a) = &n.data {
                // Two nodes of the reference install name the null DID (§6.1 "2 name
                // missing clips"); anything else must be a clip.
                if !a.asset.is_null() && a.asset.kind() != DidKind::Animation {
                    why = Some("animation asset is not a 0x05 DID");
                } else if !n.targets.is_empty() {
                    why = Some("animation node with a target");
                } else if n.ports.iter().any(|p| p.port_type != PortType::DEFAULT) {
                    why = Some("animation node with a port other than Default");
                }
            }
            if !in_scriptlet && n.kind == NodeKind::SCRIPTLET_PARAMETER {
                why = Some("scriptlet parameter node in a table");
            }
            if let NodeData::Scriptlet { file, .. } = &n.data
                && !file.is_null()
                && file.prefix() != super::SCRIPTLET_PREFIX
            {
                why = Some("scriptlet node names a non-0x0C DID");
            }
        });
    }
    // Node IDs are unique within every scriptlet but not within every table (SI8).
    why.or((in_scriptlet && dup_ids).then_some("duplicate node ID in a scriptlet"))
}

/// A decoded script table (prefix 0x07, `script-table.md` §2).
///
/// The record does not store its own DID; [`ScriptTable::did`] is the DID the caller looked
/// it up under ([`ScriptTable::parse_expecting`]), or the null DID after
/// [`ScriptTable::parse`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptTable {
    /// DID the record was decoded for (not stored in the payload).
    pub did: Did,
    /// Table kind: 1, 2 or 4 (§2.2; the difference between 1 and 2 is OPEN). Kind-4 tables
    /// carry only DID lists.
    pub kind: u32,
    /// Included tables (0x07 DIDs), in stored order.
    pub includes: Vec<Did>,
    /// Header of the script map; only the hint matters (the count is `scripts.len()`).
    pub scripts_size: TSize,
    /// Scripts in stored (bucket) order.
    pub scripts: Vec<Script>,
    /// Header of the per-script property-set map.
    pub properties_size: TSize,
    /// Per-script property sets (script-level settings, §2.4), keyed by script key.
    pub script_properties: Vec<(u32, PropertySet)>,
    /// Header of the per-script DID-list map.
    pub dids_size: TSize,
    /// Per-script preload lists (§2.5), keyed by script key.
    pub script_dids: Vec<(u32, Vec<Did>)>,
}

impl ScriptTable {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "script table";

    /// Decodes a complete payload, which must be consumed exactly. `schema` (normally the
    /// [`crate::PropertyDictionary`]) types the PVs and property sets inside nodes.
    ///
    /// # Errors
    ///
    /// Truncation, trailing bytes, an unknown node kind or target type, a port echo that
    /// differs from its type, a target marker other than 1, a non-zero switch-case reserved
    /// field, a CinematicCamera node (its data layout is OPEN, §10.3), nesting deeper than
    /// [`super::MAX_NODE_DEPTH`], an unknown PID, or the memory budget.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let (rec, trailing) = Self::parse_prefix(data, schema)?;
        if trailing != 0 {
            return Err(FormatError::TrailingBytes {
                what: Self::NAME,
                offset: data.len() - trailing,
                count: trailing,
            });
        }
        Ok(rec)
    }

    /// [`ScriptTable::parse`] for the record stored under `did`, which is checked to be a
    /// 0x07 DID and kept in [`ScriptTable::did`].
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        did: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let did = did.into();
        if did.kind() != DidKind::ScriptTable {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let mut t = Self::parse(data, schema)?;
        t.did = did;
        Ok(t)
    }

    /// Decodes the record and returns it with the number of bytes left after it, for
    /// censuses.
    pub fn parse_prefix<S: PropertySchema + ?Sized>(
        data: &[u8],
        schema: &S,
    ) -> Result<(Self, usize)> {
        let mut r = Reader::new(data);
        let mut d = NodeDecoder::new(schema);
        let kind = r.u32()?;
        let n = r.vle()?;
        let mut includes = r.alloc_vec(n, 4, usize::MAX)?;
        for _ in 0..n {
            let did = r.did()?;
            r.push(&mut includes, did, n)?;
        }
        let scripts_size = r.tsize()?;
        let n = scripts_size.count;
        // A script is at least a key and a one-byte root count.
        let mut scripts = r.alloc_vec(n, 5, usize::MAX)?;
        for _ in 0..n {
            let key = r.u32()?;
            let roots_n = r.vle()?;
            r.ensure_elements(roots_n, MIN_NODE_SIZE)?;
            let roots = d.nodes(&mut r, roots_n)?;
            r.push(&mut scripts, Script { key, roots }, n)?;
        }
        let properties_size = r.tsize()?;
        let n = properties_size.count;
        let mut script_properties = r.alloc_vec(n, 6, usize::MAX)?;
        for _ in 0..n {
            let key = r.u32()?;
            let set = d.property_set(&mut r)?;
            r.push(&mut script_properties, (key, set), n)?;
        }
        let dids_size = r.tsize()?;
        let n = dids_size.count;
        let mut script_dids = r.alloc_vec(n, 8, usize::MAX)?;
        for _ in 0..n {
            let key = r.u32()?;
            let count_at = r.position();
            let count = r.i32()?;
            let count = u32::try_from(count).map_err(|_| {
                FormatError::malformed(Self::NAME, count_at, format!("negative DID count {count}"))
            })?;
            let mut dids = r.alloc_vec(count, 4, usize::MAX)?;
            for _ in 0..count {
                let did = r.did()?;
                r.push(&mut dids, did, count)?;
            }
            r.push(&mut script_dids, (key, dids), n)?;
        }
        r.log_unknown_hints(Self::NAME);
        tracing::trace!(nodes = d.node_count(), "script table decoded");
        Ok((
            Self {
                did: Did(0),
                kind,
                includes,
                scripts_size,
                scripts,
                properties_size,
                script_properties,
                dids_size,
                script_dids,
            },
            r.remaining(),
        ))
    }

    /// The script with key `key`, if this table defines it. A linear search in stored
    /// order; for repeated lookups build an index once (the clip resolver does).
    #[must_use]
    pub fn script(&self, key: u32) -> Option<&Script> {
        self.scripts.iter().find(|s| s.key == key)
    }

    /// The property set of script `key`, if present.
    #[must_use]
    pub fn script_properties(&self, key: u32) -> Option<&PropertySet> {
        self.script_properties
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, s)| s)
    }

    /// The preload DID list of script `key`, if present.
    #[must_use]
    pub fn script_dids(&self, key: u32) -> Option<&[Did]> {
        self.script_dids
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, d)| d.as_slice())
    }

    /// Node statistics over all scripts.
    #[must_use]
    pub fn census(&self) -> NodeCensus {
        let mut c = NodeCensus::default();
        for s in &self.scripts {
            c.add(&s.roots);
        }
        c
    }

    /// The first deviation from the invariants observed on every real record (SI1, SI3,
    /// SI6, SI7, SI9; includes naming 0x07 records other than the table itself), or `None`.
    /// Such records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if !TABLE_KINDS.contains(&self.kind) {
            return Some("table kind other than 1, 2 or 4");
        }
        if self.kind == 4 && (!self.scripts.is_empty() || !self.script_properties.is_empty()) {
            return Some("kind-4 table with scripts or property sets");
        }
        if self
            .includes
            .iter()
            .any(|d| d.kind() != DidKind::ScriptTable)
        {
            return Some("include is not a 0x07 DID");
        }
        if !self.did.is_null() && self.includes.contains(&self.did) {
            return Some("table includes itself");
        }
        if keys_in_bucket_order(self.scripts_size.hint, self.scripts.iter().map(|s| s.key))
            == Some(false)
        {
            return Some("script keys not in bucket order");
        }
        let keys: HashSet<u32> = self.scripts.iter().map(|s| s.key).collect();
        if self
            .script_properties
            .iter()
            .any(|(k, _)| !keys.contains(k))
        {
            return Some("property-set key without a script");
        }
        self.scripts
            .iter()
            .find_map(|s| first_unusual_node(&s.roots, false))
    }
}

/// A decoded scriptlet (prefix 0x0C, `script-table.md` §3): root nodes and a preload list.
///
/// Like tables, scriptlets do not store their own DID; see [`ScriptTable`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scriptlet {
    /// DID the record was decoded for (not stored in the payload).
    pub did: Did,
    /// Root nodes.
    pub roots: Vec<Node>,
    /// Dependency DIDs to preload (sound info, clips, …).
    pub dependencies: Vec<Did>,
}

impl Scriptlet {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "scriptlet";

    /// Decodes a complete payload, which must be consumed exactly (errors as for
    /// [`ScriptTable::parse`]).
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let (rec, trailing) = Self::parse_prefix(data, schema)?;
        if trailing != 0 {
            return Err(FormatError::TrailingBytes {
                what: Self::NAME,
                offset: data.len() - trailing,
                count: trailing,
            });
        }
        Ok(rec)
    }

    /// [`Scriptlet::parse`] for the record stored under `did`, which is checked to be a
    /// 0x0C DID and kept in [`Scriptlet::did`].
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        did: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let did = did.into();
        if did.prefix() != super::SCRIPTLET_PREFIX {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let mut s = Self::parse(data, schema)?;
        s.did = did;
        Ok(s)
    }

    /// Decodes the record and returns it with the number of bytes left after it.
    pub fn parse_prefix<S: PropertySchema + ?Sized>(
        data: &[u8],
        schema: &S,
    ) -> Result<(Self, usize)> {
        let mut r = Reader::new(data);
        let mut d = NodeDecoder::new(schema);
        let n = r.vle()?;
        let roots = d.nodes(&mut r, n)?;
        let n = r.u32()?;
        let mut dependencies = r.alloc_vec(n, 4, usize::MAX)?;
        for _ in 0..n {
            let did = r.did()?;
            r.push(&mut dependencies, did, n)?;
        }
        r.log_unknown_hints(Self::NAME);
        Ok((
            Self {
                did: Did(0),
                roots,
                dependencies,
            },
            r.remaining(),
        ))
    }

    /// Node statistics over the roots.
    #[must_use]
    pub fn census(&self) -> NodeCensus {
        let mut c = NodeCensus::default();
        c.add(&self.roots);
        c
    }

    /// The first deviation from the node invariants observed on every real record (SI6,
    /// SI8, SI9), or `None`. Such records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        first_unusual_node(&self.roots, true)
    }
}
