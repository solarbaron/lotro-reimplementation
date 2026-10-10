// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the material chain (`docs/specs/material.md` §16 fixtures): record
//! decoders, resolution, feature flags, dyes and the library over an in-memory source.
//! Every fixture is generated here.

use std::collections::BTreeMap;
use std::sync::Mutex;

use proptest::prelude::*;
use tessera_formats::{Did, FormatError, Record};
use tessera_material::library::MAX_RESOLVE_MATERIALS;
use tessera_material::records::{
    AddressMode, BlendFactor, CullMode, FilterMode, ParamValue, VALUE_TYPE_COLOR, VALUE_TYPE_CURVE,
    VALUE_TYPE_WORD,
};
use tessera_material::resolve::{
    FALLBACK_ALPHA_THRESHOLD, IgnoreReason, MAX_IGNORED_ENTRIES, ParamOrigin,
};
use tessera_material::vocab::{AlphaUse, ColorUse};
use tessera_material::{
    AppearanceGroup, BlendMode, DyeColors, FnSource, Layering, LibraryConfig, MaterialError,
    MaterialLibrary, MaterialRecord, RecordSource, ShaderInterface, ShaderRecord, SurfaceRecord,
};

// ---------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------

#[derive(Default, Clone)]
struct W(Vec<u8>);

impl W {
    fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32(mut self, v: f32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn bytes(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(b);
        self
    }
}

/// A parameter value as stored.
#[derive(Clone)]
enum V {
    Word(u32),
    Color([f32; 4]),
    Const(f32),
    Wave(u32, [f32; 10]),
    Keyed([f32; 11], u8, Vec<(f32, f32)>),
}

/// One parameter entry (§5.1).
fn entry(id: u32, v: V, bindings: &[(u16, u32, u32)], flag: u8) -> Vec<u8> {
    let group = (id / 1000 * 1000) as u16;
    let mut w = W::default().u32(id).u16(group);
    w = match v {
        V::Word(x) => w.u32(VALUE_TYPE_WORD).u32(x),
        V::Color(c) => c.iter().fold(w.u32(VALUE_TYPE_COLOR), |w, &x| w.f32(x)),
        V::Const(x) => w.u32(VALUE_TYPE_CURVE).u32(1).f32(x),
        V::Wave(mode, p) => p
            .iter()
            .fold(w.u32(VALUE_TYPE_CURVE).u32(mode), |w, &x| w.f32(x)),
        V::Keyed(p, flag, keys) => {
            let w = p
                .iter()
                .fold(w.u32(VALUE_TYPE_CURVE).u32(10), |w, &x| w.f32(x));
            let w = w.u8(flag).u32(keys.len() as u32);
            keys.iter().fold(w, |w, &(a, b)| w.f32(a).f32(b))
        }
    };
    w = w.u32(bindings.len() as u32).u8(flag);
    for &(semantic, perm, slot) in bindings {
        w = w.u16(semantic).u16(group).u32(perm).u32(slot);
    }
    w.0
}

fn surface_record(did: u32, shader: u32, key: u32, materials: &[u32]) -> Vec<u8> {
    let w = W::default()
        .u32(did)
        .u32(shader)
        .u32(key)
        .u32(materials.len() as u32);
    materials.iter().fold(w, |w, &m| w.u32(m)).u16(1).0
}

fn material(did: u32, entries: &[Vec<u8>]) -> Vec<u8> {
    let w = W::default().u32(did).u32(entries.len() as u32);
    entries.iter().fold(w, |w, e| w.bytes(e)).0
}

struct Perm<'a> {
    programs: Option<(&'a [u8], &'a [u8])>,
    samplers: &'a [&'a str],
    extra_type5: bool,
    /// (src, dst, op, depth_write, cull, alpha_ref)
    state: (u8, u8, u8, u8, u8, f32),
}

fn sampler_state(addr: u8, filter: u8, mip: u8) -> [u8; 40] {
    let mut s = [0u8; 40];
    s[8] = addr;
    s[9] = addr;
    s[10] = filter;
    s[11] = filter;
    s[12] = mip;
    s
}

fn permutation(p: &Perm<'_>) -> Vec<u8> {
    let mut w = W::default().u32(1).u32(2).u32(3).u32(4);
    w = match p.programs {
        None => w.u32(0),
        Some((a, b)) => w
            .u32(1)
            .u32(4)
            .u32(a.len() as u32)
            .bytes(a)
            .u32(b.len() as u32)
            .bytes(b),
    };
    w = w.u32(p.samplers.len() as u32);
    for name in p.samplers {
        w = w
            .u8(name.len() as u8)
            .bytes(name.as_bytes())
            .bytes(&sampler_state(1, 2, 2));
    }
    if p.extra_type5 {
        w = w.u32(1).u8(5).u32(0);
        for _ in 0..7 {
            w = w.u32(1).u32(1);
        }
    } else {
        w = w.u32(0);
    }
    let (src, dst, op, dw, cull, aref) = p.state;
    let mut rs = [0u8; 39];
    rs[..7].copy_from_slice(&[src, dst, op, 7, dw, cull, 0]);
    rs[7..11].copy_from_slice(&1u32.to_le_bytes());
    rs[11..15].copy_from_slice(&aref.to_le_bytes());
    w.bytes(&rs).0
}

fn shader(did: u32, decls: &[Vec<u8>], perms: &[Perm<'_>]) -> Vec<u8> {
    let mut w = W::default().u32(did).u32(0).u32(decls.len() as u32);
    for d in decls {
        w = w.bytes(d);
    }
    w = w.u32(perms.len() as u32);
    for p in perms {
        w = w.bytes(&permutation(p));
    }
    w.0
}

const LIT_BASE: (u8, u8, u8, u8, u8, f32) = (5, 6, 1, 1, 2, 0.8);
const LIT_LIGHT: (u8, u8, u8, u8, u8, f32) = (5, 2, 1, 0, 2, 0.8);

/// The §16 shader: 3000 with two texture bindings, 4004 with none; two permutations,
/// one without programs, one with two small blobs; samplers `ColorAndOpacity` and
/// `TSNormal`; one type-5 extra item; SRCALPHA / INVSRCALPHA, cull 2.
fn spec_shader() -> Vec<u8> {
    shader(
        0x2B00_0001,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0), (10, 1, 0)], 0),
            entry(4004, V::Word(0), &[], 0),
        ],
        &[
            Perm {
                programs: None,
                samplers: &["ColorAndOpacity", "TSNormal"],
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: Some((&[1, 2, 3], &[4, 5])),
                samplers: &["ColorAndOpacity", "TSNormal"],
                extra_type5: true,
                state: LIT_BASE,
            },
        ],
    )
}

/// A render surface (records.md §5) in A8R8G8B8 (stored B, G, R, A).
fn surface_a8r8g8b8(did: u32, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut out = W::default()
        .u32(did)
        .u32(0)
        .u32(w)
        .u32(h)
        .u32(21)
        .u32(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let [r, g, b, a] = px(x, y);
            out = out.bytes(&[b, g, r, a]);
        }
    }
    out.0
}

fn texture_list(did: u32, kind: u8, levels: &[u32]) -> Vec<u8> {
    let n = if kind == 4 {
        levels.len() / 6
    } else {
        levels.len()
    };
    let w = W::default().u32(did).u8(kind).u8(n as u8).u8(0).u16(0);
    levels.iter().fold(w, |w, &d| w.u32(d)).u32(0).u32(15).0
}

type Store = BTreeMap<u32, Vec<u8>>;
type SourceResult = Result<Option<Vec<u8>>, std::io::Error>;

fn library(store: Store) -> MaterialLibrary<FnSource<impl Fn(Did) -> SourceResult>> {
    MaterialLibrary::new(FnSource(move |d: Did| Ok(store.get(&d.0).cloned())))
}

// ---------------------------------------------------------------------------------------
// Decoders
// ---------------------------------------------------------------------------------------

#[test]
fn surface_records_with_0_1_3_materials() {
    for mats in [
        &[][..],
        &[0x3000_0001],
        &[0x3000_0001, 0x3000_0002, 0x3000_0003],
    ] {
        let bytes = surface_record(0x3100_0001, 0x2B00_0001, 0x1000_0050, mats);
        assert_eq!(bytes.len(), 18 + 4 * mats.len());
        let s = SurfaceRecord::parse_expecting(&bytes, 0x3100_0001u32).unwrap();
        assert_eq!(s.shader, Did(0x2B00_0001));
        assert_eq!(s.binding_key, 0x1000_0050);
        assert_eq!(s.materials.iter().map(|d| d.0).collect::<Vec<_>>(), mats);
        assert_eq!(s.tail, 1);
        assert!(!s.has_unusual_fields());
    }
}

#[test]
fn surface_record_length_must_match() {
    let mut bytes = surface_record(0x3100_0001, 0x2B00_0001, 1, &[0x3000_0001]);
    bytes.push(0);
    assert!(matches!(
        SurfaceRecord::parse(&bytes),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
    bytes.truncate(bytes.len() - 2);
    assert!(matches!(
        SurfaceRecord::parse(&bytes),
        Err(FormatError::Truncated { .. })
    ));
    // A huge count is rejected without allocating.
    let mut huge = surface_record(0x3100_0001, 0x2B00_0001, 1, &[]);
    huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(SurfaceRecord::parse(&huge).is_err());
    // Wrong prefix.
    let other = surface_record(0x3000_0001, 0x2B00_0001, 1, &[]);
    assert!(matches!(
        SurfaceRecord::parse(&other),
        Err(FormatError::WrongRecordType { .. })
    ));
    // A shader DID of the wrong type decodes but is unusual.
    let odd = surface_record(0x3100_0001, 0x4000_0001, 1, &[]);
    assert!(SurfaceRecord::parse(&odd).unwrap().has_unusual_fields());
}

#[test]
fn material_with_every_value_type() {
    let keyed = V::Keyed(
        [0.5, 0., 0., 0., 0., 0., 0., 0., 0., 0., 4.0],
        1,
        vec![(0.0, 0.0), (1.0, 50.0), (0.0, 100.0)],
    );
    let bytes = material(
        0x3000_0001,
        &[
            entry(3000, V::Word(0x4000_0010), &[], 0),
            entry(4004, V::Word(1), &[], 0),
            entry(2003, V::Color([0.5, 0.25, 1.0, 1.0]), &[], 0),
            entry(1043, V::Const(0.65), &[], 0),
            entry(
                1017,
                V::Wave(4, [0.1, 0., 0.2, 0., 0., 0., 0., 0., 0., 0.]),
                &[],
                0,
            ),
            entry(1018, keyed, &[], 0),
        ],
    );
    let m = MaterialRecord::parse_expecting(&bytes, 0x3000_0001u32).unwrap();
    assert_eq!(m.entries.len(), 6);
    assert!(!m.has_unusual_fields());
    assert_eq!(
        m.get(3000).unwrap().value.as_texture(),
        Some(Did(0x4000_0010))
    );
    assert_eq!(m.get(4004).unwrap().value.as_flag(), Some(true));
    assert_eq!(
        m.get(2003).unwrap().value.as_color(),
        Some([0.5, 0.25, 1.0, 1.0])
    );
    assert_eq!(m.get(1043).unwrap().value.as_float(), Some(0.65));
    let ParamValue::Float(wave) = &m.get(1017).unwrap().value else {
        panic!("float")
    };
    assert_eq!((wave.mode, wave.stored_params().len()), (4, 10));
    assert!(wave.is_animated());
    assert_eq!(wave.evaluate(), 0.1);
    let ParamValue::Float(k) = &m.get(1018).unwrap().value else {
        panic!("float")
    };
    assert_eq!((k.mode, k.loop_flag, k.keys.len()), (10, 1, 3));
    assert_eq!((k.stored_params()[10], k.period()), (4.0, 4.0));
    // §5.3.1: w[0] is unused by mode 10; at its start p = 0.1, between (0, 0) and (1, 50).
    assert!((k.evaluate() - 0.002).abs() < 1e-6);
    assert_eq!(
        DyeColors::of_material(&m).primary,
        Some([0.5, 0.25, 1.0, 1.0])
    );
}

#[test]
fn material_rejects_unknown_value_type_and_mode() {
    let mut e = entry(1000, V::Const(1.0), &[], 0);
    e[6..10].copy_from_slice(&8u32.to_le_bytes());
    let bytes = material(0x3000_0001, &[e]);
    assert!(matches!(
        MaterialRecord::parse(&bytes),
        Err(FormatError::Malformed { .. })
    ));
    let mut e = entry(1000, V::Const(1.0), &[], 0);
    e[10..14].copy_from_slice(&11u32.to_le_bytes());
    assert!(matches!(
        MaterialRecord::parse(&material(0x3000_0001, &[e])),
        Err(FormatError::Malformed { .. })
    ));
    // Trailing bytes.
    let mut bytes = material(0x3000_0001, &[]);
    bytes.push(0);
    assert!(matches!(
        MaterialRecord::parse(&bytes),
        Err(FormatError::TrailingBytes { .. })
    ));
}

#[test]
fn material_group_mismatch_is_unusual() {
    let mut e = entry(2000, V::Color([1.0; 4]), &[], 0);
    e[4..6].copy_from_slice(&3000u16.to_le_bytes());
    let m = MaterialRecord::parse(&material(0x3000_0001, &[e])).unwrap();
    assert!(m.has_unusual_fields());
    // A repeated id is unusual too; lookup takes the last.
    let m = MaterialRecord::parse(&material(
        0x3000_0001,
        &[
            entry(4004, V::Word(0), &[], 0),
            entry(4004, V::Word(1), &[], 0),
        ],
    ))
    .unwrap();
    assert!(m.has_repeated_id());
    assert_eq!(m.get(4004).unwrap().value.as_flag(), Some(true));
}

#[test]
fn shader_record_layout() {
    let s = ShaderRecord::parse_expecting(&spec_shader(), 0x2B00_0001u32).unwrap();
    assert_eq!(s.params.len(), 2);
    assert_eq!(s.permutations.len(), 2);
    assert!(s.permutations[0].programs.is_none());
    let p = s.permutations[1].programs.unwrap();
    assert_eq!((p.kind, p.a_len, p.b_len), (4, 3, 2));
    assert_eq!(s.permutations[1].extra_items.len(), 1);
    assert_eq!(s.permutations[1].extra_items[0].arrays.len(), 7);
    let smp = &s.permutations[0].samplers[0];
    assert_eq!(smp.name, "ColorAndOpacity");
    assert_eq!(smp.state.address_u(), AddressMode::Wrap);
    assert_eq!(smp.state.min_filter(), FilterMode::Linear);
    let rs = s.permutations[0].render_state;
    assert_eq!(
        BlendFactor::from_code(rs.src_blend()),
        BlendFactor::SrcAlpha
    );
    assert_eq!(
        BlendFactor::from_code(rs.dst_blend()),
        BlendFactor::InvSrcAlpha
    );
    assert_eq!(rs.cull(), CullMode::Back);
    assert!(rs.depth_write());
    assert!((rs.alpha_ref() - 0.8).abs() < 1e-6);
    assert!(!s.has_unusual_fields());

    let iface = ShaderInterface::from_record(&s);
    assert_eq!(iface.role(3000), Some("ColorAndOpacity"));
    let slot = iface.slot(3000).unwrap();
    assert_eq!(slot.packing.alpha, AlphaUse::Opacity);
    assert!(!iface.lit);
}

#[test]
fn shader_rejects_bad_bindings_and_unknown_items() {
    let perm = || Perm {
        programs: None,
        samplers: &["Color"],
        extra_type5: false,
        state: LIT_BASE,
    };
    // Permutation out of range.
    let bad = shader(
        0x2B00_0001,
        &[entry(3000, V::Word(0x4000_0001), &[(10, 1, 0)], 0)],
        &[perm()],
    );
    assert!(matches!(
        ShaderRecord::parse(&bad),
        Err(FormatError::Malformed { .. })
    ));
    // Texture sampler slot out of range.
    let bad = shader(
        0x2B00_0001,
        &[entry(3000, V::Word(0x4000_0001), &[(10, 0, 1)], 0)],
        &[perm()],
    );
    assert!(matches!(
        ShaderRecord::parse(&bad),
        Err(FormatError::Malformed { .. })
    ));
    // A non-texture binding's slot is not checked (its meaning is OPEN).
    let ok = shader(
        0x2B00_0001,
        &[entry(1000, V::Const(9.0), &[(2, 0, 7)], 0)],
        &[perm()],
    );
    assert!(ShaderRecord::parse(&ok).is_ok());
    // has_programs other than 0/1.
    let mut bad = shader(0x2B00_0001, &[], &[perm()]);
    let at = 12 + 4 + 16;
    bad[at..at + 4].copy_from_slice(&2u32.to_le_bytes());
    assert!(matches!(
        ShaderRecord::parse(&bad),
        Err(FormatError::Malformed { .. })
    ));
    // Unknown extra item type.
    let mut p = permutation(&Perm {
        programs: None,
        samplers: &[],
        extra_type5: true,
        state: LIT_BASE,
    });
    let item_at = 16 + 4 + 4 + 4;
    p[item_at] = 4;
    let bad = W::default()
        .u32(0x2B00_0001)
        .u32(0)
        .u32(0)
        .u32(1)
        .bytes(&p)
        .0;
    assert!(matches!(
        ShaderRecord::parse(&bad),
        Err(FormatError::Malformed { .. })
    ));
    // Program blob larger than the record.
    let mut bad = shader(
        0x2B00_0001,
        &[],
        &[Perm {
            programs: Some((&[1], &[2])),
            samplers: &[],
            extra_type5: false,
            state: LIT_BASE,
        }],
    );
    let blob_len_at = 12 + 4 + 16 + 4 + 4;
    bad[blob_len_at..blob_len_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        ShaderRecord::parse(&bad),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn role_prefers_packed_name_across_permutations() {
    let s = shader(
        0x2B00_0002,
        &[entry(
            3000,
            V::Word(0x4000_0001),
            &[(10, 0, 0), (10, 1, 0)],
            0,
        )],
        &[
            Perm {
                programs: None,
                samplers: &["DiffuseTexture"],
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: None,
                samplers: &["ColorAndDye"],
                extra_type5: false,
                state: LIT_BASE,
            },
        ],
    );
    let iface = ShaderInterface::from_record(&ShaderRecord::parse(&s).unwrap());
    assert_eq!(iface.role(3000), Some("ColorAndDye"));
    assert_eq!(iface.slot(3000).unwrap().packing.alpha, AlphaUse::DyeMask);
    assert_eq!(iface.slot(3000).unwrap().packing.color, ColorUse::BaseColor);
}

// ---------------------------------------------------------------------------------------
// Resolution and feature flags
// ---------------------------------------------------------------------------------------

/// A lit cut-out shader: 3000 `ColorAndOpacity`, 3011 `TSNormal`, 4004 + 1043, 4007,
/// 2000, specular 1000/2001; a base pass and a light pass.
fn lit_shader(did: u32) -> Vec<u8> {
    shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0), (10, 1, 0)], 0),
            entry(3011, V::Word(0x4000_000D), &[(10, 0, 1)], 0),
            entry(4004, V::Word(0), &[(8, 0, 0)], 0),
            entry(1043, V::Const(0.8), &[(9, 0, 0)], 0),
            entry(4007, V::Word(0), &[], 0),
            entry(2000, V::Color([1.0; 4]), &[(0, 0, 0)], 0),
            entry(2001, V::Color([0.5; 4]), &[(1, 0, 0)], 0),
            entry(1000, V::Const(25.0), &[(2, 0, 0)], 0),
        ],
        &[
            Perm {
                programs: None,
                samplers: &["ColorAndOpacity", "TSNormal"],
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: None,
                samplers: &["ColorAndOpacity"],
                extra_type5: false,
                state: LIT_LIGHT,
            },
        ],
    )
}

/// An unlit shader whose blocks are all `state`.
fn unlit_shader(did: u32, state: (u8, u8, u8, u8, u8, f32)) -> Vec<u8> {
    shader(
        did,
        &[entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0)],
        &[Perm {
            programs: None,
            samplers: &["Color"],
            extra_type5: false,
            state,
        }],
    )
}

/// A dyeable lit shader: 3000 `ColorAndDye`, 3024 `DyeAlpha`, 2003, 2013.
fn dye_shader(did: u32) -> Vec<u8> {
    shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0100), &[(10, 0, 0)], 0),
            entry(3024, V::Word(0x4000_0101), &[(10, 0, 1)], 0),
            entry(2003, V::Color([1.0; 4]), &[(4, 0, 0)], 0),
            entry(2013, V::Color([1.0; 4]), &[], 1),
        ],
        &[
            Perm {
                programs: None,
                samplers: &["ColorAndDye", "DyeAlpha"],
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: None,
                samplers: &[],
                extra_type5: false,
                state: LIT_LIGHT,
            },
        ],
    )
}

fn base_store() -> Store {
    let mut s = Store::new();
    s.insert(0x2B00_0010, lit_shader(0x2B00_0010));
    s.insert(0x2B00_0020, dye_shader(0x2B00_0020));
    // Generic cut-out material, then a specific one (§9.2 evidence).
    s.insert(
        0x3000_0001,
        material(
            0x3000_0001,
            &[
                entry(4004, V::Word(1), &[], 0),
                entry(1043, V::Const(0.8), &[], 0),
            ],
        ),
    );
    s.insert(
        0x3000_0002,
        material(
            0x3000_0002,
            &[
                entry(3000, V::Word(0x4000_0042), &[], 0),
                entry(1043, V::Const(0.4), &[], 0),
                entry(9999, V::Word(5), &[], 0), // undeclared
            ],
        ),
    );
    // A dye material (only 2003).
    s.insert(
        0x3000_0003,
        material(
            0x3000_0003,
            &[entry(2003, V::Color([0.0, 0.0, 1.0, 1.0]), &[], 0)],
        ),
    );
    s.insert(
        0x3100_0001,
        surface_record(0x3100_0001, 0x2B00_0010, 1, &[]),
    );
    s.insert(
        0x3100_0002,
        surface_record(0x3100_0002, 0x2B00_0010, 1, &[0x3000_0001]),
    );
    s.insert(
        0x3100_0003,
        surface_record(
            0x3100_0003,
            0x2B00_0010,
            0x1000_0050,
            &[0x3000_0001, 0x3000_0002],
        ),
    );
    s.insert(
        0x3100_0004,
        surface_record(0x3100_0004, 0x2B00_0020, 0x1000_0051, &[]),
    );
    s.insert(
        0x3100_0005,
        surface_record(0x3100_0005, 0x2B00_0010, 1, &[0x3000_0099, 0x3000_0001]),
    );
    s
}

#[test]
fn defaults_only() {
    let lib = library(base_store());
    let r = lib.resolve_surface(Did(0x3100_0001)).unwrap();
    assert_eq!(r.surface, Some(Did(0x3100_0001)));
    assert_eq!(r.diffuse(), Some(Did(0x4000_0007)));
    assert_eq!(r.params.get(3000).unwrap().origin, ParamOrigin::Default);
    assert!(r.features.lit);
    // 4004 defaults to 0: no alpha test. A lit `ColorAndOpacity` surface is then
    // translucent by its texture alpha (§11.3, revised 2026-10-05).
    assert_eq!(r.features.alpha_test, None);
    assert_eq!(r.features.blend.mode, BlendMode::AlphaBlend);
    assert!(r.features.blend.is_transparent());
    assert_eq!(r.features.blend.opacity_slot, Some(3000));
    assert!(r.features.normal_mapped);
    assert!(r.features.specular);
    assert_eq!(r.specular(), Some(([0.5; 4], 25.0)));
    assert!(!r.features.two_sided);
    assert!(!r.features.dyeable);
    assert_eq!(r.texture_lists(), vec![Did(0x4000_0007), Did(0x4000_000D)]);
}

#[test]
fn one_material_and_last_wins() {
    let lib = library(base_store());
    let r = lib.resolve_surface(Did(0x3100_0002)).unwrap();
    let at = r.features.alpha_test.unwrap();
    assert_eq!((at.threshold, at.source_slot), (0.8, 3000));
    // Lit and alpha-tested: the kept texels blend by their opacity (§11.3).
    assert_eq!(r.features.blend.mode, BlendMode::AlphaBlend);

    let r = lib.resolve_surface(Did(0x3100_0003)).unwrap();
    assert_eq!(r.features.alpha_test.unwrap().threshold, 0.4);
    assert_eq!(r.diffuse(), Some(Did(0x4000_0042)));
    assert_eq!(
        r.params.get(1043).unwrap().origin,
        ParamOrigin::Material {
            index: 1,
            did: Did(0x3000_0002)
        }
    );
    let ignored = r.params.ignored();
    assert_eq!(ignored.len(), 1);
    assert_eq!(
        (ignored[0].entry.id, ignored[0].reason),
        (9999, IgnoreReason::Undeclared)
    );
}

#[test]
fn missing_material_is_skipped() {
    let lib = library(base_store());
    let r = lib.resolve_surface(Did(0x3100_0005)).unwrap();
    assert_eq!(r.materials, vec![Did(0x3000_0001)]);
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].did, Did(0x3000_0099));
    assert!(r.features.alpha_test.is_some());
}

#[test]
fn missing_and_wrong_records_are_errors() {
    let lib = library(base_store());
    assert!(matches!(
        lib.resolve_surface(Did(0x3100_7777)),
        Err(MaterialError::RecordMissing { .. })
    ));
    assert!(matches!(
        lib.resolve_surface(Did(0x3000_0001)),
        Err(MaterialError::WrongKind { .. })
    ));
    let mut store = base_store();
    store.insert(
        0x3100_0009,
        surface_record(0x3100_0009, 0x2B00_0055, 1, &[]),
    );
    store.insert(0x2B00_0055, vec![1, 2, 3]);
    let lib = library(store);
    assert!(matches!(
        lib.resolve_surface(Did(0x3100_0009)),
        Err(MaterialError::RecordDecode { .. })
    ));
}

#[test]
fn source_errors_are_reported() {
    let lib = MaterialLibrary::new(FnSource(|_d: Did| {
        Err::<Option<Vec<u8>>, _>(std::io::Error::other("boom"))
    }));
    assert!(matches!(
        lib.surface_record(Did(0x3100_0001)),
        Err(MaterialError::RecordSource { .. })
    ));
}

#[test]
fn appearance_group_with_shader_override_and_dye() {
    let lib = library(base_store());
    let surface = lib.surface_record(Did(0x3100_0003)).unwrap();
    let group = AppearanceGroup {
        shader_override: Did(0x2B00_0020),
        material: Did(0x3000_0002),
        binding_key: 0x1000_0050,
    };
    assert!(group.applies_to(&surface));
    let layer = group.layering(Some(Did(0x3000_0003)));
    assert_eq!(layer.shader_override, Some(Did(0x2B00_0020)));
    let r = lib.resolve_layered(Did(0x3100_0003), &layer).unwrap();
    assert_eq!(r.shader.did, Did(0x2B00_0020));
    // The surface's own materials, then the group's, then the dye material. The
    // group's material repeats the surface's own 0x3000_0002, which is applied at its
    // last position only (the same effective values).
    assert_eq!(
        r.materials,
        vec![Did(0x3000_0001), Did(0x3000_0002), Did(0x3000_0003)]
    );
    assert_eq!(
        r.params.get(3000).unwrap().origin,
        ParamOrigin::Material {
            index: 1,
            did: Did(0x3000_0002)
        }
    );
    assert!(r.features.dyeable);
    assert!(r.features.dyeable_2);
    let dye = r.dye.unwrap();
    assert_eq!(dye.primary.color, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(dye.primary.mask.slot, 3000);
    let second = dye.secondary.unwrap();
    assert_eq!(second.mask.slot, 3024);
    assert_eq!(second.color, [1.0; 4]);
    // Diffuse comes from the group's material (last wins).
    assert_eq!(r.diffuse(), Some(Did(0x4000_0042)));

    // No override: the surface's own shader.
    let plain = Layering {
        shader_override: None,
        materials: vec![],
    };
    let r = lib.resolve_layered(Did(0x3100_0003), &plain).unwrap();
    assert_eq!(r.shader.did, Did(0x2B00_0010));
    let none = AppearanceGroup {
        shader_override: Did(0),
        material: Did(0x3000_0002),
        binding_key: 2,
    };
    assert!(!none.applies_to(&surface));
    assert_eq!(none.layering(None).shader_override, None);
}

#[test]
fn blend_classes_of_unlit_shaders() {
    let cases = [
        ((2, 1, 1, 1, 2, 0.9), BlendMode::Opaque, false),
        ((5, 6, 1, 0, 2, 0.9), BlendMode::AlphaBlend, true),
        ((5, 2, 1, 1, 2, 0.9), BlendMode::Additive, true),
        ((9, 2, 1, 0, 1, 0.9), BlendMode::Custom, true),
    ];
    for (i, (state, mode, transparent)) in cases.into_iter().enumerate() {
        let did = 0x2B00_0100 + i as u32;
        let mut store = Store::new();
        store.insert(did, unlit_shader(did, state));
        let lib = library(store);
        let r = lib.resolve(Did(did), &[]).unwrap();
        assert!(!r.features.lit);
        assert_eq!(r.features.blend.mode, mode, "case {i}");
        assert_eq!(r.features.blend.is_transparent(), transparent);
        assert_eq!(r.features.blend.depth_write, state.3 != 0);
        assert_eq!(r.features.two_sided, state.4 == 1);
    }
}

#[test]
fn alpha_test_needs_an_opacity_source() {
    // A ColorAndDye diffuse with 4004 on: no opacity source, alpha test off (§11.2).
    let did = 0x2B00_0200;
    let sh = shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0001), &[(10, 0, 0)], 0),
            entry(4004, V::Word(1), &[], 0),
            entry(1043, V::Const(0.5), &[], 0),
        ],
        &[Perm {
            programs: None,
            samplers: &["ColorAndDye"],
            extra_type5: false,
            state: LIT_LIGHT,
        }],
    );
    let mut store = Store::new();
    store.insert(did, sh);
    let lib = library(store);
    let r = lib.resolve(Did(did), &[]).unwrap();
    assert_eq!(r.features.alpha_test, None);
    assert_eq!(r.features.opacity_slot, None);
    assert_eq!(r.features.blend.mode, BlendMode::Opaque);
}

// ---------------------------------------------------------------------------------------
// Textures and dye preview
// ---------------------------------------------------------------------------------------

fn texture_store() -> Store {
    let mut s = base_store();
    // Diffuse 0x4000_0100: level 0 (8×8) missing (no highres), levels 1 (4×4), 2 (2×2).
    // Grey, with alpha 0 (dyeable) on the left half.
    let grey = |x: u32, _y: u32| [200, 200, 200, if x < 2 { 0 } else { 255 }];
    s.insert(
        0x4000_0100,
        texture_list(0x4000_0100, 2, &[0x4100_0000, 0x4100_0001, 0x4100_0002]),
    );
    s.insert(0x4100_0001, surface_a8r8g8b8(0x4100_0001, 4, 4, grey));
    s.insert(
        0x4100_0002,
        surface_a8r8g8b8(0x4100_0002, 2, 2, |x, _| {
            [200, 200, 200, if x < 1 { 0 } else { 255 }]
        }),
    );
    // Separate dye mask 0x4000_0101: one 1×1 level, fully dyeable.
    s.insert(0x4000_0101, texture_list(0x4000_0101, 2, &[0x4100_0010]));
    s.insert(
        0x4100_0010,
        surface_a8r8g8b8(0x4100_0010, 1, 1, |_, _| [0, 0, 0, 0]),
    );
    // A cube map with six faces of one 2×2 image.
    s.insert(0x4000_0200, texture_list(0x4000_0200, 4, &[0x4100_0020; 6]));
    s.insert(
        0x4100_0020,
        surface_a8r8g8b8(0x4100_0020, 2, 2, |_, _| [1, 2, 3, 4]),
    );
    // A list with no resolving level.
    s.insert(0x4000_0300, texture_list(0x4000_0300, 2, &[0, 0x4100_0099]));
    s
}

#[test]
fn texture_effective_chain_and_cap() {
    let lib = library(texture_store());
    // Level 0 is missing: the chain starts at level 1.
    let t = lib.texture(Did(0x4000_0100), None).unwrap();
    assert_eq!((t.level, t.surface), (1, Did(0x4100_0001)));
    assert_eq!((t.image.width, t.image.height), (4, 4));
    assert_eq!(&t.image.pixels[..4], &[200, 200, 200, 0]);
    // A cap of 2 takes level 2; a cap of 1 keeps the last level.
    assert_eq!(lib.texture(Did(0x4000_0100), Some(2)).unwrap().level, 2);
    assert_eq!(lib.texture(Did(0x4000_0100), Some(1)).unwrap().level, 2);
    // Cached by level surface: both caps share the decoded level 2.
    assert_eq!(lib.cache_usage().images.0, 2);
    // Cube map: first face.
    let c = lib.texture(Did(0x4000_0200), None).unwrap();
    assert_eq!(c.surface, Did(0x4100_0020));
    // Unavailable.
    assert!(matches!(
        lib.texture(Did(0x4000_0300), None),
        Err(MaterialError::TextureUnavailable { .. })
    ));
    // Size limit.
    let small = MaterialLibrary::with_config(
        FnSource({
            let s = texture_store();
            move |d: Did| Ok::<_, std::io::Error>(s.get(&d.0).cloned())
        }),
        LibraryConfig {
            max_texture_dimension: 2,
            ..LibraryConfig::default()
        },
    );
    // The configured limit caps the level choice: level 1 (4×4) is above it, so level
    // 2 (2×2) is shown, with or without a caller's cap.
    for cap in [None, Some(1000)] {
        assert_eq!(small.texture(Did(0x4000_0100), cap).unwrap().level, 2);
    }
    small.clear_cache();
    assert_eq!(small.cache_usage().images, (0, 0));
    // Only when even the last level is above the limit is it an error.
    let tiny = MaterialLibrary::with_config(
        FnSource({
            let s = texture_store();
            move |d: Did| Ok::<_, std::io::Error>(s.get(&d.0).cloned())
        }),
        LibraryConfig {
            max_texture_dimension: 1,
            ..LibraryConfig::default()
        },
    );
    assert!(matches!(
        tiny.texture(Did(0x4000_0100), None),
        Err(MaterialError::TooLarge {
            width: 2,
            height: 2,
            limit: 1
        })
    ));
}

#[test]
fn dye_preview_applies_both_channels() {
    let lib = library(texture_store());
    let r = lib.resolve_surface(Did(0x3100_0004)).unwrap();
    assert!(r.features.dyeable);
    // Effective colours are white: nothing changes in RGB, alpha becomes opaque.
    let img = lib.dye_preview(&r, None).unwrap().unwrap();
    assert_eq!(&img.pixels[..4], &[200, 200, 200, 255]);
    // Primary red on the diffuse alpha mask (left half), secondary half-grey on the
    // separate fully dyeable mask (everywhere).
    let colors = DyeColors {
        primary: Some([1.0, 0.0, 0.0, 1.0]),
        secondary: Some([0.5, 0.5, 0.5, 1.0]),
    };
    let img = lib.dye_preview_with(&r, colors, None).unwrap().unwrap();
    // x = 0, both masks fully dyeable: the second channel dyes the undyed base,
    // a2·(200, 0, 0) + (1 − a2)·(200 × 0.5) with a2 = 0 (§20.1, lighting.md §5.2 step 3).
    assert_eq!(&img.pixels[..4], &[100, 100, 100, 255]);
    assert_eq!(&img.pixels[8..12], &[100, 100, 100, 255]); // x = 2: second only
    // A non-dyeable material has no preview.
    let plain = lib.resolve_surface(Did(0x3100_0001)).unwrap();
    assert!(lib.dye_preview(&plain, None).unwrap().is_none());
}

#[test]
fn library_is_shareable_across_threads() {
    let lib = library(texture_store());
    let seen = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                let r = lib.resolve_surface(Did(0x3100_0003)).unwrap();
                seen.lock().unwrap().push(r.diffuse());
            });
        }
    });
    assert!(
        seen.into_inner()
            .unwrap()
            .iter()
            .all(|d| *d == Some(Did(0x4000_0042)))
    );
}

// ---------------------------------------------------------------------------------------
// Hostile input
// ---------------------------------------------------------------------------------------

proptest! {
    #[test]
    fn decoders_never_panic(data in proptest::collection::vec(any::<u8>(), 0..512), prefix in 0usize..3) {
        let mut data = data;
        if data.len() >= 4 {
            data[3] = [0x31, 0x30, 0x2B][prefix];
        }
        let _ = SurfaceRecord::parse(&data);
        let _ = MaterialRecord::parse(&data);
        if let Ok(s) = ShaderRecord::parse(&data) {
            let _ = ShaderInterface::from_record(&s);
        }
    }

    #[test]
    fn mutated_records_never_panic(pos in 0usize..2048, byte in any::<u8>(), cut in 0usize..2048) {
        for mut rec in [spec_shader(), lit_shader(0x2B00_0010), dye_shader(0x2B00_0020)] {
            let i = pos % rec.len();
            rec[i] = byte;
            rec.truncate(rec.len() - cut % rec.len());
            if let Ok(s) = ShaderRecord::parse(&rec) {
                let iface = std::sync::Arc::new(ShaderInterface::from_record(&s));
                let _ = tessera_material::ResolvedMaterial::resolve(iface, &[]);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Review regressions: textures
// ---------------------------------------------------------------------------------------

/// A JPEG-format render surface whose stream declares a `w`×`h` baseline frame but holds
/// no scan: its size can be read, but decoding it fails.
fn jpeg_header_only(did: u32, w: u16, h: u16) -> Vec<u8> {
    let mut j = vec![0xFF, 0xD8];
    j.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00]);
    j.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    j.extend_from_slice(&h.to_be_bytes());
    j.extend_from_slice(&w.to_be_bytes());
    j.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    j.extend_from_slice(&[0xFF, 0xD9]);
    W::default()
        .u32(did)
        .u32(15)
        .u32(0)
        .u32(0)
        .u32(500)
        .u32(j.len() as u32)
        .bytes(&j)
        .0
}

fn library_with(store: Store, config: LibraryConfig) -> MaterialLibrary<impl RecordSource> {
    MaterialLibrary::with_config(
        FnSource(move |d: Did| Ok::<_, std::io::Error>(store.get(&d.0).cloned())),
        config,
    )
}

fn limit(max_texture_dimension: u32) -> LibraryConfig {
    LibraryConfig {
        max_texture_dimension,
        ..LibraryConfig::default()
    }
}

#[test]
fn jpeg_levels_are_sized_by_their_frame_header() {
    let mut s = Store::new();
    // [JPEG 3000×3000 (undecodable: no scan), A8R8G8B8 64×64].
    s.insert(
        0x4000_0001,
        texture_list(0x4000_0001, 2, &[0x4100_0001, 0x4100_0002]),
    );
    s.insert(0x4100_0001, jpeg_header_only(0x4100_0001, 3000, 3000));
    s.insert(
        0x4100_0002,
        surface_a8r8g8b8(0x4100_0002, 64, 64, |_, _| [1, 2, 3, 4]),
    );
    // A list whose only level is the large JPEG, and one whose JPEG has no frame header.
    s.insert(0x4000_0002, texture_list(0x4000_0002, 2, &[0x4100_0001]));
    s.insert(0x4000_0003, texture_list(0x4000_0003, 2, &[0x4100_0003]));
    let mut broken = jpeg_header_only(0x4100_0003, 8, 8);
    broken[26..28].copy_from_slice(&[0xFF, 0xDA]); // a scan before any frame header
    s.insert(0x4100_0003, broken);

    let lib = library_with(s.clone(), LibraryConfig::default());
    // A caller's cap skips the JPEG level by its frame size (it would fail to decode).
    let t = lib.texture(Did(0x4000_0001), Some(64)).unwrap();
    assert_eq!((t.level, t.image.width), (1, 64));
    // Uncapped, the JPEG level is chosen and its decoder runs (and fails).
    assert!(matches!(
        lib.texture(Did(0x4000_0001), None),
        Err(MaterialError::Decode { .. })
    ));
    // The configured limit skips it too, without decoding.
    let lib = library_with(s, limit(1024));
    assert_eq!(lib.texture(Did(0x4000_0001), None).unwrap().level, 1);
    // A JPEG above the limit with nothing smaller is refused before decoding: TooLarge,
    // not the decoder's error.
    assert!(matches!(
        lib.texture(Did(0x4000_0002), None),
        Err(MaterialError::TooLarge {
            width: 3000,
            height: 3000,
            limit: 1024
        })
    ));
    // A JPEG level whose size cannot be read does not resolve.
    assert!(matches!(
        lib.texture(Did(0x4000_0003), None),
        Err(MaterialError::TextureUnavailable { .. })
    ));
    let chain = lib.texture_chain(Did(0x4000_0001), None).unwrap();
    assert_eq!(chain.levels.len(), 1);
    assert_eq!(chain.levels[0].index, 1);
}

#[test]
fn limit_caps_the_level_choice() {
    let mut s = Store::new();
    s.insert(
        0x4000_0001,
        texture_list(0x4000_0001, 2, &[0x4100_0001, 0x4100_0002]),
    );
    s.insert(
        0x4100_0001,
        surface_a8r8g8b8(0x4100_0001, 128, 128, |_, _| [0; 4]),
    );
    s.insert(
        0x4100_0002,
        surface_a8r8g8b8(0x4100_0002, 64, 64, |_, _| [0; 4]),
    );
    let lib = library_with(s, limit(64));
    for cap in [None, Some(64), Some(1000), Some(u32::MAX)] {
        let t = lib.texture(Did(0x4000_0001), cap).unwrap();
        assert_eq!((t.level, t.image.width), (1, 64), "cap {cap:?}");
    }
    let chain = lib.texture_chain(Did(0x4000_0001), None).unwrap();
    assert_eq!(chain.levels.len(), 1);
    assert_eq!(chain.levels[0].did, Did(0x4100_0002));
}

#[test]
fn decoded_levels_are_cached_once_per_surface() {
    let mut s = Store::new();
    // Two lists sharing one 16×16 level (texture-list.md §3: identical bodies exist).
    s.insert(0x4000_0001, texture_list(0x4000_0001, 2, &[0x4100_0001]));
    s.insert(0x4000_0002, texture_list(0x4000_0002, 2, &[0x4100_0001]));
    s.insert(
        0x4100_0001,
        surface_a8r8g8b8(0x4100_0001, 16, 16, |x, y| [x as u8, y as u8, 0, 255]),
    );
    let lib = library_with(s, LibraryConfig::default());
    let first = lib.texture(Did(0x4000_0001), Some(16)).unwrap();
    for cap in 16..60 {
        let t = lib.texture(Did(0x4000_0001), Some(cap)).unwrap();
        assert!(std::sync::Arc::ptr_eq(&t.image, &first.image));
    }
    let other = lib.texture(Did(0x4000_0002), None).unwrap();
    assert_eq!(other.list, Did(0x4000_0002));
    assert!(std::sync::Arc::ptr_eq(&other.image, &first.image));
    assert_eq!(lib.cache_usage().images, (1, 16 * 16 * 4));
    // Level headers are cached too: one per surface.
    assert_eq!(lib.cache_usage().level_headers.0, 1);
}

#[test]
fn zero_sized_levels_do_not_resolve() {
    let mut s = Store::new();
    s.insert(
        0x4000_0001,
        texture_list(0x4000_0001, 2, &[0x4100_0001, 0x4100_0002]),
    );
    s.insert(0x4000_0002, texture_list(0x4000_0002, 2, &[0x4100_0001]));
    s.insert(
        0x4100_0001,
        surface_a8r8g8b8(0x4100_0001, 0, 4, |_, _| [0; 4]),
    );
    s.insert(
        0x4100_0002,
        surface_a8r8g8b8(0x4100_0002, 2, 2, |_, _| [9; 4]),
    );
    let lib = library_with(s, LibraryConfig::default());
    // The zero-sized level 0 is skipped like a missing one.
    let t = lib.texture(Did(0x4000_0001), None).unwrap();
    assert_eq!((t.level, t.image.width, t.image.height), (1, 2, 2));
    assert!(matches!(
        lib.texture(Did(0x4000_0002), None),
        Err(MaterialError::TextureUnavailable { .. })
    ));
}

#[test]
fn texture_chain_and_cube_faces() {
    let lib = library(texture_store());
    // Level 0 missing: the effective chain is levels 1 and 2, which halve.
    let chain = lib.texture_chain(Did(0x4000_0100), None).unwrap();
    assert_eq!(
        chain.levels.iter().map(|l| l.index).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        (chain.levels[0].level.width, chain.levels[1].level.width),
        (4, 2)
    );
    let plan = chain.plan.unwrap();
    assert_eq!((plan.first, plan.uploadable), (0, 2));
    // A cap drops leading levels.
    let capped = lib.texture_chain(Did(0x4000_0100), Some(2)).unwrap();
    assert_eq!(capped.levels.len(), 1);
    assert_eq!(capped.levels[0].index, 2);
    // Level payloads, for an upload.
    let payload = lib.level_payload(chain.levels[0].did).unwrap();
    let surface = tessera_formats::Surface::parse(&payload).unwrap();
    assert_eq!(surface.data.len(), 4 * 4 * 4);
    assert!(matches!(
        lib.level_payload(Did(0x4000_0100)),
        Err(MaterialError::NotSurface { .. })
    ));
    assert!(matches!(
        lib.level_payload(Did(0x4100_0000)),
        Err(MaterialError::Missing { .. })
    ));
    // Unavailable and cube lists.
    assert!(matches!(
        lib.texture_chain(Did(0x4000_0300), None),
        Err(MaterialError::TextureUnavailable { .. })
    ));
    assert!(matches!(
        lib.texture_chain(Did(0x4000_0200), None),
        Err(MaterialError::WrongKind { .. })
    ));
    // All six faces of the cube; the repeated face is decoded once.
    let faces = lib.cube_faces(Did(0x4000_0200)).unwrap().unwrap();
    for (i, f) in faces.iter().enumerate() {
        assert_eq!((f.level, f.surface), (i, Did(0x4100_0020)));
        assert!(std::sync::Arc::ptr_eq(&f.image, &faces[0].image));
    }
    assert_eq!(&faces[5].image.pixels[..4], &[1, 2, 3, 4]);
    // A mip chain has no faces.
    assert!(lib.cube_faces(Did(0x4000_0100)).unwrap().is_none());
}

// ---------------------------------------------------------------------------------------
// Review regressions: resolution bounds
// ---------------------------------------------------------------------------------------

#[test]
fn repeated_materials_are_applied_once() {
    let mut store = base_store();
    // One material with 100 undeclared entries, listed 20,000 times.
    let entries: Vec<Vec<u8>> = (0..100)
        .map(|i| entry(4100 + i, V::Word(1), &[], 0))
        .collect();
    store.insert(0x3000_0060, material(0x3000_0060, &entries));
    store.insert(
        0x3100_0060,
        surface_record(0x3100_0060, 0x2B00_0010, 1, &[0x3000_0060; 20_000]),
    );
    // Alternating repeats: A, B, A, B, … → B, A.
    let mut alternating = Vec::new();
    for _ in 0..1000 {
        alternating.extend_from_slice(&[0x3000_0001, 0x3000_0060]);
    }
    store.insert(
        0x3100_0061,
        surface_record(0x3100_0061, 0x2B00_0010, 1, &alternating),
    );
    let lib = library(store);
    let r = lib.resolve_surface(Did(0x3100_0060)).unwrap();
    assert_eq!(r.materials, vec![Did(0x3000_0060)]);
    assert_eq!(r.params.ignored().len(), 100);
    let r = lib.resolve_surface(Did(0x3100_0061)).unwrap();
    assert_eq!(r.materials, vec![Did(0x3000_0001), Did(0x3000_0060)]);
    assert_eq!(r.params.ignored().len(), 100);
    assert_eq!(r.features.alpha_test.unwrap().threshold, 0.8);
    // The pure resolver reduces repeats too.
    let m = lib.material(Did(0x3000_0060)).unwrap();
    let shader = lib.shader(Did(0x2B00_0010)).unwrap();
    let refs = vec![m.as_ref(); 5000];
    let r = tessera_material::ResolvedMaterial::resolve(shader, &refs);
    assert_eq!(r.materials.len(), 1);
    assert_eq!(r.params.ignored().len(), 100);
}

#[test]
fn ignored_entries_and_distinct_materials_are_bounded() {
    let mut store = base_store();
    let n = MAX_IGNORED_ENTRIES + 500;
    let entries: Vec<Vec<u8>> = (0..n as u32)
        .map(|i| entry(5000 + i, V::Word(1), &[], 0))
        .collect();
    store.insert(0x3000_0070, material(0x3000_0070, &entries));
    let distinct: Vec<u32> = (0..=MAX_RESOLVE_MATERIALS as u32)
        .map(|i| 0x3000_1000 + i)
        .collect();
    store.insert(
        0x3100_0070,
        surface_record(0x3100_0070, 0x2B00_0010, 1, &distinct),
    );
    store.insert(
        0x3100_0071,
        surface_record(
            0x3100_0071,
            0x2B00_0010,
            1,
            &distinct[..MAX_RESOLVE_MATERIALS],
        ),
    );
    let lib = library(store);
    let r = lib.resolve(Did(0x2B00_0010), &[Did(0x3000_0070)]).unwrap();
    assert_eq!(r.params.ignored().len(), MAX_IGNORED_ENTRIES);
    assert_eq!(r.params.ignored_omitted(), 500);
    assert!(matches!(
        lib.resolve_surface(Did(0x3100_0070)),
        Err(MaterialError::TooManyMaterials {
            count: 65,
            limit: 64
        })
    ));
    // At the limit: resolves (every material is missing, so all are skipped).
    let r = lib.resolve_surface(Did(0x3100_0071)).unwrap();
    assert_eq!(r.skipped.len(), MAX_RESOLVE_MATERIALS);
}

#[test]
fn resolved_cache_counts_real_memory() {
    let mut store = base_store();
    // One undeclared mode-10 float with 100,000 keyframes (800 kB).
    let keys: Vec<(f32, f32)> = (0..100_000).map(|i| (i as f32, 0.0)).collect();
    store.insert(
        0x3000_0080,
        material(
            0x3000_0080,
            &[entry(1999, V::Keyed([0.0; 11], 0, keys), &[], 0)],
        ),
    );
    store.insert(
        0x3100_0080,
        surface_record(0x3100_0080, 0x2B00_0010, 1, &[0x3000_0080]),
    );
    let lib = library(store);
    let r = lib.resolve_surface(Did(0x3100_0080)).unwrap();
    assert_eq!(r.params.ignored().len(), 1);
    assert!(r.heap_bytes() >= 800_000);
    let (entries, bytes) = lib.cache_usage().resolved;
    assert_eq!(entries, 1);
    assert!(bytes >= 800_000, "resolved cache charged {bytes} bytes");
}

#[test]
fn repeated_id_check_is_linear() {
    // 100,000 distinct ids (O(n²) would take minutes in a debug build), then a repeat.
    let mut entries: Vec<Vec<u8>> = (0..100_000u32)
        .map(|i| entry(10_000 + i, V::Word(0), &[], 0))
        .collect();
    let m = MaterialRecord::parse(&material(0x3000_0001, &entries)).unwrap();
    assert!(!m.has_repeated_id());
    entries.push(entry(10_000, V::Word(1), &[], 0));
    let m = MaterialRecord::parse(&material(0x3000_0001, &entries)).unwrap();
    assert!(m.has_repeated_id());
}

// ---------------------------------------------------------------------------------------
// Review regressions: opacity for blending (§18 Decision) and §11.2 branches
// ---------------------------------------------------------------------------------------

/// A shader with one permutation of `state` and `decls` bound to `samplers`.
fn one_pass_shader(
    did: u32,
    decls: &[Vec<u8>],
    samplers: &[&str],
    state: (u8, u8, u8, u8, u8, f32),
) -> Vec<u8> {
    shader(
        did,
        decls,
        &[Perm {
            programs: None,
            samplers,
            extra_type5: false,
            state,
        }],
    )
}

fn resolve_one(
    sh: Vec<u8>,
    did: u32,
    materials: &[(u32, Vec<u8>)],
) -> tessera_material::ResolvedMaterial {
    let mut store = Store::new();
    store.insert(did, sh);
    for (d, m) in materials {
        store.insert(*d, m.clone());
    }
    let lib = library(store);
    let dids: Vec<Did> = materials.iter().map(|(d, _)| Did(*d)).collect();
    lib.resolve(Did(did), &dids).unwrap()
}

#[test]
fn unlit_source_alpha_from_the_diffuse_by_role() {
    const ALPHA: (u8, u8, u8, u8, u8, f32) = (5, 6, 1, 0, 2, 0.9);
    // Additive with depth write: without it, the record would be lit (§11.3).
    const ADD: (u8, u8, u8, u8, u8, f32) = (5, 2, 1, 1, 2, 0.9);
    const ONE_ZERO: (u8, u8, u8, u8, u8, f32) = (2, 1, 1, 1, 2, 0.9);
    let diffuse = |bound: bool| {
        let b: &[(u16, u32, u32)] = if bound { &[(10, 0, 0)] } else { &[] };
        vec![entry(3000, V::Word(0x4000_0007), b, 0)]
    };
    // (sampler name, bound, state, expected blend opacity slot). §20.1: only
    // `DiffuseTexture`, `LiquidDiffuseTexture`, `ColorTexture` and `ColorTexture2` put the
    // slot-3000 alpha into the output; `Color` and the others output the 2000 alpha.
    let cases: [(&str, bool, _, Option<u32>); 12] = [
        ("DiffuseTexture", true, ALPHA, Some(3000)),
        ("Color", true, ADD, None),
        ("Color", true, ALPHA, None),
        ("DiffuseMap", true, ALPHA, None),
        ("LiquidDiffuseTexture", true, ALPHA, Some(3000)),
        ("ColorTexture", true, ADD, Some(3000)),
        ("ColorTexture2", true, ALPHA, Some(3000)),
        ("", false, ALPHA, None), // no named binding
        ("ColorAndGloss", true, ALPHA, None),
        ("ColorAndDye", true, ADD, None),
        ("DiffuseTexture", true, ONE_ZERO, None), // factors without source alpha
        ("ColorAndOpacity", true, ALPHA, Some(3000)), // the §11.2 source
    ];
    for (i, (name, bound, state, expected)) in cases.into_iter().enumerate() {
        let did = 0x2B00_0300 + i as u32;
        let r = resolve_one(
            one_pass_shader(did, &diffuse(bound), &[name], state),
            did,
            &[],
        );
        assert!(!r.features.lit);
        assert_eq!(r.features.blend.opacity_slot, expected, "case {i} ({name})");
        // The §11.2 opacity source (alpha test) is unchanged.
        let opacity = (name == "ColorAndOpacity").then_some(3000);
        assert_eq!(r.features.opacity_slot, opacity, "case {i} ({name})");
    }
    // Lit records are opaque: no blend opacity from a plain diffuse.
    let did = 0x2B00_0320;
    let sh = shader(
        did,
        &diffuse(true),
        &[
            Perm {
                programs: None,
                samplers: &["DiffuseTexture"],
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: None,
                samplers: &["DiffuseTexture"],
                extra_type5: false,
                state: LIT_LIGHT,
            },
        ],
    );
    let r = resolve_one(sh, did, &[]);
    assert!(r.features.lit);
    assert_eq!(r.features.blend.mode, BlendMode::Opaque);
    assert_eq!(r.features.blend.opacity_slot, None);
}

#[test]
fn opacity_sources_after_the_diffuse() {
    // 3020 `OpacityAlpha` (second choice of §11.2).
    let did = 0x2B00_0400;
    let sh = one_pass_shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(3020, V::Word(0x4000_0008), &[(10, 0, 1)], 0),
            entry(3018, V::Word(0x4000_0009), &[(10, 0, 2)], 0),
            entry(4004, V::Word(1), &[], 0),
            entry(1043, V::Const(0.3), &[], 0),
        ],
        &["Color", "OpacityAlpha", "EmissiveAndOpacity"],
        LIT_LIGHT,
    );
    let r = resolve_one(sh, did, &[]);
    assert_eq!(r.features.opacity_slot, Some(3020));
    let at = r.features.alpha_test.unwrap();
    assert_eq!((at.source_slot, at.threshold), (3020, 0.3));
    // 3018 `EmissiveAndOpacity` (third choice), when 3020 is not an opacity mask.
    let did = 0x2B00_0401;
    let sh = one_pass_shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(3018, V::Word(0x4000_0009), &[(10, 0, 1)], 0),
            entry(4004, V::Word(1), &[], 0),
            entry(1043, V::Const(0.6), &[], 0),
        ],
        &["ColorAndGloss", "EmissiveAndOpacity"],
        LIT_LIGHT,
    );
    let r = resolve_one(sh, did, &[]);
    assert_eq!(r.features.opacity_slot, Some(3018));
    assert_eq!(r.features.alpha_test.unwrap().source_slot, 3018);
    assert!(r.features.emissive);
}

#[test]
fn alpha_threshold_fallbacks() {
    let decls = |threshold: Option<f32>| {
        let mut d = vec![
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(4004, V::Word(1), &[], 0),
        ];
        if let Some(t) = threshold {
            d.push(entry(1043, V::Const(t), &[], 0));
        }
        d
    };
    let state = |aref: f32| (5, 2, 1, 0, 2, aref);
    // No 1043: the render-state alpha_ref.
    let did = 0x2B00_0500;
    let r = resolve_one(
        one_pass_shader(did, &decls(None), &["ColorAndOpacity"], state(0.9)),
        did,
        &[],
    );
    assert_eq!(r.features.alpha_test.unwrap().threshold, 0.9);
    // A NaN 1043: the render-state alpha_ref too.
    let did = 0x2B00_0501;
    let r = resolve_one(
        one_pass_shader(
            did,
            &decls(Some(f32::NAN)),
            &["ColorAndOpacity"],
            state(0.7),
        ),
        did,
        &[],
    );
    assert_eq!(r.features.alpha_test.unwrap().threshold, 0.7);
    // Neither usable: the fallback.
    let did = 0x2B00_0502;
    let r = resolve_one(
        one_pass_shader(
            did,
            &decls(None),
            &["ColorAndOpacity"],
            state(f32::INFINITY),
        ),
        did,
        &[],
    );
    assert_eq!(
        r.features.alpha_test.unwrap().threshold,
        FALLBACK_ALPHA_THRESHOLD
    );
}

#[test]
fn unlit_one_zero_with_alpha_test_is_cutout() {
    let did = 0x2B00_0600;
    let sh = one_pass_shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(4004, V::Word(1), &[], 0),
            entry(1043, V::Const(0.5), &[], 0),
        ],
        &["ColorAndOpacity"],
        (2, 1, 1, 1, 2, 0.9),
    );
    let r = resolve_one(sh, did, &[]);
    assert!(!r.features.lit);
    assert_eq!(r.features.blend.mode, BlendMode::Cutout);
    assert!(!r.features.blend.is_transparent());
    assert_eq!(r.features.alpha_test.unwrap().source_slot, 3000);
}

#[test]
fn normal_dye_mask_keeps_the_opacity_alpha() {
    // 3000 `ColorAndOpacity` (the texture_store diffuse: alpha 0 on the left half),
    // 3011 `TSNormalAndDye` (a 1×1 fully dyeable mask), dye colour 2003.
    let did = 0x2B00_0700;
    let sh = shader(
        did,
        &[
            entry(3000, V::Word(0x4000_0100), &[(10, 0, 0)], 0),
            entry(3011, V::Word(0x4000_0101), &[(10, 0, 1)], 0),
            entry(2003, V::Color([1.0; 4]), &[(4, 0, 0)], 0),
        ],
        &[
            Perm {
                programs: None,
                samplers: &["ColorAndOpacity", "TSNormalAndDye"],
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: None,
                samplers: &[],
                extra_type5: false,
                state: LIT_LIGHT,
            },
        ],
    );
    let mut store = texture_store();
    store.insert(did, sh);
    let lib = library(store);
    let r = lib.resolve(Did(did), &[]).unwrap();
    assert!(r.features.dyeable);
    let dye = r.dye.unwrap();
    assert_eq!(dye.primary.mask.slot, 3011);
    assert_eq!(dye.secondary, None);
    let colors = DyeColors {
        primary: Some([1.0, 0.0, 0.0, 1.0]),
        secondary: None,
    };
    let img = lib.dye_preview_with(&r, colors, None).unwrap().unwrap();
    // Dyed everywhere (the mask is fully dyeable); the diffuse alpha (opacity) is kept.
    assert_eq!(&img.pixels[..4], &[200, 0, 0, 0]);
    assert_eq!(&img.pixels[8..12], &[200, 0, 0, 255]);
}

// ---------------------------------------------------------------------------------------
// Audit regressions (2026-10-06): revised material.md §5.3, §8.5, §11.2–§11.4, §20.1;
// texture-list.md TL7
// ---------------------------------------------------------------------------------------

#[test]
fn texture_chain_stops_before_a_level_that_grows() {
    // The fuzz input of the audit: [1×1, 65×1] with a limit of 64. The second level grows
    // (TL7) and is above the limit; the chain ends before it.
    let mut s = Store::new();
    s.insert(
        0x4000_0001,
        texture_list(0x4000_0001, 2, &[0x4100_0001, 0x4100_0002]),
    );
    s.insert(
        0x4100_0001,
        surface_a8r8g8b8(0x4100_0001, 1, 1, |_, _| [0; 4]),
    );
    s.insert(
        0x4100_0002,
        surface_a8r8g8b8(0x4100_0002, 65, 1, |_, _| [0; 4]),
    );
    // A growing chain within the limit: [2×2, 4×4] ends after its first level too.
    s.insert(
        0x4000_0002,
        texture_list(0x4000_0002, 2, &[0x4100_0003, 0x4100_0004]),
    );
    s.insert(
        0x4100_0003,
        surface_a8r8g8b8(0x4100_0003, 2, 2, |_, _| [0; 4]),
    );
    s.insert(
        0x4100_0004,
        surface_a8r8g8b8(0x4100_0004, 4, 4, |_, _| [0; 4]),
    );
    // Two equal levels (texture-list.md §4.2 exception) are kept.
    s.insert(
        0x4000_0003,
        texture_list(0x4000_0003, 2, &[0x4100_0004, 0x4100_0004, 0x4100_0003]),
    );
    let lib = library_with(s, limit(64));
    for cap in [None, Some(16)] {
        let chain = lib.texture_chain(Did(0x4000_0001), cap).unwrap();
        let sizes: Vec<_> = chain
            .levels
            .iter()
            .map(|l| (l.index, l.level.width, l.level.height))
            .collect();
        assert_eq!(sizes, vec![(0, 1, 1)], "cap {cap:?}");
        let plan = chain.plan.unwrap();
        assert_eq!((plan.first, plan.uploadable, plan.stored), (0, 1, 1));
    }
    let chain = lib.texture_chain(Did(0x4000_0002), None).unwrap();
    assert_eq!(chain.levels.len(), 1);
    let chain = lib.texture_chain(Did(0x4000_0003), None).unwrap();
    assert_eq!(chain.levels.len(), 3);
}

/// A lit shader (base and light pass) with `decls` bound to `samplers` in its base pass.
fn lit_two_pass(did: u32, decls: &[Vec<u8>], samplers: &[&str]) -> Vec<u8> {
    shader(
        did,
        decls,
        &[
            Perm {
                programs: None,
                samplers,
                extra_type5: false,
                state: LIT_BASE,
            },
            Perm {
                programs: None,
                samplers,
                extra_type5: false,
                state: LIT_LIGHT,
            },
        ],
    )
}

#[test]
fn two_sided_is_flag_4003_not_4007() {
    // Every block culls (cull 2); 4003 DoubleSided and 4007 AlphaBlendPass declared.
    let did = 0x2B00_0800;
    let sh = || {
        lit_two_pass(
            did,
            &[
                entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
                entry(4003, V::Word(0), &[(5, 0, 0)], 0),
                entry(4007, V::Word(0), &[], 0),
            ],
            &["ColorAndOpacity"],
        )
    };
    let flags = |f4003: u32, f4007: u32| {
        material(
            0x3000_0001,
            &[
                entry(4003, V::Word(f4003), &[], 0),
                entry(4007, V::Word(f4007), &[], 0),
            ],
        )
    };
    let r = resolve_one(sh(), did, &[(0x3000_0001, flags(1, 0))]);
    assert!(r.features.two_sided, "4003 = 1 is two-sided (§11.4)");
    let r = resolve_one(sh(), did, &[(0x3000_0001, flags(0, 1))]);
    assert!(!r.features.two_sided, "4007 alone is not two-sided (§11.4)");
    let r = resolve_one(sh(), did, &[]);
    assert!(!r.features.two_sided);
}

#[test]
fn lit_surfaces_blend_by_opacity_and_material_alpha() {
    // (role, alpha test, 2000, expected mode, expected opacity slot)
    type Case<'a> = (&'a str, bool, [f32; 4], BlendMode, Option<u32>);
    let cases: [Case<'_>; 6] = [
        // (a) ColorAndOpacity without alpha test: translucent by texture alpha.
        (
            "ColorAndOpacity",
            false,
            [1.0; 4],
            BlendMode::AlphaBlend,
            Some(3000),
        ),
        // (b) An opaque role with 2000 alpha 0.5: translucent.
        (
            "ColorAndGloss",
            false,
            [1.0, 1.0, 1.0, 0.5],
            BlendMode::AlphaBlend,
            None,
        ),
        // (c) 2000 = (0, 0, 0, 0): blended (invisible), not opaque black.
        ("Color", false, [0.0; 4], BlendMode::AlphaBlend, None),
        // (d) Alpha-tested ColorAndOpacity: transparent pass, keeping the alpha test.
        (
            "ColorAndOpacity",
            true,
            [1.0; 4],
            BlendMode::AlphaBlend,
            Some(3000),
        ),
        // Opaque role and 2000 alpha 1: opaque by construction.
        ("ColorAndGloss", false, [1.0; 4], BlendMode::Opaque, None),
        ("ColorAndDye", true, [1.0; 4], BlendMode::Opaque, None),
    ];
    for (i, (role, test, color, mode, slot)) in cases.into_iter().enumerate() {
        let did = 0x2B00_0900 + i as u32;
        let sh = lit_two_pass(
            did,
            &[
                entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
                entry(4004, V::Word(u32::from(test)), &[(8, 0, 0)], 0),
                entry(1043, V::Const(0.5), &[(9, 0, 0)], 0),
                entry(2000, V::Color(color), &[(0, 0, 0)], 0),
            ],
            &[role],
        );
        let r = resolve_one(sh, did, &[]);
        let b = r.features.blend;
        assert!(r.features.lit);
        assert_eq!(r.material_alpha(), color[3]);
        assert_eq!(b.mode, mode, "case {i} ({role})");
        assert_eq!(
            (b.src, b.dst),
            (BlendFactor::SrcAlpha, BlendFactor::InvSrcAlpha),
            "case {i}"
        );
        assert!(b.depth_write);
        assert_eq!(b.opacity_slot, slot, "case {i} ({role})");
        assert_eq!(b.is_transparent(), mode != BlendMode::Opaque);
        assert_eq!(r.features.alpha_test.is_some(), test && slot.is_some());
    }
}

#[test]
fn depth_write_flag_overrides_the_render_state() {
    // Lit, and unlit additive with byte 4 = 1: effective 4002 = 0 turns depth write off.
    let decls = || {
        vec![
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(4001, V::Word(1), &[(6, 0, 0)], 0),
            entry(4002, V::Word(1), &[(7, 0, 0)], 0),
        ]
    };
    let off = || {
        material(
            0x3000_0001,
            &[
                entry(4001, V::Word(0), &[], 0),
                entry(4002, V::Word(0), &[], 0),
            ],
        )
    };
    let did = 0x2B00_0A00;
    let r = resolve_one(lit_two_pass(did, &decls(), &["Color"]), did, &[]);
    assert!(r.features.blend.depth_write);
    assert!(r.depth_test());
    let r = resolve_one(
        lit_two_pass(did, &decls(), &["Color"]),
        did,
        &[(0x3000_0001, off())],
    );
    assert!(!r.features.blend.depth_write);
    // 4001 DepthTest = 0: no depth test (§8.5).
    assert!(!r.depth_test());
    let did = 0x2B00_0A01;
    let add = (5, 2, 1, 1, 2, 0.9);
    let r = resolve_one(one_pass_shader(did, &decls(), &["Color"], add), did, &[]);
    assert!(r.features.blend.depth_write);
    let r = resolve_one(
        one_pass_shader(did, &decls(), &["Color"], add),
        did,
        &[(0x3000_0001, off())],
    );
    assert!(!r.features.blend.depth_write);
}

#[test]
fn wrap_flags_override_the_stored_address() {
    // 3000 is bound to sampler 0 (stored wrap); 4005 / 4006 name the same sampler.
    let did = 0x2B00_0B00;
    let sh = || {
        one_pass_shader(
            did,
            &[
                entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
                entry(3011, V::Word(0x4000_000D), &[(10, 0, 1)], 0),
                entry(4005, V::Word(1), &[(11, 0, 0)], 0),
                entry(4006, V::Word(1), &[(12, 0, 0)], 0),
            ],
            &["Color", "TSNormal"],
            (2, 1, 1, 1, 2, 0.9),
        )
    };
    let r = resolve_one(sh(), did, &[]);
    let s = r.slot(3000).unwrap().sampler.unwrap();
    assert_eq!(
        (s.address_u(), s.address_v()),
        (AddressMode::Wrap, AddressMode::Wrap)
    );
    let clamp_u = material(0x3000_0001, &[entry(4005, V::Word(0), &[], 0)]);
    let r = resolve_one(sh(), did, &[(0x3000_0001, clamp_u)]);
    let s = r.slot(3000).unwrap().sampler.unwrap();
    assert_eq!(
        (s.address_u(), s.address_v()),
        (AddressMode::Clamp, AddressMode::Wrap)
    );
    // The normal map's sampler (index 1) is not named by the flags: stored bytes.
    let n = r.slot(3011).unwrap().sampler.unwrap();
    assert_eq!(n.address_u(), AddressMode::Wrap);
}

#[test]
fn mode_10_loop_byte_above_1_is_rejected() {
    let keyed = |flag| V::Keyed([0.0; 11], flag, vec![(0.0, 0.0), (1.0, 100.0)]);
    for flag in [0u8, 1] {
        let m = material(0x3000_0001, &[entry(1017, keyed(flag), &[], 0)]);
        assert!(MaterialRecord::parse(&m).is_ok(), "loop {flag}");
    }
    for flag in [2u8, 0xFF] {
        let m = material(0x3000_0001, &[entry(1017, keyed(flag), &[], 0)]);
        assert!(
            matches!(
                MaterialRecord::parse(&m),
                Err(FormatError::Malformed { .. })
            ),
            "loop {flag}"
        );
    }
}

#[test]
fn float_curves_evaluate_at_time_zero() {
    let curve = |v: V| {
        let m = MaterialRecord::parse(&material(0x3000_0001, &[entry(1017, v, &[], 0)])).unwrap();
        m.get(1017).unwrap().value.as_float().unwrap()
    };
    // Mode 2 at t = 0: B + π·w[4] (§5.3.1).
    let v = curve(V::Wave(2, [0.1, 0., 0., 0., 0.25, 0., 0., 0., 0., 0.]));
    assert!(
        (v - (0.1 + std::f32::consts::PI * 0.25)).abs() < 1e-6,
        "{v}"
    );
    // Mode 10 at its start: p = 0.1 between (0, 0) and (1, 50): 0.002; w[0] is unused.
    let v = curve(V::Keyed(
        [0.5, 0., 0., 0., 0., 0., 0., 0., 0., 0., 4.0],
        1,
        vec![(0.0, 0.0), (1.0, 50.0), (0.0, 100.0)],
    ));
    assert!((v - 0.002).abs() < 1e-6, "{v}");
}

#[test]
fn second_colour_layer_multiplies_the_opacity() {
    // 3000 `ColorAndOpacity` + 3001 `ColorAndOpacity2`: the opacity is the product of
    // both alphas (§11.2), for the alpha test and for blending.
    let decls = |second: u32| {
        vec![
            entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
            entry(3001, V::Word(second), &[(10, 0, 1)], 0),
            entry(4004, V::Word(1), &[(8, 0, 0)], 0),
            entry(1043, V::Const(0.5), &[(9, 0, 0)], 0),
        ]
    };
    let did = 0x2B00_0C00;
    let roles = ["ColorAndOpacity", "ColorAndOpacity2"];
    let r = resolve_one(lit_two_pass(did, &decls(0x4000_0008), &roles), did, &[]);
    assert_eq!(r.features.alpha_test.unwrap().source_slot, 3000);
    assert_eq!(r.second_opacity_slot(), Some(3001));
    // No second texture: no second layer.
    let r = resolve_one(lit_two_pass(did, &decls(0), &roles), did, &[]);
    assert_eq!(r.second_opacity_slot(), None);
    // A second layer that is not an opacity layer (`ColorAndDye2`): not multiplied.
    let r = resolve_one(
        lit_two_pass(
            did,
            &decls(0x4000_0008),
            &["ColorAndOpacity", "ColorAndDye2"],
        ),
        did,
        &[],
    );
    assert_eq!(r.second_opacity_slot(), None);
    // Unlit `ColorTexture` diffuse blending by source alpha: both layers' alphas (§20.1).
    let alpha = (5, 6, 1, 0, 2, 0.9);
    let unlit = |role: &str| {
        one_pass_shader(
            did,
            &[
                entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
                entry(3001, V::Word(0x4000_0008), &[(10, 0, 1)], 0),
            ],
            &[role, "Color2"],
            alpha,
        )
    };
    let r = resolve_one(unlit("ColorTexture"), did, &[]);
    assert_eq!(r.features.blend.opacity_slot, Some(3000));
    assert_eq!(r.second_opacity_slot(), Some(3001));
    // `Color`: the 2000 alpha only, no texture alpha.
    let r = resolve_one(unlit("Color"), did, &[]);
    assert_eq!(r.features.blend.opacity_slot, None);
    assert_eq!(r.second_opacity_slot(), None);
}

/// `material.md` §10.3 MA54 and §19 answers (b), (d): only a `ColorAndOpacity2` layer's
/// alpha joins the opacity; a `Color2` layer after `ColorAndOpacity` (as in 0x2B0007E6)
/// leaves the first layer's alpha alone; the `ColorTexture` record takes the second layer
/// for its alpha only.
#[test]
fn second_layer_plan_by_role() {
    use tessera_material::{LayerAlpha, LayerCombine, second_layer_plan};
    let decls = vec![
        entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0),
        entry(3001, V::Word(0x4000_0008), &[(10, 0, 1)], 0),
    ];
    let did = 0x2B00_0C00;
    let plan = |roles: &[&str]| {
        let r = resolve_one(lit_two_pass(did, &decls, roles), did, &[]);
        (second_layer_plan(&r), r.second_opacity_slot())
    };
    let (p, slot) = plan(&["ColorAndOpacity", "ColorAndOpacity2"]);
    let p = p.expect("second layer");
    assert_eq!(
        (p.texture.0, p.combine, p.alpha),
        (0x4000_0008, LayerCombine::Multiply, LayerAlpha::Multiplied)
    );
    assert_eq!(slot, Some(3001));
    for second in ["Color2", "ColorAndGloss2", "ColorAndDye2"] {
        let (p, slot) = plan(&["ColorAndOpacity", second]);
        assert_eq!(p.map(|p| p.alpha), Some(LayerAlpha::Ignored), "{second}");
        assert_eq!(slot, None, "{second}");
    }
    let unlit = one_pass_shader(
        did,
        &decls,
        &["ColorTexture", "ColorTexture2"],
        (5, 6, 1, 0, 2, 0.9),
    );
    let r = resolve_one(unlit, did, &[]);
    let p = second_layer_plan(&r).expect("ColorTexture layer");
    assert_eq!(
        (p.texture.0, p.alpha),
        (0x4000_0008, LayerAlpha::OpacityOnly)
    );
}

// ---------------------------------------------------------------------------------------
// Program families (§11.6) and the fallback diagnosis
// ---------------------------------------------------------------------------------------

/// A one-permutation unlit shader declaring `decls` with samplers `samplers`.
fn family_shader(did: u32, decls: &[Vec<u8>], samplers: &[&str]) -> Vec<u8> {
    shader(
        did,
        decls,
        &[Perm {
            programs: None,
            samplers,
            extra_type5: false,
            state: (5, 6, 1, 1, 2, 0.5),
        }],
    )
}

#[test]
fn program_families_from_the_interface() {
    use tessera_material::interface::ProgramFamily;
    let family = |decls: &[Vec<u8>], samplers: &[&str]| {
        let bytes = family_shader(0x2B00_0042, decls, samplers);
        let rec = ShaderRecord::parse_expecting(&bytes, Did(0x2B00_0042)).unwrap();
        ShaderInterface::from_record(&rec).family
    };
    let speed = entry(1057, V::Const(0.005), &[], 1);
    let bump = entry(1056, V::Const(0.003), &[], 1);
    let mirror = entry(1101, V::Const(1.33), &[], 1);
    let colour = entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0);
    let water_colour = entry(2009, V::Color([1.0; 4]), &[], 1);
    // Water: 1057 or 1056 with a frame-buffer sampler.
    assert_eq!(
        family(
            &[speed.clone(), colour.clone()],
            &["Color", "FrameBufferTexture"]
        ),
        ProgramFamily::Water
    );
    assert_eq!(
        family(&[bump], &["FrameBufferTexture"]),
        ProgramFamily::Water
    );
    // Mirror: any of 1100-1106.
    assert_eq!(
        family(&[mirror, colour.clone()], &["DiffuseTexture"]),
        ProgramFamily::Mirror
    );
    // Liquid distortion: 1057, no frame buffer, a liquid-normal or distortion sampler.
    assert_eq!(
        family(
            &[speed.clone(), colour.clone()],
            &["LiquidDiffuseTexture", "LiquidNormalTexture"]
        ),
        ProgramFamily::LiquidDistortion
    );
    assert_eq!(
        family(
            std::slice::from_ref(&speed),
            &["ColorAndOpacity", "Distortion"]
        ),
        ProgramFamily::LiquidDistortion
    );
    // 1057 with neither sampler (the lava case) and a frame buffer without 1056/1057 (the
    // window case) stay generic.
    assert_eq!(family(&[speed], &["Color"]), ProgramFamily::Generic);
    assert_eq!(
        family(&[water_colour, colour], &["Color", "FrameBufferTexture"]),
        ProgramFamily::Generic
    );
}

#[test]
fn fallback_diagnosis_names_the_failing_step() {
    use tessera_material::diagnose::{base_reason, reason_of_texture_error};
    use tessera_material::{FallbackReason as R, MaterialKind};
    let mut s = texture_store();
    // An unlit shader without slot 3000, and one whose 3000 a material sets to 0.
    s.insert(
        0x2B00_0050,
        family_shader(0x2B00_0050, &[entry(2000, V::Color([1.0; 4]), &[], 0)], &[]),
    );
    s.insert(
        0x2B00_0051,
        family_shader(
            0x2B00_0051,
            &[entry(3000, V::Word(0x4000_0007), &[(10, 0, 0)], 0)],
            &["Color"],
        ),
    );
    // A water record whose 3000 names a missing texture list.
    s.insert(
        0x2B00_0052,
        family_shader(
            0x2B00_0052,
            &[
                entry(1057, V::Const(0.005), &[], 1),
                entry(3000, V::Word(0x4000_0FFF), &[(10, 0, 0)], 0),
            ],
            &["Color", "FrameBufferTexture"],
        ),
    );
    s.insert(
        0x3000_0050,
        material(0x3000_0050, &[entry(3000, V::Word(0), &[], 0)]),
    );
    s.insert(
        0x3100_0050,
        surface_record(0x3100_0050, 0x2B00_0050, 1, &[]),
    );
    s.insert(
        0x3100_0051,
        surface_record(0x3100_0051, 0x2B00_0051, 1, &[0x3000_0050]),
    );
    s.insert(
        0x3100_0052,
        surface_record(0x3100_0052, 0x2B00_00FF, 1, &[]),
    );
    s.insert(
        0x3100_0053,
        surface_record(0x3100_0053, 0x2B00_0052, 1, &[]),
    );
    // A list none of whose levels resolves (0x4000_0300 of the texture store).
    s.insert(
        0x3000_0051,
        material(0x3000_0051, &[entry(3000, V::Word(0x4000_0300), &[], 0)]),
    );
    s.insert(
        0x3100_0054,
        surface_record(0x3100_0054, 0x2B00_0051, 1, &[0x3000_0051]),
    );
    let lib = library(s);
    let d = |did: u32, decode: bool| lib.diagnose_surface(Did(did), Some(64), decode);
    assert_eq!(d(0, true).fallback, Some(R::NoSurface));
    assert_eq!(d(0x3100_00F0, true).fallback, Some(R::SurfaceMissing));
    assert_eq!(d(0x3100_00F0, true).kind, MaterialKind::Unresolved);
    let shaderless = d(0x3100_0052, true);
    assert_eq!(shaderless.fallback, Some(R::ShaderMissing));
    assert_eq!(shaderless.shader, Some(Did(0x2B00_00FF)));
    assert_eq!(d(0x3100_0050, true).fallback, Some(R::NoBaseSlot));
    assert_eq!(d(0x3100_0050, true).kind, MaterialKind::Unlit);
    assert_eq!(d(0x3100_0051, true).fallback, Some(R::BaseSlotEmpty));
    let water = d(0x3100_0053, false);
    assert_eq!(water.kind, MaterialKind::Water);
    assert!(water.kind.needs_own_program());
    assert_eq!(
        water.fallback, None,
        "not decoded: the slot has a texture list"
    );
    assert_eq!(water.base_texture, Some(Did(0x4000_0FFF)));
    assert_eq!(d(0x3100_0053, true).fallback, Some(R::TextureListMissing));
    assert_eq!(d(0x3100_0054, true).fallback, Some(R::TextureLevelsMissing));
    assert!(d(0x3100_0054, true).error.is_some());
    let m = lib.resolve_surface(Did(0x3100_0051)).unwrap();
    assert_eq!(base_reason(&m), Some(R::BaseSlotEmpty));
    let e = lib.texture(Did(0x4000_0FFF), None).unwrap_err();
    assert_eq!(reason_of_texture_error(&e), R::TextureListMissing);
}

// ---------------------------------------------------------------------------------------
// Run-time edits and textures (appearance.md §3.5, §8 step 6)
// ---------------------------------------------------------------------------------------

#[test]
fn edits_apply_after_every_material() {
    use tessera_material::{ParameterEdit, RUNTIME_TEXTURE_PREFIX};
    let lib = library(base_store());
    // The dyeable surface with the blue dye material layered, then a colour edit of 2003
    // and a texture edit of 3000: both win over the materials.
    let layer = Layering {
        shader_override: None,
        materials: vec![Did(0x3000_0003)],
    };
    let image = std::sync::Arc::new(tessera_formats::RgbaImage {
        width: 2,
        height: 1,
        pixels: vec![10, 20, 30, 255, 40, 50, 60, 255],
    });
    let tex = lib
        .runtime_textures()
        .register(0x1234_5678_9ABC, std::sync::Arc::clone(&image))
        .unwrap();
    assert_eq!(tex.prefix(), RUNTIME_TEXTURE_PREFIX);
    // The same identity gives the same DID.
    assert_eq!(
        lib.runtime_textures().register(0x1234_5678_9ABC, image),
        Some(tex)
    );
    let edits = [
        ParameterEdit::Colour {
            id: 2003,
            value: [0.5, 0.25, 0.0, 1.0],
        },
        ParameterEdit::Texture {
            id: 3000,
            list: tex,
        },
        // Outside its value's group: ignored.
        ParameterEdit::Colour {
            id: 3000,
            value: [0.0; 4],
        },
    ];
    let r = lib
        .resolve_edited(Did(0x3100_0004), Some(&layer), &edits)
        .unwrap();
    assert_eq!(r.surface, Some(Did(0x3100_0004)));
    assert_eq!(r.materials, vec![Did(0x3000_0003)], "stored materials only");
    assert_eq!(r.dye.unwrap().primary.color, [0.5, 0.25, 0.0, 1.0]);
    assert_eq!(r.diffuse(), Some(tex));
    // The library answers the run-time texture.
    let t = lib.texture(tex, Some(1)).unwrap();
    assert_eq!((t.image.width, t.image.height), (2, 1));
    // Without edits the layered result is unchanged.
    let plain = lib
        .resolve_edited(Did(0x3100_0004), Some(&layer), &[])
        .unwrap();
    assert_eq!(
        plain,
        lib.resolve_layered(Did(0x3100_0004), &layer).unwrap()
    );
    assert_eq!(plain.dye.unwrap().primary.color, [0.0, 0.0, 1.0, 1.0]);
    // An id the shader does not declare changes nothing.
    let undeclared = lib
        .resolve_edited(
            Did(0x3100_0004),
            None,
            &[ParameterEdit::Colour {
                id: 2999,
                value: [0.0; 4],
            }],
        )
        .unwrap();
    assert_eq!(undeclared.dye.unwrap().primary.color, [1.0; 4]);
}

/// `appearance.md` §3.5 "How material edits combine": the composed image comes before the
/// edits, so an edit of the base colour overrides it; float and flag edits apply too.
#[test]
fn figure_edits_put_the_composed_image_first() {
    use tessera_material::{ParameterEdit, figure_edits};
    let lib = library(base_store());
    let image = std::sync::Arc::new(tessera_formats::RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![1, 2, 3, 255],
    });
    let composed = figure_edits(lib.runtime_textures(), &[], Some((7, &image)));
    assert_eq!(composed.len(), 1);
    let face = composed[0].clone();
    let other = Did(0x0500_0042);
    let edits = figure_edits(
        lib.runtime_textures(),
        &[
            ParameterEdit::Texture {
                id: 3000,
                list: other,
            },
            ParameterEdit::Flag {
                id: 4001,
                value: true,
            },
            ParameterEdit::Float {
                id: 1001,
                curve: tessera_material::FloatCurve::constant(0.5),
            },
        ],
        Some((7, &image)),
    );
    assert_eq!(edits.len(), 4);
    assert_eq!(edits[0], face);
    let r = lib.resolve_edited(Did(0x3100_0004), None, &edits).unwrap();
    assert_eq!(r.diffuse(), Some(other));
}

#[test]
fn waveforms_become_float_curves() {
    use tessera_formats::{Waveform, WaveformExtra};
    use tessera_material::FloatCurve;
    let w = |subtype: u32, values: Vec<f32>, extra: Option<WaveformExtra>| Waveform {
        subtype,
        values,
        extra,
    };
    let c = FloatCurve::from_waveform(&w(1, vec![0.25], None));
    assert_eq!((c.mode, c.evaluate()), (1, 0.25));
    let ten: Vec<f32> = (0..10u8).map(f32::from).collect();
    let c = FloatCurve::from_waveform(&w(4, ten.clone(), None));
    assert_eq!((c.mode, c.stored_params()), (4, ten.as_slice()));
    // Subtype 0: the base term v0 + v1 t only.
    let c = FloatCurve::from_waveform(&w(0, ten.clone(), None));
    assert_eq!(c.mode, 2);
    assert_eq!(c.stored_params()[..3], [0.0, 1.0, 0.0]);
    let c = FloatCurve::from_waveform(&w(
        10,
        ten,
        Some(WaveformExtra {
            scalar: 2.0,
            flag: true,
            pairs: vec![(1.0, 0.0), (3.0, 100.0)],
        }),
    ));
    assert_eq!((c.mode, c.period(), c.loop_flag), (10, 2.0, 1));
    assert_eq!(c.keys, [(1.0, 0.0), (3.0, 100.0)]);
}

#[test]
fn runtime_textures_are_bounded() {
    use tessera_material::RuntimeTextures;
    let reg = RuntimeTextures::default();
    let img = |n: u8| {
        std::sync::Arc::new(tessera_formats::RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![n, n, n, 255],
        })
    };
    // Identities whose low 24 bits collide get distinct DIDs.
    let a = reg.register(0x0100_0005, img(1)).unwrap();
    let b = reg.register(0x0200_0005, img(2)).unwrap();
    assert_ne!(a, b);
    assert_eq!(reg.get(a).unwrap().pixels[0], 1);
    assert_eq!(reg.get(b).unwrap().pixels[0], 2);
    // An inconsistent image is refused.
    let bad = std::sync::Arc::new(tessera_formats::RgbaImage {
        width: 4,
        height: 4,
        pixels: vec![0; 3],
    });
    assert_eq!(reg.register(7, bad), None);
    // The least recently used entries go first once the entry cap is reached: `b` is
    // registered again (a hit) after `a`, so `a` goes first.
    assert_eq!(reg.register(0x0200_0005, img(2)), Some(b));
    let mut issued = vec![a, b];
    for i in 0..RuntimeTextures::MAX_ENTRIES as u64 - 1 {
        issued.push(reg.register(0x10_0000_0000 + i, img(3)).unwrap());
    }
    assert_eq!(reg.len(), RuntimeTextures::MAX_ENTRIES);
    assert!(reg.get(a).is_none());
    assert_eq!(reg.get(b).unwrap().pixels[0], 2);
    // A request counts as a use too: `b` was just requested, so the next registration
    // evicts the oldest other entry, not `b`.
    issued.push(reg.register(0x20_0000_0000, img(4)).unwrap());
    assert!(reg.get(b).is_some());
    // DIDs are never handed out twice, not even after their entry was forgotten.
    let mut sorted = issued.clone();
    sorted.sort_by_key(|d| d.0);
    sorted.dedup();
    assert_eq!(sorted.len(), issued.len());
    // `a`'s identity registered again gets a new DID.
    let again = reg.register(0x0100_0005, img(1)).unwrap();
    assert!(!issued.contains(&again));
    // Not a run-time DID.
    assert!(reg.get(Did(0x4000_0001)).is_none());
}
