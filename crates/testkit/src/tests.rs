// SPDX-License-Identifier: GPL-3.0-or-later
//! Census unit tests on temporary base directories outside every git work tree.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::{Against, Census, CensusError, Count, DriftRow, Options, Outcome, Row};

const V1: &str = "0123456789abcdef";
const V2: &str = "fedcba9876543210";
const V3: &str = "schema-00000000000000aa";

/// A temporary base (with a space in its name) outside every work tree.
fn base() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("temporary directory");
    crate::check_outside_work_trees(tmp.path())
        .expect("the temporary directory must lie outside every git work tree");
    let base = tmp.path().join("census base");
    (tmp, base)
}

fn opts(base: &Path) -> Options {
    Options::at(base)
}

fn open(function: &str, version: &str, options: Options) -> Census {
    Census::open_with("tessera-x", "real_data", function, version, options).expect("open")
}

/// Records the standard counts: `a` (spec 10), `b` (spec 20), `c` (no spec).
fn standard(c: &mut Census, a: u32, b: usize, cc: i64) {
    c.count_vs_spec("a", a, 10);
    c.count_vs_spec("b", b, 20);
    c.count("c", cc);
}

fn failure(e: CensusError) -> crate::Failure {
    match e {
        CensusError::Failed(f) => *f,
        other => panic!("expected a failed census, got {other}"),
    }
}

fn set_mtime(path: &Path, at: SystemTime) {
    let f = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open for mtime");
    f.set_modified(at).expect("set mtime");
}

#[test]
fn first_run_writes_a_baseline_and_passes() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    let path = c.baseline_path().to_path_buf();
    assert_eq!(
        path,
        base.join("tessera-x/real_data/f").join(format!("{V1}.tsv"))
    );
    let out = c.finish();
    let Outcome::Created {
        baseline,
        counts,
        against,
        drift,
        drift_file,
    } = out
    else {
        panic!("expected Created, got {out:?}");
    };
    assert_eq!((baseline, counts), (path.clone(), 3));
    // No previous version: the drift is against the spec values.
    assert_eq!(against, Against::Spec);
    assert_eq!(
        drift,
        vec![DriftRow {
            name: "b".into(),
            previous: None,
            observed: Some(Count(21)),
            spec: Some(Count(20)),
        }]
    );
    let drift_file = drift_file.expect("drift file");
    assert_eq!(
        drift_file,
        base.join("drift")
            .join(V1)
            .join("tessera-x/real_data/f.tsv")
    );
    let text = std::fs::read_to_string(&drift_file).expect("drift file");
    assert!(text.contains("tessera-x\treal_data\tf\tb\tspec\t-\t21\t20\n"));
    let baseline = std::fs::read_to_string(&path).expect("baseline");
    assert!(baseline.contains("\na\t10\t10\nb\t21\t20\nc\t5\t-\n"));
    assert_eq!(
        std::fs::read_to_string(base.join("drift/last-version")).expect("last-version"),
        format!("{V1}\n")
    );
    // No temporary files are left behind.
    let leftovers = std::fs::read_dir(path.parent().expect("dir"))
        .expect("dir")
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn the_same_counts_match_the_baseline() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    c.finish();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    let out = c.finish();
    assert!(matches!(out, Outcome::Matched { counts: 3, .. }), "{out:?}");
}

#[test]
fn a_changed_value_fails_with_a_table() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    c.finish();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 6);
    let f = failure(c.try_finish().expect_err("a changed value fails"));
    assert_eq!(
        f.baseline_mismatch,
        vec![Row {
            name: "c".into(),
            expected: Some(Count(5)),
            observed: Some(Count(6)),
        }]
    );
    let text = f.to_string();
    assert!(text.contains("1 counts differ from the baseline"), "{text}");
    assert!(text.contains("name  baseline  observed"), "{text}");
    assert!(text.contains("c     5         6"), "{text}");
    assert!(text.contains("TESSERA_CENSUS_REBASE=1"), "{text}");
    // The baseline is unchanged.
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    c.finish();
}

#[test]
#[should_panic(expected = "counts differ from the baseline")]
fn finish_panics_on_a_difference() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    c.count("x", 1u8);
    c.finish();
    let mut c = open("f", V1, opts(&base));
    c.count("x", 2u8);
    c.finish();
}

#[test]
fn missing_and_extra_names_fail() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    c.finish();
    let mut c = open("f", V1, opts(&base));
    c.count_vs_spec("a", 10u32, 10);
    c.count("c", 5i64);
    c.count("d", 7u16);
    let f = failure(c.try_finish().expect_err("missing and extra names fail"));
    assert_eq!(
        f.baseline_mismatch,
        vec![
            Row {
                name: "d".into(),
                expected: None,
                observed: Some(Count(7)),
            },
            Row {
                name: "b".into(),
                expected: Some(Count(21)),
                observed: None,
            },
        ]
    );
    let text = f.to_string();
    assert!(text.contains("d     -         7"), "{text}");
    assert!(text.contains("b     21        -"), "{text}");
}

#[test]
fn a_new_version_reports_drift_against_the_newest_previous_baseline() {
    let (_t, base) = base();
    // An older version V3 and the newer V1.
    let mut c = open("f", V3, opts(&base));
    standard(&mut c, 1, 1, 1);
    let p3 = c.baseline_path().to_path_buf();
    c.finish();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    c.count("gone", 9u8);
    let p1 = c.baseline_path().to_path_buf();
    c.finish();
    let now = SystemTime::now();
    set_mtime(&p3, now - Duration::from_secs(7200));
    set_mtime(&p1, now - Duration::from_secs(3600));
    // A file not named like a version is ignored even when it is newer.
    std::fs::write(p1.with_file_name("notes.tsv"), "x\t1\t-\n").expect("write");

    let mut c = open("f", V2, opts(&base));
    standard(&mut c, 10, 22, 5);
    c.count("new", 3u8);
    let p2 = c.baseline_path().to_path_buf();
    let out = c.finish();
    let Outcome::Created {
        against,
        drift,
        drift_file,
        ..
    } = out
    else {
        panic!("expected Created, got {out:?}");
    };
    assert_eq!(against, Against::Previous(V1.into()));
    assert_eq!(
        drift,
        vec![
            DriftRow {
                name: "b".into(),
                previous: Some(Count(21)),
                observed: Some(Count(22)),
                spec: Some(Count(20)),
            },
            DriftRow {
                name: "new".into(),
                previous: None,
                observed: Some(Count(3)),
                spec: None,
            },
            DriftRow {
                name: "gone".into(),
                previous: Some(Count(9)),
                observed: None,
                spec: None,
            },
        ]
    );
    let text = std::fs::read_to_string(drift_file.expect("drift file")).expect("read");
    assert!(text.contains(&format!("tessera-x\treal_data\tf\tb\t{V1}\t21\t22\t20\n")));
    assert!(text.contains(&format!("tessera-x\treal_data\tf\tgone\t{V1}\t9\t-\t-\n")));

    // When V3 is the newest, the drift is against V3.
    set_mtime(&p2, now - Duration::from_secs(1800));
    set_mtime(&p3, now);
    let mut c = open("g", V3, opts(&base));
    c.count("only", 1u8);
    c.finish();
    let mut c = open("f", "schema-00000000000000bb", opts(&base));
    standard(&mut c, 1, 1, 1);
    let out = c.finish();
    assert!(
        matches!(&out, Outcome::Created { against: Against::Previous(v), drift, drift_file: None, .. }
            if v == V3 && drift.is_empty()),
        "{out:?}"
    );
}

#[test]
fn a_new_version_without_drift_writes_no_drift_file() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 20, 5);
    let out = c.finish();
    assert!(
        matches!(&out, Outcome::Created { against: Against::Spec, drift, drift_file: None, .. }
            if drift.is_empty()),
        "{out:?}"
    );
    assert!(!base.join("drift").join(V1).exists());
}

#[test]
fn strict_fails_on_a_spec_difference() {
    let (_t, base) = base();
    let strict = Options {
        strict: true,
        ..opts(&base)
    };
    // First run: the baseline is written, but strict fails on b.
    let mut c = open("f", V1, strict.clone());
    standard(&mut c, 10, 21, 5);
    let f = failure(c.try_finish().expect_err("strict fails"));
    assert!(f.baseline_mismatch.is_empty());
    assert_eq!(
        f.spec_mismatch,
        vec![Row {
            name: "b".into(),
            expected: Some(Count(20)),
            observed: Some(Count(21)),
        }]
    );
    assert!(f.to_string().contains("1 counts differ from the spec"));
    // Without strict the same counts match the baseline written above.
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    assert!(matches!(c.finish(), Outcome::Matched { .. }));
    // Strict with every spec value met passes.
    let mut c = open("g", V1, strict);
    standard(&mut c, 10, 20, 99);
    assert!(matches!(c.finish(), Outcome::Created { .. }));
}

#[test]
fn rebase_rewrites_the_baseline() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 5);
    c.finish();
    let rebase = Options {
        rebase: true,
        ..opts(&base)
    };
    let mut c = open("f", V1, rebase);
    standard(&mut c, 10, 21, 6);
    let out = c.finish();
    assert_eq!(
        out,
        Outcome::Rebased {
            baseline: base.join("tessera-x/real_data/f").join(format!("{V1}.tsv")),
            counts: 3,
            changed: vec![Row {
                name: "c".into(),
                expected: Some(Count(5)),
                observed: Some(Count(6)),
            }],
        }
    );
    let mut c = open("f", V1, opts(&base));
    standard(&mut c, 10, 21, 6);
    assert!(matches!(c.finish(), Outcome::Matched { .. }));
}

#[test]
fn a_base_inside_a_work_tree_is_refused() {
    let (t, _) = base();
    let repo = t.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("mkdir");
    let err = Census::open_with(
        "tessera-x",
        "real_data",
        "f",
        V1,
        Options::at(repo.join("sub/census")),
    )
    .expect_err("refused");
    assert!(
        matches!(&err, CensusError::InsideWorkTree { repo: r, .. } if r.ends_with("repo")),
        "{err}"
    );
    assert!(!repo.join("sub").exists(), "nothing is written");
}

#[test]
fn a_base_inside_the_install_is_refused() {
    let (t, _) = base();
    let install = t.path().join("install");
    std::fs::create_dir_all(&install).expect("mkdir");
    for dir in [
        install.clone(),
        install.join("census"),
        install.join("a/b/census"),
    ] {
        let err = crate::check_outside_install(&dir, &install).expect_err("refused");
        assert!(matches!(&err, CensusError::InsideInstall { .. }), "{err}");
    }
    assert!(!install.join("a").exists(), "nothing is written");
    for dir in [t.path().join("census"), t.path().join("install-census/x")] {
        crate::check_outside_install(&dir, &install).expect("outside the install");
    }
}

#[test]
fn an_archive_that_does_not_open_is_an_error_not_an_absent_record() {
    let (t, _) = base();
    let path = t.path().join("client_general.dat");
    std::fs::write(&path, [0u8; 64]).expect("write");
    let err = crate::version::content_entry(&path).expect_err("a failed reader");
    assert!(
        matches!(&err, CensusError::VersionInput { archive, .. } if archive == &path),
        "{err}"
    );
}

#[test]
fn invalid_components_versions_and_names_are_refused() {
    let (_t, base) = base();
    for (p, t, f) in [
        ("", "t", "f"),
        ("a/b", "t", "f"),
        ("p", "..", "f"),
        ("p", "t", "a\tb"),
    ] {
        assert!(
            matches!(
                Census::open_with(p, t, f, V1, opts(&base)),
                Err(CensusError::InvalidComponent { .. })
            ),
            "{p:?} {t:?} {f:?}"
        );
    }
    for v in [
        "unknown",
        "0123456789ABCDEF",
        "schema-0123",
        "../0123456789abcdef",
    ] {
        assert!(
            matches!(
                Census::open_with("p", "t", "f", v, opts(&base)),
                Err(CensusError::InvalidVersion(_))
            ),
            "{v}"
        );
    }
    let mut c = open("f", V1, opts(&base));
    c.count("ok", 1u8);
    c.count("ok", 1u8);
    c.count("tab\there", 1u8);
    c.count("", 1u8);
    c.count("#comment", 1u8);
    let f = failure(c.try_finish().expect_err("invalid names fail"));
    assert_eq!(f.invalid.len(), 4, "{:?}", f.invalid);
    assert!(!base.join("tessera-x").exists(), "nothing is written");
}

#[test]
fn a_malformed_baseline_is_an_error() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    c.count("x", 1u8);
    let path = c.baseline_path().to_path_buf();
    c.finish();
    std::fs::write(&path, "# header\nx\tone\t-\n").expect("write");
    let mut c = open("f", V1, opts(&base));
    c.count("x", 1u8);
    let err = c.try_finish().expect_err("malformed");
    assert!(
        matches!(err, CensusError::BadBaseline { line: 2, .. }),
        "{err}"
    );
}

#[test]
#[should_panic(expected = "dropped without finish()")]
fn an_unfinished_census_panics() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    c.count("x", 1u8);
    drop(c);
}

#[test]
fn every_integer_type_and_literal_converts() {
    let (_t, base) = base();
    let mut c = open("f", V1, opts(&base));
    c.count_vs_spec("u8", 1u8, 1);
    c.count_vs_spec("u16", 1u16, 1);
    c.count_vs_spec("u32", 1u32, 1);
    c.count_vs_spec("u64", u64::MAX, u64::MAX);
    c.count_vs_spec("usize", 1usize, 1);
    c.count_vs_spec("i64", -5i64, -5);
    c.count_vs_spec("big literal", 5_000_000_000u64, 5_000_000_000u64);
    c.count("isize", -1isize);
    let path = c.baseline_path().to_path_buf();
    c.finish();
    let text = std::fs::read_to_string(path).expect("baseline");
    assert!(text.contains(&format!("u64\t{}\t{}\n", u64::MAX, u64::MAX)));
    assert!(text.contains("i64\t-5\t-5\n"));
}

#[test]
fn parallel_censuses_write_their_own_drift_files() {
    let (_t, base) = base();
    let threads: Vec<_> = (0..8u32)
        .map(|i| {
            let base = base.clone();
            std::thread::spawn(move || {
                let mut c =
                    Census::open_with("tessera-x", "real_data", &format!("f{i}"), V1, opts(&base))
                        .expect("open");
                for k in 0..50u32 {
                    c.count_vs_spec(&format!("n{k}"), k + i + 1, k);
                }
                c.finish()
            })
        })
        .collect();
    for t in threads {
        let out = t.join().expect("thread");
        assert!(
            matches!(&out, Outcome::Created { drift, drift_file: Some(_), .. } if drift.len() == 50),
            "{out:?}"
        );
    }
    for i in 0..8u32 {
        let text = std::fs::read_to_string(
            base.join("drift")
                .join(V1)
                .join(format!("tessera-x/real_data/f{i}.tsv")),
        )
        .expect("drift file");
        let rows = text.lines().filter(|l| !l.starts_with('#')).count();
        assert_eq!(rows, 50, "f{i}");
        assert!(text.contains(&format!("\tf{i}\tn0\tspec\t-\t{}\t0\n", i + 1)));
    }
}

#[test]
fn the_macro_names_the_calling_crate() {
    // In unit tests the crate name is this library's.
    let dir = PathBuf::from("/nonexistent/install");
    let err = crate::census!("f", &dir).expect_err("no install");
    assert!(matches!(err, CensusError::NoVersion(_)), "{err}");
}
