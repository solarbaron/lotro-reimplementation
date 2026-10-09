// SPDX-License-Identifier: GPL-3.0-or-later
//! The cell-owner index of the collision walk (`walker.md` §13.3 OPEN of the 2026-10-06
//! review: "a world-side index of the cell owners whose cell bounds reach each landblock,
//! built from landblock info").
//!
//! A cell's physics mesh belongs to its owner landblock (the landblock info that lists it,
//! `landblock.md` §4.4), and a cell's origin may lie several landblocks from that owner
//! (`walker.md` §13.3, row W-C1: up to 6 in this install). Before the mover has entered
//! such a cell the walker cannot name its owner from the cell, so a step's view could miss
//! a floor the mover is about to step onto. This index names those owners in advance.
//!
//! **Tessera rules** (engineering choices, recorded as an OPEN note in `walker.md` §13.3):
//!
//! - The index is built **around the mover**, not for the whole install at boot: reading
//!   every landblock-info record of an install takes tens of seconds (about 1 GB of
//!   records), so the landblock-info records within [`COVER_RADIUS`] landblocks
//!   (chessboard distance, one region) of the feet's landblock are read on a background
//!   thread as the mover travels and kept while they stay within [`KEEP_RADIUS`].
//! - A cell **reaches** the landblock that contains its origin (the owner's origin plus
//!   the cell's x, y offset; a cell without an offset lies at the owner's origin,
//!   `landblock.md` §4.4, L42) and its eight neighbours ([`HULL_MARGIN`]): the hull is not
//!   read here (it is in the cell's 0x01 geometry, which the collision set reads once the
//!   owner is resident; its exact bounds then decide what the view holds).
//! - The owners a step needs ([`CellOwnerIndex::owners`]) are those of the cells reaching
//!   the feet's landblock or one of its eight neighbours, other than those landblocks
//!   themselves (a step needs them anyway), nearest first, at most [`MAX_OWNERS`]. While
//!   one landblock within [`COVER_RADIUS`] is not read yet the answer is unknown (`None`)
//!   and the step waits, so a step's view never depends on how far the reading has got.
//! - [`COVER_RADIUS`] is the reach (feet square ± 1, hull margin 1) plus
//!   [`MAX_OWNER_DISTANCE`]: an owner further than that from a cell's origin is not found
//!   (none in this install).
//! - A record that is missing or does not decode has no cells (a data fault, final); a
//!   read that fails for a reason outside the data is retried after [`RETRY_AFTER`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use tessera_formats::LandblockInfo;
use tessera_formats::landblock::Cell;
use tessera_world::{LandblockKey, WorldContext, WorldPos};

/// Largest chessboard distance, in landblocks, between a cell's origin and its owner that
/// the index finds (the largest in this install is 6, `walker.md` §13.3 row W-C1).
pub const MAX_OWNER_DISTANCE: u32 = 6;
/// How far, in landblocks, a cell is taken to reach beyond the landblock of its origin.
pub const HULL_MARGIN: u32 = 1;
/// The landblock-info records within this chessboard distance of the feet's landblock are
/// read before the index answers: the feet's square (1), the hull margin and
/// [`MAX_OWNER_DISTANCE`].
pub const COVER_RADIUS: u32 = 1 + HULL_MARGIN + MAX_OWNER_DISTANCE;
/// Records further than this from the feet's landblock are forgotten once the index holds
/// more than [`MAX_KEPT`] of them.
pub const KEEP_RADIUS: u32 = 2 * COVER_RADIUS;
/// Most records the index keeps before it forgets the far ones.
pub const MAX_KEPT: usize = 4096;
/// Most owners [`CellOwnerIndex::owners`] names (the nearest).
pub const MAX_OWNERS: usize = 8;
/// A read that failed for a reason outside the data is asked again after this long.
pub const RETRY_AFTER: Duration = Duration::from_millis(500);

/// What reading one landblock's info gives the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerRead {
    /// The landblocks that hold the origins of the record's cells (empty: no cells, no
    /// record, or a record that does not decode).
    Origins(Vec<LandblockKey>),
    /// The read failed for a reason outside the data; ask again later.
    Transient,
}

/// Reads one landblock's info for the index (runs on the index's thread).
pub type OwnerReader = Arc<dyn Fn(LandblockKey) -> OwnerRead + Send + Sync>;

/// The landblocks holding the origins of `cells` (the cells of `owner`'s landblock info):
/// the owner's origin plus each cell's x, y offset (a cell without an offset at the owner's
/// origin); distinct, in the order met; origins off the grid are left out.
#[must_use]
pub fn cell_origins(owner: LandblockKey, cells: &[Cell]) -> Vec<LandblockKey> {
    let o = owner.origin();
    let mut out: Vec<LandblockKey> = Vec::new();
    for c in cells {
        let [x, y] = c
            .position
            .offset()
            .map_or([0.0, 0.0], |p| [f64::from(p[0]), f64::from(p[1])]);
        let at = WorldPos::new(owner.region(), o[0] + x, o[1] + y, 0.0);
        if let Some(k) = at.landblock()
            && !out.contains(&k)
        {
            out.push(k);
        }
    }
    out
}

/// The reader over a world context's source: the landblock info of each key, decoded with
/// the context's schema ([`cell_origins`]).
#[must_use]
pub fn context_reader(ctx: Arc<WorldContext>) -> OwnerReader {
    Arc::new(move |key: LandblockKey| {
        let did = key.info_did();
        match ctx.source().read_stamped(did) {
            Ok(None) => OwnerRead::Origins(Vec::new()),
            Ok(Some((bytes, stamp))) => {
                match LandblockInfo::parse_expecting(&bytes, did, stamp, ctx.schema()) {
                    Ok(info) => OwnerRead::Origins(cell_origins(key, info.cells())),
                    Err(e) => {
                        tracing::debug!(%key, "cell-owner index: landblock info unusable: {e}");
                        OwnerRead::Origins(Vec::new())
                    }
                }
            }
            Err(e) if e.is_data_error() => {
                tracing::debug!(%key, "cell-owner index: landblock info unreadable: {e}");
                OwnerRead::Origins(Vec::new())
            }
            Err(e) => {
                tracing::debug!(%key, "cell-owner index: read failed, retrying: {e}");
                OwnerRead::Transient
            }
        }
    })
}

/// The index's link to its reading thread: questions out, answers back.
type Channel = (Sender<LandblockKey>, Receiver<(LandblockKey, OwnerRead)>);

/// The cell-owner index (module docs).
pub struct CellOwnerIndex {
    reader: OwnerReader,
    channel: Option<Channel>,
    /// Owner → the landblocks holding its cells' origins.
    known: HashMap<LandblockKey, Arc<[LandblockKey]>>,
    /// Origin landblock → the owners with a cell there.
    by_origin: HashMap<LandblockKey, Vec<LandblockKey>>,
    /// Asked and not answered yet.
    asked: HashSet<LandblockKey>,
    /// Failed outside the data: ask again from then on.
    retry: HashMap<LandblockKey, Instant>,
}

impl std::fmt::Debug for CellOwnerIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellOwnerIndex")
            .field("known", &self.known.len())
            .field("asked", &self.asked.len())
            .finish_non_exhaustive()
    }
}

impl CellOwnerIndex {
    /// An index reading with `reader` on its own thread (started on first need).
    #[must_use]
    pub fn new(reader: OwnerReader) -> Self {
        Self {
            reader,
            channel: None,
            known: HashMap::new(),
            by_origin: HashMap::new(),
            asked: HashSet::new(),
            retry: HashMap::new(),
        }
    }

    /// An index over a world context's landblock info ([`context_reader`]).
    #[must_use]
    pub fn from_context(ctx: &Arc<WorldContext>) -> Self {
        Self::new(context_reader(Arc::clone(ctx)))
    }

    /// Records read so far.
    #[must_use]
    pub fn known(&self) -> usize {
        self.known.len()
    }

    fn channel(&mut self) -> Option<&Channel> {
        if self.channel.is_none() {
            let (ask_tx, ask_rx) = channel::<LandblockKey>();
            let (done_tx, done_rx) = channel::<(LandblockKey, OwnerRead)>();
            let reader = Arc::clone(&self.reader);
            let spawned = std::thread::Builder::new()
                .name("walk-cell-owners".into())
                .spawn(move || {
                    // Ends when the index (the sender) is dropped or stops listening.
                    while let Ok(key) = ask_rx.recv() {
                        let read =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reader(key)))
                                .unwrap_or(OwnerRead::Transient);
                        if done_tx.send((key, read)).is_err() {
                            break;
                        }
                    }
                });
            match spawned {
                Ok(_) => self.channel = Some((ask_tx, done_rx)),
                Err(e) => {
                    tracing::warn!("cell-owner index thread not started: {e}");
                    return None;
                }
            }
        }
        self.channel.as_ref()
    }

    /// Asks for every record within [`COVER_RADIUS`] of `feet` not read yet, takes in the
    /// answers that arrived, and forgets far records once more than [`MAX_KEPT`] are held.
    pub fn update(&mut self, feet: LandblockKey) {
        let now = Instant::now();
        let r = i32::try_from(COVER_RADIUS).unwrap_or(8);
        let mut wanted: Vec<LandblockKey> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                let Some(k) = feet.offset(dx, dy) else {
                    continue;
                };
                if self.known.contains_key(&k) || self.asked.contains(&k) {
                    continue;
                }
                if self.retry.get(&k).is_some_and(|t| now < *t) {
                    continue;
                }
                wanted.push(k);
            }
        }
        // Nearest first.
        wanted.sort_by_key(|k| (k.chebyshev(feet), *k));
        if let Some((tx, _)) = self.channel() {
            let tx = tx.clone();
            for k in wanted {
                if tx.send(k).is_err() {
                    break;
                }
                self.asked.insert(k);
            }
        }
        let answers: Vec<(LandblockKey, OwnerRead)> = self
            .channel
            .as_ref()
            .map(|(_, rx)| rx.try_iter().collect())
            .unwrap_or_default();
        for (k, read) in answers {
            self.asked.remove(&k);
            match read {
                OwnerRead::Origins(origins) => {
                    self.retry.remove(&k);
                    self.insert(k, origins);
                }
                OwnerRead::Transient => {
                    self.retry.insert(k, now + RETRY_AFTER);
                }
            }
        }
        if self.known.len() > MAX_KEPT {
            let far: Vec<LandblockKey> = self
                .known
                .keys()
                .filter(|k| k.chebyshev(feet).is_none_or(|d| d > KEEP_RADIUS))
                .copied()
                .collect();
            for k in far {
                self.remove(k);
            }
        }
    }

    /// Enters owner `key` with the landblocks of its cells' origins.
    pub fn insert(&mut self, key: LandblockKey, origins: Vec<LandblockKey>) {
        self.remove(key);
        for o in &origins {
            let list = self.by_origin.entry(*o).or_default();
            if !list.contains(&key) {
                list.push(key);
            }
        }
        self.known.insert(key, origins.into());
    }

    fn remove(&mut self, key: LandblockKey) {
        if let Some(origins) = self.known.remove(&key) {
            for o in origins.iter() {
                if let Some(list) = self.by_origin.get_mut(o) {
                    list.retain(|k| *k != key);
                    if list.is_empty() {
                        self.by_origin.remove(o);
                    }
                }
            }
        }
    }

    /// Whether every record within [`COVER_RADIUS`] of `feet` is read.
    #[must_use]
    pub fn covers(&self, feet: LandblockKey) -> bool {
        let r = i32::try_from(COVER_RADIUS).unwrap_or(8);
        (-r..=r).all(|dy| {
            (-r..=r).all(|dx| {
                feet.offset(dx, dy)
                    .is_none_or(|k| self.known.contains_key(&k))
            })
        })
    }

    /// The owners a step at `feet` needs besides the feet's square (module docs): the
    /// owners of cells reaching the feet's landblock or its eight neighbours, other than
    /// those nine, nearest first (then by key), at most [`MAX_OWNERS`]. `None` while the
    /// records within [`COVER_RADIUS`] are not all read.
    #[must_use]
    pub fn owners(&self, feet: LandblockKey) -> Option<Vec<LandblockKey>> {
        if !self.covers(feet) {
            return None;
        }
        let reach = i32::try_from(1 + HULL_MARGIN).unwrap_or(2);
        let mut out: Vec<LandblockKey> = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let Some(origin) = feet.offset(dx, dy) else {
                    continue;
                };
                for owner in self.by_origin.get(&origin).into_iter().flatten() {
                    if owner.chebyshev(feet).is_some_and(|d| d > 1) && !out.contains(owner) {
                        out.push(*owner);
                    }
                }
            }
        }
        out.sort_by_key(|k| (k.chebyshev(feet), *k));
        if out.len() > MAX_OWNERS {
            tracing::debug!(
                feet = %feet,
                owners = out.len(),
                kept = MAX_OWNERS,
                "cell-owner index: more owners than pins; the nearest kept"
            );
            out.truncate(MAX_OWNERS);
        }
        Some(out)
    }

    /// Updates with the landblock of `feet` until the index covers it, or `timeout`
    /// passes; whether it covers it (scripted walks and tests).
    pub fn settle(&mut self, feet: LandblockKey, timeout: Duration) -> bool {
        let started = Instant::now();
        loop {
            self.update(feet);
            if self.covers(feet) {
                return true;
            }
            if started.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::Position;

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).expect("key")
    }

    fn cell(index: u16, offset: Option<[f32; 3]>) -> Cell {
        let mut p = Position::new(Position::REGION | Position::LANDBLOCK | Position::CELL)
            .with_region(1)
            .with_landblock(50, 50)
            .with_cell(index);
        if let Some(o) = offset {
            p = Position::new(
                Position::REGION | Position::LANDBLOCK | Position::CELL | Position::OFFSET,
            )
            .with_region(1)
            .with_landblock(50, 50)
            .with_cell(index)
            .with_offset(o);
        }
        Cell {
            index,
            position: p,
            ..Cell::default()
        }
    }

    #[test]
    fn cell_origins_follow_the_offsets() {
        let owner = key(50, 50);
        let cells = [
            cell(2, Some([10.0, 10.0, -50.0])),
            cell(3, Some([400.0, -200.0, 0.0])),
            cell(4, None),
            cell(5, Some([20.0, 30.0, 0.0])),
        ];
        // 400 m east is two landblocks east, −200 m two south (−200 / 160 → −1.25).
        assert_eq!(
            cell_origins(owner, &cells),
            [key(50, 50), key(52, 48)],
            "distinct, in order; no offset: the owner's origin"
        );
        assert!(cell_origins(owner, &[]).is_empty());
    }

    /// A reader serving fixed origins (every other key: no cells), counting reads.
    fn fixed(
        map: HashMap<LandblockKey, Vec<LandblockKey>>,
        transient_once: Option<LandblockKey>,
    ) -> (OwnerReader, Arc<std::sync::atomic::AtomicUsize>) {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failed = Arc::new(std::sync::Mutex::new(false));
        let r = Arc::clone(&reads);
        let reader: OwnerReader = Arc::new(move |k| {
            r.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if Some(k) == transient_once {
                let mut f = failed.lock().unwrap();
                if !*f {
                    *f = true;
                    return OwnerRead::Transient;
                }
            }
            OwnerRead::Origins(map.get(&k).cloned().unwrap_or_default())
        });
        (reader, reads)
    }

    #[test]
    fn owners_of_far_cells_are_named_before_the_mover_enters_them() {
        let feet = key(100, 100);
        // An owner 5 landblocks west whose cell lies just west of the feet's square
        // (reached through the hull margin); an owner 7 east whose cell lies in the
        // square; an owner 3 north whose cells lie far from the feet; and an owner in the
        // square itself.
        let west = key(95, 100);
        let east = key(107, 101);
        let north = key(100, 103);
        let inside = key(101, 100);
        let map = HashMap::from([
            (west, vec![key(98, 100)]),
            (east, vec![key(101, 101)]),
            (north, vec![key(100, 106)]),
            (inside, vec![key(100, 100)]),
        ]);
        let (reader, reads) = fixed(map, Some(east));
        let mut index = CellOwnerIndex::new(reader);
        assert_eq!(index.owners(feet), None, "nothing read yet");
        assert!(
            index.settle(feet, Duration::from_secs(30)),
            "covers the feet"
        );
        // The first read of `east` failed outside the data and was asked again.
        assert!(
            reads.load(std::sync::atomic::Ordering::Relaxed)
                > ((2 * COVER_RADIUS + 1) * (2 * COVER_RADIUS + 1)) as usize
        );
        assert_eq!(
            index.owners(feet),
            Some(vec![west, east]),
            "nearest first; the square's own owner and the unreached one left out"
        );
        // Moving one landblock east asks only the new column, and the west owner's cell
        // is no longer within reach.
        let before = reads.load(std::sync::atomic::Ordering::Relaxed);
        let moved = key(101, 100);
        assert!(index.settle(moved, Duration::from_secs(30)));
        let after = reads.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(after - before, (2 * COVER_RADIUS + 1) as usize);
        assert_eq!(index.owners(moved), Some(vec![east]));
    }

    #[test]
    fn at_most_max_owners_are_named() {
        let feet = key(100, 100);
        let map: HashMap<LandblockKey, Vec<LandblockKey>> = (0..12u8)
            .map(|i| (key(90 + i, 107), vec![key(100, 101)]))
            .collect();
        let (reader, _) = fixed(map, None);
        let mut index = CellOwnerIndex::new(reader);
        assert!(index.settle(feet, Duration::from_secs(30)));
        let owners = index.owners(feet).expect("covered");
        assert_eq!(owners.len(), MAX_OWNERS);
        assert!(owners.iter().all(|k| k.chebyshev(feet) == Some(7)));
    }

    #[test]
    fn grid_edges_are_covered() {
        // Keys off the grid count as read.
        let feet = key(0, 0);
        let (reader, _) = fixed(HashMap::new(), None);
        let mut index = CellOwnerIndex::new(reader);
        assert!(index.settle(feet, Duration::from_secs(30)));
        assert_eq!(index.owners(feet), Some(Vec::new()));
    }
}
