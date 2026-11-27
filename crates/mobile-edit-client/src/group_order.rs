use super::*;

impl EditClient {
    /// Submit one complete physical order at the accepted board revision. Never retries.
    pub fn reorder_main_table_groups(
        &self,
        board: Uuid,
        board_revision: i64,
        group_ids: &[String],
    ) -> Result<(), EditError> {
        self.submit_table(
            board,
            TableCommand::GroupOrder {
                board_revision,
                ids: group_ids.to_vec(),
            },
        )
    }
}
