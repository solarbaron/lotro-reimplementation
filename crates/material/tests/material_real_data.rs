// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests of the material chain (`docs/specs/material.md`) against a user's own
//! install (read-only), skipped unless `TESSERA_GAME_DIR` is set:
//!
//! - `all_material_records`: every surface record (0x31), material (0x30) and shader
//!   record (0x2B) decodes; the invariants MT1, MT2, MT4, MT5, MT7 and MT8 are counted
//!   per record (a failure does not stop the census), and every surface record is
//!   resolved through [`MaterialLibrary`] with a census of the feature flags.
//! - `random_mesh_materials`: 500 random meshes (0x06) have their surface lists read from
//!   the raw mesh header (§9.1: DID, flags, *S*, *S* surface DIDs, submesh count *N*;
//!   MT6 *S* = *N*); every submesh's material is resolved, its diffuse texture decoded
//!   (capped at 256 px) and, for dyeable materials, a dye preview made. With
//!   `TESSERA_MATERIAL_OUT_DIR` set (a directory outside the install), a few diffuse
//!   textures and dye previews are written there as PNGs for a visual check.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-material --test
//! material_real_data -- --nocapture --test-threads 1`. `TESSERA_SAMPLE` overrides the
//! mesh sample size and `TESSERA_SEED` the seed. Only counts, DIDs and sizes are
//! printed.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{Did, Record, RgbaImage};
use tessera_material::records::{GROUP_TEXTURE, ParamValue, group_of};
use tessera_material::resolve::IgnoreReason;
use tessera_material::vocab::ids;
use tessera_material::{
    BlendMode, MaterialLibrary, MaterialRecord, RecordSource, ShaderRecord, SurfaceRecord,
};

fn archives() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open install"))
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

/// Every DID with high byte `prefix` held by any archive of the install, sorted.
fn dids_with_prefix(set: &ArchiveSet, prefix: u8) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let hi = lo | 0x00FF_FFFF;
    let mut out = BTreeSet::new();
    for member in set.members() {
        for entry in member.archive.entries_range(lo..=hi) {
            out.insert(entry.expect("directory walk").did);
        }
    }
    out.into_iter().collect()
}

fn read(set: &ArchiveSet, did: u32) -> Vec<u8> {
    set.read_record(Did(did))
        .expect("read")
        .expect("listed record exists")
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        100.0
    } else {
        100.0 * n as f64 / d as f64
    }
}

#[test]
fn all_material_records() {
    let Some(set) = archives() else { return };
    let lists: HashSet<u32> = dids_with_prefix(set, 0x40).into_iter().collect();

    // Shader records: decode all, MT4 (in the decoder), MT8.
    let shader_dids = dids_with_prefix(set, 0x2B);
    let mut shaders: BTreeMap<u32, ShaderRecord> = BTreeMap::new();
    let (mut sh_fail, mut sh_unusual, mut mt8, mut mt5_sh) = (0, 0, 0, 0);
    let (mut perms, mut samplers, mut decls) = (0usize, 0usize, 0usize);
    for &did in &shader_dids {
        match ShaderRecord::parse_expecting(&read(set, did), did) {
            Ok(s) => {
                sh_unusual += usize::from(s.has_unusual_fields());
                if s.declares(ids::ALPHA_TEST) != s.declares(ids::ALPHA_THRESHOLD) {
                    mt8 += 1;
                }
                for p in &s.params {
                    if let ParamValue::Texture(t) = p.value
                        && !t.is_null()
                        && !lists.contains(&t.0)
                    {
                        mt5_sh += 1;
                    }
                }
                perms += s.permutations.len();
                samplers += s
                    .permutations
                    .iter()
                    .map(|p| p.samplers.len())
                    .sum::<usize>();
                decls += s.params.len();
                shaders.insert(did, s);
            }
            Err(e) => {
                sh_fail += 1;
                eprintln!("shader {did:#010X}: {e}");
            }
        }
    }
    println!(
        "shader records: {} of {} decode ({:.2}%), {sh_unusual} unusual; {decls} declarations, {perms} permutations, {samplers} samplers; MT8 violations {mt8}; MT5 unresolved defaults {mt5_sh}",
        shaders.len(),
        shader_dids.len(),
        pct(shaders.len(), shader_dids.len())
    );

    // Materials: decode all, MT2, MT5.
    let material_dids = dids_with_prefix(set, 0x30);
    let mut materials: BTreeMap<u32, MaterialRecord> = BTreeMap::new();
    let (mut m_fail, mut m_unusual, mut mt5, mut entries, mut zero_tex) = (0, 0, 0, 0usize, 0);
    for &did in &material_dids {
        match MaterialRecord::parse_expecting(&read(set, did), did) {
            Ok(m) => {
                m_unusual += usize::from(m.has_unusual_fields());
                entries += m.entries.len();
                for e in &m.entries {
                    if let ParamValue::Texture(t) = e.value {
                        if t.is_null() {
                            zero_tex += 1;
                        } else if !lists.contains(&t.0) {
                            mt5 += 1;
                        }
                    }
                    assert_eq!(
                        group_of(e.id) == GROUP_TEXTURE,
                        matches!(e.value, ParamValue::Texture(_)),
                        "material {did:#010X} id {}",
                        e.id
                    );
                }
                materials.insert(did, m);
            }
            Err(e) => {
                m_fail += 1;
                eprintln!("material {did:#010X}: {e}");
            }
        }
    }
    println!(
        "materials: {} of {} decode ({:.2}%), {m_unusual} unusual (MT2); {entries} entries; texture values: {zero_tex} zero, {mt5} unresolved (MT5)",
        materials.len(),
        material_dids.len(),
        pct(materials.len(), material_dids.len())
    );

    // Surface records: decode all, MT1, MT7.
    let surface_dids = dids_with_prefix(set, 0x31);
    let mut surfaces: Vec<SurfaceRecord> = Vec::new();
    let (mut s_fail, mut s_unusual, mut mt1, mut mt7) = (0, 0, 0, 0);
    let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
    for &did in &surface_dids {
        match SurfaceRecord::parse_expecting(&read(set, did), did) {
            Ok(s) => {
                s_unusual += usize::from(s.has_unusual_fields());
                *counts.entry(s.materials.len()).or_default() += 1;
                let shader = shaders.get(&s.shader.0);
                if shader.is_none() || s.materials.iter().any(|m| !materials.contains_key(&m.0)) {
                    mt1 += 1;
                }
                if let Some(sh) = shader {
                    for m in s.materials.iter().filter_map(|m| materials.get(&m.0)) {
                        for e in &m.entries {
                            if sh
                                .declaration(e.id)
                                .is_some_and(|d| !d.value.same_type(&e.value))
                            {
                                mt7 += 1;
                            }
                        }
                    }
                }
                surfaces.push(s);
            }
            Err(e) => {
                s_fail += 1;
                eprintln!("surface record {did:#010X}: {e}");
            }
        }
    }
    println!(
        "surface records: {} of {} decode ({:.2}%), {s_unusual} unusual; material counts {counts:?}; MT1 violations {mt1}; MT7 violations {mt7}",
        surfaces.len(),
        surface_dids.len(),
        pct(surfaces.len(), surface_dids.len())
    );

    // Resolve every surface record through the library.
    let lib = MaterialLibrary::new(set);
    let mut resolved = 0usize;
    let mut failures: BTreeMap<String, usize> = BTreeMap::new();
    let mut blend: BTreeMap<String, usize> = BTreeMap::new();
    let mut thresholds: BTreeMap<String, usize> = BTreeMap::new();
    let mut opacity_src: BTreeMap<String, usize> = BTreeMap::new();
    let mut diffuse_roles: BTreeMap<String, usize> = BTreeMap::new();
    let mut f = [0usize; 12];
    let (mut skipped, mut ignored_undeclared, mut ignored_mismatch) = (0, 0, 0);
    let (mut slot_lists, mut slot_lists_ok) = (0usize, 0usize);
    let mut alpha_test_declared = 0usize;
    let mut no_diffuse = 0usize;
    // Transparent blends whose factors read source alpha, by where that alpha comes from
    // (§11.2 source, slot 3000 for the four roles of §20.1, or none: 2000 alpha only).
    let mut blend_opacity: BTreeMap<String, usize> = BTreeMap::new();
    let mut extended_lists: Vec<Did> = Vec::new();
    let mut sourceless_lists: Vec<Did> = Vec::new();
    // §11.2 table "Tested opacity": surface records by the form their shader record tests.
    let mut tested_forms: BTreeMap<String, usize> = BTreeMap::new();
    let mut sum_shaders_wrong: Vec<Did> = Vec::new();
    for s in &surfaces {
        match lib.resolve_surface(s.did) {
            Ok(r) => {
                resolved += 1;
                let ft = &r.features;
                skipped += r.skipped.len();
                *tested_forms
                    .entry(format!("{:?}", ft.tested_opacity))
                    .or_default() += 1;
                // The seven sum records test a sum whenever they have two opacity roles.
                if tessera_material::alpha::SUM_OPACITY_RECORDS.contains(&r.shader.did.0)
                    && !matches!(ft.tested_opacity, tessera_material::TestedOpacity::Sum(..))
                {
                    sum_shaders_wrong.push(r.shader.did);
                }
                for i in r.params.ignored() {
                    match i.reason {
                        IgnoreReason::Undeclared => ignored_undeclared += 1,
                        IgnoreReason::TypeMismatch => ignored_mismatch += 1,
                    }
                }
                *blend.entry(format!("{:?}", ft.blend.mode)).or_default() += 1;
                let b = &ft.blend;
                if b.is_transparent() && b.uses_source_alpha() {
                    let role = r.shader.role(ids::DIFFUSE).unwrap_or("-");
                    let key = match (ft.opacity_slot, b.opacity_slot) {
                        (Some(_), _) => "§11.2 source".to_owned(),
                        (None, Some(_)) => {
                            extended_lists.extend(r.diffuse());
                            format!("slot 3000 ({role})")
                        }
                        (None, None) => {
                            sourceless_lists.extend(r.diffuse());
                            let slots: Vec<String> = r
                                .slots
                                .iter()
                                .map(|t| format!("{}:{}", t.id, t.role.as_deref().unwrap_or("-")))
                                .collect();
                            format!("none ({role}; slots [{}])", slots.join(" "))
                        }
                    };
                    *blend_opacity.entry(key).or_default() += 1;
                }
                // Effect cards whose shape is only in the diffuse alpha (review finding):
                // their blend opacity comes from slot 3000.
                if matches!(
                    s.did.0,
                    0x3100_0103 | 0x3100_0106 | 0x3100_0071 | 0x3100_0074
                ) {
                    assert_eq!(b.opacity_slot, Some(ids::DIFFUSE), "{}", s.did);
                }
                if ft.blend.mode == BlendMode::Custom {
                    *blend
                        .entry(format!(
                            "Custom {:?}/{:?}/{:?}",
                            ft.blend.src, ft.blend.dst, ft.blend.op
                        ))
                        .or_default() += 1;
                }
                if r.shader.declares(ids::ALPHA_TEST) {
                    alpha_test_declared += 1;
                }
                if let Some(at) = ft.alpha_test {
                    *thresholds
                        .entry(format!("{:.2}", at.threshold))
                        .or_default() += 1;
                    *opacity_src.entry(at.source_slot.to_string()).or_default() += 1;
                }
                *diffuse_roles
                    .entry(r.shader.role(ids::DIFFUSE).unwrap_or("-").to_owned())
                    .or_default() += 1;
                if r.diffuse().is_none() {
                    no_diffuse += 1;
                }
                for (i, b) in [
                    ft.lit,
                    ft.alpha_test.is_some(),
                    ft.two_sided,
                    ft.normal_mapped,
                    ft.specular,
                    ft.dyeable,
                    ft.dyeable_2,
                    ft.emissive,
                    ft.glow,
                    ft.environment,
                    ft.second_layer,
                    ft.animated,
                ]
                .into_iter()
                .enumerate()
                {
                    f[i] += usize::from(b);
                }
                for list in r.texture_lists() {
                    slot_lists += 1;
                    slot_lists_ok += usize::from(lib.texture_list(list).is_ok());
                }
            }
            Err(e) => {
                let key = e.to_string().split(':').next().unwrap_or("").to_owned();
                *failures.entry(key).or_default() += 1;
            }
        }
    }
    let n = surfaces.len();
    println!(
        "resolved {resolved} of {n} surface records ({:.2}%); failures {failures:?}",
        pct(resolved, n)
    );
    println!(
        "skipped materials {skipped}; ignored entries: {ignored_undeclared} undeclared, {ignored_mismatch} type mismatch"
    );
    println!(
        "texture lists of slots: {slot_lists_ok} of {slot_lists} decode ({:.2}%)",
        pct(slot_lists_ok, slot_lists)
    );
    let names = [
        "lit",
        "alpha_test",
        "two_sided",
        "normal_mapped",
        "specular",
        "dyeable",
        "dyeable_2",
        "emissive",
        "glow",
        "environment",
        "second_layer",
        "animated",
    ];
    let flags: Vec<String> = names
        .iter()
        .zip(f)
        .map(|(n, c)| format!("{n} {c}"))
        .collect();
    println!("features: {}", flags.join(", "));
    println!("blend modes: {blend:?}");
    println!("alpha test: declared by {alpha_test_declared}; sources {opacity_src:?}");
    println!("alpha-test thresholds: {thresholds:?}");
    println!("tested opacity (material.md §11.2 table): {tested_forms:?}");
    assert!(
        sum_shaders_wrong.is_empty(),
        "sum records without two opacity roles: {sum_shaders_wrong:?}"
    );
    println!("diffuse roles: {diffuse_roles:?}; no diffuse texture: {no_diffuse}");
    println!("cache: {:?}", lib.cache_usage());
    // How many of those diffuse textures are mostly transparent (at least a quarter of
    // the pixels below alpha 128 at 64 px): with an opacity source they draw as shapes,
    // without one as solid cards.
    let mostly_transparent = |lists: &[Did]| {
        let distinct: BTreeSet<Did> = lists.iter().copied().collect();
        let n = distinct
            .iter()
            .filter(|&&d| {
                lib.texture(d, Some(64)).is_ok_and(|t| {
                    let low = t
                        .image
                        .pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|p| p[3] < 128)
                        .count();
                    low * 4 >= t.image.pixels.len() / 4
                })
            })
            .count();
        (n, distinct.len())
    };
    let (ext_low, ext_n) = mostly_transparent(&extended_lists);
    let (none_low, none_n) = mostly_transparent(&sourceless_lists);
    println!(
        "source-alpha blends by opacity source: {blend_opacity:?}; mostly transparent diffuse \
         lists: {ext_low} of {ext_n} with slot-3000 opacity, {none_low} of {none_n} without any"
    );

    // §20.1 (revised 2026-10-05, replacing the §18 Decision that also took the alpha of a
    // `Color` diffuse): a blend without a §11.2 source takes slot-3000 alpha only for the
    // roles whose alpha reaches the programs' output. Others (mostly `Color`) blend by the
    // 2000 alpha, so `none_low` above is reported, not required to be 0.
    for key in blend_opacity.keys().filter(|k| k.starts_with("slot 3000")) {
        assert!(
            [
                "DiffuseTexture",
                "LiquidDiffuseTexture",
                "ColorTexture",
                "ColorTexture2"
            ]
            .iter()
            .any(|role| *key == format!("slot 3000 ({role})")),
            "slot-3000 blend opacity for another role: {key}"
        );
    }
    assert_eq!(sh_fail + m_fail + s_fail, 0, "records that do not decode");
    assert_eq!(mt1 + mt5 + mt5_sh + mt7 + mt8, 0, "invariant violations");
    assert_eq!(resolved, n, "surface records that do not resolve");
    assert_eq!(
        slot_lists_ok, slot_lists,
        "texture lists that do not decode"
    );
}

/// The surface list of a raw mesh record (§9.1): DID, flags, *S*, *S* DIDs, *N*.
fn mesh_surfaces(bytes: &[u8]) -> Option<(Vec<Did>, u32)> {
    let u32_at = |o: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?))
    };
    let s = u32_at(8)? as usize;
    if s > bytes.len() / 4 {
        return None;
    }
    let dids = (0..s)
        .map(|i| u32_at(12 + 4 * i).map(Did))
        .collect::<Option<Vec<_>>>()?;
    let n = u32_at(12 + 4 * s)?;
    Some((dids, n))
}

#[test]
fn random_mesh_materials() {
    let Some(set) = archives() else { return };
    let meshes = dids_with_prefix(set, 0x06);
    assert!(!meshes.is_empty(), "no meshes found");
    let want = (env_u64("TESSERA_SAMPLE", 500) as usize).min(meshes.len());
    let mut rng = Rng(env_u64("TESSERA_SEED", 0x3A7E_51A1) | 1);
    let out_dir = std::env::var_os("TESSERA_MATERIAL_OUT_DIR").map(PathBuf::from);
    if let Some(dir) = &out_dir {
        let install = std::fs::canonicalize(set.dir()).expect("install dir");
        let out = std::fs::canonicalize(dir).expect("TESSERA_MATERIAL_OUT_DIR must exist");
        assert!(
            !out.starts_with(&install),
            "refusing to write inside the install"
        );
    }
    let lib = MaterialLibrary::new(set);
    let mut seen = HashSet::new();
    let (mut sampled, mut bad_header, mut mt6) = (0usize, 0usize, 0usize);
    let (mut submeshes, mut resolved, mut with_diffuse, mut diffuse_ok) = (0, 0, 0, 0);
    let (mut dyeable, mut dye_ok, mut saved) = (0usize, 0usize, 0usize);
    let mut dyed_saved = HashSet::new();
    let mut tex_errors: BTreeMap<String, usize> = BTreeMap::new();
    let mut res_errors: BTreeMap<String, usize> = BTreeMap::new();
    let mut levels: BTreeMap<usize, usize> = BTreeMap::new();
    while sampled < want {
        let did = meshes[rng.below(meshes.len())];
        if !seen.insert(did) {
            continue;
        }
        sampled += 1;
        let Some((surfaces, n)) = mesh_surfaces(&read(set, did)) else {
            bad_header += 1;
            continue;
        };
        if surfaces.len() as u32 != n {
            mt6 += 1;
        }
        for (i, r) in lib.resolve_mesh(&surfaces).into_iter().enumerate() {
            submeshes += 1;
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    let key = e.to_string().split(':').next().unwrap_or("").to_owned();
                    *res_errors.entry(key).or_default() += 1;
                    continue;
                }
            };
            resolved += 1;
            if r.diffuse().is_some() {
                with_diffuse += 1;
                match lib.slot_texture(&r, ids::DIFFUSE, Some(256)) {
                    Ok(Some(t)) => {
                        diffuse_ok += 1;
                        *levels.entry(t.level).or_default() += 1;
                        // The resolved chain for an upload starts at the level shown.
                        if t.kind != tessera_formats::TextureListKind::CubeMap {
                            let chain = lib.texture_chain(t.list, Some(256)).expect("chain");
                            assert_eq!(chain.levels[0].index, t.level, "{}", t.list);
                            let plan = chain.plan.expect("upload plan");
                            assert!(plan.first + plan.uploadable <= plan.stored);
                        }
                        if let Some(dir) = &out_dir
                            && saved < 6
                            && i == 0
                        {
                            let p = dir
                                .join(format!("{did:08X}_{i}_{:08X}_diffuse.png", surfaces[i].0));
                            write_png(&p, &t.image).expect("write png");
                            saved += 1;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let key = e.to_string().split(':').next().unwrap_or("").to_owned();
                        *tex_errors.entry(key).or_default() += 1;
                    }
                }
            }
            if r.features.dyeable {
                dyeable += 1;
                // Preview with a strong colour so the dyeable area is easy to see.
                let colors = tessera_material::DyeColors {
                    primary: Some([0.9, 0.2, 0.2, 1.0]),
                    secondary: Some([0.2, 0.3, 0.9, 1.0]),
                };
                match lib.dye_preview_with(&r, colors, Some(256)) {
                    Ok(Some(img)) => {
                        dye_ok += 1;
                        if let Some(dir) = &out_dir
                            && dyed_saved.len() < 4
                            && dyed_saved.insert(surfaces[i])
                        {
                            let p =
                                dir.join(format!("{did:08X}_{i}_{:08X}_dyed.png", surfaces[i].0));
                            write_png(&p, &img).expect("write png");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let key = e.to_string().split(':').next().unwrap_or("").to_owned();
                        *tex_errors.entry(key).or_default() += 1;
                    }
                }
            }
        }
    }
    println!(
        "meshes: {sampled} sampled, {bad_header} unreadable headers, MT6 violations {mt6}; submeshes {submeshes}, resolved {resolved} ({:.2}%), errors {res_errors:?}",
        pct(resolved, submeshes)
    );
    println!(
        "diffuse: {diffuse_ok} of {with_diffuse} decode ({:.2}%), shown levels {levels:?}; dyeable {dyeable}, dye previews {dye_ok}; texture errors {tex_errors:?}",
        pct(diffuse_ok, with_diffuse)
    );
    println!("cache: {:?}", lib.cache_usage());
    assert_eq!(bad_header + mt6, 0, "mesh headers");
    assert_eq!(resolved, submeshes, "submeshes that do not resolve");
    assert_eq!(
        diffuse_ok, with_diffuse,
        "diffuse textures that do not decode"
    );
    assert_eq!(dye_ok, dyeable, "dye previews");
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
