use super::*;
use std::collections::HashSet;

/// Bounded server inventory. Baselines and eligibility never come from the grid.
#[derive(Deserialize)]
pub struct ArchiveRead {
    board_revision: i64,
    can_manage: bool,
    total: i64,
    has_more: bool,
    items: Vec<ArchivedItem>,
    groups: Vec<ArchivedGroup>,
    next_group_offset: Option<i64>,
}
#[derive(Deserialize)]
pub struct ArchivedItem {
    id: Uuid,
    title: String,
    group_name: String,
    revision: i64,
    state: i32,
    effective_state: i32,
    restorable: bool,
    group_active: bool,
}
#[derive(Deserialize)]
pub struct ArchivedGroup {
    id: String,
    name: String,
    revision: i64,
}
impl ArchiveRead {
    pub fn revision(&self) -> i64 {
        self.board_revision
    }
    pub fn is_manageable(&self) -> bool {
        self.can_manage
    }
    pub fn total(&self) -> i64 {
        self.total
    }
    pub fn has_more(&self) -> bool {
        self.has_more
    }
    pub fn items(&self) -> &[ArchivedItem] {
        &self.items
    }
    pub fn groups(&self) -> &[ArchivedGroup] {
        &self.groups
    }
    pub fn next_group_offset(&self) -> Option<i64> {
        self.next_group_offset
    }
}
impl ArchivedItem {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn group_name(&self) -> &str {
        &self.group_name
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub fn is_restorable(&self) -> bool {
        self.restorable
    }
    pub fn is_group_active(&self) -> bool {
        self.group_active
    }
}
impl ArchivedGroup {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
}

impl EditClient {
    /// One explicit selected-root command; recovery reads do not call this method.
    pub fn archive_main_table_selection(
        &self,
        board: Uuid,
        operation: Uuid,
        board_revision: i64,
        items: &[(Uuid, i64)],
    ) -> Result<(), EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/items/archive-selection",
            self.port
        );
        let draft = serde_json::json!({"operation_id":operation,"board_revision":board_revision,
            "items":items.iter().map(|(id,revision)|serde_json::json!({"id":id,"revision":revision})).collect::<Vec<_>>()});
        let saved: serde_json::Value =
            decode(self.authorize(self.http.post(url)).send_json(draft))?;
        let affected: Vec<Uuid> = serde_json::from_value(saved["affected_ids"].clone())
            .map_err(|_| EditError::Transport)?;
        let expected: HashSet<_> = items.iter().map(|(id, _)| *id).collect();
        if saved["operation_id"] != serde_json::json!(operation)
            || saved["board_id"] != serde_json::json!(board)
            || affected.len() != items.len()
            || affected.iter().copied().collect::<HashSet<_>>() != expected
        {
            return Err(EditError::Transport);
        }
        Ok(())
    }
    pub fn read_main_table_archive(
        &self,
        board: Uuid,
        cursor: Option<(i64, i64)>,
    ) -> Result<ArchiveRead, EditError> {
        let mut url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/archive",
            self.port
        );
        if let Some((revision, offset)) = cursor {
            url += &format!("/groups/{revision}/{offset}");
        }
        let read: ArchiveRead = decode(self.authorize(self.http.get(url)).call())?;
        let mut items = HashSet::new();
        let mut groups = HashSet::new();
        if read.board_revision < 1
            || read.total < read.items.len() as i64
            || read.items.len() > 100
            || read.groups.len() > 100
            || read.has_more != (read.total > read.items.len() as i64)
            || cursor.is_some_and(|(revision, _)| revision != read.board_revision)
            || read
                .next_group_offset
                .is_some_and(|n| Some(n) != cursor.map_or(Some(100), |(_, n)| n.checked_add(100)))
            || read.items.iter().any(|r| {
                r.id.is_nil()
                    || !items.insert(r.id)
                    || r.revision < 1
                    || r.effective_state != 1
                    || ![0, 1, 3].contains(&r.state)
                    || (r.group_active && r.state != 1)
                    || r.restorable != (read.can_manage && r.group_active)
            })
            || read
                .groups
                .iter()
                .any(|g| g.id.trim().is_empty() || g.revision < 1 || !groups.insert(&g.id))
        {
            return Err(EditError::Transport);
        }
        Ok(read)
    }

    /// One explicit command. A lost acknowledgment is not permission to replay.
    pub fn archive_main_table_item(
        &self,
        board: Uuid,
        item: Uuid,
        operation: Uuid,
        board_revision: i64,
        item_revision: i64,
        restore: bool,
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Draft {
            operation_id: Uuid,
            board_revision: i64,
            item_revision: i64,
        }
        #[derive(Deserialize)]
        struct Saved {
            operation_id: Uuid,
            item_id: Uuid,
            board_id: Uuid,
        }
        let action = if restore { "restore" } else { "archive" };
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/items/{item}/{action}",
            self.port
        );
        let saved: Saved = decode(self.authorize(self.http.post(url)).send_json(Draft {
            operation_id: operation,
            board_revision,
            item_revision,
        }))?;
        if saved.board_id != board || saved.item_id != item || saved.operation_id != operation {
            return Err(EditError::Transport);
        }
        Ok(())
    }

    /// Undo and recovery Restore share this board-baselined group operation.
    pub fn archive_main_table_group(
        &self,
        board: Uuid,
        group: &str,
        board_revision: i64,
        restore: bool,
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Draft {
            board_revision: i64,
        }
        #[derive(Deserialize)]
        struct Saved {
            board_id: Uuid,
            group_id: String,
            board_revision: i64,
        }
        let action = if restore { "restore" } else { "archive" };
        let group_path: String = group.bytes().map(|b| format!("%{b:02X}")).collect();
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/groups/{group_path}/{action}",
            self.port
        );
        let saved: Saved = decode(
            self.authorize(self.http.post(url))
                .send_json(Draft { board_revision }),
        )?;
        if saved.board_id != board
            || saved.group_id != group
            || board_revision.checked_add(1) != Some(saved.board_revision)
        {
            return Err(EditError::Transport);
        }
        Ok(())
    }
}
