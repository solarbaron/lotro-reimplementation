// SPDX-License-Identifier: GPL-3.0-or-later
//! Writing a [`Mesh`] back in the record layout of `mesh.md` §3–§10.
//!
//! Used to build synthetic fixtures and fuzz seeds, and to prove that decoding is
//! lossless: a record decoded with [`crate::DecodeOptions::lossless`] encodes to the same
//! bytes.

use crate::error::{MeshError, Result};
use crate::record::{CollisionTree, Mesh, Submesh};

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn bad(msg: impl Into<String>) -> MeshError {
    MeshError::Encode(msg.into())
}

fn check_len<T>(what: &str, i: usize, v: Option<&Vec<T>>, present: bool, n: usize) -> Result<()> {
    match (v, present) {
        (Some(v), true) if v.len() == n => Ok(()),
        (None, false) => Ok(()),
        _ => Err(bad(format!(
            "submesh {i}: {what} stream does not match the format and vertex count"
        ))),
    }
}

fn write_submesh(out: &mut Vec<u8>, i: usize, sm: &Submesh) -> Result<()> {
    let l = sm.format.layout();
    let st = &sm.streams;
    let n = sm.vertex_count as usize;
    if st.positions.len() != n {
        return Err(bad(format!(
            "submesh {i}: {} positions for {n} vertices",
            st.positions.len()
        )));
    }
    check_len("normal", i, st.normals.as_ref(), l.normal.is_some(), n)?;
    check_len("scalar", i, st.scalars.as_ref(), l.scalar.is_some(), n)?;
    check_len("colour", i, st.colors.as_ref(), l.color.is_some(), n)?;
    check_len("tangent", i, st.tangents.as_ref(), l.tangent.is_some(), n)?;
    check_len(
        "binormal",
        i,
        st.binormals.as_ref(),
        l.binormal.is_some(),
        n,
    )?;
    if st.uv_sets.len() != usize::from(l.uv_sets) || st.uv_sets.iter().any(|s| s.len() != n) {
        return Err(bad(format!("submesh {i}: UV sets do not match the format")));
    }
    let k = usize::from(l.influences);
    match &st.skin {
        None if k == 0 => {}
        Some(s)
            if s.influences == l.influences
                && s.joints.len() == n * k
                && s.weights.len() == n * k => {}
        _ => return Err(bad(format!("submesh {i}: skin does not match the format"))),
    }
    put_u32(out, sm.format.raw());
    put_u32(out, sm.vertex_count);
    out.reserve(n * l.stride);
    for v in 0..n {
        for c in st.positions[v] {
            put_f32(out, c);
        }
        if let Some(x) = &st.normals {
            x[v].iter().for_each(|&c| put_f32(out, c));
        }
        if let Some(x) = &st.scalars {
            put_f32(out, x[v]);
        }
        if let Some(x) = &st.colors {
            let [r, g, b, a] = x[v];
            out.extend_from_slice(&[b, g, r, a]);
        }
        for set in &st.uv_sets {
            set[v].iter().for_each(|&c| put_f32(out, c));
        }
        if let Some(x) = &st.tangents {
            x[v].iter().for_each(|&c| put_f32(out, c));
        }
        if let Some(x) = &st.binormals {
            x[v].iter().for_each(|&c| put_f32(out, c));
        }
        if let Some(s) = &st.skin {
            out.extend_from_slice(&s.joints[v * k..v * k + k]);
            s.weights[v * k..v * k + k]
                .iter()
                .for_each(|&w| put_f32(out, w));
        }
    }
    for c in sm.stored_aabb.min.iter().chain(&sm.stored_aabb.max) {
        put_f32(out, *c);
    }
    put_u32(out, sm.influence_list.len() as u32);
    for &x in &sm.influence_list {
        put_u32(out, x);
    }
    Ok(())
}

impl Mesh {
    /// Encodes the mesh in the record layout.
    ///
    /// `build_byte`s are written exactly where [`crate::TreeCode::build_byte`] is `Some`;
    /// the result decodes back to an equal mesh when the stamp given to the decoder (or
    /// the fallback rule) agrees with them.
    ///
    /// # Errors
    ///
    /// [`MeshError::Encode`] when a stream disagrees with its submesh's format word or
    /// vertex count, the surface and index-buffer counts differ from the submesh count, or
    /// collision meshes or tree code were not kept when decoding.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let n = self.submeshes.len();
        if self.surfaces.len() != n || self.index_buffers.len() != n {
            return Err(bad("surface, submesh and index-buffer counts differ"));
        }
        let mut out = Vec::with_capacity(self.heap_size());
        put_u32(&mut out, self.did.0);
        put_u32(&mut out, self.flags);
        put_u32(&mut out, n as u32);
        for s in &self.surfaces {
            put_u32(&mut out, s.0);
        }
        put_u32(&mut out, n as u32);
        for (i, sm) in self.submeshes.iter().enumerate() {
            write_submesh(&mut out, i, sm)?;
        }
        put_u32(&mut out, n as u32);
        for ib in &self.index_buffers {
            put_u32(&mut out, ib.indices.len() as u32);
            for &x in &ib.indices {
                put_u16(&mut out, x);
            }
        }
        put_u32(&mut out, self.levels.len() as u32);
        for (li, level) in self.levels.iter().enumerate() {
            put_u32(&mut out, level.kind.raw());
            put_u32(&mut out, level.draws.len() as u32);
            for d in &level.draws {
                out.push(d.primitive);
                put_u32(&mut out, d.vertex_block);
                put_u32(&mut out, d.index_buffer);
                put_u32(&mut out, d.surface);
            }
            match (&level.collision_counts, &level.collision) {
                (None, None) => out.push(0),
                (Some(_), Some(cm)) => {
                    out.push(1);
                    put_u32(&mut out, cm.vertices.len() as u32);
                    for v in &cm.vertices {
                        out.push(v.draw);
                        put_u16(&mut out, v.vertex);
                    }
                    put_u32(&mut out, cm.triangles.len() as u32);
                    for t in &cm.triangles {
                        for k in 0..3 {
                            put_u16(&mut out, t.vertices[k]);
                            put_u16(&mut out, t.edges[k]);
                        }
                    }
                    put_u32(&mut out, cm.edges.len() as u32);
                    for e in &cm.edges {
                        for k in 0..2 {
                            put_u16(&mut out, e.vertices[k]);
                            put_u16(&mut out, e.faces[k]);
                        }
                    }
                }
                _ => return Err(bad(format!("level {li}: collision mesh was not kept"))),
            }
            match &level.tree {
                None => out.push(0),
                Some(CollisionTree::NoCode) => out.extend_from_slice(&[1, 0]),
                Some(CollisionTree::Code(t)) => {
                    let code = t
                        .code
                        .as_ref()
                        .filter(|c| c.len() == t.code_len as usize)
                        .ok_or_else(|| bad(format!("level {li}: tree code was not kept")))?;
                    out.extend_from_slice(&[1, 1]);
                    for c in t.origin {
                        put_f32(&mut out, c);
                    }
                    put_f32(&mut out, t.scale);
                    put_u32(&mut out, t.code_len);
                    out.extend_from_slice(code);
                    if let Some(b) = t.build_byte {
                        out.push(b);
                    }
                }
            }
        }
        Ok(out)
    }
}
