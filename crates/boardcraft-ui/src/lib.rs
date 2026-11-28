//! Reusable native interaction behavior. No domain, storage or service dependencies.
mod board_chrome;
mod heading_drag;
pub use board_chrome::{
    BoardFindDock, BoardHeader, BoardRow, BoardSelectionBar, BoardTabs, BoardToolbar,
};
pub use heading_drag::{HeadingDrag, SectionBounds};

/// The reference desktop's default Night palette (MobileTheme.Night).
/// Clients call this once after Kit initialization; Android retains its native theme.
pub fn apply_board_theme(cx: &mut gpui_kit::App) {
    use gpui_kit::{
        component::{Theme, ThemeMode},
        rgb,
    };
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        let background = rgb(0x111111).into();
        let surface = rgb(0x2c2c2c).into();
        let raised = rgb(0x3b3b3b).into();
        let foreground = rgb(0xeeeeee).into();
        let secondary = rgb(0xaaaaaa).into();
        let outline = rgb(0x636363).into();
        let accent = rgb(0x0073ea).into();
        let selected = rgb(0x133774).into();
        theme.background = background;
        theme.foreground = foreground;
        theme.border = outline;
        theme.input = outline;
        theme.muted = surface;
        theme.muted_foreground = secondary;
        theme.group_box = surface;
        theme.group_box_foreground = foreground;
        theme.popover = background;
        theme.popover_foreground = foreground;
        theme.sidebar = surface;
        theme.sidebar_foreground = foreground;
        theme.sidebar_border = outline;
        theme.sidebar_accent = raised;
        theme.sidebar_accent_foreground = foreground;
        theme.accent = raised;
        theme.accent_foreground = foreground;
        theme.button = background;
        theme.button_foreground = foreground;
        theme.button_hover = raised;
        theme.button_active = surface;
        theme.primary = accent;
        theme.primary_foreground = rgb(0xffffff).into();
        theme.primary_hover = rgb(0x0060b9).into();
        theme.primary_active = rgb(0x0056a8).into();
        theme.button_primary = theme.primary;
        theme.button_primary_foreground = theme.primary_foreground;
        theme.button_primary_hover = theme.primary_hover;
        theme.button_primary_active = theme.primary_active;
        theme.secondary = surface;
        theme.secondary_foreground = foreground;
        theme.secondary_hover = raised;
        theme.secondary_active = surface;
        theme.selection = selected;
        theme.ring = accent;
        theme.caret = foreground;
        theme.link = rgb(0x86b7ff).into();
        theme.colors.list = background;
        theme.list_hover = raised;
        theme.list_active = selected;
        theme.list_active_border = accent;
        theme.tab_bar = background;
        theme.tab = background;
        theme.tab_active = background;
        theme.tab_active_foreground = foreground;
        theme.tab_foreground = secondary;
    });
}
