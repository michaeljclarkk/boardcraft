use super::*;

/// Raw fields and the accepted board baseline for one deliberate group command.
#[derive(Serialize)]
pub struct GroupFields<'a> {
    board_revision: i64,
    name: &'a str,
    colour: &'a str,
}
impl<'a> GroupFields<'a> {
    pub fn new(board_revision: i64, name: &'a str, colour: &'a str) -> Self {
        Self {
            board_revision,
            name,
            colour,
        }
    }
}
#[cfg(not(target_family = "wasm"))]
impl EditClient {
    /// Submit once, then read the authoritative table. No automatic retries.
    pub fn create_main_table_group(
        &self,
        board: Uuid,
        id: &str,
        fields: &GroupFields<'_>,
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::GroupFields {
                id: id.into(),
                revision: None,
                board_revision: fields.board_revision,
                name: fields.name.into(),
                colour: fields.colour.into(),
            },
        )
    }
    /// Existing IDs are board-local, not necessarily UUIDs. Both baselines guard Save.
    pub fn edit_main_table_group(
        &self,
        board: Uuid,
        id: &str,
        group_revision: i64,
        fields: &GroupFields<'_>,
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::GroupFields {
                id: id.into(),
                revision: Some(group_revision),
                board_revision: fields.board_revision,
                name: fields.name.into(),
                colour: fields.colour.into(),
            },
        )
    }
}
