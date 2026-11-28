use crate::{
    BoardId, ItemId,
    store::{self, SaveError, StoredItem},
};
use postgres::Transaction;
use uuid::Uuid;

pub struct Entry {
    pub actor_id: Uuid,
    pub actor_name: String,
    pub occurred_at: String,
    pub field: String,
    pub before: String,
    pub after: String,
    pub revision: i64,
}

/// `before` is the title under the rename's row lock, not a preliminary read.
pub fn rename_item(
    tx: &mut Transaction<'_>,
    board: BoardId,
    item: ItemId,
    revision: i64,
    draft: &str,
    actor: Uuid,
    actor_name: &str,
) -> Result<(StoredItem, bool), SaveError> {
    let (before, saved) = store::rename_with_previous(tx, board, item, revision, draft)?;
    let changed = before != saved.title;
    if changed {
        tx.execute("INSERT INTO item_activity(board_id,item_id,revision,actor_id,actor_name,field,before_value,after_value)
            VALUES ($1,$2,$3,$4,$5,'name',$6,$7)", &[&board.as_uuid(),&item.as_uuid(),&saved.revision,&actor,&actor_name,&before,&saved.title]).map_err(SaveError::Database)?;
    }
    Ok((saved, changed))
}

pub fn entries(
    tx: &mut Transaction<'_>,
    board: BoardId,
    item: ItemId,
) -> Result<Vec<Entry>, postgres::Error> {
    tx.query("SELECT actor_id,actor_name,to_char(occurred_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),field,before_value,after_value,revision
        FROM item_activity WHERE board_id=$1 AND item_id=$2 ORDER BY revision DESC", &[&board.as_uuid(),&item.as_uuid()])?.into_iter().map(|r| Ok(Entry{actor_id:r.try_get(0)?,actor_name:r.try_get(1)?,occurred_at:r.try_get(2)?,field:r.try_get(3)?,before:r.try_get(4)?,after:r.try_get(5)?,revision:r.try_get(6)?})).collect()
}
