//! Reusable native interaction behavior. No domain, storage or service dependencies.
mod board_chrome;
mod heading_drag;
pub use board_chrome::{BoardHeader, BoardRow, BoardTabs, BoardToolbar};
pub use heading_drag::{HeadingDrag, SectionBounds};
