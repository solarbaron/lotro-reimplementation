# 0002 — Rust + wgpu, GPL-3.0-or-later

**Status:** Accepted, 2026-10-03 (chosen by the owner over C++20/Vulkan and C#/Silk.NET)

## Decision
- Rust (edition 2024, stable toolchain via `rust-toolchain.toml`), cargo workspace.
- Rendering: our own renderer on wgpu (`crates/render`), aiming at the original client's look.
- UI tooling: egui/eframe for the explorer only.
- License: GPL-3.0-or-later for all project code (SPDX header on every source file).

## Consequences
Memory-safe parsing of hostile binary data; fuzzing via cargo-fuzz/libFuzzer. No existing
LOTRO code is linked or ported; everything is written from specs.
