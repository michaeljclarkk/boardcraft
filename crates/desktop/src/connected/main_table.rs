//! The visit owns local folds and focus. Only a confirmed authorized read replaces saved state.
use super::navigation::Page;
use super::*;
use boardcraft_mobile_edit_client::MainTableRead;
use gpui_kit::base::ElementExt;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::ButtonVariants,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
};
mod archive;
mod group_fields;
mod group_order;
mod item_entry;
mod item_move;
mod item_order;
mod selection;
mod selection_move;
use archive::{Archive, RootCommand, Undo};
use group_fields::GroupDialog;
use item_entry::GroupEntry;
use selection::Selection;
use selection_move::MoveDialog;

struct Heading {
    id: SharedString,
    name: SharedString,
    colour: SharedString,
    revision: i64,
    name_focus: FocusHandle,
    count: SharedString,
    rows: Vec<Entity<Row>>,
    entry: Entity<GroupEntry>,
    collapsed: bool,
    focus: FocusHandle,
    row_drag: std::rc::Rc<std::cell::RefCell<boardcraft_ui::HeadingDrag>>,
}

pub(super) struct MainTable {
    headings: Vec<Heading>,
    revision: i64,
    board_revision: i64,
    editable: bool,
    manageable: bool,
    pending: bool,
    folding: bool,
    reconcile: bool,
    selection: Selection,
    selection_cache: bool,
    move_dialog: Option<Entity<MoveDialog>>,
    move_focus: FocusHandle,
    selection_archive_focus: FocusHandle,
    _move_subscription: Option<Subscription>,
    item_move: Option<Entity<item_move::ItemMoveDialog>>,
    _item_move_subscription: Option<Subscription>,
    failed_moves: Vec<item_move::FailedMove>,
    group_dialog: Option<Entity<GroupDialog>>,
    group_focus: FocusHandle,
    _group_subscription: Option<Subscription>,
    archive: Option<Archive>,
    archive_focus: FocusHandle,
    archive_triggers: std::collections::HashMap<Uuid, FocusHandle>,
    root_command: Option<RootCommand>,
    undo: Option<Undo>,
    drag: std::rc::Rc<std::cell::RefCell<boardcraft_ui::HeadingDrag>>,
    _subscriptions: Vec<Subscription>,
}

enum FoldFailure {
    Save(EditError),
    Read(EditError),
}

impl MainTable {
    pub(super) fn is_pending(&self) -> bool {
        self.pending
    }
    pub(super) fn focus_first(&self, w: &mut Window, cx: &mut App) {
        if let Some(h) = self.headings.first() {
            h.focus.focus(w, cx);
        }
    }
    pub(super) fn new(
        read: MainTableRead,
        board: Uuid,
        previous: Option<&Self>,
        w: &mut Window,
        cx: &mut Context<Connected>,
    ) -> Self {
        let editable = read.is_editable();
        let mut subscriptions = Vec::new();
        let headings: Vec<Heading> =
            read.groups()
                .iter()
                .map(|g| {
                    let old =
                        previous.and_then(|t| t.headings.iter().find(|h| h.id.as_ref() == g.id()));
                    let entry = old
                        .map(|h| h.entry.clone())
                        .unwrap_or_else(|| cx.new(|cx| GroupEntry::new(g.id(), g.name(), w, cx)));
                    entry.update(cx, |e, cx| {
                        e.enabled = editable;
                        e.set_group_name(g.name());
                        cx.notify();
                    });
                    subscriptions.push(cx.subscribe_in(
                        &entry,
                        w,
                        |v, entry, _: &item_entry::Create, w, cx| {
                            v.create_main_table_item(entry.clone(), w, cx)
                        },
                    ));
                    let rows =
                        g.items()
                            .iter()
                            .map(|r| {
                                let prior = previous.and_then(|t| {
                                    t.headings.iter().flat_map(|h| &h.rows).find(|row| {
                                        row.read(cx).edit.identity().1.as_uuid() == r.id
                                    })
                                });
                                let row = if let Some(prior) = prior {
                                    prior.update(cx, |row, cx| {
                                        row.editable = editable;
                                        row.edit = boardcraft_desktop::InlineEdit::new(
                                            BoardId::new(board).expect("selected board"),
                                            StoredItem {
                                                id: ItemId::new(r.id).expect("validated read"),
                                                title: r.title.clone(),
                                                revision: r.revision,
                                            },
                                        );
                                        cx.notify();
                                    });
                                    prior.clone()
                                } else {
                                    cx.new(|cx| {
                                        Row::new(
                                            editable,
                                            BoardId::new(board).expect("selected board"),
                                            StoredItem {
                                                id: ItemId::new(r.id).expect("validated read"),
                                                title: r.title.clone(),
                                                revision: r.revision,
                                            },
                                            RowRecovery::default(),
                                            w,
                                            cx,
                                        )
                                    })
                                };
                                subscriptions.push(cx.subscribe_in(
                                    &row,
                                    w,
                                    |v, row, save: &Save, w, cx| {
                                        v.rename_main_table_item(row.clone(), save.clone(), w, cx)
                                    },
                                ));
                                row
                            })
                            .collect();
                    Heading {
                        row_drag: std::rc::Rc::new(std::cell::RefCell::new(
                            boardcraft_ui::HeadingDrag::new(
                                g.items().iter().map(|r| r.id.to_string().into()),
                            ),
                        )),
                        id: g.id().to_owned().into(),
                        name: g.name().to_owned().into(),
                        colour: g.colour().to_owned().into(),
                        revision: g.revision(),
                        name_focus: old
                            .map(|h| h.name_focus.clone())
                            .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
                        count: format!(
                            "{} {}",
                            g.items().len(),
                            if g.items().len() == 1 {
                                "item"
                            } else {
                                "items"
                            }
                        )
                        .into(),
                        rows,
                        entry,
                        collapsed: g.is_collapsed(),
                        focus: old
                            .map(|h| h.focus.clone())
                            .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
                    }
                })
                .collect();
        let mut selection = previous.map(|t| t.selection.clone()).unwrap_or_default();
        let available: Vec<_> = read
            .groups()
            .iter()
            .flat_map(|g| g.items().iter().map(|r| r.id))
            .collect();
        selection.retain(&available);
        let archive_triggers = available
            .iter()
            .map(|id| {
                (
                    *id,
                    previous
                        .and_then(|t| t.archive_triggers.get(id))
                        .cloned()
                        .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
                )
            })
            .collect();
        Self {
            drag: std::rc::Rc::new(std::cell::RefCell::new(boardcraft_ui::HeadingDrag::new(
                headings.iter().map(|h| h.id.clone()),
            ))),
            revision: read.view_revision(),
            board_revision: read.revision(),
            editable,
            manageable: read.is_manageable(),
            pending: false,
            folding: false,
            reconcile: false,
            selection,
            selection_cache: false,
            move_dialog: None,
            move_focus: previous
                .map(|t| t.move_focus.clone())
                .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
            _move_subscription: None,
            selection_archive_focus: previous
                .map(|t| t.selection_archive_focus.clone())
                .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
            item_move: None,
            _item_move_subscription: None,
            failed_moves: previous.map(|t| t.failed_moves.clone()).unwrap_or_default(),
            group_dialog: None,
            group_focus: previous
                .map(|t| t.group_focus.clone())
                .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
            _group_subscription: None,
            archive: None,
            archive_focus: previous
                .map(|t| t.archive_focus.clone())
                .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
            root_command: None,
            archive_triggers,
            undo: None,
            headings,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn render(&self, cx: &mut Context<Connected>) -> AnyElement {
        if let Some(archive) = &self.archive {
            self.render_archive(archive, cx).into_any_element()
        } else {
            self.render_active(cx).into_any_element()
        }
    }

    fn render_active(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let mut column = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Main table"))
            .child(Label::new(if self.editable && !self.reconcile {
                "Board items"
            } else {
                "Board items · Read only"
            }));
        column = column.child(self.new_group_options(cx));
        column = column.child(self.failed_item_moves(cx));
        column = column.child(
            Button::new("open-archive")
                .small()
                .label("Archive…")
                .track_focus(&self.archive_focus)
                .disabled(self.pending || self.root_command.is_some())
                .on_click(cx.listener(|v, _, w, cx| v.read_archive(None, w, cx))),
        );
        if let Some(undo) = &self.undo {
            column = column
                .child(Label::new(undo.label.clone()))
                .child(
                    Button::new("archive-undo")
                        .small()
                        .label("Undo")
                        .disabled(self.pending || self.reconcile || !self.manageable)
                        .on_click(cx.listener(|v, _, w, cx| v.undo_group_archive(w, cx))),
                )
                .child(
                    Button::new("archive-dismiss")
                        .small()
                        .label("Dismiss")
                        .disabled(self.pending)
                        .on_click(cx.listener(|v, _, _, cx| {
                            if let Screen::Navigation(nav) = &mut v.screen
                                && let Page::Items(_, t) = &mut nav.page
                            {
                                t.undo = None;
                                cx.notify();
                            }
                        })),
                );
        }
        if !self.editable {
            column = column.child(Label::new(
                "Group presentation changes last only for this visit.",
            ));
        }
        if self.pending && self.folding {
            column = column.child(Label::new("Saving group presentation…"));
        }
        if !self.selection.ids.is_empty() {
            column = column.child(self.selection_toolbar(cx));
        }
        if self.headings.is_empty() {
            column = column.child(Label::new("Empty board"));
        }
        let drag = self.drag.clone();
        let mut groups = div()
            .id("physical-groups")
            .relative()
            .flex()
            .flex_col()
            .gap_3()
            .on_prepaint(move |bounds, w, _| {
                drag.borrow_mut()
                    .set_viewport(bounds.intersect(&w.visual_viewport_bounds()));
            })
            .capture_key_down(cx.listener(|v, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape"
                    && let Screen::Navigation(nav) = &mut v.screen
                    && let Page::Items(_, table) = &mut nav.page
                    && table.drag.borrow().is_held()
                {
                    table.drag.borrow_mut().cancel();
                    cx.stop_propagation();
                    cx.notify();
                }
                if event.keystroke.key == "escape"
                    && let Screen::Navigation(nav) = &mut v.screen
                    && let Page::Items(_, table) = &mut nav.page
                {
                    for h in &table.headings {
                        if h.row_drag.borrow().is_held() {
                            h.row_drag.borrow_mut().cancel();
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }
                }
            }));
        for h in &self.headings {
            groups = groups.child(self.render_heading(h, cx));
        }
        if let Some(marker) = self.drag.borrow().marker() {
            let offset = marker.origin - self.drag.borrow().surface_origin();
            // Resolved window geometry translated to this positioned surface, not layout constants.
            groups = groups.child(
                div()
                    .id("group-order-marker")
                    .test_support()
                    .absolute()
                    .left(offset.x)
                    .top(offset.y)
                    .w(marker.size.width)
                    .h(marker.size.height)
                    .bg(cx.theme().primary),
            );
        }
        if let Some(id) = self.drag.borrow().moving_id()
            && let Some(h) = self.headings.iter().find(|h| h.id.as_ref() == id)
        {
            groups = groups.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bg(cx.theme().popover)
                    .px_2()
                    .child(Label::new(format!("Moving {}", h.name))),
            );
        }
        column = column.child(
            groups
                .child(self.group_drag_events(cx))
                .child(self.item_drag_events(cx)),
        );
        column
    }

    fn render_heading_header(&self, h: &Heading, cx: &mut Context<Connected>) -> impl IntoElement {
        let id = h.id.clone();
        let menu_id = id.clone();
        let expanded = !h.collapsed;
        let group_empty = h.rows.is_empty();
        let view = cx.weak_entity();
        let manageable = self.manageable;
        let up = self.headings.first().is_some_and(|first| first.id != h.id);
        let down = self.headings.last().is_some_and(|last| last.id != h.id);
        let geometry = self.drag.clone();
        let geometry_id = h.id.clone();
        let mut header = div()
            .flex()
            .items_center()
            .gap_2()
            .on_prepaint(move |bounds, _, _| {
                if let Some(s) = geometry.borrow_mut().section_mut(&geometry_id) {
                    s.set_heading(bounds);
                }
            });
        header = header.child(
            Button::new(SharedString::from(format!("fold-{id}")))
                .small()
                .ghost()
                .track_focus(&h.focus)
                .icon(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .accessibility_label(format!("Toggle {}", h.name))
                .aria_expanded(expanded)
                .loading(self.pending)
                .disabled(self.reconcile)
                .on_click(
                    cx.listener(move |v, _, w, cx| v.fold_main_table(id.as_ref(), false, w, cx)),
                ),
        );
        header = header.child(self.group_name(h, cx));
        // This colour is saved user data, not application chrome.
        header = header.child(div().size_3().bg(rgb(
            u32::from_str_radix(&h.colour[1..], 16).expect("validated colour"),
        )));
        header = header.child(Label::new(h.count.clone()));
        header.child(
            Button::new(SharedString::from(format!("group-menu-{menu_id}")))
                .small()
                .ghost()
                .label("Group options")
                .disabled(self.pending || self.reconcile)
                .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu;
                    for (label, delta, enabled) in
                        [("Move group up", -1, up), ("Move group down", 1, down)]
                    {
                        let view = view.clone();
                        let id = menu_id.clone();
                        menu = menu.item(
                            PopupMenuItem::new(label)
                                .disabled(!manageable || !enabled)
                                .on_click(move |_, w, cx| {
                                    _ = view.update(cx, |v, cx| {
                                        v.move_main_table_group(&id, delta, w, cx)
                                    });
                                }),
                        );
                    }
                    for label in ["Rename group…", "Change group colour…"] {
                        let view = view.clone();
                        let id = menu_id.clone();
                        menu = menu.item(PopupMenuItem::new(label).disabled(!manageable).on_click(
                            move |_, w, cx| {
                                _ = view.update(cx, |v, cx| v.open_group_fields(Some(&id), w, cx));
                            },
                        ));
                    }
                    for all in [false, true] {
                        let view = view.clone();
                        let id = menu_id.clone();
                        let label = match (expanded, all) {
                            (true, false) => "Collapse group",
                            (false, false) => "Expand group",
                            (true, true) => "Collapse all groups",
                            (false, true) => "Expand all groups",
                        };
                        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                            _ = view.update(cx, |v, cx| v.fold_main_table(id.as_ref(), all, w, cx));
                        }));
                    }
                    let select_view = view.clone();
                    let id = menu_id.clone();
                    menu = menu.item(
                        PopupMenuItem::new("Select all items in group")
                            .disabled(group_empty)
                            .on_click(move |_, w, cx| {
                                _ = select_view.update(cx, |v, cx| {
                                    v.select_main_table_group(&id, true, w, cx)
                                });
                            }),
                    );
                    let view = view.clone();
                    let id = menu_id.clone();
                    menu = menu.item(
                        PopupMenuItem::new("Archive group")
                            .disabled(!manageable)
                            .on_click(move |_, w, cx| {
                                _ = view
                                    .update(cx, |v, cx| v.change_archive_group(&id, false, w, cx));
                            }),
                    );
                    menu
                }),
        )
    }

    fn render_heading(&self, h: &Heading, cx: &mut Context<Connected>) -> impl IntoElement {
        let expanded = !h.collapsed;
        let geometry = self.drag.clone();
        let id = h.id.clone();
        let mut section = div()
            .id(SharedString::from(format!("group-{}", h.id)))
            .flex()
            .flex_col()
            .gap_2()
            .on_prepaint(move |bounds, _, _| {
                if let Some(s) = geometry.borrow_mut().section_mut(&id) {
                    s.set_section(bounds);
                }
            })
            .child(self.render_heading_header(h, cx));
        if expanded {
            section = section.child(self.render_heading_body(h, cx));
        }
        section
    }

    fn render_heading_body(&self, h: &Heading, cx: &mut Context<Connected>) -> impl IntoElement {
        let geometry = h.row_drag.clone();
        let mut body = div()
            .relative()
            .flex()
            .flex_col()
            .gap_1()
            .pl_8()
            .on_prepaint(move |bounds, w, _| {
                geometry
                    .borrow_mut()
                    .set_viewport(bounds.intersect(&w.visual_viewport_bounds()));
            });
        let group = h.id.clone();
        let selected = h
            .rows
            .iter()
            .filter(|r| {
                self.selection
                    .ids
                    .contains(&r.read(cx).edit.identity().1.as_uuid())
            })
            .count();
        body = body.child(
            Checkbox::new(SharedString::from(format!("select-group-{group}")))
                .small()
                .accessibility_label(format!("Select all items in {}", h.name))
                .checked(!h.rows.is_empty() && selected == h.rows.len())
                .indeterminate(selected > 0 && selected < h.rows.len())
                .disabled(h.rows.is_empty() || self.pending || self.reconcile)
                .on_change(cx.listener(move |v, checked, w, cx| {
                    v.select_main_table_group(&group, *checked, w, cx)
                })),
        );
        if h.rows.is_empty() {
            body = body.child(Label::new("No items"));
        }
        for r in &h.rows {
            let id = r.read(cx).edit.identity().1.as_uuid();
            let checked = self.selection.ids.contains(&id);
            let name = r.read(cx).edit.accepted().to_owned();
            let geometry = h.row_drag.clone();
            let key = id.to_string();
            body = body.child(
                div()
                    .id(SharedString::from(format!("selected-row-{id}")))
                    .test_support()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_2()
                    .on_prepaint(move |bounds, _, _| {
                        if let Some(row) = geometry.borrow_mut().section_mut(&key) {
                            row.set_heading(bounds);
                            row.set_section(bounds);
                        }
                    })
                    .bg(if checked {
                        cx.theme().accent
                    } else {
                        cx.theme().background
                    })
                    .child(
                        Checkbox::new(SharedString::from(format!("select-{id}")))
                            .small()
                            .accessibility_label(format!("Select {name}"))
                            .checked(checked)
                            .disabled(self.pending || self.reconcile)
                            .on_change(cx.listener(move |v, checked, w, cx| {
                                v.select_main_table_row(id, *checked, w.modifiers().shift, w, cx)
                            })),
                    )
                    .child(self.item_grip(h, id, &name))
                    .child(r.clone())
                    .child(self.root_archive_menu(id, cx))
                    .child(self.item_drop_marker(h, id, cx)),
            );
            if matches!(
                r.read(cx).edit.phase(),
                boardcraft_desktop::Phase::Failed(_)
            ) {
                let row = r.clone();
                body = body
                    .child(Label::new(
                        "Name not saved. Correct the draft or reload the board.",
                    ))
                    .child(
                        Button::new(SharedString::from(format!(
                            "correct-{}",
                            r.read(cx).edit.identity().1.as_uuid()
                        )))
                        .small()
                        .label("Correct name")
                        .disabled(self.pending || self.reconcile || !self.editable)
                        .on_click(move |_, w, cx| {
                            row.update(cx, |r, cx| {
                                let raw = r.edit.request().map(str::to_owned);
                                r.open(w, cx);
                                if r.baseline.is_some()
                                    && let Some(raw) = raw
                                {
                                    r.input.update(cx, |input, cx| input.set_value(raw, w, cx));
                                }
                            })
                        }),
                    );
            }
        }
        if self.editable && !self.reconcile {
            body = body.child(h.entry.clone());
        }
        body
    }
}

impl Connected {
    fn fold_main_table(&mut self, id: &str, all: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.pending || table.reconcile {
            return;
        }
        let Some(heading) = table.headings.iter().find(|h| h.id.as_ref() == id) else {
            return;
        };
        let collapsed = !heading.collapsed;
        let focus = heading.focus.clone();
        focus.focus(w, cx);
        if !table.editable {
            for h in &mut table.headings {
                if all || h.id.as_ref() == id {
                    h.collapsed = collapsed;
                }
            }
            cx.notify();
            return;
        }
        let ids: Vec<String> = table
            .headings
            .iter()
            .filter(|h| all || h.id.as_ref() == id)
            .map(|h| h.id.to_string())
            .collect();
        let board = *board;
        let revision = table.revision;
        let client = nav.client.clone();
        table.set_busy(true, cx);
        table.folding = true;
        let request = self.navigation_request;
        self.message = "";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let work = cx.background_executor().spawn(async move {
            client
                .set_main_table_fold(board, revision, &ids, collapsed)
                .map_err(FoldFailure::Save)?;
            client.read_main_table(board).map_err(FoldFailure::Read)
        });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if view.navigation_request != request { return; }
                let Screen::Navigation(nav) = &mut view.screen else { return; };
                let Page::Items(current, old) = &nav.page else { return; };
                if *current != board || !old.pending { return; }
                match result {
                    Ok(read) => {
                        let table = MainTable::new(read, board, Some(old), w, cx);
                        // The original disclosure stays live across body replacement.
                        let target = table.headings.iter().find(|h| h.focus == focus).or_else(|| table.headings.first());
                        if let Some(h) = target { h.focus.focus(w, cx); }
                        nav.page = Page::Items(board, Box::new(table));
                    }
                    Err(FoldFailure::Save(EditError::SessionRejected) | FoldFailure::Read(EditError::SessionRejected)) => {
                        view.link_input = None;
                        view.screen = Self::identifier(w, cx);
                        view.message = "Session expired. Sign in again.";
                    }
                    Err(failure) => {
                        let recovery_focus = cx.focus_handle().tab_stop(true);
                        recovery_focus.focus(w, cx);
                        nav.page = Page::Recover(board, recovery_focus);
                        nav.share = None;
                        view.message = match failure {
                            FoldFailure::Save(EditError::Conflict) => "Group presentation changed. Reload board, then choose again.",
                            FoldFailure::Save(EditError::PermissionDenied) => "Group presentation not saved: permission denied. Reload board.",
                            _ => "Could not confirm group presentation. Reload board before choosing again.",
                        };
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
