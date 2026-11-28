//! Composition of stock Kit controls with the shared native/wasm drag state.
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::base::ElementExt;
use gpui_kit::component::{Sizable, button::ButtonVariants};
use gpui_kit::{
    AnyElement, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, canvas,
};

// Keep the selection column aligned through the header, item and entry bands.
// The leading slot gives the existing drag handle its own hit target.
fn selection_cell() -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_end()
        .pr_2()
        .w(gpui_kit::rems(3.5))
        .flex_shrink_0()
}

fn table_band(cx: &App) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .min_h_9()
        .border_t_1()
        .border_color(cx.theme().border)
}

impl Table {
    pub(super) fn render_groups(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = self.drag.clone();
        let scroll = self.scroll.clone();
        let mut groups = div()
            .id("physical-groups")
            .relative()
            .flex()
            .flex_col()
            .gap_6()
            .on_prepaint(move |bounds, w, _| {
                geometry.borrow_mut().set_viewport(
                    bounds
                        .intersect(&scroll.bounds())
                        .intersect(&w.visual_viewport_bounds()),
                )
            });
        for (ix, g) in self.read.groups().iter().enumerate() {
            let id = g.id().to_owned();
            let name = g.name().to_owned();
            // This user-chosen colour is part of the accepted record, independent of
            // selection, hover, and drag-feedback tokens.
            let hue = g
                .colour()
                .strip_prefix('#')
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .map(|rgb| gpui_kit::rgb(rgb).into())
                .unwrap_or(cx.theme().foreground);
            let collapsed = self.folded(&id, g.is_collapsed());
            let selected = g
                .items()
                .iter()
                .filter(|r| self.selection.contains(&r.id))
                .count();
            let mut header = div().flex().min_h_9().items_center().gap_1();
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
                    .small()
                    .ghost()
                    .icon(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
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
                            .small()
                            .ghost()
                            .accessibility_label(name.clone())
                            .child(div().text_lg().text_color(hue).child(name.clone()))
                            .accessibility_id(format!("heading-{id}"))
                            .disabled(self.busy() || !self.read.is_manageable())
                            .on_click(cx.listener(move |t, e: &gpui_kit::ClickEvent, w, cx| {
                                if e.click_count() == 2 || e.is_keyboard() {
                                    t.open_editor(Kind::Group(Some(edit_id.clone())), w, cx);
                                }
                            })),
                    ),
            );
            let count = match g.items().len() {
                0 => "No Items".into(),
                1 => "1 item".into(),
                n => format!("{n} items"),
            };
            header = header.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(count),
            );
            let weak = cx.weak_entity();
            let menu_id = id.clone();
            let manageable = self.read.is_manageable();
            let busy = self.busy();
            let last = ix + 1 == self.read.groups().len();
            let group_ids: Vec<_> = g.items().iter().map(|r| r.id).collect();
            header = header.child(
                Button::new(key(format!("group-menu-{id}")))
                    .small()
                    .ghost()
                    .accessibility_label(format!("Group menu for {name}"))
                    .icon(IconName::Ellipsis)
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
                        let select = weak.clone();
                        let ids = group_ids.clone();
                        menu = menu.item(
                            PopupMenuItem::new("Select all items in group")
                                .disabled(ids.is_empty())
                                .on_click(move |_, w, cx| {
                                    let _ = select
                                        .update(cx, |t, cx| t.select(ids.clone(), true, w, cx));
                                }),
                        );
                        let weak = weak.clone();
                        let id = menu_id.clone();
                        menu.item(
                            PopupMenuItem::new("Archive group")
                                .disabled(!manageable)
                                .on_click(move |_, w, cx| {
                                    let _ = weak.update(cx, |t, cx| {
                                        t.submit(
                                            TableCommand::Group {
                                                id: id.clone(),
                                                board_revision: t.read.revision(),
                                                restore: false,
                                            },
                                            None,
                                            None,
                                            w,
                                            cx,
                                        )
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
                .on_prepaint(move |bounds, _, _| {
                    if let Some(s) = geometry.borrow_mut().section_mut(&gid) {
                        s.set_section(bounds);
                    }
                })
                .child(header);
            if !collapsed {
                let ids: Vec<_> = g.items().iter().map(|r| r.id).collect();
                // the internal tool's default system Item column is 360px at normal zoom.
                // Only this column is returned by this viewer's table read.
                let mut grid = div()
                    .id(key(format!("grid-{id}")))
                    .role(Role::Table)
                    .aria_label(name.clone())
                    .flex()
                    .flex_col()
                    .w_full()
                    .max_w(gpui_kit::rems(26.5))
                    .border_l(gpui_kit::rems(0.375))
                    .border_color(hue)
                    .child(
                        table_band(cx)
                            .id(key(format!("columns-{id}")))
                            .role(Role::Row)
                            .border_r_1()
                            .child(
                                selection_cell()
                                    .id(key(format!("selection-header-{id}")))
                                    .role(Role::ColumnHeader)
                                    .child(
                                        Checkbox::new(key(format!("check-group-{id}")))
                                            .small()
                                            .accessibility_label(format!(
                                                "Select all items in {name}"
                                            ))
                                            .checked(!ids.is_empty() && selected == ids.len())
                                            .indeterminate(selected > 0 && selected < ids.len())
                                            .disabled(self.busy() || ids.is_empty())
                                            .on_change(cx.listener(move |t, checked, w, cx| {
                                                t.select(ids.clone(), *checked, w, cx)
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .id(key(format!("name-header-{id}")))
                                    .role(Role::ColumnHeader)
                                    .flex()
                                    .flex_1()
                                    .items_center()
                                    .justify_center()
                                    .h_9()
                                    .border_l_1()
                                    .border_color(cx.theme().border)
                                    .text_sm()
                                    .child("Item"),
                            ),
                    );
                if let Some(drag) = self.row_drags.get(&id) {
                    let geometry = drag.clone();
                    let scroll = self.scroll.clone();
                    let mut rows = div().flex().flex_col().on_prepaint(move |bounds, w, _| {
                        geometry.borrow_mut().set_viewport(
                            bounds
                                .intersect(&scroll.bounds())
                                .intersect(&w.visual_viewport_bounds()),
                        )
                    });
                    for item in g.items() {
                        let item_id = item.id;
                        let Some(name_row) = self.rows.get(&item_id) else {
                            continue;
                        };
                        let title = name_row.read(cx).accepted_title();
                        let checked = self.selection.contains(&item_id);
                        let hover = if checked {
                            cx.theme().list_active
                        } else {
                            cx.theme().list_hover
                        };
                        let geometry = drag.clone();
                        let rid = item_id.to_string();
                        let mut row = table_band(cx)
                            .id(key(format!("row-{item_id}")))
                            .role(Role::Row)
                            .relative()
                            .border_r_1()
                            .hover(move |s| s.bg(hover))
                            .bg(if checked {
                                cx.theme().list_active
                            } else {
                                cx.theme().background
                            })
                            .on_prepaint(move |bounds, _, _| {
                                if let Some(s) = geometry.borrow_mut().section_mut(&rid) {
                                    s.set_heading(bounds);
                                    s.set_section(bounds);
                                }
                            });
                        let geometry = drag.clone();
                        let rid = item_id.to_string();
                        let grip = div()
                            .on_prepaint(move |bounds, _, _| {
                                if let Some(s) = geometry.borrow_mut().section_mut(&rid) {
                                    s.set_name(bounds);
                                }
                            })
                            .child(
                                Button::new(key(format!("grip-{item_id}")))
                                    .small()
                                    .ghost()
                                    .icon(IconName::GripVertical)
                                    .accessibility_label(format!("Drag {title}"))
                                    .accessibility_id(format!("grip-{item_id}"))
                                    .disabled(
                                        self.busy()
                                            || !self.read.is_editable()
                                            || (checked && self.selection.len() > 1),
                                    ),
                            );
                        row = row.child(
                            selection_cell()
                                .id(key(format!("selection-cell-{item_id}")))
                                .role(Role::Cell)
                                .child(grip)
                                .child(
                                    Checkbox::new(key(format!("check-{item_id}")))
                                        .small()
                                        .accessibility_label(format!("Select {title}"))
                                        .checked(checked)
                                        .disabled(self.busy())
                                        .on_change(cx.listener(move |t, checked, w, cx| {
                                            t.select_row(
                                                item_id,
                                                *checked,
                                                w.modifiers().shift,
                                                w,
                                                cx,
                                            )
                                        })),
                                ),
                        );
                        let weak = cx.weak_entity();
                        let editable = self.read.is_editable();
                        let manageable = self.read.is_manageable();
                        row = row.child(
                            div()
                                .id(key(format!("name-cell-{item_id}")))
                                .role(Role::Cell)
                                .flex()
                                .items_center()
                                .flex_1()
                                .min_w_0()
                                .pr_1()
                                .child(name_row.clone())
                                .child(
                                    Button::new(key(format!("item-menu-{item_id}")))
                                        .icon(IconName::Ellipsis)
                                        .small()
                                        .ghost()
                                        .accessibility_label(format!("More options for {title}"))
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
                                                            t.open_editor(
                                                                Kind::Move(item_id),
                                                                w,
                                                                cx,
                                                            )
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Archive item…")
                                                    .disabled(!manageable)
                                                    .on_click(move |_, w, cx| {
                                                        let _ = archive.update(cx, |t, cx| {
                                                            t.open_editor(
                                                                Kind::ArchiveRoot(item_id),
                                                                w,
                                                                cx,
                                                            )
                                                        });
                                                    }),
                                            )
                                        }),
                                ),
                        );
                        if let Some((target, after)) = drag.borrow().edge()
                            && target == item_id.to_string()
                        {
                            let line = div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .h(gpui_kit::px(1.))
                                .bg(cx.theme().info);
                            row = row.child(if after { line.bottom_0() } else { line.top_0() });
                        }
                        rows = rows.child(row);
                    }
                    grid = grid.child(rows);
                }
                if let Some(entry) = self.entries.get(&id) {
                    let group = id.clone();
                    let cancel_id = id.clone();
                    grid = grid.child(
                        table_band(cx)
                            .id(key(format!("add-row-{id}")))
                            .role(Role::Row)
                            .border_b_1()
                            .border_r_1()
                            .capture_key_down(cx.listener(
                                move |t, event: &gpui_kit::KeyDownEvent, w, cx| {
                                    if event.keystroke.key == "escape"
                                        && !t.busy()
                                        && let Some(entry) = t.entries.get_mut(&cancel_id)
                                    {
                                        entry.input.update(cx, |i, cx| i.set_value("", w, cx));
                                        entry.notice = "";
                                        entry.proposed = Uuid::new_v4();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(
                                selection_cell()
                                    .h_9()
                                    .border_r_1()
                                    .border_color(cx.theme().border),
                            )
                            .child(
                                div()
                                    .id(key(format!("add-cell-{id}")))
                                    .role(Role::Cell)
                                    .flex()
                                    .items_center()
                                    .flex_1()
                                    .min_w_0()
                                    .px_1()
                                    .child(
                                        Input::new(&entry.input)
                                            .small()
                                            .bordered(false)
                                            .flex_1()
                                            .min_w_0()
                                            .aria_label(format!("Add item to {name}"))
                                            .disabled(self.busy() || !self.read.is_editable()),
                                    )
                                    .child(
                                        Button::new(key(format!("create-{id}")))
                                            .small()
                                            .ghost()
                                            .label("Add item")
                                            .accessibility_id(id.clone())
                                            .disabled(self.busy() || !self.read.is_editable())
                                            .on_click(cx.listener(move |t, _, w, cx| {
                                                t.create(&group, w, cx)
                                            })),
                                    ),
                            ),
                    );
                    if !entry.notice.is_empty() {
                        grid = grid.child(status(entry.notice));
                    }
                }
                section = section.child(grid);
            }
            groups = groups.child(section);
        }
        let drag = self.drag.borrow();
        if let Some(mark) = drag.marker() {
            let offset = mark.origin - drag.surface_origin();
            groups = groups.child(
                div()
                    .absolute()
                    .top(offset.y)
                    .left(offset.x)
                    .w(mark.size.width)
                    .h(mark.size.height)
                    .bg(cx.theme().info),
            );
        }
        if let Some(id) = drag.moving_id()
            && let Some(group) = self.read.groups().iter().find(|g| g.id() == id)
            && let Some(pointer) = drag.pointer()
        {
            let offset = pointer - drag.surface_origin();
            groups = groups.child(
                div()
                    .absolute()
                    .top(offset.y)
                    .left(offset.x)
                    .px_2()
                    .bg(cx.theme().popover)
                    .child(format!("Moving {}", group.name())),
            );
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
                                    if t.selection.len() > 1
                                        && drag
                                            .borrow()
                                            .source_id()
                                            .and_then(|id| id.parse::<Uuid>().ok())
                                            .is_some_and(|id| t.selection.contains(&id))
                                    {
                                        drag.borrow_mut().cancel();
                                    }
                                }
                            }
                        }
                        if t.drag.borrow().is_held()
                            || t.row_drags.values().any(|d| d.borrow().is_held())
                        {
                            t.focus.focus(w, cx);
                            if t.drag.borrow().is_held() {
                                t.scroll_held_heading(w, cx);
                            }
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
    fn scroll_held_heading(&self, w: &mut Window, cx: &mut Context<Self>) {
        let generation = self.drag.borrow().generation();
        let start = cx.background_executor().now();
        let table = cx.weak_entity();
        w.on_next_frame(move |w, cx| {
            let _ = table.update(cx, |t, cx| {
                if !t.drag.borrow().is_held() || generation != t.drag.borrow().generation() {
                    return;
                }
                if !w.is_window_active() || t.busy() {
                    t.drag.borrow_mut().cancel();
                    cx.notify();
                    return;
                }
                let velocity = t.drag.borrow().scroll_velocity();
                if velocity != 0.0 {
                    let seconds = cx
                        .background_executor()
                        .now()
                        .duration_since(start)
                        .as_secs_f32()
                        .min(0.05);
                    let offset = t.scroll.offset();
                    let y = (offset.y - gpui_kit::px(velocity * seconds))
                        .clamp(-t.scroll.max_offset().y, gpui_kit::px(0.));
                    t.scroll.set_offset(gpui_kit::point(offset.x, y));
                    cx.notify();
                }
                t.scroll_held_heading(w, cx);
            });
        });
    }
}
