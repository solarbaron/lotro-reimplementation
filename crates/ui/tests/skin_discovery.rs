// SPDX-License-Identifier: GPL-3.0-or-later
//! Skin discovery and the skin list (`skin-definition.md` §3, §9.1, §9.3; D1–D4, D6, D9),
//! on synthetic skin folders in temporary directories. Nothing is written outside them.

mod skin_common;

use std::path::{Path, PathBuf};

use skin_common::{resolver, write};
use tessera_ui::skin::{
    DefinitionError, MAX_WALK_DEPTH, SkinChoice, SkinSet, WalkIssue, default_skins_root,
    skins_root_in,
};

fn names() -> (tessera_ui::skin::LabelIndex, tessera_ui::skin::LabelIndex) {
    resolver(&[("Art_One", 0x1000_0001)], &[("Root_Panel", 0x1000_0100)])
}

fn def(name: &str, body: &str) -> Vec<u8> {
    format!("<opt><SkinName Name=\"{name}\"/>{body}</opt>").into_bytes()
}

const MAPPING: &str = "<Mapping ArtAssetID=\"Art_One\" FileName=\"a.tga\"/>";
const PANEL: &str =
    "<PanelFile ID=\"p\"><Element ID=\"Root_Panel\" Width=\"1\" Height=\"1\"/></PanelFile>";

fn listing(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p.clone());
            }
            out.push(p);
        }
    }
    out.sort();
    out
}

#[test]
fn worked_example_12_1() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ui/skins");
    write(&root.join("SkinDefinition.xml"), &def("Root", MAPPING));
    write(&root.join("Alpha/SkinDefinition.xml"), &def("Zed", MAPPING));
    write(
        &root.join("Alpha/extra/SkinDefinition.xml"),
        &def("alpha", PANEL),
    );
    write(
        &root.join("Beta/skindefinition.XML"),
        &def(
            "Zed",
            &format!("{MAPPING}<Mapping ArtAssetID=\"art_one\" FileName=\"b.tga\"/>"),
        ),
    );
    write(&root.join("Gamma/SkinDefinition.xml"), &def("Gamma", ""));
    write(&root.join("Delta/Skin.xml"), &def("Delta", MAPPING));
    let before = listing(tmp.path());

    let set = SkinSet::discover(&root, &names());
    let order: Vec<&str> = set.skins().iter().map(|s| s.name()).collect();
    assert_eq!(order, ["alpha", "Zed", "Zed", "Root"]);
    assert_eq!(set.skins()[1].folder, root.join("Alpha"));
    assert_eq!(set.skins()[3].folder, root);
    assert_eq!(set.rejected().len(), 1);
    assert_eq!(
        set.rejected()[0].path,
        root.join("Gamma/SkinDefinition.xml")
    );
    assert_eq!(set.rejected()[0].error, DefinitionError::NothingUsable);

    let choices = set.choices();
    assert_eq!(
        choices,
        [
            SkinChoice::NoSkin,
            SkinChoice::Skin("Root"),
            SkinChoice::Skin("Zed"),
            SkinChoice::Skin("Zed"),
            SkinChoice::Skin("alpha"),
        ]
    );
    // Either `Zed` activates the first discovered one, in Alpha/ (§9.3).
    let zed = set.find("ZED").unwrap();
    assert_eq!(zed.folder, root.join("Alpha"));
    assert!(set.find("").is_none(), "empty = no skin (D6)");
    assert!(set.find("Gamma").is_none());

    assert_eq!(listing(tmp.path()), before, "discovery writes nothing (D1)");
}

/// Whether the file system holding `dir` ignores letter case (the default on Windows and
/// macOS).
fn ignores_case(dir: &Path) -> bool {
    let probe = dir.join("case-probe.tmp");
    std::fs::write(&probe, b"").unwrap();
    let ignores = dir.join("CASE-PROBE.TMP").exists();
    std::fs::remove_file(&probe).unwrap();
    ignores
}

/// Whether two paths name the same components, ignoring ASCII letter case. Compares
/// components, not bytes, so the `/` and `\` separators Windows mixes do not matter.
fn same_ignoring_case(a: &Path, b: &Path) -> bool {
    let a: Vec<_> = a.components().collect();
    let b: Vec<_> = b.components().collect();
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| {
            x.as_os_str()
                .as_encoded_bytes()
                .eq_ignore_ascii_case(y.as_os_str().as_encoded_bytes())
        })
}

#[test]
fn skins_root_letter_case_and_settings_dir() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("UI/Skins/S/SKINDEFINITION.XML"),
        &def("S", MAPPING),
    );
    let root = skins_root_in(tmp.path());
    if ignores_case(tmp.path()) {
        // The plain `ui/skins` already exists there (D2: the exact name when it exists)
        // and names the same folder.
        assert!(
            same_ignoring_case(&root, &tmp.path().join("UI").join("Skins")),
            "{root:?}"
        );
    } else {
        assert_eq!(root, tmp.path().join("UI/Skins"));
    }
    let set = SkinSet::discover(&root, &names());
    assert_eq!(set.skins().len(), 1);

    let settings = tessera_install::SettingsDir {
        path: tmp.path().to_path_buf(),
        preferences: tmp.path().join("UserPreferences.ini"),
        source: tessera_install::SettingsSource::Home,
        exists: true,
        has_preferences: false,
    };
    assert_eq!(default_skins_root(&settings), root);

    // No root at all: an empty set, no error.
    let none = SkinSet::discover(&tmp.path().join("missing/ui/skins"), &names());
    assert!(none.skins().is_empty() && none.rejected().is_empty());
    assert_eq!(none.choices(), [SkinChoice::NoSkin]);
}

#[test]
fn rejected_files_have_reasons() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        &root.join("Broken/SkinDefinition.xml"),
        b"<opt>\n<Mapping>\n</opt>",
    );
    write(
        &root.join("Huge/SkinDefinition.xml"),
        &vec![b' '; tessera_ui::skin::MAX_DEFINITION_BYTES + 1],
    );
    write(&root.join("Good/SkinDefinition.xml"), &def("Good", MAPPING));
    let set = SkinSet::discover(root, &names());
    assert_eq!(set.skins().len(), 1);
    assert_eq!(set.rejected().len(), 2);
    let broken = &set.rejected()[0];
    assert!(broken.path.ends_with("Broken/SkinDefinition.xml"));
    assert_eq!(broken.error.line(), Some(3));
    assert!(matches!(
        set.rejected()[1].error,
        DefinitionError::TooLarge { .. }
    ));
}

#[test]
fn d4_walk_depth() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let mut ok = root.to_path_buf();
    for i in 0..MAX_WALK_DEPTH {
        ok.push(format!("d{i}"));
    }
    write(&ok.join("SkinDefinition.xml"), &def("Deepest", MAPPING));
    let mut too_deep = ok.clone();
    too_deep.push("d16");
    write(
        &too_deep.join("SkinDefinition.xml"),
        &def("TooDeep", MAPPING),
    );
    let set = SkinSet::discover(root, &names());
    let found: Vec<&str> = set.skins().iter().map(|s| s.name()).collect();
    assert_eq!(found, ["Deepest"]);
    assert!(
        set.issues()
            .iter()
            .any(|i| matches!(i, WalkIssue::TooDeep(_)))
    );
}

#[cfg(unix)]
#[test]
fn d4_links_stay_inside_the_root_and_folders_are_walked_once() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    write(
        &outside.join("SkinDefinition.xml"),
        &def("Outside", MAPPING),
    );
    let root = tmp.path().join("skins");
    write(&root.join("In/SkinDefinition.xml"), &def("Inside", MAPPING));
    symlink(&outside, root.join("Out")).unwrap();
    symlink(root.join("In"), root.join("Again")).unwrap();
    symlink(&root, root.join("In/Loop")).unwrap();
    let set = SkinSet::discover(&root, &names());
    let found: Vec<&str> = set.skins().iter().map(|s| s.name()).collect();
    // `Again` (sorted first) is the same folder as `In`, so it is walked once.
    assert_eq!(found, ["Inside"]);
}

#[test]
fn d3_order_is_reproducible() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for (folder, name) in [("b", "Two"), ("A", "One"), ("c", "Three"), ("a", "Zero")] {
        write(
            &root.join(folder).join("SkinDefinition.xml"),
            &def(name, MAPPING),
        );
    }
    let set = SkinSet::discover(root, &names());
    let found: Vec<&str> = set.skins().iter().map(|s| s.name()).collect();
    // Case folded first, then bytes: "A" < "a" < "b" < "c". (On a case-insensitive file
    // system "A" and "a" are one folder and the later write replaced the file.)
    if std::fs::read_dir(root).unwrap().count() == 4 {
        assert_eq!(found, ["One", "Zero", "Two", "Three"]);
    }
}

/// D2: definition file names that differ only in letter case in one folder give one skin,
/// the entry that sorts first by byte value; the other is reported as shadowed.
#[test]
fn d2_one_definition_file_per_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let upper = tmp.path().join("S/SkinDefinition.xml");
    let lower = tmp.path().join("S/skindefinition.xml");
    write(&upper, &def("Upper", MAPPING));
    write(&lower, &def("Lower", MAPPING));
    if std::fs::read_dir(tmp.path().join("S")).unwrap().count() != 2 {
        eprintln!("case-insensitive file system; nothing to check");
        return;
    }
    let set = SkinSet::discover(tmp.path(), &names());
    let found: Vec<&str> = set.skins().iter().map(|s| s.name()).collect();
    assert_eq!(found, ["Upper"]);
    assert_eq!(set.skins()[0].definition_file, upper);
    assert_eq!(
        set.issues(),
        [WalkIssue::ShadowedDefinition {
            path: lower,
            by: upper
        }]
    );
    assert!(set.issues()[0].to_string().contains("letter case"));
}
