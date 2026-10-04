// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI fonts, taken from the system at start-up.
//!
//! The explorer embeds no font: egui's bundled fonts carry licences (OFL-1.1, Ubuntu Font
//! Licence) outside the project's allow list (`deny.toml`), so the `default_fonts`
//! feature is off and a sans-serif and a monospace font are loaded from the system
//! instead. Order of preference:
//!
//! 1. `--font <FILE>` or the `TESSERA_EXPLORER_FONT` environment variable (used for
//!    both families when no monospace font is found);
//! 2. fontconfig (`fc-match`), where available;
//! 3. well-known font files of Linux distributions, Windows and macOS.
//!
//! A symbol font covering the arrows and stars of the UI is added as a fallback when
//! one is found. Every font file is size-limited and parsed before egui sees it (egui
//! aborts on a font it cannot parse).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{FontData, FontDefinitions, FontFamily};
use skrifa::MetadataProvider;

use crate::error::ExplorerError;

/// Environment variable naming a font file to use.
pub const FONT_ENV: &str = "TESSERA_EXPLORER_FONT";

/// Largest font file accepted.
pub const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

/// Longest output of `fc-match` read.
const MAX_FC_OUTPUT: usize = 4096;

/// Characters the UI draws besides text (navigation arrows, bookmark stars).
const SYMBOLS: [char; 4] = ['◀', '▶', '★', '☆'];

/// A font file, read and checked.
#[derive(Clone)]
pub struct LoadedFont {
    /// Where it was read from.
    pub path: PathBuf,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for LoadedFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedFont")
            .field("path", &self.path)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// The fonts of the UI.
#[derive(Debug, Clone)]
pub struct UiFonts {
    /// Text font.
    pub proportional: LoadedFont,
    /// Fixed-width font (hex dumps, DID lists); the text font when none was found.
    pub monospace: Option<LoadedFont>,
    /// Fallback for the UI's symbols, if a font with them was found.
    pub symbols: Option<LoadedFont>,
}

/// Checks that `bytes` hold a font egui can use: it parses (face 0) and maps the
/// letter `A`.
///
/// # Errors
///
/// [`ExplorerError::Output`] with the reason.
pub fn check_font(bytes: &[u8]) -> Result<(), ExplorerError> {
    let font = skrifa::FontRef::from_index(bytes, 0)
        .map_err(|e| ExplorerError::Output(format!("not a usable TrueType/OpenType font: {e}")))?;
    if font.charmap().map('A').is_none() {
        return Err(ExplorerError::Output(
            "the font has no glyph for latin letters".into(),
        ));
    }
    Ok(())
}

/// Whether the parsed font maps every character of `chars`.
fn covers(bytes: &[u8], chars: &[char]) -> bool {
    skrifa::FontRef::from_index(bytes, 0)
        .is_ok_and(|f| chars.iter().all(|&c| f.charmap().map(c).is_some()))
}

/// Reads and checks the font file at `path` (at most [`MAX_FONT_BYTES`]).
///
/// # Errors
///
/// [`ExplorerError::Output`] when the file cannot be read, is too large, or is not a
/// usable font ([`check_font`]).
pub fn load_font(path: &Path) -> Result<LoadedFont, ExplorerError> {
    let err = |what: String| ExplorerError::Output(format!("font {}: {what}", path.display()));
    let meta = std::fs::metadata(path).map_err(|e| err(e.to_string()))?;
    if !meta.is_file() {
        return Err(err("not a file".into()));
    }
    if meta.len() > MAX_FONT_BYTES {
        return Err(err(format!("larger than {MAX_FONT_BYTES} bytes")));
    }
    let bytes = std::fs::read(path).map_err(|e| err(e.to_string()))?;
    if bytes.len() as u64 > MAX_FONT_BYTES {
        return Err(err(format!("larger than {MAX_FONT_BYTES} bytes")));
    }
    check_font(&bytes).map_err(|e| err(e.to_string()))?;
    Ok(LoadedFont {
        path: path.to_path_buf(),
        bytes,
    })
}

/// The file fontconfig picks for `pattern`, if `fc-match` runs and names one.
fn fc_match(pattern: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", pattern])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() || out.stdout.len() > MAX_FC_OUTPUT {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let p = PathBuf::from(s.trim());
    p.is_absolute().then_some(p)
}

/// Well-known font files: (text, monospace, symbols).
fn known_files() -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
    let p = |s: &str| PathBuf::from(s);
    let mut text = Vec::new();
    let mut mono = Vec::new();
    let mut symbols = Vec::new();
    if cfg!(windows) {
        let fonts = std::env::var_os("WINDIR")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("Fonts");
        text.extend(["segoeui.ttf", "arial.ttf", "tahoma.ttf"].map(|f| fonts.join(f)));
        mono.extend(["consola.ttf", "cour.ttf"].map(|f| fonts.join(f)));
        symbols.extend(["seguisym.ttf"].map(|f| fonts.join(f)));
    } else if cfg!(target_os = "macos") {
        text.extend(
            [
                "/System/Library/Fonts/Supplemental/Arial.ttf",
                "/Library/Fonts/Arial.ttf",
                "/System/Library/Fonts/Geneva.ttf",
            ]
            .map(p),
        );
        mono.extend(
            [
                "/System/Library/Fonts/Supplemental/Courier New.ttf",
                "/System/Library/Fonts/Monaco.ttf",
            ]
            .map(p),
        );
        symbols.extend(["/System/Library/Fonts/Apple Symbols.ttf"].map(p));
    } else {
        for dir in ["/usr/share/fonts", "/usr/local/share/fonts"] {
            for (list, names) in [
                (
                    &mut text,
                    &[
                        "dejavu/DejaVuSans.ttf",
                        "TTF/DejaVuSans.ttf",
                        "truetype/dejavu/DejaVuSans.ttf",
                        "liberation-fonts/LiberationSans-Regular.ttf",
                        "truetype/liberation/LiberationSans-Regular.ttf",
                        "noto/NotoSans-Regular.ttf",
                        "truetype/noto/NotoSans-Regular.ttf",
                    ][..],
                ),
                (
                    &mut mono,
                    &[
                        "dejavu/DejaVuSansMono.ttf",
                        "TTF/DejaVuSansMono.ttf",
                        "truetype/dejavu/DejaVuSansMono.ttf",
                        "liberation-fonts/LiberationMono-Regular.ttf",
                        "truetype/liberation/LiberationMono-Regular.ttf",
                        "noto/NotoSansMono-Regular.ttf",
                        "truetype/noto/NotoSansMono-Regular.ttf",
                    ][..],
                ),
                (
                    &mut symbols,
                    &[
                        "dejavu/DejaVuSans.ttf",
                        "TTF/DejaVuSans.ttf",
                        "truetype/dejavu/DejaVuSans.ttf",
                        "noto/NotoSansSymbols2-Regular.ttf",
                        "truetype/noto/NotoSansSymbols2-Regular.ttf",
                    ][..],
                ),
            ] {
                list.extend(names.iter().map(|n| Path::new(dir).join(n)));
            }
        }
    }
    (text, mono, symbols)
}

/// The first of `candidates` that loads (and, if `need` is not empty, covers `need`).
fn first_usable(
    candidates: impl IntoIterator<Item = PathBuf>,
    need: &[char],
) -> Option<LoadedFont> {
    candidates.into_iter().find_map(|p| {
        let f = load_font(&p).ok()?;
        (need.is_empty() || covers(&f.bytes, need)).then_some(f)
    })
}

/// Finds the UI fonts (see the module documentation). `explicit` is the `--font`
/// option; [`FONT_ENV`] is read when it is `None`.
///
/// # Errors
///
/// When an explicitly named font does not load, or no usable text font is found.
pub fn find_fonts(explicit: Option<&Path>) -> Result<UiFonts, ExplorerError> {
    let env = std::env::var_os(FONT_ENV).map(PathBuf::from);
    let explicit = explicit.map(Path::to_path_buf).or(env);
    let (text, mono, symbols) = known_files();
    let proportional = match &explicit {
        Some(p) => load_font(p)?,
        None => {
            first_usable(fc_match("sans-serif").into_iter().chain(text), &[]).ok_or_else(|| {
                ExplorerError::Output(format!(
                    "no usable system font found; name a .ttf/.otf file with --font or {FONT_ENV}"
                ))
            })?
        }
    };
    let monospace = first_usable(fc_match("monospace").into_iter().chain(mono), &[]);
    let symbols = if covers(&proportional.bytes, &SYMBOLS) {
        None
    } else {
        let pattern = format!(
            "sans-serif:charset={}",
            SYMBOLS.map(|c| format!("{:x}", u32::from(c))).join(" ")
        );
        first_usable(fc_match(&pattern).into_iter().chain(symbols), &SYMBOLS)
    };
    Ok(UiFonts {
        proportional,
        monospace,
        symbols,
    })
}

/// egui font definitions for `fonts`.
#[must_use]
pub fn font_definitions(fonts: &UiFonts) -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    let mut add = |name: &str, f: &LoadedFont| {
        defs.font_data.insert(
            name.to_owned(),
            Arc::new(FontData::from_owned(f.bytes.clone())),
        );
        name.to_owned()
    };
    let text = add("ui-text", &fonts.proportional);
    let mono = fonts.monospace.as_ref().map(|f| add("ui-mono", f));
    let symbols = fonts.symbols.as_ref().map(|f| add("ui-symbols", f));
    let mut prop_family = vec![text.clone()];
    let mut mono_family: Vec<String> = mono.into_iter().collect();
    mono_family.push(text);
    if let Some(s) = symbols {
        prop_family.push(s.clone());
        mono_family.push(s);
    }
    defs.families.insert(FontFamily::Proportional, prop_family);
    defs.families.insert(FontFamily::Monospace, mono_family);
    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_font_bytes_are_rejected() {
        assert!(check_font(&[]).is_err());
        assert!(check_font(b"not a font at all").is_err());
        // A TrueType tag with a table directory that points nowhere.
        let mut fake = vec![0, 1, 0, 0, 0, 0x20, 0, 0, 0, 0, 0, 0];
        fake.extend_from_slice(&[0xFF; 64]);
        assert!(check_font(&fake).is_err());
    }

    #[test]
    fn missing_or_odd_files_are_errors() {
        assert!(load_font(Path::new("/nonexistent/tessera/font.ttf")).is_err());
        assert!(load_font(&std::env::temp_dir()).is_err());
        let p = std::env::temp_dir().join(format!("tessera-font-{}.ttf", std::process::id()));
        std::fs::write(&p, b"\0\x01\0\0garbage").unwrap();
        assert!(load_font(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }

    /// The system's fonts load and make complete definitions (skipped on a system
    /// without fonts, as in minimal containers).
    #[test]
    fn system_fonts_make_definitions() {
        let Ok(fonts) = find_fonts(None) else {
            eprintln!("no system font; skipping");
            return;
        };
        let defs = font_definitions(&fonts);
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let names = &defs.families[&family];
            assert!(!names.is_empty());
            assert!(names.iter().all(|n| defs.font_data.contains_key(n)));
        }
        // An explicit file is used as given.
        let again = find_fonts(Some(&fonts.proportional.path)).unwrap();
        assert_eq!(again.proportional.path, fonts.proportional.path);
    }
}
