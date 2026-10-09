// SPDX-License-Identifier: GPL-3.0-or-later
//! Compiled graphs: the nodes of one table script definition or one scriptlet, flattened
//! into an arena so that instances can refer to their node by index.
//!
//! A **unit** is the lexical scope of `script-runtime.md` §15 and §17: the nodes of one
//! script key's definition in one table, or all root trees of one scriptlet. Goto finds its
//! Label inside its unit, and a ScriptletParameter binds to the nearest Scriptlet instance
//! that invoked its unit.

use std::collections::HashMap;

use tessera_formats::Did;
use tessera_formats::script_table::{
    BaseBlock, Node, NodeData, NodeKind, PortPayload, PortType, Target,
};

/// Index of a compiled unit in the runtime.
pub(crate) type UnitId = u32;

/// What a unit was compiled from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum UnitKey {
    /// The definition of a script key in a table.
    Table(Did, u32),
    /// A scriptlet.
    Scriptlet(Did),
}

/// A node of a unit, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct NodeRef {
    pub unit: UnitId,
    pub node: u32,
}

/// A port with its children as node indices of the same unit.
#[derive(Debug, Clone)]
pub(crate) struct CPort {
    pub port_type: PortType,
    pub payload: PortPayload,
    pub children: Vec<u32>,
}

/// A compiled node: the decoded node without its child trees.
#[derive(Debug, Clone)]
pub(crate) struct CNode {
    pub kind: NodeKind,
    pub data: NodeData,
    pub base: BaseBlock,
    /// The Particle node's stored never-ending byte (`particle-runtime.md` §5.3).
    pub particle_flag: Option<bool>,
    pub targets: Vec<Target>,
    pub ports: Vec<CPort>,
    /// Indices of the Default ports in the kind's start order: by `u32` payload for
    /// Collection, Label and Sequence (§9, §10.1), by time for Timer (§11.1), stored order
    /// for every other kind. Stable, so equal payloads keep stored order (OPEN-6 Decision).
    pub ordered: Vec<u32>,
}

impl CNode {
    /// Whether the node is "persistent" for the gate of `script-runtime.md` §3.1 step 1.
    pub fn is_persistent(&self) -> bool {
        match &self.data {
            NodeData::Switch(s) => s.persistent,
            NodeData::WhenRunning { persistent, .. } => *persistent,
            NodeData::Timer(t) => t.loop_forever,
            _ => false,
        }
    }

    /// Ports of type `ty` with their indices, in stored order.
    pub fn ports_of(&self, ty: PortType) -> impl Iterator<Item = (usize, &CPort)> {
        self.ports
            .iter()
            .enumerate()
            .filter(move |(_, p)| p.port_type == ty)
    }
}

/// A compiled unit.
#[derive(Debug, Clone)]
pub(crate) struct Unit {
    pub key: UnitKey,
    pub nodes: Vec<CNode>,
    pub roots: Vec<u32>,
    /// Label name → the first Label node of that name in pre-order (§15 step 1).
    pub labels: HashMap<String, u32>,
}

impl Unit {
    /// Compiles `roots` (pre-order numbering).
    pub fn compile(key: UnitKey, roots: &[Node]) -> Self {
        let mut u = Self {
            key,
            nodes: Vec::new(),
            roots: Vec::with_capacity(roots.len()),
            labels: HashMap::new(),
        };
        for r in roots {
            let i = u.add(r);
            u.roots.push(i);
        }
        u
    }

    /// Adds `n` and its subtree; returns its index. Recursion depth is the decoded node
    /// depth, which `tessera-formats` bounds by `MAX_NODE_DEPTH`.
    fn add(&mut self, n: &Node) -> u32 {
        let idx = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        if let NodeData::Label { name } = &n.data {
            self.labels.entry(name.clone()).or_insert(idx);
        }
        self.nodes.push(CNode {
            kind: n.kind,
            data: n.data.clone(),
            base: n.base.clone(),
            particle_flag: n.particle_flag,
            targets: n.targets.clone(),
            ports: Vec::with_capacity(n.ports.len()),
            ordered: Vec::new(),
        });
        let mut ports = Vec::with_capacity(n.ports.len());
        for p in &n.ports {
            let children = p.children.iter().map(|c| self.add(c)).collect();
            ports.push(CPort {
                port_type: p.port_type,
                payload: p.payload.clone(),
                children,
            });
        }
        let ordered = order_ports(n.kind, &ports);
        if let Some(slot) = self.nodes.get_mut(idx as usize) {
            slot.ports = ports;
            slot.ordered = ordered;
        }
        idx
    }

    /// The node at `i`.
    pub fn node(&self, i: u32) -> Option<&CNode> {
        self.nodes.get(i as usize)
    }
}

/// The Default-port start order of a node kind.
fn order_ports(kind: NodeKind, ports: &[CPort]) -> Vec<u32> {
    let defaults = ports
        .iter()
        .enumerate()
        .filter(|(_, p)| p.port_type == PortType::DEFAULT);
    let mut v: Vec<(u32, &CPort)> = defaults
        .map(|(i, p)| (u32::try_from(i).unwrap_or(u32::MAX), p))
        .collect();
    match kind {
        NodeKind::COLLECTION | NodeKind::LABEL | NodeKind::SEQUENCE => {
            v.sort_by_key(|(_, p)| match p.payload {
                PortPayload::U32(x) => x,
                _ => 0,
            });
        }
        NodeKind::TIMER => {
            v.sort_by(|(_, a), (_, b)| port_time(a).total_cmp(&port_time(b)));
        }
        _ => {}
    }
    v.into_iter().map(|(i, _)| i).collect()
}

/// A Timer port's time in seconds; a missing or non-finite time counts as 0.
pub(crate) fn port_time(p: &CPort) -> f32 {
    match p.payload {
        PortPayload::F32(t) if t.is_finite() => t,
        _ => 0.0,
    }
}

/// A script's definition on an entity: the roots of every defining table in the closure,
/// grouped by table kind (1, then 2, then 4), and the merged per-script settings
/// (`script-runtime.md` §12.2–§12.3).
#[derive(Debug, Clone, Default)]
pub(crate) struct ScriptDef {
    pub roots: Vec<NodeRef>,
    /// Concurrent runs allowed per entity (≥ 1).
    pub max_instances: u32,
}
