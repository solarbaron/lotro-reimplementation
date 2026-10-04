// SPDX-License-Identifier: GPL-3.0-or-later
//! Walker command-line behaviour that needs no game data: output paths inside the
//! install are refused before anything runs, and a headless run must have an end.

use clap::Parser as _;

/// A headless walker run refuses outputs inside the game install before doing anything
/// (the "install" here is an empty temporary directory).
#[test]
fn headless_outputs_never_go_into_the_install() {
    let install = tempfile::tempdir().expect("tempdir");
    let png = install.path().join("shot.png");
    let csv = install.path().join("sub/frames.csv");
    for extra in [
        vec!["--screenshot", png.to_str().unwrap()],
        vec![
            "--fly-path",
            "1:1:1,1:2:1",
            "--headless",
            "--frame-log",
            csv.to_str().unwrap(),
        ],
    ] {
        let mut argv = vec![
            "tessera-client",
            "--game-dir",
            install.path().to_str().unwrap(),
        ];
        argv.extend(extra);
        let args = tessera_client::Args::try_parse_from(argv).expect("args");
        let err = tessera_client::headless::run(&args).expect_err("refused");
        assert!(
            format!("{err:#}").contains("inside the game install"),
            "{err:#}"
        );
    }
    assert_eq!(std::fs::read_dir(install.path()).unwrap().count(), 0);
}

#[test]
fn plain_headless_is_rejected() {
    assert!(tessera_client::Args::try_parse_from(["tessera-client", "--headless"]).is_err());
}
