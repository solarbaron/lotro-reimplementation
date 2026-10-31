// SPDX-License-Identifier: GPL-3.0-or-later
//! Dev-only helpers for Tessera's tests. Never a normal dependency: only `[dev-dependencies]`
//! name this crate, so no shipped binary links it.
//!
//! # Census baselines (decision 0045)
//!
//! Real-data tests count records, sites and categories of the user's install. A game update
//! changes those counts, so a count is checked against a **local baseline of the install
//! version**, kept outside every git work tree, never against a number written into the
//! test. Rule checks (no failures, no misclassified records, invariants) are not counts and
//! stay plain assertions.
//!
//! - **Location:** `${TESSERA_CENSUS_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/census}/
//!   <package>/<test binary>/<function>/<install version>.tsv`; the install version is the
//!   short level-1 content fingerprint (`install-layout.md` §9.4), else `schema-` and the
//!   short level-0 one, as for the tier-2 goldens of decision 0031. A directory inside a git
//!   work tree is refused (the nearest existing ancestor is canonicalised and it and every
//!   ancestor are checked for a `.git` entry).
//! - **Baseline present:** every recorded name must match it exactly; a missing or extra
//!   name, or any differing value, fails with a table of name, baseline and observed.
//! - **No baseline yet** (first run on an install version): the baseline is written
//!   atomically and the census passes. A drift report is printed against the newest previous
//!   install version's baseline of the same test (the most recently modified sibling file
//!   named like an install version), or, without one, against the spec values the test gave;
//!   the drifted rows go to `<base>/drift/<install version>/<package>/<test>/<function>.tsv`
//!   (one file per test, written atomically, so parallel test binaries never share a file).
//!   `<base>/drift/last-version` holds the version of the latest census.
//! - `TESSERA_CENSUS_STRICT=1` also fails when an observed value differs from its spec value
//!   (for spec writers verifying a table). `TESSERA_CENSUS_REBASE=1` rewrites the baseline
//!   of this install version (after the owner or a builder accepted a change).
//! - **Logs** (standard error, captured by the test harness) carry names, numbers and paths
//!   only.
//!
//! ```no_run
//! # fn census_example(c_len: u32) {
//! # let dir = std::path::PathBuf::from("/install");
//! let mut census = tessera_testkit::census!("bytecode_census", &dir).expect("census");
//! census.count_vs_spec("L", c_len, 1_919_904);
//! census.count("0x0D distinct operands", 1usize);
//! census.finish();
//! # }
//! ```
//!
//! `tools/census-drift.sh` prints the drift rows of one install version.

mod census;
mod version;

#[cfg(test)]
mod tests;

pub use census::{
    Against, Census, CensusError, Count, DriftRow, Failure, Options, Outcome, Row, default_base,
};
pub use version::{check_outside_work_trees, install_version, is_version_name};

/// Opens the census of one test function of the calling crate:
/// `census!(function, game_dir)` is [`Census::open`] with the calling package's name and
/// its test binary's crate name.
#[macro_export]
macro_rules! census {
    ($function:expr, $game_dir:expr $(,)?) => {
        $crate::Census::open(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_CRATE_NAME"),
            $function,
            $game_dir,
        )
    };
}
