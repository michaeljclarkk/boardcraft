//! Blocking loopback client. Call from a worker when loading or saving an editor.
#[cfg(not(target_family = "wasm"))]
use std::time::Duration;

#[cfg(not(target_family = "wasm"))]
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
#[cfg(not(target_family = "wasm"))]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod activity;
mod archive;
mod automation;
mod directory;
mod group_fields;
#[cfg(not(target_family = "wasm"))]
mod group_order;
mod item_placement;
mod main_table;
mod notifications;
mod realtime;
mod status;
mod table_command;
mod table_contract;
mod views;
pub use activity::ActivityEntry;
pub use archive::{ArchiveRead, ArchivedGroup, ArchivedItem};
pub use automation::{
    AutomationAction, AutomationActionGroup, AutomationActionKind, AutomationCatalog,
    AutomationChoice, AutomationColumn, AutomationCondition, AutomationConditionMode,
    AutomationConditionOperator, AutomationDateBasis, AutomationDefinition, AutomationHistory,
    AutomationItem, AutomationOutcome, AutomationPerson, AutomationRecipient, AutomationRule,
    AutomationRun, AutomationTrigger, AutomationTriggerKind, RuleDisposition, SaveAutomationRule,
};
pub use directory::DirectoryUser;
pub use group_fields::GroupFields;
pub use item_placement::ItemReorder;
pub use main_table::MainTableRead;
pub use notifications::{InboxNotification, NotificationInbox};
#[cfg(not(target_family = "wasm"))]
pub use realtime::ChangeFeed;
pub use realtime::{ChangeEvent, ChangeKind};
pub use status::{StatusCell, StatusChoice, StatusEdit, StatusLabelEdit, StatusLabels, StatusTable};
pub use table_command::{RootAction, SelectionAction, TableCommand};
pub use table_contract::MainTableGroup;
pub use views::{SavedView, ViewFilters};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedItem {
    pub id: Uuid,
    pub title: String,
    pub revision: i64,
}

pub const BOARD_ACCESS_HEADER: &str = "boardcraft-board-access";

/// One case-insensitive item-title filter rule shared by desktop, browser and
/// Android. The stored filter is data, never a pattern.
pub fn view_filter_matches(filter: &str, title: &str) -> bool {
    let filter = filter.trim();
    filter.is_empty() || title.to_lowercase().contains(&filter.to_lowercase())
}

/// Public request label, not a credential. Only the disposable loopback host accepts it.
pub const DISPOSABLE_VIEWER: &str = "disposable-viewer";
pub const VIEWER_BOARD_PATH: &str = "/viewer-board";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardSummary {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
}

pub struct BoardRead {
    pub rows: Vec<SavedItem>,
    pub can_edit: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameEdit {
    pub title: String,
    pub revision: i64,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditError {
    SessionRejected,
    PermissionDenied,
    AccessUnavailable,
    InvalidDraft,
    Conflict,
    Missing,
    HostFault,
    Transport,
}

/// Wire credentials have no Debug implementation, and are sent only in the POST body.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub identifier: String,
    pub password: String,
}

#[cfg(not(target_family = "wasm"))]
#[derive(Clone)]
pub struct EditClient {
    port: u16,
    http: ureq::Agent,
    authorization: Option<ureq::http::HeaderValue>,
}

#[cfg(not(target_family = "wasm"))]
impl EditClient {
    fn submit_table(&self, board: Uuid, command: TableCommand) -> Result<(), EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}{}",
            self.port,
            command.path()
        );
        let saved: serde_json::Value = if command.method() == "PUT" {
            decode(self.authorize(self.http.put(url)).send_json(command.body()))?
        } else {
            decode(
                self.authorize(self.http.post(url))
                    .send_json(command.body()),
            )?
        };
        command.acknowledge(board, &saved)
    }
    pub fn new(port: u16) -> Self {
        let config = ureq::Agent::config_builder()
            .proxy(None)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(10)))
            .build();
        Self {
            port,
            http: config.into(),
            authorization: None,
        }
    }

    /// Only a resolved server-side session grants access; the client supplies no role.
    pub fn with_bearer(port: u16, encoded: &str) -> Result<Self, EditError> {
        if encoded.len() != 43 || URL_SAFE_NO_PAD.decode_slice(encoded, &mut [0; 32]).is_err() {
            return Err(EditError::SessionRejected);
        }
        let mut header = ureq::http::HeaderValue::from_str(&format!("Bearer {encoded}"))
            .map_err(|_| EditError::SessionRejected)?;
        header.set_sensitive(true);
        let mut client = Self::new(port);
        client.authorization = Some(header);
        Ok(client)
    }

    fn authorize<B>(&self, request: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        match &self.authorization {
            Some(value) => request.header("Authorization", value.clone()),
            None => request,
        }
    }

    fn board_url(&self, board: Uuid) -> String {
        format!("http://127.0.0.1:{}/boards/{board}", self.port)
    }

    pub fn sign_in(port: u16, credentials: Credentials) -> Result<Self, EditError> {
        Self::sign_in_with_storage(port, credentials, |_| Ok(()))
    }

    /// Give the origin's store one opportunity to persist the validated bearer
    /// at issuance. A client is never returned if persistence fails.
    pub fn sign_in_with_storage<E: From<EditError>>(
        port: u16,
        credentials: Credentials,
        store: impl FnOnce(&str) -> Result<(), E>,
    ) -> Result<Self, E> {
        let client = Self::new(port);
        let url = format!("http://127.0.0.1:{port}/session");
        let bearer: String = decode(client.http.post(url).send_json(credentials))?;
        let client = Self::with_bearer(port, &bearer)?;
        if let Err(error) = store(&bearer) {
            // The host already committed issuance. Don't leave an unreachable
            // session active if the host can still acknowledge revocation.
            let _ = client.logout();
            return Err(error);
        }
        Ok(client)
    }

    /// Resolve the issued session before opening a connected view. The board
    /// routes still independently check the current session and board grant.
    pub fn resolve(&self) -> Result<(), EditError> {
        let url = format!("http://127.0.0.1:{}/session", self.port);
        decode(self.authorize(self.http.get(url)).call())
    }

    /// Revoke just this bearer. The view drops its client only after acknowledgment.
    pub fn logout(&self) -> Result<(), EditError> {
        let url = format!("http://127.0.0.1:{}/session/logout", self.port);
        // Check current connectivity, not a pooled stream through an old bridge.
        let http = Self::new(self.port).http;
        decode(self.authorize(http.post(url)).send_empty())
    }

    pub fn list_boards(&self) -> Result<Vec<BoardSummary>, EditError> {
        let url = format!("http://127.0.0.1:{}/navigation/boards", self.port);
        decode(self.authorize(self.http.get(url)).call())
    }

    pub fn read(&self, board: Uuid) -> Result<Vec<SavedItem>, EditError> {
        self.read_with_access(board).map(|read| read.rows)
    }

    pub fn read_selected_board(&self, board: Uuid) -> Result<Vec<SavedItem>, EditError> {
        let url = format!("http://127.0.0.1:{}/navigation/boards/{board}", self.port);
        decode(self.authorize(self.http.get(url)).call())
    }

    pub fn read_with_access(&self, board: Uuid) -> Result<BoardRead, EditError> {
        let mut response = self
            .authorize(self.http.get(self.board_url(board)))
            .call()
            .map_err(request_error)?;
        Ok(BoardRead {
            can_edit: edit_hint(response.headers()),
            rows: response
                .body_mut()
                .read_json()
                .map_err(|_| EditError::Transport)?,
        })
    }

    pub fn rename(&self, board: Uuid, item: Uuid, edit: &NameEdit) -> Result<SavedItem, EditError> {
        let url = format!("{}/items/{item}/name", self.board_url(board));
        decode(self.authorize(self.http.put(url)).send_json(edit))
    }
}

#[cfg(not(target_family = "wasm"))]
fn edit_hint(headers: &ureq::http::HeaderMap) -> bool {
    let mut hints = headers.get_all(BOARD_ACCESS_HEADER).iter();
    hints.next().is_some_and(|v| v == "edit") && hints.next().is_none()
}

#[cfg(not(target_family = "wasm"))]
fn decode<T: DeserializeOwned>(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<T, EditError> {
    response
        .map_err(request_error)?
        .body_mut()
        .read_json()
        .map_err(|_| EditError::Transport)
}

#[cfg(not(target_family = "wasm"))]
fn request_error(error: ureq::Error) -> EditError {
    match error {
        ureq::Error::StatusCode(401) => EditError::SessionRejected,
        ureq::Error::StatusCode(403) => EditError::PermissionDenied,
        ureq::Error::StatusCode(503) => EditError::AccessUnavailable,
        ureq::Error::StatusCode(404) => EditError::Missing,
        ureq::Error::StatusCode(409) => EditError::Conflict,
        ureq::Error::StatusCode(422) => EditError::InvalidDraft,
        ureq::Error::StatusCode(_) => EditError::HostFault,
        _ => EditError::Transport,
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn editing_requires_one_recognized_hint() {
        use ureq::http::{HeaderMap, HeaderValue};
        let mut headers = HeaderMap::new();
        assert!(!edit_hint(&headers));
        headers.insert(BOARD_ACCESS_HEADER, HeaderValue::from_static("unknown"));
        assert!(!edit_hint(&headers));
        headers.insert(BOARD_ACCESS_HEADER, HeaderValue::from_static("edit"));
        assert!(edit_hint(&headers));
        headers.append(BOARD_ACCESS_HEADER, HeaderValue::from_static("read"));
        assert!(!edit_hint(&headers));
    }
}
