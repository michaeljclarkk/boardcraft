use super::*;
use std::collections::HashSet;

/// Complete authorized snapshot, independent of whether a body is folded.
pub struct MainTableRead {
    groups: Vec<MainTableGroup>,
    revision: i64,
    view_revision: i64,
    editable: bool,
}

#[derive(Deserialize)]
pub struct MainTableGroup {
    id: String,
    name: String,
    collapsed: bool,
    items: Vec<SavedItem>,
}

impl MainTableGroup {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    pub fn items(&self) -> &[SavedItem] {
        &self.items
    }
}

impl MainTableRead {
    pub fn groups(&self) -> &[MainTableGroup] {
        &self.groups
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub fn view_revision(&self) -> i64 {
        self.view_revision
    }
    pub fn is_editable(&self) -> bool {
        self.editable
    }
}

fn revision(headers: &ureq::http::HeaderMap, key: &str) -> Result<i64, EditError> {
    let mut values = headers.get_all(key).iter();
    let value = values
        .next()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .ok_or(EditError::Transport)?;
    if values.next().is_some() {
        return Err(EditError::Transport);
    }
    Ok(value)
}

impl EditClient {
    pub fn read_main_table(&self, board: Uuid) -> Result<MainTableRead, EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table",
            self.port
        );
        let mut response = self
            .authorize(self.http.get(url))
            .call()
            .map_err(request_error)?;
        let board_revision = revision(response.headers(), "boardcraft-board-revision")?;
        let view_revision = revision(response.headers(), "boardcraft-view-revision")?;
        let editable = edit_hint(response.headers());
        let groups: Vec<MainTableGroup> = response
            .body_mut()
            .read_json()
            .map_err(|_| EditError::Transport)?;
        let mut ids = HashSet::new();
        let mut items = HashSet::new();
        if groups.iter().map(|g| g.items.len().max(1)).sum::<usize>() > 500
            || groups.iter().any(|g| {
                g.id.trim().is_empty()
                    || !ids.insert(&g.id)
                    || g.items
                        .iter()
                        .any(|r| r.id.is_nil() || r.revision < 1 || !items.insert(r.id))
            })
        {
            return Err(EditError::Transport);
        }
        Ok(MainTableRead {
            groups,
            revision: board_revision,
            view_revision,
            editable,
        })
    }

    /// Submit once. A lost acknowledgment must be recovered by reading, not replaying.
    pub fn set_main_table_fold(
        &self,
        board: Uuid,
        view_revision: i64,
        ids: &[String],
        collapsed: bool,
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Draft<'a> {
            view_revision: i64,
            group_ids: &'a [String],
            collapsed: bool,
        }
        #[derive(Deserialize)]
        struct Saved {
            board_id: Uuid,
            view_revision: i64,
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table-fold",
            self.port
        );
        let saved: Saved = decode(self.authorize(self.http.post(url)).send_json(Draft {
            view_revision,
            group_ids: ids,
            collapsed,
        }))?;
        if saved.board_id != board || view_revision.checked_add(1) != Some(saved.view_revision) {
            return Err(EditError::Transport);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_requires_one_positive_header() {
        use ureq::http::{HeaderMap, HeaderValue};
        let key = "boardcraft-view-revision";
        let mut headers = HeaderMap::new();
        assert_eq!(revision(&headers, key), Err(EditError::Transport));
        for value in ["0", "-1", "unknown"] {
            headers.insert(key, HeaderValue::from_static(value));
            assert_eq!(revision(&headers, key), Err(EditError::Transport));
        }
        headers.insert(key, HeaderValue::from_static("2"));
        assert_eq!(revision(&headers, key), Ok(2));
        headers.append(key, HeaderValue::from_static("2"));
        assert_eq!(revision(&headers, key), Err(EditError::Transport));
    }
}
