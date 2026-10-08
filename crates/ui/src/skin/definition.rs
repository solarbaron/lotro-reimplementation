// SPDX-License-Identifier: GPL-3.0-or-later
//! Parsing `SkinDefinition.xml` (`skin-definition.md` §4, D4, D5, D8, D9).
//!
//! Two passes: the strict XML reader of [`super::xml`] collects the three recognised kinds
//! of root children (`SkinName`, `Mapping`, `PanelFile` with its `Element` tree) while
//! checking the whole document; only a well-formed document reaches the second pass, which
//! applies the rules of §4.3–§4.8 in document order and resolves names (§5).

use std::collections::{HashMap, HashSet};

use super::tables::NameResolver;
use super::xml::{self, Attribute, Event, XmlError};

/// Largest definition file accepted, in bytes (D4).
pub const MAX_DEFINITION_BYTES: usize = 4 * 1024 * 1024;
/// Most `Mapping` elements in one definition (D4).
pub const MAX_MAPPINGS: usize = 65_536;
/// Most `Element` overrides in one definition (D4).
pub const MAX_ELEMENT_OVERRIDES: usize = 65_536;

/// Why a definition file is not a skin (§4.9; each is listed with D9).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DefinitionError {
    /// Larger than [`MAX_DEFINITION_BYTES`] (D4).
    #[error("definition file of {len} bytes exceeds the limit of {MAX_DEFINITION_BYTES} bytes")]
    TooLarge {
        /// File length.
        len: u64,
    },
    /// Not readable.
    #[error("cannot read the definition file: {0}")]
    Io(String),
    /// Not well-formed, an unsupported encoding, or nested too deeply (§4.1, D4, D5).
    #[error("not well-formed XML: {0}")]
    Xml(#[from] XmlError),
    /// More than [`MAX_MAPPINGS`] `Mapping` elements (D4).
    #[error("more than {MAX_MAPPINGS} Mapping elements")]
    TooManyMappings,
    /// More than [`MAX_ELEMENT_OVERRIDES`] `Element` overrides (D4).
    #[error("more than {MAX_ELEMENT_OVERRIDES} Element overrides")]
    TooManyElements,
    /// No stored mapping and no registered panel override (§4.8).
    #[error("nothing usable: no mapping and no panel override was kept")]
    NothingUsable,
}

impl DefinitionError {
    /// The 1-based line the error refers to, when there is one.
    #[must_use]
    pub fn line(&self) -> Option<u32> {
        match self {
            Self::Xml(e) => Some(e.line),
            _ => None,
        }
    }
}

/// A rejected entry inside an accepted definition (§4.4–§4.6, §4.9, D9). Entries that the
/// original skips silently (empty attributes, inert or repeated `Element`s) are not
/// reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// 1-based line of the element concerned.
    pub line: u32,
    /// What was rejected.
    pub kind: DiagnosticKind,
}

/// What a [`Diagnostic`] reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// A `Mapping` whose `ArtAssetID` does not resolve (§4.4 step 2).
    #[error("unknown art asset {0:?}; mapping ignored")]
    UnknownArtAsset(String),
    /// A `Mapping` of an art-asset value that an earlier mapping already holds (§4.4 step 3).
    #[error("art asset {name:?} already mapped on line {first_line}; mapping ignored")]
    RepeatedMapping {
        /// The name as written.
        name: String,
        /// Line of the kept mapping.
        first_line: u32,
    },
    /// An `Element` whose `ID` does not resolve (§4.6 step 3); the rest of its level is
    /// not read.
    #[error("unknown element {0:?}; it and its later siblings are ignored")]
    UnknownElement(String),
    /// A `PanelFile` dropped because a top-level `Element` did not resolve (§4.5 step 2).
    #[error("panel {0:?} ignored: a top-level element did not resolve")]
    PanelDropped(String),
    /// A `PanelFile` rejected because a top-level element ID is already registered by an
    /// earlier `PanelFile` (§4.5 step 3, D8).
    #[error(
        "panel {panel:?} ignored: top-level element {element:?} is already used by panel {first_panel:?}"
    )]
    RepeatedTopLevel {
        /// This panel's `ID`.
        panel: String,
        /// The repeated element name.
        element: String,
        /// The panel that registered it first.
        first_panel: String,
    },
}

/// One stored mapping (§4.4): art-asset value → `FileName` as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// Art-asset value (the resolved enum key).
    pub art_asset: u32,
    /// The art-asset name as written.
    pub name: String,
    /// `FileName` exactly as written (Unicode, D11).
    pub file_name: String,
    /// 1-based line of the `Mapping`.
    pub line: u32,
}

/// One element override (§4.6, §8.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElementOverride {
    /// Element ID value (never 0 in a kept override).
    pub element_id: u32,
    /// The `ID` as written.
    pub name: String,
    /// `X`.
    pub x: i32,
    /// `Y`.
    pub y: i32,
    /// `Width`.
    pub width: i32,
    /// `Height`.
    pub height: i32,
    /// `ScaleOriginX`, `ScaleOriginY` when either attribute was non-empty (a missing one
    /// is 0).
    pub scale_origin: Option<(i32, i32)>,
    /// `Detach` ≠ 0; parsed but never used (§8.5).
    pub detach: bool,
    /// 1-based line of the `Element`.
    pub line: u32,
    children: Vec<ElementOverride>,
    child_index: HashMap<u32, usize>,
}

impl ElementOverride {
    /// The kept child overrides, in document order (element IDs unique).
    #[must_use]
    pub fn children(&self) -> &[ElementOverride] {
        &self.children
    }

    /// The child override for `element_id`.
    #[must_use]
    pub fn child(&self, element_id: u32) -> Option<&ElementOverride> {
        self.child_index
            .get(&element_id)
            .map(|&i| &self.children[i])
    }

    /// Adds a child unless one with the same ID is present (§4.6 step 6: first wins).
    fn push_child(&mut self, child: ElementOverride) -> bool {
        push_unique(&mut self.children, &mut self.child_index, child)
    }
}

fn push_unique(
    list: &mut Vec<ElementOverride>,
    index: &mut HashMap<u32, usize>,
    item: ElementOverride,
) -> bool {
    if index.contains_key(&item.element_id) {
        return false;
    }
    index.insert(item.element_id, list.len());
    list.push(item);
    true
}

/// One registered `PanelFile` (§4.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PanelOverride {
    /// `ID` as written (used only in diagnostics).
    pub id: String,
    /// 1-based line of the `PanelFile`.
    pub line: u32,
    elements: Vec<ElementOverride>,
    index: HashMap<u32, usize>,
}

impl PanelOverride {
    /// The top-level element overrides, in document order.
    #[must_use]
    pub fn elements(&self) -> &[ElementOverride] {
        &self.elements
    }

    /// The top-level override for `element_id`.
    #[must_use]
    pub fn element(&self, element_id: u32) -> Option<&ElementOverride> {
        self.index.get(&element_id).map(|&i| &self.elements[i])
    }
}

/// A parsed definition that passed §4.8.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Definition {
    /// The skin name (§4.3): the last non-empty `SkinName` `Name`, else empty.
    pub name: String,
    mappings: Vec<Mapping>,
    mapping_index: HashMap<u32, usize>,
    panels: Vec<PanelOverride>,
    /// Top-level element ID value → panel index.
    top_level: HashMap<u32, usize>,
    used_ids: HashSet<u32>,
    /// Rejected entries, in document order (D9).
    pub diagnostics: Vec<Diagnostic>,
}

impl Definition {
    /// The stored mappings in document order (art-asset values unique).
    #[must_use]
    pub fn mappings(&self) -> &[Mapping] {
        &self.mappings
    }

    /// The mapping of an art-asset value.
    #[must_use]
    pub fn mapping(&self, art_asset: u32) -> Option<&Mapping> {
        self.mapping_index
            .get(&art_asset)
            .map(|&i| &self.mappings[i])
    }

    /// The registered panel overrides in document order.
    #[must_use]
    pub fn panels(&self) -> &[PanelOverride] {
        &self.panels
    }

    /// The top-level override for `element_id`, from whichever panel registered it
    /// (§8.3 step 2).
    #[must_use]
    pub fn top_level(&self, element_id: u32) -> Option<&ElementOverride> {
        let panel = self.panels.get(*self.top_level.get(&element_id)?)?;
        panel.element(element_id)
    }

    /// Whether `element_id` is used anywhere in the panel overrides (§8.1, §8.3 step 1).
    #[must_use]
    pub fn uses_element(&self, element_id: u32) -> bool {
        self.used_ids.contains(&element_id)
    }

    /// Whether the skin has any panel override.
    #[must_use]
    pub fn has_panel_overrides(&self) -> bool {
        !self.panels.is_empty()
    }

    /// The override for the element `element_id` built inside the ID path `path` (outermost
    /// first; empty for the root of a build), §8.3.
    #[must_use]
    pub fn find_override(&self, path: &[u32], element_id: u32) -> Option<&ElementOverride> {
        if self.panels.is_empty() || !self.uses_element(element_id) {
            return None;
        }
        let Some((&first, rest)) = path.split_first() else {
            return self.top_level(element_id);
        };
        // Top-level IDs are unique within a skin, so only the panel holding `first` can
        // match (§8.3 step 3).
        let mut node = self.top_level(first)?;
        for id in rest {
            node = node.child(*id)?;
        }
        node.child(element_id)
    }
}

/// Parses a definition file's bytes (§4) and resolves its names (§5).
///
/// # Errors
///
/// A [`DefinitionError`] when the file is not a skin; rejected entries of an accepted
/// skin are in [`Definition::diagnostics`].
pub fn parse_definition(
    bytes: &[u8],
    resolver: &dyn NameResolver,
) -> Result<Definition, DefinitionError> {
    if bytes.len() > MAX_DEFINITION_BYTES {
        return Err(DefinitionError::TooLarge {
            len: bytes.len() as u64,
        });
    }
    let text = xml::decode_document(bytes)?;
    let raw = collect(&text)?;
    let def = interpret(raw, resolver);
    if def.mappings.is_empty() && def.panels.is_empty() {
        return Err(DefinitionError::NothingUsable);
    }
    Ok(def)
}

/// Parses an integer attribute as §4.7 describes ("string to long" with automatic base,
/// saturating to the signed 32-bit range).
#[must_use]
pub fn parse_int(text: &str) -> i32 {
    let s = text.trim_start_matches([' ', '\t', '\n', '\r', '\u{0B}', '\u{0C}']);
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let b = s.as_bytes();
    let (radix, digits) = if b.len() >= 3
        && b[0] == b'0'
        && matches!(b[1], b'x' | b'X')
        && b[2].is_ascii_hexdigit()
    {
        (16, &s[2..])
    } else if b.first() == Some(&b'0') {
        (8, s)
    } else {
        (10, s)
    };
    let mut value: i64 = 0;
    for c in digits.chars() {
        let Some(d) = c.to_digit(radix) else { break };
        value = value
            .saturating_mul(i64::from(radix))
            .saturating_add(i64::from(d));
        if value > i64::from(i32::MAX) + 1 {
            value = i64::from(i32::MAX) + 1;
        }
    }
    let signed = if negative { -value } else { value };
    i32::try_from(signed.clamp(i64::from(i32::MIN), i64::from(i32::MAX))).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------
// Pass 1: collect the recognised parts of a well-formed document.

#[derive(Debug, Default)]
struct RawMapping {
    art: String,
    file: String,
    line: u32,
}

#[derive(Debug, Default)]
struct RawElement {
    id: String,
    x: String,
    y: String,
    width: String,
    height: String,
    origin_x: String,
    origin_y: String,
    detach: String,
    line: u32,
    children: Vec<RawElement>,
}

#[derive(Debug, Default)]
struct RawPanel {
    id: String,
    line: u32,
    elements: Vec<RawElement>,
}

#[derive(Debug, Default)]
struct Raw {
    name: String,
    mappings: Vec<RawMapping>,
    panels: Vec<RawPanel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frame {
    Root,
    Ignored,
    Panel,
    Element,
}

fn raw_element(attributes: &[Attribute], line: u32) -> RawElement {
    let get = |n: &str| Event::attribute(attributes, n).to_owned();
    RawElement {
        id: get("ID"),
        x: get("X"),
        y: get("Y"),
        width: get("Width"),
        height: get("Height"),
        origin_x: get("ScaleOriginX"),
        origin_y: get("ScaleOriginY"),
        detach: get("Detach"),
        line,
        children: Vec::new(),
    }
}

fn collect(text: &str) -> Result<Raw, DefinitionError> {
    let mut reader = xml::Reader::new(text);
    let mut raw = Raw::default();
    let mut frames: Vec<Frame> = Vec::new();
    let mut open_elements: Vec<RawElement> = Vec::new();
    let mut element_count = 0usize;
    while let Some(event) = reader.next_event()? {
        match event {
            Event::Start {
                name,
                attributes,
                line,
            } => {
                let frame = match frames.last() {
                    None => Frame::Root,
                    Some(Frame::Root) => {
                        if name.eq_ignore_ascii_case("SkinName") {
                            let value = Event::attribute(&attributes, "Name");
                            if !value.is_empty() {
                                value.clone_into(&mut raw.name);
                            }
                            Frame::Ignored
                        } else if name.eq_ignore_ascii_case("Mapping") {
                            if raw.mappings.len() >= MAX_MAPPINGS {
                                return Err(DefinitionError::TooManyMappings);
                            }
                            raw.mappings.push(RawMapping {
                                art: Event::attribute(&attributes, "ArtAssetID").to_owned(),
                                file: Event::attribute(&attributes, "FileName").to_owned(),
                                line,
                            });
                            Frame::Ignored
                        } else if name.eq_ignore_ascii_case("PanelFile") {
                            raw.panels.push(RawPanel {
                                id: Event::attribute(&attributes, "ID").to_owned(),
                                line,
                                elements: Vec::new(),
                            });
                            Frame::Panel
                        } else {
                            Frame::Ignored
                        }
                    }
                    Some(Frame::Panel | Frame::Element) if name.eq_ignore_ascii_case("Element") => {
                        element_count += 1;
                        if element_count > MAX_ELEMENT_OVERRIDES {
                            return Err(DefinitionError::TooManyElements);
                        }
                        open_elements.push(raw_element(&attributes, line));
                        Frame::Element
                    }
                    Some(_) => Frame::Ignored,
                };
                frames.push(frame);
            }
            Event::End => {
                if frames.pop() == Some(Frame::Element)
                    && let Some(done) = open_elements.pop()
                {
                    match open_elements.last_mut() {
                        Some(parent) => parent.children.push(done),
                        None => {
                            if let Some(panel) = raw.panels.last_mut() {
                                panel.elements.push(done);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(raw)
}

// ---------------------------------------------------------------------------------------
// Pass 2: the rules of §4.3–§4.6.

fn interpret(raw: Raw, resolver: &dyn NameResolver) -> Definition {
    let mut def = Definition {
        name: raw.name,
        ..Definition::default()
    };
    for m in raw.mappings {
        if m.art.is_empty() || m.file.is_empty() {
            continue;
        }
        let Some(value) = resolver.art_asset(&m.art) else {
            def.diagnostics.push(Diagnostic {
                line: m.line,
                kind: DiagnosticKind::UnknownArtAsset(m.art),
            });
            continue;
        };
        if let Some(first) = def.mapping(value) {
            let first_line = first.line;
            def.diagnostics.push(Diagnostic {
                line: m.line,
                kind: DiagnosticKind::RepeatedMapping {
                    name: m.art,
                    first_line,
                },
            });
            continue;
        }
        def.mapping_index.insert(value, def.mappings.len());
        def.mappings.push(Mapping {
            art_asset: value,
            name: m.art,
            file_name: m.file,
            line: m.line,
        });
    }

    for p in raw.panels {
        if p.id.is_empty() {
            continue;
        }
        let mut panel = PanelOverride {
            id: p.id,
            line: p.line,
            ..PanelOverride::default()
        };
        let mut diagnostics = Vec::new();
        let failed = parse_level(&p.elements, resolver, &mut diagnostics, &mut |ov| {
            push_unique(&mut panel.elements, &mut panel.index, ov);
        });
        def.diagnostics.append(&mut diagnostics);
        if failed {
            def.diagnostics.push(Diagnostic {
                line: panel.line,
                kind: DiagnosticKind::PanelDropped(panel.id),
            });
            continue;
        }
        // D8: a repeated top-level ID rejects the whole later panel.
        if let Some(dup) = panel
            .elements
            .iter()
            .find(|e| def.top_level.contains_key(&e.element_id))
        {
            let first_panel = def.top_level[&dup.element_id];
            def.diagnostics.push(Diagnostic {
                line: dup.line,
                kind: DiagnosticKind::RepeatedTopLevel {
                    panel: panel.id.clone(),
                    element: dup.name.clone(),
                    first_panel: def.panels[first_panel].id.clone(),
                },
            });
            continue;
        }
        let index = def.panels.len();
        for e in &panel.elements {
            def.top_level.insert(e.element_id, index);
            collect_ids(e, &mut def.used_ids);
        }
        def.panels.push(panel);
    }
    def
}

fn collect_ids(e: &ElementOverride, out: &mut HashSet<u32>) {
    out.insert(e.element_id);
    for c in &e.children {
        collect_ids(c, out);
    }
}

/// Parses one level of `Element`s (§4.6), handing each kept override to `keep` (which
/// applies step 6). Returns `true` when the level stopped at an unresolvable `ID`
/// (step 3). Recursion depth is bounded by the XML depth limit (D4).
fn parse_level(
    raws: &[RawElement],
    resolver: &dyn NameResolver,
    diagnostics: &mut Vec<Diagnostic>,
    keep: &mut dyn FnMut(ElementOverride),
) -> bool {
    for r in raws {
        let mut ov = ElementOverride {
            name: r.id.clone(),
            line: r.line,
            ..ElementOverride::default()
        };
        if !r.id.is_empty() {
            match resolver.element(&r.id) {
                Some(v) => ov.element_id = v,
                None => {
                    diagnostics.push(Diagnostic {
                        line: r.line,
                        kind: DiagnosticKind::UnknownElement(r.id.clone()),
                    });
                    return true;
                }
            }
        }
        let int = |s: &str| if s.is_empty() { 0 } else { parse_int(s) };
        ov.x = int(&r.x);
        ov.y = int(&r.y);
        ov.width = int(&r.width);
        ov.height = int(&r.height);
        if !r.origin_x.is_empty() || !r.origin_y.is_empty() {
            ov.scale_origin = Some((int(&r.origin_x), int(&r.origin_y)));
        }
        ov.detach = !r.detach.is_empty() && parse_int(&r.detach) != 0;
        // Step 4: the children's failure is ignored by this level.
        let mut children = Vec::new();
        let _ = parse_level(&r.children, resolver, diagnostics, &mut |c| {
            children.push(c)
        });
        for c in children {
            ov.push_child(c);
        }
        // Step 5.
        let sized = ov.width != 0 && ov.height != 0;
        if ov.element_id != 0 && (sized || !ov.children.is_empty()) {
            keep(ov);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_table_of_section_4_7() {
        for (text, value) in [
            ("12", 12),
            ("  -4", -4),
            ("+7", 7),
            ("0x1F", 31),
            ("010", 8),
            ("09", 0),
            ("12px", 12),
            ("abc", 0),
            ("99999999999", i32::MAX),
            ("-99999999999", i32::MIN),
            ("-2147483648", i32::MIN),
            ("2147483648", i32::MAX),
            ("0X10", 16),
            ("0x", 0),
            ("0xg", 0),
            ("-0x10", -16),
            ("+", 0),
            ("- 5", 0),
            ("", 0),
            ("\t\n 5", 5),
        ] {
            assert_eq!(parse_int(text), value, "{text:?}");
        }
    }
}
