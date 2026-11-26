//! Ordered visit state, independent of names, folds and mounted controls.
use super::*;

#[derive(Clone, Default)]
pub(super) struct Selection {
    pub(super) ids: Vec<Uuid>,
    anchor: Option<Uuid>,
}

impl Selection {
    pub(super) fn retain(&mut self, available: &[Uuid]) {
        self.ids.retain(|id| available.contains(id));
        if self.anchor.is_some_and(|id| !available.contains(&id)) {
            self.anchor = None;
        }
    }
    fn block(&mut self, ids: &[Uuid], checked: bool) {
        self.ids.retain(|id| !ids.contains(id));
        if checked {
            self.ids.extend_from_slice(ids);
        }
    }
    fn row(&mut self, id: Uuid, checked: bool, shift: bool, order: &[Uuid]) -> bool {
        if shift && let Some(anchor) = self.anchor {
            let (Some(a), Some(b)) = (
                order.iter().position(|r| *r == anchor),
                order.iter().position(|r| *r == id),
            ) else {
                return false;
            };
            self.block(&order[a.min(b)..=a.max(b)], checked);
        } else {
            self.block(&[id], checked);
            self.anchor = Some(id);
        }
        true
    }
}

#[derive(Clone)]
enum Gesture {
    Group(SharedString, bool),
    Range(Uuid, bool),
}

impl MainTable {
    fn order(&self, cx: &App) -> Vec<Uuid> {
        self.headings
            .iter()
            .flat_map(|h| {
                h.rows
                    .iter()
                    .map(|r| r.read(cx).edit.identity().1.as_uuid())
            })
            .collect()
    }
    fn apply_selection(&mut self, gesture: &Gesture, cx: &App) -> bool {
        match gesture {
            Gesture::Group(id, checked) => {
                let Some(h) = self.headings.iter().find(|h| h.id == *id) else {
                    return false;
                };
                let ids: Vec<_> = h
                    .rows
                    .iter()
                    .map(|r| r.read(cx).edit.identity().1.as_uuid())
                    .collect();
                self.selection.block(&ids, *checked);
                true
            }
            Gesture::Range(id, checked) => self.selection.row(*id, *checked, true, &self.order(cx)),
        }
    }
    pub(super) fn selection_toolbar(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        div()
            .id("selection-toolbar")
            .role(Role::Toolbar)
            .aria_label("Selected item actions")
            .flex()
            .items_center()
            .gap_2()
            .child(Label::new(format!("{} selected", self.selection.ids.len())))
            .child(
                Button::new("selection-move")
                    .track_focus(&self.move_focus)
                    .small()
                    .label("Move to…")
                    .disabled(!self.editable || self.pending || self.reconcile)
                    .on_click(cx.listener(|v, _, w, cx| v.open_selection_move(w, cx))),
            )
            .child(
                Button::new("selection-clear")
                    .small()
                    .ghost()
                    .label("Clear item selection")
                    .disabled(self.pending)
                    .on_click(cx.listener(|v, _, _, cx| {
                        if let Screen::Navigation(nav) = &mut v.screen
                            && let Page::Items(_, t) = &mut nav.page
                            && !t.pending
                        {
                            t.selection.ids.clear(); // The ordinary-row anchor deliberately survives Clear.
                            cx.notify();
                        }
                    })),
            )
    }
}

impl Connected {
    pub(super) fn select_main_table_row(
        &mut self,
        id: Uuid,
        checked: bool,
        shift: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        if table.pending
            || table.reconcile
            || table.move_dialog.is_some()
            || !table.order(cx).contains(&id)
        {
            return;
        }
        if shift && table.selection.anchor.is_some() {
            self.resolve_selection(Gesture::Range(id, checked), w, cx);
        } else {
            table.selection.row(id, checked, false, &[]);
            cx.notify();
        }
    }
    pub(super) fn select_main_table_group(
        &mut self,
        id: &str,
        checked: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.resolve_selection(Gesture::Group(id.to_owned().into(), checked), w, cx);
    }
    fn resolve_selection(&mut self, gesture: Gesture, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.pending || table.reconcile || table.move_dialog.is_some() {
            return;
        }
        if table.selection_cache {
            table.apply_selection(&gesture, cx);
            self.message = "";
            cx.notify();
            return;
        }
        let board = *board;
        let client = nav.client.clone();
        let visit = self.navigation_request;
        table.set_busy(true, cx);
        let work = cx
            .background_executor()
            .spawn(async move { client.read_main_table(board) });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                if v.navigation_request != visit { return; }
                let Screen::Navigation(nav) = &mut v.screen else { return; };
                let Page::Items(current, table) = &mut nav.page else { return; };
                if *current != board { return; }
                table.set_busy(false, cx);
                match result {
                    Ok(read) => {
                        let matches = read.groups().len() == table.headings.len() && read.groups().iter().zip(&table.headings).all(|(g,h)| {
                            g.id() == h.id.as_ref() && g.items().iter().map(|r| r.id).eq(h.rows.iter().map(|r| r.read(cx).edit.identity().1.as_uuid()))
                        });
                        if matches && table.apply_selection(&gesture, cx) {
                            table.selection_cache = true;
                            v.message = "";
                        } else { v.message = "Items changed. Reload board before extending the selection."; }
                    }
                    Err(EditError::SessionRejected) => {
                        v.link_input = None;
                        v.screen = Self::identifier(w, cx);
                        v.message = "Session expired. Sign in again.";
                    }
                    Err(EditError::PermissionDenied | EditError::Missing) => {
                        nav.page = Page::Unavailable;
                        nav.share = None;
                        v.message = "Destination unavailable";
                    }
                    Err(_) => { v.message = "Could not read selection. Repeat the gesture to retry, or reload board."; }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{Selection, Uuid};
    use core::prelude::v1::test;
    fn ids(ns: &[u128]) -> Vec<Uuid> {
        ns.iter().copied().map(Uuid::from_u128).collect()
    }
    #[test]
    fn ordered_blocks_and_hidden_anchor_survive_clear() {
        let order = ids(&[73, 61, 67, 77, 59]);
        let mut s = Selection::default();
        for n in [67, 59, 61] {
            s.row(Uuid::from_u128(n), true, false, &order);
        }
        assert_eq!(s.ids, ids(&[67, 59, 61]));
        s.block(&ids(&[73, 61, 67, 77]), true);
        assert_eq!(s.ids, ids(&[59, 73, 61, 67, 77]));
        s.ids.clear();
        s.row(Uuid::from_u128(59), true, true, &order);
        assert_eq!(s.ids, ids(&[61, 67, 77, 59]));
        s.row(Uuid::from_u128(67), false, true, &order);
        assert_eq!(s.ids, ids(&[77, 59]));
        assert!(!s.row(Uuid::from_u128(99), true, true, &order));
        assert_eq!(s.ids, ids(&[77, 59]));
    }

    #[test]
    fn reverse_ranges_remove_only_the_interval_and_group_toggles_keep_anchor() {
        let order = ids(&[73, 61, 67, 77, 59]);
        let mut s = Selection::default();
        s.row(Uuid::from_u128(59), true, false, &order);
        s.block(&ids(&[73, 61, 67, 77]), true);
        s.block(&ids(&[73, 61, 67, 77]), false);
        s.ids.clear();
        s.row(Uuid::from_u128(67), true, true, &order);
        assert_eq!(s.ids, ids(&[67, 77, 59]));
        s.row(Uuid::from_u128(77), false, true, &order);
        assert_eq!(s.ids, ids(&[67]));
        s.retain(&ids(&[73, 61, 67, 77]));
        s.row(Uuid::from_u128(73), true, true, &order);
        assert_eq!(s.ids, ids(&[67, 73])); // Missing anchor falls back to an ordinary toggle.
    }
}
