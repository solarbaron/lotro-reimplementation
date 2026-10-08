// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks against the user's own settings folder, read-only. Skipped (reported
//! as passed) unless `TESSERA_GAME_DIR` names an install.
//!
//! - Discovery and the plugin file index on the user's `Plugins/` folder: a missing folder
//!   gives an empty list and an empty index without error (`plugin-descriptor.md` §4.3).
//! - Every `PluginData/<account>/<world>/PluginOptions.xml` parses with the importer
//!   (`plugin-descriptor.md` §10.1–§10.3).
//! - Every descriptor found parses and its package resolves (§5, §7.2).

use std::path::Path;

use tessera_install::{GameInstall, SearchEnv, SettingsQuery};
use tessera_plugins::discovery::discover;
use tessera_plugins::options::parse_options;
use tessera_plugins::paths::resolve_package;
use tessera_plugins::vfs::PluginFileIndex;

#[test]
fn settings_folder_plugins_and_options() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("TESSERA_GAME_DIR is not set; skipping");
        return;
    };
    let inst = GameInstall::open(Path::new(&dir)).expect("install opens");
    let settings = inst.settings_dir(&SettingsQuery::default(), &SearchEnv::from_process());
    eprintln!(
        "settings folder: {} (exists: {})",
        settings.path.display(),
        settings.exists
    );
    let plugins = settings.path.join("Plugins");
    let (available, report) = discover(&plugins);
    let index = PluginFileIndex::build(&plugins);
    eprintln!(
        "Plugins/: {} descriptors, {} skipped, {} files indexed",
        available.len(),
        report.skipped.len(),
        index.file_count()
    );
    if !plugins.is_dir() {
        assert!(available.is_empty() && index.file_count() == 0);
    }
    for a in &available {
        assert!(
            resolve_package(&plugins, &a.descriptor.package).is_some(),
            "package of {} does not resolve",
            a.descriptor.name
        );
    }
    let mut files = 0;
    let data = settings.path.join("PluginData");
    for account in std::fs::read_dir(&data).into_iter().flatten().flatten() {
        for world in std::fs::read_dir(account.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let p = world.path().join("PluginOptions.xml");
            if !p.is_file() {
                continue;
            }
            let bytes = std::fs::read(&p).unwrap();
            let opts = parse_options(&bytes).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            eprintln!(
                "{}: {} autoload entries, {} scaling entries",
                p.display(),
                opts.autoload.len(),
                opts.scaling.len()
            );
            files += 1;
        }
    }
    eprintln!("{files} PluginOptions.xml files parsed");
}
