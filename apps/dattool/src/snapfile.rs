// SPDX-License-Identifier: GPL-3.0-or-later
//! Directory indexes of archives, and the compact on-disk *snapshot* file that stores one.
//!
//! An [`Index`] lists, for every archive of an input, each directory entry's DID, iteration,
//! stored size, write time, stamp and flags, plus optionally a content hash of the inflated
//! payload. `diff` compares two indexes; `snapshot` writes one to a file so a later install
//! can be compared against it without keeping the old archives around.
//!
//! # Snapshot file format (Tessera's own, version 1)
//!
//! All integers are little-endian. The file is a 12-byte uncompressed preamble followed by
//! one zlib stream holding the body; the stream's Adler-32 is verified on read.
//!
//! | Field | Size | Meaning |
//! |---|---|---|
//! | magic | 8 | `TSRSNAP\0` |
//! | version | 2 | 1 |
//! | flags | 2 | bit 0: content hashes present; other bits must be 0 |
//! | body | rest | one zlib stream, at most [`MAX_BODY`] bytes once inflated |
//!
//! The file must end exactly where the zlib stream ends.
//!
//! Body:
//!
//! | Field | Size | Meaning |
//! |---|---|---|
//! | hash algorithm | 1 | 0 = none, 1 = first 16 bytes of SHA-256 of the inflated payload |
//! | source kind | 1 | 0 = install directory, 1 = single archive file |
//! | created | 8 | Unix seconds (UTC) when the snapshot was taken |
//! | tool | str | producing tool and version |
//! | source | str | path the snapshot was taken from |
//! | archive count | 4 | at most [`MAX_ARCHIVES`] |
//! | archives | … | see below, in set order |
//!
//! `str` is a u16 byte length followed by that many UTF-8 bytes (at most [`MAX_STR`]).
//! Each archive is its label (`str`, at most [`MAX_LABEL`] bytes), a flags byte (bit 0: the
//! directory walk stopped early, so the list is incomplete), a u32 record count, then the
//! records in strictly ascending DID order. A record is DID (4), iteration (4), stored
//! size (4), time (4), stamp (2), entry flags (2) — 20 bytes — and, when hashes are present,
//! a state byte (0 = the payload could not be read, 1 = hash follows) and the 16-byte hash.
//! All archives together hold at most [`MAX_RECORDS`] records. The body must end exactly
//! after the last archive.

use std::io::{self, BufRead, BufReader, Read, Take, Write};

use flate2::Compression;
use flate2::bufread::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha2::{Digest, Sha256};

/// File magic of a snapshot.
pub const MAGIC: &[u8; 8] = b"TSRSNAP\0";
/// Snapshot format version written and accepted.
pub const VERSION: u16 = 1;
/// Length of a stored content hash.
pub const HASH_LEN: usize = 16;
/// Largest accepted archive count.
pub const MAX_ARCHIVES: u32 = 4096;
/// Largest accepted archive label, in bytes.
pub const MAX_LABEL: usize = 256;
/// Largest accepted `tool`/`source` string, in bytes.
pub const MAX_STR: usize = 4096;
/// Largest accepted record count over all archives of one snapshot. A full install holds
/// about 3.8 million records; the cap bounds the memory a hostile file can make the
/// decoder use (each decoded record is a [`Rec`]).
pub const MAX_RECORDS: u64 = 1 << 24;
/// Largest accepted inflated body. It is implied by the other limits (it holds
/// [`MAX_RECORDS`] hashed records plus [`MAX_ARCHIVES`] archive headers); the decoder stops
/// inflating once it is exceeded.
pub const MAX_BODY: u64 = 600 << 20;

const FLAG_HASHES: u16 = 0x0001;
const ARCHIVE_INCOMPLETE: u8 = 0x01;
const HASH_ALGO_NONE: u8 = 0;
const HASH_ALGO_SHA256_128: u8 = 1;
/// Length of a record without its hash part.
pub const RECORD_LEN: usize = 20;
/// Initial record capacity reserved per archive; vectors grow as records actually arrive.
const INITIAL_RESERVE: usize = 1 << 16;

/// A content hash: the first [`HASH_LEN`] bytes of SHA-256 over the inflated payload.
pub type Hash = [u8; HASH_LEN];

/// Hashes a payload the way snapshots and `diff --content` do.
pub fn content_hash(payload: &[u8]) -> Hash {
    let d = Sha256::digest(payload);
    let mut h = [0u8; HASH_LEN];
    h.copy_from_slice(&d[..HASH_LEN]);
    h
}

/// Errors decoding a snapshot file.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// Reading the file failed.
    #[error("cannot read snapshot: {0}")]
    Io(#[from] io::Error),
    /// The file does not start with [`MAGIC`].
    #[error("not a snapshot file (bad magic)")]
    BadMagic,
    /// The format version is not [`VERSION`].
    #[error("unsupported snapshot version {0} (this tool reads version {VERSION})")]
    Version(u16),
    /// Unknown flag bits, hash algorithm or source kind.
    #[error("unsupported snapshot feature: {0}")]
    Unsupported(String),
    /// The body inflates to more than [`MAX_BODY`] bytes.
    #[error("snapshot body exceeds {MAX_BODY} bytes")]
    TooLarge,
    /// The archives hold more than [`MAX_RECORDS`] records in total.
    #[error("snapshot claims more than {MAX_RECORDS} records (at least {0})")]
    TooManyRecords(u64),
    /// The body ended before a field.
    #[error("snapshot truncated while reading {0}")]
    Truncated(&'static str),
    /// A structural rule was broken.
    #[error("corrupt snapshot: {0}")]
    Corrupt(String),
}

/// What an index was taken from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Every archive of an install directory.
    Dir,
    /// One archive file.
    Archive,
}

impl SourceKind {
    /// Short name used in output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dir => "install",
            Self::Archive => "archive",
        }
    }
}

/// One directory entry as an index keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rec {
    /// Data identifier.
    pub did: u32,
    /// Per-record revision counter.
    pub iteration: u32,
    /// Stored payload length.
    pub size: u32,
    /// Unix time of the last write.
    pub time: u32,
    /// Small version number ("stamp").
    pub stamp: u16,
    /// Storage flags.
    pub flags: u16,
    /// Content hash, when hashing was requested and the payload could be read.
    pub hash: Option<Hash>,
}

/// The records of one archive, in ascending DID order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveIndex {
    /// Archive label (`general`, `surface_aux_1`, ...).
    pub label: String,
    /// Records in strictly ascending DID order.
    pub recs: Vec<Rec>,
    /// `false` when the directory walk stopped early. The walk visits DIDs in ascending
    /// order, so the list is then reliable only up to its last DID.
    pub complete: bool,
    /// Why the walk stopped, for live inputs (snapshots only record that it stopped).
    pub walk_error: Option<String>,
    /// Records whose payload could not be read for hashing.
    pub unreadable: u64,
}

/// Directory index of a whole input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    /// Path the index was taken from.
    pub source: String,
    /// Install directory or single archive.
    pub source_kind: SourceKind,
    /// Unix seconds when the index was taken.
    pub created: u64,
    /// Producing tool, for snapshots.
    pub tool: String,
    /// Whether records carry content hashes.
    pub hashed: bool,
    /// Archives in set order.
    pub archives: Vec<ArchiveIndex>,
}

impl Index {
    /// Total record count over all archives.
    pub fn records(&self) -> u64 {
        self.archives.iter().map(|a| a.recs.len() as u64).sum()
    }
}

// ---------------------------------------------------------------------------------------
// Encoding

fn put_str(b: &mut Vec<u8>, s: &str, max: usize) {
    // Truncate on a char boundary so the result stays valid UTF-8.
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    b.extend_from_slice(&(end as u16).to_le_bytes());
    b.extend_from_slice(&s.as_bytes()[..end]);
}

/// Serialises an index into snapshot bytes. Archives beyond [`MAX_ARCHIVES`] and records
/// beyond [`MAX_RECORDS`] are dropped (and their archives marked incomplete), so the output
/// always decodes.
pub fn encode(index: &Index) -> io::Result<Vec<u8>> {
    let mut head = Vec::with_capacity(12);
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&VERSION.to_le_bytes());
    let flags = if index.hashed { FLAG_HASHES } else { 0 };
    head.extend_from_slice(&flags.to_le_bytes());

    let mut z = ZlibEncoder::new(head, Compression::default());
    let mut b = Vec::with_capacity(1 << 16);
    b.push(if index.hashed {
        HASH_ALGO_SHA256_128
    } else {
        HASH_ALGO_NONE
    });
    b.push(match index.source_kind {
        SourceKind::Dir => 0,
        SourceKind::Archive => 1,
    });
    b.extend_from_slice(&index.created.to_le_bytes());
    put_str(&mut b, &index.tool, MAX_STR);
    put_str(&mut b, &index.source, MAX_STR);
    let n = index.archives.len().min(MAX_ARCHIVES as usize);
    b.extend_from_slice(&(n as u32).to_le_bytes());
    let mut budget = MAX_RECORDS;
    for a in index.archives.iter().take(n) {
        put_str(&mut b, &a.label, MAX_LABEL);
        let count = (a.recs.len() as u64).min(budget);
        budget -= count;
        let complete = a.complete && count == a.recs.len() as u64;
        b.push(if complete { 0 } else { ARCHIVE_INCOMPLETE });
        b.extend_from_slice(&(count as u32).to_le_bytes());
        for r in a.recs.iter().take(count as usize) {
            b.extend_from_slice(&r.did.to_le_bytes());
            b.extend_from_slice(&r.iteration.to_le_bytes());
            b.extend_from_slice(&r.size.to_le_bytes());
            b.extend_from_slice(&r.time.to_le_bytes());
            b.extend_from_slice(&r.stamp.to_le_bytes());
            b.extend_from_slice(&r.flags.to_le_bytes());
            if index.hashed {
                match &r.hash {
                    Some(h) => {
                        b.push(1);
                        b.extend_from_slice(h);
                    }
                    None => b.push(0),
                }
            }
            if b.len() >= 1 << 20 {
                z.write_all(&b)?;
                b.clear();
            }
        }
    }
    z.write_all(&b)?;
    z.finish()
}

// ---------------------------------------------------------------------------------------
// Decoding

/// The inflated body, read as a stream with a running size check.
struct Body<R: Read> {
    r: BufReader<Take<R>>,
    used: u64,
}

impl<R: Read> Body<R> {
    fn new(inflated: R) -> Self {
        Self {
            r: BufReader::with_capacity(1 << 16, inflated.take(MAX_BODY + 1)),
            used: 0,
        }
    }

    fn limit_hit(&self) -> bool {
        self.r.get_ref().limit() == 0 && self.r.buffer().is_empty()
    }

    fn fill(&mut self, buf: &mut [u8], what: &'static str) -> Result<(), SnapshotError> {
        match self.r.read_exact(buf) {
            Ok(()) => {
                self.used += buf.len() as u64;
                if self.used > MAX_BODY {
                    return Err(SnapshotError::TooLarge);
                }
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Err(if self.limit_hit() {
                SnapshotError::TooLarge
            } else {
                SnapshotError::Truncated(what)
            }),
            Err(e) => Err(SnapshotError::Corrupt(format!(
                "body does not inflate: {e}"
            ))),
        }
    }

    fn u8(&mut self, what: &'static str) -> Result<u8, SnapshotError> {
        let mut b = [0u8; 1];
        self.fill(&mut b, what)?;
        Ok(b[0])
    }

    fn u16(&mut self, what: &'static str) -> Result<u16, SnapshotError> {
        let mut b = [0u8; 2];
        self.fill(&mut b, what)?;
        Ok(u16::from_le_bytes(b))
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, SnapshotError> {
        let mut b = [0u8; 4];
        self.fill(&mut b, what)?;
        Ok(u32::from_le_bytes(b))
    }

    fn u64(&mut self, what: &'static str) -> Result<u64, SnapshotError> {
        let mut b = [0u8; 8];
        self.fill(&mut b, what)?;
        Ok(u64::from_le_bytes(b))
    }

    fn str(&mut self, max: usize, what: &'static str) -> Result<String, SnapshotError> {
        let n = usize::from(self.u16(what)?);
        if n > max {
            return Err(SnapshotError::Corrupt(format!(
                "{what} is {n} bytes (limit {max})"
            )));
        }
        let mut s = vec![0u8; n];
        self.fill(&mut s, what)?;
        String::from_utf8(s).map_err(|_| SnapshotError::Corrupt(format!("{what} is not UTF-8")))
    }

    /// Requires the zlib stream to end here. Reading to the stream's end also verifies its
    /// Adler-32.
    fn expect_end(&mut self) -> Result<(), SnapshotError> {
        let mut b = [0u8; 1];
        loop {
            return match self.r.read(&mut b) {
                Ok(0) if self.limit_hit() => Err(SnapshotError::TooLarge),
                Ok(0) => Ok(()),
                Ok(_) => Err(SnapshotError::Corrupt(
                    "trailing bytes after the last archive".into(),
                )),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    Err(SnapshotError::Truncated("zlib stream"))
                }
                Err(e) => Err(SnapshotError::Corrupt(format!(
                    "body does not inflate: {e}"
                ))),
            };
        }
    }
}

/// Whether `bytes` starts with the snapshot magic.
pub fn looks_like_snapshot(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Decodes snapshot bytes. Never panics; see [`decode_from`] for the memory bound.
pub fn decode(bytes: &[u8]) -> Result<Index, SnapshotError> {
    decode_from(bytes)
}

/// Reads as many bytes as are available, up to `buf.len()`.
fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

/// Decodes a snapshot from a stream (a file, or a byte slice). The body is inflated and
/// parsed incrementally, never buffered whole: memory is bounded by [`MAX_RECORDS`]
/// decoded records, whatever the file claims. The stream must end exactly where the zlib
/// stream ends. Never panics.
pub fn decode_from<R: BufRead>(mut r: R) -> Result<Index, SnapshotError> {
    let mut pre = [0u8; 12];
    let got = read_up_to(&mut r, &mut pre)?;
    if got < MAGIC.len() || &pre[..MAGIC.len()] != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    if got < 10 {
        return Err(SnapshotError::Truncated("version"));
    }
    let version = u16::from_le_bytes([pre[8], pre[9]]);
    if version != VERSION {
        return Err(SnapshotError::Version(version));
    }
    if got < 12 {
        return Err(SnapshotError::Truncated("flags"));
    }
    let flags = u16::from_le_bytes([pre[10], pre[11]]);
    if flags & !FLAG_HASHES != 0 {
        return Err(SnapshotError::Unsupported(format!("flags {flags:#06x}")));
    }
    let hashed = flags & FLAG_HASHES != 0;

    let mut z = ZlibDecoder::new(r);
    let index = {
        let mut body = Body::new(&mut z);
        let index = decode_body(&mut body, hashed)?;
        body.expect_end()?;
        index
    };
    let mut rest = z.into_inner();
    let left = rest.fill_buf()?.len();
    if left > 0 {
        return Err(SnapshotError::Corrupt(format!(
            "trailing bytes after the zlib stream (at least {left})"
        )));
    }
    Ok(index)
}

fn decode_body<R: Read>(c: &mut Body<R>, hashed: bool) -> Result<Index, SnapshotError> {
    let algo = c.u8("hash algorithm")?;
    match (hashed, algo) {
        (false, HASH_ALGO_NONE) | (true, HASH_ALGO_SHA256_128) => {}
        _ => {
            return Err(SnapshotError::Unsupported(format!(
                "hash algorithm {algo} with hashes {}",
                if hashed { "present" } else { "absent" }
            )));
        }
    }
    let source_kind = match c.u8("source kind")? {
        0 => SourceKind::Dir,
        1 => SourceKind::Archive,
        k => return Err(SnapshotError::Unsupported(format!("source kind {k}"))),
    };
    let created = c.u64("created")?;
    let tool = c.str(MAX_STR, "tool")?;
    let source = c.str(MAX_STR, "source")?;
    let n = c.u32("archive count")?;
    if n > MAX_ARCHIVES {
        return Err(SnapshotError::Corrupt(format!(
            "{n} archives (limit {MAX_ARCHIVES})"
        )));
    }
    let rec_len = RECORD_LEN + usize::from(hashed);
    let mut total: u64 = 0;
    let mut archives: Vec<ArchiveIndex> = Vec::with_capacity((n as usize).min(64));
    for _ in 0..n {
        let label = c.str(MAX_LABEL, "archive label")?;
        let aflags = c.u8("archive flags")?;
        if aflags & !ARCHIVE_INCOMPLETE != 0 {
            return Err(SnapshotError::Unsupported(format!(
                "archive flags {aflags:#04x}"
            )));
        }
        if archives
            .iter()
            .any(|a| a.label.eq_ignore_ascii_case(&label))
        {
            return Err(SnapshotError::Corrupt(format!(
                "archive {label} listed twice"
            )));
        }
        let count = c.u32("record count")?;
        total += u64::from(count);
        if total > MAX_RECORDS {
            return Err(SnapshotError::TooManyRecords(total));
        }
        // Reserve only a little up front: the count is untrusted until the records arrive.
        let mut recs: Vec<Rec> = Vec::with_capacity((count as usize).min(INITIAL_RESERVE));
        let mut unreadable = 0;
        let mut buf = [0u8; RECORD_LEN + 1];
        for _ in 0..count {
            c.fill(&mut buf[..rec_len], "record")?;
            let u32_at =
                |i: usize| u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
            let did = u32_at(0);
            let hash = if hashed {
                match buf[RECORD_LEN] {
                    0 => {
                        unreadable += 1;
                        None
                    }
                    1 => {
                        let mut h = [0u8; HASH_LEN];
                        c.fill(&mut h, "hash")?;
                        Some(h)
                    }
                    s => return Err(SnapshotError::Corrupt(format!("hash state {s}"))),
                }
            } else {
                None
            };
            if let Some(prev) = recs.last().map(|r| r.did)
                && prev >= did
            {
                return Err(SnapshotError::Corrupt(format!(
                    "{label}: DIDs not strictly ascending ({prev:#010x} then {did:#010x})"
                )));
            }
            recs.push(Rec {
                did,
                iteration: u32_at(4),
                size: u32_at(8),
                time: u32_at(12),
                stamp: u16::from_le_bytes([buf[16], buf[17]]),
                flags: u16::from_le_bytes([buf[18], buf[19]]),
                hash,
            });
        }
        archives.push(ArchiveIndex {
            label,
            recs,
            complete: aflags & ARCHIVE_INCOMPLETE == 0,
            walk_error: None,
            unreadable,
        });
    }
    Ok(Index {
        source,
        source_kind,
        created,
        tool,
        hashed,
        archives,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(did: u32, iteration: u32, hash: Option<Hash>) -> Rec {
        Rec {
            did,
            iteration,
            size: did % 997,
            time: 1_700_000_000 + did % 50,
            stamp: 39,
            flags: 2,
            hash,
        }
    }

    fn sample(hashed: bool) -> Index {
        let h = |x: u8| if hashed { Some([x; HASH_LEN]) } else { None };
        Index {
            source: "/somewhere/game".into(),
            source_kind: SourceKind::Dir,
            created: 1_750_000_000,
            tool: "tessera-dattool test".into(),
            hashed,
            archives: vec![
                ArchiveIndex {
                    label: "general".into(),
                    recs: vec![rec(0x0100_0001, 1, h(1)), rec(0x0100_0002, 2, None)],
                    complete: true,
                    walk_error: None,
                    unreadable: u64::from(hashed),
                },
                ArchiveIndex {
                    label: "surface_aux_1".into(),
                    recs: (0..500).map(|i| rec(0x4100_0000 + i, i, h(7))).collect(),
                    complete: false,
                    walk_error: None,
                    unreadable: 0,
                },
                ArchiveIndex {
                    label: "empty".into(),
                    recs: vec![],
                    complete: true,
                    walk_error: None,
                    unreadable: 0,
                },
            ],
        }
    }

    #[test]
    fn round_trips_with_and_without_hashes() {
        for hashed in [false, true] {
            let i = sample(hashed);
            let bytes = encode(&i).unwrap();
            assert!(looks_like_snapshot(&bytes));
            assert_eq!(decode(&bytes).unwrap(), i);
        }
    }

    #[test]
    fn limits_are_consistent() {
        // MAX_BODY must hold the largest body the other limits allow.
        let header = 1 + 1 + 8 + 2 * (2 + MAX_STR as u64) + 4;
        let archive = 2 + MAX_LABEL as u64 + 1 + 4;
        let need =
            header + u64::from(MAX_ARCHIVES) * archive + MAX_RECORDS * (RECORD_LEN as u64 + 17);
        assert!(need <= MAX_BODY, "{need} > {MAX_BODY}");
    }

    #[test]
    fn content_hash_is_truncated_sha256() {
        // SHA-256("abc") = ba7816bf 8f01cfea 414140de 5dae2223 ...
        assert_eq!(
            content_hash(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23
            ]
        );
    }

    fn rewrap(body: &[u8], flags: u16) -> Vec<u8> {
        let mut head = Vec::new();
        head.extend_from_slice(MAGIC);
        head.extend_from_slice(&VERSION.to_le_bytes());
        head.extend_from_slice(&flags.to_le_bytes());
        let mut z = ZlibEncoder::new(head, Compression::fast());
        z.write_all(body).unwrap();
        z.finish().unwrap()
    }

    fn body_of(bytes: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        ZlibDecoder::new(&bytes[12..]).read_to_end(&mut b).unwrap();
        b
    }

    #[test]
    fn header_errors() {
        let good = encode(&sample(false)).unwrap();
        assert!(matches!(decode(b""), Err(SnapshotError::BadMagic)));
        assert!(matches!(decode(b"TSRSNAP"), Err(SnapshotError::BadMagic)));
        assert!(matches!(
            decode(b"XXXXXXXXXXXX"),
            Err(SnapshotError::BadMagic)
        ));
        assert!(matches!(
            decode(b"TSRSNAP\0\x01"),
            Err(SnapshotError::Truncated("version"))
        ));
        assert!(matches!(
            decode(b"TSRSNAP\0\x01\0\0"),
            Err(SnapshotError::Truncated("flags"))
        ));
        let mut v = good.clone();
        v[8] = 2;
        assert!(matches!(decode(&v), Err(SnapshotError::Version(2))));
        let mut v = good.clone();
        v[10] = 0x80;
        assert!(matches!(decode(&v), Err(SnapshotError::Unsupported(_))));
        // Flags claim hashes but the body says none.
        let mut v = good.clone();
        v[10] = 1;
        assert!(matches!(decode(&v), Err(SnapshotError::Unsupported(_))));
    }

    /// Offset of the first archive's record count in a [`sample`] body.
    fn first_count_at(b: &[u8]) -> (usize, usize) {
        // algo(1) kind(1) created(8) tool(2+n) source(2+n) count(4) label(2+7) flags(1).
        let tool_len = usize::from(u16::from_le_bytes([b[10], b[11]]));
        let src_at = 12 + tool_len;
        let src_len = usize::from(u16::from_le_bytes([b[src_at], b[src_at + 1]]));
        let n_at = src_at + 2 + src_len;
        (n_at, n_at + 4 + 2 + "general".len() + 1)
    }

    #[test]
    fn structural_errors() {
        let good = encode(&sample(false)).unwrap();
        let body = body_of(&good);
        // Trailing byte in the body.
        let mut b = body.clone();
        b.push(0);
        assert!(matches!(
            decode(&rewrap(&b, 0)),
            Err(SnapshotError::Corrupt(_))
        ));
        // Truncated body at every length must fail cleanly, never panic.
        for n in 0..body.len() {
            assert!(decode(&rewrap(&body[..n], 0)).is_err(), "len {n}");
        }
        let (n_at, count_at) = first_count_at(&body);
        // A huge record count is refused before anything is reserved for it.
        let mut b = body.clone();
        b[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            decode(&rewrap(&b, 0)),
            Err(SnapshotError::TooManyRecords(_))
        ));
        // A count within the cap that the body cannot hold fails as truncated.
        let mut b = body.clone();
        b[count_at..count_at + 4].copy_from_slice(&1_000_000u32.to_le_bytes());
        assert!(matches!(
            decode(&rewrap(&b, 0)),
            Err(SnapshotError::Truncated(_) | SnapshotError::Corrupt(_))
        ));
        // Out-of-order DIDs.
        let mut b = body.clone();
        let first = count_at + 4;
        let second = first + RECORD_LEN;
        let d1: [u8; 4] = b[first..first + 4].try_into().unwrap();
        b[second..second + 4].copy_from_slice(&d1);
        assert!(matches!(
            decode(&rewrap(&b, 0)),
            Err(SnapshotError::Corrupt(_))
        ));
        // Too many archives.
        let mut b = body.clone();
        b[n_at..n_at + 4].copy_from_slice(&(MAX_ARCHIVES + 1).to_le_bytes());
        assert!(matches!(
            decode(&rewrap(&b, 0)),
            Err(SnapshotError::Corrupt(_))
        ));
    }

    #[test]
    fn bytes_after_the_zlib_stream_are_rejected() {
        let mut v = encode(&sample(true)).unwrap();
        assert!(decode(&v).is_ok());
        v.extend_from_slice(b"GARBAGE-APPENDED-AFTER-STREAM");
        let e = decode(&v).unwrap_err();
        assert!(
            matches!(&e, SnapshotError::Corrupt(m) if m.contains("after the zlib stream")),
            "{e}"
        );
        // One stray byte is enough.
        let mut v = encode(&sample(false)).unwrap();
        v.push(0);
        assert!(matches!(decode(&v), Err(SnapshotError::Corrupt(_))));
    }

    #[test]
    fn a_body_beyond_the_record_cap_is_refused_without_reading_it() {
        // Two archives whose counts sum past MAX_RECORDS: refused at the second count, with
        // no record data present at all.
        let mut b = vec![HASH_ALGO_NONE, 0];
        b.extend_from_slice(&0u64.to_le_bytes());
        put_str(&mut b, "t", MAX_STR);
        put_str(&mut b, "s", MAX_STR);
        b.extend_from_slice(&2u32.to_le_bytes());
        put_str(&mut b, "a", MAX_LABEL);
        b.push(0);
        b.extend_from_slice(&0u32.to_le_bytes());
        put_str(&mut b, "b", MAX_LABEL);
        b.push(0);
        b.extend_from_slice(&((MAX_RECORDS + 1) as u32).to_le_bytes());
        assert!(matches!(
            decode(&rewrap(&b, 0)),
            Err(SnapshotError::TooManyRecords(_))
        ));
    }

    #[test]
    fn a_zlib_bomb_is_not_inflated_whole() {
        // A valid preamble, then 64 MiB of zeros: the body parses as zero archives and
        // decoding stops at the first excess byte instead of inflating everything.
        let mut head = Vec::new();
        head.extend_from_slice(MAGIC);
        head.extend_from_slice(&VERSION.to_le_bytes());
        head.extend_from_slice(&0u16.to_le_bytes());
        let mut z = ZlibEncoder::new(head, Compression::best());
        let zeros = vec![0u8; 1 << 20];
        for _ in 0..64 {
            z.write_all(&zeros).unwrap();
        }
        let bytes = z.finish().unwrap();
        // Zero archives, then trailing body bytes.
        assert!(matches!(decode(&bytes), Err(SnapshotError::Corrupt(_))));
    }

    #[test]
    fn hostile_bytes_never_panic() {
        let good = encode(&sample(true)).unwrap();
        let body = body_of(&good);
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..2000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut b = body.clone();
            let at = (seed as usize) % b.len();
            b[at] ^= (seed >> 32) as u8 | 1;
            let _ = decode(&rewrap(&b, FLAG_HASHES));
            let mut f = good.clone();
            let at = (seed as usize) % f.len();
            f[at] ^= (seed >> 40) as u8 | 1;
            let _ = decode(&f);
        }
        for n in 0..good.len() {
            assert!(decode(&good[..n]).is_err(), "prefix of {n} bytes decoded");
        }
    }

    #[test]
    fn long_strings_are_truncated_on_char_boundaries() {
        let mut i = sample(false);
        i.source = "é".repeat(MAX_STR);
        let back = decode(&encode(&i).unwrap()).unwrap();
        assert!(back.source.len() <= MAX_STR);
        assert!(back.source.chars().all(|c| c == 'é'));
    }
}
