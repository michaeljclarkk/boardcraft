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
    Editing,
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

pub struct Save {
    board: BoardId,
    item: ItemId,
    expected: String,
    draft: String,
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
        store::rename_item_if_current(db, self.board, self.item, &self.expected, &self.draft)
            .map_err(Into::into)
    }
}

/// View state for exactly one row. No second request may start before completion.
pub struct InlineEdit {
    board: BoardId,
    item: Item,
    draft: String,
    phase: Phase,
}

impl InlineEdit {
    pub fn new(board: BoardId, item: Item) -> Self {
        Self {
            board,
            draft: item.title.clone(),
            item,
            phase: Phase::Clean,
        }
    }

    pub fn identity(&self) -> (BoardId, ItemId) {
        (self.board, self.item.id)
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }
    pub fn accepted(&self) -> &str {
        &self.item.title
    }
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn edit(&mut self, draft: String) {
        if self.phase != Phase::Pending && draft != self.draft {
            self.phase = if draft == self.item.title {
                Phase::Clean
            } else {
                Phase::Editing
            };
            self.draft = draft;
        }
    }

    pub fn revert(&mut self) {
        if self.phase != Phase::Pending {
            self.draft.clone_from(&self.item.title);
            self.phase = Phase::Clean;
        }
    }

    pub fn submit(&mut self) -> Option<Save> {
        if !matches!(self.phase, Phase::Editing | Phase::Failed(_)) {
            return None;
        }
        self.phase = Phase::Pending;
        Some(Save {
            board: self.board,
            item: self.item.id,
            expected: self.item.title.clone(),
            draft: self.draft.clone(),
        })
    }

    pub fn finish(&mut self, result: Result<String, Failure>) {
        debug_assert_eq!(self.phase, Phase::Pending);
        match result {
            Ok(title) => {
                self.draft.clone_from(&title);
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
