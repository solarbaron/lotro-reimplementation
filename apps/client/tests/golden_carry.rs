// SPDX-License-Identifier: GPL-3.0-or-later
//! The carry-over of tier-2 goldens after an install update (decision 0031, owner decision
//! 2026-10-31) and its review records, with synthetic pictures in a temporary base
//! directory (outside every work tree). Needs no install and no GPU.

#[path = "ui_common/goldens.rs"]
mod goldens;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use goldens::{Flags, Outcome, Rgba};

// Invented install versions (no real install's).
const OLD: &str = "0123456789abcdef";
const OLDER: &str = "fedcba9876543210";
const NEW: &str = "00112233445566ff";

/// A synthetic 4 × 3 picture whose pixels follow `seed`.
fn picture(seed: u8) -> Rgba {
    let mut data = Vec::new();
    for i in 0..12u8 {
        data.extend_from_slice(&[i.wrapping_mul(17), seed, 255 - i, 255]);
    }
    Rgba {
        width: 4,
        height: 3,
        data,
    }
}

struct Base {
    tmp: tempfile::TempDir,
}

impl Base {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn path(&self) -> &Path {
        self.tmp.path()
    }

    /// `<base>/tessera-client/golden_carry/function/<version>`.
    fn dir(&self, version: &str) -> PathBuf {
        self.path()
            .join("tessera-client")
            .join("golden_carry")
            .join("function")
            .join(version)
    }

    fn flags(&self) -> Flags {
        Flags {
            base: Some(self.path().to_path_buf()),
            ..Flags::default()
        }
    }

    fn require(&self) -> Flags {
        Flags {
            require: true,
            ..self.flags()
        }
    }

    fn bless(&self) -> Flags {
        Flags {
            bless_local: true,
            ..self.flags()
        }
    }

    /// Writes the golden `name` of `version`.
    fn plant(&self, version: &str, name: &str, img: &Rgba) -> PathBuf {
        let p = self.dir(version).join(format!("{name}.png"));
        goldens::write_atomic(&p, img).expect("planted");
        p
    }

    fn entries(&self, version: &str, kind: &str) -> Vec<PathBuf> {
        let dir = self.path().join("review").join(version).join(kind);
        let Ok(rd) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        v.sort();
        v
    }
}

fn set_mtime(dir: &Path, at: SystemTime) {
    std::fs::File::open(dir)
        .expect("open dir")
        .set_modified(at)
        .expect("mtime");
}

#[test]
fn a_match_with_the_previous_version_is_carried_over() {
    let b = Base::new();
    let old = b.plant(OLD, "pic", &picture(7));
    for flags in [b.flags(), b.require()] {
        let _ = std::fs::remove_dir_all(b.dir(NEW));
        let out = goldens::check_with(&b.dir(NEW), "pic", &picture(7), &flags).expect("carried");
        let want = b.dir(NEW).join("pic.png");
        assert_eq!(
            out,
            Outcome::CarriedOver {
                path: want.clone(),
                from_version: OLD.into()
            }
        );
        assert!(out.matched());
        assert_eq!(out.word(), "carried over");
        assert_eq!(
            std::fs::read(&want).expect("new golden"),
            std::fs::read(&old).expect("old golden"),
            "the previous golden is copied"
        );
        assert!(b.entries(NEW, "changed").is_empty(), "no review entry");
        assert_eq!(b.entries(NEW, "carried").len(), 1);
        // The next run passes against the carried golden.
        assert!(matches!(
            goldens::check_with(&b.dir(NEW), "pic", &picture(7), &flags),
            Ok(Outcome::Passed(_))
        ));
    }
    assert_eq!(
        std::fs::read_to_string(b.path().join("review/last-version")).expect("version"),
        format!("{NEW}\n")
    );
}

#[test]
fn a_mismatch_with_the_previous_version_is_never_blessed() {
    for require in [false, true] {
        let b = Base::new();
        let old = b.plant(OLD, "pic", &picture(7));
        let flags = if require { b.require() } else { b.flags() };
        let e = goldens::check_with(&b.dir(NEW), "pic", &picture(90), &flags)
            .expect_err("changed picture");
        assert!(
            e.contains(&format!(
                "changed since install {OLD}: needs the owner's review"
            )),
            "{e}"
        );
        let dir = b.dir(NEW);
        assert!(!dir.join("pic.png").exists(), "no golden written");
        assert!(dir.join("pic.actual.png").is_file());
        assert!(dir.join("pic.diff.png").is_file());
        assert!(e.contains(&dir.join("pic.actual.png").display().to_string()));
        assert!(e.contains(&dir.join("pic.diff.png").display().to_string()));
        let entries = b.entries(NEW, "changed");
        assert_eq!(entries.len(), 1);
        let line = std::fs::read_to_string(&entries[0]).expect("entry");
        let cols: Vec<&str> = line.trim_end().split('\t').collect();
        assert_eq!(
            cols,
            [
                "tessera-client",
                "golden_carry",
                "function",
                "pic",
                OLD,
                old.display().to_string().as_str(),
                dir.join("pic.actual.png").display().to_string().as_str(),
                dir.join("pic.diff.png").display().to_string().as_str(),
                "12",
                "-",
            ]
        );
        // A second run fails the same way and keeps one entry.
        assert!(goldens::check_with(&b.dir(NEW), "pic", &picture(90), &flags).is_err());
        assert_eq!(b.entries(NEW, "changed").len(), 1);
        assert!(!dir.join("pic.png").exists());
    }
}

#[test]
fn a_size_change_is_listed_for_review() {
    let b = Base::new();
    b.plant(OLD, "pic", &picture(7));
    let small = Rgba {
        width: 1,
        height: 1,
        data: vec![0, 0, 0, 255],
    };
    let e = goldens::check_with(&b.dir(NEW), "pic", &small, &b.flags()).expect_err("size");
    assert!(e.contains("needs the owner's review"), "{e}");
    assert!(!b.dir(NEW).join("pic.png").exists());
    let line = std::fs::read_to_string(&b.entries(NEW, "changed")[0]).expect("entry");
    assert_eq!(line.trim_end().split('\t').nth(7), Some("-"), "no diff");
}

#[test]
fn bless_local_blesses_a_changed_picture_and_clears_its_entry() {
    let b = Base::new();
    b.plant(OLD, "pic", &picture(7));
    assert!(goldens::check_with(&b.dir(NEW), "pic", &picture(90), &b.flags()).is_err());
    assert_eq!(b.entries(NEW, "changed").len(), 1);
    let out = goldens::check_with(&b.dir(NEW), "pic", &picture(90), &b.bless()).expect("bless");
    assert!(matches!(out, Outcome::Blessed(_)));
    assert!(b.entries(NEW, "changed").is_empty(), "entry cleared");
    assert!(!b.dir(NEW).join("pic.actual.png").exists());
    assert!(!b.dir(NEW).join("pic.diff.png").exists());
    assert!(matches!(
        goldens::check_with(&b.dir(NEW), "pic", &picture(90), &b.require()),
        Ok(Outcome::Passed(_))
    ));
}

#[test]
fn a_later_match_clears_the_review_entry() {
    let b = Base::new();
    b.plant(OLD, "pic", &picture(7));
    assert!(goldens::check_with(&b.dir(NEW), "pic", &picture(90), &b.flags()).is_err());
    assert_eq!(b.entries(NEW, "changed").len(), 1);
    // The picture is fixed and matches the previous version again.
    let out = goldens::check_with(&b.dir(NEW), "pic", &picture(7), &b.require()).expect("ok");
    assert!(matches!(out, Outcome::CarriedOver { .. }));
    assert!(b.entries(NEW, "changed").is_empty(), "entry cleared");
    assert!(!b.dir(NEW).join("pic.actual.png").exists());
}

#[test]
fn without_a_previous_golden_the_first_run_rules_hold() {
    let b = Base::new();
    let e = goldens::check_with(&b.dir(NEW), "pic", &picture(7), &b.require())
        .expect_err("missing under REQUIRE");
    assert!(e.contains("missing and TESSERA_GOLDEN_REQUIRE=1"), "{e}");
    assert!(!b.dir(NEW).join("pic.png").exists());
    let out = goldens::check_with(&b.dir(NEW), "pic", &picture(7), &b.flags()).expect("bless");
    assert_eq!(out, Outcome::Blessed(b.dir(NEW).join("pic.png")));
    assert!(b.entries(NEW, "changed").is_empty());
    let new = b.entries(NEW, "new");
    assert_eq!(new.len(), 1);
    let line = std::fs::read_to_string(&new[0]).expect("entry");
    assert_eq!(line.split('\t').nth(5), Some("blessed"));
    // A golden of another name in the previous version is not a previous golden.
    b.plant(OLD, "other", &picture(1));
    assert!(matches!(
        goldens::check_with(&b.dir(NEW), "third", &picture(7), &b.flags()),
        Ok(Outcome::Blessed(_))
    ));
}

#[test]
fn the_most_recently_modified_previous_version_is_chosen() {
    let b = Base::new();
    b.plant(OLDER, "pic", &picture(1));
    b.plant(OLD, "pic", &picture(50));
    let now = SystemTime::now();
    set_mtime(&b.dir(OLD), now - Duration::from_secs(3600));
    set_mtime(&b.dir(OLDER), now - Duration::from_secs(60));
    // OLDER is newer on disk: it is the one compared.
    let out = goldens::check_with(&b.dir(NEW), "pic", &picture(1), &b.flags()).expect("ok");
    assert_eq!(
        out,
        Outcome::CarriedOver {
            path: b.dir(NEW).join("pic.png"),
            from_version: OLDER.into()
        }
    );
    let _ = std::fs::remove_dir_all(b.dir(NEW));
    let e = goldens::check_with(&b.dir(NEW), "pic", &picture(50), &b.flags()).expect_err("no");
    assert!(e.contains(&format!("since install {OLDER}")), "{e}");
}

#[test]
fn siblings_not_named_like_a_version_are_ignored() {
    let b = Base::new();
    for odd in ["unknown", "backup", "0123456789ABCDEF", "0123456789abcde"] {
        b.plant(odd, "pic", &picture(9));
    }
    let schema = "schema-0000111122223333";
    assert!(goldens::is_version_name(schema));
    assert!(goldens::is_version_name(OLD));
    for odd in [
        "unknown",
        "backup",
        "0123456789ABCDEF",
        "0123456789abcde",
        "review",
    ] {
        assert!(!goldens::is_version_name(odd), "{odd}");
    }
    let out = goldens::check_with(&b.dir(NEW), "pic", &picture(7), &b.flags()).expect("first");
    assert!(matches!(out, Outcome::Blessed(_)), "{out:?}");
    // A `schema-` directory is a version.
    b.plant(schema, "two", &picture(7));
    let out = goldens::check_with(&b.dir(NEW), "two", &picture(7), &b.flags()).expect("ok");
    assert!(matches!(out, Outcome::CarriedOver { from_version, .. } if from_version == schema));
}

#[test]
fn the_cpu_and_custom_comparisons_judge_the_carry_over() {
    let b = Base::new();
    b.plant(OLD, "pic", &picture(7));
    // One step per channel passes the CPU comparison.
    let mut near = picture(7);
    near.data[0] = near.data[0].wrapping_add(1);
    assert!(matches!(
        goldens::check_with(&b.dir(NEW), "pic", &near, &b.flags()),
        Ok(Outcome::CarriedOver { .. })
    ));
    // A caller's comparison that refuses everything refuses the carry-over too.
    b.plant(OLD, "strict", &picture(7));
    let e = goldens::check_with_compare(&b.dir(NEW), "strict", &picture(7), &b.flags(), |w, _| {
        (false, "refused".into(), w.clone())
    })
    .expect_err("refused");
    assert!(
        e.contains("refused") && e.contains("needs the owner's review"),
        "{e}"
    );
    let line = std::fs::read_to_string(&b.entries(NEW, "changed")[0]).expect("entry");
    assert_eq!(line.trim_end().split('\t').nth(8), Some("0"));
}

#[test]
fn a_directory_outside_the_base_keeps_the_old_rules() {
    let b = Base::new();
    let other = tempfile::tempdir().expect("tempdir");
    let dir = other.path().join(OLD);
    let sib = other.path().join(NEW);
    goldens::write_atomic(&sib.join("pic.png"), &picture(7)).expect("planted");
    let out = goldens::check_with(&dir, "pic", &picture(90), &b.flags()).expect("bless");
    assert!(
        matches!(out, Outcome::Blessed(_)),
        "no carry-over outside the base"
    );
    assert!(
        !b.path().join("review").exists(),
        "no records outside the base"
    );
}

#[test]
fn pending_collects_failures() {
    let p = goldens::Pending::default();
    assert!(p.is_empty());
    p.finish();
    let r = std::panic::catch_unwind(|| {
        let mut p = goldens::Pending::default();
        p.push("one".into());
        p.push("two".into());
        p.finish();
    });
    let msg = r.expect_err("panics");
    let text = msg.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(
        text.contains("2 golden failures") && text.contains("two"),
        "{text}"
    );
}

#[test]
fn side_files_are_written_atomically_and_refused_in_work_trees() {
    let b = Base::new();
    let p = goldens::write_side_file(&b.dir(NEW), "index.txt", b"01\n").expect("written");
    assert_eq!(std::fs::read(p).expect("read"), b"01\n");
    let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("target-golden-test");
    assert!(goldens::write_side_file(&inside, "index.txt", b"x").is_err());
    assert!(!inside.exists());
}

/// A subject read from a previous version's directory: its `subject.txt`.
fn read_subject(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("subject.txt"))
        .ok()
        .map(|s| s.trim_end().to_owned())
}

fn note_subject(b: &Base, version: &str, subject: &str) {
    std::fs::write(b.dir(version).join("subject.txt"), subject).expect("subject");
}

fn check_subject(
    b: &Base,
    name: &str,
    img: &Rgba,
    id: &str,
    flags: &Flags,
) -> Result<Outcome, String> {
    let previous = |d: &Path| read_subject(d);
    let subject = goldens::Subject {
        id,
        previous: &previous,
    };
    goldens::check_with_subject(
        &b.dir(NEW),
        name,
        img,
        flags,
        |w, g| {
            let same = w == g;
            (same, String::new(), w.clone())
        },
        Some(&subject),
    )
}

#[test]
fn a_previous_golden_of_another_subject_is_not_compared() {
    let b = Base::new();
    b.plant(OLD, "slot", &picture(7));
    note_subject(&b, OLD, "item 1");
    // Another record in the slot: a new picture (blessed), never "changed".
    let out = check_subject(&b, "slot", &picture(90), "item 2", &b.flags()).expect("new");
    assert!(matches!(out, Outcome::Blessed(_)), "{out:?}");
    assert!(b.entries(NEW, "changed").is_empty());
    assert!(!b.dir(NEW).join("slot.actual.png").exists());
    let line = std::fs::read_to_string(&b.entries(NEW, "new")[0]).expect("entry");
    let cols: Vec<&str> = line.trim_end().split('\t').collect();
    assert_eq!(cols.get(5), Some(&"blessed"));
    assert_eq!(
        cols.get(7),
        Some(&format!("now item 2; {OLD} showed item 1").as_str())
    );
    // Under REQUIRE it is missing, as on a first run.
    let _ = std::fs::remove_dir_all(b.dir(NEW));
    let e = check_subject(&b, "slot", &picture(90), "item 2", &b.require()).expect_err("missing");
    assert!(e.contains("missing and TESSERA_GOLDEN_REQUIRE=1"), "{e}");
    // A subject that cannot be read there is not the same subject.
    b.plant(OLD, "unread", &picture(7));
    std::fs::remove_file(b.dir(OLD).join("subject.txt")).expect("removed");
    let out = check_subject(&b, "unread", &picture(7), "item 1", &b.flags()).expect("new");
    assert!(matches!(out, Outcome::Blessed(_)), "{out:?}");
}

#[test]
fn the_same_subject_is_carried_over_or_listed_with_it() {
    let b = Base::new();
    b.plant(OLDER, "slot", &picture(7));
    note_subject(&b, OLDER, "item 1");
    b.plant(OLD, "slot", &picture(50));
    note_subject(&b, OLD, "item 9");
    let now = SystemTime::now();
    set_mtime(&b.dir(OLDER), now - Duration::from_secs(3600));
    set_mtime(&b.dir(OLD), now - Duration::from_secs(60));
    // OLD is newer but shows another record: OLDER, of the same record, is compared.
    let out = check_subject(&b, "slot", &picture(7), "item 1", &b.require()).expect("carried");
    assert!(
        matches!(&out, Outcome::CarriedOver { from_version, .. } if from_version == OLDER),
        "{out:?}"
    );
    let line = std::fs::read_to_string(&b.entries(NEW, "carried")[0]).expect("entry");
    assert_eq!(line.trim_end().split('\t').nth(7), Some("item 1"));
    // A change of the same record is listed with its subject.
    let _ = std::fs::remove_dir_all(b.dir(NEW));
    let e = check_subject(&b, "slot", &picture(8), "item 1", &b.flags()).expect_err("changed");
    assert!(e.contains(&format!("changed since install {OLDER}")), "{e}");
    let line = std::fs::read_to_string(&b.entries(NEW, "changed")[0]).expect("entry");
    assert_eq!(line.trim_end().split('\t').nth(9), Some("item 1"));
}

#[test]
fn a_finished_run_removes_the_records_of_pictures_it_did_not_take() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let b = Base::new();
    let dir = b.dir(NEW);
    b.plant(OLD, "a", &picture(7));
    b.plant(OLD, "b", &picture(7));
    b.plant(OLD, "c", &picture(7));
    let flags = b.flags();
    // Run 1 takes a and b (both changed) and c (carried over).
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut p = goldens::Pending::for_dir(&dir, &flags);
        for n in ["a", "b"] {
            if let Err(e) = goldens::check_with(&dir, n, &picture(90), &flags) {
                p.push(e);
            }
        }
        assert!(goldens::check_with(&dir, "c", &picture(7), &flags).is_ok());
        p.finish();
    }));
    assert!(r.is_err(), "two changed pictures fail");
    assert_eq!(b.entries(NEW, "changed").len(), 2);
    assert_eq!(b.entries(NEW, "carried").len(), 1);
    // A run that panics before its end removes nothing.
    let r = catch_unwind(AssertUnwindSafe(|| {
        let _p = goldens::Pending::for_dir(&dir, &flags);
        let _ = goldens::check_with(&dir, "a", &picture(90), &flags);
        panic!("the test failed for another reason");
    }));
    assert!(r.is_err());
    assert_eq!(b.entries(NEW, "changed").len(), 2);
    // A run without any picture removes nothing.
    goldens::Pending::for_dir(&dir, &flags).finish();
    assert_eq!(b.entries(NEW, "changed").len(), 2);
    // Run 2 takes only a: the records of b and c and b's images go.
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut p = goldens::Pending::for_dir(&dir, &flags);
        if let Err(e) = goldens::check_with(&dir, "a", &picture(90), &flags) {
            p.push(e);
        }
        // Dropped, not finished: the same rule.
        drop(p);
    }));
    assert!(r.is_err(), "a is still changed");
    let changed = b.entries(NEW, "changed");
    assert_eq!(changed.len(), 1);
    assert!(
        std::fs::read_to_string(&changed[0])
            .expect("entry")
            .contains("\ta\t")
    );
    assert!(b.entries(NEW, "carried").is_empty());
    assert!(dir.join("a.actual.png").is_file());
    assert!(!dir.join("b.actual.png").exists());
    assert!(!dir.join("b.diff.png").exists());
    assert!(dir.join("c.png").is_file(), "goldens are kept");
}
