// SPDX-License-Identifier: GPL-3.0-or-later
//! Case-insensitive, confined path resolution under the plugins folder.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §4.4 (path rules) and §7.2 (package resolution).

use std::path::{Path, PathBuf};

/// Cap on directory entries examined per directory, so a hostile tree cannot stall the host.
const MAX_DIR_ENTRIES: usize = 65_536;

/// What a resolved path must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Dir,
}

/// A single path component is acceptable (§4.4): non-empty, not `.`/`..`, no separators,
/// drive colons or NUL.
pub fn is_valid_component(c: &str) -> bool {
    !c.is_empty() && c != "." && c != ".." && !c.contains(['/', '\\', ':', '\0'])
}

/// Split a resource path (`/` or `\` separated, relative to the plugins folder) into
/// components, rejecting absolute paths, drive letters, NUL and `.`/`..` (§4.4).
pub fn split_resource_path(path: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    if parts.iter().all(|p| is_valid_component(p)) {
        Some(parts)
    } else {
        None
    }
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase()
}

/// Resolve `components` below `root`, matching every component case-insensitively (first
/// match in byte order wins) and requiring the final path to be of `kind`. The result is
/// guaranteed (after following symlinks) to lie inside `root`.
pub(crate) fn resolve_ci(root: &Path, components: &[&str], kind: Kind) -> Option<PathBuf> {
    if components.is_empty() || !components.iter().all(|c| is_valid_component(c)) {
        return None;
    }
    let mut cur = root.to_path_buf();
    for (i, comp) in components.iter().enumerate() {
        let want = if i + 1 == components.len() {
            kind
        } else {
            Kind::Dir
        };
        cur = find_entry(&cur, comp, want)?;
    }
    let canon_root = root.canonicalize().ok()?;
    let canon = cur.canonicalize().ok()?;
    if canon.starts_with(&canon_root) {
        Some(cur)
    } else {
        tracing::warn!(path = %cur.display(), "plugin path escapes the plugins folder; ignored");
        None
    }
}

/// Resolve a dotted package name to its Lua file (§7.2 step 2): `C1/…/Ck.lua`, else
/// `C1/…/Ck/__init__.lua`, matched case-insensitively and confined to `plugins_dir`.
///
/// Returns the file and the number of leading components that form its namespace
/// (`k - 1` for a `.lua` file, `k` for an `__init__.lua`).
pub fn resolve_package(plugins_dir: &Path, package: &str) -> Option<(PathBuf, usize)> {
    let comps: Vec<&str> = package.split('.').collect();
    if !comps.iter().all(|c| is_valid_component(c)) {
        return None;
    }
    let k = comps.len();
    let file_name = format!("{}.lua", comps[k - 1]);
    let mut file: Vec<&str> = comps[..k - 1].to_vec();
    file.push(&file_name);
    if let Some(p) = resolve_ci(plugins_dir, &file, Kind::File) {
        return Some((p, k - 1));
    }
    let mut init = comps;
    init.push("__init__.lua");
    resolve_ci(plugins_dir, &init, Kind::File).map(|p| (p, k))
}

fn find_entry(dir: &Path, name: &str, kind: Kind) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut matches: Vec<String> = Vec::new();
    for entry in rd.take(MAX_DIR_ENTRIES).flatten() {
        let Ok(entry_name) = entry.file_name().into_string() else {
            continue;
        };
        if eq_ci(&entry_name, name) {
            matches.push(entry_name);
        }
    }
    matches.sort();
    matches
        .into_iter()
        .map(|m| dir.join(m))
        .find(|p| match kind {
            Kind::File => p.is_file(),
            Kind::Dir => p.is_dir(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components() {
        assert!(is_valid_component("abc"));
        for bad in ["", ".", "..", "a/b", "a\\b", "c:", "a\0"] {
            assert!(!is_valid_component(bad), "{bad:?}");
        }
        assert_eq!(
            split_resource_path("A/b\\c.tga"),
            Some(vec!["A", "b", "c.tga"])
        );
        assert_eq!(split_resource_path("/abs"), None);
        assert_eq!(split_resource_path("C:/x"), None);
        assert_eq!(split_resource_path("a/../b"), None);
    }

    #[test]
    fn case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Author/Plug")).unwrap();
        std::fs::write(dir.path().join("Author/Plug/main.lua"), b"").unwrap();
        let p = resolve_ci(dir.path(), &["author", "PLUG", "Main.lua"], Kind::File).unwrap();
        assert!(p.ends_with("Author/Plug/main.lua"));
        assert!(resolve_ci(dir.path(), &["author", "PLUG"], Kind::File).is_none());
        assert!(resolve_ci(dir.path(), &["author", ".."], Kind::Dir).is_none());
    }
}
