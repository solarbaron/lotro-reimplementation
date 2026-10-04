// SPDX-License-Identifier: GPL-3.0-or-later
//! DID → archive routing (spec §8).
//!
//! The table is data, not code: it is parsed from a small text format so that it can be
//! replaced or extended without a code change (spec §8.3). [`RoutingTable::default`] is
//! seeded from the observed placement in spec §8.1.

use std::fmt;
use std::str::FromStr;

use crate::Did;
use crate::error::{Error, Result};

/// Archive kinds a routing rule can name (all parts of the kind are searched).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KindTag {
    /// `client_general.dat`
    General,
    /// `client_gamelogic.dat`
    Gamelogic,
    /// `client_local_<Language>.dat` (any language)
    Local,
    /// `client_mesh*.dat[x]`
    Mesh,
    /// `client_surface*.dat[x]`
    Surface,
    /// `client_highres*.dat[x]`
    HighRes,
    /// `client_sound*.dat[x]`
    Sound,
    /// `client_anim.dat`
    Anim,
    /// `client_cell_<R>.dat` (all regions)
    Cell,
    /// `client_map_<R>.dat` (all regions)
    Map,
}

impl KindTag {
    /// Every kind, in declaration order (for CLI help and value enumeration).
    pub const ALL: [Self; 10] = [
        Self::General,
        Self::Gamelogic,
        Self::Local,
        Self::Mesh,
        Self::Surface,
        Self::HighRes,
        Self::Sound,
        Self::Anim,
        Self::Cell,
        Self::Map,
    ];

    /// The name used in file names and in the routing text format.
    pub fn name(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Gamelogic => "gamelogic",
            Self::Local => "local",
            Self::Mesh => "mesh",
            Self::Surface => "surface",
            Self::HighRes => "highres",
            Self::Sound => "sound",
            Self::Anim => "anim",
            Self::Cell => "cell",
            Self::Map => "map",
        }
    }
}

impl fmt::Display for KindTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for KindTag {
    type Err = Error;

    /// Parses a kind name as written by [`KindTag::name`]; fails with
    /// [`Error::UnknownKind`].
    fn from_str(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .ok_or_else(|| Error::UnknownKind { name: s.to_owned() })
    }
}

/// Where a routing rule sends a DID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteTarget {
    /// Every part of the given kind (for `cell`/`map`: every region).
    Kind(KindTag),
    /// The `cell` or `map` archive whose region equals the low nibble of the DID's second
    /// byte (spec §8.1: second byte = `(class << 4) | region`). Written `cell@` / `map@`.
    Region(KindTag),
}

impl fmt::Display for RouteTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kind(k) => f.write_str(k.name()),
            Self::Region(k) => write!(f, "{}@", k.name()),
        }
    }
}

/// One routing rule: a range of DID high bytes (optionally restricted to one class nibble)
/// and the archive kinds to search, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRule {
    /// Lowest DID high byte matched (inclusive).
    pub lo: u8,
    /// Highest DID high byte matched (inclusive).
    pub hi: u8,
    /// If set, the high nibble of the DID's second byte must equal this class.
    pub class: Option<u8>,
    /// Archive kinds to search, in order.
    pub targets: Vec<RouteTarget>,
}

impl RouteRule {
    /// Whether this rule applies to `did`.
    pub fn matches(&self, did: Did) -> bool {
        let [high, second, _, _] = did.to_be_bytes();
        (self.lo..=self.hi).contains(&high) && self.class.is_none_or(|c| second >> 4 == c)
    }
}

/// Ordered list of [`RouteRule`]s; the first matching rule wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingTable {
    rules: Vec<RouteRule>,
}

/// Default routing text, seeded from spec §8.1 (observed placement, 2026-10-03).
///
/// Format, one rule per line (`#` starts a comment):
/// `HH[-HH][:C]  target [target ...]` where `HH` are hexadecimal DID high bytes, `C` an
/// optional hexadecimal class nibble (high nibble of the DID's second byte), and each
/// target an archive kind (`general`, `gamelogic`, `local`, `mesh`, `surface`, `highres`,
/// `sound`, `anim`, `cell`, `map`), optionally suffixed with `@` to select the region given
/// by the low nibble of the DID's second byte.
pub const DEFAULT_ROUTING: &str = "\
# Seeded from docs/specs/dat-container.md section 8.1. Order = search order.
01-02 general
04 general
05 anim
06 mesh
07 gamelogic
08 general
0A sound local
0C gamelogic
0E general local
0F general
11-12 general
14-1A general
1D general local
1F-21 general
22 local general
23 general
25-26 local
27 cell@
28 general
2A-2B general
2D general
30 general local
31-33 general
34 gamelogic
35 general
39 general
40 general local
41 surface highres local
42 local general
43 gamelogic
44 general
47 gamelogic
51 general
56 gamelogic
58 local
66 general
70-77 gamelogic
78-7F gamelogic local
80:1 map@
80 cell@
";

impl Default for RoutingTable {
    fn default() -> Self {
        // The built-in text is covered by a unit test; fall back to an empty table
        // (search-everything) rather than panic if it were ever broken.
        Self::parse(DEFAULT_ROUTING).unwrap_or_else(|e| {
            tracing::error!(error = %e, "built-in routing table does not parse");
            Self { rules: Vec::new() }
        })
    }
}

fn hex_u8(s: &str, line: usize) -> Result<u8> {
    u8::from_str_radix(s, 16).map_err(|_| Error::Routing {
        line,
        reason: format!("`{s}` is not a hexadecimal byte"),
    })
}

impl RoutingTable {
    /// A table without rules: every lookup falls back to searching all archives.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Parses the text format described at [`DEFAULT_ROUTING`].
    pub fn parse(text: &str) -> Result<Self> {
        let mut rules = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = i + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let mut words = content.split_whitespace();
            let key = words.next().unwrap_or("");
            let (range, class) = match key.split_once(':') {
                Some((r, c)) => {
                    let c = hex_u8(c, line)?;
                    if c > 0xF {
                        return Err(Error::Routing {
                            line,
                            reason: format!("class {c:#x} is not a nibble"),
                        });
                    }
                    (r, Some(c))
                }
                None => (key, None),
            };
            let (lo, hi) = match range.split_once('-') {
                Some((a, b)) => (hex_u8(a, line)?, hex_u8(b, line)?),
                None => {
                    let v = hex_u8(range, line)?;
                    (v, v)
                }
            };
            if lo > hi {
                return Err(Error::Routing {
                    line,
                    reason: format!("empty range {lo:02X}-{hi:02X}"),
                });
            }
            let mut targets = Vec::new();
            for w in words {
                let (name, region) = match w.strip_suffix('@') {
                    Some(n) => (n, true),
                    None => (w, false),
                };
                let kind = name.parse::<KindTag>().map_err(|_| Error::Routing {
                    line,
                    reason: format!("unknown archive kind `{name}`"),
                })?;
                if region && !matches!(kind, KindTag::Cell | KindTag::Map) {
                    return Err(Error::Routing {
                        line,
                        reason: format!("`{name}@`: only cell and map have regions"),
                    });
                }
                targets.push(if region {
                    RouteTarget::Region(kind)
                } else {
                    RouteTarget::Kind(kind)
                });
            }
            if targets.is_empty() {
                return Err(Error::Routing {
                    line,
                    reason: "rule has no targets".into(),
                });
            }
            rules.push(RouteRule {
                lo,
                hi,
                class,
                targets,
            });
        }
        Ok(Self { rules })
    }

    /// The rules, in match order.
    pub fn rules(&self) -> &[RouteRule] {
        &self.rules
    }

    /// The targets of the first rule matching `did` (empty if no rule matches).
    pub fn targets(&self, did: Did) -> &[RouteTarget] {
        self.rules
            .iter()
            .find(|r| r.matches(did))
            .map_or(&[], |r| r.targets.as_slice())
    }

    /// The region number encoded in a cell/map DID (low nibble of the second byte).
    pub fn region_of(did: Did) -> u32 {
        u32::from(did.to_be_bytes()[1] & 0x0F)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_parses_and_routes() {
        let t = RoutingTable::parse(DEFAULT_ROUTING).unwrap();
        assert_eq!(t, RoutingTable::default());
        assert_eq!(
            t.targets(0x0100_0001),
            &[RouteTarget::Kind(KindTag::General)]
        );
        assert_eq!(t.targets(0x8013_0000), &[RouteTarget::Region(KindTag::Map)]);
        assert_eq!(
            t.targets(0x8043_0000),
            &[RouteTarget::Region(KindTag::Cell)]
        );
        assert_eq!(RoutingTable::region_of(0x805E_1234), 0xE);
        assert_eq!(t.targets(0x7A00_0000).len(), 2);
        assert!(t.targets(0x9900_0000).is_empty());
    }

    #[test]
    fn kind_tag_names_round_trip() {
        for k in KindTag::ALL {
            assert_eq!(k.to_string().parse::<KindTag>().unwrap(), k);
            assert_eq!(k.to_string(), k.name());
        }
        let err = "nowhere".parse::<KindTag>().unwrap_err();
        assert!(matches!(err, Error::UnknownKind { ref name } if name == "nowhere"));
        // Usable with `?` in code returning a boxed std error.
        let boxed: Box<dyn std::error::Error> = err.into();
        assert!(boxed.to_string().contains("nowhere"));
    }

    #[test]
    fn parse_errors() {
        for bad in [
            "zz general",
            "01 nowhere",
            "01",
            "05-01 general",
            "01 general@",
            "80:10 cell",
        ] {
            assert!(RoutingTable::parse(bad).is_err(), "{bad}");
        }
        assert!(
            RoutingTable::parse("# only a comment\n\n")
                .unwrap()
                .rules()
                .is_empty()
        );
    }
}
