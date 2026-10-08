// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line options.

use std::path::PathBuf;

use clap::Parser;
use tessera_formats::did::DidKind;

/// Largest `--settle-timeout` and `--data-timeout` (one day), so deadlines never overflow.
pub const MAX_TIMEOUT_SECS: u64 = 86_400;
/// Largest `--fly-speed`: boosted (× 5) it stays within the sandbox's speed limit.
pub const MAX_FLY_SPEED: f32 = 2_000.0;
const _: () = assert!(MAX_FLY_SPEED * 5.0 <= tessera_net::sandbox::MAX_SPEED);

/// Tessera client.
///
/// Reads the data files of an existing install in place and read-only. Without an
/// install it runs in no-data mode and shows an original test scene.
#[derive(Debug, Clone, Parser)]
#[command(name = "tessera-client", version, about)]
#[command(group(
    clap::ArgGroup::new("headless_task")
        .args(["screenshot", "fly_path", "duration", "walk_path"])
        .multiple(true)
))]
pub struct Args {
    /// Install directory (holds `client_general.dat`). Defaults to `TESSERA_GAME_DIR`,
    /// then automatic discovery.
    #[arg(long, value_name = "DIR")]
    pub game_dir: Option<PathBuf>,

    /// Do not look for an install; run the original test scene only.
    #[arg(long, conflicts_with = "game_dir")]
    pub no_data: bool,

    /// Render-surface DID to show (e.g. `0x41000123`). Default: the first surface in the
    /// archives that decodes.
    #[arg(long, value_name = "DID", value_parser = parse_surface_did)]
    pub surface: Option<u32>,

    /// Skip counting directory entries of every archive (faster start-up log).
    #[arg(long)]
    pub no_entry_count: bool,

    /// Plugins folder to scan (read-only). Default: `<data dir>/Plugins`.
    #[arg(long, value_name = "DIR")]
    pub plugins: Option<PathBuf>,

    /// Plugin to load after discovery (by descriptor name). Repeatable.
    #[arg(long = "load-plugin", value_name = "NAME")]
    pub load_plugins: Vec<String>,

    /// Tessera's own data folder (plugin data). Default: `$XDG_DATA_HOME/tessera`.
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Initialise everything, render frames offscreen without a window, then exit.
    #[arg(long)]
    pub smoke: bool,

    /// Number of frames rendered by `--smoke`.
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u32).range(1..=10_000))]
    pub frames: u32,

    /// Seconds `--smoke` waits for the data loader before rendering anyway (at most a day).
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(0..=MAX_TIMEOUT_SECS))]
    pub data_timeout: u64,

    /// Write the last `--smoke` frame as a binary PPM image to this path.
    #[arg(long, value_name = "FILE", requires = "smoke")]
    pub smoke_out: Option<PathBuf>,

    /// Windowed mode: close the window after this many presented frames (for scripted
    /// checks of the window path).
    #[arg(long, value_name = "N", conflicts_with = "smoke")]
    pub exit_after_frames: Option<u64>,

    /// Window / offscreen size as `WIDTHxHEIGHT`.
    #[arg(long, default_value = "1280x720", value_parser = parse_size)]
    pub size: (u32, u32),

    /// Disable vsync (present mode `Immediate`, else `Mailbox` when the surface lacks it,
    /// else `Fifo`).
    #[arg(long)]
    pub no_vsync: bool,

    // ----- world walker -----
    /// Show the M0 texture showcase instead of the world walker.
    #[arg(long, conflicts_with_all = ["spawn", "place", "fly_path", "screenshot"])]
    pub showcase: bool,

    /// Spawn at a landblock: `R,X,Y` (block centre), `R,X,Y,x,y` (block-local) or
    /// `R,X,Y,x,y,z` (exact height).
    #[arg(long, value_name = "R,X,Y[,x,y[,z]]", value_parser = parse_spawn, conflicts_with = "place")]
    pub spawn: Option<String>,

    /// Spawn at a named place: an area, territory or region name, map coordinates such as
    /// `12.3S, 45.6W`, or a pasted `/loc` line.
    #[arg(long, value_name = "NAME")]
    pub place: Option<String>,

    /// Region for `--place` names and map coordinates, and for `--fly-path` points
    /// without one.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=i64::from(crate::walker::parse::MAX_REGION)))]
    pub region: u8,

    /// Start in free-fly mode instead of walking.
    #[arg(long)]
    pub fly: bool,

    /// Base free-fly speed in m/s, at most 2000 (Shift multiplies by 5; the wheel scales
    /// it). Faster movement would exceed the world session's speed limit.
    #[arg(long, default_value_t = 60.0, value_parser = parse_fly_speed)]
    pub fly_speed: f32,

    /// Fly a straight line `FROM,TO`, each `X:Y`, `R:X:Y`, `X:Y:x:y` or `R:X:Y:x:y`
    /// (landblock, optional block-local offset), then exit.
    #[arg(long, value_name = "FROM,TO", value_parser = parse_fly_path_arg)]
    pub fly_path: Option<crate::walker::parse::FlyPath>,

    /// `--fly-path` speed in m/s.
    #[arg(long, default_value_t = 1_000.0, value_parser = parse_positive_f64)]
    pub speed: f64,

    /// `--fly-path` height above the terrain in metres.
    #[arg(long, default_value_t = 120.0, value_parser = parse_positive_f64)]
    pub fly_height: f64,

    /// Write per-frame times and residency as CSV (written when the walker exits).
    #[arg(long, value_name = "CSV")]
    pub frame_log: Option<PathBuf>,

    /// Render the spawn point offscreen once everything around it has loaded, save it
    /// as PNG and exit.
    #[arg(long, value_name = "PNG")]
    pub screenshot: Option<PathBuf>,

    /// Run the walker offscreen without a window (implied by `--screenshot` and
    /// `--walk-path`; needs `--screenshot`, `--fly-path`, `--duration` or `--walk-path`, which
    /// end the run).
    #[arg(long, requires = "headless_task")]
    pub headless: bool,

    /// Seconds `--screenshot`, `--fly-path` and `--duration` wait for the area to load, and
    /// the longest a `--walk-path` walk waits for a collision set (1 to 86,400).
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u64).range(1..=MAX_TIMEOUT_SECS))]
    pub settle_timeout: u64,

    /// Keymap file (Tessera's TOML keymap). Default: `keymap.toml` next to the settings.
    #[arg(long, value_name = "FILE")]
    pub keymap: Option<PathBuf>,

    /// Residency budget of landblock bundles in MiB (bundles carry their scenery, water and
    /// collision; the default rings out to 2,080 m hold about 600 landblocks, up to about
    /// 4.6 MiB each in dense areas, 2.7 GiB in all).
    #[arg(long, default_value_t = 4096, value_parser = clap::value_parser!(u64).range(16..=65_536))]
    pub residency_mib: u64,

    /// Terrain view distance in landblocks (far ring radius). Default: the landscape extent
    /// of `--landscape-distance` (2,080 m at the built-in level High, `walker.md` §11.3).
    #[arg(long, value_parser = clap::value_parser!(u8).range(2..=30))]
    pub view_blocks: Option<u8>,

    /// Cap on every object's draw distance in metres (the per-class distances of
    /// `walker.md` §11.4 apply below it). Default: the landscape extent.
    #[arg(long, value_parser = parse_positive_f64)]
    pub object_distance: Option<f64>,

    /// GPU memory for object meshes, models and textures (terrain layer textures
    /// included) in MiB (terrain meshes get their own share, scaled with the view).
    #[arg(long, default_value_t = 1536, value_parser = clap::value_parser!(u64).range(64..=65_536))]
    pub gpu_mib: u64,

    /// `LandscapeDrawDistance` (`walker.md` §11.2): VeryLow, Low, Medium, High (built-in)
    /// or VeryHigh.
    #[arg(long, value_name = "LEVEL", value_parser = parse_landscape_level)]
    pub landscape_distance: Option<tessera_world::draw_distance::LandscapeLevel>,

    /// `ObjectDrawDistance` (`walker.md` §11.2): Low, Medium, High (built-in), VeryHigh or
    /// UltraHigh.
    #[arg(long, value_name = "LEVEL", value_parser = parse_detail_level)]
    pub object_draw_distance: Option<tessera_world::DetailLevel>,

    /// `ModelDetail` (`walker.md` §11.2): Low, Medium, High (built-in), VeryHigh or
    /// UltraHigh.
    #[arg(long, value_name = "LEVEL", value_parser = parse_detail_level)]
    pub model_detail: Option<tessera_world::DetailLevel>,

    /// `FrillDistance` level 0–5 (`scenery.md` §8.3; built-in 2).
    #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(u8).range(0..=5))]
    pub frill_distance: Option<u8>,

    /// Switch M4.5 systems off for profiling (`walker.md` §14.2), comma-separated:
    /// layers, lit, shadows, lights, water, sky, fog, scenery, instancing, frills, lod;
    /// `all` for the M3/M4 walker.
    #[arg(long, value_name = "LIST", value_parser = parse_systems_arg)]
    pub disable: Option<crate::walker::fxparse::SystemList>,

    /// Game day number the clock starts on (`environment.md` §11.1; default 12,240).
    /// At most ±20,000,000 (`walker::env::MAX_START_DAY`: days the game clock holds).
    #[arg(long, value_name = "N", allow_negative_numbers = true, value_parser = clap::value_parser!(i64).range(-crate::walker::env::MAX_START_DAY..=crate::walker::env::MAX_START_DAY))]
    pub day: Option<i64>,

    /// Time of day the clock starts at, 0–1 (0.5 is noon; default 0.5).
    #[arg(long, value_name = "T", value_parser = parse_unit)]
    pub time_of_day: Option<f64>,

    /// Game seconds per real second (0 stops the clock; default 1).
    #[arg(long, value_name = "RATE", value_parser = parse_rate)]
    pub time_rate: Option<f64>,

    /// Day type *D* (`environment.md` §11.2; default 2).
    #[arg(long, value_name = "D")]
    pub day_type: Option<u32>,

    /// Fly the camera around a closed loop for a timing run (`m4-5-visual.md` §3):
    /// `R:X:Y:x,y;x,y[;…]`, block-local waypoints of landblock (X, Y) in region R.
    #[arg(long, value_name = "LOOP", value_parser = parse_camera_loop_arg, conflicts_with = "fly_path")]
    pub camera_loop: Option<crate::walker::fxparse::CameraLoop>,

    /// Terrain blocks farther than this (metres) are drawn with their texture layers baked
    /// into one image (`walker.md` §14.4); 0 draws every layer at every distance.
    #[arg(long, value_name = "M", default_value_t = crate::walker::DEFAULT_FAR_BAND, value_parser = parse_far_band)]
    pub far_band: f32,

    /// Hide the HUD and the minimap (clean screenshots).
    #[arg(long)]
    pub no_hud: bool,

    /// `--camera-loop` speed in m/s.
    #[arg(long, default_value_t = 6.5, value_parser = parse_positive_f64)]
    pub loop_speed: f64,

    /// `--camera-loop` height above the higher of terrain and water, metres.
    #[arg(long, default_value_t = 2.0, value_parser = parse_positive_f64)]
    pub loop_height: f64,

    /// `--camera-loop` pitch in degrees (negative looks down).
    #[arg(long, default_value_t = -10.0, allow_negative_numbers = true, value_parser = parse_pitch)]
    pub loop_pitch: f32,

    /// Turn the camera on the spot at this rate (degrees per second, clockwise) during a
    /// timing run (`m4-5-visual.md` §3 stationary spots).
    #[arg(long, value_name = "DEG", allow_negative_numbers = true, value_parser = parse_degrees, conflicts_with_all = ["fly_path", "camera_loop"])]
    pub turn_rate: Option<f32>,

    /// Largest texture edge used for objects.
    #[arg(long, default_value_t = 512, value_parser = clap::value_parser!(u32).range(16..=4096))]
    pub texture_cap: u32,

    /// Facing at the spawn as a compass heading in degrees (0 = north, 90 = east).
    #[arg(long, value_name = "DEG", value_parser = parse_degrees)]
    pub heading: Option<f32>,

    /// Camera pitch at the spawn in degrees (negative looks down).
    #[arg(long, value_name = "DEG", default_value_t = -14.0, allow_negative_numbers = true, value_parser = parse_pitch)]
    pub pitch: f32,

    /// Height of the fly camera above the spawn in metres.
    #[arg(long, default_value_t = 30.0, value_parser = parse_positive)]
    pub eye_height: f32,

    /// Also draw interior (cell) entities.
    #[arg(long)]
    pub interiors: bool,

    /// The avatar: `SPECIES,SEX` (a race-table label or key and m/f) or a figure DID,
    /// then `;ITEM[@dye=CODE][@dye2=CODE][@wielded]` per worn or held item (held items
    /// are stowed unless `@wielded`). Default `human,m`.
    #[arg(long, value_name = "SPEC", value_parser = parse_avatar_arg)]
    pub avatar: Option<crate::avatar::AvatarSpec>,

    /// Place N animated characters (every playable body in turn) on a grid in front of
    /// the spawn, standing, walking and running.
    #[arg(long, value_name = "N", default_value_t = 0, value_parser = parse_crowd)]
    pub crowd: usize,

    /// Spacing of the `--crowd` grid in metres (0.5 to 100).
    #[arg(long, value_name = "M", default_value_t = 3.0, value_parser = parse_spacing)]
    pub crowd_spacing: f32,

    /// Distance of the `--crowd`'s first row in front of the spawn, metres (0 to 1000).
    #[arg(long, value_name = "M", default_value_t = 8.0, value_parser = parse_ahead)]
    pub crowd_ahead: f32,

    /// Content layers treated as active (`entity-descriptor.md` §4.5.1), comma-separated
    /// keys (decimal or 0x hex). Default `1`.
    #[arg(long, value_name = "LIST", default_value = "1", value_delimiter = ',', num_args = 1..=64, value_parser = parse_layer)]
    pub content_layers: Vec<u32>,

    /// Characters further than this (metres) keep their last pose instead of being
    /// sampled every frame.
    #[arg(long, value_name = "M", default_value_t = 200.0, value_parser = parse_positive)]
    pub anim_distance: f32,

    /// Cap on the characters' draw distance (metres). Below it characters are drawn to
    /// min(*G*, 80 × *M_d*) with a 1.5 s fade (`PlayerAvatar` and `Creature` `d0`,
    /// `walker.md` §11.4.2); default 400.
    #[arg(long, value_name = "M", value_parser = parse_positive)]
    pub character_distance: Option<f32>,

    /// Also draw weenie markers (housing hooks, distant-battle units) as static figures.
    #[arg(long)]
    pub markers: bool,

    /// Run for this many seconds once the spawn area has settled (or the settle timeout
    /// passed), with a frame log, then exit (a timing run; `--frame-log` saves it).
    #[arg(long, value_name = "SECS", value_parser = clap::value_parser!(u64).range(1..=MAX_TIMEOUT_SECS))]
    pub duration: Option<u64>,

    /// Move the avatar forward on its own from the start (`walk` or `run`; scripted
    /// screenshots and timing runs).
    #[arg(long, value_name = "GAIT", value_parser = ["walk", "run"])]
    pub autorun: Option<String>,

    /// Headless runs: render as fast as possible instead of pacing at 60 frames per second.
    #[arg(long)]
    pub unpaced: bool,

    /// Run scripted walks headless, without a GPU, on the collision walk mode, then exit:
    /// a walk file (`docs/checklists/m4-5-visual.md` §2.1 file form) or a Markdown text
    /// holding the walk table (the checklist itself runs all its walks). Fails when a walk
    /// fails its expected outcome. The walks run offline, so `--server` does not apply.
    #[arg(long, value_name = "FILE", conflicts_with_all = ["screenshot", "fly_path", "duration", "showcase", "smoke", "no_data", "server"])]
    pub walk_path: Option<PathBuf>,

    /// Write the `--walk-path` movement log (one CSV row per 1/60 s step: position, state,
    /// cell, contacts) to this file.
    #[arg(long, value_name = "CSV", requires = "walk_path")]
    pub walk_log: Option<PathBuf>,

    // ----- Tessera test server (M7) -----
    /// Play on a Tessera test server instead of the built-in sandbox: `HOST:PORT` or a
    /// connect string `tessera://HOST:PORT[?pin=sha256:...]`. The password comes from
    /// `TESSERA_PASSWORD` or a prompt, never from the command line. The server places
    /// the character, so the sandbox's `--spawn`, `--place` and `--crowd` options do not
    /// apply.
    #[arg(
        long,
        value_name = "HOST:PORT",
        value_parser = parse_server,
        requires = "account",
        conflicts_with_all = ["spawn", "place", "crowd", "crowd_spacing", "crowd_ahead"]
    )]
    pub server: Option<String>,

    /// The server key's fingerprint (`sha256:` and 64 hex digits) to trust; without it,
    /// the known-servers file decides. A pin in the connect string must be the same.
    #[arg(long, value_name = "FP", value_parser = parse_fingerprint, requires = "server")]
    pub server_fingerprint: Option<tessera_net::remote::Fingerprint>,

    /// Known-servers file. Default: `known_servers` next to Tessera's settings file.
    #[arg(long, value_name = "FILE", requires = "server")]
    pub known_servers: Option<PathBuf>,

    /// Trust an unknown server key on first use and record it in the known-servers file
    /// (check the fingerprint the client logs with the operator first).
    #[arg(long, requires = "server")]
    pub trust_new_server: bool,

    /// Account on the test server.
    #[arg(long, value_name = "NAME", requires = "server")]
    pub account: Option<String>,

    /// Character to enter (default: the account's first).
    #[arg(long, value_name = "NAME", requires = "server")]
    pub character: Option<String>,

    // ----- audio -----
    /// Audio backend: `auto` (the output device, else silent mixing; the windowed
    /// default), `device`, `null` (mixed and discarded; the default for `--headless` and
    /// `--smoke`), `offline` (mixed into memory in real time, no device) or `off`. The
    /// device needs a build with `--features audio-device`.
    #[arg(long, value_name = "BACKEND", value_parser = parse_audio)]
    pub audio: Option<crate::audio::backend::BackendChoice>,

    /// Import the original client's `[Sound]` volumes, read-only: `auto` (the install's
    /// settings folder) or the preferences file. Tessera's own `[audio]` values win.
    #[arg(long, value_name = "auto|FILE")]
    pub audio_prefs: Option<String>,

    /// Where Tessera's own `[audio]` values come from: `user` (the settings file; the
    /// windowed default), `default` (none; the `--headless` default, so runs do not depend
    /// on the user's settings) or a settings file.
    #[arg(long, value_name = "user|default|FILE")]
    pub audio_settings: Option<String>,

    /// Random seed of the sound scripts (default: 0 for `--headless`, the clock otherwise).
    #[arg(long, value_name = "N")]
    pub audio_seed: Option<u64>,

    /// No positional emitters (static entities that make sound); music and ambience only.
    #[arg(long)]
    pub no_emitters: bool,

    /// A command line for the client's command dispatcher (decision 0020 item 7), for
    /// example a player-music command (`/play <name> [<part>] [sync]`, `/playstart`,
    /// `/playlist`, `/playstop`, `/instrument`) or `/help`. Repeatable; the lines run in
    /// order in the first frame after the archives are open (and the install's command
    /// words are read); replies go to the chat overlay and the log. A line that would chat
    /// (plain text, `/say`, ...) is logged and dropped, never sent. The same commands work
    /// typed in the walker's chat line. `--music-command` is the old name.
    #[arg(long = "command", visible_alias = "music-command", value_name = "LINE")]
    pub commands: Vec<String>,
}

/// Environment variable the test-server password is read from (else a prompt).
pub const PASSWORD_ENV: &str = "TESSERA_PASSWORD";

/// Normalises a `--server` value to a connect string and checks it
/// (`tessera-protocol.md` §4.4): `HOST:PORT` gets the `tessera://` scheme.
pub fn parse_server(s: &str) -> Result<String, String> {
    let t = s.trim();
    let text = if t.starts_with("tessera://") {
        t.to_owned()
    } else {
        format!("tessera://{t}")
    };
    tessera_net::remote::ConnectString::parse(&text)
        .map(|_| text)
        .map_err(|e| format!("not a server address (HOST:PORT or tessera://HOST:PORT): {e}"))
}

fn parse_fingerprint(s: &str) -> Result<tessera_net::remote::Fingerprint, String> {
    tessera_net::remote::Fingerprint::parse(s.trim()).map_err(|e| e.to_string())
}

/// Builds the test-server settings from `--server` and friends (`None` without
/// `--server`). The password is `env_password` when given (the caller reads
/// [`PASSWORD_ENV`]), else what `prompt` returns.
///
/// # Errors
///
/// No account, a `--server-fingerprint` that differs from the connect string's pin
/// (the pin is authoritative, `tessera-protocol.md` §4.5), or no password (empty, or the
/// prompt failed).
pub fn remote_settings(
    args: &Args,
    env_password: Option<String>,
    prompt: impl FnOnce(&str) -> std::io::Result<String>,
) -> anyhow::Result<Option<crate::net::RemoteSettings>> {
    let Some(server) = args.server.clone() else {
        return Ok(None);
    };
    let Some(account) = args.account.clone() else {
        anyhow::bail!("--server needs --account");
    };
    if let (Some(fp), Ok(cs)) = (
        args.server_fingerprint,
        tessera_net::remote::ConnectString::parse(&server),
    ) && cs.pin.is_some_and(|pin| pin != fp)
    {
        anyhow::bail!(
            "--server-fingerprint {fp} differs from the pin in the connect string; the pin \
             is authoritative, so give one fingerprint only"
        );
    }
    let password = match env_password.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => prompt(&account)
            .map_err(|e| anyhow::anyhow!("no password ({PASSWORD_ENV} unset): {e}"))?,
    };
    if password.is_empty() {
        anyhow::bail!("empty password");
    }
    Ok(Some(crate::net::RemoteSettings {
        server,
        fingerprint: args.server_fingerprint,
        known_servers: args
            .known_servers
            .clone()
            .or_else(crate::net::default_known_servers),
        accept_new_key: args.trust_new_server,
        account,
        password: tessera_net::remote::Password::new(password),
        character: args.character.clone(),
    }))
}

/// [`remote_settings`] with the password from [`PASSWORD_ENV`], else a prompt without echo
/// on the terminal.
///
/// # Errors
///
/// As [`remote_settings`]; without a terminal and without the variable there is no
/// password.
pub fn remote_settings_from_env(args: &Args) -> anyhow::Result<Option<crate::net::RemoteSettings>> {
    remote_settings(args, std::env::var(PASSWORD_ENV).ok(), read_password)
}

/// Longest password the prompt accepts, in bytes.
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Prompts for the password of `account` on standard error and reads one line from a
/// terminal without echo. Fails when standard input is not a terminal.
///
/// On unix the terminal is switched to non-canonical mode with echo and signal keys off
/// for the prompt ([`edit_password`] does the line editing: Ctrl-C, Ctrl-\\ and Ctrl-Z
/// cancel, so the terminal is always restored), and a guard restores the saved mode on
/// every return and on unwinding. On other platforms the prompt is refused (no echo
/// control yet): set `TESSERA_PASSWORD`.
///
/// # Errors
///
/// Not a terminal (or not unix), cancelled, too long, not UTF-8, or reading failed.
pub fn read_password(account: &str) -> std::io::Result<String> {
    use std::io::IsTerminal as _;
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(std::io::Error::other(format!(
            "standard input is not a terminal; set {PASSWORD_ENV}"
        )));
    }
    read_password_tty(account, &stdin)
}

#[cfg(unix)]
fn read_password_tty(account: &str, stdin: &std::io::Stdin) -> std::io::Result<String> {
    use rustix::termios::{
        LocalModes, OptionalActions, SpecialCodeIndex, Termios, tcgetattr, tcsetattr,
    };
    use std::io::{Read as _, Write as _};

    /// Restores the saved terminal mode when dropped.
    struct Restore<'a> {
        stdin: &'a std::io::Stdin,
        saved: Termios,
    }
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            let _ = tcsetattr(self.stdin, OptionalActions::Now, &self.saved);
        }
    }

    let saved = tcgetattr(stdin)?;
    let mut raw = saved.clone();
    raw.local_modes
        .remove(LocalModes::ECHO | LocalModes::ICANON | LocalModes::ISIG);
    raw.special_codes[SpecialCodeIndex::VMIN] = 1;
    raw.special_codes[SpecialCodeIndex::VTIME] = 0;
    eprint!("Password for {account}: ");
    let _ = std::io::stderr().flush();
    let guard = Restore { stdin, saved };
    tcsetattr(stdin, OptionalActions::Now, &raw)?;
    let r = edit_password(stdin.lock().bytes());
    drop(guard);
    eprintln!();
    r
}

#[cfg(not(unix))]
fn read_password_tty(_account: &str, _stdin: &std::io::Stdin) -> std::io::Result<String> {
    Err(std::io::Error::other(format!(
        "the password prompt cannot hide typing on this platform; set {PASSWORD_ENV}"
    )))
}

/// The prompt's line editor over raw terminal bytes (echo, canonical mode and signal keys
/// off): Enter (CR or LF) ends the line, Backspace (DEL or BS) deletes one character,
/// Ctrl-U clears the line, Ctrl-C, Ctrl-\\ and Ctrl-Z cancel, Ctrl-D or the end of input
/// on an empty line is an error, other control bytes are ignored. At most
/// [`MAX_PASSWORD_BYTES`] are kept.
///
/// # Errors
///
/// Cancelled ([`std::io::ErrorKind::Interrupted`]), no input
/// ([`std::io::ErrorKind::UnexpectedEof`]), too long or not UTF-8
/// ([`std::io::ErrorKind::InvalidData`]), or the read error.
pub fn edit_password(
    bytes: impl IntoIterator<Item = std::io::Result<u8>>,
) -> std::io::Result<String> {
    use std::io::{Error, ErrorKind};
    let mut buf: Vec<u8> = Vec::new();
    let mut too_long = false;
    let mut ended = false;
    for b in bytes {
        match b? {
            b'\r' | b'\n' => {
                ended = true;
                break;
            }
            0x03 | 0x1c | 0x1a => {
                return Err(Error::new(
                    ErrorKind::Interrupted,
                    "password prompt cancelled",
                ));
            }
            0x04 if buf.is_empty() => break,
            0x04 => {
                ended = true;
                break;
            }
            0x7f | 0x08 => {
                // One UTF-8 character: continuation bytes, then the lead byte.
                while let Some(last) = buf.pop() {
                    if last & 0xC0 != 0x80 {
                        break;
                    }
                }
            }
            0x15 => {
                buf.clear();
                too_long = false;
            }
            c if c < 0x20 => {}
            c if buf.len() < MAX_PASSWORD_BYTES => buf.push(c),
            _ => too_long = true,
        }
    }
    if !ended && buf.is_empty() {
        return Err(Error::new(ErrorKind::UnexpectedEof, "no password typed"));
    }
    if too_long {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("password longer than {MAX_PASSWORD_BYTES} bytes"),
        ));
    }
    String::from_utf8(buf).map_err(|_| Error::new(ErrorKind::InvalidData, "password is not UTF-8"))
}

fn parse_systems_arg(s: &str) -> Result<crate::walker::fxparse::SystemList, String> {
    crate::walker::fxparse::parse_systems(s).map(crate::walker::fxparse::SystemList)
}

fn parse_camera_loop_arg(s: &str) -> Result<crate::walker::fxparse::CameraLoop, String> {
    crate::walker::fxparse::parse_camera_loop(s)
}

fn parse_landscape_level(s: &str) -> Result<tessera_world::draw_distance::LandscapeLevel, String> {
    tessera_world::draw_distance::LandscapeLevel::from_name(s)
        .ok_or_else(|| "expected VeryLow, Low, Medium, High or VeryHigh".to_owned())
}

fn parse_detail_level(s: &str) -> Result<tessera_world::DetailLevel, String> {
    tessera_world::DetailLevel::from_name(s)
        .ok_or_else(|| "expected Low, Medium, High, VeryHigh or UltraHigh".to_owned())
}

fn parse_far_band(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (0.0..=100_000.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be a number in 0..=100000".into())
    }
}

fn parse_unit(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be a number in 0..=1".into())
    }
}

fn parse_rate(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (0.0..=crate::walker::env::MAX_TIME_RATE).contains(&v) {
        Ok(v)
    } else {
        Err(format!(
            "must be a number in 0..={}",
            crate::walker::env::MAX_TIME_RATE
        ))
    }
}

fn parse_audio(s: &str) -> Result<crate::audio::backend::BackendChoice, String> {
    crate::audio::backend::BackendChoice::parse(s)
}

fn parse_spacing(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if tessera_net::sandbox::CROWD_SPACING_RANGE.contains(&v) {
        Ok(v)
    } else {
        Err("must be a number in 0.5..=100".into())
    }
}

fn parse_ahead(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if (0.0..=tessera_net::sandbox::MAX_CROWD_AHEAD).contains(&v) {
        Ok(v)
    } else {
        Err("must be a number in 0..=1000".into())
    }
}

fn parse_crowd(s: &str) -> Result<usize, String> {
    let n: usize = s.trim().parse().map_err(|e| format!("{e}"))?;
    if n <= tessera_net::sandbox::MAX_CROWD {
        Ok(n)
    } else {
        Err(format!("at most {}", tessera_net::sandbox::MAX_CROWD))
    }
}

fn parse_avatar_arg(s: &str) -> Result<crate::avatar::AvatarSpec, String> {
    crate::avatar::parse_avatar(s).map_err(|e| e.to_string())
}

/// Parses one `--content-layers` key (decimal or `0x` hex).
pub fn parse_layer(s: &str) -> Result<u32, String> {
    let t = s.trim();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u32::from_str_radix(&h.replace('_', ""), 16),
        None => t.replace('_', "").parse::<u32>(),
    }
    .map_err(|e| format!("{t:?}: {e}"))
}

impl Args {
    /// Whether the world walker runs (instead of the M0 showcase).
    #[must_use]
    pub fn walker(&self) -> bool {
        !self.showcase && !self.no_data && !self.smoke
    }

    /// Whether the walker renders offscreen (or, for `--walk-path`, runs without a GPU).
    #[must_use]
    pub fn walker_headless(&self) -> bool {
        self.walker() && (self.headless || self.screenshot.is_some() || self.walk_path.is_some())
    }
}

/// Checks a `--spawn` value with the sandbox's spawn-point grammar.
pub fn parse_spawn(s: &str) -> Result<String, String> {
    let p = tessera_net::sandbox::world::SpawnPoint::parse(s).map_err(|e| e.to_string())?;
    if matches!(p.destination, tessera_net::Destination::Named(_)) {
        return Err("expected R,X,Y[,x,y[,z]] (use --place for names)".into());
    }
    Ok(s.to_owned())
}

fn parse_fly_path_arg(s: &str) -> Result<crate::walker::parse::FlyPath, String> {
    crate::walker::parse::parse_fly_path(s)
}

fn parse_degrees(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (-360.0..=360.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be degrees in -360..=360".into())
    }
}

fn parse_pitch(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (-89.0..=89.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be degrees in -89..=89".into())
    }
}

fn parse_fly_speed(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && v > 0.0 && v <= MAX_FLY_SPEED {
        Ok(v)
    } else {
        Err(format!("must be a number in (0, {MAX_FLY_SPEED}]"))
    }
}

fn parse_positive(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && v > 0.0 && v <= 1.0e4 {
        Ok(v)
    } else {
        Err("must be a number in (0, 10000]".into())
    }
}

fn parse_positive_f64(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && v > 0.0 && v <= 1.0e5 {
        Ok(v)
    } else {
        Err("must be a number in (0, 100000]".into())
    }
}

/// Parses a render-surface DID in hex (`0x41...`) or decimal and checks its type prefix.
pub fn parse_surface_did(s: &str) -> Result<u32, String> {
    let t = s.trim();
    let value = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(&hex.replace('_', ""), 16),
        None => t.replace('_', "").parse::<u32>(),
    }
    .map_err(|e| format!("not a DID: {e}"))?;
    if tessera_formats::Did(value).kind() != DidKind::RenderSurface {
        return Err(format!(
            "{value:#010x} is not a render-surface DID (0x41xxxxxx)"
        ));
    }
    Ok(value)
}

/// Parses `WIDTHxHEIGHT` with both sides in `1..=16384`.
pub fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| "expected WIDTHxHEIGHT".to_owned())?;
    let parse = |v: &str| -> Result<u32, String> {
        let n: u32 = v.trim().parse().map_err(|e| format!("{v:?}: {e}"))?;
        if (1..=16_384).contains(&n) {
            Ok(n)
        } else {
            Err(format!("{n} is outside 1..=16384"))
        }
    };
    Ok((parse(w)?, parse(h)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_did_parsing() {
        assert_eq!(parse_surface_did("0x41000123"), Ok(0x4100_0123));
        assert_eq!(parse_surface_did("0x4100_0123"), Ok(0x4100_0123));
        assert_eq!(
            parse_surface_did(&0x4100_0001u32.to_string()),
            Ok(0x4100_0001)
        );
        assert!(parse_surface_did("0x25000001").is_err());
        assert!(parse_surface_did("zz").is_err());
        assert!(parse_surface_did("0x1_0000_0000").is_err());
    }

    #[test]
    fn size_parsing() {
        assert_eq!(parse_size("640x360"), Ok((640, 360)));
        assert_eq!(parse_size("1X1"), Ok((1, 1)));
        assert!(parse_size("0x10").is_err());
        assert!(parse_size("640").is_err());
        assert!(parse_size("99999x10").is_err());
    }

    #[test]
    fn args_parse() {
        let a = Args::try_parse_from(["t", "--smoke", "--frames", "3", "--surface", "0x41000001"])
            .expect("valid args");
        assert!(a.smoke);
        assert_eq!(a.frames, 3);
        assert_eq!(a.surface, Some(0x4100_0001));
        assert!(Args::try_parse_from(["t", "--smoke-out", "x.ppm"]).is_err());
    }

    #[test]
    fn walker_args_parse() {
        let a = Args::try_parse_from(["t", "--spawn", "1,100,150,40,80"]).expect("spawn");
        assert!(a.walker() && !a.walker_headless());
        assert!(Args::try_parse_from(["t", "--spawn", "1,255,1"]).is_err());
        assert!(Args::try_parse_from(["t", "--spawn", "Stonebridge"]).is_err());
        let a = Args::try_parse_from([
            "t",
            "--fly-path",
            "1:10:100,1:200:100",
            "--speed",
            "800",
            "--headless",
            "--frame-log",
            "f.csv",
        ])
        .expect("fly path");
        assert!(a.walker_headless());
        assert_eq!(a.fly_path.map(|p| p.region), Some(Some(1)));
        assert!(Args::try_parse_from(["t", "--fly-path", "1:2"]).is_err());
        assert!(Args::try_parse_from(["t", "--speed", "-1"]).is_err());
        assert!(Args::try_parse_from(["t", "--place", "x", "--spawn", "1,2,3"]).is_err());
        let a = Args::try_parse_from(["t", "--screenshot", "a.png", "--place", "12.3S, 45.6W"])
            .expect("screenshot");
        assert!(a.walker_headless());
        assert!(!Args::try_parse_from(["t", "--showcase"]).unwrap().walker());
    }

    #[test]
    fn timeouts_are_bounded() {
        let max = u64::MAX.to_string();
        assert!(Args::try_parse_from(["t", "--settle-timeout", &max]).is_err());
        assert!(Args::try_parse_from(["t", "--smoke", "--data-timeout", &max]).is_err());
        assert!(Args::try_parse_from(["t", "--settle-timeout", "0"]).is_err());
        let a = Args::try_parse_from(["t", "--settle-timeout", "86400", "--data-timeout", "0"])
            .expect("in range");
        assert_eq!((a.settle_timeout, a.data_timeout), (86_400, 0));
    }

    #[test]
    fn headless_needs_an_end() {
        assert!(Args::try_parse_from(["t", "--headless"]).is_err());
        assert!(Args::try_parse_from(["t", "--headless", "--screenshot", "a.png"]).is_ok());
        assert!(Args::try_parse_from(["t", "--headless", "--fly-path", "1:2,3:4"]).is_ok());
    }

    #[test]
    fn character_args() {
        let a = Args::try_parse_from(["t"]).unwrap();
        assert_eq!(a.avatar, None);
        assert_eq!((a.crowd, a.content_layers.clone()), (0, vec![1]));
        let a = Args::try_parse_from([
            "t",
            "--avatar",
            "elf,f;0x79006BF8@stowed",
            "--crowd",
            "50",
            "--content-layers",
            "1, 0x10000019",
        ])
        .unwrap();
        assert_eq!(a.avatar.map(|s| s.items.len()), Some(1));
        assert_eq!(a.crowd, 50);
        assert_eq!(a.content_layers, vec![1, 0x1000_0019]);
        assert!(Args::try_parse_from(["t", "--avatar", "x"]).is_err());
        assert!(Args::try_parse_from(["t", "--crowd", "5000"]).is_err());
        assert!(Args::try_parse_from(["t", "--content-layers", "a"]).is_err());
        let a =
            Args::try_parse_from(["t", "--headless", "--duration", "60", "--crowd", "50"]).unwrap();
        assert!(a.walker_headless());
        assert!(Args::try_parse_from(["t", "--duration", "0"]).is_err());
    }

    #[test]
    fn audio_args() {
        use crate::audio::backend::BackendChoice;
        let a = Args::try_parse_from(["t", "--audio", "null", "--audio-prefs", "auto"]).unwrap();
        assert_eq!(a.audio, Some(BackendChoice::Null));
        assert_eq!(a.audio_prefs.as_deref(), Some("auto"));
        assert!(Args::try_parse_from(["t", "--audio", "loud"]).is_err());
        assert_eq!(Args::try_parse_from(["t"]).unwrap().audio, None);
        let a = Args::try_parse_from(["t", "--audio-settings", "default", "--audio-seed", "7"])
            .unwrap();
        assert_eq!(a.audio_settings.as_deref(), Some("default"));
        assert_eq!(a.audio_seed, Some(7));
        let c = crate::audio::config_from_args(&a);
        assert_eq!(c.seed, 7);
        assert_eq!(c.settings, crate::audio::backend::SettingsChoice::Defaults);
        // Headless runs: pinned settings and seed 0; windowed runs: the user's settings.
        let h = crate::audio::config_from_args(
            &Args::try_parse_from(["t", "--headless", "--duration", "1"]).unwrap(),
        );
        assert_eq!(h.seed, 0);
        assert_eq!(h.settings, crate::audio::backend::SettingsChoice::Defaults);
        let w = crate::audio::config_from_args(&Args::try_parse_from(["t"]).unwrap());
        assert_eq!(w.settings, crate::audio::backend::SettingsChoice::User);
        let m = Args::try_parse_from([
            "t",
            "--music-command",
            "/play tune",
            "--command",
            "/playstop",
            "--music-command",
            "/help",
        ])
        .unwrap();
        assert_eq!(m.commands, vec!["/play tune", "/playstop", "/help"]);
    }

    #[test]
    fn server_args() {
        let fp = "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";
        let a = Args::try_parse_from([
            "t",
            "--server",
            "127.0.0.1:47000",
            "--account",
            "alice",
            "--server-fingerprint",
            fp,
            "--character",
            "Aldith",
        ])
        .expect("server args");
        assert_eq!(a.server.as_deref(), Some("tessera://127.0.0.1:47000"));
        assert_eq!(
            a.server_fingerprint.map(|f| f.to_string()).as_deref(),
            Some(fp)
        );
        // The account is required; the other options need --server; no password option.
        assert!(Args::try_parse_from(["t", "--server", "127.0.0.1:1"]).is_err());
        assert!(Args::try_parse_from(["t", "--account", "alice"]).is_err());
        assert!(Args::try_parse_from(["t", "--trust-new-server"]).is_err());
        assert!(
            Args::try_parse_from(["t", "--server", "h:1", "--account", "a", "--password", "x"])
                .is_err()
        );
        assert!(Args::try_parse_from(["t", "--server", "nohost", "--account", "a"]).is_err());
        assert!(
            Args::try_parse_from([
                "t",
                "--server",
                "h:1",
                "--account",
                "a",
                "--server-fingerprint",
                "x"
            ])
            .is_err()
        );
        let pinned = format!("tessera://[2001:db8::1]:47000?pin={fp}");
        let a = Args::try_parse_from(["t", "--server", &pinned, "--account", "a"]).unwrap();
        assert_eq!(a.server.as_deref(), Some(pinned.as_str()));
    }

    #[test]
    fn remote_settings_take_the_password_from_env_or_prompt() {
        let none = Args::try_parse_from(["t"]).unwrap();
        assert!(
            remote_settings(&none, None, |_| unreachable!())
                .unwrap()
                .is_none()
        );
        let a = Args::try_parse_from([
            "t",
            "--server",
            "127.0.0.1:47000",
            "--account",
            "alice",
            "--known-servers",
            "/x/known",
        ])
        .unwrap();
        let s = remote_settings(&a, Some("from env".into()), |_| unreachable!())
            .unwrap()
            .unwrap();
        assert_eq!(s.password.expose(), "from env");
        assert_eq!(
            s.known_servers.as_deref(),
            Some(std::path::Path::new("/x/known"))
        );
        assert!(
            !format!("{s:?}").contains("from env"),
            "password never in Debug"
        );
        let s = remote_settings(&a, Some(String::new()), |acct| {
            assert_eq!(acct, "alice");
            Ok("typed".into())
        })
        .unwrap()
        .unwrap();
        assert_eq!(s.password.expose(), "typed");
        assert!(remote_settings(&a, None, |_| Err(std::io::Error::other("no tty"))).is_err());
        assert!(remote_settings(&a, None, |_| Ok(String::new())).is_err());
        let cfg = s.config().expect("config");
        assert!(cfg.server.pin.is_none());
    }

    #[test]
    fn server_conflicts_with_sandbox_spawn_and_crowd() {
        let base = ["t", "--server", "127.0.0.1:1", "--account", "a"];
        for extra in [
            &["--place", "Bree"][..],
            &["--spawn", "1,100,150"],
            &["--crowd", "50"],
            &["--crowd-spacing", "2"],
            &["--crowd-ahead", "4"],
        ] {
            let mut argv = base.to_vec();
            argv.extend_from_slice(extra);
            assert!(
                Args::try_parse_from(&argv).is_err(),
                "{extra:?} with --server"
            );
        }
        // Walker-side options still apply.
        let mut argv = base.to_vec();
        argv.extend_from_slice(&["--heading", "90", "--avatar", "elf,f"]);
        assert!(Args::try_parse_from(&argv).is_ok());
    }

    #[test]
    fn a_fingerprint_must_agree_with_the_pin() {
        let aaa = "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";
        let bbb = "sha256:16e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";
        let pinned = format!("tessera://127.0.0.1:1?pin={aaa}");
        let parse = |fp: &str| {
            Args::try_parse_from([
                "t",
                "--server",
                &pinned,
                "--account",
                "a",
                "--server-fingerprint",
                fp,
            ])
            .unwrap()
        };
        let err = remote_settings(&parse(bbb), Some("pw".into()), |_| unreachable!())
            .expect_err("conflicting fingerprints");
        assert!(err.to_string().contains("authoritative"), "{err}");
        let same = remote_settings(&parse(aaa), Some("pw".into()), |_| unreachable!())
            .unwrap()
            .unwrap();
        assert_eq!(
            same.config()
                .unwrap()
                .server
                .pin
                .map(|p| p.to_string())
                .as_deref(),
            Some(aaa)
        );
        // The settings refuse the conflict too (callers that build them directly).
        let mut direct = same.clone();
        direct.fingerprint = Some(tessera_net::remote::Fingerprint::parse(bbb).unwrap());
        assert!(direct.config().is_err());
    }

    fn typed(bytes: &[u8]) -> std::io::Result<String> {
        edit_password(bytes.iter().map(|&b| Ok(b)))
    }

    #[test]
    fn the_password_editor_handles_keys_and_cancels() {
        use std::io::ErrorKind;
        assert_eq!(typed(b"hunter2\n").unwrap(), "hunter2");
        assert_eq!(typed(b"hunter2\r").unwrap(), "hunter2");
        assert_eq!(typed(b"huntex\x7f\x7fr2\r").unwrap(), "huntr2");
        assert_eq!(
            typed("pässwörd\x7f\x7f\x7fd\n".as_bytes()).unwrap(),
            "pässwd"
        );
        assert_eq!(typed(b"junk\x15real\n").unwrap(), "real");
        assert_eq!(typed(b"a\x1b[Ab\n").unwrap(), "a[Ab", "controls ignored");
        assert_eq!(typed(b"abc\x04").unwrap(), "abc");
        assert_eq!(typed(b"abc").unwrap(), "abc", "end of input after text");
        // Ctrl-C, Ctrl-\ and Ctrl-Z cancel (signals are off while the prompt is open).
        for cancel in [b"abc\x03".as_slice(), b"\x1c", b"x\x1a\n"] {
            assert_eq!(typed(cancel).unwrap_err().kind(), ErrorKind::Interrupted);
        }
        assert_eq!(typed(b"").unwrap_err().kind(), ErrorKind::UnexpectedEof);
        assert_eq!(typed(b"\x04").unwrap_err().kind(), ErrorKind::UnexpectedEof);
        let long = vec![b'a'; MAX_PASSWORD_BYTES + 1];
        assert_eq!(typed(&long).unwrap_err().kind(), ErrorKind::InvalidData);
        assert_eq!(typed(&long[1..]).unwrap().len(), MAX_PASSWORD_BYTES);
        assert_eq!(typed(b"\xff\n").unwrap_err().kind(), ErrorKind::InvalidData);
        let failing = [Ok(b'a'), Err(std::io::Error::other("tty gone"))];
        assert_eq!(edit_password(failing).unwrap_err().to_string(), "tty gone");
    }

    #[test]
    fn walk_path_args() {
        let a = Args::try_parse_from(["t", "--walk-path", "w.txt", "--walk-log", "w.csv"])
            .expect("walk path");
        assert!(a.walker_headless());
        assert!(Args::try_parse_from(["t", "--headless", "--walk-path", "w.txt"]).is_ok());
        assert!(Args::try_parse_from(["t", "--walk-log", "w.csv"]).is_err());
        assert!(Args::try_parse_from(["t", "--walk-path", "w", "--screenshot", "a.png"]).is_err());
        // The walks need the install: never silently the windowed test scene instead.
        assert!(Args::try_parse_from(["t", "--no-data", "--walk-path", "w"]).is_err());
        // Offline walks: never silently instead of joining a test server.
        assert!(
            Args::try_parse_from(["t", "--server", "h:1", "--account", "a", "--walk-path", "w"])
                .is_err()
        );
    }

    #[test]
    fn world_fidelity_args() {
        use crate::walker::fxparse::System;
        let a = Args::try_parse_from(["t"]).unwrap();
        assert_eq!(
            (a.view_blocks, a.object_distance, a.disable.clone()),
            (None, None, None)
        );
        assert_eq!(a.character_distance, None);
        let a = Args::try_parse_from([
            "t",
            "--disable",
            "water,frills",
            "--day",
            "12240",
            "--time-of-day",
            "0.25",
            "--time-rate",
            "60",
            "--landscape-distance",
            "veryhigh",
            "--object-draw-distance",
            "UltraHigh",
            "--frill-distance",
            "5",
            "--camera-loop",
            "2:119:68:30,30;130,30;130,125;30,125",
            "--duration",
            "60",
            "--headless",
        ])
        .expect("world fidelity options");
        assert_eq!(
            a.disable.map(|d| d.0),
            Some(vec![System::Water, System::Frills])
        );
        assert_eq!(
            (a.day, a.time_of_day, a.time_rate),
            (Some(12_240), Some(0.25), Some(60.0))
        );
        assert!(a.camera_loop.is_some());
        assert!(Args::try_parse_from(["t", "--disable", "bogus"]).is_err());
        assert!(Args::try_parse_from(["t", "--time-of-day", "1.5"]).is_err());
        assert!(Args::try_parse_from(["t", "--day", "-20000000"]).is_ok());
        assert!(Args::try_parse_from(["t", "--day", "1000000000"]).is_err());
        assert!(Args::try_parse_from(["t", "--time-rate", "100000"]).is_ok());
        assert!(Args::try_parse_from(["t", "--time-rate", "100001"]).is_err());
        assert!(Args::try_parse_from(["t", "--landscape-distance", "x"]).is_err());
        assert!(Args::try_parse_from(["t", "--frill-distance", "6"]).is_err());
        assert!(
            Args::try_parse_from(["t", "--turn-rate", "6", "--camera-loop", "1:2:3:1,1;2,2"])
                .is_err()
        );
    }

    #[test]
    fn fly_speed_is_capped() {
        assert!(Args::try_parse_from(["t", "--fly-speed", "2000"]).is_ok());
        assert!(Args::try_parse_from(["t", "--fly-speed", "2001"]).is_err());
    }
}
