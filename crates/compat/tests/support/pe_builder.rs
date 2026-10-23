// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic PE image builder shared by the version-resource tests and the
//! `compat_version_resource` fuzz target (which uses it to start from valid
//! images). Every byte comes from the public PE/COFF and `VS_VERSIONINFO`
//! layouts; no game bytes are used.

#![allow(dead_code)]

pub fn utf16z(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out
}

pub fn pad4(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

/// One version block: header, key, aligned value, aligned children.
pub fn block(
    key: &str,
    value: &[u8],
    value_len_field: u16,
    kind: u16,
    children: &[Vec<u8>],
) -> Vec<u8> {
    let mut out = vec![0u8; 6];
    out.extend_from_slice(&utf16z(key));
    pad4(&mut out);
    out.extend_from_slice(value);
    for child in children {
        pad4(&mut out);
        out.extend_from_slice(child);
    }
    let len = u16::try_from(out.len()).expect("synthetic block fits u16");
    out[0..2].copy_from_slice(&len.to_le_bytes());
    out[2..4].copy_from_slice(&value_len_field.to_le_bytes());
    out[4..6].copy_from_slice(&kind.to_le_bytes());
    out
}

pub fn string_block(key: &str, value: &str) -> Vec<u8> {
    let raw = utf16z(value);
    let words = u16::try_from(raw.len() / 2).expect("short value");
    block(key, &raw, words, 1, &[])
}

pub fn fixed_info(file: (u32, u32), product: (u32, u32)) -> Vec<u8> {
    let words = [
        0xFEEF_04BDu32,
        0x0001_0000,
        file.0,
        file.1,
        product.0,
        product.1,
        0x3F,
        0,
        0x0004_0004,
        1,
        0,
        0,
        0,
    ];
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

pub fn version_info(strings: &[(&str, &str)]) -> Vec<u8> {
    let entries: Vec<Vec<u8>> = strings.iter().map(|(k, v)| string_block(k, v)).collect();
    let table = block("040904b0", &[], 0, 1, &entries);
    let string_file_info = block("StringFileInfo", &[], 0, 1, &[table]);
    let var = block("Translation", &[0x09, 0x04, 0xB0, 0x04], 4, 0, &[]);
    let var_file_info = block("VarFileInfo", &[], 0, 1, &[var]);
    let fixed = fixed_info((0x0002_0001, 0x0004_0003), (0x0006_0005, 0x0008_0007));
    block(
        "VS_VERSION_INFO",
        &fixed,
        52,
        0,
        &[string_file_info, var_file_info],
    )
}

pub const RSRC_RVA: u32 = 0x1000;
pub const RSRC_FILE: usize = 0x200;

/// Builds the `.rsrc` section: type -> name -> language -> data entry -> block.
pub fn resource_section(version_block: &[u8], resource_type: u32) -> Vec<u8> {
    let mut r = Vec::new();
    let dir = |r: &mut Vec<u8>, id: u32, target: u32| {
        r.extend_from_slice(&[0u8; 12]);
        r.extend_from_slice(&0u16.to_le_bytes()); // named entries
        r.extend_from_slice(&1u16.to_le_bytes()); // id entries
        r.extend_from_slice(&id.to_le_bytes());
        r.extend_from_slice(&target.to_le_bytes());
    };
    dir(&mut r, resource_type, 0x8000_0000 | 0x18);
    dir(&mut r, 1, 0x8000_0000 | 0x30);
    dir(&mut r, 0x409, 0x48);
    assert_eq!(r.len(), 0x48);
    r.extend_from_slice(&(RSRC_RVA + 0x58).to_le_bytes());
    r.extend_from_slice(&u32::try_from(version_block.len()).unwrap().to_le_bytes());
    r.extend_from_slice(&[0u8; 8]);
    assert_eq!(r.len(), 0x58);
    r.extend_from_slice(version_block);
    r
}

/// Builds a PE32 (`pe32_plus == false`) or PE32+ image with one `.rsrc` section.
pub fn pe_image(rsrc: &[u8], pe32_plus: bool) -> Vec<u8> {
    let (opt_len, count_at, dirs_at, magic) = if pe32_plus {
        (240usize, 108usize, 112usize, 0x020Bu16)
    } else {
        (224usize, 92usize, 96usize, 0x010Bu16)
    };
    let mut pe = vec![0u8; RSRC_FILE];
    pe[0..2].copy_from_slice(b"MZ");
    pe[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    pe[0x40..0x44].copy_from_slice(b"PE\0\0");
    pe[0x46..0x48].copy_from_slice(&1u16.to_le_bytes()); // NumberOfSections
    pe[0x54..0x56].copy_from_slice(&u16::try_from(opt_len).unwrap().to_le_bytes());
    let opt = 0x58;
    pe[opt..opt + 2].copy_from_slice(&magic.to_le_bytes());
    pe[opt + count_at..opt + count_at + 4].copy_from_slice(&16u32.to_le_bytes());
    let res_dir = opt + dirs_at + 2 * 8;
    pe[res_dir..res_dir + 4].copy_from_slice(&RSRC_RVA.to_le_bytes());
    pe[res_dir + 4..res_dir + 8].copy_from_slice(&u32::try_from(rsrc.len()).unwrap().to_le_bytes());
    let sec = opt + opt_len;
    pe[sec..sec + 8].copy_from_slice(b".rsrc\0\0\0");
    let len = u32::try_from(rsrc.len()).unwrap();
    pe[sec + 8..sec + 12].copy_from_slice(&len.to_le_bytes()); // VirtualSize
    pe[sec + 12..sec + 16].copy_from_slice(&RSRC_RVA.to_le_bytes());
    pe[sec + 16..sec + 20].copy_from_slice(&len.to_le_bytes()); // SizeOfRawData
    pe[sec + 20..sec + 24].copy_from_slice(&u32::try_from(RSRC_FILE).unwrap().to_le_bytes());
    pe.extend_from_slice(rsrc);
    pe
}

pub const STRINGS: &[(&str, &str)] = &[
    ("CompanyName", "Synthetic Test Vendor"),
    ("FileDescription", "Synthetic client"),
    ("SyntheticBuild", "4321.0001.0002.0003"),
    ("Empty", ""),
];

pub fn synthetic_pe(pe32_plus: bool) -> Vec<u8> {
    pe_image(&resource_section(&version_info(STRINGS), 16), pe32_plus)
}
