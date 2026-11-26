use crate::{fixture::Fixture, reorder_state, selection::SELECTED};
use uuid::Uuid;

pub fn seed(f: &Fixture, actor: Uuid) {
    reorder_state::seed(f, actor);
    for (n, state, title) in [(68, 2, "Trash only"), (69, 3, "Staged")] {
        f.connect().execute("INSERT INTO board_items(id,board_id,group_id,title,position,state) VALUES ($1,$2,'g-90',$3,1,$4)", &[&Uuid::from_u128(n),&SELECTED,&title,&state]).unwrap();
    }
}

pub fn seed_group_recovery(f: &Fixture, actor: Uuid) -> reorder_state::Snapshot {
    use boardcraft_board::{BoardId, ItemId, store};
    seed(f, actor);
    // Position zero ties across groups. Give the older archived fixture root
    // an explicit creation time rather than depending on fixture insert order.
    f.connect()
        .execute(
            "UPDATE board_items SET created_at='2000-01-01T00:00:00Z' WHERE id=$1",
            &[&Uuid::from_u128(57)],
        )
        .unwrap();
    let before = reorder_state::Snapshot::read(f);
    let mut db = f.connect();
    let mut tx = db.transaction().unwrap();
    store::transition_root(
        &mut tx,
        actor,
        BoardId::new(SELECTED).unwrap(),
        ItemId::new(Uuid::from_u128(67)).unwrap(),
        &store::RootCommand {
            action: store::Lifecycle::Archive,
            operation: Uuid::new_v4(),
            board_revision: 1,
            item_revision: 1,
        },
    )
    .unwrap();
    tx.commit().unwrap();
    before.lifecycle(f, actor, "archive");
    db.execute(
        "UPDATE board_groups SET position=1 WHERE board_id=$1 AND id='g-20'",
        &[&SELECTED],
    )
    .unwrap();
    reorder_state::Snapshot::read(f)
}
