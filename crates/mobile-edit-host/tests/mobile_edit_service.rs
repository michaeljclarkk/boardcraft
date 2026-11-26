use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit, SavedItem};
use boardcraft_mobile_edit_host::Host;
use postgres::{Client, Config, NoTls};
use uuid::Uuid;

const FIRST: Uuid = Uuid::from_u128(42);
const SECOND: Uuid = Uuid::from_u128(17);

struct Fixture {
    config: Config,
    schema: String,
    board: Uuid,
    host: Host,
}

impl Fixture {
    fn start() -> Self {
        let mut config: Config = std::env::var("RUSTDAY_TEST_DATABASE_URL")
            .expect("RUSTDAY_TEST_DATABASE_URL is required")
            .parse()
            .expect("invalid database configuration");
        assert_eq!(config.get_dbname(), Some("rustday_test"));
        let schema = format!("mobile_test_{}", Uuid::new_v4().simple());
        config
            .connect(NoTls)
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        config.options(&format!("-csearch_path={schema}"));
        let mut db = config.connect(NoTls).unwrap();
        store::migrate(&mut db).unwrap();
        let board = Uuid::new_v4();
        let mut fixture = Board::new(BoardId::new(board).unwrap());
        for id in [FIRST, SECOND] {
            fixture
                .add_item(ItemId::new(id).unwrap(), "Review")
                .unwrap();
        }
        store::create_board(&mut db, &fixture).unwrap();
        let host = Host::start(&config, fixture.id()).unwrap();
        Self {
            config,
            schema,
            board,
            host,
        }
    }

    fn client(&self) -> EditClient {
        EditClient::new(self.host.port)
    }
    fn connect(&self) -> Client {
        self.config.connect(NoTls).unwrap()
    }

    fn check_store(&self, expected: &[SavedItem]) {
        let mut db = self.connect();
        let id = BoardId::new(self.board).unwrap();
        let desktop = store::load_board(&mut db, id).unwrap().unwrap();
        assert_eq!(desktop.id(), id);
        assert_eq!(desktop.items().len(), expected.len());
        for (actual, expected) in desktop.items().iter().zip(expected) {
            assert_eq!(
                (actual.id.as_uuid(), &actual.title),
                (expected.id, &expected.title)
            );
        }
        let revisions: Vec<i64> = store::load_items(&mut db, id)
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

    fn finish(self) {
        drop(self.host);
        self.config
            .connect(NoTls)
            .unwrap()
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .unwrap();
    }
}

fn edit(title: &str, revision: i64) -> NameEdit {
    NameEdit {
        title: title.into(),
        revision,
    }
}

fn open(client: &EditClient, board: Uuid) -> Vec<SavedItem> {
    let rows = client.read(board).unwrap();
    assert_eq!(rows.len(), 2);
    for (row, id) in rows.iter().zip([FIRST, SECOND]) {
        assert_eq!(row.id, id);
        assert_eq!(row.title, "Review");
        assert!(row.revision > 0);
    }
    rows
}

#[test]
fn rename_then_reconnect() {
    let f = Fixture::start();
    let client = f.client();
    let mut rows = open(&client, f.board);
    let request = edit(" Review artwork ", rows[1].revision);
    let saved = client.rename(f.board, SECOND, &request).unwrap();
    assert_eq!(saved.id, SECOND);
    assert_eq!(saved.title, "Review artwork");
    assert!(saved.revision > rows[1].revision);
    rows[1] = saved.clone();
    drop(client);

    let fresh = f.client();
    assert_eq!(fresh.read(f.board).unwrap(), rows);
    f.check_store(&rows);
    assert_eq!(fresh.rename(f.board, SECOND, &request).unwrap(), saved);
    f.check_store(&rows);

    // A change committed through the Windows conditional-save path is visible on resume.
    let mut db = f.connect();
    let board = BoardId::new(f.board).unwrap();
    let item = ItemId::new(SECOND).unwrap();
    assert!(matches!(
        store::rename_item_if_current(&mut db, board, item, "Review", "Stale"),
        Err(store::SaveError::Conflict { .. })
    ));
    f.check_store(&rows);
    store::rename_item_if_current(&mut db, board, item, &saved.title, "Approved").unwrap();
    let after_desktop = fresh.read(f.board).unwrap();
    assert_eq!(after_desktop[0], rows[0]);
    assert_eq!(after_desktop[1].title, "Approved");
    assert!(after_desktop[1].revision > saved.revision);
    // An older known revision may also submit a different scalar value.
    let changed = fresh.rename(f.board, SECOND, &request).unwrap();
    assert_eq!(changed.title, saved.title);
    assert!(changed.revision > after_desktop[1].revision);
    rows[1] = changed;
    f.check_store(&rows);
    f.finish();
}

#[test]
fn reject_then_retry() {
    let f = Fixture::start();
    let client = f.client();
    let mut rows = open(&client, f.board);
    for (board, item, request, error) in [
        (
            f.board,
            SECOND,
            edit(" \t ", rows[1].revision),
            EditError::InvalidDraft,
        ),
        (
            f.board,
            SECOND,
            edit("Future", rows[1].revision + 1),
            EditError::Conflict,
        ),
        (
            Uuid::new_v4(),
            SECOND,
            edit("Wrong board", rows[1].revision),
            EditError::Missing,
        ),
        (
            f.board,
            Uuid::new_v4(),
            edit("Absent", rows[1].revision),
            EditError::Missing,
        ),
    ] {
        assert_eq!(client.rename(board, item, &request), Err(error));
        f.check_store(&rows);
    }
    let reopened = client.read(f.board).unwrap();
    assert_eq!(reopened, rows);
    let saved = client
        .rename(f.board, SECOND, &edit(" Corrected ", reopened[1].revision))
        .unwrap();
    assert_eq!(saved.id, SECOND);
    assert_eq!(saved.title, "Corrected");
    assert!(saved.revision > reopened[1].revision);
    rows[1] = saved;
    f.check_store(&rows);

    // A failed COMMIT returns only the fault category and rolls back the revision too.
    f.connect()
        .batch_execute(
            "ALTER TABLE board_items ADD UNIQUE (board_id, title) DEFERRABLE INITIALLY DEFERRED",
        )
        .unwrap();
    assert_eq!(
        client.rename(f.board, SECOND, &edit("Review", rows[1].revision)),
        Err(EditError::HostFault)
    );
    f.check_store(&rows);
    f.finish();
}
