// SPDX-License-Identifier: GPL-3.0-or-later
//! Locale selection (`docs/specs/string-table.md` §10, `install-layout.md` §3.5).
//!
//! String tables live only in `client_local_<Lang>.dat`. One archive is chosen per
//! rendering session and there is no cross-locale fallback. The archive's header subset
//! word (0x150) is reported as the locale's language ID; behaviour must key off that ID,
//! not off the spelling of the archive name.

use std::fmt;
use std::path::{Path, PathBuf};

use tessera_dat::{Archive, ArchiveId, ArchiveKind};

use crate::error::{Result, TextError};

/// A language ID: the language archive's header subset word, which is the plugin API's
/// language value (spec §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LanguageId(pub u32);

impl LanguageId {
    /// 0: invalid (Reported).
    pub const INVALID: Self = Self(0);
    /// 2: English (Confirmed: the English archive's subset word).
    pub const ENGLISH: Self = Self(2);
    /// English (GB) (Reported).
    pub const ENGLISH_GB: Self = Self(0x1000_0001);
    /// French (Reported).
    pub const FRENCH: Self = Self(0x1000_0003);
    /// German (Reported).
    pub const GERMAN: Self = Self(0x1000_0004);
    /// Russian (Reported, older API listing only).
    pub const RUSSIAN: Self = Self(0x1000_0007);

    /// A display name for a known value (for UI and logs only).
    #[must_use]
    pub fn known_name(self) -> Option<&'static str> {
        Some(match self {
            Self::INVALID => "invalid",
            Self::ENGLISH => "English",
            Self::ENGLISH_GB => "English (GB)",
            Self::FRENCH => "French",
            Self::GERMAN => "German",
            Self::RUSSIAN => "Russian",
            _ => return None,
        })
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.known_name() {
            Some(n) => write!(f, "{n} ({:#x})", self.0),
            None => write!(f, "{:#x}", self.0),
        }
    }
}

/// The selected locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    /// The language name exactly as written in the archive file name (also the value
    /// matched by `--language`).
    pub name: String,
    /// The archive's language ID (header subset word).
    pub language_id: LanguageId,
    /// Path of the language archive.
    pub path: PathBuf,
}

/// Chooses a language (spec §10.1): `preferred` if available (exact case first, then
/// ASCII case-insensitive), else `English`, else the first in lexicographic order.
///
/// A `preferred` language that is not available is logged and the default is used.
#[must_use]
pub fn select_language<'a, S: AsRef<str>>(
    available: &'a [S],
    preferred: Option<&str>,
) -> Option<&'a str> {
    let find = |name: &str| {
        available
            .iter()
            .map(AsRef::as_ref)
            .find(|l| *l == name)
            .or_else(|| {
                available
                    .iter()
                    .map(AsRef::as_ref)
                    .find(|l| l.eq_ignore_ascii_case(name))
            })
    };
    let chosen = preferred.and_then(find);
    if let Some(p) = preferred
        && chosen.is_none()
    {
        tracing::warn!(
            requested = p,
            "requested language not available; using the default"
        );
    }
    chosen
        .or_else(|| find("English"))
        .or_else(|| available.iter().map(AsRef::as_ref).min())
}

/// The language archives directly inside an install directory, as (name, path) sorted
/// by name. Only base archives (`client_local_<Lang>.dat`) are listed.
pub fn available_languages(dir: impl AsRef<Path>) -> Result<Vec<(String, PathBuf)>> {
    let dir = dir.as_ref();
    let rd = std::fs::read_dir(dir).map_err(|source| TextError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let mut out = Vec::new();
    for entry in rd {
        let entry = entry.map_err(|source| TextError::Io {
            path: dir.to_owned(),
            source,
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(ArchiveId {
            kind: ArchiveKind::Local(lang),
            part: 0,
        }) = ArchiveId::from_file_name(name)
        {
            out.push((lang, entry.path()));
        }
    }
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    Ok(out)
}

/// Selects and opens (read-only) the language archive of an install directory.
pub fn open_locale(dir: impl AsRef<Path>, preferred: Option<&str>) -> Result<(Locale, Archive)> {
    let dir = dir.as_ref();
    let langs = available_languages(dir)?;
    let names: Vec<&str> = langs.iter().map(|(n, _)| n.as_str()).collect();
    let chosen = select_language(&names, preferred)
        .ok_or_else(|| TextError::NoLocale(dir.to_owned()))?
        .to_owned();
    let path = langs
        .iter()
        .find(|(n, _)| *n == chosen)
        .map(|(_, p)| p.clone())
        .ok_or_else(|| TextError::NoLocale(dir.to_owned()))?;
    let archive = Archive::open(&path)?;
    let language_id = LanguageId(archive.header().subset);
    tracing::debug!(language = %chosen, id = %language_id, "selected locale");
    Ok((
        Locale {
            name: chosen,
            language_id,
            path,
        },
        archive,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_rule() {
        let langs = ["DE", "English", "FR"];
        assert_eq!(select_language(&langs, None), Some("English"));
        assert_eq!(select_language(&langs, Some("FR")), Some("FR"));
        assert_eq!(select_language(&langs, Some("fr")), Some("FR"));
        assert_eq!(select_language(&langs, Some("Klingon")), Some("English"));
        assert_eq!(select_language(&["FR", "DE"], None), Some("DE"));
        let empty: [&str; 0] = [];
        assert_eq!(select_language(&empty, Some("English")), None);
    }

    #[test]
    fn names() {
        assert_eq!(LanguageId(2).to_string(), "English (0x2)");
        assert_eq!(LanguageId(77).known_name(), None);
    }

    #[test]
    fn missing_dir_is_error() {
        assert!(available_languages("/nonexistent/tessera-text-test").is_err());
    }
}
