//! Same-origin transport; the host alone holds the account bearer.
use boardcraft_mobile_edit_client::{
    ArchiveRead, BOARD_ACCESS_HEADER, EditError, MainTableGroup, MainTableRead, NameEdit,
    SavedItem, StatusCell, StatusEdit, StatusTable, TableCommand,
};
use futures::{FutureExt, future::Either};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCredentials, RequestInit, RequestMode, RequestRedirect, Response};

#[cfg(target_family = "wasm")]
use boardcraft_mobile_edit_client::{BoardSummary, Credentials};
#[cfg(target_family = "wasm")]
pub(super) enum Resolution {
    Accepted,
    Rejected,
    Unavailable,
}
struct AbortOnDrop(web_sys::AbortController);
struct Metadata {
    editable: bool,
    manageable: bool,
    revision: Option<i64>,
    view_revision: Option<i64>,
}
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn request<T: DeserializeOwned>(
    binding: &str,
    method: &str,
    path: &str,
    body: Option<String>,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<(T, Metadata), EditError> {
    let controller = web_sys::AbortController::new().map_err(|_| EditError::Transport)?;
    let _abort = AbortOnDrop(controller.clone());
    let work = async {
        let options = RequestInit::new();
        options.set_method(method);
        options.set_credentials(RequestCredentials::SameOrigin);
        options.set_mode(RequestMode::SameOrigin);
        options.set_redirect(RequestRedirect::Error);
        options.set_signal(Some(&controller.signal()));
        if let Some(body) = body {
            options.set_body(&JsValue::from_str(&body));
        }
        let request = Request::new_with_str_and_init(&format!("/web/api{path}"), &options)
            .map_err(|_| EditError::Transport)?;
        request
            .headers()
            .set("Boardcraft-Web", binding)
            .map_err(|_| EditError::Transport)?;
        if method != "GET" {
            request
                .headers()
                .set("Content-Type", "application/json")
                .map_err(|_| EditError::Transport)?;
        }
        let window = web_sys::window().ok_or(EditError::Transport)?;
        let response: Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| EditError::Transport)?
            .dyn_into()
            .map_err(|_| EditError::Transport)?;
        match response.status() {
            200..=299 => (),
            401 => return Err(EditError::SessionRejected),
            403 => return Err(EditError::PermissionDenied),
            404 => return Err(EditError::Missing),
            409 => return Err(EditError::Conflict),
            422 => return Err(EditError::InvalidDraft),
            503 => return Err(EditError::AccessUnavailable),
            _ => return Err(EditError::HostFault),
        }
        let editable = response
            .headers()
            .get(BOARD_ACCESS_HEADER)
            .ok()
            .flatten()
            .as_deref()
            == Some("edit");
        let positive = |key| {
            response
                .headers()
                .get(key)
                .ok()
                .flatten()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
        };
        let metadata = Metadata {
            editable,
            manageable: response
                .headers()
                .get("boardcraft-root-lifecycle")
                .ok()
                .flatten()
                .as_deref()
                == Some("owner"),
            revision: positive("boardcraft-board-revision"),
            view_revision: positive("boardcraft-view-revision"),
        };
        let text = JsFuture::from(response.text().map_err(|_| EditError::Transport)?)
            .await
            .map_err(|_| EditError::Transport)?
            .as_string()
            .ok_or(EditError::Transport)?;
        serde_json::from_str(&text)
            .map(|value| (value, metadata))
            .map_err(|_| EditError::Transport)
    }
    .boxed_local();
    match futures::future::select(
        work,
        executor
            .timer(std::time::Duration::from_secs(12))
            .boxed_local(),
    )
    .await
    {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(EditError::Transport),
    }
}

fn body(value: &impl Serialize) -> Result<Option<String>, EditError> {
    serde_json::to_string(value)
        .map(Some)
        .map_err(|_| EditError::Transport)
}

pub(super) async fn read_status(
    binding: &str,
    board: Uuid,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<StatusTable, EditError> {
    request(
        binding,
        "GET",
        &format!("/navigation/status/{board}"),
        None,
        executor,
    )
    .await
    .map(|(table, _)| table)
}

pub(super) async fn set_status(
    binding: &str,
    board: Uuid,
    column: Uuid,
    item: Uuid,
    edit: &StatusEdit,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<StatusCell, EditError> {
    let (saved, _) = request::<StatusCell>(
        binding,
        "PUT",
        &format!("/navigation/status/{board}/{column}/{item}"),
        body(edit)?,
        executor,
    )
    .await?;
    if saved.item_id != item || saved.revision < edit.revision || saved.value != edit.value {
        return Err(EditError::Transport);
    }
    Ok(saved)
}

#[cfg(target_family = "wasm")]
pub(super) async fn resolve(binding: &str, executor: &gpui_kit::BackgroundExecutor) -> Resolution {
    match request::<serde_json::Value>(binding, "GET", "/session", None, executor).await {
        Ok(_) => Resolution::Accepted,
        Err(EditError::SessionRejected) => Resolution::Rejected,
        Err(_) => Resolution::Unavailable,
    }
}
#[cfg(target_family = "wasm")]
pub(super) async fn sign_in(
    credentials: &Credentials,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<String, EditError> {
    let (binding, _) =
        request::<String>("login", "POST", "/session", body(credentials)?, executor).await?;
    Uuid::parse_str(&binding).map_err(|_| EditError::Transport)?;
    Ok(binding)
}
#[cfg(target_family = "wasm")]
pub(super) async fn viewer_board(
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<Option<Uuid>, EditError> {
    use boardcraft_mobile_edit_client::{DISPOSABLE_VIEWER, VIEWER_BOARD_PATH};
    match request(DISPOSABLE_VIEWER, "GET", VIEWER_BOARD_PATH, None, executor).await {
        Ok((board, _)) => Ok(Some(board)),
        Err(EditError::Missing) => Ok(None),
        Err(error) => Err(error),
    }
}
#[cfg(target_family = "wasm")]
pub(super) async fn logout(
    binding: &str,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<(), EditError> {
    request::<serde_json::Value>(binding, "POST", "/session/logout", None, executor)
        .await
        .map(|_| ())
}
#[cfg(target_family = "wasm")]
pub(super) async fn list(
    binding: &str,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<Vec<BoardSummary>, EditError> {
    request(binding, "GET", "/navigation/boards", None, executor)
        .await
        .map(|(rows, _)| rows)
}
pub(super) async fn name_for_retry(
    binding: &str,
    board: Uuid,
    item: Uuid,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<SavedItem, EditError> {
    let (row, meta) = request::<SavedItem>(
        binding,
        "GET",
        &format!("/navigation/boards/{board}/items/{item}/name"),
        None,
        executor,
    )
    .await?;
    if !meta.editable {
        return Err(EditError::PermissionDenied);
    }
    if row.id != item || row.revision < 1 {
        return Err(EditError::Transport);
    }
    Ok(row)
}
pub(super) async fn table(
    binding: &str,
    board: Uuid,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<MainTableRead, EditError> {
    let (groups, meta) = request::<Vec<MainTableGroup>>(
        binding,
        "GET",
        &format!("/navigation/boards/{board}"),
        None,
        executor,
    )
    .await?;
    MainTableRead::from_response(
        groups,
        meta.revision.ok_or(EditError::Transport)?,
        meta.view_revision.ok_or(EditError::Transport)?,
        meta.editable,
        meta.manageable,
    )
}
pub(super) async fn command(
    binding: &str,
    board: Uuid,
    command: &TableCommand,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<(), EditError> {
    let (saved, _) = request::<serde_json::Value>(
        binding,
        command.method(),
        &format!("/navigation/boards/{board}{}", command.path()),
        body(&command.body())?,
        executor,
    )
    .await?;
    command.acknowledge(board, &saved)
}
pub(super) async fn recovery(
    binding: &str,
    board: Uuid,
    trash: bool,
    cursor: Option<(i64, i64)>,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<ArchiveRead, EditError> {
    let kind = if trash { "trash" } else { "archive" };
    let mut path = format!("/navigation/boards/{board}/main-table/{kind}");
    if let Some((revision, offset)) = cursor {
        path += &format!("/groups/{revision}/{offset}");
    }
    let (read, _) = request::<ArchiveRead>(binding, "GET", &path, None, executor).await?;
    read.checked(trash, cursor)
}
pub(super) async fn rename(
    binding: &str,
    board: Uuid,
    item: Uuid,
    edit: &NameEdit,
    executor: &gpui_kit::BackgroundExecutor,
) -> Result<SavedItem, EditError> {
    request(
        binding,
        "PUT",
        &format!("/navigation/boards/{board}/items/{item}/name"),
        body(edit)?,
        executor,
    )
    .await
    .map(|(row, _)| row)
}
