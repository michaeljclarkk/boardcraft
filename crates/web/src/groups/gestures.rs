//! Composition of stock Kit controls with the shared native/wasm drag state.
use super::*;
use gpui_kit::base::ElementExt;
use gpui_kit::{
    AnyElement, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, canvas,
};

impl Table {
    pub(super) fn render_groups(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = self.drag.clone();
        let mut groups = div()
            .id("physical-groups")
            .relative()
            .flex()
            .flex_col()
            .gap_3()
            .on_prepaint(move |bounds, w, _| {
                geometry
                    .borrow_mut()
                    .set_viewport(bounds.intersect(&w.visual_viewport_bounds()))
            });
        for (ix, g) in self.read.groups().iter().enumerate() {
            let id = g.id().to_owned();
            let name = g.name().to_owned();
            let collapsed = self.folded(&id, g.is_collapsed());
            let selected = g
                .items()
                .iter()
                .filter(|r| self.selection.contains(&r.id))
                .count();
            let mut header = div().flex().items_center().gap_2();
            let geometry = self.drag.clone();
            let gid = id.clone();
            header = header.on_prepaint(move |bounds, _, _| {
                if let Some(s) = geometry.borrow_mut().section_mut(&gid) {
                    s.set_heading(bounds);
                }
            });
            let fold_id = id.clone();
            header = header.child(
                Button::new(key(format!("fold-{id}")))
                    .label(if collapsed { "▸" } else { "▾" })
                    .accessibility_label(format!("Toggle {name}"))
                    .accessibility_id(id.clone())
                    .aria_expanded(!collapsed)
                    .disabled(self.busy())
                    .on_click(cx.listener(move |t, _, w, cx| {
                        t.fold(vec![fold_id.clone()], !collapsed, w, cx)
                    })),
            );
            let geometry = self.drag.clone();
            let gid = id.clone();
            let edit_id = id.clone();
            header = header.child(
                div()
                    .on_prepaint(move |bounds, _, _| {
                        if let Some(s) = geometry.borrow_mut().section_mut(&gid) {
                            s.set_name(bounds);
                        }
                    })
                    .child(
                        Button::new(key(format!("heading-{id}")))
                            .label(name.clone())
                            .accessibility_id(format!("heading-{id}"))
                            .disabled(self.busy() || !self.read.is_manageable())
                            .on_click(cx.listener(move |t, e: &gpui_kit::ClickEvent, w, cx| {
                                if e.click_count() == 2 || e.is_keyboard() {
                                    t.open_editor(Kind::Group(Some(edit_id.clone())), w, cx);
                                }
                            })),
                    ),
            );
            header = header.child(format!("{} items", g.items().len()));
            // Saved colour is data. Its text also conveys it without relying on colour perception.
            header = header.child(g.colour().to_owned());
            let weak = cx.weak_entity();
            let menu_id = id.clone();
            let manageable = self.read.is_manageable();
            let busy = self.busy();
            let last = ix + 1 == self.read.groups().len();
            header = header.child(
                Button::new(key(format!("group-menu-{id}")))
                    .label("Group options")
                    .accessibility_id(id.clone())
                    .disabled(busy)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        for (label, delta, enabled) in
                            [("Move group up", -1, ix > 0), ("Move group down", 1, !last)]
                        {
                            let weak = weak.clone();
                            let id = menu_id.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label)
                                    .disabled(!manageable || !enabled)
                                    .on_click(move |_, w, cx| {
                                        let _ = weak
                                            .update(cx, |t, cx| t.group_order(&id, delta, w, cx));
                                    }),
                            );
                        }
                        for label in ["Rename group…", "Change group colour…"] {
                            let weak = weak.clone();
                            let id = menu_id.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label).disabled(!manageable).on_click(
                                    move |_, w, cx| {
                                        let _ = weak.update(cx, |t, cx| {
                                            t.open_editor(Kind::Group(Some(id.clone())), w, cx)
                                        });
                                    },
                                ),
                            );
                        }
                        for all in [false, true] {
                            let weak = weak.clone();
                            let id = menu_id.clone();
                            let label = match (collapsed, all) {
                                (true, false) => "Expand group",
                                (true, true) => "Expand all groups",
                                (false, false) => "Collapse group",
                                (false, true) => "Collapse all groups",
                            };
                            menu =
                                menu.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                                    let _ = weak.update(cx, |t, cx| {
                                        let ids = if all {
                                            t.read
                                                .groups()
                                                .iter()
                                                .map(|g| g.id().to_owned())
                                                .collect()
                                        } else {
                                            vec![id.clone()]
                                        };
                                        t.fold(ids, !collapsed, w, cx);
                                    });
                                }));
                        }
                        let weak = weak.clone();
                        let id = menu_id.clone();
                        menu.item(
                            PopupMenuItem::new("Archive group…")
                                .disabled(!manageable)
                                .on_click(move |_, w, cx| {
                                    let _ = weak.update(cx, |t, cx| {
                                        t.open_editor(Kind::ArchiveGroup(id.clone()), w, cx)
                                    });
                                }),
                        )
                    }),
            );
            let geometry = self.drag.clone();
            let gid = id.clone();
            let mut section = div()
                .id(key(format!("section-{id}")))
                .role(Role::Group)
                .aria_label(name.clone())
                .relative()
                .flex()
                .flex_col()
                .gap_2()
                .on_prepaint(move |bounds, _, _| {
                    if let Some(s) = geometry.borrow_mut().section_mut(&gid) {
                        s.set_section(bounds);
                    }
                })
                .child(header);
            if !collapsed {
                let ids: Vec<_> = g.items().iter().map(|r| r.id).collect();
                section = section.child(
                    Checkbox::new(key(format!("check-group-{id}")))
                        .label(format!("Select group {id}"))
                        .checked(!ids.is_empty() && selected == ids.len())
                        .indeterminate(selected > 0 && selected < ids.len())
                        .disabled(self.busy() || ids.is_empty())
                        .on_change(cx.listener(move |t, checked, w, cx| {
                            t.select(ids.clone(), *checked, None, w, cx)
                        })),
                );
                if let Some(drag) = self.row_drags.get(&id) {
                    let geometry = drag.clone();
                    let mut rows =
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_prepaint(move |bounds, w, _| {
                                geometry
                                    .borrow_mut()
                                    .set_viewport(bounds.intersect(&w.visual_viewport_bounds()))
                            });
                    for item in g.items() {
                        let item_id = item.id;
                        let checked = self.selection.contains(&item_id);
                        let geometry = drag.clone();
                        let rid = item_id.to_string();
                        let mut row = div()
                            .id(key(format!("row-{item_id}")))
                            .relative()
                            .flex()
                            .items_center()
                            .gap_2()
                            .bg(if checked {
                                cx.theme().accent
                            } else {
                                cx.theme().background
                            })
                            .on_prepaint(move |bounds, _, _| {
                                if let Some(s) = geometry.borrow_mut().section_mut(&rid) {
                                    s.set_heading(bounds);
                                    s.set_section(bounds);
                                }
                            })
                            .child(
                                Checkbox::new(key(format!("check-{item_id}")))
                                    .accessibility_label(format!("Select {item_id}"))
                                    .checked(checked)
                                    .disabled(self.busy())
                                    .on_change(cx.listener(move |t, checked, w, cx| {
                                        t.select_row(item_id, *checked, w.modifiers().shift, w, cx)
                                    })),
                            );
                        let geometry = drag.clone();
                        let rid = item_id.to_string();
                        row = row.child(
                            div()
                                .on_prepaint(move |bounds, _, _| {
                                    if let Some(s) = geometry.borrow_mut().section_mut(&rid) {
                                        s.set_name(bounds);
                                    }
                                })
                                .child(
                                    Button::new(key(format!("grip-{item_id}")))
                                        .label("⋮⋮")
                                        .accessibility_label(format!("Drag {}", item.title))
                                        .accessibility_id(format!("grip-{item_id}"))
                                        .disabled(self.busy() || !self.read.is_editable()),
                                ),
                        );
                        if let Some(name) = self.rows.get(&item_id) {
                            row = row.child(name.clone());
                        }
                        let weak = cx.weak_entity();
                        let editable = self.read.is_editable();
                        let manageable = self.read.is_manageable();
                        row = row.child(
                            Button::new(key(format!("item-menu-{item_id}")))
                                .label("More options")
                                .accessibility_id(format!("menu-{item_id}"))
                                .disabled(self.busy())
                                .dropdown_menu(move |menu, _, _| {
                                    let moving = weak.clone();
                                    let archive = weak.clone();
                                    menu.item(
                                        PopupMenuItem::new("Move to…")
                                            .disabled(!editable)
                                            .on_click(move |_, w, cx| {
                                                let _ = moving.update(cx, |t, cx| {
                                                    t.open_editor(Kind::Move(item_id), w, cx)
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("Archive item…")
                                            .disabled(!manageable)
                                            .on_click(move |_, w, cx| {
                                                let _ = archive.update(cx, |t, cx| {
                                                    t.open_editor(Kind::ArchiveRoot(item_id), w, cx)
                                                });
                                            }),
                                    )
                                }),
                        );
                        if let Some((target, after)) = drag.borrow().edge()
                            && target == item_id.to_string()
                        {
                            let line = div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .h(gpui_kit::px(1.))
                                .bg(cx.theme().primary);
                            row = row.child(if after { line.bottom_0() } else { line.top_0() });
                        }
                        rows = rows.child(row);
                    }
                    section = section.child(rows);
                }
                if let Some(entry) = self.entries.get(&id) {
                    let group = id.clone();
                    let input = entry.input.clone();
                    section = section.child(
                        div()
                            .flex()
                            .gap_2()
                            .capture_key_down(cx.listener(
                                move |t, event: &gpui_kit::KeyDownEvent, w, cx| {
                                    if event.keystroke.key == "escape" && !t.pending {
                                        input.update(cx, |i, cx| i.set_value("", w, cx));
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                            .child(
                                Input::new(&entry.input)
                                    .aria_label(format!("Add item to {id}"))
                                    .disabled(self.busy() || !self.read.is_editable()),
                            )
                            .child(
                                Button::new(key(format!("create-{id}")))
                                    .label("Add item")
                                    .accessibility_id(id.clone())
                                    .disabled(self.busy() || !self.read.is_editable())
                                    .on_click(
                                        cx.listener(move |t, _, w, cx| t.create(&group, w, cx)),
                                    ),
                            ),
                    );
                }
            }
            if let Some((target, after)) = self.drag.borrow().edge()
                && target == id
            {
                // An insertion cue cannot move the heading out from under the
                // pointer before release. Keep it out of the flex flow.
                let line = div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .h(gpui_kit::px(1.))
                    .bg(cx.theme().primary);
                section = section.child(if after { line.bottom_0() } else { line.top_0() });
            }
            groups = groups.child(section);
        }
        groups.child(self.drag_events(cx))
    }
    fn drag_events(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        canvas(
            |_, _, _| (),
            move |_, _, w, _| {
                let weak = view.clone();
                w.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
                    if phase != DispatchPhase::Capture
                        || e.button != MouseButton::Left
                        || w.last_input_was_touch()
                    {
                        return;
                    }
                    let _ = weak.update(cx, |t, cx| {
                        if t.busy() || t.inventory.is_some() {
                            return;
                        }
                        if t.read.is_manageable() {
                            t.drag.borrow_mut().press(e.position);
                        }
                        if !t.drag.borrow().is_held() && t.read.is_editable() {
                            for (id, drag) in &t.row_drags {
                                if t.read
                                    .groups()
                                    .iter()
                                    .any(|g| g.id() == id && !t.folded(id, g.is_collapsed()))
                                {
                                    drag.borrow_mut().press(e.position);
                                }
                            }
                        }
                        if t.drag.borrow().is_held()
                            || t.row_drags.values().any(|d| d.borrow().is_held())
                        {
                            t.focus.focus(w, cx);
                        }
                    });
                });
                let weak = view.clone();
                w.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let _ = weak.update(cx, |t, cx| {
                        for drag in std::iter::once(&t.drag).chain(t.row_drags.values()) {
                            let mut drag = drag.borrow_mut();
                            if drag.is_held() {
                                if e.pressed_button == Some(MouseButton::Left) {
                                    drag.move_pointer(e.position);
                                } else {
                                    drag.cancel();
                                }
                                if drag.is_active() {
                                    cx.stop_propagation();
                                }
                                cx.notify();
                            }
                        }
                    });
                });
                let weak = view.clone();
                w.on_mouse_event(move |e: &MouseUpEvent, phase, w, cx| {
                    if phase != DispatchPhase::Capture || e.button != MouseButton::Left {
                        return;
                    }
                    let _ = weak.update(cx, |t, cx| {
                        let order = t.drag.borrow_mut().release(e.position);
                        if t.drag.borrow_mut().consume_click() {
                            cx.stop_propagation();
                        }
                        let mut command = order.map(|ids| TableCommand::GroupOrder {
                            board_revision: t.read.revision(),
                            ids: ids.iter().map(ToString::to_string).collect(),
                        });
                        for (group, drag) in &t.row_drags {
                            let mut drag = drag.borrow_mut();
                            let source = drag.source_id().and_then(|id| id.parse::<Uuid>().ok());
                            let edge = drag.release_edge(e.position);
                            if drag.consume_click() {
                                cx.stop_propagation();
                            }
                            if let (Some(id), Some((anchor, after))) = (source, edge)
                                && let Ok(anchor) = anchor.parse::<Uuid>()
                                && let Some(item) = t
                                    .read
                                    .groups()
                                    .iter()
                                    .flat_map(|g| g.items())
                                    .find(|r| r.id == id)
                            {
                                command = Some(TableCommand::Reorder {
                                    id,
                                    command: ItemReorder::new(
                                        Uuid::new_v4(),
                                        group,
                                        anchor,
                                        after,
                                        t.read.revision(),
                                        item.revision,
                                    ),
                                });
                            }
                        }
                        if let Some(command) = command {
                            t.submit(command, None, None, w, cx);
                        }
                        cx.notify();
                    });
                });
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}
