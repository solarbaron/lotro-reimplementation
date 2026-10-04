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
| `net_packet_decode` | datagram decode/encode round trip | `docs/specs/transport-design.md` |
| `net_endpoint` | reliable endpoint state machine under hostile datagrams | `transport-design.md` |

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

`cargo check` in this directory type-checks the targets on stable.

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
