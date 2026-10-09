// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only importer of the original plugin manager's `PluginOptions.xml`.
//!
//! Tessera keeps its own autoload and scaling state; this reader only lets a client import
//! the original's choices once. It never writes the file.
//!
//! Spec: `plugin-descriptor.md` §10 (location, structure, reading rules, invariants);
//! `lua-plugin-api.md` §5.4.

use std::collections::HashSet;

use roxmltree::{Document, Node, ParsingOptions};

/// Largest options file read.
pub const MAX_OPTIONS_BYTES: usize = 4 * 1024 * 1024;

/// Upper bound on XML nodes.
const MAX_OPTIONS_NODES: u32 = 200_000;

/// One `AutoloadOptions/Plugin` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoloadEntry {
    /// The plugin's `Name`.
    pub name: String,
    /// `AllCharacters`, true only when written exactly `true`.
    pub all_characters: bool,
    /// Characters for which the plugin autoloads, in file order.
    pub characters: Vec<String>,
}

impl AutoloadEntry {
    /// Does this entry autoload for `character`?
    #[must_use]
    pub fn applies_to(&self, character: &str) -> bool {
        self.all_characters || self.characters.iter().any(|c| c == character)
    }
}

/// The imported content of one `PluginOptions.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginOptions {
    /// Autoload entries; a later element repeating a `Name` (exact, case-sensitive
    /// comparison) is dropped (§10.2).
    pub autoload: Vec<AutoloadEntry>,
    /// Scaling choices: (`Name`, `UseScaling`), true only when written exactly `True`.
    pub scaling: Vec<(String, bool)>,
}

impl PluginOptions {
    /// Plugins that autoload for `character`, in Tessera's list order (file order of the
    /// kept entries).
    pub fn autoload_for<'a>(&'a self, character: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.autoload
            .iter()
            .filter(move |e| e.applies_to(character))
            .map(|e| e.name.as_str())
    }

    /// `Plugin:GetUsesGlobalScaling()` for `name`, when the file has a choice.
    #[must_use]
    pub fn uses_scaling(&self, name: &str) -> Option<bool> {
        self.scaling
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }
}

/// Errors of the importer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OptionsError {
    /// File larger than [`MAX_OPTIONS_BYTES`].
    #[error("options file is {0} bytes, limit is {MAX_OPTIONS_BYTES}")]
    TooLarge(usize),
    /// Not UTF-8 (the original writes UTF-8).
    #[error("options file is not valid UTF-8")]
    NotUtf8,
    /// Malformed XML or a DTD.
    #[error("malformed XML: {0}")]
    Xml(String),
    /// The root is not `PluginOptions`.
    #[error("root element is <{0}>, expected <PluginOptions>")]
    WrongRoot(String),
}

fn elem<'a, 'i>(node: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

/// Parse a `PluginOptions.xml` file (§10.2 reading rules).
///
/// # Errors
///
/// [`OptionsError`] for oversized, non-UTF-8 or malformed files.
pub fn parse_options(bytes: &[u8]) -> Result<PluginOptions, OptionsError> {
    if bytes.len() > MAX_OPTIONS_BYTES {
        return Err(OptionsError::TooLarge(bytes.len()));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| OptionsError::NotUtf8)?;
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_OPTIONS_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|e| OptionsError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "PluginOptions" {
        return Err(OptionsError::WrongRoot(root.tag_name().name().to_owned()));
    }
    let mut out = PluginOptions::default();
    // First wins; names seen so far, so the work stays linear in the entry count.
    let mut seen_autoload: HashSet<&str> = HashSet::new();
    let mut seen_scaling: HashSet<&str> = HashSet::new();
    for auto in elem(root, "AutoloadOptions") {
        for p in elem(auto, "Plugin") {
            let Some(name) = p.attribute("Name") else {
                continue;
            };
            if !seen_autoload.insert(name) {
                continue;
            }
            out.autoload.push(AutoloadEntry {
                name: name.to_owned(),
                all_characters: p.attribute("AllCharacters") == Some("true"),
                characters: elem(p, "Character")
                    .filter_map(|c| c.attribute("Name"))
                    .map(str::to_owned)
                    .collect(),
            });
        }
    }
    for scaling in elem(root, "ScalingOptions") {
        for p in elem(scaling, "Plugin") {
            let Some(name) = p.attribute("Name") else {
                continue;
            };
            if !seen_scaling.insert(name) {
                continue;
            }
            out.scaling
                .push((name.to_owned(), p.attribute("UseScaling") == Some("True")));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariants() {
        let empty = parse_options(
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\r\n<PluginOptions>\r\n<AutoloadOptions/>\r\n</PluginOptions>\r\n",
        )
        .unwrap();
        assert!(empty.autoload.is_empty());
        let o = parse_options(
            br#"<PluginOptions><AutoloadOptions>
<Plugin Name="A" AllCharacters="True"><Character Name="Bob"/></Plugin>
<Plugin Name="A" AllCharacters="true"/>
<Plugin Name="B" AllCharacters="true"/>
</AutoloadOptions><ScalingOptions><Plugin Name="A" UseScaling="True"/><Plugin Name="B" UseScaling="true"/></ScalingOptions></PluginOptions>"#,
        )
        .unwrap();
        assert_eq!(o.autoload.len(), 2);
        assert!(!o.autoload[0].all_characters); // `True` is false, the first `A` is kept
        assert_eq!(o.autoload_for("Bob").collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(o.autoload_for("Eve").collect::<Vec<_>>(), ["B"]);
        assert_eq!(o.uses_scaling("A"), Some(true));
        assert_eq!(o.uses_scaling("B"), Some(false));
        assert!(parse_options(b"<Other/>").is_err());
        assert!(parse_options(b"\xff").is_err());
    }

    #[test]
    fn many_entries_parse_in_linear_time() {
        // Review finding: the duplicate checks were a linear scan per entry (quadratic).
        let mut xml = String::from("<PluginOptions><AutoloadOptions>");
        for i in 0..60_000 {
            xml.push_str(&format!("<Plugin Name=\"p{i}\"/>"));
        }
        xml.push_str("<Plugin Name=\"p7\" AllCharacters=\"true\"/>");
        xml.push_str("</AutoloadOptions><ScalingOptions>");
        for i in 0..60_000 {
            xml.push_str(&format!("<Plugin Name=\"p{i}\" UseScaling=\"True\"/>"));
        }
        xml.push_str("</ScalingOptions></PluginOptions>");
        assert!(xml.len() < MAX_OPTIONS_BYTES);
        let start = std::time::Instant::now();
        let o = parse_options(xml.as_bytes()).unwrap();
        assert_eq!(o.autoload.len(), 60_000);
        assert!(!o.autoload[7].all_characters, "the first p7 is kept");
        assert_eq!(o.scaling.len(), 60_000);
        // Quadratic checks took minutes at this size; linear parsing takes well under
        // a second even in a debug build.
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
    }
}
