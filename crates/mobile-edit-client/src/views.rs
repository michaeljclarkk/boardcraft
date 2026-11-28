//! Saved board views shared by the connected clients.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The quick-filter choices shared by native and browser saved views.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewFilters {
    pub status_values: Vec<String>,
    pub include_empty_status: bool,
    pub group_ids: Vec<String>,
}

impl ViewFilters {
    pub fn matches(&self, status: Option<&str>, group: &str) -> bool {
        (self.group_ids.is_empty() || self.group_ids.iter().any(|id| id == group))
            && if self.include_empty_status {
                status.is_none()
            } else {
                self.status_values.is_empty()
                    || match status {
                        Some(value) => self.status_values.iter().any(|key| key == value),
                        None => false,
                    }
            }
    }
}

#[cfg(test)]
mod tests {
    use super::ViewFilters;

    #[test]
    fn quick_filters_replace_labels_with_empty_and_intersect_groups() {
        let mut filters = ViewFilters::default();
        assert!(filters.matches(None, "main"));
        assert!(filters.matches(Some("done"), "main"));
        filters.status_values.push("stuck".into());
        assert!(filters.matches(Some("stuck"), "main"));
        assert!(!filters.matches(None, "main"));
        filters.include_empty_status = true;
        filters.status_values.clear();
        assert!(filters.matches(None, "main"));
        assert!(!filters.matches(Some("stuck"), "main"));
        assert!(!filters.matches(Some("done"), "main"));
        filters.group_ids.push("main".into());
        assert!(!filters.matches(Some("stuck"), "other"));
        assert!(!filters.matches(None, "other"));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub filter: String,
    pub shared: bool,
    pub owner: Uuid,
    #[serde(default)]
    pub filters: ViewFilters,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct NewView<'a> {
    id: Uuid,
    name: &'a str,
    filter: &'a str,
    filters: &'a ViewFilters,
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
        self.create_view_with_filters(board, id, name, filter, &ViewFilters::default())
    }

    pub fn create_view_with_filters(
        &self,
        board: Uuid,
        id: Uuid,
        name: &str,
        filter: &str,
        filters: &ViewFilters,
    ) -> Result<SavedView, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/views",
            self.port
        );
        super::decode(self.authorize(self.http.post(url)).send_json(NewView {
            id,
            name,
            filter,
            filters,
        }))
    }
}
