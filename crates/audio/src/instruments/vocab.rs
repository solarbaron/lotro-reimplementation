// SPDX-License-Identifier: GPL-3.0-or-later
//! The property IDs, enum keys and labels player music needs, looked up by name in the
//! user's data (`instruments.md` §3).

use std::collections::HashSet;

use tessera_formats::{Did, EnumMapper, PropertyDictionary, PropertyType};

use super::InstrumentError;
use crate::abc::{PITCH_MAX, PITCH_MIN, note_label};

/// Number of pitches the ABC reader produces (48–84, `instruments.md` §7.1).
pub const PITCH_COUNT: usize = (PITCH_MAX - PITCH_MIN + 1) as usize;

/// Script-key label of the script run for quantized notes (`instruments.md` §3.5, §5.3).
pub const QUANTIZED_SCRIPT_LABEL: &str = "SND_PLAYERMUSIC_PlayMusic_SUS_01";
/// Script-key label of the script run for every other note (§3.5, §5.3).
pub const NORMAL_SCRIPT_LABEL: &str = "SND_PLAYERMUSIC_PlayMusic_SUS_02";

/// Longest enum-mapper base chain followed when listing labels.
const MAPPER_CHAIN_LIMIT: usize = 16;

/// What the music module looks up in the data (`instruments.md` §3.1–§3.5, §5.2, §7.1).
///
/// Build it with [`MusicVocabulary::from_data`]; tests and tools may fill the fields
/// directly. Nothing here is a hard-coded PID, DID or key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicVocabulary {
    /// `Music_Note`: the note key of the note being played (§5.2).
    pub music_note: u32,
    /// `Music_Instrument_Type` (§5.2).
    pub music_instrument_type: u32,
    /// `Music_Note_Volume`: velocity / 127 (§5.2).
    pub music_note_volume: u32,
    /// `Music_Note_From_Song` (§5.2).
    pub music_note_from_song: u32,
    /// `Music_Note_Quantized` (§5.2).
    pub music_note_quantized: u32,
    /// `Script_IntensityMultiplier`: the node field a driven multiplier names (§5.6, §10.1).
    pub intensity_multiplier: u32,
    /// `Script_MaxNumInstances`: the per-script instance limit (§8.6), if defined.
    pub max_num_instances: Option<u32>,
    /// Script key of the quantized-note script (label [`QUANTIZED_SCRIPT_LABEL`]).
    pub quantized_script: u32,
    /// Script key of the other script (label [`NORMAL_SCRIPT_LABEL`]).
    pub normal_script: u32,
    /// Note key of each pitch 48–84 by the label rule of §7.1 (`Note_` + name + octave);
    /// 0 (`Undef`) where the note mapper lacks the label.
    pub pitch_keys: [u32; PITCH_COUNT],
    /// Labels of the note mapper (§3.2), key → label.
    pub note_labels: Vec<(u32, String)>,
    /// Labels of the instrument-type mapper (§3.1), key → label.
    pub instrument_labels: Vec<(u32, String)>,
    /// Labels of the sound-channel mapper (§3.3), key → label; empty when the dictionary
    /// has no `SoundScript_Channel`.
    pub channel_labels: Vec<(u32, String)>,
    /// The note parameters among the five of §5.2 whose dictionary definition does not
    /// have the expected type (enum for `Music_Note` and `Music_Instrument_Type`, float for
    /// `Music_Note_Volume`, bool for the two flags) or is missing. The original does not set
    /// such a parameter, so the table treats it as unset ([`super::TableIssue::MistypedParameter`]).
    /// Empty in the reference install.
    pub mistyped: Vec<u32>,
}

impl Default for MusicVocabulary {
    fn default() -> Self {
        Self {
            music_note: 0,
            music_instrument_type: 0,
            music_note_volume: 0,
            music_note_from_song: 0,
            music_note_quantized: 0,
            intensity_multiplier: 0,
            max_num_instances: None,
            quantized_script: 0,
            normal_script: 0,
            pitch_keys: [0; PITCH_COUNT],
            note_labels: Vec::new(),
            instrument_labels: Vec::new(),
            channel_labels: Vec::new(),
            mistyped: Vec::new(),
        }
    }
}

/// The enum mapper a property's definition names (`property-system.md` §2.5).
fn mapper_of(dict: &PropertyDictionary, pid: u32) -> Option<Did> {
    dict.definition(pid)
        .map(|d| d.data)
        .filter(|d| !d.is_null())
}

/// Every label of the mapper chain starting at `start` (first definition of a key wins),
/// bounded and cycle-safe.
fn labels(start: Did, mappers: &mut impl FnMut(Did) -> Option<EnumMapper>) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    let mut keys = HashSet::new();
    let mut seen = HashSet::new();
    let mut did = start;
    for _ in 0..MAPPER_CHAIN_LIMIT {
        if did.is_null() || !seen.insert(did) {
            break;
        }
        let Some(m) = mappers(did) else { break };
        for (k, n) in m.names() {
            if keys.insert(*k) {
                out.push((*k, n.clone()));
            }
        }
        did = m.base;
    }
    out.sort_by_key(|(k, _)| *k);
    out
}

impl MusicVocabulary {
    /// Looks every name up in the property dictionary and every label in the enum mapper
    /// that the property's definition names (`mappers` loads a mapper by DID).
    ///
    /// # Errors
    ///
    /// A note parameter whose definition has another type than §5.2 expects is not an
    /// error: it is listed in [`Self::mistyped`].
    ///
    /// # Errors
    ///
    /// [`InstrumentError::UnknownName`] when one of the five note parameters,
    /// `Script_IntensityMultiplier`, `ScriptTable_ScriptID` or one of the two script-key
    /// labels is missing.
    pub fn from_data(
        dict: &PropertyDictionary,
        mut mappers: impl FnMut(Did) -> Option<EnumMapper>,
    ) -> Result<Self, InstrumentError> {
        let need = |name: &'static str| {
            dict.pid(name).ok_or(InstrumentError::UnknownName {
                what: "property",
                name,
            })
        };
        let music_note = need("Music_Note")?;
        let music_instrument_type = need("Music_Instrument_Type")?;
        let script_id = need("ScriptTable_ScriptID")?;
        let script_mapper = mapper_of(dict, script_id).ok_or(InstrumentError::UnknownName {
            what: "enum mapper of",
            name: "ScriptTable_ScriptID",
        })?;
        let mut script_key = |label: &'static str| {
            EnumMapper::key_in_chain_from(script_mapper, &mut mappers, label).ok_or(
                InstrumentError::UnknownName {
                    what: "script-key label",
                    name: label,
                },
            )
        };
        let quantized_script = script_key(QUANTIZED_SCRIPT_LABEL)?;
        let normal_script = script_key(NORMAL_SCRIPT_LABEL)?;
        let note_labels = mapper_of(dict, music_note)
            .map(|m| labels(m, &mut mappers))
            .unwrap_or_default();
        let instrument_labels = mapper_of(dict, music_instrument_type)
            .map(|m| labels(m, &mut mappers))
            .unwrap_or_default();
        let channel_labels = dict
            .pid("SoundScript_Channel")
            .and_then(|p| mapper_of(dict, p))
            .map(|m| labels(m, &mut mappers))
            .unwrap_or_default();
        let music_note_volume = need("Music_Note_Volume")?;
        let music_note_from_song = need("Music_Note_From_Song")?;
        let music_note_quantized = need("Music_Note_Quantized")?;
        let mistyped = [
            (music_note, PropertyType::Enum),
            (music_instrument_type, PropertyType::Enum),
            (music_note_volume, PropertyType::Float),
            (music_note_from_song, PropertyType::Bool),
            (music_note_quantized, PropertyType::Bool),
        ]
        .into_iter()
        .filter(|&(pid, ty)| dict.definition(pid).map(|d| d.ty) != Some(ty))
        .map(|(pid, _)| pid)
        .collect();
        let mut v = Self {
            music_note,
            music_instrument_type,
            music_note_volume,
            music_note_from_song,
            music_note_quantized,
            intensity_multiplier: need("Script_IntensityMultiplier")?,
            max_num_instances: dict.pid("Script_MaxNumInstances"),
            quantized_script,
            normal_script,
            pitch_keys: [0; PITCH_COUNT],
            note_labels,
            instrument_labels,
            channel_labels,
            mistyped,
        };
        v.derive_pitch_keys();
        Ok(v)
    }

    /// Fills [`Self::pitch_keys`] from [`Self::note_labels`] by the label rule of §7.1.
    pub fn derive_pitch_keys(&mut self) {
        for (i, slot) in self.pitch_keys.iter_mut().enumerate() {
            let pitch = PITCH_MIN + i as u8;
            *slot = note_label(pitch)
                .and_then(|l| self.note_labels.iter().find(|(_, n)| *n == l))
                .map_or(0, |(k, _)| *k);
        }
    }

    /// The note key of an ABC pitch (§7.1): 0 (`Undef`) outside 48–84 or when the label is
    /// missing.
    #[must_use]
    pub fn pitch_key(&self, pitch: u8) -> u32 {
        pitch
            .checked_sub(PITCH_MIN)
            .and_then(|i| self.pitch_keys.get(usize::from(i)))
            .copied()
            .unwrap_or(0)
    }

    /// The label of an instrument type, for tools and logs.
    #[must_use]
    pub fn instrument_label(&self, key: u32) -> Option<&str> {
        find_label(&self.instrument_labels, key)
    }

    /// The label of a note key, for tools and logs.
    #[must_use]
    pub fn note_label(&self, key: u32) -> Option<&str> {
        find_label(&self.note_labels, key)
    }

    /// The label of a sound channel, for tools and logs.
    #[must_use]
    pub fn channel_label(&self, key: u32) -> Option<&str> {
        find_label(&self.channel_labels, key)
    }

    /// The instrument type with this label (the lookup a debug UI or a sandbox command
    /// uses to pick an instrument by name, §12).
    #[must_use]
    pub fn instrument_key(&self, label: &str) -> Option<u32> {
        self.instrument_labels
            .iter()
            .find(|(_, n)| n == label)
            .map(|(k, _)| *k)
    }
}

fn find_label(list: &[(u32, String)], key: u32) -> Option<&str> {
    list.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, n)| n.as_str())
}
