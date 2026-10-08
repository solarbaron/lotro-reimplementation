// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the client runs besides presentation: the data loader, the world session and
//! the plugin host. Shared by the windowed client and `--smoke`.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_dat::ArchiveSet;
use tessera_install::SearchEnv;

use crate::cli::Args;
use crate::data::{ArchiveStats, DataLoader, DecodedTexture, LoaderConfig, LoaderEvent};
use crate::net::World;
use crate::plugins::Plugins;
use crate::scene::Scene;
use crate::walker::source::{SourceShared, WalkerSource};
use tessera_render::Frame;

/// Where the data side currently is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataStatus {
    /// Discovery / opening in progress.
    Loading,
    /// No install: original test scene only.
    NoData(String),
    /// Archives open.
    Ready {
        /// The install directory.
        root: PathBuf,
        /// Totals.
        stats: ArchiveStats,
    },
}

/// Non-presentation client state.
#[derive(Debug)]
pub struct Runtime {
    loader: DataLoader,
    /// Data-side status.
    pub status: DataStatus,
    /// The open archive set, once ready.
    pub archives: Option<Arc<ArchiveSet>>,
    /// World session.
    pub world: Option<World>,
    /// Plugin host.
    pub plugins: Plugins,
    /// State shared between the world walker and the sandbox (walker sessions only).
    pub walker_shared: Option<Arc<Mutex<SourceShared>>>,
    /// The walker's sandbox settings, until [`Runtime::start_walker_session`] starts the
    /// session (once the character catalog can give the avatar and crowd their bodies).
    walker_session: Option<tessera_net::sandbox::SandboxConfig>,
    /// `--server`: play on a Tessera test server instead of the sandbox (M7).
    remote: Option<crate::net::RemoteSettings>,
    /// System chat lines (sender empty) since the last [`Runtime::take_notices`].
    notices: Vec<String>,
    character: Option<String>,
    texture_settled: bool,
}

/// Tessera's own data folder: `--data-dir`, else `$XDG_DATA_HOME/tessera`, else
/// `./tessera-data`.
#[must_use]
pub fn data_root(args: &Args) -> PathBuf {
    if let Some(d) = &args.data_dir {
        return d.clone();
    }
    SearchEnv::from_process()
        .data_home()
        .map_or_else(|| PathBuf::from("tessera-data"), |d| d.join("tessera"))
}

/// `now + wait`, or a far-off instant when that overflows (never panics).
#[must_use]
pub fn deadline_after(now: Instant, wait: Duration) -> Instant {
    now.checked_add(wait)
        .or_else(|| now.checked_add(Duration::from_secs(100 * 365 * 86_400)))
        .unwrap_or(now)
}

/// `path` made absolute, with its deepest existing ancestor resolved (symbolic links,
/// `..`) and the not-yet-existing rest appended.
fn resolve_path(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |d| d.join(path))
    };
    let mut rest: Vec<Component<'_>> = Vec::new();
    let mut cur: &Path = &abs;
    loop {
        if let Ok(c) = std::fs::canonicalize(cur) {
            let mut out = c;
            for comp in rest.iter().rev() {
                match comp {
                    Component::ParentDir => {
                        out.pop();
                    }
                    Component::CurDir => {}
                    other => out.push(other.as_os_str()),
                }
            }
            return out;
        }
        let Some(parent) = cur.parent() else {
            return abs;
        };
        if let Some(c) = cur.components().next_back() {
            rest.push(c);
        }
        cur = parent;
    }
}

/// Whether `path` (which may not exist yet) lies inside `dir`.
#[must_use]
pub fn is_inside(path: &Path, dir: &Path) -> bool {
    resolve_path(path).starts_with(resolve_path(dir))
}

/// Install directories the client must never write into: `--game-dir`,
/// `TESSERA_GAME_DIR` and, once known, the opened install.
#[must_use]
pub fn install_roots(args: &Args, status: Option<&DataStatus>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = args.game_dir.iter().cloned().collect();
    roots.extend(
        std::env::var_os(tessera_install::GAME_DIR_ENV)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
    );
    if let Some(DataStatus::Ready { root, .. }) = status {
        roots.push(root.clone());
    }
    roots
}

/// Refuses an output path inside the game install (ARCH §1: the install is read-only).
///
/// # Errors
///
/// When `path` lies inside one of [`install_roots`].
pub fn refuse_install_path(
    args: &Args,
    status: Option<&DataStatus>,
    path: &Path,
) -> anyhow::Result<()> {
    if install_roots(args, status)
        .iter()
        .any(|root| is_inside(path, root))
    {
        anyhow::bail!(
            "refusing to write {} inside the game install",
            path.display()
        );
    }
    Ok(())
}

impl Runtime {
    /// Start the loader thread, the world session and the plugin host.
    pub fn start(args: &Args, now: Instant) -> anyhow::Result<Self> {
        let remote = crate::cli::remote_settings_from_env(args)?;
        // The walker shows no showcase texture and needs the disk for streaming: no
        // texture search and no directory walk then.
        let walker = args.walker();
        let loader = DataLoader::spawn(LoaderConfig {
            game_dir: args.game_dir.clone(),
            no_data: args.no_data,
            surface: args.surface,
            count_entries: !args.no_entry_count && !walker,
            showcase: !walker,
        })?;
        let walker_shared = args.walker().then(|| {
            Arc::new(Mutex::new(SourceShared {
                default_region: args.region,
                ..SourceShared::default()
            }))
        });
        // The walker's session starts once the world data is loaded
        // (`start_walker_session`); everything else gets a plain sandbox now.
        let mut walker_session = None;
        let session = match &walker_shared {
            Some(_) => match crate::walker_host::sandbox_config(args) {
                Ok(c) => {
                    walker_session = Some(c);
                    Ok(None)
                }
                Err(e) => Err(e),
            },
            None => World::plain(now, remote.as_ref())
                .map(Some)
                .map_err(anyhow::Error::from),
        };
        let world = match session {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("world session unavailable: {e}");
                None
            }
        };
        let root = data_root(args);
        let plugins_dir = args.plugins.clone().unwrap_or_else(|| root.join("Plugins"));
        if !plugins_dir.is_dir() {
            if args.plugins.is_some() {
                tracing::warn!(dir = %plugins_dir.display(), "plugins folder does not exist");
            } else {
                tracing::info!(dir = %plugins_dir.display(), "no plugins folder (use --plugins)");
            }
        }
        let mut plugins = Plugins::start(&plugins_dir, root.join("plugindata"), &args.load_plugins);
        plugins.set_music(crate::audio::player_music::PlayerMusic::new(
            crate::audio::player_music::MusicConfig::from_args(args, &root),
        ));
        plugins.set_startup_commands(args.commands.clone());
        Ok(Self {
            loader,
            status: DataStatus::Loading,
            archives: None,
            world,
            plugins,
            walker_shared,
            walker_session,
            remote,
            notices: Vec::new(),
            character: None,
            texture_settled: false,
        })
    }

    /// Starts the walker's sandbox session with the avatar's appearance and the crowd's
    /// appearances (no-op without a pending walker session). Entity events are not
    /// captured yet: the walker switches that on once it exists
    /// ([`World::capture_entity_events`]), so nothing queues events nobody drains. At most
    /// [`tessera_net::sandbox::MAX_APPEARANCES`] crowd appearances are used; a crowd the
    /// sandbox still refuses is dropped rather than the whole session.
    pub fn start_walker_session(
        &mut self,
        now: Instant,
        avatar: Option<tessera_net::Appearance>,
        mut crowd: Vec<tessera_net::Appearance>,
    ) {
        let (Some(mut config), Some(shared)) =
            (self.walker_session.take(), self.walker_shared.clone())
        else {
            return;
        };
        config.avatar = avatar;
        if config.crowd.count > 0 {
            if crowd.is_empty() {
                tracing::warn!("--crowd: no playable bodies in the data; no crowd");
                config.crowd.count = 0;
            } else {
                if crowd.len() > tessera_net::sandbox::MAX_APPEARANCES {
                    tracing::warn!(
                        bodies = crowd.len(),
                        "--crowd: using the first {} playable bodies",
                        tessera_net::sandbox::MAX_APPEARANCES
                    );
                    crowd.truncate(tessera_net::sandbox::MAX_APPEARANCES);
                }
                config.crowd.appearances = crowd;
            }
        }
        let retry = (config.crowd.count > 0).then(|| {
            let mut c = config.clone();
            c.crowd.count = 0;
            c.crowd.appearances.clear();
            c
        });
        let remote = self.remote.as_ref();
        let world = match World::for_walker(now, remote, config, WalkerSource::new(shared.clone()))
        {
            Err(e) if retry.is_some() => {
                tracing::warn!("world session refused the crowd ({e}); starting without it");
                retry.map(|c| World::for_walker(now, remote, c, WalkerSource::new(shared)))
            }
            other => Some(other),
        };
        match world {
            Some(Ok(w)) => self.world = Some(w),
            Some(Err(e)) => tracing::error!("world session unavailable: {e}"),
            None => {}
        }
    }

    /// Starts (or stops) queueing the world session's entity events for the walker
    /// (no-op without a session).
    pub fn capture_walker_events(&mut self, on: bool) {
        if let Some(w) = &mut self.world {
            w.capture_entity_events(on);
        }
    }

    /// Whether the loader has delivered (or given up on) the showcase texture.
    #[must_use]
    pub fn texture_settled(&self) -> bool {
        self.texture_settled
    }

    /// Whether the loader thread has finished.
    #[must_use]
    pub fn loader_finished(&self) -> bool {
        self.loader.is_finished()
    }

    /// Handle one loader event; returns a texture to upload.
    fn on_loader_event(&mut self, ev: LoaderEvent) -> Option<DecodedTexture> {
        match ev {
            LoaderEvent::NoData { reason } => {
                self.plugins.data_unavailable();
                self.status = DataStatus::NoData(reason);
                self.texture_settled = true;
            }
            LoaderEvent::Opened { root, set, stats } => {
                if let Some(m) = self.plugins.music_mut() {
                    m.attach_data(Arc::clone(&set), &root);
                }
                self.plugins.attach_data(Arc::clone(&set));
                self.status = DataStatus::Ready { root, stats };
                self.archives = Some(set);
            }
            LoaderEvent::EntriesCounted(n) => {
                if let DataStatus::Ready { stats, .. } = &mut self.status {
                    stats.entries = Some(n);
                }
            }
            LoaderEvent::Texture(t) => {
                self.texture_settled = true;
                return Some(t);
            }
            LoaderEvent::NoTexture { .. } => self.texture_settled = true,
            LoaderEvent::Finished => self.texture_settled = true,
        }
        None
    }

    /// Drain loader events without blocking; returns the newest texture to upload.
    pub fn poll_loader(&mut self) -> Option<DecodedTexture> {
        let mut tex = None;
        while let Some(ev) = self.loader.try_next() {
            if let Some(t) = self.on_loader_event(ev) {
                tex = Some(t);
            }
        }
        tex
    }

    /// Block up to `timeout` for the next loader event.
    pub fn wait_loader(&mut self, timeout: std::time::Duration) -> Option<DecodedTexture> {
        let ev = self.loader.next_timeout(timeout)?;
        self.on_loader_event(ev)
    }

    /// Per-frame update of the world session and the plugin host.
    pub fn tick(&mut self, now: Instant) {
        if let Some(world) = &mut self.world {
            world.update(now);
            for line in world.take_chat() {
                if line.sender.is_empty() && self.notices.len() < 64 {
                    self.notices.push(line.text.clone());
                }
                self.plugins.chat(&line.sender, &line.text);
            }
            let name = world
                .player()
                .and_then(|id| world.entities().get(&id))
                .map(|e| e.name.clone());
            if name != self.character {
                self.plugins.set_character(name.as_deref());
                self.character = name;
            }
        }
        self.plugins.pump();
    }

    /// System messages from the world session since the last call.
    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    /// Record the scene plus the world's entities into `frame`.
    pub fn draw(&self, scene: &Scene, frame: &mut Frame) {
        match &self.world {
            Some(world) => {
                let player = world.player();
                scene.draw(
                    frame,
                    world
                        .entities()
                        .iter()
                        .map(|(id, e)| (Some(*id) == player, e)),
                );
            }
            None => scene.draw(frame, std::iter::empty()),
        }
    }

    /// Orderly shutdown: plugins, then the world session, then the loader thread.
    pub fn shutdown(&mut self) {
        self.plugins.shutdown();
        if let Some(world) = &mut self.world {
            world.shutdown();
        }
        self.loader.shutdown();
        tracing::info!("runtime shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn install_paths_are_refused() {
        let install = tempfile::tempdir().expect("tempdir");
        let elsewhere = tempfile::tempdir().expect("tempdir");
        let root = install.path().to_str().expect("utf-8");
        let args = Args::try_parse_from(["t", "--game-dir", root]).expect("args");
        // Existing and not-yet-existing paths inside, also through `..` and a symlink.
        for p in [
            install.path().join("x.png"),
            install.path().join("new/dir/x.png"),
            elsewhere.path().join("../").join(
                install
                    .path()
                    .file_name()
                    .expect("name")
                    .to_str()
                    .expect("utf-8"),
            ),
        ] {
            assert!(refuse_install_path(&args, None, &p).is_err(), "{p:?}");
        }
        #[cfg(unix)]
        {
            let link = elsewhere.path().join("link");
            std::os::unix::fs::symlink(install.path(), &link).expect("symlink");
            assert!(refuse_install_path(&args, None, &link.join("y.csv")).is_err());
        }
        assert!(refuse_install_path(&args, None, &elsewhere.path().join("x.png")).is_ok());
        // The discovered install counts too.
        let plain = Args::try_parse_from(["t"]).expect("args");
        let status = DataStatus::Ready {
            root: install.path().to_owned(),
            stats: crate::data::ArchiveStats::default(),
        };
        assert!(refuse_install_path(&plain, Some(&status), &install.path().join("a")).is_err());
    }

    #[test]
    fn deadlines_never_overflow() {
        let now = Instant::now();
        assert_eq!(
            deadline_after(now, Duration::from_secs(5)),
            now + Duration::from_secs(5)
        );
        assert!(deadline_after(now, Duration::MAX) > now);
    }
}
