// SPDX-License-Identifier: GPL-3.0-or-later
//! The decoded-image cache (decision 0015 §4.5, U5).
//!
//! Images arrive decoded from `tessera-material::ui_image` (decoded on the asset workers).
//! The cache keeps them as `Arc`s keyed by DID or by path:
//!
//! - **Pinned** images (every image referenced by a displayed element drawn into a canvas)
//!   are never evicted, so a repaint never lacks pixels and partition invariance holds.
//!   Pins are counted; an image stays pinned until every pin is released.
//! - **Unpinned** images stay in an LRU within the decoded-image budget (U5: 64 MiB).
//! - **Hard cap.** When the pinned images would exceed the hard cap (U5: 512 MiB), the pin
//!   is refused with [`CacheError::PinnedCapExceeded`]: the caller draws the placeholder
//!   (`skin-definition.md` §6.5) and records a categorised failure.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_material::ui_image::UiImage;

/// Default budget of unpinned decoded images (U5).
pub const DEFAULT_LRU_BYTES: u64 = 64 * 1024 * 1024;

/// Default hard cap of pinned decoded images (U5).
pub const DEFAULT_PINNED_CAP_BYTES: u64 = 512 * 1024 * 1024;

/// Longest path key, in bytes (longer paths are refused).
pub const MAX_PATH_KEY: usize = 4096;

/// Where an image comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImageKey {
    /// A render surface (0x41) by DID.
    Did(u32),
    /// A skin or plugin file by path, compared as given (callers normalise case and
    /// separators the way their VFS matches them).
    Path(String),
}

/// Why the cache refused a request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CacheError {
    /// Pinning the image would exceed the pinned hard cap.
    #[error("pinned UI images would exceed the hard cap ({pinned} + {size} > {cap} bytes)")]
    PinnedCapExceeded {
        /// Bytes pinned now.
        pinned: u64,
        /// Bytes of the image.
        size: u64,
        /// The cap.
        cap: u64,
    },
    /// The key is not in the cache.
    #[error("UI image not in the cache")]
    Missing,
    /// A path key longer than [`MAX_PATH_KEY`].
    #[error("UI image path key too long ({0} bytes)")]
    KeyTooLong(usize),
}

#[derive(Debug)]
struct Entry {
    image: Arc<UiImage>,
    bytes: u64,
    pins: u32,
    tick: u64,
}

/// Counters of the cache (for `FrameStats` and tests).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Images held.
    pub images: usize,
    /// Bytes of pinned images.
    pub pinned_bytes: u64,
    /// Bytes of unpinned images.
    pub unpinned_bytes: u64,
    /// Images evicted since creation.
    pub evictions: u64,
    /// Pins refused by the hard cap since creation.
    pub refused_pins: u64,
}

/// The decoded-image cache.
#[derive(Debug)]
pub struct ImageCache {
    entries: HashMap<ImageKey, Entry>,
    lru: BTreeMap<u64, ImageKey>,
    tick: u64,
    lru_budget: u64,
    pinned_cap: u64,
    stats: CacheStats,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new(DEFAULT_LRU_BYTES, DEFAULT_PINNED_CAP_BYTES)
    }
}

impl ImageCache {
    /// A cache with an unpinned budget and a pinned hard cap, in bytes.
    #[must_use]
    pub fn new(lru_budget: u64, pinned_cap: u64) -> Self {
        Self {
            entries: HashMap::new(),
            lru: BTreeMap::new(),
            tick: 0,
            lru_budget,
            pinned_cap,
            stats: CacheStats::default(),
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            images: self.entries.len(),
            ..self.stats
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    fn check_key(key: &ImageKey) -> Result<(), CacheError> {
        match key {
            ImageKey::Path(p) if p.len() > MAX_PATH_KEY => Err(CacheError::KeyTooLong(p.len())),
            _ => Ok(()),
        }
    }

    /// Adds a decoded image (unpinned), replacing any image under the same key (a skin
    /// refresh). Pins on a replaced key carry over to the new image. Returns the shared
    /// image. An unpinned image larger than the whole budget is returned but not kept.
    ///
    /// # Errors
    /// [`CacheError::KeyTooLong`]; [`CacheError::PinnedCapExceeded`] when the key is pinned
    /// and the new image would push the pinned bytes over the cap (the old image stays).
    pub fn insert(&mut self, key: ImageKey, image: UiImage) -> Result<Arc<UiImage>, CacheError> {
        Self::check_key(&key)?;
        let image = Arc::new(image);
        let bytes = image.byte_size() as u64;
        let tick = self.next_tick();
        if let Some(e) = self.entries.get_mut(&key) {
            if e.pins > 0 {
                let others = self.stats.pinned_bytes - e.bytes;
                if others + bytes > self.pinned_cap {
                    self.stats.refused_pins += 1;
                    return Err(CacheError::PinnedCapExceeded {
                        pinned: others,
                        size: bytes,
                        cap: self.pinned_cap,
                    });
                }
                self.stats.pinned_bytes = others + bytes;
                e.image = Arc::clone(&image);
                e.bytes = bytes;
                e.tick = tick;
                return Ok(image);
            }
            self.lru.remove(&e.tick);
            self.stats.unpinned_bytes -= e.bytes;
            self.entries.remove(&key);
        }
        if bytes > self.lru_budget {
            return Ok(image);
        }
        self.entries.insert(
            key.clone(),
            Entry {
                image: Arc::clone(&image),
                bytes,
                pins: 0,
                tick,
            },
        );
        self.lru.insert(tick, key);
        self.stats.unpinned_bytes += bytes;
        self.evict();
        Ok(image)
    }

    /// The image under `key`, marked as recently used.
    pub fn get(&mut self, key: &ImageKey) -> Option<Arc<UiImage>> {
        let tick = self.next_tick();
        let e = self.entries.get_mut(key)?;
        if e.pins == 0 {
            self.lru.remove(&e.tick);
            self.lru.insert(tick, key.clone());
        }
        e.tick = tick;
        Some(Arc::clone(&e.image))
    }

    /// Whether `key` is held.
    #[must_use]
    pub fn contains(&self, key: &ImageKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Pins the image under `key` once more and returns it.
    ///
    /// # Errors
    /// [`CacheError::Missing`] when the key is not held; [`CacheError::PinnedCapExceeded`]
    /// when a first pin would push the pinned bytes over the hard cap.
    pub fn pin(&mut self, key: &ImageKey) -> Result<Arc<UiImage>, CacheError> {
        let e = self.entries.get_mut(key).ok_or(CacheError::Missing)?;
        if e.pins == 0 {
            if self.stats.pinned_bytes + e.bytes > self.pinned_cap {
                self.stats.refused_pins += 1;
                return Err(CacheError::PinnedCapExceeded {
                    pinned: self.stats.pinned_bytes,
                    size: e.bytes,
                    cap: self.pinned_cap,
                });
            }
            self.lru.remove(&e.tick);
            self.stats.unpinned_bytes -= e.bytes;
            self.stats.pinned_bytes += e.bytes;
        }
        e.pins = e.pins.saturating_add(1);
        Ok(Arc::clone(&e.image))
    }

    /// Adds a decoded image and pins it in one step.
    ///
    /// # Errors
    /// As [`ImageCache::insert`] and [`ImageCache::pin`]. On a refused pin
    /// ([`CacheError::PinnedCapExceeded`]) the image is not kept, pinned or unpinned: the
    /// caller draws the placeholder and reports a categorised failure (decision 0015 §4.5,
    /// hard cap).
    pub fn insert_pinned(
        &mut self,
        key: ImageKey,
        image: UiImage,
    ) -> Result<Arc<UiImage>, CacheError> {
        Self::check_key(&key)?;
        let bytes = image.byte_size() as u64;
        let already = self.entries.get(&key).is_some_and(|e| e.pins > 0);
        if !already && self.stats.pinned_bytes + bytes > self.pinned_cap {
            self.stats.refused_pins += 1;
            return Err(CacheError::PinnedCapExceeded {
                pinned: self.stats.pinned_bytes,
                size: bytes,
                cap: self.pinned_cap,
            });
        }
        if already {
            let img = self.insert(key.clone(), image)?;
            if let Some(e) = self.entries.get_mut(&key) {
                e.pins = e.pins.saturating_add(1);
            }
            return Ok(img);
        }
        // Insert directly as pinned (it must not be evicted by the LRU on the way in).
        if let Some(old) = self.entries.remove(&key) {
            self.lru.remove(&old.tick);
            self.stats.unpinned_bytes -= old.bytes;
        }
        let image = Arc::new(image);
        let tick = self.next_tick();
        self.entries.insert(
            key,
            Entry {
                image: Arc::clone(&image),
                bytes,
                pins: 1,
                tick,
            },
        );
        self.stats.pinned_bytes += bytes;
        Ok(image)
    }

    /// Releases one pin; at zero pins the image joins the LRU (and may be evicted).
    pub fn unpin(&mut self, key: &ImageKey) {
        let tick = self.next_tick();
        let Some(e) = self.entries.get_mut(key) else {
            return;
        };
        if e.pins == 0 {
            return;
        }
        e.pins -= 1;
        if e.pins == 0 {
            self.stats.pinned_bytes -= e.bytes;
            self.stats.unpinned_bytes += e.bytes;
            e.tick = tick;
            self.lru.insert(tick, key.clone());
            self.evict();
        }
    }

    /// Pins held on `key`.
    #[must_use]
    pub fn pins(&self, key: &ImageKey) -> u32 {
        self.entries.get(key).map_or(0, |e| e.pins)
    }

    /// Removes an unpinned image (a failed reload, a skin change). Pinned images stay.
    pub fn remove(&mut self, key: &ImageKey) {
        if let Some(e) = self.entries.get(key)
            && e.pins == 0
        {
            self.lru.remove(&e.tick);
            self.stats.unpinned_bytes -= e.bytes;
            self.entries.remove(key);
        }
    }

    fn evict(&mut self) {
        while self.stats.unpinned_bytes > self.lru_budget {
            let Some((_, key)) = self.lru.pop_first() else {
                break;
            };
            if let Some(e) = self.entries.remove(&key) {
                self.stats.unpinned_bytes -= e.bytes;
                self.stats.evictions += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_material::ui_image::PixelLayout;

    fn img(side: u32) -> UiImage {
        UiImage::new(
            side,
            side,
            side,
            side,
            PixelLayout::A8R8G8B8,
            vec![0xFF00_0000; (side * side) as usize],
        )
        .unwrap()
    }

    #[test]
    fn lru_evicts_oldest_unpinned() {
        // Each 16 × 16 image is 1 KiB; budget 3 KiB.
        let mut c = ImageCache::new(3 * 1024, 1 << 20);
        for i in 0..3 {
            c.insert(ImageKey::Did(i), img(16)).unwrap();
        }
        assert!(c.get(&ImageKey::Did(0)).is_some()); // 0 is now the newest
        c.insert(ImageKey::Did(3), img(16)).unwrap();
        assert!(c.contains(&ImageKey::Did(0)));
        assert!(!c.contains(&ImageKey::Did(1)), "1 was least recently used");
        assert_eq!(c.stats().evictions, 1);
    }

    #[test]
    fn pinned_never_evicted_and_hard_cap() {
        let mut c = ImageCache::new(1024, 2 * 1024);
        c.insert_pinned(ImageKey::Did(1), img(16)).unwrap();
        c.insert_pinned(ImageKey::Path("a.tga".into()), img(16))
            .unwrap();
        for i in 10..20 {
            c.insert(ImageKey::Did(i), img(16)).unwrap();
        }
        assert!(c.contains(&ImageKey::Did(1)));
        assert!(c.contains(&ImageKey::Path("a.tga".into())));
        // A third pinned image exceeds the 2 KiB hard cap.
        let e = c.insert_pinned(ImageKey::Did(2), img(16)).unwrap_err();
        assert!(matches!(e, CacheError::PinnedCapExceeded { .. }));
        assert_eq!(c.stats().refused_pins, 1);
        c.unpin(&ImageKey::Did(1));
        assert_eq!(c.stats().pinned_bytes, 1024);
        c.insert_pinned(ImageKey::Did(2), img(16)).unwrap();
        assert_eq!(c.stats().pinned_bytes, 2048);
    }

    #[test]
    fn pin_counts() {
        let mut c = ImageCache::new(1 << 20, 1 << 20);
        c.insert(ImageKey::Did(7), img(4)).unwrap();
        c.pin(&ImageKey::Did(7)).unwrap();
        c.pin(&ImageKey::Did(7)).unwrap();
        c.unpin(&ImageKey::Did(7));
        assert_eq!(c.pins(&ImageKey::Did(7)), 1);
        c.remove(&ImageKey::Did(7));
        assert!(
            c.contains(&ImageKey::Did(7)),
            "a pinned image is not removed"
        );
        c.unpin(&ImageKey::Did(7));
        assert_eq!(c.stats().pinned_bytes, 0);
        assert_eq!(c.stats().unpinned_bytes, 64);
        assert!(matches!(
            c.pin(&ImageKey::Did(99)),
            Err(CacheError::Missing)
        ));
    }
}
