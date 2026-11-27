//! Transport-neutral commands for the existing selected-board HTTP surface.
use super::{EditError, ItemReorder, Uuid};
use serde_json::{Value, json};

/// One explicit command. The caller owns baselines and never retries an uncertain result.
pub enum TableCommand {
    GroupFields {
        id: String,
        revision: Option<i64>,
        board_revision: i64,
        name: String,
        colour: String,
    },
    GroupOrder {
        board_revision: i64,
        ids: Vec<String>,
    },
    Fold {
        view_revision: i64,
        ids: Vec<String>,
        collapsed: bool,
    },
    Create {
        id: Uuid,
        group: String,
        title: String,
    },
    Move {
        id: Uuid,
        group: String,
        revision: i64,
    },
    Reorder {
        id: Uuid,
        command: ItemReorder,
    },
    Selection {
        operation: Uuid,
        board_revision: i64,
        items: Vec<(Uuid, i64)>,
        action: SelectionAction,
    },
    Root {
        id: Uuid,
        operation: Uuid,
        board_revision: i64,
        revision: i64,
        action: RootAction,
    },
    Group {
        id: String,
        board_revision: i64,
        restore: bool,
    },
}
/// Batch commands deliberately distinguish append from lifecycle changes.
pub enum SelectionAction {
    Move(String),
    Archive,
    Delete,
}
/// A restore identifies the inventory it came from.
pub enum RootAction {
    Archive,
    Restore,
    RestoreDeleted,
}
impl RootAction {
    fn path(&self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Restore => "restore",
            Self::RestoreDeleted => "restore-deleted",
        }
    }
}
impl TableCommand {
    pub fn method(&self) -> &'static str {
        if matches!(
            self,
            Self::GroupFields {
                revision: Some(_),
                ..
            } | Self::Move { .. }
        ) {
            "PUT"
        } else {
            "POST"
        }
    }
    pub fn path(&self) -> String {
        let suffix = match self {
            Self::GroupFields {
                id,
                revision: Some(_),
                ..
            } => format!("groups/{}", segment(id)),
            Self::GroupFields { .. } => "groups".into(),
            Self::GroupOrder { .. } => "group-order".into(),
            Self::Fold { .. } => return "/main-table-fold".into(),
            Self::Create { .. } => "items".into(),
            Self::Move { id, .. } => format!("items/{id}/group"),
            Self::Reorder { id, .. } => format!("items/{id}/reorder"),
            Self::Selection { action, .. } => format!(
                "items/{}",
                match action {
                    SelectionAction::Move(_) => "move-selection",
                    SelectionAction::Archive => "archive-selection",
                    SelectionAction::Delete => "delete-selection",
                }
            ),
            Self::Root { id, action, .. } => format!("items/{id}/{}", action.path()),
            Self::Group { id, restore, .. } => format!(
                "groups/{}/{}",
                segment(id),
                if *restore { "restore" } else { "archive" }
            ),
        };
        format!("/main-table/{suffix}")
    }
    pub fn body(&self) -> Value {
        match self {
            Self::GroupFields {
                id,
                revision,
                board_revision,
                name,
                colour,
            } => {
                let mut body = json!({"board_revision":board_revision,"name":name,"colour":colour});
                if let Some(revision) = revision {
                    body["group_revision"] = json!(revision);
                } else {
                    body["id"] = json!(id);
                }
                body
            }
            Self::GroupOrder {
                board_revision,
                ids,
            } => json!({"board_revision":board_revision,"group_ids":ids}),
            Self::Fold {
                view_revision,
                ids,
                collapsed,
            } => json!({"view_revision":view_revision,"group_ids":ids,"collapsed":collapsed}),
            Self::Create { id, group, title } => json!({"id":id,"group_id":group,"title":title}),
            Self::Move {
                id: _,
                group,
                revision,
            } => json!({"group_id":group,"expected_revision":revision}),
            Self::Reorder { command, .. } => {
                serde_json::to_value(command).expect("serializable wire command")
            }
            Self::Selection {
                operation,
                board_revision,
                items,
                action,
            } => {
                let mut body = json!({"operation_id":operation,"board_revision":board_revision,"items":items.iter().map(|(id,revision)|json!({"id":id,"revision":revision})).collect::<Vec<_>>()});
                if let SelectionAction::Move(group) = action {
                    body["group_id"] = json!(group);
                }
                body
            }
            Self::Root {
                operation,
                board_revision,
                revision,
                ..
            } => {
                json!({"operation_id":operation,"board_revision":board_revision,"item_revision":revision})
            }
            Self::Group { board_revision, .. } => json!({"board_revision":board_revision}),
        }
    }
    /// Validate command-bound identity before the caller reads accepted display state.
    pub fn acknowledge(&self, board: Uuid, saved: &Value) -> Result<(), EditError> {
        let valid = match self {
            Self::Create { id, group, .. } | Self::Move { id, group, .. } => {
                saved["id"] == json!(id)
                    && saved["group_id"] == *group
                    && saved["revision"].as_i64().is_some_and(|r| r > 0)
            }
            _ if saved["board_id"] != json!(board) => false,
            Self::GroupFields { id, .. } => saved["group_id"] == *id,
            Self::Group {
                id, board_revision, ..
            } => {
                saved["group_id"] == *id
                    && saved["board_revision"].as_i64() == board_revision.checked_add(1)
            }
            Self::GroupOrder { board_revision, .. } => {
                saved["board_revision"].as_i64() == board_revision.checked_add(1)
            }
            Self::Fold { view_revision, .. } => {
                saved["view_revision"].as_i64() == view_revision.checked_add(1)
            }
            Self::Reorder { id, command } => {
                saved["item_id"] == json!(id)
                    && saved["operation_id"] == self.body()["operation_id"]
                    && saved["operation_id"]
                        == serde_json::to_value(command).expect("wire command")["operation_id"]
            }
            Self::Root { id, operation, .. } => {
                saved["item_id"] == json!(id) && saved["operation_id"] == json!(operation)
            }
            Self::Selection {
                operation, items, ..
            } => {
                let affected = saved["affected_ids"].as_array();
                saved["operation_id"] == json!(operation)
                    && affected.is_some_and(|ids| {
                        ids.len() == items.len()
                            && items
                                .iter()
                                .all(|(id, _)| ids.iter().filter(|v| **v == json!(id)).count() == 1)
                    })
            }
        };
        if valid {
            Ok(())
        } else {
            Err(EditError::Transport)
        }
    }
}
fn segment(id: &str) -> String {
    id.bytes().map(|b| format!("%{b:02X}")).collect()
}
