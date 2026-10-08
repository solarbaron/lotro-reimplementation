// SPDX-License-Identifier: GPL-3.0-or-later
//! Distant-landscape sector decoder (`docs/specs/distant-landscape.md`) on synthetic
//! payloads: the worked examples of §8, the layout rules of §3 and §4, and the hostile-input
//! rules of §3.11. Every fixture is built here; none comes from game data.

use tessera_formats::landblock::{
    ALL_LANDBLOCKS_MASK, FailureCategory, InstanceId, LodRange, SECTOR_TERRAIN_SAMPLES,
    SectorChunk, SectorEntity, SectorMesh, SectorRecord, SectorSample, SectorSprite,
    SectorWaterPair, StaticEntityType, chunk_index, height_from_raw, imposter_table_did,
    vertex_block,
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
    // §8.2: chunk c = 1 (landblock x = 1, y = 0), heights 100.0–150.0, dry.
    let c = SectorChunk::expected(3, 1, 0, 100.0, 150.0);
    let mut rec = smallest(DID);
    rec.terrain.chunks[1] = c;
    let bytes = rec.to_bytes().unwrap();
    let at = 49_017 + 66;
    let want: [u8; 66] = [
        0xC3, 0x0C, 0, 0, 0x41, 0x04, 0, 0, //
        0, 0, 0x20, 0x43, 0, 0, 0, 0, 0, 0, 0xC8, 0x42, //
        0, 0, 0xA0, 0x43, 0, 0, 0x20, 0x43, 0, 0, 0x16, 0x43, //
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
    assert_eq!(got.min, [160.0, 0.0, 100.0]);
    assert_eq!(got.max, [320.0, 160.0, 150.0]);
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
    // The example's heights are not those of the samples: DL-I5 flags it.
    assert!(
        back.deviations()
            .contains(&"DL-I5: terrain chunk z range differs from its samples")
    );
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
