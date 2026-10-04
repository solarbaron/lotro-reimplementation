// SPDX-License-Identifier: GPL-3.0-or-later
//! Byte sources: a read-only file accessed with positional reads, or an in-memory buffer.

use std::fs::File;
use std::io;
use std::sync::Arc;

/// Where archive bytes come from.
///
/// Positional reads (`pread`) take `&self`, so one source can serve many threads without a
/// lock and without a shared file cursor.
pub(crate) enum Source {
    File(File),
    Memory(Arc<[u8]>),
}

impl Source {
    pub(crate) fn len(&self) -> io::Result<u64> {
        match self {
            Self::File(f) => Ok(f.metadata()?.len()),
            Self::Memory(m) => Ok(m.len() as u64),
        }
    }

    /// Fills `buf` from absolute offset `off`, failing with `UnexpectedEof` past the end.
    pub(crate) fn read_exact_at(&self, buf: &mut [u8], off: u64) -> io::Result<()> {
        match self {
            Self::File(f) => file_read_exact_at(f, buf, off),
            Self::Memory(m) => {
                let start = usize::try_from(off).map_err(|_| eof())?;
                let end = start.checked_add(buf.len()).ok_or_else(eof)?;
                let src = m.get(start..end).ok_or_else(eof)?;
                buf.copy_from_slice(src);
                Ok(())
            }
        }
    }
}

fn eof() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "read past end of data")
}

#[cfg(unix)]
fn file_read_exact_at(f: &File, buf: &mut [u8], off: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    f.read_exact_at(buf, off)
}

#[cfg(windows)]
fn file_read_exact_at(f: &File, mut buf: &mut [u8], mut off: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match f.seek_read(buf, off) {
            Ok(0) => return Err(eof()),
            Ok(n) => {
                buf = &mut buf[n..];
                off += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
