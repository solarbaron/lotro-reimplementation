// SPDX-License-Identifier: GPL-3.0-or-later
//! Instance scripts: the per-instance script table `Entity_InstanceScriptTable` (a script
//! table written as a property tree), an entity's table set with its instance definitions,
//! the lookup of a key through it, link targets, member-override names and instance
//! switches.
//!
//! Spec: `docs/specs/instance-scripts.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`InstanceScriptLabels`], [`NodeFieldLabels`] | The labels of the tree and of the node fields converted to typed data | §3.2, §3.3 |
//! | [`InstanceScriptTable`], [`InstanceScript`], [`TreeNode`], [`TreePort`], [`TreeTarget`] | The tree, parsed with the client's acceptance rules | §3, §4.3, §4.5, §9.2, §9.3 |
//! | [`TreeNode::to_node`] | Conversion into the node model of `script-table.md` §4 (Decision Q3 for absent fields) | §3.3, §4.5 |
//! | [`EntityTableSet`], [`EntityTableSet::gather`], [`EntityTableSet::merge`], [`EntityTableSet::startup_keys`] | An entity's table set: includes, start-up list, instance definitions | §4.1–§4.3, §9.1 |
//! | [`EntityTableSet::lookup`], [`KeyLookup`] | The lookup of a key with instance definitions first | §4.4 |
//! | [`link_targets`], [`SCRIPT_LINK_TYPE`] | Link-target resolution (Decision Q4) | §3.5 |
//! | [`MemberOverride`] | Member-override names (`Owner@Holder.member`) | §5.1 |
//! | [`INSTANCE_SWITCH_LABELS`] | The instance switch properties | §8.1 |
//!
//! Hostile input is bounded: node nesting at most [`MAX_TREE_DEPTH`] levels (the install
//! needs 2) and target nesting at most [`MAX_TREE_DEPTH`]; the property decoder's own limits
//! bound counts and value depth (`property-system.md` §5.3).

use std::collections::HashSet;

use super::node::{
    BaseBlock, Node, NodeData, NodeKind, Port, PortPayload, PortType, RunNode, SwitchCase,
    SwitchNode, Target, TargetData, TargetValue, TimerNode,
};
use super::record::ScriptTable;
use crate::did::Did;
use crate::gameplay::common::{Pid, PropertyAccess, PropertyDefaults, ValueExt};
use crate::landblock::{InstanceId, Link};
use crate::property::{PropertyEntry, PropertyStruct, PropertyType, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::TSize;

/// Deepest node or target nesting built from a tree (§9.3 step 4).
pub const MAX_TREE_DEPTH: usize = 32;
/// Link type of Script links (`landblock.md` §4.6).
pub const SCRIPT_LINK_TYPE: u32 = 12;
/// The `Startup` script key (`script-table.md` §8.1).
pub const STARTUP_KEY: u32 = 5;
/// `Script_CombinationType` value `Overriding` (§4.4).
pub const COMBINATION_OVERRIDING: u32 = 1;

crate::property_ids! {
    /// The labels of an instance script table and of an entity's tables (§3.2, §4.1).
    pub struct InstanceScriptLabels {
        instance_table = "Entity_InstanceScriptTable",
        scripts = "ScriptTable_Scripts",
        startup_names = "ScriptTable_StartupScriptNames",
        entry = "ScriptTable_ScriptEntry",
        script_id = "ScriptTable_ScriptID",
        script_properties = "ScriptTable_ScriptProperties",
        script_roots = "ScriptTable_ScriptRoots",
        node = "ScriptTable_Node",
        node_type = "ScriptTable_NodeType",
        node_properties = "ScriptTable_NodeProperties",
        node_outputs = "ScriptTable_NodeOutputs",
        node_port = "ScriptTable_NodePort",
        port_type = "ScriptTable_PortType",
        port_properties = "ScriptTable_PortProperties",
        nodes = "ScriptTable_Nodes",
        node_targets = "ScriptTable_NodeTargets",
        target_port = "ScriptTable_TargetPort",
        node_target_ex = "ScriptTable_NodeTargetEx",
        target_type = "ScriptTable_TargetType",
        node_target = "ScriptTable_NodeTarget",
        driven_properties = "Script_DrivenProperties",
        driven_entry = "Script_DrivenPropertyEntry",
        driven_property = "Script_DrivenProperty",
        driver_property = "Script_DriverProperty",
        link_target_name = "LinkTarget_Name",
        combination_type = "Script_CombinationType",
        class_table = "Entity_ClassScriptTable",
        class_table_override = "Entity_ClassScriptTable_Override",
        additional_tables = "Entity_AdditionalScriptTables",
        additional_tables_override = "Entity_AdditionalScriptTables_Override",
        db_script_table = "ScriptTable_DBScriptTable",
        script_channels = "ScriptTable_ScriptChannels",
        player_physics_script = "Entity_PlayerPhysicsScript",
    }
}

crate::property_ids! {
    /// The node and port fields a tree node is converted with (§3.3; field PIDs of
    /// `script-table.md` §5).
    pub struct NodeFieldLabels {
        run_id = "RunScript_ID",
        run_persistent = "RunScript_Persistent",
        run_stop = "RunScript_Stop",
        run_unbind_on_death = "RunScript_UnbindOnDeath",
        run_expose = "RunScript_ExposeAsScriptLink",
        run_queuing = "RunScript_QueuingType",
        set_property_name = "SetPropertyScript_PropertyName",
        set_property_server_only = "SetPropertyScript_ServerOnly",
        switch_property = "SwitchScript_Property",
        switch_persistent = "SwitchScript_Persistent",
        timer_duration = "TimerScript_Duration",
        scriptlet_file = "ScriptletScript_File",
        appearance = "AppearanceScript_Appearance",
        appearance_key = "AppearanceScript_Key",
        appearance_modifier = "AppearanceScript_Modifier",
        appearance_persistent = "AppearanceScript_Persistent",
        use_parent_intensity = "Script_UseParentIntensity",
        intensity_multiplier = "Script_IntensityMultiplier",
        intensity_variance = "Script_IntensityMultiplierVariance",
        probability = "Script_Probability",
        sequence_order = "SequenceScriptPort_Order",
        random_weight = "RandomScriptPort_Weight",
        timer_time = "TimerScriptPort_Time",
        switch_values = "SwitchScriptPort_Values",
        switch_operator = "SwitchScriptPort_Operator",
        switch_ramp_out = "SwitchScriptPort_RampOut",
        switch_delay = "SwitchScriptPort_Delay",
    }
}

/// A key value of type 8 (enum) or 10 (property ID), the two types the client accepts for
/// script keys, node kinds and port types (§4.1, §4.3, §4.5).
fn key_of(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::PropertyId(k) => Some(*k),
        _ => None,
    }
}

fn key_field<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Option<u32> {
    key_of(a.get_pid(pid)?)
}

// ---------------------------------------------------------------------------------------------
// The tree (§3, §4.5)
// ---------------------------------------------------------------------------------------------

/// One output port of a tree node (§3.2 `ScriptTable_NodePort`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreePort {
    /// `ScriptTable_PortType` (0 when it is neither an enum nor a property ID, §4.5 step 3).
    pub port_type: u32,
    /// `ScriptTable_PortProperties` elements (payload fields).
    pub payload: Vec<PropertyEntry>,
    /// Child nodes from `ScriptTable_Node` (one) and `ScriptTable_Nodes` (a list).
    pub children: Vec<TreeNode>,
}

/// One target of a tree node (§3.2 `ScriptTable_TargetPort`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeTarget {
    /// `ScriptTable_PortType` (0 when absent or of another type).
    pub port_type: u32,
    /// `ScriptTable_PortProperties` elements.
    pub payload: Vec<PropertyEntry>,
    /// `ScriptTable_TargetType` of the `ScriptTable_NodeTargetEx` (§4.4 of
    /// `script-table.md`: 7 `Link`, 13 `This`, …).
    pub target_type: Option<u32>,
    /// The data fields of the `ScriptTable_NodeTargetEx` (its `ScriptTable_NodeProperties`).
    pub fields: Vec<PropertyEntry>,
    /// `LinkTarget_Name` among the data fields (a Link target's link name).
    pub link_name: Option<String>,
    /// Sub-targets.
    pub sub_targets: Vec<TreeTarget>,
}

/// One node built from a `ScriptTable_Node` struct (§4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// The kind (`ScriptTable_NodeType`), a registered kind of `script-table.md` §5.1.
    pub kind: NodeKind,
    /// The field elements of `ScriptTable_NodeProperties` other than the driven map, in
    /// stored order (the element PID is the field's bound PID).
    pub fields: Vec<PropertyEntry>,
    /// The driven map (`Script_DrivenProperties`): (field PID, source PID) pairs.
    pub driven: Vec<(u32, u32)>,
    /// Output ports.
    pub ports: Vec<TreePort>,
    /// Targets.
    pub targets: Vec<TreeTarget>,
}

/// One script of an instance script table (§3.2 `ScriptTable_ScriptEntry`).
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceScript {
    /// The script key (`ScriptTable_ScriptID`).
    pub key: u32,
    /// `ScriptTable_ScriptProperties` fields when it is a struct.
    pub properties: Option<PropertySet>,
    /// The roots that built, in stored order.
    pub roots: Vec<TreeNode>,
    /// Roots that failed to build and were left out (§4.3 step 2.2).
    pub failed_roots: usize,
}

impl InstanceScript {
    /// `Script_CombinationType` of the per-script properties as an enum or property ID.
    #[must_use]
    pub fn combination_type(&self, labels: &InstanceScriptLabels) -> Option<u32> {
        key_field(self.properties.as_ref()?, labels.combination_type)
    }
}

/// An `Entity_InstanceScriptTable` value parsed with the client's rules (§4.3, §9.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InstanceScriptTable {
    /// The scripts, in stored order; a repeated key keeps its **last** definition, at the
    /// position of the first (§4.3 step 2.3).
    pub scripts: Vec<InstanceScript>,
    /// `ScriptTable_StartupScriptNames` keys (enum or property-ID elements), stored order.
    pub startup: Vec<u32>,
    /// Script entries skipped: not a struct, or no key of type 8 or 10.
    pub skipped_entries: usize,
    /// Start-up elements skipped: another type (§4.1).
    pub skipped_startup: usize,
    /// Repeated keys whose earlier definition was replaced.
    pub replaced: usize,
}

/// What the tree parser builds with.
struct TreeBuilder<'a> {
    labels: &'a InstanceScriptLabels,
}

impl TreeBuilder<'_> {
    /// §4.5: a node struct, or `None` when it fails (left out of its list).
    fn node(&self, s: &PropertyStruct, depth: usize) -> Option<TreeNode> {
        let l = self.labels;
        if depth >= MAX_TREE_DEPTH {
            tracing::debug!(depth, "instance script node nesting limit");
            return None;
        }
        let kind = key_field(s, l.node_type)?;
        let props = s.array(l.node_properties)?;
        let kind = NodeKind(u8::try_from(kind).ok()?);
        if !kind.is_known() {
            return None;
        }
        let mut fields = Vec::new();
        let mut driven = Vec::new();
        for e in &props.elements {
            if l.driven_properties.is(e.pid) {
                for d in e.value.children() {
                    if let Some(ds) = d.value.as_struct()
                        && let (Some(f), Some(src)) = (
                            ds.property_ref(l.driven_property),
                            ds.property_ref(l.driver_property),
                        )
                    {
                        driven.push((f, src));
                    }
                }
            } else {
                fields.push(e.clone());
            }
        }
        let ports = s
            .elements(l.node_outputs)
            .iter()
            .filter_map(|e| e.value.as_struct())
            .map(|p| self.port(p, depth))
            .collect();
        let targets = self.targets(s, depth);
        Some(TreeNode {
            kind,
            fields,
            driven,
            ports,
            targets,
        })
    }

    fn port(&self, p: &PropertyStruct, depth: usize) -> TreePort {
        let l = self.labels;
        let port_type = p.get_pid(l.port_type).and_then(key_of).unwrap_or(0);
        let mut children: Vec<TreeNode> = p
            .structure(l.node)
            .and_then(|c| self.node(c, depth + 1))
            .into_iter()
            .collect();
        children.extend(
            p.elements(l.nodes)
                .iter()
                .filter_map(|e| e.value.as_struct())
                .filter_map(|c| self.node(c, depth + 1)),
        );
        TreePort {
            port_type,
            payload: p.elements(l.port_properties).to_vec(),
            children,
        }
    }

    fn targets<A: PropertyAccess + ?Sized>(&self, holder: &A, depth: usize) -> Vec<TreeTarget> {
        let l = self.labels;
        if depth >= MAX_TREE_DEPTH {
            return Vec::new();
        }
        holder
            .elements(l.node_targets)
            .iter()
            .filter_map(|e| e.value.as_struct())
            .map(|t| {
                let ex = t.structure(l.node_target_ex);
                let fields: Vec<PropertyEntry> = ex
                    .map(|x| x.elements(l.node_properties).to_vec())
                    .unwrap_or_default();
                let link_name = fields
                    .iter()
                    .find(|f| l.link_target_name.is(f.pid))
                    .and_then(|f| f.value.string())
                    .map(str::to_owned);
                TreeTarget {
                    port_type: t.get_pid(l.port_type).and_then(key_of).unwrap_or(0),
                    payload: t.elements(l.port_properties).to_vec(),
                    target_type: ex
                        .and_then(|x| key_field(x, l.target_type))
                        .or_else(|| key_field(t, l.target_type)),
                    fields,
                    link_name,
                    sub_targets: ex.map(|x| self.targets(x, depth + 1)).unwrap_or_default(),
                }
            })
            .collect()
    }
}

impl InstanceScriptTable {
    /// Parses an `Entity_InstanceScriptTable` value (§4.3 step 2, §9.2). `None` when the
    /// value is not a struct; a struct without a `ScriptTable_Scripts` array has no scripts.
    #[must_use]
    pub fn parse(labels: &InstanceScriptLabels, value: &PropertyValue) -> Option<Self> {
        let s = value.as_struct()?;
        let b = TreeBuilder { labels };
        let mut out = Self::default();
        for e in s.elements(labels.startup_names) {
            match key_of(&e.value) {
                Some(k) => out.startup.push(k),
                None => out.skipped_startup += 1,
            }
        }
        let mut positions: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for e in s.elements(labels.scripts) {
            let Some(entry) = e.value.as_struct() else {
                out.skipped_entries += 1;
                continue;
            };
            let Some(key) = key_field(entry, labels.script_id) else {
                out.skipped_entries += 1;
                continue;
            };
            let mut roots = Vec::new();
            let mut failed_roots = 0;
            for r in entry.elements(labels.script_roots) {
                match r.value.as_struct().and_then(|n| b.node(n, 0)) {
                    Some(n) => roots.push(n),
                    None => failed_roots += 1,
                }
            }
            let properties = entry
                .structure(labels.script_properties)
                .map(|p| PropertySet {
                    size: TSize {
                        hint: 0,
                        count: u32::try_from(p.fields.len()).unwrap_or(u32::MAX),
                    },
                    entries: p.fields.clone(),
                });
            let script = InstanceScript {
                key,
                properties,
                roots,
                failed_roots,
            };
            // A map from key to position keeps repeated keys linear in the entry count.
            match positions.entry(key) {
                std::collections::hash_map::Entry::Occupied(at) => {
                    if let Some(old) = out.scripts.get_mut(*at.get()) {
                        *old = script;
                    }
                    out.replaced += 1;
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(out.scripts.len());
                    out.scripts.push(script);
                }
            }
        }
        Some(out)
    }

    /// The table stored in `a` under `Entity_InstanceScriptTable`, if any.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(labels: &InstanceScriptLabels, a: &A) -> Option<Self> {
        Self::parse(labels, a.get_pid(labels.instance_table)?)
    }

    /// The script of `key`.
    #[must_use]
    pub fn script(&self, key: u32) -> Option<&InstanceScript> {
        self.scripts.iter().find(|s| s.key == key)
    }

    /// Every Link target name of every node, depth first (for link resolution, §3.5).
    #[must_use]
    pub fn link_names(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for s in &self.scripts {
            for r in &s.roots {
                r.walk(&mut |n| {
                    for t in &n.targets {
                        collect_links(t, &mut out);
                    }
                });
            }
        }
        out
    }
}

fn collect_links<'a>(t: &'a TreeTarget, out: &mut Vec<&'a str>) {
    if let Some(n) = &t.link_name {
        out.push(n);
    }
    for s in &t.sub_targets {
        collect_links(s, out);
    }
}

impl TreeNode {
    /// Calls `f` for this node and every node below it, parents first.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a TreeNode)) {
        f(self);
        for p in &self.ports {
            for c in &p.children {
                c.walk(f);
            }
        }
    }

    /// The stored field `pid`.
    #[must_use]
    pub fn field(&self, pid: Pid) -> Option<&PropertyValue> {
        let id = pid.id()?;
        self.fields.iter().find(|e| e.pid == id).map(|e| &e.value)
    }

    /// Converts the node into the node model of `script-table.md` §4 (§3.3). Fields absent
    /// from the tree take **Decision Q3**: the PID's dictionary default when the definition
    /// has one, else the zero value of the type. Kinds without a typed form here keep their
    /// fields as [`NodeData::Fields`].
    #[must_use]
    pub fn to_node<D: PropertyDefaults + ?Sized>(&self, f: &NodeFieldLabels, defaults: &D) -> Node {
        let get = |pid: Pid| -> Option<PropertyValue> {
            self.field(pid)
                .cloned()
                .or_else(|| pid.id().and_then(|p| defaults.default_value(p)).cloned())
        };
        let boolean = |pid: Pid| get(pid).and_then(|v| v.boolean()).unwrap_or(false);
        let key = |pid: Pid| get(pid).as_ref().and_then(key_of).unwrap_or(0);
        let float = |pid: Pid| get(pid).and_then(|v| v.float()).unwrap_or(0.0);
        let data = match self.kind {
            NodeKind::COLLECTION | NodeKind::ON_STOP | NodeKind::TEST_CRASH => NodeData::None,
            NodeKind::RUN => NodeData::Run(Box::new(RunNode {
                version: 1,
                parameters: Vec::new(),
                script: key(f.run_id),
                persistent: boolean(f.run_persistent),
                unbind_on_death: boolean(f.run_unbind_on_death),
                stop: boolean(f.run_stop),
                expose_as_script_link: boolean(f.run_expose),
                queuing_type: key(f.run_queuing),
            })),
            NodeKind::SET_PROPERTY => {
                let property = key(f.set_property_name);
                let value = (property != 0)
                    .then(|| self.fields.iter().find(|e| e.pid == property).cloned())
                    .flatten()
                    .map(Box::new);
                NodeData::SetProperty {
                    property,
                    value,
                    server_only: boolean(f.set_property_server_only),
                }
            }
            NodeKind::SWITCH | NodeKind::WHEN => NodeData::Switch(SwitchNode {
                property: key(f.switch_property),
                persistent: boolean(f.switch_persistent),
                autonomous: false,
                synchronized: false,
            }),
            NodeKind::TIMER => NodeData::Timer(TimerNode {
                duration: float(f.timer_duration),
                ..TimerNode::default()
            }),
            NodeKind::SCRIPTLET => NodeData::Scriptlet {
                file: get(f.scriptlet_file)
                    .and_then(|v| v.did_ref())
                    .unwrap_or(Did(0)),
                parameters: PropertySet::default(),
            },
            NodeKind::SEQUENCE => NodeData::Sequence {
                loop_forever: false,
                loop_count: 0,
            },
            NodeKind::RANDOM => NodeData::Random {
                no_repeats: false,
                autonomous: false,
            },
            NodeKind::APPEARANCE => NodeData::Appearance {
                appearance: get(f.appearance)
                    .and_then(|v| v.did_ref())
                    .unwrap_or(Did(0)),
                key: key(f.appearance_key),
                modifier: float(f.appearance_modifier),
                persistent: boolean(f.appearance_persistent),
            },
            _ => NodeData::Fields(Box::new(PropertySet {
                size: TSize {
                    hint: 0,
                    count: u32::try_from(self.fields.len()).unwrap_or(u32::MAX),
                },
                entries: self.fields.clone(),
            })),
        };
        let base = BaseBlock {
            driven: self.driven.clone(),
            use_parent_intensity: boolean(f.use_parent_intensity),
            intensity_multiplier: float(f.intensity_multiplier),
            intensity_variance: float(f.intensity_variance),
            probability: float(f.probability),
            ..BaseBlock::default()
        };
        let ports = self
            .ports
            .iter()
            .map(|p| Port {
                port_type: PortType(u8::try_from(p.port_type).unwrap_or(0)),
                payload: port_payload(self.kind, p, f),
                children: p.children.iter().map(|c| c.to_node(f, defaults)).collect(),
            })
            .collect();
        Node {
            kind: self.kind,
            data,
            base,
            particle_flag: (self.kind == NodeKind::PARTICLE).then_some(false),
            ports,
            targets: self.targets.iter().map(to_target).collect(),
        }
    }
}

/// The payload of a tree port in the binary model, by (kind, port type) (§3.3 row "ports").
fn port_payload(kind: NodeKind, p: &TreePort, f: &NodeFieldLabels) -> PortPayload {
    let field = |pid: Pid| {
        let id = pid.id()?;
        p.payload.iter().find(|e| e.pid == id).map(|e| &e.value)
    };
    let u = |pid: Pid| {
        field(pid)
            .and_then(|v| v.as_bits())
            .and_then(|b| u32::try_from(b).ok())
    };
    let fl = |pid: Pid| field(pid).and_then(PropertyValue::as_f32).unwrap_or(0.0);
    match (kind.0, p.port_type) {
        (2 | 6, 6) => u(f.sequence_order).map_or(PortPayload::None, PortPayload::U32),
        (17, 6) => u(f.random_weight).map_or(PortPayload::None, PortPayload::U32),
        (65, 6) => field(f.timer_time)
            .and_then(PropertyValue::as_f32)
            .map_or(PortPayload::None, PortPayload::F32),
        (12, 5) => {
            let values = field(f.switch_values)
                .map(|v| v.children().iter().cloned().map(Some).collect())
                .unwrap_or_default();
            PortPayload::SwitchCase(Box::new(SwitchCase {
                values,
                ints: Vec::new(),
                operator: field(f.switch_operator)
                    .and_then(|v| v.as_bits())
                    .and_then(|b| i32::try_from(b).ok())
                    .unwrap_or(0),
                transition_time: fl(f.switch_ramp_out),
                second_time: fl(f.switch_delay),
            }))
        }
        (12, 6) => PortPayload::F32Pair(fl(f.switch_ramp_out), fl(f.switch_delay)),
        _ => PortPayload::None,
    }
}

fn to_target(t: &TreeTarget) -> Target {
    let target_type = t
        .target_type
        .and_then(|v| u8::try_from(v).ok())
        .unwrap_or(0);
    Target {
        port_type: PortType(u8::try_from(t.port_type).unwrap_or(0)),
        value: TargetValue::None,
        target_type,
        data: match (target_type, &t.link_name) {
            (7, Some(n)) => TargetData::Name(n.clone()),
            _ => TargetData::None,
        },
        sub_targets: t.sub_targets.iter().map(to_target).collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// The entity's table set (§4.1–§4.4)
// ---------------------------------------------------------------------------------------------

/// One own (instance) definition of a table set.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnDefinition {
    /// The script key.
    pub key: u32,
    /// The roots, converted to the node model.
    pub roots: Vec<Node>,
    /// The own per-script property set, if the entry had one.
    pub properties: Option<PropertySet>,
}

/// An entity's table set (§4.1): its own (instance) definitions and the ordered includes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityTableSet {
    /// Included 0x07 tables: the class table first, then the additional tables in stored
    /// order; null and repeated DIDs dropped.
    pub includes: Vec<Did>,
    /// The start-up list: the class table's `ScriptTable_StartupScriptNames`, then the
    /// instance table's (§4.1 steps 2–3).
    pub startup: Vec<u32>,
    /// Own definitions (the instance scripts merged by [`EntityTableSet::merge`]).
    pub own: Vec<OwnDefinition>,
    /// `Entity_PlayerPhysicsScript`, when non-zero.
    pub player_physics_script: Option<u32>,
    /// Start-up and additional-table elements of the wrong type, skipped (§4.1).
    pub skipped: usize,
}

/// The first layer that holds `pid` (the entity lookup, `entity-descriptor.md` §9.2).
fn first<'a, A: PropertyAccess + ?Sized>(layers: &[&'a A], pid: Pid) -> Option<&'a PropertyValue> {
    layers.iter().find_map(|l| l.get_pid(pid))
}

impl EntityTableSet {
    /// Builds the table set of an entity from its property lookup `layers` (the
    /// per-instance set, then the object's chain, nearest first), as §4.1 says; the
    /// instance table contributes only its start-up names here (merge its scripts with
    /// [`EntityTableSet::merge`]).
    #[must_use]
    pub fn gather<A: PropertyAccess + ?Sized>(
        labels: &InstanceScriptLabels,
        layers: &[&A],
    ) -> Self {
        let mut out = Self::default();
        let class = first(layers, labels.class_table_override)
            .or_else(|| first(layers, labels.class_table))
            .and_then(PropertyValue::as_struct);
        if let Some(c) = class {
            if let Some(d) = c.did_nonnull(labels.db_script_table) {
                out.includes.push(d);
            }
            out.push_startup(c.elements(labels.startup_names));
        }
        if let Some(ist) = first(layers, labels.instance_table).and_then(PropertyValue::as_struct) {
            out.push_startup(ist.elements(labels.startup_names));
        }
        let additional = first(layers, labels.additional_tables_override)
            .or_else(|| first(layers, labels.additional_tables));
        for e in additional.map(PropertyValue::children).unwrap_or(&[]) {
            match e.value.did_ref() {
                Some(d) if !d.is_null() && !out.includes.contains(&d) => out.includes.push(d),
                Some(_) => {}
                None => out.skipped += 1,
            }
        }
        out.player_physics_script = first(layers, labels.player_physics_script)
            .and_then(key_of)
            .filter(|&k| k != 0);
        out
    }

    fn push_startup(&mut self, elements: &[PropertyEntry]) {
        for e in elements {
            match key_of(&e.value) {
                Some(k) => self.startup.push(k),
                None => self.skipped += 1,
            }
        }
    }

    /// Merges an instance script table (§4.3): each script replaces any own definition of
    /// its key; its per-script properties become the key's own set. The includes are not
    /// touched. Roots are converted with [`TreeNode::to_node`].
    pub fn merge<D: PropertyDefaults + ?Sized>(
        &mut self,
        ist: &InstanceScriptTable,
        fields: &NodeFieldLabels,
        defaults: &D,
    ) {
        for s in &ist.scripts {
            let def = OwnDefinition {
                key: s.key,
                roots: s
                    .roots
                    .iter()
                    .map(|r| r.to_node(fields, defaults))
                    .collect(),
                properties: s.properties.clone(),
            };
            self.own.retain(|d| d.key != s.key);
            self.own.push(def);
        }
    }

    /// The keys started on activation, in order (§4.2): `Startup` (5), then
    /// `Entity_PlayerPhysicsScript` when non-zero, then the start-up list; a key already in
    /// the list is not started twice.
    #[must_use]
    pub fn startup_keys(&self) -> Vec<u32> {
        let mut seen = HashSet::new();
        std::iter::once(STARTUP_KEY)
            .chain(self.player_physics_script)
            .chain(self.startup.iter().copied())
            .filter(|k| seen.insert(*k))
            .collect()
    }

    /// The own definition of `key`.
    #[must_use]
    pub fn own_definition(&self, key: u32) -> Option<&OwnDefinition> {
        self.own.iter().find(|d| d.key == key)
    }

    /// Looks `key` up through the table set (§4.4): the own definition first (kind-2 roots,
    /// its per-script properties merged first); unless the merged set holds
    /// `Script_CombinationType` = 1 (`Overriding`), then every included table's closure in
    /// include order (`script-runtime.md` §12.2: pre-order, each table once, a table whose
    /// definition is `Overriding` not descended into). `table` returns a decoded 0x07 table.
    ///
    /// Merging of per-script properties: an entry already in the merged set is kept (the
    /// first definition's settings win; the spec does not state an order, so this is
    /// Tessera's choice).
    #[must_use]
    pub fn lookup<'t>(
        &'t self,
        labels: &InstanceScriptLabels,
        key: u32,
        table: &dyn Fn(Did) -> Option<&'t ScriptTable>,
    ) -> KeyLookup<'t> {
        let mut out = KeyLookup::default();
        if let Some(def) = self.own_definition(key) {
            if let Some(p) = &def.properties {
                out.merge(p);
            }
            out.kind2.extend(def.roots.iter());
            if key_field(&out.properties, labels.combination_type) == Some(COMBINATION_OVERRIDING) {
                return out;
            }
        }
        let mut visited: HashSet<Did> = HashSet::new();
        for &inc in &self.includes {
            out.walk(labels, key, inc, table, &mut visited, 0);
        }
        out
    }
}

/// The roots and merged per-script properties a key lookup finds (§4.4).
#[derive(Debug, Clone, Default)]
pub struct KeyLookup<'t> {
    /// Roots of kind-1 tables.
    pub kind1: Vec<&'t Node>,
    /// Roots of kind-2 tables, the instance definition's first.
    pub kind2: Vec<&'t Node>,
    /// Roots of kind-4 tables.
    pub kind4: Vec<&'t Node>,
    /// The merged per-script property set.
    pub properties: PropertySet,
    /// Tables the walk reached, in order.
    pub tables: Vec<Did>,
    /// Tables named but not found.
    pub missing: Vec<Did>,
}

impl<'t> KeyLookup<'t> {
    /// The roots in start order: kind 1, then kind 2, then kind 4 (`script-runtime.md`
    /// §12.1 step 6).
    #[must_use]
    pub fn roots(&self) -> Vec<&'t Node> {
        self.kind1
            .iter()
            .chain(&self.kind2)
            .chain(&self.kind4)
            .copied()
            .collect()
    }

    fn merge(&mut self, p: &PropertySet) {
        for e in &p.entries {
            if self.properties.get(e.pid).is_none() {
                self.properties.entries.push(e.clone());
            }
        }
        self.properties.size.count =
            u32::try_from(self.properties.entries.len()).unwrap_or(u32::MAX);
    }

    fn walk(
        &mut self,
        labels: &InstanceScriptLabels,
        key: u32,
        did: Did,
        table: &dyn Fn(Did) -> Option<&'t ScriptTable>,
        visited: &mut HashSet<Did>,
        depth: usize,
    ) {
        if depth >= MAX_TREE_DEPTH || !visited.insert(did) {
            return;
        }
        let Some(t) = table(did) else {
            self.missing.push(did);
            return;
        };
        self.tables.push(did);
        let mut overriding = false;
        if let Some(s) = t.scripts.iter().find(|s| s.key == key) {
            let list = match t.kind {
                1 => &mut self.kind1,
                4 => &mut self.kind4,
                _ => &mut self.kind2,
            };
            list.extend(s.roots.iter());
            if let Some((_, p)) = t.script_properties.iter().find(|(k, _)| *k == key) {
                overriding = key_field(p, labels.combination_type) == Some(COMBINATION_OVERRIDING);
                self.merge(p);
            }
        }
        if !overriding {
            for &inc in &t.includes {
                self.walk(labels, key, inc, table, visited, depth + 1);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Link targets (§3.5)
// ---------------------------------------------------------------------------------------------

/// **Decision Q4** (§3.5): the entities a Link target named `name` of `owner`'s instance
/// script resolves to: the `end_id_1` of every Script link (type 12) of the owner's
/// landblock-info record whose `name` equals `name` byte for byte and whose `end_id_2` is
/// the owner, in link order. (`end_id_1` is [`Link::to_iid`], `end_id_2` is
/// [`Link::from_iid`].)
#[must_use]
pub fn link_targets(links: &[Link], owner: InstanceId, name: &str) -> Vec<InstanceId> {
    links
        .iter()
        .filter(|l| {
            l.link_type == SCRIPT_LINK_TYPE
                && l.from_iid == owner
                && l.name.as_bytes() == name.as_bytes()
        })
        .map(|l| l.to_iid)
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Member overrides (§5) and instance switches (§8)
// ---------------------------------------------------------------------------------------------

/// A member-override property name split into its parts (§5.1): *Owner*`@`*Holder*, then
/// `.` or `::` and the member. The parts follow a naming convention only; the property is
/// an ordinary property (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemberOverride<'a> {
    /// The owner (a script module), before `@`.
    pub owner: &'a str,
    /// The holder (the kind of object), up to the first `.` or `::` after `@`.
    pub holder: &'a str,
    /// The member, after the separator.
    pub member: &'a str,
}

impl<'a> MemberOverride<'a> {
    /// Splits `name`; `None` when it contains no `@`, or no separator after it.
    #[must_use]
    pub fn parse(name: &'a str) -> Option<Self> {
        let (owner, rest) = name.split_once('@')?;
        let dot = rest.find('.');
        let colons = rest.find("::");
        let (at, len) = match (dot, colons) {
            (Some(d), Some(c)) if c < d => (c, 2),
            (Some(d), _) => (d, 1),
            (None, Some(c)) => (c, 2),
            (None, None) => return None,
        };
        let holder = rest.get(..at)?;
        let member = rest.get(at + len..)?;
        Some(Self {
            owner,
            holder,
            member,
        })
    }

    /// The value type the member's prefix suggests (`m_b` bool, `m_c` colour, `m_f` float,
    /// `m_e` enum, `m_i` integer, `m_w` waveform, `m_iid` instance ID), from the last
    /// segment of the member; `None` without a known prefix.
    #[must_use]
    pub fn type_hint(&self) -> Option<PropertyType> {
        let last = self.member.rsplit(['.', ':']).next().unwrap_or(self.member);
        let rest = last.strip_prefix("m_")?;
        let upper_after = |p: &str| {
            rest.strip_prefix(p)
                .is_some_and(|r| r.chars().next().is_some_and(char::is_uppercase))
        };
        Some(if upper_after("iid") {
            PropertyType::InstanceId
        } else if upper_after("b") {
            PropertyType::Bool
        } else if upper_after("c") {
            PropertyType::Colour
        } else if upper_after("f") {
            PropertyType::Float
        } else if upper_after("e") {
            PropertyType::Enum
        } else if upper_after("i") {
            PropertyType::Int
        } else if upper_after("w") {
            PropertyType::Waveform
        } else {
            return None;
        })
    }
}

/// The instance switch properties of §8.1, quoted as stored (interoperability names).
pub const INSTANCE_SWITCH_LABELS: [&str; 8] = [
    "vfx_Fire_Color",
    "vfx_Fx_Switch",
    "vfx_Color",
    "vfx_Water_Color",
    "Script_LightSource_Enable",
    "Script_ViewMountMeshFX",
    "ScriptSystem_LightsActivated",
    "ScriptSystem_TimeofDay_Activation",
];

#[cfg(test)]
mod tests;
