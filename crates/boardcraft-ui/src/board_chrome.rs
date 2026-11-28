//! The same board title, view tabs, tools and cell lanes on every Rust client.
//! Callers retain data and commands; these components own the visible structure.
use gpui_kit::{
    AnyElement, App, ElementId, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    component::{
        ActiveTheme, IconName, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        tab::{Tab, TabBar},
    },
    div, rems,
};

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
    use super::{BoardHeader, BoardRow, BoardTabs, BoardToolbar};
    use gpui_kit::{
        AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Styled,
        TestAppContext, Window, WindowBounds, WindowOptions,
        base::Disableable,
        component::{button::Button, input::InputState, tab::Tab},
        div, px, size,
        test::{TestQueryExt, TestWindowExt},
    };

    struct Board {
        search: Entity<InputState>,
        selected: usize,
        added: usize,
        disabled: bool,
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
