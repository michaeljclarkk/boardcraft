use super::*;

/// One deliberate relative drop, captured from the accepted table snapshot.
#[derive(Clone, Serialize)]
pub struct ItemReorder {
    operation_id: Uuid,
    group_id: String,
    anchor_id: Uuid,
    side: Side,
    board_revision: i64,
    item_revision: i64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Before,
    After,
}

impl ItemReorder {
    pub fn new(
        operation: Uuid,
        group: impl Into<String>,
        anchor: Uuid,
        after: bool,
        board_revision: i64,
        item_revision: i64,
    ) -> Self {
        Self {
            operation_id: operation,
            group_id: group.into(),
            anchor_id: anchor,
            side: if after { Side::After } else { Side::Before },
            board_revision,
            item_revision,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl EditClient {
    /// Save just the group field. Unlike toolbar append, this preserves position
    /// and creation order, and does not create a reorder receipt. Never retries.
    pub fn move_main_table_item(
        &self,
        board: Uuid,
        item: Uuid,
        group: &str,
        expected_revision: i64,
    ) -> Result<SavedItem, EditError> {
        #[derive(Deserialize)]
        struct Saved {
            group_id: String,
            #[serde(flatten)]
            item: SavedItem,
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/items/{item}/group",
            self.port
        );
        let command = TableCommand::Move {
            id: item,
            group: group.into(),
            revision: expected_revision,
        };
        let saved: Saved = decode(self.authorize(self.http.put(url)).send_json(command.body()))?;
        if saved.group_id != group
            || saved.item.id != item
            || saved.item.revision < expected_revision
            || saved.item.revision < 1
        {
            return Err(EditError::Transport);
        }
        Ok(saved.item)
    }

    /// Submit once; only a subsequent authorized read supplies accepted order.
    pub fn reorder_main_table_item(
        &self,
        board: Uuid,
        item: Uuid,
        command: &ItemReorder,
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::Reorder {
                id: item,
                command: command.clone(),
            },
        )
    }
}
