use super::{Deserialize, SavedItem};

/// Shared selected-board group response for native and wasm transports.
#[derive(Deserialize)]
pub struct MainTableGroup {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) colour: String,
    pub(crate) revision: i64,
    pub(crate) collapsed: bool,
    pub(crate) items: Vec<SavedItem>,
}

impl MainTableGroup {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn colour(&self) -> &str {
        &self.colour
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    pub fn items(&self) -> &[SavedItem] {
        &self.items
    }
}
