use crate::{fixture::Fixture, reorder_state, selection::SELECTED};
use uuid::Uuid;

pub fn seed(f: &Fixture, actor: Uuid) {
    reorder_state::seed(f, actor);
    for (n, state, title) in [(68, 2, "Trash only"), (69, 3, "Staged")] {
        f.connect().execute("INSERT INTO board_items(id,board_id,group_id,title,position,state) VALUES ($1,$2,'g-90',$3,1,$4)", &[&Uuid::from_u128(n),&SELECTED,&title,&state]).unwrap();
    }
}
