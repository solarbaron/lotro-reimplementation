// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh decoder in the asset pipeline and [`MeshLibrary`] over a synthetic archive
//! set written to a temporary directory by `tessera-dat`'s own test writer (included by
//! path). No game data is involved.

#[path = "../../dat/tests/common/mod.rs"]
mod datkit;

use std::sync::Arc;
use std::time::Duration;

use tessera_assets::{
    AssetKey, AssetState, AssetStore, Decoder, JobContext, MemorySource, PoolConfig, Priority,
    StoreConfig, WorkerPool,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec};
use tessera_mesh::{
    BuildByteRule, DecodeOptions, Did, FailureCategory, MeshDecoder, MeshLibrary,
    MeshLibraryConfig, StampInput,
};

const T: Duration = Duration::from_secs(30);

fn spec(did: u32, build_byte: bool) -> MeshSpec {
    MeshSpec {
        did,
        flags: 1,
        submeshes: vec![
            SubmeshSpec {
                format: 0x3000_0152,
                vertices: 12 + did % 7,
            },
            SubmeshSpec {
                format: 0x3002_0112,
                vertices: 9,
            },
        ],
        levels: vec![LevelSpec::detail(vec![0, 1])],
        build_byte,
    }
}

#[test]
fn decoder_in_a_store_over_memory() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        ..PoolConfig::default()
    });
    let mut source = MemorySource::new();
    let good = spec(0x0600_0010, true).build().unwrap().to_bytes().unwrap();
    source.insert(0x0600_0010, good.clone());
    source.insert(0x0600_0011, good[..good.len() - 3].to_vec());
    let store = AssetStore::new(
        &pool,
        source,
        MeshDecoder::default(),
        StoreConfig::default(),
    );
    let ok = store.request(0x0600_0010, Priority::Visible);
    let AssetState::Ready(mesh) = ok.wait(T) else {
        panic!("not ready")
    };
    // No stamp: the fallback rule found the build bytes.
    assert_eq!(mesh.build_byte_rule, BuildByteRule::Fallback);
    assert_eq!(mesh.submeshes.len(), 2);
    let bad = store.request(0x0600_0011, Priority::Visible);
    let AssetState::Failed(err) = bad.wait(T) else {
        panic!("should fail")
    };
    // The record carries DID 0x06000010, not 0x06000011.
    let e = tessera_mesh::MeshError::Source {
        did: Did(0x0600_0011),
        source: err,
    };
    assert_eq!(e.category(), FailureCategory::Structure);
    assert!(e.decode_error().is_some());

    // The trailer form, called directly.
    let mut stamped = good.clone();
    stamped.extend_from_slice(&39u32.to_le_bytes());
    let dec = MeshDecoder {
        options: DecodeOptions::default(),
        stamp: StampInput::Trailer,
    };
    let m = dec
        .decode(
            AssetKey::from(0x0600_0010),
            stamped,
            &JobContext::detached(),
        )
        .unwrap();
    assert_eq!(m.build_byte_rule, BuildByteRule::Stamp(39));
    assert!(dec.size_of(&m) > 0);
    assert!(
        dec.decode(
            AssetKey::from(0x0600_0010),
            vec![1, 2],
            &JobContext::detached()
        )
        .is_err()
    );
}

#[test]
fn library_over_a_synthetic_install() {
    // Old-generation records (stamp 7, no build byte) and new ones (stamp 39).
    let mut b = datkit::Builder::new();
    let mut expect = Vec::new();
    for i in 0..40u32 {
        let did = 0x0600_0100 + i;
        let new = i % 2 == 0;
        let bytes = spec(did, new).build().unwrap().to_bytes().unwrap();
        if i % 3 == 0 {
            b.compressed(did, &bytes, flate2::Compression::fast());
        } else {
            b.stored(did, bytes);
        }
        b.records.last_mut().unwrap().stamp = if new { 39 } else { 7 };
        expect.push((did, new));
    }
    // A corrupt mesh: truncated by one byte.
    let mut broken = spec(0x0600_0200, true).build().unwrap().to_bytes().unwrap();
    broken.pop();
    b.stored(0x0600_0200, broken);
    // A non-mesh record in the same archive is not listed.
    b.stored(0x0700_0001, vec![1, 0, 0, 7]);
    let built = b.build();
    let dir = datkit::TempDir::new("mesh-lib");
    std::fs::write(dir.0.join("client_mesh.dat"), &built.bytes).unwrap();

    let set = Arc::new(ArchiveSet::open(&dir.0, SetOptions::default()).unwrap());
    let pool = WorkerPool::new(PoolConfig {
        workers: 3,
        ..PoolConfig::default()
    });
    let lib = MeshLibrary::new(Arc::clone(&set), &pool, MeshLibraryConfig::default());
    let dids = lib.mesh_dids().unwrap();
    assert_eq!(dids.len(), 41);
    assert!(dids.windows(2).all(|w| w[0] < w[1]));

    let handles: Vec<_> = expect
        .iter()
        .map(|(d, _)| lib.request(Did(*d), Priority::Visible))
        .collect();
    for (h, (did, new)) in handles.iter().zip(&expect) {
        let AssetState::Ready(m) = h.wait(T) else {
            panic!("{did:#x} not ready")
        };
        assert_eq!(m.did, Did(*did));
        let stamp = if *new { 39 } else { 7 };
        assert_eq!(m.build_byte_rule, BuildByteRule::Stamp(stamp));
        assert!(m.validate().is_clean());
    }
    let first = lib.load(Did(expect[0].0)).unwrap();
    assert!(Arc::ptr_eq(&first, &lib.load(Did(expect[0].0)).unwrap()));
    assert!(lib.cached(Did(expect[0].0)).is_some());
    assert!(lib.store().stats().hits >= 1);

    let (bytes, stamp) = lib.read_record(Did(expect[1].0)).unwrap();
    assert_eq!(stamp, 7);
    let direct = lib
        .decode_now(Did(expect[1].0), Some(&DecodeOptions::lossless()))
        .unwrap();
    assert_eq!(direct.to_bytes().unwrap(), bytes);

    let err = lib.load(Did(0x0600_0200)).unwrap_err();
    assert_eq!(err.category(), FailureCategory::Truncated, "{err}");
    let err = lib.load(Did(0x0600_0FFF)).unwrap_err();
    assert_eq!(err.category(), FailureCategory::Source);
    assert!(lib.decode_now(Did(0x0600_0FFF), None).is_err());
}
