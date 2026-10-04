// SPDX-License-Identifier: GPL-3.0-or-later
//! `--screenshot` support: PNG encoding of a captured frame, refusing to write inside the
//! game install.
//!
//! The guard works on the path the operating system would actually write to: a relative
//! output path is taken against the working directory, the directory it names is
//! resolved through symbolic links (it must exist, as the PNG is created in it), and an
//! existing output file that is a symbolic link is followed. Only then is it compared
//! with the (equally resolved) install directories.

use std::io::BufWriter;
use std::path::{Path, PathBuf};

use eframe::egui::ColorImage;

use crate::error::ExplorerError;

/// `p` made absolute against `cwd` (without touching the file system).
fn absolute_in(p: &Path, cwd: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// The file `path` (relative paths taken against `cwd`) would be written to: its
/// directory resolved through symbolic links, and the file itself followed when it is an
/// existing symbolic link.
///
/// # Errors
///
/// When `path` names no file (it ends in `..` or is a root), its directory does not exist
/// or cannot be resolved, it is a directory, or it is a symbolic link that leads nowhere.
pub fn resolve_output(path: &Path, cwd: &Path) -> Result<PathBuf, ExplorerError> {
    let abs = absolute_in(path, cwd);
    let (Some(parent), Some(name)) = (abs.parent(), abs.file_name()) else {
        return Err(ExplorerError::Output(format!(
            "{} does not name a file",
            path.display()
        )));
    };
    let parent = parent
        .canonicalize()
        .map_err(|e| ExplorerError::Output(format!("{}: {e}", parent.display())))?;
    let target = parent.join(name);
    match std::fs::symlink_metadata(&target) {
        Ok(m) if m.file_type().is_symlink() => target.canonicalize().map_err(|e| {
            ExplorerError::Output(format!(
                "{} is a symbolic link that cannot be followed: {e}",
                path.display()
            ))
        }),
        Ok(m) if m.is_dir() => Err(ExplorerError::Output(format!(
            "{} is a directory",
            path.display()
        ))),
        _ => Ok(target),
    }
}

/// `dir` (relative paths taken against `cwd`) resolved through symbolic links as far as
/// it exists; the missing rest is appended unchanged.
fn resolve_dir(dir: &Path, cwd: &Path) -> PathBuf {
    let abs = absolute_in(dir, cwd);
    let mut existing = abs.clone();
    let mut rest = Vec::new();
    while existing.canonicalize().is_err() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent.to_path_buf();
            }
            // Nothing resolves (or a `..` ends the path): compare it as given.
            _ => return abs,
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for r in rest.iter().rev() {
        out.push(r);
    }
    out
}

/// Whether writing `path` would create a file inside `dir`, with relative paths taken
/// against `cwd`. An output path that cannot be resolved (see [`resolve_output`]) is
/// reported as inside, so that callers refuse it.
#[must_use]
pub fn is_inside_in(path: &Path, dir: &Path, cwd: &Path) -> bool {
    match resolve_output(path, cwd) {
        Ok(target) => target.starts_with(resolve_dir(dir, cwd)),
        Err(_) => true,
    }
}

/// [`is_inside_in`] against the process's working directory. When the working
/// directory is unknown, only absolute paths can be checked; relative ones are reported
/// as inside.
#[must_use]
pub fn is_inside(path: &Path, dir: &Path) -> bool {
    match std::env::current_dir() {
        Ok(cwd) => is_inside_in(path, dir, &cwd),
        Err(_) => !path.is_absolute() || is_inside_in(path, dir, Path::new("/")),
    }
}

/// Writes `image` (premultiplied RGBA, as captured) to `path` as an 8-bit RGBA PNG and
/// returns the file actually written (resolved as by [`resolve_output`]).
///
/// Refuses any path that would land inside one of the `guards` (the game install
/// directories: the install is read-only).
///
/// # Errors
///
/// [`ExplorerError::Output`] when the path is refused or cannot be resolved, or writing
/// fails.
pub fn save_png(
    path: &Path,
    image: &ColorImage,
    guards: &[PathBuf],
) -> Result<PathBuf, ExplorerError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExplorerError::Output(format!("working directory: {e}")))?;
    save_png_in(path, image, guards, &cwd)
}

/// [`save_png`] with relative paths taken against `cwd`.
fn save_png_in(
    path: &Path,
    image: &ColorImage,
    guards: &[PathBuf],
    cwd: &Path,
) -> Result<PathBuf, ExplorerError> {
    let target = resolve_output(path, cwd)?;
    for dir in guards {
        if target.starts_with(resolve_dir(dir, cwd)) {
            return Err(ExplorerError::Output(format!(
                "refusing to write {} inside the game install {}",
                path.display(),
                dir.display()
            )));
        }
    }
    let [w, h] = image.size;
    let (w32, h32) = (
        u32::try_from(w).map_err(|_| ExplorerError::Output("image too wide".into()))?,
        u32::try_from(h).map_err(|_| ExplorerError::Output("image too tall".into()))?,
    );
    let mut rgba = Vec::with_capacity(image.pixels.len().saturating_mul(4));
    for p in &image.pixels {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    let file = std::fs::File::create(&target)
        .map_err(|e| ExplorerError::Output(format!("{}: {e}", target.display())))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w32, h32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc
        .write_header()
        .map_err(|e| ExplorerError::Output(e.to_string()))?;
    writer
        .write_image_data(&rgba)
        .map_err(|e| ExplorerError::Output(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| ExplorerError::Output(e.to_string()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    struct Tmp(PathBuf);

    impl Tmp {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "tessera-explorer-shot-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir.canonicalize().unwrap())
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn img() -> ColorImage {
        ColorImage::new([2, 1], vec![Color32::RED, Color32::from_rgb(0, 0, 255)])
    }

    #[test]
    fn writes_png_and_guards_install() {
        let tmp = Tmp::new("abs");
        let dir = &tmp.0;
        let out = dir.join("a.png");
        let written = save_png_in(&out, &img(), &[], dir).unwrap();
        assert_eq!(written, out);
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
        let fake_install = dir.join("install");
        std::fs::create_dir_all(fake_install.join("sub")).unwrap();
        let inside = fake_install.join("sub").join("x.png");
        assert!(is_inside_in(&inside, &fake_install, dir));
        assert!(!is_inside_in(&out, &fake_install, dir));
        let guards = [fake_install.clone()];
        assert!(save_png_in(&inside, &img(), &guards, dir).is_err());
        assert!(!inside.exists());
        // A missing directory cannot be written to at all.
        let missing = fake_install.join("nope").join("x.png");
        assert!(save_png_in(&missing, &img(), &[], dir).is_err());
        assert!(is_inside_in(&missing, &fake_install, dir));
    }

    /// Regression: a bare file name with the working directory inside the install was
    /// let through (its parent is the empty path, which never resolved).
    #[test]
    fn relative_paths_are_taken_against_the_working_directory() {
        let tmp = Tmp::new("rel");
        let install = tmp.0.join("install");
        std::fs::create_dir_all(install.join("sub")).unwrap();
        let guards = [install.clone()];
        for rel in ["shot.png", "./shot.png", "sub/x.png", "sub/../y.png"] {
            assert!(
                is_inside_in(Path::new(rel), &install, &install),
                "{rel} with cwd = install"
            );
            assert!(
                save_png_in(Path::new(rel), &img(), &guards, &install).is_err(),
                "{rel}"
            );
        }
        assert!(!install.join("shot.png").exists());
        assert!(!install.join("sub").join("x.png").exists());
        // `..` out of the install is fine.
        let out = save_png_in(Path::new("../out.png"), &img(), &guards, &install).unwrap();
        assert_eq!(out, tmp.0.join("out.png"));
        // A relative guard is taken against the working directory as well.
        assert!(is_inside_in(
            &install.join("z.png"),
            Path::new("install"),
            &tmp.0
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_followed() {
        let tmp = Tmp::new("link");
        let install = tmp.0.join("install");
        let outside = tmp.0.join("outside");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let guards = [install.clone()];
        // A directory link into the install.
        std::os::unix::fs::symlink(&install, outside.join("dirlink")).unwrap();
        let via_dir = outside.join("dirlink").join("x.png");
        assert!(save_png_in(&via_dir, &img(), &guards, &tmp.0).is_err());
        // A file link into the install, existing target and dangling target.
        std::fs::write(install.join("existing.png"), b"keep").unwrap();
        std::os::unix::fs::symlink(install.join("existing.png"), outside.join("f.png")).unwrap();
        assert!(save_png_in(&outside.join("f.png"), &img(), &guards, &tmp.0).is_err());
        assert_eq!(
            std::fs::read(install.join("existing.png")).unwrap(),
            b"keep"
        );
        std::os::unix::fs::symlink(install.join("new.png"), outside.join("d.png")).unwrap();
        assert!(save_png_in(&outside.join("d.png"), &img(), &guards, &tmp.0).is_err());
        assert!(!install.join("new.png").exists());
    }

    #[test]
    fn non_file_paths_are_refused() {
        let tmp = Tmp::new("odd");
        assert!(resolve_output(Path::new("/"), &tmp.0).is_err());
        assert!(resolve_output(Path::new(".."), &tmp.0).is_err());
        assert!(resolve_output(&tmp.0, &tmp.0).is_err());
    }
}
