// SPDX-License-Identifier: GPL-3.0-or-later
//! The generic tagfile reader (`docs/specs/havok-data.md` §3, structure kinds K1–K5 of §11)
//! on arbitrary bytes.
//!
//! The first byte picks a mode:
//!
//! - 0: raw bytes after the mode byte;
//! - 1: a tagfile from `tessera_havok::testkit` (minimal skeleton, full skeleton, spline,
//!   interleaved or rich clip tagfile, reference frame), then damaged by edits and a cut;
//! - 2: a tagfile generated op by op from the input: class declarations with any parent
//!   index, type code (valid or not), tuple size and late-resolved class name, objects whose
//!   values follow their declared types (struct arrays with columns, references forward,
//!   null or dangling), raw tags and raw bytes.
//!
//! Nothing may panic. A tagfile that parses must be internally consistent.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_havok::Tagfile;
use tessera_havok::tagfile::Value;
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, ByteSource, MemberDef, SkeletonSpec, TagfileWriter, WValue,
    damage, example_interleaved_12_6, example_reference_frame_12_7, example_rich_record,
    example_skeleton_12_1, example_spline_12_3,
};

const NAMES: &[&str] = &[
    "A",
    "B",
    "hkaBone",
    "x",
    "name",
    "s",
    "",
    "hkaSkeleton",
    "C",
];
const TYPES: &[i32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 0x22,
    0x2A, 0x31, 0x40, -1,
];

fn check(bytes: &[u8]) {
    let Ok(tf) = Tagfile::parse(bytes) else {
        return;
    };
    let classes = tf.classes();
    for (i, o) in tf.objects().iter().enumerate() {
        assert!(o.class as usize >= 1 && (o.class as usize) < classes.len());
        let full = classes[o.class as usize].full_len;
        let mut last = None;
        for (idx, v) in &o.values {
            assert!(*idx < full);
            assert!(last.is_none_or(|l| l < *idx), "values in full-list order");
            last = Some(*idx);
            match v {
                Value::Object(r) => assert!(*r as usize <= tf.objects().len()),
                Value::StructArray(sa) => {
                    assert!((sa.class as usize) < classes.len());
                    for (_, col) in &sa.columns {
                        let n = match col {
                            Value::Bytes(b) => b.len(),
                            Value::IntArray { values, .. } => values.len(),
                            Value::RealArray(v) => v.len(),
                            Value::Vec4Array { values, .. } => values.len(),
                            Value::Vec12Array(v) => v.len(),
                            Value::CStringArray(v) => v.len(),
                            other => panic!("non-array column {other:?}"),
                        };
                        assert_eq!(n, sa.len);
                    }
                }
                _ => {}
            }
        }
        assert!(tf.object(i as u32 + 1).is_some_and(|x| std::ptr::eq(x, o)));
        for m in &classes[o.class as usize].members {
            assert!(tf.find_member(o.class, tf.string(m.name)).is_some());
            assert!(!(m.ty.is_array() && m.ty.is_tuple()));
        }
    }
    let _ = tf.root();
}

fn base_tagfile(src: &mut ByteSource<'_>) -> Vec<u8> {
    match src.below(6) {
        0 => example_skeleton_12_1(),
        1 => SkeletonSpec::chain(src.below(12)).to_tagfile(),
        2 => AnimationSpec::new(AnimationKindSpec::Spline(example_spline_12_3())).to_tagfile(),
        3 => AnimationSpec::new(AnimationKindSpec::Interleaved(example_interleaved_12_6()))
            .to_tagfile(),
        4 => example_rich_record().animation.to_tagfile(),
        _ => example_reference_frame_12_7(3 + src.below(2) as u8).to_tagfile(),
    }
}

fn floats<const N: usize>(src: &mut ByteSource<'_>) -> [f32; N] {
    std::array::from_fn(|_| src.f32())
}

/// A value matching `m`'s declared type (as far as the writer can express it); struct
/// arrays only when the element class is declared, else an int (which then misparses).
fn value(w: &TagfileWriter, m: &MemberDef, src: &mut ByteSource<'_>, depth: u8) -> WValue {
    let n = src.below(5);
    let kind = m.ty & 0x0F;
    let array = m.ty & 0x10 != 0;
    match (kind, array) {
        (1, false) => WValue::Byte(src.u8()),
        (2, false) => WValue::Int(i32::from(src.u16() as i16)),
        (3, false) => WValue::Real(src.f32()),
        (4, false) => WValue::Vec4(floats(src)),
        (6, false) => WValue::Vec12(floats(src)),
        (8, false) => WValue::Object(src.below(5) as i32 - 1),
        (10, false) => WValue::CString(NAMES[src.below(NAMES.len())].into()),
        (1, true) => WValue::Bytes((0..n).map(|_| src.u8()).collect()),
        (2, true) => WValue::IntArray((0..n).map(|_| i32::from(src.u16() as i16)).collect()),
        (3, true) => WValue::RealArray((0..n).map(|_| src.f32()).collect()),
        (4, true) => WValue::Vec4Array {
            components: src.below(6) as u8,
            values: (0..n).map(|_| floats(src)).collect(),
        },
        (6, true) => WValue::Vec12Array((0..n).map(|_| floats(src)).collect()),
        (10, true) => WValue::CStringArray(
            (0..n)
                .map(|_| NAMES[src.below(NAMES.len())].into())
                .collect(),
        ),
        (9, true) if depth < 2 => {
            let Some(class) = m.class_name.as_deref().and_then(|c| w.class_index(c)) else {
                return WValue::Int(0);
            };
            let mut columns = Vec::new();
            for fm in w.full_members(class) {
                if src.u8() & 1 == 1 && !columns.iter().any(|(c, _)| c == &fm.name) {
                    // Columns are written like arrays of the member's type.
                    let col_def = MemberDef {
                        ty: fm.ty | 0x10,
                        ..fm.clone()
                    };
                    let mut v = value(w, &col_def, src, depth + 1);
                    // Make the column length match the element count most of the time.
                    if !src.u8().is_multiple_of(4) {
                        v = resize(v, n);
                    }
                    columns.push((fm.name.clone(), v));
                }
            }
            WValue::StructArray { len: n, columns }
        }
        _ => WValue::Int(i32::from(src.u8())),
    }
}

fn resize(v: WValue, n: usize) -> WValue {
    match v {
        WValue::Bytes(mut b) => {
            b.resize(n, 0);
            WValue::Bytes(b)
        }
        WValue::IntArray(mut a) => {
            a.resize(n, 0);
            WValue::IntArray(a)
        }
        WValue::RealArray(mut a) => {
            a.resize(n, 0.0);
            WValue::RealArray(a)
        }
        WValue::Vec4Array {
            components,
            mut values,
        } => {
            values.resize(n, [0.0; 4]);
            WValue::Vec4Array { components, values }
        }
        WValue::Vec12Array(mut a) => {
            a.resize(n, [0.0; 12]);
            WValue::Vec12Array(a)
        }
        WValue::CStringArray(mut a) => {
            a.resize(n, "x".into());
            WValue::CStringArray(a)
        }
        other => other,
    }
}

fn generated(src: &mut ByteSource<'_>) -> Vec<u8> {
    let mut w = if src.u8().is_multiple_of(16) {
        let mut w = TagfileWriter::empty();
        w.raw(&tessera_havok::tagfile::MAGIC)
            .varint(src.below(5) as i32);
        w
    } else {
        TagfileWriter::new()
    };
    let mut classes = 0usize;
    for _ in 0..24 {
        if src.is_empty() {
            break;
        }
        match src.below(8) {
            0..=2 => {
                let name = NAMES[src.below(NAMES.len())];
                let parent = src.below(classes + 2);
                let members: Vec<MemberDef> = (0..src.below(5))
                    .map(|_| MemberDef {
                        name: NAMES[src.below(NAMES.len())].into(),
                        ty: TYPES[src.below(TYPES.len())],
                        tuple_size: src.below(4) as i32,
                        class_name: Some(NAMES[src.below(NAMES.len())].into()),
                    })
                    .collect();
                w.class(name, src.below(4) as i32, parent, &members);
                classes += 1;
            }
            3..=5 if classes > 0 => {
                let class = 1 + src.below(classes);
                let mut values = Vec::new();
                for m in w.full_members(class) {
                    if src.u8() & 1 == 1 && !values.iter().any(|(n, _)| n == &m.name) {
                        let v = value(&w, &m, src, 0);
                        values.push((m.name.clone(), v));
                    }
                }
                w.object(class, &values);
            }
            6 => {
                w.varint(i32::from(src.u8() as i8));
            }
            _ => {
                let n = src.below(6);
                let raw: Vec<u8> = (0..n).map(|_| src.u8()).collect();
                w.raw(&raw);
            }
        }
    }
    if src.u8().is_multiple_of(8) {
        w.into_bytes()
    } else {
        w.finish()
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 3 {
        0 => check(rest),
        1 => {
            let mut src = ByteSource::new(rest);
            let mut bytes = base_tagfile(&mut src);
            let changed = damage(&mut bytes, &mut src);
            if !changed {
                assert!(Tagfile::parse(&bytes).is_ok(), "testkit tagfile must parse");
            }
            check(&bytes);
        }
        _ => {
            let mut src = ByteSource::new(rest);
            let bytes = generated(&mut src);
            check(&bytes);
        }
    }
});
