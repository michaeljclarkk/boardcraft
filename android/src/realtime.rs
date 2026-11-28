//! Accepted board changes pushed by the host. One feed serves a signed-in
//! session; its worker thread blocks on the stream and the board view pumps the
//! channel, so neither the subscription nor the row read waits on the renderer.
use super::navigation::Page;
use super::*;
use boardcraft_mobile_edit_client::ChangeEvent;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};
use std::time::Duration;

/// A feed subscription owned by the board view. Dropping it asks the worker to
/// stop after its next event or keep-alive.
pub(super) struct Changes {
    stop: Arc<AtomicBool>,
}

impl Changes {
    fn same(&self, stop: &Arc<AtomicBool>) -> bool {
        Arc::ptr_eq(&self.stop, stop)
    }
}

impl Drop for Changes {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Open the feed once, off the renderer. A failed subscribe or a closed stream
/// leaves the feed stopped; this slice does not reconnect or catch up.
pub(super) fn open(client: EditClient) -> (Receiver<ChangeEvent>, Changes) {
    let (sender, events) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    std::thread::spawn(move || {
        let Ok(mut feed) = client.subscribe_changes() else {
            return;
        };
        while !worker_stop.load(Ordering::Relaxed) {
            match feed.next() {
                Ok(event) => {
                    if sender.send(event).is_err() {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });
    (events, Changes { stop })
}

impl BoardView {
    /// The board the screen shows. The login, board list, inbox and error
    /// screens have none, so their events are dropped.
    fn displayed_board(&self) -> Option<Uuid> {
        if self.login.is_some() {
            return None;
        }
        match &self.navigation {
            Some(Page::Selected { board, .. }) => Some(*board),
            None => self.connection.and_then(|conn| conn.board),
            _ => None,
        }
    }

    /// Starts the session's one feed when a board is first shown, then keeps it
    /// across board changes. The pump stops when this feed is replaced or the
    /// view is gone.
    pub(super) fn ensure_changes(&mut self, cx: &mut Context<Self>) {
        if self.changes.is_some() || self.logging_out || self.displayed_board().is_none() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let (events, changes) = realtime::open(client.clone());
        let stop = changes.stop.clone();
        self.changes = Some(changes);
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let drained = view.update(cx, |view, _| {
                    view.changes
                        .as_ref()
                        .filter(|changes| changes.same(&stop))?;
                    let mut batch = Vec::new();
                    let mut disconnected = false;
                    loop {
                        match events.try_recv() {
                            Ok(event) => {
                                if view.displayed_board() == Some(event.board_id) {
                                    batch.push(event);
                                }
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                disconnected = true;
                                break;
                            }
                        }
                    }
                    Some((batch, disconnected))
                });
                let Ok(Some((batch, disconnected))) = drained else {
                    break;
                };
                for event in batch {
                    // The feed names the row; its content comes from the
                    // ordinary authorized read, never from the stream.
                    let (board, item) = (event.board_id, event.item_id);
                    let reader = client.clone();
                    let Ok(saved) = cx
                        .background_executor()
                        .spawn(async move { reader.read_main_table_item(board, item) })
                        .await
                    else {
                        continue;
                    };
                    _ = view.update(cx, |view, cx| {
                        if !view
                            .changes
                            .as_ref()
                            .is_some_and(|changes| changes.same(&stop))
                            || view.displayed_board() != Some(board)
                        {
                            return;
                        }
                        if view.apply_accepted(saved) {
                            cx.notify();
                        }
                    });
                }
                if disconnected {
                    break;
                }
            }
        })
        .detach();
    }

    /// Applies one accepted row read. A replayed or late read never moves a row
    /// backwards, and an open editor keeps its draft.
    fn apply_accepted(&mut self, saved: SavedItem) -> bool {
        match &mut self.navigation {
            Some(Page::Selected { rows, .. }) => {
                let Some(row) = rows.iter_mut().find(|row| row.id == saved.id) else {
                    return false;
                };
                if saved.revision <= row.revision {
                    return false;
                }
                row.title = saved.title;
                row.revision = saved.revision;
                true
            }
            Some(_) => false,
            None => {
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.id == saved.id)
                {
                    return false;
                }
                let Some(item) = self.board.items.iter_mut().find(|item| item.id == saved.id)
                else {
                    return false;
                };
                match item.revision {
                    Some(revision) if saved.revision > revision => {
                        item.title = saved.title;
                        item.revision = Some(saved.revision);
                        true
                    }
                    _ => false,
                }
            }
        }
    }
}
