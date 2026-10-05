// SPDX-License-Identifier: GPL-3.0-or-later
//! Playable bodies: the race table, the avatar entity of each (species, sex), and the
//! bone-slider definition records (`appearance.md` §2, §3, §6; `skeleton-anim.md` §2.2).
//!
//! - [`RaceTable::read`] walks the race table from its directory object (the
//!   `RaceTableDirectory` label of the `WEENIECONTENT` DID map) to one [`RaceEntry`] per
//!   gender entry: species key, sex key and player-avatar game object.
//! - [`resolve_avatar_body`] follows a player-avatar object (or any 0x47 entity) to its
//!   entity, setup chain, template scale and appearance section, entity scale, appearance
//!   lists and bone-slider record: everything assembly needs to turn (species, sex) into
//!   DIDs. The data has no body-type choice beyond sex (§2.2).
//! - [`BoneSliderDefinition`] reads the 0x78 slider definitions (§6.3);
//!   [`parse_slider_values`] decodes a character's slider byte string.
//!
//! Property names are looked up in the dictionary at run time; nothing hard-codes a PID,
//! a species key or a DID.

use crate::appearance::{AppearanceKeyInfo, AppearanceListPids};
use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{PropertyNames, PropertySchema, PropertyValue};
use crate::property_set::{PropertySet, PropertySetRecord, object_property_set_did};
use crate::record::Record;
use crate::setup::{
    ENTITY_PREFIX, EntityDescriptor, ResolveError, TEMPLATE_PREFIX, VisualTemplate,
    resolve_setup_did,
};

/// Most species objects and gender entries read from a race table (observed: 8 species,
/// 15 entries).
pub const MAX_RACE_ENTRIES: usize = 1024;

fn need<N: PropertyNames + ?Sized>(names: &N, name: &'static str) -> Result<u32> {
    names.pid_of(name).ok_or(FormatError::UnknownName {
        what: "property",
        name,
    })
}

fn as_u32(v: &PropertyValue) -> Option<u32> {
    v.as_bits().and_then(|b| u32::try_from(b).ok())
}

/// Fetches `did`, mapping absence to a dangling reference from `from`.
fn fetch_existing<B, E>(
    fetch: &mut impl FnMut(Did) -> std::result::Result<Option<B>, E>,
    did: Did,
    from: Did,
) -> std::result::Result<B, ResolveError<E>> {
    match fetch(did) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(FormatError::DanglingReference {
            from: from.0,
            to: did.0,
        }
        .into()),
        Err(source) => Err(ResolveError::Fetch { did: did.0, source }),
    }
}

fn in_record(did: Did) -> impl FnOnce(FormatError) -> FormatError {
    move |e| FormatError::InRecord {
        did: did.0,
        source: Box::new(e),
    }
}

/// The property set of game object `object` (+0x09000000 rule, `property-system.md` §6.2).
pub(crate) fn object_set<S: PropertySchema + ?Sized, B: AsRef<[u8]>, E>(
    object: Did,
    schema: &S,
    fetch: &mut impl FnMut(Did) -> std::result::Result<Option<B>, E>,
) -> std::result::Result<PropertySet, ResolveError<E>> {
    let set_did = object_property_set_did(object).ok_or_else(|| FormatError::ReferenceChain {
        did: object.0,
        reason: "not a game object (0x70-0x76)".into(),
    })?;
    let bytes = fetch_existing(fetch, set_did, object)?;
    let rec = PropertySetRecord::parse_expecting(bytes.as_ref(), set_did, schema)
        .map_err(in_record(set_did))?;
    Ok(rec.set)
}

/// PIDs of the race-table properties (`skeleton-anim.md` §2.2, `appearance.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RacePids {
    /// `RaceTable_RaceTableList`: the species objects.
    pub race_table_list: u32,
    /// `RaceTable_Race`: the `SpeciesType` key of a species object.
    pub race: u32,
    /// `RaceTable_GenderList`: the gender structs of a species object.
    pub gender_list: u32,
    /// `RaceTable_GenderType`: 0x1000 male, 0x2000 female.
    pub gender_type: u32,
    /// `RaceTable_Gender_PlayerAvatar`: the player-avatar game object.
    pub player_avatar: u32,
    /// `RaceTable_GenderNullAppearance`: the "nothing to show" appearance.
    pub null_appearance: Option<u32>,
    /// `RaceTable_RaceSelect_Entity`: the race-selection entity (not the play body).
    pub race_select_entity: Option<u32>,
    /// `RaceTable_IsTallRace`.
    pub is_tall_race: Option<u32>,
}

impl RacePids {
    /// Looks the names up.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownName`] when one of the five structural names is missing.
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Result<Self> {
        Ok(Self {
            race_table_list: need(names, "RaceTable_RaceTableList")?,
            race: need(names, "RaceTable_Race")?,
            gender_list: need(names, "RaceTable_GenderList")?,
            gender_type: need(names, "RaceTable_GenderType")?,
            player_avatar: need(names, "RaceTable_Gender_PlayerAvatar")?,
            null_appearance: names.pid_of("RaceTable_GenderNullAppearance"),
            race_select_entity: names.pid_of("RaceTable_RaceSelect_Entity"),
            is_tall_race: names.pid_of("RaceTable_IsTallRace"),
        })
    }
}

/// One gender entry of the race table: a playable (species, sex) combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaceEntry {
    /// The species game object.
    pub species_object: Did,
    /// `RaceTable_Race`: a `SpeciesType` key.
    pub species: u32,
    /// `RaceTable_GenderType`: 0x1000 male, 0x2000 female (in observed data).
    pub sex: u32,
    /// `RaceTable_Gender_PlayerAvatar`: the player-avatar game object.
    pub player_avatar: Did,
    /// `RaceTable_GenderNullAppearance`.
    pub null_appearance: Option<Did>,
    /// `RaceTable_RaceSelect_Entity`.
    pub race_select_entity: Option<Did>,
    /// `RaceTable_IsTallRace` of the species object.
    pub tall_race: Option<bool>,
}

/// The race table: every playable (species, sex) and its avatar object.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RaceTable {
    /// The directory game object the table was read from.
    pub directory: Did,
    /// Gender entries in stored order (species order, then gender order). Two entries may
    /// name the same avatar object (DwarfStoutAxe, §2.2).
    pub entries: Vec<RaceEntry>,
}

impl RaceTable {
    /// Reads the table from its directory game object (`skeleton-anim.md` §2.2 steps 1–2).
    /// Entries without a player avatar are skipped.
    ///
    /// # Errors
    ///
    /// [`ResolveError::Fetch`] when `fetch` fails; [`FormatError::DanglingReference`] /
    /// [`FormatError::InRecord`] for a missing or undecodable property set;
    /// [`FormatError::CountLimit`] past [`MAX_RACE_ENTRIES`].
    pub fn read<S, B, E>(
        directory: Did,
        schema: &S,
        pids: &RacePids,
        mut fetch: impl FnMut(Did) -> std::result::Result<Option<B>, E>,
    ) -> std::result::Result<Self, ResolveError<E>>
    where
        S: PropertySchema + ?Sized,
        B: AsRef<[u8]>,
    {
        let dir = object_set(directory, schema, &mut fetch)?;
        let species: Vec<Did> = dir
            .get(pids.race_table_list)
            .map(|v| {
                v.children()
                    .iter()
                    .filter_map(|e| e.value.as_did())
                    .collect()
            })
            .unwrap_or_default();
        let limit = |n: usize| -> std::result::Result<(), ResolveError<E>> {
            if n > MAX_RACE_ENTRIES {
                return Err(FormatError::CountLimit {
                    what: "race-table entries",
                    offset: 0,
                    count: n as u64,
                    limit: MAX_RACE_ENTRIES as u64,
                }
                .into());
            }
            Ok(())
        };
        limit(species.len())?;
        let mut entries = Vec::new();
        for obj in species {
            let set = object_set(obj, schema, &mut fetch)?;
            let Some(race) = set.get(pids.race).and_then(as_u32) else {
                continue;
            };
            let tall_race = pids
                .is_tall_race
                .and_then(|p| set.get(p))
                .and_then(PropertyValue::as_f64)
                .map(|v| v != 0.0);
            for g in set
                .get(pids.gender_list)
                .map(PropertyValue::children)
                .unwrap_or(&[])
            {
                let Some(s) = g.value.as_struct() else {
                    continue;
                };
                let did = |p: Option<u32>| p.and_then(|p| s.get(p)).and_then(PropertyValue::as_did);
                let Some(avatar) = did(Some(pids.player_avatar)).filter(|d| !d.is_null()) else {
                    continue;
                };
                entries.push(RaceEntry {
                    species_object: obj,
                    species: race,
                    sex: s.get(pids.gender_type).and_then(as_u32).unwrap_or(0),
                    player_avatar: avatar,
                    null_appearance: did(pids.null_appearance),
                    race_select_entity: did(pids.race_select_entity),
                    tall_race,
                });
                limit(entries.len())?;
            }
        }
        Ok(Self { directory, entries })
    }

    /// The entry for (`species`, `sex`), the first one if several match.
    #[must_use]
    pub fn entry(&self, species: u32, sex: u32) -> Option<&RaceEntry> {
        self.entries
            .iter()
            .find(|e| e.species == species && e.sex == sex)
    }

    /// The distinct player-avatar objects, in first-seen order (14 on the reference
    /// install: the exit-criterion list of §2.2).
    #[must_use]
    pub fn avatars(&self) -> Vec<Did> {
        let mut out: Vec<Did> = Vec::new();
        for e in &self.entries {
            if !out.contains(&e.player_avatar) {
                out.push(e.player_avatar);
            }
        }
        out
    }
}

/// A whole-body scale value: a float, or a vector whose three components are equal (the
/// form `Render_AdjustableScale` is stored in on the reference install). `None` for other
/// values, non-finite or non-positive scales.
#[must_use]
pub fn uniform_scale(v: &PropertyValue) -> Option<f32> {
    let s = match v {
        PropertyValue::Float(f) => *f,
        PropertyValue::Vector3([x, y, z]) if x == y && y == z => *x,
        _ => return None,
    };
    (s.is_finite() && s > 0.0).then_some(s)
}

/// PIDs that [`resolve_avatar_body`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AvatarPids {
    /// `PhysObj`: the game object's 0x47 entity.
    pub phys_obj: u32,
    /// `Render_AdjustableScale`: the entity's whole-body scale (§6.1), stored as a uniform
    /// vector ([`uniform_scale`]).
    pub render_scale: Option<u32>,
    /// The appearance-list properties (§3.2).
    pub lists: AppearanceListPids,
    /// `AppearanceUI_BoneSliderDefinitionsFile` (inside the character-creation controls).
    pub slider_definitions: Option<u32>,
    /// `AC_BoneSlider_Value_Array`: the slider byte string.
    pub slider_values: Option<u32>,
}

impl AvatarPids {
    /// Looks the names up.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownName`] when `PhysObj` or a required appearance-list name is
    /// missing.
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Result<Self> {
        Ok(Self {
            phys_obj: need(names, "PhysObj")?,
            render_scale: names.pid_of("Render_AdjustableScale"),
            lists: AppearanceListPids::from_names(names)?,
            slider_definitions: names.pid_of("AppearanceUI_BoneSliderDefinitionsFile"),
            slider_values: names.pid_of("AC_BoneSlider_Value_Array"),
        })
    }
}

/// Everything about a body that assembly needs, as DIDs and values (`appearance.md` §8).
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarBody {
    /// The game object the resolution started from, if it was one.
    pub object: Option<Did>,
    /// The avatar entity (0x47).
    pub entity: Did,
    /// The entity chain to the setup (`setup.md` §3.1), the entity first and the setup last.
    pub chain: Vec<Did>,
    /// The setup: skeleton and default body (`appearance.md` §3.1).
    pub setup: Did,
    /// The first visual template's uniform `vec3` scale on the chain (§6.1).
    pub template_scale: Option<f32>,
    /// The entity's `Render_AdjustableScale` (§6.1).
    pub entity_scale: Option<f32>,
    /// Appearance sections of the chain's templates, as list elements (§7.4 step 2).
    pub template_appearance: Vec<AppearanceKeyInfo>,
    /// The entity's `Appearance_ClassList` (§3.2, §7.4 step 3).
    pub class_list: Vec<AppearanceKeyInfo>,
    /// The entity's own `Appearance_InstanceList` (§7.4 step 4). A placed figure whose
    /// instance set has its own list uses that list instead
    /// ([`AvatarBody::appearance_files_with_instance`]).
    pub instance_list: Vec<AppearanceKeyInfo>,
    /// The entity's property set (for toggle properties and `AC_*` values).
    pub properties: PropertySet,
    /// The bone-slider definition record (0x78) named by the entity's controls (§6.3).
    pub bone_slider_definitions: Option<Did>,
    /// The entity's default slider byte string (`AC_BoneSlider_Value_Array`).
    pub slider_values: Option<String>,
}

impl AvatarBody {
    /// Model scale (§6.1 Decision): template scale (1 if absent) × entity scale (1 if
    /// absent). Multiply by an instance's own `Render_AdjustableScale` where there is one.
    #[must_use]
    pub fn model_scale(&self) -> f32 {
        self.template_scale.unwrap_or(1.0) * self.entity_scale.unwrap_or(1.0)
    }

    /// The appearance elements to apply, in the order of §7.4 and §8 step 3: template
    /// sections, the class list, then the entity's instance list; each with the record
    /// chosen by the toggle rule (§3.3 Decision) against the entity's property set.
    #[must_use]
    pub fn appearance_files(&self) -> Vec<(AppearanceKeyInfo, Did)> {
        self.appearance_files_with_instance(None)
    }

    /// [`AvatarBody::appearance_files`] for a placed figure: when `instance_list` is
    /// `Some` (the instance set's own `Appearance_InstanceList`, read with
    /// [`AppearanceListPids::read_list`]), it **replaces** the entity's instance list
    /// (§7.4 step 4 Decision); `None` keeps the entity's. The toggle rule is evaluated
    /// against the entity's property set, as in `appearance_files`.
    #[must_use]
    pub fn appearance_files_with_instance(
        &self,
        instance_list: Option<&[AppearanceKeyInfo]>,
    ) -> Vec<(AppearanceKeyInfo, Did)> {
        self.template_appearance
            .iter()
            .chain(&self.class_list)
            .chain(instance_list.unwrap_or(&self.instance_list))
            .map(|e| (*e, e.file_in(&self.properties)))
            .collect()
    }
}

/// Resolves a player-avatar game object (0x70–0x76, through its `PhysObj`) or a 0x47
/// entity to an [`AvatarBody`].
///
/// # Errors
///
/// [`ResolveError::Fetch`] when `fetch` fails; the [`crate::resolve_setup_did`] errors for
/// the chain; [`FormatError::DanglingReference`] / [`FormatError::InRecord`] for missing or
/// undecodable records; [`FormatError::ReferenceChain`] when an object has no `PhysObj`.
pub fn resolve_avatar_body<S, B, E>(
    start: Did,
    schema: &S,
    pids: &AvatarPids,
    mut fetch: impl FnMut(Did) -> std::result::Result<Option<B>, E>,
) -> std::result::Result<AvatarBody, ResolveError<E>>
where
    S: PropertySchema + ?Sized,
    B: AsRef<[u8]>,
{
    let (object, entity) = if start.prefix() == ENTITY_PREFIX {
        (None, start)
    } else {
        let set = object_set(start, schema, &mut fetch)?;
        let phys = set
            .get(pids.phys_obj)
            .and_then(PropertyValue::as_did)
            .filter(|d| !d.is_null())
            .ok_or_else(|| FormatError::ReferenceChain {
                did: start.0,
                reason: "game object has no PhysObj".into(),
            })?;
        (Some(start), phys)
    };
    let bytes = fetch_existing(&mut fetch, entity, start)?;
    let desc = EntityDescriptor::parse_expecting(bytes.as_ref(), entity, schema)
        .map_err(in_record(entity))?;
    drop(bytes);
    let chain = resolve_setup_did(entity, &mut fetch)?;
    let setup = *chain.last().unwrap_or(&entity);
    let mut template_scale = None;
    let mut template_appearance = Vec::new();
    for &d in chain.iter().filter(|d| d.prefix() == TEMPLATE_PREFIX) {
        let bytes = fetch_existing(&mut fetch, d, entity)?;
        let t = VisualTemplate::parse_expecting(bytes.as_ref(), d).map_err(in_record(d))?;
        if template_scale.is_none() {
            template_scale = t.uniform_scale();
        }
        if let Some(a) = &t.appearance {
            template_appearance.extend(a.elements());
        }
    }
    let props = desc.properties;
    let entity_scale = pids
        .render_scale
        .and_then(|p| props.get(p))
        .and_then(uniform_scale);
    let mut bone_slider_definitions = None;
    if let Some(pid) = pids.slider_definitions {
        props.walk(|v| {
            if bone_slider_definitions.is_none() && v.entry.pid == pid {
                bone_slider_definitions = v.entry.value.as_did().filter(|d| !d.is_null());
            }
        });
    }
    let slider_values = pids
        .slider_values
        .and_then(|p| props.get(p))
        .and_then(|v| match v {
            PropertyValue::String(s) => Some(s.clone()),
            _ => None,
        });
    Ok(AvatarBody {
        object,
        entity,
        class_list: pids
            .lists
            .class_list
            .map(|l| pids.lists.read_list(&props, l))
            .unwrap_or_default(),
        instance_list: pids
            .lists
            .instance_list
            .map(|l| pids.lists.read_list(&props, l))
            .unwrap_or_default(),
        chain,
        setup,
        template_scale,
        entity_scale,
        template_appearance,
        properties: props,
        bone_slider_definitions,
        slider_values,
    })
}

/// PIDs of the bone-slider definition records (`appearance.md` §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoneSliderPids {
    /// `BoneSliderDeformer_SliderDefinitionArray`.
    pub definitions: u32,
    /// `BoneSlider_Name`.
    pub name: Option<u32>,
    /// `BoneSlider_SliderGroup`.
    pub group: Option<u32>,
    /// `BoneSlider_BoneTranformMaskArray` (the name as stored in the data).
    pub masks: Option<u32>,
    /// `BoneSlider_LimitMin`.
    pub limit_min: u32,
    /// `BoneSlider_LimitMax`.
    pub limit_max: u32,
    /// `BoneSlider_BoneIndexArray`.
    pub bones: u32,
    /// `BoneSlider_BoneIndex`.
    pub bone_index: u32,
    /// `BoneSlider_BoneReverse`.
    pub bone_reverse: Option<u32>,
    /// `BoneSlider_UI_Visible`.
    pub visible: Option<u32>,
}

impl BoneSliderPids {
    /// Looks the names up.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownName`] for a missing structural name.
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Result<Self> {
        Ok(Self {
            definitions: need(names, "BoneSliderDeformer_SliderDefinitionArray")?,
            name: names.pid_of("BoneSlider_Name"),
            group: names.pid_of("BoneSlider_SliderGroup"),
            masks: names.pid_of("BoneSlider_BoneTranformMaskArray"),
            limit_min: need(names, "BoneSlider_LimitMin")?,
            limit_max: need(names, "BoneSlider_LimitMax")?,
            bones: need(names, "BoneSlider_BoneIndexArray")?,
            bone_index: need(names, "BoneSlider_BoneIndex")?,
            bone_reverse: names.pid_of("BoneSlider_BoneReverse"),
            visible: names.pid_of("BoneSlider_UI_Visible"),
        })
    }
}

/// A bone driven by a slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliderBone {
    /// Bone index into the setup's skeleton.
    pub index: i32,
    /// Mirror the value for this bone (`BoneSlider_BoneReverse` = 1).
    pub reverse: bool,
}

/// One slider of a definition record (`BoneSlider_DefinitionStruct`, §6.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoneSliderDefinition {
    /// `BoneSlider_Name` enum key.
    pub name: u32,
    /// `BoneSlider_SliderGroup` enum key.
    pub group: u32,
    /// `BoneSlider_BoneTranformMaskArray`: one bit field per element; bit *i* set means the
    /// channel whose enum key is *i* + 1 (`TranslateX/Y/Z`, `RotateX/Y/Z`, `ScaleX/Y/Z`;
    /// labels from mapper 0x2300043D at run time).
    pub masks: Vec<u64>,
    /// Channel values at byte 0.
    pub limit_min: [f32; 3],
    /// Channel values at byte 255.
    pub limit_max: [f32; 3],
    /// Bones driven.
    pub bones: Vec<SliderBone>,
    /// Whether the slider is shown (and takes a byte of the value string); `false` for the
    /// fixed hidden sliders.
    pub visible: bool,
}

impl BoneSliderDefinition {
    /// Reads every slider of a definition record's property set, in slider order. Elements
    /// that are not structs or lack the limits are skipped.
    #[must_use]
    pub fn read_all(set: &PropertySet, pids: &BoneSliderPids) -> Vec<Self> {
        let vec3 = |v: Option<&PropertyValue>| match v {
            Some(PropertyValue::Vector3(x)) => Some(*x),
            _ => None,
        };
        let mut out = Vec::new();
        for el in set
            .get(pids.definitions)
            .map(PropertyValue::children)
            .unwrap_or(&[])
        {
            let Some(s) = el.value.as_struct() else {
                continue;
            };
            let (Some(limit_min), Some(limit_max)) =
                (vec3(s.get(pids.limit_min)), vec3(s.get(pids.limit_max)))
            else {
                continue;
            };
            let key = |p: Option<u32>| p.and_then(|p| s.get(p)).and_then(as_u32).unwrap_or(0);
            let masks = pids
                .masks
                .and_then(|p| s.get(p))
                .map(|v| {
                    v.children()
                        .iter()
                        .filter_map(|e| e.value.as_bits())
                        .collect()
                })
                .unwrap_or_default();
            let bones = s
                .get(pids.bones)
                .map(PropertyValue::children)
                .unwrap_or(&[])
                .iter()
                .filter_map(|b| {
                    let b = b.value.as_struct()?;
                    let index = b.get(pids.bone_index)?.as_bits()?;
                    #[allow(clippy::cast_possible_truncation)]
                    let index = index as u32 as i32;
                    let reverse = pids
                        .bone_reverse
                        .and_then(|p| b.get(p))
                        .and_then(PropertyValue::as_f64)
                        .is_some_and(|v| v != 0.0);
                    Some(SliderBone { index, reverse })
                })
                .collect();
            let visible = pids
                .visible
                .and_then(|p| s.get(p))
                .and_then(PropertyValue::as_f64)
                .is_none_or(|v| v != 0.0);
            out.push(Self {
                name: key(pids.name),
                group: key(pids.group),
                masks,
                limit_min,
                limit_max,
                bones,
                visible,
            });
        }
        out
    }

    /// The union of the channel bits of all mask elements.
    #[must_use]
    pub fn channel_bits(&self) -> u64 {
        self.masks.iter().fold(0, |a, m| a | m)
    }

    /// The channel values for slider byte `byte` (§6.3 Decision): *t* = byte / 255, value
    /// = min + *t* × (max − min) per component; a slider with min = max always yields that
    /// value. Only the masked components are used by the caller.
    #[must_use]
    pub fn value_for(&self, byte: u8) -> [f32; 3] {
        let t = f32::from(byte) / 255.0;
        std::array::from_fn(|i| {
            let (lo, hi) = (self.limit_min[i], self.limit_max[i]);
            if lo == hi { lo } else { lo + t * (hi - lo) }
        })
    }
}

/// Most bytes [`parse_slider_values`] accepts (observed strings hold 35 bytes).
pub const MAX_SLIDER_BYTES: usize = 4096;

/// Decodes a slider value string (`AC_BoneSlider_Value_Array`, §6.3): two hexadecimal
/// digits per visible slider. `None` for an odd length, a non-hex digit or more than
/// [`MAX_SLIDER_BYTES`] bytes.
#[must_use]
pub fn parse_slider_values(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) || b.len() / 2 > MAX_SLIDER_BYTES {
        return None;
    }
    b.as_chunks::<2>()
        .0
        .iter()
        .map(|&[hi, lo]| {
            let d = |c: u8| char::from(c).to_digit(16);
            u8::try_from(d(hi)? * 16 + d(lo)?).ok()
        })
        .collect()
}

/// Pairs each slider with its byte: visible sliders take the next byte of `values` (0x80,
/// the default, when the string is short); hidden sliders take none and use their fixed
/// value (§6.3).
#[must_use]
pub fn slider_bytes<'a>(
    defs: &'a [BoneSliderDefinition],
    values: &[u8],
) -> Vec<(&'a BoneSliderDefinition, u8)> {
    let mut it = values.iter().copied();
    defs.iter()
        .map(|d| {
            let byte = if d.visible {
                it.next().unwrap_or(0x80)
            } else {
                0x80
            };
            (d, byte)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_values_and_mapping() {
        assert_eq!(parse_slider_values("80ff00"), Some(vec![0x80, 0xFF, 0]));
        assert_eq!(parse_slider_values("8"), None);
        assert_eq!(parse_slider_values("zz"), None);
        assert_eq!(parse_slider_values(""), Some(vec![]));
        let s = BoneSliderDefinition {
            limit_min: [0.9; 3],
            limit_max: [1.1; 3],
            visible: true,
            ..BoneSliderDefinition::default()
        };
        assert_eq!(s.value_for(0xFF), [1.1; 3]);
        assert_eq!(s.value_for(0), [0.9; 3]);
        // §12.3: byte 0x80 gives 0.9 + 0.2 × 128 / 255.
        assert!((s.value_for(0x80)[0] - 1.000_392).abs() < 1e-5);
        let fixed = BoneSliderDefinition {
            limit_min: [1.0, 1.0, 0.8],
            limit_max: [1.0, 1.0, 0.8],
            visible: false,
            ..BoneSliderDefinition::default()
        };
        assert_eq!(fixed.value_for(0x17), [1.0, 1.0, 0.8]);
        let defs = [s.clone(), fixed.clone(), s];
        let b: Vec<u8> = slider_bytes(&defs, &[0x10]).iter().map(|x| x.1).collect();
        assert_eq!(b, vec![0x10, 0x80, 0x80]);
    }
}
