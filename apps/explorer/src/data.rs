// SPDX-License-Identifier: GPL-3.0-or-later
//! The opened install: archive set, property access, strings and icon composer, shared
//! (behind `Arc`) by every asset worker.
//!
//! Opening is done off the UI thread with [`spawn_open`]; nothing here is ever called on
//! the UI thread except cheap accessors.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};

use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_material::MaterialLibrary;
use tessera_material::icon::IconComposer;
use tessera_props::GameData;
use tessera_text::{
    LanguageId, Locale, NamedTables, STRING_TABLE_DIDS, STRING_TABLE_MAP_LABEL, Strings,
    TableSource, select_language,
};

use crate::error::ExplorerError;

/// Reads a surface record by DID for the icon composer: `Ok(None)` when no archive
/// holds it.
pub type SurfaceReader =
    Box<dyn Fn(Did) -> Result<Option<Vec<u8>>, tessera_dat::Error> + Send + Sync>;

/// String-table access over the selected language archive of an [`ArchiveSet`].
#[derive(Debug, Clone)]
pub struct LocalTables {
    set: Arc<ArchiveSet>,
    member: usize,
}

impl LocalTables {
    /// Tables of member `member` of `set` (a `client_local_<Lang>` archive).
    #[must_use]
    pub fn new(set: Arc<ArchiveSet>, member: usize) -> Self {
        Self { set, member }
    }
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

/// Everything the decoders need about the opened install.
pub struct DataContext {
    /// The install directory.
    pub dir: PathBuf,
    /// Every archive of the install, opened read-only.
    pub set: Arc<ArchiveSet>,
    /// Property registry, enum mappers and DID maps (loaded lazily on workers).
    pub game: GameData<Arc<ArchiveSet>>,
    /// Strings of the selected language, if the install has a language archive.
    pub strings: Option<Strings>,
    /// The selected language archive's member index.
    pub local_member: Option<usize>,
    /// Layered icon composition with a decoded-layer cache.
    pub composer: IconComposer<SurfaceReader>,
    /// Surface records, materials, shader records and textures for the mesh viewer
    /// (`material.md`), with caches shared by every worker.
    pub materials: MaterialLibrary<Arc<ArchiveSet>>,
    /// Largest texture side the GPU accepts (set by the UI after start-up).
    pub max_texture_side: AtomicUsize,
}

impl std::fmt::Debug for DataContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataContext")
            .field("dir", &self.dir)
            .field("archives", &self.set.members().len())
            .field("locale", &self.strings.as_ref().and_then(Strings::locale))
            .finish_non_exhaustive()
    }
}

/// Default largest texture side before the UI reports the real one.
pub const DEFAULT_MAX_TEXTURE_SIDE: usize = 8192;

impl DataContext {
    /// Opens the install in `dir` (read-only) and prepares string and property access.
    ///
    /// Reads only directory listings, archive headers and, for named string tables, two
    /// DID maps. `language` selects the string locale (`English` by default,
    /// `string-table.md` §10.1).
    pub fn open(dir: &Path, language: Option<&str>) -> Result<Self, ExplorerError> {
        let set = Arc::new(ArchiveSet::open(dir, SetOptions::default())?);
        for p in set.problems() {
            tracing::info!(problem = %p.message, "archive set");
        }
        Ok(Self::from_set(dir.to_path_buf(), set, language))
    }

    /// Builds the context over an already opened set (tests, tools).
    #[must_use]
    pub fn from_set(dir: PathBuf, set: Arc<ArchiveSet>, language: Option<&str>) -> Self {
        let game = GameData::new(Arc::clone(&set));
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
        let chosen = select_language(&names, language).map(str::to_owned);
        let local_member = chosen
            .as_ref()
            .and_then(|c| locals.iter().find(|(n, _)| n == c).map(|&(_, i)| i));
        let strings = local_member.map(|i| {
            let m = set.member(i);
            let locale = Locale {
                name: chosen.clone().unwrap_or_default(),
                language_id: LanguageId(m.archive.header().subset),
                path: m.path.clone(),
            };
            let named = named_tables(&game).unwrap_or_else(|e| {
                tracing::info!(error = %e, "named string tables unavailable");
                NamedTables::default()
            });
            Strings::new(LocalTables::new(Arc::clone(&set), i))
                .with_locale(locale)
                .with_named_tables(named)
        });
        let materials = MaterialLibrary::new(Arc::clone(&set));
        let reader_set = Arc::clone(&set);
        let reader: SurfaceReader = Box::new(move |did: Did| match reader_set.locate(did.0)? {
            Some(l) => Ok(Some(reader_set.read_located(&l)?)),
            None => Ok(None),
        });
        Self {
            dir,
            set,
            game,
            strings,
            local_member,
            composer: IconComposer::new(reader),
            materials,
            max_texture_side: AtomicUsize::new(DEFAULT_MAX_TEXTURE_SIDE),
        }
    }

    /// The largest texture side to produce.
    #[must_use]
    pub fn max_side(&self) -> usize {
        self.max_texture_side.load(Ordering::Relaxed).max(1)
    }

    /// Display name of member `i` (`surface_aux_1`), or `?`.
    #[must_use]
    pub fn member_name(&self, i: usize) -> String {
        self.set
            .get(i)
            .map_or_else(|| "?".into(), |m| m.id.to_string())
    }
}

/// The named string tables, through the root DID map's string-table label.
fn named_tables(game: &GameData<Arc<ArchiveSet>>) -> Result<NamedTables, ExplorerError> {
    let root = game.root_did_map()?;
    let Some(map) = root.by_label(STRING_TABLE_MAP_LABEL) else {
        return Ok(NamedTables::default());
    };
    Ok(game
        .did_map(map)?
        .map(|m| NamedTables::from_did_map(&m))
        .unwrap_or_default())
}

/// A pending [`DataContext::open`] running on a background thread.
#[derive(Debug)]
pub struct Opening {
    /// The directory being opened.
    pub dir: PathBuf,
    rx: Receiver<Result<DataContext, ExplorerError>>,
}

impl Opening {
    /// The result, once ready. Never blocks.
    pub fn poll(&self) -> Option<Result<DataContext, ExplorerError>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(ExplorerError::Worker(
                "the open thread stopped without a result".into(),
            ))),
        }
    }
}

/// Opens `dir` on a background thread; `notify` runs when the result is ready.
pub fn spawn_open(
    dir: PathBuf,
    language: Option<String>,
    notify: impl FnOnce() + Send + 'static,
) -> Opening {
    let (tx, rx) = mpsc::channel();
    let d = dir.clone();
    let spawned = std::thread::Builder::new()
        .name("explorer-open".into())
        .spawn(move || {
            let r = DataContext::open(&d, language.as_deref());
            let _ = tx.send(r);
            notify();
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start the open thread");
    }
    Opening { dir, rx }
}

/// Runs install discovery (`tessera-install`) on a background thread.
#[derive(Debug)]
pub struct Discovering {
    rx: Receiver<Vec<PathBuf>>,
}

impl Discovering {
    /// Starts discovery; `notify` runs when it finishes.
    pub fn spawn(notify: impl FnOnce() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("explorer-discover".into())
            .spawn(move || {
                let found = match tessera_install::discover() {
                    // In search order: the first is the default install.
                    Ok(d) => d.installs.iter().map(|i| i.root.clone()).collect(),
                    Err(e) => {
                        tracing::info!(error = %e, "install discovery failed");
                        Vec::new()
                    }
                };
                let _ = tx.send(found);
                notify();
            });
        if let Err(e) = spawned {
            tracing::error!(error = %e, "could not start discovery");
        }
        Self { rx }
    }

    /// The installs found, once finished. Never blocks.
    pub fn poll(&self) -> Option<Vec<PathBuf>> {
        match self.rx.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Vec::new()),
        }
    }
}
