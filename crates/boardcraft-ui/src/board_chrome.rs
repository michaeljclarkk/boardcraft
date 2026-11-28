//! The same board title, view tabs, tools and cell lanes on every Rust client.
//! Callers retain data and commands; these components own the visible structure.
use gpui_kit::{
    AnyElement, App, ElementId, Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    component::{
        ActiveTheme, IconName, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        spinner::Spinner,
        tab::{Tab, TabBar},
    },
    div, rems,
};

/// Native board quick find. The client owns the query, debounce and focus;
/// this dock presents the MAUI board-search actions without a second view layer.
#[derive(IntoElement)]
pub struct BoardFindDock {
    search: Entity<InputState>,
    busy: bool,
    disabled: bool,
    focused: bool,
    focus: Option<FocusHandle>,
    clear: Option<Button>,
    cancel: Option<Button>,
    home: Option<Button>,
    filters: Option<Button>,
}

impl BoardFindDock {
    pub fn new(search: &Entity<InputState>) -> Self {
        Self {
            search: search.clone(),
            busy: false,
            disabled: false,
            focused: false,
            focus: None,
            clear: None,
            cancel: None,
            home: None,
            filters: None,
        }
    }
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
    /// Track the whole dock so focus can move to Clear or Cancel without ending a search.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
    pub fn clear(mut self, button: Button) -> Self {
        self.clear = Some(button);
        self
    }
    pub fn cancel(mut self, button: Button) -> Self {
        self.cancel = Some(button);
        self
    }
    pub fn home(mut self, button: Button) -> Self {
        self.home = Some(button);
        self
    }
    pub fn filters(mut self, button: Button) -> Self {
        self.filters = Some(button);
        self
    }
}

impl RenderOnce for BoardFindDock {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::base::Disableable;
        let mut input = Input::new(&self.search)
            .id("mobile-quick-find")
            .large()
            .bordered(false)
            .appearance(false)
            .flex_1()
            .min_w_0()
            .aria_label("Quick find")
            .disabled(self.disabled);
        // This Android host does not register the SVG icon assets. Keep the dock's
        // actions visible with text and leave no empty search-icon lane.
        if self.busy {
            input = if cfg!(target_os = "android") {
                input.prefix(
                    div()
                        .id("mobile-filter-progress")
                        .role(Role::Status)
                        .aria_label("Filtering items")
                        .child("…"),
                )
            } else {
                input.prefix(Spinner::new().small())
            };
        } else if !cfg!(target_os = "android") {
            input = input.prefix(gpui_kit::component::Icon::new(IconName::Search));
        }
        let mut search = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .h(rems(3.25))
            .pl_3()
            .pr_2()
            .rounded_full()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(input);
        if !self.search.read(cx).value().is_empty() {
            search = search.children(self.clear.map(|button| {
                let button = if cfg!(target_os = "android") {
                    button.label("×")
                } else {
                    button.icon(IconName::Close)
                };
                button
                    .ghost()
                    .accessibility_label("Clear board filter")
                    .h(rems(2.75))
                    .w(rems(2.25))
                    .disabled(self.disabled)
            }));
        }
        let mut dock = div()
            .id("board-find-dock")
            .role(Role::Toolbar)
            .aria_label("Board quick find")
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .pt_2()
            .pb_3()
            .w_full()
            .flex_shrink_0()
            .child(search);
        if let Some(focus) = &self.focus {
            dock = dock.track_focus(focus);
        }
        if self.focused {
            dock = dock.children(self.cancel.map(|button| {
                button
                    .label("Cancel")
                    .h(rems(3.25))
                    .w(rems(7.))
                    .rounded_full()
            }));
        } else {
            dock = dock
                .children(self.filters.map(|button| {
                    button
                        .icon(IconName::Settings2)
                        .accessibility_label("Board filters")
                        .size(rems(3.25))
                        .rounded_full()
                }))
                .children(self.home.map(|button| {
                    button
                        .label("Home")
                        .accessibility_label("Home")
                        .size(rems(3.25))
                        .rounded_full()
                }));
        }
        dock
    }
}

#[derive(IntoElement)]
pub struct BoardHeader {
    title: SharedString,
    actions: Option<AnyElement>,
}

impl BoardHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            actions: None,
        }
    }

    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }
}

impl RenderOnce for BoardHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id("board-header")
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .py_2()
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("board-title")
                    .role(Role::Heading)
                    .flex_1()
                    .min_w_0()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(self.title),
            )
            .children(self.actions)
    }
}

#[derive(IntoElement)]
pub struct BoardTabs {
    tabs: TabBar,
}

impl Default for BoardTabs {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardTabs {
    pub fn new() -> Self {
        Self {
            tabs: TabBar::new("board-views")
                .underline()
                .selected_index(0)
                .child(Tab::new().label("Main table")),
        }
    }
    pub fn tab(mut self, tab: Tab) -> Self {
        self.tabs = self.tabs.child(tab);
        self
    }
    pub fn selected_index(mut self, index: usize) -> Self {
        self.tabs = self.tabs.selected_index(index);
        self
    }
    pub fn on_select(mut self, listener: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.tabs = self.tabs.on_click(listener);
        self
    }
}

impl RenderOnce for BoardTabs {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.tabs
    }
}

#[derive(IntoElement, Default)]
pub struct BoardToolbar {
    new_item: Option<Button>,
    new_item_options: Option<AnyElement>,
    search: Option<(Entity<InputState>, ElementId)>,
    tools: Option<AnyElement>,
    trailing: Vec<AnyElement>,
    busy: bool,
}

impl BoardToolbar {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn new_item(mut self, button: Button) -> Self {
        self.new_item = Some(button);
        self
    }
    pub fn new_item_options(mut self, button: impl IntoElement) -> Self {
        self.new_item_options = Some(button.into_any_element());
        self
    }
    pub fn search(mut self, state: &Entity<InputState>, id: impl Into<ElementId>) -> Self {
        self.search = Some((state.clone(), id.into()));
        self
    }
    pub fn tools(mut self, button: impl IntoElement) -> Self {
        self.tools = Some(button.into_any_element());
        self
    }
    pub fn action(mut self, button: Button) -> Self {
        self.trailing
            .push(button.ghost().small().into_any_element());
        self
    }
    pub fn search_disabled(mut self, disabled: bool) -> Self {
        self.busy = disabled;
        self
    }
    pub fn new_item_options_trigger(id: impl Into<ElementId>) -> Button {
        Button::new(id)
            .primary()
            .small()
            .icon(IconName::ChevronDown)
            .accessibility_label("New item options")
    }
    pub fn tools_trigger(id: impl Into<ElementId>) -> Button {
        Button::new(id)
            .ghost()
            .small()
            .icon(IconName::Ellipsis)
            .accessibility_label("Board tools")
    }
}

impl RenderOnce for BoardToolbar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut tools = div()
            .id("board-tools")
            .role(Role::Toolbar)
            .aria_label("Board tools")
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .py_2();
        if self.new_item.is_some() || self.new_item_options.is_some() {
            tools = tools.child(
                div()
                    .flex()
                    .items_center()
                    .children(
                        self.new_item
                            .map(|button| button.primary().small().label("New item")),
                    )
                    .children(self.new_item_options),
            );
        }
        if let Some((state, id)) = self.search {
            tools = tools.child(
                Input::new(&state)
                    .id(id)
                    .small()
                    .bordered(false)
                    .prefix(IconName::Search)
                    .w_48()
                    .aria_label("Search items")
                    .disabled(self.busy),
            );
        }
        tools.children(self.trailing).children(self.tools)
    }
}

/// Item and column-header lanes share the same geometry. Row commands and
/// editors stay inside the name lane; optional columns do not resize that lane.
#[derive(IntoElement)]
pub struct BoardRow {
    leading: Option<AnyElement>,
    name: AnyElement,
    cells: Vec<AnyElement>,
    header: bool,
    touch: bool,
}

impl BoardRow {
    pub fn new(name: impl IntoElement) -> Self {
        Self {
            leading: None,
            name: name.into_any_element(),
            cells: Vec::new(),
            header: false,
            touch: false,
        }
    }
    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }
    pub fn cell(mut self, cell: impl IntoElement) -> Self {
        self.cells.push(cell.into_any_element());
        self
    }
    pub fn header(mut self) -> Self {
        self.header = true;
        self
    }
    pub fn touch(mut self) -> Self {
        self.touch = true;
        self
    }
}

impl RenderOnce for BoardRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let name = div()
            .min_w_0()
            .px_2()
            .flex()
            .items_center()
            .w(rems(22.5))
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(self.name);
        let width = rems(26. + 10. * self.cells.len() as f32);
        let mut row = div()
            .flex()
            .w_full()
            .min_w(width)
            .items_stretch()
            .min_h(if self.touch { rems(2.75) } else { rems(2.25) })
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .w(rems(3.5))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .children(self.leading),
            )
            .child(name);
        for cell in self.cells {
            row = row.child(
                div()
                    .w(rems(10.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .child(cell),
            );
        }
        if self.header {
            row = row.text_sm().text_color(cx.theme().muted_foreground);
        }
        row.child(div().flex_1().min_w_0())
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::{BoardFindDock, BoardHeader, BoardRow, BoardTabs, BoardToolbar};
    use gpui_kit::{
        AppContext, Bounds, Context, Entity, FocusHandle, IntoElement, MouseButton, ParentElement,
        Render, Styled, Subscription, TestAppContext, Window, WindowBounds, WindowOptions,
        base::Disableable,
        component::{
            button::Button,
            input::{InputEvent, InputState},
            tab::Tab,
        },
        div, px, size,
        test::{TestEventExt, TestQueryExt, TestWindowExt},
    };

    struct Board {
        search: Entity<InputState>,
        selected: usize,
        added: usize,
        disabled: bool,
    }

    struct FindDock {
        search: Entity<InputState>,
        focus: FocusHandle,
        outside: FocusHandle,
        focused: bool,
        disabled: bool,
        homes: usize,
        cancellations: usize,
        _events: Vec<Subscription>,
    }

    impl Render for FindDock {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let dock = BoardFindDock::new(&self.search)
                .track_focus(&self.focus)
                .focused(self.focused)
                .disabled(self.disabled)
                .clear(
                    Button::new("dock-clear").on_click(cx.listener(|dock, _, w, cx| {
                        dock.search.update(cx, |input, cx| {
                            input.set_value("", w, cx);
                            input.focus(w, cx);
                        });
                        cx.notify();
                    })),
                )
                .cancel(
                    Button::new("dock-cancel").on_click(cx.listener(|dock, _, w, cx| {
                        dock.search
                            .update(cx, |input, cx| input.set_value("", w, cx));
                        dock.focused = false;
                        dock.cancellations += 1;
                        w.focus(&dock.outside, cx);
                        cx.notify();
                    })),
                )
                .home(
                    Button::new("dock-home").on_click(cx.listener(|dock, _, _, cx| {
                        dock.homes += 1;
                        cx.notify();
                    })),
                );
            div().flex().flex_col().child(dock).child(
                Button::new("outside")
                    .label("Board")
                    .track_focus(&self.outside),
            )
        }
    }

    #[gpui_kit::test]
    fn native_find_dock_clear_cancel_and_home(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, dock) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(420.), px(140.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |w, cx| {
                    cx.new(|cx| {
                        let search = cx.new(|cx| InputState::new(w, cx).placeholder("Quick find"));
                        let focus = cx.focus_handle();
                        let outside = cx.focus_handle().tab_stop(true);
                        let input_events =
                            cx.subscribe(&search, |dock: &mut FindDock, _, event, cx| {
                                if matches!(event, InputEvent::Focus) {
                                    dock.focused = true;
                                    cx.notify();
                                }
                            });
                        let weak = cx.weak_entity();
                        let focus_events = w.on_focus_out(&focus, cx, move |_, _, cx| {
                            let _ = weak.update(cx, |dock, cx| {
                                dock.focused = false;
                                cx.notify();
                            });
                        });
                        FindDock {
                            search,
                            focus,
                            outside,
                            focused: false,
                            disabled: false,
                            homes: 0,
                            cancellations: 0,
                            _events: vec![input_events, focus_events],
                        }
                    })
                },
            )
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(window, |_, w, cx| {
            w.activate_window();
            w.render_frame(cx);
            w.click("dock-home", cx);
            assert_eq!(dock.read(cx).homes, 1);
            w.click("mobile-quick-find", cx);
            w.input("Ink", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, w, cx| {
            w.render_frame(cx);
            assert!(dock.read(cx).focused);
            assert!(w.find("dock-cancel").bounds().size.height >= px(44.));
            assert!(
                w.find("mobile-quick-find").bounds().right()
                    <= w.find("dock-cancel").bounds().left()
            );
            w.press("tab", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(w.find("dock-clear").focused(), Some(true));
            assert_eq!(w.find("dock-clear").label(), Some("Clear board filter"));
            assert!(w.try_find("dock-cancel").is_some());
            w.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let cancel_position = cx
            .update_window(window, |_, w, cx| {
                w.render_frame(cx);
                assert!(dock.read(cx).search.read(cx).value().is_empty());
                assert_eq!(w.find("mobile-quick-find").focused(), Some(true));
                assert!(w.try_find("dock-cancel").is_some());
                w.input("Ink", cx);
                let position = w.find("dock-cancel").bounds().center();
                w.pointer_down(position, MouseButton::Left, cx);
                position
            })
            .unwrap();
        // Let the input's blur and dock focus subscription run before pointer release.
        cx.run_until_parked();
        cx.update_window(window, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("dock-cancel").is_some());
            w.pointer_up(cancel_position, MouseButton::Left, cx);
            assert_eq!(dock.read(cx).cancellations, 1);
            assert!(dock.read(cx).search.read(cx).value().is_empty());
            assert_eq!(w.find("outside").focused(), Some(true));
            assert!(w.try_find("dock-cancel").is_none());
            w.click("mobile-quick-find", cx);
            w.input("Sketch", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, w, cx| {
            dock.update(cx, |dock, cx| {
                dock.disabled = true;
                cx.notify();
            });
            w.render_frame(cx);
            w.click("dock-clear", cx);
            assert_eq!(dock.read(cx).search.read(cx).value().as_ref(), "Sketch");
            w.click("dock-cancel", cx);
            assert_eq!(dock.read(cx).cancellations, 2);
            assert!(dock.read(cx).search.read(cx).value().is_empty());
            w.click("dock-home", cx);
            assert_eq!(dock.read(cx).homes, 2);
        })
        .unwrap();
    }

    impl Render for Board {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(BoardHeader::new("Roadmap"))
                .child(
                    BoardTabs::new()
                        .tab(Tab::new().label("Planning"))
                        .selected_index(self.selected)
                        .on_select(cx.listener(|board, index, _, cx| {
                            board.selected = *index;
                            cx.notify();
                        })),
                )
                .child(
                    BoardToolbar::new()
                        .new_item(Button::new("new-item").disabled(self.disabled).on_click(
                            cx.listener(|board, _, _, cx| {
                                board.added += 1;
                                cx.notify();
                            }),
                        ))
                        .search(&self.search, "search")
                        .search_disabled(self.disabled),
                )
                .child(
                    BoardRow::new(Button::new("header-name").label("Item").w_full())
                        .cell(Button::new("header-status").label("Status").w_full())
                        .header(),
                )
                .child(
                    BoardRow::new(Button::new("row-name").label("Review").w_full())
                        .cell(Button::new("row-status").label("Done").w_full()),
                )
        }
    }

    #[gpui_kit::test]
    fn shared_board_controls_and_column_lanes(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, board) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(900.), px(500.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |w, cx| {
                    cx.new(|cx| Board {
                        search: cx.new(|cx| InputState::new(w, cx)),
                        selected: 0,
                        added: 0,
                        disabled: false,
                    })
                },
            )
            .unwrap()
        });
        cx.update_window(window, |_, w, cx| {
            w.render_frame(cx);
            let header = w.find("header-name").bounds();
            let row = w.find("row-name").bounds();
            assert_eq!(header.left(), row.left());
            assert_eq!(header.size.width, row.size.width);
            assert_eq!(
                w.find("header-status").bounds().left(),
                w.find("row-status").bounds().left()
            );
            assert!(w.find("row-status").bounds().left() > row.right());
            w.click("new-item", cx);
            assert_eq!(board.read(cx).added, 1);
            let tab = w.find_by_label("Planning");
            w.click(tab.path().last().unwrap().clone(), cx);
            assert_eq!(board.read(cx).selected, 1);
            w.click("search", cx);
            w.input("Review", cx);
            assert_eq!(board.read(cx).search.read(cx).value().as_ref(), "Review");
            board.update(cx, |board, cx| {
                board.disabled = true;
                cx.notify();
            });
            w.render_frame(cx);
            w.click("new-item", cx);
            assert_eq!(board.read(cx).added, 1);
        })
        .unwrap();
    }
}
