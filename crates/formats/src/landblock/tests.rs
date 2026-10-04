// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests for the stamp rules and failure categories (synthetic values only).

use super::*;

#[test]
fn tree_build_byte_rule_by_stamp() {
    for s in [7, 20, 22, 27, 29, 31, 33] {
        assert_eq!(
            tree_build_byte_rule(Some(s)),
            (Some(false), LayoutRule::Stamp)
        );
    }
    for s in [38, 39, 40, 1000] {
        assert_eq!(
            tree_build_byte_rule(Some(s)),
            (Some(true), LayoutRule::Stamp)
        );
    }
    for s in 34..=37 {
        assert_eq!(
            tree_build_byte_rule(Some(s)),
            (None, LayoutRule::UnobservedStamp)
        );
    }
    assert_eq!(tree_build_byte_rule(None), (None, LayoutRule::Fallback));
}

#[test]
fn info_trailer_rule_by_stamp() {
    for s in [7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29] {
        assert_eq!(info_trailer_rule(Some(s)), (Some(false), LayoutRule::Stamp));
    }
    for s in [31, 32, 33, 38, 39] {
        assert_eq!(info_trailer_rule(Some(s)), (Some(true), LayoutRule::Stamp));
    }
    assert_eq!(
        info_trailer_rule(Some(30)),
        (None, LayoutRule::UnobservedStamp)
    );
    assert_eq!(info_trailer_rule(None), (None, LayoutRule::Fallback));
}

#[test]
fn failure_categories() {
    let wrapped = FormatError::PropertyValue {
        pid: 1,
        type_code: 6,
        offset: 0,
        source: Box::new(FormatError::Truncated {
            offset: 0,
            needed: 4,
            available: 0,
        }),
    };
    assert_eq!(FailureCategory::of(&wrapped), FailureCategory::PropertySet);
    let count = FormatError::CountLimit {
        what: "static entity count",
        offset: 0,
        count: 2,
        limit: 1,
    };
    assert_eq!(FailureCategory::of(&count), FailureCategory::Limit);
    // Classification is structural: message text does not matter.
    let renamed = FormatError::malformed("property-ish count", 0, "x");
    assert_eq!(FailureCategory::of(&renamed), FailureCategory::Malformed);
    let truncated = FormatError::Truncated {
        offset: 0,
        needed: 4,
        available: 0,
    };
    let in_set = |source: FormatError| FormatError::InPropertySet {
        offset: 0,
        source: Box::new(source),
    };
    assert_eq!(FailureCategory::of(&truncated), FailureCategory::Truncated);
    assert_eq!(
        FailureCategory::of(&in_set(truncated.clone())),
        FailureCategory::PropertySet
    );
    let in_record = FormatError::InRecord {
        did: 1,
        source: Box::new(in_set(truncated)),
    };
    assert_eq!(
        FailureCategory::of(&in_record),
        FailureCategory::PropertySet
    );
    let mem_in_set = in_set(FormatError::MemoryLimit {
        offset: 0,
        limit: 1,
    });
    assert_eq!(FailureCategory::of(&mem_in_set), FailureCategory::Limit);
    let nan = FormatError::malformed("static entity placement", 0, "NaN");
    assert_eq!(FailureCategory::of(&nan), FailureCategory::Malformed);
    let mem = FormatError::MemoryLimit {
        offset: 0,
        limit: 1,
    };
    assert_eq!(FailureCategory::of(&mem), FailureCategory::Limit);
    assert_eq!(FailureCategory::BadCount.name(), "BadCount");
}

#[test]
fn constants_agree() {
    assert_eq!(GRID_SAMPLES, 1089);
    assert_eq!(QUAD_COUNT, 1024);
    assert_eq!(COARSE_SAMPLES, 289);
    assert_eq!(vertex_to_height_index(1088), Some(1088));
    assert_eq!(vertex_xy(34), Some((1, 1)));
}
