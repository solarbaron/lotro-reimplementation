// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for UI layout records (prefix 0x22), `docs/specs/ui-layout.md` §4, §6,
//! §12–§14: the 158-byte worked example of §14.1, every media kind, the §12.1 rejections,
//! the invariant checks, the census, references, base chains (§7.1, §13, the §14.3 and
//! §14.5 shapes) and labels (§3.3). All data is invented.

mod common;

use std::collections::BTreeMap;

use common::B;
use proptest::prelude::*;
use tessera_formats::property::PropertyType;
use tessera_formats::ui_layout::{
    Argb, BaseChainError, BaseRef, DrawMode, EdgeMode, FailureCategory, LayoutIssue, LayoutRef,
    MAX_BASE_CHAIN, MAX_ELEMENT_DEPTH, MediaItem, MediaKind, UiEnum, UiLabels, base_chain,
};
use tessera_formats::{
    AnyRecord, Did, EnumMapper, FormatError, NoSchema, PropertyValue, Record, StringSource,
    UiLayout, decode_ui_layout,
};

// ---------------------------------------------------------------------------------------
// A small encoder for synthetic records (test side only, written from §4).
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Blk {
    id: u32,
    flags: u8,
    rect: [i32; 4],
    z: i32,
    has_pos: u8,
    /// Whole embedded property set (TSize + entries); empty means `00 00`.
    props: Vec<u8>,
    media: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
struct El {
    blk: Blk,
    read_order: u32,
    id: u32,
    ty: u32,
    base_el: u32,
    base_layout: u32,
    default_state: u32,
    edges: [u32; 4],
    states_hint: u8,
    states: Vec<Blk>,
    children: Vec<El>,
}

fn enc_blk(b: B, k: &Blk) -> B {
    let mut b = b.u32(k.id).u8(k.flags);
    for v in k.rect {
        b = b.u32(v as u32);
    }
    b = b.u32(k.z as u32).u8(k.has_pos);
    b = if k.props.is_empty() {
        b.u8(0).u8(0)
    } else {
        b.bytes(&k.props)
    };
    b = b.vle(k.media.len() as u32);
    for m in &k.media {
        b = b.bytes(m);
    }
    b
}

fn enc_el(b: B, e: &El) -> B {
    let mut b = enc_blk(b.u32(e.id), &e.blk)
        .u32(e.read_order)
        .u32(e.id)
        .u32(e.ty)
        .u32(e.base_el)
        .u32(e.base_layout)
        .u32(e.default_state);
    for v in e.edges {
        b = b.u32(v);
    }
    b = b.tsize(e.states_hint, e.states.len() as u32);
    for s in &e.states {
        b = enc_blk(b.u32(s.id), s);
    }
    b = b.tsize(0, e.children.len() as u32);
    for c in &e.children {
        b = enc_el(b, c);
    }
    b
}

fn enc_layout(did: u32, w: u32, h: u32, elements: &[El]) -> Vec<u8> {
    let mut b = B::new()
        .u32(did)
        .u32(w)
        .u32(h)
        .tsize(0, elements.len() as u32);
    for e in elements {
        b = enc_el(b, e);
    }
    b.build()
}

fn item(kind: u32) -> B {
    B::new().u32(kind).u32(kind)
}
fn image(did: u32, mode: u32, path: &str) -> Vec<u8> {
    item(4).u32(did).u32(mode).pascal(path).build()
}
fn typed(id: u32, ty: u32, read_order: u32) -> El {
    El {
        id,
        ty,
        read_order,
        states_hint: 1,
        blk: Blk {
            has_pos: 1,
            ..Blk::default()
        },
        ..El::default()
    }
}
fn based(id: u32, read_order: u32, base_el: u32, base_layout: u32) -> El {
    El {
        id,
        read_order,
        base_el,
        base_layout,
        states_hint: 1,
        blk: Blk {
            has_pos: 1,
            ..Blk::default()
        },
        ..El::default()
    }
}

fn parse(data: &[u8]) -> Result<UiLayout, FormatError> {
    UiLayout::parse(data, &NoSchema)
}

// ---------------------------------------------------------------------------------------
// §14.1: the 158-byte record.
// ---------------------------------------------------------------------------------------

/// The bytes of §14.1, transcribed from the offset table.
fn example_14_1() -> Vec<u8> {
    let hex = "01F0FF22 20030000 58020000 0001 34120010 00000000 01 \
               0A000000 14000000 64000000 1E000000 FBFFFFFF 01 0000 01 \
               04000000 04000000 EFCDAB41 04000000 00 \
               00000000 34120010 09000000 00000000 00000000 0B000000 \
               05000000 05000000 04000000 04000000 0101 0B000000 0B000000 \
               00 0000000000000000000000000000000000000000 00 0000 01 \
               04000000 04000000 F0CDAB41 04000000 00 0000";
    let digits: Vec<u8> = hex.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn worked_example_14_1_bytes_and_fields() {
    let data = example_14_1();
    assert_eq!(data.len(), 158);
    // The test encoder produces the same bytes from the described fields.
    let button = El {
        blk: Blk {
            flags: 1,
            rect: [10, 20, 100, 30],
            z: -5,
            has_pos: 1,
            media: vec![image(0x41AB_CDEF, 4, "")],
            ..Blk::default()
        },
        read_order: 0,
        id: 0x1000_1234,
        ty: 9,
        default_state: 0x0B,
        edges: [5, 5, 4, 4],
        states_hint: 1,
        states: vec![Blk {
            id: 0x0B,
            media: vec![image(0x41AB_CDF0, 4, "")],
            ..Blk::default()
        }],
        ..El::default()
    };
    assert_eq!(enc_layout(0x22FF_F001, 800, 600, &[button]), data);

    let l = parse(&data).unwrap();
    assert_eq!(l.did, Did(0x22FF_F001));
    assert_eq!((l.design_width, l.design_height), (800, 600));
    assert_eq!(l.elements.len(), 1);
    let e = &l.elements[0];
    assert_eq!(e.element_id, 0x1000_1234);
    assert_eq!(e.element_type, 9);
    assert!(e.is_typed());
    assert_eq!(e.base(), None);
    assert_eq!(e.default_state, 0x0B);
    assert_eq!(e.read_order, 0);
    assert_eq!(
        e.edges.to_array(),
        [
            EdgeMode::OWN_SIDE,
            EdgeMode::OWN_SIDE,
            EdgeMode::OPPOSITE_SIDE,
            EdgeMode::OPPOSITE_SIDE
        ]
    );
    assert_eq!(e.block.state_id, 0);
    assert!(e.block.flags.pass_to_children());
    assert!(!e.block.flags.keep_inside_parent());
    assert_eq!(
        (
            e.block.rect.x,
            e.block.rect.y,
            e.block.rect.width,
            e.block.rect.height
        ),
        (10, 20, 100, 30)
    );
    assert_eq!(e.block.z_level, -5);
    assert!(e.block.has_position);
    assert!(e.block.properties.is_empty());
    assert_eq!(
        e.block.media,
        vec![MediaItem::Image {
            did: Did(0x41AB_CDEF),
            draw_mode: DrawMode::ALPHABLEND,
            path: String::new(),
        }]
    );
    assert_eq!(e.states_size.hint, 1);
    let s = e.state(0x0B).unwrap();
    assert_eq!(s.state_id, 0x0B);
    assert!(s.rect.is_zero());
    assert_eq!(
        s.media,
        vec![MediaItem::Image {
            did: Did(0x41AB_CDF0),
            draw_mode: DrawMode::ALPHABLEND,
            path: String::new(),
        }]
    );
    assert!(e.children.is_empty());
    assert_eq!(l.issues(usize::MAX), vec![]);

    let c = l.census();
    assert_eq!(
        (c.records, c.top_level, c.elements, c.typed, c.based),
        (1, 1, 1, 1, 0)
    );
    assert_eq!(
        (c.states, c.media_in_elements, c.media_in_states),
        (1, 1, 1)
    );
    assert_eq!(c.media_by_kind[4], 2);
    assert_eq!(
        (c.default_states, c.without_position, c.single_top_level),
        (1, 0, 1)
    );
    assert_eq!((c.media_paths, c.max_media_path), (2, 0));

    let mut refs = Vec::new();
    l.for_each_reference(|r| refs.push(format!("{r:?}")));
    assert_eq!(refs.len(), 2);

    // Routed through AnyRecord with a schema; not without one.
    assert!(!AnyRecord::is_decodable(0x22FF_F001u32));
    assert!(AnyRecord::is_decodable_with_schema(0x22FF_F001u32));
    assert!(AnyRecord::decode(0x22FF_F001u32, &data).unwrap().is_none());
    let any = AnyRecord::decode_with_schema(0x22FF_F001u32, &data, &NoSchema)
        .unwrap()
        .unwrap();
    assert!(matches!(any, AnyRecord::UiLayout(_)));
    assert_eq!(any.did(), Did(0x22FF_F001));
    assert_eq!(any.type_name(), "UI layout");
    assert!(matches!(
        AnyRecord::decode_with_schema(0x22FF_F002u32, &data, &NoSchema),
        Err(FormatError::DidMismatch { .. })
    ));
    assert_eq!(decode_ui_layout(&data, &NoSchema).unwrap(), l);
}

#[test]
fn every_truncation_and_extension_of_14_1_fails() {
    let data = example_14_1();
    for n in 0..data.len() {
        let e = parse(&data[..n]).unwrap_err();
        assert_eq!(
            FailureCategory::of_decode_error(&e),
            FailureCategory::RecordDoesNotDecode
        );
    }
    let mut longer = data.clone();
    longer.push(0);
    assert!(matches!(
        parse(&longer),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
}

#[test]
fn single_byte_changes_never_panic() {
    let data = example_14_1();
    for i in 0..data.len() {
        for v in [0x00, 0x01, 0x02, 0x7F, 0x80, 0xE0, 0xFF] {
            let mut d = data.clone();
            d[i] = v;
            let _ = parse(&d);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Media items (§6.1).
// ---------------------------------------------------------------------------------------

fn layout_with_media(media: Vec<Vec<u8>>) -> Vec<u8> {
    let mut e = typed(0x1000_0001, 0x0E, 0);
    e.blk.media = media;
    enc_layout(0x2200_0001, 1024, 768, &[e])
}

#[test]
fn every_media_kind_decodes() {
    let media = vec![
        item(1).u32(0).pascal("raw/intro.bik").u8(1).build(),
        item(3).u32(2).f32(0.5).build(),
        image(0, 2, "raw/loading.jpg"),
        item(5).u32(0x1B).f32(1.0).build(),
        item(6).f32(1.0).f32(0.0).f32(0.5).build(),
        item(7).u32(0x4100_0007).build(),
        item(8)
            .f32(1.1)
            .u32(4)
            .u8(1)
            .u32(3)
            .u32(0x4100_0001)
            .u32(0x4100_0002)
            .u32(0x4100_0003)
            .build(),
        item(9).u32(0).u32(6).u32(0xBF00_0000).build(),
        item(10).u32(0x4100_0010).u32(16).u32(14).build(),
        item(11).u32(0x2A00_0005).build(),
        item(12).u32(0x0B).f32(0.25).build(),
        item(13).f32(0.5).f32(0.2).build(),
    ];
    let l = parse(&layout_with_media(media)).unwrap();
    let m = &l.elements[0].block.media;
    let kinds: Vec<MediaKind> = m.iter().map(MediaItem::kind).collect();
    assert_eq!(kinds, MediaKind::ALL.to_vec());
    assert_eq!(
        m[0],
        MediaItem::Movie {
            did: Did(0),
            path: "raw/intro.bik".into(),
            fit_to_screen: true
        }
    );
    assert_eq!(m[2].path(), Some("raw/loading.jpg"));
    assert_eq!(m[2].draw_mode(), Some(DrawMode::NORMAL));
    assert_eq!(
        m[6],
        MediaItem::Animation {
            duration: 1.1,
            draw_mode: DrawMode::ALPHABLEND,
            looping: true,
            frames: vec![Did(0x4100_0001), Did(0x4100_0002), Did(0x4100_0003)],
        }
    );
    let MediaItem::ColouredImage {
        colour, draw_mode, ..
    } = m[7]
    else {
        panic!("coloured image expected");
    };
    assert_eq!(draw_mode, DrawMode::GRAYSCALE);
    assert_eq!(draw_mode.name(), Some("Grayscale"));
    assert_eq!(
        (colour.a(), colour.r(), colour.g(), colour.b()),
        (0xBF, 0, 0, 0)
    );
    assert_eq!(Argb(0x1122_3344).r(), 0x22);
    assert_eq!(
        m[8],
        MediaItem::Cursor {
            did: Did(0x4100_0010),
            hotspot_x: 16,
            hotspot_y: 14
        }
    );
    assert_eq!(m[11], MediaItem::Pause { min: 0.5, max: 0.2 });
    let mut dids = 0;
    for it in m {
        it.for_each_did(|_| dids += 1);
    }
    // Movie, Image, HitMask, 3 frames, Coloured image, Cursor, Sound.
    assert_eq!(dids, 9);
    let c = l.census();
    assert_eq!(c.media_in_elements, 12);
    assert_eq!(c.media_paths, 2);
    assert_eq!(c.max_media_path, 15);
    for k in MediaKind::ALL {
        assert_eq!(c.media_by_kind[k.code() as usize], 1);
        assert_eq!(MediaKind::from_code(k.code()), Some(k));
    }
}

#[test]
fn unknown_media_kinds_are_their_own_category() {
    for kind in [0u32, 2, 14, 0xFFFF_FFFF] {
        let data = layout_with_media(vec![item(kind).u32(0).build()]);
        let e = parse(&data).unwrap_err();
        assert!(
            matches!(e, FormatError::Unsupported { value, .. } if value == kind),
            "{e:?}"
        );
        assert_eq!(
            FailureCategory::of_decode_error(&e),
            FailureCategory::UnknownMediaKind
        );
    }
    // UL4: the repeated kind must equal the tag.
    let data = layout_with_media(vec![B::new().u32(11).u32(4).u32(0).build()]);
    let e = parse(&data).unwrap_err();
    // §12.1: a key/ID mismatch (UL2, UL3, UL4) is reported as malformed.
    assert!(
        matches!(e, FormatError::Malformed { ref reason, .. } if reason.contains("UL4")),
        "{e:?}"
    );
    assert_eq!(
        FailureCategory::of_decode_error(&e),
        FailureCategory::RecordDoesNotDecode
    );
}

#[test]
fn hostile_counts_are_bounded_by_the_input() {
    // A frame count far beyond the remaining bytes.
    let data = layout_with_media(vec![item(8).f32(1.0).u32(4).u8(0).u32(0x4000_0000).build()]);
    assert!(matches!(parse(&data), Err(FormatError::Truncated { .. })));
    // A media count, a state count and an element count beyond the input.
    for (hint, count) in [(1u8, 0x3FFF_FFFFu32), (0, 1_000_000)] {
        let data = B::new()
            .u32(0x2200_0001)
            .u32(0)
            .u32(0)
            .tsize(hint, count)
            .build();
        assert!(matches!(parse(&data), Err(FormatError::Truncated { .. })));
    }
    let mut e = typed(0x1000_0001, 1, 0);
    e.blk.media = vec![];
    let mut data = enc_layout(0x2200_0001, 0, 0, &[e]);
    // Patch the element block's media count (after the empty property set) to 100.
    let at = 4 + 12 + 2 + 4 + 1 + 16 + 4 + 1 + 2;
    assert_eq!(data[at], 0);
    data[at] = 100;
    assert!(matches!(parse(&data), Err(FormatError::Truncated { .. })));
}

#[test]
fn path_lengths_beyond_the_input_are_rejected() {
    let data = layout_with_media(vec![item(4).u32(0).u32(2).vle(0x3FFF).build()]);
    assert!(matches!(parse(&data), Err(FormatError::Truncated { .. })));
}

// ---------------------------------------------------------------------------------------
// §12.1 rejections and the depth limit.
// ---------------------------------------------------------------------------------------

#[test]
fn key_and_id_mismatches_are_malformed() {
    // UL3: element ID differs from its key.
    let e = typed(0x1000_0001, 1, 0);
    let mut data = enc_layout(0x2200_0001, 0, 0, &[e]);
    // The key is at offset 14; the element_id field follows the block (29 bytes) and the
    // read order.
    let id_at = 14 + 4 + 29 + 4;
    data[id_at] = 0x02;
    assert!(matches!(parse(&data), Err(FormatError::Malformed { .. })));

    // UL2: own block with a non-zero state ID.
    let mut e = typed(0x1000_0001, 1, 0);
    e.blk.id = 7;
    let data = enc_layout(0x2200_0001, 0, 0, &[e]);
    assert!(matches!(parse(&data), Err(FormatError::Malformed { .. })));

    // UL2: state entry whose block ID differs from its key.
    let mut e = typed(0x1000_0001, 1, 0);
    e.states = vec![Blk {
        id: 0x0B,
        ..Blk::default()
    }];
    let mut data = enc_layout(0x2200_0001, 0, 0, &[e]);
    let state_key_at = 14 + 4 + 29 + 40 + 2;
    assert_eq!(data[state_key_at], 0x0B);
    data[state_key_at + 4] = 0x0C;
    assert!(matches!(parse(&data), Err(FormatError::Malformed { .. })));
}

#[test]
fn bool8_above_one_is_rejected() {
    let mut e = typed(0x1000_0001, 1, 0);
    e.blk.has_pos = 2;
    let data = enc_layout(0x2200_0001, 0, 0, &[e]);
    assert!(matches!(
        parse(&data),
        Err(FormatError::InvalidBool { value: 2, .. })
    ));
    let data = layout_with_media(vec![item(1).u32(0).pascal("").u8(3).build()]);
    assert!(matches!(
        parse(&data),
        Err(FormatError::InvalidBool { value: 3, .. })
    ));
}

#[test]
fn wrong_prefix_is_rejected() {
    let data = enc_layout(0x4200_0001, 0, 0, &[]);
    assert!(matches!(
        parse(&data),
        Err(FormatError::WrongRecordType { .. })
    ));
    let data = enc_layout(0x2200_0001, 0, 0, &[]);
    let l = parse(&data).unwrap();
    assert!(l.elements.is_empty());
    assert_eq!(l.element_count(), 0);
}

fn nested(depth: usize) -> El {
    let mut e = typed(0x1000_0001, 1, 0);
    for _ in 0..depth {
        let mut parent = typed(0x1000_0001, 1, 0);
        e.read_order = 1;
        parent.children = vec![e];
        e = parent;
    }
    e
}

#[test]
fn nesting_is_limited() {
    let ok = enc_layout(0x2200_0001, 0, 0, &[nested(MAX_ELEMENT_DEPTH)]);
    let l = parse(&ok).unwrap();
    assert_eq!(l.census().max_depth, MAX_ELEMENT_DEPTH as u64);
    assert_eq!(l.element_count(), MAX_ELEMENT_DEPTH + 1);
    let deep = enc_layout(0x2200_0001, 0, 0, &[nested(MAX_ELEMENT_DEPTH + 1)]);
    assert!(matches!(parse(&deep), Err(FormatError::DepthLimit { .. })));
}

// ---------------------------------------------------------------------------------------
// Properties (§5.9) and references (§10).
// ---------------------------------------------------------------------------------------

const PID_DRAW_AFTER: u32 = 0x3CD;
const PID_TEXT_ENTRY: u32 = 0x70;
const PID_FONT: u32 = 0x6E9;

fn schema() -> BTreeMap<u32, PropertyType> {
    BTreeMap::from([
        (PID_DRAW_AFTER, PropertyType::Bool),
        (PID_TEXT_ENTRY, PropertyType::StringInfo),
        (PID_FONT, PropertyType::DidRef),
    ])
}

#[test]
fn properties_and_references() {
    // Bucket order for hint 0 (11 buckets): 0x70 % 11 = 2, 0x3CD % 11 = 5, 0x6E9 % 11 = 4.
    let props = B::new()
        .tsize(0, 3)
        .u32(PID_TEXT_ENTRY)
        .u32(PID_TEXT_ENTRY)
        .si_ref(0x0000_1234, 0x2500_0042)
        .u32(PID_FONT)
        .u32(PID_FONT)
        .u32(0x4200_0007)
        .u32(PID_DRAW_AFTER)
        .u32(PID_DRAW_AFTER)
        .u8(1)
        .build();
    let mut e = based(0x1000_0002, 0, 0x1000_0001, 0x2200_0009);
    e.blk.props = props;
    e.blk.media = vec![item(11).u32(0x2A00_0001).build()];
    let data = enc_layout(0x2200_0001, 1024, 768, &[e]);
    let s = schema();
    let l = UiLayout::parse(&data, &s).unwrap();
    let p = &l.elements[0].block.properties;
    assert_eq!(p.get(PID_DRAW_AFTER), Some(&PropertyValue::Bool(true)));
    assert_eq!(
        p.get(PID_FONT),
        Some(&PropertyValue::DidRef(Did(0x4200_0007)))
    );
    let mut refs = Vec::new();
    l.for_each_reference(|r| {
        refs.push(match r {
            LayoutRef::Media { kind, did } => format!("media {} {did}", kind.name()),
            LayoutRef::Property { pid, did } => format!("prop {pid:#x} {did}"),
            LayoutRef::String { source, .. } => match source {
                StringSource::Table { token, table } => format!("string {table} {token:#x}"),
                StringSource::Literal(_) => "literal".into(),
            },
            LayoutRef::Base(b) => format!("base {:#x} {}", b.element, b.layout),
        });
    });
    refs.sort();
    assert_eq!(
        refs,
        vec![
            "base 0x10000001 0x22000009",
            "media Sound 0x2A000001",
            "prop 0x6e9 0x42000007",
            "string 0x25000042 0x1234",
        ]
    );
    // Without a schema the PIDs are unknown: the record does not decode.
    let e = parse(&data).unwrap_err();
    assert!(matches!(
        e.root_cause(),
        FormatError::UnknownProperty { .. }
    ));
    assert_eq!(
        FailureCategory::of_decode_error(&e),
        FailureCategory::RecordDoesNotDecode
    );
}

// ---------------------------------------------------------------------------------------
// Invariants (§12).
// ---------------------------------------------------------------------------------------

#[test]
fn invariant_checks_report_each_rule() {
    // UL9: children numbered 1..n; top level 0..n-1.
    let mut parent = typed(0x1000_0001, 1, 0);
    parent.children = vec![typed(0x1000_0002, 1, 1), typed(0x1000_0003, 1, 2)];
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[parent.clone()])).unwrap();
    assert_eq!(l.issues(usize::MAX), vec![]);
    let ordered: Vec<u32> = l.elements[0]
        .children_in_read_order()
        .iter()
        .map(|c| c.element_id)
        .collect();
    assert_eq!(ordered, vec![0x1000_0002, 0x1000_0003]);

    let mut bad = parent.clone();
    bad.children[1].read_order = 1;
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[bad])).unwrap();
    assert_eq!(
        l.issues(usize::MAX),
        vec![LayoutIssue::ReadOrder {
            element: 0x1000_0001
        }]
    );

    // UL8: bucket order of child keys for hint 0 (11 buckets): 0x10000002 % 11 = 2,
    // 0x10000003 % 11 = 3; reversing them breaks it.
    let mut bad = parent.clone();
    bad.children.reverse();
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[bad])).unwrap();
    assert!(l.issues(usize::MAX).contains(&LayoutIssue::BucketOrder {
        element: 0x1000_0001
    }));

    // UL5, UL6, UL7, UL8 state hint, UL10, UL14.
    let mut e = typed(0x1000_0001, 1, 0);
    e.base_el = 0x1000_0009;
    e.blk.flags = 0x04;
    e.blk.rect = [0, 0, -1, 5];
    e.edges = [7, 0, 0, 0];
    e.states_hint = 0;
    e.states = vec![Blk {
        id: 3,
        z: 1,
        flags: 0x80,
        ..Blk::default()
    }];
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[e])).unwrap();
    let issues = l.issues(usize::MAX);
    let names: Vec<&str> = issues.iter().map(|i| i.invariant()).collect();
    for want in ["UL5", "UL6", "UL7", "UL8", "UL10", "UL14"] {
        assert!(names.contains(&want), "{want} missing from {names:?}");
    }
    assert!(issues.contains(&LayoutIssue::FlagBits {
        element: 0x1000_0001,
        state: 3
    }));
    assert_eq!(l.issues(2).len(), 2);

    // UL5: neither type nor base.
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[typed(0x1000_0001, 0, 0)])).unwrap();
    assert_eq!(
        l.issues(usize::MAX),
        vec![LayoutIssue::TypeAndBase {
            element: 0x1000_0001
        }]
    );
    // §4.4 / §11: such an element is neither typed nor based in the census.
    let c = l.census();
    assert!(!l.elements[0].is_typed());
    assert_eq!((c.elements, c.typed, c.based), (1, 0, 0));
    // A type and a base: counted only as based.
    let mut both = typed(0x1000_0001, 1, 0);
    both.base_el = 0x1000_0009;
    both.base_layout = 0x2200_0001;
    let c = parse(&enc_layout(0x2200_0001, 0, 0, &[both]))
        .unwrap()
        .census();
    assert_eq!((c.elements, c.typed, c.based), (1, 0, 1));
}

#[test]
fn duplicate_keys_break_ul8() {
    // §4.2 / UL8: keys are unique within a table. Two top-level elements with the same key,
    // in bucket order and with valid read orders, decode; the issue is reported and lookups
    // return the first stored entry.
    let first = typed(0x1000_0002, 1, 0);
    let mut second = typed(0x1000_0002, 2, 1);
    second.blk.z = 7;
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[first, second])).unwrap();
    assert_eq!(l.elements.len(), 2);
    assert_eq!(
        l.issues(usize::MAX),
        vec![LayoutIssue::DuplicateElementKey {
            element: 0,
            key: 0x1000_0002
        }]
    );
    assert_eq!(l.top_level(0x1000_0002).unwrap().element_type, 1);
    assert_eq!(l.top_level_index(0x1000_0002), Some(0));
    assert_eq!(
        LayoutIssue::DuplicateElementKey { element: 0, key: 1 }.invariant(),
        "UL8"
    );

    // Two children with the same key.
    let mut parent = typed(0x1000_0001, 1, 0);
    parent.children = vec![typed(0x1000_0003, 1, 1), typed(0x1000_0003, 2, 2)];
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[parent])).unwrap();
    assert_eq!(
        l.issues(usize::MAX),
        vec![LayoutIssue::DuplicateElementKey {
            element: 0x1000_0001,
            key: 0x1000_0003
        }]
    );
    assert_eq!(
        l.elements[0].child(0x1000_0003).unwrap().element_type,
        1,
        "first stored child wins"
    );

    // Two states with the same ID (three copies are reported once).
    let mut e = typed(0x1000_0001, 1, 0);
    e.states = vec![
        Blk {
            id: 3,
            ..Blk::default()
        },
        Blk {
            id: 3,
            media: vec![image(0x4100_0001, 4, "")],
            ..Blk::default()
        },
        Blk {
            id: 3,
            ..Blk::default()
        },
    ];
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[e])).unwrap();
    assert_eq!(
        l.issues(usize::MAX),
        vec![LayoutIssue::DuplicateStateKey {
            element: 0x1000_0001,
            key: 3
        }]
    );
    assert!(l.elements[0].state(3).unwrap().media.is_empty());
}

#[test]
fn media_path_length_counts_stored_bytes() {
    // §11 measures the stored Pascal string: ten Latin-1 bytes above 0x7F in a 40-byte path
    // are 40 bytes, not the 50 bytes of their UTF-8 form.
    let path: String = "é".repeat(10) + &"a".repeat(30);
    assert_eq!(path.len(), 50);
    let mut e = typed(0x1000_0001, 1, 0);
    e.blk.media = vec![image(0x4100_0001, 4, &path)];
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &[e])).unwrap();
    assert_eq!(l.census().max_media_path, 40);
}

#[test]
fn base_chains_accept_owning_handles() {
    // A decoded-asset cache hands out `Arc<UiLayout>`; the chain keeps the handles.
    use std::collections::HashMap;
    use std::sync::Arc;
    let lib = Arc::new(
        parse(&enc_layout(
            0x22FF_F030,
            0,
            0,
            &[typed(X, 0x10, 0), based(T, 1, X, 0x22FF_F030)],
        ))
        .unwrap(),
    );
    let user = parse(&enc_layout(
        0x22FF_F040,
        0,
        0,
        &[based(E, 0, T, 0x22FF_F030)],
    ))
    .unwrap();
    let cache: HashMap<Did, Arc<UiLayout>> = HashMap::from([(lib.did, Arc::clone(&lib))]);
    let chain = base_chain(user.top_level(E).unwrap(), |d| cache.get(&d).cloned()).unwrap();
    drop(cache);
    assert_eq!(chain.depth(), 2);
    assert_eq!(chain.links[1].index, 0);
    assert_eq!(chain.end().unwrap().element_id, X);
    assert!(Arc::ptr_eq(&chain.links[0].layout, &lib));
    assert_eq!(chain.links[0].element().unwrap().element_id, T);
}

// ---------------------------------------------------------------------------------------
// Base chains (§7.1, §13) over the §14.3 and §14.5 shapes.
// ---------------------------------------------------------------------------------------

const T: u32 = 0x1000_0100;
const E: u32 = 0x1000_0200;
const X: u32 = 0x1000_0300;
const P: u32 = 0x1000_0400;

fn lookup<'a>(layouts: &'a [UiLayout]) -> impl FnMut(Did) -> Option<&'a UiLayout> {
    move |d| layouts.iter().find(|l| l.did == d)
}

#[test]
fn base_chains_resolve_and_classify_failures() {
    // Library 0x22FFF030: X typed (Text); P typed (Field) with a child K based on X.
    let mut p = typed(P, 0x13, 1);
    p.children = vec![based(0x1000_0500, 1, X, 0x22FF_F030)];
    let lib = parse(&enc_layout(
        0x22FF_F030,
        1024,
        768,
        &[typed(X, 0x10, 0), p, based(T, 2, P, 0x22FF_F030)],
    ))
    .unwrap();
    // Layout 0x22FFF040: E based on T (depth 2: T, then P), and broken references.
    let user = parse(&enc_layout(
        0x22FF_F040,
        1024,
        768,
        &[
            based(E, 0, T, 0x22FF_F030),
            based(0x1000_0601, 1, 0x1000_0999, 0x22FF_F030),
            based(0x1000_0602, 2, X, 0x22FF_FFFF),
            based(0x1000_0603, 3, 0x1000_0604, 0x22FF_F040),
            based(0x1000_0604, 4, 0x1000_0603, 0x22FF_F040),
        ],
    ))
    .unwrap();
    let layouts = [lib, user];
    let user = &layouts[1];

    let chain = base_chain(user.top_level(E).unwrap(), lookup(&layouts)).unwrap();
    assert_eq!(chain.depth(), 2);
    assert_eq!(chain.end().unwrap().element_id, P);
    assert_eq!(
        chain.links[0].base,
        BaseRef {
            element: T,
            layout: Did(0x22FF_F030)
        }
    );
    // A typed element has an empty chain; the based child K of P reaches X.
    let lib = &layouts[0];
    assert_eq!(
        base_chain(lib.top_level(X).unwrap(), lookup(&layouts))
            .unwrap()
            .depth(),
        0
    );
    let k = &lib.top_level(P).unwrap().children[0];
    assert_eq!(
        base_chain(k, lookup(&layouts))
            .unwrap()
            .end()
            .unwrap()
            .element_id,
        X
    );

    let err = |id: u32| base_chain(user.top_level(id).unwrap(), lookup(&layouts)).unwrap_err();
    assert!(matches!(
        err(0x1000_0601),
        BaseChainError::ElementMissing(_)
    ));
    assert_eq!(
        err(0x1000_0601).category(),
        FailureCategory::BaseElementMissing
    );
    assert_eq!(
        err(0x1000_0602).category(),
        FailureCategory::BaseLayoutMissing
    );
    assert!(matches!(err(0x1000_0603), BaseChainError::Cycle(_)));
    assert_eq!(err(0x1000_0604).category(), FailureCategory::BaseChainCycle);
}

#[test]
fn long_base_chains_stop_at_the_limit() {
    // Element i is based on element i + 1 of the same layout; the last is typed.
    let n = MAX_BASE_CHAIN as u32 + 2;
    let mut elements: Vec<El> = (0..n)
        .map(|i| based(0x1000_0000 + i, i, 0x1000_0001 + i, 0x2200_0001))
        .collect();
    elements.push(typed(0x1000_0000 + n, 1, n));
    let l = parse(&enc_layout(0x2200_0001, 0, 0, &elements)).unwrap();
    let layouts = [l];
    let start = layouts[0].top_level(0x1000_0000).unwrap();
    let e = base_chain(start, lookup(&layouts)).unwrap_err();
    assert_eq!(e, BaseChainError::TooLong);
    assert_eq!(e.category(), FailureCategory::BaseChainCycle);
    // A chain of exactly the limit resolves.
    let start = layouts[0].top_level(0x1000_0002).unwrap();
    assert_eq!(
        base_chain(start, lookup(&layouts)).unwrap().depth(),
        MAX_BASE_CHAIN
    );
}

#[test]
fn failure_categories_are_named() {
    assert_eq!(FailureCategory::ALL.len(), 8);
    for c in FailureCategory::ALL {
        assert!(!c.name().is_empty());
    }
}

// ---------------------------------------------------------------------------------------
// Labels (§3.3).
// ---------------------------------------------------------------------------------------

fn mapper(did: u32, base: u32, names: &[(u32, &str)]) -> EnumMapper {
    let mut b = B::new().u32(did).u32(base).tsize(0, names.len() as u32);
    for (k, n) in names {
        b = b.u32(*k).pascal(n);
    }
    EnumMapper::parse(&b.tsize(0, 0).build()).unwrap()
}

#[test]
fn labels_follow_the_base_mapper() {
    let engine = mapper(
        0x23FF_0001,
        0,
        &[(0x0B, "normal"), (0x08, "normal_rollover")],
    );
    let game = mapper(0x23FF_0002, 0x23FF_0001, &[(0x1000_0001, "QuestTracked")]);
    let ids = mapper(0x23FF_0003, 0, &[(0x1000_1234, "Example_Button")]);
    let all = [engine, game, ids];
    let emap = |label: &str| match label {
        "UIStateID" => Some(Did(0x23FF_0002)),
        "UIElementID" => Some(Did(0x23FF_0003)),
        "UIElementType" => Some(Did(0x23FF_0077)),
        _ => None,
    };
    let labels = UiLabels::load(emap, |d| all.iter().find(|m| m.did == d).cloned());
    assert_eq!(labels.state(0x0B), Some("normal"));
    assert_eq!(labels.state(0x1000_0001), Some("QuestTracked"));
    assert_eq!(labels.key(UiEnum::StateId, "normal_rollover"), Some(0x08));
    assert_eq!(labels.element(0x1000_1234), Some("Example_Button"));
    assert_eq!(labels.chain(UiEnum::StateId).len(), 2);
    assert!(!labels.is_loaded(UiEnum::ElementType));
    assert!(!labels.is_loaded(UiEnum::DrawMode));
    assert_eq!(labels.element_type(9), None);
    assert_eq!(labels.message(0x1B), None);
    assert_eq!(labels.draw_mode(4), None);
    assert_eq!(
        UiEnum::ALL.map(UiEnum::emapper_label)[4],
        "UIElementMessage"
    );

    // A cyclic base chain terminates.
    let a = mapper(0x23FF_0010, 0x23FF_0011, &[]);
    let b = mapper(0x23FF_0011, 0x23FF_0010, &[]);
    let all = [a, b];
    let labels = UiLabels::load(
        |_| Some(Did(0x23FF_0010)),
        |d| all.iter().find(|m| m.did == d).cloned(),
    );
    assert_eq!(labels.chain(UiEnum::ElementId).len(), 2);
    assert_eq!(labels.element(1), None);
}

// ---------------------------------------------------------------------------------------
// Properties of the decoder on arbitrary and structured input.
// ---------------------------------------------------------------------------------------

fn arb_media() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        (any::<u32>(), 0u32..10, "[a-z/]{0,20}").prop_map(|(d, m, p)| image(d, m, &p)),
        (any::<u32>(),).prop_map(|(d,)| item(11).u32(d).build()),
        (any::<f32>(), any::<f32>()).prop_map(|(a, b)| item(13).f32(a).f32(b).build()),
        proptest::collection::vec(any::<u32>(), 0..5).prop_map(|frames| {
            let mut b = item(8).f32(1.0).u32(4).u8(1).u32(frames.len() as u32);
            for f in frames {
                b = b.u32(f);
            }
            b.build()
        }),
    ]
}

fn arb_element(depth: u32) -> BoxedStrategy<El> {
    let leaf = (
        1u32..0x2000_0000,
        0u32..20,
        proptest::collection::vec(arb_media(), 0..3),
        proptest::collection::vec(arb_media(), 0..3),
        any::<[i32; 4]>(),
        any::<i32>(),
        0u8..4,
    )
        .prop_map(|(id, ty, media, state_media, rect, z, flags)| {
            let mut e = typed(id, ty, 0);
            e.blk.media = media;
            e.blk.rect = rect;
            e.blk.z = z;
            e.blk.flags = flags;
            e.states = vec![Blk {
                id: 0x0B,
                media: state_media,
                ..Blk::default()
            }];
            e
        });
    if depth == 0 {
        return leaf.boxed();
    }
    (
        leaf,
        proptest::collection::vec(arb_element(depth - 1), 0..3),
    )
        .prop_map(|(mut e, mut children)| {
            for (i, c) in children.iter_mut().enumerate() {
                c.id = 0x1000_0000 + i as u32;
                c.read_order = i as u32 + 1;
            }
            e.children = children;
            e
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse(&data);
        let s = schema();
        let _ = UiLayout::parse(&data, &s);
        let mut prefixed = 0x2200_0001u32.to_le_bytes().to_vec();
        prefixed.extend_from_slice(&data);
        if let Ok(l) = parse(&prefixed) {
            let _ = l.issues(16);
            let _ = l.census();
        }
    }

    #[test]
    fn structured_records_decode_exactly(elements in proptest::collection::vec(arb_element(2), 0..4)) {
        let mut elements = elements;
        for (i, e) in elements.iter_mut().enumerate() {
            e.id = 0x1000_0100 + i as u32;
            e.read_order = i as u32;
        }
        let data = enc_layout(0x2200_0042, 1024, 768, &elements);
        let l = parse(&data).unwrap();
        prop_assert_eq!(l.elements.len(), elements.len());
        let mut expected = 0usize;
        fn count(e: &El, n: &mut usize) { *n += 1; for c in &e.children { count(c, n); } }
        for e in &elements { count(e, &mut expected); }
        prop_assert_eq!(l.element_count(), expected);
        let c = l.census();
        prop_assert_eq!(c.elements as usize, expected);
        prop_assert_eq!(c.states as usize, expected);
        for (got, want) in l.elements.iter().zip(&elements) {
            prop_assert_eq!(got.block.media.len(), want.blk.media.len());
            prop_assert_eq!(got.block.z_level, want.blk.z);
            prop_assert_eq!(got.children.len(), want.children.len());
        }
        for n in [0, data.len() / 2, data.len().saturating_sub(1)] {
            if n < data.len() {
                prop_assert!(parse(&data[..n]).is_err());
            }
        }
    }
}
