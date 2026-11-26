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
    /// Append roots in request order using the shared batch operation. Submit once;
    /// an uncertain acknowledgment requires an authorized read before another write.
    pub fn move_main_table_selection(
        &self,
        board: Uuid,
        operation: Uuid,
        group: &str,
        board_revision: i64,
        items: &[(Uuid, i64)],
    ) -> Result<(), EditError> {
        #[derive(Serialize)]
        struct Root {
            id: Uuid,
            revision: i64,
        }
        #[derive(Serialize)]
        struct Draft<'a> {
            operation_id: Uuid,
            group_id: &'a str,
            board_revision: i64,
            items: Vec<Root>,
        }
        #[derive(Deserialize)]
        struct Saved {
            board_id: Uuid,
            operation_id: Uuid,
            affected_ids: Vec<Uuid>,
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/items/move-selection",
            self.port
        );
        let saved: Saved = decode(
            self.authorize(self.http.post(url)).send_json(Draft {
                operation_id: operation,
                group_id: group,
                board_revision,
                items: items
                    .iter()
                    .map(|&(id, revision)| Root { id, revision })
                    .collect(),
            }),
        )?;
        let affected: HashSet<_> = saved.affected_ids.iter().copied().collect();
        if saved.board_id != board
            || saved.operation_id != operation
            || saved.affected_ids.len() != items.len()
            || affected.len() != items.len()
            || items.iter().any(|(id, _)| !affected.contains(id))
        {
            return Err(EditError::Transport);
        }
        Ok(())
    }

    /// Native selected-board create. Never replay automatically after a failed acknowledgment.
    pub fn create_main_table_item(
        &self,
        board: Uuid,
        group: &str,
        id: Uuid,
        title: &str,
    ) -> Result<SavedItem, EditError> {
        #[derive(Serialize)]
        struct Draft<'a> {
            id: Uuid,
            group_id: &'a str,
            title: &'a str,
        }
        #[derive(Deserialize)]
        struct Created {
            group_id: String,
            #[serde(flatten)]
            item: SavedItem,
        }
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/items",
            self.port
        );
        let saved: Created = decode(self.authorize(self.http.post(url)).send_json(Draft {
            id,
            group_id: group,
            title,
        }))?;
        if saved.group_id != group || saved.item.id != id || saved.item.revision < 1 {
            return Err(EditError::Transport);
        }
        Ok(saved.item)
    }

    pub fn rename_main_table_item(
        &self,
        board: Uuid,
        item: Uuid,
        edit: &NameEdit,
    ) -> Result<SavedItem, EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/main-table/items/{item}/name",
            self.port
        );
        let saved: SavedItem = decode(self.authorize(self.http.put(url)).send_json(edit))?;
        if saved.id != item || saved.revision < edit.revision || saved.revision < 1 {
            return Err(EditError::Transport);
        }
        Ok(saved)
    }

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
