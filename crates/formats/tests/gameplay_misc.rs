// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the miscellaneous record decoders (`tessera_formats::gameplay::misc`)
//! from the worked examples and invariants of `docs/specs/misc-records.md`. Every byte
//! string is made up here; none comes from game data.

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use common::B;
use tessera_formats::gameplay::common::Pid;
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::misc::*;
use tessera_formats::{Did, FormatError, PropertyType, PropertyValue, Record, RgbaImage};

// ---------------------------------------------------------------------------------------------
// §17.1 compositor
// ---------------------------------------------------------------------------------------------

fn spec_compositor_bytes() -> Vec<u8> {
    B::new()
        .u32(0x3300_0099)
        .u32(2)
        // Layer 0x10000001, one option.
        .u32(0x1000_0001)
        .u32(1)
        .u32(1)
        .u32(0x41AA_AA01)
        .u32(0x41BB_BB01)
        .u32(0)
        .u32(0)
        .u32(1)
        .u32(0x1000_0003)
        .u32(0x41AA_AA02)
        .u32(0x41BB_BB02)
        // Layer 0x10000002, one option at (16, 32), no colours.
        .u32(0x1000_0002)
        .u32(1)
        .u32(1)
        .u32(0x41AA_AA03)
        .u32(0x41BB_BB03)
        .u32(16)
        .u32(32)
        .u32(0)
        .u8(0)
        .build()
}

#[test]
fn compositor_worked_example_17_1() {
    let bytes = spec_compositor_bytes();
    assert_eq!(bytes.len(), 85);
    let c = ImageCompositor::parse(&bytes).unwrap();
    assert_eq!(c.did, Did(0x3300_0099));
    assert_eq!(c.layers.len(), 2);
    assert_eq!(c.layers[0].layer_key, 0x1000_0001);
    let o = &c.layers[0].options[0];
    assert_eq!(
        (o.index, o.legacy_did, o.image_did, o.x, o.y),
        (1, Did(0x41AA_AA01), Did(0x41BB_BB01), 0, 0)
    );
    assert_eq!(
        o.colours,
        vec![ColourEntry {
            colour_key: 0x1000_0003,
            legacy_did: Did(0x41AA_AA02),
            mask_did: Did(0x41BB_BB02),
        }]
    );
    let o = &c.layers[1].options[0];
    assert_eq!(
        (o.index, o.image_did, o.x, o.y),
        (1, Did(0x41BB_BB03), 16, 32)
    );
    assert!(o.colours.is_empty());
    assert!(!c.compress_with_alpha);
    assert_eq!(
        c.dependencies(),
        vec![Did(0x41BB_BB01), Did(0x41BB_BB02), Did(0x41BB_BB03)]
    );
    assert_eq!(c.stored_len(), 85);
    assert_eq!(c.to_bytes(), bytes);
    assert_eq!(c.first_unusual_field(), None);
    assert_eq!((c.option_count(), c.colour_entry_count()), (2, 1));
    assert!(ImageCompositor::parse_expecting(&bytes, 0x3300_0099u32).is_ok());
    assert!(matches!(
        ImageCompositor::parse_expecting(&bytes, 0x3300_0098u32),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn compositor_rejects_malformed_input() {
    let bytes = spec_compositor_bytes();
    for n in 0..bytes.len() {
        assert!(ImageCompositor::parse(&bytes[..n]).is_err(), "prefix {n}");
    }
    // Trailing boolean must be 0 or 1 (MR1).
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() = 2;
    assert!(matches!(
        ImageCompositor::parse(&bad),
        Err(FormatError::InvalidBool { .. })
    ));
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        ImageCompositor::parse(&trailing),
        Err(FormatError::TrailingBytes { .. })
    ));
    let mut wrong = bytes.clone();
    wrong[3] = 0x34;
    assert!(matches!(
        ImageCompositor::parse(&wrong),
        Err(FormatError::WrongRecordType { .. })
    ));
    // A count larger than the bytes left fails before allocating.
    let huge = B::new().u32(0x3300_0001).u32(u32::MAX).build();
    assert!(matches!(
        ImageCompositor::parse(&huge),
        Err(FormatError::Truncated { .. })
    ));
    // Readers accept any key and legacy DID (MR2 is an observation, not a check).
    let dup = B::new()
        .u32(0x3300_0001)
        .u32(2)
        .u32(7)
        .u32(0)
        .u32(7)
        .u32(0)
        .u8(1)
        .build();
    let c = ImageCompositor::parse(&dup).unwrap();
    assert!(c.compress_with_alpha);
    assert!(c.first_unusual_field().is_some());
}

// ---------------------------------------------------------------------------------------------
// §17.2 composition arithmetic and §3.6 composition
// ---------------------------------------------------------------------------------------------

#[test]
fn arithmetic_worked_example_17_2() {
    let c = colour_bytes([0.5, 0.25, 1.0]);
    assert_eq!(c, [127, 63, 255]);
    let d = masked_multiply([200, 150, 100, 255], c, Some(128));
    assert_eq!(d, [150, 94, 100, 255]);
    let o = opacity_byte(0.5);
    assert_eq!(o, 127);
    assert_eq!(mix(d, [10, 20, 30, 200], o), [95, 65, 73, 255]);
    // Edge cases of the table.
    // Mask byte 0: k = inc(0) = 0, unchanged; mask 255: k = 256, full multiply.
    assert_eq!(
        masked_multiply([9, 9, 9, 9], [0, 0, 0], Some(0)),
        [9, 9, 9, 9]
    );
    assert_eq!(
        masked_multiply([9, 9, 9, 9], [0, 0, 0], Some(255)),
        [0, 0, 0, 9]
    );
    assert_eq!(
        masked_multiply([200, 200, 200, 7], [0, 0, 0], None),
        [0, 0, 0, 7]
    );
    assert_eq!(mix([1, 2, 3, 4], [9, 9, 9, 255], 0), [1, 2, 3, 4]);
    assert_eq!(mix([1, 2, 3, 4], [9, 9, 9, 0], 255), [1, 2, 3, 4]);
    assert_eq!(mix([1, 2, 3, 4], [9, 9, 9, 255], 255), [9, 9, 9, 4]);
}

fn img(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
    RgbaImage {
        width: w,
        height: h,
        pixels: px.repeat((w * h) as usize),
    }
}

/// A two-layer compositor: base 0x41000001 (1 colour entry with mask 0x41000002) and an
/// overlay 0x41000003 at (`ox`, `oy`).
fn two_layer(ox: u32, oy: u32) -> ImageCompositor {
    ImageCompositor {
        did: Did(0x3300_0001),
        layers: vec![
            CompositorLayer {
                layer_key: 0x1000_0001,
                options: vec![CompositorOption {
                    index: 1,
                    legacy_did: Did(0x4100_AAAA),
                    image_did: Did(0x4100_0001),
                    x: 0,
                    y: 0,
                    colours: vec![ColourEntry {
                        colour_key: 0x1000_0003,
                        legacy_did: Did(0x4100_AAAB),
                        mask_did: Did(0x4100_0002),
                    }],
                }],
            },
            CompositorLayer {
                layer_key: 0x1000_0002,
                options: vec![CompositorOption {
                    index: 1,
                    legacy_did: Did(0x4100_AAAC),
                    image_did: Did(0x4100_0003),
                    x: ox,
                    y: oy,
                    colours: vec![],
                }],
            },
        ],
        compress_with_alpha: false,
    }
}

#[test]
fn composition_reproduces_17_2() {
    let c = two_layer(0, 0);
    let images: HashMap<Did, RgbaImage> = [
        (Did(0x4100_0001), img(1, 1, [200, 150, 100, 255])),
        (Did(0x4100_0002), img(1, 1, [255, 255, 255, 128])),
        (Did(0x4100_0003), img(1, 1, [10, 20, 30, 200])),
    ]
    .into();
    let mut inst = CompositorInstance::new(&c);
    assert_eq!(inst.selection, vec![LayerSelection::default(); 2]);
    assert!(inst.set_layer(&c, 0x1000_0002, 1, 0.5));
    assert!(!inst.set_layer(&c, 0x1000_0009, 1, 0.5));
    inst.set_colour(0x1000_0003, [0.5, 0.25, 1.0, 1.0]);
    let out = inst
        .compose(&c, |d| images.get(&d))
        .unwrap()
        .expect("an image");
    assert_eq!((out.width, out.height), (1, 1));
    assert_eq!(out.pixels, vec![95, 65, 73, 255]);

    // The images composition needs.
    let mut need = c.required_images(&inst.selection);
    need.sort();
    assert_eq!(
        need,
        vec![Did(0x4100_0001), Did(0x4100_0002), Did(0x4100_0003)]
    );

    // White (absent key) leaves the base untouched; the overlay at the default
    // selection (0) is skipped.
    let plain = c
        .compose(&c.default_selection(), |_| None, |d| images.get(&d))
        .unwrap()
        .unwrap();
    assert_eq!(plain.pixels, vec![200, 150, 100, 255]);

    // A mask of another size is skipped; a missing mask multiplies the whole image.
    let mut odd = images.clone();
    odd.insert(Did(0x4100_0002), img(2, 1, [0, 0, 0, 0]));
    let colours = |_| Some([0.0, 0.0, 0.0, 1.0]);
    let sel = c.default_selection();
    let o = c.compose(&sel, colours, |d| odd.get(&d)).unwrap().unwrap();
    assert_eq!(o.pixels, vec![200, 150, 100, 255]);
    let mut nomask = images.clone();
    nomask.remove(&Did(0x4100_0002));
    let o = c
        .compose(&sel, colours, |d| nomask.get(&d))
        .unwrap()
        .unwrap();
    assert_eq!(o.pixels, vec![0, 0, 0, 255]);
}

#[test]
fn composition_preconditions() {
    let c = two_layer(0, 0);
    let images: HashMap<Did, RgbaImage> = [
        (Did(0x4100_0001), img(2, 2, [1, 2, 3, 255])),
        (Did(0x4100_0003), img(1, 1, [9, 9, 9, 255])),
    ]
    .into();
    let get = |d: Did| images.get(&d);
    let sel = c.preview_selection();
    // P2.
    assert_eq!(
        c.compose(&sel[..1], |_| None, get),
        Err(CompositionFailure::SelectionLength {
            expected: 2,
            found: 1
        })
    );
    // P3: the overlay's image is not loaded.
    let base_only: HashMap<Did, RgbaImage> = [(Did(0x4100_0001), img(2, 2, [1, 2, 3, 255]))].into();
    assert_eq!(
        c.compose(&sel, |_| None, |d| base_only.get(&d)),
        Err(CompositionFailure::ImageNotLoaded { layer: 1 })
    );
    // ... but a selection with no matching option passes and is skipped.
    let miss = [
        sel[0],
        LayerSelection {
            option: 7,
            opacity: 1.0,
        },
    ];
    assert!(c.compose(&miss, |_| None, |d| base_only.get(&d)).is_ok());
    // P1.
    let mut p1 = c.clone();
    p1.layers[1].options[0].image_did = Did(0);
    assert_eq!(
        p1.compose(&sel, |_| None, get),
        Err(CompositionFailure::MissingImageDid { layer: 1 })
    );
    // P4: base option not at (0, 0).
    let mut p4 = c.clone();
    p4.layers[0].options[0].x = 1;
    assert_eq!(
        p4.compose(&sel, |_| None, get),
        Err(CompositionFailure::BaseLayerMismatch)
    );
    // P4: base images of different sizes.
    let mut p4b = c.clone();
    let mut second = p4b.layers[0].options[0].clone();
    second.index = 2;
    second.image_did = Did(0x4100_0005);
    p4b.layers[0].options.push(second);
    let mut sized = images.clone();
    sized.insert(Did(0x4100_0005), img(3, 3, [0; 4]));
    assert_eq!(
        p4b.compose(&sel, |_| None, |d| sized.get(&d)),
        Err(CompositionFailure::BaseLayerMismatch)
    );
    // No base image loaded.
    let none: HashMap<Did, RgbaImage> = HashMap::new();
    assert_eq!(
        c.compose(&c.default_selection(), |_| None, |d| none.get(&d)),
        Err(CompositionFailure::NoBaseImage)
    );
    // A record without layers composes to "no image".
    let empty = ImageCompositor {
        did: Did(0x3300_0002),
        layers: vec![],
        compress_with_alpha: false,
    };
    assert_eq!(empty.compose(&[], |_| None, get), Ok(None));
    // An image whose buffer does not match its size counts as not loaded.
    let mut broken = images.clone();
    broken.insert(
        Did(0x4100_0001),
        RgbaImage {
            width: 4,
            height: 4,
            pixels: vec![0; 3],
        },
    );
    assert_eq!(
        c.compose(&c.default_selection(), |_| None, |d| broken.get(&d)),
        Err(CompositionFailure::NoBaseImage)
    );
}

#[test]
fn composition_opacity_and_placement() {
    // 4 × 4 base, 2 × 3 overlay placed at (3, 2): clipped at the right and bottom.
    let c = two_layer(3, 2);
    let images: HashMap<Did, RgbaImage> = [
        (Did(0x4100_0001), img(4, 4, [0, 0, 0, 255])),
        (Did(0x4100_0003), img(2, 3, [100, 100, 100, 255])),
    ]
    .into();
    let get = |d: Did| images.get(&d);
    let mut sel = c.preview_selection();
    let out = c.compose(&sel, |_| None, get).unwrap().unwrap();
    let at = |x: usize, y: usize| out.pixels[(y * 4 + x) * 4];
    let changed: Vec<(usize, usize)> = (0..4)
        .flat_map(|y| (0..4).map(move |x| (x, y)))
        .filter(|&(x, y)| at(x, y) == 100)
        .collect();
    assert_eq!(changed, vec![(3, 2), (3, 3)]);

    // NaN, negative and tiny opacities skip the layer; 1.004 wraps to byte 0.
    for op in [f32::NAN, -1.0, 0.0002, 1.004, f32::INFINITY] {
        sel[1].opacity = op;
        let out = c.compose(&sel, |_| None, get).unwrap().unwrap();
        assert!(out.pixels.chunks(4).all(|p| p[0] == 0), "opacity {op}");
    }
    // Skipping layer 0 leaves a transparent black canvas.
    sel[0].opacity = 0.0;
    sel[1].opacity = 1.0;
    let out = c.compose(&sel, |_| None, get).unwrap().unwrap();
    assert_eq!(&out.pixels[..4], &[0, 0, 0, 0]);
    assert_eq!(out.pixels[(2 * 4 + 3) * 4..][..4], [100, 100, 100, 0]);
}

#[test]
fn instance_colour_bytes_and_selection() {
    let c = two_layer(0, 0);
    let mut inst = CompositorInstance::new(&c);
    // Stored order B, G, R, A.
    inst.set_colour_bytes(5, [10, 20, 30, 255]);
    let [r, g, b, a] = inst.colours[&5];
    assert_eq!(colour_bytes([r, g, b]), [30, 20, 10]);
    assert!((a - 1.0).abs() < 1e-6);
    assert_eq!(BYTE_TO_UNIT.to_bits(), 0x3B80_8081);
}

#[test]
fn compositor_control_struct() {
    let pids = CompositorControlPids {
        control: Pid::with_id("c", 1),
        file: Pid::with_id("f", 2),
        layer_name: Pid::with_id("l", 3),
        control_property: Pid::with_id("p", 4),
        min_value: Pid::with_id("m", 5),
    };
    let s = set(vec![entry(
        9,
        array(vec![entry(
            1,
            structure(vec![
                entry(2, did(0x3300_0001)),
                entry(3, PropertyValue::Enum(0x1000_0004)),
                entry(4, PropertyValue::PropertyId(77)),
                entry(5, PropertyValue::Int(1)),
            ]),
        )]),
    )]);
    let found = CompositorControl::find_all(&s, &pids);
    assert_eq!(
        found,
        vec![CompositorControl {
            file: Some(Did(0x3300_0001)),
            layer_key: Some(0x1000_0004),
            control_property: Some(77),
            min_value: Some(1),
        }]
    );
    // X3: fields of another value type read as absent.
    let wrong = set(vec![entry(
        1,
        structure(vec![
            entry(3, PropertyValue::Int(4)),
            entry(4, PropertyValue::Enum(77)),
            entry(5, PropertyValue::Enum(1)),
        ]),
    )]);
    let found = CompositorControl::find_all(&wrong, &pids);
    assert_eq!(
        found,
        vec![CompositorControl {
            file: None,
            layer_key: None,
            control_property: None,
            min_value: None,
        }]
    );
}

// ---------------------------------------------------------------------------------------------
// §17.3 imposter table
// ---------------------------------------------------------------------------------------------

fn spec_imposter_bytes() -> Vec<u8> {
    B::new()
        .u32(0x2701_001B)
        .tsize(1, 2)
        .u32(5)
        .u8(0)
        .u8(0)
        .f32(1.0)
        .f32(0.0)
        .u32(30)
        .u8(1)
        .u8(0)
        .f32(10.0)
        .f32(-1.0)
        .u32(0x4000_0010)
        .u32(0x4000_0011)
        .f32(0.484_375)
        .f32(0.968_75)
        .build()
}

#[test]
fn imposter_worked_example_17_3() {
    let bytes = spec_imposter_bytes();
    assert_eq!(bytes.len(), 50);
    let t = ImposterTable::parse(&bytes).unwrap();
    assert_eq!(
        t.square(),
        Some(ImposterSquare {
            region: 1,
            x: 0,
            y: 27
        })
    );
    assert_eq!(t.size.hint, 1);
    assert_eq!(t.entries.len(), 2);
    assert_eq!(
        (t.entries[1].name_key, t.entries[1].column, t.entries[1].row),
        (30, 1, 0)
    );
    assert_eq!((t.entries[1].value_a, t.entries[1].value_b), (10.0, -1.0));
    assert_eq!(t.is_bucket_ordered(), Some(true));
    assert_eq!(t.grid(), Some((2, 1)));
    assert_eq!(t.atlas_texels(), Some((256, 128)));
    assert_eq!(t.expected_cell_size(), Some((0.484_375, 0.968_75)));
    assert_eq!(t.cell_u.to_bits(), 0x3EF8_0000);
    assert_eq!(t.cell_v.to_bits(), 0x3F78_0000);
    assert_eq!(t.cell_uv(1, 0), Some([0.5, 0.0, 0.984_375, 0.968_75]));
    assert_eq!(t.dependencies(), vec![Did(0x4000_0010), Did(0x4000_0011)]);
    assert_eq!(t.first_unusual_field(), None);
    assert_eq!(t.to_bytes(), bytes);
    for n in 0..bytes.len() {
        assert!(ImposterTable::parse(&bytes[..n]).is_err());
    }
}

#[test]
fn imposter_hint_rule_and_geometry() {
    // MR12: hint 22 accepted, 23 rejected.
    let table = |hint: u8| {
        B::new()
            .u32(0x2701_0000)
            .tsize(hint, 0)
            .u32(0)
            .u32(0)
            .f32(0.0)
            .f32(0.0)
            .build()
    };
    let t = ImposterTable::parse(&table(22)).unwrap();
    assert_eq!(t.is_bucket_ordered(), None);
    assert!(matches!(
        ImposterTable::parse(&table(23)),
        Err(FormatError::Unsupported { .. })
    ));
    assert_eq!(t.grid(), None);
    assert_eq!(t.cell_uv(0, 0), None);

    assert_eq!(imposter_atlas_grid(19), Some((16, 2)));
    assert_eq!(imposter_atlas_grid(1), Some((1, 1)));
    assert_eq!(imposter_atlas_grid(16), Some((16, 1)));
    assert_eq!(imposter_atlas_grid(17), Some((16, 2)));
    assert_eq!(imposter_atlas_grid(128), Some((16, 8)));
    assert_eq!(imposter_atlas_grid(u32::MAX), Some((16, 1 << 28)));
    assert_eq!(imposter_atlas_grid(0), None);
    let margin = 31.0_f32 / 32.0;
    assert_eq!(margin / 16.0, 0.060_546_875);
    assert_eq!(margin / 2.0, 0.484_375);

    let sq = ImposterSquare::containing(1, 30, 60);
    assert_eq!((sq.x, sq.y), (27, 54));
    assert_eq!(sq.did(), Did(0x2701_1B36));
    assert_eq!(ImposterSquare::from_did(sq.did()), Some(sq));
    assert_eq!(ImposterSquare::containing(14, 255, 0).x, 243);
    assert!(sq.is_aligned());
    assert_eq!(ImposterSquare::from_did(Did(0x2801_0000)), None);
}

// ---------------------------------------------------------------------------------------------
// §6 compute shaders
// ---------------------------------------------------------------------------------------------

fn dxbc_blob() -> Vec<u8> {
    // Header 32 bytes + 2 offsets, then two chunks: SHEX (4 bytes: compute 5.0) and STAT.
    let header_len = 32 + 8;
    let shex_at = header_len;
    let stat_at = shex_at + 8 + 4;
    let total = stat_at + 8 + 2;
    B::new()
        .bytes(b"DXBC")
        .bytes(&[0xAB; 16])
        .u32(1)
        .u32(total as u32)
        .u32(2)
        .u32(shex_at as u32)
        .u32(stat_at as u32)
        .bytes(b"SHEX")
        .u32(4)
        .u32(0x0005_0050)
        .bytes(b"STAT")
        .u32(2)
        .u16(0)
        .build()
}

#[test]
fn compute_shader_blob() {
    let blob = dxbc_blob();
    let rec = B::new()
        .u32(0x2D00_0001)
        .u32(blob.len() as u32)
        .bytes(&blob)
        .build();
    let cs = ComputeShader::parse(&rec).unwrap();
    assert_eq!(cs.blob, blob);
    assert!(cs.is_consistent());
    let c = cs.container().unwrap();
    assert_eq!(c.version, 1);
    assert_eq!(c.chunks.len(), 2);
    assert_eq!(&c.chunks[0].fourcc, b"SHEX");
    assert_eq!(c.program, Some((5, 5, 0)));
    // Size larger than the bytes left, and leftover bytes.
    let short = B::new().u32(0x2D00_0001).u32(1000).bytes(&blob).build();
    assert!(ComputeShader::parse(&short).is_err());
    let long = B::new().u32(0x2D00_0001).u32(1).bytes(&blob).build();
    assert!(matches!(
        ComputeShader::parse(&long),
        Err(FormatError::TrailingBytes { .. })
    ));
    // Not DXBC: the record decodes, the container does not.
    let other = B::new().u32(0x2D00_0001).u32(4).bytes(b"XXXX").build();
    let cs = ComputeShader::parse(&other).unwrap();
    assert!(cs.container().is_err());
    assert!(!cs.is_consistent());
    // A chunk offset outside the blob.
    let mut bad = blob.clone();
    bad[36..40].copy_from_slice(&9999u32.to_le_bytes());
    assert!(DxbcContainer::parse(&bad).is_err());
    // Three table entries sharing one empty chunk: fewer bytes follow the table than the
    // table has entries times four, which is still a valid container.
    let shared = B::new()
        .bytes(b"DXBC")
        .bytes(&[0; 16])
        .u32(1)
        .u32(52)
        .u32(3)
        .u32(44)
        .u32(44)
        .u32(44)
        .bytes(b"STAT")
        .u32(0)
        .build();
    assert_eq!(shared.len(), 52);
    let c = DxbcContainer::parse(&shared).unwrap();
    assert_eq!(c.chunks.len(), 3);
    assert!(c.chunks.iter().all(|k| k.offset == 44 && k.size == 0));
}

// ---------------------------------------------------------------------------------------------
// §17.4 script channels
// ---------------------------------------------------------------------------------------------

#[test]
fn script_channels_worked_example_17_4() {
    let bytes = [
        0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0xAA, 0x0A, 0x00, 0x10, 0x02, 0x00, 0x00,
        0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0xAA, 0x0A, 0x00, 0x10, 0xBB, 0x0B, 0x00, 0x10,
        0x01, 0x00,
    ];
    assert_eq!(bytes.len(), 32);
    let sc = ScriptChannels::parse(&bytes).unwrap();
    assert_eq!(sc.channels.len(), 1);
    let ch = sc.channel(1).unwrap();
    assert_eq!(
        ch.scripts,
        vec![ChannelScript {
            script_key: 0x1000_0AAA,
            priority: 2,
            interruptible: true
        }]
    );
    assert_eq!(
        ch.transitions,
        vec![ChannelTransition {
            from_script: 0,
            to_script: 0x1000_0AAA,
            run_script: 0x1000_0BBB
        }]
    );
    assert!(ch.exclusive);
    assert!(sc.global_transitions.is_empty());
    assert_eq!(sc.script_keys(), BTreeSet::from([0x1000_0AAA, 0x1000_0BBB]));
    assert_eq!(sc.to_bytes(), bytes);
    for n in 0..bytes.len() {
        assert!(ScriptChannels::parse(&bytes[..n]).is_err());
    }
    let mut bad = bytes;
    bad[16] = 2; // interruptible
    assert!(matches!(
        ScriptChannels::parse(&bad),
        Err(FormatError::InvalidBool { .. })
    ));
    let mut hint = bytes;
    hint[0] = 23;
    assert!(ScriptChannels::parse(&hint).is_err());
}

// ---------------------------------------------------------------------------------------------
// §9, §10, §11 singleton tables
// ---------------------------------------------------------------------------------------------

#[test]
fn name_filter_table() {
    let rule = |b: B, key: u32, flag_a: u8| {
        b.u32(key)
            .u32(1)
            .u32(2)
            .u32(3)
            .u32(4)
            .u32(5)
            .u32(6)
            .u32(7)
            .u32(2)
            .utf16("ab")
            .utf16("c")
            .utf16("xyz")
            .utf16("")
            .tsize(1, 2)
            .u32(1)
            .utf16("abc")
            .u32(1)
            .utf16("aa")
            .u32(2)
            .utf16("")
            .u32(0)
            .u8(flag_a)
            .u32(2)
            .u32(2)
            .u32(0x1000_0001)
            .u8(0)
    };
    let bytes = rule(rule(B::new().u32(0x0E00_0001).tsize(1, 2), 1, 1), 2, 0).build();
    let t = NameFilterTable::parse(&bytes).unwrap();
    assert_eq!(t.rule_sets.len(), 2);
    let r = &t.rule_sets[0];
    assert_eq!(r.numbers, [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(r.string_list_a.len(), 2);
    assert_eq!(r.string_a.len(), 3);
    assert!(r.string_b.is_empty());
    assert_eq!(r.character_classes.len(), 2);
    assert_eq!(r.character_classes[0].strings.len(), 1);
    assert_eq!(r.u32_list, vec![2, 0x1000_0001]);
    assert!(r.flag_a && !t.rule_sets[1].flag_a && !r.flag_b);
    for n in 0..bytes.len() {
        assert!(NameFilterTable::parse(&bytes[..n]).is_err());
    }
}

#[test]
fn taboo_table() {
    let bytes = B::new()
        .u32(0x0E00_0002)
        .u32(2)
        .pascal("one.txt")
        .u32(4)
        .u32(0x7FFF_FFFF)
        .pascal("two.txt")
        .u32(1)
        .u32(4999)
        .tsize(1, 2)
        .pascal("two.txt")
        .u32(1)
        .utf16("x.*")
        .pascal("one.txt")
        .u32(2)
        .utf16("\\ba")
        .utf16("b")
        .build();
    let t = TabooTable::parse(&bytes).unwrap();
    assert_eq!(t.headers.len(), 2);
    assert_eq!((t.headers[1].a, t.headers[1].b), (1, 4999));
    assert!(t.keys_match_headers());
    assert_eq!(t.pattern_count(), 3);
    assert_eq!(t.list("one.txt").unwrap().patterns.len(), 2);
    let mut other = t.clone();
    other.lists[0].name = "three.txt".into();
    assert!(!other.keys_match_headers());
    for n in 0..bytes.len() {
        assert!(TabooTable::parse(&bytes[..n]).is_err());
    }
    // A huge header count fails on the bytes left.
    let huge = B::new().u32(0x0E00_0002).u32(u32::MAX).build();
    assert!(matches!(
        TabooTable::parse(&huge),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn compression_dictionary_envelope() {
    let rec = |m: u32, body: usize| {
        B::new()
            .bytes(&[1; 16])
            .bytes(&[2; 16])
            .u32(0)
            .u32(1)
            .u32(m)
            .bytes(&vec![0; body])
            .build()
    };
    let e = CompressionDictionaryEnvelope::parse(&rec(5, 5)).unwrap();
    assert_eq!((e.field_36, e.body_len), (1, 5));
    assert_eq!(rec(0, 0).len(), COMPRESSION_DICTIONARY_HEADER_LEN);
    assert!(CompressionDictionaryEnvelope::parse(&rec(0, 0)).is_ok());
    assert!(CompressionDictionaryEnvelope::parse(&rec(5, 4)).is_err());
    assert!(CompressionDictionaryEnvelope::parse(&rec(5, 6)).is_err());
}

#[test]
fn singleton_labels() {
    assert_eq!(
        SingletonKind::from_unique_db_label("TabooTable"),
        Some(SingletonKind::TabooTable)
    );
    assert_eq!(
        SingletonKind::from_unique_db_label("ComputeShader_WaterSimulation"),
        Some(SingletonKind::ComputeShader)
    );
    assert_eq!(SingletonKind::from_unique_db_label("Other"), None);
    assert!(is_face_compositor_label("Av_Race_Sex_Face_Compositor"));
    assert!(!is_face_compositor_label("Av__Face_Compositor"));
    assert!(!is_face_compositor_label("Face_Compositor"));
}

// ---------------------------------------------------------------------------------------------
// §4 UI scenes
// ---------------------------------------------------------------------------------------------

#[test]
fn ui_scene() {
    let schema = |pid: u32| (pid == 5).then_some(PropertyType::Int);
    let sed = |b: B, ty: u32| {
        b.u32(0x19A) // type, object, placement, properties, name
            .u32(ty)
            .u32(0x4700_0001)
            .f32(1.0)
            .f32(2.0)
            .f32(3.0)
            .f32(1.0)
            .f32(0.0)
            .f32(0.0)
            .f32(0.0)
            .tsize(0, 1)
            .u32(5)
            .u32(5)
            .u32(42)
            .pascal("cam")
    };
    let bytes = sed(sed(B::new().u32(0x2100_0001).u32(2), 7), 27)
        .u32(0)
        .u8(0)
        .u32(0xBF80_0000)
        .u32(0xBF80_0000)
        .build();
    let s = UiScene::parse(&bytes, &schema).unwrap();
    assert_eq!(s.entities.len(), 2);
    assert_eq!(
        s.first_camera().unwrap().entity_type,
        Some(UI_SCENE_CAMERA_TYPE)
    );
    assert_eq!(s.objects().count(), 2);
    assert_eq!(f32::from_bits(s.value_a), -1.0);
    assert!(!s.flag);
    assert!(UiScene::parse_expecting(&bytes, 0x2100_0001u32, &schema).is_ok());
    for n in 0..bytes.len() {
        assert!(UiScene::parse(&bytes[..n], &schema).is_err());
    }
    // MR4: a flag bit ≥ 0x800 is rejected.
    let bad = B::new()
        .u32(0x2100_0001)
        .u32(1)
        .u32(0x800)
        .u32(0)
        .u8(0)
        .u32(0)
        .u32(0)
        .build();
    assert!(UiScene::parse(&bad, &schema).is_err());
}

// ---------------------------------------------------------------------------------------------
// §12 bare sets and §13 metadata views
// ---------------------------------------------------------------------------------------------

fn names(pairs: &[(&str, u32)]) -> BTreeMap<String, u32> {
    pairs.iter().map(|&(l, p)| (l.to_owned(), p)).collect()
}

#[test]
fn bare_set_families() {
    let n = names(&[
        ("UI_Map_GameMap", 1),
        ("UI_Map_MapImage", 2),
        ("UI_Map_Scale", 3),
        ("UI_Map_PixelOffsetX", 4),
        ("Environment_LocalSky", 10),
        ("Ambient_MusicType", 11),
        ("World_IsNight", 12),
        ("Volume_IsLiquid", 13),
        ("SND_Footstep_Surfaces", 14),
        ("Landblock_LightMap", 15),
    ]);
    let c = BareSetClassifier {
        pids: BareSetPids::resolve(&n),
        path_map: BTreeSet::from([20]),
    };
    let map = set(vec![
        entry(
            1,
            structure(vec![
                entry(2, did(0x4100_0001)),
                entry(3, PropertyValue::Float(0.5)),
                entry(4, PropertyValue::Int(-3)),
            ]),
        ),
        entry(15, did(0x4100_0002)),
    ]);
    assert_eq!(c.classify(&map), BareSetFamily::GameMap);
    let v = GameMapView::from_set(&map, &c.pids).unwrap();
    assert_eq!(v.image, Some(Did(0x4100_0001)));
    assert_eq!(v.scale, Some(0.5));
    assert_eq!(v.pixel_offset, [Some(-3), None]);
    assert_eq!(v.fog_of_war, None);
    assert!(v.has_light_map);
    assert_eq!(c.classify(&set(vec![])), BareSetFamily::Empty);
    let one = |pid: u32| set(vec![entry(pid, PropertyValue::Bool(true))]);
    assert_eq!(c.classify(&one(10)), BareSetFamily::LocalSky);
    assert_eq!(c.classify(&one(14)), BareSetFamily::FootstepSounds);
    assert_eq!(c.classify(&one(11)), BareSetFamily::AmbientMusic);
    assert_eq!(c.classify(&one(12)), BareSetFamily::ContentLayerDefaults);
    assert_eq!(c.classify(&one(13)), BareSetFamily::Volume);
    assert_eq!(c.classify(&one(20)), BareSetFamily::Volume);
    assert_eq!(c.classify(&one(99)), BareSetFamily::Other);
    assert!(GameMapView::from_set(&one(99), &c.pids).is_none());
}

fn metadata_pids() -> MetadataPids {
    // Assign PIDs 1.. in declaration order.
    let mut by_label = BTreeMap::new();
    for (i, l) in MetadataPids::LABELS.iter().enumerate() {
        by_label.insert((*l).to_owned(), i as u32 + 1);
    }
    MetadataPids::resolve(&by_label)
}

fn id(p: Pid) -> u32 {
    p.id().unwrap()
}

#[test]
fn metadata_views() {
    let p = metadata_pids();
    let c = MetadataClassifier {
        pids: p,
        version: BTreeSet::from([id(p.version_number), 900]),
        // 901: a non-referrer `PropertyResponse*` label (a callback setting).
        response: BTreeSet::from([id(p.response_applied_effects), id(p.response_filters), 901]),
    };
    let pmd = set(vec![
        entry(id(p.pmd_property), PropertyValue::PropertyId(1234)),
        entry(id(p.pmd_display_as_percentage), PropertyValue::Bool(true)),
        entry(id(p.pmd_auction_category), PropertyValue::Enum(3)),
        entry(
            id(p.pmd_allowed_operator_list),
            array(vec![
                entry(77, PropertyValue::Enum(1)),
                entry(77, PropertyValue::Enum(2)),
            ]),
        ),
        entry(
            id(p.pmd_examination_mod),
            structure(vec![entry(
                id(p.pmd_examination_mod_op),
                PropertyValue::Enum(0),
            )]),
        ),
    ]);
    assert_eq!(c.classify(&pmd), Some(MetadataFamily::PropertyMetaData));
    let v = PropertyMetaDataView::from_set(&pmd, &p).unwrap();
    assert_eq!(v.property, 1234);
    assert!(v.display_as_percentage);
    assert_eq!(v.auction_category, Some(3));
    assert_eq!(v.allowed_operators, vec![1, 2]);
    assert_eq!(v.examination_op, Some(0));
    assert!(matches!(
        PropertyMetaDataView::from_set(&set(vec![]), &p),
        Err(FormatError::MissingProperty { .. })
    ));
    let mut index = PropertyMetaDataIndex::new();
    index.insert(1234, Did(0x7000_0001));
    index.insert(1234, Did(0x7000_0002));
    index.insert(5, Did(0x7000_0003));
    assert_eq!(index.get(1234), Some(Did(0x7000_0001)));
    assert_eq!(index.duplicates, vec![(1234, Did(0x7000_0002))]);
    assert_eq!(index.len(), 2);

    let control = set(vec![entry(
        id(p.pmd_list),
        array(vec![entry(1, did(0x7000_0001)), entry(1, did(0))]),
    )]);
    assert_eq!(
        property_metadata_members(&control, &p),
        vec![Did(0x7000_0001)]
    );
    assert_eq!(c.classify(&control), None);

    // Error control.
    let ec = set(vec![
        entry(id(p.error_default_category), did(0x7000_0010)),
        entry(
            id(p.error_category_array),
            array(vec![entry(
                1,
                structure(vec![
                    entry(
                        id(p.error_category_types),
                        array(vec![entry(2, PropertyValue::Enum(44))]),
                    ),
                    entry(id(p.error_category_did), did(0x7000_0011)),
                ]),
            )]),
        ),
    ]);
    let v = ErrorControlView::from_set(&ec, &p);
    assert_eq!(v.category_for(44), Some(Did(0x7000_0011)));
    assert_eq!(v.category_for(45), Some(Did(0x7000_0010)));
    let cat = set(vec![
        entry(id(p.error_brief), PropertyValue::Bool(true)),
        entry(id(p.error_sound), did(0x2A00_0001)),
    ]);
    assert_eq!(c.classify(&cat), Some(MetadataFamily::ErrorCategory));
    assert_eq!(
        ErrorCategoryView::from_set(&cat, &p).sound,
        Some(Did(0x2A00_0001))
    );

    // Tutorial hints.
    let hint = set(vec![
        entry(id(p.hint_type), PropertyValue::Enum(9)),
        entry(
            id(p.hint_list),
            array(vec![entry(
                1,
                structure(vec![entry(id(p.hint_image), did(0x4100_0009))]),
            )]),
        ),
        entry(id(p.hint_auto_show), PropertyValue::Bool(true)),
    ]);
    assert_eq!(c.classify(&hint), Some(MetadataFamily::TutorialHint));
    let v = TutorialHintView::from_set(&hint, &p);
    assert_eq!(v.hint_type, Some(9));
    assert_eq!(v.sections.len(), 1);
    assert_eq!(v.sections[0].image, Some(Did(0x4100_0009)));
    let tc = set(vec![
        entry(
            id(p.hint_control_hints),
            array(vec![entry(1, did(0x7000_0020)), entry(1, did(0x7000_0021))]),
        ),
        entry(
            id(p.hint_control_mp_hints),
            array(vec![entry(1, did(0x7000_0021))]),
        ),
    ]);
    let v = TutorialHintControlView::from_set(&tc, &p);
    assert_eq!(v.all_hints().len(), 2);

    // Movies.
    let movie = set(vec![
        entry(id(p.movie_category), PropertyValue::Enum(4)),
        entry(id(p.movie_sort_priority), PropertyValue::Int(10)),
        entry(id(p.movie_quest), did(0x7000_0030)),
    ]);
    assert_eq!(c.classify(&movie), Some(MetadataFamily::MovieLibrary));
    let v = MovieView::from_set(&movie, &p);
    assert_eq!((v.category, v.sort_priority), (Some(4), Some(10)));
    let mc = set(vec![
        entry(
            id(p.movie_category_list),
            array(vec![entry(1, PropertyValue::Enum(4))]),
        ),
        entry(id(p.movie_list), array(vec![entry(1, did(0x7000_0031))])),
    ]);
    let v = MovieLibraryControlView::from_set(&mc, &p);
    assert_eq!((v.categories.len(), v.movies.len()), (1, 1));
    assert_eq!(c.classify(&mc), None);

    // Callbacks, responses, versions.
    let cb = set(vec![entry(
        id(p.callback_return_value),
        PropertyValue::PropertyId(55),
    )]);
    assert_eq!(c.classify(&cb), Some(MetadataFamily::GameCallback));
    assert_eq!(game_callback_return_value(&cb, &p), Some(55));
    let resp = set(vec![entry(id(p.response_filters), array(vec![]))]);
    assert_eq!(c.classify(&resp), Some(MetadataFamily::PropertyResponse));
    assert_eq!(
        PropertyResponseKind::of(&resp, &p),
        Some(PropertyResponseKind::Filters)
    );
    assert_eq!(c.response_kind(&resp), Some(PropertyResponseKind::Filters));
    // Only a callback setting: a member of kind Other.
    let callback_only = set(vec![entry(901, PropertyValue::Enum(2))]);
    assert_eq!(PropertyResponseKind::of(&callback_only, &p), None);
    assert_eq!(
        c.response_kind(&callback_only),
        Some(PropertyResponseKind::Other)
    );
    assert_eq!(
        c.classify(&callback_only),
        Some(MetadataFamily::PropertyResponse)
    );
    // Only a referrer property: not a member.
    let referrer = set(vec![entry(id(p.response_entity_map_ref), did(0x7000_0050))]);
    assert_eq!(c.response_kind(&referrer), None);
    assert_eq!(c.classify(&referrer), None);
    let ver = set(vec![entry(900, PropertyValue::Int(1))]);
    assert_eq!(c.classify(&ver), Some(MetadataFamily::Version));
    let vs = set(vec![
        entry(
            id(p.version_number_data_list),
            array(vec![entry(
                1,
                structure(vec![
                    entry(id(p.version_number), PropertyValue::Enum(3)),
                    entry(
                        id(p.version_singleton_list),
                        array(vec![entry(1, did(0x7000_0040))]),
                    ),
                ]),
            )]),
        ),
        entry(id(p.version_number), PropertyValue::Enum(3)),
    ]);
    assert_eq!(c.classify(&vs), None);
    assert!(c.is_version_control(&vs));
    assert!(!c.is_version_control(&ver));
    let v = VersionSystemView::from_set(&vs, &p);
    assert_eq!(v.updates, vec![(Some(3), vec![Did(0x7000_0040)])]);
    assert_eq!(v.current, Some(3));
    assert!(v.account_updates.is_empty());

    // §13.7 (X2): one struct carries the account list instead; member candidates are the
    // union of the singleton lists and `Version_AlwaysRunOnPlayers`, each DID once.
    let vs = set(vec![
        entry(
            id(p.version_number_data_list),
            array(vec![
                entry(
                    1,
                    structure(vec![
                        entry(id(p.version_number), PropertyValue::Enum(3)),
                        entry(
                            id(p.version_singleton_list),
                            array(vec![entry(1, did(0x7000_0040)), entry(1, did(0x7000_0041))]),
                        ),
                    ]),
                ),
                entry(
                    1,
                    structure(vec![
                        entry(id(p.version_number), PropertyValue::Enum(4)),
                        entry(
                            id(p.account_version_singleton_list),
                            array(vec![entry(1, did(0x7000_0050))]),
                        ),
                    ]),
                ),
            ]),
        ),
        entry(
            id(p.version_always_run_on_players),
            array(vec![entry(1, did(0x7000_0041)), entry(1, did(0x7000_0060))]),
        ),
    ]);
    assert!(c.is_version_control(&vs));
    let v = VersionSystemView::from_set(&vs, &p);
    assert_eq!(v.updates[1], (Some(4), vec![]));
    assert_eq!(v.account_updates, vec![(Some(4), vec![Did(0x7000_0050)])]);
    assert_eq!(
        v.member_candidates(),
        vec![Did(0x7000_0040), Did(0x7000_0041), Did(0x7000_0060)]
    );
}

// §13.1 (X4): the percentage display of a modifier value.
#[test]
fn modifier_value_percentages() {
    let show = |v: PropertyValue, multiply: bool, flag: bool| {
        modifier_value_display(&v, multiply, flag).expect("numeric")
    };
    let f = PropertyValue::Float;
    // A stored 0.05 with `Add` shows as 5.
    let d = show(f(0.05), false, true);
    assert!(d.percent_template && (d.number - 5.0).abs() < 1e-5 && d.direction.is_none());
    // `Multiply` by 1.05 → 5, increase; by 0.9 → 10, decrease; by exactly 1 → 100, increase.
    let d = show(f(1.05), true, true);
    assert!((d.number - 5.0).abs() < 1e-4);
    assert_eq!(d.direction, Some(ModifierDirection::Increase));
    let d = show(f(0.9), true, true);
    assert!((d.number - 10.0).abs() < 1e-4);
    assert_eq!(d.direction, Some(ModifierDirection::Decrease));
    let d = show(f(1.0), true, true);
    assert_eq!(d.number, 100.0);
    assert_eq!(d.direction, Some(ModifierDirection::Increase));
    // NaN keeps d = v and takes the decrease form.
    let d = show(f(f32::NAN), true, true);
    assert!(d.number.is_nan());
    assert_eq!(d.direction, Some(ModifierDirection::Decrease));
    // The product is formed in single precision, then widened.
    assert_eq!(
        show(f(0.05), false, true).number,
        f64::from(0.05f32 * 100.0)
    );
    // Without the flag a float is unscaled; ints are always unscaled.
    let d = show(f(0.05), true, false);
    assert!(!d.percent_template && d.number == f64::from(0.05f32) && d.direction.is_none());
    let d = show(PropertyValue::Int(7), true, true);
    assert!(d.percent_template && d.number == 7.0 && d.direction.is_none());
    assert_eq!(show(PropertyValue::Int64(-3), false, false).number, -3.0);
    assert!(modifier_value_display(&PropertyValue::Bool(true), false, true).is_none());
}
