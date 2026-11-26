use boardcraft_board::{Board, BoardId, ItemId, RenameError, store};
use boardcraft_desktop::{Failure, InlineEdit, Phase, database};
use postgres::NoTls;
use uuid::Uuid;

#[test]
fn save_failure_and_retry_at_the_row_boundary() {
    let cfg = database().unwrap();
    let schema = format!("desktop_test_{}", Uuid::new_v4().simple());
    let mut db = cfg.connect(NoTls).unwrap();
    db.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .unwrap();
    let reload = |id| {
        let mut fresh = cfg.connect(NoTls).unwrap();
        fresh
            .batch_execute(&format!("SET search_path TO {schema}"))
            .unwrap();
        store::load_board(&mut fresh, id).unwrap().unwrap()
    };
    store::migrate(&mut db).unwrap();
    let mut expected = Board::new(BoardId::new(Uuid::from_u128(1)).unwrap());
    let board = expected.id();
    let second = ItemId::new(Uuid::from_u128(17)).unwrap();
    expected
        .add_item(ItemId::new(Uuid::from_u128(42)).unwrap(), "Review")
        .unwrap();
    expected.add_item(second, "Review").unwrap();
    store::create_board(&mut db, &expected).unwrap();
    let mut row = InlineEdit::new(board, store::current_item(&mut db, board, second).unwrap());

    let save = row.submit(" Review artwork  ".into(), "Review", 1).unwrap();
    assert_eq!(row.phase(), &Phase::Pending);
    assert_eq!(row.accepted(), "Review");
    assert!(row.submit("unrelated".into(), "Review", 1).is_none());
    row.dismiss();
    assert_eq!(row.display(), " Review artwork  ");
    row.finish(save.run(&mut db));
    assert_eq!(row.phase(), &Phase::Saved);
    assert_eq!(row.display(), "Review artwork");
    expected.rename_item(second, "Review artwork").unwrap();
    assert_eq!(reload(board), expected);

    db.batch_execute("ALTER TABLE board_items ADD CONSTRAINT distinct_titles UNIQUE (board_id, title) DEFERRABLE INITIALLY DEFERRED").unwrap();
    let save = row.submit(" Review ".into(), "Review artwork", 2).unwrap();
    row.finish(save.run(&mut db));
    assert_eq!(row.phase(), &Phase::Failed(Failure::Database));
    assert_eq!(row.request(), Some(" Review "));
    assert_eq!(row.accepted(), "Review artwork");
    assert_eq!(reload(board), expected);

    db.batch_execute("ALTER TABLE board_items DROP CONSTRAINT distinct_titles")
        .unwrap();
    let retry = row.retry().unwrap();
    row.finish(retry.run(&mut db));
    assert_eq!(row.phase(), &Phase::Saved);
    expected.rename_item(second, "Review").unwrap();
    assert_eq!(reload(board), expected);

    let blank = row.submit("  ".into(), "Review", 3).unwrap();
    row.finish(blank.run(&mut db));
    assert_eq!(
        row.phase(),
        &Phase::Failed(Failure::Rejected(RenameError::BlankTitle))
    );
    assert_eq!(row.request(), Some("  "));

    store::rename_item(&mut db, board, second, "Newer title").unwrap();
    let stale = row.retry().unwrap();
    row.finish(stale.run(&mut db));
    assert_eq!(
        row.phase(),
        &Phase::Failed(Failure::Conflict("Newer title".into()))
    );
    assert_eq!(row.request(), Some("  "));
    row.dismiss();
    assert_eq!(row.display(), "Newer title");
    assert!(row.retry().is_none());
    expected.rename_item(second, "Newer title").unwrap();
    assert_eq!(reload(board), expected);

    store::rename_item(&mut db, board, second, "Changed again").unwrap();
    let stale = row.submit(" My edit ".into(), "Newer title", 4).unwrap();
    row.finish(stale.run(&mut db));
    assert_eq!(
        row.phase(),
        &Phase::Failed(Failure::Conflict("Changed again".into()))
    );
    expected.rename_item(second, "Changed again").unwrap();
    assert_eq!(reload(board), expected);
    let retry = row.retry().unwrap();
    row.finish(retry.run(&mut db));
    assert_eq!(row.phase(), &Phase::Saved);
    expected.rename_item(second, "My edit").unwrap();
    assert_eq!(reload(board), expected);
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}
