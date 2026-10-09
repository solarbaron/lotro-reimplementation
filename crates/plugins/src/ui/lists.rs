// SPDX-License-Identifier: GPL-3.0-or-later
//! `ListBox`, `TreeView`, `TreeNode` and the list objects `ControlList`, `TreeNodeList` and
//! `MenuItemList` (`lua-plugin-api.md` §8.5, §13.7).
//!
//! The layer lays list boxes out by the list rules of `ui-widgets.md` §8.3 (cell grid,
//! reverse fill, flipped layout, wrap indent), with the plugin members mapped as the
//! module's OPEN notes say, and lays trees out as one column of indented nodes.

use std::collections::HashSet;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};
use tessera_ui::Ui;

use super::control::{child_ids, ctl_arg, ctl_get, set_owner, set_parent, with_ctl, with_ui};
use super::model::{Args, Kind, ListView, ObjData, Styled, int, truthy};
use super::reg::{Cx, class, class_plain, control_ctor, ctl_of, def};
use super::state::{Pending, Shared, State, class_table, obj_id, object_of, push};
use crate::api::{NativeApi, script_error};

/// Default indentation of a tree level, in pixels (Tessera rule).
pub(crate) const DEFAULT_INDENT: i32 = 20;

/// Registry name of the weak-keyed table `tree view → { Filter, Sort }`.
const FUNCS_KEY: &str = "tessera.plugins.ui.treefuncs";

/// Most nodes visited by one tree walk (cycle guard).
const MAX_WALK: usize = 1 << 17;

fn list_box(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::ListBox])
}

fn tree_view(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::TreeView])
}

fn tree_node(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::TreeNode])
}

fn list_obj(d: &ObjData) -> bool {
    matches!(d, ObjData::List(_))
}

fn ls<R>(sh: &Shared, id: u64, f: impl FnOnce(&mut super::model::ListState) -> R) -> Option<R> {
    with_ctl(sh, id, |c| c.list.as_deref_mut().map(f)).flatten()
}

fn ctl_ref(st: &State, id: u64) -> Option<&super::model::Ctl> {
    match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => Some(c),
        _ => None,
    }
}

fn ctl_mut(st: &mut State, id: u64) -> Option<&mut super::model::Ctl> {
    match st.objs.get_mut(&id).map(|o| &mut o.data) {
        Some(ObjData::Ctl(c)) => Some(c),
        _ => None,
    }
}

/// Remove `id` from the list box, tree, node or menu that holds it (found through the
/// record's own owner link, so the cost does not grow with the number of objects), and
/// clear a tree selection of it.
pub(crate) fn forget_item(st: &mut State, id: u64) {
    let (owner, menu_parent, view) = match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => {
            let view = c.node.as_deref().and_then(|_| view_of(st, id));
            (c.owner, None, view)
        }
        Some(ObjData::Menu(_, m)) => (None, m.parent, None),
        _ => return,
    };
    if let Some(o) = owner
        && let Some(c) = ctl_mut(st, o)
    {
        if let Some(l) = c.list.as_deref_mut() {
            l.items.retain(|x| *x != id);
            if l.selected == Some(id) {
                l.selected = None;
            }
        }
        if let Some(t) = c.tree.as_deref_mut() {
            t.nodes.retain(|x| *x != id);
            if t.selected == Some(id) {
                t.selected = None;
            }
        }
        if let Some(n) = c.node.as_deref_mut() {
            n.children.retain(|x| *x != id);
        }
    }
    if let Some(v) = view
        && let Some(t) = ctl_mut(st, v).and_then(|c| c.tree.as_deref_mut())
        && t.selected == Some(id)
    {
        t.selected = None;
    }
    if let Some(p) = menu_parent {
        if let Some(ObjData::Menu(_, m)) = st.objs.get_mut(&p).map(|o| &mut o.data) {
            m.items.retain(|x| *x != id);
        }
        if let Some(ObjData::Menu(_, m)) = st.objs.get_mut(&id).map(|o| &mut o.data) {
            m.parent = None;
        }
    }
}

/// Mark list box or tree `id` for layout (done by [`flush`] before anything reads list
/// geometry, so filling a list costs one layout, not one per item).
pub(crate) fn relayout(sh: &Shared, id: u64) {
    mark(&mut sh.st.borrow_mut(), id);
}

/// [`relayout`] with the state borrowed.
pub(crate) fn mark(st: &mut State, id: u64) {
    if matches!(
        ctl_ref(st, id).map(|c| c.kind),
        Some(Kind::ListBox | Kind::TreeView)
    ) {
        st.dirty_lists.insert(id);
    }
}

/// Lay out every list box and tree marked by [`relayout`]: before hit tests and painting
/// (the client's delivery and update steps end with it, decision 0014 D8), and before every
/// plugin call that may read geometry.
pub(crate) fn flush(sh: &Shared) {
    if sh.st.borrow().dirty_lists.is_empty() {
        return;
    }
    let Ok(mut ui) = sh.ui.try_borrow_mut() else {
        return;
    };
    let mut st = sh.st.borrow_mut();
    flush_with(&mut ui, &mut st);
}

/// [`flush`] with the UI and state borrowed.
pub(crate) fn flush_with(ui: &mut Ui, st: &mut State) {
    // A layout marks nothing itself; the bound is a guard.
    for _ in 0..4 {
        let dirty = std::mem::take(&mut st.dirty_lists);
        if dirty.is_empty() {
            break;
        }
        for id in dirty {
            layout_with(ui, st, id);
        }
    }
}

/// Lay out list box or tree `id` now.
pub(crate) fn layout_with(ui: &mut Ui, st: &mut State, id: u64) {
    st.dirty_lists.remove(&id);
    match ctl_ref(st, id).map(|c| c.kind) {
        Some(Kind::ListBox) => layout_list(ui, st, id),
        Some(Kind::TreeView) => layout_tree(ui, st, id),
        _ => return,
    }
    super::scroll::sync_bars(ui, st, id);
}

/// An `i64` position clamped to the UI's `i32` (plugin-controlled sizes and offsets can
/// push sums past it).
fn c32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// The list rules of `ui-widgets.md` §8.3 for a plugin list box.
fn layout_list(ui: &mut Ui, st: &mut State, id: u64) {
    let Some(c) = ctl_ref(st, id) else { return };
    let Some(l) = c.list.as_deref() else { return };
    let lkey = c.key;
    let (sx, sy) = c.offset;
    let items: Vec<(u64, tessera_ui::ElementKey)> = l
        .items
        .iter()
        .filter_map(|i| ctl_ref(st, *i).map(|ic| (*i, ic.key)))
        .filter(|(_, k)| ui.control_parent(*k) == Some(lkey))
        .collect();
    let n = items.len() as i64;
    let horizontal = l.orientation == 0;
    let (cols, rows) = if horizontal {
        let mc = if l.max_per_line > 0 {
            l.max_per_line
        } else {
            l.max_columns
        };
        let cols = if mc <= 0 { n } else { i64::from(mc).min(n) };
        let mut rows = if cols == 0 { 0 } else { (n + cols - 1) / cols };
        if l.max_rows > 0 {
            rows = rows.min(i64::from(l.max_rows));
        }
        (cols, rows)
    } else {
        let mr = if l.max_per_line > 0 {
            l.max_per_line
        } else {
            l.max_rows
        };
        let rows = if mr <= 0 { n } else { i64::from(mr).min(n) };
        let mut cols = if rows == 0 { 0 } else { (n + rows - 1) / rows };
        if l.max_columns > 0 {
            cols = cols.min(i64::from(l.max_columns));
        }
        (cols, rows)
    };
    let (cols, rows) = (cols.max(0) as usize, rows.max(0) as usize);
    let (reverse, flipped, wrap, ignore) = (l.reverse, l.flipped, l.wrap_indent, l.ignore_cell);
    let mut colw = vec![0i32; cols];
    let mut rowh = vec![0i32; rows];
    let advance = |c: &mut usize, r: &mut usize| {
        if horizontal {
            *c += 1;
            if *c >= cols {
                *c = 0;
                *r += 1;
            }
        } else {
            *r += 1;
            if *r >= rows {
                *r = 0;
                *c += 1;
            }
        }
    };
    let (mut c0, mut r0) = (0usize, 0usize);
    for (_, k) in &items {
        if c0 >= cols || r0 >= rows {
            break;
        }
        let (w, h) = ui.control_size(*k);
        colw[c0] = colw[c0].max(w);
        rowh[r0] = rowh[r0].max(h);
        advance(&mut c0, &mut r0);
    }
    let cw: i32 = colw.iter().fold(0i32, |a, b| a.saturating_add(*b));
    let chh: i32 = rowh.iter().fold(0i32, |a, b| a.saturating_add(*b));
    let (lw, lh) = ui.control_size(lkey);
    let (lw, lh, sx, sy, wrap) = (
        i64::from(lw),
        i64::from(lh),
        i64::from(sx),
        i64::from(sy),
        i64::from(wrap),
    );
    // Positions are computed in 64 bits and clamped when written (the inputs are plugin
    // values: sizes, scroll offsets and the wrap indent).
    let mut x = -sx;
    let mut y = -sy;
    if reverse && cols > 0 {
        x += lw - i64::from(colw[0]);
    }
    if (reverse ^ flipped) && rows > 0 {
        y += lh - i64::from(rowh[0]);
    }
    let mut wi = 0i64;
    let (mut c, mut r) = (0usize, 0usize);
    for (_, k) in &items {
        if c >= cols || r >= rows {
            break;
        }
        ui.control_set_position(*k, c32(x), c32(y));
        let (ew, eh) = ui.control_size(*k);
        let (cell_w, cell_h) = if ignore { (ew, eh) } else { (colw[c], rowh[r]) };
        let (cell_w, cell_h) = (i64::from(cell_w), i64::from(cell_h));
        if horizontal {
            if c + 1 < cols {
                x = if reverse { x - cell_w } else { x + cell_w };
            } else {
                wi = (wi + wrap).clamp(-(1 << 40), 1 << 40);
                x = if reverse {
                    lw - wi - sx - cell_w
                } else {
                    wi - sx
                };
                y = if flipped ^ reverse {
                    y - cell_h
                } else {
                    y + cell_h
                };
            }
        } else if r + 1 < rows {
            let rh = i64::from(rowh[r]);
            y = if reverse { y - rh } else { y + rh };
        } else {
            wi = (wi + wrap).clamp(-(1 << 40), 1 << 40);
            y = if reverse {
                lh - sy - i64::from(rowh[r]) - wi
            } else {
                wi - sy
            };
            let cwid = i64::from(colw[c]);
            x = if flipped ^ reverse {
                x - cwid
            } else {
                x + cwid
            };
        }
        // Keep the running sums far from overflow whatever the item count.
        x = x.clamp(-(1 << 50), 1 << 50);
        y = y.clamp(-(1 << 50), 1 << 50);
        advance(&mut c, &mut r);
    }
    if let Some(l) = ctl_mut(st, id).and_then(|c| c.list.as_deref_mut()) {
        l.content = (cw, chh);
    }
}

/// The nodes of tree `view` in display order with their depth.
fn tree_order(st: &State, view: u64) -> Vec<(u64, usize, bool)> {
    let mut out = Vec::new();
    let Some(t) = ctl_ref(st, view).and_then(|c| c.tree.as_deref()) else {
        return out;
    };
    let mut seen = HashSet::new();
    let mut stack: Vec<(u64, usize, bool)> = t.nodes.iter().rev().map(|n| (*n, 0, true)).collect();
    while let Some((n, depth, shown)) = stack.pop() {
        if out.len() >= MAX_WALK || !seen.insert(n) {
            continue;
        }
        let Some(node) = ctl_ref(st, n).and_then(|c| c.node.as_deref()) else {
            continue;
        };
        let visible = shown && !node.filtered;
        out.push((n, depth, visible));
        for ch in node.children.iter().rev() {
            stack.push((*ch, depth + 1, visible && node.expanded));
        }
    }
    out
}

fn layout_tree(ui: &mut Ui, st: &mut State, id: u64) {
    let Some(c) = ctl_ref(st, id) else { return };
    let (sx, sy) = c.offset;
    let indent = c.tree.as_deref().map_or(DEFAULT_INDENT, |t| t.indent);
    let order = tree_order(st, id);
    let mut y = 0i32;
    let mut cw = 0i32;
    for (n, depth, shown) in order {
        let Some(nc) = ctl_mut(st, n) else { continue };
        let k = nc.key;
        let hide = !shown;
        if nc.layer_hidden != hide {
            nc.layer_hidden = hide;
        }
        let want = nc.visible && !nc.layer_hidden;
        if ui.is_visible(k) != want {
            ui.control_set_visible(k, want);
            st.quiet_visible.insert(k);
        }
        if !shown {
            continue;
        }
        let x = (depth as i32).saturating_mul(indent);
        ui.control_set_position(
            k,
            c32(i64::from(x) - i64::from(sx)),
            c32(i64::from(y) - i64::from(sy)),
        );
        let (w, h) = ui.control_size(k);
        y = y.saturating_add(h.max(0));
        cw = cw.max(x.saturating_add(w));
    }
    if let Some(t) = ctl_mut(st, id).and_then(|c| c.tree.as_deref_mut()) {
        t.content = (cw, y);
    }
}

/// The view a node belongs to, walking up its parents.
fn view_of(st: &State, node: u64) -> Option<u64> {
    let mut cur = node;
    for _ in 0..MAX_WALK {
        let n = ctl_ref(st, cur)?.node.as_deref()?;
        if let Some(v) = n.view {
            return Some(v);
        }
        cur = n.parent?;
    }
    None
}

/// Every node at or below `node`.
fn subtree(st: &State, node: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if out.len() >= MAX_WALK || !seen.insert(n) {
            continue;
        }
        out.push(n);
        if let Some(nd) = ctl_ref(st, n).and_then(|c| c.node.as_deref()) {
            stack.extend(nd.children.iter().copied());
        }
    }
    out
}

/// Attach the elements of `node`'s subtree to `view` (or detach them with `None`).
fn attach_subtree(sh: &Shared, node: u64, view: Option<u64>) {
    let (keys, vkey) = {
        let st = sh.st.borrow();
        let keys: Vec<tessera_ui::ElementKey> = subtree(&st, node)
            .iter()
            .filter_map(|n| ctl_ref(&st, *n).map(|c| c.key))
            .collect();
        (keys, view.and_then(|v| ctl_ref(&st, v).map(|c| c.key)))
    };
    with_ui(sh, |u| {
        for k in keys {
            let _ = u.control_set_parent(k, vkey);
        }
    });
}

/// Add `node` under `parent` (a node) or as a top-level node of `view`.
fn add_node(
    lua: &Lua,
    sh: &Shared,
    node: u64,
    parent: Option<u64>,
    view: Option<u64>,
) -> mlua::Result<()> {
    if sh.kind_of(node) != Some(Kind::TreeNode) {
        return Err(script_error("a TreeNode is required"));
    }
    {
        let st = sh.st.borrow();
        if let Some(p) = parent
            && subtree(&st, node).contains(&p)
        {
            return Err(script_error("a node cannot be added below itself"));
        }
    }
    let old_view = {
        let mut st = sh.st.borrow_mut();
        let old = view_of(&st, node);
        forget_item(&mut st, node);
        if let Some(p) = parent
            && let Some(pn) = ctl_mut(&mut st, p).and_then(|c| c.node.as_deref_mut())
        {
            pn.children.push(node);
        }
        if let Some(v) = view
            && let Some(t) = ctl_mut(&mut st, v).and_then(|c| c.tree.as_deref_mut())
        {
            t.nodes.push(node);
        }
        if let Some(n) = ctl_mut(&mut st, node).and_then(|c| c.node.as_deref_mut()) {
            n.parent = parent;
            n.view = view;
        }
        old
    };
    set_owner(lua, sh, node, parent.or(view));
    let new_view = view_of(&sh.st.borrow(), node);
    attach_subtree(sh, node, new_view);
    if let Some(v) = old_view
        && Some(v) != new_view
    {
        relayout(sh, v);
    }
    if let Some(v) = new_view {
        relayout(sh, v);
    }
    Ok(())
}

/// Remove `node` from its parent node or view.
fn remove_node(lua: &Lua, sh: &Shared, node: u64) {
    let old_view = {
        let mut st = sh.st.borrow_mut();
        let v = view_of(&st, node);
        forget_item(&mut st, node);
        if let Some(n) = ctl_mut(&mut st, node).and_then(|c| c.node.as_deref_mut()) {
            n.parent = None;
            n.view = None;
        }
        v
    };
    set_owner(lua, sh, node, None);
    attach_subtree(sh, node, None);
    if let Some(v) = old_view {
        relayout(sh, v);
    }
}

/// The ids a list object views.
fn view_items(sh: &Shared, view: ListView) -> Vec<u64> {
    match view {
        ListView::Controls(o) => child_ids(sh, o),
        ListView::MenuItems(o) => match sh.st.borrow().objs.get(&o).map(|x| &x.data) {
            Some(ObjData::Menu(_, m)) => m.items.clone(),
            _ => Vec::new(),
        },
        ListView::TreeNodes(o) => ctl_get(sh, o, |c| {
            c.tree
                .as_deref()
                .map(|t| t.nodes.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        ListView::ChildNodes(o) => ctl_get(sh, o, |c| {
            c.node
                .as_deref()
                .map(|t| t.children.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
    }
}

fn view_of_obj(cx: &Cx<'_>) -> Option<ListView> {
    match cx.sh.st.borrow().objs.get(&cx.id).map(|o| &o.data) {
        Some(ObjData::List(v)) => Some(*v),
        _ => None,
    }
}

/// A 1-based index argument checked against `count` (§8.5).
fn index(v: Option<&Value>, count: usize, what: &str) -> mlua::Result<usize> {
    let i = int(v);
    if i < 1 || i as usize > count {
        return Err(script_error(format!("{what}: index {i} out of range")));
    }
    Ok(i as usize - 1)
}

fn item_arg(cx: &Cx<'_>, v: Option<&Value>, view: ListView) -> mlua::Result<u64> {
    let Some(Value::Table(t)) = v else {
        return Err(script_error("an object is required"));
    };
    let id = obj_id(cx.lua, t)
        .ok_or_else(|| script_error("an object of the list's class is required"))?;
    let ok = match view {
        ListView::Controls(_) => cx.sh.key_of(id).is_some(),
        ListView::MenuItems(_) => matches!(
            cx.sh.st.borrow().objs.get(&id).map(|o| &o.data),
            Some(ObjData::Menu(false, _))
        ),
        ListView::TreeNodes(_) | ListView::ChildNodes(_) => {
            cx.sh.kind_of(id) == Some(Kind::TreeNode)
        }
    };
    if !ok {
        return Err(script_error("an object of the list's class is required"));
    }
    Ok(id)
}

fn list_add(cx: &Cx<'_>, view: ListView, item: u64, at: Option<usize>) -> mlua::Result<()> {
    match view {
        ListView::Controls(o) => set_parent(cx.lua, cx.sh, item, Some(o)),
        ListView::MenuItems(o) => {
            {
                let mut st = cx.sh.st.borrow_mut();
                forget_item(&mut st, item);
                if let Some(ObjData::Menu(_, m)) = st.objs.get_mut(&o).map(|x| &mut x.data) {
                    let i = at.unwrap_or(m.items.len()).min(m.items.len());
                    m.items.insert(i, item);
                }
                if let Some(ObjData::Menu(_, m)) = st.objs.get_mut(&item).map(|x| &mut x.data) {
                    m.parent = Some(o);
                }
            }
            super::state::anchor(cx.lua, item, None, Some(o));
            Ok(())
        }
        ListView::TreeNodes(v) => add_node(cx.lua, cx.sh, item, None, Some(v)),
        ListView::ChildNodes(p) => add_node(cx.lua, cx.sh, item, Some(p), None),
    }
}

fn list_remove(cx: &Cx<'_>, view: ListView, item: u64) -> mlua::Result<()> {
    match view {
        ListView::Controls(o) => {
            if child_ids(cx.sh, o).contains(&item) {
                set_parent(cx.lua, cx.sh, item, None)?;
            }
            Ok(())
        }
        ListView::MenuItems(o) => {
            {
                let mut st = cx.sh.st.borrow_mut();
                let held = match st.objs.get_mut(&o).map(|x| &mut x.data) {
                    Some(ObjData::Menu(_, m)) => {
                        let n = m.items.len();
                        m.items.retain(|x| *x != item);
                        n != m.items.len()
                    }
                    _ => false,
                };
                if held
                    && let Some(ObjData::Menu(_, m)) = st.objs.get_mut(&item).map(|x| &mut x.data)
                {
                    m.parent = None;
                }
            }
            super::state::anchor(cx.lua, item, Some(o), None);
            Ok(())
        }
        ListView::TreeNodes(_) | ListView::ChildNodes(_) => {
            if view_items(cx.sh, view).contains(&item) {
                remove_node(cx.lua, cx.sh, item);
            }
            Ok(())
        }
    }
}

/// Install the list classes.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    object: &Table,
) -> mlua::Result<()> {
    let lua = api.lua();
    let funcs = lua.create_table()?;
    let weak = lua.create_table()?;
    weak.raw_set("__mode", "k")?;
    funcs.set_metatable(Some(weak))?;
    lua.set_named_registry_value(FUNCS_KEY, funcs)?;

    install_list_objects(api, sh, ui, object)?;
    let scrollable = class_table(lua, "ScrollableControl")?;
    let control = class_table(lua, "Control")?;
    let lb = class(
        api,
        sh,
        ui,
        "ListBox",
        "ListBox",
        Some(&scrollable),
        control_ctor(Kind::ListBox, Styled::Plain),
    )?;
    install_list_box(api, sh, &lb)?;
    let tv = class(
        api,
        sh,
        ui,
        "TreeView",
        "TreeView",
        Some(&scrollable),
        control_ctor(Kind::TreeView, Styled::Plain),
    )?;
    let tn = class(
        api,
        sh,
        ui,
        "TreeNode",
        "TreeNode",
        Some(&control),
        control_ctor(Kind::TreeNode, Styled::Plain),
    )?;
    install_tree(api, sh, &tv, &tn)?;
    Ok(())
}

fn install_list_objects(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    object: &Table,
) -> mlua::Result<()> {
    let classes = [
        (
            class_plain(api, ui, "ControlList", "ControlList", Some(object))?,
            true,
        ),
        (
            class_plain(api, ui, "MenuItemList", "MenuItemList", Some(object))?,
            true,
        ),
        (
            class_plain(api, ui, "TreeNodeList", "TreeNodeList", Some(object))?,
            false,
        ),
    ];
    for (cls, ordered) in classes {
        def(api, sh, &cls, "GetCount", list_obj, |cx, _| {
            Ok(view_of_obj(cx).map_or(0, |v| view_items(cx.sh, v).len()) as f64)
        })?;
        def(api, sh, &cls, "Get", list_obj, |cx, args| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            let items = view_items(cx.sh, v);
            let i = index(args.front(), items.len(), "Get")?;
            Ok(object_of(cx.lua, items[i]))
        })?;
        def(api, sh, &cls, "IndexOf", list_obj, |cx, args| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            let id = match args.front() {
                Some(Value::Table(t)) => obj_id(cx.lua, t),
                _ => None,
            };
            let items = view_items(cx.sh, v);
            Ok(id
                .and_then(|id| items.iter().position(|x| *x == id))
                .map_or(-1.0, |p| p as f64 + 1.0))
        })?;
        def(api, sh, &cls, "Contains", list_obj, |cx, args| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            let id = match args.front() {
                Some(Value::Table(t)) => obj_id(cx.lua, t),
                _ => None,
            };
            Ok(id.is_some_and(|id| view_items(cx.sh, v).contains(&id)))
        })?;
        def(api, sh, &cls, "Add", list_obj, |cx, args| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            let item = item_arg(cx, args.front(), v)?;
            list_add(cx, v, item, None)
        })?;
        def(api, sh, &cls, "Remove", list_obj, |cx, args| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            let Some(Value::Table(t)) = args.front() else {
                return Ok(());
            };
            match obj_id(cx.lua, t) {
                Some(item) => list_remove(cx, v, item),
                None => Ok(()),
            }
        })?;
        def(api, sh, &cls, "RemoveAt", list_obj, |cx, args| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            let items = view_items(cx.sh, v);
            let i = index(args.front(), items.len(), "RemoveAt")?;
            list_remove(cx, v, items[i])
        })?;
        def(api, sh, &cls, "Clear", list_obj, |cx, _| {
            let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
            for item in view_items(cx.sh, v) {
                list_remove(cx, v, item)?;
            }
            Ok(())
        })?;
        if ordered {
            def(api, sh, &cls, "Insert", list_obj, |cx, args| {
                let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
                let n = view_items(cx.sh, v).len();
                let i = index(args.front(), n + 1, "Insert")?;
                let item = item_arg(cx, args.get(1), v)?;
                list_add(cx, v, item, Some(i))
            })?;
            def(api, sh, &cls, "Set", list_obj, |cx, args| {
                let v = view_of_obj(cx).ok_or_else(|| script_error("not a list"))?;
                let items = view_items(cx.sh, v);
                let i = index(args.front(), items.len(), "Set")?;
                let item = item_arg(cx, args.get(1), v)?;
                if items[i] != item {
                    list_remove(cx, v, items[i])?;
                    list_add(cx, v, item, Some(i))?;
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

/// Select `item` (an item of list box `id`, or none); `SelectedIndexChanged` goes through
/// the queue when the selection changed.
pub(crate) fn select_item(st: &mut State, id: u64, item: Option<u64>) {
    let changed = match ctl_mut(st, id).and_then(|c| c.list.as_deref_mut()) {
        Some(l) => {
            let item = item.filter(|i| l.items.contains(i));
            std::mem::replace(&mut l.selected, item) != item
        }
        None => false,
    };
    if changed {
        push(
            st,
            Pending::Fire {
                target: id,
                name: "SelectedIndexChanged",
                args: Args::Empty,
            },
        );
    }
}

/// Select tree node `node` of view `view` (or none); toggles the node first (the native
/// tree rule, `ui-widgets.md` §9.4) when `toggle`.
pub(crate) fn select_node(
    _ui: &mut Ui,
    st: &mut State,
    view: u64,
    node: Option<u64>,
    toggle: bool,
) {
    if toggle
        && let Some(n) = node
        && let Some(nd) = ctl_mut(st, n).and_then(|c| c.node.as_deref_mut())
    {
        nd.expanded = if nd.children.is_empty() {
            true
        } else {
            !nd.expanded
        };
    }
    let old = match ctl_mut(st, view).and_then(|c| c.tree.as_deref_mut()) {
        Some(t) => std::mem::replace(&mut t.selected, node),
        None => return,
    };
    if old != node {
        push(
            st,
            Pending::Fire {
                target: view,
                name: "SelectedNodeChanged",
                args: Args::Empty,
            },
        );
        for n in [old, node].into_iter().flatten() {
            push(
                st,
                Pending::Fire {
                    target: n,
                    name: "SelectedChanged",
                    args: Args::Empty,
                },
            );
        }
    }
    mark(st, view);
}

/// Scroll list box or tree `id` by `dy` pixels (the wheel), within its content.
pub(crate) fn scroll_by(ui: &mut Ui, st: &mut State, id: u64, dy: i32) -> bool {
    flush_with(ui, st);
    let Some(c) = ctl_ref(st, id) else {
        return false;
    };
    let content = c
        .list
        .as_deref()
        .map(|l| l.content)
        .or_else(|| c.tree.as_deref().map(|t| t.content));
    let Some(content) = content else { return false };
    let (_, h) = ui.control_size(c.key);
    let max = content.1.saturating_sub(h).max(0);
    let new = c.offset.1.saturating_add(dy).clamp(0, max);
    if new == c.offset.1 {
        return false;
    }
    if let Some(c) = ctl_mut(st, id) {
        c.offset.1 = new;
    }
    mark(st, id);
    true
}

/// The list box or tree that holds `id` as an item or node, with that item.
pub(crate) fn owning_list(
    ui: &Ui,
    st: &State,
    key: tessera_ui::ElementKey,
) -> Option<(u64, Option<u64>)> {
    let mut cur = Some(key);
    let mut child: Option<u64> = None;
    for _ in 0..256 {
        let k = cur?;
        if let Some(id) = st.by_key.get(&k).copied()
            && let Some(c) = ctl_ref(st, id)
        {
            match c.kind {
                Kind::ListBox => {
                    let item =
                        child.filter(|ch| c.list.as_deref().is_some_and(|l| l.items.contains(ch)));
                    return Some((id, item));
                }
                Kind::TreeView => {
                    return Some((
                        id,
                        child.filter(|ch| {
                            ctl_ref(st, *ch).is_some_and(|x| x.kind == Kind::TreeNode)
                        }),
                    ));
                }
                _ => {}
            }
            child = Some(id);
        }
        cur = ui.control_parent(k);
    }
    None
}

fn install_list_box(api: &NativeApi<'_>, sh: &Rc<Shared>, lb: &Table) -> mlua::Result<()> {
    let l = list_box;
    def(api, sh, lb, "AddItem", l, |cx, args| {
        let item = ctl_arg(cx.lua, cx.sh, args.front())
            .ok_or_else(|| script_error("AddItem: a control is required"))?;
        add_item(cx, item, None)
    })?;
    def(api, sh, lb, "InsertItem", l, |cx, args| {
        let n = ls(cx.sh, cx.id, |l| l.items.len()).unwrap_or(0);
        let i = index(args.front(), n + 1, "InsertItem")?;
        let item = ctl_arg(cx.lua, cx.sh, args.get(1))
            .ok_or_else(|| script_error("InsertItem: a control is required"))?;
        add_item(cx, item, Some(i))
    })?;
    def(api, sh, lb, "RemoveItem", l, |cx, args| {
        if let Some(item) = ctl_arg(cx.lua, cx.sh, args.front()) {
            remove_item(cx, item)?;
        }
        Ok(())
    })?;
    def(api, sh, lb, "RemoveItemAt", l, |cx, args| {
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        let i = index(args.front(), items.len(), "RemoveItemAt")?;
        remove_item(cx, items[i])
    })?;
    def(api, sh, lb, "ClearItems", l, |cx, _| {
        for item in ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default() {
            remove_item(cx, item)?;
        }
        Ok(())
    })?;
    def(api, sh, lb, "GetItem", l, |cx, args| {
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        let i = index(args.front(), items.len(), "GetItem")?;
        Ok(object_of(cx.lua, items[i]))
    })?;
    def(api, sh, lb, "SetItem", l, |cx, args| {
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        let i = index(args.front(), items.len(), "SetItem")?;
        let item = ctl_arg(cx.lua, cx.sh, args.get(1))
            .ok_or_else(|| script_error("SetItem: a control is required"))?;
        if items[i] != item {
            remove_item(cx, items[i])?;
            add_item(cx, item, Some(i))?;
        }
        Ok(())
    })?;
    def(api, sh, lb, "GetItemCount", l, |cx, _| {
        Ok(ls(cx.sh, cx.id, |l| l.items.len()).unwrap_or(0) as f64)
    })?;
    def(api, sh, lb, "IndexOfItem", l, |cx, args| {
        let id = ctl_arg(cx.lua, cx.sh, args.front());
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        Ok(id
            .and_then(|id| items.iter().position(|x| *x == id))
            .map_or(-1.0, |p| p as f64 + 1.0))
    })?;
    def(api, sh, lb, "ContainsItem", l, |cx, args| {
        let id = ctl_arg(cx.lua, cx.sh, args.front());
        Ok(id.is_some_and(|id| ls(cx.sh, cx.id, |l| l.items.contains(&id)).unwrap_or(false)))
    })?;
    def(api, sh, lb, "GetItemAt", l, |cx, args| {
        let (x, y) = (int(args.front()), int(args.get(1)));
        Ok(item_at(cx, x, y).and_then(|i| object_of(cx.lua, i)))
    })?;
    def(api, sh, lb, "EnsureVisible", l, |cx, args| {
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        let Ok(i) = index(args.front(), items.len(), "EnsureVisible") else {
            return Ok(());
        };
        ensure_visible(cx, items[i]);
        Ok(())
    })?;
    def(api, sh, lb, "GetSelectedIndex", l, |cx, _| {
        Ok(ls(cx.sh, cx.id, |l| {
            l.selected
                .and_then(|s| l.items.iter().position(|x| *x == s))
        })
        .flatten()
        .map_or(0.0, |p| p as f64 + 1.0))
    })?;
    def(api, sh, lb, "SetSelectedIndex", l, |cx, args| {
        let i = int(args.front());
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        let item = (i >= 1 && (i as usize) <= items.len()).then(|| items[i as usize - 1]);
        select_item(&mut cx.sh.st.borrow_mut(), cx.id, item);
        Ok(())
    })?;
    def(api, sh, lb, "GetSelectedItem", l, |cx, _| {
        Ok(ls(cx.sh, cx.id, |l| l.selected)
            .flatten()
            .and_then(|s| object_of(cx.lua, s)))
    })?;
    def(api, sh, lb, "SetSelectedItem", l, |cx, args| {
        let item = ctl_arg(cx.lua, cx.sh, args.front());
        select_item(&mut cx.sh.st.borrow_mut(), cx.id, item);
        Ok(())
    })?;
    // Layout members.
    macro_rules! num_member {
        ($get:literal, $set:literal, $field:ident) => {
            def(api, sh, lb, $get, l, |cx, _| {
                Ok(f64::from(
                    ls(cx.sh, cx.id, |l| l.$field).unwrap_or_default(),
                ))
            })?;
            def(api, sh, lb, $set, l, |cx, args| {
                let v = int(args.front());
                ls(cx.sh, cx.id, |l| l.$field = v as _);
                relayout(cx.sh, cx.id);
                Ok(())
            })?;
        };
    }
    macro_rules! bool_member {
        ($get:literal, $set:literal, $field:ident) => {
            def(api, sh, lb, $get, l, |cx, _| {
                Ok(ls(cx.sh, cx.id, |l| l.$field).unwrap_or(false))
            })?;
            def(api, sh, lb, $set, l, |cx, args| {
                let v = truthy(args.front());
                ls(cx.sh, cx.id, |l| l.$field = v);
                relayout(cx.sh, cx.id);
                Ok(())
            })?;
        };
    }
    num_member!("GetOrientation", "SetOrientation", orientation);
    num_member!("GetMaxItemsPerLine", "SetMaxItemsPerLine", max_per_line);
    num_member!("GetMaxRows", "SetMaxRows", max_rows);
    num_member!("GetMaxColumns", "SetMaxColumns", max_columns);
    num_member!("GetWrapIndentAmount", "SetWrapIndentAmount", wrap_indent);
    bool_member!("GetReverseFill", "SetReverseFill", reverse);
    bool_member!("GetFlippedLayout", "SetFlippedLayout", flipped);
    bool_member!(
        "GetIgnoreCellAlignment",
        "SetIgnoreCellAlignment",
        ignore_cell
    );
    def(api, sh, lb, "Sort", l, |cx, args| {
        let Some(Value::Function(f)) = args.front().cloned() else {
            return Err(script_error("Sort: a comparison function is required"));
        };
        let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
        let sorted = lua_sort(cx.lua, items, &f)?;
        ls(cx.sh, cx.id, |l| {
            // Items removed by the comparator stay removed.
            let keep: HashSet<u64> = l.items.iter().copied().collect();
            l.items = sorted.into_iter().filter(|i| keep.contains(i)).collect();
        });
        relayout(cx.sh, cx.id);
        Ok(())
    })?;
    Ok(())
}

fn add_item(cx: &Cx<'_>, item: u64, at: Option<usize>) -> mlua::Result<()> {
    if item == cx.id {
        return Err(script_error("a list box cannot be its own item"));
    }
    // Only an item of this list can already be in it (`set_parent` takes it out of any
    // other list), so the search runs only then.
    let again = ctl_get(cx.sh, item, |c| c.owner == Some(cx.id)).unwrap_or(false);
    set_parent(cx.lua, cx.sh, item, Some(cx.id))?;
    ls(cx.sh, cx.id, |l| {
        if again {
            l.items.retain(|x| *x != item);
        }
        let i = at.unwrap_or(l.items.len()).min(l.items.len());
        l.items.insert(i, item);
    });
    relayout(cx.sh, cx.id);
    cx.sh.push(Pending::Fire {
        target: cx.id,
        name: "ItemAdded",
        args: Args::Item(item),
    });
    Ok(())
}

fn remove_item(cx: &Cx<'_>, item: u64) -> mlua::Result<()> {
    let had = ls(cx.sh, cx.id, |l| l.items.contains(&item)).unwrap_or(false);
    if !had {
        return Ok(());
    }
    let was_selected = ls(cx.sh, cx.id, |l| l.selected == Some(item)).unwrap_or(false);
    cx.sh.push(Pending::Fire {
        target: cx.id,
        name: "ItemRemoved",
        args: Args::Item(item),
    });
    set_parent(cx.lua, cx.sh, item, None)?;
    ls(cx.sh, cx.id, |l| {
        l.items.retain(|x| *x != item);
        if l.selected == Some(item) {
            l.selected = None;
        }
    });
    if was_selected {
        cx.sh.push(Pending::Fire {
            target: cx.id,
            name: "SelectedIndexChanged",
            args: Args::Empty,
        });
    }
    relayout(cx.sh, cx.id);
    Ok(())
}

fn item_at(cx: &Cx<'_>, x: i32, y: i32) -> Option<u64> {
    let items = ls(cx.sh, cx.id, |l| l.items.clone()).unwrap_or_default();
    let st = cx.sh.st.borrow();
    let ui = cx.sh.ui.try_borrow().ok()?;
    items.into_iter().find(|i| {
        ctl_ref(&st, *i).is_some_and(|c| {
            let (px, py) = ui.control_position(c.key);
            let (w, h) = ui.control_size(c.key);
            x >= px && y >= py && x < px.saturating_add(w) && y < py.saturating_add(h)
        })
    })
}

fn ensure_visible(cx: &Cx<'_>, item: u64) {
    let Ok(mut ui) = cx.sh.ui.try_borrow_mut() else {
        return;
    };
    let mut st = cx.sh.st.borrow_mut();
    flush_with(&mut ui, &mut st);
    let Some(ik) = ctl_ref(&st, item).map(|c| c.key) else {
        return;
    };
    let Some(c) = ctl_ref(&st, cx.id) else { return };
    let (_, h) = ui.control_size(c.key);
    let (_, py) = ui.control_position(ik);
    let (_, ih) = ui.control_size(ik);
    let mut off = c.offset.1;
    if py < 0 {
        off = off.saturating_add(py);
    } else if py.saturating_add(ih) > h {
        off = off.saturating_add(py.saturating_add(ih).saturating_sub(h));
    }
    let off = off.max(0);
    if let Some(c) = ctl_mut(&mut st, cx.id) {
        c.offset.1 = off;
    }
    layout_with(&mut ui, &mut st, cx.id);
}

/// A merge sort driven by a Lua comparison (`compare(a, b)` true when `a` sorts first).
/// An inconsistent comparison gives some order, never a failure of the host; an error
/// raised by the comparison is passed on.
pub(crate) fn lua_sort(lua: &Lua, items: Vec<u64>, f: &Function) -> mlua::Result<Vec<u64>> {
    let objs: Vec<(u64, Option<Table>)> =
        items.into_iter().map(|i| (i, object_of(lua, i))).collect();
    let less = |a: &(u64, Option<Table>), b: &(u64, Option<Table>)| -> mlua::Result<bool> {
        let r: Value = f.call((a.1.clone(), b.1.clone()))?;
        Ok(!matches!(r, Value::Nil | Value::Boolean(false)))
    };
    let mut v = objs;
    let n = v.len();
    let mut width = 1;
    while width < n {
        let mut out = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let mid = (i + width).min(n);
            let end = (i + 2 * width).min(n);
            let (mut a, mut b) = (i, mid);
            while a < mid && b < end {
                if less(&v[b], &v[a])? {
                    out.push(v[b].clone());
                    b += 1;
                } else {
                    out.push(v[a].clone());
                    a += 1;
                }
            }
            out.extend_from_slice(&v[a..mid]);
            out.extend_from_slice(&v[b..end]);
            i = end;
        }
        v = out;
        width *= 2;
    }
    Ok(v.into_iter().map(|(i, _)| i).collect())
}

fn node_list(cx: &Cx<'_>, view: ListView) -> mlua::Result<Table> {
    let cached = ctl_get(cx.sh, cx.id, |c| match view {
        ListView::TreeNodes(_) => c.tree.as_deref().and_then(|t| t.nodes_list),
        _ => c.node.as_deref().and_then(|n| n.children_list),
    })
    .flatten();
    if let Some(l) = cached
        && let Some(t) = object_of(cx.lua, l)
    {
        return Ok(t);
    }
    let t = super::model::new_object(cx.lua, cx.sh, &cx.ap, "TreeNodeList", ObjData::List(view))?;
    if let Some(l) = obj_id(cx.lua, &t) {
        with_ctl(cx.sh, cx.id, |c| match view {
            ListView::TreeNodes(_) => {
                if let Some(tr) = c.tree.as_deref_mut() {
                    tr.nodes_list = Some(l);
                }
            }
            _ => {
                if let Some(n) = c.node.as_deref_mut() {
                    n.children_list = Some(l);
                }
            }
        });
        super::state::anchor(cx.lua, l, None, Some(cx.id));
    }
    Ok(t)
}

fn set_expanded(sh: &Shared, nodes: &[u64], v: bool) {
    let view = {
        let mut st = sh.st.borrow_mut();
        for n in nodes {
            if let Some(nd) = ctl_mut(&mut st, *n).and_then(|c| c.node.as_deref_mut()) {
                nd.expanded = v;
            }
        }
        nodes.first().and_then(|n| view_of(&st, *n))
    };
    if let Some(v) = view {
        relayout(sh, v);
    }
}

fn funcs_of(lua: &Lua, obj: &Table) -> mlua::Result<Table> {
    let funcs: Table = lua.named_registry_value(FUNCS_KEY)?;
    match funcs.raw_get::<Value>(obj.clone())? {
        Value::Table(t) => Ok(t),
        _ => {
            let t = lua.create_table()?;
            funcs.raw_set(obj.clone(), t.clone())?;
            Ok(t)
        }
    }
}

/// Apply the filter and sort functions of tree view `view` (Refresh).
fn refresh_tree(cx: &Cx<'_>) -> mlua::Result<()> {
    let f = funcs_of(cx.lua, &cx.this)?;
    let filter: Option<Function> = f.raw_get("Filter").ok();
    let sort: Option<Function> = f.raw_get("Sort").ok();
    let all: Vec<u64> = {
        let st = cx.sh.st.borrow();
        tree_order(&st, cx.id)
            .into_iter()
            .map(|(n, _, _)| n)
            .collect()
    };
    for n in &all {
        let keep = match (&filter, object_of(cx.lua, *n)) {
            (Some(f), Some(obj)) => {
                !matches!(f.call::<Value>(obj)?, Value::Nil | Value::Boolean(false))
            }
            _ => true,
        };
        if let Some(nd) =
            ctl_mut(&mut cx.sh.st.borrow_mut(), *n).and_then(|c| c.node.as_deref_mut())
        {
            nd.filtered = !keep;
        }
    }
    if let Some(sort) = sort {
        let tops = ctl_get(cx.sh, cx.id, |c| {
            c.tree
                .as_deref()
                .map(|t| t.nodes.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default();
        let sorted = lua_sort(cx.lua, tops, &sort)?;
        with_ctl(cx.sh, cx.id, |c| {
            if let Some(t) = c.tree.as_deref_mut() {
                t.nodes = sorted;
            }
        });
        for n in &all {
            let kids = ctl_get(cx.sh, *n, |c| {
                c.node
                    .as_deref()
                    .map(|x| x.children.clone())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
            if kids.len() > 1 {
                let s = lua_sort(cx.lua, kids, &sort)?;
                with_ctl(cx.sh, *n, |c| {
                    if let Some(x) = c.node.as_deref_mut() {
                        x.children = s;
                    }
                });
            }
        }
    }
    relayout(cx.sh, cx.id);
    Ok(())
}

fn install_tree(api: &NativeApi<'_>, sh: &Rc<Shared>, tv: &Table, tn: &Table) -> mlua::Result<()> {
    let t = tree_view;
    def(api, sh, tv, "GetNodes", t, |cx, _| {
        node_list(cx, ListView::TreeNodes(cx.id))
    })?;
    def(api, sh, tv, "GetSelectedNode", t, |cx, _| {
        let s = ctl_get(cx.sh, cx.id, |c| c.tree.as_deref().and_then(|t| t.selected)).flatten();
        Ok(s.and_then(|s| object_of(cx.lua, s)))
    })?;
    def(api, sh, tv, "SetSelectedNode", t, |cx, args| {
        let node = ctl_arg(cx.lua, cx.sh, args.front())
            .filter(|n| cx.sh.kind_of(*n) == Some(Kind::TreeNode));
        if let Ok(mut ui) = cx.sh.ui.try_borrow_mut() {
            let mut st = cx.sh.st.borrow_mut();
            select_node(&mut ui, &mut st, cx.id, node, false);
        }
        Ok(())
    })?;
    def(api, sh, tv, "ExpandAll", t, |cx, _| {
        let all: Vec<u64> = tree_order(&cx.sh.st.borrow(), cx.id)
            .into_iter()
            .map(|x| x.0)
            .collect();
        set_expanded(cx.sh, &all, true);
        relayout(cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, tv, "CollapseAll", t, |cx, _| {
        let all: Vec<u64> = tree_order(&cx.sh.st.borrow(), cx.id)
            .into_iter()
            .map(|x| x.0)
            .collect();
        set_expanded(cx.sh, &all, false);
        relayout(cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, tv, "GetIndentationWidth", t, |cx, _| {
        Ok(f64::from(
            ctl_get(cx.sh, cx.id, |c| {
                c.tree.as_deref().map_or(DEFAULT_INDENT, |t| t.indent)
            })
            .unwrap_or(DEFAULT_INDENT),
        ))
    })?;
    def(api, sh, tv, "SetIndentationWidth", t, |cx, args| {
        let v = int(args.front());
        with_ctl(cx.sh, cx.id, |c| {
            if let Some(t) = c.tree.as_deref_mut() {
                t.indent = v;
            }
        });
        relayout(cx.sh, cx.id);
        Ok(())
    })?;
    for (get, set, field) in [
        ("GetFilter", "SetFilter", "Filter"),
        ("GetSortMethod", "SetSortMethod", "Sort"),
    ] {
        def(api, sh, tv, get, t, move |cx, _| {
            funcs_of(cx.lua, &cx.this)?.raw_get::<Value>(field)
        })?;
        def(api, sh, tv, set, t, move |cx, args| {
            let v = match args.front() {
                Some(Value::Function(f)) => Value::Function(f.clone()),
                _ => Value::Nil,
            };
            funcs_of(cx.lua, &cx.this)?.raw_set(field, v)?;
            refresh_tree(cx)
        })?;
    }
    def(api, sh, tv, "Refresh", t, |cx, _| refresh_tree(cx))?;
    def(api, sh, tv, "GetItemAt", t, |cx, args| {
        let (x, y) = (int(args.front()), int(args.get(1)));
        let found = {
            let st = cx.sh.st.borrow();
            let ui = cx
                .sh
                .ui
                .try_borrow()
                .map_err(|_| script_error("the user interface is busy"))?;
            tree_order(&st, cx.id)
                .into_iter()
                .filter(|x| x.2)
                .find_map(|(n, _, _)| {
                    let c = ctl_ref(&st, n)?;
                    let (px, py) = ui.control_position(c.key);
                    let (w, h) = ui.control_size(c.key);
                    (x >= px && y >= py && x < px.saturating_add(w) && y < py.saturating_add(h))
                        .then_some(n)
                })
        };
        Ok(found.and_then(|n| object_of(cx.lua, n)))
    })?;

    let n = tree_node;
    def(api, sh, tn, "GetChildNodes", n, |cx, _| {
        node_list(cx, ListView::ChildNodes(cx.id))
    })?;
    def(api, sh, tn, "GetParentNode", n, |cx, _| {
        let p = ctl_get(cx.sh, cx.id, |c| c.node.as_deref().and_then(|n| n.parent)).flatten();
        Ok(p.and_then(|p| object_of(cx.lua, p)))
    })?;
    def(api, sh, tn, "Expand", n, |cx, _| {
        set_expanded(cx.sh, &[cx.id], true);
        Ok(())
    })?;
    def(api, sh, tn, "Collapse", n, |cx, _| {
        set_expanded(cx.sh, &[cx.id], false);
        Ok(())
    })?;
    def(api, sh, tn, "ExpandAll", n, |cx, _| {
        let all = subtree(&cx.sh.st.borrow(), cx.id);
        set_expanded(cx.sh, &all, true);
        Ok(())
    })?;
    def(api, sh, tn, "CollapseAll", n, |cx, _| {
        let all = subtree(&cx.sh.st.borrow(), cx.id);
        set_expanded(cx.sh, &all, false);
        Ok(())
    })?;
    def(api, sh, tn, "IsExpanded", n, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| {
            c.node.as_deref().is_some_and(|n| n.expanded)
        })
        .unwrap_or(false))
    })?;
    def(api, sh, tn, "SetExpanded", n, |cx, args| {
        set_expanded(cx.sh, &[cx.id], truthy(args.front()));
        Ok(())
    })?;
    def(api, sh, tn, "IsSelected", n, |cx, _| {
        let st = cx.sh.st.borrow();
        let v = view_of(&st, cx.id);
        Ok(v.and_then(|v| {
            ctl_ref(&st, v)
                .and_then(|c| c.tree.as_deref())
                .and_then(|t| t.selected)
        }) == Some(cx.id))
    })?;
    Ok(())
}
