// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal, bounded reader for the launcher config file (§6).
//!
//! Spec-Ref: docs/specs/install-layout.md#6-lotrolauncherconfig
//! Spec-Ref: docs/specs/install-layout.md#37-game-type-identification
//!
//! Only `add` elements directly under `configuration/appSettings` are read
//! (their `key` and `value` attributes). This is not a general XML parser:
//! it tokenises tags, skips comments, processing instructions and CDATA,
//! decodes the five predefined entities and numeric character references,
//! and **rejects** every `<!...>` declaration (`DOCTYPE`, `ENTITY`, ...), so
//! no DTD or external entity is ever processed. Input is hostile: the size
//! and element depth are bounded and malformed input is an error, never a
//! panic. The file is never written.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::fsutil::{self, DirEntry};

/// Largest launcher config read, in bytes (§6).
pub const MAX_LAUNCHER_CONFIG_SIZE: usize = 64 * 1024;

/// Deepest element nesting accepted.
pub const MAX_LAUNCHER_CONFIG_DEPTH: usize = 32;

/// Key holding the game identifier (§3.7 step 2).
pub const KEY_GAME_NAME: &str = "DataCenter.GameName";

/// Key holding the settings folder name (§8.1).
pub const KEY_DOCUMENT_FOLDER: &str = "Product.DocumentFolder";

/// The `appSettings` entries of a launcher config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LauncherConfig {
    /// `(key, value)` pairs in file order, duplicates kept.
    pub settings: Vec<(String, String)>,
}

impl LauncherConfig {
    /// The value of `key`: matched case-sensitively first, then
    /// case-insensitively. When a key repeats, the last one wins (§6).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.settings
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .or_else(|| {
                self.settings
                    .iter()
                    .rev()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
            })
            .map(|(_, v)| v.as_str())
    }

    /// `DataCenter.GameName`.
    pub fn game_name(&self) -> Option<&str> {
        self.get(KEY_GAME_NAME)
    }

    /// `Product.DocumentFolder`.
    pub fn document_folder(&self) -> Option<&str> {
        self.get(KEY_DOCUMENT_FOLDER)
    }
}

/// Why a launcher config could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LauncherConfigError {
    /// The input exceeds [`MAX_LAUNCHER_CONFIG_SIZE`].
    #[error("input too large")]
    TooLarge,
    /// A `<!DOCTYPE`, `<!ENTITY` or other declaration was found.
    #[error("declarations (DTD) are not accepted")]
    Declaration,
    /// A tag, comment, CDATA section or processing instruction is not closed.
    #[error("unterminated markup")]
    Unterminated,
    /// A tag is malformed (empty name, bad attribute syntax).
    #[error("malformed tag")]
    MalformedTag,
    /// An end tag does not match the open element.
    #[error("mismatched end tag")]
    Mismatched,
    /// Elements are still open at the end of the input.
    #[error("unclosed element at end of input")]
    Unclosed,
    /// Elements are nested deeper than [`MAX_LAUNCHER_CONFIG_DEPTH`].
    #[error("elements nested too deeply")]
    TooDeep,
    /// An entity reference other than the predefined ones or a valid
    /// character reference.
    #[error("unknown or invalid entity reference")]
    BadEntity,
}

type PResult<T> = std::result::Result<T, LauncherConfigError>;

/// Decode `&lt;` `&gt;` `&amp;` `&quot;` `&apos;` and `&#N;` / `&#xN;`.
fn decode_entities(raw: &str) -> PResult<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        let end = after.find(';').ok_or(LauncherConfigError::BadEntity)?;
        let name = &after[..end];
        let ch = match name {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let num = name
                    .strip_prefix('#')
                    .ok_or(LauncherConfigError::BadEntity)?;
                let code = if let Some(hex) = num.strip_prefix(['x', 'X']) {
                    u32::from_str_radix(hex, 16)
                } else {
                    num.parse::<u32>()
                }
                .map_err(|_| LauncherConfigError::BadEntity)?;
                char::from_u32(code).ok_or(LauncherConfigError::BadEntity)?
            }
        };
        out.push(ch);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Byte offset of the `>` that ends the tag starting at `s[0] == '<'`,
/// skipping `>` inside quoted attribute values.
fn tag_end(s: &str) -> PResult<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Ok(i),
            _ => {}
        }
    }
    Err(LauncherConfigError::Unterminated)
}

fn is_name_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '/' | '>' | '=' | '<' | '"' | '\'')
}

/// A parsed start tag: its name, attributes and whether it is self-closing.
struct StartTag<'a> {
    name: &'a str,
    attrs: Vec<(&'a str, String)>,
    self_closing: bool,
}

/// Parse the inside of a start tag (between `<` and `>`).
fn parse_start_tag(inner: &str) -> PResult<StartTag<'_>> {
    let (body, self_closing) = match inner.strip_suffix('/') {
        Some(b) => (b, true),
        None => (inner, false),
    };
    let name_end = body
        .char_indices()
        .find(|&(_, c)| !is_name_char(c))
        .map_or(body.len(), |(i, _)| i);
    let name = &body[..name_end];
    if name.is_empty() {
        return Err(LauncherConfigError::MalformedTag);
    }
    let mut rest = &body[name_end..];
    let mut attrs = Vec::new();
    loop {
        let trimmed = rest.trim_start();
        if trimmed.is_empty() {
            break;
        }
        if trimmed.len() == rest.len() {
            // Attributes must be separated from the name and each other.
            return Err(LauncherConfigError::MalformedTag);
        }
        let an_end = trimmed
            .char_indices()
            .find(|&(_, c)| !is_name_char(c))
            .map_or(trimmed.len(), |(i, _)| i);
        let an = &trimmed[..an_end];
        if an.is_empty() {
            return Err(LauncherConfigError::MalformedTag);
        }
        let after = trimmed[an_end..].trim_start();
        let after = after
            .strip_prefix('=')
            .ok_or(LauncherConfigError::MalformedTag)?
            .trim_start();
        let q = after
            .chars()
            .next()
            .filter(|c| matches!(c, '"' | '\''))
            .ok_or(LauncherConfigError::MalformedTag)?;
        let val_body = &after[1..];
        let close = val_body.find(q).ok_or(LauncherConfigError::MalformedTag)?;
        attrs.push((an, decode_entities(&val_body[..close])?));
        rest = &val_body[close + 1..];
    }
    Ok(StartTag {
        name,
        attrs,
        self_closing,
    })
}

/// Skip markup that starts at `s[0] == '<'` and ends with `terminator`.
/// Returns the byte length consumed.
fn skip_until(s: &str, terminator: &str) -> PResult<usize> {
    s.find(terminator)
        .map(|i| i + terminator.len())
        .ok_or(LauncherConfigError::Unterminated)
}

/// Parse launcher config bytes (UTF-8, BOM optional; invalid sequences are
/// replaced). The size cap applies to the raw bytes.
pub fn parse_launcher_config(bytes: &[u8]) -> PResult<LauncherConfig> {
    if bytes.len() > MAX_LAUNCHER_CONFIG_SIZE {
        return Err(LauncherConfigError::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    let mut cfg = LauncherConfig::default();
    let mut stack: Vec<&str> = Vec::new();
    let mut s: &str = &text;
    while let Some(pos) = s.find('<') {
        s = &s[pos..];
        if s.starts_with("<!--") {
            s = &s[skip_until(s, "-->")?..];
        } else if s.starts_with("<![CDATA[") {
            s = &s[skip_until(s, "]]>")?..];
        } else if s.starts_with("<!") {
            return Err(LauncherConfigError::Declaration);
        } else if s.starts_with("<?") {
            s = &s[skip_until(s, "?>")?..];
        } else if let Some(rest) = s.strip_prefix("</") {
            let end = rest.find('>').ok_or(LauncherConfigError::Unterminated)?;
            let name = rest[..end].trim();
            if stack.pop() != Some(name) {
                return Err(LauncherConfigError::Mismatched);
            }
            s = &rest[end + 1..];
        } else {
            let end = tag_end(s)?;
            let tag = parse_start_tag(&s[1..end])?;
            if tag.name == "add" && stack.as_slice() == ["configuration", "appSettings"] {
                let mut key = None;
                let mut value = None;
                for (n, v) in tag.attrs {
                    match n {
                        "key" => key = Some(v),
                        "value" => value = Some(v),
                        _ => {}
                    }
                }
                if let Some(k) = key {
                    cfg.settings.push((k, value.unwrap_or_default()));
                }
            }
            if !tag.self_closing {
                if stack.len() >= MAX_LAUNCHER_CONFIG_DEPTH {
                    return Err(LauncherConfigError::TooDeep);
                }
                stack.push(tag.name);
            }
            s = &s[end + 1..];
        }
    }
    if !stack.is_empty() {
        return Err(LauncherConfigError::Unclosed);
    }
    Ok(cfg)
}

/// Read and parse a launcher config file (regular files only, at most
/// [`MAX_LAUNCHER_CONFIG_SIZE`] bytes).
pub fn read_launcher_config(path: &Path) -> Result<LauncherConfig> {
    let bytes = fsutil::read_capped(path, MAX_LAUNCHER_CONFIG_SIZE as u64)?;
    parse_launcher_config(&bytes).map_err(|source| Error::LauncherConfig {
        path: path.to_path_buf(),
        source,
    })
}

/// Launcher config files in `entries` (a listing of the install directory),
/// in the lookup order of P14: `<game_stem>.launcherconfig` (when given),
/// then any other `*.launcherconfig` (sorted by name), then
/// `TurbineLauncher.exe.config`, then `DNDLauncher.exe.config`. All names are
/// matched case-insensitively.
pub(crate) fn launcher_config_candidates(
    entries: &[DirEntry],
    game_stem: Option<&str>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(stem) = game_stem
        && let Some(e) = fsutil::pick_ci(entries, &format!("{stem}.launcherconfig"))
    {
        out.push(e.path.clone());
    }
    let mut others: Vec<&DirEntry> = entries
        .iter()
        .filter(|e| {
            let l = e.name.to_ascii_lowercase();
            l.len() > ".launcherconfig".len() && l.ends_with(".launcherconfig")
        })
        .collect();
    others.sort_by(|a, b| a.name.cmp(&b.name));
    for e in others {
        if !out.contains(&e.path) {
            out.push(e.path.clone());
        }
    }
    for name in ["TurbineLauncher.exe.config", "DNDLauncher.exe.config"] {
        if let Some(e) = fsutil::pick_ci(entries, name)
            && !out.contains(&e.path)
        {
            out.push(e.path.clone());
        }
    }
    out.retain(|p| fsutil::is_file(p));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n\
        <configuration>\r\n\
          <system.windows.forms jitDebugging=\"false\" />\r\n\
          <!-- a comment with <add key=\"x\" value=\"y\"/> inside -->\r\n\
          <startup><supportedRuntime version=\"v4.0\"/></startup>\r\n\
          <appSettings>\r\n\
            <add key=\"DataCenter.GameName\" value=\"LOTRO\" />\r\n\
            <add key='Product.DocumentFolder' value='A &amp; B &#x41;&#66;' />\r\n\
            <add key=\"DataCenter.GameName\" value=\"Second\" />\r\n\
            <add key=\"Gt\" value=\"a > b\"/>\r\n\
          </appSettings>\r\n\
          <other><appSettings><add key=\"Nested\" value=\"no\"/></appSettings></other>\r\n\
        </configuration>\r\n";

    #[test]
    fn reads_app_settings() {
        let c = parse_launcher_config(SAMPLE.as_bytes()).unwrap();
        assert_eq!(c.game_name(), Some("Second"), "last duplicate wins");
        assert_eq!(c.document_folder(), Some("A & B AB"));
        assert_eq!(c.get("gt"), Some("a > b"), "case-insensitive fallback");
        assert_eq!(c.get("Nested"), None, "only configuration/appSettings");
        assert_eq!(c.get("x"), None, "comments are skipped");
    }

    #[test]
    fn exact_case_beats_folded_case() {
        let c = parse_launcher_config(
            b"<configuration><appSettings><add key=\"K\" value=\"exact\"/>\
              <add key=\"k\" value=\"folded\"/></appSettings></configuration>",
        )
        .unwrap();
        assert_eq!(c.get("K"), Some("exact"));
        assert_eq!(c.get("k"), Some("folded"));
    }

    #[test]
    fn rejects_dtd_and_malformed_input() {
        let dtd = b"<?xml version=\"1.0\"?><!DOCTYPE c [<!ENTITY x SYSTEM \"file:///etc/passwd\">]>\
                    <configuration><appSettings><add key=\"a\" value=\"&x;\"/></appSettings></configuration>";
        assert_eq!(
            parse_launcher_config(dtd),
            Err(LauncherConfigError::Declaration)
        );
        for (input, err) in [
            (&b"<configuration>"[..], LauncherConfigError::Unclosed),
            (b"<a></b>", LauncherConfigError::Mismatched),
            (b"<a x=\"1></a>", LauncherConfigError::Unterminated),
            (b"<a x=1></a>", LauncherConfigError::MalformedTag),
            (b"< ></>", LauncherConfigError::MalformedTag),
            (b"<a x=\"&bogus;\"/>", LauncherConfigError::BadEntity),
            (b"<a x=\"&#xD800;\"/>", LauncherConfigError::BadEntity),
            (b"<!-- open", LauncherConfigError::Unterminated),
        ] {
            assert_eq!(parse_launcher_config(input), Err(err), "{input:?}");
        }
        let deep = "<a>".repeat(MAX_LAUNCHER_CONFIG_DEPTH + 1);
        assert_eq!(
            parse_launcher_config(deep.as_bytes()),
            Err(LauncherConfigError::TooDeep)
        );
        let big = vec![b' '; MAX_LAUNCHER_CONFIG_SIZE + 1];
        assert_eq!(
            parse_launcher_config(&big),
            Err(LauncherConfigError::TooLarge)
        );
    }

    proptest::proptest! {
        #[test]
        fn never_panics(b in proptest::collection::vec(proptest::num::u8::ANY, 0..2048)) {
            let _ = parse_launcher_config(&b);
        }

        #[test]
        fn never_panics_markup(s in "[<>/!?=\"' a-z&#;x0-9\\-\\[\\]]{0,400}") {
            let _ = parse_launcher_config(s.as_bytes());
        }
    }
}
