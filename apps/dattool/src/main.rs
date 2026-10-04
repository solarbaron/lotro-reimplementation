// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-dattool`: inspect the `.dat`/`.datx` archives of your own install, read-only.
//!
//! Every subcommand opens archives read-only and never writes into the game directory.
//! `extract` and `cat --raw` write only to the path or stream the user names; extraction is
//! intended for personal inspection of data you own.

mod detect;
mod info;
mod input;
mod ls;
mod read;
mod util;
mod validate;

use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use crate::input::Ctx;
use crate::util::{parse_did, parse_prefix};

#[derive(Debug, Parser)]
#[command(
    name = "tessera-dattool",
    version,
    about = "Inspect the .dat/.datx archives of your own game install (read-only)",
    long_about = "Inspect the .dat/.datx archives of a game install you own, in place and \
                  read-only.\n\nDIDs and DID prefixes are hexadecimal, with or without 0x. \
                  An <archive> is a file path or a short name such as `general`, \
                  `surface_aux_1` or `client_cell_2.dat`, looked up in --game-dir, \
                  $TESSERA_GAME_DIR or the auto-detected install. A <dir> is an install \
                  directory; every archive in it is opened as one set."
)]
struct Cli {
    /// Print machine-readable JSON instead of tables.
    #[arg(long, global = true)]
    json: bool,

    /// Game directory used to resolve bare archive names (default: $TESSERA_GAME_DIR, then
    /// auto-detection).
    #[arg(long, global = true, value_name = "DIR")]
    game_dir: Option<PathBuf>,

    /// More log output on stderr (-v info, -vv debug, -vvv trace). RUST_LOG overrides.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Only log errors.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Find installs (explicit dir, $TESSERA_GAME_DIR, Steam, launchers, Wine prefixes) and
    /// print each with its fingerprints.
    Detect(DetectArgs),
    /// Header fields, entry count and record size statistics of an archive or every
    /// archive of a directory.
    Info(TargetArgs),
    /// List the directory entries of one archive.
    Ls(LsArgs),
    /// Per-archive histogram of DID high bytes (record types).
    Stat(StatArgs),
    /// Hexdump a record's payload.
    Cat(CatArgs),
    /// Write a record's payload to a file (personal use; never into the game directory).
    Extract(ExtractArgs),
    /// Check every archive against the container invariants; non-zero exit on errors.
    Validate(ValidateArgs),
    /// Print the messages of a string table (DID prefix 0x25).
    Strings(StringsArgs),
}

#[derive(Debug, Args)]
struct DetectArgs {
    /// Skip reading the housekeeping records (content fingerprint, split-set check).
    #[arg(long)]
    schema_only: bool,
}

#[derive(Debug, Args)]
struct TargetArgs {
    /// Install directory or archive.
    #[arg(value_name = "ARCHIVE|DIR")]
    target: String,
}

#[derive(Debug, Args)]
struct LsArgs {
    /// Archive file or name.
    #[arg(value_name = "ARCHIVE")]
    archive: String,
    /// Only DIDs whose high byte is this (hex, e.g. 0x41).
    #[arg(long, value_parser = parse_prefix, value_name = "0xHH")]
    prefix: Option<u8>,
    /// Start at this DID (hex).
    #[arg(long, value_parser = parse_did, value_name = "DID")]
    from: Option<u32>,
    /// Print at most N entries.
    #[arg(short = 'n', long, value_name = "N")]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
struct StatArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Only print the combined histogram over all archives.
    #[arg(long)]
    totals: bool,
}

#[derive(Debug, Args)]
struct CatArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Record DID (hex).
    #[arg(value_parser = parse_did, value_name = "DID")]
    did: u32,
    /// Write the payload bytes to stdout instead of a hexdump (refused on a terminal).
    #[arg(long)]
    raw: bool,
    /// Use the stored on-disk bytes (still zlib-compressed if the record is) instead of
    /// the inflated payload.
    #[arg(long)]
    stored: bool,
    /// Hexdump at most N bytes (0 = all).
    #[arg(long, default_value_t = 1024, value_name = "N")]
    max_bytes: usize,
}

#[derive(Debug, Args)]
struct ExtractArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Record DID (hex).
    #[arg(value_parser = parse_did, value_name = "DID")]
    did: u32,
    /// Output file. Must not be inside the game directory.
    #[arg(short, long, value_name = "PATH")]
    output: PathBuf,
    /// Convert a render surface (0x41) to PNG.
    #[arg(long, conflicts_with_all = ["text", "stored"])]
    png: bool,
    /// Dump a string table (0x25) as `token<TAB>text` lines (newlines and tabs escaped).
    #[arg(long, conflicts_with_all = ["png", "stored"])]
    text: bool,
    /// Write the stored on-disk bytes instead of the inflated payload.
    #[arg(long)]
    stored: bool,
    /// Overwrite an existing output file.
    #[arg(short, long)]
    force: bool,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Fully inflate every compressed record (slow).
    #[arg(long)]
    inflate: bool,
    /// Skip the allocation tiling check.
    #[arg(long)]
    no_tiling: bool,
    /// Print at most N issue lines per archive.
    #[arg(long, default_value_t = 10, value_name = "N")]
    max_issues: usize,
}

#[derive(Debug, Args)]
struct StringsArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// String table DID (hex, prefix 0x25).
    #[arg(value_parser = parse_did, value_name = "DID")]
    did: u32,
    /// Only print this token (hex).
    #[arg(long, value_parser = parse_did, value_name = "TOKEN")]
    token: Option<u32>,
}

/// Output sink and format shared by the subcommands.
pub struct Out {
    /// Buffered stdout.
    pub w: Box<dyn Write>,
    /// `--json` was given.
    pub json: bool,
}

impl std::fmt::Debug for Out {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Out")
            .field("json", &self.json)
            .finish_non_exhaustive()
    }
}

impl Out {
    /// Writes a JSON value followed by a newline.
    pub fn json(&mut self, v: &serde_json::Value) -> anyhow::Result<()> {
        serde_json::to_writer_pretty(&mut self.w, v)?;
        writeln!(self.w)?;
        Ok(())
    }
}

fn init_logging(verbose: u8, quiet: bool) {
    let level = if quiet {
        "error"
    } else {
        match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_target(false)
        .try_init();
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let ctx = Ctx {
        game_dir: cli.game_dir.clone(),
    };
    let mut out = Out {
        w: Box::new(BufWriter::new(io::stdout().lock())),
        json: cli.json,
    };
    let code = match cli.cmd {
        Cmd::Detect(a) => detect::run(&mut out, &ctx, !a.schema_only)?,
        Cmd::Info(a) => info::info(&mut out, &ctx, &a.target)?,
        Cmd::Ls(a) => ls::run(&mut out, &ctx, &a.archive, a.prefix, a.from, a.limit)?,
        Cmd::Stat(a) => info::stat(&mut out, &ctx, &a.target, a.totals)?,
        Cmd::Cat(a) => read::cat(
            &mut out,
            &ctx,
            &read::CatOpts {
                target: a.target,
                did: a.did,
                raw: a.raw,
                stored: a.stored,
                max_bytes: a.max_bytes,
            },
        )?,
        Cmd::Extract(a) => read::extract(
            &mut out,
            &ctx,
            &read::ExtractOpts {
                target: a.target,
                did: a.did,
                output: a.output,
                mode: if a.png {
                    read::ExtractMode::Png
                } else if a.text {
                    read::ExtractMode::Text
                } else if a.stored {
                    read::ExtractMode::Stored
                } else {
                    read::ExtractMode::Payload
                },
                force: a.force,
            },
        )?,
        Cmd::Validate(a) => validate::run(
            &mut out,
            &ctx,
            &a.target,
            &validate::Opts {
                inflate: a.inflate,
                tiling: !a.no_tiling,
                max_issues: a.max_issues,
            },
        )?,
        Cmd::Strings(a) => read::strings(&mut out, &ctx, &a.target, a.did, a.token)?,
    };
    out.w.flush()?;
    Ok(code)
}

fn is_broken_pipe(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<io::Error>()
            .is_some_and(|io| io.kind() == io::ErrorKind::BrokenPipe)
            || c.downcast_ref::<serde_json::Error>()
                .and_then(|j| j.io_error_kind())
                .is_some_and(|k| k == io::ErrorKind::BrokenPipe)
    })
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.quiet);
    match run(cli) {
        Ok(code) => code,
        Err(e) if is_broken_pipe(&e) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn hex_args_parse() {
        let c = Cli::try_parse_from(["t", "cat", "/x", "41000001"]).unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Cat(CatArgs {
                did: 0x4100_0001,
                ..
            })
        ));
        let c = Cli::try_parse_from(["t", "ls", "general", "--prefix", "0x25", "-n", "3"]).unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Ls(LsArgs {
                prefix: Some(0x25),
                limit: Some(3),
                ..
            })
        ));
        assert!(Cli::try_parse_from(["t", "cat", "/x", "0xnothex"]).is_err());
        assert!(
            Cli::try_parse_from(["t", "extract", "/x", "1", "-o", "f", "--png", "--text"]).is_err()
        );
        assert!(Cli::try_parse_from(["t", "extract", "/x", "1"]).is_err());
    }
}
