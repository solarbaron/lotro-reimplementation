// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-client` binary entry point.

use std::process::ExitCode;

use clap::Parser as _;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::{FilterExt as _, filter_fn};
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{Layer as _, Registry};

/// The filter when `RUST_LOG` is unset or does not parse.
const DEFAULT_FILTER: &str = "info,wgpu_core=warn,wgpu_hal=warn,naga=warn,winit=warn";

/// The HTTP and TLS crates of the official login (`tessera-compat`'s `ureq` on `rustls`
/// with the platform certificate verifier, and the crates under them), turned fully off,
/// errors included, whatever `RUST_LOG` says: their records (through the `log` bridge too)
/// can name the service's host, an address, a path or a request line, which the official
/// login never logs (`official-entry.md` §7.3; decision 0044 item 3). The login logs its own
/// neutral line instead where a TLS check fails (`tessera_compat::connector::login_phase`).
/// Engineering choice, spec silent.
const QUIET_TARGETS: &[(&str, LevelFilter)] = &[
    ("ureq", LevelFilter::OFF),
    ("ureq_proto", LevelFilter::OFF),
    ("rustls", LevelFilter::OFF),
    ("rustls_platform_verifier", LevelFilter::OFF),
    ("rustls_native_certs", LevelFilter::OFF),
    ("rustls_pki_types", LevelFilter::OFF),
    ("webpki", LevelFilter::OFF),
    ("webpki_root_certs", LevelFilter::OFF),
    ("http", LevelFilter::OFF),
    ("httparse", LevelFilter::OFF),
];

/// The most `target` may log under [`QUIET_TARGETS`]: `None` when it is not one of them
/// (a crate name or a module path under one).
fn quiet_cap(target: &str) -> Option<LevelFilter> {
    QUIET_TARGETS.iter().find_map(|(name, cap)| {
        let under = target
            .strip_prefix(name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"));
        under.then_some(*cap)
    })
}

/// `rust_log` (the value of `RUST_LOG`, if any) with the [`QUIET_TARGETS`] directives
/// added after it.
fn env_filter(rust_log: Option<&str>) -> EnvFilter {
    let base = rust_log
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_FILTER));
    QUIET_TARGETS.iter().fold(base, |f, (name, cap)| {
        match format!("{name}={cap}").parse() {
            Ok(d) => f.add_directive(d),
            Err(_) => f,
        }
    })
}

/// The client's log subscriber: the format layer on `writer`, under the filter of
/// `rust_log` with the [`QUIET_TARGETS`] directives, and those caps once more as a target
/// test of their own, so a more specific `RUST_LOG` directive (`ureq::pool=trace`) cannot
/// lift them.
fn subscriber(
    rust_log: Option<&str>,
    writer: BoxMakeWriter,
) -> impl tracing::Subscriber + Send + Sync {
    let caps = filter_fn(|meta| quiet_cap(meta.target()).is_none_or(|cap| *meta.level() <= cap));
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_filter(env_filter(rust_log).and(caps));
    Registry::default().with(layer)
}

fn main() -> ExitCode {
    let args = tessera_client::Args::parse();
    if let Err(e) = args.check() {
        e.exit();
    }
    let rust_log = std::env::var(EnvFilter::DEFAULT_ENV).ok();
    // A scenario log on standard output keeps standard output to itself.
    let writer = if args
        .scenario_log
        .as_deref()
        .is_some_and(|p| p.as_os_str() == "-")
    {
        BoxMakeWriter::new(std::io::stderr)
    } else {
        BoxMakeWriter::new(std::io::stdout)
    };
    subscriber(rust_log.as_deref(), writer).init();
    ExitCode::from(tessera_client::run_to_exit_code(args))
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);

    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(b);
            Ok(b.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn logs_under(rust_log: Option<&str>) -> String {
        let buf = Buf::default();
        let out = buf.clone();
        let sub = subscriber(rust_log, BoxMakeWriter::new(move || out.clone()));
        tracing::subscriber::with_default(sub, || {
            // What the HTTP and TLS crates' records look like through the log bridge: their
            // module paths as targets, a host and a path in the message.
            tracing::trace!(target: "ureq::unversioned::transport", "connect gls.example.invalid:443");
            tracing::debug!(target: "ureq", "GET https://gls.example.invalid/status");
            tracing::trace!(target: "ureq_proto::client", "Host: gls.example.invalid");
            tracing::warn!(target: "ureq_proto", "gls.example.invalid closed");
            tracing::debug!(target: "rustls::client::hs", "server name gls.example.invalid");
            tracing::debug!(target: "rustls_platform_verifier", "verifying gls.example.invalid");
            // The record of the owner's live run: the verifier logs a failed check at error
            // level with the server's name.
            tracing::error!(
                target: "rustls_platform_verifier::verification::others",
                "failed to verify TLS certificate: invalid peer certificate: certificate not \
                 valid for name \"gls.example.invalid\""
            );
            tracing::error!(target: "rustls::conn", "gls.example.invalid: fatal alert");
            tracing::warn!(target: "rustls::conn", "sending a fatal alert");
            tracing::error!(target: "ureq::unversioned", "gls.example.invalid refused");
            tracing::error!(target: "webpki", "gls.example.invalid not valid");
            tracing::trace!(target: "http", "Host: gls.example.invalid");
            tracing::trace!(target: "other_crate", "kept: not one of the crates");
            tracing::info!(target: "tessera_client", "kept: the client's own");
            // The login's own neutral line in place of the verifier's (step and kind only).
            tracing::info!(
                target: "tessera_compat::connector::login_phase",
                step = "datacenter document",
                kind = "name mismatch",
                "official login: HTTPS certificate not valid for the step's address"
            );
            // The same records as the `log` bridge delivers them (the HTTP and TLS crates
            // log through `log`; the subscriber's `init` installs the bridge).
            for (target, level, text) in [
                (
                    "rustls_platform_verifier::verification::others",
                    log::Level::Error,
                    "bridged: failed to verify TLS certificate: certificate not valid for \
                     name \"gls.example.invalid\"",
                ),
                (
                    "rustls::conn",
                    log::Level::Warn,
                    "bridged: gls.example.invalid alert",
                ),
                (
                    "ureq::unversioned::transport",
                    log::Level::Trace,
                    "bridged: connect gls.example.invalid:443",
                ),
                (
                    "ureq_proto::client",
                    log::Level::Debug,
                    "bridged: Host: gls.example.invalid",
                ),
                ("webpki", log::Level::Error, "bridged: gls.example.invalid"),
                (
                    "other_crate::bridge",
                    log::Level::Info,
                    "kept: a bridged record",
                ),
            ] {
                let args = format_args!("{text}");
                let record = log::Record::builder()
                    .args(args)
                    .level(level)
                    .target(target)
                    .module_path_static(Some(target))
                    .build();
                let _ = tracing_log::format_trace(&record);
            }
        });
        let bytes = buf
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[test]
    fn rust_log_trace_shows_no_host_of_the_http_and_tls_crates() {
        for rust_log in [
            Some("trace"),
            Some("trace,ureq=trace,ureq_proto=trace,rustls=trace"),
            Some("info,ureq::unversioned=trace,ureq_proto::client=trace,rustls::client=trace"),
            Some("info,rustls_platform_verifier=error,rustls=error,webpki=trace,http=trace"),
            Some("not a filter ==="),
            None,
        ] {
            let logs = logs_under(rust_log);
            assert!(!logs.contains("example.invalid"), "{rust_log:?}: {logs}");
            assert!(
                logs.contains("kept: the client's own"),
                "{rust_log:?}: {logs}"
            );
            assert!(
                logs.contains("HTTPS certificate not valid for the step's address"),
                "{rust_log:?}: {logs}"
            );
            // Fully off, errors and warnings included.
            assert!(!logs.contains("fatal alert"), "{rust_log:?}: {logs}");
            assert!(!logs.contains("failed to verify"), "{rust_log:?}: {logs}");
            // The bridge reaches the subscriber, and the caps hold for its records too.
            assert!(
                logs.contains("kept: a bridged record"),
                "{rust_log:?}: {logs}"
            );
            assert!(!logs.contains("bridged:"), "{rust_log:?}: {logs}");
            if rust_log.is_some_and(|s| s.starts_with("trace")) {
                assert!(
                    logs.contains("kept: not one of the crates"),
                    "{rust_log:?}: {logs}"
                );
            }
        }
    }

    #[test]
    fn the_caps_name_crates_and_their_modules_only() {
        assert_eq!(quiet_cap("ureq"), Some(LevelFilter::OFF));
        assert_eq!(quiet_cap("ureq::pool"), Some(LevelFilter::OFF));
        assert_eq!(quiet_cap("ureq_proto::x"), Some(LevelFilter::OFF));
        assert_eq!(quiet_cap("rustls::conn"), Some(LevelFilter::OFF));
        assert_eq!(
            quiet_cap("rustls_platform_verifier::verification::others"),
            Some(LevelFilter::OFF)
        );
        assert_eq!(quiet_cap("webpki::verify_cert"), Some(LevelFilter::OFF));
        assert_eq!(quiet_cap("httpx"), None);
        assert_eq!(quiet_cap("ureqx"), None);
        assert_eq!(quiet_cap("tessera_compat::connector"), None);
    }
}
