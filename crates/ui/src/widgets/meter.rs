// SPDX-License-Identifier: GPL-3.0-or-later
//! Meter (`ui-widgets.md` §4).

use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::ElementDesc;

use crate::events::MsgArg;
use crate::geom::{sub, trunc_f32, trunc_f64};
use crate::props::{array_elements, as_key};
use crate::tree::{Element, ElementKey};
use crate::ui::Ui;
use crate::widgets::WidgetClass;

/// Fill geometry of a child meter (§4.3): ((fx, fy, fw, fh), (ix, iy, iw, ih)).
#[must_use]
pub fn child_fill(
    direction: u32,
    move_fill: bool,
    p: f32,
    offset: i32,
    w: i32,
    h: i32,
) -> ([i32; 4], [i32; 4]) {
    let (mut fx, mut fy, mut fw, mut fh) = (0, 0, w, h);
    let (mut ix, mut iy, mut iw, mut ih) = (0, 0, w, h);
    let o = offset;
    let (wf, hf) = (w as f32, h as f32);
    match (direction, move_fill) {
        (4, true) => fx = trunc_f32(0.0 - (1.0 - p) * wf),
        (4, false) => {
            fx = o;
            fw = sub(trunc_f32(wf * p), o);
            iw = fw;
        }
        (3, true) => fx = trunc_f32((1.0 - p) * wf),
        (3, false) => {
            fw = trunc_f32(wf * p);
            fx = sub(w, fw);
            ix = sub(o, fx);
            iw = sub(w, o);
        }
        (2, true) => fy = trunc_f32(0.0 - (1.0 - p) * hf),
        (2, false) => {
            fy = o;
            fh = sub(trunc_f32(hf * p), o);
            ih = fh;
        }
        (1, true) => fy = trunc_f32((1.0 - p) * hf),
        (1, false) => {
            fh = trunc_f32(hf * p);
            fy = sub(h, fh);
            iy = sub(o, fy);
            iw = sub(w, o);
        }
        _ => {}
    }
    ([fx, fy, fw, fh], [ix, iy, iw, ih])
}

/// Frame index of a frame meter (§4.4 step 2).
#[must_use]
pub fn frame_index(n: usize, p: f32) -> i32 {
    if n == 0 {
        return 0;
    }
    trunc_f64(f64::from((n - 1) as f32 * p) + 0.5)
}

impl Ui {
    /// Meter post-creation (§4.2).
    pub(crate) fn meter_post_create(&mut self, k: ElementKey) {
        let p = self.v.pids;
        if self.prop(k, p.meter_child).bool() {
            self.meter_create_parts(k);
        }
        if self.prop(k, p.meter_alphatest).bool()
            && let Some(m) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut())
        {
            m.alpha_test = true;
        }
        self.meter_update(k);
    }

    fn make_part(&mut self, parent: ElementKey, id: u32, w: i32, h: i32) -> Option<ElementKey> {
        let mut desc = ElementDesc {
            element_id: id,
            element_type: self.v.types.field,
            ..ElementDesc::default()
        };
        desc.block.rect.width = w;
        desc.block.rect.height = h;
        let mut e = Element::new(id, WidgetClass::Field, Arc::new(desc));
        e.created = true;
        let k = self.insert_element(e);
        // `UICore_Element_mouse_invisible` has no reader (UB45); the parts stay not
        // mouse-visible by the base rule.
        if self.attach_raw(k, parent).is_err() {
            self.destroy(k);
            return None;
        }
        Some(k)
    }

    fn meter_create_parts(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let dir = self.prop(k, p.meter_direction).key();
        let img = self.prop(k, p.meter_child_image).key();
        let mode = self.prop(k, p.meter_child_drawmode).key();
        if dir == 0 || img == 0 || mode == 0 {
            tracing::debug!("child meter without direction, image or draw mode: no parts");
            return;
        }
        let Some(b) = self.layout_box(k) else { return };
        let ids = self.v.ids;
        let Some(fill) = self.make_part(k, ids.meter_fill, b.w(), b.h()) else {
            return;
        };
        let Some(image) = self.make_part(fill, ids.meter_fill_image, b.w(), b.h()) else {
            return;
        };
        self.set_base_did(image, img, mode, true);
        if let Some(m) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut()) {
            m.fill = Some(fill);
            m.fill_image = Some(image);
        }
    }

    /// Updates the meter's parts (§4.3–§4.5).
    pub(crate) fn meter_update(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let Some(m) = self.el(k).and_then(|e| e.widget.meter.clone()) else {
            return;
        };
        let pos = self.prop(k, p.meter_position).float();
        let b = self.layout_box(k).unwrap_or_default();
        if let (Some(fill), Some(image)) = (m.fill, m.fill_image) {
            let dir = self.prop(k, p.meter_direction).key();
            let mv = self.prop(k, p.meter_move_fill).bool();
            let off = self.prop(k, p.meter_offset).int();
            let (f, i) = child_fill(dir, mv, pos, off, b.w(), b.h());
            // Tessera rule (§4.3 OPEN): a size below 1 counts as 0, the position is kept.
            let clamp = |v: i32| v.max(0);
            self.resize_layout(fill, clamp(f[2]), clamp(f[3]));
            self.move_to(fill, f[0], f[1]);
            self.resize_layout(image, clamp(i[2]), clamp(i[3]));
            self.move_to(image, i[0], i[1]);
        }
        if self.prop(k, p.meter_frame).bool() {
            let frames: Vec<u32> = self
                .prop(k, p.meter_frame_array)
                .value()
                .map(|v| array_elements(v).iter().map(|e| as_key(&e.value)).collect())
                .unwrap_or_default();
            if frames.is_empty() {
                tracing::debug!("frame meter without frames");
            } else {
                // Steps 2–4: the signed index is compared and remembered as computed; an
                // index outside the array (p < 0 or p > 1) leaves the image unchanged.
                let i = frame_index(frames.len(), pos);
                if m.frame != Some(i) {
                    if let Some(mp) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut()) {
                        mp.frame = Some(i);
                    }
                    let entry = usize::try_from(i).ok().and_then(|idx| frames.get(idx));
                    match entry {
                        Some(did) if *did != 0 => {
                            let dm = self.prop(k, p.meter_draw_mode);
                            let mode = if dm.is_set() { dm.key() } else { 2 };
                            self.set_base_did(k, *did, mode, true);
                        }
                        _ => tracing::debug!("frame meter frame {i} missing"),
                    }
                }
            }
        }
        if m.alpha_test {
            // The threshold 1 − p is a composition value of the meter's surface (§4.5).
            self.painter.touched = true;
        }
    }

    /// The alpha threshold of an alpha-test meter (§4.5): 1 − p.
    #[must_use]
    pub fn meter_alpha_threshold(&self, k: ElementKey) -> Option<f32> {
        let m = self.el(k)?.widget.meter.as_ref()?;
        m.alpha_test
            .then(|| 1.0 - self.prop(k, self.v.pids.meter_position).float())
    }

    /// Position change (§4.6).
    pub(crate) fn meter_position_changed(&mut self, k: ElementKey) {
        let p = self.v.pids;
        self.meter_update(k);
        let v = self.prop(k, p.meter_position).float();
        let last = self
            .el(k)
            .and_then(|e| e.widget.meter.as_ref())
            .map_or(0.0, |m| m.last);
        if (v - last).abs() > 0.0002 {
            if let Some(m) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut()) {
                m.last = v;
            }
            if self.prop(k, p.meter_notify).bool() {
                self.post(k, self.v.msgs.meter_new_position, MsgArg::default());
            }
        }
        let goal = self.prop(k, p.meter_goal).float();
        if (v - goal).abs() <= 0.0002 {
            self.post(k, self.v.msgs.meter_goal_reached, MsgArg::default());
        }
    }

    /// Goal change (§4.6).
    pub(crate) fn meter_goal_changed(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let g = self.prop(k, p.meter_goal).float();
        if !self.prop(k, p.meter_smooth).bool() {
            self.set_property(k, p.meter_position, PropertyValue::Float(g));
            return;
        }
        if let Some(m) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut()) {
            m.anim = None;
        }
        let d = self.prop(k, p.meter_smooth_duration).float();
        if d == 0.0 {
            return;
        }
        let start = self.prop(k, p.meter_position).float();
        let now = self.now;
        if let Some(m) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut()) {
            m.anim = Some((start, g, now, now + f64::from(d)));
        }
        self.want_frame(k);
    }

    /// Per-frame smooth movement (§4.6). Returns whether it keeps running.
    pub(crate) fn meter_frame(&mut self, k: ElementKey) -> bool {
        let Some((start, goal, t0, t1)) = self
            .el(k)
            .and_then(|e| e.widget.meter.as_ref())
            .and_then(|m| m.anim)
        else {
            return false;
        };
        let mut t = if t1 - t0 <= 0.0 {
            0.0
        } else {
            ((self.now - t0) / (t1 - t0)) as f32
        };
        let mut more = true;
        if t >= 1.0 {
            t = 1.0;
            more = false;
            if let Some(m) = self.el_mut(k).and_then(|e| e.widget.meter.as_mut()) {
                m.anim = None;
            }
        }
        let v = start + (goal - start) * t;
        self.set_property(k, self.v.pids.meter_position, PropertyValue::Float(v));
        more
    }
}
