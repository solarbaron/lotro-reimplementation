// SPDX-License-Identifier: GPL-3.0-or-later
//! Distant-landscape sector decoder (`docs/specs/distant-landscape.md`) on synthetic
//! payloads: the worked examples of §8, the layout rules of §3 and §4, and the hostile-input
//! rules of §3.11. Every fixture is built here; none comes from game data.

use tessera_formats::LandblockData;
use tessera_formats::landblock::sector_normals::{
    NeighbourWindow, NormalSurface, rule_normal, rule_normal_bytes,
};
use tessera_formats::landblock::{
    ALL_LANDBLOCKS_MASK, Ceiling, FailureCategory, FarPlacements, HeightGrid, InstanceId, LodRange,
    SECTOR_NEAR_RADIUS, SECTOR_TERRAIN_SAMPLES, SectorChunk, SectorEntity, SectorMesh,
    SectorRecord, SectorSample, SectorSprite, SectorWaterPair, StaticEntityType, chunk_index,
    height_from_raw, imposter_table_did, sector_radius, sprite_fade_parameter, sprite_is_mirrored,
    sprite_is_near, vertex_block,
};
use tessera_formats::{Did, FormatError, NoSchema, Position, PropertyType, PropertyValue};

/// §8.1: region 1, X 0, Y 9.
const DID: u32 = 0x8051_0009;
const COLOUR: Did = Did(0x4000_1000);
const TILE: Did = Did(0x4100_1000);

/// A mesh over the landblocks of `mask` (bit x + 3·y), blocks packed by rank in
/// vertex-block order, chunk entries and bounds following §4.5.
fn mesh(mask: u32, h: impl Fn(usize, usize, usize, usize) -> u16) -> SectorMesh {
    let mut m = SectorMesh::default();
    let mut rank = 0u32;
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for x in 0..3 {
        for y in 0..3 {
            if mask & (1 << chunk_index(x, y)) == 0 {
                continue;
            }
            let start = m.samples.len();
            for j in 0..33 {
                for i in 0..33 {
                    m.samples.push(SectorSample {
                        height: h(x, y, i, j),
                        normal: [127, 255, 127],
                    });
                }
            }
            let (a, b) = SectorMesh::raw_height_range(&m.samples[start..]).unwrap();
            let c = SectorChunk::expected(rank, x, y, height_from_raw(a), height_from_raw(b));
            for k in 0..3 {
                lo[k] = lo[k].min(c.min[k]);
                hi[k] = hi[k].max(c.max[k]);
            }
            m.chunks[chunk_index(x, y)] = c;
            rank += 1;
        }
    }
    if rank > 0 {
        m.bounds_min = lo;
        m.bounds_max = hi;
    }
    m
}

/// The smallest record of §8.1 (no ceiling, water, sprites or entities), with a terrain
/// whose heights depend on position.
fn smallest(did: u32) -> SectorRecord {
    let id = tessera_formats::LandblockId::from_did(Did(did)).unwrap();
    SectorRecord {
        did: Did(did),
        terrain: mesh(ALL_LANDBLOCKS_MASK, |x, y, i, j| {
            (1000 * x + 300 * y + 7 * i + 3 * j) as u16
        }),
        mode: 0,
        ceiling_mask: 0,
        ceiling: None,
        block_size: 160.0,
        water: Vec::new(),
        sprites: Vec::new(),
        entities: Vec::new(),
        colour_list: COLOUR,
        ceiling_colour_list: Did(0),
        tile: TILE,
        ceiling_tile: Did(0),
        flow_map: Did(0),
        imposter_table: imposter_table_did(id.region, id.x, id.y),
    }
}

fn parse(bytes: &[u8]) -> Result<SectorRecord, FormatError> {
    SectorRecord::parse(bytes, &NoSchema)
}

// ---------------------------------------------------------------- §8 worked examples

#[test]
fn smallest_record_layout() {
    let rec = smallest(DID);
    assert_eq!(rec.imposter_table, Did(0x2701_0000));
    let bytes = rec.to_bytes().unwrap();
    assert_eq!(bytes.len(), 49_673);
    assert_eq!(&bytes[0..4], &DID.to_le_bytes());
    assert_eq!(&bytes[4..8], &[0x49, 0x26, 0, 0]);
    assert_eq!(&bytes[49_013..49_017], &[0, 0, 0, 0], "index count");
    assert_eq!(bytes[49_635], 0, "mode");
    assert_eq!(&bytes[49_636..49_640], &[0, 0, 0x20, 0x43], "block_size");
    assert_eq!(bytes[49_640], 0, "water count");
    assert_eq!(&bytes[49_641..49_649], &[0; 8], "sprite and entity counts");
    assert_eq!(&bytes[49_669..], &[0, 0, 1, 0x27], "imposter table");

    let back = parse(&bytes).unwrap();
    assert_eq!(back, rec);
    assert_eq!(back.deviations(), Vec::<&str>::new());
    assert_eq!(back.key(), Some((0, 3)));
    assert_eq!(back.terrain.samples.len(), SECTOR_TERRAIN_SAMPLES as usize);
    assert_eq!(
        SectorRecord::parse_expecting(&bytes, DID, &NoSchema).unwrap(),
        rec
    );
    assert!(matches!(
        SectorRecord::parse_expecting(&bytes, DID + 1, &NoSchema),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn chunk_entry_example() {
    // §8.2: chunk c = 1 (landblock x = 1, y = 0), lowest stored height 3,277 and highest
    // 4,915 (100.00763 and 149.99619 by §4.2), dry.
    let mut rec = smallest(DID);
    rec.terrain = mesh(ALL_LANDBLOCKS_MASK, |x, y, i, j| match (x, y, i, j) {
        (1, 0, 0, 0) => 3_277,
        (1, 0, 32, 32) => 4_915,
        (1, 0, _, _) => 4_000,
        _ => (1000 * x + 300 * y + 7 * i + 3 * j) as u16,
    });
    let c = rec.terrain.chunks[1];
    assert_eq!(
        c,
        SectorChunk::expected(3, 1, 0, height_from_raw(3_277), height_from_raw(4_915))
    );
    let bytes = rec.to_bytes().unwrap();
    let at = 49_017 + 66;
    let want: [u8; 66] = [
        0xC3, 0x0C, 0, 0, 0x41, 0x04, 0, 0, //
        0, 0, 0x20, 0x43, 0, 0, 0, 0, 0xE8, 0x03, 0xC8, 0x42, //
        0, 0, 0xA0, 0x43, 0, 0, 0x20, 0x43, 0x06, 0xFF, 0x15, 0x43, //
        0, 0, //
        0xA0, 0x5F, 0, 0, 0, 0x08, 0, 0, //
        0xA0, 0x77, 0, 0, 0, 0x02, 0, 0, //
        0xA0, 0x7D, 0, 0, 0x80, 0, 0, 0, //
        0x20, 0x7F, 0, 0, 0x20, 0, 0, 0,
    ];
    assert_eq!(&bytes[at..at + 66], &want);
    let back = parse(&bytes).unwrap();
    let got = back.terrain.chunk(1, 0).unwrap();
    assert_eq!(got.vertex_start, 3_267);
    assert_eq!(got.vertex_count, 1_089);
    assert_eq!(got.min, [160.0, 0.0, 100.00763]);
    assert_eq!(got.max, [320.0, 160.0, 149.99619]);
    assert_eq!(
        got.lods,
        [
            LodRange {
                first_index: 24_480,
                triangles: 2_048
            },
            LodRange {
                first_index: 30_624,
                triangles: 512
            },
            LodRange {
                first_index: 32_160,
                triangles: 128
            },
            LodRange {
                first_index: 32_544,
                triangles: 32
            },
        ]
    );
    // The example's heights are now reachable: the record follows every invariant.
    assert!(back.deviations().is_empty(), "{:?}", back.deviations());
}

#[test]
fn stored_indices_are_skipped() {
    // §3.2 Tessera rule: the m × 2 index bytes are skipped; the generated list is used.
    let b = bytes_with(|b| {
        b[49_013..49_017].copy_from_slice(&2u32.to_le_bytes());
        b.splice(49_017..49_017, [1, 0, 2, 0]);
    });
    let rec = parse(&b).unwrap();
    assert_eq!(rec.terrain.stored_index_count, 2);
    assert!(
        rec.deviations()
            .contains(&"DL-I3: terrain index count not 0")
    );
    // The encoder writes zeros for skipped bytes and keeps the count.
    let again = rec.to_bytes().unwrap();
    assert_eq!(&again[49_013..49_021], &[2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(parse(&again).unwrap(), rec);
}

/// `water.md` §6.2 step 6.
fn level(l: u8) -> f32 {
    (f32::from(l) * 1000.0) * (1.0f32 / 255.0)
}

#[test]
fn water_example() {
    // §8.3: (0,0) and (1,0) type 3 at level 38, (0,2) type 5 at level 86.
    let mut rec = smallest(DID);
    rec.water = vec![
        SectorWaterPair {
            water_type: 3,
            height: level(38),
        },
        SectorWaterPair {
            water_type: 5,
            height: level(86),
        },
    ];
    for (x, y, w) in [(0, 0, 0), (1, 0, 0), (0, 2, 1)] {
        let c = &mut rec.terrain.chunks[chunk_index(x, y)];
        c.wet = true;
        c.water_index = w;
    }
    let bytes = rec.to_bytes().unwrap();
    assert_eq!(
        &bytes[49_640..49_651],
        &[2, 3, 0x06, 0x05, 0x15, 0x43, 5, 0xA1, 0xA0, 0xA8, 0x43]
    );
    let back = parse(&bytes).unwrap();
    assert_eq!(back.water_at(0, 0).unwrap().water_type, 3);
    assert_eq!(back.water_at(1, 0).unwrap().water_type, 3);
    assert_eq!(back.water_at(0, 2).unwrap().water_type, 5);
    assert!(back.water_at(2, 2).is_none());
    assert!(back.water_list_follows_rules());
    assert!(back.deviations().is_empty(), "{:?}", back.deviations());

    // Chunk order (c = x + 3·y) would list (0,2)'s pair second as well here; swapping the
    // pairs breaks first-use order over vertex-block order.
    let mut swapped = back.clone();
    swapped.water.swap(0, 1);
    for c in &mut swapped.terrain.chunks {
        if c.wet {
            c.water_index ^= 1;
        }
    }
    assert!(!swapped.water_list_follows_rules());
    // An unused pair breaks it too.
    let mut unused = back.clone();
    unused.water.push(SectorWaterPair {
        water_type: 9,
        height: 1.0,
    });
    assert!(!unused.water_list_follows_rules());
}

#[test]
fn sprite_and_entity_example() {
    // §8.4.
    let did = 0x8051_03F0;
    let mut rec = smallest(did);
    assert_eq!(rec.imposter_table, Did(0x2701_00D8));
    let sprite = SectorSprite {
        name_key: 600,
        east: 100.0,
        height: 250.0,
        north: 300.0,
        scale: 1.0,
        distance: 140.0,
        byte: 0x40,
    };
    let entity = SectorEntity {
        entity_type: 7,
        object: Did(0x4700_1234),
        iid: InstanceId::new(1, 1, 3, 0xF0, 5),
        scale: [1.0; 3],
        position: Position::new(0xC0)
            .with_region(1)
            .with_landblock(3, 0xF0)
            .with_cell(0)
            .with_offset([10.0, 20.0, 300.0]),
        property: None,
        view_distance_sq: 1_000_000.0,
    };
    rec.sprites.push(sprite);
    rec.entities.push(entity.clone());
    let bytes = rec.to_bytes().unwrap();
    let at = 49_641;
    assert_eq!(&bytes[at..at + 4], &[1, 0, 0, 0]);
    let sprite_bytes: [u8; 23] = [
        0x58, 0x02, 0, 0, 0xC8, 0x42, 0, 0, 0x7A, 0x43, 0, 0, 0x96, 0x43, 0, 0, 0x80, 0x3F, 0, 0,
        0x0C, 0x43, 0x40,
    ];
    assert_eq!(&bytes[at + 4..at + 27], &sprite_bytes);
    assert_eq!(&bytes[at + 27..at + 31], &[1, 0, 0, 0]);
    let entity_bytes: Vec<u8> = [
        &[7u8, 0, 0, 0][..],
        &[0x34, 0x12, 0, 0x47],
        &[5, 0, 0, 0, 0, 0x3F, 0x10, 0x01],
        &[0, 0, 0x80, 0x3F, 0, 0, 0x80, 0x3F, 0, 0, 0x80, 0x3F],
        &[
            0xDB, 1, 3, 0xF0, 0, 0, 0, 0, 0x20, 0x41, 0, 0, 0xA0, 0x41, 0, 0, 0x96, 0x43,
        ],
        &[0, 0, 0, 0],
        &[0, 0x24, 0x74, 0x49],
    ]
    .concat();
    assert_eq!(entity_bytes.len(), 54);
    assert_eq!(&bytes[at + 31..at + 31 + 54], &entity_bytes[..]);

    let back = parse(&bytes).unwrap();
    let s = back.sprites[0];
    assert_eq!(s, sprite);
    assert_eq!(s.position(), [100.0, 300.0, 250.0]);
    let e = &back.entities[0];
    assert_eq!(e, &entity);
    assert_eq!(e.kind(), StaticEntityType::Physics);
    assert_eq!(e.iid.landblock(), (3, 0xF0));
    assert_eq!(e.position.flags(), 0xDB);
    assert_eq!(e.view_distance(), 1_000.0);
    assert!(back.deviations().is_empty(), "{:?}", back.deviations());

    // An entity of another landblock, indoors or nearer than 250 breaks DL-I12.
    for broken in [
        SectorEntity {
            iid: InstanceId::new(1, 1, 6, 0xF0, 5),
            ..entity.clone()
        },
        SectorEntity {
            position: entity.position.with_cell(0x100),
            ..entity.clone()
        },
        SectorEntity {
            view_distance_sq: 100.0,
            ..entity.clone()
        },
    ] {
        let mut r = back.clone();
        r.entities = vec![broken];
        assert!(r.first_unusual_field().unwrap().starts_with("DL-I12"));
    }
}

#[test]
fn entity_property_needs_the_schema() {
    // One entity with a list-form property PID 0x4CE (typed Int here), then a null one.
    let mut rec = smallest(0x8051_03F0);
    let entity = SectorEntity {
        entity_type: 9,
        object: Did(0x4700_0001),
        iid: InstanceId::new(1, 1, 4, 0xF1, 9),
        scale: [2.0; 3],
        position: Position::new(0)
            .with_region(1)
            .with_landblock(4, 0xF1)
            .with_cell(0),
        property: None,
        view_distance_sq: 90_000.0,
    };
    rec.entities = vec![entity.clone(), entity];
    let mut bytes = rec.to_bytes().unwrap();
    // Splice a non-null property into the first entity: PID, then an i32.
    let first = 49_649;
    let prop_at = first + 4 + 4 + 8 + 12 + 1 + 1 + 2 + 2;
    assert_eq!(&bytes[prop_at..prop_at + 4], &[0; 4]);
    bytes.splice(prop_at..prop_at + 4, [0xCE, 0x04, 0, 0, 0x2A, 0, 0, 0]);
    let schema = |pid: u32| (pid == 0x4CE).then_some(PropertyType::Int);
    let rec = SectorRecord::parse(&bytes, &schema).unwrap();
    let p = rec.entities[0].property.as_ref().unwrap();
    assert_eq!(p.pid, 0x4CE);
    assert_eq!(p.value, PropertyValue::Int(42));
    assert!(rec.entities[1].property.is_none());
    assert_eq!(rec.entities[0].kind(), StaticEntityType::Light);
    // The encoder writes null properties only.
    assert!(matches!(
        rec.to_bytes(),
        Err(FormatError::Unsupported { .. })
    ));
    // Without the PID in the schema the record fails, as the client's reader does.
    let err = parse(&bytes).unwrap_err();
    assert!(matches!(
        err.root_cause(),
        FormatError::UnknownProperty { pid: 0x4CE, .. }
    ));
    assert_eq!(FailureCategory::of(&err), FailureCategory::PropertySet);
}

// ---------------------------------------------------------------- §4 meaning

#[test]
fn sample_order_and_normals() {
    let rec = smallest(DID);
    // Block k = 3·x + y, sample (i, j) at 33·j + i.
    let s = rec.terrain.sample(2, 1, 5, 7).unwrap();
    assert_eq!(s.height, (2000 + 300 + 35 + 21) as u16);
    let k = vertex_block(2, 1);
    assert_eq!(rec.terrain.samples[1089 * k + 33 * 7 + 5], s);
    assert!(rec.terrain.sample(0, 0, 33, 0).is_none());
    assert!(rec.terrain.sample(3, 0, 0, 0).is_none());
    // §4.3: (127, 255, 127) decodes to (−0.0039, −0.0039, 1.0) in (x, y, z).
    let n = s.normal_xyz();
    assert!((n[0] + 0.003_921_6).abs() < 1e-6, "{n:?}");
    assert!((n[1] + 0.003_921_6).abs() < 1e-6, "{n:?}");
    assert_eq!(n[2], 1.0);
    let tilted = SectorSample {
        height: 0,
        normal: [255, 128, 0],
    }
    .normal_xyz();
    assert_eq!(tilted[0], 1.0);
    assert_eq!(tilted[1], -1.0);
    assert!(tilted[2].abs() < 0.01);
    // Heights convert as landblock.md §3.4.
    assert_eq!(
        SectorSample {
            height: 65_535,
            normal: [0; 3]
        }
        .height_units(),
        2000.0
    );
}

#[test]
fn ceiling_mode_two_packs_by_rank() {
    // Ceilings in (0,1) (bit 3, k = 1) and (2,0) (bit 2, k = 6): ranks 0 and 1.
    let mask = (1 << chunk_index(0, 1)) | (1 << chunk_index(2, 0));
    let mut rec = smallest(DID);
    rec.mode = 2;
    rec.ceiling_mask = mask;
    rec.ceiling = Some(mesh(mask, |x, y, i, _| {
        (40_000 + 100 * x + 10 * y + i) as u16
    }));
    rec.ceiling_colour_list = Did(0x4000_2000);
    rec.ceiling_tile = Did(0x4100_2000);
    let bytes = rec.to_bytes().unwrap();
    let back = parse(&bytes).unwrap();
    assert_eq!(back, rec);
    assert!(back.deviations().is_empty(), "{:?}", back.deviations());
    let c = back.ceiling.as_ref().unwrap();
    assert_eq!(c.samples.len(), 2 * 1089);
    assert_eq!(c.chunk(0, 1).unwrap().vertex_start, 0);
    assert_eq!(c.chunk(2, 0).unwrap().vertex_start, 1089);
    assert_eq!(c.sample(2, 0, 4, 0).unwrap().height, 40_204);
    assert!(c.chunk(1, 1).unwrap().is_zero());
    assert!(c.block(1, 1).is_none());
    assert!(back.has_ceiling(0, 1) && back.has_ceiling(2, 0) && !back.has_ceiling(1, 1));

    // Mode 1: mask 0x1FF without a stored mask.
    let mut all = smallest(DID);
    all.mode = 1;
    all.ceiling_mask = ALL_LANDBLOCKS_MASK;
    all.ceiling = Some(mesh(ALL_LANDBLOCKS_MASK, |_, _, _, _| 50_000));
    all.ceiling_colour_list = Did(0x4000_2000);
    all.ceiling_tile = Did(0x4100_2000);
    let back = parse(&all.to_bytes().unwrap()).unwrap();
    assert_eq!(back.ceiling_mask, ALL_LANDBLOCKS_MASK);
    assert!(back.deviations().is_empty(), "{:?}", back.deviations());

    // Ceiling DIDs without a ceiling break DL-I13.
    let mut odd = smallest(DID);
    odd.ceiling_tile = Did(0x4100_2000);
    assert!(odd.first_unusual_field().unwrap().starts_with("DL-I13"));
}

// ---------------------------------------------------------------- §3.11 hostile input

fn bytes_with(f: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut b = smallest(DID).to_bytes().unwrap();
    f(&mut b);
    b
}

#[test]
fn rejects_malformed_records() {
    // Mode 3.
    let b = bytes_with(|b| b[49_635] = 3);
    assert!(matches!(parse(&b), Err(FormatError::Malformed { .. })));
    // Mode 2 with a mask above bit 8.
    let b = bytes_with(|b| {
        b[49_635] = 2;
        b.splice(49_636..49_636, 0x200u32.to_le_bytes());
    });
    assert!(matches!(parse(&b), Err(FormatError::Malformed { .. })));
    // Mode 2, mask 1, but no ceiling block: the sample count read is block_size's bytes.
    let b = bytes_with(|b| {
        b[49_635] = 2;
        b.splice(49_636..49_636, 1u32.to_le_bytes());
    });
    assert!(parse(&b).is_err());
    // Terrain sample count other than 9,801.
    let b = bytes_with(|b| {
        b[4..8].copy_from_slice(&9_800u32.to_le_bytes());
        b.drain(8..13);
    });
    assert!(matches!(parse(&b), Err(FormatError::CountMismatch { .. })));
    // A huge sample, index, sprite or entity count fails before allocating.
    for at in [4usize, 49_013, 49_641, 49_645] {
        let b = bytes_with(|b| b[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes()));
        assert!(
            matches!(parse(&b), Err(FormatError::Truncated { .. })),
            "count at {at}"
        );
    }
    // A huge VLE water count.
    let b = bytes_with(|b| {
        b.splice(49_640..49_641, [0xE0, 0xFF, 0xFF, 0xFF, 0x0F]);
    });
    assert!(matches!(parse(&b), Err(FormatError::Truncated { .. })));
    // Chunk vertex count neither 0 nor 1,089.
    let b = bytes_with(|b| b[49_017 + 4..49_017 + 8].copy_from_slice(&5u32.to_le_bytes()));
    assert!(matches!(parse(&b), Err(FormatError::Malformed { .. })));
    // Chunk window past the samples.
    let b = bytes_with(|b| b[49_017..49_017 + 4].copy_from_slice(&9_000u32.to_le_bytes()));
    assert!(matches!(
        parse(&b),
        Err(FormatError::IndexOutOfRange { .. })
    ));
    // `wet` above 1.
    let b = bytes_with(|b| b[49_017 + 32] = 2);
    let err = parse(&b).unwrap_err();
    assert!(matches!(err, FormatError::InvalidBool { value: 2, .. }));
    assert_eq!(FailureCategory::of(&err), FailureCategory::BadFlag);
    // Wet with a water index past the (empty) list.
    let b = bytes_with(|b| b[49_017 + 32] = 1);
    assert!(matches!(
        parse(&b),
        Err(FormatError::IndexOutOfRange { .. })
    ));
    // Trailing bytes.
    let b = bytes_with(|b| b.push(0));
    assert!(matches!(parse(&b), Err(FormatError::TrailingBytes { .. })));
    // Another landblock kind.
    let b = bytes_with(|b| b[0..4].copy_from_slice(&0x8001_0009u32.to_le_bytes()));
    assert!(matches!(
        parse(&b),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn every_truncation_fails_cleanly() {
    let mut rec = smallest(DID);
    rec.mode = 2;
    rec.ceiling_mask = 1;
    rec.ceiling = Some(mesh(1, |_, _, _, _| 7));
    rec.ceiling_colour_list = Did(0x4000_2000);
    rec.ceiling_tile = Did(0x4100_2000);
    rec.water.push(SectorWaterPair {
        water_type: 1,
        height: 5.0,
    });
    rec.terrain.chunks[0].wet = true;
    rec.sprites.push(SectorSprite::default());
    rec.entities.push(SectorEntity {
        entity_type: 7,
        object: Did(0x4700_0001),
        iid: InstanceId(0),
        scale: [1.0; 3],
        position: Position::new(0x3F),
        property: None,
        view_distance_sq: 1.0,
    });
    let bytes = rec.to_bytes().unwrap();
    assert_eq!(parse(&bytes).unwrap(), rec);
    let cut_points = (0..bytes.len())
        .step_by(97)
        .chain(bytes.len() - 200..bytes.len());
    for n in cut_points {
        assert!(parse(&bytes[..n]).is_err(), "prefix {n}");
    }
}

// ---------------------------------------------------------------- §4.8, §5.1, §5.6, §4.9

#[test]
fn sprite_heading_size_fade_and_mirror() {
    let sprite = SectorSprite {
        name_key: 600,
        east: 100.0,
        height: 250.0,
        north: 300.0,
        scale: 2.0,
        distance: 140.0,
        byte: 0x40,
    };
    // §4.8: byte × 360/256.
    assert_eq!(sprite.heading_degrees(), 90.0);
    assert_eq!(SectorSprite { byte: 0, ..sprite }.heading_degrees(), 0.0);
    assert!(
        SectorSprite {
            byte: 255,
            ..sprite
        }
        .heading_degrees()
            < 360.0
    );
    // Size S = value_a × scale, lower edge B = value_b × scale.
    assert_eq!(sprite.billboard_size(3.0, 0.1), (6.0, 0.2));
    // §5.6 step 2: sector corner landblock (3, 6), viewer at (4, 7): offset −160 each way.
    let at = sprite.viewer_frame((3, 6), (4, 7));
    assert_eq!(at, (-60.0, 140.0));
    assert!(sprite_is_near(at.0, at.1));
    assert!(sprite_is_near(-160.0, 320.0));
    assert!(!sprite_is_near(-160.1, 0.0));
    assert!(!sprite_is_near(0.0, 320.5));
    // Near: the stored distance. Far: S × K / M_d with K = (H / φ) / 248.
    assert_eq!(
        sprite_fade_parameter(&sprite, at, 6.0, 1240.0, 1.0, 2.0),
        140.0
    );
    let far = sprite.viewer_frame((0, 0), (4, 7));
    assert!(!sprite_is_near(far.0, far.1));
    let p = sprite_fade_parameter(&sprite, far, 6.0, 1240.0, 1.0, 2.0);
    assert!((p - 6.0 * 5.0 / 2.0).abs() < 1e-4, "{p}");
    // §5.6 step 4: θ = 90°: mirrored exactly when x − 160 > 0 (strictly).
    assert!(sprite_is_mirrored(&sprite, (161.0, 0.0)));
    assert!(!sprite_is_mirrored(&sprite, (159.0, 0.0)));
    // θ = 0: mirrored exactly when y − 160 > 0.
    let north = SectorSprite { byte: 0, ..sprite };
    assert!(sprite_is_mirrored(&north, (0.0, 160.5)));
    assert!(!sprite_is_mirrored(&north, (0.0, 160.0)));
}

#[test]
fn radii_of_5_1_and_5_2() {
    // F = ⌊R / 3⌋ + 1 for the landscape radii 2, 4, 6, 9, 12 of levels 0–4.
    let f: Vec<u32> = [2, 4, 6, 9, 12].into_iter().map(sector_radius).collect();
    assert_eq!(f, vec![1, 2, 3, 4, 5]);
    assert_eq!(SECTOR_NEAR_RADIUS, 1);
}

#[test]
fn far_placements_are_keyed_by_instance_id() {
    let did = 0x8051_03F0;
    let mut rec = smallest(did);
    let entity = SectorEntity {
        entity_type: 7,
        object: Did(0x4700_1234),
        iid: InstanceId::new(1, 1, 3, 0xF0, 5),
        scale: [1.0; 3],
        position: Position::new(0xC0)
            .with_region(1)
            .with_landblock(3, 0xF0)
            .with_cell(0)
            .with_offset([10.0, 20.0, 300.0]),
        property: None,
        view_distance_sq: 1_000_000.0,
    };
    rec.entities.push(entity.clone());
    rec.entities.push(SectorEntity {
        object: Did(0x4700_9999),
        ..entity.clone()
    });
    let mut table = FarPlacements::new();
    assert!(table.is_empty());
    // The second entry repeats the instance ID: skipped, the first stays.
    assert_eq!(table.register(&rec), 1);
    assert_eq!(
        table.register(&rec),
        0,
        "a sector loaded again adds nothing"
    );
    assert_eq!(table.len(), 1);
    assert_eq!(
        table.get(entity.iid.0).map(|e| e.object),
        Some(Did(0x4700_1234))
    );
}

// ---------------------------------------------------------------- §4.3 normals

fn landblock(x: u8, y: u8, h: impl Fn(i32, i32) -> u16, ceiling: Option<u16>) -> LandblockData {
    let (gx, gy) = (i32::from(x) * 32, i32::from(y) * 32);
    let mut d = LandblockData::from_heights(
        Did(0x8001_0000 | (u32::from(x) << 8) | u32::from(y)),
        HeightGrid::from_fn(|i, j| h(gx + i as i32, gy + j as i32)),
    );
    if let Some(c) = ceiling {
        d.ceiling = Some(Ceiling {
            heights: HeightGrid::flat(c),
            ..Ceiling::default()
        });
    }
    d
}

#[test]
fn normal_rule_on_planes() {
    // A plane rising 10 raw steps per sample eastwards, over a 5 × 5 neighbourhood.
    let slope = |gx: i32, _gy: i32| (20_000 + 10 * gx) as u16;
    let blocks: Vec<LandblockData> = (0..5u8)
        .flat_map(|x| (0..5u8).map(move |y| (x, y)))
        .map(|(x, y)| landblock(x, y, slope, Some(30_000)))
        .collect();
    let get = |x: i32, y: i32| -> Option<&LandblockData> {
        let (x, y) = (u8::try_from(x).ok()?, u8::try_from(y).ok()?);
        blocks
            .iter()
            .find(|d| d.did.0 & 0xFFFF == (u32::from(x) << 8 | u32::from(y)))
    };
    let mut w = NeighbourWindow::default();
    for (dy, row) in (-1..=1).zip(w.0.iter_mut()) {
        for (dx, cell) in (-1..=1).zip(row.iter_mut()) {
            *cell = get(2 + dx, 2 + dy);
        }
    }
    // Analytic: normal ∝ (−dz/dx, 0, 1), dz/dx = 10 raw steps per 5 units.
    let dzdx = f64::from(height_from_raw(10_010) - height_from_raw(10_000)) / 5.0;
    let len = (dzdx * dzdx + 1.0).sqrt();
    for (i, j) in [(0, 0), (16, 9), (32, 32), (0, 32)] {
        let n = rule_normal(&w, NormalSurface::Terrain, i, j).unwrap();
        assert!((n[0] + dzdx / len).abs() < 1e-3, "{n:?}");
        assert!(
            n[1].abs() < 1e-6 && (n[2] - 1.0 / len).abs() < 1e-3,
            "{n:?}"
        );
    }
    // A flat ceiling faces down: (127, 0, 127), middle byte ≤ 126.
    assert_eq!(
        rule_normal_bytes(&w, NormalSurface::Ceiling, 5, 5),
        Some([127, 0, 127])
    );
    // A flat terrain sample encodes as (127, 255, 127).
    let flat: Vec<LandblockData> = vec![landblock(0, 0, |_, _| 500, None)];
    let mut fw = NeighbourWindow::default();
    fw.0[1][1] = flat.first();
    assert_eq!(
        rule_normal_bytes(&fw, NormalSurface::Terrain, 0, 0),
        Some([127, 255, 127])
    );
    // No ceiling grid: nothing to compute.
    assert_eq!(rule_normal_bytes(&fw, NormalSurface::Ceiling, 3, 3), None);

    // A sector built from the same plane with the rule's bytes checks exact.
    let mut rec = smallest(0x8051_0303);
    rec.terrain = mesh(ALL_LANDBLOCKS_MASK, |x, y, i, j| {
        slope(
            32 * (1 + x as i32) + i as i32,
            32 * (1 + y as i32) + j as i32,
        )
    });
    for x in 0..3usize {
        for y in 0..3usize {
            let mut w = NeighbourWindow::default();
            for (dy, row) in (-1..=1).zip(w.0.iter_mut()) {
                for (dx, cell) in (-1..=1).zip(row.iter_mut()) {
                    *cell = get(1 + x as i32 + dx, 1 + y as i32 + dy);
                }
            }
            let start = rec.terrain.chunk(x, y).unwrap().vertex_start as usize;
            for j in 0..33 {
                for i in 0..33 {
                    let k = start + 33 * j + i;
                    rec.terrain.samples[k].normal =
                        rule_normal_bytes(&w, NormalSurface::Terrain, i, j).unwrap();
                }
            }
        }
    }
    let check = rec.check_normals(|x, y| get(x - 2, y - 2));
    assert_eq!(check.samples(), 9 * 1089);
    assert_eq!(check.terrain.exact, check.terrain.samples, "{check:?}");
    assert!(check.holds());
    // One step off in one component is still within DL-I14; two steps is not.
    rec.terrain.samples[40].normal[0] += 1;
    let c = rec.check_normals(|x, y| get(x - 2, y - 2)).terrain;
    assert_eq!((c.within_one, c.beyond), (1, 0));
    rec.terrain.samples[40].normal[0] += 1;
    let c = rec.check_normals(|x, y| get(x - 2, y - 2));
    assert_eq!(c.terrain.beyond, 1);
    assert!(!c.holds());
    // A ceiling in landblock (0, 0) of the sector: mode 2, mask 1, the flat ceiling of the
    // neighbourhood, normals facing down.
    rec.terrain.samples[40].normal[0] -= 2;
    rec.mode = 2;
    rec.ceiling_mask = 1;
    let mut ceiling = mesh(1, |_, _, _, _| 30_000);
    for s in &mut ceiling.samples {
        s.normal = [127, 0, 127];
    }
    rec.ceiling = Some(ceiling);
    let c = rec.check_normals(|x, y| get(x - 2, y - 2));
    assert_eq!(c.ceiling.samples, 1089);
    assert_eq!(c.ceiling.exact, 1089, "{c:?}");
    assert!(c.holds());
    // An up-facing stored ceiling normal breaks DL-I14.
    if let Some(m) = rec.ceiling.as_mut() {
        m.samples[0].normal = [127, 200, 127];
    }
    let c = rec.check_normals(|x, y| get(x - 2, y - 2));
    assert_eq!(c.ceiling.ceiling_up, 1);
    assert!(!c.holds());
}

#[test]
fn normal_rule_uses_the_neighbours_flag_at_index_0() {
    // §4.3 step 3: a quad at index 32 reads the neighbour's flag byte at index 0, not the
    // block's own byte at 32. A bumpy surface makes the diagonal matter.
    let bumpy = |gx: i32, gy: i32| (20_000 + ((gx * 7919 + gy * 104_729) % 97) * 40) as u16;
    let own = landblock(1, 1, bumpy, None);
    let east = landblock(2, 1, bumpy, None);
    let north = landblock(1, 2, bumpy, None);
    let ne = landblock(2, 2, bumpy, None);
    let window = |own: &'_ LandblockData, east: &'_ LandblockData| {
        let mut w = NeighbourWindow::default();
        w.0[1][1] = Some(own);
        w.0[1][2] = Some(east);
        w.0[2][1] = Some(&north);
        w.0[2][2] = Some(&ne);
        rule_normal_bytes(&w, NormalSurface::Terrain, 32, 16)
    };
    let base = window(&own, &east);
    // Changing the block's own byte at (32, 16) changes nothing.
    let mut own2 = own.clone();
    own2.vertex_flags.raw_mut()[33 * 32 + 16] ^= 0x80;
    assert_eq!(window(&own2, &east), base);
    // Changing the east neighbour's byte at (0, 16) changes the result.
    let mut east2 = east.clone();
    east2.vertex_flags.raw_mut()[16] ^= 0x80;
    assert_ne!(window(&own, &east2), base);
    // Without the east neighbour the triangles through it are left out, the rest remain.
    let mut w = NeighbourWindow::default();
    w.0[1][1] = Some(&own);
    assert!(rule_normal_bytes(&w, NormalSurface::Terrain, 32, 16).is_some());
}
