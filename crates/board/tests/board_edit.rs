use boardcraft_board::{Board, BoardId, DuplicateItem, ItemId, RenameError};
use uuid::Uuid;

const FIRST: ItemId = ItemId::new(Uuid::from_u128(42)).unwrap();
const SECOND: ItemId = ItemId::new(Uuid::from_u128(17)).unwrap();

fn sample_board() -> Board {
    let mut board = Board::new(BoardId::new(Uuid::from_u128(1)).unwrap());
    board.add_item(FIRST, "Outline").unwrap();
    board.add_item(SECOND, "Outline").unwrap();
    board
}

#[test]
fn rename_changes_only_the_selected_item_and_preserves_order() {
    let mut board = sample_board();
    board.rename_item(SECOND, " Revised outline  ").unwrap();

    let rows: Vec<_> = board
        .items()
        .iter()
        .map(|item| (item.id, item.title.as_str()))
        .collect();
    assert_eq!(rows, [(FIRST, "Outline"), (SECOND, "Revised outline")]);
}

#[test]
fn missing_rename_leaves_board_intact() {
    let mut board = sample_board();
    let before = board.items().to_vec();
    let missing = ItemId::new(Uuid::from_u128(99)).unwrap();

    assert_eq!(
        board.rename_item(missing, "Draft"),
        Err(RenameError::MissingItem(missing))
    );
    assert_eq!(board.items(), before);
}

#[test]
fn nil_record_ids_are_rejected() {
    assert!(BoardId::new(Uuid::nil()).is_none());
    assert!(ItemId::new(Uuid::nil()).is_none());
}

#[test]
fn duplicate_id_cannot_create_an_ambiguous_rename_target() {
    let mut board = sample_board();
    let before = board.items().to_vec();

    assert_eq!(board.add_item(FIRST, "Other"), Err(DuplicateItem(FIRST)));
    assert_eq!(board.items(), before);
}
