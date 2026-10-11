// SPDX-License-Identifier: GPL-3.0-or-later
//! Player music in the client (ROADMAP M6, `tessera-client`: "`/play`-style commands"):
//! the chat commands of [`command`] driving the ABC reader (`tessera_audio::abc`) and the
//! instruments (`tessera_audio::instruments`) for the local player.
//!
//! | Part | Spec | Here |
//! |---|---|---|
//! | Command syntax: `/play`, `/playstart`, `/playlist`, stop, debug instrument | `abc-dialect.md` §11; `instruments.md` §12 | [`command`]; the words are client built-ins of the command dispatcher (decision 0020), which calls [`PlayerMusic::run_tokens`] |
//! | Music folders: the original's settings folder `Music/` (read-only), then Tessera's own | `abc-dialect.md` §3.1; `install-layout.md` §8.1 | [`music_folders`], [`find_in_folders`] |
//! | No part given: the reader's "none" (the first X:) | `abc-dialect.md` §5.3, §11 OPEN (1) | [`MusicCommand::Play`] |
//! | Solo start 0.7 s after the command; synced start waits for `/playstart` | §8.5; `instruments.md` §12 OPEN (d) | [`PlayerMusic`] |
//! | Errors and notices from the user's string tables, else Tessera's own text | `abc-dialect.md` §10.2–§10.4 | [`Messages`], [`fallback_error`], [`fallback_notice`] |
//! | Instrument type from the equipped instrument item, or a debug choice | `instruments.md` §4.5, §12 | [`ItemFacts`], [`equipped_instrument`], [`resolve_instrument`] |
//! | Synchronised start (P4): waiting parts of the player and the fellowship's members, one start for all, the player's `/play … sync` and `/playstart` through the session | `instruments.md` §8.8–§8.9, §12 P4 Decision; `scenario-feed.md` §6.7 | [`PlayerMusic::social_event`], [`PlayerMusic::take_requests`], [`MusicRequest`] |
//!
//! # Threads
//!
//! Nothing here reads a file or an archive on the calling (frame) thread: start-up (the
//! instrument table, the message table, the equipped item, the volume settings and the
//! original's settings-folder `Music`) and every command that reads files (`/play` reads
//! the song and decodes the samples its notes need; `/playlist` reads the folders) run on
//! short-lived background threads. [`PlayerMusic::update`] collects their results. The one
//! exception is opening the output device when the first performance starts: the engine
//! cannot move between threads (as for [`super::WorldAudio`], whose engine starts on the
//! main thread too).
//!
//! # Order
//!
//! Commands are processed one after the other, in the order they were given, as the
//! original processes them (`abc-dialect.md` §8.5, §11): a command waits in a queue (at most
//! [`MAX_QUEUED`]) until start-up and every earlier command have finished, so `/playstop`
//! or `/playstart` given while a `/play` is still being read acts after that `/play`, and
//! of two `/play`s the later one plays. At most one background job runs at a time. A solo
//! performance starts [`SOLO_START_OFFSET`] after its command has been processed, that is
//! after its file has been read (§8.5).
//!
//! # Engine
//!
//! The player music has its own [`AudioEngine`], started at the first performance with the
//! walker's backend choice (`--audio`), so that the world audio of [`super::WorldAudio`]
//! is unchanged. The local player and the listener are both at the origin: the player hears
//! their own music centred, as the original's listener near the avatar does
//! (`instruments.md` §10.5).

pub mod command;
pub mod sync;

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use tessera_audio::abc::{
    self, AbcError, AbcErrorKind, ArgValue, MESSAGE_TABLE_KEY, MessageRef, Notice, PartRequest,
    Performance, SOLO_START_OFFSET, SongListing, StartMode,
};
use tessera_audio::decode::{Decoded, decode_record};
use tessera_audio::engine::{AudioEngine, BackendKind, LivePosition, MixerSettings};
use tessera_audio::instruments::{
    CharacterId, InstrumentTable, MusicPlayer, MusicScript, MusicScripts, MusicVocabulary,
    PlayerConfig, PlayerStats, SampleBank, SyncGroup, TableLimits,
};
use tessera_audio::pcm::Pcm;
use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::property_set::{PropertySetRecord, object_property_set_did};
use tessera_formats::script_table::resolve::ScriptVocabulary;
use tessera_formats::{DataIdMap, Did, PropertyDictionary, PropertyValue, Record};
use tessera_text::{
    LanguageId, Locale, NamedTables, RenderOptions, STRING_TABLE_DIDS, STRING_TABLE_MAP_LABEL,
    Strings, TableSource, Value, Values, select_language,
};

pub use self::command::{CommandError, MusicCommand, MusicWord, parse_args};
use self::sync::{Members, Queued, SocialItem};
pub use self::sync::{MusicRequest, StartClock};
use super::backend::{self, BackendChoice, PrefsChoice, SettingsChoice};
use super::loader::fetch;
use super::scripts::{PROPERTY_DICTIONARY, ROOT_DID_MAP, mapper};
use crate::wait::{LoadWait, Waiter};

/// The local player's character in the player-music module.
pub const LOCAL_PLAYER: CharacterId = CharacterId(1);
/// Most commands waiting for start-up or an earlier command (module docs, "Order");
/// further commands are refused with a "busy" line.
pub const MAX_QUEUED: usize = 16;
/// Most chat lines waiting for [`PlayerMusic::take_lines`]; older ones are dropped.
pub const MAX_PENDING_LINES: usize = 512;
/// Most songs one `/playlist` prints (a line says how many more matched).
pub const MAX_PLAYLIST_SONGS: usize = 100;
/// Most parts listed on one `/playlist` line.
pub const MAX_PLAYLIST_PARTS: usize = 16;
/// Most decoded instrument samples kept, in samples (f32): 128 MiB. When a new
/// performance's samples would exceed it, the samples it does not need are dropped first,
/// unless another performance that keeps going still needs them (a solo performance
/// running while a synced one is registered); samples that still do not fit are not kept,
/// their notes are silent, and a chat line says so.
pub const SAMPLE_BUDGET: usize = 32 * 1024 * 1024;
/// Most performances kept in [`PlayerMusic::schedule_log`].
pub const MAX_SCHEDULE_LOG: usize = 16;
/// Most synchronised starts kept for [`PlayerMusic::sync_starts`].
pub const MAX_SYNC_STARTS: usize = 16;
/// Most frames of offline-backend audio rendered per [`PlayerMusic::update`].
pub const MAX_OFFLINE_FRAMES: usize = 48_000 / 4;
/// `Item_EquipmentCategory` of an instrument item: the whole 64-bit value
/// (`instruments.md` §4.5, Confirmed (BIN)).
pub const EQUIPMENT_CATEGORY_INSTRUMENT: u64 = 0x40;
/// Slot-mask bits that make an equipped instrument the wielder's (`instruments.md` §4.5:
/// 0x10000, 0x20000, 0x40000).
pub const INSTRUMENT_SLOT_BITS: u64 = 0x1_0000 | 0x2_0000 | 0x4_0000;
/// The folder below Tessera's data folder that holds Tessera's own songs.
pub const TESSERA_MUSIC_FOLDER: &str = "Music";

/// Player-music settings.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicConfig {
    /// The engine backend (started at the first performance).
    pub backend: BackendChoice,
    /// Where Tessera's own `[audio]` volumes come from.
    pub settings: SettingsChoice,
    /// Importing the original's `[Sound]` volumes.
    pub prefs: PrefsChoice,
    /// The install directory (the preferences import and the settings folder).
    pub game_dir: Option<PathBuf>,
    /// Tessera's data folder (its `Music` folder is the second music folder).
    pub data_root: Option<PathBuf>,
    /// Music folders in lookup order. They need not exist: each command tests them when
    /// it runs, so a folder created while the client runs is found (`abc-dialect.md` §3.1).
    pub folders: Vec<PathBuf>,
    /// An install whose settings folder's `Music` start-up looks up (off the frame thread)
    /// and puts first in [`Self::folders`] (`install-layout.md` §8.1): `--game-dir`, or the
    /// install the archives were opened from.
    pub settings_lookup: Option<PathBuf>,
    /// The avatar's items (`--avatar`, DIDs with prefix 0x70–0x76 or 0x79), in equip order.
    pub items: Vec<u32>,
    /// The avatar's object (`--avatar <DID>`), whose script closure holds the music
    /// scripts; `None`: the race table's first player avatar.
    pub avatar: Option<u32>,
    /// A debug instrument choice (label or number) that overrides the equipped item.
    pub instrument: Option<String>,
    /// Preferred language of the message table (`string-table.md` §10.1).
    pub language: Option<String>,
    /// Seed of the instruments' random draws.
    pub seed: u64,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            backend: BackendChoice::Null,
            settings: SettingsChoice::Defaults,
            prefs: PrefsChoice::IfEnabled,
            game_dir: None,
            data_root: None,
            folders: Vec::new(),
            settings_lookup: None,
            items: Vec::new(),
            avatar: None,
            instrument: None,
            language: None,
            seed: PlayerConfig::default().seed,
        }
    }
}

impl MusicConfig {
    /// The settings of the command line: the audio choices of
    /// [`super::config_from_args`], the avatar's items and object from `--avatar`, the
    /// music folders (Tessera's data folder `data_root`; the settings folder of
    /// `--game-dir` is looked up at start-up). Reads no file. (The `--command` lines go to
    /// the command dispatcher, decision 0020 item 7.)
    #[must_use]
    pub fn from_args(args: &crate::cli::Args, data_root: &Path) -> Self {
        let audio = super::config_from_args(args);
        let (items, avatar) = match &args.avatar {
            Some(a) => (
                a.items.iter().map(|i| i.did).collect(),
                match a.body {
                    crate::avatar::AvatarBody::Object(d) => Some(d),
                    crate::avatar::AvatarBody::Playable { .. } => None,
                },
            ),
            None => (Vec::new(), None),
        };
        Self {
            backend: audio.backend,
            settings: audio.settings,
            prefs: audio.prefs,
            game_dir: args.game_dir.clone(),
            data_root: Some(data_root.to_path_buf()),
            folders: vec![data_root.join(TESSERA_MUSIC_FOLDER)],
            settings_lookup: args.game_dir.clone(),
            items,
            avatar,
            instrument: None,
            language: None,
            seed: audio.seed,
        }
    }
}

/// The music folders in lookup order (`abc-dialect.md` §3.1): `Music` in the original's
/// settings folder (`install-layout.md` §8.1, found through `tessera-install` for the
/// install at `game_dir`), then `Music` in Tessera's data folder `data_root`. The folders
/// are listed whether or not they exist yet (each command tests them when it runs); both
/// are only ever read. Blocking (the settings-folder search): not for the frame thread.
#[must_use]
pub fn music_folders(game_dir: Option<&Path>, data_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = game_dir.and_then(settings_music_folder) {
        out.push(dir);
    }
    out.push(data_root.join(TESSERA_MUSIC_FOLDER));
    out.dedup();
    out
}

/// `Music` in the original's settings folder for the install at `game_dir`, when that
/// settings folder exists (the `Music` folder itself need not). Blocking.
#[must_use]
pub fn settings_music_folder(game_dir: &Path) -> Option<PathBuf> {
    original_settings_dir(game_dir).map(|d| d.join(abc::MUSIC_FOLDER))
}

/// The original's settings folder for the install at `game_dir`, if it exists.
fn original_settings_dir(game_dir: &Path) -> Option<PathBuf> {
    let install = tessera_install::GameInstall::open(game_dir).ok()?;
    let folder = backend::document_folder(game_dir);
    let dir = install.settings_dir(
        &tessera_install::SettingsQuery {
            prefs_file: None,
            document_folder: folder.as_deref(),
        },
        &tessera_install::SearchEnv::from_process(),
    );
    dir.exists.then_some(dir.path)
}

/// Finds a song in the folders in order (`abc-dialect.md` §3.1 per folder; a folder that
/// does not exist has no song). The error of the first folder is reported when no folder
/// has it (its path is the original's `.txt` path); with no folder at all, the name joined
/// to `Music` with the `.txt` extension is reported, without looking at the file system.
///
/// # Errors
///
/// [`AbcErrorKind::FileNotFound`].
pub fn find_in_folders(folders: &[PathBuf], name: &str) -> Result<PathBuf, AbcError> {
    let mut first = None;
    for f in folders {
        match abc::find_song(f, name) {
            Ok(p) => return Ok(p),
            Err(e) => {
                first.get_or_insert(e);
            }
        }
    }
    Err(first.unwrap_or_else(|| AbcError {
        kind: AbcErrorKind::FileNotFound,
        position: None,
        path: Some(
            Path::new(abc::MUSIC_FOLDER)
                .join(name)
                .with_extension("txt"),
        ),
    }))
}

/// Reads one part of a song from the folders (`abc::read_song` on the folder that has it).
///
/// # Errors
///
/// As [`abc::read_song`].
pub fn read_from_folders(
    folders: &[PathBuf],
    name: &str,
    request: PartRequest,
) -> Result<Performance, AbcError> {
    let path = find_in_folders(folders, name)?;
    // `read_song` repeats the lookup in the folder that holds the file.
    let folder = folders
        .iter()
        .find(|f| path.starts_with(f))
        .cloned()
        .unwrap_or_default();
    abc::read_song(&folder, name, request)
}

// ---------------------------------------------------------------------------------------
// Messages (abc-dialect.md §10.2–§10.4)
// ---------------------------------------------------------------------------------------

/// String-table records of the selected language archive of a set.
#[derive(Debug, Clone)]
struct LocalTables {
    set: Arc<ArchiveSet>,
    member: usize,
}

impl TableSource for LocalTables {
    fn read_table(&self, did: u32) -> tessera_text::Result<Option<Vec<u8>>> {
        let Some(m) = self.set.get(self.member) else {
            return Ok(None);
        };
        match m.archive.lookup(did)? {
            None => Ok(None),
            Some(entry) => Ok(Some(m.archive.read_entry(&entry)?)),
        }
    }

    fn table_dids(&self) -> tessera_text::Result<Vec<u32>> {
        let Some(m) = self.set.get(self.member) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for entry in m.archive.entries_range(STRING_TABLE_DIDS) {
            out.push(entry?.did);
        }
        Ok(out)
    }
}

/// The player-music message table of the user's locale (`abc-dialect.md` §10.3): the
/// table named by key [`MESSAGE_TABLE_KEY`] of the named-string-table DID map, looked up
/// at run time. Nothing of the table's text is built in.
#[derive(Debug)]
pub struct Messages {
    strings: Strings,
    table: Option<u32>,
}

impl Messages {
    /// Messages over the selected language archive of `set` (`language`, else English,
    /// `string-table.md` §10.1). `None` when the set has no language archive.
    #[must_use]
    pub fn open(set: &Arc<ArchiveSet>, language: Option<&str>) -> Option<Self> {
        let strings = local_strings(set, language)?;
        let table = strings.named_tables().by_key(MESSAGE_TABLE_KEY);
        if table.is_none() {
            tracing::info!(
                key = format_args!("{MESSAGE_TABLE_KEY:#010x}"),
                "player music: no message table; Tessera's own texts are used"
            );
        }
        Some(Self { strings, table })
    }

    /// Messages over `strings`, with the table found through its named tables.
    #[must_use]
    pub fn from_strings(strings: Strings) -> Self {
        let table = strings.named_tables().by_key(MESSAGE_TABLE_KEY);
        Self { strings, table }
    }

    /// The message table's DID, if the named-table map has the key.
    #[must_use]
    pub fn table(&self) -> Option<u32> {
        self.table
    }

    /// Renders `msg` from the user's table, or `None` when the table or entry is missing
    /// or renders empty.
    #[must_use]
    pub fn render(&self, msg: &MessageRef) -> Option<String> {
        if msg.table_key != MESSAGE_TABLE_KEY {
            return None;
        }
        let table = self.table?;
        let message = self.strings.message(table, msg.token).ok()?;
        let mut values = Values::new();
        for a in &msg.args {
            let v: Value = match &a.value {
                ArgValue::Integer(n) => Value::from(*n),
                ArgValue::Text(t) => Value::from(t.clone()),
            };
            values.insert(a.id, v);
        }
        let text = tessera_text::render(&message, &values, &RenderOptions::default()).plain_text();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }
}

/// The string tables of the selected language archive of `set` (`language`, else
/// English, `string-table.md` §10.1) with the named-table map (§9.1). `None` when the set
/// has no language archive. Blocking (reads the archives): not for the frame thread.
pub fn local_strings(set: &Arc<ArchiveSet>, language: Option<&str>) -> Option<Strings> {
    let locals: Vec<(String, usize)> = set
        .members()
        .iter()
        .enumerate()
        .filter_map(|(i, m)| match &m.id.kind {
            ArchiveKind::Local(lang) if m.id.part == 0 => Some((lang.clone(), i)),
            _ => None,
        })
        .collect();
    let names: Vec<&str> = locals.iter().map(|(n, _)| n.as_str()).collect();
    let chosen = select_language(&names, language)?.to_owned();
    let member = locals.iter().find(|(n, _)| *n == chosen).map(|&(_, i)| i)?;
    let m = set.get(member)?;
    let locale = Locale {
        name: chosen,
        language_id: LanguageId(m.archive.header().subset),
        path: m.path.clone(),
    };
    let named = named_tables(set);
    Some(
        Strings::new(LocalTables {
            set: Arc::clone(set),
            member,
        })
        .with_locale(locale)
        .with_named_tables(named),
    )
}

/// The named string tables through the root DID map (`string-table.md` §9.1); empty when
/// they cannot be read.
fn named_tables(set: &ArchiveSet) -> NamedTables {
    let read = |did: u32| -> Option<DataIdMap> {
        let bytes = fetch(set, did).ok()??;
        DataIdMap::parse_expecting(&bytes, did).ok()
    };
    let Some(root) = read(ROOT_DID_MAP) else {
        return NamedTables::default();
    };
    root.by_label(STRING_TABLE_MAP_LABEL)
        .and_then(|d| read(d.0))
        .map(|m| NamedTables::from_did_map(&m))
        .unwrap_or_default()
}

/// Tessera's own text for an ABC error when the user's table has none (neutral wording,
/// not the original's).
#[must_use]
pub fn fallback_error(e: &AbcError) -> String {
    format!("Music: {e}")
}

/// Tessera's own text for a notice when the user's table has none.
#[must_use]
pub fn fallback_notice(n: &Notice) -> String {
    match n {
        Notice::NowPlaying { title } => format!("Music: performance starts: {title}"),
        Notice::NowPlayingUntitled => "Music: performance starts".to_owned(),
        Notice::SyncedStart => "Music: synchronised start".to_owned(),
        Notice::EventLimit => format!(
            "Music: the part has more than {} events and does not play",
            abc::MAX_EVENTS
        ),
    }
}

/// The text of an ABC error: the user's table entry (§10.2–§10.3), else [`fallback_error`].
#[must_use]
pub fn error_text(messages: Option<&Messages>, e: &AbcError) -> String {
    if e.kind == AbcErrorKind::TooManyEvents {
        return notice_text(messages, &Notice::EventLimit);
    }
    e.message()
        .and_then(|m| messages?.render(&m))
        .unwrap_or_else(|| fallback_error(e))
}

/// The text of a notice: the user's table entry (§10.4), else [`fallback_notice`].
#[must_use]
pub fn notice_text(messages: Option<&Messages>, n: &Notice) -> String {
    n.message()
        .and_then(|m| messages?.render(&m))
        .unwrap_or_else(|| fallback_notice(n))
}

// ---------------------------------------------------------------------------------------
// Instrument type (instruments.md §4.5, §12)
// ---------------------------------------------------------------------------------------

/// The facts of one equipped item that decide the instrument type (`instruments.md`
/// §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemFacts {
    /// `Item_EquipmentCategory`, the whole bit pattern.
    pub category: Option<u64>,
    /// The slot mask of the slot it is equipped in (the walker equips items in their
    /// `Inventory_DefaultSlot`).
    pub slot: Option<u64>,
    /// Whether `PhysObj` is a valid (non-null) DID.
    pub phys_obj: bool,
    /// `Item_Music_Instrument_Type`.
    pub instrument_type: Option<u32>,
}

impl ItemFacts {
    /// Reads the facts from an item's property set (names looked up in `dict`).
    #[must_use]
    pub fn from_properties(set: &tessera_formats::PropertySet, dict: &PropertyDictionary) -> Self {
        let get = |name: &str| dict.pid(name).and_then(|p| set.get(p));
        Self {
            category: get("Item_EquipmentCategory").and_then(PropertyValue::as_bits),
            slot: get("Inventory_DefaultSlot").and_then(PropertyValue::as_bits),
            phys_obj: get("PhysObj")
                .and_then(PropertyValue::as_did)
                .is_some_and(|d| !d.is_null()),
            instrument_type: get("Item_Music_Instrument_Type")
                .and_then(PropertyValue::as_bits)
                .and_then(|b| u32::try_from(b).ok()),
        }
    }

    /// Whether equipping the item sets the wielder's cached instrument type (§4.5: the
    /// category is exactly the instrument value and the slot has one of the three bits).
    #[must_use]
    pub fn sets_instrument(&self) -> bool {
        self.category == Some(EQUIPMENT_CATEGORY_INSTRUMENT)
            && self.slot.is_some_and(|s| s & INSTRUMENT_SLOT_BITS != 0)
    }
}

/// The cached instrument type after equipping `items` in order (`instruments.md` §4.5):
/// each instrument item sets it to its `Item_Music_Instrument_Type` (0 when it has none)
/// when its `PhysObj` is valid, else to 0. `None` when no item is an instrument in an
/// instrument slot.
#[must_use]
pub fn equipped_instrument(items: &[ItemFacts]) -> Option<u32> {
    let mut out = None;
    for i in items.iter().filter(|i| i.sets_instrument()) {
        out = Some(if i.phys_obj {
            i.instrument_type.unwrap_or(0)
        } else {
            0
        });
    }
    out
}

/// A debug instrument choice: `item` (`None`: use the equipped item), a decimal or `0x`
/// number, or a label of the instrument-type mapper ignoring ASCII case.
///
/// # Errors
///
/// The choice as given when it is neither.
pub fn resolve_instrument(choice: &str, vocab: &MusicVocabulary) -> Result<Option<u32>, String> {
    let c = choice.trim();
    if c.eq_ignore_ascii_case("item") {
        return Ok(None);
    }
    let number = match c.strip_prefix("0x").or_else(|| c.strip_prefix("0X")) {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => c.parse::<u32>().ok(),
    };
    if let Some(n) = number {
        return Ok(Some(n));
    }
    vocab
        .instrument_labels
        .iter()
        .find(|(_, l)| l.eq_ignore_ascii_case(c))
        .map(|(k, _)| Some(*k))
        .ok_or_else(|| c.to_owned())
}

// ---------------------------------------------------------------------------------------
// Start-up data
// ---------------------------------------------------------------------------------------

/// What start-up loads from the install and the settings.
#[derive(Debug)]
pub struct MusicData {
    /// The archives; `None` when start-up ran without game data (songs are then read and
    /// checked, nothing plays).
    pub set: Option<Arc<ArchiveSet>>,
    /// The instrument table, or why it could not be built.
    pub table: Result<Arc<InstrumentTable>, String>,
    /// The message table of the user's locale.
    pub messages: Option<Messages>,
    /// The equipped item's instrument type (§4.5), if an item is an instrument.
    pub equipped: Option<u32>,
    /// The engine's volumes: Tessera's `[audio]` values and the imported preferences
    /// (`--audio-settings`, `--audio-prefs`), with the table's channel labels.
    pub mixer: MixerSettings,
    /// `Music` in the original's settings folder of [`MusicConfig::settings_lookup`], if
    /// that settings folder was found.
    pub settings_music: Option<PathBuf>,
    /// Seconds start-up took.
    pub secs: f64,
}

impl MusicData {
    /// Loads everything (blocking: archives, settings files and the settings-folder
    /// search; run on a background thread).
    #[must_use]
    pub fn load(set: Option<Arc<ArchiveSet>>, config: &MusicConfig) -> Self {
        let t0 = Instant::now();
        let (table, messages, equipped) = match &set {
            Some(set) => load_tables(set, config),
            None => (Err("no game data".to_owned()), None, None),
        };
        let own = backend::own_settings(&config.settings);
        let settings = backend::resolve_settings(own, &config.prefs, config.game_dir.as_deref());
        let mut labels = backend::default_labels();
        if let Ok(t) = &table {
            for (k, l) in &t.vocabulary().channel_labels {
                if let Some(slot) = labels.get_mut(*k as usize) {
                    slot.clone_from(l);
                }
            }
        }
        let mixer = backend::mixer_settings(&settings, &labels);
        let settings_music = config
            .settings_lookup
            .as_deref()
            .and_then(settings_music_folder);
        Self {
            set,
            table,
            messages,
            equipped,
            mixer,
            settings_music,
            secs: t0.elapsed().as_secs_f64(),
        }
    }
}

/// The instrument table, the messages and the equipped instrument from the archives.
fn load_tables(
    set: &Arc<ArchiveSet>,
    config: &MusicConfig,
) -> (
    Result<Arc<InstrumentTable>, String>,
    Option<Messages>,
    Option<u32>,
) {
    let messages = Messages::open(set, config.language.as_deref());
    let dict = fetch(set, PROPERTY_DICTIONARY)
        .map_err(|e| e.to_string())
        .and_then(|b| b.ok_or_else(|| "property dictionary missing".to_owned()))
        .and_then(|b| {
            PropertyDictionary::parse_expecting(&b, PROPERTY_DICTIONARY).map_err(|e| e.to_string())
        });
    match dict {
        Ok(dict) => {
            let equipped = equipped_instrument(&item_facts(set, &dict, &config.items));
            (
                build_table(set, &dict, config.avatar).map(Arc::new),
                messages,
                equipped,
            )
        }
        Err(e) => (Err(format!("property dictionary: {e}")), messages, None),
    }
}

/// The facts of each item DID (prefix 0x79, or a 0x70–0x76 object's property set);
/// unreadable items are skipped.
fn item_facts(set: &ArchiveSet, dict: &PropertyDictionary, items: &[u32]) -> Vec<ItemFacts> {
    items
        .iter()
        .filter_map(|&did| {
            let ps = if did >> 24 == 0x79 {
                Did(did)
            } else {
                object_property_set_did(Did(did))?
            };
            let bytes = fetch(set, ps.0).ok()??;
            match PropertySetRecord::parse_expecting(&bytes, ps, dict) {
                Ok(r) => Some(ItemFacts::from_properties(&r.set, dict)),
                Err(e) => {
                    tracing::debug!(item = format_args!("{did:#010x}"), "item properties: {e}");
                    None
                }
            }
        })
        .collect()
}

/// The race table's first player avatar.
fn first_avatar(set: &ArchiveSet, dict: &PropertyDictionary) -> Option<Did> {
    let root = DataIdMap::parse_expecting(&fetch(set, ROOT_DID_MAP).ok()??, ROOT_DID_MAP).ok()?;
    let weenie = root.by_label("WEENIECONTENT")?;
    let weenie = DataIdMap::parse_expecting(&fetch(set, weenie.0).ok()??, weenie).ok()?;
    let directory = weenie.by_label("RaceTableDirectory")?;
    let pids = RacePids::from_names(dict).ok()?;
    let race = RaceTable::read(directory, dict, &pids, |d: Did| fetch(set, d.0)).ok()?;
    race.entries.first().map(|e| e.player_avatar)
}

/// The instrument table from the music scripts in the avatar's closure (`instruments.md`
/// §4.1, §12 Decision 1).
fn build_table(
    set: &ArchiveSet,
    dict: &PropertyDictionary,
    avatar: Option<u32>,
) -> Result<InstrumentTable, String> {
    let vocab = MusicVocabulary::from_data(dict, |d| mapper(set, d)).map_err(|e| e.to_string())?;
    let svocab = ScriptVocabulary::from_data(dict, |d| mapper(set, d))
        .map_err(|e| format!("script vocabulary: {e}"))?;
    let avatar = match avatar {
        Some(d) => Did(d),
        None => first_avatar(set, dict).ok_or("no player avatar in the race table")?,
    };
    let scripts = MusicScripts::locate(avatar, dict, &svocab, &vocab, |d: Did| fetch(set, d.0))
        .map_err(|e| format!("music scripts of {avatar}: {e}"))?;
    let table = InstrumentTable::build(scripts, &vocab, TableLimits::default(), |d: Did| {
        fetch(set, d.0)
    })
    .map_err(|e| e.to_string())?;
    for issue in table.issues().iter().take(8) {
        tracing::debug!(?issue, "player music: instrument table issue");
    }
    Ok(table)
}

/// The sample payloads the note-ons of `perf` need on `instrument` (song notes: the
/// from-song branch of the normal script, `instruments.md` §5.5), in first-use order.
fn wanted_samples(table: &InstrumentTable, perf: &Performance, instrument: u32) -> Vec<Did> {
    let mut wanted: Vec<Did> = Vec::new();
    let mut seen_wanted = HashSet::new();
    let mut seen = HashSet::new();
    for e in perf.note_ons() {
        if !seen.insert(e.pitch) {
            continue;
        }
        let key = table.pitch_key(e.pitch) as u8;
        for s in table.lookup(MusicScript::Normal, true, false, instrument, key) {
            if let Some(info) = table.sound_info(s.sound_info)
                && !info.payload.is_null()
                && seen_wanted.insert(info.payload)
            {
                wanted.push(info.payload);
            }
        }
    }
    wanted
}

/// Decodes the payloads of `wanted` that are not in `have` (`None`: no archives, every
/// payload is missing). Returns the decoded samples and one text per problem.
fn decode_samples(
    set: Option<&ArchiveSet>,
    wanted: &[Did],
    have: &HashSet<Did>,
) -> (Vec<(Did, Pcm)>, Vec<String>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for &did in wanted.iter().filter(|d| !have.contains(d)) {
        let rec = match set {
            Some(set) => fetch(set, did.0).map_err(|e| e.to_string()),
            None => Ok(None),
        };
        match rec {
            Ok(Some(rec)) => match decode_record(&rec) {
                Ok(Decoded::Pcm(pcm)) => out.push((did, pcm)),
                Ok(Decoded::NotAudio(_)) => problems.push(format!("{did}: not audio")),
                Err(e) => problems.push(format!("{did}: {e}")),
            },
            Ok(None) => problems.push(format!("{did}: missing")),
            Err(e) => problems.push(format!("{did}: {e}")),
        }
    }
    (out, problems)
}

// ---------------------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------------------

/// A `/play` that was read (and its samples decoded).
#[derive(Debug)]
struct SongJob {
    name: String,
    sync: bool,
    /// (P4) The fellowship member whose part this is (`PerformanceReady`), or `None` for
    /// the local player.
    member: Option<(String, CharacterId)>,
    /// (P4) The part came from the session (`PerformanceReady` for the player): it waits
    /// without a `ReadyPerformance` request.
    from_feed: bool,
    /// (P4) A `PerformanceStarted`: a solo-like start this long after the result is
    /// processed.
    delay_ms: Option<u32>,
    instrument: Option<u32>,
    /// The accepted part, or the chat line of the refusal.
    result: Result<Arc<Performance>, String>,
    /// Every payload the performance's notes need (also those already in the bank).
    needed: Vec<Did>,
    samples: Vec<(Did, Pcm)>,
    sample_problems: usize,
    secs: f64,
}

#[derive(Debug)]
enum JobResult {
    Song(Box<SongJob>),
    Lines(Vec<String>),
}

/// One performance as scheduled, for the log and the M6 verification.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledPerformance {
    /// The song name of the command (for a synced start, of the `/play … sync` that
    /// registered it).
    pub name: String,
    /// The title (§6.5).
    pub title: String,
    /// The X: value of the part, if any.
    pub part: Option<i32>,
    /// The instrument type.
    pub instrument: u32,
    /// Solo or synced.
    pub mode: StartMode,
    /// Time 0 of the events on the engine's audio clock, seconds.
    pub start_secs: f64,
    /// Note-on times relative to time 0, seconds, in output order.
    pub note_on_times: Vec<f64>,
    /// Events queued (note-ons, note-offs and the end).
    pub events: usize,
    /// The last event's time relative to time 0, seconds.
    pub end_time: f64,
}

/// Counters, for tools, tests and the M6 status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MusicStats {
    /// Player-music commands handled.
    pub commands: u64,
    /// Songs read and accepted.
    pub songs_accepted: u64,
    /// Songs refused by the reader.
    pub songs_refused: u64,
    /// Solo performances queued.
    pub solo_starts: u64,
    /// Synced performances registered.
    pub synced_waits: u64,
    /// Performances started by `/playstart`.
    pub synced_starts: u64,
    /// Note-ons scheduled (queued with a start time) over all performances.
    pub note_ons_scheduled: u64,
    /// Samples decoded and kept.
    pub samples_decoded: u64,
    /// Samples that could not be read or decoded, or did not fit [`SAMPLE_BUDGET`].
    pub sample_problems: u64,
    /// Commands refused because [`MAX_QUEUED`] commands waited.
    pub busy: u64,
    /// Command jobs that ended without a result (a failure on the job thread).
    pub failed_jobs: u64,
}

/// Player music for the local player (module docs).
pub struct PlayerMusic {
    config: MusicConfig,
    pending_set: Option<Arc<ArchiveSet>>,
    boot: Option<Receiver<MusicData>>,
    data: Option<Arc<MusicData>>,
    /// Commands (and P4 session items) not yet processed, in the order given.
    queue: VecDeque<Queued>,
    /// The background job of the command being processed.
    job: Option<Receiver<JobResult>>,
    engine: Option<AudioEngine>,
    player: Option<MusicPlayer>,
    bank: SampleBank,
    /// The payloads in `bank` with their sample counts (the bank has no iterator).
    bank_index: Vec<(Did, usize)>,
    bank_samples: usize,
    sample_budget: usize,
    sync: SyncGroup,
    /// The song name of the local player's waiting synced part.
    waiting_name: Option<String>,
    lines: VecDeque<String>,
    schedule: VecDeque<ScheduledPerformance>,
    offline: Option<(Instant, f64, Vec<f32>)>,
    stats: MusicStats,
    /// (P4) The fellowship members' characters in the player.
    members: Members,
    /// (P4) The last fellowship list of the session: the members other than the player,
    /// with their entities (empty without a fellowship).
    fellows: Vec<(String, Option<tessera_net::EntityId>)>,
    /// (P4) Requests for the world session ([`Self::take_requests`]).
    requests: VecDeque<MusicRequest>,
    /// (P4) The session answers `StartPerformance` with a synchronised start
    /// (`scenario-feed.md` §6.7): `/playstart` then starts nothing locally.
    session_starts: bool,
    /// (P4) How the start time of a session's synchronised start is placed on the audio
    /// clock.
    clock: StartClock,
    /// (P4) Entries of the instruments' schedule log kept (0: off).
    schedule_capacity: usize,
    /// (P4) The last synchronised starts (at most [`MAX_SYNC_STARTS`]), for the tolerance
    /// check (`instruments.md` §8.9).
    sync_starts: VecDeque<tessera_audio::instruments::SyncStart>,
}

impl std::fmt::Debug for PlayerMusic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerMusic")
            .field("config", &self.config)
            .field("data", &self.data.is_some())
            .field("queued", &self.queue.len())
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl PlayerMusic {
    /// Player music with `config`, without game data (songs are read and checked, nothing
    /// plays) until [`Self::attach_data`].
    #[must_use]
    pub fn new(config: MusicConfig) -> Self {
        tracing::info!(folders = ?config.folders, "player music: music folders");
        Self {
            config,
            pending_set: None,
            boot: None,
            data: None,
            queue: VecDeque::new(),
            job: None,
            engine: None,
            player: None,
            bank: SampleBank::new(),
            bank_index: Vec::new(),
            bank_samples: 0,
            sample_budget: SAMPLE_BUDGET,
            sync: SyncGroup::new(),
            waiting_name: None,
            lines: VecDeque::new(),
            schedule: VecDeque::new(),
            offline: None,
            stats: MusicStats::default(),
            members: Members::default(),
            fellows: Vec::new(),
            requests: VecDeque::new(),
            session_starts: false,
            clock: StartClock::Audio,
            schedule_capacity: 0,
            sync_starts: VecDeque::new(),
        }
    }

    /// Player music with start-up data already loaded (tools and tests: a synthetic
    /// instrument table without an install).
    #[must_use]
    pub fn with_data(config: MusicConfig, data: MusicData) -> Self {
        let mut m = Self::new(config);
        m.data = Some(Arc::new(data));
        m
    }

    /// Whether the world session answers `StartPerformance` with a synchronised start
    /// (the sandbox and the scenario feed do; the test server does not,
    /// `scenario-feed.md` §9.8). With it, `/playstart` asks the session for the start
    /// ([`MusicRequest::Start`]) and starts nothing locally; without it, `/playstart` starts
    /// the waiting parts locally (`instruments.md` §12 P4 Decision 3).
    pub fn set_session_starts(&mut self, on: bool) {
        self.session_starts = on;
    }

    /// How a session's synchronised start is placed on the audio clock ([`StartClock`]).
    pub fn set_start_clock(&mut self, clock: StartClock) {
        self.clock = clock;
    }

    /// Keeps the instruments' schedule log (at most `capacity` entries) from the next
    /// performance on, for the start tolerance (`instruments.md` §8.9).
    pub fn set_schedule_log(&mut self, capacity: usize) {
        self.schedule_capacity = capacity;
        if let Some(p) = self.player.as_mut() {
            p.set_schedule_log(capacity);
        }
    }

    /// The instruments' schedule log (empty when off or before the first performance).
    #[must_use]
    pub fn scheduled_events(&self) -> &[tessera_audio::instruments::ScheduledEvent] {
        self.player
            .as_ref()
            .map_or(&[], tessera_audio::instruments::MusicPlayer::schedule_log)
    }

    /// The last synchronised starts (at most [`MAX_SYNC_STARTS`], oldest first): who
    /// started and with which time-0.
    pub fn sync_starts(&self) -> impl Iterator<Item = &tessera_audio::instruments::SyncStart> {
        self.sync_starts.iter()
    }

    fn push_request(&mut self, r: MusicRequest) {
        if self.requests.len() >= sync::MAX_SESSION_ITEMS {
            self.requests.pop_front();
        }
        self.requests.push_back(r);
    }

    /// The requests for the world session since the last call, in order (`/play … sync`
    /// accepted: `ReadyPerformance`; `/playstart`: `StartPerformance`).
    pub fn take_requests(&mut self) -> Vec<MusicRequest> {
        self.requests.drain(..).collect()
    }

    /// The session refused the `StartPerformance` of a `/playstart` (the test server,
    /// decision 0035 D2): the waiting parts start locally (`instruments.md` §12 P4
    /// Decision 3), behind the commands still queued.
    pub fn start_refused(&mut self) {
        self.push_queued(Queued::Social(SocialItem::LocalStart));
    }

    /// Starts the engine and the player now (tests and headless drivers that run the audio
    /// clock in step with a virtual clock). Returns whether they run.
    pub fn start_engine(&mut self) -> bool {
        self.pump();
        self.ensure_player()
    }

    /// The engine's audio clock, seconds, once it runs.
    #[must_use]
    pub fn audio_clock(&self) -> Option<f64> {
        self.engine.as_ref().map(MusicPlayer::now)
    }

    /// Gives the archives. Start-up (the instrument table, the message table, the equipped
    /// item, the volumes and the settings folder) runs on a background thread at the next
    /// player-music command, or at [`Self::load_now`]; commands wait for it. `install` is
    /// the opened install's directory: without `--game-dir` its settings folder's `Music`
    /// is looked up at start-up. Reads no file.
    pub fn attach_data(&mut self, set: Arc<ArchiveSet>, install: &Path) {
        if self.config.game_dir.is_none() {
            self.config.game_dir = Some(install.to_path_buf());
            self.config.settings_lookup = Some(install.to_path_buf());
        }
        self.pending_set = Some(set);
    }

    /// Starts start-up now: with the archives given by [`Self::attach_data`] if they have
    /// not been loaded yet, else (without archives) once for the volumes and the settings
    /// folder. No-op while start-up runs or when there is nothing new to load.
    pub fn load_now(&mut self) {
        if self.boot.is_some() || (self.pending_set.is_none() && self.data.is_some()) {
            return;
        }
        let set = self.pending_set.take();
        let (tx, rx) = mpsc::sync_channel(1);
        let config = self.config.clone();
        let spawned = std::thread::Builder::new()
            .name("player-music-boot".into())
            .spawn(move || {
                let _ = tx.send(MusicData::load(set, &config));
            });
        match spawned {
            Ok(_) => self.boot = Some(rx),
            Err(e) => tracing::warn!("player music: start-up thread: {e}"),
        }
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &MusicConfig {
        &self.config
    }

    /// Start-up data, once loaded.
    #[must_use]
    pub fn data(&self) -> Option<&MusicData> {
        self.data.as_deref()
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> MusicStats {
        self.stats
    }

    /// The instruments' counters, once a performance has started.
    #[must_use]
    pub fn player_stats(&self) -> Option<PlayerStats> {
        self.player.as_ref().map(MusicPlayer::stats)
    }

    /// Whether the local player's performance is queued or sounding.
    #[must_use]
    pub fn is_performing(&self) -> bool {
        self.player
            .as_ref()
            .is_some_and(|p| p.is_performing(LOCAL_PLAYER))
    }

    /// Whether the local player has a synced part waiting for `/playstart`.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.sync.is_waiting(LOCAL_PLAYER)
    }

    /// The engine, once started.
    #[must_use]
    pub fn engine(&self) -> Option<&AudioEngine> {
        self.engine.as_ref()
    }

    /// The most recent performances as scheduled (at most [`MAX_SCHEDULE_LOG`]).
    pub fn schedule_log(&self) -> impl Iterator<Item = &ScheduledPerformance> {
        self.schedule.iter()
    }

    /// Whether start-up, a command job or a queued command is still pending.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.boot.is_some() || self.job.is_some() || !self.queue.is_empty()
    }

    /// Chat lines produced since the last call (errors, notices, listings).
    pub fn take_lines(&mut self) -> Vec<String> {
        self.lines.drain(..).collect()
    }

    fn say(&mut self, line: String) {
        tracing::debug!("player music: {line}");
        if self.lines.len() >= MAX_PENDING_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    fn messages(&self) -> Option<&Messages> {
        self.data.as_ref().and_then(|d| d.messages.as_ref())
    }

    /// The entry point of the command dispatcher (decision 0020 item 2): runs `word` with
    /// its argument tokens (`chat-window.md` §7.3, the command word excluded). A refused
    /// command gives a "Music: usage: ..." line.
    pub fn run_tokens(&mut self, word: MusicWord, args: &[String]) {
        self.stats.commands += 1;
        match parse_args(word, args) {
            Ok(cmd) => self.run(cmd),
            Err(e) => self.say(format!("Music: {e}")),
        }
    }

    /// Runs a typed line with the `/` prefix through the client's command-line pipeline
    /// (`crate::commands::line`, `chat-window.md` §7.2–§7.4) when its command word is a
    /// player-music word, for tests and tools without a dispatcher. Returns `false` when
    /// it is not a player-music command (nothing happens then).
    pub fn command(&mut self, line: &str) -> bool {
        let Ok(Some(inv)) =
            crate::commands::line::prepare(line, crate::commands::line::DEFAULT_PREFIX, "", "")
        else {
            return false;
        };
        let Some(word) = MusicWord::from_name(&inv.word).filter(|_| !inv.plain) else {
            return false;
        };
        self.run_tokens(word, inv.args_tokens());
        true
    }

    /// Queues a parsed command behind start-up and the earlier commands (module docs,
    /// "Order") and processes what is ready.
    pub fn run(&mut self, cmd: MusicCommand) {
        self.load_now();
        if self.queue.len() >= MAX_QUEUED {
            self.stats.busy += 1;
            self.say("Music: busy; try again".into());
            return;
        }
        self.queue.push_back(Queued::Command(cmd));
        self.pump();
    }

    /// Queues a session item behind the earlier work (never refused for a full queue: the
    /// session's items are bounded by [`sync::MAX_SESSION_ITEMS`]).
    fn push_queued(&mut self, q: Queued) {
        self.load_now();
        let session = self
            .queue
            .iter()
            .filter(|x| matches!(x, Queued::Social(_)))
            .count();
        if matches!(q, Queued::Social(_)) && session >= sync::MAX_SESSION_ITEMS {
            tracing::warn!("player music: too many session items waiting; one dropped");
            return;
        }
        self.queue.push_back(q);
        self.pump();
    }

    /// A performance or fellowship event of the world session (`scenario-feed.md` §6.7,
    /// `instruments.md` §12 P4 Decision): a member's part made ready, read from the music
    /// folders like a `/play`; a synchronised start; a withdrawn part; the fellowship's new
    /// member list (absent members' parts withdrawn); a member's performance stopped.
    /// `player` is the local character's name and entity: a `PerformanceReady` for the
    /// player acts as an accepted `/play … sync` of the player. Items wait behind the
    /// commands before them.
    pub fn social_event(
        &mut self,
        ev: &tessera_net::Event,
        player: &str,
        player_entity: Option<tessera_net::EntityId>,
    ) {
        if let Some(item) = SocialItem::from_event(ev, player, player_entity) {
            self.push_queued(Queued::Social(item));
        }
    }

    /// Processes queued commands in order until one waits for start-up or a job.
    fn pump(&mut self) {
        loop {
            if let Some(rx) = &self.boot {
                match rx.try_recv() {
                    Ok(data) => self.boot_done(data),
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => {
                        self.boot = None;
                        tracing::warn!("player music: start-up ended without a result");
                    }
                }
            }
            if let Some(rx) = &self.job {
                match rx.try_recv() {
                    Ok(r) => {
                        self.job = None;
                        self.job_done(r);
                    }
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => {
                        self.job = None;
                        self.stats.failed_jobs += 1;
                        tracing::warn!("player music: a command job ended without a result");
                        self.say("Music: the command failed".into());
                    }
                }
            }
            let Some(q) = self.queue.pop_front() else {
                return;
            };
            match q {
                Queued::Command(cmd) => self.dispatch(cmd),
                Queued::Social(item) => self.social_item(item),
            }
        }
    }

    fn social_item(&mut self, item: SocialItem) {
        match item {
            SocialItem::Ready {
                member,
                entity,
                tune,
            } => match member {
                None => self.read_song(tune, None, true, None, true, None),
                Some(name) => {
                    let id = self.members.id_of(&name).or_else(|| {
                        self.prune_members();
                        self.members.id_of(&name)
                    });
                    match id {
                        Some(id) => {
                            // The entity of the event, else of the last fellowship list, so a
                            // later `PerformanceStopped` finds the part (Decision 6).
                            let entity = entity.or_else(|| {
                                self.fellows
                                    .iter()
                                    .find(|(n, _)| n.to_lowercase() == name.to_lowercase())
                                    .and_then(|(_, e)| *e)
                            });
                            if entity.is_some() {
                                self.members.set_entity(&name, entity);
                            }
                            self.read_song(tune, None, true, Some((name, id)), true, None);
                        }
                        None => {
                            tracing::warn!(
                                "player music: too many members with parts; part refused"
                            );
                            self.say(format!(
                                "Music: the part of {name} cannot wait (too many performers)"
                            ));
                        }
                    }
                }
            },
            SocialItem::Started {
                performer,
                tune,
                delay_ms,
            } => match performer {
                None => self.read_song(tune, None, false, None, true, Some(delay_ms)),
                Some(entity) => {
                    let id = self.members.id_of_entity(entity).or_else(|| {
                        self.prune_members();
                        self.members.id_of_entity(entity)
                    });
                    match id {
                        Some(id) => {
                            let name = self.members.name_of(id).unwrap_or_default();
                            self.read_song(
                                tune,
                                None,
                                false,
                                Some((name, id)),
                                true,
                                Some(delay_ms),
                            );
                        }
                        None => {
                            tracing::warn!(
                                "player music: too many performers; performance refused"
                            );
                            self.say(
                                "Music: a performance cannot start (too many performers)".into(),
                            );
                        }
                    }
                }
            },
            SocialItem::Start { lead_ms, at_ms } => self.session_start(lead_ms, at_ms),
            SocialItem::LocalStart => self.play_start(),
            SocialItem::Withdraw { member } => {
                let id = match member {
                    None => Some(LOCAL_PLAYER),
                    Some(n) => self.members.find(&n),
                };
                if let Some(id) = id {
                    self.sync.withdraw(id);
                    if id == LOCAL_PLAYER {
                        self.waiting_name = None;
                    }
                }
            }
            SocialItem::Fellowship { members } => {
                for (n, e) in &members {
                    self.members.set_entity(n, *e);
                }
                let present: Vec<CharacterId> = members
                    .iter()
                    .filter_map(|(n, _)| self.members.find(n))
                    .collect();
                let gone = self.sync.withdraw_absent_members(&present);
                if !gone.is_empty() {
                    tracing::debug!(
                        withdrawn = gone.len(),
                        "player music: members left; parts withdrawn"
                    );
                }
                self.fellows = members;
                self.prune_members();
            }
            SocialItem::Stopped { entity } => {
                if let Some(id) = self.members.by_entity(entity)
                    && let Some(player) = self.player.as_mut()
                {
                    self.sync.cancel(player, id);
                }
            }
        }
    }

    /// Frees the member characters that are neither in the last fellowship list, nor
    /// waiting, nor performing: [`sync::MAX_MEMBERS`] bounds the performers at one time,
    /// not over the client's lifetime.
    fn prune_members(&mut self) {
        let fellows: HashSet<String> = self.fellows.iter().map(|(n, _)| n.to_lowercase()).collect();
        let sync = &self.sync;
        let player = self.player.as_ref();
        let freed = self.members.retain(|name, id| {
            fellows.contains(&name.to_lowercase())
                || sync.is_waiting(id)
                || player.is_some_and(|p| p.is_performing(id))
        });
        if let (Some(engine), Some(player)) = (self.engine.as_mut(), self.player.as_mut()) {
            for id in &freed {
                if player.has_character(*id) {
                    player.remove_character(engine, *id);
                }
            }
        }
    }

    /// The fellowship members' (and other performers') characters allocated now (at most
    /// [`sync::MAX_MEMBERS`]).
    #[must_use]
    pub fn member_characters(&self) -> usize {
        self.members.len()
    }

    /// The parts waiting for a synchronised start: the local player's and the members'.
    #[must_use]
    pub fn waiting_parts(&self) -> usize {
        self.sync.len()
    }

    /// A synchronised start of the session (`scenario-feed.md` §6.7, `instruments.md`
    /// §8.8.3, §12 P4 Decision 3–4): the start notice once, then every waiting part with the
    /// one start time *S* of [`Self::set_start_clock`]'s rule.
    fn session_start(&mut self, lead_ms: u32, at_ms: u64) {
        let notice = notice_text(self.messages(), &Notice::SyncedStart);
        self.say(notice);
        if self.sync.is_empty() || !self.ensure_player() {
            return;
        }
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        let now = MusicPlayer::now(engine);
        let start = self.clock.start_secs(now, lead_ms, at_ms);
        self.start_waiting(start, now);
    }

    fn dispatch(&mut self, cmd: MusicCommand) {
        match cmd {
            MusicCommand::Play { name, part, sync } => self.start_play(name, part, sync),
            MusicCommand::PlayStart if self.session_starts => {
                // The session's start starts the waiting parts (`scenario-feed.md` §6.7):
                // nothing starts here, so one `/playstart` gives one start.
                self.push_request(MusicRequest::Start);
            }
            MusicCommand::PlayStart => self.play_start(),
            MusicCommand::PlayList { filter } => self.start_list(filter),
            MusicCommand::Stop => self.stop(),
            MusicCommand::Instrument { choice } => self.instrument_command(choice),
        }
    }

    /// The instrument type `/play` uses now: the debug choice, else the equipped item's.
    #[must_use]
    pub fn instrument(&self) -> Option<u32> {
        let table = self.data.as_ref()?.table.as_ref().ok()?;
        match &self.config.instrument {
            Some(c) => resolve_instrument(c, table.vocabulary()).ok().flatten(),
            None => None,
        }
        .or_else(|| self.data.as_ref()?.equipped)
    }

    fn instrument_label(&self, key: u32) -> String {
        self.data
            .as_ref()
            .and_then(|d| d.table.as_ref().ok())
            .and_then(|t| t.vocabulary().instrument_label(key).map(str::to_owned))
            .map_or_else(|| format!("{key}"), |l| format!("{l} ({key})"))
    }

    fn instrument_command(&mut self, choice: Option<String>) {
        let Some(table) = self
            .data
            .as_ref()
            .and_then(|d| d.table.as_ref().ok())
            .cloned()
        else {
            self.say("Music: no instrument data (no game data or no music scripts)".into());
            return;
        };
        match choice {
            None => {
                let current = self
                    .instrument()
                    .map_or_else(|| "none".to_owned(), |k| self.instrument_label(k));
                let source = if self.config.instrument.is_some() {
                    "debug choice"
                } else {
                    "equipped item"
                };
                self.say(format!("Music: instrument {current} ({source})"));
                let names: Vec<String> = table
                    .instruments()
                    .iter()
                    .map(|&k| self.instrument_label(k))
                    .collect();
                self.say(format!("Music: instrument types: {}", names.join(", ")));
            }
            Some(c) => match resolve_instrument(&c, table.vocabulary()) {
                Ok(None) => {
                    self.config.instrument = None;
                    let now = self
                        .instrument()
                        .map_or_else(|| "none".to_owned(), |k| self.instrument_label(k));
                    self.say(format!("Music: instrument from the equipped item: {now}"));
                }
                Ok(Some(k)) if known_instrument(&table, k) => {
                    self.config.instrument = Some(c);
                    let label = self.instrument_label(k);
                    self.say(format!("Music: instrument {label} (debug choice)"));
                }
                Ok(Some(_)) | Err(_) => {
                    self.say(format!("Music: unknown instrument type {:?}", c.trim()));
                }
            },
        }
    }

    /// Runs `f` on a background thread as the job of the command being processed.
    fn spawn_job(&mut self, f: impl FnOnce() -> JobResult + Send + 'static) {
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("player-music-job".into())
            .spawn(move || {
                let _ = tx.send(f());
            }) {
            Ok(_) => self.job = Some(rx),
            Err(e) => self.say(format!("Music: cannot start a job: {e}")),
        }
    }

    fn start_play(&mut self, name: String, part: Option<i32>, sync: bool) {
        self.read_song(name, part, sync, None, false, None);
    }

    /// Reads a song on a job: the local player's (`member` none) or a fellowship member's
    /// part. `from_feed`: the part came from the session (no `ReadyPerformance` request).
    /// `delay_ms`: a `PerformanceStarted`, started solo-like at the time its result is
    /// processed plus the delay.
    fn read_song(
        &mut self,
        name: String,
        part: Option<i32>,
        sync: bool,
        member: Option<(String, CharacterId)>,
        from_feed: bool,
        delay_ms: Option<u32>,
    ) {
        let folders = self.config.folders.clone();
        // Only another performer's part falls back to the table's first instrument (their
        // instruments are not in the session's events, `instruments.md` §12 P4 OPEN); the
        // local player's own part needs an instrument (M6: otherwise nothing plays).
        let instrument = match member {
            Some(_) => self.instrument().or_else(|| self.any_instrument()),
            None => self.instrument(),
        };
        let data = self.data.clone();
        // Commands run one at a time, so nothing changes the bank before this job's result
        // is applied.
        let have: HashSet<Did> = self.bank_index.iter().map(|&(d, _)| d).collect();
        self.spawn_job(move || {
            let t0 = Instant::now();
            let read = read_from_folders(&folders, &name, PartRequest::from(part));
            let messages = data.as_ref().and_then(|d| d.messages.as_ref());
            let mut sample_problems = 0;
            let mut needed = Vec::new();
            let (result, samples) = match read {
                Ok(perf) => {
                    let mut samples = Vec::new();
                    if let (Some(d), Some(i)) = (&data, instrument)
                        && let Ok(table) = &d.table
                    {
                        needed = wanted_samples(table, &perf, i);
                        let (s, problems) = decode_samples(d.set.as_deref(), &needed, &have);
                        for p in problems.iter().take(8) {
                            tracing::warn!("player music: sample {p}");
                        }
                        sample_problems = problems.len();
                        samples = s;
                    }
                    (Ok(Arc::new(perf)), samples)
                }
                Err(e) => (Err(error_text(messages, &e)), Vec::new()),
            };
            JobResult::Song(Box::new(SongJob {
                name,
                sync,
                member,
                from_feed,
                delay_ms,
                instrument,
                result,
                needed,
                samples,
                sample_problems,
                secs: t0.elapsed().as_secs_f64(),
            }))
        });
    }

    fn start_list(&mut self, filter: Option<String>) {
        let folders = self.config.folders.clone();
        self.spawn_job(move || JobResult::Lines(playlist_lines(&folders, filter.as_deref())));
    }

    fn stop(&mut self) {
        self.sync.withdraw(LOCAL_PLAYER);
        self.waiting_name = None;
        if let (Some(engine), Some(player)) = (self.engine.as_mut(), self.player.as_mut()) {
            player.stop_now(engine, LOCAL_PLAYER);
        }
        self.say("Music: stopped".into());
    }

    fn play_start(&mut self) {
        let notice = notice_text(self.messages(), &Notice::SyncedStart);
        self.say(notice);
        if self.sync.is_empty() || !self.ensure_player() {
            return;
        }
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        let now = MusicPlayer::now(engine);
        self.start_waiting(SyncGroup::start_time(now), now);
    }

    /// Starts every waiting part with time-0 `start` (`instruments.md` §12 P4 Decision
    /// 3–4), scheduled at audio-clock time `now`.
    fn start_waiting(&mut self, start: f64, now: f64) {
        let waiting: Vec<_> = self.sync.waiting().to_vec();
        let Some(player) = self.player.as_mut() else {
            return;
        };
        match self.sync.start_at(player, start, now) {
            Ok(s) => {
                self.stats.synced_starts += s.characters.len() as u64;
                for r in &s.refused {
                    tracing::info!(character = %r.character, "player music: a waiting part did not fit the queue");
                }
                for w in waiting
                    .iter()
                    .filter(|w| s.characters.contains(&w.character))
                {
                    let name = if w.character == LOCAL_PLAYER {
                        self.waiting_name.take().unwrap_or_default()
                    } else {
                        self.members.name_of(w.character).unwrap_or_default()
                    };
                    self.log_schedule(name, &w.performance, w.instrument, StartMode::Synced, start);
                }
                for r in &s.refused {
                    let who = self.members.name_of(r.character).unwrap_or_default();
                    self.say(format!(
                        "Music: a waiting part ({who}) did not fit and does not play"
                    ));
                }
                if self.sync_starts.len() >= MAX_SYNC_STARTS {
                    self.sync_starts.pop_front();
                }
                self.sync_starts.push_back(s);
            }
            Err(e) => self.say(format!("Music: synchronised start refused: {e}")),
        }
    }

    /// The first instrument of the table (a member's part plays on it when the local
    /// player has none: members' instruments are not in the session's events).
    fn any_instrument(&self) -> Option<u32> {
        let table = self.data.as_ref()?.table.as_ref().ok()?;
        table.instruments().first().copied()
    }

    /// Starts the engine and the player (once the instrument table exists). The volumes
    /// were resolved at start-up; only the engine (the output device) starts here.
    fn ensure_player(&mut self) -> bool {
        if self.player.is_some() {
            return true;
        }
        let Some(data) = self.data.clone() else {
            return false;
        };
        let Ok(table) = data.table.clone() else {
            return false;
        };
        let mut engine = backend::start_engine(self.config.backend);
        engine.set_settings(data.mixer.clone());
        if engine.backend() == BackendKind::Offline {
            self.offline = Some((Instant::now(), 0.0, vec![0.0; MAX_OFFLINE_FRAMES * 2]));
        }
        let mut player = MusicPlayer::new(
            table,
            PlayerConfig {
                seed: self.config.seed,
                ..PlayerConfig::default()
            },
        );
        if let Err(e) = player.add_character(LOCAL_PLAYER, LivePosition::new([0.0; 3])) {
            tracing::warn!("player music: {e}");
            return false;
        }
        if self.schedule_capacity > 0 {
            player.set_schedule_log(self.schedule_capacity);
        }
        self.engine = Some(engine);
        self.player = Some(player);
        true
    }

    /// Keeps a new performance's decoded `samples` in the bank. `needed` lists every
    /// payload the performance uses; `replacing_solo` says the local player's running
    /// performance and waiting part are about to be replaced (a solo `/play`). When the
    /// budget would be exceeded, payloads the performance does not need are dropped first
    /// unless a performance that keeps going may still use them. Returns how many samples
    /// did not fit.
    fn keep_samples(
        &mut self,
        needed: &[Did],
        samples: Vec<(Did, Pcm)>,
        replacing_solo: bool,
    ) -> usize {
        let adding: usize = samples.iter().map(|(_, p)| p.samples.len()).sum();
        if self.bank_samples.saturating_add(adding) > self.sample_budget {
            // Only the local player performs here; a waiting part of the local player is
            // replaced by this performance in either mode.
            let only_local_waits = self
                .sync
                .waiting()
                .iter()
                .all(|w| w.character == LOCAL_PLAYER);
            let performing = self
                .player
                .as_ref()
                .is_some_and(|p| p.is_performing(LOCAL_PLAYER));
            if only_local_waits && (replacing_solo || !performing) {
                let keep: HashSet<Did> = needed.iter().copied().collect();
                let bank = &mut self.bank;
                let mut freed = 0usize;
                self.bank_index.retain(|&(did, n)| {
                    if keep.contains(&did) {
                        true
                    } else {
                        bank.remove(did);
                        freed += n;
                        false
                    }
                });
                self.bank_samples = self.bank_samples.saturating_sub(freed);
            }
        }
        let mut dropped = 0;
        for (did, pcm) in samples {
            let n = pcm.samples.len();
            if self.bank.contains(did) {
                continue;
            }
            if self.bank_samples.saturating_add(n) > self.sample_budget {
                dropped += 1;
                continue;
            }
            self.bank.insert(did, pcm);
            self.bank_index.push((did, n));
            self.bank_samples += n;
            self.stats.samples_decoded += 1;
        }
        dropped
    }

    fn finish_song(&mut self, job: SongJob) {
        let perf = match job.result {
            Ok(p) => p,
            Err(line) => {
                self.stats.songs_refused += 1;
                match &job.member {
                    // A member's part that cannot be read does not join (Tessera's own
                    // line, `instruments.md` §12 P4 Decision 5).
                    Some((member, _)) => {
                        tracing::info!(member, tune = %job.name, "player music: member part refused: {line}");
                        self.say(format!(
                            "Music: the part {} of {member} cannot be played",
                            job.name
                        ));
                    }
                    None => self.say(line),
                }
                return;
            }
        };
        self.stats.songs_accepted += 1;
        self.stats.sample_problems += job.sample_problems as u64;
        tracing::info!(
            name = %job.name,
            part = ?perf.part,
            title = %perf.title,
            events = perf.events.len(),
            samples = job.samples.len(),
            read_secs = job.secs,
            "player music: song read"
        );
        if self.data.as_ref().is_none_or(|d| d.table.is_err()) {
            self.say(
                "Music: no instrument data (no game data or no music scripts); nothing plays"
                    .into(),
            );
            return;
        }
        let Some(instrument) = job.instrument else {
            self.say(
                "Music: no instrument equipped; equip one (--avatar) or choose one with /instrument"
                    .into(),
            );
            return;
        };
        let dropped = self.keep_samples(&job.needed, job.samples, !job.sync);
        if dropped > 0 {
            self.stats.sample_problems += dropped as u64;
            tracing::warn!(dropped, "player music: samples over the sample budget");
            self.say(format!(
                "Music: {dropped} instrument samples did not fit in memory; some notes are silent"
            ));
        }
        if let Some((member, id)) = job.member {
            // A fellowship member's part (`instruments.md` §12 P4 Decision 5).
            if !self.ensure_player() {
                return;
            }
            if let Some(p) = self.player.as_mut()
                && !p.has_character(id)
                && let Err(e) = p.add_character(id, LivePosition::new([0.0; 3]))
            {
                self.say(format!("Music: {e}"));
                return;
            }
            if let Some(delay) = job.delay_ms {
                // `PerformanceStarted` of another performer: S = t + delay.
                let (Some(engine), Some(player)) = (self.engine.as_mut(), self.player.as_mut())
                else {
                    return;
                };
                let start = MusicPlayer::now(engine) + f64::from(delay) / 1e3;
                match player.play(id, &perf, instrument, start) {
                    Ok(_) => {
                        self.stats.solo_starts += 1;
                        self.log_schedule(job.name, &perf, instrument, StartMode::Solo, start);
                    }
                    Err(e) => self.say(format!("Music: the part of {member}: {e}")),
                }
                return;
            }
            match self.sync.register_member(id, Arc::clone(&perf), instrument) {
                Ok(()) => {
                    self.stats.synced_waits += 1;
                    let n = sync::member_ready_notice(&member, &perf.title);
                    let line = sync::render(self.messages(), &n);
                    self.say(line);
                }
                Err(e) => self.say(format!("Music: {e}")),
            }
            return;
        }
        if job.sync {
            match self
                .sync
                .register(LOCAL_PLAYER, Arc::clone(&perf), instrument)
            {
                Ok(()) => {
                    self.stats.synced_waits += 1;
                    tracing::info!(name = %job.name, "player music: waiting for /playstart");
                    // The own ready notice (`instruments.md` §8.8.6, §12 P4 Decision 1) and,
                    // for the player's own `/play … sync`, the session is told (§6.7).
                    let n = sync::own_ready_notice(&perf.title);
                    let line = sync::render(self.messages(), &n);
                    self.say(line);
                    if !job.from_feed {
                        self.push_request(MusicRequest::Ready {
                            tune: job.name.clone(),
                        });
                    }
                    self.waiting_name = Some(job.name);
                }
                Err(e) => self.say(format!("Music: {e}")),
            }
            return;
        }
        self.sync.withdraw(LOCAL_PLAYER);
        self.waiting_name = None;
        if !self.ensure_player() {
            return;
        }
        let (Some(engine), Some(player)) = (self.engine.as_mut(), self.player.as_mut()) else {
            return;
        };
        // A `PerformanceStarted` for the player: S = t + its delay.
        let start = MusicPlayer::now(engine)
            + job
                .delay_ms
                .map_or(SOLO_START_OFFSET, |d| f64::from(d) / 1e3);
        match player.play(LOCAL_PLAYER, &perf, instrument, start) {
            Ok(_) => {
                self.stats.solo_starts += 1;
                let notice = notice_text(self.messages(), &perf.start_notice(StartMode::Solo));
                self.say(notice);
                self.log_schedule(job.name, &perf, instrument, StartMode::Solo, start);
            }
            Err(e) => self.say(format!("Music: {e}")),
        }
    }

    fn log_schedule(
        &mut self,
        name: String,
        perf: &Performance,
        instrument: u32,
        mode: StartMode,
        start_secs: f64,
    ) {
        let note_on_times: Vec<f64> = perf.note_ons().map(|e| e.time).collect();
        self.stats.note_ons_scheduled += note_on_times.len() as u64;
        tracing::info!(
            name = %name,
            title = %perf.title,
            instrument,
            ?mode,
            start_secs,
            note_ons = note_on_times.len(),
            first = note_on_times.first().copied().unwrap_or(0.0),
            last = note_on_times.last().copied().unwrap_or(0.0),
            end = perf.end_time(),
            "player music: performance scheduled"
        );
        if self.schedule.len() >= MAX_SCHEDULE_LOG {
            self.schedule.pop_front();
        }
        self.schedule.push_back(ScheduledPerformance {
            name,
            title: perf.title.clone(),
            part: perf.part,
            instrument,
            mode,
            start_secs,
            events: perf.events.len(),
            end_time: perf.end_time(),
            note_on_times,
        });
    }

    fn boot_done(&mut self, data: MusicData) {
        self.boot = None;
        match &data.table {
            Ok(t) => tracing::info!(
                secs = data.secs,
                instruments = t.instruments().len(),
                sounds = t.sounds().len(),
                messages = data.messages.as_ref().and_then(Messages::table).is_some(),
                equipped = ?data.equipped,
                "player music: ready"
            ),
            Err(e) => tracing::info!(secs = data.secs, "player music: no instrument table: {e}"),
        }
        if let Some(f) = &data.settings_music {
            if !self.config.folders.contains(f) {
                self.config.folders.insert(0, f.clone());
            }
            self.config.settings_lookup = None;
            tracing::info!(folders = ?self.config.folders, "player music: music folders");
        }
        self.data = Some(Arc::new(data));
    }

    fn job_done(&mut self, r: JobResult) {
        match r {
            JobResult::Song(job) => self.finish_song(*job),
            JobResult::Lines(lines) => {
                for l in lines {
                    self.say(l);
                }
            }
        }
    }

    /// Per frame: collects start-up and job results and processes the queued commands,
    /// then runs the instruments and the engine (rendering the offline backend in real
    /// time).
    pub fn update(&mut self) {
        self.pump();
        let (Some(engine), Some(player)) = (self.engine.as_mut(), self.player.as_mut()) else {
            return;
        };
        player.update(engine, &mut self.bank);
        engine.update();
        if let Some((start, done, buf)) = &mut self.offline {
            let rate = f64::from(engine.sample_rate());
            let due = start.elapsed().as_secs_f64() * rate;
            let frames = ((due - *done).max(0.0) as usize).min(MAX_OFFLINE_FRAMES);
            if frames > 0 {
                if let Some(out) = buf.get_mut(..frames * 2)
                    && let Err(e) = engine.render(out)
                {
                    tracing::warn!("player music: offline render: {e}");
                }
                *done = due.max(*done + frames as f64);
            }
        }
        engine.drain_events().for_each(drop);
    }

    /// Blocks until start-up, every job and every queued command have finished (or
    /// `timeout` passed). For tools and tests; never on the frame thread.
    pub fn wait_idle(&mut self, timeout: Duration) -> bool {
        let deadline = crate::runtime::deadline_after(Instant::now(), timeout);
        loop {
            self.pump();
            if !self.busy() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Blocks until start-up, every job and every queued command have finished, or the
    /// wait gives up: with [`LoadWait::Progress`] only after that many polls (2 ms apart)
    /// in a row in which nothing finished (start-up, a job, a queued command), or after
    /// that many polls in all, so a loaded machine waits longer instead of failing. For
    /// tools and tests; never on the frame thread.
    pub fn wait_settled(&mut self, limit: LoadWait) -> bool {
        let mut w = Waiter::new(limit);
        loop {
            self.pump();
            if !self.busy() {
                return true;
            }
            let mark = (
                self.boot.is_some(),
                self.job.is_some(),
                self.queue.len(),
                self.stats.commands,
                self.stats.failed_jobs,
            );
            if w.give_up(mark) {
                tracing::warn!(
                    polls = w.polls(),
                    stalled = w.stalled(),
                    "player music: still busy; giving up the wait"
                );
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Renders `secs` of audio on an offline engine as fast as possible, running the
    /// instruments every 1024 frames (for tools and tests). Returns the frames rendered and
    /// the peak sample; `None` unless the engine runs the offline backend.
    pub fn render_offline(&mut self, secs: f64) -> Option<(usize, f32)> {
        const CHUNK: usize = 1024;
        let (Some(engine), Some(player)) = (self.engine.as_mut(), self.player.as_mut()) else {
            return None;
        };
        if engine.backend() != BackendKind::Offline {
            return None;
        }
        let total = (secs.max(0.0) * f64::from(engine.sample_rate())).min(1e9) as usize;
        let mut buf = vec![0.0f32; CHUNK * 2];
        let (mut done, mut peak) = (0usize, 0.0f32);
        while done < total {
            player.update(engine, &mut self.bank);
            engine.update();
            if engine.render(&mut buf).is_err() {
                break;
            }
            engine.drain_events().for_each(drop);
            peak = buf.iter().fold(peak, |m, s| m.max(s.abs()));
            done += CHUNK;
        }
        Some((done, peak))
    }
}

/// Whether `key` may be chosen with `/instrument`: 0 (the default set), a type some
/// instrument switch of the table names, or a key of the instrument-type mapper
/// (`instruments.md` §3.1).
fn known_instrument(table: &InstrumentTable, key: u32) -> bool {
    key == 0
        || table.instruments().contains(&key)
        || table
            .vocabulary()
            .instrument_labels
            .iter()
            .any(|(k, _)| *k == key)
}

/// The `/playlist` lines for the folders (`abc-dialect.md` §11): a header per folder that
/// exists, then one line per matching song (`name: X:n title, ...`). The filter matches the
/// file name or a title, ignoring ASCII case (Tessera's reading of §11 OPEN (2)). A song
/// whose `/play` name (ignoring ASCII case, as the lookup does) an earlier folder already
/// has is left out: `/play` would open the earlier folder's file. Blocking file I/O.
#[must_use]
pub fn playlist_lines(folders: &[PathBuf], filter: Option<&str>) -> Vec<String> {
    let folders: Vec<&PathBuf> = folders.iter().filter(|f| f.is_dir()).collect();
    if folders.is_empty() {
        return vec!["Music: no music folder".to_owned()];
    }
    let needle = filter.map(str::to_ascii_lowercase);
    let matches = |s: &SongListing| {
        let Some(n) = &needle else { return true };
        let has = |t: &str| t.to_ascii_lowercase().contains(n.as_str());
        has(&s.file_name)
            || s.file_title.as_deref().is_some_and(has)
            || s.parts.iter().any(|p| p.title.as_deref().is_some_and(has))
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for folder in folders {
        let songs = match abc::list_songs(folder) {
            Ok(s) => s,
            Err(e) => {
                out.push(fallback_error(&e));
                continue;
            }
        };
        let shown: Vec<&SongListing> = songs
            .iter()
            .filter(|s| !seen.contains(&s.name.to_ascii_lowercase()))
            .filter(|s| matches(s))
            .collect();
        seen.extend(songs.iter().map(|s| s.name.to_ascii_lowercase()));
        out.push(format!(
            "Music: {} ({} songs)",
            folder.display(),
            shown.len()
        ));
        for s in shown.iter().take(MAX_PLAYLIST_SONGS) {
            let mut parts: Vec<String> = s
                .parts
                .iter()
                .take(MAX_PLAYLIST_PARTS)
                .map(|p| match &p.title {
                    Some(t) => format!("X:{} {t}", p.number),
                    None => format!("X:{}", p.number),
                })
                .collect();
            if s.parts.len() > MAX_PLAYLIST_PARTS {
                parts.push(format!("… {} more", s.parts.len() - MAX_PLAYLIST_PARTS));
            }
            let title = s
                .file_title
                .as_deref()
                .map(|t| format!(" \"{t}\""))
                .unwrap_or_default();
            out.push(format!("  {}{title}: {}", s.name, parts.join(", ")));
        }
        if shown.len() > MAX_PLAYLIST_SONGS {
            out.push(format!("  … {} more", shown.len() - MAX_PLAYLIST_SONGS));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_audio::abc::{ARG_COLUMN, ARG_LINE, Position};
    use tessera_text::MemorySource;

    /// An original test tune (written for these tests).
    const TUNE: &str = "X:1\nT:Lantern Steps\nM:4/4\nL:1/8\nQ:120\nK:C\nC D E F|G2 c2|\n\
                        X:2\nT:Second Part\nK:G\n[GB] d|\n";

    fn folder_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().expect("tempdir");
        for (name, body) in files {
            std::fs::write(d.path().join(name), body).expect("write");
        }
        d
    }

    fn vle(out: &mut Vec<u8>, v: u32) {
        tessera_formats::reader::write_vle(out, v);
    }

    /// A string table (records.md §3) with one entry per (token, pieces, argument IDs).
    fn string_table(did: u32, entries: &[(u32, &[&str], &[u32])]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&did.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.push(0);
        vle(&mut b, entries.len() as u32);
        for (token, pieces, args) in entries {
            b.extend_from_slice(&token.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&(pieces.len() as u32).to_le_bytes());
            for p in *pieces {
                let units: Vec<u16> = p.encode_utf16().collect();
                vle(&mut b, units.len() as u32);
                for u in units {
                    b.extend_from_slice(&u.to_le_bytes());
                }
            }
            b.extend_from_slice(&(args.len() as u32).to_le_bytes());
            for a in *args {
                b.extend_from_slice(&a.to_le_bytes());
            }
            b.push(0);
        }
        b
    }

    const TABLE: u32 = 0x2500_0042;

    fn synthetic_messages() -> Messages {
        let meter = AbcError {
            kind: AbcErrorKind::MeterMalformed,
            position: Some(Position { line: 3, column: 7 }),
            path: None,
        };
        let meter_token = meter.message().expect("token").token;
        let untitled = Notice::NowPlayingUntitled.message().expect("token").token;
        let mut src = MemorySource::new();
        src.insert(
            TABLE,
            string_table(
                TABLE,
                &[
                    (
                        meter_token,
                        &["Odd meter, line ", " col ", "."],
                        &[ARG_LINE, ARG_COLUMN],
                    ),
                    (untitled, &["Tune begins."], &[]),
                ],
            ),
        );
        Messages {
            strings: Strings::new(src),
            table: Some(TABLE),
        }
    }

    #[test]
    fn messages_come_from_the_table_with_arguments() {
        let m = synthetic_messages();
        let meter = AbcError {
            kind: AbcErrorKind::MeterMalformed,
            position: Some(Position { line: 3, column: 7 }),
            path: None,
        };
        assert_eq!(error_text(Some(&m), &meter), "Odd meter, line 3 col 7.");
        assert_eq!(
            notice_text(Some(&m), &Notice::NowPlayingUntitled),
            "Tune begins."
        );
        // An entry the table lacks, and no table at all: Tessera's own text.
        assert_eq!(
            notice_text(Some(&m), &Notice::SyncedStart),
            fallback_notice(&Notice::SyncedStart)
        );
        let tempo = AbcError {
            kind: AbcErrorKind::TempoMalformed,
            position: Some(Position { line: 1, column: 2 }),
            path: None,
        };
        assert_eq!(error_text(Some(&m), &tempo), fallback_error(&tempo));
        assert_eq!(error_text(None, &meter), fallback_error(&meter));
        let none = Messages {
            strings: Strings::new(MemorySource::new()),
            table: None,
        };
        assert_eq!(error_text(Some(&none), &meter), fallback_error(&meter));
        // The event limit has no table entry (§10.4 OPEN).
        let many = AbcError {
            kind: AbcErrorKind::TooManyEvents,
            position: None,
            path: None,
        };
        assert_eq!(
            error_text(Some(&m), &many),
            fallback_notice(&Notice::EventLimit)
        );
    }

    #[test]
    fn equipped_instrument_follows_section_4_5() {
        let lute = ItemFacts {
            category: Some(EQUIPMENT_CATEGORY_INSTRUMENT),
            slot: Some(0x2_0000),
            phys_obj: true,
            instrument_type: Some(4),
        };
        assert!(lute.sets_instrument());
        assert_eq!(equipped_instrument(&[lute]), Some(4));
        // No type: 0 (Undef). No valid PhysObj: 0.
        let untyped = ItemFacts {
            instrument_type: None,
            ..lute
        };
        assert_eq!(equipped_instrument(&[untyped]), Some(0));
        let no_phys = ItemFacts {
            phys_obj: false,
            ..lute
        };
        assert_eq!(equipped_instrument(&[no_phys]), Some(0));
        // Category must equal the whole value; the slot needs one of the three bits.
        let other_category = ItemFacts {
            category: Some(EQUIPMENT_CATEGORY_INSTRUMENT | 1),
            ..lute
        };
        let chest_slot = ItemFacts {
            slot: Some(0x8),
            ..lute
        };
        let no_slot = ItemFacts { slot: None, ..lute };
        assert_eq!(
            equipped_instrument(&[other_category, chest_slot, no_slot]),
            None
        );
        // The last instrument equipped wins.
        let horn = ItemFacts {
            instrument_type: Some(2),
            slot: Some(0x1_0000),
            ..lute
        };
        assert_eq!(equipped_instrument(&[lute, chest_slot, horn]), Some(2));
        assert_eq!(equipped_instrument(&[]), None);
    }

    #[test]
    fn instrument_choices() {
        let v = MusicVocabulary {
            instrument_labels: vec![(4, "Lute".into()), (9, "Bagpipe".into())],
            ..MusicVocabulary::default()
        };
        assert_eq!(resolve_instrument("lute", &v), Ok(Some(4)));
        assert_eq!(resolve_instrument("BAGPIPE", &v), Ok(Some(9)));
        assert_eq!(resolve_instrument("7", &v), Ok(Some(7)));
        assert_eq!(resolve_instrument("0x1A", &v), Ok(Some(26)));
        assert_eq!(resolve_instrument("item", &v), Ok(None));
        assert_eq!(resolve_instrument("kazoo", &v), Err("kazoo".into()));
    }

    #[test]
    fn folders_are_searched_in_order() {
        let original = folder_with(&[("tune.abc", TUNE)]);
        let own = folder_with(&[("tune.txt", "X:1\nT:Other\nK:C\nC|\n"), ("mine.txt", TUNE)]);
        let folders = vec![original.path().to_path_buf(), own.path().to_path_buf()];
        assert_eq!(
            find_in_folders(&folders, "tune").unwrap(),
            original.path().join("tune.abc")
        );
        assert_eq!(
            find_in_folders(&folders, "mine").unwrap(),
            own.path().join("mine.txt")
        );
        let e = find_in_folders(&folders, "absent").unwrap_err();
        assert_eq!(e.kind, AbcErrorKind::FileNotFound);
        assert_eq!(e.path, Some(original.path().join("absent.txt")));
        let p = read_from_folders(&folders, "mine", PartRequest::Number(2)).unwrap();
        assert_eq!(p.title, "Second Part");
        let p = read_from_folders(&folders, "tune", PartRequest::Unspecified).unwrap();
        assert_eq!(p.title, "Lantern Steps");
        assert_eq!(p.note_ons().count(), 6);
        // No folder: the name joined to `Music` as `.txt`, without a file-system lookup
        // (a `Music/tune.abc` in the working directory must not matter).
        let e = find_in_folders(&[], "tune").unwrap_err();
        assert_eq!(e.kind, AbcErrorKind::FileNotFound);
        assert_eq!(e.path, Some(PathBuf::from("Music/tune.txt")));
        // A folder that does not exist: its `.txt` path is still the one reported.
        let gone = original.path().join("missing");
        let e = find_in_folders(std::slice::from_ref(&gone), "tune").unwrap_err();
        assert_eq!(e.path, Some(gone.join("tune.txt")));
    }

    #[test]
    fn playlist_lists_and_filters() {
        let d = folder_with(&[
            ("tune.abc", TUNE),
            ("other.txt", "T:Whole File\nX:5\nT:Reel\nK:C\nC|\n"),
            ("notes.md", "not a song"),
        ]);
        let folders = vec![d.path().to_path_buf()];
        let all = playlist_lines(&folders, None);
        assert_eq!(all.len(), 3, "{all:?}");
        assert!(all[0].ends_with("(2 songs)"));
        assert!(
            all.iter()
                .any(|l| l.contains("tune: X:1 Lantern Steps, X:2 Second Part"))
        );
        assert!(
            all.iter()
                .any(|l| l.contains("other \"Whole File\": X:5 Reel"))
        );
        let some = playlist_lines(&folders, Some("REEL"));
        assert_eq!(some.len(), 2, "{some:?}");
        assert!(some[1].contains("other"));
        assert_eq!(playlist_lines(&[], None), vec!["Music: no music folder"]);
        // A song an earlier folder shadows by its /play name is not listed again.
        let first = folder_with(&[("tune.txt", "X:1\nT:First\nK:C\nC|\n")]);
        let second = folder_with(&[
            ("TUNE.abc", "X:1\nT:Second\nK:C\nC|\n"),
            ("own.abc", "X:1\nT:Own\nK:C\nC|\n"),
        ]);
        let two = vec![first.path().to_path_buf(), second.path().to_path_buf()];
        let lines = playlist_lines(&two, None);
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert!(lines[1].contains("tune: X:1 First"));
        assert!(lines[2].ends_with("(1 songs)"));
        assert!(lines[3].contains("own: X:1 Own"));
        assert_eq!(
            find_in_folders(&two, "tune").unwrap(),
            first.path().join("tune.txt")
        );
    }

    #[test]
    fn commands_without_game_data_read_and_report() {
        let d = folder_with(&[("tune.abc", TUNE), ("bad.abc", "X:1\nM:4/\nK:C\nC|\n")]);
        let mut m = PlayerMusic::new(MusicConfig {
            folders: vec![d.path().to_path_buf()],
            ..MusicConfig::default()
        });
        assert!(!m.command("/plugins list"));
        assert!(m.command("/play tune"));
        assert!(m.command("/play bad"));
        assert!(m.command("/play absent 2 sync"));
        assert!(m.wait_idle(Duration::from_secs(10)));
        let lines = m.take_lines();
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("no instrument data")));
        assert!(lines.iter().any(|l| l.contains("meter malformed at")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("file not found") && l.contains("absent.txt"))
        );
        assert!(m.command("/play"));
        assert_eq!(
            m.take_lines(),
            vec![format!("Music: usage: {}", command::USAGE_PLAY)]
        );
        assert!(m.command("/playlist lantern"));
        assert!(m.wait_idle(Duration::from_secs(10)));
        let lines = m.take_lines();
        assert!(
            lines.iter().any(|l| l.contains("Lantern Steps")),
            "{lines:?}"
        );
        assert!(m.command("/playstart"));
        assert_eq!(m.take_lines(), vec![fallback_notice(&Notice::SyncedStart)]);
        assert!(m.command("/playstop"));
        assert!(m.command("/instrument lute"));
        let lines = m.take_lines();
        assert!(lines.iter().any(|l| l.contains("no instrument data")));
        let s = m.stats();
        assert_eq!((s.songs_accepted, s.songs_refused), (1, 2));
        assert_eq!(s.note_ons_scheduled, 0);
        assert!(m.player_stats().is_none());
    }

    #[test]
    fn queue_and_lines_are_bounded() {
        let mut m = PlayerMusic::new(MusicConfig::default());
        for _ in 0..MAX_PENDING_LINES + 10 {
            m.say("x".into());
        }
        assert_eq!(m.take_lines().len(), MAX_PENDING_LINES);
        for _ in 0..MAX_QUEUED + 3 {
            m.command("/playlist");
            assert!(m.queue.len() <= MAX_QUEUED);
        }
        assert!(m.wait_idle(Duration::from_secs(10)));
        // Start-up runs at the first command; while it runs, at most MAX_QUEUED wait.
        assert!(
            m.stats().busy + m.queue.len() as u64 <= 3,
            "{:?}",
            m.stats()
        );
        assert!(m.queue.is_empty());
    }

    // ----- with synthetic instrument data (no archives, offline engine) -----

    const INFO: u32 = 0x2A00_0001;
    const PAYLOAD: u32 = 0x0A00_0001;
    const Q_KEY: u32 = 0x1000_0BCD;
    const N_KEY: u32 = 0x1000_0BCE;

    /// A sound-info record (`instruments.md` §4.2).
    fn sound_info_bytes(did: u32, payload: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(did.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend((-1.0f32).to_le_bytes());
        b.extend((-1.0f32).to_le_bytes());
        b.extend(1.0f32.to_le_bytes());
        b.extend(3.0f32.to_le_bytes());
        b.extend(30.0f32.to_le_bytes());
        b.extend(payload.to_le_bytes());
        b.push(1);
        b.push(b'x');
        b
    }

    /// Both music scripts: one Sound node every note of every instrument reaches.
    fn synthetic_table() -> InstrumentTable {
        use tessera_audio::instruments::ScriptDef;
        use tessera_formats::script_table::{BaseBlock, Node, NodeData, NodeKind, SoundNode};
        let sound = Node {
            kind: NodeKind::SOUND,
            data: NodeData::Sound(Box::new(SoundNode {
                sound: Did(INFO),
                fire_and_forget: true,
                ..SoundNode::default()
            })),
            base: BaseBlock {
                use_parent_intensity: true,
                intensity_multiplier: 1.0,
                probability: 1.0,
                node_id: 1,
                ..BaseBlock::default()
            },
            particle_flag: None,
            ports: Vec::new(),
            targets: Vec::new(),
        };
        let def = |key, roots| ScriptDef {
            key,
            table: None,
            roots,
            max_instances: 8,
        };
        let scripts = MusicScripts {
            quantized: def(Q_KEY, vec![sound.clone()]),
            normal: def(N_KEY, vec![sound]),
            defining_tables: Vec::new(),
        };
        let vocab = MusicVocabulary {
            quantized_script: Q_KEY,
            normal_script: N_KEY,
            instrument_labels: vec![(4, "Lute".into()), (9, "Horn".into())],
            ..MusicVocabulary::default()
        };
        InstrumentTable::build(scripts, &vocab, TableLimits::default(), |d: Did| {
            Ok::<_, String>((d.0 == INFO).then(|| sound_info_bytes(INFO, PAYLOAD)))
        })
        .expect("synthetic table")
    }

    /// Player music with synthetic start-up data: instrument 4 equipped, no archives
    /// (samples are missing, notes are scheduled), the offline engine.
    fn synthetic(folder: &Path) -> PlayerMusic {
        let mut m = PlayerMusic::new(MusicConfig {
            backend: BackendChoice::Offline,
            folders: vec![folder.to_path_buf()],
            ..MusicConfig::default()
        });
        m.data = Some(Arc::new(MusicData {
            set: None,
            table: Ok(Arc::new(synthetic_table())),
            messages: None,
            equipped: Some(4),
            mixer: MixerSettings::default(),
            settings_music: None,
            secs: 0.0,
        }));
        m
    }

    fn note_ons(folder: &Path, name: &str, part: Option<i32>) -> Vec<f64> {
        read_from_folders(&[folder.to_path_buf()], name, PartRequest::from(part))
            .expect("read")
            .note_ons()
            .map(|e| e.time)
            .collect()
    }

    const WAIT: Duration = Duration::from_secs(10);

    #[test]
    fn solo_start_schedules_notes_after_the_offset() {
        let d = folder_with(&[("tune.abc", TUNE)]);
        let mut m = synthetic(d.path());
        assert!(m.command("/play tune"));
        assert!(m.wait_idle(WAIT));
        assert_eq!(
            m.take_lines(),
            vec![fallback_notice(&Notice::NowPlaying {
                title: "Lantern Steps".into()
            })]
        );
        assert!(m.is_performing());
        let log: Vec<_> = m.schedule_log().cloned().collect();
        assert_eq!(log.len(), 1);
        let p = &log[0];
        assert_eq!(
            (p.name.as_str(), p.mode, p.instrument),
            ("tune", StartMode::Solo, 4)
        );
        // The offline clock has not moved: time 0 is the offset after the command.
        assert!((p.start_secs - SOLO_START_OFFSET).abs() < 1e-9, "{p:?}");
        assert_eq!(p.note_on_times, note_ons(d.path(), "tune", None));
        let s = m.stats();
        assert_eq!((s.solo_starts, s.note_ons_scheduled), (1, 6));
        // No archives: the one payload is missing, counted once.
        assert_eq!(s.sample_problems, 1);
        assert!(m.render_offline(0.1).is_some());
    }

    #[test]
    fn stop_given_while_play_is_read_acts_after_it() {
        let d = folder_with(&[("tune.abc", TUNE)]);
        let mut m = synthetic(d.path());
        // Same frame: the stop is queued behind the /play's read.
        assert!(m.command("/play tune"));
        assert!(m.command("/playstop"));
        assert!(m.wait_idle(WAIT));
        let lines = m.take_lines();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[1], "Music: stopped");
        assert!(!m.is_performing());
        // A stop also withdraws a waiting synced part, also one still being read.
        assert!(m.command("/play tune sync"));
        assert!(m.command("/playstop"));
        assert!(m.command("/playstart"));
        assert!(m.wait_idle(WAIT));
        assert!(!m.is_waiting() && !m.is_performing());
        assert_eq!(m.stats().synced_starts, 0);
    }

    #[test]
    fn playstart_given_while_sync_play_is_read_starts_it() {
        let d = folder_with(&[("tune.abc", TUNE)]);
        let mut m = synthetic(d.path());
        assert!(m.command("/play tune 2 sync"));
        assert!(m.command("/playstart"));
        assert!(m.wait_idle(WAIT));
        // The own ready notice, then the start notice (`instruments.md` §8.8.6, §12 P4
        // Decision 1, 3); without a session that starts parts, the start is local.
        assert_eq!(
            m.take_lines(),
            vec![
                sync::render(None, &sync::own_ready_notice("Second Part")),
                fallback_notice(&Notice::SyncedStart)
            ]
        );
        assert_eq!(
            m.take_requests(),
            vec![MusicRequest::Ready {
                tune: "tune".into()
            }]
        );
        let s = m.stats();
        assert_eq!((s.synced_waits, s.synced_starts), (1, 1));
        assert!(!m.is_waiting() && m.is_performing());
        let p = m.schedule_log().last().cloned().expect("scheduled");
        assert_eq!(
            (p.name.as_str(), p.mode, p.part),
            ("tune", StartMode::Synced, Some(2))
        );
        assert!((p.start_secs - SyncGroup::start_time(0.0)).abs() < 1e-9);
        assert_eq!(p.note_on_times, note_ons(d.path(), "tune", Some(2)));
    }

    #[test]
    fn the_later_play_wins() {
        // A long song first: without ordering its slower read would finish last and
        // replace the later command's song.
        let mut big = String::from("X:1\nT:Big\nL:1/16\nQ:240\nK:C\n");
        for _ in 0..1000 {
            big.push_str("C D E F|");
        }
        big.push('\n');
        let d = folder_with(&[("big.abc", &big), ("tune.abc", TUNE)]);
        let mut m = synthetic(d.path());
        assert!(m.command("/play big"));
        assert!(m.command("/play tune"));
        assert!(m.wait_idle(WAIT));
        let names: Vec<String> = m.schedule_log().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["big", "tune"]);
        assert_eq!(m.stats().solo_starts, 2);
    }

    #[test]
    fn sample_budget_keeps_what_the_new_performance_needs() {
        let d = folder_with(&[("tune.abc", TUNE)]);
        let mut m = synthetic(d.path());
        m.sample_budget = 100;
        let pcm = |n: usize| Pcm {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![0.0f32; n].into(),
        };
        let [a, b, c, x, big] = [0x10, 0x11, 0x12, 0x13, 0x14].map(|n| Did(0x0A00_0000 | n));
        assert_eq!(m.keep_samples(&[x], vec![(x, pcm(60))], true), 0);
        // x is not needed any more: dropped to make room.
        assert_eq!(
            m.keep_samples(&[a, b], vec![(a, pcm(40)), (b, pcm(40))], true),
            0
        );
        assert!(!m.bank.contains(x) && m.bank.contains(a) && m.bank.contains(b));
        // a is reused (already in the bank, so not decoded again) and kept; b goes.
        assert_eq!(m.keep_samples(&[a, c], vec![(c, pcm(50))], true), 0);
        assert!(m.bank.contains(a) && m.bank.contains(c) && !m.bank.contains(b));
        assert_eq!(m.bank_samples, 90);
        // While a solo performance runs, a synced part does not evict its samples.
        assert!(m.command("/play tune"));
        assert!(m.wait_idle(WAIT));
        assert!(m.is_performing());
        assert_eq!(m.keep_samples(&[x], vec![(x, pcm(60))], false), 1);
        assert!(m.bank.contains(a) && m.bank.contains(c) && !m.bank.contains(x));
        // More than the whole budget: not kept, counted.
        assert_eq!(m.keep_samples(&[big], vec![(big, pcm(200))], true), 1);
        assert!(!m.bank.contains(big));
        // The finish path reports what did not fit.
        m.take_lines();
        m.sample_budget = 0;
        m.finish_song(SongJob {
            name: "tune".into(),
            sync: false,
            member: None,
            from_feed: false,
            delay_ms: None,
            instrument: Some(4),
            result: Ok(Arc::new(
                read_from_folders(&[d.path().to_path_buf()], "tune", PartRequest::Unspecified)
                    .unwrap(),
            )),
            needed: vec![x],
            samples: vec![(x, pcm(1))],
            sample_problems: 0,
            secs: 0.0,
        });
        assert!(
            m.take_lines().iter().any(|l| l.contains("did not fit")),
            "{:?}",
            m.stats()
        );
    }

    #[test]
    fn instrument_numbers_must_be_known() {
        let d = folder_with(&[]);
        let mut m = synthetic(d.path());
        assert!(m.command("/instrument 999"));
        assert_eq!(
            m.take_lines(),
            vec!["Music: unknown instrument type \"999\""]
        );
        assert_eq!(m.instrument(), Some(4));
        for ok in ["9", "horn", "0", "0x4"] {
            assert!(m.command(&format!("/instrument {ok}")));
            let lines = m.take_lines();
            assert!(lines[0].contains("(debug choice)"), "{ok}: {lines:?}");
        }
        assert_eq!(m.instrument(), Some(4));
        assert!(m.command("/instrument kazoo"));
        assert!(m.take_lines()[0].contains("unknown instrument type"));
    }

    #[test]
    fn a_failed_job_is_reported() {
        let mut m = PlayerMusic::new(MusicConfig::default());
        m.spawn_job(|| panic!("synthetic job failure (expected by this test)"));
        assert!(m.wait_idle(WAIT));
        assert_eq!(m.take_lines(), vec!["Music: the command failed"]);
        assert_eq!(m.stats().failed_jobs, 1);
    }

    #[test]
    fn folders_created_later_are_found() {
        let root = tempfile::tempdir().expect("tempdir");
        let folders = music_folders(None, root.path());
        assert_eq!(folders, vec![root.path().join(TESSERA_MUSIC_FOLDER)]);
        let mut m = PlayerMusic::new(MusicConfig {
            folders,
            ..MusicConfig::default()
        });
        assert!(m.command("/playlist"));
        assert!(m.command("/play tune"));
        assert!(m.wait_idle(WAIT));
        let lines = m.take_lines();
        assert_eq!(lines[0], "Music: no music folder");
        assert!(lines[1].contains("file not found"), "{lines:?}");
        let music = root.path().join(TESSERA_MUSIC_FOLDER);
        std::fs::create_dir(&music).expect("mkdir");
        std::fs::write(music.join("tune.abc"), TUNE).expect("write");
        assert!(m.command("/play tune"));
        assert!(m.wait_idle(WAIT));
        assert_eq!(m.stats().songs_accepted, 1);
    }

    #[test]
    fn start_up_puts_the_settings_folder_first() {
        let mut m = PlayerMusic::new(MusicConfig {
            folders: vec![PathBuf::from("/own/Music")],
            settings_lookup: Some(PathBuf::from("/install")),
            ..MusicConfig::default()
        });
        m.boot_done(MusicData {
            set: None,
            table: Err("no game data".into()),
            messages: None,
            equipped: None,
            mixer: MixerSettings::default(),
            settings_music: Some(PathBuf::from("/settings/Music")),
            secs: 0.0,
        });
        assert_eq!(
            m.config().folders,
            vec![
                PathBuf::from("/settings/Music"),
                PathBuf::from("/own/Music")
            ]
        );
        assert_eq!(m.config().settings_lookup, None);
    }

    #[test]
    fn lines_use_the_command_pipeline() {
        let d = folder_with(&[("tune.abc", TUNE)]);
        let mut m = PlayerMusic::new(MusicConfig {
            folders: vec![d.path().to_path_buf()],
            ..MusicConfig::default()
        });
        assert!(!m.command("hello"), "plain text is not a music command");
        assert!(!m.command("/"), "prefix only");
        assert!(m.command("//PLAYLIST"), "doubled prefix, any letter case");
        assert!(m.wait_idle(WAIT));
        let lines = m.take_lines();
        assert_eq!(
            lines.iter().filter(|l| l.contains("(1 songs)")).count(),
            1,
            "{lines:?}"
        );
        // The chat-window.md §7.3 tokens: an unmatched quote re-emits the previous token.
        assert!(m.command("/play \"open quote"));
        assert_eq!(
            m.take_lines(),
            vec![format!("Music: usage: {}", command::USAGE_PLAY)]
        );
    }
}
