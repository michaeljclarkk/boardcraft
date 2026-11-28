//! Case-insensitive substring filtering over the already-authorized board list.

use boardcraft_mobile_edit_client::BoardSummary;
use uuid::Uuid;

/// Keep host order; scope to `workspace` when set; an empty query keeps every board.
pub(crate) fn filter_workspace_boards<'a>(
    boards: &'a [BoardSummary],
    workspace: Option<Uuid>,
    query: &str,
) -> Vec<&'a BoardSummary> {
    let query = query.to_lowercase();
    boards
        .iter()
        .filter(|board| workspace.is_none_or(|id| board.workspace_id == id))
        .filter(|board| board.name.to_lowercase().contains(&query))
        .collect()
}
