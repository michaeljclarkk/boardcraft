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
    /// Check the same inventory contract for native and asynchronous transports.
    pub fn checked(self, trash: bool, cursor: Option<(i64, i64)>) -> Result<Self, EditError> {
        let mut items = HashSet::new();
        let mut groups = HashSet::new();
        if self.board_revision < 1
            || self.total < self.items.len() as i64
            || self.items.len() > 100
            || self.groups.len() > 100
            || self.has_more != (self.total > self.items.len() as i64)
            || cursor.is_some_and(|(revision, _)| revision != self.board_revision)
            || self
                .next_group_offset
                .is_some_and(|n| Some(n) != cursor.map_or(Some(100), |(_, n)| n.checked_add(100)))
            || self.items.iter().any(|r| {
                r.id.is_nil()
                    || !items.insert(r.id)
                    || r.revision < 1
                    || r.effective_state != if trash { 2 } else { 1 }
                    || !(if trash {
                        &[0, 1, 2, 3][..]
                    } else {
                        &[0, 1, 3][..]
                    })
                    .contains(&r.state)
                    || (r.group_active && r.state != if trash { 2 } else { 1 })
                    || r.restorable != (self.can_manage && r.group_active)
            })
            || self
                .groups
                .iter()
                .any(|g| g.id.trim().is_empty() || g.revision < 1 || !groups.insert(&g.id))
        {
            return Err(EditError::Transport);
        }
        Ok(self)
    }
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

#[cfg(not(target_family = "wasm"))]
impl EditClient {
    /// One explicit selected-root command; recovery reads do not call this method.
    pub fn archive_main_table_selection(
        &self,
        board: Uuid,
        operation: Uuid,
        board_revision: i64,
        items: &[(Uuid, i64)],
    ) -> Result<(), EditError> {
        self.lifecycle_selection(board, operation, board_revision, items, false)
    }
    pub fn delete_main_table_selection(
        &self,
        board: Uuid,
        operation: Uuid,
        board_revision: i64,
        items: &[(Uuid, i64)],
    ) -> Result<(), EditError> {
        self.lifecycle_selection(board, operation, board_revision, items, true)
    }
    fn lifecycle_selection(
        &self,
        board: Uuid,
        operation: Uuid,
        board_revision: i64,
        items: &[(Uuid, i64)],
        trash: bool,
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::Selection {
                operation,
                board_revision,
                items: items.to_vec(),
                action: if trash {
                    SelectionAction::Delete
                } else {
                    SelectionAction::Archive
                },
            },
        )
    }
    pub fn read_main_table_archive(
        &self,
        board: Uuid,
        cursor: Option<(i64, i64)>,
    ) -> Result<ArchiveRead, EditError> {
        self.read_recovery(board, cursor, false)
    }
    pub fn read_main_table_trash(&self, board: Uuid) -> Result<ArchiveRead, EditError> {
        self.read_recovery(board, None, true)
    }
    fn read_recovery(
        &self,
        board: Uuid,
        cursor: Option<(i64, i64)>,
        trash: bool,
    ) -> Result<ArchiveRead, EditError> {
        let kind = if trash { "trash" } else { "archive" };
        let mut url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/{kind}",
            self.port
        );
        if let Some((revision, offset)) = cursor {
            url += &format!("/groups/{revision}/{offset}");
        }
        let read: ArchiveRead = decode(self.authorize(self.http.get(url)).call())?;
        read.checked(trash, cursor)
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
        self.lifecycle_item(
            board,
            item,
            operation,
            board_revision,
            item_revision,
            if restore { "restore" } else { "archive" },
        )
    }
    pub fn restore_main_table_deleted_item(
        &self,
        board: Uuid,
        item: Uuid,
        operation: Uuid,
        board_revision: i64,
        item_revision: i64,
    ) -> Result<(), EditError> {
        self.lifecycle_item(
            board,
            item,
            operation,
            board_revision,
            item_revision,
            "restore-deleted",
        )
    }
    fn lifecycle_item(
        &self,
        board: Uuid,
        item: Uuid,
        operation: Uuid,
        board_revision: i64,
        item_revision: i64,
        action: &str,
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::Root {
                id: item,
                operation,
                board_revision,
                revision: item_revision,
                action: match action {
                    "archive" => RootAction::Archive,
                    "restore-deleted" => RootAction::RestoreDeleted,
                    _ => RootAction::Restore,
                },
            },
        )
    }

    /// Undo and recovery Restore share this board-baselined group operation.
    pub fn archive_main_table_group(
        &self,
        board: Uuid,
        group: &str,
        board_revision: i64,
        restore: bool,
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::Group {
                id: group.into(),
                board_revision,
                restore,
            },
        )
    }
}
