// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests against a user's own install (read-only), skipped unless
//! `TESSERA_GAME_DIR` is set:
//!
//! - 200 random property sets (0x79) that have item icon layers are composed
//!   (`icon-layers.md` §3); IL1 (every layer resolves), IL3 and the canvas rule are
//!   checked. With `TESSERA_ICON_OUT_DIR` set, three composites (different layer
//!   combinations of at least three layers) are written there as 8× enlarged PNGs for a visual
//!   check. The directory must be outside the install; nothing is ever written to it.
//! - Skill, effect and trait icons from the same sample range compose.
//! - 500 random texture lists (0x40) are resolved level by level, and their chains pass
//!   `texture::check_chain` apart from the documented §4.2 exception.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test -p tessera-material --test real_data --
//! --nocapture`. `TESSERA_SAMPLE` overrides the icon sample size and `TESSERA_SEED` the
//! random seed. Only counts, DIDs and sizes are printed; no game data leaves the
//! install except the opt-in PNGs in a directory the user chooses.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use tessera_formats::{Record, RgbaImage, Surface, TextureList};
use tessera_material::icon::{
    IconComposer, IconStack, LayerRole, LayerStatus, MAX_LAYER_DIMENSION, names,
};
use tessera_material::texture::{
    ChainIssue, LevelHeader, check_chain, classify_chain, plan_upload, resolve_chain, select_level,
};
use tessera_props::{Did, GameData, PropertyRegistry, PropertySet};

fn game_data() -> Option<&'static GameData> {
    static DATA: OnceLock<Option<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(GameData::open(&dir).expect("open install"))
    })
    .as_ref()
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// xorshift64*: a small deterministic generator for sampling.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Every DID in `[lo, hi)` held by any archive of the install, sorted.
fn dids_in(gd: &GameData, lo: u32, hi: u32) -> Vec<u32> {
    let mut out = BTreeSet::new();
    for member in gd.source().members() {
        for entry in member.archive.entries_range(lo..hi) {
            out.insert(entry.expect("directory walk").did);
        }
    }
    out.into_iter().collect()
}

fn lookup<'a>(
    set: &'a PropertySet,
    reg: &'a PropertyRegistry,
) -> impl Fn(&str) -> Option<Did> + 'a {
    move |name| set.get_by_name(reg, name).and_then(|v| v.as_did())
}

fn composer(
    gd: &'static GameData,
) -> IconComposer<impl Fn(Did) -> Result<Option<Vec<u8>>, tessera_props::PropsError>> {
    IconComposer::new(move |d: Did| gd.read(d))
}

#[test]
fn item_icons_compose() {
    let Some(gd) = game_data() else { return };
    let reg = gd.registry().expect("registry").clone();
    for name in [
        names::BACKGROUND,
        names::UNDERLAY,
        names::SHADOW,
        names::IMAGE,
        names::OVERLAY,
    ] {
        assert!(reg.pid(name).is_some(), "dictionary lacks {name}");
    }
    let sets = dids_in(gd, 0x7900_0000, 0x7A00_0000);
    assert!(!sets.is_empty(), "no 0x79 property sets found");
    let want = env_u64("TESSERA_SAMPLE", 200) as usize;
    let mut rng = Rng(env_u64("TESSERA_SEED", 0x1C0_1A7E5) | 1);
    let comp = composer(gd);
    let out_dir = std::env::var_os("TESSERA_ICON_OUT_DIR").map(PathBuf::from);
    if let Some(dir) = &out_dir {
        let install = std::fs::canonicalize(gd.source().dir()).expect("install dir");
        let out = std::fs::canonicalize(dir).expect("TESSERA_ICON_OUT_DIR must exist");
        assert!(
            !out.starts_with(&install),
            "refusing to write inside the install"
        );
    }

    let (mut tried, mut composed) = (0usize, 0usize);
    let mut combos: BTreeMap<String, usize> = BTreeMap::new();
    let mut sizes: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let (mut transparent_layers, mut translucent_px_icons) = (0usize, 0usize);
    let mut saved: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    while composed < want && tried < 200_000 {
        tried += 1;
        let did = sets[rng.below(sets.len())];
        if !seen.insert(did) {
            continue;
        }
        let Some(set) = gd.property_set(Did(did)).expect("property set") else {
            continue;
        };
        let stack = IconStack::item(lookup(&set, &reg));
        if stack.is_empty() {
            continue;
        }
        composed += 1;
        let roles: Vec<LayerRole> = stack.entries().iter().map(|e| e.role).collect();
        let combo: String = roles.iter().map(|r| &r.name()[..1]).collect::<String>();
        *combos.entry(combo.clone()).or_default() += 1;
        // IL3: an Image implies a Background.
        if roles.contains(&LayerRole::Image) {
            assert!(
                roles.contains(&LayerRole::Background) || stack.len() == 1,
                "set {did:#010x} has an image but no background"
            );
        }
        let c = comp.compose(&stack);
        // IL1: every non-zero layer value resolves and decodes.
        let skipped: Vec<_> = c.skipped().collect();
        assert!(
            skipped.is_empty(),
            "set {did:#010x}: layers left out: {:?}",
            skipped
                .iter()
                .map(|(e, err)| format!("{} {}: {err}", e.role, e.did))
                .collect::<Vec<_>>()
        );
        transparent_layers += c
            .layers
            .iter()
            .filter(|l| matches!(l.status, LayerStatus::Transparent { .. }))
            .count();
        let img = c.image.expect("icon with present layers");
        assert!(img.width <= MAX_LAYER_DIMENSION && img.height <= MAX_LAYER_DIMENSION);
        if let Some(bg) = stack.did_of(LayerRole::Background) {
            let bg = comp.layer(bg).expect("background");
            assert_eq!((img.width, img.height), (bg.width, bg.height));
        }
        *sizes.entry((img.width, img.height)).or_default() += 1;
        if img.pixels.as_chunks::<4>().0.iter().any(|p| p[3] < 255) {
            translucent_px_icons += 1;
        }
        // Save the first composite of up to three distinct combinations of three or more
        // layers (the interesting ones for a visual check).
        if let Some(dir) = &out_dir
            && saved.len() < 3
            && combo.len() >= 3
            && !saved.contains(&combo)
        {
            let path = dir.join(format!("icon_{did:08x}_{combo}.png"));
            write_png(&path, &enlarge(&img, 8)).expect("write png");
            println!("wrote {}", path.display());
            saved.push(combo);
        }
    }
    let (cached, bytes) = comp.cache_usage();
    println!(
        "item icons: {composed} composed from {tried} draws over {} sets; combos (b/u/s/i/o) {combos:?}; \
         sizes {sizes:?}; transparent layers skipped {transparent_layers}; icons with alpha < 255 somewhere \
         {translucent_px_icons}; cache {cached} layers / {bytes} bytes",
        sets.len()
    );
    assert_eq!(
        composed, want,
        "not enough sets with icon layers in the sample"
    );
}

#[test]
fn other_icon_families_compose() {
    let Some(gd) = game_data() else { return };
    let reg = gd.registry().expect("registry").clone();
    let sets = dids_in(gd, 0x7900_0000, 0x7A00_0000);
    let mut rng = Rng(env_u64("TESSERA_SEED", 0x5EED) | 1);
    let comp = composer(gd);
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for _ in 0..60_000 {
        let did = sets[rng.below(sets.len())];
        if !seen.insert(did) {
            continue;
        }
        let Some(set) = gd.property_set(Did(did)).expect("property set") else {
            continue;
        };
        let l = lookup(&set, &reg);
        let stacks = [
            ("skill", IconStack::single_property(&l, names::SKILL_ICON)),
            ("effect", IconStack::single_property(&l, names::EFFECT_ICON)),
            ("trait", IconStack::trait_icon(&l)),
            ("large", IconStack::large_item_image(&l)),
        ];
        for (family, stack) in stacks {
            if stack.is_empty() {
                continue;
            }
            let entry = counts.entry(family).or_default();
            if entry.0 >= 50 {
                continue;
            }
            let c = comp.compose(&stack);
            entry.0 += 1;
            if c.image.is_some() && c.skipped().next().is_none() {
                entry.1 += 1;
            } else {
                println!(
                    "{family} {did:#010x}: {:?}",
                    c.skipped()
                        .map(|(e, err)| format!("{}: {err}", e.did))
                        .collect::<Vec<_>>()
                );
            }
        }
        if counts.len() == 4 && counts.values().all(|(n, _)| *n >= 50) {
            break;
        }
    }
    println!("other families (tried, composed): {counts:?}");
    for (family, (n, ok)) in &counts {
        assert_eq!(n, ok, "{family}: some icons failed to compose");
    }
    assert!(counts.contains_key("skill") && counts.contains_key("effect"));
}

#[test]
fn texture_chains_resolve() {
    let Some(gd) = game_data() else { return };
    let lists = dids_in(gd, 0x4000_0000, 0x4100_0000);
    assert!(!lists.is_empty(), "no texture lists found");
    let mut rng = Rng(env_u64("TESSERA_SEED", 0x7E47) | 1);
    let (mut checked, mut unavailable, mut not_halved) = (0usize, 0usize, 0usize);
    let mut shapes: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_small: BTreeMap<u32, usize> = BTreeMap::new();
    for _ in 0..500 {
        let did = lists[rng.below(lists.len())];
        let bytes = gd.read(Did(did)).expect("read").expect("listed DID");
        let list = TextureList::parse_expecting(&bytes, did).expect("texture list decodes");
        if list.levels().is_none() {
            continue; // cube maps: per face
        }
        let chain = resolve_chain(
            &list,
            |d| {
                let b = gd.read(d).ok()??;
                Surface::parse_expecting(&b, d)
                    .ok()
                    .map(|s| LevelHeader::from(&s))
            },
            None,
        );
        let headers: Vec<LevelHeader> = chain.iter().map(|l| l.level).collect();
        let Some(plan) = plan_upload(&headers) else {
            unavailable += 1;
            continue;
        };
        checked += 1;
        let first = headers[0];
        *shapes
            .entry(format!(
                "{:?}",
                classify_chain(
                    list.level_count.into(),
                    first.width,
                    first.height,
                    first.format
                )
            ))
            .or_default() += 1;
        *missing_small.entry(plan.missing).or_default() += 1;
        // The plan never asks for more mips than the texture can hold, and its first
        // uploaded level has level 0's size.
        assert!(
            plan.uploadable >= 1 && plan.uploadable <= plan.complete,
            "{did:#010x}: {plan:?}"
        );
        assert!(
            plan.first + plan.uploadable <= plan.stored,
            "{did:#010x}: {plan:?}"
        );
        let top = headers[plan.first as usize];
        assert_eq!((top.width, top.height), (first.width, first.height));
        for issue in check_chain(&headers) {
            match issue {
                ChainIssue::NotHalved { .. } => not_halved += 1,
                other => panic!("list {did:#010x}: {other:?}"),
            }
        }
        let pick = select_level(&headers, 64, 64).expect("non-empty");
        assert!(pick < headers.len());
    }
    println!(
        "texture lists: {checked} chains checked of {} lists, {unavailable} unavailable, \
         {not_halved} non-halving levels; shapes {shapes:?}; missing small levels {missing_small:?}",
        lists.len()
    );
    assert!(checked > 0);
}

/// Nearest-neighbour enlargement, for viewing.
fn enlarge(img: &RgbaImage, k: u32) -> RgbaImage {
    let (w, h) = (img.width * k, img.height * k);
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let i = (((y / k) * img.width + x / k) * 4) as usize;
            pixels.extend_from_slice(&img.pixels[i..i + 4]);
        }
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A minimal RGBA8 PNG writer (filter 0 on every row).
fn write_png(path: &std::path::Path, img: &RgbaImage) -> std::io::Result<()> {
    let mut raw = Vec::with_capacity(img.pixels.len() + img.height as usize);
    for row in img.pixels.chunks_exact(img.width as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&raw)?;
    let idat = z.finish()?;
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&img.width.to_be_bytes());
    ihdr.extend_from_slice(&img.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [(b"IHDR", ihdr), (b"IDAT", idat), (b"IEND", Vec::new())] {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut chunk = kind.to_vec();
        chunk.extend_from_slice(&data);
        out.extend_from_slice(&chunk);
        out.extend_from_slice(&crc32(&chunk).to_be_bytes());
    }
    std::fs::write(path, out)
}
