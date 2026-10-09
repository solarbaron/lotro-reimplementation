// SPDX-License-Identifier: GPL-3.0-or-later
//! Case-insensitive, confined path resolution under the plugins folder, the package-name
//! grammar and Tessera's name order.
//!
//! Spec: `plugin-descriptor.md` §3.2 (case and path rules), §4.2 (order), §7.1 (package
//! grammar), §7.2 (resolution); `lua-plugin-api.md` §4.4, §7.2.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Cap on directory entries examined per directory, so a hostile tree cannot stall the host.
pub(crate) const MAX_DIR_ENTRIES: usize = 65_536;

/// What a resolved path must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Dir,
}

/// A single path component is acceptable (§3.2): non-empty, not `.`/`..`, no separators,
/// drive colons or NUL.
#[must_use]
pub fn is_valid_component(c: &str) -> bool {
    !c.is_empty() && c != "." && c != ".." && !c.contains(['/', '\\', ':', '\0'])
}

/// Split a resource path (`/` or `\` separated, relative to the plugins folder) into
/// components, rejecting absolute paths, drive letters, NUL and `.`/`..` (§3.2).
#[must_use]
pub fn split_resource_path(path: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    if parts.iter().all(|p| is_valid_component(p)) {
        Some(parts)
    } else {
        None
    }
}

/// The package-name grammar (Tessera rule, `plugin-descriptor.md` §7.1): one or more
/// components separated by single dots, each one or more ASCII letters, ASCII digits or `_`.
#[must_use]
pub fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .split('.')
            .all(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

/// Tessera's "case-insensitive byte order" of names (`plugin-descriptor.md` §4.2): compare
/// the UTF-8 bytes with `a`–`z` replaced by `A`–`Z`; on a tie, the unmodified bytes.
#[must_use]
pub fn name_order(a: &str, b: &str) -> Ordering {
    let fold = |c: u8| if c.is_ascii_lowercase() { c - 0x20 } else { c };
    a.bytes()
        .map(fold)
        .cmp(b.bytes().map(fold))
        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
}

/// ASCII case-insensitive equality (the only folding Tessera does, §4.2).
#[must_use]
pub fn eq_fold(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Is `path` inside `root` once symlinks are followed? Both must exist.
pub(crate) fn is_confined(root: &Path, path: &Path) -> bool {
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(r), Ok(p)) => p.starts_with(r),
        _ => false,
    }
}

/// Resolve `components` below `root`, matching every component case-insensitively (first
/// match in name order wins) and requiring the final path to be of `kind`. The result is
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
    if is_confined(root, &cur) {
        Some(cur)
    } else {
        tracing::warn!(path = %cur.display(), "plugin path escapes the plugins folder; ignored");
        None
    }
}

/// How a package resolved on disk (`plugin-descriptor.md` §7.2 step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskPackage {
    /// The chunk to run.
    pub file: PathBuf,
    /// `true` for a folder package (`D/__init__.lua`), `false` for a file package
    /// (`D.lua`).
    pub folder: bool,
}

impl DiskPackage {
    /// Number of leading name components that form the namespace: `k` for a folder
    /// package, `k - 1` for a file package.
    #[must_use]
    pub fn namespace_len(&self, components: usize) -> usize {
        if self.folder {
            components
        } else {
            components.saturating_sub(1)
        }
    }
}

/// Resolve a dotted package name on disk (`plugin-descriptor.md` §7.2 step 3): with
/// `D = plugins_dir/C1/…/Ck` matched case-insensitively, a directory `D` gives the folder
/// package `D/__init__.lua` (and no fallback to `D.lua` when that file is missing);
/// otherwise the file package `D.lua`. The chunk must exist, be a file and lie inside
/// `plugins_dir`. Names that fail the grammar never resolve.
#[must_use]
pub fn resolve_package(plugins_dir: &Path, package: &str) -> Option<DiskPackage> {
    if !is_valid_package_name(package) {
        return None;
    }
    let comps: Vec<&str> = package.split('.').collect();
    if resolve_ci(plugins_dir, &comps, Kind::Dir).is_some() {
        let mut init = comps;
        init.push("__init__.lua");
        return resolve_ci(plugins_dir, &init, Kind::File)
            .map(|file| DiskPackage { file, folder: true });
    }
    let k = comps.len();
    let file_name = format!("{}.lua", comps[k - 1]);
    let mut file: Vec<&str> = comps[..k - 1].to_vec();
    file.push(&file_name);
    resolve_ci(plugins_dir, &file, Kind::File).map(|file| DiskPackage {
        file,
        folder: false,
    })
}

fn find_entry(dir: &Path, name: &str, kind: Kind) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut matches: Vec<String> = Vec::new();
    for entry in rd.take(MAX_DIR_ENTRIES).flatten() {
        let Ok(entry_name) = entry.file_name().into_string() else {
            continue;
        };
        if eq_fold(&entry_name, name) {
            matches.push(entry_name);
        }
    }
    if matches.len() > 1 {
        tracing::warn!(dir = %dir.display(), name, "several entries match case-insensitively; taking the first");
    }
    matches.sort_by(|a, b| name_order(a, b));
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
    fn package_grammar() {
        for good in ["A", "A.B", "a_1.B2._"] {
            assert!(is_valid_package_name(good), "{good}");
        }
        for bad in ["", "A..B", ".A", "A.", "A-B.C", "A.B/C", "\u{e9}", "A B"] {
            assert!(!is_valid_package_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn order_rule() {
        assert_eq!(name_order("Ab", "A_b"), Ordering::Less);
        assert_eq!(name_order("abc", "ABCD"), Ordering::Less);
        assert_eq!(name_order("B", "b"), Ordering::Less);
        assert_eq!(name_order("x", "x"), Ordering::Equal);
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

    #[test]
    fn folder_before_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("A/B")).unwrap();
        std::fs::write(root.join("A/B.lua"), b"").unwrap();
        // A folder without __init__.lua does not fall back to the sibling file.
        assert_eq!(resolve_package(root, "A.B"), None);
        std::fs::write(root.join("A/B/__init__.lua"), b"").unwrap();
        let p = resolve_package(root, "a.b").unwrap();
        assert!(p.folder && p.file.ends_with("A/B/__init__.lua"));
        assert_eq!(p.namespace_len(2), 2);
        std::fs::write(root.join("A/Main.lua"), b"").unwrap();
        let p = resolve_package(root, "A.Main").unwrap();
        assert!(!p.folder && p.namespace_len(2) == 1);
        assert_eq!(resolve_package(root, "A.Missing"), None);
        assert_eq!(resolve_package(root, "A-2"), None);
    }
}
