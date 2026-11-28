//! Visit-local workspace navigation. No routing, requests or board writes live here.

use std::time::Duration;

use boardcraft_mobile_edit_client::BoardSummary;
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, Role, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Task, Window,
    assets::IconName,
    base::{
        Disableable, Selectable,
        motion::{Easing, Presence, Transition},
    },
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
    },
    deferred, div, px, rems,
};
use uuid::Uuid;

use crate::board_search::filter_workspace_boards;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    Pinned,
    Collapsed,
    Peek,
}

pub(crate) enum NavigationEvent {
    Open(Option<Uuid>),
    Refresh,
    LogOut,
}

enum BoardList {
    Loading,
    Ready(Vec<BoardSummary>),
    Failed,
    Denied,
}

/// The input owns the query; presentation, focus and the cancellable hover task
/// belong to this view and survive board changes and pin/collapse transitions.
pub(crate) struct Navigation {
    presentation: Presentation,
    compact: bool,
    drawer_open: bool,
    _window_bounds: Subscription,
    pointer: [bool; 2], // trigger and panel form one hover region
    transition: Option<(bool, Task<()>)>,
    rail_focus: FocusHandle,
    panel_focus: FocusHandle,
    search_focus: FocusHandle,
    search: Entity<InputState>,
    search_open: bool,
    _search_events: Subscription,
    boards: BoardList,
    selected: Option<Uuid>,
    workspace: Option<Uuid>,
    authenticated: bool,
    busy: bool,
}

impl EventEmitter<NavigationEvent> for Navigation {}

impl Navigation {
    pub(crate) fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(w, cx).placeholder("Search boards"));
        let search_events = cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            presentation: Presentation::Pinned,
            compact: is_compact(w),
            drawer_open: false,
            _window_bounds: cx.observe_window_bounds(w, |view, w, cx| {
                let compact = is_compact(w);
                if compact != view.compact {
                    view.compact = compact;
                    view.transition = None;
                    view.pointer = [false; 2];
                    if view.presentation == Presentation::Peek {
                        view.presentation = Presentation::Collapsed;
                    }
                    if compact && !view.drawer_open && view.panel_focus.contains_focused(w, cx) {
                        view.rail_focus.focus(w, cx);
                    }
                }
                cx.notify();
            }),
            pointer: [false; 2],
            transition: None,
            rail_focus: cx.focus_handle().tab_stop(true),
            panel_focus: cx.focus_handle(),
            search_focus: cx.focus_handle().tab_stop(true),
            search,
            search_open: false,
            _search_events: search_events,
            boards: BoardList::Loading,
            selected: None,
            workspace: None,
            authenticated: false,
            busy: false,
        }
    }

    pub(crate) fn loading(&mut self, authenticated: bool, cx: &mut Context<Self>) {
        self.boards = BoardList::Loading;
        self.authenticated = authenticated;
        self.busy = false;
        cx.notify();
    }

    pub(crate) fn loaded(
        &mut self,
        boards: Option<&[BoardSummary]>,
        selected: Option<Uuid>,
        denied: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = boards.and_then(|boards| {
            selected
                .and_then(|id| boards.iter().find(|board| board.id == id))
                .or_else(|| {
                    boards
                        .iter()
                        .find(|board| Some(board.workspace_id) == self.workspace)
                })
                .or_else(|| boards.first())
                .map(|board| board.workspace_id)
        });
        // A temporary list failure is not a workspace change. Keep its draft.
        if workspace.is_some() && workspace != self.workspace {
            self.workspace = workspace;
            self.set_search_open(false, w, cx);
        }
        self.selected = selected;
        self.boards = match boards {
            Some(boards) => BoardList::Ready(boards.to_vec()),
            None if denied => BoardList::Denied,
            None => BoardList::Failed,
        };
        cx.notify();
    }

    pub(crate) fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        if busy {
            self.transition = None;
        }
        cx.notify();
    }

    /// Called for history as well as row activation; never unpins navigation.
    pub(crate) fn dismiss(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.transition = None;
        self.pointer = [false; 2];
        if self.drawer_open || self.presentation == Presentation::Peek {
            if self.drawer_open || self.panel_focus.contains_focused(w, cx) {
                self.rail_focus.focus(w, cx);
            }
            self.drawer_open = false;
            if self.presentation == Presentation::Peek {
                self.presentation = Presentation::Collapsed;
            }
            cx.notify();
        }
    }

    fn set_pinned(&mut self, pinned: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.transition = None;
        self.pointer = [false; 2];
        if self.compact {
            self.drawer_open = pinned;
            if pinned {
                self.panel_focus.focus(w, cx);
            } else {
                self.rail_focus.focus(w, cx);
            }
            cx.notify();
            return;
        }
        self.presentation = if pinned {
            Presentation::Pinned
        } else {
            Presentation::Collapsed
        };
        self.rail_focus.focus(w, cx);
        cx.notify();
    }

    fn pointer_changed(
        &mut self,
        region: usize,
        over: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pointer[region] = over;
        if self.busy
            || self.compact
            || self.drawer_open
            || self.presentation == Presentation::Pinned
            || !hover_enabled()
        {
            self.transition = None;
            return;
        }
        let over = self.pointer.iter().any(|over| *over);
        if (self.presentation == Presentation::Peek) == over {
            self.transition = None;
            return;
        }
        if self
            .transition
            .as_ref()
            .is_some_and(|(target, _)| *target == over)
        {
            return;
        }
        self.transition = None;
        self.transition = Some((
            over,
            cx.spawn_in(w, async move |view, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(if over { 140 } else { 220 }))
                    .await;
                let _ = view.update_in(cx, |view, w, cx| {
                    if view.presentation == Presentation::Pinned
                        || view.busy
                        || view.compact
                        || view.drawer_open
                        || !hover_enabled()
                    {
                        return;
                    }
                    if over && view.pointer.iter().any(|over| *over) {
                        view.presentation = Presentation::Peek;
                        view.panel_focus.focus(w, cx);
                        cx.notify();
                    } else if !over && !view.pointer.iter().any(|over| *over) {
                        view.dismiss(w, cx);
                    }
                });
            }),
        ));
    }

    fn set_search_open(&mut self, open: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.search_open = open;
        self.search.update(cx, |input, cx| {
            input.set_value("", w, cx);
            if open {
                input.focus(w, cx);
            }
        });
        cx.notify();
    }

    fn escape(&mut self, event: &gpui_kit::KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" {
            return;
        }
        // Consume before Input's Escape action. The first Escape clears search;
        // the second can dismiss the peek without ever changing the pin.
        if self.search_open
            && self
                .search
                .read(cx)
                .focus_handle(cx)
                .contains_focused(w, cx)
        {
            self.set_search_open(false, w, cx);
            self.search_focus.focus(w, cx);
            cx.stop_propagation();
        } else if self.drawer_open || self.presentation == Presentation::Peek {
            self.dismiss(w, cx);
            cx.stop_propagation();
        }
    }

    fn rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("main-navigation")
            .role(Role::Navigation)
            .aria_label("Main navigation")
            .w(rems(4.5))
            .h_full()
            .flex_shrink_0()
            .py_4()
            .px_1()
            .bg(cx.theme().sidebar)
            .child(
                // Kit's managed tooltip installs its own on_hover on Button.
                // Track the trigger region on a distinct element, never twice.
                div()
                    .id("workspace-hover-trigger")
                    .on_hover(cx.listener(|s, over, w, cx| s.pointer_changed(0, *over, w, cx)))
                    .child(
                        Button::new("workspace-navigation-toggle")
                            .ghost()
                            .selected(true)
                            .w_full()
                            .h_12()
                            .px_0()
                            .accessibility_label("Workspace navigation")
                            .tooltip(
                                if !self.compact && self.presentation == Presentation::Pinned {
                                    "Collapse workspace navigation"
                                } else {
                                    "Open workspace navigation"
                                },
                            )
                            .aria_expanded(self.panel_visible())
                            .track_focus(&self.rail_focus)
                            .disabled(self.busy)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_1()
                                    .text_xl()
                                    .child(IconName::PanelsTopLeft)
                                    .child(div().text_xs().child("Workspace")),
                            )
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.set_pinned(
                                    if s.compact {
                                        !s.drawer_open
                                    } else {
                                        s.presentation != Presentation::Pinned
                                    },
                                    w,
                                    cx,
                                )
                            })),
                    ),
            )
    }

    fn panel_visible(&self) -> bool {
        self.drawer_open || (!self.compact && self.presentation != Presentation::Collapsed)
    }

    fn panel(
        &self,
        overlay: bool,
        visible: bool,
        width: Pixels,
        progress: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // A closing panel can still paint its slide, but cannot accept a command.
        let disabled = self.busy || !visible;
        let mut list = div()
            .id("board-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        match &self.boards {
            BoardList::Ready(boards) => {
                let query = self.search.read(cx).value();
                let matches = filter_workspace_boards(boards, self.workspace, &query);
                let empty = matches.is_empty();
                for board in matches {
                    let id = board.id;
                    let active = self.selected == Some(id);
                    list = list.child(
                        div().py_0p5().child(
                            Button::new(SharedString::from(format!("board-{id}")))
                                .ghost()
                                .selected(active)
                                .disabled(disabled)
                                .w_full()
                                .accessibility_label(if active {
                                    format!("{}, current board", board.name)
                                } else {
                                    board.name.clone()
                                })
                                .child(IconName::PanelsTopLeft)
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .child(board.name.clone()),
                                )
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.dismiss(w, cx);
                                    cx.emit(NavigationEvent::Open(Some(id)));
                                })),
                        ),
                    );
                }
                if empty {
                    let any = boards
                        .iter()
                        .any(|board| Some(board.workspace_id) == self.workspace);
                    list = list.child(
                        div()
                            .id("list-status")
                            .role(Role::Status)
                            .aria_label(if any {
                                "No matching boards."
                            } else {
                                "No boards"
                            })
                            .child(if any {
                                "No matching boards."
                            } else {
                                "No boards"
                            }),
                    );
                }
            }
            BoardList::Failed => {
                list = list.child("Could not load boards").child(
                    Button::new("retry-list")
                        .label("Retry board list")
                        .disabled(disabled)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(NavigationEvent::Refresh))),
                );
            }
            BoardList::Loading | BoardList::Denied => {}
        }
        let mut panel = div()
            .id("workspace-sidebar")
            .role(Role::Navigation)
            .aria_label("Workspace navigation")
            .track_focus(&self.panel_focus)
            .flex()
            .flex_col()
            .h_full()
            .w(width)
            .flex_shrink_0()
            .px_3()
            .py_4()
            .gap_3()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .on_hover(cx.listener(|s, over, w, cx| s.pointer_changed(1, *over, w, cx)))
            .capture_key_down(cx.listener(Self::escape))
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_2()
                    .child(div().flex_1().min_w_0().truncate().child("Workspace"))
                    .child(
                        Button::new("workspace-search-toggle")
                            .ghost()
                            .icon(IconName::Search)
                            .accessibility_label("Search workspace boards")
                            .tooltip("Search workspace boards")
                            .aria_expanded(self.search_open)
                            .track_focus(&self.search_focus)
                            .disabled(disabled)
                            .on_click(
                                cx.listener(|s, _, w, cx| s.set_search_open(!s.search_open, w, cx)),
                            ),
                    )
                    .child(
                        Button::new("workspace-navigation-close")
                            .ghost()
                            .icon(IconName::ChevronLeft)
                            .accessibility_label("Close workspace navigation")
                            .tooltip("Collapse workspace navigation")
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.drawer_open = false;
                                s.set_pinned(false, w, cx);
                            })),
                    ),
            )
            .child(
                Button::new("back")
                    .ghost()
                    .accessibility_label("Boards")
                    .child(div().flex_1().child("Boards"))
                    .selected(self.selected.is_none())
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.dismiss(w, cx);
                        cx.emit(NavigationEvent::Open(None));
                    })),
            )
            .child(div().px_2().text_sm().child("Content"))
            .children(self.search_open.then(|| {
                div().px_2().child(
                    Input::new(&self.search)
                        .id("workspace-search-input")
                        .small()
                        .w_full()
                        .disabled(disabled)
                        .aria_label("Search boards"),
                )
            }))
            .child(list);
        if self.authenticated {
            panel = panel.child("Signed in").child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("check-session")
                            .ghost()
                            .label("Check session")
                            .disabled(disabled)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(NavigationEvent::Refresh))),
                    )
                    .child(
                        Button::new("logout")
                            .ghost()
                            .label("Log out")
                            .disabled(disabled)
                            .loading(self.busy)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(NavigationEvent::LogOut))),
                    ),
            );
        }
        if overlay {
            if visible {
                // A flyout covers the board's hit targets as well as its paint.
                // Otherwise a sidebar click can also focus or activate a row below it.
                panel = panel.occlude();
            }
            panel = panel
                .absolute()
                .left(-width * (1.0 - progress))
                .top_0()
                .bottom_0()
                .shadow_lg();
        }
        panel
    }
}

impl Render for Navigation {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.panel_visible();
        let pinned = !self.compact && self.presentation == Presentation::Pinned;
        let rail_width = rems(4.5).to_pixels(w.rem_size());
        let width = rems(17.375)
            .to_pixels(w.rem_size())
            .min((w.viewport_size().width - rail_width).max(px(0.)));
        let motion = if reduced_motion(cx) {
            Duration::ZERO
        } else {
            Duration::from_millis(300)
        };
        let overlay =
            Presence::new("workspace-navigation-slide", visible && !pinned)
                .transition(Transition::new(motion).easing(
                    Easing::cubic_bezier(0.4, 0.0, 0.2, 1.0).expect("valid sidebar easing"),
                ))
                .sample(w, cx);
        div()
            .id("workspace-navigation")
            .relative()
            .flex()
            .h_full()
            .flex_shrink_0()
            .child(self.rail(cx))
            .children(pinned.then(|| self.panel(false, true, width, 1.0, cx)))
            // Desktop reveals a stationary panel; compact moves the drawer.
            // Both clip at the rail and never reserve table space.
            .children((!pinned && overlay.should_render()).then(|| {
                deferred(
                    div()
                        .id("workspace-overlay-clip")
                        .absolute()
                        .left(rail_width)
                        .top_0()
                        .bottom_0()
                        .w(if self.compact {
                            width
                        } else {
                            width * overlay.progress
                        })
                        .overflow_hidden()
                        .child(self.panel(
                            true,
                            visible,
                            width,
                            if self.compact { overlay.progress } else { 1.0 },
                            cx,
                        )),
                )
                .with_priority(1)
            }))
    }
}

fn is_compact(w: &Window) -> bool {
    // This is the reference viewport breakpoint, independent of UI font zoom.
    w.viewport_size().width <= px(700.)
}

fn reduced_motion(cx: &gpui_kit::App) -> bool {
    #[cfg(target_family = "wasm")]
    if web_sys::window().is_some_and(|window| {
        window
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()
            .flatten()
            .is_some_and(|query| query.matches())
    }) {
        return true;
    }
    cx.reduce_motion()
}
fn hover_enabled() -> bool {
    #[cfg(target_family = "wasm")]
    {
        web_sys::window().is_some_and(|window| {
            window
                .match_media("(hover: hover) and (min-width: 701px)")
                .ok()
                .flatten()
                .is_some_and(|query| query.matches())
        })
    }
    #[cfg(not(target_family = "wasm"))]
    {
        true
    }
}
