<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Fuzz targets

Every parser of untrusted input has a target here (`CLAUDE.md`, "Parsers treat all input as
hostile"). This directory is its own Cargo workspace, so the main workspace keeps building on
stable without libFuzzer.

| Target | Code under test | Spec |
|---|---|---|
| `dat_header` | `Header::parse`, `Entry::parse` round trip, `Archive::from_bytes` header checks | `docs/specs/dat-container.md` §3, §5.2 |
| `dat_node` | directory-node parsing, lookup, in-order traversal, extents, free table, validator, behind a **valid synthetic header** so every run reaches the B-tree code | §4, §5, §10 |
| `dat_archive` | a whole archive file from bytes: lookup, iteration, record reads, validator | §3-§10 |
| `dat_inflate` | `inflate_record` (length prefix + zlib, size cap) | §6.1 |
| `dat_routing` | `RoutingTable::parse` and routing | §8 |
| `formats_primitives` | `Reader` primitives, VLE round trip, `StringInfo::read`, `PropertyValue::read` for every type, DID helpers | `docs/specs/records.md` §1, §2, §4, §6 |
| `formats_records` | every record decoder and `AnyRecord::decode` | `records.md` |
| `formats_surface` | `Surface::parse` + `to_rgba8`, embedded JPEG, DXT/BCn | `records.md` §5 |
| `formats_property_set` | `decode_property_set`, `PropertySetRecord::parse`/`parse_prefix`, embedded sets, single values, with a total and a small synthetic schema | `docs/specs/property-system.md` §4-§5 |
| `formats_texture_list` | texture-list decoding and level selection | `docs/specs/texture-list.md` §2-§5 |
| `formats_string_table` | string-table message model: pieces, slots, invariant report | `records.md` §3 |
| `props_set` | `PropertyRegistry::parse`, `PropertySet::parse_record`/`parse_record_prefix`/`parse_embedded`/`parse_embedded_prefix` with a registry decoded from the same input | `property-system.md` §3-§6 |
| `text_render` | message grammar and rendering, markup, tag lists and selector options, `${NAME}` format strings and templates, a string-table record rendered through `Strings` | `docs/specs/string-table.md` |
| `material_icon` | `decode_layer_with_limit` on a surface payload, `compose_images_with_limit` over decoded and synthetic layers | `docs/specs/icon-layers.md` §3 |
| `material_records` | Surface record (0x31), material (0x30) and shader record (0x2B) decoders; shader interface analysis and resolution over an accepted shader record | `docs/specs/material.md` §4–§11 |
| `material_resolve` | `MaterialLibrary` over an in-memory source built from the input: surface → shader + materials → texture list → surface, texture decoding under a size limit, dye preview; `apply_dye` and `select_dye_block` | `docs/specs/material.md` §9–§12, `texture-list.md` §5.2 |
| `settings_file` | settings file: `Settings::parse_lenient`, `from_toml_str`, serialize/parse round trip, `parse_did` | `tessera-settings` |
| `dattool_snapshot` | `tessera-dattool` snapshot decoder, encode/decode round trip | `apps/dattool/src/snapfile.rs` |
| `net_packet_decode` | datagram decode/encode round trip | `docs/specs/transport-design.md` |
| `net_endpoint` | reliable endpoint state machine under hostile datagrams | `transport-design.md` |
| `net_session` | `Session` state machine over the sandbox backend: arbitrary config, invalid commands, stale clocks; event-order invariants | `tessera-net` `session`, `sandbox` |
| `install_vdf` | Steam text VDF reader (`parse`, `parse_bytes`), `parse_library_folders`, `parse_app_manifest` | `docs/specs/install-layout.md` §5.3 |
| `install_config` | `Ini` reader and typed getters, launcher-config `appSettings` reader, third-party launcher TOML subset, `translate_windows_path` | `install-layout.md` §5.6.2, §6, §8.3 |
| `install_records` | archive-name grammar (canonical round trip), header prefix, 0xFFFF0001/0003/0005 records, `verify_split_set`, schema and content fingerprints | `install-layout.md` §3.1, §3.4, §9 |
| `script_descriptor` | `.plugin` descriptor parser, package/resource-path/plugin-data-key validation, `sanitize_folder_name` | `docs/specs/lua-plugin-api.md` §4.4, §5, §10.2 |
| `script_plugindata` | `.plugindata` reader with limits, serialize/parse stability, `format_number` round trip, `EngineDate` | `lua-plugin-api.md` §10.4, §10.5, §12 |
| `script_pattern` | the bounded Lua pattern engine (`string.find`/`match`/`gmatch`/`gsub`) and chat-command parsing inside a real apartment | `lua-plugin-api.md` §3.1, §3.3, §6.4 |
| `render_mesh_data` | `MeshData::validate` on hostile streams (length mismatches, out-of-range indices, bad submesh ranges, NaN/inf attributes), `compute_normals`, `compute_tangents`, `flip_winding`, shape builders | `docs/specs/render.md` §2.1 |
| `render_range_alloc` | the mesh arena's first-fit `RangeAllocator` under alloc / free / bogus-free sequences, checked against a model | `render.md` §2.2 |
| `mesh_decode` | `Mesh::decode` (stamp rule and look-ahead fallback), `MeshHeader::parse`, `validate`, `level_geometry` under limits, lossless re-encoding; raw records and damaged synthetic meshes | `docs/specs/mesh.md` §3–§14 |
| `explorer_mesh_scene` | the explorer's mesh-viewer conversions (`apps/explorer/src/geometry.rs`, included by path): a decoded level into renderer streams and `MeshData::prepare`; base-colour/opacity composition, alpha masks, thumbnails, grid cells on consistent and inconsistent images | `mesh.md` §11, `material.md` §11.2, §12.2, `render.md` §2.1 |

## Running

With [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and a nightly toolchain
(`rustup toolchain install nightly --profile minimal`, `cargo install --locked cargo-fuzz`):

```sh
cd fuzz
cargo +nightly fuzz build
cargo +nightly fuzz run dat_node -- -max_total_time=600 -rss_limit_mb=2048 -malloc_limit_mb=1024
```

Without nightly, `tools/fuzz-smoke.sh` builds the same targets on stable with libFuzzer's
coverage instrumentation (no AddressSanitizer, which `unsafe_code = "deny"` makes less
important) and runs each one briefly:

```sh
tools/fuzz-smoke.sh -t 60            # all targets, 60 s each
tools/fuzz-smoke.sh -t 600 dat_node  # one target
```

`-j N` runs N targets at once; a summary table with each target's run time, executions and
result is printed at the end, and full logs go to `fuzz/target/smoke-logs/`.

`cargo check` in this directory type-checks the targets on stable.

## Dictionaries

`dict/<target>.dict` (libFuzzer `-dict` format) holds the syntax tokens of a text format,
so the fuzzer reaches the inner branches of a grammar quickly. `tools/fuzz-smoke.sh` passes
it automatically; with cargo-fuzz add `-- -dict=dict/<target>.dict`. Dictionaries contain
only format syntax written by hand, never strings taken from game data.

## Rules

- **Corpora and crash artefacts are never committed** (`fuzz/corpus/`, `fuzz/artifacts/` are
  git-ignored). Seeds harvested from a real install are game data.
- A crash found by fuzzing becomes a regression test in the crate's own `tests/`, built from
  a **synthetic** input that our own code generates, not from the raw crash file if that file
  was derived from real data.
- Always run with a memory limit (`-rss_limit_mb`, `-malloc_limit_mb`): an allocation that a
  hostile header can inflate is a bug even when it does not crash.
- A new parser gets a target in the same change, registered as a `[[bin]]` in `Cargo.toml`.

## Coverage note

`dat_archive` starts from raw bytes, so libFuzzer rarely produces a valid header by chance.
`dat_node` exists to cover the B-tree and extent layers on every run. For long `dat_archive`
campaigns, seed `fuzz/corpus/dat_archive/` with small synthetic archives (one option byte
followed by an archive written by our own test fixture builder in
`crates/dat/tests/common/`), never with real archive files.
