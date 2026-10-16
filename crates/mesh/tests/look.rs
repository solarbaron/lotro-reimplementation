// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the figure look (`appearance.md` §3.5): compositor instances replayed
//! from look candidates (starts and stops), composition through hand-built records,
//! material edits. Every record is built here; values are invented.

use std::collections::HashMap;
use std::convert::Infallible;

use tessera_formats::Did;
use tessera_formats::PropertyValue;
use tessera_formats::gameplay::misc::{
    ColourEntry, CompositorLayer, CompositorOption, ImageCompositor,
};
use tessera_formats::script_table::resolve::{
    ClipResolution, LookCandidate, LookEffect, MaterialEdit,
};
use tessera_formats::script_table::{
    CompositorColourNode, CompositorLayerNode, TextureCompositorNode,
};
use tessera_mesh::assembly::AssemblyVocabulary;
use tessera_mesh::assembly::look::{
    COMPOSITOR_COLOUR_SCALE, FigureLook, LookCache, LookLimits, compositor_colour,
    figure_after_startup, figure_look, figure_on_screen, look_matches, material_colour,
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
        stopped_at: None,
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
        waveform_value: std::sync::Arc::new(tessera_formats::Waveform {
            subtype: 1,
            values: vec![0.0],
            extra: None,
        }),
        color_value: bgra,
        texture_value: Did(0),
        bool_value: false,
    })
}

/// [`figure_look`] with the default limits and no cache.
fn plain(resolution: &ClipResolution, m: &HashMap<Did, Vec<u8>>) -> FigureLook {
    figure_look(resolution, fetch(m), &LookLimits::default(), None)
}

fn eyes(option: i32) -> LookEffect {
    LookEffect::CompositorLayer(CompositorLayerNode {
        compositor: FACE,
        layer: L_EYES,
        image_number: option,
        opacity: 1.0,
    })
}

fn stopped_at(effect: LookEffect, at: usize) -> LookCandidate {
    let mut c = cand(effect);
    c.stopped = true;
    c.stopped_at = Some(at);
    c
}

fn priorities(look: &FigureLook, slot: u32, key: u32) -> Vec<(u32, i32)> {
    look.edits_for(slot, key)
        .iter()
        .map(|e| (e.material_property, e.priority))
        .collect()
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
            // A layer change before the attach is registered and applies when the
            // compositor is composed; the later change of the same layer wins.
            cand(eyes(2)),
            cand(attach(FACE, RECORD)),
            stopped,
            cand(eyes(1)),
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
            // Below 1000: skipped.
            cand(edit(999, CREST, 5, [0; 4])),
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
    // Edits of every group from 1000 apply; the one below 1000 is skipped.
    assert_eq!(look.edits.len(), 4);
    assert_eq!(look.skipped_edits, 1);
    // Appended by descending priority, start order among equals: the priority-3 edit
    // comes last and takes effect (appearance.md §3.5).
    assert_eq!(
        priorities(&look, CREST, HAIR_MATERIAL),
        vec![(2003, 5), (2003, 5), (3000, 5), (2003, 3)]
    );
    assert_eq!(
        look.edits_for(CREST, HAIR_MATERIAL)
            .last()
            .unwrap()
            .color_value,
        [255, 0, 0, 255]
    );
    // On the head only the All edit applies; other binding keys get nothing.
    assert_eq!(priorities(&look, HEAD, HAIR_MATERIAL), vec![(2003, 5)]);
    assert!(look.edits_for(CREST, FACE_SKIN).is_empty());

    // The identity follows the inputs: another eye option composes another image.
    let mut r2 = resolution.clone();
    if let LookEffect::CompositorLayer(l) = &mut r2.look[3].effect {
        l.image_number = 2;
    }
    if let LookEffect::CompositorLayer(l) = &mut r2.look[0].effect {
        l.image_number = 1;
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
    let limits = LookLimits::default();
    let first = figure_look(&face([0, 0, 128, 255]), counting, &limits, Some(&cache));
    let first_reads = reads.get();
    // The record and its two images (the base and eye option 1).
    assert_eq!(first_reads, 3, "{:?}", first.notes);
    assert!(first.compositors[0].source_bytes > 0);
    // The same face again: nothing is read, the same image is shared.
    let again = figure_look(&face([0, 0, 128, 255]), counting, &limits, Some(&cache));
    assert_eq!(reads.get(), first_reads);
    let (a, b) = (
        first.compositors[0].image.as_ref().unwrap(),
        again.compositors[0].image.as_ref().unwrap(),
    );
    assert!(std::sync::Arc::ptr_eq(a, b));
    assert_eq!(again.compositors[0].source_bytes, 0);
    assert_eq!(again.edits, first.edits);
    // Another skin colour composes again from the cached record and images: no reads.
    let other = figure_look(&face([0, 128, 0, 255]), counting, &limits, Some(&cache));
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
fn default_decode_budget_is_the_doubled_owner_value() {
    // The owner's answer of 2026-10-16 (`character-screens.md` §5.2, `appearance.md` §9
    // bounds (6)) doubled the per-compositor and per-figure decode budgets to 128 / 256 MiB
    // so a face-layer control may offer value 0.
    let d = LookLimits::default();
    assert_eq!(d.max_compositor_bytes, 128 << 20);
    assert_eq!(d.max_figure_bytes, 256 << 20);
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
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), &tight, Some(&cache));
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
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), &figure, None);
    assert!(look.compositors[0].image.is_none(), "{:?}", look.notes);
    // An image over the texel cap is refused before it is decoded.
    let small = LookLimits {
        max_texels: 1,
        ..LookLimits::default()
    };
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), &small, None);
    assert!(
        look.notes.iter().any(|n| n.contains("too large")),
        "{:?}",
        look.notes
    );
}

/// A JPEG surface (format 500, header size 0 × 0) whose stream is only a start-of-image
/// marker, a three-component baseline frame header declaring `w` × `h` and the end-of-image
/// marker: it has no tables and no scan, so decoding it fails.
fn jpeg_header_only(did: u32, w: u16, h: u16) -> Vec<u8> {
    let mut stream = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 17, 8];
    stream.extend_from_slice(&h.to_be_bytes());
    stream.extend_from_slice(&w.to_be_bytes());
    stream.push(3);
    for c in 1..=3u8 {
        stream.extend_from_slice(&[c, 0x11, 0]);
    }
    stream.extend_from_slice(&[0xFF, 0xD9]);
    let mut out = Vec::new();
    let len = u32::try_from(stream.len()).unwrap();
    for v in [did, 0, 0, 0, 500, len] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&stream);
    out
}

#[test]
fn jpeg_sources_are_checked_before_decoding() {
    // A frame header claiming a huge image is refused by its declared size: the note is
    // the texel check's, not a decode error, so nothing was decoded.
    let mut m = records();
    m.insert(
        Did(0x4100_0001),
        jpeg_header_only(0x4100_0001, 0xFFFF, 0xFFFF),
    );
    let look = plain(&face([0, 0, 128, 255]), &m);
    assert!(look.compositors[0].image.is_none());
    assert!(
        look.notes
            .iter()
            .any(|n| n.contains("65535 × 65535 texels is too large")),
        "{:?}",
        look.notes
    );
    // A small declared size is charged against the budget before decoding.
    m.insert(Did(0x4100_0001), jpeg_header_only(0x4100_0001, 2, 1));
    let tight = LookLimits {
        max_compositor_bytes: 4,
        ..LookLimits::default()
    };
    let look = figure_look(&face([0, 0, 128, 255]), fetch(&m), &tight, None);
    assert!(look.compositors[0].image.is_none());
    assert!(
        look.notes.iter().any(|n| n.contains("budget")),
        "{:?}",
        look.notes
    );
    // Within the limits the stream is decoded, and its decode error is a note.
    let look = plain(&face([0, 0, 128, 255]), &m);
    assert!(look.compositors[0].image.is_none());
    assert!(!look.notes.is_empty());
    // A stream without a readable frame header is refused too.
    let mut broken = jpeg_header_only(0x4100_0001, 2, 1);
    broken.truncate(28);
    broken[20..24].copy_from_slice(&4u32.to_le_bytes());
    m.insert(Did(0x4100_0001), broken);
    let look = plain(&face([0, 0, 128, 255]), &m);
    assert!(
        look.notes.iter().any(|n| n.contains("frame header")),
        "{:?}",
        look.notes
    );
}

/// `appearance.md` §3.5 rules 1–2: starts and stops replay in order.
#[test]
fn starts_and_stops_replay_in_order() {
    let m = records();
    let composed = |look: Vec<LookCandidate>| {
        plain(
            &ClipResolution {
                look,
                ..ClipResolution::default()
            },
            &m,
        )
    };
    // A refused attach's stop removes the compositor of its name, whichever node
    // attached it.
    let look = composed(vec![
        cand(attach(FACE, RECORD)),
        stopped_at(attach(FACE, 0x3300_0002), 3),
        cand(eyes(2)),
    ]);
    assert!(look.compositors.is_empty(), "{:?}", look.compositors);
    // An attach stopped before a later attach under the same name: the later one holds.
    let look = composed(vec![
        stopped_at(attach(FACE, 0x3300_0002), 1),
        cand(attach(FACE, RECORD)),
    ]);
    assert_eq!(look.compositors.len(), 1);
    assert_eq!(look.compositors[0].record, Did(RECORD));
    // A withdrawn change no longer applies; the earlier one does.
    let look = composed(vec![
        cand(eyes(2)),
        stopped_at(eyes(1), 3),
        cand(edit(2003, CREST, 5, [0; 4])),
        cand(attach(FACE, RECORD)),
    ]);
    assert_eq!(look.compositors[0].selection[1].option, 2);
    // A stopped edit is withdrawn.
    let look = composed(vec![stopped_at(edit(2003, CREST, 5, [0; 4]), 1)]);
    assert!(look.edits.is_empty());
    // A layer change of a layer the record lacks is reported and skipped.
    let look = composed(vec![
        cand(attach(FACE, RECORD)),
        cand(LookEffect::CompositorLayer(CompositorLayerNode {
            compositor: FACE,
            layer: 0x1000_0777,
            image_number: 1,
            opacity: 1.0,
        })),
    ]);
    assert!(look.compositors[0].image.is_some());
    assert_eq!(look.notes.len(), 1, "{:?}", look.notes);
}

#[test]
fn slots_and_binding_keys_match_by_the_rules() {
    // MeshType 2 every part, 0 none; MaterialType 2 every surface.
    assert!(look_matches(2, 2, HEAD, FACE_SKIN));
    assert!(look_matches(HEAD, 2, HEAD, HAIR_MATERIAL));
    assert!(!look_matches(0, 2, HEAD, FACE_SKIN));
    assert!(!look_matches(0, FACE_SKIN, 0, FACE_SKIN));
    assert!(look_matches(HEAD, FACE_SKIN, HEAD, FACE_SKIN));
    assert!(!look_matches(HEAD, FACE_SKIN, CREST, FACE_SKIN));
}

#[test]
fn placed_figures_are_in_the_world_after_start_up() {
    let vocab = AssemblyVocabulary {
        physics_in_world: Some(77),
        ..AssemblyVocabulary::default()
    };
    assert_eq!(
        figure_after_startup(&vocab),
        vec![(77, PropertyValue::Bool(true))]
    );
    assert!(figure_after_startup(&AssemblyVocabulary::default()).is_empty());
}

/// `appearance.md` §3.5 Decision: where two attached compositors match one surface, the
/// one attached last sets parameter 3000; a compositor without a composed image adds
/// nothing, so the one attached before it takes effect.
#[test]
fn last_attached_compositor_wins_for_a_surface() {
    use std::sync::Arc;
    use tessera_formats::RgbaImage;
    use tessera_mesh::assembly::look::FaceCompositor;
    let compositor = |name: u32, identity: u64, image: bool| FaceCompositor {
        name,
        record: Did(RECORD),
        mesh_type: HEAD,
        material_type: FACE_SKIN,
        selection: Vec::new(),
        colours: Vec::new(),
        image: image.then(|| {
            Arc::new(RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![1, 2, 3, 255],
            })
        }),
        identity,
        source_bytes: 0,
    };
    let look = FigureLook {
        compositors: vec![compositor(FACE, 1, true), compositor(OTHER, 2, true)],
        ..FigureLook::default()
    };
    assert_eq!(
        look.compositor_for(HEAD, FACE_SKIN).map(|c| c.identity),
        Some(2)
    );
    let look = FigureLook {
        compositors: vec![compositor(OTHER, 2, true), compositor(FACE, 1, true)],
        ..FigureLook::default()
    };
    assert_eq!(
        look.compositor_for(HEAD, FACE_SKIN).map(|c| c.identity),
        Some(1)
    );
    // The later one failed to compose: the earlier one's image stays.
    let look = FigureLook {
        compositors: vec![compositor(FACE, 1, true), compositor(OTHER, 2, false)],
        ..FigureLook::default()
    };
    assert_eq!(
        look.compositor_for(HEAD, FACE_SKIN).map(|c| c.identity),
        Some(1)
    );
    assert!(look.compositor_for(CREST, FACE_SKIN).is_none());
}

#[test]
fn character_screen_figures_carry_their_stage() {
    // appearance.md §3.5 Decision: a character-screen figure carries `CharGen_Stage`.
    let vocab = AssemblyVocabulary {
        chargen_stage: Some(78),
        ..AssemblyVocabulary::default()
    };
    assert_eq!(
        figure_on_screen(&vocab, 6),
        vec![(78, PropertyValue::Enum(6))]
    );
    assert!(figure_on_screen(&AssemblyVocabulary::default(), 6).is_empty());
}
