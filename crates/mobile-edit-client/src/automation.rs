//! Automation list/create shared by the connected clients.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    pub id: Uuid,
    pub name: String,
    pub trigger: String,
    pub action: String,
    pub enabled: bool,
    pub last_item_id: Option<Uuid>,
    pub last_item_title: Option<String>,
    pub last_ran_at: Option<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct NewAutomation<'a> {
    id: Uuid,
    name: &'a str,
}

#[cfg(not(target_family = "wasm"))]
impl super::EditClient {
    pub fn list_automations(&self, board: Uuid) -> Result<Vec<Automation>, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automations",
            self.port
        );
        super::decode(self.authorize(self.http.get(url)).call())
    }

    pub fn create_automation(
        &self,
        board: Uuid,
        id: Uuid,
        name: &str,
    ) -> Result<Automation, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automations",
            self.port
        );
        super::decode(
            self.authorize(self.http.post(url))
                .send_json(NewAutomation { id, name }),
        )
    }
}
