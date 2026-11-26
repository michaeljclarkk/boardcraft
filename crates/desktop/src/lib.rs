use boardcraft_board::{BoardId, Item, ItemId, RenameError, store};
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

impl From<store::SaveError> for Failure {
    fn from(error: store::SaveError) -> Self {
        match error {
            store::SaveError::Rejected(reason) => Self::Rejected(reason),
            store::SaveError::Conflict { current } => Self::Conflict(current),
            store::SaveError::Database(_) => Self::Database,
        }
    }
}

#[derive(Clone)]
pub struct Save {
    board: BoardId,
    item: ItemId,
    expected: String,
    draft: String,
    retry: bool,
}

impl Save {
    pub fn connect_and_run(self, cfg: &Config) -> Result<String, Failure> {
        let mut db = cfg.connect(NoTls).map_err(|_| {
            eprintln!(
                "rename_item connect board={} item={}",
                self.board.as_uuid(),
                self.item.as_uuid()
            );
            Failure::Database
        })?;
        self.run(&mut db)
    }

    pub fn run(self, db: &mut Client) -> Result<String, Failure> {
        if self.retry {
            let current = store::current_title(db, self.board, self.item)?;
            if current != self.expected {
                return Err(Failure::Conflict(current));
            }
        }
        store::rename_item_if_current(db, self.board, self.item, &self.expected, &self.draft)
            .map_err(Into::into)
    }
}

/// View state for exactly one row. No second request may start before completion.
pub struct InlineEdit {
    board: BoardId,
    item: Item,
    request: Option<String>,
    phase: Phase,
}

impl InlineEdit {
    pub fn new(board: BoardId, item: Item) -> Self {
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
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn dismiss(&mut self) {
        if matches!(self.phase, Phase::Failed(_)) {
            self.request = None;
            self.phase = Phase::Clean;
        }
    }

    pub fn submit(&mut self, raw: String) -> Option<Save> {
        if self.phase == Phase::Pending || raw == self.accepted() {
            return None;
        }
        self.request = Some(raw);
        Some(self.start(false))
    }

    pub fn retry(&mut self) -> Option<Save> {
        matches!(self.phase, Phase::Failed(_)).then(|| self.start(true))
    }

    fn start(&mut self, retry: bool) -> Save {
        self.phase = Phase::Pending;
        Save {
            board: self.board,
            item: self.item.id,
            expected: self.item.title.clone(),
            draft: self.request().unwrap().to_owned(),
            retry,
        }
    }

    pub fn finish(&mut self, result: Result<String, Failure>) {
        debug_assert_eq!(self.phase, Phase::Pending);
        match result {
            Ok(title) => {
                self.request = None;
                self.item.title = title;
                self.phase = Phase::Saved;
            }
            Err(reason) => {
                if let Failure::Conflict(current) = &reason {
                    self.item.title.clone_from(current);
                }
                self.phase = Phase::Failed(reason);
            }
        }
    }
}
