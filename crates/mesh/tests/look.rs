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
use tessera_formats::script_table::resolve::{ClipResolution, LookCandidate, LookEffect};
use tessera_formats::script_table::{
    CompositorColourNode, CompositorLayerNode, EditMaterialNode, TextureCompositorNode,
};
use tessera_mesh::assembly::look::{
    COMPOSITOR_COLOUR_SCALE, LookLimits, compositor_colour, figure_look, material_colour,
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
    LookEffect::EditMaterial(Box::new(EditMaterialNode {
        material_property: parameter,
        mesh_type: mesh,
        material_type: HAIR_MATERIAL,
        priority,
        waveform_value: tessera_formats::Waveform {
            subtype: 0,
            values: vec![],
            extra: None,
        },
        color_value: bgra,
        texture_value: Did(0),
        bool_value: false,
    }))
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
    let look = figure_look(&resolution, fetch(&m), &LookLimits::default());
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
    let look2 = figure_look(&r2, fetch(&m), &LookLimits::default());
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
    let look = figure_look(&resolution, fetch(&m), &LookLimits::default());
    assert_eq!(look.compositors.len(), 2);
    assert!(look.compositors.iter().all(|c| c.image.is_none()));
    assert!(look.is_empty());
    assert!(look.notes.len() >= 2, "{:?}", look.notes);
    // Garbage instead of a compositor is a note, never a panic.
    m.insert(Did(RECORD), vec![1, 2, 3]);
    let look = figure_look(&resolution, fetch(&m), &LookLimits::default());
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
