//! Blocking loopback client. Call from a worker when loading or saving an editor.
use std::time::Duration;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedItem {
    pub id: Uuid,
    pub title: String,
    pub revision: i64,
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

pub struct EditClient {
    port: u16,
    http: ureq::Agent,
    authorization: Option<ureq::http::HeaderValue>,
}

impl EditClient {
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
        if encoded.len() != 43
            || !encoded
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
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

    pub fn read(&self, board: Uuid) -> Result<Vec<SavedItem>, EditError> {
        decode(self.authorize(self.http.get(self.board_url(board))).call())
    }

    pub fn rename(&self, board: Uuid, item: Uuid, edit: &NameEdit) -> Result<SavedItem, EditError> {
        let url = format!("{}/items/{item}/name", self.board_url(board));
        decode(self.authorize(self.http.put(url)).send_json(edit))
    }
}

fn decode<T: DeserializeOwned>(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<T, EditError> {
    let mut response = response.map_err(|error| match error {
        ureq::Error::StatusCode(401) => EditError::SessionRejected,
        ureq::Error::StatusCode(403) => EditError::PermissionDenied,
        ureq::Error::StatusCode(503) => EditError::AccessUnavailable,
        ureq::Error::StatusCode(404) => EditError::Missing,
        ureq::Error::StatusCode(409) => EditError::Conflict,
        ureq::Error::StatusCode(422) => EditError::InvalidDraft,
        ureq::Error::StatusCode(_) => EditError::HostFault,
        _ => EditError::Transport,
    })?;
    response
        .body_mut()
        .read_json()
        .map_err(|_| EditError::Transport)
}
