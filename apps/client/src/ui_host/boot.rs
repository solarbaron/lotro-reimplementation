// SPDX-License-Identifier: GPL-3.0-or-later
//! Background start-up of the game UI (ARCHITECTURE §20.1 step 9, §21): everything the UI
//! reads from the install is loaded on its own thread while the world loads, so the main
//! thread never waits on archive reads once the UI is attached.
//!
//! It reads the property dictionary, the `EMAPPER` label chains, the `UILAYOUT` map with
//! **every** layout it names (decoded once, `ui-layout.md` §3.2), the InputActions and
//! InputMap chains (`keymap.md` §12.2 table), the locale default font (`string-table.md`
//! §10.3), the string tables, the chat window's setup (`chat-window.md` §10 items 1–3), the
//! skin tables and the skins in the skins root (`skin-definition.md` §9.4: once, at UI
//! start), and prepares the minimap data reader.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record};
use tessera_ui::game::{GameLayouts, GameVocab};
use tessera_ui::skin::{SkinEnvironment, SkinSet, SkinTables};
use tessera_ui::{Ui, Vocab};
use tessera_world::ui_maps::{UiMapPids, UiMapReader};

use super::env::{UiData, decode_layout, locale_font_from_record, locale_leading_from_record};
use crate::panels::catalog::game_layouts;
use crate::panels::chat::ChatSetup;
use crate::panels::{PanelCatalog, PanelTypes};

/// Most string tables the start-up thread preloads (the string cache keeps up to 16,384).
pub const MAX_PRELOAD_TABLES: usize = 4096;

/// Loads every string table that `layouts` reference into the cache of `strings`, so
/// that resolving their labels on the main thread never reads the archive (ARCHITECTURE
/// §21). At most [`MAX_PRELOAD_TABLES`] tables. Returns (tables referenced, tables loaded).
pub fn preload_string_tables<'a>(
    strings: &tessera_text::Strings,
    layouts: impl IntoIterator<Item = &'a tessera_formats::UiLayout>,
) -> (usize, usize) {
    use tessera_formats::string_info::StringSource;
    use tessera_formats::ui_layout::LayoutRef;
    let t = Instant::now();
    let mut tables = std::collections::BTreeSet::new();
    for l in layouts {
        l.for_each_reference(|r| {
            if let LayoutRef::String {
                source: StringSource::Table { table, .. },
                ..
            } = r
                && table.0 != 0
                && tables.len() < MAX_PRELOAD_TABLES
            {
                tables.insert(table.0);
            }
        });
    }
    let loaded = tables
        .iter()
        .filter(|did| matches!(strings.table(**did), Ok(Some(_))))
        .count();
    tracing::info!(
        referenced = tables.len(),
        loaded,
        elapsed_ms = t.elapsed().as_millis(),
        "UI string tables preloaded"
    );
    (tables.len(), loaded)
}

/// Most entries of the minimap reader's caches.
const MAP_CACHE_ENTRIES: usize = 512;
/// Longest base chain followed in an enum mapper (cycle guard).
const MAX_CHAIN: usize = 64;

/// Skins found at UI start.
#[derive(Debug, Clone)]
pub struct SkinData {
    /// The discovered skins.
    pub skins: Arc<SkinSet>,
    /// The art tables from the install.
    pub tables: Arc<SkinTables>,
    /// The activation environment (Wine prefix for drive-letter paths, D12).
    pub env: SkinEnvironment,
}

/// What the UI start-up produces.
pub struct UiBooted {
    /// The install side of the environment.
    pub data: Arc<UiData>,
    /// The UI vocabulary resolved by label.
    pub vocab: Vocab,
    /// The game widgets' vocabulary.
    pub game_vocab: GameVocab,
    /// The fifteen panels.
    pub catalog: PanelCatalog,
    /// Layouts game code creates elements from.
    pub game_layouts: GameLayouts,
    /// Element types of the panels' own rules.
    pub panel_types: PanelTypes,
    /// The chat window's setup.
    pub chat: ChatSetup,
    /// The minimap and world-map reader.
    pub maps: Option<UiMapReader>,
    /// Skins, when the tables load.
    pub skins: Option<SkinData>,
    /// InputActions command names → IDs.
    pub command_ids: Vec<(String, u32)>,
    /// The P1 widgets' vocabulary (item slots, tooltips, paperdoll).
    pub char_vocab: tessera_ui::game::CharVocab,
    /// The P1 panels' layouts and element IDs (`character-panels.md` §2, §4.1).
    pub char_ids: crate::character::panel::PanelIds,
    /// The character screens' layouts and element IDs (`character-screens.md` §2).
    pub chargen: crate::screens::ui::ChargenRefs,
    /// The social panels' vocabulary and layout keys (`social-panels.md` §2).
    pub social_vocab: tessera_ui::game::social::SocialVocab,
    /// The combat widgets' layouts and vocabulary (decision 0024 P2, `combat-ui.md` §2).
    pub combat: crate::combat::boot::CombatRefs,
    /// Layouts decoded.
    pub layouts: usize,
    /// Time taken.
    pub elapsed_ms: u128,
}

impl std::fmt::Debug for UiBooted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiBooted")
            .field("data", &self.data)
            .field("catalog", &self.catalog.len())
            .field("layouts", &self.layouts)
            .field("skins", &self.skins.as_ref().map(|s| s.skins.skins().len()))
            .finish_non_exhaustive()
    }
}

/// A running or finished UI start-up.
#[derive(Debug)]
pub struct UiBoot {
    handle: Option<JoinHandle<Result<UiBooted, String>>>,
}

impl UiBoot {
    /// Starts loading from `set` (install `game_dir`); `skins_root` overrides the default
    /// skins root (D1).
    ///
    /// # Errors
    ///
    /// When the thread cannot be spawned.
    pub fn spawn(
        set: Arc<ArchiveSet>,
        game_dir: PathBuf,
        skins_root: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        let handle = std::thread::Builder::new()
            .name("tessera-ui-boot".into())
            .spawn(move || load(set, &game_dir, skins_root.as_deref()))?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    /// The result once ready (never blocks).
    pub fn try_take(&mut self) -> Option<Result<UiBooted, String>> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            return Some(match self.handle.take()?.join() {
                Ok(r) => r,
                Err(_) => Err("UI start-up panicked".into()),
            });
        }
        None
    }

    /// Waits for the result (tests and headless runs).
    pub fn wait(mut self) -> Result<UiBooted, String> {
        match self.handle.take() {
            Some(h) => h
                .join()
                .unwrap_or_else(|_| Err("UI start-up panicked".into())),
            None => Err("UI start-up already taken".into()),
        }
    }
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

fn enum_mapper(set: &ArchiveSet, did: Did) -> Option<EnumMapper> {
    let payload = set.read(did.0).ok()?;
    EnumMapper::parse_expecting(&payload, did).ok()
}

/// Every (name, key) of an enum mapper and its base chain, nearest first; the first
/// occurrence of a key wins.
fn chain_names(set: &ArchiveSet, first: Option<Did>) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut seen_keys = std::collections::BTreeSet::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut did = first;
    for _ in 0..MAX_CHAIN {
        let Some(d) = did.filter(|d| d.0 != 0 && seen.insert(d.0)) else {
            break;
        };
        let Some(m) = enum_mapper(set, d) else { break };
        for (key, name) in m.names() {
            if seen_keys.insert(*key) {
                out.push((name.clone(), *key));
            }
        }
        did = Some(m.base);
    }
    out
}

/// The UI vocabulary and the game vocabulary resolved through the dictionary and labels.
#[must_use]
pub fn vocabs(dict: &PropertyDictionary, labels: &UiLabels) -> (Vocab, GameVocab) {
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| dict.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let mut gv = GameVocab::default();
    gv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| dict.pid(n),
    );
    (vocab, gv)
}

fn load(
    set: Arc<ArchiveSet>,
    game_dir: &Path,
    skins_root: Option<&Path>,
) -> Result<UiBooted, String> {
    let t = Instant::now();
    let dict_did = tessera_props::PROPERTY_DICTIONARY_DID;
    let payload = set
        .read(dict_did.0)
        .map_err(|e| format!("property dictionary: {e}"))?;
    let dict = Arc::new(
        PropertyDictionary::parse_expecting(&payload, dict_did.0)
            .map_err(|e| format!("property dictionary: {e}"))?,
    );
    let root = did_map(&set, tessera_props::ROOT_DID_MAP).ok_or("root DID map missing")?;
    let emapper = root
        .by_label("EMAPPER")
        .and_then(|d| did_map(&set, d))
        .ok_or("EMAPPER map missing")?;
    let labels = UiLabels::load(|l| emapper.by_label(l), |d| enum_mapper(&set, d));
    let uilayout = root
        .by_label("UILAYOUT")
        .and_then(|d| did_map(&set, d))
        .ok_or("UILAYOUT map missing")?;

    // Every layout of the map, decoded once (`ui-layout.md` §3.2).
    let mut layouts = HashMap::new();
    let mut layout_keys = HashMap::new();
    for e in uilayout.iter() {
        if e.value.0 >> 24 != 0x22 {
            continue;
        }
        layout_keys.insert(e.key, e.value.0);
        layouts
            .entry(e.value.0)
            .or_insert_with(|| decode_layout(&set, &dict, e.value.0));
    }
    let decoded = layouts.values().filter(|l| l.is_some()).count();

    let command_ids = chain_names(&set, emapper.by_label("InputActions"));
    let input_maps: HashMap<u32, String> = chain_names(&set, emapper.by_label("InputMap"))
        .into_iter()
        .map(|(n, k)| (k, n))
        .collect();

    let strings_map = root.by_label("STRINGTABLE").and_then(|d| did_map(&set, d));
    let locale_record = strings_map
        .as_ref()
        .and_then(|m| m.by_label("Language"))
        .and_then(|d| set.read(d.0).ok());
    let locale_font = locale_record.as_deref().and_then(locale_font_from_record);
    let locale_leading = locale_record
        .as_deref()
        .map_or(0.0, locale_leading_from_record);
    let strings = match tessera_text::Strings::open_install(game_dir, None) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("UI strings unavailable: {e}");
            None
        }
    };
    if let Some(st) = &strings {
        preload_string_tables(st, layouts.values().flatten().map(|l| &**l));
    }

    let (vocab, game_vocab) = vocabs(&dict, &labels);
    let mut char_vocab = tessera_ui::game::CharVocab::default();
    char_vocab.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| dict.pid(n),
    );
    let chargen = crate::screens::ui::ChargenRefs::resolve(
        &mut |l| {
            uilayout
                .by_label(l)
                .filter(|d| d.0 >> 24 == 0x22)
                .map(|d| d.0)
        },
        &mut |l| labels.key(UiEnum::ElementId, l),
    );
    let char_ids = crate::character::panel::PanelIds::resolve(
        &|l| uilayout.by_label(l).filter(|d| d.0 >> 24 == 0x22),
        &|l| labels.key(UiEnum::ElementId, l),
        &|n| dict.pid(n),
        &|t| labels.key(UiEnum::ElementType, t),
    );
    let social_vocab = social_vocab(&dict, &labels, &uilayout);
    let combat = crate::combat::boot::CombatRefs::resolve(
        &mut |l| uilayout.by_label(l).filter(|d| d.0 >> 24 == 0x22),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| dict.pid(n),
    );
    let catalog = PanelCatalog::resolve(&uilayout, &mut |l| labels.key(UiEnum::ElementId, l));
    let mut panel_types = PanelTypes::default();
    panel_types.resolve_with(&mut |n| labels.key(UiEnum::ElementType, n));

    let mut chat = ChatSetup::sandbox();
    if let Some(unique) = root.by_label("UNIQUEDB").and_then(|d| did_map(&set, d)) {
        let schema: &(dyn tessera_formats::PropertySchema + Send + Sync) = &*dict;
        if let Err(e) = chat.load_defaults(&*set, schema, &unique, &game_vocab.pids) {
            tracing::info!("chat defaults not read ({e}); sandbox chat setup");
        }
        // The instant-message tab's options (revision 4, `chat-window.md` §4.6, CT6).
        if let Err(e) = chat.load_im_options(&*set, schema, &unique, &mut |n| dict.pid(n)) {
            tracing::info!("instant-message options not read ({e}); no instant-message tab");
        }
    }
    if let Some(m) = &strings_map {
        let n = chat.load_texts(&*set, m);
        tracing::debug!(texts = n, "chat texts read");
    }

    let maps = Some(UiMapReader::new(
        Arc::clone(&set) as Arc<dyn tessera_world::WorldSource>,
        Arc::clone(&dict) as Arc<dyn tessera_formats::PropertySchema + Send + Sync>,
        UiMapPids::from_names(|n| dict.pid(n)),
        MAP_CACHE_ENTRIES,
    ));

    let skins = load_skins(&set, game_dir, skins_root);

    let data = Arc::new(UiData {
        set: Arc::clone(&set),
        dict,
        layouts: Mutex::new(layouts),
        layout_keys,
        input_maps,
        strings,
        locale_font,
        locale_leading,
        game_dir: game_dir.to_path_buf(),
    });
    let elapsed_ms = t.elapsed().as_millis();
    tracing::info!(
        layouts = decoded,
        panels = catalog.len(),
        commands = command_ids.len(),
        locale_font = ?locale_font.map(|d| format!("{d:#010x}")),
        skins = skins.as_ref().map_or(0, |s| s.skins.skins().len()),
        elapsed_ms,
        "game UI data ready"
    );
    Ok(UiBooted {
        data,
        vocab,
        game_vocab,
        catalog,
        game_layouts: game_layouts(&uilayout),
        panel_types,
        chat,
        maps,
        skins,
        command_ids,
        char_vocab,
        char_ids,
        chargen,
        social_vocab,
        combat,
        layouts: decoded,
        elapsed_ms,
    })
}

fn load_skins(
    set: &Arc<ArchiveSet>,
    game_dir: &Path,
    skins_root: Option<&Path>,
) -> Option<SkinData> {
    let gd = tessera_props::GameData::new(Arc::clone(set));
    let tables = match SkinTables::load(&gd) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            tracing::warn!("skin tables unavailable: {e}");
            return None;
        }
    };
    let install = tessera_install::GameInstall::open(game_dir).ok();
    let root = match skins_root {
        Some(r) => r.to_path_buf(),
        None => {
            let install = install.as_ref()?;
            let dir = install.settings_dir(
                &tessera_install::SettingsQuery::default(),
                &tessera_install::SearchEnv::from_process(),
            );
            tessera_ui::skin::default_skins_root(&dir)
        }
    };
    let skins = Arc::new(SkinSet::discover(&root, tables.as_ref()));
    for r in skins.rejected() {
        tracing::warn!(?r, "skin definition rejected");
    }
    tracing::info!(
        root = %root.display(),
        skins = skins.skins().len(),
        rejected = skins.rejected().len(),
        "skins discovered"
    );
    Some(SkinData {
        skins,
        tables,
        env: SkinEnvironment {
            wine_prefix: install.and_then(|i| i.prefix),
        },
    })
}

/// The social panels' vocabulary (`social-panels.md` §2): element types, IDs, states and
/// properties by label, and the `UILAYOUT` keys of their layouts by label (a label the map
/// lacks keeps this install's key).
#[must_use]
pub fn social_vocab(
    dict: &PropertyDictionary,
    labels: &UiLabels,
    uilayout: &DataIdMap,
) -> tessera_ui::game::social::SocialVocab {
    let mut v = tessera_ui::game::social::SocialVocab::default();
    v.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| dict.pid(n),
    );
    let key = |label: &str, default: u32| {
        uilayout
            .iter()
            .find(|e| &*e.label == label)
            .map_or(default, |e| e.key)
    };
    let l = &mut v.layouts;
    l.social = key("social", l.social);
    l.friends = key("friends", l.friends);
    l.ignore = key("ignore", l.ignore);
    l.minifellow = key("minifellow", l.minifellow);
    l.fellowinvite = key("fellowinvite", l.fellowinvite);
    l.securetrade = key("securetrade", l.securetrade);
    l.itemdisplay = key("itemdisplay", l.itemdisplay);
    v
}

/// Registers the game widget classes on a new UI.
pub fn register(ui: &mut Ui, game_vocab: &GameVocab) {
    game_vocab.register_classes(ui);
}
