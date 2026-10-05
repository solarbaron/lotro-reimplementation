// SPDX-License-Identifier: GPL-3.0-or-later
//! Background start-up of the world: the world context (property dictionary, region and
//! block-map descriptors, `landblock.md` §5.1), the area-name resolver (`wstate.md` §11)
//! and the character catalog (race table, animation layers, weenie roles), loaded on
//! their own thread so the window stays responsive.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_world::{BundleOptions, WStateAreaNames, WorldContext};

use super::characters::catalog::CharacterCatalog;

/// What the start-up produces.
pub type Booted = (Arc<WorldContext>, Arc<CharacterCatalog>);

/// A running or finished world start-up.
#[derive(Debug)]
pub struct WorldBoot {
    handle: Option<JoinHandle<Result<Booted, String>>>,
}

impl WorldBoot {
    /// Starts loading from `set` (install root `root` for the language archive).
    ///
    /// # Errors
    ///
    /// When the thread cannot be spawned.
    pub fn spawn(
        set: Arc<ArchiveSet>,
        root: PathBuf,
        options: BundleOptions,
    ) -> std::io::Result<Self> {
        let handle = std::thread::Builder::new()
            .name("tessera-world-boot".into())
            .spawn(move || load(&set, &root, options))?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    /// The result once ready (never blocks).
    pub fn try_take(&mut self) -> Option<Result<Booted, String>> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            return Some(match self.handle.take()?.join() {
                Ok(r) => r,
                Err(_) => Err("world start-up panicked".into()),
            });
        }
        None
    }
}

fn load(
    set: &Arc<ArchiveSet>,
    root: &std::path::Path,
    options: BundleOptions,
) -> Result<Booted, String> {
    let t = Instant::now();
    let ctx = WorldContext::from_install(Arc::clone(set), options)
        .map_err(|e| format!("world context: {e}"))?;
    let ctx = match WStateAreaNames::open(Arc::clone(set), root, Some("English"))
        .or_else(|_| WStateAreaNames::open(Arc::clone(set), root, None))
    {
        Ok(names) => ctx.with_area_names(Arc::new(names)),
        Err(e) => {
            tracing::warn!("area names unavailable: {e}");
            ctx
        }
    };
    let catalog = CharacterCatalog::build(Arc::clone(set));
    tracing::info!(
        regions = ctx.regions().len(),
        bodies = catalog.bodies.len(),
        elapsed_ms = t.elapsed().as_millis(),
        "world context ready"
    );
    Ok((Arc::new(ctx), Arc::new(catalog)))
}
