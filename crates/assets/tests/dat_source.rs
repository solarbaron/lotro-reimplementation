// SPDX-License-Identifier: GPL-3.0-or-later
//! The pipeline over real `tessera-dat` archives built from synthetic bytes.
//!
//! The archives are written by the synthetic archive writer of `tessera-dat`'s own tests
//! (included by path, not copied), so no game data is involved.

#[path = "../../dat/tests/common/mod.rs"]
mod datkit;

use std::time::Duration;

use flate2::Compression;
use tessera_assets::{
    AssetError, AssetState, AssetStore, PoolConfig, Priority, RawBytes, StoreConfig, WorkerPool,
};
use tessera_dat::{Archive, ArchiveOptions};

const T: Duration = Duration::from_secs(20);

/// A 0x41 render-surface record: DID, category, width, height, format, length, data.
fn surface_record(did: u32, w: u32, h: u32) -> Vec<u8> {
    let data: Vec<u8> = (0..w * h * 4).map(|i| (i ^ did) as u8).collect();
    let mut rec = Vec::new();
    for v in [did, 0, w, h, 21, data.len() as u32] {
        rec.extend_from_slice(&v.to_le_bytes());
    }
    rec.extend_from_slice(&data);
    rec
}

fn build() -> (datkit::Built, Vec<(u32, Vec<u8>)>) {
    let mut b = datkit::Builder::new();
    let mut expect = Vec::new();
    for i in 0..300u32 {
        let did = 0x4100_0000 + i * 3;
        let rec = surface_record(did, 4 + i % 5, 2 + i % 3);
        if i % 2 == 0 {
            b.compressed(did, &rec, Compression::fast());
        } else {
            b.stored(did, rec.clone());
        }
        expect.push((did, rec));
    }
    (b.build(), expect)
}

#[test]
fn reads_every_record_of_a_synthetic_archive() {
    let (built, expect) = build();
    let archive = Archive::from_bytes(built.bytes, ArchiveOptions::default()).unwrap();
    let pool = WorkerPool::new(PoolConfig {
        workers: 4,
        ..PoolConfig::default()
    });
    let store = AssetStore::new(&pool, archive, RawBytes, StoreConfig::default());
    let handles: Vec<_> = expect
        .iter()
        .map(|(did, _)| store.request(*did, Priority::Visible))
        .collect();
    for (h, (did, rec)) in handles.iter().zip(&expect) {
        match h.wait(T) {
            AssetState::Ready(v) => assert_eq!(&*v, rec, "{did:#x}"),
            other => panic!("{did:#x}: {other:?}"),
        }
    }
    let missing = store.request(0x4100_0001, Priority::Visible);
    assert!(matches!(
        missing.wait(T),
        AssetState::Failed(AssetError::NotFound { did: 0x4100_0001 })
    ));
    assert_eq!(store.stats().completed, 300);
}

#[cfg(feature = "formats")]
#[test]
fn decodes_surfaces_from_an_archive_set() {
    use tessera_assets::decoders::SurfaceRgba;
    use tessera_dat::{ArchiveSet, SetOptions};

    let (built, expect) = build();
    let dir = datkit::TempDir::new("assets-set");
    std::fs::write(dir.0.join("client_surface.dat"), &built.bytes).unwrap();
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let pool = WorkerPool::new(PoolConfig {
        workers: 3,
        ..PoolConfig::default()
    });
    let store = AssetStore::new(&pool, set, SurfaceRgba, StoreConfig::default());
    let handles: Vec<_> = expect
        .iter()
        .map(|(did, _)| store.request(*did, Priority::Prefetch))
        .collect();
    for (h, (did, rec)) in handles.iter().zip(&expect) {
        let AssetState::Ready(img) = h.wait(T) else {
            panic!("{did:#x}: {:?}", h.state())
        };
        let w = u32::from_le_bytes(rec[8..12].try_into().unwrap());
        let hgt = u32::from_le_bytes(rec[12..16].try_into().unwrap());
        assert_eq!((img.width, img.height), (w, hgt));
        // A8R8G8B8 is stored B, G, R, A.
        assert_eq!(&img.pixels[..4], &[rec[26], rec[25], rec[24], rec[27]]);
    }
    // A DID between the synthetic ones is absent from the set.
    assert!(matches!(
        store.request(0x4100_0002, Priority::Visible).wait(T),
        AssetState::Failed(AssetError::NotFound { .. })
    ));
}
