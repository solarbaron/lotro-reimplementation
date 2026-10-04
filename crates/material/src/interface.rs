// SPDX-License-Identifier: GPL-3.0-or-later
//! What Tessera keeps of a shader record (`material.md` §7, §10, §11): the declared
//! parameters with their defaults, each texture slot's role and sampler state, and the
//! render-state summary used for feature flags. Built once per shader record; the
//! permutation list itself is dropped.

use std::collections::HashMap;
use std::hash::Hash;
use std::mem::size_of;

use tessera_formats::Did;

use crate::records::{GROUP_TEXTURE, ParamEntry, ParamValue, SamplerState, ShaderRecord, group_of};
use crate::vocab::{ChannelPacking, SlotKind, choose_role};

/// A declared parameter: its id and default value (§7.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    /// Parameter id.
    pub id: u32,
    /// Default value.
    pub default: ParamValue,
    /// Number of binding entries (0 for parameters bound by name, §7.3).
    pub binding_count: usize,
    /// The per-entry flag byte (§8.6).
    pub flag: u8,
}

/// A texture slot declared by a shader: role and sampler state (§10.1).
#[derive(Debug, Clone, PartialEq)]
pub struct SlotInterface {
    /// Texture parameter id (3000–3999).
    pub id: u32,
    /// What the id is for (§8.2).
    pub kind: SlotKind,
    /// Role name chosen from the bound sampler names (§10.1), `None` when no binding names
    /// a sampler (the slot is then sampled as plain colour).
    pub role: Option<String>,
    /// Channel packing derived from the role (§10.2).
    pub packing: ChannelPacking,
    /// The sampler state of the first binding that names a sampler (address and filter
    /// agree across permutations in observed data, §7.5).
    pub sampler: Option<SamplerState>,
}

/// The blend triple Tessera uses for an unlit shader record (§11.3 Decision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlendTriple {
    /// Source blend factor code.
    pub src: u8,
    /// Destination blend factor code.
    pub dst: u8,
    /// Blend operation code.
    pub op: u8,
    /// Depth write of the blocks carrying this triple (the most common value, ties to
    /// enabled).
    pub depth_write: bool,
    /// How many render-state blocks carry the triple.
    pub count: usize,
}

/// The analysed interface of one shader record.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderInterface {
    /// The shader record's DID.
    pub did: Did,
    /// Declared parameters in stored order (one per id; a repeated id keeps its first
    /// position and its last value).
    pub declarations: Vec<Declaration>,
    /// Declared texture slots, in declaration order.
    pub slots: Vec<SlotInterface>,
    /// Number of permutations.
    pub permutation_count: usize,
    /// Lit: some permutation is an additive per-light pass (`src_blend` 5, `dst_blend` 2,
    /// no depth write, §11.3).
    pub lit: bool,
    /// The most common (src, dst, op) triple over all render-state blocks (ties to the one
    /// met first); `None` without permutations.
    pub common_blend: Option<BlendTriple>,
    /// Whether the most common cull value is 1 (NONE) (§11.4).
    pub cull_none: bool,
    /// The most common render-state `alpha_ref` (the shader's built-in default threshold).
    pub default_alpha_ref: Option<f32>,
}

/// Most common element of `items`; ties go to the first met.
pub(crate) fn most_common<T: Hash + Eq + Copy>(
    items: impl IntoIterator<Item = T>,
) -> Option<(T, usize)> {
    // value → (count, first position)
    let mut counts: HashMap<T, (usize, usize)> = HashMap::new();
    for (i, item) in items.into_iter().enumerate() {
        counts.entry(item).or_insert((0, i)).0 += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| (a.1.0, std::cmp::Reverse(a.1.1)).cmp(&(b.1.0, std::cmp::Reverse(b.1.1))))
        .map(|(t, (c, _))| (t, c))
}

impl ShaderInterface {
    /// Analyses a decoded shader record.
    #[must_use]
    pub fn from_record(record: &ShaderRecord) -> Self {
        let mut declarations: Vec<Declaration> = Vec::with_capacity(record.params.len());
        let mut decl_entries: Vec<&ParamEntry> = Vec::with_capacity(record.params.len());
        let mut index_of: HashMap<u32, usize> = HashMap::new();
        for entry in &record.params {
            let decl = Declaration {
                id: entry.id,
                default: entry.value.clone(),
                binding_count: entry.bindings.len(),
                flag: entry.flag,
            };
            match index_of.get(&entry.id) {
                Some(&i) => {
                    declarations[i] = decl;
                    decl_entries[i] = entry;
                }
                None => {
                    index_of.insert(entry.id, declarations.len());
                    declarations.push(decl);
                    decl_entries.push(entry);
                }
            }
        }

        let slots = decl_entries
            .iter()
            .filter(|e| group_of(e.id) == GROUP_TEXTURE)
            .map(|entry| {
                let bound = || {
                    entry
                        .bindings
                        .iter()
                        .filter_map(|b| record.sampler(b.permutation, b.slot))
                };
                let role = choose_role(bound().map(|s| s.name.as_str())).map(str::to_owned);
                let sampler = bound().find(|s| !s.name.is_empty()).map(|s| s.state);
                SlotInterface {
                    id: entry.id,
                    kind: SlotKind::of(entry.id),
                    packing: ChannelPacking::from_role(role.as_deref()),
                    role,
                    sampler,
                }
            })
            .collect();

        let states = || record.permutations.iter().map(|p| p.render_state);
        let lit = states().any(|s| s.src_blend() == 5 && s.dst_blend() == 2 && !s.depth_write());
        let common_blend =
            most_common(states().map(|s| s.blend_triple())).map(|((src, dst, op), count)| {
                let writes = states()
                    .filter(|s| s.blend_triple() == (src, dst, op))
                    .filter(|s| s.depth_write())
                    .count();
                BlendTriple {
                    src,
                    dst,
                    op,
                    depth_write: writes * 2 >= count,
                    count,
                }
            });
        let cull_none = most_common(states().map(|s| s.raw[5])).is_some_and(|(c, _)| c == 1);
        let default_alpha_ref =
            most_common(states().map(|s| s.alpha_ref().to_bits())).map(|(b, _)| f32::from_bits(b));

        Self {
            did: record.did,
            declarations,
            slots,
            permutation_count: record.permutations.len(),
            lit,
            common_blend,
            cull_none,
            default_alpha_ref,
        }
    }

    /// The declaration of `id`.
    #[must_use]
    pub fn declaration(&self, id: u32) -> Option<&Declaration> {
        self.declarations.iter().find(|d| d.id == id)
    }

    /// Whether the shader declares `id`.
    #[must_use]
    pub fn declares(&self, id: u32) -> bool {
        self.declaration(id).is_some()
    }

    /// The texture slot `id`, if declared.
    #[must_use]
    pub fn slot(&self, id: u32) -> Option<&SlotInterface> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// The role name of texture slot `id`, if declared and named.
    #[must_use]
    pub fn role(&self, id: u32) -> Option<&str> {
        self.slot(id)?.role.as_deref()
    }

    /// Approximate heap bytes retained, for cache budgets.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.declarations.capacity() * size_of::<Declaration>()
            + self
                .declarations
                .iter()
                .map(|d| match &d.default {
                    ParamValue::Float(c) => c.keys.capacity() * size_of::<(f32, f32)>(),
                    _ => 0,
                })
                .sum::<usize>()
            + self.slots.capacity() * size_of::<SlotInterface>()
            + self
                .slots
                .iter()
                .map(|s| s.role.as_ref().map_or(0, String::capacity))
                .sum::<usize>()
    }
}
