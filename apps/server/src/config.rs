// SPDX-License-Identifier: GPL-3.0-or-later
//! The configuration file `server.toml` (`test-server.md` §4).
//!
//! Every key is optional except `[server] listen`, which may also come from `--listen`.
//! Unknown tables or keys, wrong types and values outside their range are `CONFIG` findings
//! that name the key; `run` refuses to start on any of them.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use tessera_protocol::limits::{MAX_LONG_TEXT_LEN, MAX_SOFTWARE_NAME_LEN};
use tessera_protocol::text::has_control;

use crate::tomlx::{self, Finding, Reader, Rule, Source, View};
use crate::words;

/// What a duplicate login does (`[server] duplicate_login`, `tessera-protocol.md` §4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateLogin {
    /// End the old session with `LOGGED_IN_ELSEWHERE` and accept the new one.
    Replace,
    /// Refuse the new login with `ALREADY_ONLINE`.
    Refuse,
}

/// Teleport policy (`[world] teleport`, §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportPolicy {
    /// Every destination kind.
    Any,
    /// Only `Named` destinations.
    Named,
    /// No teleports.
    Off,
}

/// Where Yell reaches (`[chat] yell_scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YellScope {
    /// The whole instance (the protocol default).
    Instance,
    /// Only the speaker's region in the instance.
    Region,
}

/// Log line format (`[log] format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable text.
    Text,
    /// One JSON object per line.
    Json,
}

/// `[server]` (§4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerSection {
    /// UDP addresses to listen on (1–8). Empty means "not configured"; `run` then needs
    /// `--listen`. Port 0 (any free port) is allowed only through the library API.
    pub listen: Vec<SocketAddr>,
    /// `server_name` in `HelloAck`.
    pub name: String,
    /// Replaces the content's message of the day.
    pub motd: Option<String>,
    /// Content file; `None` = the built-in default world. Relative paths are resolved
    /// against the data directory.
    pub content: Option<PathBuf>,
    /// Advertise `REGISTRATION` and accept `Register`.
    pub registration: bool,
    /// Advertise `CHARACTER_CREATION`.
    pub character_creation: bool,
    /// Advertise `APPEARANCE_CHANGE`.
    pub appearance_change: bool,
    /// Duplicate-login behaviour.
    pub duplicate_login: DuplicateLogin,
    /// Shutdown grace, seconds (0–600).
    pub shutdown_grace_s: u32,
}

/// `[accounts]` (§4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountsSection {
    /// Argon2id memory for new hashes, KiB.
    pub argon2_memory_kib: u32,
    /// Argon2id passes.
    pub argon2_iterations: u32,
    /// Argon2id parallelism.
    pub argon2_lanes: u32,
    /// Hash computations that may run at once.
    pub hash_workers: u32,
    /// Characters per account.
    pub max_characters: u32,
    /// Shortest new password, bytes.
    pub password_min_bytes: u32,
    /// Consecutive failures before an account lockout.
    pub lockout_after: u32,
    /// First lockout, seconds.
    pub lockout_base_s: u32,
    /// Longest lockout, seconds.
    pub lockout_max_s: u32,
    /// Per-address login budget per minute.
    pub attempts_per_address_per_min: u32,
    /// Accept `Object` bodies in player appearances.
    pub allow_object_bodies: bool,
}

/// `[world]` (§4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldSection {
    /// World instances (1–16).
    pub instances: u32,
    /// Characters per instance.
    pub instance_capacity: u32,
    /// `MotionBatch` rate, a divisor of 30.
    pub snapshot_rate: u8,
    /// Interest radius *R*.
    pub interest_radius: u8,
    /// Server cap on entities created per connection.
    pub max_entities_cap: u32,
    /// New `EntityCreate`s per connection per tick.
    pub creates_per_tick: u32,
    /// World-stream bytes per connection per tick.
    pub world_bytes_per_tick: u32,
    /// In-world inactivity limit, minutes (0 = never).
    pub idle_timeout_min: u32,
    /// Teleport policy.
    pub teleport: TeleportPolicy,
    /// Allow `@name` destinations.
    pub teleport_to_players: bool,
    /// Least time between two executed teleports of one player, ms.
    pub teleport_interval_ms: u32,
    /// Periodic save interval, seconds.
    pub save_interval_s: u32,
}

/// `[movement]` (§4.5).
///
/// The validation defaults follow the character controller (`collision.md` §12), not the
/// walk and run speeds, which only drive gait and animation: *V*ₕ = 8.5 m/s (fastest
/// controlled speed 6.5 m/s + 30 %), *V*ᵤ = 10 m/s, *V*_d = 120 m/s (the controller's speed
/// cap). `slide_reserve` and `settle_grace_s` use the two relaxations that
/// `tessera-protocol.md` §8.3 permits ("Server relaxations": descent credit *S* ≤ 100 m,
/// settling window ≤ 10 s): a descent credit with *S* = 58 m so slides down steep faces pass,
/// and a 3 s settling window after ground spawns and `Ground` teleports (§8.3).
#[derive(Debug, Clone, PartialEq)]
pub struct MovementSection {
    /// `WALK_SPEED` sent to each player, m/s; gait and animation only.
    pub walk_speed: f32,
    /// `RUN_SPEED` sent to each player, m/s; gait and animation only.
    pub run_speed: f32,
    /// *V*ₕ, m/s (6.5–120, default 8.5).
    pub max_horizontal_speed: f64,
    /// *V*ᵤ, m/s (6.5–120, default 10).
    pub max_up_speed: f64,
    /// *V*_d, m/s (50–200, default 120).
    pub max_down_speed: f64,
    /// *S*, the most descent credit kept for a slide's run-out, metres (0–100, default 58,
    /// §8.3); 0 turns the credit off.
    pub slide_reserve: f64,
    /// *W*, seconds.
    pub window_s: f64,
    /// Settling grace after ground spawns and `Ground` teleports, seconds (0–10, default 3).
    pub settle_grace_s: f64,
}

/// `[chat]` (§4.6). The protocol fixes the chat rate limit and flood rule
/// (`tessera-protocol.md` §10.2), so the bucket and flood keys may only make them stricter:
/// their ranges stop at the protocol's values, and the defaults equal them.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSection {
    /// Say range, metres.
    pub say_range_m: f64,
    /// Emote range, metres.
    pub emote_range_m: f64,
    /// Yell scope.
    pub yell_scope: YellScope,
    /// Chat token bucket capacity (1–5).
    pub bucket_capacity: u32,
    /// Seconds per token (2.0–60).
    pub refill_s: f64,
    /// Rejections within the window that end the session with `FLOODING` (5–20).
    pub flood_rejections: u32,
    /// Flood window, seconds (60–3,600).
    pub flood_window_s: u32,
    /// Log chat lines (debugging only); with it, channel names appear in the channel events.
    pub log_chat: bool,
    /// Named chat channels (§9.1): advertise `CHAT_CHANNELS` to connections at minor ≥ 2 and
    /// run the channel registry.
    pub channels: bool,
    /// Channels that may exist at once on the server (1–4,096).
    pub max_channels: u32,
    /// Channels one character may be a member of at once (1–32).
    pub max_channels_per_character: u32,
    /// Members of one channel (2–4,096).
    pub max_channel_members: u32,
}

/// `[limits]` (§4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct LimitsSection {
    /// Connections past the QUIC handshake.
    pub max_connections: u32,
    /// Connections per source IP (loopback exempt).
    pub max_connections_per_address: u32,
    /// QUIC handshakes in progress.
    pub max_handshakes: u32,
    /// Inbound control messages queued per connection.
    pub control_queue: u32,
    /// Accounted memory budget, MiB (83–65,536; at least `argon2_memory_kib` ×
    /// `hash_workers` + 64 MiB).
    pub memory_budget_mib: u32,
}

/// `[log]` (§4.8).
#[derive(Debug, Clone, PartialEq)]
pub struct LogSection {
    /// Default filter level.
    pub level: String,
    /// Line format.
    pub format: LogFormat,
    /// Append to this file instead of standard error.
    pub file: Option<PathBuf>,
    /// Statistics line interval, seconds (0 = off).
    pub stats_interval_s: u32,
}

/// The whole configuration with defaults (§4).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `[server]`.
    pub server: ServerSection,
    /// `[accounts]`.
    pub accounts: AccountsSection,
    /// `[world]`.
    pub world: WorldSection,
    /// `[movement]`.
    pub movement: MovementSection,
    /// `[chat]`.
    pub chat: ChatSection,
    /// `[limits]`.
    pub limits: LimitsSection,
    /// `[log]`.
    pub log: LogSection,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerSection {
                listen: Vec::new(),
                name: "Tessera test server".to_owned(),
                motd: None,
                content: None,
                registration: false,
                character_creation: true,
                appearance_change: true,
                duplicate_login: DuplicateLogin::Replace,
                shutdown_grace_s: 10,
            },
            accounts: AccountsSection {
                argon2_memory_kib: 65_536,
                argon2_iterations: 3,
                argon2_lanes: 4,
                hash_workers: 2,
                max_characters: 8,
                password_min_bytes: 8,
                lockout_after: 5,
                lockout_base_s: 30,
                lockout_max_s: 900,
                attempts_per_address_per_min: 20,
                allow_object_bodies: false,
            },
            world: WorldSection {
                instances: 1,
                instance_capacity: 64,
                snapshot_rate: 15,
                interest_radius: 2,
                max_entities_cap: 1_024,
                creates_per_tick: 16,
                world_bytes_per_tick: 65_536,
                idle_timeout_min: 30,
                teleport: TeleportPolicy::Any,
                teleport_to_players: true,
                teleport_interval_ms: 500,
                save_interval_s: 60,
            },
            movement: MovementSection {
                walk_speed: 2.0,
                run_speed: 6.0,
                max_horizontal_speed: 8.5,
                max_up_speed: 10.0,
                max_down_speed: 120.0,
                slide_reserve: 58.0,
                window_s: 1.0,
                settle_grace_s: 3.0,
            },
            chat: ChatSection {
                say_range_m: 40.0,
                emote_range_m: 40.0,
                yell_scope: YellScope::Instance,
                bucket_capacity: 5,
                refill_s: 2.0,
                flood_rejections: 20,
                flood_window_s: 60,
                log_chat: false,
                channels: true,
                max_channels: 256,
                max_channels_per_character: 10,
                max_channel_members: 256,
            },
            limits: LimitsSection {
                max_connections: 64,
                max_connections_per_address: 8,
                max_handshakes: 32,
                control_queue: 64,
                memory_budget_mib: 1_024,
            },
            log: LogSection {
                level: "info".to_owned(),
                format: LogFormat::Text,
                file: None,
                stats_interval_s: 60,
            },
        }
    }
}

/// Lowest accepted Argon2id memory, KiB (§4.3; the OWASP floor).
pub const ARGON2_MIN_MEMORY_KIB: u32 = 19_456;
/// Lowest accepted Argon2id passes.
pub const ARGON2_MIN_ITERATIONS: u32 = 2;

impl Config {
    /// The fastest Argon2id parameters the configuration accepts (m = 19 MiB, t = 2, p = 1):
    /// for load tests and the in-process test servers (`test-server.md` §13.1).
    pub fn use_hash_floor(&mut self) {
        self.accounts.argon2_memory_kib = ARGON2_MIN_MEMORY_KIB;
        self.accounts.argon2_iterations = ARGON2_MIN_ITERATIONS;
        self.accounts.argon2_lanes = 1;
    }

    /// Parses a configuration text. Returns the configuration (defaults where a key is
    /// missing or invalid) and every finding; the configuration is usable only when there
    /// are none.
    #[must_use]
    pub fn parse(text: &str) -> (Self, Vec<Finding>) {
        let src = Source::new(text);
        let mut cfg = Self::default();
        let doc = match tomlx::parse(&src) {
            Ok(d) => d,
            Err(mut f) => {
                f.rule = Rule::Config;
                return (cfg, vec![f]);
            }
        };
        let mut r = Reader::new(&src, Rule::Config, Rule::Config);
        let root = View {
            table: doc.get_ref(),
            line: 1,
            path: "",
        };
        r.check_keys(
            root,
            &[
                "server", "accounts", "world", "movement", "chat", "limits", "log",
            ],
        );
        if let Some((t, v)) = r.table(root, "server", false) {
            let line = r.line(&v.span());
            parse_server(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "server",
                },
                &mut cfg.server,
            );
        }
        if let Some((t, v)) = r.table(root, "accounts", false) {
            let line = r.line(&v.span());
            parse_accounts(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "accounts",
                },
                &mut cfg.accounts,
            );
        }
        if let Some((t, v)) = r.table(root, "world", false) {
            let line = r.line(&v.span());
            parse_world(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "world",
                },
                &mut cfg.world,
            );
        }
        if let Some((t, v)) = r.table(root, "movement", false) {
            let line = r.line(&v.span());
            parse_movement(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "movement",
                },
                &mut cfg.movement,
            );
        }
        if let Some((t, v)) = r.table(root, "chat", false) {
            let line = r.line(&v.span());
            parse_chat(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "chat",
                },
                &mut cfg.chat,
            );
        }
        if let Some((t, v)) = r.table(root, "limits", false) {
            let line = r.line(&v.span());
            parse_limits(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "limits",
                },
                &mut cfg.limits,
            );
        }
        if let Some((t, v)) = r.table(root, "log", false) {
            let line = r.line(&v.span());
            parse_log(
                &mut r,
                View {
                    table: t,
                    line,
                    path: "log",
                },
                &mut cfg.log,
            );
        }
        let mut findings = r.findings;
        findings.extend(cfg.cross_checks());
        findings.sort();
        (cfg, findings)
    }

    /// Reads the configuration file. `explicit` is true when the file was named with
    /// `--config`: a missing explicit file is an error, a missing default file means all
    /// defaults (§2.1).
    pub fn load(path: &Path, explicit: bool) -> Result<(Self, Vec<Finding>), std::io::Error> {
        match tomlx::read_text(path) {
            Ok(Ok(text)) => Ok(Self::parse(&text)),
            Ok(Err(mut f)) => {
                f.rule = Rule::Config;
                Ok((Self::default(), vec![f]))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => {
                Ok((Self::default(), Vec::new()))
            }
            Err(e) => Err(e),
        }
    }

    /// Checks that span several keys (whole-file findings, line 0).
    fn cross_checks(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        let a = &self.accounts;
        let need = u64::from(a.argon2_memory_kib) * u64::from(a.hash_workers) + 64 * 1024;
        if u64::from(self.limits.memory_budget_mib) * 1024 < need {
            out.push(Finding::new(
                0,
                Rule::Config,
                format!(
                    "`limits.memory_budget_mib` must be at least argon2_memory_kib × hash_workers + 64 MiB ({} MiB)",
                    need.div_ceil(1024)
                ),
            ));
        }
        if a.lockout_max_s < a.lockout_base_s {
            out.push(Finding::new(
                0,
                Rule::Config,
                "`accounts.lockout_max_s` must not be below `accounts.lockout_base_s`",
            ));
        }
        if self.movement.walk_speed > self.movement.run_speed {
            out.push(Finding::new(
                0,
                Rule::Config,
                "`movement.walk_speed` must not exceed `movement.run_speed`",
            ));
        }
        out
    }

    /// The address guard (§4.2): with `registration = true` every listen address must be
    /// private. Returns the offending addresses.
    #[must_use]
    pub fn registration_violations(&self) -> Vec<SocketAddr> {
        if !self.server.registration {
            return Vec::new();
        }
        self.server
            .listen
            .iter()
            .copied()
            .filter(|a| !is_private(a.ip()))
            .collect()
    }
}

/// Whether an address is private (§4.2): loopback, RFC 1918, RFC 6598, link-local, or an
/// IPv6 unique local address. Unspecified addresses are not private.
#[must_use]
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || o[0] == 10
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 169 && o[1] == 254)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private(IpAddr::V4(v4));
            }
            let s = v6.segments();
            v6.is_loopback() || (s[0] & 0xfe00) == 0xfc00 || (s[0] & 0xffc0) == 0xfe80
        }
    }
}

fn u32_key(r: &mut Reader<'_, '_>, v: View<'_, '_>, key: &str, min: i64, max: i64, out: &mut u32) {
    if let Some((i, _)) = r.int(v, key, false, min, max) {
        *out = u32::try_from(i).unwrap_or(*out);
    }
}

fn bool_key(r: &mut Reader<'_, '_>, v: View<'_, '_>, key: &str, out: &mut bool) {
    if let Some((b, _)) = r.bool(v, key, false) {
        *out = b;
    }
}

fn float_key(
    r: &mut Reader<'_, '_>,
    v: View<'_, '_>,
    key: &str,
    check: impl Fn(f64) -> bool,
    describe: &str,
    out: &mut f64,
) {
    if let Some((f, _)) = r.float_in(v, key, false, check, describe) {
        *out = f;
    }
}

fn choice<T: Copy>(
    r: &mut Reader<'_, '_>,
    v: View<'_, '_>,
    key: &str,
    options: &[(&str, T)],
    out: &mut T,
) {
    if let Some((s, line)) = r.string(v, key, false) {
        match options.iter().find(|(n, _)| *n == s) {
            Some((_, t)) => *out = *t,
            None => {
                let names: Vec<&str> = options.iter().map(|(n, _)| *n).collect();
                r.push(
                    line,
                    Rule::Config,
                    format!("`{}.{key}` must be one of {}", v.path, names.join(", ")),
                );
            }
        }
    }
}

/// Checks a `TEXT0`(max) text of the configuration and runs the word check on it.
fn config_text(r: &mut Reader<'_, '_>, path: &str, text: &str, max: usize, line: usize) -> bool {
    let mut ok = true;
    if text.len() > max || has_control(text) {
        r.push(
            line,
            Rule::Config,
            format!("`{path}` must be at most {max} bytes without control characters"),
        );
        ok = false;
    }
    if let Some(w) = words::find_rule_one(text) {
        r.push(
            line,
            Rule::Word,
            format!("`{path}` contains \"{w}\" (CLAUDE.md Naming rule)"),
        );
        ok = false;
    }
    ok
}

fn parse_server(r: &mut Reader<'_, '_>, v: View<'_, '_>, s: &mut ServerSection) {
    r.check_keys(
        v,
        &[
            "listen",
            "name",
            "motd",
            "content",
            "registration",
            "character_creation",
            "appearance_change",
            "duplicate_login",
            "shutdown_grace_s",
        ],
    );
    if let Some((arr, line)) = r.array(v, "listen", false) {
        if arr.is_empty() || arr.len() > 8 {
            r.push(
                line,
                Rule::Config,
                "`server.listen` must hold 1–8 addresses",
            );
        }
        for item in arr.iter().take(8) {
            let line = r.line(&item.span());
            match item.get_ref() {
                toml::de::DeValue::String(text) => match parse_listen(text) {
                    Some(a) => s.listen.push(a),
                    None => r.push(
                        line,
                        Rule::Config,
                        format!(
                            "`server.listen`: \"{text}\" is not an address `host:port` with an IP address and a port 1–65535"
                        ),
                    ),
                },
                other => r.push(
                    line,
                    Rule::Config,
                    format!("`server.listen` must hold strings, not {}", tomlx::kind(other)),
                ),
            }
        }
    }
    if let Some((name, line)) = r.string(v, "name", false)
        && config_text(r, "server.name", &name, MAX_SOFTWARE_NAME_LEN, line)
    {
        s.name = name;
    }
    if let Some((motd, line)) = r.string(v, "motd", false)
        && config_text(r, "server.motd", &motd, MAX_LONG_TEXT_LEN, line)
    {
        s.motd = Some(motd);
    }
    if let Some((p, line)) = r.string(v, "content", false) {
        if p.is_empty() {
            r.push(line, Rule::Config, "`server.content` must not be empty");
        } else {
            s.content = Some(PathBuf::from(p));
        }
    }
    bool_key(r, v, "registration", &mut s.registration);
    bool_key(r, v, "character_creation", &mut s.character_creation);
    bool_key(r, v, "appearance_change", &mut s.appearance_change);
    choice(
        r,
        v,
        "duplicate_login",
        &[
            ("replace", DuplicateLogin::Replace),
            ("refuse", DuplicateLogin::Refuse),
        ],
        &mut s.duplicate_login,
    );
    u32_key(r, v, "shutdown_grace_s", 0, 600, &mut s.shutdown_grace_s);
}

/// Parses a listen address `host:port` (IPv6 in brackets), port 1–65535.
#[must_use]
pub fn parse_listen(text: &str) -> Option<SocketAddr> {
    let a: SocketAddr = text.trim().parse().ok()?;
    (a.port() != 0).then_some(a)
}

fn parse_accounts(r: &mut Reader<'_, '_>, v: View<'_, '_>, a: &mut AccountsSection) {
    r.check_keys(
        v,
        &[
            "argon2_memory_kib",
            "argon2_iterations",
            "argon2_lanes",
            "hash_workers",
            "max_characters",
            "password_min_bytes",
            "lockout_after",
            "lockout_base_s",
            "lockout_max_s",
            "attempts_per_address_per_min",
            "allow_object_bodies",
        ],
    );
    u32_key(
        r,
        v,
        "argon2_memory_kib",
        19_456,
        1_048_576,
        &mut a.argon2_memory_kib,
    );
    u32_key(r, v, "argon2_iterations", 2, 10, &mut a.argon2_iterations);
    u32_key(r, v, "argon2_lanes", 1, 8, &mut a.argon2_lanes);
    u32_key(r, v, "hash_workers", 1, 8, &mut a.hash_workers);
    u32_key(r, v, "max_characters", 1, 64, &mut a.max_characters);
    u32_key(
        r,
        v,
        "password_min_bytes",
        1,
        256,
        &mut a.password_min_bytes,
    );
    u32_key(r, v, "lockout_after", 1, 100, &mut a.lockout_after);
    u32_key(r, v, "lockout_base_s", 1, 3_600, &mut a.lockout_base_s);
    u32_key(r, v, "lockout_max_s", 1, 86_400, &mut a.lockout_max_s);
    u32_key(
        r,
        v,
        "attempts_per_address_per_min",
        1,
        10_000,
        &mut a.attempts_per_address_per_min,
    );
    bool_key(r, v, "allow_object_bodies", &mut a.allow_object_bodies);
}

fn parse_world(r: &mut Reader<'_, '_>, v: View<'_, '_>, w: &mut WorldSection) {
    r.check_keys(
        v,
        &[
            "instances",
            "instance_capacity",
            "snapshot_rate",
            "interest_radius",
            "max_entities_cap",
            "creates_per_tick",
            "world_bytes_per_tick",
            "idle_timeout_min",
            "teleport",
            "teleport_to_players",
            "teleport_interval_ms",
            "save_interval_s",
        ],
    );
    u32_key(r, v, "instances", 1, 16, &mut w.instances);
    u32_key(
        r,
        v,
        "instance_capacity",
        1,
        1_024,
        &mut w.instance_capacity,
    );
    if let Some((rate, line)) = r.int(v, "snapshot_rate", false, 1, 30) {
        if [1, 2, 3, 5, 6, 10, 15, 30].contains(&rate) {
            w.snapshot_rate = u8::try_from(rate).unwrap_or(15);
        } else {
            r.push(
                line,
                Rule::Config,
                "`world.snapshot_rate` must be a divisor of 30 (1, 2, 3, 5, 6, 10, 15 or 30)",
            );
        }
    }
    if let Some((i, _)) = r.int(v, "interest_radius", false, 0, 8) {
        w.interest_radius = u8::try_from(i).unwrap_or(2);
    }
    u32_key(r, v, "max_entities_cap", 64, 4_096, &mut w.max_entities_cap);
    u32_key(r, v, "creates_per_tick", 1, 256, &mut w.creates_per_tick);
    u32_key(
        r,
        v,
        "world_bytes_per_tick",
        4_096,
        1_048_576,
        &mut w.world_bytes_per_tick,
    );
    u32_key(r, v, "idle_timeout_min", 0, 1_440, &mut w.idle_timeout_min);
    choice(
        r,
        v,
        "teleport",
        &[
            ("any", TeleportPolicy::Any),
            ("named", TeleportPolicy::Named),
            ("off", TeleportPolicy::Off),
        ],
        &mut w.teleport,
    );
    bool_key(r, v, "teleport_to_players", &mut w.teleport_to_players);
    u32_key(
        r,
        v,
        "teleport_interval_ms",
        0,
        10_000,
        &mut w.teleport_interval_ms,
    );
    u32_key(r, v, "save_interval_s", 10, 3_600, &mut w.save_interval_s);
}

fn parse_movement(r: &mut Reader<'_, '_>, v: View<'_, '_>, m: &mut MovementSection) {
    r.check_keys(
        v,
        &[
            "walk_speed",
            "run_speed",
            "max_horizontal_speed",
            "max_up_speed",
            "max_down_speed",
            "slide_reserve",
            "window_s",
            "settle_grace_s",
        ],
    );
    let mut walk = f64::from(m.walk_speed);
    float_key(
        r,
        v,
        "walk_speed",
        |f| f > 0.0 && f <= 100.0,
        "(0, 100]",
        &mut walk,
    );
    m.walk_speed = walk as f32;
    let mut run = f64::from(m.run_speed);
    float_key(
        r,
        v,
        "run_speed",
        |f| f > 0.0 && f <= 100.0,
        "(0, 100]",
        &mut run,
    );
    m.run_speed = run as f32;
    float_key(
        r,
        v,
        "max_horizontal_speed",
        |f| (6.5..=120.0).contains(&f),
        "6.5–120",
        &mut m.max_horizontal_speed,
    );
    float_key(
        r,
        v,
        "max_up_speed",
        |f| (6.5..=120.0).contains(&f),
        "6.5–120",
        &mut m.max_up_speed,
    );
    float_key(
        r,
        v,
        "max_down_speed",
        |f| (50.0..=200.0).contains(&f),
        "50–200",
        &mut m.max_down_speed,
    );
    float_key(
        r,
        v,
        "slide_reserve",
        |f| (0.0..=100.0).contains(&f),
        "0–100",
        &mut m.slide_reserve,
    );
    float_key(
        r,
        v,
        "window_s",
        |f| (0.25..=5.0).contains(&f),
        "0.25–5.0",
        &mut m.window_s,
    );
    float_key(
        r,
        v,
        "settle_grace_s",
        |f| (0.0..=10.0).contains(&f),
        "0–10",
        &mut m.settle_grace_s,
    );
}

fn parse_chat(r: &mut Reader<'_, '_>, v: View<'_, '_>, c: &mut ChatSection) {
    r.check_keys(
        v,
        &[
            "say_range_m",
            "emote_range_m",
            "yell_scope",
            "bucket_capacity",
            "refill_s",
            "flood_rejections",
            "flood_window_s",
            "log_chat",
            "channels",
            "max_channels",
            "max_channels_per_character",
            "max_channel_members",
        ],
    );
    float_key(
        r,
        v,
        "say_range_m",
        |f| (1.0..=1_000.0).contains(&f),
        "1–1000",
        &mut c.say_range_m,
    );
    float_key(
        r,
        v,
        "emote_range_m",
        |f| (1.0..=1_000.0).contains(&f),
        "1–1000",
        &mut c.emote_range_m,
    );
    choice(
        r,
        v,
        "yell_scope",
        &[
            ("instance", YellScope::Instance),
            ("region", YellScope::Region),
        ],
        &mut c.yell_scope,
    );
    // The protocol's fixed limits (§10.2 there: 5 tokens, one per 2 s, 20 rejections in
    // 60 s) bound these ranges, so a configured value is never looser (§4.6).
    u32_key(r, v, "bucket_capacity", 1, 5, &mut c.bucket_capacity);
    float_key(
        r,
        v,
        "refill_s",
        |f| (2.0..=60.0).contains(&f),
        "2.0–60",
        &mut c.refill_s,
    );
    u32_key(r, v, "flood_rejections", 5, 20, &mut c.flood_rejections);
    u32_key(r, v, "flood_window_s", 60, 3_600, &mut c.flood_window_s);
    bool_key(r, v, "log_chat", &mut c.log_chat);
    // Named channels (§4.6, §9.1; decision 0035).
    bool_key(r, v, "channels", &mut c.channels);
    u32_key(r, v, "max_channels", 1, 4_096, &mut c.max_channels);
    u32_key(
        r,
        v,
        "max_channels_per_character",
        1,
        32,
        &mut c.max_channels_per_character,
    );
    u32_key(
        r,
        v,
        "max_channel_members",
        2,
        4_096,
        &mut c.max_channel_members,
    );
}

fn parse_limits(r: &mut Reader<'_, '_>, v: View<'_, '_>, l: &mut LimitsSection) {
    r.check_keys(
        v,
        &[
            "max_connections",
            "max_connections_per_address",
            "max_handshakes",
            "control_queue",
            "memory_budget_mib",
        ],
    );
    u32_key(r, v, "max_connections", 1, 4_096, &mut l.max_connections);
    u32_key(
        r,
        v,
        "max_connections_per_address",
        1,
        4_096,
        &mut l.max_connections_per_address,
    );
    u32_key(r, v, "max_handshakes", 1, 1_024, &mut l.max_handshakes);
    u32_key(r, v, "control_queue", 16, 1_024, &mut l.control_queue);
    u32_key(
        r,
        v,
        "memory_budget_mib",
        83,
        65_536,
        &mut l.memory_budget_mib,
    );
}

fn parse_log(r: &mut Reader<'_, '_>, v: View<'_, '_>, l: &mut LogSection) {
    r.check_keys(v, &["level", "format", "file", "stats_interval_s"]);
    if let Some((level, line)) = r.string(v, "level", false) {
        if ["error", "warn", "info", "debug", "trace"].contains(&level.as_str()) {
            l.level = level;
        } else {
            r.push(
                line,
                Rule::Config,
                "`log.level` must be one of error, warn, info, debug, trace",
            );
        }
    }
    choice(
        r,
        v,
        "format",
        &[("text", LogFormat::Text), ("json", LogFormat::Json)],
        &mut l.format,
    );
    if let Some((p, line)) = r.string(v, "file", false) {
        if p.is_empty() {
            r.push(line, Rule::Config, "`log.file` must not be empty");
        } else {
            l.file = Some(PathBuf::from(p));
        }
    }
    u32_key(r, v, "stats_interval_s", 0, 3_600, &mut l.stats_interval_s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_findings() {
        let (c, f) = Config::parse("");
        assert!(f.is_empty());
        assert_eq!(c, Config::default());

        let (c, f) = Config::parse(
            "[server]\nlisten = [\"127.0.0.1:47000\", \"[::1]:47001\"]\nregistration = true\n\n[world]\nsnapshot_rate = 10\n",
        );
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(c.server.listen.len(), 2);
        assert_eq!(c.world.snapshot_rate, 10);
        assert!(c.registration_violations().is_empty());

        let (_, f) = Config::parse(
            "[server]\nlisten = [\"0.0.0.0:0\"]\nbogus = 1\nname = \"The Shire\"\n[world]\nsnapshot_rate = 7\ninstances = 99\n[nope]\n",
        );
        let rules: Vec<_> = f.iter().map(|f| (f.line, f.rule)).collect();
        assert!(rules.contains(&(2, Rule::Config)), "{f:?}");
        assert!(rules.contains(&(3, Rule::Config)));
        assert!(rules.contains(&(4, Rule::Word)));
        assert!(rules.contains(&(6, Rule::Config)));
        assert!(rules.contains(&(7, Rule::Config)));
        assert!(rules.contains(&(8, Rule::Config)));

        let (_, f) = Config::parse("not toml = = 1");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, Rule::Config);

        let (_, f) = Config::parse("[accounts]\nhash_workers = 8\nargon2_memory_kib = 1048576\n");
        assert_eq!(f.len(), 1, "{f:?}");
    }

    #[test]
    fn movement_defaults_and_ranges() {
        // §4.5: the defaults follow the character controller, not the walk and run speeds.
        let m = Config::default().movement;
        assert_eq!((m.walk_speed, m.run_speed), (2.0, 6.0));
        assert_eq!(m.max_horizontal_speed, 8.5);
        assert_eq!(m.max_up_speed, 10.0);
        assert_eq!(m.max_down_speed, 120.0);
        assert_eq!(m.slide_reserve, 58.0);
        assert_eq!((m.window_s, m.settle_grace_s), (1.0, 3.0));

        let (c, f) = Config::parse(
            "[movement]
max_horizontal_speed = 120.0
max_up_speed = 6.5
max_down_speed = 50.0
slide_reserve = 0.0
",
        );
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(c.movement.max_horizontal_speed, 120.0);
        assert_eq!(c.movement.max_up_speed, 6.5);
        assert_eq!(c.movement.max_down_speed, 50.0);
        assert_eq!(c.movement.slide_reserve, 0.0);

        // Out of range, and the retired `tolerance` key, are findings on their lines.
        let (c, f) = Config::parse(
            "[movement]
max_horizontal_speed = 6.4
max_up_speed = 121.0
max_down_speed = 49.0
slide_reserve = 100.5
tolerance = 1.25
",
        );
        let lines: Vec<_> = f.iter().map(|f| (f.line, f.rule)).collect();
        for line in 2..=6 {
            assert!(lines.contains(&(line, Rule::Config)), "line {line}: {f:?}");
        }
        assert_eq!(c.movement, Config::default().movement);
    }

    /// §4.1 and §8.3: `[movement]` values outside their range, including the relaxations
    /// beyond the protocol's bounds (`tessera-protocol.md` §8.3 "Bounds of the values in
    /// force"), are refused, never clamped; the range ends are accepted as given.
    #[test]
    fn movement_out_of_range_is_refused_not_clamped() {
        for (key, bad) in [
            ("settle_grace_s", "10.001"),
            ("settle_grace_s", "60.0"),
            ("settle_grace_s", "-1.0"),
            ("settle_grace_s", "nan"),
            ("settle_grace_s", "inf"),
            ("slide_reserve", "100.001"),
            ("slide_reserve", "-0.5"),
            ("slide_reserve", "nan"),
            ("slide_reserve", "-inf"),
            ("window_s", "0.2"),
            ("window_s", "5.5"),
            ("max_down_speed", "201.0"),
            ("max_horizontal_speed", "nan"),
        ] {
            let (c, f) = Config::parse(&format!("[movement]\n{key} = {bad}\n"));
            assert!(
                f.iter()
                    .any(|f| f.line == 2 && f.rule == Rule::Config && f.message.contains(key)),
                "{key} = {bad}: {f:?}"
            );
            assert_eq!(c.movement, Config::default().movement, "{key} = {bad}");
        }
        let (c, f) = Config::parse("[movement]\nsettle_grace_s = 10.0\nslide_reserve = 100.0\n");
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(
            (c.movement.settle_grace_s, c.movement.slide_reserve),
            (10.0, 100.0)
        );
        let (c, f) = Config::parse("[movement]\nsettle_grace_s = 0.0\nslide_reserve = 0.0\n");
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(
            (c.movement.settle_grace_s, c.movement.slide_reserve),
            (0.0, 0.0)
        );
    }

    #[test]
    fn chat_and_memory_ranges() {
        // §4.6: the chat keys stop at the protocol's limits (they may only tighten them).
        let c = Config::default().chat;
        assert_eq!(
            (
                c.bucket_capacity,
                c.refill_s,
                c.flood_rejections,
                c.flood_window_s
            ),
            (5, 2.0, 20, 60)
        );
        let (c, f) = Config::parse(
            "[chat]\nbucket_capacity = 1\nrefill_s = 60.0\nflood_rejections = 5\nflood_window_s = 3600\n",
        );
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(
            (
                c.chat.bucket_capacity,
                c.chat.refill_s,
                c.chat.flood_rejections,
                c.chat.flood_window_s
            ),
            (1, 60.0, 5, 3_600)
        );
        let (c, f) = Config::parse(
            "[chat]\nbucket_capacity = 6\nrefill_s = 1.9\nflood_rejections = 21\nflood_window_s = 59\n",
        );
        let lines: Vec<_> = f.iter().map(|f| (f.line, f.rule)).collect();
        for line in 2..=5 {
            assert!(lines.contains(&(line, Rule::Config)), "line {line}: {f:?}");
        }
        assert_eq!(c.chat, Config::default().chat);
        // §4.6 channel keys (decision 0035): defaults, the ends of their ranges, outside.
        let c = Config::default().chat;
        assert_eq!(
            (
                c.channels,
                c.max_channels,
                c.max_channels_per_character,
                c.max_channel_members
            ),
            (true, 256, 10, 256)
        );
        let (c, f) = Config::parse(
            "[chat]\nchannels = false\nmax_channels = 4096\nmax_channels_per_character = 1\nmax_channel_members = 2\n",
        );
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(
            (
                c.chat.channels,
                c.chat.max_channels,
                c.chat.max_channels_per_character,
                c.chat.max_channel_members
            ),
            (false, 4_096, 1, 2)
        );
        let (c, f) = Config::parse(
            "[chat]\nmax_channels = 0\nmax_channels_per_character = 33\nmax_channel_members = 1\n",
        );
        let lines: Vec<_> = f.iter().map(|f| (f.line, f.rule)).collect();
        for line in 2..=4 {
            assert!(lines.contains(&(line, Rule::Config)), "line {line}: {f:?}");
        }
        assert_eq!(c.chat, Config::default().chat);
        // §4.7: 83 MiB is the lowest budget (it meets the constraint at the Argon2id floor).
        let (c, f) = Config::parse(
            "[accounts]\nargon2_memory_kib = 19456\nhash_workers = 1\n[limits]\nmemory_budget_mib = 83\n",
        );
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(c.limits.memory_budget_mib, 83);
        let (_, f) = Config::parse(
            "[accounts]\nargon2_memory_kib = 19456\nhash_workers = 1\n[limits]\nmemory_budget_mib = 82\n",
        );
        assert!(
            f.iter().any(|f| f.line == 5 && f.rule == Rule::Config),
            "{f:?}"
        );
    }

    #[test]
    fn private_addresses() {
        for a in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.9.9",
            "192.168.1.1",
            "100.64.0.1",
            "169.254.1.1",
            "::1",
            "fd00::1",
            "fe80::1",
        ] {
            assert!(is_private(a.parse().unwrap()), "{a}");
        }
        for a in [
            "0.0.0.0",
            "::",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "2001:db8::1",
        ] {
            assert!(!is_private(a.parse().unwrap()), "{a}");
        }
        let mut c = Config::default();
        c.server.registration = true;
        c.server.listen = vec!["0.0.0.0:5000".parse().unwrap()];
        assert_eq!(c.registration_violations().len(), 1);
    }
}
