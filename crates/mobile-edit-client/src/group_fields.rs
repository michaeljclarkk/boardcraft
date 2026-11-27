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
#[derive(Deserialize)]
struct Saved {
    board_id: Uuid,
    group_id: String,
}
impl Saved {
    fn check(self, board: Uuid, id: &str) -> Result<(), EditError> {
        if self.board_id != board || self.group_id != id {
            return Err(EditError::Transport);
        }
        Ok(())
    }
}
impl EditClient {
    /// Submit once, then read the authoritative table. No automatic retries.
    pub fn create_main_table_group(
        &self,
        board: Uuid,
        id: &str,
        fields: &GroupFields<'_>,
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Draft<'a, 'b> {
            id: &'a str,
            #[serde(flatten)]
            fields: &'a GroupFields<'b>,
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/groups",
            self.port
        );
        let saved: Saved = decode(
            self.authorize(self.http.post(url))
                .send_json(Draft { id, fields }),
        )?;
        saved.check(board, id)
    }
    /// Existing IDs are board-local, not necessarily UUIDs. Both baselines guard Save.
    pub fn edit_main_table_group(
        &self,
        board: Uuid,
        id: &str,
        group_revision: i64,
        fields: &GroupFields<'_>,
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Draft<'a, 'b> {
            group_revision: i64,
            #[serde(flatten)]
            fields: &'a GroupFields<'b>,
        }
        let path: String = id.bytes().map(|b| format!("%{b:02X}")).collect();
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/groups/{path}",
            self.port
        );
        let saved: Saved = decode(self.authorize(self.http.put(url)).send_json(Draft {
            group_revision,
            fields,
        }))?;
        saved.check(board, id)
    }
}
