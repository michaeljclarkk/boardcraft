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
    let mut row = InlineEdit::new(board, reload(board).items()[1].clone());

    row.edit(" Review artwork  ".into());
    let save = row.submit().unwrap();
    assert_eq!(row.phase(), &Phase::Pending);
    assert_eq!(row.accepted(), "Review");
    assert!(row.submit().is_none());
    row.revert();
    assert_eq!(row.draft(), " Review artwork  ");
    row.finish(save.run(&mut db));
    assert_eq!(row.phase(), &Phase::Saved);
    assert_eq!(row.draft(), "Review artwork");
    expected.rename_item(second, "Review artwork").unwrap();
    assert_eq!(reload(board), expected);

    db.batch_execute("ALTER TABLE board_items ADD CONSTRAINT distinct_titles UNIQUE (board_id, title) DEFERRABLE INITIALLY DEFERRED").unwrap();
    row.edit(" Review ".into());
    let save = row.submit().unwrap();
    row.finish(save.run(&mut db));
    assert_eq!(row.phase(), &Phase::Failed(Failure::Database));
    assert_eq!(row.draft(), " Review ");
    assert_eq!(row.accepted(), "Review artwork");
    assert_eq!(reload(board), expected);

    db.batch_execute("ALTER TABLE board_items DROP CONSTRAINT distinct_titles")
        .unwrap();
    let retry = row.submit().unwrap();
    row.finish(retry.run(&mut db));
    assert_eq!(row.phase(), &Phase::Saved);
    expected.rename_item(second, "Review").unwrap();
    assert_eq!(reload(board), expected);

    row.edit("  ".into());
    let blank = row.submit().unwrap();
    row.finish(blank.run(&mut db));
    assert_eq!(
        row.phase(),
        &Phase::Failed(Failure::Rejected(RenameError::BlankTitle))
    );
    assert_eq!(row.draft(), "  ");

    store::rename_item(&mut db, board, second, "Newer title").unwrap();
    row.edit("Stale edit".into());
    let stale = row.submit().unwrap();
    row.finish(stale.run(&mut db));
    assert_eq!(
        row.phase(),
        &Phase::Failed(Failure::Conflict("Newer title".into()))
    );
    assert_eq!(row.draft(), "Stale edit");
    row.revert();
    assert_eq!(row.draft(), "Newer title");
    expected.rename_item(second, "Newer title").unwrap();
    assert_eq!(reload(board), expected);
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}
