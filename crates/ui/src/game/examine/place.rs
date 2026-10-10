// SPDX-License-Identifier: GPL-3.0-or-later
//! Placing examination tooltips (`item-tooltips.md` §9) and their comparison tooltips
//! (§8.3 Tessera rule). Pure arithmetic in screen pixels.

/// The placement of §9: the pointer at `pointer`, the cursor image `cursor` (width,
/// height), the tooltip `size` and the `screen` size.
///
/// 1. *x* = *px* + *cw* when *px* / *SW* < 0.5, else *px* − *w*;
/// 2. *y* = *py* + *ch* when *py* / *SH* < 0.5, else *py* − *h*;
/// 3. a negative *x* or *y* becomes 0;
/// 4. *x* = min(*x*, *SW* − *w*), *y* = min(*y*, *SH* − *h*).
///
/// A screen side that is not positive counts as 1 for the ratio of steps 1–2.
#[must_use]
pub fn place(
    pointer: (i32, i32),
    cursor: (i32, i32),
    size: (i32, i32),
    screen: (i32, i32),
) -> (i32, i32) {
    let axis = |p: i32, c: i32, s: i32, sw: i32| -> i32 {
        // p / sw < 0.5 ⇔ 2·p < sw (sw > 0), in 64 bits.
        let left_half = 2 * i64::from(p) < i64::from(sw.max(1));
        let v = if left_half {
            i64::from(p) + i64::from(c)
        } else {
            i64::from(p) - i64::from(s)
        };
        let v = v.max(0).min(i64::from(sw) - i64::from(s));
        v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    (
        axis(pointer.0, cursor.0, size.0, screen.0),
        axis(pointer.1, cursor.1, size.1, screen.1),
    )
}

/// Where the hover tooltip and its comparison tooltips go (`item-tooltips.md` §8.3
/// Tessera rule): side by side in the order given, on the side of the hover tooltip that
/// faces the screen's centre, tops aligned, the group clamped to the screen as one box.
///
/// `hover` is the hover tooltip's position by [`place`] and `sizes[0]` its size; the other
/// entries of `sizes` are the comparison tooltips. Returns one position per entry of
/// `sizes`.
#[must_use]
pub fn place_group(hover: (i32, i32), sizes: &[(i32, i32)], screen: (i32, i32)) -> Vec<(i32, i32)> {
    let Some((&first, rest)) = sizes.split_first() else {
        return Vec::new();
    };
    if rest.is_empty() {
        return vec![hover];
    }
    let total_w: i64 = sizes.iter().map(|s| i64::from(s.0.max(0))).sum();
    let rest_w: i64 = rest.iter().map(|s| i64::from(s.0.max(0))).sum();
    let max_h: i64 = sizes
        .iter()
        .map(|s| i64::from(s.1.max(0)))
        .max()
        .unwrap_or(0);
    // The hover tooltip's centre against the screen's centre.
    let centre = 2 * i64::from(hover.0) + i64::from(first.0.max(0));
    let to_right = centre < i64::from(screen.0);
    let mut left = if to_right {
        i64::from(hover.0)
    } else {
        i64::from(hover.0) - rest_w
    };
    let mut top = i64::from(hover.1);
    // Clamp the group as one box, in the order of §9 steps 3–4.
    left = left.max(0).min(i64::from(screen.0) - total_w);
    top = top.max(0).min(i64::from(screen.1) - max_h);
    let clamp = |v: i64| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let y = clamp(top);
    let mut out = vec![(0, 0); sizes.len()];
    if to_right {
        // Hover first, then the comparisons to its right.
        let mut x = left;
        for (o, s) in out.iter_mut().zip(sizes) {
            *o = (clamp(x), y);
            x += i64::from(s.0.max(0));
        }
    } else {
        // The comparisons to its left, the first comparison next to the hover tooltip.
        let mut x = left + rest_w;
        out[0] = (clamp(x), y);
        for (o, s) in out.iter_mut().skip(1).zip(rest) {
            x -= i64::from(s.0.max(0));
            *o = (clamp(x), y);
        }
    }
    out
}
