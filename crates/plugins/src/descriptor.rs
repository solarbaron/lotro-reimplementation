// SPDX-License-Identifier: GPL-3.0-or-later
//! `.plugin` descriptor parsing.
//!
//! Spec: `plugin-descriptor.md` §5 (document shape, parse rules, robust-parsing
//! requirements, invariants, worked examples E1–E24); `lua-plugin-api.md` §5.

use roxmltree::{Document, Node, ParsingOptions};

/// Largest descriptor file accepted (§5.3: "64 KiB is ample").
pub const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;

/// Upper bound on XML nodes in one descriptor; real files have a few dozen.
const MAX_XML_NODES: u32 = 4096;

/// Deepest element nesting accepted (§5.3: bound the nesting depth).
const MAX_XML_DEPTH: usize = 64;

/// Longest `Name`, `Version` and `Package` (UTF-16 units), and the cut length of `Author`
/// and `Image` (§5.2 rule 4).
pub const MAX_FIELD_UNITS: usize = 120;

/// Cut length of `Description` (UTF-16 units, §5.2 rule 4).
pub const MAX_DESCRIPTION_UNITS: usize = 2048;

/// The apartment of a descriptor without `Configuration/@Apartment` (§5.2 rule 8).
pub const DEFAULT_APARTMENT: &str = "Default";

/// A validated plugin descriptor (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDescriptor {
    /// `Information/Name`: key for loading and for the `Plugins` table (1–120 units).
    pub name: String,
    /// `Information/Author` (empty when absent; cut to 120 units).
    pub author: String,
    /// `Information/Version`, free-form (empty when absent; at most 120 units).
    pub version: String,
    /// `Information/Description`, internal newlines kept (cut to 2048 units).
    pub description: Option<String>,
    /// `Information/Image`, a path relative to the plugins folder (cut to 120 units).
    pub image: Option<String>,
    /// `Package`: dotted package path of the main file (1–120 units). The parse checks
    /// only its length; a name that fails the package grammar cannot load (§7.1).
    pub package: String,
    /// `Configuration/@Apartment` exactly as written (not trimmed, possibly empty), when
    /// present; `None` means [`DEFAULT_APARTMENT`].
    pub apartment: Option<String>,
}

impl PluginDescriptor {
    /// Package path split into its components.
    #[must_use]
    pub fn package_components(&self) -> Vec<&str> {
        self.package.split('.').collect()
    }

    /// The apartment this plugin loads into.
    #[must_use]
    pub fn apartment_name(&self) -> &str {
        self.apartment.as_deref().unwrap_or(DEFAULT_APARTMENT)
    }
}

/// Why a descriptor was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    /// File is larger than [`MAX_DESCRIPTOR_BYTES`].
    #[error("descriptor is {0} bytes, limit is {MAX_DESCRIPTOR_BYTES}")]
    TooLarge(usize),
    /// Bytes are not legal in the detected encoding (§5.2 rule 1).
    #[error("descriptor bytes are not valid {0}")]
    BadEncoding(&'static str),
    /// The declaration names an encoding Tessera does not decode (§5.3).
    #[error("unsupported encoding {0:?}")]
    UnsupportedEncoding(String),
    /// XML syntax error, DTD, node or depth limit exceeded.
    #[error("malformed XML: {0}")]
    Xml(String),
    /// The document element is not `Plugin`.
    #[error("root element is <{0}>, expected <Plugin>")]
    WrongRoot(String),
    /// A required element is absent or empty after trimming.
    #[error("required element {0} is missing or empty")]
    Missing(&'static str),
    /// An element occurs more often than allowed (§5.2 rules 3, 4.1, 6, 7).
    #[error("element {0} occurs more often than allowed")]
    Repeated(&'static str),
    /// A value breaks a rule of §5.2.
    #[error("invalid {field}: {reason}")]
    Invalid {
        /// Element or attribute name.
        field: &'static str,
        /// Human-readable reason.
        reason: &'static str,
    },
}

/// Parse and validate a `.plugin` descriptor from raw file bytes (§5.2, §5.3).
///
/// Never panics; all input is treated as hostile.
///
/// # Errors
///
/// [`DescriptorError`] naming the first rule that failed.
pub fn parse_descriptor(bytes: &[u8]) -> Result<PluginDescriptor, DescriptorError> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DescriptorError::TooLarge(bytes.len()));
    }
    let text = decode(bytes)?;
    let text = without_namespaces(&text);
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_XML_NODES,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(&text, options)
        .map_err(|e| DescriptorError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root
        .descendants()
        .any(|n| n.ancestors().count() > MAX_XML_DEPTH)
    {
        return Err(DescriptorError::Xml(
            "elements nested too deeply".to_owned(),
        ));
    }
    if !named(root, "Plugin") {
        return Err(DescriptorError::WrongRoot(
            root.tag_name().name().to_owned(),
        ));
    }

    // Rule 3: exactly one `Information` anywhere below the root.
    let mut infos = elements_below(root, "Information");
    let info = infos
        .next()
        .ok_or(DescriptorError::Missing("Information"))?;
    if infos.next().is_some() {
        return Err(DescriptorError::Repeated("Information"));
    }

    // Rule 4: the fields, direct children of `Information`.
    let mut name = None;
    let mut version = None;
    let mut author = None;
    let mut description = None;
    let mut image = None;
    for child in info.children().filter(Node::is_element) {
        let (slot, field, rule): (&mut Option<String>, &'static str, Rule) = match child_name(child)
        {
            Some("Name") => (&mut name, "Name", Rule::Reject(MAX_FIELD_UNITS)),
            Some("Version") => (&mut version, "Version", Rule::Reject(MAX_FIELD_UNITS)),
            Some("Author") => (&mut author, "Author", Rule::Cut(MAX_FIELD_UNITS)),
            Some("Description") => (
                &mut description,
                "Description",
                Rule::Cut(MAX_DESCRIPTION_UNITS),
            ),
            Some("Image") => (&mut image, "Image", Rule::Cut(MAX_FIELD_UNITS)),
            _ => continue,
        };
        if slot.is_some() {
            return Err(DescriptorError::Repeated(field));
        }
        let value = trim(&text_content(child)).to_owned();
        let value = match rule {
            Rule::Reject(max) if utf16_len(&value) > max => {
                return Err(DescriptorError::Invalid {
                    field,
                    reason: "longer than 120 characters",
                });
            }
            Rule::Reject(_) => value,
            Rule::Cut(max) => cut_utf16(&value, max),
        };
        if value.is_empty() {
            return Err(DescriptorError::Missing(field));
        }
        *slot = Some(value);
    }
    // Rule 5.
    let name = name.ok_or(DescriptorError::Missing("Name"))?;

    // Rule 6: exactly one `Package` anywhere.
    let mut packages = elements_below(root, "Package");
    let package_el = packages.next().ok_or(DescriptorError::Missing("Package"))?;
    if packages.next().is_some() {
        return Err(DescriptorError::Repeated("Package"));
    }
    let package = trim(&text_content(package_el)).to_owned();
    if package.is_empty() {
        return Err(DescriptorError::Missing("Package"));
    }
    if utf16_len(&package) > MAX_FIELD_UNITS {
        return Err(DescriptorError::Invalid {
            field: "Package",
            reason: "longer than 120 characters",
        });
    }

    // Rules 7 and 8.
    let mut configs = elements_below(root, "Configuration");
    let apartment = match configs.next() {
        None => None,
        Some(cfg) => {
            if configs.next().is_some() {
                return Err(DescriptorError::Repeated("Configuration"));
            }
            cfg.attributes()
                .find(|a| a.namespace().is_none() && a.name() == "Apartment")
                .map(|a| a.value().to_owned())
        }
    };
    if apartment.as_deref() == Some("") {
        tracing::warn!(plugin = %name, "descriptor names the empty apartment");
    }

    Ok(PluginDescriptor {
        name,
        author: author.unwrap_or_default(),
        version: version.unwrap_or_default(),
        description,
        image,
        package,
        apartment,
    })
}

#[derive(Debug, Clone, Copy)]
enum Rule {
    Reject(usize),
    Cut(usize),
}

/// The element's name. Names are compared as written (§3.2: `x:Name` does not match
/// `Name`): [`without_namespaces`] turned every prefixed name into one with `_` in place of
/// the colon, which matches none of the names the parser looks for.
fn child_name<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    Some(node.tag_name().name())
}

/// The descriptor text with namespace processing taken out of the parser's hands: in every
/// element and attribute name, a `:` becomes `_`. So a qualified name is compared as
/// written, a prefix that is never declared is not an error (an element of an unknown
/// prefix inside `Information` is simply not one of the examined fields, §5.2), and
/// `xmlns:x` declarations are plain attributes. Text, attribute values, comments, CDATA
/// sections and processing instructions are copied unchanged. (`plugin-descriptor.md`
/// §5.2 OPEN: whether the original's parser processes namespaces.)
fn without_namespaces(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains(':') {
        return std::borrow::Cow::Borrowed(text);
    }
    let b = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    let copy_until = |out: &mut Vec<u8>, i: &mut usize, end: &[u8]| {
        let rest = &b[*i..];
        let n = rest
            .windows(end.len())
            .position(|w| w == end)
            .map_or(rest.len(), |p| p + end.len());
        out.extend_from_slice(&rest[..n]);
        *i += n;
    };
    let is_name_end = |c: u8| c.is_ascii_whitespace() || matches!(c, b'/' | b'>' | b'=');
    while i < b.len() {
        let rest = &b[i..];
        if rest.starts_with(b"<!--") {
            copy_until(&mut out, &mut i, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            copy_until(&mut out, &mut i, b"]]>");
        } else if rest.starts_with(b"<?") {
            copy_until(&mut out, &mut i, b"?>");
        } else if rest.starts_with(b"<!") {
            copy_until(&mut out, &mut i, b">");
        } else if b[i] == b'<' {
            // A start or end tag: its name, then attribute names and quoted values.
            out.push(b'<');
            i += 1;
            if b.get(i) == Some(&b'/') {
                out.push(b'/');
                i += 1;
            }
            let mut in_name = true;
            while i < b.len() && b[i] != b'>' {
                let c = b[i];
                if c == b'"' || c == b'\'' {
                    let close = b[i + 1..]
                        .iter()
                        .position(|&q| q == c)
                        .map_or(b.len(), |p| i + 1 + p + 1);
                    out.extend_from_slice(&b[i..close]);
                    i = close;
                    in_name = false;
                    continue;
                }
                if is_name_end(c) {
                    in_name = false;
                } else if !in_name {
                    in_name = true;
                }
                out.push(if in_name && c == b':' { b'_' } else { c });
                i += 1;
            }
            if i < b.len() {
                out.push(b'>');
                i += 1;
            }
        } else {
            let n = rest.iter().position(|&c| c == b'<').unwrap_or(rest.len());
            out.extend_from_slice(&rest[..n]);
            i += n;
        }
    }
    // Only ASCII bytes were replaced by ASCII bytes, so the text is still UTF-8.
    std::borrow::Cow::Owned(String::from_utf8(out).unwrap_or_else(|_| text.to_owned()))
}

fn named(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element() && child_name(node) == Some(name)
}

/// Elements named `name` among all descendants of `root` (not `root` itself).
fn elements_below<'a, 'i>(
    root: Node<'a, 'i>,
    name: &'static str,
) -> impl Iterator<Item = Node<'a, 'i>> {
    root.descendants().skip(1).filter(move |n| named(*n, name))
}

/// All descendant text and CDATA of `el`, concatenated, without comments (§5.2 rule 4.2).
fn text_content(el: Node<'_, '_>) -> String {
    let mut out = String::new();
    for n in el.descendants().filter(Node::is_text) {
        if let Some(t) = n.text() {
            out.push_str(t);
        }
    }
    out
}

/// Trim space, line feed, carriage return, tab and form feed (§5.2 rule 4.3).
fn trim(s: &str) -> &str {
    s.trim_matches([' ', '\n', '\r', '\t', '\u{0C}'])
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The longest prefix of `s` of at most `max` UTF-16 units; a character that would be cut
/// in half (a surrogate pair at the boundary) is dropped (§5.3).
fn cut_utf16(s: &str, max: usize) -> String {
    let mut units = 0;
    let mut end = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max {
            break;
        }
        end = i + c.len_utf8();
    }
    s[..end].to_owned()
}

/// Decode the file to text (§5.2 rule 1, §5.3): a byte-order mark first, else the encoding
/// declaration, else UTF-8. Bytes that are not legal in the detected encoding reject.
fn decode(bytes: &[u8]) -> Result<String, DescriptorError> {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return String::from_utf8(rest.to_vec()).map_err(|_| DescriptorError::BadEncoding("UTF-8"));
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return decode_utf16(rest, u16::from_le_bytes);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return decode_utf16(rest, u16::from_be_bytes);
    }
    let label = declared_encoding(bytes).map(|e| e.to_ascii_lowercase());
    match label.as_deref() {
        None | Some("utf-8" | "utf8") => {
            String::from_utf8(bytes.to_vec()).map_err(|_| DescriptorError::BadEncoding("UTF-8"))
        }
        Some("iso-8859-1" | "iso_8859-1" | "iso8859-1" | "latin1" | "l1" | "latin-1") => {
            Ok(bytes.iter().map(|&b| char::from(b)).collect())
        }
        Some("us-ascii" | "ascii") => {
            if bytes.is_ascii() {
                Ok(bytes.iter().map(|&b| char::from(b)).collect())
            } else {
                Err(DescriptorError::BadEncoding("US-ASCII"))
            }
        }
        Some("windows-1252" | "cp1252") => bytes
            .iter()
            .map(|&b| cp1252(b).ok_or(DescriptorError::BadEncoding("windows-1252")))
            .collect(),
        Some(other) => {
            tracing::warn!(encoding = other, "descriptor names an unsupported encoding");
            Err(DescriptorError::UnsupportedEncoding(other.to_owned()))
        }
    }
}

fn decode_utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> Result<String, DescriptorError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(DescriptorError::BadEncoding("UTF-16"));
    }
    let units = bytes.as_chunks::<2>().0.iter().map(|c| unit(*c));
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|_| DescriptorError::BadEncoding("UTF-16"))
}

/// The `encoding` pseudo-attribute of an XML declaration at the very start of `bytes`.
fn declared_encoding(bytes: &[u8]) -> Option<String> {
    let rest = bytes.strip_prefix(b"<?xml")?;
    if !rest.first().is_some_and(|b| b.is_ascii_whitespace()) {
        return None;
    }
    let end = rest.windows(2).position(|w| w == b"?>")?;
    let decl = std::str::from_utf8(&rest[..end]).ok()?;
    let at = decl.find("encoding")?;
    let after = decl[at + "encoding".len()..].trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &after[1..];
    let close = body.find(quote)?;
    Some(body[..close].to_owned())
}

/// Windows code page 1252; the five undefined bytes are illegal.
fn cp1252(b: u8) -> Option<char> {
    const HIGH: [u16; 32] = [
        0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
        0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
        0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
    ];
    match b {
        0x80..=0x9F => match HIGH[usize::from(b - 0x80)] {
            0 => None,
            u => char::from_u32(u32::from(u)),
        },
        _ => Some(char::from(b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic descriptor: `info` is the inner XML of `Information`, `extra` follows
    /// the `Package` element.
    fn doc(info: &str, extra: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<Plugin>\n<Information>{info}</Information>\n<Package>Example.Alpha.Main</Package>{extra}\n</Plugin>"
        )
    }

    fn ok(s: &str) -> PluginDescriptor {
        parse_descriptor(s.as_bytes()).unwrap_or_else(|e| panic!("{e}: {s}"))
    }

    fn rejected(s: &str) {
        assert!(parse_descriptor(s.as_bytes()).is_err(), "accepted {s}");
    }

    #[test]
    fn worked_examples() {
        let e1 = ok(&doc("<Name>Alpha</Name>", ""));
        assert_eq!(e1.name, "Alpha");
        assert_eq!((e1.version.as_str(), e1.author.as_str()), ("", ""));
        assert_eq!((e1.description.clone(), e1.image.clone()), (None, None));
        assert_eq!(e1.package, "Example.Alpha.Main");
        assert_eq!(e1.apartment_name(), "Default");
        assert_eq!(ok(&doc("<Name>\n  Alpha\t\n</Name>", "")).name, "Alpha"); // E2
        rejected(&doc("<Name>   \n</Name>", "")); // E3
        rejected(&doc("<Author>Someone</Author>", "")); // E4
        rejected(&doc("<Name>Alpha</Name><Name>Beta</Name>", "")); // E5
        assert_eq!(
            ok(&doc(&format!("<Name>{}</Name>", "a".repeat(120)), ""))
                .name
                .len(),
            120
        ); // E6
        rejected(&doc(&format!("<Name>{}</Name>", "a".repeat(121)), "")); // E7
        let e8 = ok(&doc(
            &format!("<Name>A</Name><Author>{}</Author>", "b".repeat(130)),
            "",
        ));
        assert_eq!(e8.author, "b".repeat(120));
        let e9 = ok(&doc(
            &format!(
                "<Name>A</Name><Description>{}</Description>",
                "c".repeat(3000)
            ),
            "",
        ));
        assert_eq!(e9.description.unwrap(), "c".repeat(2048));
        rejected(&doc("<Name>A</Name><Image></Image>", "")); // E10
        assert_eq!(ok(&doc("<Name>A</Name><Notes><x/>y</Notes>", "")).name, "A"); // E11
        assert_eq!(
            ok(&doc("<Name>Al<Extra>pha</Extra><!-- c --></Name>", "")).name,
            "Alpha"
        ); // E12
        rejected(&doc("<Group><Name>A</Name></Group>", "")); // E13
        rejected(&doc("<Name>A</Name><Package>X</Package>", "")); // E14
        rejected(&format!(
            "<Plugin><Information><Name>A</Name></Information><Package>{}</Package></Plugin>",
            "p".repeat(121)
        )); // E15
        rejected("<Plugin><Information><Name>A</Name></Information><Package>  </Package></Plugin>"); // E16
        assert_eq!(
            ok(&doc("<Name>A</Name>", "<Configuration/>")).apartment_name(),
            "Default"
        ); // E17
        assert_eq!(
            ok(&doc(
                "<Name>A</Name>",
                "<Configuration Apartment=\"Shared\"/>"
            ))
            .apartment_name(),
            "Shared"
        ); // E18
        assert_eq!(
            ok(&doc(
                "<Name>A</Name>",
                "<Configuration Apartment=\" Shared \"/>"
            ))
            .apartment_name(),
            " Shared "
        ); // E19
        rejected(&doc(
            "<Name>A</Name>",
            "<Configuration Apartment=\"x\"/><Configuration/>",
        )); // E20
        rejected("<plugin><Information><Name>A</Name></Information><Package>P</Package></plugin>"); // E21
        let mut e22 = b"<Plugin><Information><Name>".to_vec();
        e22.push(0xE9);
        e22.extend_from_slice(b"</Name></Information><Package>P</Package></Plugin>");
        assert!(parse_descriptor(&e22).is_err()); // E22
        let mut e23 =
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><Plugin><Information><Name>".to_vec();
        e23.push(0xE9);
        e23.extend_from_slice(b"</Name></Information><Package>P</Package></Plugin>");
        assert_eq!(parse_descriptor(&e23).unwrap().name, "\u{e9}"); // E23
        assert_eq!(
            ok("<Plugin><Wrapper><Information><Name>A</Name></Information></Wrapper><Package>P</Package></Plugin>").name,
            "A"
        ); // E24
    }

    #[test]
    fn encodings_and_bounds() {
        let utf16: Vec<u8> = std::iter::once(0xFEFF_u16)
            .chain(doc("<Name>W</Name>", "").encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(parse_descriptor(&utf16).unwrap().name, "W");
        let bom8 = format!("\u{FEFF}{}", doc("<Name>B</Name>", ""));
        assert_eq!(ok(&bom8).name, "B");
        rejected("<?xml version=\"1.0\" encoding=\"EBCDIC\"?><Plugin/>");
        rejected("<!DOCTYPE x [<!ENTITY a \"b\">]><Plugin/>");
        rejected("");
        rejected("<NotPlugin/>");
        rejected(
            "<x:Plugin xmlns:x=\"u\"><Information><Name>A</Name></Information><Package>P</Package></x:Plugin>",
        );
        let big = vec![b' '; MAX_DESCRIPTOR_BYTES + 1];
        assert_eq!(
            parse_descriptor(&big),
            Err(DescriptorError::TooLarge(MAX_DESCRIPTOR_BYTES + 1))
        );
        let deep = format!("{}{}", "<a>".repeat(100), "</a>".repeat(100));
        assert!(parse_descriptor(deep.as_bytes()).is_err());
        // A supplementary character at the cut boundary is dropped whole.
        let author = format!("{}\u{1F600}", "x".repeat(119));
        let d = ok(&doc(
            &format!("<Name>A</Name><Author>{author}</Author>"),
            "",
        ));
        assert_eq!(d.author, "x".repeat(119));
        assert_eq!(cp1252(0x80), Some('\u{20AC}'));
        assert_eq!(cp1252(0x81), None);
    }

    #[test]
    fn names_compare_as_written_without_namespace_processing() {
        // Review finding: `x:Name` bound to the default namespace matched `Name`, and an
        // undeclared prefix inside `Information` rejected the whole descriptor.
        rejected(
            "<Plugin xmlns=\"u\" xmlns:x=\"u\"><Information><x:Name>A</x:Name></Information><Package>P</Package></Plugin>",
        );
        rejected(
            "<x:Plugin xmlns=\"u\" xmlns:x=\"u\"><Information><Name>A</Name></Information><Package>P</Package></x:Plugin>",
        );
        let d = ok(
            "<Plugin><Information><Name>A</Name><x:Notes a:b='1>2'>n</x:Notes></Information>\
             <Package>P</Package><Configuration x:Apartment=\"Q\"/></Plugin>",
        );
        assert_eq!((d.name.as_str(), d.apartment), ("A", None));
        // A default namespace does not change names; text, comments and CDATA keep colons.
        let d = ok(
            "<Plugin xmlns=\"urn:x\"><Information><Name>A:B</Name><!-- c:d -->\
             <Description><![CDATA[e:f]]></Description></Information><Package>P</Package></Plugin>",
        );
        assert_eq!(d.name, "A:B");
        assert_eq!(d.description.as_deref(), Some("e:f"));
        assert_eq!(
            without_namespaces("<a:b c:d=\"e:f\" g='h:i'>j:k</a:b><?p q:r?>"),
            "<a_b c_d=\"e:f\" g='h:i'>j:k</a_b><?p q:r?>"
        );
    }
}
