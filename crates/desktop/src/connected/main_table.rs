//! The visit owns local folds and focus. Only a confirmed authorized read replaces saved state.
use super::navigation::Page;
use super::*;
use boardcraft_mobile_edit_client::{Automation, ChangeEvent, MainTableRead, SavedItem, SavedView};
use boardcraft_ui::{BoardRow, BoardTabs, BoardToolbar};
use gpui_kit::base::{ElementExt, Selectable};
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::ButtonVariants,
    checkbox::Checkbox,
    input::InputEvent,
    menu::{DropdownMenu, PopupMenuItem},
};
mod activity;
mod archive;
mod group_fields;
mod group_order;
mod item_entry;
mod item_move;
mod item_order;
mod selection;
mod selection_move;
mod status;
mod views;
#[cfg(all(test, feature = "native-smoke"))]
mod tests;
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
    status: status::StatusColumn,
    headings: Vec<Heading>,
    revision: i64,
    board_revision: i64,
    pub(super) views: Vec<SavedView>,
    pub(super) automations: Vec<Automation>,
    pub(super) filter: String,
    filters: boardcraft_mobile_edit_client::ViewFilters,
    filters_open: bool,
    filter_focus: FocusHandle,
    view_save_id: Option<Uuid>,
    view_save_unconfirmed: bool,
    view_filter: Entity<InputState>,
    search_task: Option<Task<()>>,
    search_invalid: bool,
    view_name: Entity<InputState>,
    automation_name: Entity<InputState>,
    views_open: bool,
    automations_open: bool,
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
    selection_delete_focus: FocusHandle,
    _move_subscription: Option<Subscription>,
    item_move: Option<Entity<item_move::ItemMoveDialog>>,
    _item_move_subscription: Option<Subscription>,
    failed_moves: Vec<item_move::FailedMove>,
    group_dialog: Option<Entity<GroupDialog>>,
    group_focus: FocusHandle,
    _group_subscription: Option<Subscription>,
    archive: Option<Archive>,
    recovery_trash: bool,
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
    pub(super) fn focus_item(&self, id: Uuid, w: &mut Window, cx: &mut App) {
        for row in self.headings.iter().flat_map(|h| &h.rows) {
            let (row_item, focus) = {
                let row = row.read(cx);
                (row.edit.identity().1.as_uuid(), row.display_focus.clone())
            };
            if row_item == id {
                focus.focus(w, cx);
                return;
            }
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
                    subscriptions.push(cx.observe(&entry, |_, _, cx| cx.notify()));
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
                                subscriptions.push(cx.observe(&row, |_, _, cx| cx.notify()));
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
        let view_filter = previous
            .map(|t| t.view_filter.clone())
            .unwrap_or_else(|| cx.new(|cx| InputState::new(w, cx).placeholder("Search")));
        subscriptions.push(
            cx.subscribe_in(&view_filter, w, |v, _, event: &InputEvent, w, cx| {
                if matches!(event, InputEvent::Change) {
                    v.queue_view_filter(w, cx);
                }
            }),
        );
        if previous.is_some_and(|table| view_filter.read(cx).value().trim() != table.filter) {
            // Reload replaces the table while keeping its input. Restart an unfinished search
            // against the replacement visit rather than losing the user's last keystrokes.
            cx.defer_in(w, |view, w, cx| view.queue_view_filter(w, cx));
        }
        let view_name = previous
            .map(|t| t.view_name.clone())
            .unwrap_or_else(|| cx.new(|cx| InputState::new(w, cx).placeholder("View name")));
        let automation_name = previous
            .map(|t| t.automation_name.clone())
            .unwrap_or_else(|| cx.new(|cx| InputState::new(w, cx).placeholder("Automation name")));
        subscriptions.push(
            cx.subscribe_in(&view_name, w, |v, _, event: &InputEvent, w, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    v.save_view(w, cx);
                }
            }),
        );
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
            status: status::StatusColumn::load(board, w, cx),
            drag: std::rc::Rc::new(std::cell::RefCell::new(boardcraft_ui::HeadingDrag::new(
                headings.iter().map(|h| h.id.clone()),
            ))),
            revision: read.view_revision(),
            board_revision: read.revision(),
            views: previous.map(|t| t.views.clone()).unwrap_or_default(),
            automations: previous.map(|t| t.automations.clone()).unwrap_or_default(),
            filter: previous.map(|t| t.filter.clone()).unwrap_or_default(),
            filters: previous.map(|t| t.filters.clone()).unwrap_or_default(),
            filters_open: false,
            filter_focus: previous
                .map(|t| t.filter_focus.clone())
                .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
            view_save_id: previous.and_then(|t| t.view_save_id),
            view_save_unconfirmed: previous.is_some_and(|t| t.view_save_unconfirmed),
            view_filter,
            search_task: None,
            search_invalid: previous.is_some_and(|t| t.search_invalid),
            view_name,
            automation_name,
            views_open: previous.is_some_and(|t| t.views_open),
            automations_open: previous.is_some_and(|t| t.automations_open),
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
            selection_delete_focus: previous
                .map(|t| t.selection_delete_focus.clone())
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
            recovery_trash: previous.is_some_and(|t| t.recovery_trash),
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
            .child(BoardTabs::new())
            .child(self.board_toolbar(cx))
            .child(
                Label::new(if self.editable && !self.reconcile {
                    "Board items"
                } else {
                    "Board items · Read only"
                })
                .text_sm(),
            );
        column = column.child(self.failed_item_moves(cx));
        if self.search_invalid {
            column = column.child(Label::new("Search must be 240 characters or fewer."));
        }
        if self.has_filters() {
            column = column.child(Label::new(format!(
                "{} matching items",
                self.order(cx).len()
            )));
        }
        if self.views_open {
            column = column.child(self.render_views(cx));
        }
        if self.automations_open {
            column = column.child(self.render_automations(cx));
        }
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
        if self.headings.is_empty() {
            column = column.child(Label::new("Empty board"));
        }
        let drag = self.drag.clone();
        let mut groups = div()
            .id("physical-groups")
            .min_w_0()
            .overflow_x_scroll()
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
        if self.has_filters() && self.order(cx).is_empty() {
            groups = groups
                .child(Label::new("No results were found"))
                .child(Label::new(
                    "Try another search or review this view’s filters.",
                ))
                .child(
                    Button::new("clear-board-search")
                        .small()
                        .label(if self.editable {
                            "Clear filters"
                        } else {
                            "Clear search"
                        })
                        .disabled(!self.editable && self.filter.is_empty())
                        .on_click(cx.listener(|v, _, w, cx| v.clear_all_view_filters(w, cx))),
                );
        }
        for h in &self.headings {
            if self.has_filters() && self.matching_count(h, cx) == 0 {
                continue;
            }
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

    fn board_toolbar(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let owner = cx.weak_entity();
        let archive = cx.weak_entity();
        let trash = cx.weak_entity();
        let manageable = self.manageable && !self.reconcile;
        BoardToolbar::new()
            .new_item(
                Button::new("new-item")
                    .disabled(!self.editable || self.pending || self.reconcile)
                    .on_click(cx.listener(|view, _, w, cx| {
                        if let Screen::Navigation(nav) = &view.screen
                            && let Page::Items(_, table) = &nav.page
                            && let Some(heading) = table.headings.iter().find(|h| !h.collapsed)
                        {
                            let entry = heading.entry.clone();
                            if table.has_filters() && table.matching_count(heading, cx) == 0 {
                                view.clear_all_view_filters(w, cx);
                            }
                            entry.update(cx, |entry, cx| entry.focus_input(w, cx));
                        }
                    })),
            )
            .new_item_options(self.new_group_options(cx))
            .search(&self.view_filter, "view-filter")
            .search_disabled(self.pending || self.reconcile)
            .action(
                Button::new("quick-filters")
                    .label("Filter")
                    .selected(self.filters != Default::default())
                    .track_focus(&self.filter_focus)
                    .disabled(self.view_actions_blocked(cx) || !self.status.filters_ready())
                    .on_click(cx.listener(|view, _, w, cx| view.open_quick_filters(w, cx))),
            )
            .action(
                Button::new("views-toggle")
                    .label("Views")
                    .selected(self.views_open)
                    .on_click(cx.listener(|view, _, _, cx| {
                        if let Screen::Navigation(nav) = &mut view.screen
                            && let Page::Items(_, table) = &mut nav.page
                        {
                            table.views_open = !table.views_open;
                            cx.notify();
                        }
                    })),
            )
            .action(
                Button::new("automations-toggle")
                    .label("Automate")
                    .selected(self.automations_open)
                    .on_click(cx.listener(|view, _, _, cx| {
                        if let Screen::Navigation(nav) = &mut view.screen
                            && let Page::Items(_, table) = &mut nav.page
                        {
                            table.automations_open = !table.automations_open;
                            cx.notify();
                        }
                    })),
            )
            .tools(
                BoardToolbar::tools_trigger("board-tools-menu")
                    .track_focus(&self.archive_focus)
                    .disabled(self.pending || self.root_command.is_some())
                    .dropdown_menu(move |menu, _, _| {
                        let clear = owner.clone();
                        let add_group = owner.clone();
                        let automations = owner.clone();
                        let reload = owner.clone();
                        let archive = archive.clone();
                        let trash = trash.clone();
                        menu.item(
                            PopupMenuItem::new("Add group")
                                .disabled(!manageable)
                                .on_click(move |_, w, cx| {
                                    let _ = add_group.update(cx, |view, cx| {
                                        let trigger = if let Screen::Navigation(nav) = &view.screen
                                            && let Page::Items(_, table) = &nav.page
                                        {
                                            Some(table.archive_focus.clone())
                                        } else {
                                            None
                                        };
                                        view.open_group_fields(None, trigger, w, cx);
                                    });
                                }),
                        )
                        .item(PopupMenuItem::new("Automations").on_click(move |_, _, cx| {
                            let _ = automations.update(cx, |view, cx| {
                                if let Screen::Navigation(nav) = &mut view.screen
                                    && let Page::Items(_, table) = &mut nav.page
                                {
                                    table.automations_open = true;
                                    cx.notify();
                                }
                            });
                        }))
                        .item(
                            PopupMenuItem::new("Reload board").on_click(move |_, w, cx| {
                                let _ = reload.update(cx, |view, cx| {
                                    if let Screen::Navigation(nav) = &view.screen
                                        && let Page::Items(board, _) = &nav.page
                                    {
                                        view.reload_main_table(*board, w, cx);
                                    }
                                });
                            }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Clear filter").on_click(move |_, w, cx| {
                                let _ = clear.update(cx, |view, cx| view.clear_all_view_filters(w, cx));
                            }),
                        )
                        .item(PopupMenuItem::new("Archive…").on_click(move |_, w, cx| {
                            let _ = archive.update(cx, |view, cx| view.open_recovery(false, w, cx));
                        }))
                        .item(PopupMenuItem::new("Trash…").on_click(move |_, w, cx| {
                            let _ = trash.update(cx, |view, cx| view.open_recovery(true, w, cx));
                        }))
                    }),
            )
    }

    fn render_automations(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let mut panel = div()
            .id("automations")
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Automations"));
        if self.editable {
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Input::new(&self.automation_name).id("automation-name"))
                    .child(
                        Button::new("create-automation")
                            .small()
                            .label("Create automation")
                            .on_click(cx.listener(|view, _, w, cx| view.create_automation(w, cx))),
                    ),
            );
        }
        if self.automations.is_empty() {
            panel = panel.child(Label::new("No automations"));
        }
        for rule in &self.automations {
            let id = rule.id;
            let last = rule
                .last_item_title
                .as_ref()
                .map(|title| format!("Last run: {title}"))
                .unwrap_or_else(|| "Never run".to_owned());
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Label::new(rule.name.clone())
                            .id(SharedString::from(format!("automation-{id}"))),
                    )
                    .child(
                        Label::new("When item renamed → Record a run")
                            .id(SharedString::from(format!("automation-rule-{id}"))),
                    )
                    .child(
                        Label::new(last).id(SharedString::from(format!("automation-last-{id}"))),
                    ),
            );
        }
        panel
    }

    fn render_heading_header(&self, h: &Heading, cx: &mut Context<Connected>) -> impl IntoElement {
        let id = h.id.clone();
        let menu_id = id.clone();
        let expanded = !h.collapsed;
        let group_empty = self.matching_count(h, cx) == 0;
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
        let visible = self.matching_count(h, cx);
        let count: SharedString = if !self.has_filters() {
            h.count.clone()
        } else {
            format!("{visible} {}", if visible == 1 { "item" } else { "items" }).into()
        };
        header = header.child(Label::new(count));
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
                                _ = view.update(cx, |v, cx| {
                                    v.open_group_fields(Some(&id), None, w, cx)
                                });
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
        let mut body =
            div()
                .relative()
                .flex()
                .flex_col()
                .gap_0()
                .on_prepaint(move |bounds, w, _| {
                    geometry
                        .borrow_mut()
                        .set_viewport(bounds.intersect(&w.visual_viewport_bounds()));
                });
        let group = h.id.clone();
        let visible = self.matching_count(h, cx);
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
            BoardRow::new(Label::new("Item"))
                .header()
                .leading(
                    Checkbox::new(SharedString::from(format!("select-group-{group}")))
                        .small()
                        .accessibility_label(format!("Select all items in {}", h.name))
                        // Tuesday counts selected physical-group records, including hidden rows,
                        // against the filtered count; selection itself survives search.
                        .checked(visible > 0 && selected == visible)
                        .indeterminate(selected > 0 && selected < visible)
                        .disabled(visible == 0 || self.pending || self.reconcile)
                        .on_change(cx.listener(move |v, checked, w, cx| {
                            v.select_main_table_group(&group, *checked, w, cx)
                        })),
                )
                .cell(self.status.header()),
        );
        if h.rows.is_empty() || visible == 0 {
            body = body.child(Label::new("No items"));
        }
        for r in &h.rows {
            let name = r.read(cx).edit.accepted().to_owned();
            if !self.matches_filter(r, &h.id, cx) {
                continue;
            }
            let id = r.read(cx).edit.identity().1.as_uuid();
            let checked = self.selection.ids.contains(&id);
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
                        BoardRow::new(
                            div()
                                .flex()
                                .items_center()
                                .w_full()
                                .min_w_0()
                                .child(div().flex_1().min_w_0().child(r.clone()))
                                .child(self.root_archive_menu(id, cx)),
                        )
                        .leading(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Checkbox::new(SharedString::from(format!("select-{id}")))
                                        .small()
                                        .accessibility_label(format!("Select {name}"))
                                        .checked(checked)
                                        .disabled(self.pending || self.reconcile)
                                        .on_change(cx.listener(move |v, checked, w, cx| {
                                            v.select_main_table_row(
                                                id,
                                                *checked,
                                                w.modifiers().shift,
                                                w,
                                                cx,
                                            )
                                        })),
                                )
                                .child(self.item_grip(h, id, &name)),
                        )
                        .cell(self.status_cell(id, &name, cx)),
                    )
                    .child(self.item_drop_marker(h, id, cx)),
            );
            if let Some(message) = self.status.notice_for(id) {
                body = body.child(Label::new(message));
            }
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
    fn queue_view_filter(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        let board = *board;
        let visit = self.navigation_request;
        let query = table.view_filter.read(cx).value().to_string();
        table.search_task = Some(cx.spawn_in(w, async move |v, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            _ = v.update_in(cx, |v, _, cx| {
                if v.navigation_request != visit {
                    return;
                }
                let Screen::Navigation(nav) = &mut v.screen else {
                    return;
                };
                let Page::Items(current, table) = &mut nav.page else {
                    return;
                };
                if *current != board {
                    return;
                }
                table.search_invalid = query.encode_utf16().count() > 240;
                if !table.search_invalid {
                    table.filter = query.trim().to_owned();
                    table.selection_cache = false;
                    table.drag.borrow_mut().cancel();
                    for h in &table.headings {
                        h.row_drag.borrow_mut().cancel();
                    }
                }
                cx.notify();
            });
        }));
    }

    fn clear_view_filter(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let input = {
            let Screen::Navigation(nav) = &mut self.screen else {
                return;
            };
            let Page::Items(_, table) = &mut nav.page else {
                return;
            };
            table.filter.clear();
            table.search_task = None;
            table.search_invalid = false;
            table.selection_cache = false;
            table.view_filter.clone()
        };
        input.update(cx, |input, cx| input.set_value("", w, cx));
        input.update(cx, |input, cx| input.focus(w, cx));
        cx.notify();
    }

}

impl Connected {
    fn create_automation(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let (client, board, name, name_input) = {
            let Screen::Navigation(nav) = &self.screen else {
                return;
            };
            let Page::Items(board, table) = &nav.page else {
                return;
            };
            (
                nav.client.clone(),
                *board,
                table.automation_name.read(cx).value().trim().to_owned(),
                table.automation_name.clone(),
            )
        };
        if name.is_empty() {
            self.message = "Enter an automation name.";
            cx.notify();
            return;
        }
        self.message = "";
        let work = cx
            .background_executor()
            .spawn(async move { client.create_automation(board, Uuid::new_v4(), &name) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if matches!(&result, Err(EditError::SessionRejected)) {
                    view.link_input = None;
                    view.message = "Session expired. Sign in again.";
                    view.screen = Self::identifier(w, cx);
                    return;
                }
                let message;
                {
                    let Screen::Navigation(nav) = &mut view.screen else {
                        return;
                    };
                    let Page::Items(current, table) = &mut nav.page else {
                        return;
                    };
                    if *current != board {
                        return;
                    }
                    match result {
                        Ok(saved) => {
                            table.automations.push(saved);
                            table.automations.sort_by_key(|a| a.name.to_lowercase());
                            name_input.update(cx, |input, cx| input.set_value("", w, cx));
                            message = "Automation created";
                        }
                        Err(EditError::Conflict) => {
                            message = "An automation with that name already exists.";
                        }
                        Err(EditError::InvalidDraft) => message = "Enter an automation name.",
                        Err(_) => message = "Could not create the automation.",
                    }
                }
                view.message = message;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
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

impl Connected {
    /// One accepted change. The event names a row of the open board; ignore it
    /// anywhere else, read that exact row, then replace only its accepted name.
    pub(super) fn receive_change(
        &mut self,
        event: ChangeEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(board, table) = &nav.page else {
            return;
        };
        if *board != event.board_id {
            return;
        }
        let Some(row) = table
            .headings
            .iter()
            .flat_map(|h| &h.rows)
            .find(|row| row.read(cx).edit.identity().1.as_uuid() == event.item_id)
            .cloned()
        else {
            return;
        };
        let client = nav.client.clone();
        let board = *board;
        let item = event.item_id;
        // The feed carries no content; only this authorized read may supply it.
        let work = cx
            .background_executor()
            .spawn(async move { client.read_main_table_item(board, item) });
        cx.spawn_in(w, async move |view, cx| {
            let saved = work.await;
            _ = view.update_in(cx, |view, _, cx| {
                // A failed read leaves the row exactly as it was; its own save
                // path still owns any error it shows.
                if let Ok(saved) = saved {
                    view.apply_accepted(board, row, saved, cx);
                }
            });
        })
        .detach();
    }

    /// Replace one row's accepted name. The visit, focus, folds and every other
    /// row stay as they are; no board read or rebuild happens here.
    fn apply_accepted(
        &mut self,
        board: Uuid,
        row: Entity<Row>,
        saved: SavedItem,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(current, table) = &nav.page else {
            return;
        };
        if *current != board
            || !table
                .headings
                .iter()
                .flat_map(|h| &h.rows)
                .any(|r| r == &row)
        {
            return;
        }
        let Some(id) = ItemId::new(saved.id) else {
            return;
        };
        let revision = saved.revision;
        row.update(cx, |row, cx| {
            let applied = row.edit.accept_remote(StoredItem {
                id,
                title: saved.title,
                revision,
            });
            if !applied {
                return;
            }
            // A rename already in progress keeps its draft but now saves against
            // the revision the other window got accepted.
            if let Some((_, expected)) = &mut row.baseline {
                *expected = revision;
            }
            cx.notify();
        });
        cx.notify();
    }
}
