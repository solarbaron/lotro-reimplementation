// SPDX-License-Identifier: GPL-3.0-or-later
//! Clip selection (`docs/specs/script-table.md` §8, §11.3) over a world of two tables, which
//! include each other, and four scriptlets, any of which may invoke the others or
//! themselves. The resolver runs every motion state and the all-branches mode on them. The
//! first two parts also serve as a game object's property set and its entity, for the
//! entity-chain collection (as they are, and behind a harness-written prologue); the
//! vocabulary PIDs are typed (under the `pid % 24` schema) as the resolver reads them, so
//! chain-derived tables, start-up scripts and scriptlet parameter overrides can apply.
//!
//! Byte 0 selects how the world is built. `% 4` = 0: the rest of the input is split on
//! 0xFF 0xFE into the six records and the two parts as they are (the decoders on raw
//! bytes). Otherwise the harness *writes* the records from the input with the layouts of
//! §2.1, §3 and §4–§6: tables with includes among the world's tables (and missing or
//! wrong-type DIDs), `Startup` and other keyed scripts, scriptlets, and node trees of every
//! kind the evaluator follows (Animation, Collection, Switch with cases, defaults and
//! transitions, When tests, ScalarBlend anchors, Random weights, Scriptlet invocations of
//! the world's scriptlets and of themselves with parameter sets and named bindings,
//! ScriptletParameter, Run, WhenRunning, SetProperty, both deformers, Sequence, Timer,
//! Label, OnStop, and the look nodes TextureCompositor, ModifyCompositorLayer,
//! ModifyCompositorColor and EditMaterial with occasional large waveforms, `appearance.md`
//! §3.5) with driven pairs on the vocabulary's animation and EditMaterial fields (and
//! scriptlet parameters on both), hostile floats and
//! zero or NaN probabilities; and two property sets naming the tables, start-up scripts and
//! entity of the chain. Now and then one byte of one record is corrupted, so undecodable
//! records are met too. Most such worlds decode, so the evaluator itself runs. Once the
//! input is used up the writer continues from a pseudo-random stream seeded with the whole
//! input, so that the short inputs libFuzzer starts with still give whole worlds.
//!
//! When bits 2–3 of byte 0 are both set, the resolver runs under tight limits (one table,
//! shallow includes, calls and nesting, a few dozen node visits and work units, three
//! candidates), so that every limit is reached and reported.
//!
//! Nothing may panic, every evaluation stays within its limits (bounded node visits and
//! candidates, cycles reported instead of followed), every candidate names a clip, each
//! issue is reported once, and held fractions stay in [0, 1].
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_clip_resolver -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;
use std::convert::Infallible;

use libfuzzer_sys::fuzz_target;
use tessera_formats::reader::write_vle;
use tessera_formats::script_table::resolve::{
    AnimationFieldPids, ClipResolution, ClipResolver, EditMaterialFieldPids, EntityScripts,
    MeshDeformerFieldPids, MotionState, ResolveLimits, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::setup::ENTITY_FLAGS_REQUIRED;
use tessera_formats::{
    BitfieldAny, Did, EntityDescriptor, EntityHeader, PropertySet, PropertyType, PropertyValue,
};

/// Each problem is reported once per resolution.
fn assert_issues_unique(res: &ClipResolution) {
    for (i, a) in res.issues.iter().enumerate() {
        assert!(!res.issues[..i].contains(a), "{a:?} reported twice");
    }
}

/// Every PID has a type, so PVs inside nodes decode.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

/// A PID that `total` types with `code` (`PropertyType::from_code`): 24 × `k` + `code`.
const fn typed(k: u32, code: u32) -> u32 {
    24 * k + code
}

// Type codes (`property-system.md` §4.1) of the values the harness writes.
const STRING: u32 = 1;
const TRI_STATE: u32 = 5;
const VECTOR3: u32 = 6;
const ENUM: u32 = 8;
const FLOAT: u32 = 9;
const STRUCT: u32 = 11;
const ARRAY: u32 = 12;
const BITFIELD64: u32 = 14;
const INT: u32 = 15;
const COLOUR: u32 = 16;
const INT64: u32 = 19;
const DID_REF: u32 = 20;
const BOOL: u32 = 21;
const BITFIELD_ANY: u32 = 22;

// Vocabulary PIDs, each typed as the resolver reads it: the class table is a struct
// (`as_struct`), the table DID inside it, `PhysObj` and the clip asset are DID references
// (`as_did`), the additional tables and start-up script names are arrays (`children`), the
// speed is a number, the displacement a vector; the motion-state properties get the types
// the resolver itself stores in them (enum style and element, float sliders and speed
// multiplier). Distinct `k`, so distinct PIDs.
const CLASS_SCRIPT_TABLE: u32 = typed(1, STRUCT);
const DB_SCRIPT_TABLE: u32 = typed(2, DID_REF);
const MOVEMENT_STYLE: u32 = typed(3, ENUM);
const SLIDER_PROPEL: u32 = typed(4, FLOAT);
const SLIDER_STRAFE: u32 = typed(5, FLOAT);
const SLIDER_TURN: u32 = typed(6, FLOAT);
const ENTITY_ELEMENT: u32 = typed(7, ENUM);
const SPEED_MULTIPLIER: u32 = typed(8, FLOAT);
const ADDITIONAL_SCRIPT_TABLES: u32 = typed(9, ARRAY);
const PHYS_OBJ: u32 = typed(10, DID_REF);
const STARTUP_SCRIPT_NAMES: u32 = typed(11, ARRAY);
const ANIM_SPEED: u32 = typed(12, FLOAT);
const ANIM_DISPLACEMENT: u32 = typed(13, VECTOR3);
const ANIM_ASSET: u32 = typed(14, DID_REF);
// The other animation-node fields (§6.1), typed as their stored values.
const ANIM_LOOP_FOREVER: u32 = typed(15, BOOL);
const ANIM_LOOP_COUNT: u32 = typed(16, INT);
const ANIM_RAMP_IN: u32 = typed(17, FLOAT);
const ANIM_RAMP_OUT: u32 = typed(18, FLOAT);
const ANIM_LAYER: u32 = typed(19, ENUM);
const ANIM_BONE_OPACITY: u32 = typed(20, BOOL);
const ANIM_MANUAL_UPDATE: u32 = typed(21, BOOL);
const ANIM_MANUAL_TIME: u32 = typed(22, FLOAT);
const ANIM_START_TIME: u32 = typed(23, FLOAT);
const ANIM_END_TIME: u32 = typed(24, FLOAT);
const ANIM_RANDOM_START: u32 = typed(25, BOOL);
const ANIM_SPEED_MULTIPLIER: u32 = typed(26, FLOAT);
const ANIM_SYNCHRONIZE: u32 = typed(27, BOOL);
const COMBAT_VIOLENT: u32 = typed(28, INT);
const CHARGEN_STAGE: u32 = typed(29, ENUM);
// The SliderMeshDeformer's fields (§6.9).
const SLIDER_VALUES: u32 = typed(30, STRING);
const SLIDER_DEFINITIONS: u32 = typed(31, DID_REF);
// Further entity properties for switch, When and blend tests: every comparison kind of
// `case_matches` (wide and 64-bit bit tests, numbers of each width), and two that nothing
// sets (the defaults hook answers for PIDs divisible by 3 only).
const STATE_BITS: u32 = typed(32, BITFIELD_ANY);
const STATE_MASK: u32 = typed(33, BITFIELD64);
const STATE_INT: u32 = typed(34, INT);
const STATE_INT64: u32 = typed(35, INT64);
const STATE_TRI: u32 = typed(36, TRI_STATE);
const STATE_BOOL: u32 = typed(37, BOOL);
const UNSET_DEFAULTED: u32 = typed(38, INT);
const UNSET_PLAIN: u32 = typed(39, ENUM);
// Element types of the arrays in the entity sets: table structs, table DIDs, script keys.
const ELEMENT_STRUCT: u32 = typed(40, STRUCT);
const ELEMENT_DID: u32 = typed(41, DID_REF);
const ELEMENT_KEY: u32 = typed(42, ENUM);
// `Script_CombinationType` in the per-script sets (§2.4): `Overriding` (1, the value §8.2
// step 1 states) cuts the gathering descent (§8.2 step 2).
const COMBINATION_TYPE: u32 = typed(43, ENUM);
// The EditMaterial fields (`script-table.md` §5.3, `EditMaterialScript_*`), typed as the
// resolver applies them, and a colour property to drive colours from.
const EM_MATERIAL_PROPERTY: u32 = typed(44, ENUM);
const EM_MESH_TYPE: u32 = typed(45, ENUM);
const EM_MATERIAL_TYPE: u32 = typed(46, ENUM);
const EM_PRIORITY: u32 = typed(47, INT);
const EM_COLOR: u32 = typed(48, COLOUR);
const EM_TEXTURE: u32 = typed(49, DID_REF);
const EM_BOOL: u32 = typed(50, BOOL);
const STATE_COLOUR: u32 = typed(51, COLOUR);

const EDIT_MATERIAL_FIELDS: [u32; 7] = [
    EM_MATERIAL_PROPERTY,
    EM_MESH_TYPE,
    EM_MATERIAL_TYPE,
    EM_PRIORITY,
    EM_COLOR,
    EM_TEXTURE,
    EM_BOOL,
];

const ANIMATION_FIELDS: [u32; 16] = [
    ANIM_ASSET,
    ANIM_LOOP_FOREVER,
    ANIM_LOOP_COUNT,
    ANIM_RAMP_IN,
    ANIM_RAMP_OUT,
    ANIM_LAYER,
    ANIM_BONE_OPACITY,
    ANIM_MANUAL_UPDATE,
    ANIM_MANUAL_TIME,
    ANIM_START_TIME,
    ANIM_END_TIME,
    ANIM_RANDOM_START,
    ANIM_SPEED,
    ANIM_SPEED_MULTIPLIER,
    ANIM_DISPLACEMENT,
    ANIM_SYNCHRONIZE,
];

/// Properties nodes test, blend on, set and drive from.
const STATE: [u32; 19] = [
    STATE_COLOUR,
    MOVEMENT_STYLE,
    SLIDER_PROPEL,
    SLIDER_STRAFE,
    SLIDER_TURN,
    ENTITY_ELEMENT,
    SPEED_MULTIPLIER,
    COMBAT_VIOLENT,
    CHARGEN_STAGE,
    STATE_BITS,
    STATE_MASK,
    STATE_INT,
    STATE_INT64,
    STATE_TRI,
    STATE_BOOL,
    UNSET_DEFAULTED,
    UNSET_PLAIN,
    ANIM_ASSET,
    SLIDER_VALUES,
];

const DIDS: [u32; 6] = [
    0x0700_0001,
    0x0700_0002,
    0x0C00_0001,
    0x0C00_0002,
    0x0C00_0003,
    0x0C00_0004,
];

/// Script keys: `Startup` (the vocabulary's key 5), a start-up name of the entity, keys
/// that Run and WhenRunning nodes name, and one no table defines.
const KEYS: [u32; 7] = [5, 0x1000_0039, 7, 9, 12, 13, 0x1000_0837];

/// Parameter names bound by Scriptlet ports and read by ScriptletParameter nodes (case
/// differs: §7.8 compares them ignoring ASCII case).
const NAMES: [&str; 6] = ["Idle", "idle", "WALK", "walk", "Run_Fwd", ""];

/// Nodes one record may hold, and their deepest nesting.
const RECORD_NODES: usize = 40;
const MAX_DEPTH: u8 = 6;

/// Reads the input front to back; once it is used up, a pseudo-random stream seeded from
/// the whole input, so that short inputs still give whole worlds (libFuzzer starts with
/// inputs of a few dozen bytes) and every input gives the same world.
struct Src<'a> {
    data: &'a [u8],
    at: usize,
    rng: u64,
    /// Nodes the current record may still add.
    budget: usize,
}

impl<'a> Src<'a> {
    fn new(data: &'a [u8]) -> Self {
        // FNV-1a over the input; never zero (xorshift would stay at zero).
        let seed = data.iter().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3)
        });
        Self {
            data,
            at: 0,
            rng: seed | 1,
            budget: 0,
        }
    }

    fn u8(&mut self) -> u8 {
        if let Some(&b) = self.data.get(self.at) {
            self.at += 1;
            return b;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng.to_le_bytes()[0]
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn below(&mut self, n: u8) -> u8 {
        self.u8() % n
    }

    fn bool(&mut self) -> bool {
        self.u8() & 1 == 1
    }

    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        let i = usize::from(self.u8()) % from.len();
        from[i]
    }

    /// Mostly ordinary values, sometimes hostile ones.
    fn float(&mut self) -> f32 {
        match self.below(12) {
            0 => 0.0,
            1 | 2 => 1.0,
            3 => -1.0,
            4 => 0.5,
            5 => 0.2,
            6 => 2.0,
            7 => f32::NAN,
            8 => f32::INFINITY,
            9 => -1.0e30,
            _ => f32::from_bits(self.u32()),
        }
    }

    fn did(&mut self) -> u32 {
        match self.below(10) {
            0 => 0,
            1..=3 => 0x0500_0001 + u32::from(self.below(8)), // clips
            4 => self.pick(&DIDS),
            5 => 0x4700_0002,
            _ => self.u32(),
        }
    }

    fn key(&mut self) -> u32 {
        match self.below(4) {
            0 => self.pick(&KEYS),
            1 => self.u32(),
            _ => u32::from(self.below(6)),
        }
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_bool(out: &mut Vec<u8>, src: &mut Src<'_>) {
    out.push(u8::from(src.bool()));
}

/// A TSize header (`records.md` §1.3) with hint 1.
fn put_tsize(out: &mut Vec<u8>, count: u32) {
    out.push(1);
    write_vle(out, count);
}

/// A Pascal string (`records.md` §1.4).
#[allow(clippy::cast_possible_truncation)] // the names are short
fn put_pascal(out: &mut Vec<u8>, s: &str) {
    write_vle(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

/// A value of type `code` (`property-system.md` §4.1). Containers stop growing at depth 3.
fn put_value(out: &mut Vec<u8>, src: &mut Src<'_>, code: u32, depth: u8) {
    match code {
        STRING => {
            let n = src.below(12);
            write_vle(out, u32::from(n));
            for _ in 0..n {
                out.push(b"0123456789ABCDEFxz"[usize::from(src.below(18))]);
            }
        }
        TRI_STATE => out.push(src.pick(&[0, 1, 255])),
        VECTOR3 => {
            for _ in 0..3 {
                put_f32(out, src.float());
            }
        }
        FLOAT => put_f32(out, src.float()),
        STRUCT => {
            let n = if depth >= 3 { 0 } else { src.below(4) };
            let mut fields: Vec<u32> = Vec::new();
            for _ in 0..n {
                let pid = match src.below(4) {
                    0 => DB_SCRIPT_TABLE,
                    1 => STARTUP_SCRIPT_NAMES,
                    _ => src.pick(&STATE),
                };
                // Struct field PIDs are unique.
                if !fields.contains(&pid) {
                    fields.push(pid);
                }
            }
            #[allow(clippy::cast_possible_truncation)] // at most 3 fields
            put_tsize(out, fields.len() as u32);
            for pid in fields {
                put_u32(out, pid);
                put_u32(out, pid);
                put_value(out, src, pid % 24, depth + 1);
            }
        }
        ARRAY => {
            let n = if depth >= 3 { 0 } else { src.below(4) };
            put_u32(out, u32::from(n));
            for _ in 0..n {
                let pid = src.pick(&[ELEMENT_STRUCT, ELEMENT_DID, ELEMENT_KEY]);
                put_u32(out, pid);
                put_value(out, src, pid % 24, depth + 1);
            }
        }
        BITFIELD64 | INT64 => {
            let v = match src.below(4) {
                0 => 0,
                1 => u64::MAX,
                2 => 1 << src.below(64),
                _ => u64::from(src.u32()) | (u64::from(src.u32()) << 32),
            };
            out.extend_from_slice(&v.to_le_bytes());
        }
        BITFIELD_ANY => {
            let bits = u32::from(src.pick(&[0u8, 1, 8, 52, 64, 65, 130]));
            write_vle(out, bits);
            let bytes = bits.div_ceil(8);
            for i in 0..bytes {
                let mut b = src.u8();
                // Writers zero the unused high bits of the last byte (§4.4).
                let used = bits - i * 8;
                if used < 8 {
                    b &= (1u8 << used) - 1;
                }
                out.push(b);
            }
        }
        DID_REF => put_u32(out, src.did()),
        BOOL => out.push(u8::from(src.bool())),
        ENUM | INT => put_u32(out, src.key()),
        _ => put_u32(out, src.u32()),
    }
}

/// A PV (an entry without echo): `pid` and its value, or a null entry now and then.
fn put_pv(out: &mut Vec<u8>, src: &mut Src<'_>, pid: u32) {
    if src.below(16) == 0 {
        put_u32(out, 0);
        return;
    }
    put_u32(out, pid);
    put_value(out, src, pid % 24, 1);
}

/// An embedded property set (TSize, then PID, echo and value) over PIDs from `pids`.
fn put_set(out: &mut Vec<u8>, src: &mut Src<'_>, pids: &[u32], most: u8) {
    let n = src.below(most + 1);
    let mut chosen: Vec<u32> = Vec::new();
    for _ in 0..n {
        let pid = src.pick(pids);
        if !chosen.contains(&pid) {
            chosen.push(pid);
        }
    }
    #[allow(clippy::cast_possible_truncation)] // at most `most` entries
    put_tsize(out, chosen.len() as u32);
    for pid in chosen {
        put_u32(out, pid);
        put_u32(out, pid);
        put_value(out, src, pid % 24, 0);
    }
}

/// A per-script property set (§2.4): usually a `Script_CombinationType` entry, mostly
/// `Overriding`, sometimes followed by an entity property.
fn put_script_set(out: &mut Vec<u8>, src: &mut Src<'_>) {
    if src.below(4) == 0 {
        put_set(out, src, &STATE, 2);
        return;
    }
    let extra = src.bool();
    put_tsize(out, 1 + u32::from(extra));
    put_u32(out, COMBINATION_TYPE);
    put_u32(out, COMBINATION_TYPE);
    let v = match src.below(4) {
        0 => 0,
        1 => src.u32(),
        _ => 1,
    };
    put_u32(out, v);
    if extra {
        let pid = src.pick(&STATE);
        put_u32(out, pid);
        put_u32(out, pid);
        put_value(out, src, pid % 24, 0);
    }
}

/// The base block (§4.2), with driven pairs from `fields` ← entity properties.
fn put_base(out: &mut Vec<u8>, src: &mut Src<'_>, fields: &[u32]) {
    let n = src.pick(&[0u8, 0, 1, 1, 2, 3]);
    put_tsize(out, u32::from(n));
    for _ in 0..n {
        put_u32(out, src.pick(fields));
        put_u32(out, src.pick(&STATE));
    }
    put_tsize(out, 0); // scriptlet-driven pairs
    out.push(0); // scriptlet parameter IDs
    out.push(1); // UseParentIntensity
    put_f32(out, 1.0);
    put_f32(out, 0.0);
    let p = match src.below(12) {
        0 => 0.0,
        1 => f32::NAN,
        2 => 0.25,
        3 => 2.0,
        4 => -1.0,
        _ => 1.0,
    };
    put_f32(out, p);
    put_u32(out, 1 + u32::from(src.u8()));
}

/// A port (§4.3): type, payload and echo when there is one, then children.
fn put_port(out: &mut Vec<u8>, src: &mut Src<'_>, ty: u8, payload: Option<&[u8]>, depth: u8) {
    out.push(ty);
    if let Some(p) = payload {
        out.extend_from_slice(p);
        put_u32(out, u32::from(ty));
    }
    put_children(out, src, depth);
}

/// A child count and that many nodes, within the record's node budget and depth.
fn put_children(out: &mut Vec<u8>, src: &mut Src<'_>, depth: u8) {
    let want = if depth >= MAX_DEPTH {
        0
    } else {
        usize::from(src.pick(&[0u8, 1, 1, 1, 2, 2, 3]))
    };
    let n = want.min(src.budget);
    src.budget -= n;
    #[allow(clippy::cast_possible_truncation)] // at most 3
    out.push(n as u8);
    for _ in 0..n {
        put_node(out, src, depth + 1);
    }
}

/// `count` ports produced by `payload` (`None`: no payload) with port type `ty`.
fn put_ports(
    out: &mut Vec<u8>,
    src: &mut Src<'_>,
    depth: u8,
    count: u8,
    mut port: impl FnMut(&mut Src<'_>) -> (u8, Option<Vec<u8>>),
) {
    put_u32(out, u32::from(count));
    for _ in 0..count {
        let (ty, payload) = port(src);
        put_port(out, src, ty, payload.as_deref(), depth);
    }
}

fn f32_payload(src: &mut Src<'_>, anchors: &[f32]) -> Vec<u8> {
    let v = if src.below(4) == 0 {
        src.float()
    } else {
        src.pick(anchors)
    };
    v.to_le_bytes().to_vec()
}

/// One node of a kind the evaluator follows (§5–§7), with its ports, children and an
/// empty target list.
#[allow(clippy::too_many_lines)] // one arm per node kind
fn put_node(out: &mut Vec<u8>, src: &mut Src<'_>, depth: u8) {
    let sel = src.below(22);
    let kind = match sel {
        0..=4 => 36u8,
        5 => 2,
        6 => 12,
        7 => 63,
        8 => 57,
        9 => 17,
        10 | 11 => 19,
        12 => 22,
        13 => 5,
        14 => 14,
        15 => 40,
        16 => 18,
        17 => 81,
        18 => src.pick(&[6u8, 65, 25, 10]),
        20 | 21 => src.pick(&[33u8, 44, 53, 60]),
        _ => 28,
    };
    out.push(kind);
    let ports = src.below(4);
    match kind {
        36 => {
            // §6.1, 59 bytes.
            put_u32(out, src.did());
            put_bool(out, src);
            put_u32(
                out,
                src.pick(&[1u32, 1, 2, 0, u32::MAX, 600, i32::MAX as u32]),
            );
            put_f32(out, src.float());
            put_f32(out, src.float());
            put_u32(out, src.pick(&[10u32, 20, 30, 40, 110, 15, 18]));
            put_bool(out, src);
            put_bool(out, src);
            put_f32(out, src.float());
            put_f32(out, src.pick(&[0.0f32, 0.0, 0.25, f32::NAN]));
            put_f32(out, src.pick(&[1.0f32, 1.0, 0.5, f32::INFINITY]));
            put_bool(out, src);
            put_f32(out, src.float());
            put_f32(out, src.float());
            for _ in 0..3 {
                put_f32(out, src.float());
            }
            put_bool(out, src);
            put_bool(out, src);
            put_bool(out, src);
            put_base(out, src, &ANIMATION_FIELDS);
            // Event times (Default ports with an f32), no children.
            put_u32(out, u32::from(ports));
            for _ in 0..ports {
                out.push(6);
                put_f32(out, src.float());
                put_u32(out, 6);
                out.push(0);
            }
        }
        2 | 6 | 25 => {
            match kind {
                6 => {
                    put_bool(out, src);
                    put_u32(out, src.pick(&[1u32, 0, 3, u32::MAX]));
                }
                25 => put_pascal(out, src.pick(&NAMES)),
                _ => {}
            }
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports, |s| {
                (6, Some(u32::from(s.below(4)).to_le_bytes().to_vec()))
            });
        }
        10 => {
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports, |_| (6, None));
        }
        65 => {
            put_bool(out, src);
            put_f32(out, src.float());
            put_u32(out, 1);
            put_bool(out, src);
            put_f32(out, src.float());
            put_f32(out, src.float());
            put_u32(out, src.did());
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports, |s| {
                (6, Some(f32_payload(s, &[0.0, 0.5, 1.0])))
            });
        }
        12 => {
            let property = src.pick(&STATE);
            put_u32(out, property);
            put_bool(out, src);
            put_bool(out, src);
            put_bool(out, src);
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports + 1, |s| match s.below(6) {
                0..=2 => {
                    // Switch case: values, ints, reserved 0, operator, two times.
                    let mut p = Vec::new();
                    let v = s.pick(&[1u8, 1, 1, 2, 0, 3]);
                    p.push(v);
                    for _ in 0..v {
                        let pid = if s.below(4) == 0 {
                            s.pick(&STATE)
                        } else {
                            property
                        };
                        put_pv(&mut p, s, pid);
                    }
                    let m = s.below(2);
                    p.push(m);
                    for _ in 0..m {
                        put_u32(&mut p, s.u32());
                    }
                    put_u32(&mut p, 0);
                    put_u32(&mut p, s.pick(&[3u32, 3, 3, 1, 2, 0, 7]));
                    put_f32(&mut p, s.pick(&[0.0f32, 0.2, 0.3]));
                    put_f32(&mut p, 0.0);
                    (5, Some(p))
                }
                3 | 4 => {
                    let mut p = Vec::new();
                    put_f32(&mut p, 0.0);
                    put_f32(&mut p, 0.0);
                    (6, Some(p))
                }
                _ => {
                    // Switch transition: two PVs and the tail.
                    let mut p = Vec::new();
                    put_pv(&mut p, s, property);
                    put_pv(&mut p, s, property);
                    p.extend_from_slice(&[0, 0]);
                    put_f32(&mut p, 0.0);
                    p.push(1);
                    for v in [0.0f32, -1.0, 0.0] {
                        put_f32(&mut p, v);
                    }
                    (28, Some(p))
                }
            });
        }
        63 => {
            let property = src.pick(&STATE);
            put_u32(out, property);
            put_bool(out, src);
            put_bool(out, src);
            put_bool(out, src);
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports + 1, |s| {
                if s.below(4) == 0 {
                    return (6, None);
                }
                // When test: constant, right-hand property, operator, two zeros.
                let mut p = Vec::new();
                put_pv(&mut p, s, property);
                let rhs = if s.below(4) == 0 { s.pick(&STATE) } else { 0 };
                put_u32(&mut p, rhs);
                put_u32(&mut p, s.pick(&[1u32, 2, 3, 4, 5, 6, 0, 9]));
                put_u32(&mut p, 0);
                put_u32(&mut p, 0);
                (24, Some(p))
            });
        }
        57 => {
            let property = match src.below(4) {
                0 => SLIDER_PROPEL,
                1 => SLIDER_STRAFE,
                2 => SLIDER_TURN,
                _ => src.pick(&STATE),
            };
            put_u32(out, property);
            put_bool(out, src);
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports + 1, |s| {
                (
                    6,
                    Some(f32_payload(s, &[-1.0, 0.0, 1.0, 0.57, -0.5, -0.57])),
                )
            });
        }
        17 => {
            put_bool(out, src);
            put_bool(out, src);
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports + 1, |s| {
                let w = s.pick(&[1u32, 1, 2, 3, 0, 100, u32::MAX]);
                (6, Some(w.to_le_bytes().to_vec()))
            });
        }
        19 => {
            let file = match src.below(8) {
                0 => 0,
                1 => 0x0C00_0099, // missing
                2 => DIDS[0],     // not a scriptlet
                _ => src.pick(&DIDS[2..]),
            };
            put_u32(out, file);
            if src.below(4) == 0 {
                put_set(out, src, &EDIT_MATERIAL_FIELDS, 3);
            } else {
                put_set(out, src, &ANIMATION_FIELDS, 3);
            }
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports, |s| {
                let mut p = Vec::new();
                put_pascal(&mut p, s.pick(&NAMES));
                (6, Some(p))
            });
        }
        22 => {
            put_pascal(out, src.pick(&NAMES));
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports.min(1), |_| (6, None));
        }
        5 => {
            let version = if src.below(32) == 0 { 2 } else { 1 };
            out.push(version);
            let n = src.below(3);
            out.push(n);
            for _ in 0..n {
                put_u32(out, src.pick(&STATE));
                put_u32(out, src.pick(&[0, MOVEMENT_STYLE, STATE_INT]));
                let pid = src.pick(&STATE);
                put_pv(out, src, pid);
            }
            put_u32(out, src.pick(&KEYS));
            out.push(0);
            out.push(0);
            out.push(u8::from(src.below(8) == 0)); // stop
            out.push(0);
            put_u32(out, 0);
            put_base(out, src, &ANIMATION_FIELDS);
            put_u32(out, 0);
        }
        14 => {
            let n = src.below(3) + 1;
            out.push(n);
            for _ in 0..n {
                put_u32(out, src.pick(&KEYS));
            }
            out.push(0);
            put_bool(out, src);
            put_base(out, src, &ANIMATION_FIELDS);
            put_ports(out, src, depth, ports, |s| (s.pick(&[10u8, 11]), None));
        }
        40 => {
            let property = src.pick(&STATE);
            put_u32(out, property);
            put_pv(out, src, property);
            put_bool(out, src);
            put_base(out, src, &STATE);
            put_u32(out, 0);
        }
        18 => {
            put_u32(out, src.did());
            put_f32(out, src.float());
            put_bool(out, src);
            put_base(
                out,
                src,
                &[
                    ANIM_MANUAL_TIME,
                    ANIM_ASSET,
                    ANIM_LOOP_FOREVER,
                    ANIM_SPEED,
                    STATE_INT,
                ],
            );
            put_u32(out, 0);
        }
        33 => {
            // ModifyCompositorColor: name, colour key, colour.
            put_u32(out, src.pick(&[1u32, 2, 0x1000_0001]));
            put_u32(out, src.pick(&[0x1000_000B, 0x1000_0007, 3]));
            put_u32(out, src.u32());
            put_base(out, src, &[STATE_COLOUR, STATE_INT, ANIM_ASSET]);
            put_u32(out, 0);
        }
        44 => {
            // ModifyCompositorLayer: name, layer key, image number, opacity.
            put_u32(out, src.pick(&[1u32, 2, 0x1000_0001]));
            put_u32(out, src.pick(&[0x1000_0001, 0x1000_0004, 5]));
            put_u32(out, src.pick(&[0u32, 1, 2, u32::MAX, 0x8000_0000]));
            put_f32(out, src.float());
            put_base(out, src, &[STATE_INT, STATE_INT64, SLIDER_PROPEL]);
            put_u32(out, 0);
        }
        53 => {
            // TextureCompositor: name, record, mesh type, material type.
            put_u32(out, src.pick(&[1u32, 2, 0x1000_0001]));
            put_u32(out, src.did());
            put_u32(out, src.pick(&[2u32, 0x1000_0002, 7]));
            put_u32(out, src.pick(&[0x1000_006C, 9]));
            put_base(out, src, &[ANIM_ASSET, STATE_INT]);
            put_u32(out, 0);
        }
        60 => {
            // EditMaterial: property, mesh type, material type, priority, waveform, colour,
            // texture, bool.
            put_u32(out, src.pick(&[2003u32, 3000, 2000, 1, u32::MAX]));
            put_u32(out, src.pick(&[2u32, 0x1000_000E, 0]));
            put_u32(out, src.key());
            put_u32(out, src.pick(&[5u32, 20, 0, u32::MAX]));
            put_waveform(out, src);
            put_u32(out, src.u32());
            put_u32(out, src.did());
            put_bool(out, src);
            put_base(out, src, &EDIT_MATERIAL_FIELDS);
            put_u32(out, 0);
        }
        81 => {
            put_pascal(out, src.pick(&["0A1B", "", "zz", "00FF7F"]));
            put_u32(out, src.did());
            put_base(out, src, &[SLIDER_VALUES, SLIDER_DEFINITIONS, ANIM_ASSET]);
            put_u32(out, 0);
        }
        _ => {
            // StoryHook: a type, no ports.
            put_u32(out, src.u32());
            put_base(out, src, &ANIMATION_FIELDS);
            put_u32(out, 0);
        }
    }
    put_u32(out, 0); // targets
}

/// A waveform (`property-system.md` §4.2): mostly empty or short, now and then subtype 10
/// with many pairs.
fn put_waveform(out: &mut Vec<u8>, src: &mut Src<'_>) {
    match src.below(8) {
        0..=3 => put_u32(out, 0),
        4 => {
            put_u32(out, 1);
            put_f32(out, src.float());
        }
        5 => {
            put_u32(out, 3);
            for _ in 0..10 {
                put_f32(out, src.float());
            }
        }
        _ => {
            put_u32(out, 10);
            for _ in 0..10 {
                put_f32(out, src.float());
            }
            put_f32(out, src.float());
            put_bool(out, src);
            let k: u32 = src.pick(&[0, 1, 3, 500, 4000]);
            put_u32(out, k);
            for _ in 0..k {
                put_f32(out, 0.5);
                put_f32(out, 1.0);
            }
        }
    }
}

/// `n` root nodes.
fn put_roots(out: &mut Vec<u8>, src: &mut Src<'_>) {
    let n = usize::from(src.below(3)) + 1;
    let n = n.min(src.budget.max(1));
    src.budget = src.budget.saturating_sub(n);
    #[allow(clippy::cast_possible_truncation)] // at most 3
    write_vle(out, n as u32);
    for _ in 0..n {
        put_node(out, src, 0);
    }
}

/// A 0x07 record (§2.1): kind, includes, scripts, per-script sets and DID lists.
fn script_table(src: &mut Src<'_>) -> Vec<u8> {
    src.budget = RECORD_NODES;
    let mut out = Vec::new();
    put_u32(&mut out, src.pick(&[1u32, 2, 2, 4, 3]));
    let n = src.below(4);
    write_vle(&mut out, u32::from(n));
    for _ in 0..n {
        let did = match src.below(6) {
            0 => 0x0700_0099, // missing
            1 => DIDS[2],     // not a table
            _ => src.pick(&DIDS[..2]),
        };
        put_u32(&mut out, did);
    }
    let scripts = src.below(4) + 1;
    put_tsize(&mut out, u32::from(scripts));
    let mut keys: Vec<u32> = Vec::new();
    for i in 0..scripts {
        let key = if i == 0 && src.below(4) != 0 {
            KEYS[0]
        } else {
            src.pick(&KEYS)
        };
        keys.push(key);
        put_u32(&mut out, key);
        put_roots(&mut out, src);
    }
    let sets = src.below(2);
    put_tsize(&mut out, u32::from(sets));
    for _ in 0..sets {
        put_u32(&mut out, src.pick(&keys));
        put_script_set(&mut out, src);
    }
    let lists = src.below(2);
    put_tsize(&mut out, u32::from(lists));
    for _ in 0..lists {
        put_u32(&mut out, src.pick(&keys));
        let c = src.below(3);
        put_u32(&mut out, u32::from(c));
        for _ in 0..c {
            put_u32(&mut out, src.did());
        }
    }
    out
}

/// A 0x0C record (§3): roots and a dependency list.
fn scriptlet(src: &mut Src<'_>) -> Vec<u8> {
    src.budget = RECORD_NODES;
    let mut out = Vec::new();
    put_roots(&mut out, src);
    let n = src.below(3);
    put_u32(&mut out, u32::from(n));
    for _ in 0..n {
        put_u32(&mut out, src.did());
    }
    out
}

/// An embedded property set for the entity chain: the class table struct, additional
/// tables, `PhysObj` and entity properties.
fn entity_set(src: &mut Src<'_>) -> Vec<u8> {
    let mut entries: Vec<(u32, Vec<u8>)> = Vec::new();
    if src.below(4) != 0 {
        // The class table struct: a table DID and start-up script names.
        let mut v = Vec::new();
        put_tsize(&mut v, 2);
        put_u32(&mut v, DB_SCRIPT_TABLE);
        put_u32(&mut v, DB_SCRIPT_TABLE);
        put_u32(&mut v, src.pick(&[DIDS[0], DIDS[1], 0, 0x0700_0099]));
        put_u32(&mut v, STARTUP_SCRIPT_NAMES);
        put_u32(&mut v, STARTUP_SCRIPT_NAMES);
        let n = src.below(3);
        put_u32(&mut v, u32::from(n));
        for _ in 0..n {
            put_u32(&mut v, ELEMENT_KEY);
            put_u32(&mut v, src.pick(&KEYS));
        }
        entries.push((CLASS_SCRIPT_TABLE, v));
    }
    if src.below(2) == 0 {
        let mut v = Vec::new();
        put_value(&mut v, src, ARRAY, 1);
        entries.push((ADDITIONAL_SCRIPT_TABLES, v));
    }
    if src.below(2) == 0 {
        let d = src.pick(&[0x4700_0002u32, 0x4700_0003, 0, 0x7000_0001, DIDS[0]]);
        entries.push((PHYS_OBJ, d.to_le_bytes().to_vec()));
    }
    for _ in 0..src.below(4) {
        let pid = src.pick(&STATE);
        if entries.iter().any(|(p, _)| *p == pid) {
            continue;
        }
        let mut v = Vec::new();
        put_value(&mut v, src, pid % 24, 1);
        entries.push((pid, v));
    }
    let mut out = Vec::new();
    #[allow(clippy::cast_possible_truncation)] // at most 7 entries
    put_tsize(&mut out, entries.len() as u32);
    for (pid, v) in entries {
        put_u32(&mut out, pid);
        put_u32(&mut out, pid);
        out.extend_from_slice(&v);
    }
    out
}

/// The entity's own property values for the resolutions: every comparison kind.
fn entity_state(src: &mut Src<'_>, entity: &mut EntityScripts) {
    for pid in STATE {
        if src.below(3) != 0 {
            continue;
        }
        let v = match pid % 24 {
            BITFIELD_ANY => {
                let bits = u32::from(src.pick(&[0u8, 8, 52, 130]));
                PropertyValue::BitfieldAny(BitfieldAny {
                    bits,
                    bytes: (0..bits.div_ceil(8)).map(|_| src.u8()).collect(),
                })
            }
            BITFIELD64 => PropertyValue::Bitfield64(u64::from(src.u32())),
            INT64 => PropertyValue::Int64(i64::from(src.u32() as i32)),
            TRI_STATE => PropertyValue::TriState(src.pick(&[0, 1, 255])),
            BOOL => PropertyValue::Bool(src.bool()),
            COLOUR => PropertyValue::Colour(src.u32().to_le_bytes()),
            FLOAT => PropertyValue::Float(src.float()),
            DID_REF => PropertyValue::DidRef(Did(src.did())),
            STRING => PropertyValue::String(src.pick(&["0A1B", "zz", ""]).to_owned()),
            INT => PropertyValue::Int(src.key() as i32),
            _ => PropertyValue::Enum(src.key()),
        };
        entity.properties.set(pid, v);
    }
}

/// The six records and the two set parts, written by the harness from `data`.
fn structured(data: &[u8], entity: &mut EntityScripts) -> (Vec<Vec<u8>>, Vec<u8>, Vec<u8>) {
    let mut src = Src::new(data);
    let mut records = vec![script_table(&mut src), script_table(&mut src)];
    for _ in 2..DIDS.len() {
        records.push(scriptlet(&mut src));
    }
    // Now and then one byte of one record is damaged.
    if src.below(8) == 0 {
        let r = usize::from(src.u8()) % records.len();
        let flip = src.u8() | 1;
        if let Some(rec) = records.get_mut(r) {
            let len = rec.len().max(1);
            let at = usize::from(src.u8()) * 257 % len;
            if let Some(b) = rec.get_mut(at) {
                *b ^= flip;
            }
        }
    }
    let object = entity_set(&mut src);
    let ent = entity_set(&mut src);
    entity_state(&mut src, entity);
    (records, object, ent)
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let mut entity = EntityScripts {
        class_table: Some(Did(DIDS[0])),
        additional_tables: vec![Did(DIDS[1])],
        startup_scripts: vec![0x1000_0039, 5],
        ..EntityScripts::default()
    };
    let mut world: HashMap<Did, Vec<u8>> = HashMap::new();
    let (records, part0, part1): (Vec<Vec<u8>>, Vec<u8>, Vec<u8>) = if mode % 4 == 0 {
        // Split on a 0xFF 0xFE marker pair into at most six records.
        let mut parts: Vec<&[u8]> = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i + 1 < rest.len() && parts.len() < DIDS.len() - 1 {
            if rest[i] == 0xFF && rest[i + 1] == 0xFE {
                parts.push(&rest[start..i]);
                start = i + 2;
                i += 2;
            } else {
                i += 1;
            }
        }
        parts.push(&rest[start.min(rest.len())..]);
        let p0 = parts[0].to_vec();
        let p1 = parts.get(1).unwrap_or(&parts[0]).to_vec();
        (parts.iter().map(|p| p.to_vec()).collect(), p0, p1)
    } else {
        structured(rest, &mut entity)
    };
    for (did, bytes) in DIDS.iter().zip(&records) {
        world.insert(Did(*did), bytes.clone());
    }
    // The first two parts also serve as a game object's property set and its entity.
    world.insert(Did(0x7900_0001), part0.clone());
    world.insert(Did(0x4700_0001), part1.clone());
    let vocab = ScriptVocabulary {
        class_script_table: CLASS_SCRIPT_TABLE,
        db_script_table: DB_SCRIPT_TABLE,
        startup_key: 5,
        movement_style: MOVEMENT_STYLE,
        style_walking: 1,
        style_running: 2,
        slider_propel: SLIDER_PROPEL,
        slider_strafe: SLIDER_STRAFE,
        slider_turn: SLIDER_TURN,
        entity_element: Some(ENTITY_ELEMENT),
        element_ground: Some(5),
        element_no_surface: Some(4),
        speed_multiplier: Some(SPEED_MULTIPLIER),
        combat_violent: Some(COMBAT_VIOLENT),
        chargen_stage: Some(CHARGEN_STAGE),
        additional_script_tables: Some(ADDITIONAL_SCRIPT_TABLES),
        phys_obj: Some(PHYS_OBJ),
        startup_script_names: Some(STARTUP_SCRIPT_NAMES),
        slider_values: Some(SLIDER_VALUES),
        slider_definitions: Some(SLIDER_DEFINITIONS),
        // The MeshDeformer's fields reuse three animation PIDs (§6.9); ANIM_SPEED pairs on a
        // deformer name another float field and must leave its time alone.
        mesh_deformer: MeshDeformerFieldPids {
            asset: Some(ANIM_ASSET),
            time: Some(ANIM_MANUAL_TIME),
            use_bone_weights: Some(ANIM_LOOP_FOREVER),
        },
        animation: AnimationFieldPids {
            asset: Some(ANIM_ASSET),
            loop_forever: Some(ANIM_LOOP_FOREVER),
            loop_count: Some(ANIM_LOOP_COUNT),
            ramp_in: Some(ANIM_RAMP_IN),
            ramp_out: Some(ANIM_RAMP_OUT),
            layer: Some(ANIM_LAYER),
            use_bone_opacity: Some(ANIM_BONE_OPACITY),
            manual_update: Some(ANIM_MANUAL_UPDATE),
            manual_time: Some(ANIM_MANUAL_TIME),
            start_time: Some(ANIM_START_TIME),
            end_time: Some(ANIM_END_TIME),
            randomize_start_time: Some(ANIM_RANDOM_START),
            speed: Some(ANIM_SPEED),
            speed_multiplier: Some(ANIM_SPEED_MULTIPLIER),
            displacement: Some(ANIM_DISPLACEMENT),
            synchronize_timing: Some(ANIM_SYNCHRONIZE),
        },
        // Without it every definition is additive and `gather` never takes its
        // overriding branch.
        combination_type: Some(COMBINATION_TYPE),
        edit_material: EditMaterialFieldPids {
            material_property: Some(EM_MATERIAL_PROPERTY),
            mesh_type: Some(EM_MESH_TYPE),
            material_type: Some(EM_MATERIAL_TYPE),
            priority: Some(EM_PRIORITY),
            color_value: Some(EM_COLOR),
            texture_value: Some(EM_TEXTURE),
            bool_value: Some(EM_BOOL),
        },
        ..ScriptVocabulary::default()
    };
    // Bits 2–3 of byte 0 all set: tight limits, so that every limit is reached.
    let limits = if (mode >> 2) % 4 == 3 {
        ResolveLimits {
            max_tables: 1,
            max_include_depth: 1,
            max_call_depth: 2,
            max_eval_depth: 4,
            max_node_visits: 40,
            max_work: 400,
            max_candidates: 3,
        }
    } else {
        ResolveLimits {
            max_node_visits: 20_000,
            max_candidates: 1_000,
            ..ResolveLimits::default()
        }
    };
    let mut r = ClipResolver::new(&total, vocab, |d: Did| {
        Ok::<_, Infallible>(world.get(&d).cloned())
    })
    .with_limits(limits)
    .with_defaults(|pid| (pid % 3 == 0).then_some(PropertyValue::Int(0)))
    // Seeded Random picks for the Appearance candidates (`appearance.md` §7.4); the same
    // seed must give the same picks.
    .with_random_seed(Some(u64::from(limits.max_candidates as u32)));
    let first = r
        .resolve(&entity, MotionState::Idle)
        .expect("fetch never fails");
    let again = r
        .resolve(&entity, MotionState::Idle)
        .expect("fetch never fails");
    let picks = |r: &tessera_formats::script_table::resolve::ClipResolution| {
        r.appearances
            .iter()
            .map(|a| (a.appearance, a.key, a.modifier.to_bits(), a.picked))
            .collect::<Vec<_>>()
    };
    assert_eq!(picks(&first), picks(&again), "deterministic picks");
    for a in &first.appearances {
        assert!(a.chance.is_nan() || (0.0..=1.0).contains(&a.chance));
    }
    for state in MotionState::ALL {
        let res = r.resolve(&entity, state).expect("fetch never fails");
        assert!(res.appearances.len() <= limits.max_candidates);
        assert!(res.nodes_visited <= limits.max_node_visits + 1);
        assert!(res.candidates.len() <= limits.max_candidates);
        for c in &res.candidates {
            assert!(!c.clip.is_null());
            let _ = c.natural_speed(1.0);
            let _ = c.playback_rate();
            let f = c.held_fraction();
            assert!((0.0..=1.0).contains(&f), "held fractions stay in range");
            let t =
                tessera_formats::script_table::resolve::held_clip_time(f, 2.0, c.held_end_crop());
            assert!(
                (0.0..=2.0).contains(&t),
                "held times stay in the played length"
            );
        }
        assert!(res.deformers.len() <= limits.max_candidates);
        assert!(res.look.len() <= limits.max_candidates);
        assert!(res.held().len() <= res.candidates.len());
        assert_issues_unique(&res);
        let _ = res.primary(10);
        let _ = res.clips();
    }
    let all = r.reachable(&entity).expect("fetch never fails");
    assert!(all.candidates.len() <= limits.max_candidates);
    assert!(all.impulses.len() <= limits.max_candidates);
    assert_issues_unique(&all);
    // One-script runs (`collision.md` §11.10, the jump script): every key the start-up
    // run knows plus a key from the input, with the element set to each state.
    for key in [5, 0x1000_0039, u32::from(mode)] {
        for element in 1..=6u32 {
            let mut st = entity.properties.clone();
            st.set(ENTITY_ELEMENT, PropertyValue::Enum(element));
            let one = r
                .resolve_script(&entity, key, st)
                .expect("fetch never fails");
            assert!(one.nodes_visited <= limits.max_node_visits + 1);
            assert!(one.impulses.len() <= limits.max_candidates);
            assert_issues_unique(&one);
        }
    }
    // Entity chains from a game object and from an entity: errors, never panics; a chain
    // that resolves is evaluated too. The 0x…0001 starts read the raw parts above; the
    // 0x…0002 starts read the same parts behind a prologue the harness writes (the set
    // record's DID, an entity header linked to a template), so that the parts only have to
    // be property sets for the chain to resolve. Every other set or entity DID (a
    // `PhysObj`) gets the framed form too.
    let framed = |d: Did| -> Option<Vec<u8>> {
        if let Some(b) = world.get(&d) {
            return Some(b.clone());
        }
        match d.prefix() {
            0x79 => {
                let mut out = d.0.to_le_bytes().to_vec();
                out.extend_from_slice(&part0);
                Some(out)
            }
            0x47 => EntityDescriptor {
                header: EntityHeader {
                    flags: ENTITY_FLAGS_REQUIRED,
                    did: d,
                    kind: 7,
                    template: Did(0x1F00_0001),
                    block: None,
                },
                properties: PropertySet::default(),
                extra_sets: Vec::new(),
            }
            .to_bytes_with(&part1)
            .ok(),
            _ => None,
        }
    };
    for start in [
        Did(0x7000_0001),
        Did(0x4700_0001),
        Did(0x7000_0002),
        Did(0x4700_0002),
    ] {
        if let Ok(chain) = collect_entity_scripts(start, &total, r.vocabulary(), |d: Did| {
            Ok::<_, Infallible>(framed(d))
        }) {
            let res = r
                .resolve(&chain.scripts, MotionState::Idle)
                .expect("fetch never fails");
            assert!(res.candidates.len() <= limits.max_candidates);
            assert_issues_unique(&res);
        }
    }
    let _ = r
        .closure(&[Did(DIDS[0]), Did(DIDS[1])])
        .expect("fetch never fails");
});
