// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin host of a start-screen run (`docs/specs/official-entry.md` §4.5).
//!
//! In a run that opens the start screen the client's plugin host is not started at start-up:
//! the offline walker's runtime starts its own when "Explore offline" starts the walker (with
//! the `--load-plugin` queue), and for official play [`PluginSlot`] would start one when real
//! character screens take the session at §8.7 (not while the placeholder shows, which is all
//! this revision has) and unloads every apartment and stops it before the account form or the
//! start screen shows again. No `--load-plugin` name is queued here (`--official` conflicts
//! with it; a `--start-screen` run drops its names once official play is chosen).

use std::path::PathBuf;

use super::flow::PluginHostControl;
use crate::plugins::Plugins;

/// A plugin host that exists only between [`PluginHostControl::start`] and
/// [`PluginHostControl::stop`].
#[derive(Debug)]
pub struct PluginSlot {
    plugins_dir: PathBuf,
    data_root: PathBuf,
    host: Option<Plugins>,
    starts: u32,
}

impl PluginSlot {
    /// A stopped host that would scan `plugins_dir` (read-only) and keep plugin data under
    /// `data_root`.
    #[must_use]
    pub fn new(plugins_dir: PathBuf, data_root: PathBuf) -> Self {
        Self {
            plugins_dir,
            data_root,
            host: None,
            starts: 0,
        }
    }
}

impl PluginHostControl for PluginSlot {
    fn start(&mut self) {
        if self.host.is_none() {
            self.starts += 1;
            self.host = Some(Plugins::start(
                &self.plugins_dir,
                self.data_root.clone(),
                &[],
            ));
        }
    }

    fn stop(&mut self) {
        if let Some(mut host) = self.host.take() {
            host.shutdown();
        }
    }

    fn is_running(&self) -> bool {
        self.host.is_some()
    }

    fn apartments(&self) -> usize {
        self.host
            .as_ref()
            .map_or(0, |h| h.manager().apartment_names().len())
    }

    fn starts(&self) -> u32 {
        self.starts
    }
}
