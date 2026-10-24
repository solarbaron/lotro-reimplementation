// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the consent module (`official-entry.md` §5.3, §5.6, §6.1; OE-T3, OE-T4,
//! OE-T5, OE-T12, OE-T15 on the record's side).

// Several helpers serve only the tests that need a persisted record (Unix-like systems).
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::path::Path;

use super::record::{KEY_FILE, RECORD_FILE, RecordValues, evaluate, record_mac, render_document};
use super::*;

/// OE-T4: the digest of the shipped §5.2 texts. Changing any of the five texts, the language
/// or the revision fails this test until this value is updated and the revision raised in the
/// same change (§5.3).
const PINNED_DIGEST: &str = "de59eadd38a122fb099c2fd511d98a96211b6ba9eff5ea86ef36c38db79e903e";

#[test]
fn oe_t4_shipped_digest_is_pinned() {
    let text = WarningText::shipped();
    assert_eq!(text.revision, 1);
    assert_eq!(text.language, "en");
    assert_eq!(text.digest().hex(), PINNED_DIGEST);
}

#[test]
fn digest_changes_with_every_item() {
    let base = WarningText::shipped();
    let d = base.digest();
    type Change = Box<dyn Fn(&mut WarningText)>;
    let variants: [Change; 8] = [
        Box::new(|t| t.language = "de".into()),
        Box::new(|t| t.revision += 1),
        Box::new(|t| t.title.push('.')),
        Box::new(|t| t.body = t.body.replacen("Tessera", "Tesera", 1)),
        Box::new(|t| t.checkbox.push(' ')),
        Box::new(|t| t.accept.make_ascii_uppercase()),
        Box::new(|t| t.decline = "Refuse".into()),
        // Moving text between items changes the digest (length prefixes).
        Box::new(|t| {
            t.title.push('X');
            t.body.remove(0);
            t.body.insert(0, 'X');
        }),
    ];
    for (i, change) in variants.iter().enumerate() {
        let mut t = base.clone();
        change(&mut t);
        assert_ne!(t.digest(), d, "variant {i}");
    }
}

#[test]
fn digest_uses_normalisation_form_c() {
    let mut composed = WarningText::shipped();
    composed.title = "Caf\u{e9}".into();
    let mut decomposed = composed.clone();
    decomposed.title = "Cafe\u{301}".into();
    assert_eq!(composed.digest(), decomposed.digest());
}

/// Makes a FIFO at `path` without starting a program (OE-T8 (b)) and without unsafe code
/// (the crate forbids it): `mkfifoat` of rustix where it has one, `mkfifo` of nix on Apple
/// systems, which rustix leaves out.
#[cfg(all(unix, not(target_vendor = "apple")))]
fn make_fifo(path: &Path) {
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        path,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .expect("fifo");
}

/// See the other `make_fifo`.
#[cfg(target_vendor = "apple")]
fn make_fifo(path: &Path) {
    nix::unistd::mkfifo(
        path,
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .expect("fifo");
}

fn accept_now(dir: &Path, text: &WarningText) -> Acceptance {
    let (acc, write) = accept(Some(dir), text, UserConfirmation::confirmed_by_user());
    write.wait().expect("record written");
    acc
}

#[cfg(unix)]
#[test]
fn accepted_record_checks_and_gives_a_proof() {
    let dir = tempfile::tempdir().expect("temp dir");
    let text = WarningText::shipped();
    let acc = accept_now(dir.path(), &text);
    assert_eq!(acc.digest(), text.digest());
    assert!(acc.proof().is_current());
    let CheckOutcome::Accepted(again) = check(dir.path(), &text) else {
        panic!("not accepted");
    };
    assert_eq!(again.accepted_at(), acc.accepted_at());
    assert_eq!(again.accepted_date().len(), 10);
    // Owner-only permissions on both files.
    use std::os::unix::fs::PermissionsExt;
    for name in [RECORD_FILE, KEY_FILE] {
        let mode = std::fs::metadata(dir.path().join(name))
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "{name}: {mode:o}");
    }
    let key = std::fs::read(dir.path().join(KEY_FILE)).expect("key");
    assert_eq!(key.len(), KEY_BYTES);
}

#[cfg(unix)]
#[test]
fn oe_t3_text_change_reprompts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let shipped = WarningText::shipped();
    let _ = accept_now(dir.path(), &shipped);
    let mut other_body = shipped.clone();
    other_body.body.push_str(" Changed.");
    let mut other_revision = shipped.clone();
    other_revision.revision = 2;
    let mut other_language = shipped.clone();
    other_language.language = "en-GB".into();
    for t in [&other_body, &other_revision, &other_language] {
        assert_eq!(
            check(dir.path(), t).reason(),
            Some(NotAcceptedReason::TextChanged)
        );
        // Accepting again rewrites the record for the new text.
        let _ = accept_now(dir.path(), t);
        assert!(check(dir.path(), t).is_accepted());
        assert_eq!(
            check(dir.path(), &shipped).reason(),
            Some(NotAcceptedReason::TextChanged)
        );
        let _ = accept_now(dir.path(), &shipped);
    }
}

fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).expect("dir").flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let bytes = std::fs::symlink_metadata(e.path())
            .ok()
            .filter(|m| m.is_file())
            .and_then(|_| std::fs::read(e.path()).ok())
            .unwrap_or_default();
        out.push((name, bytes));
    }
    out.sort();
    out
}

/// Checks that `dir`'s record is not accepted and the files are unchanged.
fn assert_rejected(dir: &Path, text: &WarningText) -> NotAcceptedReason {
    let before = snapshot(dir);
    let outcome = check(dir, text);
    assert_eq!(snapshot(dir), before, "the check changed a file");
    outcome.reason().expect("not accepted")
}

#[cfg(unix)]
#[test]
fn oe_t5_rejected_records() {
    use std::os::unix::fs::PermissionsExt;
    let text = WarningText::shipped();
    let fresh = || {
        let d = tempfile::tempdir().expect("temp dir");
        let _ = accept_now(d.path(), &text);
        d
    };
    let rec = |d: &Path| d.join(RECORD_FILE);
    let key = |d: &Path| d.join(KEY_FILE);
    let edit_doc = |d: &Path, f: &dyn Fn(String) -> String| {
        let s = std::fs::read_to_string(rec(d)).expect("doc");
        std::fs::write(rec(d), f(s)).expect("write");
    };

    // Wrong code.
    let d = fresh();
    edit_doc(d.path(), &|s| {
        let i = s.find("mac = \"").expect("mac") + 7;
        let mut b = s.into_bytes();
        b[i] = if b[i] == b'0' { b'1' } else { b'0' };
        String::from_utf8(b).expect("utf8")
    });
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::WrongBindingOrCode
    );
    // A changed authenticated value (the date).
    let d = fresh();
    edit_doc(d.path(), &|s| {
        s.replacen("accepted_at = \"2", "accepted_at = \"1", 1)
    });
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::WrongBindingOrCode
    );
    // Key missing; key of 31 or 33 bytes.
    let d = fresh();
    std::fs::remove_file(key(d.path())).expect("rm");
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::WrongBindingOrCode
    );
    for len in [31, 33] {
        let d = fresh();
        let mut k = std::fs::read(key(d.path())).expect("key");
        k.resize(len, 7);
        std::fs::write(key(d.path()), k).expect("write");
        assert_ne!(
            check(d.path(), &text).reason(),
            None,
            "key of {len} bytes accepted"
        );
    }
    // Bound to another user ID, machine ID or directory (binding values injected).
    let current = Binding::current(fresh().path()).expect("binding");
    for (i, other) in [
        Binding::new(b"4242424", current.machine(), current.directory()),
        Binding::new(current.user(), b"another-machine", current.directory()),
        Binding::new(current.user(), current.machine(), b"/elsewhere"),
    ]
    .iter()
    .enumerate()
    {
        let d = tempfile::tempdir().expect("temp dir");
        let other = if i == 2 {
            other.clone()
        } else {
            let canon = std::fs::canonicalize(d.path()).expect("canon");
            other.with_directory(canon.as_os_str().as_encoded_bytes())
        };
        write_with_binding(
            d.path(),
            &text,
            UserConfirmation::confirmed_by_user(),
            "2026-10-23T12:00:00Z",
            &other,
        )
        .expect("written");
        assert!(check_with_binding(d.path(), &text, &other).is_ok(), "{i}");
        assert_eq!(
            assert_rejected(d.path(), &text),
            NotAcceptedReason::WrongBindingOrCode,
            "binding {i}"
        );
    }
    // Copied with its key to another directory.
    let d = fresh();
    let e = tempfile::tempdir().expect("temp dir");
    for name in [RECORD_FILE, KEY_FILE] {
        std::fs::copy(d.path().join(name), e.path().join(name)).expect("copy");
        let mut p = std::fs::metadata(e.path().join(name))
            .expect("m")
            .permissions();
        p.set_mode(0o600);
        std::fs::set_permissions(e.path().join(name), p).expect("perm");
    }
    assert_eq!(
        assert_rejected(e.path(), &text),
        NotAcceptedReason::WrongBindingOrCode
    );
    // Unknown, missing or out-of-range keys.
    for f in [
        &(|s: String| s + "extra = 1\n") as &dyn Fn(String) -> String,
        &|s: String| {
            s.lines()
                .filter(|l| !l.starts_with("language"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        &|s: String| s.replace("schema = 1", "schema = 2"),
        &|s: String| s.replace("revision = 1", "revision = 0"),
        &|s: String| s.replace("revision = 1", "revision = 1000001"),
        &|s: String| s.replace("language = \"en\"", "language = \"e\""),
        &|s: String| s.replace("language = \"en\"", "language = \"en_GB\""),
        &|s: String| s.replace("schema = 1", "schema = \"1\""),
        &|s: String| s.replace("Z\"", "+00:00\""),
        &|s: String| {
            s.replace("[", "")
                .replacen("schema = 1", "[t]\nschema = 1", 1)
        },
    ] {
        let d = fresh();
        edit_doc(d.path(), f);
        assert_eq!(assert_rejected(d.path(), &text), NotAcceptedReason::Invalid);
    }
    // 4,097 bytes.
    let d = fresh();
    edit_doc(d.path(), &|s| {
        let pad = 4097 - s.len() - 1;
        format!("{s}#{}", "x".repeat(pad))
    });
    assert_eq!(std::fs::metadata(rec(d.path())).expect("m").len(), 4097);
    assert_eq!(assert_rejected(d.path(), &text), NotAcceptedReason::Invalid);
    // Only the document as Tessera writes it: other spellings of the same values (which the
    // TOML parser reads alike) are refused, byte for byte.
    for f in [
        &(|s: String| format!("{s}# a comment\n")) as &dyn Fn(String) -> String,
        &|s: String| s.replace("revision = 1\n", "revision = 0x1\n"),
        &|s: String| s.replace("revision = 1\n", "revision = +1\n"),
        &|s: String| s.replace("language = \"en\"", "language = 'en'"),
        &|s: String| s.replace("language = \"en\"", "language = \"\\u0065n\""),
        &|s: String| s.replace('\n', "\r\n"),
        &|s: String| s.replace("schema = 1", "schema  = 1"),
        &|s: String| s.trim_end().to_owned(),
        &|s: String| s.replace("revision = 1\n", "revision = 01\n"),
        &|s: String| s.replace("revision = 1\n", "revision = 1_0\n"),
        &|s: String| s.replace("language = \"en\"", "language = \"\"\"en\"\"\""),
        &|s: String| s.replace("language = \"en\"", "language = \"\"\"\nen\"\"\""),
        &|s: String| {
            // Reordered keys: `revision` before `schema`.
            let mut lines: Vec<&str> = s.lines().collect();
            lines.swap(0, 1);
            lines.join("\n") + "\n"
        },
        &|s: String| s.replacen('\n', "\n\n", 1),
        &|s: String| s.replace("Z\"\n", "z\"\n"),
        &|s: String| s.replace("Z\"\n", "+00:00\"\n"),
        &|s: String| format!("\u{feff}{s}"),
    ] {
        let d = fresh();
        edit_doc(d.path(), f);
        assert_eq!(assert_rejected(d.path(), &text), NotAcceptedReason::Invalid);
    }
    // Not UTF-8.
    let d = fresh();
    let mut b = std::fs::read(rec(d.path())).expect("doc");
    b.extend_from_slice(b"# \xff\xfe\n");
    std::fs::write(rec(d.path()), b).expect("write");
    assert_eq!(assert_rejected(d.path(), &text), NotAcceptedReason::Invalid);
    // A symbolic link as the final component (the real record elsewhere).
    let d = fresh();
    let e = tempfile::tempdir().expect("temp dir");
    std::fs::rename(rec(d.path()), e.path().join("real.toml")).expect("mv");
    std::os::unix::fs::symlink(e.path().join("real.toml"), rec(d.path())).expect("link");
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::Unreadable
    );
    // A directory and a FIFO in its place.
    let d = fresh();
    std::fs::remove_file(rec(d.path())).expect("rm");
    std::fs::create_dir(rec(d.path())).expect("dir");
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::Unreadable
    );
    let d = fresh();
    std::fs::remove_file(rec(d.path())).expect("rm");
    // The FIFO is made in-process (no other program is started, OE-T8 (b)).
    make_fifo(&rec(d.path()));
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::Unreadable
    );
    // A device in its place (a link to /dev/null is refused as a link; the check never
    // follows it).
    let d = fresh();
    std::fs::remove_file(rec(d.path())).expect("rm");
    std::os::unix::fs::symlink("/dev/null", rec(d.path())).expect("link");
    assert_eq!(
        assert_rejected(d.path(), &text),
        NotAcceptedReason::Unreadable
    );
    // Group- or world-writable record or key.
    for (name, mode) in [
        (RECORD_FILE, 0o620),
        (RECORD_FILE, 0o602),
        (KEY_FILE, 0o660),
        (KEY_FILE, 0o606),
    ] {
        let d = fresh();
        let p = d.path().join(name);
        let mut perm = std::fs::metadata(&p).expect("m").permissions();
        perm.set_mode(mode);
        std::fs::set_permissions(&p, perm).expect("perm");
        assert_eq!(
            assert_rejected(d.path(), &text),
            NotAcceptedReason::Unreadable,
            "{name} {mode:o}"
        );
    }
    // Missing record.
    let d = tempfile::tempdir().expect("temp dir");
    assert_eq!(assert_rejected(d.path(), &text), NotAcceptedReason::Missing);
    assert_eq!(
        check(&d.path().join("no-such-dir"), &text).reason(),
        Some(NotAcceptedReason::Missing)
    );
}

#[cfg(unix)]
#[test]
fn oe_t12_withdraw_removes_both_files() {
    let d = tempfile::tempdir().expect("temp dir");
    let text = WarningText::shipped();
    let _ = accept_now(d.path(), &text);
    withdraw(d.path()).expect("withdraw");
    assert!(!d.path().join(RECORD_FILE).exists() && !d.path().join(KEY_FILE).exists());
    assert_eq!(
        check(d.path(), &text).reason(),
        Some(NotAcceptedReason::Missing)
    );
    // Withdrawing nothing is not an error.
    withdraw(d.path()).expect("withdraw again");
}

#[cfg(unix)]
#[test]
fn oe_t15_failed_write_keeps_the_run_acceptance() {
    use std::os::unix::fs::PermissionsExt;
    let text = WarningText::shipped();
    let d = tempfile::tempdir().expect("temp dir");
    let ro = d.path().join("ro");
    std::fs::create_dir(&ro).expect("dir");
    let mut p = std::fs::metadata(&ro).expect("m").permissions();
    p.set_mode(0o500);
    std::fs::set_permissions(&ro, p).expect("perm");
    // A process with the rights to write anyway (root) cannot test this.
    if std::fs::write(ro.join("probe"), b"x").is_ok() {
        let _ = std::fs::remove_file(ro.join("probe"));
        return;
    }
    let (acc, write) = accept(Some(&ro), &text, UserConfirmation::confirmed_by_user());
    assert!(write.wait().is_err());
    // The acceptance counts for the run: it gives proofs.
    assert!(acc.proof().is_current());
    // The next run's check gives not accepted.
    assert!(!check(&ro, &text).is_accepted());
    // A bad key file (31 bytes; writable by group or others) is never overwritten or deleted
    // (invariant 11): the write fails, the next check gives not accepted, the file is
    // unchanged byte for byte and in its permissions.
    for (bytes, mode) in [
        (vec![7u8; 31], 0o600),
        (vec![7u8; 32], 0o620),
        (vec![7u8; 32], 0o606),
    ] {
        let d = tempfile::tempdir().expect("temp dir");
        let key = d.path().join(KEY_FILE);
        std::fs::write(&key, &bytes).expect("key");
        let mut p = std::fs::metadata(&key).expect("m").permissions();
        p.set_mode(mode);
        std::fs::set_permissions(&key, p).expect("perm");
        let (acc, write) = accept(Some(d.path()), &text, UserConfirmation::confirmed_by_user());
        assert!(matches!(write.wait(), Err(WriteError::KeyFile)), "{mode:o}");
        assert!(acc.proof().is_current());
        assert!(!check(d.path(), &text).is_accepted());
        assert_eq!(std::fs::read(&key).expect("key"), bytes);
        let after = std::fs::metadata(&key).expect("m").permissions().mode() & 0o777;
        assert_eq!(after, mode);
        assert!(!d.path().join(RECORD_FILE).exists());
    }
}

/// OE-T16: a key file appears between the writer's check and its no-replace rename.
#[cfg(unix)]
#[test]
fn oe_t16_key_race() {
    use std::os::unix::fs::PermissionsExt;
    // A valid racing key: not replaced, used.
    let d = tempfile::tempdir().expect("temp dir");
    let key = d.path().join(KEY_FILE);
    let theirs = [9u8; KEY_BYTES];
    let got = record::make_key_racing(d.path(), || {
        std::fs::write(&key, theirs).expect("racing key");
        let mut p = std::fs::metadata(&key).expect("m").permissions();
        p.set_mode(0o600);
        std::fs::set_permissions(&key, p).expect("perm");
    })
    .expect("their key is used");
    assert_eq!(got, theirs);
    assert_eq!(std::fs::read(&key).expect("key"), theirs);
    // An invalid racing key: not replaced, the write fails closed.
    let d = tempfile::tempdir().expect("temp dir");
    let key = d.path().join(KEY_FILE);
    let r = record::make_key_racing(d.path(), || {
        std::fs::write(&key, [1u8; 31]).expect("racing key");
    });
    assert!(matches!(r, Err(WriteError::KeyFile)));
    assert_eq!(std::fs::read(&key).expect("key"), vec![1u8; 31]);
    // No temporary file is left behind.
    let left: Vec<_> = std::fs::read_dir(d.path())
        .expect("dir")
        .flatten()
        .collect();
    assert_eq!(left.len(), 1);
}

/// OE-T17 and invariant 10: on Windows no consent file is written or read.
#[cfg(windows)]
#[test]
fn oe_t17_windows_stores_nothing() {
    let d = tempfile::tempdir().expect("temp dir");
    let text = WarningText::shipped();
    // A file in the record's place is not opened (a directory would make an open fail; the
    // check does not even look).
    std::fs::create_dir(d.path().join(RECORD_FILE)).expect("dir");
    assert_eq!(
        check(d.path(), &text).reason(),
        Some(NotAcceptedReason::NotStored)
    );
    std::fs::remove_dir(d.path().join(RECORD_FILE)).expect("rm");
    let (acc, write) = accept(Some(d.path()), &text, UserConfirmation::confirmed_by_user());
    assert!(matches!(write.wait(), Err(WriteError::NotStored)));
    assert!(acc.proof().is_current(), "the acceptance lasts for the run");
    assert_eq!(
        std::fs::read_dir(d.path()).expect("dir").count(),
        0,
        "no file written"
    );
}

#[test]
fn evaluate_accepts_only_a_matching_code() {
    let binding = Binding::new(b"1", b"m", b"/d");
    let text = WarningText::shipped();
    let values = RecordValues {
        schema: SCHEMA,
        revision: 1,
        language: "en".into(),
        text_sha256: text.digest().hex(),
        accepted_at: "2026-10-23T12:00:00Z".into(),
    };
    let key = [9u8; KEY_BYTES];
    let doc = render_document(&values, &record_mac(&key, &values, &binding));
    assert_eq!(
        evaluate(doc.as_bytes(), &key, &binding, &text.digest()),
        Ok(values.clone())
    );
    let mut other_key = key;
    other_key[0] = 8;
    assert_eq!(
        evaluate(doc.as_bytes(), &other_key, &binding, &text.digest()),
        Err(NotAcceptedReason::WrongBindingOrCode)
    );
    let mut changed = text.clone();
    changed.revision = 2;
    assert_eq!(
        evaluate(doc.as_bytes(), &key, &binding, &changed.digest()),
        Err(NotAcceptedReason::TextChanged)
    );
}

#[test]
fn settings_consent_keys_are_never_read() {
    // OE-T6 (c), the record side: the record location is never the settings file's, and a
    // settings document is not a record.
    let d = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        d.path().join(RECORD_FILE),
        "[Official]\naccepted = true\nconsent = \"yes\"\n",
    )
    .expect("write");
    assert!(!check(d.path(), &WarningText::shipped()).is_accepted());
}
