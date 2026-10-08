// SPDX-License-Identifier: GPL-3.0-or-later
//! The figure's shape inputs from its species table: held proportion clips, body-shape
//! deformer clips and bone sliders (`appearance.md` §6.2–§6.4, `script-table.md`
//! §6.7–§6.9).
//!
//! Clip selection ([`tessera_formats::script_table::resolve`]) reports the held animation
//! nodes and the deformer nodes the avatar's `Startup` scripts register, with the entity's
//! driven values. [`figure_shape`] combines them with the figure's own height and slider
//! values ([`AssembledCharacter::sliders`]) into plain data that `tessera-anim` turns into
//! held playbacks and a bind-space deformation. The explorer and the client share it, so
//! both show the same figure.

use tessera_formats::avatar::{
    ABSENT_SLIDER_STRING, BoneSliderDefinition, BoneSliderPids, slider_positions,
    slider_value_pieces,
};
use tessera_formats::script_table::resolve::{ClipResolution, DeformerCandidate};
use tessera_formats::{Did, PropertySetRecord};
use tessera_props::{EnumSource, GameData, RecordSource};

use super::{AssembledCharacter, AssemblyVocabulary};

/// Slider channel bits in Tessera's order: the layout of `tessera_anim::SliderChannels`
/// (`SliderChannels::from_bits` takes these bits).
pub mod channel {
    /// Translation along local x.
    pub const TRANSLATE_X: u16 = 1 << 0;
    /// Translation along local y.
    pub const TRANSLATE_Y: u16 = 1 << 1;
    /// Translation along local z.
    pub const TRANSLATE_Z: u16 = 1 << 2;
    /// The `RotateX` channel.
    pub const ROTATE_X: u16 = 1 << 3;
    /// The `RotateY` channel.
    pub const ROTATE_Y: u16 = 1 << 4;
    /// The `RotateZ` channel.
    pub const ROTATE_Z: u16 = 1 << 5;
    /// Scale along local x.
    pub const SCALE_X: u16 = 1 << 6;
    /// Scale along local y.
    pub const SCALE_Y: u16 = 1 << 7;
    /// Scale along local z.
    pub const SCALE_Z: u16 = 1 << 8;

    /// The bit of a `BoneSlider_BoneTranformMaskArray` label (`appearance.md` §6.3).
    #[must_use]
    pub fn of_label(label: &str) -> Option<u16> {
        Some(match label {
            "TranslateX" => TRANSLATE_X,
            "TranslateY" => TRANSLATE_Y,
            "TranslateZ" => TRANSLATE_Z,
            "RotateX" => ROTATE_X,
            "RotateY" => ROTATE_Y,
            "RotateZ" => ROTATE_Z,
            "ScaleX" => SCALE_X,
            "ScaleY" => SCALE_Y,
            "ScaleZ" => SCALE_Z,
            _ => return None,
        })
    }
}

/// One slider of a definition record with its channels resolved (`appearance.md` §6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct SliderSpec {
    /// `BoneSlider_Name` enum key.
    pub name: u32,
    /// `BoneSlider_SliderGroup` enum key.
    pub group: u32,
    /// Shown in the character-creation UI.
    pub visible: bool,
    /// Channels driven ([`channel`] bits).
    pub channels: u16,
    /// Channel values at *t* = 0.
    pub min: [f32; 3],
    /// Channel values at *t* = 1.
    pub max: [f32; 3],
    /// Bones (indices into the setup's skeleton) and their reverse flag.
    pub bones: Vec<(i64, bool)>,
}

/// A slider definition record read for a figure.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SliderTable {
    /// The record.
    pub record: Did,
    /// Sliders in definition order.
    pub sliders: Vec<SliderSpec>,
    /// Channel bits without a known label (ignored).
    pub unknown_channel_bits: usize,
}

/// Most sliders read from one record (the data has 35–37).
pub const MAX_SLIDERS: usize = 512;

/// Reads the slider definitions of record `record` and resolves each mask bit to a channel
/// through the labels of the mask array's enum mapper (key = bit + 1, `appearance.md` §6.3).
///
/// # Errors
///
/// A message when the record is missing or undecodable or the slider properties are
/// unknown to the dictionary.
pub fn read_slider_table<S: RecordSource>(
    data: &GameData<S>,
    record: Did,
) -> Result<SliderTable, String> {
    let registry = data.registry().map_err(|e| e.to_string())?;
    let dict = registry.dictionary();
    let pids = BoneSliderPids::from_names(dict).map_err(|e| format!("slider properties: {e}"))?;
    let set = match data.read(record) {
        Ok(Some(b)) => {
            PropertySetRecord::parse_expecting(&b, record, registry.as_ref())
                .map_err(|e| format!("slider record {record}: {e}"))?
                .set
        }
        Ok(None) => return Err(format!("slider record {record} is missing")),
        Err(e) => return Err(format!("slider record {record}: {e}")),
    };
    let defs = BoneSliderDefinition::read_all(&set, &pids);
    // The channel labels: the mapper of the mask array's element (or of the array).
    let mapper = pids.masks.and_then(|m| {
        registry
            .children(m)
            .iter()
            .chain(std::iter::once(&m))
            .find_map(|&p| match registry.enum_source(p)? {
                EnumSource::Mapper(d) => Some(d),
                EnumSource::DidMap(_) => None,
            })
    });
    let mut labels: [Option<u16>; 64] = [None; 64];
    if let Some(m) = mapper {
        for (b, slot) in labels.iter_mut().enumerate() {
            *slot = label_of(data, m, b as u32 + 1).and_then(|l| channel::of_label(&l));
        }
    }
    let mut out = SliderTable {
        record,
        ..SliderTable::default()
    };
    for d in defs.iter().take(MAX_SLIDERS) {
        let bits = d.channel_bits();
        let mut channels = 0u16;
        for (b, label) in labels.iter().enumerate() {
            if bits & (1u64 << b) != 0 {
                match label {
                    Some(c) => channels |= c,
                    None => out.unknown_channel_bits += 1,
                }
            }
        }
        out.sliders.push(SliderSpec {
            name: d.name,
            group: d.group,
            visible: d.visible,
            channels,
            min: d.limit_min,
            max: d.limit_max,
            bones: d
                .bones
                .iter()
                .map(|b| (i64::from(b.index), b.reverse))
                .collect(),
        });
    }
    Ok(out)
}

/// The label of `key` in enum mapper `mapper` or its base chain (at most 16 links).
fn label_of<S: RecordSource>(data: &GameData<S>, mapper: Did, key: u32) -> Option<String> {
    let mut did = mapper;
    for _ in 0..16 {
        if did.is_null() {
            return None;
        }
        let m = data.enum_mapper(did).ok().flatten()?;
        if let Some(n) = m.name(key) {
            return Some(n.to_owned());
        }
        did = m.base;
    }
    None
}

/// The figure value that drives a shape node's field (`appearance.md` §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeDriver {
    /// `AC_Avatar_Height` (the River-hobbit layer-110 clips).
    AvatarHeight,
    /// `AC_BoneScale_Test` (body-shape deformers, the DwarfStoutAxe layer-110 clip).
    BoneScale,
    /// Another property (its PID), read from the entity by the resolver.
    Other(u32),
}

impl ShapeDriver {
    fn of(vocab: &AssemblyVocabulary, source: Option<u32>) -> Option<Self> {
        let source = source?;
        Some(if Some(source) == vocab.avatar_height {
            Self::AvatarHeight
        } else if Some(source) == vocab.bone_scale {
            Self::BoneScale
        } else {
            Self::Other(source)
        })
    }
}

/// A held proportion clip (`script-table.md` §6.7, §6.8).
#[derive(Debug, Clone, PartialEq)]
pub struct HeldClip {
    /// The clip.
    pub clip: Did,
    /// The data's animation layer.
    pub layer: u32,
    /// The held fraction *f* of §6.7 step 1 for the figure's driving value (`0 ..= 1`).
    pub fraction: f32,
    /// Seconds cut from the end of the clip's played length (§6.7 step 2: 1/30 s for
    /// `LoopForever` 0 with `LoopCount` 1, else 0). The clip time is *f* × (duration −
    /// `end_crop`), limited to that length
    /// ([`tessera_formats::script_table::resolve::held_clip_time`]).
    pub end_crop: f32,
    /// `UseBoneOpacity`.
    pub use_bone_opacity: bool,
    /// What drives `ManualTime` (`None`: the node's own value).
    pub driver: Option<ShapeDriver>,
}

/// A body-shape deformer clip (`script-table.md` §6.9 kind 18).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyShapeClip {
    /// The clip.
    pub clip: Did,
    /// The time fraction (the figure's `AC_BoneScale_Test` where it drives the node; the
    /// deformation limits it to [0, 1]).
    pub fraction: f32,
    /// `…_UseBoneWeights`.
    pub use_bone_weights: bool,
}

/// The figure's sliders: the node's definitions and the position of every slider.
#[derive(Debug, Clone, PartialEq)]
pub struct FigureSliders {
    /// The definitions record the SliderMeshDeformer node names.
    pub table: SliderTable,
    /// The value pieces in use (`appearance.md` §6.3, A16, A29; see [`slider_values`]).
    pub values: Vec<u32>,
    /// Position *t* of every slider of `table`, in definition order: piece / 255, or 0.5
    /// for a slider without a piece.
    pub positions: Vec<f32>,
}

/// Everything that shapes a figure beyond its parts and clips.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FigureShape {
    /// Held clips, all of them (several may share a layer, `script-table.md` §6.2), in
    /// layer order (`script-table.md` §6.7, §6.8).
    pub held: Vec<HeldClip>,
    /// Body-shape deformer clips (`appearance.md` §6.4).
    pub body_shape: Vec<BodyShapeClip>,
    /// Bone sliders, for the Human, Elf, HighElf and River-hobbit bodies.
    pub sliders: Option<FigureSliders>,
    /// Problems, for the log.
    pub notes: Vec<String>,
}

/// The value of a figure property that drives a node field: the figure's own height or
/// body-shape value (request or entity), when `source` is that property.
fn figure_value(
    vocab: &AssemblyVocabulary,
    c: &AssembledCharacter,
    source: Option<u32>,
) -> Option<f32> {
    match ShapeDriver::of(vocab, source)? {
        ShapeDriver::AvatarHeight => c.sliders.avatar_height,
        ShapeDriver::BoneScale => c.sliders.bone_scale,
        ShapeDriver::Other(_) => None,
    }
}

/// The value pieces of a SliderMeshDeformer node (`appearance.md` §6.3, A16, A29;
/// `script-table.md` §6.9: the node parses whatever string its field holds):
///
/// - the node's string is **not driven** (`values_source` `None`): its stored string
///   (`node_values`), whatever the figure carries;
/// - driven by **`AC_BoneSlider_Value_Array`** (`value_array`): the figure's own pieces
///   (`figure_values`, from the request's or else the entity's string) when it has any,
///   else the resolved driven string;
/// - driven by **another property**: the resolved driven string;
/// - a driven string the entity lacks ([`DeformerCandidate::Slider::values`] `None`):
///   [`ABSENT_SLIDER_STRING`] (the §6.3 Decision while the absent-property behaviour is
///   OPEN).
///
/// Strings are cut into pieces by [`slider_value_pieces`].
#[must_use]
pub fn slider_values(
    values_source: Option<u32>,
    node_values: Option<&str>,
    figure_values: &[u32],
    value_array: Option<u32>,
) -> Vec<u32> {
    let figure_driven = values_source.is_some() && values_source == value_array;
    if figure_driven && !figure_values.is_empty() {
        return figure_values.to_vec();
    }
    slider_value_pieces(node_values.unwrap_or(ABSENT_SLIDER_STRING))
}

/// Most body-shape deformer clips kept per figure: a Tessera bound, not a client rule
/// (`script-table.md` §6.9: the client keeps any number; the install has one per figure).
/// A deformer beyond it is dropped and logged.
pub const MAX_BODY_SHAPE_CLIPS: usize = 16;

/// Collects the shape inputs of figure `c` from a clip resolution of its scripts (any
/// motion state: the species table's `Startup` registers them in every state).
///
/// - **Held clips**: every one of [`ClipResolution::held`]; the driving value is the
///   figure's height or body-shape value when the node is driven by it, else the resolved
///   `ManualTime`; the fraction follows §6.7 step 1
///   ([`tessera_formats::script_table::resolve::ClipCandidate::held_fraction_at`]) and the
///   end crop §6.7 step 2.
/// - **Body shape**: every MeshDeformer (at most [`MAX_BODY_SHAPE_CLIPS`]); its fraction
///   likewise.
/// - **Sliders**: the entity holds one slider set, that of the **most recently started**
///   SliderMeshDeformer (`script-table.md` §6.9, `appearance.md` §6.4): the last one in
///   evaluation order. Its definitions record; the pieces by [`slider_values`].
pub fn figure_shape<S: RecordSource>(
    data: &GameData<S>,
    vocab: &AssemblyVocabulary,
    c: &AssembledCharacter,
    resolution: &ClipResolution,
) -> FigureShape {
    let mut out = FigureShape::default();
    for h in resolution.held() {
        let value = figure_value(vocab, c, h.manual_time_source).unwrap_or(h.manual_time);
        out.held.push(HeldClip {
            clip: h.clip,
            layer: h.layer,
            fraction: h.held_fraction_at(value),
            end_crop: h.held_end_crop(),
            use_bone_opacity: h.use_bone_opacity,
            driver: ShapeDriver::of(vocab, h.manual_time_source),
        });
    }
    let mut slider_node = None;
    let mut dropped_deformers = 0usize;
    let mut replaced_slider_sets = 0usize;
    for d in &resolution.deformers {
        match d {
            DeformerCandidate::Mesh {
                clip,
                time,
                time_source,
                use_bone_weights,
                ..
            } => {
                if out.body_shape.len() < MAX_BODY_SHAPE_CLIPS {
                    out.body_shape.push(BodyShapeClip {
                        clip: *clip,
                        fraction: figure_value(vocab, c, *time_source).unwrap_or(*time),
                        use_bone_weights: *use_bone_weights,
                    });
                } else {
                    dropped_deformers += 1;
                }
            }
            DeformerCandidate::Slider {
                definitions,
                values,
                values_source,
                ..
            } => {
                if slider_node.is_some() {
                    replaced_slider_sets += 1;
                }
                slider_node = Some((*definitions, values.as_deref(), *values_source));
            }
            _ => {}
        }
    }
    if dropped_deformers > 0 {
        tracing::warn!(
            dropped = dropped_deformers,
            kept = MAX_BODY_SHAPE_CLIPS,
            "body-shape deformers beyond the Tessera bound dropped"
        );
        out.notes.push(format!(
            "{dropped_deformers} body-shape deformers beyond {MAX_BODY_SHAPE_CLIPS} dropped"
        ));
    }
    if replaced_slider_sets > 0 {
        out.notes.push(format!(
            "{} slider deformers; the most recently started one's set is used",
            replaced_slider_sets + 1
        ));
    }
    if let Some((record, node_values, values_source)) = slider_node {
        match read_slider_table(data, record) {
            Ok(table) => {
                if table.unknown_channel_bits > 0 {
                    out.notes.push(format!(
                        "{} slider channel bits have no known label; ignored",
                        table.unknown_channel_bits
                    ));
                }
                let values = slider_values(
                    values_source,
                    node_values,
                    &c.sliders.values,
                    vocab.slider_values,
                );
                let positions = slider_positions(table.sliders.len(), &values);
                out.sliders = Some(FigureSliders {
                    table,
                    values,
                    positions,
                });
            }
            Err(e) => out.notes.push(e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_labels() {
        assert_eq!(channel::of_label("ScaleZ"), Some(channel::SCALE_Z));
        assert_eq!(channel::of_label("RotateX"), Some(1 << 3));
        assert_eq!(channel::of_label("Bogus"), None);
    }

    /// The precedence of `slider_values` (`script-table.md` §6.9, `appearance.md` §6.3):
    /// the figure's pieces only for a string driven by `AC_BoneSlider_Value_Array`; strings
    /// cut by the client's rule (the empty piece after an even-length string).
    #[test]
    fn slider_value_precedence() {
        const ARRAY: u32 = 0x1000_0001;
        const OTHER: u32 = 0x1000_0002;
        let figure = [0x10, 0x20, 0];
        let absent = slider_value_pieces(&"80".repeat(35));
        // Driven by the value array: the figure's pieces win over the resolved string.
        assert_eq!(
            slider_values(Some(ARRAY), Some("8080"), &figure, Some(ARRAY)),
            vec![0x10, 0x20, 0]
        );
        // ... else the resolved driven string, else the absent-property Decision.
        assert_eq!(
            slider_values(Some(ARRAY), Some("80ff"), &[], Some(ARRAY)),
            vec![0x80, 0xFF, 0]
        );
        assert_eq!(slider_values(Some(ARRAY), None, &[], Some(ARRAY)), absent);
        assert_eq!(absent.len(), 36);
        assert!(absent[..35].iter().all(|&b| b == 0x80));
        // Not driven: the node's stored string, even with figure pieces.
        assert_eq!(
            slider_values(None, Some("00ff"), &figure, Some(ARRAY)),
            vec![0x00, 0xFF, 0]
        );
        // Driven by another property: its resolved value, never the figure's pieces.
        assert_eq!(
            slider_values(Some(OTHER), Some("01"), &figure, Some(ARRAY)),
            vec![0x01, 0]
        );
        assert_eq!(
            slider_values(Some(OTHER), None, &figure, Some(ARRAY)),
            absent
        );
        // Without the value-array PID in the dictionary nothing is the figure's.
        assert_eq!(
            slider_values(Some(ARRAY), Some("02"), &figure, None),
            vec![0x02, 0]
        );
        // An odd-length string ends with a one-character piece and no empty piece.
        assert_eq!(slider_values(None, Some("8z1"), &[], None), vec![8, 1]);
        // Positions of the pieces in use (A16): hidden sliders included, 0 for the empty
        // piece, 0.5 past the end.
        assert_eq!(
            slider_positions(4, &slider_values(None, Some("ff00"), &[], None)),
            vec![1.0, 0.0, 0.0, 0.5]
        );
    }
}
