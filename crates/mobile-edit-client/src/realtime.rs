//! Accepted board changes, pushed by the host. The event names the record that
//! changed; the client reads that record through the ordinary authorized route,
//! so a feed connection never carries content from a board the session cannot see.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    ItemRenamed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeEvent {
    pub seq: u64,
    pub board_id: Uuid,
    pub item_id: Uuid,
    pub kind: ChangeKind,
}

#[cfg(not(target_family = "wasm"))]
mod feed {
    use super::ChangeEvent;
    use crate::{EditClient, EditError, request_error};
    use std::io::{BufRead, BufReader, Read};
    use std::time::Duration;

    /// One long-lived subscription. `next` blocks until the host sends an event
    /// or a keep-alive; callers run it on a worker thread.
    pub struct ChangeFeed {
        source: BufReader<Box<dyn Read + Send>>,
        data: String,
    }

    impl ChangeFeed {
        pub fn next(&mut self) -> Result<ChangeEvent, EditError> {
            loop {
                let mut line = String::new();
                let read = self
                    .source
                    .read_line(&mut line)
                    .map_err(|_| EditError::Transport)?;
                if read == 0 {
                    return Err(EditError::Transport);
                }
                if line.starts_with(':') {
                    continue;
                }
                if let Some(value) = line.strip_prefix("data:") {
                    if !self.data.is_empty() {
                        self.data.push('\n');
                    }
                    self.data
                        .push_str(value.strip_prefix(' ').unwrap_or(value).trim_end());
                    if self.data.len() > 64 * 1024 {
                        return Err(EditError::Transport);
                    }
                    continue;
                }
                if line.trim_end().is_empty() && !self.data.is_empty() {
                    let event =
                        serde_json::from_str(&self.data).map_err(|_| EditError::Transport)?;
                    self.data.clear();
                    return Ok(event);
                }
            }
        }
    }

    impl EditClient {
        /// Open the accepted-change feed. The event body is text/event-stream;
        /// exactly one content type is required so an ordinary JSON response can
        /// never be mistaken for a subscription.
        pub fn subscribe_changes(&self) -> Result<ChangeFeed, EditError> {
            let config = ureq::Agent::config_builder()
                .proxy(None)
                .max_redirects(0)
                .timeout_global(None)
                .timeout_recv_response(Some(Duration::from_secs(10)))
                .build();
            let http: ureq::Agent = config.into();
            let url = format!("http://127.0.0.1:{}/navigation/events", self.port);
            let response = self
                .authorize(http.get(url))
                .call()
                .map_err(request_error)?;
            let mut types = response
                .headers()
                .get_all("Content-Type")
                .iter()
                .filter(|value| value.to_str().ok() == Some("text/event-stream"));
            if types.next().is_none() || types.next().is_some() {
                return Err(EditError::Transport);
            }
            let source: Box<dyn Read + Send> = Box::new(response.into_body().into_reader());
            Ok(ChangeFeed {
                source: BufReader::new(source),
                data: String::new(),
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Cursor;

        fn feed(bytes: &[u8]) -> ChangeFeed {
            ChangeFeed {
                source: BufReader::new(Box::new(Cursor::new(bytes.to_vec()))),
                data: String::new(),
            }
        }

        #[test]
        fn one_event_is_returned_before_a_padded_tail_arrives() {
            let event = ChangeEvent {
                seq: 7,
                board_id: uuid::Uuid::from_u128(1),
                item_id: uuid::Uuid::from_u128(2),
                kind: crate::ChangeKind::ItemRenamed,
            };
            let mut bytes = format!(
                "event: change\ndata: {}\n\n:",
                serde_json::to_string(&event).unwrap()
            )
            .into_bytes();
            bytes.resize(9000, b' ');
            let mut feed = feed(&bytes);
            assert_eq!(feed.next().unwrap(), event);
        }

        #[test]
        fn comments_and_multiline_data_are_ignored_and_joined() {
            let mut feed = feed(b": keep-alive\n\ndata: {\"seq\":3,\"board_id\":\ndata: \"00000000-0000-0000-0000-000000000001\",\"item_id\":\"00000000-0000-0000-0000-000000000002\",\"kind\":\"item_renamed\"}\n\n");
            let event = feed.next().unwrap();
            assert_eq!(event.seq, 3);
            assert_eq!(event.kind, crate::ChangeKind::ItemRenamed);
        }

        #[test]
        fn a_closed_stream_is_a_transport_failure() {
            let mut feed = feed(b"event: change\ndata: {\"seq\"");
            assert_eq!(feed.next(), Err(EditError::Transport));
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use feed::ChangeFeed;
