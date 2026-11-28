//! Recipient inbox shared by the connected clients. The host resolves the
//! recipient from the session, so no method takes one.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InboxNotification {
    pub id: Uuid,
    pub board_id: Uuid,
    pub item_id: Uuid,
    pub automation_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub message: String,
    pub board_name: String,
    pub item_name: String,
    pub automation_name: Option<String>,
    pub created_at: String,
    pub read_at: Option<String>,
    pub seen_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationInbox {
    pub unread: i64,
    pub notifications: Vec<InboxNotification>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ReadDraft {
    read: bool,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SeenDraft<'a> {
    ids: &'a [Uuid],
}

#[cfg(not(target_family = "wasm"))]
impl super::EditClient {
    pub fn list_notifications(
        &self,
        unread_only: bool,
    ) -> Result<NotificationInbox, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/notifications{}",
            self.port,
            if unread_only { "?unread=1" } else { "" }
        );
        super::decode(self.authorize(self.http.get(url)).call())
    }

    pub fn set_notification_read(
        &self,
        id: Uuid,
        read: bool,
    ) -> Result<InboxNotification, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/notifications/{id}/read",
            self.port
        );
        super::decode(
            self.authorize(self.http.post(url))
                .send_json(ReadDraft { read }),
        )
    }

    pub fn mark_notifications_seen(&self, ids: &[Uuid]) -> Result<bool, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/notifications/seen",
            self.port
        );
        super::decode(
            self.authorize(self.http.post(url))
                .send_json(SeenDraft { ids }),
        )
    }

    pub fn mark_all_notifications_read(&self) -> Result<bool, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/notifications/read-all",
            self.port
        );
        super::decode(self.authorize(self.http.post(url)).send_json(&()))
    }

    pub fn delete_notification(&self, id: Uuid) -> Result<bool, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/notifications/{id}",
            self.port
        );
        super::decode(self.authorize(self.http.delete(url)).call())
    }
}
