use super::*;

impl EditClient {
    /// Submit one complete physical order at the accepted board revision. Never retries.
    pub fn reorder_main_table_groups(
        &self,
        board: Uuid,
        board_revision: i64,
        group_ids: &[String],
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Draft<'a> {
            board_revision: i64,
            group_ids: &'a [String],
        }
        #[derive(Deserialize)]
        struct Saved {
            board_id: Uuid,
            board_revision: i64,
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/group-order",
            self.port
        );
        let saved: Saved = decode(self.authorize(self.http.post(url)).send_json(Draft {
            board_revision,
            group_ids,
        }))?;
        if saved.board_id != board || board_revision.checked_add(1) != Some(saved.board_revision) {
            return Err(EditError::Transport);
        }
        Ok(())
    }
}
