// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-server` binary: see the library documentation and `docs/specs/test-server.md`.

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(tessera_server::cli::main())
}
