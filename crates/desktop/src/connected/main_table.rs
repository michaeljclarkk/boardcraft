//! The visit owns local folds and focus. Only a confirmed authorized read replaces saved state.
use super::navigation::Page;
use super::*;
use boardcraft_mobile_edit_client::MainTableRead;
use gpui_kit::component::{
    IconName, Sizable,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenuItem},
};

struct RootName {
    id: Uuid,
    title: SharedString,
}

struct Heading {
    id: SharedString,
    name: SharedString,
    count: SharedString,
    rows: Vec<RootName>,
    collapsed: bool,
    focus: FocusHandle,
}

pub(super) struct MainTable {
    headings: Vec<Heading>,
    revision: i64,
    editable: bool,
    pending: bool,
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
    pub(super) fn new(read: MainTableRead, cx: &mut App) -> Self {
        Self {
            revision: read.view_revision(),
            editable: read.is_editable(),
            pending: false,
            headings: read
                .groups()
                .iter()
                .map(|g| Heading {
                    id: g.id().to_owned().into(),
                    name: g.name().to_owned().into(),
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
                    rows: g
                        .items()
                        .iter()
                        .map(|r| RootName {
                            id: r.id,
                            title: r.title.clone().into(),
                        })
                        .collect(),
                    collapsed: g.is_collapsed(),
                    focus: cx.focus_handle().tab_stop(true),
                })
                .collect(),
        }
    }

    pub(super) fn render(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let mut column = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Main table"))
            .child(Label::new("Board items · Read only"));
        if !self.editable {
            column = column.child(Label::new(
                "Group presentation changes last only for this visit.",
            ));
        }
        if self.pending {
            column = column.child(Label::new("Saving group presentation…"));
        }
        if self.headings.is_empty() {
            column = column.child(Label::new("Empty board"));
        }
        for h in &self.headings {
            let id = h.id.clone();
            let menu_id = id.clone();
            let expanded = !h.collapsed;
            let view = cx.weak_entity();
            let mut section = div()
                .id(SharedString::from(format!("group-{id}")))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
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
                                .on_click(cx.listener(move |v, _, w, cx| {
                                    v.fold_main_table(id.as_ref(), false, w, cx)
                                })),
                        )
                        .child(Label::new(h.name.clone()))
                        .child(Label::new(h.count.clone()))
                        .child(
                            Button::new(SharedString::from(format!("group-menu-{menu_id}")))
                                .small()
                                .ghost()
                                .label("Group options")
                                .disabled(self.pending)
                                .dropdown_menu(move |menu, _, _| {
                                    let mut menu = menu;
                                    for all in [false, true] {
                                        let view = view.clone();
                                        let id = menu_id.clone();
                                        let label = match (expanded, all) {
                                            (true, false) => "Collapse group",
                                            (false, false) => "Expand group",
                                            (true, true) => "Collapse all groups",
                                            (false, true) => "Expand all groups",
                                        };
                                        menu = menu.item(PopupMenuItem::new(label).on_click(
                                            move |_, w, cx| {
                                                _ = view.update(cx, |v, cx| {
                                                    v.fold_main_table(id.as_ref(), all, w, cx)
                                                });
                                            },
                                        ));
                                    }
                                    menu
                                }),
                        ),
                );
            if expanded {
                let mut body = div().flex().flex_col().gap_1().pl_8();
                if h.rows.is_empty() {
                    body = body.child(Label::new("No items"));
                }
                for r in &h.rows {
                    body = body.child(
                        Button::new(SharedString::from(format!("title-{}", r.id)))
                            .small()
                            .ghost()
                            .label(r.title.clone())
                            .disabled(true),
                    );
                }
                section = section.child(body);
            }
            column = column.child(section);
        }
        column
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
        if table.pending {
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
        table.pending = true;
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
                        let mut table = MainTable::new(read, cx);
                        for h in &mut table.headings {
                            if let Some(previous) = old.headings.iter().find(|p| p.id == h.id) { h.focus = previous.focus.clone(); }
                        }
                        // The original disclosure stays live across body replacement.
                        let target = table.headings.iter().find(|h| h.focus == focus).or_else(|| table.headings.first());
                        if let Some(h) = target { h.focus.focus(w, cx); }
                        nav.page = Page::Items(board, table);
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
