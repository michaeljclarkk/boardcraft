//! The real navigation view, with a sibling standing in for the board surface.
use super::*;
use gpui_kit::{
    AnyWindowHandle, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size,
    test::TestWindowExt,
};

struct Visit {
    navigation: Entity<Navigation>,
    board_focus: FocusHandle,
    opened: Vec<Option<Uuid>>,
    _events: Subscription,
}

impl Render for Visit {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(self.navigation.clone())
            .child(
                Button::new("board-surface")
                    .label("Board surface")
                    .track_focus(&self.board_focus)
                    .on_click(cx.listener(|s, _, w, cx| s.board_focus.focus(w, cx)))
                    .flex_1()
                    .h_full(),
            )
    }
}

fn boards() -> Vec<BoardSummary> {
    [(42, "Review", 1), (17, "Backlog", 1), (63, "Review", 2)]
        .into_iter()
        .map(|(id, name, workspace)| BoardSummary {
            id: Uuid::from_u128(id),
            workspace_id: Uuid::from_u128(workspace),
            name: name.into(),
        })
        .collect()
}

fn visit(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Visit>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.set_reduce_motion(true));
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1000.), px(800.)),
                ))),
                ..Default::default()
            },
            cx,
            |w, cx| {
                cx.new(|cx| {
                    let navigation = cx.new(|cx| {
                        let mut navigation = Navigation::new(w, cx);
                        navigation.loading(false, cx);
                        navigation.loaded(Some(&boards()), Some(Uuid::from_u128(42)), false, w, cx);
                        navigation
                    });
                    let events = cx.subscribe(&navigation, |view: &mut Visit, _, event, _| {
                        if let NavigationEvent::Open(id) = event {
                            view.opened.push(*id);
                        }
                    });
                    Visit {
                        navigation,
                        board_focus: cx.focus_handle().tab_stop(true),
                        opened: Vec::new(),
                        _events: events,
                    }
                })
            },
        )
        .unwrap()
    })
}

fn advance(cx: &mut TestAppContext, millis: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(millis));
    cx.run_until_parked();
}

fn board(id: u128) -> String {
    format!("board-{}", Uuid::from_u128(id))
}

#[gpui_kit::test]
fn search_filters_current_workspace_and_escape_clears_and_restores_toggle(cx: &mut TestAppContext) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx); // Also guards the former tooltip + on_hover panic.
        assert!(w.try_find(board(63)).is_none());
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("workspace-search-input").focused(), Some(true));
        w.input("CKlo", cx);
        assert!(w.try_find(board(42)).is_none());
        assert!(w.try_find(board(17)).is_some());
        assert!(visit.read(cx).opened.is_empty());
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("workspace-search-input").is_none());
        assert_eq!(w.find("workspace-search-toggle").focused(), Some(true));
        assert!(w.try_find(board(42)).is_some());
        assert!(w.try_find(board(17)).is_some());
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("workspace-search-input").value(), Some(""));
        w.input(" review", cx);
        assert!(w.try_find(board(42)).is_none());
        assert!(w.try_find(board(17)).is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn hover_opens_without_shifting_board_and_leaving_then_reentering_cancels_dismiss(
    cx: &mut TestAppContext,
) {
    let (handle, visit) = visit(cx);
    let collapsed = cx
        .update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            w.click("workspace-navigation-close", cx);
            let bounds = w.find("board-surface").bounds();
            w.hover("workspace-navigation-toggle", cx);
            bounds
        })
        .unwrap();
    advance(cx, 139);
    cx.update(|cx| {
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Collapsed
        )
    });
    advance(cx, 1);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Peek
        );
        assert_eq!(w.find("board-surface").bounds(), collapsed);
        w.hover("workspace-search-toggle", cx);
    })
    .unwrap();
    advance(cx, 300);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Peek
        );
        w.hover("board-surface", cx);
    })
    .unwrap();
    advance(cx, 219);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Peek
        );
        w.hover("workspace-search-toggle", cx);
    })
    .unwrap();
    advance(cx, 300);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Peek
        );
        w.hover("board-surface", cx);
    })
    .unwrap();
    advance(cx, 220);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Collapsed
        );
        assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
        assert_eq!(w.find("board-surface").bounds(), collapsed);
    })
    .unwrap();
}

#[gpui_kit::test]
fn peek_search_escape_then_panel_escape_and_pin_keep_their_separate_contracts(
    cx: &mut TestAppContext,
) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-close", cx);
        w.hover("workspace-navigation-toggle", cx);
    })
    .unwrap();
    advance(cx, 140);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.input("rev", cx);
        w.press("escape", cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Peek
        );
        w.press("escape", cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Collapsed
        );
        assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
        w.press("enter", cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Pinned
        );
        w.hover("board-surface", cx);
    })
    .unwrap();
    advance(cx, 300);
    cx.update(|cx| {
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Pinned
        )
    });
}

#[gpui_kit::test]
fn pin_close_and_board_choice_cancel_timers_but_keep_the_search_draft(cx: &mut TestAppContext) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.input("rev", cx);
        w.click("workspace-navigation-close", cx);
        w.hover("workspace-navigation-toggle", cx);
    })
    .unwrap();
    advance(cx, 100);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-toggle", cx);
        assert_eq!(w.find("workspace-search-input").value(), Some("rev"));
    })
    .unwrap();
    advance(cx, 300);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Pinned
        );
        w.click("workspace-navigation-close", cx);
        w.hover("workspace-navigation-toggle", cx);
    })
    .unwrap();
    advance(cx, 140);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("workspace-search-input").value(), Some("rev"));
        w.click(board(42), cx);
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Collapsed
        );
    })
    .unwrap();
    advance(cx, 300);
    cx.update(|cx| {
        assert_eq!(visit.read(cx).opened, [Some(Uuid::from_u128(42))]);
        let nav = visit.read(cx).navigation.read(cx);
        assert_eq!(nav.presentation, Presentation::Collapsed);
        assert_eq!(nav.search.read(cx).value().as_ref(), "rev");
    });
}

#[gpui_kit::test]
fn pending_logout_cancels_hover_and_disables_navigation_without_erasing_search(
    cx: &mut TestAppContext,
) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-close", cx);
        w.hover("workspace-navigation-toggle", cx);
        let nav = visit.read(cx).navigation.clone();
        nav.update(cx, |nav, cx| nav.set_busy(true, cx));
        w.render_frame(cx);
        w.click("workspace-navigation-toggle", cx);
    })
    .unwrap();
    advance(cx, 300);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        let nav = visit.read(cx).navigation.clone();
        assert_eq!(nav.read(cx).presentation, Presentation::Collapsed);
        assert!(visit.read(cx).opened.is_empty());
        nav.update(cx, |nav, cx| nav.set_busy(false, cx));
        w.render_frame(cx);
        w.click("workspace-navigation-toggle", cx);
        assert_eq!(nav.read(cx).presentation, Presentation::Pinned);
    })
    .unwrap();
}

#[gpui_kit::test]
fn workspace_change_resets_search_but_failed_reload_does_not(cx: &mut TestAppContext) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.input("draft", cx);
        let nav = visit.read(cx).navigation.clone();
        nav.update(cx, |nav, cx| {
            nav.loaded(None, Some(Uuid::from_u128(42)), false, w, cx)
        });
        assert_eq!(nav.read(cx).search.read(cx).value().as_ref(), "draft");
        nav.update(cx, |nav, cx| {
            nav.loaded(Some(&boards()), Some(Uuid::from_u128(63)), false, w, cx)
        });
        w.render_frame(cx);
        assert!(w.try_find("workspace-search-input").is_none());
        assert!(w.try_find(board(42)).is_none());
        assert!(w.try_find(board(63)).is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn compact_drawer_overlays_the_board_and_keeps_desktop_pin_and_search(cx: &mut TestAppContext) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.input("rev", cx);
    })
    .unwrap();
    cx.simulate_window_resize(handle, size(px(700.), px(800.)));
    cx.run_until_parked();
    let narrow = cx
        .update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("workspace-search-toggle").is_none());
            assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
            let bounds = w.find("board-surface").bounds();
            w.click("workspace-navigation-toggle", cx);
            assert_eq!(w.find("board-surface").bounds(), bounds);
            assert_eq!(w.find("workspace-search-input").value(), Some("rev"));
            assert!(visit.read(cx).navigation.read(cx).drawer_open);
            assert_eq!(
                visit.read(cx).navigation.read(cx).presentation,
                Presentation::Pinned
            );
            bounds
        })
        .unwrap();
    cx.update_window(handle, |_, w, cx| {
        w.press("escape", cx);
        assert!(w.try_find("workspace-search-toggle").is_none());
        assert_eq!(w.find("board-surface").bounds(), narrow);
        assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
    })
    .unwrap();
    cx.simulate_window_resize(handle, size(px(701.), px(800.)));
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("workspace-search-toggle").is_some());
        assert_eq!(w.find("workspace-search-input").value(), Some("rev"));
        assert_eq!(
            visit.read(cx).navigation.read(cx).presentation,
            Presentation::Pinned
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn compact_hover_does_not_open_and_search_escape_precedes_drawer_escape(cx: &mut TestAppContext) {
    let (handle, visit) = visit(cx);
    cx.simulate_window_resize(handle, size(px(600.), px(800.)));
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.hover("workspace-navigation-toggle", cx);
    })
    .unwrap();
    advance(cx, 500);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(!visit.read(cx).navigation.read(cx).drawer_open);
        assert!(w.try_find("workspace-search-toggle").is_none());
        w.click("workspace-navigation-toggle", cx);
        w.click("workspace-search-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.input("QXZ", cx);
        assert!(w.try_find(board(42)).is_none());
        w.press("escape", cx);
        assert!(visit.read(cx).navigation.read(cx).drawer_open);
        assert!(w.try_find("workspace-search-input").is_none());
        assert_eq!(w.find("workspace-search-toggle").focused(), Some(true));
        w.press("escape", cx);
        assert!(!visit.read(cx).navigation.read(cx).drawer_open);
        assert!(w.try_find("workspace-search-toggle").is_none());
        assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn compact_selection_dismisses_but_outside_click_keeps_drawer(cx: &mut TestAppContext) {
    let (handle, visit) = visit(cx);
    cx.simulate_window_resize(handle, size(px(680.), px(800.)));
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-toggle", cx);
        w.click("board-surface", cx);
        w.press("escape", cx);
        assert!(visit.read(cx).navigation.read(cx).drawer_open);
        w.click(board(17), cx);
        assert!(!visit.read(cx).navigation.read(cx).drawer_open);
        assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(visit.read(cx).opened, [Some(Uuid::from_u128(17))]));
}

#[gpui_kit::test]
fn explicitly_open_drawer_survives_widening_without_losing_desktop_collapse(
    cx: &mut TestAppContext,
) {
    let (handle, visit) = visit(cx);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-close", cx);
    })
    .unwrap();
    cx.simulate_window_resize(handle, size(px(680.), px(800.)));
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-toggle", cx);
    })
    .unwrap();
    cx.simulate_window_resize(handle, size(px(1000.), px(800.)));
    cx.run_until_parked();
    let table = cx
        .update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("workspace-search-toggle").is_some());
            assert_eq!(
                visit.read(cx).navigation.read(cx).presentation,
                Presentation::Collapsed
            );
            let table = w.find("board-surface").bounds();
            w.hover("board-surface", cx);
            table
        })
        .unwrap();
    advance(cx, 500);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("workspace-search-toggle").is_some());
        w.click("workspace-navigation-close", cx);
        assert!(w.try_find("workspace-search-toggle").is_none());
        assert_eq!(w.find("board-surface").bounds(), table);
        assert_eq!(w.find("workspace-navigation-toggle").focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn drawer_slide_reverses_without_shifting_table_and_reduced_motion_finishes(
    cx: &mut TestAppContext,
) {
    let (handle, visit) = visit(cx);
    cx.simulate_window_resize(handle, size(px(600.), px(800.)));
    cx.run_until_parked();
    let (board_bounds, open_button) = cx
        .update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            let bounds = w.find("board-surface").bounds();
            w.click("workspace-navigation-toggle", cx);
            let open_button = w.find("workspace-search-toggle").bounds();
            w.click("workspace-navigation-toggle", cx);
            (bounds, open_button)
        })
        .unwrap();
    cx.update(|cx| cx.set_reduce_motion(false));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("workspace-navigation-toggle", cx);
    })
    .unwrap();
    advance(cx, 150);
    let middle = cx
        .update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(w.find("board-surface").bounds(), board_bounds);
            let bounds = w.find("workspace-search-toggle").bounds();
            assert!(bounds.left() < open_button.left());
            w.click("workspace-navigation-toggle", cx);
            assert!(!visit.read(cx).navigation.read(cx).drawer_open);
            bounds
        })
        .unwrap();
    advance(cx, 40);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.find("workspace-search-toggle").bounds().left() < middle.left());
        assert_eq!(w.find("board-surface").bounds(), board_bounds);
    })
    .unwrap();
    cx.update(|cx| cx.set_reduce_motion(true));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("workspace-search-toggle").is_none());
        w.click("workspace-navigation-toggle", cx);
        assert_eq!(w.find("workspace-search-toggle").bounds(), open_button);
        assert_eq!(w.find("board-surface").bounds(), board_bounds);
    })
    .unwrap();
}
