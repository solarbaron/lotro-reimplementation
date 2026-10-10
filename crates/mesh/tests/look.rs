// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the figure look (`appearance.md` §3.5): compositor instances played
//! from look candidates, composition through hand-built records, colour edits. Every
//! record is built here; values are invented.

use std::collections::HashMap;
use std::convert::Infallible;

use tessera_formats::Did;
use tessera_formats::gameplay::misc::{
    ColourEntry, CompositorLayer, CompositorOption, ImageCompositor,
};
use tessera_formats::script_table::resolve::{
    ClipResolution, LookCandidate, LookEffect, MaterialEdit,
};
use tessera_formats::script_table::{
    CompositorColourNode, CompositorLayerNode, TextureCompositorNode,
};
use tessera_mesh::assembly::AssemblyOptions;
use tessera_mesh::assembly::look::{
    COMPOSITOR_COLOUR_SCALE, FigureLook, LookCache, LookLimits, compositor_colour, figure_look,
    material_colour,
};

const FACE: u32 = 0x1000_0001;
const OTHER: u32 = 0x1000_000A;
const HEAD: u32 = 0x1000_0002;
const CREST: u32 = 0x1000_000E;
const FACE_SKIN: u32 = 0x1000_006C;
const HAIR_MATERIAL: u32 = 0x1000_0024;
const L_BASE: u32 = 0x1000_000B;
const L_EYES: u32 = 0x1000_0004;
const C_SKIN: u32 = 0x1000_000B;
const RECORD: u32 = 0x3300_0001;

fn surface(did: u32, w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [did, 0, w, h, 21, w * h * 4] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for _ in 0..w * h {
        out.extend_from_slice(&[rgba[2], rgba[1], rgba[0], rgba[3]]);
    }
    out
}

fn records() -> HashMap<Did, Vec<u8>> {
    let option = |index: u32, image: u32, x: u32, colours: Vec<ColourEntry>| CompositorOption {
        index,
        legacy_did: Did(0x9900_0000 + image),
        image_did: Did(image),
        x,
        y: 0,
        colours,
    };
    let record = ImageCompositor {
        did: Did(RECORD),
        layers: vec![
            CompositorLayer {
                layer_key: L_BASE,
                options: vec![option(
                    1,
                    0x4100_0001,
                    0,
                    vec![ColourEntry {
                        colour_key: C_SKIN,
                        legacy_did: Did(0),
                        mask_did: Did(0),
                    }],
                )],
            },
            CompositorLayer {
                layer_key: L_EYES,
                options: vec![
                    option(1, 0x4100_0002, 1, vec![]),
                    option(2, 0x4100_0003, 1, vec![]),
                ],
            },
        ],
        compress_with_alpha: false,
    };
    let mut m = HashMap::new();
    m.insert(Did(RECORD), record.to_bytes());
    m.insert(Did(0x4100_0001), surface(0x4100_0001, 2, 1, [255; 4]));
    m.insert(
        Did(0x4100_0002),
        surface(0x4100_0002, 1, 1, [255, 0, 0, 255]),
    );
    m.insert(
        Did(0x4100_0003),
        surface(0x4100_0003, 1, 1, [0, 255, 0, 255]),
    );
    m
}

fn cand(effect: LookEffect) -> LookCandidate {
    LookCandidate {
        effect,
        driven: false,
        picked: true,
        chance: 1.0,
        stopped: false,
        table: Did(0x0700_0001),
        scriptlet: Did(0),
        node_id: 1,
        scripts: vec![],
    }
}

fn attach(name: u32, file: u32) -> LookEffect {
    LookEffect::AttachCompositor(TextureCompositorNode {
        compositor: name,
        file: Did(file),
        mesh_type: HEAD,
        material_type: FACE_SKIN,
    })
}

fn edit(parameter: u32, mesh: u32, priority: i32, bgra: [u8; 4]) -> LookEffect {
    LookEffect::EditMaterial(MaterialEdit {
        material_property: parameter,
        mesh_type: mesh,
        material_type: HAIR_MATERIAL,
        priority,
        color_value: bgra,
        texture_value: Did(0),
        bool_value: false,
    })
}

/// [`figure_look`] with both options on, the default limits and no cache.
fn plain(resolution: &ClipResolution, m: &HashMap<Did, Vec<u8>>) -> FigureLook {
    figure_look(
        resolution,
        fetch(m),
        AssemblyOptions::default(),
        &LookLimits::default(),
        None,
    )
}

fn fetch(m: &HashMap<Did, Vec<u8>>) -> impl FnMut(Did) -> Result<Option<Vec<u8>>, Infallible> + '_ {
    |d| Ok(m.get(&d).cloned())
}

#[test]
fn compositor_instances_compose_and_edits_rank() {
    let m = records();
    let mut stopped = cand(attach(OTHER, RECORD));
    stopped.stopped = true;
    let mut unpicked = cand(edit(2003, CREST, 99, [9, 9, 9, 255]));
    unpicked.picked = false;
    let resolution = ClipResolution {
        look: vec![
            // A layer change before the attach names an unknown instance: ignored.
            cand(LookEffect::CompositorLayer(CompositorLayerNode {
                compositor: FACE,
                layer: L_EYES,
                image_number: 2,
                opacity: 1.0,
            })),
            cand(attach(FACE, RECORD)),
            stopped,
            cand(LookEffect::CompositorLayer(CompositorLayerNode {
                compositor: FACE,
                layer: L_EYES,
                image_number: 1,
                opacity: 1.0,
            })),
            cand(LookEffect::CompositorColour(CompositorColourNode {
                compositor: FACE,
                colour: C_SKIN,
                color: [0, 0, 128, 255],
            })),
            // A second attach under the same name is refused.
            cand(attach(FACE, 0x3300_0002)),
            cand(edit(2003, CREST, 5, [0, 0, 255, 255])),
            cand(edit(2003, CREST, 3, [255, 0, 0, 255])),
            cand(edit(2003, 2, 5, [0, 255, 0, 255])),
            cand(edit(3000, CREST, 5, [0; 4])),
            unpicked,
        ],
        ..ClipResolution::default()
    };
    let look = plain(&resolution, &m);
    assert_eq!(look.compositors.len(), 1, "{:?}", look.notes);
    let face = &look.compositors[0];
    assert_eq!((face.name, face.record), (FACE, Did(RECORD)));
    assert_eq!(face.selection[1].option, 1);
    let img = face.image.as_ref().expect("composed");
    assert_eq!((img.width, img.height), (2, 1));
    // Texel 0: white multiplied by the skin colour (R 128, G 0, B 0, misc-records.md
    // §3.6.3); texel 1: the eye layer's red mixed at full opacity.
    assert_eq!(img.pixels, vec![128, 0, 0, 255, 255, 0, 0, 255]);
    assert_eq!(look.notes.len(), 2, "{:?}", look.notes);
    assert!(look.compositor_for(HEAD, FACE_SKIN).is_some());
    assert!(look.compositor_for(CREST, FACE_SKIN).is_none());
    // Edits: three colour edits on 2003 and one on a texture id (counted only).
    assert_eq!(look.edits.len(), 3);
    assert_eq!(look.other_edits, 1);
    // On the crest the priority-5 edits tie; the later one (slot All) wins.
    assert_eq!(
        look.colour_edits_for(CREST, HAIR_MATERIAL),
        vec![(2003, [0.0, 1.0, 0.0, 1.0])]
    );
    // On the head only the All edit applies; other binding keys get nothing.
    assert_eq!(
        look.colour_edits_for(HEAD, HAIR_MATERIAL),
        vec![(2003, [0.0, 1.0, 0.0, 1.0])]
    );
    assert!(look.colour_edits_for(CREST, FACE_SKIN).is_empty());

    // The identity follows the inputs: another eye option composes another image.
    let mut r2 = resolution.clone();
    if let LookEffect::CompositorLayer(l) = &mut r2.look[3].effect {
        l.image_number = 2;
    }
    let look2 = plain(&r2, &m);
    let face2 = &look2.compositors[0];
    assert_ne!(face2.identity, face.identity);
    assert_eq!(face2.image.as_ref().unwrap().pixels[4..8], [0, 255, 0, 255]);
}

#[test]
fn missing_records_and_images_are_notes() {
    let mut m = records();
    m.remove(&Did(0x4100_0002));
    let resolution = ClipResolution {
        look: vec![
            cand(attach(FACE, RECORD)),
            cand(LookEffect::CompositorLayer(CompositorLayerNode {
                compositor: FACE,
                layer: L_EYES,
                image_number: 1,
                opacity: 1.0,
            })),
            cand(attach(OTHER, 0x3300_00FF)),
        ],
        ..ClipResolution::default()
    };
    let look = plain(&resolution, &m);
    assert_eq!(look.compositors.len(), 2);
    assert!(look.compositors.iter().all(|c| c.image.is_none()));
    assert!(look.is_empty());
    assert!(look.notes.len() >= 2, "{:?}", look.notes);
    // Garbage instead of a compositor is a note, never a panic.
    m.insert(Did(RECORD), vec![1, 2, 3]);
    let look = plain(&resolution, &m);
    assert!(look.compositors.iter().all(|c| c.image.is_none()));
}

#[test]
fn colour_conversions_follow_the_specs() {
    assert_eq!(COMPOSITOR_COLOUR_SCALE.to_bits(), 0x3B80_8081);
    // Stored bytes B, G, R, A.
    let c = compositor_colour([1, 2, 255, 128]);
    assert_eq!(c[0], 255.0 * COMPOSITOR_COLOUR_SCALE);
    assert_eq!(c[2], COMPOSITOR_COLOUR_SCALE);
    assert_eq!(material_colour([0, 0, 255, 255]), [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(material_colour([51, 102, 0, 0]), [0.0, 0.4, 0.2, 0.0]);
}

/// The face of [`compositor_instances_compose_and_edits_rank`]: attach, eye option 1, skin
/// colour.
fn face(skin: [u8; 4]) -> ClipResolution {
    ClipResolution {
        look: vec![
            cand(attach(FACE, RECORD)),
            cand(LookEffect::CompositorLayer(CompositorLayerNode {
                compositor: FACE,
                layer: L_EYES,
                image_number: 1,
                opacity: 1.0,
            })),
            cand(LookEffect::CompositorColour(CompositorColourNode {
                compositor: FACE,
                colour: C_SKIN,
                color: skin,
            })),
            cand(edit(2003, CREST, 5, [0, 0, 255, 255])),
        ],
        ..ClipResolution::default()
    }
}

#[test]
fn the_cache_composes_a_face_once() {
    let m = records();
    let cache = LookCache::default();
    let reads = std::cell::Cell::new(0usize);
    let counting = |d: Did| -> Result<Option<Vec<u8>>, Infallible> {
        reads.set(reads.get() + 1);
        Ok(m.get(&d).cloned())
    };
    let on = AssemblyOptions::default();
    let limits = LookLimits::default();
    let first = figure_look(&face([0, 0, 128, 255]), counting, on, &limits, Some(&cache));
    let first_reads = reads.get();
    // The record and its two images (the base and eye option 1).
    assert_eq!(first_reads, 3, "{:?}", first.notes);
    assert!(first.compositors[0].source_bytes > 0);
    // The same face again: nothing is read, the same image is shared.
    let again = figure_look(&face([0, 0, 128, 255]), counting, on, &limits, Some(&cache));
    assert_eq!(reads.get(), first_reads);
    let (a, b) = (
        first.compositors[0].image.as_ref().unwrap(),
        again.compositors[0].image.as_ref().unwrap(),
    );
    assert!(std::sync::Arc::ptr_eq(a, b));
    assert_eq!(again.compositors[0].source_bytes, 0);
    assert_eq!(again.edits, first.edits);
    // Another skin colour composes again from the cached record and images: no reads.
    let other = figure_look(&face([0, 128, 0, 255]), counting, on, &limits, Some(&cache));
    assert_eq!(reads.get(), first_reads);
    assert_ne!(other.compositors[0].identity, first.compositors[0].identity);
    assert_eq!(
        other.compositors[0].image.as_ref().unwrap().pixels[..4],
        [0, 128, 0, 255]
    );
    // Without the cache the result is the same image.
    let uncached = plain(&face([0, 128, 0, 255]), &m);
    assert_eq!(uncached.compositors[0].image, other.compositors[0].image);
    assert_eq!(cache.len(), (1, 2, 2));
}

#[test]
fn decoded_bytes_are_budgeted() {
    let m = records();
    // The base image is 2 × 1 texels (8 bytes), each eye option 1 texel (4 bytes).
    let tight = LookLimits {
        max_compositor_bytes: 12,
        ..LookLimits::default()
    };
    let cache = LookCache::default();
    let on = AssemblyOptions::default();
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), on, &tight, Some(&cache));
    assert!(
        look.notes.iter().any(|n| n.contains("budget")),
        "{:?}",
        look.notes
    );
    // A result cut by a limit is not kept.
    assert_eq!(cache.len().2, 0);
    // The figure budget also counts the composed image.
    let figure = LookLimits {
        max_figure_bytes: 16,
        ..LookLimits::default()
    };
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), on, &figure, None);
    assert!(look.compositors[0].image.is_none(), "{:?}", look.notes);
    // An image over the texel cap is refused before it is decoded.
    let small = LookLimits {
        max_texels: 1,
        ..LookLimits::default()
    };
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), on, &small, None);
    assert!(
        look.notes.iter().any(|n| n.contains("too large")),
        "{:?}",
        look.notes
    );
}

#[test]
fn face_compositing_off_attaches_no_compositor() {
    let m = records();
    let off = AssemblyOptions {
        avatar_face_compositing: false,
        ..AssemblyOptions::default()
    };
    let look = figure_look(
        &face([0, 0, 128, 255]),
        fetch(&m),
        off,
        &LookLimits::default(),
        None,
    );
    // appearance.md §3.5: neither compositor is attached; the colour edits stay.
    assert!(look.compositors.is_empty());
    assert_eq!(look.edits.len(), 1);
    assert_eq!(look.notes.len(), 1, "{:?}", look.notes);
}
