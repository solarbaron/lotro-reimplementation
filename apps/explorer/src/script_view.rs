// SPDX-License-Identifier: GPL-3.0-or-later
//! The text display of script-graph nodes (`script-table.md` §4–§7), shared by the record
//! view of script tables (0x07) and scriptlets (0x0C) and by the per-instance script view
//! of the landblock card (`instance-scripts.md` §9.6 item 1).
//!
//! One line per node, indented by depth: the kind, the editor ID, a summary of the kind's
//! data with property names and script keys labelled, the driven pairs; then one line per
//! non-empty port (its type and payload) with the children below it, and one per target.
//! Output is bounded by the caller's line budget; a cut is marked with a final line.

use tessera_formats::PropertyValue;
use tessera_formats::script_table::{Node, NodeData, PortPayload, Target, TargetData};

/// Most lines of one node display (a whole record or one entity's view).
pub const MAX_NODE_LINES: usize = 20_000;

/// Longest summary of a value or payload, in characters.
const MAX_SUMMARY: usize = 120;

/// How names are shown: property IDs and script keys.
pub struct NodeNames<'a> {
    /// A property ID's name.
    pub pid: &'a dyn Fn(u32) -> String,
    /// A script key with its label.
    pub key: &'a dyn Fn(u32) -> String,
}

impl std::fmt::Debug for NodeNames<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeNames").finish_non_exhaustive()
    }
}

fn cut(mut s: String) -> String {
    if s.chars().count() > MAX_SUMMARY {
        s = s.chars().take(MAX_SUMMARY).collect();
        s.push('…');
    }
    s
}

fn value_text(v: &PropertyValue) -> String {
    match v {
        PropertyValue::DidRef(d) => d.to_string(),
        PropertyValue::Float(f) => format!("{f}"),
        PropertyValue::Bool(b) => b.to_string(),
        PropertyValue::Enum(k) => k.to_string(),
        PropertyValue::Int(i) => i.to_string(),
        other => cut(format!("{other:?}")),
    }
}

/// A one-line summary of a node's kind data.
#[must_use]
pub fn data_text(d: &NodeData, names: &NodeNames<'_>) -> String {
    match d {
        NodeData::None => String::new(),
        NodeData::Animation(a) => format!("clip {} layer {}", a.asset, a.layer),
        NodeData::Switch(sw) => format!(
            "on {}{}",
            (names.pid)(sw.property),
            if sw.persistent { " (persistent)" } else { "" }
        ),
        NodeData::Sequence {
            loop_forever,
            loop_count,
        } => format!("loop forever {loop_forever}, count {loop_count}"),
        NodeData::Random { no_repeats, .. } => format!("no repeats {no_repeats}"),
        NodeData::Scriptlet { file, parameters } => {
            format!("{file} ({} parameters)", parameters.entries.len())
        }
        NodeData::ScriptletParameter { name } => format!("parameter {name:?}"),
        NodeData::ScalarBlend { property, .. } => format!("by {}", (names.pid)(*property)),
        NodeData::Run(r) => format!(
            "{} {}{}",
            if r.stop { "stop" } else { "run" },
            (names.key)(r.script),
            if r.persistent { " (persistent)" } else { "" }
        ),
        NodeData::WhenRunning { scripts, .. } => format!(
            "while running {}",
            scripts
                .iter()
                .map(|k| (names.key)(*k))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        NodeData::SetProperty {
            property,
            value,
            server_only,
        } => format!(
            "{} = {}{}",
            (names.pid)(*property),
            value
                .as_deref()
                .map_or_else(|| "null".to_owned(), |e| value_text(&e.value)),
            if *server_only { " (server only)" } else { "" }
        ),
        NodeData::Timer(t) => format!("duration {} s", t.duration),
        NodeData::Label { name } => format!("label {name:?}"),
        NodeData::Goto { name } => format!("goto {name:?}"),
        NodeData::StoryHook { hook_type } => format!("type {hook_type}"),
        NodeData::Appearance {
            appearance, key, ..
        } => format!("{appearance} key {key:#x}"),
        NodeData::MeshDeformer { clip, time, .. } => format!("clip {clip} at {time}"),
        NodeData::Sound(s) => format!("sound {} channel {}", s.sound, s.channel),
        NodeData::SoundEnvironment { name, priority } => {
            format!("environment {name} priority {priority}")
        }
        NodeData::SliderMeshDeformer { definitions, .. } => format!("sliders {definitions}"),
        NodeData::Light(l) => format!(
            "range {} falloff {} diffuse {:?} intensity {:?} key {} shadow {}",
            l.range, l.falloff, l.diffuse, l.diffuse_intensity.values, l.key_light, l.shadow_caster
        ),
        NodeData::MeshFx(m) => format!(
            "entity {} at location {} offset {:?} scale {:?} behaviour {}",
            m.entity,
            m.holding_location,
            m.bone_offset,
            m.scale,
            (names.key)(m.behavior)
        ),
        NodeData::Particle(p) => format!(
            "particle {} fork {} at location {} offset {:?}",
            p.particle_id, p.fork_particle_id, p.holding_location, p.bone_offset
        ),
        NodeData::While(w) => format!(
            "while {} {} [{}]{}",
            (names.pid)(w.property),
            if w.inverse { "not in" } else { "in" },
            w.values
                .iter()
                .map(|v| v
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |e| value_text(&e.value)))
                .chain(w.name_pids().map(|p| (names.pid)(p)))
                .collect::<Vec<_>>()
                .join(", "),
            if w.names.is_empty() { "" } else { " (names)" }
        ),
        NodeData::GameCallback(g) => format!(
            "package {} default {} ({} call properties)",
            g.package,
            g.default_value
                .as_ref()
                .map_or_else(|| "null".to_owned(), |e| value_text(&e.value)),
            g.call_properties.entries.len()
        ),
        NodeData::Opaque { len, .. } => format!("({len} bytes of data)"),
        NodeData::Fields(set) => cut(set
            .entries
            .iter()
            .map(|e| format!("{} = {}", (names.pid)(e.pid), value_text(&e.value)))
            .collect::<Vec<_>>()
            .join(", ")),
        other => cut(format!("{other:?}")),
    }
}

fn payload_text(p: &PortPayload, names: &NodeNames<'_>) -> String {
    match p {
        PortPayload::None => String::new(),
        PortPayload::U32(v) => v.to_string(),
        PortPayload::F32(v) => format!("{v}"),
        PortPayload::SwitchCase(c) => format!(
            "case {} op {}",
            c.values
                .iter()
                .map(|v| v
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |e| value_text(&e.value)))
                .collect::<Vec<_>>()
                .join(" | "),
            c.operator
        ),
        PortPayload::WhenTest(t) => format!(
            "test op {} {}",
            t.operator,
            if t.rhs_property != 0 {
                #[allow(clippy::cast_sign_loss)]
                (names.pid)(t.rhs_property as u32)
            } else {
                t.constant
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |e| value_text(&e.value))
            }
        ),
        PortPayload::Name(n) => format!("{n:?}"),
        other => cut(format!("{other:?}")),
    }
}

fn target_lines(t: &Target, depth: usize, out: &mut Vec<String>, max: usize) {
    if out.len() >= max {
        return;
    }
    let data = match &t.data {
        TargetData::Name(n) => format!(" {n:?}"),
        TargetData::None => String::new(),
        other => format!(" {}", cut(format!("{other:?}"))),
    };
    out.push(format!(
        "{}target type {} port {}{data}",
        "  ".repeat(depth),
        t.target_type,
        t.port_type.0
    ));
    for s in &t.sub_targets {
        target_lines(s, depth + 1, out, max);
    }
}

/// Appends the lines of `n` and everything below it at `depth`, while `out` holds fewer
/// than `max` lines.
pub fn node_lines(
    n: &Node,
    names: &NodeNames<'_>,
    depth: usize,
    out: &mut Vec<String>,
    max: usize,
) {
    if out.len() >= max {
        return;
    }
    let pad = "  ".repeat(depth);
    let kind = n
        .kind
        .name()
        .map_or_else(|| format!("kind {}", n.kind.0), str::to_owned);
    let mut line = format!("{pad}{kind} #{}", n.base.node_id);
    let data = data_text(&n.data, names);
    if !data.is_empty() {
        line.push_str(": ");
        line.push_str(&data);
    }
    if !n.base.driven.is_empty() {
        let pairs: Vec<String> = n
            .base
            .driven
            .iter()
            .map(|&(f, s)| format!("{} ← {}", (names.pid)(f), (names.pid)(s)))
            .collect();
        line.push_str(&format!(" [driven: {}]", cut(pairs.join(", "))));
    }
    if n.base.probability < 1.0 {
        line.push_str(&format!(" [probability {}]", n.base.probability));
    }
    out.push(line);
    for p in &n.ports {
        if out.len() >= max {
            return;
        }
        if p.port_type.0 == 0 && p.children.is_empty() {
            continue;
        }
        let payload = payload_text(&p.payload, names);
        out.push(format!(
            "{pad}  port {}{}{}",
            p.port_type.0,
            if payload.is_empty() { "" } else { " " },
            payload
        ));
        for c in &p.children {
            node_lines(c, names, depth + 2, out, max);
        }
    }
    for t in &n.targets {
        target_lines(t, depth + 1, out, max);
    }
}

/// The lines of every root of one script, with a cut marker when the budget ran out.
pub fn roots_lines<'n>(
    roots: impl IntoIterator<Item = &'n Node>,
    names: &NodeNames<'_>,
    depth: usize,
    out: &mut Vec<String>,
    max: usize,
) {
    for r in roots {
        node_lines(r, names, depth, out, max);
    }
    if out.len() >= max {
        out.push(format!("{}… cut at {max} lines", "  ".repeat(depth)));
    }
}

/// The DIDs written in `line` as `0x` and eight hex digits (the display form of
/// [`tessera_formats::Did`]), in order.
#[must_use]
pub fn dids_in_line(line: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("0x") {
        rest = rest.get(at + 2..).unwrap_or("");
        let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        if hex.len() == 8
            && let Ok(d) = u32::from_str_radix(&hex, 16)
            && d != 0
        {
            out.push(d);
        }
    }
    out
}

/// Rows of a record view: (caption, text) pairs.
pub type TextRows = Vec<(String, String)>;

/// The record view of a script table (0x07) or scriptlet (0x0C): summary lines and one
/// row per display line (script label, line).
///
/// # Errors
///
/// When the record does not decode with `schema`.
pub fn record_rows<S: tessera_formats::PropertySchema + ?Sized>(
    did: u32,
    bytes: &[u8],
    schema: &S,
    names: &NodeNames<'_>,
) -> Result<(TextRows, TextRows), tessera_formats::FormatError> {
    use tessera_formats::{Did, ScriptTable, Scriptlet};
    let mut summary = Vec::new();
    let mut rows = Vec::new();
    if did >> 24 == 0x07 {
        let t = ScriptTable::parse_expecting(bytes, Did(did), schema)?;
        summary.push(("kind".to_owned(), t.kind.to_string()));
        summary.push(("scripts".to_owned(), t.scripts.len().to_string()));
        summary.push((
            "includes".to_owned(),
            t.includes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ));
        let mut lines = Vec::new();
        for sc in &t.scripts {
            if lines.len() >= MAX_NODE_LINES {
                break;
            }
            let start = lines.len();
            let props = t
                .script_properties
                .iter()
                .find(|(k, _)| *k == sc.key)
                .map(|(_, p)| p.entries.len())
                .unwrap_or(0);
            lines.push(format!(
                "script {} ({} roots, {props} per-script properties)",
                (names.key)(sc.key),
                sc.roots.len()
            ));
            roots_lines(&sc.roots, names, 1, &mut lines, MAX_NODE_LINES);
            let label = (names.key)(sc.key);
            rows.extend(lines[start..].iter().map(|l| (label.clone(), l.clone())));
        }
    } else {
        let s = Scriptlet::parse_expecting(bytes, Did(did), schema)?;
        summary.push(("roots".to_owned(), s.roots.len().to_string()));
        summary.push(("dependencies".to_owned(), s.dependencies.len().to_string()));
        let mut lines = Vec::new();
        roots_lines(&s.roots, names, 0, &mut lines, MAX_NODE_LINES);
        rows.extend(lines.into_iter().map(|l| ("roots".to_owned(), l)));
    }
    Ok((summary, rows))
}

#[cfg(test)]
mod tests {
    use tessera_formats::script_table::{BaseBlock, NodeKind, Port, PortType, RunNode, SwitchNode};

    use super::*;

    fn node(kind: NodeKind, data: NodeData, ports: Vec<Port>) -> Node {
        Node {
            kind,
            data,
            base: BaseBlock {
                probability: 1.0,
                node_id: 7,
                ..BaseBlock::default()
            },
            particle_flag: None,
            ports,
            targets: Vec::new(),
        }
    }

    #[test]
    fn lines_are_indented_and_bounded() {
        let run = node(
            NodeKind::RUN,
            NodeData::Run(Box::new(RunNode {
                version: 1,
                parameters: Vec::new(),
                script: 42,
                persistent: true,
                unbind_on_death: false,
                stop: false,
                expose_as_script_link: false,
                queuing_type: 0,
            })),
            Vec::new(),
        );
        let sw = node(
            NodeKind::SWITCH,
            NodeData::Switch(SwitchNode {
                property: 9,
                persistent: true,
                autonomous: false,
                synchronized: false,
            }),
            vec![Port {
                port_type: PortType::DEFAULT,
                payload: PortPayload::None,
                children: vec![run],
            }],
        );
        let pid = |p: u32| format!("P{p}");
        let key = |k: u32| format!("K{k}");
        let names = NodeNames {
            pid: &pid,
            key: &key,
        };
        let mut out = Vec::new();
        roots_lines([&sw], &names, 0, &mut out, 100);
        assert_eq!(out[0], "switch #7: on P9 (persistent)");
        assert_eq!(out[1], "  port 6");
        assert_eq!(out[2], "    run #7: run K42 (persistent)");
        let mut short = Vec::new();
        roots_lines([&sw], &names, 0, &mut short, 1);
        assert_eq!(short.len(), 2);
        assert!(short[1].contains("cut at 1 lines"));
        assert_eq!(
            dids_in_line("clip 0x05000001 layer 10 and 0x0700ABCD, 0x123"),
            vec![0x0500_0001, 0x0700_ABCD]
        );
    }
}
