// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-dattool`: inspect the `.dat`/`.datx` archives of your own install, read-only.
//!
//! Every subcommand opens archives read-only and never writes into the game directory.
//! `extract` and `cat --raw` write only to the path or stream the user names; extraction is
//! intended for personal inspection of data you own.

mod bytecode;
mod census;
mod detect;
mod diff;
mod fallback;
mod info;
mod input;
mod instance;
mod logic;
mod ls;
mod output;
mod read;
mod show;
mod snapshot;
mod util;
mod validate;
mod world;

use std::io::{self, BufWriter, IsTerminal, Write};
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
                  read-only.\n\nDIDs and DID prefixes are always hexadecimal, with or \
                  without 0x (`41000001` is 0x41000001). An <archive> is a file path or a \
                  short name such as `general`, `surface_aux_1` or `client_cell_2.dat`, \
                  looked up in --game-dir, $TESSERA_GAME_DIR or the auto-detected install. A \
                  <dir> is an install directory; every archive in it is opened as one set.\n\n\
                  Output files (extract, snapshot) are never written inside the directory \
                  holding the input archives, --game-dir, $TESSERA_GAME_DIR or any detected \
                  install, and never through a symlink or hard link into one.\n\n\
                  Exit status: 0 success; 1 a check failed (validate found errors, a \
                  directory walk stopped early or an archive of a <dir> could not be opened \
                  in info, stat, snapshot or census, census found a record that does not \
                  decode or a collision tree that fails its walk or key check, or diff \
                  --exit-code found differences); 2 error (bad arguments, \
                  unreadable input, or a diff whose inputs could not be read completely)."
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
    /// Decode one record and print its fields: terrain type tables (0x19), terrain-scene
    /// and scene-choice tables (0x12, 0x11), scenes (0x02), water and fog-zone tables (0x16,
    /// 0x17), the sky description (0x15), day descriptions (0x1A), game time (0x35), LOD
    /// classes (0x32), camera states (0x08), collision meshes (0x66) and cells (0x01),
    /// region descriptors (0x0F) with their table slots, and landblock records (0x80:
    /// trees, ceilings, light maps, distant-landscape sectors).
    Show(ShowArgs),
    /// Decode every record of the selected DID prefixes (default: the M4.5 decode criterion)
    /// and count decoded and failed records, items and collision-tree walks; exit 1 on any
    /// failure.
    Census(CensusArgs),
    /// Why surfaces end up without their base-colour texture (drawn white or untextured):
    /// the reason per surface record (0x31), mesh (0x06) or cell (0x01), or counts over
    /// every surface record with --all.
    Fallback(FallbackArgs),
    /// Compare two installs, archives or snapshots: added, removed and changed DIDs per
    /// archive, with a summary by DID type.
    Diff(DiffArgs),
    /// Write a compact index (DID, archive, iteration, size, time, optional content hash)
    /// that `diff` accepts in place of an install.
    Snapshot(SnapshotArgs),
    /// Decode the class library's code block (record 0x56000000): census counts and
    /// load-time checks, or the disassembly of one body. Never prints message text.
    Bytecode(BytecodeArgs),
    /// Show the instanced-content view of one object record or property set (templates,
    /// controls, skirmish, world join, battles, fellowship maker; `instances.md`).
    Instance(InstanceArgs),
    /// Show the landblock-borne logic of a landblock-info record, or count it over every
    /// record with --all: decals, phasing boxes, logic wiring, instance scripts, dramas,
    /// riders (counts, DIDs and keys only; never text).
    Logic(LogicArgs),
}

#[derive(Debug, Args)]
struct InstanceArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Object record (0x70...) or property-set (0x79...) DID (hex).
    #[arg(value_parser = parse_did, value_name = "DID")]
    did: u32,
    /// List at most N elements per list in text output (0 = all).
    #[arg(long, default_value_t = 20, value_name = "N")]
    max_list: usize,
}

#[derive(Debug, Args)]
struct LogicArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Landblock-info DID (hex, 0x8RR2XXYY).
    #[arg(value_parser = parse_did, value_name = "DID", required_unless_present = "all")]
    did: Option<u32>,
    /// Count over every landblock-info record of the input instead.
    #[arg(long, conflicts_with = "did")]
    all: bool,
    /// List at most N rows in text output (0 = all).
    #[arg(long, default_value_t = 20, value_name = "N")]
    max_list: usize,
}

#[derive(Debug, Args)]
struct BytecodeArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Library record DID (hex).
    #[arg(long, value_parser = parse_did, default_value = "0x56000000", value_name = "DID")]
    did: u32,
    /// Disassemble the body starting at or containing this code offset (hex).
    #[arg(long, value_parser = parse_did, value_name = "OFFSET")]
    body: Option<u32>,
    /// Rows of the opcode and package tables (0 = all).
    #[arg(long, default_value_t = 20, value_name = "N")]
    top: usize,
}

#[derive(Debug, Args)]
struct DetectArgs {
    /// Skip reading the housekeeping records (content fingerprint, split-set check).
    #[arg(long)]
    schema_only: bool,
    /// Print only the default install's directory (for
    /// `export TESSERA_GAME_DIR="$(tessera-dattool detect --path)"`); exit 1 if none.
    #[arg(long, conflicts_with = "schema_only")]
    path: bool,
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
    /// Output file. Refused inside the directory holding the input archive(s), the game
    /// directory or any detected install.
    #[arg(short, long, value_name = "PATH")]
    output: PathBuf,
    /// Convert a render surface (0x41) to PNG.
    #[arg(long, conflicts_with_all = ["text", "stored"])]
    png: bool,
    /// Dump a string table (0x25) as `token<TAB>text` lines (backslash, newline, CR and tab
    /// escaped as \\, \n, \r and \t).
    #[arg(long, conflicts_with_all = ["png", "stored"])]
    text: bool,
    /// Write the stored on-disk bytes instead of the inflated payload.
    #[arg(long)]
    stored: bool,
    /// Replace an existing output file (by renaming a new file over it; a symlink or hard
    /// link at that path is replaced, never written through).
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

#[derive(Debug, Args)]
struct ShowArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Record DID (hex).
    #[arg(value_parser = parse_did, value_name = "DID")]
    did: u32,
    /// List at most N rows per list in text output (0 = all). JSON lists every row, but
    /// summarises property sets, the embedded render mesh of cell geometry, landblock
    /// info, block maps and block-map descriptors as counts.
    #[arg(long, default_value_t = 20, value_name = "N")]
    max_list: usize,
}

#[derive(Debug, Args)]
struct CensusArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Only DIDs whose high byte is this (hex; repeatable). Default: the kinds of the M4.5
    /// decode criterion, 01 02 08 0F 11 12 15 16 17 19 1A 2B 30 31 32 35 66. 80 selects the
    /// landblock family, 06 the meshes (their level trees are part of the criterion; the
    /// mesh archives are large, so they are not in the default).
    #[arg(long, value_parser = parse_prefix, value_name = "0xHH")]
    prefix: Vec<u8>,
    /// List at most N failed records (0 = none).
    #[arg(long, default_value_t = 20, value_name = "N")]
    max_failures: usize,
}

#[derive(Debug, Args)]
struct FallbackArgs {
    /// Install directory.
    #[arg(value_name = "DIR")]
    target: String,
    /// Surface records (0x31), meshes (0x06) or cells (0x01), hex.
    #[arg(value_parser = parse_did, value_name = "DID")]
    dids: Vec<u32>,
    /// Diagnose every surface record and print counts by reason and material kind.
    #[arg(long)]
    all: bool,
    /// Do not decode the base textures (material-level reasons only; faster).
    #[arg(long)]
    no_decode: bool,
    /// With --all: list at most N surfaces per reason and kind (0 = none).
    #[arg(long, default_value_t = 10, value_name = "N")]
    top: usize,
    /// With --all: list only water, mirror and liquid-distortion surfaces (records with
    /// programs of their own).
    #[arg(long)]
    water: bool,
}

#[derive(Debug, Args)]
struct DiffArgs {
    /// Older side: install directory, archive, or snapshot file.
    #[arg(value_name = "A")]
    a: String,
    /// Newer side: install directory, archive, or snapshot file.
    #[arg(value_name = "B")]
    b: String,
    /// Also read, inflate and hash the payloads of live inputs that can be paired with the
    /// other side (only under --prefix, if given), and count a differing hash as a change
    /// (slow). Two snapshots taken with --content are compared by hash without this flag.
    #[arg(long)]
    content: bool,
    /// Only DIDs whose high byte is this (hex, e.g. 0x41).
    #[arg(long, value_parser = parse_prefix, value_name = "0xHH")]
    prefix: Option<u8>,
    /// List at most N DIDs per archive and category in text output (0 = none).
    #[arg(long, default_value_t = 20, value_name = "N")]
    max_list: usize,
    /// Only print counts, no DID lists (also shortens --json output).
    #[arg(long)]
    summary: bool,
    /// Exit with status 1 when differences are found (0 when none). Errors, and a
    /// comparison left incomplete (a directory walk that stopped early, an archive that
    /// could not be opened, or a payload whose content could not be hashed), exit with 2
    /// whether or not this is given.
    #[arg(long)]
    exit_code: bool,
}

#[derive(Debug, Args)]
struct SnapshotArgs {
    /// Install directory or archive.
    #[arg(value_name = "DIR|ARCHIVE")]
    target: String,
    /// Snapshot file to write. Refused inside the directory holding the input archive(s),
    /// the game directory or any detected install.
    #[arg(short, long, value_name = "PATH")]
    output: PathBuf,
    /// Store a content hash of every inflated payload (reads every record; slow).
    #[arg(long)]
    content: bool,
    /// Replace an existing output file (by renaming a new file over it; a symlink or hard
    /// link at that path is replaced, never written through).
    #[arg(short, long)]
    force: bool,
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
        .with_ansi(io::stderr().is_terminal())
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
        Cmd::Detect(a) if a.path => detect::run_path(&mut out, &ctx)?,
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
        Cmd::Show(a) => show::run(
            &mut out,
            &ctx,
            &show::Opts {
                target: a.target,
                did: a.did,
                max_list: a.max_list,
            },
        )?,
        Cmd::Instance(a) => instance::run(
            &mut out,
            &ctx,
            &instance::Opts {
                target: a.target,
                did: a.did,
                max_list: a.max_list,
            },
        )?,
        Cmd::Logic(a) => logic::run(
            &mut out,
            &ctx,
            &logic::Opts {
                target: a.target,
                did: a.did,
                all: a.all,
                max_list: a.max_list,
            },
        )?,
        Cmd::Bytecode(a) => bytecode::run(
            &mut out,
            &ctx,
            &bytecode::Opts {
                target: a.target,
                did: a.did,
                body: a.body,
                top: a.top,
            },
        )?,
        Cmd::Census(a) => census::run(
            &mut out,
            &ctx,
            &census::Opts {
                target: a.target,
                prefixes: a.prefix,
                max_failures: a.max_failures,
            },
        )?,
        Cmd::Fallback(a) => fallback::run(
            &mut out,
            &ctx,
            &fallback::Opts {
                target: a.target,
                dids: a.dids,
                all: a.all,
                decode: !a.no_decode,
                top: a.top,
                water: a.water,
            },
        )?,
        Cmd::Diff(a) => diff::run(
            &mut out,
            &ctx,
            &diff::Opts {
                a: a.a,
                b: a.b,
                content: a.content,
                prefix: a.prefix,
                max_list: a.max_list,
                summary: a.summary,
                exit_code: a.exit_code,
            },
        )?,
        Cmd::Snapshot(a) => snapshot::run(
            &mut out,
            &ctx,
            &snapshot::Opts {
                target: a.target,
                output: a.output,
                content: a.content,
                force: a.force,
            },
        )?,
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

/// Exit status of every error (the same as clap's usage errors), kept apart from status 1,
/// which means "a check failed" or, for `diff --exit-code`, "differences found".
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.quiet);
    match run(cli) {
        Ok(code) => code,
        Err(e) if is_broken_pipe(&e) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(EXIT_ERROR)
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
        let c =
            Cli::try_parse_from(["t", "diff", "a", "b", "--content", "--prefix", "41"]).unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Diff(DiffArgs {
                content: true,
                prefix: Some(0x41),
                max_list: 20,
                ..
            })
        ));
        assert!(Cli::try_parse_from(["t", "snapshot", "/x"]).is_err());
        let c = Cli::try_parse_from(["t", "show", "/x", "19000001", "--max-list", "0"]).unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Show(ShowArgs {
                did: 0x1900_0001,
                max_list: 0,
                ..
            })
        ));
        let c = Cli::try_parse_from(["t", "census", "/x", "--prefix", "66", "--prefix", "0x80"])
            .unwrap();
        assert!(
            matches!(c.cmd, Cmd::Census(CensusArgs { ref prefix, .. }) if prefix == &[0x66, 0x80])
        );
        assert!(Cli::try_parse_from(["t", "snapshot", "/x", "-o", "f", "--content"]).is_ok());
        let c = Cli::try_parse_from(["t", "fallback", "/x", "3100053C", "0x06000001", "--water"])
            .unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Fallback(FallbackArgs { ref dids, water: true, all: false, top: 10, .. })
                if dids == &[0x3100_053C, 0x0600_0001]
        ));
        assert!(Cli::try_parse_from(["t", "fallback", "/x", "--all", "--no-decode"]).is_ok());
    }
}
