//! People directory reads shared by the desktop window and the browser shell.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub active: bool,
    pub teams: Vec<String>,
}

#[cfg(not(target_family = "wasm"))]
impl super::EditClient {
    /// Blank lists the whole directory; the host applies the same substring rule.
    pub fn list_users(&self, query: &str) -> Result<Vec<DirectoryUser>, super::EditError> {
        let url = if query.is_empty() {
            format!("http://127.0.0.1:{}/navigation/users", self.port)
        } else {
            format!(
                "http://127.0.0.1:{}/navigation/users?q={}",
                self.port,
                encode(query)
            )
        };
        super::decode(self.authorize(self.http.get(url)).call())
    }
}

#[cfg(not(target_family = "wasm"))]
fn encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
