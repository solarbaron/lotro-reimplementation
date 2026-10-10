// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading the creation screens' data from the install (`player-character.md` §2.3, §5–§8;
//! `appearance.md` §2.1, §3.4; `misc-records.md` §9).

use std::collections::BTreeSet;

use tessera_formats::gameplay::common::{Pid, enum_mapper_of};
use tessera_formats::gameplay::misc::NameFilterTable;
use tessera_formats::{Did, EntityDescriptor, Record, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet, RecordSource, Value};
use tessera_wstate::ObjectStore;
use tessera_wstate::character::{
    ClassCreationInfo, ClassPriority, LevelTableDirectory, NameGenerator, NationalityDirectory,
    NationalityRecord, RaceDirectory, RaceRecord,
};

use super::appearance::{AppearancePids, AvatarControls, creation_controls, read_controls};
use super::model::{ChargenData, ClassChoice, GenderChoice, RaceChoice};
use super::names::{NameData, NamePolicy};
use crate::error::{GameDataError, Result};
use crate::vocab::{label_key, mapper_chain};

/// WEENIECONTENT labels read here (`player-character.md` §2.2).
const RACE_DIRECTORY: &str = "RaceTableDirectory";
const NATIONALITY_DIRECTORY: &str = "NationalityTableDirectory";
const LEVEL_TABLE_DIRECTORY: &str = "LevelTableDirectory";
const CLASS_PRIORITY: &str = "ChargenClassPriorityControl";
const NAME_GENERATOR: &str = "RandomNameGenerationDirectory";
/// UNIQUEDB label of the name-filter table (`misc-records.md` §2).
const NAME_FILTER_TABLE: &str = "NameFilterTable";
const UNIQUEDB: &str = "UNIQUEDB";
/// The availability label of controls offered at creation (`appearance.md` §3.4).
const CHARGEN_LABEL: &str = "CharGen";

fn missing(label: &str) -> GameDataError {
    GameDataError::MissingRecord(label.into())
}

fn control<S: RecordSource>(store: &ObjectStore<S>, label: &str) -> Result<Option<PropertySet>> {
    match store.weenie(label)? {
        Some(did) => Ok(store.properties(did)?),
        None => Ok(None),
    }
}

fn required<S: RecordSource>(store: &ObjectStore<S>, label: &str) -> Result<PropertySet> {
    control(store, label)?.ok_or_else(|| missing(label))
}

fn get<'a>(set: &'a PropertySet, reg: &PropertyRegistry, name: &str) -> Option<&'a Value> {
    set.get_by_name(reg, name)
}

fn did(set: &PropertySet, reg: &PropertyRegistry, name: &str) -> Option<Did> {
    get(set, reg, name)
        .and_then(Value::as_did)
        .filter(|d| !d.is_null())
}

fn sinfo(set: &PropertySet, reg: &PropertyRegistry, name: &str) -> Option<StringInfo> {
    get(set, reg, name).and_then(Value::as_string_info).cloned()
}

/// Adds the display fields of the race table's `RaceTable_GenderList` structs to the
/// gender choices built from the view ([`RaceChoice::from_record`]). The view accepted
/// the list, so every element is a struct and the two are index-aligned.
fn add_gender_display(set: &PropertySet, reg: &PropertyRegistry, genders: &mut [GenderChoice]) {
    let Some(list) = get(set, reg, "RaceTable_GenderList") else {
        return;
    };
    let structs = list.children().iter().filter_map(|e| e.value.as_struct());
    for (c, g) in genders.iter_mut().zip(structs) {
        c.icon = did(g, reg, "RaceTable_RaceSelect_Icon");
        c.race_name = sinfo(g, reg, "RaceTable_Gender_Race_Name");
        c.name = sinfo(g, reg, "RaceTable_Gender_Name");
        c.description = sinfo(g, reg, "RaceTable_Gender_Desc");
        c.chargen_zoom_camera =
            get(g, reg, "RaceTable_Chargen_Zoom_Camera").and_then(Value::as_enum);
        c.select_zoom_camera =
            get(g, reg, "RaceTable_Character_Select_Zoom_Camera").and_then(Value::as_enum);
    }
}

/// Cuts `list` to `max` entries, logging when it was longer.
fn cap<T>(list: &mut Vec<T>, max: usize, what: &str, table: Did) {
    if list.len() > max {
        tracing::warn!(race = %table, len = list.len(), max, "{what} too long; cut");
        list.truncate(max);
    }
}

/// The property set of record `did`, or the reason it is skipped.
fn record_set<S: RecordSource>(
    store: &ObjectStore<S>,
    did: Did,
) -> std::result::Result<PropertySet, String> {
    match store.properties(did) {
        Ok(Some(set)) => Ok(set),
        Ok(None) => Err("the record is missing".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Records a skipped record in `out` and logs it.
fn skip(out: &mut ChargenData, did: Did, what: &str, reason: String) {
    tracing::warn!(record = %did, %reason, "{what} skipped");
    out.unreadable.push((did, reason));
}

fn class_choice(set: &PropertySet, reg: &PropertyRegistry, class: u32, table: Did) -> ClassChoice {
    let info = get(set, reg, "AdvTable_ClassInfo").and_then(Value::as_struct);
    ClassChoice {
        class,
        table: Some(table),
        icon: info.and_then(|i| did(i, reg, "AdvTable_ClassIcon")),
        name: info.and_then(|i| sinfo(i, reg, "AdvTable_ClassName")),
        description: info.and_then(|i| sinfo(i, reg, "AdvTable_ClassDesc")),
        background: did(set, reg, "AdvTable_CharGenBackground"),
    }
}

/// The appearance controls of one player avatar: the game object's `PhysObj` entity
/// (`appearance.md` §2.1) and its `AppearanceUI_Controls`.
fn avatar_controls<S: RecordSource>(
    store: &ObjectStore<S>,
    reg: &PropertyRegistry,
    pids: &AppearancePids,
    chargen_bit: Option<u64>,
    avatar: Did,
) -> std::result::Result<AvatarControls, String> {
    let set = store
        .properties(avatar)
        .map_err(|e| e.to_string())?
        .ok_or("the avatar object has no property set")?;
    let entity = did(&set, reg, "PhysObj").ok_or("the avatar object has no PhysObj")?;
    let bytes = store
        .game_data()
        .read(entity)
        .map_err(|e| e.to_string())?
        .ok_or("the avatar entity is missing")?;
    let desc = EntityDescriptor::parse_expecting(&bytes, entity, reg).map_err(|e| e.to_string())?;
    let props = PropertySet::from_raw(desc.properties).map_err(|e| e.to_string())?;
    let all = read_controls(&props, pids)?;
    let creation = creation_controls(&all, chargen_bit);
    Ok(AvatarControls {
        entity,
        all,
        creation,
    })
}

/// Most race tables [`load_chargen`] keeps (hostile-input bound; the data has 8).
pub const MAX_RACES: usize = 256;
/// Most gender entries kept per race (the screen has two gender buttons; the data has 1–2).
pub const MAX_GENDERS_PER_RACE: usize = 16;
/// Most class and nationality keys kept per race (the data has at most 11 classes).
pub const MAX_RACE_LIST_ENTRIES: usize = 256;

/// The view or object-layer reading of an optional control or directory named by its
/// WEENIECONTENT label: `None` when absent or skipped (recorded in `out`).
fn optional_control<S: RecordSource, T>(
    store: &ObjectStore<S>,
    out: &mut ChargenData,
    label: &str,
    view: impl FnOnce(&PropertySet) -> tessera_wstate::Result<T>,
) -> Option<T> {
    let did = match store.weenie(label) {
        Ok(Some(did)) => did,
        Ok(None) => return None,
        Err(e) => {
            skip(out, Did(0), label, e.to_string());
            return None;
        }
    };
    let set = match store.properties(did) {
        Ok(Some(set)) => set,
        Ok(None) => return None,
        Err(e) => {
            skip(out, did, label, e.to_string());
            return None;
        }
    };
    match view(&set) {
        Ok(v) => Some(v),
        Err(e) => {
            skip(out, did, label, e.to_string());
            None
        }
    }
}

/// Reads every list of the creation screens from the install (`character-screens.md` §4,
/// §5): race tables with their gender entries, class display data and priorities,
/// nationality tables, and each player avatar's appearance controls.
///
/// A race, nationality or class table, a directory other than the race directory, or the
/// class-priority control that is missing, unreadable or rejected by its view is skipped,
/// logged and listed in [`ChargenData::unreadable`] (reported by
/// [`ChargenData::problems`]), so one broken record never empties the screens. A table
/// listed twice is read once. At most [`MAX_RACES`] races are kept, each with at most
/// [`MAX_GENDERS_PER_RACE`] gender entries and [`MAX_RACE_LIST_ENTRIES`] class and
/// nationality keys (cuts are logged).
///
/// # Errors
///
/// [`GameDataError::MissingRecord`] without a race directory; the object layer's errors
/// when the dictionary or the race directory cannot be read or its view rejects it. An
/// avatar whose controls cannot be read is recorded in [`ChargenData::controls`], not an
/// error.
pub fn load_chargen<S: RecordSource>(store: &ObjectStore<S>) -> Result<ChargenData> {
    let reg = store.game_data().registry()?.clone();
    let reg = reg.as_ref();
    let mut out = ChargenData::default();

    let mut seen = BTreeSet::new();
    for t in RaceDirectory::from_properties(&required(store, RACE_DIRECTORY)?, reg)?.tables {
        if !seen.insert(t) {
            tracing::warn!(race = %t, "race table listed again; read once");
            continue;
        }
        if out.races.len() >= MAX_RACES {
            tracing::warn!("more than {MAX_RACES} race tables; cut");
            break;
        }
        let set = match record_set(store, t) {
            Ok(set) => set,
            Err(reason) => {
                skip(&mut out, t, "race table", reason);
                continue;
            }
        };
        let mut record = match RaceRecord::from_properties(&set, reg) {
            Ok(r) => r,
            Err(e) => {
                skip(&mut out, t, "race table", e.to_string());
                continue;
            }
        };
        cap(&mut record.genders, MAX_GENDERS_PER_RACE, "gender list", t);
        cap(&mut record.classes, MAX_RACE_LIST_ENTRIES, "class list", t);
        cap(
            &mut record.nationalities,
            MAX_RACE_LIST_ENTRIES,
            "nationality list",
            t,
        );
        let mut race = RaceChoice::from_record(t, record);
        add_gender_display(&set, reg, &mut race.genders);
        out.races.push(race);
    }

    if let Some(dir) = optional_control(store, &mut out, NATIONALITY_DIRECTORY, |s| {
        NationalityDirectory::from_properties(s, reg)
    }) {
        let mut seen = BTreeSet::new();
        for t in dir.tables {
            if !seen.insert(t) {
                continue;
            }
            let read = record_set(store, t).and_then(|s| {
                NationalityRecord::from_properties(&s, reg).map_err(|e| e.to_string())
            });
            match read {
                Ok(n) => {
                    out.nationalities.entry(n.nationality).or_insert((t, n));
                }
                Err(reason) => skip(&mut out, t, "nationality table", reason),
            }
        }
    }

    if let Some(p) = optional_control(store, &mut out, CLASS_PRIORITY, |s| {
        ClassPriority::from_properties(s, reg)
    }) {
        out.priority = p;
    }

    if let Some(dir) = optional_control(store, &mut out, LEVEL_TABLE_DIRECTORY, |s| {
        LevelTableDirectory::from_properties(s, reg)
    }) {
        let mut seen = BTreeSet::new();
        for t in dir.class_tables {
            if !seen.insert(t) {
                continue;
            }
            // A class table that is missing is not one of the screens' (it names no class).
            let s = match store.properties(t) {
                Ok(Some(s)) => s,
                Ok(None) => continue,
                Err(e) => {
                    skip(&mut out, t, "class table", e.to_string());
                    continue;
                }
            };
            let class = match ClassCreationInfo::from_properties(&s, reg) {
                Ok(info) => info.class,
                Err(e) => {
                    skip(&mut out, t, "class table", e.to_string());
                    continue;
                }
            };
            let Some(class) = class else {
                continue;
            };
            out.classes
                .entry(class)
                .or_insert_with(|| class_choice(&s, reg, class, t));
        }
    }

    let data = store.game_data();
    let dict = reg.dictionary();
    let availability = Pid::resolve(dict, "AppearanceUI_Availability");
    let chargen_bit =
        label_key(data, dict, availability, CHARGEN_LABEL).and_then(crate::inventory::slot_bit);
    if chargen_bit.is_none() {
        tracing::warn!("no CharGen availability label; every appearance control is offered");
    }
    if let Some(start) = enum_mapper_of(dict, Pid::resolve(dict, "AppearanceUI_Group")) {
        for m in mapper_chain(data, start) {
            for (k, n) in m.names() {
                out.group_labels.entry(*k).or_insert_with(|| n.clone());
            }
        }
    }
    let pids = AppearancePids::resolve(reg);
    let avatars: Vec<Did> = out
        .races
        .iter()
        .flat_map(|r| r.genders.iter().filter_map(|g| g.player_avatar))
        .collect();
    for a in avatars {
        if out.controls.contains_key(&a) {
            continue;
        }
        let c = avatar_controls(store, reg, &pids, chargen_bit, a);
        out.controls.insert(a, c);
    }
    Ok(out)
}

/// Reads the name generator and the name-filter table (`player-character.md` §8.2;
/// `misc-records.md` §9) and resolves the generator's texts with `resolve` (normally the
/// install's string tables; an unresolvable text is `None`). The policy is
/// [`NamePolicy::tessera_default`]; without a name-filter table there is none.
///
/// # Errors
///
/// [`GameDataError::MissingRecord`] without a generator; the object layer's errors; a
/// name-filter table that does not decode is [`GameDataError::View`].
pub fn load_names<S: RecordSource>(
    store: &ObjectStore<S>,
    resolve: impl FnMut(&StringInfo) -> Option<String>,
) -> Result<NameData> {
    let reg = store.game_data().registry()?.clone();
    let generator = NameGenerator::from_properties(&required(store, NAME_GENERATOR)?, &reg)?;
    let text = generator.resolve(resolve)?;
    let policy = match load_name_filter(store)? {
        Some(t) => {
            let p = NamePolicy::tessera_default(&t, &generator.valid_rules);
            if p.is_none() {
                tracing::warn!("no usable name rule set; generated names are not checked");
            }
            p
        }
        None => None,
    };
    Ok(NameData {
        generator,
        text,
        policy,
    })
}

/// The name-filter table (`misc-records.md` §9) through its UNIQUEDB label; `None` when
/// the install has none.
///
/// # Errors
///
/// The property layer's errors; [`GameDataError::View`] when it does not decode.
pub fn load_name_filter<S: RecordSource>(
    store: &ObjectStore<S>,
) -> Result<Option<NameFilterTable>> {
    let gd = store.game_data();
    let Some(did) = gd.lookup_path(&[UNIQUEDB, NAME_FILTER_TABLE])? else {
        return Ok(None);
    };
    let Some(bytes) = gd.read(did)? else {
        return Ok(None);
    };
    NameFilterTable::parse_expecting(&bytes, did)
        .map(Some)
        .map_err(|e| GameDataError::View {
            did,
            reason: e.to_string(),
        })
}
