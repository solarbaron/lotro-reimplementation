// SPDX-License-Identifier: GPL-3.0-or-later
//! Writing user-named output files (`extract -o`, `snapshot -o`) without ever touching a
//! game install.
//!
//! Three rules:
//!
//! 1. The *real* destination is checked, not the path as typed: [`resolve`] follows every
//!    symlink on the way, including a dangling one at the end and one that comes after a
//!    `missing/..` step, and the check is repeated on the canonical parent directory once it
//!    exists. Missing directories are created along the resolved path, never along the
//!    typed one (where the OS would resolve `missing/..` by first creating `missing`).
//!    Containment is decided on canonical forms of both sides (on Windows the verbatim,
//!    long-name, real-case form `fs::canonicalize` gives) and, on Unix, also by directory
//!    identity (device and inode of every existing ancestor), so an alias of a protected
//!    directory (a bind mount, or other letter case on a case-insensitive file system) is
//!    caught too.
//! 2. The protected set ([`protected_dirs`]) holds the input's own directory (and the real
//!    directory of every input archive reached through a symlink), `--game-dir`,
//!    `$TESSERA_GAME_DIR`, and every install or candidate a full discovery finds.
//! 3. The final path component is never written through: a new file is created with
//!    `O_EXCL` semantics (which does not follow a symlink), and `--force` writes a temporary
//!    file next to the destination and renames it over the destination, which replaces the
//!    directory entry (a symlink or a hard link to an archive) instead of writing into the
//!    file it points to.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::input::{Ctx, Input};

/// Symlinks followed while resolving one path before giving up (as `ELOOP` does).
const MAX_LINKS: usize = 40;

/// A directory output must stay out of, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protected {
    /// The directory as given.
    pub path: PathBuf,
    /// What it is, for the error message (`the game directory`, ...).
    pub why: &'static str,
}

impl Protected {
    /// A protected directory.
    pub fn new(path: impl Into<PathBuf>, why: &'static str) -> Self {
        Self {
            path: path.into(),
            why,
        }
    }
}

enum Step {
    Root(PathBuf),
    Parent,
    Name(OsString),
}

fn steps_of(p: &Path) -> Vec<Step> {
    let mut v = Vec::new();
    let mut root = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(x) => root.push(x.as_os_str()),
            Component::RootDir => {
                root.push(Component::RootDir.as_os_str());
                v.push(Step::Root(std::mem::take(&mut root)));
            }
            Component::CurDir => {}
            Component::ParentDir => v.push(Step::Parent),
            Component::Normal(n) => v.push(Step::Name(n.to_owned())),
        }
    }
    if !root.as_os_str().is_empty() {
        v.insert(0, Step::Root(root));
    }
    v
}

/// The real location `p` names: an absolute path with every symlink resolved, also where
/// the path does not exist yet. A symlink is followed even when its target is missing; a
/// component that does not exist is taken literally, and a `..` after it cancels it (as it
/// will once `create_dir_all` has made it a real directory).
pub fn resolve(p: &Path) -> Result<PathBuf> {
    let abs = std::path::absolute(p).with_context(|| format!("bad path {}", p.display()))?;
    let mut todo: Vec<Step> = steps_of(&abs);
    todo.reverse();
    let mut out = PathBuf::new();
    let mut links = 0usize;
    while let Some(step) = todo.pop() {
        match step {
            Step::Root(r) => out = r,
            Step::Parent => {
                out.pop();
            }
            Step::Name(n) => {
                let next = out.join(&n);
                match fs::symlink_metadata(&next) {
                    Ok(m) if m.file_type().is_symlink() => {
                        links += 1;
                        if links > MAX_LINKS {
                            bail!("too many levels of symbolic links in {}", p.display());
                        }
                        let target = fs::read_link(&next)
                            .with_context(|| format!("cannot read link {}", next.display()))?;
                        // Relative targets resolve against the link's directory (`out`).
                        let mut t = steps_of(&target);
                        t.reverse();
                        todo.extend(t);
                    }
                    // Exists, or does not (yet): either way the name is taken as is.
                    _ => out = next,
                }
            }
        }
    }
    Ok(out)
}

/// The real location of `p`, an absolute path without `..` (as [`resolve`] returns): its
/// deepest existing ancestor in canonical form with the components that do not exist yet
/// appended. Both sides of a containment test go through this, so they are compared in the
/// same form (on Windows `fs::canonicalize` gives `\\?\C:\...` while `std::path::absolute`
/// gives `C:\...`, and the two never match as path prefixes).
fn real_location(p: &Path) -> PathBuf {
    let mut base = p.to_path_buf();
    let mut tail: Vec<OsString> = Vec::new();
    loop {
        if let Ok(mut out) = fs::canonicalize(&base) {
            for n in tail.iter().rev() {
                out.push(n);
            }
            return out;
        }
        match base.file_name() {
            Some(n) => {
                tail.push(n.to_owned());
                base.pop();
            }
            None => return p.to_path_buf(),
        }
    }
}

/// The canonical form of a protected directory, or `None` if it cannot be resolved.
fn canonical_dir(d: &Path) -> Option<PathBuf> {
    resolve(d).ok().map(|r| real_location(&r))
}

/// Identity of an existing directory: (device, inode).
#[cfg(unix)]
fn dir_id(p: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(p)
        .ok()
        .filter(fs::Metadata::is_dir)
        .map(|m| (m.dev(), m.ino()))
}

/// Identities of every existing directory on the way from `p` up to the root.
#[cfg(unix)]
fn ancestor_ids(p: &Path) -> Vec<(u64, u64)> {
    p.ancestors().filter_map(dir_id).collect()
}

/// The parent directory of a typed output path (`.` for a bare file name).
fn parent_of(output: &Path) -> &Path {
    output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Fails if the real location of `output` is inside any of `protected`: both the file a
/// final symlink points to and the directory entry that is actually written (rule 3).
pub fn check_output_path(output: &Path, protected: &[Protected]) -> Result<()> {
    check_resolved(output, &resolve(output)?, protected)?;
    if let Some(name) = output.file_name() {
        check_resolved(output, &resolve(parent_of(output))?.join(name), protected)?;
    }
    Ok(())
}

fn check_resolved(output: &Path, target: &Path, protected: &[Protected]) -> Result<()> {
    let target = real_location(target);
    #[cfg(unix)]
    let ids = ancestor_ids(&target);
    for p in protected {
        let Some(dir) = canonical_dir(&p.path) else {
            continue;
        };
        let inside = target.starts_with(&dir);
        #[cfg(unix)]
        let inside = inside || dir_id(&dir).is_some_and(|id| ids.contains(&id));
        if inside {
            bail!(
                "refusing to write {}: it is inside {} {} (archives and the directories \
                 holding them are only ever read)",
                output.display(),
                p.why,
                dir.display()
            );
        }
    }
    Ok(())
}

/// Every directory output must stay out of when reading `input`: the input's directory and
/// the real directories of its archives, `--game-dir`, `$TESSERA_GAME_DIR`, and every
/// install and rejected candidate that discovery finds (searched without the environment
/// override, so Steam, launcher and Wine installs are covered even when it is set).
pub fn protected_dirs(ctx: &Ctx, input: &Input) -> Vec<Protected> {
    let mut v = input.protected_dirs();
    if let Some(d) = &ctx.game_dir {
        v.push(Protected::new(d, "the game directory (--game-dir)"));
    }
    if let Some(d) = std::env::var_os(tessera_install::GAME_DIR_ENV).filter(|d| !d.is_empty()) {
        v.push(Protected::new(d, "the game directory ($TESSERA_GAME_DIR)"));
    }
    let env = tessera_install::SearchEnv {
        game_dir: None,
        ..tessera_install::SearchEnv::from_process()
    };
    match tessera_install::discover_with(&env, None) {
        Ok(d) => {
            v.extend(
                d.installs
                    .into_iter()
                    .map(|i| Protected::new(i.root, "a detected game install")),
            );
            v.extend(
                d.rejected
                    .into_iter()
                    .map(|r| Protected::new(r.path, "a candidate game directory")),
            );
        }
        Err(e) => tracing::debug!(error = %e, "install discovery failed"),
    }
    v
}

/// Writes `bytes` to `output` under the rules in the module docs. Without `force` an
/// existing file (or symlink) is an error.
pub fn write_output(
    output: &Path,
    bytes: &[u8],
    force: bool,
    protected: &[Protected],
) -> Result<()> {
    let Some(name) = output.file_name().map(ToOwned::to_owned) else {
        bail!("{} does not name a file", output.display());
    };
    check_output_path(output, protected)?;
    // Create the directories along the resolved parent, which has no `..` left: every one
    // created is an ancestor of the checked destination, so none is inside a protected
    // directory. (`create_dir_all` on the typed path would create `missing` to resolve
    // `missing/..`, possibly inside one.)
    let parent = resolve(parent_of(output))?;
    fs::create_dir_all(&parent).with_context(|| format!("cannot create {}", parent.display()))?;
    // The parent exists now: check the real destination once more, from its canonical form.
    let dir = fs::canonicalize(&parent)
        .with_context(|| format!("cannot resolve {}", parent.display()))?;
    let dest = dir.join(&name);
    check_resolved(output, &dest, protected)?;
    if force {
        replace(&dir, &dest, bytes).with_context(|| format!("cannot write {}", output.display()))
    } else {
        // create_new is O_CREAT|O_EXCL: it fails on any existing entry, symlinks included,
        // and never follows one.
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)
            .with_context(|| {
                format!(
                    "cannot create {} (use --force to overwrite)",
                    output.display()
                )
            })?;
        f.write_all(bytes)
            .and_then(|()| f.flush())
            .with_context(|| format!("cannot write {}", output.display()))
    }
}

/// Writes a temporary file in `dir` and renames it to `dest`. The temporary name does not
/// embed the destination's name, so it stays short whatever that is (a name near the
/// file-name limit would otherwise make it too long).
fn replace(dir: &Path, dest: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut last = None;
    for attempt in 0..16u32 {
        let tmp = dir.join(format!(".tessera-tmp-{}-{attempt}", std::process::id()));
        let mut f = match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                last = Some(e);
                continue;
            }
            Err(e) => return Err(e),
        };
        let r = f
            .write_all(bytes)
            .and_then(|()| f.flush())
            .and_then(|()| fs::rename(&tmp, dest));
        if r.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        return r;
    }
    Err(last.unwrap_or_else(|| io::Error::other("no free temporary file name")))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, symlink};

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(tag: &str) -> (Tmp, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "dattool-output-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&base);
        let game = base.join("game");
        let out = base.join("out");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&out).unwrap();
        (Tmp(base.clone()), game, out)
    }

    fn prot(game: &Path) -> Vec<Protected> {
        vec![Protected::new(game, "the game directory")]
    }

    #[test]
    fn plain_paths() {
        let (t, game, out) = scratch("plain");
        let p = prot(&game);
        assert!(check_output_path(&game.join("x.bin"), &p).is_err());
        assert!(check_output_path(&game.join("new/dir/x.bin"), &p).is_err());
        assert!(check_output_path(&out.join("../game/x.bin"), &p).is_err());
        assert!(check_output_path(&out.join("x.bin"), &p).is_ok());
        assert!(check_output_path(&t.0.join("gamex.bin"), &p).is_ok());
    }

    #[test]
    fn symlink_after_a_missing_dir_and_dotdot_is_resolved() {
        let (_t, game, out) = scratch("dotdot");
        symlink(&game, out.join("gamelink")).unwrap();
        let p = prot(&game);
        assert!(check_output_path(&out.join("gamelink/a.bin"), &p).is_err());
        let sneaky = out.join("nope/../gamelink/b.bin");
        assert!(check_output_path(&sneaky, &p).is_err());
        assert!(write_output(&sneaky, b"x", false, &p).is_err());
        assert!(write_output(&out.join("nope/../gamelink/newdir/x"), b"x", false, &p).is_err());
        assert!(!game.join("b.bin").exists());
        assert!(!game.join("newdir").exists());
        assert!(!out.join("nope").exists());
        // A relative link target resolves against the link's directory.
        symlink("../game", out.join("rel")).unwrap();
        assert!(check_output_path(&out.join("rel/c.bin"), &p).is_err());
    }

    #[test]
    fn dangling_symlink_into_the_game_dir_is_refused_even_with_force() {
        let (_t, game, out) = scratch("dangling");
        symlink(game.join("planted.bin"), out.join("link.bin")).unwrap();
        let p = prot(&game);
        for force in [false, true] {
            assert!(write_output(&out.join("link.bin"), b"x", force, &p).is_err());
        }
        assert!(!game.join("planted.bin").exists());
    }

    #[test]
    fn symlink_loops_fail_cleanly() {
        let (_t, game, out) = scratch("loop");
        symlink(out.join("b"), out.join("a")).unwrap();
        symlink(out.join("a"), out.join("b")).unwrap();
        assert!(check_output_path(&out.join("a/x"), &prot(&game)).is_err());
    }

    #[test]
    fn force_replaces_links_instead_of_writing_through_them() {
        let (_t, game, out) = scratch("force");
        // A hard link to an "archive" outside any protected dir (hard links cannot be
        // resolved by path, so the rename is what keeps the original intact).
        let archive = game.join("client_x.dat");
        fs::write(&archive, vec![7u8; 4096]).unwrap();
        let hard = out.join("h.bin");
        fs::hard_link(&archive, &hard).unwrap();
        write_output(&hard, b"new", true, &[]).unwrap();
        assert_eq!(fs::read(&archive).unwrap(), vec![7u8; 4096]);
        assert_eq!(fs::read(&hard).unwrap(), b"new");
        assert_eq!(fs::metadata(&archive).unwrap().nlink(), 1);
        // A symlink to an unprotected file is replaced by a regular file.
        let elsewhere = out.join("target.bin");
        fs::write(&elsewhere, b"keep").unwrap();
        symlink(&elsewhere, out.join("s.bin")).unwrap();
        assert!(write_output(&out.join("s.bin"), b"x", false, &[]).is_err());
        write_output(&out.join("s.bin"), b"x", true, &[]).unwrap();
        assert_eq!(fs::read(&elsewhere).unwrap(), b"keep");
        assert!(
            !fs::symlink_metadata(out.join("s.bin"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        // No temporary files are left behind.
        let names: Vec<_> = fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().all(|n| !n.contains("tessera-tmp")),
            "{names:?}"
        );
    }

    fn listing(d: &Path) -> Vec<OsString> {
        let mut v: Vec<OsString> = fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn dotdot_after_a_missing_dir_inside_the_game_dir_creates_nothing_there() {
        let (t, game, out) = scratch("mkdir");
        symlink(&game, out.join("gamelink")).unwrap();
        let p = prot(&game);
        let before = listing(&game);
        // `game/sub` does not exist: `sub/..` cancels, so the destination is out/x1.bin,
        // and no directory may be created on the way inside the game dir.
        write_output(&game.join("sub/../../out/x1.bin"), b"1", false, &p).unwrap();
        write_output(&out.join("gamelink/sub2/../../out/x2.bin"), b"2", false, &p).unwrap();
        write_output(&game.join("a/b/../../../out/new/x3.bin"), b"3", true, &p).unwrap();
        assert_eq!(listing(&game), before);
        assert_eq!(fs::read(out.join("x1.bin")).unwrap(), b"1");
        assert_eq!(fs::read(out.join("x2.bin")).unwrap(), b"2");
        assert_eq!(fs::read(out.join("new/x3.bin")).unwrap(), b"3");
        // Refused targets create nothing either.
        assert!(write_output(&game.join("sub/../new/x.bin"), b"x", false, &p).is_err());
        assert!(write_output(&t.0.join("out/../game/new/x.bin"), b"x", false, &p).is_err());
        assert_eq!(listing(&game), before);
    }

    #[test]
    fn protected_dirs_are_matched_by_identity_not_only_by_path() {
        // A bind mount or another letter case on a case-insensitive file system names the
        // game dir by a path the prefix test cannot match; the identity test still does.
        // Neither can be set up without privileges, so the identity test is checked alone.
        let (_t, game, out) = scratch("identity");
        let id = dir_id(&game).unwrap();
        assert!(ancestor_ids(&game.join("new/dir/x.bin")).contains(&id));
        assert!(ancestor_ids(&game.join("x.bin")).contains(&id));
        assert!(!ancestor_ids(&out.join("x.bin")).contains(&id));
        assert!(!ancestor_ids(&out.join("new/x.bin")).contains(&id));
        assert_eq!(dir_id(&out.join("..").join("game")), Some(id));
        assert_eq!(dir_id(&out.join("missing")), None);
    }

    #[test]
    fn force_works_for_long_file_names() {
        let (_t, game, out) = scratch("longname");
        // 240 bytes: legal (NAME_MAX is 255), but too long once wrapped in a temp-file
        // name that embeds it.
        let f = out.join("n".repeat(240));
        write_output(&f, b"one", false, &prot(&game)).unwrap();
        write_output(&f, b"two", true, &prot(&game)).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"two");
    }

    #[test]
    fn new_files_are_created_and_existing_ones_kept() {
        let (_t, game, out) = scratch("create");
        let f = out.join("sub/dir/x.bin");
        write_output(&f, b"one", false, &prot(&game)).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"one");
        let e = write_output(&f, b"two", false, &prot(&game)).unwrap_err();
        assert!(format!("{e:#}").contains("--force"));
        write_output(&f, b"two", true, &prot(&game)).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"two");
        assert!(write_output(&out.join(".."), b"x", false, &[]).is_err());
    }
}
