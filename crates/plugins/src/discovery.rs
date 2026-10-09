// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin discovery: the list of available plugins.
//!
//! Spec: `plugin-descriptor.md` §4.2 (algorithm, order), §4.3 (invariants);
//! `lua-plugin-api.md` §4.3.

use std::path::{Path, PathBuf};

use crate::descriptor::{MAX_DESCRIPTOR_BYTES, PluginDescriptor, parse_descriptor};
use crate::paths::{MAX_DIR_ENTRIES, is_confined, name_order};

/// A plugin found by discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailablePlugin {
    /// The parsed descriptor.
    pub descriptor: PluginDescriptor,
    /// Path of the `.plugin` file.
    pub path: PathBuf,
}

impl AvailablePlugin {
    /// Apartment this plugin loads into.
    #[must_use]
    pub fn apartment(&self) -> &str {
        self.descriptor.apartment_name()
    }
}

/// Outcome of a discovery scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshReport {
    /// Number of plugins now available.
    pub found: usize,
    /// Descriptors skipped, with the reason.
    pub skipped: Vec<(PathBuf, String)>,
}

/// Does `name` match `*.plugin`, case-insensitively, over the whole name (§4.2 step 3)?
fn is_descriptor_name(name: &str) -> bool {
    name.len() >= ".plugin".len()
        && name
            .get(name.len() - ".plugin".len()..)
            .is_some_and(|ext| ext.eq_ignore_ascii_case(".plugin"))
}

/// Entries of `dir` as (name, path, is_dir), at most [`MAX_DIR_ENTRIES`], sorted by the
/// Tessera name order (§4.2 "Order"). Names that are not valid UTF-8 are skipped. A
/// symlinked folder is never a folder here (discovery does not follow symlinks out of the
/// tree, §3.2, as the plugin file index does not enter them).
fn sorted_entries(dir: &Path) -> Option<Vec<(String, PathBuf, bool)>> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut out: Vec<(String, PathBuf, bool)> = rd
        .take(MAX_DIR_ENTRIES)
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let path = e.path();
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            Some((name, path, is_dir))
        })
        .collect();
    out.sort_by(|a, b| name_order(&a.0, &b.0));
    Some(out)
}

fn read_descriptor(plugins_dir: &Path, path: &Path) -> Result<PluginDescriptor, String> {
    // A symlinked descriptor is read only when it resolves inside `Plugins/` (§3.2).
    if !is_confined(plugins_dir, path) {
        return Err("resolves outside the plugins folder".to_owned());
    }
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".to_owned());
    }
    if meta.len() > MAX_DESCRIPTOR_BYTES as u64 {
        return Err(format!("descriptor is {} bytes", meta.len()));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    parse_descriptor(&bytes).map_err(|e| e.to_string())
}

/// Scan `plugins_dir` (§4.2): directories directly inside it, in name order; in each, the
/// `*.plugin` entries directly inside, in name order; parse each; the first descriptor of
/// a name (compared ASCII case-insensitively) wins. A missing folder gives an empty list.
#[must_use]
pub fn discover(plugins_dir: &Path) -> (Vec<AvailablePlugin>, RefreshReport) {
    let mut report = RefreshReport::default();
    let mut available: Vec<AvailablePlugin> = Vec::new();
    let Some(top) = sorted_entries(plugins_dir) else {
        tracing::debug!(dir = %plugins_dir.display(), "no plugins folder");
        return (available, report);
    };
    for (_, folder, is_dir) in top {
        if !is_dir {
            continue;
        }
        let Some(entries) = sorted_entries(&folder) else {
            continue;
        };
        for (name, path, _) in entries {
            if !is_descriptor_name(&name) {
                continue;
            }
            match read_descriptor(plugins_dir, &path) {
                Ok(descriptor) => {
                    if available
                        .iter()
                        .any(|a| a.descriptor.name.eq_ignore_ascii_case(&descriptor.name))
                    {
                        let why = format!("duplicate plugin name {:?}", descriptor.name);
                        tracing::warn!(path = %path.display(), "{why}");
                        report.skipped.push((path, why));
                    } else {
                        available.push(AvailablePlugin { descriptor, path });
                    }
                }
                Err(why) => {
                    tracing::warn!(path = %path.display(), "skipping plugin descriptor: {why}");
                    report.skipped.push((path, why));
                }
            }
        }
    }
    report.found = available.len();
    (available, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(name: &str) -> String {
        format!(
            "<Plugin><Information><Name>{name}</Name></Information><Package>P.{}</Package></Plugin>",
            name.replace(' ', "_")
        )
    }

    #[test]
    fn invariants() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let w = |rel: &str, body: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        w("a.plugin", &desc("Top"));
        w("A/B/c.plugin", &desc("Deep"));
        w("A/x.plugin", &desc("X"));
        w("A/Y.PLUGIN", &desc("Y"));
        w("A/x.plugin.bak", &desc("Bak"));
        w("A/.plugin", &desc("Dot"));
        w("A_b/s.plugin", &desc("Same"));
        w("Ab/s.plugin", &desc("Same"));
        w("C/alpha.plugin", &desc("Alpha"));
        w("C/beta.plugin", &desc("ALPHA"));
        w("C/broken.plugin", "<Plugin>");
        let (list, report) = discover(root);
        let names: Vec<&str> = list.iter().map(|a| a.descriptor.name.as_str()).collect();
        assert_eq!(names, ["Dot", "X", "Y", "Same", "Alpha"]);
        let same = list.iter().find(|a| a.descriptor.name == "Same").unwrap();
        assert!(same.path.starts_with(root.join("Ab")));
        assert_eq!(report.found, 5);
        assert_eq!(report.skipped.len(), 3); // the duplicate Same, ALPHA, and broken
        assert!(discover(&root.join("missing")).0.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_out_of_the_tree_are_not_followed() {
        // Review finding: a symlinked author folder (or descriptor) pointing outside
        // `Plugins/` was listed.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Plugins");
        let outside = dir.path().join("Outside");
        std::fs::create_dir_all(root.join("In")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("o.plugin"), desc("OutsideTree")).unwrap();
        std::fs::write(root.join("In/p.plugin"), desc("P")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("Link")).unwrap();
        std::os::unix::fs::symlink(outside.join("o.plugin"), root.join("In/l.plugin")).unwrap();
        // A symlinked descriptor that stays inside the tree is fine.
        std::fs::create_dir_all(root.join("Other")).unwrap();
        std::fs::write(root.join("Other/q.plugin"), desc("Q")).unwrap();
        std::os::unix::fs::symlink(root.join("Other/q.plugin"), root.join("In/q.plugin")).unwrap();
        let (list, report) = discover(&root);
        let names: Vec<&str> = list.iter().map(|a| a.descriptor.name.as_str()).collect();
        assert_eq!(names, ["P", "Q"]);
        assert!(
            report
                .skipped
                .iter()
                .any(|(p, why)| p.ends_with("In/l.plugin") && why.contains("outside"))
        );
    }
}
