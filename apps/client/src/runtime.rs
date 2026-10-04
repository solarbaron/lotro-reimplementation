// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the client runs besides presentation: the data loader, the world session and
//! the plugin host. Shared by the windowed client and `--smoke`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_install::SearchEnv;

use crate::cli::Args;
use crate::data::{ArchiveStats, DataLoader, DecodedTexture, LoaderConfig, LoaderEvent};
use crate::net::World;
use crate::plugins::Plugins;
use crate::scene::Scene;
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

impl Runtime {
    /// Start the loader thread, the world session and the plugin host.
    pub fn start(args: &Args, now: Instant) -> anyhow::Result<Self> {
        let loader = DataLoader::spawn(LoaderConfig {
            game_dir: args.game_dir.clone(),
            no_data: args.no_data,
            surface: args.surface,
            count_entries: !args.no_entry_count,
        })?;
        let world = match World::sandbox(now) {
            Ok(w) => Some(w),
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
        let plugins = Plugins::start(&plugins_dir, root.join("plugindata"), &args.load_plugins);
        Ok(Self {
            loader,
            status: DataStatus::Loading,
            archives: None,
            world,
            plugins,
            character: None,
            texture_settled: false,
        })
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
                self.status = DataStatus::NoData(reason);
                self.texture_settled = true;
            }
            LoaderEvent::Opened { root, set, stats } => {
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
