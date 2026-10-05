// SPDX-License-Identifier: GPL-3.0-or-later
//! Steam library and app-manifest discovery.
//!
//! Spec-Ref: docs/specs/install-layout.md#53-steam-on-linux-and-macos
//! Spec-Ref: docs/specs/install-layout.md#54-steam-on-windows

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crate::discover::SearchEnv;
use crate::error::{Error, Result};
use crate::fsutil;
use crate::vdf::{self, MAX_VDF_SIZE, VdfBlock};

/// Steam app id of the game whose data files this project reads.
pub const STEAM_APP_ID: &str = "212500";

/// Folder name under `steamapps/common/` used when no manifest is available.
pub const DEFAULT_STEAM_INSTALLDIR: &str = "Lord of the Rings Online";

/// One Steam library folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamLibrary {
    /// The library root (holds `steamapps/`).
    pub path: PathBuf,
    /// Whether `libraryfolders.vdf` lists the app in this library. Presence
    /// only; the listed size is never used.
    pub lists_app: bool,
}

/// Fields of `appmanifest_<id>.acf` this crate uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    /// `appid`.
    pub appid: String,
    /// `installdir`: a single folder name under `steamapps/common/`.
    pub installdir: Option<String>,
    /// `StateFlags` (4 = fully installed, by Steam convention).
    pub state_flags: Option<u32>,
    /// `buildid`: Steam's coarse build label.
    pub buildid: Option<u64>,
    /// `UserConfig` → `language`.
    pub language: Option<String>,
}

/// A game directory found through Steam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamApp {
    /// The library that holds the game.
    pub library: PathBuf,
    /// The parsed manifest, if one was found.
    pub manifest: Option<AppManifest>,
    /// `<library>/steamapps/common/<installdir>`.
    pub game_dir: PathBuf,
    /// `<library>/steamapps/compatdata/<id>/pfx`, if it exists.
    pub proton_prefix: Option<PathBuf>,
    /// Contents of `<library>/steamapps/compatdata/<id>/version`, trimmed.
    pub proton_version: Option<String>,
}

/// Candidate Steam roots that exist, in the order of §5.3 (Linux/macOS) or
/// §5.4 fallbacks (Windows), followed by `env.extra_steam_roots`.
pub fn steam_roots(env: &SearchEnv) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(home) = &env.home {
        for rel in [
            ".local/share/Steam",
            ".steam/steam",
            ".steam/root",
            ".steam",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            "snap/steam/common/.local/share/Steam",
            "Library/Application Support/Steam",
        ] {
            out.push(home.join(rel));
        }
    }
    if cfg!(windows) && env.system_locations {
        out.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        out.push(PathBuf::from(r"C:\Program Files\Steam"));
    }
    out.extend(env.extra_steam_roots.iter().cloned());
    dedup_existing(out)
}

fn dedup_existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in paths {
        if fsutil::is_dir(&p) && seen.insert(fsutil::canonical(&p)) {
            out.push(p);
        }
    }
    out
}

fn read_vdf(path: &Path) -> Result<VdfBlock> {
    let bytes = fsutil::read_capped(path, MAX_VDF_SIZE as u64)?;
    vdf::parse_bytes(&bytes).map_err(|source| Error::Vdf {
        path: path.to_path_buf(),
        source,
    })
}

/// Extract the libraries from a parsed `libraryfolders.vdf`. Accepts both the
/// current layout (numbered blocks with `path` and `apps`) and the legacy one
/// (numbered keys whose value is the path string).
pub fn parse_library_folders(doc: &VdfBlock) -> Vec<SteamLibrary> {
    let Some(top) = doc.get_block("libraryfolders") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, value) in top.iter() {
        if !key.bytes().all(|b| b.is_ascii_digit()) || key.is_empty() {
            continue;
        }
        let (path, lists_app) = match value {
            vdf::VdfValue::Str(p) => (p.as_str(), false),
            vdf::VdfValue::Block(b) => {
                let Some(p) = b.get_str("path") else { continue };
                let lists = b
                    .get_block("apps")
                    .is_some_and(|apps| apps.get(STEAM_APP_ID).is_some());
                (p, lists)
            }
        };
        if path.is_empty() {
            continue;
        }
        out.push(SteamLibrary {
            path: PathBuf::from(path),
            lists_app,
        });
    }
    out
}

/// Libraries of one Steam root. The root itself is always included.
pub fn libraries_of_root(root: &Path) -> Vec<SteamLibrary> {
    let mut libs = Vec::new();
    let candidates = [
        fsutil::resolve_path_ci(root, &["steamapps", "libraryfolders.vdf"]),
        fsutil::resolve_path_ci(root, &["config", "libraryfolders.vdf"]),
    ];
    if let Some(file) = candidates
        .into_iter()
        .flatten()
        .find(|p| fsutil::is_file(p))
    {
        match read_vdf(&file) {
            Ok(doc) => libs = parse_library_folders(&doc),
            Err(e) => tracing::debug!(error = %e, "cannot read library list"),
        }
    }
    let root_canon = fsutil::canonical(root);
    if !libs
        .iter()
        .any(|l| fsutil::canonical(&l.path) == root_canon)
    {
        libs.insert(
            0,
            SteamLibrary {
                path: root.to_path_buf(),
                lists_app: false,
            },
        );
    }
    libs
}

/// Parse the fields this crate uses from an app manifest document. Returns
/// `None` when the top-level `AppState` block is missing.
pub fn parse_app_manifest(doc: &VdfBlock) -> Option<AppManifest> {
    let st = doc.get_block("AppState")?;
    let installdir = st
        .get_str("installdir")
        .filter(|d| is_single_component(d))
        .map(str::to_owned);
    if st.get_str("installdir").is_some() && installdir.is_none() {
        tracing::warn!("app manifest installdir is not a single folder name; ignored");
    }
    Some(AppManifest {
        appid: st.get_str("appid").unwrap_or_default().to_owned(),
        installdir,
        state_flags: st.get_str("StateFlags").and_then(|s| s.trim().parse().ok()),
        buildid: st.get_str("buildid").and_then(|s| s.trim().parse().ok()),
        language: st
            .get_block("UserConfig")
            .and_then(|u| u.get_str("language"))
            .map(str::to_owned),
    })
}

/// Whether `s` names exactly one folder directly under `steamapps/common/` on every
/// platform (`docs/specs/install-layout.md` §5.3: `installdir` is a folder name there).
///
/// Besides separators and NUL this rejects what Windows would read as something else:
/// `:` (`C:` and `C:x` are drive-relative paths that replace the base when joined),
/// names ending in `.` or a space (Windows drops those, so `.. ` becomes `..`), control
/// characters, and reserved device names such as `CON` or `nul.txt`. A Steam folder
/// name never needs any of them, so they are refused on every platform.
fn is_single_component(s: &str) -> bool {
    if s.is_empty()
        || s.contains(['/', '\\', ':'])
        || s.chars().any(char::is_control)
        || s.ends_with(['.', ' '])
        || is_windows_device_name(s)
    {
        return false;
    }
    let mut parts = Path::new(s).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(c)), None) if c == std::ffi::OsStr::new(s)
    )
}

/// Windows device names, which name a device in every directory, with or without an
/// extension (`NUL`, `com1.txt`, `LPT²`).
fn is_windows_device_name(s: &str) -> bool {
    let stem = s.split('.').next().unwrap_or(s).trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let mut chars = upper.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let digit: Vec<char> = chars.collect();
    (prefix == "COM" || prefix == "LPT")
        && matches!(digit[..], [d] if d.is_ascii_digit() || matches!(d, '\u{b9}' | '\u{b2}' | '\u{b3}'))
}

/// Look for the game in one library.
pub fn find_in_library(lib: &SteamLibrary) -> Option<SteamApp> {
    let steamapps = fsutil::resolve_ci(&lib.path, "steamapps")?;
    let manifest = fsutil::resolve_ci(&steamapps, &format!("appmanifest_{STEAM_APP_ID}.acf"))
        .filter(|p| fsutil::is_file(p))
        .and_then(|p| match read_vdf(&p) {
            Ok(doc) => parse_app_manifest(&doc),
            Err(e) => {
                tracing::debug!(error = %e, "cannot read app manifest");
                None
            }
        })
        .filter(|m| {
            let ok = m.appid == STEAM_APP_ID;
            if !ok {
                tracing::warn!(appid = %m.appid, "app manifest has an unexpected appid");
            }
            ok
        });
    if manifest.is_none() && !lib.lists_app {
        return None;
    }
    let installdir = manifest
        .as_ref()
        .and_then(|m| m.installdir.as_deref())
        .unwrap_or(DEFAULT_STEAM_INSTALLDIR);
    let game_dir = fsutil::resolve_path_ci(&steamapps, &["common", installdir])
        .unwrap_or_else(|| steamapps.join("common").join(installdir));
    let compat = fsutil::resolve_path_ci(&steamapps, &["compatdata", STEAM_APP_ID]);
    let proton_prefix = compat
        .as_ref()
        .and_then(|c| fsutil::resolve_ci(c, "pfx"))
        .filter(|p| fsutil::is_dir(p));
    let proton_version = compat
        .as_ref()
        .and_then(|c| fsutil::resolve_ci(c, "version"))
        .and_then(|p| fsutil::read_prefix(&p, 256).ok())
        .map(|b| String::from_utf8_lossy(&b).trim().to_owned())
        .filter(|s| !s.is_empty());
    Some(SteamApp {
        library: lib.path.clone(),
        manifest,
        game_dir,
        proton_prefix,
        proton_version,
    })
}

/// Every Steam library on this machine, de-duplicated, in root order.
pub fn all_libraries(env: &SearchEnv) -> Vec<SteamLibrary> {
    let mut out: Vec<SteamLibrary> = Vec::new();
    // Canonical path → index in `out`; a hash map keeps this linear even for
    // a huge (hostile) library list.
    let mut seen: HashMap<PathBuf, usize> = HashMap::new();
    for root in steam_roots(env) {
        for lib in libraries_of_root(&root) {
            let c = fsutil::canonical(&lib.path);
            if let Some(&i) = seen.get(&c) {
                out[i].lists_app |= lib.lists_app;
            } else {
                seen.insert(c, out.len());
                out.push(lib);
            }
        }
    }
    out
}

/// Find the game in every Steam library.
pub fn find_steam_apps(env: &SearchEnv) -> Vec<SteamApp> {
    let mut apps: Vec<SteamApp> = all_libraries(env)
        .iter()
        .filter_map(find_in_library)
        .collect();
    if cfg!(windows) && env.system_locations {
        let fallback = PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common")
            .join(DEFAULT_STEAM_INSTALLDIR);
        if !apps.iter().any(|a| a.game_dir == fallback) {
            apps.push(SteamApp {
                library: PathBuf::from(r"C:\Program Files (x86)\Steam"),
                manifest: None,
                game_dir: fallback,
                proton_prefix: None,
                proton_version: None,
            });
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/a/Steam"
		"label"		""
		"apps"
		{
			"22320"		"1"
		}
	}
	"1"
	{
		"path"		"/b/Lib"
		"apps"
		{
			"212500"		"0"
		}
	}
}
"#;

    #[test]
    fn library_folders() {
        let doc = vdf::parse(LIBS).unwrap();
        let libs = parse_library_folders(&doc);
        assert_eq!(libs.len(), 2);
        assert!(!libs[0].lists_app);
        assert!(libs[1].lists_app, "size 0 still counts as present");
        assert_eq!(libs[1].path, PathBuf::from("/b/Lib"));
    }

    #[test]
    fn legacy_library_folders() {
        let doc =
            vdf::parse(r#""LibraryFolders" { "TimeNextStatsReport" "1" "1" "/old/lib" }"#).unwrap();
        let libs = parse_library_folders(&doc);
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].path, PathBuf::from("/old/lib"));
    }

    #[test]
    fn manifest_fields() {
        let doc = vdf::parse(
            r#""AppState" { "appid" "212500" "StateFlags" "4" "installdir" "Game Dir"
               "buildid" "12345" "UserConfig" { "language" "english" } }"#,
        )
        .unwrap();
        let m = parse_app_manifest(&doc).unwrap();
        assert_eq!(m.installdir.as_deref(), Some("Game Dir"));
        assert_eq!(m.state_flags, Some(4));
        assert_eq!(m.buildid, Some(12345));
        assert_eq!(m.language.as_deref(), Some("english"));

        let doc =
            vdf::parse(r#""AppState" { "appid" "212500" "installdir" "../../etc" }"#).unwrap();
        assert_eq!(parse_app_manifest(&doc).unwrap().installdir, None);
    }

    /// Regression: `installdir` values that Windows reads as a drive-relative path, a
    /// device or a name it rewrites are refused on every platform.
    #[test]
    fn installdir_must_be_one_plain_folder_name() {
        for ok in [
            "Game Dir",
            "Some Game Online",
            "a.b",
            "x",
            "CONSOLE",
            "COM10",
        ] {
            assert!(is_single_component(ok), "{ok:?}");
        }
        for bad in [
            "",
            ".",
            "..",
            "...",
            ".. ",
            "x.",
            "x ",
            "C:",
            "C:foo",
            "c:\\x",
            "a/b",
            "a\\b",
            "a\0b",
            "a\nb",
            "CON",
            "con",
            "nul.txt",
            "Aux .log",
            "COM1",
            "lpt9.x",
            "COM\u{b9}",
            "CONIN$",
            "\\\\?\\x",
            "/abs",
        ] {
            assert!(!is_single_component(bad), "{bad:?}");
            let doc = vdf::parse(&format!(
                r#""AppState" {{ "appid" "212500" "installdir" "{}" }}"#,
                bad.replace('\\', "\\\\").replace('"', "\\\"")
            ));
            if let Ok(doc) = doc {
                assert_eq!(
                    parse_app_manifest(&doc).unwrap().installdir,
                    None,
                    "{bad:?}"
                );
            }
        }
    }
}
