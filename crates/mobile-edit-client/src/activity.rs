use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One captured item change. `occurred_at` is an ISO timestamp in UTC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityEntry {
    pub board_id: Uuid,
    pub item_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub occurred_at: String,
    pub field: String,
    pub before: String,
    pub after: String,
    pub revision: i64,
}

#[cfg(not(target_family = "wasm"))]
impl super::EditClient {
    /// Fetch this item's changes using the current session, newest first.
    pub fn item_activity(
        &self,
        board: Uuid,
        item: Uuid,
    ) -> Result<Vec<ActivityEntry>, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/activity/{board}/{item}",
            self.port
        );
        let entries: Vec<ActivityEntry> = super::decode(self.authorize(self.http.get(url)).call())?;
        if entries
            .iter()
            .any(|entry| entry.board_id != board || entry.item_id != item)
        {
            return Err(super::EditError::HostFault);
        }
        Ok(entries)
    }
}
