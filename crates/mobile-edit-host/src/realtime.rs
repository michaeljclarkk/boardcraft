//! Accepted-change feed. The host assigns one sequence number under a lock,
//! retains a short history for a client that connects after the change, and
//! hands each connection its own reader. An event names the changed record
//! only; a client reads the record through the ordinary authorized route.
use crate::access;
use boardcraft_board::{BoardId, ItemId, store::StoredItem};
use boardcraft_mobile_edit_client::{ChangeEvent, ChangeKind, SavedItem};
use postgres::{Client, Transaction};
use std::collections::VecDeque;
use std::io::Read;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, StatusCode};
use uuid::Uuid;

const HISTORY: usize = 256;
const HEARTBEAT: Duration = Duration::from_secs(10);
/// tiny_http's chunked encoder buffers 8 KiB before it writes. A padded
/// comment line after the event pushes the data line through immediately;
/// SSE readers ignore comment lines.
const FLUSH: usize = 9000;

struct State {
    seq: u64,
    history: VecDeque<ChangeEvent>,
    subscribers: Vec<Sender<ChangeEvent>>,
}

pub(super) struct Hub {
    state: Arc<Mutex<State>>,
}

impl Hub {
    pub(super) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                seq: 0,
                history: VecDeque::with_capacity(HISTORY),
                subscribers: Vec::new(),
            })),
        }
    }

    /// Publish only after the transaction committed. A subscriber can never
    /// observe a change that the store rolled back.
    pub(super) fn publish_rename(&self, board: Uuid, item: Uuid) {
        let mut state = self.state.lock().expect("event hub");
        state.seq += 1;
        let event = ChangeEvent {
            seq: state.seq,
            board_id: board,
            item_id: item,
            kind: ChangeKind::ItemRenamed,
        };
        state.history.push_back(event.clone());
        while state.history.len() > HISTORY {
            state.history.pop_front();
        }
        state
            .subscribers
            .retain(|subscriber| subscriber.send(event.clone()).is_ok());
    }

    fn subscribe(&self) -> (VecDeque<ChangeEvent>, Receiver<ChangeEvent>) {
        let (sender, receiver) = mpsc::channel();
        let mut state = self.state.lock().expect("event hub");
        state.subscribers.push(sender);
        (state.history.clone(), receiver)
    }
}

/// Validate the bearer, register the reader, then hand the request to a thread.
/// The accept loop stays free to serve board requests while the feed is open.
pub(super) fn route(
    db: &mut Client,
    req: Request,
    path: &str,
    actor: Result<access::Actor, u16>,
    hub: &Hub,
) {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let status = match subscribe(db, path, query, actor) {
        Ok(()) => {
            let (history, changes) = hub.subscribe();
            let stream = EventStream {
                changes,
                history,
                buffer: Vec::new(),
                position: 0,
            };
            let response = Response::new(
                StatusCode(200),
                vec![
                    Header::from_bytes("Content-Type", "text/event-stream").expect("header"),
                    Header::from_bytes("Cache-Control", "no-store").expect("header"),
                ],
                stream,
                None,
                None,
            );
            std::thread::spawn(move || {
                let _ = req.respond(response);
            });
            return;
        }
        Err(status) => status,
    };
    let _ = req.respond(Response::empty(status));
}

fn subscribe(
    db: &mut Client,
    path: &str,
    query: &str,
    actor: Result<access::Actor, u16>,
) -> Result<(), u16> {
    if path != "/navigation/events" || !query.is_empty() {
        return Err(404);
    }
    let actor = actor?;
    let mut tx = db.transaction().map_err(unavailable)?;
    actor.resolve(&mut tx)?;
    tx.commit().map_err(unavailable)
}

/// Read exactly the row named by an event through the ordinary board
/// authorization. This keeps a row refresh independent of table size.
pub(super) fn read_item(
    db: &mut Client,
    req: Request,
    path: &str,
    actor: Result<access::Actor, u16>,
) {
    let method = req.method().clone();
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let result = item_body(db, &method, path, query, actor);
    let response = match result {
        Ok(body) => Response::from_data(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").expect("header")),
        Err(status) => Response::from_data(Vec::new()).with_status_code(status),
    }
    .with_header(Header::from_bytes("Cache-Control", "no-store").expect("header"));
    let _ = req.respond(response);
}

fn item_body(
    db: &mut Client,
    method: &Method,
    path: &str,
    query: &str,
    actor: Result<access::Actor, u16>,
) -> Result<Vec<u8>, u16> {
    if method != &Method::Get || !query.is_empty() {
        return Err(404);
    }
    let path: Vec<_> = path.split('/').collect();
    let ["", "navigation", "events", "items", board, item] = path.as_slice() else {
        return Err(404);
    };
    let board = BoardId::new(Uuid::parse_str(board).map_err(|_| 404u16)?).ok_or(404u16)?;
    let item = ItemId::new(Uuid::parse_str(item).map_err(|_| 404u16)?).ok_or(404u16)?;
    let actor = actor?;
    let mut tx = db.transaction().map_err(unavailable)?;
    let account = actor.resolve(&mut tx)?;
    access::authorize(&mut tx, &account, board, access::Operation::SelectedRead)?;
    let row = current_row(&mut tx, board, item)
        .map_err(unavailable)?
        .ok_or(404u16)?;
    tx.commit().map_err(unavailable)?;
    Ok(serde_json::to_vec(&SavedItem {
        id: row.id.as_uuid(),
        title: row.title,
        revision: row.revision,
    })
    .expect("serializable item"))
}

/// The same accepted-row predicate as the board read, scoped to one item so a
/// feed event never loads a whole table.
pub(super) fn current_row(
    tx: &mut Transaction<'_>,
    board: BoardId,
    item: ItemId,
) -> Result<Option<StoredItem>, postgres::Error> {
    tx.query_opt(
        "SELECT i.title,i.revision FROM board_items i JOIN board_groups g
         ON (g.board_id,g.id)=(i.board_id,i.group_id)
         WHERE i.board_id=$1 AND i.id=$2 AND g.active AND i.state=0",
        &[&board.as_uuid(), &item.as_uuid()],
    )
    .map(|row| {
        row.map(|row| StoredItem {
            id: item,
            title: row.get(0),
            revision: row.get(1),
        })
    })
}

fn unavailable(_: postgres::Error) -> u16 {
    503
}

struct EventStream {
    changes: Receiver<ChangeEvent>,
    history: VecDeque<ChangeEvent>,
    buffer: Vec<u8>,
    position: usize,
}

impl Read for EventStream {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.position == self.buffer.len() {
            self.buffer.clear();
            self.position = 0;
            self.buffer = if let Some(event) = self.history.pop_front() {
                event_frame(&event)
            } else {
                match self.changes.recv_timeout(HEARTBEAT) {
                    Ok(event) => event_frame(&event),
                    Err(RecvTimeoutError::Timeout) => comment_frame("keep-alive"),
                    Err(RecvTimeoutError::Disconnected) => return Ok(0),
                }
            };
        }
        let count = (self.buffer.len() - self.position).min(out.len());
        out[..count].copy_from_slice(&self.buffer[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

fn event_frame(event: &ChangeEvent) -> Vec<u8> {
    let body = serde_json::to_string(event).expect("serializable change");
    // The blank line ends the event before the padding starts, so the first
    // flushed chunk always contains a complete event.
    let mut frame = format!("event: change\ndata: {body}\n\n:").into_bytes();
    pad(&mut frame);
    frame.extend_from_slice(b"\n");
    frame
}

fn comment_frame(text: &str) -> Vec<u8> {
    let mut frame = format!(": {text}\n:").into_bytes();
    pad(&mut frame);
    frame.extend_from_slice(b"\n");
    frame
}

fn pad(frame: &mut Vec<u8>) {
    if frame.len() < FLUSH {
        frame.resize(FLUSH, b' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_bounded_and_distinct_per_subscriber() {
        let hub = Hub::new();
        for n in 0..(HISTORY + 3) {
            hub.publish_rename(Uuid::nil(), Uuid::from_u128(n as u128));
        }
        let (history, _receiver) = hub.subscribe();
        assert_eq!(history.len(), HISTORY);
        assert_eq!(history.front().expect("oldest").item_id, Uuid::from_u128(3));
        assert_eq!(history.back().expect("newest").seq, (HISTORY + 3) as u64);
    }

    #[test]
    fn a_frame_is_flushed_without_changing_the_event() {
        let event = ChangeEvent {
            seq: 1,
            board_id: Uuid::nil(),
            item_id: Uuid::nil(),
            kind: ChangeKind::ItemRenamed,
        };
        let frame = event_frame(&event);
        assert!(frame.len() >= FLUSH);
        let text = String::from_utf8(frame).expect("utf8");
        let data = text
            .lines()
            .take_while(|line| !line.is_empty())
            .find_map(|line| line.strip_prefix("data: "))
            .expect("data line");
        assert_eq!(serde_json::from_str::<ChangeEvent>(data).unwrap(), event);
    }
}
