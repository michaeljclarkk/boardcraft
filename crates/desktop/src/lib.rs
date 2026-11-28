use boardcraft_board::{
    BoardId, ItemId, RenameError,
    store::{self, StoredItem},
};
use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit};
use postgres::{Client, Config, NoTls};

/// This development window only opens the explicitly supplied disposable database.
pub fn database() -> Result<Config, &'static str> {
    let url = std::env::var("RUSTDAY_TEST_DATABASE_URL")
        .map_err(|_| "Set RUSTDAY_TEST_DATABASE_URL before opening the app.")?;
    url.parse::<Config>()
        .ok()
        .filter(|cfg| cfg.get_dbname() == Some("rustday_test"))
        .ok_or("RUSTDAY_TEST_DATABASE_URL must explicitly name rustday_test.")
}

#[derive(Debug, PartialEq, Eq)]
pub enum Phase {
    Clean,
    Pending,
    Saved,
    Failed(Failure),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Rejected(RenameError),
    Conflict(String),
    Database,
}

impl From<&store::SaveError> for Failure {
    fn from(error: &store::SaveError) -> Self {
        match error {
            store::SaveError::Rejected(reason) => Self::Rejected(reason.clone()),
            store::SaveError::Conflict { current, .. } => Self::Conflict(current.clone()),
            store::SaveError::Database(_) => Self::Database,
        }
    }
}

#[derive(Clone)]
pub struct Save {
    board: BoardId,
    item: ItemId,
    expected: String,
    entry_revision: i64,
    draft: String,
    retry: bool,
}

impl Save {
    pub fn send(self, session: &EditClient) -> Result<StoredItem, EditError> {
        self.send_name(session, false)
    }

    pub fn send_selected(self, session: &EditClient) -> Result<StoredItem, EditError> {
        self.send_name(session, true)
    }

    fn send_name(self, session: &EditClient, selected: bool) -> Result<StoredItem, EditError> {
        let edit = NameEdit {
            title: self.draft,
            revision: self.entry_revision,
        };
        let saved = if selected {
            session.rename_main_table_item(self.board.as_uuid(), self.item.as_uuid(), &edit)
        } else {
            session.rename(self.board.as_uuid(), self.item.as_uuid(), &edit)
        }?;
        if saved.id != self.item.as_uuid() || saved.revision < self.entry_revision {
            return Err(EditError::Transport);
        }
        Ok(StoredItem {
            id: self.item,
            title: saved.title,
            revision: saved.revision,
        })
    }

    pub fn connect_and_run(self, cfg: &Config) -> Result<StoredItem, store::SaveError> {
        let mut db = cfg.connect(NoTls).map_err(|error| {
            eprintln!(
                "rename_item connect board={} item={}",
                self.board.as_uuid(),
                self.item.as_uuid()
            );
            store::SaveError::Database(error)
        })?;
        self.run(&mut db)
    }

    pub fn run(self, db: &mut Client) -> Result<StoredItem, store::SaveError> {
        if self.retry {
            let current = store::current_item(db, self.board, self.item)?;
            if current.title != self.expected {
                return Err(store::SaveError::Conflict {
                    current: current.title,
                    revision: current.revision,
                });
            }
        }
        store::finish_name_edit(
            db,
            self.board,
            self.item,
            self.entry_revision,
            &self.expected,
            &self.draft,
        )
    }
}

/// View state for exactly one row. No second request may start before completion.
pub struct InlineEdit {
    board: BoardId,
    item: StoredItem,
    request: Option<String>,
    phase: Phase,
}

impl InlineEdit {
    /// Called only when mounting a row loaded from the accepted board.
    pub fn restore_failure(&mut self, request: String, reason: Failure) {
        assert_eq!(self.phase, Phase::Clean);
        self.request = Some(request);
        self.phase = Phase::Failed(reason);
    }

    pub fn new(board: BoardId, item: StoredItem) -> Self {
        Self {
            board,
            request: None,
            item,
            phase: Phase::Clean,
        }
    }

    pub fn identity(&self) -> (BoardId, ItemId) {
        (self.board, self.item.id)
    }
    pub fn request(&self) -> Option<&str> {
        self.request.as_deref()
    }
    pub fn display(&self) -> &str {
        if self.phase == Phase::Pending {
            self.request().expect("pending name")
        } else {
            self.accepted()
        }
    }
    pub fn accepted(&self) -> &str {
        &self.item.title
    }
    pub fn revision(&self) -> i64 {
        self.item.revision
    }
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn dismiss(&mut self) {
        if matches!(self.phase, Phase::Failed(_)) {
            self.request = None;
            self.phase = Phase::Clean;
        }
    }

    /// Drop only the local pending projection. The caller must read the server
    /// before enabling another edit; this does not claim the write was undone.
    pub fn discard_unconfirmed(&mut self) {
        if self.phase == Phase::Pending {
            self.request = None;
            self.phase = Phase::Clean;
        }
    }

    pub fn submit(&mut self, raw: String, entry: &str, entry_revision: i64) -> Option<Save> {
        if self.phase == Phase::Pending || raw == entry {
            return None;
        }
        self.request = Some(raw);
        Some(self.start(false, entry_revision))
    }

    pub fn retry(&mut self) -> Option<Save> {
        matches!(self.phase, Phase::Failed(_)).then(|| self.start(true, self.revision()))
    }

    fn start(&mut self, retry: bool, entry_revision: i64) -> Save {
        self.phase = Phase::Pending;
        Save {
            board: self.board,
            item: self.item.id,
            expected: self.item.title.clone(),
            entry_revision,
            draft: self.request().unwrap().to_owned(),
            retry,
        }
    }

    /// Replace the clean baseline with a row the host accepted elsewhere.
    /// A pending save owns this row until its own path finishes, and a failure
    /// keeps the local recovery record, so neither takes a remote value here.
    pub fn accept_remote(&mut self, item: StoredItem) -> bool {
        if self.item.id != item.id
            || item.revision <= self.item.revision
            || !matches!(self.phase, Phase::Clean | Phase::Saved)
        {
            return false;
        }
        self.item = item;
        self.request = None;
        self.phase = Phase::Clean;
        true
    }

    pub fn finish(&mut self, result: Result<StoredItem, store::SaveError>) {
        debug_assert_eq!(self.phase, Phase::Pending);
        match result {
            Ok(item) => {
                self.request = None;
                self.item = item;
                self.phase = Phase::Saved;
            }
            Err(reason) => {
                if let store::SaveError::Conflict { current, revision } = &reason {
                    self.item.title.clone_from(current);
                    self.item.revision = *revision;
                }
                self.phase = Phase::Failed((&reason).into());
            }
        }
    }
}
