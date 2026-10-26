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

use std::collections::{BTreeMap, HashMap};
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
    /// Toggle types of the default action map's table A, by command ID (`keymap.md`
    /// §12.10, `lua-plugin-api.md` §13.12.1): the UI's Impulse test (decision 0014 D9).
    /// Empty when the record is missing or does not decode.
    pub toggle_types: BTreeMap<u32, u8>,
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

/// Key and label of the default action map in the `ACTIONMAP` DID map (`keymap.md` §12.2).
const ACTIONMAP_DEFAULT: (u32, &str) = (1, "default");

/// Most table-A rows taken from the action map (Tessera bound; the record's own decoder
/// already checks its counts against the payload).
const MAX_TOGGLE_ROWS: usize = 65_536;

/// The toggle types of table A of the default action map (`keymap.md` §12.10), reached
/// through the root DID map's `ACTIONMAP` entry and its key 1 labelled `default` (§12.2).
/// A type that does not fit a byte is left out (types are 0–3). Empty, with a log line, when
/// any step is missing or the record does not decode: the UI then treats every command as
/// non-Impulse (`lua-plugin-api.md` §13.12.1).
fn action_map_toggle_types(set: &ArchiveSet, root: &DataIdMap) -> BTreeMap<u32, u8> {
    let Some(map) = root.by_label("ACTIONMAP").and_then(|d| did_map(set, d)) else {
        tracing::info!("no ACTIONMAP DID map: no toggle types");
        return BTreeMap::new();
    };
    let did = match map.get(ACTIONMAP_DEFAULT.0) {
        Some(e) if &*e.label == ACTIONMAP_DEFAULT.1 && e.value.0 != 0 => e.value.0,
        _ => {
            tracing::info!("no default action map: no toggle types");
            return BTreeMap::new();
        }
    };
    let decoded = set
        .read(did)
        .map_err(|e| e.to_string())
        .and_then(|b| tessera_input::import::decode_action_map(&b).map_err(|e| e.to_string()));
    match decoded {
        Ok(m) => m
            .toggle_types
            .iter()
            .take(MAX_TOGGLE_ROWS)
            .filter_map(|(c, t)| u8::try_from(*t).ok().map(|t| (*c, t)))
            .collect(),
        Err(e) => {
            tracing::info!("action map {did:#010x}: {e}; no toggle types");
            BTreeMap::new()
        }
    }
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

/// The dictionary default of `pid` (`property-system.md` §2.2, `ui-widgets.md` §2.2 step 2):
/// the default value its definition stores, `None` when the PID has no definition or the
/// definition stores none.
#[must_use]
pub fn dictionary_default(
    dict: &PropertyDictionary,
    pid: u32,
) -> Option<tessera_formats::PropertyValue> {
    dict.definition(pid).and_then(|d| d.default.clone())
}

/// The PIDs whose default value another spec states as a rule of the property, kept even
/// when the install's dictionary defines the PID without a stored default (`ui-widgets.md`
/// §2.2 implementer OPEN of 2026-10-26): `UICore_Element_alphablendmod` (default 1,
/// `ui-layout.md` §9.6; a plugin background keeps 1.0, `lua-plugin-api.md` §13.17) and
/// `UICore_Element_AllowStretchOnDrag` (default true, `ui-behaviour.md` §7.2). Without this
/// an install that stores no default for the image opacity would draw every base image
/// fully transparent.
fn stated_defaults(vocab: &Vocab) -> [u32; 2] {
    [vocab.pids.alphablendmod, vocab.pids.allow_stretch_on_drag]
}

/// The UI vocabulary and the game vocabulary resolved through the dictionary and labels.
/// The UI vocabulary's dictionary defaults follow `ui-widgets.md` §2.2 with the install's
/// dictionary read at run time (UB49): a PID whose definition stores a default takes the
/// install's; a PID the install defines **without** a default has none (§2.2 step 3: zero,
/// false or none for its type), even where the spec's table of observed defaults lists one;
/// only a PID the dictionary does not define keeps the spec table's value (the table stands
/// in for a dictionary that lacks the definition), and the defaults other specs state as a
/// rule of the property ([`stated_defaults`]) are always kept. Runs on the start-up thread
/// ([`UiBoot`]), never on the frame thread.
#[must_use]
pub fn vocabs(dict: &PropertyDictionary, labels: &UiLabels) -> (Vocab, GameVocab) {
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| dict.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |p| dictionary_default(dict, p),
    );
    // §2.2 steps 2 and 3: a definition without a stored default gives no default, except
    // for the PIDs whose default other specs state as a rule of the property rather than as
    // an observed dictionary default ([`stated_defaults`]).
    let stated = stated_defaults(&vocab);
    vocab.defaults.retain(|&p, _| {
        stated.contains(&p) || dict.definition(p).is_none_or(|d| d.default.is_some())
    });
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
    let toggle_types = action_map_toggle_types(&set, &root);
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
        toggle_types = toggle_types.len(),
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
        toggle_types,
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

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::PropertyValue;

    /// A synthetic property dictionary (`property-system.md` §2.1, §2.2): `(pid, name, type
    /// code, default bytes)`; a definition stores a default when its bytes are given.
    /// Counts and name lengths stay below 128, so every VLE is one byte.
    fn dictionary(defs: &[(u32, &str, u32, Option<&[u8]>)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x3400_0000u32.to_le_bytes());
        out.extend_from_slice(&[0; 8]);
        out.push(0);
        out.push(u8::try_from(defs.len()).expect("few"));
        for (pid, name, _, _) in defs {
            out.extend_from_slice(&pid.to_le_bytes());
            out.push(u8::try_from(name.len()).expect("short"));
            out.extend_from_slice(name.as_bytes());
        }
        out.extend_from_slice(&[0, 0]);
        out.push(0);
        out.push(u8::try_from(defs.len()).expect("few"));
        for (pid, _, ty, default) in defs {
            for v in [*pid, *pid, *ty, 0, 4, 0, 0] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            let flags = if default.is_some() {
                tessera_formats::property::FLAG_HAS_DEFAULT
            } else {
                0
            };
            out.extend_from_slice(&flags.to_le_bytes());
            if let Some(d) = default {
                out.extend_from_slice(d);
            }
            out.extend_from_slice(&0x3FC0_0000u32.to_le_bytes());
            out.extend_from_slice(&[2, 2, 4, 0, 1]);
            out.push(0); // children (VLE)
            out.extend_from_slice(&0u32.to_le_bytes()); // subset count
            out.extend_from_slice(&0u32.to_le_bytes()); // terminator
        }
        out
    }

    /// `ui-widgets.md` §2.2 steps 2 and 3: the UI vocabulary takes the install's dictionary
    /// defaults, which win over the spec table; a PID whose definition stores no default has
    /// none (step 3: false here), even where the spec table lists one; a PID outside the
    /// spec table gets the install's default; a table PID the dictionary does not define
    /// keeps the table's value.
    #[test]
    fn vocab_defaults_come_from_the_dictionary() {
        let widget = 2.5f32.to_le_bytes();
        let mode = 2u32.to_le_bytes();
        let bytes = dictionary(&[
            (0x06DF, "UICore_Scrollbar_widget_size", 9, Some(&widget)),
            (0x0170, "UICore_Scrollable_keep_in_bounds", 21, None),
            (0x0203, "UICore_Element_ObjectMode", 8, Some(&mode)),
            (0x0C00, "UICore_Element_AllowStretchOnDrag", 21, Some(&[0])),
            (0x06E5, "UICore_Element_alphablendmod", 9, None),
        ]);
        let dict = PropertyDictionary::parse(&bytes).expect("synthetic dictionary");
        assert_eq!(
            dictionary_default(&dict, 0x06DF),
            Some(PropertyValue::Float(2.5))
        );
        assert_eq!(dictionary_default(&dict, 0x0170), None);
        assert_eq!(dictionary_default(&dict, 0x7777), None);

        let (vocab, _) = vocabs(&dict, &UiLabels::default());
        let p = vocab.pids;
        assert_eq!(p.allow_stretch_on_drag, 0x0C00, "resolved by name");
        let d = &vocab.defaults;
        assert_eq!(
            d.get(&0x06DF),
            Some(&PropertyValue::Float(2.5)),
            "install wins"
        );
        assert_eq!(
            d.get(&p.scrollable_keep_in_bounds),
            None,
            "defined without a default: none (step 3), not the table's true"
        );
        assert_eq!(
            d.get(&0x0203),
            Some(&PropertyValue::Enum(2)),
            "outside the table"
        );
        assert_eq!(d.get(&0x0C00), Some(&PropertyValue::Bool(false)));
        // A default another spec states as the property's rule is kept without a stored one.
        assert_eq!(p.alphablendmod, 0x06E5);
        assert_eq!(
            d.get(&0x06E5),
            Some(&PropertyValue::Float(1.0)),
            "ui-layout.md §9.6"
        );
        // A spec-table PID the dictionary does not define keeps the table's value.
        assert_eq!(d.get(&p.object_opacity), Some(&PropertyValue::Float(1.0)));
    }

    /// A dictionary that defines none of the table's PIDs (and stores no default) leaves
    /// exactly the spec table's defaults; defining a table PID without a default drops only
    /// that entry (§2.2 step 3).
    #[test]
    fn vocab_defaults_without_dictionary_defaults_are_the_spec_table() {
        let bytes = dictionary(&[(0x0C01, "Tessera_Test_unrelated", 21, None)]);
        let dict = PropertyDictionary::parse(&bytes).expect("synthetic dictionary");
        let (vocab, _) = vocabs(&dict, &UiLabels::default());
        assert_eq!(vocab.defaults, Vocab::default().defaults);

        let bytes = dictionary(&[(0x06DF, "UICore_Scrollbar_widget_size", 9, None)]);
        let dict = PropertyDictionary::parse(&bytes).expect("synthetic dictionary");
        let (vocab, _) = vocabs(&dict, &UiLabels::default());
        let mut want = Vocab::default().defaults;
        assert!(want.remove(&0x06DF).is_some(), "a table PID");
        assert_eq!(vocab.defaults, want);
    }

    /// Real data (`TESSERA_GAME_DIR`, read-only): with §2.2 step 3 applied, no default of
    /// Tessera's table is dropped: the install stores a default for every table PID it
    /// defines, apart from the stated defaults of [`stated_defaults`], which are kept;
    /// `ObjectOpacity` keeps 1.0 (`ui-behaviour.md` UB49) and the image opacity 1
    /// (`ui-layout.md` §9.6), so no element or image turns transparent by default. Only
    /// counts are printed.
    #[test]
    fn vocab_defaults_real_data() {
        let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        };
        let set = ArchiveSet::open(Path::new(&dir), tessera_dat::SetOptions::default())
            .expect("open the archive set");
        let did = tessera_props::PROPERTY_DICTIONARY_DID;
        let payload = set.read(did.0).expect("property dictionary");
        let dict = PropertyDictionary::parse_expecting(&payload, did.0).expect("dictionary");
        let (vocab, _) = vocabs(&dict, &UiLabels::default());
        let mut table = Vocab::default();
        table.resolve_with(
            &mut |n| dict.pid(n),
            &mut |_| None,
            &mut |_| None,
            &mut |_| None,
            &mut |_| None,
            &mut |_| None,
        );
        let defined: Vec<u32> = table
            .defaults
            .keys()
            .copied()
            .filter(|&p| dict.definition(p).is_some())
            .collect();
        let dropped: Vec<u32> = defined
            .iter()
            .copied()
            .filter(|p| !vocab.defaults.contains_key(p))
            .collect();
        let stated = stated_defaults(&vocab);
        let unstored = defined
            .iter()
            .filter(|&&p| dict.definition(p).is_some_and(|d| d.default.is_none()))
            .count();
        eprintln!(
            "table defaults {}, defined by the install {}, without a stored default {}, dropped {}",
            table.defaults.len(),
            defined.len(),
            unstored,
            dropped.len()
        );
        assert!(
            dropped.is_empty(),
            "table PIDs without a stored default: {dropped:#x?}"
        );
        assert_eq!(
            vocab.defaults.get(&vocab.pids.object_opacity),
            Some(&tessera_formats::PropertyValue::Float(1.0))
        );
        for p in stated {
            assert!(
                vocab.defaults.contains_key(&p),
                "stated default {p:#x} kept"
            );
        }
        assert_eq!(
            vocab.defaults.get(&vocab.pids.alphablendmod),
            Some(&tessera_formats::PropertyValue::Float(1.0)),
            "image opacity 1 (ui-layout.md §9.6)"
        );
    }
}
