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

impl StatusChoice {
    /// The configured default label describes a cleared cell.
    pub fn value(&self) -> Option<String> {
        (self.key != "empty").then(|| self.key.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusLabels {
    pub actor_id: Uuid,
    pub column_id: Uuid,
    pub choices: Vec<StatusChoice>,
    pub can_edit: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusLabelEdit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
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
    pub fn read_status_labels(&self, board: Uuid, column: Uuid) -> Result<StatusLabels, EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/status/{board}/{column}/labels",
            self.port
        );
        let labels: StatusLabels = decode(self.authorize(self.http.get(url)).call())?;
        if labels.column_id != column {
            return Err(EditError::Transport);
        }
        Ok(labels)
    }

    /// Submit a partial patch once. Read configuration after an uncertain reply.
    pub fn save_status_label(
        &self,
        board: Uuid,
        column: Uuid,
        key: &str,
        edit: &StatusLabelEdit,
    ) -> Result<StatusChoice, EditError> {
        // Product-generated keys contain only path-safe ASCII.
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(EditError::InvalidDraft);
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/status/{board}/{column}/labels/{key}",
            self.port
        );
        let saved: StatusChoice = decode(self.authorize(self.http.put(url)).send_json(edit))?;
        if saved.key != key
            || edit.label.as_ref().is_some_and(|s| s != &saved.label)
            || edit.color.as_ref().is_some_and(|s| s != &saved.color)
        {
            return Err(EditError::Transport);
        }
        Ok(saved)
    }

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
    use super::{StatusChoice, StatusEdit, StatusLabelEdit};
    use serde_json::{from_value, json, to_value};

    #[test]
    fn clearing_requires_an_explicit_null() {
        let clear = json!({"revision": 3, "value": null});
        let edit: StatusEdit = from_value(clear.clone()).unwrap();
        assert_eq!(to_value(edit).unwrap(), clear);
        assert!(from_value::<StatusEdit>(json!({"revision": 3})).is_err());
    }

    #[test]
    fn label_patch_omits_untouched_fields_and_default_maps_to_clear() {
        let patch: StatusLabelEdit = from_value(json!({"label":""})).unwrap();
        assert_eq!(to_value(patch).unwrap(), json!({"label":""}));
        assert!(from_value::<StatusLabelEdit>(json!({"position":2})).is_err());
        let choice = StatusChoice {
            key: "empty".into(),
            label: "Unassigned".into(),
            color: "#123456".into(),
        };
        assert_eq!(choice.value(), None);
    }
}
