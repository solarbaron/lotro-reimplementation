// SPDX-License-Identifier: GPL-3.0-or-later
//! ListBox (`ui-widgets.md` §8) and Tree (§9).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::UiError;
use crate::events::{Answer, Message, MsgArg};
use crate::geom::{add, sub};
use crate::props::{array_elements, as_key, struct_field};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::commands;
use crate::widgets::{TreeNode, TreePart};

use std::collections::{HashMap, HashSet};

/// Most entries one list box keeps (Tessera cap, hostile input).
pub const MAX_ENTRIES: usize = 1 << 16;

impl Ui {
    /// The entries of a list box, in entry-list order.
    #[must_use]
    pub fn list_entries(&self, list: ElementKey) -> Vec<ElementKey> {
        self.el(list)
            .and_then(|e| e.widget.list.as_ref())
            .map(|l| l.entries.clone())
            .unwrap_or_default()
    }

    /// The selected entry.
    #[must_use]
    pub fn list_selected(&self, list: ElementKey) -> Option<ElementKey> {
        self.el(list)
            .and_then(|e| e.widget.list.as_ref())
            .and_then(|l| l.selected)
    }

    /// The grid (columns, rows) of the last layout.
    #[must_use]
    pub fn list_grid(&self, list: ElementKey) -> Option<(i32, i32)> {
        self.el(list)
            .and_then(|e| e.widget.list.as_ref())
            .map(|l| l.grid)
    }

    /// Appends an entry to a list box (making it a child first). For a Tree it becomes a
    /// node under `tree_parent` (a top-level node when `None`).
    ///
    /// # Errors
    /// [`UiError::NotAList`], [`UiError::Full`] or an attachment error.
    pub fn list_add(
        &mut self,
        list: ElementKey,
        entry: ElementKey,
        tree_parent: Option<ElementKey>,
    ) -> Result<(), UiError> {
        let Some(l) = self.el(list).and_then(|e| e.widget.list.as_ref()) else {
            return Err(UiError::NotAList);
        };
        if l.entries.len() >= MAX_ENTRIES {
            return Err(UiError::Full);
        }
        if self.parent(entry) != Some(list) {
            self.attach_raw(entry, list)?;
        }
        if let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut())
            && !l.entries.contains(&entry)
        {
            l.entries.push(entry);
            l.dirty = true;
        }
        if self.el(list).is_some_and(|e| e.widget.tree.is_some()) {
            // Once per entry: `tree_add_node` adds nothing when the entry has a node.
            self.tree_add_node(list, entry, tree_parent);
        }
        self.list_layout_if_dirty(list);
        Ok(())
    }

    /// Removes an entry from a list box and destroys it.
    pub fn list_remove(&mut self, list: ElementKey, entry: ElementKey) {
        let had = self
            .el_mut(list)
            .and_then(|e| e.widget.list.as_mut())
            .is_some_and(|l| {
                let had = l.entries.contains(&entry);
                l.entries.retain(|c| *c != entry);
                l.dirty = true;
                had
            });
        if had {
            self.destroy(entry);
            self.list_layout_if_dirty(list);
        }
    }

    /// Removes every entry of a list box and destroys them in one batch
    /// ([`Ui::destroy_many`]), then lays the list out once.
    pub fn list_clear(&mut self, list: ElementKey) {
        let entries = match self.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
            Some(l) => {
                l.dirty = true;
                std::mem::take(&mut l.entries)
            }
            None => return,
        };
        self.destroy_many(&entries);
        self.list_layout_if_dirty(list);
    }

    /// Appends several entries to a list box (not a Tree) as [`Ui::list_add`] does, laying
    /// the list out once at the end. Stops at the first entry that cannot be added and
    /// returns how many were added with that error; entries after it are untouched.
    ///
    /// # Errors
    /// `(added, error)`: [`UiError::NotAList`] (a Tree is refused too: its entries need
    /// [`Ui::list_add`]), [`UiError::Full`] or an attachment error.
    pub fn list_add_many(
        &mut self,
        list: ElementKey,
        entries: &[ElementKey],
    ) -> Result<(), (usize, UiError)> {
        let mut present: HashSet<ElementKey> = match self.el(list) {
            Some(e) if e.widget.tree.is_none() => match e.widget.list.as_ref() {
                Some(l) => l.entries.iter().copied().collect(),
                None => return Err((0, UiError::NotAList)),
            },
            _ => return Err((0, UiError::NotAList)),
        };
        let mut result = Ok(());
        for (i, &entry) in entries.iter().enumerate() {
            if present.len() >= MAX_ENTRIES && !present.contains(&entry) {
                result = Err((i, UiError::Full));
                break;
            }
            if self.parent(entry) != Some(list)
                && let Err(e) = self.attach_raw(entry, list)
            {
                result = Err((i, e));
                break;
            }
            if present.insert(entry)
                && let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut())
            {
                l.entries.push(entry);
                l.dirty = true;
            }
        }
        self.list_layout_if_dirty(list);
        result
    }

    /// Adds an entry from template *i* of `UICore_ListBox_entry_templates` (§8.2).
    ///
    /// # Errors
    /// [`UiError::NoTemplate`] when the template is missing or malformed, or a creation error.
    pub fn list_add_template(&mut self, list: ElementKey, i: usize) -> Result<ElementKey, UiError> {
        let p = self.v.pids;
        let (layout, element) = {
            let v = self
                .prop(list, p.lb_templates)
                .value()
                .ok_or(UiError::NoTemplate)?;
            let t = &array_elements(v).get(i).ok_or(UiError::NoTemplate)?.value;
            let lay = struct_field(t, p.lb_template_layout)
                .map(as_key)
                .ok_or(UiError::NoTemplate)?;
            let el = struct_field(t, p.lb_template_element)
                .map(as_key)
                .ok_or(UiError::NoTemplate)?;
            (lay, el)
        };
        let k = self.create_element(Some(list), Did(layout), element)?;
        if let Err(e) = self.list_add(list, k, None) {
            self.destroy(k);
            return Err(e);
        }
        Ok(k)
    }

    /// `entry_templates` changed with autofill (§8.2).
    pub(crate) fn list_autofill(&mut self, list: ElementKey) {
        let p = self.v.pids;
        if !self.prop(list, p.lb_autofill).bool() {
            return;
        }
        let n = self
            .prop(list, p.lb_templates)
            .value()
            .map_or(0, |v| array_elements(v).len());
        for i in 0..n.min(MAX_ENTRIES) {
            let _ = self.list_add_template(list, i);
        }
    }

    /// Runs the list layout when it is dirty (§8.3; Tessera runs it before the next hit
    /// test or paint, decision 0014 D8).
    pub(crate) fn list_layout_if_dirty(&mut self, list: ElementKey) {
        if self
            .el(list)
            .and_then(|e| e.widget.list.as_ref())
            .is_some_and(|l| l.dirty)
        {
            self.list_layout(list);
        }
    }

    /// Lays out every dirty list (before hit tests and painting).
    pub(crate) fn flush_list_layouts(&mut self) {
        let lists: Vec<ElementKey> = self.lists.iter().copied().collect();
        for k in lists {
            self.list_layout_if_dirty(k);
        }
    }

    /// The tree's nodes by entry, or `None` for a list box that is not a tree.
    fn tree_map(&self, list: ElementKey) -> Option<HashMap<ElementKey, TreeNode>> {
        let t = self.el(list)?.widget.tree.as_ref()?;
        Some(t.nodes.iter().map(|n| (n.entry, n.clone())).collect())
    }

    /// The tree width addition of an entry (§9.3): depth × indent + icon width. The depth
    /// walk is bounded by the node count.
    fn tree_extra_width(
        &self,
        list: ElementKey,
        nodes: &HashMap<ElementKey, TreeNode>,
        entry: ElementKey,
    ) -> i32 {
        let Some(n) = nodes.get(&entry) else {
            return 0;
        };
        let indent = self.prop(list, self.v.pids.tree_indent).int();
        let mut depth = 1i32;
        let mut cur = n.parent;
        while let Some(pe) = cur {
            if depth as usize > nodes.len() {
                break;
            }
            depth += 1;
            cur = nodes.get(&pe).and_then(|pn| pn.parent);
        }
        let icon_w = n
            .icon
            .and_then(|ic| self.layout_box(ic))
            .map_or(0, |b| b.w());
        depth.saturating_mul(indent).saturating_add(icon_w)
    }

    /// The list layout (§8.3).
    pub(crate) fn list_layout(&mut self, list: ElementKey) {
        let p = self.v.pids;
        let Some(lp) = self.el(list).and_then(|e| e.widget.list.clone()) else {
            return;
        };
        if let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
            l.dirty = false;
        }
        // Step 1.
        let only_visible = self.prop(list, p.lb_only_visible).bool();
        let laid: Vec<ElementKey> = lp
            .entries
            .iter()
            .copied()
            .filter(|e| self.exists(*e) && (!only_visible || self.is_visible(*e)))
            .collect();
        if let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
            l.laid_out = laid.clone();
        }
        if let Some(sel) = lp.selected
            && !laid.contains(&sel)
        {
            if let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
                l.selected = None;
            }
            self.post(
                list,
                self.v.msgs.listbox_new_selection,
                MsgArg {
                    value: -1,
                    ..MsgArg::default()
                },
            );
        }
        // Step 2.
        let n = laid.len() as i32;
        let horizontal = self.prop(list, p.lb_horizontal).bool();
        let mcp = self.prop(list, p.lb_max_columns);
        let mrp = self.prop(list, p.lb_max_rows);
        let (mc, mr) = (mcp.int(), mrp.int());
        let ceil_div = |a: i32, b: i32| {
            if b <= 0 {
                0
            } else {
                ((i64::from(a) + i64::from(b) - 1) / i64::from(b)) as i32
            }
        };
        let (cols, rows) = if horizontal {
            if mc < 0 {
                (n, i32::from(n > 0))
            } else {
                let cols = if mc == 0 { n } else { mc.min(n) };
                let mut rows = if cols == 0 { 0 } else { ceil_div(n, cols) };
                if mrp.is_set() {
                    rows = rows.min(mr);
                }
                (cols, rows)
            }
        } else if mr < 0 {
            (i32::from(n > 0), n)
        } else {
            let rows = if mr == 0 { n } else { mr.min(n) };
            let mut cols = if rows == 0 { 0 } else { ceil_div(n, rows) };
            if mcp.is_set() {
                cols = cols.min(mc);
            }
            (cols, rows)
        };
        let (cols, rows) = (cols.max(0), rows.max(0));
        if let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
            l.grid = (cols, rows);
        }
        // Step 3.
        let tree_nodes = self.tree_map(list);
        let mut colw = vec![0i32; cols as usize];
        let mut rowh = vec![0i32; rows as usize];
        let advance = |c: &mut i32, r: &mut i32| {
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
        let inside = |c: i32, r: i32| c < cols && r < rows;
        let (mut c, mut r) = (0, 0);
        for e in &laid {
            if !inside(c, r) {
                break;
            }
            let b = self.layout_box(*e).unwrap_or_default();
            let extra = tree_nodes
                .as_ref()
                .map_or(0, |m| self.tree_extra_width(list, m, *e));
            let w = b.w().saturating_add(extra);
            colw[c as usize] = colw[c as usize].max(w);
            rowh[r as usize] = rowh[r as usize].max(b.h());
            advance(&mut c, &mut r);
        }
        // Step 4.
        let cw: i32 = colw.iter().fold(0i32, |a, b| a.saturating_add(*b));
        let ch: i32 = rowh.iter().fold(0i32, |a, b| a.saturating_add(*b));
        self.set_content_size(list, cw, ch);
        // Step 5.
        let Some(bx) = self.layout_box(list) else {
            return;
        };
        let (big_w, big_h) = (bx.w(), bx.h());
        let (sx, sy) = self
            .el(list)
            .and_then(|e| e.widget.scroll.as_ref())
            .map(|s| s.offset)
            .unwrap_or((0, 0));
        let reverse = self.prop(list, p.lb_reverse).bool();
        let flipped = self.prop(list, p.lb_flipped).bool();
        let ignore = self.prop(list, p.lb_ignore_cell).bool();
        let wrap = self.prop(list, p.lb_wrap_indent).int();
        let mut x = sub(0, sx);
        let mut y = sub(0, sy);
        if reverse && let Some(w0) = colw.first() {
            x = add(x, sub(big_w, *w0));
        }
        if (reverse ^ flipped)
            && let Some(h0) = rowh.first()
        {
            y = add(y, sub(big_h, *h0));
        }
        let mut wi = 0i32;
        let (mut c, mut r) = (0i32, 0i32);
        for e in &laid {
            if !inside(c, r) {
                break;
            }
            let eb = self.layout_box(*e).unwrap_or_default();
            let (cellw, cellh) = if ignore {
                (eb.w(), eb.h())
            } else {
                (colw[c as usize], rowh[r as usize])
            };
            if self.parent(*e) == Some(list) {
                if let Some(m) = tree_nodes.as_ref() {
                    self.tree_place(list, m, *e, x, y);
                } else {
                    self.move_to(*e, x, y);
                }
            }
            if horizontal {
                if c + 1 < cols {
                    x = if reverse {
                        sub(x, cellw)
                    } else {
                        add(x, cellw)
                    };
                } else {
                    wi = wi.saturating_add(wrap);
                    x = if reverse {
                        sub(sub(sub(big_w, wi), sx), cellw)
                    } else {
                        sub(wi, sx)
                    };
                    y = if !(flipped ^ reverse) {
                        add(y, cellh)
                    } else {
                        sub(y, cellh)
                    };
                }
            } else if r + 1 < rows {
                let h = rowh[r as usize];
                y = if reverse { sub(y, h) } else { add(y, h) };
            } else {
                wi = wi.saturating_add(wrap);
                let h = rowh[r as usize];
                y = if reverse {
                    sub(sub(sub(big_h, sy), h), wi)
                } else {
                    sub(wi, sy)
                };
                let w = colw[c as usize];
                x = if !(flipped ^ reverse) {
                    add(x, w)
                } else {
                    sub(x, w)
                };
            }
            advance(&mut c, &mut r);
        }
        self.mark_dirty(list);
    }

    /// Select entry *e* (§8.4; a Tree toggles the node first, §9.4).
    pub fn list_select(&mut self, list: ElementKey, entry: Option<ElementKey>, notify: bool) {
        let p = self.v.pids;
        if let Some(e) = entry
            && self.el(list).is_some_and(|l| l.widget.tree.is_some())
        {
            self.tree_toggle(list, e);
        }
        let Some(lp) = self.el(list).and_then(|e| e.widget.list.clone()) else {
            return;
        };
        // Step 1.
        if entry.is_some() && lp.selected == entry {
            if notify {
                self.post(
                    list,
                    self.v.msgs.listbox_same_selection,
                    MsgArg {
                        value2: entry.map_or(0, |e| e.owner() as i64),
                        ..MsgArg::default()
                    },
                );
            }
            return;
        }
        // Step 2.
        let idx = entry.and_then(|e| lp.laid_out.iter().position(|x| *x == e));
        let entry = if idx.is_some() { entry } else { None };
        // Step 3.
        let old = lp.selected;
        if let Some(l) = self.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
            l.selected = entry;
        }
        // Step 4.
        if self.prop(list, p.lb_state_change).bool() {
            let ns = self.prop(list, p.lb_normal_state).key();
            let ss = self.prop(list, p.lb_selected_state).key();
            if let Some(o) = old {
                self.request_state(o, ns);
            }
            if let Some(e) = entry {
                self.request_state(e, ss);
            }
        }
        // Step 5.
        if notify {
            self.post(
                list,
                self.v.msgs.listbox_new_selection,
                MsgArg {
                    value: idx.map_or(-1, |i| i as i64),
                    value2: entry.map_or(0, |e| e.owner() as i64),
                    ..MsgArg::default()
                },
            );
        }
    }

    /// The list box's handling of presses from itself and its entries (§8.4).
    pub(crate) fn list_message(&mut self, list: ElementKey, m: &Message) -> Answer {
        if m.msg != self.v.msgs.mouse_press {
            return Answer::NotHandled;
        }
        let left = m
            .arg
            .command
            .as_ref()
            .is_some_and(|c| &*c.name == commands::SELECT_LEFT);
        if !left || !self.prop(list, self.v.pids.lb_click_select).bool() {
            return Answer::NotHandled;
        }
        self.list_layout_if_dirty(list);
        // Tessera rule (§8.4 OPEN): the first laid-out entry whose box contains the point.
        let (px, py) = self.input.pointer.unwrap_or((i32::MIN, i32::MIN));
        let laid = self
            .el(list)
            .and_then(|e| e.widget.list.as_ref())
            .map(|l| l.laid_out.clone())
            .unwrap_or_default();
        let hit = laid
            .into_iter()
            .find(|e| self.is_visible(*e) && self.displayed_box(*e).contains(px, py));
        if hit.is_some() {
            self.list_select(list, hit, true);
        }
        Answer::NotHandled
    }

    // ----- Tree (§9) ---------------------------------------------------------------------

    /// The node of `entry` in `tree`, if any.
    fn tree_node(&self, tree: ElementKey, entry: ElementKey) -> Option<TreeNode> {
        self.el(tree)
            .and_then(|e| e.widget.tree.as_ref())
            .and_then(|t| t.nodes.iter().find(|n| n.entry == entry).cloned())
    }

    fn tree_add_node(&mut self, tree: ElementKey, entry: ElementKey, parent: Option<ElementKey>) {
        let p = self.v.pids;
        // A node is created once per entry (a second `list_add` of the same entry adds no
        // node and no icon part).
        if self.tree_node(tree, entry).is_some() {
            return;
        }
        // The parent node must exist (and not be the entry itself), else the node is a
        // top-level node. Parents therefore always exist before their children, so the
        // parent links never form a cycle.
        let parent = parent.filter(|pe| *pe != entry && self.tree_node(tree, *pe).is_some());
        let expanded = self.prop(tree, p.tree_auto_expand).bool();
        let indent_id = self.prop(tree, p.tree_indent_element).key();
        let indent = (indent_id != 0)
            .then(|| self.find_child(entry, indent_id))
            .flatten();
        let icon_layout = self.prop(tree, p.tree_icon_layout).key();
        let icon_id = self.prop(tree, p.tree_icon_id).key();
        let icon = if icon_layout != 0 && icon_id != 0 {
            self.create_element(Some(tree), Did(icon_layout), icon_id)
                .ok()
        } else {
            None
        };
        if let Some(t) = self.el_mut(tree).and_then(|e| e.widget.tree.as_mut()) {
            t.nodes.push(TreeNode {
                entry,
                parent,
                expanded,
                icon,
                indent,
            });
        }
        let st = if expanded {
            self.v.states.open
        } else {
            self.v.states.closed
        };
        if let Some(ic) = icon {
            self.request_state(ic, st);
        }
        // A node under a collapsed or hidden parent starts hidden.
        if let Some(pe) = parent {
            let show = self.tree_node_open_and_visible(tree, pe);
            self.set_shown(entry, show);
            if let Some(ic) = icon {
                self.set_shown(ic, show);
            }
        }
    }

    fn tree_node_open_and_visible(&self, tree: ElementKey, entry: ElementKey) -> bool {
        self.tree_node(tree, entry)
            .is_some_and(|n| n.expanded && self.is_visible(n.entry))
    }

    /// Repairs a tree after nodes were removed from it ([`tree_remove_nodes`]): destroys the
    /// icon parts of the removed nodes and re-applies visibility to the nodes that moved up
    /// to a new parent.
    pub(crate) fn tree_after_removal(&mut self, tree: ElementKey, r: TreeRemoval) {
        for ic in r.icons {
            self.destroy(ic);
        }
        if !self.exists(tree) {
            return;
        }
        for e in r.reparented {
            let Some(n) = self.tree_node(tree, e) else {
                continue;
            };
            let show = match n.parent {
                Some(pe) => self.tree_node_open_and_visible(tree, pe),
                None => true,
            };
            self.set_shown(e, show);
            if let Some(ic) = n.icon {
                self.set_shown(ic, show);
            }
            self.tree_apply_children(tree, e);
        }
        if let Some(l) = self.el_mut(tree).and_then(|e| e.widget.list.as_mut()) {
            l.dirty = true;
        }
    }

    /// Expands or collapses the node of `entry` (§9.2).
    pub fn tree_set_expanded(&mut self, tree: ElementKey, entry: ElementKey, expanded: bool) {
        if self.tree_node(tree, entry).is_none() {
            return;
        }
        self.tree_set_expanded_node(tree, entry, expanded);
        if let Some(l) = self.el_mut(tree).and_then(|e| e.widget.list.as_mut()) {
            l.dirty = true;
        }
        self.list_layout_if_dirty(tree);
    }

    /// §9.2 "Expanding or collapsing": when expanding, the collapsed ancestors directly
    /// above are expanded first, topmost first (iteratively, bounded by the node count).
    fn tree_set_expanded_node(&mut self, tree: ElementKey, entry: ElementKey, expanded: bool) {
        let mut chain = vec![entry];
        if expanded {
            let limit = self
                .el(tree)
                .and_then(|e| e.widget.tree.as_ref())
                .map_or(0, |t| t.nodes.len());
            let mut cur = self.tree_node(tree, entry).and_then(|n| n.parent);
            while let Some(pe) = cur {
                if chain.len() > limit || chain.contains(&pe) {
                    break;
                }
                match self.tree_node(tree, pe) {
                    Some(pn) if !pn.expanded => {
                        chain.push(pe);
                        cur = pn.parent;
                    }
                    _ => break,
                }
            }
        }
        for e in chain.into_iter().rev() {
            self.tree_set_one(tree, e, expanded);
        }
    }

    /// Records the flag of one node, sets its icon and applies it to the node's subtree.
    fn tree_set_one(&mut self, tree: ElementKey, entry: ElementKey, expanded: bool) {
        let mut icon = None;
        if let Some(t) = self.el_mut(tree).and_then(|e| e.widget.tree.as_mut())
            && let Some(nn) = t.nodes.iter_mut().find(|n| n.entry == entry)
        {
            nn.expanded = expanded;
            icon = nn.icon;
        }
        let st = if expanded {
            self.v.states.open
        } else {
            self.v.states.closed
        };
        if let Some(ic) = icon {
            self.request_state(ic, st);
            let v = self.is_visible(entry);
            self.set_shown(ic, v);
        }
        self.tree_apply_children(tree, entry);
    }

    /// Shows or hides the subtree below the node of `entry` (§9.2): each child's entry and
    /// icon are shown exactly when its parent is expanded and visible. Iterative, each node
    /// visited once.
    fn tree_apply_children(&mut self, tree: ElementKey, entry: ElementKey) {
        let Some(nodes) = self
            .el(tree)
            .and_then(|e| e.widget.tree.as_ref())
            .map(|t| t.nodes.clone())
        else {
            return;
        };
        let mut kids: HashMap<ElementKey, Vec<usize>> = HashMap::new();
        let mut expanded: HashMap<ElementKey, bool> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            expanded.insert(n.entry, n.expanded);
            if let Some(pe) = n.parent {
                kids.entry(pe).or_default().push(i);
            }
        }
        let mut seen: HashSet<ElementKey> = HashSet::new();
        let mut stack = vec![entry];
        while let Some(pe) = stack.pop() {
            if !seen.insert(pe) {
                continue;
            }
            let show = expanded.get(&pe).copied().unwrap_or(false) && self.is_visible(pe);
            for ci in kids.get(&pe).map(Vec::as_slice).unwrap_or(&[]) {
                let c = &nodes[*ci];
                self.set_shown(c.entry, show);
                if let Some(ic) = c.icon {
                    self.set_shown(ic, show);
                }
                stack.push(c.entry);
            }
        }
    }

    /// Toggle on select (§9.4).
    fn tree_toggle(&mut self, tree: ElementKey, entry: ElementKey) {
        let Some(t) = self.el(tree).and_then(|e| e.widget.tree.as_ref()) else {
            return;
        };
        let Some(n) = t.nodes.iter().find(|n| n.entry == entry) else {
            return;
        };
        let has_children = t.nodes.iter().any(|c| c.parent == Some(entry));
        let exp = if has_children { !n.expanded } else { true };
        self.tree_set_expanded_node(tree, entry, exp);
        if let Some(l) = self.el_mut(tree).and_then(|e| e.widget.list.as_mut()) {
            l.dirty = true;
        }
        self.list_layout_if_dirty(tree);
    }

    /// Placement of a node's parts (§9.3); `nodes` is the tree's node map by entry.
    fn tree_place(
        &mut self,
        tree: ElementKey,
        nodes: &HashMap<ElementKey, TreeNode>,
        entry: ElementKey,
        x: i32,
        y: i32,
    ) {
        let Some(n) = nodes.get(&entry) else {
            self.move_to(entry, x, y);
            return;
        };
        let indent = self.prop(tree, self.v.pids.tree_indent).int();
        let mut ex = x;
        if let Some(pe) = n.parent {
            let b = match nodes.get(&pe) {
                Some(pn) => match pn.icon {
                    Some(ic) => self.layout_box(ic).map_or(0, |b| b.l),
                    None => {
                        let ent = self.layout_box(pn.entry).map_or(0, |b| b.l);
                        let ind = pn
                            .indent
                            .and_then(|i| self.layout_box(i))
                            .map_or(0, |b| b.l);
                        ent.saturating_add(ind)
                    }
                },
                None => 0,
            };
            ex = indent.saturating_add(b);
        }
        if let Some(ic) = n.icon {
            self.move_to(ic, ex, y);
            if self.prop(tree, self.v.pids.tree_indent_for_icon).bool() {
                ex = ex.saturating_add(self.layout_box(ic).map_or(0, |b| b.w()));
            }
        }
        if let Some(ind) = n.indent {
            self.move_to(entry, x, y);
            let iy = self.layout_box(ind).map_or(0, |b| b.t);
            self.move_to(ind, sub(ex, x), iy);
        } else {
            self.move_to(entry, ex, y);
        }
    }

    /// Tree completion (§9.1): click selection and layout of visible entries only.
    pub(crate) fn tree_completion(&mut self, tree: ElementKey) {
        let p = self.v.pids;
        self.set_property(tree, p.lb_click_select, PropertyValue::Bool(true));
        self.set_property(tree, p.lb_only_visible, PropertyValue::Bool(true));
    }
}

/// What removing nodes from a tree left to do ([`Ui::tree_after_removal`]).
#[derive(Debug, Default)]
pub(crate) struct TreeRemoval {
    /// Icon parts of removed nodes that are still alive (children of the tree).
    pub icons: Vec<ElementKey>,
    /// Entries of nodes that moved up to a new parent.
    pub reparented: Vec<ElementKey>,
}

impl TreeRemoval {
    /// Whether nothing is left to do.
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty() && self.reparented.is_empty()
    }
}

/// Removes from a tree every node whose entry is `gone`, and drops references to `gone`
/// icon and indent parts. A node whose parent was removed moves up to the nearest ancestor
/// that stays (a top-level node when none does; Tessera rule, `ui-widgets.md` §9.2 OPEN).
pub(crate) fn tree_remove_nodes(
    t: &mut TreePart,
    gone: impl Fn(ElementKey) -> bool,
) -> TreeRemoval {
    let mut r = TreeRemoval::default();
    let removed: HashMap<ElementKey, Option<ElementKey>> = t
        .nodes
        .iter()
        .filter(|n| gone(n.entry))
        .map(|n| (n.entry, n.parent))
        .collect();
    for n in t.nodes.iter().filter(|n| gone(n.entry)) {
        if let Some(ic) = n.icon
            && !gone(ic)
        {
            r.icons.push(ic);
        }
    }
    if !removed.is_empty() {
        t.nodes.retain(|n| !gone(n.entry));
    }
    for n in &mut t.nodes {
        if n.icon.is_some_and(&gone) {
            n.icon = None;
        }
        if n.indent.is_some_and(&gone) {
            n.indent = None;
        }
        if removed.is_empty() {
            continue;
        }
        let mut p = n.parent;
        let mut moved = false;
        let mut steps = 0usize;
        while let Some(pe) = p {
            match removed.get(&pe) {
                Some(pp) => {
                    p = *pp;
                    moved = true;
                    steps += 1;
                    if steps > removed.len() {
                        p = None;
                        break;
                    }
                }
                None => break,
            }
        }
        if moved {
            n.parent = p;
            r.reparented.push(n.entry);
        }
    }
    r
}
