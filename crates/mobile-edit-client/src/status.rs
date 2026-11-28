use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(not(target_family = "wasm"))]
use crate::{EditClient, EditError, decode};

/// The board's Status column, ordered palette and active item cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusTable {
    pub column_id: Uuid,
    pub title: String,
    pub choices: Vec<StatusChoice>,
    pub cells: Vec<StatusCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusChoice {
    pub key: String,
    pub label: String,
    pub color: String,
}

/// A cell has its own revision, independent of the item's name revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusCell {
    pub item_id: Uuid,
    pub revision: i64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub value: Option<String>,
}

/// A clear is explicitly `null`; an absent value is a malformed request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusEdit {
    pub revision: i64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub value: Option<String>,
}

#[cfg(not(target_family = "wasm"))]
impl EditClient {
    pub fn read_status(&self, board: Uuid) -> Result<StatusTable, EditError> {
        let url = format!("http://127.0.0.1:{}/navigation/status/{board}", self.port);
        decode(self.authorize(self.http.get(url)).call())
    }

    /// Submit once. An uncertain response needs an authorized read, never a replay.
    pub fn set_status(
        &self,
        board: Uuid,
        column: Uuid,
        item: Uuid,
        revision: i64,
        value: Option<String>,
    ) -> Result<StatusCell, EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/status/{board}/{column}/{item}",
            self.port
        );
        let edit = StatusEdit { revision, value };
        let saved: StatusCell = decode(self.authorize(self.http.put(url)).send_json(&edit))?;
        if saved.item_id != item || saved.revision < revision || saved.value != edit.value {
            return Err(EditError::Transport);
        }
        Ok(saved)
    }
}

#[cfg(test)]
mod tests {
    use super::StatusEdit;
    use serde_json::{from_value, json, to_value};

    #[test]
    fn clearing_requires_an_explicit_null() {
        let clear = json!({"revision": 3, "value": null});
        let edit: StatusEdit = from_value(clear.clone()).unwrap();
        assert_eq!(to_value(edit).unwrap(), clear);
        assert!(from_value::<StatusEdit>(json!({"revision": 3})).is_err());
    }
}
