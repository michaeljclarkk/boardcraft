//! Saved board views shared by the connected clients.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub filter: String,
    pub shared: bool,
    pub owner: Uuid,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct NewView<'a> {
    id: Uuid,
    name: &'a str,
    filter: &'a str,
}

#[cfg(not(target_family = "wasm"))]
impl super::EditClient {
    pub fn list_views(&self, board: Uuid) -> Result<Vec<SavedView>, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/views",
            self.port
        );
        super::decode(self.authorize(self.http.get(url)).call())
    }

    /// Saving is personal in this slice; sharing is a separate later action.
    pub fn create_view(
        &self,
        board: Uuid,
        id: Uuid,
        name: &str,
        filter: &str,
    ) -> Result<SavedView, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/views",
            self.port
        );
        super::decode(
            self.authorize(self.http.post(url))
                .send_json(NewView { id, name, filter }),
        )
    }
}
