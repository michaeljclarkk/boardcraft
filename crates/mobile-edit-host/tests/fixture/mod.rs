mod account;
pub use account::seed_account_on;
use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_mobile_edit_client::SavedItem;
use boardcraft_mobile_edit_host::Host;
use postgres::{Client, Config, NoTls};
use std::time::Duration;
use uuid::Uuid;

pub const FIRST: Uuid = Uuid::from_u128(42);
pub const SECOND: Uuid = Uuid::from_u128(17);

pub struct Schema {
    pub config: Config,
    pub name: String,
}

impl Drop for Schema {
    fn drop(&mut self) {
        let result = self
            .config
            .connect(NoTls)
            .and_then(|mut db| db.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.name)));
        if result.is_err() {
            if std::thread::panicking() {
                eprintln!("also failed to remove fixture schema {}", self.name);
            } else {
                panic!("failed to remove fixture schema {}", self.name);
            }
        }
    }
}

pub struct Fixture {
    // Fields drop in order: stop serving before disposing of the schema.
    pub host: Host,
    pub schema: Schema,
    pub board: Uuid,
}

impl Schema {
    pub fn new() -> Self {
        let mut config: Config = std::env::var("RUSTDAY_TEST_DATABASE_URL")
            .expect("RUSTDAY_TEST_DATABASE_URL is required")
            .parse()
            .expect("invalid database configuration");
        assert_eq!(config.get_dbname(), Some("rustday_test"));
        let name = format!("mobile_test_{}", Uuid::new_v4().simple());
        config
            .connect_timeout(Duration::from_secs(5))
            .options(&format!(
                "-csearch_path={name} -cstatement_timeout=10000 -clock_timeout=5000"
            ));
        let mut db = config.connect(NoTls).unwrap();
        db.batch_execute(&format!("CREATE SCHEMA {name}")).unwrap();
        Self { config, name }
    }
}

impl Fixture {
    #[cfg(test)]
    pub fn start() -> Self {
        Self::with_titles(["Review", "Review"])
    }

    pub fn with_titles(titles: [&str; 2]) -> Self {
        Self::before_host(titles, |_, _, _| {})
    }

    pub fn before_host(titles: [&str; 2], setup: impl FnOnce(&Config, &mut Client, Uuid)) -> Self {
        let schema = Schema::new();
        let mut db = schema.config.connect(NoTls).unwrap();
        store::migrate(&mut db).unwrap();
        let board = Uuid::new_v4();
        let mut seed = Board::new(BoardId::new(board).unwrap());
        for (id, title) in [FIRST, SECOND].into_iter().zip(titles) {
            seed.add_item(ItemId::new(id).unwrap(), title).unwrap();
        }
        store::create_board(&mut db, &seed).unwrap();
        setup(&schema.config, &mut db, board);
        let host = Host::start(&schema.config, seed.id()).unwrap();
        Self {
            host,
            schema,
            board,
        }
    }

    pub fn connect(&self) -> Client {
        self.schema.config.connect(NoTls).unwrap()
    }

    pub fn check_store(&self, expected: &[SavedItem]) {
        self.check_board(self.board, expected);
    }

    pub fn check_board(&self, board: Uuid, expected: &[SavedItem]) {
        let mut db = self.connect();
        let id = BoardId::new(board).unwrap();
        let desktop = store::load_board(&mut db, id).unwrap().unwrap();
        assert_eq!(desktop.id(), id);
        assert_eq!(desktop.items().len(), expected.len());
        for (actual, expected) in desktop.items().iter().zip(expected) {
            assert_eq!(
                (actual.id.as_uuid(), &actual.title),
                (expected.id, &expected.title)
            );
        }
        let revisions: Vec<_> = store::load_items(&mut db, id)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|row| row.revision)
            .collect();
        assert_eq!(
            revisions,
            expected.iter().map(|row| row.revision).collect::<Vec<_>>()
        );
    }
}
