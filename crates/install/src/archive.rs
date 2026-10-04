// SPDX-License-Identifier: GPL-3.0-or-later
//! Archive file names and the few header fields needed for identification.
//!
//! Spec-Ref: docs/specs/install-layout.md#31-archive-name-grammar
//! Spec-Ref: docs/specs/install-layout.md#33-required-and-optional-archives
//! Spec-Ref: docs/specs/install-layout.md#91-header-fields-used
//!
//! This module deliberately does not depend on the dat container crate. It
//! reads only the first [`HEADER_READ_LEN`] bytes of an archive with plain
//! `std::fs` reads, opening the file read-only.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::fsutil;

/// Number of bytes read from the start of an archive for identification.
pub const HEADER_READ_LEN: usize = 0x1A8;

/// The bytes a header must have for every field in [`ArchiveHeader`].
const HEADER_MIN_LEN: usize = 0x1A0;

/// Required value of the u32 at offset 0x140.
pub const ARCHIVE_MAGIC: u32 = 0x0000_5442;

/// Smallest size a core archive may have (§4).
pub const MIN_CORE_ARCHIVE_SIZE: u64 = 1024;

/// Highest aux member index probed (§3.4).
pub const MAX_AUX_INDEX: u32 = 16;

/// What an archive file is, derived from its name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArchiveKind {
    /// `client_general.dat`
    General,
    /// `client_gamelogic.dat`
    Gamelogic,
    /// `client_anim.dat`
    Anim,
    /// `client_mesh.dat`
    Mesh,
    /// `client_sound.dat`
    Sound,
    /// `client_surface.dat`
    Surface,
    /// `client_highres.dat`
    Highres,
    /// `client_local_<Lang>.dat`; the language name keeps its written case.
    Local(String),
    /// `client_cell_<r>.dat`
    Cell(u32),
    /// `client_map_<r>.dat`
    Map(u32),
    /// Any other `client_<word>.dat` with a lower-case letter word.
    Other(String),
}

/// How important an archive is for a usable install (§3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Without it the directory is not a usable install.
    Core,
    /// Usable without it, but flagged incomplete.
    Content,
    /// Usable without it.
    Optional,
    /// Not classified by the spec.
    Unknown,
}

impl ArchiveKind {
    /// The archive key: the `<key>` in `client_<key>.dat`.
    pub fn key(&self) -> String {
        match self {
            Self::General => "general".into(),
            Self::Gamelogic => "gamelogic".into(),
            Self::Anim => "anim".into(),
            Self::Mesh => "mesh".into(),
            Self::Sound => "sound".into(),
            Self::Surface => "surface".into(),
            Self::Highres => "highres".into(),
            Self::Local(l) => format!("local_{l}"),
            Self::Cell(r) => format!("cell_{r}"),
            Self::Map(r) => format!("map_{r}"),
            Self::Other(w) => w.clone(),
        }
    }

    /// The tier of this archive.
    pub fn tier(&self) -> Tier {
        match self {
            Self::General | Self::Gamelogic | Self::Local(_) => Tier::Core,
            Self::Anim
            | Self::Mesh
            | Self::Sound
            | Self::Surface
            | Self::Cell(_)
            | Self::Map(_) => Tier::Content,
            Self::Highres => Tier::Optional,
            Self::Other(_) => Tier::Unknown,
        }
    }

    fn from_word(word: &str) -> Self {
        match word {
            "general" => Self::General,
            "gamelogic" => Self::Gamelogic,
            "anim" => Self::Anim,
            "mesh" => Self::Mesh,
            "sound" => Self::Sound,
            "surface" => Self::Surface,
            "highres" => Self::Highres,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Is this a shared-data archive that may be split into aux members?
    pub fn may_have_aux(&self) -> bool {
        matches!(
            self,
            Self::General
                | Self::Gamelogic
                | Self::Anim
                | Self::Mesh
                | Self::Sound
                | Self::Surface
                | Self::Highres
                | Self::Other(_)
        )
    }

    /// The header word at 0x14C observed for this kind in the reference
    /// install (§3.2). Used only for diagnostic warnings, never to reject.
    pub fn observed_kind_word(&self) -> Option<u32> {
        Some(match self {
            Self::General => 0x0103_0000,
            Self::Gamelogic => 0x0101_0000,
            Self::Mesh => 0x0102_0000,
            Self::Anim => 0x0104_0000,
            Self::Sound => 0x0107_0000,
            Self::Highres => 0x0108_0000,
            Self::Surface => 0x0109_0000,
            Self::Local(_) => 0x0303_0000,
            Self::Cell(_) => 0x0203_0000,
            Self::Map(_) => 0x020A_0000,
            Self::Other(_) => return None,
        })
    }
}

impl fmt::Display for ArchiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key())
    }
}

/// A parsed archive file name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchiveName {
    /// The kind of archive.
    pub kind: ArchiveKind,
    /// 0 for the `.dat` base, `n` for `_aux_<n>.datx`.
    pub aux_index: u32,
}

impl ArchiveName {
    /// The canonical file name for this archive.
    pub fn file_name(&self) -> String {
        if self.aux_index == 0 {
            format!("client_{}.dat", self.kind.key())
        } else {
            format!("client_{}_aux_{}.datx", self.kind.key(), self.aux_index)
        }
    }
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix)
    {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let cut = s.len().checked_sub(suffix.len())?;
    if s.is_char_boundary(cut) && s[cut..].eq_ignore_ascii_case(suffix) {
        Some(&s[..cut])
    } else {
        None
    }
}

/// Decimal with no leading zeros, fitting a u32.
fn parse_region(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0'))
    {
        return None;
    }
    s.parse().ok()
}

fn is_word(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_lang(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Parse a file name against the archive name grammar (§3.1). Matching is
/// ASCII case-insensitive; the language name keeps its case. Returns `None`
/// for anything that is not an archive name.
pub fn parse_archive_name(name: &str) -> Option<ArchiveName> {
    let rest = strip_prefix_ci(name, "client_")?;
    if let Some(stem) = strip_suffix_ci(rest, ".datx") {
        // <word>_aux_<n>
        let lower = stem.to_ascii_lowercase();
        let pos = lower.rfind("_aux_")?;
        let word = &lower[..pos];
        let n = parse_region(&lower[pos + 5..])?;
        if n == 0 || !is_word(word) {
            return None;
        }
        return Some(ArchiveName {
            kind: ArchiveKind::from_word(word),
            aux_index: n,
        });
    }
    let stem = strip_suffix_ci(rest, ".dat")?;
    if let Some(lang) = strip_prefix_ci(stem, "local_") {
        return is_lang(lang).then(|| ArchiveName {
            kind: ArchiveKind::Local(lang.to_owned()),
            aux_index: 0,
        });
    }
    if let Some(r) = strip_prefix_ci(stem, "cell_") {
        return parse_region(r).map(|r| ArchiveName {
            kind: ArchiveKind::Cell(r),
            aux_index: 0,
        });
    }
    if let Some(r) = strip_prefix_ci(stem, "map_") {
        return parse_region(r).map(|r| ArchiveName {
            kind: ArchiveKind::Map(r),
            aux_index: 0,
        });
    }
    let lower = stem.to_ascii_lowercase();
    is_word(&lower).then(|| ArchiveName {
        kind: ArchiveKind::from_word(&lower),
        aux_index: 0,
    })
}

/// The header fields used for validation and identification (§9.1).
///
/// Fields whose meaning is not known are kept under their offset name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchiveHeader {
    /// True when the 256-byte version string area at 0x000 starts with 0.
    pub version_area_empty: bool,
    /// The 4 bytes at 0x100 (observed `00 50 4C 00`).
    pub marker_100: [u8; 4],
    /// u32 at 0x140; must be [`ARCHIVE_MAGIC`].
    pub magic: u32,
    /// u32 at 0x144: block size.
    pub block_size: u32,
    /// u32 at 0x148: file length; must equal the real length.
    pub file_length: u32,
    /// u32 at 0x14C: archive kind word. The top byte is the family code.
    pub kind_word: u32,
    /// u32 at 0x150: region number or language code.
    pub sub_code: u32,
    /// u32 at 0x160: root directory node offset (layout field).
    pub root_node_offset: u32,
    /// u32 at 0x170.
    pub field_170: u32,
    /// u32 at 0x174 (observed 112 everywhere).
    pub field_174: u32,
    /// u32 at 0x178 (observed 3, or 0 for region archives).
    pub field_178: u32,
    /// 16 bytes at 0x17C.
    pub identifier_17c: [u8; 16],
    /// u32 at 0x18C.
    pub field_18c: u32,
    /// Three u32 at 0x190.
    pub field_190: [u32; 3],
    /// u32 at 0x19C.
    pub field_19c: u32,
}

/// Why header bytes could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("header truncated: {have} bytes, need {need}")]
pub struct HeaderTruncated {
    /// Bytes available.
    pub have: usize,
    /// Bytes needed.
    pub need: usize,
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    let mut w = [0u8; 4];
    w.copy_from_slice(&b[off..off + 4]);
    u32::from_le_bytes(w)
}

fn arr_at<const N: usize>(b: &[u8], off: usize) -> [u8; N] {
    let mut w = [0u8; N];
    w.copy_from_slice(&b[off..off + N]);
    w
}

impl ArchiveHeader {
    /// Parse header fields from the first bytes of an archive. Does not check
    /// the magic; see [`ArchiveHeader::magic_ok`].
    pub fn parse(bytes: &[u8]) -> Result<Self, HeaderTruncated> {
        if bytes.len() < HEADER_MIN_LEN {
            return Err(HeaderTruncated {
                have: bytes.len(),
                need: HEADER_MIN_LEN,
            });
        }
        let b = bytes;
        Ok(Self {
            version_area_empty: b[0] == 0,
            marker_100: arr_at(b, 0x100),
            magic: u32_at(b, 0x140),
            block_size: u32_at(b, 0x144),
            file_length: u32_at(b, 0x148),
            kind_word: u32_at(b, 0x14C),
            sub_code: u32_at(b, 0x150),
            root_node_offset: u32_at(b, 0x160),
            field_170: u32_at(b, 0x170),
            field_174: u32_at(b, 0x174),
            field_178: u32_at(b, 0x178),
            identifier_17c: arr_at(b, 0x17C),
            field_18c: u32_at(b, 0x18C),
            field_190: [u32_at(b, 0x190), u32_at(b, 0x194), u32_at(b, 0x198)],
            field_19c: u32_at(b, 0x19C),
        })
    }

    /// Does the magic match?
    pub fn magic_ok(&self) -> bool {
        self.magic == ARCHIVE_MAGIC
    }

    /// The family code: the top byte of the kind word (1 shared data,
    /// 2 region data, 3 language data in the reference install).
    pub fn family(&self) -> u8 {
        self.kind_word.to_be_bytes()[0]
    }
}

/// Outcome of checking one archive's header against its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveStatus {
    /// Magic and length match.
    Ok,
    /// The magic at 0x140 is wrong (the value found is kept).
    BadMagic(u32),
    /// The header length differs from the real file length.
    SizeMismatch {
        /// Value of the header field at 0x148.
        header: u32,
        /// Real file length.
        actual: u64,
    },
    /// The file is shorter than the header.
    Truncated,
    /// The file could not be opened or read.
    Unreadable(String),
}

impl ArchiveStatus {
    /// True for [`ArchiveStatus::Ok`].
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// One archive file found in an install directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    /// What the archive is.
    pub kind: ArchiveKind,
    /// 0 for the base, `n` for aux member `n`.
    pub aux_index: u32,
    /// Full path, as found on disk (original case).
    pub path: PathBuf,
    /// Real file length in bytes.
    pub size: u64,
    /// Parsed header fields, when at least the header could be read.
    pub header: Option<ArchiveHeader>,
    /// Result of the header checks.
    pub status: ArchiveStatus,
}

impl ArchiveFile {
    /// File name as found on disk.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Inspect an archive: stat it and read its header. Never fails; problems
    /// end up in [`ArchiveFile::status`].
    pub fn inspect(path: &Path, name: ArchiveName) -> Self {
        let mut out = Self {
            kind: name.kind,
            aux_index: name.aux_index,
            path: path.to_path_buf(),
            size: 0,
            header: None,
            status: ArchiveStatus::Ok,
        };
        match std::fs::metadata(path) {
            Ok(m) => out.size = m.len(),
            Err(e) => {
                out.status = ArchiveStatus::Unreadable(e.to_string());
                return out;
            }
        }
        let bytes = match fsutil::read_prefix(path, HEADER_READ_LEN) {
            Ok(b) => b,
            Err(e) => {
                out.status = ArchiveStatus::Unreadable(e.to_string());
                return out;
            }
        };
        let header = match ArchiveHeader::parse(&bytes) {
            Ok(h) => h,
            Err(_) => {
                out.status = ArchiveStatus::Truncated;
                return out;
            }
        };
        out.status = if !header.magic_ok() {
            ArchiveStatus::BadMagic(header.magic)
        } else if u64::from(header.file_length) != out.size {
            ArchiveStatus::SizeMismatch {
                header: header.file_length,
                actual: out.size,
            }
        } else {
            ArchiveStatus::Ok
        };
        out.header = Some(header);
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build a synthetic header image of `len` bytes (at least
    /// [`HEADER_READ_LEN`]).
    pub(crate) fn synthetic_header(
        len: u32,
        block_size: u32,
        kind_word: u32,
        sub_code: u32,
    ) -> Vec<u8> {
        let mut b = vec![0u8; HEADER_READ_LEN];
        b[0x100..0x104].copy_from_slice(&[0, 0x50, 0x4C, 0]);
        b[0x140..0x144].copy_from_slice(&ARCHIVE_MAGIC.to_le_bytes());
        b[0x144..0x148].copy_from_slice(&block_size.to_le_bytes());
        b[0x148..0x14C].copy_from_slice(&len.to_le_bytes());
        b[0x14C..0x150].copy_from_slice(&kind_word.to_le_bytes());
        b[0x150..0x154].copy_from_slice(&sub_code.to_le_bytes());
        b[0x174..0x178].copy_from_slice(&112u32.to_le_bytes());
        let f178: u32 = if kind_word >> 24 == 2 { 0 } else { 3 };
        b[0x178..0x17C].copy_from_slice(&f178.to_le_bytes());
        b[0x17C..0x18C].copy_from_slice(&[0xAB; 16]);
        b[0x18C..0x190].copy_from_slice(&1100u32.to_le_bytes());
        b
    }

    #[test]
    fn parses_names() {
        let n = |s| parse_archive_name(s);
        assert_eq!(n("client_general.dat").unwrap().kind, ArchiveKind::General);
        assert_eq!(n("CLIENT_GENERAL.DAT").unwrap().kind, ArchiveKind::General);
        assert_eq!(
            n("client_local_English.dat").unwrap().kind,
            ArchiveKind::Local("English".into())
        );
        assert_eq!(n("client_cell_14.dat").unwrap().kind, ArchiveKind::Cell(14));
        assert_eq!(n("client_map_5.dat").unwrap().kind, ArchiveKind::Map(5));
        let aux = n("client_surface_aux_2.datx").unwrap();
        assert_eq!((aux.kind, aux.aux_index), (ArchiveKind::Surface, 2));
        assert_eq!(
            n("client_highres_aux_3.datx").unwrap().file_name(),
            "client_highres_aux_3.datx"
        );
        for bad in [
            "client_cell_05.dat",
            "client_cell_.dat",
            "client_mesh_aux_0.datx",
            "client_mesh_aux_01.datx",
            "client_mesh.datx",
            "client_cell_1_aux_1.datx",
            "client_local_.dat",
            "client_foo1.dat",
            "general.dat",
            "client_general.dat.bak",
            "client_",
            "client_ü.dat",
        ] {
            assert!(n(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn header_roundtrip() {
        let b = synthetic_header(4096, 1024, 0x0103_0000, 0);
        let h = ArchiveHeader::parse(&b).unwrap();
        assert!(h.magic_ok());
        assert_eq!(h.block_size, 1024);
        assert_eq!(h.file_length, 4096);
        assert_eq!(h.family(), 1);
        assert_eq!(h.field_174, 112);
        assert!(h.version_area_empty);
        assert!(ArchiveHeader::parse(&b[..0x19F]).is_err());
    }

    #[test]
    fn inspect_reports_status() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("client_general.dat");
        let mut img = synthetic_header(2048, 1024, 0x0103_0000, 0);
        img.resize(2048, 0);
        std::fs::write(&p, &img).unwrap();
        let name = parse_archive_name("client_general.dat").unwrap();
        assert_eq!(
            ArchiveFile::inspect(&p, name.clone()).status,
            ArchiveStatus::Ok
        );

        img.resize(4000, 0);
        std::fs::write(&p, &img).unwrap();
        assert!(matches!(
            ArchiveFile::inspect(&p, name.clone()).status,
            ArchiveStatus::SizeMismatch { .. }
        ));

        img[0x140] = 0;
        std::fs::write(&p, &img).unwrap();
        assert!(matches!(
            ArchiveFile::inspect(&p, name.clone()).status,
            ArchiveStatus::BadMagic(_)
        ));

        std::fs::write(&p, [0u8; 10]).unwrap();
        assert_eq!(
            ArchiveFile::inspect(&p, name.clone()).status,
            ArchiveStatus::Truncated
        );

        let missing = t.path().join("nope.dat");
        assert!(matches!(
            ArchiveFile::inspect(&missing, name).status,
            ArchiveStatus::Unreadable(_)
        ));
    }

    proptest::proptest! {
        #[test]
        fn name_parser_never_panics(s in "\\PC{0,40}") {
            let _ = parse_archive_name(&s);
        }

        #[test]
        fn header_parser_never_panics(b in proptest::collection::vec(proptest::num::u8::ANY, 0..0x200)) {
            let _ = ArchiveHeader::parse(&b);
        }
    }
}
