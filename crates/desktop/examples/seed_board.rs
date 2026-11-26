use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_desktop::database;
use postgres::NoTls;
use uuid::Uuid;

fn main() -> Result<(), &'static str> {
    let mut db = database()?
        .connect(NoTls)
        .map_err(|_| "Cannot connect to rustday_test")?;
    store::migrate(&mut db).map_err(|_| "Cannot migrate fixture schema")?;
    let mut board = Board::new(BoardId::new(Uuid::new_v4()).unwrap());
    for _ in 0..2 {
        board
            .add_item(ItemId::new(Uuid::new_v4()).unwrap(), "Review")
            .unwrap();
    }
    store::create_board(&mut db, &board).map_err(|_| "Cannot create synthetic board")?;
    println!("{}", board.id().as_uuid());
    Ok(())
}
