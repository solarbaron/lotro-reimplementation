// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for the PE version-resource reader and the client network
//! version discovery of decision 0042 item 1. Every image is built here from
//! the public PE/COFF and `VS_VERSIONINFO` layouts; no game bytes are used.

use std::path::Path;

use tessera_compat::login::{
    AskUserReason, LoginError, LoginFailureClass, NetworkVersionDiscovery, NetworkVersionQuery,
    NetworkVersionSource, discover_network_version, is_valid_network_version,
    network_version_from_resource,
};
use tessera_compat::version_resource::{
    FixedVersion, MAX_VERSION_STRINGS, VersionResource, VersionResourceError, parse_version_info,
    read_version_resource, read_version_resource_from_file,
};

#[path = "support/pe_builder.rs"]
mod pe_builder;

use pe_builder::{
    RSRC_FILE, STRINGS, block, pe_image, resource_section, string_block, synthetic_pe, utf16z,
    version_info,
};

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

#[test]
fn reads_string_table_and_fixed_versions_from_pe32_plus_and_pe32() {
    for pe32_plus in [true, false] {
        let res = read_version_resource(&synthetic_pe(pe32_plus)).expect("synthetic PE parses");
        let pairs: Vec<(&str, &str)> = res.string_pairs().collect();
        assert_eq!(pairs, STRINGS.to_vec());
        assert!(res.strings.iter().all(|s| s.table == "040904b0"));
        assert_eq!(
            res.file_version,
            Some(FixedVersion {
                major: 2,
                minor: 1,
                build: 4,
                revision: 3
            })
        );
        assert_eq!(
            res.product_version.map(|v| v.to_string()).as_deref(),
            Some("6.5.8.7")
        );
    }
}

#[test]
fn lookup_prefers_exact_key_then_ignores_ascii_case() {
    let res = read_version_resource(&synthetic_pe(true)).unwrap();
    assert_eq!(res.lookup("SyntheticBuild"), Some("4321.0001.0002.0003"));
    assert_eq!(res.lookup("syntheticbuild"), Some("4321.0001.0002.0003"));
    assert_eq!(res.lookup("Missing"), None);
    assert_eq!(res.lookup("Empty"), Some(""));
}

#[test]
fn file_reader_matches_slice_reader_and_reads_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.exe");
    let image = synthetic_pe(true);
    std::fs::write(&path, &image).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();

    let from_file = read_version_resource_from_file(&path).unwrap();
    assert_eq!(from_file, read_version_resource(&image).unwrap());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        image,
        "file must be unchanged"
    );
}

#[test]
fn missing_file_is_an_io_error() {
    let err = read_version_resource_from_file(Path::new("/nonexistent/synthetic.exe")).unwrap_err();
    assert!(matches!(err, VersionResourceError::Io { .. }));
}

#[test]
fn rejects_non_pe_and_structural_errors() {
    assert!(matches!(
        read_version_resource(
            b"not a pe image at all, just bytes......................................"
        ),
        Err(VersionResourceError::NotPeImage { .. })
    ));
    assert!(matches!(
        read_version_resource(&[]),
        Err(VersionResourceError::Truncated { .. })
    ));

    // Bad PE signature.
    let mut img = synthetic_pe(true);
    img[0x40] = b'X';
    assert!(matches!(
        read_version_resource(&img),
        Err(VersionResourceError::NotPeImage { .. })
    ));

    // Unknown optional header magic.
    let mut img = synthetic_pe(true);
    img[0x58..0x5A].copy_from_slice(&0x0107u16.to_le_bytes());
    assert_eq!(
        read_version_resource(&img),
        Err(VersionResourceError::UnsupportedOptionalHeader { magic: 0x0107 })
    );

    // Zero and excessive section counts.
    for count in [0u16, 97, u16::MAX] {
        let mut img = synthetic_pe(true);
        img[0x46..0x48].copy_from_slice(&count.to_le_bytes());
        assert!(matches!(
            read_version_resource(&img),
            Err(VersionResourceError::BadSectionCount { .. })
        ));
    }

    // No resource directory.
    let mut img = synthetic_pe(true);
    let res_dir = 0x58 + 112 + 16;
    img[res_dir..res_dir + 8].fill(0);
    assert_eq!(
        read_version_resource(&img),
        Err(VersionResourceError::NoResourceDirectory)
    );

    // Resource directory RVA outside every section.
    let mut img = synthetic_pe(true);
    img[res_dir..res_dir + 4].copy_from_slice(&0x0090_0000u32.to_le_bytes());
    assert!(matches!(
        read_version_resource(&img),
        Err(VersionResourceError::RvaNotMapped { .. })
    ));

    // No RT_VERSION entry.
    let img = pe_image(&resource_section(&version_info(STRINGS), 3), true);
    assert_eq!(
        read_version_resource(&img),
        Err(VersionResourceError::NoVersionResource)
    );
}

#[test]
fn rejects_oversized_entries_and_directory_counts() {
    // Data entry size above the cap.
    let mut img = synthetic_pe(true);
    let size_at = RSRC_FILE + 0x48 + 4;
    img[size_at..size_at + 4].copy_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
    assert!(matches!(
        read_version_resource(&img),
        Err(VersionResourceError::ResourceTooLarge { .. })
    ));

    // Directory entry count above the cap (named + id entries).
    let mut img = synthetic_pe(true);
    img[RSRC_FILE + 12..RSRC_FILE + 14].copy_from_slice(&u16::MAX.to_le_bytes());
    img[RSRC_FILE + 14..RSRC_FILE + 16].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        read_version_resource(&img),
        Err(VersionResourceError::TooManyDirectoryEntries { .. })
    ));

    // A directory pointing outside the resource directory.
    let mut img = synthetic_pe(true);
    img[RSRC_FILE + 20..RSRC_FILE + 24]
        .copy_from_slice(&(0x8000_0000u32 | 0x00FF_FFF0).to_le_bytes());
    assert!(matches!(
        read_version_resource(&img),
        Err(VersionResourceError::MalformedResourceDirectory { .. })
    ));

    // A leaf where a directory belongs.
    let mut img = synthetic_pe(true);
    img[RSRC_FILE + 20..RSRC_FILE + 24].copy_from_slice(&0x48u32.to_le_bytes());
    assert!(matches!(
        read_version_resource(&img),
        Err(VersionResourceError::MalformedResourceDirectory { .. })
    ));
}

#[test]
fn version_block_errors() {
    let good = version_info(STRINGS);
    assert!(parse_version_info(&good).is_ok());

    // Wrong root key.
    let mut bad_key = good.clone();
    bad_key[6] = b'X';
    assert!(matches!(
        parse_version_info(&bad_key),
        Err(VersionResourceError::MalformedVersionInfo { .. })
    ));

    // Root length past the data.
    let mut overrun = good.clone();
    overrun[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        parse_version_info(&overrun),
        Err(VersionResourceError::MalformedVersionInfo { .. })
    ));

    // Root length shorter than a header.
    let mut tiny = good.clone();
    tiny[0..2].copy_from_slice(&3u16.to_le_bytes());
    assert!(matches!(
        parse_version_info(&tiny),
        Err(VersionResourceError::MalformedVersionInfo { .. })
    ));

    // Fixed information longer than its block.
    let mut fixed = good.clone();
    fixed[2..4].copy_from_slice(&0xFFF0u16.to_le_bytes());
    assert!(matches!(
        parse_version_info(&fixed),
        Err(VersionResourceError::MalformedVersionInfo { .. })
    ));

    // A zero `wLength` ends the children (it is padding): nothing loops, and
    // what came before is kept.
    let first_child = good
        .windows(utf16z("StringFileInfo").len())
        .position(|w| w == utf16z("StringFileInfo").as_slice())
        .unwrap()
        - 6;
    let mut zero = good.clone();
    zero[first_child..first_child + 2].copy_from_slice(&0u16.to_le_bytes());
    let res = parse_version_info(&zero).unwrap();
    assert!(res.strings.is_empty());
    assert!(res.file_version.is_some());

    // Zero padding after the last string of a table loses nothing.
    let padded_table = block(
        "040904b0",
        &[],
        0,
        1,
        &[string_block("Key", "Value"), vec![0u8; 8]],
    );
    let sfi = block("StringFileInfo", &[], 0, 1, &[padded_table]);
    let root = block("VS_VERSION_INFO", &[], 0, 0, &[sfi, vec![0u8; 12]]);
    let res = parse_version_info(&root).unwrap();
    assert_eq!(res.lookup("Key"), Some("Value"));
    assert_eq!(res.strings.len(), 1);

    // Every truncation is an error, never a panic.
    for len in 0..good.len() {
        let _ = parse_version_info(&good[..len]);
    }

    // No fixed information: strings are still read.
    let entries = vec![string_block("Key", "Value")];
    let table = block("040904b0", &[], 0, 1, &entries);
    let sfi = block("StringFileInfo", &[], 0, 1, &[table]);
    let root = block("VS_VERSION_INFO", &[], 0, 0, &[sfi]);
    let res = parse_version_info(&root).unwrap();
    assert_eq!(res.file_version, None);
    assert_eq!(res.lookup("Key"), Some("Value"));
}

#[test]
fn string_count_is_capped() {
    let keys: Vec<String> = (0..=MAX_VERSION_STRINGS)
        .map(|i| format!("k{i:04}"))
        .collect();
    let at_cap: Vec<(&str, &str)> = keys[..MAX_VERSION_STRINGS]
        .iter()
        .map(|k| (k.as_str(), ""))
        .collect();
    let res = parse_version_info(&version_info(&at_cap)).unwrap();
    assert_eq!(res.strings.len(), MAX_VERSION_STRINGS);

    let over: Vec<(&str, &str)> = keys.iter().map(|k| (k.as_str(), "")).collect();
    assert_eq!(
        parse_version_info(&version_info(&over)),
        Err(VersionResourceError::TooManyStrings)
    );
}

#[test]
fn mutated_images_never_panic() {
    let image = synthetic_pe(true);
    for len in 0..image.len() {
        let _ = read_version_resource(&image[..len]);
    }
    for at in 0..image.len() {
        for value in [0x00u8, 0x7F, 0x80, 0xFF] {
            let mut copy = image.clone();
            copy[at] = value;
            let _ = read_version_resource(&copy);
        }
    }
}

// ---------------------------------------------------------------------------
// Discovery (decision 0042 item 1)
// ---------------------------------------------------------------------------

fn write_exe(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("client.exe");
    std::fs::write(&path, synthetic_pe(true)).unwrap();
    path
}

#[test]
fn user_value_wins_over_the_version_resource() {
    let dir = tempfile::tempdir().unwrap();
    let exe = write_exe(dir.path());
    let found = discover_network_version(&NetworkVersionQuery {
        user_value: Some("  9.8.7.6 "),
        client_executable: Some(&exe),
        resource_key: Some("SyntheticBuild"),
    });
    assert_eq!(
        found,
        NetworkVersionDiscovery::Found {
            version: "9.8.7.6".to_owned(),
            source: NetworkVersionSource::UserSupplied,
        }
    );
}

#[test]
fn invalid_user_value_asks_the_user_and_skips_the_resource() {
    let dir = tempfile::tempdir().unwrap();
    let exe = write_exe(dir.path());
    for bad in [
        "has space",
        "x".repeat(65).as_str(),
        "tab\there",
        "non-ascii-é",
    ] {
        let outcome = discover_network_version(&NetworkVersionQuery {
            user_value: Some(bad),
            client_executable: Some(&exe),
            resource_key: Some("SyntheticBuild"),
        });
        assert_eq!(
            outcome,
            NetworkVersionDiscovery::AskUser(AskUserReason::UserValueInvalid),
            "{bad:?}"
        );
    }
}

#[test]
fn empty_user_value_counts_as_not_supplied() {
    let dir = tempfile::tempdir().unwrap();
    let exe = write_exe(dir.path());
    for empty in ["", "   ", "\t\n"] {
        assert_eq!(
            discover_network_version(&NetworkVersionQuery {
                user_value: Some(empty),
                client_executable: Some(&exe),
                resource_key: Some("SyntheticBuild"),
            }),
            NetworkVersionDiscovery::Found {
                version: "4321.0001.0002.0003".to_owned(),
                source: NetworkVersionSource::VersionResource {
                    key: "SyntheticBuild".to_owned()
                },
            },
            "{empty:?}"
        );
    }
    assert_eq!(
        discover_network_version(&NetworkVersionQuery {
            user_value: Some(" "),
            ..NetworkVersionQuery::default()
        }),
        NetworkVersionDiscovery::AskUser(AskUserReason::NoResourceKey)
    );
}

#[test]
fn resource_entry_named_by_the_caller_is_used() {
    let dir = tempfile::tempdir().unwrap();
    let exe = write_exe(dir.path());
    let found = discover_network_version(&NetworkVersionQuery {
        user_value: None,
        client_executable: Some(&exe),
        resource_key: Some("SyntheticBuild"),
    });
    assert_eq!(
        found,
        NetworkVersionDiscovery::Found {
            version: "4321.0001.0002.0003".to_owned(),
            source: NetworkVersionSource::VersionResource {
                key: "SyntheticBuild".to_owned()
            },
        }
    );
}

#[test]
fn no_default_key_and_every_gap_asks_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let exe = write_exe(dir.path());

    // No key named: the resource is not consulted even though it exists.
    assert_eq!(
        discover_network_version(&NetworkVersionQuery {
            user_value: None,
            client_executable: Some(&exe),
            resource_key: None,
        }),
        NetworkVersionDiscovery::AskUser(AskUserReason::NoResourceKey)
    );
    assert_eq!(
        discover_network_version(&NetworkVersionQuery::default()),
        NetworkVersionDiscovery::AskUser(AskUserReason::NoResourceKey)
    );
    assert_eq!(
        discover_network_version(&NetworkVersionQuery {
            user_value: None,
            client_executable: None,
            resource_key: Some("SyntheticBuild"),
        }),
        NetworkVersionDiscovery::AskUser(AskUserReason::NoExecutable)
    );
    assert_eq!(
        discover_network_version(&NetworkVersionQuery {
            user_value: None,
            client_executable: Some(&exe),
            resource_key: Some("Missing"),
        }),
        NetworkVersionDiscovery::AskUser(AskUserReason::ResourceEntryMissing {
            key: "Missing".to_owned()
        })
    );
    // "FileDescription" holds a space and "Empty" nothing: neither is a version.
    for key in ["FileDescription", "Empty"] {
        assert_eq!(
            discover_network_version(&NetworkVersionQuery {
                user_value: None,
                client_executable: Some(&exe),
                resource_key: Some(key),
            }),
            NetworkVersionDiscovery::AskUser(AskUserReason::ResourceEntryInvalid {
                key: key.to_owned()
            })
        );
    }

    // An unreadable executable asks the user and carries the reader's error.
    let garbage = dir.path().join("garbage.exe");
    std::fs::write(&garbage, b"MZ").unwrap();
    let outcome = discover_network_version(&NetworkVersionQuery {
        user_value: None,
        client_executable: Some(&garbage),
        resource_key: Some("SyntheticBuild"),
    });
    assert!(matches!(
        outcome,
        NetworkVersionDiscovery::AskUser(AskUserReason::ResourceUnreadable(
            VersionResourceError::Truncated { .. }
        ))
    ));
}

#[test]
fn network_version_validation_and_resource_lookup() {
    assert!(is_valid_network_version("1.2.3.4"));
    assert!(is_valid_network_version(&"9".repeat(64)));
    assert!(!is_valid_network_version(&"9".repeat(65)));
    assert!(!is_valid_network_version(""));
    assert!(!is_valid_network_version("a;b c"));

    let resource = VersionResource {
        strings: vec![tessera_compat::version_resource::VersionString {
            table: "0".to_owned(),
            key: "V".to_owned(),
            value: " 1.2 ".to_owned(),
        }],
        ..VersionResource::default()
    };
    assert_eq!(
        network_version_from_resource(&resource, "v"),
        NetworkVersionDiscovery::Found {
            version: "1.2".to_owned(),
            source: NetworkVersionSource::VersionResource {
                key: "v".to_owned()
            },
        }
    );
}

#[test]
fn version_resource_errors_map_to_config_invalid() {
    let err = LoginError::from(VersionResourceError::NoVersionResource);
    assert_eq!(err.failure_class(), LoginFailureClass::F02_CONFIG_INVALID);
    assert!(err.to_string().contains("version resource"));
}
