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
//! # Threads
//!
//! The UI thread never reads or decodes a record. Opening the install, indexing the
//! archives and searching run on background threads ([`data::spawn_open`],
//! [`index::Indexer`], [`search::Search`]); every decode runs on `tessera-assets`
//! workers through the decoders in [`decoders`] and [`scene`]. 3D rendering runs on its
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
//! | [`geometry`] | Mesh level → renderer streams; texture compositions (pure) |
//! | [`viewport`] | The render thread (offscreen `tessera-render`) and the orbit camera |
//! | [`mesh_view`] | The 3D view's UI: toolbar, viewport, part list, material inspector |
//! | [`character`] | Character viewer data (M4): bodies, assembled figures, clips, clip catalogue (workers) |
//! | [`anim_view`] | The character view's UI: body, equipment, sliders, clip browser, playback |
//! | [`sheet`] | `--sheet`: the character view rendered headless into a contact sheet |
//! | [`charspec`] | Text forms of `--character`, `--wear`, `--clip`, `--time`, `--sliders` |

#![warn(missing_docs)]

pub mod anim_view;
pub mod app;
pub mod character;
pub mod charspec;
pub mod cli;
pub mod data;
pub mod decoders;
mod error;
pub mod fonts;
pub mod frame_stats;
pub mod geometry;
pub mod hex;
pub mod history;
pub mod image_ops;
pub mod index;
pub mod mesh_view;
pub mod persist;
pub mod query;
pub mod richtext;
pub mod scene;
pub mod screenshot;
pub mod search;
pub mod sheet;
pub mod viewport;
mod views;

pub use app::{AppConfig, ExplorerApp, ScreenshotPlan, run};
pub use error::ExplorerError;
