// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`: a GUI browser for the archives of an install, read-only.
//!
//! The explorer is the M1 workbench (`docs/ROADMAP.md`): an archive tree (archives →
//! DID type groups → DIDs, virtualized for millions of entries), DID search (hex,
//! prefix, names from DID maps and the property registry), and views for any record:
//! entry information, hex, textures (zoom, channels, alpha checkerboard, mip levels),
//! layered icons, string tables with variable placeholders and markup, property sets
//! with names, enum labels and clickable DID links, and decoded record tables (DID
//! maps, enum mappers, the property dictionary).
//!
//! The M2 mesh viewer (the "3D" view) draws meshes, setups (part assemblies), visual
//! templates, entity descriptors, appearance entries, objects with a geometry link, and
//! surface records or materials on a preview sphere, with `tessera-render`'s mesh
//! pipelines: orbit/pan/zoom, wireframe, normals, UV checker, level selection, a
//! per-part and per-draw list with visibility toggles, a material inspector (slots with
//! thumbnails, effective parameters, feature flags, blend state) and a dye preview. Back/forward history, bookmarks,
//! recent DIDs and the game directory persist in Tessera's settings file.
//!
//! The M4.5 world views: the record view tables terrain type tables, terrain-scene and
//! scene-choice tables, scene records, water and fog-zone tables, sky and day
//! descriptions, game time, LOD classes, camera states, collision meshes, cell geometry
//! and region descriptors (`world_records`). The "Landblock" view ([`world_view`]) maps a
//! landblock's terrain, generated scenery, statics, cells, landblock water and liquid
//! boxes, built on a worker by `tessera-world` as the walker builds them ([`world`]). The
//! "Sky" view draws the blended keyframes of a day description over the game day, and for
//! the sky description the day chosen per sky key, day type and game day. The 3D view
//! draws collision meshes (0x66), cells (0x01) with their render mesh, setups and whole
//! landblocks with `tessera-physics` collision geometry as overlays
//! ([`collision_overlay`]).
//!
//! # Threads
//!
//! The UI thread never reads or decodes a record. Opening the install, indexing the
//! archives and searching run on background threads ([`data::spawn_open`],
//! [`index::Indexer`], [`search::Search`]); every decode runs on `tessera-assets`
//! workers through the decoders in [`decoders`], [`scene`] and [`world`] (sky timelines
//! included). 3D rendering runs on its
//! own thread ([`viewport`]); the UI only shows the finished frames. A frame-time overlay shows the cost of
//! each frame, and frames over 50 ms are logged ([`frame_stats`]).
//!
//! # Headless checks
//!
//! `--screenshot out.png --select <DID> [--view texture|strings|props|…]` renders until
//! the selection is ready, saves the window as PNG and exits ([`app`]). With
//! `--view mesh` it waits for the scene and its rendered frame; `--mesh-shading`,
//! `--mesh-wireframe`, `--mesh-normals`, `--mesh-lod`, `--mesh-choice`, `--mesh-angle`
//! and `--dye` set the view up; `--mesh-vertex-colors` and `--mesh-diffuse-tint` turn on
//! the two material inputs whose meaning is OPEN (both off by default). Screenshot mode
//! uses X11 whenever `DISPLAY` is set, so `xvfb-run` gives the requested `--width` ×
//! `--height` even inside a Wayland session; a capture of another size is logged as a
//! warning.
//!
//! The character view (M4, [`anim_view`]) is reproducible from the command line:
//! `--view character --character human,m --wear 0x790006C3 --clip walk --time 0.4
//! --mesh-angle 160,15 [--bones] [--trail] [--sliders HEX]` assembles the figure, loads
//! the clip, samples the pose at that time on the workers and waits for its frame
//! ([`charspec`] has the grammar). Without `--select`, the root DID map is selected.
//!
//! # Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`app`] | The eframe application: panels, views, navigation |
//! | [`cli`] | Command-line options and the [`cli::ViewKind`] list |
//! | [`data`] | The opened install shared by the workers ([`data::DataContext`]) |
//! | [`decoders`] | Worker decoders and their display-ready products |
//! | [`index`] | Archive directory indexes grouped by DID type |
//! | [`search`], [`query`] | DID search |
//! | [`image_ops`] | Channel masking, size limiting, checkerboard |
//! | [`richtext`] | Rendered message text → egui layout |
//! | [`hex`] | Hex-dump and size formatting |
//! | [`history`] | Back/forward history |
//! | [`frame_stats`] | Frame-time statistics |
//! | [`persist`] | Settings persistence (`tessera-settings`) |
//! | [`screenshot`] | PNG output for `--screenshot` |
//! | [`fonts`] | UI fonts from the system (none are embedded) |
//! | [`scene`] | Mesh-viewer scenes built on workers: parts, materials, textures |
//! | [`script_view`] | Text display of script-graph nodes (record view of 0x07/0x0C, per-instance script view) |
//! | [`geometry`] | Mesh level → renderer streams; texture compositions (pure) |
//! | [`viewport`] | The render thread (offscreen `tessera-render`) and the orbit camera |
//! | [`mesh_view`] | The 3D view's UI: toolbar, viewport, part list, material inspector |
//! | [`character`] | Character viewer data (M4): bodies, assembled figures, clips, clip catalogue (workers) |
//! | [`anim_view`] | The character view's UI: body, equipment, sliders, clip browser, playback, mount viewer, drama player |
//! | [`drama_play`] | The character view's drama player (`npc-drama.md` §11 item 4): preparation on workers, the T1/T2/T5 run on the UI thread |
//! | [`mount_view`] | Mount viewer data (`mounts.md` §10): the rider's mount state, the MeshFX branch, the mount figure and its clip (workers) |
//! | [`sheet`] | `--sheet`: the character view rendered headless into a contact sheet |
//! | [`particle_view`] | particle preview of 0x39 records in the gameplay view (play, pause, restart, seed) and `--particles DID --sheet` |
//! | [`charspec`] | Text forms of `--character`, `--wear`, `--clip`, `--time`, `--sliders` |
//! | [`world`] | M4.5 world data on workers: world context, landblock products, sky products with their timelines |
//! | [`timeline`] | Sky timelines from decoded records (pure, bounded) |
//! | [`world_view`] | The landblock and sky views |
//! | `world_records` | Record-view tables of the M4.5 world records |
//! | [`collision_overlay`] | Collision overlays of the 3D view from `tessera-physics` geometry |
//! | [`decal_preview`] | Decal previews of the 3D view for landblock-info records (box, axes, preview mesh with the decal's material) |
//! | [`gameplay`] | The "Gameplay" view: item, skill, effect, trait, creature, quest, vendor, loot, recipe, map and particle cards from `tessera_formats::gameplay` (workers) |

#![warn(missing_docs)]

pub mod anim_view;
pub mod app;
pub mod character;
pub mod charspec;
pub mod cli;
pub mod collision_overlay;
pub mod data;
pub mod decal_preview;
pub mod decoders;
pub mod drama_play;
mod error;
pub mod fonts;
pub mod frame_stats;
pub mod gameplay;
pub mod geometry;
pub mod hex;
pub mod history;
pub mod image_ops;
pub mod index;
pub mod mesh_view;
pub mod mount_view;
pub mod particle_view;
pub mod persist;
pub mod query;
pub mod richtext;
pub mod scene;
pub mod screenshot;
pub mod script_view;
pub mod search;
pub mod sheet;
pub mod timeline;
pub mod viewport;
mod views;
pub mod world;
pub(crate) mod world_records;
pub mod world_view;

pub use app::{AppConfig, ExplorerApp, ScreenshotPlan, run};
pub use error::ExplorerError;
