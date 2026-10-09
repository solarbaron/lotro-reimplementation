// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of instance script tables (`instance-scripts.md` §3–§5, §11.1).

use std::collections::{BTreeMap, HashMap};

use super::*;
use crate::gameplay::common::build::{array, entry, set, structure};
use crate::script_table::Script;

/// The install's PIDs of §3.2 and §11.1, other labels numbered from 0x5000.
fn names() -> HashMap<String, u32> {
    let mut m: HashMap<String, u32> = InstanceScriptLabels::LABELS
        .iter()
        .chain(NodeFieldLabels::LABELS)
        .enumerate()
        .map(|(i, l)| ((*l).to_owned(), 0x5000 + u32::try_from(i).unwrap()))
        .collect();
    for (l, p) in [
        ("Entity_InstanceScriptTable", 0x430),
        ("ScriptTable_Scripts", 0x7E),
        ("ScriptTable_StartupScriptNames", 0x301),
        ("ScriptTable_ScriptEntry", 0x438),
        ("ScriptTable_ScriptID", 0x25B),
        ("ScriptTable_ScriptProperties", 0xAF7),
        ("ScriptTable_ScriptRoots", 0x2AB),
        ("ScriptTable_Node", 0x356),
        ("ScriptTable_NodeType", 0x56A),
        ("ScriptTable_NodeProperties", 0x437),
        ("ScriptTable_NodeOutputs", 0x5B9),
        ("ScriptTable_NodePort", 0x17E),
        ("ScriptTable_PortType", 0x2AA),
        ("ScriptTable_PortProperties", 0x300),
        ("ScriptTable_Nodes", 0x5B7),
        ("ScriptTable_NodeTargets", 0x5B8),
        ("ScriptTable_TargetPort", 0x436),
        ("ScriptTable_NodeTargetEx", 0xD2),
        ("ScriptTable_TargetType", 0x2D),
        ("LinkTarget_Name", 0x449),
        ("RunScript_ID", 0x26E),
        ("RunScript_Persistent", 0x189),
    ] {
        m.insert(l.to_owned(), p);
    }
    m
}

fn schema(pid: u32) -> Option<PropertyType> {
    Some(match pid {
        0x430 | 0x438 | 0x356 | 0x436 | 0xD2 | 0x17E | 0xAF7 => PropertyType::Struct,
        0x7E | 0x2AB | 0x5B8 | 0x437 | 0x300 | 0x301 | 0x5B9 | 0x5B7 => PropertyType::Array,
        0x56A | 0x2AA | 0x2D | 0x26E | 0x25B => PropertyType::Enum,
        0x449 => PropertyType::String,
        0x189 => PropertyType::Bool,
        _ => return None,
    })
}

/// The 181 bytes of §11.1.
fn example_bytes() -> Vec<u8> {
    let hex = "30 04 00 00 30 04 00 00 00 01 7E 00 00 00 7E 00 00 00 01 00 00 00 38 04 00 00 00 02 \
               AB 02 00 00 AB 02 00 00 01 00 00 00 56 03 00 00 00 03 6A 05 00 00 6A 05 00 00 05 00 00 00 \
               B8 05 00 00 B8 05 00 00 01 00 00 00 36 04 00 00 00 03 AA 02 00 00 AA 02 00 00 06 00 00 00 \
               D2 00 00 00 D2 00 00 00 00 02 2D 00 00 00 2D 00 00 00 07 00 00 00 \
               37 04 00 00 37 04 00 00 01 00 00 00 49 04 00 00 05 4C 69 6E 6B 41 \
               00 03 00 00 00 03 00 00 00 00 00 00 37 04 00 00 37 04 00 00 02 00 00 00 \
               6E 02 00 00 02 00 00 10 89 01 00 00 00 5B 02 00 00 5B 02 00 00 01 00 00 10";
    hex.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

fn labels() -> (InstanceScriptLabels, NodeFieldLabels) {
    let n = names();
    (
        InstanceScriptLabels::resolve(&n),
        NodeFieldLabels::resolve(&n),
    )
}

#[test]
fn worked_example_script_link_ist() {
    let bytes = example_bytes();
    assert_eq!(bytes.len(), 181);
    let mut payload = vec![0u8, 1];
    payload.extend_from_slice(&bytes);
    let set = PropertySet::parse(&payload, &schema).unwrap();
    let (l, f) = labels();
    let ist = InstanceScriptTable::read(&l, &set).unwrap();
    assert_eq!(ist.scripts.len(), 1);
    let s = &ist.scripts[0];
    assert_eq!(s.key, 0x1000_0001);
    assert_eq!(s.roots.len(), 1);
    let root = &s.roots[0];
    assert_eq!(root.kind, NodeKind::RUN);
    assert_eq!(root.targets.len(), 1);
    assert_eq!(root.targets[0].target_type, Some(7));
    assert_eq!(root.targets[0].port_type, 6);
    assert_eq!(root.targets[0].link_name.as_deref(), Some("LinkA"));
    assert_eq!(ist.link_names(), vec!["LinkA"]);
    let node = root.to_node(&f, &BTreeMap::new());
    match &node.data {
        NodeData::Run(r) => {
            assert_eq!(r.script, 0x1000_0002);
            assert!(!r.persistent);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&node.targets[0].data, TargetData::Name(n) if n == "LinkA"));

    // Link resolution (Decision Q4): Script links named LinkA that start at the owner.
    let owner = InstanceId(0x0101_0000_0000_0001);
    let b = InstanceId(0x0101_0000_0000_0002);
    let c = InstanceId(0x0101_0000_0000_0003);
    let link = |name: &str, ty, from, to| Link {
        name: name.into(),
        link_type: ty,
        from_iid: from,
        to_iid: to,
        ..Link::default()
    };
    let links = vec![
        link("LinkA", 12, owner, b),
        link("LinkA", 8, owner, c),
        link("linka", 12, owner, c),
        link("LinkA", 12, b, owner),
        link("LinkA", 12, owner, c),
    ];
    assert_eq!(link_targets(&links, owner, "LinkA"), vec![b, c]);
    assert!(link_targets(&links, owner, "LinkB").is_empty());

    // The table set: the instance root first among kind-2 roots, then the class table's.
    let mut ts = EntityTableSet::default();
    ts.includes.push(Did(0x0700_0001));
    ts.merge(&ist, &f, &BTreeMap::new());
    let class_root = node.clone();
    let class = ScriptTable {
        did: Did(0x0700_0001),
        kind: 2,
        scripts: vec![Script {
            key: 0x1000_0001,
            roots: vec![class_root],
        }],
        ..ScriptTable::default()
    };
    let fetch = |d: Did| (d == class.did).then_some(&class);
    let found = ts.lookup(&l, 0x1000_0001, &fetch);
    assert_eq!(found.kind2.len(), 2);
    assert!(std::ptr::eq(found.kind2[0], &ts.own[0].roots[0]));
    assert_eq!(found.tables, vec![Did(0x0700_0001)]);

    // `Script_CombinationType` = 1 in the entry's properties: the instance root only.
    let mut ist2 = ist.clone();
    ist2.scripts[0].properties = Some(crate::gameplay::common::build::set(vec![entry(
        l.combination_type.id().unwrap(),
        PropertyValue::Enum(1),
    )]));
    let mut ts2 = EntityTableSet::default();
    ts2.includes.push(Did(0x0700_0001));
    ts2.merge(&ist2, &f, &BTreeMap::new());
    let found = ts2.lookup(&l, 0x1000_0001, &fetch);
    assert_eq!(found.kind2.len(), 1);
    assert!(found.tables.is_empty());
}

fn node_struct(l: &InstanceScriptLabels, kind: u32, props: Vec<PropertyEntry>) -> PropertyValue {
    structure(vec![
        entry(l.node_type.id().unwrap(), PropertyValue::Enum(kind)),
        entry(l.node_properties.id().unwrap(), array(props)),
    ])
}

#[test]
fn acceptance_rules() {
    let (l, f) = labels();
    let id = |p: Pid| p.id().unwrap();
    let script = |key: PropertyValue, roots: Vec<PropertyValue>| {
        entry(
            id(l.entry),
            structure(vec![
                entry(id(l.script_id), key),
                entry(
                    id(l.script_roots),
                    array(roots.into_iter().map(|r| entry(id(l.node), r)).collect()),
                ),
            ]),
        )
    };
    let run = |k: u32| node_struct(&l, 5, vec![entry(id(f.run_id), PropertyValue::Enum(k))]);
    let value = structure(vec![
        entry(
            id(l.scripts),
            array(vec![
                // A key of the wrong type is skipped.
                script(PropertyValue::Int(3), vec![run(1)]),
                // Unregistered kind 8 and a node without properties fail; the run survives.
                script(
                    PropertyValue::Enum(0x10),
                    vec![
                        node_struct(&l, 8, vec![]),
                        structure(vec![entry(id(l.node_type), PropertyValue::Enum(5))]),
                        run(2),
                    ],
                ),
                // A repeated key keeps the last definition.
                script(PropertyValue::PropertyId(0x10), vec![run(3)]),
                entry(id(l.entry), PropertyValue::Int(0)),
            ]),
        ),
        entry(
            id(l.startup_names),
            array(vec![
                entry(id(l.script_id), PropertyValue::Enum(0x20)),
                entry(id(l.script_id), PropertyValue::Int(9)),
            ]),
        ),
    ]);
    let ist = InstanceScriptTable::parse(&l, &value).unwrap();
    assert_eq!(ist.skipped_entries, 2);
    assert_eq!(ist.replaced, 1);
    assert_eq!(ist.scripts.len(), 1);
    assert_eq!(ist.scripts[0].failed_roots, 0);
    assert_eq!(ist.scripts[0].roots.len(), 1);
    assert_eq!(ist.startup, vec![0x20]);
    assert_eq!(ist.skipped_startup, 1);
    assert!(InstanceScriptTable::parse(&l, &PropertyValue::Int(1)).is_none());

    // Decision Q3: an absent RunScript_Persistent takes the dictionary default.
    let defaults: BTreeMap<u32, PropertyValue> =
        [(id(f.run_persistent), PropertyValue::Bool(true))]
            .into_iter()
            .collect();
    match &ist.scripts[0].roots[0].to_node(&f, &defaults).data {
        NodeData::Run(r) => assert!(r.persistent && r.script == 3),
        other => panic!("{other:?}"),
    }
}

#[test]
fn table_set_gathering_and_startup() {
    let (l, _) = labels();
    let id = |p: Pid| p.id().unwrap();
    let class = structure(vec![
        entry(
            id(l.db_script_table),
            PropertyValue::DidRef(Did(0x0700_0010)),
        ),
        entry(
            id(l.startup_names),
            array(vec![entry(id(l.script_id), PropertyValue::Enum(0x30))]),
        ),
    ]);
    let instance = set(vec![entry(
        id(l.instance_table),
        structure(vec![entry(
            id(l.startup_names),
            array(vec![
                entry(id(l.script_id), PropertyValue::Enum(0x31)),
                entry(id(l.script_id), PropertyValue::Enum(5)),
            ]),
        )]),
    )]);
    let object = set(vec![
        entry(id(l.class_table), class),
        entry(
            id(l.additional_tables),
            array(vec![
                entry(1, PropertyValue::DidRef(Did(0x0700_0011))),
                entry(1, PropertyValue::DidRef(Did(0))),
                entry(1, PropertyValue::DidRef(Did(0x0700_0010))),
                entry(1, PropertyValue::Int(4)),
            ]),
        ),
        entry(id(l.player_physics_script), PropertyValue::Enum(0x40)),
    ]);
    let layers: [&PropertySet; 2] = [&instance, &object];
    let ts = EntityTableSet::gather(&l, &layers);
    assert_eq!(ts.includes, vec![Did(0x0700_0010), Did(0x0700_0011)]);
    assert_eq!(ts.startup, vec![0x30, 0x31, 5]);
    assert_eq!(ts.skipped, 1);
    assert_eq!(ts.startup_keys(), vec![5, 0x40, 0x30, 0x31]);
}

#[test]
fn member_override_names() {
    let m = MemberOverride::parse("Light.Script@Entity.Night.m_fLightRange").unwrap();
    assert_eq!(
        (m.owner, m.holder, m.member),
        ("Light.Script", "Entity", "Night.m_fLightRange")
    );
    assert_eq!(m.type_hint(), Some(PropertyType::Float));
    let b = MemberOverride::parse("Behavior@NPC::Vitals::DisplayGroundFX").unwrap();
    assert_eq!((b.holder, b.member), ("NPC", "Vitals::DisplayGroundFX"));
    assert_eq!(b.type_hint(), None);
    assert_eq!(
        MemberOverride::parse("State@Entity.m_iidOwner")
            .unwrap()
            .type_hint(),
        Some(PropertyType::InstanceId)
    );
    assert_eq!(
        MemberOverride::parse("Toggle.Script@Entity.m_bOpen")
            .unwrap()
            .type_hint(),
        Some(PropertyType::Bool)
    );
    assert!(MemberOverride::parse("Render_Draw").is_none());
    assert!(MemberOverride::parse("A@B").is_none());
}

/// IS12 (BIN2, §4.4.1): per-script property sets are merged first value wins, the instance
/// definition's set first, then the included tables in include order, each table before its
/// own includes; a merged `Script_CombinationType` of 1 stops the descent into includes
/// for properties only.
#[test]
fn is12_per_script_properties_merge_first_value_wins() {
    let (l, f) = labels();
    const KEY: u32 = 0x1000_0001;
    const MAX: u32 = 0x6000;
    let ct = l.combination_type.id().unwrap();
    let props = |entries: Vec<(u32, PropertyValue)>| {
        set(entries.into_iter().map(|(p, v)| entry(p, v)).collect())
    };
    let table = |did: u32, includes: Vec<u32>, p: Option<PropertySet>| ScriptTable {
        did: Did(did),
        kind: 2,
        includes: includes.into_iter().map(Did).collect(),
        scripts: vec![Script {
            key: KEY,
            roots: Vec::new(),
        }],
        script_properties: p.map(|p| vec![(KEY, p)]).unwrap_or_default(),
        ..ScriptTable::default()
    };
    let max_of = |found: &KeyLookup<'_>| found.properties.get(MAX).cloned();
    let instance = |p: Option<PropertySet>| {
        let mut ts = EntityTableSet::default();
        ts.own.push(OwnDefinition {
            key: KEY,
            roots: Vec::new(),
            properties: p,
        });
        ts
    };
    let _ = &f;

    // Instance 3, included table 10: 3. Instance set empty: 10.
    let t10 = table(
        0x0700_0001,
        vec![],
        Some(props(vec![(MAX, PropertyValue::Int(10))])),
    );
    let fetch1 = |d: Did| (d == t10.did).then_some(&t10);
    let mut ts = instance(Some(props(vec![(MAX, PropertyValue::Int(3))])));
    ts.includes.push(t10.did);
    assert_eq!(
        max_of(&ts.lookup(&l, KEY, &fetch1)),
        Some(PropertyValue::Int(3))
    );
    let mut ts = instance(Some(props(vec![])));
    ts.includes.push(t10.did);
    assert_eq!(
        max_of(&ts.lookup(&l, KEY, &fetch1)),
        Some(PropertyValue::Int(10))
    );

    // Two included tables holding 4 and 7 in include order: 4.
    let t4 = table(
        0x0700_0004,
        vec![],
        Some(props(vec![(MAX, PropertyValue::Int(4))])),
    );
    let t7 = table(
        0x0700_0007,
        vec![],
        Some(props(vec![(MAX, PropertyValue::Int(7))])),
    );
    let tables = [&t4, &t7];
    let fetch2 = |d: Did| tables.iter().copied().find(|t| t.did == d);
    let mut ts = instance(None);
    ts.includes.extend([t4.did, t7.did]);
    assert_eq!(
        max_of(&ts.lookup(&l, KEY, &fetch2)),
        Some(PropertyValue::Int(4))
    );

    // A combination type of 1 merged from an earlier table stops the property descent into
    // a later table's includes, but not the root walk; a table including itself (and a null
    // include) cannot loop.
    let first = table(
        0x0700_0011,
        vec![0x0700_0011, 0],
        Some(props(vec![(ct, PropertyValue::Enum(1))])),
    );
    let second = table(0x0700_0012, vec![0x0700_0013], None);
    let inner = table(
        0x0700_0013,
        vec![],
        Some(props(vec![(MAX, PropertyValue::Int(9))])),
    );
    let tables = [&first, &second, &inner];
    let fetch3 = |d: Did| tables.iter().copied().find(|t| t.did == d);
    let mut ts = instance(None);
    ts.includes.extend([first.did, second.did]);
    let found = ts.lookup(&l, KEY, &fetch3);
    assert_eq!(
        max_of(&found),
        None,
        "descent into the second table's includes stopped"
    );
    assert_eq!(found.tables, vec![first.did, second.did, inner.did]);
}

/// §4.5.1 Decision: tree ports of type 1 or more are kept for any kind and any count; a
/// port of type 0 is dropped with its subtree.
#[test]
fn tree_ports_of_type_zero_are_dropped() {
    let (_, f) = labels();
    let child = TreeNode {
        kind: NodeKind::COLLECTION,
        fields: Vec::new(),
        driven: Vec::new(),
        ports: Vec::new(),
        targets: Vec::new(),
    };
    let port = |t: u32| TreePort {
        port_type: t,
        payload: Vec::new(),
        children: vec![child.clone()],
    };
    let n = TreeNode {
        kind: NodeKind::RUN,
        fields: Vec::new(),
        driven: Vec::new(),
        ports: vec![port(0), port(9), port(9), port(9), port(77), port(300)],
        targets: Vec::new(),
    };
    let node = n.to_node(&f, &BTreeMap::new());
    let types: Vec<u8> = node.ports.iter().map(|p| p.port_type.0).collect();
    assert_eq!(types, vec![9, 9, 9, 77]);
    assert!(node.ports.iter().all(|p| p.children.len() == 1));
}
