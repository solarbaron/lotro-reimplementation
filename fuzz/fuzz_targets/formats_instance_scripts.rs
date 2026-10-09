// SPDX-License-Identifier: GPL-3.0-or-later
//! Instance script tables (`tessera_formats::script_table::instance`,
//! `docs/specs/instance-scripts.md`) on arbitrary bytes.
//!
//! The input after byte 0 is an embedded property set decoded with a total synthetic schema
//! (type = PID mod 24); the labels of §3.2 get PIDs of the types the dictionary gives them
//! (tables, entries, nodes, ports and targets structs; scripts, roots, properties, outputs,
//! targets and child lists arrays; keys and kinds enums), so fuzzed trees reach the node
//! builder. Byte 0 picks the PID of the instance table: the set itself is also tried as the
//! table value.
//!
//! The table is parsed, merged into a table set gathered from the same set, every root is
//! converted to the node model, every key is looked up (with a synthetic included table
//! that defines the same keys), link targets are resolved against synthetic links, and the
//! input is read as a member-override name.
//!
//! Nothing may panic. A key is defined at most once among the own definitions, the
//! instance roots come first among the kind-2 roots of a lookup, and the start-up keys hold
//! no repeats and begin with `Startup`.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_instance_scripts -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::{BTreeMap, HashMap, HashSet};

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{InstanceId, Link};
use tessera_formats::script_table::instance::{
    EntityTableSet, InstanceScriptLabels, InstanceScriptTable, MemberOverride, NodeFieldLabels,
    STARTUP_KEY, link_targets,
};
use tessera_formats::script_table::{Script, ScriptTable};
use tessera_formats::{Did, PropertySet, PropertyType, PropertyValue};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn spec_type(label: &str) -> u32 {
    match label {
        "Entity_InstanceScriptTable"
        | "ScriptTable_ScriptEntry"
        | "ScriptTable_ScriptProperties"
        | "ScriptTable_Node"
        | "ScriptTable_NodePort"
        | "ScriptTable_TargetPort"
        | "ScriptTable_NodeTargetEx"
        | "Script_DrivenPropertyEntry"
        | "Entity_ClassScriptTable"
        | "Entity_ClassScriptTable_Override" => 11,
        "ScriptTable_Scripts"
        | "ScriptTable_StartupScriptNames"
        | "ScriptTable_ScriptRoots"
        | "ScriptTable_NodeProperties"
        | "ScriptTable_NodeOutputs"
        | "ScriptTable_PortProperties"
        | "ScriptTable_Nodes"
        | "ScriptTable_NodeTargets"
        | "ScriptTable_NodeTarget"
        | "Script_DrivenProperties"
        | "Entity_AdditionalScriptTables"
        | "Entity_AdditionalScriptTables_Override"
        | "SwitchScriptPort_Values" => 12,
        "LinkTarget_Name" => 1,
        "Script_DrivenProperty"
        | "Script_DriverProperty"
        | "SetPropertyScript_PropertyName"
        | "SwitchScript_Property" => 10,
        "ScriptTable_DBScriptTable"
        | "ScriptTable_ScriptChannels"
        | "ScriptletScript_File"
        | "AppearanceScript_Appearance" => 20,
        l if l.ends_with("Persistent")
            || l.ends_with("Stop")
            || l.ends_with("OnDeath")
            || l.ends_with("ScriptLink")
            || l.ends_with("ServerOnly")
            || l.ends_with("ParentIntensity") =>
        {
            21
        }
        l if l.ends_with("Duration")
            || l.ends_with("Modifier")
            || l.ends_with("Multiplier")
            || l.ends_with("Variance")
            || l.ends_with("Probability")
            || l.ends_with("Time")
            || l.ends_with("RampOut")
            || l.ends_with("Delay") =>
        {
            9
        }
        l if l.ends_with("Order") || l.ends_with("Weight") => 15,
        _ => 8,
    }
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let Ok(set) = PropertySet::parse(rest, &total) else {
        return;
    };
    let map: HashMap<String, u32> = InstanceScriptLabels::LABELS
        .iter()
        .chain(NodeFieldLabels::LABELS)
        .enumerate()
        .map(|(i, l)| {
            let i = u32::try_from(i).unwrap_or(0);
            ((*l).to_owned(), (i + 1) * 24 + spec_type(l))
        })
        .collect();
    let labels = InstanceScriptLabels::resolve(&map);
    let fields = NodeFieldLabels::resolve(&map);
    let defaults: BTreeMap<u32, PropertyValue> = fields
        .run_persistent
        .id()
        .map(|p| (p, PropertyValue::Bool(true)))
        .into_iter()
        .collect();

    let as_value = PropertyValue::Struct(tessera_formats::PropertyStruct {
        size: set.size,
        fields: set.entries.clone(),
    });
    let tables: Vec<InstanceScriptTable> = InstanceScriptTable::read(&labels, &set)
        .into_iter()
        .chain(InstanceScriptTable::parse(&labels, &as_value))
        .collect();
    let layers = [&set];
    let mut ts = EntityTableSet::gather(&labels, &layers);
    let startup = ts.startup_keys();
    assert_eq!(startup.first(), Some(&STARTUP_KEY));
    assert_eq!(startup.iter().collect::<HashSet<_>>().len(), startup.len());
    for ist in &tables {
        let keys: HashSet<u32> = ist.scripts.iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), ist.scripts.len());
        let _ = ist.link_names();
        ts.merge(ist, &fields, &defaults);
    }
    let own: HashSet<u32> = ts.own.iter().map(|d| d.key).collect();
    assert_eq!(own.len(), ts.own.len());

    // A synthetic included table that defines every own key once more.
    let included = ScriptTable {
        did: Did(0x0700_0001),
        kind: u32::from(*mode % 5),
        includes: vec![Did(0x0700_0001), Did(0x0700_0002)],
        scripts: ts
            .own
            .iter()
            .map(|d| Script {
                key: d.key,
                roots: d.roots.clone(),
            })
            .collect(),
        ..ScriptTable::default()
    };
    ts.includes.push(included.did);
    let fetch = |d: Did| (d == included.did).then_some(&included);
    for d in &ts.own {
        let found = ts.lookup(&labels, d.key, &fetch);
        assert!(found.kind2.len() >= d.roots.len());
        for (a, b) in found.kind2.iter().zip(&d.roots) {
            assert!(std::ptr::eq(*a, b));
        }
        let _ = found.roots();
    }

    let owner = InstanceId(u64::from(*mode) + 1);
    let links: Vec<Link> = rest
        .chunks(3)
        .take(16)
        .map(|c| Link {
            name: String::from_utf8_lossy(&c[..1]).into_owned(),
            link_type: u32::from(c.get(1).copied().unwrap_or(0) % 14),
            from_iid: InstanceId(u64::from(c.get(2).copied().unwrap_or(0) % 4)),
            to_iid: InstanceId(9),
            ..Link::default()
        })
        .collect();
    for name in ["\0", "a", ""] {
        assert!(link_targets(&links, owner, name).len() <= links.len());
    }
    let text = String::from_utf8_lossy(rest);
    if let Some(m) = MemberOverride::parse(&text) {
        let _ = m.type_hint();
        assert!(text.contains('@'));
    }
});
