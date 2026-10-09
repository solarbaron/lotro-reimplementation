// SPDX-License-Identifier: GPL-3.0-or-later
//! Day and night lights of placed objects (`instance-scripts.md` §6, §9.4): lamps,
//! lanterns and lit windows whose class table runs the light-glow scriptlet, computed
//! directly instead of by running the script tables.
//!
//! | Item | Spec |
//! |---|---|
//! | [`GlowPids`] | the member overrides of §6.3 and the two engine properties of §6.2 |
//! | [`GlowThresholds::from_scriptlet`] | the day-fraction thresholds and comparisons, read from the scriptlet (§6.2) |
//! | [`running_variants`], [`TimeOverride`] | which of the day and night scripts run (§6.2, §9.4 step 1) |
//! | [`GlowStored`], [`GlowStored::from_light`], [`GlowTableLights::from_table`] | the stored fields of the day and night Light nodes, read from the light-glow table (§6.3) |
//! | [`glow_light`], [`GlowLight`] | the light of a running, activated variant (§6.3, §6.4, §9.4 step 2) |
//!
//! The thresholds and comparisons are data of the scriptlet and the stored light fields
//! are data of the table's Light nodes (kind 13, `script-table.md` §5.3); both are read from
//! the install, nothing here hard-codes them.

use tessera_formats::script_table::{
    LightNode, Node, NodeData, NodeKind, PortPayload, ScriptTable, Scriptlet, when_op,
};
use tessera_formats::terrain_type::FloatCurve;
use tessera_formats::{PropertyNames, PropertyValue};

use super::entities::{LightParams, waveform_curve};
use crate::environment::property_colour;
use crate::scenery::value_bool;

/// Number of thresholds the light-glow scriptlet compares the day fraction with (§6.2).
pub const THRESHOLD_COUNT: usize = 4;

/// The property IDs of the light-glow rule, looked up by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlowPids {
    /// `TimeOfDay@Entity.m_eTimeOverride` (enum; dictionary default 2 `Automatic`).
    pub time_override: Option<u32>,
    /// `TimeOfDay@Entity.m_bIgnoreInsideOutside`.
    pub ignore_inside_outside: Option<u32>,
    /// `World_PresentTimeOfDay` (the day fraction the client publishes).
    pub present_time: Option<u32>,
    /// `Volume_IsOutside` (tri-state, default 255).
    pub is_outside: Option<u32>,
    /// The `Day` members, in [`MEMBERS`] order.
    pub day: [Option<u32>; 8],
    /// The `Night` members, in [`MEMBERS`] order.
    pub night: [Option<u32>; 8],
}

/// The member suffixes of `Light.Script@Entity.Day.*` / `….Night.*` (§6.3): the seven driven
/// light fields, then the activation switch.
pub const MEMBERS: [&str; 8] = [
    "m_cLightDiffuse",
    "m_fLightRange",
    "m_fLightFalloff",
    "m_wLightDiffuseIntensity",
    "m_cLightSpecular",
    "m_bKeyLight",
    "m_bShadowCaster",
    "m_bLightActivated",
];

/// Index of `m_bLightActivated` in [`MEMBERS`].
const ACTIVATED: usize = 7;

impl GlowPids {
    /// Resolves the names against `names`.
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let member =
            |variant: &str, m: &str| names.pid_of(&format!("Light.Script@Entity.{variant}.{m}"));
        Self {
            time_override: names.pid_of("TimeOfDay@Entity.m_eTimeOverride"),
            ignore_inside_outside: names.pid_of("TimeOfDay@Entity.m_bIgnoreInsideOutside"),
            present_time: names.pid_of("World_PresentTimeOfDay"),
            is_outside: names.pid_of("Volume_IsOutside"),
            day: MEMBERS.map(|m| member("Day", m)),
            night: MEMBERS.map(|m| member("Night", m)),
        }
    }

    /// The members of `variant`.
    #[must_use]
    pub fn members(&self, variant: Variant) -> &[Option<u32>; 8] {
        match variant {
            Variant::Day => &self.day,
            Variant::Night => &self.night,
        }
    }
}

/// One of the two light-glow scripts (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// The day script (label `Light_Glow_Off`).
    Day,
    /// The night script (label `Light_Glow_On`).
    Night,
}

/// The four day-fraction thresholds of the scriptlet, ascending (§6.2: night up to the
/// first, both up to the second, day up to the third, both up to the fourth, then night).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowThresholds(pub [f32; THRESHOLD_COUNT]);

impl GlowThresholds {
    /// Reads the thresholds from the light-glow scriptlet: the constants of the tests of
    /// every When node on `World_PresentTimeOfDay` (`present_time`), which must be exactly
    /// four distinct finite values compared with `LessThanOrEqual` or `GreaterThan` (the two
    /// comparisons of §6.2). `None` when the scriptlet does not have that shape.
    #[must_use]
    pub fn from_scriptlet(scriptlet: &Scriptlet, present_time: u32) -> Option<Self> {
        let mut values: Vec<f32> = Vec::new();
        for t in time_tests(scriptlet, present_time) {
            if t.operator != when_op::LESS_THAN_OR_EQUAL && t.operator != when_op::GREATER_THAN {
                return None;
            }
            match t.constant {
                Some(v) if v.is_finite() => values.push(v),
                _ => return None,
            }
        }
        values.sort_by(f32::total_cmp);
        values.dedup();
        <[f32; THRESHOLD_COUNT]>::try_from(values).ok().map(Self)
    }
}

/// One test of a When node on the day fraction (§6.2): the constant and the comparison
/// (enum 0x230000A3 key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeTest {
    /// The constant compared with the day fraction (`None` when it is not a float).
    pub constant: Option<f32>,
    /// The comparison key.
    pub operator: i32,
}

/// Every test of the When nodes on `World_PresentTimeOfDay` (`present_time`) in the
/// scriptlet, in walk order (roots in order, parents before children).
#[must_use]
pub fn time_tests(scriptlet: &Scriptlet, present_time: u32) -> Vec<TimeTest> {
    let mut out = Vec::new();
    for root in &scriptlet.roots {
        root.walk(&mut |n: &Node, _| {
            if n.kind != NodeKind::WHEN {
                return;
            }
            let NodeData::Switch(sw) = &n.data else {
                return;
            };
            if sw.property != present_time {
                return;
            }
            for p in &n.ports {
                if let PortPayload::WhenTest(t) = &p.payload {
                    out.push(TimeTest {
                        constant: t.constant.as_ref().and_then(|c| c.value.as_f32()),
                        operator: t.operator,
                    });
                }
            }
        });
    }
    out
}

/// `TimeOfDay@Entity.m_eTimeOverride` by its mapper label (§6.2 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeOverride {
    /// `Never`: day.
    Never,
    /// `Night`: night.
    Night,
    /// `Day`: day.
    Day,
    /// `Dusk`: day and night.
    Dusk,
    /// `Dawn`: day and night.
    Dawn,
    /// `Always`: night.
    Always,
    /// `Automatic` (the dictionary default): by place and day fraction.
    Automatic,
}

impl TimeOverride {
    /// The case of mapper label `label`; `None` for any other label (no case runs).
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "Never" => Self::Never,
            "Night" => Self::Night,
            "Day" => Self::Day,
            "Dusk" => Self::Dusk,
            "Dawn" => Self::Dawn,
            "Always" => Self::Always,
            "Automatic" => Self::Automatic,
            _ => return None,
        })
    }
}

/// Which scripts run (§6.2, §9.4 step 1): (day, night). `ignore_inside_outside` is
/// `m_bIgnoreInsideOutside` (default false); `is_outside` is `Volume_IsOutside` (0 inside,
/// 1 outside, any other value unset); `t` is `World_PresentTimeOfDay`.
#[must_use]
pub fn running_variants(
    case: TimeOverride,
    ignore_inside_outside: bool,
    is_outside: u8,
    t: f32,
    thresholds: &GlowThresholds,
) -> (bool, bool) {
    match case {
        TimeOverride::Never | TimeOverride::Day => (true, false),
        TimeOverride::Night | TimeOverride::Always => (false, true),
        TimeOverride::Dusk | TimeOverride::Dawn => (true, true),
        TimeOverride::Automatic => {
            if !ignore_inside_outside && is_outside != 1 {
                return (false, false);
            }
            let [a, b, c, d] = thresholds.0;
            if t <= a {
                (false, true)
            } else if t <= b {
                (true, true)
            } else if t <= c {
                (true, false)
            } else if t <= d {
                (true, true)
            } else {
                (false, true)
            }
        }
    }
}

/// The Light node's stored fields (§6.3), used for a member the entity does not hold
/// (Decision Q2). Colours are the stored bytes (B, G, R, A).
#[derive(Debug, Clone, PartialEq)]
pub struct GlowStored {
    /// `LightDiffuse`.
    pub diffuse: [u8; 4],
    /// `LightRange`.
    pub range: f32,
    /// `LightFalloff`.
    pub falloff: f32,
    /// `LightDiffuseIntensity`.
    pub intensity: FloatCurve,
    /// `LightSpecular`.
    pub specular: [u8; 4],
    /// `KeyLight`.
    pub key_light: bool,
    /// `ShadowCaster`.
    pub shadow_caster: bool,
}

impl GlowStored {
    /// The stored fields of a Light node (§6.3, `script-table.md` §5.3). An intensity
    /// waveform without a curve form keeps the light default.
    #[must_use]
    pub fn from_light(l: &LightNode) -> Self {
        Self {
            diffuse: l.diffuse,
            range: l.range,
            falloff: l.falloff,
            intensity: waveform_curve(&l.diffuse_intensity)
                .unwrap_or_else(|| LightParams::default().intensity),
            specular: l.specular,
            key_light: l.key_light,
            shadow_caster: l.shadow_caster,
        }
    }
}

/// The stored fields of the day and night Light nodes of a light-glow table (§6.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlowTableLights {
    /// The day script's Light node.
    pub day: Option<GlowStored>,
    /// The night script's Light node.
    pub night: Option<GlowStored>,
    /// The day script's Light node as stored (every field).
    pub day_node: Option<LightNode>,
    /// The night script's Light node as stored.
    pub night_node: Option<LightNode>,
}

impl GlowTableLights {
    /// Finds the Light nodes of `table` (§6.3): each variant's script is a Switch on that
    /// variant's `m_bLightActivated` member whose case holds a Light node, so the variant is
    /// told by the Switch's property ([`GlowPids`]), not by a script key. The first Light
    /// node found under such a Switch, in script and walk order, is taken.
    #[must_use]
    pub fn from_table(table: &ScriptTable, pids: &GlowPids) -> Self {
        let mut out = Self::default();
        let day = pids.day[ACTIVATED];
        let night = pids.night[ACTIVATED];
        for script in &table.scripts {
            for root in &script.roots {
                root.walk(&mut |n: &Node, _| {
                    let NodeData::Switch(sw) = &n.data else {
                        return;
                    };
                    if n.kind != NodeKind::SWITCH {
                        return;
                    }
                    let slot = if Some(sw.property) == day {
                        &mut out.day_node
                    } else if Some(sw.property) == night {
                        &mut out.night_node
                    } else {
                        return;
                    };
                    if slot.is_some() {
                        return;
                    }
                    let mut found = None;
                    for p in &n.ports {
                        for c in &p.children {
                            c.walk(&mut |m: &Node, _| {
                                if found.is_none()
                                    && let NodeData::Light(l) = &m.data
                                {
                                    found = Some((**l).clone());
                                }
                            });
                        }
                    }
                    *slot = found;
                });
            }
        }
        out.day = out.day_node.as_ref().map(GlowStored::from_light);
        out.night = out.night_node.as_ref().map(GlowStored::from_light);
        out
    }

    /// The stored fields of `variant`.
    #[must_use]
    pub fn stored(&self, variant: Variant) -> Option<&GlowStored> {
        match variant {
            Variant::Day => self.day.as_ref(),
            Variant::Night => self.night.as_ref(),
        }
    }
}

/// A light-glow light (§6.3): the light fields plus `ShadowCaster`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlowLight {
    /// The light fields (point light).
    pub params: LightParams,
    /// `ShadowCaster`.
    pub shadow_caster: bool,
}

/// The light of `variant` (§9.4 step 2), or `None` when its `m_bLightActivated` is false.
/// `member` reads a member through the entity's lookup (per-instance set, then the object's
/// chain; no dictionary defaults, Decision Q2); `activated_default` is the member's
/// dictionary default (false for `Day`, true for `Night` in the install), used when the
/// lookup finds nothing.
pub fn glow_light<'v>(
    pids: &GlowPids,
    variant: Variant,
    member: impl Fn(u32) -> Option<&'v PropertyValue>,
    activated_default: bool,
    stored: &GlowStored,
) -> Option<GlowLight> {
    let m = pids.members(variant);
    let get = |i: usize| m[i].and_then(&member);
    let activated = get(ACTIVATED)
        .and_then(value_bool)
        .unwrap_or(activated_default);
    if !activated {
        return None;
    }
    let colour = |i: usize, fallback: [u8; 4]| {
        let bytes = match get(i) {
            Some(PropertyValue::Colour(c)) => *c,
            _ => fallback,
        };
        let [r, g, b, _] = property_colour(bytes);
        [r, g, b]
    };
    let float = |i: usize, fallback: f32| match get(i) {
        Some(PropertyValue::Float(f)) if f.is_finite() => *f,
        _ => fallback,
    };
    let boolean = |i: usize, fallback: bool| get(i).and_then(value_bool).unwrap_or(fallback);
    let intensity = match get(3) {
        Some(PropertyValue::Waveform(w)) => {
            waveform_curve(w).unwrap_or_else(|| stored.intensity.clone())
        }
        _ => stored.intensity.clone(),
    };
    let specular_stored = matches!(get(4), Some(PropertyValue::Colour(_)));
    let params = LightParams {
        diffuse: colour(0, stored.diffuse),
        intensity,
        range: float(1, stored.range),
        falloff: float(2, stored.falloff),
        specular: colour(4, stored.specular),
        specular_stored,
        key_light: boolean(5, stored.key_light),
        ..LightParams::default()
    };
    Some(GlowLight {
        params,
        shadow_caster: boolean(6, stored.shadow_caster),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tessera_formats::script_table::{BaseBlock, Port, PortType, SwitchNode, WhenTest};
    use tessera_formats::{Did, PropertyEntry, Waveform};

    use super::*;

    const TIME: u32 = 0x682;

    fn when(value: f32, op: i32) -> Node {
        Node {
            kind: NodeKind::WHEN,
            data: NodeData::Switch(SwitchNode {
                property: TIME,
                persistent: true,
                autonomous: false,
                synchronized: false,
            }),
            base: BaseBlock::default(),
            particle_flag: None,
            ports: vec![Port {
                port_type: PortType::WHEN_TEST,
                payload: PortPayload::WhenTest(Box::new(WhenTest {
                    constant: Some(PropertyEntry {
                        pid: TIME,
                        value: PropertyValue::Float(value),
                    }),
                    rhs_property: 0,
                    operator: op,
                    tail: [0, 0],
                })),
                children: Vec::new(),
            }],
            targets: Vec::new(),
        }
    }

    fn scriptlet() -> Scriptlet {
        Scriptlet {
            did: Did(0x0C00_0001),
            roots: vec![
                when(0.25, when_op::LESS_THAN_OR_EQUAL),
                when(0.25, when_op::GREATER_THAN),
                when(0.35, when_op::LESS_THAN_OR_EQUAL),
                when(0.7, when_op::GREATER_THAN),
                when(0.77, when_op::LESS_THAN_OR_EQUAL),
            ],
            dependencies: Vec::new(),
        }
    }

    fn stored() -> GlowStored {
        GlowStored {
            diffuse: [255; 4],
            range: 10.0,
            falloff: 1.0,
            intensity: FloatCurve::Constant(0.5),
            specular: [255; 4],
            key_light: false,
            shadow_caster: false,
        }
    }

    #[test]
    fn worked_example_lamp() {
        // instance-scripts.md §11.2.
        let th = GlowThresholds::from_scriptlet(&scriptlet(), TIME).unwrap();
        assert_eq!(th.0, [0.25, 0.35, 0.7, 0.77]);
        let auto = TimeOverride::from_label("Automatic").unwrap();
        assert_eq!(running_variants(auto, false, 1, 0.10, &th), (false, true));
        assert_eq!(running_variants(auto, false, 1, 0.50, &th), (true, false));
        assert_eq!(running_variants(auto, false, 1, 0.30, &th), (true, true));
        assert_eq!(running_variants(auto, false, 0, 0.10, &th), (false, false));
        assert_eq!(running_variants(auto, true, 0, 0.10, &th), (false, true));
        assert_eq!(
            running_variants(auto, false, 255, 0.10, &th),
            (false, false)
        );
        assert_eq!(
            running_variants(TimeOverride::Always, false, 0, 0.5, &th),
            (false, true)
        );

        let names: HashMap<String, u32> = MEMBERS
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                let i = u32::try_from(i).unwrap();
                [
                    (format!("Light.Script@Entity.Day.{m}"), 100 + i),
                    (format!("Light.Script@Entity.Night.{m}"), 200 + i),
                ]
            })
            .collect();
        let pids = GlowPids::resolve(&names);
        let instance: HashMap<u32, PropertyValue> = [
            (201, PropertyValue::Float(8.5)),
            (200, PropertyValue::Colour([255, 213, 87, 255])),
        ]
        .into_iter()
        .collect();
        let night =
            glow_light(&pids, Variant::Night, |p| instance.get(&p), true, &stored()).unwrap();
        assert!((night.params.range - 8.5).abs() < f32::EPSILON);
        assert!((night.params.diffuse[0] - 87.0 / 255.0).abs() < 1e-6);
        assert!((night.params.falloff - 1.0).abs() < f32::EPSILON);
        assert!(!night.params.key_light && !night.shadow_caster);
        // By day the lamp is off (Day.m_bLightActivated absent, default false) …
        assert!(glow_light(&pids, Variant::Day, |p| instance.get(&p), false, &stored()).is_none());
        // … unless activated: every stored value, range 10.
        let mut on = instance.clone();
        on.insert(107, PropertyValue::Bool(true));
        let day = glow_light(&pids, Variant::Day, |p| on.get(&p), false, &stored()).unwrap();
        assert!((day.params.range - 10.0).abs() < f32::EPSILON);
        assert_eq!(day.params.diffuse, [1.0; 3]);
        // A waveform member drives the intensity.
        on.insert(
            103,
            PropertyValue::Waveform(Box::new(Waveform {
                subtype: 1,
                values: vec![0.8],
                extra: None,
            })),
        );
        let day = glow_light(&pids, Variant::Day, |p| on.get(&p), false, &stored()).unwrap();
        assert_eq!(day.params.intensity, FloatCurve::Constant(0.8));
    }

    fn light(range: f32, diffuse: [u8; 4]) -> LightNode {
        let wave = |subtype: u32, values: Vec<f32>| Waveform {
            subtype,
            values,
            extra: None,
        };
        LightNode {
            direction: [0.0; 3],
            range,
            diffuse,
            diffuse_intensity: wave(1, vec![0.5]),
            specular: [255; 4],
            falloff: 1.0,
            projector_enabled: false,
            projector_texture: Did(0),
            projector_yaw: wave(0, Vec::new()),
            projector_pitch: wave(0, Vec::new()),
            projector_roll: wave(0, Vec::new()),
            corona_enabled: false,
            corona_texture: Did(0),
            corona_fade_speed: 0.0,
            corona_size: 0.0,
            corona_opacity: 0.0,
            corona_occlusion: false,
            corona_occlusion_size: 0.0,
            key_light: false,
            shadow_caster: true,
            allow_baked_shadows: true,
            lifespan: 0.0,
            fade_time: 0.25,
            offset: [0.0; 3],
            run_on_local_player_only: false,
            only_influence_parent: false,
            never_influence_parent: false,
            never_cull_or_occlude: false,
            global_light: false,
            holding_location: 0,
        }
    }

    fn bare(kind: NodeKind, data: NodeData, children: Vec<Node>) -> Node {
        Node {
            kind,
            data,
            base: BaseBlock::default(),
            particle_flag: None,
            ports: vec![Port {
                port_type: PortType::SWITCH_VALUE,
                payload: PortPayload::None,
                children,
            }],
            targets: Vec::new(),
        }
    }

    #[test]
    fn light_nodes_are_read_from_the_table() {
        let pids = GlowPids {
            day: [None, None, None, None, None, None, None, Some(107)],
            night: [None, None, None, None, None, None, None, Some(207)],
            ..GlowPids::default()
        };
        let switch = |property: u32, l: LightNode| {
            bare(
                NodeKind::SWITCH,
                NodeData::Switch(SwitchNode {
                    property,
                    persistent: true,
                    autonomous: false,
                    synchronized: false,
                }),
                vec![bare(
                    NodeKind::LIGHT,
                    NodeData::Light(Box::new(l)),
                    Vec::new(),
                )],
            )
        };
        let table = ScriptTable {
            did: Did(0x0700_0001),
            kind: 1,
            includes: Vec::new(),
            scripts_size: tessera_formats::TSize::default(),
            scripts: vec![
                tessera_formats::Script {
                    key: 0x1000_0001,
                    roots: vec![switch(207, light(10.0, [1, 2, 3, 255]))],
                },
                tessera_formats::Script {
                    key: 0x1000_0002,
                    roots: vec![switch(107, light(12.0, [255; 4]))],
                },
                tessera_formats::Script {
                    key: 0x1000_0003,
                    roots: vec![switch(999, light(99.0, [0; 4]))],
                },
            ],
            properties_size: tessera_formats::TSize::default(),
            script_properties: Vec::new(),
            dids_size: tessera_formats::TSize::default(),
            script_dids: Vec::new(),
        };
        let t = GlowTableLights::from_table(&table, &pids);
        let night = t.stored(Variant::Night).unwrap();
        assert_eq!(night.range, 10.0);
        assert_eq!(night.diffuse, [1, 2, 3, 255]);
        assert_eq!(night.intensity, FloatCurve::Constant(0.5));
        assert!(night.shadow_caster && !night.key_light);
        assert_eq!(t.stored(Variant::Day).unwrap().range, 12.0);
        assert_eq!(t.night_node.as_ref().unwrap().fade_time, 0.25);
        // A table without the switches has no stored fields.
        let none = GlowTableLights::from_table(
            &ScriptTable {
                scripts: Vec::new(),
                ..table
            },
            &pids,
        );
        assert_eq!(none, GlowTableLights::default());
    }

    #[test]
    fn thresholds_need_the_shape() {
        let mut s = scriptlet();
        s.roots.pop();
        assert!(GlowThresholds::from_scriptlet(&s, TIME).is_none());
        let mut s = scriptlet();
        s.roots.push(when(0.9, when_op::EQUAL));
        assert!(GlowThresholds::from_scriptlet(&s, TIME).is_none());
        assert!(GlowThresholds::from_scriptlet(&scriptlet(), TIME + 1).is_none());
    }
}
