// SPDX-License-Identifier: GPL-3.0-or-later
//! Memory bounds: refused flattening does not touch memory proportional to the refused
//! output (checked with the process's peak resident size on Linux), and
//! [`Mesh::heap_size`] counts the fixed per-submesh and per-level overhead of a
//! header-heavy record, which stays within the documented bound. Synthetic data only.
//!
//! Everything runs in one test function, peak-memory check first, so that no other test
//! in this binary raises the high-water mark.

use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec, TreeSpec};
use tessera_mesh::{
    Aabb, BuildByteRule, DecodeOptions, Did, Draw, GeometryLimits, IndexBuffer, Level, LevelKind,
    MAX_LEVELS, MAX_SUBMESHES, Mesh, Skin, Submesh, VertexFormat, VertexStreams,
};

/// The process's peak resident set size in KiB (`VmHWM`), on Linux.
fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The probe of the review: about 822 MB of streams from a 64 KB record before the byte
/// bounds; now refused before any output is allocated.
fn refused_flattening_stays_small() {
    let spec = MeshSpec {
        submeshes: vec![
            SubmeshSpec {
                format: 0x2,
                vertices: 2048,
            },
            SubmeshSpec {
                format: 0x3004_0F52,
                vertices: 3,
            },
        ],
        levels: vec![LevelSpec {
            collision: false,
            tree: TreeSpec::None,
            ..LevelSpec::detail(
                std::iter::once(1)
                    .chain(std::iter::repeat_n(0, 2047))
                    .collect(),
            )
        }],
        ..MeshSpec::single(0x112, 3)
    };
    let mesh = spec.build().unwrap();
    let before = peak_rss_kib();
    assert!(mesh.level_geometry(0, &GeometryLimits::default()).is_err());
    if let (Some(a), Some(b)) = (before, peak_rss_kib()) {
        assert!(b - a < 64 << 10, "peak resident size grew by {} KiB", b - a);
    }

    // Accepted output stays within max_bytes.
    let ok = MeshSpec::single(0x3004_0F52, 50_000).build().unwrap();
    let limits = GeometryLimits::default();
    let g = ok.level_geometry(0, &limits).unwrap();
    assert!(g.stream_bytes() <= limits.max_bytes);
}

/// A submesh with no vertices whose format has every stream (15 UV sets, K = 255): the
/// largest fixed per-submesh overhead.
fn empty_submesh() -> Submesh {
    Submesh {
        format: VertexFormat::new(0x30FF_0F72).unwrap(),
        vertex_count: 0,
        streams: VertexStreams {
            positions: Vec::new(),
            normals: Some(Vec::new()),
            scalars: Some(Vec::new()),
            colors: Some(Vec::new()),
            uv_sets: vec![Vec::new(); 15],
            tangents: Some(Vec::new()),
            binormals: Some(Vec::new()),
            skin: Some(Skin {
                influences: 255,
                joints: Vec::new(),
                weights: Vec::new(),
            }),
        },
        stored_aabb: Aabb {
            min: [0.0; 3],
            max: [0.0; 3],
        },
        influence_list: Vec::new(),
    }
}

/// A record made only of headers: `MAX_SUBMESHES` empty submeshes and `MAX_LEVELS`
/// one-draw levels.
fn header_heavy_records_are_counted_and_bounded() {
    use std::mem::size_of;
    let n = MAX_SUBMESHES as usize;
    let l = MAX_LEVELS as usize;
    let mesh = Mesh {
        did: Did(0x0600_0001),
        flags: 1,
        surfaces: (0..n as u32).map(|i| Did(0x3100_0000 + i)).collect(),
        submeshes: vec![empty_submesh(); n],
        index_buffers: vec![IndexBuffer::default(); n],
        levels: vec![
            Level {
                kind: LevelKind::Detail,
                draws: vec![Draw {
                    primitive: 4,
                    vertex_block: 0,
                    index_buffer: 0,
                    surface: 0,
                }],
                collision_counts: None,
                collision: None,
                tree: None,
            };
            l
        ],
        bounds: None,
        build_byte_rule: BuildByteRule::Stamp(39),
    };
    let bytes = mesh.to_bytes().unwrap();
    drop(mesh);
    let decoded = Mesh::decode(&bytes, Some(39), &DecodeOptions::lossless()).unwrap();
    let heap = decoded.heap_size();
    // Everything the decoder must have allocated: the outer vectors at their element
    // sizes, each submesh's 15 UV-set vectors, one draw per level.
    let floor = n
        * (size_of::<Did>()
            + size_of::<Submesh>()
            + 15 * size_of::<Vec<[f32; 2]>>()
            + size_of::<IndexBuffer>())
        + l * (size_of::<Level>() + size_of::<Draw>());
    assert!(heap >= floor, "heap_size {heap} < {floor}");
    // The documented bound: about 1.4 × the record plus a few MB.
    let bound = bytes.len() * 3 / 2 + (4 << 20);
    assert!(
        heap < bound,
        "{heap} bytes for a {}-byte record",
        bytes.len()
    );
}

#[test]
fn memory_bounds() {
    refused_flattening_stays_small();
    header_heavy_records_are_counted_and_bounded();
}
