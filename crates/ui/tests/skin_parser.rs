// SPDX-License-Identifier: GPL-3.0-or-later
//! `SkinDefinition.xml` parser (`skin-definition.md` §4, §5.2, §8.3; D4, D5, D8): the
//! invariants SP1–SP10 and the worked example of §12.2, on synthetic definitions.

mod skin_common;

use skin_common::resolver;
use tessera_ui::skin::{
    DefinitionError, DiagnosticKind, MAX_DEFINITION_BYTES, MAX_MAPPINGS, parse_definition,
    xml::{MAX_XML_ATTRIBUTES, XmlErrorKind},
};

const ART_A: u32 = 0x1000_005C;
const ART_B: u32 = 0x1000_0010;

fn names() -> (tessera_ui::skin::LabelIndex, tessera_ui::skin::LabelIndex) {
    resolver(
        &[
            ("basepanel_topleft", ART_A),
            ("Frame_Middle", ART_B),
            ("Invalid", 0),
        ],
        &[
            ("PanelA", 0x1000_0100),
            ("PanelA_Title", 0x1000_0101),
            ("PanelA_Body", 0x1000_0102),
            ("PanelB", 0x1000_0200),
            ("Frame_Close", 0x1000_0201),
            ("Undefined", 0),
        ],
    )
}

fn parse(xml: &str) -> Result<tessera_ui::skin::Definition, DefinitionError> {
    parse_definition(xml.as_bytes(), &names())
}

#[test]
fn sp1_any_root_name() {
    for root in ["opt", "anything", "x:y", "SKIN"] {
        let d = parse(&format!(
            "<{root}><Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"/></{root}>"
        ))
        .unwrap();
        assert_eq!(d.mappings().len(), 1, "{root}");
    }
}

#[test]
fn sp2_attribute_case_vs_element_case() {
    let e = parse("<opt><mapping artassetid=\"Frame_Middle\" filename=\"a.tga\"/></opt>");
    assert_eq!(e.unwrap_err(), DefinitionError::NothingUsable);
    let d = parse("<opt><MAPPING ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"/></opt>").unwrap();
    assert_eq!(d.mapping(ART_B).unwrap().file_name, "a.tga");
    // A namespace prefix is part of the name and prevents a match.
    let e = parse("<opt><s:Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"/></opt>");
    assert_eq!(e.unwrap_err(), DefinitionError::NothingUsable);
}

#[test]
fn sp3_first_mapping_wins() {
    let d = parse(
        "<opt>\n<Mapping ArtAssetID=\"FRAME_MIDDLE\" FileName=\"first.tga\"/>\n\
         <Mapping ArtAssetID=\"frame_middle\" FileName=\"second.tga\"/></opt>",
    )
    .unwrap();
    assert_eq!(d.mappings().len(), 1);
    assert_eq!(d.mapping(ART_B).unwrap().file_name, "first.tga");
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(d.diagnostics[0].line, 3);
    assert!(matches!(
        d.diagnostics[0].kind,
        DiagnosticKind::RepeatedMapping { first_line: 2, .. }
    ));
}

#[test]
fn sp4_last_non_empty_skin_name() {
    let d = parse(
        "<opt><SkinName Name=\"One\"/><skinname Name=\"Two\"/><SkinName Name=\"\"/>\
         <SkinName/><Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"/></opt>",
    )
    .unwrap();
    assert_eq!(d.name, "Two");
    let d = parse("<opt><Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"/></opt>").unwrap();
    assert_eq!(d.name, "");
    // Kept exactly as written (no trimming).
    let d = parse("<opt><SkinName Name=\" Spaced  Name \"/><PanelFile ID=\"x\"/></opt>").unwrap();
    assert_eq!(d.name, " Spaced  Name ");
}

#[test]
fn sp5_panel_id() {
    let e = parse(
        "<opt><PanelFile ID=\"\"><Element ID=\"PanelA\" Width=\"1\" Height=\"1\"/></PanelFile></opt>",
    );
    assert_eq!(e.unwrap_err(), DefinitionError::NothingUsable);
    let d = parse(
        "<opt><PanelFile ID=\"any text at all\"><Element ID=\"PanelA\" Width=\"1\" Height=\"1\"/></PanelFile></opt>",
    )
    .unwrap();
    assert_eq!(d.panels().len(), 1);
    assert_eq!(d.panels()[0].id, "any text at all");
}

#[test]
fn sp6_unresolvable_element_ids() {
    // Top level: the whole PanelFile is dropped.
    let d = parse(
        "<opt><PanelFile ID=\"p\">\
           <Element ID=\"PanelA\" Width=\"1\" Height=\"1\"/>\
           <Element ID=\"NoSuchElement\" Width=\"1\" Height=\"1\"/>\
         </PanelFile><PanelFile ID=\"q\"/></opt>",
    )
    .unwrap();
    assert_eq!(d.panels().len(), 1);
    assert_eq!(d.panels()[0].id, "q");
    assert!(d.top_level(0x1000_0100).is_none());
    assert!(
        d.diagnostics
            .iter()
            .any(|x| matches!(&x.kind, DiagnosticKind::UnknownElement(n) if n == "NoSuchElement"))
    );
    assert!(
        d.diagnostics
            .iter()
            .any(|x| matches!(&x.kind, DiagnosticKind::PanelDropped(n) if n == "p"))
    );
    // Nested: that element and its later siblings only.
    let d = parse(
        "<opt><PanelFile ID=\"p\">\
           <Element ID=\"PanelA\" X=\"1\" Y=\"2\" Width=\"3\" Height=\"4\">\
             <Element ID=\"PanelA_Title\" Width=\"5\" Height=\"5\"/>\
             <Element ID=\"Nope\" Width=\"5\" Height=\"5\"/>\
             <Element ID=\"PanelA_Body\" Width=\"5\" Height=\"5\"/>\
           </Element>\
           <Element ID=\"PanelB\" Width=\"6\" Height=\"6\"/>\
         </PanelFile></opt>",
    )
    .unwrap();
    let a = d.top_level(0x1000_0100).unwrap();
    assert_eq!((a.x, a.y, a.width, a.height), (1, 2, 3, 4));
    assert!(a.child(0x1000_0101).is_some());
    assert!(a.child(0x1000_0102).is_none());
    assert!(
        d.top_level(0x1000_0200).is_some(),
        "the enclosing level continues"
    );
}

#[test]
fn sp7_leaf_without_size_dropped_container_kept() {
    let d = parse(
        "<opt><PanelFile ID=\"p\">\
           <Element ID=\"PanelB\" X=\"5\" Y=\"5\" Width=\"0\" Height=\"9\"/>\
           <Element ID=\"PanelA\"><Element ID=\"PanelA_Title\" Width=\"2\" Height=\"3\"/></Element>\
         </PanelFile></opt>",
    )
    .unwrap();
    assert!(
        d.top_level(0x1000_0200).is_none(),
        "leaf with Width 0 dropped"
    );
    let a = d.top_level(0x1000_0100).unwrap();
    assert_eq!((a.x, a.y, a.width, a.height), (0, 0, 0, 0));
    assert_eq!(a.children().len(), 1);
    // A label whose value is 0 makes the override inert.
    let e = parse(
        "<opt><PanelFile ID=\"p\"><Element ID=\"Undefined\" Width=\"1\" Height=\"1\"/></PanelFile></opt>",
    )
    .unwrap();
    assert!(e.panels()[0].elements().is_empty());
}

#[test]
fn sp8_integers_in_attributes() {
    let d = parse(
        "<opt><PanelFile ID=\"p\"><Element ID=\"PanelA\" X=\" -4\" Y=\"0x14\" Width=\"020\" \
         Height=\"12px\" ScaleOriginX=\"+7\" Detach=\"09\"/></PanelFile></opt>",
    )
    .unwrap();
    let a = d.top_level(0x1000_0100).unwrap();
    assert_eq!((a.x, a.y, a.width, a.height), (-4, 20, 16, 12));
    assert_eq!(a.scale_origin, Some((7, 0)), "a missing origin half is 0");
    assert!(!a.detach, "09 is octal 0");
    let d = parse(
        "<opt><PanelFile ID=\"p\"><Element ID=\"PanelA\" Width=\"99999999999\" Height=\"abc1\" \
         Detach=\"1\"/></PanelFile></opt>",
    )
    .unwrap();
    // Height parses to 0, so the leaf is dropped.
    assert!(d.top_level(0x1000_0100).is_none());
}

#[test]
fn sp9_acceptance() {
    assert_eq!(
        parse("<opt><SkinName Name=\"Only a name\"/></opt>").unwrap_err(),
        DefinitionError::NothingUsable
    );
    assert_eq!(
        parse(
            "<opt><Mapping ArtAssetID=\"nope\" FileName=\"a.tga\"/>\
             <Mapping ArtAssetID=\"Frame_Middle\" FileName=\"\"/>\
             <PanelFile ID=\"p\"><Element ID=\"Nope\" Width=\"1\" Height=\"1\"/></PanelFile></opt>"
        )
        .unwrap_err(),
        DefinitionError::NothingUsable
    );
    let d = parse("<opt><PanelFile ID=\"x\"/></opt>").unwrap();
    assert_eq!(d.panels().len(), 1);
    assert!(d.has_panel_overrides());
    // An art-asset value with no base entry, or 0, is still stored (§4.4, §5.2).
    let d = parse("<opt><Mapping ArtAssetID=\"invalid\" FileName=\"a.tga\"/></opt>").unwrap();
    assert_eq!(d.mappings()[0].art_asset, 0);
}

#[test]
fn sp10_repeated_top_level_rejects_the_later_panel() {
    let d = parse(
        "<opt>\
           <PanelFile ID=\"first\"><Element ID=\"PanelA\" X=\"1\" Width=\"1\" Height=\"1\"/></PanelFile>\
           <PanelFile ID=\"second\">\
             <Element ID=\"PanelB\" Width=\"2\" Height=\"2\"/>\
             <Element ID=\"PanelA\" X=\"9\" Width=\"2\" Height=\"2\"/>\
           </PanelFile>\
         </opt>",
    )
    .unwrap();
    assert_eq!(d.panels().len(), 1);
    assert_eq!(
        d.top_level(0x1000_0100).unwrap().x,
        1,
        "the first stays effective"
    );
    assert!(
        d.top_level(0x1000_0200).is_none(),
        "D8: none of the later panel's IDs"
    );
    assert!(!d.uses_element(0x1000_0200));
    assert!(d.diagnostics.iter().any(|x| matches!(
        &x.kind,
        DiagnosticKind::RepeatedTopLevel { panel, first_panel, .. } if panel == "second" && first_panel == "first"
    )));
}

#[test]
fn first_sibling_wins_and_matching_mirrors_the_tree() {
    let d = parse(
        "<opt><PanelFile ID=\"p\">\
           <Element ID=\"PanelA\" Width=\"1\" Height=\"1\">\
             <Element ID=\"PanelA_Title\" X=\"1\" Width=\"1\" Height=\"1\">\
               <Element ID=\"Frame_Close\" X=\"7\" Width=\"1\" Height=\"1\"/>\
             </Element>\
             <Element ID=\"PanelA_Title\" X=\"2\" Width=\"1\" Height=\"1\"/>\
           </Element>\
         </PanelFile></opt>",
    )
    .unwrap();
    let a = d.top_level(0x1000_0100).unwrap();
    assert_eq!(a.children().len(), 1);
    assert_eq!(a.child(0x1000_0101).unwrap().x, 1);
    // §8.3: root of a build, then nested paths; an intermediate level cannot be skipped.
    assert_eq!(d.find_override(&[], 0x1000_0100).unwrap().x, 0);
    assert_eq!(d.find_override(&[0x1000_0100], 0x1000_0101).unwrap().x, 1);
    assert_eq!(
        d.find_override(&[0x1000_0100, 0x1000_0101], 0x1000_0201)
            .unwrap()
            .x,
        7
    );
    assert!(d.find_override(&[0x1000_0100], 0x1000_0201).is_none());
    assert!(
        d.find_override(&[], 0x1000_0101).is_none(),
        "not a top-level ID"
    );
    assert!(d.find_override(&[0x0BAD], 0x1000_0101).is_none());
}

#[test]
fn worked_example_12_2() {
    let xml = "<anything>\n\
      <SkinName Name=\"Example\"/>\n\
      <Mapping ArtAssetID=\"BasePanel_TopLeft\" FileName=\"art\\tl.tga\"/>\n\
      <Mapping ArtAssetID=\"basepanel_topleft\" FileName=\"art/other.tga\"/>\n\
      <Mapping ArtAssetID=\"no_such_label\" FileName=\"x.tga\"/>\n\
      <Mapping ArtAssetID=\"\" FileName=\"y.tga\"/>\n\
    </anything>";
    let d = parse(xml).unwrap();
    assert_eq!(d.name, "Example");
    assert_eq!(d.mappings().len(), 1);
    assert_eq!(d.mapping(ART_A).unwrap().file_name, "art\\tl.tga");
    assert_eq!(d.diagnostics.len(), 2, "{:?}", d.diagnostics);
    assert!(matches!(
        d.diagnostics[0].kind,
        DiagnosticKind::RepeatedMapping { .. }
    ));
    assert_eq!(d.diagnostics[0].line, 4);
    assert!(
        matches!(&d.diagnostics[1].kind, DiagnosticKind::UnknownArtAsset(n) if n == "no_such_label")
    );
    assert_eq!(d.diagnostics[1].line, 5);
}

#[test]
fn ignored_content_and_xml_features() {
    let d = parse(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- c --><?pi data?>\n\
         <opt><Other><Mapping ArtAssetID=\"basepanel_topleft\" FileName=\"no.tga\"/></Other>\
         text <![CDATA[ <Mapping/> ]]>&amp;\
         <Mapping ArtAssetID=\"Frame&#95;Middle\" FileName=\"a&apos;b.tga\"><Child/></Mapping>\
         <PanelFile ID=\"p\"><Junk/><Element ID=\"PanelA\" Width=\"1\" Height=\"1\"><Junk><Element ID=\"Nope\"/></Junk></Element></PanelFile>\
         </opt>",
    )
    .unwrap();
    assert_eq!(
        d.mappings().len(),
        1,
        "a nested Mapping is not a root child"
    );
    assert_eq!(d.mapping(ART_B).unwrap().file_name, "a'b.tga");
    assert!(d.diagnostics.is_empty(), "{:?}", d.diagnostics);
}

#[test]
fn d5_dtd_and_encodings() {
    // A DOCTYPE with an external subset is ignored, never fetched.
    let d = parse(
        "<!DOCTYPE opt SYSTEM \"http://example.invalid/skin.dtd\"><opt><PanelFile ID=\"x\"/></opt>",
    )
    .unwrap();
    assert_eq!(d.panels().len(), 1);
    // Internal entities are not expanded: the document is rejected.
    let e = parse(
        "<!DOCTYPE opt [<!ENTITY n \"Frame_Middle\">]><opt><Mapping ArtAssetID=\"&n;\" FileName=\"a.tga\"/></opt>",
    )
    .unwrap_err();
    assert!(
        matches!(&e, DefinitionError::Xml(x) if matches!(x.kind, XmlErrorKind::UndefinedEntity(_)))
    );
    // UTF-16 with a byte-order mark.
    let text = "<opt><SkinName Name=\"Ünïcode\"/><PanelFile ID=\"x\"/></opt>";
    let bytes: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(parse_definition(&bytes, &names()).unwrap().name, "Ünïcode");
    // Other encodings are rejected.
    let e = parse_definition(
        b"<?xml version='1.0' encoding='EUC-JP'?><opt><PanelFile ID='x'/></opt>",
        &names(),
    )
    .unwrap_err();
    assert!(
        matches!(&e, DefinitionError::Xml(x) if matches!(x.kind, XmlErrorKind::UnsupportedEncoding(_)))
    );
    // White space inside the declaration is unbounded: a declaration that ends after the
    // first kilobyte still has its encoding checked.
    for (enc, ok) in [("Shift_JIS", false), ("ISO-8859-1", true)] {
        let doc = format!(
            "<?xml version=\"1.0\"{}encoding=\"{enc}\"?><opt><Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"/></opt>",
            " ".repeat(1_100)
        );
        let r = parse_definition(doc.as_bytes(), &names());
        if ok {
            assert_eq!(r.unwrap().mappings().len(), 1);
        } else {
            assert!(
                matches!(&r, Err(DefinitionError::Xml(x)) if matches!(&x.kind, XmlErrorKind::UnsupportedEncoding(e) if e == enc)),
                "{r:?}"
            );
        }
    }
}

#[test]
fn malformed_documents_report_a_line() {
    for (xml, line) in [
        (
            "<opt>\n<Mapping ArtAssetID=\"a\" FileName=\"b\">\n</opt>",
            3,
        ),
        ("<opt>\n\n<!-- a -- b -->\n</opt>", 3),
        ("<opt/>\n<second/>", 2),
        ("", 1),
    ] {
        let e = parse(xml).unwrap_err();
        assert_eq!(e.line(), Some(line), "{xml:?}: {e}");
    }
}

#[test]
fn d4_limits() {
    let big = vec![b' '; MAX_DEFINITION_BYTES + 1];
    assert!(matches!(
        parse_definition(&big, &names()),
        Err(DefinitionError::TooLarge { .. })
    ));
    let mut many = String::from("<opt>");
    for _ in 0..=MAX_MAPPINGS {
        many.push_str("<Mapping/>");
    }
    many.push_str("</opt>");
    assert_eq!(parse(&many).unwrap_err(), DefinitionError::TooManyMappings);
    // Element nesting deeper than 64 levels is malformed.
    let depth = 70;
    let mut deep = String::from("<opt><PanelFile ID=\"p\">");
    for _ in 0..depth {
        deep.push_str("<Element ID=\"PanelA\">");
    }
    for _ in 0..depth {
        deep.push_str("</Element>");
    }
    deep.push_str("</PanelFile></opt>");
    assert!(
        matches!(&parse(&deep).unwrap_err(), DefinitionError::Xml(x) if x.kind == XmlErrorKind::TooDeep)
    );
    // At most MAX_XML_ATTRIBUTES attributes on one tag (a Tessera bound).
    let tag = |n: usize| {
        let mut t = String::from("<opt><Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"");
        for i in 2..n {
            t.push_str(&format!(" a{i}=''"));
        }
        t.push_str("/></opt>");
        t
    };
    assert_eq!(parse(&tag(MAX_XML_ATTRIBUTES)).unwrap().mappings().len(), 1);
    assert!(
        matches!(&parse(&tag(MAX_XML_ATTRIBUTES + 1)).unwrap_err(), DefinitionError::Xml(x) if x.kind == XmlErrorKind::TooManyAttributes)
    );
    // Duplicates are still found beyond the first sixteen attributes.
    let mut dup = String::from("<opt><Mapping ArtAssetID=\"Frame_Middle\" FileName=\"a.tga\"");
    for i in 0..40 {
        dup.push_str(&format!(" a{i}=''"));
    }
    dup.push_str(" a3='' /></opt>");
    assert!(
        matches!(&parse(&dup).unwrap_err(), DefinitionError::Xml(x) if matches!(&x.kind, XmlErrorKind::DuplicateAttribute(n) if n == "a3"))
    );
}
