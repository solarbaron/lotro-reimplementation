// SPDX-License-Identifier: GPL-3.0-or-later
//! Texture lists with synthetic surfaces (`docs/specs/texture-list.md` §2–§5, fixtures §10).

mod common;

use std::collections::HashMap;

use common::B;
use tessera_formats::{
    AnyRecord, Did, FormatError, Record, Surface, SurfaceFormat, TextureList, TextureListKind,
    cap_chain_resolution, decode_texture_list,
};

const LIST: u32 = 0x40AB_CDEF;

fn list(kind: u8, level_count: u8, flags: u8, dids: &[u32]) -> Vec<u8> {
    let mut b = B::new().u32(LIST).u8(kind).u8(level_count).u8(flags).u16(0);
    for &d in dids {
        b = b.u32(d);
    }
    b.u32(0).u32(15).build()
}

fn surface(did: u32, format: SurfaceFormat, w: u32, h: u32) -> Vec<u8> {
    let len = format.expected_len(w, h).unwrap() as u32;
    B::new()
        .u32(did)
        .u32(0)
        .u32(w)
        .u32(h)
        .u32(format.code())
        .u32(len)
        .bytes(&vec![0x5A; len as usize])
        .build()
}

/// A synthetic archive: surface DID → payload.
fn archive(surfaces: &[(u32, SurfaceFormat, u32, u32)]) -> HashMap<Did, Vec<u8>> {
    surfaces
        .iter()
        .map(|&(d, f, w, h)| (Did(d), surface(d, f, w, h)))
        .collect()
}

#[test]
fn three_level_dxt1_chain_resolves_and_decodes() {
    let dids = [0x41AB_C000, 0x41AB_C001, 0x41AB_C002];
    let data = list(2, 3, 0, &dids);
    let t = TextureList::parse_expecting(&data, LIST).unwrap();
    let store = archive(&[
        (dids[0], SurfaceFormat::Dxt1, 16, 16),
        (dids[1], SurfaceFormat::Dxt1, 8, 8),
        (dids[2], SurfaceFormat::Dxt1, 4, 4),
    ]);
    let chain = t.effective_chain(|d| {
        let payload = store.get(&d)?;
        let s = Surface::parse_expecting(payload, d).ok()?;
        s.to_rgba8().ok().map(|img| (img.width, img.height))
    });
    let sizes: Vec<_> = chain.iter().map(|l| l.level).collect();
    assert_eq!(sizes, vec![(16, 16), (8, 8), (4, 4)]);
    // TL7: non-increasing sizes.
    assert!(
        sizes
            .windows(2)
            .all(|w| w[1].0 <= w[0].0 && w[1].1 <= w[0].1)
    );

    let capped = cap_chain_resolution(chain, 8, |&(w, h)| (w, h));
    assert_eq!(capped.first().map(|l| l.index), Some(1));
}

#[test]
fn single_level_a8r8g8b8() {
    let t = decode_texture_list(&list(2, 1, 0, &[0x4100_0001])).unwrap();
    let store = archive(&[(0x4100_0001, SurfaceFormat::A8R8G8B8, 3, 5)]);
    let chain = t.effective_chain(|d| store.get(&d).cloned());
    assert_eq!(chain.len(), 1);
    assert_eq!(t.levels().unwrap().len(), 1);
}

#[test]
fn cube_maps() {
    let faces: Vec<u32> = (0..6).map(|i| 0x4100_0100 + i).collect();
    let t = TextureList::parse(&list(4, 1, 0, &faces)).unwrap();
    assert_eq!(t.kind, TextureListKind::CubeMap);
    let f = t.cube_faces().unwrap();
    assert_eq!(f.iter().map(|d| d.0).collect::<Vec<_>>(), faces);

    let repeated = [0x4100_0200u32; 6];
    let t = TextureList::parse(&list(4, 1, 0, &repeated)).unwrap();
    assert!(
        t.cube_faces()
            .unwrap()
            .iter()
            .all(|&d| d == Did(0x4100_0200))
    );

    // A two-level cube stores 12 DIDs; faces are not split because the order is OPEN.
    let twelve: Vec<u32> = (0..12).map(|i| 0x4100_0300 + i).collect();
    let t = TextureList::parse(&list(4, 2, 0, &twelve)).unwrap();
    assert_eq!(t.surfaces.len(), 12);
    assert!(t.cube_faces().is_none());

    // Kind 4 with only n DIDs is short by 5·n.
    assert!(matches!(
        TextureList::parse(&list(4, 1, 0, &faces[..1])),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn length_must_match_level_count() {
    let short = list(2, 3, 0, &[0x4100_0001, 0x4100_0002]);
    assert!(matches!(
        TextureList::parse(&short),
        Err(FormatError::Truncated { .. })
    ));
    let long = list(2, 1, 0, &[0x4100_0001, 0x4100_0002]);
    assert!(matches!(
        TextureList::parse(&long),
        Err(FormatError::TrailingBytes { count: 4, .. })
    ));
    let full = list(2, 2, 0, &[0x4100_0001, 0x4100_0002]);
    for n in 0..full.len() {
        assert!(TextureList::parse(&full[..n]).is_err(), "len {n}");
    }
}

#[test]
fn unusual_fields_decode() {
    let t = TextureList::parse(&list(3, 1, 0, &[0x4100_0001])).unwrap();
    assert_eq!(t.kind, TextureListKind::Other(3));
    assert!(t.has_unusual_fields());
    assert_eq!(t.levels().unwrap().len(), 1);

    let t = TextureList::parse(&list(2, 1, 2, &[0x4100_0001])).unwrap();
    assert_eq!(t.flags, 2);
    assert!(t.has_unusual_fields());

    let mut tails = list(2, 1, 1, &[0x4100_0001]);
    let n = tails.len();
    tails[n - 4] = 16;
    let t = TextureList::parse(&tails).unwrap();
    assert_eq!((t.flags, t.tail_b), (1, 16));
    assert!(t.has_unusual_fields());

    // Zero levels: a valid 17-byte record with no surfaces.
    let t = TextureList::parse(&list(2, 0, 0, &[])).unwrap();
    assert!(t.surfaces.is_empty());
    assert!(t.effective_chain(|_| Some(())).is_empty());
}

#[test]
fn missing_level_zero_and_broken_halving() {
    let dids = [0x4100_0010, 0x4100_0011, 0x4100_0012];
    let t = TextureList::parse(&list(2, 3, 0, &dids)).unwrap();
    // Level 0 lives in an absent highres archive; level 1 does not halve from it but is
    // accepted with its own size.
    let store = archive(&[
        (dids[1], SurfaceFormat::Dxt5, 64, 64),
        (dids[2], SurfaceFormat::Dxt5, 64, 32),
    ]);
    let chain = t.effective_chain(|d| {
        let s = Surface::parse(store.get(&d)?).ok()?;
        Some((s.width, s.height))
    });
    assert_eq!(
        chain.iter().map(|l| (l.index, l.level)).collect::<Vec<_>>(),
        vec![(1, (64, 64)), (2, (64, 32))]
    );
}

#[test]
fn zero_did_ends_the_chain() {
    let t = TextureList::parse(&list(2, 3, 0, &[0x4100_0001, 0, 0x4100_0003])).unwrap();
    let mut asked = Vec::new();
    let chain = t.effective_chain(|d| {
        asked.push(d.0);
        Some(())
    });
    assert_eq!(chain.len(), 1);
    assert_eq!(asked, vec![0x4100_0001]);
}

#[test]
fn any_record_dispatch() {
    let data = list(2, 1, 0, &[0x4100_0001]);
    let rec = AnyRecord::decode(LIST, &data).unwrap().unwrap();
    assert!(matches!(rec, AnyRecord::TextureList(_)));
    assert_eq!(rec.type_name(), "texture list");
    assert!(AnyRecord::is_decodable(LIST));
    assert!(matches!(
        AnyRecord::decode(0x40AB_CDEEu32, &data),
        Err(FormatError::DidMismatch { .. })
    ));
}
