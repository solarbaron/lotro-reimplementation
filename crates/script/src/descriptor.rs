// SPDX-License-Identifier: GPL-3.0-or-later
//! `.plugin` descriptor parsing.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §5 (elements, parsing rules, invariants).

use roxmltree::{Document, Node, ParsingOptions};

/// Largest descriptor file accepted (§5.2 rule 1).
pub const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;

/// Upper bound on XML nodes in one descriptor; real files have a few dozen.
const MAX_XML_NODES: u32 = 4096;

/// A validated plugin descriptor (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDescriptor {
    /// `/Plugin/Information/Name`: key for loading and for the `Plugins` table.
    pub name: String,
    /// `/Plugin/Information/Author` (empty when absent).
    pub author: String,
    /// `/Plugin/Information/Version`, free-form (empty when absent).
    pub version: String,
    /// `/Plugin/Information/Description`, internal newlines kept.
    pub description: Option<String>,
    /// `/Plugin/Information/Image`, a path relative to the plugins folder.
    pub image: Option<String>,
    /// `/Plugin/Package`: dotted package path of the main file.
    pub package: String,
    /// `/Plugin/Configuration/@Apartment`, if a `Configuration` element exists.
    pub apartment: Option<String>,
}

impl PluginDescriptor {
    /// Package path split into its components (validated non-empty at parse time).
    pub fn package_components(&self) -> Vec<&str> {
        self.package.split('.').collect()
    }
}

/// Why a descriptor was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    /// File is larger than [`MAX_DESCRIPTOR_BYTES`].
    #[error("descriptor is {0} bytes, limit is {MAX_DESCRIPTOR_BYTES}")]
    TooLarge(usize),
    /// Bytes are not valid UTF-8 (§5.2 rule 5).
    #[error("descriptor is not valid UTF-8")]
    NotUtf8,
    /// XML syntax error, DTD, or node limit exceeded.
    #[error("malformed XML: {0}")]
    Xml(String),
    /// The document element is not `Plugin`.
    #[error("root element is <{0}>, expected <Plugin>")]
    WrongRoot(String),
    /// A required element is absent or empty after trimming.
    #[error("required element {0} is missing or empty")]
    Missing(&'static str),
    /// A value breaks an invariant of §5.3.
    #[error("invalid {field}: {reason}")]
    Invalid {
        /// Element or attribute name.
        field: &'static str,
        /// Human-readable reason.
        reason: &'static str,
    },
}

/// Parse and validate a `.plugin` descriptor from raw file bytes.
///
/// Never panics; all input is treated as hostile.
pub fn parse_descriptor(bytes: &[u8]) -> Result<PluginDescriptor, DescriptorError> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DescriptorError::TooLarge(bytes.len()));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| DescriptorError::NotUtf8)?;
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_XML_NODES,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(text, options)
        .map_err(|e| DescriptorError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "Plugin" {
        return Err(DescriptorError::WrongRoot(
            root.tag_name().name().to_owned(),
        ));
    }

    let info = child(root, "Information").ok_or(DescriptorError::Missing("Information"))?;
    let name = child_text(info, "Name")
        .filter(|s| !s.is_empty())
        .ok_or(DescriptorError::Missing("Name"))?;
    if name.chars().any(char::is_control) {
        return Err(DescriptorError::Invalid {
            field: "Name",
            reason: "contains control characters",
        });
    }
    let package = child_text(root, "Package")
        .filter(|s| !s.is_empty())
        .ok_or(DescriptorError::Missing("Package"))?;
    validate_package(&package)?;

    let apartment = match child(root, "Configuration") {
        None => None,
        Some(cfg) => {
            let ap = cfg.attribute("Apartment").map(str::trim).unwrap_or("");
            if ap.is_empty() {
                return Err(DescriptorError::Missing("Configuration/@Apartment"));
            }
            if ap.chars().any(char::is_control) {
                return Err(DescriptorError::Invalid {
                    field: "Apartment",
                    reason: "contains control characters",
                });
            }
            Some(ap.to_owned())
        }
    };

    Ok(PluginDescriptor {
        name,
        author: child_text(info, "Author").unwrap_or_default(),
        version: child_text(info, "Version").unwrap_or_default(),
        description: child_text(info, "Description").filter(|s| !s.is_empty()),
        image: child_text(info, "Image").filter(|s| !s.is_empty()),
        package,
        apartment,
    })
}

/// Check the `Package` invariant of §5.3.
pub fn validate_package(package: &str) -> Result<(), DescriptorError> {
    for comp in package.split('.') {
        if comp.is_empty() {
            return Err(DescriptorError::Invalid {
                field: "Package",
                reason: "empty component",
            });
        }
        if comp.contains(['/', '\\', ':', '\0']) {
            return Err(DescriptorError::Invalid {
                field: "Package",
                reason: "component contains '/', '\\', ':' or NUL",
            });
        }
    }
    Ok(())
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// Concatenated text (including CDATA) directly inside the first child `name`, trimmed.
fn child_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    let el = child(node, name)?;
    let mut out = String::new();
    for c in el.children().filter(Node::is_text) {
        if let Some(t) = c.text() {
            out.push_str(t);
        }
    }
    Some(out.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "<?xml version=\"1.0\"?>\n<Plugin>\n\t<Information>\n\t\t<Name>Hello World</Name>\n\t\t<Author>Someone</Author>\n\t\t<Version>1.0</Version>\n\t\t<Description>Line one\nLine two</Description>\n\t</Information>\n\t<!-- <Configuration Apartment=\"x\"/> -->\n\t<Package>Someone.Hello.Main</Package>\n\t<Unknown>ignored</Unknown>\n</Plugin>\n";

    #[test]
    fn parses_basic() {
        let d = parse_descriptor(GOOD.as_bytes()).unwrap();
        assert_eq!(d.name, "Hello World");
        assert_eq!(d.author, "Someone");
        assert_eq!(d.version, "1.0");
        assert_eq!(d.description.as_deref(), Some("Line one\nLine two"));
        assert_eq!(d.package, "Someone.Hello.Main");
        assert_eq!(d.apartment, None);
        assert_eq!(d.package_components(), ["Someone", "Hello", "Main"]);
    }

    #[test]
    fn bom_and_apartment() {
        let src = "\u{FEFF}<Plugin><Information><Name> X </Name></Information><Package>A.B</Package><Configuration Apartment=\" Mine \"/></Plugin>";
        let d = parse_descriptor(src.as_bytes()).unwrap();
        assert_eq!(d.name, "X");
        assert_eq!(d.apartment.as_deref(), Some("Mine"));
        assert_eq!(d.author, "");
    }

    #[test]
    fn rejects_bad() {
        let cases: &[&[u8]] = &[
            b"",
            b"<NotPlugin/>",
            b"<Plugin><Information><Name></Name></Information><Package>A</Package></Plugin>",
            b"<Plugin><Information><Name>N</Name></Information></Plugin>",
            b"<Plugin><Information><Name>N</Name></Information><Package>A..B</Package></Plugin>",
            b"<Plugin><Information><Name>N</Name></Information><Package>A/B</Package></Plugin>",
            b"<Plugin><Information><Name>N</Name></Information><Package>A</Package><Configuration/></Plugin>",
            b"<!DOCTYPE x [<!ENTITY a \"b\">]><Plugin/>",
            b"\xff\xfe<Plugin/>",
        ];
        for c in cases {
            assert!(
                parse_descriptor(c).is_err(),
                "accepted {:?}",
                String::from_utf8_lossy(c)
            );
        }
        let big = vec![b' '; MAX_DESCRIPTOR_BYTES + 1];
        assert_eq!(
            parse_descriptor(&big),
            Err(DescriptorError::TooLarge(MAX_DESCRIPTOR_BYTES + 1))
        );
    }
}
