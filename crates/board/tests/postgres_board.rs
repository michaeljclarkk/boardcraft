use boardcraft_board::store::SaveError;
use boardcraft_board::{Board, BoardId, ItemId, RenameError, store};
use postgres::{Client, Config, NoTls, error::SqlState};
use uuid::Uuid;

fn test_config() -> Config {
    let url = std::env::var("RUSTDAY_TEST_DATABASE_URL")
        .expect("RUSTDAY_TEST_DATABASE_URL must name a disposable boardcraft test database");
    let config: Config = url.parse().expect("invalid PostgreSQL connection settings");
    assert!(
        config.get_dbname() == Some("rustday_test"),
        "expected explicit rustday_test database"
    );
    config
}

fn reconnect(config: &Config, schema: &str) -> Client {
    let mut client = config.connect(NoTls).unwrap();
    client
        .batch_execute(&format!("SET search_path TO {schema}"))
        .unwrap();
    client
}

#[test]
fn reconnect_after_save_rejection_and_failed_commit() {
    let config = test_config();
    let schema = format!("board_test_{}", Uuid::new_v4().simple());
    let mut db = config.connect(NoTls).unwrap();
    db.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .unwrap();
    store::migrate(&mut db).unwrap();

    let board = BoardId::new(Uuid::from_u128(5)).unwrap();
    let other = BoardId::new(Uuid::from_u128(6)).unwrap();
    let first = ItemId::new(Uuid::from_u128(42)).unwrap();
    let second = ItemId::new(Uuid::from_u128(17)).unwrap();
    let foreign = ItemId::new(Uuid::from_u128(30)).unwrap();
    let mut expected = Board::new(board);
    expected.add_item(first, "Outline").unwrap();
    expected.add_item(second, "Outline").unwrap();
    let mut untouched = Board::new(other);
    untouched
        .add_item(foreign, "  Preserve spelling  ")
        .unwrap();
    store::create_board(&mut db, &expected).unwrap();
    store::create_board(&mut db, &untouched).unwrap();

    let max_title = format!("{}!", "🪴".repeat(127)); // 255 UTF-16 units
    let draft = format!("\t{max_title}  ");
    assert_eq!(
        store::rename_item(&mut db, board, second, &draft).unwrap(),
        max_title
    );
    expected.rename_item(second, &max_title).unwrap();
    drop(db);

    let mut db = reconnect(&config, &schema);
    store::migrate(&mut db).unwrap(); // reopening a populated schema is safe
    assert_eq!(
        store::load_board(&mut db, board).unwrap().unwrap(),
        expected
    );
    for (item, draft, error) in [
        (second, " \t\n".to_owned(), RenameError::BlankTitle),
        (second, "🪴".repeat(128), RenameError::TitleTooLong),
        (
            foreign,
            "Wrong board".to_owned(),
            RenameError::MissingItem(foreign),
        ),
    ] {
        assert!(matches!(store::rename_item(&mut db, board, item, &draft),
            Err(SaveError::Rejected(actual)) if actual == error));
    }

    // Test-only constraint: UPDATE ... RETURNING succeeds, but COMMIT fails.
    db.batch_execute(
        "ALTER TABLE board_items ADD UNIQUE (board_id, title) DEFERRABLE INITIALLY DEFERRED",
    )
    .unwrap();
    assert!(
        matches!(store::rename_item(&mut db, board, second, "Outline"),
        Err(SaveError::Database(e)) if e.code() == Some(&SqlState::UNIQUE_VIOLATION))
    );
    drop(db);

    let mut db = reconnect(&config, &schema);
    assert_eq!(
        store::load_board(&mut db, board).unwrap().unwrap(),
        expected
    );
    assert_eq!(
        store::load_board(&mut db, other).unwrap().unwrap(),
        untouched
    );
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}
